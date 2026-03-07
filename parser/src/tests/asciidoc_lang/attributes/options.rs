use crate::tests::prelude::*;

track_file!("docs/modules/attributes/pages/options.adoc");

non_normative!(
    r#"
= Options Attribute

The `options` attribute (often abbreviated as `opts`) is a versatile xref:positional-and-named-attributes.adoc#named[named attribute] that can be assigned one or more values.
It can be defined globally as document attribute as well as a block attribute on an individual block.

There is no strict schema for options.
Any options which are not recognized are ignored.

"#
);

mod assign_options_to_blocks {
    non_normative!(
        r#"
== Assign options to blocks

You can assign one or more options to a block using the shorthand or formal syntax for the `options` attribute.

"#
    );

    use pretty_assertions_sorted::assert_eq;

    use crate::{
        Parser,
        blocks::{IsBlock, SimpleBlockStyle},
        tests::prelude::*,
    };

    #[test]
    fn shorthand_syntax_single() {
        verifies!(
            r#"
=== Shorthand options syntax for blocks

To assign an option to a block, prefix the value with a percent sign (`%`) in an attribute list.
The percent sign implicitly sets the `options` attribute.

.Sidebar block with an option assigned using the shorthand dot
[source#ex-block]
----
[%option]
****
This is a sidebar with an option assigned to it, named option.
****
----

"#
        );

        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(crate::Span::new(
            "[%option]\n****\nThis is a sidebar with an option assigned to it, named option.\n****",
        ), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

        assert_eq!(
            mi.item,
            Block::CompoundDelimited(CompoundDelimitedBlock {
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "This is a sidebar with an option assigned to it, named option.",
                            line: 3,
                            col: 1,
                            offset: 15,
                        },
                        rendered: "This is a sidebar with an option assigned to it, named option.",
                    },
                    source: Span {
                        data: "This is a sidebar with an option assigned to it, named option.",
                        line: 3,
                        col: 1,
                        offset: 15,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                context: "sidebar",
                source: Span {
                    data: "[%option]\n****\nThis is a sidebar with an option assigned to it, named option.\n****",
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
                        shorthand_items: &["%option"],
                        value: "%option"
                    },],
                    anchor: None,
                    source: Span {
                        data: "%option",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },)
        );

        let options = mi.item.options();
        let mut options = options.iter();

        assert_eq!(options.next().unwrap(), &"option");

        assert!(options.next().is_none());

        assert!(mi.item.has_option("option"));
        assert!(!mi.item.has_option("option1"));
    }

    #[test]
    fn shorthand_syntax_multiple() {
        verifies!(
            r#"
You can assign multiple options to a block by prefixing each value with a percent sign (`%`).

.Sidebar with two options assigned using the shorthand dot
[source#ex-two-options]
----
[%option1%option2]
****
This is a sidebar with two options assigned to it, named option1 and option2.
****
----

"#
        );

        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(crate::Span::new(
            "[%option1%option2]\n****\nThis is a sidebar with two options assigned to it, named option1 and option2.\n****",
        ), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

        assert_eq!(
            mi.item,
            Block::CompoundDelimited(CompoundDelimitedBlock {
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "This is a sidebar with two options assigned to it, named option1 and option2.",
                            line: 3,
                            col: 1,
                            offset: 24,
                        },
                        rendered: "This is a sidebar with two options assigned to it, named option1 and option2.",
                    },
                    source: Span {
                        data: "This is a sidebar with two options assigned to it, named option1 and option2.",
                        line: 3,
                        col: 1,
                        offset: 24,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                context: "sidebar",
                source: Span {
                    data: "[%option1%option2]\n****\nThis is a sidebar with two options assigned to it, named option1 and option2.\n****",
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
                        shorthand_items: &["%option1", "%option2"],
                        value: "%option1%option2"
                    },],
                    anchor: None,
                    source: Span {
                        data: "%option1%option2",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },)
        );

        let options = mi.item.options();
        let mut options = options.iter();

        assert_eq!(options.next().unwrap(), &"option1");

        assert_eq!(options.next().unwrap(), &"option2");

        assert!(options.next().is_none());

        assert!(mi.item.has_option("option1"));
        assert!(mi.item.has_option("option2"));
        assert!(!mi.item.has_option("option3"));
    }

    #[test]
    fn table_with_three_options() {
        verifies!(
            r#"
For instance, consider a table with the three built-in option values, `header`, `footer`, and `autowidth`, assigned to it.
<<ex-table-short>> shows how the values are assigned using the shorthand notation.

.Table with three options assigned using the shorthand syntax
[source#ex-table-short]
----
[%header%footer%autowidth,cols=2*~]
|===
|Cell A1 |Cell B1

|Cell A2 |Cell B2

|Cell A3 |Cell B3
|===
----

"#
        );

        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(crate::Span::new(
            "[%header%footer%autowidth,cols=2*~]\n|===\n|Cell A1 |Cell B1\n\n|Cell A2 |Cell B2\n\n|Cell A3 |Cell B3\n|===",
        ), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

        // IMPORTANT: This test will have to be revised once we parse tables.

        assert_eq!(
            mi.item,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "|===\n|Cell A1 |Cell B1",
                        line: 2,
                        col: 1,
                        offset: 36,
                    },
                    rendered: "|===\n|Cell A1 |Cell B1",
                },
                source: Span {
                    data: "[%header%footer%autowidth,cols=2*~]\n|===\n|Cell A1 |Cell B1",
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
                            shorthand_items: &["%header", "%footer", "%autowidth",],
                            value: "%header%footer%autowidth"
                        },
                        ElementAttribute {
                            name: Some("cols"),
                            shorthand_items: &[],
                            value: "2*~"
                        },
                    ],
                    anchor: None,
                    source: Span {
                        data: "%header%footer%autowidth,cols=2*~",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },)
        );

        let options = mi.item.options();
        let mut options = options.iter();

        assert_eq!(options.next().unwrap(), &"header");

        assert_eq!(options.next().unwrap(), &"footer");

        assert_eq!(options.next().unwrap(), &"autowidth");

        assert!(options.next().is_none());

        assert!(mi.item.has_option("header"));
        assert!(mi.item.has_option("footer"));
        assert!(mi.item.has_option("autowidth"));
        assert!(!mi.item.has_option("option"));
    }

