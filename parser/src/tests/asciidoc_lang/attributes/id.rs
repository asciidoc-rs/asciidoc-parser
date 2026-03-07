use crate::tests::prelude::*;

track_file!("docs/modules/attributes/pages/id.adoc");

non_normative!(
    r#"
= ID Attribute
:page-aliases: ids.adoc

You can assign an identifier (i.e., unique name) to a block or inline element using the `id` attribute.
The `id` attribute is a xref:positional-and-named-attributes.adoc#named[named attribute].
Its purpose is to identify the element when linking, scripting, or styling.
Thus, the identifier can only be used once in a document.

An ID:

. provides an internal link or cross reference anchor for an element
. can be used for adding additional styling to specific elements (e.g., via a CSS ID selector)

You can assign an ID to blocks using the shorthand hash (`+#+`) syntax, longhand (`id=`) syntax, or the anchor (`[[]]`) syntax.
You can assign an ID to inline elements using the shorthand hash (`+#+`) syntax or by adding an anchor adjacent to the inline element using the anchor (`[[]]`) syntax.
You can assign an ID to a table cell by using an anchor (`[[]]`) at the start of the cell.
Likewise, you can assign an ID to a list item by using an anchor (`[[]]`) at the start of the principal text.

"#
);

mod valid_id_characters {
    use pretty_assertions_sorted::assert_eq;

    use crate::{Parser, blocks::SimpleBlockStyle, tests::prelude::*, warnings::WarningType};

    non_normative!(
        r#"
== Valid ID characters

AsciiDoc does not restrict the set of characters that can be used for an ID when the ID is defined using the named `id` attribute.
"#
    );

