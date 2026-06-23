use crate::tests::prelude::*;

track_file!("docs/modules/tables/pages/borders.adoc");

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

/// Return the first [`TableBlock`] in `doc`.
///
/// Used by the tests that exercise the `table-frame` / `table-grid` document
/// attributes: those attributes must be set in the document header, so the
/// whole document is parsed rather than a single block.
///
/// [`TableBlock`]: crate::blocks::TableBlock
fn first_table<'a>(doc: &'a crate::Document<'a>) -> &'a crate::blocks::TableBlock<'a> {
    doc.nested_blocks()
        .find_map(|block| match block {
            crate::blocks::Block::Table(table) => Some(table),
            _ => None,
        })
        .expect("expected a table block")
}

// Expansion of `include::example$row.adoc[tag=base-h]`: the standard
// three-column table with a header row and two body rows used throughout the
// table documentation.
const BASE_H: &str = "|===\n|Column 1, header row |Column 2, header row |Column 3, header row\n\n|Cell in column 1, row 2\n|Cell in column 2, row 2\n|Cell in column 3, row 2\n\n|Cell in column 1, row 3\n|Cell in column 2, row 3\n|Cell in column 3, row 3\n|===";

non_normative!(
    r#"
= Table Borders
:!table-frame:
:!table-grid:

"#
);

#[test]
fn intro() {
    verifies!(
        r#"
The borders on a table are controlled using the `frame` and `grid` block attributes.
You can combine these two attributes to achieve a variety of border styles for your tables.

"#
    );

    // With no `frame` or `grid` attribute, a table draws a border on every side
    // and between every cell.
    let table = parse_table(BASE_H);
    assert_eq!(table.frame(), Frame::All);
    assert_eq!(table.grid(), Grid::All);
}

