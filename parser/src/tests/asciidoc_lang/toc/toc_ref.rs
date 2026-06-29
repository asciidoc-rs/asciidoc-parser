use crate::{document::TocMode, tests::prelude::*};

track_file!("docs/modules/toc/pages/toc-ref.adoc");

// This page is a per-converter (backend) reference table for the `toc` family
// of attributes. This crate is a parser with no converter backends, so the
// backend-specific rows are non-normative here, but the documented default
// values are verified below.

non_normative!(
    r#"
= TOC Attributes Reference
:endash: &#8211;

//[cols="1,1,2,2,1"]
[%autowidth]
|===
|Attribute |Values |Example Syntax |Notes |Backends

.4+|`toc`
|`auto`, `left`, `right`, `macro`, `preamble`
|`:toc: left`
"#
);

#[test]
fn toc_default() {
    verifies!(
        r#"
|Not set by default.
Defaults to `auto` if value is unspecified.
"#
    );

    // Not set by default: with no `toc` attribute, no table of contents is
    // generated.
    let unset = Parser::default().parse("= Title\n\n== Section\n\nhi");
    assert_eq!(unset.toc_mode(), TocMode::Disabled);
    assert_css(&unset, ".toc", 0);

    // Defaults to `auto` if the value is unspecified.
    let empty = Parser::default().parse("= Title\n:toc:\n\n== Section\n\nhi");
    assert_eq!(empty.toc_mode(), TocMode::Auto);
}

non_normative!(
    r#"
|html

|`auto`, `macro`, `preamble`
|`:toc: macro`
|Not set by default.
Defaults to `auto` if value is unspecified.
|html (embeddable)

|`auto`
|`:toc:`
|Not set by default.
When the title page is enabled in PDF output, the table of contents is placed directly after the title page.
|pdf

|`auto`
|`:toc:`
|Not set by default.
The placement and styling of the table of contents is determined by the DocBook toolchain configuration.
|docbook

|`toclevels`
|0{endash}5
|`:toclevels: 4`
"#
);

#[test]
fn toclevels_default() {
    verifies!(
        r#"
|Default value is `2`.
"#
    );

    let doc = Parser::default().parse("= Title\n:toc:\n\n== Section\n\nhi");
    assert_eq!(doc.toc_levels(), 2);
}

non_normative!(
    r#"
|html, pdf

|`toc-title`
|user-defined
|`:toc-title: Contents`
"#
);

#[test]
fn toc_title_default() {
    verifies!(
        r#"
|Default value is _Table of Contents_.
"#
    );

    let doc = Parser::default().parse("= Title\n:toc:\n\n== Section\n\nhi");
    assert_eq!(doc.toc_title(), "Table of Contents");
}

non_normative!(
    r#"
|html, pdf

|`toc-class`
|valid CSS class name
|`:toc-class: floating-toc`
"#
);

#[test]
fn toc_class_default() {
    verifies!(
        r#"
|Default value is _toc_.
"#
    );

    let doc = Parser::default().parse("= Title\n:toc:\n\n== Section\n\nhi");
    assert_eq!(doc.toc_class(), "toc");
}

non_normative!(
    r#"
|html
|===
"#
);
