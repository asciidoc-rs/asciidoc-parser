use crate::tests::prelude::*;

track_file!("docs/modules/ROOT/pages/document-structure.adoc");

non_normative!(
    r#"
= Document Structure
:page-aliases: document.adoc

On this page, you'll learn about the overall structure of an AsciiDoc document.
Don't worry about the details of the syntax at this point.
That topic will be covered thoroughly later in the documentation.
Right now, we're just aiming to get a sense of what makes up an AsciiDoc document.

"#
);

mod documents {
    use crate::tests::prelude::*;

    non_normative!(
        r#"
== Documents

AsciiDoc is a plain text writing format with no boilerplate enclosure or prologue.
An AsciiDoc document may consist of only a single sentence (or even a single character, to be academic).

"#
    );

    #[test]
    fn single_sentence() {
        verifies!(
            r#"
The example below is a valid AsciiDoc document.
It contains a single paragraph that consists of a single sentence.

----
This is a basic AsciiDoc document.
----

"#
        );

        assert_eq!(
            Parser::default().parse("This is a basic AsciiDoc document.\n"),
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
                        offset: 0
                    }
                },
                source: Span {
                    data: "This is a basic AsciiDoc document.",
                    line: 1,
                    col: 1,
                    offset: 0
                },
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "This is a basic AsciiDoc document.",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "This is a basic AsciiDoc document.",
                    },
                    source: Span {
                        data: "This is a basic AsciiDoc document.",
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
                })],
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }

    #[test]
    fn multiple_paragraphs() {
        verifies!(
            r#"
Of course, you can have more content than a single sentence!
What we want to emphasize here is that it's simple to get started.

An AsciiDoc document is a series of blocks that are stacked linewise.
These blocks are typically offset from one another by an empty line.
(While these empty lines aren't always required, we do recommend using them for readability.)

To expand the previous document from one paragraph to two, you'd separate the two paragraphs by an empty line:

----
This is a basic AsciiDoc document.

This document contains two paragraphs.
----

"#
        );

        assert_eq!(
            Parser::default().parse(
                "This is a basic AsciiDoc document.\n\nThis document contains two paragraphs.\n"
            ),
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
                        offset: 0
                    }
                },
                source: Span {
                    data: "This is a basic AsciiDoc document.\n\nThis document contains two paragraphs.",
                    line: 1,
                    col: 1,
                    offset: 0
                },
                blocks: &[
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "This is a basic AsciiDoc document.",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "This is a basic AsciiDoc document.",
                        },
                        source: Span {
                            data: "This is a basic AsciiDoc document.",
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
                    }),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "This document contains two paragraphs.",
                                line: 3,
                                col: 1,
                                offset: 36,
                            },
                            rendered: "This document contains two paragraphs.",
                        },
                        source: Span {
                            data: "This document contains two paragraphs.",
                            line: 3,
                            col: 1,
                            offset: 36,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    })
                ],
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }

    #[test]
    fn header() {
        verifies!(
            r#"
An AsciiDoc document may begin with a document header.
Although the document header is optional, it's often used because it allows you to specify a document title as well as document-wide configuration and reusable text in the form of document attributes.

[source]
----
= Document Title
Author Name
:reproducible:

This is a basic AsciiDoc document by {author}.

This document contains two paragraphs.
It also has a header that specifies the document title and some attibutes.
----

"#
        );

        let doc = Parser::default().parse(
            "= Document Title\nAuthor Name\n:reproducible:\n\nThis is a basic AsciiDoc document by {author}.\n\nThis document contains two paragraphs.\nIt also has a header that specifies the document title."
        );

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: Some(Span {
                        data: "Document Title",
                        line: 1,
                        col: 3,
                        offset: 2,
                    },),
                    title: Some("Document Title",),
                    attributes: &[Attribute {
                        name: Span {
                            data: "reproducible",
                            line: 3,
                            col: 2,
                            offset: 30,
                        },
                        value_source: None,
                        value: InterpretedValue::Set,
                        source: Span {
                            data: ":reproducible:",
                            line: 3,
                            col: 1,
                            offset: 29,
                        },
                    },],
                    author_line: Some(AuthorLine {
                        authors: &[Author {
                            name: "Author Name",
                            firstname: "Author",
                            middlename: None,
                            lastname: Some("Name",),
                            email: None,
                        },],
                        source: Span {
                            data: "Author Name",
                            line: 2,
                            col: 1,
                            offset: 17,
                        },
                    },),
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "= Document Title\nAuthor Name\n:reproducible:",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "This is a basic AsciiDoc document by {author}.",
                                line: 5,
                                col: 1,
                                offset: 45,
                            },
                            rendered: "This is a basic AsciiDoc document by Author Name.",
                        },
                        source: Span {
                            data: "This is a basic AsciiDoc document by {author}.",
                            line: 5,
                            col: 1,
                            offset: 45,
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
                                data: "This document contains two paragraphs.\nIt also has a header that specifies the document title.",
                                line: 7,
                                col: 1,
                                offset: 93,
                            },
                            rendered: "This document contains two paragraphs.\nIt also has a header that specifies the document title.",
                        },
                        source: Span {
                            data: "This document contains two paragraphs.\nIt also has a header that specifies the document title.",
                            line: 7,
                            col: 1,
                            offset: 93,
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
                    data: "= Document Title\nAuthor Name\n:reproducible:\n\nThis is a basic AsciiDoc document by {author}.\n\nThis document contains two paragraphs.\nIt also has a header that specifies the document title.",
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

    non_normative!(
        r#"
Almost any combination of blocks constitutes a valid AsciiDoc document (with some structural requirements dictated by the xref:document:doctypes.adoc[document type]).
Documents can range from a single sentence to a multi-part book.

"#
    );
}

