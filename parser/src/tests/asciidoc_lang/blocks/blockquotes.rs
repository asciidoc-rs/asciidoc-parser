use crate::tests::prelude::*;

track_file!("ref/asciidoc-lang/docs/modules/blocks/pages/blockquotes.adoc");

fn as_quote<'a>(block: &'a crate::blocks::Block<'a>) -> &'a crate::blocks::QuoteBlock<'a> {
    match block {
        crate::blocks::Block::Quote(quote) => quote,
        other => panic!("expected a quote block, got {other:?}"),
    }
}

non_normative!(
    r#"
= Blockquotes

Prose excerpts, quotes and verses share the same syntax structure, including:

* block name, either `quote` or `verse`
* name of who the content is attributed to
* bibliographical information of the book, speech, play, poem, etc., where the content was drawn from
* excerpt text

"#
);

#[test]
fn basic_quote_syntax() {
    non_normative!(
        r#"
== Basic quote syntax

// tag::basic[]
"#
    );

    verifies!(
        r#"
For content that doesn't require the preservation of line breaks, set the `quote` attribute in the first position of the attribute list.
Next, set the attribution and relevant citation information.
These positional attributes are all optional.

"#
    );

    // The `quote` style in the first position, followed by the attribution and
    // citation positional attributes.
    let doc = Parser::default().parse(
        "[quote,Abraham Lincoln,Address delivered at Gettysburg]\n____\nFour score and seven years ago\n____",
    );

    assert_css(&doc, ".quoteblock", 1);
    assert_rendered_contains(&doc, "Abraham Lincoln");
    assert_xpath(
        &doc,
        "//div[@class=\"attribution\"]/cite[text()=\"Address delivered at Gettysburg\"]",
        1,
    );

    // The attribution and citation are both optional: a bare `quote` style still
    // produces a quote block, with no attribution.
    let doc = Parser::default().parse("[quote]\n____\nA quote with no attribution.\n____");

    assert_css(&doc, ".quoteblock", 1);
    assert_xpath(&doc, "//div[@class=\"attribution\"]", 0);

    non_normative!(
        r#"
.Anatomy of a basic quote
[#ex-basic]
----
[quote,attribution,citation title and information]
Quote or excerpt text
----

"#
    );

    verifies!(
        r#"
You can include an optional space after the comma that separates each positional attribute.
If an attribute value includes a comma, enclose the value in double or single quotes.

"#
    );

    // An optional space after each comma is allowed and is not part of the
    // attribute value.
    let doc =
        Parser::default().parse("[quote, Albert Einstein, Riddles of the Sphinx]\n____\nx\n____");
    let quote = as_quote(doc.nested_blocks().next().unwrap());
    assert_eq!(quote.attribution(), Some("Albert Einstein"));
    assert_eq!(quote.citetitle(), Some("Riddles of the Sphinx"));

    // A value that itself contains a comma is enclosed in double (or single)
    // quotes so the comma is not read as an attribute separator.
    let doc = Parser::default().parse("[quote,\"Doe, Jane\",A Book]\n____\nx\n____");
    let quote = as_quote(doc.nested_blocks().next().unwrap());
    assert_eq!(quote.attribution(), Some("Doe, Jane"));
    assert_eq!(quote.citetitle(), Some("A Book"));

    let doc = Parser::default().parse("[quote,'Doe, Jane',A Book]\n____\nx\n____");
    let quote = as_quote(doc.nested_blocks().next().unwrap());
    assert_eq!(quote.attribution(), Some("Doe, Jane"));

    verifies!(
        r#"
If the quote is a single line or paragraph (i.e., a styled paragraph), you can place the attribute list directly on top of the text.

"#
    );

    // A styled paragraph: the attribute list sits directly on top of the quote
    // text and yields a quote block with a simple content model.
    let doc = Parser::default()
        .parse("[quote,Captain James T. Kirk]\nEverybody remember where we parked.");

    assert_css(&doc, ".quoteblock", 1);

    let quote = as_quote(doc.nested_blocks().next().unwrap());
    assert_eq!(quote.content_model(), ContentModel::Simple);

    non_normative!(
        r#"
.Quote paragraph syntax
[#ex-style]
----
include::example$quote.adoc[tag=para2-c]
----
"#
    );

    verifies!(
        r#"
<.> Mark lead-in text explaining the context or setting of the quote using a period (`.`). (optional)
<.> For content that doesn't require the preservation of line breaks, set `quote` in the first position of the attribute list.
<.> The second position contains who the excerpt is attributed to. (optional)
<.> Enter additional citation information in the third position. (optional)
<.> Enter the excerpt or quote text on the line immediately following the attribute list.

The result of <<ex-style>> is displayed below.

include::example$quote.adoc[tag=para2]

"#
    );

    // The `para2` example exercises every callout: a lead-in title (the `.`
    // line), the `quote` style with attribution and citation positions, and the
    // quote text on the following line.
    let doc = Parser::default().parse(
        ".After landing the cloaked Klingon bird of prey in Golden Gate park:\n[quote,Captain James T. Kirk,Star Trek IV: The Voyage Home]\nEverybody remember where we parked.",
    );
    let quote = as_quote(doc.nested_blocks().next().unwrap());
    assert_eq!(quote.content_model(), ContentModel::Simple);
    assert_eq!(
        quote.title(),
        Some("After landing the cloaked Klingon bird of prey in Golden Gate park:")
    );
    assert_eq!(quote.attribution(), Some("Captain James T. Kirk"));
    assert_eq!(quote.citetitle(), Some("Star Trek IV: The Voyage Home"));
    assert_eq!(
        quote.content().unwrap().rendered(),
        "Everybody remember where we parked."
    );
}

#[test]
fn quoted_block() {
    non_normative!(
        r#"
== Quoted block

"#
    );

    verifies!(
        r#"
If the quote or excerpt is more than one paragraph, place the text between delimiter lines consisting of four underscores (`+____+`).

"#
    );

    // A quote delimited block contains multiple paragraphs, each rendered inside
    // the blockquote.
    let doc = Parser::default()
        .parse("[quote,Monty Python]\n____\nDennis: Help! I'm being repressed!\n\nKing Arthur: Bloody peasant!\n____");

    assert_css(&doc, ".quoteblock", 1);
    assert_xpath(&doc, "//blockquote/div[@class=\"paragraph\"]/p", 2);

    non_normative!(
        r#"
.Quote block syntax
[#ex-block]
----
include::example$quote.adoc[tag=comp]
----

"#
    );

    verifies!(
        r#"
The result of <<ex-block>> is displayed below.

include::example$quote.adoc[tag=comp]
"#
    );

    // The `comp` example: a multi-paragraph quote block attributed to a source
    // with no citation.
    let doc = Parser::default().parse(
        "[quote,Monty Python and the Holy Grail]\n____\nDennis: Come and see the violence inherent in the system. Help! Help! I'm being repressed!\n\nKing Arthur: Bloody peasant!\n\nDennis: Oh, what a giveaway!\n____",
    );
    let quote = as_quote(doc.nested_blocks().next().unwrap());
    assert_eq!(quote.content_model(), ContentModel::Compound);
    assert_eq!(quote.attribution(), Some("Monty Python and the Holy Grail"));
    assert!(quote.citetitle().is_none());
    assert_eq!(quote.blocks().len(), 3);

    non_normative!(
        r#"
// end::basic[]

"#
    );
}

#[test]
fn quoted_paragraph() {
    non_normative!(
        r#"
== Quoted paragraph

"#
    );

    verifies!(
        r#"
You can turn a single paragraph into a blockquote by:

. surrounding it with double quotes
. adding an optional attribution (prefixed with two dashes) below the quoted text

"#
    );

    // A paragraph wrapped in double quotes, with an attribution line introduced
    // by two dashes, becomes a quote block.
    let doc = Parser::default().parse(
        "\"I hold it that a little rebellion now and then is a good thing.\"\n-- Thomas Jefferson, Papers of Thomas Jefferson: Volume 11",
    );

    assert_css(&doc, ".quoteblock", 1);
    assert_rendered_contains(&doc, "Thomas Jefferson");
    assert_xpath(
        &doc,
        "//div[@class=\"attribution\"]/cite[text()=\"Papers of Thomas Jefferson: Volume 11\"]",
        1,
    );

    // The double quotes wrap the blockquote text itself, which is rendered
    // without them.
    refute_rendered_contains(&doc, "\"I hold it");

    non_normative!(
        r#"
.Quoted paragraph syntax
[#ex-quoted]
----
include::example$quote.adoc[tag=abbr]
----

"#
    );

    verifies!(
        r#"
The result of <<ex-quoted>> is displayed below.

include::example$quote.adoc[tag=abbr]

"#
    );

    // The `abbr` example: a two-line quoted paragraph with an attribution and
    // citation supplied on the `--` line.
    let doc = Parser::default().parse(
        "\"I hold it that a little rebellion now and then is a good thing,\nand as necessary in the political world as storms in the physical.\"\n-- Thomas Jefferson, Papers of Thomas Jefferson: Volume 11",
    );
    let quote = as_quote(doc.nested_blocks().next().unwrap());
    assert_eq!(quote.content_model(), ContentModel::Simple);
    assert_eq!(quote.attribution(), Some("Thomas Jefferson"));
    assert_eq!(
        quote.citetitle(),
        Some("Papers of Thomas Jefferson: Volume 11")
    );
}

#[test]
fn excerpt() {
    non_normative!(
        r#"
== Excerpt

"#
    );

    verifies!(
        r#"
The quote block can be designated as an excerpt by adding the `excerpt` role.
The exceptation is that this role makes the quote block appear with the quote decoration.

"#
    );

    // The `excerpt` role is carried through to the rendered block as a CSS class
    // on the quote block.
    let doc = Parser::default().parse("[.excerpt]\n____\nThis text is an excerpt.\n____");

    assert_css(&doc, ".quoteblock.excerpt", 1);

    non_normative!(
        r#"
[source]
----
[.excerpt]
____
This text is an excerpt from the referenced literature.
____
----

The impact of this role is strictly a presentation concern and is thus handled by the styling system, such as the stylesheet for HTML.

"#
    );
}

#[test]
fn markdown_style_blockquotes() {
    non_normative!(
        r#"
== Markdown-style blockquotes

"#
    );

    verifies!(
        r#"
Asciidoctor supports Markdown-style blockquotes.
This syntax was adopted both to ease the transition from Markdown and because it's the most common method of quoting in email messages.

"#
    );

    // A run of lines introduced by a `>` marker produces a quote block.
    let doc = Parser::default().parse(
        "> I hold it that a little rebellion now and then is a good thing,\n> and as necessary in the political world as storms in the physical.\n> -- Thomas Jefferson, Papers of Thomas Jefferson: Volume 11",
    );

    assert_css(&doc, ".quoteblock", 1);
    assert_xpath(&doc, "//blockquote/div[@class=\"paragraph\"]/p", 1);
    assert_xpath(&doc, "//div[@class=\"attribution\"]/cite", 1);

    non_normative!(
        r#"
.Markdown-style blockquote syntax
[source#ex-md]
----
include::example$quote.adoc[tag=md]
----

"#
    );

    verifies!(
        r#"
The result of <<ex-md>> is displayed below.

include::example$quote.adoc[tag=md]

"#
    );

    // The `md` example renders as a quote block whose `-- ` line becomes the
    // attribution.
    let doc = Parser::default().parse(
        "> I hold it that a little rebellion now and then is a good thing,\n> and as necessary in the political world as storms in the physical.\n> -- Thomas Jefferson, Papers of Thomas Jefferson: Volume 11",
    );
    let quote = as_quote(doc.nested_blocks().next().unwrap());
    assert_eq!(quote.attribution(), Some("Thomas Jefferson"));
    assert_eq!(
        quote.citetitle(),
        Some("Papers of Thomas Jefferson: Volume 11")
    );

    verifies!(
        r#"
Like Markdown, Asciidoctor supports some block content inside the blockquote, including paragraphs, lists, and nested blockquotes.

"#
    );

    // Markdown-style blockquotes support nested block content, including nested
    // blockquotes and lists.
    let doc = Parser::default().parse(
        "> > What's new?\n>\n> I've got Markdown in my AsciiDoc!\n>\n> * Blockquotes\n> * Headings",
    );

    // The outer blockquote plus the nested blockquote.
    assert_css(&doc, ".quoteblock", 2);
    assert_css(&doc, ".ulist", 1);
    assert_xpath(&doc, "//ul/li", 2);

    non_normative!(
        r#"
.Markdown-style blockquote containing block content
[source#ex-md-block]
....
include::example$quote.adoc[tag=md-alt]
....

"#
    );

    verifies!(
        r#"
Here's how the conversation from <<ex-md-block>> is rendered.

include::example$quote.adoc[tag=md-alt]

"#
    );

    // The `md-alt` example: nested blockquotes and a list inside a Markdown
    // blockquote. The outer blockquote plus the three nested ones make four
    // quote blocks in all.
    let doc = Parser::default().parse(
        "> > What's new?\n>\n> I've got Markdown in my AsciiDoc!\n>\n> > Like what?\n>\n> * Blockquotes\n> * Headings\n> * Fenced code blocks\n>\n> > Is there more?\n>\n> Yep. AsciiDoc and Markdown share a lot of common syntax already.",
    );
    assert_css(&doc, ".quoteblock", 4);
    assert_css(&doc, ".ulist", 1);

    verifies!(
        r#"
Be aware that not all AsciiDoc block elements are supported inside a Markdown-style blockquote.
In particular, a description list is not permitted.
The parser looks for the Markdown-style blockquote only after looking for a description list, meaning the description list takes precedence.
Since the quote marker is a valid prefix for a description list term, the Markdown-style blockquote is not recognized in this case.
If you need to put a description list inside a blockquote, you should use the AsciiDoc syntax for a blockquote instead.

"#
    );

    // A `>`-prefixed line that is also a valid description-list term is parsed as
    // a description list, not a Markdown-style blockquote.
    let doc = Parser::default().parse("> term:: definition");

    assert_css(&doc, ".dlist", 1);
    assert_css(&doc, ".quoteblock", 0);

    non_normative!(
        r#"
The Markdown-style blockquote should only be used in simple cases and when migrating from Markdown.
The AsciiDoc syntax should always be preferred, if possible.
"#
    );
}
