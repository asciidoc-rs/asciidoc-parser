//! The special-characters substitution step, and its counterparts for an
//! effective order that never runs it — or that runs it *late*, after a step
//! that already produced markup.

use super::fold_html;
use crate::{
    HasSpan, Parser, Span,
    inlines::{CharRef, InlineNode, RawForm, RawOrigin, RefVariant},
    parser::InlineRenderer,
    strings::CowStr,
};

/// Which leaf kind a literal `<`/`>`/`&` becomes when a text run is split.
///
/// The kind a fragment becomes is **not** a fixed property of where it came
/// from; it is decided by which substitution steps still act on it under the
/// group's effective order.
#[derive(Clone, Copy)]
enum SpecialLeaf {
    /// A [`CharRef::Special`] the fold escapes — what the `SpecialCharacters`
    /// step itself produces when it acts on a run.
    CharRef,

    /// A [`Raw`](InlineNode::Raw) leaf the fold emits verbatim — what a
    /// literal special is under an effective order that never runs
    /// `SpecialCharacters`, since nothing in that order escapes it.
    Raw,
}

/// The special-characters substitution, as a node transducer: every
/// [`Text`](InlineNode::Text) run is split on `<`/`>`/`&` into
/// [`Text`](InlineNode::Text) and [`CharRef`](InlineNode::CharRef) nodes, and
/// every other node passes through (recursing into parent nodes' children).
///
/// The split is driven by the node's **logical `value`**, not by its source
/// span, so a *synthesized* value — an attribute expansion or a joined
/// multi-line run that a later step may feed in under a custom `subs` order —
/// is preserved rather than replaced by its source spelling. Precise spans are
/// kept for the common verbatim run, where the value coincides with the source
/// its `location` covers; see [`split_text`].
pub(super) fn apply_special_characters<'src>(
    nodes: Vec<InlineNode<'src>>,
) -> Vec<InlineNode<'src>> {
    let mut out = Vec::with_capacity(nodes.len());

    for node in nodes {
        match node {
            InlineNode::Text { value, location } => {
                split_text(value, location, SpecialLeaf::CharRef, &mut out);
            }

            InlineNode::Styled(mut styled) => {
                styled.children = apply_special_characters(styled.children);
                out.push(InlineNode::Styled(styled));
            }

            InlineNode::Ref(mut reference) => {
                reference.children = apply_special_characters(reference.children);
                out.push(InlineNode::Ref(reference));
            }

            other => out.push(other),
        }
    }

    out
}

/// Classifies every literal `<`/`>`/`&` left in the finished tree as a
/// [`Raw`](InlineNode::Raw) leaf — the policy for an effective substitution
/// order whose steps **never include**
/// [`SpecialCharacters`](crate::content::SubstitutionStep::SpecialCharacters).
///
/// A [`Text`](InlineNode::Text) node is *logical* text the fold escapes,
/// which is exactly right when the `SpecialCharacters` step acted on the
/// content — and exactly wrong when it never ran, because there the author's
/// `<` renders unescaped. `subs=quotes` on a paragraph, a
/// passthrough block ([`Pass`](crate::content::SubstitutionGroup::Pass)), a
/// comment block ([`None`](crate::content::SubstitutionGroup::None)), and
/// `subs=callouts` on a listing block all take that path, so the classification
/// has to follow the *order*, not the node's origin.
///
/// This runs **after** every one of the group's own steps rather than in place
/// of `apply_special_characters`, and that ordering is what keeps it faithful:
/// under such an order every other step still matches over text in which the
/// specials are still literal, so every transducer must see them as ordinary
/// [`Text`](InlineNode::Text) characters — not as the opaque leaf a `Raw` node
/// is to [`build_match_string`](super::quotes::build_match_string). Only the
/// finished tree's *classification* differs, so nothing about recognition
/// changes.
///
/// It recurses into every container a text run can be nested inside — a
/// [`Styled`](crate::inlines::Styled) span, a [`Ref`](crate::inlines::Ref)'s
/// own display children, an [`Anchor`](crate::inlines::Anchor)'s reference
/// text, and a [`Footnote`](crate::inlines::Footnote)'s own children —
/// mirroring the containers [`fold_html`] itself descends into.
pub(super) fn classify_unescaped_specials<'src>(
    nodes: Vec<InlineNode<'src>>,
) -> Vec<InlineNode<'src>> {
    let mut out = Vec::with_capacity(nodes.len());

    for node in nodes {
        match node {
            InlineNode::Text { value, location } => {
                split_text(value, location, SpecialLeaf::Raw, &mut out);
            }

            InlineNode::Styled(mut styled) => {
                styled.children = classify_unescaped_specials(styled.children);
                out.push(InlineNode::Styled(styled));
            }

            InlineNode::Ref(mut reference) => {
                reference.children = classify_unescaped_specials(reference.children);
                out.push(InlineNode::Ref(reference));
            }

            InlineNode::Anchor(mut anchor) => {
                anchor.reftext = anchor.reftext.map(classify_unescaped_specials);
                out.push(InlineNode::Anchor(anchor));
            }

            InlineNode::Footnote(mut footnote) => {
                footnote.children = classify_unescaped_specials(footnote.children);
                out.push(InlineNode::Footnote(footnote));
            }

            InlineNode::IndexTerm(mut index_term) => {
                index_term.children = classify_unescaped_specials(index_term.children);
                out.push(InlineNode::IndexTerm(index_term));
            }

            other => out.push(other),
        }
    }

    out
}

