use crate::tests::prelude::*;

track_file!("docs/modules/blocks/pages/index.adoc");

non_normative!(
    r#"
= Blocks

Block elements form the main structure of an AsciiDoc document, starting with the document itself.

== What is a block?

A block element (aka block) is a discrete, line-oriented chunk of content in an AsciiDoc document.
Once parsed, that chunk of content becomes a block element in the parsed document model.
Certain blocks may contain other blocks, so we say that blocks can be nested.
The converter visits each block in turn, in document order, converting it to a corresponding chunk of output.

== Block forms

How the boundaries of a block are defined in the AsciiDoc syntax varies.
The boundaries of some blocks, like lists, paragraphs, and block macro, are implicit.
Other blocks have boundaries that are explicitly marked using delimiters (i.e., delimited blocks).
The main commonality is that a block is always line-oriented.

A _paragraph block_ is defined as a discrete set of contiguous (non-empty) lines.
A _delimited block_ is bounded by delimiter lines.
A _section block_ (aka section) is defined by a section title that's prefixed by one or more equal signs.
The section includes all content that follows the section title line until the next sibling or parent section title or the document boundary.
A _list block_ is defined by a group of sibling list items, each denoted by a marker.
A _description list_ block is defined by a sibling group of list items, each denoted by one or more terms.
A _block macro_ is defined by a single line that matches the block macro syntax.
And the _document_ is also a block.

A block (including its metadata lines) should always be bounded by an empty line or document boundary on either side.

Whether or not a block supports nested blocks depends on content model of the block (and what the syntax allows).

== Content model

The content model of a block determines what kind of content the block can have (if any) and how that content is processed.
The content models of blocks in AsciiDoc are as follows:

compound:: a block that may only contain other blocks (e.g., a section)
simple:: a block that's treated as contiguous lines of paragraph text (and subject to normal substitutions) (e.g., a paragraph block)
verbatim:: a block that holds verbatim text (displayed "`as is`") (and subject to verbatim substitutions) (e.g., a listing block)
raw:: a block that holds unprocessed content passed directly through to the output with no substitutions applied (e.g., a passthrough block)
empty:: a block that has no content (e.g., an image block)
table:: a special content model reserved for tables that enforces a fixed structure

The content model is inferred for all built-in syntax (as determined by the context), but can be configured for custom blocks.
Blocks may also support different content models under different circumstances.
The circumstance is determined by the context and style, and in the case of a delimited block, the structural container as well.

"#
);

mod context {
    use std::ops::Deref;

    use crate::{
        blocks::{ContentModel, is_built_in_context},
        tests::prelude::*,
    };

    non_normative!(
        r#"
== Context

You may often hear a block referred to by a name, such as an example block, a sidebar block, an admonition block, or a section.
That name is the block's context.

"#
    );

