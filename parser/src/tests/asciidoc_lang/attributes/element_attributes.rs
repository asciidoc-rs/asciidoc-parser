use crate::tests::prelude::*;

track_file!("docs/modules/attributes/pages/element-attributes.adoc");

non_normative!(
    r#"
= Element Attributes

Element attributes are a powerful means of controlling the built-in settings of individual block and inline elements in the AsciiDoc syntax.
They can also be used to add supplemental information, such as citation metadata and fallback content, to certain elements.

== What are element attributes?

[.term]*Element attributes* define the built-in and user-defined settings and metadata that can be applied to an individual block element or inline element in a document (including macros).
Although the include directive is not technically an element, element attributes can also be defined on an include directive.

Element attributes may be positional (value only) or named (key/value pair).
Some built-in and extension elements will map a positional attribute to a named attribute.
Each element recognizes a predefined set of positional and/or named element attributes.
Authors may define any number of custom element attributes for passing information to an extension or document analyzer.

Like document attributes, there's no strict schema for element attributes, or for the value of the `options` element attribute.
There's a core set of reserved attributes shared by all block elements and most inline elements, which includes id, role, opts, and title.
Certain elements may reserve additional attributes and option values.
For example, the source block reserves the `lang` attribute to set the source language and the `linenums` option to enable line numbers.
The link macro reserves the `window` attribute to change the target window of a link and the `nofollow` option to prevent crawlers from following it.
Otherwise, the schema for element attributes is open-ended, thus allowing extensions to use them for their own purpose.

Element attributes are commonly used for the following purposes:

* Declare the ID of an element
* Turn on or turn off an individual element's built-in features
* Configure the built-in features of an individual element
* Apply user-defined information, such as citation metadata, fallback text, link text, and target content, to an individual element
* Apply user-defined roles and behaviors to an individual element

Unlike document attributes, element attributes are defined directly on the element to which they apply using an <<attribute-list,attribute list>>.

"#
);

mod attrlist {
    use crate::{
        HasSpan,
        attributes::AttrlistContext,
        blocks::{MediaType, SimpleBlockStyle},
        tests::prelude::*,
    };

    non_normative!(
        r#"
[#attribute-list]
== Attribute lists

Attributes can be assigned to block and inline elements using an [.term]*attribute list* (often abbreviated as attrlist).

"#
    );

    #[test]
    fn anatomy_of_attrlist() {
        verifies!(
            r#"
.Anatomy of an attribute list
----
first-positional,second-positional,named="value of named"
----

Entries in an attribute list are separated by commas, excluding commas inside quotes.
The syntax used for an attribute list entry determines whether it's a positional or named attribute.
The space after the comma separating entries is optional.
To learn more about how the attribute list is parsed, see xref:positional-and-named-attributes.adoc[].

"#
        );

        const ATTRLIST_EXAMPLE: &str =
            r#"first-positional,second-positional,named="value of named""#;

        let p = Parser::default();

        let mi = crate::attributes::Attrlist::parse(
            crate::Span::new(ATTRLIST_EXAMPLE),
            &p,
            AttrlistContext::Inline,
        )
        .unwrap_if_no_warnings();

        assert_eq!(
            mi.item,
            Attrlist {
                attributes: &[
                    ElementAttribute {
                        name: None,
                        shorthand_items: &["first-positional"],
                        value: "first-positional",
                    },
                    ElementAttribute {
                        name: None,
                        shorthand_items: &[],
                        value: "second-positional",
                    },
                    ElementAttribute {
                        name: Some("named"),
                        shorthand_items: &[],
                        value: "value of named",
                    }
                ],
                anchor: None,
                source: Span {
                    data: r#"first-positional,second-positional,named="value of named""#,
                    line: 1,
                    col: 1,
                    offset: 0
                }
            }
        );

        assert!(mi.item.named_attribute("foo").is_none());
        assert!(mi.item.nth_attribute(0).is_none());

        assert_eq!(
            mi.item.nth_attribute(1).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &["first-positional"],
                value: "first-positional",
            }
        );

        assert_eq!(
            mi.item.nth_attribute(2).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &[],
                value: "second-positional",
            }
        );

        assert!(mi.item.nth_attribute(3).is_none());

        assert_eq!(
            mi.item.named_attribute("named").unwrap(),
            ElementAttribute {
                name: Some("named"),
                shorthand_items: &[],
                value: "value of named",
            }
        );

