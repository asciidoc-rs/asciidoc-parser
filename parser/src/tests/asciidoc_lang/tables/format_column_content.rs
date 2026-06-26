use crate::tests::prelude::*;

track_file!("docs/modules/tables/pages/format-column-content.adoc");

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

/// Collect each column's style.
fn column_styles(table: &crate::blocks::TableBlock<'_>) -> Vec<ColumnStyle> {
    table.columns().iter().map(|c| c.style()).collect()
}

/// Return the rendered text of a [`Simple`](crate::blocks::TableCellContent)
/// cell, panicking if the cell holds AsciiDoc block content instead.
fn simple_text(cell: &crate::blocks::TableCell<'_>) -> String {
    match cell.content() {
        crate::blocks::TableCellContent::Simple(content) => content.rendered().to_string(),
        crate::blocks::TableCellContent::AsciiDoc(_) => panic!("expected simple cell content"),
    }
}

/// Parse `source` as a standalone document and return the rendered HTML of its
/// first (paragraph) block.
fn rendered_paragraph(source: &str) -> String {
    let doc = Parser::default().parse(source);
    let crate::blocks::Block::Simple(para) = &doc.nested_blocks().next().unwrap() else {
        panic!("expected a paragraph");
    };
    para.content().rendered().to_string()
}

non_normative!(
    r#"
= Format Content by Column

"#
);

#[test]
fn intro() {
    verifies!(
        r#"
A column style operator is applied to a column specifier and xref:add-columns.adoc#cols-attribute[assigned to the cols attribute].

"#
    );

    // A style operator on a column specifier in the `cols` attribute is applied
    // to that column.
    let table = parse_table("[cols=\"e\"]\n|===\n|content\n|===");
    assert_eq!(column_styles(&table), vec![ColumnStyle::Emphasis]);
}

