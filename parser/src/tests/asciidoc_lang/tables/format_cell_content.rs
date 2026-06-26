use crate::{document::InterpretedValue, tests::prelude::*};

track_file!("docs/modules/tables/pages/format-cell-content.adoc");

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

/// Collect the [style](ColumnStyle) of each cell in each body row of `table`.
fn body_styles(table: &crate::blocks::TableBlock<'_>) -> Vec<Vec<ColumnStyle>> {
    table
        .body_rows()
        .iter()
        .map(|row| row.cells().iter().map(|cell| cell.style()).collect())
        .collect()
}

/// Return the rendered text of a [`Simple`](crate::blocks::TableCellContent)
/// cell, panicking if the cell holds AsciiDoc block content instead.
fn simple_text(cell: &crate::blocks::TableCell<'_>) -> String {
    match cell.content() {
        crate::blocks::TableCellContent::Simple(content) => content.rendered().to_string(),
        crate::blocks::TableCellContent::AsciiDoc(_) => panic!("expected simple cell content"),
    }
}

/// Find the first table in `doc` and return the joined rendered text of the
/// AsciiDoc blocks in its first body cell.
fn asciidoc_cell_text(doc: &crate::Document<'_>) -> String {
    let table = doc
        .nested_blocks()
        .find_map(|b| match b {
            crate::blocks::Block::Table(table) => Some(table),
            _ => None,
        })
        .expect("expected a table block");

    match table.body_rows()[0].cells()[0].content() {
        crate::blocks::TableCellContent::AsciiDoc(cell) => cell
            .blocks()
            .iter()
            .filter_map(|block| block.rendered_content())
            .collect::<Vec<_>>()
            .join("\n"),
        other => panic!("expected nested AsciiDoc blocks, got {other:?}"),
    }
}

non_normative!(
    r#"
= Format Content by Cell

"#
);

#[test]
fn cell_styles_and_their_operators() {
    non_normative!(
        r#"
== Cell styles and their operators

"#
    );

    verifies!(
        r#"
You can style all of the content in an individual cell by adding a style operator to the xref:add-cells-and-rows.adoc#specifiers[cell's specifier].

"#
    );

    // A style operator on a cell specifier styles all of that cell's content.
    // Each operator maps to its corresponding style.
    let table = parse_table("|===\n|h\n\na|adoc\ne|emph\nh|head\nl|lit\nm|mono\ns|strong\n|===");
    assert_eq!(
        body_styles(&table),
        vec![
            vec![ColumnStyle::AsciiDoc],
            vec![ColumnStyle::Emphasis],
            vec![ColumnStyle::Header],
            vec![ColumnStyle::Literal],
            vec![ColumnStyle::Monospace],
            vec![ColumnStyle::Strong],
        ]
    );

    verifies!(
        r#"
include::partial$style-operators.adoc[]

When a style operator isn't explicitly assigned to a cell specifier (or xref:format-column-content.adoc[column specifier]), the cell falls back to the default (`d`) style and is processed as regular paragraph text.
(The explicit `d` style is only needed if you want to revert the cell style based to a normal (default) cell when a style is applied to the column.)

"#
    );

    // With no style operator and no column style, a cell falls back to the
    // default (`d`) style.
    let table = parse_table("|===\n|h\n\n|plain\n|===");
    assert_eq!(body_styles(&table), vec![vec![ColumnStyle::Default]]);

    // The explicit `d` operator only matters when the column carries a style: it
    // reverts that one cell to the default while its siblings keep the column's
    // style.
    let table = parse_table("[cols=\"m,m\"]\n|===\n|h1 |h2\n\nd|reverted |still monospace\n|===");
    assert_eq!(
        body_styles(&table),
        vec![vec![ColumnStyle::Default, ColumnStyle::Monospace]]
    );
}