    #[test]
    fn section_context() {
        verifies!(
            r#"
Let's consider the following normal section:

----
== Section Title

Content of section.
----

The context of this block is `section`.
We often refer to this as a section (or section block), using the context as an adjective to describe the block.
The writer does not have to specify the context in this case since it's implied by the syntax.

Every block has a context.
The context is often implied by the syntax, but can be declared explicitly in certain cases.
The context is what distinguishes one kind of block from another.
You can think of the context as the block's type.

"#
        );

        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(
            crate::Span::new("== Section Title\n\nContent of section."),
            &mut parser,
        )
        .unwrap_if_no_warnings()
        .unwrap();

        assert_eq!(mi.item.raw_context().deref(), "section");
        assert_eq!(mi.item.resolved_context().deref(), "section");
        assert!(mi.item.declared_style().is_none());
        assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);
    }

    #[test]
    #[ignore]
    fn block_style() {
        non_normative!(
            r#"
The context can be further modified using a block style to create a family of blocks that share a common type, as is the case with admonition blocks and sections.
We'll cover that modifier shortly.

"#
        );

        todo!("Redundant: Covered by block_style test below.");
    }

    #[test]
    #[ignore]
    fn block_name() {
        to_do_verifies!(
            r#"
For blocks, the context is sometimes referred to as the block name.
This comes up in particular when talking about custom blocks.
The block name is just another layer of abstraction.
All the built-in block names map to exactly one context.
But a block extension can map an arbitrary block name to one or more contexts.
Which context is ultimately used depends on what is returned from the extension's process method.
In the end, it's the context that determines how the block is converted.

"#
        );

        todo!("Revisit when we support block extensions.");
    }

    #[test]
    fn sections_are_compound() {
        verifies!(
            r#"
The context often determines the content model.
For example, all sections implicitly have the compound content model because a section may only contain other blocks.
"#
        );

        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(
            crate::Span::new("== Section Title\n\nContent of section."),
            &mut parser,
        )
        .unwrap_if_no_warnings()
        .unwrap();

        assert_eq!(mi.item.content_model(), ContentModel::Compound);
    }

    #[test]
    #[ignore]
    fn literal_blocks_are_verbatim() {
        verifies!(
            r#"
All literal blocks implicitly have the verbatim content model because the purpose of this block is to present verbatim output.

"#
        );

        let mut parser = Parser::default();

        let mi =
            crate::blocks::Block::parse(crate::Span::new("....\nliteral text\n...."), &mut parser)
                .unwrap_if_no_warnings()
                .unwrap();

        assert_eq!(mi.item.content_model(), ContentModel::Verbatim);
    }

    #[test]
    fn built_in_contexts() {
        non_normative!(
            r#"
=== Summary of built-in contexts

Here's a list of the contexts of all the built-in blocks in AsciiDoc.

NOTE: In the Asciidoctor API, the contexts are represented as symbols.
In Ruby, a symbol is a name prefixed with a colon (e.g., `:listing`).
This documentation will sometimes use this notation when referring to the name of a context.
However, this notation is not universal.
Some processors, such as Asciidoctor.js, store the context as a string instead.

.Built-in contexts
[#table-of-contexts,cols="1s,2"]
|===
|Name | Purpose

"#
        );

        verifies!(
            r#"
|admonition
|One of five admonition blocks.

"#
        );

        assert!(is_built_in_context("admonition"));

        verifies!(
            r#"
|audio
|An audio block.

"#
        );

        assert!(is_built_in_context("audio"));

        verifies!(
            r#"
|colist
|A callout list.

"#
        );

        assert!(is_built_in_context("colist"));

        verifies!(
            r#"
|dlist
|A description list.

"#
        );

        assert!(is_built_in_context("dlist"));

        verifies!(
            r#"
|document
|The top-level document or the document in an AsciiDoc table cell

"#
        );

        assert!(is_built_in_context("document"));

        verifies!(
            r#"
|example
|An example block.

"#
        );

        assert!(is_built_in_context("example"));

        verifies!(
            r#"
|floating_title
|A discrete heading.

"#
        );

        assert!(is_built_in_context("floating_title"));

        verifies!(
            r#"
|image
|An image block.

"#
        );

        assert!(is_built_in_context("image"));

        verifies!(
            r#"
|list_item
|An item in an ordered, unordered, or description list (only relevant inside a list or description list block).
In a description list, this block is used to represent the term and the description.

"#
        );

        assert!(is_built_in_context("list_item"));

        verifies!(
            r#"
|listing
|A listing block.

"#
        );

        assert!(is_built_in_context("listing"));

        verifies!(
            r#"
|literal
|A literal block.

"#
        );

        assert!(is_built_in_context("literal"));

        verifies!(
            r#"
|olist
|An ordered list.

"#
        );

        assert!(is_built_in_context("olist"));

        verifies!(
            r#"
|open
|An open block.

"#
        );

        assert!(is_built_in_context("open"));

        verifies!(
            r#"
|page_break
|A page break.

"#
        );

        assert!(is_built_in_context("page_break"));

        verifies!(
            r#"
|paragraph
|A paragraph.

"#
        );

        assert!(is_built_in_context("paragraph"));

        verifies!(
            r#"
|pass
|A passthrough block.

"#
        );

        assert!(is_built_in_context("pass"));

        verifies!(
            r#"
|preamble
|The preamble of the document.

"#
        );

        assert!(is_built_in_context("preamble"));

        verifies!(
            r#"
|quote
|A quote block (aka blockquote).

"#
        );

        assert!(is_built_in_context("quote"));

        verifies!(
            r#"
|section
|A section.
May also be a part, chapter, or special section.

"#
        );

        assert!(is_built_in_context("section"));

        verifies!(
            r#"
|sidebar
|A sidebar block.

"#
        );

        assert!(is_built_in_context("sidebar"));

        verifies!(
            r#"
|table
|A table block.

"#
        );

        assert!(is_built_in_context("table"));

        verifies!(
            r#"
|table_cell
|A table cell (only relevant inside a table block).

"#
        );

        assert!(is_built_in_context("table_cell"));

        verifies!(
            r#"
|thematic_break
|A thematic break (aka horizontal rule).

"#
        );

        assert!(is_built_in_context("thematic_break"));

        verifies!(
            r#"
|toc
|A TOC block (to designate custom TOC placement).

"#
        );

        assert!(is_built_in_context("toc"));

        verifies!(
            r#"
|ulist
|An unordered list.

"#
        );

        assert!(is_built_in_context("ulist"));

        verifies!(
            r#"
|verse
|A verse block.

"#
        );

        assert!(is_built_in_context("verse"));

        verifies!(
            r#"
|video
|A video block.
"#
        );

        assert!(is_built_in_context("video"));

        non_normative!(
            r#"
|===

NOTE: Each inline element also has a context, but those elements are not (yet) accessible from the parsed document model.

Additional contexts may be introduced through the use of the block, block macro, or inline macro extension points.

"#
        );

        // Test a few context names that are not built-in contexts as a spot-check
        // against false positives.
        assert!(!is_built_in_context("documentx"));
        assert!(!is_built_in_context("sentence"));
        assert!(!is_built_in_context(""));
    }

    #[test]
    fn contexts_used_by_converter() {
        non_normative!(
            r#"
=== Contexts used by the converter

The context is what the converter uses to dispatch to a convert method.
The style is then used by the converter to apply special behavior to blocks of the same family.

With two exceptions, there's a 1-to-1 mapping between the contexts and the handler methods of a converter.
Those exceptions are the `list_item` and `table_cell` contexts, which are not mapped to a handler method.
In the converter, these blocks must be accessed from their parent block.

"#
        );

        // Treated as non-normative because asciidoc-parser is only a parser,
        // not a converter.
    }
}

