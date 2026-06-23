use crate::{
    HasSpan, Parser, Span,
    attributes::Attrlist,
    blocks::{
        Block, ContentModel, IsBlock, metadata::BlockMetadata, parse_utils::parse_blocks_until,
    },
    content::{Content, SubstitutionGroup},
    document::InterpretedValue,
    parser::{InlineSubstitutionRenderer, ReferenceResolver, ReferenceWarning},
    span::MatchedItem,
    strings::CowStr,
    warnings::{MatchAndWarnings, Warning, WarningType},
};

/// Attributes that an AsciiDoc table cell may modify even when they are set in
/// the parent document.
///
/// An AsciiDoc cell inherits the parent's attributes and cannot modify them,
/// but the AsciiDoc specification carves out a handful of exceptions:
/// `doctype`, `toc`, `notitle` (and its complement, `showtitle`), and
/// `compat-mode`.
const ASCIIDOC_CELL_MODIFIABLE_ATTRIBUTES: &[&str] =
    &["doctype", "toc", "notitle", "showtitle", "compat-mode"];

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
///
/// Column specifier style operators (the `a`, `d`, `e`, `h`, `l`, `m`, and `s`
/// operators) are supported, along with proportional width and the horizontal
/// and vertical alignment operators. Per-cell horizontal and vertical alignment
/// operators are supported and override the column's alignment, and a per-cell
/// style operator (in the last position of the cell specifier) is supported and
/// overrides the column's style. The per-cell span (`+`) operator is supported:
/// a cell can span multiple columns (`<n>+`), multiple rows (`.<n>+`), or a
/// block of both (`<n>.<n>+`). The per-cell duplication (`*`) operator is
/// supported: a cell with a duplication factor (`<n>*`) clones its content and
/// properties into `<n>` consecutive cells.
///
/// Table sizing is supported: the [`width`](Self::width) attribute sets a fixed
/// table width, the `autowidth` option ([`is_autowidth`](Self::is_autowidth))
/// sizes the table and its columns to their content, and an individual column
/// can be made [autowidth](TableColumn::is_autowidth) with the `~` width value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableBlock<'src> {
    columns: Vec<TableColumn>,
    header_row: Option<TableRow<'src>>,
    body_rows: Vec<TableRow<'src>>,
    footer_row: Option<TableRow<'src>>,
    source: Span<'src>,
    title_source: Option<Span<'src>>,
    title: Option<String>,
    caption: Option<String>,
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
        // `len() >= 4` plus the leading `|` guarantees `rest` holds at least three
        // bytes, so the closure only needs to confirm they are all `=`.
        data.len() >= 4
            && data.starts_with('|')
            && data
                .get(1..)
                .is_some_and(|rest| rest.bytes().all(|b| b == b'='))
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

        // When the column count is implicit, it is the number of column slots in
        // the first non-empty line: a cell that spans columns (`<n>+`) counts as
        // `<n>` slots, not one, and a cell duplicated `<n>` times (`<n>*`) counts
        // as `<n>` single-column slots (one per clone).
        let first_line_cells: usize = scan_cells(inside.discard_empty_lines().take_line().item)
            .iter()
            .map(|c| c.spec.colspan.max(1) * c.spec.repeat.min(MAX_DUPLICATION_FACTOR))
            .sum();

        let ncols = if columns.is_empty() {
            first_line_cells
        } else {
            columns.len()
        };

        let mut columns: Vec<TableColumn> = if columns.is_empty() {
            (0..ncols).map(|_| TableColumn::default()).collect()
        } else {
            columns
        };

        // The `autowidth` option sizes the table to its content; the columns
        // inherit the setting, so every column becomes autowidth regardless of
        // any proportional width set on its specifier.
        if metadata
            .attrlist
            .as_ref()
            .is_some_and(|a| a.has_option("autowidth"))
        {
            for column in columns.iter_mut() {
                column.autowidth = true;
            }
        }

        // The first row is an (implicit) header row when the line directly after
        // the opening delimiter is non-empty and is itself followed by an empty
        // line. The `header` option forces the same interpretation; the
        // `noheader` option suppresses only the implicit detection, so an
        // explicit `header` still wins when both are present.
        let opts_header = metadata
            .attrlist
            .as_ref()
            .is_some_and(|a| a.has_option("header"));
        let opts_noheader = metadata
            .attrlist
            .as_ref()
            .is_some_and(|a| a.has_option("noheader"));

        // The last row is promoted to a footer row when the `footer` option is
        // set. Unlike the header row, a footer cell is processed with its
        // column's style (it is simply the last body row, relabeled).
        let opts_footer = metadata
            .attrlist
            .as_ref()
            .is_some_and(|a| a.has_option("footer"));

        // The blank line must genuinely exist after the first row; the end of the
        // table (an empty remainder) does not count, so a single-row table is not
        // mistaken for an all-header table.
        let line1 = inside.take_line();
        let line1_blank = line1.item.data().trim().is_empty();
        let line2_blank =
            !line1.after.is_empty() && line1.after.take_line().item.data().trim().is_empty();
        let has_header = opts_header || (!opts_noheader && !line1_blank && line2_blank);

        // A titled table is given a caption (e.g. "Table 1. ") that a processor
        // prepends to the title.
        //
        // An explicit `caption` attribute on the table sets the label verbatim
        // (including any trailing whitespace) and is used as-is, with no
        // automatically incremented number; it applies even when
        // `table-caption` has been unset. An explicitly empty `caption` (e.g.
        // `[caption=]`) removes the label entirely, so the title renders with no
        // prefix. Otherwise the label comes from the `table-caption` attribute
        // (which defaults to "Table"), and each such captioned table consumes
        // the next value of a document-wide table counter. When `table-caption`
        // is unset and no explicit `caption` is given, no caption (and no
        // number) is assigned.
        //
        // Computed before the cell iterator below borrows `parser` immutably, so
        // that the mutable counter update does not conflict with that borrow.
        let caption = if metadata.title.is_some() {
            match metadata
                .attrlist
                .as_ref()
                .and_then(|a| a.named_attribute("caption"))
            {
                Some(attr) if attr.value().is_empty() => None,
                Some(attr) => Some(attr.value().to_string()),
                None => match parser.attribute_value("table-caption") {
                    InterpretedValue::Value(label) if !label.is_empty() => {
                        let number = parser.assign_table_number();
                        Some(format!("{label} {number}. "))
                    }
                    _ => None,
                },
            }
        } else {
            None
        };

        // Scan every cell in the table, in document order, then partition into
        // rows by walking the grid: a cell's span (colspan/rowspan) governs how
        // many column slots it occupies, so a column-spanning cell fills its row
        // with fewer cells and a row-spanning cell carries its columns down into
        // the rows below.
        //
        // This mirrors Asciidoctor's grid walk. `active_rowspans[k]` records the
        // number of column slots that cells from earlier rows occupy in the row
        // `k` steps ahead of the one being filled; a row closes once its own
        // cells' colspans plus the slots carried into it (`active_rowspans[0]`)
        // reach `ncols`. A cell whose span pushes the row *past* `ncols` overruns
        // the grid: the whole overrunning row is dropped (with a warning), again
        // matching Asciidoctor. A row whose columns are entirely pre-filled by
        // carried slots has no cells of its own to close it, so the next cell
        // overruns and is dropped together with that pre-filled row.
        // A duplicated cell (`<n>*`) is expanded into `<n>` independent cells —
        // each carrying the original's content, alignment, and style — before the
        // grid walk, so each clone occupies its own column slot exactly like an
        // ordinary cell. A duplication factor of zero drops the cell entirely.
        let mut warnings: Vec<Warning<'src>> = vec![];
        let raw_cells = expand_duplicates(scan_cells(inside));

        // A table can never have more rows than it has cells, so a row span is
        // clamped to the cell count for the `active_rowspans` bookkeeping below: a
        // larger span carries into rows that can't exist and so has no additional
        // layout effect. The clamp also bounds the `active_rowspans` allocation,
        // so a hostile specifier such as `.1000000000+` can't trigger a
        // multi-gigabyte allocation. (The cell's reported [`rowspan`] keeps the
        // literal parsed value, matching Asciidoctor.)
        //
        // [`rowspan`]: TableCell::rowspan
        let max_rowspan = raw_cells.len().saturating_add(1);

        let mut raw_rows: Vec<Vec<RawCell<'src>>> = vec![];
        if ncols > 0 {
            let mut active_rowspans: Vec<usize> = vec![0];
            let mut column_visits = 0usize;
            let mut current_row: Vec<RawCell<'src>> = vec![];

            for raw in raw_cells {
                let colspan = raw.spec.colspan.max(1);
                let rowspan = raw.spec.rowspan.max(1).min(max_rowspan);

                // A cell that spans more than one row reserves `colspan` slots in
                // each of the rows it extends into (but not its own row).
                if rowspan > 1 {
                    if active_rowspans.len() < rowspan {
                        active_rowspans.resize(rowspan, 0);
                    }
                    for slot in active_rowspans.iter_mut().take(rowspan).skip(1) {
                        *slot += colspan;
                    }
                }

                column_visits += colspan;
                let cell_source = raw.content;
                current_row.push(raw);

                // The slots carried into the current row are `active_rowspans[0]`;
                // the vector is never empty here, so the fallback is unreachable.
                let carried = active_rowspans.first().copied().unwrap_or(0);
                let effective = column_visits + carried;
                if effective >= ncols {
                    if effective == ncols {
                        raw_rows.push(std::mem::take(&mut current_row));
                    } else {
                        // Overrun: this cell's span pushes the row past `ncols`.
                        // Discard the whole row so the remaining cells stay
                        // aligned to the grid.
                        current_row.clear();
                        warnings.push(Warning {
                            source: cell_source,
                            warning: WarningType::TableCellExceedsColumnCount,
                        });
                    }
                    column_visits = 0;
                    active_rowspans.remove(0);
                    if active_rowspans.is_empty() {
                        active_rowspans.push(0);
                    }
                }
            }

            // A trailing incomplete row (one that never reached `ncols`) is still
            // emitted, matching the existing handling of short final rows.
            if !current_row.is_empty() {
                raw_rows.push(current_row);
            }
        }

        // Each cell is processed according to the style of the column it falls
        // in. A cell's column is its ordinal position within its row (matching
        // Asciidoctor, which assigns the column by cell count, not grid slot). The
        // header row (when present) is the first row and is always processed as
        // plain header content, regardless of the column styles, so that a style
        // operator doesn't affect the header row.
        let mut rows: Vec<TableRow<'src>> = Vec::with_capacity(raw_rows.len());
        for (row_idx, raw_row) in raw_rows.into_iter().enumerate() {
            let is_header = has_header && row_idx == 0;
            let mut cells = Vec::with_capacity(raw_row.len());
            for (col_idx, raw) in raw_row.into_iter().enumerate() {
                let column = columns.get(col_idx).cloned().unwrap_or_default();
                cells.push(TableCell::parse(
                    raw,
                    &column,
                    is_header,
                    parser,
                    &mut warnings,
                ));
            }
            rows.push(TableRow { cells });
        }

        let mut rows = rows.into_iter();
        let header_row = if has_header { rows.next() } else { None };
        let mut body_rows: Vec<TableRow<'src>> = rows.collect();

        // The footer row, when requested, is the last row of the table. It is
        // moved out of the body so the caller sees it as a distinct footer. When
        // the table has no rows to spare, no footer is produced.
        let footer_row = if opts_footer { body_rows.pop() } else { None };

        let source = metadata
            .source
            .trim_remainder(closing_delimiter.discard_all())
            .trim_trailing_whitespace();

        if closing_delimiter.is_empty() {
            warnings.push(Warning {
                source: delimiter.item,
                warning: WarningType::UnterminatedDelimitedBlock,
            });
        }

        Some(MatchAndWarnings {
            item: Some(MatchedItem {
                item: Self {
                    columns,
                    header_row,
                    body_rows,
                    footer_row,
                    source,
                    title_source: metadata.title_source,
                    title: metadata.title.clone(),
                    caption,
                    anchor: metadata.anchor,
                    anchor_reftext: metadata.anchor_reftext,
                    attrlist: metadata.attrlist.clone(),
                },
                after,
            }),
            warnings,
        })
    }

    /// Returns the caption assigned to this table, if any.
    ///
    /// A titled table is captioned with a label that a processor prepends to
    /// the [`title`](IsBlock::title). By default the label combines the
    /// `table-caption` attribute and an automatically incremented number (e.g.
    /// `"Table 1. "`). An explicit `caption` attribute on the table overrides
    /// this with a verbatim label and no number; an explicitly empty `caption`
    /// (e.g. `[caption=]`) removes the label entirely. The caption is absent
    /// when the table has no title, when `table-caption` has been unset and no
    /// explicit `caption` is given, or when an empty `caption` was supplied.
    pub fn caption(&self) -> Option<&str> {
        self.caption.as_deref()
    }

    /// Returns the columns of this table.
    pub fn columns(&self) -> &[TableColumn] {
        &self.columns
    }

    /// Returns the fixed width of this table, as a percentage of the content
    /// area, when the `width` attribute is set.
    ///
    /// The `width` attribute is an integer percentage from 1 to 100; the
    /// trailing `%` sign is optional (`[width=75%]` and `[width=75]` are
    /// equivalent). A value outside that range, or one that is not an integer,
    /// is ignored and reported as `None`. When the attribute is absent the
    /// table spans the width of the content area and this returns `None`.
    pub fn width(&self) -> Option<usize> {
        let raw = self
            .attrlist
            .as_ref()
            .and_then(|a| a.named_attribute("width"))?
            .value();

        let raw = raw.strip_suffix('%').unwrap_or(raw);
        match raw.parse::<usize>() {
            Ok(width) if (1..=100).contains(&width) => Some(width),
            _ => None,
        }
    }

    /// Returns `true` if this table carries the `autowidth` option.
    ///
    /// An autowidth table is sized to fit its content rather than spanning the
    /// width of the content area, and each of its [columns](TableColumn) is
    /// likewise [autowidth](TableColumn::is_autowidth).
    pub fn is_autowidth(&self) -> bool {
        self.attrlist
            .as_ref()
            .is_some_and(|a| a.has_option("autowidth"))
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
                cell.resolve_references(resolver, renderer, warnings);
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
/// A column carries its proportional width, the horizontal and vertical
/// alignment applied to its cells' content, and the [style](ColumnStyle) used
/// to process and render that content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableColumn {
    width: usize,
    autowidth: bool,
    h_align: HorizontalAlignment,
    v_align: VerticalAlignment,
    style: ColumnStyle,
}

