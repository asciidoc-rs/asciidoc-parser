//! The footnote substitution step.

use super::{
    fold_deferring_xrefs,
    macros::{emit_range_unescaping_brackets, image::range_has_no_opaque_piece},
    quotes::{Piece, build_match_string, source_slice, text_slice},
    special_chars::Masked,
};
use crate::{
    Parser, Span,
    content::INLINE_FOOTNOTE_MACRO,
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
/// exception — it is a side effect of recognition order itself (see
/// [`build_footnote_node`]'s doc comment) — so depth-first recursion would
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
/// before the first / after the last) — see [`rebuild_footnote_level`] and
/// [`emit_range_recursing_footnotes`], which recurse in place of the generic
/// [`rebuild_macro_level`](super::macros::rebuild_macro_level) /
/// [`emit_range`](super::quotes::emit_range)'s verbatim clone.
pub(super) fn apply_footnotes<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    // A subtree with no `"tnote"` substring anywhere — the overwhelmingly
    // common case — has nothing for this pass to do, so return it completely
    // untouched: no clone, no rebuild. This check is not just an optimization.
    // `rebuild_footnote_level`'s gap emission clones *every* atomic piece
    // (any already-recognized `Image`/`Ui`/`Ref`/`Anchor`/`IndexTerm`/`Styled`
    // node) it touches — including ones with nothing to do with footnotes —
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

    let (s, pieces) = build_match_string(&nodes, Masked::UNKNOWN);

    // Cheap pre-filter mirroring the string step's `found_macroish &&
    // text.contains("tnote")`: both `footnote:` and the deprecated
    // `footnoteref:` contain "tnote". Even when this level's *own* text has
    // no match, a `Styled`/`Ref` child might still carry one in its own
    // subtree (that is what the pre-check above just confirmed), so the
    // rebuild below runs regardless — it is what performs the recursion.
    let matches = if s.contains("tnote") {
        find_footnote_matches(&s)
    } else {
        Vec::new()
    };

    rebuild_footnote_level(&nodes, &pieces, &s, matches, root, parser)
}

/// Reports whether `nodes`, or any
/// [`Styled`](crate::inlines::Styled)/[`Ref`](crate::inlines::Ref) descendant's
/// own subtree at any depth, might carry the `"tnote"` substring
/// [`apply_footnotes`]'s pre-filter looks for. A read-only recursive check —
/// it builds a match string per level exactly as [`apply_footnotes`] does
/// (so it cannot miss a `"tnote"` split across two adjacent
/// [`Text`](InlineNode::Text) pieces the way a per-node substring check
/// could) but allocates no [`InlineNode`], letting a subtree with nothing to
/// find come back from `apply_footnotes` completely unchanged.
fn subtree_might_have_footnote(nodes: &[InlineNode<'_>]) -> bool {
    let (s, _) = build_match_string(nodes, Masked::UNKNOWN);

    if s.contains("tnote") {
        return true;
    }

    nodes.iter().any(|node| match node {
        InlineNode::Styled(styled) => subtree_might_have_footnote(&styled.children),
        InlineNode::Ref(reference) => subtree_might_have_footnote(&reference.children),
        InlineNode::IndexTerm(index_term) => subtree_might_have_footnote(&index_term.children),
        _ => false,
    })
}

/// Like [`rebuild_macro_level`](super::macros::rebuild_macro_level), but for
/// [`apply_footnotes`]: rebuilds a level's node list from its footnote
/// matches, using [`emit_range_recursing_footnotes`] (not
/// [`emit_range`](super::quotes::emit_range)) for every gap, so a
/// [`Styled`](crate::inlines::Styled)/[`Ref`](crate::inlines::Ref)/
/// [`IndexTerm`](crate::inlines::IndexTerm) child encountered between two
/// matches is recursed into — in source order — rather than cloned whole.
///
/// Each candidate's node is **built here**, immediately after the gap that
/// precedes it, rather than during the scan (see [`FootnoteMatch`]). That is
/// what puts the numbers in the string pipeline's own order: a footnote nested
/// in a child that falls between two of this level's is numbered between them,
/// because the gap carrying that child is emitted — and recursed into — before
/// this match's own node is made.
///
/// A candidate that turns out to be unrecognized advances the cursor no
/// further than its own start, so its text joins the following gap exactly as
/// it did when the scan dropped such a match: the same bytes, emitted by the
/// same range walk.
fn rebuild_footnote_level<'src>(
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    s: &str,
    matches: Vec<FootnoteMatch<'_>>,
    root: Span<'src>,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    let mut out = Vec::new();
    let mut cursor = 0usize;

    for m in matches {
        match m {
            FootnoteMatch::Unescape { full, backslash } => {
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

                cursor = full.end;
            }

            FootnoteMatch::Candidate { full, caps } => {
                emit_range_recursing_footnotes(
                    nodes,
                    pieces,
                    cursor..full.start,
                    root,
                    parser,
                    &mut out,
                );

                cursor = full.start;

                if let Some(node) =
                    build_candidate_node(&caps, &full, s, pieces, nodes, root, parser)
                {
                    out.push(node);
                    cursor = full.end;
                }
            }
        }
    }

    if cursor < s.len() {
        emit_range_recursing_footnotes(nodes, pieces, cursor..s.len(), root, parser, &mut out);
    }

    out
}

