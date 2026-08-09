//! The post-replacements substitution step (hard line breaks).

use super::quotes::{build_match_string, emit_range, source_slice};
use crate::{Span, content::hard_line_break_pattern, inlines::InlineNode};

/// The post-replacement substitution, as a node transducer: a line ending in
/// ` +` has that ` +` replaced by a [`LineBreak`](InlineNode::LineBreak) leaf,
/// the line content before it staying in place.
///
/// Only the default hard-line-break form is handled here. The block-wide
/// `hardbreaks` option – which turns *every* line ending into a break – needs
/// the block's attribute list and the document's `hardbreaks-option` attribute,
/// neither yet threaded into the builder, so it is deferred to the cutover
/// (design §5.2 Phase 4, step 6).
pub(super) fn apply_post_replacements<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
) -> Vec<InlineNode<'src>> {
    // Descend into spans/refs first, matching the string pipeline's
    // whole-string pass.
    let nodes: Vec<InlineNode<'src>> = nodes
        .into_iter()
        .map(|node| match node {
            InlineNode::Styled(mut styled) => {
                styled.children = apply_post_replacements(styled.children, root);
                InlineNode::Styled(styled)
            }

            InlineNode::Ref(mut reference) => {
                reference.children = apply_post_replacements(reference.children, root);
                InlineNode::Ref(reference)
            }

            other => other,
        })
        .collect();

    let (s, pieces) = build_match_string(&nodes);

    // The string pipeline guards on both a `+` and a newline being present.
    if !(s.contains('+') && s.contains('\n')) {
        return nodes;
    }

    // Each match's `[content.end, whole.end)` is the trailing ` +` the break
    // replaces; the line content before it is kept. Group 0 (the whole match)
    // and group 1 (the `(.*)` line content) always participate in this pattern.
    let breaks: Vec<std::ops::Range<usize>> = hard_line_break_pattern()
        .captures_iter(&s)
        .map(|caps| {
            #[allow(clippy::unwrap_used)]
            let whole = caps.get(0).unwrap();

            #[allow(clippy::unwrap_used)]
            let content = caps.get(1).unwrap();

            content.end()..whole.end()
        })
        .collect();

    if breaks.is_empty() {
        return nodes;
    }

    let mut out = Vec::new();
    let mut cursor = 0usize;

    for br in breaks {
        emit_range(&nodes, &pieces, cursor..br.start, &mut out);

        out.push(InlineNode::LineBreak {
            location: source_slice(&pieces, br.clone(), root),
        });

        cursor = br.end;
    }

    if cursor < s.len() {
        emit_range(&nodes, &pieces, cursor..s.len(), &mut out);
    }

    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use super::{
        super::test_support::{assert_text, build_src, fold_html},
        apply_post_replacements,
    };
    use crate::{
        Span,
        inlines::{InlineNode, Ref, RefVariant},
        parser::HtmlSubstitutionRenderer,
        strings::CowStr,
    };

    #[test]
    fn a_hard_line_break_becomes_a_line_break_leaf() {
        // A line ending in ` +` yields a `LineBreak` leaf in place of the ` +`;
        // the newline and following line stay as text.
        let nodes = build_src(Span::new("foo +\nbar"));

        assert_eq!(nodes.len(), 3);
        assert_text(&nodes[0], "foo", 1, 1);

        match &nodes[1] {
            InlineNode::LineBreak { location } => {
                assert_eq!(location.data(), " +");
            }

            other => panic!("expected LineBreak, got {other:?}"),
        }

        assert_text(&nodes[2], "\nbar", 1, 6);

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            "foo<br>\nbar"
        );
    }

    #[test]
    fn post_replacements_recurse_into_ref_children() {
        // A hard line break inside a reference's display text is likewise
        // recognized (driven directly, as above).
        let loc = Span::new("x +\ny");

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
            location: loc,
        });

        let out = apply_post_replacements(vec![reference], loc);

        match &out[0] {
            InlineNode::Ref(reference) => {
                assert_eq!(reference.children.len(), 3);
                assert_text(&reference.children[0], "x", 1, 1);
                assert!(matches!(
                    reference.children[1],
                    InlineNode::LineBreak { .. }
                ));
                assert_text(&reference.children[2], "\ny", 1, 4);
            }

            other => panic!("expected Ref, got {other:?}"),
        }
    }
}
