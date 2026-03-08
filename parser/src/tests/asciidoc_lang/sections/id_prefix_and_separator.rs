use crate::tests::prelude::*;

track_file!("docs/modules/sections/pages/id-prefix-and-separator.adoc");

non_normative!(
    r#"
= Change the ID Prefix and Separator

When an AsciiDoc processor xref:auto-ids.adoc[auto-generates section IDs], it begins the value with an underscore and uses a hyphen between each word.
These characters can be customized with the `idprefix` and `idseparator` attributes.

"#
);

mod prefix {
    use crate::{Parser, document::RefType, tests::prelude::*};

    non_normative!(
        r#"
[#prefix]
== Change the ID prefix

By default, the AsciiDoc processor begins an auto-generated section ID with an underscore (`+_+`).
This default can cause problems when referencing the ID in an xref (either within the same file or a deep link to another file).
The leading underscore may get paired with an underscore somewhere else in the paragraph, thus resulting in unexpected text formatting.
One workaround is to disrupt the match by prefixing the ID with `\{empty}` (e.g., `\{empty}_section_title`) or using an attribute to refer to the target.
Instead, we strongly encourage you to customize the ID prefix.

"#
    );

    #[test]
    fn new_value() {
        verifies!(
            r#"
You can change this prefix by setting the `idprefix` attribute and assigning it a new value.
The value of `idprefix` must begin with a valid ID start character and can have any number of additional valid ID characters.

[source]
----
:idprefix: id_
----

"#
        );

        let doc = Parser::default().parse(":idprefix: id_\n\n== Section Title");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[Attribute {
                        name: Span {
                            data: "idprefix",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                        value_source: Some(Span {
                            data: "id_",
                            line: 1,
                            col: 12,
                            offset: 11,
                        },),
                        value: InterpretedValue::Value("id_",),
                        source: Span {
                            data: ":idprefix: id_",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: ":idprefix: id_",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::Section(SectionBlock {
                    level: 1,
                    section_title: Content {
                        original: Span {
                            data: "Section Title",
                            line: 3,
                            col: 4,
                            offset: 19,
                        },
                        rendered: "Section Title",
                    },
                    blocks: &[],
                    source: Span {
                        data: "== Section Title",
                        line: 3,
                        col: 1,
                        offset: 16,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    section_type: SectionType::Normal,
                    section_id: Some("id_section_title",),
                    section_number: None,
                },),],
                source: Span {
                    data: ":idprefix: id_\n\n== Section Title",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([(
                        "id_section_title",
                        RefEntry {
                            id: "id_section_title",
                            reftext: Some("Section Title",),
                            ref_type: RefType::Section,
                        }
                    ),]),
                    reftext_to_id: HashMap::from([("Section Title", "id_section_title"),]),
                },
            }
        );
    }

    #[test]
    fn remove_prefix() {
        verifies!(
            r#"
If you want to remove the prefix, set the attribute to an empty value.

[source]
----
:idprefix:
----

"#
        );

        let doc = Parser::default().parse(":idprefix:\n\n== Section Title");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[Attribute {
                        name: Span {
                            data: "idprefix",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                        value_source: None,
                        value: InterpretedValue::Set,
                        source: Span {
                            data: ":idprefix:",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: ":idprefix:",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::Section(SectionBlock {
                    level: 1,
                    section_title: Content {
                        original: Span {
                            data: "Section Title",
                            line: 3,
                            col: 4,
                            offset: 15,
                        },
                        rendered: "Section Title",
                    },
                    blocks: &[],
                    source: Span {
                        data: "== Section Title",
                        line: 3,
                        col: 1,
                        offset: 12,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    section_type: SectionType::Normal,
                    section_id: Some("section_title",),
                    section_number: None,
                },),],
                source: Span {
                    data: ":idprefix:\n\n== Section Title",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([(
                        "section_title",
                        RefEntry {
                            id: "section_title",
                            reftext: Some("Section Title",),
                            ref_type: RefType::Section,
                        }
                    ),]),
                    reftext_to_id: HashMap::from([("Section Title", "section_title"),]),
                },
            }
        );
    }

    non_normative!(
        r#"
WARNING: If you set the `idprefix` to empty, you could end up generating IDs that are invalid in DocBook output (e.g., an ID that begins with a number) or that match a built-in ID in the HTML output (e.g., `header`).
In this case, we recommend either using a non-empty value of `idprefix` or assigning xref:custom-ids.adoc[explicit IDs to your sections].

"#
    );
}

mod separator {
    use crate::{Parser, document::RefType, tests::prelude::*};

    non_normative!(
        r#"
[#separator]
== Change the ID word separator

The default section ID word separator is an underscore (`+_+`).
You can change the separator with the `idseparator` attribute.
Unless empty, the value of the `idseparator` must be _exactly one valid ID character_.

"#
    );

    #[test]
    fn new_value() {
        verifies!(
            r#"
[source]
----
:idseparator: -
----

"#
        );

        let doc = Parser::default().parse(":idseparator: .\n\n== Section Title");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[Attribute {
                        name: Span {
                            data: "idseparator",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                        value_source: Some(Span {
                            data: ".",
                            line: 1,
                            col: 15,
                            offset: 14,
                        },),
                        value: InterpretedValue::Value(".",),
                        source: Span {
                            data: ":idseparator: .",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: ":idseparator: .",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::Section(SectionBlock {
                    level: 1,
                    section_title: Content {
                        original: Span {
                            data: "Section Title",
                            line: 3,
                            col: 4,
                            offset: 20,
                        },
                        rendered: "Section Title",
                    },
                    blocks: &[],
                    source: Span {
                        data: "== Section Title",
                        line: 3,
                        col: 1,
                        offset: 17,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    section_type: SectionType::Normal,
                    section_id: Some("_section.title",),
                    section_number: None,
                },),],
                source: Span {
                    data: ":idseparator: .\n\n== Section Title",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([(
                        "_section.title",
                        RefEntry {
                            id: "_section.title",
                            reftext: Some("Section Title",),
                            ref_type: RefType::Section,
                        }
                    ),]),
                    reftext_to_id: HashMap::from([("Section Title", "_section.title"),]),
                },
            }
        );
    }

    #[test]
    fn remove_value() {
        verifies!(
            r#"
If you don't want to use a separator, set the attribute to an empty value.

[source]
----
:idseparator:
----

"#
        );

        let doc = Parser::default().parse(":idseparator:\n\n== Section Title");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[Attribute {
                        name: Span {
                            data: "idseparator",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                        value_source: None,
                        value: InterpretedValue::Set,
                        source: Span {
                            data: ":idseparator:",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                    },],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: ":idseparator:",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::Section(SectionBlock {
                    level: 1,
                    section_title: Content {
                        original: Span {
                            data: "Section Title",
                            line: 3,
                            col: 4,
                            offset: 18,
                        },
                        rendered: "Section Title",
                    },
                    blocks: &[],
                    source: Span {
                        data: "== Section Title",
                        line: 3,
                        col: 1,
                        offset: 15,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    section_type: SectionType::Normal,
                    section_id: Some("_sectiontitle",),
                    section_number: None,
                },),],
                source: Span {
                    data: ":idseparator:\n\n== Section Title",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([(
                        "_sectiontitle",
                        RefEntry {
                            id: "_sectiontitle",
                            reftext: Some("Section Title",),
                            ref_type: RefType::Section,
                        }
                    ),]),
                    reftext_to_id: HashMap::from([("Section Title", "_sectiontitle"),]),
                },
            }
        );
    }

    non_normative!(
        r#"
NOTE: When a document is rendered on GitHub, the `idprefix` is set to an empty value and the `idseparator` is set to `-`.
These settings are used to ensure that the IDs generated by GitHub match the IDs generated by Asciidoctor.
"#
    );
}
