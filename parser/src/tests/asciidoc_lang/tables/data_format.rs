use crate::tests::prelude::*;

track_file!("docs/modules/tables/pages/data-format.adoc");

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

/// Return the rendered text of every cell in the table, in document order
/// (header row first, then body rows, then any footer row).
fn all_texts(table: &crate::blocks::TableBlock<'_>) -> Vec<String> {
    table
        .header_row()
        .into_iter()
        .chain(table.body_rows())
        .chain(table.footer_row())
        .flat_map(|row| row.cells().iter().map(simple_text))
        .collect()
}

non_normative!(
    r#"
= Table Data Formats
:navtitle: CSV, TSV and DSV Data
:url-dsv: https://en.wikipedia.org/wiki/Delimiter-separated_values
:url-rfc-4180: https://tools.ietf.org/html/rfc4180

== Default table syntax

A table is delimited by a vertical bar and three equal signs (`|===`).
It contains cells that are arranged into rows according to the number of columns the table is assigned.
The number of columns a table contains can be specified implicitly using the number of cells in the table's first row or by setting the `cols` attribute.
Each cell is specified by a vertical bar (`|`).

If you're new to AsciiDoc tables, xref:build-a-basic-table.adoc[] provides step by step directions for creating your first table.

== Style and layout options

Table content can be:

* styled and aligned by column or cell,
* aligned by row,
* duplicated across multiple rows, and
* marked up by any AsciiDoc syntax.

Table cells can span rows and columns.

You can adjust a table's:

* width,
* orientation, and
* border style.

You can also specify each column's width and designate header and footer rows.

== Supported data formats

"#
);

#[test]
fn supported_data_formats() {
    verifies!(
        r#"
The default table data format is prefix-separated values (PSV); that means the processor creates a new cell each time it encounters a vertical bar (`|`).
AsciiDoc also supports comma-separated values (CSV), tab-separated values (TSV), and delimited data values (DSV).

"#
    );

    // PSV is the default data format.
    assert_eq!(
        parse_table("|===\n|a |b\n|===").data_format(),
        DataFormat::Psv
    );

    // CSV, TSV, and DSV are also supported, selected with the `format` attribute.
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
}

#[test]
fn escape_the_cell_separator() {
    verifies!(
        r#"
== Escape the cell separator

The parser scans for the cell separator to partition cells _before_ it processes the cell text.
So even if you try to hide the cell separator using an inline passthrough, the parser will see it.
If the cell contain contains the cell separator, you must escape that character.
There are three ways to escape it:

* Prefix the character with a leading backslash (i.e., `\|`), which will be removed from the output.
* Use the `\{vbar}` attribute reference in place of `|` in content.
* Change the cell separator used by the table.

Unless you do one of these things, the cell separator will be interpreted as a cell boundary.

Consider the following example, which escapes the cell separator using a leading backslash:

[source]
----
[cols=2*]
|===
|The default separator in PSV tables is the \| character.
|The \| character is often referred to as a "`pipe`".
|===
----

This table will render as follows:

.Result: Converted PSV table that contains pipe characters
[cols=2*]
|===
|The default separator in PSV tables is the \| character.
|The \| character is often referred to as a "`pipe`".
|===

"#
    );

    // The backslash-escaped separator (`\|`) is unescaped to a bare `|`, and the
    // leading backslash is removed from the rendered cell.
    let table = parse_table(
        "[cols=2*]\n|===\n|The default separator in PSV tables is the \\| character.\n|The \\| character is often referred to as a \"`pipe`\".\n|===",
    );
    assert_eq!(table.body_rows().len(), 1);
    let cells: Vec<String> = table.body_rows()[0]
        .cells()
        .iter()
        .map(simple_text)
        .collect();
    assert_eq!(
        cells[0],
        "The default separator in PSV tables is the | character."
    );
    assert!(cells[1].contains("The | character"), "got: {:?}", cells[1]);
}

#[test]
fn escape_with_backslash() {
    verifies!(
        r#"
Notice that the pipe character appears without the leading backslash (i.e., unescaped) in the rendered result.

"#
    );

    // The escaped separator (`\|`) is unescaped to a bare `|` in the rendered
    // cell, and the leading backslash is removed.
    let table = parse_table(
        "[cols=2*]\n|===\n|The default separator in PSV tables is the \\| character.\n|The \\| character is often referred to as a \"`pipe`\".\n|===",
    );
    let cell = simple_text(&table.body_rows()[0].cells()[0]);
    assert!(cell.contains("the | character"), "got: {cell:?}");
    assert!(
        !cell.contains("\\|"),
        "backslash should be removed: {cell:?}"
    );
}

