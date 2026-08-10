//! Auto-link, formal-URL-link, and `link:`/`mailto:` macro recognition.

use super::{MacroMatch, MacroMatchKind, image::range_is_verbatim, rebuild_macro_level};
use crate::{
    Parser, Span,
    content::{
        INLINE_LINK, INLINE_LINK_MACRO, NormalizedCaps, URI_SNIFF,
        inline_builder::quotes::{Piece, build_match_string, source_slice},
    },
    inlines::{InlineNode, Ref, RefVariant},
    parser::has_dangerous_scheme,
    strings::CowStr,
};

/// The auto-link / formal-URL-link pass at a level: matches [`INLINE_LINK`]
/// over the level's escaped text and replaces each verbatim, recognized match
/// with the [`Ref`](InlineNode::Ref) link node it produces.
///
/// # Scope
///
/// This increment covers [`INLINE_LINK`]'s **non-angle branch** – a bare
/// auto-linked URL (`https://example.org`) and a formal URL link
/// (`https://example.org[text]`, `https://example.org[]`) – in its verbatim,
/// attribute-list-free forms, reproducing the string replacer's boundary-prefix
/// preservation, bare-URL trailing-punctuation stripping, `^` new-window
/// suffix, `hide-uri-scheme` display text, and `\` scheme escape. It
/// deliberately leaves several forms **unrecognized** for a later increment,
/// each left as literal source here (so the differential corpus only pins the
/// forms this increment claims):
///
/// - The **angle-bracketed URL** form (`<https://example.org>`) requires a
///   leading `&lt;`, which is always an escaped
///   [`CharRef`](InlineNode::CharRef) by the time macros run – so its match is
///   never verbatim, exactly the boundary the image increment documents.
/// - The **`link:` URL macro** form (`link:https://example.org[text]`, the
///   pattern's LINK-MACRO branch) is left to [`link_macro_level`], which folds
///   the identical node; running that pass second mirrors the string step's
///   order.
/// - A **formal text carrying an attribute list** (an `=` selecting roles / id
///   / title / window) is deferred, exactly as [`link_macro_level`] defers it:
///   the attribute list is parsed from a newline-normalized *copy* of the text,
///   so it cannot ride on the node as an
///   [`Attrlist`](crate::attributes::Attrlist)`<'src>` yet.
/// - A **non-verbatim match** – a URL crossing an escaped special
///   ([`CharRef`](InlineNode::CharRef)) or a rendered
///   [`Styled`](crate::inlines::Styled) span – is deferred exactly as the image
///   increment defers `image:a&b.png[]`.
///
/// An invalid quoted bare URL (`"https://example.org`) and a bare scheme with no
/// body (`http://;`) are left literal by the string step *and* the builder, so
/// they render identically and are covered by the differential corpus rather
/// than a divergence test.
pub(super) fn inline_link_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes);

    // Cheap pre-filter mirroring the string step's guard: an auto-link needs a
    // `://` scheme separator somewhere in the level.
    if !s.contains("://") {
        return nodes;
    }

    let matches = find_inline_link_matches(&s, &pieces, root, parser);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every recognized auto-link / formal-URL link at this level, skipping
/// an angle-bracketed or `link:` macro match, a non-verbatim match, and any
/// form [`build_inline_link_node`] defers (see [`inline_link_level`]).
fn find_inline_link_matches<'src>(
    s: &str,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Vec<MacroMatch<'src>> {
    let mut matches = Vec::new();

    for caps in INLINE_LINK.captures_iter(s) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();

        let full = whole.start()..whole.end();

        let n = NormalizedCaps::new(&caps);

        // The angle-bracketed form always crosses a `&lt;` `CharRef` (so it is
        // never verbatim), and the `link:` URL macro form is left to
        // `link_macro_level`, which folds the identical node.
        if n.is_angle() || n.is_link_macro() {
            continue;
        }

        // Only a wholly-verbatim match can slice its target/text from `'src`; a
        // match crossing an escaped special or a rendered span is left for a
        // later increment.
        if !range_is_verbatim(pieces, &full) {
            continue;
        }

        match build_inline_link_node(&n, &full, pieces, root, parser) {
            Some(m) => matches.push(m),

            // A deferred or invalid form (an escaped scheme is handled inside as
            // an `Unescape`; a quoted bare URL, a bare scheme, or an
            // attribute-list text is left as literal source).
            None => continue,
        }
    }

    matches
}

