use crate::{
    HasSpan, Parser, Span,
    attributes::Attrlist,
    blocks::{
        Block, ChildBlocks, ContentModel, IsBlock, ListItem, ListItemMarker,
        metadata::BlockMetadata,
    },
    content::Content,
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
#[derive(Clone, Eq, Hash, PartialEq)]
pub struct ListBlock<'src> {
    type_: ListType,
    items: Vec<Block<'src>>,
    source: Span<'src>,
    title_source: Option<Span<'src>>,
    title: Option<Content<'src>>,
    anchor: Option<Span<'src>>,
    anchor_reftext: Option<Span<'src>>,
    attrlist: Option<Attrlist<'src>>,
    is_checklist: bool,
    is_bibliography: bool,
}

impl<'src> ListBlock<'src> {
    /// Returns a document-order iterator over this list's direct child blocks
    /// (its list items).
    ///
    /// For the full subtree, or to search from a [`Block`] or [`Document`], use
    /// [`FindBlocks`](crate::blocks::FindBlocks).
    ///
    /// [`Document`]: crate::Document
    pub fn child_blocks(&'src self) -> ChildBlocks<'src> {
        ChildBlocks::from_slice(&self.items)
    }

    /// Returns the block's title as a mutable [`Content`], if the block has
    /// one.
    ///
    /// This narrow seam exists for the document-order title resolution pass
    /// (see `document::title_refs`), which installs the re-rendered title
    /// after resolving any cross-references embedded in it. All other access
    /// goes through the read-only [`IsBlock::title`] accessor.
    pub(crate) fn title_content_mut(&mut self) -> Option<&mut Content<'src>> {
        self.title.as_mut()
    }

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

        // A list carries the `bibliography` style in two ways, which differ in
        // scope (matching Asciidoctor):
        //
        // * An explicit `[bibliography]` attribute marks the list a bibliography
        //   regardless of its type (even an ordered list).
        // * A `bibliography` section implicitly marks each of its top-level *unordered*
        //   lists (only) a bibliography. A nested list never inherits the section
        //   style, so this is gated on `parent_list_markers` being empty; the list-type
        //   restriction is applied below, once the type is known.
        let own_style_bibliography = metadata
            .attrlist
            .as_ref()
            .and_then(|attrlist| attrlist.block_style())
            == Some("bibliography");
        let section_propagated_bibliography =
            parent_list_markers.is_empty() && parser.parsing_bibliography_section_body;

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
                                origin: None,
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

            // The bibliography anchor (`[[[id]]]`) is recognized in the principal
            // text of any item of an explicitly-styled bibliography list, or of an
            // unordered-list item when the style is inherited from the section.
            // Pass that context down so the item's inline substitution can detect
            // it.
            let item_is_bibliography = own_style_bibliography
                || (section_propagated_bibliography
                    && matches!(
                        this_item_marker,
                        ListItemMarker::Asterisks(_)
                            | ListItemMarker::Hyphen(_)
                            | ListItemMarker::Bullet(_)
                    ));

            let Some(list_item_mi) = ListItem::parse(
                &list_item_metadata,
                parent_list_markers,
                item_is_bibliography,
                parser,
                warnings,
            ) else {
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
            ListItemMarker::Callout(_) => ListType::Callout,

            ListItemMarker::DefinedTerm {
                term: _,
                marker: _,
                source: _,
            } => ListType::Description,
        };

        // A callout list annotates the callouts of a preceding verbatim block.
        // For each item (by position): an explicit `<N>` marker that doesn't
        // match the item's position is out of sequence, and an item position
        // with no callout registered while substituting the block has no
        // matching callout. Both mirror Asciidoctor's `parse_callout_list`
        // warnings. The list is then closed so the next block's callouts start
        // fresh.
        if type_ == ListType::Callout {
            for (index, item) in items.iter().enumerate() {
                let position = (index + 1) as u32;

                if let Some(marker_number) = item
                    .as_list_item()
                    .and_then(|li| li.list_item_marker().callout_number())
                    && marker_number != position
                {
                    warnings.push(Warning {
                        source: item.span(),
                        warning: WarningType::CalloutListItemOutOfSequence(
                            position as usize,
                            marker_number as usize,
                        ),
                        origin: None,
                    });
                }

                if !parser.callout_defined(position) {
                    warnings.push(Warning {
                        source: item.span(),
                        warning: WarningType::NoCalloutFound(position as usize),
                        origin: None,
                    });
                }
            }
            parser.close_callout_list();
        }

        // An unordered list is a checklist (i.e. task list) when at least one of
        // its items has checkbox syntax. This mirrors Asciidoctor, which sets the
        // `checklist` option on the list once any item carries a checkbox.
        let is_checklist = type_ == ListType::Unordered
            && items.iter().any(|item| {
                item.as_list_item()
                    .is_some_and(|li| li.checkbox().is_some())
            });