#[test]
fn substitute_with_vbar() {
    verifies!(
        r#"
An alternative is to use the `\{vbar}` attribute reference as a substitute.
This approach produces the same result as the previous example.

"#
    );

    // The `{vbar}` attribute reference produces the same result as the escaped
    // separator: a bare `|` in the rendered cell.
    let table = parse_table(
        "[cols=2*]\n|===\n|The default separator in PSV tables is the {vbar} character.\n|The {vbar} character is often referred to as a \"`pipe`\".\n|===",
    );
    let cell = simple_text(&table.body_rows()[0].cells()[0]);
    assert!(cell.contains("the | character"), "got: {cell:?}");
}

#[test]
fn substitute_the_vbar_reference() {
    verifies!(
        r#"
[source]
----
[cols=2*]
|===
|The default separator in PSV tables is the {vbar} character.
|The {vbar} character is often referred to as a "`pipe`".
|===
----

Escaping each cell separator character that appears in the content of a cell can be tedious.
There are also times when you can't or don't want to modify the cell content (perhaps because it is being included from another file).
To address these cases, AsciiDoc allows you to override the cell separator.

"#
    );

    // The `{vbar}` attribute reference renders as a bare `|`, producing the same
    // result as the backslash escape.
    let table = parse_table(
        "[cols=2*]\n|===\n|The default separator in PSV tables is the {vbar} character.\n|The {vbar} character is often referred to as a \"`pipe`\".\n|===",
    );
    let cells: Vec<String> = table.body_rows()[0]
        .cells()
        .iter()
        .map(simple_text)
        .collect();
    assert_eq!(
        cells[0],
        "The default separator in PSV tables is the | character."
    );
    assert!(cells[1].contains("The | character"), "got: {:?}", cells[1]);
}

#[test]
fn override_the_cell_separator() {
    verifies!(
        r#"
The cell separator is controlled using the `separator` attribute on the table block.
You'll want to select any single character that is not found in the content.
A good candidate is the broken bar, or `¦`.

"#
    );

    // With the separator overridden to the broken bar (`¦`), the vertical bar in
    // the cell content is ordinary text rather than a cell boundary.
    let table = parse_table(
        "[cols=2*,separator=¦]\n|===\n¦The default separator in PSV tables is the | character.\n¦The | character is often referred to as a \"`pipe`\".\n|===",
    );
    assert_eq!(table.body_rows().len(), 1);
    assert_eq!(table.body_rows()[0].cells().len(), 2);
    let cell = simple_text(&table.body_rows()[0].cells()[0]);
    assert_eq!(
        cell,
        "The default separator in PSV tables is the | character."
    );
}

