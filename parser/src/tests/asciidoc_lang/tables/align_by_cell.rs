use crate::tests::prelude::*;

track_file!("docs/modules/tables/pages/align-by-cell.adoc");

non_normative!(
    r#"
= Align Content by Cell

The alignment operators are applied to a xref:add-cells-and-rows.adoc#specifiers[cell's specifier] and allow you to horizontally and vertically align a cell's content.

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

/// Collect the `(horizontal, vertical)` alignment of each cell in each body
/// row of `table`.
fn body_alignments(
    table: &crate::blocks::TableBlock<'_>,
) -> Vec<Vec<(HorizontalAlignment, VerticalAlignment)>> {
    table
        .body_rows()
        .iter()
        .map(|row| {
            row.cells()
                .iter()
                .map(|cell| (cell.h_align(), cell.v_align()))
                .collect()
        })
        .collect()
}

#[test]
fn horizontal_alignment_operators() {
    non_normative!(
        r#"
[#horizontal-operators]
== Horizontal alignment operators

"#
    );

    verifies!(
        r#"
Content can be horizontally aligned to the left or right side of the cell as well as the center of the cell.

Flush left operator (<):: The less-than sign (`<`) aligns the content to the left side of the cell.
This is the default horizontal alignment.
Flush right operator (>):: The greater-than sign (`>`) aligns the content to the right side of the cell.
Center operator (^):: The caret (`+^+`) centers the content horizontally in the cell.

A horizontal alignment operator is entered after a span or duplication operator (if present) and in front a <<vertical-operators,vertical alignment operator>> (if present).

====
<factor><span or duplication operator><**horizontal alignment operator**><vertical alignment operator><style operator>|<cell's content>
====

"#
    );

    // The three horizontal alignment operators on cell specifiers: `<` (left,
    // the default), `>` (right), and `^` (center). Entered directly in front of
    // the cell separator (`|`).
    let table = parse_table("|===\n<|a >|b ^|c\n|===");
    assert_eq!(
        body_alignments(&table),
        vec![vec![
            (HorizontalAlignment::Left, VerticalAlignment::Top),
            (HorizontalAlignment::Right, VerticalAlignment::Top),
            (HorizontalAlignment::Center, VerticalAlignment::Top),
        ]]
    );
}

#[test]
fn center_content_horizontally_in_a_cell() {
    non_normative!(
        r#"
=== Center content horizontally in a cell

"#
    );

    verifies!(
        r#"
To horizontally center the content in a cell, place the `+^+` operator in front of the xref:add-cells-and-rows.adoc#cell-separator[cell's separator] (`|`).
Don't insert any spaces between the `|` and the operator.

.Center the content of a cell horizontally
[source#ex-hcenter]
----
include::example$align-cell.adoc[tag=hcenter]
----

The table from <<ex-hcenter>> is rendered below.

.Result of <<ex-hcenter>>
include::example$align-cell.adoc[tag=hcenter]

"#
    );

    // Expansion of `example$align-cell.adoc[tag=hcenter]`. The first cell is
    // centered (`^|`); the second falls back to the default (left).
    let table = parse_table(
        "|===\n|Column Name |Column Name\n\n^|This content is horizontally centered because the cell specifier includes the `+^+` operator.\n|There isn't a horizontal alignment operator on this cell specifier, so the cell falls back to the default horizontal alignment.\nContent is aligned to the left side of the cell by default.\n|===",
    );
    assert_eq!(
        body_alignments(&table),
        vec![vec![
            (HorizontalAlignment::Center, VerticalAlignment::Top),
            (HorizontalAlignment::Left, VerticalAlignment::Top),
        ]]
    );

    verifies!(
        r#"
If the cell specifier includes a span (`<n>+`) or duplication (`+<n>*+`), place the `+^+` directly after the span or duplication operator.

.Center content horizontally in spanned columns and duplicated cells
[source#ex-factor]
----
include::example$align-cell.adoc[tag=factor]
----

The table from <<ex-factor>> is rendered below.

.Result of <<ex-factor>>
include::example$align-cell.adoc[tag=factor]

"#
    );

    // Expansion of `example$align-cell.adoc[tag=factor]`. The `+^+` operator is
    // placed directly after the span (`2+`) and duplication (`2*`) operators;
    // both cells are centered. (The span/duplication operators are recognized
    // but their layout effect is not yet applied.)
    let table = parse_table(
        "|===\n|Column Name |Column Name\n\n2+^|This cell spans two columns, and its content is horizontally centered because the cell specifier includes the `+^+` operator.\n2*^|This content is duplicated in two adjacent columns.\nIts content is horizontally centered because the cell specifier\nincludes the `+^+` operator.\n|===",
    );
    assert_eq!(
        body_alignments(&table),
        vec![vec![
            (HorizontalAlignment::Center, VerticalAlignment::Top),
            (HorizontalAlignment::Center, VerticalAlignment::Top),
        ]]
    );
}

#[test]
fn align_the_content_of_a_cell_to_the_right() {
    non_normative!(
        r#"
== Align the content of a cell to the right

"#
    );

    verifies!(
        r#"
To align the content in a cell to its right side, place the `>` operator in front of the xref:add-cells-and-rows.adoc#cell-separator[cell's separator] (`|`), but after a span (`<n>+`) or duplication (`+<n>*+`) operator (if present).
Don't insert any spaces between the `|` and the operators.

.Right align the content of a cell
[source#ex-right]
----
include::example$align-cell.adoc[tag=right]
----

The table from <<ex-right>> is rendered below.

.Result of <<ex-right>>
include::example$align-cell.adoc[tag=right]

"#
    );

    // Expansion of `example$align-cell.adoc[tag=right]`. The `>` operator
    // right-aligns the first cell and the spanned cell (`2+>`); the cell with no
    // operator falls back to the default (left).
    let table = parse_table(
        "|===\n|Column Name |Column Name\n\n>|This content is aligned to the right side of the cell because the cell specifier includes the `>` operator.\n|There isn't a horizontal alignment operator on this cell specifier, so the cell falls back to the default horizontal alignment.\nContent is aligned to the left side of the cell by default.\n\n2+>|This cell spans two columns.\n\nIts content is aligned to the right because the cell specifier includes the `>` operator.\nThe `>` operator must be placed directly after the span operator (`+`).\n|===",
    );
    assert_eq!(
        body_alignments(&table),
        vec![
            vec![
                (HorizontalAlignment::Right, VerticalAlignment::Top),
                (HorizontalAlignment::Left, VerticalAlignment::Top),
            ],
            vec![(HorizontalAlignment::Right, VerticalAlignment::Top)],
        ]
    );
}

#[test]
fn vertical_alignment_operators() {
    non_normative!(
        r#"
[#vertical-operators]
== Vertical alignment operators

"#
    );

    verifies!(
        r#"
Content can be vertically aligned to the top or bottom of a cell as well as the center of a cell.
Vertical alignment operators always begin with a dot (`.`).

Flush top operator (.<):: The dot and less-than sign (`.<`) aligns the content to the top of the cell.
This is the default vertical alignment.
Flush bottom operator (.>):: The dot and greater-than sign (`.>`) aligns the content to the bottom of the cell.
Center operator (.^):: The dot and caret (`+.^+`) centers the content vertically.

A vertical alignment operator is entered after a <<horizontal-operators,horizontal alignment operator>> (if present) and in front of a style operator (if present).

====
<factor><span or duplication operator><horizontal alignment operator><**vertical alignment operator**><style operator>|<cell's content>
====

"#
    );

    // The three vertical alignment operators on cell specifiers: `.<` (top, the
    // default), `.>` (bottom), and `.^` (center).
    let table = parse_table("|===\n.<|a .>|b .^|c\n|===");
    assert_eq!(
        body_alignments(&table),
        vec![vec![
            (HorizontalAlignment::Left, VerticalAlignment::Top),
            (HorizontalAlignment::Left, VerticalAlignment::Bottom),
            (HorizontalAlignment::Left, VerticalAlignment::Middle),
        ]]
    );
}

#[test]
fn align_content_to_the_bottom_of_a_cell() {
    non_normative!(
        r#"
=== Align content to the bottom of a cell

"#
    );

    verifies!(
        r#"
To align the content to the bottom of a cell, place the `+.>+` operator in front of the xref:add-cells-and-rows.adoc#cell-separator[cell's separator] (`|`).
Don't insert any spaces between the `|` and the operator.

.Align content to the bottom of a cell
[source#ex-bottom]
----
include::example$align-cell.adoc[tag=bottom]
----

The table from <<ex-bottom>> is rendered below.

.Result of <<ex-bottom>>
include::example$align-cell.adoc[tag=bottom]

"#
    );

    // Expansion of `example$align-cell.adoc[tag=bottom]`. The first cell is
    // bottom-aligned (`.>|`); the second has no vertical alignment operator and
    // falls back to the column's alignment (the `cols="2,1"` columns are
    // top-aligned by default).
    let table = parse_table(
        "[cols=\"2,1\"]\n|===\n|Column Name |Column Name\n\n.>|This content is aligned to the bottom of the cell because the cell specifier includes the `.>` operator.\n|There isn't a vertical alignment operator on this cell specifier, so the cell falls back to the alignment assigned via the column specifier or the default vertical alignment.\nContent is aligned to the top of the cell by default.\n|===",
    );
    assert_eq!(
        body_alignments(&table),
        vec![vec![
            (HorizontalAlignment::Left, VerticalAlignment::Bottom),
            (HorizontalAlignment::Left, VerticalAlignment::Top),
        ]]
    );

    verifies!(
        r#"
If the cell specifier includes a span (`<n>+`) or duplication (`+<n>*+`), place the `.>` after the span or duplication operator.

.Align content to the bottom of a cell that spans rows
[source#ex-span-vertical]
----
include::example$align-cell.adoc[tag=vspan]
----

The table from <<ex-span-vertical>> is rendered below.

.Result of <<ex-span-vertical>>
include::example$align-cell.adoc[tag=vspan]

"#
    );

    // Expansion of `example$align-cell.adoc[tag=vspan]`. The `.>` is placed
    // after the row-span operator (`.2+`); that cell is bottom-aligned, while
    // the cells with no vertical alignment operator stay top-aligned. (The
    // row-span operator is recognized but its layout effect is not yet applied.)
    let table = parse_table(
        "|===\n|Column Name |Column Name\n\n|There isn't a vertical alignment operator on this cell specifier, so the content is aligned to the top of the cell by default.\n\n.2+.>|This cell spans two rows, and its content is aligned to the bottom because the cell specifier includes the `.>` operator.\n\n|This content is aligned to the top of the cell by default.\n|===",
    );
    assert_eq!(
        body_alignments(&table),
        vec![
            vec![
                (HorizontalAlignment::Left, VerticalAlignment::Top),
                (HorizontalAlignment::Left, VerticalAlignment::Bottom),
            ],
            vec![(HorizontalAlignment::Left, VerticalAlignment::Top)],
        ]
    );
}

#[test]
fn center_content_vertically_in_a_cell() {
    non_normative!(
        r#"
=== Center content vertically in a cell

"#
    );

    verifies!(
        r#"
To vertically center the content in a cell, place the `+.^+` operator in front of the xref:add-cells-and-rows.adoc#cell-separator[cell's separator] (`|`).
Don't insert any spaces between the `|` and the operator.

.Center the content of a cell vertically
[source#ex-vcenter]
----
include::example$align-cell.adoc[tag=vcenter]
----

The table from <<ex-vcenter>> is rendered below.

.Result of <<ex-vcenter>>
include::example$align-cell.adoc[tag=vcenter]

"#
    );

    // Expansion of `example$align-cell.adoc[tag=vcenter]`. The first cell is
    // vertically centered (`.^|`); the second falls back to the default (top).
    let table = parse_table(
        "|===\n|Column Name |Column Name\n\n.^|This content is vertically centered because the cell specifier includes the `+.^+` operator.\n|There isn't a vertical alignment operator on this cell specifier, so the cell falls back to the default vertical alignment.\nContent is aligned to the top of the cell by default.\n|===",
    );
    assert_eq!(
        body_alignments(&table),
        vec![vec![
            (HorizontalAlignment::Left, VerticalAlignment::Middle),
            (HorizontalAlignment::Left, VerticalAlignment::Top),
        ]]
    );
}

#[test]
fn apply_horizontal_and_vertical_alignment_operators_to_the_same_cell() {
    non_normative!(
        r#"
== Apply horizontal and vertical alignment operators to the same cell

"#
    );

    verifies!(
        r#"
A cell can have a vertical and horizontal alignment operator included in its cell specifier.
The <<horizontal-operators,horizontal operator>> always precedes the <<vertical-operators,vertical operator>>.

.Align cells horizontally and vertically
[source#ex-combine]
----
include::example$align-cell.adoc[tag=combine]
----

The table from <<ex-combine>> is rendered below.

.Result <<ex-combine>>
include::example$align-cell.adoc[tag=combine]
"#
    );

    // Expansion of `example$align-cell.adoc[tag=combine]`. Each specifier
    // applies both a horizontal and a vertical operator (the horizontal always
    // first): `^.>` (center, bottom), `>.^` (right, middle), `2.3+^.^` (center,
    // middle, after a span), and `3*.>` (bottom, after a duplication; the
    // horizontal alignment falls back to the column default). The cell with no
    // operators falls back to the default alignments (left, top).
    let table = parse_table(
        "|===\n|Column 1 |Column 2 |Column 3\n\n^.>|The specifier for this cell is `^.>`.\nThe content is centered horizontally and aligned to the bottom of the cell.\n|There aren't any alignment operators on this cell's specifier, so the cell falls back to the default alignments.\nThe default horizontal alignment is the left side of the cell.\nThe default vertical alignment is the top of the cell.\n>.^|The specifier for this cell is `>.^`.\nThe content is aligned to the right side of the cell and centered vertically.\n\n2.3+^.^|The specifier for this cell is `pass:[2.3+^.^]`.\nIt spans two columns and three rows.\n\nIts content is centered horizontally and vertically.\n3*.>|The specifier for this cell is `3*.>`.\nThe cell is duplicated in three consecutive rows in the same column.\nIt's content is aligned to the bottom of the cell.\n|===",
    );
    assert_eq!(
        body_alignments(&table),
        vec![
            vec![
                (HorizontalAlignment::Center, VerticalAlignment::Bottom),
                (HorizontalAlignment::Left, VerticalAlignment::Top),
                (HorizontalAlignment::Right, VerticalAlignment::Middle),
            ],
            vec![
                (HorizontalAlignment::Center, VerticalAlignment::Middle),
                (HorizontalAlignment::Left, VerticalAlignment::Bottom),
            ],
        ]
    );
}
