use crate::tests::prelude::*;

track_file!("docs/modules/tables/pages/span-cells.adoc");

non_normative!(
    r#"
= Span Columns and Rows

A table cell can span more than one column and row.

"#
);

/// Parse `source` as a single block and return the [`TableBlock`] it produced.
///
/// [`TableBlock`]: crate::blocks::TableBlock
fn parse_table(source: &str) -> crate::blocks::TableBlock<'_> {
    let mut parser = Parser::default();
    let mi = crate::blocks::Block::parse(crate::Span::new(source), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    match mi.item {
        crate::blocks::Block::Table(table) => table,
        other => panic!("expected a table block, got {other:?}"),
    }
}

/// Collect the `(colspan, rowspan)` of each cell in each body row of `table`.
fn body_spans(table: &crate::blocks::TableBlock<'_>) -> Vec<Vec<(usize, usize)>> {
    table
        .body_rows()
        .iter()
        .map(|row| {
            row.cells()
                .iter()
                .map(|cell| (cell.colspan(), cell.rowspan()))
                .collect()
        })
        .collect()
}

#[test]
fn span_factor_and_operator() {
    non_normative!(
        r#"
== Span factor and operator

"#
    );

    verifies!(
        r#"
With a [.term]*span* a table cell can stretch across adjacent columns, rows, or a block of adjacent columns and rows.
A span consists of a span factor and a span operator.

The [.term]*span factor* indicates the number columns, rows, or columns and rows a cell should span.

[[col-factor]]Column span factor:: A single integer (`<n>`) that represents the number of consecutive columns a cell should span.
[[row-factor]]Row span factor:: A single integer prefixed with a dot (`.<n>`) that represents the number of consecutive rows a cell should span.
[[block-factor]]Block span factor:: Two integers (`<n>.<n>`) that represent a block of adjacent columns and rows a cell should span.
The first integer, `<n>`, is the column span factor.
The second integer, which is prefixed with a dot, `.<n>`, is the row span factor.

The [.term]*span operator* is a plus sign (`\+`) placed directly after the span factor (`<n>.<n>+`).
The span operator tells the converter to interpret the span factor as part of a span instead of a duplication.

A span is the first operator in a xref:add-cells-and-rows.adoc#specifiers[cell specifier].

====
<**span factor**><**span operator**><horizontal alignment operator><vertical alignment operator><style operator>|<cell's content>
====

"#
    );

    // A column span factor (`<n>`) followed by the span operator (`+`) makes the
    // cell span `<n>` consecutive columns (and a single row).
    let table = parse_table("|===\n3+|x\n|===");
    assert_eq!(body_spans(&table), vec![vec![(3, 1)]]);

    // A row span factor (`.<n>`) followed by the span operator makes the cell
    // span `<n>` consecutive rows (and a single column).
    let table = parse_table("|===\n.2+|x\n|===");
    assert_eq!(body_spans(&table), vec![vec![(1, 2)]]);

    // A block span factor (`<n>.<n>`): the first integer is the column span and
    // the second (dot-prefixed) integer is the row span.
    let table = parse_table("|===\n2.3+|x\n|===");
    assert_eq!(body_spans(&table), vec![vec![(2, 3)]]);
}

#[test]
fn span_multiple_columns() {
    non_normative!(
        r#"
== Span multiple columns

"#
    );

    verifies!(
        r#"
To have a cell span consecutive columns, enter the <<col-factor,column span factor>> and span operator (`<n>+`) in the cell specifier.
Don't insert any spaces between the span, any alignment or style operators (if present), and the xref:add-cells-and-rows.adoc#cell-separator[cell's separator] (`|`).

.Span three columns with a cell
[source#ex-span-columns]
----
include::example$cell.adoc[tag=span-cols]
----

The table from <<ex-span-columns>> is displayed below.

.Result of <<ex-span-columns>>
include::example$cell.adoc[tag=span-cols]

"#
    );

    // Expansion of `example$cell.adoc[tag=span-cols]`. The `3+` cell spans the
    // first three columns of its row, leaving room for a single trailing cell;
    // the rows above and below it are ordinary four-cell rows.
    let table = parse_table(
        "|===\n|Column 1, header row |Column 2, header row |Column 3, header row |Column 4, header row\n\n3+|This cell spans columns 1, 2, and 3 because its specifier contains a span of `3+`\n|Cell in column 4, row 2\n\n|Cell in column 1, row 3\n|Cell in column 2, row 3\n|Cell in column 3, row 3\n|Cell in column 4, row 3\n|===",
    );

    assert_eq!(table.columns().len(), 4);
    assert_eq!(table.header_row().unwrap().cells().len(), 4);
    assert_eq!(
        body_spans(&table),
        vec![vec![(3, 1), (1, 1)], vec![(1, 1), (1, 1), (1, 1), (1, 1)],]
    );
}

