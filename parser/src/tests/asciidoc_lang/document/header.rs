use crate::tests::prelude::*;

track_file!("docs/modules/document/pages/header.adoc");

non_normative!(
    r#"
= Document Header

An AsciiDoc document may begin with a document header.
The document header encapsulates the document title, author and revision information, document-wide attributes, and other document metadata.

"#
);

mod structure {
    use crate::{blocks::SimpleBlockStyle, tests::prelude::*};

    non_normative!(
        r#"
== Document header structure

"#
    );

    #[test]
    fn skip_blank_or_comment_lines() {
        verifies!(
            r#"
The optional document header is a series of contiguous lines at the start of the AsciiDoc source, after skipping any empty or comment lines.
If a document has a header, _no content blocks are permitted above it_.
In other words, the document must start with a document header if it has one.

"#
        );

        let doc = Parser::default().parse("\n// comment\n= Title");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: Some(Span {
                        data: "Title",
                        line: 3,
                        col: 3,
                        offset: 14,
                    },),
                    title: Some("Title",),
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[Span {
                        data: "// comment",
                        line: 2,
                        col: 1,
                        offset: 1,
                    },],
                    source: Span {
                        data: "// comment\n= Title",
                        line: 2,
                        col: 1,
                        offset: 1,
                    },
                },
                blocks: &[],
                source: Span {
                    data: "\n// comment\n= Title",
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

    // Treating as non-normative because this condition is well-covered
    // elsewhere.
    non_normative!(
        r#"
[IMPORTANT]
====
[.lead]
*The document header may not contain empty lines.*
The first empty line the processor encounters after the document header begins marks the <<when-does-the-document-header-end,end of the document header>> and the start of the document body.
====

"#
    );

    #[test]
    fn author_revision_lines() {
        verifies!(
            r#"
A header typically begins with a xref:title.adoc[].
When a document title is specified, it may be immediately followed by one or two designated lines of content.
These implicit content lines are used to assign xref:author-information.adoc[] and xref:revision-information.adoc[] to the document.

"#
        );

        let doc = Parser::default().parse("= Document Title\nJohn Doe <john@example.com>\nv1.0, 2023-01-15: Initial release\n\nSome content.");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: Some(Span {
                        data: "Document Title",
                        line: 1,
                        col: 3,
                        offset: 2,
                    }),
                    title: Some("Document Title"),
                    attributes: &[],
                    author_line: Some(AuthorLine {
                        authors: &[Author {
                            name: "John Doe",
                            firstname: "John",
                            middlename: None,
                            lastname: Some("Doe"),
                            email: Some("john@example.com"),
                        }],
                        source: Span {
                            data: "John Doe <john@example.com>",
                            line: 2,
                            col: 1,
                            offset: 17,
                        },
                    }),
                    revision_line: Some(RevisionLine {
                        revnumber: Some("1.0"),
                        revdate: "2023-01-15",
                        revremark: Some("Initial release"),
                        source: Span {
                            data: "v1.0, 2023-01-15: Initial release",
                            line: 3,
                            col: 1,
                            offset: 45,
                        },
                    }),
                    comments: &[],
                    source: Span {
                        data: "= Document Title\nJohn Doe <john@example.com>\nv1.0, 2023-01-15: Initial release",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "Some content.",
                            line: 5,
                            col: 1,
                            offset: 80,
                        },
                        rendered: "Some content.",
                    },
                    source: Span {
                        data: "Some content.",
                        line: 5,
                        col: 1,
                        offset: 80,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                })],
                source: Span {
                    data: "= Document Title\nJohn Doe <john@example.com>\nv1.0, 2023-01-15: Initial release\n\nSome content.",
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

    // Treating as non-normative because these conditions are well-verified in the
    // test suite for header parsing.
    non_normative!(
        r#"
The header may contain the following elements as long as there aren't any empty lines between them:

* optional document title (a level-0 heading)
* optional author line or author and revision lines if the document title is present (should immediately follow the document title)
* optional document-wide attributes (built-in and user-defined) declared using xref:attributes:attribute-entries.adoc[attribute entries],
** includes optional xref:metadata.adoc[metadata], such as a description or keywords
* optional xref:ROOT:comments.adoc#comment-lines[comment lines]

"#
    );

    #[test]
    fn ex_basic_header() {
        verifies!(
            r#"
Notice in <<ex-basic-header>> that there are no empty lines between any of the entries.
In other words, the lines are contiguous.

.Common elements in a header
[source#ex-basic-header]
----
// this comment line is ignored
= Document Title <.>
Kismet R. Lee <kismet@asciidoctor.org> <.>
:description: The document's description. <.>
:sectanchors: <.>
:url-repo: https://my-git-repo.com <.>
<.>
The document body starts here.
----
<.> Document title
<.> Author line
<.> Attribute entry assigning metadata to a built-in document attribute
<.> Attribute entry setting a built-in document attribute
<.> Attribute entry assigning a value to a user-defined document attribute
<.> The document body is separated from the document header by an empty line

There are a few attribute entries in <<ex-basic-header>>.
Each attribute entry, whether built-in or user-defined, must be entered on its own line.
While attribute entries may be placed anywhere in the header, including above the document title, the preferred placement is below the title, if it's present.
Since the document title is optional, it's possible for the header to only consist of attribute entries.

"#
        );

        let doc = Parser::default().parse("// this comment line is ignored\n= Document Title\nKismet R. Lee <kismet@asciidoctor.org>\n:description: The document's description.\n:sectanchors:\n:url-repo: https://my-git-repo.com\n\nThe document body starts here.");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: Some(Span {
                        data: "Document Title",
                        line: 2,
                        col: 3,
                        offset: 34,
                    },),
                    title: Some("Document Title",),
                    attributes: &[
                        Attribute {
                            name: Span {
                                data: "description",
                                line: 4,
                                col: 2,
                                offset: 89,
                            },
                            value_source: Some(Span {
                                data: "The document's description.",
                                line: 4,
                                col: 15,
                                offset: 102,
                            },),
                            value: InterpretedValue::Value("The document's description.",),
                            source: Span {
                                data: ":description: The document's description.",
                                line: 4,
                                col: 1,
                                offset: 88,
                            },
                        },
                        Attribute {
                            name: Span {
                                data: "sectanchors",
                                line: 5,
                                col: 2,
                                offset: 131,
                            },
                            value_source: None,
                            value: InterpretedValue::Set,
                            source: Span {
                                data: ":sectanchors:",
                                line: 5,
                                col: 1,
                                offset: 130,
                            },
                        },
                        Attribute {
                            name: Span {
                                data: "url-repo",
                                line: 6,
                                col: 2,
                                offset: 145,
                            },
                            value_source: Some(Span {
                                data: "https://my-git-repo.com",
                                line: 6,
                                col: 12,
                                offset: 155,
                            },),
                            value: InterpretedValue::Value("https://my-git-repo.com",),
                            source: Span {
                                data: ":url-repo: https://my-git-repo.com",
                                line: 6,
                                col: 1,
                                offset: 144,
                            },
                        },
                    ],
                    author_line: Some(AuthorLine {
                        authors: &[Author {
                            name: "Kismet R. Lee",
                            firstname: "Kismet",
                            middlename: Some("R.",),
                            lastname: Some("Lee",),
                            email: Some("kismet@asciidoctor.org",),
                        },],
                        source: Span {
                            data: "Kismet R. Lee <kismet@asciidoctor.org>",
                            line: 3,
                            col: 1,
                            offset: 49,
                        },
                    },),
                    revision_line: None,
                    comments: &[Span {
                        data: "// this comment line is ignored",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },],
                    source: Span {
                        data: "// this comment line is ignored\n= Document Title\nKismet R. Lee <kismet@asciidoctor.org>\n:description: The document's description.\n:sectanchors:\n:url-repo: https://my-git-repo.com",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "The document body starts here.",
                            line: 8,
                            col: 1,
                            offset: 180,
                        },
                        rendered: "The document body starts here.",
                    },
                    source: Span {
                        data: "The document body starts here.",
                        line: 8,
                        col: 1,
                        offset: 180,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: "// this comment line is ignored\n= Document Title\nKismet R. Lee <kismet@asciidoctor.org>\n:description: The document's description.\n:sectanchors:\n:url-repo: https://my-git-repo.com\n\nThe document body starts here.",
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
}

mod header_end {
    use crate::{blocks::SimpleBlockStyle, tests::prelude::*};

    non_normative!(
        r#"
== When does the document header end?

"#
    );

    #[test]
    fn ex_terminate() {
        verifies!(
            r#"
*The first empty line in the document marks the end of the header.*
The next line after the first empty line that contains content is interpreted as the beginning of the document's body.

.Terminating a document header
[source#ex-terminate]
----
= Document Title
Kismet R. Lee <kismet@asciidoctor.org>
:url-repo: https://my-git-repo.com
<.>
This is the first line of content in the document body. <.>
----
<.> An empty line ends the document header.
<.> After the empty line, the next line with content starts the body of the document.

The first line of the document body can be any valid AsciiDoc content, such as a section heading, paragraph, table, include directive, image, etc.
Any attributes defined below the first empty line are not part of the document header and will not be scoped to the entire document.

"#
        );

        let doc = Parser::default().parse("= Document Title\nKismet R. Lee <kismet@asciidoctor.org>\n:url-repo: https://my-git-repo.com\n\nThis is the first line of content in the document body.");

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
                            data: "url-repo",
                            line: 3,
                            col: 2,
                            offset: 57,
                        },
                        value_source: Some(Span {
                            data: "https://my-git-repo.com",
                            line: 3,
                            col: 12,
                            offset: 67,
                        },),
                        value: InterpretedValue::Value("https://my-git-repo.com",),
                        source: Span {
                            data: ":url-repo: https://my-git-repo.com",
                            line: 3,
                            col: 1,
                            offset: 56,
                        },
                    },],
                    author_line: Some(AuthorLine {
                        authors: &[Author {
                            name: "Kismet R. Lee",
                            firstname: "Kismet",
                            middlename: Some("R.",),
                            lastname: Some("Lee",),
                            email: Some("kismet@asciidoctor.org",),
                        },],
                        source: Span {
                            data: "Kismet R. Lee <kismet@asciidoctor.org>",
                            line: 2,
                            col: 1,
                            offset: 17,
                        },
                    },),
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "= Document Title\nKismet R. Lee <kismet@asciidoctor.org>\n:url-repo: https://my-git-repo.com",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "This is the first line of content in the document body.",
                            line: 5,
                            col: 1,
                            offset: 92,
                        },
                        rendered: "This is the first line of content in the document body.",
                    },
                    source: Span {
                        data: "This is the first line of content in the document body.",
                        line: 5,
                        col: 1,
                        offset: 92,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: "= Document Title\nKismet R. Lee <kismet@asciidoctor.org>\n:url-repo: https://my-git-repo.com\n\nThis is the first line of content in the document body.",
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
}

