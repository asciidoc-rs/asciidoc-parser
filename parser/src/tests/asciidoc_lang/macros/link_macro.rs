use crate::tests::prelude::*;

track_file!("docs/modules/macros/pages/link-macro.adoc");

non_normative!(
    r#"
= Link Macro

The link macro is the most explicit method of making a link in AsciiDoc.
It's only necessary when the behavior of autolinks and URL macros proves insufficient.
This page covers the anatomy of the link macro, when it's required, and how to use it.

"#
);

mod anatomy {
    use crate::{blocks::SimpleBlockStyle, tests::prelude::*};

    non_normative!(
        r#"
== Anatomy

"#
    );

    #[test]
    fn example() {
        verifies!(
            r#"
To transform The link macro is an inline macro.
Like other inline macros, its syntax follows the familiar pattern of the macro name and target separated by a colon followed by an attribute list enclosed in square brackets.

[source]
----
link:<target>[<attrlist>]
----

The `<target>` becomes the target of the link.
the `<attrlist>` is the link text unless a named attribute is detected.
"#
        );

        let doc = Parser::default().parse("link:target[link text]");

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
                            data: "link:target[link text]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: r#"<a href="target">link text</a>"#,
                    },
                    source: Span {
                        data: "link:target[link text]",
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
                    data: "link:target[link text]",
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
See xref:link-macro-attribute-parsing.adoc[link macro attribute list] to learn how the `<attrlist>` is parsed.

Like all inline macros, the link macro can be escaped using a leading backslash (`\`).

"#
    );
}

mod link_to_relative_file {
    use crate::{blocks::SimpleBlockStyle, tests::prelude::*};

    non_normative!(
        r#"
== Link to a relative file

If you want to link to a non-AsciiDoc file that is relative to the current document, use the `link` macro in front of the file name.

TIP: To link to a relative AsciiDoc file, use the xref:inter-document-xref.adoc[xref macro] instead.

"#
    );

    #[test]
    fn example() {
        verifies!(
            r#"
Here's an example that demonstrates how to use the link macro to link to a relative file path:

[source]
----
include::example$url.adoc[tag=link]
----

The AsciiDoc processor will create a link to _report.pdf_ with the text "Get Report", even though the target is not a URL.

"#
        );

        let doc = Parser::default().parse("link:downloads/report.pdf[Get Report]");

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
                            data: "link:downloads/report.pdf[Get Report]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: r#"<a href="downloads/report.pdf">Get Report</a>"#,
                    },
                    source: Span {
                        data: "link:downloads/report.pdf[Get Report]",
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
                    data: "link:downloads/report.pdf[Get Report]",
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
    fn hash_example() {
        verifies!(
            r#"
If the target file is an HTML file, and you want to link directly to an anchor within that document, append a hash (`#`) followed by the name of the anchor after the file name:

[source]
----
include::example$url.adoc[tag=hash]
----

Note that when linking to a relative file, even if it's an HTML file, the link text is required.

"#
        );

        let doc = Parser::default().parse("link:tools.html#editors[]");

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
                            data: "link:tools.html#editors[]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: r#"<a href="tools.html#editors" class="bare">tools.html#editors</a>"#,
                    },
                    source: Span {
                        data: "link:tools.html#editors[]",
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
                    data: "link:tools.html#editors[]",
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

mod when_to_use {
    use crate::{blocks::SimpleBlockStyle, tests::prelude::*};

    non_normative!(
        r#"
// FIXME: this feels like it needs subsections
== When to use the link macro

Since AsciiDoc provides autolinks and URL macros, the link macro is not often needed.
Here are the few cases when the link macro is necessary:

* The target is not a URL (e.g., a relative path)
* The target must be enclosed in a passthrough to escape characters with special meaning
* The URL macro is not bounded by spaces, brackets, or quotes.
* The target is a URL that does not start with a scheme recognized by AsciiDoc

"#
    );

    #[test]
    fn example() {
        verifies!(
            r#"
The most common situation is when the target is not a URL.
For example, you would use the link macro to create a link to a relative path.

[source]
----
link:report.pdf[Get Report]
----

"#
        );

        let doc = Parser::default().parse("link:report.pdf[Get Report]");

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
                            data: "link:report.pdf[Get Report]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: r#"<a href="report.pdf">Get Report</a>"#,
                    },
                    source: Span {
                        data: "link:report.pdf[Get Report]",
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
                    data: "link:report.pdf[Get Report]",
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
TIP: If the relative path is another AsciiDoc file, you should use the xref:inter-document-xref.adoc[xref macro] instead.

"#
    );

    #[test]
    fn escape_using_passthrough() {
        verifies!(
            r#"
You may also discover that spaces are not permitted in the target of the link macro, at least not in the AsciiDoc source.
The space character in the target prevents the parser from recognizing the macro.
So it's necessary to escape or encode it.
Here are three ways to do it:

.Escape a space using a passthrough
[source]
----
link:pass:[My Documents/report.pdf][Get Report]
----

"#
        );

        let doc = Parser::default().parse("link:pass:[My Documents/report.pdf][Get Report]");

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
                            data: "link:pass:[My Documents/report.pdf][Get Report]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: r#"<a href="My Documents/report.pdf">Get Report</a>"#,
                    },
                    source: Span {
                        data: "link:pass:[My Documents/report.pdf][Get Report]",
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
                    data: "link:pass:[My Documents/report.pdf][Get Report]",
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
    fn use_character_reference() {
        verifies!(
            r#"
.Encode a space using a character reference (\&#32;)
[source]
----
link:My&#32;Documents/report.pdf[Get Report]
----

"#
        );

        let doc = Parser::default().parse("link:My&#32;Documents/report.pdf[Get Report]");

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
                            data: "link:My&#32;Documents/report.pdf[Get Report]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: r#"<a href="My&#32;Documents/report.pdf">Get Report</a>"#,
                    },
                    source: Span {
                        data: "link:My&#32;Documents/report.pdf[Get Report]",
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
                    data: "link:My&#32;Documents/report.pdf[Get Report]",
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
    fn use_url_encoding() {
        verifies!(
            r#"
.Encode a space using URL encoding (%20)
[source]
----
link:My%20Documents/report.pdf[Get Report]
----

"#
        );

        let doc = Parser::default().parse("link:My%20Documents/report.pdf[Get Report]");

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
                            data: "link:My%20Documents/report.pdf[Get Report]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: r#"<a href="My%20Documents/report.pdf">Get Report</a>"#,
                    },
                    source: Span {
                        data: "link:My%20Documents/report.pdf[Get Report]",
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
                    data: "link:My%20Documents/report.pdf[Get Report]",
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
Escaping or encoding the space ensures that the space does not prevent the link macro from being recognized.
The downside of using URL encoding is that it will be visible in the automatic link text since the browser does not decode it in that location.
In this case, the character reference is preferable.

"#
    );

    #[test]
    fn url_encode_colon() {
        verifies!(
            r#"
There are other characters that are not permitted in a link target as well, such as a colon.
You can escape those using the same technique.

.Encode a colon using URL encoding (%3A)
[source]
----
link:Avengers%3A%20Endgame.html[]
----

"#
        );

        let doc = Parser::default().parse("link:Avengers%3A%20Endgame.html[]");

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
                            data: "link:Avengers%3A%20Endgame.html[]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: r#"<a href="Avengers%3A%20Endgame.html" class="bare">Avengers%3A%20Endgame.html</a>"#,
                    },
                    source: Span {
                        data: "link:Avengers%3A%20Endgame.html[]",
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
                    data: "link:Avengers%3A%20Endgame.html[]",
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
    fn passthrough_escape_underscores() {
        verifies!(
            r#"
Another common case is when you need to use a passthrough to escape characters with special meaning.
In this case, the AsciiDoc processor will not recognize the target as a URL, and thus the link macro is necessary.
An example is when the URL contains repeating underscore characters.

[source]
----
link:++https://example.org/now_this__link_works.html++[]
----

"#
        );

        let doc =
            Parser::default().parse("link:++https://example.org/now_this__link_works.html++[]");

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
                            data: "link:++https://example.org/now_this__link_works.html++[]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: r#"<a href="https://example.org/now_this__link_works.html" class="bare">https://example.org/now_this__link_works.html</a>"#,
                    },
                    source: Span {
                        data: "link:++https://example.org/now_this__link_works.html++[]",
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
                    data: "link:++https://example.org/now_this__link_works.html++[]",
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
    fn not_bounded_by_word_break() {
        verifies!(
            r#"
A similar situation is when the URL macro is not bounded by spaces, brackets, or quotes.
In this case, the link macro prefix is required to increase the precedence so that the macro can be recognized.

[source]
----
|link:https://asciidoctor.org[]|
----

"#
        );

        let doc = Parser::default().parse("|link:https://asciidoctor.org[]|");

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
                            data: "|link:https://asciidoctor.org[]|",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: r#"|<a href="https://asciidoctor.org" class="bare">https://asciidoctor.org</a>|"#,
                    },
                    source: Span {
                        data: "|link:https://asciidoctor.org[]|",
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
                    data: "|link:https://asciidoctor.org[]|",
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
    fn unrecognized_ui_scheme() {
        verifies!(
            r#"
Finally, if the target is not recognized as a URL by AsciiDoc, the link macro is necessary.
For example, you might use the link macro to make a file link.

[source]
----
link:file:///home/username[Your files]
----

"#
        );

        let doc = Parser::default().parse("link:file:///home/username[Your files]");

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
                            data: "link:file:///home/username[Your files]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: r#"<a href="file:///home/username">Your files</a>"#,
                    },
                    source: Span {
                        data: "link:file:///home/username[Your files]",
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
                    data: "link:file:///home/username[Your files]",
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
== Final word

The general rule of thumb is that you should only put the `link:` macro prefix in front of the target if the target is _not_ a URL.
Otherwise, the prefix just adds verbosity.
"#
);
