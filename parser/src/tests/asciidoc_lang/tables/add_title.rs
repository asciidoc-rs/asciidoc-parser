use crate::tests::prelude::*;

track_file!("docs/modules/tables/pages/add-title.adoc");

non_normative!(
    r#"
= Add a Title to a Table
:navtitle: Add a Title
// TODO/FIX: When soft unset is used from the Antora playbook, and then the attribute is reset in the document, it doesn't use the default value, so "Table" has to be explicitly assigned. Otherwise the label is simply the incremented number (i.e., "1.").
:table-caption: Table

A table can have an optional title (i.e., table caption).
To add a title to a table, use the block title syntax.

"#
);

#[test]
fn add_a_title_to_a_table() {
    verifies!(
        r#"
.Add an optional title to a table
[source#ex-title]
----
.A table with a title <.>
[%autowidth]
|===
|Column 1, header row |Column 2, header row

|Cell in column 1, row 2
|Cell in column 2, row 2
|===
----
<.> On the line directly above the table's opening delimiter (or above its optional attribute line, as shown here), enter a dot (`.`) directly followed by the text of the title.

"#
    );

    let source = ".A table with a title\n[%autowidth]\n|===\n|Column 1, header row |Column 2, header row\n\n|Cell in column 1, row 2\n|Cell in column 2, row 2\n|===";

    let doc = Parser::default().parse(source);

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
            blocks: &[Block::Table(TableBlock {
                columns: &[
                    TableColumn {
                        width: 1,
                        h_align: HorizontalAlignment::Left,
                        v_align: VerticalAlignment::Top,
                        style: ColumnStyle::Default,
                    },
                    TableColumn {
                        width: 1,
                        h_align: HorizontalAlignment::Left,
                        v_align: VerticalAlignment::Top,
                        style: ColumnStyle::Default,
                    }
                ],
                header_row: Some(TableRow {
                    cells: &[
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "Column 1, header row",
                                    line: 4,
                                    col: 2,
                                    offset: 41,
                                },
                                rendered: "Column 1, header row",
                            }),
                        },
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "Column 2, header row",
                                    line: 4,
                                    col: 24,
                                    offset: 63,
                                },
                                rendered: "Column 2, header row",
                            }),
                        },
                    ],
                }),
                body_rows: &[TableRow {
                    cells: &[
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "Cell in column 1, row 2",
                                    line: 6,
                                    col: 2,
                                    offset: 86,
                                },
                                rendered: "Cell in column 1, row 2",
                            }),
                        },
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "Cell in column 2, row 2",
                                    line: 7,
                                    col: 2,
                                    offset: 111,
                                },
                                rendered: "Cell in column 2, row 2",
                            }),
                        },
                    ],
                }],
                footer_row: None,
                source: Span {
                    data: ".A table with a title\n[%autowidth]\n|===\n|Column 1, header row |Column 2, header row\n\n|Cell in column 1, row 2\n|Cell in column 2, row 2\n|===",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: Some(Span {
                    data: "A table with a title",
                    line: 1,
                    col: 2,
                    offset: 1,
                }),
                title: Some("A table with a title"),
                caption: Some("Table 1. "),
                number: Some(1),
                anchor: None,
                anchor_reftext: None,
                attrlist: Some(Attrlist {
                    attributes: &[ElementAttribute {
                        name: None,
                        shorthand_items: &["%autowidth"],
                        value: "%autowidth",
                    }],
                    anchor: None,
                    source: Span {
                        data: "%autowidth",
                        line: 2,
                        col: 2,
                        offset: 23,
                    },
                }),
            })],
            source: Span {
                data: source,
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog::default(),
        }
    );

    verifies!(
        r#"
The table from <<ex-title>> is displayed below.

.A table with a title
[%autowidth]
|===
|Column 1, header row |Column 2, header row

|Cell in column 1, row 2
|Cell in column 2, row 2
|===

You'll notice in the above result, that the processor automatically added _Table 1._ in front of the table's title.
"#
    );

    // The displayed table renders with the processor's automatic caption
    // ("Table 1. ") prepended to the block title, both inside the table's
    // `<caption class="title">`.
    assert_xpath(
        &doc,
        "//caption[text()=\"Table 1. A table with a title\"]",
        1,
    );

    verifies!(
        r#"
This title label can be xref:customize-title-label.adoc[customized] or xref:turn-off-title-label.adoc[deactivated].
"#
    );

    // The two cross-references render as links to the sibling pages that cover
    // customizing and deactivating the title label.
    let see_also = Parser::default().parse(
        "This title label can be xref:customize-title-label.adoc[customized] or xref:turn-off-title-label.adoc[deactivated].",
    );
    let crate::blocks::Block::Simple(para) = &see_also.nested_blocks().next().unwrap() else {
        panic!("expected a paragraph");
    };
    assert_eq!(
        para.content().rendered(),
        "This title label can be <a href=\"#customize-title-label.adoc\">customized</a> or <a href=\"#turn-off-title-label.adoc\">deactivated</a>."
    );
}
