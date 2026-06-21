use crate::tests::prelude::*;

track_file!("docs/modules/tables/pages/add-footer-row.adoc");

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

/// Return the rendered text of a [`Simple`](crate::blocks::TableCellContent)
/// cell, panicking if the cell holds AsciiDoc block content instead.
fn simple_text(cell: &crate::blocks::TableCell<'_>) -> String {
    match cell.content() {
        crate::blocks::TableCellContent::Simple(content) => content.rendered().to_string(),
        crate::blocks::TableCellContent::AsciiDoc(_) => panic!("expected simple cell content"),
    }
}

/// Return the rendered text of each cell in the table's footer row, panicking
/// if the table has no footer row.
fn footer_texts(table: &crate::blocks::TableBlock<'_>) -> Vec<String> {
    table
        .footer_row()
        .expect("expected a footer row")
        .cells()
        .iter()
        .map(simple_text)
        .collect()
}

non_normative!(
    r#"
= Create a Footer Row

"#
);

#[test]
fn intro() {
    verifies!(
        r#"
The last row of a table is promoted to a footer row if the `footer` value is assigned to the table's `options` attribute.

"#
    );

    // Assigning `footer` to the `options` attribute promotes the last row,
    // moving it out of the body and into the footer. (The second line of
    // content is non-empty, so no implicit header is detected and both rows
    // start out in the body.)
    let table = parse_table("[options=\"footer\"]\n|===\n|Cell 1 |Cell 2\n|Foot 1 |Foot 2\n|===");
    assert_eq!(footer_texts(&table), vec!["Foot 1", "Foot 2"]);
    assert_eq!(table.body_rows().len(), 1);

    // Without the `footer` option, the last row stays in the body.
    let no_footer = parse_table("|===\n|Cell 1 |Cell 2\n|Foot 1 |Foot 2\n|===");
    assert!(no_footer.footer_row().is_none());
}

