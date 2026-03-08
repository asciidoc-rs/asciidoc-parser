use crate::{document::RefType, tests::prelude::*};

track_file!("docs/modules/sections/pages/titles-and-levels.adoc");

non_normative!(
    r#"
= Section Titles and Levels

Sections partition the document into a content hierarchy.
A section is an implicit enclosure.
Each section begins with a title and ends at the next sibling section, ancestor section, or end of document.
Nested section levels must be sequential.
A section can be a child of a document or another section, but it cannot be the child of any other block (i.e., you cannot put a section inside of a delimited block or list).

"#
);

#[test]
fn section_level_syntax() {
    non_normative!(
        r#"
== Section level syntax

"#
    );

    verifies!(
        r#"
A section title marks the beginning of a section and also acts as the heading for that section.
The section title must be prefixed with a section marker, which indicates the section level.
The number of equal signs in the marker represents the section level using a 0-based index (e.g., two equal signs represents level 1).
A section marker can range from two to six equal signs and must be followed by a space.

IMPORTANT: The section title line is interpreted as paragraph text if it's found inside of a non-section block unless it marked as a xref:blocks:discrete-headings.adoc[discrete heading].

In the HTML output, the section title is represented by a heading tag.
The number of the heading tag is one more than the section level (e.g., section level 1 becomes an h2 tag).
The section level ranges from 0-5.
This limit was established primarily due to the fact that HTML only provides heading tags from h1 to h6 (making level 5 the upper limit).

.Section titles available in an article doctype
[source]
----
include::example$section.adoc[tag=base]
----

The section titles are rendered as:

====
include::example$section.adoc[tag=b-base]
====

"#
    );

    let doc = Parser::default().parse("= Document Title (Level 0)\n\n== Level 1 Section Title\n\n=== Level 2 Section Title\n\n==== Level 3 Section Title\n\n===== Level 4 Section Title\n\n====== Level 5 Section Title\n\n== Another Level 1 Section Title");

    assert_eq!(
        doc,
        Document {
            header: Header {
                title_source: Some(Span {
                    data: "Document Title (Level 0)",
                    line: 1,
                    col: 3,
                    offset: 2,
                },),
                title: Some("Document Title (Level 0)",),
                attributes: &[],
                author_line: None,
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= Document Title (Level 0)",
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
                            data: "Level 1 Section Title",
                            line: 3,
                            col: 4,
                            offset: 31,
                        },
                        rendered: "Level 1 Section Title",
                    },
                    blocks: &[Block::Section(SectionBlock {
                        level: 2,
                        section_title: Content {
                            original: Span {
                                data: "Level 2 Section Title",
                                line: 5,
                                col: 5,
                                offset: 58,
                            },
                            rendered: "Level 2 Section Title",
                        },
                        blocks: &[Block::Section(SectionBlock {
                            level: 3,
                            section_title: Content {
                                original: Span {
                                    data: "Level 3 Section Title",
                                    line: 7,
                                    col: 6,
                                    offset: 86,
                                },
                                rendered: "Level 3 Section Title",
                            },
                            blocks: &[Block::Section(SectionBlock {
                                level: 4,
                                section_title: Content {
                                    original: Span {
                                        data: "Level 4 Section Title",
                                        line: 9,
                                        col: 7,
                                        offset: 115,
                                    },
                                    rendered: "Level 4 Section Title",
                                },
                                blocks: &[Block::Section(SectionBlock {
                                    level: 5,
                                    section_title: Content {
                                        original: Span {
                                            data: "Level 5 Section Title",
                                            line: 11,
                                            col: 8,
                                            offset: 145,
                                        },
                                        rendered: "Level 5 Section Title",
                                    },
                                    blocks: &[],
                                    source: Span {
                                        data: "====== Level 5 Section Title",
                                        line: 11,
                                        col: 1,
                                        offset: 138,
                                    },
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                    section_type: SectionType::Normal,
                                    section_id: Some("_level_5_section_title"),
                                    section_number: None,
                                },),],
                                source: Span {
                                    data: "===== Level 4 Section Title\n\n====== Level 5 Section Title",
                                    line: 9,
                                    col: 1,
                                    offset: 109,
                                },
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                                section_type: SectionType::Normal,
                                section_id: Some("_level_4_section_title"),
                                section_number: None,
                            },),],
                            source: Span {
                                data: "==== Level 3 Section Title\n\n===== Level 4 Section Title\n\n====== Level 5 Section Title",
                                line: 7,
                                col: 1,
                                offset: 81,
                            },
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                            section_type: SectionType::Normal,
                            section_id: Some("_level_3_section_title"),
                            section_number: None,
                        },),],
                        source: Span {
                            data: "=== Level 2 Section Title\n\n==== Level 3 Section Title\n\n===== Level 4 Section Title\n\n====== Level 5 Section Title",
                            line: 5,
                            col: 1,
                            offset: 54,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                        section_type: SectionType::Normal,
                        section_id: Some("_level_2_section_title"),
                        section_number: None,
                    },),],
                    source: Span {
                        data: "== Level 1 Section Title\n\n=== Level 2 Section Title\n\n==== Level 3 Section Title\n\n===== Level 4 Section Title\n\n====== Level 5 Section Title",
                        line: 3,
                        col: 1,
                        offset: 28,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    section_type: SectionType::Normal,
                    section_id: Some("_level_1_section_title"),
                    section_number: None,
                },),
                Block::Section(SectionBlock {
                    level: 1,
                    section_title: Content {
                        original: Span {
                            data: "Another Level 1 Section Title",
                            line: 13,
                            col: 4,
                            offset: 171,
                        },
                        rendered: "Another Level 1 Section Title",
                    },
                    blocks: &[],
                    source: Span {
                        data: "== Another Level 1 Section Title",
                        line: 13,
                        col: 1,
                        offset: 168,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    section_type: SectionType::Normal,
                    section_id: Some("_another_level_1_section_title"),
                    section_number: None,
                },),
            ],
            source: Span {
                data: "= Document Title (Level 0)\n\n== Level 1 Section Title\n\n=== Level 2 Section Title\n\n==== Level 3 Section Title\n\n===== Level 4 Section Title\n\n====== Level 5 Section Title\n\n== Another Level 1 Section Title",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog {
                refs: HashMap::from([
                    (
                        "_level_2_section_title",
                        RefEntry {
                            id: "_level_2_section_title",
                            reftext: Some("Level 2 Section Title",),
                            ref_type: RefType::Section,
                        }
                    ),
                    (
                        "_level_4_section_title",
                        RefEntry {
                            id: "_level_4_section_title",
                            reftext: Some("Level 4 Section Title",),
                            ref_type: RefType::Section,
                        }
                    ),
                    (
                        "_another_level_1_section_title",
                        RefEntry {
                            id: "_another_level_1_section_title",
                            reftext: Some("Another Level 1 Section Title",),
                            ref_type: RefType::Section,
                        }
                    ),
                    (
                        "_level_3_section_title",
                        RefEntry {
                            id: "_level_3_section_title",
                            reftext: Some("Level 3 Section Title",),
                            ref_type: RefType::Section,
                        }
                    ),
                    (
                        "_level_1_section_title",
                        RefEntry {
                            id: "_level_1_section_title",
                            reftext: Some("Level 1 Section Title",),
                            ref_type: RefType::Section,
                        }
                    ),
                    (
                        "_level_5_section_title",
                        RefEntry {
                            id: "_level_5_section_title",
                            reftext: Some("Level 5 Section Title",),
                            ref_type: RefType::Section,
                        }
                    ),
                ]),
                reftext_to_id: HashMap::from([
                    ("Level 3 Section Title", "_level_3_section_title"),
                    ("Level 1 Section Title", "_level_1_section_title"),
                    ("Level 5 Section Title", "_level_5_section_title"),
                    ("Level 4 Section Title", "_level_4_section_title"),
                    (
                        "Another Level 1 Section Title",
                        "_another_level_1_section_title"
                    ),
                    ("Level 2 Section Title", "_level_2_section_title"),
                ]),
            },
        }
    );
}

// TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/380):
// Option to support multiple level-0 headings.
// Ignoring this rule for now.
non_normative!(
    r#"
Section levels must be nested logically.
There are two rules you must follow:

. A document can only have multiple level 0 sections if the `doctype` is set to `book`.
 ** The first level 0 section is the document title; subsequent level 0 sections represent parts.
"#
);

#[test]
fn section_levels_cant_be_skipped() {
    verifies!(
        r#"
. Section levels cannot be skipped when nesting sections (e.g., you can't nest a level 5 section directly inside a level 3 section; an intermediary level 4 section is required).

For example, the following syntax is illegal:

[source]
----
include::example$section.adoc[tag=bad]
----

Content above the first section title is designated as the document's preamble.
Once the first section title is reached, content is associated with the section it is nested in.

[source]
----
include::example$section.adoc[tag=content]
----

"#
    );

    let doc = Parser::default().parse("= Document Title\n\n= Illegal Level 0 Section (violates rule #1)\n\n== First Section\n\n==== Illegal Nested Section (violates rule #2)");

    assert_eq!(
        Document {
            header: Header {
                title_source: Some(Span {
                    data: "Document Title",
                    line: 1,
                    col: 3,
                    offset: 2,
                },),
                title: Some("Document Title",),
                attributes: &[],
                author_line: None,
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= Document Title",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[
                Block::Preamble(Preamble {
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "= Illegal Level 0 Section (violates rule #1)",
                                line: 3,
                                col: 1,
                                offset: 18,
                            },
                            rendered: "= Illegal Level 0 Section (violates rule #1)",
                        },
                        source: Span {
                            data: "= Illegal Level 0 Section (violates rule #1)",
                            line: 3,
                            col: 1,
                            offset: 18,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "= Illegal Level 0 Section (violates rule #1)",
                        line: 3,
                        col: 1,
                        offset: 18,
                    },
                },),
                Block::Section(SectionBlock {
                    level: 1,
                    section_title: Content {
                        original: Span {
                            data: "First Section",
                            line: 5,
                            col: 4,
                            offset: 67,
                        },
                        rendered: "First Section",
                    },
                    blocks: &[Block::Section(SectionBlock {
                        level: 3,
                        section_title: Content {
                            original: Span {
                                data: "Illegal Nested Section (violates rule #2)",
                                line: 7,
                                col: 6,
                                offset: 87,
                            },
                            rendered: "Illegal Nested Section (violates rule #2)",
                        },
                        blocks: &[],
                        source: Span {
                            data: "==== Illegal Nested Section (violates rule #2)",
                            line: 7,
                            col: 1,
                            offset: 82,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                        section_type: SectionType::Normal,
                        section_id: Some("_illegal_nested_section_violates_rule_2"),
                        section_number: None,
                    },),],
                    source: Span {
                        data: "== First Section\n\n==== Illegal Nested Section (violates rule #2)",
                        line: 5,
                        col: 1,
                        offset: 64,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    section_type: SectionType::Normal,
                    section_id: Some("_first_section"),
                    section_number: None,
                },),
            ],
            source: Span {
                data: "= Document Title\n\n= Illegal Level 0 Section (violates rule #1)\n\n== First Section\n\n==== Illegal Nested Section (violates rule #2)",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[
                Warning {
                    source: Span {
                        data: "= Illegal Level 0 Section (violates rule #1)",
                        line: 3,
                        col: 1,
                        offset: 18,
                    },
                    warning: WarningType::Level0SectionHeadingNotSupported,
                },
                Warning {
                    source: Span {
                        data: "==== Illegal Nested Section (violates rule #2)",
                        line: 7,
                        col: 1,
                        offset: 82,
                    },
                    warning: WarningType::SectionHeadingLevelSkipped(1, 3,),
                },
            ],
            source_map: SourceMap(&[]),
            catalog: Catalog {
                refs: HashMap::from([
                    (
                        "_first_section",
                        RefEntry {
                            id: "_first_section",
                            reftext: Some("First Section",),
                            ref_type: RefType::Section,
                        }
                    ),
                    (
                        "_illegal_nested_section_violates_rule_2",
                        RefEntry {
                            id: "_illegal_nested_section_violates_rule_2",
                            reftext: Some("Illegal Nested Section (violates rule #2)",),
                            ref_type: RefType::Section,
                        }
                    ),
                ]),
                reftext_to_id: HashMap::from([
                    ("First Section", "_first_section"),
                    (
                        "Illegal Nested Section (violates rule #2)",
                        "_illegal_nested_section_violates_rule_2"
                    ),
                ]),
            },
        },
        doc
    );
}

#[test]
fn markdown_style() {
    verifies!(
        r#"
TIP: In addition to the equals sign marker used for defining section titles, Asciidoctor recognizes the hash symbol (`#`) from Markdown.
That means the outline of a Markdown document will be converted just fine as an AsciiDoc document.

"#
    );

    let doc = Parser::default().parse("= Document Title (Level 0)\n\n## Level 1 Section Title\n\n### Level 2 Section Title\n\n#### Level 3 Section Title\n\n##### Level 4 Section Title\n\n###### Level 5 Section Title\n\n## Another Level 1 Section Title");

    assert_eq!(
        doc,
        Document {
            header: Header {
                title_source: Some(Span {
                    data: "Document Title (Level 0)",
                    line: 1,
                    col: 3,
                    offset: 2,
                },),
                title: Some("Document Title (Level 0)",),
                attributes: &[],
                author_line: None,
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= Document Title (Level 0)",
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
                            data: "Level 1 Section Title",
                            line: 3,
                            col: 4,
                            offset: 31,
                        },
                        rendered: "Level 1 Section Title",
                    },
                    blocks: &[Block::Section(SectionBlock {
                        level: 2,
                        section_title: Content {
                            original: Span {
                                data: "Level 2 Section Title",
                                line: 5,
                                col: 5,
                                offset: 58,
                            },
                            rendered: "Level 2 Section Title",
                        },
                        blocks: &[Block::Section(SectionBlock {
                            level: 3,
                            section_title: Content {
                                original: Span {
                                    data: "Level 3 Section Title",
                                    line: 7,
                                    col: 6,
                                    offset: 86,
                                },
                                rendered: "Level 3 Section Title",
                            },
                            blocks: &[Block::Section(SectionBlock {
                                level: 4,
                                section_title: Content {
                                    original: Span {
                                        data: "Level 4 Section Title",
                                        line: 9,
                                        col: 7,
                                        offset: 115,
                                    },
                                    rendered: "Level 4 Section Title",
                                },
                                blocks: &[Block::Section(SectionBlock {
                                    level: 5,
                                    section_title: Content {
                                        original: Span {
                                            data: "Level 5 Section Title",
                                            line: 11,
                                            col: 8,
                                            offset: 145,
                                        },
                                        rendered: "Level 5 Section Title",
                                    },
                                    blocks: &[],
                                    source: Span {
                                        data: "###### Level 5 Section Title",
                                        line: 11,
                                        col: 1,
                                        offset: 138,
                                    },
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                    section_type: SectionType::Normal,
                                    section_id: Some("_level_5_section_title"),
                                    section_number: None,
                                },),],
                                source: Span {
                                    data: "##### Level 4 Section Title\n\n###### Level 5 Section Title",
                                    line: 9,
                                    col: 1,
                                    offset: 109,
                                },
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                                section_type: SectionType::Normal,
                                section_id: Some("_level_4_section_title"),
                                section_number: None,
                            },),],
                            source: Span {
                                data: "#### Level 3 Section Title\n\n##### Level 4 Section Title\n\n###### Level 5 Section Title",
                                line: 7,
                                col: 1,
                                offset: 81,
                            },
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                            section_type: SectionType::Normal,
                            section_id: Some("_level_3_section_title"),
                            section_number: None,
                        },),],
                        source: Span {
                            data: "### Level 2 Section Title\n\n#### Level 3 Section Title\n\n##### Level 4 Section Title\n\n###### Level 5 Section Title",
                            line: 5,
                            col: 1,
                            offset: 54,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                        section_type: SectionType::Normal,
                        section_id: Some("_level_2_section_title"),
                        section_number: None,
                    },),],
                    source: Span {
                        data: "## Level 1 Section Title\n\n### Level 2 Section Title\n\n#### Level 3 Section Title\n\n##### Level 4 Section Title\n\n###### Level 5 Section Title",
                        line: 3,
                        col: 1,
                        offset: 28,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    section_type: SectionType::Normal,
                    section_id: Some("_level_1_section_title"),
                    section_number: None,
                },),
                Block::Section(SectionBlock {
                    level: 1,
                    section_title: Content {
                        original: Span {
                            data: "Another Level 1 Section Title",
                            line: 13,
                            col: 4,
                            offset: 171,
                        },
                        rendered: "Another Level 1 Section Title",
                    },
                    blocks: &[],
                    source: Span {
                        data: "## Another Level 1 Section Title",
                        line: 13,
                        col: 1,
                        offset: 168,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    section_type: SectionType::Normal,
                    section_id: Some("_another_level_1_section_title"),
                    section_number: None,
                },),
            ],
            source: Span {
                data: "= Document Title (Level 0)\n\n## Level 1 Section Title\n\n### Level 2 Section Title\n\n#### Level 3 Section Title\n\n##### Level 4 Section Title\n\n###### Level 5 Section Title\n\n## Another Level 1 Section Title",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog {
                refs: HashMap::from([
                    (
                        "_level_1_section_title",
                        RefEntry {
                            id: "_level_1_section_title",
                            reftext: Some("Level 1 Section Title",),
                            ref_type: RefType::Section,
                        }
                    ),
                    (
                        "_another_level_1_section_title",
                        RefEntry {
                            id: "_another_level_1_section_title",
                            reftext: Some("Another Level 1 Section Title",),
                            ref_type: RefType::Section,
                        }
                    ),
                    (
                        "_level_5_section_title",
                        RefEntry {
                            id: "_level_5_section_title",
                            reftext: Some("Level 5 Section Title",),
                            ref_type: RefType::Section,
                        }
                    ),
                    (
                        "_level_4_section_title",
                        RefEntry {
                            id: "_level_4_section_title",
                            reftext: Some("Level 4 Section Title",),
                            ref_type: RefType::Section,
                        }
                    ),
                    (
                        "_level_3_section_title",
                        RefEntry {
                            id: "_level_3_section_title",
                            reftext: Some("Level 3 Section Title",),
                            ref_type: RefType::Section,
                        }
                    ),
                    (
                        "_level_2_section_title",
                        RefEntry {
                            id: "_level_2_section_title",
                            reftext: Some("Level 2 Section Title",),
                            ref_type: RefType::Section,
                        }
                    ),
                ]),
                reftext_to_id: HashMap::from([
                    ("Level 4 Section Title", "_level_4_section_title"),
                    ("Level 2 Section Title", "_level_2_section_title"),
                    (
                        "Another Level 1 Section Title",
                        "_another_level_1_section_title"
                    ),
                    ("Level 5 Section Title", "_level_5_section_title"),
                    ("Level 1 Section Title", "_level_1_section_title"),
                    ("Level 3 Section Title", "_level_3_section_title"),
                ]),
            },
        }
    );
}

// Treating as non-normative since asciidoc-parser says nothing about how
// content is rendered.
non_normative!(
    r#"
== Titles as HTML headings

When the document is converted to HTML 5 (using the built-in `html5` backend), each section title becomes a heading element where the heading level matches the number of equal signs.
For example, a level 1 section (`==`) maps to an `<h2>` element.
"#
);
