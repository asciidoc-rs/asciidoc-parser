//! Image and icon macro recognition (`image:target[…]`, `icon:target[…]`).

use std::borrow::Cow;

use super::{MacroMatch, MacroMatchKind, links::restore_masked_passthroughs, rebuild_macro_level};
use crate::{
    Parser, Span,
    attributes::{Attrlist, AttrlistContext},
    content::{
        INLINE_IMAGE_MACRO, basename,
        inline_builder::{
            fold::{fold_html, fold_stem, render_char},
            quotes::{
                LevelContext, Piece, build_match_string, charref_entity, single_text_value,
                source_slice, text_slice,
            },
            special_chars::Masked,
        },
        normalize_text_lf_escaped_bracket,
    },
    inlines::{Image, InlineNode, RawForm, RawOrigin},
    parser::{InlineRenderer, has_dangerous_scheme, has_dangerous_self_href, is_uri_ish},
    strings::CowStr,
    warnings::WarningType,
};

/// Mirrors the string step's `found_macroish`: an image or icon macro needs
/// its name prefix and an opening bracket. Shared between
/// [`image_macros_level`]'s pre-build sniff and its post-build one, so the
/// two answers cannot drift apart.
fn image_macro_prefilter(s: &str) -> bool {
    (s.contains("image:") || s.contains("icon:")) && s.contains('[')
}

/// Matches `INLINE_IMAGE_MACRO` at this level's escaped text, replacing each
/// recognized match with the [`Image`](InlineNode::Image) node it produces and
/// leaving everything else in place.
pub(super) fn image_macros_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
    ctx: LevelContext,
    masked: Masked<'_>,
) -> Vec<InlineNode<'src>> {
    // Cheap pre-filter, taken *before* the match string is materialized: a
    // single, unsplit `Text` node's match string is its own value, so the
    // check below can run against that directly. A level already split by
    // an earlier step falls back to the build, exactly as before.
    if single_text_value(&nodes).is_some_and(|value| !image_macro_prefilter(value)) {
        return nodes;
    }

    let (s, pieces) = build_match_string(&nodes, masked);

    // Cheap pre-filter mirroring the string step's `found_macroish`: an image
    // or icon macro needs its name prefix and an opening bracket.
    if !image_macro_prefilter(&s) {
        return nodes;
    }

    // Matched over the level wrapped in the boundary character its enclosing
    // construct presents, with the level's own pieces moved into that string's
    // coordinates — see `apply_macro_families`'s own doc comment.
    let (s, pieces) = ctx.shift(s, pieces);

    // …and with each masked passthrough's or STEM expression's placeholder
    // widened into the three-byte token [`INLINE_IMAGE_MACRO`]'s target class
    // needs to match it — see `widen_masked_pieces` for why this family alone
    // needs that.
    let (s, pieces) = widen_masked_pieces(s, pieces, &nodes);

    let matches = find_image_matches(&s, &pieces, root, parser, &nodes);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every image/icon macro at this level, skipping any whose match crosses
/// an [`opaque`](range_has_no_opaque_piece) piece (see
/// [`apply_macros`](super::apply_macros)).
fn find_image_matches<'src>(
    s: &str,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
    nodes: &[InlineNode<'src>],
) -> Vec<MacroMatch<'src>> {
    let mut matches = Vec::new();

    for caps in INLINE_IMAGE_MACRO.captures_iter(s) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();

        let full = whole.start()..whole.end();

        // An escape (`\image:`) is honored by dropping the backslash and
        // keeping the rest literal, mirroring `InlineImageMacroReplacer`'s own
        // leading `caps[0].starts_with('\\')` check — which it makes *before*
        // looking at anything else, so the escape needs no gate of its own
        // here either: dropping the backslash keeps the rest of the match as
        // its **own original nodes** (a rendered span or an escaped special
        // among them), which fold back to exactly the bytes the replacer's
        // `caps[0][1..]` emits. (This is the same check-order fix the
        // `footnoteref:`, menu, cross-reference, and link increments made for
        // their own families; before it, an escaped `\image:x.png[*bold*]`
        // whose match the gate rejected was left unrecognized, backslash and
        // all.)
        if whole.as_str().starts_with('\\') {
            matches.push(MacroMatch {
                kind: MacroMatchKind::Unescape {
                    backslash: full.start,
                },
                full,
            });

            continue;
        }

        // The gates cover the bytes each capture *reads*, not the whole
        // match. The **bracket** (group 2) comes back from a *parse* —
        // `bracket_attrlist` reads its bytes as content — so a placeholder
        // there would be read as literal text, and it keeps the opaque-piece
        // gate for a rendered span, admitting a masked passthrough or STEM
        // expression (whose placeholder `Attrlist::parse` swallows into a
        // value that only *restores* after the split — the
        // order [`tokened_bracket`] and
        // [`Attrlist::into_owned_restoring`](Attrlist) reproduce). The
        // **target** (group 1) is the one value this family computes off the
        // match string, and — like the `link:`/`mailto:` family's — it
        // admits both masked kinds, restored into the computed values
        // exactly as `Passthroughs::restore_to` rewrites the rendered `src`
        // (see [`range_is_restorable`] and [`restore_masked_passthroughs`];
        // the fold-time `web_path` this family alone sits behind runs over
        // the restored ranges *masked* — see
        // [`Image::restored_target_ranges`](crate::inlines::Image)). The
        // macro name and the two square brackets need no gate of their own:
        // those bytes are literal, and no atomic piece — a placeholder, or an
        // entity delimited by `&` and `;` — can supply them.
        if let Some(target) = caps.get(1)
            && !range_is_restorable(nodes, pieces, &(target.start()..target.end()))
        {
            continue;
        }

        let node = build_image_node(&caps, whole.as_str(), &full, pieces, root, parser, nodes);

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

/// Reports whether every piece overlapping the match-string range `range` is a
/// verbatim [`Text`](InlineNode::Text) run (non-atomic, non-synthesized). Only
/// then does the range map one-to-one onto contiguous source, so its captures
/// can slice `'src` directly. A [`synthesized`](Piece::synthesized) piece
/// (an attribute-expanded value, a `counter` directive) is rejected here too:
/// [`apply_character_replacements`](super::super::char_replacements::apply_character_replacements)
/// can recognize a construct inside one (it produces a leaf needing no `'src`
/// slice of its own, falling back to the piece's coarse `location`), but a
/// macro node bakes its target/attribute list straight from source, so it still
/// needs a real `'src` slice a synthesized run cannot provide — the same
/// boundary an escaped-special or a rendered span already documents.
pub(in crate::content::inline_builder) fn range_is_verbatim(
    pieces: &[Piece],
    range: &std::ops::Range<usize>,
) -> bool {
    for piece in pieces {
        let p_start = piece.s_start;
        let p_end = piece.s_start + piece.s_len;

        // Skip pieces that do not overlap the range.
        if p_end <= range.start || p_start >= range.end {
            continue;
        }

        if piece.atomic || piece.synthesized {
            return false;
        }
    }

    true
}

/// The relaxed counterpart of [`range_is_verbatim`]: also accepts a range
/// that overlaps a [`synthesized`](Piece::synthesized) piece (an attribute
/// expansion, or — reached at a tree's root — a filtered multi-line block's
/// own joined seed), rejecting only an [`atomic`](Piece::atomic) overlap (an
/// escaped special or a rendered span) — the boundary every macro family
/// still enforces unchanged. A caller that accepts a range under this check
/// still cannot slice `'src` directly for it (the same reason
/// [`range_is_verbatim`] exists at all); it needs
/// [`text_slice`] rather than
/// [`source_slice`] to recover the
/// range's own *text* precisely, since `source_slice` only ever offers a
/// synthesized piece's coarse *location* as a fallback, never its
/// exact bytes.
pub(in crate::content::inline_builder) fn range_is_verbatim_or_synthesized(
    pieces: &[Piece],
    range: &std::ops::Range<usize>,
) -> bool {
    for piece in pieces {
        let p_start = piece.s_start;
        let p_end = piece.s_start + piece.s_len;

        // Skip pieces that do not overlap the range.
        if p_end <= range.start || p_start >= range.end {
            continue;
        }

        if piece.atomic {
            return false;
        }
    }

    true
}

