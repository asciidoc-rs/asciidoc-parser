use std::{collections::VecDeque, sync::Arc};

use self_cell::self_cell;

use crate::{
    HasSpan, Parser, Span,
    attributes::Attrlist,
    blocks::{
        Block, ContentModel, IsBlock, caption::assign_block_caption, metadata::BlockMetadata,
        parse_utils::parse_blocks_until,
    },
    content::{Content, SubstitutionGroup},
    document::{InterpretedValue, TocConfig, TocMode},
    parser::{
        AttributeValue, InlineSubstitutionRenderer, ModificationContext, ReferenceResolver,
        ReferenceWarning, ResolvedAttributes, built_in_attr, built_in_attrs_iter,
        preprocessor::preprocess_with_initial_file_name,
    },
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
/// A table is introduced by a table delimiter (`|===`, or `!===` for a nested
/// table) and closed by a matching delimiter. By default cells are separated
/// using prefix-separated value (PSV) syntax: the table's cell separator — a
/// vertical bar (`|`) by default — at the start of a line or preceded by
/// whitespace begins a new cell. Cells flow, in document order, into rows whose
/// length is fixed by the number of columns. (The separator defaults to `!`
/// inside a nested table and can be overridden with the `separator` attribute;
/// see below.)
///
/// The number of columns is determined either by the `cols` attribute or,
/// implicitly, by the number of cells found in the first non-empty line after
/// the opening delimiter.
///
/// # Data formats
///
/// In addition to the default PSV format, a table can be populated from
/// delimiter-separated data with the [`format`](Self::data_format) attribute:
/// `csv` (comma-separated values), `tsv` (tab-separated values), or `dsv`
/// (delimited values, colon-separated by default). The `,===` and `:===`
/// shorthand delimiters select the CSV and DSV formats respectively without an
/// explicit `format` attribute. In a data format the separator is placed
/// *between* values (not in front of each cell) and a cell carries no
/// formatting spec; cell formatting is instead applied per column with the
/// `cols` attribute. See [`DataFormat`] for the parsing rules.
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
///
/// Table borders are supported: the [`frame`](Self::frame) attribute controls
/// the border around the table and the [`grid`](Self::grid) attribute controls
/// the borders between cells. Each falls back to a document-level default
/// (`table-frame` / `table-grid`) and then to `all`.
///
/// Zebra striping is supported via the [`stripes`](Self::stripes) attribute,
/// which falls back to the `table-stripes` document attribute and then to
/// `none`.
///
/// Nested tables are supported: an [`AsciiDoc`](ColumnStyle::AsciiDoc) cell may
/// contain its own table. The cell separator defaults to the vertical bar (`|`)
/// but switches to the exclamation mark (`!`) inside an AsciiDoc cell, so a
/// nested table is opened with `!===` and separates its cells with `!`. The
/// `separator` attribute overrides the default separator with an explicit
/// character at any level.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableBlock<'src> {
    columns: Vec<TableColumn>,
    data_format: DataFormat,
    header_row: Option<TableRow<'src>>,
    body_rows: Vec<TableRow<'src>>,
    footer_row: Option<TableRow<'src>>,
    source: Span<'src>,
    title_source: Option<Span<'src>>,
    title: Option<String>,
    caption: Option<String>,
    number: Option<usize>,
    frame: Frame,
    grid: Grid,
    stripes: Stripes,
    anchor: Option<Span<'src>>,
    anchor_reftext: Option<Span<'src>>,
    attrlist: Option<Attrlist<'src>>,
}