/// Like [`emit_range`](super::quotes::emit_range), but recurses into a
/// [`Styled`](crate::inlines::Styled)/[`Ref`](crate::inlines::Ref)/
/// [`IndexTerm`](crate::inlines::IndexTerm) piece's children with
/// [`apply_footnotes`] instead of cloning the node whole — the piece that makes
/// [`apply_footnotes`] a true whole-tree, source-order walk rather than a
/// single level pass. Every other aspect (slicing a verbatim or synthesized
/// [`Text`](InlineNode::Text) run at the overlap, an empty range
/// emitting nothing) is identical.
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

                // A **visible** index term's shown text reaches the flow, so
                // the string replacer's footnote pass scans it like any other
                // text — the same reason the later macro families are handed
                // that text (see the index-term family's own note).
                //
                // An [`Anchor`](InlineNode::Anchor)'s `reftext` is the
                // opposite case and is deliberately *not* recursed into: the
                // anchor replacer consumes that text rather than emitting it,
                // so a `footnote:[…]` written there never reaches the string
                // pipeline's footnote pass either.
                InlineNode::IndexTerm(mut index_term) => {
                    index_term.children = apply_footnotes(index_term.children, root, parser);
                    out.push(InlineNode::IndexTerm(index_term));
                }

                other => out.push(other),
            }

            continue;
        }

        // A verbatim or synthesized text run: slice it to the overlap (see
        // [`emit_range`](super::quotes::emit_range)'s own synthesized branch).
        let lo = range.start.max(p_start) - p_start;
        let hi = range.end.min(p_end) - p_start;

        if let InlineNode::Text { value, location } = node {
            if piece.synthesized {
                let Some(sliced) = value.get(lo..hi) else {
                    continue;
                };

                out.push(InlineNode::Text {
                    value: CowStr::from(sliced.to_string()),
                    location: *location,
                });
            } else {
                let sliced = location.slice(lo..hi);

                out.push(InlineNode::Text {
                    value: CowStr::from(sliced.data()),
                    location: sliced,
                });
            }
        }
    }
}

/// One `INLINE_FOOTNOTE_MACRO` occurrence at this level, recognized but **not
/// yet built**.
///
/// Deferring construction is what keeps the numbers right. A footnote's
/// assigned number is a side effect of *recognition order*, and the string
/// pipeline recognizes in one left-to-right sweep over one flat string — so a
/// footnote nested in a [`Styled`](crate::inlines::Styled) span that sits
/// between two of this level's own footnotes must be numbered between them.
/// Building every match up front (as this scan used to) assigns all of this
/// level's numbers before the rebuild walk descends into any child, which
/// reverses exactly that pair. Carrying the capture instead lets
/// [`rebuild_footnote_level`] build each node at the moment its walk reaches
/// it, after the gap before it — children and all — has been emitted.
enum FootnoteMatch<'h> {
    /// An escape (`\footnote:…`, `\footnoteref:…`): the backslash is dropped
    /// and the rest kept literal, mirroring the string replacer's leading
    /// `caps[0].starts_with('\\')` check — which runs *before* the
    /// ref-vs-plain branch, so this is decided during the scan, not at build
    /// time. It creates no node and needs no number.
    Unescape {
        full: std::ops::Range<usize>,
        backslash: usize,
    },

    /// A candidate whose node — and therefore whose number — is produced when
    /// the rebuild walk reaches it. It may still turn out to be one of the
    /// forms this family leaves unrecognized, in which case its own text stays
    /// literal, exactly as it did when the scan decided that.
    Candidate {
        full: std::ops::Range<usize>,
        caps: regex::Captures<'h>,
    },
}

/// Finds every footnote occurrence at this level, without building any of them
/// — see [`FootnoteMatch`] for why construction is deferred.
fn find_footnote_matches(s: &str) -> Vec<FootnoteMatch<'_>> {
    let mut matches = Vec::new();

    for caps in INLINE_FOOTNOTE_MACRO.captures_iter(s) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();

        let full = whole.start()..whole.end();

        if whole.as_str().starts_with('\\') {
            matches.push(FootnoteMatch::Unescape {
                backslash: full.start,
                full,
            });

            continue;
        }

        matches.push(FootnoteMatch::Candidate { full, caps });
    }

    matches
}

/// Builds the [`Footnote`](InlineNode::Footnote) node one
/// [`Candidate`](FootnoteMatch::Candidate) stands for, assigning its number
/// here — at the point [`rebuild_footnote_level`]'s source-order walk reaches
/// it — or `None` for the forms this family leaves unrecognized.
#[allow(clippy::too_many_arguments)]
fn build_candidate_node<'src>(
    caps: &regex::Captures<'_>,
    full: &std::ops::Range<usize>,
    s: &str,
    pieces: &[Piece],
    nodes: &[InlineNode<'src>],
    root: Span<'src>,
    parser: &Parser,
) -> Option<InlineNode<'src>> {
    // The deprecated `footnoteref:[id,text]` / `footnoteref:[id]` form
    // (group 1) packs its id and text into one bracket, split on the first
    // comma, rather than taking the id from the macro target the way
    // `footnote:id[…]` does.
    if caps.get(1).is_some() {
        // With no bracketed text at all (`footnoteref:[]`), it is left
        // unrecognized — mirroring the string replacer's `next $&`.
        let raw = caps.get(3)?;

        return build_footnoteref_node(raw, full, s, pieces, nodes, root, parser);
    }

    build_footnote_node(caps, full, s, pieces, nodes, root, parser)
}

