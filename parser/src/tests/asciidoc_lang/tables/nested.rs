use crate::tests::prelude::*;

track_file!("docs/modules/tables/pages/nested.adoc");

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

/// Return the single nested [`TableBlock`] held by `cell`, panicking if the
/// cell is not an [`AsciiDoc`](crate::blocks::TableCellContent::AsciiDoc) cell
/// or does not contain exactly one table.
fn nested_table<'a>(cell: &'a crate::blocks::TableCell<'_>) -> &'a crate::blocks::TableBlock<'a> {
    match cell.content() {
        crate::blocks::TableCellContent::AsciiDoc(cell) => {
            let blocks = cell.blocks();
            let tables: Vec<&crate::blocks::TableBlock<'_>> = blocks
                .iter()
                .filter_map(|block| match block {
                    crate::blocks::Block::Table(table) => Some(table),
                    _ => None,
                })
                .collect();
            assert_eq!(tables.len(), 1, "expected exactly one nested table");
            tables[0]
        }
        other => panic!("expected an AsciiDoc cell, got {other:?}"),
    }
}

/// Return the rendered text of a [`Simple`](crate::blocks::TableCellContent)
/// cell.
fn simple_text(cell: &crate::blocks::TableCell<'_>) -> String {
    match cell.content() {
        crate::blocks::TableCellContent::Simple(content) => content.rendered().to_string(),
        crate::blocks::TableCellContent::AsciiDoc(_) => panic!("expected simple cell content"),
    }
}

// The `tag=nested` example from the page: an outer two-column table whose last
// cell carries the AsciiDoc style (`a`) and holds a nested table. The nested
// table has its own column spec, delimiter (`!===`), and cell separator (`!`).
const NESTED_EXAMPLE: &str = "[cols=\"1,2a\"]\n|===\n| Col 1 | Col 2\n\n| Cell 1.1\n| Cell 1.2\n\n| Cell 2.1\n| Cell 2.2\n\n[cols=\"2,1\"]\n!===\n! Col1 ! Col2\n\n! C11\n! C12\n\n!===\n\n|===";

non_normative!(
    r#"
= Nesting Tables

"#
);

#[test]
fn distinguish_inner_from_outer() {
    verifies!(
        r#"
Table cells marked with the AsciiDoc table style (`a`) support nested tables in addition to normal block content.
To distinguish the inner table from the enclosing one, you need to use `!===` as the table delimiter and a cell separator that differs from that used for the enclosing table.
"#
    );

    // The outer table separates its cells with the default vertical bar (`|`).
    // Its last column carries the AsciiDoc style (`a`), so the last body cell
    // holds block content rather than inline content.
    let outer = parse_table(NESTED_EXAMPLE);
    assert_eq!(outer.body_rows().len(), 2);

    let last_cell = &outer.body_rows()[1].cells()[1];
    assert_eq!(last_cell.style(), ColumnStyle::AsciiDoc);

    // The AsciiDoc cell holds a nested table in addition to its normal block
    // content (the leading `Cell 2.2` paragraph).
    match last_cell.content() {
        crate::blocks::TableCellContent::AsciiDoc(cell) => {
            let blocks = cell.blocks();
            assert!(
                blocks
                    .iter()
                    .any(|b| matches!(b, crate::blocks::Block::Table(_))),
                "expected a nested table among the cell's blocks"
            );
            assert!(
                blocks
                    .iter()
                    .any(|b| !matches!(b, crate::blocks::Block::Table(_))),
                "expected normal block content alongside the nested table"
            );
        }
        other => panic!("expected an AsciiDoc cell, got {other:?}"),
    }

    // The inner table is delimited by `!===` and separates its cells with `!`,
    // which differs from the outer table's `|`. Because the separators differ,
    // the outer table's `|` scan does not break the inner table apart: the inner
    // `!`-separated cells are parsed as a complete two-column table.
    let inner = nested_table(last_cell);
    assert_eq!(inner.header_row().unwrap().cells().len(), 2);
    assert_eq!(inner.body_rows().len(), 1);
    assert_eq!(inner.body_rows()[0].cells().len(), 2);
    assert_eq!(simple_text(&inner.body_rows()[0].cells()[0]), "C11");
    assert_eq!(simple_text(&inner.body_rows()[0].cells()[1]), "C12");

    // A `!===` table at the top level (not inside an AsciiDoc cell) is still a
    // table — `parse_table` would panic if the `!===` delimiter were not
    // recognized — but it separates on the default `|`, so the `|` begins the
    // cell and a bare `!` is literal text that does not begin a cell.
    let top_level = parse_table("!===\n| a ! b\n!===");
    assert_eq!(top_level.body_rows()[0].cells().len(), 1);
    assert_eq!(simple_text(&top_level.body_rows()[0].cells()[0]), "a ! b");

    // The same top-level `!===` table behaves identically to its `|===`
    // counterpart, confirming the leading delimiter character does not change
    // the (top-level) default separator.
    let pipe_level = parse_table("|===\n| a ! b\n|===");
    assert_eq!(
        simple_text(&top_level.body_rows()[0].cells()[0]),
        simple_text(&pipe_level.body_rows()[0].cells()[0]),
    );
}

