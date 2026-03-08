use crate::{
    blocks::{ContentModel, SimpleBlockStyle},
    content::SubstitutionGroup,
    tests::prelude::*,
};

track_file!("docs/modules/blocks/pages/add-title.adoc");

non_normative!(
    r#"
= Add a Title to a Block

You can assign a title to a block, whether it's styled using its style name or delimiters.

"#
);

#[test]
fn block_title_syntax() {
    verifies!(
        r#"
== Block title syntax

A block title is defined on its own line directly above the block's attribute list, opening delimiter, or block content--which ever comes first.
As shown in <<ex-basic>>, the line must begin with a dot (`.`) and immediately be followed by the text of the title.
The block title must only occupy a single line and thus cannot be wrapped.

.Block title syntax
[#ex-basic]
----
.This is the title of a sidebar block
****
This is the content of the sidebar block.
****
----

"#
    );

    let mut parser = Parser::default();

    let block = crate::blocks::Block::parse(crate::Span::new(
        ".This is the title of a sidebar block\n****\nThis is the content of the sidebar block.\n****\n",
    ), &mut parser)
    .unwrap_if_no_warnings()
    .unwrap()
    .item;

    assert_eq!(
        block,
        Block::CompoundDelimited(CompoundDelimitedBlock {
            blocks: &[Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "This is the content of the sidebar block.",
                        line: 3,
                        col: 1,
                        offset: 43,
                    },
                    rendered: "This is the content of the sidebar block.",
                },
                source: Span {
                    data: "This is the content of the sidebar block.",
                    line: 3,
                    col: 1,
                    offset: 43,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },),],
            context: "sidebar",
            source: Span {
                data: ".This is the title of a sidebar block\n****\nThis is the content of the sidebar block.\n****",
                line: 1,
                col: 1,
                offset: 0,
            },
            title_source: Some(Span {
                data: "This is the title of a sidebar block",
                line: 1,
                col: 2,
                offset: 1,
            },),
            title: Some("This is the title of a sidebar block"),
            anchor: None,
            anchor_reftext: None,
            attrlist: None,
        },)
    );
}

non_normative!(
    r#"
CAUTION: The block title line should not be confused with an ordered list item that uses the `.` marker.
A block title line has no space after the `.`, whereas the space after a list marker is required.

The next sections will show how to add titles to delimited blocks and blocks with attribute lists.

"#
);