        assert_eq!(
            mi.item.span(),
            Span {
                data: r#"first-positional,second-positional,named="value of named""#,
                line: 1,
                col: 1,
                offset: 0
            }
        );

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 1,
                col: 58,
                offset: 57
            }
        );
    }

    #[test]
    fn block_attrlist() {
        verifies!(
            r#"
For *block elements*, the attribute list is placed inside one or more block attribute lines.
A block attribute line is any line of text above the start of a block (e.g., the opening delimiter or simple content) that begins with `[` and ends with `]`.
This line can be interspersed with other block metadata lines, such as the block title.
The text enclosed in the `[` and `]` boundaries is assumed to be a valid attribute list and the line is automatically consumed.
If the text cannot be parsed, an error message will be emitted to the log.

.A block attribute line
----
[style,second-positional,named="value of named"]
----

"#
        );

        let mut parser = Parser::default();

        let block = crate::blocks::Block::parse(
            crate::Span::new("[style,second-positional,named=\"value of named\"]\nSimple block\n"),
            &mut parser,
        )
        .unwrap_if_no_warnings()
        .unwrap()
        .item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "Simple block",
                        line: 2,
                        col: 1,
                        offset: 49,
                    },
                    rendered: "Simple block",
                },
                source: Span {
                    data: "[style,second-positional,named=\"value of named\"]\nSimple block",
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
                    attributes: &[
                        ElementAttribute {
                            name: None,
                            shorthand_items: &["style"],
                            value: "style",
                        },
                        ElementAttribute {
                            name: None,
                            shorthand_items: &[],
                            value: "second-positional",
                        },
                        ElementAttribute {
                            name: Some("named"),
                            shorthand_items: &[],
                            value: "value of named"
                        },
                    ],
                    anchor: None,
                    source: Span {
                        data: "style,second-positional,named=\"value of named\"",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },)
        );
    }

    #[test]
    fn avoid_attrlist_with_empty() {
        verifies!(
            r#"
WARNING: The opening line of a paragraph may inadvertently match the syntax of a block attribute line.
If this happens, append `+{empty}+` to the end of the line to disrupt the syntax match.

"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse("{empty}[blah, blah blah]\nSome text goes here.");

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
                            data: "{empty}[blah, blah blah]\nSome text goes here.",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "[blah, blah blah]\nSome text goes here.",
                    },
                    source: Span {
                        data: "{empty}[blah, blah blah]\nSome text goes here.",
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
                    data: "{empty}[blah, blah blah]\nSome text goes here.",
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

    #[ignore]
    #[test]
    fn block_macro_attrlist() {
        // Disabling this test for now since we no longer have a generic
        // macro block.
        verifies!(
            r#"
For *block and inline macros*, the attribute list is placed between the square brackets of the macro.
The text in an attribute list of a block macro never needs to be escaped.
For an inline macro, it may be necessary to escape the text in the attribute list to avoid prematurely ending the macro or unwanted substitutions.

.A block macro with an attribute list
----
name::target[first-positional,second-positional,named="value of named"]
----

"#
        );

        let mut parser = Parser::default();
        let block = crate::blocks::Block::parse(
            crate::Span::new(
                "name::target[first-positional,second-positional,named=\"value of named\"]\n",
            ),
            &mut parser,
        )
        .unwrap_if_no_warnings()
        .unwrap()
        .item;

        assert_eq!(
            block,
            Block::Media(MediaBlock {
                type_: MediaType::Image,
                target: Span {
                    data: "target",
                    line: 1,
                    col: 7,
                    offset: 6,
                },
                macro_attrlist: Attrlist {
                    attributes: &[
                        ElementAttribute {
                            name: None,
                            shorthand_items: &["first-positional"],
                            value: "first-positional"
                        },
                        ElementAttribute {
                            name: None,
                            shorthand_items: &[],
                            value: "second-positional"
                        },
                        ElementAttribute {
                            name: Some("named"),
                            shorthand_items: &[],
                            value: "value of named"
                        },
                    ],
                    anchor: None,
                    source: Span {
                        data: "first-positional,second-positional,named=\"value of named\"",
                        line: 1,
                        col: 14,
                        offset: 13,
                    },
                },
                source: Span {
                    data: "name::target[first-positional,second-positional,named=\"value of named\"]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },)
        );
    }

    #[test]
    fn inline_attrlist() {
        verifies!(
            r#"
For *formatted text*, the attribute list is placed in the square brackets in front of the text enclosure.
However, formatted text only supports a restricted form of the attribute list.
Specifically, it does not support named attributes, only the attribute shorthand syntax.

.Formatted text with an attribute list
----
[#idname.rolename]*text with id and role*
----

"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse("[#idname.rolename]*text with id and role*");

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
                            data: "[#idname.rolename]*text with id and role*",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<strong id=\"idname\" class=\"rolename\">text with id and role</strong>",
                    },
                    source: Span {
                        data: "[#idname.rolename]*text with id and role*",
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
                    data: "[#idname.rolename]*text with id and role*",
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

    // Treated as non-normative because these requirements are covered elsewhere.
    non_normative!(
        r#"
Attribute lists:

* apply to blocks, macros, and inline elements,
* can contain xref:positional-and-named-attributes.adoc[positional and named attributes], and
* take precedence over xref:document-attributes.adoc[document attributes] if the element supports the override.

As mentioned in the previous section, the schema for element attributes is open-ended.
Any positional or named attributes that are not recognized will be stored on the element, but will not have an impact on the behavior or output.
Extensions may use this auxiliary information to influence their behavior and/or customize the output.
"#
    );
}