impl TableColumn {
    /// Returns the width of this column relative to the other columns in the
    /// table. The default width is `1`.
    ///
    /// This value carries two different meanings depending on the table, and a
    /// caller that resolves columns to final sizes must check which applies:
    ///
    /// * In an ordinary table (no column is [autowidth](Self::is_autowidth)),
    ///   the width is a *proportional* ratio. Each column's share of the table
    ///   is its width divided by the sum of all the column widths, so
    ///   `[cols="1,2,3"]` yields shares of 1/6, 2/6, and 3/6.
    /// * When at least one column in the table is autowidth (its specifier uses
    ///   the special width value `~`), the AsciiDoc specification instead reads
    ///   these widths as literal *percentages* (100-based): in
    ///   `[cols="25,~,~"]` the first column is 25% wide and the `~` columns are
    ///   sized to their content.
    ///
    /// The two cases are distinguished by whether any column in the table is
    /// autowidth, which the caller can test with
    /// `table.columns().iter().any(TableColumn::is_autowidth)`.
    ///
    /// When this column itself is autowidth, this width is not used to size the
    /// column (the column is sized to its content instead) and reports the
    /// default value of `1`.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Returns `true` if this column is sized to fit its content rather than to
    /// a proportional width.
    ///
    /// A column is autowidth when its column specifier uses the special width
    /// value `~`, or when the table as a whole carries the `autowidth` option
    /// (in which case every column inherits the setting).
    pub fn is_autowidth(&self) -> bool {
        self.autowidth
    }

