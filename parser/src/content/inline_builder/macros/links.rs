//! Auto-link, formal-URL-link, and `link:`/`mailto:` macro recognition.

use super::{
    MacroMatch, MacroMatchKind,
    image::{range_has_no_opaque_piece, range_is_verbatim, range_is_verbatim_or_synthesized},
    macro_text_children, rebuild_macro_level,
};
use crate::{
    Parser, Span,
    attributes::Attrlist,
    content::{
        INLINE_EMAIL, INLINE_LINK, INLINE_LINK_MACRO, NormalizedCaps, URI_SNIFF,
        encode_uri_component, extract_attributes_from_text,
        inline_builder::quotes::{
            Piece, SPAN_PLACEHOLDER, build_match_string, source_slice, text_slice,
        },
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
/// This increment covers [`INLINE_LINK`]'s **non-angle branch** — a bare
/// auto-linked URL (`https://example.org`) and a formal URL link
/// (`https://example.org[text]`, `https://example.org[]`) — and its **ANGLE
/// branch** — an angle-bracketed URL (`<https://example.org>`) and the
/// bracketed form that keeps its `&lt;` (`<https://example.org[text]`) — in
/// their verbatim,
/// attribute-list-free forms, reproducing the string replacer's boundary-prefix
/// preservation, bare-URL trailing-punctuation stripping, `^` new-window
/// suffix, `hide-uri-scheme` display text, and `\` scheme escape. It
/// deliberately leaves several forms **unrecognized** for a later increment,
/// each left as literal source here (so the differential corpus only pins the
/// forms this increment claims):
///
/// - The **`link:` URL macro** form (`link:https://example.org[text]`, the
///   pattern's LINK-MACRO branch) is left to [`link_macro_level`], which folds
///   the identical node; running that pass second mirrors the string step's
///   order.
/// - A **formal text carrying an attribute list** (an `=` selecting roles / id
///   / title / window) is deferred when the bracketed text is not verbatim
///   `'src`, exactly as [`link_macro_level`] defers it: the attribute list is
///   parsed from a newline-normalized *copy* of the text, and only a text with
///   no embedded newline *and* no [`synthesized`](Piece::synthesized) run in it
///   has an `'src` slice that copy coincides with — the one thing an
///   [`Attrlist`]`<'src>` cannot do without.
/// - A match crossing an [`atomic`](Piece::atomic) piece — a URL crossing an
///   escaped special ([`CharRef`](InlineNode::CharRef)) or a rendered
///   [`Styled`](crate::inlines::Styled) span — is deferred exactly as the image
///   increment defers `image:a&b.png[]`. For the ANGLE branch this means the
///   URL *between* the delimiters: the delimiters themselves are escaped
///   specials by construction (see [`build_inline_link_node`]).
///
/// A [`synthesized`](Piece::synthesized) run (an attribute expansion, or —
/// reached at a tree's root — a filtered multi-line block's own joined seed)
/// **is** admitted: every value this pass's nodes hold — the scheme, the URL,
/// and the bracketed display text — is computed out of the level's match
/// string, which carries a synthesized run's bytes exactly, so
/// `https://{host}/path` and `{url}[Docs]` are recognized with only the node's
/// `location` taking design §4.4's coarse fallback. The one exception is the
/// attribute-list text above, which is what `Attrlist<'src>` needs a real
/// slice for.
///
/// An invalid quoted bare URL (`"https://example.org`) and a bare scheme with no
/// body (`http://;`) are left literal by the string step *and* the builder, so
/// they render identically and are covered by the differential corpus rather
/// than a divergence test. So is an ANGLE-branch URL with **no** closing `&gt;`
/// and no `[…]` (`<https://example.org`), which the string replacer's own angle
/// path emits unchanged.
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

/// Finds every recognized auto-link / formal-URL / angle-bracketed link at this
/// level, skipping a `link:` macro match and any form
/// [`build_inline_link_node`] defers — including a non-verbatim one, whose gate
/// now lives inside that function (it needs the branch's own capture groups to
/// know which sub-range of the match must be verbatim; see
/// [`inline_link_level`]).
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

        // The `link:` URL macro form is left to `link_macro_level`, which folds
        // the identical node.
        if n.is_link_macro() {
            continue;
        }

        match build_inline_link_node(&n, &full, pieces, root, parser) {
            Some(m) => matches.push(m),

            // A deferred or invalid form (an escaped scheme is handled inside as
            // an `Unescape`; a non-verbatim match, a quoted bare URL, a bare
            // scheme, an unterminated angle-bracketed URL, or an attribute-list
            // text is left as literal source).
            None => continue,
        }
    }

    matches
}

