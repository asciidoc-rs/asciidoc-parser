use crate::{
    HasSpan, Parser, Span,
    attributes::Attrlist,
    blocks::{
        CompoundDelimitedBlock, ContentModel, IsBlock, ListItemMarker, RawDelimitedBlock,
        metadata::BlockMetadata,
    },
    content::{Content, SubstitutionGroup},
    span::MatchedItem,
    strings::CowStr,
};

/// The style of a simple block.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum SimpleBlockStyle {
    /// A paragraph block with normal substitutions.
    Paragraph,

    /// A literal block with no substitutions.
    Literal,

    /// Blocks and paragraphs assigned the listing style display their rendered
    /// content exactly as you see it in the source. Listing content is
    /// converted to preformatted text (i.e., `<pre>`). The content is presented
    /// in a fixed-width font and endlines are preserved. Only [special
    /// characters] and callouts are replaced when the document is converted.
    ///
    /// [special characters]: https://docs.asciidoctor.org/asciidoc/latest/subs/special-characters/
    Listing,

    /// A source block is a specialization of a listing block. Developers are
    /// accustomed to seeing source code colorized to emphasize the code’s
    /// structure (i.e., keywords, types, delimiters, etc.).
    Source,
}

impl std::fmt::Debug for SimpleBlockStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimpleBlockStyle::Paragraph => write!(f, "SimpleBlockStyle::Paragraph"),
            SimpleBlockStyle::Literal => write!(f, "SimpleBlockStyle::Literal"),
            SimpleBlockStyle::Listing => write!(f, "SimpleBlockStyle::Listing"),
            SimpleBlockStyle::Source => write!(f, "SimpleBlockStyle::Source"),
        }
    }
}

/// A block that's treated as contiguous lines of paragraph text (and subject to
/// normal substitutions) (e.g., a paragraph block).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimpleBlock<'src> {
    content: Content<'src>,
    source: Span<'src>,
    style: SimpleBlockStyle,
    title_source: Option<Span<'src>>,
    title: Option<String>,
    anchor: Option<Span<'src>>,
    anchor_reftext: Option<Span<'src>>,
    attrlist: Option<Attrlist<'src>>,
}

impl<'src> SimpleBlock<'src> {
    pub(crate) fn parse(
        metadata: &BlockMetadata<'src>,
        parser: &mut Parser,
    ) -> Option<MatchedItem<'src, Self>> {
        let MatchedItem {
            item: (content, style),
            after,
        } = parse_lines(
            metadata.block_start,
            &metadata.attrlist,
            false,
            false,
            false,
            parser,
            &[],
        )?;

