use crate::{
    blocks::{ContentModel, IsBlock, SimpleBlockStyle},
    tests::prelude::*,
    warnings::WarningType,
};

track_file!("docs/modules/blocks/pages/delimited.adoc");

non_normative!(
    r#"
= Delimited Blocks

In AsciiDoc, a delimited block is a region of content that's bounded on either side by a pair of congruent linewise delimiters.
A delimited block is used either to enclose other blocks (e.g.., multiple paragraphs) or set the content model of the content (e.g., verbatim).
Delimited blocks are a subset of all block types in AsciiDoc.

"#
);

#[test]
fn overview() {
    verifies!(
        r#"
== Overview

Delimited blocks are defined using structural containers, which are the fixed set of recognized block enclosures in the AsciiDoc syntax.
Here's the structural container for a literal block:

----
....
This text will be treated as verbatim content.
....
----

"#
    );

    let mut parser = Parser::default();

    let block = crate::blocks::Block::parse(
        crate::Span::new("....\nThis text will be treated as verbatim content.\n...."),
        &mut parser,
    )
    .unwrap_if_no_warnings()
    .unwrap()
    .item;

    assert_eq!(
        block,
        Block::RawDelimited(RawDelimitedBlock {
            content: Content {
                original: Span {
                    data: "This text will be treated as verbatim content.",
                    line: 2,
                    col: 1,
                    offset: 5,
                },
                rendered: "This text will be treated as verbatim content.",
            },
            content_model: ContentModel::Verbatim,
            context: "literal",
            source: Span {
                data: "....\nThis text will be treated as verbatim content.\n....",
                line: 1,
                col: 1,
                offset: 0,
            },
            title_source: None,
            title: None,
            anchor: None,
            anchor_reftext: None,
            attrlist: None,
            substitution_group: SubstitutionGroup::Verbatim,
        },)
    );
}

#[test]
fn structural_container() {
    verifies!(
        r#"
A structural container has an opening delimiter and a closing delimiter.
The opening delimiter follows the block metadata, if present.
Leading and trailing empty lines in a structure container are not considered significant and are automatically removed.
The remaining lines define a block's content.

"#
    );

    let mut parser = Parser::default();

    let block = crate::blocks::Block::parse(
        crate::Span::new("....\n\n\nThis text will be treated as verbatim content.\n\n\n...."),
        &mut parser,
    )
    .unwrap_if_no_warnings()
    .unwrap()
    .item;

    assert_eq!(
        block,
        Block::RawDelimited(RawDelimitedBlock {
            content: Content {
                original: Span {
                    data: "\n\nThis text will be treated as verbatim content.\n\n",
                    line: 2,
                    col: 1,
                    offset: 5,
                },
                rendered: "\n\nThis text will be treated as verbatim content.\n\n",
            },
            content_model: ContentModel::Verbatim,
            context: "literal",
            source: Span {
                data: "....\n\n\nThis text will be treated as verbatim content.\n\n\n....",
                line: 1,
                col: 1,
                offset: 0,
            },
            title_source: None,
            title: None,
            anchor: None,
            anchor_reftext: None,
            attrlist: None,
            substitution_group: SubstitutionGroup::Verbatim,
        },)
    );
}

non_normative!(
    r#"
These enclosures not only define the boundaries of a block's content, but also imply a content model (e.g., verbatim content or a subtree).
In certain cases, they provide a mechanism for nesting blocks.
However, delimited blocks cannot be interleaved.

A delimited block has the unique ability of being able to be repurposed by the AsciiDoc syntax, through both built-in mappings and mappings for custom blocks defined by an extension.
To understand how delimited blocks work, it's important to understand structural containers, their linewise delimiters, their default contexts, and their expected content models, as well as block nesting and masquerading.

"#
);

#[test]
fn linewise_delimiters() {
    verifies!(
        r#"
== Linewise delimiters

A delimited block is characterized by a pair of congruent linewise delimiters.
The opening and closing delimiter must match exactly, both in length and in sequence of characters.
These delimiters, sometimes referred to as fences, enclose the content and explicitly mark its boundaries.
Within the boundaries of a delimited block, you can enter any content or empty lines.
The block doesn't end until the ending delimiter is found.
The block metadata (block attribute and anchor lines) goes above the opening delimiter (thus outside the delimited region).

"#
    );

    let mut parser = Parser::default();

    let maw_block = crate::blocks::Block::parse(
        crate::Span::new("....\nThis text will be treated as verbatim content.\n....."),
        &mut parser,
    );

    assert_eq!(
        maw_block.warnings,
        vec![Warning {
            source: Span {
                data: "....",
                line: 1,
                col: 1,
                offset: 0,
            },
            warning: WarningType::UnterminatedDelimitedBlock,
        },]
    );
}

