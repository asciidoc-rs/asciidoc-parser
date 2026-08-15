//! Inline anchor recognition (`[[id]]`, `[[id,reftext]]`, `anchor:id[…]`), and
//! the bibliography anchor (`[[[label]]]`) that prefixes a bibliography list
//! item.

use super::{
    MacroMatch, MacroMatchKind, image::range_is_verbatim_or_synthesized, rebuild_macro_level,
};
use crate::{
    Parser, Span,
    content::{
        INLINE_ANCHOR, INLINE_BIBLIO_ANCHOR,
        inline_builder::quotes::{
            Piece, SPAN_PLACEHOLDER, build_match_string, emit_range, range_overlaps_synthesized,
            source_slice, text_slice,
        },
    },
    document::RefType,
    inlines::{Anchor, InlineNode},
    strings::CowStr,
    warnings::WarningType,
};

/// Matches `INLINE_BIBLIO_ANCHOR` at the **content's own top level**, replacing
/// a bibliography anchor (`[[[label]]]` / `[[[label,xreftext]]]`) with the
/// [`Anchor`](InlineNode::Anchor) node it produces – `is_bibliography` set –
/// followed by the bracketed label the string replacer emits into the flow.
///
/// # Where this runs, and why only here
///
/// The string pipeline runs this pass **first**, ahead of every other macro
/// family, and only when the parser flags that it is substituting the principal
/// text of a bibliography list item
/// ([`in_bibliography_list_item`](Parser::in_bibliography_list_item), set in
/// `blocks::list_item`); this mirrors both. The pattern is `^`-anchored – a
/// `[[[…]]]` appearing later in the entry is left to the ordinary inline-anchor
/// pass, which renders it but never catalogs its id (see
/// [`is_bibliography_inner`]) – so this level pass runs once, at the top level
/// [`apply_macros`](super::apply_macros) is called with, and never descends
/// into a span's children: `^` matches only the very start of the *whole*
/// content, exactly as it does for the string pipeline's own haystack.
///
/// # The bracketed label stays in the flow
///
/// The replacer renders the anchor from its id alone (`render_anchor(id,
/// None)`) and then pushes the bracketed label (`[label]`, or `[xreftext]` when
/// one was supplied) into the output as ordinary text – text every *later*
/// string pass then scans. So the label is emitted here as the sibling nodes
/// that follow the anchor node (sliced from the match's own outer brackets and
/// its label range with [`emit_range`], so each keeps its exact `'src`
/// provenance), rather than as the anchor's own children: that is what lets
/// every family after this one see the label exactly as the string pipeline's
/// later passes see it (an auto-link written in an xreftext is linked in both),
/// with no container to descend into.
///
/// The node's own `reftext` instead carries the bracketed label as the
/// **registered** reference text – what a cross-reference to the entry
/// displays, and what [`apply_biblio_side_effects`] hands
/// [`register_ref`](Parser::register_ref), mirroring the replacer's own single
/// `format!("[{label}]")` serving both purposes.
///
/// # The registered label is already-substituted text
///
/// The string replacer captures its label out of the *escaped,
/// already-rendered* haystack and registers **that** (`[[[gof,A & B]]]`
/// catalogs `[A &amp; B]`), so the node's `reftext` holds the label in the same
/// already-substituted form – the contract an
/// [`IndexTerm`](InlineNode::IndexTerm)'s own `terms` already uses – taken
/// straight from this level's match string, which reconstructs exactly that
/// haystack (a [`CharRef`](InlineNode::CharRef) contributes its
/// canonical entity, so an escaped special and a character replacement alike
/// come out byte-identical to the replacer's own capture). Nothing re-escapes
/// it: the fold hands `render_anchor` `None` for a bibliography anchor (see
/// `fold_anchor`), exactly as the replacer does.
///
/// # Deferred: a label crossing an opaque piece
///
/// What the match string cannot reconstruct is an opaque piece – a rendered
/// [`Styled`](crate::inlines::Styled) span (`[[[gof,*G*]]]`), a passthrough or
/// STEM expression (not even restored yet), or a character replacement
/// (`[[[gof,(C) 1995]]]`, `[[[oreilly,O'Reilly]]]`) – which stands in as a
/// single [`SPAN_PLACEHOLDER`] here rather than as the markup or entity the
/// string pipeline's haystack holds there. Such an anchor is left unrecognized,
/// exactly the boundary the index-term family's own visible term documents
/// (and, for the character replacements, the same one every macro family
/// already has at this point: `build_match_string` serves the quotes step too,
/// where the replacements have not run yet, so it can only treat them as
/// opaque). A label reached through a synthesized run (an attribute expansion,
/// or a filtered block's joined seed) *is* recognized – the run contributes its
/// expanded value to the match string, just as it does to the string pipeline's
/// own haystack.
///
/// As in the additive builder generally, this performs *no* recognition side
/// effect; [`apply_biblio_side_effects`] stages the `register_ref` for the
/// cutover.
pub(super) fn biblio_anchor_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    if !parser.in_bibliography_list_item.get() {
        return nodes;
    }

    let (s, pieces) = build_match_string(&nodes);

    // Cheap pre-filter, mirroring the string step's own `text.contains("[[[")`
    // guard: the pattern is `^`-anchored, so only content *starting* with the
    // triple bracket can match at all.
    if !s.starts_with("[[[") {
        return nodes;
    }

    let Some(caps) = INLINE_BIBLIO_ANCHOR.captures(&s) else {
        return nodes;
    };

    // `unwrap` on groups 0 and 1 is safe: a capture always has an overall
    // match, and the label is not optional in the pattern.
    #[allow(clippy::unwrap_used)]
    let full = {
        let whole = caps.get(0).unwrap();
        whole.start()..whole.end()
    };

    #[allow(clippy::unwrap_used)]
    let id_match = caps.get(1).unwrap();

    // The displayed (and registered) label is the xreftext when one was
    // supplied, else the id itself – exactly the string replacer's own
    // `caps.get(2)…unwrap_or(id)`.
    let label_match = caps.get(2).unwrap_or(id_match);

    let id_range = id_match.start()..id_match.end();
    let label_range = label_match.start()..label_match.end();

    // The label is registered (and shown) as already-substituted text, which
    // this level's match string reproduces for every piece except an opaque one
    // – a rendered span, a passthrough, or a STEM expression – which stands in
    // as a single placeholder.
    let label = match s.get(label_range.clone()) {
        Some(label) if !label.contains(SPAN_PLACEHOLDER) => label,
        _ => return nodes,
    };

    let reftext = CowStr::from(format!("[{label}]"));

    // The id, by contrast, rides on the node as logical text, so it is sliced
    // back to `'src` (borrowing where it can) exactly as an ordinary anchor's
    // own id is. Its character class admits neither a special nor a placeholder,
    // so the two readings coincide – and, for the same reason, the `None` arm
    // (the id crossing an atomic piece) is not actually reachable, kept only
    // for symmetry with [`build_anchor_node`]'s own gate.
    let Some(id) = text_slice(&nodes, &pieces, id_range) else {
        return nodes;
    };

    let location = source_slice(&pieces, full.clone(), root);

    let mut out = vec![InlineNode::Anchor(Anchor {
        id,
        reftext: Some(vec![InlineNode::Text {
            value: reftext,
            location,
        }]),
        is_bibliography: true,
        location,
    })];

    // The bracketed label the replacer pushes into the flow. Its brackets are
    // the match's own outer `[` and `]` (the very characters the triple bracket
    // opens and closes with), so each emitted piece keeps an honest `'src`
    // slice instead of a synthesized value.
    emit_range(&nodes, &pieces, full.start..full.start + 1, &mut out);
    emit_range(&nodes, &pieces, label_range, &mut out);
    emit_range(&nodes, &pieces, full.end - 1..full.end, &mut out);

    // Everything after the anchor is untouched.
    emit_range(&nodes, &pieces, full.end..s.len(), &mut out);

    out
}