/// The most relaxed of the three gates: accepts a range whose overlapping
/// pieces are all ones the tree can reproduce **exactly** — a
/// [`Text`](InlineNode::Text) run (verbatim or
/// [`synthesized`](Piece::synthesized)) or any of the three
/// [`CharRef`](InlineNode::CharRef) leaves, an *escaped special*
/// ([`Special`](crate::inlines::CharRef::Special)), a *restored entity*
/// ([`Entity`](crate::inlines::CharRef::Entity)), or a *typographic
/// replacement* ([`Replacement`](crate::inlines::CharRef::Replacement)) —
/// rejecting only an **opaque** piece: a rendered
/// [`Styled`](crate::inlines::Styled) span, an
/// earlier-recognized macro node, or a masked passthrough or STEM expression,
/// each of which [`build_match_string`] stands in as one
/// `SPAN_PLACEHOLDER` rather than its rendered markup or entity.
///
/// All three `CharRef` leaves are admissible for the same reason: their
/// match-string bytes — a special's canonical entity (`&lt;`, `&gt;`,
/// `&amp;`), a restored entity's own text (`&copy;`, `&#8217;`), a
/// replacement's built-in rendering (`&#169;` for `(C)`, `&#8217;` for `'`,
/// via [`replacement_entity`](super::super::quotes::replacement_entity)) — are
/// exactly the bytes each leaf renders as, so a family that reads its values
/// out of the match string sees the same escaped form its own output would.
/// What such a family cannot do is *slice* those bytes from `'src` (the source
/// holds one character, or `(C)`, where the match string holds an entity, and
/// `&amp;copy;` where it holds `&copy;`), so a value that must ride on the node
/// as an `'src` slice — an [`Attrlist`]`<'src>`, an
/// [`Image`](InlineNode::Image)'s bracket — keeps [`range_is_verbatim`], and a
/// *display text* recovered under this gate is rebuilt as structured children
/// with [`emit_range`](super::super::quotes::emit_range) (the leaf staying its
/// own `CharRef` child, which folds back to the same bytes) rather than as one
/// sliced `Text`.
pub(in crate::content::inline_builder) fn range_has_no_opaque_piece(
    nodes: &[InlineNode<'_>],
    pieces: &[Piece],
    range: &std::ops::Range<usize>,
) -> bool {
    for piece in pieces {
        let p_start = piece.s_start;
        let p_end = piece.s_start + piece.s_len;

        // Skip pieces that do not overlap the range.
        if p_end <= range.start || p_start >= range.end {
            continue;
        }

        if !piece.atomic {
            continue;
        }

        if !atomic_piece_is_recoverable(nodes, piece) {
            return false;
        }
    }

    true
}

/// Tells whether one [`atomic`](Piece::atomic) piece is a leaf
/// [`build_match_string`] gives real bytes to — the classification
/// [`range_has_no_opaque_piece`] applies per overlapping piece (see its own
/// doc comment for why exactly these three [`CharRef`](crate::inlines::CharRef)
/// leaves qualify).
fn atomic_piece_is_recoverable(nodes: &[InlineNode<'_>], piece: &Piece) -> bool {
    // The atomic pieces `build_match_string` gives real bytes to are the
    // three `CharRef` leaves — an escaped special, a restored entity, and
    // a typographic replacement the built-in backend has a rendering for;
    // everything else it stands in as one placeholder. Asking
    // [`charref_entity`](super::super::quotes::charref_entity) for those very
    // bytes is what makes the classification and the match string agree by
    // construction — including on a hand-built replacement carrying a value no
    // rule produces, which stays opaque here exactly as it does there.
    nodes
        .get(piece.node_index)
        .and_then(charref_entity)
        .is_some()
}

/// [`range_has_no_opaque_piece`], further admitting a
/// [`Raw`](InlineNode::Raw) leaf a **substitution produced in place** — an
/// expanded attribute value's literal `<`, `>`, or `&`, left unescaped
/// because the value expands *after* `specialcharacters` ran
/// ([`RawOrigin::Substitution`]).
///
/// The match string stands such a leaf in as one placeholder, so a value
/// reading it needs the same splice a masked construct's does
/// ([`restore_masked_passthroughs`](super::links)) — but the *timing* that
/// makes [`range_is_restorable`] wrong for a cross-reference does not apply
/// here, and that is the whole distinction:
///
/// - A **masked passthrough** is restored by a later pass. A deferred
///   cross-reference's target is captured into its segment *before* that pass
///   runs, so the captured segment still holds the placeholder — and a tree
///   that read the restored bytes instead would diverge from that documented
///   behavior (see
///   `a_deferred_xref_target_over_a_passthrough_is_a_documented_divergence`).
///   [`range_is_restorable`] admits it; this does not.
///
/// - A **substitution-produced** leaf was never extracted and is never
///   restored. Its bytes are simply *there*, in the match string's own
///   haystack, so filling the placeholder in reaches parity rather than
///   departing from it.
///
/// Deciding this from the node's own [`RawOrigin`] rather than from the
/// extraction pass's `Masked` list is what makes it reliable: that list is
/// keyed by location identity and is empty on call paths where the identity is
/// not in hand, so the same node classified differently depending on which pass
/// was asking.
pub(in crate::content::inline_builder) fn range_is_substitution_restorable(
    nodes: &[InlineNode<'_>],
    pieces: &[Piece],
    range: &std::ops::Range<usize>,
) -> bool {
    for piece in pieces {
        let p_end = piece.s_start + piece.s_len;

        // Skip pieces that do not overlap the range.
        if p_end <= range.start || piece.s_start >= range.end {
            continue;
        }

        if !piece.atomic {
            continue;
        }

        if matches!(
            nodes.get(piece.node_index),
            Some(InlineNode::Raw {
                origin: RawOrigin::Substitution,
                ..
            })
        ) {
            continue;
        }

        if !atomic_piece_is_recoverable(nodes, piece) {
            return false;
        }
    }

    true
}

/// [`range_has_no_opaque_piece`], further admitting a **masked** piece — a
/// passthrough or a STEM expression — for a value the caller *restores*
/// rather than reads: the match string represents it only as the
/// `\u{96}`*n*`\u{97}` placeholder, not its real rendered body, but that body
/// **is** known at build time — it is what `Passthroughs::restore_to`
/// splices over the placeholder after the steps run — so a computed value
/// that substitutes it for the placeholder (see
/// [`restore_masked_passthroughs`](super::links))
/// finishes with exactly the restored string's bytes.
///
/// That makes this gate right only for a value whose *recognition* treats the
/// masked span as one swallowed token and whose *use* happens after restore —
/// the `link:`/`mailto:` macro family's **target**, whose
/// `[^\s\[\]]+` body class swallows the placeholder like any other byte run,
/// and whose bytes reach the output (the `href`, and a bare macro's shown
/// text) only in the restored rendered string; and the `image:`/`icon:`
/// family's target, whose recognition needs the placeholder widened to that
/// same shape first ([`widen_masked_pieces`]), whose one
/// pre-restore computation, the `default_alt` derivation, runs over the
/// masked bytes itself ([`masked_default_alt`]), and whose fold-time
/// `web_path` runs over the restored ranges *masked* (see
/// [`Image::restored_target_ranges`](crate::inlines::Image)); and the
/// auto-link / formal-URL family's target, whose three URL classes swallow
/// either spelling with no widening at all and whose two pre-restore
/// decisions — rejecting a quoted URL, stripping a bare one's trailing
/// punctuation — read the placeholder token the same way they read ordinary
/// text (`links::build_inline_link_node`). It is right for a value that
/// comes back from a **parse** too, once that parse is given the
/// placeholder's own shape first — the `image:`/`icon:` bracket and the
/// link families' display-text attribute list, each tokened by
/// [`tokened_bracket`] and restored after the
/// split. A family that *matches
/// over* the masked bytes with a class the two spellings answer differently,
/// or reads them into a value used **before** restore (a
/// deferred cross-reference's target, captured into its placeholder template
/// with the placeholder still in it — see
/// `a_deferred_xref_target_over_a_passthrough_is_a_documented_divergence`),
/// keeps [`range_has_no_opaque_piece`].
pub(in crate::content::inline_builder) fn range_is_restorable(
    nodes: &[InlineNode<'_>],
    pieces: &[Piece],
    range: &std::ops::Range<usize>,
) -> bool {
    for piece in pieces {
        let p_start = piece.s_start;
        let p_end = piece.s_start + piece.s_len;

        // Skip pieces that do not overlap the range.
        if p_end <= range.start || p_start >= range.end {
            continue;
        }

        if !piece.atomic {
            continue;
        }

        if nodes.get(piece.node_index).is_some_and(node_is_restorable) {
            continue;
        }

        if !atomic_piece_is_recoverable(nodes, piece) {
            return false;
        }
    }

    true
}

/// Reports whether `node` is one a computed value can **restore**: a masked
/// passthrough ([`Raw`](InlineNode::Raw)) or a masked STEM expression
/// ([`Stem`](InlineNode::Stem)).
///
/// These are exactly the two node kinds the *same* extraction pass
/// (`Passthroughs::extract_from`)
/// masks before any substitution step runs — STEM being an implicit
/// passthrough, as [`Stem::value`](crate::inlines::Stem) documents — so each
/// stands in this module's match string as one
/// [`SPAN_PLACEHOLDER`](super::super::quotes) character, and each has a body
/// known at build time that `Passthroughs::restore_to` splices over that
/// placeholder once the steps have run.
///
/// Every restoring family admits both kinds. The `image:`/`icon:` family's
/// values are the only restored ones the **fold** re-processes —
/// [`PathResolver::web_path`](crate::parser::PathResolver::web_path) resolves
/// the target (and an interactive SVG's `fallback=`) into the `src`, and a
/// rendered STEM body always carries a backslash `web_path` would posixify
/// on a Windows-separator resolver — but that re-processing runs over the
/// restored ranges *masked*: `web_path` only ever sees the backslash-free
/// placeholder there, and the restore splices the body in afterwards. See
/// [`Image::restored_target_ranges`](crate::inlines::Image) and
/// [`ElementAttribute`]'s own restored ranges.
///
/// This is the cheap discriminant half of [`restorable_body`], which produces
/// those bytes: the two return `true`/`Some` for the same set, pinned by
/// `restorable_body_agrees_with_node_is_restorable`. A gate uses this one so
/// a range it is about to *reject* costs no rendering.
///
/// [`ElementAttribute`]: crate::attributes::ElementAttribute
pub(in crate::content::inline_builder) fn node_is_restorable(node: &InlineNode<'_>) -> bool {
    matches!(node, InlineNode::Raw { .. } | InlineNode::Stem(_))
}

/// The bytes a [`node_is_restorable`] node restores to — the very text
/// `Passthroughs::restore_to`
/// splices over the placeholder — or `None` for any other
/// node.
///
/// The invariant both callers rest on is that this returns **exactly what the
/// fold of that node emits**, so a value finished with these bytes reads the
/// same as the surrounding tree:
///
/// - a [`Raw`](InlineNode::Raw) leaf's body is its `value`, which the fold also
///   emits verbatim, so it is borrowed rather than rendered;
/// - a [`Stem`](InlineNode::Stem) leaf's body is [`fold_stem`]'s own output —
///   `render_styled` over the already-substituted `value`, with no attribute
///   list or id — which is the same call `PassthroughRestoreReplacer` makes for
///   a STEM entry. Sharing that one function is what keeps the restore and the
///   fold from drifting.
///
/// `renderer` is the **parser's** renderer, mirroring `restore_to`'s own
/// (`Passthroughs::restore_to` renders a STEM entry through `parser.renderer`
/// before splicing it into the rendered string). A computed target therefore
/// freezes its STEM bytes at build time to match what a `Stem` node standing
/// in the flow renders at fold time instead; the two agree whenever the fold
/// uses the parser's renderer, which is the only renderer seam `Content`
/// uses.
pub(in crate::content::inline_builder) fn restorable_body<'a>(
    node: &'a InlineNode<'_>,
    renderer: &dyn InlineRenderer,
) -> Option<Cow<'a, str>> {
    match node {
        InlineNode::Raw {
            value,
            form: RawForm::AsIs,
            ..
        } => Some(Cow::Borrowed(value.as_ref())),

        // An escaped-form body carries the author's logical text, so the bytes
        // spliced over the placeholder are that text
        // *escaped* — the same bytes this node's own fold emits, which is the
        // invariant this function exists to hold (see `node_is_restorable`).
        InlineNode::Raw {
            value,
            form: RawForm::Escaped,
            ..
        } => {
            let mut out = String::with_capacity(value.len());

            for ch in value.chars() {
                render_char(ch, renderer, &mut out);
            }

            Some(Cow::Owned(out))
        }

        InlineNode::Stem(stem) => {
            let mut out = String::new();
            fold_stem(stem, renderer, &mut out);
            Some(Cow::Owned(out))
        }

        _ => None,
    }
}

/// Builds one [`Image`](InlineNode::Image) node from a recognized image/icon
/// match, pre-extracting the alt/width/height up front so the fold
/// reproduces the same bytes.
///
/// # What must be verbatim, and what need not be
///
/// The macro name and the target are read from the level's own **match
/// string** (`whole`, and [`text_slice`] for the target) rather than from a
/// source slice, so both are exact even when they come from a
/// [`synthesized`](Piece::synthesized) run — an expanded attribute value
/// (`image:{logo}[Logo]`) or a filtered multi-line block's own joined seed —
/// or cross an **escaped special** (`image:a&b.png[]`, whose target reads as
/// `a&amp;b.png` out of the match string's own escaped haystack: the very
/// bytes this match string carries, and the ones
/// [`apply_image_side_effects`] registers). Only the node's `location` then
/// falls back to the enclosing piece's coarse span. A target crossing a
/// masked **passthrough** or **STEM** expression finishes into the restored
/// bytes instead — see
/// [`restore_masked_passthroughs`] and [`masked_default_alt`] — and the side
/// effect registers that honest restored value rather than the raw
/// placeholder bytes (see
/// `registers_the_restored_target_for_an_image_over_a_passthrough`).
///
/// The **attribute list** follows the same rule, one step removed. An
/// [`Attrlist`]`<'src>` reads its own `Span<'src>`'s bytes *as content*, not
/// merely as a location tag, so a bracket with no honest `'src` slice — one
/// crossing a [`synthesized`](Piece::synthesized) run
/// (`image:sunset.jpg[{caption}]`), an escaped special
/// (`image:x.png[a < b]`), or a restored entity (`image:x.png[Tom &amp;
/// Jerry]`) — cannot be parsed from the source. It is parsed from the **match
/// string** instead, through [`bracket_attrlist`]
/// (`Attrlist::parse(Span::new(&caps[2]), …)`, over the match string's own
/// escaped, already-expanded haystack); the resulting list is then
/// [`into_owned`](Attrlist::into_owned)ed off that temporary and tagged with
/// the enclosing piece's coarse span. A bracket that *is* verbatim keeps its
/// `'src` slice, so its attribute values still borrow.
fn build_image_node<'src>(
    caps: &regex::Captures<'_>,
    whole: &str,
    full: &std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
    nodes: &[InlineNode<'src>],
) -> InlineNode<'src> {
    let location = source_slice(pieces, full.clone(), root);

    // The macro name (past any — here absent — escape) is `image:` or `icon:`.
    // It is read from the match string rather than `location.data()`, which is
    // the coarse enclosing span for a macro reached through a synthesized run.
    let is_icon = !whole.starts_with("image:");

    // Group 1 is the target and group 2 the bracket text; both always
    // participate. The target's own pattern requires a first character and
    // makes only the *remainder* optional, so a target-less `image:[…]` is not
    // a match at all — which is why the fallback here is a degenerate stand-in
    // for the empty values an absent group would mean rather than a branch of
    // its own that no input can take, exactly as group 2's below is. Both are
    // written with `map_or` and an eagerly built default (neither allocates)
    // so no unreachable arm is left for a reader — or a coverage run — to
    // wonder about.
    let (target, restored_target_ranges) = caps.get(1).map_or(
        (CowStr::from(""), Vec::new()),
        // Borrowed from `'src` for a verbatim target, the expansion's
        // own exact bytes for a synthesized one. A target crossing an escaped
        // special has no `'src` slice at all — the source holds one character
        // where the match string holds an entity — so it falls back to the
        // match string's own bytes, which is what `text_slice` declines to
        // recover and precisely what `caps[1]` holds.
        // A target crossing a masked **passthrough** or **STEM** expression
        // finishes into the restored bytes — the node's own rendered body
        // substituted for its placeholder, the same rewrite
        // `Passthroughs::restore_to` performs on the rendered `src` — while
        // the `default_alt` *arithmetic* below stays on the bytes as
        // matched (see
        // [`masked_default_alt`]), and the node records which ranges of the
        // restored target came from a masked body, so the fold-time
        // `web_path` can keep them out of its own way (see
        // [`Image::restored_target_ranges`](crate::inlines::Image)).
        |m| match restore_masked_passthroughs(
            m.as_str(),
            &(m.start()..m.end()),
            nodes,
            pieces,
            parser.renderer.as_ref(),
        ) {
            Some((restored, ranges)) => (CowStr::from(restored), ranges),

            None => (
                text_slice(nodes, pieces, m.start()..m.end())
                    .unwrap_or_else(|| CowStr::from(m.as_str().to_string())),
                Vec::new(),
            ),
        },
    );

    let (bracket_text, bracket_range) = caps.get(2).map_or(("", full.end..full.end), |m| {
        (m.as_str(), m.start()..m.end())
    });

    let attrlist = bracket_attrlist(bracket_text, bracket_range, nodes, pieces, root, parser);

    // The default alt text derives from the target's basename, with `_`/`-`
    // read as spaces, running over the *masked* bytes (the haystack holds the
    // passthrough or STEM placeholder there), with the masked bodies restored
    // into whatever survives the arithmetic.
    let default_alt = caps.get(1).map_or_else(String::new, |m| {
        masked_default_alt(
            m.as_str(),
            &(m.start()..m.end()),
            nodes,
            pieces,
            parser.renderer.as_ref(),
        )
    });

    // Pre-extract the resolved alt/width/height into owned values, ending the
    // `&'src self`-tied borrows before the attribute list is moved into the
    // node (the same read-then-move shape [`attributes_of`] uses). An icon
    // carries a `size` rather than width/height, recomputed at fold time
    // from `attrs`.
    let (alt, width, height) = if is_icon {
        let alt = attrlist.named_attribute("alt").map_or(default_alt, |a| {
            normalize_text_lf_escaped_bracket(a.value())
        });

        (Some(CowStr::from(alt)), None, None)
    } else {
        let alt = attrlist
            .named_or_positional_attribute("alt", 1)
            .map_or(default_alt, |a| {
                normalize_text_lf_escaped_bracket(a.value())
            });

        let width = attrlist
            .named_or_positional_attribute("width", 2)
            .map(|a| CowStr::from(a.value().to_string()));

        let height = attrlist
            .named_or_positional_attribute("height", 3)
            .map(|a| CowStr::from(a.value().to_string()));

        (Some(CowStr::from(alt)), width, height)
    };

    InlineNode::Image(Image {
        is_icon,
        target,
        restored_target_ranges,
        alt,
        width,
        height,
        attrs: attrlist,
        location,
    })
}

/// Rewrites this level's match string so each masked **passthrough** or
/// **STEM** expression's placeholder becomes a three-byte token —
/// `\u{96}`*n*`\u{97}` — moving the pieces into the rewritten string's
/// coordinates (each [`node_is_restorable`] piece keeps its node, wider;
/// every other piece keeps its bytes).
///
/// This family alone needs the widening because [`INLINE_IMAGE_MACRO`]'s
/// target class is the one in this module that requires **two** characters
/// (`[^:\s\[\n][^\[\n]*?[^\s\[\n]`): a target written wholly inside a
/// passthrough (`image:++sunset.jpg++[]`) or a STEM expression
/// (`image:stem:[x][]`) is a single placeholder character, which that class
/// cannot match — where the three-byte token matches it exactly. Widening
/// the placeholder to that shape makes recognition agree byte-for-byte with
/// [`INLINE_IMAGE_MACRO`]'s own pattern without touching the shared regex.
/// (The `link:`/`mailto:` family's one-or-more target class never faced
/// this, so its increment left the placeholder bare.)
///
/// The token's bytes never reach an output node: an unmatched token sits in
/// a gap [`rebuild_macro_level`] re-emits from the piece's own *node*, a
/// matched one lies inside a computed value that
/// [`restore_masked_passthroughs`] or [`masked_default_alt`] substitutes the
/// node's own body over, and no match boundary can cut one (no byte of
/// `\u{96}`, a digit, `\u{97}` can begin or end an image match, whose ends
/// are the literal macro name and `]`). The numbering is per level and
/// exists only to keep tokens distinct across this level's own placeholders.
fn widen_masked_pieces(
    s: String,
    pieces: Vec<Piece>,
    nodes: &[InlineNode<'_>],
) -> (String, Vec<Piece>) {
    if !pieces
        .iter()
        .any(|piece| nodes.get(piece.node_index).is_some_and(node_is_restorable))
    {
        return (s, pieces);
    }

    let mut out = String::with_capacity(s.len() + 8);
    let mut out_pieces = Vec::with_capacity(pieces.len());

    // In `s` coordinates; the bytes between pieces (a `LevelContext` boundary
    // wrap) belong to no piece and are copied through as-is.
    let mut cursor = 0usize;
    let mut n = 0usize;

    for piece in pieces {
        let p_start = piece.s_start;
        let p_end = piece.s_start + piece.s_len;

        out.push_str(s.get(cursor..p_start).unwrap_or_default());

        let s_start = out.len();

        if nodes.get(piece.node_index).is_some_and(node_is_restorable) {
            out.push_str(&format!("\u{96}{n}\u{97}"));
            n += 1;
        } else {
            out.push_str(s.get(p_start..p_end).unwrap_or_default());
        }

        out_pieces.push(Piece {
            s_start,
            s_len: out.len().saturating_sub(s_start),
            ..piece
        });

        cursor = p_end;
    }

    out.push_str(s.get(cursor..).unwrap_or_default());

    (out, out_pieces)
}

/// This family's `default_alt` derivation —
/// `basename(&target.replace(['_', '-'], " "))` — performed over the
/// **masked** bytes, with each masked passthrough's or STEM expression's
/// body restored into whatever survives the arithmetic.
///
/// A masked construct sits in the haystack as the `\u{96}`*n*`\u{97}`
/// token: an opaque run carrying none of the bytes the arithmetic acts on
/// (no `_`/`-` for the replace, no `/` or `.` for [`basename`]'s stem cut),
/// so the derivation treats it as one indivisible run and the restore then
/// splices the extracted body over whatever token reaches the derived
/// `alt` — which is how `image:++a_b-c.jpg++[]` keeps `alt="a_b-c.jpg"`
/// where the verbatim spelling shows `a b c`, its underscores hidden from
/// the replace inside the token. Each overlapping
/// [`node_is_restorable`] piece's placeholder becomes that same
/// token, the arithmetic runs, and each *surviving* token is
/// restored with its own node's body ([`restorable_body`]) — index-keyed, as
/// `Passthroughs::restore_to` is,
/// so a token the basename cut dropped (a masked construct wholly inside a
/// directory prefix or an extension) does not shift the ones that survive. A
/// token survives whole or is dropped whole: both of the cut points
/// [`basename`] reads (the last `/`, the last `.`) are bytes no token contains.
///
/// A range holding no masked construct takes the plain derivation over the
/// match-string bytes, since `masked` here is the same value `caps[1]`
/// holds.
fn masked_default_alt(
    masked: &str,
    range: &std::ops::Range<usize>,
    nodes: &[InlineNode<'_>],
    pieces: &[Piece],
    renderer: &dyn InlineRenderer,
) -> String {
    let mut tokened = String::new();
    let mut values: Vec<Cow<'_, str>> = Vec::new();

    // In match-string coordinates; `masked` is indexed relative to
    // `range.start`.
    let mut cursor = range.start;

    for piece in pieces {
        let p_start = piece.s_start;
        let p_end = piece.s_start + piece.s_len;

        // Skip pieces that do not overlap the range.
        if p_end <= range.start || p_start >= range.end {
            continue;
        }

        let Some(value) = nodes
            .get(piece.node_index)
            .and_then(|node| restorable_body(node, renderer))
        else {
            continue;
        };

        // A masked piece is one placeholder character, atomic and never
        // sliced, so an overlapping one lies wholly inside the range and
        // `p_start`/`p_end` are safe bounds.
        tokened.push_str(
            masked
                .get(cursor.saturating_sub(range.start)..p_start.saturating_sub(range.start))
                .unwrap_or_default(),
        );
        tokened.push_str(&format!("\u{96}{n}\u{97}", n = values.len()));
        values.push(value);
        cursor = p_end;
    }

    if values.is_empty() {
        return basename(&masked.replace(['_', '-'], " "));
    }

    tokened.push_str(
        masked
            .get(cursor.saturating_sub(range.start)..)
            .unwrap_or_default(),
    );

    let derived = basename(&tokened.replace(['_', '-'], " "));

    // One left-to-right pass, like `Passthroughs::restore_to`'s own
    // `replace_all`: each token is sought only in the bytes after the
    // previous splice, so a restored body that itself carries
    // placeholder-shaped bytes can never be matched as a later token. Surviving
    // tokens appear in index order (they were emitted in piece order and the
    // arithmetic never reorders), and a dropped token is simply not found —
    // the ones after it still restore, keyed by their own index.
    let mut out = String::new();
    let mut rest = derived.as_str();

    for (n, value) in values.iter().enumerate() {
        let token = format!("\u{96}{n}\u{97}");

        if let Some(pos) = rest.find(&token) {
            out.push_str(rest.get(..pos).unwrap_or_default());
            out.push_str(value.as_ref());
            rest = rest.get(pos + token.len()..).unwrap_or_default();
        }
    }

    out.push_str(rest);
    out
}

/// Parses the macro's bracket into the [`Attrlist`]`<'src>` its node carries.
///
/// A **verbatim** bracket is parsed straight from its `'src` slice, so its
/// attribute names and values borrow from the source — the shape every
/// ordinary `image:x.png[Alt,200]` takes.
///
/// Any other bracket has no `'src` slice whose bytes are the attrlist text: a
/// [`synthesized`](Piece::synthesized) run holds the expansion's bytes where
/// the source holds `{caption}`, and a
/// [`CharRef`](InlineNode::CharRef) leaf holds one character (`<`) or the
/// entity as written (`&amp;copy;`) where the match string holds the escaped
/// or restored form (`&lt;`, `&copy;`). Those *match-string* bytes are exactly
/// the ones `InlineImageMacroReplacer` parses out of its own haystack, so this
/// parses the same bytes — from a [`Span::new`] over the capture, whose
/// `line`/`col`/`offset` are meaningless and never escape this function —
/// and [`into_owned`](Attrlist::into_owned)s the result onto the bracket's
/// coarse source span, the same fallback the node's `location`
/// takes.
///
/// An **empty** bracket carries no bytes either way and parses from a
/// zero-length slice of the macro's own span.
fn bracket_attrlist<'src>(
    bracket_text: &str,
    bracket_range: std::ops::Range<usize>,
    nodes: &[InlineNode<'_>],
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Attrlist<'src> {
    let bracket = source_slice(pieces, bracket_range.clone(), root);

    if bracket_range.is_empty() {
        // An empty attribute list carries no bytes to slice, so it parses from
        // a zero-length span wherever the macro sits.
        return parse_attrlist(bracket.slice(0..0), parser);
    }

    if range_is_verbatim(pieces, &bracket_range) {
        return parse_attrlist(bracket, parser);
    }

    let (tokened, masked) = tokened_bracket(
        bracket_text,
        &bracket_range,
        nodes,
        pieces,
        parser,
        Tokened::MaskedOrRendered,
    );

    let bodies: Vec<&str> = masked.iter().map(|piece| piece.body.as_ref()).collect();

    parse_attrlist(Span::new(&tokened), parser).into_owned_restoring(bracket, &bodies)
}

/// One masked piece a [`tokened_bracket`] token stands for: the node itself,
/// and the bytes it restores to.
///
/// The two are produced together, by the one
/// [`node_is_restorable`]/[`restorable_body`] chain, because a caller may need
/// either: the `image:`/`icon:` family splices the **body** into the parsed
/// attribute values ([`Attrlist::into_owned_restoring`]), while the link
/// families' display text — which becomes a node's *children* rather than a
/// string — splices the **node**, so the fold emits those same bytes without
/// re-escaping them. Pairing them here rather than re-deriving
/// one from the other is what keeps the two spellings of "what this token
/// restores to" from drifting.
pub(in crate::content::inline_builder) struct MaskedPiece<'a, 'src> {
    /// The node the token stands for — a [`Raw`](InlineNode::Raw) passthrough
    /// or a [`Stem`](InlineNode::Stem) expression, and (under
    /// [`Tokened::MaskedOrRendered`]) any other opaque piece.
    pub(in crate::content::inline_builder) node: &'a InlineNode<'src>,

    /// What the token restores to: exactly what the fold of
    /// [`node`](Self::node) emits (see [`restorable_body`]).
    ///
    /// For a *masked* construct those bytes are known at build time —
    /// `Passthroughs::restore_to` splices exactly these into its own finished
    /// string. For a **rendered** piece (admitted under
    /// [`Tokened::MaskedOrRendered`]) they are the build-time fold with the
    /// parser's own renderer; freezing them is what lets a
    /// slot a renderer writes out carry markup the author wrote, at the cost
    /// of a *later* fold with some other renderer seeing the parse-time
    /// renderer's bytes in that slot.
    pub(in crate::content::inline_builder) body: Cow<'a, str>,
}

/// Which pieces [`tokened_bracket`] gives a token to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::content::inline_builder) enum Tokened {
    /// Only a **masked** construct — a passthrough or a STEM expression —
    /// whose body `Passthroughs::restore_to` splices over its own placeholder.
    /// The bracket's parsed values may then be restored wherever they go.
    Masked,

    /// Also any other **opaque** piece: a rendered span, an
    /// earlier-recognized macro node. Its body is the build-time **fold** with
    /// the parser's own renderer, which is what lets a value
    /// carry markup an author wrote into a slot a renderer writes out (see
    /// [`MaskedPiece::body`]).
    MaskedOrRendered,
}

/// Rewrites a macro **bracket**'s own match-string bytes so each masked piece
/// in it becomes an index-keyed `\u{96}`*n*`\u{97}` token, returning that text
/// alongside the [`MaskedPiece`]s those tokens stand for.
///
/// This is the *before the split* half of every bracket restore, and it
/// exists so the text handed to [`Attrlist::parse`] holds the placeholder
/// token in place of each masked body's own bytes. Two spellings have to be
/// normalized into
/// one: [`widen_masked_pieces`] has already rewritten a masked piece to a
/// three-byte token for the image family's *recognition*, but its
/// numbering is per level, and for the link families' display-text list a
/// masked piece is still the bare one-character
/// [`SPAN_PLACEHOLDER`](super::super::quotes). Renumbering every restorable
/// piece from zero, per bracket, is what lets the restore be **index-keyed**
/// on the way back out ([`Attrlist::into_owned_restoring`]) — the parse can
/// drop a token (a blank slot, a value the split discards) without shifting
/// the ones that survive, exactly as
/// `Passthroughs::restore_to` is
/// unshifted by a placeholder that never reached the rendered string.
///
/// Shared by the two families whose bracket comes back from a parse — the
/// `image:`/`icon:` bracket here, and the link families' display-text list
/// ([`text_attrlist`](super::links)) — so the two cannot disagree about what
/// a token may stand for: both masked kinds, the set [`node_is_restorable`]
/// names. (The image bracket's one `web_path`-bound value, an interactive
/// SVG's `fallback=`, resolves over its restored ranges *masked* — see
/// [`ElementAttribute::into_owned_restoring`](crate::attributes::ElementAttribute) —
/// so a STEM body's backslash never reaches the resolver there either.)
pub(in crate::content::inline_builder) fn tokened_bracket<'a, 'src>(
    bracket_text: &str,
    range: &std::ops::Range<usize>,
    nodes: &'a [InlineNode<'src>],
    pieces: &[Piece],
    parser: &Parser,
    admits: Tokened,
) -> (String, Vec<MaskedPiece<'a, 'src>>) {
    let renderer = parser.renderer.as_ref();

    let mut tokened = String::new();
    let mut masked_pieces: Vec<MaskedPiece<'a, 'src>> = Vec::new();

    // Walked **piece by piece** rather than by copying the gaps between the
    // tokened ones, because a byte of the match string may belong to no piece
    // at all: `styled_sibling_boundaries` wraps an opaque span's placeholder
    // in the two characters its own rendering presents to a neighbour (the `<`
    // and `>` of a tag), which exist for *recognition* and stand for markup
    // the token already carries whole. Copying them would splice a stray `<`
    // and `>` into the parsed value beside the piece.
    for piece in pieces {
        let p_start = piece.s_start;
        let p_end = piece.s_start + piece.s_len;

        // Skip pieces that do not overlap the range.
        if p_end <= range.start || p_start >= range.end {
            continue;
        }

        let lo = p_start.max(range.start);
        let hi = p_end.min(range.end);

        // The discriminant and the body are one step: `restorable_body`
        // answers `Some` for exactly the nodes `node_is_restorable` admits
        // (pinned by `restorable_body_agrees_with_node_is_restorable`), so
        // gating on one before producing the other would leave a branch no
        // input can take. A piece this leaves untokened contributes its own
        // bytes: a `Text` run's, or a `CharRef` leaf's canonical entity.
        let masked = piece
            .atomic
            .then(|| nodes.get(piece.node_index))
            .flatten()
            .and_then(|node| {
                if let Some(body) = restorable_body(node, renderer) {
                    return Some(MaskedPiece { node, body });
                }

                if admits == Tokened::Masked || charref_entity(node).is_some() {
                    return None;
                }

                Some(MaskedPiece {
                    node,
                    body: Cow::Owned(fold_html(
                        std::slice::from_ref(node),
                        renderer,
                        &parser.render_context(),
                    )),
                })
            });

        match masked {
            Some(masked) => {
                tokened.push_str(&format!("\u{96}{n}\u{97}", n = masked_pieces.len()));
                masked_pieces.push(masked);
            }

            None => tokened.push_str(
                bracket_text
                    .get(lo.saturating_sub(range.start)..hi.saturating_sub(range.start))
                    .unwrap_or_default(),
            ),
        }
    }

    if masked_pieces.is_empty() {
        return (bracket_text.to_string(), masked_pieces);
    }

    (tokened, masked_pieces)
}

/// Parses one inline attribute list, discarding the warnings — the shared
/// spelling both of [`bracket_attrlist`]'s paths use.
fn parse_attrlist<'a>(source: Span<'a>, parser: &Parser) -> Attrlist<'a> {
    Attrlist::parse(source, parser, AttrlistContext::Inline)
        .item
        .item
}

