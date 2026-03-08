use crate::tests::prelude::*;

track_file!("docs/modules/document/pages/multiple-authors.adoc");

non_normative!(
    r#"
= Add Multiple Authors to a Document

The xref:author-line.adoc[author line] is the only way to assign more than one author to a document for display in the byline.
Additionally, only the HTML 5 and Docbook converters can convert documents with multiple authors.

"#
);

#[test]
fn multi_author_syntax() {
    non_normative!(
        r#"
== Multi-author syntax

"#
    );

    verifies!(
        r#"
The information for each author is concluded with a semicolon (`;`).

.Author line structure for multiple authors
[source]
----
= Document Title
firstname middlename lastname <email>; firstname middlename lastname <email>
----

Directly after each author's last name or optional email, enter a semicolon (`;`) followed by a space, and then enter the next author's information.

"#
    );

    let doc = Parser::default().parse("= The Intrepid Chronicles\nKismet R. Lee <kismet@asciidoctor.org>; B. Steppenwolf; Pax Draeke <pax@asciidoctor.org>");

    assert_eq!(
        doc,
        Document {
            header: Header {
                title_source: Some(Span {
                    data: "The Intrepid Chronicles",
                    line: 1,
                    col: 3,
                    offset: 2,
                },),
                title: Some("The Intrepid Chronicles",),
                attributes: &[],
                author_line: Some(AuthorLine {
                    authors: &[
                        Author {
                            name: "Kismet R. Lee",
                            firstname: "Kismet",
                            middlename: Some("R.",),
                            lastname: Some("Lee",),
                            email: Some("kismet@asciidoctor.org",),
                        },
                        Author {
                            name: "B. Steppenwolf",
                            firstname: "B.",
                            middlename: None,
                            lastname: Some("Steppenwolf",),
                            email: None,
                        },
                        Author {
                            name: "Pax Draeke",
                            firstname: "Pax",
                            middlename: None,
                            lastname: Some("Draeke",),
                            email: Some("pax@asciidoctor.org",),
                        },
                    ],
                    source: Span {
                        data: "Kismet R. Lee <kismet@asciidoctor.org>; B. Steppenwolf; Pax Draeke <pax@asciidoctor.org>",
                        line: 2,
                        col: 1,
                        offset: 26,
                    },
                },),
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= The Intrepid Chronicles\nKismet R. Lee <kismet@asciidoctor.org>; B. Steppenwolf; Pax Draeke <pax@asciidoctor.org>",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[],
            source: Span {
                data: "= The Intrepid Chronicles\nKismet R. Lee <kismet@asciidoctor.org>; B. Steppenwolf; Pax Draeke <pax@asciidoctor.org>",
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
fn escape_trailing_character_reference() {
    non_normative!(
        r#"
=== Escape a trailing character reference

"#
    );

    verifies!(
        r#"
If an author name segment ends with a character reference (e.g., `\&#174;`), you must escape it from processing.
One way to escape it is to add a trailing attribute reference (e.g., `\{empty}`).
If the character reference appears at the end of the last author name segment, you can use a second semicolon instead.

A better way of escaping the character reference is to replace it with an attribute reference (e.g., `\{reg}`).

Even if the character reference is escaped, the segments of the author name will not be processed.
Instead, the whole name will be assigned to the `author` and `firstname` attributes.
This limitation may be lifted in the future.

"#
    );

    let doc = Parser::default().parse("= Document Title\nAsciiDoc&#174;{empty} WG; Another Author");

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
                attributes: &[],
                author_line: Some(AuthorLine {
                    authors: &[
                        Author {
                            name: "AsciiDoc&#174; WG",
                            firstname: "AsciiDoc&#174;",
                            middlename: None,
                            lastname: Some("WG"),
                            email: None,
                        },
                        Author {
                            name: "Another Author",
                            firstname: "Another",
                            middlename: None,
                            lastname: Some("Author",),
                            email: None,
                        },
                    ],
                    source: Span {
                        data: "AsciiDoc&#174;{empty} WG; Another Author",
                        line: 2,
                        col: 1,
                        offset: 17,
                    },
                },),
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= Document Title\nAsciiDoc&#174;{empty} WG; Another Author",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[],
            source: Span {
                data: "= Document Title\nAsciiDoc&#174;{empty} WG; Another Author",
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
fn list_multiple_authors_on_author_line() {
    non_normative!(
        r#"
== List multiple authors on the author line

"#
    );

    verifies!(
        r#"
The author line in <<ex-line-multiple>> lists the information for three authors.
Each author's information is separated by a semicolon (`;`).
Notice that the author _B. Steppenwolf_ doesn't have an email, so the semicolon is placed at the end of their name.

.An author line with three authors and two email addresses
[source#ex-line-multiple]
----
include::example$multiple-authors.adoc[tag=header]
----

The result of <<ex-line-multiple>> is displayed below.

image::author-line-with-multiple-authors.png[Multiple authors and their emails displayed on the byline,role=screenshot]

The information for each author can also be xref:reference-author-attributes.adoc#reference-multiple-authors[referenced in the document] using their respective built-in attribute.

"#
    );

    // Test the exact example from the documentation.
    let doc = Parser::default().parse("= The Intrepid Chronicles\nKismet R. Lee <kismet@asciidoctor.org>; B. Steppenwolf; Pax Draeke <pax@asciidoctor.org>");

    assert_eq!(
        doc,
        Document {
            header: Header {
                title_source: Some(Span {
                    data: "The Intrepid Chronicles",
                    line: 1,
                    col: 3,
                    offset: 2,
                },),
                title: Some("The Intrepid Chronicles",),
                attributes: &[],
                author_line: Some(AuthorLine {
                    authors: &[
                        Author {
                            name: "Kismet R. Lee",
                            firstname: "Kismet",
                            middlename: Some("R.",),
                            lastname: Some("Lee",),
                            email: Some("kismet@asciidoctor.org",),
                        },
                        Author {
                            name: "B. Steppenwolf",
                            firstname: "B.",
                            middlename: None,
                            lastname: Some("Steppenwolf",),
                            email: None,
                        },
                        Author {
                            name: "Pax Draeke",
                            firstname: "Pax",
                            middlename: None,
                            lastname: Some("Draeke",),
                            email: Some("pax@asciidoctor.org",),
                        },
                    ],
                    source: Span {
                        data: "Kismet R. Lee <kismet@asciidoctor.org>; B. Steppenwolf; Pax Draeke <pax@asciidoctor.org>",
                        line: 2,
                        col: 1,
                        offset: 26,
                    },
                },),
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= The Intrepid Chronicles\nKismet R. Lee <kismet@asciidoctor.org>; B. Steppenwolf; Pax Draeke <pax@asciidoctor.org>",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[],
            source: Span {
                data: "= The Intrepid Chronicles\nKismet R. Lee <kismet@asciidoctor.org>; B. Steppenwolf; Pax Draeke <pax@asciidoctor.org>",
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
fn preserve_semicolon_in_character_reference_with_trailing_attribute() {
    verifies!(
        r#"
If an author name ends with with a character reference, you can preserve the semicolon in the character reference by adding a trailing attribute reference:

----
AsciiDoc&#174;{empty} WG; Another Author
----

"#
    );

    // Test the exact example from the documentation.
    let doc = Parser::default().parse("= Document Title\nAsciiDoc&#174;{empty} WG; Another Author");

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
                attributes: &[],
                author_line: Some(AuthorLine {
                    authors: &[
                        Author {
                            name: "AsciiDoc&#174; WG",
                            firstname: "AsciiDoc&#174;",
                            middlename: None,
                            lastname: Some("WG"),
                            email: None,
                        },
                        Author {
                            name: "Another Author",
                            firstname: "Another",
                            middlename: None,
                            lastname: Some("Author",),
                            email: None,
                        },
                    ],
                    source: Span {
                        data: "AsciiDoc&#174;{empty} WG; Another Author",
                        line: 2,
                        col: 1,
                        offset: 17,
                    },
                },),
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= Document Title\nAsciiDoc&#174;{empty} WG; Another Author",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[],
            source: Span {
                data: "= Document Title\nAsciiDoc&#174;{empty} WG; Another Author",
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
fn move_character_reference_to_attribute() {
    verifies!(
        r#"
Another solution entails moving the character reference to an attribute and inserting it using an attribute reference:

----
:reg: &#174;
AsciiDoc{reg} WG; Another Author
----

Even though the character reference is escaped, the segments of the author name will not be processed.

"#
    );

    // Test the exact example from the documentation.
    let doc =
        Parser::default().parse(":reg: &#174;\n= Document Title\nAsciiDoc{reg} WG; Another Author");

    assert_eq!(
        doc,
        Document {
            header: Header {
                title_source: Some(Span {
                    data: "Document Title",
                    line: 2,
                    col: 3,
                    offset: 15,
                },),
                title: Some("Document Title",),
                attributes: &[Attribute {
                    name: Span {
                        data: "reg",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                    value_source: Some(Span {
                        data: "&#174;",
                        line: 1,
                        col: 7,
                        offset: 6,
                    },),
                    value: InterpretedValue::Value("&amp;#174;",),
                    source: Span {
                        data: ":reg: &#174;",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },],
                author_line: Some(AuthorLine {
                    authors: &[
                        Author {
                            name: "AsciiDoc&amp;#174; WG",
                            firstname: "AsciiDoc&amp;#174;",
                            middlename: None,
                            lastname: Some("WG",),
                            email: None,
                        },
                        Author {
                            name: "Another Author",
                            firstname: "Another",
                            middlename: None,
                            lastname: Some("Author",),
                            email: None,
                        },
                    ],
                    source: Span {
                        data: "AsciiDoc{reg} WG; Another Author",
                        line: 3,
                        col: 1,
                        offset: 30,
                    },
                },),
                revision_line: None,
                comments: &[],
                source: Span {
                    data: ":reg: &#174;\n= Document Title\nAsciiDoc{reg} WG; Another Author",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[],
            source: Span {
                data: ":reg: &#174;\n= Document Title\nAsciiDoc{reg} WG; Another Author",
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
