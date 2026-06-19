//! Tests for the basic PSV table block, covering the examples in
//! `docs/modules/tables/pages/build-a-basic-table.adoc`.

use crate::{
    HasSpan, Parser, Span,
    blocks::{Block, ContentModel, IsBlock, TableBlock},
    content::SubstitutionGroup,
};

/// Parse `source` as a single block and return the [`TableBlock`] it produced.
fn parse_table(source: &str) -> TableBlock<'_> {
    let mut parser = Parser::default();
    let mi = Block::parse(Span::new(source), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    match mi.item {
        Block::Table(table) => table,
        other => panic!("expected a table block, got {other:?}"),
    }
}

/// Collect the rendered content of every cell in a row.
fn row_text(row: &crate::blocks::TableRow<'_>) -> Vec<String> {
    row.cells()
        .iter()
        .map(|cell| cell.content().rendered().to_string())
        .collect()
}

#[test]
fn two_columns_three_rows() {
    // From <<ex-rows>>: two columns via `cols`, three body rows, no header.
    let table = parse_table(
        "[cols=\"1,1\"]\n|===\n|Cell in column 1, row 1\n|Cell in column 2, row 1\n\n|Cell in column 1, row 2\n|Cell in column 2, row 2\n\n|Cell in column 1, row 3\n|Cell in column 2, row 3\n|===",
    );

    assert_eq!(table.content_model(), ContentModel::Table);
    assert_eq!(table.raw_context().as_ref(), "table");
    assert_eq!(table.columns().len(), 2);
    assert!(table.header_row().is_none());

    let rows: Vec<_> = table.body_rows().iter().map(row_text).collect();
    assert_eq!(
        rows,
        vec![
            vec![
                "Cell in column 1, row 1".to_string(),
                "Cell in column 2, row 1".to_string()
            ],
            vec![
                "Cell in column 1, row 2".to_string(),
                "Cell in column 2, row 2".to_string()
            ],
            vec![
                "Cell in column 1, row 3".to_string(),
                "Cell in column 2, row 3".to_string()
            ],
        ]
    );
}

#[test]
fn multiple_cells_on_one_line() {
    // From <<ex-rows>>: a row may place several cells on a single line, each
    // separated by a space followed by a vertical bar.
    let table = parse_table(
        "[cols=\"1,1\"]\n|===\n|Cell in column 1, row 1\n|Cell in column 2, row 1\n\n|Cell in column 1, row 2 |Cell in column 2, row 2\n|Cell in column 1, row 3 |Cell in column 2, row 3\n|===",
    );

    assert_eq!(table.body_rows().len(), 3);
    assert_eq!(
        row_text(&table.body_rows()[2]),
        vec![
            "Cell in column 1, row 3".to_string(),
            "Cell in column 2, row 3".to_string()
        ]
    );
}

#[test]
fn implicit_header_row() {
    // From <<ex-header>>: the first line after the delimiter is non-empty and is
    // followed by a blank line, so it becomes the header row.
    let table = parse_table(
        "[cols=\"1,1\"]\n|===\n|Cell in column 1, header row |Cell in column 2, header row\n\n|Cell in column 1, row 2\n|Cell in column 2, row 2\n\n|Cell in column 1, row 3\n|Cell in column 2, row 3\n|===",
    );

    let header = table.header_row().unwrap();
    assert_eq!(
        row_text(header),
        vec![
            "Cell in column 1, header row".to_string(),
            "Cell in column 2, header row".to_string()
        ]
    );

    assert_eq!(table.body_rows().len(), 2);
}

#[test]
fn implicit_columns_from_first_row() {
    // From add-columns.adoc <<ex-implicit>>: with no `cols` attribute and a blank
    // line before the first row, the column count comes from the first row's cell
    // count and there is no header.
    let table = parse_table(
        "|===\n\n|Cell in column 1, row 1 |Cell in column 2, row 1 |Cell in column 3, row 1\n\n|Cell in column 1, row 2\n|Cell in column 2, row 2\n|Cell in column 3, row 2\n|===",
    );

    assert_eq!(table.columns().len(), 3);
    assert!(table.header_row().is_none());
    assert_eq!(table.body_rows().len(), 2);
    assert_eq!(
        row_text(&table.body_rows()[0]),
        vec![
            "Cell in column 1, row 1".to_string(),
            "Cell in column 2, row 1".to_string(),
            "Cell in column 3, row 1".to_string()
        ]
    );
}

#[test]
fn column_multiplier() {
    // From add-columns.adoc: `cols="5,3*"` yields one column then three more.
    let table = parse_table("[cols=\"5,3*\"]\n|===\n|a |b |c |d\n|===");

    let widths: Vec<usize> = table.columns().iter().map(|c| c.width()).collect();
    assert_eq!(widths, vec![5, 1, 1, 1]);
}

#[test]
fn cell_content_is_substituted() {
    // Cell content flows through the normal substitution pipeline.
    let table = parse_table("|===\n|*bold* and _italic_\n|===");

    assert_eq!(
        row_text(&table.body_rows()[0]),
        vec!["<strong>bold</strong> and <em>italic</em>".to_string()]
    );
}

#[test]
fn leading_and_trailing_whitespace_stripped() {
    // From <<ex-more-cells>>: leading and trailing spaces around cell content are
    // stripped.
    let table = parse_table("[cols=\"1,1\"]\n|===\n|a |    b spaced\n|===");

    assert_eq!(
        row_text(&table.body_rows()[0]),
        vec!["a".to_string(), "b spaced".to_string()]
    );
}

