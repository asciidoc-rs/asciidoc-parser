//! This module contains tests related to each of the cases of the `Block` enum.
//! Many of these tests are repeated from the tests for the individual block
//! types.

#![allow(clippy::panic)]
#![allow(clippy::unwrap_used)]

mod r#break;
mod compound_delimited;
mod media;
mod raw_delimited;
mod section;
mod simple;

mod content_model {
    use pretty_assertions_sorted::assert_eq;

    use crate::blocks::ContentModel;

    #[test]
    fn impl_copy() {
        // Silly test to mark the #[derive(...)] line as covered.
        let c1 = ContentModel::Simple;
        let c2 = c1;
        assert_eq!(c1, c2);
    }
}

mod impl_debug {
    use crate::{Parser, blocks::Block, span::Span};

    #[test]
    fn simple() {
        let mut parser = Parser::default();
        let mi = Block::parse(Span::new("This is a simple paragraph."), &mut parser)
            .unwrap_if_no_warnings()
            .unwrap();

        let debug_output = format!("{:?}", mi.item);
        assert!(debug_output.starts_with("Block::Simple"));
    }

    #[test]
    fn media() {
        let mut parser = Parser::default();
        let mi = Block::parse(Span::new("image::example.png[]"), &mut parser)
            .unwrap_if_no_warnings()
            .unwrap();

        let debug_output = format!("{:?}", mi.item);
        assert!(debug_output.starts_with("Block::Media"));
    }

    #[test]
    fn section() {
        let mut parser = Parser::default();
        let mi = Block::parse(Span::new("== Section Title\n"), &mut parser)
            .unwrap_if_no_warnings()
            .unwrap();

        let debug_output = format!("{:?}", mi.item);
        assert!(debug_output.starts_with("Block::Section"));
    }

    #[test]
    fn raw_delimited() {
        let mut parser = Parser::default();
        let mi = Block::parse(Span::new("----\ncode\n----"), &mut parser)
            .unwrap_if_no_warnings()
            .unwrap();

        let debug_output = format!("{:?}", mi.item);
        assert!(debug_output.starts_with("Block::RawDelimited"));
    }

    #[test]
    fn compound_delimited() {
        let mut parser = Parser::default();
        let mi = Block::parse(Span::new("****\nSidebar content\n****"), &mut parser)
            .unwrap_if_no_warnings()
            .unwrap();

        let debug_output = format!("{:?}", mi.item);
        assert!(debug_output.starts_with("Block::CompoundDelimited"));
    }

    #[test]
    fn document_attribute() {
        let mut parser = Parser::default();
        let mi = Block::parse(Span::new(":author: John Doe"), &mut parser)
            .unwrap_if_no_warnings()
            .unwrap();

        let debug_output = format!("{:?}", mi.item);
        assert!(debug_output.starts_with("Block::DocumentAttribute"));
    }

    #[test]
    fn r#break() {
        let mut parser = Parser::default();
        let mi = Block::parse(Span::new("'''"), &mut parser)
            .unwrap_if_no_warnings()
            .unwrap();

        let debug_output = format!("{:?}", mi.item);
        assert!(debug_output.starts_with("Block::Break"));
    }
}

mod error_cases {
    use std::ops::Deref;

    use pretty_assertions_sorted::assert_eq;

    use crate::{
        Parser,
        blocks::{ContentModel, IsBlock, ListType, SimpleBlockStyle, metadata::BlockMetadata},
        content::SubstitutionGroup,
        span::HasSpan,
        tests::prelude::*,
        warnings::{MatchAndWarnings, WarningType},
    };