/// Builds one [`MacroMatch`] for a verbatim [`INLINE_LINK`] match: a
/// [`Ref`](InlineNode::Ref) link node (with the boundary prefix kept before it
/// and any stripped trailing punctuation kept after it), or an
/// [`Unescape`](MacroMatchKind::Unescape) for an escaped scheme. Returns `None`
/// for a form this increment defers or that the string step leaves literal (see
/// [`inline_link_level`]).
///
/// The value computation mirrors [`InlineLinkReplacer`] exactly — target, bare
/// vs. labeled display text, `hide-uri-scheme`, the `^` window suffix, and the
/// trailing-punctuation strip — so the fold reproduces the same bytes through
/// the same `render_link` [`link_macro_level`]'s nodes fold through. The
/// replacer's own angle-bracketed special case (`<url>`: no boundary prefix
/// kept, no trailing-punctuation strip, the whole match consumed) is mirrored
/// by [`build_angle_link_node`], to which this delegates on the same condition
/// the replacer branches on.
///
/// # The gate lives here
///
/// A match crossing an [`atomic`](Piece::atomic) piece — an escaped special
/// ([`CharRef`](InlineNode::CharRef)) or a rendered
/// [`Styled`](crate::inlines::Styled) span — is deferred. *Which* sub-range the
/// gate covers depends on the branch, which is why the check sits here rather
/// than in [`find_inline_link_matches`]: the ANGLE branch's `&lt;` prefix and
/// `&gt;` terminator are themselves escaped specials — atomic pieces — under
/// every effective order that escapes them, so gating the whole match would
/// defer that branch outright. Those two delimiters carry no value a node
/// slices (the replacer emits neither), so for the ANGLE branch the gate covers
/// only the interior between them: the scheme, the URL, and any `[…]`
/// attribute list.
///
/// A [`synthesized`](Piece::synthesized) run is admitted (see
/// [`inline_link_level`]); only the attribute-list branch below, which parses a
/// real [`Attrlist`]`<'src>` out of the bracketed text's own source slice,
/// still requires that one sub-range to be verbatim.
///
/// Like every macro family in this additive builder, it deliberately performs
/// *no* recognition side effect: it does **not** `register_link` the target in
/// the document's asset catalog, because the builder is not yet the
/// authoritative recognition sink — the string pipeline still registers it, and
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
    // `scheme_match` is always present (no branch can match without it); fall
    // through defensively if it is somehow absent.
    let scheme_m = n.scheme_match()?;
    let scheme = scheme_m.as_str();

    // The sub-range the gate covers (see this function's own "the gate lives
    // here" note): the whole match, except for the ANGLE branch, whose
    // escaped-special delimiters sit outside the interior.
    let gated_range = if n.is_angle() {
        scheme_m.start()..n.angle_url().map_or(full.end, |m| m.end())
    } else {
        full.clone()
    };

    if !range_is_verbatim_or_synthesized(pieces, &gated_range) {
        return None;
    }

    // The `<url>` form is a separate computation in the string replacer, taken
    // on exactly this condition.
    if n.is_angle() && n.attrlist().is_none() {
        return build_angle_link_node(n, &scheme_m, full, pieces, root, parser);
    }

    // An escaped scheme (`\https://…`) keeps the boundary prefix and drops the
    // single backslash, leaving the rest of the match literal — no link node.
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
        // keeping it as literal text after the link — mirroring the string
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
    let mut attrs: Option<Attrlist<'src>> = None;

    let link_text = if let Some(mut link_text) = link_text {
        link_text = link_text.replace("\\]", "]");

        // A text carrying an `=` splits into an attribute list. `InlineLink
        // Replacer` parses it from a newline-normalized *copy* of the text;
        // when the text has no embedded newline that copy is byte-identical
        // to the bracketed text's own `'src` slice, so the node can carry the
        // real, honestly-borrowed `Attrlist<'src>` `render_link` needs
        // (`Ref::attrs`'s own field docs explain why `roles`/`window` alone
        // are not enough). A text that *does* embed a newline still needs a
        // synthesized (owned) copy the node cannot hold yet, so that one form
        // remains deferred — as does one crossing a
        // [`synthesized`](Piece::synthesized) run, whose match-string bytes
        // have no `'src` slice at all (the one sub-range this family's
        // expanded-value lift cannot cover).
        if link_text.contains('=') {
            #[allow(clippy::unwrap_used)]
            let range = text_location_range.clone().unwrap();

            if !range_is_verbatim(pieces, &range) {
                return None;
            }

            let text_span = source_slice(pieces, range, root);

            if text_span.data().contains('\n') {
                return None;
            }

            let (lt, parsed) = extract_attributes_from_text(text_span, parser, None);

            // Mirrors `InlineLinkReplacer`'s own guard: only adopt the parsed
            // result when a real named attribute actually split off from the
            // text (otherwise the `=` was incidental and `extract_attributes_
            // from_text` already returned the text unchanged with an empty
            // attrlist, matching this fallthrough).
            if lt != text_span.data() {
                link_text = lt.replace("\\\"", "\"");
                attrs = Some(parsed);
            }
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
        attrs,
        resolved: None,
        derived: None,
        xrefstyle: None,
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

/// Builds one [`MacroMatch`] for an angle-bracketed URL (`<https://example.org>`
/// — [`INLINE_LINK`]'s ANGLE branch with no `[…]`), mirroring
/// [`InlineLinkReplacer`]'s own angle path exactly. That path differs from the
/// general one in three ways, each reproduced here:
///
/// - **The delimiters are consumed, not kept.** The replacer emits *only* the
///   rendered link for the whole match, so the node's `consumed` range is the
///   whole match — the `&lt;` prefix and `&gt;` terminator included — rather
///   than starting at the scheme the way a boundary-prefixed non-angle link
///   does.
/// - **No trailing-punctuation strip and no "bare scheme with no body"
///   rejection.** The target is simply the scheme plus the URL captured between
///   the delimiters, which the pattern guarantees is non-empty. The link is
///   always `bare` (the replacer passes `extra_roles: vec!["bare"]`), never
///   carries a `window`, and folds through an empty attribute list.
/// - **Both escapes are honored.** A `\` before the `&lt;` and a `\` before the
///   scheme each drop that one backslash and leave the rest of the match
///   literal — two [`Unescape`](MacroMatchKind::Unescape)s where the general
///   path has only the scheme one.
///
/// Returns `None` for the ANGLE branch's remaining alternative — a URL with no
/// closing `&gt;` (`<https://example.org`) — which the replacer emits unchanged.
/// (Its third alternative, a `[…]` attribute list, never reaches here: the
/// caller delegates only when `attrlist` did not participate, exactly as the
/// replacer branches.)
///
/// Like the rest of this additive builder it performs no `register_link` side
/// effect; [`apply_link_side_effects`] stages that for the cutover, and needs
/// no angle-specific case — an angle node is `InlineLinkReplacer`'s own pass,
/// so [`link_form`] already classifies it as
/// [`AutoOrFormal`](LinkForm::AutoOrFormal) from its `location` and `target`.
///
/// [`InlineLinkReplacer`]: crate::content::macros
fn build_angle_link_node<'src>(
    n: &NormalizedCaps<'_, '_>,
    scheme_m: &regex::Match<'_>,
    full: &std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Option<MacroMatch<'src>> {
    // An escaped `&lt;` (`\<https://…>`) drops that backslash and leaves the
    // whole match literal, exactly as the replacer's `caps[0][1..]` does.
    if n.prefix_str().starts_with('\\') {
        return Some(MacroMatch {
            kind: MacroMatchKind::Unescape {
                backslash: full.start,
            },
            full: full.clone(),
        });
    }

    // An escaped scheme (`<\https://…>`) keeps the `&lt;` and drops the
    // backslash after it, leaving the rest literal.
    if scheme_m.as_str().starts_with('\\') {
        return Some(MacroMatch {
            kind: MacroMatchKind::Unescape {
                backslash: scheme_m.start(),
            },
            full: full.clone(),
        });
    }

    // The `<url>` alternative did not participate: an unterminated
    // angle-bracketed URL, which the replacer leaves wholly literal.
    let angle_url = n.angle_url()?;

    let target = format!(
        "{scheme}{url}",
        scheme = scheme_m.as_str(),
        url = angle_url.as_str()
    );

    let link_text = hide_uri_scheme_text(&target, parser);

    let location = source_slice(pieces, full.clone(), root);

    let node = InlineNode::Ref(Ref {
        variant: RefVariant::Link,
        target: CowStr::from(target),

        children: vec![InlineNode::Text {
            value: CowStr::from(link_text),
            location,
        }],

        roles: vec![CowStr::from("bare")],
        window: None,
        attrs: None,
        resolved: None,
        derived: None,
        xrefstyle: None,
        location,
    });

    Some(MacroMatch {
        kind: MacroMatchKind::Node {
            consumed: full.clone(),
            node: Box::new(node),
        },
        full: full.clone(),
    })
}

/// The display text for a bare link, dropping the URI scheme under
/// `hide-uri-scheme` exactly as the string replacer's `URI_SNIFF` strip does.
/// Neither of the two callers can be left with nothing by the strip:
/// [`INLINE_LINK`]'s bare branch rejects a bare scheme with no body upstream,
/// and its ANGLE branch's `<url>` alternative requires at least one character
/// between the delimiters — so, unlike a bare `link:`/`mailto:` macro, this
/// does *not* fall back to the whole target.
fn hide_uri_scheme_text(target: &str, parser: &Parser) -> String {
    if parser.is_attribute_set("hide-uri-scheme") {
        URI_SNIFF.replace_all(target, "").into_owned()
    } else {
        target.to_string()
    }
}

/// The `link:`/`mailto:` macro pass at a level: matches [`INLINE_LINK_MACRO`]
/// over the level's escaped text and replaces each recognized match with the
/// [`Ref`](InlineNode::Ref) link node it produces.
///
/// # Scope
///
/// This increment covers the **explicit `link:`/`mailto:` macro** in its
/// attribute-list-free forms: `link:target[text]`, `link:target[]`
/// (a bare link), `mailto:addr[text]`, and `mailto:addr[]`, plus the `^`
/// new-window suffix and the `\` escape. It deliberately leaves several forms
/// **unrecognized** for a later increment, each left as literal source here (so
/// the differential corpus only pins the forms this increment claims):
///
/// - **Auto-links and formal-URL links** (`https://example.org`, `https://example.org[text]`)
///   are matched by a *different* pattern (`INLINE_LINK`, with its bare-URL
///   trailing-punctuation handling) and are a separate later increment.
/// - **A link text that carries an attribute list** — a `,` in a `mailto:` text
///   (its `subject`/`body`) or an `=` in a `link:` text (roles / id / title /
///   window) — is deferred unless the bracketed text is verbatim `'src`,
///   because that attribute list is parsed from a newline-normalized *copy* of
///   the text and only a text with no embedded newline, no
///   [`synthesized`](Piece::synthesized) run, and no escaped special in it has
///   a source slice that copy coincides with — the one thing an
///   [`Attrlist`]`<'src>` cannot do without. This is the one capture that keeps
///   the escaped-special boundary the rest of the family lifts below.
/// - **A match crossing a rendered [`Styled`](crate::inlines::Styled) span**
///   (`link:x[*bold*]`) — one opaque piece here where the string pipeline's own
///   haystack holds its markup inline, and that markup, which exists only at
///   fold time, is what the replacer's attribute-list probe and the pattern's
///   `]` boundary read.
/// - **A macro whose own `link:`/`mailto:` marker is not verbatim** — a
///   *wholly* expanded macro (`:m: link:index.html[Docs]`, then `{m}`) — is
///   deferred. Its target and bracketed text could be read from the match
///   string like every other value here, but the node's `location` would then
///   fall back to the expansion's coarse span (design §4.4), and that location
///   is the very signal [`link_form`] reads to tell this pass's nodes apart
///   from the other two link passes' when [`apply_link_side_effects`] replays
///   the string pipeline's own family-pass registration order. A macro whose
///   marker *is* written in the source (`link:{url}[Docs]`,
///   `mailto:{addr}[Team]`) keeps an honest location and is recognized.
///
/// Apart from that marker, a [`synthesized`](Piece::synthesized) run is
/// admitted: the target and display text are read out of the level's match
/// string, which carries an expanded value's bytes exactly.
///
/// An **escaped special** ([`CharRef`](InlineNode::CharRef)`::Special`) is
/// admitted too, anywhere but the attribute-list text above (`link:a&b.html[]`,
/// `link:index.html[a < b]`). The match string carries such a leaf's canonical
/// entity — the very bytes the string replacer's own escaped haystack holds
/// there — so the target this pass computes off that string (`a&amp;b.html`)
/// *is* the one the replacer computed, and no value on the node needs the
/// source's own `<`/`>`/`&`. The display text then becomes **structured
/// children** rather than one baked `Text` (see [`macro_text_children`]), so
/// the special folds back to its own entity instead of being escaped twice —
/// for a bare macro too, whose shown text is a slice of the target group rather
/// than a computed string (see [`build_link_node`]).
///
/// A `link:` (not `mailto:`) target whose scheme could execute script
/// (`javascript:`, `data:`, `vbscript:`) is likewise left literal — matching
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

    let matches = find_link_macro_matches(&nodes, &s, &pieces, root, parser);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every recognized `link:`/`mailto:` macro at this level, skipping any
/// match that crosses an **opaque** piece (see
/// [`range_has_no_opaque_piece`]), whose own marker is not verbatim source, or
/// that [`build_link_node`] defers (see [`link_macro_level`]).
fn find_link_macro_matches<'src>(
    nodes: &[InlineNode<'src>],
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

        // An escape (`\link:`) is honored by dropping the backslash and keeping
        // the rest literal, mirroring `InlineLinkMacroReplacer`'s own leading
        // `caps[0].starts_with('\\')` check — which it makes *before* looking
        // at anything else, so the escape needs no gate of its own here either:
        // dropping the backslash keeps the rest of the match as its **own
        // original nodes** (a rendered span or an escaped special among them),
        // which fold back to exactly the bytes the replacer's `caps[0][1..]`
        // emits. (This is the same check-order fix the `footnoteref:`, menu, and
        // cross-reference increments made for their own families; before it, an
        // escaped `\link:x[*bold*]` whose match the gate rejected was left
        // unrecognized, backslash and all.)
        if whole.as_str().starts_with('\\') {
            matches.push(MacroMatch {
                kind: MacroMatchKind::Unescape {
                    backslash: full.start,
                },
                full,
            });

            continue;
        }

        // A match crossing a rendered span is left for a later increment; one
        // crossing an expanded attribute value or an escaped special is
        // admitted, since the target and text are read from the match string —
        // whose bytes are, for both, exactly the ones the string replacer's own
        // haystack carries there.
        if !range_has_no_opaque_piece(nodes, pieces, &full) {
            continue;
        }

        // The macro's own `link:`/`mailto:` marker must be verbatim source, so
        // the node's `location` still starts with it — the signal
        // [`link_form`] reads (see [`link_macro_level`]'s own scope note). The
        // marker runs from the match's start to wherever the target group
        // begins; one of groups 2 (empty target) and 3 always participates.
        let marker = full.start
            ..caps
                .get(2)
                .or_else(|| caps.get(3))
                .map_or(full.end, |m| m.start());

        if !range_is_verbatim(pieces, &marker) {
            continue;
        }

        match build_link_node(&caps, &full, nodes, pieces, root, parser) {
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

/// Builds one [`Ref`](InlineNode::Ref) link node from a recognized
/// `link:`/`mailto:` match, computing the target, display text, window, and
/// roles exactly as the string replacer does so the fold reproduces the same
/// bytes. Returns `None` for a form this increment defers (see
/// [`link_macro_level`]): a rejected dangerous `link:` scheme, or a link text
/// that carries an attribute list this builder cannot parse from `'src`.
///
/// The display text becomes the node's children, so the fold recovers
/// `link_text` by folding them and needs no build-time state
/// (bare-vs-labeled, `hide-uri-scheme`, `mailto:`) at fold time; the `bare`
/// role, when the string step would add one, rides on the node's `roles`.
/// Which shape those children take follows what the text *is*:
///
/// - a **bracketed text** is sliced out of the bracket by
///   [`macro_text_children`] — borrowed from `'src` in the common case,
///   structured (a [`CharRef`](InlineNode::CharRef) of its own per escaped
///   special) when it crosses one;
/// - a **bare macro's** text is the target itself, which is likewise a slice —
///   the whole target group, or (under `hide-uri-scheme`) its scheme-stripped
///   tail, always a *suffix* since [`URI_SNIFF`] is `^`-anchored — so it takes
///   the same treatment rather than baking the already-escaped target into one
///   `Text` the fold would escape a second time (design §3.4);
/// - an **attribute list's positional value** is the one text this builder
///   *computes*, out of a parse of the bracket's own verbatim `'src` slice, so
///   it stays a single synthesized `Text` (that slice carries no entity to
///   undo).
///
/// As in the additive builder generally, this performs *no* recognition side
/// effect — notably it does **not** `register_link` the target in the asset
/// catalog, which the string replacer does; the cutover (design §5.2 Phase 4,
/// step 6) re-attaches that (see [`build_inline_link_node`]).
pub(super) fn build_link_node<'src>(
    caps: &regex::Captures<'_>,
    full: &std::ops::Range<usize>,
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Option<InlineNode<'src>> {
    let location = source_slice(pieces, full.clone(), root);

    // Group 1 present ⟺ a `mailto:` macro; group 3 is the (optional) target;
    // group 5 is the (optional) bracketed text.
    let is_mailto = caps.get(1).is_some();
    let target_str = caps.get(3).map_or("", |m| m.as_str());

    let mut target = if is_mailto {
        format!("mailto:{target_str}")
    } else {
        target_str.to_string()
    };

    // A `link:` target whose scheme could execute script is neutralized by the
    // string step (left literal); mirror that by deferring — the node would
    // otherwise render a live, dangerous link. `mailto:` carries its own safe
    // scheme and is exempt.
    if !is_mailto && has_dangerous_scheme(&target) {
        return None;
    }

    let mut window: Option<CowStr<'src>> = None;
    let mut attrs: Option<Attrlist<'src>> = None;

    let raw_text_m = caps.get(5);
    let raw_text = raw_text_m.map_or("", |m| m.as_str());
    let mut link_text = raw_text.to_string();

    // Set when the display text stops being one the children can *slice* out of
    // the bracketed text: an attribute list's positional value is computed by
    // `extract_attributes_from_text`, and a bare macro's text is derived from
    // the target below.
    let mut computed_text = false;

    // Set when the `^` new-window suffix was trimmed off the display text, so
    // the children's range stops one (ASCII) byte short of the bracket's end.
    let mut caret_stripped = false;

    if !link_text.is_empty() {
        link_text = link_text.replace("\\]", "]");

        // An attribute list embedded in the text (`mailto:` subject/body via a
        // comma, or `link:` roles/id/title via an `=`) is parsed by
        // `InlineLinkMacroReplacer` from a newline-normalized *copy* of the
        // (pre-`\]`-unescape) bracketed text; when that text has no embedded
        // newline the copy is byte-identical to the bracket's own `'src`
        // slice, so the node can carry the real `Attrlist<'src>` `render_link`
        // needs (`Ref::attrs`'s own field docs). A text that *does* embed a
        // newline still needs a synthesized copy the node cannot hold yet, so
        // that one form remains deferred — as does one crossing a
        // [`synthesized`](Piece::synthesized) run, whose match-string bytes
        // have no `'src` slice at all.
        if is_mailto {
            if link_text.contains(',') {
                #[allow(clippy::unwrap_used)]
                let m = raw_text_m.unwrap();
                let range = m.start()..m.end();

                if !range_is_verbatim(pieces, &range) {
                    return None;
                }

                let text_span = source_slice(pieces, range, root);

                if text_span.data().contains('\n') {
                    return None;
                }

                let (lt, parsed) = extract_attributes_from_text(text_span, parser, None);
                link_text = lt;
                computed_text = true;

                if let Some(target_attr) = parsed.nth_attribute(2) {
                    target = format!(
                        "{target}?subject={subject}",
                        subject = encode_uri_component(target_attr.value())
                    );

                    if let Some(body) = parsed.nth_attribute(3) {
                        target = format!(
                            "{target}&amp;body={body}",
                            body = encode_uri_component(body.value())
                        );
                    }
                }

                attrs = Some(parsed);
            }
        } else if link_text.contains('=') {
            #[allow(clippy::unwrap_used)]
            let m = raw_text_m.unwrap();
            let range = m.start()..m.end();

            if !range_is_verbatim(pieces, &range) {
                return None;
            }

            let text_span = source_slice(pieces, range, root);

            if text_span.data().contains('\n') {
                return None;
            }

            let (lt, parsed) = extract_attributes_from_text(text_span, parser, None);
            link_text = lt;
            computed_text = true;
            attrs = Some(parsed);
        }

        if link_text.ends_with('^') {
            link_text.truncate(link_text.len() - 1);
            window = Some(CowStr::from("_blank"));
            caret_stripped = true;
        }
    }

    let mut roles: Vec<CowStr<'src>> = vec![];

    // A bare macro's display text is the target itself — the whole target, or
    // (under `hide-uri-scheme`) its scheme-stripped tail — so it is a *slice*
    // of the target group rather than a value this builder computes, and its
    // children are recovered from that range below. For a `mailto:` the slice
    // is the address as written (group 3), which is also what the target's own
    // `mailto:` prefix was built from.
    let mut bare_text_range: Option<std::ops::Range<usize>> = None;

    if link_text.is_empty() {
        if is_mailto {
            // A bare `mailto:` shows the address (group 3) and takes no `bare`
            // role.
            link_text = target_str.to_string();
            bare_text_range = caps.get(3).map(|m| m.start()..m.end());
        } else {
            // A bare `link:` shows the target (with the scheme optionally
            // hidden) and takes the `bare` role. `target` is `target_str` here:
            // only the `mailto:` branch above ever rewrites it, and that branch
            // takes the address path instead.
            let stripped = if parser.is_attribute_set("hide-uri-scheme") {
                // `URI_SNIFF` is `^`-anchored, so `replace_all` strips exactly
                // one prefix and the shown text is always a suffix of the
                // target. A strip that would leave nothing falls back to the
                // whole target, mirroring the string replacer's own
                // `if lt.is_empty()` guard.
                URI_SNIFF
                    .find(&target)
                    .map(|m| m.end())
                    .filter(|end| *end < target.len())
                    .unwrap_or(0)
            } else {
                0
            };

            link_text = target.get(stripped..).unwrap_or(&target).to_string();
            bare_text_range = caps.get(3).map(|m| (m.start() + stripped)..m.end());

            roles.push(CowStr::from("bare"));
        }
    }

    // The display text becomes the node's children, located at the bracketed
    // text (or the whole macro when there is none).
    let text_location =
        raw_text_m.map_or(location, |m| source_slice(pieces, m.start()..m.end(), root));

    let children = if link_text.is_empty() {
        vec![]
    } else if let Some(range) = bare_text_range {
        // A bare macro's display text, recovered from the target's own range so
        // an escaped special in it stays the `CharRef` it already is (see
        // `macro_text_children`) rather than being baked — already escaped —
        // into one `Text` node the fold would escape a second time. There is no
        // `\]` bracket unescape here: this text comes from the target group,
        // which the pattern's own character class never lets a bracket into.
        macro_text_children(&link_text, range, false, nodes, pieces, root)
    } else if computed_text {
        // A value this builder *computed* rather than sliced: an attribute
        // list's positional value, so a *synthesized* `Text` whose value need
        // not coincide with its source. It needs no unescaping — that branch
        // requires a verbatim `'src` slice, which carries no entity.
        vec![InlineNode::Text {
            value: CowStr::from(link_text),
            location: text_location,
        }]
    } else {
        // A text sliced straight out of the bracket. `macro_text_children`
        // borrows it from `'src` in the common verbatim case (§4.5), owns it
        // when it crosses an expanded attribute value, and rebuilds it as
        // structured children — each escaped special staying the `CharRef` it
        // already is — when it crosses one, applying the same `\]` unescape
        // `InlineLinkMacroReplacer` performs. The `^` window suffix is one
        // ASCII byte at the end of the bracketed text, so the range simply
        // stops short of it.
        #[allow(clippy::unwrap_used)]
        let m = raw_text_m.unwrap();

        let end = if caret_stripped { m.end() - 1 } else { m.end() };

        let trimmed = raw_text.get(..end - m.start()).unwrap_or(raw_text);

        macro_text_children(trimmed, m.start()..end, true, nodes, pieces, root)
    };

    Some(InlineNode::Ref(Ref {
        variant: RefVariant::Link,
        target: CowStr::from(target),
        children,
        roles,
        window,
        attrs,
        resolved: None,
        derived: None,
        xrefstyle: None,
        location,
    }))
}

/// The bare e-mail auto-link pass at a level: matches [`INLINE_EMAIL`] over the
/// level's escaped text and replaces each recognized address with the
/// [`Ref`](InlineNode::Ref)`{Link}` node it produces — the same node kind the
/// two URL-link passes above build, so it folds through the identical
/// `render_link`.
///
/// # Scope
///
/// This is the **last** of the link family's spellings: a bare address written
/// in the flow (`doc.writer@example.com`), which the string pipeline turns into
/// a `mailto:` link whose display text is the address itself (no `bare` role,
/// unlike an auto-linked URL — see [`build_email_node`]). It reuses the string
/// pipeline's *exact* recognition, so only the recognition *sink* differs
/// (design §4.1), including the pattern's own "prefix that causes a mismatch"
/// group: a `\` escape drops its backslash and leaves the address literal,
/// while a `>`, `:`, or `/` before the address means it is not a bare address
/// at all (it is the tail of a URL, or a `mailto:` macro's own target) and the
/// whole match is left untouched.
///
/// It runs **after** both URL-link passes and before the anchor pass, exactly
/// where the string step runs `InlineEmailReplacer` — which matters, because by
/// then a `mailto:`/`link:` macro and an auto-linked URL are already opaque
/// nodes here (they are already-rendered `<a …>` markup there), so an address
/// *inside* one is never re-recognized.
///
/// Two forms are left **unrecognized** for a later increment, each documented
/// and pinned by its own divergence test:
///
/// - An address carrying a literal `&` (`a&b@example.org`), which reaches this
///   pass as an atomic [`CharRef`](InlineNode::CharRef) (`&amp;`, admitted by
///   the pattern's own local-part class) that a node cannot carry as text — the
///   same escaped-special boundary every other macro family documents.
/// - An address **abutting an already-recognized construct**
///   (`**bold**doc@example.org`, `link:x[y]doc@example.org`,
///   `image:x.png[]doc@example.org`). The mismatch-prefix group reads the
///   character immediately before the address; in the string pipeline that is
///   the preceding construct's *rendered* last character (`</strong>`, `</a>`,
///   and `<img …>` all end in `>`, a mismatch character, so the address stays
///   literal there), while here [`build_match_string`] stands the construct in
///   as one opaque [`SPAN_PLACEHOLDER`] belonging to no mismatch class. A tree
///   whose markup exists only at fold time cannot reproduce that decision, so
///   the address is left literal rather than recognized into a link the string
///   pipeline does not build. The sibling auto-link family reaches the same
///   outcome structurally — [`INLINE_LINK`]'s own boundary-prefix group is
///   *required*, so a placeholder simply fails its match
///   (`**bold**https://example.org` is already deferred for exactly this
///   reason, independently of this pass). The deferral is deliberately
///   unconditional rather than keyed on what the preceding node *would* render
///   to: a construct that renders to nothing (a concealed index term) or to
///   text not ending in a mismatch character (a STEM expression, a
///   passthrough) is one the string pipeline *does* link, so those defer too —
///   reading that would mean invoking a renderer while building the tree.
///
/// An address's bytes *may*, by contrast, come from a
/// [`synthesized`](Piece::synthesized) run (an attribute expansion, or —
/// reached at a tree's root — a filtered multi-line block's own joined seed):
/// like an anchor's id, and unlike a URL link's own target, an e-mail node
/// needs no `Span`-typed field, so [`build_email_node`] recovers the exact
/// address text there too rather than deferring.
pub(super) fn email_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes);

    // Cheap pre-filter mirroring the string step's own `text.contains('@')`
    // guard.
    if !s.contains('@') {
        return nodes;
    }

    let matches = find_email_matches(&nodes, &s, &pieces, root);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every recognized bare e-mail address at this level, honoring the
/// pattern's own mismatch-prefix group and skipping any address
/// [`build_email_node`] defers (see [`email_level`]).
fn find_email_matches<'src>(
    nodes: &[InlineNode<'src>],
    s: &str,
    pieces: &[Piece],
    root: Span<'src>,
) -> Vec<MacroMatch<'src>> {
    let mut matches = Vec::new();

    for caps in INLINE_EMAIL.captures_iter(s) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();

        let full = whole.start()..whole.end();

        // Group 1 is the optional "prefix that causes a mismatch".
        let prefix = caps.get(1).map_or("", |m| m.as_str());

        if !prefix.is_empty() {
            if prefix == "\\" {
                // The escape drops its single backslash and keeps the rest of
                // the match literal, mirroring the string replacer's
                // `caps[0][1..]`.
                matches.push(MacroMatch {
                    kind: MacroMatchKind::Unescape {
                        backslash: full.start,
                    },
                    full,
                });
            }

            // Any other prefix (`>`, `:`, `/`) makes the string replacer emit
            // the whole match unchanged — which is exactly what recording no
            // match at all does here.
            continue;
        }

        // The mismatch-prefix group above read an *empty* prefix — but that
        // decision is only faithful when the tree can actually see the
        // character the string pipeline reads there. When the address abuts an
        // already-recognized construct (`**bold**doc@example.org`,
        // `link:x[y]doc@example.org`), the string pipeline reads that
        // construct's *rendered* last character — `</strong>`, `</a>`, and
        // `<img …>` all end in `>`, one of the three mismatch characters — and
        // suppresses the address, while [`build_match_string`] stands the
        // construct in as one opaque [`SPAN_PLACEHOLDER`], which no mismatch
        // class contains. Recognizing here would build a link the string
        // pipeline does not, so this defers instead — leaving the address as
        // literal text, never a wrong node, exactly as the sibling auto-link
        // family already behaves for the same input ([`INLINE_LINK`]'s own
        // boundary-prefix group is *required*, so a placeholder simply fails
        // its match). See [`email_level`]'s own scope note.
        if s.get(..full.start)
            .is_some_and(|before| before.ends_with(SPAN_PLACEHOLDER))
        {
            continue;
        }

        match build_email_node(&caps, pieces, root, nodes) {
            Some(node) => matches.push(MacroMatch {
                kind: MacroMatchKind::Node {
                    consumed: full.clone(),
                    node: Box::new(node),
                },
                full,
            }),

            // An address crossing an atomic piece (an escaped `&`); left as
            // literal source, exactly as every other macro family defers a
            // match it cannot recover the text of.
            None => continue,
        }
    }

    matches
}