/// Rebuilds a value the macros step **computed** off the level's match string
/// under an effective order whose escaping step has not run by the time
/// `Macros` reaches it — the
/// [`Verbatim`](super::macros::ComputedSpecials::Verbatim) half of the decision
/// [`ComputedSpecials`](super::macros::ComputedSpecials) carries, and the
/// counterpart of
/// [`escaped_value_children`](super::macros::escaped_value_children).
///
/// There is no entity to unwind here: the value's bytes are the author's own,
/// spliced into the tree exactly as they stand. So this is the same
/// classification [`classify_unescaped_specials`] performs
/// over the finished tree — a literal `<`/`>`/`&` is a
/// [`Raw`](InlineNode::Raw) leaf the fold emits verbatim, everything else a
/// [`Text`](InlineNode::Text) run — reached through the very same
/// [`split_text`], so the two cannot drift on what a literal special is worth.
///
/// Running it *here*, rather than leaving the value as one `Text` run for
/// `classify_unescaped_specials` to split later, is what covers the second of
/// the two orders this half serves: an order that escapes **after** `Macros`
/// (`subs=macros,specialcharacters`) never reaches that final pass, and
/// [`flatten_prior_markup`] folds this node's markup — including this value —
/// before the escaping step splits the result.
///
/// An **empty** value yields no children at all, matching
/// `escaped_value_children`'s own answer for one: unlike the empty `Text` a
/// `<<id,>>` reference text is built with directly, an empty *computed* value
/// is a value the caller has already filtered out.
pub(super) fn unescaped_value_children<'src>(
    text: &str,
    location: Span<'src>,
) -> Vec<InlineNode<'src>> {
    if text.is_empty() {
        return vec![];
    }

    let mut out = Vec::new();
    split_text(
        CowStr::from(text.to_string()),
        location,
        SpecialLeaf::Raw,
        &mut out,
    );
    out
}

/// Splits a [`Text`](InlineNode::Text) node's logical `value` into alternating
/// text runs and `<`/`>`/`&` leaves of the kind `leaf` names.
///
/// When `value` is exactly the source its `location` covers — the common
/// verbatim run — each sub-node is sliced from `location`, so its
/// `line`/`col`/`offset` stay honest (issue #944) and its run text borrows from
/// `'src`. When `value` is *synthesized* — it has no source of its own — the
/// runs are owned slices of the value and every sub-node falls back to the
/// whole `location` span, the documented coarse fallback.
///
/// An **empty** value is kept as the node it already is rather than split.
/// Neither splitter ever emits an empty run (there is nothing in one to
/// escape), so splitting an empty node would silently delete it — and an empty
/// `Text` can be load-bearing: a `<<id,>>` cross-reference's present-but-empty
/// reference text is exactly one, and the fold tells it from an absent text by
/// the child's *presence* (see `build_xref_shorthand_node` in
/// [`macros`](super::macros)).
/// Finds the first special character in `text` — one `memchr` sweep over the
/// three bytes rather than a per-character `is_special` scan, which for the
/// common special-free run is the whole cost of splitting it — returning its
/// byte offset and which special it is. The three specials are ASCII, so
/// byte search and character search coincide: a special's byte never occurs
/// inside a multi-byte character's encoding.
fn find_special(text: &str) -> Option<(usize, char)> {
    let pos = memchr::memchr3(b'<', b'>', b'&', text.as_bytes())?;

    // The byte at `pos` is one of the three searched-for bytes; reading it
    // back tells which. The catch-all arm is the ampersand's, `get` having
    // no `None` to answer for an offset `memchr3` itself just found.
    let ch = match text.as_bytes().get(pos) {
        Some(b'<') => '<',
        Some(b'>') => '>',
        _ => '&',
    };

    Some((pos, ch))
}

fn split_text<'src>(
    value: CowStr<'src>,
    location: Span<'src>,
    leaf: SpecialLeaf,
    out: &mut Vec<InlineNode<'src>>,
) {
    if value.is_empty() {
        out.push(InlineNode::Text { value, location });
    } else if value.as_ref() == location.data() {
        split_verbatim(location, leaf, out);
    } else {
        split_synthesized(value.as_ref(), location, leaf, out);
    }
}