#[test]
fn default_separator_and_override() {
    verifies!(
        r#"
The default cell separator for a nested table is `!`, though you can choose another character by defining the `separator` attribute on the table.

"#
    );

    // With no `separator` attribute, the nested table's cell separator defaults
    // to `!`: the `!`-prefixed tokens begin cells and a `|` would be literal.
    let outer = parse_table("|===\na|\n!===\n! one ! two\n!===\n|===");
    let inner = nested_table(&outer.body_rows()[0].cells()[0]);
    assert_eq!(inner.body_rows()[0].cells().len(), 2);
    assert_eq!(simple_text(&inner.body_rows()[0].cells()[0]), "one");
    assert_eq!(simple_text(&inner.body_rows()[0].cells()[1]), "two");

    // The `separator` attribute overrides the default: here the nested table
    // separates on `:` instead, so the `:`-prefixed tokens begin cells.
    let outer = parse_table("|===\na|\n[separator=:]\n!===\n:one :two\n!===\n|===");
    let inner = nested_table(&outer.body_rows()[0].cells()[0]);
    assert_eq!(inner.body_rows()[0].cells().len(), 2);
    assert_eq!(simple_text(&inner.body_rows()[0].cells()[0]), "one");
    assert_eq!(simple_text(&inner.body_rows()[0].cells()[1]), "two");

    // The `separator` attribute applies to an outer table as well, replacing the
    // default `|` with the chosen character.
    let outer = parse_table("[separator=;]\n|===\n;one ;two\n|===");
    assert_eq!(outer.body_rows()[0].cells().len(), 2);
    assert_eq!(simple_text(&outer.body_rows()[0].cells()[0]), "one");
    assert_eq!(simple_text(&outer.body_rows()[0].cells()[1]), "two");
}

non_normative!(
    r#"
NOTE: Although nested tables are not technically valid in DocBook 5.0, the DocBook toolchain processes them anyway.

"#
);

#[test]
fn nested_table_example() {
    verifies!(
        r#"
The following example contains a nested table in the last cell.
Notice the nested table has its own format, independent of that of the outer table:

[source]
----
include::example$table.adoc[tag=nested]
----

.Result: A nested table
include::example$table.adoc[tag=nested]

"#
    );

    // Expansion of `example$table.adoc[tag=nested]`, which is identical to
    // `NESTED_EXAMPLE`: a two-column outer table whose AsciiDoc-styled (`a`) last
    // cell carries a nested table.
    let outer = parse_table(NESTED_EXAMPLE);
    assert_eq!(outer.columns().len(), 2);
    assert_eq!(outer.body_rows().len(), 2);

    let last_cell = &outer.body_rows()[1].cells()[1];
    assert_eq!(last_cell.style(), ColumnStyle::AsciiDoc);

    // The nested table renders with its own format, independent of the outer
    // table: its own column spec produces a two-cell implicit header row
    // (`Col1`/`Col2`) followed by one body row (`C11`/`C12`).
    let inner = nested_table(last_cell);
    assert_eq!(inner.columns().len(), 2);
    assert_eq!(
        inner
            .header_row()
            .unwrap()
            .cells()
            .iter()
            .map(simple_text)
            .collect::<Vec<_>>(),
        vec!["Col1", "Col2"]
    );
    assert_eq!(inner.body_rows().len(), 1);
    assert_eq!(
        inner.body_rows()[0]
            .cells()
            .iter()
            .map(simple_text)
            .collect::<Vec<_>>(),
        vec!["C11", "C12"]
    );
}

non_normative!(
    r#"
We recommend using nested tables sparingly.
There's usually a better way to present the information.
"#
);
