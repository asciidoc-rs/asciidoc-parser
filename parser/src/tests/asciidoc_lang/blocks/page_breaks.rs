use crate::{blocks::BreakType, tests::prelude::*};

track_file!("docs/modules/blocks/partials/page-breaks.adoc");

// This partial is included by `blocks/pages/breaks.adoc` (see `breaks.rs`). The
// parsing behavior is also exercised there; here we track the partial's own
// normative statements.

#[test]
fn page_break_syntax() {
    verifies!(
        r#"
== Page breaks

A line with three less-than characters (i.e., `<<<`), shown in <<ex-page-break>>, is a special macro that serves as a hint to the converter to insert a page break.
Like other block forms, the line must be offset by a preceding paragraph by at least one empty line.

.Page break syntax
[#ex-page-break]
----
<<<
----

A page break is only relevant for page-oriented / printable output formats such as DocBook, PDF, and HTML in print mode.

"#
    );

    // A line of three less-than characters is a page break.
    let doc = Parser::default().parse("<<<");
    assert_eq!(first_break(&doc).type_(), BreakType::Page);
}

#[test]
fn forced_page_break() {
    verifies!(
        r#"
If the page break macro falls at the top of an empty page, it will be ignored.
This behavior can be overridden by setting the `always` option on the macro as shown in <<ex-forced-page-break>>.

.Forced page break
[#ex-forced-page-break]
----
[%always]
<<<
----

"#
    );

    // The `always` option is captured on the page break block.
    let doc = Parser::default().parse("[%always]\n<<<");
    let brk = first_break(&doc);
    assert_eq!(brk.type_(), BreakType::Page);
    assert!(brk.has_option("always"));

    non_normative!(
        r#"
Some converters support additional options on the page break macro.
For example, Asciidoctor PDF allows the page layout of the new page to be specified.

.With page layout
[#ex-page-layout]
----
[page-layout=landscape]
<<<
----

If a converter supports columns, the page break can be converted into a column break by the addition of the `column` role.

.Column break
[#ex-column-break]
----
left column

[.column]
<<<

right column
----

When columns are not enabled or supported, the column break is expected to act as a page break.
"#
    );

    // The page-layout option, the `column` role, and how columns behave when
    // unsupported are all converter concerns; the parser captures the block's
    // attribute list for a downstream converter to act on (exercised in
    // `breaks.rs`).
}

fn first_break<'src>(doc: &'src crate::Document) -> &'src crate::blocks::Break<'src> {
    let block = doc.nested_blocks().next().unwrap();
    let crate::blocks::Block::Break(brk) = block else {
        panic!("Wrong block type: {block:#?}");
    };

    brk
}