/// Builds one [`Ref`](InlineNode::Ref)`{Link}` node from a bare e-mail match,
/// computing the target and display text exactly as `InlineEmailReplacer` does
/// so the fold reproduces the same bytes: the target is the address prefixed
/// with `mailto:`, and the display text is the address as written. Unlike a
/// bare *URL* auto-link, no `bare` role is added and `hide-uri-scheme` plays no
/// part — the string replacer passes `extra_roles: vec![]` and the raw address
/// as its `link_text`.
///
/// The address is recovered with [`text_slice`] rather than
/// [`source_slice`]`.data()`, so it is exact even when its bytes come from a
/// [`synthesized`](Piece::synthesized) run — the same treatment an anchor's id
/// receives, and available for the same reason: an e-mail node carries no
/// `Span`-typed field (no [`Attrlist`] parsed out of `'src`), only plain text,
/// so only the node's `location` needs design §4.4's coarse fallback. Returns
/// `None` when the address crosses an [`atomic`](Piece::atomic) piece — an
/// escaped `&`, the one form [`email_level`] defers.
///
/// As in the additive builder generally, this performs *no* recognition side
/// effect: it does **not** `register_link` the target, which
/// `InlineEmailReplacer` does. [`apply_link_side_effects`] is that deferred
/// piece, staged for the cutover alongside the other link forms'.
fn build_email_node<'src>(
    caps: &regex::Captures<'_>,
    pieces: &[Piece],
    root: Span<'src>,
    nodes: &[InlineNode<'src>],
) -> Option<InlineNode<'src>> {
    // Group 2 is the address itself. The `unwrap` is safe: the group is the
    // pattern's one mandatory capture, so it participates in every match (the
    // same reason the overall-match `unwrap` above is safe).
    #[allow(clippy::unwrap_used)]
    let address_m = caps.get(2).unwrap();

    let range = address_m.start()..address_m.end();

    if !range_is_verbatim_or_synthesized(pieces, &range) {
        return None;
    }

    let address = text_slice(nodes, pieces, range.clone())?;
    let location = source_slice(pieces, range, root);

    Some(InlineNode::Ref(Ref {
        variant: RefVariant::Link,
        target: CowStr::from(format!("mailto:{address}")),

        children: vec![InlineNode::Text {
            value: address,
            location,
        }],

        roles: vec![],
        window: None,
        attrs: None,
        resolved: None,
        derived: None,
        xrefstyle: None,
        location,
    }))
}

