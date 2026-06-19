use crate::{
    HasSpan, Parser, Span,
    attributes::Attrlist,
    blocks::{ContentModel, IsBlock, metadata::BlockMetadata},
    content::{Content, SubstitutionGroup},
    parser::{InlineSubstitutionRenderer, ReferenceResolver, ReferenceWarning},
    span::MatchedItem,
    strings::CowStr,
    warnings::{MatchAndWarnings, Warning, WarningType},
};

/// A table is a delimited block that arranges content into a grid of rows and
/// columns.
///
/// A table is introduced by a table delimiter (`|===`) and closed by a matching
/// delimiter. Cells are separated using prefix-separated value (PSV) syntax: a
/// vertical bar (`|`) at the start of a line or preceded by whitespace begins a
/// new cell. Cells flow, in document order, into rows whose length is fixed by
/// the number of columns.
///
/// The number of columns is determined either by the `cols` attribute or,
/// implicitly, by the number of cells found in the first non-empty line after
/// the opening delimiter.
///
/// # Not yet supported
///
/// This is an initial implementation covering the basic PSV table. The
/// following table features are recognized by the wider AsciiDoc specification
/// but are not yet implemented:
///
/// * The CSV, TSV, and DSV data formats (and the `,===` / `:===` shorthand
///   delimiters).
/// * Column specifiers and multipliers beyond a proportional width (alignment
///   and style operators).
/// * Cell specifiers (spans, duplication, per-cell alignment and style, and the
///   `a` AsciiDoc style that nests block content inside a cell).
/// * Footer rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableBlock<'src> {
    columns: Vec<TableColumn>,
    header_row: Option<TableRow<'src>>,
    body_rows: Vec<TableRow<'src>>,
    footer_row: Option<TableRow<'src>>,
    source: Span<'src>,
    title_source: Option<Span<'src>>,
    title: Option<String>,
    anchor: Option<Span<'src>>,
    anchor_reftext: Option<Span<'src>>,
    attrlist: Option<Attrlist<'src>>,
}

impl<'src> TableBlock<'src> {
    /// Returns `true` if `line` is a table delimiter.
    ///
    /// A table delimiter is a vertical bar (`|`) followed by three or more
    /// equals signs (`===`).
    ///
    /// **NOTE:** The `,===` (CSV), `:===` (DSV), and `!===` (nested) shorthand
    /// delimiters are not yet recognized.
    pub(crate) fn is_table_delimiter(line: &Span<'src>) -> bool {
        let data = line.data();
        data.len() >= 4
            && data.starts_with('|')
            && data
                .get(1..)
                .is_some_and(|rest| rest.len() >= 3 && rest.bytes().all(|b| b == b'='))
    }

