use crate::tests::prelude::*;

track_file!("docs/modules/document/pages/author-line.adoc");

non_normative!(
    r#"
= Using the Author Line

The author attributes can be implicitly set and assigned values using the author line.

[#author-line]
== What's the author line?

The [.term]*author line* is directly after the document title line in the document header.
When the content on this line is structured correctly, the processor assigns the content to the built-in `author` and `email` attributes.

"#
);

#[test]
fn when_can_i_use() {
    non_normative!(
        r#"
== When can I use the author line?

"#
    );

    verifies!(
        r#"
In order for the processor to properly detect the author line and assign the content to the correct attributes, all of the following criteria must be met:

. The header must contain a xref:title.adoc[document title].
. The author information must be entered on the line directly beneath the document title.
. The author line must start with an author name.
. The content in the author line must be placed in a specific order and separated with the correct syntax.

.Author line structure for single author
[source]
----
= Document Title
firstname middlename lastname <email>
----

The author's middle name is optional.
An email following the author's last name is also optional.
If included, the email address must be enclosed in a pair of angle brackets (`< >`).

TIP: The email can be replaced by a URL, though the value is still stored in the `email` attribute.

The author line also accepts xref:multiple-authors.adoc[multiple authors].

"#
    );

    let doc = Parser::default().parse("= Document Title\nfirstname middlename lastname <email>");

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
                        name: "firstname middlename lastname",
                        firstname: "firstname",
                        middlename: Some("middlename",),
                        lastname: Some("lastname",),
                        email: Some("email",),
                    },],
                    source: Span {
                        data: "firstname middlename lastname <email>",
                        line: 2,
                        col: 1,
                        offset: 17,
                    },
                },),
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= Document Title\nfirstname middlename lastname <email>",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[],
            source: Span {
                data: "= Document Title\nfirstname middlename lastname <email>",
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
fn author_and_email() {
    non_normative!(
        r#"
== Assign an author and email

"#
    );

    verifies!(
        r#"
In <<ex-line>>, let's add an author and their email address using the author line.
The author line must be placed on the line directly below the xref:title.adoc[document title] and start with an author's name.

.Add an author and email using the author line
[source#ex-line]
----
= The Intrepid Chronicles
Kismet R. Lee <kismet@asciidoctor.org> <.> <.>
----
<.> Enter the author's name on the line below the document title.
<.> In a pair of angle brackets (`< >`), enter the author's email.

Remember, a middle name and email are optional.
The processor assigns the content on the author line to the built-in attributes using word position, word count, and syntax.

"#
    );

    let doc = Parser::default()
        .parse("= The Intrepid Chronicles\nKismet R. Lee <kismet@asciidoctor.org>");

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
                        offset: 26,
                    },
                },),
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= The Intrepid Chronicles\nKismet R. Lee <kismet@asciidoctor.org>",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[],
            source: Span {
                data: "= The Intrepid Chronicles\nKismet R. Lee <kismet@asciidoctor.org>",
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
TIP: The email can be replaced by a URL, though the value is still stored in the `email` attribute.

When the default stylesheet is applied, the author information is displayed on the byline.
The [.term]*byline* displays the author information and the xref:revision-information.adoc[revision information] directly beneath the document's title.

image::author-line-with-author-and-email.png[Author and email information displayed on the byline,role=screenshot]

.Using attribute references in the author line
****
The author line is not intended to support the arbitrary placement of attribute references.
While attribute references are replaced in the author line (as part of the header substitution group), they aren't substituted until after the line is parsed.
This ordering can sometimes produce undesirable results.
It's best to use the author line strictly as a shorthand for defining static author and email information.

If you do need to use attribute references in the author or email values, you should xref:author-attribute-entries.adoc[define the attributes explicitly using attribute entries].
****
"#
);