    /// Returns the horizontal alignment applied to this column's content.
    ///
    /// The alignment comes from a horizontal alignment operator (`<`, `>`, or
    /// `^`) on the column's specifier and defaults to
    /// [`HorizontalAlignment::Left`].
    pub fn h_align(&self) -> HorizontalAlignment {
        self.h_align
    }

    /// Returns the vertical alignment applied to this column's content.
    ///
    /// The alignment comes from a vertical alignment operator (`.<`, `.>`, or
    /// `.^`) on the column's specifier and defaults to
    /// [`VerticalAlignment::Top`].
    pub fn v_align(&self) -> VerticalAlignment {
        self.v_align
    }

    /// Returns the [style](ColumnStyle) applied to this column's content.
    ///
    /// The style comes from a style operator in the last position of the
    /// column's specifier (`a`, `d`, `e`, `h`, `l`, `m`, or `s`) and defaults
    /// to [`ColumnStyle::Default`].
    pub fn style(&self) -> ColumnStyle {
        self.style
    }
}

impl Default for TableColumn {
    fn default() -> Self {
        Self {
            width: 1,
            autowidth: false,
            h_align: HorizontalAlignment::Left,
            v_align: VerticalAlignment::Top,
            style: ColumnStyle::Default,
        }
    }
}