        // An explicit `[bibliography]` style applies to any list type; the style
        // inherited from a section applies only to unordered lists.
        let is_bibliography = own_style_bibliography
            || (section_propagated_bibliography && type_ == ListType::Unordered);

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
                is_checklist,
                is_bibliography,
            },
            after: next_item_source,
        })
    }

    /// Returns the type of this list.
    pub fn type_(&self) -> ListType {
        self.type_
    }

    /// Returns `true` if this list is a checklist (i.e. task list).
    ///
    /// An unordered list becomes a checklist when at least one of its items
    /// uses checkbox syntax (`[ ]`, `[x]`, or `[*]`). See
    /// [`ListItem::checkbox`].
    ///
    /// [`ListItem::checkbox`]: crate::blocks::ListItem::checkbox
    pub fn is_checklist(&self) -> bool {
        self.is_checklist
    }

    /// Returns `true` if this list carries the `bibliography` style.
    ///
    /// A list is a bibliography list when it is an unordered list that is
    /// either explicitly marked `[bibliography]` or appears as a top-level
    /// list within a section that carries the `bibliography` style (the
    /// section implicitly adds the style to each of its unordered lists).
    /// Each item of such a list may begin with a bibliography anchor
    /// (`[[[id]]]`).
    pub fn is_bibliography(&self) -> bool {
        self.is_bibliography
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
            ListItemMarker::Callout(_) => Some("arabic"),
            ListItemMarker::AlphaListLower(_) => Some("loweralpha"),
            ListItemMarker::AlphaListCapital(_) => Some("upperalpha"),
            ListItemMarker::RomanNumeralLower(_) => Some("lowerroman"),
            ListItemMarker::RomanNumeralUpper(_) => Some("upperroman"),
            _ => None,
        }
    }

    /// Returns the starting ordinal a converter should emit as the `start`
    /// attribute of an HTML `<ol>`, if any.
    ///
    /// An ordered list can begin at a value other than 1 in two ways (matching
    /// Asciidoctor):
    ///
    /// * an explicit `[start=N]` attribute, which takes precedence; or
    /// * the ordinal of an explicit first-item marker – for example `7.`
    ///   (arabic), `c.` (loweralpha, ⇒ 3), or `iv)` (lowerroman, ⇒ 4).
    ///
    /// The result is `None` whenever the start resolves to the default of 1 –
    /// whether from implicit markers (e.g. `.`), an explicit ordinal-1 marker
    /// (`1.`, `a.`, `i)`), or `[start=1]` – because a converter emits a bare
    /// `<ol>` in that case. It is likewise `None` for a list that is not
    /// ordered. So `start()` is `Some(n)` exactly when a converter must emit a
    /// non-default `start="n"`, mirroring the `ordinal != 1` guard in this
    /// crate's own reference renderer.
    pub fn start(&self) -> Option<i64> {
        if self.type_ != ListType::Ordered {
            return None;
        }

        // An explicit `[start=N]` attribute takes precedence; otherwise derive
        // the start from an explicit first-item marker.
        let resolved = self
            .attrlist
            .as_ref()
            .and_then(|attrlist| attrlist.named_attribute("start"))
            .and_then(|attr| attr.value().trim().parse::<i64>().ok())
            .or_else(|| {
                self.items
                    .first()
                    .and_then(|item| item.as_list_item())
                    .and_then(|li| li.list_item_marker().ordinal_value())
                    .map(i64::from)
            });

        // A start of 1 is the default, which a converter renders as a bare
        // `<ol>`, so it is reported as `None` rather than `Some(1)`.
        resolved.filter(|&n| n != 1)
    }
}

impl<'src> IsBlock<'src> for ListBlock<'src> {
    fn content_model(&self) -> ContentModel {
        ContentModel::Compound
    }

    fn raw_context(&self) -> CowStr<'src> {
        "list".into()
    }

    fn child_blocks_mut(&mut self) -> &mut [Block<'src>] {
        &mut self.items
    }

    fn title_source(&'src self) -> Option<Span<'src>> {
        self.title_source
    }

    fn title(&self) -> Option<&str> {
        self.title.as_ref().map(Content::rendered_str)
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
            .field("is_checklist", &self.is_checklist)
            .field("is_bibliography", &self.is_bibliography)
            .finish()
    }
}

/// Represents the type of a list.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
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

    /// A callout list provides annotations for lines in a preceding verbatim
    /// block. Its items are marked with `<1>`, `<2>`, … (or `<.>` for automatic
    /// numbering).
    Callout,
}