/// Builds one [`Footnote`](InlineNode::Footnote) node from a `footnote:` match,
/// resolving it into the same three (id, content) cases the string
/// replacer's `InlineFootnoteMacroReplacer` distinguishes, so the fold —
/// which reconstructs
/// [`FootnoteRenderParams`](crate::parser::FootnoteRenderParams) from the node
/// alone (see `fold_footnote`) — reproduces the same bytes. Returns `None` for
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
/// [`Parser::define_footnote`] — the same document-counter-advancing calls
/// the string replacer makes — or the differential corpus below could never
/// pass. The two code paths never share a `Parser` (each independently
/// numbers footnotes over the same source in the same left-to-right order),
/// so this never double-counts a registration; see the module's test helpers.
///
/// The registered catalog `text` is the footnote's **own subtree, folded** —
/// see [`register_footnote_number`], which is where the entry a
/// `Document::catalog().footnotes()` reader sees is now built, and why it can
/// only be built here.
///
/// # Content carrying an escaped closing bracket
///
/// A content bracket may carry an escaped closing bracket (`\]`), which the
/// string replacer unescapes to a literal `]`
/// ([`normalize_footnote_text`](crate::content::macros::normalize_footnote_text)).
/// The subtree carries it the same way: [`footnote_children`] emits the
/// content through the reference-bearing families' own
/// [`emit_range_unescaping_brackets`], which drops each backslash as a *gap*
/// in the ranges it emits rather than rebuilding the pieces around it — see
/// that helper for why a per-node `replace` cannot express the same unescape.
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

    // The id (`[\w-]+`) admits neither a special character (an entity's own
    // `&`/`;` fits no character class it has) nor the `SPAN_PLACEHOLDER` an
    // opaque piece contributes (Unicode category `Co`, which `\w` does not
    // match), so its range can only ever overlap `Text` pieces — verbatim or
    // [`synthesized`](Piece::synthesized) — and needs no gate of its own, the
    // same structural argument the bare e-mail family makes for its address.
    // [`footnote_id_text`] therefore recovers it exactly in both cases. `id`
    // is `None` only when the macro carries none (an anonymous
    // `footnote:[…]`).
    let id: Option<CowStr<'src>> = caps
        .get(2)
        .map(|m| footnote_id_text(m.start()..m.end(), s, pieces, nodes));

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
                let children = footnote_children(content.range(), s, pieces, nodes);
                let number = register_footnote_number(parser, Some(id.as_ref()), &children, root);

                Some(InlineNode::Footnote(Footnote {
                    id: Some(id),
                    number: Some(CowStr::from(number)),
                    is_reference: false,
                    children,
                    location,
                }))
            }

            // A reference to an id that was never defined, reported exactly
            // where the string replacer reports it. Recorded rather than
            // replayed: the node is a reference with no number, which is also
            // what a *forward* reference looks like mid-parse, so the tree
            // cannot tell the two apart after the fact.
            None => {
                parser.record_builder_diagnostic(
                    root,
                    crate::warnings::WarningType::InvalidFootnoteReference(id.to_string()),
                );

                Some(InlineNode::Footnote(Footnote {
                    id: Some(id),
                    number: None,
                    is_reference: true,
                    children: vec![],
                    location,
                }))
            }
        };
    }

    // An anonymous defining occurrence.
    let content = content_match?;

    let children = footnote_children(content.range(), s, pieces, nodes);
    let number = register_footnote_number(parser, None, &children, root);

    Some(InlineNode::Footnote(Footnote {
        id: None,
        number: Some(CowStr::from(number)),
        is_reference: false,
        children,
        location,
    }))
}

/// Builds one [`Footnote`](InlineNode::Footnote) node from a `footnoteref:`
/// match — the deprecated form's own counterpart to [`build_footnote_node`].
/// Unlike `footnote:id[…]`, which takes its id from the macro target,
/// `footnoteref:[id,text]` / `footnoteref:[id]` packs both into one bracket
/// (`raw`, group 3 of [`INLINE_FOOTNOTE_MACRO`]), split on the **first**
/// comma — mirroring `InlineFootnoteMacroReplacer`'s own `raw.split_once(',')`
/// exactly, including that an id is *always* present (a bracket with no
/// comma is the id alone, with no text — a bare reference) and that a
/// trailing comma (`footnoteref:[id,]`) yields an *empty*, not absent,
/// content (a defining occurrence with empty text), unlike `footnote:id[]`'s
/// own no-comma-at-all "reference" shape. Once split, the (id, content) pair
/// resolves through the exact same three cases
/// [`build_footnote_node`] does (reuse an already-defined id's number, define
/// a new id-carrying occurrence, or fall back to an unresolved reference) —
/// see that function's own doc comment for the shared reasoning (the
/// required `footnote_index_for_id`/`define_footnote` side effect, and how a
/// content-side `\]` is unescaped, which applies here identically since `raw`
/// is matched by the very same bracket group). The **id** half is the one
/// place the two differ: the string replacer splits the *raw* bracket on its
/// first comma and never normalizes what precedes it, so an id carrying a `\]`
/// keeps its backslash — as the id [`footnote_id_text`] recovers does.
///
/// The deprecation warning `InlineFootnoteMacroReplacer` records outside
/// `compat-mode` is raised here too, from the whole match's own text, exactly
/// as that replacer raises it.
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

    // The `footnoteref:` macro is deprecated outside compatibility mode.
    // Recorded rather than replayed because the node this builds is an
    // ordinary `Footnote`, indistinguishable from the `footnote:` spelling's —
    // the deprecation is a fact about the *source*, which only the recognition
    // site sees.
    if !parser.is_attribute_set("compat-mode")
        && let Some(matched) = s.get(full.clone())
    {
        parser.record_builder_diagnostic(
            root,
            crate::warnings::WarningType::DeprecatedFootnoterefMacro(matched.to_string()),
        );
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

    // Unlike the `footnote:` form's own `[\w-]+` id — which no opaque piece
    // can reach — this id is *whatever precedes the first comma* in an
    // arbitrary bracket, so it can cross a rendered span, whose markup the
    // string replacer splits on and reads as the id while this side sees one
    // placeholder standing in for it. Such a macro is left unrecognized (the
    // boundary every macro family keeps), rather than built with an id no
    // pipeline would produce.
    if !range_has_no_opaque_piece(nodes, pieces, &id_range) {
        return None;
    }

    let id = footnote_id_text(id_range, s, pieces, nodes);

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
            let children = footnote_children(content_range, s, pieces, nodes);
            let number = register_footnote_number(parser, Some(id.as_ref()), &children, root);

            Some(InlineNode::Footnote(Footnote {
                id: Some(id),
                number: Some(CowStr::from(number)),
                is_reference: false,
                children,
                location,
            }))
        }

        // A reference to an id that was never defined — see the sibling site
        // in `build_footnote_node` for why this is recorded rather than
        // replayed.
        None => {
            parser.record_builder_diagnostic(
                root,
                crate::warnings::WarningType::InvalidFootnoteReference(id.to_string()),
            );

            Some(InlineNode::Footnote(Footnote {
                id: Some(id),
                number: None,
                is_reference: true,
                children: vec![],
                location,
            }))
        }
    }
}

