use crate::tests::prelude::*;

track_file!("docs/modules/macros/pages/link-macro-attribute-parsing.adoc");

non_normative!(
    r#"
= Link & URL Macro Attribute Parsing

If named attributes are detected between the square brackets of a link or URL macro, that text is parsed as an attribute list.
This page explains the conditions when this occurs and how to write the link text so it is recognized as a single positional attribute.

"#
);

mod link_text_alongside_named_attributes {
    use crate::{Parser, blocks::SimpleBlockStyle, tests::prelude::*};

    non_normative!(
        r#"
== Link text alongside named attributes

"#
    );

    #[test]
    fn whole_text() {
        verifies!(
            r#"
Normally, the whole text between the square brackets of a link macro is treated as the link text (i.e., the first positional attribute).

[source]
----
https://chat.asciidoc.org[Discuss AsciiDoc]
----

"#
        );

        let doc = Parser::default().parse("https://chat.asciidoc.org[Discuss AsciiDoc]");

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
                            data: "https://chat.asciidoc.org[Discuss AsciiDoc]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a href=\"https://chat.asciidoc.org\">Discuss AsciiDoc</a>",
                    },
                    source: Span {
                        data: "https://chat.asciidoc.org[Discuss AsciiDoc]",
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
                    data: "https://chat.asciidoc.org[Discuss AsciiDoc]",
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
    fn attrlist_with_role_and_target() {
        verifies!(
            r#"
However, if the text contains an equals sign (`=`), the text is parsed as an xref:attributes:element-attributes.adoc#attribute-list[attribute list].
The exact rules for attribute list parsing and positional attributes are rather complex, and discussed on xref:attributes:positional-and-named-attributes.adoc[].
To be sure the link text is recognized properly, you can apply these two simple checks:

* contains no comma (`,`) or equals sign (`=`) or
* enclosed in double quotes (`"`)

There are several other situations in which text before the first comma may be recognized as the link text.
Let's consider some examples.

The following example shows a URL macro with custom link text alongside named attributes.

[source]
----
https://chat.asciidoc.org[Discuss AsciiDoc,role=resource,window=_blank]
----

"#
        );

        let doc = Parser::default().parse(
            "https://chat.asciidoc.org[Discuss AsciiDoc,role=resource,window=_blank]
",
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
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "https://chat.asciidoc.org[Discuss AsciiDoc,role=resource,window=_blank]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a href=\"https://chat.asciidoc.org\" class=\"resource\" target=\"_blank\" rel=\"noopener\">Discuss AsciiDoc</a>",
                    },
                    source: Span {
                        data: "https://chat.asciidoc.org[Discuss AsciiDoc,role=resource,window=_blank]",
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
                    data: "https://chat.asciidoc.org[Discuss AsciiDoc,role=resource,window=_blank]",
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
    fn attrlist_with_quoted_title() {
        verifies!(
            r#"
Let's consider a case where the link text contains a comma and the macro also has named attributes.
In this case, you must enclose the link text in double quotes so that it is capture in its entirety as the first positional attribute.

[source]
----
https://example.org["Google, DuckDuckGo, Ecosia",role=teal]
----

"#
        );

        let doc = Parser::default()
            .parse("https://example.org[\"Google, DuckDuckGo, Ecosia\",role=teal]");

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
                            data: "https://example.org[\"Google, DuckDuckGo, Ecosia\",role=teal]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a href=\"https://example.org\" class=\"teal\">Google, DuckDuckGo, Ecosia</a>",
                    },
                    source: Span {
                        data: "https://example.org[\"Google, DuckDuckGo, Ecosia\",role=teal]",
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
                    data: "https://example.org[\"Google, DuckDuckGo, Ecosia\",role=teal]",
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
    fn quoted_title_only() {
        verifies!(
            r#"
Similarly, if the link text contains an equals sign, you can enclose the link text in double quotes to ensure the parser recognizes it as the first positional attribute.

[source]
----
https://example.org["1=2 posits the problem of inequality"]
----

"#
        );

        let doc = Parser::default()
            .parse("https://example.org[\"1=2 posits the problem of inequality\"]");

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
                            data: "https://example.org[\"1=2 posits the problem of inequality\"]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a href=\"https://example.org\">1=2 posits the problem of inequality</a>",
                    },
                    source: Span {
                        data: "https://example.org[\"1=2 posits the problem of inequality\"]",
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
                    data: "https://example.org[\"1=2 posits the problem of inequality\"]",
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
    fn quoted_title_with_escaped_quote() {
        verifies!(
            r##"
If the quoted link text itself contains the quote character used to enclose the text, escape the quote character in the text by prefixing it with a backslash.

[source]
----
https://example.org["href=\"#top\" attribute"] creates link to top of page
----

"##
        );

        let doc = Parser::default().parse(
            "https://example.org[\"href=\\\"#top\\\" attribute\"] creates link to top of page",
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
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "https://example.org[\"href=\\\"#top\\\" attribute\"] creates link to top of page",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a href=\"https://example.org\">href=\"#top\" attribute</a> creates link to top of page",
                    },
                    source: Span {
                        data: "https://example.org[\"href=\\\"#top\\\" attribute\"] creates link to top of page",
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
                    data: "https://example.org[\"href=\\\"#top\\\" attribute\"] creates link to top of page",
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
The double quote enclosure is not required in all cases when the link text contains an equals sign.
Strictly speaking, the enclosure is only required when the text preceding the equals sign matches a valid attribute name.
However, it's best to use the double quotes just to be safe.

"#
    );

    #[test]
    fn named_attributes_without_link_text() {
        verifies!(
            r#"
Finally, to use named attributes without specifying link text, you simply specify the named attributes.
(In other words, you leave the first positional attribute empty, in which case the target will be used as the link text).

[source]
----
https://chat.asciidoc.org[role=button,window=_blank,opts=nofollow]
----

"#
        );

        let doc = Parser::default()
            .parse("https://chat.asciidoc.org[role=button,window=_blank,opts=nofollow]");

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
                            data: "https://chat.asciidoc.org[role=button,window=_blank,opts=nofollow]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a href=\"https://chat.asciidoc.org\" class=\"bare button\" target=\"_blank\" rel=\"nofollow\" noopener>https://chat.asciidoc.org</a>",
                    },
                    source: Span {
                        data: "https://chat.asciidoc.org[role=button,window=_blank,opts=nofollow]",
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
                    data: "https://chat.asciidoc.org[role=button,window=_blank,opts=nofollow]",
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
The link macro recognizes all the common attributes (id, role, and opts).
It also recognizes a handful of attributes that are specific to the link macro.

"#
    );
}

mod target_separate_window {
    use crate::{Parser, blocks::SimpleBlockStyle, tests::prelude::*};

    non_normative!(
        r#"
== Target a separate window

By default, the link produced by a link macro will target the current window.
In other words, clicking on it will replace the current page.

"#
    );

    #[test]
    fn window_attribute() {
        verifies!(
            r#"
You can configure the link to open in a separate window (or tab) using the `window` attribute.

[source]
----
https://asciidoctor.org[Asciidoctor,window=read-later]
----

In the HTML output, the value of the `window` attribute is assigned to the `target` attribute on the `<a>` tag (e.g., `target=read-later`).

"#
        );

        let doc = Parser::default().parse("https://asciidoctor.org[Asciidoctor,window=read-later]");

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
                            data: "https://asciidoctor.org[Asciidoctor,window=read-later]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a href=\"https://asciidoctor.org\" target=\"read-later\">Asciidoctor</a>",
                    },
                    source: Span {
                        data: "https://asciidoctor.org[Asciidoctor,window=read-later]",
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
                    data: "https://asciidoctor.org[Asciidoctor,window=read-later]",
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
    fn target_blank_window() {
        verifies!(
            r#"
=== Target a blank window

Most of the time, you'll use the `window` attribute to target a blank window.
Configuring a link that points to a location outside the current site is common practice to avoid disrupting the reader's flow.
To enable this behavior, you set the `window` attribute to the special value `_blank`.

[source]
----
https://asciidoctor.org[Asciidoctor,window=_blank]
----

In the HTML output, the value of the `window` attribute is assigned to the `target` attribute on the `<a>` tag (e.g., `target=_blank`).
If the target is `_blank`, the processor will automatically add the <<noopener and nofollow,`rel=noopener` attribute>> as well.

"#
        );

        let doc = Parser::default().parse("https://asciidoctor.org[Asciidoctor,window=_blank]");

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
                            data: "https://asciidoctor.org[Asciidoctor,window=_blank]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a href=\"https://asciidoctor.org\" target=\"_blank\" rel=\"noopener\">Asciidoctor</a>",
                    },
                    source: Span {
                        data: "https://asciidoctor.org[Asciidoctor,window=_blank]",
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
                    data: "https://asciidoctor.org[Asciidoctor,window=_blank]",
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
CAUTION: The underscore at the start of the value `_blank` can unexpectedly form a constrained formatting pair when another underscore appears somewhere else in the line or paragraph, thus causing the macro to break.
You can avoid this problem either by escaping the underscore at the start of the value (i.e., `+window=\_blank+`) or by using the <<Blank window shorthand>> instead.

"#
    );

    #[test]
    fn implicit_noopener_for_blank() {
        verifies!(
            r#"
=== noopener and nofollow

The `noopener` option is used to control access to the window opened by a link.
*This option is only available if the `window` attribute is set.*
This option adds the `noopener` flag to the `rel` attribute on the `<a>` element in the HTML output (e.g., `rel="noopener"`).

When the value of the `window` attribute is `_blank`, the AsciiDoc processor implicitly sets the `noopener` option.
Doing so is considered a security best practice.

[source]
----
https://asciidoctor.org[Asciidoctor,window=_blank]
----

"#
        );

        let doc = Parser::default().parse("https://asciidoctor.org[Asciidoctor,window=_blank]");

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
                            data: "https://asciidoctor.org[Asciidoctor,window=_blank]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a href=\"https://asciidoctor.org\" target=\"_blank\" rel=\"noopener\">Asciidoctor</a>",
                    },
                    source: Span {
                        data: "https://asciidoctor.org[Asciidoctor,window=_blank]",
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
                    data: "https://asciidoctor.org[Asciidoctor,window=_blank]",
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
    fn explicit_noopener_for_other_windows() {
        verifies!(
            r#"
If the window is not `_blank`, you need to enable the `noopener` flag explicitly by setting the `noopener` option on the macro:

[source]
----
https://asciidoctor.org[Asciidoctor,window=read-later,opts=noopener]
----

"#
        );

        let doc = Parser::default()
            .parse("https://asciidoctor.org[Asciidoctor,window=read-later,opts=noopener]");

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
                            data: "https://asciidoctor.org[Asciidoctor,window=read-later,opts=noopener]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a href=\"https://asciidoctor.org\" target=\"read-later\" rel=\"noopener\">Asciidoctor</a>",
                    },
                    source: Span {
                        data: "https://asciidoctor.org[Asciidoctor,window=read-later,opts=noopener]",
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
                    data: "https://asciidoctor.org[Asciidoctor,window=read-later,opts=noopener]",
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
    fn nofollow_option() {
        verifies!(
            r#"
If you don't want the search indexer to follow the link, you can add the `nofollow` option to the macro.
This option adds the `nofollow` flag to the `rel` attribute on the `<a>` element in the HTML output, alongside `noopener` if present (e.g., `rel="nofollow noopener"`).

[source]
----
https://asciidoctor.org[Asciidoctor,window=_blank,opts=nofollow]
----

"#
        );

        let doc = Parser::default()
            .parse("https://asciidoctor.org[Asciidoctor,window=_blank,opts=nofollow]");

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
                            data: "https://asciidoctor.org[Asciidoctor,window=_blank,opts=nofollow]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a href=\"https://asciidoctor.org\" target=\"_blank\" rel=\"nofollow\" noopener>Asciidoctor</a>",
                    },
                    source: Span {
                        data: "https://asciidoctor.org[Asciidoctor,window=_blank,opts=nofollow]",
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
                    data: "https://asciidoctor.org[Asciidoctor,window=_blank,opts=nofollow]",
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
    fn nofollow_and_noopener_options() {
        verifies!(
            r#"
or

[source]
----
https://asciidoctor.org[Asciidoctor,window=read-later,opts="noopener,nofollow"]
----

"#
        );

        let doc = Parser::default().parse(
            "https://asciidoctor.org[Asciidoctor,window=read-later,opts=\"noopener,nofollow\"]",
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
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "https://asciidoctor.org[Asciidoctor,window=read-later,opts=\"noopener,nofollow\"]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a href=\"https://asciidoctor.org\" target=\"read-later\" rel=\"nofollow\" noopener>Asciidoctor</a>",
                    },
                    source: Span {
                        data: "https://asciidoctor.org[Asciidoctor,window=read-later,opts=\"noopener,nofollow\"]",
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
                    data: "https://asciidoctor.org[Asciidoctor,window=read-later,opts=\"noopener,nofollow\"]",
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
    fn nofollow_without_window_target() {
        verifies!(
            r#"
To fine tune indexing within the site, you can specify the `nofollow` option even if the link does not target a separate window.

[source]
----
link:post.html[My Post,opts=nofollow]
----

"#
        );

        let doc = Parser::default().parse("link:post.html[My Post,opts=nofollow]");

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
                            data: "link:post.html[My Post,opts=nofollow]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a href=\"post.html\" rel=\"nofollow\">My Post</a>",
                    },
                    source: Span {
                        data: "link:post.html[My Post,opts=nofollow]",
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
                    data: "link:post.html[My Post,opts=nofollow]",
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
    fn blank_window_shorthand() {
        verifies!(
            r#"
=== Blank window shorthand

Configuring an external link to target a blank window is a common practice.
Therefore, AsciiDoc provides a shorthand for it.

In place of the named attribute `+window=_blank+`, you can insert a caret (`+^+`) at the end of the link text.
This syntax has the added benefit of not having to worry about the underscore at the start of the value `+_blank+` unexpectedly forming a constrained formatting pair when another underscore appears in the same line or paragraph.

[source]
----
include::example$url.adoc[tag=linkattrs-s]
----

"#
        );

        let doc = Parser::default().parse("Let's view the raw HTML of the link:view-source:asciidoctor.org[Asciidoctor homepage^].");

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
                            data: "Let's view the raw HTML of the link:view-source:asciidoctor.org[Asciidoctor homepage^].",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "Let&#8217;s view the raw HTML of the <a href=\"view-source:asciidoctor.org\" target=\"_blank\" rel=\"noopener\">Asciidoctor homepage</a>.",
                    },
                    source: Span {
                        data: "Let's view the raw HTML of the link:view-source:asciidoctor.org[Asciidoctor homepage^].",
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
                    data: "Let's view the raw HTML of the link:view-source:asciidoctor.org[Asciidoctor homepage^].",
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
CAUTION: In rare circumstances, if you use the caret syntax more than once in the same line or paragraph, you may need to escape the first occurrence with a backslash.
However, the processor should try to avoid making this a requirement.

"#
    );

    #[test]
    fn blank_window_shorthand_with_role() {
        verifies!(
            r#"
If the attribute list has both link text in double quotes and named attributes, the caret should be placed at the end of the link text, but inside the double quotes.

[source]
----
https://example.org["Google, DuckDuckGo, Ecosia^",role=btn]
----

"#
        );

        let doc = Parser::default()
            .parse("https://example.org[\"Google, DuckDuckGo, Ecosia^\",role=btn]");

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
                            data: "https://example.org[\"Google, DuckDuckGo, Ecosia^\",role=btn]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a href=\"https://example.org\" class=\"btn\" target=\"_blank\" rel=\"noopener\">Google, DuckDuckGo, Ecosia</a>",
                    },
                    source: Span {
                        data: "https://example.org[\"Google, DuckDuckGo, Ecosia^\",role=btn]",
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
                    data: "https://example.org[\"Google, DuckDuckGo, Ecosia^\",role=btn]",
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
    fn no_named_attributes() {
        verifies!(
            r#"
If no named attributes are present, the link text should not be enclosed in quotes.

[source]
----
https://example.org[Google, DuckDuckGo, Ecosia^]
----
"#
        );

        let doc = Parser::default().parse("https://example.org[Google, DuckDuckGo, Ecosia^]");

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
                            data: "https://example.org[Google, DuckDuckGo, Ecosia^]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a href=\"https://example.org\" target=\"_blank\" rel=\"noopener\">Google, DuckDuckGo, Ecosia</a>",
                    },
                    source: Span {
                        data: "https://example.org[Google, DuckDuckGo, Ecosia^]",
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
                    data: "https://example.org[Google, DuckDuckGo, Ecosia^]",
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
