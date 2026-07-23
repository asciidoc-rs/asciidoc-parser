//! Tests for the basic PSV table block, covering the examples in
//! `ref/asciidoc-lang/docs/modules/tables/pages/build-a-basic-table.adoc`.

use crate::{
    HasSpan, Parser, Span,
    blocks::{
        Block, ColumnStyle, ContentModel, DataFormat, HorizontalAlignment, IsBlock, TableBlock,
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
fn table_width_attribute() {
    // The `width` attribute reports an integer percentage; the trailing `%` is
    // optional.
    assert_eq!(parse_table("[width=75%]\n|===\n|a\n|===").width(), Some(75));
    assert_eq!(parse_table("[width=75]\n|===\n|a\n|===").width(), Some(75));

    // No `width` attribute means no fixed width.
    assert_eq!(parse_table("|===\n|a\n|===").width(), None);

    // Values outside the 1-to-100 range, or non-integer values, are ignored.
    assert_eq!(parse_table("[width=0]\n|===\n|a\n|===").width(), None);
    assert_eq!(parse_table("[width=101]\n|===\n|a\n|===").width(), None);
    assert_eq!(parse_table("[width=half]\n|===\n|a\n|===").width(), None);
}

#[test]
fn autowidth_option_makes_columns_autowidth() {
    // The shorthand `%autowidth` and the formal `options="autowidth"` both set
    // the table to autowidth, and every column inherits the setting.
    for source in [
        "[%autowidth]\n|===\n|a |b\n|===",
        "[options=\"autowidth\"]\n|===\n|a |b\n|===",
    ] {
        let table = parse_table(source);
        assert!(table.is_autowidth());
        assert!(table.columns().iter().all(|c| c.is_autowidth()));
    }

    // A column that inherits autowidth from the table's `autowidth` option
    // still becomes autowidth, but retains the explicit width from its
    // specifier (the width is simply not used to size an autowidth column).
    let table = parse_table("[%autowidth,cols=\"2,1\"]\n|===\n|a |b\n|===");
    assert!(table.columns().iter().all(|c| c.is_autowidth()));
    let widths: Vec<usize> = table.columns().iter().map(|c| c.width()).collect();
    assert_eq!(widths, vec![2, 1]);

    // Without the option, the table and its columns keep proportional widths.
    let table = parse_table("|===\n|a |b\n|===");
    assert!(!table.is_autowidth());
    assert!(table.columns().iter().all(|c| !c.is_autowidth()));
}

#[test]
fn autowidth_column_via_tilde() {
    // The `~` width value marks only that column as autowidth, leaving the
    // others at their explicit width; the table itself is not autowidth.
    let table = parse_table("[cols=\"25h,~,~\"]\n|===\n|a |b |c\n|===");

    let columns: Vec<(usize, bool)> = table
        .columns()
        .iter()
        .map(|c| (c.width(), c.is_autowidth()))
        .collect();
    assert_eq!(columns, vec![(25, false), (1, true), (1, true)]);
    assert!(!table.is_autowidth());
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
fn separator_after_backslash_is_escaped() {
    // The escape check inspects only the single byte before the separator, so a
    // separator preceded by a backslash is escaped even when that backslash is
    // itself preceded by another one. `|a\\|b` is therefore one cell whose
    // content is `a\|b` (the escaping backslash is stripped), matching
    // Asciidoctor, whose check is likewise the single-character
    // `pre_match.end_with? '\'`.
    let table = parse_table("|===\n|a\\\\|b\n|===");

    assert_eq!(table.body_rows().len(), 1);
    assert_eq!(row_text(&table.body_rows()[0]), vec!["a\\|b".to_string()]);
}

#[test]
fn cols_with_empty_specifier() {
    // An empty entry in the `cols` list (e.g. from a doubled comma) contributes a
    // default column, matching Asciidoctor: `cols="1,,1"` yields three columns.
    let table = parse_table("[cols=\"1,,1\"]\n|===\n|a |b |c\n|===");

    assert_eq!(table.columns().len(), 3);
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
    assert_eq!(table.number(), Some(1));
}

#[test]
fn untitled_table_has_no_caption() {
    // Without a title, a table is not captioned and does not consume a number.
    let table = parse_table("|===\n|a |b\n|===");

    assert!(table.title().is_none());
    assert!(table.caption().is_none());
    assert!(table.number().is_none());
}

#[test]
fn caption_and_number_are_reported_through_the_isblock_trait() {
    // A generic `T: IsBlock` consumer resolves to the trait methods rather than
    // the inherent ones, so a captioned table must be reported correctly through
    // the trait interface too (not silently `None`).
    fn via_trait<'src>(block: &impl IsBlock<'src>) -> (Option<String>, Option<usize>) {
        (block.caption().map(str::to_string), block.number())
    }

    let table = parse_table(".A table with a title\n|===\n|a |b\n|===");
    assert_eq!(via_trait(&table), (Some("Table 1. ".to_string()), Some(1)));

    let untitled = parse_table("|===\n|a |b\n|===");
    assert_eq!(via_trait(&untitled), (None, None));
}

#[test]
fn captioned_tables_are_numbered_in_document_order() {
    // Only titled tables consume a number, and they are numbered in document
    // order across the whole document.
    let doc = Parser::default()
        .parse(".First\n|===\n|a\n|===\n\n|===\n|b\n|===\n\n.Second\n|===\n|c\n|===");

    let captions: Vec<(Option<&str>, Option<usize>)> = doc
        .nested_blocks()
        .filter_map(|block| match block {
            Block::Table(table) => Some((table.caption(), table.number())),
            _ => None,
        })
        .collect();

    assert_eq!(
        captions,
        vec![
            (Some("Table 1. "), Some(1)),
            (None, None),
            (Some("Table 2. "), Some(2)),
        ]
    );
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

    let captions: Vec<(Option<&str>, Option<usize>)> = doc
        .nested_blocks()
        .filter_map(|block| match block {
            Block::Table(table) => Some((table.caption(), table.number())),
            _ => None,
        })
        .collect();

    // The explicitly-captioned middle table carries its label but no number.
    assert_eq!(
        captions,
        vec![
            (Some("Table 1. "), Some(1)),
            (Some("Table A. "), None),
            (Some("Table 2. "), Some(2)),
        ]
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

    let captions: Vec<(Option<&str>, Option<usize>)> = doc
        .nested_blocks()
        .filter_map(|block| match block {
            Block::Table(table) => Some((table.caption(), table.number())),
            _ => None,
        })
        .collect();

    assert_eq!(
        captions,
        vec![
            (Some("Table 1. "), Some(1)),
            (None, None),
            (Some("Table 2. "), Some(2)),
        ]
    );
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
        TableCellContent::AsciiDoc(cell) => cell.blocks(),
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
        TableCellContent::AsciiDoc(cell) => cell.blocks(),
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
fn asciidoc_cell_is_always_nested() {
    // An AsciiDoc cell is a nested, standalone document, so `is_nested()` is
    // always true (mirroring Asciidoctor's `Document#nested?`).
    let doc = Parser::default().parse("[cols=\"a\"]\n|===\n|nested content\n|===");

    let table = doc
        .nested_blocks()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .unwrap();

    match table.body_rows()[0].cells()[0].content() {
        TableCellContent::AsciiDoc(cell) => assert!(cell.is_nested()),
        TableCellContent::Simple(_) => panic!("expected AsciiDoc cell content"),
    }
}

#[test]
fn asciidoc_cell_exposes_inherited_attributes_for_introspection() {
    // The cell's attribute state is scoped to the cell and restored on the parser
    // once the cell is parsed, so it can no longer be read from the parser. The
    // cell nonetheless retains a snapshot of it, letting a caller introspect the
    // nested document's attributes — both those inherited from the parent and any
    // the cell set for itself — after parsing has finished.
    let doc = Parser::default().parse(
        ":parent-attr: inherited\n\n[cols=\"a\"]\n|===\n|\n:cell-attr: local\ncontent\n|===",
    );

    let table = doc
        .nested_blocks()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .unwrap();

    let TableCellContent::AsciiDoc(cell) = table.body_rows()[0].cells()[0].content() else {
        panic!("expected AsciiDoc cell content");
    };

    // The inherited parent attribute is visible on the nested document and equal
    // to what the parent document sees.
    assert!(cell.is_attribute_set("parent-attr"));
    assert_eq!(
        cell.attribute_value("parent-attr"),
        doc.attribute_value("parent-attr")
    );

    // The attribute the cell set for itself is visible on the cell but did not
    // leak into the parent document.
    assert!(cell.is_attribute_set("cell-attr"));
    assert!(cell.has_attribute("cell-attr"));
    assert!(!doc.has_attribute("cell-attr"));

    // An attribute that neither the parent nor the cell defines is absent.
    assert!(!cell.has_attribute("no-such-attr"));
    assert!(!cell.is_attribute_set("no-such-attr"));
}

#[test]
fn include_expanded_asciidoc_cell_exposes_inherited_attributes_for_introspection() {
    // A cell whose first line is an `include::` directive is parsed from an
    // owned, include-expanded source (the `AsciiDocCell::Owned` variant) rather
    // than borrowed from the document source. The introspection accessors reach
    // through that owned store just the same, so the nested document still
    // reports itself nested and exposes the attributes it inherited.
    use crate::{SafeMode, tests::fixtures::inline_file_handler::InlineFileHandler};

    let handler = InlineFileHandler::from_pairs([("fixtures/cell.adoc", "included cell content")]);
    let doc = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler)
        .with_intrinsic_attribute("outdir", "/path/to/output", ModificationContext::ApiOnly)
        .parse("|===\na|include::fixtures/cell.adoc[]\n|===");

    let table = doc
        .nested_blocks()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .unwrap();

    let TableCellContent::AsciiDoc(cell) = table.body_rows()[0].cells()[0].content() else {
        panic!("expected AsciiDoc cell content");
    };

    // The cell is the include-expanded (owned) variant, so this exercises the
    // owned-store branch of the introspection accessors.
    assert!(cell.is_nested());
    assert!(cell.is_attribute_set("outdir"));
    assert_eq!(
        cell.attribute_value("outdir"),
        doc.attribute_value("outdir")
    );
    assert!(!cell.has_attribute("no-such-attr"));
}

/// Collect the rendered text of every block in an AsciiDoc cell.
fn asciidoc_cell_text(table: &TableBlock<'_>, row: usize, col: usize) -> String {
    match table.body_rows()[row].cells()[col].content() {
        TableCellContent::AsciiDoc(cell) => cell
            .blocks()
            .iter()
            .filter_map(|block| block.rendered_content())
            .collect::<Vec<_>>()
            .join("\n"),
        TableCellContent::Simple(_) => panic!("expected AsciiDoc cell content"),
    }
}

#[test]
fn asciidoc_cell_cannot_modify_a_parent_attribute() {
    // An attribute set in the parent document is locked inside an AsciiDoc cell:
    // a reassignment is silently ignored, so the inherited value is what the cell
    // sees. The lock is scoped to the cell, so the parent value is unchanged
    // afterward.
    let mut parser = Parser::default();
    let doc = parser.parse(
        ":locked: parent\n\n[cols=\"a\"]\n|===\n|\n:locked: child\n\nvalue is {locked}\n|===",
    );
    let table = doc
        .nested_blocks()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .unwrap();

    assert_eq!(asciidoc_cell_text(table, 0, 0), "value is parent");
    assert_eq!(
        parser.attribute_value("locked"),
        crate::document::InterpretedValue::Value("parent".to_string())
    );
}

#[test]
fn asciidoc_cell_may_set_an_attribute_unset_in_the_parent() {
    // Only attributes that are *set* in the parent are locked. An attribute that
    // is explicitly unset in the parent is not locked, so the cell may assign it
    // and the cell sees its own value (matching Asciidoctor, which here diverges
    // from the spec's "set or explicitly unset" wording).
    let mut parser = Parser::default();
    let doc = parser.parse(
        ":locked: value\n:locked!:\n\n[cols=\"a\"]\n|===\n|\n:locked: child\n\nvalue is {locked}\n|===",
    );
    let table = doc
        .nested_blocks()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .unwrap();

    assert_eq!(asciidoc_cell_text(table, 0, 0), "value is child");
}

#[test]
fn asciidoc_cell_may_modify_an_exempt_attribute() {
    // A handful of attributes are exempt from the cell lock; `compat-mode` is one
    // of them, so a cell may modify it even though the parent set it, while a
    // non-exempt attribute (`locked`) stays at the inherited value.
    let mut parser = Parser::default();
    let doc = parser.parse(
        ":compat-mode: parent\n:locked: parent\n\n[cols=\"a\"]\n|===\n|\n:compat-mode: child\n:locked: child\n\nc={compat-mode} l={locked}\n|===",
    );
    let table = doc
        .nested_blocks()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .unwrap();

    assert_eq!(asciidoc_cell_text(table, 0, 0), "c=child l=parent");
}

#[test]
fn cell_specifier_style_operator_locates_separator() {
    // A single lowercase letter directly in front of a `|` is a (style) cell
    // specifier, so the `|` is a cell separator: `a s|b` is two cells, not one.
    // A recognized style operator (here `s`) is applied to the cell.
    let table = parse_table("|===\n|a s|b\n|===");

    assert_eq!(table.columns().len(), 2);
    let rows: Vec<_> = table.body_rows().iter().map(row_text).collect();
    assert_eq!(rows, vec![vec!["a".to_string(), "b".to_string()]]);

    let cells = table.body_rows()[0].cells();
    assert_eq!(cells[0].style(), ColumnStyle::Default);
    assert_eq!(cells[1].style(), ColumnStyle::Strong);
}

#[test]
fn unrecognized_cell_style_operator_inherits_column_style() {
    // A single lowercase letter that isn't a recognized style operator (here `z`)
    // still locates the cell separator, but it leaves the cell's style unset, so
    // the cell inherits its column's style rather than reverting to the default.
    // This matches Asciidoctor, which ignores an unrecognized cell style operator.
    let table = parse_table("[cols=\"m,m\"]\n|===\n|head1 |head2\n\nz|zee s|ess\n|===");

    let cells = table.body_rows()[0].cells();
    assert_eq!(cells[0].style(), ColumnStyle::Monospace);
    assert_eq!(cells[1].style(), ColumnStyle::Strong);
}

#[test]
fn cell_specifier_span_operator_without_factor_locates_separator() {
    // The span (`+`) and duplication (`*`) operators may appear without a count.
    // A bare `+` directly in front of a `|` is still a valid cell specifier, so
    // `a +|b` is two cells. With no count the span factor defaults to 1, so the
    // bare `+` cell spans a single column (no layout effect).
    let table = parse_table("|===\n|a +|b\n|===");

    assert_eq!(table.columns().len(), 2);
    let rows: Vec<_> = table.body_rows().iter().map(row_text).collect();
    assert_eq!(rows, vec![vec!["a".to_string(), "b".to_string()]]);
}

#[test]
fn non_specifier_token_is_a_plain_cell_separator() {
    // A token in front of a `|` that does not parse as a cell specifier (here the
    // word `foo`, which is more than a single style letter) is ordinary content:
    // the `|` is still a plain cell separator, so `a foo|b` is two cells (matching
    // Asciidoctor).
    let table = parse_table("|===\n|a foo|b\n|===");

    let rows: Vec<_> = table.body_rows().iter().map(row_text).collect();
    assert_eq!(rows, vec![vec!["a foo".to_string(), "b".to_string()]]);
}

#[test]
fn dot_not_followed_by_vertical_operator_is_a_plain_cell_separator() {
    // A vertical alignment operator is a dot followed by `<`, `>`, or `^`. A dot
    // followed by anything else (here `.x`) is not a valid cell specifier, so the
    // `.x` is content and the `|` is a plain cell separator: `a .x|b` is two cells
    // (matching Asciidoctor).
    let table = parse_table("|===\n|a .x|b\n|===");

    let rows: Vec<_> = table.body_rows().iter().map(row_text).collect();
    assert_eq!(rows, vec![vec!["a .x".to_string(), "b".to_string()]]);
}

#[test]
fn cell_span_exceeding_columns_drops_overrunning_row() {
    // A span wider than the table overruns the grid. Matching Asciidoctor, the
    // whole overrunning row is dropped and a warning is emitted: here the leading
    // `x` cell is discarded along with the `3+|y` cell that overshoots the two
    // columns, and the following row (`z`, `w`) stays aligned to the grid.
    let mut parser = Parser::default();
    let maw = Block::parse(
        Span::new("[cols=\"2*\"]\n|===\n|x 3+|y\n|z |w\n|==="),
        &mut parser,
    );

    assert!(maw.warnings.iter().any(|w| matches!(
        w.warning,
        crate::warnings::WarningType::TableCellExceedsColumnCount
    )));

    let table = match maw.item.unwrap().item {
        Block::Table(table) => table,
        other => panic!("expected a table block, got {other:?}"),
    };
    let rows: Vec<_> = table.body_rows().iter().map(row_text).collect();
    assert_eq!(rows, vec![vec!["z".to_string(), "w".to_string()]]);
}

#[test]
fn row_fully_covered_by_rowspans_drops_following_cell() {
    // Two row-spanning cells (`.2+`) together cover every column of the next row,
    // so that row has no cells of its own to close it. The following cell (`c1`)
    // therefore overruns the pre-filled row and is dropped with it (a column-count
    // overrun warning), leaving `c2` and `d1` aligned as the second body row. The
    // trailing `d2` cell can't fill a row of its own, so the table ends on an
    // incomplete row that is dropped with an end-of-table warning (matching
    // Asciidoctor, which renders only two rows here).
    let mut parser = Parser::default();
    let maw = Block::parse(
        Span::new("[cols=\"2*\"]\n|===\n.2+|a .2+|b\n|c1 |c2\n|d1 |d2\n|==="),
        &mut parser,
    );

    assert_eq!(
        maw.warnings
            .iter()
            .filter(|w| matches!(
                w.warning,
                crate::warnings::WarningType::TableCellExceedsColumnCount
            ))
            .count(),
        1
    );
    assert_eq!(
        maw.warnings
            .iter()
            .filter(|w| matches!(
                w.warning,
                crate::warnings::WarningType::TableIncompleteRowAtEndOfTable
            ))
            .count(),
        1
    );

    let table = match maw.item.unwrap().item {
        Block::Table(table) => table,
        other => panic!("expected a table block, got {other:?}"),
    };
    let rows: Vec<_> = table.body_rows().iter().map(row_text).collect();
    assert_eq!(
        rows,
        vec![
            vec!["a".to_string(), "b".to_string()],
            vec!["c2".to_string(), "d1".to_string()],
        ]
    );
}

#[test]
fn dropped_overrunning_cell_keeps_its_rowspan_carryover() {
    // When a cell that carries a row span is itself part of an overrunning row,
    // its already-reserved slots in the next rows are *not* rolled back when the
    // row is dropped. Matching Asciidoctor, the `2.2+|b` cell (colspan 2, rowspan
    // 2) overruns the two-column row and is dropped with `a`, but its rowspan
    // still pre-fills the next row, so `c` overruns that pre-filled row and is
    // dropped too. Only `d` and `e` survive.
    let mut parser = Parser::default();
    let maw = Block::parse(
        Span::new("[cols=\"2*\"]\n|===\n|a 2.2+|b\n|c\n|d\n|e\n|==="),
        &mut parser,
    );

    assert_eq!(
        maw.warnings
            .iter()
            .filter(|w| matches!(
                w.warning,
                crate::warnings::WarningType::TableCellExceedsColumnCount
            ))
            .count(),
        2
    );

    let table = match maw.item.unwrap().item {
        Block::Table(table) => table,
        other => panic!("expected a table block, got {other:?}"),
    };
    let rows: Vec<_> = table.body_rows().iter().map(row_text).collect();
    assert_eq!(rows, vec![vec!["d".to_string(), "e".to_string()]]);
}

#[test]
fn huge_row_span_factor_does_not_overallocate() {
    // A row span factor far larger than the table can never carry into rows that
    // don't exist, so it is clamped for the grid bookkeeping; without the clamp,
    // `.1000000000+` would resize the `active_rowspans` vector to a billion
    // entries (a multi-gigabyte allocation). The cell still reports its literal
    // parsed `rowspan` (matching Asciidoctor), and the following cell, covered by
    // the span, overruns and is dropped.
    let mut parser = Parser::default();
    let maw = Block::parse(Span::new("|===\n.1000000000+|a\n|b\n|==="), &mut parser);

    assert!(maw.warnings.iter().any(|w| matches!(
        w.warning,
        crate::warnings::WarningType::TableCellExceedsColumnCount
    )));

    let table = match maw.item.unwrap().item {
        Block::Table(table) => table,
        other => panic!("expected a table block, got {other:?}"),
    };
    assert_eq!(table.body_rows().len(), 1);
    let cell = &table.body_rows()[0].cells()[0];
    assert_eq!(cell.colspan(), 1);
    assert_eq!(cell.rowspan(), 1_000_000_000);
    assert_eq!(row_text(&table.body_rows()[0]), vec!["a".to_string()]);
}

#[test]
fn duplication_clones_content_and_properties() {
    // A duplication factor (`<n>*`) clones the cell into `<n>` independent cells,
    // each an ordinary single-slot cell (colspan and rowspan of 1) that carries
    // the original's content and style. Here `3*e|x` becomes three emphasized
    // cells, each holding `x`.
    let table = parse_table("[cols=\"3*\"]\n|===\n3*e|x\n|===");
    let cells = table.body_rows()[0].cells();
    assert_eq!(cells.len(), 3);
    for cell in cells {
        assert_eq!(cell.colspan(), 1);
        assert_eq!(cell.rowspan(), 1);
        assert_eq!(cell.style(), ColumnStyle::Emphasis);
    }
    assert_eq!(
        row_text(&table.body_rows()[0]),
        vec!["x".to_string(), "x".to_string(), "x".to_string()]
    );
}

#[test]
fn duplication_factor_ignores_row_part() {
    // Only the column part of the factor is the duplication count; a row part
    // (`<n>.<n>*`) is ignored, matching Asciidoctor. So `2.3*` duplicates the cell
    // twice (not six times), and each clone keeps a rowspan of 1.
    let table = parse_table("[cols=\"4*\"]\n|===\n2.3*|a |b |c\n|===");
    let cells = table.body_rows()[0].cells();
    assert_eq!(cells.len(), 4);
    assert_eq!(cells[0].rowspan(), 1);
    assert_eq!(
        row_text(&table.body_rows()[0]),
        vec![
            "a".to_string(),
            "a".to_string(),
            "b".to_string(),
            "c".to_string()
        ]
    );
}

#[test]
fn duplication_factor_of_zero_drops_the_cell() {
    // A duplication factor of zero (`0*`) clones the cell zero times, dropping it
    // entirely; the following cells flow normally into the grid. Matching
    // Asciidoctor, `0*|x |a |b |c` yields a single row of `a`, `b`, `c`.
    let table = parse_table("[cols=\"3*\"]\n|===\n0*|x |a |b |c\n|===");
    let rows: Vec<_> = table.body_rows().iter().map(row_text).collect();
    assert_eq!(
        rows,
        vec![vec!["a".to_string(), "b".to_string(), "c".to_string()]]
    );
}

#[test]
fn bare_duplication_operator_clones_once() {
    // A bare duplication operator (`*`) with no factor defaults to a count of 1,
    // so `*|a` is a single cell and locates the separator just like a plain `|`.
    let table = parse_table("|===\n|a *|b\n|===");
    assert_eq!(table.columns().len(), 2);
    let rows: Vec<_> = table.body_rows().iter().map(row_text).collect();
    assert_eq!(rows, vec![vec!["a".to_string(), "b".to_string()]]);
}

#[test]
fn huge_duplication_factor_does_not_overallocate() {
    // A duplication factor is an amplifier — `1000000000*` would otherwise
    // materialize a billion cells (a multi-gigabyte allocation). The factor is
    // clamped to `MAX_DUPLICATION_FACTOR` (1,000), so the expansion stays
    // bounded. This is the one place the implementation diverges from Asciidoctor,
    // which would expand the literal factor. The table is built from the clamped
    // cells without hanging or exhausting memory.
    let table = parse_table("|===\n1000000000*|x\n|===");
    let total: usize = table.body_rows().iter().map(|r| r.cells().len()).sum();
    assert_eq!(total, 1_000);
    assert_eq!(row_text(&table.body_rows()[0])[0], "x".to_string());
}

// The `format` attribute selects the data format, and an unrecognized value
// falls back to PSV. (Exercises `resolve_data_format`, including its
// unrecognized-value arm.)
#[test]
fn data_format_attribute_recognizes_each_value() {
    assert_eq!(
        parse_table("|===\n|a |b\n|===").data_format(),
        DataFormat::Psv
    );
    assert_eq!(
        parse_table("[format=psv]\n|===\n|a |b\n|===").data_format(),
        DataFormat::Psv
    );
    assert_eq!(
        parse_table("[format=csv]\n|===\na,b\n|===").data_format(),
        DataFormat::Csv
    );
    assert_eq!(
        parse_table("[format=tsv]\n|===\na\tb\n|===").data_format(),
        DataFormat::Tsv
    );
    assert_eq!(
        parse_table("[format=dsv]\n|===\na:b\n|===").data_format(),
        DataFormat::Dsv
    );

    // An unrecognized format value falls back to PSV.
    assert_eq!(
        parse_table("[format=bogus]\n|===\n|a |b\n|===").data_format(),
        DataFormat::Psv
    );
}

// A CSV value whose opening double quote is never closed keeps the quote and is
// treated as literal text, matching Asciidoctor
// (`buffer_has_unclosed_quotes?`).
#[test]
fn csv_unclosed_quote_is_literal() {
    let table = parse_table("[format=csv]\n|===\nx,\"unterminated\n|===");
    assert_eq!(table.columns().len(), 2);
    assert_eq!(row_text(&table.body_rows()[0]), ["x", "\"unterminated"]);

    // A bare leading quote leaves the value unclosed, so the following separator
    // is absorbed into the (single) cell rather than ending it.
    let table = parse_table("[format=csv]\n|===\n\",a\n|===");
    assert_eq!(table.columns().len(), 1);
    assert_eq!(row_text(&table.body_rows()[0]), ["\",a"]);
}

#[test]
fn csv_lone_quote_is_unclosed_and_empty_with_warning() {
    // A cell that is a single bare quote is an unclosed, empty quoted value: it is
    // set to empty and an error is logged (matching Asciidoctor).
    let mut parser = Parser::default();
    let maw = Block::parse(Span::new("[format=csv]\n|===\n\"\n|==="), &mut parser);

    assert!(maw.warnings.iter().any(|w| matches!(
        w.warning,
        crate::warnings::WarningType::TableCsvDataHasUnclosedQuote
    )));

    let table = match maw.item.unwrap().item {
        Block::Table(table) => table,
        other => panic!("expected a table block, got {other:?}"),
    };
    assert_eq!(table.columns().len(), 1);
    assert_eq!(row_text(&table.body_rows()[0]), [""]);
}

// A CSV value with characters after its closing quote is not a properly
// enclosed value, so it (and any separators it spans) are taken literally as
// one cell.
#[test]
fn csv_text_after_closing_quote_is_literal() {
    let table = parse_table("[format=csv]\n|===\n\"abc\"x,d\n|===");
    assert_eq!(table.columns().len(), 1);
    assert_eq!(row_text(&table.body_rows()[0]), ["\"abc\"x,d"]);
}

// An escaped double quote (`""`) collapses to a single `"`, whether the value
// is enclosed in quotes or not.
#[test]
fn csv_escaped_quotes_collapse() {
    let table = parse_table("[format=csv]\n|===\n\"a\"\"b\",a\"\"b\n|===");
    assert_eq!(table.columns().len(), 2);
    assert_eq!(row_text(&table.body_rows()[0]), ["a\"b", "a\"b"]);

    // An odd run of trailing quotes still leaves the value unclosed, so the
    // separator is absorbed and the quotes are only collapsed.
    let table = parse_table("[format=csv]\n|===\n\"a\"\",b\n|===");
    assert_eq!(table.columns().len(), 1);
    assert_eq!(row_text(&table.body_rows()[0]), ["\"a\",b"]);
}

// A trailing separator produces an empty cell at the end of the row
// (Asciidoctor preserves it), so the row's column count includes that empty
// cell.
#[test]
fn csv_trailing_separator_keeps_empty_cell() {
    let table = parse_table("[format=csv]\n|===\na,b,\nc,d,e\n|===");
    assert_eq!(table.columns().len(), 3);
    assert_eq!(row_text(&table.body_rows()[0]), ["a", "b", ""]);
    assert_eq!(row_text(&table.body_rows()[1]), ["c", "d", "e"]);
}
