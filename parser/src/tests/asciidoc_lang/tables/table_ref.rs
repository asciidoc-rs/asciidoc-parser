use crate::tests::prelude::*;

track_file!("docs/modules/tables/pages/table-ref.adoc");

use crate::blocks::{DataFormat, TableCellContent};

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

/// Return the rendered text of a [`Simple`](TableCellContent::Simple) cell,
/// panicking if the cell holds AsciiDoc block content.
fn simple_text(cell: &crate::blocks::TableCell<'_>) -> String {
    match cell.content() {
        TableCellContent::Simple(content) => content.rendered().to_string(),
        TableCellContent::AsciiDoc(_) => panic!("expected simple cell content"),
    }
}

/// Return the single nested [`TableBlock`] held by an AsciiDoc `cell`.
///
/// [`TableBlock`]: crate::blocks::TableBlock
fn nested_table<'a, 'src>(
    cell: &'a crate::blocks::TableCell<'src>,
) -> &'a crate::blocks::TableBlock<'src> {
    match cell.content() {
        TableCellContent::AsciiDoc(cell) => {
            let blocks = cell.blocks();
            let tables: Vec<&crate::blocks::TableBlock<'src>> = blocks
                .iter()
                .filter_map(|block| match block {
                    crate::blocks::Block::Table(table) => Some(table),
                    _ => None,
                })
                .collect();
            assert_eq!(tables.len(), 1, "expected exactly one nested table");
            tables[0]
        }
        other => panic!("expected an AsciiDoc cell, got {other:?}"),
    }
}

// This page is a single reference table; each of its rows describes one
// table-level attribute. The intro (page title, `cols` spec, table delimiter,
// and header row) carries no rule to verify.
non_normative!(
    r#"
= Table Syntax and Attribute Reference
:navtitle: Table Reference

[cols="1m,2,1m,2,2"]
|===
|Attribute |Description |Value |Description |Notes

"#
);

#[test]
fn caption_attribute() {
    verifies!(
        r#"
|caption
|defines the title label on a single table
d|user-defined
|
|

"#
    );

    // An explicit `caption` defines a user-defined title label on the single
    // table it is set on, used verbatim (including its trailing space) and with
    // no automatically incremented number.
    let table = parse_table("[caption=\"Spec Table. \"]\n.A titled table\n|===\n|a\n|===");
    assert_eq!(table.caption(), Some("Spec Table. "));
}

#[test]
fn cols_attribute() {
    verifies!(
        r#"
|cols
|comma-separated list of column specifiers
d|specifiers
|Specifies the number of columns and the distribution ratio and default formatting for each column. See xref:add-columns.adoc[] for details
|

"#
    );

    // A comma-separated list of column specifiers sets the number of columns, the
    // per-column distribution ratio, and the default formatting for each column.
    // The page's own spec, `1m,2,1m,2,2`, declares five columns: the first and
    // third are width 1 with the monospace style, the rest are width 2 with the
    // default style.
    let table = parse_table("[cols=\"1m,2,1m,2,2\"]\n|===\n|a |b |c |d |e\n|===");
    assert_eq!(table.columns().len(), 5);

    assert_eq!(table.columns()[0].width(), 1);
    assert_eq!(table.columns()[0].style(), ColumnStyle::Monospace);
    assert_eq!(table.columns()[1].width(), 2);
    assert_eq!(table.columns()[1].style(), ColumnStyle::Default);
    assert_eq!(table.columns()[2].width(), 1);
    assert_eq!(table.columns()[2].style(), ColumnStyle::Monospace);
    assert_eq!(table.columns()[3].width(), 2);
    assert_eq!(table.columns()[3].style(), ColumnStyle::Default);
    assert_eq!(table.columns()[4].width(), 2);
    assert_eq!(table.columns()[4].style(), ColumnStyle::Default);
}

