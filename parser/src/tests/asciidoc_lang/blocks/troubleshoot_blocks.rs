use crate::{Parser, blocks::SimpleBlockStyle, tests::prelude::*, warnings::WarningType};

track_file!("docs/modules/blocks/pages/troubleshoot-blocks.adoc");

non_normative!(
    r#"
= Troubleshooting Blocks

"#
);

#[test]
fn opening_and_closing_delimiters_must_match() {
    verifies!(
        r#"
== Opening and closing delimiters

The opening and closing delimiters of a delimited block must be the same length.
For example, a sidebar is specified by an opening delimiter of four asterisks (`+****+`).
Its closing delimiter must also be four asterisks (`+****+`).

Here's a sidebar using valid delimiter lengths:

----
****
This is a valid delimited block.
It will be styled as a sidebar.
****
----

"#
    );

    let doc = Parser::default().parse(
        "****\nThis is a valid delimited block.\nIt will be styled as a sidebar.\n****
",
    );

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
            blocks: &[Block::CompoundDelimited(CompoundDelimitedBlock {
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "This is a valid delimited block.\nIt will be styled as a sidebar.",
                            line: 2,
                            col: 1,
                            offset: 5,
                        },
                        rendered: "This is a valid delimited block.\nIt will be styled as a sidebar.",
                    },
                    source: Span {
                        data: "This is a valid delimited block.\nIt will be styled as a sidebar.",
                        line: 2,
                        col: 1,
                        offset: 5,
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
                    data: "****\nThis is a valid delimited block.\nIt will be styled as a sidebar.\n****",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },),],
            source: Span {
                data: "****\nThis is a valid delimited block.\nIt will be styled as a sidebar.\n****",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog::default(),
        }
    );
}

#[test]
fn delimiters_mismatch_example() {
    verifies!(
        r#"
However, the delimiter lengths in the following delimited block are not equal in length and therefore invalid:

----
********
This is an invalid sidebar block because the delimiter lines are different lengths.
****
----

When an AsciiDoc processor encounters the previous example, it will put the remainder of the content in the document inside the delimited block.
As far as the processor is concerned, the closing delimiter is just a line of content.
However, the processor will issue a warning if a matching closing delimiter is never found.

If you want the processor to recognize a closing delimiter, it must be the same length as the opening delimiter.
"#
    );

    let doc = Parser::default().parse(
        "********\nThis is an invalid sidebar block because the delimiter lines are different lengths.\n****"
    );

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
            blocks: &[Block::CompoundDelimited(CompoundDelimitedBlock {
                blocks: &[
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "This is an invalid sidebar block because the delimiter lines are different lengths.",
                                line: 2,
                                col: 1,
                                offset: 9,
                            },
                            rendered: "This is an invalid sidebar block because the delimiter lines are different lengths.",
                        },
                        source: Span {
                            data: "This is an invalid sidebar block because the delimiter lines are different lengths.",
                            line: 2,
                            col: 1,
                            offset: 9,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::CompoundDelimited(CompoundDelimitedBlock {
                        blocks: &[],
                        context: "sidebar",
                        source: Span {
                            data: "****",
                            line: 3,
                            col: 1,
                            offset: 93,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                context: "sidebar",
                source: Span {
                    data: "********\nThis is an invalid sidebar block because the delimiter lines are different lengths.\n****",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },),],
            source: Span {
                data: "********\nThis is an invalid sidebar block because the delimiter lines are different lengths.\n****",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[
                Warning {
                    source: Span {
                        data: "********",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    warning: WarningType::UnterminatedDelimitedBlock,
                },
                Warning {
                    source: Span {
                        data: "****",
                        line: 3,
                        col: 1,
                        offset: 93,
                    },
                    warning: WarningType::UnterminatedDelimitedBlock,
                },
            ],
            source_map: SourceMap(&[]),
            catalog: Catalog {
                refs: HashMap::from([]),
                reftext_to_id: HashMap::from([]),
            },
        }
    );
}
