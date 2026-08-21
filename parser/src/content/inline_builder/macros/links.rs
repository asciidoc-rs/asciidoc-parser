//! Auto-link, formal-URL-link, and `link:`/`mailto:` macro recognition.

use super::{
    MacroMatch, MacroMatchKind, escaped_value_children,
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
            LevelContext, Piece, SPAN_PLACEHOLDER, build_match_string, source_slice,
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
/// their verbatim
/// forms, reproducing the string replacer's boundary-prefix
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
/// - A **target** — or, for a bare link, the whole match, whose shown text is a
///   slice of that same target — crossing an **opaque** piece: a rendered
///   [`Styled`](crate::inlines::Styled) span, or any other construct
///   [`build_match_string`] stands in as one [`SPAN_PLACEHOLDER`]. A
///   **bracketed display text** crossing one is admitted (see below).
/// - A **bare URL whose stripped trailing punctuation is not its own**: the
///   strip keys off the target's final character (`;` or `:`, plus an adjacent
///   `)`), and a bare URL ending in an escaped special has an entity there
///   (`https://example.org/a&` reaches this pass as `…/a&amp;`), whose own
///   final `;` the strip would cut *inside* the [`CharRef`](InlineNode::CharRef)
///   leaf — a boundary no node can express (see [`build_inline_link_node`]).
///
/// A [`synthesized`](Piece::synthesized) run (an attribute expansion, or —
/// reached at a tree's root — a filtered multi-line block's own joined seed)
/// **is** admitted: every value this pass's nodes hold — the scheme, the URL,
/// and the bracketed display text — is computed out of the level's match
/// string, which carries a synthesized run's bytes exactly, so
/// `https://{host}/path` and `{url}[Docs]` are recognized with only the node's
/// `location` taking design §4.4's coarse fallback. A **formal text carrying
/// an attribute list** (an `=` selecting roles / id / title / window) is no
/// exception any more: [`text_attrlist`] parses that list from the same match
/// string when the text has no `'src` slice of its own, and owns the result
/// off it, so `{url}[{label},role=hl]` — and a text spanning two lines, which
/// the parse joins with a space — is recognized too.
///
/// An **escaped special** ([`CharRef`](InlineNode::CharRef)`::Special`) is
/// admitted too, in an attribute-list text as much as anywhere else
/// (`https://example.org/?a=1&b=2`, `https://example.org[a < b]`,
/// `<https://example.org/a&b>`) — the third family to take
/// [`range_has_no_opaque_piece`], after the cross-reference and
/// `link:`/`mailto:` macro families. The match string carries such a leaf's
/// canonical entity — the very bytes the string replacer's own escaped haystack
/// holds there — so the target this pass computes off that string
/// (`https://example.org/?a=1&amp;b=2`) *is* the one the replacer computed, and
/// no value on the node needs the source's own `<`/`>`/`&`. The display text
/// then becomes **structured children** rather than one baked `Text` (see
/// [`macro_text_children`]), so the special folds back to its own entity
/// instead of being escaped twice — for a bare URL too, whose shown text is a
/// slice of the URL's own range rather than a computed string.
///
/// # A rendered span inside the display text
///
/// A **rendered span** — a [`Styled`](crate::inlines::Styled) span, an
/// already-recognized macro node of another family, a masked passthrough — is
/// *not* recoverable: [`build_match_string`] stands it in as one
/// [`SPAN_PLACEHOLDER`] where the string pipeline's haystack holds its markup
/// (or its own passthrough mask) inline, and that markup exists only at fold
/// time. It is nonetheless admitted **inside the bracketed display text** of a
/// formal URL link (`https://example.org[a *b* c]`, and the angle spelling
/// `<https://example.org[a *b* c]` that keeps its `&lt;`), because that text is
/// the one capture this family never reads as bytes: it becomes the node's
/// children through [`macro_text_children`], whose
/// [`emit_range`](super::super::quotes::emit_range) path clones the opaque
/// piece's own node whole into them — so the text is carried *structurally*,
/// and the fold re-renders exactly the markup the string replacer captured
/// there. Everything this pass *computes* stays gated: the **target**, and
/// with it a **bare** link's shown text (a slice of the target's own range)
/// and the `<url>` form's whole interior (see [`build_angle_link_node`]). An
/// **attribute-list text** is computed too — its display text comes back from
/// an [`Attrlist`] parse rather than from a range — so [`text_attrlist`] keeps
/// the same gate there: a placeholder inside a *parsed* value has no node it
/// can be mapped back to. This is the third family to take that lift, after
/// the cross-reference one (`xref::find_xref_matches`) and the
/// `link:`/`mailto:` macro ([`find_link_macro_matches`]).
///
/// What the admission cannot do is make the *recognition* agree in every case,
/// because the string replacer matches over the markup itself where this
/// matches over one placeholder standing in for it — and this pass cannot know
/// what that markup carries without folding, which building a tree must never
/// do. The two read the same extent unless the markup carries a character the
/// pattern (or the replacer's own attribute-list probe) is sensitive to, which
/// leaves two documented divergences of *extent*, each pinned by its own test
/// and each one where the string pipeline's reading is the markup-perturbed one
/// and the tree's the well-formed one — exactly as the quotes step's own
/// crossed-delimiter divergence is:
///
/// - a `]` inside the span (`https://example.org[a *b ] c* d]`), which ends
///   [`INLINE_LINK`]'s own lazy text capture early for the string replacer but
///   not here;
/// - markup carrying an `=` beside a comma elsewhere in the text
///   (`…example.org[one, [.hl]#two#]`): the replacer's attribute-list probe
///   fires on the markup's own `=`, and the parse then keeps only what precedes
///   that comma.
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
    ctx: LevelContext,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes);

    // Cheap pre-filter mirroring the string step's guard: an auto-link needs a
    // `://` scheme separator somewhere in the level.
    if !s.contains("://") {
        return nodes;
    }

    // Matched over the level wrapped in the boundary character its enclosing
    // construct presents, with the level's own pieces moved into that string's
    // coordinates — see `apply_macro_families`'s own doc comment.
    let (s, pieces) = ctx.shift(s, pieces);

    let matches = find_inline_link_matches(&nodes, &s, &pieces, root, parser);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every recognized auto-link / formal-URL / angle-bracketed link at this