#[test]
fn format_attribute() {
    verifies!(
        r#"
.4+|format
.4+|data format of the table's contents
|psv
|cells are delimited by `separator` (default `{vbar}`) (aka prefix-separated values)
.4+|

|dsv
|cells are delimited by a colon (`:`) (aka delimiter-separated values)

|csv
|cells are delimited by a comma (`,`) (aka comma-separated values)

|tsv
|cells are delimited by a tab character (aka tab-separated values)

"#
    );

    // The `format` attribute selects the data format. PSV is the default, and its
    // cells are delimited by the `separator` (default `|`), placed in front of
    // each cell.
    assert_eq!(
        parse_table("|===\n|a |b\n|===").data_format(),
        DataFormat::Psv
    );
    assert_eq!(
        parse_table("[format=psv]\n|===\n|a |b\n|===").data_format(),
        DataFormat::Psv
    );

    // DSV cells are delimited by a colon (`:`).
    assert_eq!(
        parse_table("[format=dsv]\n|===\na:b\n|===").data_format(),
        DataFormat::Dsv
    );

    // CSV cells are delimited by a comma (`,`).
    assert_eq!(
        parse_table("[format=csv]\n|===\na,b\n|===").data_format(),
        DataFormat::Csv
    );

    // TSV cells are delimited by a tab character.
    assert_eq!(
        parse_table("[format=tsv]\n|===\na\tb\n|===").data_format(),
        DataFormat::Tsv
    );
}

#[test]
fn separator_attribute() {
    verifies!(
        r#"
.3+|separator
.3+|character used to separate cells
|{vbar}
|default for top-level tables
.3+|
|!
|default for nested tables
d|user-defined
|any single character or `\t` for tab (e.g., `{brvbar}` or `%`).
_Ideally a character not found in the cell content._

"#
    );

    // The default separator for a top-level table is the vertical bar (`|`), so
    // each `|` begins a new cell.
    let table = parse_table("|===\n|a |b\n|===");
    assert_eq!(table.body_rows()[0].cells().len(), 2);

    // The default separator for a nested table is the exclamation mark (`!`).
    let outer = parse_table("[cols=\"1a\"]\n|===\na|\n!===\n! x ! y\n!===\n|===");
    let inner = nested_table(&outer.body_rows()[0].cells()[0]);
    assert_eq!(inner.body_rows()[0].cells().len(), 2);
    assert_eq!(simple_text(&inner.body_rows()[0].cells()[0]), "x");
    assert_eq!(simple_text(&inner.body_rows()[0].cells()[1]), "y");

    // A user-defined separator can be any single character (here `%`), after
    // which the original cell separator (`|`) is ordinary text within a cell. As
    // in Asciidoctor's PSV, the separator only begins a new cell when it follows
    // whitespace, so that boundary whitespace is necessarily trimmed from the
    // preceding cell; the explicit assertion messages make a trimming regression
    // fail loudly rather than silently.
    let table = parse_table("[separator=%]\n|===\n%a | b %c\n|===");
    assert_eq!(table.body_rows()[0].cells().len(), 2);
    assert_eq!(
        simple_text(&table.body_rows()[0].cells()[0]),
        "a | b",
        "the literal `|` is kept and the boundary whitespace before `%` is trimmed"
    );
    assert_eq!(
        simple_text(&table.body_rows()[0].cells()[1]),
        "c",
        "content after the separator forms the next cell"
    );
}

#[test]
fn frame_attribute() {
    verifies!(
        r#"
.4+|frame
.4+|draws a border around the table
|all
|border on all sides (default)
.4+|

|ends
|border on top and bottom ends

|none
|no borders

|sides
|border on left and right sides

"#
    );

    // The `frame` attribute draws a border around the table. `all` (the default)
    // borders every side; `ends` borders the top and bottom; `none` removes the
    // border; `sides` borders the left and right.
    assert_eq!(parse_table("|===\n|a\n|===").frame(), Frame::All);
    assert_eq!(
        parse_table("[frame=all]\n|===\n|a\n|===").frame(),
        Frame::All
    );
    assert_eq!(
        parse_table("[frame=ends]\n|===\n|a\n|===").frame(),
        Frame::Ends
    );
    assert_eq!(
        parse_table("[frame=none]\n|===\n|a\n|===").frame(),
        Frame::None
    );
    assert_eq!(
        parse_table("[frame=sides]\n|===\n|a\n|===").frame(),
        Frame::Sides
    );
}

