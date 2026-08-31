//! Inline anchor recognition (`[[id]]`, `[[id,reftext]]`, `anchor:id[…]`), and
//! the bibliography anchor (`[[[label]]]`) that prefixes a bibliography list
//! item.

use super::{
    MacroMatch, MacroMatchKind, emit_range_unescaping_brackets,
    image::range_is_verbatim_or_synthesized, rebuild_macro_level,
};
use crate::{
    Parser, Span,
    content::{
        INLINE_ANCHOR, INLINE_BIBLIO_ANCHOR,
        inline_builder::{
            fold_html,
            quotes::{
                LevelContext, Piece, SPAN_PLACEHOLDER, build_match_string, emit_range,
                range_overlaps_synthesized, single_text_value, source_slice, text_slice,
            },
            special_chars::Masked,
        },
    },
    document::RefType,
    inlines::{Anchor, InlineNode},
    strings::CowStr,
    warnings::WarningType,
};

/// Matches `INLINE_BIBLIO_ANCHOR` at the **content's own top level**, replacing
/// a bibliography anchor (`[[[label]]]` / `[[[label,xreftext]]]`) with the
/// [`Anchor`](InlineNode::Anchor) node it produces — `is_bibliography` set —
/// followed by the bracketed label pushed into the flow, matching
/// Asciidoctor's own rendering.
///
/// # Where this runs, and why only here
///
/// This pass runs **first**, ahead of every other macro
/// family, and only when the parser flags that it is substituting the principal
/// text of a bibliography list item
/// ([`in_bibliography_list_item`](Parser::in_bibliography_list_item), set in
/// `blocks::list_item`) — matching Asciidoctor on both counts. The pattern is
/// `^`-anchored — a
/// `[[[…]]]` appearing later in the entry is left to the ordinary inline-anchor
/// pass, which renders it but never catalogs its id (see
/// [`is_bibliography_inner`]) — so this level pass runs once, at the top level
/// [`apply_macros`](super::apply_macros) is called with, and never descends
/// into a span's children: `^` matches only the very start of the *whole*
/// content, matching Asciidoctor's own anchoring.
///
/// # The bracketed label stays in the flow
///
/// The anchor renders from its id alone (`render_anchor(id,
/// None)`), and the bracketed label (`[label]`, or `[xreftext]` when
/// one was supplied) is pushed into the output as ordinary text — text every
/// *later* step then scans. So the label is emitted here as the sibling nodes
/// that follow the anchor node (sliced from the match's own outer brackets and
/// its label range with [`emit_range`], so each keeps its exact `'src`
/// provenance), rather than as the anchor's own children: that is what lets
/// every family after this one see the label as ordinary flow text (an
/// auto-link written in an xreftext is linked),
/// with no container to descend into.
///
/// The node's own `reftext` instead carries the bracketed label as the
/// **registered** reference text — what a cross-reference to the entry
/// displays, and what [`apply_biblio_side_effects`] hands
/// [`register_ref`](Parser::register_ref) — one
/// `format!("[{label}]")` serving both the displayed and the registered
/// purpose.
///
/// # The registered label is already-substituted text
///
/// The registered label is captured in *escaped,
/// already-substituted* form (`[[[gof,A & B]]]`
/// catalogs `[A &amp; B]`), so the node's `reftext` holds the label in that
/// same already-substituted form — the contract an
/// [`IndexTerm`](InlineNode::IndexTerm)'s own `terms` already uses — taken
/// straight from this level's match string (a [`CharRef`](InlineNode::CharRef)
/// contributes its
/// canonical entity, so an escaped special and a character replacement alike
/// come out byte-identical to what Asciidoctor captures). Nothing re-escapes
/// it: the fold hands `render_anchor` `None` for a bibliography anchor (see
/// `fold_anchor`), matching Asciidoctor's own rendering.
///
/// # Deferred: a label crossing an opaque piece
///
/// What the match string cannot reconstruct is an opaque piece — a rendered
/// [`Styled`](crate::inlines::Styled) span (`[[[gof,*G*]]]`), a passthrough or
/// STEM expression (not even restored yet), or a character replacement
/// (`[[[gof,(C) 1995]]]`, `[[[oreilly,O'Reilly]]]`) — which stands in as a
/// single [`SPAN_PLACEHOLDER`] here rather than as the markup or entity
/// itself. Such an anchor is left unrecognized,
/// exactly the boundary the index-term family's own visible term documents
/// (and, for the character replacements, the same one every macro family
/// already has at this point: `build_match_string` serves the quotes step too,
/// where the replacements have not run yet, so it can only treat them as
/// opaque). A label reached through a synthesized run (an attribute expansion,
/// or a filtered block's joined seed) *is* recognized — the run contributes its
/// expanded value to the match string directly.
///
/// As in the additive builder generally, this performs *no* recognition side
/// effect itself; [`apply_biblio_side_effects`] performs the `register_ref`
/// call once per parse, after the tree is built and folded.
pub(super) fn biblio_anchor_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
    masked: Masked<'_>,
) -> Vec<InlineNode<'src>> {
    if !parser.in_bibliography_list_item.get() {
        return nodes;
    }

    // Cheap pre-filter, taken *before* the match string is materialized: a
    // single, unsplit `Text` node's match string is its own value, so the
    // check below can run against that directly. A level already split by
    // an earlier step falls back to the build, exactly as before.
    if single_text_value(&nodes).is_some_and(|value| !value.starts_with("[[[")) {
        return nodes;
    }

    let (s, pieces) = build_match_string(&nodes, masked);

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
    // supplied, else the id itself — `caps.get(2)…unwrap_or(id)`.
    let label_match = caps.get(2).unwrap_or(id_match);

    let id_range = id_match.start()..id_match.end();
    let label_range = label_match.start()..label_match.end();

    // The label is registered (and shown) as already-substituted text, which
    // this level's match string reproduces for every piece except an opaque one
    // — a rendered span, a passthrough, or a STEM expression — which stands in
    // as a single placeholder.
    let label = match s.get(label_range.clone()) {
        Some(label) if !label.contains(SPAN_PLACEHOLDER) => label,
        _ => return nodes,
    };

    let reftext = CowStr::from(format!("[{label}]"));

    // The id, by contrast, rides on the node as logical text, so it is sliced
    // back to `'src` (borrowing where it can) exactly as an ordinary anchor's
    // own id is. Its character class admits neither a special nor a
    // placeholder, so the two readings coincide — and, for the same reason,
    // the `None` arm (the id crossing an atomic piece) is not actually
    // reachable, kept only for symmetry with [`build_anchor_node`]'s own
    // gate.
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

    // The bracketed label pushed into the flow. Its brackets are
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

