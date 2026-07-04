use crate::tests::prelude::*;

track_file!("ref/asciidoc-lang/docs/modules/blocks/pages/example-blocks.adoc");

non_normative!(
    r#"
= Example Blocks
// Moved upstream from the Antora documentation at docs.antora.org
:!example-caption:

On this page, you'll learn:

* [x] How to mark up an example block with AsciiDoc.

"#
);

#[test]
fn example_content_and_substitutions() {
    verifies!(
        r#"
An example block is useful for visually delineating content that illustrates a concept or showing the result of an operation.

An example can contain any type of content and AsciiDoc syntax.
Normal substitutions are applied to example content.

"#
    );

    // Normal substitutions are applied to example content: inline markup in the
    // body of an example block is rendered, just as it would be in a paragraph.
    let doc = Parser::default().parse("====\nThis is *bold* content.\n====");
    assert_xpath(
        &doc,
        "//div[@class=\"exampleblock\"]/div[@class=\"content\"]//strong[text()=\"bold\"]",
        1,
    );

    // An example can contain any type of content and AsciiDoc syntax. Here the
    // block encloses a paragraph, an admonition, and a nested unordered list;
    // each renders inside the example's content wrapper.
    let doc = Parser::default().parse("====\nA paragraph.\n\nTIP: A tip.\n\n* one\n* two\n====");
    assert_css(&doc, "div.exampleblock > div.content > div.paragraph", 1);
    assert_css(
        &doc,
        "div.exampleblock > div.content .admonitionblock.tip",
        1,
    );
    assert_css(&doc, "div.exampleblock > div.content .ulist", 1);
}