/// The style applied to the content of a [column](TableColumn) (and, by
/// extension, to each body cell in that column).
///
/// A style is specified by a style operator in the last position of a column
/// specifier. When no style operator is present, [`Default`](Self::Default) is
/// assigned and the column is processed as paragraph text.
///
/// The style governs both how a cell's content is parsed and how it is
/// rendered: most styles leave the content as inline markup (changing only the
/// surrounding formatting), [`Literal`](Self::Literal) processes the content
/// verbatim, and [`AsciiDoc`](Self::AsciiDoc) parses the content as a nested,
/// standalone AsciiDoc document.
///
/// The verse operator (`v`) recognized by older versions of AsciiDoc has been
/// deprecated and is not modeled here.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ColumnStyle {
    /// Block elements (lists, delimited blocks, and block macros) are
    /// supported; the content is parsed as a nested, standalone AsciiDoc
    /// document (the `a` operator).
    AsciiDoc,

    /// All of the markup permitted in a paragraph (inline formatting and inline
    /// macros) is supported (the `d` operator). This is the default style,
    /// assigned automatically when no style operator is present.
    #[default]
    Default,

    /// Text is italicized (the `e` operator).
    Emphasis,

    /// The header semantics and styles are applied to the text and cell borders
    /// (the `h` operator).
    Header,

    /// Content is treated as if it were inside a literal block (the `l`
    /// operator).
    Literal,

    /// Text is rendered using a monospace font (the `m` operator).
    Monospace,

    /// Text is bold (the `s` operator).
    Strong,
}

/// The horizontal alignment of a column's content.
///
/// Specified by a horizontal alignment operator at the start of a
/// [column specifier](TableColumn): the less-than sign (`<`) for
/// [`Left`](Self::Left), the greater-than sign (`>`) for
/// [`Right`](Self::Right), and the caret (`^`) for [`Center`](Self::Center).
/// The default is [`Left`](Self::Left).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HorizontalAlignment {
    /// Content is aligned to the left side of the column (the `<` operator).
    /// This is the default horizontal alignment.
    Left,

    /// Content is centered horizontally in the column (the `^` operator).
    Center,

    /// Content is aligned to the right side of the column (the `>` operator).
    Right,
}

/// The vertical alignment of a column's content.
///
/// Specified by a vertical alignment operator on a
/// [column specifier](TableColumn), always introduced by a dot (`.`): `.<` for
/// [`Top`](Self::Top), `.>` for [`Bottom`](Self::Bottom), and `.^` for
/// [`Middle`](Self::Middle). The default is [`Top`](Self::Top).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerticalAlignment {
    /// Content is aligned to the top of the column's cells (the `.<` operator).
    /// This is the default vertical alignment.
    Top,

    /// Content is centered vertically in the column's cells (the `.^`
    /// operator).
    Middle,

    /// Content is aligned to the bottom of the column's cells (the `.>`
    /// operator).
    Bottom,
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
    h_align: HorizontalAlignment,
    v_align: VerticalAlignment,
    style: ColumnStyle,
    colspan: usize,
    rowspan: usize,
    content: TableCellContent<'src>,
}

