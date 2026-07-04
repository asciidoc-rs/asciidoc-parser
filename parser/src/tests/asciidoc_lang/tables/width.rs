use crate::tests::prelude::*;

track_file!("ref/asciidoc-lang/docs/modules/tables/pages/width.adoc");

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

/// Collect each column's `(width, autowidth)` pair.
fn column_widths(table: &crate::blocks::TableBlock<'_>) -> Vec<(usize, bool)> {
    table
        .columns()
        .iter()
        .map(|c| (c.width(), c.is_autowidth()))
        .collect()
}

// Expansion of `include::example$row.adoc[tag=base-h]`: the standard
// three-column table with a header row and two body rows used throughout the
// table documentation.
const BASE_H: &str = "|===\n|Column 1, header row |Column 2, header row |Column 3, header row\n\n|Cell in column 1, row 2\n|Cell in column 2, row 2\n|Cell in column 3, row 2\n\n|Cell in column 1, row 3\n|Cell in column 2, row 3\n|Cell in column 3, row 3\n|===";

non_normative!(
    r#"
= Table Width

"#
);

#[test]
fn fixed_width() {
    verifies!(
        r#"
By default, a table will span the width of the content area.

"#
    );

    // With no `width` attribute the table has no fixed width (it spans the
    // content area) and is not autowidth.
    let table = parse_table(BASE_H);
    assert_eq!(table.width(), None);
    assert!(!table.is_autowidth());

    non_normative!(
        r#"
== Fixed width

"#
    );

    verifies!(
        r#"
To constrain the width of the table to a fixed value, set the `width` attribute in the table's attribute list.
The width is an integer percentage value ranging from 1 to 100.
The `%` sign is optional.

"#
    );

    // The `width` attribute constrains the table to a fixed percentage; the
    // trailing `%` is optional, so `75%` and `75` are equivalent.
    let src = format!("[width=75%]\n{BASE_H}");
    assert_eq!(parse_table(&src).width(), Some(75));

    let src = format!("[width=75]\n{BASE_H}");
    assert_eq!(parse_table(&src).width(), Some(75));

    // A value outside the 1-to-100 range (or one that isn't an integer) is
    // ignored.
    let src = format!("[width=0]\n{BASE_H}");
    assert_eq!(parse_table(&src).width(), None);
    let src = format!("[width=150]\n{BASE_H}");
    assert_eq!(parse_table(&src).width(), None);
    let src = format!("[width=wide]\n{BASE_H}");
    assert_eq!(parse_table(&src).width(), None);

    verifies!(
        r#"
.Table with width set to 75%
[source#ex-fixed]
----
[width=75%]
include::example$row.adoc[tag=base-h]
----

.Result of <<ex-fixed>>
[width=75%]
include::example$row.adoc[tag=base-h]

"#
    );

    // The fixed width applies to the standard three-column table; the columns
    // themselves keep their default proportional width and are not autowidth.
    let src = format!("[width=75%]\n{BASE_H}");
    let table = parse_table(&src);
    assert_eq!(table.width(), Some(75));
    assert_eq!(
        column_widths(&table),
        vec![(1, false), (1, false), (1, false)]
    );
    assert!(table.header_row().is_some());
    assert_eq!(table.body_rows().len(), 2);
}

#[test]
fn autowidth() {
    non_normative!(
        r#"
== Autowidth

"#
    );

    verifies!(
        r#"
Alternately, you can make the width fit the content by setting the `autowidth` option.
The columns inherit this setting, so individual columns will also be sized according to the content.

"#
    );

    // The `autowidth` option makes the table fit its content; it has no fixed
    // width, and every column inherits the setting (each becomes autowidth).
    let src = format!("[%autowidth]\n{BASE_H}");
    let table = parse_table(&src);
    assert!(table.is_autowidth());
    assert_eq!(table.width(), None);
    assert_eq!(column_widths(&table), vec![(1, true), (1, true), (1, true)]);

    verifies!(
        r#"
.Table using autowidth
[source#ex-auto]
----
[%autowidth]
include::example$row.adoc[tag=base-h]
----

.Result of <<ex-auto>>
[%autowidth]
include::example$row.adoc[tag=base-h]

"#
    );

    // The autowidth table from <<ex-auto>> applied to the standard three-column
    // table: the table is autowidth, every column is autowidth, and the header
    // and body rows are otherwise unchanged.
    let src = format!("[%autowidth]\n{BASE_H}");
    let table = parse_table(&src);
    assert!(table.is_autowidth());
    assert_eq!(column_widths(&table), vec![(1, true), (1, true), (1, true)]);
    assert!(table.header_row().is_some());
    assert_eq!(table.body_rows().len(), 2);

    verifies!(
        r#"
If you want each column to have an automatic width, but want the table to span the width of the content area, add the `stretch` role to the table.
(Alternatively, you can set the `width` attribute to `100%`.)

"#
    );

    // Adding the `stretch` role keeps the columns autowidth while the role
    // records the intent to span the content area.
    let src = format!("[%autowidth.stretch]\n{BASE_H}");
    let table = parse_table(&src);
    assert!(table.is_autowidth());
    assert_eq!(column_widths(&table), vec![(1, true), (1, true), (1, true)]);
    assert!(table.attrlist().unwrap().roles().contains(&"stretch"));

    // Alternatively, setting the `width` attribute to `100%` spans the content
    // area.
    let src = format!("[width=100%]\n{BASE_H}");
    let table = parse_table(&src);
    assert_eq!(table.width(), Some(100));

    verifies!(
        r#"
.Full-width table with autowidth columns
[source#ex-stretch]
----
[%autowidth.stretch]
include::example$row.adoc[tag=base-h]
----

The table from <<ex-stretch>> is rendered below.
The columns are sized to the content, but the table spans the width of the page.

.Result of <<ex-stretch>>
[%autowidth.stretch]
include::example$row.adoc[tag=base-h]

"#
    );

    // <<ex-stretch>>: the columns are sized to their content (each autowidth)
    // while the `stretch` role records that the table spans the full page
    // width.
    let src = format!("[%autowidth.stretch]\n{BASE_H}");
    let table = parse_table(&src);
    assert!(table.is_autowidth());
    assert_eq!(column_widths(&table), vec![(1, true), (1, true), (1, true)]);
    assert!(table.attrlist().unwrap().roles().contains(&"stretch"));

    // The DocBook converter is out of scope for this parser, so the warning
    // below describes downstream behavior we don't model.
    non_normative!(
        r#"
WARNING: The `autowidth` option is not recognized by the DocBook converter.

"#
    );
}

#[test]
fn mix_fixed_and_autowidth_columns() {
    non_normative!(
        r#"
== Mix fixed and autowidth columns

"#
    );

    verifies!(
        r#"
If you want to apply `autowidth` only to certain columns, use the special value `~` as the width of the column.
In this case, width values are assumed to be a percentage value (i.e., 100-based).

"#
    );

    verifies!(
        r#"
.Table with fixed and autowidth columns
[source#ex-mix]
----
[cols="25h,~,~"]
|===
|small |as big as the column needs to be |the rest
|===
----

.Result of <<ex-mix>>
[cols="25h,~,~"]
|===
|small |as big as the column needs to be |the rest
|===
"#
    );

    // <<ex-mix>>: the `~` width value marks an individual column as autowidth;
    // the other columns keep their explicit (percentage-based) width. Here the
    // first column is a 25% header column and the remaining two are autowidth.
    let table = parse_table(
        "[cols=\"25h,~,~\"]\n|===\n|small |as big as the column needs to be |the rest\n|===",
    );
    assert_eq!(
        column_widths(&table),
        vec![(25, false), (1, true), (1, true)]
    );
    assert_eq!(table.columns()[0].style(), ColumnStyle::Header);

    // Only the `~` columns are autowidth; the table itself carries no
    // `autowidth` option.
    assert!(!table.is_autowidth());
}
