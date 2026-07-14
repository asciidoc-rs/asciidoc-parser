// Adapted from Asciidoctor's paragraphs test suite, found in
// https://github.com/asciidoctor/asciidoctor/blob/main/test/paragraphs_test.rb.
//
// IMPORTANT: In porting this, I've disregarded compatibility mode (stated
// limitation of `asciidoc-parser` crate) and alternate (non-HTML) back ends.

mod normal {
    use crate::{document::RefType, tests::prelude::*};

    #[test]
    fn should_treat_plain_text_separated_by_blank_lines_as_paragraphs() {
        let doc =
            Parser::default().parse("Plain text for the win!\n\nYep. Text. Plain and simple.");

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
                                data: "Plain text for the win!",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "Plain text for the win!",
                        },
                        source: Span {
                            data: "Plain text for the win!",
                            line: 1,
                            col: 1,
                            offset: 0,
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
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "Yep. Text. Plain and simple.",
                                line: 3,
                                col: 1,
                                offset: 25,
                            },
                            rendered: "Yep. Text. Plain and simple.",
                        },
                        source: Span {
                            data: "Yep. Text. Plain and simple.",
                            line: 3,
                            col: 1,
                            offset: 25,
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
                ],
                source: Span {
                    data: "Plain text for the win!\n\nYep. Text. Plain and simple.",
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

    #[test]
    fn should_associate_block_title_with_paragraph() {
        let doc = Parser::default().parse(".Titled\nParagraph.\n\nWinning.");

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
                                data: "Paragraph.",
                                line: 2,
                                col: 1,
                                offset: 8,
                            },
                            rendered: "Paragraph.",
                        },
                        source: Span {
                            data: ".Titled\nParagraph.",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: Some(Span {
                            data: "Titled",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },),
                        title: Some("Titled",),
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "Winning.",
                                line: 4,
                                col: 1,
                                offset: 20,
                            },
                            rendered: "Winning.",
                        },
                        source: Span {
                            data: "Winning.",
                            line: 4,
                            col: 1,
                            offset: 20,
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
                ],
                source: Span {
                    data: ".Titled\nParagraph.\n\nWinning.",
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

    #[test]
    fn no_duplicate_block_before_next_section() {
        let doc = Parser::default().parse("= Title\n\nPreamble\n\n== First Section\n\nParagraph 1\n\nParagraph 2\n\n== Second Section\n\nLast words");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: Some(Span {
                        data: "Title",
                        line: 1,
                        col: 3,
                        offset: 2,
                    },),
                    title: Some("Title",),
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "= Title",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[
                    Block::Preamble(Preamble {
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "Preamble",
                                    line: 3,
                                    col: 1,
                                    offset: 9,
                                },
                                rendered: "Preamble",
                            },
                            source: Span {
                                data: "Preamble",
                                line: 3,
                                col: 1,
                                offset: 9,
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
                            data: "Preamble",
                            line: 3,
                            col: 1,
                            offset: 9,
                        },
                    },),
                    Block::Section(SectionBlock {
                        level: 1,
                        section_title: Content {
                            original: Span {
                                data: "First Section",
                                line: 5,
                                col: 4,
                                offset: 22,
                            },
                            rendered: "First Section",
                        },
                        blocks: &[
                            Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Paragraph 1",
                                        line: 7,
                                        col: 1,
                                        offset: 37,
                                    },
                                    rendered: "Paragraph 1",
                                },
                                source: Span {
                                    data: "Paragraph 1",
                                    line: 7,
                                    col: 1,
                                    offset: 37,
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
                            Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Paragraph 2",
                                        line: 9,
                                        col: 1,
                                        offset: 50,
                                    },
                                    rendered: "Paragraph 2",
                                },
                                source: Span {
                                    data: "Paragraph 2",
                                    line: 9,
                                    col: 1,
                                    offset: 50,
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
                        ],
                        source: Span {
                            data: "== First Section\n\nParagraph 1\n\nParagraph 2",
                            line: 5,
                            col: 1,
                            offset: 19,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                        section_type: SectionType::Normal,
                        section_id: Some("_first_section",),
                        caption: None,
                        section_number: None,
                    },),
                    Block::Section(SectionBlock {
                        level: 1,
                        section_title: Content {
                            original: Span {
                                data: "Second Section",
                                line: 11,
                                col: 4,
                                offset: 66,
                            },
                            rendered: "Second Section",
                        },
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "Last words",
                                    line: 13,
                                    col: 1,
                                    offset: 82,
                                },
                                rendered: "Last words",
                            },
                            source: Span {
                                data: "Last words",
                                line: 13,
                                col: 1,
                                offset: 82,
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
                            data: "== Second Section\n\nLast words",
                            line: 11,
                            col: 1,
                            offset: 63,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                        section_type: SectionType::Normal,
                        section_id: Some("_second_section",),
                        caption: None,
                        section_number: None,
                    },),
                ],
                source: Span {
                    data: "= Title\n\nPreamble\n\n== First Section\n\nParagraph 1\n\nParagraph 2\n\n== Second Section\n\nLast words",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([
                        (
                            "_first_section",
                            RefEntry {
                                id: "_first_section",
                                reftext: Some("First Section",),
                                ref_type: RefType::Section,
                            },
                        ),
                        (
                            "_second_section",
                            RefEntry {
                                id: "_second_section",
                                reftext: Some("Second Section",),
                                ref_type: RefType::Section,
                            },
                        ),
                    ]),
                    reftext_to_id: HashMap::from([
                        ("First Section", "_first_section",),
                        ("Second Section", "_second_section",),
                    ]),
                },
            }
        );
    }

    #[test]
    fn does_not_treat_wrapped_line_as_a_list_item() {
        let doc = Parser::default().parse("paragraph\n. wrapped line");

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
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "paragraph\n. wrapped line",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "paragraph\n. wrapped line",
                    },
                    source: Span {
                        data: "paragraph\n. wrapped line",
                        line: 1,
                        col: 1,
                        offset: 0,
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
                    data: "paragraph\n. wrapped line",
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

    #[test]
    fn does_not_treat_wrapped_line_as_a_block_title() {
        let doc = Parser::default().parse("paragraph\n.wrapped line");

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
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "paragraph\n.wrapped line",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "paragraph\n.wrapped line",
                    },
                    source: Span {
                        data: "paragraph\n.wrapped line",
                        line: 1,
                        col: 1,
                        offset: 0,
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
                    data: "paragraph\n.wrapped line",
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

    #[test]
    fn interprets_normal_paragraph_style_as_normal_paragraph() {
        let doc = Parser::default().parse("[normal]\nNormal paragraph.\nNothing special.");

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
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "Normal paragraph.\nNothing special.",
                            line: 2,
                            col: 1,
                            offset: 9,
                        },
                        rendered: "Normal paragraph.\nNothing special.",
                    },
                    source: Span {
                        data: "[normal]\nNormal paragraph.\nNothing special.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: None,
                            value: "normal",
                            shorthand_items: &["normal",],
                        },],
                        anchor: None,
                        source: Span {
                            data: "normal",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[normal]\nNormal paragraph.\nNothing special.",
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

    #[test]
    fn removes_indentation_from_literal_paragraph_marked_as_normal() {
        let doc = Parser::default()
            .parse("[normal]\n Normal paragraph.\n  Nothing special.\n Last line.");

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
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: " Normal paragraph.\n  Nothing special.\n Last line.",
                            line: 2,
                            col: 1,
                            offset: 9,
                        },
                        rendered: "Normal paragraph.\n Nothing special.\nLast line.",
                    },
                    source: Span {
                        data: "[normal]\n Normal paragraph.\n  Nothing special.\n Last line.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: None,
                            value: "normal",
                            shorthand_items: &["normal"],
                        },],
                        anchor: None,
                        source: Span {
                            data: "normal",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[normal]\n Normal paragraph.\n  Nothing special.\n Last line.",
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

    #[test]
    fn normal_paragraph_terminates_at_block_attribute_list() {
        let doc = Parser::default().parse("normal text\n[literal]\nliteral text");

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
                                data: "normal text",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "normal text",
                        },
                        source: Span {
                            data: "normal text",
                            line: 1,
                            col: 1,
                            offset: 0,
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
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "literal text",
                                line: 3,
                                col: 1,
                                offset: 22,
                            },
                            rendered: "literal text",
                        },
                        source: Span {
                            data: "[literal]\nliteral text",
                            line: 2,
                            col: 1,
                            offset: 12,
                        },
                        style: SimpleBlockStyle::Literal,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: Some(Attrlist {
                            attributes: &[ElementAttribute {
                                name: None,
                                value: "literal",
                                shorthand_items: &["literal"],
                            },],
                            anchor: None,
                            source: Span {
                                data: "literal",
                                line: 2,
                                col: 2,
                                offset: 13,
                            },
                        },),
                    },),
                ],
                source: Span {
                    data: "normal text\n[literal]\nliteral text",
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

    #[test]
    fn normal_paragraph_terminates_at_block_delimiter() {
        let doc = Parser::default().parse("normal text\n--\ntext in open block\n--");

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
                                data: "normal text",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "normal text",
                        },
                        source: Span {
                            data: "normal text",
                            line: 1,
                            col: 1,
                            offset: 0,
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
                    Block::CompoundDelimited(CompoundDelimitedBlock {
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "text in open block",
                                    line: 3,
                                    col: 1,
                                    offset: 15,
                                },
                                rendered: "text in open block",
                            },
                            source: Span {
                                data: "text in open block",
                                line: 3,
                                col: 1,
                                offset: 15,
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
                        context: "open",
                        source: Span {
                            data: "--\ntext in open block\n--",
                            line: 2,
                            col: 1,
                            offset: 12,
                        },
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: "normal text\n--\ntext in open block\n--",
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

    #[test]
    fn normal_paragraph_terminates_at_list_continuation() {
        let doc = Parser::default().parse("normal text\n+");

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
                                data: "normal text",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "normal text",
                        },
                        source: Span {
                            data: "normal text",
                            line: 1,
                            col: 1,
                            offset: 0,
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
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "+",
                                line: 2,
                                col: 1,
                                offset: 12,
                            },
                            rendered: "+",
                        },
                        source: Span {
                            data: "+",
                            line: 2,
                            col: 1,
                            offset: 12,
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
                ],
                source: Span {
                    data: "normal text\n+",
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

    #[test]
    fn normal_style_turns_literal_paragraph_into_normal_paragraph() {
        let doc =
            Parser::default().parse("[normal]\n normal paragraph,\n despite the leading indent");

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
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: " normal paragraph,\n despite the leading indent",
                            line: 2,
                            col: 1,
                            offset: 9,
                        },
                        rendered: "normal paragraph,\ndespite the leading indent",
                    },
                    source: Span {
                        data: "[normal]\n normal paragraph,\n despite the leading indent",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: None,
                            value: "normal",
                            shorthand_items: &["normal"],
                        },],
                        anchor: None,
                        source: Span {
                            data: "normal",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[normal]\n normal paragraph,\n despite the leading indent",
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

    // No planned support for DocBook output, so these tests were not ported from
    // asciidoctor_rb:
    //
    // * test 'automatically promotes index terms in DocBook output if
    //   indexterm-promotion-option is set'
    // * test 'does not automatically promote index terms in DocBook output if
    //   indexterm-promotion-option is not set'

    #[test]
    fn normal_paragraph_should_honor_explicit_subs_list() {
        let doc = Parser::default().parse("[subs=\"specialcharacters\"]\n*<Hey Jude>*");

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
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "*<Hey Jude>*",
                            line: 2,
                            col: 1,
                            offset: 27,
                        },
                        rendered: "*&lt;Hey Jude&gt;*",
                    },
                    source: Span {
                        data: "[subs=\"specialcharacters\"]\n*<Hey Jude>*",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: Some("subs",),
                            value: "specialcharacters",
                            shorthand_items: &[],
                        },],
                        anchor: None,
                        source: Span {
                            data: "subs=\"specialcharacters\"",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[subs=\"specialcharacters\"]\n*<Hey Jude>*",
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

    #[test]
    fn normal_paragraph_should_honor_specialchars_shorthand() {
        let doc = Parser::default().parse("[subs=\"specialchars\"]\n*<Hey Jude>*");

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
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "*<Hey Jude>*",
                            line: 2,
                            col: 1,
                            offset: 22,
                        },
                        rendered: "*&lt;Hey Jude&gt;*",
                    },
                    source: Span {
                        data: "[subs=\"specialchars\"]\n*<Hey Jude>*",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: Some("subs",),
                            value: "specialchars",
                            shorthand_items: &[],
                        },],
                        anchor: None,
                        source: Span {
                            data: "subs=\"specialchars\"",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[subs=\"specialchars\"]\n*<Hey Jude>*",
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

    #[test]
    fn should_add_a_hardbreak_at_end_of_each_line_when_hardbreaks_option_is_set() {
        let doc = Parser::default().parse("[%hardbreaks]\nread\nmy\nlips");

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
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "read\nmy\nlips",
                            line: 2,
                            col: 1,
                            offset: 14,
                        },
                        rendered: "read<br>\nmy<br>\nlips",
                    },
                    source: Span {
                        data: "[%hardbreaks]\nread\nmy\nlips",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: None,
                            value: "%hardbreaks",
                            shorthand_items: &["%hardbreaks"],
                        },],
                        anchor: None,
                        source: Span {
                            data: "%hardbreaks",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[%hardbreaks]\nread\nmy\nlips",
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

    #[test]
    fn should_be_able_to_toggle_hardbreaks_by_setting_hardbreaks_option_on_document() {
        // NOTE: I substituted different test material in this test.
        // See https://github.com/asciidoctor/asciidoctor/issues/4818 for why.

        let doc = Parser::default()
            .parse(":hardbreaks-option:\n\nmake\nit\nso\n\n:!hardbreaks:\n\nroll\nit\nback");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[Attribute {
                        name: Span {
                            data: "hardbreaks-option",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                        value_source: None,
                        value: InterpretedValue::Set,
                        source: Span {
                            data: ":hardbreaks-option:",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: ":hardbreaks-option:",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "make\nit\nso",
                                line: 3,
                                col: 1,
                                offset: 21,
                            },
                            rendered: "make<br>\nit<br>\nso",
                        },
                        source: Span {
                            data: "make\nit\nso",
                            line: 3,
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
                    },),
                    Block::DocumentAttribute(Attribute {
                        name: Span {
                            data: "hardbreaks",
                            line: 7,
                            col: 3,
                            offset: 35,
                        },
                        value_source: None,
                        value: InterpretedValue::Unset,
                        source: Span {
                            data: ":!hardbreaks:",
                            line: 7,
                            col: 1,
                            offset: 33,
                        },
                    },),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "roll\nit\nback",
                                line: 9,
                                col: 1,
                                offset: 48,
                            },
                            rendered: "roll\nit\nback",
                        },
                        source: Span {
                            data: "roll\nit\nback",
                            line: 9,
                            col: 1,
                            offset: 48,
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
                ],
                source: Span {
                    data: ":hardbreaks-option:\n\nmake\nit\nso\n\n:!hardbreaks:\n\nroll\nit\nback",
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
}

mod literal {
    use crate::tests::prelude::*;

    #[test]
    fn single_line_literal_paragraphs() {
        let doc =
            Parser::default().parse("you know what?\n\n LITERALS\n\n ARE LITERALLY\n\n AWESOME!");

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
                                data: "you know what?",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "you know what?",
                        },
                        source: Span {
                            data: "you know what?",
                            line: 1,
                            col: 1,
                            offset: 0,
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
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: " LITERALS",
                                line: 3,
                                col: 1,
                                offset: 16,
                            },
                            rendered: "LITERALS",
                        },
                        source: Span {
                            data: " LITERALS",
                            line: 3,
                            col: 1,
                            offset: 16,
                        },
                        style: SimpleBlockStyle::Literal,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: " ARE LITERALLY",
                                line: 5,
                                col: 1,
                                offset: 27,
                            },
                            rendered: "ARE LITERALLY",
                        },
                        source: Span {
                            data: " ARE LITERALLY",
                            line: 5,
                            col: 1,
                            offset: 27,
                        },
                        style: SimpleBlockStyle::Literal,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: " AWESOME!",
                                line: 7,
                                col: 1,
                                offset: 43,
                            },
                            rendered: "AWESOME!",
                        },
                        source: Span {
                            data: " AWESOME!",
                            line: 7,
                            col: 1,
                            offset: 43,
                        },
                        style: SimpleBlockStyle::Literal,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: "you know what?\n\n LITERALS\n\n ARE LITERALLY\n\n AWESOME!",
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

    #[test]
    fn multi_line_literal_paragraphs() {
        let doc =
            Parser::default().parse("Install instructions:\n\n yum install ruby rubygems\n gem install asciidoctor\n\nYou're good to go!");

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
                                data: "Install instructions:",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "Install instructions:",
                        },
                        source: Span {
                            data: "Install instructions:",
                            line: 1,
                            col: 1,
                            offset: 0,
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
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: " yum install ruby rubygems\n gem install asciidoctor",
                                line: 3,
                                col: 1,
                                offset: 23,
                            },
                            rendered: "yum install ruby rubygems\ngem install asciidoctor",
                        },
                        source: Span {
                            data: " yum install ruby rubygems\n gem install asciidoctor",
                            line: 3,
                            col: 1,
                            offset: 23,
                        },
                        style: SimpleBlockStyle::Literal,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "You're good to go!",
                                line: 6,
                                col: 1,
                                offset: 76,
                            },
                            rendered: "You&#8217;re good to go!",
                        },
                        source: Span {
                            data: "You're good to go!",
                            line: 6,
                            col: 1,
                            offset: 76,
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
                ],
                source: Span {
                    data: "Install instructions:\n\n yum install ruby rubygems\n gem install asciidoctor\n\nYou're good to go!",
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

    #[test]
    fn literal_paragraph() {
        let doc = Parser::default().parse("[literal]\nthis text is literally literal");

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
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "this text is literally literal",
                            line: 2,
                            col: 1,
                            offset: 10,
                        },
                        rendered: "this text is literally literal",
                    },
                    source: Span {
                        data: "[literal]\nthis text is literally literal",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Literal,
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: None,
                            value: "literal",
                            shorthand_items: &["literal"],
                        },],
                        anchor: None,
                        source: Span {
                            data: "literal",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[literal]\nthis text is literally literal",
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

    #[test]
    fn should_read_content_below_literal_style_verbatim() {
        let doc = Parser::default().parse("[literal]\nimage::not-an-image-block[]");

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
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "image::not-an-image-block[]",
                            line: 2,
                            col: 1,
                            offset: 10,
                        },
                        rendered: "image::not-an-image-block[]",
                    },
                    source: Span {
                        data: "[literal]\nimage::not-an-image-block[]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Literal,
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: None,
                            value: "literal",
                            shorthand_items: &["literal"],
                        },],
                        anchor: None,
                        source: Span {
                            data: "literal",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[literal]\nimage::not-an-image-block[]",
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

    #[test]
    fn listing_paragraph() {
        let doc = Parser::default().parse("[listing]\nthis text is a listing");

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
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "this text is a listing",
                            line: 2,
                            col: 1,
                            offset: 10,
                        },
                        rendered: "this text is a listing",
                    },
                    source: Span {
                        data: "[listing]\nthis text is a listing",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Listing,
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: None,
                            value: "listing",
                            shorthand_items: &["listing"],
                        },],
                        anchor: None,
                        source: Span {
                            data: "listing",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[listing]\nthis text is a listing",
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

    #[test]
    fn source_paragraph() {
        let doc = Parser::default().parse("[source]\nuse the source, luke!");

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
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "use the source, luke!",
                            line: 2,
                            col: 1,
                            offset: 9,
                        },
                        rendered: "use the source, luke!",
                    },
                    source: Span {
                        data: "[source]\nuse the source, luke!",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Source,
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: None,
                            value: "source",
                            shorthand_items: &["source"],
                        },],
                        anchor: None,
                        source: Span {
                            data: "source",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[source]\nuse the source, luke!",
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

    #[test]
    fn source_code_paragraph_with_language() {
        let doc = Parser::default().parse("[source, perl]\ndie 'zomg perl is tough';");

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
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "die 'zomg perl is tough';",
                            line: 2,
                            col: 1,
                            offset: 15,
                        },
                        rendered: "die 'zomg perl is tough';",
                    },
                    source: Span {
                        data: "[source, perl]\ndie 'zomg perl is tough';",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Source,
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[
                            ElementAttribute {
                                name: None,
                                value: "source",
                                shorthand_items: &["source"],
                            },
                            ElementAttribute {
                                name: None,
                                value: "perl",
                                shorthand_items: &[],
                            },
                        ],
                        anchor: None,
                        source: Span {
                            data: "source, perl",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[source, perl]\ndie 'zomg perl is tough';",
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

    #[test]
    fn literal_paragraph_terminates_at_block_attribute_list() {
        let doc = Parser::default().parse(" literal text\n[normal]\nnormal text");

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
                                data: " literal text",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "literal text",
                        },
                        source: Span {
                            data: " literal text",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Literal,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "normal text",
                                line: 3,
                                col: 1,
                                offset: 23,
                            },
                            rendered: "normal text",
                        },
                        source: Span {
                            data: "[normal]\nnormal text",
                            line: 2,
                            col: 1,
                            offset: 14,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: Some(Attrlist {
                            attributes: &[ElementAttribute {
                                name: None,
                                value: "normal",
                                shorthand_items: &["normal"],
                            },],
                            anchor: None,
                            source: Span {
                                data: "normal",
                                line: 2,
                                col: 2,
                                offset: 15,
                            },
                        },),
                    },),
                ],
                source: Span {
                    data: " literal text\n[normal]\nnormal text",
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

    #[test]
    fn literal_paragraph_terminates_at_block_delimiter() {
        let doc = Parser::default().parse(" literal text\n--\nnormal text\n--");

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
                                data: " literal text",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "literal text",
                        },
                        source: Span {
                            data: " literal text",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Literal,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::CompoundDelimited(CompoundDelimitedBlock {
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "normal text",
                                    line: 3,
                                    col: 1,
                                    offset: 17,
                                },
                                rendered: "normal text",
                            },
                            source: Span {
                                data: "normal text",
                                line: 3,
                                col: 1,
                                offset: 17,
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
                        context: "open",
                        source: Span {
                            data: "--\nnormal text\n--",
                            line: 2,
                            col: 1,
                            offset: 14,
                        },
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: " literal text\n--\nnormal text\n--",
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

    #[test]
    fn literal_paragraph_terminates_at_list_continuation() {
        let doc = Parser::default().parse(" literal text\n+");

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
                                data: " literal text",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "literal text",
                        },
                        source: Span {
                            data: " literal text",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Literal,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "+",
                                line: 2,
                                col: 1,
                                offset: 14,
                            },
                            rendered: "+",
                        },
                        source: Span {
                            data: "+",
                            line: 2,
                            col: 1,
                            offset: 14,
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
                ],
                source: Span {
                    data: " literal text\n+",
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
}

// Adapted from the `context 'Quote'` section of Asciidoctor's paragraphs test
// suite. The blockquote feature was unimplemented when the rest of this file
// was first ported, so these were left in the `port_from_ruby` stub below.
mod quote {
    use crate::tests::prelude::*;

    #[test]
    fn single_line_quote_paragraph() {
        let doc = Parser::default().parse("[quote]\nFamous quote.");
        assert_xpath(&doc, "//*[@class = \"quoteblock\"]", 1);
        // A styled quote paragraph renders its text directly inside the
        // blockquote, not wrapped in a `<p>`.
        assert_xpath(&doc, "//*[@class = \"quoteblock\"]//p", 0);
        assert_rendered_contains(&doc, "Famous quote.");
    }

    #[test]
    fn quote_paragraph_terminates_at_list_continuation() {
        let doc = Parser::default().parse("[quote]\nA famouse quote.\n+");
        assert_css(&doc, ".quoteblock", 1);
        // The list-continuation marker (`+`) terminates the quote paragraph and
        // becomes its own paragraph.
        assert_css(&doc, ".paragraph", 1);
        assert_xpath(&doc, "//*[@class=\"paragraph\"]/p[text() = \"+\"]", 1);
    }

    #[test]
    fn verse_paragraph() {
        let doc = Parser::default().parse("[verse]\nFamous verse.");
        assert_xpath(&doc, "//*[@class = \"verseblock\"]", 1);
        assert_xpath(&doc, "//*[@class = \"verseblock\"]/pre", 1);
        assert_xpath(&doc, "//*[@class = \"verseblock\"]//p", 0);
        assert_xpath(
            &doc,
            "//*[@class = \"verseblock\"]/pre[normalize-space(text()) = \"Famous verse.\"]",
            1,
        );
    }

    #[test]
    fn should_perform_normal_subs_on_a_verse_paragraph() {
        let doc = Parser::default().parse("[verse]\n_GET /groups/link:#group-id[{group-id}]_");
        let block = doc.nested_blocks().next().unwrap();
        assert_eq!(
            block.rendered_content(),
            Some("<em>GET /groups/<a href=\"#group-id\">{group-id}</a></em>")
        );
    }

    #[test]
    fn quote_paragraph_should_honor_explicit_subs_list() {
        let doc = Parser::default().parse("[subs=\"specialcharacters\"]\n[quote]\n*Hey Jude*");
        // Only special-character substitution runs, so the `*` is left intact
        // (not converted to a `<strong>`).
        assert_rendered_contains(&doc, "*Hey Jude*");
    }
}

mod special {
    use crate::tests::prelude::*;

    // Ported from Ruby Asciidoctor's paragraphs_test.rb
    // ('should process preprocessor conditional in paragraph content').
    #[test]
    fn should_process_preprocessor_conditional_in_paragraph_content() {
        // `asciidoctor-version` and `backend` are not set by default in this
        // crate, so they are supplied explicitly to mirror the Ruby environment.
        let doc = Parser::default()
            .with_intrinsic_attribute(
                "asciidoctor-version",
                "2.0",
                ModificationContext::Anywhere,
            )
            .with_intrinsic_attribute("backend", "html5", ModificationContext::Anywhere)
            .parse(
                "ifdef::asciidoctor-version[]\n[sidebar]\nFirst line of sidebar.\nifdef::backend[The backend is {backend}.]\nLast line of sidebar.\nendif::[]",
            );

        // The outer `ifdef` includes the sidebar; the inner single-line `ifdef`
        // resolves to the backend value.
        assert_output_contains(&doc, "First line of sidebar.");
        assert_output_contains(&doc, "The backend is html5.");
        assert_output_contains(&doc, "Last line of sidebar.");
    }

    // Asciidoctor's `ADMONITION_STYLES`: the five built-in admonition labels.
    const ADMONITION_STYLES: [&str; 5] = ["NOTE", "TIP", "IMPORTANT", "WARNING", "CAUTION"];

    // Ported from Ruby Asciidoctor's paragraphs_test.rb ('note multiline
    // syntax'). A styled paragraph whose style is an admonition label renders as
    // an admonition block.
    #[test]
    fn note_multiline_syntax() {
        for style in ADMONITION_STYLES {
            let doc = Parser::default().parse(&format!("[{style}]\nThis is a winner."));
            assert_xpath(
                &doc,
                &format!(
                    "//div[@class = \"admonitionblock {}\"]",
                    style.to_lowercase()
                ),
                1,
            );
        }
    }

    // Ported from Ruby Asciidoctor's paragraphs_test.rb ('note block syntax').
    // An example-delimited block carrying an admonition style renders as an
    // admonition block.
    #[test]
    fn note_block_syntax() {
        for style in ADMONITION_STYLES {
            let doc = Parser::default().parse(&format!("[{style}]\n====\nThis is a winner.\n===="));
            assert_xpath(
                &doc,
                &format!(
                    "//div[@class = \"admonitionblock {}\"]",
                    style.to_lowercase()
                ),
                1,
            );
        }
    }

    // Ported from Ruby Asciidoctor's paragraphs_test.rb ('note inline syntax').
    // The inline label form (e.g. `NOTE: ...`) renders as an admonition block.
    #[test]
    fn note_inline_syntax() {
        for style in ADMONITION_STYLES {
            let doc = Parser::default().parse(&format!("{style}: This is important, fool!"));
            assert_xpath(
                &doc,
                &format!(
                    "//div[@class = \"admonitionblock {}\"]",
                    style.to_lowercase()
                ),
                1,
            );
        }
    }
}

#[ignore]
#[test]
fn port_from_ruby() {
    todo!(
        "Port this: {}",
        r###"
  context 'special' do
    # NOTE: 'note multiline syntax', 'note block syntax', and 'note inline
    # syntax' have been ported to `mod special` now that admonitions are
    # implemented.

    # NOTE: 'should process preprocessor conditional in paragraph content' has
    # been ported to `mod special` below now that conditional preprocessor
    # directives are implemented.

    context 'Styled Paragraphs' do
      test 'should wrap text in simpara for styled paragraphs when converted to DocBook' do
        input = <<~'EOS'
        = Book
        :doctype: book

        [preface]
        = About this book

        [abstract]
        An abstract for the book.

        = Part 1

        [partintro]
        An intro to this part.

        == Chapter 1

        [sidebar]
        Just a side note.

        [example]
        As you can see here.

        [quote]
        Wise words from a wise person.

        [open]
        Make it what you want.
        EOS

        output = convert_string input, backend: 'docbook'
        assert_css 'abstract > simpara', output, 1
        assert_css 'partintro > simpara', output, 1
        assert_css 'sidebar > simpara', output, 1
        assert_css 'informalexample > simpara', output, 1
        assert_css 'blockquote > simpara', output, 1
        assert_css 'chapter > simpara', output, 1
      end

      test 'should convert open paragraph to open block' do
        input = <<~'EOS'
        [open]
        Make it what you want.
        EOS

        output = convert_string_to_embedded input
        assert_css '.openblock', output, 1
        assert_css '.openblock p', output, 0
      end

      test 'should wrap text in simpara for styled paragraphs with title when converted to DocBook' do
        input = <<~'EOS'
        = Book
        :doctype: book

        [preface]
        = About this book

        [abstract]
        .Abstract title
        An abstract for the book.

        = Part 1

        [partintro]
        .Part intro title
        An intro to this part.

        == Chapter 1

        [sidebar]
        .Sidebar title
        Just a side note.

        [example]
        .Example title
        As you can see here.

        [quote]
        .Quote title
        Wise words from a wise person.
        EOS

        output = convert_string input, backend: 'docbook'
        assert_css 'abstract > title', output, 1
        assert_xpath '//abstract/title[text() = "Abstract title"]', output, 1
        assert_css 'abstract > title + simpara', output, 1
        assert_css 'partintro > title', output, 1
        assert_xpath '//partintro/title[text() = "Part intro title"]', output, 1
        assert_css 'partintro > title + simpara', output, 1
        assert_css 'sidebar > title', output, 1
        assert_xpath '//sidebar/title[text() = "Sidebar title"]', output, 1
        assert_css 'sidebar > title + simpara', output, 1
        assert_css 'example > title', output, 1
        assert_xpath '//example/title[text() = "Example title"]', output, 1
        assert_css 'example > title + simpara', output, 1
        assert_css 'blockquote > title', output, 1
        assert_xpath '//blockquote/title[text() = "Quote title"]', output, 1
        assert_css 'blockquote > title + simpara', output, 1
      end
    end

    context 'Inline doctype' do
      test 'should only format and output text in first paragraph when doctype is inline' do
        input = "http://asciidoc.org[AsciiDoc] is a _lightweight_ markup language...\n\nignored"
        output = convert_string input, doctype: 'inline'
        assert_equal '<a href="http://asciidoc.org">AsciiDoc</a> is a <em>lightweight</em> markup language&#8230;&#8203;', output
      end

      test 'should output nil and warn if first block is not a paragraph' do
        input = '* bullet'
        using_memory_logger do |logger|
          output = convert_string input, doctype: 'inline'
          assert_nil output
          assert_message logger, :WARN, '~no inline candidate'
        end
      end
    end
  end

  context 'Custom' do
    test 'should not warn if paragraph style is unregisted' do
      input = <<~'EOS'
      [foo]
      bar
      EOS
      using_memory_logger do |logger|
        convert_string_to_embedded input
        assert_empty logger.messages
      end
    end

    test 'should log debug message if paragraph style is unknown and debug level is enabled' do
      input = <<~'EOS'
      [foo]
      bar
      EOS
      using_memory_logger Logger::Severity::DEBUG do |logger|
        convert_string_to_embedded input
        assert_message logger, :DEBUG, '<stdin>: line 2: unknown style for paragraph: foo', Hash
      end
    end
  end

"###
    );
}
