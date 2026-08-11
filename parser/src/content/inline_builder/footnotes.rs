//! The footnote substitution step.

use super::{
    macros::{MacroMatch, MacroMatchKind},
    quotes::{Piece, build_match_string, emit_range, source_slice},
};
use crate::{
    Parser, Span,
    content::{INLINE_FOOTNOTE_MACRO, normalize_footnote_text},
    inlines::{Footnote, InlineNode},
    strings::CowStr,
};

/// The footnote substitution, as a **whole-tree**, order-preserving
/// transducer: matches [`INLINE_FOOTNOTE_MACRO`] at every level of `nodes`,
/// replacing each recognized occurrence with the
/// [`Footnote`](InlineNode::Footnote) node it produces, and recurses into
/// every [`Styled`](crate::inlines::Styled)/[`Ref`](crate::inlines::Ref) child
/// it finds along the way.
///
/// This is the last macro family (design §5.2 Phase 4, step 4b(ii) part 4c)
/// and runs as [`build`](super::build)'s own step, *after*
/// [`apply_macros`](super::macros::apply_macros) has fully resolved every other
/// family at every level, mirroring the string step's order exactly: footnotes
/// run last, once, over the whole (already substituted) string (macros.rs).
///
/// # Why this cannot be a level pass *within* [`apply_macros`](super::macros::apply_macros)
///
/// Every other macro family is recognition-order-independent: nothing
/// observes *when*, relative to a sibling subtree, an image or a link is
/// recognized, so it is safe for [`apply_macros`](super::macros::apply_macros)
/// to resolve a node's children *before* resolving that node's own level (its
/// recursion runs depth-first). A footnote's assigned **number** is the one
/// exception – it is a side effect of recognition order itself (see
/// [`build_footnote_node`]'s doc comment) – so depth-first recursion would
/// number a footnote nested in an *earlier*-created
/// [`Styled`](crate::inlines::Styled) span (e.g. a
/// span [`apply_quotes`](super::quotes::apply_quotes) already built before
/// [`apply_macros`](super::macros::apply_macros) ever runs) **before** a
/// footnote that precedes that span in the source, reversing their markers
/// relative to the string pipeline's left-to-right sweep over one flat string.
/// This function fixes that by walking `nodes` in true source order: it
/// recognizes every footnote at *this* level, but recurses
/// into a [`Styled`](crate::inlines::Styled)/[`Ref`](crate::inlines::Ref) child
/// at exactly the point that child falls between two such recognitions (or
/// before the first / after the last) – see [`rebuild_footnote_level`] and
/// [`emit_range_recursing_footnotes`], which recurse in place of the generic
/// [`rebuild_macro_level`](super::macros::rebuild_macro_level) /
/// [`emit_range`]'s verbatim clone.
pub(super) fn apply_footnotes<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    // A subtree with no `"tnote"` substring anywhere – the overwhelmingly
    // common case – has nothing for this pass to do, so return it completely
    // untouched: no clone, no rebuild. This check is not just an optimization.
    // `rebuild_footnote_level`'s gap emission clones *every* atomic piece
    // (any already-recognized `Image`/`Ui`/`Ref`/`Anchor`/`IndexTerm`/`Styled`
    // node) it touches – including ones with nothing to do with footnotes –
    // and cloning is not free of *observable* effect here: `CowStr`'s `Clone`
    // opportunistically demotes a short-enough `Boxed` value to `Inlined`
    // (see `strings.rs`), so an unconditional whole-tree rebuild would flip
    // that representation on every short owned string in the document, purely
    // as a side effect of a pass that found nothing to recognize. Skipping
    // the rebuild entirely when there is nothing to find keeps every other
    // family's nodes byte- *and* representation-identical to what
    // `apply_macros` produced.
    if !subtree_might_have_footnote(&nodes) {
        return nodes;
    }

    let (s, pieces) = build_match_string(&nodes);

    // Cheap pre-filter mirroring the string step's `found_macroish &&
    // text.contains("tnote")`: both `footnote:` and the deprecated
    // `footnoteref:` contain "tnote". Even when this level's *own* text has
    // no match, a `Styled`/`Ref` child might still carry one in its own
    // subtree (that is what the pre-check above just confirmed), so the
    // rebuild below runs regardless – it is what performs the recursion.
    let matches = if s.contains("tnote") {
        find_footnote_matches(&s, &pieces, &nodes, root, parser)
    } else {
        Vec::new()
    };

    rebuild_footnote_level(&nodes, &pieces, &s, matches, root, parser)
}

