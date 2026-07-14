use crate::tests::prelude::*;

track_file!("ref/asciidoc-lang/docs/modules/tables/pages/add-header-row.adoc");

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

/// Return the rendered text of each cell in the table's header row, panicking
/// if the table has no header row.
fn header_texts(table: &crate::blocks::TableBlock<'_>) -> Vec<String> {
    table
        .header_row()
        .expect("expected a header row")
        .cells()
        .iter()
        .map(simple_text)
        .collect()
}

/// Return the `(horizontal, vertical)` alignment of each cell in the table's
/// header row, panicking if the table has no header row.
fn header_alignments(
    table: &crate::blocks::TableBlock<'_>,
) -> Vec<(HorizontalAlignment, VerticalAlignment)> {
    table
        .header_row()
        .expect("expected a header row")
        .cells()
        .iter()
        .map(|cell| (cell.h_align(), cell.v_align()))
        .collect()
}

non_normative!(
    r#"
= Create a Header Row

"#
);

#[test]
fn intro() {
    verifies!(
        r#"
The first row of a table is promoted to a header row if the `header` value is assigned to the table's `options` attribute.
You can assign `header` to a table's first row explicitly or implicitly.

"#
    );

    // Assigning `header` to the `options` attribute promotes the first row,
    // regardless of the table's layout.
    let explicit =
        parse_table("[options=\"header\"]\n|===\n|Column 1 |Column 2\n|Cell 1 |Cell 2\n|===");
    assert!(explicit.header_row().is_some());

    // The first row can also be promoted implicitly, based on the table's
    // layout (a non-empty first line followed by an empty line).
    let implicit = parse_table("|===\n|Column 1 |Column 2\n\n|Cell 1 |Cell 2\n|===");
    assert!(implicit.header_row().is_some());
}

#[test]
fn header_ignores_operators() {
    verifies!(
        r#"
TIP: The header row ignores any style operators assigned via column and cell specifiers.
"#
    );

    // A column style operator (here the AsciiDoc `a` operator) governs the body
    // cells in its column but is ignored for the header row: the header cell is
    // processed as plain, default content even though its column is styled.
    let table = parse_table(
        "[cols=\"a,1\",options=\"header\"]\n|===\n|Column 1 |Column 2\n\n|* item\n|Cell 2\n|===",
    );
    assert!(matches!(
        table.header_row().unwrap().cells()[0].content(),
        crate::blocks::TableCellContent::Simple(_)
    ));

    // The body cell in the same column *is* processed with the column's style,
    // so its content is parsed as a nested AsciiDoc list block.
    assert!(matches!(
        table.body_rows()[0].cells()[0].content(),
        crate::blocks::TableCellContent::AsciiDoc(_)
    ));

    verifies!(
        r#"
It also ignores alignment operators assigned to the table's column specifiers; however, any alignment operators assigned to a cell specifier in the header row are applied.

"#
    );

    // The columns carry alignment operators (`^` centers, `.>` bottom-aligns),
    // and the first row is promoted to the header. The header row ignores the
    // column alignment operators, so its cells fall back to the default
    // alignment (left, top) ...
    let table = parse_table(
        "[cols=\"^.>,^.>\",options=\"header\"]\n|===\n|Column 1 |Column 2\n\n|Cell 1 |Cell 2\n|===",
    );
    assert_eq!(
        header_alignments(&table),
        vec![
            (HorizontalAlignment::Left, VerticalAlignment::Top),
            (HorizontalAlignment::Left, VerticalAlignment::Top),
        ]
    );

    // ... while the body cells in the same columns *do* honor the column
    // alignment operators.
    assert_eq!(
        table.body_rows()[0]
            .cells()
            .iter()
            .map(|cell| (cell.h_align(), cell.v_align()))
            .collect::<Vec<_>>(),
        vec![
            (HorizontalAlignment::Center, VerticalAlignment::Bottom),
            (HorizontalAlignment::Center, VerticalAlignment::Bottom),
        ]
    );

    // Alignment operators on a cell specifier *in the header row* are applied.
    // Here the columns are centered (`^`), but the header cells carry their own
    // operators (`>` right, `.>` bottom), which override the ignored column
    // alignment; the horizontal alignment of the second header cell has no cell
    // operator, so it falls back to the default (left) rather than the column's
    // center.
    let table = parse_table(
        "[cols=\"2*^\",options=\"header\"]\n|===\n>|Column 1 .>|Column 2\n\n|Cell 1 |Cell 2\n|===",
    );
    assert_eq!(
        header_alignments(&table),
        vec![
            (HorizontalAlignment::Right, VerticalAlignment::Top),
            (HorizontalAlignment::Left, VerticalAlignment::Bottom),
        ]
    );

    // The body cells, which have no cell specifier, still honor the column's
    // center alignment.
    assert_eq!(
        table.body_rows()[0]
            .cells()
            .iter()
            .map(|cell| (cell.h_align(), cell.v_align()))
            .collect::<Vec<_>>(),
        vec![
            (HorizontalAlignment::Center, VerticalAlignment::Top),
            (HorizontalAlignment::Center, VerticalAlignment::Top),
        ]
    );
}

