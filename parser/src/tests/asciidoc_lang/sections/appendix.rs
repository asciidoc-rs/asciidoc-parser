use crate::{document::RefType, tests::prelude::*};

track_file!("docs/modules/sections/pages/appendix.adoc");

non_normative!(
    r#"
= Appendix

The `appendix` section style can be used in books and articles, and it can have subsections.
While the AsciiDoc structure allows appendices to be placed anywhere, it's customary to place them near the end of the document.

"#
);

#[test]
fn appendix_section_syntax() {
    verifies!(
        r#"
== Appendix section syntax

For articles, the appendix must be defined as a level 1 section (`==`).
For example:

[source]
----
include::example$appendix.adoc[tag=appx-article]
----

The table of contents will appear as follows:

----
include::example$appendix.adoc[tag=appx-article-out]
----

"#
    );

    // NOTE: Removed `appendix-caption` and `toc` flags because those aren't
    // supported (yet?).
    let doc = Parser::default().parse("= Article Title\n:sectnums:\n\n== Section\n\n=== Subsection\n\n[appendix]\n== First Appendix\n\n=== First Subsection\n\n=== Second Subsection\n\n[appendix]\n== Second Appendix");

    assert_eq!(
        doc,
        Document {
            header: Header {
                title_source: Some(Span {
                    data: "Article Title",
                    line: 1,
                    col: 3,
                    offset: 2,
                },),
                title: Some("Article Title",),
                attributes: &[Attribute {
                    name: Span {
                        data: "sectnums",
                        line: 2,
                        col: 2,
                        offset: 17,
                    },
                    value_source: None,
                    value: InterpretedValue::Set,
                    source: Span {
                        data: ":sectnums:",
                        line: 2,
                        col: 1,
                        offset: 16,
                    },
                },],
                author_line: None,
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= Article Title\n:sectnums:",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[
                Block::Section(SectionBlock {
                    level: 1,
                    section_title: Content {
                        original: Span {
                            data: "Section",
                            line: 4,
                            col: 4,
                            offset: 31,
                        },
                        rendered: "Section",
                    },
                    blocks: &[Block::Section(SectionBlock {
                        level: 2,
                        section_title: Content {
                            original: Span {
                                data: "Subsection",
                                line: 6,
                                col: 5,
                                offset: 44,
                            },
                            rendered: "Subsection",
                        },
                        blocks: &[],
                        source: Span {
                            data: "=== Subsection",
                            line: 6,
                            col: 1,
                            offset: 40,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                        section_type: SectionType::Normal,
                        section_id: Some("_subsection",),
                        section_number: Some(SectionNumber {
                            section_type: SectionType::Normal,
                            components: &[1, 1,],
                        },),
                    },),],
                    source: Span {
                        data: "== Section\n\n=== Subsection",
                        line: 4,
                        col: 1,
                        offset: 28,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    section_type: SectionType::Normal,
                    section_id: Some("_section",),
                    section_number: Some(SectionNumber {
                        section_type: SectionType::Normal,
                        components: &[1,],
                    },),
                },),
                Block::Section(SectionBlock {
                    level: 1,
                    section_title: Content {
                        original: Span {
                            data: "First Appendix",
                            line: 9,
                            col: 4,
                            offset: 70,
                        },
                        rendered: "First Appendix",
                    },
                    blocks: &[
                        Block::Section(SectionBlock {
                            level: 2,
                            section_title: Content {
                                original: Span {
                                    data: "First Subsection",
                                    line: 11,
                                    col: 5,
                                    offset: 90,
                                },
                                rendered: "First Subsection",
                            },
                            blocks: &[],
                            source: Span {
                                data: "=== First Subsection",
                                line: 11,
                                col: 1,
                                offset: 86,
                            },
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                            section_type: SectionType::Appendix,
                            section_id: Some("_first_subsection",),
                            section_number: Some(SectionNumber {
                                section_type: SectionType::Appendix,
                                components: &[1, 1,],
                            },),
                        },),
                        Block::Section(SectionBlock {
                            level: 2,
                            section_title: Content {
                                original: Span {
                                    data: "Second Subsection",
                                    line: 13,
                                    col: 5,
                                    offset: 112,
                                },
                                rendered: "Second Subsection",
                            },
                            blocks: &[],
                            source: Span {
                                data: "=== Second Subsection",
                                line: 13,
                                col: 1,
                                offset: 108,
                            },
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                            section_type: SectionType::Appendix,
                            section_id: Some("_second_subsection",),
                            section_number: Some(SectionNumber {
                                section_type: SectionType::Appendix,
                                components: &[1, 2,],
                            },),
                        },),
                    ],
                    source: Span {
                        data: "[appendix]\n== First Appendix\n\n=== First Subsection\n\n=== Second Subsection",
                        line: 8,
                        col: 1,
                        offset: 56,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: None,
                            value: "appendix",
                            shorthand_items: &["appendix"],
                        },],
                        anchor: None,
                        source: Span {
                            data: "appendix",
                            line: 8,
                            col: 2,
                            offset: 57,
                        },
                    },),
                    section_type: SectionType::Appendix,
                    section_id: Some("_first_appendix",),
                    section_number: Some(SectionNumber {
                        section_type: SectionType::Appendix,
                        components: &[1,],
                    },),
                },),
                Block::Section(SectionBlock {
                    level: 1,
                    section_title: Content {
                        original: Span {
                            data: "Second Appendix",
                            line: 16,
                            col: 4,
                            offset: 145,
                        },
                        rendered: "Second Appendix",
                    },
                    blocks: &[],
                    source: Span {
                        data: "[appendix]\n== Second Appendix",
                        line: 15,
                        col: 1,
                        offset: 131,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: None,
                            value: "appendix",
                            shorthand_items: &["appendix"],
                        },],
                        anchor: None,
                        source: Span {
                            data: "appendix",
                            line: 15,
                            col: 2,
                            offset: 132,
                        },
                    },),
                    section_type: SectionType::Appendix,
                    section_id: Some("_second_appendix",),
                    section_number: Some(SectionNumber {
                        section_type: SectionType::Appendix,
                        components: &[2,],
                    },),
                },),
            ],
            source: Span {
                data: "= Article Title\n:sectnums:\n\n== Section\n\n=== Subsection\n\n[appendix]\n== First Appendix\n\n=== First Subsection\n\n=== Second Subsection\n\n[appendix]\n== Second Appendix",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog {
                refs: HashMap::from([
                    (
                        "_first_appendix",
                        RefEntry {
                            id: "_first_appendix",
                            reftext: Some("First Appendix",),
                            ref_type: RefType::Section,
                        },
                    ),
                    (
                        "_first_subsection",
                        RefEntry {
                            id: "_first_subsection",
                            reftext: Some("First Subsection",),
                            ref_type: RefType::Section,
                        },
                    ),
                    (
                        "_second_appendix",
                        RefEntry {
                            id: "_second_appendix",
                            reftext: Some("Second Appendix",),
                            ref_type: RefType::Section,
                        },
                    ),
                    (
                        "_second_subsection",
                        RefEntry {
                            id: "_second_subsection",
                            reftext: Some("Second Subsection",),
                            ref_type: RefType::Section,
                        },
                    ),
                    (
                        "_section",
                        RefEntry {
                            id: "_section",
                            reftext: Some("Section",),
                            ref_type: RefType::Section,
                        },
                    ),
                    (
                        "_subsection",
                        RefEntry {
                            id: "_subsection",
                            reftext: Some("Subsection",),
                            ref_type: RefType::Section,
                        },
                    ),
                ]),
                reftext_to_id: HashMap::from([
                    ("First Appendix", "_first_appendix",),
                    ("First Subsection", "_first_subsection",),
                    ("Second Appendix", "_second_appendix",),
                    ("Second Subsection", "_second_subsection",),
                    ("Section", "_section",),
                    ("Subsection", "_subsection",),
                ]),
            },
        }
    );
}

// Treating this part as non-normative because we don't support `doctype`.
non_normative!(
    r#"
For books, the appendix must be defined as a level 1 section (`==`) if you want the appendix to be a adjacent to the chapters.
In a multi-part book, if you want the appendix to be adjacent to other parts, the appendix must be defined as a level 0 section (`=`).
In either case, the first subsection of the appendix must be a level 2 section (`===`).

The following example shows how to define an appendix for a multi-part book.

[source]
----
include::example$appendix.adoc[tag=appx-book]
----

The table of contents will appear as follows:

----
include::example$appendix.adoc[tag=appx-book-out]
----

"#
);

// TO DO: Adapt the appendix label portion. Not sure how to handle this yet.