/// Reports whether `nodes`, or any
/// [`Styled`](crate::inlines::Styled)/[`Ref`](crate::inlines::Ref) descendant's
/// own subtree at any depth, might carry the `"tnote"` substring
/// [`apply_footnotes`]'s pre-filter looks for. A read-only recursive check –
/// it builds a match string per level exactly as [`apply_footnotes`] does
/// (so it cannot miss a `"tnote"` split across two adjacent
/// [`Text`](InlineNode::Text) pieces the way a per-node substring check
/// could) but allocates no [`InlineNode`], letting a subtree with nothing to
/// find come back from `apply_footnotes` completely unchanged.
fn subtree_might_have_footnote(nodes: &[InlineNode<'_>]) -> bool {
    let (s, _) = build_match_string(nodes);

    if s.contains("tnote") {
        return true;
    }

    nodes.iter().any(|node| match node {
        InlineNode::Styled(styled) => subtree_might_have_footnote(&styled.children),
        InlineNode::Ref(reference) => subtree_might_have_footnote(&reference.children),
        _ => false,
    })
}

/// Like [`rebuild_macro_level`](super::macros::rebuild_macro_level), but for
/// [`apply_footnotes`]: rebuilds a level's node list from its footnote matches,
/// using [`emit_range_recursing_footnotes`] (not [`emit_range`]) for every gap,
/// so a [`Styled`](crate::inlines::Styled)/[`Ref`](crate::inlines::Ref) child
/// encountered between two matches is recursed into – in source order – rather
/// than cloned whole.
fn rebuild_footnote_level<'src>(
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    s: &str,
    matches: Vec<MacroMatch<'src>>,
    root: Span<'src>,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    let mut out = Vec::new();
    let mut cursor = 0usize;

    for m in matches {
        let MacroMatch { full, kind } = m;

        match kind {
            MacroMatchKind::Unescape { backslash } => {
                emit_range_recursing_footnotes(
                    nodes,
                    pieces,
                    cursor..backslash,
                    root,
                    parser,
                    &mut out,
                );
                emit_range_recursing_footnotes(
                    nodes,
                    pieces,
                    (backslash + 1)..full.end,
                    root,
                    parser,
                    &mut out,
                );
            }

            MacroMatchKind::Node { consumed, node } => {
                emit_range_recursing_footnotes(
                    nodes,
                    pieces,
                    cursor..consumed.start,
                    root,
                    parser,
                    &mut out,
                );
                out.push(*node);
                emit_range_recursing_footnotes(
                    nodes,
                    pieces,
                    consumed.end..full.end,
                    root,
                    parser,
                    &mut out,
                );
            }
        }

        cursor = full.end;
    }

    if cursor < s.len() {
        emit_range_recursing_footnotes(nodes, pieces, cursor..s.len(), root, parser, &mut out);
    }

    out
}

/// Like [`emit_range`], but recurses into a
/// [`Styled`](crate::inlines::Styled)/[`Ref`](crate::inlines::Ref) piece's
/// children with [`apply_footnotes`] instead of cloning the node whole – the
/// piece that makes [`apply_footnotes`] a true whole-tree, source-order walk
/// rather than a single level pass. Every other aspect (slicing a verbatim
/// [`Text`](InlineNode::Text) run at the overlap, an empty range emitting
/// nothing) is identical.
fn emit_range_recursing_footnotes<'src>(
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    range: std::ops::Range<usize>,
    root: Span<'src>,
    parser: &Parser,
    out: &mut Vec<InlineNode<'src>>,
) {
    if range.start >= range.end {
        return;
    }

    for piece in pieces {
        let p_start = piece.s_start;
        let p_end = piece.s_start + piece.s_len;

        // Skip pieces that do not overlap the requested range.
        if p_end <= range.start || p_start >= range.end {
            continue;
        }

        let Some(node) = nodes.get(piece.node_index) else {
            continue;
        };

        if piece.atomic {
            match node.clone() {
                InlineNode::Styled(mut styled) => {
                    styled.children = apply_footnotes(styled.children, root, parser);
                    out.push(InlineNode::Styled(styled));
                }

                InlineNode::Ref(mut reference) => {
                    reference.children = apply_footnotes(reference.children, root, parser);
                    out.push(InlineNode::Ref(reference));
                }

                other => out.push(other),
            }

            continue;
        }

        // A verbatim text run: slice it to the overlap.
        let lo = range.start.max(p_start) - p_start;
        let hi = range.end.min(p_end) - p_start;

        if let InlineNode::Text { location, .. } = node {
            let sliced = location.slice(lo..hi);

            out.push(InlineNode::Text {
                value: CowStr::from(sliced.data()),
                location: sliced,
            });
        }
    }
}

/// Finds every recognized footnote occurrence at this level as a
/// [`MacroMatch`], skipping the forms this increment defers (see
/// [`build_footnote_node`] and [`build_footnoteref_node`]).
fn find_footnote_matches<'src>(
    s: &str,
    pieces: &[Piece],
    nodes: &[InlineNode<'src>],
    root: Span<'src>,
    parser: &Parser,
) -> Vec<MacroMatch<'src>> {
    let mut matches = Vec::new();

    for caps in INLINE_FOOTNOTE_MACRO.captures_iter(s) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();

        let full = whole.start()..whole.end();

        // An escape (`\footnote:…`, `\footnoteref:…`) is honored by dropping
        // the backslash and keeping the rest literal, mirroring the string
        // replacer's leading `caps[0].starts_with('\\')` check – which runs
        // *before* the ref-vs-plain branch below, so this must too.
        if whole.as_str().starts_with('\\') {
            matches.push(MacroMatch {
                kind: MacroMatchKind::Unescape {
                    backslash: full.start,
                },
                full,
            });

            continue;
        }

        // The deprecated `footnoteref:[id,text]` / `footnoteref:[id]` form
        // (group 1) packs its id and text into one bracket, split on the
        // first comma, rather than taking the id from the macro target the
        // way `footnote:id[…]` does.
        if caps.get(1).is_some() {
            // With no bracketed text at all (`footnoteref:[]`), it is left
            // unrecognized – mirroring the string replacer's `next $&`.
            let Some(raw) = caps.get(3) else {
                continue;
            };

            let Some(node) = build_footnoteref_node(raw, &full, s, pieces, nodes, root, parser)
            else {
                continue;
            };

            matches.push(MacroMatch {
                kind: MacroMatchKind::Node {
                    consumed: full.clone(),
                    node: Box::new(node),
                },
                full,
            });

            continue;
        }

        let Some(node) = build_footnote_node(&caps, &full, s, pieces, nodes, root, parser) else {
            continue;
        };

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

