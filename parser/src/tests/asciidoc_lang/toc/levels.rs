use crate::tests::prelude::*;

track_file!("ref/asciidoc-lang/docs/modules/toc/pages/levels.adoc");

non_normative!(
    r#"
= Adjust the TOC Depth
:page-aliases: section-depth.adoc

You can adjust the depth of section levels displayed in the table of contents (TOC) using the `toclevels` attribute.

"#
);

#[test]
fn set_toclevels() {
    verifies!(
        r#"
== Set toclevels

The `toclevels` document attribute controls the depth of the TOC.
Accepted values are the integers 0 through 5.
These values represent the section levels.
(A section level is one less than the number of `=` signs the precede the title in the source.)

If the `toclevels` attribute is not specified, it defaults to `2`.
That means the TOC displays level 1 (`==`) and level 2 (`===`) section titles and, in the case of a multipart book, level 0 (`=`) section titles (parts).

Let's use the `toclevels` attribute to increase the depth of the TOC from 2 to 4.

.Define toclevels value
[source#ex-levels]
----
include::example$toc.adoc[tag=header]
:toc: <.>
:toclevels: 4 <.>
include::example$toc.adoc[tag=body]
----
<.> The `toc` attribute must be set in order to use `toclevels`.
<.> `toclevels` is set and assigned the value `4` in the document header.
The TOC will list the titles of any sections up to level 4 (i.e., `====`), when the document is rendered.

The result of <<ex-levels>> is displayed below.

image::toclevels.png[table of contents with the toclevels attribute set,role=screenshot]

"#
    );

    // If `toclevels` is not specified, it defaults to 2.
    let default = Parser::default().parse("= Title\n:toc:\n\n== One\n\n=== Two\n\nx");
    assert_eq!(default.toc_levels(), 2);

    // Setting `toclevels` to 4 lists sections up to level 4 (`====`).
    let doc = Parser::default()
        .parse("= Title\n:toc:\n:toclevels: 4\n\n== L1\n\n=== L2\n\n==== L3\n\n===== L4\n\nx");
    assert_eq!(doc.toc_levels(), 4);
    assert_css(&doc, "ul.sectlevel1", 1);
    assert_css(&doc, "ul.sectlevel2", 1);
    assert_css(&doc, "ul.sectlevel3", 1);
    assert_css(&doc, "ul.sectlevel4", 1);
    // The `=====` (level 4 section) is the deepest; a sixth level would be
    // excluded.
    assert_css(&doc, "ul.sectlevel5", 0);
}

#[test]
fn toclevels_zero_coerced_without_parts() {
    verifies!(
        r#"
In a multipart book, if you only want to see part titles (as well as any special sections at level 0) in the TOC, set `toclevels` to 0.
If the document does not have parts, and you set `toclevels` to 0, the value is coerced to 1.
"#
    );

    // This crate parses single (non-multipart) documents, so a `toclevels` of 0
    // is coerced to 1.
    let doc = Parser::default().parse("= Title\n:toc:\n:toclevels: 0\n\n== L1\n\n=== L2\n\nx");
    assert_eq!(doc.toc_levels(), 1);
    assert_css(&doc, "ul.sectlevel1", 1);
    assert_css(&doc, "ul.sectlevel2", 0);
}
