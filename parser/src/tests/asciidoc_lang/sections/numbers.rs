use crate::{document::RefType, tests::prelude::*};

track_file!("docs/modules/sections/pages/numbers.adoc");

non_normative!(
    r#"
= Section Numbers
// New page, content taken from sections.adoc

"#
);

#[test]
fn turn_on_section_numbers() {
    verifies!(
        r#"
== Turn on section numbers

Sections aren't numbered by default.
However, you can enable this feature by setting the attribute `sectnums`.

[source]
----
= Title
:sectnums:
----

When `sectnums` is set, level 1 (`==`) through level 3 (`====`) section titles are prefixed with arabic numbers in the form of _1._, _1.1._, etc.
"#
    );

    let doc = Parser::default()
        .parse("= Title\n:sectnums:\n\n== Level 1\n\n=== Level 2\n\n==== Level 3\n\n===== Level 4");

    assert_eq!(
        doc,
        Document {
            header: Header {
                title_source: Some(Span {
                    data: "Title",
                    line: 1,
                    col: 3,
                    offset: 2,
                },),
                title: Some("Title",),
                attributes: &[Attribute {
                    name: Span {
                        data: "sectnums",
                        line: 2,
                        col: 2,
                        offset: 9,
                    },
                    value_source: None,
                    value: InterpretedValue::Set,
                    source: Span {
                        data: ":sectnums:",
                        line: 2,
                        col: 1,
                        offset: 8,
                    },
                },],
                author_line: None,
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= Title\n:sectnums:",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[Block::Section(SectionBlock {
                level: 1,
                section_title: Content {
                    original: Span {
                        data: "Level 1",
                        line: 4,
                        col: 4,
                        offset: 23,
                    },
                    rendered: "Level 1",
                },
                blocks: &[Block::Section(SectionBlock {
                    level: 2,
                    section_title: Content {
                        original: Span {
                            data: "Level 2",
                            line: 6,
                            col: 5,
                            offset: 36,
                        },
                        rendered: "Level 2",
                    },
                    blocks: &[Block::Section(SectionBlock {
                        level: 3,
                        section_title: Content {
                            original: Span {
                                data: "Level 3",
                                line: 8,
                                col: 6,
                                offset: 50,
                            },
                            rendered: "Level 3",
                        },
                        blocks: &[Block::Section(SectionBlock {
                            level: 4,
                            section_title: Content {
                                original: Span {
                                    data: "Level 4",
                                    line: 10,
                                    col: 7,
                                    offset: 65,
                                },
                                rendered: "Level 4",
                            },
                            blocks: &[],
                            source: Span {
                                data: "===== Level 4",
                                line: 10,
                                col: 1,
                                offset: 59,
                            },
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                            section_type: SectionType::Normal,
                            section_id: Some("_level_4",),
                            section_number: None,
                        },),],
                        source: Span {
                            data: "==== Level 3\n\n===== Level 4",
                            line: 8,
                            col: 1,
                            offset: 45,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                        section_type: SectionType::Normal,
                        section_id: Some("_level_3",),
                        section_number: Some(SectionNumber {
                            section_type: SectionType::Normal,
                            components: &[1, 1, 1,],
                        },),
                    },),],
                    source: Span {
                        data: "=== Level 2\n\n==== Level 3\n\n===== Level 4",
                        line: 6,
                        col: 1,
                        offset: 32,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    section_type: SectionType::Normal,
                    section_id: Some("_level_2",),
                    section_number: Some(SectionNumber {
                        section_type: SectionType::Normal,
                        components: &[1, 1,],
                    },),
                },),],
                source: Span {
                    data: "== Level 1\n\n=== Level 2\n\n==== Level 3\n\n===== Level 4",
                    line: 4,
                    col: 1,
                    offset: 20,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
                section_type: SectionType::Normal,
                section_id: Some("_level_1",),
                section_number: Some(SectionNumber {
                    section_type: SectionType::Normal,
                    components: &[1,]
                },),
            },),],
            source: Span {
                data: "= Title\n:sectnums:\n\n== Level 1\n\n=== Level 2\n\n==== Level 3\n\n===== Level 4",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog {
                refs: HashMap::from([
                    (
                        "_level_1",
                        RefEntry {
                            id: "_level_1",
                            reftext: Some("Level 1",),
                            ref_type: RefType::Section,
                        },
                    ),
                    (
                        "_level_2",
                        RefEntry {
                            id: "_level_2",
                            reftext: Some("Level 2",),
                            ref_type: RefType::Section,
                        },
                    ),
                    (
                        "_level_3",
                        RefEntry {
                            id: "_level_3",
                            reftext: Some("Level 3",),
                            ref_type: RefType::Section,
                        },
                    ),
                    (
                        "_level_4",
                        RefEntry {
                            id: "_level_4",
                            reftext: Some("Level 4",),
                            ref_type: RefType::Section,
                        },
                    ),
                ]),
                reftext_to_id: HashMap::from([
                    ("Level 1", "_level_1",),
                    ("Level 2", "_level_2",),
                    ("Level 3", "_level_3",),
                    ("Level 4", "_level_4",),
                ]),
            },
        }
    );
}

non_normative!(
    r#"
Section numbers can be set and unset via the document header, CLI, and API.
Once you've set `sectnums`, you can reduce or increase the section levels that get numbered in the whole document with the <<numlevels,sectnumlevels attribute>>.
You can also control whether a section is numbered on a section by section basis.

"#
);

#[test]
fn toggle_section_numbers() {
    verifies!(
        r#"
=== Toggle section numbers on or off per section

The `sectnums` attribute is a unique attribute.
It's a [.term]*flexible attribute*, which means it can be set and unset midstream in a document, even if it is enabled through the API or CLI.
This allows you to toggle numbering on and off throughout a document.

//You can also use `sectnums` above any section title in the document to toggle the auto-numbering setting.
To turn off numbering for one or more sections, insert the attribute above the section where you want numbering to cease and unset it by adding an exclamation point to the end of its name.
To turn section numbering back on midstream, reset the attribute above the section where numbering should resume.

[source]
----
include::example$section.adoc[tag=num-off]
----

For regions of the document where section numbering is turned off, the section numbering will not be incremented.
Given the above example, the sections will be numbered as follows:

....
include::example$section.adoc[tag=num-out]
....

The section number does not increment in regions of the document where section numbers are turned off.

"#
    );

    let doc = Parser::default()
        .parse("= Title\n:sectnums:\n\n== Numbered Section\n\n:sectnums!:\n\n== Unnumbered Section\n\n== Unnumbered Section\n\n=== Unnumbered Section\n\n:sectnums:\n\n== Numbered Section");

    assert_eq!(
        doc,
        Document {
            header: Header {
                title_source: Some(Span {
                    data: "Title",
                    line: 1,
                    col: 3,
                    offset: 2,
                },),
                title: Some("Title",),
                attributes: &[Attribute {
                    name: Span {
                        data: "sectnums",
                        line: 2,
                        col: 2,
                        offset: 9,
                    },
                    value_source: None,
                    value: InterpretedValue::Set,
                    source: Span {
                        data: ":sectnums:",
                        line: 2,
                        col: 1,
                        offset: 8,
                    },
                },],
                author_line: None,
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= Title\n:sectnums:",
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
                            data: "Numbered Section",
                            line: 4,
                            col: 4,
                            offset: 23,
                        },
                        rendered: "Numbered Section",
                    },
                    blocks: &[Block::DocumentAttribute(Attribute {
                        name: Span {
                            data: "sectnums",
                            line: 6,
                            col: 2,
                            offset: 42,
                        },
                        value_source: None,
                        value: InterpretedValue::Unset,
                        source: Span {
                            data: ":sectnums!:",
                            line: 6,
                            col: 1,
                            offset: 41,
                        },
                    },),],
                    source: Span {
                        data: "== Numbered Section\n\n:sectnums!:",
                        line: 4,
                        col: 1,
                        offset: 20,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    section_type: SectionType::Normal,
                    section_id: Some("_numbered_section",),
                    section_number: Some(SectionNumber {
                        section_type: SectionType::Normal,
                        components: &[1,]
                    },),
                },),
                Block::Section(SectionBlock {
                    level: 1,
                    section_title: Content {
                        original: Span {
                            data: "Unnumbered Section",
                            line: 8,
                            col: 4,
                            offset: 57,
                        },
                        rendered: "Unnumbered Section",
                    },
                    blocks: &[],
                    source: Span {
                        data: "== Unnumbered Section",
                        line: 8,
                        col: 1,
                        offset: 54,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    section_type: SectionType::Normal,
                    section_id: Some("_unnumbered_section",),
                    section_number: None,
                },),
                Block::Section(SectionBlock {
                    level: 1,
                    section_title: Content {
                        original: Span {
                            data: "Unnumbered Section",
                            line: 10,
                            col: 4,
                            offset: 80,
                        },
                        rendered: "Unnumbered Section",
                    },
                    blocks: &[Block::Section(SectionBlock {
                        level: 2,
                        section_title: Content {
                            original: Span {
                                data: "Unnumbered Section",
                                line: 12,
                                col: 5,
                                offset: 104,
                            },
                            rendered: "Unnumbered Section",
                        },
                        blocks: &[Block::DocumentAttribute(Attribute {
                            name: Span {
                                data: "sectnums",
                                line: 14,
                                col: 2,
                                offset: 125,
                            },
                            value_source: None,
                            value: InterpretedValue::Set,
                            source: Span {
                                data: ":sectnums:",
                                line: 14,
                                col: 1,
                                offset: 124,
                            },
                        },),],
                        source: Span {
                            data: "=== Unnumbered Section\n\n:sectnums:",
                            line: 12,
                            col: 1,
                            offset: 100,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                        section_type: SectionType::Normal,
                        section_id: Some("_unnumbered_section-2",),
                        section_number: None,
                    },),],
                    source: Span {
                        data: "== Unnumbered Section\n\n=== Unnumbered Section\n\n:sectnums:",
                        line: 10,
                        col: 1,
                        offset: 77,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    section_type: SectionType::Normal,
                    section_id: Some("_unnumbered_section-3",),
                    section_number: None,
                },),
                Block::Section(SectionBlock {
                    level: 1,
                    section_title: Content {
                        original: Span {
                            data: "Numbered Section",
                            line: 16,
                            col: 4,
                            offset: 139,
                        },
                        rendered: "Numbered Section",
                    },
                    blocks: &[],
                    source: Span {
                        data: "== Numbered Section",
                        line: 16,
                        col: 1,
                        offset: 136,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    section_type: SectionType::Normal,
                    section_id: Some("_numbered_section-2",),
                    section_number: Some(SectionNumber {
                        section_type: SectionType::Normal,
                        components: &[2,]
                    },),
                },),
            ],
            source: Span {
                data: "= Title\n:sectnums:\n\n== Numbered Section\n\n:sectnums!:\n\n== Unnumbered Section\n\n== Unnumbered Section\n\n=== Unnumbered Section\n\n:sectnums:\n\n== Numbered Section",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog {
                refs: HashMap::from([
                    (
                        "_numbered_section",
                        RefEntry {
                            id: "_numbered_section",
                            reftext: Some("Numbered Section",),
                            ref_type: RefType::Section,
                        },
                    ),
                    (
                        "_numbered_section-2",
                        RefEntry {
                            id: "_numbered_section-2",
                            reftext: Some("Numbered Section",),
                            ref_type: RefType::Section,
                        },
                    ),
                    (
                        "_unnumbered_section",
                        RefEntry {
                            id: "_unnumbered_section",
                            reftext: Some("Unnumbered Section",),
                            ref_type: RefType::Section,
                        },
                    ),
                    (
                        "_unnumbered_section-2",
                        RefEntry {
                            id: "_unnumbered_section-2",
                            reftext: Some("Unnumbered Section",),
                            ref_type: RefType::Section,
                        },
                    ),
                    (
                        "_unnumbered_section-3",
                        RefEntry {
                            id: "_unnumbered_section-3",
                            reftext: Some("Unnumbered Section",),
                            ref_type: RefType::Section,
                        },
                    ),
                ]),
                reftext_to_id: HashMap::from([
                    ("Numbered Section", "_numbered_section",),
                    ("Unnumbered Section", "_unnumbered_section",),
                ]),
            },
        }
    );
}

#[test]
fn order_of_precedence() {
    verifies!(
        r#"
=== sectnums order of precedence

If `sectnums` is set on the command line or API, it overrides the value set in the document header, but it does not prevent the document from toggling the value for regions of the document.

If it is unset (`sectnums!`) on the command line or API, then the numbers are disabled regardless of the setting within the document.

"#
    );

    let doc = Parser::default().with_intrinsic_attribute_bool("sectnums", false, ModificationContext::ApiOrDocumentBody)
        .parse("= Title\n:sectnums:\n\n== Numbered Section\n\n:sectnums!:\n\n== Unnumbered Section\n\n== Unnumbered Section\n\n=== Unnumbered Section\n\n:sectnums:\n\n== Numbered Section");

    let mut section_blocks = doc.nested_blocks().filter_map(|b| {
        if let crate::blocks::Block::Section(section_block) = b {
            Some(section_block)
        } else {
            None
        }
    });

    // Numbered Section
    let sb = section_blocks.next().unwrap();
    assert!(sb.section_number().is_none());

    // Unnumbered Section (first)
    let sb = section_blocks.next().unwrap();
    assert!(sb.section_number().is_none());

    // Unnumbered Section (second)
    let sb = section_blocks.next().unwrap();
    assert!(sb.section_number().is_none());

    // Numbered Section
    let sb = section_blocks.next().unwrap();
    assert_eq!(
        sb.section_number().unwrap(),
        &SectionNumber {
            section_type: SectionType::Normal,
            components: &[1]
        }
    );

    assert!(section_blocks.next().is_none());
}

#[test]
fn numlevels() {
    verifies!(
        r#"
[#numlevels]
== Specify the section levels that are numbered

When `sectnums` is set, level 1 (`==`) through level 3 (`====`) section titles are numbered by default.
You can increase or reduce the section level limit by setting the `sectnumlevels` attribute and assigning it the section level you want it to number.
The `sectnumlevels` attribute accepts a value of 0 through 5, and it can only be set in the document header.

[source]
----
include::example$section.adoc[tag=sectnuml]
----
<.> When the `sectnumlevels` attribute is assigned a value of `2`, level 3 through 5 section titles are not numbered.
// (i.e., not prefixed with a number).

"#
    );

    let doc = Parser::default().parse(
        "= Title\n:sectnums:\n:sectnumlevels: 2\n\n== Level 1\n\n=== Level 2\n\n==== Level 4",
    );

    assert_eq!(
        doc,
        Document {
            header: Header {
                title_source: Some(Span {
                    data: "Title",
                    line: 1,
                    col: 3,
                    offset: 2,
                },),
                title: Some("Title",),
                attributes: &[
                    Attribute {
                        name: Span {
                            data: "sectnums",
                            line: 2,
                            col: 2,
                            offset: 9,
                        },
                        value_source: None,
                        value: InterpretedValue::Set,
                        source: Span {
                            data: ":sectnums:",
                            line: 2,
                            col: 1,
                            offset: 8,
                        },
                    },
                    Attribute {
                        name: Span {
                            data: "sectnumlevels",
                            line: 3,
                            col: 2,
                            offset: 20,
                        },
                        value_source: Some(Span {
                            data: "2",
                            line: 3,
                            col: 17,
                            offset: 35,
                        },),
                        value: InterpretedValue::Value("2",),
                        source: Span {
                            data: ":sectnumlevels: 2",
                            line: 3,
                            col: 1,
                            offset: 19,
                        },
                    },
                ],
                author_line: None,
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= Title\n:sectnums:\n:sectnumlevels: 2",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[Block::Section(SectionBlock {
                level: 1,
                section_title: Content {
                    original: Span {
                        data: "Level 1",
                        line: 5,
                        col: 4,
                        offset: 41,
                    },
                    rendered: "Level 1",
                },
                blocks: &[Block::Section(SectionBlock {
                    level: 2,
                    section_title: Content {
                        original: Span {
                            data: "Level 2",
                            line: 7,
                            col: 5,
                            offset: 54,
                        },
                        rendered: "Level 2",
                    },
                    blocks: &[Block::Section(SectionBlock {
                        level: 3,
                        section_title: Content {
                            original: Span {
                                data: "Level 4",
                                line: 9,
                                col: 6,
                                offset: 68,
                            },
                            rendered: "Level 4",
                        },
                        blocks: &[],
                        source: Span {
                            data: "==== Level 4",
                            line: 9,
                            col: 1,
                            offset: 63,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                        section_type: SectionType::Normal,
                        section_id: Some("_level_4",),
                        section_number: None,
                    },),],
                    source: Span {
                        data: "=== Level 2\n\n==== Level 4",
                        line: 7,
                        col: 1,
                        offset: 50,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    section_type: SectionType::Normal,
                    section_id: Some("_level_2",),
                    section_number: Some(SectionNumber {
                        section_type: SectionType::Normal,
                        components: &[1, 1,],
                    },),
                },),],
                source: Span {
                    data: "== Level 1\n\n=== Level 2\n\n==== Level 4",
                    line: 5,
                    col: 1,
                    offset: 38,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
                section_type: SectionType::Normal,
                section_id: Some("_level_1",),
                section_number: Some(SectionNumber {
                    section_type: SectionType::Normal,
                    components: &[1,]
                },),
            },),],
            source: Span {
                data: "= Title\n:sectnums:\n:sectnumlevels: 2\n\n== Level 1\n\n=== Level 2\n\n==== Level 4",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog {
                refs: HashMap::from([
                    (
                        "_level_1",
                        RefEntry {
                            id: "_level_1",
                            reftext: Some("Level 1",),
                            ref_type: RefType::Section,
                        },
                    ),
                    (
                        "_level_2",
                        RefEntry {
                            id: "_level_2",
                            reftext: Some("Level 2",),
                            ref_type: RefType::Section,
                        },
                    ),
                    (
                        "_level_4",
                        RefEntry {
                            id: "_level_4",
                            reftext: Some("Level 4",),
                            ref_type: RefType::Section,
                        },
                    ),
                ]),
                reftext_to_id: HashMap::from([
                    ("Level 1", "_level_1",),
                    ("Level 2", "_level_2",),
                    ("Level 4", "_level_4",),
                ]),
            },
        }
    );
}

// Treating this section as non-normative because `doctype` is not supported in
// this crate.
non_normative!(
    r#"
When the `doctype` is `book`, level 1 sections become xref:chapters.adoc[chapters].
Therefore, a `sectnumlevels` of `4` translates to 3 levels of numbered sections inside each chapter.

Assigning `sectnumlevels` a value of `0` is effectively the same as disabling section numbering (`sectnums!`).
However, if your document is a xref:parts.adoc[multi-part book] with xref:part-numbers-and-labels.adoc#partnums[part numbering enabled], then you'd have to set `sectnumlevels` to `-1` to disable part numbering too (the equivalent of `partnums!`).
"#
);