impl<'src> TableBlock<'src> {
    /// Returns `true` if `line` is a table delimiter.
    ///
    /// A table delimiter is one of the lead characters `|`, `!`, `,`, or `:`
    /// followed by three or more equals signs (`===`). The lead character also
    /// selects the table's data format and default cell separator:
    ///
    /// * `|===` is the ordinary (PSV) table delimiter.
    /// * `!===` opens a table whose default cell separator is the exclamation
    ///   mark, which lets a nested table be distinguished from the
    ///   `|`-separated table that encloses it.
    /// * `,===` is the shorthand for a CSV table.
    /// * `:===` is the shorthand for a DSV table.
    pub(crate) fn is_table_delimiter(line: &Span<'src>) -> bool {
        let data = line.data();
        // `len() >= 4` plus the leading delimiter character guarantees `rest`
        // holds at least three bytes, so the closure only needs to confirm they
        // are all `=`.
        data.len() >= 4
            && matches!(data.as_bytes().first(), Some(b'|' | b'!' | b',' | b':'))
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

        // The data format governs how the table body is split into cells. It
        // defaults to PSV, but the `format` attribute selects CSV, TSV, or DSV,
        // and the `,===` / `:===` shorthand delimiters select CSV / DSV. The
        // lead character of the delimiter (`delimiter_text`) is passed so the
        // shorthand can be honored.
        let data_format = resolve_data_format(metadata, delimiter_text);

        // The cell separator partitions each row into cells. In PSV it defaults
        // to the vertical bar (`|`), except inside an AsciiDoc table cell — a
        // nested, standalone document — where it defaults to the exclamation
        // mark (`!`) so a nested table is distinguished from the `|`-separated
        // table that encloses it. Each data format has its own default (CSV =
        // comma, TSV = tab, DSV = colon). The `separator` attribute overrides
        // the default; an empty `separator` falls back to the default, and the
        // two-character sequence `\t` is interpreted as a tab.
        let separator = resolve_separator(metadata, parser, data_format);

        // The `cols` attribute, when present, fixes the number of columns and
        // carries the per-column formatting. When it is absent the column count
        // is implicit (resolved per format below).
        let cols_attr: Vec<TableColumn> = metadata
            .attrlist
            .as_ref()
            .and_then(|a| a.named_attribute("cols"))
            .map(|attr| parse_cols(attr.value()))
            .unwrap_or_default();

        // The `autowidth` option sizes the table to its content; the columns
        // inherit the setting, so every column becomes autowidth regardless of
        // any proportional width set on its specifier.
        let autowidth = metadata
            .attrlist
            .as_ref()
            .is_some_and(|a| a.has_option("autowidth"));

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

        // An implicit header additionally requires that the first row be complete
        // on the first line. If the first cell spans multiple lines — for PSV,
        // the first non-blank line after the blank gap continues the cell instead
        // of starting a new one; for CSV/TSV, the first line opens a quoted value
        // that is not closed on that line — there is no implicit header (matching
        // Asciidoctor, which cancels the implicit header in these cases).
        let first_row_complete = match data_format {
            DataFormat::Psv => first_nonblank_line(line1.after)
                .is_none_or(|line| psv_line_starts_cell(line.data(), separator.as_str())),
            DataFormat::Csv | DataFormat::Tsv => !line_has_unclosed_quote(line1.item.data()),
            DataFormat::Dsv => true,
        };

        let has_header =
            opts_header || (!opts_noheader && !line1_blank && line2_blank && first_row_complete);

        // A titled table is given a caption (e.g. "Table 1. ") that a processor
        // prepends to the title, drawn from the `table-caption` attribute (which
        // defaults to "Table"); each such captioned table consumes the next
        // value of a document-wide table counter. An explicit `caption`
        // attribute sets the label verbatim with no number; an explicitly empty
        // `caption` (e.g. `[caption=]`) removes the label entirely. When
        // `table-caption` is unset and no explicit `caption` is given, no caption
        // (and no number) is assigned. See [`assign_block_caption`] for the full,
        // shared rules.
        //
        // Computed before the cell iterator below borrows `parser` immutably, so
        // that the mutable counter update does not conflict with that borrow.
        let caption = assign_block_caption(
            parser,
            "table",
            metadata.attrlist.as_ref(),
            metadata.title.is_some(),
        );
        let number = caption.as_ref().and_then(|caption| caption.number);
        let caption = caption.map(|caption| caption.prefix);

        // The `frame` and `grid` attributes control the table's borders, and the
        // `stripes` attribute controls zebra striping. The borders each default
        // to `all` and stripes defaults to `none`; the default can be changed for
        // the whole document with the `table-frame` / `table-grid` /
        // `table-stripes` attribute, and an explicit attribute on the table
        // overrides both. Each value is resolved here (while `parser` is borrowed
        // only immutably) and stored on the block so the accessors need no further
        // document lookup.
        let frame = resolve_table_attribute::<Frame>(metadata, parser, "frame", "table-frame");
        let grid = resolve_table_attribute::<Grid>(metadata, parser, "grid", "table-grid");
        let stripes =
            resolve_table_attribute::<Stripes>(metadata, parser, "stripes", "table-stripes");

        // Split the body into columns and rows according to the data format.
        // PSV walks a grid that honors cell spans and duplication; the data
        // formats (CSV/TSV/DSV) split on a separator with no per-cell spec and
        // flow the values into fixed-width rows.
        let mut warnings: Vec<Warning<'src>> = vec![];
        let body = TableBody {
            inside,
            separator,
            cols_attr,
            autowidth,
            has_header,
        };
        let (columns, rows) = match data_format {
            DataFormat::Psv => build_psv_table(body, parser, &mut warnings),
            DataFormat::Csv | DataFormat::Tsv | DataFormat::Dsv => {
                build_data_table(body, data_format, parser, &mut warnings)
            }
        };

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
                    data_format,
                    header_row,
                    body_rows,
                    footer_row,
                    source,
                    title_source: metadata.title_source,
                    title: metadata.title.clone(),
                    caption,
                    number,
                    frame,
                    grid,
                    stripes,
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

    /// Returns the number assigned to this table, if any.
    ///
    /// A titled table for which the `table-caption` attribute is set is
    /// numbered with an automatically incremented, document-wide table counter
    /// (the same number that appears in its [`caption`](Self::caption), e.g.
    /// the `1` in `"Table 1. "`). The number is absent when the table is
    /// not captioned, or when its caption comes from an explicit
    /// (unnumbered) `caption` attribute.
    pub fn number(&self) -> Option<usize> {
        self.number
    }

    /// Returns the columns of this table.
    pub fn columns(&self) -> &[TableColumn] {
        &self.columns
    }

    /// Returns the [`DataFormat`] used to populate this table.
    ///
    /// The format comes from the `format` attribute on the table (`psv`, `csv`,
    /// `tsv`, or `dsv`) or from a shorthand delimiter (`,===` selects CSV,
    /// `:===` selects DSV). When neither is present the format defaults to
    /// [`DataFormat::Psv`].
    pub fn data_format(&self) -> DataFormat {
        self.data_format
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

    /// Returns the [`Frame`] that controls the border drawn around this table.
    ///
    /// The frame comes from the `frame` attribute on the table, which accepts
    /// `all`, `ends`, `sides`, or `none`. When the attribute is absent the
    /// value is taken from the `table-frame` document attribute, and when
    /// that too is absent it defaults to [`Frame::All`].
    pub fn frame(&self) -> Frame {
        self.frame
    }

    /// Returns the [`Grid`] that controls the borders drawn between this
    /// table's cells.
    ///
    /// The grid comes from the `grid` attribute on the table, which accepts
    /// `all`, `rows`, `cols`, or `none`. When the attribute is absent the value
    /// is taken from the `table-grid` document attribute, and when that too is
    /// absent it defaults to [`Grid::All`].
    pub fn grid(&self) -> Grid {
        self.grid
    }

    /// Returns the [`Stripes`] that control which rows of this table are shaded
    /// to create a zebra-striping effect.
    ///
    /// The stripes come from the `stripes` attribute on the table, which
    /// accepts `none`, `even`, `odd`, `all`, or `hover`. When the attribute
    /// is absent the value is taken from the `table-stripes` document
    /// attribute, and when that too is absent it defaults to
    /// [`Stripes::None`].
    ///
    /// As a shorthand, a `stripes-<value>` role on the table (e.g.
    /// `[.stripes-even]`) applies the same CSS class directly without setting
    /// the `stripes` attribute. That shorthand does not affect this value
    /// (which remains [`Stripes::None`]); the role is instead reported
    /// among the table's [roles](crate::attributes::Attrlist::roles).
    pub fn stripes(&self) -> Stripes {
        self.stripes
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

    // These forward to the inherent `caption()`/`number()` (the documented
    // public accessors) so that the captioned table is reported correctly
    // through the trait interface too — `dyn IsBlock` / generic `T: IsBlock`
    // consumers resolve to these rather than the inherent methods.
    fn caption(&self) -> Option<&str> {
        self.caption.as_deref()
    }

    fn number(&self) -> Option<usize> {
        self.number
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
    /// column (the column is sized to its content instead). A column made
    /// autowidth by the `~` specifier reports the default width of `1`, but one
    /// that inherits autowidth from the table's `autowidth` option retains
    /// whatever width its specifier set (e.g. `2` for the first column of
    /// `[%autowidth,cols="2,1"]`).
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

/// The data format that governs how a [`TableBlock`]'s body is split into
/// cells.
///
/// The format is selected by the `format` attribute (`psv`, `csv`, `tsv`, or
/// `dsv`) or by a shorthand delimiter (`,===` for CSV, `:===` for DSV). The
/// default is [`Psv`](Self::Psv).
///
/// In the PSV format the separator is placed in front of each cell and a cell
/// may carry a formatting spec. In the delimiter-separated formats (CSV, TSV,
/// and DSV) the separator is placed *between* values and a cell carries no
/// spec; cell formatting is applied per column with the `cols` attribute
/// instead. In every delimiter-separated format empty lines are skipped,
/// whitespace surrounding each value is stripped, and a "ragged" table (whose
/// rows do not all have the same number of cells) has its cells flowed into
/// fixed-width rows, dropping any cells left over at the end.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DataFormat {
    /// Prefix-separated values: the default format. The separator (a vertical
    /// bar, `|`, by default) is placed in front of each cell.
    #[default]
    Psv,

    /// Comma-separated values (the `csv` format). The default separator is a
    /// comma (`,`). Values may be enclosed in double quotes (`"`), within which
    /// the separator and newlines are literal and a double quote is written by
    /// doubling it (`""`); a newline that is not inside a quoted value begins a
    /// new row. Loosely based on RFC 4180.
    Csv,

    /// Tab-separated values (the `tsv` format). Parsed by the same rules as
    /// [`Csv`](Self::Csv), but the default separator is a tab.
    Tsv,

    /// Delimited values (the `dsv` format). The default separator is a colon
    /// (`:`). Unlike CSV and TSV, an enclosing character is not recognized;
    /// instead the separator can be included in a value by escaping it with a
    /// single backslash (`\:`).
    Dsv,
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

/// The border drawn around a [`TableBlock`].
///
/// The frame is set with the `frame` attribute on the table (or, document-wide,
/// the `table-frame` attribute). The default is [`All`](Self::All).
///
/// An unrecognized value falls back to [`All`](Self::All). (Asciidoctor instead
/// passes an unrecognized value straight through to a CSS class, which the
/// stylesheet ignores; this parser models only the four documented values.)
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Frame {
    /// A border is drawn on every side of the table (the `all` value). This is
    /// the default frame.
    #[default]
    All,

    /// A border is drawn on the top and bottom of the table (the `ends` value).
    ///
    /// The `topbot` value recognized by older versions of AsciiDoc is accepted
    /// as a synonym.
    Ends,

    /// A border is drawn on the left and right sides of the table (the `sides`
    /// value).
    Sides,

    /// No border is drawn around the table (the `none` value).
    None,
}

/// The borders drawn between the cells of a [`TableBlock`].
///
/// The grid is set with the `grid` attribute on the table (or, document-wide,
/// the `table-grid` attribute). The default is [`All`](Self::All).
///
/// An unrecognized value falls back to [`All`](Self::All). (Asciidoctor instead
/// passes an unrecognized value straight through to a CSS class, which the
/// stylesheet ignores; this parser models only the four documented values.)
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Grid {
    /// A border is drawn between all cells (the `all` value). This is the
    /// default grid.
    #[default]
    All,

    /// A border is drawn between the rows of the table (the `rows` value).
    Rows,

    /// A border is drawn between the columns of the table (the `cols` value).
    Cols,

    /// No border is drawn between the cells (the `none` value).
    None,
}

/// The zebra striping applied to the rows of a [`TableBlock`].
///
/// Striping shades the specified rows with a background color to create a zebra
/// effect. It is set with the `stripes` attribute on the table (or,
/// document-wide, the `table-stripes` attribute). The default is
/// [`None`](Self::None).
///
/// Under the covers, a converter applies the CSS class `stripes-<value>` to the
/// table; the actual shading depends on the stylesheet. As a shorthand, the
/// same class can be applied directly with a role (e.g. `[.stripes-even]`)
/// rather than the `stripes` attribute. A role does not set this value (see
/// [`TableBlock::stripes`]).
///
/// An unrecognized value falls back to [`None`](Self::None). (Asciidoctor
/// instead passes an unrecognized value straight through to a CSS class, which
/// the stylesheet ignores; this parser models only the five documented values.)
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Stripes {
    /// No rows are shaded (the `none` value). This is the default.
    #[default]
    None,

    /// Even rows are shaded (the `even` value).
    Even,

    /// Odd rows are shaded (the `odd` value).
    Odd,

    /// All rows are shaded (the `all` value).
    All,

    /// The row under the mouse cursor is shaded (the `hover` value). This has
    /// an effect only in HTML output.
    Hover,
}

/// A table-level attribute value ([`Frame`], [`Grid`], or [`Stripes`]) that can
/// be parsed from an attribute value and has a default.
trait TableAttributeValue: Copy + Default {
    /// Parse the value of the table attribute (or its document-level
    /// `table-<name>` counterpart). An unrecognized value yields the default.
    fn from_attr_value(value: &str) -> Self;
}

impl TableAttributeValue for Frame {
    fn from_attr_value(value: &str) -> Self {
        match value.trim() {
            // `topbot` is the older synonym for `ends`.
            "ends" | "topbot" => Frame::Ends,
            "sides" => Frame::Sides,
            "none" => Frame::None,
            // `all` and any unrecognized value.
            _ => Frame::All,
        }
    }
}

impl TableAttributeValue for Grid {
    fn from_attr_value(value: &str) -> Self {
        match value.trim() {
            "rows" => Grid::Rows,
            "cols" => Grid::Cols,
            "none" => Grid::None,
            // `all` and any unrecognized value.
            _ => Grid::All,
        }
    }
}

impl TableAttributeValue for Stripes {
    fn from_attr_value(value: &str) -> Self {
        match value.trim() {
            "even" => Stripes::Even,
            "odd" => Stripes::Odd,
            "all" => Stripes::All,
            "hover" => Stripes::Hover,
            // `none` and any unrecognized value.
            _ => Stripes::None,
        }
    }
}

/// Resolve a table-level attribute ([`Frame`], [`Grid`], or [`Stripes`]).
///
/// An explicit attribute on the table (`attr_name`) wins; otherwise the
/// document-level default (`doc_attr_name`) is consulted; otherwise the value
/// falls back to the type's default.
fn resolve_table_attribute<B: TableAttributeValue>(
    metadata: &BlockMetadata<'_>,
    parser: &Parser,
    attr_name: &str,
    doc_attr_name: &str,
) -> B {
    if let Some(attr) = metadata
        .attrlist
        .as_ref()
        .and_then(|a| a.named_attribute(attr_name))
    {
        B::from_attr_value(attr.value())
    } else if let InterpretedValue::Value(value) = parser.attribute_value(doc_attr_name) {
        B::from_attr_value(&value)
    } else {
        B::default()
    }
}

/// Resolve the [`DataFormat`] of a table.
///
/// An explicit, recognized `format` attribute (`psv`, `csv`, `tsv`, or `dsv`)
/// always wins. Otherwise the lead character of the delimiter selects the
/// format via its shorthand — `,===` is CSV and `:===` is DSV — and any other
/// delimiter (`|===`, `!===`) is PSV.
fn resolve_data_format(metadata: &BlockMetadata<'_>, delimiter_text: &str) -> DataFormat {
    if let Some(attr) = metadata
        .attrlist
        .as_ref()
        .and_then(|a| a.named_attribute("format"))
    {
        match attr.value().trim() {
            "psv" => return DataFormat::Psv,
            "csv" => return DataFormat::Csv,
            "tsv" => return DataFormat::Tsv,
            "dsv" => return DataFormat::Dsv,
            // An unrecognized format value falls through to the shorthand (or
            // the PSV default).
            _ => {}
        }
    }

    match delimiter_text.as_bytes().first() {
        Some(b',') => DataFormat::Csv,
        Some(b':') => DataFormat::Dsv,
        _ => DataFormat::Psv,
    }
}

/// Resolve the cell separator for a table.
///
/// Each [`DataFormat`] supplies a default separator: PSV uses the vertical bar
/// (`|`), except inside an AsciiDoc table cell — a nested, standalone document
/// — where it defaults to the exclamation mark (`!`) so a nested table is
/// distinguished from the `|`-separated table that encloses it; CSV defaults to
/// a comma (`,`), TSV to a tab, and DSV to a colon (`:`). An explicit
/// `separator` attribute on the table overrides the default; an empty
/// `separator` value (e.g. `[separator=]`) falls back to the default. The
/// two-character sequence `\t` in the attribute value is interpreted as a tab,
/// so a tab-separated table can be written `[format=csv,separator=\t]`.
fn resolve_separator(
    metadata: &BlockMetadata<'_>,
    parser: &Parser,
    data_format: DataFormat,
) -> String {
    let default = match data_format {
        DataFormat::Psv => {
            if parser.nested_document_depth > 0 {
                "!"
            } else {
                "|"
            }
        }
        DataFormat::Csv => ",",
        DataFormat::Tsv => "\t",
        DataFormat::Dsv => ":",
    };

    metadata
        .attrlist
        .as_ref()
        .and_then(|a| a.named_attribute("separator"))
        .map(|attr| attr.value())
        .filter(|value| !value.is_empty())
        // The author writes a literal tab as the escape sequence `\t`.
        .map(|value| value.replace("\\t", "\t"))
        .unwrap_or_else(|| default.to_string())
}

/// Finalize a table's columns once the column count is known.
///
/// When the `cols` attribute supplied columns (`cols_attr` is non-empty) they
/// are used as-is; otherwise `ncols` default columns are created. When the
/// table carries the `autowidth` option, every column is made autowidth
/// regardless of the proportional width set on its specifier.
fn finalize_columns(
    cols_attr: Vec<TableColumn>,
    ncols: usize,
    autowidth: bool,
) -> Vec<TableColumn> {
    let mut columns = if cols_attr.is_empty() {
        (0..ncols).map(|_| TableColumn::default()).collect()
    } else {
        cols_attr
    };

    if autowidth {
        for column in columns.iter_mut() {
            column.autowidth = true;
        }
    }

    columns
}

/// The inputs shared by the PSV and data-format table-body builders.
struct TableBody<'src> {
    /// The region between the opening and closing delimiters.
    inside: Span<'src>,

    /// The resolved cell separator.
    separator: String,

    /// Columns parsed from the `cols` attribute (empty when the attribute is
    /// absent, in which case the column count is implicit).
    cols_attr: Vec<TableColumn>,

    /// Whether the table carries the `autowidth` option.
    autowidth: bool,

    /// Whether the first row is a header row.
    has_header: bool,
}

/// Build the columns and rows of a PSV (prefix-separated values) table.
///
/// The column count comes from the `cols` attribute (`cols_attr`) or, when that
/// is absent, from the number of column slots in the first non-empty line.
/// Cells are then scanned in document order and partitioned into rows by
/// walking the grid: a cell's span (colspan/rowspan) governs how many column
/// slots it occupies, so a column-spanning cell fills its row with fewer cells
/// and a row-spanning cell carries its columns down into the rows below.
///
/// This mirrors Asciidoctor's grid walk. `active_rowspans[k]` records the
/// number of column slots that cells from earlier rows occupy in the row `k`
/// steps ahead of the one being filled; a row closes once its own cells'
/// colspans plus the slots carried into it (`active_rowspans[0]`) reach
/// `ncols`. A cell whose span pushes the row *past* `ncols` overruns the grid:
/// the whole overrunning row is dropped (with a warning), again matching
/// Asciidoctor. A row whose columns are entirely pre-filled by carried slots
/// has no cells of its own to close it, so the next cell overruns and is
/// dropped together with that pre-filled row. A duplicated cell (`<n>*`) is
/// expanded into `<n>` independent cells — each carrying the original's
/// content, alignment, and style — before the grid walk, so each clone occupies
/// its own column slot exactly like an ordinary cell. A duplication factor of
/// zero drops the cell entirely.
fn build_psv_table<'src>(
    body: TableBody<'src>,
    parser: &mut Parser,
    warnings: &mut Vec<Warning<'src>>,
) -> (Vec<TableColumn>, Vec<TableRow<'src>>) {
    let TableBody {
        inside,
        separator,
        cols_attr,
        autowidth,
        has_header,
    } = body;

    let separator = separator.as_str();

    // When the column count is implicit, it is the number of column slots in the
    // first non-empty line: a cell that spans columns (`<n>+`) counts as `<n>`
    // slots, not one, and a cell duplicated `<n>` times (`<n>*`) counts as `<n>`
    // single-column slots (one per clone).
    let first_line_cells: usize =
        scan_cells(inside.discard_empty_lines().take_line().item, separator)
            .0
            .iter()
            .map(|c| c.spec.colspan.max(1) * c.spec.repeat.min(MAX_DUPLICATION_FACTOR))
            .sum();

    let ncols = if cols_attr.is_empty() {
        first_line_cells
    } else {
        cols_attr.len()
    };

    let columns = finalize_columns(cols_attr, ncols, autowidth);

    let (raw_cells, recovered_first_cell) = scan_cells(inside, separator);
    if let Some(source) = recovered_first_cell {
        warnings.push(Warning {
            source,
            warning: WarningType::TableMissingLeadingSeparator,
        });
    }

    let raw_cells = expand_duplicates(raw_cells);

    // A table can never have more rows than it has cells, so a row span is
    // clamped to the cell count for the `active_rowspans` bookkeeping below: a
    // larger span carries into rows that can't exist and so has no additional
    // layout effect. The clamp also bounds the `active_rowspans` allocation, so a
    // hostile specifier such as `.1000000000+` can't trigger a multi-gigabyte
    // allocation. (The cell's reported [`rowspan`] keeps the literal parsed
    // value, matching Asciidoctor.)
    //
    // [`rowspan`]: TableCell::rowspan
    let max_rowspan = raw_cells.len().saturating_add(1);

    let mut raw_rows: Vec<Vec<RawCell<'src>>> = vec![];

    if ncols > 0 {
        // A queue: each completed row consumes the slots carried into it from the
        // front (`pop_front`), while a multi-row cell reserves slots in the rows it
        // extends into via the back. `VecDeque` keeps both ends O(1); a `Vec` would
        // pay an O(n) shift on every `remove(0)`.
        let mut active_rowspans: VecDeque<usize> = VecDeque::from([0]);
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

            // The slots carried into the current row are `active_rowspans[0]`; the
            // deque is never empty here, so the fallback is unreachable.
            let carried = active_rowspans.front().copied().unwrap_or(0);
            let effective = column_visits + carried;
            if effective >= ncols {
                if effective == ncols {
                    raw_rows.push(std::mem::take(&mut current_row));
                } else {
                    // Overrun: this cell's span pushes the row past `ncols`.
                    // Discard the whole row so the remaining cells stay aligned to
                    // the grid.
                    current_row.clear();
                    warnings.push(Warning {
                        source: cell_source,
                        warning: WarningType::TableCellExceedsColumnCount,
                    });
                }
                column_visits = 0;
                active_rowspans.pop_front();
                if active_rowspans.is_empty() {
                    active_rowspans.push_back(0);
                }
            }
        }

        // If the table ends mid-row, the cells accumulated since the last
        // complete row never filled `ncols`. Matching Asciidoctor's
        // `close_table`, that incomplete row is dropped and an error is logged
        // against its last cell.
        if let Some(last) = current_row.last() {
            warnings.push(Warning {
                source: last.content,
                warning: WarningType::TableDroppingIncompleteRowAtEndOfTable,
            });
        }
    }

    // Each cell is processed according to the style of the column it falls in. A
    // cell's column is its ordinal position within its row (matching Asciidoctor,
    // which assigns the column by cell count, not grid slot). The header row
    // (when present) is the first row and is always processed as plain header
    // content, regardless of the column styles, so that a style operator doesn't
    // affect the header row.
    let mut rows: Vec<TableRow<'src>> = Vec::with_capacity(raw_rows.len());
    for (row_idx, raw_row) in raw_rows.into_iter().enumerate() {
        let is_header = has_header && row_idx == 0;
        let mut cells = Vec::with_capacity(raw_row.len());
        for (col_idx, raw) in raw_row.into_iter().enumerate() {
            let column = columns.get(col_idx).cloned().unwrap_or_default();
            cells.push(TableCell::parse(
                raw, &column, is_header, separator, parser, warnings,
            ));
        }
        rows.push(TableRow { cells });
    }

    (columns, rows)
}

/// Build the columns and rows of a delimiter-separated table (CSV, TSV, or
/// DSV).
///
/// The body is split into a flat list of [fields](DataField) by the format's
/// parser, then flowed into fixed-width rows. The column count comes from the
/// `cols` attribute (`cols_attr`) or, when that is absent, from the number of
/// fields in the first row. Because a data cell carries no span, the fields are
/// simply chunked `ncols` at a time; any fields left over after the last
/// complete row are dropped ("extra cells at the end of the last row get
/// dropped"). The first row is the header when `has_header` is set.
fn build_data_table<'src>(
    body: TableBody<'src>,
    data_format: DataFormat,
    parser: &mut Parser,
    warnings: &mut Vec<Warning<'src>>,
) -> (Vec<TableColumn>, Vec<TableRow<'src>>) {
    let TableBody {
        inside,
        separator,
        cols_attr,
        autowidth,
        has_header,
    } = body;

    let separator = separator.as_str();

    // DSV is parsed by its own, simpler rules; CSV and TSV share their rules and
    // differ only in the default separator (resolved by the caller). PSV never
    // reaches this builder.
    let (fields, first_row_len) = if data_format == DataFormat::Dsv {
        parse_dsv_fields(inside, separator)
    } else {
        parse_csv_fields(inside, separator, warnings)
    };

    let ncols = if cols_attr.is_empty() {
        first_row_len
    } else {
        cols_attr.len()
    };

    let columns = finalize_columns(cols_attr, ncols, autowidth);

    // Integer division drops any partial trailing row; `checked_div` yields zero
    // rows when there are no columns.
    let nrows = fields.len().checked_div(ncols).unwrap_or(0);
    let mut rows: Vec<TableRow<'src>> = Vec::with_capacity(nrows);
    let mut fields = fields.into_iter();
    for row_idx in 0..nrows {
        let is_header = has_header && row_idx == 0;
        let mut cells = Vec::with_capacity(ncols);
        for col_idx in 0..ncols {
            // `nrows * ncols <= fields.len()`, so the iterator always yields.
            let Some(field) = fields.next() else { break };
            let column = columns.get(col_idx).cloned().unwrap_or_default();
            cells.push(TableCell::parse_data(
                field, &column, is_header, parser, warnings,
            ));
        }
        rows.push(TableRow { cells });
    }

    (columns, rows)
}

/// A single field of a delimiter-separated (CSV, TSV, or DSV) table, as located
/// by [`parse_csv_fields`] or [`parse_dsv_fields`].
///
/// `content` is the field's value span with surrounding whitespace already
/// stripped. `replacement` holds the value after quote or escape processing
/// when it differs from `content` (a CSV value with a doubled-quote escape, or
/// a DSV value with a backslash-escaped separator); it is `None` when the span
/// is the verbatim value.
struct DataField<'src> {
    content: Span<'src>,
    replacement: Option<String>,
}