#[test]
fn custom_separator_example() {
    verifies!(
        r#"
Here's the previous example rewritten using a custom separator.

[source]
----
[cols=2*,separator=¦]
|===
¦The default separator in PSV tables is the | character.
¦The | character is often referred to as a "`pipe`".
|===
----

Notice that it's no longer necessary to escape the pipe character in the content of the table cells.
You can safely use the original cell separator in the cell content and not worry about it being interpreted as the boundary of a cell.

[#delimiter-separated-values]
== Delimiter-separated values

"#
    );

    // With the separator overridden to the broken bar (`¦`), the vertical bars in
    // the cell content are ordinary text and need no escaping.
    let table = parse_table(
        "[cols=2*,separator=¦]\n|===\n¦The default separator in PSV tables is the | character.\n¦The | character is often referred to as a \"`pipe`\".\n|===",
    );
    assert_eq!(table.body_rows().len(), 1);
    let cells: Vec<String> = table.body_rows()[0]
        .cells()
        .iter()
        .map(simple_text)
        .collect();
    assert_eq!(
        cells[0],
        "The default separator in PSV tables is the | character."
    );
    assert!(cells[1].contains("The | character"), "got: {:?}", cells[1]);
}

#[test]
fn delimiter_between_values() {
    verifies!(
        r#"
Tables can also be populated from data formatted as delimiter-separated values (i.e., data tables).
In contrast with the PSV format, in which the delimiter is placed in front of each cell value, the delimiter in a delimiter-separated format (CSV, TSV, DSV) is placed between the cell values (called a _separator_) and does not accept a cell formatting spec.
Each line of data is assumed to represent a single row, though you'll learn that's not a strict rule.
How the table data gets interpreted is controlled by the `format` and `separator` attributes on the table.

"#
    );

    // The separator sits between the values, so three comma-separated values
    // produce three cells (a three-column row).
    let table = parse_table("[format=csv]\n|===\na,b,c\n|===");
    assert_eq!(table.data_format(), DataFormat::Csv);
    assert_eq!(table.columns().len(), 3);
    assert_eq!(all_texts(&table), ["a", "b", "c"]);

    // A data cell does not accept a cell formatting spec: what would be a colspan
    // operator in PSV (`2+`) is taken literally as the cell's value.
    let table = parse_table("[format=csv]\n|===\n2+,b,c\n|===");
    assert_eq!(table.columns().len(), 3);
    assert_eq!(table.body_rows()[0].cells()[0].colspan(), 1);
    assert_eq!(simple_text(&table.body_rows()[0].cells()[0]), "2+");
}

non_normative!(
    r#"
.What the delimiter?
****
Aren't comma-separated values a subset of {url-dsv}[delimiter-separated values^]?
It really depends on who you consult.

The term "`delimiter-separated values`" in this text refers to the family of data formats that use a delimiter, including comma-separated values (CSV), tab-separated values (TSV) and delimited data (DSV), all of which are supported in AsciiDoc tables.
CSV is the data format used most often.

"`Comma-separated values`" is really a misleading term since CSV can use delimiters other than `,` as the field separator (which, in this context, separates cells).
What we're really talking about is how the data is interpreted.

CSV and TSV both use a delimiter and an optional enclosing character, loosely based on {url-rfc-4180}[RFC 4180^].
DSV (i.e., delimited data) only uses a delimiter, which can be escaped using a backslash; an enclosing character is not recognized.
These parsing rules are described in detail in <<data-table-formats>>.
****

"#
);

#[test]
fn csv_format() {
    verifies!(
        r#"
Let's consider an example of using comma-separated values (CSV) to populate an AsciiDoc table with data.
To instruct the processor to read the data as CSV, set the value of the `format` attribute on the table to `csv`.
When the `format` attribute is set to `csv`, the default data separator is a comma (`,`), as seen in the table below.

"#
    );

    // With `format=csv`, the default separator is a comma.
    let table = parse_table(
        "[%header,format=csv]\n|===\nArtist,Track,Genre\nBaauer,Harlem Shake,Hip Hop\nThe Lumineers,Ho Hey,Folk Rock\n|===",
    );
    assert_eq!(table.data_format(), DataFormat::Csv);
    assert_eq!(table.columns().len(), 3);

    let header: Vec<String> = table
        .header_row()
        .unwrap()
        .cells()
        .iter()
        .map(simple_text)
        .collect();
    assert_eq!(header, ["Artist", "Track", "Genre"]);
    assert_eq!(table.body_rows().len(), 2);
}

#[test]
fn csv_example() {
    verifies!(
        r#"
[source]
----
include::example$data.adoc[tag=csv]
----

.Result: Rendered CSV table
[width=90%]
include::example$data.adoc[tag=csv]

This feature is particularly useful when you want to populate a table in your manuscript from data stored in a separate file.
You can do so using the xref:directives:include.adoc[include directive] between the table delimiters, as shown here:

[source]
----
[%header,format=csv]
|===
\include::tracks.csv[]
|===
----

"#
    );

    // `include::example$data.adoc[tag=csv]` expands to this CSV table.
    let table = parse_table(
        "[%header,format=csv]\n|===\nArtist,Track,Genre\nBaauer,Harlem Shake,Hip Hop\nThe Lumineers,Ho Hey,Folk Rock\n|===",
    );
    assert_eq!(table.data_format(), DataFormat::Csv);
    let header: Vec<String> = table
        .header_row()
        .unwrap()
        .cells()
        .iter()
        .map(simple_text)
        .collect();
    assert_eq!(header, ["Artist", "Track", "Genre"]);
    assert_eq!(
        all_texts(&table),
        [
            "Artist",
            "Track",
            "Genre",
            "Baauer",
            "Harlem Shake",
            "Hip Hop",
            "The Lumineers",
            "Ho Hey",
            "Folk Rock"
        ]
    );
}

#[test]
fn tsv_format() {
    verifies!(
        r#"
If your data is separated by tabs instead of commas, set the `format` to `tsv` (tab-separated values) instead.

"#
    );

    // With `format=tsv`, the default separator is a tab.
    let table = parse_table("[format=tsv]\n|===\nArtist\tTrack\tGenre\n|===");
    assert_eq!(table.data_format(), DataFormat::Tsv);
    assert_eq!(table.columns().len(), 3);
    assert_eq!(all_texts(&table), ["Artist", "Track", "Genre"]);
}

#[test]
fn dsv_format() {
    verifies!(
        r#"
Now let's consider an example of using delimited data (DSV) to populate an AsciiDoc table with data.
To instruct the processor to read the data as DSV, set the value of the `format` attribute on the table to `dsv`.
When the `format` attribute is set to `dsv`, the default data separator is a colon (`:`), as seen in the table below.

"#
    );

    // With `format=dsv`, the default separator is a colon.
    let table = parse_table(
        "[%header,format=dsv]\n|===\nArtist:Track:Genre\nRobyn:Indestructible:Dance\nThe Piano Guys:Code Name Vivaldi:Classical\n|===",
    );
    assert_eq!(table.data_format(), DataFormat::Dsv);
    assert_eq!(table.columns().len(), 3);
    assert_eq!(table.body_rows().len(), 2);
}

#[test]
fn dsv_example() {
    verifies!(
        r#"
[source]
----
include::example$data.adoc[tag=dsv]
----

.Result: Rendered DSV table
[width=90%]
include::example$data.adoc[tag=dsv]

"#
    );

    // `include::example$data.adoc[tag=dsv]` expands to this DSV table.
    let table = parse_table(
        "[%header,format=dsv]\n|===\nArtist:Track:Genre\nRobyn:Indestructible:Dance\nThe Piano Guys:Code Name Vivaldi:Classical\n|===",
    );
    assert_eq!(table.data_format(), DataFormat::Dsv);
    assert_eq!(
        all_texts(&table),
        [
            "Artist",
            "Track",
            "Genre",
            "Robyn",
            "Indestructible",
            "Dance",
            "The Piano Guys",
            "Code Name Vivaldi",
            "Classical"
        ]
    );
}

non_normative!(
    r#"
== Data table formats

The CSV and TSV data formats are parsed differently from the DSV data format.
The following two sections outline those differences.

=== CSV and TSV

"#
);

#[test]
fn csv_and_tsv_parsing_rules() {
    verifies!(
        r#"
Table data in either CSV or TSV format is parsed according to the following rules, loosely based on {url-rfc-4180}[RFC 4180^]:

* The default delimiter for CSV is a comma (`,`) while the default delimiter for TSV is a tab character.
* Empty lines are skipped (unless enclosed in a quoted value).
* Whitespace surrounding each value is stripped.
* Values can be enclosed in double quotes (`"`).
 ** A quoted value may contain zero or more separator or newline characters.
 ** A newline begins a new row unless the newline is enclosed in double quotes.
 ** A quoted value may include the double quote character if escaped using another double quote (`""`).
 ** Newlines in quoted values are retained.
* If rows do not have the same number of cells ("`ragged`" tables), cells are shuffled to fully fill the rows.
 ** This is different behavior than Excel, which pads short rows with empty cells.
 ** Extra cells at the end of the last row get dropped.
 ** As a rule of thumb, data for a single row should be on the same line.

"#
    );

    // The default delimiter is a comma for CSV and a tab for TSV.
    assert_eq!(
        parse_table("[format=csv]\n|===\na,b,c\n|===")
            .columns()
            .len(),
        3
    );
    assert_eq!(
        parse_table("[format=tsv]\n|===\na\tb\tc\n|===")
            .columns()
            .len(),
        3
    );

    // Empty lines are skipped and the whitespace surrounding each value is
    // stripped.
    let table = parse_table("[format=csv]\n|===\na, b ,c\nd,e,f\n\ng,h,i\n|===");
    assert!(table.header_row().is_none());
    assert_eq!(
        all_texts(&table),
        ["a", "b", "c", "d", "e", "f", "g", "h", "i"]
    );

    // A quoted value may contain the separator, which is then literal.
    let table = parse_table("[format=csv]\n|===\na,\"b,c\",d\n|===");
    assert_eq!(all_texts(&table), ["a", "b,c", "d"]);

    // A quoted value may contain newlines, which are retained, and a newline
    // inside the quotes does not begin a new row.
    let table = parse_table("[format=csv,cols=2*]\n|===\n\"line1\nline2\",x\ny,z\n|===");
    assert_eq!(table.body_rows().len(), 2);
    assert_eq!(
        simple_text(&table.body_rows()[0].cells()[0]),
        "line1\nline2"
    );
    assert_eq!(simple_text(&table.body_rows()[0].cells()[1]), "x");
    assert_eq!(simple_text(&table.body_rows()[1].cells()[0]), "y");

    // A doubled double quote inside a quoted value is an escaped literal quote.
    let table = parse_table("[format=csv]\n|===\n\"he said \"\"hi\"\"\",p,q\n|===");
    assert_eq!(
        simple_text(&table.body_rows()[0].cells()[0]),
        "he said \"hi\""
    );

    // A ragged table flows its cells into complete rows (it is not padded like
    // Excel); cells left over after the last complete row are dropped.
    let table = parse_table("[format=csv,cols=3*]\n|===\na,b,c,d,e\n|===");
    assert_eq!(all_texts(&table), ["a", "b", "c"]);

    let table = parse_table("[format=csv]\n|===\na,b,c\nd,e\nf,g,h,i\n|===");
    assert!(table.header_row().is_none());
    assert_eq!(
        all_texts(&table),
        ["a", "b", "c", "d", "e", "f", "g", "h", "i"]
    );
}

non_normative!(
    r#"
=== DSV

"#
);

#[test]
fn dsv_parsing_rules() {
    verifies!(
        r#"
Table data in DSV format is parsed according to the following rules:

* The default delimiter for DSV is a colon (`:`).
* Empty lines are skipped.
* Whitespace surrounding each value is stripped.
* The delimiter character can be included in the value if escaped using a single backslash (`\:`).
* If rows do not have the same number of cells ("`ragged`" tables), cells are shuffled to fully fill the rows.

"#
    );

    // The default delimiter for DSV is a colon, empty lines are skipped, and the
    // whitespace surrounding each value is stripped.
    let table = parse_table("[format=dsv]\n|===\na : b :c\nd:e:f\n|===");
    assert!(table.header_row().is_none());
    assert_eq!(all_texts(&table), ["a", "b", "c", "d", "e", "f"]);

    let table = parse_table("[format=dsv]\n|===\na:b\n\nc:d\n|===");
    assert!(table.header_row().is_some());
    assert_eq!(all_texts(&table), ["a", "b", "c", "d"]);

    // The delimiter can be included in a value by escaping it with a single
    // backslash; an enclosing character is not recognized.
    let table = parse_table("[format=dsv]\n|===\nd\\:e:f:g\n|===");
    assert_eq!(all_texts(&table), ["d:e", "f", "g"]);

    // A ragged DSV table also flows its cells into complete rows.
    let table = parse_table("[format=dsv,cols=2*]\n|===\na:b:c\n|===");
    assert_eq!(all_texts(&table), ["a", "b"]);
}

non_normative!(
    r#"
== Custom delimiters

"#
);

#[test]
fn custom_delimiters() {
    verifies!(
        r#"
Each data format has a default separator associated with it (csv = comma, tsv = tab, dsv = colon), but the separator can be changed to any character (or even a string of characters) by setting the `separator` attribute on the table.

"#
    );

    // The separator can be changed to any character...
    let table = parse_table("[format=dsv,separator=;]\n|===\na;b;c\nd;e;f\n|===");
    assert_eq!(all_texts(&table), ["a", "b", "c", "d", "e", "f"]);

    // ...or even to a string of characters.
    let table = parse_table("[format=csv,separator=::]\n|===\na::b::c\n|===");
    assert_eq!(table.columns().len(), 3);
    assert_eq!(all_texts(&table), ["a", "b", "c"]);
}

non_normative!(
    r#"
Here's an example of a DSV table that uses a custom separator character (i.e., delimiter):

.A DSV table with a custom separator
[source]
----
[format=dsv,separator=;]
|===
a;b;c
d;e;f
|===
----

"#
);

#[test]
fn tsv_via_csv_and_tab_separator() {
    verifies!(
        r#"
TIP: To make a TSV table, you can set the `format` attribute to `csv` and the separator to `\t`.
Though the `tsv` format is preferred.

"#
    );

    // A TSV table can be produced with `format=csv` and the separator set to a
    // tab (written `\t`); the preferred `tsv` format yields the same result.
    let via_csv = parse_table("[format=csv,separator=\\t]\n|===\na\tb\tc\n|===");
    assert_eq!(via_csv.data_format(), DataFormat::Csv);
    assert_eq!(all_texts(&via_csv), ["a", "b", "c"]);

    let via_tsv = parse_table("[format=tsv]\n|===\na\tb\tc\n|===");
    assert_eq!(all_texts(&via_tsv), all_texts(&via_csv));
}

#[test]
fn separator_independent_of_format() {
    verifies!(
        r#"
The separator is independent of the processing rules for the format.
If you set `format=dsv` and `separator=,`, the data will be processed using the DSV rules, even though the data looks like CSV.

"#
    );

    // The same separator is processed by whichever format's rules are in effect.
    // Under DSV a double quote is not an enclosing character, so it stays in the
    // value and the separator still splits around it.
    let table = parse_table("[format=dsv,separator=;]\n|===\n\"a;b\";c\n|===");
    assert_eq!(table.data_format(), DataFormat::Dsv);
    assert_eq!(table.columns().len(), 3);
    assert_eq!(all_texts(&table), ["\"a", "b\"", "c"]);

    // With the very same separator but `format=csv`, the double quotes instead
    // enclose the value, so the separator inside them is literal.
    let table = parse_table("[format=csv,separator=;]\n|===\n\"a;b\";c\n|===");
    assert_eq!(table.data_format(), DataFormat::Csv);
    assert_eq!(table.columns().len(), 2);
    assert_eq!(all_texts(&table), ["a;b", "c"]);
}

non_normative!(
    r#"
== Shorthand notation for data tables

"#
);

#[test]
fn shorthand_csv_delimiter() {
    verifies!(
        r#"
AsciiDoc provides shorthand notation for specifying the data format of a table.
The first position of the table block delimiter (i.e., `|===`) can be replaced by a built-in delimiter to set the table format (e.g., `,===` for CSV).

To make a CSV table, you can use `,===` as the table block delimiter:

"#
    );

    // The `,===` shorthand delimiter sets the CSV format.
    let table = parse_table(",===\nArtist,Track,Genre\n\nBaauer,Harlem Shake,Hip Hop\n,===");
    assert_eq!(table.data_format(), DataFormat::Csv);
    assert!(table.header_row().is_some());
    assert_eq!(table.columns().len(), 3);
}

#[test]
fn shorthand_csv_example() {
    verifies!(
        r#"
[source]
----
include::example$data.adoc[tag=s-csv]
----

.Result: Rendered CSV table using shorthand syntax
[width=90%]
include::example$data.adoc[tag=s-csv]

"#
    );

    // `include::example$data.adoc[tag=s-csv]` expands to this shorthand CSV table.
    let table = parse_table(",===\nArtist,Track,Genre\n\nBaauer,Harlem Shake,Hip Hop\n,===");
    assert_eq!(table.data_format(), DataFormat::Csv);
    assert!(table.header_row().is_some());
    assert_eq!(
        all_texts(&table),
        [
            "Artist",
            "Track",
            "Genre",
            "Baauer",
            "Harlem Shake",
            "Hip Hop"
        ]
    );
}

#[test]
fn shorthand_dsv_delimiter() {
    verifies!(
        r#"
To make a DSV table, you can use `:===` as the table block delimiter:

"#
    );

    // The `:===` shorthand delimiter sets the DSV format.
    let table = parse_table(":===\nArtist:Track:Genre\n\nRobyn:Indestructible:Dance\n:===");
    assert_eq!(table.data_format(), DataFormat::Dsv);
    assert!(table.header_row().is_some());
    assert_eq!(table.columns().len(), 3);
}

#[test]
fn shorthand_dsv_example() {
    verifies!(
        r#"
[source]
----
include::example$data.adoc[tag=s-dsv]
----

.Result: Rendered DSV table using shorthand syntax
[width=90%]
include::example$data.adoc[tag=s-dsv]

"#
    );

    // `include::example$data.adoc[tag=s-dsv]` expands to this shorthand DSV table.
    let table = parse_table(":===\nArtist:Track:Genre\n\nRobyn:Indestructible:Dance\n:===");
    assert_eq!(table.data_format(), DataFormat::Dsv);
    assert!(table.header_row().is_some());
    assert_eq!(
        all_texts(&table),
        [
            "Artist",
            "Track",
            "Genre",
            "Robyn",
            "Indestructible",
            "Dance"
        ]
    );
}

#[test]
fn shorthand_implies_format() {
    verifies!(
        r#"
When using either the CSV or DSV shorthand, you do not need to set the `format` attribute as it's implied.

"#
    );

    // The shorthand implies the format, so no `format` attribute is required.
    assert_eq!(
        parse_table(",===\na,b,c\n,===").data_format(),
        DataFormat::Csv
    );
    assert_eq!(
        parse_table(":===\na:b:c\n:===").data_format(),
        DataFormat::Dsv
    );
}

#[test]
fn tsv_has_no_shorthand_delimiter() {
    verifies!(
        r#"
To make a TSV table, you can set the `format` attribute to `tsv` instead of having to set the `format` to `csv` and the separator to `\t`.
In this case, you can use either `|===` or `,===` as the table block delimiter.
There is no special delimited block notation for a TSV table.

"#
    );

    // TSV is selected with `format=tsv`; there is no shorthand delimiter for it.
    let table = parse_table("[format=tsv]\n|===\na\tb\tc\n|===");
    assert_eq!(table.data_format(), DataFormat::Tsv);

    // Either `|===` or `,===` may be used as the block delimiter for a TSV table.
    let table = parse_table("[format=tsv]\n,===\na\tb\tc\n,===");
    assert_eq!(table.data_format(), DataFormat::Tsv);
    assert_eq!(all_texts(&table), ["a", "b", "c"]);
}

non_normative!(
    r#"
== Formatting cells in a data table

"#
);

#[test]
fn formatting_cells_via_cols_spec() {
    verifies!(
        r#"
The delimited formats do not provide a way to express formatting of individual table cells.
Instead, you can apply cell formatting to all cells in a given column using the `cols` spec on the table:

"#
    );

    // The delimited formats carry no per-cell spec, so cell formatting is applied
    // per column with the `cols` spec: here column 1 is a header column and
    // column 2 is an AsciiDoc column.
    let table = parse_table(
        "[format=csv,cols=\"1h,1a\"]\n|===\nSky,image::sky.jpg[]\nForest,image::forest.jpg[]\n|===",
    );
    assert_eq!(table.columns()[0].style(), ColumnStyle::Header);
    assert_eq!(table.columns()[1].style(), ColumnStyle::AsciiDoc);

    // The second column's cells are parsed as AsciiDoc block content.
    assert!(matches!(
        table.body_rows()[0].cells()[1].content(),
        TableCellContent::AsciiDoc(_)
    ));
}

#[test]
fn data_formatting_example() {
    verifies!(
        r#"
[source]
----
[format=csv,cols="1h,1a"]
|===
Sky,image::sky.jpg[]
Forest,image::forest.jpg[]
|===
----

"#
    );

    // The `cols="1h,1a"` example applies a header style to the first column and
    // the AsciiDoc style to the second.
    let table = parse_table(
        "[format=csv,cols=\"1h,1a\"]\n|===\nSky,image::sky.jpg[]\nForest,image::forest.jpg[]\n|===",
    );
    assert_eq!(table.columns()[0].style(), ColumnStyle::Header);
    assert_eq!(table.columns()[1].style(), ColumnStyle::AsciiDoc);
    assert!(matches!(
        table.body_rows()[0].cells()[1].content(),
        TableCellContent::AsciiDoc(_)
    ));
}

#[test]
fn data_tables_do_not_support_spans() {
    verifies!(
        r#"
Data tables do not support cells that span multiple rows or columns, since that information can only be expressed at the cell level.
You are advised to use the PSV format if you need that functionality.
"#
    );

    // A data table cannot express a row or column span: a `2+` operator is
    // ordinary cell data, spanning a single column.
    let table = parse_table("[format=csv]\n|===\n2+,b,c\nd,e,f\n|===");
    assert_eq!(table.columns().len(), 3);
    let cell = &table.body_rows()[0].cells()[0];
    assert_eq!(cell.colspan(), 1);
    assert_eq!(cell.rowspan(), 1);
    assert_eq!(simple_text(cell), "2+");
}
