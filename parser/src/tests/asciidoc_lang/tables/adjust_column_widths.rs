use crate::tests::prelude::*;

track_file!("docs/modules/tables/pages/adjust-column-widths.adoc");

non_normative!(
    r#"
= Adjust Column Widths
// Check "proportional" usage

"#
);

#[test]
fn column_width() {
    non_normative!(
        r#"
== Column width

"#
    );

    verifies!(
        r#"
The width of a column is assigned by its xref:add-columns.adoc#col-specifier[column specifier].
The value of a column's width is either an integer or a percentage.
The default column width is `1`.
The integer or percentage represents the width of the column in proportion to the other columns within the total width of the table.
The total width of a table is backend dependent.
When using the HTML5 backend with the default Asciidoctor stylesheet, tables stretch the width of the page body unless the xref:width.adoc[table width attribute] is explicitly set.

"#
    );

    let doc = Parser::default().parse("|===\n|Column 1 |Column 2 |Column 3\n|===");

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
                body_rows: &[TableRow {
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
                },],
                footer_row: None,
                source: Span {
                    data: "|===\n|Column 1 |Column 2 |Column 3\n|===",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                caption: None,
                number: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },),],
            source: Span {
                data: "|===\n|Column 1 |Column 2 |Column 3\n|===",
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
fn assign_column_widths_using_integers() {
    non_normative!(
        r#"
== Assign column widths using integers

"#
    );

    verifies!(
        r#"
To assign widths to the columns in a table, set the `cols` attribute and assign it a list of comma-separated column specifiers using integers.

.Assign column widths using integers
[source#ex-int]
----
[cols="2,1,3"]
|===
|Column 1 |Column 2 |Column 3

|This column has a proportional width of 2
|This column has a proportional width of 1
|This column has a proportional width of 3
|===
----

As seen below, the columns stretch across the width of the page according to their proportional widths.

.Result of <<ex-int>>
[cols="2,1,3"]
|===
|Column 1 |Column 2 |Column 3

|This column has a proportional width of 2
|This column has a proportional width of 1
|This column has a proportional width of 3
|===

"#
    );

    let doc = Parser::default().parse("[cols=\"2,1,3\"]\n|===\n|Column 1 |Column 2 |Column 3\n\n|This column has a proportional width of 2\n|This column has a proportional width of 1\n|This column has a proportional width of 3\n|===");

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
                        width: 2,
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
                        width: 3,
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
                                    line: 3,
                                    col: 2,
                                    offset: 21,
                                },
                                rendered: "Column 1",
                            }),
                        },
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "Column 2",
                                    line: 3,
                                    col: 12,
                                    offset: 31,
                                },
                                rendered: "Column 2",
                            }),
                        },
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "Column 3",
                                    line: 3,
                                    col: 22,
                                    offset: 41,
                                },
                                rendered: "Column 3",
                            }),
                        },
                    ],
                },),
                body_rows: &[TableRow {
                    cells: &[
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "This column has a proportional width of 2",
                                    line: 5,
                                    col: 2,
                                    offset: 52,
                                },
                                rendered: "This column has a proportional width of 2",
                            }),
                        },
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "This column has a proportional width of 1",
                                    line: 6,
                                    col: 2,
                                    offset: 95,
                                },
                                rendered: "This column has a proportional width of 1",
                            }),
                        },
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "This column has a proportional width of 3",
                                    line: 7,
                                    col: 2,
                                    offset: 138,
                                },
                                rendered: "This column has a proportional width of 3",
                            }),
                        },
                    ],
                },],
                footer_row: None,
                source: Span {
                    data: "[cols=\"2,1,3\"]\n|===\n|Column 1 |Column 2 |Column 3\n\n|This column has a proportional width of 2\n|This column has a proportional width of 1\n|This column has a proportional width of 3\n|===",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                caption: None,
                number: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: Some(Attrlist {
                    attributes: &[ElementAttribute {
                        name: Some("cols",),
                        value: "2,1,3",
                        shorthand_items: &[],
                    },],
                    anchor: None,
                    source: Span {
                        data: "cols=\"2,1,3\"",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },),],
            source: Span {
                data: "[cols=\"2,1,3\"]\n|===\n|Column 1 |Column 2 |Column 3\n\n|This column has a proportional width of 2\n|This column has a proportional width of 1\n|This column has a proportional width of 3\n|===",
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
fn increase_or_decrease_the_width_of_a_column() {
    non_normative!(
        r#"
=== Increase or decrease the width of a column

"#
    );

    verifies!(
        r#"
To increase the width of a column, use a bigger integer in the column's specifier.
Let's make column 1 from <<ex-int>> the largest column in the table by increasing its width from `2` to `6` in <<ex-increase>>.

.Increase the width of a column
[source#ex-increase]
----
[cols="6,1,3"]
|===
|Column 1 |Column 2 |Column 3

|This column has a proportional width of 6
|This column has a proportional width of 1
|This column has a proportional width of 3
|===
----

Below, the result of <<ex-increase>> shows that column 1 is now much wider than column 3.

.Result of <<ex-increase>>
[cols="6,1,3"]
|===
|Column 1 |Column 2 |Column 3

|This column has a proportional width of 6
|This column has a proportional width of 1
|This column has a proportional width of 3
|===

To decrease the width of a column, use a smaller integer in its specifier.
In <<ex-decrease>>, let's make the width of column 3 smaller, but not quite as small as column 2, by decreasing its width from `3` to `2`.

.Decrease the width of a column
[source#ex-decrease]
----
[cols="6,1,2"]
|===
|Column 1 |Column 2 |Column 3

|This column has a proportional width of 6
|This column has a proportional width of 1
|This column has a proportional width of 2
|===
----

The columns, displayed in the table below, have adjusted across the width of the page according to their proportional widths.

.Result of <<ex-decrease>>
[cols="6,1,2"]
|===
|Column 1 |Column 2 |Column 3

|This column has a proportional width of 6
|This column has a proportional width of 1
|This column has a proportional width of 2
|===

"#
    );

    let doc = Parser::default().parse("[cols=\"6,1,3\"]\n|===\n|Column 1 |Column 2 |Column 3\n\n|This column has a proportional width of 6\n|This column has a proportional width of 1\n|This column has a proportional width of 3\n|===");

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
                        width: 6,
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
                        width: 3,
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
                                    line: 3,
                                    col: 2,
                                    offset: 21,
                                },
                                rendered: "Column 1",
                            }),
                        },
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "Column 2",
                                    line: 3,
                                    col: 12,
                                    offset: 31,
                                },
                                rendered: "Column 2",
                            }),
                        },
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "Column 3",
                                    line: 3,
                                    col: 22,
                                    offset: 41,
                                },
                                rendered: "Column 3",
                            }),
                        },
                    ],
                },),
                body_rows: &[TableRow {
                    cells: &[
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "This column has a proportional width of 6",
                                    line: 5,
                                    col: 2,
                                    offset: 52,
                                },
                                rendered: "This column has a proportional width of 6",
                            }),
                        },
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "This column has a proportional width of 1",
                                    line: 6,
                                    col: 2,
                                    offset: 95,
                                },
                                rendered: "This column has a proportional width of 1",
                            }),
                        },
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "This column has a proportional width of 3",
                                    line: 7,
                                    col: 2,
                                    offset: 138,
                                },
                                rendered: "This column has a proportional width of 3",
                            }),
                        },
                    ],
                },],
                footer_row: None,
                source: Span {
                    data: "[cols=\"6,1,3\"]\n|===\n|Column 1 |Column 2 |Column 3\n\n|This column has a proportional width of 6\n|This column has a proportional width of 1\n|This column has a proportional width of 3\n|===",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                caption: None,
                number: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: Some(Attrlist {
                    attributes: &[ElementAttribute {
                        name: Some("cols",),
                        value: "6,1,3",
                        shorthand_items: &[],
                    },],
                    anchor: None,
                    source: Span {
                        data: "cols=\"6,1,3\"",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },),],
            source: Span {
                data: "[cols=\"6,1,3\"]\n|===\n|Column 1 |Column 2 |Column 3\n\n|This column has a proportional width of 6\n|This column has a proportional width of 1\n|This column has a proportional width of 3\n|===",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog::default(),
        }
    );

    let doc = Parser::default().parse("[cols=\"6,1,2\"]\n|===\n|Column 1 |Column 2 |Column 3\n\n|This column has a proportional width of 6\n|This column has a proportional width of 1\n|This column has a proportional width of 2\n|===");

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
                        width: 6,
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
                        width: 2,
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
                                    line: 3,
                                    col: 2,
                                    offset: 21,
                                },
                                rendered: "Column 1",
                            }),
                        },
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "Column 2",
                                    line: 3,
                                    col: 12,
                                    offset: 31,
                                },
                                rendered: "Column 2",
                            }),
                        },
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "Column 3",
                                    line: 3,
                                    col: 22,
                                    offset: 41,
                                },
                                rendered: "Column 3",
                            }),
                        },
                    ],
                },),
                body_rows: &[TableRow {
                    cells: &[
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "This column has a proportional width of 6",
                                    line: 5,
                                    col: 2,
                                    offset: 52,
                                },
                                rendered: "This column has a proportional width of 6",
                            }),
                        },
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "This column has a proportional width of 1",
                                    line: 6,
                                    col: 2,
                                    offset: 95,
                                },
                                rendered: "This column has a proportional width of 1",
                            }),
                        },
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "This column has a proportional width of 2",
                                    line: 7,
                                    col: 2,
                                    offset: 138,
                                },
                                rendered: "This column has a proportional width of 2",
                            }),
                        },
                    ],
                },],
                footer_row: None,
                source: Span {
                    data: "[cols=\"6,1,2\"]\n|===\n|Column 1 |Column 2 |Column 3\n\n|This column has a proportional width of 6\n|This column has a proportional width of 1\n|This column has a proportional width of 2\n|===",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                caption: None,
                number: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: Some(Attrlist {
                    attributes: &[ElementAttribute {
                        name: Some("cols",),
                        value: "6,1,2",
                        shorthand_items: &[],
                    },],
                    anchor: None,
                    source: Span {
                        data: "cols=\"6,1,2\"",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },),],
            source: Span {
                data: "[cols=\"6,1,2\"]\n|===\n|Column 1 |Column 2 |Column 3\n\n|This column has a proportional width of 6\n|This column has a proportional width of 1\n|This column has a proportional width of 2\n|===",
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
fn change_column_widths_using_percentage_values() {
    non_normative!(
        r#"
== Change column widths using percentage values

"#
    );

    verifies!(
        r#"
Column widths can be assigned using a percentage between `1%` and `100%`.
Like with integer values, set `cols` and assign it a list of comma-separated column specifiers using percentages.

.Assign column widths using percentages
[source#ex-percent]
----
[cols="15%,30%,55%"]
|===
|Column 1 |Column 2 |Column 3

|This column has a width of 15%
|This column has a width of 30%
|This column has a width of 55%
|===
----

As seen in the table below, the columns stretch across the width of the page according to the percentage assigned via their column specifiers.

.Result of <<ex-percent>>
[cols="15%,30%,55%"]
|===
|Column 1 |Column 2 |Column 3

|This column has a width of 15%
|This column has a width of 30%
|This column has a width of 55%
|===

When assigning percentages to `cols`, you don't have to include the percent sign (`%`).
For instance, both `[cols="15%,30%,55%"]` and `[cols="15,30,55"]` are valid.
"#
    );

    let doc = Parser::default().parse("[cols=\"15%,30%,55%\"]\n|===\n|Column 1 |Column 2 |Column 3\n\n|This column has a width of 15%\n|This column has a width of 30%\n|This column has a width of 55%\n|===");

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
                        width: 15,
                        h_align: HorizontalAlignment::Left,
                        v_align: VerticalAlignment::Top,
                        style: ColumnStyle::Default,
                    },
                    TableColumn {
                        width: 30,
                        h_align: HorizontalAlignment::Left,
                        v_align: VerticalAlignment::Top,
                        style: ColumnStyle::Default,
                    },
                    TableColumn {
                        width: 55,
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
                                    line: 3,
                                    col: 2,
                                    offset: 27,
                                },
                                rendered: "Column 1",
                            }),
                        },
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "Column 2",
                                    line: 3,
                                    col: 12,
                                    offset: 37,
                                },
                                rendered: "Column 2",
                            }),
                        },
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "Column 3",
                                    line: 3,
                                    col: 22,
                                    offset: 47,
                                },
                                rendered: "Column 3",
                            }),
                        },
                    ],
                },),
                body_rows: &[TableRow {
                    cells: &[
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "This column has a width of 15%",
                                    line: 5,
                                    col: 2,
                                    offset: 58,
                                },
                                rendered: "This column has a width of 15%",
                            }),
                        },
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "This column has a width of 30%",
                                    line: 6,
                                    col: 2,
                                    offset: 90,
                                },
                                rendered: "This column has a width of 30%",
                            }),
                        },
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "This column has a width of 55%",
                                    line: 7,
                                    col: 2,
                                    offset: 122,
                                },
                                rendered: "This column has a width of 55%",
                            }),
                        },
                    ],
                },],
                footer_row: None,
                source: Span {
                    data: "[cols=\"15%,30%,55%\"]\n|===\n|Column 1 |Column 2 |Column 3\n\n|This column has a width of 15%\n|This column has a width of 30%\n|This column has a width of 55%\n|===",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                caption: None,
                number: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: Some(Attrlist {
                    attributes: &[ElementAttribute {
                        name: Some("cols",),
                        value: "15%,30%,55%",
                        shorthand_items: &[],
                    },],
                    anchor: None,
                    source: Span {
                        data: "cols=\"15%,30%,55%\"",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },),],
            source: Span {
                data: "[cols=\"15%,30%,55%\"]\n|===\n|Column 1 |Column 2 |Column 3\n\n|This column has a width of 15%\n|This column has a width of 30%\n|This column has a width of 55%\n|===",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog::default(),
        }
    );

    let doc = Parser::default().parse("[cols=\"15,30,55\"]\n|===\n|Column 1 |Column 2 |Column 3\n\n|This column has a width of 15%\n|This column has a width of 30%\n|This column has a width of 55%\n|===");

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
                        width: 15,
                        h_align: HorizontalAlignment::Left,
                        v_align: VerticalAlignment::Top,
                        style: ColumnStyle::Default,
                    },
                    TableColumn {
                        width: 30,
                        h_align: HorizontalAlignment::Left,
                        v_align: VerticalAlignment::Top,
                        style: ColumnStyle::Default,
                    },
                    TableColumn {
                        width: 55,
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
                                    line: 3,
                                    col: 2,
                                    offset: 24,
                                },
                                rendered: "Column 1",
                            }),
                        },
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "Column 2",
                                    line: 3,
                                    col: 12,
                                    offset: 34,
                                },
                                rendered: "Column 2",
                            }),
                        },
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "Column 3",
                                    line: 3,
                                    col: 22,
                                    offset: 44,
                                },
                                rendered: "Column 3",
                            }),
                        },
                    ],
                },),
                body_rows: &[TableRow {
                    cells: &[
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "This column has a width of 15%",
                                    line: 5,
                                    col: 2,
                                    offset: 55,
                                },
                                rendered: "This column has a width of 15%",
                            }),
                        },
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "This column has a width of 30%",
                                    line: 6,
                                    col: 2,
                                    offset: 87,
                                },
                                rendered: "This column has a width of 30%",
                            }),
                        },
                        TableCell {
                            content: TableCellContent::Simple(Content {
                                original: Span {
                                    data: "This column has a width of 55%",
                                    line: 7,
                                    col: 2,
                                    offset: 119,
                                },
                                rendered: "This column has a width of 55%",
                            }),
                        },
                    ],
                },],
                footer_row: None,
                source: Span {
                    data: "[cols=\"15,30,55\"]\n|===\n|Column 1 |Column 2 |Column 3\n\n|This column has a width of 15%\n|This column has a width of 30%\n|This column has a width of 55%\n|===",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                caption: None,
                number: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: Some(Attrlist {
                    attributes: &[ElementAttribute {
                        name: Some("cols",),
                        value: "15,30,55",
                        shorthand_items: &[],
                    },],
                    anchor: None,
                    source: Span {
                        data: "cols=\"15,30,55\"",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },),],
            source: Span {
                data: "[cols=\"15,30,55\"]\n|===\n|Column 1 |Column 2 |Column 3\n\n|This column has a width of 15%\n|This column has a width of 30%\n|This column has a width of 55%\n|===",
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
