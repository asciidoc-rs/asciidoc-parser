use crate::tests::prelude::*;

track_file!("docs/modules/tables/pages/build-a-basic-table.adoc");

non_normative!(
    r#"
= Build a Basic Table
:page-aliases: index.adoc

A table is a delimited block that can have optional customizations, such as an ID and a title, as well as table-specific attributes, options, and roles.
However, at its most basic, a table only needs columns and rows.

On this page, you'll learn:

* [x] How to set up an AsciiDoc table block and its attribute list.
* [x] How to add columns to a table using the `cols` attribute.
* [x] How to add cells to a table and arrange them into rows.
* [x] How to designate a row as the table's header row.

"#
);

#[test]
fn create_a_table_with_two_columns_and_three_rows() {
    non_normative!(
        r#"
== Create a table with two columns and three rows
"#
    );

    verifies!(
        r#"

In <<ex-cols>>, we'll assign the `cols` attribute a list of column specifiers.
A column specifier represents a column.

.Set up a table with two columns
[source#ex-cols]
----
[cols="1,1"] <.> <.>
|=== <.>
----
<.> On a new line, create an attribute list.
Set the `cols` attribute, followed by an equals sign (`=`).
<.> Assign a list of comma-separated column specifiers enclosed in double quotation marks (`"`) to `cols`.
Each column specifier represents a column.
<.> On the line directly after the attribute list, enter the opening table delimiter.
A table delimiter is one vertical bar followed by three equals signs (`|===`).
This delimiter starts the table block.

The table in <<ex-cols>> will contain two columns because there are two comma-separated entries in the list assigned to `cols`.
Each entry in the list is called a column specifier.
A [.term]*column specifier* represents a column and the width, alignment, and style properties assigned to that column.
When each column specifier is the same number, in this case the integer `1`, all of the columns`' widths will be identical.
Each column in <<ex-cols>> will be the same width regardless of how much content they contain.

Next, let's add three rows to the table.
Each row has the same number of cells.
Since the table in <<ex-rows>> has two columns, each row will contain two cells.
A cell starts with a vertical bar (`|`).

.Add three rows to the table
[source#ex-rows]
----
[cols="1,1"]
|===
|Cell in column 1, row 1 <.>
|Cell in column 2, row 1 <.>
<.>
|Cell in column 1, row 2
|Cell in column 2, row 2

|Cell in column 1, row 3
|Cell in column 2, row 3 <.>
|=== <.>
----
<.> To create a new cell, press kbd:[Shift+|].
After the vertical bar (`|`), enter the content you want displayed in that cell.
<.> On a new line, start another cell with a `|`.
Each consecutive cell is placed in a separate, consecutive column in a row.
<.> Rows are separated by one or more empty lines.
<.> When you finish adding cells to your table, press kbd:[Enter] to go to a new line.
<.> Enter the closing delimiter (`|===`) to end the table block.

TIP: The suggestion to start each cell on its own line and to separate rows by empty lines is merely a stylistic choice.
You can enter xref:add-cells-and-rows.adoc[more than one cell or all of the cells in a row on the same line] since the processor creates a new cell each time it encounters a vertical bar (`|`).

The table from <<ex-rows>> is displayed below.
It contains two columns and three rows of text positioned and styled using the default alignment, style, border, and width attribute values.

[cols="1,1"]
|===
|Cell in column 1, row 1
|Cell in column 2, row 1

|Cell in column 1, row 2 |Cell in column 2, row 2
|Cell in column 1, row 3 |Cell in column 2, row 3
|===

In addition to the xref:add-columns.adoc[cols attribute], you can identify the number of columns using a xref:add-columns.adoc#column-multiplier[column multiplier] or xref:add-columns.adoc#implicit-cols[the table's first row].
However, the `cols` attribute is required to customize the xref:adjust-column-widths.adoc[width], xref:align-by-column.adoc[alignment], or xref:format-column-content.adoc[style] of a column.

"#
    );

    let doc = Parser::default().parse(
        "[cols=\"1,1\"]\n|===\n|Cell in column 1, row 1\n|Cell in column 2, row 1\n\n|Cell in column 1, row 2 |Cell in column 2, row 2\n|Cell in column 1, row 3 |Cell in column 2, row 3\n|===",
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
                header_row: None,
                body_rows: &[
                    TableRow {
                        cells: &[
                            TableCell {
                                content: TableCellContent::Simple(Content {
                                    original: Span {
                                        data: "Cell in column 1, row 1",
                                        line: 3,
                                        col: 2,
                                        offset: 19,
                                    },
                                    rendered: "Cell in column 1, row 1",
                                }),
                            },
                            TableCell {
                                content: TableCellContent::Simple(Content {
                                    original: Span {
                                        data: "Cell in column 2, row 1",
                                        line: 4,
                                        col: 2,
                                        offset: 44,
                                    },
                                    rendered: "Cell in column 2, row 1",
                                }),
                            },
                        ],
                    },
                    TableRow {
                        cells: &[
                            TableCell {
                                content: TableCellContent::Simple(Content {
                                    original: Span {
                                        data: "Cell in column 1, row 2",
                                        line: 6,
                                        col: 2,
                                        offset: 70,
                                    },
                                    rendered: "Cell in column 1, row 2",
                                }),
                            },
                            TableCell {
                                content: TableCellContent::Simple(Content {
                                    original: Span {
                                        data: "Cell in column 2, row 2",
                                        line: 6,
                                        col: 27,
                                        offset: 95,
                                    },
                                    rendered: "Cell in column 2, row 2",
                                }),
                            },
                        ],
                    },
                    TableRow {
                        cells: &[
                            TableCell {
                                content: TableCellContent::Simple(Content {
                                    original: Span {
                                        data: "Cell in column 1, row 3",
                                        line: 7,
                                        col: 2,
                                        offset: 120,
                                    },
                                    rendered: "Cell in column 1, row 3",
                                }),
                            },
                            TableCell {
                                content: TableCellContent::Simple(Content {
                                    original: Span {
                                        data: "Cell in column 2, row 3",
                                        line: 7,
                                        col: 27,
                                        offset: 145,
                                    },
                                    rendered: "Cell in column 2, row 3",
                                }),
                            },
                        ],
                    },
                ],
                footer_row: None,
                source: Span {
                    data: "[cols=\"1,1\"]\n|===\n|Cell in column 1, row 1\n|Cell in column 2, row 1\n\n|Cell in column 1, row 2 |Cell in column 2, row 2\n|Cell in column 1, row 3 |Cell in column 2, row 3\n|===",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                caption: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: Some(Attrlist {
                    attributes: &[ElementAttribute {
                        name: Some("cols"),
                        shorthand_items: &[],
                        value: "1,1",
                    }],
                    anchor: None,
                    source: Span {
                        data: "cols=\"1,1\"",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                }),
            })],
            source: Span {
                data: "[cols=\"1,1\"]\n|===\n|Cell in column 1, row 1\n|Cell in column 2, row 1\n\n|Cell in column 1, row 2 |Cell in column 2, row 2\n|Cell in column 1, row 3 |Cell in column 2, row 3\n|===",
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
fn add_a_header_row_to_the_table() {
    non_normative!(
        r#"
=== Add a header row to the table
"#
    );

    verifies!(
        r#"

Let's add a header row to the table in <<ex-header>>.
You can implicitly identify the first row of a table as a header row by entering all of the first row's cells on the line directly after the opening table delimiter.

.Add a header row to the table
[source#ex-header]
----
[cols="1,1"]
|===
|Cell in column 1, header row |Cell in column 2, header row <.>
<.>
|Cell in column 1, row 2
|Cell in column 2, row 2

|Cell in column 1, row 3
|Cell in column 2, row 3

|Cell in column 1, row 4
|Cell in column 2, row 4
|===
----
<.> On the line directly after the opening delimiter (`|===`), enter all of the first row's cells on a single line.
<.> Leave the line directly after the header row empty.

The table from <<ex-header>> is displayed below.

[cols="1,1"]
|===
|Cell in column 1, header row |Cell in column 2, header row

|Cell in column 1, row 2
|Cell in column 2, row 2

|Cell in column 1, row 3
|Cell in column 2, row 3

|Cell in column 1, row 4
|Cell in column 2, row 4
|===

A header row can also be identified by assigning xref:add-header-row.adoc[header to the options attribute].
"#
    );

    let doc = Parser::default().parse(
        "[cols=\"1,1\"]\n|===\n|Cell in column 1, header row |Cell in column 2, header row\n\n|Cell in column 1, row 2\n|Cell in column 2, row 2\n\n|Cell in column 1, row 3\n|Cell in column 2, row 3\n\n|Cell in column 1, row 4\n|Cell in column 2, row 4\n|===",
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
                                    data: "Cell in column 1, header row",
                                    line: 3,
                                    col: 2,
                                    offset: 19,
                                },
                                rendered: "Cell in column 1, header row",
                            }),
                        },
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "Cell in column 2, header row",
                                    line: 3,
                                    col: 32,
                                    offset: 49,
                                },
                                rendered: "Cell in column 2, header row",
                            }),
                        },
                    ],
                }),
                body_rows: &[
                    TableRow {
                        cells: &[
                            TableCell {
                                content: TableCellContent::Simple(Content {
                                    original: Span {
                                        data: "Cell in column 1, row 2",
                                        line: 5,
                                        col: 2,
                                        offset: 80,
                                    },
                                    rendered: "Cell in column 1, row 2",
                                }),
                            },
                            TableCell {
                                content: TableCellContent::Simple(Content {
                                    original: Span {
                                        data: "Cell in column 2, row 2",
                                        line: 6,
                                        col: 2,
                                        offset: 105,
                                    },
                                    rendered: "Cell in column 2, row 2",
                                }),
                            },
                        ],
                    },
                    TableRow {
                        cells: &[
                            TableCell {
                                content: TableCellContent::Simple(Content {
                                    original: Span {
                                        data: "Cell in column 1, row 3",
                                        line: 8,
                                        col: 2,
                                        offset: 131,
                                    },
                                    rendered: "Cell in column 1, row 3",
                                }),
                            },
                            TableCell {
                                content: TableCellContent::Simple(Content {
                                    original: Span {
                                        data: "Cell in column 2, row 3",
                                        line: 9,
                                        col: 2,
                                        offset: 156,
                                    },
                                    rendered: "Cell in column 2, row 3",
                                }),
                            },
                        ],
                    },
                    TableRow {
                        cells: &[
                            TableCell {
                                content: TableCellContent::Simple(Content {
                                    original: Span {
                                        data: "Cell in column 1, row 4",
                                        line: 11,
                                        col: 2,
                                        offset: 182,
                                    },
                                    rendered: "Cell in column 1, row 4",
                                }),
                            },
                            TableCell {
                                content: TableCellContent::Simple(Content {
                                    original: Span {
                                        data: "Cell in column 2, row 4",
                                        line: 12,
                                        col: 2,
                                        offset: 207,
                                    },
                                    rendered: "Cell in column 2, row 4",
                                }),
                            },
                        ],
                    },
                ],
                footer_row: None,
                source: Span {
                    data: "[cols=\"1,1\"]\n|===\n|Cell in column 1, header row |Cell in column 2, header row\n\n|Cell in column 1, row 2\n|Cell in column 2, row 2\n\n|Cell in column 1, row 3\n|Cell in column 2, row 3\n\n|Cell in column 1, row 4\n|Cell in column 2, row 4\n|===",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                caption: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: Some(Attrlist {
                    attributes: &[ElementAttribute {
                        name: Some("cols"),
                        shorthand_items: &[],
                        value: "1,1",
                    }],
                    anchor: None,
                    source: Span {
                        data: "cols=\"1,1\"",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                }),
            })],
            source: Span {
                data: "[cols=\"1,1\"]\n|===\n|Cell in column 1, header row |Cell in column 2, header row\n\n|Cell in column 1, row 2\n|Cell in column 2, row 2\n\n|Cell in column 1, row 3\n|Cell in column 2, row 3\n\n|Cell in column 1, row 4\n|Cell in column 2, row 4\n|===",
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