    pub(crate) fn parse(
        metadata: &BlockMetadata<'src>,
        parser: &mut Parser,
    ) -> Option<MatchAndWarnings<'src, Option<MatchedItem<'src, Self>>>> {
        let delimiter = metadata.block_start.take_normalized_line();

        if !Self::is_table_delimiter(&delimiter.item) {
            return None;
        }

        let delimiter_text = delimiter.item.data();

        // Find the matching closing delimiter.
        let mut next = delimiter.after;
        let (closing_delimiter, after) = loop {
            if next.is_empty() {
                break (next, next);
            }

            let line = next.take_normalized_line();
            if line.item.data() == delimiter_text {
                break (line.item, line.after);
            }
            next = line.after;
        };

        let inside = delimiter.after.trim_remainder(closing_delimiter);

        // Determine the number of columns, either from the `cols` attribute or
        // implicitly from the number of cells in the first non-empty line.
        let columns: Vec<TableColumn> = metadata
            .attrlist
            .as_ref()
            .and_then(|a| a.named_attribute("cols"))
            .map(|attr| parse_cols(attr.value()))
            .unwrap_or_default();

        let first_line_cells = scan_cells(inside.discard_empty_lines().take_line().item).len();

        let ncols = if columns.is_empty() {
            first_line_cells
        } else {
            columns.len()
        };

        let columns = if columns.is_empty() {
            (0..ncols).map(|_| TableColumn::default()).collect()
        } else {
            columns
        };

        // The first row is an (implicit) header row when the line directly after
        // the opening delimiter is non-empty and is itself followed by an empty
        // line. The `header` option forces the same interpretation.
        let opts_header = metadata
            .attrlist
            .as_ref()
            .is_some_and(|a| a.has_option("header"));

        // The blank line must genuinely exist after the first row; the end of the
        // table (an empty remainder) does not count, so a single-row table is not
        // mistaken for an all-header table.
        let line1 = inside.take_line();
        let line1_blank = line1.item.data().trim().is_empty();
        let line2_blank =
            !line1.after.is_empty() && line1.after.take_line().item.data().trim().is_empty();
        let has_header = opts_header || (!line1_blank && line2_blank);

        // Scan every cell in the table, in document order, then partition into
        // rows of `ncols` cells each.
        let mut cells = scan_cells(inside)
            .into_iter()
            .map(|raw| TableCell::parse(raw, parser));

        let header_row = if has_header && ncols > 0 {
            let row: Vec<TableCell<'src>> = cells.by_ref().take(ncols).collect();
            if row.is_empty() {
                None
            } else {
                Some(TableRow { cells: row })
            }
        } else {
            None
        };

        let mut body_rows: Vec<TableRow<'src>> = vec![];
        if ncols > 0 {
            loop {
                let row: Vec<TableCell<'src>> = cells.by_ref().take(ncols).collect();
                if row.is_empty() {
                    break;
                }
                body_rows.push(TableRow { cells: row });
            }
        }

        let source = metadata
            .source
            .trim_remainder(closing_delimiter.discard_all())
            .trim_trailing_whitespace();

        let warnings = if closing_delimiter.is_empty() {
            vec![Warning {
                source: delimiter.item,
                warning: WarningType::UnterminatedDelimitedBlock,
            }]
        } else {
            vec![]
        };

        Some(MatchAndWarnings {
            item: Some(MatchedItem {
                item: Self {
                    columns,
                    header_row,
                    body_rows,
                    footer_row: None,
                    source,
                    title_source: metadata.title_source,
                    title: metadata.title.clone(),
                    anchor: metadata.anchor,
                    anchor_reftext: metadata.anchor_reftext,
                    attrlist: metadata.attrlist.clone(),
                },
                after,
            }),
            warnings,
        })
    }

    /// Returns the columns of this table.
    pub fn columns(&self) -> &[TableColumn] {
        &self.columns
    }

    /// Returns the header row of this table, if one was declared.
    pub fn header_row(&self) -> Option<&TableRow<'src>> {
        self.header_row.as_ref()
    }

    /// Returns the body rows of this table.
    pub fn body_rows(&self) -> &[TableRow<'src>] {
        &self.body_rows
    }

    /// Returns the footer row of this table, if one was declared.
    pub fn footer_row(&self) -> Option<&TableRow<'src>> {
        self.footer_row.as_ref()
    }

    /// Resolves any deferred cross-references in this table's cells.
    pub(crate) fn resolve_references(
        &mut self,
        resolver: &dyn ReferenceResolver,
        renderer: &dyn InlineSubstitutionRenderer,
        warnings: &mut Vec<ReferenceWarning>,
    ) {
        let rows = self
            .header_row
            .iter_mut()
            .chain(self.body_rows.iter_mut())
            .chain(self.footer_row.iter_mut());

        for row in rows {
            for cell in row.cells.iter_mut() {
                cell.content
                    .resolve_references(resolver, renderer, warnings);
            }
        }
    }
}

impl<'src> IsBlock<'src> for TableBlock<'src> {
    fn content_model(&self) -> ContentModel {
        ContentModel::Table
    }

    fn raw_context(&self) -> CowStr<'src> {
        "table".into()
    }

    fn title_source(&'src self) -> Option<Span<'src>> {
        self.title_source
    }

    fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    fn anchor(&'src self) -> Option<Span<'src>> {
        self.anchor
    }

    fn anchor_reftext(&'src self) -> Option<Span<'src>> {
        self.anchor_reftext
    }

    fn attrlist(&'src self) -> Option<&'src Attrlist<'src>> {
        self.attrlist.as_ref()
    }
}

impl<'src> HasSpan<'src> for TableBlock<'src> {
    fn span(&self) -> Span<'src> {
        self.source
    }
}

/// A column in a [`TableBlock`].
///
/// For now a column carries only its proportional width. Alignment and style
/// operators on the `cols` specifier are not yet modeled.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableColumn {
    width: usize,
}