/// Performs the recognition side effects an `image:`/`icon:` match needs —
/// registering the image target in the document's asset catalog (`image:`
/// only, and only when [`catalog_assets`](Parser::with_catalog_assets) is
/// enabled) and recording the `link=` dangerous-scheme/self-href warning —
/// by walking an already-built tree and reading each
/// [`Image`](InlineNode::Image) node's own stored fields instead of a regex
/// capture.
///
/// Every macro family this module recognizes defers exactly this kind of
/// side effect (see this file's own `register_image` note, and the anchor,
/// link, and footnote families' own): recognition runs once per level as the
/// tree is built, but a side effect must run exactly once per parse, so it is
/// replayed from the finished tree afterward, via
/// [`apply_macro_side_effects`](super::apply_macro_side_effects), rather than
/// performed inline as each family recognizes its construct. This function is
/// that replay for the image/icon family, and is also
/// exercised directly by this module's own tests, against their own `Parser`.
///
/// Recurses into every container an `Image` node can be nested inside —
/// [`Styled`](InlineNode::Styled), [`Ref`](InlineNode::Ref),
/// [`Footnote`](InlineNode::Footnote), and
/// [`IndexTerm`](InlineNode::IndexTerm) children, and an
/// [`Anchor`](InlineNode::Anchor)'s `reftext` — mirroring exactly where
/// [`apply_macros`](super::apply_macros) and the footnote increment's own
/// `emit_range` can place one. Both of the last two hold an image the image
/// pass had already recognized when the enclosing node was built
/// (`((a term with an image:t.png[T] inside))`,
/// `[[id,see image:t.png[T]]]`), which is precisely when a registration can
/// hide there.
///
/// The five are every nested node list an [`InlineNode`] holds: the four
/// `children` fields, and an [`Anchor`](InlineNode::Anchor)'s `reftext`, which
/// is one despite not being named like one. A sixth would be a new place a
/// macro node can hide, and the corpus-wide side-effect sweep
/// (`tests::inline_builder_side_effect_parity`) is what would catch one going
/// unwalked, as it caught `IndexTerm` and `reftext` in turn.
pub(crate) fn apply_image_side_effects(
    nodes: &[InlineNode<'_>],
    parser: &Parser,
    source: Span<'_>,
) {
    for node in nodes {
        match node {
            InlineNode::Image(image) => {
                if !image.is_icon {
                    parser.register_image(
                        image.target.to_string(),
                        parser
                            .attribute_value("imagesdir")
                            .as_maybe_str()
                            .map(str::to_owned),
                    );
                }

                if let Some(rejected) = rejected_link_target(image, parser) {
                    parser.record_substitution_warning(
                        source,
                        WarningType::UnsafeLinkSchemeRejected(rejected.to_owned()),
                    );
                }
            }

            InlineNode::Styled(styled) => {
                apply_image_side_effects(&styled.children, parser, source);
            }

            InlineNode::Ref(reference) => {
                apply_image_side_effects(&reference.children, parser, source);
            }

            InlineNode::Footnote(footnote) => {
                apply_image_side_effects(&footnote.children, parser, source);
            }

            InlineNode::IndexTerm(index_term) => {
                apply_image_side_effects(&index_term.children, parser, source);
            }

            InlineNode::Anchor(anchor) => {
                if let Some(reftext) = &anchor.reftext {
                    apply_image_side_effects(reftext, parser, source);
                }
            }

            _ => {}
        }
    }
}

/// Mirrors `InlineImageMacroReplacer::link_self_resolves_to_src`: whether
/// `link=self` on this image/icon node resolves to a real `src` the renderer
/// promotes into the anchor `href` (an icon has one only in image-icon mode —
/// icons enabled and not font-based).
fn link_self_resolves_to_src(image: &Image<'_>, parser: &Parser) -> bool {
    !image.is_icon
        || (parser.is_attribute_set("icons")
            && parser.attribute_value("icons").as_maybe_str() != Some("font"))
}

/// Mirrors `InlineImageMacroReplacer::replace_append`'s own `link=`
/// rejection check, returning the target string the renderer would refuse to
/// promote into an `href`, if any.
fn rejected_link_target<'a>(image: &'a Image<'_>, parser: &Parser) -> Option<&'a str> {
    let link = image.attrs.named_attribute("link")?;

    if link.value() == "self" {
        (link_self_resolves_to_src(image, parser)
            && has_dangerous_self_href(&image.target, is_uri_ish(&image.target)))
        .then_some(image.target.as_ref())
    } else {
        has_dangerous_scheme(link.value()).then_some(link.value())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use super::{
        super::{
            super::test_support::{
                assert_styled, assert_text, build_src, build_through_quotes, fold_html,
                golden_macros, golden_macros_in,
            },
            ComputedSpecials,
        },
        node_is_restorable, restorable_body,
    };
    use crate::{
        HasSpan, Parser, Span,
        content::inline_builder::{
            build, char_replacements::apply_character_replacements, macros::apply_macros,
            special_chars::Masked,
        },
        inlines::{CharRef, Image, InlineNode, RawForm, RawOrigin, SpanForm, StyleVariant},
        parser::{DefaultPathResolver, HtmlInlineRenderer},
        strings::CowStr,
    };

    #[test]
    fn fold_matches_the_string_pipeline_through_macros() {
        // For each fixture, folding the single-pass tree (all five steps)
        // reproduces the string pipeline's output byte-for-byte. This is the
        // differential corpus (design §5.3) that pins the image/icon increment.
        let fixtures = [
            // No macro despite macro-ish characters.
            "plain text",
            "a colon : and a bracket [ apart",
            "image without a bracket image:foo.png stays literal",
            // Images: empty, alt, defaulted alt, dimensions, named attrs.
            "image:sunset.jpg[]",
            "image:sunset.jpg[Sunset]",
            "image:sunset.jpg[Sunset Mountain]",
            "image:photo.png[Alt Text,200,100]",
            "image:photo.png[alt=Alt Text,width=200,height=100]",
            "image:a_b-c.png[]",
            "image:d/e/f.png[]",
            // The two shapes `INLINE_IMAGE_MACRO`'s target group decides (both
            // families share the pattern, so the rule reaches the tree
            // unchanged): a one-character target is a macro, and a *missing*
            // one is not a match at all and stays literal.
            "image:a[]",
            "image:a[Alt Text]",
            "icon:t[]",
            "image:[]",
            "image:[Alt Text]",
            "icon:[]",
            "See image:[Alt Text] here.",
            "image:logo.png[Logo,role=thumb]",
            "image:logo.png[title=Hover text]",
            "image:logo.png[link=https://example.org]",
            "image:logo.png[float=left]",
            // Icons.
            "icon:tags[]",
            "icon:home[Home]",
            "icon:home[size=2x]",
            // An icon's named `alt` attribute (its value normalized, escaped
            // brackets unescaped).
            "icon:home[alt=Home Page]",
            "icon:home[alt=a\\]b]",
            // A macro embedded in surrounding flow, and next to other constructs.
            "See image:sunset.jpg[Sunset] here.",
            "*bold* then image:x.png[X] and _em_",
            "before image:a.png[A] middle image:b.png[B] after",
            "a copyright (C) then image:x.png[X]",
            // Escapes: the macro stays literal, minus the backslash.
            "\\image:sunset.jpg[]",
            "\\image:sunset.jpg[Sunset]",
            "\\icon:home[]",
            // A macro inside a rendered span (recognized inside the span body).
            "*see image:x.png[X]*",
            "_image:y.png[Y] in em_",
        ];

        let renderer = HtmlInlineRenderer {};

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
    fn fold_matches_the_string_pipeline_with_document_context() {
        // The image/icon fold reads document attributes (an icon's `icons`
        // mode, an image's `imagesdir`), so the parity must hold under a
        // non-default document too. Build and fold with the *same* parser the
        // golden uses.
        use crate::parser::ModificationContext;

        let parser = Parser::default()
            .with_intrinsic_attribute("imagesdir", "assets/img", ModificationContext::Anywhere)
            .with_intrinsic_attribute("icons", "font", ModificationContext::Anywhere)
            .with_intrinsic_attribute("icontype", "svg", ModificationContext::Anywhere);

        let fixtures = [
            "image:sunset.jpg[Sunset]",
            "image:sub/dir/pic.png[Pic,320]",
            "icon:heart[]",
            "icon:heart[2x]",
            "icon:heart[size=lg,role=fav]",
            "text with icon:star[] inline",
        ];

        let renderer = HtmlInlineRenderer {};

        for fixture in fixtures {
            let folded = crate::content::inline_builder::fold_html(
                &build(Span::new(fixture), &parser, None),
                &renderer,
                &parser.render_context(),
            );

            assert_eq!(
                folded,
                golden_macros_in("macros_imagesdir", fixture, &parser),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    /// Asserts that `node` is an [`Image`](InlineNode::Image), returning it for
    /// further inspection.
    fn assert_image<'a, 'src>(node: &'a InlineNode<'src>) -> &'a Image<'src> {
        match node {
            InlineNode::Image(image) => image,

            other => panic!("expected Image, got {other:?}"),
        }
    }

    #[test]
    fn an_image_macro_becomes_a_self_describing_node() {
        let nodes = build_src(Span::new("image:sunset.jpg[Sunset]"));

        assert_eq!(nodes.len(), 1);
        let image = assert_image(&nodes[0]);

        assert!(!image.is_icon);

        // The target borrows from source (no allocation), and the alt is the
        // supplied positional value.
        assert!(matches!(image.target, CowStr::Borrowed(_)));
        assert_eq!(image.target.as_ref(), "sunset.jpg");
        assert_eq!(image.alt.as_deref(), Some("Sunset"));
        assert_eq!(image.width, None);
        assert_eq!(image.height, None);

        // The node captures its own attribute list — the property that makes it
        // self-describing (and unblocks a faithful fold).
        assert_ne!(
            image.attrs.attributes().len(),
            0,
            "the attribute list is retained"
        );

        // Its location covers the whole macro, delimiters included.
        assert_eq!(image.location.data(), "image:sunset.jpg[Sunset]");
        assert_eq!(image.location.line(), 1);
        assert_eq!(image.location.col(), 1);
    }

    #[test]
    fn image_dimensions_are_captured_positionally() {
        let nodes = build_src(Span::new("image:p.png[Alt,200,100]"));

        let image = assert_image(&nodes[0]);
        assert_eq!(image.alt.as_deref(), Some("Alt"));
        assert_eq!(image.width.as_deref(), Some("200"));
        assert_eq!(image.height.as_deref(), Some("100"));
    }

    #[test]
    fn image_default_alt_derives_from_the_basename() {
        // With no alt, the default is the target's basename with `_`/`-` read
        // as spaces and the extension dropped.
        let nodes = build_src(Span::new("image:a_b-c.png[]"));

        let image = assert_image(&nodes[0]);
        assert_eq!(image.alt.as_deref(), Some("a b c"));
    }

    #[test]
    fn an_icon_macro_becomes_an_icon_node() {
        let nodes = build_src(Span::new("icon:home[size=2x]"));

        let image = assert_image(&nodes[0]);
        assert!(image.is_icon);
        assert_eq!(image.target.as_ref(), "home");

        // An icon has no positional width/height; its `size` lives in the
        // attribute list (read back at fold time), and its default alt is the
        // target itself.
        assert_eq!(image.width, None);
        assert_eq!(image.height, None);
        assert_eq!(image.alt.as_deref(), Some("home"));
    }

    #[test]
    fn an_image_macro_is_recognized_inside_a_span() {
        // A macro can appear inside a rendered span; the transducer descends
        // into the span body and builds the node there.
        let nodes = build_src(Span::new("*see image:x.png[X]*"));

        let children = assert_styled(&nodes[0], StyleVariant::Strong, SpanForm::Constrained);
        assert_eq!(children.len(), 2);
        assert_text(&children[0], "see ", 1, 2);

        let image = assert_image(&children[1]);
        assert_eq!(image.target.as_ref(), "x.png");
        assert_eq!(image.alt.as_deref(), Some("X"));
    }

    #[test]
    fn an_escaped_image_macro_stays_literal() {
        // `\image:…` drops the backslash and keeps the macro as literal text —
        // no image node.
        let nodes = build_src(Span::new("\\image:sunset.jpg[Sunset]"));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Image(_))),
            "an escaped macro must not produce an image node: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_macros("\\image:sunset.jpg[Sunset]")
        );
    }

    #[test]
    fn an_escaped_macro_over_a_rendered_span_still_drops_its_backslash() {
        // The escape check runs *ahead* of the gate (the same check-order fix
        // the `footnoteref:`, menu, cross-reference, and link families made),
        // so a macro the gate would reject still honors its own escape:
        // dropping the backslash leaves the rest as its own nodes, which fold
        // back to exactly the bytes `caps[0][1..]` emits.
        let source = "\\image:x.png[*bold*]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Image(_))),
            "an escaped macro must not produce an image node: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_a_target_crossing_an_escaped_special() {
        // The string pipeline matches macros over *escaped* text, so a target
        // containing `&` is matched as `a&amp;b.png`. Those entity bytes are
        // exactly what this level's match string carries, so the node's target
        // is read off it and the fold reproduces the same `src`/default alt —
        // the escaped special being the one atomic piece
        // `range_has_no_opaque_piece` admits.
        let fixtures = [
            // A target crossing each of the three specials, alone and doubled.
            "image:a&b.png[]",
            "image:a<b.png[]",
            "image:a>b.png[]",
            "image:a&b&c.png[]",
            "icon:a&b[]",
            // A verbatim attribute list beside such a target: positional alt,
            // positional width/height, and named attributes (including the
            // `link=` forms `apply_image_side_effects` reads).
            "image:a&b.png[Alt]",
            "image:a&b.png[Alt,200,100]",
            "image:a&b.png[alt=Alt Text,role=thumb]",
            "image:a&b.png[link=self]",
            "image:a&b.png[alt=X,link=javascript:alert(1)]",
            // In surrounding flow, doubled, inside a rendered span, and beside
            // a sibling family that takes the same lift.
            "before image:a&b.png[A] after",
            "image:a&b.png[] image:c&d.png[]",
            "*image:a&b.png[A]*",
            "image:a&b.png[Alt] and link:x&y.html[]",
            // A special the target's own character classes reject (a newline
            // is impossible, but a space ends the target), so neither pipeline
            // builds a macro.
            "image:a & b.png[]",
            // The escape still keeps the macro literal, backslash dropped.
            "\\image:a&b.png[A]",
            // A special *beside* the macro rather than inside it.
            "image:sunset.jpg[]&amp;",
        ];

        let renderer = HtmlInlineRenderer {};

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
    fn a_target_crossing_an_escaped_special_carries_the_entity_bytes() {
        // The node's `target` is the string replacer's own `caps[1]` — the
        // escaped haystack's bytes, not the source's single `&` — which is
        // also what `apply_image_side_effects` registers. The default alt
        // derives from that same string, exactly as `default_alt` does.
        let source = "image:a&b.png[]";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        let image = assert_image(&nodes[0]);

        assert_eq!(image.target.as_ref(), "a&amp;b.png");
        assert_eq!(image.alt.as_deref(), Some("a&amp;b"));

        // The whole macro's span is still precise: neither the match's start
        // (`i`, or a backslash) nor its end (`]`) can fall inside an entity,
        // so no boundary lands in an atomic piece.
        assert_eq!(image.location.data(), source);
        assert_eq!(image.location.line(), 1);
        assert_eq!(image.location.col(), 1);
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_an_attribute_list_crossing_an_escaped_special() {
        // The bracket has no `'src` slice here — the source holds one
        // character where the match string holds an entity — so it is parsed
        // from the match string, which is the very `caps[2]` the string
        // replacer parses out of its own escaped haystack, and owned off that
        // temporary. Every attribute the two read is therefore the same.
        let fixtures = [
            // A special in a positional alt, in a named value, and in both a
            // named value and the target at once.
            "image:x.png[a < b]",
            "image:x.png[alt=a & b]",
            "image:a&b.png[a < b]",
            // Every special, and more than one in the same bracket.
            "image:x.png[a > b]",
            "image:x.png[a < b & c > d]",
            // Beside the positional width/height, and with a role.
            "image:x.png[a & b,200,100]",
            "image:x.png[a & b,role=thumb]",
            // The `icon:` spelling, whose size is read back from `attrs` at
            // fold time rather than pre-extracted.
            "icon:home[a & b]",
            "icon:home[2x,role=a & b]",
            // In surrounding flow, inside a rendered span, and doubled.
            "before image:x.png[a < b] after",
            "*image:x.png[a < b]*",
            "image:x.png[a & b] image:y.png[c & d]",
            // The escape still keeps the macro literal, backslash dropped.
            "\\image:x.png[a < b]",
        ];

        let renderer = HtmlInlineRenderer {};

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
    fn an_attribute_list_crossing_an_escaped_special_is_owned_and_coarsely_located() {
        // The parsed values are the *escaped* ones the string replacer reads
        // (`a &lt; b`, not `a < b`), they own their bytes rather than
        // borrowing from the temporary they were parsed from, and the list's
        // own span falls back to the bracket's coarse source range (design
        // §4.4) — the same split the node's `location` already takes for a
        // synthesized run.
        let source = "image:x.png[a < b,role=hl]";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        let image = assert_image(&nodes[0]);

        assert_eq!(image.alt.as_deref(), Some("a &lt; b"));

        let attrs = &image.attrs;
        assert_eq!(attrs.nth_attribute(1).unwrap().value(), "a &lt; b");
        assert_eq!(attrs.named_attribute("role").unwrap().value(), "hl");

        // The location tag is the bracket's own source text, which is *not*
        // what was parsed — the coarse fallback, kept honest as a location.
        assert_eq!(attrs.span().data(), "a < b,role=hl");

        // The whole macro's span is still precise: neither the match's start
        // nor its end can fall inside an entity.
        assert_eq!(image.location.data(), source);
        assert_eq!(image.location.line(), 1);
        assert_eq!(image.location.col(), 1);
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_a_target_crossing_a_restored_entity() {
        // An author-written entity (`&amp;copy;`, `&amp;#8217;`) is escaped by
        // `SpecialCharacters` and then *restored* by `CharacterReplacements`
        // into a `CharRef::Entity` leaf whose value is the entity itself. Those
        // bytes are what the string pipeline's own haystack carries from the
        // replacements step onward, and what the fold emits verbatim, so
        // `range_has_no_opaque_piece` admits the leaf exactly as it admits an
        // escaped special.
        let fixtures = [
            // A target crossing a restored entity — the `&amp;` spelling of
            // `&`, a named entity, and a numeric one.
            "image:a&amp;b.png[]",
            "image:&lt;.png[]",
            "image:a&copy;b.png[]",
            "image:a&#8217;b.png[]",
            "icon:a&copy;b[]",
            // Beside a verbatim attribute list, and doubled.
            "image:a&copy;b.png[Alt]",
            "image:a&copy;b.png[Alt,200,100]",
            "image:a&copy;b&reg;c.png[]",
            // A target crossing *both* a restored entity and an escaped
            // special, which only the match string carries the bytes of.
            "image:a&copy;b&c.png[]",
            // In surrounding flow, inside a rendered span, and beside a
            // sibling family that takes the same lift.
            "before image:a&copy;b.png[A] after",
            "*image:a&copy;b.png[A]*",
            "image:a&copy;b.png[Alt] and link:x&reg;y.html[]",
            // The escape still keeps the macro literal, backslash dropped.
            "\\image:a&copy;b.png[A]",
            // An entity *beside* the macro rather than inside it.
            "&copy;image:sunset.jpg[]&reg;",
        ];

        let renderer = HtmlInlineRenderer {};

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
    fn a_target_crossing_a_restored_entity_carries_the_entity_bytes() {
        // As for an escaped special, the node's `target` is the string
        // replacer's own `caps[1]` — here the *restored* entity's bytes, which
        // are also the source's own — and the default alt derives from that
        // same string.
        let source = "image:a&copy;b.png[]";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        let image = assert_image(&nodes[0]);

        assert_eq!(image.target.as_ref(), "a&copy;b.png");
        assert_eq!(image.alt.as_deref(), Some("a&copy;b"));

        // The whole macro's span is still precise: neither the match's start
        // nor its end can fall inside an entity.
        assert_eq!(image.location.data(), source);
        assert_eq!(image.location.line(), 1);
        assert_eq!(image.location.col(), 1);
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_an_attribute_list_crossing_a_restored_entity() {
        // A restored entity takes the same lift as an escaped special, for the
        // same reason: the source holds `&amp;copy;` where the match string
        // holds `&copy;`, so the bracket has no `'src` slice — and the match
        // string's bytes are the ones the string replacer parses.
        let fixtures = [
            "image:x.png[Tom &amp; Jerry]",
            "image:x.png[alt=a &copy; b]",
            "image:x.png[a &#8217; b]",
            // An entity in the bracket *and* in the target.
            "image:a&copy;b.png[Tom &amp; Jerry]",
            // An entity and an escaped special in the same bracket.
            "image:x.png[a &copy; b < c]",
            // Beside the positional width/height, and the `icon:` spelling.
            "image:x.png[Tom &amp; Jerry,200,100]",
            "icon:home[Tom &amp; Jerry]",
            // In surrounding flow and inside a rendered span.
            "before image:x.png[Tom &amp; Jerry] after",
            "*image:x.png[Tom &amp; Jerry]*",
            // The escape still keeps the macro literal, backslash dropped.
            "\\image:x.png[Tom &amp; Jerry]",
        ];

        let renderer = HtmlInlineRenderer {};

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
    fn fold_matches_the_string_pipeline_for_a_macro_crossing_a_character_replacement() {
        // A typographic replacement (`(C)`, `(R)`, `'`, `...`) is the third
        // recoverable piece, admitted for the same reason the two `CharRef`
        // entity leaves are: `build_match_string` gives it the entity the
        // built-in backend renders it as, which is what the string pipeline's
        // own haystack carries from the replacements step onward — so both the
        // target read off that string and the bracket parsed from it are the
        // string replacer's own bytes.
        let fixtures = [
            // A target crossing one, alone and beside an attribute list.
            "image:a(C)b.png[]",
            "image:a(C)b.png[Alt]",
            "icon:a(C)b[]",
            // An **attribute list** crossing one — the shape three real
            // fixtures in this crate's own corpora write.
            "image:pause.png[title=Pause (C) Resume]",
            "image:x.png[A tiger's roar]",
            "image:x.png[alt=a (C) b]",
            "image:x.png[Wait...]",
            "image:x.png[Tom (C) Jerry,200,100]",
            "icon:home[Tom (C) Jerry]",
            // A replacement in the bracket *and* in the target, and one
            // beside an escaped special and a restored entity.
            "image:a(C)b.png[Tom (C) Jerry]",
            "image:x.png[a (C) b < c &copy; d]",
            // In surrounding flow, inside a rendered span, doubled, and beside
            // a sibling family that takes the same lift.
            "before image:x.png[Tom (C) Jerry] after",
            "*image:x.png[Tom (C) Jerry]*",
            "image:a(C)b.png[] image:c(R)d.png[]",
            "image:x.png[Tom (C) Jerry] and link:x(R)y.html[]",
            // The escape still keeps the macro literal, backslash dropped.
            "\\image:x.png[Tom (C) Jerry]",
            // A replacement *beside* the macro rather than inside it.
            "(C)image:sunset.jpg[](R)",
        ];

        let renderer = HtmlInlineRenderer {};

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
    fn an_attribute_list_crossing_a_character_replacement_reads_the_rendered_entity() {
        // The structural companion: the bracket has no `'src` slice (the
        // source holds `(C)` where the match string holds `&#169;`), so it is
        // parsed from the match string and owned onto design §4.4's coarse
        // span — carrying the already-substituted value the string replacer
        // parses, entity and all.
        let source = "image:x.png[title=Pause (C) Resume]";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        let image = assert_image(&nodes[0]);

        let attrlist = &image.attrs;
        assert_eq!(
            attrlist.named_attribute("title").unwrap().value(),
            "Pause &#169; Resume"
        );

        assert_eq!(image.location.data(), source);
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_a_bracket_over_a_rendered_span() {
        // The boundary this family used to keep, now lifted for the half that
        // could not survive it: an image's **bracket** crossing a rendered
        // span. See [`bracket_is_recognizable`] for why a frozen span is both
        // necessary and safe — the rule the link families now share.
        for source in [
            // The fixture from the AsciiDoc language docs that named this.
            "Click image:pause.png[title=*Pause* and Resume] when you need a break.",
            // The span in the positional `alt`, alone and beside plain text.
            "image:x.png[*bold*]",
            "image:x.png[*Alt* text]",
            "image:x.png[text *Alt*]",
            // In a named value, with a plain positional beside it.
            "image:x.png[alt,title=a `code` b]",
            "image:x.png[alt,title=*T*,role=hl]",
            // Other span kinds, and two spans in one bracket.
            "image:x.png[_em_ and #mark#]",
            "image:x.png[alt,title=*a* and _b_]",
            // The icon spelling, which shares this bracket.
            "icon:home[title=*T*]",
            // (A masked passthrough beside a rendered span — both token kinds
            // in one bracket — is driven by the whole-pipeline sweep in the
            // parent module instead: `golden_macros` runs the six steps
            // `build` runs and *not* passthrough extraction, so a `$$…$$`
            // reaches it undelimited on one side and extracted on the other,
            // for a reason that has nothing to do with this boundary.)
            // Already at parity, unchanged.
            "image:x.png[Alt Text]",
            "image:x.png[alt,200,100]",
        ] {
            assert_eq!(
                fold_html(&build_src(Span::new(source)), &HtmlInlineRenderer {}),
                golden_macros(source),
                "fold diverged from the string pipeline for {source:?}"
            );
        }
    }

    #[test]
    fn a_target_over_a_rendered_span_is_a_documented_divergence() {
        // The half that stays: a rendered span in the **target**.
        // `build_match_string` stands it in as one `SPAN_PLACEHOLDER`, and the
        // target's own [`range_is_restorable`] still rejects it — a rendered
        // span's markup exists only at fold time, unlike a masked
        // passthrough's known body, and a target is not a value the string
        // replacer reads back out of its own rendered haystack the way a
        // bracket is: it is resolved as a *path* (`web_path`), where splicing
        // markup in has no meaning.
        //
        // If this boundary is ever lifted, fold this fixture into the parity
        // corpus above.
        let source = "image:a**b**c.png[]";

        let nodes = apply_macros(
            build_through_special_and_replacements(Span::new(source)),
            Span::new(source),
            &Parser::default(),
            Masked::UNKNOWN,
            ComputedSpecials::Escaped,
        );

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Image(_))),
            "a target crossing a rendered span must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* build an image here.
        assert!(golden_macros(source).contains("<img"));
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_an_image_target_over_a_passthrough() {
        // The differential corpus for an `image:`/`icon:` target crossing a
        // masked **passthrough** — the string pipeline swallows the
        // `\u{96}`*n*`\u{97}` sentinel into the target (the widened match
        // string carries the same bytes, see [`widen_masked_pieces`])
        // and the restore pass then splices the extracted body over every
        // sentinel in the rendered string, so the tree's computed target
        // substitutes the `Raw` node's value for its placeholder the same
        // way, and the `default_alt` *arithmetic* runs over the masked bytes
        // first (see [`masked_default_alt`]).
        use super::super::super::test_support::golden_passthroughs;

        let fixtures = [
            // The double-plus idiom, bare-bracketed and with an alt — and the
            // default-alt arithmetic over the sentinel: the underscores and
            // the extension hide inside it, so the whole restored body is the
            // alt (`alt="a_b-c.jpg"`), where the verbatim spelling shows
            // `a b c`.
            "image:++sunset.jpg++[Alt]",
            "image:++a_b-c.jpg++[]",
            // A `pass:[…]` target (a space-carrying body has its own test —
            // `a_space_restored_into_an_image_target_stays_out_of_web_paths_way`).
            "image:pass:[chart,v2.png][]",
            "image:pass:[chart,v2.png][Chart,200]",
            // A passthrough covering only part of the target, at either edge
            // and in the middle: the arithmetic sees the verbatim bytes
            // around an opaque token, so a visible extension still comes off,
            // a visible underscore still reads as a space, and a `/` hidden
            // inside the token hides from the stem cut in both pipelines.
            "image:++dir_name/++photo.png[]",
            "image:a_++b++_c.png[]",
            "image:shot++2++.png[]",
            // A URI target: both pipelines see a URI-ish haystack (the
            // scheme sits before the token) and preserve it verbatim.
            "image:https://++example.org/x++.png[]",
            // `web_path`'s own `..` arithmetic consumes the masked segment,
            // so the token never reaches the resolved path and its body is
            // dropped — in both pipelines (the string pipeline's restore
            // cannot find the sentinel either).
            "image:++dropped++/../kept.png[]",
            // The icon form, which derives its default alt the same way.
            "icon:++a_b++[]",
            // A triple-plus passthrough (no substitutions on the body).
            "image:+++a_b+++.png[T]",
            // Inside a rendered span, and two in one flow.
            "*a image:++x_y++.png[] b*",
            "image:++a,b++[A] then image:pass:[c=d.png][C]",
            // Escaped: the backslash drops and the rest stays literal — the
            // passthrough's own restore still applies, in both pipelines.
            "\\image:++sunset.jpg++[Alt]",
        ];

        let renderer = HtmlInlineRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_src(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden_passthroughs(fixture),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn an_image_target_over_a_passthrough_is_recognized() {
        // The target is the restored bytes; the default alt is the masked
        // derivation with the surviving sentinel restored — the whole body,
        // underscores, hyphens, and extension intact, since all of them hide
        // from the arithmetic inside the sentinel.
        let nodes = build_src(Span::new("image:++a_b-c.jpg++[]"));

        let image = assert_image(&nodes[0]);
        assert!(!image.is_icon);
        assert_eq!(image.target.as_ref(), "a_b-c.jpg");
        assert_eq!(image.alt.as_deref(), Some("a_b-c.jpg"));

        // A visible extension still comes off, and a token the stem cut
        // drops (the directory prefix) does not shift the one that survives.
        let nodes = build_src(Span::new("image:++dir_1++/++file_2++.png[]"));

        let image = assert_image(&nodes[0]);
        assert_eq!(image.target.as_ref(), "dir_1/file_2.png");
        assert_eq!(image.alt.as_deref(), Some("file_2"));
    }

    #[test]
    fn a_restored_body_carrying_sentinel_shaped_bytes_is_not_re_matched() {
        // The default-alt restore is one left-to-right pass, like
        // `Passthroughs::restore_to`'s own `replace_all`: a passthrough body
        // that itself contains the bytes of a *later* token
        // (`\u{96}1\u{97}` here) must not have that later token spliced into
        // it — each token is sought only after the previous splice.
        let source = "image:++x\u{96}1\u{97}y++_++c_d++[]";
        let nodes = build_src(Span::new(source));

        let image = assert_image(&nodes[0]);
        assert_eq!(image.target.as_ref(), "x\u{96}1\u{97}y_c_d");
        assert_eq!(image.alt.as_deref(), Some("x\u{96}1\u{97}y c_d"));
    }

    #[test]
    fn a_dangerous_target_inside_a_passthrough_is_a_documented_divergence() {
        // The one place this increment chooses the safe reading over byte
        // parity, mirroring the link family's own passthrough increment: the
        // renderer's `link=self` dangerous-target check runs over the node's
        // *restored* target, where the string pipeline's renderer checks the
        // sentinel it matched — through which a smuggled `javascript:` target
        // passes, the restore then completing a live link around the image in
        // the golden output. The tree's fold rejects it instead (the image
        // renders without the wrapping anchor), pinned here rather than by
        // the corpus above.
        use super::super::super::test_support::golden_passthroughs;

        let source = "image:++javascript:alert(1)++[link=self]";
        let nodes = build_src(Span::new(source));

        let image = assert_image(&nodes[0]);
        assert_eq!(image.target.as_ref(), "javascript:alert(1)");

        let golden = golden_passthroughs(source);

        assert!(
            golden.contains("href=\"javascript:"),
            "expected the documented divergence to still reproduce: {golden:?}"
        );

        let folded = fold_html(&nodes, &HtmlInlineRenderer {});

        assert!(
            !folded.contains("href="),
            "the fold must not emit the live link: {folded:?}"
        );
    }

    #[test]
    fn a_space_restored_into_an_image_target_stays_out_of_web_paths_way() {
        // Formerly this module's own documented divergence: the fold-time
        // `web_path` used to run over the node's *restored* target, so a
        // space the passthrough smuggled past the target class was
        // percent-encoded into the `src` where the string pipeline
        // normalized its space-free sentinel and spliced the raw space in
        // afterwards. The masked-resolve order closed it — `render_image`
        // resolves the `src` with the node's
        // [`restored_target_ranges`](Image) masked
        // to the same sentinel shape and splices the bodies back in, so the
        // space never reaches `web_path` in either pipeline.
        use super::super::super::test_support::golden_passthroughs;

        let source = "image:pass:[My Documents/chart.png][]";
        let nodes = build_src(Span::new(source));

        let image = assert_image(&nodes[0]);
        assert_eq!(image.target.as_ref(), "My Documents/chart.png");
        assert_eq!(image.restored_target_ranges, vec![0..22]);

        let golden = golden_passthroughs(source);

        assert!(
            golden.contains("src=\"My Documents/chart.png\""),
            "expected the raw space in the golden src: {golden:?}"
        );

        assert_eq!(fold_html(&nodes, &HtmlInlineRenderer {}), golden);
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_an_image_bracket_over_a_passthrough() {
        // The differential corpus for an `image:`/`icon:` **bracket**
        // crossing a masked passthrough. The bracket comes back from a
        // *parse*, so the restore is the one the string pipeline performs:
        // `Attrlist::parse` reads the `\u{96}`*n*`\u{97}` sentinel as one
        // opaque run — carrying none of the `,`/`=`/`"` bytes the split
        // reads — and the restore pass splices each body over whatever
        // sentinel reached the rendered string. `tokened_bracket` puts the
        // match string into that same shape and
        // `Attrlist::into_owned_restoring` performs the after-the-split
        // half.
        use super::super::super::test_support::golden_passthroughs;

        let fixtures = [
            // The plain alt, whole and partial, and a body whose own
            // formatting characters stay literal inside the passthrough.
            "image:sunset.jpg[++Alt text++]",
            "image:x.png[a ++b_c__d++ e]",
            "image:x.png[++a++ and ++b++]",
            // The split invariant: a `,` or an `=` inside the body must not
            // divide the list, because the string pipeline's own parse never
            // sees it. These are the fixtures a restore-*then*-parse fails.
            "image:x.png[++a,b++]",
            "image:x.png[++a=b++]",
            r#"image:x.png["++q,r++"]"#,
            // Named values, and the positional width/height slots.
            "image:x.png[title=++t_t__t++]",
            "image:x.png[Sunset,role=++hl++]",
            "image:x.png[alt,++100++,50]",
            "image:x.png[++a++,++200++]",
            // Shorthand: the `#id`/`.role` the scan finds sit *after* a
            // token, so their offsets have to shift with the restore while
            // the items themselves stay the ones the string pipeline found.
            "image:x.png[++abc++#myid]",
            "image:x.png[++abc++.myrole]",
            "image:x.png[++a b++.myrole#myid]",
            // A restored `&` passes through `encode_attribute_value`
            // untouched in both pipelines (only `"` is encoded — see the
            // quote divergence below).
            "image:x.png[++A & B++]",
            // A non-restorable atomic piece — a restored entity, which the
            // match string gives real bytes to — beside a masked one in the
            // same bracket: the gate admits both, and only the masked piece
            // is tokened.
            "image:x.png[Tom &amp; Jerry ++and co++]",
            // The other passthrough spellings.
            "image:x.png[pass:[a,b]]",
            "image:x.png[+++a_b+++]",
            // An attribute reference hidden inside the body: neither
            // pipeline expands it, since both parse the masked text.
            "image:x.png[++{name}++]",
            // Target and bracket both over passthroughs.
            "image:++s.png++[++alt++]",
            // The icon form, whose alt comes from the same bracket.
            "icon:home[++Home++]",
            // Inside a rendered span, two in one flow, and escaped.
            "*a image:x.png[++alt++] b*",
            "image:x.png[++A++] then image:y.png[++B,C++]",
            "\\image:x.png[++alt++]",
        ];

        let renderer = HtmlInlineRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_src(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden_passthroughs(fixture),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn an_image_bracket_over_a_passthrough_is_recognized() {
        // The parsed values carry the *restored* bytes, owned off the
        // temporary the parse read and tagged with the bracket's own coarse
        // span (design §4.4), exactly as every other non-verbatim bracket is.
        let source = "image:x.png[++Alt text++,++100++,50]";
        let nodes = build_src(Span::new(source));

        let image = assert_image(&nodes[0]);
        assert_eq!(image.target.as_ref(), "x.png");
        assert_eq!(image.alt.as_deref(), Some("Alt text"));
        assert_eq!(image.width.as_deref(), Some("100"));
        assert_eq!(image.height.as_deref(), Some("50"));

        // A body carrying the split's own delimiters stays inside one value.
        let nodes = build_src(Span::new("image:x.png[++a,b=c++]"));
        let image = assert_image(&nodes[0]);
        assert_eq!(image.alt.as_deref(), Some("a,b=c"));

        // The shorthand items keep pointing at the same characters after the
        // restore lengthens the value ahead of them.
        let nodes = build_src(Span::new("image:x.png[++abc++.myrole#myid]"));
        let image = assert_image(&nodes[0]);
        assert_eq!(image.alt.as_deref(), Some("abc.myrole#myid"));

        let attrlist = &image.attrs;

        assert_eq!(attrlist.id(), Some("myid"));
        assert_eq!(attrlist.roles(), vec!["myrole"]);
    }

    #[test]
    fn a_bracket_body_carrying_sentinel_shaped_bytes_is_not_re_matched() {
        // The bracket restore is one left-to-right pass, like
        // `Passthroughs::restore_to`'s own: a body that itself contains the
        // bytes of a *later* token must not have that token spliced into it,
        // and a token index the bracket never issued is left as written
        // rather than renumbering the ones after it.
        let source = "image:x.png[++x\u{96}1\u{97}y++ ++b++]";
        let nodes = build_src(Span::new(source));

        let image = assert_image(&nodes[0]);
        assert_eq!(image.alt.as_deref(), Some("x\u{96}1\u{97}y b"));

        // The leniency the index-keyed restore rests on, in both spellings a
        // bracket can present: a run that is not a well-formed token (no
        // digits) and one whose index the bracket never issued are each left
        // exactly as the author wrote them, rather than renumbering — or
        // consuming — the real tokens around them.
        let nodes = build_src(Span::new(
            "image:x.png[++a++ \u{96}x\u{97} \u{96}9\u{97} ++b++]",
        ));
        let image = assert_image(&nodes[0]);

        assert_eq!(
            image.alt.as_deref(),
            Some("a \u{96}x\u{97} \u{96}9\u{97} b")
        );

        // The string pipeline reads this one differently, and the difference
        // is its own wart rather than something to reproduce: `restore_to`
        // is a `replace_all` over the *finished* rendered string, so it also
        // rewrites the sentinel-shaped bytes the author wrote — splicing
        // passthrough 1's body into the middle of passthrough 0's. The tree
        // restores per token, into the value each token actually stands in,
        // so an author's own bytes survive. Its sibling
        // `a_restored_body_carrying_sentinel_shaped_bytes_is_not_re_matched`
        // pins the same reading for a target.
        use super::super::super::test_support::golden_passthroughs;

        assert!(
            golden_passthroughs(source).contains(r#"alt="xby b""#),
            "expected the documented divergence to still reproduce"
        );
    }

    #[test]
    fn a_dangerous_link_inside_a_bracket_passthrough_is_a_documented_divergence() {
        // The bracket's own version of
        // `a_dangerous_target_inside_a_passthrough_is_a_documented_divergence`,
        // and the same safe reading. The renderer's dangerous-scheme check
        // reads the `link=` attribute, which now carries the *restored*
        // bytes; the string pipeline's renderer checks the sentinel its own
        // parse put there, so a smuggled `javascript:` passes and its restore
        // pass then completes a live anchor around the image.
        use super::super::super::test_support::golden_passthroughs;

        let source = "image:x.png[Alt,link=++javascript:alert(1)++]";
        let nodes = build_src(Span::new(source));

        let image = assert_image(&nodes[0]);

        assert_eq!(
            image.attrs.named_attribute("link").unwrap().value(),
            "javascript:alert(1)"
        );

        let golden = golden_passthroughs(source);

        assert!(
            golden.contains("href=\"javascript:"),
            "expected the documented divergence to still reproduce: {golden:?}"
        );

        let folded = fold_html(&nodes, &HtmlInlineRenderer {});

        assert!(
            !folded.contains("href="),
            "the fold must not emit the live link: {folded:?}"
        );
    }

    #[test]
    fn a_quote_restored_into_an_image_bracket_is_a_documented_divergence() {
        // The one shape the bracket restore does not reach byte-for-byte,
        // and the same well-formed reading the two link families' own
        // restores take for a `"` in a target. The string pipeline encodes
        // its quote-free *sentinel* into `alt="…"` and the restore pass then
        // splices the raw `"` into the finished attribute, closing it; the
        // tree holds the restored bytes as the node's own `alt`, so the
        // fold's `encode_attribute_value` escapes the quote and the
        // attribute stays well formed.
        use super::super::super::test_support::golden_passthroughs;

        let source = r#"image:x.png[++a"b++]"#;
        let nodes = build_src(Span::new(source));

        let image = assert_image(&nodes[0]);
        assert_eq!(image.alt.as_deref(), Some(r#"a"b"#));

        let golden = golden_passthroughs(source);

        assert!(
            golden.contains(r#"alt="a"b""#),
            "expected the documented divergence to still reproduce: {golden:?}"
        );

        assert!(
            fold_html(&nodes, &HtmlInlineRenderer {}).contains(r#"alt="a&quot;b""#),
            "the fold must emit the encoded quote"
        );
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_an_image_bracket_over_a_stem_expression() {
        // The bracket admits a masked STEM expression exactly as it admits a
        // masked passthrough — the two are one extraction pass's node kinds
        // ([`node_is_restorable`]) — tokened through the parse and restored
        // into the parsed values, with a `Stem` piece's body coming from its
        // own fold ([`restorable_body`]). Formerly this family's documented
        // deferral, blocked on the bracket's one `web_path`-bound value (an
        // interactive SVG's `fallback=`, covered by its own corpus below):
        // the masked-resolve order lifted it.
        use super::super::super::test_support::golden_passthroughs;

        let fixtures = [
            // The positional alt, whole and beside plain text, and the
            // `icon:` form.
            "image:x.png[stem:[y]]",
            "image:x.png[see stem:[y] here]",
            "icon:home[stem:[y]]",
            // The split invariant: a `,` or `=` inside the STEM body must
            // not divide the list — the string pipeline's own parse only
            // ever sees the sentinel.
            "image:x.png[stem:[a,b]]",
            "image:x.png[stem:[a=b],200]",
            // Named values, the positional width slot, and a role.
            "image:x.png[title=stem:[t]]",
            "image:x.png[alt,stem:[2],50]",
            "image:x.png[Sunset,role=stem:[r]]",
            // The other STEM notations.
            "image:x.png[latexmath:[y]]",
            "image:x.png[asciimath:[y]]",
            // Both masked kinds in one bracket, and target and bracket both
            // masked.
            "image:x.png[stem:[y] and ++z++]",
            "image:stem:[t][stem:[u]]",
            // In a rendered span, two in one flow, and escaped.
            "*a image:x.png[stem:[y]] b*",
            "image:x.png[stem:[a]] image:y.png[stem:[b]]",
            "\\image:x.png[stem:[y]]",
        ];

        let renderer = HtmlInlineRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_src(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden_passthroughs(fixture),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn registers_the_restored_target_for_an_image_over_a_passthrough() {
        // The staged side effect registers the node's own target — the
        // *restored* bytes. The string pipeline registers the sentinel it
        // matched (its restore pass rewrites only the rendered string, never
        // the catalog), which no consumer can read anything from; the cutover
        // deliberately adopts the tree's honest answer rather than
        // reproducing that wart, so this pins the policy with no
        // golden-catalog comparison — exactly as the link family's own
        // increment did.
        let source = "image:++sunset photo.jpg++[] and image:pass:[My Documents/chart.png][] and image:stem:[s].png[]";
        let parser = Parser::default().with_catalog_assets(true);
        let nodes = build_with(Span::new(source), &parser);

        apply_image_side_effects(&nodes, &parser, Span::new(source));

        let targets: Vec<_> = parser
            .catalog()
            .images()
            .iter()
            .map(|i| i.target.clone())
            .collect();

        assert_eq!(
            targets,
            ["sunset photo.jpg", "My Documents/chart.png", "\\$s\\$.png"]
        );
    }

    #[test]
    fn a_real_documents_passthrough_image_targets_fold_to_their_rendered_strings() {
        // End-to-end, through the real parse path, on the shapes that named
        // this increment: an `image:`/`icon:` target wrapped in (or crossing)
        // a passthrough must finish into the restored bytes the rendered
        // string carries — the `src` and the masked-derived default alt alike
        // — so a tree that kept it literal, or restored it differently, would
        // regress the moment `rendered_html()` becomes a fold of this tree.
        use crate::blocks::{FindBlocks, IsBlock};

        let doc = Parser::default().parse(concat!(
            "== A heading\n",
            "\n",
            "A sunset: image:++sunset_beach.jpg++[] under a masked name.\n",
            "\n",
            "See image:pass:[chart,v2.png][Chart] or icon:++a_b++[] today.\n",
            "\n",
            "A formula: image:stem:[E = mc^2].png[] as a target,\n",
            "and image:x.png[stem:[a,b]] in a bracket.\n",
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
                    &HtmlInlineRenderer {},
                    &Parser::default().render_context()
                ),
                rendered,
                "fold diverged from the rendered string for {inlines:?}"
            );

            folded_blocks += 1;
        }

        assert_eq!(folded_blocks, 3, "expected every paragraph to carry a tree");
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_an_image_target_over_a_stem_expression() {
        // Formerly this family's documented deferral — the last of the
        // restore-the-value class. A masked STEM expression restores into
        // the target exactly as a masked passthrough does (its body is
        // [`fold_stem`](super::super::fold::fold_stem)'s own output), and
        // the fold-time `web_path` this family alone sits behind runs over
        // the node's [`restored_target_ranges`](Image) *masked*, so the
        // backslash every rendered STEM body carries — and any `/`, `.`,
        // or space a masked body smuggles past the target class — never
        // reaches the resolver, in either pipeline.
        use super::super::super::test_support::golden_passthroughs;

        let fixtures = [
            // The wholly-masked target, bare and with an extension, in both
            // macro forms — and with an alt beside it.
            "image:stem:[x][]",
            "image:stem:[x].png[]",
            "icon:stem:[x][]",
            "image:stem:[x].png[Alt]",
            // The other STEM notations.
            "image:latexmath:[y].png[]",
            "image:asciimath:[y].png[]",
            // A partial mask: the default-alt arithmetic runs over the
            // masked bytes (the visible `_` reads as a space, the visible
            // extension comes off), with the surviving token restored.
            "image:stem:[x]_suffix.png[]",
            "image:dir/stem:[x].png[]",
            // Both masked kinds in one target.
            "image:++a++stem:[x].png[]",
            // A macro-level `imagesdir=`: the resolver joins the directory
            // and the masked target into one path, so the two mask sets
            // share one token numbering.
            "image:stem:[x].png[imagesdir=assets]",
            "image:x.png[imagesdir=stem:[d]]",
            "image:stem:[t].png[imagesdir=stem:[d]]",
            "image:x.png[imagesdir=pass:[my docs]]",
            // Two in one flow, inside a rendered span, and escaped.
            "image:stem:[a][] icon:stem:[b][]",
            "*see image:stem:[x].png[] here*",
            "\\image:stem:[x].png[]",
        ];

        let renderer = HtmlInlineRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_src(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden_passthroughs(fixture),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn an_image_target_over_a_stem_expression_is_recognized() {
        // The target is the restored bytes with the spliced ranges recorded
        // on the node, and the default alt is the masked derivation — the
        // rendered expression hides whole inside the sentinel, so the
        // extension still comes off around it.
        let nodes = build_src(Span::new("image:stem:[x].png[]"));

        let image = assert_image(&nodes[0]);
        assert_eq!(image.target.as_ref(), "\\$x\\$.png");
        assert_eq!(image.restored_target_ranges, vec![0..5]);
        assert_eq!(image.alt.as_deref(), Some("\\$x\\$"));

        // Both masked kinds in one target, each range its own record.
        let nodes = build_src(Span::new("image:++a++stem:[x].png[]"));

        let image = assert_image(&nodes[0]);
        assert_eq!(image.target.as_ref(), "a\\$x\\$.png");
        assert_eq!(image.restored_target_ranges, vec![0..1, 1..6]);

        // A verbatim target records none.
        let nodes = build_src(Span::new("image:sunset.jpg[]"));
        assert!(
            assert_image(&nodes[0]).restored_target_ranges.is_empty(),
            "a verbatim target must record no restored ranges"
        );
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_a_masked_fallback() {
        // The bracket's one `web_path`-bound value: an interactive SVG's
        // `fallback=` is run through `image_src` at fold time, so it
        // resolves over the attribute's own restored ranges masked (see
        // [`ElementAttribute::into_owned_restoring`](crate::attributes::ElementAttribute)) —
        // the same order the target takes, at the same seam. Interactive
        // SVGs render only below `SafeMode::Secure`, so this corpus runs
        // under `Unsafe`, on both pipelines' side of the comparison.
        use super::super::super::test_support::golden_passthroughs_with;
        use crate::parser::SafeMode;

        let fixtures = [
            "image:x.svg[opts=interactive,fallback=stem:[y].png]",
            "image:x.svg[opts=interactive,fallback=pass:[my docs/f].png]",
            "image:x.svg[Alt,opts=interactive,fallback=++a b++.png]",
        ];

        let renderer = HtmlInlineRenderer {};

        for fixture in fixtures {
            let parser = Parser::default().with_safe_mode(SafeMode::Unsafe);

            let folded = crate::content::inline_builder::fold_html(
                &build_with(Span::new(fixture), &parser),
                &renderer,
                &parser.render_context(),
            );

            assert_eq!(
                folded,
                golden_passthroughs_with(fixture, &parser),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn an_image_target_over_a_stem_expression_is_platform_independent() {
        // The reason this family deferred the restore for as long as it did:
        // a rendered STEM body always carries a backslash, and `web_path`
        // posixifies the platform separator, so a restore that reached the
        // resolver would make the `src` differ by platform (CI runs all
        // three). The masked resolve keeps the body away from the resolver,
        // so the fold is byte-identical under either separator — and equal
        // to the golden, whose own resolver only ever saw the sentinel.
        use super::super::super::test_support::golden_passthroughs;

        for source in [
            "image:stem:[x].png[]",
            "image:stem:[t].png[imagesdir=stem:[d]]",
        ] {
            let posix = Parser::default().with_path_resolver(DefaultPathResolver {
                file_separator: '/',
            });

            let windows = Parser::default().with_path_resolver(DefaultPathResolver {
                file_separator: '\\',
            });

            let renderer = HtmlInlineRenderer {};

            let fold_with = |parser: &Parser| {
                crate::content::inline_builder::fold_html(
                    &build_with(Span::new(source), parser),
                    &renderer,
                    &parser.render_context(),
                )
            };

            let posix_fold = fold_with(&posix);

            assert_eq!(posix_fold, fold_with(&windows));
            assert_eq!(posix_fold, golden_passthroughs(source));
        }
    }

    #[test]
    fn restorable_body_agrees_with_node_is_restorable() {
        // The cheap discriminant and the body producer must name the same set
        // — a gate that admits a piece whose body cannot be produced would
        // leave the placeholder in a computed value, and one that rejects a
        // piece whose body *can* be would keep a construct needlessly
        // literal. Pinned over one node of every kind the two decide between.
        let renderer = HtmlInlineRenderer {};
        let root = Span::new("x");

        let restorable = [
            InlineNode::Raw {
                value: CowStr::from("raw"),
                form: RawForm::AsIs,
                origin: RawOrigin::Passthrough {
                    subs: crate::content::SubstitutionGroup::None,
                    source_text: None,
                },
                location: root,
            },
            InlineNode::Stem(crate::inlines::Stem {
                notation: crate::inlines::StemNotation::AsciiMath,
                value: CowStr::from("x"),
                subs: crate::content::SubstitutionGroup::Stem,
                source_text: None,
                children: vec![],
                location: root,
            }),
        ];

        for node in &restorable {
            assert!(node_is_restorable(node), "expected restorable: {node:?}");
            assert!(
                restorable_body(node, &renderer).is_some(),
                "expected a body: {node:?}"
            );
        }

        let opaque = [
            InlineNode::Text {
                value: CowStr::from("t"),
                location: root,
            },
            InlineNode::CharRef {
                value: CharRef::Special('<'),
                location: root,
            },
            InlineNode::LineBreak { location: root },
        ];

        for node in &opaque {
            assert!(!node_is_restorable(node), "expected opaque: {node:?}");
            assert!(
                restorable_body(node, &renderer).is_none(),
                "expected no body: {node:?}"
            );
        }
    }

    #[test]
    fn registers_the_recorded_target_for_a_target_crossing_an_escaped_special() {
        // The staged `register_image` reads the node's own stored `target`,
        // which is the escaped one — byte-identical to the `caps[1]` the
        // string replacer registered. Frozen at the last differentially-
        // verified parity, like the broad set above.
        let fixtures = [
            ("image:a&b.png[]", r#"["a&amp;b.png"]"#),
            ("image:a<b.png[Alt]", r#"["a&lt;b.png"]"#),
            (
                "image:a&b.png[] image:c&d.png[]",
                r#"["a&amp;b.png", "c&amp;d.png"]"#,
            ),
            ("icon:a&b[]", "[]"),
            ("\\image:a&b.png[A]", "[]"),
        ];

        for (fixture, expected) in fixtures {
            let builder_parser = Parser::default().with_catalog_assets(true);
            let nodes = build_with(Span::new(fixture), &builder_parser);
            apply_image_side_effects(&nodes, &builder_parser, Span::new(fixture));

            let got: Vec<_> = builder_parser
                .catalog()
                .images()
                .iter()
                .map(|i| i.target.clone())
                .collect();

            assert_eq!(
                format!("{got:?}"),
                expected,
                "registered images diverged for {fixture:?}"
            );
        }
    }

    /// Builds the tree **through character replacements** (special characters,
    /// quotes, character replacements) — the state the macros step consumes —
    /// so a test can drive [`apply_macros`] directly.
    fn build_through_special_and_replacements(source: Span<'_>) -> Vec<InlineNode<'_>> {
        apply_character_replacements(build_through_quotes(source), source)
    }

    // ---- `apply_image_side_effects` (staged for the eventual cutover) -----

    use super::apply_image_side_effects;
    use crate::warnings::WarningType;

    /// Builds the single-pass tree for `source` against `parser` (unlike
    /// [`build_src`], which always uses its own fresh default parser).
    fn build_with<'src>(source: Span<'src>, parser: &Parser) -> Vec<InlineNode<'src>> {
        build(source, parser, None)
    }

    #[test]
    fn registers_an_image_target_when_catalog_assets_is_enabled() {
        let source = "image:sunset.jpg[Sunset]";
        let parser = Parser::default().with_catalog_assets(true);
        let nodes = build_with(Span::new(source), &parser);

        apply_image_side_effects(&nodes, &parser, Span::new(source));

        let catalog = parser.catalog();
        let images = catalog.images();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].target, "sunset.jpg");
    }

    #[test]
    fn does_not_register_an_icon_as_an_image() {
        // `icon:` shares the `Image` node with `image:`, but only `image:` is
        // registered in the asset catalog — mirroring
        // `InlineImageMacroReplacer`'s own `caps[0].starts_with("image:")`
        // gate.
        let source = "icon:home[]";
        let parser = Parser::default().with_catalog_assets(true);
        let nodes = build_with(Span::new(source), &parser);

        apply_image_side_effects(&nodes, &parser, Span::new(source));

        assert!(parser.catalog().images().is_empty());
    }

    #[test]
    fn registration_is_a_no_op_when_catalog_assets_is_disabled() {
        // `catalog_assets` defaults to off; `register_image` is then a no-op,
        // mirroring the string pipeline's own `Parser::register_image`.
        let source = "image:sunset.jpg[Sunset]";
        let parser = Parser::default();
        let nodes = build_with(Span::new(source), &parser);

        apply_image_side_effects(&nodes, &parser, Span::new(source));

        assert!(parser.catalog().images().is_empty());
    }

    #[test]
    fn registers_an_image_nested_inside_a_styled_span_and_a_footnote() {
        // An `Image` node can be nested inside a `Styled` span (matched inside
        // a rendered span, mirroring the string pipeline) or captured whole
        // into a `Footnote`'s own children (the footnote increment's own
        // `emit_range`); both containers must be walked.
        let source = "*see image:a.png[]* and footnote:[see image:b.png[]]";
        let parser = Parser::default().with_catalog_assets(true);
        let nodes = build_with(Span::new(source), &parser);

        apply_image_side_effects(&nodes, &parser, Span::new(source));

        let catalog = parser.catalog();
        let images = catalog.images();
        assert_eq!(images.len(), 2);
        assert_eq!(images[0].target, "a.png");
        assert_eq!(images[1].target, "b.png");
    }

    #[test]
    fn registers_an_image_nested_inside_a_refs_display_children() {
        // A `Ref`'s display children can hold an `Image` too — `apply_macros`
        // itself descends into a `Ref`'s own children before matching at its
        // level (see
        // `apply_macros_recognizes_a_macro_inside_reference_children`
        // in `macros/mod.rs`'s own tests), so a hand-built `Ref` exercises the
        // same container here.
        use crate::inlines::{Ref, RefVariant};

        let root = Span::new("image:a.png[]");
        let image = build_with(root, &Parser::default());
        assert_eq!(image.len(), 1);

        let reference = InlineNode::Ref(Ref {
            variant: RefVariant::Link,
            link_form: Some(crate::inlines::LinkForm::Macro),
            target: CowStr::from("https://example.org"),
            children: image,
            roles: vec![],
            window: None,
            resolved: None,
            derived: None,
            xrefstyle: None,
            attrs: crate::attributes::Attrlist::empty(root.slice(0..0)),
            location: root,
        });

        let parser = Parser::default().with_catalog_assets(true);
        apply_image_side_effects(std::slice::from_ref(&reference), &parser, root);

        let catalog = parser.catalog();
        let images = catalog.images();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].target, "a.png");
    }

    #[test]
    fn registers_the_recorded_targets_for_a_broad_fixture_set() {
        // Each expected set is **frozen at the last differentially-verified
        // parity**: until the string pipeline's deletion this test registered
        // each fixture through it on an independent parser and compared the
        // two catalogs, green at the commit that deleted it — the literals are
        // that pipeline's own answer.
        let fixtures = [
            ("image:sunset.jpg[Sunset]", r#"["sunset.jpg"]"#),
            ("icon:home[]", "[]"),
            (
                "image:sunset.jpg[Sunset]{sp}image:other.png[]",
                r#"["sunset.jpg", "other.png"]"#,
            ),
            ("image without a bracket image:foo.png stays literal", "[]"),
            ("\\image:sunset.jpg[Sunset]", "[]"),
            // A bracket with no `'src` slice of its own: the attribute list
            // is parsed from the match string, but `register_image` reads the
            // node's `target`, so the recorded target is still the match's.
            ("image:sunset.jpg[a < b]", r#"["sunset.jpg"]"#),
            ("image:a&b.png[Tom &amp; Jerry,200]", r#"["a&amp;b.png"]"#),
        ];

        for (fixture, expected) in fixtures {
            let builder_parser = Parser::default().with_catalog_assets(true);
            let nodes = build_with(Span::new(fixture), &builder_parser);
            apply_image_side_effects(&nodes, &builder_parser, Span::new(fixture));

            let got: Vec<_> = builder_parser
                .catalog()
                .images()
                .iter()
                .map(|i| i.target.clone())
                .collect();

            assert_eq!(
                format!("{got:?}"),
                expected,
                "registered images diverged for {fixture:?}"
            );
        }
    }

    #[test]
    fn records_the_dangerous_scheme_warning_from_a_bracket_with_no_source_slice() {
        // The `link=` warning reads the node's stored attribute list, so a
        // bracket parsed from the match string must carry it exactly as a
        // verbatim one does — here reached through an escaped special earlier
        // in the same bracket, which is what denies it an `'src` slice.
        let source = "image:safe.png[a < b,link=javascript:alert(1)]";
        let parser = Parser::default();
        let nodes = build_with(Span::new(source), &parser);

        let before = parser.substitution_warnings_len();
        apply_image_side_effects(&nodes, &parser, Span::new(source));
        let warnings = parser.drain_substitution_warnings_since(before);

        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::UnsafeLinkSchemeRejected("javascript:alert(1)".to_string())
        );
    }

    #[test]
    fn records_the_dangerous_scheme_warning_for_an_explicit_link_target() {
        let source = "image:safe.png[alt,link=javascript:alert(1)]";
        let parser = Parser::default();
        let nodes = build_with(Span::new(source), &parser);

        let before = parser.substitution_warnings_len();
        apply_image_side_effects(&nodes, &parser, Span::new(source));
        let warnings = parser.drain_substitution_warnings_since(before);

        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::UnsafeLinkSchemeRejected("javascript:alert(1)".to_string())
        );
    }

    #[test]
    fn records_the_dangerous_scheme_warning_case_insensitively() {
        let source = "image:safe.png[alt,link=JavaScript:alert(1)]";
        let parser = Parser::default();
        let nodes = build_with(Span::new(source), &parser);

        let before = parser.substitution_warnings_len();
        apply_image_side_effects(&nodes, &parser, Span::new(source));
        let warnings = parser.drain_substitution_warnings_since(before);

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn does_not_warn_for_a_safe_explicit_link_target() {
        let source = "image:safe.png[alt,link=https://example.org]";
        let parser = Parser::default();
        let nodes = build_with(Span::new(source), &parser);

        let before = parser.substitution_warnings_len();
        apply_image_side_effects(&nodes, &parser, Span::new(source));

        assert_eq!(parser.substitution_warnings_len(), before);
    }

    #[test]
    fn records_the_warning_for_a_dangerous_image_target_promoted_by_link_self() {
        // `link=self` resolves the anchor `href` to the image's own `src`
        // (`target`); a dangerous target is rejected exactly as an explicit
        // `link=` value would be.
        let source = "image:javascript:alert(1)[alt,link=self]";
        let parser = Parser::default();
        let nodes = build_with(Span::new(source), &parser);

        let before = parser.substitution_warnings_len();
        apply_image_side_effects(&nodes, &parser, Span::new(source));
        let warnings = parser.drain_substitution_warnings_since(before);

        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::UnsafeLinkSchemeRejected("javascript:alert(1)".to_string())
        );
    }

    #[test]
    fn does_not_warn_for_link_self_on_a_safe_image_target() {
        let source = "image:safe.png[alt,link=self]";
        let parser = Parser::default();
        let nodes = build_with(Span::new(source), &parser);

        let before = parser.substitution_warnings_len();
        apply_image_side_effects(&nodes, &parser, Span::new(source));

        assert_eq!(parser.substitution_warnings_len(), before);
    }

    #[test]
    fn records_the_warning_for_a_dangerous_icon_target_promoted_by_link_self_in_image_icon_mode() {
        // An `icon:` target only promotes into a live `href` in image-icon
        // mode (`icons` set, not `font`); with `icons` unset (the default) an
        // icon has no `src` at all, so `link=self` stays the literal `self`
        // and nothing is rejected — see the companion
        // `does_not_warn_for_link_self_on_a_font_icon` test for that case.
        use crate::parser::ModificationContext;

        let source = "icon:javascript:alert(1)[link=self]";
        let parser =
            Parser::default().with_intrinsic_attribute("icons", "", ModificationContext::ApiOnly);
        let nodes = build_with(Span::new(source), &parser);

        let before = parser.substitution_warnings_len();
        apply_image_side_effects(&nodes, &parser, Span::new(source));
        let warnings = parser.drain_substitution_warnings_since(before);

        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::UnsafeLinkSchemeRejected("javascript:alert(1)".to_string())
        );
    }

    #[test]
    fn does_not_warn_for_link_self_on_a_font_icon() {
        // A font icon has no `src`, so `link=self` resolves to the literal
        // `self` (harmless) rather than the dangerous target — nothing is
        // rejected, mirroring
        // `font_icon_link_self_with_dangerous_target_keeps_literal_self_without_warning`
        // in `parser/src/tests/security.rs`.
        use crate::parser::ModificationContext;

        let source = "icon:javascript:alert(1)[link=self]";
        let parser = Parser::default().with_intrinsic_attribute(
            "icons",
            "font",
            ModificationContext::ApiOnly,
        );
        let nodes = build_with(Span::new(source), &parser);

        let before = parser.substitution_warnings_len();
        apply_image_side_effects(&nodes, &parser, Span::new(source));

        assert_eq!(parser.substitution_warnings_len(), before);
    }

    /// A parser carrying the attributes the expanded-value fixtures below
    /// reference.
    fn expanding_parser() -> Parser {
        use crate::parser::ModificationContext;

        Parser::default()
            .with_intrinsic_attribute("logo", "sunset.jpg", ModificationContext::Anywhere)
            .with_intrinsic_attribute("dir", "assets", ModificationContext::Anywhere)
            .with_intrinsic_attribute("iconname", "home", ModificationContext::Anywhere)
            .with_intrinsic_attribute("caption", "Sunset", ModificationContext::Anywhere)
            .with_intrinsic_attribute(
                "img-src",
                "image:sunset.jpg[]",
                ModificationContext::Anywhere,
            )
            .with_intrinsic_attribute(
                "img-src-alt",
                "image:sunset.jpg[Sunset]",
                ModificationContext::Anywhere,
            )
    }

    /// The real, public pipeline's output for `source` — the golden for the
    /// expanded-value fixtures, which need the `AttributeReferences` step
    /// [`golden_macros_with`] deliberately omits.
    fn golden_normal(source: &str, _parser: &Parser) -> String {
        crate::content::inline_builder::snapshot::recorded("image_normal", source)
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_images_inside_expanded_values() {
        // An image or icon macro whose *target* (or whose whole macro name)
        // crosses a synthesized run is now recognized: the name and target are
        // read from the level's match string, which carries an expanded value's
        // bytes exactly, so only the node's `location` takes design §4.4's
        // coarse fallback. The macro's own attribute list is the one part that
        // still needs an honest `'src` slice — see the divergence test below —
        // so every fixture here either carries a verbatim bracket or an empty
        // one.
        let parser = expanding_parser();

        let fixtures = [
            // An expanded target, with and without an attribute list.
            "image:{logo}[Logo]",
            "image:{logo}[]",
            "icon:{iconname}[]",
            "icon:{iconname}[Home]",
            // A partially-expanded target: the expansion is one piece of it.
            "image:{dir}/sunset.jpg[Sunset]",
            "image:{dir}/{logo}[]",
            // A target crossing *both* a synthesized run and an escaped
            // special, so neither the source nor the expansion alone carries
            // its bytes — only the level's own match string does.
            "image:{dir}/a&b.png[]",
            "image:{dir}/a&b.png[Alt]",
            // A verbatim attribute list beside an expanded target, including
            // the positional width/height and a named attribute.
            "image:{logo}[Alt Text,200,100]",
            "image:{logo}[alt=Alt Text,width=200]",
            "image:{logo}[Logo,role=thumb]",
            // The whole macro arriving from an expanded value — recognized
            // only because its bracket is empty (see the divergence test for
            // the non-empty case).
            "see {img-src} here",
            // Embedded in surrounding flow, beside another macro, and inside a
            // rendered span.
            "See image:{logo}[Logo] here.",
            "image:{logo}[A] then image:other.png[B]",
            "*image:{logo}[Logo]*",
            // An escape still keeps the macro literal.
            "\\image:{logo}[Logo]",
        ];

        let renderer = HtmlInlineRenderer {};

        for fixture in fixtures {
            let folded = crate::content::inline_builder::fold_html(
                &build(Span::new(fixture), &parser, None),
                &renderer,
                &parser.render_context(),
            );

            assert_eq!(
                folded,
                golden_normal(fixture, &parser),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn an_image_inside_an_expanded_value_keeps_a_coarse_location() {
        // The target is recovered *exactly* (through `text_slice`), while the
        // node's `location` falls back to the enclosing synthesized run's own
        // span — design §4.4's documented split, the same one the anchor, UI,
        // index-term, and cross-reference families already take.
        let parser = expanding_parser();
        let source = "image:{logo}[Logo]";
        let nodes = build(Span::new(source), &parser, None);

        assert_eq!(nodes.len(), 1);
        let image = assert_image(&nodes[0]);

        assert!(!image.is_icon);
        assert_eq!(image.target.as_ref(), "sunset.jpg");
        assert_eq!(image.alt.as_deref(), Some("Logo"));

        // The whole macro's span: its own source bytes, not the expansion's.
        assert_eq!(image.location.data(), source);
    }

    #[test]
    fn an_icons_name_inside_an_expanded_value_is_read_from_the_match_string() {
        // The `image:`/`icon:` discriminant is read from the match string, not
        // from `location.data()` — which, for a wholly-expanded macro, is the
        // attribute reference rather than the macro.
        use crate::parser::ModificationContext;

        let parser = Parser::default().with_intrinsic_attribute(
            "icon-src",
            "icon:home[]",
            ModificationContext::Anywhere,
        );

        let nodes = build(Span::new("{icon-src}"), &parser, None);

        assert_eq!(nodes.len(), 1);
        let image = assert_image(&nodes[0]);

        assert!(
            image.is_icon,
            "the macro name must come from the match string"
        );
        assert_eq!(image.target.as_ref(), "home");
        assert_eq!(image.location.data(), "{icon-src}");
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_an_attribute_list_inside_an_expanded_value() {
        // A non-empty attribute list crossing a synthesized run takes the same
        // lift: the expansion's bytes live only in the level's match string —
        // which is exactly the already-substituted haystack the string
        // replacer parses its own `caps[2]` out of — so the bracket is parsed
        // from there and owned off it.
        let parser = expanding_parser();

        let fixtures = [
            // An expanded alt, whole and in part, with and without an
            // expanded target beside it.
            "image:sunset.jpg[{caption}]",
            "image:{logo}[{caption}]",
            "image:sunset.jpg[The {caption} photo]",
            // An expanded value in a *named* attribute, and beside the
            // positional width/height.
            "image:sunset.jpg[alt={caption}]",
            "image:{logo}[{caption},200]",
            "image:{logo}[{caption},role=thumb]",
            // The whole macro arriving from an expanded value, this time with
            // a non-empty bracket — the empty-bracket spelling of the same
            // shape was already recognized.
            "see {img-src-alt} here",
            // An expansion and an escaped special in the same bracket, so
            // neither the source nor the expansion alone carries its bytes.
            "image:sunset.jpg[{caption} < x]",
            // The `icon:` spelling, in surrounding flow, and inside a
            // rendered span.
            "icon:{iconname}[{caption}]",
            "See image:sunset.jpg[{caption}] here.",
            "*image:sunset.jpg[{caption}]*",
            // An escape still keeps the macro literal.
            "\\image:sunset.jpg[{caption}]",
        ];

        let renderer = HtmlInlineRenderer {};

        for fixture in fixtures {
            let folded = crate::content::inline_builder::fold_html(
                &build(Span::new(fixture), &parser, None),
                &renderer,
                &parser.render_context(),
            );

            assert_eq!(
                folded,
                golden_normal(fixture, &parser),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn an_attribute_list_inside_an_expanded_value_is_owned_and_coarsely_located() {
        // The attribute values are the expansion's own bytes, owned rather
        // than borrowed from the temporary they were parsed from; the list's
        // span takes design §4.4's coarse fallback — here the whole enclosing
        // synthesized run — exactly as the node's `location` does.
        let parser = expanding_parser();
        let source = "image:sunset.jpg[{caption},role=hl]";
        let nodes = build(Span::new(source), &parser, None);

        assert_eq!(nodes.len(), 1);
        let image = assert_image(&nodes[0]);

        assert_eq!(image.alt.as_deref(), Some("Sunset"));

        let attrs = &image.attrs;
        assert_eq!(attrs.nth_attribute(1).unwrap().value(), "Sunset");
        assert_eq!(attrs.named_attribute("role").unwrap().value(), "hl");

        // The location tag covers the bracket's own source, `{caption}`
        // included — the coarse fallback, not the parsed text.
        assert_eq!(attrs.span().data(), "{caption},role=hl");
        assert_eq!(image.location.data(), source);
    }

    #[test]
    fn registers_the_recorded_target_for_images_inside_expanded_values() {
        // The staged `register_image` side effect reads the node's own stored
        // `target`, which is the *expanded* one — what the string pipeline's
        // own expanded haystack matched and registered. Frozen at the last
        // differentially-verified parity, like the broad set above.
        let fixtures = [
            ("image:{logo}[Logo]", r#"["sunset.jpg"]"#),
            ("image:{logo}[]", r#"["sunset.jpg"]"#),
            ("image:{dir}/{logo}[]", r#"["assets/sunset.jpg"]"#),
            ("see {img-src} here", r#"["sunset.jpg"]"#),
            (
                "image:{logo}[A] then image:other.png[B]",
                r#"["sunset.jpg", "other.png"]"#,
            ),
        ];

        for (fixture, expected) in fixtures {
            let builder_parser = expanding_parser().with_catalog_assets(true);
            let nodes = build(Span::new(fixture), &builder_parser, None);
            apply_image_side_effects(&nodes, &builder_parser, Span::new(fixture));

            let got: Vec<_> = builder_parser
                .catalog()
                .images()
                .iter()
                .map(|i| i.target.clone())
                .collect();

            assert_eq!(
                format!("{got:?}"),
                expected,
                "registered images diverged for {fixture:?}"
            );
        }
    }

    #[test]
    fn a_targetless_macro_is_not_recognized() {
        // `INLINE_IMAGE_MACRO` requires a target and makes only its trailing
        // portion optional, so `image:[…]` is not a match: the author's text
        // stays on the page as literal text and no node is built.
        //
        // Until the target group was made mandatory this shape *did* match
        // here, with an empty target — while the string pipeline's own
        // `InlineImageMacroReplacer` panicked reading `&caps[1]`, so the two
        // could not be compared at all. Both families read the one shared
        // pattern, so the fix reached this side with it; the shape now sits in
        // the differential corpus above, and what is pinned here is the
        // *structure* the fold comes from.
        let nodes = build_src(Span::new("image:[Alt Text]"));

        assert_eq!(nodes.len(), 1);
        assert_text(&nodes[0], "image:[Alt Text]", 1, 1);
    }

    #[test]
    fn a_one_character_target_is_recognized() {
        // The complement of the shape above, and the other half of what making
        // the target mandatory-with-an-optional-remainder changed: `image:a[]`
        // needed two characters under the old pattern and was silently left
        // literal.
        let nodes = build_src(Span::new("image:a[Alt Text]"));

        assert_eq!(nodes.len(), 1);
        let image = assert_image(&nodes[0]);

        assert!(!image.is_icon);
        assert_eq!(image.target.as_ref(), "a");
        assert_eq!(image.alt.as_deref(), Some("Alt Text"));
        assert_eq!(image.location.data(), "image:a[Alt Text]");
    }
}