#[test]
fn explicitly_assign_header() {
    non_normative!(
        r#"
== Explicitly assign header to the first row

"#
    );

    verifies!(
        r#"
The header row semantics and styles are explicitly assigned to the first row in a table by assigning `header` to the `options` attribute.
The `options` attribute is set in the table's attribute list using the shorthand (`%value`) or formal syntax (`options="value"`).

"#
    );

    // Shorthand `%header` (entered before `cols`) promotes the first row; this
    // is the `ex-short` example from the page.
    let shorthand = parse_table(
        "[%header,cols=\"2,2,1\"]\n|===\n|Column 1, header row\n|Column 2, header row\n|Column 3, header row\n\n|Cell in column 1, row 2\n|Cell in column 2, row 2\n|Cell in column 3, row 2\n|===",
    );
    assert_eq!(
        header_texts(&shorthand),
        vec![
            "Column 1, header row",
            "Column 2, header row",
            "Column 3, header row"
        ]
    );
    assert_eq!(shorthand.body_rows().len(), 1);

    // Formal `options="header"` promotes the first row too; this is the `opt-h`
    // example (row.adoc) referenced by the page.
    let formal = parse_table(
        "[cols=\"2*\",options=\"header\"]\n|===\n|Column 1, header row\n|Column 2, header row\n\n|Cell in column 1, row 2\n|Cell in column 2, row 2\n\n|Cell in column 1, row 3\n|Cell in column 2, row 3\n|===",
    );
    assert_eq!(
        header_texts(&formal),
        vec!["Column 1, header row", "Column 2, header row"]
    );
    assert_eq!(formal.body_rows().len(), 2);

    verifies!(
        r#"
The `options` attribute is represented by the percent sign (`%`) when it's set using the shorthand syntax.
"#
    );

    // The `%` shorthand sets the `options` attribute, so `%header` assigns the
    // `header` option. (No blank line follows the first row here, so the header
    // can only come from the option, not from implicit detection.)
    let table = parse_table("[%header]\n|===\n|Column 1 |Column 2\n|Cell 1 |Cell 2\n|===");
    assert!(table.header_row().is_some());

    verifies!(
        r#"
In <<ex-short>>, `header` is assigned to using the shorthand syntax for `options`.

.Table with `header` assigned using the shorthand syntax
[source#ex-short]
----
[%header,cols="2,2,1"] <.>
|===
|Column 1, header row
|Column 2, header row
|Column 3, header row

|Cell in column 1, row 2
|Cell in column 2, row 2
|Cell in column 3, row 2
|===
----
"#
    );

    // The `ex-short` example assigns `header` using the `%` shorthand syntax, so
    // the first row is promoted to the header. (The callout `<.>` on the
    // attribute line is doc-authoring markup, not part of the table source.)
    let ex_short = parse_table(
        "[%header,cols=\"2,2,1\"]\n|===\n|Column 1, header row\n|Column 2, header row\n|Column 3, header row\n\n|Cell in column 1, row 2\n|Cell in column 2, row 2\n|Cell in column 3, row 2\n|===",
    );
    assert!(ex_short.header_row().is_some());

    verifies!(
        r#"
<.> Values assigned using the shorthand syntax must be entered before the `cols` attribute (or any other named attributes) in a table's attribute list, otherwise the processor will ignore them.

"#
    );

    // Shorthand `%header` entered before `cols` is honored.
    let before = parse_table(
        "[%header,cols=\"2,2,1\"]\n|===\n|Column 1 |Column 2 |Column 3\n|Cell 1 |Cell 2 |Cell 3\n|===",
    );
    assert!(before.header_row().is_some());

    // The same shorthand entered after the `cols` (named) attribute is ignored,
    // so the first row is not promoted. (No blank line follows it, so implicit
    // detection does not apply either.)
    let after = parse_table(
        "[cols=\"2,2,1\",%header]\n|===\n|Column 1 |Column 2 |Column 3\n|Cell 1 |Cell 2 |Cell 3\n|===",
    );
    assert!(after.header_row().is_none());

    verifies!(
        r#"
The table from <<ex-short>> is displayed below.

.Result of <<ex-short>>
[%header,cols="2,2,1"]
|===
|Column 1, header row
|Column 2, header row
|Column 3, header row

|Cell in column 1, row 2
|Cell in column 2, row 2
|Cell in column 3, row 2
|===

"#
    );

    // The rendered result of <<ex-short>>: the first row renders as the header.
    let ex_short_result = parse_table(
        "[%header,cols=\"2,2,1\"]\n|===\n|Column 1, header row\n|Column 2, header row\n|Column 3, header row\n\n|Cell in column 1, row 2\n|Cell in column 2, row 2\n|Cell in column 3, row 2\n|===",
    );
    assert_eq!(
        header_texts(&ex_short_result),
        vec![
            "Column 1, header row",
            "Column 2, header row",
            "Column 3, header row"
        ]
    );

    verifies!(
        r#"
In <<ex-formal>>, the `options` attribute is set and assigned the `header` value using the formal syntax.
The `options` attribute accepts a comma-separated list of values.

.Table with header assigned to the options attribute
[source#ex-formal]
----
include::example$row.adoc[tag=opt-h]
----

"#
    );

    // The `ex-formal` example (`opt-h` from row.adoc) sets the `options`
    // attribute to `header` using the formal syntax, promoting the first row.
    let ex_formal = parse_table(
        "[cols=\"2*\",options=\"header\"]\n|===\n|Column 1, header row\n|Column 2, header row\n\n|Cell in column 1, row 2\n|Cell in column 2, row 2\n\n|Cell in column 1, row 3\n|Cell in column 2, row 3\n|===",
    );
    assert!(ex_formal.header_row().is_some());

    // The formal `options` value is a comma-separated list; `header` is honored
    // when it appears among other values.
    let table = parse_table(
        "[options=\"header,unbreakable\"]\n|===\n|Column 1 |Column 2\n|Cell 1 |Cell 2\n|===",
    );
    assert!(table.header_row().is_some());

    verifies!(
        r#"
The first row of the table in <<ex-formal>> is rendered using the corresponding header styles and semantics.

.Result of <<ex-formal>>
include::example$row.adoc[tag=opt-h]

"#
    );

    // The rendered result of <<ex-formal>>: the first row renders as the header.
    let ex_formal_result = parse_table(
        "[cols=\"2*\",options=\"header\"]\n|===\n|Column 1, header row\n|Column 2, header row\n\n|Cell in column 1, row 2\n|Cell in column 2, row 2\n\n|Cell in column 1, row 3\n|Cell in column 2, row 3\n|===",
    );
    assert_eq!(
        header_texts(&ex_formal_result),
        vec!["Column 1, header row", "Column 2, header row"]
    );
}

#[test]
fn implicitly_assign_header() {
    non_normative!(
        r#"
== Implicitly assign header to the first row

"#
    );

    verifies!(
        r#"
You can implicitly define a header row based on how you layout the table.
The following conventions determine when the first row automatically becomes the header row:

. The first line of content inside the table delimiters is not empty.
. The second line of content inside the table delimiters is empty.

"#
    );

    // Both conventions hold: the first line of content is non-empty and the
    // second line is empty, so the first row becomes the header (the `impl-h`
    // example from row.adoc).
    let table = parse_table(
        "|===\n|Column 1, header row |Column 2, header row\n\n|Cell in column 1, row 2\n|Cell in column 2, row 2\n\n|Cell in column 1, row 3\n|Cell in column 2, row 3\n|===",
    );
    assert_eq!(
        header_texts(&table),
        vec!["Column 1, header row", "Column 2, header row"]
    );
    assert_eq!(table.body_rows().len(), 2);

    // When the second line of content is *not* empty, the convention fails and
    // the first row is not promoted to a header row.
    let no_header = parse_table("|===\n|Cell 1 |Cell 2\n|Cell 3 |Cell 4\n|===");
    assert!(no_header.header_row().is_none());

    verifies!(
        r#"
.First row is implicitly assigned header
[source#ex-implicit]
----
include::example$row.adoc[tag=impl-h]
----

"#
    );

    // The `ex-implicit` example (`impl-h` from row.adoc) relies on layout alone:
    // a non-empty first line followed by a blank line promotes the first row.
    let ex_implicit = parse_table(
        "|===\n|Column 1, header row |Column 2, header row\n\n|Cell in column 1, row 2\n|Cell in column 2, row 2\n\n|Cell in column 1, row 3\n|Cell in column 2, row 3\n|===",
    );
    assert!(ex_implicit.header_row().is_some());

    verifies!(
        r#"
As seen in the result below, if all of these rules hold true, then the first row of the table is treated as a header row.

.Result of <<ex-implicit>>
include::example$row.adoc[tag=impl-h]

"#
    );

    // The rendered result of <<ex-implicit>>: the first row renders as the header.
    let ex_implicit_result = parse_table(
        "|===\n|Column 1, header row |Column 2, header row\n\n|Cell in column 1, row 2\n|Cell in column 2, row 2\n\n|Cell in column 1, row 3\n|Cell in column 2, row 3\n|===",
    );
    assert_eq!(
        header_texts(&ex_implicit_result),
        vec!["Column 1, header row", "Column 2, header row"]
    );
}

#[test]
fn deactivate_implicit_header() {
    non_normative!(
        r#"
=== Deactivate the implicit assignment of header

"#
    );

    verifies!(
        r#"
To suppress the implicit behavior of promoting the first row to a header row, assign the value `noheader` to the `options` attribute using the formal (`options=noheader`) or shorthand (`%noheader`) syntax.
In <<ex-noheader>>, `noheader` is assigned using the shorthand syntax.

"#
    );

    // Shorthand `%noheader` suppresses the implicit header (the `ex-noheader`
    // example from the page): the first row stays in the body.
    let table = parse_table(
        "[%noheader]\n|===\n|Cell in column 1, row 1 |Cell in column 2, row 1\n\n|Cell in column 1, row 2 |Cell in column 2, row 2\n|===",
    );
    assert!(table.header_row().is_none());
    assert_eq!(table.body_rows().len(), 2);

    // Formal `options="noheader"` suppresses the implicit header as well.
    let table =
        parse_table("[options=\"noheader\"]\n|===\n|Cell 1 |Cell 2\n\n|Cell 3 |Cell 4\n|===");
    assert!(table.header_row().is_none());

    // `noheader` only suppresses the *implicit* assignment; an explicit `header`
    // still wins when both options are present.
    let table = parse_table("[%header%noheader]\n|===\n|Cell 1 |Cell 2\n\n|Cell 3 |Cell 4\n|===");
    assert!(table.header_row().is_some());

    verifies!(
        r#"
.Deactivate implicit header row with noheader
[source#ex-noheader]
----
[%noheader]
|===
|Cell in column 1, row 1 |Cell in column 2, row 1

|Cell in column 1, row 2 |Cell in column 2, row 2
|===
----

"#
    );

    // The `ex-noheader` example suppresses the implicit header via the `%noheader`
    // shorthand, so the first row stays in the body rather than being promoted.
    let ex_noheader = parse_table(
        "[%noheader]\n|===\n|Cell in column 1, row 1 |Cell in column 2, row 1\n\n|Cell in column 1, row 2 |Cell in column 2, row 2\n|===",
    );
    assert!(ex_noheader.header_row().is_none());
    assert_eq!(ex_noheader.body_rows().len(), 2);

    verifies!(
        r#"
The table from <<ex-noheader>> is displayed below.

.Result of <<ex-noheader>>
[%noheader]
|===
|Cell in column 1, row 1 |Cell in column 2, row 1

|Cell in column 1, row 2 |Cell in column 2, row 2
|===

"#
    );

    // The rendered result of <<ex-noheader>>: no header row is present and both
    // rows remain in the body.
    let ex_noheader_result = parse_table(
        "[%noheader]\n|===\n|Cell in column 1, row 1 |Cell in column 2, row 1\n\n|Cell in column 1, row 2 |Cell in column 2, row 2\n|===",
    );
    assert!(ex_noheader_result.header_row().is_none());
    assert_eq!(ex_noheader_result.body_rows().len(), 2);

    non_normative!(
        r#"
//CAUTION: We're considering using a similar convention for enabling the footer in the future.
//Thus, if you rely on this convention to enable the header row, it's advised that you not put all the cells in the last row on the same line unless you intend on making it the footer row.
"#
    );
}