#[test]
fn grid_attribute() {
    verifies!(
        r#"
.4+|grid
.4+|draws boundary lines between rows and columns
|all
|draws boundary lines around each cell (default)
.4+|

|cols
|draws boundary lines between columns

|rows
|draws boundary lines between rows

|none
|no boundary lines

"#
    );

    // The `grid` attribute draws boundary lines between cells. `all` (the default)
    // borders every cell; `cols` borders between columns; `rows` borders between
    // rows; `none` removes the boundary lines.
    assert_eq!(parse_table("|===\n|a\n|===").grid(), Grid::All);
    assert_eq!(parse_table("[grid=all]\n|===\n|a\n|===").grid(), Grid::All);
    assert_eq!(
        parse_table("[grid=cols]\n|===\n|a\n|===").grid(),
        Grid::Cols
    );
    assert_eq!(
        parse_table("[grid=rows]\n|===\n|a\n|===").grid(),
        Grid::Rows
    );
    assert_eq!(
        parse_table("[grid=none]\n|===\n|a\n|===").grid(),
        Grid::None
    );
}

#[test]
fn stripes_attribute() {
    verifies!(
        r#"
.5+|stripes
.5+|controls row shading (via background color)
|none
|no rows are shaded (default)
.5+|

|even
|even rows are shaded

|odd
|odd rows are shaded

|hover
|row under the mouse cursor is shaded (HTML only)

|all
|all rows are shaded

"#
    );

    // The `stripes` attribute controls which rows are shaded. `none` (the default)
    // shades nothing; `even` and `odd` shade alternating rows; `hover` shades the
    // row under the mouse cursor; `all` shades every row.
    assert_eq!(parse_table("|===\n|a\n|===").stripes(), Stripes::None);
    assert_eq!(
        parse_table("[stripes=none]\n|===\n|a\n|===").stripes(),
        Stripes::None
    );
    assert_eq!(
        parse_table("[stripes=even]\n|===\n|a\n|===").stripes(),
        Stripes::Even
    );
    assert_eq!(
        parse_table("[stripes=odd]\n|===\n|a\n|===").stripes(),
        Stripes::Odd
    );
    assert_eq!(
        parse_table("[stripes=hover]\n|===\n|a\n|===").stripes(),
        Stripes::Hover
    );
    assert_eq!(
        parse_table("[stripes=all]\n|===\n|a\n|===").stripes(),
        Stripes::All
    );
}

#[test]
fn align_attribute() {
    verifies!(
        r#"
.3+|align
.3+|horizontally aligns a table with restricted width
|left
|aligns to left side of page (default)
.3+|Not recognized by Asciidoctor.
To align the table, use an alignment role (e.g., `[role=center]` or `[.center]`).
The alignment roles work for both HTML and PDF output.
Alignment roles and the `float` attributes are mutually exclusive.

|right
|aligns to right side of page

|center
|horizontally aligns to center of page

"#
    );

    // `align` is not recognized by Asciidoctor, and this crate matches that: the
    // attribute is accepted but given no special meaning. Whatever value it is
    // set to, the table's structure is unchanged and its columns keep their
    // default left alignment (alignment is expressed instead with a role or with
    // column/cell specifiers).
    for value in ["left", "right", "center"] {
        let src = format!("[align={value}]\n|===\n|a |b\n|===");
        let table = parse_table(&src);
        assert_eq!(table.columns().len(), 2);
        assert_eq!(table.columns()[0].h_align(), HorizontalAlignment::Left);
        assert_eq!(table.columns()[1].h_align(), HorizontalAlignment::Left);
    }
}