impl<'src> TableCell<'src> {
    /// Build a cell from the raw (untrimmed) span of its content, processing it
    /// according to the [style](ColumnStyle) of the `column` the cell belongs
    /// to.
    ///
    /// The cell's horizontal and vertical alignment come from the alignment
    /// operators on its [specifier](RawCell::spec) when present; otherwise they
    /// are inherited from the column. Likewise, a style operator on the cell's
    /// specifier overrides the column's [style](ColumnStyle); with no cell
    /// style operator, the cell is processed with the column's style. A
    /// header cell (`is_header`) is always processed as plain header
    /// content, regardless of any style operator on the column or the cell.
    ///
    /// Leading and trailing whitespace is always stripped. For every style but
    /// [`AsciiDoc`](ColumnStyle::AsciiDoc) the cell holds inline
    /// [`Content`](TableCellContent::Simple): escaped cell separators (`\|`)
    /// are unescaped and substitutions are applied — the verbatim group for
    /// [`Literal`](ColumnStyle::Literal), the normal group otherwise. An
    /// [`AsciiDoc`](ColumnStyle::AsciiDoc) cell instead parses its content as a
    /// nested sequence of [blocks](TableCellContent::AsciiDoc).
    fn parse(
        raw: RawCell<'src>,
        column: &TableColumn,
        is_header: bool,
        parser: &mut Parser,
        warnings: &mut Vec<Warning<'src>>,
    ) -> Self {
        // A cell's own alignment operator overrides the column's alignment; with
        // no operator, the cell inherits the column's alignment.
        let h_align = raw.spec.h_align.unwrap_or(column.h_align);
        let v_align = raw.spec.v_align.unwrap_or(column.v_align);

        // A cell's own style operator overrides the column's style; with no
        // operator, the cell is processed with the column's style. The header
        // row is always processed as plain header content, so neither a column
        // nor a cell style operator ever affects a header cell.
        let style = if is_header {
            ColumnStyle::Default
        } else {
            raw.spec.style.unwrap_or(column.style)
        };

        let trimmed = trim_surrounding_whitespace(raw.content);

        let content = if style == ColumnStyle::AsciiDoc {
            // The AsciiDoc style effectively creates a nested, standalone
            // AsciiDoc document in the cell. It inherits the parent document's
            // attributes, but any attribute it defines is scoped to the cell and
            // must not leak back into the parent. Snapshot the attribute set
            // before parsing and restore it afterward to enforce that boundary
            // (matching Asciidoctor, where a `:foo:` set inside a cell is not
            // visible after the table).
            let saved_attributes = parser.attribute_values.clone();

            // An attribute that is set in the parent document cannot be modified
            // inside the cell. Lock every inherited attribute that currently
            // holds a value for the duration of the cell (other than the handful
            // of exceptions the spec carves out), so a body assignment to one of
            // them is ignored. An attribute that is unset in the parent is not
            // locked: the cell may assign it (matching Asciidoctor, which here
            // diverges from the spec's "set or explicitly unset" wording). The
            // lock set is saved and restored so it applies only within the cell
            // and nests correctly.
            let saved_locks = parser.locked_attribute_names.clone();
            for (name, value) in saved_attributes.iter() {
                if !matches!(value.value, InterpretedValue::Unset)
                    && !ASCIIDOC_CELL_MODIFIABLE_ATTRIBUTES.contains(&name.as_str())
                {
                    parser.locked_attribute_names.insert(name.clone());
                }
            }

            let mut maw = parse_blocks_until(trimmed, |_| false, parser);

            parser.locked_attribute_names = saved_locks;
            parser.attribute_values = saved_attributes;
            warnings.append(&mut maw.warnings);
            TableCellContent::AsciiDoc(maw.item.item)
        } else {
            let data = trimmed.data();

            let mut content = if data.contains("\\|") {
                Content::from_filtered(trimmed, data.replace("\\|", "|"))
            } else {
                Content::from(trimmed)
            };

            let substitutions = if style == ColumnStyle::Literal {
                SubstitutionGroup::Verbatim
            } else {
                SubstitutionGroup::Normal
            };
            substitutions.apply(&mut content, parser, None);

            TableCellContent::Simple(content)
        };

        Self {
            h_align,
            v_align,
            style,
            colspan: raw.spec.colspan.max(1),
            rowspan: raw.spec.rowspan.max(1),
            content,
        }
    }

    /// Returns the horizontal alignment of this cell's content.
    ///
    /// The alignment comes from a horizontal alignment operator (`<`, `>`, or
    /// `^`) on the cell's specifier, which overrides the column's alignment. A
    /// cell with no horizontal alignment operator inherits its column's
    /// [`h_align`](TableColumn::h_align).
    pub fn h_align(&self) -> HorizontalAlignment {
        self.h_align
    }

    /// Returns the vertical alignment of this cell's content.
    ///
    /// The alignment comes from a vertical alignment operator (`.<`, `.>`, or
    /// `.^`) on the cell's specifier, which overrides the column's alignment. A
    /// cell with no vertical alignment operator inherits its column's
    /// [`v_align`](TableColumn::v_align).
    pub fn v_align(&self) -> VerticalAlignment {
        self.v_align
    }

    /// Returns the [style](ColumnStyle) applied to this cell's content.
    ///
    /// The style comes from a style operator in the last position of the cell's
    /// specifier (`a`, `d`, `e`, `h`, `l`, `m`, or `s`), which overrides the
    /// column's style. A cell with no style operator inherits its column's
    /// [`style`](TableColumn::style). A header cell is always
    /// [`Default`](ColumnStyle::Default), because the header row ignores style
    /// operators on both column and cell specifiers.
    pub fn style(&self) -> ColumnStyle {
        self.style
    }

    /// Returns the number of columns this cell spans.
    ///
    /// The span comes from a column span factor (`<n>`) or block span factor
    /// (`<n>.<n>`) in front of the span operator (`+`) on the cell's specifier.
    /// A cell with no column span factor spans a single column, so the default
    /// is `1`.
    pub fn colspan(&self) -> usize {
        self.colspan
    }

    /// Returns the number of rows this cell spans.
    ///
    /// The span comes from a row span factor (`.<n>`) or block span factor
    /// (`<n>.<n>`) in front of the span operator (`+`) on the cell's specifier.
    /// A cell with no row span factor spans a single row, so the default is
    /// `1`.
    pub fn rowspan(&self) -> usize {
        self.rowspan
    }

    /// Returns the interpreted content of this cell.
    pub fn content(&self) -> &TableCellContent<'src> {
        &self.content
    }

    /// Resolves any deferred cross-references in this cell's content.
    fn resolve_references(
        &mut self,
        resolver: &dyn ReferenceResolver,
        renderer: &dyn InlineSubstitutionRenderer,
        warnings: &mut Vec<ReferenceWarning>,
    ) {
        match &mut self.content {
            TableCellContent::Simple(content) => {
                content.resolve_references(resolver, renderer, warnings);
            }
            TableCellContent::AsciiDoc(blocks) => {
                for block in blocks.iter_mut() {
                    block.resolve_references(resolver, renderer, warnings);
                }
            }
        }
    }
}

