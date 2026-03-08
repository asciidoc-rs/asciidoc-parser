use crate::{document::RefType, tests::prelude::*};

track_file!("docs/modules/document/pages/reference-revision-attributes.adoc");

non_normative!(
    r#"
= Reference the Revision Attributes

You can reference the revision information attributes in your document regardless of whether they're set via the revision line or attribute entries.

[#reference-revnumber]
== Reference revnumber

"#
);

#[test]
fn ex_line_references() {
    verifies!(
        r#"
Remember, when `revnumber` is assigned via the revision line, any characters preceding the version number are dropped.
For instance, the revision number in <<ex-line-references>> is prefixed with a _v_.

.Revision line and revision attribute references
[source#ex-line-references]
----
include::example$reference-revision-line.adoc[]
----

The result of <<ex-line-references>> below shows that the _v_ in the version number has been removed when it's rendered in the byline and referenced in the document.

image::reference-revision-line.png["Revision line and rendered revision references to revnumber, revdate and revremark",role=screenshot]

"#
    );

    let doc = Parser::default().parse("= The Intrepid Chronicles\nKismet Lee\nv8.3, July 29, 2025: Summertime!\n\n== Colophon\n\n[%hardbreaks]\nRevision number: {revnumber}\nRevision date: {revdate}\nRevision notes: {revremark}");

    assert_eq!(
        doc,
        Document {
            header: Header {
                title_source: Some(Span {
                    data: "The Intrepid Chronicles",
                    line: 1,
                    col: 3,
                    offset: 2,
                },),
                title: Some("The Intrepid Chronicles",),
                attributes: &[],
                author_line: Some(AuthorLine {
                    authors: &[Author {
                        name: "Kismet Lee",
                        firstname: "Kismet",
                        middlename: None,
                        lastname: Some("Lee",),
                        email: None,
                    },],
                    source: Span {
                        data: "Kismet Lee",
                        line: 2,
                        col: 1,
                        offset: 26,
                    },
                },),
                revision_line: Some(RevisionLine {
                    revnumber: Some("8.3",),
                    revdate: "July 29, 2025",
                    revremark: Some("Summertime!",),
                    source: Span {
                        data: "v8.3, July 29, 2025: Summertime!",
                        line: 3,
                        col: 1,
                        offset: 37,
                    },
                },),
                comments: &[],
                source: Span {
                    data: "= The Intrepid Chronicles\nKismet Lee\nv8.3, July 29, 2025: Summertime!",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[Block::Section(SectionBlock {
                level: 1,
                section_title: Content {
                    original: Span {
                        data: "Colophon",
                        line: 5,
                        col: 4,
                        offset: 74,
                    },
                    rendered: "Colophon",
                },
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "Revision number: {revnumber}\nRevision date: {revdate}\nRevision notes: {revremark}",
                            line: 8,
                            col: 1,
                            offset: 98,
                        },
                        rendered: "Revision number: 8.3<br>\nRevision date: July 29, 2025<br>\nRevision notes: Summertime!",
                    },
                    source: Span {
                        data: "[%hardbreaks]\nRevision number: {revnumber}\nRevision date: {revdate}\nRevision notes: {revremark}",
                        line: 7,
                        col: 1,
                        offset: 84,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: None,
                            value: "%hardbreaks",
                            shorthand_items: &["%hardbreaks"],
                        },],
                        anchor: None,
                        source: Span {
                            data: "%hardbreaks",
                            line: 7,
                            col: 2,
                            offset: 85,
                        },
                    },),
                },),],
                source: Span {
                    data: "== Colophon\n\n[%hardbreaks]\nRevision number: {revnumber}\nRevision date: {revdate}\nRevision notes: {revremark}",
                    line: 5,
                    col: 1,
                    offset: 71,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
                section_type: SectionType::Normal,
                section_id: Some("_colophon"),
                section_number: None,
            },),],
            source: Span {
                data: "= The Intrepid Chronicles\nKismet Lee\nv8.3, July 29, 2025: Summertime!\n\n== Colophon\n\n[%hardbreaks]\nRevision number: {revnumber}\nRevision date: {revdate}\nRevision notes: {revremark}",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog {
                refs: HashMap::from([(
                    "_colophon",
                    RefEntry {
                        id: "_colophon",
                        reftext: Some("Colophon",),
                        ref_type: RefType::Section,
                    }
                ),]),
                reftext_to_id: HashMap::from([("Colophon", "_colophon"),]),
            },
        }
    );
}

