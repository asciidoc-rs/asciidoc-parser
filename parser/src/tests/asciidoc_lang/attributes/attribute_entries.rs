use crate::tests::prelude::*;

track_file!("docs/modules/attributes/pages/attribute-entries.adoc");

non_normative!(
    r#"
= Attribute Entries

== What is an attribute entry?

Before you can use a document attribute in your document, you have to declare it.
An [.term]*attribute entry* is the primary mechanism for defining a document attribute in an AsciiDoc document.
You can think of an attribute entry as a global variable assignment for AsciiDoc.
The document attribute it creates becomes available from that point forward in the document.
Attribute entries are also frequently used to toggle features.

"#
);

#[test]
fn set_boolean_attribute() {
    verifies!(
        r#"
An attribute entry consists of two parts: an attribute *name* and an attribute *value*.
The attribute name comes first, followed by the optional value.
Each attribute entry must be entered on its own line.
An attribute entry starts with an opening colon (`:`), directly followed by the attribute's name, and then a closing colon (`:`).
This [.term]*sets* -- that is, turns on -- the document attribute so you can use it in your document.

[source]
----
:name-of-an-attribute: <.>
----
<.> The attribute's name is directly preceded with a opening colon (`:`) and directly followed by a closing colon (`:`).

"#
    );

    let mi = crate::document::Attribute::parse(
        crate::Span::new(":name-of-an-attribute:"),
        &Parser::default(),
    )
    .unwrap();

    assert_eq!(
        mi.item,
        Attribute {
            name: Span {
                data: "name-of-an-attribute",
                line: 1,
                col: 2,
                offset: 1,
            },
            value_source: None,
            value: InterpretedValue::Set,
            source: Span {
                data: ":name-of-an-attribute:",
                line: 1,
                col: 1,
                offset: 0,
            },
        }
    );

    assert_eq!(mi.item.value(), &InterpretedValue::Set);
}

#[test]
fn explicit_value() {
    verifies!(
        r#"
In many cases, you explicitly assign a value to a document attribute by entering information after its name in the attribute entry.
The value must be offset from the closing colon (`:`) by at least one space.

[source]
----
:name-of-an-attribute: value of the attribute <.>
----
<.> An explicitly assigned value is offset from the closing colon (`:`) by at least one space.
At the end of the value, press kbd:[Enter].

"#
    );

    let mut parser = Parser::default();

    let doc = parser.parse("= Testing\n:name-of-an-attribute: value of the attribute\n\nThe value of the attribute named `name-of-an-attribute` is: {name-of-an-attribute}");

    assert_eq!(
        doc,
        Document {
            header: Header {
                title_source: Some(Span {
                    data: "Testing",
                    line: 1,
                    col: 3,
                    offset: 2,
                },),
                title: Some("Testing"),
                attributes: &[Attribute {
                    name: Span {
                        data: "name-of-an-attribute",
                        line: 2,
                        col: 2,
                        offset: 11,
                    },
                    value_source: Some(Span {
                        data: "value of the attribute",
                        line: 2,
                        col: 24,
                        offset: 33,
                    }),
                    value: InterpretedValue::Value("value of the attribute"),
                    source: Span {
                        data: ":name-of-an-attribute: value of the attribute",
                        line: 2,
                        col: 1,
                        offset: 10,
                    },
                },],
                author_line: None,
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= Testing\n:name-of-an-attribute: value of the attribute",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "The value of the attribute named `name-of-an-attribute` is: {name-of-an-attribute}",
                        line: 4,
                        col: 1,
                        offset: 57,
                    },
                    rendered: "The value of the attribute named <code>name-of-an-attribute</code> is: value of the attribute",
                },
                source: Span {
                    data: "The value of the attribute named `name-of-an-attribute` is: {name-of-an-attribute}",
                    line: 4,
                    col: 1,
                    offset: 57,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },),],
            source: Span {
                data: "= Testing\n:name-of-an-attribute: value of the attribute\n\nThe value of the attribute named `name-of-an-attribute` is: {name-of-an-attribute}",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog::default(),
        }
    );

    assert_eq!(
        parser.attribute_value("name-of-an-attribute"),
        InterpretedValue::Value("value of the attribute")
    );
}