#[test]
fn apply_a_style_to_a_table_cell() {
    non_normative!(
        r#"
== Apply a style to a table cell

"#
    );

    verifies!(
        r#"
The style operator is always entered last in a xref:add-cells-and-rows.adoc#specifiers[cell specifier].
Don't insert any spaces between the `|` and the operator.

====
<factor><span or duplication operator><horizontal alignment operator><vertical alignment operator><**style operator**>|<cell's content>
====

"#
    );

    // The style operator occupies the last position of the specifier, after any
    // span/duplication operator and the horizontal and vertical alignment
    // operators: `2*>m` (duplication, right, monospace) and `.3+^.>s` (row span,
    // center, bottom, strong). The `2*` duplication clones its cell, so the first
    // two cells both carry the right-aligned monospace specifier; the `.3+^.>s`
    // cell follows.
    let table =
        parse_table("|===\n|h\n\n2*>m|dup right mono\n.3+^.>s|span center bottom strong\n|===");
    let cells: Vec<&crate::blocks::TableCell<'_>> =
        table.body_rows().iter().flat_map(|r| r.cells()).collect();
    assert_eq!(cells[0].style(), ColumnStyle::Monospace);
    assert_eq!(cells[0].h_align(), HorizontalAlignment::Right);
    assert_eq!(cells[1].style(), ColumnStyle::Monospace);
    assert_eq!(cells[1].h_align(), HorizontalAlignment::Right);
    assert_eq!(cells[2].style(), ColumnStyle::Strong);
    assert_eq!(cells[2].h_align(), HorizontalAlignment::Center);
    assert_eq!(cells[2].v_align(), VerticalAlignment::Bottom);

    verifies!(
        r#"
Let's apply a style operator to each cell in <<ex-cell-styles>>.

.Apply a style operator to a cell
[source#ex-cell-styles]
----
include::example$cell.adoc[tag=styles]
----

The table from <<ex-cell-styles>> is rendered below.

.Result of <<ex-cell-styles>>
include::example$cell.adoc[tag=styles]

"#
    );

    // Expansion of `example$cell.adoc[tag=styles]`. Each body cell carries its
    // own style operator. The `2*>m` duplication clones its monospace content
    // into the two cells of the first row. The `.3+` row span on the next cell
    // carries its column down through the two rows below it, so each of those
    // rows needs only one explicit cell to be complete.
    let table = parse_table(
        "|===\n|Column 1 |Column 2\n\n2*>m|This content is duplicated across two columns (2*) and aligned to the right side of the cell (>).\n\nIt's rendered using a monospace font (m).\n\n.3+^.>s|This cell spans 3 rows (`3+`).\nThe content is centered horizontally (`+^+`), vertically aligned to the bottom of the cell (`.>`), and styled as strong (`s`).\ne|This content is italicized (`e`).\n\nm|This content is rendered using a monospace font (m).\n\ns|This content is bold (`s`).\n|===",
    );
    assert_eq!(
        body_styles(&table),
        vec![
            vec![ColumnStyle::Monospace, ColumnStyle::Monospace],
            vec![ColumnStyle::Strong, ColumnStyle::Emphasis],
            vec![ColumnStyle::Monospace],
            vec![ColumnStyle::Strong],
        ]
    );
}

#[test]
fn override_the_column_style_on_a_cell() {
    non_normative!(
        r#"
[#override-column-style]
== Override the column style on a cell

"#
    );

    verifies!(
        r#"
When you assign a style operator to a cell, it overrides the xref:format-column-content.adoc[column's style operator].
In <<ex-override>>, the style operator assigned to the first column is overridden on two cells.
The header row also overrides style operators.
However, inline formatting markup is applied in addition to the style specified by an operator.

"#
    );

    verifies!(
        r#"
.Override the column style using a cell style operator
[source#ex-override]
----
[cols="m,m"] <.>
|===
|Column 1, header row |Column 2, header row <.>

|This content is rendered using a monospace font because the column's specifier includes the `m` operator.
|This content is rendered using a monospace font because the column's specifier includes the `m` operator.

s|This content is rendered as bold paragraph text because the `s` operator in the cell's specifier overrides the style operator in the column specifier. <.>
|*This content is rendered using a monospace font because the column's specifier includes the `m` operator.
It's also bold because it's marked up with the inline syntax for bold formatting.* <.>

d|This content is rendered as regular paragraph text because the `d` operator in the cell's specifier overrides the style operator in the column specifier. <.>
|This content is rendered using a monospace font because the column's specifier includes the `m` operator.
|===
----
<.> The monospace operator (`m`) is assigned to both columns.
<.> The header row ignores any style operators assigned via column or cell specifiers.
<.> The strong operator (`s`) is assigned to this cell's specifier, overriding the column's monospace style.
<.> Inline formatting is applied in addition to the style assigned via a column specifier.
<.> The default operator (`d`) is assigned to this cell's specifier, resetting it to the default text style.

The table from <<ex-override>> is displayed below.

.Result of <<ex-override>>
[cols="m,m"]
|===
|Column 1, header row |Column 2, header row

|This content is rendered using a monospace font because the column's specifier includes the `m` operator.
|This content is rendered using a monospace font because the column's specifier includes the `m` operator.

s|This content is rendered as bold paragraph text because the `s` operator in the cell's specifier overrides the style operator in the column specifier.
|*This content is rendered using a monospace font because the column's specifier includes the `m` operator.
It's also bold because it's marked up with the inline syntax for bold formatting.*

d|This content is rendered as regular paragraph text because the `d` operator in the cell's specifier overrides the style operator in the column specifier.
|This content is rendered using a monospace font because the column's specifier includes the `m` operator.
|===

"#
    );

    // Expansion of `example$ex-override`. The column style operator (`m`) is
    // overridden by the cell style operators `s` and `d`; cells without an
    // operator inherit the column's monospace style.
    let table = parse_table(
        "[cols=\"m,m\"]\n|===\n|Column 1, header row |Column 2, header row\n\n|This content is rendered using a monospace font because the column's specifier includes the `m` operator.\n|This content is rendered using a monospace font because the column's specifier includes the `m` operator.\n\ns|This content is rendered as bold paragraph text because the `s` operator in the cell's specifier overrides the style operator in the column specifier.\n|*This content is rendered using a monospace font because the column's specifier includes the `m` operator.\nIt's also bold because it's marked up with the inline syntax for bold formatting.*\n\nd|This content is rendered as regular paragraph text because the `d` operator in the cell's specifier overrides the style operator in the column specifier.\n|This content is rendered using a monospace font because the column's specifier includes the `m` operator.\n|===",
    );

    // A cell style operator overrides the column's style; a cell with no operator
    // inherits it.
    assert_eq!(
        body_styles(&table),
        vec![
            vec![ColumnStyle::Monospace, ColumnStyle::Monospace],
            vec![ColumnStyle::Strong, ColumnStyle::Monospace],
            vec![ColumnStyle::Default, ColumnStyle::Monospace],
        ]
    );

    // The header row overrides (ignores) style operators: both header cells stay
    // the default style even though their column carries the `m` operator.
    let header = table.header_row().unwrap();
    assert_eq!(
        header.cells().iter().map(|c| c.style()).collect::<Vec<_>>(),
        vec![ColumnStyle::Default, ColumnStyle::Default]
    );

    // Inline formatting markup is applied in addition to the style operator: the
    // monospace cell whose content is wrapped in `*...*` still renders a strong
    // span.
    let bold_in_mono = &table.body_rows()[1].cells()[1];
    assert_eq!(bold_in_mono.style(), ColumnStyle::Monospace);
    assert!(simple_text(bold_in_mono).contains("<strong>"));
}

#[test]
fn use_asciidoc_block_elements_in_a_table_cell() {
    non_normative!(
        r#"
[#a-operator]
== Use AsciiDoc block elements in a table cell

"#
    );

    verifies!(
        r#"
To use AsciiDoc block elements, such as delimited source blocks and lists, in a cell, place the `a` operator directly in front of the xref:add-cells-and-rows.adoc#cell-separator[cell's separator] (`|`).
Don't insert any spaces between the `|` and the operator.
The `a` can also be specified on the column in the `cols` attribute on the table.

"#
    );

    // A cell prefixed with the `a` operator parses its content as a nested
    // sequence of blocks; a cell with no operator keeps the same markup as inline
    // text.
    let table = parse_table("|===\n|Normal |AsciiDoc\n\n|* not a list\na|* a list\n|===");
    let row = &table.body_rows()[0];
    assert!(matches!(
        row.cells()[0].content(),
        crate::blocks::TableCellContent::Simple(_)
    ));
    match row.cells()[1].content() {
        crate::blocks::TableCellContent::AsciiDoc(cell) => {
            let blocks = cell.blocks();
            assert_eq!(blocks.len(), 1);
            assert_eq!(blocks[0].raw_context().as_ref(), "list");
        }
        other => panic!("expected nested AsciiDoc blocks, got {other:?}"),
    }

    // The `a` operator can equally be set on the column via the `cols` attribute,
    // which makes every cell in that column an AsciiDoc cell.
    let table = parse_table("[cols=\"a,d\"]\n|===\n|h1 |h2\n\n|* a list\n|* not a list\n|===");
    let row = &table.body_rows()[0];
    assert!(matches!(
        row.cells()[0].content(),
        crate::blocks::TableCellContent::AsciiDoc(_)
    ));
    assert!(matches!(
        row.cells()[1].content(),
        crate::blocks::TableCellContent::Simple(_)
    ));

    verifies!(
        r#"
.Apply the AsciiDoc block style operator to two cells
[source#ex-asciidoc]
....
include::example$cell.adoc[tag=adoc]
....

The table from <<ex-asciidoc>> is rendered below.

.Result of <<ex-asciidoc>>
include::example$cell.adoc[tag=adoc]

"#
    );

    // Expansion of `example$cell.adoc[tag=adoc]`. In each row the first cell has
    // no operator (its list / listing markup stays inline text), while the second
    // cell's `a` operator parses the same markup as nested AsciiDoc blocks.
    let table = parse_table(
        "|===\n|Normal Style |AsciiDoc Style\n\n|This cell isn't prefixed with an `a`, so the processor doesn't interpret the following lines as an AsciiDoc list.\n\n* List item 1\n* List item 2\n* List item 3\n\na|This cell is prefixed with an `a`, so the processor interprets the following lines as an AsciiDoc list.\n\n* List item 1\n* List item 2\n* List item 3\n\n|This cell isn't prefixed with an `a`, so the processor doesn't interpret the listing block delimiters or the `source` style.\n\n[source,python]\n----\nimport os\nprint (\"%s\" %(os.uname()))\n----\n\na|This cell is prefixed with an `a`, so the listing block is processed and rendered according to the `source` style rules.\n\n[source,python]\n----\nimport os\nprint \"%s\" %(os.uname())\n----\n\n|===",
    );
    assert_eq!(table.body_rows().len(), 2);

    // First row: the list markup. The plain cell keeps it inline; the `a` cell
    // parses its leading sentence as a paragraph and the following lines as a
    // list block.
    let row = &table.body_rows()[0];
    assert!(matches!(
        row.cells()[0].content(),
        crate::blocks::TableCellContent::Simple(_)
    ));
    match row.cells()[1].content() {
        crate::blocks::TableCellContent::AsciiDoc(cell) => {
            let blocks = cell.blocks();
            assert_eq!(blocks.len(), 2);
            assert_eq!(blocks[0].raw_context().as_ref(), "paragraph");
            assert_eq!(blocks[1].raw_context().as_ref(), "list");
        }
        other => panic!("expected nested AsciiDoc blocks, got {other:?}"),
    }

    // Second row: the source listing. The plain cell keeps it inline; the `a`
    // cell parses its leading sentence as a paragraph and the delimited block as
    // a listing block.
    let row = &table.body_rows()[1];
    assert!(matches!(
        row.cells()[0].content(),
        crate::blocks::TableCellContent::Simple(_)
    ));
    match row.cells()[1].content() {
        crate::blocks::TableCellContent::AsciiDoc(cell) => {
            let blocks = cell.blocks();
            assert_eq!(blocks.len(), 2);
            assert_eq!(blocks[0].raw_context().as_ref(), "paragraph");
            assert_eq!(blocks[1].raw_context().as_ref(), "listing");
        }
        other => panic!("expected nested AsciiDoc blocks, got {other:?}"),
    }

    verifies!(
        r#"
An AsciiDoc table cell effectively creates a nested document.
As such, it inherits attributes from the parent document.
"#
    );

    // An AsciiDoc cell inherits attributes from the parent document: a `{name}`
    // reference inside the cell resolves to the value set on the parent.
    let mut parser = Parser::default();
    let doc = parser.parse(":name: Tester\n\n|===\n|h\n\na|Hello {name}\n|===");
    let table = doc
        .nested_blocks()
        .find_map(|b| match b {
            crate::blocks::Block::Table(table) => Some(table),
            _ => None,
        })
        .expect("expected a table block");
    match table.body_rows()[0].cells()[0].content() {
        crate::blocks::TableCellContent::AsciiDoc(cell) => {
            let blocks = cell.blocks();
            assert_eq!(blocks.len(), 1);
            match &blocks[0] {
                crate::blocks::Block::Simple(simple) => {
                    assert_eq!(simple.content().rendered(), "Hello Tester");
                }
                other => panic!("expected a simple block, got {other:?}"),
            }
        }
        other => panic!("expected nested AsciiDoc blocks, got {other:?}"),
    }

    verifies!(
        r#"
If an attribute is set or explicitly unset in the parent document, it _cannot_ be modified in the AsciiDoc table cell.
There are a handful of exceptions to this rule, which includes doctype, toc, notitle (and its complement, showtitle), and compat-mode.
"#
    );

    // An attribute set in the parent is locked inside the cell: a reassignment
    // (placed before the reference, so it would take effect if it were honored)
    // is ignored, so the cell still sees the inherited value. Asciidoctor only
    // locks attributes that are *set*, not ones explicitly unset, so the crate
    // follows suit; see the unit tests in `blocks::tests::table` for the full
    // set/unset/exception matrix.
    let mut parser = Parser::default();
    let doc = parser
        .parse(":locked: parent\n\n|===\n|h\n\na|\n:locked: child\n\nvalue is {locked}\n|===");
    assert_eq!(asciidoc_cell_text(&doc), "value is parent");

    // One of the carved-out exceptions (here `compat-mode`) may still be modified
    // inside the cell even though the parent set it.
    let mut parser = Parser::default();
    let doc = parser
        .parse(":compat-mode: parent\n\n|===\n|h\n\na|\n:compat-mode: child\n\nmode is {compat-mode}\n|===");
    assert_eq!(asciidoc_cell_text(&doc), "mode is child");

    verifies!(
        r#"
Any newly defined attributes in the AsciiDoc table do not impact the attributes in the parent document.
Instead, these attributes are scoped to the table cell.

"#
    );

    // An attribute newly defined inside an AsciiDoc cell is scoped to that cell
    // and does not leak back into the parent document.
    let mut parser = Parser::default();
    let _ = parser.parse("|===\n|h\n\na|\n:scoped: value\n\nbody\n|===");
    assert_eq!(parser.attribute_value("scoped"), InterpretedValue::Unset);

    non_normative!(
        r#"
If the AsciiDoc table cell starts with a preprocessor directive, that directive should be placed on the line after the cell separator.
While it can be placed on the same line as the cell separator, that style is not recommended.
That's because the preprocessor directive that starts after the cell separator must be treated with special handling and is thus limited to a single line (for example, a multiline preprocessor conditional is not allow in this case).
By starting the contents of the AsciiDoc table cell on the line after the cell separator, the contents will be parsed as normal.
"#
    );
}