/// Performs the recognition side effect the string pipeline's five link
/// replacers (`InlineLinkReplacer`'s angle/formal/bare branches,
/// `InlineLinkMacroReplacer`, and `InlineEmailReplacer`) attach to a matched
/// link — registering the target in the document's asset catalog — by walking
/// an already-built tree and reading each [`Ref`](InlineNode::Ref)`{Link}`
/// node's own stored `target` instead of a regex capture. `target` already
/// holds exactly the string the string pipeline registers (see
/// [`build_inline_link_node`], [`build_link_node`], and [`build_email_node`]),
/// so no recomputation is needed.
///
/// Every macro family this module recognizes defers exactly this kind of side
/// effect (see
/// [`image::apply_image_side_effects`](super::image::apply_image_side_effects)'
/// s own note): while the additive builder runs *alongside* the authoritative
/// string pipeline — each against its own, independent [`Parser`] — performing
/// it from every additive pass would risk double-counting a registration once
/// the two paths ever share one `Parser`. This function is that deferred piece
/// for the link family, staged as its own building block for the eventual
/// cutover (design §5.2, Phase 4 step 6): re-attaching it for real means
/// calling it exactly once per parse, after the single-pass builder replaces
/// the recorder as `Content`'s tree source, so nothing here is wired into a
/// real parse yet — it is exercised only by this module's own tests, against
/// their own `Parser`.
///
/// # Registration order across the three link forms
///
/// The string pipeline registers a link's target *when its own replacer's
/// regex pass matches it* — `InlineLinkReplacer` (auto-links and formal-URL
/// links, `INLINE_LINK`'s non-angle branch) runs as one whole-string pass, then
/// `InlineLinkMacroReplacer` (`link:`/`mailto:`, `INLINE_LINK_MACRO`) runs as a
/// *second*, later pass, then `InlineEmailReplacer` (a bare address,
/// `INLINE_EMAIL`) as a *third* — exactly the order [`inline_link_level`],
/// [`link_macro_level`], and [`email_level`] apply the three families in. So
/// the catalog ends up in **family-pass order, not true source order**: every
/// auto-link/formal-URL link in the content registers before every
/// `link:`/`mailto:` macro in it, and both before every bare address,
/// regardless of which appears first in the source (see
/// `catalog_records_link_targets_when_catalog_assets_enabled` in
/// `tests/asciidoctor_rb/substitutions_test.rs`, which pins this exact
/// behavior). A single tree walk in document order would get this wrong for a
/// content that interleaves the forms out of that relative order (for
/// example `link:b.html[B] then https://a.example`, which the golden pipeline
/// registers as `["https://a.example", "b.html"]`, not `["b.html",
/// "https://a.example"]`), so this function makes **three** passes over the
/// tree — all auto-link/formal-URL matches first, then all `link:`/`mailto:`
/// macro matches, then all bare addresses — rather than one. [`link_form`]
/// tells them apart from the node's own `location` and `target` (a
/// `link:`/`mailto:` macro's location always starts with its literal prefix,
/// and only a bare address yields a `mailto:` target without one;
/// [`inline_link_level`] never builds a node for `INLINE_LINK`'s own
/// link-macro branch, deferring that whole form to [`link_macro_level`] — see
/// [`inline_link_level`]'s own doc comment — so this is a reliable,
/// no-recomputation signal, not a heuristic).
///
/// Recurses into every container a `Ref` node can be nested inside —
/// [`Styled`](InlineNode::Styled), another [`Ref`](InlineNode::Ref) (a link's
/// own display children, or a cross-reference's), and
/// [`Footnote`](InlineNode::Footnote) children — mirroring exactly where
/// [`apply_macros`](super::apply_macros) and the footnote increment's own
/// `emit_range` can place one. A cross-reference node itself is not
/// registered — only a [`Link`](RefVariant::Link) has an asset-catalog entry —
/// but its children are still walked, since a formatted cross-reference text
/// could itself carry a nested link.
pub(crate) fn apply_link_side_effects(nodes: &[InlineNode<'_>], parser: &Parser) {
    register_links_of_form(nodes, parser, LinkForm::AutoOrFormal);
    register_links_of_form(nodes, parser, LinkForm::Macro);
    register_links_of_form(nodes, parser, LinkForm::Email);
}

/// Which of the three link-recognizing passes built a
/// [`Ref`](InlineNode::Ref) node — see [`apply_link_side_effects`]'s own
/// "Registration order" note.
#[derive(Clone, Copy, Eq, PartialEq)]
enum LinkForm {
    /// An auto-link or formal-URL link, built by [`inline_link_level`].
    AutoOrFormal,

    /// A `link:`/`mailto:` macro, built by [`link_macro_level`].
    Macro,

    /// A bare e-mail address, built by [`email_level`].
    Email,
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
/// `location` and `target`: only [`link_macro_level`] ever builds a node whose
/// matched source starts with a literal `link:`/`mailto:` prefix, and — of the
/// two passes left — only [`email_level`] builds one whose target carries the
/// `mailto:` scheme ([`inline_link_level`]'s own targets always carry one of
/// [`INLINE_LINK`]'s `https?`/`file`/`ftp`/`irc` schemes instead). See
/// [`apply_link_side_effects`]'s own doc comment.
fn link_form(reference: &Ref<'_>) -> LinkForm {
    let text = reference.location.data();

    if text.starts_with("link:") || text.starts_with("mailto:") {
        LinkForm::Macro
    } else if reference.target.starts_with("mailto:") {
        LinkForm::Email
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
        inlines::{CharRef, InlineNode, SpanForm, StyleVariant},
        parser::HtmlSubstitutionRenderer,
        strings::CowStr,
    };

    #[test]
    fn fold_matches_the_string_pipeline_through_link_macros() {
        // For each fixture, folding the single-pass tree (all five steps)
        // reproduces the string pipeline's output byte-for-byte. This is the
        // differential corpus (design §5.3) that pins the `link:`/`mailto:`
        // macro increment. Every fixture is a *verbatim* `link:`/`mailto:`
        // macro — the boundary this increment claims (a URL target, a
        // multi-line attribute-list text, a display text crossing a rendered
        // span, or a special character inside the macro is deferred and lives
        // in a divergence test below).
        let fixtures = [
            // No link macro despite macro-ish characters.
            "plain text with a colon: but no bracket",
            "a link without a bracket link:index.html stays literal",
            // Link macro: labeled, bare, relative and pathed targets.
            "link:index.html[Docs]",
            "link:downloads/report.pdf[Report]",
            "link:index.html[]",
            "link:index.html[Read the docs]",
            // The `^` suffix opens the link in a new window — including when it
            // is the whole text, which leaves a *bare* link that still opens in
            // one.
            "link:index.html[Open^]",
            "link:index.html[^]",
            "mailto:hello@example.org[^]",
            // An escaped `]` inside the text is unescaped.
            "link:index.html[a\\]b]",
            // A text carrying an attribute list (an `=`): the first positional
            // attribute is the display text, and the named `role` attribute is
            // honored. The `^` suffix still applies after the attrlist split.
            "link:index.html[Docs,role=hl]",
            "link:index.html[Docs,role=hl^]",
            // An `=` that is not a real attribute list (the incidental case).
            "link:index.html[=text]",
            // mailto: labeled and bare (bare shows the address).
            "mailto:hello@example.org[Email us]",
            "mailto:hello@example.org[]",
            // A `mailto:` text carrying a `,` encodes a subject (and, with a
            // second `,`, a body) into the target.
            "mailto:team@example.org[Team,Hello there]",
            "mailto:team@example.org[Team,Hello,Body text]",
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
            // An escaped special (a `CharRef` by macro time) in the target, in
            // the display text, and in both — the match string carries its
            // canonical entity, which is exactly what the string replacer's own
            // haystack holds there.
            "link:a&b.html[x]",
            "link:a&b.html[]",
            "mailto:a&b@example.org[]",
            "mailto:a&b@example.org[Write us]",
            "link:index.html[a < b]",
            "link:index.html[Tom & Jerry]",
            "link:a&b.html[a > b]",
            // The `\]` unescape and the `^` window suffix still apply around a
            // special.
            "link:index.html[a\\] < b]",
            "link:index.html[a < b^]",
            // A special beside the macro, and two specials inside one text.
            "a & b then link:index.html[x < y & z]",
            // An escaped macro crossing a special or a rendered span: the
            // backslash is dropped and the rest stays literal.
            "\\link:a&b.html[x]",
            "\\link:index.html[with *bold* text]",
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
                &build(Span::new(fixture), &parser, None),
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
        // `\link:…` drops the backslash and keeps the macro as literal text — no
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
    fn a_link_macro_target_crossing_an_escaped_special_is_recognized() {
        // The string pipeline matches macros over *escaped* text, so a target
        // containing `&` is matched as `a&amp;b.html` — and the level's match
        // string carries those very bytes for the `CharRef::Special` the
        // `SpecialCharacters` step made, so the node's target is exactly the
        // one the string replacer computed. `target` is a computed value, not
        // an `'src` slice, so nothing here needs the source's own `&`.
        let source = "link:a&b.html[x]";
        let nodes = build_src(Span::new(source));

        let reference = assert_link(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "a&amp;b.html");
        assert_eq!(link_text_of(reference), "x");

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn a_bare_link_macro_shows_its_target_as_structured_children() {
        // A *bare* macro's display text is the target itself, so it is a slice
        // of the target group rather than a computed value: an escaped special
        // in it stays the `CharRef` it already is (with its own precise `'src`
        // span), where one `Text` child holding the already-escaped `a&amp;b`
        // would be escaped a second time by the fold.
        let source = "link:a&b.html[]";
        let nodes = build_src(Span::new(source));

        let reference = assert_link(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "a&amp;b.html");
        assert_eq!(reference.roles, vec![CowStr::from("bare")]);
        assert_eq!(reference.children.len(), 3);

        assert_text(&reference.children[0], "a", 1, 6);

        let InlineNode::CharRef {
            value: CharRef::Special(ch),
            location,
        } = &reference.children[1]
        else {
            panic!("expected a special character: {:?}", reference.children[1]);
        };

        assert_eq!(*ch, '&');
        assert_eq!(location.data(), "&");

        assert_text(&reference.children[2], "b.html", 1, 8);

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn a_bare_mailto_shows_its_address_as_structured_children() {
        // The same treatment for the other bare spelling, whose display text is
        // the address as written (and which takes no `bare` role).
        let source = "mailto:a&b@example.org[]";
        let nodes = build_src(Span::new(source));

        let reference = assert_link(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "mailto:a&amp;b@example.org");
        assert!(reference.roles.is_empty());

        assert_eq!(reference.children.len(), 3);
        assert_text(&reference.children[0], "a", 1, 8);
        assert_text(&reference.children[2], "b@example.org", 1, 10);

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn a_bare_link_macro_under_hide_uri_scheme_slices_past_the_scheme() {
        // Under `hide-uri-scheme` the shown text is the target's
        // scheme-stripped tail — still a *suffix* of the target, since
        // `URI_SNIFF` is `^`-anchored — so the children start that many bytes
        // into the target's own range rather than being recomputed.
        use crate::parser::ModificationContext;

        let parser = Parser::default().with_intrinsic_attribute_bool(
            "hide-uri-scheme",
            true,
            ModificationContext::Anywhere,
        );

        let source = "link:foo:a&b[]";
        let nodes = build(Span::new(source), &parser, None);

        let reference = assert_link(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "foo:a&amp;b");

        // `foo:` is stripped; what remains is `a`, the `&`, and `b`.
        assert_eq!(reference.children.len(), 3);
        assert_text(&reference.children[0], "a", 1, 10);
        assert_text(&reference.children[2], "b", 1, 12);

        assert_eq!(
            crate::content::inline_builder::fold_html(
                &nodes,
                &HtmlSubstitutionRenderer {},
                &parser
            ),
            golden_macros_with(source, &parser)
        );
    }

    #[test]
    fn a_link_macro_display_text_crossing_an_escaped_special_becomes_structured_children() {
        // A display text crossing an escaped special is rebuilt out of the
        // nodes it covers rather than baked into one `Text` child: the special
        // stays the `CharRef` it already is — keeping its own precise `'src`
        // span (#944) — and folds back to the same entity the string replacer's
        // text carries, where a single `Text` holding `&lt;` would be escaped a
        // second time.
        let source = "link:index.html[a < b]";
        let nodes = build_src(Span::new(source));

        let reference = assert_link(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "index.html");
        assert_eq!(reference.children.len(), 3);

        assert_text(&reference.children[0], "a ", 1, 17);

        let InlineNode::CharRef {
            value: CharRef::Special(ch),
            location,
        } = &reference.children[1]
        else {
            panic!("expected a special character: {:?}", reference.children[1]);
        };

        assert_eq!(*ch, '<');
        assert_eq!(location.data(), "<");
        assert_eq!(location.byte_offset(), 18);

        assert_text(&reference.children[2], " b", 1, 20);

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn a_link_macro_display_text_crossing_an_escaped_special_keeps_its_own_escapes() {
        // The structured rebuild still applies `InlineLinkMacroReplacer`'s own
        // `\]` unescape (as a gap in the emitted ranges, so a pair astride two
        // runs is caught too) and still strips the `^` window suffix, which
        // sits one byte past the text the children cover.
        let source = "link:index.html[a\\] < b^]";
        let nodes = build_src(Span::new(source));

        let reference = assert_link(&nodes[0]);
        assert_eq!(reference.window.as_deref(), Some("_blank"));

        // The backslash is a gap in the emitted ranges (so the `]` survives on
        // its own run), the `^` is outside them, and the special is its own
        // child — `link_text_of` sees only the `Text` runs.
        assert_eq!(link_text_of(reference), "a]  b");

        assert!(
            reference.children.iter().any(|child| matches!(
                child,
                InlineNode::CharRef {
                    value: CharRef::Special('<'),
                    ..
                }
            )),
            "the escaped special must survive as its own child: {:?}",
            reference.children
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn an_escaped_link_macro_crossing_a_rendered_span_still_drops_its_backslash() {
        // The escape check runs *before* the gate, mirroring
        // `InlineLinkMacroReplacer`'s own `caps[0].starts_with('\\')`-first
        // order: an escaped macro whose match the gate rejects still drops its
        // backslash and keeps the rest — which, for a rendered span, means the
        // span's own nodes fold back to exactly the markup `caps[0][1..]`
        // emits.
        let source = "\\link:index.html[with *bold* text]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an escaped macro must not build a link node: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn a_link_text_attribute_list_over_a_special_character_is_a_documented_divergence() {
        // A text carrying an attribute list is parsed as a real
        // `Attrlist<'src>`, which reads its own source span's bytes *as
        // content* — and those bytes are the source's `<`, not the `&lt;` the
        // string replacer parses from its escaped copy. That one capture keeps
        // the escaped-special boundary the rest of the family just lifted, for
        // both the `link:` (`=`) and `mailto:` (`,`) spellings.
        //
        // If this boundary is ever lifted, fold these fixtures into the parity
        // corpus above.
        for source in [
            "link:index.html[a < b,role=hl]",
            "mailto:hello@example.org[Sub & ject,Hello there]",
        ] {
            let nodes = build_src(Span::new(source));

            assert!(
                nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
                "an attribute-list text crossing an escaped special must stay literal: {nodes:?}"
            );

            // The string pipeline, by contrast, *does* build a link here.
            assert!(golden_macros(source).contains("<a href"));
        }
    }

    #[test]
    fn a_link_text_attribute_list_populates_the_nodes_attrs() {
        // A `link:` text carrying an `=` splits into an attribute list (here a
        // role): the first positional attribute becomes the display text, and
        // the parsed `Attrlist<'src>` rides on the node's own `attrs` field, so
        // the fold reproduces the role exactly as the string replacer does.
        let source = "link:index.html[Docs,role=hl]";
        let nodes = build_src(Span::new(source));

        let reference = assert_link(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "index.html");
        assert_eq!(link_text_of(reference), "Docs");
        assert_eq!(
            reference
                .attrs
                .as_ref()
                .and_then(|a| a.roles().into_iter().next()),
            Some("hl")
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn a_mailto_subject_and_body_are_encoded_into_the_target() {
        // A `mailto:` text carrying a `,` encodes a `subject` (and optional
        // `body`) into the target, mirroring `InlineLinkMacroReplacer`'s own
        // `extract_attributes_from_text` handling exactly.
        let source = "mailto:team@example.org[Team,Hello there]";
        let nodes = build_src(Span::new(source));

        let reference = assert_link(&nodes[0]);
        assert_eq!(
            reference.target.as_ref(),
            "mailto:team@example.org?subject=Hello%20there"
        );
        assert_eq!(link_text_of(reference), "Team");

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn a_mailto_text_with_a_quoted_comma_and_no_subject_is_not_encoded() {
        // A comma inside a quoted positional value (`"Full, Name"`) is not a
        // subject/body separator: `extract_attributes_from_text` still parses
        // only *one* positional attribute, so `nth_attribute(2)` is `None` and
        // no `?subject=` is appended — the target stays the bare address,
        // exactly as the golden pipeline leaves it.
        let source = "mailto:team@example.org[\"Full, Name\"]";
        let nodes = build_src(Span::new(source));

        let reference = assert_link(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "mailto:team@example.org");
        assert!(!reference.target.contains("subject="));
        assert_eq!(link_text_of(reference), "Full, Name");

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn a_mailto_subject_with_a_body_is_encoded_into_the_target() {
        let source = "mailto:team@example.org[Team,Hello,Body text]";
        let nodes = build_src(Span::new(source));

        let reference = assert_link(&nodes[0]);
        assert_eq!(
            reference.target.as_ref(),
            "mailto:team@example.org?subject=Hello&amp;body=Body%20text"
        );
        assert_eq!(link_text_of(reference), "Team");

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn a_link_text_attribute_list_over_a_multi_line_text_is_a_documented_divergence() {
        // A text carrying an `=`/`,` still needs a real `'src` slice with no
        // embedded newline to carry the parsed `Attrlist<'src>` honestly (see
        // `build_link_node`'s own doc comment); a multi-line attribute-list
        // text is deferred, exactly as the crossed-special/rendered-span forms
        // are.
        let source = "link:index.html[Docs\nmore,role=hl]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "a multi-line attribute-list-in-text link must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, joins the lines with a space and
        // applies the role.
        assert!(golden_macros(source).contains(r#"class="hl""#));
    }

    #[test]
    fn a_mailto_subject_over_a_multi_line_text_is_a_documented_divergence() {
        // The same multi-line boundary as
        // `a_link_text_attribute_list_over_a_multi_line_text_is_a_documented_divergence`,
        // for a `mailto:` text carrying a subject/body.
        let source = "mailto:team@example.org[Team,Hello\nthere]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "a multi-line mailto subject must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, joins the lines with a space and
        // encodes the subject.
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
        // link increment. Every fixture is a *verbatim* link — the boundary
        // this increment claims (a URL crossing a special, a multi-line
        // attribute-list text, or a display text crossing a rendered span is
        // deferred and lives in a divergence test below). The pattern's ANGLE
        // branch has its own corpus, alongside its own structural tests.
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
            // A text carrying an attribute list (an `=`): the first positional
            // attribute is the display text, and the named `role` attribute is
            // honored.
            "https://example.org[Example,role=hl]",
            "https://example.org[Example,role=hl^]",
            // An `=` that is not a real attribute list (the incidental case).
            "https://example.org[=text]",
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
            // An angle-bracketed link is always bare, so it always drops the
            // scheme (the replacer's angle path shares the same `URI_SNIFF`
            // strip).
            "<https://example.org>",
            "see <https://example.org/path> ok",
        ];

        let renderer = HtmlSubstitutionRenderer {};

        for fixture in fixtures {
            let folded = crate::content::inline_builder::fold_html(
                &build(Span::new(fixture), &parser, None),
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
        // the link — it stays as literal text before the node.
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
        // `\https://…` drops the backslash and keeps the URL as literal text — no
        // link node — with the boundary prefix preserved.
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
    fn fold_matches_the_string_pipeline_through_angle_bracketed_links() {
        // The differential corpus for `INLINE_LINK`'s ANGLE branch, whose
        // `&lt;`/`&gt;` delimiters are escaped `CharRef`s by the time macros
        // run — so these fixtures are exactly the ones the whole-match verbatim
        // gate used to defer (see `build_inline_link_node`'s own note). Each is
        // driven through the same fold-vs-string-pipeline comparison the
        // non-angle corpus above uses.
        let fixtures = [
            // The `<url>` form, alone, mid-flow, and repeated.
            "<https://example.org>",
            "see <https://example.org> ok",
            "a <https://example.org> b <ftp://x.example/y> c",
            // Other schemes, paths, and fragments.
            "<file:///etc/hosts>",
            "<irc://irc.example.org/channel>",
            "<https://example.org/path/to/page.html>",
            // Unlike a bare auto-link, the angle form applies no
            // trailing-punctuation strip and no bare-scheme rejection: the
            // whole bracketed body is the target.
            "<http://;>",
            // A following `>` or `.` stays outside the link.
            "<https://example.org>>",
            "<https://example.org>.",
            // Both escapes: a `\` before the `<` and one before the scheme each
            // drop that backslash and leave the rest literal.
            "\\<https://example.org>",
            "<\\https://example.org>",
            // The branch's `[…]` alternative keeps its `&lt;` as literal text
            // before the link, including with an attribute list, an empty
            // (bare) text, and an escaped `<`.
            "<https://example.org[text]",
            "<https://example.org[text]>",
            "<https://example.org[]",
            "<https://example.org[Docs,role=hl]",
            "\\<https://example.org[text]",
            // The branch's third alternative — no closing `>`, no `[…]` — is
            // left wholly literal by both, and still honors both escapes
            // (which the replacer's angle path checks before it reaches that
            // alternative, so the builder must too).
            "<https://example.org",
            "\\<https://example.org",
            "<\\https://example.org",
            // A `<…>` that is not a URL at all stays literal.
            "text <not a url> more",
            // Beside and inside other constructs, including a footnote's own
            // extracted text (which the tree recovers separately from the
            // block's).
            "*<https://example.org>*",
            "a copyright (C) then <https://example.org>",
            "A claim.footnote:[see <https://example.org> for the evidence]",
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
    fn an_angle_bracketed_url_becomes_a_bare_ref_node_consuming_its_delimiters() {
        // The string replacer emits *only* the rendered link for the whole
        // match, so the node consumes its `&lt;`/`&gt;` delimiters rather than
        // keeping them as literal text: one node, whose location covers the
        // brackets too.
        let nodes = build_src(Span::new("<https://example.org>"));

        assert_eq!(nodes.len(), 1);
        let reference = assert_link(&nodes[0]);

        assert_eq!(reference.target.as_ref(), "https://example.org");
        assert_eq!(link_text_of(reference), "https://example.org");
        assert_eq!(reference.roles, [CowStr::from("bare")]);
        assert_eq!(reference.window, None);
        assert!(reference.attrs.is_none());

        assert_eq!(reference.location.data(), "<https://example.org>");
        assert_eq!(reference.location.line(), 1);
        assert_eq!(reference.location.col(), 1);
    }

    #[test]
    fn an_angle_bracketed_url_keeps_no_trailing_punctuation_strip() {
        // A bare auto-link strips a trailing ';'/':' off its target; the angle
        // form does not, because the replacer's angle path takes the bracketed
        // body verbatim. `<http://;>` is therefore a link whose target *is*
        // `http://;` — the very target the bare branch rejects as a bare
        // scheme with no body.
        let nodes = build_src(Span::new("<http://;>"));

        assert_eq!(nodes.len(), 1);
        let reference = assert_link(&nodes[0]);

        assert_eq!(reference.target.as_ref(), "http://;");
        assert_eq!(link_text_of(reference), "http://;");
    }

    #[test]
    fn an_angle_bracketed_url_with_a_bracketed_text_keeps_its_opening_delimiter() {
        // The ANGLE branch's `[…]` alternative goes through the *general* path,
        // which keeps the match's boundary prefix — here the `&lt;` `CharRef` —
        // as literal text before the link node.
        let nodes = build_src(Span::new("<https://example.org[text]"));

        assert_eq!(nodes.len(), 2);

        match &nodes[0] {
            InlineNode::CharRef { value, .. } => {
                assert_eq!(*value, CharRef::Special('<'));
            }

            other => panic!("expected the kept `<` CharRef, got {other:?}"),
        }

        let reference = assert_link(&nodes[1]);
        assert_eq!(reference.target.as_ref(), "https://example.org");
        assert_eq!(link_text_of(reference), "text");
        assert_eq!(reference.location.data(), "https://example.org[text]");
    }

    #[test]
    fn an_unterminated_angle_bracketed_url_is_left_literal() {
        // The ANGLE branch's third alternative — no closing `&gt;` and no
        // `[…]` — is emitted unchanged by the string replacer, so the builder
        // builds no node for it either (and, because the branch's own match
        // consumed the URL, no *non-angle* auto-link is found inside it).
        let nodes = build_src(Span::new("<https://example.org"));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an unterminated angle-bracketed URL must stay literal: {nodes:?}"
        );
    }

    #[test]
    fn an_angle_bracketed_url_over_a_special_character_is_a_documented_divergence() {
        // The delimiters of an angle link may be escaped specials — that is the
        // whole point of the relaxed gate — but the URL *between* them may not:
        // a `&` in it is an atomic `CharRef` the node cannot slice from `'src`,
        // exactly the boundary `an_auto_link_over_a_special_character_is_a_
        // documented_divergence` pins for the non-angle branch.
        let nodes = build_src(Span::new("<https://example.org/?a=1&b=2>"));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an angle URL crossing an escaped special must stay literal: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* build a link here.
        assert!(golden_macros("<https://example.org/?a=1&b=2>").contains("<a href"));
    }

    #[test]
    fn an_angle_bracketed_link_text_over_a_rendered_span_is_a_documented_divergence() {
        // The `[…]` alternative inherits the general path's own deferral: a
        // display text containing a quoted span is a `Styled` node by macro
        // time, so the match is not verbatim.
        let source = "<https://example.org[*bold*]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an angle link text crossing a rendered span must stay literal: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* build a link here.
        assert!(golden_macros(source).contains("<a href"));
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
    fn a_formal_url_link_attribute_list_populates_the_nodes_attrs() {
        // A formal URL text carrying an `=` splits into an attribute list
        // (here a role): the first positional attribute becomes the display
        // text, and the parsed `Attrlist<'src>` rides on the node's own
        // `attrs` field, so the fold reproduces the role exactly as the
        // string replacer does.
        let source = "https://example.org[Example,role=hl]";
        let nodes = build_src(Span::new(source));

        let reference = assert_link(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "https://example.org");
        assert_eq!(link_text_of(reference), "Example");
        assert_eq!(
            reference
                .attrs
                .as_ref()
                .and_then(|a| a.roles().into_iter().next()),
            Some("hl")
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn a_formal_url_link_attribute_list_over_a_multi_line_text_is_a_documented_divergence() {
        // The same multi-line boundary `build_link_node` documents: an
        // honest `'src` slice is available only when the bracketed text has
        // no embedded newline.
        let source = "https://example.org[Example\nmore,role=hl]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "a multi-line attribute-list-in-text link must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, joins the lines with a space and
        // applies the role.
        assert!(golden_macros(source).contains(r#"class="hl""#));
    }

    #[test]
    fn an_incidental_equals_in_link_text_is_not_an_attribute_list() {
        // `=text` contains an `=`, but no valid attribute name precedes it, so
        // the attrlist parse yields one positional value spanning the *whole*
        // text rather than a named attribute — the `=` was incidental,
        // mirroring `InlineLinkReplacer`'s own `extract_attributes_from_text`
        // fallback (the same case `xref`'s own part 3c increment pins).
        let source = "https://example.org[=text]";
        let nodes = build_src(Span::new(source));

        let reference = assert_link(&nodes[0]);
        assert_eq!(link_text_of(reference), "=text");
        assert!(reference.attrs.is_none() || reference.attrs.as_ref().unwrap().roles().is_empty());

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn a_link_display_text_over_a_rendered_span_is_a_documented_divergence() {
        // The macros step matches over *escaped, already-rendered* text, so a
        // display text containing a quoted span (`*bold*`) has already become
        // a `Styled` node by the time macros run — an opaque piece the node's
        // single `Text` child cannot absorb without becoming structured
        // children (the same shape a footnote's own content needs). Left
        // unrecognized for a later increment.
        let source = "link:https://example.org[with *bold* text]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "a display text crossing a rendered span must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* build a link here, with the
        // span rendered inside the anchor text.
        assert!(golden_macros(source).contains("<a href"));
        assert!(golden_macros(source).contains("<strong>bold</strong>"));
    }

    // ---- bare e-mail addresses (`INLINE_EMAIL`) ---------------------------

    #[test]
    fn fold_matches_the_string_pipeline_through_bare_emails() {
        // For each fixture, folding the single-pass tree (all five steps)
        // reproduces the string pipeline's output byte-for-byte. This is the
        // differential corpus (design §5.3) that pins the bare e-mail
        // increment: the address forms the pattern claims, the prefixes it
        // treats as mismatches, and the escape.
        let fixtures = [
            // No address despite an `@`.
            "an @ sign on its own",
            "@example.org has no local part",
            "user@ has no domain",
            // Plain addresses, in and out of surrounding flow.
            "doc.writer@example.com",
            "Write to doc.writer@example.com today.",
            "doc_writer@example.com",
            "doc-writer+tag@example.com",
            "doc%writer@example.com",
            "a@b.io",
            // Subdomains, hyphens, and digits in the domain.
            "info@mail.example.co.uk",
            "info@my-host.example.com",
            "info@example2.com",
            // Two addresses in one content.
            "first@example.org and second@example.org",
            // A TLD outside the pattern's 2–5 letter range is not an address.
            "user@example.c",
            "user@example.toolongtld",
            // The mismatch prefixes: a `mailto:` macro's own target (`:`), a
            // URL's user-info or path (`/`), and a `>` right before the local
            // part.
            "mailto:hello@example.org[Email us]",
            "mailto:hello@example.org[]",
            "https://example.org/x@y.com",
            "see https://user@example.org/path here",
            // Escapes: the address stays literal, minus the backslash.
            "\\doc.writer@example.com",
            "a \\doc@example.com b",
            // An address beside and inside other constructs.
            "*bold* then doc@example.com and _em_",
            "*write to doc@example.com*",
            "a copyright (C) then doc@example.com",
            "link:index.html[Docs] then doc@example.com",
            "https://example.org then doc@example.com",
            // An address inside a footnote's own text (extracted after the
            // e-mail pass has already recognized it, exactly as the string
            // pipeline substitutes it before the footnote text is pulled out).
            "A claim.footnote:[write to doc@example.com]",
            // An address inside an anchor's reference text and beside an
            // anchor (both later passes).
            "[[the-anchor]]doc@example.com",
        ];

        for fixture in fixtures {
            let nodes = build_src(Span::new(fixture));

            assert_eq!(
                fold_html(&nodes, &HtmlSubstitutionRenderer {}),
                golden_macros(fixture),
                "fold diverged for {fixture:?}"
            );
        }
    }

    #[test]
    fn a_bare_email_becomes_a_ref_node() {
        // The address is the node's display text and, prefixed with `mailto:`,
        // its target; unlike a bare URL auto-link it takes no `bare` role. The
        // node's own location is the address itself, sliced precisely from
        // `'src`.
        let source = "Write to doc.writer@example.com today.";
        let nodes = build_src(Span::new(source));

        let reference = nodes
            .iter()
            .find_map(|n| match n {
                InlineNode::Ref(reference) => Some(reference),
                _ => None,
            })
            .unwrap_or_else(|| panic!("expected a Ref node: {nodes:?}"));

        assert_eq!(reference.target.as_ref(), "mailto:doc.writer@example.com");
        assert_eq!(link_text_of(reference), "doc.writer@example.com");
        assert!(reference.roles.is_empty());
        assert!(reference.window.is_none());
        assert!(reference.attrs.is_none());

        assert_eq!(reference.location.data(), "doc.writer@example.com");
        assert_eq!(reference.location.line(), 1);
        assert_eq!(reference.location.col(), 10);

        // The display text borrows the very bytes its location covers.
        assert_text(&reference.children[0], "doc.writer@example.com", 1, 10);

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn an_escaped_email_stays_literal() {
        // `\doc@example.com` drops the single backslash and leaves the address
        // as literal text — no link node — mirroring the string replacer's
        // `caps[0][1..]`.
        let source = "\\doc@example.com";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an escaped address must stay literal: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
        assert_eq!(golden_macros(source), "doc@example.com");
    }

    #[test]
    fn a_mailto_macro_target_is_not_re_recognized_as_a_bare_email() {
        // The e-mail pass runs *after* the `link:`/`mailto:` macro pass, so by
        // then the macro is one opaque node here (already-rendered `<a …>`
        // markup in the string pipeline, whose `mailto:` prefix the pattern's
        // own `:` mismatch group rejects). Either way exactly one link node
        // results, and it is the macro's.
        let source = "mailto:hello@example.org[Email us]";
        let nodes = build_src(Span::new(source));

        let refs: Vec<_> = nodes
            .iter()
            .filter(|n| matches!(n, InlineNode::Ref(_)))
            .collect();

        assert_eq!(refs.len(), 1, "expected exactly one link node: {nodes:?}");

        let reference = assert_link(refs[0]);
        assert_eq!(reference.target.as_ref(), "mailto:hello@example.org");
        assert_eq!(link_text_of(reference), "Email us");
    }

    #[test]
    fn an_email_is_recognized_inside_a_span() {
        // The macros step descends into a `Styled` span's children before
        // matching at its own level, so an address inside a quoted span is
        // recognized there.
        let source = "*write to doc@example.com*";
        let nodes = build_src(Span::new(source));

        let children = assert_styled(&nodes[0], StyleVariant::Strong, SpanForm::Constrained);

        assert!(
            children.iter().any(|n| matches!(n, InlineNode::Ref(_))),
            "expected an address inside the span: {children:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn an_email_inside_an_expanded_attribute_value_is_recognized() {
        // An address whose bytes come from a *synthesized* run (an attribute
        // reference's resolved value) — recovered exactly by `text_slice`,
        // since an e-mail node carries only plain text (design §3.4.1's "a
        // macro inside an expanded value" boundary). The two URL-link passes
        // now make the same lift for their own targets and display texts; the
        // one part of this family that still needs an honest `'src` slice is
        // an attribute-list-bearing display text (see
        // `a_link_attribute_list_text_inside_an_expanded_value_is_a_documented_divergence`).
        // Byte-parity for this shape is pinned by the whole-pipeline corpus in
        // this module's `mod.rs`, which runs the real `AttributeReferences`
        // step.
        use crate::parser::ModificationContext;

        let parser = Parser::default().with_intrinsic_attribute(
            "contact",
            "doc@example.com",
            ModificationContext::Anywhere,
        );

        let source = "write to {contact} now";
        let nodes = build(Span::new(source), &parser, None);

        let reference = nodes
            .iter()
            .find_map(|n| match n {
                InlineNode::Ref(reference) => Some(reference),
                _ => None,
            })
            .unwrap_or_else(|| panic!("expected a Ref node: {nodes:?}"));

        assert_eq!(reference.target.as_ref(), "mailto:doc@example.com");
        assert_eq!(link_text_of(reference), "doc@example.com");

        // The address has no honest `'src` slice of its own, so its location
        // falls back to the enclosing synthesized run's coarse span (design
        // §4.4) while its text stays exact.
        assert_eq!(reference.location.data(), "{contact}");
    }

    #[test]
    fn an_email_abutting_a_rendered_construct_stays_literal() {
        // The mismatch-prefix group reads the character immediately before the
        // address. The string pipeline reads it out of already-rendered markup
        // — `</strong>`, `</a>`, and `<img …>` all end in `>`, a mismatch
        // character — and so leaves the address literal. The tree stands the
        // construct in as one opaque placeholder, which belongs to no mismatch
        // class, so `find_email_matches` defers explicitly instead of building
        // a link the string pipeline does not: parity, not a divergence, for
        // every construct whose rendering ends in one of `\`, `>`, `:`, `/`.
        for source in [
            "**bold**doc@example.com",
            "__em__doc@example.com",
            "link:index.html[Docs]doc@example.com",
            "https://example.org[Site]doc@example.com",
            "image:x.png[]doc@example.com",
            "icon:home[]doc@example.com",
        ] {
            let nodes = build_src(Span::new(source));

            assert!(
                !nodes.iter().any(
                    |n| matches!(n, InlineNode::Ref(reference) if reference.target.starts_with("mailto:"))
                ),
                "an address abutting a rendered construct must stay literal: {nodes:?}"
            );

            assert_eq!(
                fold_html(&nodes, &HtmlSubstitutionRenderer {}),
                golden_macros(source),
                "fold diverged for {source:?}"
            );
        }
    }

    #[test]
    fn an_email_abutting_a_construct_that_hides_its_boundary_is_a_documented_divergence() {
        // What the unconditional deferral above costs, pinned exactly. In each
        // of these the string pipeline's mismatch-prefix group does *not* see a
        // mismatch character before the address, so it links it — a concealed
        // index term renders to nothing, and a passthrough or STEM expression
        // is still masked by its own sentinel when the macros step runs (it is
        // restored afterwards), so neither presents rendered markup there. The
        // tree cannot tell those apart from a construct that *did* render
        // markup without folding the preceding node while building, so all of
        // them defer — see `email_level`'s own scope note.
        use super::super::super::test_support::golden_passthroughs;

        let concealed_term = "indexterm:[a]doc@example.com";
        let nodes = build_src(Span::new(concealed_term));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an address abutting an opaque construct must stay literal: {nodes:?}"
        );
        assert!(golden_macros(concealed_term).contains(r#"href="mailto:doc@example.com""#));

        // A passthrough and a STEM expression, whose goldens need the
        // extract/restore pass around the steps.
        for source in [
            "+++raw/+++doc@example.com",
            "pass:[x]doc@example.com",
            "stem:[x]doc@example.com",
        ] {
            let nodes = build_src(Span::new(source));

            assert!(
                nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
                "an address abutting an opaque construct must stay literal: {nodes:?}"
            );

            assert!(
                golden_passthroughs(source).contains(r#"href="mailto:doc@example.com""#),
                "golden fixture stopped linking the address for {source:?}"
            );
        }
    }

    #[test]
    fn an_auto_link_abutting_a_rendered_span_is_a_documented_divergence() {
        // The mirror image of the boundary above, in the sibling auto-link
        // family, which predates the e-mail pass: `INLINE_LINK`'s own
        // boundary-prefix group *requires* one of `^`, a blank, or
        // `[>()\[\];"']`. The string pipeline reads `</strong>`'s own `>`
        // there and builds a link; the placeholder standing in for the span
        // here belongs to no such class, so the pattern simply fails to match
        // and the URL is left literal. Pinned here so the two directions of
        // this one boundary are recorded together.
        let source = "**bold**https://example.org";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "a URL abutting a rendered span must be left unrecognized: {nodes:?}"
        );

        assert!(golden_macros(source).contains(r#"href="https://example.org""#));
    }

    #[test]
    fn an_email_over_a_special_character_is_a_documented_divergence() {
        // The pattern's local-part class admits `&amp;`, so the string
        // pipeline matches an address carrying a literal `&` over its own
        // *escaped* text. That `&amp;` is an atomic `CharRef` by the time
        // macros run, so the builder cannot recover the address as text and
        // leaves it unrecognized for a later increment — the same boundary the
        // auto-link and image families document.
        let source = "a&b@example.com";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an address crossing an escaped special must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* build a link here.
        assert!(golden_macros(source).contains("<a href"));
    }

    // ---- `apply_link_side_effects` (staged for the eventual cutover) ------

    use super::apply_link_side_effects;

    /// Builds the single-pass tree for `source` against `parser` (unlike
    /// [`build_src`], which always uses its own fresh default parser).
    fn build_with<'src>(source: Span<'src>, parser: &Parser) -> Vec<InlineNode<'src>> {
        build(source, parser, None)
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
    fn registers_an_angle_bracketed_target_in_the_auto_link_pass() {
        // The angle form is `InlineLinkReplacer`'s own branch — the *first* of
        // the three link passes — so it registers alongside (and in source
        // order with) the auto-links and formal-URL links, before any
        // `link:`/`mailto:` macro. `link_form` reaches that classification with
        // no angle-specific case: the node's location does not start with a
        // `link:`/`mailto:` prefix, and its target is not a `mailto:` one.
        let source = "link:b.html[B] then <https://a.example> then https://c.example";
        let parser = Parser::default().with_catalog_assets(true);
        let nodes = build_with(Span::new(source), &parser);

        apply_link_side_effects(&nodes, &parser);

        assert_eq!(
            parser.catalog().links(),
            ["https://a.example", "https://c.example", "b.html"]
        );

        // The golden string pipeline agrees.
        let golden_parser = Parser::default().with_catalog_assets(true);
        golden_macros_with(source, &golden_parser);
        assert_eq!(golden_parser.catalog().links(), parser.catalog().links());
    }

    #[test]
    fn registers_interleaved_forms_in_family_pass_order_not_source_order() {
        // `link:b.html[B]` appears first in the source, but the golden
        // pipeline's `link:`/`mailto:` pass runs *after* its auto-link/
        // formal-URL pass (see `apply_link_side_effects`'s own "Registration
        // order" doc note), so `https://a.example` — which appears second —
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
    fn registers_a_bare_email_target_with_its_scheme() {
        let source = "write to doc@example.com";
        let parser = Parser::default().with_catalog_assets(true);
        let nodes = build_with(Span::new(source), &parser);

        apply_link_side_effects(&nodes, &parser);

        assert_eq!(parser.catalog().links(), ["mailto:doc@example.com"]);
    }

    #[test]
    fn registers_a_bare_email_after_both_url_link_forms() {
        // The e-mail pass is the *third* of the three link-recognizing passes,
        // so a bare address registers after every auto-link/formal-URL link
        // and every `link:`/`mailto:` macro in the content — regardless of
        // where it appears in the source (see `apply_link_side_effects`'s own
        // "Registration order" doc note).
        let source = "first@example.org then link:b.html[B] then https://a.example";
        let parser = Parser::default().with_catalog_assets(true);
        let nodes = build_with(Span::new(source), &parser);

        apply_link_side_effects(&nodes, &parser);

        assert_eq!(
            parser.catalog().links(),
            ["https://a.example", "b.html", "mailto:first@example.org"]
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
        //
        // The separators are plain spaces, not `{sp}` attribute references:
        // `golden_macros_with` deliberately skips the `AttributeReferences`
        // step (see its own doc comment), so a reference in a fixture makes the
        // two sides read *different* text — latent while every link family
        // deferred inside a synthesized run, but live now that they no longer
        // do (a `{sp}` before a bare URL leaves the golden a `}` boundary
        // character, which `INLINE_LINK` rejects, while the builder sees the
        // expanded space and links it).
        let fixtures = [
            "link:index.html[Docs]",
            "link:[]",
            "mailto:hello@example.org[Email us]",
            "mailto:[]",
            "https://example.org",
            "https://example.org[Example]",
            "link:https://example.org[Example]",
            "\\link:index.html[Docs]",
            "link:a.html[A] link:b.html[B]",
            // Interleaved forms, out of source order relative to the family
            // passes that register them (see `apply_link_side_effects`'s own
            // "Registration order" doc note).
            "link:b.html[B] then https://a.example",
            // The bare e-mail form, alone and interleaved with both URL-link
            // forms — it registers last of the three, wherever it appears.
            "doc@example.com",
            "\\doc@example.com",
            "doc@example.com then link:b.html[B]",
            "a@example.org link:b.html[B] https://c.example d@example.org",
            // A macro whose target crosses an escaped special registers the
            // *escaped* target, exactly as the string replacer does.
            "link:a&b.html[x]",
            "mailto:a&b@example.org[]",
            "link:a&b.html[x] then link:c.html[C]",
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

    /// A parser carrying the attributes the expanded-value fixtures below
    /// reference.
    fn expanding_parser() -> Parser {
        use crate::parser::ModificationContext;

        Parser::default()
            .with_intrinsic_attribute("url", "index.html", ModificationContext::Anywhere)
            .with_intrinsic_attribute("host", "example.org", ModificationContext::Anywhere)
            .with_intrinsic_attribute("addr", "hello@example.org", ModificationContext::Anywhere)
            .with_intrinsic_attribute("label", "Docs", ModificationContext::Anywhere)
            .with_intrinsic_attribute("site", "https://example.org", ModificationContext::Anywhere)
            .with_intrinsic_attribute(
                "link-src",
                "link:index.html[Docs]",
                ModificationContext::Anywhere,
            )
    }

    /// The real, public pipeline's output for `source` — the golden for the
    /// expanded-value fixtures, which need the `AttributeReferences` step
    /// [`golden_macros_with`] deliberately omits.
    fn golden_normal(source: &str, parser: &Parser) -> String {
        use crate::content::{Content, SubstitutionGroup};

        let mut content = Content::from(Span::new(source));
        SubstitutionGroup::Normal.apply(&mut content, parser, None);
        content.rendered_str().to_string()
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_links_inside_expanded_values() {
        // A link whose target or display text crosses a synthesized run (an
        // attribute expansion) is now recognized: every value these nodes hold
        // is computed out of the level's match string, which carries an
        // expanded value's bytes exactly, so only the node's `location` takes
        // design §4.4's coarse fallback. The two shapes that still defer — an
        // attribute-list-bearing display text, and a wholly expanded
        // `link:`/`mailto:` macro — have their own divergence tests below.
        let parser = expanding_parser();

        let fixtures = [
            // The `link:`/`mailto:` macro with an expanded target: labeled,
            // bare, and with the `^` new-window suffix.
            "link:{url}[Docs]",
            "link:{url}[]",
            "link:{url}[Open^]",
            "mailto:{addr}[Team]",
            "mailto:{addr}[]",
            // An expanded *display text* beside a verbatim target.
            "link:index.html[{label}]",
            "link:{url}[{label}]",
            // Auto-links and formal-URL links over an expanded host.
            "https://{host}",
            "https://{host}/path",
            "https://{host}[Example]",
            "https://{host}[{label}]",
            // A wholly expanded auto-link (the URL-link passes need no
            // literal marker of their own, unlike the `link:` macro).
            "see {site} now",
            "see {site}[Home] now",
            // The angle-bracketed spellings.
            "<https://{host}>",
            "<https://{host}[Example]",
            // Embedded in surrounding flow, beside another link, and inside a
            // rendered span.
            "See link:{url}[Docs] here.",
            "link:{url}[A] and link:other.html[B]",
            "*link:{url}[Docs]*",
            // The escapes still keep the macro literal.
            "\\link:{url}[Docs]",
            "\\https://{host}",
        ];

        for fixture in fixtures {
            let folded = crate::content::inline_builder::fold_html(
                &build(Span::new(fixture), &parser, None),
                &HtmlSubstitutionRenderer {},
                &parser,
            );

            assert_eq!(
                folded,
                golden_normal(fixture, &parser),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn a_link_inside_an_expanded_value_keeps_a_coarse_location() {
        // The target and display text are recovered *exactly* from the match
        // string, while the node's `location` falls back to the enclosing
        // synthesized run's own span — design §4.4's documented split.
        let parser = expanding_parser();
        let source = "link:{url}[{label}]";
        let nodes = build(Span::new(source), &parser, None);

        assert_eq!(nodes.len(), 1);
        let reference = assert_link(&nodes[0]);

        assert_eq!(reference.target.as_ref(), "index.html");
        assert_eq!(link_text_of(reference), "Docs");

        // The macro's own source bytes: its `link:` marker is verbatim (which
        // is what this pass requires), so the location is honest end to end
        // here even though both captures came from expansions.
        assert_eq!(reference.location.data(), source);
    }

    #[test]
    fn a_wholly_expanded_link_macro_is_a_documented_divergence() {
        // A `link:`/`mailto:` macro whose own marker comes from the expansion
        // has no location starting with that marker, and that location is the
        // signal `link_form` reads to replay the string pipeline's family-pass
        // registration order. Rather than build a node the side-effect walk
        // would then mis-attribute, the macro is left literal.
        //
        // If this boundary is ever lifted (with a signal that does not depend
        // on the location), fold this fixture into the parity corpus above.
        let parser = expanding_parser();
        let source = "see {link-src} now";

        let nodes = build(Span::new(source), &parser, None);

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "a wholly expanded link macro must stay literal: {nodes:?}"
        );

        assert!(
            golden_normal(source, &parser).contains("<a href"),
            "the golden fixture stopped recognizing the link"
        );
    }

    #[test]
    fn a_link_attribute_list_text_inside_an_expanded_value_is_a_documented_divergence() {
        // A display text carrying an attribute list is parsed as a real
        // `Attrlist<'src>`, which reads its own source span's bytes *as
        // content* — so, unlike every other value these nodes hold, it cannot
        // be taken from the match string. Both link-recognizing passes defer
        // it, as does a `mailto:` text carrying a `,` subject.
        //
        // If this boundary is ever lifted, fold these fixtures into the parity
        // corpus above.
        let parser = expanding_parser();

        for source in [
            "link:index.html[{label},role=hl]",
            "https://example.org[{label},role=hl]",
            "mailto:hello@example.org[{label},Hi there]",
        ] {
            let nodes = build(Span::new(source), &parser, None);

            assert!(
                nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
                "a link whose attribute-list text crosses an expansion must stay literal: \
                 {nodes:?}"
            );

            assert!(
                golden_normal(source, &parser).contains("<a href"),
                "the golden fixture stopped recognizing the link for {source:?}"
            );
        }
    }

    #[test]
    fn matches_the_golden_pipelines_registration_for_links_inside_expanded_values() {
        // The staged `register_link` side effect classifies each node by the
        // pass that built it, from the node's own `location` — which is exactly
        // why `link_macro_level` still requires its `link:`/`mailto:` marker to
        // be verbatim. These fixtures interleave the two URL-link passes' forms
        // over expanded values, in both relative orders, so a misclassification
        // would show up as the wrong catalog order.
        let fixtures = [
            "link:{url}[Docs]",
            "https://{host}",
            "link:{url}[Docs] and https://{host}",
            "https://{host} then link:{url}[Docs]",
            "see {site} now",
            "mailto:{addr}[Team] and https://{host}",
        ];

        for fixture in fixtures {
            let builder_parser = expanding_parser().with_catalog_assets(true);
            let nodes = build(Span::new(fixture), &builder_parser, None);
            apply_link_side_effects(&nodes, &builder_parser);

            let golden_parser = expanding_parser().with_catalog_assets(true);
            golden_normal(fixture, &golden_parser);

            assert_eq!(
                builder_parser.catalog().links(),
                golden_parser.catalog().links(),
                "registered links diverged for {fixture:?}"
            );
        }
    }
}