/// An anchor needs either the shorthand `[[` opener or the `anchor:` macro
/// prefix. The `[` characters are not special, so a shorthand reaches the
/// macros step with its `[[` intact. Shared between
/// [`anchor_macros_level`]'s pre-build sniff and its post-build one, so the
/// two answers cannot drift apart.
fn anchor_prefilter(s: &str) -> bool {
    s.contains("[[") || s.contains("anchor:")
}

/// Matches `INLINE_ANCHOR` at this level's escaped text, replacing each
/// recognized inline anchor — the `[[id]]` / `[[id,reftext]]` shorthand and the
/// `anchor:id[reftext]` macro — with the [`Anchor`](InlineNode::Anchor) node it
/// produces and leaving everything else in place.
pub(super) fn anchor_macros_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    ctx: LevelContext,
    masked: Masked<'_>,
) -> Vec<InlineNode<'src>> {
    // Cheap pre-filter, taken *before* the match string is materialized: see
    // `biblio_anchor_level`'s own copy of this comment.
    if single_text_value(&nodes).is_some_and(|value| !anchor_prefilter(value)) {
        return nodes;
    }

    let (s, pieces) = build_match_string(&nodes, masked);

    // Cheap pre-filter: an anchor needs either the shorthand `[[` opener or the
    // `anchor:` macro prefix. The `[` characters are not special, so a
    // shorthand reaches the macros step with its `[[` intact.
    if !anchor_prefilter(&s) {
        return nodes;
    }

    // Matched over the level wrapped in the boundary character its enclosing
    // construct presents, with the level's own pieces moved into that string's
    // coordinates — see `apply_macro_families`'s own doc comment.
    let (s, pieces) = ctx.shift(s, pieces);

    let matches = find_anchor_matches(&nodes, &s, &pieces, root);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every recognized inline anchor at this level — both spellings — as a
/// [`MacroMatch`].
///
/// An anchor's HTML rendering (`<a id="…"></a>`) is a function of its **id
/// alone**, and an id admits no special character (the pattern's id class is
/// letters/digits/`_`/`-`/`:`/`.`), so an id crossing an *escaped special* or a
/// *rendered span* can never occur — unlike the link/xref families, an anchor
/// is never deferred on *that* boundary. An id's characters can, though, come
/// from a [`synthesized`](Piece::synthesized) run (an attribute reference whose
/// expanded value happens to contain `[[id]]`, or — reached at a tree's root —
/// a filtered multi-line block's own joined seed): [`build_anchor_node`] no
/// longer defers on that alone, recovering the id's exact text via
/// [`text_slice`] even though it has no honest `'src` slice of its own (the
/// "a macro inside an expanded value" boundary, reached here
/// through the id rather than a target/attribute list) — the node's
/// `location` still falls back to the coarse enclosing span used when a
/// construct has no `Span`-typed field of its own, since only the *text*
/// needed the precision. A non-verbatim
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
        // and keeping the rest literal, checked *before* anything else via a
        // leading `caps.get(1)` check. [`rebuild_macro_level`] emits the kept
        // range with [`emit_range`], which clones an atomic piece (a
        // rendered-span reference text) whole, so the unescape works
        // even across a non-verbatim reference text — just as the id
        // itself is always verbatim, the whole anchor
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
            // rendered span — never actually reachable given the id's own
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
/// `<a id="…"></a>` exactly. Returns `None` only when
/// the id crosses an [`atomic`](Piece::atomic) piece (see below) — a form this
/// increment still defers.
///
/// Two spellings share this builder: the `[[id,reftext]]` shorthand (groups
/// 2/3) and the `anchor:id[reftext]` macro (groups 4/5). Exactly one id group
/// matches.
///
/// An id's character class (letters/digits/`_`/`-`/`:`/`.`) admits no escaped
/// special or rendered span, so [`range_is_verbatim_or_synthesized`]'s atomic
/// check can in practice never fail for an id — it is kept for symmetry with
/// every other macro family's own gate. Its bytes *can* come from a
/// [`synthesized`](Piece::synthesized) run — an attribute reference whose
/// expanded value happens to contain `[[id]]`, or — reached at a tree's root —
/// a filtered multi-line block's own joined seed (the "a macro
/// inside an expanded value" boundary) — and [`text_slice`] recovers the exact
/// id text for that case too, unlike [`source_slice`], which would silently
/// fall back to the enclosing synthesized run's *coarse* span
/// for both the id and the node's `location`. Only `location` keeps that
/// coarse fallback here; the id text itself is always precise.
///
/// The optional reference text is captured as the node's `reftext` — a single
/// [`Text`](InlineNode::Text) child — whenever it does not cross an atomic
/// piece (the common verbatim case borrows `'src`; a synthesized one is
/// recovered via [`text_slice`] into an owned value, `location` falling back
/// to the coarse span exactly as the id's own does). A shorthand's trailing
/// whitespace is trimmed and a macro's escaped `\]` is unescaped, matching
/// Asciidoctor. A reference text that carries a rendered span or an
/// escaped special is left non-verbatim; because it never reaches the flow
/// (the anchor renders from its id alone), the anchor is still recognized but
/// its `reftext` is left `None` rather than sliced wrongly — a narrower
/// boundary than the id's own, and a shape a re-flow consumer can refine later
/// (the field is provisional, per the node's Phase-0 note).
///
/// As in the additive builder generally, this performs *no* recognition side
/// effect itself — notably it does **not** `register_ref` the id in the catalog
/// (so a cross-reference can resolve against it), nor emit the duplicate-id
/// warning; [`apply_ref_side_effects`] performs both once per parse, after the
/// tree is built and folded.
fn build_anchor_node<'src>(
    caps: &regex::Captures<'_>,
    full: &std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
    nodes: &[InlineNode<'src>],
) -> Option<InlineNode<'src>> {
    let location = source_slice(pieces, full.clone(), root);

    // Exactly one id group matches: group 2 for the `[[…]]` shorthand (with its
    // reference text in group 3), else group 4 for the `anchor:…[…]` macro
    // (with its reference text in group 5).
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

    let reftext = reftext_match.and_then(|m| {
        build_anchor_reftext(
            m.as_str(),
            m.start()..m.end(),
            pieces,
            root,
            nodes,
            is_shorthand,
        )
    });

    Some(InlineNode::Anchor(Anchor {
        id,
        reftext,
        is_bibliography: false,
        location,
    }))
}