    #[test]
    fn formal_options_syntax_single() {
        verifies!(
            r#"
=== Formal options syntax for blocks

Explicitly set `options` or `opts`, followed by the equals sign (`=`), and then the value in an attribute list.

.Sidebar block with an option assigned using the formal syntax
[source#ex-block-formal]
----
[opts=option]
****
This is a sidebar with an option assigned to it, named option.
****
----

"#
        );

        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(crate::Span::new(
            "[opts=option]\n****\nThis is a sidebar with an option assigned to it, named option.\n****",
        ), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

        assert_eq!(
            mi.item,
            Block::CompoundDelimited(CompoundDelimitedBlock {
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "This is a sidebar with an option assigned to it, named option.",
                            line: 3,
                            col: 1,
                            offset: 19,
                        },
                        rendered: "This is a sidebar with an option assigned to it, named option.",
                    },
                    source: Span {
                        data: "This is a sidebar with an option assigned to it, named option.",
                        line: 3,
                        col: 1,
                        offset: 19,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                context: "sidebar",
                source: Span {
                    data: "[opts=option]\n****\nThis is a sidebar with an option assigned to it, named option.\n****",
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
                        name: Some("opts"),
                        shorthand_items: &[],
                        value: "option"
                    },],
                    anchor: None,
                    source: Span {
                        data: "opts=option",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },)
        );

        let options = mi.item.options();
        let mut options = options.iter();

        assert_eq!(options.next().unwrap(), &"option");

        assert!(options.next().is_none());

        assert!(mi.item.has_option("option"));
    }

    #[test]
    fn formal_options_syntax_multiple() {
        verifies!(
            r#"
Separate multiple option values with commas (`,`).

.Sidebar with three options assigned using the formal syntax
[source#ex-three-roles-formal]
----
[opts="option1,option2"]
****
This is a sidebar with two options assigned to it, option1 and option2.
****
----

"#
        );

        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(crate::Span::new(
            "[opts=\"option1,option2\"]\n****\nThis is a sidebar with two options assigned to it, option1 and option2.\n****",
        ), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

        assert_eq!(
            mi.item,
            Block::CompoundDelimited(CompoundDelimitedBlock {
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "This is a sidebar with two options assigned to it, option1 and option2.",
                            line: 3,
                            col: 1,
                            offset: 30,
                        },
                        rendered: "This is a sidebar with two options assigned to it, option1 and option2.",
                    },
                    source: Span {
                        data: "This is a sidebar with two options assigned to it, option1 and option2.",
                        line: 3,
                        col: 1,
                        offset: 30,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                context: "sidebar",
                source: Span {
                    data: "[opts=\"option1,option2\"]\n****\nThis is a sidebar with two options assigned to it, option1 and option2.\n****",
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
                        name: Some("opts"),
                        shorthand_items: &[],
                        value: "option1,option2"
                    },],
                    anchor: None,
                    source: Span {
                        data: "opts=\"option1,option2\"",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },)
        );

        let options = mi.item.options();
        let mut options = options.iter();

        assert_eq!(options.next().unwrap(), &"option1");

        assert_eq!(options.next().unwrap(), &"option2");

        assert!(options.next().is_none());

        assert!(mi.item.has_option("option1"));
        assert!(mi.item.has_option("option2"));
        assert!(!mi.item.has_option("option3"));
    }

    #[test]
    fn table_with_three_options_formal() {
        verifies!(
            r#"
Let's revisit the table in <<ex-table-short>> that has the three built-in option values, `header`, `footer`, and `autowidth`, assigned to it using the shorthand notation (`%`).
Instead of using the shorthand notation, <<ex-table-formal>> shows how the values are assigned using the formal syntax.

.Table with three options assigned using the formal syntax
[source#ex-table-formal]
----
[cols=2*~,opts="header,footer,autowidth"]
|===
|Cell A1 |Cell B1

|Cell A2 |Cell B2

|Cell A3 |Cell B3
|===
----

"#
        );

        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(crate::Span::new(
            "[cols=2*~,opts=\"header,footer,autowidth\"]\n|===\n|Cell A1 |Cell B1\n\n|Cell A2 |Cell B2\n\n|Cell A3 |Cell B3\n|===",
        ), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

        // IMPORTANT: This test will have to be revised once we support tables.

        assert_eq!(
            mi.item,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "|===\n|Cell A1 |Cell B1",
                        line: 2,
                        col: 1,
                        offset: 42,
                    },
                    rendered: "|===\n|Cell A1 |Cell B1",
                },
                source: Span {
                    data: "[cols=2*~,opts=\"header,footer,autowidth\"]\n|===\n|Cell A1 |Cell B1",
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
                            name: Some("cols"),
                            shorthand_items: &[],
                            value: "2*~"
                        },
                        ElementAttribute {
                            name: Some("opts"),
                            shorthand_items: &[],
                            value: "header,footer,autowidth"
                        },
                    ],
                    anchor: None,
                    source: Span {
                        data: "cols=2*~,opts=\"header,footer,autowidth\"",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },)
        );

        let options = mi.item.options();
        let mut options = options.iter();

        assert_eq!(options.next().unwrap(), &"header");

        assert_eq!(options.next().unwrap(), &"footer");

        assert_eq!(options.next().unwrap(), &"autowidth");

        assert!(options.next().is_none());

        assert!(mi.item.has_option("header"));
        assert!(mi.item.has_option("footer"));
        assert!(mi.item.has_option("autowidth"));
        assert!(!mi.item.has_option("option1"));
    }
}

