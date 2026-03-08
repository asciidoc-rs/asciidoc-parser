use crate::{
    Parser,
    blocks::{ContentModel, SimpleBlockStyle},
    content::SubstitutionGroup,
    tests::prelude::*,
};

track_file!("docs/modules/verbatim/pages/literal-blocks.adoc");

non_normative!(
    r##"
= Literal Blocks

Literal blocks display the text you write exactly as you see it in the source.
Literal text is treated as preformatted text.
The text is presented in a fixed-width font and endlines are preserved.
Only xref:subs:special-characters.adoc[special characters] and callouts are replaced when the document is converted.

The literal style can be applied to content using any of the following methods:

* indenting the first line of a paragraph by one or more spaces,
* setting the `literal` style on a block using an attribute list, or
* enclosing the content within a pair of literal block delimiters (`\....` ).

"##
);

#[test]
fn indent_method() {
    verifies!(
        r##"
== Indent method

When a line begins with one or more spaces it is displayed as a literal block.
This method is an easy way to insert simple code snippets.

.Indicate literal text using an indent
[source#ex-indent]
----
include::example$literal.adoc[tag=indent]
----

The result of <<ex-indent>> is rendered below.

include::example$literal.adoc[tag=indent]

"##
    );

    let doc = Parser::default().parse(" ~/secure/vault/defops");

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
                        data: " ~/secure/vault/defops",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "~/secure/vault/defops",
                },
                source: Span {
                    data: " ~/secure/vault/defops",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                style: SimpleBlockStyle::Literal,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },),],
            source: Span {
                data: " ~/secure/vault/defops",
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
fn literal_style_syntax() {
    verifies!(
        r##"
=== literal style syntax

The literal style can be applied to a block, such as a paragraph, by setting the style attribute `literal` on the block using an attribute list.

.Literal style syntax
[source#ex-style]
----
include::example$literal.adoc[tag=style]
----

The result of <<ex-style>> is rendered below.

include::example$literal.adoc[tag=style]

"##
    );

    let doc = Parser::default().parse("[literal]\nerror: 1954 Forbidden search\nabsolutely fatal: operation lost in the dodecahedron of doom\nWould you like to try again? y/n");

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
                        data: "error: 1954 Forbidden search\nabsolutely fatal: operation lost in the dodecahedron of doom\nWould you like to try again? y/n",
                        line: 2,
                        col: 1,
                        offset: 10,
                    },
                    rendered: "error: 1954 Forbidden search\nabsolutely fatal: operation lost in the dodecahedron of doom\nWould you like to try again? y/n",
                },
                source: Span {
                    data: "[literal]\nerror: 1954 Forbidden search\nabsolutely fatal: operation lost in the dodecahedron of doom\nWould you like to try again? y/n",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                style: SimpleBlockStyle::Literal,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: Some(Attrlist {
                    attributes: &[ElementAttribute {
                        name: None,
                        value: "literal",
                        shorthand_items: &["literal"],
                    },],
                    anchor: None,
                    source: Span {
                        data: "literal",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },),],
            source: Span {
                data: "[literal]\nerror: 1954 Forbidden search\nabsolutely fatal: operation lost in the dodecahedron of doom\nWould you like to try again? y/n",
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
fn delimited_literal_block() {
    verifies!(
        r##"
=== Delimited literal block

Finally, you can surround the content you want rendered as literal by enclosing it in a pair of literal block delimiters (`\....`).
This method is useful when the content contains empty lines.

.Delimited literal block syntax
[source#ex-block]
----
include::example$literal.adoc[tag=block]
----

The result of <<ex-block>> is rendered below.

include::example$literal.adoc[tag=block]

Notice in the output that the bold text formatting is not applied to the text nor are the three consecutive periods replaced by the ellipsis Unicode character.
"##
    );

    let doc = Parser::default().parse("....\nKismet: Where is the *defensive operations manual*?\n\nComputer: Calculating ...\nCan not locate object.\nYou are not authorized to know it exists.\n\nKismet: Did the werewolves tell you to say that?\n\nComputer: Calculating ...\n....");

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
                        data: "Kismet: Where is the *defensive operations manual*?\n\nComputer: Calculating ...\nCan not locate object.\nYou are not authorized to know it exists.\n\nKismet: Did the werewolves tell you to say that?\n\nComputer: Calculating ...",
                        line: 2,
                        col: 1,
                        offset: 5,
                    },
                    rendered: "Kismet: Where is the *defensive operations manual*?\n\nComputer: Calculating ...\nCan not locate object.\nYou are not authorized to know it exists.\n\nKismet: Did the werewolves tell you to say that?\n\nComputer: Calculating ...",
                },
                content_model: ContentModel::Verbatim,
                context: "literal",
                source: Span {
                    data: "....\nKismet: Where is the *defensive operations manual*?\n\nComputer: Calculating ...\nCan not locate object.\nYou are not authorized to know it exists.\n\nKismet: Did the werewolves tell you to say that?\n\nComputer: Calculating ...\n....",
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
                data: "....\nKismet: Where is the *defensive operations manual*?\n\nComputer: Calculating ...\nCan not locate object.\nYou are not authorized to know it exists.\n\nKismet: Did the werewolves tell you to say that?\n\nComputer: Calculating ...\n....",
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