/// Parse a CSV/TSV region into its [fields](DataField), returning them in
/// document order together with the number of fields in the first row.
///
/// The rules, loosely based on RFC 4180: empty lines are skipped; whitespace
/// surrounding each value is stripped; a value may be enclosed in double
/// quotes, within which the separator and newlines are literal and a double
/// quote is written by doubling it (`""`). A newline that is not inside a
/// quoted value ends the row. The fields are returned flat; the caller flows
/// them into rows.
///
/// This mirrors Asciidoctor's `Table::ParserContext`: a separator or newline is
/// a cell boundary only when the text accumulated since the previous boundary
/// has no [unclosed quote](has_unclosed_quotes); otherwise it is part of the
/// value. As a result a value whose opening quote is never properly closed (or
/// that has trailing characters after its closing quote) keeps its quotes and
/// absorbs the following separators, rather than being treated as enclosed.
fn parse_csv_fields<'src>(
    region: Span<'src>,
    separator: &str,
    warnings: &mut Vec<Warning<'src>>,
) -> (Vec<DataField<'src>>, usize) {
    let data = region.data();
    let n = data.len();
    let sep_len = separator.len().max(1);
    let at = |k: usize| data.as_bytes().get(k).copied();
    let starts_with_sep = |pos: usize| data.get(pos..).is_some_and(|s| s.starts_with(separator));

    let mut fields: Vec<DataField<'src>> = vec![];
    let mut first_row_len = 0usize;
    let mut first_row_done = false;
    let mut fields_in_row = 0usize;

    // The raw text of the cell currently being accumulated runs from `cell_start`
    // to the next boundary.
    let mut cell_start = 0usize;
    let mut i = 0usize;

    while i <= n {
        let at_eof = i == n;
        let at_sep = !at_eof && starts_with_sep(i);
        let at_nl = !at_eof && at(i) == Some(b'\n');

        if !(at_eof || at_sep || at_nl) {
            i += 1;
            continue;
        }

        let raw = data.get(cell_start..i).unwrap_or_default();

        // A separator or newline that falls inside an unclosed quoted value is
        // part of the value, not a boundary; absorb it and keep scanning.
        if !at_eof && has_unclosed_quotes(raw) {
            i += if at_sep { sep_len } else { 1 };
            continue;
        }

        // A wholly blank physical line (or trailing blank text at the end of the
        // region) between rows is skipped rather than emitted as an empty cell. A
        // blank cell that follows a separator on a populated line is kept.
        let blank_skip = (at_nl || at_eof) && fields_in_row == 0 && raw.trim().is_empty();
        if !blank_skip {
            fields.push(make_csv_field(region, cell_start, i, warnings));
            fields_in_row += 1;
            if !first_row_done {
                first_row_len = fields_in_row;
            }
        }

        if at_eof {
            break;
        }

        if at_nl {
            // The newline ends the row. The first populated row fixes the implicit
            // column count.
            if fields_in_row > 0 {
                first_row_done = true;
            }
            fields_in_row = 0;
            cell_start = i + 1;
            i += 1;
        } else {
            cell_start = i + sep_len;
            i += sep_len;
        }
    }

    (fields, first_row_len)
}