mod using_options_with_other_attributes {
    non_normative!(
        r#"
== Using options with other attributes

Let's consider `options` when combined with other attributes.
"#
    );

    use pretty_assertions_sorted::assert_eq;

    use crate::{
        Parser,
        blocks::{IsBlock, ListType, SimpleBlockStyle},
        content::SubstitutionGroup,
        tests::prelude::*,
    };

    #[test]
    fn style_role_and_options() {
        verifies!(
            r#"
The following example shows how to structure an attribute list when you have style, role, and options attributes.

.Shorthand
[source]
----
[horizontal.properties%step] <.> <.> <.>
property 1:: does stuff
property 2:: does different stuff
----
<.> xref:blocks:styles.adoc[The block style attribute], declared as `horizontal` in this example, is a positional attribute.
A block style value is always placed at the start of the attribute list.
<.> `properties` is prefixed with a dot (`.`), signifying that it's assigned to the xref:role.adoc[role attribute].
The role and options attributes can be set in either order, i.e., `[horizontal%step.properties]`.
<.> The percent sign (`%`) sets the `options` attribute and assigns the `step` value to it.

"#
        );

        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(crate::Span::new(
            "[horizontal.properties%step]\nproperty 1:: does stuff\nproperty 2:: does different stuff",
        ), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

        assert_eq!(
            mi.item,
            Block::List(ListBlock {
                type_: ListType::Description,
                items: &[
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::DefinedTerm {
                            term: Content {
                                original: Span {
                                    data: "property 1",
                                    line: 2,
                                    col: 1,
                                    offset: 29,
                                },
                                rendered: "property 1",
                            },
                            marker: Span {
                                data: "::",
                                line: 2,
                                col: 11,
                                offset: 39,
                            },
                            source: Span {
                                data: "property 1::",
                                line: 2,
                                col: 1,
                                offset: 29,
                            },
                        },
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "does stuff",
                                    line: 2,
                                    col: 14,
                                    offset: 42,
                                },
                                rendered: "does stuff",
                            },
                            source: Span {
                                data: "does stuff",
                                line: 2,
                                col: 14,
                                offset: 42,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "property 1:: does stuff",
                            line: 2,
                            col: 1,
                            offset: 29,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::DefinedTerm {
                            term: Content {
                                original: Span {
                                    data: "property 2",
                                    line: 3,
                                    col: 1,
                                    offset: 53,
                                },
                                rendered: "property 2",
                            },
                            marker: Span {
                                data: "::",
                                line: 3,
                                col: 11,
                                offset: 63,
                            },
                            source: Span {
                                data: "property 2::",
                                line: 3,
                                col: 1,
                                offset: 53,
                            },
                        },
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "does different stuff",
                                    line: 3,
                                    col: 14,
                                    offset: 66,
                                },
                                rendered: "does different stuff",
                            },
                            source: Span {
                                data: "does different stuff",
                                line: 3,
                                col: 14,
                                offset: 66,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "property 2:: does different stuff",
                            line: 3,
                            col: 1,
                            offset: 53,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: "[horizontal.properties%step]\nproperty 1:: does stuff\nproperty 2:: does different stuff",
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
                        value: "horizontal.properties%step",
                        shorthand_items: &["horizontal", ".properties", "%step"]
                    },],
                    anchor: None,
                    source: Span {
                        data: "horizontal.properties%step",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },)
        );

