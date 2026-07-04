use crate::tests::prelude::*;

track_file!("ref/asciidoc-lang/docs/modules/blocks/pages/sidebars.adoc");

non_normative!(
    r#"
= Sidebars
// Moved upstream from the Antora documentation at docs.antora.org

On this page, you'll learn:

* [x] How to mark up a sidebar with AsciiDoc.

"#
);

#[test]
fn sidebar_content_and_substitutions() {
    verifies!(
        r#"
A sidebar can contain any type of content, such as quotes, equations, and images.
Normal substitutions are applied to sidebar content.

"#
    );

    // Normal substitutions are applied to sidebar content: inline markup in the
    // body of a sidebar is rendered, just as it would be in a paragraph.
    let doc = Parser::default().parse("[sidebar]\nThis is *bold* content.");
    assert_xpath(
        &doc,
        "//div[@class=\"sidebarblock\"]/div[@class=\"content\"]/strong[text()=\"bold\"]",
        1,
    );
}

#[test]
fn sidebar_style_syntax() {
    non_normative!(
        r#"
== Sidebar style syntax

"#
    );

    verifies!(
        r#"
If the sidebar content is contiguous, the block style `sidebar` can be placed directly on top of the text in an attribute list (`[]`).

.Assign sidebar block style to paragraph
[#ex-style]
----
[sidebar]
Sidebars are used to visually separate auxiliary bits of content
that supplement the main text.
----

The result of <<ex-style>> is displayed below.

[sidebar]
Sidebars are used to visually separate auxiliary bits of content that supplement the main text.

"#
    );

    // The `sidebar` block style on a contiguous paragraph produces a sidebar
    // block: `div.sidebarblock` wrapping a single `div.content`.
    let doc = Parser::default().parse(
        "[sidebar]\nSidebars are used to visually separate auxiliary bits of content\nthat supplement the main text.",
    );

    assert_css(&doc, ".sidebarblock", 1);
    assert_css(&doc, "div.sidebarblock > div.content", 1);

    // The paragraph's content sits directly inside `div.content`, without a
    // wrapping `<p>` element.
    assert_css(&doc, "div.sidebarblock > div.content > p", 0);
    assert_xpath(
        &doc,
        "//div[@class=\"sidebarblock\"]/div[@class=\"content\"][contains(normalize-space(text()), \"Sidebars are used to visually separate auxiliary bits of content that supplement the main text.\")]",
        1,
    );

    // A title on a sidebar paragraph is rendered as the first child of
    // `div.content`, and it survives alongside inline markup in the body: the
    // title `div.title` and the rendered `<strong>` are siblings inside the
    // content wrapper.
    let doc = Parser::default().parse(".My Title\n[sidebar]\nThis is *bold* content.");
    assert_xpath(
        &doc,
        "//div[@class=\"sidebarblock\"]/div[@class=\"content\"]/div[@class=\"title\"][text()=\"My Title\"]",
        1,
    );
    assert_xpath(
        &doc,
        "//div[@class=\"sidebarblock\"]/div[@class=\"content\"]/strong[text()=\"bold\"]",
        1,
    );
}

#[test]
fn delimited_sidebar_syntax() {
    non_normative!(
        r#"
== Delimited sidebar syntax

"#
    );

    verifies!(
        r#"
A delimited sidebar block is delimited by a pair of four consecutive asterisks (`pass:[****]`).
You don't need to set the style name when you use the sidebar's delimiters.

.Sidebar block delimiter syntax
[#ex-block]
------
.Optional Title
****
Sidebars are used to visually separate auxiliary bits of content
that supplement the main text.

TIP: They can contain any type of content.

.Source code block in a sidebar
[source,js]
----
const { expect, expectCalledWith, heredoc } = require('../test/test-utils')
----
****
------

The result of <<ex-block>> is displayed below.

.Optional Title
****
Sidebars are used to visually separate auxiliary bits of content that supplement the main text.

TIP: They can contain any type of content.

.Source code block in a sidebar
[source,js]
----
const { expect, expectCalledWith, heredoc } = require('../test/test-utils')
----
****
"#
    );

    // A pair of four consecutive asterisks delimits a sidebar block; no `sidebar`
    // style is needed. The block can contain any type of content (here: a title,
    // a paragraph, a TIP admonition, and a source block).
    let doc = Parser::default().parse(
        ".Optional Title\n****\nSidebars are used to visually separate auxiliary bits of content\nthat supplement the main text.\n\nTIP: They can contain any type of content.\n\n.Source code block in a sidebar\n[source,js]\n----\nconst { expect, expectCalledWith, heredoc } = require('../test/test-utils')\n----\n****",
    );

    assert_css(&doc, ".sidebarblock", 1);
    assert_css(&doc, "div.sidebarblock > div.content", 1);

    // The optional title is rendered as the first child of `div.content` (a
    // `div.title`), not as a numbered caption outside the block.
    assert_xpath(
        &doc,
        "//div[@class=\"sidebarblock\"]/div[@class=\"content\"]/div[@class=\"title\"][text()=\"Optional Title\"]",
        1,
    );

    // The body paragraph is wrapped in `div.paragraph` inside the content.
    assert_css(&doc, "div.sidebarblock > div.content > div.paragraph", 1);

    // The TIP admonition renders inside the sidebar content.
    assert_css(
        &doc,
        "div.sidebarblock > div.content .admonitionblock.tip",
        1,
    );

    // The nested source block renders inside the sidebar content, carrying its
    // own title and a `<pre><code>` with the source language.
    assert_css(&doc, "div.sidebarblock > div.content .listingblock", 1);
    assert_xpath(
        &doc,
        "//div[@class=\"sidebarblock\"]//pre/code[@data-lang=\"js\"]",
        1,
    );
}
