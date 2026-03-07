use std::collections::HashMap;

use pretty_assertions_sorted::assert_eq;

use crate::{
    Parser,
    blocks::{ListType, SimpleBlockStyle},
    tests::prelude::*,
};

track_file!("docs/modules/lists/pages/qanda.adoc");

non_normative!(
    r#"
= Question and Answer Lists
:keywords: qanda, Q & A, Q and A

A question and answer (qanda) list is a special form of a description list that renders as an ordered list.
The entries are numbered using Arabic numerals starting at 1.

== Question and answer list syntax

"#
);

#[test]
fn syntax_example() {
    verifies!(
        r#"
Each entry in the description list represents one question and answer combination.
The term or terms are used as the question and the description is used as the answer.
If an entry has multiple questions, each question is rendered on a new line.

----
include::example$description.adoc[tag=qa]
----

.Rendered qanda list
====
include::example$description.adoc[tag=qa]
====
"#
    );

    let doc = Parser::default().parse(
            "[qanda]\nWhat is the answer?::\nThis is the answer.\n\nAre cameras allowed?::\nAre backpacks allowed?::\nNo.",
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
                                    data: "What is the answer?",
                                    line: 2,
                                    col: 1,
                                    offset: 8,
                                },
                                rendered: "What is the answer?",
                            },
                            marker: Span {
                                data: "::",
                                line: 2,
                                col: 20,
                                offset: 27,
                            },
                            source: Span {
                                data: "What is the answer?::",
                                line: 2,
                                col: 1,
                                offset: 8,
                            },
                        },
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "This is the answer.",
                                    line: 3,
                                    col: 1,
                                    offset: 30,
                                },
                                rendered: "This is the answer.",
                            },
                            source: Span {
                                data: "This is the answer.",
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
                        source: Span {
                            data: "What is the answer?::\nThis is the answer.",
                            line: 2,
                            col: 1,
                            offset: 8,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::DefinedTerm {
                            term: Content {
                                original: Span {
                                    data: "Are cameras allowed?",
                                    line: 5,
                                    col: 1,
                                    offset: 51,
                                },
                                rendered: "Are cameras allowed?",
                            },
                            marker: Span {
                                data: "::",
                                line: 5,
                                col: 21,
                                offset: 71,
                            },
                            source: Span {
                                data: "Are cameras allowed?::",
                                line: 5,
                                col: 1,
                                offset: 51,
                            },
                        },
                        blocks: &[],
                        source: Span {
                            data: "Are cameras allowed?::",
                            line: 5,
                            col: 1,
                            offset: 51,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::ListItem(ListItem {
                        marker: ListItemMarker::DefinedTerm {
                            term: Content {
                                original: Span {
                                    data: "Are backpacks allowed?",
                                    line: 6,
                                    col: 1,
                                    offset: 74,
                                },
                                rendered: "Are backpacks allowed?",
                            },
                            marker: Span {
                                data: "::",
                                line: 6,
                                col: 23,
                                offset: 96,
                            },
                            source: Span {
                                data: "Are backpacks allowed?::",
                                line: 6,
                                col: 1,
                                offset: 74,
                            },
                        },
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "No.",
                                    line: 7,
                                    col: 1,
                                    offset: 99,
                                },
                                rendered: "No.",
                            },
                            source: Span {
                                data: "No.",
                                line: 7,
                                col: 1,
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
                            data: "Are backpacks allowed?::\nNo.",
                            line: 6,
                            col: 1,
                            offset: 74,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: "[qanda]\nWhat is the answer?::\nThis is the answer.\n\nAre cameras allowed?::\nAre backpacks allowed?::\nNo.",
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
                        value: "qanda",
                        shorthand_items: &["qanda"],
                    },],
                    anchor: None,
                    source: Span {
                        data: "qanda",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },),],
            source: Span {
                data: "[qanda]\nWhat is the answer?::\nThis is the answer.\n\nAre cameras allowed?::\nAre backpacks allowed?::\nNo.",
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
