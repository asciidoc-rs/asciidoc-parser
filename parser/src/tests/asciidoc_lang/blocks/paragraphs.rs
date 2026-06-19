use crate::tests::prelude::*;

track_file!("docs/modules/blocks/pages/paragraphs.adoc");

non_normative!(
    r#"
= Paragraphs

The primary block type in most documents is the paragraph.
That's why in AsciiDoc, you don't need to use any special markup or attributes to create paragraphs.
You can just start typing sentences and that content becomes a paragraph.

This page introduces you to the paragraph in AsciiDoc and explains how to set it apart from other paragraphs.

"#
);

#[test]
fn create_a_paragraph() {
    verifies!(
        r#"
== Create a paragraph

Adjacent or consecutive lines of text form a paragraph element.
To start a new paragraph after another element, such as a section title or table, hit the kbd:[RETURN] key twice to insert an empty line, and then continue typing your content.

.Two paragraphs in an AsciiDoc document
[#ex-paragraph]
----
include::example$paragraph.adoc[tag=para]
----

The result of <<ex-paragraph>> is displayed below.

====
include::example$paragraph.adoc[tag=para]
====
"#
    );

    let doc = Parser::default().parse(
        "Paragraphs don't require any special markup in AsciiDoc.\nA paragraph is just one or more lines of consecutive text.\n\nTo begin a new paragraph, separate it by at least one empty line from the previous paragraph or block."
    );

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
            blocks: &[
                Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "Paragraphs don't require any special markup in AsciiDoc.\nA paragraph is just one or more lines of consecutive text.",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "Paragraphs don&#8217;t require any special markup in AsciiDoc.\nA paragraph is just one or more lines of consecutive text.",
                    },
                    source: Span {
                        data: "Paragraphs don't require any special markup in AsciiDoc.\nA paragraph is just one or more lines of consecutive text.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),
                Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "To begin a new paragraph, separate it by at least one empty line from the previous paragraph or block.",
                            line: 4,
                            col: 1,
                            offset: 117,
                        },
                        rendered: "To begin a new paragraph, separate it by at least one empty line from the previous paragraph or block.",
                    },
                    source: Span {
                        data: "To begin a new paragraph, separate it by at least one empty line from the previous paragraph or block.",
                        line: 4,
                        col: 1,
                        offset: 117,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),
            ],
            source: Span {
                data: "Paragraphs don't require any special markup in AsciiDoc.\nA paragraph is just one or more lines of consecutive text.\n\nTo begin a new paragraph, separate it by at least one empty line from the previous paragraph or block.",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog::default(),
        }
    );
}

#[test]
fn block_attribute_line_conflict() {
    verifies!(
        r#"
[#block-attribute-line-conflict]
== Conflict with a block attribute line

The first line of a paragraph cannot start with `[` and end with `]`.
That's because it will be treated as block metadata, specifically a block attribute line.

There are two ways to work around this issue.
First, you can add any text in front of the `[` that will otherwise not be rendered to disrupt the syntax match.
For example, you can start the paragraph with the attribute reference `+{empty}+`.

----
{empty}[.rolename]*main text*footnote:[The footnote.]
----

Another way to solve the problem is to write the paragraph as a literal paragraph, then override the implicit style using the explicit style normal.

----
[normal]
 [.rolename]*main text*footnote:[The footnote.]
----

The normal style often comes in handy when you want to prevent the parser from matching beginning-of-line syntax.
"#
    );

    // A line that starts with `[` and ends with `]` is consumed as a block
    // attribute line, not as paragraph text. Because no block follows it, the
    // attribute list is left dangling (`MissingBlockAfterTitleOrAttributeList`)
    // and the intended inline formatting is never applied: the rendered content
    // is identical to the raw source.
    let mut parser = Parser::default();
    assert_eq!(
        parser.parse("[.rolename]*main text*footnote:[The footnote.]"),
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
                        data: "[.rolename]*main text*footnote:[The footnote.]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "[.rolename]*main text*footnote:[The footnote.]",
                },
                source: Span {
                    data: "[.rolename]*main text*footnote:[The footnote.]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },),],
            source: Span {
                data: "[.rolename]*main text*footnote:[The footnote.]",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[
                Warning {
                    source: Span {
                        data: ".rolename]*main text*footnote:[The footnote.",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                    warning: WarningType::EmptyShorthandItem,
                },
                Warning {
                    source: Span {
                        data: "[.rolename]*main text*footnote:[The footnote.]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    warning: WarningType::MissingBlockAfterTitleOrAttributeList,
                },
            ],
            source_map: SourceMap(&[]),
            catalog: Catalog::default(),
        }
    );

    // Prefixing with `{empty}` means the line no longer begins with `[`, so the
    // block-attribute-line match is disrupted and the line is parsed as an
    // ordinary paragraph with no warnings.
    assert_eq!(
        Parser::default().parse("{empty}[.rolename]*main text*footnote:[The footnote.]"),
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
                        data: "{empty}[.rolename]*main text*footnote:[The footnote.]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "[.rolename]*main text*footnote:[The footnote.]",
                },
                source: Span {
                    data: "{empty}[.rolename]*main text*footnote:[The footnote.]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },),],
            source: Span {
                data: "{empty}[.rolename]*main text*footnote:[The footnote.]",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog::default(),
        }
    );

    // Writing the bracketed text as an indented (literal) paragraph and applying
    // the explicit `[normal]` style attaches `normal` as the block's attribute
    // list, so the bracketed line is treated as paragraph content rather than as
    // block metadata, again with no warnings.
    assert_eq!(
        Parser::default().parse("[normal]\n [.rolename]*main text*footnote:[The footnote.]"),
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
                        data: " [.rolename]*main text*footnote:[The footnote.]",
                        line: 2,
                        col: 1,
                        offset: 9,
                    },
                    rendered: "[.rolename]*main text*footnote:[The footnote.]",
                },
                source: Span {
                    data: "[normal]\n [.rolename]*main text*footnote:[The footnote.]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: Some(Attrlist {
                    attributes: &[ElementAttribute {
                        name: None,
                        shorthand_items: &["normal"],
                        value: "normal",
                    },],
                    anchor: None,
                    source: Span {
                        data: "normal",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },),],
            source: Span {
                data: "[normal]\n [.rolename]*main text*footnote:[The footnote.]",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog::default(),
        }
    );
}