/// Splits a verbatim run — text that coincides with the source `location`
/// covers — slicing each sub-span from `location` with the crate's span
/// primitives so `line`/`col`/`offset` stay honest; a run is never emitted
/// empty.
fn split_verbatim<'src>(location: Span<'src>, leaf: SpecialLeaf, out: &mut Vec<InlineNode<'src>>) {
    let mut rest = location;

    // Finding the character alongside its offset keeps the special's own
    // `char` in hand, so neither arm below has to re-derive it from the sliced
    // span through a fallible `chars().next()` whose failure branch could
    // never be reached (and so could never be tested).
    while let Some((pos, ch)) = find_special(rest.data()) {
        // Emit the borrowed text run preceding the special, when non-empty.
        if pos > 0 {
            let text = rest.slice_to(..pos);

            out.push(InlineNode::Text {
                value: CowStr::from(text.data()),
                location: text,
            });
        }

        // The three specials are ASCII, so the match is exactly one byte wide.
        let end = pos + ch.len_utf8();
        let ch_span = rest.slice(pos..end);

        out.push(match leaf {
            SpecialLeaf::CharRef => InlineNode::CharRef {
                value: CharRef::Special(ch),
                location: ch_span,
            },

            SpecialLeaf::Raw => InlineNode::Raw {
                value: CowStr::from(ch_span.data()),
                form: RawForm::AsIs,
                origin: RawOrigin::Substitution,
                location: ch_span,
            },
        });

        rest = rest.slice_from(end..);
    }

    if !rest.data().is_empty() {
        out.push(InlineNode::Text {
            value: CowStr::from(rest.data()),
            location: rest,
        });
    }
}

/// Rewrites every node an **earlier step of this same order** already turned
/// into markup as the logical [`Text`](InlineNode::Text) that markup is, so the
/// `SpecialCharacters` step that is about to run escapes those tags exactly as
/// it escapes any other text.
///
/// This is the same escaping-order rule applied to the escaping step's own
/// *position* rather than to a spliced value's classification (which
/// `split_attribute_value` already keys on the same question). Every built-in
/// group runs `specialcharacters` first, so this is a `subs=` list's question
/// alone — `subs=quotes,specialcharacters` runs the quotes step first, and the
/// tree already holds `<strong>bold</strong>` by the time the escaping step
/// reaches it, so that step emits `&lt;strong&gt;bold&lt;/strong&gt;`.
///
/// A tree has no rendered tags at that point: a
/// [`Styled`](crate::inlines::Styled) span's markup exists only at fold time.
/// So the policy is to *reach* fold time for that one node, early: the node is
/// folded through the configured renderer and the result becomes one `Text`
/// node's value. That is the same "a node's value is already-substituted text"
/// seam a delimited passthrough's and a STEM expression's body already use —
/// the only place this module consults the renderer while building — and it is
/// what the document genuinely says under such an order: the content is no
/// longer a strong span, it is text that reads like a tag, which is exactly
/// what a `Text` node the fold escapes means.
///
/// The nodes that must **not** be folded are the ones the tree is still
/// holding as a *placeholder* rather than as markup at this point, since no
/// escaping step acts on those either — see [`covers_masked`] for which they
/// are and how each is told apart, and [`masked_locations`] for the one that
/// needs an identity rather than a node kind to say so.
///
/// A node whose own subtree *contains* one of those is left alone too, and is
/// this policy's own documented divergence: folding it whole would inline the
/// placeholder's content into the escaped text, where the tree instead escapes
/// *around* the placeholder and restores it unescaped afterwards. Splitting
/// one node's fold back around its placeholder descendants would mean
/// reconstructing partial markup piecewise, which this module deliberately
/// avoids.
pub(super) fn flatten_prior_markup<'src>(
    nodes: Vec<InlineNode<'src>>,
    masked: &[(usize, usize)],
    renderer: &dyn InlineRenderer,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    nodes
        .into_iter()
        .map(|node| {
            if matches!(node, InlineNode::Text { .. }) || covers_masked(&node, masked) {
                return node;
            }

            let location = node.span();
            let value = fold_html(
                std::slice::from_ref(&as_pre_escape(node)),
                renderer,
                &parser.render_context(),
            );

            InlineNode::Text {
                value: CowStr::from(value),
                location,
            }
        })
        .collect()
}