        Some(MatchedItem {
            item: Self {
                content,
                source: metadata
                    .source
                    .trim_remainder(after)
                    .trim_trailing_whitespace(),
                style,
                title_source: metadata.title_source,
                title: metadata.title.clone(),
                anchor: metadata.anchor,
                anchor_reftext: metadata.anchor_reftext,
                attrlist: metadata.attrlist.clone(),
            },
            after: after.discard_empty_lines(),
        })
    }

    pub(crate) fn parse_for_list_item(
        metadata: &BlockMetadata<'src>,
        parser: &mut Parser,
        is_continuation: bool,
        parent_list_markers: &[ListItemMarker<'src>],
    ) -> Option<MatchedItem<'src, Self>> {
        let MatchedItem {
            item: (content, style),
            after,
        } = parse_lines(
            metadata.block_start,
            &metadata.attrlist,
            true,
            false,
            is_continuation,
            parser,
            parent_list_markers,
        )?;

        Some(MatchedItem {
            item: Self {
                content,
                source: metadata
                    .source
                    .trim_remainder(after)
                    .trim_trailing_whitespace(),
                style,
                title_source: metadata.title_source,
                title: metadata.title.clone(),
                anchor: metadata.anchor,
                anchor_reftext: metadata.anchor_reftext,
                attrlist: metadata.attrlist.clone(),
            },
            after,
        })
    }

    /// Parse a simple block for use in a definition list item.
    ///
    /// In definition lists, indented content is treated as a paragraph
    /// with the indentation stripped, not as a literal block.
    pub(crate) fn parse_for_definition_list(
        metadata: &BlockMetadata<'src>,
        parser: &mut Parser,
    ) -> Option<MatchedItem<'src, Self>> {
        let MatchedItem {
            item: (content, style),
            after,
        } = parse_lines(
            metadata.block_start,
            &metadata.attrlist,
            true,
            true,
            false,
            parser,
            &[],
        )?;

        Some(MatchedItem {
            item: Self {
                content,
                source: metadata
                    .source
                    .trim_remainder(after)
                    .trim_trailing_whitespace(),
                style,
                title_source: metadata.title_source,
                title: metadata.title.clone(),
                anchor: metadata.anchor,
                anchor_reftext: metadata.anchor_reftext,
                attrlist: metadata.attrlist.clone(),
            },
            after,
        })
    }

    pub(crate) fn parse_fast(
        source: Span<'src>,
        parser: &Parser,
    ) -> Option<MatchedItem<'src, Self>> {
        let MatchedItem {
            item: (content, style),
            after,
        } = parse_lines(source, &None, false, false, false, parser, &[])?;

        let source = content.original();

        Some(MatchedItem {
            item: Self {
                content,
                source,
                style,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },
            after: after.discard_empty_lines(),
        })
    }

    /// Return the interpreted content of this block.
    pub fn content(&self) -> &Content<'src> {
        &self.content
    }

    /// Return the style of this block.
    pub fn style(&self) -> SimpleBlockStyle {
        self.style
    }
}

