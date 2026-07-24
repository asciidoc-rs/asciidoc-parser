use crate::tests::prelude::*;

track_file!("ref/asciidoc-lang/docs/modules/blocks/pages/open-blocks.adoc");

non_normative!(
    r#"
= Open Blocks

"#
);

#[test]
fn open_block_is_a_generic_container() {
    verifies!(
        r#"
The most versatile block of all is the open block.
It allows you to apply block attributes to a chunk of text without giving it any other semantics.
In other words, it provides a generic structural container for enclosing content.
The only notable limitation is that an open block cannot be nested inside of another open block.

"#
    );

    // An open block is a generic structural container: the `--` delimiters wrap
    // their content in `div.openblock > div.content`, adding no other semantics.
    let doc = Parser::default().parse("--\nGeneric content.\n--");
    assert_css(&doc, "div.openblock > div.content", 1);
    assert_css(&doc, "div.openblock > div.content > div.paragraph", 1);

    // The block's raw context is `open`, and (absent a masquerade style) its
    // resolved context is `open` as well.
    let open = doc.child_blocks().next().unwrap();
    assert_eq!(open.raw_context().as_ref(), "open");
    assert_eq!(open.resolved_context().as_ref(), "open");

    // An open block cannot be nested inside another open block: a second `--`
    // line closes the first container rather than opening a child. Given four
    // `--` lines, the parser pairs them into two sibling open blocks, with the
    // intervening text left as a top-level paragraph — never an open block
    // within an open block.
    let doc = Parser::default().parse("--\nouter\n--\n\ninner\n\n--\nafter\n--");
    assert_css(&doc, "div.openblock", 2);
    assert_css(&doc, "div.openblock div.openblock", 0);
}