/// Matches `INLINE_ANCHOR` at this level's escaped text, replacing each
/// recognized inline anchor – the `[[id]]` / `[[id,reftext]]` shorthand and the
/// `anchor:id[reftext]` macro – with the [`Anchor`](InlineNode::Anchor) node it
/// produces and leaving everything else in place.
pub(super) fn anchor_macros_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes);

    // Cheap pre-filter: an anchor needs either the shorthand `[[` opener or the
    // `anchor:` macro prefix. The `[` characters are not special, so a shorthand
    // reaches the macros step with its `[[` intact.
    if !s.contains("[[") && !s.contains("anchor:") {
        return nodes;
    }

    let matches = find_anchor_matches(&nodes, &s, &pieces, root);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every recognized inline anchor at this level – both spellings – as a
/// [`MacroMatch`].
///
/// An anchor's HTML rendering (`<a id="…"></a>`) is a function of its **id
/// alone**, and an id admits no special character (the pattern's id class is
/// letters/digits/`_`/`-`/`:`/`.`), so an id crossing an *escaped special* or a
/// *rendered span* can never occur – unlike the link/xref families, an anchor
/// is never deferred on *that* boundary. An id's characters can, though, come
/// from a [`synthesized`](Piece::synthesized) run (an attribute reference whose
/// expanded value happens to contain `[[id]]`, or – reached at a tree's root –
/// a filtered multi-line block's own joined seed): [`build_anchor_node`] no
/// longer defers on that alone, recovering the id's exact text via
/// [`text_slice`] even though it has no honest `'src` slice of its own (design
/// §3.4.1/§4.1's "a macro inside an expanded value" boundary, reached here
/// through the id rather than a target/attribute list) – the node's
/// `location` still falls back to the coarse enclosing span design §4.4
/// documents, since only the *text* needed the precision. A non-verbatim
/// *reference text* (one carrying a rendered span or an escaped special) is a
/// narrower case that does not reach the flow at all, so it only leaves the
/// node's `reftext` unpopulated rather than deferring the whole anchor (see
/// [`build_anchor_reftext`]).
pub(super) fn find_anchor_matches<'src>(
    nodes: &[InlineNode<'src>],
    s: &str,
    pieces: &[Piece],
    root: Span<'src>,
) -> Vec<MacroMatch<'src>> {
    let mut matches = Vec::new();

    for caps in INLINE_ANCHOR.captures_iter(s) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();

        let full = whole.start()..whole.end();

        // An escape (`\[[…` / `\anchor:…`) is honored by dropping the backslash
        // and keeping the rest literal, mirroring the string replacer's leading
        // `caps.get(1)` check. [`rebuild_macro_level`] emits the kept range with
        // [`emit_range`], which clones an atomic piece (a rendered-span reference
        // text) whole, so the unescape works even across a non-verbatim reference
        // text – just as the id itself is always verbatim, the whole anchor
        // unescapes regardless.
        if caps.get(1).is_some() {
            matches.push(MacroMatch {
                kind: MacroMatchKind::Unescape {
                    backslash: full.start,
                },
                full,
            });

            continue;
        }

        let node = match build_anchor_node(&caps, &full, pieces, root, nodes) {
            Some(node) => node,

            // The id itself crosses an atomic piece (an escaped special or a
            // rendered span – never actually reachable given the id's own
            // character class, kept for symmetry with every other macro
            // family's own gate); left as literal source, exactly as every
            // other macro family defers a match it cannot slice at all.
            None => continue,
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

/// Builds one [`Anchor`](InlineNode::Anchor) node from an inline-anchor match,
/// recovering the id's exact text via [`text_slice`] so the fold reproduces
/// the string replacer's `<a id="…"></a>` exactly. Returns `None` only when
/// the id crosses an [`atomic`](Piece::atomic) piece (see below) – a form this
/// increment still defers.
///
/// Two spellings share this builder: the `[[id,reftext]]` shorthand (groups
/// 2/3) and the `anchor:id[reftext]` macro (groups 4/5). Exactly one id group
/// matches.
///
/// An id's character class (letters/digits/`_`/`-`/`:`/`.`) admits no escaped
/// special or rendered span, so [`range_is_verbatim_or_synthesized`]'s atomic
/// check can in practice never fail for an id – it is kept for symmetry with
/// every other macro family's own gate. Its bytes *can* come from a
/// [`synthesized`](Piece::synthesized) run – an attribute reference whose
/// expanded value happens to contain `[[id]]`, or – reached at a tree's root –
/// a filtered multi-line block's own joined seed (design §3.4.1's "a macro
/// inside an expanded value" boundary) – and [`text_slice`] recovers the exact
/// id text for that case too, unlike [`source_slice`], which would silently
/// fall back to the enclosing synthesized run's *coarse* span (design §4.4)
/// for both the id and the node's `location`. Only `location` keeps that
/// coarse fallback here; the id text itself is always precise.
///
/// The optional reference text is captured as the node's `reftext` – a single
/// [`Text`](InlineNode::Text) child – whenever it does not cross an atomic
/// piece (the common verbatim case borrows `'src`; a synthesized one is
/// recovered via [`text_slice`] into an owned value, `location` falling back
/// to the coarse span exactly as the id's own does). A shorthand's trailing
/// whitespace is trimmed and a macro's escaped `\]` is unescaped, mirroring
/// the string replacer. A reference text that carries a rendered span or an
/// escaped special is left non-verbatim; because it never reaches the flow
/// (the anchor renders from its id alone), the anchor is still recognized but
/// its `reftext` is left `None` rather than sliced wrongly – a narrower
/// boundary than the id's own, and a shape a re-flow consumer can refine later
/// (the field is provisional, per the node's Phase-0 note).
///
/// As in the additive builder generally, this performs *no* recognition side
/// effect – notably it does **not** `register_ref` the id in the catalog (so a
/// cross-reference can resolve against it), nor emit the duplicate-id warning
/// the string replacer raises; the cutover (design §5.2 Phase 4, step 6) wires
/// those in.
fn build_anchor_node<'src>(
    caps: &regex::Captures<'_>,
    full: &std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
    nodes: &[InlineNode<'src>],
) -> Option<InlineNode<'src>> {
    let location = source_slice(pieces, full.clone(), root);

    // Exactly one id group matches: group 2 for the `[[…]]` shorthand (with its
    // reference text in group 3), else group 4 for the `anchor:…[…]` macro (with
    // its reference text in group 5).
    #[allow(clippy::unwrap_used)]
    let (id_match, reftext_match, is_shorthand) = if let Some(id) = caps.get(2) {
        (id, caps.get(3), true)
    } else {
        // Group 4 always matches when group 2 does not; the alternation admits
        // no third form.
        (caps.get(4).unwrap(), caps.get(5), false)
    };

    let id_range = id_match.start()..id_match.end();

    if !range_is_verbatim_or_synthesized(pieces, &id_range) {
        return None;
    }

    let id = text_slice(nodes, pieces, id_range)?;

    let reftext = reftext_match
        .and_then(|m| build_anchor_reftext(m.start()..m.end(), pieces, root, nodes, is_shorthand));

    Some(InlineNode::Anchor(Anchor {
        id,
        reftext,
        is_bibliography: false,
        location,
    }))
}

/// Builds an inline anchor's `reftext` – a single [`Text`](InlineNode::Text)
/// child – from the reference-text capture's match-string `range`, or `None`
/// when the reference text crosses an atomic piece or trims to empty (see
/// [`build_anchor_node`] for why crossing an atomic piece is not an error for
/// the anchor as a whole).
///
/// A `shorthand` reference text has its trailing whitespace stripped (the
/// string replacer's `trim_end`; leading whitespace was already excluded by
/// the pattern's `, \s*`). A macro reference text unescapes an escaped `\]`
/// into an owned value, mirroring the replacer's `replace("\\]", "]")`.
///
/// The verbatim case (the common one) keeps its exact prior shape: the value
/// borrows `'src`, and a shorthand's `location` is sliced down to the trimmed
/// text precisely. A [`synthesized`](Piece::synthesized) range instead
/// recovers its exact text via [`text_slice`] but keeps the whole range's
/// coarse `location` regardless of trimming or unescaping (design §4.4) –
/// sub-slicing a location has no honest meaning for bytes with no `'src`
/// counterpart of their own, the same policy
/// [`emit_range`] already applies to every fragment of an expanded value.
fn build_anchor_reftext<'src>(
    range: std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
    nodes: &[InlineNode<'src>],
    shorthand: bool,
) -> Option<Vec<InlineNode<'src>>> {
    if !range_is_verbatim_or_synthesized(pieces, &range) {
        return None;
    }

    let synthesized = range_overlaps_synthesized(pieces, &range);
    let location = source_slice(pieces, range.clone(), root);
    let text = text_slice(nodes, pieces, range)?;

    let child = if shorthand {
        let trimmed = text.trim_end();

        if trimmed.is_empty() {
            return None;
        }

        if synthesized {
            InlineNode::Text {
                value: CowStr::from(trimmed.to_string()),
                location,
            }
        } else {
            let text_location = location.slice(0..trimmed.len());

            InlineNode::Text {
                value: CowStr::from(text_location.data()),
                location: text_location,
            }
        }
    } else if text.contains("\\]") {
        // An escaped bracket makes the logical text an owned value whose
        // `location` still covers the raw source (or its coarse fallback) it
        // derives from.
        InlineNode::Text {
            value: CowStr::from(text.replace("\\]", "]")),
            location,
        }
    } else {
        InlineNode::Text {
            value: text,
            location,
        }
    };

    Some(vec![child])
}