#[test]
fn float_attribute() {
    verifies!(
        r#"
.2+|float
.2+|floats the table to the specified side of the page
|left
|floats the table to the left side of the page (default)
.2+|Applies to HTML output only.
Must be used in conjunction with the table's `width` attribute to take effect.
The `float` and `align` attributes are mutually exclusive.

|right
|floats the table to the right side of the page

"#
    );

    // `float` only floats the table in HTML output; this crate does no rendering,
    // so the attribute is accepted but leaves the parsed table unchanged.
    for value in ["left", "right"] {
        let src = format!("[float={value},width=50%]\n|===\n|a |b\n|===");
        let table = parse_table(&src);
        assert_eq!(table.columns().len(), 2);
        assert_eq!(table.width(), Some(50));
    }
}

#[test]
fn halign_attribute() {
    verifies!(
        r#"
.3+|halign
.3+|horizontally aligns all of the cell contents in a table
|left
|aligns the contents of the cells to the left (default)
.3+|Not recognized by Asciidoctor.
Define instead using column or cell specifiers (e.g., `3*>`), which take precedence over this value.

|right
|aligns the contents of the cells to the right

|center
|aligns the contents to the cell centers

"#
    );

    // `halign` is not recognized by Asciidoctor, and this crate matches that:
    // whatever value it is set to, the columns retain their default left
    // alignment (a column or cell specifier must be used instead).
    for value in ["left", "right", "center"] {
        let src = format!("[halign={value}]\n|===\n|a |b\n|===");
        let table = parse_table(&src);
        assert_eq!(table.columns()[0].h_align(), HorizontalAlignment::Left);
        assert_eq!(table.columns()[1].h_align(), HorizontalAlignment::Left);
    }
}

#[test]
fn valign_attribute() {
    verifies!(
        r#"
.3+|valign
.3+|vertically aligns all of the cell contents in a table
|top
|aligns the cell contents to the top of the cell (default)
.3+|Not recognized by Asciidoctor.
Define instead using column or cell specifiers (e.g., `3*.>`), which take precedence over this value.

|bottom
|aligns the cell contents to the bottom of the cell

|middle
|aligns the cell contents to the middle of the cell

"#
    );

    // `valign` is not recognized by Asciidoctor, and this crate matches that:
    // whatever value it is set to, the columns retain their default top alignment
    // (a column or cell specifier must be used instead).
    for value in ["top", "bottom", "middle"] {
        let src = format!("[valign={value}]\n|===\n|a |b\n|===");
        let table = parse_table(&src);
        assert_eq!(table.columns()[0].v_align(), VerticalAlignment::Top);
        assert_eq!(table.columns()[1].v_align(), VerticalAlignment::Top);
    }
}

#[test]
fn orientation_attribute() {
    verifies!(
        r#"
|orientation
|rotates the table
|landscape
|rotated 90 degrees counterclockwise
|Equivalent to setting the rotate option, which is preferred.
DocBook only.

"#
    );

    // `orientation` only rotates the table in the DocBook backend (it sets
    // `orient="land"`); this crate does no rendering, so the attribute is accepted
    // but leaves the parsed table unchanged.
    let table = parse_table("[orientation=landscape]\n|===\n|a |b\n|===");
    assert_eq!(table.columns().len(), 2);
}

