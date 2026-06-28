use crate::tests::prelude::*;

track_file!("docs/modules/blocks/pages/verses.adoc");

fn as_quote<'a>(block: &'a crate::blocks::Block<'a>) -> &'a crate::blocks::QuoteBlock<'a> {
    match block {
        crate::blocks::Block::Quote(quote) => quote,
        other => panic!("expected a quote block, got {other:?}"),
    }
}

non_normative!(
    r#"
= Verses

"#
);

#[test]
fn intro() {
    verifies!(
        r#"
When you need to preserve indents and line breaks, use a `verse` block.
Verses are defined by setting `verse` on a paragraph or an excerpt block delimited by four underscores (`+____+`).

"#
    );

    // A verse can be set on a paragraph by placing the `verse` style in the
    // first position of the attribute list. Line breaks (and leading indents)
    // are preserved verbatim in the rendered `<pre class="content">`.
    let doc = Parser::default().parse("[verse]\nRoses are red,\n  violets are blue.");
    let block = doc.nested_blocks().next().unwrap();
    let quote = as_quote(block);
    assert_eq!(quote.type_(), QuoteType::Verse);
    assert_css(&doc, ".verseblock", 1);
    assert_css(&doc, ".verseblock > pre.content", 1);
    assert_eq!(
        quote.content().unwrap().rendered(),
        "Roses are red,\n  violets are blue."
    );

    // A verse can also be set on an excerpt block delimited by four underscores.
    let doc = Parser::default().parse("[verse]\n____\nRoses are red,\n\nviolets are blue.\n____");
    let block = doc.nested_blocks().next().unwrap();
    let quote = as_quote(block);
    assert_eq!(quote.type_(), QuoteType::Verse);
    assert_css(&doc, ".verseblock", 1);
}

#[test]
fn verse_style_syntax() {
    non_normative!(
        r#"
== verse style syntax

"#
    );

    verifies!(
        r#"
When verse content doesn't contain any empty lines, you can assign the `verse` style using the first position in an attribute list.

.Verse style syntax
[#ex-style]
----
include::example$verse.adoc[tag=para]
----

The result of <<ex-style>> is displayed below.

include::example$verse.adoc[tag=para]

"#
    );

    // The `verse` style in the first position, followed by the attribution and
    // citation positional attributes (`example$verse.adoc[tag=para]`). The
    // single-paragraph content carries no empty lines, so no delimiters are
    // needed.
    let doc = Parser::default().parse(
        "[verse,Carl Sandburg, two lines from the poem Fog]\nThe fog comes\non little cat feet.",
    );

    let block = doc.nested_blocks().next().unwrap();
    let quote = as_quote(block);
    assert_eq!(quote.type_(), QuoteType::Verse);

    // The verse renders inside `<pre class="content">`, preserving the line
    // break between the two lines verbatim.
    assert_css(&doc, ".verseblock", 1);
    assert_css(&doc, ".verseblock > pre.content", 1);
    assert_css(&doc, ".verseblock p", 0);
    assert_eq!(
        quote.content().unwrap().rendered(),
        "The fog comes\non little cat feet."
    );

    // The second and third positional attributes supply the attribution and the
    // citation; both render into a trailing `<div class="attribution">`.
    assert_eq!(quote.attribution(), Some("Carl Sandburg"));
    assert_eq!(quote.citetitle(), Some("two lines from the poem Fog"));
    assert_rendered_contains(&doc, "Carl Sandburg");
    assert_xpath(
        &doc,
        "//div[@class=\"attribution\"]/cite[text()=\"two lines from the poem Fog\"]",
        1,
    );
}

#[test]
fn delimited_verse_block_syntax() {
    non_normative!(
        r#"
== Delimited verse block syntax

"#
    );

    verifies!(
        r#"
When the verse content includes empty lines, enclose it in a delimited excerpt block.

.Verse delimited block syntax
[#ex-block]
----
include::example$verse.adoc[tag=bl]
----

The delimited verse block from <<ex-block>> is rendered below.

include::example$verse.adoc[tag=bl]
"#
    );

    // The delimited excerpt block (four underscores) lets the verse content
    // include empty lines; the `verse` style still applies to the delimited
    // block (`example$verse.adoc[tag=bl]`).
    let doc = Parser::default().parse(
        "[verse,Carl Sandburg,Fog]\n____\nThe fog comes\non little cat feet.\n\nIt sits looking\nover harbor and city\non silent haunches\nand then moves on.\n____",
    );

    let block = doc.nested_blocks().next().unwrap();
    let quote = as_quote(block);
    assert_eq!(quote.type_(), QuoteType::Verse);

    // The verse content is rendered verbatim inside `<pre class="content">`,
    // including the empty line that separates the two stanzas.
    assert_css(&doc, ".verseblock", 1);
    assert_css(&doc, ".verseblock > pre.content", 1);
    assert_eq!(
        quote.content().unwrap().rendered(),
        "The fog comes\non little cat feet.\n\nIt sits looking\nover harbor and city\non silent haunches\nand then moves on."
    );

    // The attribution and citation positional attributes render into the
    // trailing attribution.
    assert_eq!(quote.attribution(), Some("Carl Sandburg"));
    assert_eq!(quote.citetitle(), Some("Fog"));
    assert_xpath(
        &doc,
        "//div[@class=\"attribution\"]/cite[text()=\"Fog\"]",
        1,
    );
}