/// Builds one [`Footnote`](InlineNode::Footnote) node from a `footnote:` match,
/// resolving it into the same three (id, content) cases the string
/// replacer's `InlineFootnoteMacroReplacer` distinguishes, so the fold –
/// which reconstructs
/// [`FootnoteRenderParams`](crate::parser::FootnoteRenderParams) from the node
/// alone (see `fold_footnote`) – reproduces the same bytes. Returns `None` for
/// a form this increment defers, or for `footnote:[]` (neither an id nor
/// content), which is not a footnote at all.
///
/// # The one *required* recognition side effect
///
/// Every other macro family in this module performs *no* recognition side
/// effect (no catalog registration, no warning), deferring that to the
/// cutover (design §5.2 Phase 4, step 6) because omitting it does not change
/// the fold's output bytes. A footnote's own marker is the one exception: its
/// rendered digits (`[1]`, `[2]`, …) *are* the assigned footnote number, so
/// this builder must call [`Parser::footnote_index_for_id`] /
/// [`Parser::define_footnote`] – the same document-counter-advancing calls
/// the string replacer makes – or the differential corpus below could never
/// pass. The two code paths never share a `Parser` (each independently
/// numbers footnotes over the same source in the same left-to-right order),
/// so this never double-counts a registration; see the module's test helpers.
///
/// The registered catalog `text` is a best-effort rendering of the raw
/// bracket content (normalized, but not folded through a renderer – building
/// the tree must not itself invoke one), so – like every other deferred
/// registration in this module – a tree-built footnote's
/// `Document::catalog().footnotes()` entry is not yet byte-faithful; only the
/// returned *number*, this pass's one load-bearing side effect, is relied on.
///
/// # Deferred forms
///
/// - Content containing an escaped closing bracket (`\]`): unescaping it would
///   mean splicing a literal `]` into the middle of a `Text` piece the content
///   range slices, which – unlike a single-node substitution such as `xref`'s
///   escaped text – would require rebuilding part of the content's own node
///   structure around the splice; deferred, exactly as the anchor and
///   cross-reference macros defer their own escaped-bracket forms when they
///   cannot represent them without a similar rebuild (divergence test:
///   `a_footnote_with_an_escaped_bracket_is_a_documented_divergence`).
fn build_footnote_node<'src>(
    caps: &regex::Captures<'_>,
    full: &std::ops::Range<usize>,
    s: &str,
    pieces: &[Piece],
    nodes: &[InlineNode<'src>],
    root: Span<'src>,
    parser: &Parser,
) -> Option<InlineNode<'src>> {
    let location = source_slice(pieces, full.clone(), root);

    // The id (`[\w-]+`) admits no special character or opaque span, so a
    // captured id is always verbatim and borrows `'src`; `id` is `None` only
    // when the macro carries none (an anonymous `footnote:[…]`).
    let id: Option<CowStr<'src>> = caps
        .get(2)
        .map(|m| CowStr::from(source_slice(pieces, m.start()..m.end(), root).data()));

    let content_match = caps.get(3);

    if let Some(id) = id {
        if let Some(number) = parser.footnote_index_for_id(id.as_ref()) {
            // A reference to an already-defined footnote: reuse its number.
            return Some(InlineNode::Footnote(Footnote {
                id: Some(id),
                number: Some(CowStr::from(number)),
                is_reference: true,
                children: vec![],
                location,
            }));
        }

        return match content_match {
            Some(content) => {
                // A defining occurrence that also carries an id.
                if s[content.range()].contains("\\]") {
                    return None;
                }

                let children = footnote_children(content.range(), pieces, nodes);
                let number = register_footnote_number(
                    parser,
                    Some(id.as_ref()),
                    &s[content.range()],
                    location,
                );

                Some(InlineNode::Footnote(Footnote {
                    id: Some(id),
                    number: Some(CowStr::from(number)),
                    is_reference: false,
                    children,
                    location,
                }))
            }

            // A reference to an id that was never defined. The string replacer
            // warns here (`InvalidFootnoteReference`); this pass, like every
            // other diagnostic the additive builder skips, leaves that to the
            // cutover.
            None => Some(InlineNode::Footnote(Footnote {
                id: Some(id),
                number: None,
                is_reference: true,
                children: vec![],
                location,
            })),
        };
    }

    // An anonymous defining occurrence.
    let content = content_match?;

    if s[content.range()].contains("\\]") {
        return None;
    }

    let children = footnote_children(content.range(), pieces, nodes);
    let number = register_footnote_number(parser, None, &s[content.range()], location);

    Some(InlineNode::Footnote(Footnote {
        id: None,
        number: Some(CowStr::from(number)),
        is_reference: false,
        children,
        location,
    }))
}