#[test]
fn open_block_syntax() {
    non_normative!(
        r#"
== Open block syntax

.Open block syntax
[#ex-open]
----
include::example$open.adoc[tag=base]
----

The result of <<ex-open>> is displayed below.

include::example$open.adoc[tag=base]

"#
    );

    // The `base` example: a pair of `--` delimiters forms an anonymous open
    // block, rendered as `div.openblock > div.content` wrapping the paragraph.
    let doc = Parser::default().parse(
        "--\nAn open block can be an anonymous container,\nor it can masquerade as any other block.\n--",
    );

    assert_css(&doc, ".openblock", 1);
    assert_css(&doc, "div.openblock > div.content", 1);
    assert_css(&doc, "div.openblock > div.content > div.paragraph > p", 1);
    assert_xpath(
        &doc,
        "//div[@class=\"openblock\"]/div[@class=\"content\"]/div[@class=\"paragraph\"]/p[contains(text(), \"anonymous container\")]",
        1,
    );
}

#[test]
fn open_block_masquerading_as_a_sidebar() {
    verifies!(
        r#"
An open block can act as any other paragraph or delimited block, with the exception of _pass_ and _table_.
For instance, in <<ex-open-sidebar>> an open block is acting as a sidebar.

"#
    );

    verifies!(
        r#"
.Open block masquerading as a sidebar
[#ex-open-sidebar]
----
include::example$open.adoc[tag=sb]
----

The result of <<ex-open-sidebar>> is displayed below.

include::example$open.adoc[tag=sb]

"#
    );

    // The `sb` example: a `[sidebar]` style on an open block makes it act as a
    // sidebar. The block adopts the sidebar context — `div.sidebarblock >
    // div.content` — with the title rendered as the first child of the content
    // wrapper, exactly as a `****` sidebar would render. No open block remains.
    let doc = Parser::default().parse(
        "[sidebar]\n.Related information\n--\nThis is aside text.\n\nIt is used to present information related to the main content.\n--",
    );

    assert_css(&doc, ".openblock", 0);
    assert_css(&doc, "div.sidebarblock > div.content", 1);
    assert_xpath(
        &doc,
        "//div[@class=\"sidebarblock\"]/div[@class=\"content\"]/div[@class=\"title\"][text()=\"Related information\"]",
        1,
    );
    assert_css(&doc, "div.sidebarblock > div.content > div.paragraph", 2);

    // The masquerade style replaces the open context: the resolved context is
    // `sidebar` even though the raw (delimiter-derived) context is `open`.
    let block = doc.child_blocks().next().unwrap();
    assert_eq!(block.raw_context().as_ref(), "open");
    assert_eq!(block.resolved_context().as_ref(), "sidebar");
}

#[test]
fn open_block_masquerading_as_a_source_block() {
    verifies!(
        r#"
<<ex-open-source>> is an open block acting as a source block.

"#
    );

    verifies!(
        r#"
.Open block masquerading as a source block
[#ex-open-source]
----
include::example$open.adoc[tag=src]
----

The result of <<ex-open-source>> is displayed below.

include::example$open.adoc[tag=src]
"#
    );

    // The `src` example: a `[source]` style on an open block makes it act as a
    // source block. The open block adopts the verbatim listing context and
    // renders as `div.listingblock` with the body preserved verbatim inside
    // `pre > code`, exactly as a `----` source block would. No open block
    // remains.
    let doc = Parser::default().parse("[source]\n--\nputs \"I'm a source block!\"\n--");

    assert_css(&doc, ".openblock", 0);
    assert_css(&doc, "div.listingblock", 1);
    assert_xpath(
        &doc,
        "//div[@class=\"listingblock\"]//pre/code[contains(text(), \"I'm a source block!\")]",
        1,
    );

    // The body is verbatim: the content model is verbatim, not the compound
    // (nested-block) model of a plain open block.
    let block = doc.child_blocks().next().unwrap();
    assert_eq!(block.content_model(), ContentModel::Verbatim);
}

// The following tests are not tied to a specific line of the spec page. They
// cover the remaining masquerade forms implied by "can act as any other
// paragraph or delimited block" (line 20): the `[source]` example above is one
// of several verbatim/compound masquerades that an open block honors, none of
// which any other delimited block does.

#[test]
fn open_block_carries_id_and_roles() {
    // Like other blocks, an open block recognizes the `id` and `role`
    // attributes; the id and roles travel to the outer `div.openblock`, and an
    // optional title is rendered as a `div.title` before the content wrapper.
    let doc = Parser::default().parse(".My open block\n[#myid.myrole]\n--\nbody.\n--");
    assert_xpath(&doc, "//div[@id=\"myid\"]", 1);
    assert_css(&doc, "div.openblock.myrole", 1);
    assert_xpath(
        &doc,
        "//div[@class=\"openblock myrole\"]/div[@class=\"title\"][text()=\"My open block\"]",
        1,
    );
    // The title precedes the content wrapper (it is not inside it).
    assert_css(&doc, "div.openblock > div.title + div.content", 1);
}

#[test]
fn open_block_masquerading_as_a_listing_or_literal_block() {
    // A `[listing]` style on an open block makes it act as a listing block: the
    // body is verbatim inside `div.listingblock > pre`.
    let doc = Parser::default().parse("[listing]\n--\nlisting body\n--");
    assert_css(&doc, ".openblock", 0);
    assert_css(&doc, "div.listingblock > pre", 1);
    assert_eq!(
        doc.child_blocks().next().unwrap().content_model(),
        ContentModel::Verbatim
    );

    // A `[literal]` style on an open block makes it act as a literal block,
    // overriding the usual treatment of `[literal]` as a paragraph style.
    let doc = Parser::default().parse("[literal]\n--\nliteral body\n--");
    assert_css(&doc, ".openblock", 0);
    assert_css(&doc, "div.literalblock > pre", 1);
    assert_eq!(
        doc.child_blocks().next().unwrap().content_model(),
        ContentModel::Verbatim
    );
}

#[test]
fn open_block_masquerading_as_a_quote_or_verse_block() {
    // A `[quote]` style on an open block makes it act as a quote block: the body
    // is a compound (nested-block) content model rendered inside a
    // `div.quoteblock > blockquote`, with the attribution surfaced.
    let doc = Parser::default().parse("[quote,Author]\n--\nQuoted open block.\n--");
    assert_css(&doc, ".openblock", 0);
    assert_css(&doc, "div.quoteblock > blockquote", 1);

    // A `[verse]` style on an open block makes it act as a verse block, with the
    // body preserved verbatim inside `pre.content`.
    let doc = Parser::default().parse("[verse]\n--\nverse line\n--");
    assert_css(&doc, ".openblock", 0);
    assert_css(&doc, "div.verseblock > pre.content", 1);
}

#[test]
fn masquerade_style_only_applies_to_an_open_block() {
    // A masquerade style replaces the context only on an open block. On any other
    // delimited block the delimiter's own context wins and the style is ignored:
    // a `[quote]` style on an example block (`====`) stays an example block.
    let doc = Parser::default().parse("[quote]\n====\nx\n====");
    assert_css(&doc, "div.exampleblock", 1);
    assert_css(&doc, "div.quoteblock", 0);
}
