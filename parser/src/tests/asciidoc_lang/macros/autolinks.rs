use crate::tests::prelude::*;

track_file!("docs/modules/macros/pages/autolinks.adoc");

non_normative!(
    r#"
= Autolinks

The AsciiDoc processor will detect common URLs (unless escaped) wherever the macros substitution step is applied and automatically convert them into links.
This page documents the recognized URL schemes and how to disable this behavior on a case-by-case basis.

"#
);

mod url_schemes_for_autolinks {
    use crate::tests::prelude::*;

    non_normative!(
        r#"
== URL schemes for autolinks

AsciiDoc recognizes the following common URL schemes without the help of any markup:

[#schemes]
"#
    );

    #[test]
    fn http() {
        verifies!(
            r#"
* http
"#
        );

        let doc = Parser::default().parse("http://example.org");

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
                            data: "http://example.org",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: r#"<a href="http://example.org" class="bare">http://example.org</a>"#,
                    },
                    source: Span {
                        data: "http://example.org",
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
                    data: "http://example.org",
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
    fn https() {
        verifies!(
            r#"
* https
"#
        );

        let doc = Parser::default().parse("https://example.org");

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
                            data: "https://example.org",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: r#"<a href="https://example.org" class="bare">https://example.org</a>"#,
                    },
                    source: Span {
                        data: "https://example.org",
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
                    data: "https://example.org",
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
    fn ftp() {
        verifies!(
            r#"
* ftp
"#
        );

        let doc = Parser::default().parse("ftp://example.org");

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
                            data: "ftp://example.org",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: r#"<a href="ftp://example.org" class="bare">ftp://example.org</a>"#,
                    },
                    source: Span {
                        data: "ftp://example.org",
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
                    data: "ftp://example.org",
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
    fn irc() {
        verifies!(
            r#"
* irc
"#
        );

        let doc = Parser::default().parse("irc://example.org");

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
                            data: "irc://example.org",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: r#"<a href="irc://example.org" class="bare">irc://example.org</a>"#,
                    },
                    source: Span {
                        data: "irc://example.org",
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
                    data: "irc://example.org",
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
    fn mailto() {
        verifies!(
            r#"
* mailto

"#
        );

        let doc = Parser::default().parse("join@discuss.example.org");

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
                            data: "join@discuss.example.org",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: r#"<a href="mailto:join@discuss.example.org">join@discuss.example.org</a>"#,
                    },
                    source: Span {
                        data: "join@discuss.example.org",
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
                    data: "join@discuss.example.org",
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
    fn https_example() {
        verifies!(
            r#"
The URL in the following example begins with a recognized protocol (i.e., https), so the AsciiDoc processor will automatically turn it into a hyperlink.

[source]
----
include::example$url.adoc[tag=base-co]
----
<.> The trailing period will not get caught up in the link.

The URL is also used as the link text.
If you want to use xref:url-macro.adoc#link-text[custom link text], you must use the xref:url-macro.adoc[URL macro].

"#
        );

        let doc = Parser::default()
            .parse("The homepage for the Asciidoctor Project is https://www.asciidoctor.org.");

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
                            data: "The homepage for the Asciidoctor Project is https://www.asciidoctor.org.",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: r#"The homepage for the Asciidoctor Project is <a href="https://www.asciidoctor.org" class="bare">https://www.asciidoctor.org</a>."#,
                    },
                    source: Span {
                        data: "The homepage for the Asciidoctor Project is https://www.asciidoctor.org.",
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
                    data: "The homepage for the Asciidoctor Project is https://www.asciidoctor.org.",
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
    fn angle_brackets_1() {
        verifies!(
            r#"
In plain text documents, a bare URL is often enclosed in angle brackets.

[source]
----
You'll often see <https://example.org> used in examples.
----

To accommodate this convention, the AsciiDoc processor will still recognize the URL as an autolink, but will discard the angle brackets in the output (as they are not deemed significant).

"#
        );

        let doc =
            Parser::default().parse("You'll often see <https://example.org> used in examples.");

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
                            data: "You'll often see <https://example.org> used in examples.",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: r#"You&#8217;ll often see <a href="https://example.org" class="bare">https://example.org</a> used in examples."#,
                    },
                    source: Span {
                        data: "You'll often see <https://example.org> used in examples.",
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
                    data: "You'll often see <https://example.org> used in examples.",
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
    fn angle_brackets_2() {
        verifies!(
            r#"
The angled brackets are also a convenient way to delinate the boundaries of the URL.
If the bare URL is capturing more characters than it should, you can mark the boundaries explicitly using the angle brackets.

[source]
----
<https://google.com> -- where you find stuff
----

Without the angle brackets delinating the boundaries of the URL, the emdash gets caught up in the URL.
This happens because the replacements substitution is applied before the macros substitution.
So instead of the URL being followed by a space, it's followed directly by the emdash.
The angle brackets ensures it does get included in the URL.

Any link created from a bare URL (i.e., an autolink) automatically gets assigned the "bare" role.
This allows the theming system (e.g., CSS) to recognize autolinks (and other bare URLs) and style them distinctly.

"#
        );

        let doc = Parser::default().parse("<https://google.com> -- where you find stuff");

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
                            data: "<https://google.com> -- where you find stuff",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a href=\"https://google.com\" class=\"bare\">https://google.com</a>&#8201;&#8212;&#8201;where you find stuff",
                    },
                    source: Span {
                        data: "<https://google.com> -- where you find stuff",
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
                    data: "<https://google.com> -- where you find stuff",
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

mod email_autolinks {
    use crate::tests::prelude::*;

    #[test]
    fn example() {
        verifies!(
            r#"
== Email autolinks

AsciiDoc also detects and autolinks most email addresses.

[source]
----
include::example$url.adoc[tag=bare-email]
----

In order for this to work, the domain suffix must be between 2 and 5 characters (e.g., .com) and only common symbols like period (`.`), hyphen (`-`), and plus (`+`) are permitted.
For email address which do not conform to these restriction, you can use the xref:mailto-macro.adoc[email macro].

"#
        );

        let doc = Parser::default().parse("Email us at hello@example.com to say hello.");

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
                            data: "Email us at hello@example.com to say hello.",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: r#"Email us at <a href="mailto:hello@example.com">hello@example.com</a> to say hello."#,
                    },
                    source: Span {
                        data: "Email us at hello@example.com to say hello.",
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
                    data: "Email us at hello@example.com to say hello.",
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

mod escaping_urls_and_email_addresses {
    use crate::tests::prelude::*;

    #[test]
    fn url_and_email_examples() {
        verifies!(
            r#"
== Escaping URLs and email addresses

To prevent automatic linking of a URL or email address, you can add a single backslash (`\`) in front of it.

[source]
----
Once launched, the site will be available at \https://example.org.

If you cannot access the site, email \help@example.org for assistance.
----

The backslash in front of the URL and email address will not appear in the output.
The URL and email address will both be shown in plain text.

"#
        );

        let doc = Parser::default().parse("Once launched, the site will be available at \\https://example.org.\n\nIf you cannot access the site, email \\help@example.org for assistance.");

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
                                data: "Once launched, the site will be available at \\https://example.org.",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: r#"Once launched, the site will be available at https://example.org."#,
                        },
                        source: Span {
                            data: "Once launched, the site will be available at \\https://example.org.",
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
                                data: "If you cannot access the site, email \\help@example.org for assistance.",
                                line: 3,
                                col: 1,
                                offset: 68,
                            },
                            rendered: r#"If you cannot access the site, email help@example.org for assistance."#,
                        },
                        source: Span {
                            data: "If you cannot access the site, email \\help@example.org for assistance.",
                            line: 3,
                            col: 1,
                            offset: 68,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },)
                ],
                source: Span {
                    data: "Once launched, the site will be available at \\https://example.org.\n\nIf you cannot access the site, email \\help@example.org for assistance.",
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
    fn disable_via_incremental_subs() {
        verifies!(
            r#"
Since autolinks are a feature of the xref:subs:macros.adoc[macros substitution], another way to prevent automatic linking of a URL or email address is to turn off the macros substitution using xref:subs:apply-subs-to-blocks.adoc#incremental[incremental subs].

[source]
----
[subs=-macros]
Once launched, the site will be available at https://example.org.
----

The `subs` attribute is only recognized on a leaf block, such as a paragraph.
"#
        );

        let doc = Parser::default().parse(
            "[subs=-macros]\nOnce launched, the site will be available at https://example.org.",
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
                            data: "Once launched, the site will be available at https://example.org.",
                            line: 2,
                            col: 1,
                            offset: 15,
                        },
                        rendered: r#"Once launched, the site will be available at https://example.org."#,
                    },
                    source: Span {
                        data: "[subs=-macros]\nOnce launched, the site will be available at https://example.org.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: Some("subs",),
                            value: "-macros",
                            shorthand_items: &[],
                        },],
                        anchor: None,
                        source: Span {
                            data: "subs=-macros",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[subs=-macros]\nOnce launched, the site will be available at https://example.org.",
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
