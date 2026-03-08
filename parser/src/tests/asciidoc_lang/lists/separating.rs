use std::collections::HashMap;

use crate::{
    Parser,
    blocks::{ListType, SimpleBlockStyle},
    tests::prelude::*,
};

track_file!("docs/modules/lists/pages/separating.adoc");

non_normative!(
    r#"
= Separating Lists

In AsciiDoc, lists items have natural affinity towards one another.
If adjacent lines start with the same list marker, they are joined together into the same list, even if separated by empty lines.
If the adjacent line starts with a different list marker, even if offset by empty lines, it will be placed into a nested list.

These rules make it easier to keep list items together in a single list.
However, they can present a challenge if what you want is to create separate lists.
Fortunately, there are ways to force a change to this behavior.
The techniques described on this page work for all list types.

"#
);

#[test]
fn using_line_comment() {
    non_normative!(
        r#"
== Using a line comment

"#
    );

    verifies!(
        r#"
To force lists apart, you can insert a line comment (i.e., `//`) surrounded by empty lines on either side between the two lists.

Here's an example that shows where to place the line comment to separate two adjacent unordered lists.
The `-` following the line comment prefix is a hint to authors to indicate that the comment line serves as an "`end of list`" marker:

----
include::example$unordered.adoc[tag=divide]
----

This technique works for separating any type of list.

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

#[test]
fn using_block_attribute_line() {
    non_normative!(
        r#"
== Using a block attribute line

"#
    );

    verifies!(
        r#"
Another way to start a new list is to place a block attribute line (even an empty one) above the second list, offset by an empty line.

Here's an example that shows where to place the block attribute line to separate unordered and ordered lists that are adjacent.

----
include::example$unordered.adoc[tag=divide-alt]
----

The preceding empty line is important.
If that were not present, the ordered list would still be nested inside the ordered list.
If the second list requires block attributes, you can add them to the block attribute line.

This technique works for separating any type of list.
"#
    );

    let doc = Parser::default().parse("* Apples\n* Oranges\n\n[]\n. Wash\n. Slice");

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
                Block::List(ListBlock {
                    type_: ListType::Ordered,
                    items: &[
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Dots(Span {
                                data: ".",
                                line: 5,
                                col: 1,
                                offset: 23,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Wash",
                                        line: 5,
                                        col: 3,
                                        offset: 25,
                                    },
                                    rendered: "Wash",
                                },
                                source: Span {
                                    data: "Wash",
                                    line: 5,
                                    col: 3,
                                    offset: 25,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: ". Wash",
                                line: 5,
                                col: 1,
                                offset: 23,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Dots(Span {
                                data: ".",
                                line: 6,
                                col: 1,
                                offset: 30,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Slice",
                                        line: 6,
                                        col: 3,
                                        offset: 32,
                                    },
                                    rendered: "Slice",
                                },
                                source: Span {
                                    data: "Slice",
                                    line: 6,
                                    col: 3,
                                    offset: 32,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: ". Slice",
                                line: 6,
                                col: 1,
                                offset: 30,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                    ],
                    source: Span {
                        data: "[]\n. Wash\n. Slice",
                        line: 4,
                        col: 1,
                        offset: 20,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[],
                        anchor: None,
                        source: Span {
                            data: "",
                            line: 4,
                            col: 2,
                            offset: 21,
                        },
                    },),
                },),
            ],
            source: Span {
                data: "* Apples\n* Oranges\n\n[]\n. Wash\n. Slice",
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
