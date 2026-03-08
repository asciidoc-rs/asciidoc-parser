use crate::{Parser, blocks::SimpleBlockStyle, tests::prelude::*};

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
