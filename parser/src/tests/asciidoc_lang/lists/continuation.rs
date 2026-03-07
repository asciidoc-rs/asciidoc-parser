use crate::tests::prelude::*;

track_file!("docs/modules/lists/pages/continuation.adoc");

non_normative!(
    r#"
= Compound List Items

This page covers how to create lists that have compound list items.

A [.term]*compound list item* is a list item that has blocks attached to it, including paragraphs, which follow the (optionally empty) principal text.
In other words, the list item contains block content.
This scenario is different from a list item whose principal text merely spans multiple lines, a distinction which is further explained on this page.
The page goes on to explain how to attach a block to a list item in an ancestor list.

In additional to unordered and ordered lists, callout and description lists also support compound list items.
On this page, the term list item refers to any list item in an unordered, ordered, callout, and description list.
For a description list, it refers specifically to the description of the list item (not the list item term).

The main focus of the syntax covered on this page is to keep the list continuous (i.e., to prevent the list from breaking).

"#
);

mod multiline_principal_text {
    use std::collections::HashMap;

    use pretty_assertions_sorted::assert_eq;

    use crate::{
        Parser,
        blocks::{ListType, SimpleBlockStyle},
        tests::prelude::*,
    };

    non_normative!(
        r#"
== Multiline principal text

"#
    );

    #[test]
    fn basic() {
        verifies!(
            r#"
As with regular paragraph text, the principal text in a list item can span any number of lines as long as those lines are contiguous (i.e., adjacent with no empty lines).
Multiple lines are combined into a single paragraph and wrap as regular paragraph text.
"#
        );

        let doc = Parser::default().parse("* Text of a list item.\nMore text goes here.");

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
                    items: &[Block::ListItem(ListItem {
                        marker: ListItemMarker::Asterisks(Span {
                            data: "*",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },),
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "Text of a list item.\nMore text goes here.",
                                    line: 1,
                                    col: 3,
                                    offset: 2,
                                },
                                rendered: "Text of a list item.\nMore text goes here.",
                            },
                            source: Span {
                                data: "Text of a list item.\nMore text goes here.",
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
                            data: "* Text of a list item.\nMore text goes here.",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "* Text of a list item.\nMore text goes here.",
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
                    data: "* Text of a list item.\nMore text goes here.",
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
    fn indented() {
        verifies!(
            r#"
This behavior holds even if the lines are indented, as shown in the third bullet in this example:

----
include::example$complex.adoc[tag=indent]
----

====
include::example$complex.adoc[tag=indent]
====

"#
        );

        let doc = Parser::default().parse("* The document header in AsciiDoc is optional.\nIf present, it must start with a document title.\n\n* Optional author and revision information lines\nimmediately follow the document title.\n\n* The document header must be separated from\n  the remainder of the document by one or more\n  empty lines and it cannot contain empty lines.");

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
                                        data: "The document header in AsciiDoc is optional.\nIf present, it must start with a document title.",
                                        line: 1,
                                        col: 3,
                                        offset: 2,
                                    },
                                    rendered: "The document header in AsciiDoc is optional.\nIf present, it must start with a document title.",
                                },
                                source: Span {
                                    data: "The document header in AsciiDoc is optional.\nIf present, it must start with a document title.",
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
                                data: "* The document header in AsciiDoc is optional.\nIf present, it must start with a document title.",
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
                                line: 4,
                                col: 1,
                                offset: 97,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Optional author and revision information lines\nimmediately follow the document title.",
                                        line: 4,
                                        col: 3,
                                        offset: 99,
                                    },
                                    rendered: "Optional author and revision information lines\nimmediately follow the document title.",
                                },
                                source: Span {
                                    data: "Optional author and revision information lines\nimmediately follow the document title.",
                                    line: 4,
                                    col: 3,
                                    offset: 99,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: "* Optional author and revision information lines\nimmediately follow the document title.",
                                line: 4,
                                col: 1,
                                offset: 97,
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
                                offset: 186,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "The document header must be separated from\n  the remainder of the document by one or more\n  empty lines and it cannot contain empty lines.",
                                        line: 7,
                                        col: 3,
                                        offset: 188,
                                    },
                                    rendered: "The document header must be separated from\nthe remainder of the document by one or more\nempty lines and it cannot contain empty lines.",
                                },
                                source: Span {
                                    data: "The document header must be separated from\n  the remainder of the document by one or more\n  empty lines and it cannot contain empty lines.",
                                    line: 7,
                                    col: 3,
                                    offset: 188,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: "* The document header must be separated from\n  the remainder of the document by one or more\n  empty lines and it cannot contain empty lines.",
                                line: 7,
                                col: 1,
                                offset: 186,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                    ],
                    source: Span {
                        data: "* The document header in AsciiDoc is optional.\nIf present, it must start with a document title.\n\n* Optional author and revision information lines\nimmediately follow the document title.\n\n* The document header must be separated from\n  the remainder of the document by one or more\n  empty lines and it cannot contain empty lines.",
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
                    data: "* The document header in AsciiDoc is optional.\nIf present, it must start with a document title.\n\n* Optional author and revision information lines\nimmediately follow the document title.\n\n* The document header must be separated from\n  the remainder of the document by one or more\n  empty lines and it cannot contain empty lines.",
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
TIP: When list items contain more than one line of text, leave an empty line between items to make the list easier to read while working in the code.
An empty line between two list items will not break the list.

"#
    );

    mod empty_lines {
        use std::collections::HashMap;

        use pretty_assertions_sorted::assert_eq;

        use crate::{
            Parser,
            blocks::{ListType, SimpleBlockStyle},
            tests::prelude::*,
        };

        non_normative!(
            r#"
=== Empty lines in a list

"#
        );

