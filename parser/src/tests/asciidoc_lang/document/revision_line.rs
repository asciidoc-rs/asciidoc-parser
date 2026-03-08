use crate::tests::prelude::*;

track_file!("docs/modules/document/pages/revision-line.adoc");

non_normative!(
    r#"
= Using the Revision Line

The revision attributes can be set and assigned values using the revision line.

[#revision-line]
== What's the revision line?

The [.term]*revision line* is the line directly after the author line in the document header.
When the content on this line is structured correctly, the processor assigns the content to the built-in `revnumber`, `revdate` and `revremark` attributes.

== When can I use the revision line?

In order for the processor to properly detect the revision line and assign the content to the correct attributes, all of the following criteria must be met:

. The document header must contain a xref:title.adoc[document title] and an author line.
. The revision information must be entered on the line directly beneath the xref:author-line.adoc[author line].
. The revision line must start with the revision number.
. The revision number must contain at least one number, but a number doesn't have to be the first character in the version.
. The values in the revision line must be placed in a specific order and separated with the correct syntax.

.Revision line structure
[source]
----
= Document Title
author <email>
revision number, revision date: revision remark
----

When using the revision line, the revision date and remark are optional.

"#
);

#[test]
fn revision_number_only() {
    verifies!(
        r#"
* `pass:q[#v#]7.5` When the revision line only contains a revision number, prefix the number with a `v`.
"#
    );

    let mut parser = Parser::default();
    let doc = parser.parse("= Document Title\nauthor\nv7.5");

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
                    authors: &[Author {
                        name: "author",
                        firstname: "author",
                        middlename: None,
                        lastname: None,
                        email: None,
                    },],
                    source: Span {
                        data: "author",
                        line: 2,
                        col: 1,
                        offset: 17,
                    },
                },),
                revision_line: Some(RevisionLine {
                    revnumber: Some("7.5",),
                    revdate: "",
                    revremark: None,
                    source: Span {
                        data: "v7.5",
                        line: 3,
                        col: 1,
                        offset: 24,
                    },
                },),
                comments: &[],
                source: Span {
                    data: "= Document Title\nauthor\nv7.5",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[],
            source: Span {
                data: "= Document Title\nauthor\nv7.5",
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
fn version_and_date() {
    verifies!(
        r#"
* `7.5pass:q[#,#] 1-29-2020` When the revision line contains a version and a date, separate the version number from the date with a comma (`,`).
"#
    );

    let mut parser = Parser::default();
    let doc = parser.parse("= Document Title\nauthor\nv7.5, 1-29-2020");

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
                    authors: &[Author {
                        name: "author",
                        firstname: "author",
                        middlename: None,
                        lastname: None,
                        email: None,
                    },],
                    source: Span {
                        data: "author",
                        line: 2,
                        col: 1,
                        offset: 17,
                    },
                },),
                revision_line: Some(RevisionLine {
                    revnumber: Some("7.5",),
                    revdate: "1-29-2020",
                    revremark: None,
                    source: Span {
                        data: "v7.5, 1-29-2020",
                        line: 3,
                        col: 1,
                        offset: 24,
                    },
                },),
                comments: &[],
                source: Span {
                    data: "= Document Title\nauthor\nv7.5, 1-29-2020",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[],
            source: Span {
                data: "= Document Title\nauthor\nv7.5, 1-29-2020",
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
fn v_prefix_optional() {
    verifies!(
        r#"
A `v` prefix before the version number is optional.
"#
    );

    let mut parser = Parser::default();
    let doc = parser.parse("= Document Title\nauthor\n7.5, 1-29-2020");

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
                    authors: &[Author {
                        name: "author",
                        firstname: "author",
                        middlename: None,
                        lastname: None,
                        email: None,
                    },],
                    source: Span {
                        data: "author",
                        line: 2,
                        col: 1,
                        offset: 17,
                    },
                },),
                revision_line: Some(RevisionLine {
                    revnumber: Some("7.5",),
                    revdate: "1-29-2020",
                    revremark: None,
                    source: Span {
                        data: "7.5, 1-29-2020",
                        line: 3,
                        col: 1,
                        offset: 24,
                    },
                },),
                comments: &[],
                source: Span {
                    data: "= Document Title\nauthor\n7.5, 1-29-2020",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[],
            source: Span {
                data: "= Document Title\nauthor\n7.5, 1-29-2020",
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
fn version_and_remark() {
    verifies!(
        r#"
* `7.5pass:q[#:#] A new analysis` When the revision line contains a version and a remark, separate the version number from the remark with a colon (`:`).
"#
    );

    let mut parser = Parser::default();
    let doc = parser.parse("= Document Title\nauthor\n7.5: A new analysis");

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
                    authors: &[Author {
                        name: "author",
                        firstname: "author",
                        middlename: None,
                        lastname: None,
                        email: None,
                    },],
                    source: Span {
                        data: "author",
                        line: 2,
                        col: 1,
                        offset: 17,
                    },
                },),
                revision_line: Some(RevisionLine {
                    revnumber: None,
                    revdate: "7.5",
                    revremark: Some("A new analysis",),
                    source: Span {
                        data: "7.5: A new analysis",
                        line: 3,
                        col: 1,
                        offset: 24,
                    },
                },),
                comments: &[],
                source: Span {
                    data: "= Document Title\nauthor\n7.5: A new analysis",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[],
            source: Span {
                data: "= Document Title\nauthor\n7.5: A new analysis",
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
fn version_and_remark_with_v_prefix() {
    verifies!(
        r#"
A `v` prefix before the version number is optional.
"#
    );

    let mut parser = Parser::default();
    let doc = parser.parse("= Document Title\nauthor\nv7.5: A new analysis");

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
                    authors: &[Author {
                        name: "author",
                        firstname: "author",
                        middlename: None,
                        lastname: None,
                        email: None,
                    },],
                    source: Span {
                        data: "author",
                        line: 2,
                        col: 1,
                        offset: 17,
                    },
                },),
                revision_line: Some(RevisionLine {
                    revnumber: Some("7.5",),
                    revdate: "",
                    revremark: Some("A new analysis",),
                    source: Span {
                        data: "v7.5: A new analysis",
                        line: 3,
                        col: 1,
                        offset: 24,
                    },
                },),
                comments: &[],
                source: Span {
                    data: "= Document Title\nauthor\nv7.5: A new analysis",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[],
            source: Span {
                data: "= Document Title\nauthor\nv7.5: A new analysis",
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
fn version_date_and_remark() {
    verifies!(
        r#"
* `7.5pass:q[#,#] 1-29-2020pass:q[#:#] A new analysis` When the revision line contains a version, date, and a remark, separate the version number from the date with a comma (`,`) and separate the date from the remark with a colon (`:`).
"#
    );

    let mut parser = Parser::default();
    let doc = parser.parse("= Document Title\nauthor\nv7.5, 1-29-20: A new analysis");

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
                    authors: &[Author {
                        name: "author",
                        firstname: "author",
                        middlename: None,
                        lastname: None,
                        email: None,
                    },],
                    source: Span {
                        data: "author",
                        line: 2,
                        col: 1,
                        offset: 17,
                    },
                },),
                revision_line: Some(RevisionLine {
                    revnumber: Some("7.5",),
                    revdate: "1-29-20",
                    revremark: Some("A new analysis",),
                    source: Span {
                        data: "v7.5, 1-29-20: A new analysis",
                        line: 3,
                        col: 1,
                        offset: 24,
                    },
                },),
                comments: &[],
                source: Span {
                    data: "= Document Title\nauthor\nv7.5, 1-29-20: A new analysis",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[],
            source: Span {
                data: "= Document Title\nauthor\nv7.5, 1-29-20: A new analysis",
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
fn version_date_and_remark_without_v_prefix() {
    verifies!(
        r#"
A `v` prefix before the version number is optional.

"#
    );

    let mut parser = Parser::default();
    let doc = parser.parse("= Document Title\nauthor\n7.5, 1-29-20: A new analysis");

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
                    authors: &[Author {
                        name: "author",
                        firstname: "author",
                        middlename: None,
                        lastname: None,
                        email: None,
                    },],
                    source: Span {
                        data: "author",
                        line: 2,
                        col: 1,
                        offset: 17,
                    },
                },),
                revision_line: Some(RevisionLine {
                    revnumber: Some("7.5",),
                    revdate: "1-29-20",
                    revremark: Some("A new analysis",),
                    source: Span {
                        data: "7.5, 1-29-20: A new analysis",
                        line: 3,
                        col: 1,
                        offset: 24,
                    },
                },),
                comments: &[],
                source: Span {
                    data: "= Document Title\nauthor\n7.5, 1-29-20: A new analysis",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[],
            source: Span {
                data: "= Document Title\nauthor\n7.5, 1-29-20: A new analysis",
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
fn ex_line() {
    verifies!(
        r#"
== Assign revision information using the revision line

The revision line in <<ex-line>> contains a revision number, date, and remark.

.Revision line with a version, date and remark
[source#ex-line]
----
= The Intrepid Chronicles
Kismet Lee <.>
2.9, October 31, 2021: Fall incarnation <.> <.> <.>
----
<.> The author line must be directly above the revision line.
<.> The revision line must begin with the revision number.
<.> The date is separated from the version by a comma (`,`).
The date can contain letters, numbers, symbols, and attribute references.
<.> The remark is separated from the date by a colon (`:`).

"#
    );

    let mut parser = Parser::default();
    let doc = parser
        .parse("= The Intrepid Chronicles\nKismet Lee\n2.9, October 31, 2021: Fall incarnation");

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
                    authors: &[Author {
                        name: "Kismet Lee",
                        firstname: "Kismet",
                        middlename: None,
                        lastname: Some("Lee",),
                        email: None,
                    },],
                    source: Span {
                        data: "Kismet Lee",
                        line: 2,
                        col: 1,
                        offset: 26,
                    },
                },),
                revision_line: Some(RevisionLine {
                    revnumber: Some("2.9",),
                    revdate: "October 31, 2021",
                    revremark: Some("Fall incarnation",),
                    source: Span {
                        data: "2.9, October 31, 2021: Fall incarnation",
                        line: 3,
                        col: 1,
                        offset: 37,
                    },
                },),
                comments: &[],
                source: Span {
                    data: "= The Intrepid Chronicles\nKismet Lee\n2.9, October 31, 2021: Fall incarnation",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[],
            source: Span {
                data: "= The Intrepid Chronicles\nKismet Lee\n2.9, October 31, 2021: Fall incarnation",
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
When the default stylesheet is applied, the revision information is displayed on the same line as the author information.
Note that the revision number is preceded with the word _Version_.
This label is automatically added by the processor.
It can be xref:version-label.adoc[changed or turned off with the version-label attribute].

image::revision-line.png["Byline with a version number, revision date, and revision remark",role=screenshot]

"#
);

#[test]
fn ex_prefix() {
    verifies!(
        r#"
Let's look at another revision line.
In <<ex-prefix>>, the version starts with a letter, the date is a reference to the attribute `docdate`, and there's a Unicode glyph in the remark.

.Revision line with a version prefix, attribute reference and Unicode glyph
[source#ex-prefix]
----
include::example$revision-line-with-version-prefix.adoc[]
----

The result of <<ex-prefix>> is displayed below.

image::revision-line-with-version-prefix.png["Byline with the date derived from docdate and a remark with a Unicode glyph",role=screenshot]

_LPR_ was removed from the version because any letters or symbols that precede the revision number in the revision line are dropped.
To display the letters or symbols in front of a revision number, xref:revision-attribute-entries.adoc[set revnumber using an attribute entry].
"#
    );

    let mut parser = Parser::default();
    let doc = parser
        .parse("= The Intrepid Chronicles\nKismet Lee\nLPR55, {docdate}: A Special ⚄ Edition");

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
                    authors: &[Author {
                        name: "Kismet Lee",
                        firstname: "Kismet",
                        middlename: None,
                        lastname: Some("Lee",),
                        email: None,
                    },],
                    source: Span {
                        data: "Kismet Lee",
                        line: 2,
                        col: 1,
                        offset: 26,
                    },
                },),
                revision_line: Some(RevisionLine {
                    revnumber: Some("55",),
                    revdate: "{docdate}",
                    revremark: Some("A Special ⚄ Edition",),
                    source: Span {
                        data: "LPR55, {docdate}: A Special ⚄ Edition",
                        line: 3,
                        col: 1,
                        offset: 37,
                    },
                },),
                comments: &[],
                source: Span {
                    data: "= The Intrepid Chronicles\nKismet Lee\nLPR55, {docdate}: A Special ⚄ Edition",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[],
            source: Span {
                data: "= The Intrepid Chronicles\nKismet Lee\nLPR55, {docdate}: A Special ⚄ Edition",
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
