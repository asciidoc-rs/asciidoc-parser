use std::collections::HashMap;

use crate::{
    Parser,
    blocks::SimpleBlockStyle,
    document::RefType,
    tests::prelude::{inline_file_handler::InlineFileHandler, *},
};

track_file!("docs/modules/directives/pages/include-multiple-times-in-same-document.adoc");

non_normative!(
    r#"
= Use an Include File Multiple Times
//Include a File Multiple Times in the Same Document
//[#include-multiple]
// Title and anchor are from user-manual.adoc
// Section content from multiple-include.adoc

"#
);

#[test]
fn same_subsection_twice() {
    verifies!(
        r#"
A document can include the same file any number of times.
The problem comes if there are IDs in the included file; the output document (HTML or DocBook) will then have duplicate IDs which will make it not well-formed.
To fix this, you can reference a dynamic variable from the primary document in the ID.

For example, let's say you want to include the same subsection describing a bike chain in both the operation and maintenance chapters:

----
= Bike Manual

:chapter: operation
== Operation

\include::fragment-chain.adoc[]

:chapter: maintenance
== Maintenance

\include::fragment-chain.adoc[]
----

Write [.path]_fragment-chain.adoc_ as:

----
[id=chain-{chapter}]
=== Chain

See xref:chain-{chapter}[].
----

The first time the [.path]_fragment-chain.adoc_ file is included, the ID of the included section resolves to `chain-operation`.
The second time the file included, the ID resolves to `chain-maintenance`.

In order for this to work, you must use the long-hand forms of both the ID assignment and the cross reference.
The single quotes around the variable name in the assignment are required to force variable substitution (aka interpolation).
"#
    );

    let source = "= Bike Manual\n\n:chapter: operation\n== Operation\n\ninclude::fragment-chain.adoc[]\n\n:chapter: maintenance\n== Maintenance\n\ninclude::fragment-chain.adoc[]";

    let handler = InlineFileHandler::from_pairs([(
        "fragment-chain.adoc",
        "[id=chain-{chapter}]\n=== Chain\n\nSee xref:chain-{chapter}[].\n",
    )]);

    let doc = Parser::default()
        .with_include_file_handler(handler)
        .parse(source);

    assert_eq!(
        doc,
        Document {
            header: Header {
                title_source: Some(Span {
                    data: "Bike Manual",
                    line: 1,
                    col: 3,
                    offset: 2,
                },),
                title: Some("Bike Manual",),
                attributes: &[],
                author_line: None,
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= Bike Manual",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[
                Block::DocumentAttribute(Attribute {
                    name: Span {
                        data: "chapter",
                        line: 3,
                        col: 2,
                        offset: 16,
                    },
                    value_source: Some(Span {
                        data: "operation",
                        line: 3,
                        col: 11,
                        offset: 25,
                    },),
                    value: InterpretedValue::Value("operation",),
                    source: Span {
                        data: ":chapter: operation",
                        line: 3,
                        col: 1,
                        offset: 15,
                    },
                },),
                Block::Section(SectionBlock {
                    level: 1,
                    section_title: Content {
                        original: Span {
                            data: "Operation",
                            line: 4,
                            col: 4,
                            offset: 38,
                        },
                        rendered: "Operation",
                    },
                    blocks: &[Block::Section(SectionBlock {
                        level: 2,
                        section_title: Content {
                            original: Span {
                                data: "Chain",
                                line: 7,
                                col: 5,
                                offset: 74,
                            },
                            rendered: "Chain",
                        },
                        blocks: &[
                            Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "See xref:chain-{chapter}[].",
                                        line: 9,
                                        col: 1,
                                        offset: 81,
                                    },
                                    rendered: "See xref:chain-operation[].",
                                },
                                source: Span {
                                    data: "See xref:chain-{chapter}[].",
                                    line: 9,
                                    col: 1,
                                    offset: 81,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),
                            Block::DocumentAttribute(Attribute {
                                name: Span {
                                    data: "chapter",
                                    line: 11,
                                    col: 2,
                                    offset: 111,
                                },
                                value_source: Some(Span {
                                    data: "maintenance",
                                    line: 11,
                                    col: 11,
                                    offset: 120,
                                },),
                                value: InterpretedValue::Value("maintenance",),
                                source: Span {
                                    data: ":chapter: maintenance",
                                    line: 11,
                                    col: 1,
                                    offset: 110,
                                },
                            },),
                        ],
                        source: Span {
                            data: "[id=chain-{chapter}]\n=== Chain\n\nSee xref:chain-{chapter}[].\n\n:chapter: maintenance",
                            line: 6,
                            col: 1,
                            offset: 49,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: Some(Attrlist {
                            attributes: &[ElementAttribute {
                                name: Some("id",),
                                value: "chain-operation",
                                shorthand_items: &[],
                            },],
                            anchor: None,
                            source: Span {
                                data: "id=chain-{chapter}",
                                line: 6,
                                col: 2,
                                offset: 50,
                            },
                        },),
                        section_type: SectionType::Normal,
                        section_id: None,
                        section_number: None,
                    },),],
                    source: Span {
                        data: "== Operation\n\n[id=chain-{chapter}]\n=== Chain\n\nSee xref:chain-{chapter}[].\n\n:chapter: maintenance",
                        line: 4,
                        col: 1,
                        offset: 35,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    section_type: SectionType::Normal,
                    section_id: Some("_operation"),
                    section_number: None,
                },),
                Block::Section(SectionBlock {
                    level: 1,
                    section_title: Content {
                        original: Span {
                            data: "Maintenance",
                            line: 12,
                            col: 4,
                            offset: 135,
                        },
                        rendered: "Maintenance",
                    },
                    blocks: &[Block::Section(SectionBlock {
                        level: 2,
                        section_title: Content {
                            original: Span {
                                data: "Chain",
                                line: 15,
                                col: 5,
                                offset: 173,
                            },
                            rendered: "Chain",
                        },
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "See xref:chain-{chapter}[].",
                                    line: 17,
                                    col: 1,
                                    offset: 180,
                                },
                                rendered: "See xref:chain-maintenance[].",
                            },
                            source: Span {
                                data: "See xref:chain-{chapter}[].",
                                line: 17,
                                col: 1,
                                offset: 180,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "[id=chain-{chapter}]\n=== Chain\n\nSee xref:chain-{chapter}[].",
                            line: 14,
                            col: 1,
                            offset: 148,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: Some(Attrlist {
                            attributes: &[ElementAttribute {
                                name: Some("id",),
                                value: "chain-maintenance",
                                shorthand_items: &[],
                            },],
                            anchor: None,
                            source: Span {
                                data: "id=chain-{chapter}",
                                line: 14,
                                col: 2,
                                offset: 149,
                            },
                        },),
                        section_type: SectionType::Normal,
                        section_id: None,
                        section_number: None,
                    },),],
                    source: Span {
                        data: "== Maintenance\n\n[id=chain-{chapter}]\n=== Chain\n\nSee xref:chain-{chapter}[].",
                        line: 12,
                        col: 1,
                        offset: 132,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    section_type: SectionType::Normal,
                    section_id: Some("_maintenance"),
                    section_number: None,
                },),
            ],
            source: Span {
                data: "= Bike Manual\n\n:chapter: operation\n== Operation\n\n[id=chain-{chapter}]\n=== Chain\n\nSee xref:chain-{chapter}[].\n\n:chapter: maintenance\n== Maintenance\n\n[id=chain-{chapter}]\n=== Chain\n\nSee xref:chain-{chapter}[].",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[
                (6, SourceLine(Some("fragment-chain.adoc",), 1,),),
                (10, SourceLine(None, 7,),),
                (14, SourceLine(Some("fragment-chain.adoc",), 1,),),
            ],),
            catalog: Catalog {
                refs: HashMap::from([
                    (
                        "chain-maintenance",
                        RefEntry {
                            id: "chain-maintenance",
                            reftext: Some("Chain",),
                            ref_type: RefType::Section,
                        }
                    ),
                    (
                        "chain-operation",
                        RefEntry {
                            id: "chain-operation",
                            reftext: Some("Chain",),
                            ref_type: RefType::Section,
                        }
                    ),
                    (
                        "_maintenance",
                        RefEntry {
                            id: "_maintenance",
                            reftext: Some("Maintenance",),
                            ref_type: RefType::Section
                        }
                    ),
                    (
                        "_operation",
                        RefEntry {
                            id: "_operation",
                            reftext: Some("Operation",),
                            ref_type: RefType::Section,
                        },
                    ),
                ]),
                reftext_to_id: HashMap::from([
                    ("Maintenance", "_maintenance"),
                    ("Operation", "_operation"),
                    ("Chain", "chain-operation"),
                ])
            },
        }
    );

    // Verify line number mapping.
    // This verifies that the source map correctly tracks the original file and line
    // numbers for included content. Each line in the preprocessed document should
    // map back to either the main document (None) or the included file
    // (Some(filename)).
    let source_map = doc.source_map();

    // Main document lines.
    assert_eq!(
        source_map.original_file_and_line(1),
        Some(crate::parser::SourceLine(None, 1)) // = Bike Manual
    );
    assert_eq!(
        source_map.original_file_and_line(2),
        Some(crate::parser::SourceLine(None, 2)) // blank line
    );
    assert_eq!(
        source_map.original_file_and_line(3),
        Some(crate::parser::SourceLine(None, 3)) // :chapter: operation
    );
    assert_eq!(
        source_map.original_file_and_line(4),
        Some(crate::parser::SourceLine(None, 4)) // == Operation
    );
    assert_eq!(
        source_map.original_file_and_line(5),
        Some(crate::parser::SourceLine(None, 5)) // blank line
    );

    // First inclusion of fragment-chain.adoc.
    assert_eq!(
        source_map.original_file_and_line(6),
        Some(crate::parser::SourceLine(
            Some("fragment-chain.adoc".to_owned()),
            1
        )) // [id=chain-{chapter}]
    );
    assert_eq!(
        source_map.original_file_and_line(7),
        Some(crate::parser::SourceLine(
            Some("fragment-chain.adoc".to_owned()),
            2
        )) // === Chain
    );
    assert_eq!(
        source_map.original_file_and_line(8),
        Some(crate::parser::SourceLine(
            Some("fragment-chain.adoc".to_owned()),
            3
        )) // blank line
    );
    assert_eq!(
        source_map.original_file_and_line(9),
        Some(crate::parser::SourceLine(
            Some("fragment-chain.adoc".to_owned()),
            4
        )) // See xref:chain-{chapter}[].
    );

    // Main document continues.
    assert_eq!(
        source_map.original_file_and_line(10),
        Some(crate::parser::SourceLine(None, 7)) // blank line after first include
    );
    assert_eq!(
        source_map.original_file_and_line(11),
        Some(crate::parser::SourceLine(None, 8)) // :chapter: maintenance
    );
    assert_eq!(
        source_map.original_file_and_line(12),
        Some(crate::parser::SourceLine(None, 9)) // == Maintenance
    );
    assert_eq!(
        source_map.original_file_and_line(13),
        Some(crate::parser::SourceLine(None, 10)) // blank line
    );

    // Second inclusion of fragment-chain.adoc.
    assert_eq!(
        source_map.original_file_and_line(14),
        Some(crate::parser::SourceLine(
            Some("fragment-chain.adoc".to_owned()),
            1
        )) // [id=chain-{chapter}]
    );
    assert_eq!(
        source_map.original_file_and_line(15),
        Some(crate::parser::SourceLine(
            Some("fragment-chain.adoc".to_owned()),
            2
        )) // === Chain
    );
    assert_eq!(
        source_map.original_file_and_line(16),
        Some(crate::parser::SourceLine(
            Some("fragment-chain.adoc".to_owned()),
            3
        )) // blank line
    );
    assert_eq!(
        source_map.original_file_and_line(17),
        Some(crate::parser::SourceLine(
            Some("fragment-chain.adoc".to_owned()),
            4
        )) // See xref:chain-{chapter}[].
    );
}