/// level, skipping a `link:` macro match and any form
/// [`build_inline_link_node`] defers — including one whose *computed* bytes
/// cross an opaque piece, whose gate lives inside that function (it needs the
/// branch's own capture groups to know which sub-range the gate covers, and
/// must run after the escape checks; see [`inline_link_level`]).
fn find_inline_link_matches<'src>(
    nodes: &[InlineNode<'src>],
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

        match build_inline_link_node(&n, &full, nodes, pieces, root, parser) {
            Some(m) => matches.push(m),

            // A deferred or invalid form (an escaped scheme is handled inside as
            // an `Unescape`; a match crossing an opaque piece, a quoted bare
            // URL, a bare scheme, or an unterminated angle-bracketed URL is
            // left as literal source).
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
/// The bytes this pass *computes* off the match string — the boundary prefix it
/// inspects, the scheme, and the URL that becomes the target — must not cross
/// an **opaque** piece: a rendered [`Styled`](crate::inlines::Styled) span, or
/// anything else [`build_match_string`] stands in as one [`SPAN_PLACEHOLDER`].
/// The gate therefore covers the match up to the bracketed display text, which
/// reads nothing and is carried structurally (see [`inline_link_level`]'s own
/// rendered-span section) — and, for a **bare** link, the whole match, whose
/// shown text is a slice of the target's own range. The check sits here, not in
/// [`find_inline_link_matches`], for two reasons: it must run *after* the
/// escape checks (so an escaped link the gate would reject still drops its
/// backslash, mirroring the replacer's own check order), and the ANGLE
/// branch's `<url>` form gates only its own interior (see
/// [`build_angle_link_node`]).
///
/// A [`synthesized`](Piece::synthesized) run and an *escaped special* (a
/// [`CharRef`](InlineNode::CharRef)`::Special`) are both admitted (see
/// [`inline_link_level`]); only the attribute-list branch below, which parses a
/// real [`Attrlist`]`<'src>` out of the bracketed text's own source slice,
/// still requires that one sub-range to be verbatim. The bare-URL
/// trailing-punctuation strip keeps a narrower boundary of its own, at the one
/// place where the strip's own arithmetic would cut *inside* an escaped
/// special.
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
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Option<MacroMatch<'src>> {
    // `scheme_match` is always present (no branch can match without it); fall
    // through defensively if it is somehow absent.
    let scheme_m = n.scheme_match()?;
    let scheme = scheme_m.as_str();

    // The `<url>` form is a separate computation in the string replacer, taken
    // on exactly this condition — and, as there, taken before anything else,
    // including this branch's own escape check and gate.
    if n.is_angle() && n.attrlist().is_none() {
        return build_angle_link_node(n, &scheme_m, full, nodes, pieces, root, parser);
    }

    // An escaped scheme (`\https://…`) keeps the boundary prefix and drops the
    // single backslash, leaving the rest of the match literal — no link node.
    //
    // This runs **ahead of the gate**, mirroring `InlineLinkReplacer`'s own
    // check order (it inspects the scheme's leading backslash before reading
    // any capture) and closing the same latent gap the `footnoteref:`, menu,
    // cross-reference, and `link:`-macro increments closed for their families:
    // an escaped `\https://example.org/*bold*` whose match the gate rejects
    // must still drop its backslash. It needs no gate of its own — dropping the
    // backslash keeps the rest of the match as its **own original nodes**,
    // which fold back to exactly the bytes the replacer's own
    // `caps[0][prefix.len() + 1..]` emits.
    if scheme.starts_with('\\') {
        return Some(MacroMatch {
            kind: MacroMatchKind::Unescape {
                backslash: scheme_m.start(),
            },
            full: full.clone(),
        });
    }

    // The gate covers the bytes this pass *reads*, not the whole match. Every
    // value it computes lies before the bracketed display text: the boundary
    // prefix it inspects, the scheme, and the URL that becomes the target — out
    // of which a *bare* link's shown text is sliced too, which is why that form
    // keeps the whole match gated. The bracketed **display text** reads
    // nothing: it becomes structured children through `macro_text_children`,
    // whose `emit_range` path carries an opaque piece's own node, so it is
    // admitted — see [`inline_link_level`]'s own rendered-span note. Its
    // closing `]` needs no gate of its own: that byte is literal, and no atomic
    // piece — a placeholder, or an entity delimited by `&` and `;` — can supply
    // it. (A text carrying an attribute list keeps this same gate inside
    // `text_attrlist`, since its display text comes back from a *parse*
    // rather than from a range.)
    //
    // An expanded attribute value and an escaped special are admitted
    // throughout, since every value read here comes off the match string —
    // whose bytes are, for both, exactly the ones the string replacer's own
    // haystack carries there.
    let computed_end = n.attrlist().map_or(full.end, |m| m.start());

    if !range_has_no_opaque_piece(nodes, pieces, &(full.start..computed_end)) {
        return None;
    }

    let prefix = n.prefix_str();

    // The URL body is the formal-macro target group or, for a bare link, the
    // bare group; the two are mutually exclusive.
    let url_m = n.target().or_else(|| n.bare());
    let url_part = url_m.map_or("", |m| m.as_str());
    let mut target = format!("{scheme}{url_part}");

    // The node consumes the URL (and, for a formal macro, its `[…]` attrlist)
    // but not the boundary prefix; a bare URL additionally stops short of any
    // trailing punctuation the string step strips out.
    let mut consumed_end = full.end;

    // Where the URL itself ends in the match string — which is where a *bare*
    // link's own display text ends, since that text is the target (see the
    // children below). It coincides with `consumed_end` for a bare link and
    // stops at the target group for a formal one, whose `[…]` the node also
    // consumes but does not show.
    let mut url_end = url_m.map_or(full.end, |m| m.end());

    let mut link_text: Option<String> = None;

    let raw_text_m = n.attrlist();
    let raw_text = raw_text_m.map_or("", |m| m.as_str());

    if let Some(attrlist_m) = raw_text_m {
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
        url_end = consumed_end;

        // The strip's arithmetic runs over the *match string*, exactly as the
        // replacer's runs over its own escaped haystack — but the boundary it
        // lands on must still be one a node list can be cut at, and an escaped
        // special is one piece, not five bytes. A bare URL ending in a literal
        // `&` reaches this pass as `…&amp;`, whose own final `;` satisfies the
        // strip: the replacer happily splits the entity (target `…&amp`, suffix
        // `;`), while here `consumed_end` would land *inside* a
        // [`CharRef`](InlineNode::CharRef) leaf that
        // [`emit_range`](super::super::quotes::emit_range) can only emit whole.
        // Left literal, the one form this family's escaped-special lift does
        // not reach.
        if stripped > 0 && !range_is_verbatim_or_synthesized(pieces, &(consumed_end..full.end)) {
            return None;
        }
    }

    // The display text becomes the node's children, located at the bracketed
    // text (a formal link) or the node itself (a bare link).
    let text_location_range = raw_text_m.map(|m| m.start()..m.end());

    let mut window: Option<CowStr<'src>> = None;
    let mut bare = false;
    let mut attrs: Option<Attrlist<'src>> = None;

    // Set when the display text stops being one the children can *slice* out of
    // the bracketed text: an attribute list's positional value is computed by
    // `extract_attributes_from_text`.
    let mut computed_text = false;

    // Set when that computed value came back from a parse of the level's
    // **match string** rather than of the source's own bytes, so it carries
    // already-escaped text the children must be rebuilt from (see
    // [`text_attrlist`]).
    let mut escaped_computed_text = false;

    // Set when the `^` new-window suffix was trimmed off the display text, so
    // the children's range stops one (ASCII) byte short of the bracket's end.
    let mut caret_stripped = false;

    let link_text = if let Some(mut link_text) = link_text {
        link_text = link_text.replace("\\]", "]");

        // A text carrying an `=` splits into an attribute list, which
        // `InlineLinkReplacer` parses from a newline-normalized *copy* of the
        // text — the copy [`text_attrlist`] reproduces, from the source's own
        // bytes when they are that copy and from the level's match string
        // otherwise. Only a text crossing an **opaque** piece is deferred
        // there.
        if link_text.contains('=') {
            #[allow(clippy::unwrap_used)]
            let range = text_location_range.clone().unwrap();

            let parsed = text_attrlist(raw_text, range, nodes, pieces, root, parser)?;

            // Mirrors `InlineLinkReplacer`'s own guard: only adopt the parsed
            // result when a real named attribute actually split off from the
            // text (otherwise the `=` was incidental and `extract_attributes_
            // from_text` already returned the text unchanged with an empty
            // attrlist, matching this fallthrough).
            if parsed.adopted {
                link_text = parsed.text.replace("\\\"", "\"");
                attrs = Some(parsed.attrs);
                computed_text = true;
                escaped_computed_text = parsed.escaped;
            }
        }

        if link_text.ends_with('^') {
            link_text.truncate(link_text.len() - 1);
            window = Some(CowStr::from("_blank"));
            caret_stripped = true;
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

    let children = if bare {
        // A bare link's shown text *is* the URL — the whole target, or (under
        // `hide-uri-scheme`) its scheme-stripped tail, always a suffix since
        // [`URI_SNIFF`] is `^`-anchored — so it is a *slice* of the match's own
        // URL range rather than a value this builder computes, and takes the
        // same structured recovery a bare `link:`/`mailto:` macro's does (see
        // [`build_link_node`]): baking the already-escaped target into one
        // `Text` would have the fold escape it a second time (design §3.4).
        // There is no `\]` unescape here — this text comes from the URL groups,
        // whose own character classes never admit a bracket.
        let hidden_scheme = target.len() - link_text.len();

        macro_text_children(
            &link_text,
            (scheme_m.start() + hidden_scheme)..url_end,
            false,
            nodes,
            pieces,
            root,
        )
    } else if computed_text {
        // A value this builder *computed* rather than sliced: an attribute
        // list's positional value, so a *synthesized* value whose bytes need
        // not coincide with its source.
        if escaped_computed_text {
            // Parsed out of the level's match string, so it holds an escaped
            // special's canonical entity (and a restored entity's own bytes)
            // where a node holds logical text: rebuild design §3.4's
            // trichotomy from those bytes, exactly as the cross-reference
            // family's own attribute-list value is rebuilt.
            escaped_value_children(&link_text, text_location)
        } else {
            // Parsed out of the source's own bytes, which are already logical
            // text carrying no entity to undo: one synthesized `Text`.
            vec![InlineNode::Text {
                value: CowStr::from(link_text),
                location: text_location,
            }]
        }
    } else {
        // A text sliced straight out of the bracket. `macro_text_children`
        // borrows it from `'src` in the common verbatim case (§4.5), owns it
        // when it crosses an expanded attribute value, and rebuilds it as
        // structured children — each escaped special staying the `CharRef` it
        // already is, each **opaque** piece staying its own node — when it
        // crosses either, applying the same `\]` unescape
        // `InlineLinkReplacer` performs. The `^` window suffix is one ASCII
        // byte at the end of the bracketed text, so the range simply stops
        // short of it.
        #[allow(clippy::unwrap_used)]
        let m = raw_text_m.unwrap();

        let end = if caret_stripped { m.end() - 1 } else { m.end() };

        let trimmed = raw_text.get(..end - m.start()).unwrap_or(raw_text);

        macro_text_children(trimmed, m.start()..end, true, nodes, pieces, root)
    };

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
/// # The gate covers only the interior
///
/// This branch's `&lt;` prefix and `&gt;` terminator are themselves escaped
/// specials — [`atomic`](Piece::atomic) pieces — under every effective order
/// that escapes them, and the node consumes both without slicing either (the
/// replacer emits neither), so the gate covers only what lies *between* them:
/// the scheme and the URL. As in the general path an escaped special inside
/// that interior is admitted — the target is read off the match string, and the
/// display text derived from it is recovered as structured children — while an
/// **opaque** piece defers. This form has no *bracketed* display text to carry
/// structurally, so every byte the gate covers is one the target is computed
/// from and the gate stays whole, where its `[…]` sibling — handled by the
/// general path — admits an opaque piece inside its text. The two escape checks
/// above run ahead of the gate, for the same reason the general path's does.
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
    nodes: &[InlineNode<'src>],
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

    let interior = scheme_m.start()..angle_url.end();

    if !range_has_no_opaque_piece(nodes, pieces, &interior) {
        return None;
    }

    let target = format!(
        "{scheme}{url}",
        scheme = scheme_m.as_str(),
        url = angle_url.as_str()
    );

    let link_text = hide_uri_scheme_text(&target, parser);

    let location = source_slice(pieces, full.clone(), root);

    // The shown text is the target, so — as in the general path's bare case —
    // it is recovered from the interior's own range rather than baked, already
    // escaped, into one `Text` the fold would escape a second time.
    let hidden_scheme = target.len() - link_text.len();

    let children = macro_text_children(
        &link_text,
        (interior.start + hidden_scheme)..interior.end,
        false,
        nodes,
        pieces,
        root,
    );

    let node = InlineNode::Ref(Ref {
        variant: RefVariant::Link,
        target: CowStr::from(target),
        children,
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
/// This increment covers the **explicit `link:`/`mailto:` macro**:
/// `link:target[text]`, `link:target[]`
/// (a bare link), `mailto:addr[text]`, and `mailto:addr[]`, plus the `^`
/// new-window suffix, the `\` escape, and a display text carrying its own
/// attribute list — a `,` in a `mailto:` text (its `subject`/`body`) or an `=`
/// in a `link:` text (roles / id / title / window), parsed by
/// [`text_attrlist`]. It deliberately leaves several forms
/// **unrecognized** for a later increment, each left as literal source here (so
/// the differential corpus only pins the forms this increment claims):
///
/// - **Auto-links and formal-URL links** (`https://example.org`, `https://example.org[text]`)
///   are matched by a *different* pattern (`INLINE_LINK`, with its bare-URL
///   trailing-punctuation handling) and are a separate later increment.
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
/// admitted too, in an attribute-list text as much as anywhere else
/// (`link:a&b.html[]`,
/// `link:index.html[a < b]`, `link:index.html[a < b,role=hl]`). The match
/// string carries such a leaf's canonical
/// entity — the very bytes the string replacer's own escaped haystack holds
/// there — so the target this pass computes off that string (`a&amp;b.html`)
/// *is* the one the replacer computed, and no value on the node needs the
/// source's own `<`/`>`/`&`. The display text then becomes **structured
/// children** rather than one baked `Text` (see [`macro_text_children`]), so
/// the special folds back to its own entity instead of being escaped twice —
/// for a bare macro too, whose shown text is a slice of the target group rather
/// than a computed string (see [`build_link_node`]).
///
/// # A rendered span inside the display text
///
/// A **rendered span** — a [`Styled`](crate::inlines::Styled) span, an
/// already-recognized macro node of another family, a masked passthrough — is
/// *not* recoverable: [`build_match_string`] stands it in as one
/// [`SPAN_PLACEHOLDER`] where the string pipeline's haystack holds its markup
/// (or its own passthrough mask) inline, and that markup exists only at fold
/// time. It is nonetheless admitted **inside the bracketed display text**,
/// because that text is the one capture this family never reads as bytes: it
/// becomes the node's children through [`macro_text_children`], whose
/// [`emit_range`](super::super::quotes::emit_range) path clones the opaque
/// piece's own node whole into them — so the text is carried *structurally*,
/// and the fold re-renders exactly the markup the string replacer captured
/// there. The **target** stays gated, since it is a value this pass computes
/// off the match string. So does an **attribute-list text**, for the same
/// reason: its display text comes back from an [`Attrlist`] parse rather than
/// from a range, and a placeholder inside a *parsed* value has no node it can
/// be mapped back to (see [`text_attrlist`]).
/// This is the second family to take that lift, after the cross-reference one
/// (see `xref::find_xref_matches`).
///
/// What the admission cannot do is make the *recognition* agree in every case,
/// because the string replacer matches over the markup itself where this
/// matches over one placeholder standing in for it — and this pass cannot know
/// what that markup carries without folding, which building a tree must never
/// do. The two read the same extent unless the markup carries a character the
/// pattern (or the replacer's own probe) is sensitive to, which leaves three
/// documented divergences of *extent*, each pinned by its own test and each one
/// where the string pipeline's reading is the markup-perturbed one and the
/// tree's the well-formed one — exactly as the quotes step's own
/// crossed-delimiter divergence is:
///
/// - a `]` inside the span (`link:x[a *b ] c* d]`), which ends
///   [`INLINE_LINK_MACRO`]'s own lazy text capture early for the string
///   replacer but not here;
/// - markup carrying an `=` beside a comma elsewhere in a `link:` text
///   (`link:x[one, [.hl]#two#]`): the replacer's attribute-list probe fires on
///   the markup's own `=`, and the parse then keeps only what precedes that
///   comma;
/// - a comma inside the span of a `mailto:` text (`mailto:a@x.org[a *b, c*
///   d]`), which is that spelling's own attribute-list probe, read the same
///   way.
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
    ctx: LevelContext,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes);

    // Cheap pre-filter mirroring the string step's guard: a link/mailto macro
    // needs its prefix and an opening bracket.
    if !((s.contains("link:") || s.contains("mailto:")) && s.contains('[')) {
        return nodes;
    }

    // Matched over the level wrapped in the boundary character its enclosing
    // construct presents, with the level's own pieces moved into that string's
    // coordinates — see `apply_macro_families`'s own doc comment.
    let (s, pieces) = ctx.shift(s, pieces);

    let matches = find_link_macro_matches(&nodes, &s, &pieces, root, parser);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every recognized `link:`/`mailto:` macro at this level, skipping any
/// match whose **target** crosses an **opaque** piece (see
/// [`range_has_no_opaque_piece`]), whose own marker is not verbatim source, or
/// that [`build_link_node`] defers (see [`link_macro_level`]).
///
/// The gate covers the bytes the node *reads*, not the whole match: the target
/// is computed off the level's match string, while the bracketed display text
/// becomes structured children and so needs no recoverable bytes at all — see
/// [`link_macro_level`]'s own rendered-span section.
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

        // The gate covers the bytes the node *reads*, not the whole match. The
        // one value this family computes off the match string is its **target**
        // (group 3; group 2 is the empty-target alternative, which has no bytes
        // to gate), so only that range needs a match string whose bytes are the
        // string replacer's own. The bracketed **display text** reads nothing:
        // it becomes structured children through `macro_text_children`, whose
        // `emit_range` path carries an opaque piece's own node, so it is
        // admitted — see [`link_macro_level`]'s own rendered-span note. The
        // `link:`/`mailto:` marker and the brackets need no gate of their own:
        // those bytes are literal, and no atomic piece — a placeholder, or an
        // entity delimited by `&` and `;` — can supply them. (The marker keeps
        // its own stricter, verbatim gate below, for `link_form`'s sake; a text
        // carrying an attribute list keeps this same gate inside
        // `text_attrlist`, since its display text comes back from a *parse*.)
        if let Some(target) = caps.get(3)
            && !range_has_no_opaque_piece(nodes, pieces, &(target.start()..target.end()))
        {
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
/// that carries an attribute list *and* crosses an opaque piece (see
/// [`text_attrlist`]).
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
///   special, an **opaque** piece's own node cloned whole) when it crosses
///   either;
/// - a **bare macro's** text is the target itself, which is likewise a slice —
///   the whole target group, or (under `hide-uri-scheme`) its scheme-stripped
///   tail, always a *suffix* since [`URI_SNIFF`] is `^`-anchored — so it takes
///   the same treatment rather than baking the already-escaped target into one
///   `Text` the fold would escape a second time (design §3.4);
/// - an **attribute list's positional value** is the one text this builder
///   *computes*, out of an [`Attrlist`] parse (see [`text_attrlist`]). A parse
///   of the bracket's own verbatim `'src` slice yields logical text, so it
///   stays a single synthesized `Text` (that slice carries no entity to undo);
///   a parse of the level's **match string** yields already-escaped text
///   instead, so it is rebuilt through [`escaped_value_children`] — design
///   §3.4's trichotomy — exactly as the cross-reference family's own
///   attribute-list value is.
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

    // Set when that computed value came back from a parse of the level's
    // **match string** rather than of the source's own bytes, so it carries
    // already-escaped text the children must be rebuilt from (see
    // [`text_attrlist`]).
    let mut escaped_computed_text = false;

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

                let parsed =
                    text_attrlist(raw_text, m.start()..m.end(), nodes, pieces, root, parser)?;

                link_text = parsed.text;
                computed_text = true;
                escaped_computed_text = parsed.escaped;

                if let Some(target_attr) = parsed.attrs.nth_attribute(2) {
                    target = format!(
                        "{target}?subject={subject}",
                        subject = encode_uri_component(target_attr.value())
                    );

                    if let Some(body) = parsed.attrs.nth_attribute(3) {
                        target = format!(
                            "{target}&amp;body={body}",
                            body = encode_uri_component(body.value())
                        );
                    }
                }

                attrs = Some(parsed.attrs);
            }
        } else if link_text.contains('=') {
            #[allow(clippy::unwrap_used)]
            let m = raw_text_m.unwrap();

            let parsed = text_attrlist(raw_text, m.start()..m.end(), nodes, pieces, root, parser)?;

            link_text = parsed.text;
            computed_text = true;
            escaped_computed_text = parsed.escaped;
            attrs = Some(parsed.attrs);
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
        // list's positional value, so a *synthesized* value whose bytes need
        // not coincide with its source.
        if escaped_computed_text {
            // Parsed out of the level's match string, so it holds an escaped
            // special's canonical entity (and a restored entity's own bytes)
            // where a node holds logical text: rebuild design §3.4's
            // trichotomy from those bytes, exactly as the cross-reference
            // family's own attribute-list value is rebuilt.
            escaped_value_children(&link_text, text_location)
        } else {
            // Parsed out of the source's own bytes, which are already logical
            // text carrying no entity to undo: one synthesized `Text`.
            vec![InlineNode::Text {
                value: CowStr::from(link_text),
                location: text_location,
            }]
        }
    } else {
        // A text sliced straight out of the bracket. `macro_text_children`
        // borrows it from `'src` in the common verbatim case (§4.5), owns it
        // when it crosses an expanded attribute value, and rebuilds it as
        // structured children — each escaped special staying the `CharRef` it
        // already is, each **opaque** piece staying its own node — when it
        // crosses either, applying the same `\]` unescape
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

/// One link display text read as an attribute list — the result
/// [`text_attrlist`] returns.
struct TextAttrlist<'src> {
    /// The list's first positional value: the display text the node shows.
    text: String,

    /// The list itself, which rides on the node's own
    /// [`attrs`](Ref::attrs) so the fold can hand `render_link` the same
    /// `id`/`title`/`nofollow`/`noopener` the string replacer hands it.
    attrs: Attrlist<'src>,

    /// Whether [`text`](Self::text) came back from a parse of the level's
    /// **match string** (already-escaped bytes) rather than of the source's own
    /// (logical text). The caller rebuilds the former through
    /// [`escaped_value_children`] so an entity in it is not escaped twice.
    escaped: bool,

    /// Whether a real named attribute actually split off — the string
    /// replacers' own `lt != link_text_for_attrlist` guard, which
    /// [`InlineLinkReplacer`](crate::content::macros) applies and
    /// `InlineLinkMacroReplacer` does not (see this module's two call sites).
    /// `false` means the `=` was incidental and the whole text is the sole
    /// positional value.
    adopted: bool,
}

/// Parses a link's bracketed **display text** as the [`Attrlist`]`<'src>` its
/// node carries, mirroring what the string replacers parse: a
/// newline-normalized copy of the (pre-`\]`-unescape) bracketed text, joined
/// with spaces so `link:x[Foo\nBar,role=hl]` reads as `Foo Bar`.
///
/// Which bytes that copy is taken from is the whole question. An
/// [`Attrlist`]`<'src>` reads its own `Span<'src>`'s bytes **as content**, not
/// merely as a location tag, so this used to require the text to be a
/// contiguous, single-line `'src` slice and deferred every other shape. It no
/// longer does, by the same move the image family's own bracket made
/// ([`bracket_attrlist`](super::image)): the list is parsed from the level's
/// **match string** — which is exactly what the replacer parses, out of its own
/// escaped, already-expanded haystack — and
/// [`into_owned`](Attrlist::into_owned)ed onto the text's coarse source span
/// (design §4.4, the fallback the node's `location` already takes). So a text
/// crossing an escaped special (`link:index.html[a < b,role=hl]`), a restored
/// entity (`mailto:a@b.com[Tom &copy; Jerry,Subject]`), an expanded attribute
/// value (`link:index.html[{label},role=hl]`), or a line break
/// (`link:index.html[Docs\nmore,role=hl]`) is now recognized.
///
/// A **verbatim** text whose source slice *is* those same bytes keeps the
/// `'src` parse, and with it the borrow (§4.5) — the shape every ordinary
/// `link:index.html[Docs,role=hl]` takes. Both halves of that test are load
/// bearing. The range must be verbatim because bytes can coincide without the
/// text being the source's: [`build_match_string`] gives a *restored* entity
/// leaf its own bytes as written, so `link:x[a &copy; b,role=hl]` reads
/// identically either way while its parsed value is escaped text, not logical
/// text. And the bytes must be compared because a verbatim range need not be
/// contiguous in the source: the attribute-references step drops an escaped
/// reference's backslash as a *gap* (`link:x[\{name},role=hl]`), so the
/// enclosing slice carries a byte the replacer's own text does not.
///
/// Returns `None` — the one shape still deferred — when the text crosses an
/// **opaque** piece (a rendered span, an earlier-recognized macro node, a
/// masked passthrough). That is not a bytes problem the match string can
/// solve: [`build_match_string`] stands such a piece in as one
/// [`SPAN_PLACEHOLDER`] where the string replacer's haystack holds its markup,
/// and a placeholder inside a *parsed* value has no node it can be mapped back
/// to (see [`link_macro_level`]'s own rendered-span note).
fn text_attrlist<'src>(
    raw_text: &str,
    text_range: std::ops::Range<usize>,
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Option<TextAttrlist<'src>> {
    if !range_has_no_opaque_piece(nodes, pieces, &text_range) {
        return None;
    }

    let normalized = raw_text.replace('\n', " ");
    let verbatim_range = text_range.clone();
    let source = source_slice(pieces, text_range, root);

    if range_is_verbatim(pieces, &verbatim_range) && source.data() == normalized {
        let (text, attrs) = extract_attributes_from_text(source, parser, None);

        return Some(TextAttrlist {
            adopted: text != normalized,
            text,
            attrs,
            escaped: false,
        });
    }

    let (text, attrs) = extract_attributes_from_text(Span::new(&normalized), parser, None);

    Some(TextAttrlist {
        adopted: text != normalized,
        text,
        attrs: attrs.into_owned(source),
        escaped: true,
    })
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
/// One form is left **unrecognized** for a later increment, documented and
/// pinned by its own divergence test:
///
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
///
/// An **escaped special** ([`CharRef`](InlineNode::CharRef)`::Special`) is
/// admitted too (`a&b@example.org`, whose literal `&` the pattern's own
/// local-part class matches as `&amp;`) — the fourth family to lift that
/// boundary, after the cross-reference, `link:`/`mailto:` macro, and
/// auto-link/formal-URL families, and the last of the link family's own
/// spellings to lift it. The match string carries such a leaf's canonical
/// entity — the very bytes the string replacer's own escaped haystack holds
/// there — so the target this pass computes off that string
/// (`mailto:a&amp;b@example.org`) *is* the one the replacer computed and
/// registered, and no value on the node needs the source's own `&`. The
/// display text, which is the address itself, then becomes **structured
/// children** rather than one baked `Text` (see [`macro_text_children`]), so
/// the special folds back to its own entity instead of being escaped twice.
///
/// Unlike its three predecessors, this family needs no
/// [`range_has_no_opaque_piece`] gate to express that lift: an address
/// **cannot** cross an opaque piece in the first place. Every such piece is
/// exactly one [`SPAN_PLACEHOLDER`] (U+E0F0, Unicode category `Co`), which
/// none of the pattern's character classes admit — not the local part's
/// `[\w_]` / `[\w\-.%+]`, not the domain's `[\p{L}\p{Nd}_\-.]`, not the TLD's
/// `[a-zA-Z]` — so a match can never contain one. Nor can a match *begin* or
/// *end* strictly inside an escaped special's entity: an entity's leading `&`
/// is in no class the domain or TLD accepts, and its trailing `;` is neither
/// a local-part character nor the `@` a local part must be followed by. So
/// the only atomic piece an address range can overlap is a **wholly
/// contained** `&amp;`, which is precisely the one this lift admits — the
/// same structural argument the sibling auto-link family already makes for
/// its own required boundary-prefix group ("a placeholder simply fails its
/// match"). A gate here would be a branch no input can reach.
pub(super) fn email_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    ctx: LevelContext,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes);

    // Cheap pre-filter mirroring the string step's own `text.contains('@')`
    // guard.
    if !s.contains('@') {
        return nodes;
    }

    // Matched over the level wrapped in the boundary character its enclosing
    // construct presents, with the level's own pieces moved into that string's
    // coordinates — see `apply_macro_families`'s own doc comment.
    let (s, pieces) = ctx.shift(s, pieces);

    let matches = find_email_matches(&nodes, &s, &pieces, root);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every recognized bare e-mail address at this level, honoring the
/// pattern's own mismatch-prefix group and skipping an address that **abuts**
/// an opaque piece (one it *crosses* being structurally impossible — see
/// [`email_level`]).
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

        let node = build_email_node(&caps, pieces, root, nodes);

        matches.push(MacroMatch {
            kind: MacroMatchKind::Node {
                consumed: full.clone(),
                node: Box::new(node),
            },
            full,
        });
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
/// The target is read straight off the level's **match string**, exactly as
/// `InlineEmailReplacer` reads `caps[2]` off its own escaped haystack — the
/// same bytes, whether the address is verbatim `'src`, comes from a
/// [`synthesized`](Piece::synthesized) run (an attribute expansion), or
/// carries an escaped special (`a&amp;b@example.org`). An e-mail node holds no
/// `Span`-typed field (no [`Attrlist`] parsed out of `'src`), only plain text,
/// so only the node's `location` needs design §4.4's coarse fallback.
///
/// The display text is that same address, so it is a *slice* of the match's own
/// range rather than a value this builder computes: [`macro_text_children`]
/// recovers it, borrowing from `'src` in the common case (§4.5) and rebuilding
/// it as structured children — each escaped special staying the
/// [`CharRef`](InlineNode::CharRef) it already is — when it crosses one, rather
/// than baking the already-escaped address into one `Text` node the fold would
/// escape a second time (design §3.4). There is no `\]` bracket unescape: this
/// text comes from the address, which the pattern's own character classes never
/// let a bracket into.
///
/// This is **total**: what the family defers is decided entirely by the gate
/// [`find_email_matches`] applies before calling it.
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
) -> InlineNode<'src> {
    // Group 2 is the address itself. The `unwrap` is safe: the group is the
    // pattern's one mandatory capture, so it participates in every match (the
    // same reason the overall-match `unwrap` above is safe).
    #[allow(clippy::unwrap_used)]
    let address_m = caps.get(2).unwrap();

    let address = address_m.as_str();
    let range = address_m.start()..address_m.end();
    let location = source_slice(pieces, range.clone(), root);

    InlineNode::Ref(Ref {
        variant: RefVariant::Link,
        target: CowStr::from(format!("mailto:{address}")),
        children: macro_text_children(address, range, false, nodes, pieces, root),
        roles: vec![],
        window: None,
        attrs: None,
        resolved: None,
        derived: None,
        xrefstyle: None,
        location,
    })
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
        assert_entity, assert_link, assert_special_char, assert_styled, assert_text, build_src,
        fold_html, golden_macros, golden_macros_with, link_text_of,
    };
    use crate::{
        HasSpan, Parser, Span,
        content::inline_builder::build,
        inlines::{CharRef, InlineNode, SpanForm, StyleVariant},
        parser::HtmlSubstitutionRenderer,
        strings::CowStr,
    };

    /// A parser with the `experimental` attribute set, so a UI macro inside a
    /// link's display text is recognized (the string step gates the UI family
    /// on it, and the builder mirrors that gate).
    fn experimental_parser() -> Parser {
        use crate::parser::ModificationContext;

        Parser::default().with_intrinsic_attribute_bool(
            "experimental",
            true,
            ModificationContext::Anywhere,
        )
    }

    #[test]
    fn fold_matches_the_string_pipeline_through_link_macros() {
        // For each fixture, folding the single-pass tree (all five steps)
        // reproduces the string pipeline's output byte-for-byte. This is the
        // differential corpus (design §5.3) that pins the `link:`/`mailto:`
        // macro increment. What is still deferred — a URL target (the
        // `INLINE_LINK` pass's own territory), a display text crossing a
        // rendered span, and a *wholly* expanded macro — lives in a divergence
        // test below.
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
            // An **attribute-list-bearing** text with no `'src` slice of its
            // own: one crossing an escaped special, one crossing a restored
            // entity, and one spanning two lines (which the attrlist parse
            // joins with a space). Each is parsed from the level's match
            // string — the same bytes the string replacer parses — and owned
            // off it, in both the `link:` (`=`) and `mailto:` (`,`) spellings.
            "link:index.html[a < b,role=hl]",
            "link:index.html[Tom & Jerry,role=hl^]",
            "mailto:hello@example.org[Sub & ject,Hello there]",
            "link:index.html[a &copy; b,role=hl]",
            "mailto:a@b.com[Tom &copy; Jerry,Subject here]",
            "link:index.html[Docs\nmore,role=hl]",
            "mailto:team@example.org[Team,Hello\nthere]",
            // An incidental `=` in a text with no `'src` slice: the parse
            // finds no named attribute, so the whole text stays the display
            // text — the `InlineLinkMacroReplacer` path adopts it either way.
            "link:index.html[=a < b]",
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

        let location = assert_special_char(&reference.children[1], '&');
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

        let location = assert_special_char(&reference.children[1], '<');
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
        // link increment. A fixture may cross an escaped special anywhere but
        // its own attribute-list text; the forms still deferred (an
        // attribute-list text crossing a special, a multi-line attribute-list
        // text, a display text crossing a rendered span, and a bare URL whose
        // trailing-punctuation strip would split a special) each live in a
        // divergence test below. The pattern's ANGLE branch has its own corpus,
        // alongside its own structural tests.
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
            // An **attribute-list-bearing** text with no `'src` slice of its
            // own: one crossing an escaped special and one spanning two lines
            // (which the attrlist parse joins with a space). Each is parsed
            // from the level's match string — the same bytes the string
            // replacer parses — and owned off it. The incidental-`=` fallback
            // reaches the same path.
            "https://example.org[a < b,role=hl]",
            "https://example.org[Tom & Jerry,role=hl^]",
            "https://example.org[Example\nmore,role=hl]",
            "https://example.org[=a < b]",
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
            // A URL crossing an *escaped special*: the target this pass reads
            // off the level's match string is the escaped one the string
            // replacer computed, and a bare link's shown text is recovered from
            // that same range as structured children rather than baked.
            "https://example.org/?a=1&b=2",
            "See https://example.org/?a=1&b=2, then stop.",
            "(https://example.org/?a=1&b=2)",
            "ftp://example.org/a&b",
            "https://example.org/?a=1&b=2 https://other.example/?c=3&d=4",
            "*https://example.org/a&b*",
            // A formal link whose *target* crosses one, with every shape of
            // display text: labeled, bare, escaped `]`, `^` suffix, and an
            // attribute list (whose own bracketed text is verbatim here).
            "https://example.org/a&b[Text]",
            "https://example.org/a&b[]",
            "https://example.org/a&b[Text^]",
            "https://example.org/a&b[a\\]b]",
            "https://example.org/a&b[Text,role=hl]",
            // A formal link whose *display text* crosses one, alone and
            // together with the target.
            "https://example.org[a < b]",
            "https://example.org[a < b^]",
            "https://example.org/a&b[a<b]",
            // A `;` that really is the URL's own last character still strips
            // (the entity beside it is left whole).
            "https://example.org/a&;",
            // The escape still drops its backslash over a special.
            "\\https://example.org/?a=1&b=2",
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
            // The scheme strip is a byte count into the URL's own range, so it
            // composes with a target crossing an escaped special (the shown
            // text starts past the scheme and keeps the `CharRef` child).
            "https://example.org/a&b",
            "https://example.org/a&b[]",
            "<https://example.org/a&b>",
            "https://example.org/?a=1&b=2",
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
    fn an_auto_link_at_a_spans_own_edge_reads_that_spans_boundary_characters() {
        // `INLINE_LINK`'s non-angle branch requires a boundary prefix
        // (`( ^ | [\ \t\p{Zs}] | [>\(\)\[\];"'] )`), which inside a span the
        // string pipeline reads out of that span's own rendered markup. Every
        // shape a [`LevelContext`](super::super::quotes::LevelContext) can
        // present — the `>` ending a tag and the `;` ending a smart quote's
        // entity — is in that class, so the URL is recognized on both sides of
        // the seam; this pins that the context the macros step now takes
        // leaves that agreement intact rather than turning a start anchor into
        // a prefix the class rejects.
        for source in [
            "*https://example.org*",
            "_https://example.org_",
            "`https://example.org`",
            "[.hl]#https://example.org#",
            "*https://example.org[Docs]*",
            r#"He said "`https://example.org[Docs]`" ok."#,
            r#"He said '`https://example.org[Docs]`' ok."#,
            // Away from either edge, and with the boundary prefix the level
            // itself supplies.
            "*see https://example.org now*",
            r#""`see https://example.org now`""#,
            // An escaped scheme at a span's own edge still drops its backslash.
            "*\\https://example.org*",
            // The `link:`/`mailto:` macro and the angle-bracketed form read no
            // boundary of their own, so they are unaffected either way.
            "*link:index.html[Docs]*",
            "*mailto:doc@example.org[Write]*",
            "*<https://example.org>*",
            // And the same at the content's own top level.
            "https://example.org",
            "see https://example.org now",
        ] {
            assert_eq!(
                fold_html(&build_src(Span::new(source)), &HtmlSubstitutionRenderer {}),
                golden_macros(source),
                "fold diverged from the string pipeline for {source:?}"
            );
        }
    }

    #[test]
    fn a_bare_url_at_an_entity_rendered_spans_closing_edge_is_a_documented_divergence() {
        // The closing half of the same seam, which no single character can
        // answer — and which the macros step therefore does not take (see
        // [`LevelContext::shift`](super::super::quotes::LevelContext)).
        // A boundary class reads exactly one character, but a bare URL's own
        // body class (`[^\s\[\]<]*`) consumes greedily: it excludes a `<`, so
        // a tag-rendered span's closing markup stops it in both pipelines, and
        // it admits an `&`, so at a smart quote's closing `&#8220;…&#8221;` the
        // string pipeline swallows the whole entity into the target and leaves
        // a stray `;` behind. Supplying the level one `&` would build a third,
        // differently wrong target, so the closing character is dropped rather
        // than half-supplied and the tree keeps the well-formed reading — the
        // same shape as the quotes step's own crossed-delimiter divergence.
        let source = r#"He said "`https://example.org`" ok."#;
        let folded = fold_html(&build_src(Span::new(source)), &HtmlSubstitutionRenderer {});

        assert_ne!(
            golden_macros(source),
            folded,
            "expected the documented divergence to still reproduce"
        );

        // The string pipeline's own reading is the markup-perturbed one.
        assert!(golden_macros(source).contains("https://example.org&#8221"));
        assert!(folded.contains(r#"href="https://example.org""#));
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
            // An escaped special *inside* the delimiters, in both spellings of
            // the branch: the interior gate admits it, and the target-derived
            // display text is recovered as structured children.
            "<https://example.org/?a=1&b=2>",
            "see <https://example.org/x&y> and https://z.example/p&q[Q].",
            "<https://example.org/a&b[text]",
            "<https://example.org/a&b[a<b]",
            "<\\https://example.org/a&b>",
            "\\<https://example.org/a&b>",
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
    fn an_angle_bracketed_url_crossing_an_escaped_special_shows_structured_children() {
        // The delimiters of an angle link are escaped specials the node
        // consumes; so, now, may the URL *between* them be crossed by one. The
        // target is read off the level's match string (the escaped bytes the
        // string replacer computed), and the shown text — which for this
        // always-`bare` form *is* the target — is recovered from the interior's
        // own range as structured children, so the `&` stays the `CharRef` it
        // already is rather than being escaped a second time.
        let source = "<https://example.org/?a=1&b=2>";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        let reference = assert_link(&nodes[0]);

        assert_eq!(
            reference.target.as_ref(),
            "https://example.org/?a=1&amp;b=2"
        );
        assert_eq!(reference.roles, [CowStr::from("bare")]);

        // The node still consumes both delimiters.
        assert_eq!(reference.location.data(), source);

        assert_eq!(reference.children.len(), 3);
        assert_text(&reference.children[0], "https://example.org/?a=1", 1, 2);

        let location = assert_special_char(&reference.children[1], '&');
        assert_eq!(location.data(), "&");
        assert_eq!(location.byte_offset(), 25);

        assert_text(&reference.children[2], "b=2", 1, 27);

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn an_angle_bracketed_url_over_a_rendered_span_is_a_documented_divergence() {
        // The interior gate admits an escaped special but still rejects an
        // *opaque* piece: a quoted span inside the delimiters is a `Styled`
        // node by macro time, standing in as one placeholder where the string
        // pipeline's haystack holds its markup, so the angle link is left
        // literal — the `<url>` form's own half of the boundary its `[…]`
        // sibling keeps below.
        let source = "<https://example.org/*bold*>";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an angle URL crossing a rendered span must stay literal: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* build a link here.
        assert!(golden_macros(source).contains("<a href"));
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_a_formal_url_display_text_crossing_a_rendered_span() {
        // The differential corpus for this increment: a formal URL link whose
        // bracketed display text crosses an **opaque** piece — a rendered span,
        // an already-recognized macro node of another family, a masked
        // passthrough. The text is carried structurally (each opaque piece's
        // own node becomes a child), so the fold re-renders exactly the markup
        // the string replacer captured in its own text.
        let fixtures = [
            // (A masked passthrough is opaque here too — and in the string
            // pipeline, which restores passthroughs only after every step — but
            // this oracle runs the steps directly, without the extraction the
            // real `SubstitutionGroup::apply` performs around them, so those
            // fixtures live in the whole-pipeline sweep instead; see
            // `inline_builder::tests`.)
            //
            // A span at the end of the text, at the start, in the middle, and
            // spanning the whole of it.
            "https://example.org[with *bold* text]",
            "https://example.org[*bold* leads]",
            "https://example.org[a *b* c]",
            "https://example.org[*bold*]",
            // Every quoted form the earlier step can have produced, including
            // an attributed span (whose markup carries an `=` the string
            // replacer's own attribute-list probe reads, and this one does not:
            // with no comma to split on, the parse yields one positional value
            // equal to the whole text, so both take the plain-text path).
            "https://example.org[_em_ and `code` and #mark#]",
            "https://example.org[super^script^ and sub~script~]",
            "https://example.org[[.hl]#roled#]",
            // An already-recognized macro node of another family — an image, an
            // icon, an index term — which is opaque here for the same reason
            // (`build_match_string` stands each in as one placeholder), plus a
            // character replacement, which is *not* opaque (it carries its own
            // bytes) but is exercised beside a span all the same.
            "https://example.org[the image:logo.png[Logo] here]",
            "https://example.org[the icon:tags[] here]",
            "https://example.org[a ((index term)) *b*]",
            "https://example.org[Acme(C) *now*]",
            // A span *and* an escaped special / restored entity in one text —
            // the recoverable and structural recoveries side by side — and a
            // span beside the target's own escaped special.
            "https://example.org[a < b and *bold*]",
            "https://example.org[a &copy; b and _em_]",
            "https://example.org/a&b[*bold* text]",
            // The `\]` unescape and the `^` window suffix still apply around a
            // span, as does a text that is only whitespace around one.
            "https://example.org[a\\] *b* c]",
            "https://example.org[*Open*^]",
            "https://example.org[ *b* ]",
            // The ANGLE branch's `[…]` alternative, which keeps its `&lt;` and
            // takes this same general path.
            "<https://example.org[*bold*]",
            "<https://example.org[a *b* c]",
            // Escaped: the backslash is dropped and the span stays in the flow.
            "\\https://example.org[*bold*]",
            // In surrounding flow, beside a sibling link of each spelling, and
            // inside a rendered span of its own.
            "See https://example.org[the *bold* one] for details.",
            "See https://x.org[*a*] and https://y.org[_b_].",
            "See https://x.org[*a*] and link:y.html[_b_].",
            "_a https://x.org[*b*] c_",
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

        // The UI family is recognized only under `experimental`, so its own
        // fixtures take a parser that sets it — on both sides of the
        // comparison.
        let parser = experimental_parser();

        for fixture in [
            // The UI pass runs *before* this one, so the macro's own `]` is
            // already inside the placeholder by the time the link's lazy text
            // capture runs — exactly as it is inside `<kbd>…</kbd>` on the
            // string side.
            "https://example.org[a kbd:[Ctrl+T] b]",
            "https://example.org[a btn:[Save] *b*]",
            "https://example.org[a menu:File[Save] b]",
        ] {
            let folded = fold_html(&build(Span::new(fixture), &parser, None), &renderer);

            assert_eq!(
                folded,
                golden_macros_with(fixture, &parser),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn a_formal_url_display_text_carries_a_rendered_span_as_its_own_child() {
        // A display text crossing a rendered span is carried *structurally*:
        // the span is one opaque placeholder in the match string, but
        // `macro_text_children` recovers the text with `emit_range`, which
        // clones the span's own node whole into the link's children. The fold
        // then re-renders exactly the markup the string replacer captured in
        // its own display text.
        let source = "https://example.org[a *bold* b]";
        let nodes = build_src(Span::new(source));

        let reference = assert_link(&nodes[0]);

        // Three children: the text before the span, the span itself, the text
        // after it — each borrowing its own precise `'src` slice, which the
        // one-`Text`-child shape this replaced could not express.
        assert_eq!(reference.children.len(), 3);
        assert_text(&reference.children[0], "a ", 1, 21);

        let styled = assert_styled(
            &reference.children[1],
            StyleVariant::Strong,
            SpanForm::Constrained,
        );

        assert_text(&styled[0], "bold", 1, 24);
        assert_text(&reference.children[2], " b", 1, 29);

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn a_formal_url_target_over_a_rendered_span_is_a_documented_divergence() {
        // The **target** is a value this pass computes off the match string,
        // where a rendered span is one opaque placeholder standing in for
        // markup that exists only at fold time — so the target keeps
        // `range_has_no_opaque_piece` while the display text lifts it.
        //
        // If this boundary is ever lifted, fold these fixtures into the parity
        // corpus above.
        for source in [
            "https://example.org/a``b``c[x]",
            "<https://example.org/a``b``c[x]",
        ] {
            let nodes = build_src(Span::new(source));

            assert!(
                nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
                "a target crossing a rendered span must stay literal: {nodes:?}"
            );

            // The string pipeline, by contrast, *does* build a link here.
            assert!(golden_macros(source).contains("<a href"));
        }
    }

    #[test]
    fn a_formal_url_attribute_list_text_over_a_rendered_span_is_a_documented_divergence() {
        // A text carrying an attribute list is parsed as a real
        // `Attrlist<'src>` from the source's own bytes, and a placeholder
        // inside a *parsed* value has no node it can be mapped back to — the
        // same reason the cross-reference and `link:`/`mailto:` macro families
        // defer their own `Attrlist`-bearing capture. That one text shape keeps
        // the stricter gate outright.
        //
        // If this boundary is ever lifted, fold these fixtures into the parity
        // corpus above.
        for source in [
            "https://example.org[a *b* c,role=hl]",
            "https://example.org[a *b* c,window=_blank]",
        ] {
            let nodes = build_src(Span::new(source));

            assert!(
                nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
                "an attribute-list text crossing a rendered span must stay literal: {nodes:?}"
            );

            // The string pipeline, by contrast, *does* build a link here.
            assert!(golden_macros(source).contains("<a href"));
        }
    }

    #[test]
    fn a_span_whose_markup_perturbs_the_inline_link_pattern_is_a_documented_divergence() {
        // What the structural recovery cannot do is make the *recognition*
        // agree in every case: the string replacer matches over the span's
        // markup where this matches over the one placeholder standing in for
        // it, so the two read the same extent only while that markup carries
        // no character the pattern (or the replacer's own attribute-list
        // probe) is sensitive to. These are the two shapes where it does — and
        // in each the string pipeline's reading is the markup-perturbed one (a
        // truncated text, a text the attribute-list parse cut in half) and the
        // tree's the well-formed one, exactly as the quotes step's own
        // crossed-delimiter divergence is.
        for source in [
            // A `]` inside the span ends `INLINE_LINK`'s own lazy text capture
            // early for the string replacer, but not here.
            "https://example.org[a *b ] c* d]",
            // Markup carrying an `=` (an attributed span) *and* a comma
            // elsewhere in the text: the string replacer's attribute-list probe
            // fires on the markup's own `=`, and the parse then splits the text
            // at that comma, keeping only what precedes it.
            "https://example.org[one, [.hl]#two#]",
        ] {
            let nodes = build_src(Span::new(source));

            assert_ne!(
                fold_html(&nodes, &HtmlSubstitutionRenderer {}),
                golden_macros(source),
                "{source:?} now agrees with the string pipeline; fold it into the parity corpus"
            );

            // The tree's own reading is the well-formed one: one link node
            // whose text carries the whole span.
            assert!(
                nodes.iter().any(|n| matches!(n, InlineNode::Ref(_))),
                "the tree's own reading must still build a link: {nodes:?}"
            );
        }
    }

    #[test]
    fn a_bare_auto_link_crossing_an_escaped_special_shows_structured_children() {
        // A URL whose body contains `&` is matched by the string pipeline over
        // the *escaped* text (`…?a=1&amp;b=2`) — and the level's match string
        // carries exactly those bytes for the `CharRef::Special` the
        // `SpecialCharacters` step made, so the target this pass computes is
        // the one the replacer computed. A bare link's shown text *is* that
        // target, so it is recovered from the URL's own range as structured
        // children (each special keeping its precise `'src` span, #944) rather
        // than baked, already escaped, into one `Text` the fold would escape
        // twice.
        let source = "https://example.org/?a=1&b=2";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        let reference = assert_link(&nodes[0]);

        assert_eq!(
            reference.target.as_ref(),
            "https://example.org/?a=1&amp;b=2"
        );
        assert_eq!(reference.roles, [CowStr::from("bare")]);
        assert_eq!(reference.location.data(), source);

        assert_eq!(reference.children.len(), 3);
        assert_text(&reference.children[0], "https://example.org/?a=1", 1, 1);

        let location = assert_special_char(&reference.children[1], '&');
        assert_eq!(location.data(), "&");
        assert_eq!(location.byte_offset(), 24);

        assert_text(&reference.children[2], "b=2", 1, 26);

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn a_formal_url_display_text_crossing_an_escaped_special_becomes_structured_children() {
        // The bracketed display text takes the same structured rebuild the
        // `link:`/`mailto:` macro family's does, through the shared
        // `macro_text_children`: the special stays its own `CharRef` child and
        // folds back to one entity, where a single `Text` holding `&lt;` would
        // be escaped a second time.
        let source = "https://example.org[a < b]";
        let nodes = build_src(Span::new(source));

        let reference = assert_link(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "https://example.org");
        assert!(reference.roles.is_empty());

        assert_eq!(reference.children.len(), 3);
        assert_text(&reference.children[0], "a ", 1, 21);

        let location = assert_special_char(&reference.children[1], '<');
        assert_eq!(location.data(), "<");

        assert_text(&reference.children[2], " b", 1, 24);

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn a_formal_url_display_text_crossing_an_escaped_special_keeps_its_own_escapes() {
        // The structured rebuild still applies `InlineLinkReplacer`'s own `\]`
        // unescape (expressed as a gap in the emitted ranges) and still strips
        // the `^` window suffix, which sits one byte past the text the children
        // cover.
        let source = "https://example.org[a\\] < b^]";
        let nodes = build_src(Span::new(source));

        let reference = assert_link(&nodes[0]);
        assert_eq!(reference.window.as_deref(), Some("_blank"));
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
    fn an_escaped_auto_link_crossing_a_rendered_span_still_drops_its_backslash() {
        // The escape check runs *before* the gate, mirroring
        // `InlineLinkReplacer`'s own scheme-backslash-first order: an escaped
        // auto-link whose match the gate rejects still drops its backslash and
        // keeps the rest, which for a rendered span means the span's own nodes
        // fold back to exactly the markup `caps[0][prefix.len() + 1..]` emits.
        let source = "\\https://example.org/*bold*";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an escaped auto-link must not build a link node: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn a_bare_url_whose_trailing_strip_would_split_a_special_is_a_documented_divergence() {
        // The trailing-punctuation strip keys off the target's *final
        // character*, over the escaped text — so a bare URL ending in a literal
        // `&` (whose match-string tail is `&amp;`) satisfies it on that
        // entity's own `;`. The string replacer happily splits the entity
        // (target `…/a&amp`, a literal `;` after the link); here the boundary
        // would fall *inside* a `CharRef` leaf, which `emit_range` can only
        // emit whole, so the link is left literal instead — the one form this
        // family's escaped-special lift does not reach.
        let source = "https://example.org/a&";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "a bare URL whose strip splits a special must stay literal: {nodes:?}"
        );

        // The string pipeline, by contrast, builds a link on the split entity.
        assert!(golden_macros(source).contains(r#"href="https://example.org/a&amp""#));
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_a_link_crossing_a_restored_entity() {
        // A *restored entity* (`&amp;copy;` written in the source, which the
        // character-replacements step turns back into a `CharRef::Entity`
        // whose value is `&copy;`) is admitted for the same reason an escaped
        // special is: its match-string bytes are the string pipeline's own
        // haystack bytes there, and the fold emits them verbatim. It is
        // recovered as its own `CharRef` child rather than baked into a `Text`
        // the fold would escape a second time.
        let fixtures = [
            // A display text crossing one, in each of the four link spellings.
            "https://example.org[a &amp; b]",
            "https://example.org[a &copy; b]",
            "link:index.html[a &copy; b]",
            "mailto:a@b.com[Tom &copy; Jerry]",
            "<https://example.org/a&copy;b>",
            // A *target* crossing one, bare and bracketed.
            "https://example.org/?a=&copy;b",
            "link:a&copy;b.html[]",
            "link:a&copy;b.html[Text]",
            // A bare e-mail address, the family's fifth spelling.
            "doc&copy;a@example.org",
            // A target crossing both a restored entity and an escaped special.
            "link:a&copy;b&c.html[]",
            // In flow, doubled, inside a rendered span, and escaped.
            "see link:a&copy;b.html[Docs] now",
            "link:a&copy;b.html[] link:c&reg;d.html[]",
            "*link:a&copy;b.html[Docs]*",
            "\\link:a&copy;b.html[Docs]",
            // An entity beside the macro rather than inside it.
            "&copy;link:x.html[Docs]&reg;",
            // A text carrying an **attribute list** across one, in all three
            // spellings: the parse reads the match string's own `&copy;`, and
            // the positional value it returns is rebuilt through
            // `escaped_value_children` so the entity folds back once.
            "https://example.org[a &copy; b,role=hl]",
            "link:index.html[a &copy; b,role=hl]",
            "mailto:a@b.com[Tom &copy; Jerry,Subject here]",
            // The numeric spellings, as three real-world fixtures elsewhere in
            // this crate's Asciidoctor port write them: a display text
            // carrying an escaped `]`, a target carrying an escaped space, and
            // a typographic apostrophe immediately before a bare auto-link.
            "http://example.com[sam&#93;ple]bracket]",
            "link:My&#32;Documents/report.pdf[Get Report]",
            "l&#8217;http://www.irit.fr[IRIT]",
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
    fn fold_matches_the_string_pipeline_for_a_link_crossing_a_character_replacement() {
        // A *typographic replacement* (`(C)` and `'`, which the
        // character-replacements step turns into `CharRef::Replacement`
        // leaves) is admitted for the same reason the two other `CharRef`
        // leaves are: its match-string bytes — the entity the built-in backend
        // renders it as (`&#169;`, `&#8217;`) — are the string pipeline's own
        // haystack bytes there, and the fold routes the leaf back through the
        // renderer to those same bytes. A display text carrying one keeps it
        // as its own child rather than baking the entity into a `Text` the
        // fold would escape.
        let fixtures = [
            // A display text crossing one, in each of the four link spellings.
            "https://example.org[a (C) b]",
            "https://example.org[O'Reilly]",
            "link:index.html[a (C) b]",
            "mailto:a@b.com[Tom (C) Jerry]",
            "<https://example.org/a(C)b>",
            // A *target* crossing one, bare and bracketed.
            "link:a(C)b.html[]",
            "link:a(C)b.html[Text]",
            "https://example.org/?a=(C)b",
            // A bare e-mail address abutting one — the `;` a replacement's
            // entity ends in is no mismatch character, so the string pipeline
            // links the address that follows it, and now so does the tree.
            "a(C)b@example.com",
            // A target crossing a replacement, an escaped special, and a
            // restored entity at once.
            "link:a(C)b&c&copy;d.html[]",
            // In flow, doubled, inside a rendered span, and escaped.
            "see link:a(C)b.html[Docs] now",
            "link:a(C)b.html[] link:c(R)d.html[]",
            "*link:a(C)b.html[Docs]*",
            "\\link:a(C)b.html[Docs]",
            // A replacement beside the macro rather than inside it.
            "(C)link:x.html[Docs](R)",
            // A text carrying an **attribute list** across one, in all three
            // spellings: the parse reads the match string's own entity, and
            // the positional value it returns is rebuilt through
            // `escaped_value_children` so the entity folds back once.
            "https://example.org[a (C) b,role=hl]",
            "link:index.html[a (C) b,role=hl]",
            "mailto:a@b.com[Tom (C) Jerry,Subject here]",
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
    fn a_display_text_crossing_a_character_replacement_keeps_it_as_its_own_child() {
        // The structural companion: the text is rebuilt through
        // `macro_text_children`'s `emit_range` path, so the replacement stays
        // the `CharRef::Replacement` leaf it already is — folding back through
        // the renderer — with `Text` runs on either side borrowing `'src`.
        let source = "https://example.org[a (C) b]";
        let nodes = build_src(Span::new(source));

        let reference = match &nodes[0] {
            InlineNode::Ref(reference) => reference,
            other => panic!("expected a Ref, got {other:?}"),
        };

        assert_eq!(reference.children.len(), 3, "{:?}", reference.children);
        assert_text(&reference.children[0], "a ", 1, 21);

        assert!(
            matches!(
                &reference.children[1],
                InlineNode::CharRef {
                    value: CharRef::Replacement(value),
                    ..
                } if *value == "\u{a9}"
            ),
            "{:?}",
            reference.children[1]
        );

        assert_text(&reference.children[2], " b", 1, 26);
    }

    #[test]
    fn a_display_text_crossing_a_restored_entity_keeps_the_entity_as_its_own_child() {
        // The structural companion: the text is rebuilt through
        // `macro_text_children`'s `emit_range` path, so the entity stays the
        // `CharRef::Entity` leaf it already is — folding back to its own bytes
        // — with `Text` runs on either side borrowing `'src`.
        let source = "https://example.org[a &copy; b]";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        let reference = assert_link(&nodes[0]);

        assert_eq!(reference.target.as_ref(), "https://example.org");
        assert_eq!(reference.children.len(), 3);
        assert_text(&reference.children[0], "a ", 1, 21);

        // The leaf's own span is precise — the entity as the author wrote it,
        // which is also the value it carries (the `SpecialCharacters` escape
        // the replacements step undid leaves no trace in either).
        let entity = assert_entity(&reference.children[1], "&copy;");
        assert_eq!(entity.data(), "&copy;");
        assert_eq!(entity.col(), 23);

        assert_text(&reference.children[2], " b", 1, 29);
    }

    #[test]
    fn an_attribute_list_value_crossing_a_restored_entity_keeps_the_entity_as_its_own_child() {
        // The same structural guarantee for the one display text this family
        // *computes* rather than slices. An attribute-list text with no `'src`
        // slice is parsed from the level's match string, so its positional
        // value comes back already escaped (`a &copy; b`); rebuilding it
        // through `escaped_value_children` splits the entity into its own
        // `CharRef::Entity` leaf — which the fold emits verbatim — where one
        // `Text` holding the whole value would escape its `&` a second time.
        // Every part of the value shares the bracketed text's coarse span
        // (design §4.4): a parsed value has no `'src` slice of its own.
        let source = "https://example.org[a &copy; b,role=hl]";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        let reference = assert_link(&nodes[0]);

        assert_eq!(reference.children.len(), 3);

        // Every part shares the bracketed text's span, and the `Text` runs
        // hold *owned* values (they come from a parse of a temporary, not from
        // a slice of `'src`) — which is why they are read directly here rather
        // than through `assert_text`, whose borrow check they cannot satisfy.
        match &reference.children[0] {
            InlineNode::Text { value, location } => {
                assert_eq!(value.as_ref(), "a ");
                assert_eq!(location.data(), "a &copy; b,role=hl");
            }

            other => panic!("expected Text, got {other:?}"),
        }

        let entity = assert_entity(&reference.children[1], "&copy;");
        assert_eq!(entity.data(), "a &copy; b,role=hl");

        match &reference.children[2] {
            InlineNode::Text { value, location } => {
                assert_eq!(value.as_ref(), " b");
                assert_eq!(location.data(), "a &copy; b,role=hl");
            }

            other => panic!("expected Text, got {other:?}"),
        }

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn an_attribute_list_text_crossing_an_escaped_special_owns_its_parsed_values() {
        // The escaped-special counterpart: the match string holds the
        // canonical entity (`a &lt; b`) where the source holds `a < b`, so the
        // list is parsed from that string and `into_owned`ed onto the
        // bracketed text's own coarse span. The positional value the parse
        // returns is escaped text, which `escaped_value_children` turns back
        // into the logical `<` a `Text` node holds — folded back to `&lt;`
        // exactly once.
        let source = "link:index.html[a < b,role=hl]";
        let nodes = build_src(Span::new(source));

        let reference = assert_link(&nodes[0]);
        assert_eq!(link_text_of(reference), "a < b");

        let attrs = reference.attrs.as_ref().unwrap();
        assert_eq!(attrs.roles().into_iter().next(), Some("hl"));

        // The parsed positional value is the *escaped* text the string
        // replacer's own attrlist carries, and the list's location tag is the
        // bracketed text as written.
        assert_eq!(attrs.nth_attribute(1).unwrap().value(), "a &lt; b");
        assert_eq!(attrs.span().data(), "a < b,role=hl");

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
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
    fn fold_matches_the_string_pipeline_for_a_display_text_crossing_a_rendered_span() {
        // The differential corpus for this increment: a `link:`/`mailto:`
        // macro whose bracketed display text crosses an **opaque** piece — a
        // rendered span, an already-recognized macro node of another family, a
        // masked passthrough. The text is carried structurally (each opaque
        // piece's own node becomes a child), so the fold re-renders exactly the
        // markup the string replacer captured in its own text.
        let fixtures = [
            // (A masked passthrough is opaque here too — and in the string
            // pipeline, which restores passthroughs only after every step — but
            // this oracle runs the steps directly, without the extraction the
            // real `SubstitutionGroup::apply` performs around them, so those
            // fixtures live in the whole-pipeline sweep instead; see
            // `inline_builder::tests`.)
            //
            // A span at the end of the text, at the start, in the middle, and
            // spanning the whole of it.
            "link:index.html[with *bold* text]",
            "link:index.html[*bold* leads]",
            "link:index.html[a *b* c]",
            "link:index.html[*bold*]",
            // Every quoted form the earlier step can have produced, including
            // an attributed span (whose markup carries an `=` the string
            // replacer's own attribute-list probe reads, and this one does not:
            // with no comma to split on, the parse yields one positional value
            // equal to the whole text, so both take the plain-text path).
            "link:index.html[_em_ and `code` and #mark#]",
            "link:index.html[super^script^ and sub~script~]",
            "link:index.html[[.hl]#roled#]",
            // An already-recognized macro node of another family: an image, an
            // icon, a UI macro, an index term.
            "link:index.html[the image:logo.png[Logo] here]",
            "link:index.html[the icon:tags[] here]",
            "link:index.html[a kbd:[Ctrl+T] b]",
            "link:index.html[a ((index term)) *b*]",
            // A span *and* an escaped special / restored entity in one text —
            // the recoverable and structural recoveries side by side — and a
            // span crossing the target's own escaped special.
            "link:index.html[a < b and *bold*]",
            "link:index.html[a &copy; b and _em_]",
            "link:a&b.html[*bold* text]",
            // The `\]` unescape and the `^` window suffix still apply around a
            // span, as does the text-trimming the replacer performs.
            "link:index.html[a\\] *b* c]",
            "link:index.html[*Open*^]",
            "link:index.html[ *b* ]",
            // The `mailto:` spelling, whose display text takes the identical
            // path (its own attribute-list probe is a `,`, not an `=`).
            "mailto:hello@example.org[write *now*]",
            "mailto:hello@example.org[*now*]",
            "mailto:a&b@example.org[*bold* mail]",
            // Escaped: the backslash is dropped and the span stays in the flow.
            "\\link:index.html[*bold*]",
            "\\mailto:a@example.org[*bold*]",
            // In surrounding flow, beside a sibling macro, and inside a
            // rendered span of its own.
            "See link:about.html[the *bold* one] for details.",
            "See link:x.html[*a*] and link:y.html[_b_].",
            "_a link:x.html[*b*] c_",
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
    fn a_display_text_carries_a_rendered_span_as_its_own_child() {
        // A display text crossing a rendered span is carried *structurally*:
        // the span is one opaque placeholder in the match string, but
        // `macro_text_children` recovers the text with `emit_range`, which
        // clones the span's own node whole into the link's children. The fold
        // then re-renders exactly the markup the string replacer captured in
        // its own display text.
        let source = "link:index.html[a *bold* b]";
        let nodes = build_src(Span::new(source));

        let reference = assert_link(&nodes[0]);

        // Three children: the text before the span, the span itself, the text
        // after it — each borrowing its own precise `'src` slice, which the
        // one-`Text`-child shape this replaced could not express.
        assert_eq!(reference.children.len(), 3);
        assert_text(&reference.children[0], "a ", 1, 17);

        let styled = assert_styled(
            &reference.children[1],
            StyleVariant::Strong,
            SpanForm::Constrained,
        );

        assert_text(&styled[0], "bold", 1, 20);
        assert_text(&reference.children[2], " b", 1, 25);

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn a_link_target_over_a_rendered_span_is_a_documented_divergence() {
        // The **target** is a value this pass computes off the match string,
        // where a rendered span is one opaque placeholder standing in for
        // markup that exists only at fold time — so the target keeps
        // `range_has_no_opaque_piece` while the display text lifts it.
        //
        // If this boundary is ever lifted, fold these fixtures into the parity
        // corpus above.
        for source in ["link:a``b``c.html[x]", "mailto:a``b``c@example.org[x]"] {
            let nodes = build_src(Span::new(source));

            assert!(
                nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
                "a target crossing a rendered span must stay literal: {nodes:?}"
            );

            // The string pipeline, by contrast, *does* build a link here.
            assert!(golden_macros(source).contains("<a href"));
        }
    }

    #[test]
    fn a_link_text_attribute_list_over_a_rendered_span_is_a_documented_divergence() {
        // A text carrying an attribute list is parsed as a real
        // `Attrlist<'src>` from the source's own bytes, and a placeholder
        // inside a *parsed* value has no node it can be mapped back to — the
        // same reason the cross-reference family defers its own
        // `Attrlist`-bearing capture. That one text shape keeps the stricter
        // gate outright, for both the `link:` (`=`) and `mailto:` (`,`)
        // spellings.
        //
        // If this boundary is ever lifted, fold these fixtures into the parity
        // corpus above.
        for source in [
            "link:index.html[a *b* c,role=hl]",
            "mailto:a@example.org[a *b* c,Subj]",
        ] {
            let nodes = build_src(Span::new(source));

            assert!(
                nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
                "an attribute-list text crossing a rendered span must stay literal: {nodes:?}"
            );

            // The string pipeline, by contrast, *does* build a link here.
            assert!(golden_macros(source).contains("<a href"));
        }
    }

    #[test]
    fn a_span_whose_markup_perturbs_the_string_pipeline_is_a_documented_divergence() {
        // What the structural recovery cannot do is make the *recognition*
        // agree in every case: the string replacer matches over the span's
        // markup where this matches over the one placeholder standing in for
        // it, so the two read the same extent only while that markup carries
        // no character the pattern (or the replacer's own attribute-list
        // probe) is sensitive to. These are the three shapes where it does —
        // and in each the string pipeline's reading is the markup-perturbed
        // one (a truncated text, a text the attribute-list parse cut in half)
        // and the tree's the well-formed one, exactly as the quotes step's own
        // crossed-delimiter divergence is.
        for source in [
            // A `]` inside the span ends `INLINE_LINK_MACRO`'s own lazy text
            // capture early for the string replacer, but not here.
            "link:index.html[a *b ] c* d]",
            // Markup carrying an `=` (an attributed span) *and* a comma
            // elsewhere in the text: the string replacer's attribute-list
            // probe fires on the markup's own `=`, and the parse then splits
            // the text at that comma, keeping only what precedes it.
            "link:index.html[one, [.hl]#two#]",
            // The `mailto:` spelling's own probe is a `,`, so a comma the span
            // itself carries sends the string replacer down the subject branch
            // where this one stays plain.
            "mailto:a@example.org[a *b, c* d]",
        ] {
            let nodes = build_src(Span::new(source));

            assert_ne!(
                fold_html(&build_src(Span::new(source)), &HtmlSubstitutionRenderer {}),
                golden_macros(source),
                "{source:?} now agrees with the string pipeline; fold it into the parity corpus"
            );

            // The tree's own reading is the well-formed one: one link node
            // whose text carries the whole span.
            assert!(
                nodes.iter().any(|n| matches!(n, InlineNode::Ref(_))),
                "the tree's own reading must still build a link: {nodes:?}"
            );
        }
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
            // An address crossing an escaped special: the pattern's local-part
            // class admits `&amp;`, so the `&` is part of the address in both
            // pipelines.
            "a&b@example.com",
            "write to a&b@example.com today",
            "a&b&c@example.com",
            "a&b@example.com and d&e@example.org",
            // A literal `&` the pattern's classes do *not* admit — in the
            // domain, or opening the local part — so neither pipeline matches
            // the whole address there.
            "user@ex&ample.com",
            "&doc@example.com",
            // An address beside, but not crossing, an escaped special.
            "a < b then doc@example.com",
            "doc@example.com & more",
            // A construct *inside* what would otherwise be an address. Every
            // opaque piece is one `SPAN_PLACEHOLDER`, which none of the
            // pattern's character classes admit, so neither pipeline matches
            // across one — the invariant that lets this family lift the
            // escaped-special boundary with no gate of its own (see
            // `email_level`).
            "a*b*c@example.com",
            "a`b`c@example.com",
            "doc@ex*a*ample.com",
            "doc@ex(C)ample.com",
            "doc@exa--mple.com",
            "doc@example.co(C)m",
            // Escapes: the address stays literal, minus the backslash.
            "\\doc.writer@example.com",
            "a \\doc@example.com b",
            "\\a&b@example.com",
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
        // now make the same lift for their own targets and display texts —
        // an attribute-list-bearing one included, since `text_attrlist` parses
        // a text with no `'src` slice from the level's match string and owns
        // the result off it.
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
    fn an_address_at_a_spans_own_edge_reads_that_spans_boundary_characters() {
        // The mismatch-prefix group reads the character immediately before the
        // address, and inside a span the string pipeline reads that span's own
        // *rendered* markup there: `<strong>` ends in `>`, one of the group's
        // three mismatch characters, so `*doc@example.org*` keeps the address
        // literal where a level matched in isolation sees a start anchor and
        // links it. The macros step takes the same
        // [`LevelContext`](super::super::quotes::LevelContext) the quotes and
        // character-replacements steps do, so the two pipelines agree again.
        for source in [
            // Against a span's opening edge, in each variant's own rendering
            // shape. Every tag-rendered variant ends its opening markup in `>`
            // — a mismatch character — so the address stays literal in both.
            "*doc@example.org*",
            "_doc@example.org writes_",
            "`doc@example.org`",
            "#doc@example.org#",
            "^doc@example.org^",
            "~doc@example.org~",
            "**doc@example.org**",
            "[.hl]#doc@example.org#",
            // The two smart-quote variants end theirs in the `;` of an entity,
            // which is *not* a mismatch character — so both pipelines link the
            // address there.
            r#"He said "`doc@example.org`" today."#,
            r#"He said '`doc@example.org`' today."#,
            // Away from the opening edge, where the character before the
            // address is the level's own text in both pipelines.
            "*write to doc@example.org now*",
            "*a doc@example.org b*",
            r#""`write to doc@example.org now`""#,
            // An escape at a span's own edge still drops its backslash: the
            // prefix group takes the `\` rather than the context character.
            "*\\doc@example.org*",
            // A replacement or a restored entity before the address presents
            // its own last character (`;` of the entity the built-in backend
            // renders), which is no mismatch character in either pipeline.
            "*(C)doc@example.org*",
            "*&copy;doc@example.org*",
            // And the same addresses at the content's own top level, where a
            // level's start is exactly what the string pipeline presents.
            "doc@example.org",
            "write to doc@example.org now",
        ] {
            assert_eq!(
                fold_html(&build_src(Span::new(source)), &HtmlSubstitutionRenderer {}),
                golden_macros(source),
                "fold diverged from the string pipeline for {source:?}"
            );
        }
    }

    #[test]
    fn an_address_at_a_tag_rendered_spans_edge_builds_no_node() {
        // The parity above, read structurally: the address is left as literal
        // text rather than recognized into a link the string pipeline does not
        // build.
        let nodes = build_src(Span::new("*doc@example.org*"));
        let children = assert_styled(&nodes[0], StyleVariant::Strong, SpanForm::Constrained);

        assert!(
            children.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an address at a span's own edge must stay literal: {children:?}"
        );
    }

    #[test]
    fn an_address_after_a_transparent_spans_sibling_is_a_documented_divergence() {
        // An unquoted span whose attribute list resolves to neither a role nor
        // an id renders to its body and nothing else, so its children inherit
        // the context the span itself sits in
        // ([`LevelContext::inside_styled`](super::super::quotes::LevelContext)).
        // That is right whenever the span is all its parent's level holds and
        // wrong when a *sibling* precedes it: the string pipeline's haystack
        // shows what that sibling ends with (a space here) where the inherited
        // context still shows the enclosing `<strong>`'s own `>`, so the
        // address stays literal here and links there.
        //
        // This is the transparent-span half of the same class the quotes and
        // character-replacements steps already document
        // (`a_replacement_beside_a_transparent_span_is_a_documented_divergence`);
        // closing it means deriving a level's context from its *siblings*
        // rather than from its enclosing construct alone. If that lands, fold
        // this fixture into the parity corpus above.
        let source = "*x [width=10]#doc@example.org#*";
        let folded = fold_html(&build_src(Span::new(source)), &HtmlSubstitutionRenderer {});

        assert_ne!(
            golden_macros(source),
            folded,
            "expected the documented divergence to still reproduce"
        );

        assert!(!folded.contains("mailto:"), "{folded:?}");
        assert!(golden_macros(source).contains("mailto:doc@example.org"));
    }

    #[test]
    fn a_real_documents_span_edge_addresses_fold_to_their_rendered_strings() {
        // End-to-end, through the real parse path, on the shapes that named
        // this increment: an address written against a span's own edge must
        // stay literal (as the string pipeline leaves it) and one written
        // against a smart quote's must link, so a tree that decided either the
        // other way would regress the moment `rendered_html()` becomes a fold
        // of this tree.
        use crate::blocks::{FindBlocks, IsBlock};

        let doc = Parser::default().with_inline_tree(true).parse(concat!(
            "== A heading\n",
            "\n",
            "Mail *doc@example.org* or _doc@example.org writes_ here.\n",
            "\n",
            "He said \"`doc@example.org`\" and *https://example.org* too.\n",
        ));

        let mut folded_blocks = 0;

        for block in doc.descendant_blocks() {
            let (Some(rendered), Some(inlines)) = (block.rendered_html_content(), block.inlines())
            else {
                continue;
            };

            assert_eq!(
                crate::content::inline_builder::fold_html(
                    inlines,
                    &HtmlSubstitutionRenderer {},
                    &Parser::default()
                ),
                rendered,
                "fold diverged from the rendered string for {inlines:?}"
            );

            folded_blocks += 1;
        }

        // The two paragraphs; the section that contains them holds no inline
        // content of its own and is skipped.
        assert_eq!(folded_blocks, 2, "expected both paragraphs to be checked");
    }

    #[test]
    fn an_email_crossing_an_escaped_special_shows_structured_children() {
        // The pattern's local-part class admits `&amp;`, so the string
        // pipeline matches an address carrying a literal `&` over its own
        // *escaped* text. The match string carries the same entity there, so
        // the target this pass computes off it is the very one the replacer
        // computed (and registers), and the shown text — which for this form
        // *is* the address — is recovered from the match's own range as
        // structured children, so the `&` stays the `CharRef` it already is
        // rather than being escaped a second time.
        let source = "a&b@example.com";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        let reference = assert_link(&nodes[0]);

        assert_eq!(reference.target.as_ref(), "mailto:a&amp;b@example.com");
        assert!(reference.roles.is_empty());
        assert_eq!(reference.location.data(), source);

        assert_eq!(reference.children.len(), 3);
        assert_text(&reference.children[0], "a", 1, 1);

        let location = assert_special_char(&reference.children[1], '&');
        assert_eq!(location.data(), "&");
        assert_eq!(location.byte_offset(), 1);

        assert_text(&reference.children[2], "b@example.com", 1, 3);

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
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
            // The same, for the auto-link / formal-URL family (whose own
            // registration pass runs first) and the ANGLE branch.
            "https://example.org/?a=1&b=2",
            "https://example.org/a&b[Example]",
            "<https://example.org/a&b>",
            "https://a.example/?x=1&y=2 then link:b&c.html[B]",
            // And for the bare-e-mail family, whose registration pass runs
            // last: the `mailto:` target it records carries the address's own
            // `&amp;`.
            "a&b@example.com",
            "a&b@example.com then link:c.html[C] then https://d.example",
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
            .with_intrinsic_attribute("attrs", "Docs,role=hl", ModificationContext::Anywhere)
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
        // design §4.4's coarse fallback — an attribute-list-bearing display
        // text included, which `text_attrlist` parses from that same match
        // string and owns off it. The one shape that still defers, a wholly
        // expanded `link:`/`mailto:` macro, has its own divergence test
        // below.
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
            // A display text carrying an **attribute list** over a spliced
            // value, in all three spellings: the list is parsed from the
            // level's match string — which carries the expansion's own bytes,
            // exactly as the string replacer's already-expanded haystack does
            // — and owned onto design §4.4's coarse span.
            "link:index.html[{label},role=hl]",
            "https://example.org[{label},role=hl]",
            "mailto:hello@example.org[{label},Hi there]",
            // The whole attribute list from one expansion, and a text that is
            // partly spliced.
            "link:index.html[{attrs}]",
            "https://example.org[Docs for {label},role=hl]",
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
    fn an_attribute_list_text_inside_an_expanded_value_owns_its_parsed_values() {
        // The structural companion of the parity fixtures above, and the
        // counterpart of `a_link_inside_an_expanded_value_keeps_a_coarse_
        // location`: a display text with no `'src` slice is parsed from the
        // level's match string, so every value on the resulting `Attrlist`
        // is **owned** off that temporary, and the list's own location tag
        // falls back to the bracketed text's coarse span (design §4.4).
        let parser = expanding_parser();
        let source = "link:index.html[{label},role=hl]";
        let nodes = build(Span::new(source), &parser, None);

        assert_eq!(nodes.len(), 1);
        let reference = assert_link(&nodes[0]);

        assert_eq!(link_text_of(reference), "Docs");

        let attrs = reference.attrs.as_ref().unwrap();
        assert_eq!(attrs.roles().into_iter().next(), Some("hl"));

        // The positional value is the *expansion*, which no `'src` slice
        // holds — it came back from a parse of the match string.
        assert_eq!(attrs.nth_attribute(1).unwrap().value(), "Docs");

        // The list's location tag is the bracketed text as *written*, not the
        // expansion it stands for (design §4.4's coarse fallback).
        assert_eq!(attrs.span().data(), "{label},role=hl");
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
