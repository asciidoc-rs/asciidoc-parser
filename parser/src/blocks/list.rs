use std::slice::Iter;

use crate::{
    HasSpan, Parser, Span,
    attributes::Attrlist,
    blocks::{Block, ContentModel, IsBlock, ListItem, ListItemMarker, metadata::BlockMetadata},
    internal::debug::DebugSliceReference,
    span::MatchedItem,
    strings::CowStr,
    warnings::{Warning, WarningType},
};

/// A list contains a sequence of items prefixed with symbol, such as a disc
/// (aka bullet). Each individual item in the list is represented by a
/// [`ListItem`].
///
/// [`ListItem`]: crate::blocks::ListItem
#[derive(Clone, Eq, PartialEq)]
pub struct ListBlock<'src> {
    type_: ListType,
    items: Vec<Block<'src>>,
    source: Span<'src>,
    title_source: Option<Span<'src>>,
    title: Option<String>,
    anchor: Option<Span<'src>>,
    anchor_reftext: Option<Span<'src>>,
    attrlist: Option<Attrlist<'src>>,
}

impl<'src> ListBlock<'src> {
    pub(crate) fn parse(
        metadata: &BlockMetadata<'src>,
        parser: &mut Parser,
        warnings: &mut Vec<Warning<'src>>,
    ) -> Option<MatchedItem<'src, Self>> {
        Self::parse_inside_list(metadata, &[], parser, warnings)
    }

    pub(crate) fn parse_inside_list(
        metadata: &BlockMetadata<'src>,
        parent_list_markers: &[ListItemMarker<'src>],
        parser: &mut Parser,
        warnings: &mut Vec<Warning<'src>>,
    ) -> Option<MatchedItem<'src, Self>> {
        let source = metadata.block_start.discard_empty_lines();

        let mut items: Vec<Block<'src>> = vec![];
        let mut next_item_source = source;
        let mut first_marker: Option<ListItemMarker<'src>> = None;
        let mut expected_ordinal: Option<u32> = None;

        loop {
            let next_line_mi = next_item_source.take_normalized_line();

            if next_line_mi.item.data().is_empty() || next_line_mi.item.data() == "+" {
                if next_item_source.is_empty() || !parent_list_markers.is_empty() {
                    break;
                } else {
                    next_item_source = next_line_mi.after;
                    continue;
                }
            }

            // TEMPORARY: Ignore block metadata for list items.
            let list_item_metadata = BlockMetadata {
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
                source: next_item_source,
                block_start: next_item_source,
            };

            let Some(list_item_marker_mi) =
                ListItemMarker::parse(list_item_metadata.block_start, parser)
            else {
                break;
            };

            let this_item_marker = list_item_marker_mi.item;

            // If this item's marker doesn't match the existing list marker, we are changing
            // levels in the list hierarchy.
            if let Some(ref first_marker) = first_marker {
                if !first_marker.is_match_for(&this_item_marker)
                    && parent_list_markers
                        .iter()
                        .any(|parent| parent.is_match_for(&this_item_marker))
                {
                    // We matched a parent marker type. This list is complete; roll up the
                    // hierarchy.
                    break;
                }

                // Check if the marker is in sequence for explicit ordered lists.
                if let Some(actual_ordinal) = this_item_marker.ordinal_value() {
                    if let Some(expected) = expected_ordinal
                        && actual_ordinal != expected
                    {
                        // Warn about out-of-sequence marker.
                        if let (Some(expected_text), Some(actual_text)) = (
                            first_marker.ordinal_to_marker_text(expected),
                            first_marker.ordinal_to_marker_text(actual_ordinal),
                        ) {
                            warnings.push(Warning {
                                source: this_item_marker.span(),
                                warning: WarningType::ListItemOutOfSequence(
                                    expected_text,
                                    actual_text,
                                ),
                            });
                        }
                    }
                    expected_ordinal = Some(actual_ordinal + 1);
                }
            } else {
                first_marker = Some(this_item_marker.clone());

                // Initialize expected ordinal from first marker's value.
                if let Some(ordinal) = this_item_marker.ordinal_value() {
                    expected_ordinal = Some(ordinal + 1);
                }
            }

            let Some(list_item_mi) =
                ListItem::parse(&list_item_metadata, parent_list_markers, parser, warnings)
            else {
                break;
            };

            items.push(Block::ListItem(list_item_mi.item));
            next_item_source = list_item_mi.after;
        }

        if items.is_empty() {
            return None;
        }

        let first_marker = first_marker?;
        let type_ = match first_marker {
            ListItemMarker::Asterisks(_) => ListType::Unordered,
            ListItemMarker::Hyphen(_) => ListType::Unordered,
            ListItemMarker::Bullet(_) => ListType::Unordered,
            ListItemMarker::Dots(_) => ListType::Ordered,
            ListItemMarker::AlphaListCapital(_) => ListType::Ordered,
            ListItemMarker::AlphaListLower(_) => ListType::Ordered,
            ListItemMarker::RomanNumeralLower(_) => ListType::Ordered,
            ListItemMarker::RomanNumeralUpper(_) => ListType::Ordered,
            ListItemMarker::ArabicNumeral(_) => ListType::Ordered,

            ListItemMarker::DefinedTerm {
                term: _,
                marker: _,
                source: _,
            } => ListType::Description,
        };

        Some(MatchedItem {
            item: Self {
                type_,
                items,
                source: metadata
                    .source
                    .trim_remainder(next_item_source)
                    .trim_trailing_line_end()
                    .trim_trailing_whitespace(),
                title_source: metadata.title_source,
                title: metadata.title.clone(),
                anchor: metadata.anchor,
                anchor_reftext: metadata.anchor_reftext,
                attrlist: metadata.attrlist.clone(),
            },
            after: next_item_source,
        })
    }

    /// Returns the type of this list.
    pub fn type_(&self) -> ListType {
        self.type_
    }

    /// Returns the style class for this list based on the marker length.
    /// For ordered lists, the style is determined by the number of dots:
    /// - 1 dot: arabic (1, 2, 3, ...)
    /// - 2 dots: loweralpha (a, b, c, ...)
    /// - 3 dots: lowerroman (i, ii, iii, ...)
    /// - 4 dots: upperalpha (A, B, C, ...)
    /// - 5 dots: upperroman (I, II, III, ...)
    pub fn marker_style(&self) -> Option<&'static str> {
        let first_marker = self.items.first()?.as_list_item()?.list_item_marker();

        match first_marker {
            ListItemMarker::Dots(span) => {
                let marker_len = span.data().len();
                match marker_len {
                    1 => Some("arabic"),
                    2 => Some("loweralpha"),
                    3 => Some("lowerroman"),
                    4 => Some("upperalpha"),
                    5 => Some("upperroman"),
                    _ => Some("arabic"),
                }
            }
            ListItemMarker::ArabicNumeral(_) => Some("arabic"),
            ListItemMarker::AlphaListLower(_) => Some("loweralpha"),
            ListItemMarker::AlphaListCapital(_) => Some("upperalpha"),
            ListItemMarker::RomanNumeralLower(_) => Some("lowerroman"),
            ListItemMarker::RomanNumeralUpper(_) => Some("upperroman"),
            _ => None,
        }
    }
}