#[test]
fn column_styles_and_their_operators() {
    non_normative!(
        r#"
[#cols-style]
== Column styles and their operators

"#
    );

    verifies!(
        r#"
You can style all of the content in a column by adding a style operator to a column's specifier.

"#
    );

    // The style operator governs the whole column: every body cell in the
    // column is processed with the column's style. (The two adjacent rows are
    // both body rows because no blank line follows the first, so neither
    // becomes a header.)
    let table = parse_table("[cols=\"s,m\"]\n|===\n|a |b\n|c |d\n|===");
    assert_eq!(
        column_styles(&table),
        vec![ColumnStyle::Strong, ColumnStyle::Monospace]
    );
    assert_eq!(table.body_rows().len(), 2);

    verifies!(
        r#"
include::partial$style-operators.adoc[]

"#
    );

    // Expansion of `partial$style-operators.adoc`: a three-column reference
    // table listing each style and its operator. Its first row is an implicit
    // header (non-empty first line followed by a blank line), and it has one
    // body row per documented style (AsciiDoc, Default, Emphasis, Header,
    // Literal, Monospace, Strong).
    let style_operators = parse_table(
        "|===\n|Style |Operator |Description\n\n|AsciiDoc\n|`a`\n|Supports block elements (lists, delimited blocks, and block macros).\nThis style effectively creates a nested, standalone AsciiDoc document.\nThe parent document's implicit attributes, such as `doctitle`, are shadowed and custom attributes are inherited.\n// what does \"shadowed\" actually mean???\n\n|Default\n|`d`\n|All of the markup that is permitted in a paragraph (i.e., inline formatting, inline macros) is supported.\n\n|Emphasis\n|`e`\n|Text is italicized.\n\n|Header\n|`h`\n|Applies the header semantics and styles to the text and cell borders.\n\n|Literal\n|`l`\n|Content is treated as if it were inside a literal block.\n\n|Monospace\n|`m`\n|Text is rendered using a monospace font.\n\n|Strong\n|`s`\n|Text is bold.\n|===",
    );
    assert_eq!(style_operators.columns().len(), 3);
    assert_eq!(
        style_operators
            .header_row()
            .unwrap()
            .cells()
            .iter()
            .map(simple_text)
            .collect::<Vec<_>>(),
        vec!["Style", "Operator", "Description"]
    );
    assert_eq!(style_operators.body_rows().len(), 7);

    verifies!(
        r#"
When a style operator isn't explicitly applied to a column specifier, the `d` style is assigned automatically and the column is processed as paragraph text.

"#
    );

    // With no style operator, a column is assigned the default (`d`) style.
    let table = parse_table("[cols=\"1,1\"]\n|===\n|a |b\n|===");
    assert_eq!(
        column_styles(&table),
        vec![ColumnStyle::Default, ColumnStyle::Default]
    );

    // The default style can also be requested explicitly with the `d` operator.
    let table = parse_table("[cols=\"d,d\"]\n|===\n|a |b\n|===");
    assert_eq!(
        column_styles(&table),
        vec![ColumnStyle::Default, ColumnStyle::Default]
    );
}

#[test]
fn apply_a_style_operator_to_a_column() {
    non_normative!(
        r#"
== Apply a style operator to a column

"#
    );

    verifies!(
        r#"
A style operator is always placed in the last position on a column's specifier or multiplier.

* `[cols=">pass:q[#e#],.^3pass:q[#s#]"]` A style operator is placed directly after any other operators and the column width in the column's specifier.
* `[cols="pass:q[#h#],pass:q[#e#]"]` When a column width isn't specified, the style operator can represent both the column and the column's content style.
* `[cols="3*.>pass:q[#m#]"]` When a multiplier is present, the style operator is placed after any horizontal and vertical alignment operators.

"#
    );

    // The style operator follows any alignment operators and the column width.
    let table = parse_table("[cols=\">e,.^3s\"]\n|===\n|a |b\n|===");
    let columns = table.columns();
    assert_eq!(columns[0].h_align(), HorizontalAlignment::Right);
    assert_eq!(columns[0].width(), 1);
    assert_eq!(columns[0].style(), ColumnStyle::Emphasis);
    assert_eq!(columns[1].v_align(), VerticalAlignment::Middle);
    assert_eq!(columns[1].width(), 3);
    assert_eq!(columns[1].style(), ColumnStyle::Strong);

    // When a column width isn't specified, the style operator can represent both
    // the column and its content style.
    let table = parse_table("[cols=\"h,e\"]\n|===\n|a |b\n|===");
    let columns = table.columns();
    assert_eq!(
        (columns[0].width(), columns[0].style()),
        (1, ColumnStyle::Header)
    );
    assert_eq!(
        (columns[1].width(), columns[1].style()),
        (1, ColumnStyle::Emphasis)
    );

    // When a multiplier is present, the style operator follows the alignment
    // operators on the (single) specifier that the multiplier repeats.
    let table = parse_table("[cols=\"3*.>m\"]\n|===\n|a |b |c\n|===");
    assert_eq!(
        column_styles(&table),
        vec![
            ColumnStyle::Monospace,
            ColumnStyle::Monospace,
            ColumnStyle::Monospace
        ]
    );
    for column in table.columns() {
        assert_eq!(column.v_align(), VerticalAlignment::Bottom);
    }

    verifies!(
        r#"
Let's apply a different style to each column in <<ex-style>>.

.Add a style operator to each column
[source#ex-style]
----
[cols="h,m,s,e"]
|===
|Column 1 |Column 2 |Column 3 |Column 4

|This column's content and borders are rendered using the table header (`h`) styles.
|This column's content is rendered using a monospace font (m).
|This column's content is bold (`s`).
|This column's content is italicized (`e`).

|This column's content and borders are rendered using the table header (`h`) styles.
|This column's content is rendered using a monospace font (m).
|This column's content is bold (`s`).
|This column's content is italicized (`e`).
|===
----

"#
    );

    // The `ex-style` example applies a different style operator to each of the
    // four columns.
    let ex_style = parse_table(
        "[cols=\"h,m,s,e\"]\n|===\n|Column 1 |Column 2 |Column 3 |Column 4\n\n|This column's content and borders are rendered using the table header (`h`) styles.\n|This column's content is rendered using a monospace font (m).\n|This column's content is bold (`s`).\n|This column's content is italicized (`e`).\n|This column's content and borders are rendered using the table header (`h`) styles.\n|This column's content is rendered using a monospace font (m).\n|This column's content is bold (`s`).\n|This column's content is italicized (`e`).\n|===",
    );
    assert_eq!(
        column_styles(&ex_style),
        vec![
            ColumnStyle::Header,
            ColumnStyle::Monospace,
            ColumnStyle::Strong,
            ColumnStyle::Emphasis,
        ]
    );

    verifies!(
        r#"
The table from <<ex-style>> is displayed below.
Note that the style applied to each column doesn't affect the xref:add-header-row.adoc[header row] or override any inline formatting.

.Result of <<ex-style>>
[cols="h,m,s,e"]
|===
|Column 1 |Column 2 |Column 3 |Column 4

|This column's content and borders are rendered using the table header (`h`) styles.
|This column's content is rendered using a monospace font (m).
|This column's content is bold (`s`).
|This column's content is italicized (`e`).

|This column's content and borders are rendered using the table header (`h`) styles.
|This column's content is rendered using a monospace font (m).
|This column's content is bold (`s`).
|This column's content is italicized (`e`).
|===

"#
    );

    // The rendered result of <<ex-style>>: the style doesn't affect the header
    // row (each header cell holds plain inline content with no style applied,
    // even though its column is styled)...
    let ex_style_result = parse_table(
        "[cols=\"h,m,s,e\"]\n|===\n|Column 1 |Column 2 |Column 3 |Column 4\n\n|This column's content and borders are rendered using the table header (`h`) styles.\n|This column's content is rendered using a monospace font (m).\n|This column's content is bold (`s`).\n|This column's content is italicized (`e`).\n|This column's content and borders are rendered using the table header (`h`) styles.\n|This column's content is rendered using a monospace font (m).\n|This column's content is bold (`s`).\n|This column's content is italicized (`e`).\n|===",
    );
    let header = ex_style_result.header_row().unwrap();
    let header_text: Vec<String> = header.cells().iter().map(simple_text).collect();
    assert_eq!(
        header_text,
        vec!["Column 1", "Column 2", "Column 3", "Column 4"]
    );

    // ...nor does it override inline formatting: the body cell in the
    // header-styled column still resolves its inline backtick markup to a
    // monospace span.
    let body = &ex_style_result.body_rows()[0];
    assert!(simple_text(&body.cells()[0]).contains("<code>h</code>"));

    verifies!(
        r#"
Additionally, if a xref:format-cell-content.adoc#override-column-style[cell specifier contains a style operator], that style will override a column's style operator.

"#
    );

    // The cross-reference to the cell-specifier override behavior renders as a
    // link to the format-cell-content page.
    let rendered = rendered_paragraph(
        "Additionally, if a xref:format-cell-content.adoc#override-column-style[cell specifier contains a style operator], that style will override a column's style operator.",
    );
    assert!(rendered.contains("<a href="));
    assert!(rendered.contains("cell specifier contains a style operator</a>"));
}

#[test]
fn use_asciidoc_block_elements_in_a_column() {
    non_normative!(
        r#"
== Use AsciiDoc block elements in a column

"#
    );

    verifies!(
        r#"
To use AsciiDoc block elements, such as delimited source blocks and lists, in a column, place the lowercase letter `a` on the column specifier.

"#
    );

    // The `a` operator marks a column as AsciiDoc; other columns keep the
    // default style.
    let table = parse_table("[cols=\"2a,2\"]\n|===\n|a |b\n|===");
    assert_eq!(
        column_styles(&table),
        vec![ColumnStyle::AsciiDoc, ColumnStyle::Default]
    );

    verifies!(
        r#"
.Apply the AsciiDoc block style operator to the first column
[source#ex-asciidoc]
....
[cols="2a,2"]
|===
|Column with the `a` style operator applied to its specifier |Column using the default style

|
* List item 1
* List item 2
* List item 3
|
* List item 1
* List item 2
* List item 3

|
[source,python]
----
import os
print "%s" %(os.uname())
----
|
[source,python]
----
import os
print ("%s" %(os.uname()))
----
|===
....

"#
    );

    // The `ex-asciidoc` example applies the AsciiDoc block style (`a`) to the
    // first column, so its cells parse nested block content (a list, then a
    // delimited source block) while the default-styled column keeps the same
    // markup as inline content.
    let ex_asciidoc = parse_table(
        "[cols=\"2a,2\"]\n|===\n|Column with the `a` style operator applied to its specifier |Column using the default style\n\n|\n* List item 1\n* List item 2\n* List item 3\n|\n* List item 1\n* List item 2\n* List item 3\n\n|\n[source,python]\n----\nimport os\nprint \"%s\" %(os.uname())\n----\n|\n[source,python]\n----\nimport os\nprint (\"%s\" %(os.uname()))\n----\n|===",
    );
    assert_eq!(
        column_styles(&ex_asciidoc),
        vec![ColumnStyle::AsciiDoc, ColumnStyle::Default]
    );

    // First body row: the AsciiDoc cell parses a list block; the default cell
    // keeps the list markup as plain inline text.
    let row = &ex_asciidoc.body_rows()[0];
    match row.cells()[0].content() {
        crate::blocks::TableCellContent::AsciiDoc(cell) => {
            let blocks = cell.blocks();
            assert_eq!(blocks.len(), 1);
            assert_eq!(blocks[0].raw_context().as_ref(), "list");
        }
        other => panic!("expected nested AsciiDoc blocks, got {other:?}"),
    }
    assert!(matches!(
        row.cells()[1].content(),
        crate::blocks::TableCellContent::Simple(_)
    ));

    // Second body row: the AsciiDoc cell parses the delimited source block.
    let row = &ex_asciidoc.body_rows()[1];
    match row.cells()[0].content() {
        crate::blocks::TableCellContent::AsciiDoc(cell) => {
            let blocks = cell.blocks();
            assert_eq!(blocks.len(), 1);
            assert_eq!(blocks[0].raw_context().as_ref(), "listing");
        }
        other => panic!("expected nested AsciiDoc blocks, got {other:?}"),
    }

    verifies!(
        r#"
The AsciiDoc block style effectively creates a nested, standalone AsciiDoc document in each cell in the column.
"#
    );

    // A full structural comparison of a minimal AsciiDoc cell: its content is a
    // nested list block parsed as a standalone document fragment.
    use crate::blocks::ListType;
    assert_eq!(
        TableBlock {
            columns: &[TableColumn {
                width: 1,
                h_align: HorizontalAlignment::Left,
                v_align: VerticalAlignment::Top,
                style: ColumnStyle::AsciiDoc,
            }],
            header_row: None,
            body_rows: &[TableRow {
                cells: &[TableCell {
                    content: TableCellContent::AsciiDoc(&[Block::List(ListBlock {
                        type_: ListType::Unordered,
                        items: &[Block::ListItem(ListItem {
                            marker: ListItemMarker::Asterisks(Span {
                                data: "*",
                                line: 3,
                                col: 3,
                                offset: 18,
                            }),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "item one",
                                        line: 3,
                                        col: 5,
                                        offset: 20,
                                    },
                                    rendered: "item one",
                                },
                                source: Span {
                                    data: "item one",
                                    line: 3,
                                    col: 5,
                                    offset: 20,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            })],
                            source: Span {
                                data: "* item one",
                                line: 3,
                                col: 3,
                                offset: 18,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        })],
                        source: Span {
                            data: "* item one",
                            line: 3,
                            col: 3,
                            offset: 18,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    })]),
                }],
            }],
            footer_row: None,
            source: Span {
                data: "[cols=\"a\"]\n|===\n| * item one\n|===",
                line: 1,
                col: 1,
                offset: 0,
            },
            title_source: None,
            title: None,
            caption: None,
            anchor: None,
            anchor_reftext: None,
            attrlist: Some(Attrlist {
                attributes: &[ElementAttribute {
                    name: Some("cols"),
                    shorthand_items: &[],
                    value: "a",
                }],
                anchor: None,
                source: Span {
                    data: "cols=\"a\"",
                    line: 1,
                    col: 2,
                    offset: 1,
                },
            }),
        },
        parse_table("[cols=\"a\"]\n|===\n| * item one\n|===")
    );

    non_normative!(
        r#"
The parent document's implicit attributes, such as `doctitle`, are shadowed and custom attributes are inherited.
// what does "shadowed" actually mean???

"#
    );

    verifies!(
        r#"
.Result of <<ex-asciidoc>>
[cols="2a,2"]
|===
|Column with the `a` style operator applied to its specifier |Column using the default style

|
* List item 1
* List item 2
* List item 3
|
* List item 1
* List item 2
* List item 3

|
[source,python]
----
import os
print "%s" %(os.uname())
----

|
[source,python]
----
import os
print ("%s" %(os.uname()))
----
|===

"#
    );

    // The rendered result of <<ex-asciidoc>>: the first column is AsciiDoc, the
    // second is default, and every body cell in the AsciiDoc column renders as
    // nested block content.
    let ex_asciidoc_result = parse_table(
        "[cols=\"2a,2\"]\n|===\n|Column with the `a` style operator applied to its specifier |Column using the default style\n\n|\n* List item 1\n* List item 2\n* List item 3\n|\n* List item 1\n* List item 2\n* List item 3\n\n|\n[source,python]\n----\nimport os\nprint \"%s\" %(os.uname())\n----\n\n|\n[source,python]\n----\nimport os\nprint (\"%s\" %(os.uname()))\n----\n|===",
    );
    assert_eq!(
        column_styles(&ex_asciidoc_result),
        vec![ColumnStyle::AsciiDoc, ColumnStyle::Default]
    );
    for row in ex_asciidoc_result.body_rows() {
        assert!(matches!(
            row.cells()[0].content(),
            crate::blocks::TableCellContent::AsciiDoc(_)
        ));
    }

    verifies!(
        r#"
You can also apply the xref:format-cell-content.adoc#a-operator[AsciiDoc block style operator to individual cells].
"#
    );

    // The closing cross-reference renders as a link to the format-cell-content
    // page's section on the per-cell AsciiDoc operator.
    let rendered = rendered_paragraph(
        "You can also apply the xref:format-cell-content.adoc#a-operator[AsciiDoc block style operator to individual cells].",
    );
    assert!(rendered.contains("<a href="));
    assert!(rendered.contains("AsciiDoc block style operator to individual cells</a>"));
}
