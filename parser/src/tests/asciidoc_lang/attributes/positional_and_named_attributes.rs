use crate::tests::prelude::*;

track_file!("docs/modules/attributes/pages/positional-and-named-attributes.adoc");

non_normative!(
    r#"
= Positional and Named Attributes

This page breaks down the difference between positional and named attributes on an element and the rules for parsing an attribute list.

"#
);

mod positional_attribute {
    use pretty_assertions_sorted::assert_eq;

    use crate::{
        Parser,
        blocks::{IsBlock, ListType, SimpleBlockStyle, metadata::BlockMetadata},
        content::SubstitutionGroup,
        tests::prelude::*,
    };

    non_normative!(
        r#"
[#positional]
== Positional attribute

// tag::pos[]
Entries in an attribute list that only consist of a value are referred to as positional attributes.
The position is the 1-based index of the entry once all named attributes have been removed (so they may be interspersed).

"#
    );

    #[test]
    fn implicit_attribute_name() {
        verifies!(
            r#"
The positional attribute may be dually assigned to an implicit attribute name if the block or macro defines a mapping for positional attributes.
Here are some examples of those mappings:

* `icon:` 1 => size
* `image:` and `image::` 1 => alt (text), 2 => width, 3 => height
* Delimited blocks: 1 => block style and attribute shorthand
* Other inline quoted text: 1 => attribute shorthand
* `link:` and `xref:` 1 => text
* Custom blocks and macros can also specify positional attributes

For example, the following two image macros are equivalent.

[source]
----
image::sunset.jpg[Sunset,300,400]

image::sunset.jpg[alt=Sunset,width=300,height=400]
----

The second macro is the same as the first, but written out in longhand form.
// end::pos[]

"#
        );

        let mut parser = Parser::default();

        let m1 = crate::blocks::MediaBlock::parse(
            &BlockMetadata::new("image::sunset.jpg[Sunset,300,400]"),
            &mut parser,
        )
        .unwrap_if_no_warnings()
        .unwrap();

        let mut parser = Parser::default();

        let m2 = crate::blocks::MediaBlock::parse(
            &BlockMetadata::new("image::sunset.jpg[alt=Sunset,width=300,height=400]"),
            &mut parser,
        )
        .unwrap_if_no_warnings()
        .unwrap();

        let a1 = m1.item.macro_attrlist();
        let a2 = m2.item.macro_attrlist();

        assert_eq!(
            a1.named_or_positional_attribute("alt", 1).unwrap(),
            ElementAttribute {
                name: None,
                shorthand_items: &["Sunset"],
                value: "Sunset"
            },
        );

        assert_eq!(
            a2.named_or_positional_attribute("alt", 1).unwrap(),
            ElementAttribute {
                name: Some("alt"),
                shorthand_items: &[],
                value: "Sunset"
            }
        );
    }

    #[test]
    fn block_style_and_attribute_shorthand() {
        non_normative!(
            r#"
=== Block style and attribute shorthand

The first positional attribute on all blocks (including sections) is special.
It's used to define the xref:blocks:index.adoc#block-style[block style].
It also supports a shorthand notation for defining the ID, role, and options attributes.
This shorthand notation can also be used on formatted text, even though formatted text doesn't technically support attributes.

To be clear, the shorthand notation is allowed in two places:

* The first positional attribute in block attribute line (i.e., the location of the block style)
* The text inside the brackets of formatted text (which is otherwise treated as the role)

The attribute shorthand is inspired by the HAML and Slim template languages as a way of saving the author some typing.
Instead of having to use the longhand form of a name attribute, it's possible to compress the assignment to a value prefixed by a special marker.
The markers are mapped as follows:

* `#` - ID
* `.` - role
* `%` - option

Each shorthand entry is placed directly adjacent to previous one, starting immediately after the optional block style.
The order of the entries does not matter, except for the style, which must come first.
The typical order is style, ID, role(s), option(s).

All of these are valid:

* `#idname.rolename`
* `.rolename1.rolename2`
* `.rolename#idname`
* `#idname.rolename%optionname`

"#
        );

        verifies!(
            r#"
Here's an example that shows how to set an ID on a section using this shorthand notation:

----
[#custom-id]
== Section with Custom ID
----

"#
        );

        let mut parser = Parser::default();

        let block = crate::blocks::Block::parse(
            crate::Span::new("[#custom-id]\n== Section with Custom ID\n"),
            &mut parser,
        )
        .unwrap_if_no_warnings()
        .unwrap()
        .item;

        assert_eq!(
            block,
            Block::Section(SectionBlock {
                level: 1,
                section_title: Content {
                    original: Span {
                        data: "Section with Custom ID",
                        line: 2,
                        col: 4,
                        offset: 16,
                    },
                    rendered: "Section with Custom ID",
                },
                blocks: &[],
                source: Span {
                    data: "[#custom-id]\n== Section with Custom ID",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: Some(Attrlist {
                    attributes: &[ElementAttribute {
                        name: None,
                        shorthand_items: &["#custom-id"],
                        value: "#custom-id"
                    },],
                    anchor: None,
                    source: Span {
                        data: "#custom-id",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
                section_type: SectionType::Normal,
                section_id: None,
                section_number: None,
            },)
        );

        assert_eq!(block.id().unwrap(), "custom-id");

        verifies!(
            r#"
The shorthand entry must follow the block style, if present.
Here's an example that shows how to set an ID on an appendix section using this shorthand notation:

----
[appendix#custom-id]
== Appendix with Custom ID
----

"#
        );

        let mut parser = Parser::default();

        let block = crate::blocks::Block::parse(
            crate::Span::new("[appendix#custom-id]\n== Appendix with Custom ID\n"),
            &mut parser,
        )
        .unwrap_if_no_warnings()
        .unwrap()
        .item;

        assert_eq!(
            block,
            Block::Section(SectionBlock {
                level: 1,
                section_title: Content {
                    original: Span {
                        data: "Appendix with Custom ID",
                        line: 2,
                        col: 4,
                        offset: 24,
                    },
                    rendered: "Appendix with Custom ID",
                },
                blocks: &[],
                source: Span {
                    data: "[appendix#custom-id]\n== Appendix with Custom ID",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: Some(Attrlist {
                    attributes: &[ElementAttribute {
                        name: None,
                        shorthand_items: &["appendix", "#custom-id"],
                        value: "appendix#custom-id"
                    },],
                    anchor: None,
                    source: Span {
                        data: "appendix#custom-id",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
                section_type: SectionType::Appendix,
                section_id: None,
                section_number: None,
            },)
        );

        assert_eq!(block.declared_style().unwrap(), "appendix");
        assert_eq!(block.id().unwrap(), "custom-id");
        assert_eq!(block.substitution_group(), SubstitutionGroup::Normal);

        verifies!(
            r#"
Here's an example of a block that uses the shorthand notation to set the ID, a role, and an option for a list.
Specifically, this syntax sets the ID to `rules`, adds the role `prominent`, and sets the option `incremental`.

----
[#rules.prominent%incremental]
* Work hard
* Play hard
* Be happy
----

"#
        );

        let mut parser = Parser::default();

        let block = crate::blocks::Block::parse(
            crate::Span::new(
                "[#rules.prominent%incremental]\n* Work hard\n* Play hard\n* Be happy\n",
            ),
            &mut parser,
        )
        .unwrap_if_no_warnings()
        .unwrap()
        .item;

        assert_eq!(
            block,
            Block::List(ListBlock {
                type_: ListType::Unordered,
                items: &[
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::Asterisks(Span {
                            data: "*",
                            line: 2,
                            col: 1,
                            offset: 31,
                        },),
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "Work hard",
                                    line: 2,
                                    col: 3,
                                    offset: 33,
                                },
                                rendered: "Work hard",
                            },
                            source: Span {
                                data: "Work hard",
                                line: 2,
                                col: 3,
                                offset: 33,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "* Work hard",
                            line: 2,
                            col: 1,
                            offset: 31,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::Asterisks(Span {
                            data: "*",
                            line: 3,
                            col: 1,
                            offset: 43,
                        },),
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "Play hard",
                                    line: 3,
                                    col: 3,
                                    offset: 45,
                                },
                                rendered: "Play hard",
                            },
                            source: Span {
                                data: "Play hard",
                                line: 3,
                                col: 3,
                                offset: 45,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "* Play hard",
                            line: 3,
                            col: 1,
                            offset: 43,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::Asterisks(Span {
                            data: "*",
                            line: 4,
                            col: 1,
                            offset: 55,
                        },),
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "Be happy",
                                    line: 4,
                                    col: 3,
                                    offset: 57,
                                },
                                rendered: "Be happy",
                            },
                            source: Span {
                                data: "Be happy",
                                line: 4,
                                col: 3,
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
                            data: "* Be happy",
                            line: 4,
                            col: 1,
                            offset: 55,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: "[#rules.prominent%incremental]\n* Work hard\n* Play hard\n* Be happy",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: Some(Attrlist {
                    attributes: &[ElementAttribute {
                        name: None,
                        value: "#rules.prominent%incremental",
                        shorthand_items: &["#rules", ".prominent", "%incremental",],
                    },],
                    anchor: None,
                    source: Span {
                        data: "#rules.prominent%incremental",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },)
        );

        assert_eq!(block.id().unwrap(), "rules");
        assert_eq!(block.roles().first().unwrap(), &"prominent");
        assert_eq!(block.options().first().unwrap(), &"incremental");

        assert!(!block.has_option("rules"));
        assert!(!block.has_option("prominent"));
        assert!(block.has_option("incremental"));

        verifies!(
            r#"
A block can have multiple roles and options, so these shorthand entries may be repeated.
Here's an example that shows how to set several options on a table.
Specifically, this syntax sets the `header`, `footer`, and `autowidth` options.

----
[%header%footer%autowidth]
|===
|Header A |Header B
|Footer A |Footer B
|===
----

"#
        );

        let mut parser = Parser::default();

        let block = crate::blocks::Block::parse(crate::Span::new(
            "[%header%footer%autowidth]\n|===\n|Header A |Header B\n|Footer A |Footer B\n|===\n",
        ), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap()
        .item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "|===\n|Header A |Header B\n|Footer A |Footer B\n|===",
                        line: 2,
                        col: 1,
                        offset: 27,
                    },
                    rendered: "|===\n|Header A |Header B\n|Footer A |Footer B\n|===",
                },
                source: Span {
                    data: "[%header%footer%autowidth]\n|===\n|Header A |Header B\n|Footer A |Footer B\n|===",
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
                        name: None,
                        shorthand_items: &["%header", "%footer", "%autowidth",],
                        value: "%header%footer%autowidth"
                    },],
                    anchor: None,
                    source: Span {
                        data: "%header%footer%autowidth",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },)
        );

        verifies!(
            r#"
This shorthand notation also appears on formatted text.
Here's an example that shows how to set the ID and add a role to a strong phrase.
Specifically, this syntax sets the ID to `free-world` and adds the `goals` role.

----
[#free-world.goals]*free the world*
----

Formatted text does not support a style, so the first and only positional attribute is always the shorthand notation.

"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse("[#free-world.goals]*free the world*");

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
                            data: "[#free-world.goals]*free the world*",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<strong id=\"free-world\" class=\"goals\">free the world</strong>",
                    },
                    source: Span {
                        data: "[#free-world.goals]*free the world*",
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
                    data: "[#free-world.goals]*free the world*",
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

mod named_attribute {
    use pretty_assertions_sorted::assert_eq;

    use crate::{Parser, blocks::SimpleBlockStyle, tests::prelude::*};

    non_normative!(
        r#"
[#named]
== Named attribute

// tag::name[]
"#
    );

    #[test]
    fn basic_syntax() {
        verifies!(
            r#"
A named attribute consists of a name and a value separated by an `=` character (e.g., `name=value`).

"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse("[foo=bar]\nSome text here.");

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
                            data: "Some text here.",
                            line: 2,
                            col: 1,
                            offset: 10,
                        },
                        rendered: "Some text here.",
                    },
                    source: Span {
                        data: "[foo=bar]\nSome text here.",
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
                            name: Some("foo"),
                            value: "bar",
                            shorthand_items: &[],
                        },],
                        anchor: None,
                        source: Span {
                            data: "foo=bar",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[foo=bar]\nSome text here.",
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
    fn enclose_value_in_quotes() {
        verifies!(
            r#"
If the value contains a space, comma, or quote character, it must be enclosed in double or single quotes (e.g., `name="value with space"`).
In all other cases, the surrounding quotes are optional.

"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse("[name=\"value with space\"]\nSome text here.");

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
                            data: "Some text here.",
                            line: 2,
                            col: 1,
                            offset: 26,
                        },
                        rendered: "Some text here.",
                    },
                    source: Span {
                        data: "[name=\"value with space\"]\nSome text here.",
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
                            name: Some("name",),
                            value: "value with space",
                            shorthand_items: &[],
                        },],
                        anchor: None,
                        source: Span {
                            data: "name=\"value with space\"",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[name=\"value with space\"]\nSome text here.",
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
    fn escape_same_quote_with_backslash() {
        verifies!(
            r#"
If the value contains the *same* quote character used to enclose the value, the quote character in the value must be escaped by prefixing it with a backslash (e.g., `value="the song \"Dark Horse\""`).

If enclosing quotes are used, they are dropped from the parsed value and the preceding backslash is dropped from any escaped quotes.

"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse("[value=\"the song \\\"Dark Horse\\\"\"]\nSome text here.");

        assert_eq!(
            &doc,
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
                            data: "Some text here.",
                            line: 2,
                            col: 1,
                            offset: 34,
                        },
                        rendered: "Some text here.",
                    },
                    source: Span {
                        data: "[value=\"the song \\\"Dark Horse\\\"\"]\nSome text here.",
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
                            name: Some("value",),
                            value: "the song \"Dark Horse\"",
                            shorthand_items: &[],
                        },],
                        anchor: None,
                        source: Span {
                            data: "value=\"the song \\\"Dark Horse\\\"\"",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[value=\"the song \\\"Dark Horse\\\"\"]\nSome text here.",
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
    fn unset() {
        verifies!(
            r#"
 [#unset]
 === Unset a named attribute
 
 To undefine a named attribute, set the value to `None` (case sensitive).
 // end::name[]
 
"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse("[foo=bar,value=None]\nSome text here.");

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
                            data: "Some text here.",
                            line: 2,
                            col: 1,
                            offset: 21,
                        },
                        rendered: "Some text here.",
                    },
                    source: Span {
                        data: "[foo=bar,value=None]\nSome text here.",
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
                            name: Some("foo",),
                            value: "bar",
                            shorthand_items: &[],
                        },],
                        anchor: None,
                        source: Span {
                            data: "foo=bar,value=None",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[foo=bar,value=None]\nSome text here.",
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

mod attribute_list_parsing {
    use pretty_assertions_sorted::assert_eq;

    use crate::{Parser, blocks::SimpleBlockStyle, tests::prelude::*, warnings::WarningType};

    non_normative!(
        r#"
== Attribute list parsing

The source text that's used to define attributes for an element is referred to as an [.term]*attrlist*.
An attrlist is always enclosed in a pair of square brackets.
This applies for block attributes as well as attributes on a block or inline macro.
The processor splits the attrlist into individual attribute entries, determines whether each entry is a positional or named attribute, parses the entry accordingly, and assigns the result as an attribute on the node.

The rules for what defines the boundaries of an individual attribute, and whether the attribute is positional or named, are defined below.
"#
    );

    #[test]
    fn definition_of_name() {
        verifies!(
            r#"
In these rules, `name` consists of a word character (letter or numeral) followed by any number of word or `-` characters (e.g., `see-also`).

"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse("[foo=bar,94-pages=94]\nSome text here.");

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
                            data: "Some text here.",
                            line: 2,
                            col: 1,
                            offset: 22,
                        },
                        rendered: "Some text here.",
                    },
                    source: Span {
                        data: "[foo=bar,94-pages=94]\nSome text here.",
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
                                name: Some("foo"),
                                value: "bar",
                                shorthand_items: &[],
                            },
                            ElementAttribute {
                                name: Some("94-pages",),
                                value: "94",
                                shorthand_items: &[],
                            },
                        ],
                        anchor: None,
                        source: Span {
                            data: "foo=bar,94-pages=94",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[foo=bar,94-pages=94]\nSome text here.",
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
    fn attribute_substitution_before_parsing() {
        verifies!(
            r#"
* Attribute references are expanded before the attrlist is parsed (i.e., the attributes substitution is applied).
"#
        );

        let mut parser = Parser::default();

        let doc =
            parser.parse(":url: https://example.com\n\n[foo=bar,target={url}]\nSome text here.");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[Attribute {
                        name: Span {
                            data: "url",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                        value_source: Some(Span {
                            data: "https://example.com",
                            line: 1,
                            col: 7,
                            offset: 6,
                        },),
                        value: InterpretedValue::Value("https://example.com",),
                        source: Span {
                            data: ":url: https://example.com",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: ":url: https://example.com",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "Some text here.",
                            line: 4,
                            col: 1,
                            offset: 50,
                        },
                        rendered: "Some text here.",
                    },
                    source: Span {
                        data: "[foo=bar,target={url}]\nSome text here.",
                        line: 3,
                        col: 1,
                        offset: 27,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[
                            ElementAttribute {
                                name: Some("foo",),
                                value: "bar",
                                shorthand_items: &[],
                            },
                            ElementAttribute {
                                name: Some("target",),
                                value: "https://example.com",
                                shorthand_items: &[],
                            },
                        ],
                        anchor: None,
                        source: Span {
                            data: "foo=bar,target={url}",
                            line: 3,
                            col: 2,
                            offset: 28,
                        },
                    },),
                },),],
                source: Span {
                    data: ":url: https://example.com\n\n[foo=bar,target={url}]\nSome text here.",
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
    fn attrlist_cant_start_with_space() {
        // NOTE: The requirement about parsing an attribute is covered elsewhere.
        verifies!(
            r#"
* Parsing an attribute proceeds from the beginning of the attribute list string or after a previously identified delimiter (`,`).
** The first character of an attribute list cannot be a tab or space.
"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse("[ foo=bar,target={url}]\nSome text here.");

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
                            data: "[ foo=bar,target={url}]\nSome text here.",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "[ foo=bar,target={url}]\nSome text here.",
                    },
                    source: Span {
                        data: "[ foo=bar,target={url}]\nSome text here.",
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
                    data: "[ foo=bar,target={url}]\nSome text here.",
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
    fn subsequent_attrs_ignore_leading_space_or_tab() {
        verifies!(
            r#"
For subsequent attributes, any leading space or tab characters are skipped.
"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse("[foo=bar, target=url]\nSome text here.");

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
                            data: "Some text here.",
                            line: 2,
                            col: 1,
                            offset: 22,
                        },
                        rendered: "Some text here.",
                    },
                    source: Span {
                        data: "[foo=bar, target=url]\nSome text here.",
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
                                name: Some("foo",),
                                value: "bar",
                                shorthand_items: &[],
                            },
                            ElementAttribute {
                                name: Some("target",),
                                value: "url",
                                shorthand_items: &[],
                            },
                        ],
                        anchor: None,
                        source: Span {
                            data: "foo=bar, target=url",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[foo=bar, target=url]\nSome text here.",
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
    fn named_attributes() {
        verifies!(
            r#"
* If a valid attribute name is found, and it is followed by an equals sign (=), then the parser recognizes this as a named attribute.
The text after the equals sign (=) and up to the next comma or end of list is taken as the attribute value.
Space and tab characters around the equals sign (=) and at the end of the value are ignored.
"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse("[foo = bar, target=url]\nSome text here.");

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
                            data: "Some text here.",
                            line: 2,
                            col: 1,
                            offset: 24,
                        },
                        rendered: "Some text here.",
                    },
                    source: Span {
                        data: "[foo = bar, target=url]\nSome text here.",
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
                                name: Some("foo",),
                                value: "bar",
                                shorthand_items: &[],
                            },
                            ElementAttribute {
                                name: Some("target",),
                                value: "url",
                                shorthand_items: &[],
                            },
                        ],
                        anchor: None,
                        source: Span {
                            data: "foo = bar, target=url",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[foo = bar, target=url]\nSome text here.",
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
    fn otherwise_positional() {
        verifies!(
            r#"
* Otherwise, this is a positional attribute with a value that ends at the next delimiter or end of list.
Any space or tab characters at the boundaries of the value are ignored.
"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse("[_foo = bar , zip , target=url]\nSome text here.");

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
                            data: "Some text here.",
                            line: 2,
                            col: 1,
                            offset: 32,
                        },
                        rendered: "Some text here.",
                    },
                    source: Span {
                        data: "[_foo = bar , zip , target=url]\nSome text here.",
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
                                value: "_foo = bar",
                                shorthand_items: &["_foo = bar"]
                            },
                            ElementAttribute {
                                name: None,
                                value: "zip",
                                shorthand_items: &[],
                            },
                            ElementAttribute {
                                name: Some("target",),
                                value: "url",
                                shorthand_items: &[],
                            },
                        ],
                        anchor: None,
                        source: Span {
                            data: "_foo = bar , zip , target=url",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[_foo = bar , zip , target=url]\nSome text here.",
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
    fn parse_value_unquoted() {
        verifies!(
            r#"
* To parse the attribute value:
** If the first character is not a quote, the string is read until the next delimiter or end of string.
"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse("[foo = bar, zip , target=url]\nSome text here.");

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
                            data: "Some text here.",
                            line: 2,
                            col: 1,
                            offset: 30,
                        },
                        rendered: "Some text here.",
                    },
                    source: Span {
                        data: "[foo = bar, zip , target=url]\nSome text here.",
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
                                name: Some("foo",),
                                value: "bar",
                                shorthand_items: &[],
                            },
                            ElementAttribute {
                                name: None,
                                value: "zip",
                                shorthand_items: &["zip"],
                            },
                            ElementAttribute {
                                name: Some("target",),
                                value: "url",
                                shorthand_items: &[],
                            },
                        ],
                        anchor: None,
                        source: Span {
                            data: "foo = bar, zip , target=url",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[foo = bar, zip , target=url]\nSome text here.",
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
    fn parse_value_double_quote_unclosed() {
        verifies!(
            r#"
** If the first character is a double quote (i.e., `"`), then the string is read until the next unescaped double quote or, if there is no closing double quote, the next delimiter.
"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse("[foo = \"bar, zip , target=url]\nSome text here.");

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
                            data: "Some text here.",
                            line: 2,
                            col: 1,
                            offset: 31,
                        },
                        rendered: "Some text here.",
                    },
                    source: Span {
                        data: "[foo = \"bar, zip , target=url]\nSome text here.",
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
                                name: Some("foo",),
                                value: "\"bar",
                                shorthand_items: &[],
                            },
                            ElementAttribute {
                                name: None,
                                value: "zip",
                                shorthand_items: &["zip"],
                            },
                            ElementAttribute {
                                name: Some("target",),
                                value: "url",
                                shorthand_items: &[],
                            },
                        ],
                        anchor: None,
                        source: Span {
                            data: "foo = \"bar, zip , target=url",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[foo = \"bar, zip , target=url]\nSome text here.",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[Warning {
                    source: Span {
                        data: "foo = \"bar, zip , target=url",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                    warning: WarningType::AttributeValueMissingTerminatingQuote,
                },],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }

    #[test]
    fn parse_value_double_quote_closed() {
        verifies!(
            r#"
If there is a closing double quote, the enclosing double quote characters are removed and escaped double quote characters are unescaped; if not, the initial double quote is retained.
"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse("[foo = \"bar\\\"boop\", zip , target=url]\nSome text here.");

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
                            data: "Some text here.",
                            line: 2,
                            col: 1,
                            offset: 38,
                        },
                        rendered: "Some text here.",
                    },
                    source: Span {
                        data: "[foo = \"bar\\\"boop\", zip , target=url]\nSome text here.",
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
                                name: Some("foo",),
                                value: "bar\"boop",
                                shorthand_items: &[],
                            },
                            ElementAttribute {
                                name: None,
                                value: "zip",
                                shorthand_items: &["zip"],
                            },
                            ElementAttribute {
                                name: Some("target",),
                                value: "url",
                                shorthand_items: &[],
                            },
                        ],
                        anchor: None,
                        source: Span {
                            data: "foo = \"bar\\\"boop\", zip , target=url",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[foo = \"bar\\\"boop\", zip , target=url]\nSome text here.",
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
    fn parse_value_single_quote_unclosed() {
        verifies!(
            r#"
** If the next character is a single quote (i.e., `'`), then the string is read until the next unescaped single quote or, if there is no closing single quote, the next delimiter.
"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse("[foo = \'bar, zip , target=url]\nSome text here.");

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
                            data: "Some text here.",
                            line: 2,
                            col: 1,
                            offset: 31,
                        },
                        rendered: "Some text here.",
                    },
                    source: Span {
                        data: "[foo = 'bar, zip , target=url]\nSome text here.",
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
                                name: Some("foo",),
                                value: "'bar",
                                shorthand_items: &[],
                            },
                            ElementAttribute {
                                name: None,
                                value: "zip",
                                shorthand_items: &["zip"],
                            },
                            ElementAttribute {
                                name: Some("target",),
                                value: "url",
                                shorthand_items: &[],
                            },
                        ],
                        anchor: None,
                        source: Span {
                            data: "foo = 'bar, zip , target=url",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[foo = 'bar, zip , target=url]\nSome text here.",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[Warning {
                    source: Span {
                        data: "foo = 'bar, zip , target=url",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                    warning: WarningType::AttributeValueMissingTerminatingQuote,
                },],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }

    #[test]
    fn parse_value_single_quote_closed() {
        verifies!(
            r#"
If there is a closing single quote, the enclosing single quote characters are removed and escaped single quote characters are unescaped; if not, the initial single quote is retained.
"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse("[foo = \'bar\\\'boop\', zip , target=url]\nSome text here.");

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
                            data: "Some text here.",
                            line: 2,
                            col: 1,
                            offset: 38,
                        },
                        rendered: "Some text here.",
                    },
                    source: Span {
                        data: "[foo = 'bar\\'boop', zip , target=url]\nSome text here.",
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
                                name: Some("foo",),
                                value: "bar&#8217;boop",
                                shorthand_items: &[],
                            },
                            ElementAttribute {
                                name: None,
                                value: "zip",
                                shorthand_items: &["zip"],
                            },
                            ElementAttribute {
                                name: Some("target",),
                                value: "url",
                                shorthand_items: &[],
                            },
                        ],
                        anchor: None,
                        source: Span {
                            data: "foo = 'bar\\'boop', zip , target=url",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[foo = 'bar\\'boop', zip , target=url]\nSome text here.",
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
    fn single_quoted_gets_substititions() {
        verifies!(
            r#"
If there is a closing single quote, and the first character is not an escaped single quote, substitutions are performed on the value as described in <<Substitutions>>.

"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse("[quote, author='*Strong* and _emphasis_']\n____\nThis shows formatting substitutions in single-quoted values\n____");

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
                blocks: &[Block::CompoundDelimited(CompoundDelimitedBlock {
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "This shows formatting substitutions in single-quoted values",
                                line: 3,
                                col: 1,
                                offset: 47,
                            },
                            rendered: "This shows formatting substitutions in single-quoted values",
                        },
                        source: Span {
                            data: "This shows formatting substitutions in single-quoted values",
                            line: 3,
                            col: 1,
                            offset: 47,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    context: "quote",
                    source: Span {
                        data: "[quote, author='*Strong* and _emphasis_']\n____\nThis shows formatting substitutions in single-quoted values\n____",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[
                            ElementAttribute {
                                name: None,
                                value: "quote",
                                shorthand_items: &["quote"],
                            },
                            ElementAttribute {
                                name: Some("author",),
                                value: "<strong>Strong</strong> and <em>emphasis</em>",
                                shorthand_items: &[],
                            },
                        ],
                        anchor: None,
                        source: Span {
                            data: "quote, author='*Strong* and _emphasis_'",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[quote, author='*Strong* and _emphasis_']\n____\nThis shows formatting substitutions in single-quoted values\n____",
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
    fn no_escape_in_block_attrlist() {
        verifies!(
            r#"
.When to escape a closing square bracket
****
Since the terminal of an attrlist is a closing square bracket, it's sometimes necessary to escape a closing square bracket if it appears in the value of an attribute.

In line-oriented syntax such as a block attribute list, a block macro, and an include directive, you do not have to escape closing square brackets that appear in the attrlist itself.
That's because the parser already knows to look for the closing square bracket at the end of the line.

"#
        );

        let mut parser = Parser::default();

        let doc = parser
            .parse("[quote, author=my [bracketed] name]\n____\nWho wrote this, anyway?\n____");

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
                blocks: &[Block::CompoundDelimited(CompoundDelimitedBlock {
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "Who wrote this, anyway?",
                                line: 3,
                                col: 1,
                                offset: 41,
                            },
                            rendered: "Who wrote this, anyway?",
                        },
                        source: Span {
                            data: "Who wrote this, anyway?",
                            line: 3,
                            col: 1,
                            offset: 41,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    context: "quote",
                    source: Span {
                        data: "[quote, author=my [bracketed] name]\n____\nWho wrote this, anyway?\n____",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[
                            ElementAttribute {
                                name: None,
                                value: "quote",
                                shorthand_items: &["quote"],
                            },
                            ElementAttribute {
                                name: Some("author",),
                                value: "my [bracketed] name",
                                shorthand_items: &[],
                            },
                        ],
                        anchor: None,
                        source: Span {
                            data: "quote, author=my [bracketed] name",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[quote, author=my [bracketed] name]\n____\nWho wrote this, anyway?\n____",
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
    fn escape_in_inline_attrlist() {
        verifies!(
            r#"
If a closing square bracket appears in the attrlist of an inline element, such as an inline macro, it usually has to be escaped using a backslash or by using the character reference `+&#93;+`.
There are some exceptions to this rule, such as a link macro in a footnote, which are influenced by the substitution order.
****

"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse("http://example.com[sam&#93;ple]bracket]");

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
                            data: "http://example.com[sam&#93;ple]bracket]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a href=\"http://example.com\">sam&#93;ple</a>bracket]",
                    },
                    source: Span {
                        data: "http://example.com[sam&#93;ple]bracket]",
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
                    data: "http://example.com[sam&#93;ple]bracket]",
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

// What follows is covered already by tests earlier in this file.
non_normative!(
    r#"
== Substitutions

// tag::subs[]
Recall that attribute references are expanded before the attrlist is parsed.
Therefore, it's not necessary to force substitutions to be applied to a value if you're only interested in applying the attributes substitution.
The attributes substitution has already been applied at this point.

If the attribute name (in the case of a positional attribute) or value (in the case of a named attribute) is enclosed in single quotes (e.g., `+citetitle='Processed by https://asciidoctor.org'+`), and the attribute is defined in an attrlist on a block, then the xref:subs:index.adoc#normal-group[normal substitution group] is applied to the value at assignment time.
No special processing is performed, aside from the expansion of attribute references, if the value is not enclosed in quotes or is enclosed in double quotes.

If the value contains the same quote character used to enclose the value, escape the quote character in the value by prefixing it with a backslash (e.g., `+citetitle='A \'use case\' diagram, generated by https://plantuml.com'+`).
// end::subs[]
"#
);