/// Builds one [`Footnote`](InlineNode::Footnote) node from a `footnoteref:`
/// match – the deprecated form's own counterpart to [`build_footnote_node`].
/// Unlike `footnote:id[…]`, which takes its id from the macro target,
/// `footnoteref:[id,text]` / `footnoteref:[id]` packs both into one bracket
/// (`raw`, group 3 of [`INLINE_FOOTNOTE_MACRO`]), split on the **first**
/// comma – mirroring `InlineFootnoteMacroReplacer`'s own `raw.split_once(',')`
/// exactly, including that an id is *always* present (a bracket with no
/// comma is the id alone, with no text – a bare reference) and that a
/// trailing comma (`footnoteref:[id,]`) yields an *empty*, not absent,
/// content (a defining occurrence with empty text), unlike `footnote:id[]`'s
/// own no-comma-at-all "reference" shape. Once split, the (id, content) pair
/// resolves through the exact same three cases
/// [`build_footnote_node`] does (reuse an already-defined id's number, define
/// a new id-carrying occurrence, or fall back to an unresolved reference) –
/// see that function's own doc comment for the shared reasoning (the
/// required `footnote_index_for_id`/`define_footnote` side effect, and why a
/// content-side `\]` is deferred, which applies here identically since `raw`
/// is matched by the very same bracket group).
///
/// The one thing this increment does **not** yet do is the deprecation
/// warning `InlineFootnoteMacroReplacer` records outside `compat-mode`: like
/// every other macro family's own catalog/warning side effect, that is a
/// diagnostic that does not change the fold's output bytes, so – unlike the
/// footnote number itself – it is left to the cutover (design §5.2 Phase 4,
/// step 6) rather than performed here.
fn build_footnoteref_node<'src>(
    raw: regex::Match<'_>,
    full: &std::ops::Range<usize>,
    s: &str,
    pieces: &[Piece],
    nodes: &[InlineNode<'src>],
    root: Span<'src>,
    parser: &Parser,
) -> Option<InlineNode<'src>> {
    let location = source_slice(pieces, full.clone(), root);

    if s[raw.range()].contains("\\]") {
        return None;
    }

    // Split on the first comma: `id` is everything before it (the whole raw
    // text when there is no comma at all), `content` is everything after it
    // (present, possibly empty, only when a comma was found).
    let (id_range, content_range) = match s[raw.range()].find(',') {
        Some(offset) => (
            raw.start()..(raw.start() + offset),
            Some((raw.start() + offset + 1)..raw.end()),
        ),

        None => (raw.range(), None),
    };

    let id = CowStr::from(source_slice(pieces, id_range, root).data());

    if let Some(number) = parser.footnote_index_for_id(id.as_ref()) {
        // A reference to an already-defined footnote: reuse its number.
        return Some(InlineNode::Footnote(Footnote {
            id: Some(id),
            number: Some(CowStr::from(number)),
            is_reference: true,
            children: vec![],
            location,
        }));
    }

    match content_range {
        Some(content_range) => {
            // A defining occurrence that also carries an id.
            let children = footnote_children(content_range.clone(), pieces, nodes);
            let number =
                register_footnote_number(parser, Some(id.as_ref()), &s[content_range], location);

            Some(InlineNode::Footnote(Footnote {
                id: Some(id),
                number: Some(CowStr::from(number)),
                is_reference: false,
                children,
                location,
            }))
        }

        // A reference to an id that was never defined. The string replacer
        // warns here (`InvalidFootnoteReference`); this pass, like every
        // other diagnostic the additive builder skips, leaves that to the
        // cutover.
        None => Some(InlineNode::Footnote(Footnote {
            id: Some(id),
            number: None,
            is_reference: true,
            children: vec![],
            location,
        })),
    }
}

/// Builds a footnote's `children` from its bracket content's match-string
/// `range` via [`emit_range`] – *not*
/// [`range_is_verbatim`](super::macros::image::range_is_verbatim) the way every
/// other macro family's target/text slicing does. A footnote's content
/// becomes structured children rather than a literal attribute value, so a
/// range crossing an already-recognized construct is not a boundary to defer
/// on; it is the whole point – `emit_range` clones that construct's node
/// whole into the footnote's subtree, exactly mirroring how the string
/// pipeline's footnote text captures an already-substituted macro verbatim
/// (see [`apply_footnotes`]'s doc comment on ordering). This uses the plain
/// [`emit_range`], not the recursing [`emit_range_recursing_footnotes`] –
/// footnotes do not nest (the string pipeline's own lazy bracket match cannot
/// recognize one inside another's content either, since it always stops at
/// the *first* unescaped `]`), so a footnote's own content is captured as-is.
fn footnote_children<'src>(
    range: std::ops::Range<usize>,
    pieces: &[Piece],
    nodes: &[InlineNode<'src>],
) -> Vec<InlineNode<'src>> {
    let mut children = Vec::new();
    emit_range(nodes, pieces, range, &mut children);
    children
}

