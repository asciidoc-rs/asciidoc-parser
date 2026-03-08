use std::collections::HashMap;

use crate::{Parser, blocks::SimpleBlockStyle, document::RefType, tests::prelude::*};

track_file!("docs/modules/blocks/pages/discrete-headings.adoc");

non_normative!(
    r#"
= Discrete Headings
:page-aliases: sections:discrete-titles.adoc, sections:discrete-headings.adoc

"#
);

// These conditions are verified in the discrete_headings test suite in
// section.rs.
verifies!(
    r#"
A discrete heading is declared and styled in a manner similar to that of a section title, but:

* it's not part of the section hierarchy,
* it can be nested in other blocks,
* it cannot have any child blocks,
* it's not included in the table of contents.

In other words, it's a unique block element that looks like a section title, but is not an offshoot of a section title.

The `discrete` style effectively demotes the section title to a normal heading.
Discrete headings are the closest match to headings in other markup languages such as Markdown.

"#
);

#[test]
fn discrete_attribute() {
    verifies!(
        r#"
To make a discrete heading, add the `discrete` attribute to any section title.
Here's an example of a discrete heading in use.

[source]
----
**** <.>
Discrete headings are useful for making headings inside of other blocks, like this sidebar.

[discrete] <.>
== Discrete Heading <.>

Discrete headings can be used where sections are not permitted.
****
----
<.> A delimiter line that indicates the start of a sidebar block.
<.> Set the `discrete` attribute above the section title to demote it to a discrete heading.
<.> The discrete heading is designated by one to six equal signs, just like a regular section title.

"#,
    );

    let doc = Parser::default()
        .parse("****\nDiscrete headings are useful for making headings inside of other blocks, like this sidebar.\n\n[discrete]\n== Discrete Heading\n\nDiscrete headings can be used where sections are not permitted.\n****");

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
                blocks: &[
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "Discrete headings are useful for making headings inside of other blocks, like this sidebar.",
                                line: 2,
                                col: 1,
                                offset: 5,
                            },
                            rendered: "Discrete headings are useful for making headings inside of other blocks, like this sidebar.",
                        },
                        source: Span {
                            data: "Discrete headings are useful for making headings inside of other blocks, like this sidebar.",
                            line: 2,
                            col: 1,
                            offset: 5,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::Section(SectionBlock {
                        level: 1,
                        section_title: Content {
                            original: Span {
                                data: "Discrete Heading",
                                line: 5,
                                col: 4,
                                offset: 112,
                            },
                            rendered: "Discrete Heading",
                        },
                        blocks: &[],
                        source: Span {
                            data: "[discrete]\n== Discrete Heading",
                            line: 4,
                            col: 1,
                            offset: 98,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: Some(Attrlist {
                            attributes: &[ElementAttribute {
                                name: None,
                                value: "discrete",
                                shorthand_items: &["discrete"],
                            },],
                            anchor: None,
                            source: Span {
                                data: "discrete",
                                line: 4,
                                col: 2,
                                offset: 99,
                            },
                        },),
                        section_type: SectionType::Discrete,
                        section_id: Some("_discrete_heading",),
                        section_number: None,
                    },),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "Discrete headings can be used where sections are not permitted.",
                                line: 7,
                                col: 1,
                                offset: 130,
                            },
                            rendered: "Discrete headings can be used where sections are not permitted.",
                        },
                        source: Span {
                            data: "Discrete headings can be used where sections are not permitted.",
                            line: 7,
                            col: 1,
                            offset: 130,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                context: "sidebar",
                source: Span {
                    data: "****\nDiscrete headings are useful for making headings inside of other blocks, like this sidebar.\n\n[discrete]\n== Discrete Heading\n\nDiscrete headings can be used where sections are not permitted.\n****",
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
                data: "****\nDiscrete headings are useful for making headings inside of other blocks, like this sidebar.\n\n[discrete]\n== Discrete Heading\n\nDiscrete headings can be used where sections are not permitted.\n****",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog {
                refs: HashMap::from([(
                    "_discrete_heading",
                    RefEntry {
                        id: "_discrete_heading",
                        reftext: Some("Discrete Heading",),
                        ref_type: RefType::Section,
                    },
                ),]),
                reftext_to_id: HashMap::from([("Discrete Heading", "_discrete_heading",),]),
            },
        }
    );
}

#[test]
fn float_attribute() {
    verifies!(
        r#"
Alternately, you may use the `float` attribute to identify a discrete heading.
In this context, the term "`float`" does not refer to a layout.
Rather, it means not bound to the section hierarchy.
The term comes from an older version of AsciiDoc, in which discrete headings were called Floating Titles.
DocBook refers to a discrete heading as a bridgehead, or free-floating heading.
"#
    );

    let doc = Parser::default()
        .parse("****\nDiscrete headings are useful for making headings inside of other blocks, like this sidebar.\n\n[float]\n== Float Heading\n\nFloat headings (aka discrete headings) can be used where sections are not permitted.\n****");

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
                blocks: &[
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "Discrete headings are useful for making headings inside of other blocks, like this sidebar.",
                                line: 2,
                                col: 1,
                                offset: 5,
                            },
                            rendered: "Discrete headings are useful for making headings inside of other blocks, like this sidebar.",
                        },
                        source: Span {
                            data: "Discrete headings are useful for making headings inside of other blocks, like this sidebar.",
                            line: 2,
                            col: 1,
                            offset: 5,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::Section(SectionBlock {
                        level: 1,
                        section_title: Content {
                            original: Span {
                                data: "Float Heading",
                                line: 5,
                                col: 4,
                                offset: 109,
                            },
                            rendered: "Float Heading",
                        },
                        blocks: &[],
                        source: Span {
                            data: "[float]\n== Float Heading",
                            line: 4,
                            col: 1,
                            offset: 98,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: Some(Attrlist {
                            attributes: &[ElementAttribute {
                                name: None,
                                value: "float",
                                shorthand_items: &["float"],
                            },],
                            anchor: None,
                            source: Span {
                                data: "float",
                                line: 4,
                                col: 2,
                                offset: 99,
                            },
                        },),
                        section_type: SectionType::Discrete,
                        section_id: Some("_float_heading",),
                        section_number: None,
                    },),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "Float headings (aka discrete headings) can be used where sections are not permitted.",
                                line: 7,
                                col: 1,
                                offset: 124,
                            },
                            rendered: "Float headings (aka discrete headings) can be used where sections are not permitted.",
                        },
                        source: Span {
                            data: "Float headings (aka discrete headings) can be used where sections are not permitted.",
                            line: 7,
                            col: 1,
                            offset: 124,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                context: "sidebar",
                source: Span {
                    data: "****\nDiscrete headings are useful for making headings inside of other blocks, like this sidebar.\n\n[float]\n== Float Heading\n\nFloat headings (aka discrete headings) can be used where sections are not permitted.\n****",
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
                data: "****\nDiscrete headings are useful for making headings inside of other blocks, like this sidebar.\n\n[float]\n== Float Heading\n\nFloat headings (aka discrete headings) can be used where sections are not permitted.\n****",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog {
                refs: HashMap::from([(
                    "_float_heading",
                    RefEntry {
                        id: "_float_heading",
                        reftext: Some("Float Heading",),
                        ref_type: RefType::Section,
                    },
                ),]),
                reftext_to_id: HashMap::from([("Float Heading", "_float_heading",),]),
            },
        }
    );
}
