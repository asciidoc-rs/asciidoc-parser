//! The post-replacements substitution step (hard line breaks).

use super::quotes::{Piece, build_match_string, emit_range, source_slice};
use crate::{
    Parser, Span, attributes::Attrlist, content::hard_line_break_pattern, inlines::InlineNode,
};

/// The post-replacement substitution, as a node transducer: a line ending in
/// ` +` has that ` +` replaced by a [`LineBreak`](InlineNode::LineBreak) leaf,
/// the line content before it staying in place.
///
/// Under the block-wide `hardbreaks` option – set on `attrlist` (the block's
/// own attribute list) or via the document's `hardbreaks-option` attribute –
/// [`apply_hardbreaks`] runs instead: *every* line ending becomes a break, not
/// only one already marked with ` +`.
pub(super) fn apply_post_replacements<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
    attrlist: Option<&Attrlist<'src>>,
) -> Vec<InlineNode<'src>> {
    // Descend into spans/refs first, matching the string pipeline's
    // whole-string pass.
    let nodes: Vec<InlineNode<'src>> = nodes
        .into_iter()
        .map(|node| match node {
            InlineNode::Styled(mut styled) => {
                styled.children = apply_post_replacements(styled.children, root, parser, attrlist);
                InlineNode::Styled(styled)
            }

            InlineNode::Ref(mut reference) => {
                reference.children =
                    apply_post_replacements(reference.children, root, parser, attrlist);
                InlineNode::Ref(reference)
            }

            other => other,
        })
        .collect();

    if parser.is_attribute_set("hardbreaks-option")
        || attrlist.is_some_and(|attrlist| attrlist.has_option("hardbreaks"))
    {
        return apply_hardbreaks(nodes, root);
    }

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

    emit_breaks(&nodes, &pieces, &s, &breaks, root)
}

/// The `hardbreaks` form of the post-replacement substitution: every line
/// ending (`\n`) in the level's match string becomes a break, mirroring the
/// string pipeline's own `line.ends_with(" +")`-stripping, line-by-line
/// rejoin exactly – a trailing ` +` is stripped rather than kept *and*
/// doubled, and the level's *last* line (nothing follows its own `\n`, since
/// there is none) never gets one, matching the string pipeline leaving the
/// popped last line unbroken.
fn apply_hardbreaks<'src>(nodes: Vec<InlineNode<'src>>, root: Span<'src>) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes);

    if !s.contains('\n') {
        return nodes;
    }

    let breaks: Vec<std::ops::Range<usize>> = s
        .match_indices('\n')
        .map(|(nl, _)| {
            if s.get(nl.saturating_sub(2)..nl) == Some(" +") {
                nl - 2..nl
            } else {
                nl..nl
            }
        })
        .collect();

    emit_breaks(&nodes, &pieces, &s, &breaks, root)
}

/// Shared tail of both post-replacement forms: replaces each `breaks` range
/// (already ordered, non-overlapping, over the level's match string `s`) with
/// a [`LineBreak`](InlineNode::LineBreak) leaf, keeping everything between and
/// around them.
fn emit_breaks<'src>(
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    s: &str,
    breaks: &[std::ops::Range<usize>],
    root: Span<'src>,
) -> Vec<InlineNode<'src>> {
    let mut out = Vec::new();
    let mut cursor = 0usize;

    for br in breaks {
        emit_range(nodes, pieces, cursor..br.start, &mut out);

        out.push(InlineNode::LineBreak {
            location: source_slice(pieces, br.clone(), root),
        });

        cursor = br.end;
    }

    if cursor < s.len() {
        emit_range(nodes, pieces, cursor..s.len(), &mut out);
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
        Parser, Span,
        attributes::{Attrlist, AttrlistContext},
        content::{Content, SubstitutionGroup},
        inlines::{InlineNode, Ref, RefVariant},
        parser::{HtmlSubstitutionRenderer, ModificationContext},
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
            xrefstyle: None,
            attrs: None,
            location: loc,
        });

        let out = apply_post_replacements(vec![reference], loc, &Parser::default(), None);

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

    /// Runs `source` through the real, public `SubstitutionGroup::Normal`
    /// pipeline (the golden oracle), and through [`super::super::build`] +
    /// [`fold_html`] (the candidate), under a document configured by
    /// `parser` and a block `attrlist` parsed from `attrlist_src`.
    fn assert_hardbreaks_parity(source: &str, attrlist_src: &str, parser: &Parser) {
        let attrlist = if attrlist_src.is_empty() {
            None
        } else {
            Some(
                Attrlist::parse(Span::new(attrlist_src), parser, AttrlistContext::Block)
                    .item
                    .item,
            )
        };

        let mut content = Content::from(Span::new(source));
        SubstitutionGroup::Normal.apply(&mut content, parser, attrlist.as_ref());
        let golden = content.rendered_str().to_string();

        let nodes = super::super::build(Span::new(source), parser, attrlist.as_ref());
        let built = fold_html(&nodes, &HtmlSubstitutionRenderer {});

        assert_eq!(golden, built, "hardbreaks fold diverged for {source:?}");
    }

    #[test]
    fn hardbreaks_option_on_the_block_breaks_every_line() {
        assert_hardbreaks_parity(
            "line one\nline two\nline three",
            "%hardbreaks",
            &Parser::default(),
        );
    }

    #[test]
    fn hardbreaks_option_strips_a_redundant_trailing_plus() {
        // A line already ending in ` +` is not double-broken.
        assert_hardbreaks_parity("line one +\nline two", "%hardbreaks", &Parser::default());
    }

    #[test]
    fn hardbreaks_option_on_a_single_line_breaks_nothing() {
        assert_hardbreaks_parity("just one line", "%hardbreaks", &Parser::default());
    }

    #[test]
    fn hardbreaks_option_document_attribute_breaks_every_line() {
        let parser = Parser::default().with_intrinsic_attribute_bool(
            "hardbreaks-option",
            true,
            ModificationContext::Anywhere,
        );

        assert_hardbreaks_parity("line one\nline two", "", &parser);
    }

    #[test]
    fn hardbreaks_recurses_into_a_quoted_span() {
        assert_hardbreaks_parity(
            "*line one\nline two*\nline three",
            "%hardbreaks",
            &Parser::default(),
        );
    }
}
