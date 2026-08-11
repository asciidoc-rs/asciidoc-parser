//! The special-characters substitution step.

use super::passthrough_step::is_special;
use crate::{
    Span,
    inlines::{CharRef, InlineNode},
    strings::CowStr,
};

/// The special-characters substitution, as a node transducer: every
/// [`Text`](InlineNode::Text) run is split on `<`/`>`/`&` into
/// [`Text`](InlineNode::Text) and [`CharRef`](InlineNode::CharRef) nodes, and
/// every other node passes through (recursing into parent nodes' children).
///
/// The split is driven by the node's **logical `value`**, not by its source
/// span, so a *synthesized* value – an attribute expansion or a joined
/// multi-line run that a later step may feed in under a custom `subs` order –
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
                split_text(value, location, &mut out);
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

/// Splits a [`Text`](InlineNode::Text) node's logical `value` into alternating
/// text runs and `<`/`>`/`&` [`CharRef`](InlineNode::CharRef) specials.
///
/// When `value` is exactly the source its `location` covers – the common
/// verbatim run – each sub-node is sliced from `location`, so its
/// `line`/`col`/`offset` stay honest (issue #944) and its run text borrows from
/// `'src`. When `value` is *synthesized* – it has no source of its own – the
/// runs are owned slices of the value and every sub-node falls back to the
/// whole `location` span, the documented coarse fallback (design §4.4).
fn split_text<'src>(value: CowStr<'src>, location: Span<'src>, out: &mut Vec<InlineNode<'src>>) {
    if value.as_ref() == location.data() {
        split_verbatim(location, out);
    } else {
        split_synthesized(value.as_ref(), location, out);
    }
}

/// Splits a verbatim run – text that coincides with the source `location`
/// covers – slicing each sub-span from `location` with the crate's span
/// primitives so `line`/`col`/`offset` stay honest; a run is never emitted
/// empty.
fn split_verbatim<'src>(location: Span<'src>, out: &mut Vec<InlineNode<'src>>) {
    let mut rest = location;

    while let Some(pos) = rest.position(is_special) {
        // Emit the borrowed text run preceding the special, when non-empty.
        if pos > 0 {
            let text = rest.slice_to(..pos);

            out.push(InlineNode::Text {
                value: CowStr::from(text.data()),
                location: text,
            });
        }

        // The three specials are ASCII, so the match is exactly one byte wide.
        let ch_span = rest.slice(pos..pos + 1);

        let ch = ch_span.data().chars().next().unwrap_or('\u{FFFD}');

        out.push(InlineNode::CharRef {
            value: CharRef::Special(ch),
            location: ch_span,
        });

        rest = rest.slice_from(pos + 1..);
    }

    if !rest.data().is_empty() {
        out.push(InlineNode::Text {
            value: CowStr::from(rest.data()),
            location: rest,
        });
    }
}

/// Splits a synthesized `value` – text with no source span of its own – into
/// owned [`Text`](InlineNode::Text) runs and [`CharRef`](InlineNode::CharRef)
/// specials, each carrying the whole `location` as its coarse fallback span; a
/// run is never emitted empty.
fn split_synthesized<'src>(value: &str, location: Span<'src>, out: &mut Vec<InlineNode<'src>>) {
    let mut rest = value;

    while let Some(pos) = rest.find(is_special) {
        // Emit the owned text run preceding the special, when non-empty.
        if pos > 0 {
            out.push(InlineNode::Text {
                value: CowStr::from(rest[..pos].to_string()),
                location,
            });
        }

        // The three specials are ASCII, so the match is exactly one byte wide.
        let ch = rest[pos..].chars().next().unwrap_or('\u{FFFD}');

        out.push(InlineNode::CharRef {
            value: CharRef::Special(ch),
            location,
        });

        rest = &rest[pos + 1..];
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
        apply_special_characters,
    };
    use crate::{
        Span,
        content::{Content, SubstitutionStep},
        inlines::{CharRef, InlineNode, Ref, RefVariant, SpanForm, StyleVariant, Styled},
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
        // split the *logical value* – not re-derive text from the span – so the
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
