use crate::tests::prelude::*;

track_file!("ref/asciidoc-lang/docs/modules/tables/pages/striping.adoc");

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

/// Return the first [`TableBlock`] in `doc`.
///
/// Used by the tests that exercise the `table-stripes` document attribute: that
/// attribute must be set in the document header, so the whole document is
/// parsed rather than a single block.
///
/// [`TableBlock`]: crate::blocks::TableBlock
fn first_table<'a>(doc: &'a crate::Document<'a>) -> &'a crate::blocks::TableBlock<'a> {
    doc.child_blocks()
        .find_map(|block| match block {
            crate::blocks::Block::Table(table) => Some(table),
            _ => None,
        })
        .expect("expected a table block")
}

// A minimal table (no attribute line of its own) used as the base for the
// `stripes` / `table-stripes` assertions, which care only about the table-level
// value and not the table's shape.
const BASE: &str = "|===\n|A1\n|B1\n|===";

non_normative!(
    r#"
= Table Striping

"#
);

#[test]
fn intro() {
    verifies!(
        r#"
You can add zebra-striping to rows of a table.
When this feature is enabled, the specified rows are shaded using a background color to create a zebra striping effect.

"#
    );

    // With no `stripes` attribute, a table is not striped.
    assert_eq!(parse_table(BASE).stripes(), Stripes::None);

    // The note below describes a rendering concern (CSS and the stylesheet), not
    // anything this parser models.
    non_normative!(
        r#"
NOTE: In the HTML output, table striping is done using CSS and thus depends on the stylesheet to supply the necessary styles.
The default stylesheet for Asciidoctor includes the necessary styles for table striping.

"#
    );
}

#[test]
fn striping_attributes() {
    non_normative!(
        r#"
== Striping attributes

"#
    );

    verifies!(
        r#"
Which rows are striped is controlled using the `stripes` attribute on the table.
The stripes attribute defaults to the value `none` (implied value), which means rows are not striped by default.
This default can be changed by setting the `table-stripes` document attribute.
You can override the default value by setting the stripes attribute on the table.

"#
    );

    // The stripes default to `none`, both when the attribute is absent and when
    // it is given the implied value `none`.
    assert_eq!(parse_table(BASE).stripes(), Stripes::None);
    let src = format!("[stripes=none]\n{BASE}");
    assert_eq!(parse_table(&src).stripes(), Stripes::None);

    // The `table-stripes` document attribute changes the default for tables that
    // don't set their own `stripes`.
    let src = format!(":table-stripes: odd\n\n{BASE}");
    let doc = Parser::default().parse(&src);
    assert_eq!(first_table(&doc).stripes(), Stripes::Odd);

    // An explicit `stripes` on the table overrides the document attribute.
    let src = format!(":table-stripes: odd\n\n[stripes=even]\n{BASE}");
    let doc = Parser::default().parse(&src);
    assert_eq!(first_table(&doc).stripes(), Stripes::Even);

    verifies!(
        r#"
The `stripes` attribute on a table and the `table-stripes` document attribute accept the following values:

* `none` - no rows are shaded (default)
* `even` - even rows are shaded
* `odd` - odd rows are shaded
* `all` - all rows are shaded
* `hover` - the row under the mouse cursor is shaded (HTML only)

"#
    );

    // Each documented value resolves to its corresponding variant.
    let src = format!("[stripes=none]\n{BASE}");
    assert_eq!(parse_table(&src).stripes(), Stripes::None);
    let src = format!("[stripes=even]\n{BASE}");
    assert_eq!(parse_table(&src).stripes(), Stripes::Even);
    let src = format!("[stripes=odd]\n{BASE}");
    assert_eq!(parse_table(&src).stripes(), Stripes::Odd);
    let src = format!("[stripes=all]\n{BASE}");
    assert_eq!(parse_table(&src).stripes(), Stripes::All);
    let src = format!("[stripes=hover]\n{BASE}");
    assert_eq!(parse_table(&src).stripes(), Stripes::Hover);

    // An unrecognized value falls back to the default.
    let src = format!("[stripes=bogus]\n{BASE}");
    assert_eq!(parse_table(&src).stripes(), Stripes::None);
}

#[test]
fn stripes_block_attribute() {
    non_normative!(
        r#"
== stripes block attribute

"#
    );

    verifies!(
        r#"
In the following example, the stripes are enabled for even rows in the table body (the row that contains A2 and B2).

[source]
----
[cols=2*,stripes=even]
|===
|A1
|B1
|A2
|B2
|A3
|B3
|===
----

"#
    );

    // The example table: two columns, three body rows, with even-row striping.
    let src = "[cols=2*,stripes=even]\n|===\n|A1\n|B1\n|A2\n|B2\n|A3\n|B3\n|===";
    let table = parse_table(src);
    assert_eq!(table.stripes(), Stripes::Even);
    assert_eq!(table.columns().len(), 2);
    assert!(table.header_row().is_none());
    assert_eq!(table.body_rows().len(), 3);

    verifies!(
        r#"
Under the covers, the stripes attribute applies the CSS class `stripes-<value>` (e.g., `stripes-none`) to the table tag.
As a shorthand, you can simply apply the CSS class to the table directly using a role.

[source]
----
[.stripes-even,cols=2*]
|===
|A1
|B1
|A2
|B2
|A3
|B3
|===
----

"#
    );

    // The role shorthand applies the `stripes-even` CSS class directly without
    // setting the `stripes` attribute, so the parsed `stripes` value stays
    // `none` and the class is reported among the table's roles instead.
    let src = "[.stripes-even,cols=2*]\n|===\n|A1\n|B1\n|A2\n|B2\n|A3\n|B3\n|===";
    let table = parse_table(src);
    assert_eq!(table.stripes(), Stripes::None);
    assert!(table.attrlist().unwrap().roles().contains(&"stripes-even"));
}

#[test]
fn table_stripes_attribute() {
    non_normative!(
        r#"
== table-stripes attribute

"#
    );

    verifies!(
        r#"
If you want to apply stripes to all tables in the document, set the `table-stripes` attribute in the document header.
You can still override this setting per table.

[source]
----
= Document Title
:table-stripes: even

[cols=2*]
|===
|A1
|B1
|A2
|B2
|A3
|B3
|===
----
"#
    );

    // The document-header attribute stripes every table that doesn't set its
    // own `stripes`.
    let src = "= Document Title\n:table-stripes: even\n\n[cols=2*]\n|===\n|A1\n|B1\n|A2\n|B2\n|A3\n|B3\n|===";
    let doc = Parser::default().parse(src);
    assert_eq!(first_table(&doc).stripes(), Stripes::Even);

    // A table can still override the document-wide setting.
    let src =
        "= Document Title\n:table-stripes: even\n\n[stripes=odd,cols=2*]\n|===\n|A1\n|B1\n|===";
    let doc = Parser::default().parse(src);
    assert_eq!(first_table(&doc).stripes(), Stripes::Odd);
}
