use std::collections::HashMap;

use crate::{
    Parser,
    blocks::{BreakType, IsBlock, SimpleBlockStyle},
    tests::prelude::*,
};

track_file!("docs/modules/blocks/pages/breaks.adoc");

non_normative!(
    r#"
= Breaks

"#
);

#[test]
fn thematic_break_syntax() {
    verifies!(
        r#"
include::partial$thematic-breaks.adoc[]

"#
    );

    let doc = Parser::default().parse("'''");
    let brk = first_block_from(&doc);
    assert_eq!(brk.type_(), BreakType::Thematic);

    let doc = Parser::default().parse("---");
    let brk = first_block_from(&doc);
    assert_eq!(brk.type_(), BreakType::Thematic);

    let doc = Parser::default().parse("- - -");
    let brk = first_block_from(&doc);
    assert_eq!(brk.type_(), BreakType::Thematic);

    let doc = Parser::default().parse("***");
    let brk = first_block_from(&doc);
    assert_eq!(brk.type_(), BreakType::Thematic);

    let doc = Parser::default().parse("* * *");
    let brk = first_block_from(&doc);
    assert_eq!(brk.type_(), BreakType::Thematic);
}

#[test]
fn page_break_syntax() {
    verifies!(
        r#"
include::partial$page-breaks.adoc[]
"#
    );

    let doc = Parser::default().parse("<<<");
    let brk = first_block_from(&doc);
    assert_eq!(brk.type_(), BreakType::Page);

    // Forced page break.
    let doc = Parser::default().parse("[%always]\n<<<");

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
            blocks: &[Block::Break(Break {
                type_: BreakType::Page,
                source: Span {
                    data: "[%always]\n<<<",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                attrlist: Some(Attrlist {
                    attributes: &[ElementAttribute {
                        name: None,
                        value: "%always",
                        shorthand_items: &["%always"],
                    },],
                    anchor: None,
                    source: Span {
                        data: "%always",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },),],
            source: Span {
                data: "[%always]\n<<<",
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

    // With page layout.
    let doc = Parser::default().parse("[page-layout=landscape]\n<<<");

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
            blocks: &[Block::Break(Break {
                type_: BreakType::Page,
                source: Span {
                    data: "[page-layout=landscape]\n<<<",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                attrlist: Some(Attrlist {
                    attributes: &[ElementAttribute {
                        name: Some("page-layout",),
                        value: "landscape",
                        shorthand_items: &[],
                    },],
                    anchor: None,
                    source: Span {
                        data: "page-layout=landscape",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },),],
            source: Span {
                data: "[page-layout=landscape]\n<<<",
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

    // Column break.
    let doc = Parser::default().parse("left column\n\n[.column]\n<<<\n\nright column");

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
                            data: "left column",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "left column",
                    },
                    source: Span {
                        data: "left column",
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
                },),
                Block::Break(Break {
                    type_: BreakType::Page,
                    source: Span {
                        data: "[.column]\n<<<",
                        line: 3,
                        col: 1,
                        offset: 13,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: None,
                            value: ".column",
                            shorthand_items: &[".column"],
                        },],
                        anchor: None,
                        source: Span {
                            data: ".column",
                            line: 3,
                            col: 2,
                            offset: 14,
                        },
                    },),
                },),
                Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "right column",
                            line: 6,
                            col: 1,
                            offset: 28,
                        },
                        rendered: "right column",
                    },
                    source: Span {
                        data: "right column",
                        line: 6,
                        col: 1,
                        offset: 28,
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
                data: "left column\n\n[.column]\n<<<\n\nright column",
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

fn first_block_from<'src>(doc: &'src crate::Document) -> &'src crate::blocks::Break<'src> {
    let block = doc.nested_blocks().next().unwrap();
    let crate::blocks::Block::Break(brk) = block else {
        panic!("Wrong block type: {block:#?}");
    };

    brk
}