/// Rewrites every [`Text`](InlineNode::Text) run *nested inside* `node` as a
/// [`Raw`](InlineNode::Raw) leaf, so folding `node` reproduces the markup its
/// match string holds at this point — **before** the escaping step runs —
/// rather than the fully-escaped markup a finished fold emits.
///
/// A `Text` node is logical text the fold escapes, which is what makes it the
/// right kind for the *result* of this flattening: the escaping step that is
/// about to run does that escaping, once. But the same rule applied to the
/// node's own children would escape them a second time, since those children
/// have not been escaped either at the moment the markup-producing step wrote
/// those tags (`*a < b*` folds to `<strong>a < b</strong>` at that point, and
/// the one escaping pass that follows turns both the tags and the `<` into
/// entities together). `Raw` is exactly "emit this verbatim", so the fold
/// reproduces that pre-escape haystack.
///
/// Every other leaf already carries its own already-substituted bytes — a
/// [`CharRef`](InlineNode::CharRef) an earlier `replacements` step built folds
/// to the entity that step wrote, a [`LineBreak`](InlineNode::LineBreak) to
/// the break `post_replacements` wrote — so only `Text` needs the rewrite.
fn as_pre_escape<'src>(node: InlineNode<'src>) -> InlineNode<'src> {
    match node {
        InlineNode::Text { value, location } => InlineNode::Raw {
            value,
            form: RawForm::AsIs,
            origin: RawOrigin::Substitution,
            location,
        },

        InlineNode::Styled(mut styled) => {
            styled.children = styled.children.into_iter().map(as_pre_escape).collect();
            InlineNode::Styled(styled)
        }

        InlineNode::Ref(mut reference) => {
            reference.children = reference.children.into_iter().map(as_pre_escape).collect();
            InlineNode::Ref(reference)
        }

        InlineNode::Anchor(mut anchor) => {
            anchor.reftext = anchor
                .reftext
                .map(|reftext| reftext.into_iter().map(as_pre_escape).collect());
            InlineNode::Anchor(anchor)
        }

        InlineNode::Footnote(mut footnote) => {
            footnote.children = footnote.children.into_iter().map(as_pre_escape).collect();
            InlineNode::Footnote(footnote)
        }

        other => other,
    }
}

