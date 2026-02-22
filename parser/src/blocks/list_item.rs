use std::slice::Iter;

use crate::{
    HasSpan, Parser, Span,
    attributes::Attrlist,
    blocks::{
        Block, ContentModel, IsBlock, ListBlock, ListItemMarker, SimpleBlock,
        metadata::BlockMetadata,
    },
    internal::debug::DebugSliceReference,
    span::MatchedItem,
    strings::CowStr,
    warnings::Warning,
};

/// A list item is a special kind of block that contains one or more blocks
/// attached to it. In the simplest case, this will be a single [`SimpleBlock`]
/// with the principal text for the list item. In other cases, it may be any
/// number of blocks of any type which, together, form an entry in a list which
/// is the immediate parent of this block.
///
/// [`SimpleBlock`]: crate::blocks::SimpleBlock
#[derive(Clone, Eq, PartialEq)]
pub struct ListItem<'src> {
    marker: ListItemMarker<'src>,
    blocks: Vec<Block<'src>>,
    source: Span<'src>,
    anchor: Option<Span<'src>>,
    anchor_reftext: Option<Span<'src>>,
    attrlist: Option<Attrlist<'src>>,
}

impl<'src> ListItem<'src> {
    pub(crate) fn parse(
        metadata: &BlockMetadata<'src>,
        parent_list_markers: &[ListItemMarker<'src>],
        parser: &mut Parser,
        warnings: &mut Vec<Warning<'src>>,
    ) -> Option<MatchedItem<'src, Self>> {
        let source = metadata.block_start.discard_empty_lines();

        let marker_mi = ListItemMarker::parse(source, parser)?;
        let mut marker = marker_mi.item;

        // Register any leading inline anchors in the description list term and apply
        // macros substitution to render the anchor.
        marker.register_leading_anchors(parser, warnings);

        let mut list_markers_including_peer = parent_list_markers.to_vec();
        list_markers_including_peer.push(marker.clone());

        let mut blocks: Vec<Block<'src>> = vec![];

        // Text after list item marker is always a simple block with no metadata.
        let no_metadata = BlockMetadata {
            title_source: None,
            title: None,
            anchor: None,
            anchor_reftext: None,
            attrlist: None,
            source: marker_mi.after,
            block_start: marker_mi.after,
        };

        // For description lists, the content after the marker can be empty.
        // For other list types, we require content.
        let mut next = if let Some(simple_block_mi) = SimpleBlock::parse_for_list_item(
            &no_metadata,
            parser,
            false,
            &list_markers_including_peer,
        ) {
            blocks.push(Block::Simple(simple_block_mi.item));
            simple_block_mi.after
        } else if matches!(marker, ListItemMarker::DefinedTerm { .. }) {
            // Description list items can have empty content on the same line as the marker.
            // The content may be on subsequent lines, so we try to parse from the next
            // non-empty line.
            let next_source = marker_mi.after.discard_empty_lines();

            // Check for continuation marker before parsing. If a continuation marker is
            // present, skip directly to the main loop which handles continuations properly.
            let next_line_mi = next_source.take_normalized_line();

            if next_line_mi.item.data() == "+" {
                // Continuation marker found; skip straight to the main loop.
                // Use next_source (not marker_mi.after) since we already skipped empty lines.
                next_source
            } else if ListItemMarker::parse(next_source, parser).is_some() {
                // Next line is another list item marker (possibly a sibling term).
                // Don't parse it as content; let the list parser handle it.
                marker_mi.after
            } else {
                let next_line_metadata = BlockMetadata {
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    source: next_source,
                    block_start: next_source,
                };

                // For definition lists, indented content is treated as a paragraph
                // (not literal), with the indentation stripped.
                if let Some(simple_block_mi) =
                    SimpleBlock::parse_for_definition_list(&next_line_metadata, parser)
                {
                    blocks.push(Block::Simple(simple_block_mi.item));
                    simple_block_mi.after
                } else {
                    marker_mi.after
                }
            }
        } else {
            // Other list types require content after the marker.
            return None;
        };

        let mut next_block_must_be_indented = false;
        let mut continuation_active = false;
        let mut had_content_starting_with_plus = false;

        loop {
            if next.is_empty() {
                break;
            }

            let next_line_mi: MatchedItem<'_, Span<'_>> = next.take_normalized_line();

            // Don't consume `+` as continuation if:
            // - A continuation is already active (consecutive `+` - second one becomes
            //   content)
            // - We've already had a block that started with `+` as content (trailing `+`
            //   markers)
            if next_line_mi.item.data() == "+"
                && !continuation_active
                && !had_content_starting_with_plus
            {
                next = next_line_mi.after;
                next_block_must_be_indented = false;
                continuation_active = true;
                continue;
            }

            if next_line_mi.item.data().is_empty() {
                if parent_list_markers.is_empty() {
                    next = next.discard_empty_lines();
                    next_block_must_be_indented = true;
                    continue;
                } else {
                    next = next.discard_empty_lines();
                    break;
                }
            }

            let is_indented = next.starts_with(' ') || next.starts_with('\t');
            let metadata = BlockMetadata::parse(next, parser);

            if let Some(list_item_marker_mi) =
                ListItemMarker::parse(metadata.item.block_start, parser)
            {
                // We've found a new list item. How does it compare with the existing item in
                // the hierarchy?
                let new_item_marker = list_item_marker_mi.item;

                if marker.is_match_for(&new_item_marker) {
                    // New item is a peer to this item; nothing further for the current item.
                    break;
                }

                if parent_list_markers
                    .iter()
                    .any(|parent| parent.is_match_for(&new_item_marker))
                {
                    // We matched a parent marker type. This list is complete; roll up the
                    // hierarchy.
                    break;
                }

                // We haven't encountered this marker before. Add a new nesting level. The new
                // list will be a child block of this list item.

                let mut nested_list_markers = parent_list_markers.to_owned();
                nested_list_markers.push(marker.clone());

                // NOTE: The call to `ListBlock::parse` *should* succeed (as in I can't think of
                // a test case where it would fail). We use the `?` to provide a safe escape in
                // case it doesn't.
                let nested_list_mi = ListBlock::parse_inside_list(
                    &metadata.item,
                    &nested_list_markers,
                    parser,
                    warnings,
                )?;

                blocks.push(Block::List(nested_list_mi.item));

                next = nested_list_mi.after;
                continuation_active = false;
                next_block_must_be_indented = true;
                continue;
            }

            if next_block_must_be_indented && !is_indented {
                break;
            }

            // If there's block metadata but no block, just discard it and continue.
            if metadata
                .item
                .block_start
                .take_normalized_line()
                .item
                .is_empty()
            {
                next = metadata.item.block_start.discard_empty_lines();
                continue;
            }

            // A list item does not terminate if subsequent blocks are indented (i.e. use
            // literal syntax).
            //
            // For description lists with continuation, use minimum indentation logic to
            // determine if indentation should be stripped. If any line has 0 indentation,
            // preserve all indentation.
            let is_dlist_continuation =
                continuation_active && matches!(marker, ListItemMarker::DefinedTerm { .. });

            let indented_block_maw = Block::parse_for_list_item(
                next,
                parser,
                &list_markers_including_peer,
                is_dlist_continuation,
            );
            warnings.extend(indented_block_maw.warnings);

            let Some(indented_block_mi) = indented_block_maw.item else {
                break;
            };

            // After a continuation marker, subsequent blocks don't need to be indented.
            // However, document attributes don't consume the continuation status.
            let is_document_attribute =
                matches!(indented_block_mi.item, Block::DocumentAttribute(_));

            // Document attributes should not be added to the list item blocks.
            // They're processed for their side effects but don't appear in the output.
            // Similarly, orphaned metadata blocks shouldn't be added; they'll be
            // re-parsed on the next iteration where they can attach to a real block.
            if !is_document_attribute {
                blocks.push(indented_block_mi.item);
            }
            next = indented_block_mi.after;

            if is_document_attribute {
                // Document attributes and orphaned metadata are transparent to
                // continuation logic. Keep continuation_active
                // and next_block_must_be_indented unchanged.
            } else if continuation_active {
                // This block consumed the continuation.
                // The next block after this one will need to be indented (or have another
                // continuation).
                //
                // If the block started with `+` as content (not as continuation), mark it
                // so we don't allow more continuation markers. This handles odd input like
                // consecutive `+` markers.
                if next_line_mi.item.data() == "+" {
                    had_content_starting_with_plus = true;
                }
                continuation_active = false;
                next_block_must_be_indented = true;
            } else {
                // No active continuation; next block must be indented.
                next_block_must_be_indented = true;
            }
        }

        let source = source.trim_remainder(next).trim_trailing_whitespace();

        Some(MatchedItem {
            item: Self {
                marker,
                blocks,
                source,
                anchor: metadata.anchor,
                anchor_reftext: metadata.anchor_reftext,
                attrlist: metadata.attrlist.clone(),
            },
            after: next,
        })
    }

