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
/// * Cell specifiers (spans, duplication, per-cell alignment and style); the
///   per-cell style operator that overrides a column's style operator is not
///   yet recognized.
///
/// Column specifier style operators (the `a`, `d`, `e`, `h`, `l`, `m`, and `s`
/// operators) are supported, along with proportional width and the horizontal
/// and vertical alignment operators.
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
        // rows of `ncols` cells each.
        //
        // Each cell is processed according to the style of the column it falls
        // in. The header row (when present) is the first `ncols` cells and is
        // always processed as plain header content, regardless of the column
        // styles, so that a style operator doesn't affect the header row.
        let mut warnings: Vec<Warning<'src>> = vec![];
        let raw_cells = scan_cells(inside);
        let header_len = if has_header { ncols } else { 0 };

        let mut cells = Vec::with_capacity(raw_cells.len());
        for (idx, raw) in raw_cells.into_iter().enumerate() {
            let style = if idx < header_len {
                ColumnStyle::Default
            } else {
                // `idx % ncols` is in bounds whenever `ncols > 0`; an empty table
                // (`ncols == 0`) has no columns, so the cell falls back to the
                // default style.
                columns
                    .get(idx % ncols.max(1))
                    .map_or(ColumnStyle::Default, |column| column.style)
            };
            cells.push(TableCell::parse(raw, style, parser, &mut warnings));
        }
        let mut cells = cells.into_iter();

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
    h_align: HorizontalAlignment,
    v_align: VerticalAlignment,
    style: ColumnStyle,
}

impl TableColumn {
    /// Returns the proportional width of this column relative to the other
    /// columns in the table.
    pub fn width(&self) -> usize {
        self.width
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
    content: TableCellContent<'src>,
}

impl<'src> TableCell<'src> {
    /// Build a cell from the raw (untrimmed) span of its content, processing it
    /// according to the [style](ColumnStyle) of the column the cell belongs to.
    ///
    /// Leading and trailing whitespace is always stripped. For every style but
    /// [`AsciiDoc`](ColumnStyle::AsciiDoc) the cell holds inline
    /// [`Content`](TableCellContent::Simple): escaped cell separators (`\|`)
    /// are unescaped and substitutions are applied — the verbatim group for
    /// [`Literal`](ColumnStyle::Literal), the normal group otherwise. An
    /// [`AsciiDoc`](ColumnStyle::AsciiDoc) cell instead parses its content as a
    /// nested sequence of [blocks](TableCellContent::AsciiDoc).
    fn parse(
        raw: Span<'src>,
        style: ColumnStyle,
        parser: &mut Parser,
        warnings: &mut Vec<Warning<'src>>,
    ) -> Self {
        let trimmed = trim_surrounding_whitespace(raw);

        if style == ColumnStyle::AsciiDoc {
            // The AsciiDoc style effectively creates a nested, standalone
            // AsciiDoc document in the cell. It inherits the parent document's
            // attributes, but any attribute it defines is scoped to the cell and
            // must not leak back into the parent. Snapshot the attribute set
            // before parsing and restore it afterward to enforce that boundary
            // (matching Asciidoctor, where a `:foo:` set inside a cell is not
            // visible after the table).
            let saved_attributes = parser.attribute_values.clone();
            let mut maw = parse_blocks_until(trimmed, |_| false, parser);
            parser.attribute_values = saved_attributes;
            warnings.append(&mut maw.warnings);
            return Self {
                content: TableCellContent::AsciiDoc(maw.item.item),
            };
        }

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

        Self {
            content: TableCellContent::Simple(content),
        }
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
/// The width is the first contiguous run of digits after any alignment
/// operators; a spec with no digits falls back to the default width. The style
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

    // Width is the first run of digits after the alignment operators.
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    let width = match digits.parse::<usize>() {
        Ok(width) if width > 0 => width,
        _ => TableColumn::default().width,
    };
    rest = &rest[digits.len()..];

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
        h_align,
        v_align,
        style,
    }
}

/// Scan a region for PSV cell boundaries, returning the raw (untrimmed) span of
/// each cell's content.
///
/// A cell boundary is a vertical bar (`|`) that appears at the start of a line
/// or is preceded by whitespace. Content before the first boundary is ignored.
///
/// An escaped separator (`\|`) is preceded by a backslash — neither a line
/// start nor whitespace — so it already fails the boundary test and needs no
/// special handling here; the backslash is stripped later in
/// [`TableCell::parse`].
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

            if at_line_start || after_space {
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