impl std::fmt::Debug for ListType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListType::Unordered => write!(f, "ListType::Unordered"),
            ListType::Ordered => write!(f, "ListType::Ordered"),
            ListType::Description => write!(f, "ListType::Description"),
            ListType::Callout => write!(f, "ListType::Callout"),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use crate::{
        blocks::{ContentModel, ListType, metadata::BlockMetadata},
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

    /// Like [`list_parse`], but also returns the warnings produced. Used for
    /// callout lists, which warn when an item has no matching callout in a
    /// preceding verbatim block.
    fn list_parse_with_warnings<'a>(
        source: &'a str,
    ) -> (
        Option<MatchedItem<'a, crate::blocks::ListBlock<'a>>>,
        Vec<Warning<'a>>,
    ) {
        let mut parser = crate::Parser::default();
        let mut warnings: Vec<Warning<'a>> = vec![];

        let metadata = BlockMetadata::parse(crate::Span::new(source), &mut parser).item;

        let result = crate::blocks::list::ListBlock::parse(&metadata, &mut parser, &mut warnings);

        (result, warnings)
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
                        caption: None,
                        number: None,
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

        let mut list_blocks = list.item.child_blocks();

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
                    caption: None,
                    number: None,
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

        let mut li_blocks = list_item.child_blocks();

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
                caption: None,
                number: None,
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
            "ListBlock {\n    type_: ListType::Unordered,\n    items: &[\n        Block::ListItem(\n            ListItem {\n                marker: ListItemMarker::Hyphen(\n                    Span {\n                        data: \"-\",\n                        line: 1,\n                        col: 1,\n                        offset: 0,\n                    },\n                ),\n                blocks: &[\n                    Block::Simple(\n                        SimpleBlock {\n                            content: Content {\n                                original: Span {\n                                    data: \"blah\",\n                                    line: 1,\n                                    col: 3,\n                                    offset: 2,\n                                },\n                                rendered: \"blah\",\n                            },\n                            source: Span {\n                                data: \"blah\",\n                                line: 1,\n                                col: 3,\n                                offset: 2,\n                            },\n                            style: SimpleBlockStyle::Paragraph,\n                            title_source: None,\n                            title: None,\n                            caption: None,\n                            number: None,\n                            anchor: None,\n                            anchor_reftext: None,\n                            attrlist: None,\n                        },\n                    ),\n                ],\n                source: Span {\n                    data: \"- blah\",\n                    line: 1,\n                    col: 1,\n                    offset: 0,\n                },\n                anchor: None,\n                anchor_reftext: None,\n                attrlist: None,\n                checkbox: None,\n            },\n        ),\n    ],\n    source: Span {\n        data: \"- blah\",\n        line: 1,\n        col: 1,\n        offset: 0,\n    },\n    title_source: None,\n    title: None,\n    anchor: None,\n    anchor_reftext: None,\n    attrlist: None,\n    is_checklist: false,\n    is_bibliography: false,\n}"
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

        assert_eq!(format!("{:#?}", ListType::Callout), "ListType::Callout");
    }

    #[test]
    fn callout_list() {
        // Parsed in isolation (no preceding verbatim block), so each item warns
        // that it has no matching callout.
        let (list, warnings) = list_parse_with_warnings("<1> First\n<2> Second\n");
        let list = list.unwrap();

        assert_eq!(list.item.type_(), ListType::Callout);
        assert_eq!(list.item.marker_style(), Some("arabic"));

        let items: Vec<_> = list.item.child_blocks().collect();
        assert_eq!(items.len(), 2);

        assert_eq!(
            items[0].child_blocks().next().unwrap().rendered_content(),
            Some("First")
        );
        assert_eq!(
            items[1].child_blocks().next().unwrap().rendered_content(),
            Some("Second")
        );

        let warning_types: Vec<_> = warnings.iter().map(|w| &w.warning).collect();
        assert_eq!(
            warning_types,
            vec![
                &WarningType::NoCalloutFound(1),
                &WarningType::NoCalloutFound(2),
            ]
        );
    }

    #[test]
    fn callout_list_auto_numbered() {
        // `<.>` markers form a single callout list.
        let (list, warnings) = list_parse_with_warnings("<.> First\n<.> Second\n<.> Third\n");
        let list = list.unwrap();

        assert_eq!(list.item.type_(), ListType::Callout);
        assert_eq!(list.item.child_blocks().count(), 3);

        // No preceding verbatim block defines these callouts.
        assert_eq!(warnings.len(), 3);
    }

    #[test]
    fn callout_list_marker_only_trailing_bracket_is_not_a_list() {
        // `1>` (trailing bracket only) is not a callout list marker.
        assert!(list_parse("1> Not a callout list item\n").is_none());
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
                                caption: None,
                                number: None,
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
                                        caption: None,
                                        number: None,
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
                            caption: None,
                            number: None,
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
                                caption: None,
                                number: None,
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
                                        caption: None,
                                        number: None,
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
                            caption: None,
                            number: None,
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
        assert_eq!(list.item.child_blocks().count(), 2);
        assert_eq!(list.item.type_(), ListType::Unordered);

        let mut outer_items = list.item.child_blocks();

        // First outer item should contain a nested ** list.
        let first_outer = outer_items.next().unwrap();
        let first_outer_blocks: Vec<_> = first_outer.child_blocks().collect();
        assert_eq!(first_outer_blocks.len(), 2); // SimpleBlock + ListBlock

        // The nested ** list should have one item.
        let nested_list = &first_outer_blocks[1];
        assert_eq!(nested_list.child_blocks().count(), 1);

        // That ** item should contain a nested *** list.
        let parent_item = nested_list.child_blocks().next().unwrap();
        let parent_blocks: Vec<_> = parent_item.child_blocks().collect();
        assert_eq!(parent_blocks.len(), 2); // SimpleBlock + ListBlock

        // The *** list should have one item.
        let innermost_list = &parent_blocks[1];
        assert_eq!(innermost_list.child_blocks().count(), 1);

        // The *** item should have only its principal text.
        let innermost_item = innermost_list.child_blocks().next().unwrap();
        assert_eq!(innermost_item.child_blocks().count(), 1);

        // Second outer item is "back to grandparent".
        let second_outer = outer_items.next().unwrap();
        assert_eq!(second_outer.child_blocks().count(), 1);
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
        let mut items = list.item.child_blocks();
        let item = items.next().unwrap();
        assert!(items.next().is_none());

        // The item should have two blocks: the principal text and the
        // continuation paragraph. The orphaned `.Title` should be discarded.
        let blocks: Vec<_> = item.child_blocks().collect();
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
                caption: None,
                number: None,
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
                caption: None,
                number: None,
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
        assert_eq!(mi.item.child_blocks().count(), 1);
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

    mod start {
        use super::list_parse;
        use crate::blocks::ListType;

        #[test]
        fn unordered_list_has_no_start() {
            let mi = list_parse("* one\n* two").unwrap();
            assert_eq!(mi.item.type_(), ListType::Unordered);
            assert_eq!(mi.item.start(), None);
        }

        #[test]
        fn implicit_ordered_marker_has_no_start() {
            // Implicit `.` markers with no `[start]` attribute default to 1,
            // reported as `None`.
            let mi = list_parse(". one\n. two").unwrap();
            assert_eq!(mi.item.type_(), ListType::Ordered);
            assert_eq!(mi.item.start(), None);
        }

        #[test]
        fn explicit_arabic_first_marker_sets_start() {
            let mi = list_parse("7. one\n8. two").unwrap();
            assert_eq!(mi.item.type_(), ListType::Ordered);
            assert_eq!(mi.item.start(), Some(7));
        }

        #[test]
        fn explicit_alpha_first_marker_sets_start() {
            // `c.` is the third letter, so the list starts at 3.
            let mi = list_parse("c. one\nd. two").unwrap();
            assert_eq!(mi.item.start(), Some(3));
        }

        #[test]
        fn explicit_ordinal_one_marker_defaults_to_none() {
            // An explicit ordinal-1 marker resolves to the default start of 1,
            // which a converter renders as a bare `<ol>`, so `start()` is
            // `None` rather than `Some(1)`.
            assert_eq!(list_parse("1. one\n2. two").unwrap().item.start(), None);
            assert_eq!(list_parse("a. one\nb. two").unwrap().item.start(), None);
        }

        #[test]
        fn start_attribute_takes_precedence() {
            let mi = list_parse("[start=5]\n. one\n. two").unwrap();
            assert_eq!(mi.item.type_(), ListType::Ordered);
            assert_eq!(mi.item.start(), Some(5));
        }

        #[test]
        fn start_attribute_of_one_is_none() {
            // `[start=1]` is the default too, so it is also reported as `None`.
            let mi = list_parse("[start=1]\n. one\n. two").unwrap();
            assert_eq!(mi.item.start(), None);
        }

        #[test]
        fn start_attribute_overrides_first_marker() {
            let mi = list_parse("[start=5]\n7. one\n8. two").unwrap();
            assert_eq!(mi.item.start(), Some(5));
        }

        #[test]
        fn non_numeric_start_attribute_falls_back_to_marker() {
            let mi = list_parse("[start=abc]\n7. one\n8. two").unwrap();
            assert_eq!(mi.item.start(), Some(7));
        }
    }
}
