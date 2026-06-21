//! Tests for the basic PSV table block, covering the examples in
//! `docs/modules/tables/pages/build-a-basic-table.adoc`.

use crate::{
    HasSpan, Parser, Span,
    blocks::{
        Block, ColumnStyle, ContentModel, HorizontalAlignment, IsBlock, TableBlock,
        TableCellContent, VerticalAlignment,
    },
    content::SubstitutionGroup,
    parser::ModificationContext,
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
///
/// Every cell in these basic-table tests uses the default (inline) style, so a
/// cell is expected to hold [`TableCellContent::Simple`].
fn row_text(row: &crate::blocks::TableRow<'_>) -> Vec<String> {
    row.cells()
        .iter()
        .map(|cell| match cell.content() {
            TableCellContent::Simple(content) => content.rendered().to_string(),
            TableCellContent::AsciiDoc(_) => panic!("expected simple cell content"),
        })
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

#[test]
fn empty_table_caption_suppresses_the_label() {
    // An explicitly empty `table-caption` value (a distinct AsciiDoc operation
    // from a hard unset, e.g. `:!table-caption:`) is also treated as "no label":
    // each titled table keeps its title but receives no caption and does not
    // consume a table number. This exercises the empty-label guard separately
    // from the `Unset` path.
    let mut parser = Parser::default().with_intrinsic_attribute(
        "table-caption",
        "",
        ModificationContext::Anywhere,
    );
    let doc = parser.parse(".First\n|===\n|a\n|===\n\n.Second\n|===\n|b\n|===");

    let observed: Vec<(Option<&str>, Option<&str>)> = doc
        .nested_blocks()
        .filter_map(|block| match block {
            Block::Table(table) => Some((table.title(), table.caption())),
            _ => None,
        })
        .collect();

    assert_eq!(
        observed,
        vec![(Some("First"), None), (Some("Second"), None)]
    );
}

#[test]
fn caption_attribute_sets_the_label_verbatim() {
    // An explicit `caption` attribute provides the label exactly as written,
    // including its trailing space, with no automatically inserted number.
    let table =
        parse_table("[caption=\"Table A. \"]\n.A table with a custom label\n|===\n|a\n|===");

    assert_eq!(table.title(), Some("A table with a custom label"));
    assert_eq!(table.caption(), Some("Table A. "));
}

#[test]
fn caption_attribute_does_not_consume_a_table_number() {
    // A table labeled with an explicit `caption` is skipped by the document-wide
    // counter, so a following `table-caption` table is numbered as if the
    // explicitly captioned table were not there.
    let doc = Parser::default().parse(
        ".First\n|===\n|a\n|===\n\n[caption=\"Table A. \"]\n.Custom\n|===\n|b\n|===\n\n.Third\n|===\n|c\n|===",
    );

    let captions: Vec<Option<&str>> = doc
        .nested_blocks()
        .filter_map(|block| match block {
            Block::Table(table) => Some(table.caption()),
            _ => None,
        })
        .collect();

    assert_eq!(
        captions,
        vec![Some("Table 1. "), Some("Table A. "), Some("Table 2. ")]
    );
}

#[test]
fn caption_attribute_applies_even_when_table_caption_is_unset() {
    // The `caption` attribute is honored independently of `table-caption`, so it
    // still labels a titled table even when `table-caption` has been unset.
    let doc = Parser::default()
        .parse(":!table-caption:\n\n[caption=\"Forced. \"]\n.Numbers\n|===\n|a\n|===");

    let caption = doc.nested_blocks().find_map(|block| match block {
        Block::Table(table) => table.caption().map(|c| c.to_string()),
        _ => None,
    });

    assert_eq!(caption.as_deref(), Some("Forced. "));
}

#[test]
fn caption_attribute_is_ignored_without_a_title() {
    // The caption labels a title; with no title there is nothing to caption, so
    // an untitled table carries no caption even when `caption` is set.
    let table = parse_table("[caption=\"Table A. \"]\n|===\n|a\n|===");

    assert!(table.title().is_none());
    assert!(table.caption().is_none());
}

#[test]
fn empty_caption_attribute_suppresses_the_label() {
    // An explicitly empty `caption` (e.g. `[caption=]`) removes the label on the
    // table: the title is kept but no caption (and no number) is assigned, so the
    // title renders with no prefix.
    let table = parse_table("[caption=]\n.A table with a title but no label\n|===\n|a\n|===");

    assert_eq!(table.title(), Some("A table with a title but no label"));
    assert!(table.caption().is_none());
}

#[test]
fn empty_caption_attribute_does_not_consume_a_table_number() {
    // A table whose label is removed with an empty `caption` is skipped by the
    // document-wide counter, so a following `table-caption` table is numbered as
    // if the unlabeled table were not there.
    let doc = Parser::default().parse(
        ".First\n|===\n|a\n|===\n\n[caption=]\n.Unlabeled\n|===\n|b\n|===\n\n.Third\n|===\n|c\n|===",
    );

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
fn malformed_vertical_operator_falls_back_to_defaults() {
    // A dot in a column specifier introduces a vertical alignment operator, which
    // must be followed by `<`, `>`, or `^`. When the dot is followed by anything
    // else (here the letter `x`), the operator is malformed; rather than panic,
    // the parser leaves the dot unconsumed so the column falls back to the
    // default vertical alignment (top) and default width. The leftover `.x` is
    // not a recognized single-letter style operator, so the style also defaults.
    let table = parse_table("[cols=\".x,1\"]\n|===\n|a |b\n|===");

    let columns = table.columns();
    assert_eq!(columns.len(), 2);
    assert_eq!(columns[0].width(), 1);
    assert_eq!(columns[0].h_align(), HorizontalAlignment::Left);
    assert_eq!(columns[0].v_align(), VerticalAlignment::Top);
    assert_eq!(columns[0].style(), ColumnStyle::Default);
}

#[test]
fn literal_column_processes_content_verbatim() {
    // The `l` (literal) style processes a cell's content with the verbatim
    // substitution group: inline markup like `*z*` is left intact and only the
    // special characters are escaped, in contrast to the default style's normal
    // substitutions.
    let table = parse_table("[cols=\"l\"]\n|===\n|lit *z* and <x>\n|===");

    assert_eq!(table.columns()[0].style(), ColumnStyle::Literal);

    match table.body_rows()[0].cells()[0].content() {
        TableCellContent::Simple(content) => {
            assert_eq!(content.rendered(), "lit *z* and &lt;x&gt;");
        }
        TableCellContent::AsciiDoc(_) => panic!("expected simple cell content"),
    }
}

#[test]
fn malformed_style_operator_falls_back_to_default() {
    // The style operator is the entire remainder after the width, so trailing
    // junk (here `em`, perhaps a typo for the `e` style) is not a recognized
    // single-letter operator and the column falls back to the default style
    // rather than silently honoring the first letter.
    let table = parse_table("[cols=\"1em\"]\n|===\n|a\n|===");

    assert_eq!(table.columns()[0].style(), ColumnStyle::Default);
}

#[test]
fn asciidoc_cell_resolves_references_in_nested_blocks() {
    // Cross-references inside an AsciiDoc cell are resolved during the document's
    // reference-resolution pass, which descends into the cell's nested blocks.
    let doc = Parser::default()
        .parse("[#target]\nTarget paragraph.\n\n[cols=\"a\"]\n|===\n|See xref:target[].\n|===");

    let table = doc
        .nested_blocks()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .unwrap();

    let blocks = match table.body_rows()[0].cells()[0].content() {
        TableCellContent::AsciiDoc(blocks) => blocks,
        TableCellContent::Simple(_) => panic!("expected AsciiDoc cell content"),
    };

    let rendered = blocks[0].rendered_content().unwrap();
    assert!(
        rendered.contains("href=\"#target\""),
        "xref was not resolved: {rendered}"
    );
}

#[test]
fn asciidoc_cell_attributes_are_scoped_to_the_cell() {
    // An AsciiDoc cell inherits the parent document's attributes, but an
    // attribute it defines is scoped to the cell and does not leak back into the
    // parent document (matching Asciidoctor).
    let mut parser = Parser::default();
    let doc = parser.parse(
        ":parent-attr: inherited\n\n[cols=\"a\"]\n|===\n|\n:cell-attr: leaked\ncell sees: {parent-attr} {cell-attr}\n|===",
    );

    let table = doc
        .nested_blocks()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .unwrap();

    let blocks = match table.body_rows()[0].cells()[0].content() {
        TableCellContent::AsciiDoc(blocks) => blocks,
        TableCellContent::Simple(_) => panic!("expected AsciiDoc cell content"),
    };

    // Inside the cell, both the inherited parent attribute and the cell's own
    // attribute resolve.
    let rendered: String = blocks
        .iter()
        .filter_map(|block| block.rendered_content())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("inherited"),
        "cell did not inherit the parent attribute: {rendered}"
    );
    assert!(
        rendered.contains("leaked"),
        "cell did not see its own attribute: {rendered}"
    );

    // The attribute defined inside the cell did not leak into the parent, while
    // the parent's own attribute is unaffected.
    assert!(!parser.has_attribute("cell-attr"));
    assert!(parser.has_attribute("parent-attr"));
}

#[test]
fn cell_specifier_style_operator_locates_separator() {
    // A single lowercase letter directly in front of a `|` is a (style) cell
    // specifier, so the `|` is a cell separator: `a s|b` is two cells, not one.
    // The style operator is recognized only so the separator is located; its
    // styling effect is not yet applied, so the cell renders as plain content.
    let table = parse_table("|===\n|a s|b\n|===");

    assert_eq!(table.columns().len(), 2);
    let rows: Vec<_> = table.body_rows().iter().map(row_text).collect();
    assert_eq!(rows, vec![vec!["a".to_string(), "b".to_string()]]);
}

#[test]
fn cell_specifier_span_operator_without_factor_locates_separator() {
    // The span (`+`) and duplication (`*`) operators may appear without a count.
    // A bare `+` directly in front of a `|` is still a valid cell specifier, so
    // `a +|b` is two cells. (The span operator's layout effect is not yet
    // applied.)
    let table = parse_table("|===\n|a +|b\n|===");

    assert_eq!(table.columns().len(), 2);
    let rows: Vec<_> = table.body_rows().iter().map(row_text).collect();
    assert_eq!(rows, vec![vec!["a".to_string(), "b".to_string()]]);
}

#[test]
fn non_specifier_token_is_not_a_cell_separator() {
    // A token in front of a `|` that does not parse as a cell specifier (here the
    // word `foo`, which is more than a single style letter) means the `|` is not
    // a cell separator: `a foo|b` is a single cell whose content includes the
    // literal `|`.
    let table = parse_table("|===\n|a foo|b\n|===");

    assert_eq!(table.columns().len(), 1);
    let rows: Vec<_> = table.body_rows().iter().map(row_text).collect();
    assert_eq!(rows, vec![vec!["a foo|b".to_string()]]);
}
