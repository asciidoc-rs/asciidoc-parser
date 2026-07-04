use crate::tests::prelude::*;

track_file!("ref/asciidoc-lang/docs/modules/toc/pages/title.adoc");

non_normative!(
    r#"
= Customize the TOC Title

You can change the title of the table of contents with the `toc-title` attribute.

"#
);

#[test]
fn set_toc_title() {
    verifies!(
        r#"
== Set toc-title

To generate a TOC with a custom title, set the `toc-title` attribute in the header and assign it your preferred title.

.Define a custom TOC title
[source#ex-title]
----
include::example$toc.adoc[tags=header;title]
----
<.> The `toc` attribute must be set in order to use `toc-title`.
<.> The `toc-title` is set and assigned the value `Table of Adventures` in the document's header.

The result of <<ex-title>> is displayed below.

image::toc-title.png[Table of contents with a custom title,role=screenshot]
"#
    );

    let doc = Parser::default()
        .parse("= Title\n:toc:\n:toc-title: Table of Adventures\n\n== Section\n\nhi");

    // The custom title replaces the default _Table of Contents_ heading.
    assert_eq!(doc.toc_title(), "Table of Adventures");
    let vdom = doc.to_virtual_dom();
    let title = &vdom.children[0].children[0];
    assert_eq!(title.id.as_deref(), Some("toctitle"));
    assert_eq!(title.text.as_deref(), Some("Table of Adventures"));
}