        assert_eq!(mi.item.declared_style().unwrap(), "horizontal");

        let options = mi.item.options();
        let mut options = options.iter();

        assert_eq!(options.next().unwrap(), &"step");

        assert!(options.next().is_none());

        let roles = mi.item.roles();
        let mut roles = roles.iter();

        assert_eq!(roles.next().unwrap(), &"properties");

        assert!(roles.next().is_none());

        assert!(mi.item.has_option("step"));
        assert!(!mi.item.has_option("properties"));

        assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);
    }

    #[test]
    fn style_role_and_options_formal() {
        verifies!(
            r#"
When you use the formal syntax, the positional and named attributes are separated by commas (`,`).

.Formal
[source]
----
[horizontal,role=properties,opts=step] <.>
property 1:: does stuff
property 2:: does different stuff
----
<.> Like in the shorthand example, named attributes such as `role` and `options` can be set in any order in the attribute list once any xref:positional-and-named-attributes.adoc#positional[positional attributes] are set.
"#
        );

        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(crate::Span::new(
            "[horizontal,role=properties,opts=step]\nproperty 1:: does stuff\nproperty 2:: does different stuff",
        ), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

        assert_eq!(
            mi.item,
            Block::List(ListBlock {
                type_: ListType::Description,
                items: &[
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::DefinedTerm {
                            term: Content {
                                original: Span {
                                    data: "property 1",
                                    line: 2,
                                    col: 1,
                                    offset: 39,
                                },
                                rendered: "property 1",
                            },
                            marker: Span {
                                data: "::",
                                line: 2,
                                col: 11,
                                offset: 49,
                            },
                            source: Span {
                                data: "property 1::",
                                line: 2,
                                col: 1,
                                offset: 39,
                            },
                        },
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "does stuff",
                                    line: 2,
                                    col: 14,
                                    offset: 52,
                                },
                                rendered: "does stuff",
                            },
                            source: Span {
                                data: "does stuff",
                                line: 2,
                                col: 14,
                                offset: 52,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "property 1:: does stuff",
                            line: 2,
                            col: 1,
                            offset: 39,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::DefinedTerm {
                            term: Content {
                                original: Span {
                                    data: "property 2",
                                    line: 3,
                                    col: 1,
                                    offset: 63,
                                },
                                rendered: "property 2",
                            },
                            marker: Span {
                                data: "::",
                                line: 3,
                                col: 11,
                                offset: 73,
                            },
                            source: Span {
                                data: "property 2::",
                                line: 3,
                                col: 1,
                                offset: 63,
                            },
                        },
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "does different stuff",
                                    line: 3,
                                    col: 14,
                                    offset: 76,
                                },
                                rendered: "does different stuff",
                            },
                            source: Span {
                                data: "does different stuff",
                                line: 3,
                                col: 14,
                                offset: 76,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "property 2:: does different stuff",
                            line: 3,
                            col: 1,
                            offset: 63,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: "[horizontal,role=properties,opts=step]\nproperty 1:: does stuff\nproperty 2:: does different stuff",
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
                            value: "horizontal",
                            shorthand_items: &["horizontal"],
                        },
                        ElementAttribute {
                            name: Some("role",),
                            value: "properties",
                            shorthand_items: &[],
                        },
                        ElementAttribute {
                            name: Some("opts",),
                            value: "step",
                            shorthand_items: &[],
                        },
                    ],
                    anchor: None,
                    source: Span {
                        data: "horizontal,role=properties,opts=step",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },)
        );

        assert_eq!(mi.item.declared_style().unwrap(), "horizontal");

        let options = mi.item.options();
        let mut options = options.iter();

        assert_eq!(options.next().unwrap(), &"step");

        assert!(options.next().is_none());

        assert!(mi.item.has_option("step"));
        assert!(!mi.item.has_option("horizontal"));
        assert!(!mi.item.has_option("properties"));

        let roles = mi.item.roles();
        let mut roles = roles.iter();

        assert_eq!(roles.next().unwrap(), &"properties");

        assert!(roles.next().is_none());

        assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);
    }
}