#[test]
fn example_style_syntax() {
    non_normative!(
        r#"
== Example style syntax

"#
    );

    verifies!(
        r#"
If the example content is contiguous, i.e., not interrupted by empty lines, the block style name `example` can be placed directly on top of the text in an attribute list (`[]`).

.Assign example block style to paragraph
[#ex-style]
----
.Optional title
[example]
This is an example of an example block.
----

The result of <<ex-style>> is displayed below.

.Optional title
[example]
This is an example of an example block.

"#
    );

    // The `example` block style on a contiguous paragraph produces an example
    // block: `div.exampleblock` wrapping a single `div.content`.
    let doc = Parser::default().parse("[example]\nThis is an example of an example block.");

    assert_css(&doc, ".exampleblock", 1);
    assert_css(&doc, "div.exampleblock > div.content", 1);

    // The paragraph's content sits directly inside `div.content`, without a
    // wrapping `<p>` element.
    assert_css(&doc, "div.exampleblock > div.content > p", 0);
    assert_xpath(
        &doc,
        "//div[@class=\"exampleblock\"]/div[@class=\"content\"][normalize-space(text())=\"This is an example of an example block.\"]",
        1,
    );

    // The optional title renders as a `div.title` caption that precedes the
    // content wrapper. On this page `example-caption` is unset, so the title is
    // shown without a number, exactly as in the page's rendered result.
    let doc = Parser::default().parse(
        ":!example-caption:\n\n.Optional title\n[example]\nThis is an example of an example block.",
    );
    assert_xpath(
        &doc,
        "//div[@class=\"exampleblock\"]/div[@class=\"title\"][text()=\"Optional title\"]",
        1,
    );
}

#[test]
fn delimited_example_syntax() {
    non_normative!(
        r#"
[#delimited]
== Delimited example syntax

"#
    );

    verifies!(
        r#"
If the example content includes multiple blocks or content separated by empty lines, place the content between delimiter lines consisting of four equals signs (`pass:[====]`).

You don't need to set the block style name when you use the example delimiters.

.Example block delimiter syntax
[#ex-block]
----
.Onomatopoeia
====
The book hit the floor with a *thud*.

He could hear doves *cooing* in the pine trees`' branches.
====
----

The result of <<ex-block>> is displayed below.

.Onomatopoeia
====
The book hit the floor with a *thud*.

He could hear doves *cooing* in the pine trees`' branches.
====

"#
    );

    // A pair of four equals signs delimits an example block; no `example` style
    // is needed. The block can hold content separated by empty lines (here two
    // paragraphs), each wrapped in `div.paragraph` inside the content wrapper.
    let doc = Parser::default().parse(
        ":!example-caption:\n\n.Onomatopoeia\n====\nThe book hit the floor with a *thud*.\n\nHe could hear doves *cooing* in the pine trees`' branches.\n====",
    );

    assert_css(&doc, ".exampleblock", 1);
    assert_css(&doc, "div.exampleblock > div.content", 1);
    assert_css(&doc, "div.exampleblock > div.content > div.paragraph", 2);

    // The optional title renders as a `div.title` caption before the content
    // wrapper (with `example-caption` unset, no number is prepended).
    assert_xpath(
        &doc,
        "//div[@class=\"exampleblock\"]/div[@class=\"title\"][text()=\"Onomatopoeia\"]",
        1,
    );

    // Inline markup in the body is rendered (normal substitutions apply): the
    // `*thud*` and `*cooing*` become `<strong>` elements, and the curved-quote
    // shorthand becomes a typographic apostrophe.
    assert_xpath(
        &doc,
        "//div[@class=\"exampleblock\"]//div[@class=\"paragraph\"]//strong[text()=\"thud\"]",
        1,
    );
    assert_xpath(
        &doc,
        "//div[@class=\"exampleblock\"]//div[@class=\"paragraph\"]//strong[text()=\"cooing\"]",
        1,
    );

    non_normative!(
        r#"
TIP: xref:admonitions.adoc[Complex admonitions] use the delimited example syntax.
"#
    );
}

// The following tests are not tied to a specific line of the spec page; they
// cover the default example-block caption numbering (the behavior that the page
// suppresses with `:!example-caption:`).

#[test]
fn titled_example_block_is_numbered() {
    // By default `example-caption` is set to "Example", so a titled example
    // block is captioned with an automatically incremented number.
    let doc = Parser::default().parse(".Onomatopoeia\n====\nbody.\n====");
    assert_xpath(
        &doc,
        "//div[@class=\"exampleblock\"]/div[@class=\"title\"][text()=\"Example 1. Onomatopoeia\"]",
        1,
    );

    // The example paragraph style shares the same document-wide counter as the
    // delimited form, and an untitled example consumes no number.
    let doc = Parser::default()
        .parse("====\nuntitled.\n====\n\n.First\n====\none.\n====\n\n.Second\n[example]\ntwo.");
    assert_xpath(
        &doc,
        "//div[@class=\"title\"][text()=\"Example 1. First\"]",
        1,
    );
    assert_xpath(
        &doc,
        "//div[@class=\"title\"][text()=\"Example 2. Second\"]",
        1,
    );

    // A nested example block is numbered before its container (parse-completion
    // order), so the inner example takes the lower number.
    let doc = Parser::default().parse(".Outer\n====\nouter.\n\n.Inner\n=====\ninner.\n=====\n====");
    assert_xpath(
        &doc,
        "//div[@class=\"title\"][text()=\"Example 1. Inner\"]",
        1,
    );
    assert_xpath(
        &doc,
        "//div[@class=\"title\"][text()=\"Example 2. Outer\"]",
        1,
    );
}

#[test]
fn example_caption_can_be_customized() {
    // A custom `example-caption` value replaces the "Example" label.
    let doc = Parser::default().parse(":example-caption: Exhibit\n\n.Foo\n====\nbar.\n====");
    assert_xpath(
        &doc,
        "//div[@class=\"exampleblock\"]/div[@class=\"title\"][text()=\"Exhibit 1. Foo\"]",
        1,
    );
}

#[test]
fn example_block_carries_id_and_roles() {
    // Like other blocks, an example block recognizes the `id` and `role`
    // attributes; the id and roles travel to the outer `div.exampleblock`.
    let doc = Parser::default().parse("[#myid.myrole]\n====\nbody.\n====");
    assert_xpath(&doc, "//div[@id=\"myid\"]", 1);
    assert_css(&doc, "div.exampleblock.myrole", 1);

    // The same attributes are honored on the example paragraph style.
    let doc = Parser::default().parse("[#pid.prole]\n[example]\nbody.");
    assert_xpath(&doc, "//div[@id=\"pid\"]", 1);
    assert_css(&doc, "div.exampleblock.prole", 1);
}
