use crate::tests::prelude::*;

track_file!("docs/modules/tables/pages/add-cells-and-rows.adoc");

non_normative!(
    r#"
= Add Cells and Rows to a Table
//let's add a tip or xref to something about the "default" cell separator and data format and the alternatives that's not too invasive to the flow

"#
);

#[test]
fn table_cells() {
    non_normative!(
        r#"
== Table cells

"#
    );

    verifies!(
        r#"
[[cell-separator]]Each new cell in a table is declared with a cell separator.
The default [.term]*cell separator* is a vertical bar (`|`).
All of the content entered after a cell separator is included in that cell until the processor encounters a space followed by another vertical bar (`|`) or a new line that begins with a `|`.

.Creating table cells with the default cell separator
[source#ex-separator]
----
[cols="3,2,3"]
|===
|This content is placed in the first cell of column 1
|This line starts with a vertical bar so this content is placed in a new cell in column 2 |When the processor encounters a whitespace followed by a vertical bar it ends the previous cell and starts a new cell
|===
----

When the processor encounters another `|`, it creates a new cell in the next consecutive column.
Once the processor reaches the xref:add-columns.adoc[last column assigned to the table], the next cell it encounters is placed in a new row.
Taking into account any xref:span-cells.adoc[spans], which are applied via a <<specifiers,cell specifier>>, each row consists of the same number of cells.

"#
    );

    // A space (or tab) followed by a `|`, or a `|` at the start of a line, ends
    // the previous cell and begins a new one. The three cells declared here flow
    // into a single row because the table has three columns.
    let doc = Parser::default().parse(
        "[cols=\"3,2,3\"]\n|===\n|This content is placed in the first cell of column 1\n|This line starts with a vertical bar so this content is placed in a new cell in column 2 |When the processor encounters a whitespace followed by a vertical bar it ends the previous cell and starts a new cell\n|===",
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
                        width: 3,
                        h_align: HorizontalAlignment::Left,
                        v_align: VerticalAlignment::Top,
                        style: ColumnStyle::Default,
                    },
                    TableColumn {
                        width: 2,
                        h_align: HorizontalAlignment::Left,
                        v_align: VerticalAlignment::Top,
                        style: ColumnStyle::Default,
                    },
                    TableColumn {
                        width: 3,
                        h_align: HorizontalAlignment::Left,
                        v_align: VerticalAlignment::Top,
                        style: ColumnStyle::Default,
                    },
                ],
                header_row: None,
                body_rows: &[TableRow {
                    cells: &[
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "This content is placed in the first cell of column 1",
                                    line: 3,
                                    col: 2,
                                    offset: 21,
                                },
                                rendered: "This content is placed in the first cell of column 1",
                            }),
                        },
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "This line starts with a vertical bar so this content is placed in a new cell in column 2",
                                    line: 4,
                                    col: 2,
                                    offset: 75,
                                },
                                rendered: "This line starts with a vertical bar so this content is placed in a new cell in column 2",
                            }),
                        },
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "When the processor encounters a whitespace followed by a vertical bar it ends the previous cell and starts a new cell",
                                    line: 4,
                                    col: 92,
                                    offset: 165,
                                },
                                rendered: "When the processor encounters a whitespace followed by a vertical bar it ends the previous cell and starts a new cell",
                            }),
                        },
                    ],
                }],
                footer_row: None,
                source: Span {
                    data: "[cols=\"3,2,3\"]\n|===\n|This content is placed in the first cell of column 1\n|This line starts with a vertical bar so this content is placed in a new cell in column 2 |When the processor encounters a whitespace followed by a vertical bar it ends the previous cell and starts a new cell\n|===",
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
                        value: "3,2,3",
                    }],
                    anchor: None,
                    source: Span {
                        data: "cols=\"3,2,3\"",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                }),
            })],
            source: Span {
                data: "[cols=\"3,2,3\"]\n|===\n|This content is placed in the first cell of column 1\n|This line starts with a vertical bar so this content is placed in a new cell in column 2 |When the processor encounters a whitespace followed by a vertical bar it ends the previous cell and starts a new cell\n|===",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog::default(),
        }
    );

    // Cell specifiers (per-cell spans, duplication, alignment, and style
    // operators) are introduced here but specified in detail on dedicated
    // pages (span-cells, duplicate-cells, align-by-cell, format-cell-content).
    // They are not yet implemented, so this normative section is flagged as a
    // TODO: come back and verify it (especially the `ex-specifier` example)
    // once per-cell specifiers are supported and those pages are covered.
    to_do_verifies!(
        r#"
[#specifiers]
=== Cell specifiers and operators

A [.term]*cell specifier* is a positional attribute placed directly in front of a cell separator that represents the position and style properties assigned to a cell's content.
In the example below, a horizontal alignment operator and style operator have been assigned to the first cell's specifier.

.Using cell specifiers
[source#ex-specifier]
----
[cols="2*"]
|===
>s|This cell's specifier indicates that this cell's content is right-aligned and bold.
|The cell specifier on this cell hasn't been set explicitly, so the  default properties are applied.
|===
----

AsciiDoc provides operators to control the following cell properties:

* xref:span-cells.adoc[span]
* xref:duplicate-cells.adoc[duplication]
* xref:align-by-cell.adoc#horizontal-operators[horizontal alignment]
* xref:align-by-cell.adoc#vertical-operators[vertical alignment]
* xref:format-cell-content.adoc[content style]

A cell specifier only applies to the cell it's placed on, not to all of the cells in the same row.
Also, the operator in a cell specifier will override the operator in a xref:add-columns.adoc#col-specifier[column specifier] if they belong to the same property.

"#
    );
}

#[test]
fn create_a_table_cell() {
    non_normative!(
        r#"
== Create a table cell

In this section, we'll set up a table and add two rows of cells to it.
First, let's create two cells in <<ex-cells>> and see how they get arranged into a row.

"#
    );

    verifies!(
        r#"
.Add two cells to a table
[source#ex-cells]
----
[cols="1,1"] <.>
|===
|This cell is in column 1, row 1 <.>
|This cell is in column 2, row 1 <.>
|===
----
<.> The table has two columns because two column specifiers are assigned to the `cols` attribute.
<.> The processor places this cell in the first column and row of the table because the vertical bar (`|`) at the beginning of this cell is the first cell separator the processor encounters after the opening table delimiter (`|===`).
<.> This is the second `|` the processor encounters, so this cell is placed in the second column of the first row.

Though the two cells in <<ex-cells>> were entered on separate lines, the processor places them in the same row.

.Result of <<ex-cells>>
[cols="1,1"]
|===
|This cell is in column 1, row 1
|This cell is in column 2, row 1
|===

Since the number of columns in <<ex-cells>> is set to two by the `cols` attribute, and there are two cells entered in the table, the first row is complete.
Now, let's add two more cells to the table.

"#
    );

    // Two cells entered on separate lines flow into a single row because the
    // table has two columns.
    let doc = Parser::default().parse(
        "[cols=\"1,1\"]\n|===\n|This cell is in column 1, row 1\n|This cell is in column 2, row 1\n|===",
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
                    },
                ],
                header_row: None,
                body_rows: &[TableRow {
                    cells: &[
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "This cell is in column 1, row 1",
                                    line: 3,
                                    col: 2,
                                    offset: 19,
                                },
                                rendered: "This cell is in column 1, row 1",
                            }),
                        },
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "This cell is in column 2, row 1",
                                    line: 4,
                                    col: 2,
                                    offset: 52,
                                },
                                rendered: "This cell is in column 2, row 1",
                            }),
                        },
                    ],
                }],
                footer_row: None,
                source: Span {
                    data: "[cols=\"1,1\"]\n|===\n|This cell is in column 1, row 1\n|This cell is in column 2, row 1\n|===",
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
                data: "[cols=\"1,1\"]\n|===\n|This cell is in column 1, row 1\n|This cell is in column 2, row 1\n|===",
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
.Add two more cells to a table
[source#ex-more-cells]
----
[cols="1,1"]
|===
|This cell is in column 1, row 1
|This cell is in column 2, row 1
<.>
|This cell is in column 1, row 2 <.>
|    This cell is in column 2, row 2 <.>
|===
----
<.> Separate rows by one or more empty lines.
<.> The processor places this cell on the second row because the table has two columns and this is the third cell separator (`|`) it encounters.
<.> Any leading or trailing spaces around the cell content is stripped by the processor.

The table from <<ex-more-cells>> now has four cells arranged into two consecutive rows.

.Result of <<ex-more-cells>>
[cols="1,1"]
|===
|This cell is in column 1, row 1
|This cell is in column 2, row 1

|This cell is in column 1, row 2
|    This cell is in column 2, row 2
|===

The cells in a row can be entered on the same line or consecutive lines because the row a cell in placed on is determined by the number of columns in a table and the order in which the processor encounters the cell's separator (`|`).

"#
    );

    // Four cells flow into two rows of two. Rows are separated by one or more
    // empty lines, and leading/trailing whitespace around cell content is
    // stripped (the second cell of the second row is indented four spaces).
    let doc = Parser::default().parse(
        "[cols=\"1,1\"]\n|===\n|This cell is in column 1, row 1\n|This cell is in column 2, row 1\n\n|This cell is in column 1, row 2\n|    This cell is in column 2, row 2\n|===",
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
                    },
                ],
                header_row: None,
                body_rows: &[
                    TableRow {
                        cells: &[
                            TableCell {
                                content: TableCellContent::Simple(Content {
                                    original: Span {
                                        data: "This cell is in column 1, row 1",
                                        line: 3,
                                        col: 2,
                                        offset: 19,
                                    },
                                    rendered: "This cell is in column 1, row 1",
                                }),
                            },
                            TableCell {
                                content: TableCellContent::Simple(Content {
                                    original: Span {
                                        data: "This cell is in column 2, row 1",
                                        line: 4,
                                        col: 2,
                                        offset: 52,
                                    },
                                    rendered: "This cell is in column 2, row 1",
                                }),
                            },
                        ],
                    },
                    TableRow {
                        cells: &[
                            TableCell {
                                content: TableCellContent::Simple(Content {
                                    original: Span {
                                        data: "This cell is in column 1, row 2",
                                        line: 6,
                                        col: 2,
                                        offset: 86,
                                    },
                                    rendered: "This cell is in column 1, row 2",
                                }),
                            },
                            TableCell {
                                content: TableCellContent::Simple(Content {
                                    original: Span {
                                        data: "This cell is in column 2, row 2",
                                        line: 7,
                                        col: 6,
                                        offset: 123,
                                    },
                                    rendered: "This cell is in column 2, row 2",
                                }),
                            },
                        ],
                    },
                ],
                footer_row: None,
                source: Span {
                    data: "[cols=\"1,1\"]\n|===\n|This cell is in column 1, row 1\n|This cell is in column 2, row 1\n\n|This cell is in column 1, row 2\n|    This cell is in column 2, row 2\n|===",
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
                data: "[cols=\"1,1\"]\n|===\n|This cell is in column 1, row 1\n|This cell is in column 2, row 1\n\n|This cell is in column 1, row 2\n|    This cell is in column 2, row 2\n|===",
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
fn enter_a_rows_cells_on_a_single_line() {
    non_normative!(
        r#"
[#same]
== Enter a row's cells on a single line

"#
    );

    verifies!(
        r#"
You can enter a row's cells on a single line.
//This method is how the number or columns in a table are implicitly assigned and implicitly assign the `header` option to the table's first row.
When entering cells on a single line, *at least one space must be entered between the last character of the previous cell's content and the cell separator (`|`) of the next cell*.

.Cells entered on the same line
[source#ex-single-line]
----
|===
|Column 1 |Column 2 |Column 3 <.> <.>

|Cell in column 1, row 2 |Cell in column 2, row 2 |Cell in column 3, row 2 <.>

|Cell in column 1, row 3 <.>
|Cell in column 2, row 3 |Cell in column 3, row 3
|===
----
<.> Since `cols` is not set, the first row in this table must have the cells entered on a single line in order to implicitly assign three columns to the table.
<.> The first row is entered on the line directly after the opening table delimiter (`|===`) and followed by an empty line.
This automatically assigns the `header` option to it.
<.> When multiple cells are entered on a single line, enter at least one space between the last character of the previous cell's content and the cell separator (`|`) of the next cell.
<.> A row's cells can be entered on a combination of lines as long as the lines are consecutive.

The table created in <<ex-single-line>> contains three columns and three rows.

.Result of <<ex-single-line>>
|===
|Column 1 |Column 2 |Column 3

|Cell in column 1, row 2 |Cell in column 2, row 2 |Cell in column 3, row 2

|Cell in column 1, row 3
|Cell in column 2, row 3 |Cell in column 3, row 3
|===

Any leading and trailing spaces around the cell content are stripped and don't affect the table's layout when rendered.

"#
    );

    // `cols` is not set, so the three cells on the first line implicitly assign
    // three columns. That first line, followed by an empty line, is treated as
    // the header row. The remaining cells flow into two body rows, whether
    // entered on a single line or across consecutive lines.
    let doc = Parser::default().parse(
        "|===\n|Column 1 |Column 2 |Column 3\n\n|Cell in column 1, row 2 |Cell in column 2, row 2 |Cell in column 3, row 2\n\n|Cell in column 1, row 3\n|Cell in column 2, row 3 |Cell in column 3, row 3\n|===",
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
                    },
                    TableColumn {
                        width: 1,
                        h_align: HorizontalAlignment::Left,
                        v_align: VerticalAlignment::Top,
                        style: ColumnStyle::Default,
                    },
                ],
                header_row: Some(TableRow {
                    cells: &[
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "Column 1",
                                    line: 2,
                                    col: 2,
                                    offset: 6,
                                },
                                rendered: "Column 1",
                            }),
                        },
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "Column 2",
                                    line: 2,
                                    col: 12,
                                    offset: 16,
                                },
                                rendered: "Column 2",
                            }),
                        },
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "Column 3",
                                    line: 2,
                                    col: 22,
                                    offset: 26,
                                },
                                rendered: "Column 3",
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
                                        line: 4,
                                        col: 2,
                                        offset: 37,
                                    },
                                    rendered: "Cell in column 1, row 2",
                                }),
                            },
                            TableCell {
                                content: TableCellContent::Simple(Content {
                                    original: Span {
                                        data: "Cell in column 2, row 2",
                                        line: 4,
                                        col: 27,
                                        offset: 62,
                                    },
                                    rendered: "Cell in column 2, row 2",
                                }),
                            },
                            TableCell {
                                content: TableCellContent::Simple(Content {
                                    original: Span {
                                        data: "Cell in column 3, row 2",
                                        line: 4,
                                        col: 52,
                                        offset: 87,
                                    },
                                    rendered: "Cell in column 3, row 2",
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
                                        line: 6,
                                        col: 2,
                                        offset: 113,
                                    },
                                    rendered: "Cell in column 1, row 3",
                                }),
                            },
                            TableCell {
                                content: TableCellContent::Simple(Content {
                                    original: Span {
                                        data: "Cell in column 2, row 3",
                                        line: 7,
                                        col: 2,
                                        offset: 138,
                                    },
                                    rendered: "Cell in column 2, row 3",
                                }),
                            },
                            TableCell {
                                content: TableCellContent::Simple(Content {
                                    original: Span {
                                        data: "Cell in column 3, row 3",
                                        line: 7,
                                        col: 27,
                                        offset: 163,
                                    },
                                    rendered: "Cell in column 3, row 3",
                                }),
                            },
                        ],
                    },
                ],
                footer_row: None,
                source: Span {
                    data: "|===\n|Column 1 |Column 2 |Column 3\n\n|Cell in column 1, row 2 |Cell in column 2, row 2 |Cell in column 3, row 2\n\n|Cell in column 1, row 3\n|Cell in column 2, row 3 |Cell in column 3, row 3\n|===",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                caption: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            })],
            source: Span {
                data: "|===\n|Column 1 |Column 2 |Column 3\n\n|Cell in column 1, row 2 |Cell in column 2, row 2 |Cell in column 3, row 2\n\n|Cell in column 1, row 3\n|Cell in column 2, row 3 |Cell in column 3, row 3\n|===",
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
fn enter_a_rows_cells_on_consecutive_lines() {
    non_normative!(
        r#"
[#consecutive]
== Enter a row's cells on consecutive lines

The cells in a row can be entered on consecutive, individual lines.
When using this method, remember to either xref:add-columns.adoc[set the cols attribute] or xref:add-columns.adoc#implicit-cols[enter the first row's cells on a single line].

"#
    );

    verifies!(
        r#"
.Cells on consecutive, individual lines
[source#ex-consecutive-lines]
----
include::example$row.adoc[tag=indv]
----

The `cols` attribute in <<ex-consecutive-lines>> is assigned a xref:add-columns.adoc#column-multiplier[multiplier] of `+3*+`, indicating that this table has three columns.

.Result of <<ex-consecutive-lines>>
include::example$row.adoc[tag=indv]

Entering cells on consecutive lines can improve the readability (and debugging) of your raw AsciiDoc content when you're applying <<specifiers,specifiers to the cells>>, using xref:format-cell-content.adoc#a-operator[AsciiDoc block elements in the cells], or entering a lot of content into the cells.
"#
    );

    // The example pulls in `example$row.adoc[tag=indv]`, whose expanded content
    // is asserted here directly. The `cols="3*"` multiplier sets three columns,
    // and the cells (entered on consecutive, individual lines) flow into two
    // body rows. The first row is not a header because the line after the
    // opening delimiter is immediately followed by another cell, not a blank.
    let doc = Parser::default().parse(
        "[cols=\"3*\"]\n|===\n|Cell in column 1, row 1\n|Cell in column 2, row 1\n|Cell in column 3, row 1\n\n|Cell in column 1, row 2\n|Cell in column 2, row 2\n|Cell in column 3, row 2\n|===",
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
                    },
                    TableColumn {
                        width: 1,
                        h_align: HorizontalAlignment::Left,
                        v_align: VerticalAlignment::Top,
                        style: ColumnStyle::Default,
                    },
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
                                        offset: 18,
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
                                        offset: 43,
                                    },
                                    rendered: "Cell in column 2, row 1",
                                }),
                            },
                            TableCell {
                                content: TableCellContent::Simple(Content {
                                    original: Span {
                                        data: "Cell in column 3, row 1",
                                        line: 5,
                                        col: 2,
                                        offset: 68,
                                    },
                                    rendered: "Cell in column 3, row 1",
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
                                        line: 7,
                                        col: 2,
                                        offset: 94,
                                    },
                                    rendered: "Cell in column 1, row 2",
                                }),
                            },
                            TableCell {
                                content: TableCellContent::Simple(Content {
                                    original: Span {
                                        data: "Cell in column 2, row 2",
                                        line: 8,
                                        col: 2,
                                        offset: 119,
                                    },
                                    rendered: "Cell in column 2, row 2",
                                }),
                            },
                            TableCell {
                                content: TableCellContent::Simple(Content {
                                    original: Span {
                                        data: "Cell in column 3, row 2",
                                        line: 9,
                                        col: 2,
                                        offset: 144,
                                    },
                                    rendered: "Cell in column 3, row 2",
                                }),
                            },
                        ],
                    },
                ],
                footer_row: None,
                source: Span {
                    data: "[cols=\"3*\"]\n|===\n|Cell in column 1, row 1\n|Cell in column 2, row 1\n|Cell in column 3, row 1\n\n|Cell in column 1, row 2\n|Cell in column 2, row 2\n|Cell in column 3, row 2\n|===",
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
                        value: "3*",
                    }],
                    anchor: None,
                    source: Span {
                        data: "cols=\"3*\"",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                }),
            })],
            source: Span {
                data: "[cols=\"3*\"]\n|===\n|Cell in column 1, row 1\n|Cell in column 2, row 1\n|Cell in column 3, row 1\n\n|Cell in column 1, row 2\n|Cell in column 2, row 2\n|Cell in column 3, row 2\n|===",
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
