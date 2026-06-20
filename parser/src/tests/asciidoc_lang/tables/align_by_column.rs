use crate::tests::prelude::*;

track_file!("docs/modules/tables/pages/align-by-column.adoc");

non_normative!(
    r#"
= Align Content by Column
// Using Wikipedia's names for the operators. For reference, see https://en.wikipedia.org/wiki/Less-than_sign

The alignment operators allow you to horizontally and vertically align a column's content.
They're applied to a column specifier and xref:add-columns.adoc#cols-attribute[assigned to the cols attribute].

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

/// Collect each column's `(width, horizontal alignment, vertical alignment)`.
fn column_alignments(
    table: &crate::blocks::TableBlock<'_>,
) -> Vec<(usize, HorizontalAlignment, VerticalAlignment)> {
    table
        .columns()
        .iter()
        .map(|c| (c.width(), c.h_align(), c.v_align()))
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
Content can be horizontally aligned to the left or right side of the column as well as the center of the column.

Flush left operator (<):: The less-than sign (`<`) left aligns the content.
This is the default horizontal alignment.
Flush right operator (>):: The greater-than sign (`>`) right aligns the content.
Center operator (^):: The caret (`+^+`) centers the content.

A horizontal alignment operator is entered in front a <<vertical-operators,vertical alignment operator>> (if present) and in front of a xref:adjust-column-widths.adoc[column's width] (if present).
If the number of columns is assigned using a multiplier (`+<n>*+`), the horizontal alignment operator is placed directly after the multiplier operator (`+*+`).

* `[cols="2,pass:q[#^#]1"]` A horizontal alignment operator is placed in front of the column width.
* `[cols="pass:q[#>#].^1,2"]` A horizontal alignment operator is placed in front of a vertical alignment operator.
* `[cols="pass:q[#>#],pass:q[#^#]"]` When a column width isn't specified, a horizontal alignment operator can represent both the column and the column content's alignment.
* `[cols="3*pass:q[#>#]"]` The horizontal alignment operator is placed directly after a multiplier.

"#
    );

    // The `<` operator is the default, so its presence does not change the
    // alignment from the left.
    let table = parse_table("[cols=\"<2,1\"]\n|===\n|a |b\n|===");
    assert_eq!(
        column_alignments(&table),
        vec![
            (2, HorizontalAlignment::Left, VerticalAlignment::Top),
            (1, HorizontalAlignment::Left, VerticalAlignment::Top),
        ]
    );

    // A horizontal alignment operator is placed in front of the column width.
    let table = parse_table("[cols=\"2,^1\"]\n|===\n|a |b\n|===");
    assert_eq!(
        column_alignments(&table),
        vec![
            (2, HorizontalAlignment::Left, VerticalAlignment::Top),
            (1, HorizontalAlignment::Center, VerticalAlignment::Top),
        ]
    );

    // A horizontal alignment operator is placed in front of a vertical alignment
    // operator.
    let table = parse_table("[cols=\">.^1,2\"]\n|===\n|a |b\n|===");
    assert_eq!(
        column_alignments(&table),
        vec![
            (1, HorizontalAlignment::Right, VerticalAlignment::Middle),
            (2, HorizontalAlignment::Left, VerticalAlignment::Top),
        ]
    );

    // When a column width isn't specified, a horizontal alignment operator can
    // represent both the column and the column content's alignment.
    let table = parse_table("[cols=\">,^\"]\n|===\n|a |b\n|===");
    assert_eq!(
        column_alignments(&table),
        vec![
            (1, HorizontalAlignment::Right, VerticalAlignment::Top),
            (1, HorizontalAlignment::Center, VerticalAlignment::Top),
        ]
    );

    // The horizontal alignment operator is placed directly after a multiplier.
    let table = parse_table("[cols=\"3*>\"]\n|===\n|a |b |c\n|===");
    assert_eq!(
        column_alignments(&table),
        vec![
            (1, HorizontalAlignment::Right, VerticalAlignment::Top),
            (1, HorizontalAlignment::Right, VerticalAlignment::Top),
            (1, HorizontalAlignment::Right, VerticalAlignment::Top),
        ]
    );
}

#[test]
fn center_content_horizontally_in_a_column() {
    non_normative!(
        r#"
=== Center content horizontally in a column

"#
    );

    verifies!(
        r#"
To horizontally center the content in a column, place the `+^+` operator at the beginning of the xref:add-columns.adoc#col-specifier[column's specifier].

.Center column content horizontally
[source#ex-horizontal]
----
[cols="^4,1"]
|===
|This content is horizontally centered.
|There isn't a horizontal alignment operator on this column's specifier, so the column falls back to the default horizontal alignment.
Content is left-aligned by default.
|===
----

The table from <<ex-horizontal>> is rendered below.

.Result of <<ex-horizontal>>
[cols="^4,1"]
|===
|This content is horizontally centered.
|There isn't a horizontal alignment operator on this column's specifier, so the column falls back to the default horizontal alignment.
Content is left-aligned by default.
|===

"#
    );

    let table = parse_table(
        "[cols=\"^4,1\"]\n|===\n|This content is horizontally centered.\n|There isn't a horizontal alignment operator on this column's specifier, so the column falls back to the default horizontal alignment.\nContent is left-aligned by default.\n|===",
    );
    assert_eq!(
        column_alignments(&table),
        vec![
            (4, HorizontalAlignment::Center, VerticalAlignment::Top),
            (1, HorizontalAlignment::Left, VerticalAlignment::Top),
        ]
    );

    verifies!(
        r#"
When the columns are specified using the xref:add-columns.adoc#column-multiplier[multiplier], place the `+^+` operator after the multiplier operator (`+*+`).

.Horizontal alignment and multiplier operator order
[source#ex-horizontal-multiplier]
----
[cols="2*^",options=header]
|===
|Column name
|Column name

|This content is horizontally centered.
|This content is also horizontally centered.
|===
----

The table from <<ex-horizontal-multiplier>> is rendered below.

.Result of <<ex-horizontal-multiplier>>
[cols="2*^",options=header]
|===
|Column name
|Column name

|This content is horizontally centered.
|This content is also horizontally centered.
|===

"#
    );

    let table = parse_table(
        "[cols=\"2*^\",options=header]\n|===\n|Column name\n|Column name\n\n|This content is horizontally centered.\n|This content is also horizontally centered.\n|===",
    );
    assert_eq!(
        column_alignments(&table),
        vec![
            (1, HorizontalAlignment::Center, VerticalAlignment::Top),
            (1, HorizontalAlignment::Center, VerticalAlignment::Top),
        ]
    );
    assert!(table.header_row().is_some());
}

#[test]
fn right_align_content_in_a_column() {
    non_normative!(
        r#"
=== Right align content in a column

"#
    );

    verifies!(
        r#"
To align the content in a column to its right side, place the `+>+` operator in front of the column's specifier.

.Right align column content
[source#ex-right]
----
[cols=">4,1"]
|===
|This content is aligned to the right side of the column.
|There isn't a horizontal alignment operator on this column's specifier, so the column falls back to the default horizontal alignment.
Content is left-aligned by default.
|===
----

The table <<ex-right>> is rendered below.

.Result of <<ex-right>>
[cols=">4,1"]
|===
|This content is aligned to the right side of the column.
|There isn't a horizontal alignment operator on this column's specifier, so the column falls back to the default horizontal alignment.
Content is left-aligned by default.
|===

"#
    );

    let table = parse_table(
        "[cols=\">4,1\"]\n|===\n|This content is aligned to the right side of the column.\n|There isn't a horizontal alignment operator on this column's specifier, so the column falls back to the default horizontal alignment.\nContent is left-aligned by default.\n|===",
    );
    assert_eq!(
        column_alignments(&table),
        vec![
            (4, HorizontalAlignment::Right, VerticalAlignment::Top),
            (1, HorizontalAlignment::Left, VerticalAlignment::Top),
        ]
    );

    verifies!(
        r#"
When the columns are specified using the xref:add-columns.adoc#column-multiplier[multiplier], place the `+>+` operator after the multiplier operator (`+*+`).

.Right alignment and multiplier operator order
[source#ex-right-multiplier]
----
[cols="2*>",options=header]
|===
|Column name
|Column name

|This content is aligned to the right side of the column.
|This content is also aligned to the right side of the column.
|===
----

The table from <<ex-right-multiplier>> is rendered below.

.Result of <<ex-right-multiplier>>
[cols="2*>",options=header]
|===
|Column name
|Column name

|This content is aligned to the right side of the column.
|This content is also aligned to the right side of the column.
|===

"#
    );

    let table = parse_table(
        "[cols=\"2*>\",options=header]\n|===\n|Column name\n|Column name\n\n|This content is aligned to the right side of the column.\n|This content is also aligned to the right side of the column.\n|===",
    );
    assert_eq!(
        column_alignments(&table),
        vec![
            (1, HorizontalAlignment::Right, VerticalAlignment::Top),
            (1, HorizontalAlignment::Right, VerticalAlignment::Top),
        ]
    );
    assert!(table.header_row().is_some());
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
Content can be vertically aligned to the top or bottom of a column's cells as well as the center of a column.
Vertical alignment operators always begin with a dot (`.`).

Flush top operator (.<):: The dot and less-than sign (`.<`) aligns the content to the top of the column's cells.
This is the default vertical alignment.
Flush bottom operator (.>):: The dot and greater-than sign (`.>`) aligns the content to the bottom of the column's cells.
Center operator (.^):: The dot and caret (`+.^+`) centers the content vertically.

A vertical alignment operator is entered directly after a <<horizontal-operators,horizontal alignment operator>> (if present) and before a xref:adjust-column-widths.adoc[column's width] (if present).
If the number of columns is assigned using a multiplier (`+<n>*+`), the vertical alignment operator is placed directly after the horizontal alignment operator (if present).
Otherwise, it's placed directly after the multiplier operator (`+*+`).

* `[cols="2,pass:q[#.^#]1"]` A vertical alignment operator is placed in front of the column width.
* `[cols=">pass:q[#.^#]1,2"]` The vertical alignment operator is placed after the horizontal alignment operator but before the column width.
* `[cols="pass:q[#.^#],pass:q[#.>#]"]` When a column width doesn't need to be specified, a vertical alignment operator can represent both the column and the column content's alignment.
* `[cols="3*pass:q[#.>#]"]` The vertical alignment operator is placed directly after a multiplier unless there is a horizontal alignment operator.
Then it's placed after the horizontal alignment operator, (e.g., `[cols="3*^pass:q[#.>#]"]`)

"#
    );

    // The `.<` operator is the default, so its presence does not change the
    // alignment from the top.
    let table = parse_table("[cols=\"2,.<1\"]\n|===\n|a |b\n|===");
    assert_eq!(
        column_alignments(&table),
        vec![
            (2, HorizontalAlignment::Left, VerticalAlignment::Top),
            (1, HorizontalAlignment::Left, VerticalAlignment::Top),
        ]
    );

    // A vertical alignment operator is placed in front of the column width.
    let table = parse_table("[cols=\"2,.^1\"]\n|===\n|a |b\n|===");
    assert_eq!(
        column_alignments(&table),
        vec![
            (2, HorizontalAlignment::Left, VerticalAlignment::Top),
            (1, HorizontalAlignment::Left, VerticalAlignment::Middle),
        ]
    );

    // The vertical alignment operator is placed after the horizontal alignment
    // operator but before the column width.
    let table = parse_table("[cols=\">.^1,2\"]\n|===\n|a |b\n|===");
    assert_eq!(
        column_alignments(&table),
        vec![
            (1, HorizontalAlignment::Right, VerticalAlignment::Middle),
            (2, HorizontalAlignment::Left, VerticalAlignment::Top),
        ]
    );

    // When a column width doesn't need to be specified, a vertical alignment
    // operator can represent both the column and the column content's alignment.
    let table = parse_table("[cols=\".^,.>\"]\n|===\n|a |b\n|===");
    assert_eq!(
        column_alignments(&table),
        vec![
            (1, HorizontalAlignment::Left, VerticalAlignment::Middle),
            (1, HorizontalAlignment::Left, VerticalAlignment::Bottom),
        ]
    );

    // The vertical alignment operator is placed directly after a multiplier when
    // there is no horizontal alignment operator.
    let table = parse_table("[cols=\"3*.>\"]\n|===\n|a |b |c\n|===");
    assert_eq!(
        column_alignments(&table),
        vec![
            (1, HorizontalAlignment::Left, VerticalAlignment::Bottom),
            (1, HorizontalAlignment::Left, VerticalAlignment::Bottom),
            (1, HorizontalAlignment::Left, VerticalAlignment::Bottom),
        ]
    );

    // When a horizontal alignment operator is present, the vertical alignment
    // operator is placed after it.
    let table = parse_table("[cols=\"3*^.>\"]\n|===\n|a |b |c\n|===");
    assert_eq!(
        column_alignments(&table),
        vec![
            (1, HorizontalAlignment::Center, VerticalAlignment::Bottom),
            (1, HorizontalAlignment::Center, VerticalAlignment::Bottom),
            (1, HorizontalAlignment::Center, VerticalAlignment::Bottom),
        ]
    );
}

#[test]
fn align_content_to_the_bottom_of_a_columns_cells() {
    non_normative!(
        r#"
=== Align content to the bottom of a column's cells

"#
    );

    verifies!(
        r#"
To align the content in a column to the bottom of each cell, place the `+.>+` operator directly in front of the xref:adjust-column-widths.adoc[column's width].

.Bottom align column content
[source#ex-bottom]
----
[cols=".>2,1"]
|===
|This content is vertically aligned to the bottom of the cell.
|There isn't a vertical alignment operator on this column's specifier, so the column falls back to the default vertical alignment.
Content is top-aligned by default.
|===
----

The table from <<ex-bottom>> is rendered below.

.Result of <<ex-bottom>>
[cols=".>2,1"]
|===
|This content is vertically aligned to the bottom of the cell.
|There isn't a vertical alignment operator on this column's specifier, so the column falls back to the default vertical alignment.
Content is top-aligned by default.
|===

"#
    );

    let table = parse_table(
        "[cols=\".>2,1\"]\n|===\n|This content is vertically aligned to the bottom of the cell.\n|There isn't a vertical alignment operator on this column's specifier, so the column falls back to the default vertical alignment.\nContent is top-aligned by default.\n|===",
    );
    assert_eq!(
        column_alignments(&table),
        vec![
            (2, HorizontalAlignment::Left, VerticalAlignment::Bottom),
            (1, HorizontalAlignment::Left, VerticalAlignment::Top),
        ]
    );
}

#[test]
fn center_content_vertically_in_a_column() {
    non_normative!(
        r#"
=== Center content vertically in a column

"#
    );

    verifies!(
        r#"
To vertically center the content in a column, place the `+.^+` operator directly in front of the xref:adjust-column-widths.adoc[column's width].

.Center column content vertically
[source#ex-vertical]
----
[cols=".^2,1"]
|===
|This content is centered vertically in the cell.
|There isn't a vertical alignment operator on this column's specifier, so the column falls back to the default vertical alignment.
Content is top-aligned by default.
|===
----

The table from <<ex-vertical>> is rendered below.

.Result of <<ex-vertical>>
[cols=".^2,1"]
|===
|This content is centered vertically in the cell.
|There isn't a vertical alignment operator on this column's specifier, so the column falls back to the default vertical alignment.
Content is top-aligned by default.
|===

"#
    );

    let table = parse_table(
        "[cols=\".^2,1\"]\n|===\n|This content is centered vertically in the cell.\n|There isn't a vertical alignment operator on this column's specifier, so the column falls back to the default vertical alignment.\nContent is top-aligned by default.\n|===",
    );
    assert_eq!(
        column_alignments(&table),
        vec![
            (2, HorizontalAlignment::Left, VerticalAlignment::Middle),
            (1, HorizontalAlignment::Left, VerticalAlignment::Top),
        ]
    );

    verifies!(
        r#"
To vertically align the content to the middle of the cells in all of the columns, enter the  `.^` operator after the xref:add-columns.adoc#column-multiplier[multiplier].

.Vertical alignment and multiplier operator order
[source#ex-vertical-multiplier]
----
[cols="2*.^",options=header]
|===
|Column name
|Column name

|This content is vertically centered.
|This content is also vertically centered.
|===
----

The table from <<ex-vertical-multiplier>> is rendered below.

.Result of <<ex-vertical-multiplier>>
[cols="2*.^",options=header]
|===
|Column name
|Column name

|This content is centered vertically in the cell.
|This content is also centered vertically in the cell.
|===

"#
    );

    let table = parse_table(
        "[cols=\"2*.^\",options=header]\n|===\n|Column name\n|Column name\n\n|This content is vertically centered.\n|This content is also vertically centered.\n|===",
    );
    assert_eq!(
        column_alignments(&table),
        vec![
            (1, HorizontalAlignment::Left, VerticalAlignment::Middle),
            (1, HorizontalAlignment::Left, VerticalAlignment::Middle),
        ]
    );
    assert!(table.header_row().is_some());

    verifies!(
        r#"
When a horizontal alignment operator is also applied to the multiplier, then the vertical alignment operator is placed directly after the horizontal operator (e.g., `[cols="2*>.^"]`).

"#
    );

    let table = parse_table("[cols=\"2*>.^\"]\n|===\n|a |b\n|===");
    assert_eq!(
        column_alignments(&table),
        vec![
            (1, HorizontalAlignment::Right, VerticalAlignment::Middle),
            (1, HorizontalAlignment::Right, VerticalAlignment::Middle),
        ]
    );
}

#[test]
fn apply_horizontal_and_vertical_alignment_operators_to_the_same_column() {
    non_normative!(
        r#"
== Apply horizontal and vertical alignment operators to the same column

"#
    );

    verifies!(
        r#"
A column can have a vertical and horizontal alignment operator placed on its xref:add-columns.adoc#col-specifier[specifier].
The <<horizontal-operators,horizontal operator>> always precedes the <<vertical-operators,vertical operator>>.
Both operators precede the column width.
When a xref:add-columns.adoc#column-multiplier[multiplier] is used, the operators are placed after the multiplier.

.Horizontally and vertically align column content
[source#ex-center]
----
[cols="^.>2,1,>.^1"]
|===
|Column name |Column name |Column name

|This content is centered horizontally and aligned to the bottom
of the cell.
|There aren't any alignment operators on this column's specifier,
so the column falls back to the default alignments.
The default horizontal alignment is left-aligned.
The default vertical alignment is top-aligned.
|This content is aligned to the right side of the cell and
centered vertically.
|===
----

The table from <<ex-center>> is rendered below.

.Result of <<ex-center>>
[cols="^.>2,1,>.^1"]
|===
|Column name |Column name |Column name

|This content is centered horizontally and aligned to the bottom
of the cell.
|There aren't any alignment operators on this column's specifier,
so the column falls back to the default alignments.
The default horizontal alignment is left-aligned.
The default vertical alignment is top-aligned.
|This content is aligned to the right side of the cell and
centered vertically.
|===

"#
    );

    let table = parse_table(
        "[cols=\"^.>2,1,>.^1\"]\n|===\n|Column name |Column name |Column name\n\n|This content is centered horizontally and aligned to the bottom\nof the cell.\n|There aren't any alignment operators on this column's specifier,\nso the column falls back to the default alignments.\nThe default horizontal alignment is left-aligned.\nThe default vertical alignment is top-aligned.\n|This content is aligned to the right side of the cell and\ncentered vertically.\n|===",
    );
    assert_eq!(
        column_alignments(&table),
        vec![
            (2, HorizontalAlignment::Center, VerticalAlignment::Bottom),
            (1, HorizontalAlignment::Left, VerticalAlignment::Top),
            (1, HorizontalAlignment::Right, VerticalAlignment::Middle),
        ]
    );

    // Cell specifiers (and thus their per-cell alignment operators) are not yet
    // parsed, so the override of a column's alignment by a cell's alignment can't
    // be verified yet.
    to_do_verifies!(
        r#"
IMPORTANT: If there is an xref:align-by-cell.adoc[alignment operator on a cell's specifier], it will override the column's alignment operator.
"#
    );
}