/// Builds one [`MacroMatch`] for a verbatim non-angle [`INLINE_LINK`] match: a
/// [`Ref`](InlineNode::Ref) link node (with the boundary prefix kept before it
/// and any stripped trailing punctuation kept after it), or an
/// [`Unescape`](MacroMatchKind::Unescape) for an escaped scheme. Returns `None`
/// for a form this increment defers or that the string step leaves literal (see
/// [`inline_link_level`]).
///
/// The value computation mirrors [`InlineLinkReplacer`] exactly – target, bare
/// vs. labeled display text, `hide-uri-scheme`, the `^` window suffix, and the
/// trailing-punctuation strip – so the fold reproduces the same bytes through
/// the same `render_link` [`link_macro_level`]'s nodes fold through.
///
/// Like every macro family in this additive builder, it deliberately performs
/// *no* recognition side effect: it does **not** `register_link` the target in
/// the document's asset catalog, because the builder is not yet the
/// authoritative recognition sink – the string pipeline still registers it, and
/// registering it here too would double-count it. The cutover (design §5.2
/// Phase 4, step 6) re-attaches this registration, so
/// `Document::catalog().links()` stays populated by the string pipeline until
/// then. (The same applies to the `link:`/`mailto:` macro node built by
/// [`build_link_node`].)
///
/// [`InlineLinkReplacer`]: crate::content::macros
fn build_inline_link_node<'src>(
    n: &NormalizedCaps<'_, '_>,
    full: &std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Option<MacroMatch<'src>> {
    // `scheme_match` is always present in a non-angle match (the branch cannot
    // match without it); fall through defensively if it is somehow absent.
    let scheme_m = n.scheme_match()?;
    let scheme = scheme_m.as_str();

    // An escaped scheme (`\https://…`) keeps the boundary prefix and drops the
    // single backslash, leaving the rest of the match literal – no link node.
    if scheme.starts_with('\\') {
        return Some(MacroMatch {
            kind: MacroMatchKind::Unescape {
                backslash: scheme_m.start(),
            },
            full: full.clone(),
        });
    }

    let prefix = n.prefix_str();

    // The URL body is the formal-macro target group or, for a bare link, the
    // bare group; the two are mutually exclusive.
    let url_part = n.target().or_else(|| n.bare()).map_or("", |m| m.as_str());
    let mut target = format!("{scheme}{url_part}");

    // The node consumes the URL (and, for a formal macro, its `[…]` attrlist)
    // but not the boundary prefix; a bare URL additionally stops short of any
    // trailing punctuation the string step strips out.
    let mut consumed_end = full.end;

    let mut link_text: Option<String> = None;

    if let Some(attrlist_m) = n.attrlist() {
        // A formal URL link: the bracketed text is the display text (empty means
        // a bare link, handled by the shared post-processing below).
        if !attrlist_m.is_empty() {
            link_text = Some(attrlist_m.as_str().to_string());
        }
    } else {
        // A bare auto-link.

        // A URL wrapped in quotes with no brackets is invalid macro syntax; the
        // string step leaves the whole match literal.
        if prefix == "\"" || prefix == "'" {
            return None;
        }

        // Strip a trailing ';' or ':' (and an adjacent ')') off a bare URL,
        // keeping it as literal text after the link – mirroring the string
        // replacer, which keys off the target's final character.
        let mut stripped = 0usize;

        if target.ends_with([';', ':']) {
            target.truncate(target.len() - 1);
            stripped += 1;

            if target.ends_with(')') {
                target.truncate(target.len() - 1);
                stripped += 1;
            }
        }

        // A bare scheme with nothing left after trimming is not a link; leave it
        // literal, exactly as the string step does.
        if target.ends_with("://") {
            return None;
        }

        // The bare group is the last group in the match, so the URL ends at
        // `full.end`; the node stops short of the stripped punctuation, which the
        // stripped bytes (ASCII) place `stripped` bytes back.
        consumed_end = full.end - stripped;
    }

    // The display text becomes the node's children, located at the bracketed
    // text (a formal link) or the node itself (a bare link). Its value is a
    // computed (owned) string, so this is a *synthesized* `Text`.
    let text_location_range = n.attrlist().map(|m| m.start()..m.end());

    let mut window: Option<CowStr<'src>> = None;
    let mut bare = false;

    let link_text = if let Some(mut link_text) = link_text {
        link_text = link_text.replace("\\]", "]");

        // A text carrying an `=` splits into an attribute list, parsed from a
        // newline-normalized copy of the text (not from `'src`); defer the whole
        // macro until the node can carry an `Attrlist<'src>`.
        if link_text.contains('=') {
            return None;
        }

        if link_text.ends_with('^') {
            link_text.truncate(link_text.len() - 1);
            window = Some(CowStr::from("_blank"));
        }

        if link_text.is_empty() {
            bare = true;
            hide_uri_scheme_text(&target, parser)
        } else {
            link_text
        }
    } else {
        bare = true;
        hide_uri_scheme_text(&target, parser)
    };

    let mut roles: Vec<CowStr<'src>> = vec![];
    if bare {
        roles.push(CowStr::from("bare"));
    }

    let consumed = scheme_m.start()..consumed_end;
    let location = source_slice(pieces, consumed.clone(), root);

    let text_location =
        text_location_range.map_or(location, |range| source_slice(pieces, range, root));

    let children = vec![InlineNode::Text {
        value: CowStr::from(link_text),
        location: text_location,
    }];

    let node = InlineNode::Ref(Ref {
        variant: RefVariant::Link,
        target: CowStr::from(target),
        children,
        roles,
        window,
        resolved: None,
        derived: None,
        location,
    });

    Some(MacroMatch {
        kind: MacroMatchKind::Node {
            consumed,
            node: Box::new(node),
        },
        full: full.clone(),
    })
}

/// The display text for a bare link, dropping the URI scheme under
/// `hide-uri-scheme` exactly as the string replacer's `URI_SNIFF` strip does.
/// Unlike a bare `link:`/`mailto:` macro, [`INLINE_LINK`]'s bare branch does
/// *not* fall back to the whole target when the strip leaves nothing – it
/// cannot leave nothing, because a bare scheme with no body is rejected
/// upstream.
fn hide_uri_scheme_text(target: &str, parser: &Parser) -> String {
    if parser.is_attribute_set("hide-uri-scheme") {
        URI_SNIFF.replace_all(target, "").into_owned()
    } else {
        target.to_string()
    }
}

/// The `link:`/`mailto:` macro pass at a level: matches [`INLINE_LINK_MACRO`]
/// over the level's escaped text and replaces each verbatim, recognized match
/// with the [`Ref`](InlineNode::Ref) link node it produces.
///
/// # Scope
///
/// This increment covers the **explicit `link:`/`mailto:` macro** in its
/// verbatim, attribute-list-free forms: `link:target[text]`, `link:target[]`
/// (a bare link), `mailto:addr[text]`, and `mailto:addr[]`, plus the `^`
/// new-window suffix and the `\` escape. It deliberately leaves several forms
/// **unrecognized** for a later increment, each left as literal source here (so
/// the differential corpus only pins the forms this increment claims):
///
/// - **Auto-links and formal-URL links** (`https://example.org`, `https://example.org[text]`)
///   are matched by a *different* pattern (`INLINE_LINK`, with its bare-URL
///   trailing-punctuation handling) and are a separate later increment.
/// - **A link text that carries an attribute list** – a `,` in a `mailto:` text
///   (its `subject`/`body`) or an `=` in a `link:` text (roles / id / title /
///   window) – is deferred, because that attribute list is parsed from a
///   newline-normalized *copy* of the text (not from `'src`) and so cannot be
///   carried as an [`Attrlist`](crate::attributes::Attrlist)`<'src>` on the
///   node yet.
/// - **A non-verbatim match** – a macro whose target or text crosses an escaped
///   special ([`CharRef`](InlineNode::CharRef)) or a rendered
///   [`Styled`](crate::inlines::Styled) span (`link:a&b[]`, `link:x[*bold*]`) –
///   is deferred exactly as the image increment defers `image:a&b.png[]`.
///
/// A `link:` (not `mailto:`) target whose scheme could execute script
/// (`javascript:`, `data:`, `vbscript:`) is likewise left literal – matching
/// the string step, which neutralizes it, so it renders identically; the
/// additive builder simply skips the `record_substitution_warning` side effect
/// the string step performs there.
pub(super) fn link_macro_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes);

    // Cheap pre-filter mirroring the string step's guard: a link/mailto macro
    // needs its prefix and an opening bracket.
    if !((s.contains("link:") || s.contains("mailto:")) && s.contains('[')) {
        return nodes;
    }

    let matches = find_link_macro_matches(&s, &pieces, root, parser);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every recognized `link:`/`mailto:` macro at this level, skipping any