/// Performs the recognition side effects the string pipeline attaches to an
/// assigned id at two distinct points – `InlineAnchorReplacer` (an inline
/// anchor, `[[id]]` / `anchor:id[…]`) and the attributed-quote handling in
/// [`SubstitutionStep::Quotes`](crate::content::SubstitutionStep::Quotes)
/// (`[#id]#…#`) – by walking an already-built tree and reading each
/// [`Anchor`](InlineNode::Anchor) node's own stored `id`/`reftext` and each
/// [`Styled`](crate::inlines::Styled) span's own optional `id`, instead of a
/// regex capture. Both register the id in the document's reference catalog
/// under [`RefType::Anchor`] so a later cross-reference can resolve against
/// it; only the inline-anchor form also raises a duplicate-id warning (the
/// attributed-span form is silently non-fatal in the string pipeline too –
/// see [`attributes_of`](super::super::quotes::attributes_of)'s own note).
///
/// Every macro family this module recognizes defers exactly this kind of side
/// effect (see
/// [`image::apply_image_side_effects`](super::image::apply_image_side_effects)'
/// s own note): while the additive builder runs *alongside* the authoritative
/// string pipeline – each against its own, independent [`Parser`] – performing
/// it from every additive pass would risk double-counting a registration once
/// the two paths ever share one `Parser`. This function is the last of the
/// deferred registrations, staged as its own building block for the eventual
/// cutover (design §5.2, Phase 4 step 6): re-attaching it for real means
/// calling it exactly once per parse, after the single-pass builder replaces
/// the recorder as `Content`'s tree source, so nothing here is wired into a
/// real parse yet – it is exercised only by this module's own tests, against
/// their own `Parser`.
///
/// `source` is the whole original content span being processed, used – like
/// [`image::apply_image_side_effects`](super::image::apply_image_side_effects)'
/// s own `source` parameter – to locate the duplicate-id warning exactly as
/// [`InlineAnchorReplacer`](crate::content::macros) does (against the
/// content's own span, not the individual anchor's).
///
/// `leading_anchor_registered` mirrors
/// [`apply_macros_with_leading_anchor_registered`](super::apply_macros)'s own
/// parameter: a description-list term pre-registers its own leading
/// `[[id]]`/`[[id,reftext]]` before running the macros pass (see
/// `DefinedTerm::substitute` in `blocks::list_item_marker`), so – once this
/// function is wired in for real at the same call site – passing `true` there
/// suppresses the duplicate-id warning this function would otherwise raise
/// for that very same anchor, which sits at byte offset `0` of `source`.
/// Every other caller passes `false`.
///
/// Recurses into every container an id-bearing node can be nested inside –
/// [`Styled`](InlineNode::Styled), [`Ref`](InlineNode::Ref), and
/// [`Footnote`](InlineNode::Footnote) children – mirroring exactly where the
/// image and link increments' own side-effect functions recurse.
pub(crate) fn apply_ref_side_effects(
    nodes: &[InlineNode<'_>],
    parser: &Parser,
    source: Span<'_>,
    leading_anchor_registered: bool,
) {
    for node in nodes {
        match node {
            InlineNode::Anchor(anchor) => {
                // A bibliography anchor registers under its own [`RefType`],
                // from its own earlier pass (see
                // [`apply_biblio_side_effects`]), so it is skipped here.
                if !anchor.is_bibliography && !is_bibliography_inner(anchor, source) {
                    let reftext = anchor_reftext_str(anchor);

                    if parser
                        .register_ref(&anchor.id, reftext, RefType::Anchor)
                        .is_err()
                        && !(leading_anchor_registered
                            && anchor.location.byte_offset() == source.byte_offset())
                    {
                        parser.record_substitution_warning(
                            source,
                            WarningType::DuplicateId(anchor.id.to_string()),
                        );
                    }
                }
            }

            InlineNode::Styled(styled) => {
                if let Some(id) = &styled.id {
                    let _ = parser.register_ref(id, None, RefType::Anchor);
                }

                apply_ref_side_effects(&styled.children, parser, source, leading_anchor_registered);
            }

            InlineNode::Ref(reference) => {
                apply_ref_side_effects(
                    &reference.children,
                    parser,
                    source,
                    leading_anchor_registered,
                );
            }

            InlineNode::Footnote(footnote) => {
                apply_ref_side_effects(
                    &footnote.children,
                    parser,
                    source,
                    leading_anchor_registered,
                );
            }

            _ => {}
        }
    }
}

/// Performs the recognition side effect the string pipeline's
/// `InlineBiblioAnchorReplacer` attaches to a **bibliography** anchor: it
/// registers the entry's id under [`RefType::Bibliography`], with the bracketed
/// label the node carries as its `reftext` (so a cross-reference to the entry
/// renders identically to the label shown in the flow), and raises the same
/// duplicate-id warning against the whole content's `source` span when the id
/// is already taken.
///
/// Kept separate from [`apply_ref_side_effects`] – rather than folded into its
/// walk – because the string pipeline runs the bibliography-anchor pass
/// **first**, ahead of every other macro family, and
/// [`apply_macro_side_effects`](super::apply_macro_side_effects) must reproduce
/// that order: a duplicate-id warning from a bibliography anchor precedes an
/// image's dangerous-link-scheme warning in the one shared warnings list, the
/// same ordering concern that function's own doc comment already records for
/// image-before-anchor.
///
/// The pattern is `^`-anchored, so a bibliography anchor is always the
/// content's *first* node and is never nested inside a container – hence no
/// recursion here (and none needed for the bracketed label either: it stays in
/// the flow as ordinary sibling nodes, see [`biblio_anchor_level`]).
///
/// As with every staged side effect in this module, **nothing here is wired
/// into a real parse path yet** – it is exercised only by this module's own
/// tests, against their own [`Parser`].
pub(crate) fn apply_biblio_side_effects(
    nodes: &[InlineNode<'_>],
    parser: &Parser,
    source: Span<'_>,
) {
    let Some(InlineNode::Anchor(anchor)) = nodes.first() else {
        return;
    };

    if !anchor.is_bibliography {
        return;
    }

    if parser
        .register_ref(
            &anchor.id,
            anchor_reftext_str(anchor),
            RefType::Bibliography,
        )
        .is_err()
    {
        parser.record_substitution_warning(source, WarningType::DuplicateId(anchor.id.to_string()));
    }
}

/// The reference text `str` a built [`Anchor`] node's `reftext` carries, when
/// it is populated (a single verbatim [`Text`](InlineNode::Text) child – see
/// [`build_anchor_reftext`]), mirroring the `Option<&str>`
/// [`register_ref`](Parser::register_ref) itself expects.
fn anchor_reftext_str<'a>(anchor: &'a Anchor<'_>) -> Option<&'a str> {
    match anchor.reftext.as_deref() {
        Some([InlineNode::Text { value, .. }]) => Some(value.as_ref()),
        _ => None,
    }
}

/// Mirrors `InlineAnchorReplacer`'s own `is_bibliography_inner` check: a
/// shorthand `[[id]]` anchor immediately preceded by a `[` in the source is
/// the inner anchor of a bibliography-style `[[[id]]]` sequence appearing
/// *outside* a bibliography list item (inside one, a genuine bibliography
/// anchor at the entry's start is consumed whole by the separate,
/// list-item-gated [`biblio_anchor_level`] pass, so it never reaches this
/// function). Asciidoctor's own
/// inline-anchor *scan* excludes a `[[id]]` preceded by a `[`, so it renders
/// the anchor (already handled by [`build_anchor_node`], which does not
/// exclude this case – see its own doc) but never catalogs the id; this
/// mirrors that by skipping only the registration, not the recognition. See
/// #769.
///
/// This peeks at `anchor.location`'s own bytes and the byte immediately
/// before it in `source`, which is only honest for a **verbatim** anchor: its
/// `location` is then a precise slice of the real source, so both reads are
/// meaningful. A **synthesized** anchor's `id` (design §4.4's coarse-fallback
/// case, lifted for this family by [`text_slice`]) instead carries a
/// `location` that is only the enclosing run's *whole* coarse span, not the
/// exact `[[id]]` text – peeking at a byte relative to that span would answer
/// a question about the wrong bytes (the source immediately before the
/// *attribute reference*, not before the *id* inside its expanded value), so
/// this bails out to `false` (not bibliography-inner) for any non-verbatim id
/// rather than risk a wrong answer in either direction from bytes that were
/// never the id's own. A genuinely bibliography-style `[[[id]]]` sequence
/// reached through an attribute expansion is therefore registered as an
/// ordinary reference rather than suppressed – a narrower, documented gap the
/// eventual `apply_ref_side_effects` wiring (design §5.2 Phase 4, step 6) can
/// close if it proves to matter, the same "a coarse fallback trades precision
/// for correctness of the common case" policy design §4.4 already establishes
/// elsewhere.
fn is_bibliography_inner(anchor: &Anchor<'_>, source: Span<'_>) -> bool {
    if !matches!(anchor.id, CowStr::Borrowed(_)) {
        return false;
    }

    if !anchor.location.data().starts_with("[[") {
        return false;
    }

    // `anchor.location` always falls within `source` (every node this builder
    // produces slices from the same root it was built against), so this is
    // never a subtraction underflow.
    let local_offset = anchor.location.byte_offset() - source.byte_offset();

    local_offset
        .checked_sub(1)
        .and_then(|i| source.data().as_bytes().get(i))
        == Some(&b'[')
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use super::super::super::test_support::{
        assert_styled, assert_text, build_src, fold_html, golden_macros, golden_macros_with,
        golden_passthroughs_with,
    };
    use crate::{
        Parser, Span,
        inlines::{Anchor, InlineNode, SpanForm, StyleVariant},
        parser::HtmlSubstitutionRenderer,
        strings::CowStr,
    };

    /// Asserts that `node` is an [`Anchor`](InlineNode::Anchor), returning it.
    fn assert_anchor<'a, 'src>(node: &'a InlineNode<'src>) -> &'a Anchor<'src> {
        match node {
            InlineNode::Anchor(anchor) => anchor,

            other => panic!("expected an Anchor, got {other:?}"),
        }
    }

    #[test]
    fn fold_matches_the_string_pipeline_through_anchors() {
        // For each fixture, folding the single-pass tree (all five steps)
        // reproduces the string pipeline's output byte-for-byte. This is the
        // differential corpus (design §5.3) that pins the inline-anchor
        // increment. An anchor renders from its id alone, so every fixture is
        // recognized (there is no deferred-output boundary for anchors); the
        // reference-text cases additionally exercise the node's `reftext`
        // population, which does not affect the flow.
        let fixtures = [
            // No anchor despite bracket/colon characters.
            "plain text without an anchor",
            "a lone [ bracket and a colon : apart",
            "an anchor without brackets anchor:foo stays literal",
            // Shorthand: bare id, and with a reference text.
            "[[install]]",
            "[[install,Installation]]",
            "[[sect-one,Section One]]",
            // Macro form: empty brackets, and with a reference text.
            "anchor:install[]",
            "anchor:install[Installation]",
            // Id character classes: `_`, `:`, `-`, `.`, digits.
            "[[_foo]]",
            "[[:bar]]",
            "[[a-b.c:d]]",
            "[[sect_1]]",
            "anchor:a.b-c:d[]",
            // A reference text with trailing whitespace (trimmed by the string
            // replacer) and an escaped `]` (unescaped by the macro form).
            "[[install, Installation ]]",
            "anchor:foo[a\\]b]",
            // A shorthand whose reference text is whitespace only trims to empty:
            // the node carries no reference text, exactly as the bare form.
            "[[install, ]]",
            // Embedded in surrounding flow, and next to other constructs.
            "See [[here]] for details.",
            "text anchor:x[X] more",
            "*bold* then [[x]] and _em_",
            "a copyright (C) then [[x]]",
            "[[a]] and [[b]] and anchor:c[C]",
            // Escapes: the anchor stays literal, minus the backslash.
            "\\[[install]]",
            "\\[[install,Installation]]",
            "\\anchor:install[]",
            "\\anchor:foo[Ref]",
            // Recognized inside a rendered span.
            "*see [[x]]*",
            "_anchor:y[] in em_",
            // A reference text that is a rendered span or a special: the anchor is
            // still recognized (its id alone renders), and the reference text –
            // which never reaches the flow – is consumed with the match.
            "[[id,*bold*]]",
            "anchor:id[*bold*]",
            "[[id,A & B]]",
            // A triple-bracket `[[[id]]]` outside a bibliography list item is not
            // a bibliography anchor (that pass fires only inside such an item); the
            // inner `[[id]]` is a plain anchor with literal outer brackets, exactly
            // as the string pipeline routes it here.
            "[[[id]]]",
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
    fn an_anchor_shorthand_becomes_a_node() {
        let nodes = build_src(Span::new("[[install,Installation]]"));

        assert_eq!(nodes.len(), 1);
        let anchor = assert_anchor(&nodes[0]);

        // The id borrows from source (no allocation).
        assert!(matches!(anchor.id, CowStr::Borrowed(_)));
        assert_eq!(anchor.id.as_ref(), "install");

        // Its location covers the whole anchor, the `[[` / `]]` included.
        assert_eq!(anchor.location.data(), "[[install,Installation]]");
        assert_eq!(anchor.location.line(), 1);
        assert_eq!(anchor.location.col(), 1);

        // The reference text is a single borrowed `Text` child located at its
        // source (`[[install,` is 10 characters, so it starts at column 11).
        let reftext = anchor.reftext.as_ref().unwrap();
        assert_eq!(reftext.len(), 1);
        assert_text(&reftext[0], "Installation", 1, 11);
    }

    #[test]
    fn an_anchor_macro_becomes_a_node() {
        let nodes = build_src(Span::new("anchor:install[Installation]"));

        let anchor = assert_anchor(&nodes[0]);
        assert!(matches!(anchor.id, CowStr::Borrowed(_)));
        assert_eq!(anchor.id.as_ref(), "install");
        assert_eq!(anchor.location.data(), "anchor:install[Installation]");

        // `anchor:install[` is 15 characters, so the text starts at column 16.
        let reftext = anchor.reftext.as_ref().unwrap();
        assert_text(&reftext[0], "Installation", 1, 16);
    }

    #[test]
    fn a_bare_anchor_has_no_reftext() {
        // A shorthand without a `, reference text` and an empty-bracket macro
        // both leave `reftext` `None`.
        for source in ["[[install]]", "anchor:install[]"] {
            let nodes = build_src(Span::new(source));
            let anchor = assert_anchor(&nodes[0]);
            assert_eq!(anchor.id.as_ref(), "install");
            assert!(anchor.reftext.is_none(), "no reftext for {source:?}");
        }
    }

    #[test]
    fn an_anchor_shorthand_reftext_is_trimmed() {
        // A shorthand's trailing whitespace is stripped (the string replacer's
        // `trim_end`); leading whitespace was already excluded by the pattern's
        // `, \s*`.
        let nodes = build_src(Span::new("[[install, Installation ]]"));

        let anchor = assert_anchor(&nodes[0]);
        let reftext = anchor.reftext.as_ref().unwrap();

        // `[[install, ` is 11 characters, so the trimmed text starts at column 12.
        assert_text(&reftext[0], "Installation", 1, 12);
    }

    #[test]
    fn an_anchor_shorthand_reftext_that_is_whitespace_only_has_no_reftext() {
        // A shorthand reference text that trims to empty (the pattern's `(.+?)`
        // matched only the whitespace the string replacer's `trim_end` strips)
        // leaves `reftext` `None`, the same shape as the bare `[[id]]` form – and
        // it still folds to the same `<a id="…"></a>` the string pipeline emits.
        let source = "[[install, ]]";
        let nodes = build_src(Span::new(source));

        let anchor = assert_anchor(&nodes[0]);
        assert_eq!(anchor.id.as_ref(), "install");
        assert!(
            anchor.reftext.is_none(),
            "a whitespace-only reftext trims away"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn an_anchor_macro_reftext_unescapes_a_bracket() {
        // A macro reference text unescapes `\]` into `]`, making the logical text
        // a synthesized (owned) value whose `location` still covers the raw
        // source it derives from.
        let nodes = build_src(Span::new("anchor:foo[a\\]b]"));

        let anchor = assert_anchor(&nodes[0]);
        let reftext = anchor.reftext.as_ref().unwrap();
        assert_eq!(reftext.len(), 1);

        match &reftext[0] {
            InlineNode::Text { value, location } => {
                assert_eq!(value.as_ref(), "a]b");
                assert!(
                    matches!(value, CowStr::Boxed(_)),
                    "an unescaped value is synthesized (owned), got {value:?}"
                );
                assert_eq!(location.data(), "a\\]b");
            }

            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn an_anchor_is_recognized_inside_a_span() {
        // An anchor can appear inside a rendered span; the transducer descends
        // into the span body and builds the node there.
        let nodes = build_src(Span::new("*see [[x]]*"));

        let children = assert_styled(&nodes[0], StyleVariant::Strong, SpanForm::Constrained);
        assert_eq!(children.len(), 2);
        assert_text(&children[0], "see ", 1, 2);

        let anchor = assert_anchor(&children[1]);
        assert_eq!(anchor.id.as_ref(), "x");
    }

    #[test]
    fn an_escaped_anchor_stays_literal() {
        // `\[[…]]` and `\anchor:…[…]` drop the backslash and keep the anchor as
        // literal text – no anchor node – exactly as the string replacer's escape
        // branch does.
        for source in ["\\[[install]]", "\\anchor:install[Installation]"] {
            let nodes = build_src(Span::new(source));

            assert!(
                nodes.iter().all(|n| !matches!(n, InlineNode::Anchor(_))),
                "an escaped anchor must not produce an anchor node: {nodes:?}"
            );

            assert_eq!(
                fold_html(&nodes, &HtmlSubstitutionRenderer {}),
                golden_macros(source),
                "fold diverged for {source:?}"
            );
        }
    }

    #[test]
    fn an_anchor_reference_text_over_a_span_is_consumed() {
        // A reference text that is a rendered span (`[[id,*bold*]]`) does not
        // reach the flow: the anchor is still recognized (its id alone renders),
        // the span is consumed with the match, and the node's `reftext` is left
        // `None` (a non-verbatim reference text the builder does not slice from
        // `'src`). The fold still matches the string pipeline byte-for-byte.
        let source = "[[id,*bold*]]";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        let anchor = assert_anchor(&nodes[0]);
        assert_eq!(anchor.id.as_ref(), "id");
        assert!(
            anchor.reftext.is_none(),
            "a non-verbatim reference text is left unpopulated"
        );
        assert_eq!(anchor.location.data(), "[[id,*bold*]]");

        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});
        assert_eq!(folded, golden_macros(source));

        // The consumed span does not render into the flow.
        assert!(!folded.contains("<strong>"), "folded: {folded}");
    }

    /// The string pipeline's output through the **attribute-references** step
    /// for `source`, run against `parser` – the six steps [`build`] runs, in
    /// order (special characters, quotes, attribute references, character
    /// replacements, macros, post replacement). Unlike [`golden_macros_with`],
    /// this exercises `AttributeReferences` too, so an attribute whose
    /// expanded value contains `[[id]]` is spliced in before `Macros` runs –
    /// the scenario the divergence test below needs.
    fn golden_attributes_with(source: &str, parser: &Parser) -> String {
        use crate::content::{Content, SubstitutionStep};

        let mut content = Content::from(Span::new(source));
        SubstitutionStep::SpecialCharacters.apply(&mut content, parser, None);
        SubstitutionStep::Quotes.apply(&mut content, parser, None);
        SubstitutionStep::AttributeReferences.apply(&mut content, parser, None);
        SubstitutionStep::CharacterReplacements.apply(&mut content, parser, None);
        SubstitutionStep::Macros.apply(&mut content, parser, None);
        SubstitutionStep::PostReplacement.apply(&mut content, parser, None);
        content.rendered_str().to_string()
    }

    #[test]
    fn an_anchor_inside_an_expanded_attribute_value_is_now_recognized() {
        // An attribute reference whose resolved value happens to contain
        // `[[id]]` (design §3.4.1's "a macro inside an expanded value" case,
        // reached here through an anchor's own id instead of a target/
        // attribute list). The string pipeline splices the value in during
        // `AttributeReferences`, then genuinely recognizes the anchor when
        // `Macros` runs over the now-literal `[[custom-id]]` text. Once this
        // was a documented divergence (#1177 made `build_anchor_node` defer
        // here rather than build a wrongly-sourced node); this increment
        // lifts it: [`text_slice`] recovers the id's exact text from the
        // synthesized run, so the anchor is now recognized and the fold
        // matches the golden string pipeline byte-for-byte.
        use crate::parser::ModificationContext;

        let parser = Parser::default().with_intrinsic_attribute(
            "myattr",
            "[[custom-id]]",
            ModificationContext::Anywhere,
        );

        let source = "before {myattr} after";
        let nodes = build(Span::new(source), &parser, None);

        let anchor = nodes
            .iter()
            .find_map(|n| match n {
                InlineNode::Anchor(anchor) => Some(anchor),
                _ => None,
            })
            .unwrap_or_else(|| panic!("expected an Anchor node: {nodes:?}"));

        assert_eq!(anchor.id.as_ref(), "custom-id");
        // The id has no honest `'src` slice of its own (it comes from the
        // attribute's resolved value), so it is necessarily owned.
        assert!(matches!(anchor.id, CowStr::Boxed(_)));

        let golden = golden_attributes_with(source, &parser);
        assert!(
            golden.contains(r##"id="custom-id""##),
            "golden fixture stopped recognizing the anchor: {golden:?}"
        );

        let folded = crate::content::inline_builder::fold_html(
            &nodes,
            &HtmlSubstitutionRenderer {},
            &parser,
        );

        assert_eq!(folded, golden, "fold diverged from the string pipeline");
    }

    #[test]
    fn an_anchor_is_recognized_when_the_whole_seed_is_synthesized() {
        // The same boundary as the test above, reached at the tree's root
        // instead of a nested splice: `build_from_value`'s synthesized-seed
        // path (design §4.4, the shape `Content::from_filtered_lines`
        // produces for a genuinely multi-line, filtered block) now also
        // recognizes an anchor, mirroring `mod.rs`'s own
        // `a_macro_construct_is_deferred_when_the_whole_seed_is_synthesized`
        // – which still pins this boundary for the *other* macro families
        // (link/image/xref) this increment does not touch.
        use crate::content::inline_builder::build_from_value;

        let filtered = "see [[target]] here";
        let source = "  see [[target]] here";

        let parser = Parser::default();
        let nodes = build_from_value(
            CowStr::from(filtered.to_string()),
            Span::new(source),
            &parser,
            None,
        );

        let anchor = nodes
            .iter()
            .find_map(|n| match n {
                InlineNode::Anchor(anchor) => Some(anchor),
                _ => None,
            })
            .unwrap_or_else(|| panic!("expected an Anchor node: {nodes:?}"));

        assert_eq!(anchor.id.as_ref(), "target");

        let golden = golden_macros(filtered);
        assert!(
            golden.contains(r##"id="target""##),
            "golden fixture stopped recognizing the anchor: {golden:?}"
        );

        let folded = crate::content::inline_builder::fold_html(
            &nodes,
            &HtmlSubstitutionRenderer {},
            &parser,
        );
        assert_eq!(folded, golden, "fold diverged from the real pipeline");
    }

    #[test]
    fn an_anchors_reftext_inside_an_expanded_attribute_value_is_now_recognized() {
        // `build_anchor_reftext`'s own synthesized branch: the reference text
        // – not just the id – can come from a synthesized run too. Its
        // `location` falls back to the coarse enclosing span (design §4.4)
        // rather than a sub-slice of it, since there is no honest source
        // position to slice for owned bytes; only the recovered `value` is
        // exact.
        use crate::parser::ModificationContext;

        let parser = Parser::default().with_intrinsic_attribute(
            "myattr",
            "[[custom-id,Custom Text]]",
            ModificationContext::Anywhere,
        );

        let source = "before {myattr} after";
        let nodes = build(Span::new(source), &parser, None);

        let anchor = nodes
            .iter()
            .find_map(|n| match n {
                InlineNode::Anchor(anchor) => Some(anchor),
                _ => None,
            })
            .unwrap_or_else(|| panic!("expected an Anchor node: {nodes:?}"));

        assert_eq!(anchor.id.as_ref(), "custom-id");

        let reftext = anchor.reftext.as_ref().unwrap();
        assert_eq!(reftext.len(), 1);
        match &reftext[0] {
            InlineNode::Text { value, .. } => {
                assert_eq!(value.as_ref(), "Custom Text");
                assert!(matches!(value, CowStr::Boxed(_)));
            }
            other => panic!("expected Text, got {other:?}"),
        }

        let golden = golden_attributes_with(source, &parser);
        let folded = crate::content::inline_builder::fold_html(
            &nodes,
            &HtmlSubstitutionRenderer {},
            &parser,
        );
        assert_eq!(folded, golden, "fold diverged from the string pipeline");
    }

    // ---- `apply_ref_side_effects` (staged for the eventual cutover) -------

    use super::apply_ref_side_effects;
    use crate::{content::inline_builder::build, document::RefType, warnings::WarningType};

    /// Builds the single-pass tree for `source` against `parser` (unlike
    /// [`build_src`], which always uses its own fresh default parser).
    fn build_with<'src>(source: Span<'src>, parser: &Parser) -> Vec<InlineNode<'src>> {
        build(source, parser, None)
    }

    #[test]
    fn registers_an_anchor_id_in_the_catalog() {
        let source = "[[install]]";
        let parser = Parser::default();
        let nodes = build_with(Span::new(source), &parser);

        apply_ref_side_effects(&nodes, &parser, Span::new(source), false);

        let catalog = parser.catalog();
        assert!(catalog.contains_id("install"));
        assert_eq!(catalog.get_ref("install").unwrap().reftext, None);
    }

    #[test]
    fn registers_an_anchors_reftext() {
        let source = "[[install,Installation]]";
        let parser = Parser::default();
        let nodes = build_with(Span::new(source), &parser);

        apply_ref_side_effects(&nodes, &parser, Span::new(source), false);

        let catalog = parser.catalog();
        assert_eq!(
            catalog.get_ref("install").unwrap().reftext.as_deref(),
            Some("Installation")
        );
    }

    #[test]
    fn records_a_duplicate_id_warning_for_a_repeated_anchor() {
        let source = "[[a]] [[a]]";
        let parser = Parser::default();
        let nodes = build_with(Span::new(source), &parser);

        let before = parser.substitution_warnings_len();
        apply_ref_side_effects(&nodes, &parser, Span::new(source), false);
        let warnings = parser.drain_substitution_warnings_since(before);

        // The first registration wins; the catalog holds one entry.
        assert_eq!(parser.catalog().ids().collect::<Vec<_>>(), ["a"]);

        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::DuplicateId("a".to_string())
        );
    }

    #[test]
    fn registers_an_attributed_spans_id() {
        // `[#anchor]*bold*` assigns an id to the span via its attribute list
        // (see `attributes_of`'s own note on this deferred registration).
        let source = "[#anchor]*bold*";
        let parser = Parser::default();
        let nodes = build_with(Span::new(source), &parser);

        apply_ref_side_effects(&nodes, &parser, Span::new(source), false);

        assert!(parser.catalog().contains_id("anchor"));
    }

    #[test]
    fn an_attributed_spans_duplicate_id_is_silently_non_fatal() {
        // Unlike an inline anchor, a duplicate id assigned via an attributed
        // span raises no warning, mirroring the string pipeline's own
        // `let _ = register_ref(...)` in `SubstitutionStep::Quotes`.
        let source = "[#dup]*a* [#dup]*b*";
        let parser = Parser::default();
        let nodes = build_with(Span::new(source), &parser);

        let before = parser.substitution_warnings_len();
        apply_ref_side_effects(&nodes, &parser, Span::new(source), false);

        assert_eq!(parser.substitution_warnings_len(), before);
        assert_eq!(parser.catalog().ids().collect::<Vec<_>>(), ["dup"]);
    }

    #[test]
    fn does_not_register_the_inner_anchor_of_a_bibliography_style_triple_bracket() {
        // The `[[id]]` inside `[[[id]]]` is recognized as an `Anchor` node
        // (see the differential corpus above), but – outside a bibliography
        // list item – the string pipeline renders it without cataloging its
        // id (`is_bibliography_inner`); this mirrors that.
        let source = "[[[id]]]";
        let parser = Parser::default();
        let nodes = build_with(Span::new(source), &parser);

        apply_ref_side_effects(&nodes, &parser, Span::new(source), false);

        assert!(!parser.catalog().contains_id("id"));
    }

    #[test]
    fn a_bibliography_style_triple_bracket_reached_through_a_synthesized_run_still_registers() {
        // `is_bibliography_inner`'s own documented gap: it only recognizes the
        // `[[[id]]]` shape via a *verbatim* anchor's `location`, since a
        // synthesized anchor's `location` is only the enclosing run's coarse
        // span (not the literal `[[id]]` text), so peeking at its bytes could
        // never answer the "preceded by `[`" question honestly. Unlike the
        // test above (a literal `[[[id]]]` in real source, suppressed), the
        // same shape reached through an attribute expansion is registered as
        // an ordinary reference instead – the anchor is still recognized (its
        // id is exact, per the fold-parity tests above), just not suppressed.
        use crate::parser::ModificationContext;

        let parser = Parser::default().with_intrinsic_attribute(
            "myattr",
            "[[[id]]]",
            ModificationContext::Anywhere,
        );

        let source = "before {myattr} after";
        let nodes = build_with(Span::new(source), &parser);

        let anchor = nodes
            .iter()
            .find_map(|n| match n {
                InlineNode::Anchor(anchor) => Some(anchor),
                _ => None,
            })
            .unwrap_or_else(|| panic!("expected an Anchor node: {nodes:?}"));
        assert!(matches!(anchor.id, CowStr::Boxed(_)), "id is synthesized");

        apply_ref_side_effects(&nodes, &parser, Span::new(source), false);

        assert!(
            parser.catalog().contains_id("id"),
            "a synthesized bibliography-style anchor is registered, not suppressed"
        );
    }

    #[test]
    fn leading_anchor_registered_suppresses_the_warning_for_the_anchor_at_the_start() {
        // Mirrors `DefinedTerm::substitute`'s own pre-registration dance: the
        // id is already registered by the time this pass runs, and – because
        // the anchor sits at byte offset `0` of `source` – the
        // `leading_anchor_registered` flag suppresses the warning the second
        // (redundant) registration attempt would otherwise raise.
        let source = "[[install]]";
        let parser = Parser::default();
        parser
            .register_ref("install", None, RefType::Anchor)
            .unwrap();
        let nodes = build_with(Span::new(source), &parser);

        let before = parser.substitution_warnings_len();
        apply_ref_side_effects(&nodes, &parser, Span::new(source), true);

        assert_eq!(parser.substitution_warnings_len(), before);
    }

    #[test]
    fn leading_anchor_registered_still_warns_for_an_anchor_not_at_the_start() {
        // The suppression is specific to the anchor at offset `0`; a
        // duplicate anywhere else still warns even with the flag set.
        let source = "x [[install]]";
        let parser = Parser::default();
        parser
            .register_ref("install", None, RefType::Anchor)
            .unwrap();
        let nodes = build_with(Span::new(source), &parser);

        let before = parser.substitution_warnings_len();
        apply_ref_side_effects(&nodes, &parser, Span::new(source), true);
        let warnings = parser.drain_substitution_warnings_since(before);

        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::DuplicateId("install".to_string())
        );
    }

    #[test]
    fn registers_ids_nested_inside_a_styled_span_a_ref_and_a_footnote() {
        use crate::inlines::{Ref, RefVariant};

        let root = Span::new("[[nested]]");
        let anchor = build_with(root, &Parser::default());
        assert_eq!(anchor.len(), 1);

        let reference = InlineNode::Ref(Ref {
            variant: RefVariant::Link,
            target: CowStr::from("https://example.org"),
            children: anchor,
            roles: vec![],
            window: None,
            resolved: None,
            derived: None,
            xrefstyle: None,
            attrs: None,
            location: root,
        });

        let source = "*see [[a]]* and footnote:[see [[b]]]";
        let parser = Parser::default();
        let mut nodes = build_with(Span::new(source), &parser);
        nodes.push(reference);

        apply_ref_side_effects(&nodes, &parser, Span::new(source), false);

        assert_eq!(
            parser.catalog().ids().collect::<Vec<_>>(),
            ["a", "b", "nested"]
        );
    }

    #[test]
    fn matches_the_golden_pipelines_registration_for_a_broad_fixture_set() {
        // Each fixture uses its own pair of *independent* parsers (design
        // §5.3's two-independent-parsers discipline, already established by
        // the image increment's own differential corpus): one that the
        // additive builder builds against and this function then walks, one
        // that the real string pipeline (`golden_macros_with`, which also
        // runs the `Quotes` step and so exercises the attributed-span
        // registration too) runs against directly.
        let fixtures = [
            "[[install]]",
            "[[install,Installation]]",
            "anchor:install[Installation]",
            "[#free_the_world]#free the world#",
            "[[a]] [[a]]",
            "[[[id]]]",
            "*see [[x]]* and footnote:[see [[y]]]",
        ];

        for fixture in fixtures {
            let builder_parser = Parser::default();
            let nodes = build_with(Span::new(fixture), &builder_parser);
            apply_ref_side_effects(&nodes, &builder_parser, Span::new(fixture), false);

            let golden_parser = Parser::default();
            golden_macros_with(fixture, &golden_parser);

            assert_eq!(
                builder_parser.catalog().ids().collect::<Vec<_>>(),
                golden_parser.catalog().ids().collect::<Vec<_>>(),
                "registered ids diverged for {fixture:?}"
            );
        }
    }

    // ---- the bibliography anchor (`[[[label]]]`) ------------------------

    use super::{apply_biblio_side_effects, biblio_anchor_level};

    /// A [`Parser`] flagged as substituting the principal text of a
    /// bibliography list item – the context `blocks::list_item` puts the parser
    /// in, and the only one in which either pipeline recognizes a bibliography
    /// anchor.
    fn biblio_parser() -> Parser {
        let parser = Parser::default();
        parser.in_bibliography_list_item.set(true);
        parser
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_a_bibliography_anchor() {
        // The differential corpus (design §5.3) pinning the bibliography-anchor
        // increment: for each fixture, folding the single-pass tree reproduces
        // the string pipeline's output byte-for-byte, with both sides run
        // against a parser flagged as being inside a bibliography list item.
        let fixtures = [
            // Both spellings, alone and prefixing a real entry.
            "[[[gof]]]",
            "[[[gof]]] Gamma, Erich et al. _Design Patterns_.",
            "[[[gof,GoF]]] Gamma, Erich et al. _Design Patterns_.",
            "[[[gof, GoF]]] leading space after the comma is dropped",
            // Label character classes (the pattern admits digits, but never a
            // leading one).
            "[[[_gof]]] leading underscore",
            "[[[:gof]]] leading colon",
            "[[[gof-2.a:b]]] punctuation in the label",
            "[[[gof1995]]] digits after the first character",
            // A label that must *not* be recognized: it starts with a digit,
            // so the entry keeps its literal brackets (and the inner `[[…]]`
            // falls through to the ordinary anchor pass, which renders it
            // without cataloging its id).
            "[[[1984]]] Orwell, George.",
            // Not at the start of the entry: the `^`-anchored pass declines it,
            // exactly as the string step does.
            "See [[[mid]]] inline.",
            // A backslash is not an escape here – `\\[[[x]]]` does not begin
            // with `[[[`, so it is not a bibliography anchor at all.
            "\\[[[gof]]] Gamma.",
            // An xreftext carrying flow constructs of its own: the label stays
            // in the flow, so every later family sees it exactly as the string
            // pipeline's later passes do.
            "[[[gof,see https://example.org]]] auto-linked inside the label",
            "[[[gof,see link:x.html[X]]]] a link macro inside the label",
            // A label carrying an escaped special: a `CharRef::Special` piece
            // contributes its canonical entity to this level's match string, so
            // the label is reconstructed – and registered – exactly as the
            // string replacer captures it.
            "[[[gof,A & B]]] an escaped special inside the label",
            // Constructs after the entry's anchor.
            "[[[gof]]] *bold* and _em_ and https://example.org",
            "[[[gof]]] an inline [[mid]] anchor later in the entry",
        ];

        let parser = biblio_parser();
        let renderer = HtmlSubstitutionRenderer {};

        for fixture in fixtures {
            let folded = crate::content::inline_builder::fold_html(
                &build_with(Span::new(fixture), &parser),
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
    fn a_bibliography_anchor_becomes_a_node_followed_by_its_bracketed_label() {
        let parser = biblio_parser();
        let source = "[[[gof,GoF]]] Gamma.";
        let nodes = build_with(Span::new(source), &parser);

        let anchor = assert_anchor(&nodes[0]);
        assert!(anchor.is_bibliography);

        // The id borrows from source (no allocation).
        assert!(matches!(anchor.id, CowStr::Borrowed(_)));
        assert_eq!(anchor.id.as_ref(), "gof");

        // Its location covers the whole anchor, the triple brackets included.
        assert_eq!(anchor.location.data(), "[[[gof,GoF]]]");
        assert_eq!(anchor.location.line(), 1);
        assert_eq!(anchor.location.col(), 1);

        // The node's own reference text is the *bracketed* label – what the
        // entry is registered with, and what a cross-reference to it displays.
        let reftext = anchor.reftext.as_ref().unwrap();
        assert_eq!(reftext.len(), 1);
        match &reftext[0] {
            InlineNode::Text { value, .. } => assert_eq!(value.as_ref(), "[GoF]"),
            other => panic!("expected Text, got {other:?}"),
        }

        // The same bracketed label is *also* in the flow, as the sibling nodes
        // that follow – each sliced from the match's own source characters (the
        // outer `[` at column 1, the label, and the outer `]` at column 13).
        assert_text(&nodes[1], "[", 1, 1);
        assert_text(&nodes[2], "GoF", 1, 8);
        assert_text(&nodes[3], "]", 1, 13);
        assert_text(&nodes[4], " Gamma.", 1, 14);
        assert_eq!(nodes.len(), 5);
    }

    #[test]
    fn a_bibliography_anchor_with_no_xreftext_shows_its_label() {
        let parser = biblio_parser();
        let source = "[[[gof]]]";
        let nodes = build_with(Span::new(source), &parser);

        let anchor = assert_anchor(&nodes[0]);
        assert!(anchor.is_bibliography);

        match &anchor.reftext.as_ref().unwrap()[0] {
            InlineNode::Text { value, .. } => assert_eq!(value.as_ref(), "[gof]"),
            other => panic!("expected Text, got {other:?}"),
        }

        assert_text(&nodes[1], "[", 1, 1);
        assert_text(&nodes[2], "gof", 1, 4);
        assert_text(&nodes[3], "]", 1, 9);
        assert_eq!(nodes.len(), 4);
    }

    #[test]
    fn a_bibliography_anchor_is_recognized_only_inside_a_bibliography_list_item() {
        // The same source, built against a parser that is *not* flagged: the
        // triple bracket falls through to the ordinary inline-anchor pass, whose
        // node carries no bibliography flag (and whose id the side-effect pass
        // deliberately does not catalog – see
        // `does_not_register_the_inner_anchor_of_a_bibliography_style_triple_bracket`).
        let nodes = build_src(Span::new("[[[gof]]]"));

        let anchor = nodes
            .iter()
            .find_map(|n| match n {
                InlineNode::Anchor(anchor) => Some(anchor),
                _ => None,
            })
            .unwrap_or_else(|| panic!("expected an Anchor node: {nodes:?}"));

        assert!(!anchor.is_bibliography);
    }

    #[test]
    fn a_bibliography_anchor_inside_a_synthesized_run_is_recognized() {
        // A whole bibliography anchor supplied by an attribute reference: its
        // id and label have no `'src` slice of their own, but – as with the
        // ordinary anchor family's id – `text_slice` recovers their exact text,
        // so the anchor is recognized (its `location` taking design §4.4's
        // coarse fallback), and the fold matches the string pipeline.
        use crate::parser::ModificationContext;

        let parser = biblio_parser().with_intrinsic_attribute(
            "entry",
            "[[[gof,GoF]]]",
            ModificationContext::Anywhere,
        );

        let source = "{entry} Gamma.";
        let nodes = build_with(Span::new(source), &parser);

        let anchor = assert_anchor(&nodes[0]);
        assert!(anchor.is_bibliography);
        assert_eq!(anchor.id.as_ref(), "gof");
        assert!(matches!(anchor.id, CowStr::Boxed(_)), "id is synthesized");

        let folded = crate::content::inline_builder::fold_html(
            &nodes,
            &HtmlSubstitutionRenderer {},
            &parser,
        );

        assert_eq!(folded, golden_attributes_with(source, &parser));
    }

    #[test]
    fn a_bibliography_label_over_an_opaque_piece_is_a_documented_divergence() {
        // A label crossing an opaque piece – a rendered span, a passthrough, or
        // a character replacement, each a single placeholder in this level's
        // match string rather than the markup/entity the string pipeline's
        // haystack holds there – cannot be reconstructed as the
        // already-substituted text the string replacer registers and shows, so
        // the anchor is left unrecognized. The entry then keeps the shape it had
        // before this increment (the inner `[[…]]` as an ordinary anchor), which
        // is what diverges. This is exactly the boundary the index-term family's
        // own visible term documents, and – for the character replacements –
        // the same one every macro family already has at this point in the
        // pipeline (a `(C)` or a smart apostrophe is atomic by macro time).
        let parser = biblio_parser();

        for source in [
            "[[[gof,*GoF*]]] Gamma.",
            "[[[gof,+++<b>GoF</b>+++]]] Gamma.",
            "[[[gof,(C) 1995]]] Gamma.",
            "[[[oreilly,O'Reilly]]] Hunt.",
        ] {
            let nodes = build_with(Span::new(source), &parser);

            assert!(
                !nodes
                    .iter()
                    .any(|n| matches!(n, InlineNode::Anchor(a) if a.is_bibliography)),
                "expected no bibliography anchor node for {source:?}: {nodes:?}"
            );

            let folded = crate::content::inline_builder::fold_html(
                &nodes,
                &HtmlSubstitutionRenderer {},
                &parser,
            );

            assert_ne!(folded, golden_passthroughs_with(source, &parser));
        }
    }

    #[test]
    fn the_bibliography_pass_declines_a_level_it_can_never_match() {
        // The pass is `^`-anchored to the whole content, so it is a no-op for a
        // level whose text merely *contains* a triple bracket, and for one that
        // starts with it but under a parser that is not inside a bibliography
        // list item. Driven directly, since `apply_macros` only ever calls it at
        // the content's top level.
        let root = Span::new("x [[[gof]]]");
        let nodes = vec![InlineNode::Text {
            value: CowStr::from(root.data()),
            location: root,
        }];

        let out = biblio_anchor_level(nodes, root, &biblio_parser());
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], InlineNode::Text { .. }));
    }

    #[test]
    fn registers_a_bibliography_entry() {
        let source = "[[[gof,GoF]]] Gamma.";
        let parser = biblio_parser();
        let nodes = build_with(Span::new(source), &parser);

        apply_biblio_side_effects(&nodes, &parser, Span::new(source));

        let catalog = parser.catalog();
        let entry = catalog.get_ref("gof").unwrap();

        // The bracketed label is the registered reference text, so a
        // cross-reference to the entry renders exactly as the label does.
        assert_eq!(entry.reftext.as_deref(), Some("[GoF]"));
        assert_eq!(entry.ref_type, RefType::Bibliography);
    }

    #[test]
    fn matches_the_golden_pipelines_bibliography_registrations() {
        // Each side registers against its own *independent* parser (design
        // §5.3's two-independent-parsers discipline), so the staged side effect
        // is compared with the real pipeline's own registrations – id, reference
        // text, and `RefType` alike.
        let fixtures = [
            "[[[gof]]] Gamma.",
            "[[[gof,GoF]]] Gamma.",
            "[[[gof, GoF ]]] Gamma.",
            "[[[gof,A & B]]] Gamma.",
            "[[[1984]]] Orwell.",
            "See [[[mid]]] inline.",
            "[[[gof]]] and an inline [[extra]] anchor",
            // An ordinary anchor at the very start of the content: the
            // bibliography pass leaves it to `apply_ref_side_effects`, which
            // catalogs it under `RefType::Anchor`.
            "[[plain]] leads an entry that has no bibliography anchor",
        ];

        for fixture in fixtures {
            let builder_parser = biblio_parser();
            let nodes = build_with(Span::new(fixture), &builder_parser);
            apply_biblio_side_effects(&nodes, &builder_parser, Span::new(fixture));
            apply_ref_side_effects(&nodes, &builder_parser, Span::new(fixture), false);

            let golden_parser = biblio_parser();
            golden_macros_with(fixture, &golden_parser);

            let entries = |parser: &Parser| {
                let catalog = parser.catalog();

                catalog
                    .ids()
                    .map(|id| {
                        let entry = catalog.get_ref(id).unwrap();
                        (
                            id.to_string(),
                            entry.reftext.clone(),
                            entry.ref_type.clone(),
                        )
                    })
                    .collect::<Vec<_>>()
            };

            assert_eq!(
                entries(&builder_parser),
                entries(&golden_parser),
                "registered references diverged for {fixture:?}"
            );
        }
    }

    #[test]
    fn records_a_duplicate_id_warning_for_a_repeated_bibliography_entry() {
        let source = "[[[gof]]] Gamma.";
        let parser = biblio_parser();
        parser
            .register_ref("gof", None, RefType::Bibliography)
            .unwrap();

        let nodes = build_with(Span::new(source), &parser);

        let before = parser.substitution_warnings_len();
        apply_biblio_side_effects(&nodes, &parser, Span::new(source));
        let warnings = parser.drain_substitution_warnings_since(before);

        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::DuplicateId("gof".to_string())
        );
    }

    #[test]
    fn a_bibliography_anchor_is_registered_once_not_twice() {
        // `apply_ref_side_effects` skips a bibliography anchor (its own earlier
        // pass owns it), so composing the two – as
        // `apply_macro_side_effects` does – neither double-registers the entry
        // nor raises a spurious duplicate-id warning against itself.
        let source = "[[[gof]]] Gamma.";
        let parser = biblio_parser();
        let nodes = build_with(Span::new(source), &parser);

        let before = parser.substitution_warnings_len();
        apply_biblio_side_effects(&nodes, &parser, Span::new(source));
        apply_ref_side_effects(&nodes, &parser, Span::new(source), false);

        assert_eq!(parser.substitution_warnings_len(), before);
        assert_eq!(
            parser.catalog().get_ref("gof").unwrap().ref_type,
            RefType::Bibliography
        );
    }

    #[test]
    fn a_real_bibliography_list_items_tree_folds_to_its_rendered_string() {
        // End-to-end, through the real parse path: the flag this pass reads is
        // set by `blocks::list_item` while the entry's principal text is
        // substituted, and `SubstitutionGroup::apply` clones the parser (flag
        // included) to build the tree – so a real bibliography entry's tree
        // folds to exactly the rendered string the string pipeline produced.
        use crate::blocks::{FindBlocks, IsBlock};

        let doc = Parser::default().with_inline_tree(true).parse(
            "[bibliography]\n* [[[gof,GoF]]] Gamma, Erich et al.\n* [[[pp]]] Hunt, Andrew.\n",
        );

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
                    &Parser::default(),
                ),
                rendered,
                "fold diverged from the rendered string for {inlines:?}"
            );

            folded_blocks += 1;
        }

        assert_eq!(folded_blocks, 2, "expected both entries to be checked");
    }
}