mod lines {
    use crate::tests::prelude::*;

    #[test]
    fn section_title() {
        verifies!(
            r#"
== Lines

The line is a significant building block in AsciiDoc.
A line is defined as text that's separated on either side by a newline character or the boundary of the document.
Many aspects of the syntax must occupy a whole line.
That's why we say AsciiDoc is a line-oriented language.

For example, a section title must be on a line by itself.
The same is true for an attribute entry, a block title, a block attribute list, a block macro, a list item, a block delimiter, and so forth.

.Example of a section title, which must occupy a single line
[source]
----
== Section Title
----

"#
        );

        let span = crate::Span::new("== Section Title\n");
        let l = span.take_line();

        assert_eq!(
            l.after,
            Span {
                data: "",
                line: 2,
                col: 1,
                offset: 17
            }
        );

        assert_eq!(
            l.item,
            Span {
                data: "== Section Title",
                line: 1,
                col: 1,
                offset: 0
            }
        );
    }

    #[test]
    fn one_line_attribute() {
        verifies!(
            r#"
.Example of an attribute entry, which must also occupy at least one line
[source]
-----
:name: value
-----

"#
        );

        let mi = crate::document::Attribute::parse(
            crate::Span::new(":name: value\n"),
            &Parser::default(),
        )
        .unwrap();

        assert_eq!(
            mi.item,
            Attribute {
                name: Span {
                    data: "name",
                    line: 1,
                    col: 2,
                    offset: 1,
                },
                value_source: Some(Span {
                    data: "value",
                    line: 1,
                    col: 8,
                    offset: 7,
                }),
                value: InterpretedValue::Value("value"),
                source: Span {
                    data: ":name: value",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            }
        );

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 2,
                col: 1,
                offset: 13
            }
        );
    }

    #[test]
    fn two_line_attribute() {
        verifies!(
            r#"
.Example of an attribute entry that extends to two lines
[source]
-----
:name: value \
more value
-----

Notice that the attribute entry must be explicitly continued to extend onto another line.

"#
        );

        let mi = crate::document::Attribute::parse(
            crate::Span::new(":name: value \\\nmore value\n"),
            &Parser::default(),
        )
        .unwrap();

        assert_eq!(
            mi.item,
            Attribute {
                name: Span {
                    data: "name",
                    line: 1,
                    col: 2,
                    offset: 1,
                },
                value_source: Some(Span {
                    data: "value \\\nmore value",
                    line: 1,
                    col: 8,
                    offset: 7,
                }),
                value: InterpretedValue::Value("value more value"),
                source: Span {
                    data: ":name: value \\\nmore value",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            }
        );

        assert_eq!(mi.item.value(), InterpretedValue::Value("value more value"));

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 3,
                col: 1,
                offset: 26
            }
        );
    }

    #[test]
    fn blank_line_between_header_and_body() {
        verifies!(
            r#"
Empty lines can also be significant.
For example, a single empty line separates the header from the body.
"#
        );

        let mut parser = Parser::default();
        let doc = parser.parse("= Title\n:name: value\n\nBody text goes here.");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: Some(Span {
                        data: "Title",
                        line: 1,
                        col: 3,
                        offset: 2,
                    },),
                    title: Some("Title",),
                    attributes: &[Attribute {
                        name: Span {
                            data: "name",
                            line: 2,
                            col: 2,
                            offset: 9,
                        },
                        value_source: Some(Span {
                            data: "value",
                            line: 2,
                            col: 8,
                            offset: 15,
                        },),
                        value: InterpretedValue::Value("value",),
                        source: Span {
                            data: ":name: value",
                            line: 2,
                            col: 1,
                            offset: 8,
                        },
                    },],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "= Title\n:name: value",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "Body text goes here.",
                            line: 4,
                            col: 1,
                            offset: 22,
                        },
                        rendered: "Body text goes here.",
                    },
                    source: Span {
                        data: "Body text goes here.",
                        line: 4,
                        col: 1,
                        offset: 22,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: "= Title\n:name: value\n\nBody text goes here.",
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

    non_normative!(
        r#"
Many blocks are also separated by an empty line, as you saw in the example earlier with two paragraphs.

In contrast, lines within paragraph content are insignificant.
Keep these points in mind as you're learning about the AsciiDoc syntax.

"#
    );
}