/// Parse the content-bearing lines for this block.
///
/// If `force_paragraph_style` is true, indented content is treated as a
/// paragraph (with indentation stripped) rather than as a literal block. This
/// is used for definition list items where indentation is purely visual
/// formatting.
///
/// If `preserve_literal_indent` is true and the content is a literal block,
/// indentation is preserved as-is (used for `+` continuation content).
fn parse_lines<'src>(
    source: Span<'src>,
    attrlist: &Option<Attrlist<'src>>,
    mut stop_for_list_items: bool,
    force_paragraph_style: bool,
    preserve_literal_indent: bool,
    parser: &Parser,
    parent_list_markers: &[ListItemMarker<'src>],
) -> Option<MatchedItem<'src, (Content<'src>, SimpleBlockStyle)>> {
    let source_after_whitespace = source.discard_whitespace();
    let first_line_indent = source_after_whitespace.col() - 1;

    // Track if we're in "indented literal" mode (literal style from indentation).
    // In this mode, we should still stop for list markers that are NOT indented.
    let mut indented_literal_mode = false;

    let mut style = if source_after_whitespace.col() == source.col() || force_paragraph_style {
        // When force_paragraph_style is true, we still need to track that the content
        // is indented so we can properly stop at unindented list markers.
        if source_after_whitespace.col() != source.col() {
            indented_literal_mode = true;
        }
        SimpleBlockStyle::Paragraph
    } else {
        // Indented content treated as literal: don't stop for list markers
        // (they become part of the literal content).
        stop_for_list_items = false;
        SimpleBlockStyle::Literal
    };

    // Block style can override the interpretation of literal from reading
    // indentation.
    if let Some(attrlist) = attrlist {
        match attrlist.block_style() {
            Some("normal") => {
                style = SimpleBlockStyle::Paragraph;
            }

            Some("literal") => {
                stop_for_list_items = false;
                indented_literal_mode = false;
                style = SimpleBlockStyle::Literal;
            }

            Some("listing") => {
                stop_for_list_items = false;
                indented_literal_mode = false;
                style = SimpleBlockStyle::Listing;
            }

            Some("source") => {
                stop_for_list_items = false;
                indented_literal_mode = false;
                style = SimpleBlockStyle::Source;
            }

            _ => {}
        }
    }

    let mut next = source;
    let mut filtered_lines: Vec<&'src str> = vec![];
    let mut skipped_comment_line = false;

    // Determine how much indentation to strip from literal paragraphs.
    // In definition list continuations, use minimum indentation across all
    // lines to preserve relative indent. In outline list continuations and
    // non-continuation contexts, strip based on the first line's indent.
    let in_definition_list = parent_list_markers
        .iter()
        .any(|m| matches!(m, ListItemMarker::DefinedTerm { .. }));

    let strip_indent =
        if preserve_literal_indent && style == SimpleBlockStyle::Literal && in_definition_list {
            // Two-pass approach: find minimum indentation across all lines.
            let mut scan = source;
            let mut min_indent = first_line_indent;
            let mut line_count = 0;

            while let Some(line_mi) = scan.take_non_empty_line() {
                let line = line_mi.item;

                // Apply same stop conditions as the main loop.
                if line_count > 0 && line.data() == "+" {
                    break;
                }

                if let Some(n) = line.position(|c| c != ' ' && c != '\t') {
                    min_indent = min_indent.min(n);
                }

                line_count += 1;
                scan = line_mi.after;
            }
            min_indent
        } else {
            first_line_indent
        };

    while let Some(line_mi) = next.take_non_empty_line() {
        let mut line = line_mi.item;

        // If we've skipped a comment line and this is a section header, stop here
        // so the section can be parsed as a separate block. Only do this at the
        // top level (not inside lists), indicated by stop_for_list_items being false.
        if !stop_for_list_items
            && skipped_comment_line
            && style == SimpleBlockStyle::Paragraph
            && is_section_header(line.data())
        {
            break;
        }

        // There are several stop conditions for simple paragraph blocks. These
        // "shouldn't" be encountered on the first line (we shouldn't be calling
        // `SimpleBlock::parse` in these conditions), but in case it is, we simply
        // ignore them on the first line.
        if !filtered_lines.is_empty() {
            // In indented literal mode, only stop for list markers that are NOT indented
            // (at column 1). This allows definition list items to be properly separated.
            let should_check_for_list_marker =
                stop_for_list_items && (!indented_literal_mode || line.col() == 1);

            // If we've already started accumulating content for this list item paragraph,
            // we don't stop for list markers at any level other than our own or a parent
            // level.
            if should_check_for_list_marker
                && let Some(marker_mi) = ListItemMarker::parse(line, parser)
            {
                // In description list continuation context, don't stop for
                // deeper-nested description list markers (e.g., ::: when the
                // current context is ::). They are treated as paragraph text.
                let is_ancestor_list = parent_list_markers
                    .iter()
                    .any(|p| p.is_match_for(&marker_mi.item));

                if is_ancestor_list || !preserve_literal_indent {
                    break;
                }
            }

            if line.data() == "+" {
                break;
            }

            if line.starts_with('[') && line.ends_with(']') {
                break;
            }

            if (line.starts_with('/')
                || line.starts_with('-')
                || line.starts_with('.')
                || line.starts_with('+')
                || line.starts_with('=')
                || line.starts_with('*')
                || line.starts_with('_'))
                && (RawDelimitedBlock::is_valid_delimiter(&line)
                    || CompoundDelimitedBlock::is_valid_delimiter(&line))
            {
                break;
            }
        }

        next = line_mi.after;

        // Only strip comment lines in paragraph style. In literal/listing/source
        // blocks, "//" lines are preserved as content.
        if style == SimpleBlockStyle::Paragraph
            && line.starts_with("//")
            && !line.starts_with("///")
        {
            skipped_comment_line = true;
            continue;
        }

        // Strip at most the calculated indentation amount.
        let should_strip_indent = strip_indent > 0;

        if should_strip_indent && let Some(n) = line.position(|c| c != ' ' && c != '\t') {
            line = line.into_parse_result(n.min(strip_indent)).after;
        };

        filtered_lines.push(line.trim_trailing_whitespace().data());
    }

    let source = source.trim_remainder(next).trim_trailing_whitespace();
    if source.is_empty() {
        return None;
    }

    let filtered_lines = filtered_lines.join("\n");
    let mut content: Content<'src> = Content::from_filtered(source, filtered_lines);

    let sub_group = match style {
        // Only apply Verbatim substitutions to literal blocks detected by indentation.
        // Listing and Source styles declared via attribute list still use Normal subs.
        SimpleBlockStyle::Literal => SubstitutionGroup::Verbatim,
        SimpleBlockStyle::Listing | SimpleBlockStyle::Source | SimpleBlockStyle::Paragraph => {
            SubstitutionGroup::Normal
        }
    };

    sub_group.override_via_attrlist(attrlist.as_ref()).apply(
        &mut content,
        parser,
        attrlist.as_ref(),
    );

    Some(MatchedItem {
        item: (content, style),
        after: next,
    })
}