#[test]
fn header_substitutions_applied() {
    verifies!(
        r#"
Take note that xref:attribute-entry-substitutions.adoc[header substitutions] automatically get applied to the value by default.
That means you don't need to escape special characters such in an HTML tag.
"#
    );

    let mut parser = Parser::default();

    let doc = parser.parse("= Testing\n:lt-attribute: <\n\nThe value of the attribute named `lt-attribute` is: {lt-attribute}");

    assert_eq!(
        doc,
        Document {
            header: Header {
                title_source: Some(Span {
                    data: "Testing",
                    line: 1,
                    col: 3,
                    offset: 2,
                },),
                title: Some("Testing"),
                attributes: &[Attribute {
                    name: Span {
                        data: "lt-attribute",
                        line: 2,
                        col: 2,
                        offset: 11,
                    },
                    value_source: Some(Span {
                        data: "<",
                        line: 2,
                        col: 16,
                        offset: 25,
                    },),
                    value: InterpretedValue::Value("&lt;"),
                    source: Span {
                        data: ":lt-attribute: <",
                        line: 2,
                        col: 1,
                        offset: 10,
                    },
                },],
                author_line: None,
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= Testing\n:lt-attribute: <",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "The value of the attribute named `lt-attribute` is: {lt-attribute}",
                        line: 4,
                        col: 1,
                        offset: 28,
                    },
                    rendered: "The value of the attribute named <code>lt-attribute</code> is: &lt;",
                },
                source: Span {
                    data: "The value of the attribute named `lt-attribute` is: {lt-attribute}",
                    line: 4,
                    col: 1,
                    offset: 28,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },),],
            source: Span {
                data: "= Testing\n:lt-attribute: <\n\nThe value of the attribute named `lt-attribute` is: {lt-attribute}",
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
fn reference_existing_attributes() {
    verifies!(
        r#"
It also means you can reference the value of attributes which have already been defined when defining the value of an attribute.
Attribute references in the value of an attribute entry are resolved immediately.

[source]
----
:url-org: https://example.org/projects
:url-project: {url-org}/project-name <.>
----
<.> You can reuse the value of an attribute which has already been set using using an attribute reference in the value.

"#
    );

    let mut parser = Parser::default();

    let doc = parser
        .parse(":url-org: https://example.org/projects\n:url-project: {url-org}/project-name");

    assert_eq!(
        doc,
        Document {
            header: Header {
                title_source: None,
                title: None,
                attributes: &[
                    Attribute {
                        name: Span {
                            data: "url-org",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                        value_source: Some(Span {
                            data: "https://example.org/projects",
                            line: 1,
                            col: 11,
                            offset: 10,
                        },),
                        value: InterpretedValue::Value("https://example.org/projects",),
                        source: Span {
                            data: ":url-org: https://example.org/projects",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },
                    Attribute {
                        name: Span {
                            data: "url-project",
                            line: 2,
                            col: 2,
                            offset: 40,
                        },
                        value_source: Some(Span {
                            data: "{url-org}/project-name",
                            line: 2,
                            col: 15,
                            offset: 53,
                        },),
                        value: InterpretedValue::Value("https://example.org/projects/project-name",),
                        source: Span {
                            data: ":url-project: {url-org}/project-name",
                            line: 2,
                            col: 1,
                            offset: 39,
                        },
                    },
                ],
                author_line: None,
                revision_line: None,
                comments: &[],
                source: Span {
                    data: ":url-org: https://example.org/projects\n:url-project: {url-org}/project-name",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[],
            source: Span {
                data: ":url-org: https://example.org/projects\n:url-project: {url-org}/project-name",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog::default(),
        }
    );

    assert_eq!(
        parser.attribute_value("url-project"),
        InterpretedValue::Value("https://example.org/projects/project-name")
    );
}

#[test]
fn implicit_value() {
    verifies!(
        r#"
Some built-in attributes don't require a value to be explicitly assigned in an attribute entry because they're a boolean attribute or have an implied value.

[source]
----
:name-of-an-attribute: <.>
----
<.> If you don't want to explicitly assign a value to the attribute, press kbd:[Enter] after the closing colon (`:`).

When set, the value of a built-in boolean attribute is always empty (i.e., an _empty string_).
If you set a built-in attribute and leave its value empty, the AsciiDoc processor may infer a value at processing time.

"#
    );

    let mut parser = Parser::default();

    let doc = parser.parse(":name-of-an-attribute:");

    assert_eq!(
        doc,
        Document {
            header: Header {
                title_source: None,
                title: None,
                attributes: &[Attribute {
                    name: Span {
                        data: "name-of-an-attribute",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                    value_source: None,
                    value: InterpretedValue::Set,
                    source: Span {
                        data: ":name-of-an-attribute:",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },],
                author_line: None,
                revision_line: None,
                comments: &[],
                source: Span {
                    data: ":name-of-an-attribute:",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[],
            source: Span {
                data: ":name-of-an-attribute:",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog::default(),
        }
    );

    assert_eq!(
        parser.attribute_value("name-of-an-attribute"),
        InterpretedValue::Set
    );
}

mod where_declared {
    use crate::tests::prelude::*;

    non_normative!(
        r#"
== Where can an attribute entry be declared?

An attribute entry is most often declared in the document header.
"#
    );

    #[test]
    fn declared_between_blocks() {
        verifies!(
            r#"
For attributes that allow it (which includes general purpose attributes), the attribute entry can alternately be declared between blocks in the document body (i.e., the portion of the document below the header).

"#
        );
        let mut parser = Parser::default().with_intrinsic_attribute(
            "agreed",
            "yes",
            ModificationContext::Anywhere,
        );

        let doc =
            parser.parse("We are agreed? {agreed}\n\n:agreed: no\n\nAre we still agreed? {agreed}");

        let mut blocks = doc.nested_blocks();

        let block1 = blocks.next().unwrap();

        assert_eq!(
            block1,
            &Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "We are agreed? {agreed}",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "We are agreed? yes",
                },
                source: Span {
                    data: "We are agreed? {agreed}",
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
            })
        );

        let _ = blocks.next().unwrap();

        let block3 = blocks.next().unwrap();

        assert_eq!(
            block3,
            &Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "Are we still agreed? {agreed}",
                        line: 5,
                        col: 1,
                        offset: 38,
                    },
                    rendered: "Are we still agreed? no",
                },
                source: Span {
                    data: "Are we still agreed? {agreed}",
                    line: 5,
                    col: 1,
                    offset: 38,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            })
        );

        let mut warnings = doc.warnings();
        assert!(warnings.next().is_none());
    }

    non_normative!(
        r#"
WARNING: An attribute entry should not be declared inside the boundaries of a delimited block.
When an attribute entry is declared inside a delimited block, the behavior is undefined.
What's certain is that preprocessor directives (i.e., include directive, conditional directives) cannot see attributes defined inside a delimited block.

When an attribute is defined in the document header using an attribute entry, that's referred to as a header attribute.
A header attribute is available to the entire document until it is unset.
A header attribute is also accessible from the document metadata for use by built-in behavior, extensions, and other applications that need to consult its value (e.g., `source-highlighter`).

When an attribute is defined in the document body using an attribute entry, that's simply referred to as a document attribute.
For any attribute defined in the body, the attribute is available from the point it is set until it is unset.
Attributes defined in the body are not available via the document metadata.

Unless the attribute is locked, it can be unset or assigned a new value in the document header or body.
However, note that unsetting or redefining a header attribute that controls behavior in the document body usually has no affect.
See the xref:document-attributes-ref.adoc[] for where in a document each attribute can be set.

"#
    );
}

mod defining_without_attribute_entry {
    use crate::tests::prelude::*;

    // Non-normative because we have a different API and no CLI.
    non_normative!(
        r#"
== Defining document attributes without an attribute entry

Document attributes can also be declared (set with an optional value or unset) outside the document via the CLI and API.
The attribute entry syntax is not used in these cases.
Rather, they are declared using the provided option.
For the API, attributes are declared using the `:attributes` option (which supports various entry formats).
For the CLI, the attribute is declared using the `-a` option.

"#
    );

    #[test]
    fn no_substitutions_applied() {
        verifies!(
            r#"
When an attribute is assigned a value outside of the document, the value is stored as is, meaning substitutions are not applied to it.
That also means that the xref:subs:index.adoc[special characters and quote substitutions] are not applied to the value of that attribute when it is referenced in the document.
However, subsequent substitutions, such as the macro substitution, do get applied.
This behavior is due to that fact that the attributes substitution is applied after the special characters and quote substitutions.
In order to force these substitutions to be applied to the value of the attribute, you must alter the substitution order at the point of reference.
Here's an example using the inline pass macro.

[,asciidoc]
----
pass:a,q[{attribute-with-formatted-text}]
----

"#
        );

        let mut parser = Parser::default().with_intrinsic_attribute(
            "attribute-with-formatted-text",
            "attribute with *formatted* _text_",
            ModificationContext::Anywhere,
        );

        let doc =
            parser.parse("formatting applied: pass:a,q[{attribute-with-formatted-text}]\n\nformatting suppressed: {attribute-with-formatted-text}");

        let mut blocks = doc.nested_blocks();

        let block1 = blocks.next().unwrap();

        assert_eq!(
            block1,
            &Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "formatting applied: pass:a,q[{attribute-with-formatted-text}]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "formatting applied: attribute with <strong>formatted</strong> <em>text</em>",
                },
                source: Span {
                    data: "formatting applied: pass:a,q[{attribute-with-formatted-text}]",
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
            })
        );

        let block2 = blocks.next().unwrap();

        assert_eq!(
            block2,
            &Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "formatting suppressed: {attribute-with-formatted-text}",
                        line: 3,
                        col: 1,
                        offset: 63,
                    },
                    rendered: "formatting suppressed: attribute with *formatted* _text_",
                },
                source: Span {
                    data: "formatting suppressed: {attribute-with-formatted-text}",
                    line: 3,
                    col: 1,
                    offset: 63,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            })
        );
    }

    // Non-normative because we have a different API and no CLI.
    non_normative!(
        r#"
When an attribute is declared from the command line or API, it is implicitly a document header attribute.
By default, the attribute becomes locked (i.e., hard set or unset) and thus cannot be changed by the document.
This behavior can be changed by adding an `@` to the end of the attribute name or value (i.e., the soft set modifier).
See xref:assignment-precedence.adoc[] for more information.

The one exception to this rule is the `sectnums` attribute, which can always be changed.

"#
    );
}

// Non-normative because this block is commented out.
non_normative!(
    r#"
////
An exclamation point (`!`) before (or after) the attribute name unsets the attribute.

[source]
----
:!name: <1>
----
<1> The leading `!` indicates this attribute should be unset.
In this case, the value is ignored.

An attribute entry must start at the beginning of the line.
If the attribute entry follows a paragraph, it must be offset by an empty line.
////
"#
);