/// Builds an inline anchor's `reftext` — a single [`Text`](InlineNode::Text)
/// child — from the reference-text capture's match-string `range`, or `None`
/// when the reference text crosses an atomic piece or trims to empty (see
/// [`build_anchor_node`] for why crossing an atomic piece is not an error for
/// the anchor as a whole).
///
/// A `shorthand` reference text has its trailing whitespace stripped
/// (`trim_end`, matching Asciidoctor; leading whitespace was already excluded
/// by the pattern's `, \s*`). A macro reference text unescapes an escaped `\]`
/// into an owned value, matching Asciidoctor's `replace("\\]", "]")`.
///
/// The verbatim case (the common one) keeps its exact prior shape: the value
/// borrows `'src`, and a shorthand's `location` is sliced down to the trimmed
/// text precisely. A [`synthesized`](Piece::synthesized) range instead
/// recovers its exact text via [`text_slice`] but keeps the whole range's
/// coarse `location` regardless of trimming or unescaping —
/// sub-slicing a location has no honest meaning for bytes with no `'src`
/// counterpart of their own, the same policy
/// [`emit_range`] already applies to every fragment of an expanded value.
fn build_anchor_reftext<'src>(
    raw_text: &str,
    range: std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
    nodes: &[InlineNode<'src>],
    shorthand: bool,
) -> Option<Vec<InlineNode<'src>>> {
    if !range_is_verbatim_or_synthesized(pieces, &range) {
        return structural_anchor_reftext(raw_text, range, pieces, nodes, shorthand);
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

/// An anchor's reference text that crosses an **atomic** piece — an
/// earlier-recognized construct the reference text encloses
/// (`[[id,see image:t.png[T]]]`), carried here as one
/// [`SPAN_PLACEHOLDER`] rather than as the bytes it renders to.
///
/// No string built now can spell such a text: the construct's markup exists
/// only at fold time. So it is carried **structurally**, as the nodes the
/// range covers, which is what the field's own type has always allowed and
/// what the sibling families already do for a display text
/// ([`IndexTerm::children`](crate::inlines::IndexTerm), a link's or a
/// cross-reference's own children). Nothing about the anchor's *rendering*
/// changes — `render_anchor` emits the id and nothing else — but the reference
/// text is what a cross-reference to this anchor shows, and what the
/// registration walk descends into to find a construct hiding there.
///
/// The two byte rewrites the verbatim path performs with `str` methods are
/// performed as ranges instead: a shorthand's `trim_end` narrows the range
/// (trailing whitespace is ordinary text, never a placeholder), and a macro's
/// escaped `\]` drops its backslash as a *gap* between two emitted ranges —
/// the same structural unescape
/// [`emit_range_unescaping_brackets`] performs for the reference-bearing
/// families.
fn structural_anchor_reftext<'src>(
    raw_text: &str,
    range: std::ops::Range<usize>,
    pieces: &[Piece],
    nodes: &[InlineNode<'src>],
    shorthand: bool,
) -> Option<Vec<InlineNode<'src>>> {
    let mut out = Vec::new();

    if shorthand {
        // Trailing whitespace is ordinary text, never a placeholder, so
        // trimming it off the *text* trims exactly the same bytes off the
        // range. It cannot trim the range away entirely: this path is reached
        // only for a text crossing an **atomic** piece, which is not
        // whitespace — so the emptiness the verbatim path has to guard
        // against is unreachable here, and the `is_empty` check below covers
        // it anyway without a branch of its own.
        let trimmed = raw_text.trim_end();

        emit_range(
            nodes,
            pieces,
            range.start..(range.start + trimmed.len()),
            &mut out,
        );
    } else {
        emit_range_unescaping_brackets(raw_text, range, nodes, pieces, &mut out);
    }

    (!out.is_empty()).then_some(out)
}