#[test]
fn options_attribute() {
    verifies!(
        r#"
.6+|options
.6+|comma separated list of option names
|header
|promotes first row to the table header
.2+d|header and footer rows are omitted by default

|footer
|promotes last row to the table footer

"#
    );

    // Header and footer rows are omitted by default: a table whose rows run
    // together (no blank line that would implicitly promote a header) has neither.
    let plain = parse_table("|===\n|a |b\n|c |d\n|===");
    assert!(plain.header_row().is_none());
    assert!(plain.footer_row().is_none());

    // The `header` option promotes the first row to the table header.
    let with_header = parse_table("[%header]\n|===\n|H1 |H2\n|a |b\n|===");
    assert!(with_header.header_row().is_some());

    // The `footer` option promotes the last row to the table footer.
    let with_footer = parse_table("[%footer]\n|===\n|a |b\n|c |d\n|===");
    assert!(with_footer.footer_row().is_some());

    verifies!(
        r#"
|breakable
|allows the table to split across a page (default)
.2+d|Mutually exclusive.
DocBook only (specifically for generating PDF output).

|unbreakable
|prevents the table from being split across a page

"#
    );

    // The `breakable`/`unbreakable` options only control page splitting in DocBook
    // PDF output; this crate does no rendering, so either option is accepted but
    // leaves the parsed table unchanged.
    for option in ["breakable", "unbreakable"] {
        let src = format!("[%{option}]\n|===\n|a |b\n|===");
        let table = parse_table(&src);
        assert_eq!(table.columns().len(), 2);
    }

    verifies!(
        r#"
|autowidth
|disables explicit column widths (ignores distribution ratios in `cols` attribute)
|

"#
    );

    // The `autowidth` option disables explicit column widths, sizing the table
    // and each of its columns to their content.
    let table = parse_table("[%autowidth,cols=\"2,1\"]\n|===\n|a |b\n|===");
    assert!(table.is_autowidth());
    assert!(table.columns().iter().all(|c| c.is_autowidth()));
}

#[test]
fn rotate_option() {
    verifies!(
        r#"
|rotate
|Prints the table in landscape
d|Equivalent to setting the orientation to landscape.
DocBook only.

"#
    );

    // The `rotate` option only prints the table in landscape in the DocBook
    // backend (equivalent to `orientation=landscape`); this crate does no
    // rendering, so the option is accepted but leaves the parsed table unchanged.
    let table = parse_table("[%rotate]\n|===\n|a |b\n|===");
    assert_eq!(table.columns().len(), 2);
}

#[test]
fn role_attribute() {
    verifies!(
        r#"
.4+|role
.4+|comma-separated list of role names
|left
|floats the table to the left margin
.3+d|The role is the preferred way to specify the alignment of a table with restricted width.
May be specified using role shorthand (e.g., `[.center]`).

|right
|floats the table to the right

|center
|aligns the table to center

|stretch
|stretches an autowidth table to the width of the page
|

"#
    );

    // The `role` attribute holds a comma-separated list of role names. Each of
    // the documented alignment roles (`left`, `right`, `center`, `stretch`) is
    // captured on the table; the alignment/float/stretch behavior they imply is
    // a rendering concern this crate does not implement.
    for role in ["left", "right", "center", "stretch"] {
        let src = format!("[role={role}]\n|===\n|a\n|===");
        let table = parse_table(&src);
        assert_eq!(table.roles(), vec![role]);
    }

    // The role shorthand (e.g. `[.center]`) captures the same role.
    let table = parse_table("[.center]\n|===\n|a\n|===");
    assert_eq!(table.roles(), vec!["center"]);
}

#[test]
fn width_attribute() {
    verifies!(
        r#"
|width
|the table width relative to the available page width
d|user defined value
|a percentage value between 0% and 100%
|
"#
    );

    // The `width` attribute sets the table width as a user-defined percentage of
    // the available page width. The trailing `%` is optional.
    assert_eq!(parse_table("[width=50%]\n|===\n|a\n|===").width(), Some(50));
    assert_eq!(parse_table("[width=75]\n|===\n|a\n|===").width(), Some(75));
    assert_eq!(
        parse_table("[width=100%]\n|===\n|a\n|===").width(),
        Some(100)
    );

    // When the attribute is absent, the table spans the available width and no
    // explicit width is reported.
    assert_eq!(parse_table("|===\n|a\n|===").width(), None);
}

// The closing table delimiter carries no rule to verify.
non_normative!(
    r#"
|===
"#
);
