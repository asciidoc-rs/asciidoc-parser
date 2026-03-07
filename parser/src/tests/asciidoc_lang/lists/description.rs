use std::collections::HashMap;

use pretty_assertions_sorted::assert_eq;

use crate::{
    Parser,
    blocks::{ListType, SimpleBlockStyle},
    tests::prelude::*,
};

track_file!("docs/modules/lists/pages/description.adoc");

non_normative!(
    r#"
= Description Lists
:keywords: dlist, definition list, labeled list

A description list (often abbreviated as dlist in AsciiDoc) is an association list that consists of one or more terms (or sets of terms) that each have a description.
This list type is useful when you have a list of terms that you want to emphasize and describe with text or other supporting content.

NOTE: You may know this list variation by the antiquated term _definition list_.
The preferred term is now _description list_, which matches the terminology used by the https://html.spec.whatwg.org/multipage/grouping-content.html#the-dl-element[HTML specification^].

== Anatomy

A description list item marks the beginning of a description list.
Each item in a description list consists of:

* one or more terms, each followed by a term delimiter (typically a double colon, `::`, unless the list is nested)
* one space or newline character
* the description in the form of text, attached blocks, or both

If a term has an anchor, the anchor must be defined at the start of the same line as the term.

The first term defines which term delimiter is used for the description list.
The terms for the remaining entries at that level must use the same delimiter.

The valid set of term delimiters is fixed.
When the term delimiter is changed, that term begins a new, nested description list (similar to how ordered and unordered lists work).
The available term delimiters you can use for this purpose are as follows:

* `::`
* `:::`
* `::::`
* `;;`

There's no direct correlation between the number of characters in the delimiter and the nesting level.
Each time you change delimiters (selected from this set), it introduces a new level of nesting.
This is how list depth is implied in a language with a left-aligned syntax.
It's customary to use the delimiters in the order shown above to provide a hint that the list is nested at a certain level.

"#
);

