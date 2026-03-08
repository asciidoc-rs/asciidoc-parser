use crate::{Parser, blocks::SimpleBlockStyle, tests::prelude::*};

track_file!("docs/modules/attributes/pages/reference-attributes.adoc");

non_normative!(
    r#"
= Reference Document Attributes
:navtitle: Reference Attributes
:disclaimer: Don't pet the wild Wolpertingers. We're not responsible for any loss \
of hair, chocolate, or purple socks.
:url-repo: https://github.com/asciidoctor/asciidoctor

You'll likely want to insert the value of a user-defined or built-in document attribute in various locations throughout a document.
To reference a document attribute for insertion, enclose the attribute's name in curly brackets (e.g., `+{name-of-attribute}+`).
This inline element is called an *attribute reference*.
The AsciiDoc processor replaces the attribute reference with the value of the attribute.
To prevent this replacement, you can prefix the element with a backslash (e.g., `+\{name-of-attribute}+`).

[#reference-custom]
== Reference a custom attribute

Before you can reference a custom (i.e., user-defined) attribute in a document, it must first be declared using an attribute entry in the document header.
"#
);

#[test]
fn reference_custom() {
    verifies!(
        r#"
In <<ex-set-custom>>, we declare two user-defined attributes that we'll later be able to reference.

.Custom attributes set in the document header
[source#ex-set-custom,subs=+attributes]
----
= Ops Manual
:disclaimer: {disclaimer}
:url-repo: {url-repo}
----

Once you've set and assigned a value to a document attribute, you can reference that attribute throughout your document.
In <<ex-reference>>, the attribute `url-repo` is referenced twice and `disclaimer` is referenced once.

.Custom attributes referenced in the document body
[source#ex-reference]
----
Asciidoctor is {url-repo}[open source]. <.>

WARNING: {disclaimer} <.>
If you're missing a lime colored sock, file a ticket in
the {url-repo}/issues[Asciidoctor issue tracker]. <.>
(Actually, please don't).
----
<.> Attribute references can be used in macros.
<.> Attribute references can be used in blocks, such as xref:blocks:admonitions.adoc[admonitions], and inline.
Since there isn't an empty line between the `disclaimer` reference and the next sentence, the sentence will be directly appended to the end of the attribute's value when it's processed.
<.> The reference to the `url-repo` attribute is inserted to build the complete URL address, which is interpreted as a xref:macros:url-macro.adoc[URL macro].

As you can see below, the attribute references are replaced with the corresponding attribute value when the document is processed.

====
Asciidoctor is {url-repo}[open source].

WARNING: {disclaimer}
If you're missing a lime colored sock, file a ticket in the {url-repo}/issues[Asciidoctor issue tracker].
Actually, please don't.
====

"#
    );

    let mut parser = Parser::default();

    let doc = parser.parse("= Ops Manual\n:disclaimer: Don't pet the wild Wolpertingers. We're not responsible for any loss \\\nof hair, chocolate, or purple socks.\n:url-repo: https://github.com/asciidoctor/asciidoctor\n\nAsciidoctor is {url-repo}[open source].\n\nWARNING: {disclaimer}\nIf you're missing a lime colored sock, file a ticket in\nthe {url-repo}/issues[Asciidoctor issue tracker].\n(Actually, please don't).");

    assert_eq!(
        doc,
        Document {
            header: Header {
                title_source: Some(Span {
                    data: "Ops Manual",
                    line: 1,
                    col: 3,
                    offset: 2,
                },),
                title: Some("Ops Manual",),
                attributes: &[
                    Attribute {
                        name: Span {
                            data: "disclaimer",
                            line: 2,
                            col: 2,
                            offset: 14,
                        },
                        value_source: Some(Span {
                            data: "Don't pet the wild Wolpertingers. We're not responsible for any loss \\\nof hair, chocolate, or purple socks.",
                            line: 2,
                            col: 14,
                            offset: 26,
                        },),
                        value: InterpretedValue::Value(
                            "Don't pet the wild Wolpertingers. We're not responsible for any loss of hair, chocolate, or purple socks.",
                        ),
                        source: Span {
                            data: ":disclaimer: Don't pet the wild Wolpertingers. We're not responsible for any loss \\\nof hair, chocolate, or purple socks.",
                            line: 2,
                            col: 1,
                            offset: 13,
                        },
                    },
                    Attribute {
                        name: Span {
                            data: "url-repo",
                            line: 4,
                            col: 2,
                            offset: 135,
                        },
                        value_source: Some(Span {
                            data: "https://github.com/asciidoctor/asciidoctor",
                            line: 4,
                            col: 12,
                            offset: 145,
                        },),
                        value: InterpretedValue::Value(
                            "https://github.com/asciidoctor/asciidoctor",
                        ),
                        source: Span {
                            data: ":url-repo: https://github.com/asciidoctor/asciidoctor",
                            line: 4,
                            col: 1,
                            offset: 134,
                        },
                    },
                ],
                author_line: None,
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= Ops Manual\n:disclaimer: Don't pet the wild Wolpertingers. We're not responsible for any loss \\\nof hair, chocolate, or purple socks.\n:url-repo: https://github.com/asciidoctor/asciidoctor",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[
                Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "Asciidoctor is {url-repo}[open source].",
                            line: 6,
                            col: 1,
                            offset: 189,
                        },
                        rendered: "Asciidoctor is <a href=\"https://github.com/asciidoctor/asciidoctor\">open source</a>.",
                    },
                    source: Span {
                        data: "Asciidoctor is {url-repo}[open source].",
                        line: 6,
                        col: 1,
                        offset: 189,
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
                            data: "WARNING: {disclaimer}\nIf you're missing a lime colored sock, file a ticket in\nthe {url-repo}/issues[Asciidoctor issue tracker].\n(Actually, please don't).",
                            line: 8,
                            col: 1,
                            offset: 230,
                        },
                        rendered: "WARNING: Don&#8217;t pet the wild Wolpertingers. We&#8217;re not responsible for any loss of hair, chocolate, or purple socks.\nIf you&#8217;re missing a lime colored sock, file a ticket in\nthe <a href=\"https://github.com/asciidoctor/asciidoctor/issues\">Asciidoctor issue tracker</a>.\n(Actually, please don&#8217;t).",
                    },
                    source: Span {
                        data: "WARNING: {disclaimer}\nIf you're missing a lime colored sock, file a ticket in\nthe {url-repo}/issues[Asciidoctor issue tracker].\n(Actually, please don't).",
                        line: 8,
                        col: 1,
                        offset: 230,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),
            ],
            source: Span {
                data: "= Ops Manual\n:disclaimer: Don't pet the wild Wolpertingers. We're not responsible for any loss \\\nof hair, chocolate, or purple socks.\n:url-repo: https://github.com/asciidoctor/asciidoctor\n\nAsciidoctor is {url-repo}[open source].\n\nWARNING: {disclaimer}\nIf you're missing a lime colored sock, file a ticket in\nthe {url-repo}/issues[Asciidoctor issue tracker].\n(Actually, please don't).",
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
fn reference_built_in() {
    verifies!(
        r#"
[#reference-built-in]
== Reference a built-in attribute

A built-in document attribute (i.e., a document attribute which is automatically set by the processor) is referenced the same way as a custom (i.e., user-defined) document attribute.
For instance, an AsciiDoc processor automatically sets these supported xref:character-replacement-ref.adoc[character replacement attributes].
That means that you can reference them throughout your document without having to create an attribute entry in its header.

[source]
----
TIP: Wolpertingers don't like temperatures above 100{deg}C. <.>
Our servers don't like them either.
----
<.> Reference the character replacement attribute `deg` by enclosing its name in a pair of curly brackets (`{` and `}`).

As you can see below, the attribute reference is replaced with the attribute's value when the document is processed.

TIP: Wolpertingers don't like temperatures above 100{deg}C.
Our servers don't like them either.

"#
    );

    let mut parser = Parser::default();

    let doc = parser.parse("TIP: Wolpertingers don't like temperatures above 100{deg}C.\nOur servers don't like them either.");

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
                        data: "TIP: Wolpertingers don't like temperatures above 100{deg}C.\nOur servers don't like them either.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "TIP: Wolpertingers don&#8217;t like temperatures above 100&#176;C.\nOur servers don&#8217;t like them either.",
                },
                source: Span {
                    data: "TIP: Wolpertingers don't like temperatures above 100{deg}C.\nOur servers don't like them either.",
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
                data: "TIP: Wolpertingers don't like temperatures above 100{deg}C.\nOur servers don't like them either.",
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

mod escape_attribute_reference {
    use crate::{Parser, blocks::SimpleBlockStyle, tests::prelude::*};

    non_normative!(
        r#"
== Escape an attribute reference

You may have a situation where a sequence of characters occurs in your content that matches the syntax of an AsciiDoc attribute reference, but is not, in fact, an AsciiDoc attribute reference.
For example, if you're documenting path templating, you may need to reference a replaceable section of a URL path, which is also enclosed in curly braces (e.g., /items/\{id}).
In this case, you need a way to escape the attribute reference so the AsciiDoc processor knows to skip over it.
Otherwise, the processor could warn about a missing attribute reference or perform an unexpected replacement.
AsciiDoc provides several ways to escape an attribute reference.

"#
    );

    #[test]
    fn prefix_with_backslash() {
        verifies!(
            r#"
=== Prefix with a backslash

You can escape an attribute reference by prefixing it with a backslash.
When the processor encounters this syntax, it will remove the backslash and pass through the remainder of what looks to be an attribute reference as written.

In <<ex-backslash-escape>>, the attribute reference is escaped using a backslash.

.An attribute reference escaped using a backslash
[#ex-backslash-escape]
----
In the path /items/\{id}, id is a path parameter.
----

In the output of <<ex-backslash-escape>>, we can see that the `\{id}` expression in the path is preserved.

====
In the path /items/\{id}, id is a path parameter.
====

"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse(":id: foo\n\nIn the path /items/\\{id}, id is a path parameter.");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[Attribute {
                        name: Span {
                            data: "id",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                        value_source: Some(Span {
                            data: "foo",
                            line: 1,
                            col: 6,
                            offset: 5,
                        },),
                        value: InterpretedValue::Value("foo",),
                        source: Span {
                            data: ":id: foo",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: ":id: foo",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "In the path /items/\\{id}, id is a path parameter.",
                            line: 3,
                            col: 1,
                            offset: 10,
                        },
                        rendered: "In the path /items/{id}, id is a path parameter.",
                    },
                    source: Span {
                        data: "In the path /items/\\{id}, id is a path parameter.",
                        line: 3,
                        col: 1,
                        offset: 10,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: ":id: foo\n\nIn the path /items/\\{id}, id is a path parameter.",
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
    fn backslash_remains_if_no_such_attribute() {
        verifies!(
            r#"
Keep in mind that the backslash will only be recognized if the text between the curly braces is a valid attribute name.
If the syntax that follows the backslash does not match an attribute reference, the backslash will not be removed during processing.

"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse("In the path /items/\\{id}, id is a path parameter.");

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
                            data: "In the path /items/\\{id}, id is a path parameter.",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "In the path /items/\\{id}, id is a path parameter.",
                    },
                    source: Span {
                        data: "In the path /items/\\{id}, id is a path parameter.",
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
                    data: "In the path /items/\\{id}, id is a path parameter.",
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
    fn enclose_in_passthrough() {
        verifies!(
            r#"
=== Enclose in a passthrough

You can also escape an attribute reference by enclosing it in an inline passthrough.
In this case, the processor uses the normal substitution rules for the passthrough type you have chosen.

In <<ex-passthrough-escape>>, the attribute reference is escaped by enclosing it in an inline passthrough.

.An attribute reference escaped by enclosing it in an inline passthrough
[#ex-passthrough-escape]
----
In the path +/items/{id}+, id is a path parameter.
----

In the output of <<ex-passthrough-escape>>, we can see that the `\{id}` expression in the path is preserved.

====
In the path +/items/{id}+, id is a path parameter.
====

"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse(":id: foo\n\nIn the path +/items/{id}+, id is a path parameter.");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[Attribute {
                        name: Span {
                            data: "id",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                        value_source: Some(Span {
                            data: "foo",
                            line: 1,
                            col: 6,
                            offset: 5,
                        },),
                        value: InterpretedValue::Value("foo",),
                        source: Span {
                            data: ":id: foo",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: ":id: foo",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "In the path +/items/{id}+, id is a path parameter.",
                            line: 3,
                            col: 1,
                            offset: 10,
                        },
                        rendered: "In the path /items/{id}, id is a path parameter.",
                    },
                    source: Span {
                        data: "In the path +/items/{id}+, id is a path parameter.",
                        line: 3,
                        col: 1,
                        offset: 10,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: ":id: foo\n\nIn the path +/items/{id}+, id is a path parameter.",
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
    fn enclose_in_passthrough_no_such_attribute() {
        verifies!(
            r#"
When using an inline passthrough, you don't have to worry whether the curly braces form an attribute reference or not.
All the text between the passthrough enclosure will get passed through to the output.

"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse("In the path +/items/{id}+, id is a path parameter.");

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
                            data: "In the path +/items/{id}+, id is a path parameter.",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "In the path /items/{id}, id is a path parameter.",
                    },
                    source: Span {
                        data: "In the path +/items/{id}+, id is a path parameter.",
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
                    data: "In the path +/items/{id}+, id is a path parameter.",
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
=== Alternative escape mechanisms

Attribute references are replaced by the xref:subs:attributes.adoc[attributes substitution].
Therefore, wherever you can control substitutions, you can prevent attribute references from being replaced.
This includes the inline pass macro as well as the subs attribute on a block.
See xref:subs:prevent.adoc#passthroughs[using passthroughs to prevent substitutions] for more details.
"#
);