/// match that is not wholly verbatim source or that this increment defers (see
/// [`link_macro_level`]).
fn find_link_macro_matches<'src>(
    s: &str,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Vec<MacroMatch<'src>> {
    let mut matches = Vec::new();

    for caps in INLINE_LINK_MACRO.captures_iter(s) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();

        let full = whole.start()..whole.end();

        // Only a wholly-verbatim match can slice its target/text from `'src`; a
        // match crossing an escaped special or a rendered span is left for a
        // later increment.
        if !range_is_verbatim(pieces, &full) {
            continue;
        }

        if whole.as_str().starts_with('\\') {
            matches.push(MacroMatch {
                kind: MacroMatchKind::Unescape {
                    backslash: full.start,
                },
                full,
            });

            continue;
        }

        match build_link_node(&caps, &full, pieces, root, parser) {
            Some(node) => matches.push(MacroMatch {
                kind: MacroMatchKind::Node {
                    consumed: full.clone(),
                    node: Box::new(node),
                },
                full,
            }),

            // A deferred form (an attribute-list-in-text macro, or a rejected
            // dangerous `link:` scheme) is left as literal source: parity with
            // the string step for the dangerous case, a documented divergence
            // for the attribute-list case.
            None => continue,
        }
    }

    matches
}

/// Builds one [`Ref`](InlineNode::Ref) link node from a verbatim
/// `link:`/`mailto:` match, computing the target, display text, window, and
/// roles exactly as the string replacer does so the fold reproduces the same
/// bytes. Returns `None` for a form this increment defers (see
/// [`link_macro_level`]): a rejected dangerous `link:` scheme, or a link text
/// that carries an attribute list.
///
/// The display text is baked into a single [`Text`](InlineNode::Text) child, so
/// the fold recovers `link_text` by folding the children and needs no
/// build-time state (bare-vs-labeled, `hide-uri-scheme`, `mailto:`) at fold
/// time; the `bare` role, when the string step would add one, rides on the
/// node's `roles`.
///
/// As in the additive builder generally, this performs *no* recognition side
/// effect – notably it does **not** `register_link` the target in the asset
/// catalog, which the string replacer does; the cutover (design §5.2 Phase 4,
/// step 6) re-attaches that (see [`build_inline_link_node`]).
pub(super) fn build_link_node<'src>(
    caps: &regex::Captures<'_>,
    full: &std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Option<InlineNode<'src>> {
    let location = source_slice(pieces, full.clone(), root);

    // Group 1 present ⟺ a `mailto:` macro; group 3 is the (optional) target;
    // group 5 is the (optional) bracketed text.
    let is_mailto = caps.get(1).is_some();
    let target_str = caps.get(3).map_or("", |m| m.as_str());

    let target = if is_mailto {
        format!("mailto:{target_str}")
    } else {
        target_str.to_string()
    };

    // A `link:` target whose scheme could execute script is neutralized by the
    // string step (left literal); mirror that by deferring – the node would
    // otherwise render a live, dangerous link. `mailto:` carries its own safe
    // scheme and is exempt.
    if !is_mailto && has_dangerous_scheme(&target) {
        return None;
    }

    let mut window: Option<CowStr<'src>> = None;

    let mut link_text = caps
        .get(5)
        .map_or_else(String::new, |m| m.as_str().to_string());

    if !link_text.is_empty() {
        // An attribute list embedded in the text (`mailto:` subject/body via a
        // comma, or `link:` roles/id/title via an `=`) is parsed from a
        // newline-normalized copy of the text, so it cannot be carried as an
        // `Attrlist<'src>` on the node yet; defer the whole macro.
        if (is_mailto && link_text.contains(',')) || (!is_mailto && link_text.contains('=')) {
            return None;
        }

        link_text = link_text.replace("\\]", "]");

        if link_text.ends_with('^') {
            link_text.truncate(link_text.len() - 1);
            window = Some(CowStr::from("_blank"));
        }
    }

    let mut roles: Vec<CowStr<'src>> = vec![];

    if link_text.is_empty() {
        if is_mailto {
            // A bare `mailto:` shows the address (group 3) and takes no `bare`
            // role.
            link_text = target_str.to_string();
        } else {
            // A bare `link:` shows the target (with the scheme optionally hidden)
            // and takes the `bare` role.
            link_text = if parser.is_attribute_set("hide-uri-scheme") {
                let stripped = URI_SNIFF.replace_all(&target, "").into_owned();

                if stripped.is_empty() {
                    target.clone()
                } else {
                    stripped
                }
            } else {
                target.clone()
            };

            roles.push(CowStr::from("bare"));
        }
    }

    // The display text becomes the node's children, located at the bracketed
    // text (or the whole macro when there is none). `link_text` is a computed
    // (owned) value, so this is a *synthesized* `Text` whose value need not
    // coincide with its source.
    let text_location = caps
        .get(5)
        .map_or(location, |m| source_slice(pieces, m.start()..m.end(), root));

    let children = if link_text.is_empty() {
        vec![]
    } else {
        vec![InlineNode::Text {
            value: CowStr::from(link_text),
            location: text_location,
        }]
    };

    Some(InlineNode::Ref(Ref {
        variant: RefVariant::Link,
        target: CowStr::from(target),
        children,
        roles,
        window,
        resolved: None,
        derived: None,
        location,
    }))
}

