use crate::tests::prelude::*;

track_file!("docs/modules/sections/pages/bibliography.adoc");

/// Collects the rendered (post-substitution) text of every simple block
/// (paragraph) in the document, in document order. Bibliography anchors and
/// cross-references are produced as inline content at parse time, so this lets
/// the tests below inspect them even though the crate doesn't render whole
/// blocks to HTML.
fn rendered_paragraphs(doc: &crate::Document<'_>) -> Vec<String> {
    fn walk(block: &crate::blocks::Block<'_>, out: &mut Vec<String>) {
        if let crate::blocks::Block::Simple(simple) = block {
            out.push(simple.content().rendered().to_string());
        }
        for child in block.nested_blocks() {
            walk(child, out);
        }
    }

    let mut out = vec![];
    for block in doc.nested_blocks() {
        walk(block, &mut out);
    }
    out
}

non_normative!(
    r#"
= Bibliography

AsciiDoc has basic support for bibliographies.
AsciiDoc doesn't concern itself with the structure of the bibliography entry itself, which is entirely freeform.
What it does is provide a way to make references to the entries from the same document and output the bibliography with proper semantics for processing by other toolchains (such as DocBook).

"#
);

#[test]
fn bibliography_section_syntax() {
    verifies!(
        r#"
== Bibliography section syntax

To conform to output formats, a bibliography must be its own section at any level.
The section must be assigned the `bibliography` section style.
By adding the `bibliography` style to the section, you implicitly add it to each unordered list in that section.

"#
    );

    // A section assigned the `bibliography` style implicitly adds that style to
    // each of its (top-level) unordered lists.
    let doc = Parser::default().parse("[bibliography]\n== Bibliography\n\n* [[[ref]]] An entry.\n");
    assert_css(&doc, ".ulist.bibliography", 1);

    non_normative!(
        r#"
You would define the bibliography as a level 1 section (`==`) when:

* the doctype is `article`
* the doctype is `book` and the book doesn't contain any parts
* the bibliography is for a part

"#
    );

    verifies!(
        r#"
[source]
----
[bibliography]
== Bibliography
----

"#
    );

    // The `bibliography` section style is recognized as a level 1 section.
    let doc = Parser::default().parse("[bibliography]\n== Bibliography\n\n* [[[a]]] An entry.\n");
    let section = doc.nested_blocks().next().unwrap();
    assert_eq!(section.declared_style(), Some("bibliography"));

    // Treating the remaining guidance as non-normative because it concerns
    // `doctype` and multi-part books, which this crate does not model. The
    // `bibliography` style is still recognized on a section at any level.
    non_normative!(
        r#"
You can also define it as a deeper section, in which case the doctype doesn't matter and it's scoped to the parent section.

If the book has parts, and the bibliography is for the whole book, the section is defined as a level 0 section (`=`).

[source]
----
[bibliography]
= Bibliography
----

"#
    );
}