#[test]
fn basic_example() {
    non_normative!(
        r#"
== Basic description list

"#
    );

    verifies!(
        r#"
Here's an example of a description list that identifies parts of a computer:

----
include::example$description.adoc[tag=base]
----

By default, the content of each item is displayed below the label when rendered.
Here's a preview of how this list is rendered:

.A basic description list
====
include::example$description.adoc[tag=base]
====

"#
    );

    let doc: crate::Document<'_> =
            Parser::default().parse("CPU:: The brain of the computer.\nHard drive:: Permanent storage for operating system and/or user files.\nRAM:: Temporarily stores information the CPU uses during operation.\nKeyboard:: Used to enter text or control items on the screen.\nMouse:: Used to point to and select items on your computer screen.\nMonitor:: Displays information in visual form using text and graphics.");

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
                type_: ListType::Description,
                items: &[
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::DefinedTerm {
                            term: Content {
                                original: Span {
                                    data: "CPU",
                                    line: 1,
                                    col: 1,
                                    offset: 0,
                                },
                                rendered: "CPU",
                            },
                            marker: Span {
                                data: "::",
                                line: 1,
                                col: 4,
                                offset: 3,
                            },
                            source: Span {
                                data: "CPU::",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                        },
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "The brain of the computer.",
                                    line: 1,
                                    col: 7,
                                    offset: 6,
                                },
                                rendered: "The brain of the computer.",
                            },
                            source: Span {
                                data: "The brain of the computer.",
                                line: 1,
                                col: 7,
                                offset: 6,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "CPU:: The brain of the computer.",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::DefinedTerm {
                            term: Content {
                                original: Span {
                                    data: "Hard drive",
                                    line: 2,
                                    col: 1,
                                    offset: 33,
                                },
                                rendered: "Hard drive",
                            },
                            marker: Span {
                                data: "::",
                                line: 2,
                                col: 11,
                                offset: 43,
                            },
                            source: Span {
                                data: "Hard drive::",
                                line: 2,
                                col: 1,
                                offset: 33,
                            },
                        },
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "Permanent storage for operating system and/or user files.",
                                    line: 2,
                                    col: 14,
                                    offset: 46,
                                },
                                rendered: "Permanent storage for operating system and/or user files.",
                            },
                            source: Span {
                                data: "Permanent storage for operating system and/or user files.",
                                line: 2,
                                col: 14,
                                offset: 46,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "Hard drive:: Permanent storage for operating system and/or user files.",
                            line: 2,
                            col: 1,
                            offset: 33,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::DefinedTerm {
                            term: Content {
                                original: Span {
                                    data: "RAM",
                                    line: 3,
                                    col: 1,
                                    offset: 104,
                                },
                                rendered: "RAM",
                            },
                            marker: Span {
                                data: "::",
                                line: 3,
                                col: 4,
                                offset: 107,
                            },
                            source: Span {
                                data: "RAM::",
                                line: 3,
                                col: 1,
                                offset: 104,
                            },
                        },
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "Temporarily stores information the CPU uses during operation.",
                                    line: 3,
                                    col: 7,
                                    offset: 110,
                                },
                                rendered: "Temporarily stores information the CPU uses during operation.",
                            },
                            source: Span {
                                data: "Temporarily stores information the CPU uses during operation.",
                                line: 3,
                                col: 7,
                                offset: 110,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "RAM:: Temporarily stores information the CPU uses during operation.",
                            line: 3,
                            col: 1,
                            offset: 104,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::DefinedTerm {
                            term: Content {
                                original: Span {
                                    data: "Keyboard",
                                    line: 4,
                                    col: 1,
                                    offset: 172,
                                },
                                rendered: "Keyboard",
                            },
                            marker: Span {
                                data: "::",
                                line: 4,
                                col: 9,
                                offset: 180,
                            },
                            source: Span {
                                data: "Keyboard::",
                                line: 4,
                                col: 1,
                                offset: 172,
                            },
                        },
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "Used to enter text or control items on the screen.",
                                    line: 4,
                                    col: 12,
                                    offset: 183,
                                },
                                rendered: "Used to enter text or control items on the screen.",
                            },
                            source: Span {
                                data: "Used to enter text or control items on the screen.",
                                line: 4,
                                col: 12,
                                offset: 183,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "Keyboard:: Used to enter text or control items on the screen.",
                            line: 4,
                            col: 1,
                            offset: 172,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::DefinedTerm {
                            term: Content {
                                original: Span {
                                    data: "Mouse",
                                    line: 5,
                                    col: 1,
                                    offset: 234,
                                },
                                rendered: "Mouse",
                            },
                            marker: Span {
                                data: "::",
                                line: 5,
                                col: 6,
                                offset: 239,
                            },
                            source: Span {
                                data: "Mouse::",
                                line: 5,
                                col: 1,
                                offset: 234,
                            },
                        },
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "Used to point to and select items on your computer screen.",
                                    line: 5,
                                    col: 9,
                                    offset: 242,
                                },
                                rendered: "Used to point to and select items on your computer screen.",
                            },
                            source: Span {
                                data: "Used to point to and select items on your computer screen.",
                                line: 5,
                                col: 9,
                                offset: 242,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "Mouse:: Used to point to and select items on your computer screen.",
                            line: 5,
                            col: 1,
                            offset: 234,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::DefinedTerm {
                            term: Content {
                                original: Span {
                                    data: "Monitor",
                                    line: 6,
                                    col: 1,
                                    offset: 301,
                                },
                                rendered: "Monitor",
                            },
                            marker: Span {
                                data: "::",
                                line: 6,
                                col: 8,
                                offset: 308,
                            },
                            source: Span {
                                data: "Monitor::",
                                line: 6,
                                col: 1,
                                offset: 301,
                            },
                        },
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "Displays information in visual form using text and graphics.",
                                    line: 6,
                                    col: 11,
                                    offset: 311,
                                },
                                rendered: "Displays information in visual form using text and graphics.",
                            },
                            source: Span {
                                data: "Displays information in visual form using text and graphics.",
                                line: 6,
                                col: 11,
                                offset: 311,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "Monitor:: Displays information in visual form using text and graphics.",
                            line: 6,
                            col: 1,
                            offset: 301,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: "CPU:: The brain of the computer.\nHard drive:: Permanent storage for operating system and/or user files.\nRAM:: Temporarily stores information the CPU uses during operation.\nKeyboard:: Used to enter text or control items on the screen.\nMouse:: Used to point to and select items on your computer screen.\nMonitor:: Displays information in visual form using text and graphics.",
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
                data: "CPU:: The brain of the computer.\nHard drive:: Permanent storage for operating system and/or user files.\nRAM:: Temporarily stores information the CPU uses during operation.\nKeyboard:: Used to enter text or control items on the screen.\nMouse:: Used to point to and select items on your computer screen.\nMonitor:: Displays information in visual form using text and graphics.",
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
fn mixing_lists() {
    non_normative!(
        r#"
== Mixing lists

"#
    );

    verifies!(
        r#"
The content of a description list can be any AsciiDoc element.
For instance, we could split up a grocery list by aisle, using description list terms for the aisle names.

----
include::example$description.adoc[tag=base-mix]
----

====
include::example$description.adoc[tag=base-mix]
====

"#
    );

    let doc: crate::Document<'_> =
        Parser::default().parse("Dairy::\n* Milk\n* Eggs\nBakery::\n* Bread\nProduce::\n* Bananas");

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
                type_: ListType::Description,
                items: &[
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::DefinedTerm {
                            term: Content {
                                original: Span {
                                    data: "Dairy",
                                    line: 1,
                                    col: 1,
                                    offset: 0,
                                },
                                rendered: "Dairy",
                            },
                            marker: Span {
                                data: "::",
                                line: 1,
                                col: 6,
                                offset: 5,
                            },
                            source: Span {
                                data: "Dairy::",
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
                                        offset: 8,
                                    },),
                                    blocks: &[Block::Simple(SimpleBlock {
                                        content: Content {
                                            original: Span {
                                                data: "Milk",
                                                line: 2,
                                                col: 3,
                                                offset: 10,
                                            },
                                            rendered: "Milk",
                                        },
                                        source: Span {
                                            data: "Milk",
                                            line: 2,
                                            col: 3,
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
                                        data: "* Milk",
                                        line: 2,
                                        col: 1,
                                        offset: 8,
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
                                        offset: 15,
                                    },),
                                    blocks: &[Block::Simple(SimpleBlock {
                                        content: Content {
                                            original: Span {
                                                data: "Eggs",
                                                line: 3,
                                                col: 3,
                                                offset: 17,
                                            },
                                            rendered: "Eggs",
                                        },
                                        source: Span {
                                            data: "Eggs",
                                            line: 3,
                                            col: 3,
                                            offset: 17,
                                        },
                                        style: SimpleBlockStyle::Paragraph,
                                        title_source: None,
                                        title: None,
                                        anchor: None,
                                        anchor_reftext: None,
                                        attrlist: None,
                                    },),],
                                    source: Span {
                                        data: "* Eggs",
                                        line: 3,
                                        col: 1,
                                        offset: 15,
                                    },
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                            ],
                            source: Span {
                                data: "* Milk\n* Eggs",
                                line: 2,
                                col: 1,
                                offset: 8,
                            },
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "Dairy::\n* Milk\n* Eggs",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::DefinedTerm {
                            term: Content {
                                original: Span {
                                    data: "Bakery",
                                    line: 4,
                                    col: 1,
                                    offset: 22,
                                },
                                rendered: "Bakery",
                            },
                            marker: Span {
                                data: "::",
                                line: 4,
                                col: 7,
                                offset: 28,
                            },
                            source: Span {
                                data: "Bakery::",
                                line: 4,
                                col: 1,
                                offset: 22,
                            },
                        },
                        blocks: &[Block::List(ListBlock {
                            type_: ListType::Unordered,
                            items: &[Block::ListItem(ListItem {
                                marker: ListItemMarker::Asterisks(Span {
                                    data: "*",
                                    line: 5,
                                    col: 1,
                                    offset: 31,
                                },),
                                blocks: &[Block::Simple(SimpleBlock {
                                    content: Content {
                                        original: Span {
                                            data: "Bread",
                                            line: 5,
                                            col: 3,
                                            offset: 33,
                                        },
                                        rendered: "Bread",
                                    },
                                    source: Span {
                                        data: "Bread",
                                        line: 5,
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
                                    data: "* Bread",
                                    line: 5,
                                    col: 1,
                                    offset: 31,
                                },
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: "* Bread",
                                line: 5,
                                col: 1,
                                offset: 31,
                            },
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "Bakery::\n* Bread",
                            line: 4,
                            col: 1,
                            offset: 22,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::DefinedTerm {
                            term: Content {
                                original: Span {
                                    data: "Produce",
                                    line: 6,
                                    col: 1,
                                    offset: 39,
                                },
                                rendered: "Produce",
                            },
                            marker: Span {
                                data: "::",
                                line: 6,
                                col: 8,
                                offset: 46,
                            },
                            source: Span {
                                data: "Produce::",
                                line: 6,
                                col: 1,
                                offset: 39,
                            },
                        },
                        blocks: &[Block::List(ListBlock {
                            type_: ListType::Unordered,
                            items: &[Block::ListItem(ListItem {
                                marker: ListItemMarker::Asterisks(Span {
                                    data: "*",
                                    line: 7,
                                    col: 1,
                                    offset: 49,
                                },),
                                blocks: &[Block::Simple(SimpleBlock {
                                    content: Content {
                                        original: Span {
                                            data: "Bananas",
                                            line: 7,
                                            col: 3,
                                            offset: 51,
                                        },
                                        rendered: "Bananas",
                                    },
                                    source: Span {
                                        data: "Bananas",
                                        line: 7,
                                        col: 3,
                                        offset: 51,
                                    },
                                    style: SimpleBlockStyle::Paragraph,
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),],
                                source: Span {
                                    data: "* Bananas",
                                    line: 7,
                                    col: 1,
                                    offset: 49,
                                },
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: "* Bananas",
                                line: 7,
                                col: 1,
                                offset: 49,
                            },
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "Produce::\n* Bananas",
                            line: 6,
                            col: 1,
                            offset: 39,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: "Dairy::\n* Milk\n* Eggs\nBakery::\n* Bread\nProduce::\n* Bananas",
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
                data: "Dairy::\n* Milk\n* Eggs\nBakery::\n* Bread\nProduce::\n* Bananas",
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
fn lenient_whitespace() {
    verifies!(
        r#"
Description lists are quite lenient about whitespace, so you can spread the items out and even indent the content if that makes it more readable for you:

----
include::example$description.adoc[tag=base-mix-alt]
----

"#
    );

    let doc: crate::Document<'_> = Parser::default().parse(
        "Dairy::\n\n  * Milk\n  * Eggs\n\nBakery::\n\n  * Bread\n\nProduce::\n\n  * Bananas",
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
                type_: ListType::Description,
                items: &[
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::DefinedTerm {
                            term: Content {
                                original: Span {
                                    data: "Dairy",
                                    line: 1,
                                    col: 1,
                                    offset: 0,
                                },
                                rendered: "Dairy",
                            },
                            marker: Span {
                                data: "::",
                                line: 1,
                                col: 6,
                                offset: 5,
                            },
                            source: Span {
                                data: "Dairy::",
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
                                        line: 3,
                                        col: 3,
                                        offset: 11,
                                    },),
                                    blocks: &[Block::Simple(SimpleBlock {
                                        content: Content {
                                            original: Span {
                                                data: "Milk",
                                                line: 3,
                                                col: 5,
                                                offset: 13,
                                            },
                                            rendered: "Milk",
                                        },
                                        source: Span {
                                            data: "Milk",
                                            line: 3,
                                            col: 5,
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
                                        data: "  * Milk",
                                        line: 3,
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
                                        line: 4,
                                        col: 3,
                                        offset: 20,
                                    },),
                                    blocks: &[Block::Simple(SimpleBlock {
                                        content: Content {
                                            original: Span {
                                                data: "Eggs",
                                                line: 4,
                                                col: 5,
                                                offset: 22,
                                            },
                                            rendered: "Eggs",
                                        },
                                        source: Span {
                                            data: "Eggs",
                                            line: 4,
                                            col: 5,
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
                                        data: "  * Eggs",
                                        line: 4,
                                        col: 1,
                                        offset: 18,
                                    },
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                            ],
                            source: Span {
                                data: "  * Milk\n  * Eggs",
                                line: 3,
                                col: 1,
                                offset: 9,
                            },
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "Dairy::\n\n  * Milk\n  * Eggs",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::DefinedTerm {
                            term: Content {
                                original: Span {
                                    data: "Bakery",
                                    line: 6,
                                    col: 1,
                                    offset: 28,
                                },
                                rendered: "Bakery",
                            },
                            marker: Span {
                                data: "::",
                                line: 6,
                                col: 7,
                                offset: 34,
                            },
                            source: Span {
                                data: "Bakery::",
                                line: 6,
                                col: 1,
                                offset: 28,
                            },
                        },
                        blocks: &[Block::List(ListBlock {
                            type_: ListType::Unordered,
                            items: &[Block::ListItem(ListItem {
                                marker: ListItemMarker::Asterisks(Span {
                                    data: "*",
                                    line: 8,
                                    col: 3,
                                    offset: 40,
                                },),
                                blocks: &[Block::Simple(SimpleBlock {
                                    content: Content {
                                        original: Span {
                                            data: "Bread",
                                            line: 8,
                                            col: 5,
                                            offset: 42,
                                        },
                                        rendered: "Bread",
                                    },
                                    source: Span {
                                        data: "Bread",
                                        line: 8,
                                        col: 5,
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
                                    data: "  * Bread",
                                    line: 8,
                                    col: 1,
                                    offset: 38,
                                },
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: "  * Bread",
                                line: 8,
                                col: 1,
                                offset: 38,
                            },
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "Bakery::\n\n  * Bread",
                            line: 6,
                            col: 1,
                            offset: 28,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::DefinedTerm {
                            term: Content {
                                original: Span {
                                    data: "Produce",
                                    line: 10,
                                    col: 1,
                                    offset: 49,
                                },
                                rendered: "Produce",
                            },
                            marker: Span {
                                data: "::",
                                line: 10,
                                col: 8,
                                offset: 56,
                            },
                            source: Span {
                                data: "Produce::",
                                line: 10,
                                col: 1,
                                offset: 49,
                            },
                        },
                        blocks: &[Block::List(ListBlock {
                            type_: ListType::Unordered,
                            items: &[Block::ListItem(ListItem {
                                marker: ListItemMarker::Asterisks(Span {
                                    data: "*",
                                    line: 12,
                                    col: 3,
                                    offset: 62,
                                },),
                                blocks: &[Block::Simple(SimpleBlock {
                                    content: Content {
                                        original: Span {
                                            data: "Bananas",
                                            line: 12,
                                            col: 5,
                                            offset: 64,
                                        },
                                        rendered: "Bananas",
                                    },
                                    source: Span {
                                        data: "Bananas",
                                        line: 12,
                                        col: 5,
                                        offset: 64,
                                    },
                                    style: SimpleBlockStyle::Paragraph,
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),],
                                source: Span {
                                    data: "  * Bananas",
                                    line: 12,
                                    col: 1,
                                    offset: 60,
                                },
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: "  * Bananas",
                                line: 12,
                                col: 1,
                                offset: 60,
                            },
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "Produce::\n\n  * Bananas",
                            line: 10,
                            col: 1,
                            offset: 49,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: "Dairy::\n\n  * Milk\n  * Eggs\n\nBakery::\n\n  * Bread\n\nProduce::\n\n  * Bananas",
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
                data: "Dairy::\n\n  * Milk\n  * Eggs\n\nBakery::\n\n  * Bread\n\nProduce::\n\n  * Bananas",
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
fn nested_description_list() {
    verifies!(
        r#"
== Nested description list

[#three-hybrid]
Finally, you can mix and match the three list types within a single hybrid list.
The AsciiDoc syntax tries hard to infer the relationships between the items that are most intuitive to us humans.

Here's a list that mixes description, ordered, and unordered lists.
Notice how the term delimiter is changed from `::` to `:::` to create a nested description list.

----
include::example$description.adoc[tag=3-mix]
----

Here's how the list is rendered:

.A hybrid list
====
include::example$description.adoc[tag=3-mix]
====

You can include more xref:continuation.adoc[compound content in a list item] as well.
"#
    );

    let doc: crate::Document<'_> =
        Parser::default().parse("Operating Systems::\n  Linux:::\n    . Fedora\n      * Desktop\n    . Ubuntu\n      * Desktop\n      * Server\n  BSD:::\n    . FreeBSD\n    . NetBSD\n\nCloud Providers::\n  PaaS:::\n    . OpenShift\n    . CloudBees\n  IaaS:::\n    . Amazon EC2\n    . Rackspace");

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
                type_: ListType::Description,
                items: &[
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::DefinedTerm {
                            term: Content {
                                original: Span {
                                    data: "Operating Systems",
                                    line: 1,
                                    col: 1,
                                    offset: 0,
                                },
                                rendered: "Operating Systems",
                            },
                            marker: Span {
                                data: "::",
                                line: 1,
                                col: 18,
                                offset: 17,
                            },
                            source: Span {
                                data: "Operating Systems::",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                        },
                        blocks: &[Block::List(ListBlock {
                            type_: ListType::Description,
                            items: &[
                                Block::ListItem(ListItem {
                                    marker: ListItemMarker::DefinedTerm {
                                        term: Content {
                                            original: Span {
                                                data: "Linux",
                                                line: 2,
                                                col: 3,
                                                offset: 22,
                                            },
                                            rendered: "Linux",
                                        },
                                        marker: Span {
                                            data: ":::",
                                            line: 2,
                                            col: 8,
                                            offset: 27,
                                        },
                                        source: Span {
                                            data: "Linux:::",
                                            line: 2,
                                            col: 3,
                                            offset: 22,
                                        },
                                    },
                                    blocks: &[],
                                    source: Span {
                                        data: "  Linux:::",
                                        line: 2,
                                        col: 1,
                                        offset: 20,
                                    },
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                                Block::ListItem(ListItem {
                                    marker: ListItemMarker::Dots(Span {
                                        data: ".",
                                        line: 3,
                                        col: 5,
                                        offset: 35,
                                    },),
                                    blocks: &[
                                        Block::Simple(SimpleBlock {
                                            content: Content {
                                                original: Span {
                                                    data: "Fedora",
                                                    line: 3,
                                                    col: 7,
                                                    offset: 37,
                                                },
                                                rendered: "Fedora",
                                            },
                                            source: Span {
                                                data: "Fedora",
                                                line: 3,
                                                col: 7,
                                                offset: 37,
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
                                                    data: "*",
                                                    line: 4,
                                                    col: 7,
                                                    offset: 50,
                                                },),
                                                blocks: &[Block::Simple(SimpleBlock {
                                                    content: Content {
                                                        original: Span {
                                                            data: "Desktop",
                                                            line: 4,
                                                            col: 9,
                                                            offset: 52,
                                                        },
                                                        rendered: "Desktop",
                                                    },
                                                    source: Span {
                                                        data: "Desktop",
                                                        line: 4,
                                                        col: 9,
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
                                                    data: "      * Desktop",
                                                    line: 4,
                                                    col: 1,
                                                    offset: 44,
                                                },
                                                anchor: None,
                                                anchor_reftext: None,
                                                attrlist: None,
                                            },),],
                                            source: Span {
                                                data: "      * Desktop",
                                                line: 4,
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
                                        data: "    . Fedora\n      * Desktop",
                                        line: 3,
                                        col: 1,
                                        offset: 31,
                                    },
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                                Block::ListItem(ListItem {
                                    marker: ListItemMarker::Dots(Span {
                                        data: ".",
                                        line: 5,
                                        col: 5,
                                        offset: 64,
                                    },),
                                    blocks: &[
                                        Block::Simple(SimpleBlock {
                                            content: Content {
                                                original: Span {
                                                    data: "Ubuntu",
                                                    line: 5,
                                                    col: 7,
                                                    offset: 66,
                                                },
                                                rendered: "Ubuntu",
                                            },
                                            source: Span {
                                                data: "Ubuntu",
                                                line: 5,
                                                col: 7,
                                                offset: 66,
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
                                                        col: 7,
                                                        offset: 79,
                                                    },),
                                                    blocks: &[Block::Simple(SimpleBlock {
                                                        content: Content {
                                                            original: Span {
                                                                data: "Desktop",
                                                                line: 6,
                                                                col: 9,
                                                                offset: 81,
                                                            },
                                                            rendered: "Desktop",
                                                        },
                                                        source: Span {
                                                            data: "Desktop",
                                                            line: 6,
                                                            col: 9,
                                                            offset: 81,
                                                        },
                                                        style: SimpleBlockStyle::Paragraph,
                                                        title_source: None,
                                                        title: None,
                                                        anchor: None,
                                                        anchor_reftext: None,
                                                        attrlist: None,
                                                    },),],
                                                    source: Span {
                                                        data: "      * Desktop",
                                                        line: 6,
                                                        col: 1,
                                                        offset: 73,
                                                    },
                                                    anchor: None,
                                                    anchor_reftext: None,
                                                    attrlist: None,
                                                },),
                                                Block::ListItem(ListItem {
                                                    marker: ListItemMarker::Asterisks(Span {
                                                        data: "*",
                                                        line: 7,
                                                        col: 7,
                                                        offset: 95,
                                                    },),
                                                    blocks: &[
                                                        Block::Simple(SimpleBlock {
                                                            content: Content {
                                                                original: Span {
                                                                    data: "Server",
                                                                    line: 7,
                                                                    col: 9,
                                                                    offset: 97,
                                                                },
                                                                rendered: "Server",
                                                            },
                                                            source: Span {
                                                                data: "Server",
                                                                line: 7,
                                                                col: 9,
                                                                offset: 97,
                                                            },
                                                            style: SimpleBlockStyle::Paragraph,
                                                            title_source: None,
                                                            title: None,
                                                            anchor: None,
                                                            anchor_reftext: None,
                                                            attrlist: None,
                                                        },),
                                                        Block::List(ListBlock {
                                                            type_: ListType::Description,
                                                            items: &[Block::ListItem(ListItem {
                                                                marker:
                                                                    ListItemMarker::DefinedTerm {
                                                                        term: Content {
                                                                            original: Span {
                                                                                data: "BSD",
                                                                                line: 8,
                                                                                col: 3,
                                                                                offset: 106,
                                                                            },
                                                                            rendered: "BSD",
                                                                        },
                                                                        marker: Span {
                                                                            data: ":::",
                                                                            line: 8,
                                                                            col: 6,
                                                                            offset: 109,
                                                                        },
                                                                        source: Span {
                                                                            data: "BSD:::",
                                                                            line: 8,
                                                                            col: 3,
                                                                            offset: 106,
                                                                        },
                                                                    },
                                                                blocks: &[],
                                                                source: Span {
                                                                    data: "  BSD:::",
                                                                    line: 8,
                                                                    col: 1,
                                                                    offset: 104,
                                                                },
                                                                anchor: None,
                                                                anchor_reftext: None,
                                                                attrlist: None,
                                                            },),],
                                                            source: Span {
                                                                data: "  BSD:::",
                                                                line: 8,
                                                                col: 1,
                                                                offset: 104,
                                                            },
                                                            title_source: None,
                                                            title: None,
                                                            anchor: None,
                                                            anchor_reftext: None,
                                                            attrlist: None,
                                                        },),
                                                    ],
                                                    source: Span {
                                                        data: "      * Server\n  BSD:::",
                                                        line: 7,
                                                        col: 1,
                                                        offset: 89,
                                                    },
                                                    anchor: None,
                                                    anchor_reftext: None,
                                                    attrlist: None,
                                                },),
                                            ],
                                            source: Span {
                                                data: "      * Desktop\n      * Server\n  BSD:::",
                                                line: 6,
                                                col: 1,
                                                offset: 73,
                                            },
                                            title_source: None,
                                            title: None,
                                            anchor: None,
                                            anchor_reftext: None,
                                            attrlist: None,
                                        },),
                                    ],
                                    source: Span {
                                        data: "    . Ubuntu\n      * Desktop\n      * Server\n  BSD:::",
                                        line: 5,
                                        col: 1,
                                        offset: 60,
                                    },
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                                Block::ListItem(ListItem {
                                    marker: ListItemMarker::Dots(Span {
                                        data: ".",
                                        line: 9,
                                        col: 5,
                                        offset: 117,
                                    },),
                                    blocks: &[Block::Simple(SimpleBlock {
                                        content: Content {
                                            original: Span {
                                                data: "FreeBSD",
                                                line: 9,
                                                col: 7,
                                                offset: 119,
                                            },
                                            rendered: "FreeBSD",
                                        },
                                        source: Span {
                                            data: "FreeBSD",
                                            line: 9,
                                            col: 7,
                                            offset: 119,
                                        },
                                        style: SimpleBlockStyle::Paragraph,
                                        title_source: None,
                                        title: None,
                                        anchor: None,
                                        anchor_reftext: None,
                                        attrlist: None,
                                    },),],
                                    source: Span {
                                        data: "    . FreeBSD",
                                        line: 9,
                                        col: 1,
                                        offset: 113,
                                    },
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                                Block::ListItem(ListItem {
                                    marker: ListItemMarker::Dots(Span {
                                        data: ".",
                                        line: 10,
                                        col: 5,
                                        offset: 131,
                                    },),
                                    blocks: &[Block::Simple(SimpleBlock {
                                        content: Content {
                                            original: Span {
                                                data: "NetBSD",
                                                line: 10,
                                                col: 7,
                                                offset: 133,
                                            },
                                            rendered: "NetBSD",
                                        },
                                        source: Span {
                                            data: "NetBSD",
                                            line: 10,
                                            col: 7,
                                            offset: 133,
                                        },
                                        style: SimpleBlockStyle::Paragraph,
                                        title_source: None,
                                        title: None,
                                        anchor: None,
                                        anchor_reftext: None,
                                        attrlist: None,
                                    },),],
                                    source: Span {
                                        data: "    . NetBSD",
                                        line: 10,
                                        col: 1,
                                        offset: 127,
                                    },
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                            ],
                            source: Span {
                                data: "  Linux:::\n    . Fedora\n      * Desktop\n    . Ubuntu\n      * Desktop\n      * Server\n  BSD:::\n    . FreeBSD\n    . NetBSD",
                                line: 2,
                                col: 1,
                                offset: 20,
                            },
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "Operating Systems::\n  Linux:::\n    . Fedora\n      * Desktop\n    . Ubuntu\n      * Desktop\n      * Server\n  BSD:::\n    . FreeBSD\n    . NetBSD",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::DefinedTerm {
                            term: Content {
                                original: Span {
                                    data: "Cloud Providers",
                                    line: 12,
                                    col: 1,
                                    offset: 141,
                                },
                                rendered: "Cloud Providers",
                            },
                            marker: Span {
                                data: "::",
                                line: 12,
                                col: 16,
                                offset: 156,
                            },
                            source: Span {
                                data: "Cloud Providers::",
                                line: 12,
                                col: 1,
                                offset: 141,
                            },
                        },
                        blocks: &[Block::List(ListBlock {
                            type_: ListType::Description,
                            items: &[
                                Block::ListItem(ListItem {
                                    marker: ListItemMarker::DefinedTerm {
                                        term: Content {
                                            original: Span {
                                                data: "PaaS",
                                                line: 13,
                                                col: 3,
                                                offset: 161,
                                            },
                                            rendered: "PaaS",
                                        },
                                        marker: Span {
                                            data: ":::",
                                            line: 13,
                                            col: 7,
                                            offset: 165,
                                        },
                                        source: Span {
                                            data: "PaaS:::",
                                            line: 13,
                                            col: 3,
                                            offset: 161,
                                        },
                                    },
                                    blocks: &[],
                                    source: Span {
                                        data: "  PaaS:::",
                                        line: 13,
                                        col: 1,
                                        offset: 159,
                                    },
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                                Block::ListItem(ListItem {
                                    marker: ListItemMarker::Dots(Span {
                                        data: ".",
                                        line: 14,
                                        col: 5,
                                        offset: 173,
                                    },),
                                    blocks: &[Block::Simple(SimpleBlock {
                                        content: Content {
                                            original: Span {
                                                data: "OpenShift",
                                                line: 14,
                                                col: 7,
                                                offset: 175,
                                            },
                                            rendered: "OpenShift",
                                        },
                                        source: Span {
                                            data: "OpenShift",
                                            line: 14,
                                            col: 7,
                                            offset: 175,
                                        },
                                        style: SimpleBlockStyle::Paragraph,
                                        title_source: None,
                                        title: None,
                                        anchor: None,
                                        anchor_reftext: None,
                                        attrlist: None,
                                    },),],
                                    source: Span {
                                        data: "    . OpenShift",
                                        line: 14,
                                        col: 1,
                                        offset: 169,
                                    },
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                                Block::ListItem(ListItem {
                                    marker: ListItemMarker::Dots(Span {
                                        data: ".",
                                        line: 15,
                                        col: 5,
                                        offset: 189,
                                    },),
                                    blocks: &[
                                        Block::Simple(SimpleBlock {
                                            content: Content {
                                                original: Span {
                                                    data: "CloudBees",
                                                    line: 15,
                                                    col: 7,
                                                    offset: 191,
                                                },
                                                rendered: "CloudBees",
                                            },
                                            source: Span {
                                                data: "CloudBees",
                                                line: 15,
                                                col: 7,
                                                offset: 191,
                                            },
                                            style: SimpleBlockStyle::Paragraph,
                                            title_source: None,
                                            title: None,
                                            anchor: None,
                                            anchor_reftext: None,
                                            attrlist: None,
                                        },),
                                        Block::List(ListBlock {
                                            type_: ListType::Description,
                                            items: &[Block::ListItem(ListItem {
                                                marker: ListItemMarker::DefinedTerm {
                                                    term: Content {
                                                        original: Span {
                                                            data: "IaaS",
                                                            line: 16,
                                                            col: 3,
                                                            offset: 203,
                                                        },
                                                        rendered: "IaaS",
                                                    },
                                                    marker: Span {
                                                        data: ":::",
                                                        line: 16,
                                                        col: 7,
                                                        offset: 207,
                                                    },
                                                    source: Span {
                                                        data: "IaaS:::",
                                                        line: 16,
                                                        col: 3,
                                                        offset: 203,
                                                    },
                                                },
                                                blocks: &[],
                                                source: Span {
                                                    data: "  IaaS:::",
                                                    line: 16,
                                                    col: 1,
                                                    offset: 201,
                                                },
                                                anchor: None,
                                                anchor_reftext: None,
                                                attrlist: None,
                                            },),],
                                            source: Span {
                                                data: "  IaaS:::",
                                                line: 16,
                                                col: 1,
                                                offset: 201,
                                            },
                                            title_source: None,
                                            title: None,
                                            anchor: None,
                                            anchor_reftext: None,
                                            attrlist: None,
                                        },),
                                    ],
                                    source: Span {
                                        data: "    . CloudBees\n  IaaS:::",
                                        line: 15,
                                        col: 1,
                                        offset: 185,
                                    },
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                                Block::ListItem(ListItem {
                                    marker: ListItemMarker::Dots(Span {
                                        data: ".",
                                        line: 17,
                                        col: 5,
                                        offset: 215,
                                    },),
                                    blocks: &[Block::Simple(SimpleBlock {
                                        content: Content {
                                            original: Span {
                                                data: "Amazon EC2",
                                                line: 17,
                                                col: 7,
                                                offset: 217,
                                            },
                                            rendered: "Amazon EC2",
                                        },
                                        source: Span {
                                            data: "Amazon EC2",
                                            line: 17,
                                            col: 7,
                                            offset: 217,
                                        },
                                        style: SimpleBlockStyle::Paragraph,
                                        title_source: None,
                                        title: None,
                                        anchor: None,
                                        anchor_reftext: None,
                                        attrlist: None,
                                    },),],
                                    source: Span {
                                        data: "    . Amazon EC2",
                                        line: 17,
                                        col: 1,
                                        offset: 211,
                                    },
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                                Block::ListItem(ListItem {
                                    marker: ListItemMarker::Dots(Span {
                                        data: ".",
                                        line: 18,
                                        col: 5,
                                        offset: 232,
                                    },),
                                    blocks: &[Block::Simple(SimpleBlock {
                                        content: Content {
                                            original: Span {
                                                data: "Rackspace",
                                                line: 18,
                                                col: 7,
                                                offset: 234,
                                            },
                                            rendered: "Rackspace",
                                        },
                                        source: Span {
                                            data: "Rackspace",
                                            line: 18,
                                            col: 7,
                                            offset: 234,
                                        },
                                        style: SimpleBlockStyle::Paragraph,
                                        title_source: None,
                                        title: None,
                                        anchor: None,
                                        anchor_reftext: None,
                                        attrlist: None,
                                    },),],
                                    source: Span {
                                        data: "    . Rackspace",
                                        line: 18,
                                        col: 1,
                                        offset: 228,
                                    },
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                            ],
                            source: Span {
                                data: "  PaaS:::\n    . OpenShift\n    . CloudBees\n  IaaS:::\n    . Amazon EC2\n    . Rackspace",
                                line: 13,
                                col: 1,
                                offset: 159,
                            },
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "Cloud Providers::\n  PaaS:::\n    . OpenShift\n    . CloudBees\n  IaaS:::\n    . Amazon EC2\n    . Rackspace",
                            line: 12,
                            col: 1,
                            offset: 141,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: "Operating Systems::\n  Linux:::\n    . Fedora\n      * Desktop\n    . Ubuntu\n      * Desktop\n      * Server\n  BSD:::\n    . FreeBSD\n    . NetBSD\n\nCloud Providers::\n  PaaS:::\n    . OpenShift\n    . CloudBees\n  IaaS:::\n    . Amazon EC2\n    . Rackspace",
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
                data: "Operating Systems::\n  Linux:::\n    . Fedora\n      * Desktop\n    . Ubuntu\n      * Desktop\n      * Server\n  BSD:::\n    . FreeBSD\n    . NetBSD\n\nCloud Providers::\n  PaaS:::\n    . OpenShift\n    . CloudBees\n  IaaS:::\n    . Amazon EC2\n    . Rackspace",
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