#[test]
fn delimited_example_block() {
    verifies!(
        r#"
Here's an example of a delimited example block:

----
====
This is an example of an example block.
That's so meta.
====
----

"#
    );

    let mut parser = Parser::default();

    let block = crate::blocks::Block::parse(
        crate::Span::new("====\nThis is an example of an example block.\nThat's so meta.\n===="),
        &mut parser,
    )
    .unwrap_if_no_warnings()
    .unwrap()
    .item;

    assert_eq!(
        block,
        Block::CompoundDelimited(CompoundDelimitedBlock {
            blocks: &[Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "This is an example of an example block.\nThat's so meta.",
                        line: 2,
                        col: 1,
                        offset: 5,
                    },
                    rendered: "This is an example of an example block.\nThat&#8217;s so meta.",
                },
                source: Span {
                    data: "This is an example of an example block.\nThat's so meta.",
                    line: 2,
                    col: 1,
                    offset: 5,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },),],
            context: "example",
            source: Span {
                data: "====\nThis is an example of an example block.\nThat's so meta.\n====",
                line: 1,
                col: 1,
                offset: 0,
            },
            title_source: None,
            title: None,
            anchor: None,
            anchor_reftext: None,
            attrlist: None,
        },)
    );
}

non_normative!(
    r#"
Typically, the delimiter is written using the minimum allowable length (4 characters, with the exception of the open block, which currently has a fixed length of 2 characters).
The length of the delimiter lines can be varied to accommodate nested blocks.

The valid set of delimiters for defining a delimited block, and the meaning they have, is defined by the available structural containers, covered next.

"#
);

