use crate::tests::prelude::*;

track_file!("docs/modules/tables/pages/add-columns.adoc");

non_normative!(
    r#"
= Add Columns to a Table

The number of columns in a table is specified by the `cols` attribute or <<implicit-cols,by the number of cells found in the first non-empty line>> after the opening table delimiter (`|===`).

"#
);

#[test]
fn specify_the_number_of_columns_with_the_cols_attribute() {
    non_normative!(
        r#"
[#cols-attribute]
== Specify the number of columns with the cols attribute

"#
    );

    verifies!(
        r#"
The `cols` attribute is set in the attribute list on a table block.
It accepts a comma-separated list of column specifiers.
[[col-specifier]]Each [.term]*column specifier* represents a column and the width, alignment, and style properties assigned to that column.
A column specifier is commonly represented by a number, but in some cases, can be represented by a symbol or letter.
In <<ex-cols>>, `cols` is assigned a list of four numeric column specifiers.

.Assign column specifiers to the cols attribute
[source#ex-cols]
----
[cols="1,1,1,1"]
----

In <<ex-cols>>, the value assigned to `cols`  contains four column specifiers.
The number of entries in the value's list determines the number of columns in the table.
That means the table in the above example will contain four columns.
When the specifier is a number, such as `1` or `50`, the integer represents the xref:adjust-column-widths.adoc[width of the column in proportion to the other columns in the table].
In <<ex-cols>>, each column will be the same width because the integer in each specifier is the same.
Let's look at the column specifiers in <<ex-cols-alt>> and compare it to <<ex-cols>>.

.Assign column specifiers to the cols attribute
[source#ex-cols-alt]
----
[cols="3,3,3,3"]
----

Both <<ex-cols>> and <<ex-cols-alt>> will produce tables with four columns of equal width.
Let's use the `cols` value in <<ex-cols-alt>> to create a table.

.Create a table with four columns of equal width
[source#ex-cols-table]
----
[cols="3,3,3,3"] <.>
|=== <.>
|Column 1 |Column 2 |Column 3 |Column 4 <.>
<.>
|Cell in column 1 <.>
|Cell in column 2
|Cell in column 3
|Cell in column 4
|=== <.>
----
<.> In an attribute list, set the `cols` attribute, followed by an equals sign (`=`), and then a list of comma-separated column specifiers enclosed in double quotation marks (`"`).
<.> On the line directly after the attribute list, enter the opening table delimiter.
A table delimiter is one vertical bar followed by three equals signs (`|===`).
<.> A table cell is specified by a vertical bar (`|`).
Since four consecutive cells are entered on the first line directly after the delimiter, this row is implicitly set as the table's header row.
<.> Insert an empty line after the header row.
<.> The cells for the next row can be entered on a single line or on individual lines.
<.> On a new line after the last cell of the last row, enter another table delimiter (`|===`) to close the table block.

<<ex-cols-table>> creates the table displayed below.

.Result of <<ex-cols-table>>
[cols="3,3,3,3"]
|===
|Column 1 |Column 2 |Column 3 |Column 4

|Cell in column 1
|Cell in column 2
|Cell in column 3
|Cell in column 4
|===

As specified, the table includes four columns of equal width, a header row, and a regular row.
Since all of the columns in <<ex-cols-table>> are assigned the same width via their column specifiers (i.e., `3`), the number of columns could be specified with a <<column-multiplier,column multiplier>>.
Or, you could adjust the width of an individual column by xref:adjust-column-widths.adoc[increasing the numerical value of its specifier].

"#
    );

    let doc = Parser::default().parse(
        "[cols=\"3,3,3,3\"]\n|===\n|Column 1 |Column 2 |Column 3 |Column 4\n\n|Cell in column 1\n|Cell in column 2\n|Cell in column 3\n|Cell in column 4\n|===",
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
                    },
                    TableColumn {
                        width: 3,
                        h_align: HorizontalAlignment::Left,
                        v_align: VerticalAlignment::Top,
                    },
                    TableColumn {
                        width: 3,
                        h_align: HorizontalAlignment::Left,
                        v_align: VerticalAlignment::Top,
                    },
                    TableColumn {
                        width: 3,
                        h_align: HorizontalAlignment::Left,
                        v_align: VerticalAlignment::Top,
                    },
                ],
                header_row: Some(TableRow {
                    cells: &[
                        TableCell {
                            content: Content {
                                original: Span {
                                    data: "Column 1",
                                    line: 3,
                                    col: 2,
                                    offset: 23,
                                },
                                rendered: "Column 1",
                            },
                        },
                        TableCell {
                            content: Content {
                                original: Span {
                                    data: "Column 2",
                                    line: 3,
                                    col: 12,
                                    offset: 33,
                                },
                                rendered: "Column 2",
                            },
                        },
                        TableCell {
                            content: Content {
                                original: Span {
                                    data: "Column 3",
                                    line: 3,
                                    col: 22,
                                    offset: 43,
                                },
                                rendered: "Column 3",
                            },
                        },
                        TableCell {
                            content: Content {
                                original: Span {
                                    data: "Column 4",
                                    line: 3,
                                    col: 32,
                                    offset: 53,
                                },
                                rendered: "Column 4",
                            },
                        },
                    ],
                }),
                body_rows: &[TableRow {
                    cells: &[
                        TableCell {
                            content: Content {
                                original: Span {
                                    data: "Cell in column 1",
                                    line: 5,
                                    col: 2,
                                    offset: 64,
                                },
                                rendered: "Cell in column 1",
                            },
                        },
                        TableCell {
                            content: Content {
                                original: Span {
                                    data: "Cell in column 2",
                                    line: 6,
                                    col: 2,
                                    offset: 82,
                                },
                                rendered: "Cell in column 2",
                            },
                        },
                        TableCell {
                            content: Content {
                                original: Span {
                                    data: "Cell in column 3",
                                    line: 7,
                                    col: 2,
                                    offset: 100,
                                },
                                rendered: "Cell in column 3",
                            },
                        },
                        TableCell {
                            content: Content {
                                original: Span {
                                    data: "Cell in column 4",
                                    line: 8,
                                    col: 2,
                                    offset: 118,
                                },
                                rendered: "Cell in column 4",
                            },
                        },
                    ],
                }],
                footer_row: None,
                source: Span {
                    data: "[cols=\"3,3,3,3\"]\n|===\n|Column 1 |Column 2 |Column 3 |Column 4\n\n|Cell in column 1\n|Cell in column 2\n|Cell in column 3\n|Cell in column 4\n|===",
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
                        value: "3,3,3,3",
                    }],
                    anchor: None,
                    source: Span {
                        data: "cols=\"3,3,3,3\"",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                }),
            })],
            source: Span {
                data: "[cols=\"3,3,3,3\"]\n|===\n|Column 1 |Column 2 |Column 3 |Column 4\n\n|Cell in column 1\n|Cell in column 2\n|Cell in column 3\n|Cell in column 4\n|===",
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
fn using_a_column_multiplier() {
    non_normative!(
        r#"
[#column-multiplier]
=== Using a column multiplier

"#
    );

    verifies!(
        r#"
A [.term]*column multiplier* allows you to apply the same width, horizontal alignment, vertical alignment, and content style to multiple, consecutive columns in a table.
A multiplier consists of an integer (`<n>`) and an asterisk (`+*+`).
The integer represents the number of consecutive columns to be added to the table.
The asterisk (`+*+`) is called the [.term]*multiplier operator* and is placed directly after the integer (`+<n>*+`).
The operator tells the converter to interpret the integer as part of a column multiplier instead of a column specifier.

For example, let's rewrite the value of `[cols="5,5,5"]` as a column multiplier.

.Represent [cols="5,5,5"] using a column multiplier
[source]
----
[cols="3*"] <.>
----
<.> Assign an integer to `cols` that represents the number of columns in the table.
Enter the multiplier operator (`+*+`) directly after the integer.

The integer `3`, combined with the `+*+` operator, indicates that the table will contain three columns of equal width.

"#
    );

    let doc = Parser::default().parse("[cols=\"3*\"]\n|===\n|Column 1 |Column 2 |Column 3\n|===");

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
                    },
                    TableColumn {
                        width: 1,
                        h_align: HorizontalAlignment::Left,
                        v_align: VerticalAlignment::Top,
                    },
                    TableColumn {
                        width: 1,
                        h_align: HorizontalAlignment::Left,
                        v_align: VerticalAlignment::Top,
                    },
                ],
                header_row: None,
                body_rows: &[TableRow {
                    cells: &[
                        TableCell {
                            content: Content {
                                original: Span {
                                    data: "Column 1",
                                    line: 3,
                                    col: 2,
                                    offset: 18,
                                },
                                rendered: "Column 1",
                            },
                        },
                        TableCell {
                            content: Content {
                                original: Span {
                                    data: "Column 2",
                                    line: 3,
                                    col: 12,
                                    offset: 28,
                                },
                                rendered: "Column 2",
                            },
                        },
                        TableCell {
                            content: Content {
                                original: Span {
                                    data: "Column 3",
                                    line: 3,
                                    col: 22,
                                    offset: 38,
                                },
                                rendered: "Column 3",
                            },
                        },
                    ],
                }],
                footer_row: None,
                source: Span {
                    data: "[cols=\"3*\"]\n|===\n|Column 1 |Column 2 |Column 3\n|===",
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
                data: "[cols=\"3*\"]\n|===\n|Column 1 |Column 2 |Column 3\n|===",
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
fn combine_a_column_specifier_and_a_column_multiplier() {
    verifies!(
        r#"
You can use a multiplier in a comma-separated list with column specifiers, too.
In <<ex-spec-and-multiplier>>, the first column is represented by a column specifier, and the next three columns are represented by a multiplier.

.Assign a column specifier and a column multiplier to cols
[source#ex-spec-and-multiplier]
----
[cols="5,3*"]
|===
|Column 1 |Column 2 |Column 3 |Column 4

|Cell in column 1
|Cell in column 2
|Cell in column 3
|Cell in column 4
|===
----

As shown below, <<ex-spec-and-multiplier>> creates a table containing a xref:adjust-column-widths.adoc[wide first column] followed by three columns of equal width.

.Result of <<ex-spec-and-multiplier>>
[cols="5,3*"]
|===
|Column 1 |Column 2 |Column 3 |Column 4

|Cell in column 1
|Cell in column 2
|Cell in column 3
|Cell in column 4
|===

"#
    );

    let doc = Parser::default().parse(
        "[cols=\"5,3*\"]\n|===\n|Column 1 |Column 2 |Column 3 |Column 4\n\n|Cell in column 1\n|Cell in column 2\n|Cell in column 3\n|Cell in column 4\n|===",
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
                        width: 5,
                        h_align: HorizontalAlignment::Left,
                        v_align: VerticalAlignment::Top,
                    },
                    TableColumn {
                        width: 1,
                        h_align: HorizontalAlignment::Left,
                        v_align: VerticalAlignment::Top,
                    },
                    TableColumn {
                        width: 1,
                        h_align: HorizontalAlignment::Left,
                        v_align: VerticalAlignment::Top,
                    },
                    TableColumn {
                        width: 1,
                        h_align: HorizontalAlignment::Left,
                        v_align: VerticalAlignment::Top,
                    },
                ],
                header_row: Some(TableRow {
                    cells: &[
                        TableCell {
                            content: Content {
                                original: Span {
                                    data: "Column 1",
                                    line: 3,
                                    col: 2,
                                    offset: 20,
                                },
                                rendered: "Column 1",
                            },
                        },
                        TableCell {
                            content: Content {
                                original: Span {
                                    data: "Column 2",
                                    line: 3,
                                    col: 12,
                                    offset: 30,
                                },
                                rendered: "Column 2",
                            },
                        },
                        TableCell {
                            content: Content {
                                original: Span {
                                    data: "Column 3",
                                    line: 3,
                                    col: 22,
                                    offset: 40,
                                },
                                rendered: "Column 3",
                            },
                        },
                        TableCell {
                            content: Content {
                                original: Span {
                                    data: "Column 4",
                                    line: 3,
                                    col: 32,
                                    offset: 50,
                                },
                                rendered: "Column 4",
                            },
                        },
                    ],
                }),
                body_rows: &[TableRow {
                    cells: &[
                        TableCell {
                            content: Content {
                                original: Span {
                                    data: "Cell in column 1",
                                    line: 5,
                                    col: 2,
                                    offset: 61,
                                },
                                rendered: "Cell in column 1",
                            },
                        },
                        TableCell {
                            content: Content {
                                original: Span {
                                    data: "Cell in column 2",
                                    line: 6,
                                    col: 2,
                                    offset: 79,
                                },
                                rendered: "Cell in column 2",
                            },
                        },
                        TableCell {
                            content: Content {
                                original: Span {
                                    data: "Cell in column 3",
                                    line: 7,
                                    col: 2,
                                    offset: 97,
                                },
                                rendered: "Cell in column 3",
                            },
                        },
                        TableCell {
                            content: Content {
                                original: Span {
                                    data: "Cell in column 4",
                                    line: 8,
                                    col: 2,
                                    offset: 115,
                                },
                                rendered: "Cell in column 4",
                            },
                        },
                    ],
                }],
                footer_row: None,
                source: Span {
                    data: "[cols=\"5,3*\"]\n|===\n|Column 1 |Column 2 |Column 3 |Column 4\n\n|Cell in column 1\n|Cell in column 2\n|Cell in column 3\n|Cell in column 4\n|===",
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
                        value: "5,3*",
                    }],
                    anchor: None,
                    source: Span {
                        data: "cols=\"5,3*\"",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                }),
            })],
            source: Span {
                data: "[cols=\"5,3*\"]\n|===\n|Column 1 |Column 2 |Column 3 |Column 4\n\n|Cell in column 1\n|Cell in column 2\n|Cell in column 3\n|Cell in column 4\n|===",
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

non_normative!(
    r#"
[#cols-format]
=== Alignment and style column operators

AsciiDoc provides operators that control the positioning and style of column content when the `cols` attribute is set.
A column specifier or multiplier can contain these optional operators for one or more of the following properties:

* xref:align-by-column.adoc#horizontal-operators[horizontal alignment]
* xref:align-by-column.adoc#vertical-operators[vertical alignment]
* xref:format-column-content.adoc[content style]

Many of these operators can be applied to individual cells as well.

"#
);

#[test]
fn specify_the_number_of_columns_using_the_first_row() {
    non_normative!(
        r#"
[#implicit-cols]
== Specify the number of columns using the first row

"#
    );

    verifies!(
        r#"
When all of the columns in a table use the default width, alignment, and style values, you don't need to set the `cols` attribute.
Instead, you can implicitly declare the number of columns by entering all of the first row's cells on the same line.
The processor will derive the number columns from the number of cells in this row.
<<ex-implicit>> uses its first row to indicate that it has three columns.

.Create a table with three columns using its first row
[source#ex-implicit]
----
|===
<.>
|Cell in column 1, row 1 |Cell in column 2, row 1 |Cell in column 3, row 1 <.>

|Cell in column 1, row 2 <.>
|Cell in column 2, row 2
|Cell in column 3, row 2
|===
----
<.> After the opening delimiter, insert an empty line before the first row, unless you want the first row to be treated as header row.
<.> Enter all of the first row's cells on a single line.
Each cell represents one column.
<.> The cells in subsequent rows don't need to be entered on a single line.

The table in <<ex-implicit>> has three columns since its first row contains three cells.

.Result of <<ex-implicit>>
|===

|Cell in column 1, row 1 |Cell in column 2, row 1 |Cell in column 3, row 1

|Cell in column 1, row 2 |Cell in column 2, row 2 |Cell in column 3, row 2
|===
"#
    );

    let doc = Parser::default().parse(
        "|===\n\n|Cell in column 1, row 1 |Cell in column 2, row 1 |Cell in column 3, row 1\n\n|Cell in column 1, row 2 |Cell in column 2, row 2 |Cell in column 3, row 2\n|===",
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
                    },
                    TableColumn {
                        width: 1,
                        h_align: HorizontalAlignment::Left,
                        v_align: VerticalAlignment::Top,
                    },
                    TableColumn {
                        width: 1,
                        h_align: HorizontalAlignment::Left,
                        v_align: VerticalAlignment::Top,
                    },
                ],
                header_row: None,
                body_rows: &[
                    TableRow {
                        cells: &[
                            TableCell {
                                content: Content {
                                    original: Span {
                                        data: "Cell in column 1, row 1",
                                        line: 3,
                                        col: 2,
                                        offset: 7,
                                    },
                                    rendered: "Cell in column 1, row 1",
                                },
                            },
                            TableCell {
                                content: Content {
                                    original: Span {
                                        data: "Cell in column 2, row 1",
                                        line: 3,
                                        col: 27,
                                        offset: 32,
                                    },
                                    rendered: "Cell in column 2, row 1",
                                },
                            },
                            TableCell {
                                content: Content {
                                    original: Span {
                                        data: "Cell in column 3, row 1",
                                        line: 3,
                                        col: 52,
                                        offset: 57,
                                    },
                                    rendered: "Cell in column 3, row 1",
                                },
                            },
                        ],
                    },
                    TableRow {
                        cells: &[
                            TableCell {
                                content: Content {
                                    original: Span {
                                        data: "Cell in column 1, row 2",
                                        line: 5,
                                        col: 2,
                                        offset: 83,
                                    },
                                    rendered: "Cell in column 1, row 2",
                                },
                            },
                            TableCell {
                                content: Content {
                                    original: Span {
                                        data: "Cell in column 2, row 2",
                                        line: 5,
                                        col: 27,
                                        offset: 108,
                                    },
                                    rendered: "Cell in column 2, row 2",
                                },
                            },
                            TableCell {
                                content: Content {
                                    original: Span {
                                        data: "Cell in column 3, row 2",
                                        line: 5,
                                        col: 52,
                                        offset: 133,
                                    },
                                    rendered: "Cell in column 3, row 2",
                                },
                            },
                        ],
                    },
                ],
                footer_row: None,
                source: Span {
                    data: "|===\n\n|Cell in column 1, row 1 |Cell in column 2, row 1 |Cell in column 3, row 1\n\n|Cell in column 1, row 2 |Cell in column 2, row 2 |Cell in column 3, row 2\n|===",
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
                data: "|===\n\n|Cell in column 1, row 1 |Cell in column 2, row 1 |Cell in column 3, row 1\n\n|Cell in column 1, row 2 |Cell in column 2, row 2 |Cell in column 3, row 2\n|===",
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
