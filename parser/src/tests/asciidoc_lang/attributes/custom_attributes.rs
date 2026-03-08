use crate::{Parser, tests::prelude::*};

track_file!("docs/modules/attributes/pages/custom-attributes.adoc");

non_normative!(
    r#"
= Declare Custom Document Attributes
:navtitle: Declare Custom Attributes
// [#set-user-defined]

When you find yourself typing the same text repeatedly, or text that often needs to be updated, consider creating your own attribute.

"#
);

mod user_defined_names {
    use crate::{Parser, tests::prelude::*};

    verifies!(
        r#"
[#user-defined-names]
== User-defined attribute names and values

A user-defined attribute must have a name and explicitly assigned value.

The attribute's name must:

* be at least one character long,
* begin with a word character (A-Z, a-z, 0-9, or _), and
* only contain word characters and hyphens.

The name cannot contain dots or spaces.

Although uppercase characters are permitted in an attribute name, the name is converted to lowercase before being stored.
For example, `URL` and `Url` are treated as `url`.
A best practice is to only use lowercase letters in the name and avoid starting the name with a number.

"#
    );

    #[test]
    fn at_least_one_character_long() {
        assert!(
            crate::document::Attribute::parse(crate::Span::new("::"), &Parser::default()).is_none()
        );

        let mi =
            crate::document::Attribute::parse(crate::Span::new(":a:"), &Parser::default()).unwrap();

        assert_eq!(
            mi.item,
            Attribute {
                name: Span {
                    data: "a",
                    line: 1,
                    col: 2,
                    offset: 1,
                },
                value_source: None,
                value: InterpretedValue::Set,
                source: Span {
                    data: ":a:",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            }
        );

        assert_eq!(mi.item.value(), &InterpretedValue::Set);
    }

    #[test]
    fn begin_with_word_character() {
        assert!(
            crate::document::Attribute::parse(crate::Span::new(":-abc:"), &Parser::default())
                .is_none()
        );

        let mi = crate::document::Attribute::parse(crate::Span::new(":9abc:"), &Parser::default())
            .unwrap();

        assert_eq!(
            mi.item,
            Attribute {
                name: Span {
                    data: "9abc",
                    line: 1,
                    col: 2,
                    offset: 1,
                },
                value_source: None,
                value: InterpretedValue::Set,
                source: Span {
                    data: ":9abc:",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            }
        );

        assert_eq!(mi.item.value(), &InterpretedValue::Set);

        let mi = crate::document::Attribute::parse(crate::Span::new(":_abc:"), &Parser::default())
            .unwrap();

        assert_eq!(
            mi.item,
            Attribute {
                name: Span {
                    data: "_abc",
                    line: 1,
                    col: 2,
                    offset: 1,
                },
                value_source: None,
                value: InterpretedValue::Set,
                source: Span {
                    data: ":_abc:",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            }
        );

        assert_eq!(mi.item.value(), &InterpretedValue::Set);
    }

    #[test]
    fn only_contain_word_characters_and_hyphens() {
        assert!(
            crate::document::Attribute::parse(crate::Span::new(":abc def:"), &Parser::default())
                .is_none()
        );
        assert!(
            crate::document::Attribute::parse(crate::Span::new(":abc.def:"), &Parser::default())
                .is_none()
        );

        let mi =
            crate::document::Attribute::parse(crate::Span::new(":9ab-cdef:"), &Parser::default())
                .unwrap();

        assert_eq!(
            mi.item,
            Attribute {
                name: Span {
                    data: "9ab-cdef",
                    line: 1,
                    col: 2,
                    offset: 1,
                },
                value_source: None,
                value: InterpretedValue::Set,
                source: Span {
                    data: ":9ab-cdef:",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            }
        );

        assert_eq!(mi.item.value(), &InterpretedValue::Set);
    }

    #[test]
    fn may_contain_uppercase() {
        let mut parser = Parser::default();
        parser.parse(":URL: /foo/bar");

        assert_eq!(parser.attribute_value("URL"), InterpretedValue::Unset);

        assert_eq!(
            parser.attribute_value("url"),
            InterpretedValue::Value("/foo/bar")
        );
    }
}

// NOTE: This test is redundant with ./wrap_values.rs.
verifies!(
    r#"
[[user-values]]Attribute values can:

* be any inline content, and
* contain line breaks, but only if an xref:wrap-values.adoc#hard[explicit line continuation] (`+`) is used.

"#
);

#[test]
fn create_custom_attribute_and_value() {
    verifies!(
        r#"
== Create a custom attribute and value

A prime use case for attribute entries is to promote frequently used text and URLs to the top of the document.

.Create a user-defined attribute and value
[source#ex-user-set]
----
:disclaimer: Don't pet the wild Wolpertingers. If you let them into your system, we're \ <.>
not responsible for any loss of hair, chocolate, or purple socks.
:url-repo: https://github.com/asciidoctor/asciidoctor
----
<.> Long values can be xref:wrap-values.adoc[soft wrapped] using a backslash (`\`).

Now, you can xref:reference-attributes.adoc#reference-custom[reference these attributes] throughout the document.
"#
    );

    let mi = crate::document::Attribute::parse(crate::Span::new(":disclaimer: Don't pet the wild Wolpertingers. If you let them into your system, we're \\\nnot responsible for any loss of hair, chocolate, or purple socks."), &Parser::default()).unwrap();

    assert_eq!(
        mi.item,
        Attribute {
            name: Span {
                data: "disclaimer",
                line: 1,
                col: 2,
                offset: 1,
            },
            value_source: Some(Span {
                data: "Don't pet the wild Wolpertingers. If you let them into your system, we're \\\nnot responsible for any loss of hair, chocolate, or purple socks.",
                line: 1,
                col: 14,
                offset: 13,
            }),
            value: InterpretedValue::Value(
                "Don't pet the wild Wolpertingers. If you let them into your system, we're not responsible for any loss of hair, chocolate, or purple socks."
            ),
            source: Span {
                data: ":disclaimer: Don't pet the wild Wolpertingers. If you let them into your system, we're \\\nnot responsible for any loss of hair, chocolate, or purple socks.",
                line: 1,
                col: 1,
                offset: 0,
            },
        }
    );

    assert_eq!(
        mi.item.value(),
        InterpretedValue::Value(
            "Don't pet the wild Wolpertingers. If you let them into your system, we're not responsible for any loss of hair, chocolate, or purple socks."
        )
    );

    let mi = crate::document::Attribute::parse(
        crate::Span::new(":url-repo: https://github.com/asciidoctor/asciidoctor"),
        &Parser::default(),
    )
    .unwrap();

    assert_eq!(
        mi.item,
        Attribute {
            name: Span {
                data: "url-repo",
                line: 1,
                col: 2,
                offset: 1,
            },
            value_source: Some(Span {
                data: "https://github.com/asciidoctor/asciidoctor",
                line: 1,
                col: 12,
                offset: 11,
            }),
            value: InterpretedValue::Value("https://github.com/asciidoctor/asciidoctor"),
            source: Span {
                data: ":url-repo: https://github.com/asciidoctor/asciidoctor",
                line: 1,
                col: 1,
                offset: 0,
            },
        }
    );

    assert_eq!(
        mi.item.value(),
        InterpretedValue::Value("https://github.com/asciidoctor/asciidoctor")
    );
}