#[test]
fn block_is_recognized_via_debug() {
    let mut parser = Parser::default();
    let mi = Block::parse(Span::new("|===\n|a |b\n|==="), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    let debug_output = format!("{:?}", mi.item);
    assert!(debug_output.starts_with("Block::Table"));
}

#[test]
fn unterminated_table_warns() {
    let mut parser = Parser::default();
    let maw = Block::parse(Span::new("|===\n|a |b"), &mut parser);

    assert!(maw.warnings.iter().any(|w| matches!(
        w.warning,
        crate::warnings::WarningType::UnterminatedDelimitedBlock
    )));
}

#[test]
fn block_level_accessors() {
    // Exercise every `IsBlock`/`HasSpan` accessor through the `Block` enum
    // (rather than the unwrapped `TableBlock`) so the delegating match arms in
    // `block.rs` are covered.
    let mut parser = Parser::default();
    let block = Block::parse(Span::new("|===\n|a |b\n|==="), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap()
        .item;

    assert_eq!(block.content_model(), ContentModel::Table);
    assert_eq!(block.raw_context().as_ref(), "table");
    assert_eq!(block.resolved_context().as_ref(), "table");
    assert!(block.rendered_content().is_none());
    assert_eq!(block.nested_blocks().count(), 0);
    assert!(block.declared_style().is_none());
    assert!(block.id().is_none());
    assert!(block.roles().is_empty());
    assert!(block.options().is_empty());
    assert!(block.title_source().is_none());
    assert!(block.title().is_none());
    assert!(block.anchor().is_none());
    assert!(block.anchor_reftext().is_none());
    assert!(block.attrlist().is_none());
    assert_eq!(block.substitution_group(), SubstitutionGroup::Normal);
    assert_eq!(block.span().data(), "|===\n|a |b\n|===");
}

#[test]
fn escaped_cell_separator() {
    // A backslash-escaped separator (`\|`) is not a cell boundary; the backslash
    // is stripped from the rendered cell content.
    let table = parse_table("|===\n|a \\| b\n|===");

    assert_eq!(table.body_rows().len(), 1);
    assert_eq!(row_text(&table.body_rows()[0]), vec!["a | b".to_string()]);
}

#[test]
fn cols_with_empty_specifier() {
    // Empty entries in the `cols` list (e.g. from a doubled comma) are skipped.
    let table = parse_table("[cols=\"1,,1\"]\n|===\n|a |b\n|===");

    assert_eq!(table.columns().len(), 2);
}

#[test]
fn no_cols_and_first_line_without_a_cell() {
    // With no `cols` attribute and a first line that contains no cell separator,
    // the column count is zero and the body loop is skipped entirely.
    let table = parse_table("|===\nnot a cell\n|===");

    assert!(table.columns().is_empty());
    assert!(table.header_row().is_none());
    assert!(table.body_rows().is_empty());
}

#[test]
fn header_option_without_cells() {
    // The `header` option forces header handling even when the table has no
    // cells, exercising the empty-header-row branch.
    let table = parse_table("[%header,cols=\"1,1\"]\n|===\nnot a cell\n|===");

    assert_eq!(table.columns().len(), 2);
    assert!(table.header_row().is_none());
    assert!(table.body_rows().is_empty());
}

#[test]
fn titled_table_is_captioned() {
    // A block title on a table produces both a title and an automatic caption
    // ("Table 1. ") drawn from the default `table-caption` value.
    let table = parse_table(".A table with a title\n|===\n|a |b\n|===");

    assert_eq!(table.title(), Some("A table with a title"));
    assert_eq!(table.caption(), Some("Table 1. "));
}

#[test]
fn untitled_table_has_no_caption() {
    // Without a title, a table is not captioned and does not consume a number.
    let table = parse_table("|===\n|a |b\n|===");

    assert!(table.title().is_none());
    assert!(table.caption().is_none());
}

#[test]
fn captioned_tables_are_numbered_in_document_order() {
    // Only titled tables consume a number, and they are numbered in document
    // order across the whole document.
    let doc = Parser::default()
        .parse(".First\n|===\n|a\n|===\n\n|===\n|b\n|===\n\n.Second\n|===\n|c\n|===");

    let captions: Vec<Option<&str>> = doc
        .nested_blocks()
        .filter_map(|block| match block {
            Block::Table(table) => Some(table.caption()),
            _ => None,
        })
        .collect();

    assert_eq!(captions, vec![Some("Table 1. "), None, Some("Table 2. ")]);
}

#[test]
fn table_caption_can_be_relabeled() {
    // The label portion of the caption is taken from the `table-caption`
    // document attribute.
    let doc = Parser::default().parse(":table-caption: Spreadsheet\n\n.Numbers\n|===\n|a\n|===");

    let caption = doc.nested_blocks().find_map(|block| match block {
        Block::Table(table) => table.caption().map(|c| c.to_string()),
        _ => None,
    });

    assert_eq!(caption.as_deref(), Some("Spreadsheet 1. "));
}

#[test]
fn unsetting_table_caption_suppresses_the_label() {
    // When `table-caption` is unset, a titled table keeps its title but receives
    // no caption (and no number).
    let doc = Parser::default().parse(":!table-caption:\n\n.Numbers\n|===\n|a\n|===");

    let table = doc
        .nested_blocks()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .unwrap();

    assert_eq!(table.title(), Some("Numbers"));
    assert!(table.caption().is_none());
}