/// Performs the recognition side effects an assigned id needs at two distinct
/// points — an inline anchor (`[[id]]` / `anchor:id[…]`) and the
/// attributed-quote handling in
/// [`SubstitutionStep::Quotes`](crate::content::SubstitutionStep::Quotes)
/// (`[#id]#…#`) — by walking the built tree and reading each
/// [`Anchor`](InlineNode::Anchor) node's own stored `id`/`reftext` and each
/// [`Styled`](crate::inlines::Styled) span's own optional `id`. Both register
/// the id in the document's reference catalog
/// under [`RefType::Anchor`] so a later cross-reference can resolve against
/// it; only the inline-anchor form also raises a duplicate-id warning (the
/// attributed-span form is silently non-fatal —
/// see [`attributes_of`](super::super::quotes::attributes_of)'s own note).
///
/// Every macro family this module recognizes defers exactly this kind of side
/// effect (see
/// [`image::apply_image_side_effects`](super::image::apply_image_side_effects)'
/// s own note): recognition and registration are kept separate so a family's
/// own tests can build and inspect a tree without a full `Parser`/`Content`
/// round trip.
/// [`SubstitutionGroup::apply`](crate::content::SubstitutionGroup) calls this
/// once per content, from the tree it just built.
///
/// `source` is the whole original content span being processed, used — like
/// [`image::apply_image_side_effects`](super::image::apply_image_side_effects)'
/// s own `source` parameter — to locate the duplicate-id warning exactly as
/// [`InlineAnchorReplacer`](crate::content::macros) does (against the
/// content's own span, not the individual anchor's).
///
/// `leading_anchor_registered` says a description-list **term**'s leading
/// `[[id]]`/`[[id,reftext]]` was already registered, with the rest of the term
/// as its default reference text — the term's own rule, which runs from the
/// same tree just before this function (see
/// `SubstitutionGroup::apply_to_description_list_term`). Passing `true`
/// suppresses the duplicate-id warning this function would otherwise raise for
/// that very anchor, which sits at byte offset `0` of `source`. Every other
/// caller passes `false`.
///
/// Recurses into every container an id-bearing node can be nested inside —
/// [`Styled`](InlineNode::Styled), [`Ref`](InlineNode::Ref),
/// [`Footnote`](InlineNode::Footnote), and
/// [`IndexTerm`](InlineNode::IndexTerm) children, and an
/// [`Anchor`](InlineNode::Anchor)'s own `reftext` — mirroring exactly where the
/// image and link increments' own side-effect functions recurse.
///
/// The five are every nested node list an [`InlineNode`] holds: the four
/// `children` fields, and an [`Anchor`](InlineNode::Anchor)'s `reftext`, which
/// is one despite not being named like one. A sixth would be a new place a
/// macro node can hide, and the corpus-wide side-effect sweep
/// (`tests::inline_builder_side_effect_parity`) is what would catch one going
/// unwalked, as it caught `IndexTerm` and `reftext` in turn.
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
                    let reftext = anchor_reftext_string(anchor, parser);

                    if parser
                        .register_ref(&anchor.id, reftext.as_deref(), RefType::Anchor)
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

                if let Some(reftext) = &anchor.reftext {
                    apply_ref_side_effects(reftext, parser, source, leading_anchor_registered);
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

            InlineNode::IndexTerm(index_term) => {
                apply_ref_side_effects(
                    &index_term.children,
                    parser,
                    source,
                    leading_anchor_registered,
                );
            }

            _ => {}
        }
    }
}

/// Performs the recognition side effect a **bibliography** anchor needs: it
/// registers the entry's id under [`RefType::Bibliography`], with the bracketed
/// label the node carries as its `reftext` (so a cross-reference to the entry
/// renders identically to the label shown in the flow), and raises the same
/// duplicate-id warning against the whole content's `source` span when the id
/// is already taken.
///
/// Kept separate from [`apply_ref_side_effects`] — rather than folded into its
/// walk — because the bibliography-anchor pass runs
/// **first**, ahead of every other macro family, and
/// [`apply_macro_side_effects`](super::apply_macro_side_effects) must preserve
/// that order: a duplicate-id warning from a bibliography anchor precedes an
/// image's dangerous-link-scheme warning in the one shared warnings list, the
/// same ordering concern that function's own doc comment already records for
/// image-before-anchor.
///
/// The pattern is `^`-anchored, so a bibliography anchor is always the
/// content's *first* node and is never nested inside a container — hence no
/// recursion here (and none needed for the bracketed label either: it stays in
/// the flow as ordinary sibling nodes, see [`biblio_anchor_level`]).
///
/// As with every recognition side effect in this module, this now runs on the
/// real parse path — see
/// [`apply_macro_side_effects`](super::apply_macro_side_effects). It is also
/// exercised directly by this module's own
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
            anchor_reftext_string(anchor, parser).as_deref(),
            RefType::Bibliography,
        )
        .is_err()
    {
        parser.record_substitution_warning(source, WarningType::DuplicateId(anchor.id.to_string()));
    }
}