    #[test]
    fn missing_block_after_title_line() {
        let mut parser = Parser::default();
        let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

        let mi = crate::blocks::SectionBlock::parse(
            &BlockMetadata::new("=== Section Title\n\nabc\n\n.ancestor section== Section 2\n\ndef"),
            &mut parser,
            &mut warnings,
        )
        .unwrap();

        let item = &mi.item;

        assert_eq!(item.content_model(), ContentModel::Compound);
        assert_eq!(item.raw_context().deref(), "section");
        assert_eq!(item.resolved_context().deref(), "section");
        assert!(item.declared_style().is_none());
        assert_eq!(item.id().unwrap(), "_section_title");
        assert!(item.roles().is_empty());
        assert!(item.options().is_empty());
        assert!(item.title_source().is_none());
        assert!(item.title().is_none());
        assert!(item.anchor().is_none());
        assert!(item.anchor_reftext().is_none());
        assert!(item.attrlist().is_none());
        assert_eq!(item.substitution_group(), SubstitutionGroup::Normal);

        assert_eq!(
            mi.item,
            SectionBlock {
                level: 2,
                section_title: Content {
                    original: Span {
                        data: "Section Title",
                        line: 1,
                        col: 5,
                        offset: 4,
                    },
                    rendered: "Section Title",
                },
                blocks: &[
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "abc",
                                line: 3,
                                col: 1,
                                offset: 19,
                            },
                            rendered: "abc",
                        },
                        source: Span {
                            data: "abc",
                            line: 3,
                            col: 1,
                            offset: 19,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    }),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: ".ancestor section== Section 2",
                                line: 5,
                                col: 1,
                                offset: 24,
                            },
                            rendered: ".ancestor section== Section 2",
                        },
                        source: Span {
                            data: ".ancestor section== Section 2",
                            line: 5,
                            col: 1,
                            offset: 24,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "def",
                                line: 7,
                                col: 1,
                                offset: 55,
                            },
                            rendered: "def",
                        },
                        source: Span {
                            data: "def",
                            line: 7,
                            col: 1,
                            offset: 55,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    // TO DO: Fix bug that includes blank lines.
                    data: "=== Section Title\n\nabc\n\n.ancestor section== Section 2\n\ndef",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
                section_type: SectionType::Normal,
                section_id: Some("_section_title"),
                section_number: None,
            }
        );

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 7,
                col: 4,
                offset: 58
            }
        );

        assert_eq!(
            warnings,
            vec![Warning {
                source: Span {
                    data: ".ancestor section== Section 2\n\ndef",
                    line: 5,
                    col: 1,
                    offset: 24,
                },
                warning: WarningType::MissingBlockAfterTitleOrAttributeList,
            },]
        );
    }

    #[test]
    fn missing_close_brace_on_attrlist() {
        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(
            crate::Span::new("[incomplete attrlist\n=== Section Title (except it isn't)\n\nabc\n"),
            &mut parser,
        )
        .unwrap_if_no_warnings()
        .unwrap();

        assert_eq!(mi.item.content_model(), ContentModel::Simple);
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
            mi.item,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "[incomplete attrlist\n=== Section Title (except it isn't)",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "[incomplete attrlist\n=== Section Title (except it isn&#8217;t)",
                },
                source: Span {
                    data: "[incomplete attrlist\n=== Section Title (except it isn't)",
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
            },)
        );

        assert_eq!(
            mi.after,
            Span {
                data: "abc\n",
                line: 4,
                col: 1,
                offset: 58
            }
        );
    }

    #[test]
    fn attrlist_warning_carried_forward() {
        let mut parser = Parser::default();

        let MatchAndWarnings { item: mi, warnings } = crate::blocks::Block::parse(
            crate::Span::new(
                "[alt=\"Sunset\"width=300]\n=== Section Title (except it isn't)\n\nabc\n",
            ),
            &mut parser,
        );

        let mi = mi.unwrap();

        assert_eq!(mi.item.content_model(), ContentModel::Compound);
        assert_eq!(mi.item.raw_context().deref(), "section");
        assert_eq!(mi.item.resolved_context().deref(), "section");
        assert!(mi.item.declared_style().is_none());
        assert!(mi.item.id().is_none());
        assert!(mi.item.roles().is_empty());
        assert!(mi.item.options().is_empty());
        assert!(mi.item.title_source().is_none());
        assert!(mi.item.title().is_none());
        assert!(mi.item.anchor().is_none());
        assert!(mi.item.anchor_reftext().is_none());
        assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

        assert_eq!(
            mi.item.attrlist().unwrap(),
            Attrlist {
                attributes: &[ElementAttribute {
                    name: Some("alt"),
                    shorthand_items: &[],
                    value: "Sunset"
                },],
                anchor: None,
                source: Span {
                    data: "alt=\"Sunset\"width=300",
                    line: 1,
                    col: 2,
                    offset: 1,
                },
            }
        );

        assert_eq!(
            mi.item,
            Block::Section(SectionBlock {
                level: 2,
                section_title: Content {
                    original: Span {
                        data: "Section Title (except it isn't)",
                        line: 2,
                        col: 5,
                        offset: 28,
                    },
                    rendered: "Section Title (except it isn&#8217;t)",
                },
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "abc",
                            line: 4,
                            col: 1,
                            offset: 61,
                        },
                        rendered: "abc",
                    },
                    source: Span {
                        data: "abc",
                        line: 4,
                        col: 1,
                        offset: 61,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: "[alt=\"Sunset\"width=300]\n=== Section Title (except it isn't)\n\nabc",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: Some(Attrlist {
                    attributes: &[ElementAttribute {
                        name: Some("alt"),
                        shorthand_items: &[],
                        value: "Sunset"
                    },],
                    anchor: None,
                    source: Span {
                        data: "alt=\"Sunset\"width=300",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
                section_type: SectionType::Normal,
                section_id: Some("_section_title_except_it_isnt"),
                section_number: None,
            },)
        );

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 5,
                col: 1,
                offset: 65
            }
        );

        assert_eq!(
            warnings,
            vec![Warning {
                source: Span {
                    data: "alt=\"Sunset\"width=300",
                    line: 1,
                    col: 2,
                    offset: 1,
                },
                warning: WarningType::MissingCommaAfterQuotedAttributeValue,
            },]
        );
    }

    #[test]
    fn list_item_not_title() {
        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(crate::Span::new(". abc\ndef"), &mut parser)
            .unwrap_if_no_warnings()
            .unwrap();

        assert_eq!(
            mi.item,
            Block::List(ListBlock {
                type_: ListType::Ordered,
                items: &[Block::ListItem(ListItem {
                    marker: ListItemMarker::Dots(Span {
                        data: ".",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },),
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "abc\ndef",
                                line: 1,
                                col: 3,
                                offset: 2,
                            },
                            rendered: "abc\ndef",
                        },
                        source: Span {
                            data: "abc\ndef",
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
                        data: ". abc\ndef",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: ". abc\ndef",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },)
        );

        assert_eq!(
            mi.item.span(),
            Span {
                data: ". abc\ndef",
                line: 1,
                col: 1,
                offset: 0,
            }
        );
    }

    #[test]
    fn duplicate_id_warning() {
        let mut parser = Parser::default();

        let doc = parser.parse("[#this_one]\nBlah\n\n[#this_one]\nAgain");

        let mut warnings = doc.warnings();

        assert_eq!(
            warnings.next().unwrap(),
            Warning {
                source: Span {
                    data: "[#this_one]\nAgain",
                    line: 4,
                    col: 1,
                    offset: 18,
                },
                warning: WarningType::DuplicateId("this_one".to_owned(),),
            }
        );

        assert!(warnings.next().is_none());
    }
}
