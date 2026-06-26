use crate::tests::prelude::*;

track_file!("docs/modules/tables/pages/duplicate-cells.adoc");

non_normative!(
    r#"
= Duplicate Cells
// clone, copy, replicate, duplicate

The contents of a cell can be duplicated in consecutive cells.

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

/// Collect the rendered text of each cell in each body row of `table`.
fn body_texts(table: &crate::blocks::TableBlock<'_>) -> Vec<Vec<String>> {
    table
        .body_rows()
        .iter()
        .map(|row| {
            row.cells()
                .iter()
                .map(|cell| match cell.content() {
                    crate::blocks::TableCellContent::Simple(content) => {
                        content.rendered().to_string()
                    }
                    crate::blocks::TableCellContent::AsciiDoc(_) => {
                        panic!("expected simple cell content")
                    }
                })
                .collect()
        })
        .collect()
}

/// Collect the [`ColumnStyle`] of each cell in each body row of `table`.
fn body_styles(table: &crate::blocks::TableBlock<'_>) -> Vec<Vec<crate::blocks::ColumnStyle>> {
    table
        .body_rows()
        .iter()
        .map(|row| row.cells().iter().map(|cell| cell.style()).collect())
        .collect()
}

#[test]
fn duplication_factor_and_operator() {
    non_normative!(
        r#"
== Duplication factor and operator

"#
    );

    verifies!(
        r#"
The duplication factor and operator are applied to a xref:add-cells-and-rows.adoc#specifiers[cell's specifier] and allow you to clone a cell's content and properties across consecutive cells.
A duplication is the first operator in a cell specifier.

====
<**duplication factor**><**duplication operator**><horizontal alignment operator><vertical alignment operator><style operator>|<cell's content>
====

The [.term]*duplication factor* is a single integer (`<n>`) that indicates how many times the cell's content should be duplicated.

The [.term]*duplication operator* is an asterisk (`+*+`) placed directly after the duplication factor (`+<n>*+`).
The duplication operator tells the converter to interpret the duplication factor as part of a duplication instead of a span.

"#
    );

    // The duplication factor is a single integer (`<n>`): a `<n>*` specifier
    // clones the cell's content into `<n>` consecutive cells. Here `2*` produces
    // two identical cells, leaving the trailing `|c` cell to complete the row.
    // (The implicit column count is three, because each clone is counted as a
    // separate column slot.)
    let table = parse_table("|===\n2*|dup |c\n|===");
    assert_eq!(table.columns().len(), 3);
    assert_eq!(
        body_texts(&table),
        vec![vec!["dup".to_string(), "dup".to_string(), "c".to_string()]]
    );

    // The duplication factor is interpreted as a duplication, not a span, so each
    // clone is an independent single-column cell (colspan 1) rather than one cell
    // stretched across two columns. Contrast the span operator (`+`), which makes
    // a single cell span the columns.
    let dup = parse_table("|===\n2*|x\n|===");
    assert_eq!(dup.body_rows()[0].cells().len(), 2);
    assert_eq!(dup.body_rows()[0].cells()[0].colspan(), 1);
    assert_eq!(dup.body_rows()[0].cells()[1].colspan(), 1);

    let span = parse_table("|===\n2+|x\n|===");
    assert_eq!(span.body_rows()[0].cells().len(), 1);
    assert_eq!(span.body_rows()[0].cells()[0].colspan(), 2);

    // A duplication is the first operator in the cell specifier: the duplication
    // factor and operator come before any alignment or style operator. Here `3*e`
    // is the duplication `3*` followed by the style operator `e`, so all three
    // clones are emphasized.
    let table = parse_table("|===\n3*e|x\n|===");
    assert_eq!(
        body_styles(&table),
        vec![vec![
            crate::blocks::ColumnStyle::Emphasis,
            crate::blocks::ColumnStyle::Emphasis,
            crate::blocks::ColumnStyle::Emphasis,
        ]]
    );
}

#[test]
fn duplicate_a_cell_and_its_properties() {
    non_normative!(
        r#"
== Duplicate a cell and its properties

"#
    );

    verifies!(
        r#"
To duplicate a cell, enter the duplication factor and duplication operator (`+<n>*+`) in the cell specifier.
Don't insert any spaces between the duplication, any alignment or style operators (if present), and the xref:add-cells-and-rows.adoc#cell-separator[cell's separator] (`|`).

.Duplicate the contents of two cells
[source#ex-clone]
----
include::example$cell.adoc[tag=clone]
----

The table from <<ex-clone>> is displayed below.

.Result of <<ex-clone>>
include::example$cell.adoc[tag=clone]
"#
    );

    // Expansion of `example$cell.adoc[tag=clone]`. The `2*` cell clones its
    // content into the first two columns of row 2, leaving the trailing `|Cell in
    // column 3, row 2` cell to finish that row. The `3*e` cell clones its
    // emphasized content into three cells: the first finishes row 3 (after the
    // two plain cells) and the next two open row 4, which is then completed by the
    // trailing `|Cell in column 3, row 4` cell.
    let table = parse_table(
        "|===\n|Column 1, header row |Column 2, header row |Column 3, header row\n\n2*|This cell is duplicated in columns 1 and 2 because its specifier contains a duplication of `2*`\n|Cell in column 3, row 2\n\n|Cell in column 1, row 3\n|Cell in column 2, row 3\n3*e|This cell specifier contains the duplication `3*` and style operator `e`.\n\nThe cell's text is italicized and duplicated in column 3, row 3 and columns 1 and 2 on row 4.\n\n|Cell in column 3, row 4\n|===",
    );

    assert_eq!(table.columns().len(), 3);
    assert_eq!(table.header_row().unwrap().cells().len(), 3);

    // The `2*` cell's content is cloned verbatim into both of the first two
    // columns; the `3*e` cell's content is cloned into three cells.
    let dup_text = "This cell is duplicated in columns 1 and 2 because its specifier contains a duplication of <code>2*</code>";
    let clone_text = "This cell specifier contains the duplication <code>3*</code> and style operator <code>e</code>.\n\nThe cell&#8217;s text is italicized and duplicated in column 3, row 3 and columns 1 and 2 on row 4.";
    assert_eq!(
        body_texts(&table),
        vec![
            vec![
                dup_text.to_string(),
                dup_text.to_string(),
                "Cell in column 3, row 2".to_string(),
            ],
            vec![
                "Cell in column 1, row 3".to_string(),
                "Cell in column 2, row 3".to_string(),
                clone_text.to_string(),
            ],
            vec![
                clone_text.to_string(),
                clone_text.to_string(),
                "Cell in column 3, row 4".to_string(),
            ],
        ]
    );

    // The duplication clones the cell's properties as well as its content: the
    // `3*e` style operator emphasizes every clone, while the cells with no style
    // operator keep the default style.
    assert_eq!(
        body_styles(&table),
        vec![
            vec![
                crate::blocks::ColumnStyle::Default,
                crate::blocks::ColumnStyle::Default,
                crate::blocks::ColumnStyle::Default,
            ],
            vec![
                crate::blocks::ColumnStyle::Default,
                crate::blocks::ColumnStyle::Default,
                crate::blocks::ColumnStyle::Emphasis,
            ],
            vec![
                crate::blocks::ColumnStyle::Emphasis,
                crate::blocks::ColumnStyle::Emphasis,
                crate::blocks::ColumnStyle::Default,
            ],
        ]
    );
}
