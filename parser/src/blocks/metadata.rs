use std::ops::{RangeFrom, RangeTo};

use crate::{
    Parser, Span,
    attributes::{Attrlist, AttrlistContext},
    content::{Content, SubstitutionGroup},
    span::MatchedItem,
    warnings::{MatchAndWarnings, Warning, WarningType},
};

/// `BlockMetadata` represents the common elements that can precede any block
/// type (such as title and attribute list). It is used internally to track
/// those values before the specific block type is fully formed.
#[derive(Debug)]
pub(crate) struct BlockMetadata<'src> {
    /// The block's raw title, if any.
    pub(crate) title_source: Option<Span<'src>>,

    /// The block's rendered title, if any.
    pub(crate) title: Option<String>,

    /// The block's anchor, if any. The span does not include the opening or
    /// closing square brace pair, nor reftext if it exists.
    pub(crate) anchor: Option<Span<'src>>,

    /// The block anchor's reftext, if any. The span includes only the portion
    /// from the first comma to just inside the closing square brace pair.
    pub(crate) anchor_reftext: Option<Span<'src>>,

    /// The block's attribute list, if any.
    pub(crate) attrlist: Option<Attrlist<'src>>,

    /// The source span as understood when the block metadata was first
    /// encountered. Does not necessarily end at the end of the block.
    pub(crate) source: Span<'src>,

    /// The source span after reading the optional title and attribute list.
    /// This is the beginning of content for the specific block type.
    pub(crate) block_start: Span<'src>,
}

impl<'src> BlockMetadata<'src> {
    /// (For testing only) Parse the block metadata from a raw text constant.
    #[cfg(test)]
    pub(crate) fn new(data: &'src str) -> Self {
        let mut temp_parser = Parser::default();
        Self::parse(Span::new(data), &mut temp_parser).item
    }

    /// Parse the title and attribute list for a block, if any.
    pub(crate) fn parse(source: Span<'src>, parser: &mut Parser) -> MatchAndWarnings<'src, Self> {
        let mut warnings: Vec<Warning<'src>> = vec![];
        let source = source.discard_empty_lines();

        // Block metadata items (title, anchor, and attribute list) can appear in any
        // order. We loop through lines until we can't parse any more metadata
        // items.
        let mut title_source: Option<Span<'src>> = None;
        let mut anchor: Option<Span<'src>> = None;
        let mut reftext: Option<Span<'src>> = None;
        let mut attrlist: Option<Attrlist<'src>> = None;
        let mut block_start = source;

        loop {
            let original_block_start = block_start;

            // Try to parse a title.
            if title_source.is_none() {
                let maybe_title = block_start.take_normalized_line();
                if maybe_title.item.starts_with('.') && !maybe_title.item.starts_with("..") {
                    let title = maybe_title.item.discard(1);
                    if title.take_whitespace().item.is_empty() {
                        title_source = Some(title);
                        block_start = maybe_title.after;
                        continue;
                    }
                }
            }

            // Try to parse a block anchor.
            if anchor.is_none() {
                let mut anchor_maw = parse_maybe_block_anchor(block_start);

                // Collect any warnings from the anchor parsing (e.g., empty anchor).
                if !anchor_maw.warnings.is_empty() {
                    warnings.append(&mut anchor_maw.warnings);
                }

                if let Some(mi) = anchor_maw.item {
                    if let Some(comma_position) = mi.item.position(|c| c == ',')
                        && comma_position < mi.item.len() - 1
                    {
                        let anchor_span = mi.item.slice_to(RangeTo {
                            end: comma_position,
                        });
                        let reftext_span = mi.item.slice_from(RangeFrom {
                            start: comma_position + 1,
                        });

                        // Validate anchor name.
                        if anchor_span.is_xml_name() {
                            anchor = Some(anchor_span);
                            reftext = Some(reftext_span);
                            block_start = mi.after;
                        } else {
                            warnings.push(Warning {
                                source: anchor_span,
                                warning: WarningType::InvalidBlockAnchorName,
                            });
                        }
                    } else {
                        // Validate anchor name.
                        if mi.item.is_xml_name() {
                            anchor = Some(mi.item);
                            block_start = mi.after;
                        } else {
                            warnings.push(Warning {
                                source: mi.item,
                                warning: WarningType::InvalidBlockAnchorName,
                            });
                        }
                    }

                    if block_start != original_block_start {
                        continue;
                    }
                }
            }

            // Try to parse an attribute list. A block may be preceded by more
            // than one attribute list line (optionally straddling the title);
            // Asciidoctor merges them into a single set of attributes, with a
            // later line winning on conflict and otherwise accumulating.
            if let Some(MatchAndWarnings {
                item:
                    MatchedItem {
                        item: attrlist_item,
                        after: new_block_start,
                    },
                warnings: mut attrlist_warnings,
            }) = parse_maybe_attrlist_line(block_start, parser)
            {
                if !attrlist_warnings.is_empty() {
                    warnings.append(&mut attrlist_warnings);
                }

                match attrlist {
                    Some(ref mut existing) => existing.merge_block_attribute_line(attrlist_item),
                    None => attrlist = Some(attrlist_item),
                }
                block_start = new_block_start;
                continue;
            }

            // No more metadata items found.
            break;
        }

        let title = title_source.as_ref().map(|span| {
            let mut content = Content::from(*span);
            SubstitutionGroup::Normal.apply(&mut content, parser, None);
            content.rendered.into_string()
        });

        MatchAndWarnings {
            item: Self {
                title_source,
                title,
                anchor,
                anchor_reftext: reftext,
                attrlist,
                source,
                block_start,
            },
            warnings,
        }
    }

    /// Return `true` if title, anchor, and attrlist are all empty.
    pub(crate) fn is_empty(&self) -> bool {
        self.title.is_none() && self.anchor.is_none() && self.attrlist.is_none()
    }

    /// Return `true` if this block metadata has either the `discrete` or
    /// `float` block style.
    ///
    /// When used in the context of a section heading, this indicates that the
    /// heading should not mark the start of a new section.
    pub(crate) fn is_discrete(&self) -> bool {
        if let Some(ref attrlist) = self.attrlist
            && let Some(block_style) = attrlist.block_style()
        {
            block_style == "discrete" || block_style == "float"
        } else {
            false
        }
    }
}

fn parse_maybe_block_anchor(
    source: Span<'_>,
) -> MatchAndWarnings<'_, Option<MatchedItem<'_, Span<'_>>>> {
    if !source.starts_with("[[") {
        return MatchAndWarnings {
            item: None,
            warnings: vec![],
        };
    }