impl<'src> IsBlock<'src> for ListBlock<'src> {
    fn content_model(&self) -> ContentModel {
        ContentModel::Compound
    }

    fn raw_context(&self) -> CowStr<'src> {
        "list".into()
    }

    fn nested_blocks(&'src self) -> Iter<'src, Block<'src>> {
        self.items.iter()
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

impl<'src> HasSpan<'src> for ListBlock<'src> {
    fn span(&self) -> Span<'src> {
        self.source
    }
}

impl std::fmt::Debug for ListBlock<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ListBlock")
            .field("type_", &self.type_)
            .field("items", &DebugSliceReference(&self.items))
            .field("source", &self.source)
            .field("title_source", &self.title_source)
            .field("title", &self.title)
            .field("anchor", &self.anchor)
            .field("anchor_reftext", &self.anchor_reftext)
            .field("attrlist", &self.attrlist)
            .finish()
    }
}

/// Represents the type of a list.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum ListType {
    /// An unordered list is a list with items prefixed with symbol, such as a
    /// disc (aka bullet).
    Unordered,

    /// An ordered list is a list with items prefixed with a number or other
    /// sequential mark.
    Ordered,

    /// A description list is an association list that consists of one or more
    /// terms (or sets of terms) that each have a description.
    Description,
}