/// Recovers a footnote **id** from the level's match-string `range` — the
/// *already-substituted* text the string replacer itself reads out of its own
/// escaped haystack (`caps[2]`, or the first half of a `footnoteref:`
/// bracket), and the exact string it looks the footnote up by, registers under,
/// and — for an unresolved reference — renders.
///
/// [`text_slice`] recovers that text precisely wherever the range's pieces are
/// [`Text`](InlineNode::Text) runs: borrowed from `'src` for a verbatim one
/// (design §4.5), and the expansion's own bytes for a
/// [`synthesized`](Piece::synthesized) one — an attribute reference
/// (`{fn-disclaimer}` expanding to `footnote:disclaimer[…]`), or a filtered
/// multi-line block's own joined seed. That is the lift this helper exists for:
/// reading the id from the enclosing span instead (as this family did before)
/// yields the *reference* (`{fn-disclaimer}`) rather than the id, a wrong node
/// whose registration and rendered `id="_footnote_…"` attribute both diverge.
///
/// A [`CharRef`](InlineNode::CharRef) leaf in the range — an escaped special or
/// a restored entity, reachable only through a `footnoteref:` id — has no
/// `'src` slice at all (the source holds one character where the match string
/// holds an entity), which is exactly what `text_slice` declines to recover, so
/// it falls back to the match string's own bytes: `footnoteref:[a&b,…]`
/// registers `a&amp;b`, precisely what the string replacer's
/// `raw.split_once(',')` reads from its own haystack. Only an **opaque** piece
/// cannot be recovered this way, and its one reachable call site rejects such a
/// range before calling here.
fn footnote_id_text<'src>(
    range: std::ops::Range<usize>,
    s: &str,
    pieces: &[Piece],
    nodes: &[InlineNode<'src>],
) -> CowStr<'src> {
    text_slice(nodes, pieces, range.clone()).unwrap_or_else(|| CowStr::from(s[range].to_string()))
}

/// Builds a footnote's `children` from its bracket content's match-string
/// `range` via [`emit_range_unescaping_brackets`] — *not*
/// [`range_is_verbatim`](super::macros::image::range_is_verbatim) the way every
/// other macro family's target/text slicing does. A footnote's content
/// becomes structured children rather than a literal attribute value, so a
/// range crossing an already-recognized construct is not a boundary to defer
/// on; it is the whole point — the emitter clones that construct's node
/// whole into the footnote's subtree, exactly mirroring how the string
/// pipeline's footnote text captures an already-substituted macro verbatim
/// (see [`apply_footnotes`]'s doc comment on ordering). It emits through the
/// plain [`emit_range`](super::quotes::emit_range), not the recursing
/// [`emit_range_recursing_footnotes`] — footnotes do not nest (the string
/// pipeline's own lazy bracket match cannot recognize one inside another's
/// content either, since it always stops at the *first* unescaped `]`), so a
/// footnote's own content is captured as-is.
///
/// # The `\]` unescape
///
/// A bracket's content may carry an **escaped closing bracket** (`\]`), which
/// [`normalize_footnote_text`](crate::content::macros::normalize_footnote_text)
/// — the string replacer's own normalization — turns back into a literal `]`.
/// The subtree must carry the same text, so the shared
/// [`emit_range_unescaping_brackets`] drops
/// each backslash as a *gap* in the ranges it emits: `footnote:[a \] b]`
/// becomes the two `'src`-borrowing [`Text`](InlineNode::Text) children `a `
/// and `] b`, never an owned rebuild. Sharing the reference-bearing families'
/// own helper is what keeps the two readings of "which backslashes pair off"
/// from drifting; it is also why this form is no longer deferred (recognizing
/// it once meant splicing a literal `]` into the middle of a `Text` piece,
/// which nothing here could then express).
///
/// Unescaping *here* is also why [`register_footnote_number`] applies only the
/// other two halves of that normalization (the trim and the newline collapse)
/// to the template it folds out of these children: the backslash is already
/// gone by then, and a second pass would unescape a string that was never
/// escaped.
fn footnote_children<'src>(
    range: std::ops::Range<usize>,
    s: &str,
    pieces: &[Piece],
    nodes: &[InlineNode<'src>],
) -> Vec<InlineNode<'src>> {
    let mut children = Vec::new();

    emit_range_unescaping_brackets(&s[range.clone()], range, nodes, pieces, &mut children);

    children
}

