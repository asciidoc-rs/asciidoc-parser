use crate::tests::prelude::*;

track_file!("docs/modules/blocks/pages/collapsible.adoc");

non_normative!(
    r#"
= Collapsible Blocks

You can designate block content to be revealed or hidden on interaction using the collapsible option and its companion, the open option.
When the AsciiDoc source is converted to HTML, this block gets mapped to a disclosure summary element (i.e., a summary/details pair).
If the output format does not support this interaction, it may be rendered as an unstyled block (akin to an open block).

"#
);

#[test]
fn collapsible_block_syntax() {
    non_normative!(
        r#"
== Collapsible block syntax

"#
    );

    verifies!(
        r#"
You make block content collapsible by specifying the `collapsible` option on the example structural container.
This option changes the block from an example block to a collapsible block.

.Collapsible block syntax
[#ex-collapsible]
----
include::example$collapsible.adoc[tag=basic]
----

In the output, the content of this block is hidden until the reader clicks the default title, "`Details`".
The result of <<ex-collapsible>> is displayed below.

include::example$collapsible.adoc[tag=basic]

Like other blocks, the collapsible block recognizes the `id` and `role` attributes.

"#
    );

    // The `basic` example tag: the `collapsible` option on an example block
    // turns it into a disclosure element rather than an example block.
    let doc = Parser::default().parse(
        "[%collapsible]\n====\nThis content is only revealed when the user clicks the block title.\n====",
    );

    // The block renders as a `<details>` disclosure, not an example block.
    assert_css(&doc, "details", 1);
    assert_css(&doc, ".exampleblock", 0);

    // The default toggle text is "Details", shown in the `<summary>`.
    assert_xpath(
        &doc,
        "//details/summary[@class=\"title\"][text()=\"Details\"]",
        1,
    );

    // The hidden content is wrapped in `<div class="content">`.
    assert_css(&doc, "details > div.content", 1);
    assert_xpath(
        &doc,
        "//details/div[@class=\"content\"]//p[text()=\"This content is only revealed when the user clicks the block title.\"]",
        1,
    );

    // The disclosure is closed by default (no `open` attribute).
    assert_xpath(&doc, "//details[@open]", 0);

    // Without the `collapsible` option, the same example block renders as an
    // ordinary example block.
    let doc = Parser::default().parse("====\nNot collapsible.\n====");
    assert_css(&doc, "details", 0);
    assert_css(&doc, ".exampleblock", 1);

    // The collapsible block recognizes the `id` and `role` attributes, which
    // travel to the `<details>` element.
    let doc = Parser::default().parse("[#disclosure.scary%collapsible]\n====\nHidden.\n====");
    assert_xpath(&doc, "//details[@id=\"disclosure\"]", 1);
    assert_css(&doc, "details.scary", 1);
}

#[test]
fn collapsible_paragraph_syntax() {
    non_normative!(
        r#"
== Collapsible paragraph syntax

"#
    );

    verifies!(
        r#"
If the content of the block is only a single paragraph, you can use the example paragraph style instead of the example structural container to make a collapsible paragraph.

.Collapsible paragraph syntax
[#ex-collapsible-paragraph]
----
include::example$collapsible.adoc[tag=paragraph]
----

In the output, the content of this block is hidden until the reader clicks the default title, "`Details`".
The result of <<ex-collapsible-paragraph>> is displayed below.

include::example$collapsible.adoc[tag=paragraph]

"#
    );

    // The `paragraph` example tag: the example paragraph style (`example`) plus
    // the `collapsible` option makes a single paragraph collapsible.
    let doc = Parser::default().parse(
        "[example%collapsible]\nThis content is only revealed when the user clicks the block title.",
    );

    assert_css(&doc, "details", 1);
    assert_xpath(
        &doc,
        "//details/summary[@class=\"title\"][text()=\"Details\"]",
        1,
    );

    // A collapsible paragraph's content sits directly in `<div class="content">`,
    // without a wrapping paragraph.
    assert_xpath(
        &doc,
        "//details/div[@class=\"content\"][normalize-space(text())=\"This content is only revealed when the user clicks the block title.\"]",
        1,
    );

    // Inline markup in a collapsible paragraph is rendered (the content is
    // subject to normal substitutions) directly inside the content wrapper.
    let doc = Parser::default().parse("[example%collapsible]\nThis is *bold*.");
    assert_xpath(
        &doc,
        "//details/div[@class=\"content\"]/strong[text()=\"bold\"]",
        1,
    );

    // The example paragraph style is required: a bare `[%collapsible]` paragraph
    // (with no `example` style) is not collapsible; it remains an ordinary
    // paragraph.
    let doc = Parser::default().parse(
        "[%collapsible]\nThis content is only revealed when the user clicks the block title.",
    );
    assert_css(&doc, "details", 0);
    assert_css(&doc, ".paragraph", 1);
}

#[test]
fn customize_the_toggle_text() {
    non_normative!(
        r#"
== Customize the toggle text

"#
    );

    verifies!(
        r#"
If you want to customize the text that toggles the display of the collapsible content, specify a title on the block or paragraph.

.Collapsible block with custom title
[#ex-collapsible-with-title]
----
include::example$collapsible.adoc[tag=title]
----

The result of <<ex-collapsible-with-title>> is displayed below.

include::example$collapsible.adoc[tag=title]

Notice that even though this block has a title, it's not numbered and does not have a caption prefix.
That's because it's not an example block and thus does not get a numbered caption prefix like an example block would.

"#
    );

    // The `title` example tag: a title on the block becomes the toggle text.
    let doc = Parser::default()
        .parse(".Click to reveal the answer\n[%collapsible]\n====\nThis is the answer.\n====");

    // The title is shown verbatim in the `<summary>` as the toggle text ...
    assert_xpath(
        &doc,
        "//details/summary[@class=\"title\"][text()=\"Click to reveal the answer\"]",
        1,
    );

    // ... and it is not numbered nor given a caption prefix: the only `.title`
    // is the summary, with no separate `div.title` caption.
    assert_css(&doc, "div.title", 0);
}

#[test]
fn default_to_open() {
    non_normative!(
        r#"
== Default to open

"#
    );

    verifies!(
        r#"
If you want the collapsible block to be open by default, specify the `open` option as well.

.Collapsible block that defaults to open
[#ex-collapsible-open]
----
include::example$collapsible.adoc[tag=open]
----

The result of <<ex-collapsible-open>> is displayed below.

include::example$collapsible.adoc[tag=open]

"#
    );

    // The `open` example tag: the `open` option expands the disclosure by
    // default, adding the boolean `open` attribute.
    let doc = Parser::default().parse(
        ".Too much detail? Click here.\n[%collapsible%open]\n====\nThis content is revealed by default.\n\nIf it's taking up too much space, the reader can hide it.\n====",
    );

    assert_xpath(&doc, "//details[@open]", 1);
    assert_xpath(
        &doc,
        "//details/summary[@class=\"title\"][text()=\"Too much detail? Click here.\"]",
        1,
    );

    // The enclosed content (two paragraphs) is rendered inside the disclosure.
    assert_css(&doc, "details > div.content > div.paragraph", 2);
}

#[test]
fn use_as_an_enclosure() {
    non_normative!(
        r#"
== Use as an enclosure

"#
    );

    verifies!(
        r#"
Much like the open block, the collapsible block is an enclosure.
If you want to make other types of blocks collapsible, such as an listing block, you can nest the block inside the collapsible block.

.Collapsible block that encloses a literal block
[#ex-collapsible-nested]
----
include::example$collapsible.adoc[tag=nested]
----

The result of <<ex-collapsible-nested>> is displayed below.

include::example$collapsible.adoc[tag=nested]

Since the toggle text acts as the block title, you may decide to not put a title on the nested block, as in this example.
"#
    );

    // The `nested` example tag: a collapsible block enclosing a nested literal
    // block.
    let doc = Parser::default().parse(
        ".Show stacktrace\n[%collapsible]\n====\n....\nError: Content repository not found (url: https://git.example.org/repo.git)\n    at transformGitCloneError\n....\n====",
    );

    assert_css(&doc, "details", 1);
    assert_xpath(
        &doc,
        "//details/summary[@class=\"title\"][text()=\"Show stacktrace\"]",
        1,
    );

    // The nested literal block renders inside the disclosure's content wrapper.
    assert_css(&doc, "details > div.content .literalblock", 1);
    assert_css(&doc, "details > div.content .literalblock pre", 1);
}