/// Build a CSV/TSV [field](DataField) from the byte range `start..end`,
/// applying Asciidoctor's `close_cell` value processing.
///
/// The value is stripped of surrounding whitespace; then, if it is enclosed in
/// double quotes, the quotes are removed and the inner value is stripped again,
/// so the field's [content](DataField::content) span points at the actual value
/// (this matters for an AsciiDoc cell, which parses that span). Finally any run
/// of consecutive double quotes is collapsed to one (so an escaped `""` becomes
/// a single `"`). A value that is not enclosed (no leading quote, or trailing
/// characters after the closing quote) keeps its quotes and is only collapsed.
///
/// A lone double quote is an unclosed quoted value: it logs an error and the
/// cell is set to empty (matching Asciidoctor).
fn make_csv_field<'src>(
    region: Span<'src>,
    start: usize,
    end: usize,
    warnings: &mut Vec<Warning<'src>>,
) -> DataField<'src> {
    let trimmed = trim_surrounding_whitespace(region.slice(start..end));
    let data = trimmed.data();

    let content = if data == "\"" {
        warnings.push(Warning {
            source: trimmed,
            warning: WarningType::TableCsvDataHasUnclosedQuote,
        });
        trimmed.slice(0..0)
    } else if data.len() >= 2 && data.starts_with('"') && data.ends_with('"') {
        trim_surrounding_whitespace(trimmed.slice(1..data.len() - 1))
    } else {
        trimmed
    };

    let value = squeeze_quotes(content.data());
    let replacement = (value != content.data()).then_some(value);

    DataField {
        content,
        replacement,
    }
}

/// Collapse every run of consecutive double quotes to a single double quote,
/// matching Ruby's `String#squeeze('"')`.
///
/// Note: the `continue` intentionally leaves `prev_quote` set, so a run of
/// *N ≥ 2* consecutive `"` collapses to a single `"` (e.g. `""""` -> `"`), not
/// to pairs. This deliberately matches Asciidoctor rather than strict RFC 4180,
/// under which only `""` is a double-quote escape — don't "fix" it to a
/// two-character collapse without also changing Asciidoctor.
fn squeeze_quotes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut prev_quote = false;
    for c in text.chars() {
        if c == '"' {
            if prev_quote {
                continue;
            }
            prev_quote = true;
        } else {
            prev_quote = false;
        }
        out.push(c);
    }
    out
}

/// Determine whether `buffer` (the cell text accumulated so far) holds an
/// unclosed double quote, a direct port of Asciidoctor's
/// `Table::ParserContext#buffer_has_unclosed_quotes?`.
///
/// Only a value that begins with a double quote can be "quoted"; for any other
/// value embedded quotes are literal and this returns `false`. A leading quote
/// is unclosed until a matching trailing quote appears (accounting for escaped
/// `""` pairs).
///
/// Note: the escaped-pair collapse (`replace("\"\"", "")`) runs before the
/// start/end check, so `"""` collapses to a single `"` and is reported
/// *closed*. Strict RFC 4180 would read `"""` as an unclosed field (open quote
/// plus escaped `""` + missing close); this matches Asciidoctor's
/// `buffer_has_unclosed_quotes?` instead, so the divergence is intentional.
fn has_unclosed_quotes(buffer: &str) -> bool {
    let record = buffer.trim();

    if record == "\"" {
        return true;
    }

    if !record.starts_with('"') {
        return false;
    }

    let trailing_quote = record.ends_with('"');
    if (trailing_quote && record.ends_with("\"\"")) || record.starts_with("\"\"") {
        let collapsed = record.replace("\"\"", "");
        collapsed.starts_with('"') && !collapsed.ends_with('"')
    } else {
        !trailing_quote
    }
}

/// Parse a DSV region into its [fields](DataField), returning them in document
/// order together with the number of fields in the first row.
///
/// Each non-empty line is a row. Whitespace surrounding each value is stripped,
/// and the separator can be included in a value by escaping it with a single
/// backslash (`\:`). An enclosing character is not recognized.
fn parse_dsv_fields<'src>(region: Span<'src>, separator: &str) -> (Vec<DataField<'src>>, usize) {
    let data = region.data();
    let n = data.len();
    let sep_len = separator.len().max(1);
    let escaped = format!("\\{separator}");
    let at = |k: usize| data.as_bytes().get(k).copied();

    let mut fields: Vec<DataField<'src>> = vec![];
    let mut first_row_len = 0usize;
    let mut row_count = 0usize;
    let mut i = 0usize;

    while i < n {
        let mut line_end = i;
        while line_end < n && at(line_end) != Some(b'\n') {
            line_end += 1;
        }

        if data.get(i..line_end).unwrap_or("").trim().is_empty() {
            i = if line_end < n { line_end + 1 } else { line_end };
            continue;
        }

        let in_line = |pos: usize| {
            data.get(pos..line_end)
                .is_some_and(|s| s.starts_with(separator))
        };

        let mut fields_in_row = 0usize;
        let mut field_start = i;
        let mut p = i;

        while p < line_end {
            // A backslash that escapes the separator (`\:`) is not a boundary;
            // skip past both so the separator stays in the value.
            if at(p) == Some(b'\\')
                && data
                    .get(p + 1..line_end)
                    .is_some_and(|s| s.starts_with(separator))
            {
                p += 1 + sep_len;
                continue;
            }

            if in_line(p) {
                fields.push(make_dsv_field(region, field_start, p, &escaped, separator));
                fields_in_row += 1;
                p += sep_len;
                field_start = p;
                continue;
            }

            p += 1;
        }

        // The final field of the line runs to the line end.
        fields.push(make_dsv_field(
            region,
            field_start,
            line_end,
            &escaped,
            separator,
        ));
        fields_in_row += 1;

        if row_count == 0 {
            first_row_len = fields_in_row;
        }
        row_count += 1;

        i = if line_end < n { line_end + 1 } else { line_end };
    }

    (fields, first_row_len)
}

/// Build a DSV [field](DataField) from the byte range `start..end`, unescaping
/// any backslash-escaped separators (`escaped`, e.g. `\:`) into the bare
/// separator.
fn make_dsv_field<'src>(
    region: Span<'src>,
    start: usize,
    end: usize,
    escaped: &str,
    separator: &str,
) -> DataField<'src> {
    let trimmed = trim_surrounding_whitespace(region.slice(start..end));
    let replacement = if trimmed.data().contains(escaped) {
        Some(trimmed.data().replace(escaped, separator))
    } else {
        None
    };

    DataField {
        content: trimmed,
        replacement,
    }
}