#[test]
fn span_multiple_rows() {
    non_normative!(
        r#"
== Span multiple rows

"#
    );

    verifies!(
        r#"
To have a cell span consecutive rows, enter the <<row-factor,row span factor>> and span operator (`.<n>+`) in the cell specifier.
Remember to prefix the span factor with a dot (`.`).
Don't insert any spaces between the span, any alignment or style operators (if present), and the xref:add-cells-and-rows.adoc#cell-separator[cell's separator] (`|`).

.Span two rows with a cell
[source#ex-span-rows]
----
include::example$cell.adoc[tag=span-rows]
----

The table from <<ex-span-rows>> is displayed below.

.Result of <<ex-span-rows>>
include::example$cell.adoc[tag=span-rows]

"#
    );

    // Expansion of `example$cell.adoc[tag=span-rows]`. The `.2+` cell spans rows
    // 2 and 3 in the first column, so it carries that column down into row 3,
    // which then needs only its single remaining cell.
    let table = parse_table(
        "|===\n|Column 1, header row |Column 2, header row\n\n.2+|This cell spans rows 2 and 3 because its specifier contains a span of `.2+`\n|Cell in column 2, row 2\n\n|Cell in column 2, row 3\n\n|Cell in column 1, row 4\n|Cell in column 2, row 4\n|===",
    );

    assert_eq!(table.columns().len(), 2);
    assert_eq!(table.header_row().unwrap().cells().len(), 2);
    assert_eq!(
        body_spans(&table),
        vec![vec![(1, 2), (1, 1)], vec![(1, 1)], vec![(1, 1), (1, 1)],]
    );
}

#[test]
fn span_columns_and_rows() {
    non_normative!(
        r#"
== Span columns and rows

"#
    );

    verifies!(
        r#"
A single cell can span a block of adjacent columns and rows.
Enter the column span factor (`<n>`), followed by the row span factor (`.<n>`), and then the span operator (`+`).

.Span two columns and three rows with a single cell
[source#ex-block]
----
include::example$cell.adoc[tag=span-block]
----

The table from <<ex-block>> is displayed below.

.Result of <<ex-block>>
include::example$cell.adoc[tag=span-block]
"#
    );

    // Expansion of `example$cell.adoc[tag=span-block]`. The `2.3+` cell spans
    // columns 2 and 3 across rows 2, 3, and 4, so it carries two columns down
    // into the two rows below it; each of those rows then needs only its two
    // remaining (first- and last-column) cells.
    let table = parse_table(
        "|===\n|Column 1, header row |Column 2, header row |Column 3, header row |Column 4, header row\n\n|Cell in column 1, row 2\n2.3+|This cell spans columns 2 and 3 and rows 2, 3, and 4 because its specifier contains a span of `2.3+`\n|Cell in column 4, row 2\n\n|Cell in column 1, row 3\n|Cell in column 4, row 3\n\n|Cell in column 1, row 4\n|Cell in column 4, row 4\n|===",
    );

    assert_eq!(table.columns().len(), 4);
    assert_eq!(table.header_row().unwrap().cells().len(), 4);
    assert_eq!(
        body_spans(&table),
        vec![
            vec![(1, 1), (2, 3), (1, 1)],
            vec![(1, 1), (1, 1)],
            vec![(1, 1), (1, 1)],
        ]
    );
}