/// Registers a footnote's defining occurrence with the parser, advancing the
/// `footnote-number` counter and returning the assigned number — the one
/// recognition side effect [`build_footnote_node`]'s doc comment explains this
/// pass must perform.
///
/// The catalog entry is built from the footnote's own `children`, folded
/// through [`fold_deferring_xrefs`]: that fold yields, in one pass, the
/// placeholder **template** the entry stores and the cross-reference
/// **segments** that fill it, which is exactly the pair `define_footnote`
/// turns into a
/// [`FootnoteDeferred`](crate::content::FootnoteDeferred). The string
/// replacer reaches the same pair from the other direction — it re-homes the
/// block template's placeholders out of the already-substituted text it cut
/// the footnote from
/// ([`rehome_xref_placeholders`](crate::content::rehome_xref_placeholders)) —
/// so a footnote's own `<<tgt>>` resolves on either side.
///
/// Folding the subtree is also what makes the entry's `text` byte-faithful at
/// all: the raw bracket content this used to register is the *match string*,
/// in which an already-recognized construct is one opaque `SPAN_PLACEHOLDER`
/// codepoint, so `footnote:[see https://github.com[GitHub\]]` registered
/// `"see \u{e0f0}"` where the string pipeline registers
/// `"see <a href=\"https://github.com\">GitHub</a>"`.
///
/// # Why the anchor is `root`, not the macro's own span
///
/// `define_footnote` records where the *enclosing content* was written, not
/// where the macro sits: the entry's location anchors an unresolved-reference
/// warning raised much later, when the catalog resolves the footnote's own
/// cross-references, and the string replacer passes its whole `self.source`
/// there (paragraph granularity, matching how a non-footnote reference is
/// anchored at its `Content` — see the field's own docs). `root` is that same
/// span; it is what this pass already passes to
/// [`Parser::record_builder_diagnostic`] for the two diagnostics it raises.
/// The macro's own span is the *node's* `location`, a different question with
/// a different answer.
///
/// # Normalization
///
/// [`normalize_footnote_text`](crate::content::macros::normalize_footnote_text)
/// does three things to the string replacer's raw content: trims it, collapses
/// each embedded newline to a space, and unescapes `\]` to `]`. The first two
/// apply here unchanged — they are about the *text*, whichever pipeline
/// produced it. The third does **not**: [`footnote_children`] already dropped
/// each such backslash while emitting the subtree (see its doc comment), so
/// re-applying the unescape here would be a second pass over an
/// already-unescaped string.
fn register_footnote_number(
    parser: &Parser,
    id: Option<&str>,
    children: &[InlineNode<'_>],
    root: Span<'_>,
) -> String {
    let (template, xrefs) =
        fold_deferring_xrefs(children, &*parser.renderer, &parser.render_context());

    // Trim and collapse, but do not unescape — see the doc comment above.
    let template = template.trim().replace('\n', " ");

    parser.define_footnote(id, template, xrefs, root)
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
    fn emit_range_recursing_footnotes_skips_a_synthesized_piece_whose_declared_length_overruns_its_value()
     {
        // The same defensive posture as
        // `quotes::tests::emit_range_skips_a_synthesized_piece_whose_declared_length_overruns_its_value`,
        // exercised directly against this module's own copy of the
        // synthesized-slicing branch (see `emit_range_recursing_footnotes`'s
        // own doc comment for why it duplicates `emit_range` rather than
        // calling it).
        use super::{Piece, emit_range_recursing_footnotes};
        use crate::Parser;

        let location = Span::new("{x}");

        let node = InlineNode::Text {
            value: CowStr::from("ab"),
            location,
        };

        let piece = Piece {
            node_index: 0,
            s_start: 0,
            s_len: 5, // overruns "ab"'s 2 bytes
            src_offset: location.byte_offset(),
            src_len: location.data().len(),
            atomic: false,
            synthesized: true,
        };

        let mut out = Vec::new();
        emit_range_recursing_footnotes(
            std::slice::from_ref(&node),
            std::slice::from_ref(&piece),
            0..5,
            location,
            &Parser::default(),
            &mut out,
        );
        assert!(out.is_empty(), "{out:?}");
    }

    #[test]
    fn an_attribute_reference_beside_a_footnote_keeps_its_resolved_value() {
        // Regression test: `emit_range_recursing_footnotes` (used for every
        // *gap* around a recognized footnote, not the footnote's own content —
        // see `footnote_content_children`) has its own copy of `emit_range`'s
        // verbatim-slicing logic, and until it also special-cased a
        // `synthesized` piece (see `quotes::Piece::synthesized`), it applied a
        // match-string-coordinate range straight to the node's *source*
        // `location` — valid for a verbatim run (whose `s_len` and source
        // length agree) but not for a synthesized one (an attribute
        // expansion), where they can differ. Concretely, `{product}`
        // (9 source bytes) expanding to `"Widget"` (6 bytes) corrupted the gap
        // *before* a sibling footnote into a truncated slice of the raw
        // source (`"{produ"`) instead of the resolved value.
        use crate::{
            Parser,
            content::{Content, SubstitutionGroup, inline_builder::build},
            parser::ModificationContext,
        };

        // Two *independent* parsers (design §5.3's discipline, established by
        // this module's own differential corpus below): `build` and the real
        // pipeline each advance the footnote registry for real, so sharing
        // one parser would double-count the footnote's assigned number.
        let configure = || {
            Parser::default().with_intrinsic_attribute(
                "product",
                "Widget",
                ModificationContext::Anywhere,
            )
        };

        let source = "The {product} is great, footnote:[x] right?";
        let nodes = build(Span::new(source), &configure(), None);
        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});

        let mut golden = Content::from(Span::new(source));
        SubstitutionGroup::Normal.apply_string_pipeline(&mut golden, &configure(), None);

        assert_eq!(
            folded,
            crate::content::inline_builder::snapshot::recorded_golden(
                "footnotes_expanded_attribute",
                source,
                golden.rendered_str(),
            ),
            "{nodes:#?}"
        );
        assert!(folded.contains("Widget"), "{folded:?}");
    }

    #[test]
    fn fold_matches_the_string_pipeline_through_footnotes() {
        // For each fixture, folding the single-pass tree (all five steps)
        // reproduces the string pipeline's output byte-for-byte. This is the
        // differential corpus (design §5.3) that pins the footnote increment —
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
            // all — left untouched by both the string replacer and the builder.
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
            // pass at this level — a formatting span, a character replacement,
            // an image, a link, an index term, and (since footnotes run last)
            // a cross-reference — is captured as that construct's node, not
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
            // the same level as a genuine footnote match — not nested inside
            // the footnote's own content — exercising
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
            // An escaped closing bracket (`\\]`) in the content: the bracket
            // group runs past it and both sides unescape it the same way — in
            // every position (interior, leading, trailing), doubled, more than
            // once in one content, in both spellings, and beside a construct
            // the content captures as a node rather than as text.
            "footnote:[a note ending in a\\]bracket]",
            "footnote:disc[a note ending in a\\]bracket]",
            "footnote:[\\]]",
            "footnote:[\\]leading]",
            "footnote:[trailing\\]]",
            "footnote:[a \\]\\] b]",
            "footnote:[a \\] b \\] c]",
            "footnoteref:[disc,a note ending in a\\]bracket]",
            // No comma at all: the whole (still-escaped) bracket is the id of
            // an unresolved reference, which the string replacer never
            // normalizes.
            "footnoteref:[a note ending in a\\]bracket]",
            "footnote:[the *strong* \\] evidence]",
            "footnote:[an escaped \\] beside a < special]",
            // A defining occurrence carrying one, then a bare reference to it:
            // the number the first assigns is the one the second reuses.
            "Named.footnote:disc[a \\] note] then footnote:disc[].",
            // Two anonymous ones, the first carrying an escaped bracket:
            // recognizing it is what keeps the second numbered `2`.
            "one footnote:[a \\] note] two footnote:[b]",
            // The escape of the *macro itself* still wins over the bracket's
            // own escape.
            "\\footnote:[a \\] note]",
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
    fn a_footnote_nested_in_a_child_numbers_in_source_order() {
        // The property this pass exists for, asserted where it is actually
        // decided: a footnote nested in a child that falls **between** two of
        // its level's own footnotes is numbered between them, because the
        // string pipeline recognizes in one left-to-right sweep over one flat
        // string.
        //
        // Every fixture here places a nested footnote in that position — after
        // one sibling and before another — since that is the only arrangement
        // the two orders disagree about: a nested footnote before every
        // sibling, or after every sibling, numbers the same either way.
        let renderer = HtmlSubstitutionRenderer {};

        for fixture in [
            // Each container the walk descends into, with a plain sibling on
            // either side.
            "before footnote:[a] *span footnote:[b]* after footnote:[c]",
            "before footnote:[a] _em footnote:[b]_ after footnote:[c]",
            "before footnote:[a] #mark footnote:[b]# after footnote:[c]",
            "before footnote:[a] ((a term footnote:[b])) after footnote:[c]",
            // Nested two deep, so the recursion's own order is exercised.
            "before footnote:[a] *outer _inner footnote:[b]_* after footnote:[c]",
            "before footnote:[a] ((a *term footnote:[b]*)) after footnote:[c]",
            // Two children, each carrying one, between two siblings.
            "footnote:[a] *x footnote:[b]* mid *y footnote:[c]* footnote:[d]",
            // The child's own footnote beside a sibling *inside* the same
            // child, so the level-vs-child interleaving happens twice.
            "footnote:[a] *x footnote:[b] y footnote:[c]* footnote:[d]",
            // A named footnote reused from inside a child, whose number is
            // taken from the definition rather than assigned.
            "footnote:d[a note] *span footnote:d[]* after footnote:[c]",
            // A child carrying an *unrecognized* footnote form beside a real
            // one, so the cursor handling for a candidate that does not build
            // is exercised in a nested walk too.
            "before footnote:[a] *span footnote:[] and footnote:[b]* after footnote:[c]",
        ] {
            assert_eq!(
                fold_html(&build_src(Span::new(fixture)), &renderer),
                golden_macros(fixture),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn an_anchors_reference_text_is_not_scanned_for_footnotes() {
        // The complement, and the reason the walk descends into an index term
        // but not into an anchor's reference text: the anchor replacer
        // *consumes* that text rather than emitting it, so a `footnote:[…]`
        // written there never reaches the string pipeline's footnote pass
        // either. Both sides agree that nothing is numbered — and a real
        // footnote beside it still takes number 1.
        let renderer = HtmlSubstitutionRenderer {};

        for fixture in [
            "[[a,see footnote:[note]]] end",
            "[[a,see footnote:[note]]] and footnote:[real] end",
            "anchor:a[see footnote:[note]] and footnote:[real] end",
        ] {
            let folded = fold_html(&build_src(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden_macros(fixture),
                "fold diverged from the string pipeline for {fixture:?}"
            );

            assert!(
                !folded.contains("_footnoteref_2"),
                "an anchor's reference text must not be numbered: {folded:?}"
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
        // content is captured, the link is already a `Ref` node — captured
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
        // footnote at all — left as literal text, exactly as the string
        // replacer's `next $&` branch leaves it.
        let nodes = build_src(Span::new("footnote:[]"));

        assert_eq!(nodes.len(), 1);
        assert_text(&nodes[0], "footnote:[]", 1, 1);
    }

    #[test]
    fn a_deprecated_footnoteref_macro_with_an_id_and_text_is_recognized() {
        // The deprecated `footnoteref:[id,text]` form packs its id and text
        // into one bracket, split on the first comma — a different split
        // from `footnote:id[text]`'s own (id from the macro target, text
        // from the bracket) — but resolves into the same node shape and
        // folds through the same `render_footnote`, so its output is
        // byte-for-byte identical to the golden pipeline's (the deprecation
        // warning itself remains deferred to the cutover; it does not affect
        // the fold's output bytes — see `build_footnoteref_node`'s doc
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
        // already-defined footnote — the same shape `footnote:id[]` produces,
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
        // (present, not absent) — a defining occurrence with empty text,
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
        // unrecognized — mirroring the string replacer's `next $&` branch,
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
    fn a_footnoteref_macro_with_an_escaped_bracket_unescapes_its_content() {
        // The same unescape
        // `a_footnote_with_an_escaped_bracket_unescapes_it_into_the_subtree`
        // pins for `footnote:`, through `build_footnoteref_node`'s own
        // (identical) bracket group.
        let source = "footnoteref:[disc,a note ending in a\\]bracket]";
        let folded = fold_html(&build_src(Span::new(source)), &HtmlSubstitutionRenderer {});

        assert_eq!(folded, golden_macros(source));

        let nodes = build_src(Span::new(source));
        let footnote = assert_footnote(&nodes[0]);
        assert_eq!(footnote.id.as_deref(), Some("disc"));
        assert_eq!(footnote.children.len(), 2);
        assert_text(&footnote.children[0], "a note ending in a", 1, 19);
        assert_text(&footnote.children[1], "]bracket", 1, 38);
    }

    #[test]
    fn a_footnoteref_id_keeps_its_own_escaped_bracket() {
        // The id half is the one place the two forms differ: the string
        // replacer splits the *raw* bracket on its first comma and never
        // normalizes what precedes it, so an id carrying a `\]` keeps its
        // backslash — and, with no comma at all, the whole bracket is that id
        // (an unresolved reference, rendered as written).
        let source = "footnoteref:[a note ending in a\\]bracket]";
        let folded = fold_html(&build_src(Span::new(source)), &HtmlSubstitutionRenderer {});

        assert_eq!(folded, golden_macros(source));

        let nodes = build_src(Span::new(source));
        let footnote = assert_footnote(&nodes[0]);
        assert_eq!(footnote.id.as_deref(), Some("a note ending in a\\]bracket"));
        assert!(footnote.is_reference);
        assert!(footnote.number.is_none());
    }

    #[test]
    fn a_footnote_with_an_escaped_bracket_unescapes_it_into_the_subtree() {
        // Content carrying an escaped closing bracket (`\]`) is recognized:
        // the string replacer unescapes it to a literal `]`
        // (`normalize_footnote_text`) and the subtree carries the same text,
        // `footnote_children` dropping the backslash as a *gap* in the ranges
        // it emits (see `emit_range_unescaping_brackets`). Both the anonymous
        // form and the id-carrying form share that path, so both are
        // exercised here.
        for (source, text_col) in [
            ("footnote:[a note ending in a\\]bracket]", 11),
            ("footnote:disc[a note ending in a\\]bracket]", 15),
        ] {
            let folded = fold_html(&build_src(Span::new(source)), &HtmlSubstitutionRenderer {});

            assert_eq!(
                folded,
                golden_macros(source),
                "fold diverged from the string pipeline for {source:?}"
            );

            let nodes = build_src(Span::new(source));
            let footnote = assert_footnote(&nodes[0]);
            assert!(!footnote.is_reference);
            assert_eq!(footnote.number.as_deref(), Some("1"));

            // The backslash is a gap between two `'src`-borrowing runs, not an
            // owned rebuild of the whole text.
            assert_eq!(footnote.children.len(), 2);
            assert_text(&footnote.children[0], "a note ending in a", 1, text_col);
            assert_text(&footnote.children[1], "]bracket", 1, text_col + 19);
        }
    }

    #[test]
    fn an_escaped_bracket_reaches_the_footnote_catalog_unescaped() {
        // The catalog `text` this pass registers goes through the string
        // replacer's own `normalize_footnote_text`, so it carries the literal
        // `]` the subtree now does — the two readings of the same content
        // agree.
        let parser = crate::Parser::default();
        let _nodes = super::super::build(
            Span::new("footnote:[a note ending in a\\]bracket]"),
            &parser,
            None,
        );

        let catalog = parser.catalog();
        let footnotes = catalog.footnotes();

        assert_eq!(footnotes.len(), 1);
        assert_eq!(footnotes[0].text, "a note ending in a]bracket");
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
        // The opposite nesting from the test above: a *link* — a macro-built
        // `Ref` node, created *during* `apply_macros` rather than already
        // existing by the time it runs like a quotes-built `Styled` span —
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
        // `link:` matches its bracketed label *lazily* — up to the first
        // unescaped `]` — so an unbalanced inner bracket like
        // `footnote:[note]` (itself containing a `]`) truncates the link's
        // own match early, leaving `footnote:[note` as the link's label and a
        // stray `]` as literal text after it (mirrored exactly by the
        // builder's `link_macro_level`, which shares the same regex — see the
        // `fold_matches_the_string_pipeline_through_link_macros` corpus for
        // that shared behavior on its own). This is not specific to links: a
        // footnote nested inside *any* bracket-delimited macro argument that
        // runs *before* footnotes (image, index terms, anchors,
        // cross-references — every family footnotes.rs runs after) hits the
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
        // literal text no different from any other — so the footnote regex,
        // finding only one `]` left in the whole string, matches straight
        // through the `</a>` and consumes it as part of its own (nonsensical)
        // content, producing malformed, never-closed markup. That is a direct
        // consequence of matching over *rendered* text rather than structure
        // — exactly the class of divergence
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

    /// A parser carrying the attributes the expanded-value fixtures below
    /// reference — including two whose *whole value* is a footnote macro, the
    /// externalized-footnote idiom the AsciiDoc docs themselves document
    /// (`tests::asciidoc_lang::macros::footnote::externalized_footnote`).
    fn expanding_parser() -> crate::Parser {
        use crate::{Parser, parser::ModificationContext};

        Parser::default()
            .with_intrinsic_attribute("id", "disc", ModificationContext::Anywhere)
            .with_intrinsic_attribute("product", "Widget", ModificationContext::Anywhere)
            .with_intrinsic_attribute(
                "fn-disclaimer",
                "footnote:disclaimer[Opinions are my own.]",
                ModificationContext::Anywhere,
            )
            .with_intrinsic_attribute(
                "fn-anon",
                "footnote:[An unnamed aside.]",
                ModificationContext::Anywhere,
            )
    }

    /// The real, public pipeline's output for `source` — the golden for the
    /// expanded-value fixtures, which need the `AttributeReferences` step the
    /// module's own [`golden_macros`] helper deliberately omits.
    fn golden_normal(source: &str, parser: &crate::Parser) -> String {
        use crate::content::{Content, SubstitutionGroup};

        let mut content = Content::from(Span::new(source));
        SubstitutionGroup::Normal.apply_string_pipeline(&mut content, parser, None);

        crate::content::inline_builder::snapshot::recorded_golden(
            "footnotes_normal",
            source,
            content.rendered_str(),
        )
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_footnotes_inside_expanded_values() {
        // A footnote whose id — or whose whole macro — comes from an expanded
        // attribute value is now recognized with that id's *exact* text. This
        // is the same lift the anchor, bare-e-mail, UI, index-term,
        // cross-reference, image, and link families each made for their own
        // values; unlike those, this family did not *defer* such a macro
        // before, it built one whose id came from the enclosing `{reference}`
        // instead — a wrong node, and (since the id is what a footnote is
        // registered and looked up under) one that renumbered every later
        // reference to it.
        //
        // Each fixture uses its own pair of *independent* parsers (design
        // §5.3's discipline), since both `build` and the real pipeline
        // advance the `footnote-number` counter for real.
        use crate::content::inline_builder::build;

        let fixtures = [
            // The whole macro arriving from an expanded value: the
            // externalized-footnote idiom, defining and then referencing.
            "{fn-disclaimer}",
            "A bold statement!{fn-disclaimer}",
            "First.{fn-disclaimer} Then again.{fn-disclaimer}",
            "{fn-anon} and {fn-anon}",
            "{fn-disclaimer} then footnote:disclaimer[]",
            // The id alone arriving from an expanded value, whole or partial.
            "footnote:{id}[a note]",
            "footnote:{id}x[a note]",
            "footnote:{id}[a note] then footnote:disc[]",
            "footnote:{id}[] with nothing defined",
            // The deprecated form's own id half, likewise.
            "footnoteref:[{id},a note]",
            "footnoteref:[{id},a note] then footnoteref:[{id}]",
            // Content from an expanded value (already parity before this
            // increment — its children never needed an `'src` slice).
            "footnote:[a {product} note]",
            "footnote:{id}[a {product} note]",
        ];

        for source in fixtures {
            let nodes = build(Span::new(source), &expanding_parser(), None);

            assert_eq!(
                crate::content::inline_builder::fold_html(
                    &nodes,
                    &HtmlSubstitutionRenderer {},
                    &expanding_parser().render_context()
                ),
                golden_normal(source, &expanding_parser()),
                "fold diverged from the string pipeline for {source:?}"
            );
        }
    }

    #[test]
    fn a_footnote_inside_an_expanded_value_keeps_a_coarse_location() {
        // The id is exact — recovered from the expansion's own bytes by
        // `footnote_id_text`, and necessarily owned — while only the node's
        // `location` falls back to the enclosing synthesized run's coarse
        // span (design §4.4), since an expanded value's bytes have no `'src`
        // counterpart of their own.
        use crate::content::inline_builder::build;

        let source = "A bold statement!{fn-disclaimer}";
        let nodes = build(Span::new(source), &expanding_parser(), None);

        let footnote = assert_footnote(&nodes[1]);

        // The id is the *expansion's* own text, which — having no `'src`
        // bytes of its own — the node necessarily owns; the `location`
        // assertion below is the other half of that same split.
        assert_eq!(footnote.id.as_deref(), Some("disclaimer"));
        assert_eq!(footnote.number.as_deref(), Some("1"));
        assert!(!footnote.is_reference);

        // The whole attribute *reference* is the node's location: its bytes
        // are the source's `{fn-disclaimer}`, not the expanded macro's.
        assert_eq!(footnote.location.data(), "{fn-disclaimer}");
        assert_eq!(footnote.location.line(), 1);
        assert_eq!(footnote.location.col(), 18);
    }

    #[test]
    fn footnotes_are_recognized_when_the_whole_seed_is_synthesized() {
        // The same lift reached at the tree's *root* rather than a nested
        // splice: `build_from_value`'s synthesized-seed path (the shape
        // `Content::from_filtered_lines` produces for a genuinely multi-line,
        // filtered block), mirroring the sibling families' own
        // `…_are_recognized_when_the_whole_seed_is_synthesized` tests. Before
        // this increment an id-carrying footnote reached this way took the
        // *whole seed* as its id.
        use crate::{
            Parser,
            content::{Content, SubstitutionGroup, inline_builder::build_from_value},
            strings::CowStr,
        };

        for (filtered, source) in [
            (
                "a claim.footnote:disc[a note]
and a reference.footnote:disc[]",
                "  a claim.footnote:disc[a note]
  and a reference.footnote:disc[]",
            ),
            (
                "an anonymous.footnote:[note one]
and another.footnote:[note two]",
                "  an anonymous.footnote:[note one]
  and another.footnote:[note two]",
            ),
        ] {
            let nodes = build_from_value(
                CowStr::from(filtered),
                Span::new(source),
                &Parser::default(),
                None,
            );

            let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});

            let mut golden = Content::from(Span::new(filtered));
            SubstitutionGroup::Normal.apply_string_pipeline(&mut golden, &Parser::default(), None);

            assert_eq!(
                folded,
                crate::content::inline_builder::snapshot::recorded_golden(
                    "footnotes_build_from_value",
                    filtered,
                    golden.rendered_str(),
                ),
                "for {filtered:?}: {nodes:#?}"
            );
        }
    }

    #[test]
    fn a_footnoteref_id_crossing_an_escaped_special_reads_the_match_string() {
        // The deprecated form's id half is *whatever precedes the first
        // comma*, so — unlike the `footnote:` form's own `[\w-]+` id — it can
        // cross an escaped special. `text_slice` declines such a range (the
        // source holds one character where the match string holds an entity),
        // so `footnote_id_text` falls back to the match string's own bytes:
        // exactly what the string replacer's `raw.split_once(',')` reads out
        // of its own escaped haystack, and registers.
        let source = "footnoteref:[a&b,a note] then footnoteref:[a&b]";
        let nodes = build_src(Span::new(source));

        let defining = assert_footnote(&nodes[0]);
        assert_eq!(defining.id.as_deref(), Some("a&amp;b"));
        assert_eq!(defining.number.as_deref(), Some("1"));

        let reference = assert_footnote(&nodes[2]);
        assert_eq!(reference.id.as_deref(), Some("a&amp;b"));
        assert_eq!(reference.number.as_deref(), Some("1"));
        assert!(reference.is_reference);

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn a_footnoteref_id_crossing_a_rendered_span_is_a_documented_divergence() {
        // The one piece class `footnote_id_text` cannot recover: a rendered
        // span, whose markup exists only at fold time, is one opaque
        // placeholder here where the string replacer's own haystack holds the
        // `<strong>…</strong>` tags it happily splits on and registers as the
        // id. Such a macro is left unrecognized — literal text, never a wrong
        // node — the boundary every macro family keeps. (If a later increment
        // lifts it, fold this fixture into the parity corpus above.)
        let source = "footnoteref:[*bold*,a note]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Footnote(_))),
            "a footnoteref: id crossing a rendered span must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, does build a footnote here.
        assert!(golden_macros(source).contains("class=\"footnote\""));
    }

    #[test]
    fn a_real_documents_externalized_footnote_reaches_its_tree() {
        // End-to-end, through the real parse path, on the AsciiDoc docs' own
        // externalized-footnote shape (the fixture
        // `tests::asciidoc_lang::macros::footnote::externalized_footnote`
        // parses): the second reference to `{fn-disclaimer}` reuses the first
        // occurrence's number, which only works once the id is read from the
        // expansion rather than from the `{fn-disclaimer}` reference itself.
        use crate::{
            Parser,
            blocks::{FindBlocks, IsBlock},
        };

        let doc = Parser::default().parse(
            ":fn-disclaimer: footnote:disclaimer[Opinions are my own.]\n\n\
             A bold statement!{fn-disclaimer}\n\n\
             Another outrageous statement.{fn-disclaimer}",
        );

        let blocks: Vec<_> = doc.descendant_blocks().collect();

        for block in &blocks {
            let rendered = block.rendered_html_content().unwrap();
            let inlines = block.inlines().unwrap();

            assert!(
                inlines.iter().any(|n| matches!(n, InlineNode::Footnote(_))),
                "expected a Footnote node in the block's tree: {inlines:?}"
            );

            assert_eq!(
                crate::content::inline_builder::fold_html(
                    inlines,
                    &HtmlSubstitutionRenderer {},
                    &Parser::default().render_context()
                ),
                rendered,
                "fold diverged from the rendered string for {inlines:?}"
            );
        }

        // The second occurrence is a *reference* to the first: one footnote
        // definition, one `sup.footnoteref` marker.
        assert_eq!(doc.catalog().footnotes().len(), 1);
    }
}