/// Performs the recognition side effect the string pipeline's four link
/// replacers (`InlineLinkReplacer`'s angle/formal/bare branches and
/// `InlineLinkMacroReplacer`, plus `InlineEmailReplacer`'s own registration for
/// the bare e-mail form this module does not yet build) attach to a matched
/// link – registering the target in the document's asset catalog – by walking
/// an already-built tree and reading each [`Ref`](InlineNode::Ref)`{Link}`
/// node's own stored `target` instead of a regex capture. `target` already
/// holds exactly the string the string pipeline registers (see
/// [`build_inline_link_node`] and [`build_link_node`]), so no recomputation is
/// needed.
///
/// Every macro family this module recognizes defers exactly this kind of side
/// effect (see [`image::apply_image_side_effects`](super::image::apply_image_side_effects)'s
/// own note): while the additive builder runs *alongside* the authoritative
/// string pipeline – each against its own, independent [`Parser`] – performing
/// it from every additive pass would risk double-counting a registration once
/// the two paths ever share one `Parser`. This function is that deferred piece
/// for the link family, staged as its own building block for the eventual
/// cutover (design §5.2, Phase 4 step 6): re-attaching it for real means
/// calling it exactly once per parse, after the single-pass builder replaces
/// the recorder as `Content`'s tree source, so nothing here is wired into a
/// real parse yet – it is exercised only by this module's own tests, against
/// their own `Parser`.
///
/// # Registration order across the two link forms
///
/// The string pipeline registers a link's target *when its own replacer's
/// regex pass matches it* – `InlineLinkReplacer` (auto-links and formal-URL
/// links, `INLINE_LINK`'s non-angle branch) runs as one whole-string pass, then
/// `InlineLinkMacroReplacer` (`link:`/`mailto:`, `INLINE_LINK_MACRO`) runs as a
/// *second*, later pass – exactly the order [`inline_link_level`] and
/// [`link_macro_level`] apply the two families in. So the catalog ends up in
/// **family-pass order, not true source order**: every auto-link/formal-URL
/// link in the content registers before every `link:`/`mailto:` macro in it,
/// regardless of which appears first in the source (see
/// `catalog_records_link_targets_when_catalog_assets_enabled` in
/// `tests/asciidoctor_rb/substitutions_test.rs`, which pins this exact
/// behavior). A single tree walk in document order would get this wrong for a
/// content that interleaves the two forms out of that relative order (for
/// example `link:b.html[B] then https://a.example`, which the golden pipeline
/// registers as `["https://a.example", "b.html"]`, not `["b.html",
/// "https://a.example"]`), so this function makes **two** passes over the
/// tree – all auto-link/formal-URL matches first, then all `link:`/`mailto:`
/// macro matches – rather than one. [`link_form`] tells the two apart from the
/// node's own `location` (a `link:`/`mailto:` macro's location always starts
/// with its literal prefix; [`inline_link_level`] never builds a node for
/// `INLINE_LINK`'s own link-macro branch, deferring that whole form to
/// [`link_macro_level`] – see [`inline_link_level`]'s own doc comment – so this
/// is a reliable, no-recomputation signal, not a heuristic).
///
/// Recurses into every container a `Ref` node can be nested inside –
/// [`Styled`](InlineNode::Styled), another [`Ref`](InlineNode::Ref) (a link's
/// own display children, or a cross-reference's), and
/// [`Footnote`](InlineNode::Footnote) children – mirroring exactly where
/// [`apply_macros`](super::apply_macros) and the footnote increment's own
/// `emit_range` can place one. A cross-reference node itself is not
/// registered – only a [`Link`](RefVariant::Link) has an asset-catalog entry –
/// but its children are still walked, since a formatted cross-reference text
/// could itself carry a nested link.
pub(crate) fn apply_link_side_effects(nodes: &[InlineNode<'_>], parser: &Parser) {
    register_links_of_form(nodes, parser, LinkForm::AutoOrFormal);
    register_links_of_form(nodes, parser, LinkForm::Macro);
}

/// Which of the two link-recognizing passes built a [`Ref`](InlineNode::Ref)
/// node – see [`apply_link_side_effects`]'s own "Registration order" note.
#[derive(Clone, Copy, Eq, PartialEq)]
enum LinkForm {
    /// An auto-link or formal-URL link, built by [`inline_link_level`].
    AutoOrFormal,

    /// A `link:`/`mailto:` macro, built by [`link_macro_level`].
    Macro,
}

/// Walks `nodes`, registering only the [`Ref`](InlineNode::Ref)`{Link}` nodes
/// of the given `form`, in document order.
fn register_links_of_form(nodes: &[InlineNode<'_>], parser: &Parser, form: LinkForm) {
    for node in nodes {
        match node {
            InlineNode::Ref(reference) => {
                if reference.variant == RefVariant::Link && link_form(reference) == form {
                    parser.register_link(reference.target.to_string());
                }

                register_links_of_form(&reference.children, parser, form);
            }

            InlineNode::Styled(styled) => {
                register_links_of_form(&styled.children, parser, form);
            }

            InlineNode::Footnote(footnote) => {
                register_links_of_form(&footnote.children, parser, form);
            }

            _ => {}
        }
    }
}

/// Tells which pass built a link [`Ref`](InlineNode::Ref) node from its own
/// `location`: only [`link_macro_level`] ever builds a node whose matched
/// source starts with a literal `link:`/`mailto:` prefix (see
/// [`apply_link_side_effects`]'s own doc comment).
fn link_form(reference: &Ref<'_>) -> LinkForm {
    let text = reference.location.data();

    if text.starts_with("link:") || text.starts_with("mailto:") {
        LinkForm::Macro
    } else {
        LinkForm::AutoOrFormal
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use super::super::super::test_support::{
        assert_link, assert_styled, assert_text, build_src, fold_html, golden_macros,
        golden_macros_with, link_text_of,
    };
    use crate::{
        Parser, Span,
        content::inline_builder::build,
        inlines::{InlineNode, SpanForm, StyleVariant},
        parser::HtmlSubstitutionRenderer,
        strings::CowStr,
    };

    #[test]
    fn fold_matches_the_string_pipeline_through_link_macros() {
        // For each fixture, folding the single-pass tree (all five steps)
        // reproduces the string pipeline's output byte-for-byte. This is the
        // differential corpus (design §5.3) that pins the `link:`/`mailto:`
        // macro increment. Every fixture is a *verbatim*, attribute-list-free
        // `link:`/`mailto:` macro – the boundary this increment claims (a URL
        // target, an attribute list in the text, or a special character inside
        // the macro is deferred and lives in a divergence test below).
        let fixtures = [
            // No link macro despite macro-ish characters.
            "plain text with a colon: but no bracket",
            "a link without a bracket link:index.html stays literal",
            // Link macro: labeled, bare, relative and pathed targets.
            "link:index.html[Docs]",
            "link:downloads/report.pdf[Report]",
            "link:index.html[]",
            "link:index.html[Read the docs]",
            // The `^` suffix opens the link in a new window.
            "link:index.html[Open^]",
            // An escaped `]` inside the text is unescaped.
            "link:index.html[a\\]b]",
            // mailto: labeled and bare (bare shows the address).
            "mailto:hello@example.org[Email us]",
            "mailto:hello@example.org[]",
            // Degenerate empty targets: a bare link/mailto with no display text.
            "link:[]",
            "mailto:[]",
            // A macro embedded in surrounding flow, and next to other constructs.
            "See link:about.html[about] for details.",
            "*bold* then link:x.html[X] and _em_",
            "a copyright (C) then link:x.html[X]",
            // Escapes: the macro stays literal, minus the backslash.
            "\\link:index.html[Docs]",
            "\\mailto:hello@example.org[Email]",
            // A `link:` target whose scheme could execute script is left literal
            // by the string step *and* the builder, so it renders identically.
            "link:javascript:alert(1)[Click]",
            // A macro inside a rendered span (recognized inside the span body).
            "*see link:x.html[X]*",
            "_link:y.html[Y] in em_",
        ];

        let renderer = HtmlSubstitutionRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_src(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden_macros(fixture),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn fold_matches_the_string_pipeline_with_hide_uri_scheme() {
        // Under `hide-uri-scheme`, a bare link's display text drops the URI
        // scheme; the builder reproduces the string step's `URI_SNIFF` stripping,
        // including the fall-back to the whole target when the strip leaves
        // nothing. Every fixture is a non-`://` `link:` target, so the string
        // pipeline routes it through the same `INLINE_LINK_MACRO` the builder
        // uses (a `://` target is `INLINE_LINK`'s territory, a later increment).
        use crate::parser::ModificationContext;

        let parser = Parser::default().with_intrinsic_attribute_bool(
            "hide-uri-scheme",
            true,
            ModificationContext::Anywhere,
        );

        let fixtures = [
            // A scheme prefix is stripped, leaving the remainder as the text.
            "link:foo:bar[]",
            // The whole target is a scheme: the strip leaves nothing, so the
            // text falls back to the target itself.
            "link:foo:[]",
            // No scheme to strip: the target shows unchanged.
            "link:index.html[]",
            // A bare mailto shows the address regardless of `hide-uri-scheme`.
            "mailto:hello@example.org[]",
        ];

        let renderer = HtmlSubstitutionRenderer {};

        for fixture in fixtures {
            let folded = crate::content::inline_builder::fold_html(
                &build(Span::new(fixture), &parser),
                &renderer,
                &parser,
            );

            assert_eq!(
                folded,
                golden_macros_with(fixture, &parser),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn a_link_macro_becomes_a_ref_node() {
        let nodes = build_src(Span::new("link:index.html[Docs]"));

        assert_eq!(nodes.len(), 1);
        let reference = assert_link(&nodes[0]);

        assert_eq!(reference.target.as_ref(), "index.html");
        assert_eq!(link_text_of(reference), "Docs");
        assert!(reference.roles.is_empty());
        assert_eq!(reference.window, None);
        assert_eq!(reference.resolved, None);

        // Its location covers the whole macro, delimiters included.
        assert_eq!(reference.location.data(), "link:index.html[Docs]");
        assert_eq!(reference.location.line(), 1);
        assert_eq!(reference.location.col(), 1);
    }

    #[test]
    fn a_bare_link_takes_the_bare_role() {
        // An empty text is a bare link: the display text is the target, and the
        // `bare` role rides on the node so the fold reproduces `class="bare"`.
        let nodes = build_src(Span::new("link:index.html[]"));

        let reference = assert_link(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "index.html");
        assert_eq!(link_text_of(reference), "index.html");
        assert_eq!(reference.roles, [CowStr::from("bare")]);
    }

    #[test]
    fn a_mailto_macro_targets_the_address() {
        // A labeled mailto prefixes the address with `mailto:` and shows the
        // label; a bare mailto shows the address itself and takes no `bare` role.
        let labeled = build_src(Span::new("mailto:hello@example.org[Email us]"));
        let reference = assert_link(&labeled[0]);
        assert_eq!(reference.target.as_ref(), "mailto:hello@example.org");
        assert_eq!(link_text_of(reference), "Email us");
        assert!(reference.roles.is_empty());

        let bare = build_src(Span::new("mailto:hello@example.org[]"));
        let reference = assert_link(&bare[0]);
        assert_eq!(reference.target.as_ref(), "mailto:hello@example.org");
        assert_eq!(link_text_of(reference), "hello@example.org");
        assert!(
            reference.roles.is_empty(),
            "a bare mailto takes no `bare` role"
        );
    }

    #[test]
    fn a_link_window_suffix_opens_blank() {
        // A trailing `^` in the text is stripped and selects the `_blank`
        // window, exactly as the string replacer does.
        let nodes = build_src(Span::new("link:index.html[Open^]"));

        let reference = assert_link(&nodes[0]);
        assert_eq!(link_text_of(reference), "Open");
        assert_eq!(reference.window.as_deref(), Some("_blank"));
    }

    #[test]
    fn an_escaped_link_macro_stays_literal() {
        // `\link:…` drops the backslash and keeps the macro as literal text – no
        // link node.
        let nodes = build_src(Span::new("\\link:index.html[Docs]"));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an escaped macro must not produce a link node: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros("\\link:index.html[Docs]")
        );
    }

    #[test]
    fn a_dangerous_link_scheme_is_left_literal() {
        // A `link:` target whose scheme could execute script is neutralized by
        // the string step (left literal); the builder mirrors that by leaving
        // the macro unrecognized, so the two render identically. (The builder
        // additionally skips the warning side effect the string step records.)
        let source = "link:javascript:alert(1)[Click]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "a dangerous link scheme must not produce a link node: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn a_link_over_a_special_character_is_a_documented_divergence() {
        // The string pipeline matches macros over *escaped* text, so a target
        // containing `&` is matched as `a&amp;b.html`. A self-describing node
        // cannot carry that escaped text as an `'src` slice, so the single-pass
        // builder leaves such a macro *unrecognized* for a later increment,
        // exactly as the image increment defers `image:a&b.png[]`.
        let nodes = build_src(Span::new("link:a&b.html[x]"));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "a macro crossing an escaped special must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* build a link here.
        assert!(golden_macros("link:a&b.html[x]").contains("<a href"));
    }

    #[test]
    fn a_link_text_attribute_list_is_a_documented_divergence() {
        // A `link:` text carrying an `=` splits into an attribute list (here a
        // role), which the string replacer parses from a newline-normalized copy
        // of the text – not from `'src`. The builder cannot carry that as an
        // `Attrlist<'src>` yet, so it defers the whole macro (left literal),
        // pending the increment that adds an attribute list to the link node.
        let source = "link:index.html[Docs,role=hl]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an attribute-list-in-text link must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, applies the role.
        assert!(golden_macros(source).contains(r#"class="hl""#));
    }

    #[test]
    fn a_mailto_subject_is_a_documented_divergence() {
        // A `mailto:` text carrying a `,` encodes a `subject` (and optional
        // `body`) into the target – the same attribute-list-from-a-copy handling
        // the builder defers.
        let source = "mailto:team@example.org[Team,Hello there]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "a mailto with a subject must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, encodes the subject into the href.
        assert!(golden_macros(source).contains("subject="));
    }

    #[test]
    fn a_link_is_recognized_inside_a_span() {
        // A macro can appear inside a rendered span; the transducer descends into
        // the span body and builds the node there.
        let nodes = build_src(Span::new("*see link:x.html[X]*"));

        let children = assert_styled(&nodes[0], StyleVariant::Strong, SpanForm::Constrained);
        assert_eq!(children.len(), 2);
        assert_text(&children[0], "see ", 1, 2);

        let reference = assert_link(&children[1]);
        assert_eq!(reference.target.as_ref(), "x.html");
        assert_eq!(link_text_of(reference), "X");
    }

    #[test]
    fn fold_matches_the_string_pipeline_through_inline_links() {
        // For each fixture, folding the single-pass tree (all five steps)
        // reproduces the string pipeline's output byte-for-byte. This is the
        // differential corpus (design §5.3) that pins the auto-link / formal-URL
        // link increment. Every fixture is a *verbatim*, attribute-list-free
        // link – the boundary this increment claims (an angle form, a URL
        // crossing a special, or an attribute-list text is deferred and lives in
        // a divergence test below).
        let fixtures = [
            // No auto-link despite a colon or a `//`.
            "plain text with a colon: but no scheme",
            "a bare host example.org stays literal",
            // Bare auto-links: at the start, mid-flow, and with a path/query.
            "https://example.org",
            "Visit https://example.org for details.",
            "https://example.org/path/to/page.html",
            "https://example.org/search?q=rust",
            // Other schemes the pattern recognizes.
            "ftp://ftp.example.org/pub",
            "irc://irc.example.org/channel",
            "file:///etc/hosts here",
            // A trailing sentence period is left outside the link (the pattern
            // stops before it); a trailing ';'/':' is stripped back out.
            "See https://example.org. Next sentence.",
            "https://example.org; and more",
            "read https://example.org: really",
            // A bracketed / parenthesized URL keeps the surrounding punctuation.
            "(see https://example.org)",
            "[https://example.org]",
            // A ')' adjacent to a stripped ';'/':' is stripped out with it,
            // keeping both as literal text after the link.
            "See (https://example.org); done.",
            "read (https://example.org): really",
            // Formal URL links: labeled, bare, and the `^` new-window suffix.
            "https://example.org[Example]",
            "https://example.org[]",
            "https://example.org[Open^]",
            // A formal text that is only the `^` suffix becomes an empty, bare,
            // new-window link (the display text falls back to the target).
            "https://example.org[^]",
            // An escaped `]` inside the text is unescaped.
            "https://example.org[a\\]b]",
            // A bare scheme with nothing left after trimming is left literal by
            // both (a `://`-only rejection).
            "http://; is not a link",
            // A URL wrapped in quotes with no brackets is invalid macro syntax:
            // left literal by both.
            "\"https://example.org\" in quotes",
            // Escapes: the scheme's backslash is dropped and the URL stays
            // literal (no link), the boundary prefix preserved.
            "\\https://example.org",
            "see \\https://example.org here",
            "\\https://example.org[text]",
            // Next to other constructs, and inside a rendered span.
            "*bold* then https://example.org and _em_",
            "a copyright (C) then https://example.org",
            "*see https://example.org*",
            "_https://example.org in em_",
        ];

        let renderer = HtmlSubstitutionRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_src(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden_macros(fixture),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_auto_links_with_hide_uri_scheme() {
        // Under `hide-uri-scheme`, a bare auto-link's display text drops the URI
        // scheme; the builder reproduces the string step's `URI_SNIFF` stripping.
        use crate::parser::ModificationContext;

        let parser = Parser::default().with_intrinsic_attribute_bool(
            "hide-uri-scheme",
            true,
            ModificationContext::Anywhere,
        );

        let fixtures = [
            // A bare auto-link shows the scheme-stripped URL.
            "https://example.org",
            "Visit https://example.org for details.",
            // A formal bare link (`[]`) likewise drops the scheme.
            "https://example.org[]",
            // A labeled link keeps its explicit text unchanged.
            "https://example.org[Example]",
        ];

        let renderer = HtmlSubstitutionRenderer {};

        for fixture in fixtures {
            let folded = crate::content::inline_builder::fold_html(
                &build(Span::new(fixture), &parser),
                &renderer,
                &parser,
            );

            assert_eq!(
                folded,
                golden_macros_with(fixture, &parser),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn a_bare_auto_link_becomes_a_ref_node() {
        // A bare URL is a link whose display text is the target and which takes
        // the `bare` role, so the fold reproduces `class="bare"`.
        let nodes = build_src(Span::new("https://example.org"));

        assert_eq!(nodes.len(), 1);
        let reference = assert_link(&nodes[0]);

        assert_eq!(reference.target.as_ref(), "https://example.org");
        assert_eq!(link_text_of(reference), "https://example.org");
        assert_eq!(reference.roles, [CowStr::from("bare")]);
        assert_eq!(reference.window, None);
        assert_eq!(reference.resolved, None);

        // Its location covers the whole URL.
        assert_eq!(reference.location.data(), "https://example.org");
        assert_eq!(reference.location.line(), 1);
        assert_eq!(reference.location.col(), 1);
    }

    #[test]
    fn a_bare_auto_link_keeps_its_boundary_prefix() {
        // The boundary character before the URL (here a space) is not part of
        // the link – it stays as literal text before the node.
        let nodes = build_src(Span::new("Visit https://example.org now"));

        // Text "Visit ", the link, then Text " now".
        assert_eq!(nodes.len(), 3);
        assert_text(&nodes[0], "Visit ", 1, 1);

        let reference = assert_link(&nodes[1]);
        assert_eq!(reference.target.as_ref(), "https://example.org");
        assert_eq!(reference.location.col(), 7);

        assert_text(&nodes[2], " now", 1, 26);
    }

    #[test]
    fn a_bare_auto_link_strips_trailing_punctuation() {
        // A trailing ';' is stripped off the target and kept as literal text
        // after the link, exactly as the string replacer does.
        let nodes = build_src(Span::new("https://example.org; done"));

        let reference = assert_link(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "https://example.org");
        assert_eq!(reference.location.data(), "https://example.org");

        // The stripped ';' is kept as its own literal run, then the rest.
        assert_text(&nodes[1], ";", 1, 20);
        assert_text(&nodes[2], " done", 1, 21);
    }

    #[test]
    fn a_bare_auto_link_strips_a_trailing_paren_before_punctuation() {
        // A ')' adjacent to a stripped ';' is stripped out with it: the link
        // covers only the URL, and both trailing characters are kept as literal
        // text after it (a single run).
        let nodes = build_src(Span::new("(https://example.org);"));

        // Text "(", the link, then Text ");".
        assert_eq!(nodes.len(), 3);
        assert_text(&nodes[0], "(", 1, 1);

        let reference = assert_link(&nodes[1]);
        assert_eq!(reference.target.as_ref(), "https://example.org");
        assert_eq!(reference.location.data(), "https://example.org");

        assert_text(&nodes[2], ");", 1, 21);
    }

    #[test]
    fn a_formal_url_link_that_is_only_a_window_suffix_is_bare() {
        // A `[^]` text is empty once the `^` is stripped, so the link is bare
        // (its display text falls back to the target) *and* opens a new window.
        let nodes = build_src(Span::new("https://example.org[^]"));

        let reference = assert_link(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "https://example.org");
        assert_eq!(link_text_of(reference), "https://example.org");
        assert_eq!(reference.roles, [CowStr::from("bare")]);
        assert_eq!(reference.window.as_deref(), Some("_blank"));
    }

    #[test]
    fn a_formal_url_link_becomes_a_ref_node() {
        // A URL with a `[…]` text is a labeled link: no `bare` role.
        let nodes = build_src(Span::new("https://example.org[Example]"));

        let reference = assert_link(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "https://example.org");
        assert_eq!(link_text_of(reference), "Example");
        assert!(reference.roles.is_empty());

        // Its location covers the whole macro, the `[…]` included.
        assert_eq!(reference.location.data(), "https://example.org[Example]");
    }

    #[test]
    fn a_formal_url_window_suffix_opens_blank() {
        // A trailing `^` in the text is stripped and selects the `_blank`
        // window.
        let nodes = build_src(Span::new("https://example.org[Open^]"));

        let reference = assert_link(&nodes[0]);
        assert_eq!(link_text_of(reference), "Open");
        assert_eq!(reference.window.as_deref(), Some("_blank"));
    }

    #[test]
    fn an_escaped_auto_link_stays_literal() {
        // `\https://…` drops the backslash and keeps the URL as literal text – no
        // link node – with the boundary prefix preserved.
        let nodes = build_src(Span::new("see \\https://example.org here"));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an escaped auto-link must not produce a link node: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros("see \\https://example.org here")
        );
    }

    #[test]
    fn a_link_macro_url_target_is_left_for_the_link_macro_pass() {
        // `link:https://…[…]` is the pattern's LINK-MACRO branch; the auto-link
        // pass leaves it, and `link_macro_level` builds the identical node, so
        // the fold still matches the string pipeline byte-for-byte.
        let source = "link:https://example.org[Example]";
        let nodes = build_src(Span::new(source));

        let reference = assert_link(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "https://example.org");
        assert_eq!(link_text_of(reference), "Example");

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn an_auto_link_is_recognized_inside_a_span() {
        // A URL can appear inside a rendered span; the transducer descends into
        // the span body and builds the node there.
        let nodes = build_src(Span::new("*see https://example.org*"));

        let children = assert_styled(&nodes[0], StyleVariant::Strong, SpanForm::Constrained);
        assert_eq!(children.len(), 2);
        assert_text(&children[0], "see ", 1, 2);

        let reference = assert_link(&children[1]);
        assert_eq!(reference.target.as_ref(), "https://example.org");
        assert_eq!(link_text_of(reference), "https://example.org");
    }

    #[test]
    fn an_angle_bracketed_url_is_a_documented_divergence() {
        // `<https://example.org>` requires a leading `&lt;`, which is an escaped
        // `CharRef` by the time macros run, so the match is never verbatim and
        // the single-pass builder leaves it unrecognized for a later increment.
        let nodes = build_src(Span::new("<https://example.org>"));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an angle-bracketed URL must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* build a link here.
        assert!(golden_macros("<https://example.org>").contains("<a href"));
    }

    #[test]
    fn an_auto_link_over_a_special_character_is_a_documented_divergence() {
        // A URL whose body contains `&` is matched by the string pipeline over
        // the *escaped* text (`…?a=1&amp;b=2`). A self-describing node cannot
        // carry that escaped text as an `'src` slice, so the single-pass builder
        // leaves such a link *unrecognized* for a later increment, exactly as the
        // image increment defers `image:a&b.png[]`.
        let nodes = build_src(Span::new("https://example.org/?a=1&b=2"));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "a URL crossing an escaped special must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* build a link here.
        assert!(golden_macros("https://example.org/?a=1&b=2").contains("<a href"));
    }

    #[test]
    fn a_formal_url_link_attribute_list_is_a_documented_divergence() {
        // A formal URL text carrying an `=` splits into an attribute list (here a
        // role), which the string replacer parses from a newline-normalized copy
        // of the text – not from `'src`. The builder cannot carry that as an
        // `Attrlist<'src>` yet, so it defers the whole macro (left literal).
        let source = "https://example.org[Example,role=hl]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an attribute-list-in-text link must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, applies the role.
        assert!(golden_macros(source).contains(r#"class="hl""#));
    }

    // ---- `apply_link_side_effects` (staged for the eventual cutover) ------

    use super::apply_link_side_effects;

    /// Builds the single-pass tree for `source` against `parser` (unlike
    /// [`build_src`], which always uses its own fresh default parser).
    fn build_with<'src>(source: Span<'src>, parser: &Parser) -> Vec<InlineNode<'src>> {
        build(source, parser)
    }

    #[test]
    fn registers_a_link_macro_target_when_catalog_assets_is_enabled() {
        let source = "link:index.html[Docs]";
        let parser = Parser::default().with_catalog_assets(true);
        let nodes = build_with(Span::new(source), &parser);

        apply_link_side_effects(&nodes, &parser);

        let catalog = parser.catalog();
        let links = catalog.links();
        assert_eq!(links, ["index.html"]);
    }

    #[test]
    fn registers_a_mailto_target_with_its_scheme() {
        let source = "mailto:hello@example.org[Email us]";
        let parser = Parser::default().with_catalog_assets(true);
        let nodes = build_with(Span::new(source), &parser);

        apply_link_side_effects(&nodes, &parser);

        assert_eq!(parser.catalog().links(), ["mailto:hello@example.org"]);
    }

    #[test]
    fn registers_an_auto_link_and_a_formal_url_link_target() {
        let source = "https://example.org and https://example.org/docs[Docs]";
        let parser = Parser::default().with_catalog_assets(true);
        let nodes = build_with(Span::new(source), &parser);

        apply_link_side_effects(&nodes, &parser);

        assert_eq!(
            parser.catalog().links(),
            ["https://example.org", "https://example.org/docs"]
        );
    }

    #[test]
    fn registers_interleaved_forms_in_family_pass_order_not_source_order() {
        // `link:b.html[B]` appears first in the source, but the golden
        // pipeline's `link:`/`mailto:` pass runs *after* its auto-link/
        // formal-URL pass (see `apply_link_side_effects`'s own "Registration
        // order" doc note), so `https://a.example` – which appears second –
        // registers first. A single document-order tree walk would get this
        // backwards; the two-pass split must reproduce it.
        let source = "link:b.html[B] then https://a.example then link:c.html[C]";
        let parser = Parser::default().with_catalog_assets(true);
        let nodes = build_with(Span::new(source), &parser);

        apply_link_side_effects(&nodes, &parser);

        assert_eq!(
            parser.catalog().links(),
            ["https://a.example", "b.html", "c.html"]
        );

        // The golden string pipeline agrees.
        let golden_parser = Parser::default().with_catalog_assets(true);
        golden_macros_with(source, &golden_parser);
        assert_eq!(golden_parser.catalog().links(), parser.catalog().links());
    }

    #[test]
    fn does_not_register_a_cross_reference_as_a_link() {
        // A cross-reference is also a `Ref` node, but only a `Link` variant has
        // an asset-catalog entry.
        let source = "xref:intro[Introduction]";
        let parser = Parser::default().with_catalog_assets(true);
        let nodes = build_with(Span::new(source), &parser);

        apply_link_side_effects(&nodes, &parser);

        assert!(parser.catalog().links().is_empty());
    }

    #[test]
    fn registration_is_a_no_op_when_catalog_assets_is_disabled() {
        let source = "link:index.html[Docs]";
        let parser = Parser::default();
        let nodes = build_with(Span::new(source), &parser);

        apply_link_side_effects(&nodes, &parser);

        assert!(parser.catalog().links().is_empty());
    }

    #[test]
    fn registers_a_link_nested_inside_a_styled_span_and_a_footnote() {
        let source = "*see link:a.html[]* and footnote:[see link:b.html[]]";
        let parser = Parser::default().with_catalog_assets(true);
        let nodes = build_with(Span::new(source), &parser);

        apply_link_side_effects(&nodes, &parser);

        assert_eq!(parser.catalog().links(), ["a.html", "b.html"]);
    }

    #[test]
    fn matches_the_golden_pipelines_registration_for_a_broad_fixture_set() {
        // Each fixture uses its own pair of *independent* parsers (design
        // §5.3's two-independent-parsers discipline, already established by
        // the image increment's own differential corpus): one that the
        // additive builder builds against and this function then walks, one
        // that the real string pipeline (`golden_macros_with`) runs against
        // directly. Because neither path is wired into the other, comparing
        // their two catalogs after the fact is the whole test.
        let fixtures = [
            "link:index.html[Docs]",
            "link:[]",
            "mailto:hello@example.org[Email us]",
            "mailto:[]",
            "https://example.org",
            "https://example.org[Example]",
            "link:https://example.org[Example]",
            "\\link:index.html[Docs]",
            "link:a.html[A]{sp}link:b.html[B]",
            // Interleaved forms, out of source order relative to the family
            // passes that register them (see `apply_link_side_effects`'s own
            // "Registration order" doc note).
            "link:b.html[B]{sp}then{sp}https://a.example",
        ];

        for fixture in fixtures {
            let builder_parser = Parser::default().with_catalog_assets(true);
            let nodes = build_with(Span::new(fixture), &builder_parser);
            apply_link_side_effects(&nodes, &builder_parser);

            let golden_parser = Parser::default().with_catalog_assets(true);
            golden_macros_with(fixture, &golden_parser);

            assert_eq!(
                builder_parser.catalog().links(),
                golden_parser.catalog().links(),
                "registered links diverged for {fixture:?}"
            );
        }
    }
}