#[test]
fn add_title_to_delimited_block() {
    verifies!(
        r#"
== Add a title to a delimited block

Any delimited block can have a title.
If the block doesn't have an attribute list, enter the title on a new line directly above the opening delimiter.
The delimited literal block in <<ex-title>> is titled _Terminal Output_.

.Add a title to a delimited block
[#ex-title]
----
.Terminal Output <.>
.... <.>
From github.com:asciidoctor/asciidoctor
 * branch        main   -> FETCH_HEAD
Already up to date.
....
----
<.> The block title is entered on a new line.
The title must begin with a dot (`.`).
Don't put a space between the dot and the first character of the title.
<.> If you aren't applying attributes to a block, enter the opening delimiter on a new line directly after the title.

"#
    );

    let mut parser = Parser::default();

    let block = crate::blocks::Block::parse(crate::Span::new(
        ".Terminal Output\n....\nFrom github.com:asciidoctor/asciidoctor\n* branch        main   -> FETCH_HEAD\nAlready up to date.\n....\n",
    ), &mut parser)
    .unwrap_if_no_warnings()
    .unwrap()
    .item;

    assert_eq!(
        block,
        Block::RawDelimited(RawDelimitedBlock {
            content: Content {
                original: Span {
                    data: "From github.com:asciidoctor/asciidoctor\n* branch        main   -> FETCH_HEAD\nAlready up to date.",
                    line: 3,
                    col: 1,
                    offset: 22,
                },
                rendered: "From github.com:asciidoctor/asciidoctor\n* branch        main   -&gt; FETCH_HEAD\nAlready up to date.",
            },
            content_model: ContentModel::Verbatim,
            context: "literal",
            source: Span {
                data: ".Terminal Output\n....\nFrom github.com:asciidoctor/asciidoctor\n* branch        main   -> FETCH_HEAD\nAlready up to date.\n....",
                line: 1,
                col: 1,
                offset: 0,
            },
            title_source: Some(Span {
                data: "Terminal Output",
                line: 1,
                col: 2,
                offset: 1,
            },),
            title: Some("Terminal Output"),
            anchor: None,
            anchor_reftext: None,
            attrlist: None,
            substitution_group: SubstitutionGroup::Verbatim,
        },)
    );
}

non_normative!(
    r#"
The result of <<ex-title>> is displayed below.

.Terminal Output
....
From github.com:asciidoctor/asciidoctor
 * branch        main   -> FETCH_HEAD
Already up to date.
....

In the next section, you'll see how a title is placed on a block that has an attribute list.

"#
);

#[test]
fn add_title_to_block_with_attributes() {
    verifies!(
        r#"
== Add a title to a block with attributes

When you're applying attributes to a block, the title is placed on the line above the attribute list (or lists).
<<ex-title-list>> shows a delimited source code block that's titled _Specify GitLab CI stages_.

.Add a title to a delimited source code block
[source#ex-title-list]
....
.Specify GitLab CI stages <.>
[source,yaml] <.>
----
image: node:16-buster
stages: [ init, verify, deploy ]
----
....
<.> The block title is entered on a new line.
<.> The block's attribute list is entered on a new line directly after the title.

"#
    );

    let mut parser = Parser::default();

    let block = crate::blocks::Block::parse(crate::Span::new(
        ".Specify GitLab CI stages\n[source,yaml]\n----\nimage: node:16-buster\nstages: [ init, verify, deploy ]\n----",
    ), &mut parser)
    .unwrap_if_no_warnings()
    .unwrap()
    .item;

    assert_eq!(
        block,
        Block::RawDelimited(RawDelimitedBlock {
            content: Content {
                original: Span {
                    data: "image: node:16-buster\nstages: [ init, verify, deploy ]",
                    line: 4,
                    col: 1,
                    offset: 45,
                },
                rendered: "image: node:16-buster\nstages: [ init, verify, deploy ]",
            },
            content_model: ContentModel::Verbatim,
            context: "listing",
            source: Span {
                data: ".Specify GitLab CI stages\n[source,yaml]\n----\nimage: node:16-buster\nstages: [ init, verify, deploy ]\n----",
                line: 1,
                col: 1,
                offset: 0,
            },
            title_source: Some(Span {
                data: "Specify GitLab CI stages",
                line: 1,
                col: 2,
                offset: 1,
            },),
            title: Some("Specify GitLab CI stages"),
            anchor: None,
            anchor_reftext: None,
            attrlist: Some(Attrlist {
                attributes: &[
                    ElementAttribute {
                        name: None,
                        shorthand_items: &["source"],
                        value: "source"
                    },
                    ElementAttribute {
                        name: None,
                        shorthand_items: &[],
                        value: "yaml"
                    },
                ],
                anchor: None,
                source: Span {
                    data: "source,yaml",
                    line: 2,
                    col: 2,
                    offset: 27,
                },
            },),
            substitution_group: SubstitutionGroup::Verbatim,
        },)
    );
}

non_normative!(
    r#"
The result of <<ex-title-list>> is displayed below.

[caption=]
.Specify GitLab CI stages
[source,yaml]
----
image: node:16-buster
stages: [ init, verify, deploy ]
----

"#
);

#[test]
fn add_title_to_non_delimited_block() {
    verifies!(
        r#"
As shown in <<ex-title-style>>, a block's title is placed above the attribute list when a block isn't delimited.

.Add a title to a non-delimited block
[#ex-title-style]
----
.Mint
[sidebar]
Mint has visions of global conquest.
If you don't plant it in a container, it will take over your garden.
----

"#
    );

    let mut parser = Parser::default();

    let block = crate::blocks::Block::parse(crate::Span::new(
        ".Mint\n[sidebar]\nMint has visions of global conquest.\nIf you don't plant it in a container, it will take over your garden.\n",
    ), &mut parser)
    .unwrap_if_no_warnings()
    .unwrap()
    .item;

    assert_eq!(
        block,
        Block::Simple(SimpleBlock {
            content: Content {
                original: Span {
                    data: "Mint has visions of global conquest.\nIf you don't plant it in a container, it will take over your garden.",
                    line: 3,
                    col: 1,
                    offset: 16,
                },
                rendered: "Mint has visions of global conquest.\nIf you don&#8217;t plant it in a container, it will take over your garden.",
            },
            source: Span {
                data: ".Mint\n[sidebar]\nMint has visions of global conquest.\nIf you don't plant it in a container, it will take over your garden.",
                line: 1,
                col: 1,
                offset: 0,
            },
            style: SimpleBlockStyle::Paragraph,
            title_source: Some(Span {
                data: "Mint",
                line: 1,
                col: 2,
                offset: 1,
            },),
            title: Some("Mint"),
            anchor: None,
            anchor_reftext: None,
            attrlist: Some(Attrlist {
                attributes: &[ElementAttribute {
                    name: None,
                    shorthand_items: &["sidebar"],
                    value: "sidebar"
                },],
                anchor: None,
                source: Span {
                    data: "sidebar",
                    line: 2,
                    col: 2,
                    offset: 7,
                },
            },),
        },)
    );

    // The result of <<ex-title-style>> is displayed below.

    // .Mint
    // [sidebar]
    // Mint has visions of global conquest.
    // If you don't plant it in a container, it will take over your garden.

    // You may notice that unlike the titles in the previous rendered listing
    // and source block examples, the sidebar's title is centered and
    // displayed inside the sidebar's background. How the title of a block
    // is displayed depends on the converter and stylesheet you're applying
    // to your AsciiDoc documents.
}

non_normative!(
    r#"
The result of <<ex-title-style>> is displayed below.

.Mint
[sidebar]
Mint has visions of global conquest.
If you don't plant it in a container, it will take over your garden.

You may notice that unlike the titles in the previous rendered listing and source block examples, the sidebar's title is centered and displayed inside the sidebar's background.
How the title of a block is displayed depends on the converter and stylesheet you're applying to your AsciiDoc documents.

"#
);

#[test]
#[ignore]
fn captioned_titles() {
    // == Captioned titles

    // Several block contexts support captioned titles.
    // A [.term]*captioned title* is a title that's prefixed with a caption
    // label and a number followed by a dot (e.g., `Table 1. Properties`).

    // The captioned title is only used if the corresponding caption attribute
    // is set. Otherwise, the original title is displayed.

    // The following table lists the blocks that support captioned titles and
    // the attributes that the converter uses to generate and control them.

    // .Blocks that support captioned titles
    // [cols=1;m;m]
    // |===
    // |Block context | Caption attribute | Counter attribute

    // |appendix
    // |appendix-caption
    // |appendix-number

    // |example
    // |example-caption
    // |example-number

    // |image
    // |figure-caption
    // |figure-number

    // |listing, source
    // |listing-caption
    // |listing-number

    // |table
    // |table-caption
    // |table-number
    // |===

    // All caption attributes are set by default except for the attribute for
    // listing and source blocks (`listing-caption`). The number is sequential,
    // computed automatically, and stored in a corresponding counter attribute.

    // Let's assume you've added a title to an example block as follows:

    // [,asciidoc]
    // ----
    // .Block that supports captioned title
    // ====
    // Block content
    // ====
    // ----

    // The block title will be displayed with a caption label and number, as
    // shown here:

    // :example-caption: Example
    // ifdef::example-number[:prev-example-number: {example-number}]
    // :example-number: 0

    // .Block that supports captioned title
    // ====
    // Block content
    // ====

    // :!example-caption:
    // ifdef::prev-example-number[:example-number: {prev-example-number}]
    // :!prev-example-number:

    // If you unset the `example-caption` attribute, the caption will not be
    // prepended to the title.

    // .Block that supports captioned title
    // ====
    // Block content
    // ====

    // The counter attribute (e.g., `example-number`) can be used to influence
    // the start number for the first block with that context or the next
    // number selected in the sequence for subsequent occurrences. However,
    // this practice should be used judiciously.

    // The caption can be overridden using the `caption` attribute on the block.
    // The value of the caption attribute replaces the entire caption, including
    // the space that precedes the title.

    // Here's how to define a custom caption on a block:

    // [,asciidoc]
    // ----
    // .Block Title
    // [caption="Example {counter:my-example-number:A}: "]
    // ====
    // Block content
    // ====
    // ----

    // Here's how the block will be displayed with the custom caption:

    // .Block Title
    // [caption="Example {counter:my-example-number:A}: "]
    // ====
    // Block content
    // ====

    // Notice we've used a counter attribute in the value of the caption
    // attribute to create a custom number sequence.

    // If you refer to a block with a custom caption using an xref, you may not
    // get the result that you expect. Therefore, it's always best to define
    // custom xref:attributes:id.adoc#customize-automatic-xreftext[xreftext]
    // when you define a custom caption.
}