/// Process a cell's content according to its [style](ColumnStyle), shared by
/// the PSV and data-format cell builders.
///
/// `trimmed` is the cell's content span with surrounding whitespace already
/// removed. `replacement` is the pre-filtered value (an escaped separator
/// unescaped, or a CSV/DSV value after quote/escape processing) when it differs
/// from `trimmed`; it is ignored for the [`AsciiDoc`](ColumnStyle::AsciiDoc)
/// style, which parses `trimmed` verbatim as a nested document. Every other
/// style produces inline [`Simple`](TableCellContent::Simple) content with the
/// verbatim substitution group for [`Literal`](ColumnStyle::Literal) and the
/// normal group otherwise.
fn process_content<'src>(
    trimmed: Span<'src>,
    replacement: Option<String>,
    style: ColumnStyle,
    parser: &mut Parser,
    warnings: &mut Vec<Warning<'src>>,
) -> TableCellContent<'src> {
    if style == ColumnStyle::AsciiDoc {
        // The AsciiDoc style effectively creates a nested, standalone AsciiDoc
        // document in the cell. It inherits the parent document's attributes, but
        // any attribute it defines is scoped to the cell and must not leak back
        // into the parent. Snapshot the attribute set before parsing and restore
        // it afterward to enforce that boundary (matching Asciidoctor, where a
        // `:foo:` set inside a cell is not visible after the table).
        let saved_attributes = parser.attribute_values.clone();

        // An attribute that is set in the parent document cannot be modified
        // inside the cell. Lock every inherited attribute that currently holds a
        // value for the duration of the cell (other than the handful of
        // exceptions the spec carves out), so a body assignment to one of them is
        // ignored. An attribute that is unset in the parent is not locked: the
        // cell may assign it (matching Asciidoctor, which here diverges from the
        // spec's "set or explicitly unset" wording). The lock set is saved and
        // restored so it applies only within the cell and nests correctly.
        // An attribute set in the parent is locked, as is one hard set or unset
        // through the API (its modification context is `ApiOnly`) even though it
        // is unset — matching Asciidoctor, where an API-controlled attribute can
        // never be overridden in a cell. An attribute merely unset in the parent
        // document is not locked, so the cell may assign it.
        //
        // The inherited attribute set is the shared built-in defaults with the
        // parent's per-parser entries (`saved_attributes`) layered on top, so
        // walk both, letting a per-parser entry shadow a like-named built-in.
        // The synthesized `backend-html5-doctype-*` and `safe-mode-*` flags need
        // no lock here: they are read-only intrinsics that reject a cell-body
        // assignment on their own (see `DERIVED_DOCTYPE_ATTR` /
        // `SAFE_MODE_ACTIVE_FLAG`), which a static lock could not do anyway once
        // the cell changes its own doctype.
        let saved_locks = parser.locked_attribute_names.clone();
        {
            let locks = &mut parser.locked_attribute_names;
            let mut maybe_lock = |name: &str, value: &AttributeValue| {
                let api_locked = value.modification_context == ModificationContext::ApiOnly;
                if (!matches!(value.value, InterpretedValue::Unset) || api_locked)
                    && !ASCIIDOC_CELL_MODIFIABLE_ATTRIBUTES.contains(&name)
                {
                    locks.insert(name.to_owned());
                }
            };
            for (name, value) in built_in_attrs_iter() {
                if !saved_attributes.contains_key(name) {
                    maybe_lock(name, value);
                }
            }
            for (name, value) in saved_attributes.iter() {
                maybe_lock(name, value);
            }
        }

        // The modifiable attributes may always be changed inside a cell, even
        // when the parent or the API set them with a restrictive modification
        // context. Materialize each into the per-parser map (a built-in such as
        // `toc` otherwise lives only in the shared table) with a relaxed context
        // for the duration of the cell so a body assignment is honored; the
        // snapshot restore reverts it afterward.
        for name in ASCIIDOC_CELL_MODIFIABLE_ATTRIBUTES {
            let attrs = Arc::make_mut(&mut parser.attribute_values);
            if let Some(mut attr) = attrs.get(*name).or_else(|| built_in_attr(name)).cloned() {
                attr.modification_context = ModificationContext::Anywhere;
                attrs.insert((*name).to_owned(), attr);
            }
        }

        // A cell does not inherit the parent's doctype; it resets to the default
        // (`article`). The cell body may still set its own doctype, and the
        // derived `backend-html5-doctype-*` attribute is refreshed to match.
        parser.force_doctype("article");

        // Likewise, a cell does not inherit the parent's `toc` setting: a nested
        // document starts without a table of contents and may enable its own.
        // Reset the value to unset; the relax loop above already made `toc`
        // modifiable inside the cell, so a cell-body `:toc:` is still honored.
        if let Some(toc) = Arc::make_mut(&mut parser.attribute_values).get_mut("toc") {
            toc.value = InterpretedValue::Unset;
        }

        // A cell whose content holds a preprocessor directive (an `include::`)
        // is parsed from an owned, expanded source the cell carries; every other
        // cell is parsed in place from the parent document's source, which keeps
        // its spans (and line numbers) and avoids a copy.
        let cell = if content_has_directive(trimmed.data()) {
            // `trimmed` indexes the document source only when this cell is not
            // itself being parsed from some *other* cell's owned (include-
            // expanded) source: an owned source is a private copy whose spans do
            // not index the document source map. A cell nested inside a borrowed
            // cell keeps document spans and so is still at "document level" here.
            //
            // KNOWN LIMITATION: when this cell *is* inside an owned source, its
            // directive lives in content that was expanded privately into that
            // owned copy and is absent from the document source, so no document
            // span maps to it. Such a directive is therefore attributed to the
            // root file and its warning is dropped (see below). Reporting its
            // true cursor needs a cursor representation that reaches into
            // owned-cell content; tracked in
            // https://github.com/asciidoc-rs/asciidoc-parser/issues/641.
            let at_document_level = parser.owned_cell_source_depth == 0;

            // The cell content is a contiguous slice of the (preprocessed)
            // document source, so it may itself have originated from an
            // `include::`d file. Look up the file and line the cell's first line
            // came from, so a directive that fails to resolve reports the correct
            // originating file (rather than "(root file)") and so its warning can
            // be re-anchored to that original cursor below.
            let cell_origin = if at_document_level {
                parser
                    .source_map
                    .clone()
                    .and_then(|sm| sm.original_file_and_line(trimmed.line()))
            } else {
                None
            };
            let cell_origin_file = cell_origin.as_ref().and_then(|sl| sl.0.clone());

            // Re-run the preprocessor over the cell content, naming the file it
            // came from so an unresolved directive is attributed to it.
            let (expanded, _cell_source_map, preprocessor_warnings) =
                preprocess_with_initial_file_name(
                    trimmed.data(),
                    parser,
                    cell_origin_file.as_deref(),
                );

            // The preprocessor locates each warning (e.g. an unresolved include
            // target) by byte offset into the expanded cell source, which is
            // owned by the cell and cannot escape it. When `trimmed` indexes the
            // document source, re-anchor each one to the cell's directive line
            // there so the warning's cursor maps back to the directive's true
            // (file, line) through the document source map. Only a directive on
            // the cell's *first* line reaches this inner preprocessor — a
            // directive at the start of any later line sits at document column 0
            // and is already expanded by the document-level preprocessor — so
            // every such warning belongs to that first line. When this cell is
            // itself inside an owned source, `trimmed` does not index the document
            // source, so the warnings cannot be re-anchored and are dropped (as
            // they were before this attribution existed); see the known
            // limitation above (issue #641).
            if at_document_level {
                let directive_line = trimmed.take_line().item;
                for pw in preprocessor_warnings {
                    warnings.push(Warning {
                        source: directive_line,
                        warning: pw.warning,
                    });
                }
            }

            let owned = OwnedCell::new(expanded, |source| {
                // Warnings from the owned parse borrow the owned source and so
                // cannot escape it; the include path is rare and currently
                // warning-free, so they are dropped here. The `debug_assert`
                // turns any future warning added to this path into a loud test
                // failure rather than a silent loss.
                let mut owned_warnings: Vec<Warning<'_>> = vec![];

                // Substitution warnings (e.g. `attribute-missing=warn`) recorded
                // while parsing this owned source carry offsets into it, not the
                // primary document source, so they too must be discarded.
                let substitution_warnings_mark = parser.substitution_warnings_len();

                // Mark that the blocks below are parsed from this cell's owned
                // (include-expanded) source, so a table nested within cannot
                // mis-map its spans against the document source map.
                parser.owned_cell_source_depth += 1;
                let (title, inline, toc, blocks, attributes) =
                    parse_asciidoc_cell_body(Span::new(source), parser, &mut owned_warnings);
                parser.owned_cell_source_depth -= 1;

                parser.truncate_substitution_warnings(substitution_warnings_mark);

                debug_assert!(
                    owned_warnings.is_empty(),
                    "warnings from an include-expanded AsciiDoc cell are dropped; \
                     propagate them before adding any to this path"
                );

                OwnedCellInner {
                    title,
                    inline,
                    toc,
                    blocks,
                    attributes,
                }
            });
            AsciiDocCell::Owned(Arc::new(owned))
        } else {
            let (title, inline, toc, blocks, attributes) =
                parse_asciidoc_cell_body(trimmed, parser, warnings);
            AsciiDocCell::Borrowed(BorrowedCell {
                title,
                inline,
                toc,
                blocks,
                attributes,
            })
        };

        parser.locked_attribute_names = saved_locks;
        parser.attribute_values = saved_attributes;
        TableCellContent::AsciiDoc(cell)
    } else {
        let mut content = match replacement {
            Some(replacement) => Content::from_filtered(trimmed, replacement),
            None => Content::from(trimmed),
        };

        let substitutions = if style == ColumnStyle::Literal {
            SubstitutionGroup::Verbatim
        } else {
            SubstitutionGroup::Normal
        };
        substitutions.apply(&mut content, parser, None);

        TableCellContent::Simple(content)
    }
}