    /// Returns the list item marker that was used for this item.
    pub fn list_item_marker(&self) -> ListItemMarker<'src> {
        self.marker.clone()
    }
}

impl<'src> IsBlock<'src> for ListItem<'src> {
    fn content_model(&self) -> ContentModel {
        ContentModel::Compound
    }

    fn raw_context(&self) -> CowStr<'src> {
        "list_item".into()
    }

    fn nested_blocks(&'src self) -> Iter<'src, Block<'src>> {
        self.blocks.iter()
    }

    fn title_source(&'src self) -> Option<Span<'src>> {
        None
    }

    fn title(&self) -> Option<&str> {
        None
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

impl<'src> HasSpan<'src> for ListItem<'src> {
    fn span(&self) -> Span<'src> {
        self.source
    }
}

impl std::fmt::Debug for ListItem<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ListItem")
            .field("marker", &self.marker)
            .field("blocks", &DebugSliceReference(&self.blocks))
            .field("source", &self.source)
            .field("anchor", &self.anchor)
            .field("anchor_reftext", &self.anchor_reftext)
            .field("attrlist", &self.attrlist)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use pretty_assertions_sorted::assert_eq;

    use crate::{
        HasSpan,
        blocks::{ContentModel, IsBlock, SimpleBlockStyle, metadata::BlockMetadata},
        span::MatchedItem,
        tests::prelude::*,
        warnings::Warning,
    };

    fn li_parse<'a>(source: &'a str) -> Option<MatchedItem<'a, crate::blocks::ListItem<'a>>> {
        let mut parser = crate::Parser::default();
        let mut warnings: Vec<Warning<'a>> = vec![];

        let metadata = BlockMetadata::parse(crate::Span::new(source), &mut parser).item;

        let result =
            crate::blocks::list_item::ListItem::parse(&metadata, &[], &mut parser, &mut warnings);

        assert!(warnings.is_empty());

        result
    }

    #[test]
    fn hyphen() {
        assert!(li_parse("-xyz").is_none());
        assert!(li_parse("-- x").is_none());

        let li = li_parse("- blah").unwrap();

        assert_eq!(
            li.item,
            ListItem {
                marker: ListItemMarker::Hyphen(Span {
                    data: "-",
                    line: 1,
                    col: 1,
                    offset: 0,
                },),
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "blah",
                            line: 1,
                            col: 3,
                            offset: 2,
                        },
                        rendered: "blah",
                    },
                    source: Span {
                        data: "blah",
                        line: 1,
                        col: 3,
                        offset: 2,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: "- blah",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            }
        );

        assert_eq!(li.item.content_model(), ContentModel::Compound);
        assert_eq!(li.item.raw_context().as_ref(), "list_item");

        let mut li_blocks = li.item.nested_blocks();

        assert_eq!(
            li_blocks.next().unwrap(),
            &Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "blah",
                        line: 1,
                        col: 3,
                        offset: 2,
                    },
                    rendered: "blah",
                },
                source: Span {
                    data: "blah",
                    line: 1,
                    col: 3,
                    offset: 2,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            })
        );
        assert!(li_blocks.next().is_none());

        assert!(li.item.title_source().is_none());
        assert!(li.item.title().is_none());
        assert!(li.item.anchor().is_none());
        assert!(li.item.anchor_reftext().is_none());
        assert!(li.item.attrlist().is_none());

        assert_eq!(
            li.item.span(),
            Span {
                data: "- blah",
                line: 1,
                col: 1,
                offset: 0,
            }
        );

        assert_eq!(
            li.after,
            Span {
                data: "",
                line: 1,
                col: 7,
                offset: 6,
            }
        );

        assert_eq!(
            format!("{:#?}", li.item),
            "ListItem {\n    marker: ListItemMarker::Hyphen(\n        Span {\n            data: \"-\",\n            line: 1,\n            col: 1,\n            offset: 0,\n        },\n    ),\n    blocks: &[\n        Block::Simple(\n            SimpleBlock {\n                content: Content {\n                    original: Span {\n                        data: \"blah\",\n                        line: 1,\n                        col: 3,\n                        offset: 2,\n                    },\n                    rendered: \"blah\",\n                },\n                source: Span {\n                    data: \"blah\",\n                    line: 1,\n                    col: 3,\n                    offset: 2,\n                },\n                style: SimpleBlockStyle::Paragraph,\n                title_source: None,\n                title: None,\n                anchor: None,\n                anchor_reftext: None,\n                attrlist: None,\n            },\n        ),\n    ],\n    source: Span {\n        data: \"- blah\",\n        line: 1,\n        col: 1,\n        offset: 0,\n    },\n    anchor: None,\n    anchor_reftext: None,\n    attrlist: None,\n}"
        );
    }

    #[test]
    fn asterisks() {
        assert!(li_parse("*").is_none());
        assert!(li_parse("*xyz").is_none());
        assert!(li_parse("*- xyz").is_none());

        let li = li_parse("* blah").unwrap();

        assert_eq!(
            li.item,
            ListItem {
                marker: ListItemMarker::Asterisks(Span {
                    data: "*",
                    line: 1,
                    col: 1,
                    offset: 0,
                },),
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "blah",
                            line: 1,
                            col: 3,
                            offset: 2,
                        },
                        rendered: "blah",
                    },
                    source: Span {
                        data: "blah",
                        line: 1,
                        col: 3,
                        offset: 2,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: "* blah",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            }
        );

        assert_eq!(
            li.item.span(),
            Span {
                data: "* blah",
                line: 1,
                col: 1,
                offset: 0,
            }
        );

        assert_eq!(
            li.after,
            Span {
                data: "",
                line: 1,
                col: 7,
                offset: 6,
            }
        );
    }
}