/// The interpreted content of a [`TableCell`].
///
/// The variant is determined by the [style](ColumnStyle) of the cell's column:
/// an [`AsciiDoc`](ColumnStyle::AsciiDoc) column produces
/// [`AsciiDoc`](Self::AsciiDoc) content, and every other style produces
/// [`Simple`](Self::Simple) inline content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TableCellContent<'src> {
    /// Inline content: the cell's text after its substitutions (normal for most
    /// styles, verbatim for [`Literal`](ColumnStyle::Literal)) have been
    /// applied.
    Simple(Content<'src>),

    /// Block content: the cell's text parsed as a nested, standalone AsciiDoc
    /// document. Produced by the [`AsciiDoc`](ColumnStyle::AsciiDoc) style.
    AsciiDoc(Vec<Block<'src>>),
}

/// Parse the value of the `cols` attribute into a list of columns.
///
/// The value is a comma-separated list of column specifiers. A specifier may be
/// preceded by a multiplier (`<n>*`) that repeats the column `n` times. The
/// alignment operators and proportional width of a specifier are interpreted;
/// the style operator is not yet.
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

/// Parse a single column specifier, extracting its alignment, proportional
/// width, and style.
///
/// A column specifier is positional: an optional horizontal alignment operator
/// (`<`, `>`, or `^`) comes first, followed by an optional vertical alignment
/// operator (`.<`, `.>`, or `.^`), followed by the width, and finally an
/// optional style operator in the last position. When a multiplier (`<n>*`) is
/// present, the operators follow the multiplier, so the `spec` passed here is
/// the portion after the `*`.
///
/// The width is either the special autowidth value `~` (sizing the column to
/// its content) or the first contiguous run of digits after any alignment
/// operators; a spec with neither falls back to the default width. The style
/// operator is the trailing letter (`a`, `d`, `e`, `h`, `l`, `m`, or `s`); an
/// unrecognized trailing letter leaves the style at its default.
fn parse_col_spec(spec: &str) -> TableColumn {
    let mut rest = spec.trim();

    // Horizontal alignment operator (if present) always comes first.
    let mut h_align = HorizontalAlignment::Left;
    match rest.as_bytes().first() {
        Some(b'<') => {
            h_align = HorizontalAlignment::Left;
            rest = &rest[1..];
        }
        Some(b'>') => {
            h_align = HorizontalAlignment::Right;
            rest = &rest[1..];
        }
        Some(b'^') => {
            h_align = HorizontalAlignment::Center;
            rest = &rest[1..];
        }
        _ => {}
    }

    // Vertical alignment operator (if present) follows, introduced by a dot.
    let mut v_align = VerticalAlignment::Top;
    if let Some(after_dot) = rest.strip_prefix('.') {
        match after_dot.as_bytes().first() {
            Some(b'<') => {
                v_align = VerticalAlignment::Top;
                rest = &after_dot[1..];
            }
            Some(b'>') => {
                v_align = VerticalAlignment::Bottom;
                rest = &after_dot[1..];
            }
            Some(b'^') => {
                v_align = VerticalAlignment::Middle;
                rest = &after_dot[1..];
            }
            _ => {}
        }
    }

    // Width comes after the alignment operators. The special value `~` marks
    // the column as autowidth (sized to its content); otherwise the width is
    // the first run of digits. A spec with neither falls back to the default
    // proportional width.
    let mut autowidth = false;
    let mut width = TableColumn::default().width;
    if let Some(after_tilde) = rest.strip_prefix('~') {
        autowidth = true;
        rest = after_tilde;
    } else {
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(parsed) = digits.parse::<usize>()
            && parsed > 0
        {
            width = parsed;
        }
        rest = &rest[digits.len()..];
    }

    // The style operator, if present, occupies the last position on the
    // specifier, so it is the entire remainder after the width. Matching the
    // whole remainder (rather than just its first byte) means a malformed spec
    // with trailing junk — e.g. `1em` — falls back to the default style instead
    // of silently honoring the first letter and discarding the rest.
    let style = match rest.trim() {
        "a" => ColumnStyle::AsciiDoc,
        "d" => ColumnStyle::Default,
        "e" => ColumnStyle::Emphasis,
        "h" => ColumnStyle::Header,
        "l" => ColumnStyle::Literal,
        "m" => ColumnStyle::Monospace,
        "s" => ColumnStyle::Strong,
        _ => ColumnStyle::Default,
    };

    TableColumn {
        width,
        autowidth,
        h_align,
        v_align,
        style,
    }
}

/// The span, alignment, and style overrides parsed from a
/// [cell specifier](RawCell::spec).
///
/// Each alignment and style field is `None` when the corresponding operator is
/// absent from the specifier, in which case the cell inherits that alignment
/// (or style) from its column. `colspan` and `rowspan` are the number of
/// columns and rows the cell spans; they default to `1` (no span). `repeat` is
/// the duplication factor — the number of consecutive cells the content is
/// cloned into — and defaults to `1` (no duplication).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CellSpec {
    h_align: Option<HorizontalAlignment>,
    v_align: Option<VerticalAlignment>,
    style: Option<ColumnStyle>,
    colspan: usize,
    rowspan: usize,
    repeat: usize,
}

impl Default for CellSpec {
    fn default() -> Self {
        Self {
            h_align: None,
            v_align: None,
            style: None,
            colspan: 1,
            rowspan: 1,
            repeat: 1,
        }
    }
}