        #[test]
        fn empty_lines_between_items() {
            verifies!(
                r#"
Empty lines between two items in a list (ordered or unordered) will not break the list.
For ordered lists, this means the numbering will be continuous rather than restarting at 1.
(See xref:separating.adoc[] to learn how to force two adjacent lists apart).

"#
            );

            let doc = Parser::default().parse("1. Item 1\n\n2. Item 2\n\n\n3. Item 3");

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
                        type_: ListType::Ordered,
                        items: &[
                            Block::ListItem(ListItem {
                                marker: ListItemMarker::ArabicNumeral(Span {
                                    data: "1.",
                                    line: 1,
                                    col: 1,
                                    offset: 0,
                                },),
                                blocks: &[Block::Simple(SimpleBlock {
                                    content: Content {
                                        original: Span {
                                            data: "Item 1",
                                            line: 1,
                                            col: 4,
                                            offset: 3,
                                        },
                                        rendered: "Item 1",
                                    },
                                    source: Span {
                                        data: "Item 1",
                                        line: 1,
                                        col: 4,
                                        offset: 3,
                                    },
                                    style: SimpleBlockStyle::Paragraph,
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),],
                                source: Span {
                                    data: "1. Item 1",
                                    line: 1,
                                    col: 1,
                                    offset: 0,
                                },
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),
                            Block::ListItem(ListItem {
                                marker: ListItemMarker::ArabicNumeral(Span {
                                    data: "2.",
                                    line: 3,
                                    col: 1,
                                    offset: 11,
                                },),
                                blocks: &[Block::Simple(SimpleBlock {
                                    content: Content {
                                        original: Span {
                                            data: "Item 2",
                                            line: 3,
                                            col: 4,
                                            offset: 14,
                                        },
                                        rendered: "Item 2",
                                    },
                                    source: Span {
                                        data: "Item 2",
                                        line: 3,
                                        col: 4,
                                        offset: 14,
                                    },
                                    style: SimpleBlockStyle::Paragraph,
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),],
                                source: Span {
                                    data: "2. Item 2",
                                    line: 3,
                                    col: 1,
                                    offset: 11,
                                },
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),
                            Block::ListItem(ListItem {
                                marker: ListItemMarker::ArabicNumeral(Span {
                                    data: "3.",
                                    line: 6,
                                    col: 1,
                                    offset: 23,
                                },),
                                blocks: &[Block::Simple(SimpleBlock {
                                    content: Content {
                                        original: Span {
                                            data: "Item 3",
                                            line: 6,
                                            col: 4,
                                            offset: 26,
                                        },
                                        rendered: "Item 3",
                                    },
                                    source: Span {
                                        data: "Item 3",
                                        line: 6,
                                        col: 4,
                                        offset: 26,
                                    },
                                    style: SimpleBlockStyle::Paragraph,
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),],
                                source: Span {
                                    data: "3. Item 3",
                                    line: 6,
                                    col: 1,
                                    offset: 23,
                                },
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),
                        ],
                        source: Span {
                            data: "1. Item 1\n\n2. Item 2\n\n\n3. Item 3",
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
                        data: "1. Item 1\n\n2. Item 2\n\n\n3. Item 3",
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
        fn empty_line_new_block() {
            verifies!(
                r#"
If an empty line after a list item is followed by the start of a block, such as a paragraph or delimited block rather than another list item, the list will terminate at this point.
If this happens, you'll notice that a subsequent list item will be placed into a new list.
For ordered lists, that means the numbering will restart (at 1).

"#
            );
            let doc = Parser::default().parse("* Item 1\n\n* Item 2\n\nAlmost item 3");

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
                                                data: "Item 1",
                                                line: 1,
                                                col: 3,
                                                offset: 2,
                                            },
                                            rendered: "Item 1",
                                        },
                                        source: Span {
                                            data: "Item 1",
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
                                        data: "* Item 1",
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
                                        offset: 10,
                                    },),
                                    blocks: &[Block::Simple(SimpleBlock {
                                        content: Content {
                                            original: Span {
                                                data: "Item 2",
                                                line: 3,
                                                col: 3,
                                                offset: 12,
                                            },
                                            rendered: "Item 2",
                                        },
                                        source: Span {
                                            data: "Item 2",
                                            line: 3,
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
                                        data: "* Item 2",
                                        line: 3,
                                        col: 1,
                                        offset: 10,
                                    },
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                            ],
                            source: Span {
                                data: "* Item 1\n\n* Item 2",
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
                                    data: "Almost item 3",
                                    line: 5,
                                    col: 1,
                                    offset: 20,
                                },
                                rendered: "Almost item 3",
                            },
                            source: Span {
                                data: "Almost item 3",
                                line: 5,
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
                    ],
                    source: Span {
                        data: "* Item 1\n\n* Item 2\n\nAlmost item 3",
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
To keep the list continuous in those cases--such as when you're documenting complex steps in a procedure--you must use a <<list-continuation,list continuation>> to attach blocks to the list item.
For ordered lists, this will ensure that the numbering continues from one list item to the next rather than being reset.

"#
        );
    }
}

mod list_continuation {
    use std::collections::HashMap;

    use pretty_assertions_sorted::assert_eq;

    use crate::{
        Parser,
        blocks::{ContentModel, ListType, SimpleBlockStyle},
        content::SubstitutionGroup,
        tests::prelude::{inline_file_handler::InlineFileHandler, *},
    };

    non_normative!(
        r#"
[#list-continuation]
== Attach blocks using a list continuation

"#
    );

    #[test]
    fn basic() {
        verifies!(
            r#"
In addition to the principal text, a list item may contain block elements, including paragraphs, delimited blocks, and block macros.
To add block elements to a list item, you must "`attach`" them (in a series) using a list continuation.
This technique works for unordered and ordered lists as well as callout and description lists.

A [.term]*list continuation* is a `{plus}` symbol on a line by itself, immediately adjacent to the block being attached.
The attached block must be left-aligned, just like all blocks in AsciiDoc.

NOTE: A `{plus}` at the end of a line, rather than on a line by itself, is not a list continuation.
Instead, it creates a hard line break.

Here's an example of a list item that uses a list continuation:

----
include::example$complex.adoc[tag=cont]
----

====
include::example$complex.adoc[tag=cont]
====

"#
        );

        let doc = Parser::default().parse("* The header in AsciiDoc must start with a document title.\n+\nThe header is optional.");

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
                    items: &[Block::ListItem(ListItem {
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
                                        data: "The header in AsciiDoc must start with a document title.",
                                        line: 1,
                                        col: 3,
                                        offset: 2,
                                    },
                                    rendered: "The header in AsciiDoc must start with a document title.",
                                },
                                source: Span {
                                    data: "The header in AsciiDoc must start with a document title.",
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
                            Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "The header is optional.",
                                        line: 3,
                                        col: 1,
                                        offset: 61,
                                    },
                                    rendered: "The header is optional.",
                                },
                                source: Span {
                                    data: "The header is optional.",
                                    line: 3,
                                    col: 1,
                                    offset: 61,
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
                            data: "* The header in AsciiDoc must start with a document title.\n+\nThe header is optional.",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "* The header in AsciiDoc must start with a document title.\n+\nThe header is optional.",
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
                    data: "* The header in AsciiDoc must start with a document title.\n+\nThe header is optional.",
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
    fn complex() {
        verifies!(
            r#"
Using a list continuation, you can attach any number of block elements to a list item.
Unless the block is inside a delimited block which itself has been attached, each block must be preceded by a list continuation to form a chain of blocks.

Here's an example that attaches both a listing block and a paragraph to the first list item:

[source]
....
include::example$complex.adoc[tag=complex]
....

Here's how the source is rendered:

.A list with compound content
====
include::example$complex.adoc[tag=complex]
====

"#
        );

        let doc: crate::Document<'_> =
            Parser::default().parse("* The header in AsciiDoc must start with a document title.\n+\n----\n= Document Title\n----\n+\nKeep in mind that the header is optional.\n\n* Optional author and revision information lines immediately follow the document title.\n+\n----\n= Document Title\nDoc Writer <doc.writer@asciidoc.org>\nv1.0, 2022-01-01\n----");

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
                                            data: "The header in AsciiDoc must start with a document title.",
                                            line: 1,
                                            col: 3,
                                            offset: 2,
                                        },
                                        rendered: "The header in AsciiDoc must start with a document title.",
                                    },
                                    source: Span {
                                        data: "The header in AsciiDoc must start with a document title.",
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
                                Block::RawDelimited(RawDelimitedBlock {
                                    content: Content {
                                        original: Span {
                                            data: "= Document Title",
                                            line: 4,
                                            col: 1,
                                            offset: 66,
                                        },
                                        rendered: "= Document Title",
                                    },
                                    content_model: ContentModel::Verbatim,
                                    context: "listing",
                                    source: Span {
                                        data: "----\n= Document Title\n----",
                                        line: 3,
                                        col: 1,
                                        offset: 61,
                                    },
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                    substitution_group: SubstitutionGroup::Verbatim,
                                },),
                                Block::Simple(SimpleBlock {
                                    content: Content {
                                        original: Span {
                                            data: "Keep in mind that the header is optional.",
                                            line: 7,
                                            col: 1,
                                            offset: 90,
                                        },
                                        rendered: "Keep in mind that the header is optional.",
                                    },
                                    source: Span {
                                        data: "Keep in mind that the header is optional.",
                                        line: 7,
                                        col: 1,
                                        offset: 90,
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
                                data: "* The header in AsciiDoc must start with a document title.\n+\n----\n= Document Title\n----\n+\nKeep in mind that the header is optional.",
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
                                line: 9,
                                col: 1,
                                offset: 133,
                            },),
                            blocks: &[
                                Block::Simple(SimpleBlock {
                                    content: Content {
                                        original: Span {
                                            data: "Optional author and revision information lines immediately follow the document title.",
                                            line: 9,
                                            col: 3,
                                            offset: 135,
                                        },
                                        rendered: "Optional author and revision information lines immediately follow the document title.",
                                    },
                                    source: Span {
                                        data: "Optional author and revision information lines immediately follow the document title.",
                                        line: 9,
                                        col: 3,
                                        offset: 135,
                                    },
                                    style: SimpleBlockStyle::Paragraph,
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                                Block::RawDelimited(RawDelimitedBlock {
                                    content: Content {
                                        original: Span {
                                            data: "= Document Title\nDoc Writer <doc.writer@asciidoc.org>\nv1.0, 2022-01-01",
                                            line: 12,
                                            col: 1,
                                            offset: 228,
                                        },
                                        rendered: "= Document Title\nDoc Writer &lt;doc.writer@asciidoc.org&gt;\nv1.0, 2022-01-01",
                                    },
                                    content_model: ContentModel::Verbatim,
                                    context: "listing",
                                    source: Span {
                                        data: "----\n= Document Title\nDoc Writer <doc.writer@asciidoc.org>\nv1.0, 2022-01-01\n----",
                                        line: 11,
                                        col: 1,
                                        offset: 223,
                                    },
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                    substitution_group: SubstitutionGroup::Verbatim,
                                },),
                            ],
                            source: Span {
                                data: "* Optional author and revision information lines immediately follow the document title.\n+\n----\n= Document Title\nDoc Writer <doc.writer@asciidoc.org>\nv1.0, 2022-01-01\n----",
                                line: 9,
                                col: 1,
                                offset: 133,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                    ],
                    source: Span {
                        data: "* The header in AsciiDoc must start with a document title.\n+\n----\n= Document Title\n----\n+\nKeep in mind that the header is optional.\n\n* Optional author and revision information lines immediately follow the document title.\n+\n----\n= Document Title\nDoc Writer <doc.writer@asciidoc.org>\nv1.0, 2022-01-01\n----",
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
                    data: "* The header in AsciiDoc must start with a document title.\n+\n----\n= Document Title\n----\n+\nKeep in mind that the header is optional.\n\n* Optional author and revision information lines immediately follow the document title.\n+\n----\n= Document Title\nDoc Writer <doc.writer@asciidoc.org>\nv1.0, 2022-01-01\n----",
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
    fn sibling_interrupts_paragraph() {
        verifies!(
            r#"
Notice that we inserted an empty line after the attached paragraph block.
That's because only a sibling list item can interrupt a paragraph.
"#
        );

        let doc: crate::Document<'_> =
            Parser::default().parse("* The header in AsciiDoc must start with a document title.\n+\n----\n= Document Title\n----\n+\nKeep in mind that the header is optional.\n* Optional author and revision information lines immediately follow the document title.\n+\n----\n= Document Title\nDoc Writer <doc.writer@asciidoc.org>\nv1.0, 2022-01-01\n----");

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
                                            data: "The header in AsciiDoc must start with a document title.",
                                            line: 1,
                                            col: 3,
                                            offset: 2,
                                        },
                                        rendered: "The header in AsciiDoc must start with a document title.",
                                    },
                                    source: Span {
                                        data: "The header in AsciiDoc must start with a document title.",
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
                                Block::RawDelimited(RawDelimitedBlock {
                                    content: Content {
                                        original: Span {
                                            data: "= Document Title",
                                            line: 4,
                                            col: 1,
                                            offset: 66,
                                        },
                                        rendered: "= Document Title",
                                    },
                                    content_model: ContentModel::Verbatim,
                                    context: "listing",
                                    source: Span {
                                        data: "----\n= Document Title\n----",
                                        line: 3,
                                        col: 1,
                                        offset: 61,
                                    },
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                    substitution_group: SubstitutionGroup::Verbatim,
                                },),
                                Block::Simple(SimpleBlock {
                                    content: Content {
                                        original: Span {
                                            data: "Keep in mind that the header is optional.",
                                            line: 7,
                                            col: 1,
                                            offset: 90,
                                        },
                                        rendered: "Keep in mind that the header is optional.",
                                    },
                                    source: Span {
                                        data: "Keep in mind that the header is optional.",
                                        line: 7,
                                        col: 1,
                                        offset: 90,
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
                                data: "* The header in AsciiDoc must start with a document title.\n+\n----\n= Document Title\n----\n+\nKeep in mind that the header is optional.",
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
                                line: 8,
                                col: 1,
                                offset: 132,
                            },),
                            blocks: &[
                                Block::Simple(SimpleBlock {
                                    content: Content {
                                        original: Span {
                                            data: "Optional author and revision information lines immediately follow the document title.",
                                            line: 8,
                                            col: 3,
                                            offset: 134,
                                        },
                                        rendered: "Optional author and revision information lines immediately follow the document title.",
                                    },
                                    source: Span {
                                        data: "Optional author and revision information lines immediately follow the document title.",
                                        line: 8,
                                        col: 3,
                                        offset: 134,
                                    },
                                    style: SimpleBlockStyle::Paragraph,
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                                Block::RawDelimited(RawDelimitedBlock {
                                    content: Content {
                                        original: Span {
                                            data: "= Document Title\nDoc Writer <doc.writer@asciidoc.org>\nv1.0, 2022-01-01",
                                            line: 11,
                                            col: 1,
                                            offset: 227,
                                        },
                                        rendered: "= Document Title\nDoc Writer &lt;doc.writer@asciidoc.org&gt;\nv1.0, 2022-01-01",
                                    },
                                    content_model: ContentModel::Verbatim,
                                    context: "listing",
                                    source: Span {
                                        data: "----\n= Document Title\nDoc Writer <doc.writer@asciidoc.org>\nv1.0, 2022-01-01\n----",
                                        line: 10,
                                        col: 1,
                                        offset: 222,
                                    },
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                    substitution_group: SubstitutionGroup::Verbatim,
                                },),
                            ],
                            source: Span {
                                data: "* Optional author and revision information lines immediately follow the document title.\n+\n----\n= Document Title\nDoc Writer <doc.writer@asciidoc.org>\nv1.0, 2022-01-01\n----",
                                line: 8,
                                col: 1,
                                offset: 132,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                    ],
                    source: Span {
                        data: "* The header in AsciiDoc must start with a document title.\n+\n----\n= Document Title\n----\n+\nKeep in mind that the header is optional.\n* Optional author and revision information lines immediately follow the document title.\n+\n----\n= Document Title\nDoc Writer <doc.writer@asciidoc.org>\nv1.0, 2022-01-01\n----",
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
                    data: "* The header in AsciiDoc must start with a document title.\n+\n----\n= Document Title\n----\n+\nKeep in mind that the header is optional.\n* Optional author and revision information lines immediately follow the document title.\n+\n----\n= Document Title\nDoc Writer <doc.writer@asciidoc.org>\nv1.0, 2022-01-01\n----",
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
    fn nested_doesnt_interrupt_paragraph() {
        verifies!(
            r#"
If the next list item had been a nested list item instead of a sibling, this empty line would have been required.
Otherwise, the nested list marker and text would have just become the next line of the paragraph.
            "#
        );

        let doc: crate::Document<'_> =
            Parser::default().parse("* The header in AsciiDoc must start with a document title.\n+\n----\n= Document Title\n----\n+\nKeep in mind that the header is optional.\n** Optional author and revision information lines immediately follow the document title.\n+\n----\n= Document Title\nDoc Writer <doc.writer@asciidoc.org>\nv1.0, 2022-01-01\n----");

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
                    items: &[Block::ListItem(ListItem {
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
                                        data: "The header in AsciiDoc must start with a document title.",
                                        line: 1,
                                        col: 3,
                                        offset: 2,
                                    },
                                    rendered: "The header in AsciiDoc must start with a document title.",
                                },
                                source: Span {
                                    data: "The header in AsciiDoc must start with a document title.",
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
                            Block::RawDelimited(RawDelimitedBlock {
                                content: Content {
                                    original: Span {
                                        data: "= Document Title",
                                        line: 4,
                                        col: 1,
                                        offset: 66,
                                    },
                                    rendered: "= Document Title",
                                },
                                content_model: ContentModel::Verbatim,
                                context: "listing",
                                source: Span {
                                    data: "----\n= Document Title\n----",
                                    line: 3,
                                    col: 1,
                                    offset: 61,
                                },
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                                substitution_group: SubstitutionGroup::Verbatim,
                            },),
                            Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Keep in mind that the header is optional.\n** Optional author and revision information lines immediately follow the document title.",
                                        line: 7,
                                        col: 1,
                                        offset: 90,
                                    },
                                    rendered: "Keep in mind that the header is optional.\n** Optional author and revision information lines immediately follow the document title.",
                                },
                                source: Span {
                                    data: "Keep in mind that the header is optional.\n** Optional author and revision information lines immediately follow the document title.",
                                    line: 7,
                                    col: 1,
                                    offset: 90,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),
                            Block::RawDelimited(RawDelimitedBlock {
                                content: Content {
                                    original: Span {
                                        data: "= Document Title\nDoc Writer <doc.writer@asciidoc.org>\nv1.0, 2022-01-01",
                                        line: 11,
                                        col: 1,
                                        offset: 228,
                                    },
                                    rendered: "= Document Title\nDoc Writer &lt;doc.writer@asciidoc.org&gt;\nv1.0, 2022-01-01",
                                },
                                content_model: ContentModel::Verbatim,
                                context: "listing",
                                source: Span {
                                    data: "----\n= Document Title\nDoc Writer <doc.writer@asciidoc.org>\nv1.0, 2022-01-01\n----",
                                    line: 10,
                                    col: 1,
                                    offset: 223,
                                },
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                                substitution_group: SubstitutionGroup::Verbatim,
                            },),
                        ],
                        source: Span {
                            data: "* The header in AsciiDoc must start with a document title.\n+\n----\n= Document Title\n----\n+\nKeep in mind that the header is optional.\n** Optional author and revision information lines immediately follow the document title.\n+\n----\n= Document Title\nDoc Writer <doc.writer@asciidoc.org>\nv1.0, 2022-01-01\n----",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "* The header in AsciiDoc must start with a document title.\n+\n----\n= Document Title\n----\n+\nKeep in mind that the header is optional.\n** Optional author and revision information lines immediately follow the document title.\n+\n----\n= Document Title\nDoc Writer <doc.writer@asciidoc.org>\nv1.0, 2022-01-01\n----",
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
                    data: "* The header in AsciiDoc must start with a document title.\n+\n----\n= Document Title\n----\n+\nKeep in mind that the header is optional.\n** Optional author and revision information lines immediately follow the document title.\n+\n----\n= Document Title\nDoc Writer <doc.writer@asciidoc.org>\nv1.0, 2022-01-01\n----",
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
For consistency, a best practice is to always include an empty line at the end of a compound list item.
That way, you never have to remember when it's required.

IMPORTANT: A sibling or nested list item acts as an interrupting line for the principal text of a list item.
Only a sibling list item acts as an interrupting line for an attached block, such as a paragraph.
(The AsciiDoc Language working group has decided that the latter exception will be removed, so it's best not to depend on it.)

"#
    );

    #[test]
    fn wrap_multiple_blocks_in_open_block() {
        verifies!(
            r#"
If you're attaching more than one block to a list item, you're strongly encouraged to wrap the content inside an open block.
That way, you only need a single list continuation line to attach the open block to the list item.
Within the open block, you write like you normally would, no longer having to worry about adding list continuations between the blocks to keep them attached to the list item.

Here's an example of wrapping compound list content in an open block:

[source]
....
include::example$complex.adoc[tag=complex-o]
....

Here's how that content is rendered:

.A list with compound content wrapped in an open block
====
include::example$complex.adoc[tag=complex-o]
====

"#
        );

        let doc: crate::Document<'_> =
            Parser::default().parse("* The header in AsciiDoc must start with a document title.\n+\n--\nHere's an example of a document title:\n\n----\n= Document Title\n----\n\nNOTE: The header is optional.\n--");

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
                    items: &[Block::ListItem(ListItem {
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
                                        data: "The header in AsciiDoc must start with a document title.",
                                        line: 1,
                                        col: 3,
                                        offset: 2,
                                    },
                                    rendered: "The header in AsciiDoc must start with a document title.",
                                },
                                source: Span {
                                    data: "The header in AsciiDoc must start with a document title.",
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
                            Block::CompoundDelimited(CompoundDelimitedBlock {
                                blocks: &[
                                    Block::Simple(SimpleBlock {
                                        content: Content {
                                            original: Span {
                                                data: "Here's an example of a document title:",
                                                line: 4,
                                                col: 1,
                                                offset: 64,
                                            },
                                            rendered: "Here&#8217;s an example of a document title:",
                                        },
                                        source: Span {
                                            data: "Here's an example of a document title:",
                                            line: 4,
                                            col: 1,
                                            offset: 64,
                                        },
                                        style: SimpleBlockStyle::Paragraph,
                                        title_source: None,
                                        title: None,
                                        anchor: None,
                                        anchor_reftext: None,
                                        attrlist: None,
                                    },),
                                    Block::RawDelimited(RawDelimitedBlock {
                                        content: Content {
                                            original: Span {
                                                data: "= Document Title",
                                                line: 7,
                                                col: 1,
                                                offset: 109,
                                            },
                                            rendered: "= Document Title",
                                        },
                                        content_model: ContentModel::Verbatim,
                                        context: "listing",
                                        source: Span {
                                            data: "----\n= Document Title\n----",
                                            line: 6,
                                            col: 1,
                                            offset: 104,
                                        },
                                        title_source: None,
                                        title: None,
                                        anchor: None,
                                        anchor_reftext: None,
                                        attrlist: None,
                                        substitution_group: SubstitutionGroup::Verbatim,
                                    },),
                                    Block::Simple(SimpleBlock {
                                        content: Content {
                                            original: Span {
                                                data: "NOTE: The header is optional.",
                                                line: 10,
                                                col: 1,
                                                offset: 132,
                                            },
                                            rendered: "NOTE: The header is optional.",
                                        },
                                        source: Span {
                                            data: "NOTE: The header is optional.",
                                            line: 10,
                                            col: 1,
                                            offset: 132,
                                        },
                                        style: SimpleBlockStyle::Paragraph,
                                        title_source: None,
                                        title: None,
                                        anchor: None,
                                        anchor_reftext: None,
                                        attrlist: None,
                                    },),
                                ],
                                context: "open",
                                source: Span {
                                    data: "--\nHere's an example of a document title:\n\n----\n= Document Title\n----\n\nNOTE: The header is optional.\n--",
                                    line: 3,
                                    col: 1,
                                    offset: 61,
                                },
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),
                        ],
                        source: Span {
                            data: "* The header in AsciiDoc must start with a document title.\n+\n--\nHere's an example of a document title:\n\n----\n= Document Title\n----\n\nNOTE: The header is optional.\n--",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "* The header in AsciiDoc must start with a document title.\n+\n--\nHere's an example of a document title:\n\n----\n= Document Title\n----\n\nNOTE: The header is optional.\n--",
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
                    data: "* The header in AsciiDoc must start with a document title.\n+\n--\nHere's an example of a document title:\n\n----\n= Document Title\n----\n\nNOTE: The header is optional.\n--",
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
    fn open_block_for_include() {
        verifies!(
            r#"

The open block wrapper is also useful if you're including content from a shared file into a list item.
For example:

----
* list item
+
--
\include::shared-content.adoc[]
--
----

By wrapping the include directive in an open block, the content can be used unmodified.

The only limitation of this technique is that the content itself may not contain an open block since open blocks cannot (yet) be nested.

"#
        );

        let source = "* list item\n+\n--\ninclude::shared-content.adoc[]\n--";

        let handler = InlineFileHandler::from_pairs([(
            "shared-content.adoc",
            "(shared content goes here)\n",
        )]);

        let doc = Parser::default()
            .with_include_file_handler(handler)
            .parse(source);

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
                    items: &[Block::ListItem(ListItem {
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
                                        data: "list item",
                                        line: 1,
                                        col: 3,
                                        offset: 2,
                                    },
                                    rendered: "list item",
                                },
                                source: Span {
                                    data: "list item",
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
                            Block::CompoundDelimited(CompoundDelimitedBlock {
                                blocks: &[Block::Simple(SimpleBlock {
                                    content: Content {
                                        original: Span {
                                            data: "(shared content goes here)",
                                            line: 4,
                                            col: 1,
                                            offset: 17,
                                        },
                                        rendered: "(shared content goes here)",
                                    },
                                    source: Span {
                                        data: "(shared content goes here)",
                                        line: 4,
                                        col: 1,
                                        offset: 17,
                                    },
                                    style: SimpleBlockStyle::Paragraph,
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),],
                                context: "open",
                                source: Span {
                                    data: "--\n(shared content goes here)\n--",
                                    line: 3,
                                    col: 1,
                                    offset: 14,
                                },
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),
                        ],
                        source: Span {
                            data: "* list item\n+\n--\n(shared content goes here)\n--",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "* list item\n+\n--\n(shared content goes here)\n--",
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
                    data: "* list item\n+\n--\n(shared content goes here)\n--",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[
                    (4, SourceLine(Some("shared-content.adoc",), 1,),),
                    (5, SourceLine(None, 5,),),
                ]),
                catalog: Catalog {
                    refs: HashMap::from([]),
                    reftext_to_id: HashMap::from([]),
                },
            }
        );
    }
}

mod drop_principal_text {
    use std::collections::HashMap;

    use pretty_assertions_sorted::assert_eq;

    use crate::{
        Parser,
        blocks::{ContentModel, ListType},
        content::SubstitutionGroup,
        tests::prelude::*,
    };

    non_normative!(
        r#"
[#drop-principal-text]
== Drop the principal text

"#
    );

    #[test]
    fn use_empty_attribute() {
        verifies!(
            r#"
If the principal text of a list item is empty, the node for the principal text is dropped.
This is how you can get the first block (such as a listing block) to line up with the list marker.
You can make the principal text empty by using the `+{empty}+` attribute reference.

Here's an example of a list that has items with _only_ compound content.

[source]
....
include::example$complex.adoc[tag=complex-only]
....

Here's how the source is rendered:

.A list with compound content
====
include::example$complex.adoc[tag=complex-only]
====

"#
        );

        let doc = Parser::default().parse(
            ". {empty}\n+\n----\nprint(\"one\")\n----\n. {empty}\n+\n----\nprint(\"one\")\n----",
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
                    type_: ListType::Ordered,
                    items: &[
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Dots(Span {
                                data: ".",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },),
                            blocks: &[Block::RawDelimited(RawDelimitedBlock {
                                content: Content {
                                    original: Span {
                                        data: "print(\"one\")",
                                        line: 4,
                                        col: 1,
                                        offset: 17,
                                    },
                                    rendered: "print(\"one\")",
                                },
                                content_model: ContentModel::Verbatim,
                                context: "listing",
                                source: Span {
                                    data: "----\nprint(\"one\")\n----",
                                    line: 3,
                                    col: 1,
                                    offset: 12,
                                },
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                                substitution_group: SubstitutionGroup::Verbatim,
                            },),],
                            source: Span {
                                data: ". {empty}\n+\n----\nprint(\"one\")\n----",
                                line: 1,
                                col: 1,
                                offset: 0,
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
                                offset: 35,
                            },),
                            blocks: &[Block::RawDelimited(RawDelimitedBlock {
                                content: Content {
                                    original: Span {
                                        data: "print(\"one\")",
                                        line: 9,
                                        col: 1,
                                        offset: 52,
                                    },
                                    rendered: "print(\"one\")",
                                },
                                content_model: ContentModel::Verbatim,
                                context: "listing",
                                source: Span {
                                    data: "----\nprint(\"one\")\n----",
                                    line: 8,
                                    col: 1,
                                    offset: 47,
                                },
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                                substitution_group: SubstitutionGroup::Verbatim,
                            },),],
                            source: Span {
                                data: ". {empty}\n+\n----\nprint(\"one\")\n----",
                                line: 6,
                                col: 1,
                                offset: 35,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                    ],
                    source: Span {
                        data: ". {empty}\n+\n----\nprint(\"one\")\n----\n. {empty}\n+\n----\nprint(\"one\")\n----",
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
                    data: ". {empty}\n+\n----\nprint(\"one\")\n----\n. {empty}\n+\n----\nprint(\"one\")\n----",
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

mod attach_to_ancestor_list {
    use std::collections::HashMap;

    use pretty_assertions_sorted::assert_eq;

    use crate::{
        Parser,
        blocks::{ListType, SimpleBlockStyle},
        tests::prelude::*,
    };

    non_normative!(
        r#"
[#attach-to-ancestor-list]
== Attach blocks to an ancestor list

Instead of attaching a block to the current list item, you may need to end that list and attach a block to its ancestor instead.
There are two ways to express this composition in the AsciiDoc syntax.
You can either enclose the child list in an open block, or you can use insert empty lines above the list continuation to first escape from the nesting.
Let's look at enclosing the child list in an open block first, since that is the preferred method.

"#
    );

    #[test]
    fn enclose_in_open_block() {
        non_normative!(
            r#"
=== Enclose in open block

"#
        );

        verifies!(
            r#"
If you plan to attach blocks to a list item as a sibling of a nested list, the most robust way of creating that structure is to enclose the nested list in an open block.
That way, it's clear where the nested list ends and the current list item continues.

Here's an example of a list item with a nested list followed by an attached paragraph.
The open block makes the boundaries of the nested list clear.

[source]
....
include::example$complex.adoc[tag=complex-enclosed]
....

Here's how the source is rendered:

.A nested list enclosed in an open block
====
include::example$complex.adoc[tag=complex-enclosed]
====

The main limitation of this approach is that it can only be used once in the hierarchy (i.e., it can only enclose a single nested list).
That's because the open block itself cannot be nested.
If you require more control, then you must use the ancestor list continuation.

"#
        );

        let doc = Parser::default().parse(
            "* grandparent list item\n+\n--\n** parent list item\n*** child list item\n--\n+\nparagraph attached to grandparent list item",
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
                    items: &[Block::ListItem(ListItem {
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
                                        data: "grandparent list item",
                                        line: 1,
                                        col: 3,
                                        offset: 2,
                                    },
                                    rendered: "grandparent list item",
                                },
                                source: Span {
                                    data: "grandparent list item",
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
                            Block::CompoundDelimited(CompoundDelimitedBlock {
                                blocks: &[Block::List(ListBlock {
                                    type_: ListType::Unordered,
                                    items: &[Block::ListItem(ListItem {
                                        marker: ListItemMarker::Asterisks(Span {
                                            data: "**",
                                            line: 4,
                                            col: 1,
                                            offset: 29,
                                        },),
                                        blocks: &[
                                            Block::Simple(SimpleBlock {
                                                content: Content {
                                                    original: Span {
                                                        data: "parent list item",
                                                        line: 4,
                                                        col: 4,
                                                        offset: 32,
                                                    },
                                                    rendered: "parent list item",
                                                },
                                                source: Span {
                                                    data: "parent list item",
                                                    line: 4,
                                                    col: 4,
                                                    offset: 32,
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
                                                        line: 5,
                                                        col: 1,
                                                        offset: 49,
                                                    },),
                                                    blocks: &[Block::Simple(SimpleBlock {
                                                        content: Content {
                                                            original: Span {
                                                                data: "child list item",
                                                                line: 5,
                                                                col: 5,
                                                                offset: 53,
                                                            },
                                                            rendered: "child list item",
                                                        },
                                                        source: Span {
                                                            data: "child list item",
                                                            line: 5,
                                                            col: 5,
                                                            offset: 53,
                                                        },
                                                        style: SimpleBlockStyle::Paragraph,
                                                        title_source: None,
                                                        title: None,
                                                        anchor: None,
                                                        anchor_reftext: None,
                                                        attrlist: None,
                                                    },),],
                                                    source: Span {
                                                        data: "*** child list item",
                                                        line: 5,
                                                        col: 1,
                                                        offset: 49,
                                                    },
                                                    anchor: None,
                                                    anchor_reftext: None,
                                                    attrlist: None,
                                                },),],
                                                source: Span {
                                                    data: "*** child list item",
                                                    line: 5,
                                                    col: 1,
                                                    offset: 49,
                                                },
                                                title_source: None,
                                                title: None,
                                                anchor: None,
                                                anchor_reftext: None,
                                                attrlist: None,
                                            },),
                                        ],
                                        source: Span {
                                            data: "** parent list item\n*** child list item",
                                            line: 4,
                                            col: 1,
                                            offset: 29,
                                        },
                                        anchor: None,
                                        anchor_reftext: None,
                                        attrlist: None,
                                    },),],
                                    source: Span {
                                        data: "** parent list item\n*** child list item",
                                        line: 4,
                                        col: 1,
                                        offset: 29,
                                    },
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),],
                                context: "open",
                                source: Span {
                                    data: "--\n** parent list item\n*** child list item\n--",
                                    line: 3,
                                    col: 1,
                                    offset: 26,
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
                                        data: "paragraph attached to grandparent list item",
                                        line: 8,
                                        col: 1,
                                        offset: 74,
                                    },
                                    rendered: "paragraph attached to grandparent list item",
                                },
                                source: Span {
                                    data: "paragraph attached to grandparent list item",
                                    line: 8,
                                    col: 1,
                                    offset: 74,
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
                            data: "* grandparent list item\n+\n--\n** parent list item\n*** child list item\n--\n+\nparagraph attached to grandparent list item",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "* grandparent list item\n+\n--\n** parent list item\n*** child list item\n--\n+\nparagraph attached to grandparent list item",
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
                    data: "* grandparent list item\n+\n--\n** parent list item\n*** child list item\n--\n+\nparagraph attached to grandparent list item",
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
    fn ancestor_list_continuation() {
        non_normative!(
            r#"
=== Ancestor list continuation

"#
        );

        verifies!(
            r#"
Normally, a list continuation will attach a block to the current list item.
For each empty line you add before the list continuation, the association will move up one level in the nesting.
In other words, an empty line signals to the list continuation to back out of the current list by one level.
As a result, the block will be attached to the current item in an ancestor list.
This syntax is referred to as an [.term]*ancestor list continuation*.

WARNING: The ancestor list continuation is a fragile syntax.
For one, it may not be apparent to new authors that the empty lines before the list continuation are significant.
That's because the AsciiDoc syntax generally ignores repeating empty lines.
There are also scenarios where even these empty lines are collapsed, thus preventing the ancestor list continuation from working as expected.
Use this feature of the syntax with caution.
If possible, enclose the nested list in an open block, as described in the previous section.

Here's an example of a paragraph that's attached to the parent list item after the nested list ends.
The empty line above the list continuation indicates that the block should be attached to current list item in the parent list.

[source]
....
include::example$complex.adoc[tag=complex-parent]
....

Here's how the source is rendered:

.A block attached to the parent list item
====
include::example$complex.adoc[tag=complex-parent]
====

"#
        );

        let doc = Parser::default().parse(
            "* parent list item\n** child list item\n\n+\nparagraph attached to parent list item",
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
                    items: &[Block::ListItem(ListItem {
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
                                        data: "parent list item",
                                        line: 1,
                                        col: 3,
                                        offset: 2,
                                    },
                                    rendered: "parent list item",
                                },
                                source: Span {
                                    data: "parent list item",
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
                                    marker: ListItemMarker::Asterisks(Span {
                                        data: "**",
                                        line: 2,
                                        col: 1,
                                        offset: 19,
                                    },),
                                    blocks: &[Block::Simple(SimpleBlock {
                                        content: Content {
                                            original: Span {
                                                data: "child list item",
                                                line: 2,
                                                col: 4,
                                                offset: 22,
                                            },
                                            rendered: "child list item",
                                        },
                                        source: Span {
                                            data: "child list item",
                                            line: 2,
                                            col: 4,
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
                                        data: "** child list item",
                                        line: 2,
                                        col: 1,
                                        offset: 19,
                                    },
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),],
                                source: Span {
                                    data: "** child list item",
                                    line: 2,
                                    col: 1,
                                    offset: 19,
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
                                        data: "paragraph attached to parent list item",
                                        line: 5,
                                        col: 1,
                                        offset: 41,
                                    },
                                    rendered: "paragraph attached to parent list item",
                                },
                                source: Span {
                                    data: "paragraph attached to parent list item",
                                    line: 5,
                                    col: 1,
                                    offset: 41,
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
                            data: "* parent list item\n** child list item\n\n+\nparagraph attached to parent list item",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "* parent list item\n** child list item\n\n+\nparagraph attached to parent list item",
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
                    data: "* parent list item\n** child list item\n\n+\nparagraph attached to parent list item",
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
    fn empty_line_moves_up() {
        verifies!(
            r#"

Each empty line that precedes the list continuation signals a move up one level of nesting.
Here's an example that shows how to attach a paragraph to a grandparent list item using two leading empty lines:

[source]
....
include::example$complex.adoc[tag=complex-grandparent]
....

Here's how the source is rendered:

.A block attached to the grandparent list item
====
include::example$complex.adoc[tag=complex-grandparent]
====

"#
        );

        let doc = Parser::default().parse(
            "* grandparent list item\n** parent list item\n*** child list item\n\n\n+\nparagraph attached to grandparent list item",
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
                    items: &[Block::ListItem(ListItem {
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
                                        data: "grandparent list item",
                                        line: 1,
                                        col: 3,
                                        offset: 2,
                                    },
                                    rendered: "grandparent list item",
                                },
                                source: Span {
                                    data: "grandparent list item",
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
                                    marker: ListItemMarker::Asterisks(Span {
                                        data: "**",
                                        line: 2,
                                        col: 1,
                                        offset: 24,
                                    },),
                                    blocks: &[
                                        Block::Simple(SimpleBlock {
                                            content: Content {
                                                original: Span {
                                                    data: "parent list item",
                                                    line: 2,
                                                    col: 4,
                                                    offset: 27,
                                                },
                                                rendered: "parent list item",
                                            },
                                            source: Span {
                                                data: "parent list item",
                                                line: 2,
                                                col: 4,
                                                offset: 27,
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
                                                    line: 3,
                                                    col: 1,
                                                    offset: 44,
                                                },),
                                                blocks: &[Block::Simple(SimpleBlock {
                                                    content: Content {
                                                        original: Span {
                                                            data: "child list item",
                                                            line: 3,
                                                            col: 5,
                                                            offset: 48,
                                                        },
                                                        rendered: "child list item",
                                                    },
                                                    source: Span {
                                                        data: "child list item",
                                                        line: 3,
                                                        col: 5,
                                                        offset: 48,
                                                    },
                                                    style: SimpleBlockStyle::Paragraph,
                                                    title_source: None,
                                                    title: None,
                                                    anchor: None,
                                                    anchor_reftext: None,
                                                    attrlist: None,
                                                },),],
                                                source: Span {
                                                    data: "*** child list item",
                                                    line: 3,
                                                    col: 1,
                                                    offset: 44,
                                                },
                                                anchor: None,
                                                anchor_reftext: None,
                                                attrlist: None,
                                            },),],
                                            source: Span {
                                                data: "*** child list item",
                                                line: 3,
                                                col: 1,
                                                offset: 44,
                                            },
                                            title_source: None,
                                            title: None,
                                            anchor: None,
                                            anchor_reftext: None,
                                            attrlist: None,
                                        },),
                                    ],
                                    source: Span {
                                        data: "** parent list item\n*** child list item",
                                        line: 2,
                                        col: 1,
                                        offset: 24,
                                    },
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),],
                                source: Span {
                                    data: "** parent list item\n*** child list item",
                                    line: 2,
                                    col: 1,
                                    offset: 24,
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
                                        data: "paragraph attached to grandparent list item",
                                        line: 7,
                                        col: 1,
                                        offset: 68,
                                    },
                                    rendered: "paragraph attached to grandparent list item",
                                },
                                source: Span {
                                    data: "paragraph attached to grandparent list item",
                                    line: 7,
                                    col: 1,
                                    offset: 68,
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
                            data: "* grandparent list item\n** parent list item\n*** child list item\n\n\n+\nparagraph attached to grandparent list item",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "* grandparent list item\n** parent list item\n*** child list item\n\n\n+\nparagraph attached to grandparent list item",
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
                    data: "* grandparent list item\n** parent list item\n*** child list item\n\n\n+\nparagraph attached to grandparent list item",
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

non_normative!(
    r#"
== Summary

On this page, you learned that the principal text of a list item can span multiple contiguous lines, and that those lines can be indented for readability without affecting the output.
You learned that you can attach any type of block content to a list item using the list continuation.
You also learned that using this feature in combination with the open block makes it easier to create list items with compound content, to attach blocks to a parent list, or to drop the principal text.
Finally, you learned that you can use the

use crate::tests::sdd::non_normative; ancestor list continuation to attach blocks to the current item in an ancestor list, and the risks with doing so.
"#
);