/// The reference text a built [`Anchor`] node's `reftext` carries, as the
/// **string** [`register_ref`](Parser::register_ref) takes.
///
/// A cross-reference to this anchor shows what the catalog holds, so the
/// registered text has to be the reference text's *rendering* — matching
/// Asciidoctor, which likewise renders it before cataloging the id.
///
/// A reference text of a single verbatim [`Text`](InlineNode::Text) run — the
/// overwhelmingly common case — is that rendering already, and contributes its
/// bytes unchanged, exactly as the field's original single-`Text` reader gave
/// them. A construct the reference text **encloses**
/// ([`structural_anchor_reftext`]) has no such bytes until the tree is folded,
/// so it is folded here, through the parser's own renderer: the same trade
/// [`restorable_body`](super::image::restorable_body) makes for a `Stem`, and
/// the same one the link families' own attribute lists make for a rendered
/// span. Folding is faithful because these bytes go into the catalog rather
/// than straight to output, and a cross-reference reaching them is rendered by
/// this same renderer.
fn anchor_reftext_string(anchor: &Anchor<'_>, parser: &Parser) -> Option<String> {
    let reftext = anchor.reftext.as_deref()?;
    let mut out = String::new();

    for node in reftext {
        match node {
            // A reference text's own [`Text`](InlineNode::Text) runs carry the
            // level's **match-string** bytes — already substituted, since a
            // reference text is read after the escaping and quotes steps have
            // run over the content. Folding one would escape it a second time
            // (`[&#169; 1995]` → `[&amp;#169; 1995]`), so it contributes its
            // value as it stands, exactly as the field's original single-`Text`
            // reader did.
            InlineNode::Text { value, .. } => out.push_str(value.as_ref()),

            // Everything else is an earlier-recognized construct the reference
            // text encloses, whose bytes exist only at fold time.
            other => out.push_str(&fold_html(
                std::slice::from_ref(other),
                parser.renderer.as_ref(),
                &parser.render_context(),
            )),
        }
    }

    Some(out)
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
/// exclude this case — see its own doc) but never catalogs the id; this
/// mirrors that by skipping only the registration, not the recognition. See
/// #769.
///
/// This peeks at `anchor.location`'s own bytes and the byte immediately
/// before it in `source`, which is only honest for a **verbatim** anchor: its
/// `location` is then a precise slice of the real source, so both reads are
/// meaningful. A **synthesized** anchor's `id` (the coarse-fallback
/// case, lifted for this family by [`text_slice`]) instead carries a
/// `location` that is only the enclosing run's *whole* coarse span, not the
/// exact `[[id]]` text — peeking at a byte relative to that span would answer
/// a question about the wrong bytes (the source immediately before the
/// *attribute reference*, not before the *id* inside its expanded value), so
/// this bails out to `false` (not bibliography-inner) for any non-verbatim id
/// rather than risk a wrong answer in either direction from bytes that were
/// never the id's own. A genuinely bibliography-style `[[[id]]]` sequence
/// reached through an attribute expansion is therefore registered as an
/// ordinary reference rather than suppressed — a narrower, documented gap that
/// could be closed in [`apply_ref_side_effects`] if it proves to matter, the
/// same "a coarse fallback trades precision
/// for correctness of the common case" policy already established
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
        content::inline_builder::special_chars::Masked,
        inlines::{Anchor, InlineNode, SpanForm, StyleVariant},
        parser::HtmlInlineRenderer,
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
        // reproduces the frozen recording byte-for-byte. This is the
        // differential corpus that pins inline-anchor behavior. An anchor
        // renders from its id alone, so every fixture is
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
            // A reference text with trailing whitespace (trimmed, matching
            // Asciidoctor) and an escaped `]` (unescaped by the macro form).
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
            // still recognized (its id alone renders), and the reference text —
            // which never reaches the flow — is consumed with the match.
            "[[id,*bold*]]",
            "anchor:id[*bold*]",
            "[[id,A & B]]",
            // A triple-bracket `[[[id]]]` outside a bibliography list item is not
            // a bibliography anchor (that pass fires only inside such an item); the
            // inner `[[id]]` is a plain anchor with literal outer brackets.
            "[[[id]]]",
        ];

        let renderer = HtmlInlineRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_src(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden_macros(fixture),
                "fold diverged from the frozen recording for {fixture:?}"
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
        // A shorthand's trailing whitespace is stripped (`trim_end`, matching
        // Asciidoctor); leading whitespace was already excluded by the
        // pattern's `, \s*`.
        let nodes = build_src(Span::new("[[install, Installation ]]"));

        let anchor = assert_anchor(&nodes[0]);
        let reftext = anchor.reftext.as_ref().unwrap();

        // `[[install, ` is 11 characters, so the trimmed text starts at column
        // 12.
        assert_text(&reftext[0], "Installation", 1, 12);
    }

    #[test]
    fn an_anchor_shorthand_reftext_that_is_whitespace_only_has_no_reftext() {
        // A shorthand reference text that trims to empty (the pattern's `(.+?)`
        // matched only the whitespace `trim_end` strips)
        // leaves `reftext` `None`, the same shape as the bare `[[id]]` form —
        // and it still folds to the same `<a id="…"></a>`.
        let source = "[[install, ]]";
        let nodes = build_src(Span::new(source));

        let anchor = assert_anchor(&nodes[0]);
        assert_eq!(anchor.id.as_ref(), "install");
        assert!(
            anchor.reftext.is_none(),
            "a whitespace-only reftext trims away"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn an_anchor_macro_reftext_unescapes_a_bracket() {
        // A macro reference text unescapes `\]` into `]`, making the logical
        // text a synthesized (owned) value whose `location` still
        // covers the raw source it derives from.
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
        // literal text — no anchor node.
        for source in ["\\[[install]]", "\\anchor:install[Installation]"] {
            let nodes = build_src(Span::new(source));

            assert!(
                nodes.iter().all(|n| !matches!(n, InlineNode::Anchor(_))),
                "an escaped anchor must not produce an anchor node: {nodes:?}"
            );

            assert_eq!(
                fold_html(&nodes, &HtmlInlineRenderer {}),
                golden_macros(source),
                "fold diverged for {source:?}"
            );
        }
    }

    #[test]
    fn a_cross_reference_shows_a_structural_reference_text_in_a_real_document() {
        // What the reference text is *for*, end to end: a cross-reference to
        // the anchor shows what the catalog holds for it, so a reference text
        // enclosing an earlier-recognized construct has to reach the catalog
        // with that construct's own rendering in it. Driven through the real
        // parse path, where the registration and the resolution both happen.
        use crate::blocks::{FindBlocks, IsBlock};

        // The heading carries no content of its own, so the walk below also
        // reaches its `None` arm.
        let doc = crate::Parser::default().parse(concat!(
            "== A heading\n",
            "\n",
            "[[tgt,see image:t.png[T] there]]The target.\n",
            "\n",
            "Back to <<tgt>>.\n",
        ));

        let mut checked = 0;

        for block in doc.descendant_blocks() {
            let (Some(rendered), Some(inlines)) = (block.rendered_html_content(), block.inlines())
            else {
                continue;
            };

            assert_eq!(
                crate::content::inline_builder::fold_html(
                    inlines,
                    &HtmlInlineRenderer {},
                    &crate::Parser::default().render_context()
                ),
                rendered,
                "fold diverged from the rendered string for {inlines:?}"
            );

            checked += 1;
        }

        assert_eq!(checked, 2, "expected both paragraphs to carry a tree");

        // The reference text the catalog holds carries the image's own
        // rendering, and that is what the cross-reference shows.
        assert_eq!(
            doc.catalog().get_ref("tgt").and_then(|e| e.reftext.clone()),
            Some(r##"see <span class="image"><img src="t.png" alt="T"></span> there"##.to_string())
        );
    }

    #[test]
    fn an_anchor_reference_text_over_a_span_is_carried_structurally() {
        // A reference text enclosing a rendered span (`[[id,*bold*]]`) does not
        // reach the flow — the anchor's id alone renders, and the span is
        // consumed with the match — but it *is* what a cross-reference to this
        // anchor shows, so the node carries it: as the nodes the range covers,
        // since no string built at parse time can spell the span's markup.
        let source = "[[id,*bold*]]";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        let anchor = assert_anchor(&nodes[0]);
        assert_eq!(anchor.id.as_ref(), "id");
        assert_eq!(anchor.location.data(), "[[id,*bold*]]");

        match anchor.reftext.as_deref() {
            Some([InlineNode::Styled(_)]) => {}

            other => panic!("expected a reference text of the span itself, got {other:?}"),
        }

        let folded = fold_html(&nodes, &HtmlInlineRenderer {});
        assert_eq!(folded, golden_macros(source));

        // The consumed span does not render into the flow.
        assert!(!folded.contains("<strong>"), "folded: {folded}");
    }

    /// The frozen recording of `source`'s rendered output through the
    /// **attribute-references** step, run against `parser` — the six steps
    /// [`build`] runs, in order (special characters, quotes, attribute
    /// references, character replacements, macros, post replacement).
    /// Unlike [`golden_macros_with`], this exercises `AttributeReferences`
    /// too, so an attribute whose expanded value contains `[[id]]` is
    /// spliced in before `Macros` runs — the scenario the divergence test
    /// below needs.
    fn golden_attributes_with(source: &str, _parser: &Parser) -> String {
        crate::content::inline_builder::snapshot::recorded("anchors_attributes", source)
    }

    #[test]
    fn an_anchor_inside_an_expanded_attribute_value_is_now_recognized() {
        // An attribute reference whose resolved value happens to contain
        // `[[id]]` (the "a macro inside an expanded value" case,
        // reached here through an anchor's own id instead of a target/
        // attribute list). The value is spliced in during the
        // attribute-references step, then genuinely recognized as an anchor
        // once the macros step runs over the now-literal `[[custom-id]]`
        // text: [`text_slice`] recovers the id's exact text from the
        // synthesized run, so the fold matches the frozen recording
        // byte-for-byte. (Once this was a documented divergence — #1177 made
        // `build_anchor_node` defer here rather than build a wrongly-sourced
        // node.)
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
            &HtmlInlineRenderer {},
            &parser.render_context(),
        );

        assert_eq!(folded, golden, "fold diverged from the frozen recording");
    }

    #[test]
    fn an_anchor_is_recognized_when_the_whole_seed_is_synthesized() {
        // The same boundary as the test above, reached at the tree's root
        // instead of a nested splice: `build_from_value`'s synthesized-seed
        // path (the shape `Content::from_filtered_lines`
        // produces for a genuinely multi-line, filtered block) now also
        // recognizes an anchor, mirroring `mod.rs`'s own
        // `a_macro_construct_is_deferred_when_the_whole_seed_is_synthesized`
        // — which still pins this boundary for the *other* macro families
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
            &HtmlInlineRenderer {},
            &parser.render_context(),
        );
        assert_eq!(folded, golden, "fold diverged from the real pipeline");
    }

    #[test]
    fn an_anchors_reftext_inside_an_expanded_attribute_value_is_now_recognized() {
        // `build_anchor_reftext`'s own synthesized branch: the reference text
        // — not just the id — can come from a synthesized run too. Its
        // `location` falls back to the coarse enclosing span
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
            &HtmlInlineRenderer {},
            &parser.render_context(),
        );
        assert_eq!(folded, golden, "fold diverged from the frozen recording");
    }

    // ---- `apply_ref_side_effects` -------

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
        // span raises no warning — `attributes_of` performs a
        // `let _ = register_ref(...)` there too.
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
        // (see the differential corpus above), but — outside a bibliography
        // list item — Asciidoctor renders it without cataloging its
        // id (`is_bibliography_inner`); this matches that.
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
        // an ordinary reference instead — the anchor is still recognized (its
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
        // id is already registered by the time this pass runs, and — because
        // the anchor sits at byte offset `0` of `source` — the
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
            link_form: Some(crate::inlines::LinkForm::Macro),
            target: CowStr::from("https://example.org"),
            children: anchor,
            roles: vec![],
            window: None,
            resolved: None,
            derived: None,
            xrefstyle: None,
            attrs: crate::attributes::Attrlist::empty(root.slice(0..0)),
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
    fn registers_the_recorded_ids_for_a_broad_fixture_set() {
        // Each expected set is **frozen at the last differentially-verified
        // parity**: while the crate's old string-rewriting implementation
        // existed, this test registered
        // each fixture through it on an independent parser and compared the
        // two catalogs, and the suite was green at the commit that deleted
        // it — so the literals below are that implementation's own answer,
        // recorded the same way every frozen corpus's bytes were.
        let fixtures = [
            ("[[install]]", r#"["install"]"#),
            ("[[install,Installation]]", r#"["install"]"#),
            ("anchor:install[Installation]", r#"["install"]"#),
            ("[#free_the_world]#free the world#", r#"["free_the_world"]"#),
            // An id carrying a special character: the *escaped* id is
            // registered, since the attribute list is parsed out of text the
            // escaping step already ran over (see
            // [`quote_attributes`](crate::content::inline_builder::quotes)).
            ("[#a&b]#x#", r#"["a&amp;b"]"#),
            ("[[a]] [[a]]", r#"["a"]"#),
            ("[[[id]]]", "[]"),
            ("*see [[x]]* and footnote:[see [[y]]]", r#"["x", "y"]"#),
        ];

        for (fixture, expected) in fixtures {
            let builder_parser = Parser::default();
            let nodes = build_with(Span::new(fixture), &builder_parser);
            apply_ref_side_effects(&nodes, &builder_parser, Span::new(fixture), false);

            assert_eq!(
                format!("{:?}", builder_parser.catalog().ids().collect::<Vec<_>>()),
                expected,
                "registered ids diverged for {fixture:?}"
            );
        }
    }

    // ---- the bibliography anchor (`[[[label]]]`) ------------------------

    use super::{apply_biblio_side_effects, biblio_anchor_level};

    /// A [`Parser`] flagged as substituting the principal text of a
    /// bibliography list item — the context `blocks::list_item` puts the parser
    /// in, and the only one in which either pipeline recognizes a bibliography
    /// anchor.
    fn biblio_parser() -> Parser {
        let parser = Parser::default();
        parser.in_bibliography_list_item.set(true);
        parser
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_a_bibliography_anchor() {
        // The differential corpus pinning bibliography-anchor behavior: for
        // each fixture, folding the single-pass tree reproduces
        // the frozen recording byte-for-byte, with both run
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
            // A backslash is not an escape here — `\\[[[x]]]` does not begin
            // with `[[[`, so it is not a bibliography anchor at all.
            "\\[[[gof]]] Gamma.",
            // An xreftext carrying flow constructs of its own: the label stays
            // in the flow, so every later family sees it as ordinary flow
            // text.
            "[[[gof,see https://example.org]]] auto-linked inside the label",
            "[[[gof,see link:x.html[X]]]] a link macro inside the label",
            // A label carrying an escaped special: a `CharRef::Special` piece
            // contributes its canonical entity to this level's match string, so
            // the label is reconstructed — and registered — in the same
            // already-substituted form Asciidoctor captures.
            "[[[gof,A & B]]] an escaped special inside the label",
            // Constructs after the entry's anchor.
            "[[[gof]]] *bold* and _em_ and https://example.org",
            "[[[gof]]] an inline [[mid]] anchor later in the entry",
        ];

        let parser = biblio_parser();
        let renderer = HtmlInlineRenderer {};

        for fixture in fixtures {
            let folded = crate::content::inline_builder::fold_html(
                &build_with(Span::new(fixture), &parser),
                &renderer,
                &parser.render_context(),
            );

            assert_eq!(
                folded,
                golden_macros_with(fixture, &parser),
                "fold diverged from the frozen recording for {fixture:?}"
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

        // The node's own reference text is the *bracketed* label — what the
        // entry is registered with, and what a cross-reference to it displays.
        let reftext = anchor.reftext.as_ref().unwrap();
        assert_eq!(reftext.len(), 1);
        match &reftext[0] {
            InlineNode::Text { value, .. } => assert_eq!(value.as_ref(), "[GoF]"),
            other => panic!("expected Text, got {other:?}"),
        }

        // The same bracketed label is *also* in the flow, as the sibling nodes
        // that follow — each sliced from the match's own source characters (the
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
        // triple bracket falls through to the ordinary inline-anchor pass,
        // whose node carries no bibliography flag (and whose id the
        // side-effect pass deliberately does not catalog — see
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
        // id and label have no `'src` slice of their own, but — as with the
        // ordinary anchor family's id — `text_slice` recovers their exact text,
        // so the anchor is recognized (its `location` taking the coarse
        // fallback span), and the fold matches the frozen recording.
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
            &HtmlInlineRenderer {},
            &parser.render_context(),
        );

        assert_eq!(folded, golden_attributes_with(source, &parser));
    }

    #[test]
    fn a_bibliography_label_crossing_a_character_replacement_is_recognized() {
        // A label crossing a *typographic replacement* — a `(C)` the
        // replacements step turned into a copyright sign, a smart apostrophe —
        // is recognized, because `build_match_string` gives such a leaf the
        // entity the built-in backend renders it as (`&#169;`, `&#8217;`);
        // `text_slice` therefore recovers exactly the
        // already-substituted label that gets registered and shown.
        // (Once a documented divergence — a replacement was one opaque
        // placeholder, like a rendered span — before the "third recoverable
        // piece" work lifted it for every family at once.)
        let parser = biblio_parser();

        for (source, id, label) in [
            ("[[[gof,(C) 1995]]] Gamma.", "gof", "&#169; 1995"),
            ("[[[oreilly,O'Reilly]]] Hunt.", "oreilly", "O&#8217;Reilly"),
        ] {
            let nodes = build_with(Span::new(source), &parser);

            let anchor = assert_anchor(&nodes[0]);
            assert!(anchor.is_bibliography);
            assert_eq!(anchor.id.as_ref(), id);

            let folded = crate::content::inline_builder::fold_html(
                &nodes,
                &HtmlInlineRenderer {},
                &parser.render_context(),
            );

            assert_eq!(folded, golden_passthroughs_with(source, &parser));

            // The registered reference text is the already-substituted label.
            let side_effect_parser = biblio_parser();
            apply_biblio_side_effects(&nodes, &side_effect_parser, Span::new(source));

            let catalog = side_effect_parser.catalog();
            let entry = catalog.get_ref(id).unwrap();
            assert_eq!(
                entry.reftext.as_deref(),
                Some(format!("[{label}]").as_str())
            );
        }
    }

    #[test]
    fn a_bibliography_label_over_an_opaque_piece_is_a_documented_divergence() {
        // A label crossing an opaque piece — a rendered span or a passthrough,
        // each a single placeholder in this level's match string rather than
        // the markup itself — cannot be
        // reconstructed as the already-substituted text that gets registered
        // and shown, so the anchor is left unrecognized. The entry
        // then keeps the shape of an ordinary, unregistered anchor (the inner
        // `[[…]]` as an ordinary anchor), which is what diverges from the
        // frozen recording. This is exactly the
        // boundary the index-term family's own visible term documents, and the
        // one every macro family still has for a rendered span.
        let parser = biblio_parser();

        for source in [
            "[[[gof,*GoF*]]] Gamma.",
            "[[[gof,+++<b>GoF</b>+++]]] Gamma.",
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
                &HtmlInlineRenderer {},
                &parser.render_context(),
            );

            assert_ne!(folded, golden_passthroughs_with(source, &parser));
        }
    }

    #[test]
    fn the_bibliography_pass_declines_a_level_it_can_never_match() {
        // The pass is `^`-anchored to the whole content, so it is a no-op for a
        // level whose text merely *contains* a triple bracket, and for one that
        // starts with it but under a parser that is not inside a bibliography
        // list item. Driven directly, since `apply_macros` only ever calls it
        // at the content's top level.
        let root = Span::new("x [[[gof]]]");
        let nodes = vec![InlineNode::Text {
            value: CowStr::from(root.data()),
            location: root,
        }];

        let out = biblio_anchor_level(nodes, root, &biblio_parser(), Masked::UNKNOWN);
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], InlineNode::Text { .. }));
    }

    #[test]
    fn the_bibliography_pass_declines_a_multi_node_level_it_can_never_match() {
        // The single-`Text`-node pre-filter (`single_text_value`) is silent
        // for a level already split into more than one node, so this level's
        // "does it start with `[[[`?" answer still has to come from the built
        // match string — the same declined answer as the single-node case
        // above, reached by a different path. Driven directly with two
        // adjoining `Text` nodes, for the same reason that test is.
        let root = Span::new("x [[[gof]]]");

        let nodes = vec![
            InlineNode::Text {
                value: CowStr::from("x "),
                location: root.slice(0..2),
            },
            InlineNode::Text {
                value: CowStr::from("[[[gof]]]"),
                location: root.slice(2..11),
            },
        ];

        let out = biblio_anchor_level(nodes, root, &biblio_parser(), Masked::UNKNOWN);
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|n| matches!(n, InlineNode::Text { .. })));
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
    fn registers_the_recorded_bibliography_entries() {
        // Frozen at the last differentially-verified parity — see
        // `registers_the_recorded_ids_for_a_broad_fixture_set` for the
        // provenance of these literals (id, reference text, and `RefType`
        // alike were compared against the old string-rewriting
        // implementation's own registrations until the commit that deleted
        // it).
        let fixtures = [
            (
                "[[[gof]]] Gamma.",
                r#"[("gof", Some("[gof]"), RefType::Bibliography)]"#,
            ),
            (
                "[[[gof,GoF]]] Gamma.",
                r#"[("gof", Some("[GoF]"), RefType::Bibliography)]"#,
            ),
            (
                "[[[gof, GoF ]]] Gamma.",
                r#"[("gof", Some("[GoF ]"), RefType::Bibliography)]"#,
            ),
            (
                "[[[gof,A & B]]] Gamma.",
                r#"[("gof", Some("[A &amp; B]"), RefType::Bibliography)]"#,
            ),
            ("[[[1984]]] Orwell.", "[]"),
            ("See [[[mid]]] inline.", "[]"),
            (
                "[[[gof]]] and an inline [[extra]] anchor",
                r#"[("extra", None, RefType::Anchor), ("gof", Some("[gof]"), RefType::Bibliography)]"#,
            ),
            // An ordinary anchor at the very start of the content: the
            // bibliography pass leaves it to `apply_ref_side_effects`, which
            // catalogs it under `RefType::Anchor`.
            (
                "[[plain]] leads an entry that has no bibliography anchor",
                r#"[("plain", None, RefType::Anchor)]"#,
            ),
        ];

        for (fixture, expected) in fixtures {
            let builder_parser = biblio_parser();
            let nodes = build_with(Span::new(fixture), &builder_parser);
            apply_biblio_side_effects(&nodes, &builder_parser, Span::new(fixture));
            apply_ref_side_effects(&nodes, &builder_parser, Span::new(fixture), false);

            let entries = {
                let catalog = builder_parser.catalog();

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
                format!("{entries:?}"),
                expected,
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
        // pass owns it), so composing the two — as
        // `apply_macro_side_effects` does — neither double-registers the entry
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
        // included) to build the tree — so a real bibliography entry's tree
        // folds to exactly its own rendered string.
        use crate::blocks::{FindBlocks, IsBlock};

        let doc = Parser::default().parse(
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
                    &HtmlInlineRenderer {},
                    &Parser::default().render_context(),
                ),
                rendered,
                "fold diverged from the rendered string for {inlines:?}"
            );

            folded_blocks += 1;
        }

        assert_eq!(folded_blocks, 2, "expected both entries to be checked");
    }
}