impl TableColumn {
    /// Returns the proportional width of this column relative to the other
    /// columns in the table.
    pub fn width(&self) -> usize {
        self.width
    }
}

impl Default for TableColumn {
    fn default() -> Self {
        Self { width: 1 }
    }
}

/// A row of cells in a [`TableBlock`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableRow<'src> {
    cells: Vec<TableCell<'src>>,
}

impl<'src> TableRow<'src> {
    /// Returns the cells in this row.
    pub fn cells(&self) -> &[TableCell<'src>] {
        &self.cells
    }
}

/// A single cell in a [`TableBlock`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableCell<'src> {
    content: Content<'src>,
}

impl<'src> TableCell<'src> {
    /// Build a cell from the raw (untrimmed) span of its content.
    ///
    /// Leading and trailing whitespace is stripped, escaped cell separators
    /// (`\|`) are unescaped, and normal inline substitutions are applied.
    fn parse(raw: Span<'src>, parser: &Parser) -> Self {
        let trimmed = trim_surrounding_whitespace(raw);
        let data = trimmed.data();

        let mut content = if data.contains("\\|") {
            Content::from_filtered(trimmed, data.replace("\\|", "|"))
        } else {
            Content::from(trimmed)
        };

        SubstitutionGroup::Normal.apply(&mut content, parser, None);

        Self { content }
    }

    /// Returns the interpreted content of this cell.
    pub fn content(&self) -> &Content<'src> {
        &self.content
    }
}

/// Parse the value of the `cols` attribute into a list of columns.
///
/// The value is a comma-separated list of column specifiers. A specifier may be
/// preceded by a multiplier (`<n>*`) that repeats the column `n` times. Only
/// the proportional width portion of a specifier is currently interpreted.
fn parse_cols(value: &str) -> Vec<TableColumn> {
    let mut columns: Vec<TableColumn> = vec![];

    for part in value.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if let Some((count, spec)) = part.split_once('*') {
            let repeat = count.trim().parse::<usize>().unwrap_or(1).max(1);
            let column = parse_col_spec(spec);
            for _ in 0..repeat {
                columns.push(column.clone());
            }
        } else {
            columns.push(parse_col_spec(part));
        }
    }

    columns
}

/// Parse a single column specifier, extracting its proportional width.
///
/// Alignment and style operators are not yet interpreted; any non-digit
/// characters are ignored and the column falls back to the default width.
fn parse_col_spec(spec: &str) -> TableColumn {
    let digits: String = spec.chars().filter(|c| c.is_ascii_digit()).collect();
    match digits.parse::<usize>() {
        Ok(width) if width > 0 => TableColumn { width },
        _ => TableColumn::default(),
    }
}

/// Scan a region for PSV cell boundaries, returning the raw (untrimmed) span of
/// each cell's content.
///
/// A cell boundary is a vertical bar (`|`) that appears at the start of a line
/// or is preceded by whitespace, and that is not escaped with a leading
/// backslash. Content before the first boundary is ignored.
fn scan_cells(region: Span<'_>) -> Vec<Span<'_>> {
    let bytes = region.data().as_bytes();
    let len = bytes.len();

    let mut cells: Vec<Span<'_>> = vec![];
    let mut content_start: Option<usize> = None;
    let mut i = 0;

    while i < len {
        if bytes.get(i) == Some(&b'|') {
            let prev = i.checked_sub(1).and_then(|p| bytes.get(p)).copied();
            let at_line_start = prev.is_none() || prev == Some(b'\n');
            let after_space = prev == Some(b' ') || prev == Some(b'\t');
            let escaped = prev == Some(b'\\');

            if (at_line_start || after_space) && !escaped {
                if let Some(start) = content_start {
                    cells.push(region.slice(start..i));
                }
                content_start = Some(i + 1);
            }
        }

        i += 1;
    }

    if let Some(start) = content_start {
        cells.push(region.slice(start..len));
    }

    cells
}

/// Return the subspan of `s` with surrounding whitespace (including newlines)
/// removed.
fn trim_surrounding_whitespace(s: Span<'_>) -> Span<'_> {
    let data = s.data();
    let start = data.len() - data.trim_start().len();
    let len = data.trim().len();
    s.slice(start..start + len)
}
