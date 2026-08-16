//! The special-characters substitution step, and its §3.4.1 counterpart for an
//! effective order that never runs it.

use super::passthrough_step::is_special;
use crate::{
    Span,
    inlines::{CharRef, InlineNode},
    strings::CowStr,
};

/// Which leaf kind a literal `<`/`>`/`&` becomes when a text run is split.
///
/// Design §3.4.1: the kind a fragment becomes is **not** a fixed property of
/// where it came from; it is decided by which substitution steps still act on
/// it under the group's effective order.
#[derive(Clone, Copy)]
enum SpecialLeaf {
    /// A [`CharRef::Special`] the fold escapes — what the `SpecialCharacters`
    /// step itself produces when it acts on a run.
    CharRef,

    /// A [`Raw`](InlineNode::Raw) leaf the fold emits verbatim — what a
    /// literal special is under an effective order that never runs
    /// `SpecialCharacters`, since the string pipeline leaves it untouched.
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
/// [`Raw`](InlineNode::Raw) leaf — the §3.4.1 policy for an effective
/// substitution order whose steps **never include**
/// [`SpecialCharacters`](crate::content::SubstitutionStep::SpecialCharacters).
///
/// A [`Text`](InlineNode::Text) node is *logical* text the fold escapes (§3.4),
/// which is exactly right when the `SpecialCharacters` step acted on the
/// content — and exactly wrong when it never ran, because there the string
/// pipeline emits the author's `<` unescaped. `subs=quotes` on a paragraph, a
/// passthrough block ([`Pass`](crate::content::SubstitutionGroup::Pass)), a
/// comment block ([`None`](crate::content::SubstitutionGroup::None)), and
/// `subs=callouts` on a listing block all take that path, so the classification
/// has to follow the *order*, not the node's origin.
///
/// This runs **after** every one of the group's own steps rather than in place
/// of `apply_special_characters`, and that ordering is what keeps it faithful:
/// under such an order the string pipeline's own steps also match over text in
/// which the specials are still literal, so every transducer must see them as
/// ordinary [`Text`](InlineNode::Text) characters — not as the opaque leaf a
/// `Raw` node is to [`build_match_string`](super::quotes::build_match_string).
/// Only the finished tree's *classification* differs, so nothing about
/// recognition changes.
///
/// It recurses into every container a text run can be nested inside — a
/// [`Styled`](crate::inlines::Styled) span, a [`Ref`](crate::inlines::Ref)'s
/// own display children, an [`Anchor`](crate::inlines::Anchor)'s reference
/// text, and a [`Footnote`](crate::inlines::Footnote)'s own children —
/// mirroring the containers [`fold_html`](super::fold_html) itself descends
/// into.
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

            other => out.push(other),
        }
    }

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
/// whole `location` span, the documented coarse fallback (design §4.4).
///
/// An **empty** value is kept as the node it already is rather than split.
/// Neither splitter ever emits an empty run (there is nothing in one to
/// escape), so splitting an empty node would silently delete it — and an empty
/// `Text` can be load-bearing: a `<<id,>>` cross-reference's present-but-empty
/// reference text is exactly one, and the fold tells it from an absent text by
/// the child's *presence* (see `build_xref_shorthand_node` in
/// [`macros`](super::macros)).
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
    while let Some((pos, ch)) = rest.data().char_indices().find(|(_, ch)| is_special(*ch)) {
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
    while let Some((pos, ch)) = rest.char_indices().find(|(_, ch)| is_special(*ch)) {
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
            Anchor, CharRef, Footnote, InlineNode, Ref, RefVariant, SpanForm, StyleVariant, Styled,
        },
        parser::HtmlSubstitutionRenderer,
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

        // The middle run is "\nb": it starts right after `<` (line 1, col 3) and
        // carries into line 2.
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
        // A custom `subs` order can run quotes before special characters, so the
        // step must descend into a `Styled` span's children.
        let loc = Span::new("a<b");

        let styled = InlineNode::Styled(Styled {
            variant: StyleVariant::Strong,
            form: SpanForm::Constrained,
            id: None,
            roles: vec![],
            attrs: None,
            children: vec![InlineNode::Text {
                value: CowStr::from(loc.data()),
                location: loc,
            }],
            location: loc,
        });

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

        let reference = InlineNode::Ref(Ref {
            variant: RefVariant::Link,
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
            attrs: None,
            location: loc,
        });

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

    /// The string pipeline's special-characters output for `source`, used as
    /// the golden oracle: `Content::from` then the `SpecialCharacters`
    /// step.
    fn golden(source: &str) -> String {
        let parser = crate::Parser::default();
        let mut content = Content::from(Span::new(source));
        SubstitutionStep::SpecialCharacters.apply(&mut content, &parser, None);
        content.rendered_str().to_string()
    }

    /// Asserts that `node` is a [`Raw`](InlineNode::Raw) leaf holding `ch`,
    /// located at `col` on line 1 with `offset`.
    fn assert_raw_special(node: &InlineNode<'_>, ch: char, col: usize, offset: usize) {
        match node {
            InlineNode::Raw { value, location } => {
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
        // The `Raw` counterpart of `splits_text_and_specials_with_precise_spans`
        // above: the same split, keeping the same honest per-node spans, but
        // classifying each special as the verbatim leaf an order that never
        // runs `SpecialCharacters` calls for (design §3.4.1).
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
        // `location` as its coarse fallback span (design §4.4).
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
            InlineNode::Styled(Styled {
                variant: StyleVariant::Strong,
                form: SpanForm::Constrained,
                id: None,
                roles: vec![],
                attrs: None,
                children: child(),
                location: loc,
            }),
            InlineNode::Ref(Ref {
                variant: RefVariant::Link,
                target: CowStr::from("https://example.com"),
                children: child(),
                roles: vec![],
                window: None,
                resolved: None,
                derived: None,
                xrefstyle: None,
                attrs: None,
                location: loc,
            }),
            InlineNode::Anchor(Anchor {
                id: CowStr::from("id"),
                reftext: Some(child()),
                is_bibliography: false,
                location: loc,
            }),
            InlineNode::Footnote(Footnote {
                id: None,
                number: Some(CowStr::from("1")),
                is_reference: false,
                children: child(),
                location: loc,
            }),
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
        // tree reproduces the string pipeline's escaped output exactly.
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

        let renderer = HtmlSubstitutionRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_through_special(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden(fixture),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }
}