/// Parses the body of an AsciiDoc table cell — a nested, standalone AsciiDoc
/// document — returning its (shown) title, whether its doctype is `inline`, its
/// table-of-contents configuration, its blocks, and a snapshot of the cell's
/// resolved attribute state.
///
/// A leading level-0 title line (`= Title`) is the nested document's title
/// rather than a section, so it is split off and rendered here (a level-0
/// heading is otherwise rejected in block parsing). The render-time decisions
/// (`inline`, and whether the title is shown) depend on the cell's now-mutated
/// attribute state, so they are resolved before the caller restores the
/// parent's attribute snapshot.
///
/// The attribute snapshot is likewise taken here, before that restore, so the
/// cell can be introspected as the nested document it is: it captures the
/// attributes the cell inherited from the parent (plus any the cell body set),
/// mirroring how a top-level [`Document`](crate::Document) retains its own
/// resolved attribute state.
fn parse_asciidoc_cell_body<'src>(
    content: Span<'src>,
    parser: &mut Parser,
    warnings: &mut Vec<Warning<'src>>,
) -> (
    Option<String>,
    bool,
    TocConfig,
    Vec<Block<'src>>,
    ResolvedAttributes,
) {
    let first_line = content.take_line();
    let (title_source, body) = if first_line.item.data().starts_with("= ") {
        (
            Some(first_line.item.discard(2).discard_whitespace()),
            first_line.after,
        )
    } else {
        (None, content)
    };

    // A nested document keeps its own footnote registry: footnotes defined
    // inside this cell must not be shared with (or numbered into the list of)
    // the enclosing document. We swap in a fresh, empty footnote list for the
    // duration of the cell parse and restore the parent's afterward, discarding
    // the cell's footnotes (see issue #544). The `footnote-number` counter is a
    // document-wide attribute and is deliberately *not* reset, so footnote
    // numbering continues across the cell as Asciidoctor does.
    let saved_footnotes = parser.take_footnotes();

    // Mark that we are inside an AsciiDoc cell (a nested document) for the
    // duration of the parse, so a table found within defaults its cell separator
    // to `!` rather than `|` (matching Asciidoctor's `Document#nested?`).
    parser.nested_document_depth += 1;
    let mut maw = parse_blocks_until(body, |_, _| false, parser);
    parser.nested_document_depth -= 1;
    warnings.append(&mut maw.warnings);

    parser.restore_footnotes(saved_footnotes);

    let inline = matches!(
        parser.attribute_value("doctype"),
        InterpretedValue::Value(ref v) if v == "inline"
    );

    let title = if parser.resolve_show_title(true) {
        title_source.map(|span| {
            let mut content = Content::from(span);
            SubstitutionGroup::Header.apply(&mut content, parser, None);
            content.rendered().to_string()
        })
    } else {
        None
    };

    // The cell is its own standalone document, so its table-of-contents
    // configuration comes from the cell's own `toc` family of attributes (which
    // it does not inherit from the parent). Resolve it here, before the caller
    // restores the parent's attribute snapshot.
    let toc = TocConfig::from_parser(parser);

    // Snapshot the cell's resolved attribute state while the parser still holds
    // it (the caller restores the parent's snapshot immediately after this
    // returns). The snapshot shares the parser's attribute tables by `Arc`, so
    // it is cheap. It lets a caller introspect the nested cell document —
    // including the attributes it inherited from the parent — the same way the
    // top-level `Document` exposes its own.
    let attributes = parser.snapshot_attributes();

    (title, inline, toc, maw.item.item, attributes)
}

/// Returns `true` when the cell content holds an `include::` preprocessor
/// directive at the start of a line, which must be expanded before the cell is
/// parsed.
fn content_has_directive(content: &str) -> bool {
    content.starts_with("include::") || content.contains("\ninclude::")
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
    source: Span<'src>,
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
    /// [`Content`](TableCellContent::Simple): escaped cell separators (the
    /// table's `separator` character preceded by a backslash, e.g. `\|`) are
    /// unescaped and substitutions are applied — the verbatim group for
    /// [`Literal`](ColumnStyle::Literal), the normal group otherwise. An
    /// [`AsciiDoc`](ColumnStyle::AsciiDoc) cell instead parses its content as a
    /// nested sequence of [blocks](TableCellContent::AsciiDoc).
    fn parse(
        raw: RawCell<'src>,
        column: &TableColumn,
        is_header: bool,
        separator: &str,
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

        let trimmed = trim_cell_content(raw.content, style);

        // An escaped cell separator (a backslash in front of the table's
        // separator, e.g. `\|` or `\!`) is unescaped to the bare separator. Only
        // the active separator is unescaped, so a `\|` in a `!`-separated table
        // is left untouched. The replacement is computed only for the inline
        // styles; an AsciiDoc cell parses its content verbatim (see
        // [`process_content`]).
        let escaped = format!("\\{separator}");
        let replacement = if style != ColumnStyle::AsciiDoc && trimmed.data().contains(&escaped) {
            Some(trimmed.data().replace(&escaped, separator))
        } else {
            None
        };

        let content = process_content(trimmed, replacement, style, parser, warnings);

        Self {
            h_align,
            v_align,
            style,
            colspan: raw.spec.colspan.max(1),
            rowspan: raw.spec.rowspan.max(1),
            content,
            // The cell's source begins at its content, immediately after the
            // separator (before any trimming), so the cell's reported line is
            // the separator's line.
            source: raw.content,
        }
    }

    /// Build a cell from a [data field](DataField) of a delimiter-separated
    /// table (CSV, TSV, or DSV).
    ///
    /// Unlike a PSV cell, a data cell carries no per-cell specifier: its
    /// alignment and [style](ColumnStyle) come entirely from the `column`, and
    /// it always spans a single row and column. The separator escaping is
    /// handled by the format parser before this point, so the field already
    /// holds the extracted value (its [`replacement`](DataField::replacement),
    /// when present, is the value after quote/escape processing). A header cell
    /// (`is_header`) is processed as plain header content.
    fn parse_data(
        field: DataField<'src>,
        column: &TableColumn,
        is_header: bool,
        parser: &mut Parser,
        warnings: &mut Vec<Warning<'src>>,
    ) -> Self {
        let style = if is_header {
            ColumnStyle::Default
        } else {
            column.style
        };

        let source = field.content;
        let content = process_content(field.content, field.replacement, style, parser, warnings);

        Self {
            h_align: column.h_align,
            v_align: column.v_align,
            style,
            colspan: 1,
            rowspan: 1,
            content,
            source,
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
            TableCellContent::AsciiDoc(cell) => {
                cell.resolve_references(resolver, renderer, warnings);
            }
        }
    }
}

impl<'src> HasSpan<'src> for TableCell<'src> {
    /// Returns the cell's source span, which begins at the cell's content
    /// immediately after its separator. Its [line](Span::line) is therefore the
    /// line on which the cell starts.
    fn span(&self) -> Span<'src> {
        self.source
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
    AsciiDoc(AsciiDocCell<'src>),
}

/// The content of an [`AsciiDoc`](TableCellContent::AsciiDoc) table cell: a
/// nested, standalone AsciiDoc document.
///
/// Because the cell behaves like its own document, a few render-time decisions
/// depend on attribute state that is scoped to the cell and gone by the time
/// the document is rendered. They are therefore resolved while the cell is
/// parsed and captured here: whether the cell's nested document title is shown
/// (and its rendered text), and whether the cell's `doctype` is `inline` (in
/// which case a lone paragraph renders without the usual block wrapper).
///
/// A cell whose content has no preprocessor directives is parsed in place from
/// the parent document's source ([`Borrowed`](Self::Borrowed)). A cell that
/// expands an `include::` directive owns its preprocessed source
/// ([`Owned`](Self::Owned)); the owned store is shared behind an [`Arc`] so the
/// cell stays cheaply cloneable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AsciiDocCell<'src> {
    /// Parsed in place from the parent document's source.
    Borrowed(BorrowedCell<'src>),

    /// Parsed from an owned, include-expanded source the cell carries.
    Owned(Arc<OwnedCell>),
}

impl<'src> AsciiDocCell<'src> {
    /// Returns the cell's nested-document title, rendered to its display text.
    ///
    /// This is `Some` only when the cell began with a level-0 title line
    /// (`= Title`) *and* the cell's effective `showtitle`/`notitle` state means
    /// that title is shown; otherwise it is `None`.
    pub fn title(&self) -> Option<&str> {
        match self {
            Self::Borrowed(cell) => cell.title.as_deref(),
            Self::Owned(cell) => cell.borrow_dependent().title.as_deref(),
        }
    }

    /// Returns `true` when the cell's `doctype` resolves to `inline`.
    ///
    /// An `inline` document renders a lone paragraph as bare inline content,
    /// without the enclosing block wrapper.
    pub fn is_inline(&self) -> bool {
        match self {
            Self::Borrowed(cell) => cell.inline,
            Self::Owned(cell) => cell.borrow_dependent().inline,
        }
    }

    /// Returns where (and whether) the cell's table of contents is generated.
    ///
    /// The cell is a standalone nested document, so this is resolved from the
    /// cell's own `toc` attribute and is independent of the parent document's
    /// setting.
    pub fn toc_mode(&self) -> TocMode {
        self.toc().mode
    }

    /// Returns the depth of section levels included in the cell's table of
    /// contents, resolved from the cell's own `toclevels` attribute (default
    /// `2`).
    pub fn toc_levels(&self) -> usize {
        self.toc().levels
    }

    /// Returns the title of the cell's table of contents, resolved from the
    /// cell's own `toc-title` attribute (default _Table of Contents_).
    pub fn toc_title(&self) -> &str {
        &self.toc().title
    }

    /// Returns the CSS class applied to the cell's table-of-contents container,
    /// resolved from the cell's own `toc-class` attribute (default `toc`).
    pub fn toc_class(&self) -> &str {
        &self.toc().class
    }

    /// Returns the resolved table-of-contents configuration for the cell.
    pub(crate) fn toc(&self) -> &TocConfig {
        match self {
            Self::Borrowed(cell) => &cell.toc,
            Self::Owned(cell) => &cell.borrow_dependent().toc,
        }
    }

    /// Returns the blocks parsed from the cell's content.
    pub fn blocks(&self) -> &[Block<'_>] {
        match self {
            Self::Borrowed(cell) => &cell.blocks,
            Self::Owned(cell) => &cell.borrow_dependent().blocks,
        }
    }