/// Registers a footnote's defining occurrence with the parser, advancing the
/// `footnote-number` counter and returning the assigned number – the one
/// recognition side effect [`build_footnote_node`]'s doc comment explains this
/// pass must perform. `raw_content` is normalized exactly as the string
/// replacer normalizes it ([`normalize_footnote_text`]: trimmed, embedded
/// newlines collapsed) before being registered as the catalog's best-effort
/// `text`; no cross-reference placeholders are threaded through (the additive
/// builder has none to give it – a `Ref{Xref}` is a node, not a placeholder),
/// so `define_footnote` never builds a `FootnoteDeferred` for a tree-built
/// footnote.
fn register_footnote_number(
    parser: &Parser,
    id: Option<&str>,
    raw_content: &str,
    location: Span<'_>,
) -> String {
    let text = normalize_footnote_text(raw_content);
    parser.define_footnote(id, text, vec![], location)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use super::super::test_support::{
        assert_link, assert_styled, assert_text, build_src, fold_html, golden_macros,
    };
    use crate::{
        Span,
        inlines::{Footnote, InlineNode, SpanForm, StyleVariant},
        parser::HtmlSubstitutionRenderer,
        strings::CowStr,
    };

    /// Asserts that `node` is a [`Footnote`](InlineNode::Footnote), returning
    /// it for further inspection.
    fn assert_footnote<'a, 'src>(node: &'a InlineNode<'src>) -> &'a Footnote<'src> {
        match node {
            InlineNode::Footnote(footnote) => footnote,

            other => panic!("expected a Footnote, got {other:?}"),
        }
    }

    #[test]
    fn fold_matches_the_string_pipeline_through_footnotes() {
        // For each fixture, folding the single-pass tree (all five steps)
        // reproduces the string pipeline's output byte-for-byte. This is the
        // differential corpus (design §5.3) that pins the footnote increment –
        // the last of the macro families (part 4c). Each fixture uses its own
        // pair of *independent* default parsers (one inside `build_src`, one
        // inside `golden_macros`), so the `footnote-number` counter each one
        // advances never crosses over; as long as both recognize the same
        // occurrences in the same left-to-right order, their numbering stays in
        // lockstep.
        let fixtures = [
            // No footnote despite macro-ish characters.
            "plain text without a footnote",
            "a footnote without a bracket footnote:foo stays literal",
            // `footnote:[]` (neither an id nor content) is not a footnote at
            // all – left untouched by both the string replacer and the builder.
            "footnote:[]",
            // An anonymous defining occurrence, and one whose text needs no
            // further substitution.
            "footnote:[the evidence]",
            "A claim.footnote:[the evidence]",
            // A defining occurrence with an id, and a reference that reuses its
            // number.
            "footnote:disc[a discussion]",
            "Named.footnote:disc[a discussion] then footnote:disc[].",
            // A reference to an id that was never defined (the unresolved
            // fallback).
            "See footnote:missing[] here.",
            // Multiple anonymous footnotes number in document order.
            "one footnote:[a] two footnote:[b] three footnote:[c]",
            // Id character classes: `_`, `-`, digits.
            "footnote:my_id-1[text]",
            // Content already carrying a rendered construct from an earlier
            // pass at this level – a formatting span, a character replacement,
            // an image, a link, an index term, and (since footnotes run last)
            // a cross-reference – is captured as that construct's node, not
            // re-recognized. None of this affects the fold: only the marker
            // (not the footnote's text) reaches the flow.
            "footnote:[the *strong* evidence]",
            "footnote:[a copyright (C) note]",
            "footnote:[see image:x.png[X]]",
            "footnote:[see link:https://example.org[source]]",
            "footnote:[an index (((term))) here]",
            "footnote:[see xref:install[the guide]]",
            "footnote:[a < b]",
            // A macro embedded in surrounding flow, and next to other
            // constructs.
            "See footnote:[a note] here.",
            "*bold* then footnote:[fn] and _em_",
            // An already-recognized construct as an *unrelated sibling* gap at
            // the same level as a genuine footnote match – not nested inside
            // the footnote's own content – exercising
            // `emit_range_recursing_footnotes`'s `Ref` and non-`Styled`/`Ref`
            // (`other`) branches, which nothing above reaches (every other
            // fixture's nested construct sits *inside* a footnote's own
            // content, built by the plain, non-recursing `emit_range`).
            "image:x.png[X] footnote:[a note]",
            "link:https://example.org[text] footnote:[a note]",
            // Escape: the macro stays literal, minus the backslash.
            "\\footnote:[not a footnote]",
            "\\footnote:disc[not a footnote]",
            // A footnote inside a rendered span (recognized inside the body).
            "*see footnote:[fn]*",
            "_footnote:x[fn] in em_",
            // The deprecated `footnoteref:` form: an anonymous-looking id+text
            // defining occurrence, a bare reference to an id defined the
            // ordinary way, a trailing comma (empty, not absent, content), the
            // empty-bracket non-match, and the escape.
            "footnoteref:[disc,a discussion]",
            "footnote:disc[a discussion] then footnoteref:[disc].",
            "footnoteref:[missing]",
            "footnoteref:[disc,]",
            "footnoteref:[]",
            "\\footnoteref:[disc,a discussion]",
            "footnoteref:[disc,the *strong* evidence]",
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
    fn an_anonymous_footnote_becomes_a_node() {
        let nodes = build_src(Span::new("footnote:[the evidence]"));

        assert_eq!(nodes.len(), 1);
        let footnote = assert_footnote(&nodes[0]);

        assert!(footnote.id.is_none());
        assert_eq!(footnote.number.as_deref(), Some("1"));
        assert!(!footnote.is_reference);

        assert_eq!(footnote.location.data(), "footnote:[the evidence]");
        assert_eq!(footnote.location.line(), 1);
        assert_eq!(footnote.location.col(), 1);

        // The content becomes a single borrowed `Text` child, located at its
        // source (`footnote:[` is 10 characters, so the text starts at column
        // 11).
        assert_eq!(footnote.children.len(), 1);
        assert_text(&footnote.children[0], "the evidence", 1, 11);
    }

    #[test]
    fn a_footnote_with_an_id_becomes_a_node() {
        let nodes = build_src(Span::new("footnote:disc[a discussion]"));

        let footnote = assert_footnote(&nodes[0]);

        // The id borrows from source (no allocation).
        assert!(matches!(footnote.id, Some(CowStr::Borrowed(_))));
        assert_eq!(footnote.id.as_deref(), Some("disc"));
        assert_eq!(footnote.number.as_deref(), Some("1"));
        assert!(!footnote.is_reference);
        assert_text(&footnote.children[0], "a discussion", 1, 15);
    }

    #[test]
    fn a_footnote_reference_reuses_the_defining_number() {
        let source = "footnote:disc[a discussion] then footnote:disc[].";
        let nodes = build_src(Span::new(source));

        // [Footnote(defining), Text(" then "), Footnote(reference), Text(".")].
        assert_eq!(nodes.len(), 4);

        let defining = assert_footnote(&nodes[0]);
        assert!(!defining.is_reference);
        assert_eq!(defining.number.as_deref(), Some("1"));

        let reference = assert_footnote(&nodes[2]);
        assert!(reference.is_reference);
        assert_eq!(reference.number.as_deref(), Some("1"));
        assert_eq!(reference.id.as_deref(), Some("disc"));
    }

    #[test]
    fn a_footnote_reference_keeps_an_empty_subtree() {
        let source = "footnote:disc[a discussion] then footnote:disc[].";
        let nodes = build_src(Span::new(source));

        let reference = assert_footnote(&nodes[2]);
        assert!(reference.children.is_empty());
    }

    #[test]
    fn an_unresolved_footnote_reference_falls_back_to_its_id() {
        let nodes = build_src(Span::new("footnote:missing[]"));

        let footnote = assert_footnote(&nodes[0]);
        assert!(footnote.is_reference);
        assert!(footnote.number.is_none());
        assert_eq!(footnote.id.as_deref(), Some("missing"));
        assert!(footnote.children.is_empty());
    }

    #[test]
    fn anonymous_footnotes_number_in_document_order() {
        let nodes = build_src(Span::new("one footnote:[a] two footnote:[b] three"));

        let first = assert_footnote(&nodes[1]);
        assert_eq!(first.number.as_deref(), Some("1"));

        let second = assert_footnote(&nodes[3]);
        assert_eq!(second.number.as_deref(), Some("2"));
    }

    #[test]
    fn a_footnote_carries_its_text_as_child_nodes() {
        let nodes = build_src(Span::new("footnote:[the *strong* evidence]"));
        let footnote = assert_footnote(&nodes[0]);

        assert_eq!(footnote.children.len(), 3);
        assert_text(&footnote.children[0], "the ", 1, 11);
        assert_styled(
            &footnote.children[1],
            StyleVariant::Strong,
            SpanForm::Constrained,
        );
        assert_text(&footnote.children[2], " evidence", 1, 23);
    }

    #[test]
    fn a_footnote_subtree_carries_a_nested_link() {
        // The `link:` macro runs before the footnote pass in `apply_macros`
        // (mirroring the string step's order), so by the time the footnote's
        // content is captured, the link is already a `Ref` node – captured
        // whole into the footnote's children, not re-recognized from its
        // source text.
        let nodes = build_src(Span::new("footnote:[see link:https://example.org[source]]"));
        let footnote = assert_footnote(&nodes[0]);

        assert_eq!(footnote.children.len(), 2);
        assert_text(&footnote.children[0], "see ", 1, 11);
        assert_link(&footnote.children[1]);
    }

    #[test]
    fn an_empty_footnote_macro_stays_literal() {
        // `footnote:[]` carries neither an id nor content, so it is not a
        // footnote at all – left as literal text, exactly as the string
        // replacer's `next $&` branch leaves it.
        let nodes = build_src(Span::new("footnote:[]"));

        assert_eq!(nodes.len(), 1);
        assert_text(&nodes[0], "footnote:[]", 1, 1);
    }

    #[test]
    fn a_deprecated_footnoteref_macro_with_an_id_and_text_is_recognized() {
        // The deprecated `footnoteref:[id,text]` form packs its id and text
        // into one bracket, split on the first comma – a different split
        // from `footnote:id[text]`'s own (id from the macro target, text
        // from the bracket) – but resolves into the same node shape and
        // folds through the same `render_footnote`, so its output is
        // byte-for-byte identical to the golden pipeline's (the deprecation
        // warning itself remains deferred to the cutover; it does not affect
        // the fold's output bytes – see `build_footnoteref_node`'s doc
        // comment).
        let source = "footnoteref:[disc,a discussion]";
        let folded = fold_html(&build_src(Span::new(source)), &HtmlSubstitutionRenderer {});

        assert_eq!(folded, golden_macros(source));

        let nodes = build_src(Span::new(source));
        let footnote = assert_footnote(&nodes[0]);
        assert_eq!(footnote.id.as_deref(), Some("disc"));
        assert_eq!(footnote.number.as_deref(), Some("1"));
        assert!(!footnote.is_reference);
        assert_eq!(footnote.children.len(), 1);
        assert_text(&footnote.children[0], "a discussion", 1, 19);
    }

    #[test]
    fn a_deprecated_footnoteref_macro_referencing_an_id_reuses_its_number() {
        // A comma-free `footnoteref:[id]` is a bare reference to an
        // already-defined footnote – the same shape `footnote:id[]` produces,
        // just spelled the deprecated way.
        let source = "footnote:disc[a discussion] then footnoteref:[disc].";
        let folded = fold_html(&build_src(Span::new(source)), &HtmlSubstitutionRenderer {});

        assert_eq!(folded, golden_macros(source));

        let nodes = build_src(Span::new(source));
        let reference = assert_footnote(&nodes[2]);
        assert!(reference.is_reference);
        assert_eq!(reference.number.as_deref(), Some("1"));
        assert_eq!(reference.id.as_deref(), Some("disc"));
        assert!(reference.children.is_empty());
    }

    #[test]
    fn a_deprecated_footnoteref_macro_referencing_an_undefined_id_falls_back_to_it() {
        // A comma-free `footnoteref:[id]` whose id was never defined resolves
        // through the same unresolved fallback `footnote:id[]` does (the
        // string replacer's own `InvalidFootnoteReference` warning is a
        // diagnostic, deferred to the cutover like every other one this
        // builder skips).
        let source = "footnoteref:[missing]";
        let folded = fold_html(&build_src(Span::new(source)), &HtmlSubstitutionRenderer {});

        assert_eq!(folded, golden_macros(source));

        let nodes = build_src(Span::new(source));
        let footnote = assert_footnote(&nodes[0]);
        assert!(footnote.is_reference);
        assert!(footnote.number.is_none());
        assert_eq!(footnote.id.as_deref(), Some("missing"));
        assert!(footnote.children.is_empty());
    }

    #[test]
    fn a_deprecated_footnoteref_macro_with_a_trailing_comma_has_empty_content() {
        // `footnoteref:[id,]` splits into an id and an *empty* content
        // (present, not absent) – a defining occurrence with empty text,
        // unlike the no-comma-at-all `footnoteref:[id]` reference shape.
        let source = "footnoteref:[disc,]";
        let folded = fold_html(&build_src(Span::new(source)), &HtmlSubstitutionRenderer {});

        assert_eq!(folded, golden_macros(source));

        let nodes = build_src(Span::new(source));
        let footnote = assert_footnote(&nodes[0]);
        assert!(!footnote.is_reference);
        assert!(footnote.children.is_empty());
    }

    #[test]
    fn an_empty_footnoteref_macro_stays_literal() {
        // `footnoteref:[]` carries no bracketed text at all, so it is left
        // unrecognized – mirroring the string replacer's `next $&` branch,
        // the same way `footnote:[]` is.
        let nodes = build_src(Span::new("footnoteref:[]"));

        assert_eq!(nodes.len(), 1);
        assert_text(&nodes[0], "footnoteref:[]", 1, 1);
    }

    #[test]
    fn an_escaped_footnoteref_macro_drops_the_backslash() {
        let source = "\\footnoteref:[disc,a discussion]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Footnote(_))),
            "an escaped footnoteref: macro must not produce a Footnote: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn a_footnoteref_macro_with_an_escaped_bracket_is_a_documented_divergence() {
        // The same escaped-closing-bracket boundary
        // `a_footnote_with_an_escaped_bracket_is_a_documented_divergence`
        // documents for `footnote:`, since `build_footnoteref_node` checks
        // its own captured `raw` text the same way.
        let source = "footnoteref:[disc,a note ending in a\\]bracket]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Footnote(_))),
            "a footnoteref: macro with an escaped bracket must be left unrecognized: {nodes:?}"
        );

        assert!(golden_macros(source).contains("class=\"footnote\""));
    }

    #[test]
    fn a_footnote_with_an_escaped_bracket_is_a_documented_divergence() {
        // Content carrying an escaped closing bracket (`\]`) needs it unescaped
        // to a literal `]` in the middle of the content's own node structure –
        // deferred, exactly as the anchor and cross-reference macros defer
        // their own escaped-bracket forms when they cannot represent the
        // unescape without a similar rebuild. Both the anonymous form and the
        // id-carrying form share the same check (see `build_footnote_node`),
        // so both are exercised here.
        for source in [
            "footnote:[a note ending in a\\]bracket]",
            "footnote:disc[a note ending in a\\]bracket]",
        ] {
            let nodes = build_src(Span::new(source));

            assert!(
                nodes.iter().all(|n| !matches!(n, InlineNode::Footnote(_))),
                "a footnote with an escaped bracket must be left unrecognized: {nodes:?}"
            );

            // The string pipeline, by contrast, does build a footnote marker here.
            assert!(golden_macros(source).contains("class=\"footnote\""));
        }
    }

    #[test]
    fn footnotes_number_in_source_order_across_nesting() {
        // A footnote preceding a `Styled` span must be numbered *before* a
        // footnote nested inside that span, even though `apply_macros`'s
        // depth-first recursion (see `apply_footnotes`'s doc comment)
        // resolves the span's children before the outer level. This is the
        // regression the `apply_footnotes`/`rebuild_footnote_level` split
        // fixes: numbering must follow true left-to-right source order, not
        // tree depth.
        let source = "footnote:[outer] *footnote:[inner]*";
        let folded = fold_html(&build_src(Span::new(source)), &HtmlSubstitutionRenderer {});

        assert_eq!(folded, golden_macros(source));

        let nodes = build_src(Span::new(source));
        let outer = assert_footnote(&nodes[0]);
        assert_eq!(outer.number.as_deref(), Some("1"));

        let inner_children = assert_styled(&nodes[2], StyleVariant::Strong, SpanForm::Constrained);
        let inner = assert_footnote(&inner_children[0]);
        assert_eq!(inner.number.as_deref(), Some("2"));
    }

    #[test]
    fn footnotes_number_in_source_order_across_a_footnote_bearing_link() {
        // The opposite nesting from the test above: a *link* – a macro-built
        // `Ref` node, created *during* `apply_macros` rather than already
        // existing by the time it runs like a quotes-built `Styled` span –
        // nested inside a footnote's own content. This is the valid
        // direction to nest the two: because footnotes run *last* (after
        // `link_macro_level`), the link's brackets are already consumed into
        // a `Ref` node (no literal `[`/`]` left to collide with) by the time
        // the footnote's own lazy bracket match runs, unlike the reverse
        // nesting (see `a_footnote_nested_in_link_text_is_a_documented_divergence`
        // just below, which explains why that direction can never be clean).
        let source = "footnote:[outer] footnote:[see link:https://example.org[inner]]";
        let folded = fold_html(&build_src(Span::new(source)), &HtmlSubstitutionRenderer {});

        assert_eq!(folded, golden_macros(source));

        let nodes = build_src(Span::new(source));
        let outer = assert_footnote(&nodes[0]);
        assert_eq!(outer.number.as_deref(), Some("1"));

        let inner = assert_footnote(&nodes[2]);
        assert_eq!(inner.number.as_deref(), Some("2"));
        assert_link(&inner.children[1]);
    }

    #[test]
    fn a_footnote_nested_in_link_text_is_a_documented_divergence() {
        // `link:` matches its bracketed label *lazily* – up to the first
        // unescaped `]` – so an unbalanced inner bracket like
        // `footnote:[note]` (itself containing a `]`) truncates the link's
        // own match early, leaving `footnote:[note` as the link's label and a
        // stray `]` as literal text after it (mirrored exactly by the
        // builder's `link_macro_level`, which shares the same regex – see the
        // `fold_matches_the_string_pipeline_through_link_macros` corpus for
        // that shared behavior on its own). This is not specific to links: a
        // footnote nested inside *any* bracket-delimited macro argument that
        // runs *before* footnotes (image, index terms, anchors,
        // cross-references – every family footnotes.rs runs after) hits the
        // same collision, since footnote syntax's own `[`/`]` inherently
        // satisfies that macro's lazy closing-bracket search before it
        // reaches its intended one. It is fundamentally the reverse of the
        // clean nesting
        // `footnotes_number_in_source_order_across_a_footnote_bearing_link`
        // exercises: nesting an *earlier*-running macro inside a footnote's
        // content is fine (by the time footnotes run, that macro's brackets
        // are already consumed into a node), but nesting *footnote* syntax
        // inside an earlier macro's argument is not, in the string pipeline
        // or otherwise.
        //
        // In the *string* pipeline, the footnote pass runs over the
        // *already-rendered* flat string, where the link's `</a>` is just
        // literal text no different from any other – so the footnote regex,
        // finding only one `]` left in the whole string, matches straight
        // through the `</a>` and consumes it as part of its own (nonsensical)
        // content, producing malformed, never-closed markup. That is a direct
        // consequence of matching over *rendered* text rather than structure
        // – exactly the class of divergence
        // `crossed_delimiters_are_a_documented_divergence` documents for
        // quotes. A `Ref` node's closing tag is never a `Text` node in the
        // tree (it exists only as fold *output*, not as node content), so
        // the builder's footnote pass has no way to "reach into" it, and
        // correctly does not: the link's label stays literal, unrecognized
        // text, and the tree never reproduces the string pipeline's
        // malformed markup here.
        let source = "link:https://example.org[footnote:[note]]";
        let nodes = build_src(Span::new(source));

        let link = assert_link(&nodes[0]);
        assert_text(&link.children[0], "footnote:[note", 1, 26);

        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});
        assert_eq!(
            folded,
            "<a href=\"https://example.org\">footnote:[note</a>]"
        );

        // The string pipeline, by contrast, produces unclosed markup: the
        // link's own closing `</a>` is consumed into the footnote's content,
        // and the footnote's own numbered marker (with its *own*, unrelated
        // `<a>…</a>`) takes its place.
        assert_eq!(
            golden_macros(source),
            "<a href=\"https://example.org\"><sup class=\"footnote\">\
             [<a id=\"_footnoteref_1\" class=\"footnote\" href=\"#_footnotedef_1\" \
             title=\"View footnote.\">1</a>]</sup>"
        );
    }
}