#[test]
fn structural_containers() {
    non_normative!(
        r#"
== Structural containers

Structural containers are the fixed set of recognized block enclosures (delimited regions) defined by the AsciiDoc language.
These enclosures provide a reusable building block in the AsciiDoc syntax.
By evaluating the structural container and the block metadata, the processor will determine which kind of block to make.

Each structural container has an expected content model.
For built-in blocks, it's the context of the block that determines the content model, though most built-in blocks adhere to the expected content model.
Custom blocks have the ability to designate the content model.
Even in these cases, though, the content model should be chosen to honor the semantics of the structural container.
//This allows a text editor to understand how to parse the block and provide a reasonable fallback when the extension is not loaded.

Some structural containers are reused for different purposes, such as the structural container for the quote block being used for a verse block.

"#
    );

    verifies!(
        r#"
=== Summary of structural containers

The table below lists the structural containers, documenting the name, default context, and delimiter line for each.

.Structural containers in AsciiDoc
[#table-of-structural-containers,cols="1,2m,2,2l"]
|===
|Type |Default context |Content model (expected) |Minimum delimiter

|comment
|_n/a_
|_n/a_
|////

"#
    );

    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("////\n////"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(mi.item.content_model(), ContentModel::Raw);
    assert_eq!(mi.item.raw_context().as_ref(), "comment");
    assert_eq!(mi.item.resolved_context().as_ref(), "comment");

    verifies!(
        r#"
|example
|:example
|compound
|====

"#
    );

    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("====\n===="), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(mi.item.content_model(), ContentModel::Compound);
    assert_eq!(mi.item.raw_context().as_ref(), "example");
    assert_eq!(mi.item.resolved_context().as_ref(), "example");

    verifies!(
        r#"
|listing
|:listing
|verbatim
|----

"#
    );

    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("----\n----"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(mi.item.content_model(), ContentModel::Verbatim);
    assert_eq!(mi.item.raw_context().as_ref(), "listing");
    assert_eq!(mi.item.resolved_context().as_ref(), "listing");

    verifies!(
        r#"
|literal
|:literal
|verbatim
|....

"#
    );

    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("....\n...."), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(mi.item.content_model(), ContentModel::Verbatim);
    assert_eq!(mi.item.raw_context().as_ref(), "literal");
    assert_eq!(mi.item.resolved_context().as_ref(), "literal");

    verifies!(
        r#"
|open
|:open
|compound
|--

"#
    );

    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("--\n--"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(mi.item.content_model(), ContentModel::Compound);
    assert_eq!(mi.item.raw_context().as_ref(), "open");
    assert_eq!(mi.item.resolved_context().as_ref(), "open");

    verifies!(
        r#"
|sidebar
|:sidebar
|compound
|****

"#
    );

    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("****\n****"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(mi.item.content_model(), ContentModel::Compound);
    assert_eq!(mi.item.raw_context().as_ref(), "sidebar");
    assert_eq!(mi.item.resolved_context().as_ref(), "sidebar");

    to_do_verifies!(
        r#"
|table
|:table
|table
|\|===
,===
:===
!===

"#
    );

    if false {
        todo!("Support for table parsing");
    }

    verifies!(
        r#"
|pass
|:pass
|raw
|++++

"#
    );

    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("++++\n++++"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(mi.item.content_model(), ContentModel::Raw);
    assert_eq!(mi.item.raw_context().as_ref(), "pass");
    assert_eq!(mi.item.resolved_context().as_ref(), "pass");

    verifies!(
        r#"
|quote
|:quote
|compound
|____
|===

"#
    );

    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("____\n____"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(mi.item.content_model(), ContentModel::Compound);
    assert_eq!(mi.item.raw_context().as_ref(), "quote");
    assert_eq!(mi.item.resolved_context().as_ref(), "quote");

    non_normative!(
        r#"
You may notice the absence of the source block.
That's because source is not a container type.
Rather, it's a specialization of the listing (or literal) container as designated by the block style.
The verse and admonition blocks are also noticeably absent.
That's because they repurpose the structural containers for quote and example blocks, respectively.

The default context is assumed when an explicit block style is not present.

Currently, table is a specialized structural container that cannot be enlisted as a custom block.

Unlike other structural containers, a comment block is not preserved in the parsed document and therefore doesn't have a context or content model.

TIP: When creating a custom block, it's important to choose a structural container that provides the right content model.
This allows a text editor to understand how to parse the block and provide a reasonable fallback when the extension is not loaded.

Structural containers are used to define delimited blocks.
The structural container provides a default context and expected content model, but the actual context and content model is determined after considering the metadata on the block (specifically the declared block content).

"#
    );
}

mod nesting_blocks {
    use crate::{
        blocks::{ContentModel, SimpleBlockStyle},
        content::SubstitutionGroup,
        tests::prelude::*,
    };

    non_normative!(
        r#"
[#nesting]
== Nesting blocks

Using delimited blocks, you can nest blocks inside of one other.
(Blocks can also be nested inside sections, list items, and table cells, which is a separate topic).

First, the parent block must have the compound content model.
The compound content model means that the block's content is a sequence of zero or more blocks.

When nesting a block that uses a different structural container from the parent, it's enough to ensure that the child block is entirely inside the parent block.
Delimited blocks cannot be interleaved.

"#
    );

    #[test]
    fn different_structural_containers() {
        verifies!(
            r#"
[source]
....
====
Here's a sample AsciiDoc document:

----
= Document Title
Author Name

Content goes here.
----

The document header is useful, but not required.
====
....

"#
        );

        let mut parser = Parser::default();

        let block = crate::blocks::Block::parse(crate::Span::new(
            "====\nHere's a sample AsciiDoc document:\n\n----\n= Document Title\nAuthor Name\n\nContent goes here.\n----\n\nThe document header is useful, but not required.\n====\n",
        ), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap()
        .item;

        assert_eq!(
            block,
            Block::CompoundDelimited(CompoundDelimitedBlock {
                blocks: &[
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "Here's a sample AsciiDoc document:",
                                line: 2,
                                col: 1,
                                offset: 5,
                            },
                            rendered: "Here&#8217;s a sample AsciiDoc document:",
                        },
                        source: Span {
                            data: "Here's a sample AsciiDoc document:",
                            line: 2,
                            col: 1,
                            offset: 5,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::RawDelimited(RawDelimitedBlock {
                        content: Content {
                            original: Span {
                                data: "= Document Title\nAuthor Name\n\nContent goes here.",
                                line: 5,
                                col: 1,
                                offset: 46,
                            },
                            rendered: "= Document Title\nAuthor Name\n\nContent goes here.",
                        },
                        content_model: ContentModel::Verbatim,
                        context: "listing",
                        source: Span {
                            data: "----\n= Document Title\nAuthor Name\n\nContent goes here.\n----",
                            line: 4,
                            col: 1,
                            offset: 41,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                        substitution_group: SubstitutionGroup::Verbatim,
                    },),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "The document header is useful, but not required.",
                                line: 11,
                                col: 1,
                                offset: 101,
                            },
                            rendered: "The document header is useful, but not required.",
                        },
                        source: Span {
                            data: "The document header is useful, but not required.",
                            line: 11,
                            col: 1,
                            offset: 101,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                context: "example",
                source: Span {
                    data: "====\nHere's a sample AsciiDoc document:\n\n----\n= Document Title\nAuthor Name\n\nContent goes here.\n----\n\nThe document header is useful, but not required.\n====",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },)
        );
    }

    #[test]
    fn same_structural_container() {
        verifies!(
            r#"
When nesting a delimited block that uses the same structural container, it's necessary to vary the length of the delimiter lines (i.e., make the length of the delimiter lines for the child block different than the length of the delimiter lines for the parent block).
Varying the delimiter line length allows the parser to distinguish one block from another.

----
====
Here are your options:

.Red Pill
[%collapsible]
======
Escape into the real world.
======

.Blue Pill
[%collapsible]
======
Live within the simulated reality without want or fear.
======
====
----

"#
        );

        let mut parser = Parser::default();

        let block = crate::blocks::Block::parse(crate::Span::new(
            "====\nHere are your options:\n\n.Red Pill\n[%collapsible]\n======\nEscape into the real world.\n======\n\n.Blue Pill\n[%collapsible]\n======\nLive within the simulated reality without want or fear.\n======\n====",
        ), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap()
        .item;

        assert_eq!(
            block,
            Block::CompoundDelimited(CompoundDelimitedBlock {
                blocks: &[
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "Here are your options:",
                                line: 2,
                                col: 1,
                                offset: 5,
                            },
                            rendered: "Here are your options:",
                        },
                        source: Span {
                            data: "Here are your options:",
                            line: 2,
                            col: 1,
                            offset: 5,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::CompoundDelimited(CompoundDelimitedBlock {
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "Escape into the real world.",
                                    line: 7,
                                    col: 1,
                                    offset: 61,
                                },
                                rendered: "Escape into the real world.",
                            },
                            source: Span {
                                data: "Escape into the real world.",
                                line: 7,
                                col: 1,
                                offset: 61,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        context: "example",
                        source: Span {
                            data: ".Red Pill\n[%collapsible]\n======\nEscape into the real world.\n======",
                            line: 4,
                            col: 1,
                            offset: 29,
                        },
                        title_source: Some(Span {
                            data: "Red Pill",
                            line: 4,
                            col: 2,
                            offset: 30,
                        },),
                        title: Some("Red Pill"),
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: Some(Attrlist {
                            attributes: &[ElementAttribute {
                                name: None,
                                shorthand_items: &["%collapsible"],
                                value: "%collapsible"
                            },],
                            anchor: None,
                            source: Span {
                                data: "%collapsible",
                                line: 5,
                                col: 2,
                                offset: 40,
                            },
                        },),
                    },),
                    Block::CompoundDelimited(CompoundDelimitedBlock {
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "Live within the simulated reality without want or fear.",
                                    line: 13,
                                    col: 1,
                                    offset: 130,
                                },
                                rendered: "Live within the simulated reality without want or fear.",
                            },
                            source: Span {
                                data: "Live within the simulated reality without want or fear.",
                                line: 13,
                                col: 1,
                                offset: 130,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        context: "example",
                        source: Span {
                            data: ".Blue Pill\n[%collapsible]\n======\nLive within the simulated reality without want or fear.\n======",
                            line: 10,
                            col: 1,
                            offset: 97,
                        },
                        title_source: Some(Span {
                            data: "Blue Pill",
                            line: 10,
                            col: 2,
                            offset: 98,
                        },),
                        title: Some("Blue Pill"),
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: Some(Attrlist {
                            attributes: &[ElementAttribute {
                                name: None,
                                shorthand_items: &["%collapsible"],
                                value: "%collapsible"
                            },],
                            anchor: None,
                            source: Span {
                                data: "%collapsible",
                                line: 11,
                                col: 2,
                                offset: 109,
                            },
                        },),
                    },),
                ],
                context: "example",
                source: Span {
                    data: "====\nHere are your options:\n\n.Red Pill\n[%collapsible]\n======\nEscape into the real world.\n======\n\n.Blue Pill\n[%collapsible]\n======\nLive within the simulated reality without want or fear.\n======\n====",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },)
        );
    }

    non_normative!(
        r#"
The delimiter length for the nested structural container can either be shorter or longer than the parent.
That's a personal style choice.
"#
    );
}
