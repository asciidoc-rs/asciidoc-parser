use crate::{
    blocks::{ContentModel, IsBlock},
    content::SubstitutionStep,
    parser::ModificationContext,
    tests::prelude::*,
};

track_file!("docs/modules/verbatim/pages/listing-blocks.adoc");

non_normative!(
    r##"
= Listing Blocks
:replace-me: I've been replaced!

Blocks and paragraphs assigned the `listing` style display their rendered content exactly as you see it in the source.
Listing content is converted to preformatted text (i.e., `<pre>`).
The content is presented in a fixed-width font and endlines are preserved.
Only xref:subs:special-characters.adoc[special characters] and callouts are replaced when the document is converted.

The listing style can be applied to content using one of the following methods:

* setting the `listing` style on a block or paragraph using an attribute list, or
* enclosing the content within a pair of listing block delimiters (`----`).

"##
);

#[test]
fn listing_style_syntax() {
    verifies!(
        r##"
== Listing style syntax

The block style `listing` can be applied to a block or paragraph, by setting the attribute `listing` using an attribute list.

.Listing style syntax
[#ex-style]
----
include::example$listing.adoc[tag=style]
----

The result of <<ex-style>> is rendered below.

include::example$listing.adoc[tag=style]

"##
    );

    let doc = Parser::default().parse("[listing]\nThis is an example of a paragraph assigned\nthe `listing` style in an attribute list.\nNotice that the monospace marks are\npreserved in the output.");

    assert_eq!(
        doc,
        Document {
            header: Header {
                title_source: None,
                title: None,
                attributes: &[],
                author_line: None,
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "This is an example of a paragraph assigned\nthe `listing` style in an attribute list.\nNotice that the monospace marks are\npreserved in the output.",
                        line: 2,
                        col: 1,
                        offset: 10,
                    },
                    rendered: "This is an example of a paragraph assigned\nthe <code>listing</code> style in an attribute list.\nNotice that the monospace marks are\npreserved in the output.",
                },
                source: Span {
                    data: "[listing]\nThis is an example of a paragraph assigned\nthe `listing` style in an attribute list.\nNotice that the monospace marks are\npreserved in the output.",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                style: SimpleBlockStyle::Listing,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: Some(Attrlist {
                    attributes: &[ElementAttribute {
                        name: None,
                        value: "listing",
                        shorthand_items: &["listing"],
                    },],
                    anchor: None,
                    source: Span {
                        data: "listing",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },),],
            source: Span {
                data: "[listing]\nThis is an example of a paragraph assigned\nthe `listing` style in an attribute list.\nNotice that the monospace marks are\npreserved in the output.",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog {
                refs: HashMap::from([]),
                reftext_to_id: HashMap::from([]),
            },
        }
    );
}

#[test]
fn delimited_listing_block() {
    verifies!(
        r##"
== Delimited listing block

A delimited listing block is surrounded by lines composed of four hyphens (`----`).
This method is useful when the content contains empty lines.

.Delimited listing block syntax
[#ex-block]
------
include::example$listing.adoc[tag=block]
------

Here's how the block in <<ex-block>> appears when rendered.

// The listing style must be added above the rendered block so it is rendered correctly because we're setting `source-language` in the component descriptor which automatically promotes unstyled listing blocks to source blocks.
[listing]
include::example$listing.adoc[tag=block]

You should notice a few things about how the content is processed.

* The HTML element `<pre>` is escaped, that is, it's displayed verbatim, not interpreted.
* The endlines are preserved.
* The phrase _delimited listing block_ isn't italicized, despite having the underscore formatting marks around it.

Listing blocks are good for displaying snippets of raw source code, especially when used in tandem with the `source` style and `source-highlighter` attribute.
See xref:source-blocks.adoc[] to learn more about `source` and `source-highlighter`.

"##
    );

    let doc = Parser::default().parse("----\nThis is a _delimited listing block_.\n\nThe content inside is displayed as <pre> text.\n----");

    assert_eq!(
        doc,
        Document {
            header: Header {
                title_source: None,
                title: None,
                attributes: &[],
                author_line: None,
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[Block::RawDelimited(RawDelimitedBlock {
                content: Content {
                    original: Span {
                        data: "This is a _delimited listing block_.\n\nThe content inside is displayed as <pre> text.",
                        line: 2,
                        col: 1,
                        offset: 5,
                    },
                    rendered: "This is a _delimited listing block_.\n\nThe content inside is displayed as &lt;pre&gt; text.",
                },
                content_model: ContentModel::Verbatim,
                context: "listing",
                source: Span {
                    data: "----\nThis is a _delimited listing block_.\n\nThe content inside is displayed as <pre> text.\n----",
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
            },),],
            source: Span {
                data: "----\nThis is a _delimited listing block_.\n\nThe content inside is displayed as <pre> text.\n----",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog {
                refs: HashMap::from([]),
                reftext_to_id: HashMap::from([]),
            },
        }
    );
}

#[test]
fn listing_substitutions() {
    verifies!(
        r##"
== Listing substitutions

Content that is assigned the `listing` style, either via the explicit block style or the listing delimiters is subject to the xref:subs:index.adoc#verbatim-group[verbatim substitution group].
Only xref:subs:special-characters.adoc[special characters] and callouts are replaced automatically in listing content.

You can control the substitutions applied to a listing block using the `subs` attribute.

.Delimited listing block with custom substitutions syntax
[#ex-subs]
------
[subs="+attributes"]
----
This is a _delimited listing block_
with the `subs` attribute assigned
the incremental value `+attributes`.
This attribute reference:

{replace-me}

will be replaced with the attribute's
value when rendered.
----
------

The result of <<ex-subs>> is rendered below.

// The listing style must be added above the rendered block so it is rendered correctly because we're setting `source-language` in the component descriptor which automatically promotes unstyled listing blocks to source blocks.
[listing,subs="+attributes"]
----
This is a _delimited listing block_
with the `subs` attribute assigned
the incremental value `+attributes`.
This attribute reference:

{replace-me}

will be replaced with the attribute's
value when rendered.
----

See xref:subs:apply-subs-to-blocks.adoc[] to learn more about the `subs` attribute and how to apply incremental substitutions to listing content.
"##
    );

    let sg = vec![
        SubstitutionStep::SpecialCharacters,
        SubstitutionStep::AttributeReferences,
    ];

    let expected_block = Block::RawDelimited(RawDelimitedBlock {
        content: Content {
            original: Span {
                data: "This is a _delimited listing block_\nwith the `subs` attribute assigned\nthe incremental value `+attributes`.\nThis attribute reference:\n\n{replace-me}\n\nwill be replaced with the attribute's\nvalue when rendered.",
                line: 3,
                col: 1,
                offset: 26,
            },
            rendered: "This is a _delimited listing block_\nwith the `subs` attribute assigned\nthe incremental value `+attributes`.\nThis attribute reference:\n\nI've been replaced!\n\nwill be replaced with the attribute's\nvalue when rendered.",
        },
        content_model: ContentModel::Verbatim,
        context: "listing",
        source: Span {
            data: "[subs=\"+attributes\"]\n----\nThis is a _delimited listing block_\nwith the `subs` attribute assigned\nthe incremental value `+attributes`.\nThis attribute reference:\n\n{replace-me}\n\nwill be replaced with the attribute's\nvalue when rendered.\n----",
            line: 1,
            col: 1,
            offset: 0,
        },
        title_source: None,
        title: None,
        anchor: None,
        anchor_reftext: None,
        attrlist: Some(Attrlist {
            attributes: &[ElementAttribute {
                name: Some("subs"),
                value: "+attributes",
                shorthand_items: &[],
            }],
            anchor: None,
            source: Span {
                data: "subs=\"+attributes\"",
                line: 1,
                col: 2,
                offset: 1,
            },
        }),
        substitution_group: SubstitutionGroup::Custom(sg),
    });

    let doc = Parser::default().with_intrinsic_attribute(
            "replace-me",
            "I've been replaced!",
            ModificationContext::Anywhere,
        ).parse("[subs=\"+attributes\"]\n----\nThis is a _delimited listing block_\nwith the `subs` attribute assigned\nthe incremental value `+attributes`.\nThis attribute reference:\n\n{replace-me}\n\nwill be replaced with the attribute's\nvalue when rendered.\n----");

    let mut doc_blocks = doc.nested_blocks();

    let block = doc_blocks.next().unwrap();
    assert_eq!(block, &expected_block);

    assert!(doc_blocks.next().is_none());
}