impl std::fmt::Debug for ListType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListType::Unordered => write!(f, "ListType::Unordered"),
            ListType::Ordered => write!(f, "ListType::Ordered"),
            ListType::Description => write!(f, "ListType::Description"),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use crate::{
        blocks::{ContentModel, IsBlock, ListType, SimpleBlockStyle, metadata::BlockMetadata},
        content::SubstitutionGroup,
        span::MatchedItem,
        tests::prelude::*,
        warnings::Warning,
    };

    fn list_parse<'a>(source: &'a str) -> Option<MatchedItem<'a, crate::blocks::ListBlock<'a>>> {
        let mut parser = crate::Parser::default();
        let mut warnings: Vec<Warning<'a>> = vec![];

        let metadata = BlockMetadata::parse(crate::Span::new(source), &mut parser).item;

        let result = crate::blocks::list::ListBlock::parse(&metadata, &mut parser, &mut warnings);

        assert!(warnings.is_empty());

        result
    }

    #[test]
    fn basic_case() {
        assert!(list_parse("-xyz").is_none());
        assert!(list_parse("-- x").is_none());

        let list = list_parse("- blah").unwrap();

        assert_eq!(
            list.item,
            ListBlock {
                type_: ListType::Unordered,
                items: &[Block::ListItem(ListItem {
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
                },),],
                source: Span {
                    data: "- blah",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            }
        );

        assert_eq!(list.item.type_(), ListType::Unordered);
        assert_eq!(list.item.content_model(), ContentModel::Compound);
        assert_eq!(list.item.raw_context().as_ref(), "list");

        let mut list_blocks = list.item.nested_blocks();

        let list_item = list_blocks.next().unwrap();

        assert_eq!(
            list_item,
            &Block::ListItem(ListItem {
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
            })
        );

        assert_eq!(list_item.content_model(), ContentModel::Compound);
        assert_eq!(list_item.raw_context().as_ref(), "list_item");

        let mut li_blocks = list_item.nested_blocks();

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

        assert!(list_item.title_source().is_none());
        assert!(list_item.title().is_none());
        assert!(list_item.anchor().is_none());
        assert!(list_item.anchor_reftext().is_none());
        assert!(list_item.attrlist().is_none());
        assert_eq!(list_item.substitution_group(), SubstitutionGroup::Normal);
        assert_eq!(
            list_item.span(),
            Span {
                data: "- blah",
                line: 1,
                col: 1,
                offset: 0,
            }
        );

        assert!(list_blocks.next().is_none());

        assert!(list.item.title_source().is_none());
        assert!(list.item.title().is_none());
        assert!(list.item.anchor().is_none());
        assert!(list.item.anchor_reftext().is_none());
        assert!(list.item.attrlist().is_none());

        assert_eq!(
            format!("{:#?}", list.item),
            "ListBlock {\n    type_: ListType::Unordered,\n    items: &[\n        Block::ListItem(\n            ListItem {\n                marker: ListItemMarker::Hyphen(\n                    Span {\n                        data: \"-\",\n                        line: 1,\n                        col: 1,\n                        offset: 0,\n                    },\n                ),\n                blocks: &[\n                    Block::Simple(\n                        SimpleBlock {\n                            content: Content {\n                                original: Span {\n                                    data: \"blah\",\n                                    line: 1,\n                                    col: 3,\n                                    offset: 2,\n                                },\n                                rendered: \"blah\",\n                            },\n                            source: Span {\n                                data: \"blah\",\n                                line: 1,\n                                col: 3,\n                                offset: 2,\n                            },\n                            style: SimpleBlockStyle::Paragraph,\n                            title_source: None,\n                            title: None,\n                            anchor: None,\n                            anchor_reftext: None,\n                            attrlist: None,\n                        },\n                    ),\n                ],\n                source: Span {\n                    data: \"- blah\",\n                    line: 1,\n                    col: 1,\n                    offset: 0,\n                },\n                anchor: None,\n                anchor_reftext: None,\n                attrlist: None,\n            },\n        ),\n    ],\n    source: Span {\n        data: \"- blah\",\n        line: 1,\n        col: 1,\n        offset: 0,\n    },\n    title_source: None,\n    title: None,\n    anchor: None,\n    anchor_reftext: None,\n    attrlist: None,\n}"
        );

        assert_eq!(
            list.after,
            Span {
                data: "",
                line: 1,
                col: 7,
                offset: 6,
            }
        );
    }

    #[test]
    fn list_type_impl_debug() {
        assert_eq!(format!("{:#?}", ListType::Unordered), "ListType::Unordered");
        assert_eq!(format!("{:#?}", ListType::Ordered), "ListType::Ordered");

        assert_eq!(
            format!("{:#?}", ListType::Description),
            "ListType::Description"
        );
    }

    #[test]
    fn attrlist_doesnt_exit() {
        let list = list_parse("* Foo\n[loweralpha]\n. Boo\n* Blech").unwrap();

        assert_eq!(
            list.item,
            ListBlock {
                type_: ListType::Unordered,
                items: &[
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::Asterisks(Span {
                            data: "*",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },),
                        blocks: &[
                            Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Foo",
                                        line: 1,
                                        col: 3,
                                        offset: 2,
                                    },
                                    rendered: "Foo",
                                },
                                source: Span {
                                    data: "Foo",
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
                            },),
                            Block::List(ListBlock {
                                type_: ListType::Ordered,
                                items: &[Block::ListItem(ListItem {
                                    marker: ListItemMarker::Dots(Span {
                                        data: ".",
                                        line: 3,
                                        col: 1,
                                        offset: 19,
                                    },),
                                    blocks: &[Block::Simple(SimpleBlock {
                                        content: Content {
                                            original: Span {
                                                data: "Boo",
                                                line: 3,
                                                col: 3,
                                                offset: 21,
                                            },
                                            rendered: "Boo",
                                        },
                                        source: Span {
                                            data: "Boo",
                                            line: 3,
                                            col: 3,
                                            offset: 21,
                                        },
                                        style: SimpleBlockStyle::Paragraph,
                                        title_source: None,
                                        title: None,
                                        anchor: None,
                                        anchor_reftext: None,
                                        attrlist: None,
                                    },),],
                                    source: Span {
                                        data: ". Boo",
                                        line: 3,
                                        col: 1,
                                        offset: 19,
                                    },
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),],
                                source: Span {
                                    data: "[loweralpha]\n. Boo",
                                    line: 2,
                                    col: 1,
                                    offset: 6,
                                },
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: Some(Attrlist {
                                    attributes: &[ElementAttribute {
                                        name: None,
                                        value: "loweralpha",
                                        shorthand_items: &["loweralpha"],
                                    },],
                                    anchor: None,
                                    source: Span {
                                        data: "loweralpha",
                                        line: 2,
                                        col: 2,
                                        offset: 7,
                                    },
                                },),
                            },),
                        ],
                        source: Span {
                            data: "* Foo\n[loweralpha]\n. Boo",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::Asterisks(Span {
                            data: "*",
                            line: 4,
                            col: 1,
                            offset: 25,
                        },),
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "Blech",
                                    line: 4,
                                    col: 3,
                                    offset: 27,
                                },
                                rendered: "Blech",
                            },
                            source: Span {
                                data: "Blech",
                                line: 4,
                                col: 3,
                                offset: 27,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "* Blech",
                            line: 4,
                            col: 1,
                            offset: 25,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: "* Foo\n[loweralpha]\n. Boo\n* Blech",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            }
        );

        assert_eq!(
            list.after,
            Span {
                data: "",
                line: 4,
                col: 8,
                offset: 32,
            }
        );
    }

    #[test]
    fn metadata_merged_across_empty_lines_for_nested_list() {
        // Exercises the `if ext_anchor.is_none()` merge path in
        // ListItem::parse (circa line 283 of list_item.rs).
        let list = list_parse("* Foo\n[loweralpha]\n\n[[anchor]]\n. Boo\n* Blech").unwrap();

        assert_eq!(
            list.item,
            ListBlock {
                type_: ListType::Unordered,
                items: &[
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::Asterisks(Span {
                            data: "*",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },),
                        blocks: &[
                            Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Foo",
                                        line: 1,
                                        col: 3,
                                        offset: 2,
                                    },
                                    rendered: "Foo",
                                },
                                source: Span {
                                    data: "Foo",
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
                            },),
                            Block::List(ListBlock {
                                type_: ListType::Ordered,
                                items: &[Block::ListItem(ListItem {
                                    marker: ListItemMarker::Dots(Span {
                                        data: ".",
                                        line: 5,
                                        col: 1,
                                        offset: 31,
                                    },),
                                    blocks: &[Block::Simple(SimpleBlock {
                                        content: Content {
                                            original: Span {
                                                data: "Boo",
                                                line: 5,
                                                col: 3,
                                                offset: 33,
                                            },
                                            rendered: "Boo",
                                        },
                                        source: Span {
                                            data: "Boo",
                                            line: 5,
                                            col: 3,
                                            offset: 33,
                                        },
                                        style: SimpleBlockStyle::Paragraph,
                                        title_source: None,
                                        title: None,
                                        anchor: None,
                                        anchor_reftext: None,
                                        attrlist: None,
                                    },),],
                                    source: Span {
                                        data: ". Boo",
                                        line: 5,
                                        col: 1,
                                        offset: 31,
                                    },
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),],
                                source: Span {
                                    data: "[loweralpha]\n\n[[anchor]]\n. Boo",
                                    line: 2,
                                    col: 1,
                                    offset: 6,
                                },
                                title_source: None,
                                title: None,
                                anchor: Some(Span {
                                    data: "anchor",
                                    line: 4,
                                    col: 3,
                                    offset: 22,
                                },),
                                anchor_reftext: None,
                                attrlist: Some(Attrlist {
                                    attributes: &[ElementAttribute {
                                        name: None,
                                        value: "loweralpha",
                                        shorthand_items: &["loweralpha"],
                                    },],
                                    anchor: None,
                                    source: Span {
                                        data: "loweralpha",
                                        line: 2,
                                        col: 2,
                                        offset: 7,
                                    },
                                },),
                            },),
                        ],
                        source: Span {
                            data: "* Foo\n[loweralpha]\n\n[[anchor]]\n. Boo",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::Asterisks(Span {
                            data: "*",
                            line: 6,
                            col: 1,
                            offset: 37,
                        },),
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "Blech",
                                    line: 6,
                                    col: 3,
                                    offset: 39,
                                },
                                rendered: "Blech",
                            },
                            source: Span {
                                data: "Blech",
                                line: 6,
                                col: 3,
                                offset: 39,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "* Blech",
                            line: 6,
                            col: 1,
                            offset: 37,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: "* Foo\n[loweralpha]\n\n[[anchor]]\n. Boo\n* Blech",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            }
        );
    }

    #[test]
    fn parent_marker_after_metadata_separated_by_empty_lines() {
        // Exercises the parent_list_markers check in ListItem::parse
        // (circa line 308) where a list marker found after extending metadata
        // past empty lines matches a grandparent marker.
        //
        // Input: three nesting levels, then [[anchor]] + blank line + * marker.
        // The *** item should recognize * as a grandparent marker and break.
        let list =
            list_parse("* grandparent\n** parent\n*** nested\n[[anchor]]\n\n* back to grandparent")
                .unwrap();

        // Outer list has two * items.
        assert_eq!(list.item.nested_blocks().count(), 2);
        assert_eq!(list.item.type_(), ListType::Unordered);

        let mut outer_items = list.item.nested_blocks();

        // First outer item should contain a nested ** list.
        let first_outer = outer_items.next().unwrap();
        let first_outer_blocks: Vec<_> = first_outer.nested_blocks().collect();
        assert_eq!(first_outer_blocks.len(), 2); // SimpleBlock + ListBlock

        // The nested ** list should have one item.
        let nested_list = &first_outer_blocks[1];
        assert_eq!(nested_list.nested_blocks().count(), 1);

        // That ** item should contain a nested *** list.
        let parent_item = nested_list.nested_blocks().next().unwrap();
        let parent_blocks: Vec<_> = parent_item.nested_blocks().collect();
        assert_eq!(parent_blocks.len(), 2); // SimpleBlock + ListBlock

        // The *** list should have one item.
        let innermost_list = &parent_blocks[1];
        assert_eq!(innermost_list.nested_blocks().count(), 1);

        // The *** item should have only its principal text.
        let innermost_item = innermost_list.nested_blocks().next().unwrap();
        assert_eq!(innermost_item.nested_blocks().count(), 1);

        // Second outer item is "back to grandparent".
        let second_outer = outer_items.next().unwrap();
        assert_eq!(second_outer.nested_blocks().count(), 1);
        assert!(outer_items.next().is_none());
    }

    #[test]
    fn marker_style_single_dot() {
        let list = list_parse(". Item one\n. Item two\n").unwrap();
        assert_eq!(list.item.marker_style(), Some("arabic"));
    }

    #[test]
    fn marker_style_double_dots() {
        let list = list_parse(".. Item a\n.. Item b\n").unwrap();
        assert_eq!(list.item.marker_style(), Some("loweralpha"));
    }

    #[test]
    fn marker_style_triple_dots() {
        let list = list_parse("... Item i\n... Item ii\n").unwrap();
        assert_eq!(list.item.marker_style(), Some("lowerroman"));
    }

    #[test]
    fn marker_style_four_dots() {
        let list = list_parse(".... Item A\n.... Item B\n").unwrap();
        assert_eq!(list.item.marker_style(), Some("upperalpha"));
    }

    #[test]
    fn marker_style_five_dots() {
        let list = list_parse("..... Item I\n..... Item II\n").unwrap();
        assert_eq!(list.item.marker_style(), Some("upperroman"));
    }

    #[test]
    fn marker_style_hyphen_returns_none() {
        let list = list_parse("- Item one\n- Item two\n").unwrap();
        assert_eq!(list.item.marker_style(), None);
    }

    #[test]
    fn marker_style_asterisk_returns_none() {
        let list = list_parse("* Item one\n* Item two\n").unwrap();
        assert_eq!(list.item.marker_style(), None);
    }

    #[test]
    fn marker_with_no_content() {
        // Exercises the `break` in `parse_inside_list` when
        // `ListItemMarker::parse` succeeds but `ListItem::parse`
        // returns `None` (marker present, no content after it).
        assert!(list_parse("- ").is_none());
        assert!(list_parse("* ").is_none());
        assert!(list_parse(". ").is_none());
    }

    #[test]
    fn orphaned_title_after_continuation_is_discarded() {
        // Exercises the "If there's block metadata but no block, just discard
        // it and continue." path in ListItem::parse (circa line 368 of
        // list_item.rs). A `+` continuation followed by a block title (`.Title`)
        // and then an empty line means the title is orphaned (no block
        // immediately follows). The title metadata is discarded and the
        // subsequent paragraph is parsed as a continuation block.
        let list = list_parse("* item one\n+\n.Title\n\nsecond paragraph").unwrap();

        // The list should have one item.
        let mut items = list.item.nested_blocks();
        let item = items.next().unwrap();
        assert!(items.next().is_none());

        // The item should have two blocks: the principal text and the
        // continuation paragraph. The orphaned `.Title` should be discarded.
        let blocks: Vec<_> = item.nested_blocks().collect();
        assert_eq!(blocks.len(), 2);

        // First block is the principal text.
        assert_eq!(
            blocks[0],
            &Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "item one",
                        line: 1,
                        col: 3,
                        offset: 2,
                    },
                    rendered: "item one",
                },
                source: Span {
                    data: "item one",
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

        // Second block is the continuation paragraph (no title attached).
        assert_eq!(
            blocks[1],
            &Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "second paragraph",
                        line: 5,
                        col: 1,
                        offset: 21,
                    },
                    rendered: "second paragraph",
                },
                source: Span {
                    data: "second paragraph",
                    line: 5,
                    col: 1,
                    offset: 21,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            })
        );
    }

    #[test]
    fn block_list_enum_case() {
        let mut parser = crate::Parser::default();

        let mi = crate::blocks::Block::parse(crate::Span::new("- blah"), &mut parser)
            .unwrap_if_no_warnings()
            .unwrap();

        assert!(matches!(mi.item, crate::blocks::Block::List(_)));

        assert_eq!(mi.item.content_model(), ContentModel::Compound);
        assert!(mi.item.rendered_content().is_none());
        assert_eq!(mi.item.raw_context().as_ref(), "list");
        assert_eq!(mi.item.nested_blocks().count(), 1);
        assert!(mi.item.title_source().is_none());
        assert!(mi.item.title().is_none());
        assert!(mi.item.anchor().is_none());
        assert!(mi.item.anchor_reftext().is_none());
        assert!(mi.item.attrlist().is_none());
        assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

        assert_eq!(
            mi.item.span(),
            Span {
                data: "- blah",
                line: 1,
                col: 1,
                offset: 0,
            }
        );

        let debug_str = format!("{:?}", mi.item);
        assert!(debug_str.starts_with("Block::List("));
    }
}
