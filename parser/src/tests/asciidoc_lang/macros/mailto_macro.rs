use crate::tests::prelude::*;

track_file!("docs/modules/macros/pages/mailto-macro.adoc");

non_normative!(
    r#"
= Mailto Macro
:page-aliases: email-macro.adoc

The mailto macro is a specialization of the xref:url-macro.adoc[URL macro] that adds support for defining an email link with text and augmenting it with additional metadata, such as a subject and body.

"#
);

mod link_text_and_named_attributes {
    use crate::{Parser, blocks::SimpleBlockStyle, tests::prelude::*};

    non_normative!(
        r#"
== Link text and named attributes

"#
    );

    #[test]
    fn whole_text() {
        verifies!(
            r#"
Using an attribute list, you can specify link text as well as named attributes such as `id` and `role`.
Unlike other URL macros, you must add the `mailto:` prefix in front of the email address in order to append an attribute list.

Here's an example of an email link with explicit link text.

----
mailto:join@discuss.example.org[Subscribe]
----

"#
        );

        let doc = Parser::default().parse("mailto:join@discuss.example.org[Subscribe]");

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
                            data: "mailto:join@discuss.example.org[Subscribe]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a href=\"mailto:join@discuss.example.org\">Subscribe</a>",
                    },
                    source: Span {
                        data: "mailto:join@discuss.example.org[Subscribe]",
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
                    data: "mailto:join@discuss.example.org[Subscribe]",
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
    fn add_role() {
        verifies!(
            r#"
If you want to add a role to this link, you can do so by appending the `role` attribute after a comma.

----
mailto:join@discuss.example.org[Subscribe,role=email]
----

"#
        );

        let doc = Parser::default().parse("mailto:join@discuss.example.org[Subscribe,role=email]");

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
                            data: "mailto:join@discuss.example.org[Subscribe,role=email]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a href=\"mailto:join@discuss.example.org\" class=\"email\">Subscribe</a>",
                    },
                    source: Span {
                        data: "mailto:join@discuss.example.org[Subscribe,role=email]",
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
                    data: "mailto:join@discuss.example.org[Subscribe,role=email]",
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
    fn link_text_contains_comma() {
        verifies!(
            r#"
If the link text contains a comma, you must enclose the text in double quotes.
Otherwise, the portion of the text that follows the comma will be interpreted as additional attribute list entries.

----
mailto:join@discuss.example.org["Click, subscribe, and participate!"]
----

"#
        );

        let doc = Parser::default()
            .parse("mailto:join@discuss.example.org[\"Click, subscribe, and participate!\"]");

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
                            data: "mailto:join@discuss.example.org[\"Click, subscribe, and participate!\"]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a href=\"mailto:join@discuss.example.org\">Click, subscribe, and participate!</a>",
                    },
                    source: Span {
                        data: "mailto:join@discuss.example.org[\"Click, subscribe, and participate!\"]",
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
                    data: "mailto:join@discuss.example.org[\"Click, subscribe, and participate!\"]",
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
To learn more about how the attributes are parsed, refer to xref:link-macro-attribute-parsing.adoc[attribute parsing].

"#
    );
}

mod subject_and_body {
    use crate::{Parser, blocks::SimpleBlockStyle, tests::prelude::*};

    non_normative!(
        r#"
== Subject and body

Like with other URL macros, the first positional attribute of the email macro is the link text.
If a comma is present in the text, and the text is not enclosed in quotes, or the comma comes after the closing quote, the next positional attribute is treated as the subject line.

"#
    );

    #[test]
    fn second_arg_is_subject() {
        verifies!(
            r#"
For example, you can configure the email link to populate the subject line when the reader clicks the link as follows:

----
mailto:join@discuss.example.org[Subscribe,Subscribe me]
----

When the reader clicks the link, a conforming email client will fill in the subject line with "`Subscribe me`".

"#
        );

        let doc =
            Parser::default().parse("mailto:join@discuss.example.org[Subscribe,Subscribe me]");

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
                            data: "mailto:join@discuss.example.org[Subscribe,Subscribe me]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a href=\"mailto:join@discuss.example.org?subject=Subscribe%20me\">Subscribe</a>",
                    },
                    source: Span {
                        data: "mailto:join@discuss.example.org[Subscribe,Subscribe me]",
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
                    data: "mailto:join@discuss.example.org[Subscribe,Subscribe me]",
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
    fn third_arg_is_body() {
        verifies!(
            r#"
If you want the body of the email to also be populated, specify the body text in the third positional argument.

----
mailto:join@discuss.example.org[Subscribe,Subscribe me,I want to participate.]
----

When the reader clicks the link, a conforming email client will fill in the body with "`I want to participate.`"

"#
        );

        let doc = Parser::default().parse(
            "mailto:join@discuss.example.org[Subscribe,Subscribe me,I want to participate.]",
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
                            data: "mailto:join@discuss.example.org[Subscribe,Subscribe me,I want to participate.]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a href=\"mailto:join@discuss.example.org?subject=Subscribe%20me&amp;body=I%20want%20to%20participate.\">Subscribe</a>",
                    },
                    source: Span {
                        data: "mailto:join@discuss.example.org[Subscribe,Subscribe me,I want to participate.]",
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
                    data: "mailto:join@discuss.example.org[Subscribe,Subscribe me,I want to participate.]",
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
    fn reuse_email_address_as_link_text() {
        verifies!(
            r#"
If you want to reuse the email address as the link text, leave the first positional attribute empty.

----
mailto:join@discuss.example.org[,Subscribe me,I want to participate.]
----

"#
        );

        let doc = Parser::default()
            .parse("mailto:join@discuss.example.org[,Subscribe me,I want to participate.]");

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
                            data: "mailto:join@discuss.example.org[,Subscribe me,I want to participate.]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a href=\"mailto:join@discuss.example.org?subject=Subscribe%20me&amp;body=I%20want%20to%20participate.\">join@discuss.example.org</a>",
                    },
                    source: Span {
                        data: "mailto:join@discuss.example.org[,Subscribe me,I want to participate.]",
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
                    data: "mailto:join@discuss.example.org[,Subscribe me,I want to participate.]",
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
    fn only_subject() {
        verifies!(
            r#"
If you only want to specify a subject, leave off the body.

----
mailto:join@discuss.example.org[,Subscribe me]
----

"#
        );

        let doc = Parser::default().parse("mailto:join@discuss.example.org[,Subscribe me]");

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
                            data: "mailto:join@discuss.example.org[,Subscribe me]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a href=\"mailto:join@discuss.example.org?subject=Subscribe%20me\">join@discuss.example.org</a>",
                    },
                    source: Span {
                        data: "mailto:join@discuss.example.org[,Subscribe me]",
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
                    data: "mailto:join@discuss.example.org[,Subscribe me]",
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
    fn body_enclosed_in_quotes() {
        verifies!(
            r#"
If either the subject or body contains a comma, that value must be enclosed in double quotes.

----
mailto:join@discuss.example.org[Subscribe,"I want to participate, so please subscribe me"]
----

"#
        );

        let doc = Parser::default().parse("mailto:join@discuss.example.org[Subscribe,\"I want to participate, so please subscribe me\"]");

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
                            data: "mailto:join@discuss.example.org[Subscribe,\"I want to participate, so please subscribe me\"]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a href=\"mailto:join@discuss.example.org?subject=I%20want%20to%20participate%2C%20so%20please%20subscribe%20me\">Subscribe</a>",
                    },
                    source: Span {
                        data: "mailto:join@discuss.example.org[Subscribe,\"I want to participate, so please subscribe me\"]",
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
                    data: "mailto:join@discuss.example.org[Subscribe,\"I want to participate, so please subscribe me\"]",
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
To learn more about how the attributes are parsed, refer to xref:link-macro-attribute-parsing.adoc[attribute parsing].
"#
    );
}