// This section treated as non-normative because the parser doesn't behave
// differently based on doctype.
non_normative!(
    r#"
== Header requirements per doctype

The header is optional when the `doctype` is `article` or `book`.
A header is required when the document type is `manpage`.
See the xref:asciidoctor:manpage-backend:index.adoc[manpage doctype] section for manual page (man page) requirements.

If you put content blocks above the document header when using the default article doctype, you will see the following warning:

....
level 0 sections can only be used when doctype is book
....

While this warning can be mitigated by changing the doctype to book, it may lead to a secondary warning about an invalid part.
That's because the document title will be repurposed as a part title and any lines that follow it as content blocks.
If you're going to use the book doctype, you must structure your document to use xref:sections:parts.adoc[].

"#
);

// Treating as non-normative because the parser crate doesn't render content.
non_normative!(
    r#"
== Header processing

The information in the document header is displayed by default when converting to a standalone document.
If you don't want the header of a document to be displayed, set the `noheader` attribute in the document's header or via the CLI.

"#
);

// Treating as non-normative because the parser crate doesn't support front
// matter. (This can be handled before passing data in for parsing.)
non_normative!(
    r#"
== Front matter

Many static site generators, such as Jekyll and Middleman, rely on front matter added to the top of the document to determine how to convert the content.
Asciidoctor has a number of attributes available to correctly handle front matter.
See xref:asciidoctor:html-backend:skip-front-matter.adoc[] to learn more.
"#
);