    #[test]
    fn value_non_empty() {
        verifies!(
            r#"
All the language requires in this case is that the value be non-empty.
"#
        );

        let mut parser = Parser::default();

        let maw = crate::blocks::Block::parse(
            crate::Span::new("[[]]\nThis paragraph gets a lot of attention.\n"),
            &mut parser,
        );

        assert_eq!(
            maw.warnings,
            vec![Warning {
                source: Span {
                    data: "",
                    line: 1,
                    col: 3,
                    offset: 2,
                },
                warning: WarningType::EmptyBlockAnchorName,
            },]
        );

        let mi = maw.item.unwrap();

        assert_eq!(
            mi.item,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "[[]]\nThis paragraph gets a lot of attention.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "[[]]\nThis paragraph gets a lot of attention.",
                },
                source: Span {
                    data: "[[]]\nThis paragraph gets a lot of attention.",
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
    }

    non_normative!(
        r#"
When the ID is defined using the shorthand hash syntax or the anchor syntax, the acceptable characters is more limited (for example, spaces are not permitted).
Regardless, it's not advisable to exploit the ability to use any characters the AsciiDoc syntax allows.
The reason to be cautious is because the ID is passed through to the output, and not all output formats afford the same latitude.
For example, XML is far more restrictive about which characters are permitted in an ID value.

"#
    );

    #[test]
    fn check_valid_xml_name() {
        verifies!(
            r#"
To ensure portability of your IDs, it's best to conform to a universal standard.
The standard we recommend following is a https://www.w3.org/TR/REC-xml/#NT-Name[Name] value as defined by the XML specification.
At a high level, the first character of a Name must be a letter, colon, or underscore and the optional following characters must be a letter, colon, underscore, hyphen, period, or digit.
You should not use any space characters in an ID.
Starting the ID with a digit is less likely to be problematic, but still best to avoid.
It's best to use lowercase letters whenever possible as this solves portability problem when using case-insensitive platforms.

When the AsciiDoc processor auto-generates IDs for section titles and discrete headings, it adheres to this standard.

Here are examples of valid IDs (according to the recommendations above):

[listing]
----
install
data-structures
error-handling
subject-and-body
unset_an_attribute
----

Here are examples of invalid IDs:

[listing]
----
install the gem
3 blind mice
-about-the-author
----

"#
        );

        let mut parser = Parser::default();

        let maw = crate::blocks::Block::parse(
            crate::Span::new("[[3 blind mice]]\nThis paragraph gets a lot of attention.\n"),
            &mut parser,
        );

        assert_eq!(
            maw.warnings,
            vec![Warning {
                source: Span {
                    data: "3 blind mice",
                    line: 1,
                    col: 3,
                    offset: 2,
                },
                warning: WarningType::InvalidBlockAnchorName,
            },]
        );

        let mi = maw.item.unwrap();

        assert_eq!(
            mi.item,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "[[3 blind mice]]\nThis paragraph gets a lot of attention.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "[[3 blind mice]]\nThis paragraph gets a lot of attention.",
                },
                source: Span {
                    data: "[[3 blind mice]]\nThis paragraph gets a lot of attention.",
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
    }

    non_normative!(
        r#"
////
BlockId

NOTE: Section pending
////

"#
    );
}

mod block_assignment {
    use pretty_assertions_sorted::assert_eq;

    use crate::{
        Parser,
        blocks::{IsBlock, ListType, SimpleBlockStyle},
        tests::prelude::*,
    };

    non_normative!(
        r#"
== Block assignment

// tag::bl[]
You can assign an ID to a block using the shorthand syntax, the longhand syntax, or a block anchor.

"#
    );

    #[test]
    fn shorthand_syntax() {
        verifies!(
            r#"
In the shorthand syntax, you prefix the name with a hash (`#`) in the first position attribute.

[source]
----
[#goals]
* Goal 1
* Goal 2
----

"#
        );

        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(
            crate::Span::new("[#goals]\n* Goal 1\n* Goal 2"),
            &mut parser,
        )
        .unwrap_if_no_warnings()
        .unwrap();

        assert_eq!(
            mi.item,
            Block::List(ListBlock {
                type_: ListType::Unordered,
                items: &[
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::Asterisks(Span {
                            data: "*",
                            line: 2,
                            col: 1,
                            offset: 9,
                        },),
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "Goal 1",
                                    line: 2,
                                    col: 3,
                                    offset: 11,
                                },
                                rendered: "Goal 1",
                            },
                            source: Span {
                                data: "Goal 1",
                                line: 2,
                                col: 3,
                                offset: 11,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "* Goal 1",
                            line: 2,
                            col: 1,
                            offset: 9,
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
                            offset: 18,
                        },),
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "Goal 2",
                                    line: 3,
                                    col: 3,
                                    offset: 20,
                                },
                                rendered: "Goal 2",
                            },
                            source: Span {
                                data: "Goal 2",
                                line: 3,
                                col: 3,
                                offset: 20,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "* Goal 2",
                            line: 3,
                            col: 1,
                            offset: 18,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: "[#goals]\n* Goal 1\n* Goal 2",
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
                        value: "#goals",
                        shorthand_items: &["#goals"],
                    },],
                    anchor: None,
                    source: Span {
                        data: "#goals",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },)
        );
    }

    #[test]
    fn longhand_syntax() {
        verifies!(
            r#"
In the longhand syntax, you use a standard named attribute.

[source]
----
[id=goals]
* Goal 1
* Goal 2
----

"#
        );

        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(
            crate::Span::new("[id=goals]\n* Goal 1\n* Goal 2"),
            &mut parser,
        )
        .unwrap_if_no_warnings()
        .unwrap();

        assert_eq!(
            mi.item,
            Block::List(ListBlock {
                type_: ListType::Unordered,
                items: &[
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::Asterisks(Span {
                            data: "*",
                            line: 2,
                            col: 1,
                            offset: 11,
                        },),
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "Goal 1",
                                    line: 2,
                                    col: 3,
                                    offset: 13,
                                },
                                rendered: "Goal 1",
                            },
                            source: Span {
                                data: "Goal 1",
                                line: 2,
                                col: 3,
                                offset: 13,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "* Goal 1",
                            line: 2,
                            col: 1,
                            offset: 11,
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
                            offset: 20,
                        },),
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "Goal 2",
                                    line: 3,
                                    col: 3,
                                    offset: 22,
                                },
                                rendered: "Goal 2",
                            },
                            source: Span {
                                data: "Goal 2",
                                line: 3,
                                col: 3,
                                offset: 22,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "* Goal 2",
                            line: 3,
                            col: 1,
                            offset: 20,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: "[id=goals]\n* Goal 1\n* Goal 2",
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
                        name: Some("id",),
                        value: "goals",
                        shorthand_items: &[],
                    },],
                    anchor: None,
                    source: Span {
                        data: "id=goals",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },)
        );
    }

    #[test]
    fn block_anchor_syntax() {
        verifies!(
            r#"
In the block anchor syntax, you surround the name with double square brackets:

[source]
----
[[goals]]
* Goal 1
* Goal 2
----

"#
        );

        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(
            crate::Span::new("[[goals]]\n* Goal 1\n* Goal 2"),
            &mut parser,
        )
        .unwrap_if_no_warnings()
        .unwrap();

        assert_eq!(
            mi.item,
            Block::List(ListBlock {
                type_: ListType::Unordered,
                items: &[
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::Asterisks(Span {
                            data: "*",
                            line: 2,
                            col: 1,
                            offset: 10,
                        },),
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "Goal 1",
                                    line: 2,
                                    col: 3,
                                    offset: 12,
                                },
                                rendered: "Goal 1",
                            },
                            source: Span {
                                data: "Goal 1",
                                line: 2,
                                col: 3,
                                offset: 12,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "* Goal 1",
                            line: 2,
                            col: 1,
                            offset: 10,
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
                            offset: 19,
                        },),
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "Goal 2",
                                    line: 3,
                                    col: 3,
                                    offset: 21,
                                },
                                rendered: "Goal 2",
                            },
                            source: Span {
                                data: "Goal 2",
                                line: 3,
                                col: 3,
                                offset: 21,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "* Goal 2",
                            line: 3,
                            col: 1,
                            offset: 19,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: "[[goals]]\n* Goal 1\n* Goal 2",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: Some(Span {
                    data: "goals",
                    line: 1,
                    col: 3,
                    offset: 2,
                },),
                anchor_reftext: None,
                attrlist: None,
            },)
        );
    }

    #[test]
    fn block_style_with_shorthand_syntax() {
        verifies!(
            r#"
Let's say you want to create a blockquote from an open block and assign it an ID and xref:role.adoc[role].
You add `quote` (the block style) in front of the `#` (the ID) in the first attribute position, as this example shows:

[source]
----
[quote.movie#roads,Dr. Emmett Brown]
____
Roads? Where we're going, we don't need roads.
____
----

"#
        );

        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(crate::Span::new("[quote.movie#roads,Dr. Emmett Brown]\n____\nRoads? Where we're going, we don't need roads.\n____"), &mut parser)
            .unwrap_if_no_warnings()
            .unwrap();

        assert_eq!(
            mi.item,
            Block::CompoundDelimited(CompoundDelimitedBlock {
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "Roads? Where we're going, we don't need roads.",
                            line: 3,
                            col: 1,
                            offset: 42,
                        },
                        rendered: "Roads? Where we&#8217;re going, we don&#8217;t need roads.",
                    },
                    source: Span {
                        data: "Roads? Where we're going, we don't need roads.",
                        line: 3,
                        col: 1,
                        offset: 42,
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
                    data: "[quote.movie#roads,Dr. Emmett Brown]\n____\nRoads? Where we're going, we don't need roads.\n____",
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
                            shorthand_items: &["quote", ".movie", "#roads",],
                            value: "quote.movie#roads"
                        },
                        ElementAttribute {
                            name: None,
                            shorthand_items: &[],
                            value: "Dr. Emmett Brown"
                        },
                    ],
                    anchor: None,
                    source: Span {
                        data: "quote.movie#roads,Dr. Emmett Brown",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },)
        );
    }

    non_normative!(
        r#"
TIP: The order of ID and role values in the shorthand syntax does not matter.

"#
    );

    #[test]
    fn block_id_containing_dot() {
        verifies!(
            r#"
CAUTION: If the ID contains a `.`, you must define it using either a longhand assignment (e.g., `id=classname.propertyname`) or the anchor shorthand (e.g., `+[[classname.propertyname]]+`).
This is necessary since the `.` character in the shorthand syntax is the delimiter for a role, and thus gets misinterpreted as such.
// end::bl[]

"#
        );

        let doc = Parser::default()
            .parse("[id=classname.propertyname1]\nprop1\n\n[[classname.propertyname2]]\nprop2");

        let mut blocks = doc.nested_blocks();

        let block1 = blocks.next().unwrap();
        assert_eq!(block1.id().unwrap(), "classname.propertyname1");

        let block2 = blocks.next().unwrap();
        assert_eq!(block2.id().unwrap(), "classname.propertyname2");

        assert!(blocks.next().is_none());
    }
}