#[test]
fn ex_references() {
    verifies!(
        r#"
To display the entire value of `revnumber` when it's referenced in the document, you must set and assign it a value using an attribute entry.

.Revision attribute entries and references
[source#ex-references]
----
include::example$reference-revision-attributes.adoc[]
----

The entire value of the `revnumber` from <<ex-references>> is displayed in the byline, including the default `version-label` value _Version_.
When referenced in the document, the entire value of `revnumber` is displayed because it was set with an attribute entry.

image::reference-revision-attributes.png["Revision attribute entries and rendered revision references to revnumber, revdate and revremark",role=screenshot]

If you don't want the default version label to be displayed in the byline, xref:version-label.adoc#unset[unset the version-label attribute].
"#
    );

    let doc = Parser::default().parse("= The Intrepid Chronicles\nKismet Lee\n:revnumber: v8.3\n:revdate: July 29, 2025\n:revremark: Summertime!\n\n== Colophon\n\n[%hardbreaks]\nRevision number: {revnumber}\nRevision date: {revdate}\nRevision notes: {revremark}");

    assert_eq!(
        doc,
        Document {
            header: Header {
                title_source: Some(Span {
                    data: "The Intrepid Chronicles",
                    line: 1,
                    col: 3,
                    offset: 2,
                },),
                title: Some("The Intrepid Chronicles",),
                attributes: &[
                    Attribute {
                        name: Span {
                            data: "revnumber",
                            line: 3,
                            col: 2,
                            offset: 38,
                        },
                        value_source: Some(Span {
                            data: "v8.3",
                            line: 3,
                            col: 13,
                            offset: 49,
                        },),
                        value: InterpretedValue::Value("v8.3",),
                        source: Span {
                            data: ":revnumber: v8.3",
                            line: 3,
                            col: 1,
                            offset: 37,
                        },
                    },
                    Attribute {
                        name: Span {
                            data: "revdate",
                            line: 4,
                            col: 2,
                            offset: 55,
                        },
                        value_source: Some(Span {
                            data: "July 29, 2025",
                            line: 4,
                            col: 11,
                            offset: 64,
                        },),
                        value: InterpretedValue::Value("July 29, 2025",),
                        source: Span {
                            data: ":revdate: July 29, 2025",
                            line: 4,
                            col: 1,
                            offset: 54,
                        },
                    },
                    Attribute {
                        name: Span {
                            data: "revremark",
                            line: 5,
                            col: 2,
                            offset: 79,
                        },
                        value_source: Some(Span {
                            data: "Summertime!",
                            line: 5,
                            col: 13,
                            offset: 90,
                        },),
                        value: InterpretedValue::Value("Summertime!",),
                        source: Span {
                            data: ":revremark: Summertime!",
                            line: 5,
                            col: 1,
                            offset: 78,
                        },
                    },
                ],
                author_line: Some(AuthorLine {
                    authors: &[Author {
                        name: "Kismet Lee",
                        firstname: "Kismet",
                        middlename: None,
                        lastname: Some("Lee",),
                        email: None,
                    },],
                    source: Span {
                        data: "Kismet Lee",
                        line: 2,
                        col: 1,
                        offset: 26,
                    },
                },),
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= The Intrepid Chronicles\nKismet Lee\n:revnumber: v8.3\n:revdate: July 29, 2025\n:revremark: Summertime!",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[Block::Section(SectionBlock {
                level: 1,
                section_title: Content {
                    original: Span {
                        data: "Colophon",
                        line: 7,
                        col: 4,
                        offset: 106,
                    },
                    rendered: "Colophon",
                },
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "Revision number: {revnumber}\nRevision date: {revdate}\nRevision notes: {revremark}",
                            line: 10,
                            col: 1,
                            offset: 130,
                        },
                        rendered: "Revision number: v8.3<br>\nRevision date: July 29, 2025<br>\nRevision notes: Summertime!",
                    },
                    source: Span {
                        data: "[%hardbreaks]\nRevision number: {revnumber}\nRevision date: {revdate}\nRevision notes: {revremark}",
                        line: 9,
                        col: 1,
                        offset: 116,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: None,
                            value: "%hardbreaks",
                            shorthand_items: &["%hardbreaks"],
                        },],
                        anchor: None,
                        source: Span {
                            data: "%hardbreaks",
                            line: 9,
                            col: 2,
                            offset: 117,
                        },
                    },),
                },),],
                source: Span {
                    data: "== Colophon\n\n[%hardbreaks]\nRevision number: {revnumber}\nRevision date: {revdate}\nRevision notes: {revremark}",
                    line: 7,
                    col: 1,
                    offset: 103,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
                section_type: SectionType::Normal,
                section_id: Some("_colophon"),
                section_number: None,
            },),],
            source: Span {
                data: "= The Intrepid Chronicles\nKismet Lee\n:revnumber: v8.3\n:revdate: July 29, 2025\n:revremark: Summertime!\n\n== Colophon\n\n[%hardbreaks]\nRevision number: {revnumber}\nRevision date: {revdate}\nRevision notes: {revremark}",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog {
                refs: HashMap::from([(
                    "_colophon",
                    RefEntry {
                        id: "_colophon",
                        reftext: Some("Colophon",),
                        ref_type: RefType::Section,
                    }
                ),]),
                reftext_to_id: HashMap::from([("Colophon", "_colophon"),]),
            },
        }
    );
}
