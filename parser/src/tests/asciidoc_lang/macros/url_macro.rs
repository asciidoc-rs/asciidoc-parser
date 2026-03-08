use crate::tests::prelude::*;

track_file!("docs/modules/macros/pages/url-macro.adoc");

non_normative!(
    r#"
= URL Macro
// The term URL is now preferred over the term URI. See https://en.wikipedia.org/wiki/Uniform_Resource_Identifier#URLs_and_URNs

If you're familiar with the AsciiDoc syntax, you may notice that a URL almost looks like an inline macro.
All that's missing is the pair of the trailing square brackets.
In fact, if you add them, then a URL is treated as an inline macro.
We call this a URL macro.

This page introduces the URL macro, when you would want to use it, and how it differs from the link macro.

"#
);

mod from_url_to_macro {
    use crate::{blocks::SimpleBlockStyle, tests::prelude::*};

    non_normative!(
        r#"
== From URL to macro

"#
    );

    #[test]
    fn example() {
        verifies!(
            r#"
To transform a URL into a macro, add a pair of square brackets to the end of the URL.
For example:

[source]
----
https://asciidoctor.org[]
----

Since no text is specified, this macro behaves the same as an autolink.
In this case, the link automatically gets assigned the "`bare`" role.

When the URL is followed by a pair of square brackets, the URL scheme dually serves as the macro name.
AsciiDoc recognizes all the xref:autolinks.adoc#schemes[URL schemes] for autolinks as macro names (e.g., `https`).
That's why we say "`URL macros`" and not just "`URL macro`".
It's a family of macros.
With the exception of the xref:mailto-macro.adoc[mailto macro], all the URL macros behave the same, and also behave the same as the xref:link-macro.adoc[link macro].

"#
        );

        let doc = Parser::default().parse("https://asciidoctor.org[]");

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
                            data: "https://asciidoctor.org[]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: r#"<a href="https://asciidoctor.org" class="bare">https://asciidoctor.org</a>"#,
                    },
                    source: Span {
                        data: "https://asciidoctor.org[]",
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
                    data: "https://asciidoctor.org[]",
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
    fn inside_quotes() {
        verifies!(
            r#"
So why might you graduate from a URL to a URL macro?
One reason is to force the URL to be parsed when it would not normally be recognized, such as if it's enclosed in double quotes:

[source]
----
Type "https://asciidoctor.org[]" into the location bar of your browser.
----

"#
        );

        let doc = Parser::default()
            .parse(r#"Type "https://asciidoctor.org[]" into the location bar of your browser."#);

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
                            data: r#"Type "https://asciidoctor.org[]" into the location bar of your browser."#,
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: r#"Type "<a href="https://asciidoctor.org" class="bare">https://asciidoctor.org</a>" into the location bar of your browser."#,
                    },
                    source: Span {
                        data: r#"Type "https://asciidoctor.org[]" into the location bar of your browser."#,
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
                    data: r#"Type "https://asciidoctor.org[]" into the location bar of your browser."#,
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
    fn autolink_ended_too_soon() {
        verifies!(
            r#"
Another is when the URL of an autolink is being ended too soon.
The URL macro allows you to explicitly mark the boundary of the URL.

[source]
----
Use http://example.com?menu=<value>[] to open to the menu named `<value>`.
----

A more common reason is to specify custom link text.

"#
        );

        let doc = Parser::default()
            .parse(r#"Use http://example.com?menu=<value>[] to open to the menu named `<value>`."#);

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
                            data: "Use http://example.com?menu=<value>[] to open to the menu named `<value>`.",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "Use <a href=\"http://example.com?menu=&lt;value&gt;\" class=\"bare\">http://example.com?menu=&lt;value&gt;</a> to open to the menu named <code>&lt;value&gt;</code>.",
                    },
                    source: Span {
                        data: "Use http://example.com?menu=<value>[] to open to the menu named `<value>`.",
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
                    data: "Use http://example.com?menu=<value>[] to open to the menu named `<value>`.",
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
}

mod custom_link_text {
    use crate::{blocks::SimpleBlockStyle, tests::prelude::*};

    non_normative!(
        r#"
[#link-text]
== Custom link text

"#
    );

    #[test]
    fn irc_example() {
        verifies!(
            r#"
Instead of displaying the URL, you can configure the link to display custom text.
When the reader clicks on the text, they are directed to the target of the link, the URL.

To customize the text of the link, insert that text between the square brackets of the URL macro.

[source]
----
include::example$url.adoc[tag=irc]
----

"#
        );

        let doc = Parser::default().parse("Chat with other Fedora users in the irc://irc.freenode.org/#fedora[Fedora IRC channel].");

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
                            data: "Chat with other Fedora users in the irc://irc.freenode.org/#fedora[Fedora IRC channel].",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: r#"Chat with other Fedora users in the <a href="irc://irc.freenode.org/#fedora">Fedora IRC channel</a>."#,
                    },
                    source: Span {
                        data: "Chat with other Fedora users in the irc://irc.freenode.org/#fedora[Fedora IRC channel].",
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
                    data: "Chat with other Fedora users in the irc://irc.freenode.org/#fedora[Fedora IRC channel].",
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
    fn add_formatting() {
        verifies!(
            r#"
Since the text is subject to normal substitutions, you can apply formatting to it.

[source]
----
include::example$url.adoc[tag=text]
----

"#
        );

        let doc = Parser::default()
            .parse("Ask questions in the https://chat.asciidoc.org[*community chat*].");

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
                            data: "Ask questions in the https://chat.asciidoc.org[*community chat*].",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: r#"Ask questions in the <a href="https://chat.asciidoc.org"><strong>community chat</strong></a>."#,
                    },
                    source: Span {
                        data: "Ask questions in the https://chat.asciidoc.org[*community chat*].",
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
                    data: "Ask questions in the https://chat.asciidoc.org[*community chat*].",
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

mod link_attributes {
    use crate::{blocks::SimpleBlockStyle, tests::prelude::*};

    non_normative!(
        r#"
== Link attributes

You can use the attribute list to further customize the link, such as to make it target a new window and apply a role to it.

"#
    );

    #[test]
    fn css_example() {
        verifies!(
            r#"
[source]
----
include::example$url.adoc[tag=css]
----

"#
        );

        let doc = Parser::default().parse("Chat with other AsciiDoc users in the https://chat.asciidoc.org[*project chat*^,role=green].");

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
                            data: "Chat with other AsciiDoc users in the https://chat.asciidoc.org[*project chat*^,role=green].",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: r#"Chat with other AsciiDoc users in the <a href="https://chat.asciidoc.org" class="green" target="_blank" rel="noopener"><strong>project chat</strong></a>."#,
                    },
                    source: Span {
                        data: "Chat with other AsciiDoc users in the https://chat.asciidoc.org[*project chat*^,role=green].",
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
                    data: "Chat with other AsciiDoc users in the https://chat.asciidoc.org[*project chat*^,role=green].",
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
To understand how the text between the square brackets of a URL macro is parsed, see xref:link-macro-attribute-parsing.adoc[attribute parsing].
"#
    );
}