    /// Returns `true` because an AsciiDoc table cell is always a nested,
    /// standalone document.
    ///
    /// This mirrors Asciidoctor's `Document#nested?`, which is `true` for the
    /// document parsed from an AsciiDoc (`a`) cell and `false` for a top-level
    /// document. It is provided so a caller that has navigated to the cell can
    /// confirm it is introspecting a nested document (see also
    /// [`attribute_value`](Self::attribute_value) and its siblings, which
    /// expose the attributes the cell inherited from its parent).
    pub fn is_nested(&self) -> bool {
        true
    }

    /// Returns the resolved interpreted value of the named document attribute
    /// as the cell's nested document saw it.
    ///
    /// The cell inherits the parent document's attributes, so this reports an
    /// inherited value (such as a directory option the parent was configured
    /// with) as well as any attribute the cell body set for itself. It mirrors
    /// [`Document::attribute_value`](crate::Document::attribute_value) exactly,
    /// resolving the cell's introspectable attribute state the same way the
    /// top-level document resolves its own.
    pub fn attribute_value<N: AsRef<str>>(&self, name: N) -> InterpretedValue {
        self.attributes().attribute_value(name)
    }

    /// Returns `true` if the cell's nested document has a document attribute by
    /// this name (whether or not it is set).
    ///
    /// Mirrors [`Document::has_attribute`](crate::Document::has_attribute).
    pub fn has_attribute<N: AsRef<str>>(&self, name: N) -> bool {
        self.attributes().has_attribute(name)
    }

    /// Returns `true` if the cell's nested document has a document attribute by
    /// this name and it is set (i.e. not unset).
    ///
    /// Mirrors [`Document::is_attribute_set`](crate::Document::is_attribute_set).
    pub fn is_attribute_set<N: AsRef<str>>(&self, name: N) -> bool {
        self.attributes().is_attribute_set(name)
    }

    /// Returns the snapshot of the cell's resolved attribute state.
    fn attributes(&self) -> &ResolvedAttributes {
        match self {
            Self::Borrowed(cell) => &cell.attributes,
            Self::Owned(cell) => &cell.borrow_dependent().attributes,
        }
    }

    /// Resolves any deferred cross-references in the cell's blocks.
    fn resolve_references(
        &mut self,
        resolver: &dyn ReferenceResolver,
        renderer: &dyn InlineSubstitutionRenderer,
        warnings: &mut Vec<ReferenceWarning>,
    ) {
        match self {
            Self::Borrowed(cell) => {
                for block in &mut cell.blocks {
                    block.resolve_references(resolver, renderer, warnings);
                }
            }

            // The owned store is shared behind an `Arc`, but references are
            // resolved immediately after parsing while the cell is still its sole
            // owner, so `get_mut` succeeds.
            Self::Owned(cell) => {
                if let Some(cell) = Arc::get_mut(cell) {
                    cell.with_dependent_mut(|_, dependent| {
                        for block in &mut dependent.blocks {
                            block.resolve_references(resolver, renderer, warnings);
                        }
                    });
                }
            }
        }
    }
}

/// An [`AsciiDoc`](TableCellContent::AsciiDoc) cell parsed in place from the
/// parent document's source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BorrowedCell<'src> {
    title: Option<String>,
    inline: bool,
    toc: TocConfig,
    blocks: Vec<Block<'src>>,
    attributes: ResolvedAttributes,
}

self_cell! {
    /// An [`AsciiDoc`](TableCellContent::AsciiDoc) cell that owns its
    /// (include-expanded) source, with the parsed blocks borrowing from it.
    pub struct OwnedCell {
        owner: String,

        #[covariant]
        dependent: OwnedCellInner,
    }

    impl {Debug, Eq, PartialEq}
}

/// The parsed contents of an [`OwnedCell`], borrowing its owned source.
#[derive(Debug, Eq, PartialEq)]
struct OwnedCellInner<'src> {
    title: Option<String>,
    inline: bool,
    toc: TocConfig,
    blocks: Vec<Block<'src>>,
    attributes: ResolvedAttributes,
}