mod block_style {
    use crate::{blocks::ContentModel, tests::prelude::*};

    non_normative!(
        r#"
[#block-style]
== Block style

The context does not always tell the whole story of a block's identity.
Some blocks require specialization.
That's where the block style comes into play.

Above some blocks, you may notice a name at the start of the block attribute list (e.g., `[source]` or `[verse]`).
The first positional (unnamed) attribute in the block attribute list is used to declare the block style.

The declared block style is the value the author supplies.
That value is then interpreted and resolved.
The resolved block style, if non-empty, specializes the block's context.
(It may instead, or in addition to, alter the block's context).

"#
    );

    #[test]
    fn source_block() {
        verifies!(
            r#"
Consider the following example of a source block:

[source]
....
[source,ruby]
----
puts "Hello, World!"
----
....

The context of a source block is `listing` (as inferred from the block delimiters) and the style is `source` (as specified by the writer).
We say that the style specializes the block as a source block.
(Technically, the presence of a source language already implies the `source` style, but under the covers this is what's happening).
The context of the block is still the same, but it has additional metadata to indicate that it requires special processing.

"#
        );

        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(
            crate::Span::new("[source,ruby]\n----\nputs \"Hello, World!\"\n----"),
            &mut parser,
        )
        .unwrap_if_no_warnings()
        .unwrap();

        assert_eq!(mi.item.raw_context().as_ref(), "listing");
        assert_eq!(mi.item.resolved_context().as_ref(), "listing");

        assert_eq!(mi.item.declared_style().unwrap(), "source");

        assert_eq!(mi.item.content_model(), ContentModel::Verbatim);
        assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Verbatim);
    }

    non_normative!(
        r#"
We also see the block style used for other purposes.
"#
    );

    #[test]
    fn appendix_style_specializes_a_section() {
        verifies!(
            r#"
The `appendix` block style (e.g., `[appendix]`) above the section title specializes the section as an appendix (a special section) and thus has special semantics and behavior.
"#
        );

        // The `[appendix]` block style above a section title declares the
        // `appendix` style and specializes the section as an appendix (a
        // special section).
        let doc = Parser::default().parse("[appendix]\n== My Appendix\n\nContent.");
        let block = doc.nested_blocks().next().expect("expected a block");

        assert_eq!(block.declared_style(), Some("appendix"));
        assert_eq!(block.raw_context().as_ref(), "section");

        let crate::blocks::Block::Section(section) = block else {
            panic!("expected a section block, got {block:?}");
        };

        assert_eq!(section.section_type(), SectionType::Appendix);

        non_normative!(
            r#"
In the model, the section's style is dually stored as the `sectname`.
"#
        );

        // Asciidoctor's Ruby model stores the special-section style both as the
        // block style and as the `sectname`. This crate models the same
        // distinction with the strongly typed `SectionType` (asserted above)
        // alongside the declared block style.
    }

    #[test]
    fn admonition_style_transforms_an_example_block() {
        verifies!(
            r#"
One of the five admonition styles (e.g., `[TIP]`) above an example block transforms the example block into an admonition block with that name (i.e., label).
"#
        );

        // A `[TIP]` block style above an example structural container transforms
        // the example block into an admonition block whose label is `Tip`.
        let doc = Parser::default().parse("[TIP]\n====\nPay attention.\n====");
        let block = doc.nested_blocks().next().expect("expected a block");

        assert_eq!(block.raw_context().as_ref(), "admonition");

        let crate::blocks::Block::Admonition(admonition) = block else {
            panic!("expected an admonition block, got {block:?}");
        };

        assert_eq!(admonition.label(), "Tip");

        non_normative!(
            r#"
In the model, the admonition style in lowercase is stored in the `name` attribute.
"#
        );

        // The admonition style in lowercase (`tip`) is exposed via `name()`.
        assert_eq!(admonition.name(), "tip");
    }

    #[test]
    fn list_style_alters_the_marker() {
        verifies!(
            r#"
A block style (e.g., `[circle]` or `[loweralpha]`) above an unordered or ordered list, respectively, alters the marker used in front of list items when displayed.
"#
        );

        // A `[circle]` block style above an unordered list declares the `circle`
        // style on the list.
        let doc = Parser::default().parse("[circle]\n* one\n* two");
        let block = doc.nested_blocks().next().expect("expected a block");
        assert_eq!(block.raw_context().as_ref(), "list");
        assert_eq!(block.declared_style(), Some("circle"));

        // A `[loweralpha]` block style above an ordered list declares the
        // `loweralpha` style on the list.
        let doc = Parser::default().parse("[loweralpha]\n. one\n. two");
        let block = doc.nested_blocks().next().expect("expected a block");
        assert_eq!(block.raw_context().as_ref(), "list");
        assert_eq!(block.declared_style(), Some("loweralpha"));

        // The parser captures the declared block style; how it "alters the
        // marker used in front of list items when displayed" is a converter
        // concern.
    }

    #[test]
    fn description_list_style() {
        verifies!(
            r#"
A block style (e.g., `[qanda]` and `[horizontal]`) above a description list can either change its semantics or layout.

"#
        );

        // A `[qanda]` block style above a description list declares the `qanda`
        // style on the list.
        let doc = Parser::default().parse("[qanda]\nQuestion::\n  Answer.");
        let block = doc.nested_blocks().next().expect("expected a block");
        assert_eq!(block.raw_context().as_ref(), "list");
        assert_eq!(block.declared_style(), Some("qanda"));

        // A `[horizontal]` block style above a description list declares the
        // `horizontal` style on the list.
        let doc =
            Parser::default().parse("[horizontal]\nCPU:: The brain.\nRAM:: The short-term memory.");
        let block = doc.nested_blocks().next().expect("expected a block");
        assert_eq!(block.raw_context().as_ref(), "list");
        assert_eq!(block.declared_style(), Some("horizontal"));

        // The parser captures the declared block style; whether it changes the
        // description list's semantics or layout is a converter concern.
    }

    #[test]
    fn block_masquerading() {
        verifies!(
            r#"
The declared block style can be used to change the context of a block, referred to as xref:masquerading.adoc[block masquerading].
Consider the case of this alternate syntax for a listing block using the literal block delimiters.

[source]
----
[listing]
....
a > b
....
----

Since the declared block style matches the name of a context, the context of the block becomes `listing` and the resolved block style remains unset.
That means the resolved block style differs from the declared block style.
To learn more about how to change the context of a block using the declared block style, see xref:masquerading.adoc[].

"#
        );

        // The `[listing]` block style above the literal (`....`) delimiters
        // changes the context of the block: the raw (default) context is
        // `literal`, but the declared block style matches the `listing` context,
        // so the resolved context becomes `listing` and the resolved block style
        // is left unset.
        let doc = Parser::default().parse("[listing]\n....\na > b\n....");
        let block = doc.nested_blocks().next().expect("expected a block");

        assert_eq!(block.raw_context().as_ref(), "literal");
        assert_eq!(block.resolved_context().as_ref(), "listing");
        assert_eq!(block.declared_style(), Some("listing"));
    }

    non_normative!(
        r#"
To get a complete picture of a block's identity, you must consider both the context and the style.
The resolved style specializes the context to give it special behavior or semantics.
"#
    );
}

mod block_commonalities {
    use crate::{blocks::ContentModel, tests::prelude::*};

    non_normative!(
        r#"
== Block commonalities

Blocks are defined using some form of line-oriented syntax.
Section blocks begin with a section title line.
Delimited blocks are enclosed in a matching pair of delimiter lines.
Paragraph blocks must be contiguous lines.

"#
    );

    #[test]
    fn metadata_lines() {
        verifies!(
            r#"
All blocks accommodate zero or more lines of metadata stacked linewise directly on top of the block.
These lines populate the properties of the block, such as the ID, title, and options.
These metadata lines are as follows:

* Zero or more block attribute lines (which populate the block's attributes)
* An optional block anchor line
* An optional block title line (many blocks also support a corresponding caption)
* An optional ID
* An optional set of roles
* An optional set of options

"#
        );

        // A sidebar block stacked with a title line and a block attribute line
        // (declaring an ID, a role, and an option via attribute shorthand)
        // populates the corresponding block properties.
        let doc = Parser::default()
            .parse(".Styles of music\n[#music-styles.feature%collapsible]\n****\nAn aside.\n****");
        let block = doc.nested_blocks().next().expect("expected a block");

        assert!(block.attrlist().is_some());
        assert_eq!(block.title(), Some("Styles of music"));
        assert_eq!(block.id(), Some("music-styles"));
        assert_eq!(block.roles(), vec!["feature"]);
        assert_eq!(block.options(), vec!["collapsible"]);
    }

    #[test]
    fn first_bracketed_line_is_a_block_attribute_line() {
        verifies!(
            r#"
CAUTION: If the first line of a block begins with `[` and ends with `]`, that line will be interpretted as a block attribute line.
It does not matter what text is contained between those brackets.
For example, if the first description list term starts with `[` and definition on the same line ends with `]`, it will not appear to the parser as a description list entry, but rather as a block attribute line.
To workaround this interpretation of the source, you need to move the trailing `]` (and whatever goes with it) to the next line.

"#
        );

        // A first line that begins with `[` and ends with `]` is consumed as a
        // block attribute line rather than as content, regardless of the text
        // between the brackets. Here it is interpreted as the block's attribute
        // list (declaring the `sidebar` style) rather than as a description list
        // term/definition.
        let doc = Parser::default().parse("[sidebar]\nAn aside.");
        let block = doc.nested_blocks().next().expect("expected a block");

        assert_eq!(block.resolved_context().as_ref(), "sidebar");
        assert_eq!(block.declared_style(), Some("sidebar"));
    }

    #[test]
    fn sidebar_with_a_title_and_id() {
        verifies!(
            r#"
For example, consider a sidebar block with a title and ID:

----
.Styles of music
[#music-styles]
****
Go off on a tangent to describe what a style of music is.
****
----

"#
        );

        let doc = Parser::default().parse(
            ".Styles of music\n[#music-styles]\n****\nGo off on a tangent to describe what a style of music is.\n****",
        );
        let block = doc.nested_blocks().next().expect("expected a block");

        assert_eq!(block.resolved_context().as_ref(), "sidebar");
        assert_eq!(block.title(), Some("Styles of music"));
        assert_eq!(block.id(), Some("music-styles"));
    }

    #[test]
    fn content_processing_groups() {
        non_normative!(
            r#"
When it comes to processing content, blocks split off into different groups.
These groups are primarily associated with the block's content model.

"#
        );

        verifies!(
            r#"
Paragraph blocks and verbatim blocks have an implicit and modifiable set of xref:subs:index.adoc[substitutions].
Substitutions do not apply to compound blocks (i.e., blocks that may contain nested blocks).

"#
        );

        // A paragraph block (simple content model) has the normal substitution
        // group.
        let doc = Parser::default().parse("A normal paragraph.");
        let paragraph = doc.nested_blocks().next().expect("expected a block");
        assert_eq!(paragraph.content_model(), ContentModel::Simple);
        assert_eq!(paragraph.substitution_group(), SubstitutionGroup::Normal);

        // A verbatim block has the verbatim substitution group.
        let doc = Parser::default().parse("....\nliteral text\n....");
        let verbatim = doc.nested_blocks().next().expect("expected a block");
        assert_eq!(verbatim.content_model(), ContentModel::Verbatim);
        assert_eq!(verbatim.substitution_group(), SubstitutionGroup::Verbatim);

        // A compound block (a section) contains nested blocks; substitutions do
        // not apply to it.
        let doc = Parser::default().parse("== Section Title\n\nContent of section.");
        let compound = doc.nested_blocks().next().expect("expected a block");
        assert_eq!(compound.content_model(), ContentModel::Compound);
    }
}