mod inline_assignment {
    use pretty_assertions_sorted::assert_eq;

    use crate::{Parser, blocks::SimpleBlockStyle, tests::prelude::*};

    non_normative!(
        r#"
== Inline assignment

// tag::in[]
"#
    );

    #[test]
    fn inline_shorthand_syntax() {
        verifies!(
            r#"
// tag::in[]
The id (`#`) shorthand can be used on inline quoted text.

.Quoted text with ID assignment using shorthand syntax
----
[#free_the_world]#free the world#
----

"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse("[#free_the_world]#free the world#");

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
                            data: "[#free_the_world]#free the world#",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<span id=\"free_the_world\">free the world</span>",
                    },
                    source: Span {
                        data: "[#free_the_world]#free the world#",
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
                    data: "[#free_the_world]#free the world#",
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
    fn inline_anchor_syntax() {
        verifies!(
            r#"
.General text with preceding ID assignment using inline anchor syntax
----
[[free_the_world]]free the world
----
// end::in[]

"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse("[[free_the_world]]free the world");

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
                            data: "[[free_the_world]]free the world",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a id=\"free_the_world\"></a>free the world",
                    },
                    source: Span {
                        data: "[[free_the_world]]free the world",
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
                    data: "[[free_the_world]]free the world",
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

mod anchor {
    use std::collections::HashMap;

    use pretty_assertions_sorted::assert_eq;

    use crate::{Parser, blocks::SimpleBlockStyle, document::RefType, tests::prelude::*};

    non_normative!(
        r#"
[#anchor]
== Use an ID as an anchor

An anchor (aka ID) can be defined almost anywhere in the document, including on a section title, on a discrete heading, on a paragraph, on an image, on a delimited block, on an inline phrase, and so forth.
The anchor is declared by enclosing a _valid_ XML Name in double square brackets (e.g., `+[[idname]]+`) or using the shorthand ID syntax (e.g., `[#idname]`) at the start of an attribute list.
The shorthand form is the preferred syntax.

"#
    );

    #[test]
    fn double_square_bracket_rules() {
        verifies!(
            r#"
The double square bracket form requires the ID to start with a letter, an underscore, or a colon, ensuring the ID is portable.
According to the https://www.w3.org/TR/REC-xml/#NT-Name[XML Name] rules, a portable ID may not begin with a number, even though a number is allowed elsewhere in the name.
"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse("[[start_with_letter]]#free the world#");

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
                            data: "[[start_with_letter]]#free the world#",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a id=\"start_with_letter\"></a><mark>free the world</mark>",
                    },
                    source: Span {
                        data: "[[start_with_letter]]#free the world#",
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
                    data: "[[start_with_letter]]#free the world#",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );

        let mut parser = Parser::default();

        let doc = parser.parse("[[_start_with_underscore]]#free the world#");

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
                            data: "[[_start_with_underscore]]#free the world#",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a id=\"_start_with_underscore\"></a><mark>free the world</mark>",
                    },
                    source: Span {
                        data: "[[_start_with_underscore]]#free the world#",
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
                    data: "[[_start_with_underscore]]#free the world#",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );

        let mut parser = Parser::default();

        let doc = parser.parse("[[:start_with_colon]]#free the world#");

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
                            data: "[[:start_with_colon]]#free the world#",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a id=\":start_with_colon\"></a><mark>free the world</mark>",
                    },
                    source: Span {
                        data: "[[:start_with_colon]]#free the world#",
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
                    data: "[[:start_with_colon]]#free the world#",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );

        let mut parser = Parser::default();

        let doc = parser.parse("[[1start_with_number]]#free the world#");

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
                            data: "[[1start_with_number]]#free the world#",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "[[1start_with_number]]<mark>free the world</mark>",
                    },
                    source: Span {
                        data: "[[1start_with_number]]#free the world#",
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
                    data: "[[1start_with_number]]#free the world#",
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
    fn shorthand_form() {
        verifies!(
            r#"
The shorthand form in an attribute list does not impose this restriction.
"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse("[#1start_with_number]#free the world#");

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
                            data: "[#1start_with_number]#free the world#",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<span id=\"1start_with_number\">free the world</span>",
                    },
                    source: Span {
                        data: "[#1start_with_number]#free the world#",
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
                    data: "[#1start_with_number]#free the world#",
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
    fn block_element_shorthand_syntax() {
        verifies!(
            r#"
=== On block element

To reference a block element, you must assign an ID to that block.
You can define an ID using the shorthand syntax:

.Assign an ID to a paragraph using shorthand syntax
[source]
----
include::example$id.adoc[tag=block-id-shorthand]
----

"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse("[#notice]\nThis paragraph gets a lot of attention.");

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
                            data: "This paragraph gets a lot of attention.",
                            line: 2,
                            col: 1,
                            offset: 10,
                        },
                        rendered: "This paragraph gets a lot of attention.",
                    },
                    source: Span {
                        data: "[#notice]\nThis paragraph gets a lot of attention.",
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
                            value: "#notice",
                            shorthand_items: &["#notice"],
                        },],
                        anchor: None,
                        source: Span {
                            data: "#notice",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[#notice]\nThis paragraph gets a lot of attention.",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([(
                        "notice",
                        RefEntry {
                            id: "notice",
                            reftext: None,
                            ref_type: RefType::Anchor,
                        }
                    ),]),
                    reftext_to_id: HashMap::default(),
                },
            }
        );
    }

    #[test]
    fn block_element_anchor_syntax() {
        verifies!(
            r#"
or you can define it using the block anchor syntax:

.Assign an ID to a paragraph using block anchor syntax
[source]
----
include::example$id.adoc[tag=block-id-brackets]
----

"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse("[[notice]]\nThis paragraph gets a lot of attention.");

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
                            data: "This paragraph gets a lot of attention.",
                            line: 2,
                            col: 1,
                            offset: 11,
                        },
                        rendered: "This paragraph gets a lot of attention.",
                    },
                    source: Span {
                        data: "[[notice]]\nThis paragraph gets a lot of attention.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: Some(Span {
                        data: "notice",
                        line: 1,
                        col: 3,
                        offset: 2,
                    },),
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: "[[notice]]\nThis paragraph gets a lot of attention.",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([(
                        "notice",
                        RefEntry {
                            id: "notice",
                            reftext: None,
                            ref_type: RefType::Anchor,
                        }
                    ),]),
                    reftext_to_id: HashMap::default(),
                },
            }
        );
    }