/// Parse the value of the `cols` attribute into a list of columns, mirroring
/// Asciidoctor's `parse_colspecs`.
///
/// All spaces are first removed from the value. A wholly blank value yields no
/// columns (the caller then takes the column count from the first row), and a
/// lone integer (the deprecated `cols="3"` form) yields that many default
/// columns. Otherwise the value is a list of column specifiers separated by
/// commas, or by semicolons when no comma is present. An empty record (e.g. the
/// trailing field of `cols="1,,1"`) contributes a default column, and a
/// specifier may be preceded by a multiplier (`<n>*`) that repeats the column
/// `n` times. Each specifier's alignment operators, proportional width, and
/// [style operator](parse_col_spec) are interpreted.
fn parse_cols(value: &str) -> Vec<TableColumn> {
    // Asciidoctor strips every space from the cols value before parsing, so
    // `cols=" 1, 1 "` is equivalent to `cols="1,1"`.
    let records: String = value.chars().filter(|c| !c.is_whitespace()).collect();

    // A wholly blank cols value is ignored: the caller falls back to the column
    // count of the first row.
    if records.is_empty() {
        return vec![];
    }

    // Deprecated single-integer form: `cols=3` is equivalent to `cols="3*"` and
    // produces that many equally sized columns.
    if let Ok(count) = records.parse::<usize>() {
        return vec![TableColumn::default(); count];
    }

    // Split on commas when present, otherwise on semicolons (Asciidoctor accepts
    // either as the column-spec separator, but not a mix). Empty records are
    // kept: each one contributes a default column.
    let parts: Vec<&str> = if records.contains(',') {
        records.split(',').collect()
    } else {
        records.split(';').collect()
    };

    let mut columns: Vec<TableColumn> = vec![];
    for part in parts {
        if part.is_empty() {
            columns.push(TableColumn::default());
        } else if let Some((count, spec)) = part.split_once('*') {
            let repeat = count.parse::<usize>().unwrap_or(1).max(1);
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
/// Every unescaped occurrence of the table's `separator` (the vertical bar
/// (`|`) by default, the exclamation mark (`!`) for a nested table, or any
/// string set with the `separator` attribute, e.g. the broken bar `¦`) is a
/// cell boundary, matching Asciidoctor. The token immediately preceding a
/// separator is treated as that cell's [specifier](CellSpec) (e.g. `^`, `2+`,
/// `.>`) only when it parses as one (see [`parse_cell_spec`]) *and* is anchored
/// at the line start or preceded by whitespace; otherwise the token is ordinary
/// content of the preceding cell and the separator is a plain boundary (so the
/// `a` in `|a|b` is content, not a style operator). Content before the first
/// boundary is ignored.
///
/// A separator immediately preceded by a backslash (e.g. `\|`) is escaped: it
/// is literal content rather than a boundary, and the backslash is stripped
/// later in [`TableCell::parse`]. Only the single byte before the separator is
/// inspected, so `\\|` is also read as an escaped separator — matching
/// Asciidoctor, whose check is likewise the single-character
/// `pre_match.end_with? '\'`.
fn scan_cells<'src>(
    region: Span<'src>,
    separator: &str,
) -> (Vec<RawCell<'src>>, Option<Span<'src>>) {
    let data = region.data();
    let bytes = data.as_bytes();
    let len = bytes.len();

    // A zero-length separator would never advance; treat it as a single byte to
    // stay safe. (The resolver never produces an empty separator.)
    let sep_len = separator.len().max(1);

    let mut cells: Vec<RawCell<'src>> = vec![];

    // The content start and specifier of the cell currently being accumulated.
    let mut content_start: Option<usize> = None;

    let mut cur_spec = CellSpec::default();

    // The span of a cell recovered from content that precedes the first
    // separator (see below); `Some` drives a missing-leading-separator warning.
    let mut recovered: Option<Span<'src>> = None;

    let mut i = 0;
    while i < len {
        if data
            .get(i..)
            .is_some_and(|rest| rest.starts_with(separator))
        {
            // A separator immediately preceded by a backslash is escaped: it is
            // literal content, not a cell boundary. The backslash is stripped
            // from the rendered cell later (see `TableCell::parse`).
            if i > 0 && bytes.get(i - 1).copied() == Some(b'\\') {
                i += sep_len;
                continue;
            }

            // Walk back to the start of the token directly preceding this
            // separator. The token (a possible cell specifier) runs back to the
            // previous whitespace, tab, or newline, or to the start of the
            // region.
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

            // Every unescaped separator is a cell boundary (matching
            // Asciidoctor). When the token is empty or a valid specifier it
            // belongs to the *next* cell, so the previous cell's content ends
            // before the token. Otherwise the token is ordinary content of the
            // previous cell (e.g. the `a` in `|a|b`, where `a` is not preceded
            // by whitespace and so is not a specifier), the separator is plain,
            // and the next cell takes the default specifier.
            let (content_end, next_spec) = match spec {
                Some(spec) => (tok_start, spec),
                None => (i, CellSpec::default()),
            };

            match content_start {
                Some(start) => {
                    // The separating whitespace, included in the slice, is
                    // trimmed later in `TableCell::parse`.
                    cells.push(RawCell {
                        spec: cur_spec,
                        content: region.slice(start..content_end),
                    });
                }

                None => {
                    // No cell has been opened yet, so this is the table's first
                    // separator. Non-blank content in front of it means the first
                    // cell is missing its leading separator; recover that content
                    // as the first cell (with the default specifier) and record
                    // its span so the caller can warn, matching Asciidoctor.
                    let leading = region.slice(0..content_end);
                    if !leading.data().trim().is_empty() {
                        cells.push(RawCell {
                            spec: CellSpec::default(),
                            content: leading,
                        });
                        recovered = Some(leading);
                    }
                }
            }

            cur_spec = next_spec;
            content_start = Some(i + sep_len);
            i += sep_len;
            continue;
        }

        i += 1;
    }

    if let Some(start) = content_start {
        cells.push(RawCell {
            spec: cur_spec,
            content: region.slice(start..len),
        });
    }

    (cells, recovered)
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

/// Trim a PSV cell's content according to its [style](ColumnStyle), matching
/// Asciidoctor's `Table::Cell` initializer:
///
/// * A [`Literal`](ColumnStyle::Literal) cell has its trailing whitespace
///   removed and any leading blank lines stripped, but the leading indentation
///   of its first content line is preserved (so an indented literal cell keeps
///   its indentation).
/// * An [`AsciiDoc`](ColumnStyle::AsciiDoc) cell likewise removes trailing
///   whitespace; if the remaining content begins with a newline it strips the
///   leading blank lines (preserving the first content line's indentation, so a
///   leading-indented line is interpreted as a literal block), otherwise it
///   strips the leading whitespace.
/// * Every other style has all surrounding whitespace removed.
fn trim_cell_content(s: Span<'_>, style: ColumnStyle) -> Span<'_> {
    let data = s.data();
    match style {
        ColumnStyle::Literal => {
            let end = data.trim_end().len();
            let mut start = 0;
            while data[start..end].starts_with('\n') {
                start += 1;
            }
            s.slice(start..end)
        }

        ColumnStyle::AsciiDoc => {
            let end = data.trim_end().len();
            if data[..end].starts_with('\n') {
                let mut start = 0;
                while data[start..end].starts_with('\n') {
                    start += 1;
                }
                s.slice(start..end)
            } else {
                let start = end - data[..end].trim_start().len();
                s.slice(start..end)
            }
        }

        _ => trim_surrounding_whitespace(s),
    }
}

/// Returns the first non-blank line in `rest`, or `None` when every remaining
/// line is blank (or `rest` is empty).
fn first_nonblank_line(mut rest: Span<'_>) -> Option<Span<'_>> {
    while !rest.is_empty() {
        let line = rest.take_line();
        if !line.item.data().trim().is_empty() {
            return Some(line.item);
        }
        rest = line.after;
    }
    None
}

/// Returns `true` when `line` begins a new PSV cell, i.e. it contains the
/// separator and the text before the first separator (after any leading
/// whitespace) is either empty or a valid cell specifier. A line that continues
/// the previous cell returns `false`.
fn psv_line_starts_cell(line: &str, separator: &str) -> bool {
    match line.find(separator) {
        Some(pos) => {
            let prefix = line[..pos].trim_start();
            prefix.is_empty() || parse_cell_spec(prefix).is_some()
        }
        None => false,
    }
}

/// Returns `true` when `line` contains an odd number of double quotes, i.e. it
/// opens a quoted CSV/TSV value that is not closed on the same line.
fn line_has_unclosed_quote(line: &str) -> bool {
    line.bytes().filter(|&b| b == b'"').count() % 2 == 1
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{AsciiDocCell, OwnedCell, OwnedCellInner, ResolvedAttributes, TocConfig};
    use crate::parser::{
        HtmlSubstitutionRenderer, ReferenceResolver, ResolutionContext, ResolvedReference,
    };

    /// A resolver that resolves nothing; the owned-cell resolution path under
    /// test carries no references, so it is never actually consulted.
    struct NoopResolver;

    impl ReferenceResolver for NoopResolver {
        fn resolve(&self, _context: &ResolutionContext<'_>) -> Option<ResolvedReference> {
            None
        }
    }

    /// When an owned (include-expanded) AsciiDoc cell is shared behind more
    /// than one `Arc` reference, `resolve_references` cannot obtain a
    /// mutable borrow of the store and leaves it untouched rather than
    /// panicking. Production code resolves while the cell is its sole
    /// owner, so this defensive branch is exercised here by deliberately
    /// holding a second reference.
    #[test]
    fn resolve_references_skips_shared_owned_cell() {
        let mut cell = AsciiDocCell::Owned(Arc::new(OwnedCell::new(String::new(), |_source| {
            OwnedCellInner {
                title: None,
                inline: false,
                toc: TocConfig::disabled(),
                blocks: vec![],
                attributes: ResolvedAttributes::default(),
            }
        })));

        // Hold a second reference to the same store so `Arc::get_mut` fails.
        let shared = cell.clone();

        let mut warnings = vec![];
        cell.resolve_references(&NoopResolver, &HtmlSubstitutionRenderer {}, &mut warnings);

        // Resolution was skipped silently: no warnings, and the two references
        // still describe the same (unmodified) cell.
        assert!(warnings.is_empty());
        assert_eq!(cell, shared);
    }

    mod unresolved_directive_in_asciidoc_cell {
        #![allow(clippy::indexing_slicing)]

        use crate::{
            parser::SourceLine,
            tests::prelude::{inline_file_handler::InlineFileHandler, *},
        };

        // The faithful port of Ruby Asciidoctor `tables_test.rb` 1728 (an
        // unresolved directive in a cell reached via an outer `include::`) lives
        // in `tests::asciidoctor_rb::tables_test`. These are additional
        // regression tests for the same fix, kept next to the code under test.

        // The table is in the primary document itself, so the unresolved
        // directive is attributed to the root file (not an included one).
        #[test]
        fn root_document_cell_reports_root_cursor() {
            // No include handler: `does-not-exist.adoc` cannot be resolved.
            let doc = Parser::default()
                .with_safe_mode(SafeMode::Server)
                .parse("|===\na|include::does-not-exist.adoc[]\n|===");

            assert_rendered_contains(&doc, "Unresolved directive in (root file)");

            let warnings: Vec<_> = doc.warnings().collect();
            assert_eq!(warnings.len(), 1);
            assert_eq!(
                warnings[0].warning,
                WarningType::IncludeFileNotFound("does-not-exist.adoc".to_string())
            );

            // The directive is on line 2 of the primary document.
            assert_eq!(
                doc.source_map()
                    .original_file_and_line(warnings[0].source.line()),
                Some(SourceLine(None, 2))
            );
        }

        // A table nested inside a *borrowed* AsciiDoc cell (one whose own content
        // is not include-expanded) is still parsed in place from the document
        // source, so an unresolved directive in the inner cell maps through the
        // document source map like any other. Here the whole document is the root
        // file, so the cursor is the root file at the inner directive's line.
        #[test]
        fn nested_table_cell_maps_through_document_source() {
            let doc = Parser::default()
                .with_safe_mode(SafeMode::Server)
                .parse("|===\na|\n!===\na!include::does-not-exist.adoc[]\n!===\n|===");

            assert_rendered_contains(&doc, "Unresolved directive in (root file)");

            let warnings: Vec<_> = doc.warnings().collect();
            assert_eq!(warnings.len(), 1);
            assert_eq!(
                warnings[0].warning,
                WarningType::IncludeFileNotFound("does-not-exist.adoc".to_string())
            );

            // The inner directive is on line 4 of the primary document.
            assert_eq!(
                doc.source_map()
                    .original_file_and_line(warnings[0].source.line()),
                Some(SourceLine(None, 4))
            );
        }

        // Greptile #639: a table nested inside a (borrowed) cell of an *included*
        // file must attribute an inner unresolved directive to that included
        // file, not the root file.
        #[test]
        fn nested_table_cell_in_included_file_reports_include_cursor() {
            let handler = InlineFileHandler::from_pairs([(
                "outer.adoc",
                "|===\na|\n!===\na!include::does-not-exist.adoc[]\n!===\n|===",
            )]);
            let doc = Parser::default()
                .with_safe_mode(SafeMode::Server)
                .with_include_file_handler(handler)
                .parse("include::outer.adoc[]");

            assert_rendered_contains(&doc, "Unresolved directive in outer.adoc");

            let warnings: Vec<_> = doc.warnings().collect();
            assert_eq!(warnings.len(), 1);
            assert_eq!(
                warnings[0].warning,
                WarningType::IncludeFileNotFound("does-not-exist.adoc".to_string())
            );

            // The inner directive is on line 4 of `outer.adoc`.
            assert_eq!(
                doc.source_map()
                    .original_file_and_line(warnings[0].source.line()),
                Some(SourceLine(Some("outer.adoc".to_string()), 4))
            );
        }

        // A table nested inside an *owned* (include-expanded) cell is parsed from
        // that cell's private source, whose spans do not index the document
        // source map. An unresolved directive in the inner cell therefore cannot
        // be re-anchored: it is rendered (attributed to the root file) but its
        // warning is dropped rather than mis-mapped. Reporting its true cursor is
        // tracked as a known limitation in
        // https://github.com/asciidoc-rs/asciidoc-parser/issues/641.
        #[test]
        fn unresolved_directive_inside_owned_cell_source_is_dropped() {
            // `cell.adoc` is pulled in as the top cell's owned source; it holds a
            // nested table (so its cells use the `!` separator) whose own cell has
            // an unresolvable include.
            let handler = InlineFileHandler::from_pairs([(
                "cell.adoc",
                "!===\na!include::does-not-exist.adoc[]\n!===",
            )]);
            let doc = Parser::default()
                .with_safe_mode(SafeMode::Server)
                .with_include_file_handler(handler)
                .parse("|===\na|include::cell.adoc[]\n|===");

            // The inner directive is still expanded into an "Unresolved
            // directive" message, so the reader sees the problem in the output.
            assert_rendered_contains(&doc, "Unresolved directive in (root file)");

            // But its warning cannot be re-anchored to the document source, so it
            // is dropped rather than reported against a bogus cursor.
            assert_eq!(doc.warnings().count(), 0);
        }
    }

    // Cataloging a leading anchor found in a table cell (issue #543) is covered
    // for header and default-style cells by the ported tests in
    // `tests::asciidoctor_rb::tables_test`. Those styled-column fixtures place
    // the anchor in the first (header) row, and a header cell is always parsed
    // with the default column style — so `cols=1a` never actually parses the
    // anchored value as an AsciiDoc-style cell there. This exercises that
    // missing case directly: a leading anchor in an AsciiDoc-style *body* cell
    // must still be cataloged in the main document.
    mod anchor_in_asciidoc_body_cell {
        use crate::tests::prelude::*;

        #[test]
        fn leading_anchor_in_asciidoc_body_cell_is_cataloged() {
            // Two `|` rows with no blank line between them defeat the implicit-
            // header heuristic (which requires a blank line after the first row),
            // so both cells are AsciiDoc-style *body* cells rather than a header.
            let doc = Parser::default()
                .parse("[cols=1a]\n|===\n|[[foo,Foo]]body anchor\n|second cell\n|===");

            // Guard the premise: the anchored cell is a genuine AsciiDoc-style
            // body cell (each `a` cell renders its content as a nested document in
            // `div.content`), not a header cell — no `th` is produced, and the
            // anchor renders as a target inside the cell.
            assert_css(&doc, "th", 0);
            assert_css(&doc, "table.tableblock td.tableblock > div.content", 2);
            assert_xpath(&doc, "//td//div[@class=\"content\"]//a[@id=\"foo\"]", 1);

            // The leading anchor is cataloged in the main document's catalog.
            assert!(doc.catalog().contains_id("foo"));
        }
    }
}
