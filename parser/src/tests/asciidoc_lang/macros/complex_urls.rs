use crate::tests::prelude::*;

track_file!("docs/modules/macros/pages/complex-urls.adoc");

non_normative!(
    r#"
= Troubleshooting Complex URLs

A URL may not display correctly when it contains characters such as underscores (`+_+`) or carets (`+^+`).
"#
);

// This source file is tricky because our prototype spec coverage tool isn't
// prepared to follow includes or partials.

#[test]
fn assign_to_attribute() {
    verifies!(
        r#"
include::partial$ts-url-format.adoc[tag=sb]
"#
    );

    let doc = Parser::default().parse("= Document Title\n:link-with-underscores: https://asciidoctor.org/now_this__link_works.html\n\nThis URL has repeating underscores {link-with-underscores}.");

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
                        data: "link-with-underscores",
                        line: 2,
                        col: 2,
                        offset: 18,
                    },
                    value_source: Some(Span {
                        data: "https://asciidoctor.org/now_this__link_works.html",
                        line: 2,
                        col: 25,
                        offset: 41,
                    },),
                    value: InterpretedValue::Value(
                        "https://asciidoctor.org/now_this__link_works.html",
                    ),
                    source: Span {
                        data: ":link-with-underscores: https://asciidoctor.org/now_this__link_works.html",
                        line: 2,
                        col: 1,
                        offset: 17,
                    },
                },],
                author_line: None,
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= Document Title\n:link-with-underscores: https://asciidoctor.org/now_this__link_works.html",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "This URL has repeating underscores {link-with-underscores}.",
                        line: 4,
                        col: 1,
                        offset: 92,
                    },
                    rendered: "This URL has repeating underscores <a href=\"https://asciidoctor.org/now_this__link_works.html\" class=\"bare\">https://asciidoctor.org/now_this__link_works.html</a>.",
                },
                source: Span {
                    data: "This URL has repeating underscores {link-with-underscores}.",
                    line: 4,
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
                data: "= Document Title\n:link-with-underscores: https://asciidoctor.org/now_this__link_works.html\n\nThis URL has repeating underscores {link-with-underscores}.",
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
fn pass_macro() {
    let doc = Parser::default().parse("This URL has repeating underscores pass:macros[https://asciidoctor.org/now_this__link_works.html].");

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
                        data: "This URL has repeating underscores pass:macros[https://asciidoctor.org/now_this__link_works.html].",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "This URL has repeating underscores <a href=\"https://asciidoctor.org/now_this__link_works.html\" class=\"bare\">https://asciidoctor.org/now_this__link_works.html</a>.",
                },
                source: Span {
                    data: "This URL has repeating underscores pass:macros[https://asciidoctor.org/now_this__link_works.html].",
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
                data: "This URL has repeating underscores pass:macros[https://asciidoctor.org/now_this__link_works.html].",
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
fn double_plus_inline_macro() {
    let doc = Parser::default().parse("This URL has repeating underscores link:++https://asciidoctor.org/now_this__link_works.html++[].
");

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
                        data: "This URL has repeating underscores link:++https://asciidoctor.org/now_this__link_works.html++[].",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "This URL has repeating underscores <a href=\"https://asciidoctor.org/now_this__link_works.html\" class=\"bare\">https://asciidoctor.org/now_this__link_works.html</a>.",
                },
                source: Span {
                    data: "This URL has repeating underscores link:++https://asciidoctor.org/now_this__link_works.html++[].",
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
                data: "This URL has repeating underscores link:++https://asciidoctor.org/now_this__link_works.html++[].",
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