    #[test]
    fn inline_anchor_brackets() {
        verifies!(
            r#"
=== As an inline anchor

You can also define an anchor anywhere in content that receives normal substitutions (specifically the macros substitution).
You can enclose the ID in double square brackets:

.Define an inline anchor
[source]
----
include::example$id.adoc[tag=anchor-brackets]
----

"#
        );

        let mut parser = Parser::default();

        let doc =
            parser.parse("[[bookmark-a]]Inline anchors make arbitrary content referenceable.");

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
                            data: "[[bookmark-a]]Inline anchors make arbitrary content referenceable.",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<a id=\"bookmark-a\"></a>Inline anchors make arbitrary content referenceable.",
                    },
                    source: Span {
                        data: "[[bookmark-a]]Inline anchors make arbitrary content referenceable.",
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
                    data: "[[bookmark-a]]Inline anchors make arbitrary content referenceable.",
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
    fn inline_anchor_shorthand() {
        verifies!(
            r#"
or using the shorthand ID syntax.

.Define an inline anchor using shorthand syntax
[source]
----
include::example$id.adoc[tag=anchor-shorthand]
----

"#
        );

        let mut parser = Parser::default();

        let doc =
            parser.parse("[#bookmark-b]#Inline anchors can be applied to a phrase like this one.#");

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
                            data: "[#bookmark-b]#Inline anchors can be applied to a phrase like this one.#",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<span id=\"bookmark-b\">Inline anchors can be applied to a phrase like this one.</span>",
                    },
                    source: Span {
                        data: "[#bookmark-b]#Inline anchors can be applied to a phrase like this one.#",
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
                    data: "[#bookmark-b]#Inline anchors can be applied to a phrase like this one.#",
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
