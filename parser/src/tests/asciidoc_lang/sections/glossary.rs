use crate::tests::prelude::*;

track_file!("ref/asciidoc-lang/docs/modules/sections/pages/glossary.adoc");

non_normative!(
    r#"
= Glossary

"#
);

#[test]
fn intro() {
    verifies!(
        r#"
You can include a glossary in an article, book, and book part by setting the `glossary` style on a section.

"#
    );

    // A section assigned the `glossary` style carries that style.
    let doc = Parser::default().parse("[glossary]\n== Terminology\n");
    let section = doc.child_blocks().next().unwrap();
    assert_eq!(section.declared_style(), Some("glossary"));
}

#[test]
fn glossary_section_syntax() {
    verifies!(
        r#"
== Glossary section syntax

"#
    );

    // The `glossary` style is recognized on a level 1 section.
    let doc = Parser::default().parse("[glossary]\n== Terminology\n");
    let section = doc.child_blocks().next().unwrap();
    assert_eq!(section.declared_style(), Some("glossary"));
    assert_eq!(section.raw_context().as_ref(), "section");

    // The `doctype` and book-part conditions that select level 1 are
    // non-normative for this crate, which models a single article-style
    // document. The `glossary` style is still recognized on the section.
    non_normative!(
        r#"
The glossary section is defined as a level 1 section (`==`) when:

* the doctype is `article`
* the doctype is `book` and the book doesn't contain any parts
* the glossary is for a book part

"#
    );

    verifies!(
        r#"
[source]
----
[glossary]
== Terminology
----

"#
    );

    // The `glossary` section style is recognized as a level 1 section.
    let doc = Parser::default().parse("[glossary]\n== Terminology\n");
    let section = doc.child_blocks().next().unwrap();
    assert_eq!(section.declared_style(), Some("glossary"));
    assert_eq!(section.raw_context().as_ref(), "section");

    // The level 0 (`=`) form applies only to a multi-part book, which this
    // crate does not model: a level 0 heading is reserved for the document
    // title. The remaining guidance is therefore non-normative here.
    non_normative!(
        r#"
If the book has parts, and the glossary is for the whole book, the section is defined as a level 0 section (`=`).

[source]
----
[glossary]
= Glossary
----

"#
    );
}

#[test]
fn glossary_description_list_style_syntax() {
    verifies!(
        r#"
== Glossary description list style syntax

In addition to setting `glossary` on the section, the block style `glossary` must be set on the description list in the section.

"#
    );

    verifies!(
        r#"
[source]
----
[glossary]
== Glossary

[glossary]
mud:: wet, cold dirt
rain::
	water falling from the sky
----
"#
    );

    // Setting `glossary` on both the section and the description list within it:
    // the section carries the `glossary` style, and the description list renders
    // with the `glossary` style class. Per the spec, the section style is not
    // implicitly propagated to the list; the list must declare it explicitly.
    let doc = Parser::default().parse(
        "[glossary]\n== Glossary\n\n[glossary]\nmud:: wet, cold dirt\nrain::\n\twater falling from the sky\n",
    );

    let section = doc.child_blocks().next().unwrap();
    assert_eq!(section.declared_style(), Some("glossary"));

    let dlist = section.child_blocks().next().unwrap();
    assert_eq!(dlist.declared_style(), Some("glossary"));

    assert_css(&doc, ".dlist.glossary", 1);
    assert_css(&doc, ".dlist dt:not([class])", 2);
}
