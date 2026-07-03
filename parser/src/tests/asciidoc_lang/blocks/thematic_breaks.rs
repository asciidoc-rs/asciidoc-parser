use crate::{blocks::BreakType, tests::prelude::*};

track_file!("docs/modules/blocks/partials/thematic-breaks.adoc");

// This partial is included by `blocks/pages/breaks.adoc` (see `breaks.rs`). The
// parsing behavior is also exercised there; here we track the partial's own
// normative statements.

#[test]
fn thematic_break_syntax() {
    verifies!(
        r#"
== Thematic breaks

A line with three single quotation marks (i.e., `pass:[''']`), shown in <<ex-rule>>, is a special macro that inserts a thematic break (aka horizontal rule).
Like other block forms, the line must be offset by a preceding paragraph by at least one empty line.

.Thematic break syntax
[#ex-rule]
----
'''
----

The result of <<ex-rule>> is displayed below.

====
'''
====

"#
    );

    // A line of three single quotation marks inserts a thematic break.
    let doc = Parser::default().parse("'''");
    assert_eq!(first_break(&doc).type_(), BreakType::Thematic);
}

#[test]
fn markdown_style_thematic_breaks() {
    non_normative!(
        r#"
=== Markdown-style thematic breaks

Asciidoctor recognizes Markdown thematic breaks.
The motivation for this support is to ease migration of Markdown documents to AsciiDoc documents.

"#
    );

    verifies!(
        r#"
To avoid conflicts with AsciiDoc's block delimiter syntax, only 3 repeating characters (`-` or `+*+`) are recognized.
As with Markdown, spaces between the characters is optional.

.Markdown-style thematic break syntax
----
---

- - -

***

* * *
----
"#
    );

    // Each recognized Markdown-style form (with or without spaces between the
    // characters) inserts a thematic break.
    for src in ["---", "- - -", "***", "* * *"] {
        let doc = Parser::default().parse(src);
        assert_eq!(
            first_break(&doc).type_(),
            BreakType::Thematic,
            "{src:?} should be a thematic break"
        );
    }

    // Only 3 repeating characters are recognized: a run of four or more is not a
    // thematic break (four dashes is a listing delimiter instead).
    let doc = Parser::default().parse("----");
    let block = doc.nested_blocks().next().expect("expected a block");
    assert_eq!(block.resolved_context().as_ref(), "listing");
}