impl<'src> IsBlock<'src> for SimpleBlock<'src> {
    fn content_model(&self) -> ContentModel {
        ContentModel::Simple
    }

    fn rendered_content(&self) -> Option<&str> {
        Some(self.content.rendered())
    }

    fn raw_context(&self) -> CowStr<'src> {
        "paragraph".into()
    }

    fn title_source(&'src self) -> Option<Span<'src>> {
        self.title_source
    }

    fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    fn anchor(&'src self) -> Option<Span<'src>> {
        self.anchor
    }

    fn anchor_reftext(&'src self) -> Option<Span<'src>> {
        self.anchor_reftext
    }

    fn attrlist(&'src self) -> Option<&'src Attrlist<'src>> {
        self.attrlist.as_ref()
    }
}

impl<'src> HasSpan<'src> for SimpleBlock<'src> {
    fn span(&self) -> Span<'src> {
        self.source
    }
}

/// Returns true if the line looks like a section header.
/// Matches `== `, `=== `, etc. (AsciiDoc) or `## `, `### `, etc. (Markdown).
fn is_section_header(line: &str) -> bool {
    // AsciiDoc style: `== `, `=== `, etc. (at least 2 `=` followed by space)
    if line.starts_with("==") {
        let rest = line.trim_start_matches('=');
        if rest.starts_with(' ') {
            return true;
        }
    }

    // Markdown style: `## `, `### `, etc. (at least 2 `#` followed by space)
    if line.starts_with("##") {
        let rest = line.trim_start_matches('#');
        if rest.starts_with(' ') {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::ops::Deref;

    use crate::{
        blocks::{ContentModel, SimpleBlockStyle, metadata::BlockMetadata},
        tests::prelude::*,
    };

    #[test]
    fn impl_clone() {
        // Silly test to mark the #[derive(...)] line as covered.
        let mut parser = Parser::default();

        let b1 =
            crate::blocks::SimpleBlock::parse(&BlockMetadata::new("abc"), &mut parser).unwrap();

        let b2 = b1.item.clone();
        assert_eq!(b1.item, b2);
    }

    #[test]
    fn style_enum_impl_debug() {
        assert_eq!(
            format!("{:?}", SimpleBlockStyle::Paragraph),
            "SimpleBlockStyle::Paragraph"
        );

        assert_eq!(
            format!("{:?}", SimpleBlockStyle::Literal),
            "SimpleBlockStyle::Literal"
        );

        assert_eq!(
            format!("{:?}", SimpleBlockStyle::Listing),
            "SimpleBlockStyle::Listing"
        );

        assert_eq!(
            format!("{:?}", SimpleBlockStyle::Source),
            "SimpleBlockStyle::Source"
        );
    }

    #[test]
    fn empty_source() {
        let mut parser = Parser::default();
        assert!(crate::blocks::SimpleBlock::parse(&BlockMetadata::new(""), &mut parser).is_none());
    }

    #[test]
    fn only_spaces() {
        let mut parser = Parser::default();
        assert!(
            crate::blocks::SimpleBlock::parse(&BlockMetadata::new("    "), &mut parser).is_none()
        );
    }

    #[test]
    fn single_line() {
        let mut parser = Parser::default();
        let mi =
            crate::blocks::SimpleBlock::parse(&BlockMetadata::new("abc"), &mut parser).unwrap();

        assert_eq!(
            mi.item,
            SimpleBlock {
                content: Content {
                    original: Span {
                        data: "abc",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "abc",
                },
                source: Span {
                    data: "abc",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },
        );

        assert_eq!(mi.item.content_model(), ContentModel::Simple);
        assert_eq!(mi.item.rendered_content().unwrap(), "abc");
        assert_eq!(mi.item.raw_context().deref(), "paragraph");
        assert_eq!(mi.item.resolved_context().deref(), "paragraph");
        assert!(mi.item.declared_style().is_none());
        assert!(mi.item.id().is_none());
        assert!(mi.item.roles().is_empty());
        assert!(mi.item.options().is_empty());
        assert!(mi.item.title_source().is_none());
        assert!(mi.item.title().is_none());
        assert!(mi.item.anchor().is_none());
        assert!(mi.item.anchor_reftext().is_none());
        assert!(mi.item.attrlist().is_none());
        assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 1,
                col: 4,
                offset: 3
            }
        );
    }

    #[test]
    fn multiple_lines() {
        let mut parser = Parser::default();
        let mi = crate::blocks::SimpleBlock::parse(&BlockMetadata::new("abc\ndef"), &mut parser)
            .unwrap();

        assert_eq!(
            mi.item,
            SimpleBlock {
                content: Content {
                    original: Span {
                        data: "abc\ndef",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "abc\ndef",
                },
                source: Span {
                    data: "abc\ndef",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            }
        );

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 2,
                col: 4,
                offset: 7
            }
        );

        assert_eq!(mi.item.rendered_content().unwrap(), "abc\ndef");
    }

    #[test]
    fn consumes_blank_lines_after() {
        let mut parser = Parser::default();
        let mi = crate::blocks::SimpleBlock::parse(&BlockMetadata::new("abc\n\ndef"), &mut parser)
            .unwrap();

        assert_eq!(
            mi.item,
            SimpleBlock {
                content: Content {
                    original: Span {
                        data: "abc",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "abc",
                },
                source: Span {
                    data: "abc",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            }
        );

        assert_eq!(
            mi.after,
            Span {
                data: "def",
                line: 3,
                col: 1,
                offset: 5
            }
        );
    }

    #[test]
    fn overrides_sub_group_via_subs_attribute() {
        let mut parser = Parser::default();
        let mi = crate::blocks::SimpleBlock::parse(
            &BlockMetadata::new("[subs=quotes]\na<b>c *bold*\n\ndef"),
            &mut parser,
        )
        .unwrap();

        assert_eq!(
            mi.item,
            SimpleBlock {
                content: Content {
                    original: Span {
                        data: "a<b>c *bold*",
                        line: 2,
                        col: 1,
                        offset: 14,
                    },
                    rendered: "a<b>c <strong>bold</strong>",
                },
                source: Span {
                    data: "[subs=quotes]\na<b>c *bold*",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: Some(Attrlist {
                    attributes: &[ElementAttribute {
                        name: Some("subs"),
                        value: "quotes",
                        shorthand_items: &[],
                    },],
                    anchor: None,
                    source: Span {
                        data: "subs=quotes",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            }
        );

        assert_eq!(
            mi.after,
            Span {
                data: "def",
                line: 4,
                col: 1,
                offset: 28
            }
        );

        assert_eq!(
            mi.item.rendered_content().unwrap(),
            "a<b>c <strong>bold</strong>"
        );
    }
}