/// The `(offset, len)` identity of every [`Styled`](crate::inlines::Styled)
/// node in `nodes` — the one *container* the passthrough-extraction pass
/// builds (an attribute-list-prefixed passthrough, `[quotes]++text++`).
///
/// A location is a sound identity for these: extraction recognizes only a
/// wholly *verbatim* match (see
/// [`range_is_verbatim`](super::macros::image::range_is_verbatim)), so each
/// carries an honest, precise `'src` span rather than a synthesized run's
/// coarse fallback span — and a later step's own node can never claim the
/// identical range,
/// since a masked node is one opaque piece to
/// [`build_match_string`](super::quotes::build_match_string) and any match
/// containing it extends past it on at least one side.
///
/// Extraction's *leaf* artifacts need no entry here, because their node kind
/// already says what they are: under an order that has a `SpecialCharacters`
/// step at all, a [`Raw`](InlineNode::Raw) leaf can only have come from this
/// pass (a spliced attribute value classifies as `Text` while that step is
/// still ahead — see
/// [`SplicedSpecials`](super::attribute_refs::SplicedSpecials) — and
/// [`classify_unescaped_specials`] runs only for an order with no such step),
/// and a [`Stem`](crate::inlines::Stem) node has no other origin at all.
pub(super) fn masked_locations(nodes: &[InlineNode<'_>]) -> Vec<(usize, usize)> {
    nodes
        .iter()
        .filter(|node| matches!(node, InlineNode::Styled(_)))
        .map(|node| identity(node))
        .collect()
}

/// What a caller can say about which [`Styled`](crate::inlines::Styled) nodes
/// are the passthrough-extraction pass's own wrappers — carried to the one
/// place *recognition* needs to tell one from a span whose markup has actually
/// been rendered.
///
/// [`masked_locations`] collects that identity once, before any step runs; this
/// is how it reaches
/// [`build_match_string`](super::quotes::build_match_string), which stands
/// every opaque node in as one placeholder and wraps it in the characters its
/// own rendering presents to a sibling. A wrapper has no such characters — it
/// is represented as its own bare `\u{96}…\u{97}` placeholder for every step
/// this module runs — so a sibling reads there exactly what the bare
/// placeholder already reads as, and the wrapper must stay unwrapped.
///
/// The third state is the point of the type. A caller that does **not** hold
/// the identity says so ([`UNKNOWN`](Self::UNKNOWN)) rather than passing an
/// empty list, which would claim that *nothing* is a wrapper; an unknown
/// identity leaves every tag-rendered span with the bare placeholder, which is
/// the answer that module gave before this reached it at all.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Masked<'a>(Option<&'a [(usize, usize)]>);

impl<'a> Masked<'a> {
    /// The identity is not in hand here, so no tag-rendered span is
    /// classified. See the type's own note for why this is not the same as an
    /// empty list.
    pub(super) const UNKNOWN: Self = Self(None);

    /// The identity of every extraction-pass wrapper in this content, as
    /// [`masked_locations`] collected it.
    pub(super) fn known(locations: &'a [(usize, usize)]) -> Self {
        Self(Some(locations))
    }

    /// Reports whether the node at `location` is one of the wrappers this
    /// list names — false whenever the identity is not in hand, since an
    /// unknown identity names nothing.
    pub(super) fn covers(self, location: Span<'_>) -> bool {
        self.0
            .is_some_and(|masked| masked.contains(&span_identity(location)))
    }

    /// Reports whether what a sibling reads beside the node at `location` is
    /// that node's own rendering — true only when the identity is in hand
    /// *and* the node is not one of the wrappers it names.
    pub(super) fn renders_to_its_siblings(self, location: Span<'_>) -> bool {
        self.0.is_some() && !self.covers(location)
    }
}

/// Reports whether `node` — or anything nested inside it — is hidden from the
/// escaping step.
///
/// Three constructs are: a passthrough body and an inline-STEM expression,
/// extracted ahead of every step and restored after the last one (told by
/// their node kind, or — for an attribute-list-prefixed passthrough's own
/// wrapper — by a `masked` entry), and a **deferred cross-reference**, which
/// the macros step records as an `XrefSegment` rather than as markup and
/// [`Content::finalize_deferred`](crate::content::Content) renders once every
/// step has run. No escaping step ever acts on any of their tags.
fn covers_masked(node: &InlineNode<'_>, masked: &[(usize, usize)]) -> bool {
    let hidden = match node {
        InlineNode::Raw { .. } | InlineNode::Stem(_) => true,
        InlineNode::Ref(reference) => reference.variant == RefVariant::Xref,
        _ => masked.contains(&identity(node)),
    };

    if hidden {
        return true;
    }

    // The containers a hidden construct can be nested inside. An
    // [`Anchor`](crate::inlines::Anchor)'s reference text is not one: it holds
    // a single verbatim `Text` child or nothing at all, since a reference text
    // crossing an opaque piece leaves the field `None` (see
    // `build_anchor_reftext`).
    match node {
        InlineNode::Styled(styled) => styled
            .children
            .iter()
            .any(|child| covers_masked(child, masked)),

        InlineNode::Ref(reference) => reference
            .children
            .iter()
            .any(|child| covers_masked(child, masked)),

        InlineNode::Footnote(footnote) => footnote
            .children
            .iter()
            .any(|child| covers_masked(child, masked)),

        _ => false,
    }
}

fn identity(node: &InlineNode<'_>) -> (usize, usize) {
    span_identity(node.span())
}

/// [`identity`] for a caller that holds the node's own `location` rather than
/// the node.
fn span_identity(location: Span<'_>) -> (usize, usize) {
    (location.byte_offset(), location.data().len())
}

/// Splits a synthesized `value` — text with no source span of its own — into
/// owned [`Text`](InlineNode::Text) runs and specials of the kind `leaf` names,
/// each carrying the whole `location` as its coarse fallback span; a run is
/// never emitted empty.
fn split_synthesized<'src>(
    value: &str,
    location: Span<'src>,
    leaf: SpecialLeaf,
    out: &mut Vec<InlineNode<'src>>,
) {
    let mut rest = value;

    // As in [`split_verbatim`], the character comes back with its offset, so
    // there is no unreachable fallback to re-derive it through.
    while let Some((pos, ch)) = find_special(rest) {
        // Emit the owned text run preceding the special, when non-empty.
        if pos > 0 {
            out.push(InlineNode::Text {
                value: CowStr::from(rest[..pos].to_string()),
                location,
            });
        }

        out.push(match leaf {
            SpecialLeaf::CharRef => InlineNode::CharRef {
                value: CharRef::Special(ch),
                location,
            },

            SpecialLeaf::Raw => InlineNode::Raw {
                value: CowStr::from(ch.to_string()),
                form: RawForm::AsIs,
                origin: RawOrigin::Substitution,
                location,
            },
        });

        // The three specials are ASCII, so each is exactly one byte wide.
        rest = &rest[pos + ch.len_utf8()..];
    }

    if !rest.is_empty() {
        out.push(InlineNode::Text {
            value: CowStr::from(rest.to_string()),
            location,
        });
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use super::{
        super::test_support::{assert_text, build_src, build_through_special, fold_html},
        apply_special_characters, classify_unescaped_specials,
    };
    use crate::{
        Span,
        content::{Content, SubstitutionStep},
        inlines::{
            Anchor, CharRef, Footnote, InlineNode, PassthroughOrigin, RawForm, RawOrigin, Ref,
            RefVariant, SpanForm, StyleVariant, Styled,
        },
        parser::HtmlInlineRenderer,
        strings::CowStr,
    };

    /// Asserts that `node` is a special [`CharRef`](InlineNode::CharRef) for
    /// `ch`, located at `col` on line 1 with `offset`.
    fn assert_special(node: &InlineNode<'_>, ch: char, col: usize, offset: usize) {
        match node {
            InlineNode::CharRef {
                value: CharRef::Special(got),
                location,
            } => {
                assert_eq!(*got, ch);
                assert_eq!(location.data(), ch.to_string());
                assert_eq!(location.col(), col, "col for {ch:?}");
                assert_eq!(location.byte_offset(), offset, "offset for {ch:?}");
            }

            other => panic!("expected CharRef::Special({ch:?}), got {other:?}"),
        }
    }

    #[test]
    fn splits_text_and_specials_with_precise_spans() {
        let nodes = build_src(Span::new("a<b>c&d"));

        assert_eq!(nodes.len(), 7);
        assert_text(&nodes[0], "a", 1, 1);
        assert_special(&nodes[1], '<', 2, 1);
        assert_text(&nodes[2], "b", 1, 3);
        assert_special(&nodes[3], '>', 4, 3);
        assert_text(&nodes[4], "c", 1, 5);
        assert_special(&nodes[5], '&', 6, 5);
        assert_text(&nodes[6], "d", 1, 7);
    }

    #[test]
    fn all_specials_yield_only_char_refs() {
        let nodes = build_src(Span::new("<>&"));

        assert_eq!(nodes.len(), 3);
        assert_special(&nodes[0], '<', 1, 0);
        assert_special(&nodes[1], '>', 2, 1);
        assert_special(&nodes[2], '&', 3, 2);
    }

    #[test]
    fn adjacent_specials_produce_no_empty_runs() {
        let nodes = build_src(Span::new("<<"));

        assert_eq!(nodes.len(), 2);
        assert_special(&nodes[0], '<', 1, 0);
        assert_special(&nodes[1], '<', 2, 1);
    }

    #[test]
    fn plain_text_is_a_single_borrowed_node() {
        let nodes = build_src(Span::new("hello"));

        assert_eq!(nodes.len(), 1);
        assert_text(&nodes[0], "hello", 1, 1);
    }

    #[test]
    fn empty_source_yields_no_nodes() {
        assert!(build_src(Span::new("")).is_empty());
    }

    #[test]
    fn a_run_spanning_a_newline_tracks_line_and_col() {
        // The text run between the two specials includes the newline, so the
        // node after it is located on line 2.
        let nodes = build_src(Span::new("a<\nb>"));

        assert_eq!(nodes.len(), 4);
        assert_text(&nodes[0], "a", 1, 1);
        assert_special(&nodes[1], '<', 2, 1);

        // The middle run is "\nb": it starts right after `<` (line 1, col 3)
        // and carries into line 2.
        assert_text(&nodes[2], "\nb", 1, 3);

        // The closing `>` lands on line 2.
        match &nodes[3] {
            InlineNode::CharRef {
                value: CharRef::Special('>'),
                location,
            } => {
                assert_eq!(location.line(), 2);
                assert_eq!(location.col(), 2);
            }

            other => panic!("expected CharRef::Special('>'), got {other:?}"),
        }
    }

    #[test]
    fn special_characters_recurses_into_styled_children() {
        // A custom `subs` order can run quotes before special characters, so
        // the step must descend into a `Styled` span's children.
        let loc = Span::new("a<b");

        let styled = InlineNode::Styled(Box::new(Styled {
            variant: StyleVariant::Strong,
            form: SpanForm::Constrained,
            id: None,
            roles: vec![],
            attrs: crate::attributes::Attrlist::empty(loc.slice(0..0)),
            children: vec![InlineNode::Text {
                value: CowStr::from(loc.data()),
                location: loc,
            }],
            passthrough: None,
            location: loc,
        }));

        let out = apply_special_characters(vec![styled]);

        assert_eq!(out.len(), 1);

        match &out[0] {
            InlineNode::Styled(styled) => {
                assert_eq!(styled.children.len(), 3);
                assert_text(&styled.children[0], "a", 1, 1);
                assert_special(&styled.children[1], '<', 2, 1);
                assert_text(&styled.children[2], "b", 1, 3);
            }

            other => panic!("expected Styled, got {other:?}"),
        }
    }

    #[test]
    fn special_characters_recurses_into_ref_children() {
        // A reference's display text is likewise refined in place.
        let loc = Span::new("x&y");

        let reference = InlineNode::Ref(Box::new(Ref {
            variant: RefVariant::Link,
            link_form: Some(crate::inlines::LinkForm::Macro),
            target: CowStr::from("https://example.com"),
            children: vec![InlineNode::Text {
                value: CowStr::from(loc.data()),
                location: loc,
            }],
            roles: vec![],
            window: None,
            resolved: None,
            derived: None,
            xrefstyle: None,
            attrs: crate::attributes::Attrlist::empty(loc.slice(0..0)),
            location: loc,
        }));

        let out = apply_special_characters(vec![reference]);

        assert_eq!(out.len(), 1);

        match &out[0] {
            InlineNode::Ref(reference) => {
                assert_eq!(reference.children.len(), 3);
                assert_text(&reference.children[0], "x", 1, 1);
                assert_special(&reference.children[1], '&', 2, 1);
                assert_text(&reference.children[2], "y", 1, 3);
            }

            other => panic!("expected Ref, got {other:?}"),
        }
    }

    #[test]
    fn special_characters_passes_other_nodes_through() {
        // A node kind the step does not split (here a line break) is forwarded
        // unchanged.
        let location = Span::new("");
        let out = apply_special_characters(vec![InlineNode::LineBreak { location }]);

        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], InlineNode::LineBreak { .. }));
    }

    #[test]
    fn special_characters_preserves_a_synthesized_text_value() {
        // A synthesized `value` (standing in for an attribute expansion) does
        // not coincide with the source its `location` covers. The step must
        // split the *logical value* — not re-derive text from the span — so the
        // expansion survives, with the whole `location` kept as each sub-node's
        // coarse fallback span.
        let location = Span::new("{x}");

        let text = InlineNode::Text {
            value: CowStr::from("a<b".to_string()),
            location,
        };

        let out = apply_special_characters(vec![text]);

        assert_eq!(out.len(), 3);

        // Leading run: the value's text, not the span's, and the coarse span.
        match &out[0] {
            InlineNode::Text { value, location } => {
                assert_eq!(value.as_ref(), "a");
                assert_eq!(location.data(), "{x}");
            }

            other => panic!("expected Text, got {other:?}"),
        }

        match &out[1] {
            InlineNode::CharRef {
                value: CharRef::Special(ch),
                location,
            } => {
                assert_eq!(*ch, '<');
                assert_eq!(location.data(), "{x}");
            }

            other => panic!("expected CharRef::Special, got {other:?}"),
        }

        // Trailing run, exercising the loop's post-special tail.
        match &out[2] {
            InlineNode::Text { value, location } => {
                assert_eq!(value.as_ref(), "b");
                assert_eq!(location.data(), "{x}");
            }

            other => panic!("expected Text, got {other:?}"),
        }
    }

    /// The frozen recording (see `parser/snapshots/README.md`) for `source`,
    /// used as the golden oracle.
    fn golden(source: &str) -> String {
        let parser = crate::Parser::default();
        let mut content = Content::from(Span::new(source));
        SubstitutionStep::SpecialCharacters.apply(&mut content, &parser, None);

        crate::content::inline_builder::snapshot::recorded("special_chars", source)
    }

    /// Asserts that `node` is a [`Raw`](InlineNode::Raw) leaf holding `ch`,
    /// located at `col` on line 1 with `offset`.
    fn assert_raw_special(node: &InlineNode<'_>, ch: char, col: usize, offset: usize) {
        match node {
            InlineNode::Raw {
                value, location, ..
            } => {
                assert_eq!(value.as_ref(), ch.to_string());
                assert_eq!(location.data(), ch.to_string());
                assert_eq!(location.col(), col, "col for {ch:?}");
                assert_eq!(location.byte_offset(), offset, "offset for {ch:?}");
            }

            other => panic!("expected Raw({ch:?}), got {other:?}"),
        }
    }

    /// A single borrowed [`Text`](InlineNode::Text) node over the whole of
    /// `source`, the seed shape `build_for_group` starts every group from.
    fn seed(source: &str) -> Vec<InlineNode<'_>> {
        let location = Span::new(source);

        vec![InlineNode::Text {
            value: CowStr::from(location.data()),
            location,
        }]
    }

    #[test]
    fn classification_splits_specials_into_raw_with_precise_spans() {
        // The `Raw` counterpart of
        // `splits_text_and_specials_with_precise_spans` above: the same
        // split, keeping the same honest per-node spans, but
        // classifying each special as the verbatim leaf an order that never
        // runs `SpecialCharacters` calls for.
        let nodes = classify_unescaped_specials(seed("a<b>c&d"));

        assert_eq!(nodes.len(), 7);
        assert_text(&nodes[0], "a", 1, 1);
        assert_raw_special(&nodes[1], '<', 2, 1);
        assert_text(&nodes[2], "b", 1, 3);
        assert_raw_special(&nodes[3], '>', 4, 3);
        assert_text(&nodes[4], "c", 1, 5);
        assert_raw_special(&nodes[5], '&', 6, 5);
        assert_text(&nodes[6], "d", 1, 7);
    }

    #[test]
    fn classification_leaves_specials_free_text_untouched() {
        // Nothing to classify: the seed passes through as the single borrowed
        // run it already was, so the common case allocates nothing new.
        let nodes = classify_unescaped_specials(seed("plain text"));

        assert_eq!(nodes.len(), 1);
        assert_text(&nodes[0], "plain text", 1, 1);
    }

    #[test]
    fn classification_preserves_a_synthesized_text_value() {
        // The synthesized (attribute-expansion) counterpart of
        // `special_characters_preserves_a_synthesized_text_value`: the split
        // follows the *logical value*, and every fragment keeps the whole
        // `location` as its coarse fallback span.
        let location = Span::new("{x}");

        let out = classify_unescaped_specials(vec![InlineNode::Text {
            value: CowStr::from("a<b".to_string()),
            location,
        }]);

        assert_eq!(out.len(), 3);

        match (&out[0], &out[1], &out[2]) {
            (
                InlineNode::Text {
                    value: leading,
                    location: leading_loc,
                },
                InlineNode::Raw {
                    value: special,
                    location: special_loc,
                    ..
                },
                InlineNode::Text {
                    value: trailing,
                    location: trailing_loc,
                },
            ) => {
                assert_eq!(leading.as_ref(), "a");
                assert_eq!(special.as_ref(), "<");
                assert_eq!(trailing.as_ref(), "b");

                for loc in [leading_loc, special_loc, trailing_loc] {
                    assert_eq!(loc.data(), "{x}");
                }
            }

            other => panic!("expected Text/Raw/Text, got {other:?}"),
        }
    }

    #[test]
    fn classification_recurses_into_every_container_the_fold_descends_into() {
        // A `subs=` order that omits `specialcharacters` can still build a
        // `Styled` span (`quotes`), a `Ref` and a `Footnote` (`macros`), and an
        // `Anchor` with a reference text, so the classification must reach the
        // text nested inside each of them — the same containers `fold_html`
        // itself descends into.
        let loc = Span::new("a<b");

        let child = || {
            vec![InlineNode::Text {
                value: CowStr::from(loc.data()),
                location: loc,
            }]
        };

        let out = classify_unescaped_specials(vec![
            InlineNode::Styled(Box::new(Styled {
                variant: StyleVariant::Strong,
                form: SpanForm::Constrained,
                id: None,
                roles: vec![],
                attrs: crate::attributes::Attrlist::empty(loc.slice(0..0)),
                children: child(),
                passthrough: None,
                location: loc,
            })),
            InlineNode::Ref(Box::new(Ref {
                variant: RefVariant::Link,
                link_form: Some(crate::inlines::LinkForm::Macro),
                target: CowStr::from("https://example.com"),
                children: child(),
                roles: vec![],
                window: None,
                resolved: None,
                derived: None,
                xrefstyle: None,
                attrs: crate::attributes::Attrlist::empty(loc.slice(0..0)),
                location: loc,
            })),
            InlineNode::Anchor(Box::new(Anchor {
                id: CowStr::from("id"),
                reftext: Some(child()),
                is_bibliography: false,
                location: loc,
            })),
            InlineNode::Footnote(Box::new(Footnote {
                id: None,
                number: Some(CowStr::from("1")),
                is_reference: false,
                children: child(),
                location: loc,
            })),
        ]);

        assert_eq!(out.len(), 4);

        let assert_classified = |children: &[InlineNode<'_>], what: &str| {
            assert_eq!(children.len(), 3, "children of {what}: {children:?}");
            assert_text(&children[0], "a", 1, 1);
            assert_raw_special(&children[1], '<', 2, 1);
            assert_text(&children[2], "b", 1, 3);
        };

        match (&out[0], &out[1], &out[2], &out[3]) {
            (
                InlineNode::Styled(styled),
                InlineNode::Ref(reference),
                InlineNode::Anchor(anchor),
                InlineNode::Footnote(footnote),
            ) => {
                assert_classified(&styled.children, "Styled");
                assert_classified(&reference.children, "Ref");
                // An absent reftext yields an empty slice, which
                // `assert_classified` rejects on its own length check — so the
                // missing case still fails loudly, with no unreachable arm.
                assert_classified(anchor.reftext.as_deref().unwrap_or(&[]), "Anchor");
                assert_classified(&footnote.children, "Footnote");
            }

            other => panic!("expected Styled/Ref/Anchor/Footnote, got {other:?}"),
        }
    }

    #[test]
    fn classification_passes_other_nodes_through() {
        // A node kind carrying no text of its own (here a line break) is
        // forwarded unchanged, and an already-`Raw` passthrough leaf is never
        // re-split.
        let location = Span::new("<raw>");

        let out = classify_unescaped_specials(vec![
            InlineNode::LineBreak { location },
            InlineNode::Raw {
                value: CowStr::from(location.data()),
                form: RawForm::AsIs,
                origin: RawOrigin::Passthrough(Box::new(PassthroughOrigin {
                    subs: crate::content::SubstitutionGroup::None,
                    source_text: None,
                })),
                location,
            },
        ]);

        assert_eq!(out.len(), 2);
        assert!(matches!(out[0], InlineNode::LineBreak { .. }));
        assert!(
            matches!(&out[1], InlineNode::Raw { value, .. } if value.as_ref() == "<raw>"),
            "an existing Raw leaf must pass through whole: {out:?}"
        );
    }

    #[test]
    fn fold_matches_the_string_pipeline_byte_for_byte() {
        // Special-characters-only fixtures: for these, folding the single-pass
        // tree reproduces the frozen recording's escaped output exactly.
        let fixtures = [
            "",
            "plain text",
            "a<b>c&d",
            "<>&",
            "<<",
            "&<>&",
            "less < and & more >",
            "trailing &",
            "multi\nline < with & specials >",
            "unicode π < ω &",
        ];

        let renderer = HtmlInlineRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_through_special(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden(fixture),
                "fold diverged from golden for {fixture:?}"
            );
        }
    }
}
