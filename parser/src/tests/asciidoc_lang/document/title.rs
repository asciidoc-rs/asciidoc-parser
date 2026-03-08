use crate::tests::prelude::*;

track_file!("docs/modules/document/pages/title.adoc");

non_normative!(
    r#"
= Document Title

A document title (aka doctitle) is defined in the document header, typically on the first line of the document.
Like all elements of the document header, the document title is optional.

"#
);

mod title_syntax {
    use crate::{Parser, blocks::SimpleBlockStyle, tests::prelude::*};

    non_normative!(
        r#"
== Title syntax

"#
    );

    #[test]
    fn ex_title() {
        verifies!(
            r#"
A document title is specified using a single equals sign (`=`), followed by a space, then the title text.

.Document with a title
[source#ex-title]
----
include::example$document-title.adoc[]
----

In <<ex-title>>, notice the empty line between the document title and the first line of prose.
That empty line is what separates the document header from the document body.

image::document-title.png[Title of document]

"#
        );

        let doc = Parser::default()
            .parse("= The Intrepid Chronicles\n\nThis adventure begins on a frigid morning.");

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
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "= The Intrepid Chronicles",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "This adventure begins on a frigid morning.",
                            line: 3,
                            col: 1,
                            offset: 27,
                        },
                        rendered: "This adventure begins on a frigid morning.",
                    },
                    source: Span {
                        data: "This adventure begins on a frigid morning.",
                        line: 3,
                        col: 1,
                        offset: 27,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: "= The Intrepid Chronicles\n\nThis adventure begins on a frigid morning.",
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

    // TO DO (https://github.com/asciidoc-rs/asciidoc-parser/issues/380):
    // Add option to support multiple level-0 headings.
    non_normative!(
        r#"
=== Doctypes and titles

Technically, a document title is a level 0 section title (`=`).
The `article` and `manpage` document types (`doctype`) can only have one level 0 section.

The `book` document type permits multiple level 0 section titles.
When the `doctype` is `book`, the title of the level 0 section in the header is used as the document's title.
Subsequent level 0 section titles in the document body are interpreted as xref:sections:parts.adoc[part titles], unless labeled with a xref:sections:styles.adoc[style].

"#
    );
}

// Treating as non-normative because this crate doesn't say anything about
// rendering.
non_normative!(
    r#"
[#hide-or-show]
== Hide or show the document title

When converting a standalone document, the document title is shown by default.
You can control whether the document title appears with the `showtitle` attribute.
If you don't want the title to be shown, unset the `showtitle` attribute using `showtitle!` in the document header or via the CLI or API.

//Need to link to a definition of embeddable doc
When converted to an embeddable document, the document title isn't shown by default.
To show the title in the embeddable document, set `showtitle` in the document header or via the CLI or API.
The author and revision information isn't shown below the document title in the embeddable version of the document like it is in the standalone document, even when `showtitle` is set.

"#
);

mod reference_doctitle {
    use crate::{Parser, blocks::SimpleBlockStyle, tests::prelude::*};

    non_normative!(
        r#"
[#reference-doctitle]
== Reference the document title

"#
    );

    #[test]
    fn ex_doctitle() {
        verifies!(
            r#"
The level 0 section title in a document's header, that is, its title, is automatically assigned to the document attribute `doctitle`.
You can reference the `doctitle` attribute anywhere in your document and the document's title will be displayed.

.Reference the doctitle attribute
[source#ex-doctitle]
----
include::example$doctitle.adoc[]
----

image::doctitle.png[The document title is displayed wherever the doctitle attribute is referenced]

The `doctitle` attribute can also be explicitly set and assigned a value using an attribute entry in the header.
//Its value is identical to the value returned by `Document#doctitle`.

"#
        );

        let doc = Parser::default()
            .parse("= The Intrepid Chronicles\n\n{doctitle} begin on a frigid morning.");

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
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "= The Intrepid Chronicles",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "{doctitle} begin on a frigid morning.",
                            line: 3,
                            col: 1,
                            offset: 27,
                        },
                        rendered: "The Intrepid Chronicles begin on a frigid morning.",
                    },
                    source: Span {
                        data: "{doctitle} begin on a frigid morning.",
                        line: 3,
                        col: 1,
                        offset: 27,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: "= The Intrepid Chronicles\n\n{doctitle} begin on a frigid morning.",
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

non_normative!(
    r#"
[#title-attr]
== title attribute

By default, the text of the document title is used as the value of the HTML `<title>` element and main DocBook `<info>` element.
You can override this behavior by setting the `title` attribute in the header with an attribute entry.
If neither a level 0 section title or `doctitle` is specified in the header, but `title` is, its value is used as a fallback document title.
"#
);
