//! The post-replacements substitution step (hard line breaks).

use super::{
    quotes::{Piece, build_match_string, emit_range, source_slice},
    special_chars::Masked,
};
use crate::{
    Parser, Span, attributes::Attrlist, content::hard_line_break_pattern, inlines::InlineNode,
};

/// The post-replacement substitution, as a node transducer: a line ending in
/// ` +` has that ` +` replaced by a [`LineBreak`](InlineNode::LineBreak) leaf,
/// the line content before it staying in place.
///
/// Under the block-wide `hardbreaks` option — set on `attrlist` (the block's
/// own attribute list) or via the document's `hardbreaks-option` attribute —
/// [`apply_hardbreaks`] runs instead: *every* line ending becomes a break, not
/// only one already marked with ` +`.
pub(super) fn apply_post_replacements<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
    attrlist: Option<&Attrlist<'src>>,
) -> Vec<InlineNode<'src>> {
    apply_level(nodes, root, parser, attrlist, true)
}

/// One level of the post-replacement pass. `is_root` marks the content's own
/// top level, which is the only level whose end coincides with the end of the
/// string pipeline's haystack — see the match filter below for why that
/// decides whether a ` +` ending the level is a break.
fn apply_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
    attrlist: Option<&Attrlist<'src>>,
    is_root: bool,
) -> Vec<InlineNode<'src>> {
    // Descend into spans/refs first, matching the string pipeline's
    // whole-string pass. A nested level is never the root.
    let nodes: Vec<InlineNode<'src>> = nodes
        .into_iter()
        .map(|node| match node {
            InlineNode::Styled(mut styled) => {
                styled.children = apply_level(styled.children, root, parser, attrlist, false);
                InlineNode::Styled(styled)
            }

            InlineNode::Ref(mut reference) => {
                reference.children = apply_level(reference.children, root, parser, attrlist, false);
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

    let (s, pieces) = build_match_string(&nodes, Masked::UNKNOWN);

    // A break needs a `+`, and — off the root level, where an end-of-level
    // match is discarded below — a `\n` for the pattern's `$` to anchor
    // against. The string pipeline's own pre-check is the root form of this: it
    // guards on a `+` alone, since its haystack is the whole rendered string.
    if !(s.contains('+') && (is_root || s.contains('\n'))) {
        return nodes;
    }

    // Each match's `[content.end, whole.end)` is the trailing ` +` the break
    // replaces; the line content before it is kept. Group 0 (the whole match)
    // and group 1 (the `(.*)` line content) always participate in this pattern.
    //
    // The pattern's `$` (multiline) matches before each `\n` *and* at the end
    // of the haystack. The string pipeline's haystack is the whole rendered
    // string, so its end-of-haystack match can only fire at the very end of the
    // content; this transducer matches over one level at a time, where end of
    // level and end of haystack coincide only at the root. A nested `Styled` or
    // `Ref` level is always followed by its own closing markup (`</strong>`,
    // `</a>`, …) in the string pipeline's haystack, so a ` +` ending a span is
    // not at a line end there and stays literal: `*bold +*` renders
    // `<strong>bold +</strong>`, not `<strong>bold<br></strong>`. Dropping the
    // end-of-level match off the root reproduces that, while a ` +` before a
    // `\n` is unaffected at every level.
    let breaks: Vec<std::ops::Range<usize>> = hard_line_break_pattern()
        .captures_iter(&s)
        .filter_map(|caps| {
            #[allow(clippy::unwrap_used)]
            let whole = caps.get(0).unwrap();

            if !is_root && whole.end() == s.len() {
                return None;
            }

            #[allow(clippy::unwrap_used)]
            let content = caps.get(1).unwrap();

            Some(content.end()..whole.end())
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
/// rejoin exactly — a trailing ` +` is stripped rather than kept *and*
/// doubled, and the level's *last* line (nothing follows its own `\n`, since
/// there is none) never gets one, matching the string pipeline leaving the
/// popped last line unbroken.
fn apply_hardbreaks<'src>(nodes: Vec<InlineNode<'src>>, root: Span<'src>) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes, Masked::UNKNOWN);

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
    fn a_trailing_hard_line_break_with_no_newline_after_it_still_breaks() {
        // The hard-line-break pattern anchors on `$` in multiline mode, which
        // matches at the end of the haystack as well as before each `\n`, so a
        // ` +` that *ends* the content is a break too — there is no newline
        // anywhere in this source (issue #1067).
        let nodes = build_src(Span::new("only +"));

        assert_eq!(nodes.len(), 2);
        assert_text(&nodes[0], "only", 1, 1);

        match &nodes[1] {
            InlineNode::LineBreak { location } => {
                assert_eq!(location.data(), " +");
            }

            other => panic!("expected LineBreak, got {other:?}"),
        }

        assert_eq!(fold_html(&nodes, &HtmlSubstitutionRenderer {}), "only<br>");
    }

    #[test]
    fn the_last_line_of_a_multi_line_level_breaks_too() {
        // The same `$` anchor makes the *final* line eligible even when earlier
        // lines are not: only the trailing ` +` here becomes a break.
        let nodes = build_src(Span::new("first\nsecond +"));

        assert_eq!(nodes.len(), 2);
        assert_text(&nodes[0], "first\nsecond", 1, 1);
        assert!(matches!(nodes[1], InlineNode::LineBreak { .. }));

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            "first\nsecond<br>"
        );
    }

    /// Runs `source` through the string pipeline (the golden oracle) and
    /// through [`super::super::build`] + [`fold_html`], asserting byte
    /// parity.
    fn assert_parity(source: &str) {
        let parser = Parser::default();

        let mut content = Content::from(Span::new(source));
        SubstitutionGroup::Normal.apply(&mut content, &parser, None);
        let golden = content.rendered_str().to_string();

        let nodes = super::super::build(Span::new(source), &parser, None);
        let built = fold_html(&nodes, &HtmlSubstitutionRenderer {});

        assert_eq!(golden, built, "fold diverged for {source:?}");
    }

    #[test]
    fn a_hard_line_break_ending_a_span_stays_literal() {
        // The pattern's `$` reaches the end of a *level*, but in the string
        // pipeline's whole-string haystack a span's content is followed by its
        // own closing tag, so a ` +` there is not at a line end and never
        // becomes a break. Only the content's root level ends where the
        // haystack ends.
        let nodes = build_src(Span::new("*bold +*"));

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            "<strong>bold +</strong>"
        );

        for source in [
            // Every nesting the recursion descends into: a styled span, a
            // reference's display text, and one inside the other.
            "*bold +*",
            "_em +_",
            "`code +`",
            "link:https://example.org[text +]",
            "*_deep +_*",
            "text *bold +* tail",
            "*bold +*\ntail",
            // A span that ends in ` +` *and* contains a real break: the interior
            // one fires, the one ending the span does not. This case diverged
            // even before the end-of-level match was allowed, because the
            // level's own `\n` was enough to get past the cheap pre-check.
            "*bold +\nmore +*",
            // The root level still breaks at its end, inside or outside a span.
            "*a +\nb* +",
            "*x +* y +",
            "foo +\n*bar +*",
        ] {
            assert_parity(source);
        }
    }

    #[test]
    fn post_replacements_recurse_into_ref_children() {
        // A hard line break inside a reference's display text is likewise
        // recognized (driven directly, as above).
        let loc = Span::new("x +\ny");

        let reference = InlineNode::Ref(Ref {
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
