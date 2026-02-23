use crate::tests::prelude::*;

track_file!("docs/modules/lists/pages/unordered.adoc");

non_normative!(
    r#"
= Unordered Lists
:keywords: bulleted list

You can make unordered lists in AsciiDoc by starting lines with a designated marker.
An unordered list is a list with items prefixed with symbol, such as a disc (aka bullet).

AsciiDoc builds on the well-established convention of using either an asterisk or hyphen to identify a list item.
Adjacent list items are joined into a single list.
Unordered lists can be nested by varying the marker character or length (asterisk only).
List items may contain attached blocks.
They can also be interleaved with other types of lists.

"#
);

mod basic {
    use std::collections::HashMap;

    use pretty_assertions_sorted::assert_eq;

    use crate::{
        Parser,
        blocks::{ListType, SimpleBlockStyle},
        tests::prelude::*,
    };

    non_normative!(
        r#"
== Basic unordered list

"#
    );

    #[test]
    fn example() {
        verifies!(
            r#"
In the example below, each list item is marked using an asterisk (`+*+`), the AsciiDoc syntax specifying an unordered list item.

----
include::example$unordered.adoc[tag=base]
----

A list item's first line of text must be offset from the marker (`+*+`) by at least one space.
Empty lines are required before and after a list.
Additionally, empty lines are permitted, but not required, between list items.

.Rendered unordered list
====
include::example$unordered.adoc[tag=base]
====

"#
        );

        let doc = Parser::default().parse("* Edgar Allan Poe\n* Sheri S. Tepper\n* Bill Bryson");

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
                blocks: &[Block::List(ListBlock {
                    type_: ListType::Unordered,
                    items: &[
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Asterisks(Span {
                                data: "*",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Edgar Allan Poe",
                                        line: 1,
                                        col: 3,
                                        offset: 2,
                                    },
                                    rendered: "Edgar Allan Poe",
                                },
                                source: Span {
                                    data: "Edgar Allan Poe",
                                    line: 1,
                                    col: 3,
                                    offset: 2,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: "* Edgar Allan Poe",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Asterisks(Span {
                                data: "*",
                                line: 2,
                                col: 1,
                                offset: 18,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Sheri S. Tepper",
                                        line: 2,
                                        col: 3,
                                        offset: 20,
                                    },
                                    rendered: "Sheri S. Tepper",
                                },
                                source: Span {
                                    data: "Sheri S. Tepper",
                                    line: 2,
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
                                data: "* Sheri S. Tepper",
                                line: 2,
                                col: 1,
                                offset: 18,
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
                                offset: 36,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Bill Bryson",
                                        line: 3,
                                        col: 3,
                                        offset: 38,
                                    },
                                    rendered: "Bill Bryson",
                                },
                                source: Span {
                                    data: "Bill Bryson",
                                    line: 3,
                                    col: 3,
                                    offset: 38,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: "* Bill Bryson",
                                line: 3,
                                col: 1,
                                offset: 36,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                    ],
                    source: Span {
                        data: "* Edgar Allan Poe\n* Sheri S. Tepper\n* Bill Bryson",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: "* Edgar Allan Poe\n* Sheri S. Tepper\n* Bill Bryson",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([]),
                    reftext_to_id: HashMap::from([]),
                },
            }
        );
    }

    #[test]
    fn prefix_title_with_period() {
        verifies!(
            r#"
You can add a title to a list by prefixing the title with a period (`.`).

----
include::example$unordered.adoc[tag=base-t]
----

.Rendered unordered list with a title
====
include::example$unordered.adoc[tag=base-t]
====

"#
        );

        let doc = Parser::default().parse(
            ".Kizmet's Favorite Authors\n* Edgar Allan Poe\n* Sheri S. Tepper\n* Bill Bryson",
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
                blocks: &[Block::List(ListBlock {
                    type_: ListType::Unordered,
                    items: &[
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Asterisks(Span {
                                data: "*",
                                line: 2,
                                col: 1,
                                offset: 27,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Edgar Allan Poe",
                                        line: 2,
                                        col: 3,
                                        offset: 29,
                                    },
                                    rendered: "Edgar Allan Poe",
                                },
                                source: Span {
                                    data: "Edgar Allan Poe",
                                    line: 2,
                                    col: 3,
                                    offset: 29,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: "* Edgar Allan Poe",
                                line: 2,
                                col: 1,
                                offset: 27,
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
                                offset: 45,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Sheri S. Tepper",
                                        line: 3,
                                        col: 3,
                                        offset: 47,
                                    },
                                    rendered: "Sheri S. Tepper",
                                },
                                source: Span {
                                    data: "Sheri S. Tepper",
                                    line: 3,
                                    col: 3,
                                    offset: 47,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: "* Sheri S. Tepper",
                                line: 3,
                                col: 1,
                                offset: 45,
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
                                offset: 63,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Bill Bryson",
                                        line: 4,
                                        col: 3,
                                        offset: 65,
                                    },
                                    rendered: "Bill Bryson",
                                },
                                source: Span {
                                    data: "Bill Bryson",
                                    line: 4,
                                    col: 3,
                                    offset: 65,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: "* Bill Bryson",
                                line: 4,
                                col: 1,
                                offset: 63,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                    ],
                    source: Span {
                        data: ".Kizmet's Favorite Authors\n* Edgar Allan Poe\n* Sheri S. Tepper\n* Bill Bryson",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: Some(Span {
                        data: "Kizmet's Favorite Authors",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },),
                    title: Some("Kizmet&#8217;s Favorite Authors",),
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: ".Kizmet's Favorite Authors\n* Edgar Allan Poe\n* Sheri S. Tepper\n* Bill Bryson",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([]),
                    reftext_to_id: HashMap::from([]),
                },
            }
        );
    }

    #[test]
    fn hyphen_list_marker() {
        verifies!(
            r#"
Was your instinct to use a hyphen (`-`) instead of an asterisk to mark list items?
Guess what?
That works too!

----
include::example$unordered.adoc[tag=base-alt]
----

You should reserve the hyphen for lists that only have a single level because the hyphen marker (`-`) doesn't work for nested lists.
"#
        );

        let doc = Parser::default().parse("- Edgar Allan Poe\n- Sheri S. Tepper\n- Bill Bryson");

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
                blocks: &[Block::List(ListBlock {
                    type_: ListType::Unordered,
                    items: &[
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Hyphen(Span {
                                data: "-",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Edgar Allan Poe",
                                        line: 1,
                                        col: 3,
                                        offset: 2,
                                    },
                                    rendered: "Edgar Allan Poe",
                                },
                                source: Span {
                                    data: "Edgar Allan Poe",
                                    line: 1,
                                    col: 3,
                                    offset: 2,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: "- Edgar Allan Poe",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Hyphen(Span {
                                data: "-",
                                line: 2,
                                col: 1,
                                offset: 18,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Sheri S. Tepper",
                                        line: 2,
                                        col: 3,
                                        offset: 20,
                                    },
                                    rendered: "Sheri S. Tepper",
                                },
                                source: Span {
                                    data: "Sheri S. Tepper",
                                    line: 2,
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
                                data: "- Sheri S. Tepper",
                                line: 2,
                                col: 1,
                                offset: 18,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Hyphen(Span {
                                data: "-",
                                line: 3,
                                col: 1,
                                offset: 36,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Bill Bryson",
                                        line: 3,
                                        col: 3,
                                        offset: 38,
                                    },
                                    rendered: "Bill Bryson",
                                },
                                source: Span {
                                    data: "Bill Bryson",
                                    line: 3,
                                    col: 3,
                                    offset: 38,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: "- Bill Bryson",
                                line: 3,
                                col: 1,
                                offset: 36,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                    ],
                    source: Span {
                        data: "- Edgar Allan Poe\n- Sheri S. Tepper\n- Bill Bryson",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: "- Edgar Allan Poe\n- Sheri S. Tepper\n- Bill Bryson",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([]),
                    reftext_to_id: HashMap::from([]),
                },
            }
        );
    }

    non_normative!(
        r#"
Now that we've mentioned nested lists, let's go to the next section and learn how to create lists with multiple levels.

"#
    );

    #[test]
    fn forcing_lists_apart() {
        verifies!(
            r#"
[#separating-lists]
.Forcing lists apart
****
If you have adjacent lists, they have the tendency to want to fuse together.
To force lists apart, insert a line comment (`//`) surrounded by empty lines between the two lists.
Here's an example, where the `-` text in the line comment indicates the line serves as an "`end of list`" marker:

----
include::example$unordered.adoc[tag=divide]
----

This technique works for all list types.
See xref:separating.adoc[] for more details.
****

"#
        );

        let doc = Parser::default().parse("* Apples\n* Oranges\n\n//-\n\n* Walnuts\n* Almonds");

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
                    Block::List(ListBlock {
                        type_: ListType::Unordered,
                        items: &[
                            Block::ListItem(ListItem {
                                marker: ListItemMarker::Asterisks(Span {
                                    data: "*",
                                    line: 1,
                                    col: 1,
                                    offset: 0,
                                },),
                                blocks: &[Block::Simple(SimpleBlock {
                                    content: Content {
                                        original: Span {
                                            data: "Apples",
                                            line: 1,
                                            col: 3,
                                            offset: 2,
                                        },
                                        rendered: "Apples",
                                    },
                                    source: Span {
                                        data: "Apples",
                                        line: 1,
                                        col: 3,
                                        offset: 2,
                                    },
                                    style: SimpleBlockStyle::Paragraph,
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),],
                                source: Span {
                                    data: "* Apples",
                                    line: 1,
                                    col: 1,
                                    offset: 0,
                                },
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),
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
                                            data: "Oranges",
                                            line: 2,
                                            col: 3,
                                            offset: 11,
                                        },
                                        rendered: "Oranges",
                                    },
                                    source: Span {
                                        data: "Oranges",
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
                                    data: "* Oranges",
                                    line: 2,
                                    col: 1,
                                    offset: 9,
                                },
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),
                        ],
                        source: Span {
                            data: "* Apples\n* Oranges",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "//-",
                                line: 4,
                                col: 1,
                                offset: 20,
                            },
                            rendered: "",
                        },
                        source: Span {
                            data: "//-",
                            line: 4,
                            col: 1,
                            offset: 20,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::List(ListBlock {
                        type_: ListType::Unordered,
                        items: &[
                            Block::ListItem(ListItem {
                                marker: ListItemMarker::Asterisks(Span {
                                    data: "*",
                                    line: 6,
                                    col: 1,
                                    offset: 25,
                                },),
                                blocks: &[Block::Simple(SimpleBlock {
                                    content: Content {
                                        original: Span {
                                            data: "Walnuts",
                                            line: 6,
                                            col: 3,
                                            offset: 27,
                                        },
                                        rendered: "Walnuts",
                                    },
                                    source: Span {
                                        data: "Walnuts",
                                        line: 6,
                                        col: 3,
                                        offset: 27,
                                    },
                                    style: SimpleBlockStyle::Paragraph,
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),],
                                source: Span {
                                    data: "* Walnuts",
                                    line: 6,
                                    col: 1,
                                    offset: 25,
                                },
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),
                            Block::ListItem(ListItem {
                                marker: ListItemMarker::Asterisks(Span {
                                    data: "*",
                                    line: 7,
                                    col: 1,
                                    offset: 35,
                                },),
                                blocks: &[Block::Simple(SimpleBlock {
                                    content: Content {
                                        original: Span {
                                            data: "Almonds",
                                            line: 7,
                                            col: 3,
                                            offset: 37,
                                        },
                                        rendered: "Almonds",
                                    },
                                    source: Span {
                                        data: "Almonds",
                                        line: 7,
                                        col: 3,
                                        offset: 37,
                                    },
                                    style: SimpleBlockStyle::Paragraph,
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),],
                                source: Span {
                                    data: "* Almonds",
                                    line: 7,
                                    col: 1,
                                    offset: 35,
                                },
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),
                        ],
                        source: Span {
                            data: "* Walnuts\n* Almonds",
                            line: 6,
                            col: 1,
                            offset: 25,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: "* Apples\n* Oranges\n\n//-\n\n* Walnuts\n* Almonds",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([]),
                    reftext_to_id: HashMap::from([]),
                },
            }
        );
    }
}

mod nested {
    use std::collections::HashMap;

    use pretty_assertions_sorted::assert_eq;

    use crate::{
        Parser,
        blocks::{ListType, SimpleBlockStyle},
        tests::prelude::*,
    };

    non_normative!(
        r#"
== Nested unordered list

"#
    );

    #[test]
    fn nested_asterisks() {
        verifies!(
            r#"
To nest an item, just add another asterisk (`+*+`) to the marker.
Continue doing this for each subsequent level.

----
include::example$unordered.adoc[tag=nest]
----

.Rendered nested, unordered list
====
include::example$unordered.adoc[tag=nest]
====

"#
        );

        let doc = Parser::default().parse(".Possible DefOps manual locations\n* West wood maze\n** Maze heart\n*** Reflection pool\n** Secret exit\n* Untracked file in git repository");

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
                blocks: &[Block::List(ListBlock {
                    type_: ListType::Unordered,
                    items: &[
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Asterisks(Span {
                                data: "*",
                                line: 2,
                                col: 1,
                                offset: 34,
                            },),
                            blocks: &[
                                Block::Simple(SimpleBlock {
                                    content: Content {
                                        original: Span {
                                            data: "West wood maze",
                                            line: 2,
                                            col: 3,
                                            offset: 36,
                                        },
                                        rendered: "West wood maze",
                                    },
                                    source: Span {
                                        data: "West wood maze",
                                        line: 2,
                                        col: 3,
                                        offset: 36,
                                    },
                                    style: SimpleBlockStyle::Paragraph,
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                                Block::List(ListBlock {
                                    type_: ListType::Unordered,
                                    items: &[
                                        Block::ListItem(ListItem {
                                            marker: ListItemMarker::Asterisks(Span {
                                                data: "**",
                                                line: 3,
                                                col: 1,
                                                offset: 51,
                                            },),
                                            blocks: &[
                                                Block::Simple(SimpleBlock {
                                                    content: Content {
                                                        original: Span {
                                                            data: "Maze heart",
                                                            line: 3,
                                                            col: 4,
                                                            offset: 54,
                                                        },
                                                        rendered: "Maze heart",
                                                    },
                                                    source: Span {
                                                        data: "Maze heart",
                                                        line: 3,
                                                        col: 4,
                                                        offset: 54,
                                                    },
                                                    style: SimpleBlockStyle::Paragraph,
                                                    title_source: None,
                                                    title: None,
                                                    anchor: None,
                                                    anchor_reftext: None,
                                                    attrlist: None,
                                                },),
                                                Block::List(ListBlock {
                                                    type_: ListType::Unordered,
                                                    items: &[Block::ListItem(ListItem {
                                                        marker: ListItemMarker::Asterisks(Span {
                                                            data: "***",
                                                            line: 4,
                                                            col: 1,
                                                            offset: 65,
                                                        },),
                                                        blocks: &[Block::Simple(SimpleBlock {
                                                            content: Content {
                                                                original: Span {
                                                                    data: "Reflection pool",
                                                                    line: 4,
                                                                    col: 5,
                                                                    offset: 69,
                                                                },
                                                                rendered: "Reflection pool",
                                                            },
                                                            source: Span {
                                                                data: "Reflection pool",
                                                                line: 4,
                                                                col: 5,
                                                                offset: 69,
                                                            },
                                                            style: SimpleBlockStyle::Paragraph,
                                                            title_source: None,
                                                            title: None,
                                                            anchor: None,
                                                            anchor_reftext: None,
                                                            attrlist: None,
                                                        },),],
                                                        source: Span {
                                                            data: "*** Reflection pool",
                                                            line: 4,
                                                            col: 1,
                                                            offset: 65,
                                                        },
                                                        anchor: None,
                                                        anchor_reftext: None,
                                                        attrlist: None,
                                                    },),],
                                                    source: Span {
                                                        data: "*** Reflection pool",
                                                        line: 4,
                                                        col: 1,
                                                        offset: 65,
                                                    },
                                                    title_source: None,
                                                    title: None,
                                                    anchor: None,
                                                    anchor_reftext: None,
                                                    attrlist: None,
                                                },),
                                            ],
                                            source: Span {
                                                data: "** Maze heart\n*** Reflection pool",
                                                line: 3,
                                                col: 1,
                                                offset: 51,
                                            },
                                            anchor: None,
                                            anchor_reftext: None,
                                            attrlist: None,
                                        },),
                                        Block::ListItem(ListItem {
                                            marker: ListItemMarker::Asterisks(Span {
                                                data: "**",
                                                line: 5,
                                                col: 1,
                                                offset: 85,
                                            },),
                                            blocks: &[Block::Simple(SimpleBlock {
                                                content: Content {
                                                    original: Span {
                                                        data: "Secret exit",
                                                        line: 5,
                                                        col: 4,
                                                        offset: 88,
                                                    },
                                                    rendered: "Secret exit",
                                                },
                                                source: Span {
                                                    data: "Secret exit",
                                                    line: 5,
                                                    col: 4,
                                                    offset: 88,
                                                },
                                                style: SimpleBlockStyle::Paragraph,
                                                title_source: None,
                                                title: None,
                                                anchor: None,
                                                anchor_reftext: None,
                                                attrlist: None,
                                            },),],
                                            source: Span {
                                                data: "** Secret exit",
                                                line: 5,
                                                col: 1,
                                                offset: 85,
                                            },
                                            anchor: None,
                                            anchor_reftext: None,
                                            attrlist: None,
                                        },),
                                    ],
                                    source: Span {
                                        data: "** Maze heart\n*** Reflection pool\n** Secret exit",
                                        line: 3,
                                        col: 1,
                                        offset: 51,
                                    },
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                            ],
                            source: Span {
                                data: "* West wood maze\n** Maze heart\n*** Reflection pool\n** Secret exit",
                                line: 2,
                                col: 1,
                                offset: 34,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Asterisks(Span {
                                data: "*",
                                line: 6,
                                col: 1,
                                offset: 100,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Untracked file in git repository",
                                        line: 6,
                                        col: 3,
                                        offset: 102,
                                    },
                                    rendered: "Untracked file in git repository",
                                },
                                source: Span {
                                    data: "Untracked file in git repository",
                                    line: 6,
                                    col: 3,
                                    offset: 102,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: "* Untracked file in git repository",
                                line: 6,
                                col: 1,
                                offset: 100,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                    ],
                    source: Span {
                        data: ".Possible DefOps manual locations\n* West wood maze\n** Maze heart\n*** Reflection pool\n** Secret exit\n* Untracked file in git repository",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: Some(Span {
                        data: "Possible DefOps manual locations",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },),
                    title: Some("Possible DefOps manual locations",),
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: ".Possible DefOps manual locations\n* West wood maze\n** Maze heart\n*** Reflection pool\n** Secret exit\n* Untracked file in git repository",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([]),
                    reftext_to_id: HashMap::from([]),
                },
            }
        );
    }

    #[test]
    fn indentation_not_significant() {
        verifies!(
            r#"
If you prefer, you can indent the marker an arbitrary number of spaces from the left margin.
The indentation is not significant and may aid in visualizing the nesting level.

"#
        );

        let doc =
            Parser::default().parse("* Edgar Allan Poe\n * Sheri S. Tepper\n     * Bill Bryson");

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
                blocks: &[Block::List(ListBlock {
                    type_: ListType::Unordered,
                    items: &[
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Asterisks(Span {
                                data: "*",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Edgar Allan Poe",
                                        line: 1,
                                        col: 3,
                                        offset: 2,
                                    },
                                    rendered: "Edgar Allan Poe",
                                },
                                source: Span {
                                    data: "Edgar Allan Poe",
                                    line: 1,
                                    col: 3,
                                    offset: 2,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: "* Edgar Allan Poe",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Asterisks(Span {
                                data: "*",
                                line: 2,
                                col: 2,
                                offset: 19,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Sheri S. Tepper",
                                        line: 2,
                                        col: 4,
                                        offset: 21,
                                    },
                                    rendered: "Sheri S. Tepper",
                                },
                                source: Span {
                                    data: "Sheri S. Tepper",
                                    line: 2,
                                    col: 4,
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
                                data: " * Sheri S. Tepper",
                                line: 2,
                                col: 1,
                                offset: 18,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Asterisks(Span {
                                data: "*",
                                line: 3,
                                col: 6,
                                offset: 42,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Bill Bryson",
                                        line: 3,
                                        col: 8,
                                        offset: 44,
                                    },
                                    rendered: "Bill Bryson",
                                },
                                source: Span {
                                    data: "Bill Bryson",
                                    line: 3,
                                    col: 8,
                                    offset: 44,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: "     * Bill Bryson",
                                line: 3,
                                col: 1,
                                offset: 37,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                    ],
                    source: Span {
                        data: "* Edgar Allan Poe\n * Sheri S. Tepper\n     * Bill Bryson",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: "* Edgar Allan Poe\n * Sheri S. Tepper\n     * Bill Bryson",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([]),
                    reftext_to_id: HashMap::from([]),
                },
            }
        );
    }

    #[test]
    fn nest_to_any_depth() {
        verifies!(
            r#"
You can nest unordered lists to any depth.
Keep in mind, however, that some interfaces will begin flattening lists after a certain depth.
For instance, GitHub starts flattening list after 10 levels of nesting.

----
include::example$unordered.adoc[tag=max]
----

[#ex-deep]
.Unordered lists can be nested to any depth
====
include::example$unordered.adoc[tag=max]
====

"#
        );

        let doc =
            Parser::default().parse("* Level 1 list item\n** Level 2 list item\n*** Level 3 list item\n**** Level 4 list item\n***** Level 5 list item\n****** etc.\n* Level 1 list item");

        assert_eq!(doc, Document {
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
                Block::List(
                    ListBlock {
                        type_: ListType::Unordered,
                        items: &[
                            Block::ListItem(
                                ListItem {
                                    marker: ListItemMarker::Asterisks(
                                        Span {
                                            data: "*",
                                            line: 1,
                                            col: 1,
                                            offset: 0,
                                        },
                                    ),
                                    blocks: &[
                                        Block::Simple(
                                            SimpleBlock {
                                                content: Content {
                                                    original: Span {
                                                        data: "Level 1 list item",
                                                        line: 1,
                                                        col: 3,
                                                        offset: 2,
                                                    },
                                                    rendered: "Level 1 list item",
                                                },
                                                source: Span {
                                                    data: "Level 1 list item",
                                                    line: 1,
                                                    col: 3,
                                                    offset: 2,
                                                },
                                                style: SimpleBlockStyle::Paragraph,
                                                title_source: None,
                                                title: None,
                                                anchor: None,
                                                anchor_reftext: None,
                                                attrlist: None,
                                            },
                                        ),
                                        Block::List(
                                            ListBlock {
                                                type_: ListType::Unordered,
                                                items: &[
                                                    Block::ListItem(
                                                        ListItem {
                                                            marker: ListItemMarker::Asterisks(
                                                                Span {
                                                                    data: "**",
                                                                    line: 2,
                                                                    col: 1,
                                                                    offset: 20,
                                                                },
                                                            ),
                                                            blocks: &[
                                                                Block::Simple(
                                                                    SimpleBlock {
                                                                        content: Content {
                                                                            original: Span {
                                                                                data: "Level 2 list item",
                                                                                line: 2,
                                                                                col: 4,
                                                                                offset: 23,
                                                                            },
                                                                            rendered: "Level 2 list item",
                                                                        },
                                                                        source: Span {
                                                                            data: "Level 2 list item",
                                                                            line: 2,
                                                                            col: 4,
                                                                            offset: 23,
                                                                        },
                                                                        style: SimpleBlockStyle::Paragraph,
                                                                        title_source: None,
                                                                        title: None,
                                                                        anchor: None,
                                                                        anchor_reftext: None,
                                                                        attrlist: None,
                                                                    },
                                                                ),
                                                                Block::List(
                                                                    ListBlock {
                                                                        type_: ListType::Unordered,
                                                                        items: &[
                                                                            Block::ListItem(
                                                                                ListItem {
                                                                                    marker: ListItemMarker::Asterisks(
                                                                                        Span {
                                                                                            data: "***",
                                                                                            line: 3,
                                                                                            col: 1,
                                                                                            offset: 41,
                                                                                        },
                                                                                    ),
                                                                                    blocks: &[
                                                                                        Block::Simple(
                                                                                            SimpleBlock {
                                                                                                content: Content {
                                                                                                    original: Span {
                                                                                                        data: "Level 3 list item",
                                                                                                        line: 3,
                                                                                                        col: 5,
                                                                                                        offset: 45,
                                                                                                    },
                                                                                                    rendered: "Level 3 list item",
                                                                                                },
                                                                                                source: Span {
                                                                                                    data: "Level 3 list item",
                                                                                                    line: 3,
                                                                                                    col: 5,
                                                                                                    offset: 45,
                                                                                                },
                                                                                                style: SimpleBlockStyle::Paragraph,
                                                                                                title_source: None,
                                                                                                title: None,
                                                                                                anchor: None,
                                                                                                anchor_reftext: None,
                                                                                                attrlist: None,
                                                                                            },
                                                                                        ),
                                                                                        Block::List(
                                                                                            ListBlock {
                                                                                                type_: ListType::Unordered,
                                                                                                items: &[
                                                                                                    Block::ListItem(
                                                                                                        ListItem {
                                                                                                            marker: ListItemMarker::Asterisks(
                                                                                                                Span {
                                                                                                                    data: "****",
                                                                                                                    line: 4,
                                                                                                                    col: 1,
                                                                                                                    offset: 63,
                                                                                                                },
                                                                                                            ),
                                                                                                            blocks: &[
                                                                                                                Block::Simple(
                                                                                                                    SimpleBlock {
                                                                                                                        content: Content {
                                                                                                                            original: Span {
                                                                                                                                data: "Level 4 list item",
                                                                                                                                line: 4,
                                                                                                                                col: 6,
                                                                                                                                offset: 68,
                                                                                                                            },
                                                                                                                            rendered: "Level 4 list item",
                                                                                                                        },
                                                                                                                        source: Span {
                                                                                                                            data: "Level 4 list item",
                                                                                                                            line: 4,
                                                                                                                            col: 6,
                                                                                                                            offset: 68,
                                                                                                                        },
                                                                                                                        style: SimpleBlockStyle::Paragraph,
                                                                                                                        title_source: None,
                                                                                                                        title: None,
                                                                                                                        anchor: None,
                                                                                                                        anchor_reftext: None,
                                                                                                                        attrlist: None,
                                                                                                                    },
                                                                                                                ),
                                                                                                                Block::List(
                                                                                                                    ListBlock {
                                                                                                                        type_: ListType::Unordered,
                                                                                                                        items: &[
                                                                                                                            Block::ListItem(
                                                                                                                                ListItem {
                                                                                                                                    marker: ListItemMarker::Asterisks(
                                                                                                                                        Span {
                                                                                                                                            data: "*****",
                                                                                                                                            line: 5,
                                                                                                                                            col: 1,
                                                                                                                                            offset: 86,
                                                                                                                                        },
                                                                                                                                    ),
                                                                                                                                    blocks: &[
                                                                                                                                        Block::Simple(
                                                                                                                                            SimpleBlock {
                                                                                                                                                content: Content {
                                                                                                                                                    original: Span {
                                                                                                                                                        data: "Level 5 list item",
                                                                                                                                                        line: 5,
                                                                                                                                                        col: 7,
                                                                                                                                                        offset: 92,
                                                                                                                                                    },
                                                                                                                                                    rendered: "Level 5 list item",
                                                                                                                                                },
                                                                                                                                                source: Span {
                                                                                                                                                    data: "Level 5 list item",
                                                                                                                                                    line: 5,
                                                                                                                                                    col: 7,
                                                                                                                                                    offset: 92,
                                                                                                                                                },
                                                                                                                                                style: SimpleBlockStyle::Paragraph,
                                                                                                                                                title_source: None,
                                                                                                                                                title: None,
                                                                                                                                                anchor: None,
                                                                                                                                                anchor_reftext: None,
                                                                                                                                                attrlist: None,
                                                                                                                                            },
                                                                                                                                        ),
                                                                                                                                        Block::List(
                                                                                                                                            ListBlock {
                                                                                                                                                type_: ListType::Unordered,
                                                                                                                                                items: &[
                                                                                                                                                    Block::ListItem(
                                                                                                                                                        ListItem {
                                                                                                                                                            marker: ListItemMarker::Asterisks(
                                                                                                                                                                Span {
                                                                                                                                                                    data: "******",
                                                                                                                                                                    line: 6,
                                                                                                                                                                    col: 1,
                                                                                                                                                                    offset: 110,
                                                                                                                                                                },
                                                                                                                                                            ),
                                                                                                                                                            blocks: &[
                                                                                                                                                                Block::Simple(
                                                                                                                                                                    SimpleBlock {
                                                                                                                                                                        content: Content {
                                                                                                                                                                            original: Span {
                                                                                                                                                                                data: "etc.",
                                                                                                                                                                                line: 6,
                                                                                                                                                                                col: 8,
                                                                                                                                                                                offset: 117,
                                                                                                                                                                            },
                                                                                                                                                                            rendered: "etc.",
                                                                                                                                                                        },
                                                                                                                                                                        source: Span {
                                                                                                                                                                            data: "etc.",
                                                                                                                                                                            line: 6,
                                                                                                                                                                            col: 8,
                                                                                                                                                                            offset: 117,
                                                                                                                                                                        },
                                                                                                                                                                        style: SimpleBlockStyle::Paragraph,
                                                                                                                                                                        title_source: None,
                                                                                                                                                                        title: None,
                                                                                                                                                                        anchor: None,
                                                                                                                                                                        anchor_reftext: None,
                                                                                                                                                                        attrlist: None,
                                                                                                                                                                    },
                                                                                                                                                                ),
                                                                                                                                                            ],
                                                                                                                                                            source: Span {
                                                                                                                                                                data: "****** etc.",
                                                                                                                                                                line: 6,
                                                                                                                                                                col: 1,
                                                                                                                                                                offset: 110,
                                                                                                                                                            },
                                                                                                                                                            anchor: None,
                                                                                                                                                            anchor_reftext: None,
                                                                                                                                                            attrlist: None,
                                                                                                                                                        },
                                                                                                                                                    ),
                                                                                                                                                ],
                                                                                                                                                source: Span {
                                                                                                                                                    data: "****** etc.",
                                                                                                                                                    line: 6,
                                                                                                                                                    col: 1,
                                                                                                                                                    offset: 110,
                                                                                                                                                },
                                                                                                                                                title_source: None,
                                                                                                                                                title: None,
                                                                                                                                                anchor: None,
                                                                                                                                                anchor_reftext: None,
                                                                                                                                                attrlist: None,
                                                                                                                                            },
                                                                                                                                        ),
                                                                                                                                    ],
                                                                                                                                    source: Span {
                                                                                                                                        data: "***** Level 5 list item\n****** etc.",
                                                                                                                                        line: 5,
                                                                                                                                        col: 1,
                                                                                                                                        offset: 86,
                                                                                                                                    },
                                                                                                                                    anchor: None,
                                                                                                                                    anchor_reftext: None,
                                                                                                                                    attrlist: None,
                                                                                                                                },
                                                                                                                            ),
                                                                                                                        ],
                                                                                                                        source: Span {
                                                                                                                            data: "***** Level 5 list item\n****** etc.",
                                                                                                                            line: 5,
                                                                                                                            col: 1,
                                                                                                                            offset: 86,
                                                                                                                        },
                                                                                                                        title_source: None,
                                                                                                                        title: None,
                                                                                                                        anchor: None,
                                                                                                                        anchor_reftext: None,
                                                                                                                        attrlist: None,
                                                                                                                    },
                                                                                                                ),
                                                                                                            ],
                                                                                                            source: Span {
                                                                                                                data: "**** Level 4 list item\n***** Level 5 list item\n****** etc.",
                                                                                                                line: 4,
                                                                                                                col: 1,
                                                                                                                offset: 63,
                                                                                                            },
                                                                                                            anchor: None,
                                                                                                            anchor_reftext: None,
                                                                                                            attrlist: None,
                                                                                                        },
                                                                                                    ),
                                                                                                ],
                                                                                                source: Span {
                                                                                                    data: "**** Level 4 list item\n***** Level 5 list item\n****** etc.",
                                                                                                    line: 4,
                                                                                                    col: 1,
                                                                                                    offset: 63,
                                                                                                },
                                                                                                title_source: None,
                                                                                                title: None,
                                                                                                anchor: None,
                                                                                                anchor_reftext: None,
                                                                                                attrlist: None,
                                                                                            },
                                                                                        ),
                                                                                    ],
                                                                                    source: Span {
                                                                                        data: "*** Level 3 list item\n**** Level 4 list item\n***** Level 5 list item\n****** etc.",
                                                                                        line: 3,
                                                                                        col: 1,
                                                                                        offset: 41,
                                                                                    },
                                                                                    anchor: None,
                                                                                    anchor_reftext: None,
                                                                                    attrlist: None,
                                                                                },
                                                                            ),
                                                                        ],
                                                                        source: Span {
                                                                            data: "*** Level 3 list item\n**** Level 4 list item\n***** Level 5 list item\n****** etc.",
                                                                            line: 3,
                                                                            col: 1,
                                                                            offset: 41,
                                                                        },
                                                                        title_source: None,
                                                                        title: None,
                                                                        anchor: None,
                                                                        anchor_reftext: None,
                                                                        attrlist: None,
                                                                    },
                                                                ),
                                                            ],
                                                            source: Span {
                                                                data: "** Level 2 list item\n*** Level 3 list item\n**** Level 4 list item\n***** Level 5 list item\n****** etc.",
                                                                line: 2,
                                                                col: 1,
                                                                offset: 20,
                                                            },
                                                            anchor: None,
                                                            anchor_reftext: None,
                                                            attrlist: None,
                                                        },
                                                    ),
                                                ],
                                                source: Span {
                                                    data: "** Level 2 list item\n*** Level 3 list item\n**** Level 4 list item\n***** Level 5 list item\n****** etc.",
                                                    line: 2,
                                                    col: 1,
                                                    offset: 20,
                                                },
                                                title_source: None,
                                                title: None,
                                                anchor: None,
                                                anchor_reftext: None,
                                                attrlist: None,
                                            },
                                        ),
                                    ],
                                    source: Span {
                                        data: "* Level 1 list item\n** Level 2 list item\n*** Level 3 list item\n**** Level 4 list item\n***** Level 5 list item\n****** etc.",
                                        line: 1,
                                        col: 1,
                                        offset: 0,
                                    },
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },
                            ),
                            Block::ListItem(
                                ListItem {
                                    marker: ListItemMarker::Asterisks(
                                        Span {
                                            data: "*",
                                            line: 7,
                                            col: 1,
                                            offset: 122,
                                        },
                                    ),
                                    blocks: &[
                                        Block::Simple(
                                            SimpleBlock {
                                                content: Content {
                                                    original: Span {
                                                        data: "Level 1 list item",
                                                        line: 7,
                                                        col: 3,
                                                        offset: 124,
                                                    },
                                                    rendered: "Level 1 list item",
                                                },
                                                source: Span {
                                                    data: "Level 1 list item",
                                                    line: 7,
                                                    col: 3,
                                                    offset: 124,
                                                },
                                                style: SimpleBlockStyle::Paragraph,
                                                title_source: None,
                                                title: None,
                                                anchor: None,
                                                anchor_reftext: None,
                                                attrlist: None,
                                            },
                                        ),
                                    ],
                                    source: Span {
                                        data: "* Level 1 list item",
                                        line: 7,
                                        col: 1,
                                        offset: 122,
                                    },
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },
                            ),
                        ],
                        source: Span {
                            data: "* Level 1 list item\n** Level 2 list item\n*** Level 3 list item\n**** Level 4 list item\n***** Level 5 list item\n****** etc.\n* Level 1 list item",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },
                ),
            ],
            source: Span {
                data: "* Level 1 list item\n** Level 2 list item\n*** Level 3 list item\n**** Level 4 list item\n***** Level 5 list item\n****** etc.\n* Level 1 list item",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog {
                refs: HashMap::from([]),
                reftext_to_id: HashMap::from([]),
            },
        });
    }

    #[test]
    fn determining_list_depth() {
        verifies!(
            r#"
=== Determining list depth

While it would seem as though the number of asterisks represents the nesting level, that's not how depth is determined.
A new level is created for each unique marker encountered.
For example, you can create a second level using the hyphen marker instead of two asterisks.

.Using hyphen to mark the second level is not recommended
----
include::example$unordered.adoc[tag=nest-alt]
----

However, it's much more intuitive to follow the convention that the marker length (i.e., number of asterisks) equals the level of nesting.
The hyphen should only be used as the marker for the first level.

*marker length = level of nesting*

After all, we're shooting for plain text markup that is readable _as is_.

"#
        );

        let doc = Parser::default()
            .parse("* Level 1 list item\n- Level 2 list item\n* Level 1 list item");

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
                blocks: &[Block::List(ListBlock {
                    type_: ListType::Unordered,
                    items: &[
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Asterisks(Span {
                                data: "*",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },),
                            blocks: &[
                                Block::Simple(SimpleBlock {
                                    content: Content {
                                        original: Span {
                                            data: "Level 1 list item",
                                            line: 1,
                                            col: 3,
                                            offset: 2,
                                        },
                                        rendered: "Level 1 list item",
                                    },
                                    source: Span {
                                        data: "Level 1 list item",
                                        line: 1,
                                        col: 3,
                                        offset: 2,
                                    },
                                    style: SimpleBlockStyle::Paragraph,
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                                Block::List(ListBlock {
                                    type_: ListType::Unordered,
                                    items: &[Block::ListItem(ListItem {
                                        marker: ListItemMarker::Hyphen(Span {
                                            data: "-",
                                            line: 2,
                                            col: 1,
                                            offset: 20,
                                        },),
                                        blocks: &[Block::Simple(SimpleBlock {
                                            content: Content {
                                                original: Span {
                                                    data: "Level 2 list item",
                                                    line: 2,
                                                    col: 3,
                                                    offset: 22,
                                                },
                                                rendered: "Level 2 list item",
                                            },
                                            source: Span {
                                                data: "Level 2 list item",
                                                line: 2,
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
                                            data: "- Level 2 list item",
                                            line: 2,
                                            col: 1,
                                            offset: 20,
                                        },
                                        anchor: None,
                                        anchor_reftext: None,
                                        attrlist: None,
                                    },),],
                                    source: Span {
                                        data: "- Level 2 list item",
                                        line: 2,
                                        col: 1,
                                        offset: 20,
                                    },
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                            ],
                            source: Span {
                                data: "* Level 1 list item\n- Level 2 list item",
                                line: 1,
                                col: 1,
                                offset: 0,
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
                                offset: 40,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Level 1 list item",
                                        line: 3,
                                        col: 3,
                                        offset: 42,
                                    },
                                    rendered: "Level 1 list item",
                                },
                                source: Span {
                                    data: "Level 1 list item",
                                    line: 3,
                                    col: 3,
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
                                data: "* Level 1 list item",
                                line: 3,
                                col: 1,
                                offset: 40,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                    ],
                    source: Span {
                        data: "* Level 1 list item\n- Level 2 list item\n* Level 1 list item",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: "* Level 1 list item\n- Level 2 list item\n* Level 1 list item",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([]),
                    reftext_to_id: HashMap::from([]),
                },
            }
        );
    }

    // Considering the Markers section as non-normative because this crate doesn't
    // say anything about how rendering is performed.
    non_normative!(
        r#"                
[#markers]
== Markers

When rendered, an unordered list item is designated by a leading marker (bullet) (not to be confused with the marker used to define the list).
This marker can be controlled using the list style.
If no marker is specified, a default marker will be selected by the renderer.

=== Default markers

By default, AsciiDoc assumes that the first three levels of an unordered list will be styled using the markers disc, circle, and squared when rendered.
Consider the following list:

----
include::example$unordered.adoc[tag=markers]
----

.Default alternating markers for nested lists
====
include::example$unordered.adoc[tag=markers]
====

Observe that the marker for the first level is a disc (filled circle), the second level is a circle (outline), and the third level is a square (filled).
The AsciiDoc processor does not specify these markers explicitly in the model or converted output.
Rather, these defaults are added by the renderer (e.g., CSS), adhering to a convention established by HTML.

Beyond the third level of nesting, the marker choice is not specified.
Typically, the renderer will continue to use the square marker, as shown in <<ex-deep,an earlier example>>.

=== Custom markers

AsciiDoc offers numerous marker styles for lists.
The list marker can be specified using the list's block style.

The unordered list marker can be set using any of the following block styles:

* square
* circle
* disc
* none or no-bullet (indented, but no bullet)
* unstyled (no indentation or bullet) (not supported in DocBook output)

NOTE: These styles are supported by the default Asciidoctor stylesheet.

When present, the style name is assigned to the unordered list element as follows:

For HTML:: the style name is assigned to the `class` attribute on the `<ul>` element.

For DocBook:: the style name is assigned to the `mark` attribute on the `<itemizedlist>` element.

Here's an unordered list that has square markers:

----
include::example$unordered.adoc[tag=square]
----

.A list with square markers
====
include::example$unordered.adoc[tag=square]
====

Once the list style is set, that style is used for all nested lists until it is set again.
The assumption is that it's no longer possible to infer the alternation, so it stops.
The inherited style is not specified in the model, but rather applied by the renderer (e.g., CSS).
For example, if we set the list style to circle on the top-level list, it will be used for all levels.

----
include::example$unordered.adoc[tag=marker-lock]
----

.The list style is inherited once set
====
include::example$unordered.adoc[tag=marker-lock]
====

The inherited style can be set or reset at any level.

----
include::example$unordered.adoc[tag=marker-override]
----

.The list style can be reset
====
include::example$unordered.adoc[tag=marker-override]
====
"#
    );
}