// Skipping because this is well-covered in other parts of the language
// description.
non_normative!(
    r#"
== Blocks

Blocks in an AsciiDoc document lay down the document structure.
Some blocks may contain other blocks, so the document structure is inherently hierarchical (i.e., a tree structure).
You can inspect this section structure, for example, by enabling the automatic table of contents.
Examples of blocks include paragraphs, sections, lists, delimited blocks, tables, and block macros.

Blocks are easy to identify because they're usually offset from other blocks by an empty line (though not always required).
Blocks always start on a new line, terminate at the end of a line, and are aligned to the left margin.

Every block can have one or more lines of block metadata.
This metadata can be in the form of block attributes, a block anchor, or a block title.
These metadata lines must be above and directly adjacent to the block itself.

Sections, non-verbatim delimited blocks, and AsciiDoc table cells may contain other blocks.
Despite the fact that blocks form a hierarchy, even nested blocks start at the left margin.
By requiring blocks to start at the left margin, it avoids the tedium of having to track and maintain levels of indentation.
It also happens to make the content more reusable.

== Text and inline elements

Surrounded by the markers, delimiters, and metadata lines is the text.
The text is the main focus of a document and the reason the AsciiDoc syntax gives it so much room to breathe.
Text is most often found in the lines of a block (e.g., paragraph), the block title (e.g., section title), and in list items, though there are other places where it can exist.

Text is subject to substitutions.
Substitutions interpret markup as text formatting, replace macros with text or non-text elements, expand attribute references, and perform other sorts of text replacement.

Normal text is subject to all substitutions, unless specified otherwise.
Verbatim text is subject to a minimal set of substitutions to allow it to be displayed in the output as it appears in the source.
It's also possible to disable all substitutions in order to pass the text through to the output unmodified (i.e., raw).
The parsing of text ends up being a mix of inline elements and other forms of transformations.

== Encodings and AsciiDoc files

An AsciiDoc file is a text file that has the _.adoc_ file extension (e.g., [.path]_document.adoc_).
Most AsciiDoc processors assume the text in the file uses UTF-8 encoding.
UTF-16 encodings are supported only if the file starts with a BOM.

An AsciiDoc processor can process AsciiDoc from a string (i.e., character sequence).
However, most of the time you'll save your AsciiDoc documents to a file.
"#
);