#[test]
fn frame() {
    non_normative!(
        r#"
== Frame

"#
    );

    verifies!(
        r#"
The border around a table is controlled using the `frame` attribute on the table.
The frame attribute defaults to the value `all` (implied value), which draws a border on each side of the table.
This default can be changed by setting the `table-frame` document attribute.
You can override the default value by setting the frame attribute on the table to the value `all`, `ends`, `sides` or `none`.

"#
    );

    // The frame defaults to `all`, and the implied (empty) value resolves the
    // same way.
    assert_eq!(parse_table(BASE_H).frame(), Frame::All);
    let src = format!("[frame=all]\n{BASE_H}");
    assert_eq!(parse_table(&src).frame(), Frame::All);

    // The `table-frame` document attribute changes the default for tables that
    // don't set their own `frame`.
    let src = format!(":table-frame: sides\n\n{BASE_H}");
    let doc = Parser::default().parse(&src);
    assert_eq!(first_table(&doc).frame(), Frame::Sides);

    // An explicit `frame` on the table overrides the document attribute.
    let src = format!(":table-frame: sides\n\n[frame=ends]\n{BASE_H}");
    let doc = Parser::default().parse(&src);
    assert_eq!(first_table(&doc).frame(), Frame::Ends);

    // An unrecognized value falls back to the default.
    let src = format!("[frame=bogus]\n{BASE_H}");
    assert_eq!(parse_table(&src).frame(), Frame::All);

    verifies!(
        r#"
The `ends` value draws a border on the top and bottom of the table.

"#
    );

    // `ends` draws a border on the top and bottom; `topbot` is accepted as the
    // older synonym for the same value.
    let src = format!("[frame=ends]\n{BASE_H}");
    assert_eq!(parse_table(&src).frame(), Frame::Ends);
    let src = format!("[frame=topbot]\n{BASE_H}");
    assert_eq!(parse_table(&src).frame(), Frame::Ends);

    verifies!(
        r#"
.Table with frame=ends
[source#ex-ends]
----
[frame=ends]
include::example$row.adoc[tag=base-h]
----

.Result of <<ex-ends>>
[frame=ends]
include::example$row.adoc[tag=base-h]

"#
    );

    // <<ex-ends>>: the standard three-column table framed on its ends. The frame
    // is the only thing that changes; the header and body rows are unaffected.
    let src = format!("[frame=ends]\n{BASE_H}");
    let table = parse_table(&src);
    assert_eq!(table.frame(), Frame::Ends);
    assert_eq!(table.grid(), Grid::All);
    assert!(table.header_row().is_some());
    assert_eq!(table.body_rows().len(), 2);

    verifies!(
        r#"
The `sides` value draws a border on the right and left side of the table.

"#
    );

    // `sides` draws a border on the left and right.
    let src = format!("[frame=sides]\n{BASE_H}");
    assert_eq!(parse_table(&src).frame(), Frame::Sides);

    verifies!(
        r#"
.Table with frame=sides
[source#ex-sides]
----
[frame=sides]
include::example$row.adoc[tag=base-h]
----

.Result of <<ex-sides>>
[frame=sides]
include::example$row.adoc[tag=base-h]

"#
    );

    // <<ex-sides>>: the standard three-column table framed on its sides.
    let src = format!("[frame=sides]\n{BASE_H}");
    let table = parse_table(&src);
    assert_eq!(table.frame(), Frame::Sides);
    assert!(table.header_row().is_some());
    assert_eq!(table.body_rows().len(), 2);

    verifies!(
        r#"
The `none` value removes the borders around the table.

"#
    );

    // `none` removes the frame entirely.
    let src = format!("[frame=none]\n{BASE_H}");
    assert_eq!(parse_table(&src).frame(), Frame::None);

    verifies!(
        r#"
.Table with frame=none
[source#ex-noframe]
----
[frame=none]
include::example$row.adoc[tag=base-h]
----

.Result of <<ex-noframe>>
[frame=none]
include::example$row.adoc[tag=base-h]

"#
    );

    // <<ex-noframe>>: the standard three-column table with no frame.
    let src = format!("[frame=none]\n{BASE_H}");
    let table = parse_table(&src);
    assert_eq!(table.frame(), Frame::None);
    assert!(table.header_row().is_some());
    assert_eq!(table.body_rows().len(), 2);
}

#[test]
fn grid() {
    non_normative!(
        r#"
== Grid

"#
    );

    verifies!(
        r#"
The borders between the cells in a table are controlled using the `grid` attribute on the table.
The grid attribute defaults to the value `all` (implied value), which draws a border between all cells.
This default can be changed by setting the `table-grid` document attribute.
You can override the default value by setting the grid attribute on the table to the value `all`, `rows`, `cols` or `none`.

"#
    );

    // The grid defaults to `all`, and the implied (empty) value resolves the
    // same way.
    assert_eq!(parse_table(BASE_H).grid(), Grid::All);
    let src = format!("[grid=all]\n{BASE_H}");
    assert_eq!(parse_table(&src).grid(), Grid::All);

    // The `table-grid` document attribute changes the default for tables that
    // don't set their own `grid`.
    let src = format!(":table-grid: cols\n\n{BASE_H}");
    let doc = Parser::default().parse(&src);
    assert_eq!(first_table(&doc).grid(), Grid::Cols);

    // An explicit `grid` on the table overrides the document attribute.
    let src = format!(":table-grid: cols\n\n[grid=rows]\n{BASE_H}");
    let doc = Parser::default().parse(&src);
    assert_eq!(first_table(&doc).grid(), Grid::Rows);

    // An unrecognized value falls back to the default.
    let src = format!("[grid=bogus]\n{BASE_H}");
    assert_eq!(parse_table(&src).grid(), Grid::All);

    verifies!(
        r#"
The `rows` value draws a border between the rows of the table.

"#
    );

    // `rows` draws a border between the rows.
    let src = format!("[grid=rows]\n{BASE_H}");
    assert_eq!(parse_table(&src).grid(), Grid::Rows);

    verifies!(
        r#"
.Table with grid=rows
[source#ex-rows]
----
[grid=rows]
include::example$row.adoc[tag=base-h]
----

.Result of <<ex-rows>>
[grid=rows]
include::example$row.adoc[tag=base-h]

"#
    );

    // <<ex-rows>>: the standard three-column table with a border only between
    // its rows. The grid is the only thing that changes.
    let src = format!("[grid=rows]\n{BASE_H}");
    let table = parse_table(&src);
    assert_eq!(table.grid(), Grid::Rows);
    assert_eq!(table.frame(), Frame::All);
    assert!(table.header_row().is_some());
    assert_eq!(table.body_rows().len(), 2);

    verifies!(
        r#"
The `cols` value draws borders between the columns.

"#
    );

    // `cols` draws a border between the columns.
    let src = format!("[grid=cols]\n{BASE_H}");
    assert_eq!(parse_table(&src).grid(), Grid::Cols);

    verifies!(
        r#"
.Table with grid=cols
[source#ex-cols]
----
[grid=cols]
include::example$row.adoc[tag=base-h]
----

.Result of <<ex-cols>>
[grid=cols]
include::example$row.adoc[tag=base-h]

"#
    );

    // <<ex-cols>>: the standard three-column table with a border only between
    // its columns.
    let src = format!("[grid=cols]\n{BASE_H}");
    let table = parse_table(&src);
    assert_eq!(table.grid(), Grid::Cols);
    assert!(table.header_row().is_some());
    assert_eq!(table.body_rows().len(), 2);

    verifies!(
        r#"
The `none` value removes all borders between the cells.

"#
    );

    // `none` removes the grid entirely.
    let src = format!("[grid=none]\n{BASE_H}");
    assert_eq!(parse_table(&src).grid(), Grid::None);

    verifies!(
        r#"
.Table with grid=none
[source#ex-nogrid]
----
[grid=none]
include::example$row.adoc[tag=base-h]
----

.Result of <<ex-nogrid>>
[grid=none]
include::example$row.adoc[tag=base-h]

"#
    );

    // <<ex-nogrid>>: the standard three-column table with no grid.
    let src = format!("[grid=none]\n{BASE_H}");
    let table = parse_table(&src);
    assert_eq!(table.grid(), Grid::None);
    assert!(table.header_row().is_some());
    assert_eq!(table.body_rows().len(), 2);
}

// The interaction between row/column spans and border placement is a rendering
// limitation of styling HTML with CSS. It describes downstream converter
// behavior rather than anything this parser models, so the whole section is
// non-normative here.
non_normative!(
    r#"
== Interaction with row and column spans

Using row and column spans may interfere with the placement of borders on a table.
This is a limitation of styling HTML using CSS.

When a cell extends into other rows or columns, that cell is not represented in the HTML for the rows or columns it extends into.
This is a problem if the cell reaches the boundary of the table.
The CSS selector only matches the cell where it starts and thus does not detect when it is touching the table boundary.
It therefore cannot add or remove the border as it would for a 1x1 cell (i.e., a cell confined to a single row and column).

The interference with border placement caused by row and column spans does not always happen.
Borders on a table with a rowspan or colspan that reaches the table boundary will always work correctly when the frame and grid are congruent.
In this context, congruent means the frame and grid are contributing borders to the same edges.

Here are those scenarios in which the frame and grid are congruent:

* frame=all, grid=all
* frame=all, grid=none
* frame=all, grid=rows
* frame=all, grid=cols
* frame=ends, grid=rows
* frame=sides, grid=cols
* frame=none, grid=none

If you use row and column spans in a table, you are strongly encouraged to use one of these frame and grid combinations.
"#
);