#[test]
fn bibliography_entries_syntax() {
    verifies!(
        r#"
== Bibliography entries syntax

Bibliography entries are declared as items in an unordered list.

"#
    );

    // Each entry of a bibliography list is an unordered list item; the entry's
    // anchor is registered and rendered.
    let doc = Parser::default()
        .parse("[bibliography]\n== References\n\n* [[[pp]]] Andy Hunt & Dave Thomas. 1999.\n");
    assert_css(&doc, ".ulist.bibliography ul li", 1);
    assert_css(&doc, "a#pp", 1);

    non_normative!(
        r#"
.Bibliography with references
[source]
----
include::example$bibliography.adoc[tag=base]
----

"#
    );

    verifies!(
        r#"
In order to reference a bibliography entry, you need to assign a _non-numeric_ label to the entry.
To assign this label, prefix the entry with the label enclosed in a pair of triple square brackets (e.g., `+[[[label]]]+`).
We call this a bibliography anchor.
Using this label, you can then reference the entry from anywhere above the bibliography in the same document using the normal cross reference syntax (e.g., `+<<label>>+`).

"#
    );

    // A bibliography entry can be cross-referenced from anywhere above it, using
    // the normal cross-reference syntax. The reference renders the bracketed
    // label.
    let doc = Parser::default().parse(
        "Refer to <<pp>>.\n\n[bibliography]\n== References\n\n* [[[pp]]] Andy Hunt & Dave Thomas. 1999.\n",
    );
    let paragraphs = rendered_paragraphs(&doc);
    assert!(paragraphs[0].contains("<a href=\"#pp\">[pp]</a>"));
    assert!(paragraphs[1].starts_with("<a id=\"pp\"></a>[pp] "));

    // The label must be _non-numeric_: a label that begins with a digit is not
    // recognized as a bibliography anchor.
    let doc =
        Parser::default().parse("[bibliography]\n== References\n\n* [[[1984]]] George Orwell.\n");
    assert!(rendered_paragraphs(&doc)[0].starts_with("[[[1984]]] "));

    non_normative!(
        r#"
|===
a|
include::example$bibliography.adoc[tag=base]
|===

"#
    );

    verifies!(
        r#"
TIP: To escape a bibliography anchor anywhere in the text, use the syntax `[\[[word]]]`.
This prevents the anchor from being matched as a bibliography anchor or a normal anchor.

"#
    );

    // The escape syntax `[\[[word]]]` leaves the triple brackets untouched: it is
    // matched neither as a bibliography anchor nor as a normal anchor.
    let doc =
        Parser::default().parse("[bibliography]\n== References\n\n* [\\[[word]]] Not an anchor.\n");
    let rendered = &rendered_paragraphs(&doc)[0];
    assert!(rendered.contains("[[[word]]]"));
    assert!(!rendered.contains("<a id=\"word\""));

    verifies!(
        r#"
By default, the bibliography anchor and reference to the bibliography entry is converted to `[<label>]`, where <label> is the ID of the entry.
If you specify xreftext on the bibliography anchor (e.g., `+[[[label,xreftext]]]+`), the bibliography anchor and reference to the bibliography entry converts to `[<xreftext>]` instead.

"#
    );

    // By default the anchor and any reference are converted to `[<label>]`. When
    // an xreftext is supplied (`[[[label,xreftext]]]`), `[<xreftext>]` is used
    // instead, both at the entry and at every cross-reference to it.
    let doc = Parser::default().parse(
        "See <<pp>> and <<gof>>.\n\n[bibliography]\n== References\n\n* [[[pp]]] Andy Hunt.\n* [[[gof,gang]]] Erich Gamma.\n",
    );
    let paragraphs = rendered_paragraphs(&doc);
    assert!(paragraphs[0].contains("<a href=\"#pp\">[pp]</a>"));
    assert!(paragraphs[0].contains("<a href=\"#gof\">[gang]</a>"));
    assert!(paragraphs[1].starts_with("<a id=\"pp\"></a>[pp] "));
    assert!(paragraphs[2].starts_with("<a id=\"gof\"></a>[gang] "));

    verifies!(
        r#"
If you want the bibliography anchor and reference to appear as a number, assign the number of the entry using the xreftext.
For example, `+[[[label,1]]]+` will be converted to `[1]`.

"#
    );

    // A numeric xreftext makes the anchor and reference appear as that number.
    let doc = Parser::default()
        .parse("Read <<ref>>.\n\n[bibliography]\n== References\n\n* [[[ref,1]]] An entry.\n");
    let paragraphs = rendered_paragraphs(&doc);
    assert!(paragraphs[0].contains("<a href=\"#ref\">[1]</a>"));
    assert!(paragraphs[1].starts_with("<a id=\"ref\"></a>[1] "));

    non_normative!(
        r#"
If you want more advanced features such as automatic numbering and custom citation styles, try the https://github.com/asciidoctor/asciidoctor-bibtex[asciidoctor-bibtex^] project.
"#
    );
}
