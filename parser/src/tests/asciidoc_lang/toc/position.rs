use crate::{document::TocMode, tests::prelude::*};

track_file!("ref/asciidoc-lang/docs/modules/toc/pages/position.adoc");

non_normative!(
    r#"
= Position the TOC

"#
);

#[test]
fn positional_values() {
    verifies!(
        r#"
By default, the table of contents is inserted directly below the document title, author, and revision lines when the `toc` attribute is set and its value is left empty or set to `auto`.
This location can be changed by assigning one of the built-in positional values to the `toc` attribute.
The values are:

* `left`
* `right`
* `preamble`
* `macro`
* `auto` (default)

"#
    );

    // An empty value (or `auto`) inserts the TOC directly below the header.
    for value in ["", " auto"] {
        let doc = Parser::default().parse(&format!("= Title\n:toc:{value}\n\n== Section\n\nhi"));
        assert_eq!(doc.toc_mode(), TocMode::Auto, "value: {value:?}");
        assert_eq!(doc.to_virtual_dom().children[0].id.as_deref(), Some("toc"));
    }

    // Each built-in positional value resolves to its own placement.
    for (value, mode) in [
        ("left", TocMode::Left),
        ("right", TocMode::Right),
        ("preamble", TocMode::Preamble),
        ("macro", TocMode::Macro),
        ("auto", TocMode::Auto),
    ] {
        let doc = Parser::default().parse(&format!("= Title\n:toc: {value}\n\n== Section\n\nhi"));
        assert_eq!(doc.toc_mode(), mode, "value: {value}");
    }
}

#[test]
fn side_column() {
    verifies!(
        r#"
[#side-column]
== Display the TOC as a side column

When converting to HTML, you can position the TOC to the left or right of the main content column by assigning the value `left` or `right` to the `toc` attribute, respectively.
The sidebar column containing the TOC is both fixed and scrollable.

"#
    );

    // The `left`/`right` side-column styling needs standalone HTML body framing
    // this crate does not model, but the values still resolve to their own
    // placements and generate a TOC.
    for (value, mode) in [("left", TocMode::Left), ("right", TocMode::Right)] {
        let doc = Parser::default().parse(&format!("= Title\n:toc: {value}\n\n== Section\n\nhi"));
        assert_eq!(doc.toc_mode(), mode, "value: {value}");
        assert_css(&doc, "#toc.toc", 1);
    }
}

non_normative!(
    r#"
.Assign the left value to toc
[source#ex-left]
----
include::example$toc.adoc[tag=header]
:toc: left
include::example$toc.adoc[tag=body]
----

The result of <<ex-left>> is displayed below.

image::toc-left.png[Display the table of contents as a side column,role=screenshot]

This positioning is achieved using CSS and depends on support from the stylesheet.

[#min-width-requirement]
WARNING: The side positions (left and right) have a width requirement.
These positions are only honored if there's sufficient room on the screen to fit the sidebar column (typically at least 768px).
If sufficient room available is not available (i.e., the screen width falls below the breakpoint), the TOC *automatically shifts back to the center*, appearing directly below the document title.

NOTE: The TOC is always placed in the center in an embeddable HTML document, regardless of the value of the `toc` attribute.

"#
);

#[test]
fn beneath_preamble() {
    verifies!(
        r#"
[#beneath-preamble]
== Display the TOC beneath the preamble

When `toc` is assigned the built-in value `preamble`, the TOC is placed immediately below the xref:blocks:preamble-and-lead.adoc[preamble].

.Assign the preamble value to toc
[source#ex-preamble]
----
include::example$toc.adoc[tags=header;preamble]
----

The result of <<ex-preamble>> is displayed below.

image::toc-preamble.png[Display the table of contents below the preamble,role=screenshot]

CAUTION: When using the `preamble` value, the TOC will not appear if your document does not have a preamble.
To fix this problem, set the `toc` attribute to an empty value (i.e., leave the value empty) or assign it the value `auto`.

"#
    );

    // A `preamble` placement renders the TOC immediately below the preamble.
    let doc = Parser::default().parse("= Title\n:toc: preamble\n\nintro para\n\n== Section\n\nhi");
    assert_eq!(doc.toc_mode(), TocMode::Preamble);
    assert_css(&doc, "#preamble > #toc.toc", 1);
    // It is not placed at the top of the body.
    assert_ne!(doc.to_virtual_dom().children[0].id.as_deref(), Some("toc"));

    // The TOC does not appear when the document has no preamble.
    let no_preamble = Parser::default().parse("= Title\n:toc: preamble\n\n== Section\n\nhi");
    assert_eq!(no_preamble.toc_mode(), TocMode::Preamble);
    assert_css(&no_preamble, ".toc", 0);
}

#[test]
fn toc_macro() {
    verifies!(
        r#"
[#at-macro]
== Use the TOC macro to position the TOC

To place the TOC in specific location in the document, assign the `macro` value to the `toc` attribute.
Then, enter the table of contents block macro (i.e., TOC macro) on the line in your document where you want the TOC to appear.
The TOC macro should only be used once in a document.

If the `toc` document attribute isn't assigned the value `macro`, any TOC macro in the document will be ignored.

.Assign the macro value to toc
[source#ex-macro]
----
include::example$toc.adoc[tags=header;macro]
----
<.> The `toc` attribute must be set to `macro` to enable the use of the TOC macro.
<.> In this example, the TOC macro is placed below the first section's title, indicating that this is the location where the TOC will be displayed once the document is rendered.

"#
    );

    // With `toc: macro`, the TOC is rendered where the `toc::[]` macro appears.
    let doc =
        Parser::default().parse("= Title\n:toc: macro\n\nintro\n\ntoc::[]\n\n== Section\n\nhi");
    assert_eq!(doc.toc_mode(), TocMode::Macro);
    assert_css(&doc, "#preamble .toc", 1);

    // When `toc` is not `macro`, a `toc::[]` macro is ignored (no extra TOC is
    // generated beyond the automatically placed one).
    let auto = Parser::default().parse("= Title\n:toc:\n\ntoc::[]\n\n== Section\n\nhi");
    assert_eq!(auto.toc_mode(), TocMode::Auto);
    assert_css(&auto, ".toc", 1);
    assert_eq!(auto.to_virtual_dom().children[0].id.as_deref(), Some("toc"));
}

non_normative!(
    r#"
The result of <<ex-macro>> is displayed below.

image::toc-macro.png[Display the table of contents using the TOC macro,role=screenshot]

[#limitations]
== Embeddable HTML, editor and previewer limitations

When AsciiDoc is converted to embeddable HTML (i.e., the `header_footer` option is `false`), there are only three valid values for the `toc` attribute:

* `auto`
* `preamble`
* `macro`

All of the following environments convert AsciiDoc to embeddable HTML:

* the file viewer on GitHub and GitLab
* the AsciiDoc preview in an editor like Atom, Brackets or AsciidocFX
* the Asciidoctor browser extensions

IMPORTANT: The side column placement (left or right) isn't available in this mode.
That's because the embeddable HTML doesn't have the outer framing (or the CSS) necessary to support a side column TOC.
"#
);