/// A single PSV cell as located by [`scan_cells`]: the alignment operators from
/// its specifier together with the raw (untrimmed) span of its content.
#[derive(Clone, Copy)]
struct RawCell<'src> {
    spec: CellSpec,
    content: Span<'src>,
}

/// The largest number of cells a single duplication factor (`<n>*`) is allowed
/// to expand into.
///
/// A duplicated cell is materialized as `<n>` independent cells, so the factor
/// is an amplification: a dozen source bytes such as `1000000000*` would
/// otherwise request a billion `RawCell`s (a multi-gigabyte allocation).
/// Capping the per-specifier factor bounds that amplification while leaving any
/// realistic table — which never duplicates a cell more than a handful of times
/// — untouched. (This is the one point where the implementation diverges from
/// Asciidoctor, which expands the literal factor however large.)
const MAX_DUPLICATION_FACTOR: usize = 1_000;

/// Expand each duplicated cell into the `<n>` independent cells it represents.
///
/// A cell specifier with a duplication factor (`<n>*`) clones the cell's
/// content and properties into `<n>` consecutive cells. Each clone is an
/// ordinary single-slot cell (colspan and rowspan of 1), so expanding here —
/// before the grid is walked — lets the clones flow into rows exactly like
/// cells the author typed out by hand. A duplication factor of zero produces no
/// cells, dropping the original (matching Asciidoctor). A cell with no
/// duplication factor has a `repeat` of 1 and so passes through unchanged. The
/// factor is clamped to [`MAX_DUPLICATION_FACTOR`] so a hostile specifier can't
/// trigger a runaway allocation.
fn expand_duplicates(cells: Vec<RawCell<'_>>) -> Vec<RawCell<'_>> {
    // The common case is no duplication at all, so only the clones beyond the
    // first add to the count.
    let extra: usize = cells
        .iter()
        .map(|c| c.spec.repeat.min(MAX_DUPLICATION_FACTOR).saturating_sub(1))
        .sum();
    let mut expanded = Vec::with_capacity(cells.len() + extra);
    for cell in cells {
        for _ in 0..cell.spec.repeat.min(MAX_DUPLICATION_FACTOR) {
            expanded.push(cell);
        }
    }
    expanded
}

/// Scan a region for PSV cell boundaries, returning the [specifier](CellSpec)
/// and raw (untrimmed) content span of each cell.
///
/// A cell boundary is a vertical bar (`|`) that appears at the start of a line
/// or is preceded by whitespace, optionally with a [cell specifier](CellSpec)
/// (e.g. `^`, `2+`, `.>`) directly in front of the `|`. The token immediately
/// preceding a `|` is taken to be a specifier when it parses as one (see
/// [`parse_cell_spec`]); a token that doesn't parse as a specifier means the
/// `|` is not a cell boundary. Content before the first boundary is ignored.
///
/// An escaped separator (`\|`) is preceded by a backslash, which is not a valid
/// specifier, so the `|` already fails the boundary test and needs no special
/// handling here; the backslash is stripped later in [`TableCell::parse`].
fn scan_cells(region: Span<'_>) -> Vec<RawCell<'_>> {
    let data = region.data();
    let bytes = data.as_bytes();
    let len = bytes.len();

    let mut cells: Vec<RawCell<'_>> = vec![];
    // The content start and specifier of the cell currently being accumulated.
    let mut content_start: Option<usize> = None;
    let mut cur_spec = CellSpec::default();
    let mut i = 0;

    while i < len {
        if bytes.get(i).copied() == Some(b'|') {
            // Walk back to the start of the token directly preceding this `|`.
            // The token (a possible cell specifier) runs back to the previous
            // whitespace, tab, or newline, or to the start of the region; either
            // way the token is anchored at a line start or after whitespace, as a
            // cell boundary requires. (When `tok_start == i` the token is empty
            // and the separator is plain.)
            let mut tok_start = i;
            while tok_start > 0
                && !matches!(
                    bytes.get(tok_start - 1).copied(),
                    Some(b' ' | b'\t' | b'\n')
                )
            {
                tok_start -= 1;
            }

            let token = data.get(tok_start..i).unwrap_or_default();
            let spec = if token.is_empty() {
                Some(CellSpec::default())
            } else {
                parse_cell_spec(token)
            };

            if let Some(spec) = spec {
                if let Some(start) = content_start {
                    // The previous cell's content ends at the start of this
                    // cell's specifier; the separating whitespace, included in
                    // the slice, is trimmed later in `TableCell::parse`.
                    cells.push(RawCell {
                        spec: cur_spec,
                        content: region.slice(start..tok_start),
                    });
                }
                cur_spec = spec;
                content_start = Some(i + 1);
            }
        }

        i += 1;
    }

    if let Some(start) = content_start {
        cells.push(RawCell {
            spec: cur_spec,
            content: region.slice(start..len),
        });
    }

    cells
}

/// Parse a cell specifier, returning its [span and overrides](CellSpec), or
/// `None` if `token` is not a valid cell specifier.
///
/// A cell specifier is positional and every part is optional, but the whole
/// token must be consumed for it to be valid:
///
/// ```text
/// <factor><span or duplication operator><horizontal><vertical><style>
/// ```
///
/// * The factor and span/duplication operator are an optional count (e.g. `2`,
///   `2.3`, `.3`) that, when present, must be followed by `+` (span) or `*`
///   (duplication). For a span the factor is interpreted as the cell's colspan
///   and rowspan (a missing column or row count defaults to 1). For a
///   duplication the column part of the factor is the duplication count — the
///   number of consecutive cells the content is cloned into — and any row part
///   is ignored; a duplicated cell keeps a colspan and rowspan of 1.
/// * The horizontal alignment operator is `<`, `>`, or `^`.
/// * The vertical alignment operator is a dot followed by `<`, `>`, or `^`.
/// * The style operator is a single lowercase letter in the last position. A
///   recognized operator (`a`, `d`, `e`, `h`, `l`, `m`, or `s`) overrides the
///   column's style on this cell. Any other single lowercase letter still
///   locates the separator but leaves the style at `None`, so the cell inherits
///   its column's style (matching Asciidoctor, which ignores an unrecognized
///   style operator).
fn parse_cell_spec(token: &str) -> Option<CellSpec> {
    let b = token.as_bytes();
    let mut i = 0;

    // Optional span/duplication: an optional span factor followed by `+` (span)
    // or `*` (duplication). The factor is a column count, an optional dot, and an
    // optional row count (`<n>`, `.<n>`, or `<n>.<n>`). The factor is committed
    // only when the operator that must follow it is present; otherwise the
    // leading digits remain and the token fails the full-consumption check below.
    let mut colspan = 1;
    let mut rowspan = 1;
    let mut repeat = 1;
    let col_start = i;
    let mut j = i;
    while matches!(b.get(j).copied(), Some(c) if c.is_ascii_digit()) {
        j += 1;
    }
    let col_end = j;
    let mut has_dot = false;
    let mut row_start = j;
    if b.get(j).copied() == Some(b'.') {
        has_dot = true;
        j += 1;
        row_start = j;
        while matches!(b.get(j).copied(), Some(c) if c.is_ascii_digit()) {
            j += 1;
        }
    }
    let row_end = j;
    match b.get(j).copied() {
        // Span: the factor is interpreted as a colspan and rowspan. A missing
        // column or row count defaults to 1, so `2+` spans two columns, `.3+`
        // spans three rows, and `2.3+` spans a 2x3 block.
        Some(b'+') => {
            // The factor consists only of ASCII digits and dots, so these ranges
            // are always valid `str` slices.
            let col_digits = token.get(col_start..col_end).unwrap_or_default();
            if !col_digits.is_empty() {
                colspan = col_digits.parse().unwrap_or(1);
            }
            if has_dot {
                let row_digits = token.get(row_start..row_end).unwrap_or_default();
                if !row_digits.is_empty() {
                    rowspan = row_digits.parse().unwrap_or(1);
                }
            }
            i = j + 1;
        }
        // Duplication: the factor is interpreted as a duplication count, so the
        // cell's content and properties are cloned into `<n>` consecutive cells.
        // Only the column part of the factor is the count; any row part (`<n>.`)
        // is ignored, matching Asciidoctor. A missing column count defaults to 1.
        // Unlike a span, a duplication leaves `colspan` and `rowspan` at 1: each
        // clone is an ordinary single-slot cell.
        Some(b'*') => {
            let col_digits = token.get(col_start..col_end).unwrap_or_default();
            if !col_digits.is_empty() {
                repeat = col_digits.parse().unwrap_or(1);
            }
            i = j + 1;
        }
        _ => {}
    }

    // Optional horizontal alignment operator.
    let mut h_align = None;
    match b.get(i).copied() {
        Some(b'<') => {
            h_align = Some(HorizontalAlignment::Left);
            i += 1;
        }
        Some(b'>') => {
            h_align = Some(HorizontalAlignment::Right);
            i += 1;
        }
        Some(b'^') => {
            h_align = Some(HorizontalAlignment::Center);
            i += 1;
        }
        _ => {}
    }

    // Optional vertical alignment operator, introduced by a dot.
    let mut v_align = None;
    if b.get(i).copied() == Some(b'.') {
        match b.get(i + 1).copied() {
            Some(b'<') => {
                v_align = Some(VerticalAlignment::Top);
                i += 2;
            }
            Some(b'>') => {
                v_align = Some(VerticalAlignment::Bottom);
                i += 2;
            }
            Some(b'^') => {
                v_align = Some(VerticalAlignment::Middle);
                i += 2;
            }
            _ => {}
        }
    }

    // Optional style operator: a single lowercase letter in the last position.
    // A recognized letter overrides the column's style; any other lowercase
    // letter is consumed (so the separator is still located) but leaves the
    // style at `None`, so the cell inherits its column's style.
    let mut style = None;
    if let Some(c) = b.get(i).copied()
        && c.is_ascii_lowercase()
    {
        style = match c {
            b'a' => Some(ColumnStyle::AsciiDoc),
            b'd' => Some(ColumnStyle::Default),
            b'e' => Some(ColumnStyle::Emphasis),
            b'h' => Some(ColumnStyle::Header),
            b'l' => Some(ColumnStyle::Literal),
            b'm' => Some(ColumnStyle::Monospace),
            b's' => Some(ColumnStyle::Strong),
            _ => None,
        };
        i += 1;
    }

    // The token is a cell specifier only if it was consumed in its entirety.
    if i == b.len() {
        Some(CellSpec {
            h_align,
            v_align,
            style,
            colspan,
            rowspan,
            repeat,
        })
    } else {
        None
    }
}

/// Return the subspan of `s` with surrounding whitespace (including newlines)
/// removed.
fn trim_surrounding_whitespace(s: Span<'_>) -> Span<'_> {
    let data = s.data();
    let start = data.len() - data.trim_start().len();
    let len = data.trim().len();
    s.slice(start..start + len)
}