    let MatchedItem {
        item: line,
        after: block_start,
    } = source.take_normalized_line();

    if !line.ends_with("]]") {
        return MatchAndWarnings {
            item: None,
            warnings: vec![],
        };
    }

    // Drop opening and closing brace pairs now that we know they are there.
    let anchor_src = line.slice(2..line.len() - 2);
    if anchor_src.is_empty() {
        return MatchAndWarnings {
            item: None,
            warnings: vec![Warning {
                source: anchor_src,
                warning: WarningType::EmptyBlockAnchorName,
            }],
        };
    }

    MatchAndWarnings {
        item: Some(MatchedItem {
            item: anchor_src,
            after: block_start,
        }),
        warnings: vec![],
    }
}

fn parse_maybe_attrlist_line<'src>(
    source: Span<'src>,
    parser: &Parser,
) -> Option<MatchAndWarnings<'src, MatchedItem<'src, Attrlist<'src>>>> {
    let first_char = source.chars().next()?;
    if first_char != '[' {
        return None;
    }

    let MatchedItem {
        item: line,
        after: block_start,
    } = source.take_normalized_line();

    if !line.ends_with(']') {
        return None;
    }

    // Drop opening and closing braces now that we know they are there.
    let attrlist_src = line.slice(1..line.len() - 1);

    if attrlist_src.starts_with(' ')
        || attrlist_src.starts_with('\t')
        || (attrlist_src.starts_with('[') && attrlist_src.ends_with(']'))
    {
        return None;
    }

    let MatchAndWarnings {
        item: MatchedItem {
            item: attrlist,
            after: _,
        },
        warnings,
    } = Attrlist::parse(attrlist_src, parser, AttrlistContext::Block);

    Some(MatchAndWarnings {
        item: MatchedItem {
            item: attrlist,
            after: block_start,
        },
        warnings,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use crate::tests::prelude::*;

    #[test]
    fn metadata_order_title_anchor_attrlist() {
        let input = ".My Title\n[[my-anchor]]\n[role=\"example\"]\nContent\n";
        let metadata = super::BlockMetadata::new(input);

        assert_eq!(metadata.title.as_deref(), Some("My Title"));
        assert_eq!(
            metadata.anchor.unwrap(),
            Span {
                data: "my-anchor",
                line: 2,
                col: 3,
                offset: 12,
            }
        );
        assert!(metadata.attrlist.is_some());
    }

    #[test]
    fn metadata_order_anchor_title_attrlist() {
        let input = "[[another-anchor]]\n.Another Title\n[role=\"sidebar\"]\nContent\n";
        let metadata = super::BlockMetadata::new(input);

        assert_eq!(metadata.title.as_deref(), Some("Another Title"));
        assert_eq!(
            metadata.anchor.unwrap(),
            Span {
                data: "another-anchor",
                line: 1,
                col: 3,
                offset: 2,
            }
        );
        assert!(metadata.attrlist.is_some());
    }

    #[test]
    fn metadata_order_attrlist_title_anchor() {
        let input = "[role=\"note\"]\n.Third Title\n[[third-anchor]]\nContent\n";
        let metadata = super::BlockMetadata::new(input);

        assert_eq!(metadata.title.as_deref(), Some("Third Title"));
        assert_eq!(
            metadata.anchor.unwrap(),
            Span {
                data: "third-anchor",
                line: 3,
                col: 3,
                offset: 29,
            }
        );
        assert!(metadata.attrlist.is_some());
    }

    #[test]
    fn metadata_order_anchor_attrlist_title() {
        let input = "[[fourth-anchor]]\n[role=\"warning\"]\n.Fourth Title\nContent\n";
        let metadata = super::BlockMetadata::new(input);

        assert_eq!(metadata.title.as_deref(), Some("Fourth Title"));
        assert_eq!(
            metadata.anchor.unwrap(),
            Span {
                data: "fourth-anchor",
                line: 1,
                col: 3,
                offset: 2,
            }
        );
        assert!(metadata.attrlist.is_some());
    }

    #[test]
    fn metadata_order_title_attrlist_only() {
        let input = ".Just Title\n[role=\"tip\"]\nContent\n";
        let metadata = super::BlockMetadata::new(input);

        assert_eq!(metadata.title.as_deref(), Some("Just Title"));
        assert!(metadata.anchor.is_none());
        assert!(metadata.attrlist.is_some());
    }

    #[test]
    fn metadata_order_anchor_attrlist_only() {
        let input = "[[just-anchor]]\n[role=\"caution\"]\nContent\n";
        let metadata = super::BlockMetadata::new(input);

        assert!(metadata.title.is_none());
        assert_eq!(
            metadata.anchor.unwrap(),
            Span {
                data: "just-anchor",
                line: 1,
                col: 3,
                offset: 2,
            }
        );
        assert!(metadata.attrlist.is_some());
    }

    #[test]
    fn metadata_order_attrlist_anchor_only() {
        let input = "[role=\"important\"]\n[[attrlist-first]]\nContent\n";
        let metadata = super::BlockMetadata::new(input);

        assert!(metadata.title.is_none());
        assert_eq!(
            metadata.anchor.unwrap(),
            Span {
                data: "attrlist-first",
                line: 2,
                col: 3,
                offset: 21,
            }
        );
        assert!(metadata.attrlist.is_some());
    }

    #[test]
    fn title_does_not_extend_via_plus_syntax() {
        let doc: crate::Document<'_> =
            Parser::default().parse(".Title abc +\ndef\n****\nStuff > nonsense\n****");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "def",
                                line: 2,
                                col: 1,
                                offset: 13,
                            },
                            rendered: "def",
                        },
                        source: Span {
                            data: ".Title abc +\ndef",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: Some(Span {
                            data: "Title abc +",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },),
                        title: Some("Title abc +",),
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::CompoundDelimited(CompoundDelimitedBlock {
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "Stuff > nonsense",
                                    line: 4,
                                    col: 1,
                                    offset: 22,
                                },
                                rendered: "Stuff &gt; nonsense",
                            },
                            source: Span {
                                data: "Stuff > nonsense",
                                line: 4,
                                col: 1,
                                offset: 22,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        context: "sidebar",
                        source: Span {
                            data: "****\nStuff > nonsense\n****",
                            line: 3,
                            col: 1,
                            offset: 17,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: ".Title abc +\ndef\n****\nStuff > nonsense\n****",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([]),
                    reftext_to_id: HashMap::from([]),
                },
            }
        );
    }

    mod merge_attribute_lines {
        use crate::tests::prelude::*;

        #[test]
        fn two_non_conflicting_named() {
            let metadata =
                crate::blocks::metadata::BlockMetadata::new("[foo=bar]\n[baz=qux]\ncontent\n");

            let attrlist = metadata.attrlist.as_ref().unwrap();
            assert_eq!(attrlist.named_attribute("foo").unwrap().value(), "bar");
            assert_eq!(attrlist.named_attribute("baz").unwrap().value(), "qux");
        }

        #[test]
        fn conflicting_named_later_wins() {
            let metadata =
                crate::blocks::metadata::BlockMetadata::new("[foo=bar]\n[foo=qux]\ncontent\n");

            let attrlist = metadata.attrlist.as_ref().unwrap();
            assert_eq!(attrlist.named_attribute("foo").unwrap().value(), "qux");

            // Only one `foo` attribute should remain.
            assert_eq!(
                attrlist
                    .attributes()
                    .filter(|a| a.name() == Some("foo"))
                    .count(),
                1
            );
        }

        #[test]
        fn lines_straddling_a_title() {
            let metadata = crate::blocks::metadata::BlockMetadata::new(
                "[foo=bar]\n.My Title\n[baz=qux]\ncontent\n",
            );

            assert_eq!(metadata.title.as_deref(), Some("My Title"));

            let attrlist = metadata.attrlist.as_ref().unwrap();
            assert_eq!(attrlist.named_attribute("foo").unwrap().value(), "bar");
            assert_eq!(attrlist.named_attribute("baz").unwrap().value(), "qux");
        }

        #[test]
        fn combined_with_anchor() {
            let metadata = crate::blocks::metadata::BlockMetadata::new(
                "[foo=bar]\n[[my-anchor]]\n[baz=qux]\ncontent\n",
            );

            assert_eq!(
                metadata.anchor.unwrap(),
                Span {
                    data: "my-anchor",
                    line: 2,
                    col: 3,
                    offset: 12,
                }
            );

            let attrlist = metadata.attrlist.as_ref().unwrap();
            assert_eq!(attrlist.named_attribute("foo").unwrap().value(), "bar");
            assert_eq!(attrlist.named_attribute("baz").unwrap().value(), "qux");
        }

        #[test]
        fn block_style_later_wins() {
            let metadata =
                crate::blocks::metadata::BlockMetadata::new("[sidebar]\n[example]\ncontent\n");

            let attrlist = metadata.attrlist.as_ref().unwrap();
            assert_eq!(attrlist.block_style().unwrap(), "example");
        }

        #[test]
        fn shorthand_id_and_role_across_lines() {
            let metadata =
                crate::blocks::metadata::BlockMetadata::new("[#myid]\n[.myrole]\ncontent\n");

            let attrlist = metadata.attrlist.as_ref().unwrap();
            assert_eq!(attrlist.id().unwrap(), "myid");
            assert_eq!(attrlist.roles(), vec!["myrole"]);
        }

        #[test]
        fn shorthand_roles_accumulate() {
            let metadata =
                crate::blocks::metadata::BlockMetadata::new("[.role1]\n[.role2]\ncontent\n");

            let attrlist = metadata.attrlist.as_ref().unwrap();
            assert_eq!(attrlist.roles(), vec!["role1", "role2"]);
        }

        #[test]
        fn shorthand_id_later_wins() {
            let metadata = crate::blocks::metadata::BlockMetadata::new("[#id1]\n[#id2]\ncontent\n");

            let attrlist = metadata.attrlist.as_ref().unwrap();
            assert_eq!(attrlist.id().unwrap(), "id2");
        }

        #[test]
        fn shorthand_options_accumulate() {
            let metadata =
                crate::blocks::metadata::BlockMetadata::new("[%opt1]\n[%opt2]\ncontent\n");

            let attrlist = metadata.attrlist.as_ref().unwrap();
            assert_eq!(attrlist.options(), vec!["opt1", "opt2"]);
        }

        #[test]
        fn second_positional_later_wins() {
            let metadata = crate::blocks::metadata::BlockMetadata::new(
                "[quote,Author1]\n[quote,Author2]\ncontent\n",
            );

            let attrlist = metadata.attrlist.as_ref().unwrap();
            assert_eq!(attrlist.block_style().unwrap(), "quote");
            assert_eq!(attrlist.nth_attribute(2).unwrap().value(), "Author2");
        }

        #[test]
        fn first_positional_only_on_later_line() {
            let metadata =
                crate::blocks::metadata::BlockMetadata::new("[foo=bar]\n[sidebar]\ncontent\n");

            let attrlist = metadata.attrlist.as_ref().unwrap();
            assert_eq!(attrlist.block_style().unwrap(), "sidebar");
            assert_eq!(attrlist.named_attribute("foo").unwrap().value(), "bar");
        }

        #[test]
        fn extra_positional_only_on_later_line() {
            let metadata =
                crate::blocks::metadata::BlockMetadata::new("[quote]\n[quote,Author]\ncontent\n");

            let attrlist = metadata.attrlist.as_ref().unwrap();
            assert_eq!(attrlist.block_style().unwrap(), "quote");
            assert_eq!(attrlist.nth_attribute(2).unwrap().value(), "Author");
        }

        #[test]
        fn three_lines_mixed_with_title() {
            let metadata = crate::blocks::metadata::BlockMetadata::new(
                "[#id1]\n.Title\n[.r1.r2]\n[foo=bar]\ncontent\n",
            );

            assert_eq!(metadata.title.as_deref(), Some("Title"));

            let attrlist = metadata.attrlist.as_ref().unwrap();
            assert_eq!(attrlist.id().unwrap(), "id1");
            assert_eq!(attrlist.roles(), vec!["r1", "r2"]);
            assert_eq!(attrlist.named_attribute("foo").unwrap().value(), "bar");
        }
    }
}