#[test]
fn assign_footer_to_last_row() {
    non_normative!(
        r#"
== Assign footer to the last row

"#
    );

    verifies!(
        r#"
The footer row semantics and styles are applied to the last row in a table by assigning `footer` to the `options` attribute.
The `options` attribute is set in the table's attribute list using the shorthand (`%value`) or formal syntax (`options="value"`).

"#
    );

    // Shorthand `%footer` promotes the last row to the footer.
    let shorthand = parse_table("[%footer]\n|===\n|Cell 1 |Cell 2\n|Foot 1 |Foot 2\n|===");
    assert_eq!(footer_texts(&shorthand), vec!["Foot 1", "Foot 2"]);

    // Formal `options="footer"` promotes the last row too.
    let formal = parse_table("[options=\"footer\"]\n|===\n|Cell 1 |Cell 2\n|Foot 1 |Foot 2\n|===");
    assert_eq!(footer_texts(&formal), vec!["Foot 1", "Foot 2"]);

    // Unlike the header row (which ignores column style operators), a footer
    // cell is processed with its column's style: here the `a` operator parses
    // the footer cell's content as a nested AsciiDoc list block.
    let styled =
        parse_table("[%footer,cols=\"a,1\"]\n|===\n|Cell 1 |Cell 2\n|* item\n|Foot 2\n|===");
    assert!(matches!(
        styled.footer_row().unwrap().cells()[0].content(),
        crate::blocks::TableCellContent::AsciiDoc(_)
    ));

    verifies!(
        r#"
The `options` attribute is represented by the percent sign (`%`) when it's set using the shorthand syntax.
"#
    );

    // The `%` shorthand sets the `options` attribute, so `%footer` assigns the
    // `footer` option and promotes the last row.
    let table = parse_table("[%footer]\n|===\n|Cell 1 |Cell 2\n|Foot 1 |Foot 2\n|===");
    assert!(table.footer_row().is_some());

    verifies!(
        r#"
In <<ex-short>>, `footer` is assigned using the shorthand syntax for `options`.

.Table with footer assigned using the shorthand syntax
[source#ex-short]
----
[%header%footer,cols="2,2,1"] <.>
|===
|Column 1, header row
|Column 2, header row
|Column 3, header row

|Cell in column 1, row 2
|Cell in column 2, row 2
|Cell in column 3, row 2

|Column 1, footer row
|Column 2, footer row
|Column 3, footer row
|===
----
"#
    );

    // The `ex-short` example assigns `footer` (alongside `header`) using the
    // `%` shorthand syntax, so the last row is promoted to the footer.
    let ex_short = parse_table(
        "[%header%footer,cols=\"2,2,1\"]\n|===\n|Column 1, header row\n|Column 2, header row\n|Column 3, header row\n\n|Cell in column 1, row 2\n|Cell in column 2, row 2\n|Cell in column 3, row 2\n\n|Column 1, footer row\n|Column 2, footer row\n|Column 3, footer row\n|===",
    );
    assert!(ex_short.footer_row().is_some());

    verifies!(
        r#"
<.> Values assigned using the shorthand syntax must be entered before the `cols` attribute (or any other named attributes) in a table's attribute list, otherwise the processor will ignore them.

"#
    );

    // Shorthand `%header%footer` entered before `cols` is honored: the first row
    // is the header and the last row is the footer (this is the `ex-short`
    // example from the page).
    let before = parse_table(
        "[%header%footer,cols=\"2,2,1\"]\n|===\n|Column 1, header row\n|Column 2, header row\n|Column 3, header row\n\n|Cell in column 1, row 2\n|Cell in column 2, row 2\n|Cell in column 3, row 2\n\n|Column 1, footer row\n|Column 2, footer row\n|Column 3, footer row\n|===",
    );
    assert!(before.header_row().is_some());
    assert_eq!(
        footer_texts(&before),
        vec![
            "Column 1, footer row",
            "Column 2, footer row",
            "Column 3, footer row"
        ]
    );
    assert_eq!(before.body_rows().len(), 1);

    // The same shorthand entered after the `cols` (named) attribute is ignored,
    // so the last row is not promoted to a footer row.
    let after = parse_table(
        "[cols=\"2,2,1\",%footer]\n|===\n|Cell 1 |Cell 2 |Cell 3\n\n|Foot 1 |Foot 2 |Foot 3\n|===",
    );
    assert!(after.footer_row().is_none());

    verifies!(
        r#"
The table from <<ex-short>> is displayed below.

.Result of <<ex-short>>
[%header%footer,cols="2,2,1"]
|===
|Column 1, header row
|Column 2, header row
|Column 3, header row

|Cell in column 1, row 2
|Cell in column 2, row 2
|Cell in column 3, row 2

|Column 1, footer row
|Column 2, footer row
|Column 3, footer row
|===

"#
    );

    // The rendered result of <<ex-short>>: the last row renders as the footer.
    let ex_short_result = parse_table(
        "[%header%footer,cols=\"2,2,1\"]\n|===\n|Column 1, header row\n|Column 2, header row\n|Column 3, header row\n\n|Cell in column 1, row 2\n|Cell in column 2, row 2\n|Cell in column 3, row 2\n\n|Column 1, footer row\n|Column 2, footer row\n|Column 3, footer row\n|===",
    );
    assert_eq!(
        footer_texts(&ex_short_result),
        vec![
            "Column 1, footer row",
            "Column 2, footer row",
            "Column 3, footer row"
        ]
    );

    verifies!(
        r#"
In <<ex-formal>>, the `options` attribute is set and assigned the `footer` value using the formal syntax.
The `options` attribute accepts a comma-separated list of values.

.Table with footer assigned to the options attribute
[source#ex-formal]
----
include::example$row.adoc[tag=opt-f]
----

"#
    );

    // The `ex-formal` example sets the `options` attribute to `footer` using the
    // formal syntax, promoting the last row to the footer.
    let ex_formal =
        parse_table("[options=\"footer\"]\n|===\n|Cell 1 |Cell 2\n|Foot 1 |Foot 2\n|===");
    assert!(ex_formal.footer_row().is_some());

    // The formal `options` value is a comma-separated list; `footer` is honored
    // when it appears among other values.
    let table = parse_table(
        "[options=\"footer,unbreakable\"]\n|===\n|Cell 1 |Cell 2\n|Foot 1 |Foot 2\n|===",
    );
    assert!(table.footer_row().is_some());

    verifies!(
        r#"
The last row of the table in <<ex-formal>> is rendered using the corresponding footer styles.

.Result of <<ex-formal>>
include::example$row.adoc[tag=opt-f]
"#
    );

    // The `opt-f` example from row.adoc: an implicit header (first line
    // non-empty, second line blank) plus the formal `footer` option, so the
    // first row is the header, the middle row is the body, and the last row is
    // the footer.
    let formal = parse_table(
        "[options=\"footer\"]\n|===\n|Column 1, header row |Column 2, header row\n\n|Cell in column 1, row 2\n|Cell in column 2, row 2\n\n|Column 1, footer row\n|Column 2, footer row\n|===",
    );
    assert!(formal.header_row().is_some());
    assert_eq!(formal.body_rows().len(), 1);
    assert_eq!(
        footer_texts(&formal),
        vec!["Column 1, footer row", "Column 2, footer row"]
    );
}
