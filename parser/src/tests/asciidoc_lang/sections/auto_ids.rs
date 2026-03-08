use crate::tests::prelude::*;

track_file!("docs/modules/sections/pages/auto-ids.adoc");

non_normative!(
    r#"
= Autogenerate Section IDs
:page-aliases: ids.adoc
:url-ntname: https://www.w3.org/TR/REC-xml/#NT-Name

Sections and discrete headings support automatic ID generation.
Unless you've assigned a custom ID to one of these blocks, or you've unset the `sectids` document attribute, the AsciiDoc processor will automatically generate and assign an ID for the block using the title.
This page explains how the ID is derived and how to control this behavior.

"#
);

mod how_a_section_id_is_computed {
    use std::collections::HashMap;

    use crate::{
        Parser,
        blocks::{SimpleBlockStyle, metadata::BlockMetadata},
        document::RefType,
        parser::ModificationContext,
        tests::prelude::*,
    };

    non_normative!(
        r#"
== How a section ID is computed

The AsciiDoc processor builds an ID from the title using the following order of events and rules:

"#
    );

    #[test]
    fn inline_formatting_applied() {
        verifies!(
            r#"
* Inline formatting is applied (in title substitution order).
"#
        );

        let mut parser = Parser::default();
        let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

        let mi = crate::blocks::SectionBlock::parse(
            &BlockMetadata::new("== Section *Title*"),
            &mut parser,
            &mut warnings,
        )
        .unwrap()
        .item;

        assert_eq!(mi.section_id().unwrap(), "_section_title");
    }

    #[test]
    fn all_characters_converted_to_lowercase() {
        verifies!(
            r#"
* All characters are converted to lowercase.
"#
        );

        let mut parser = Parser::default();
        let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

        let mi = crate::blocks::SectionBlock::parse(
            &BlockMetadata::new("== TÍTULO de la sección"),
            &mut parser,
            &mut warnings,
        )
        .unwrap()
        .item;

        assert_eq!(mi.section_id().unwrap(), "_título_de_la_sección");
    }

    #[test]
    fn id_prefix_prepended() {
        verifies!(
            r#"
* The value of the xref:id-prefix-and-separator.adoc#prefix[idprefix attribute] (`+_+` by default) is prepended.
"#
        );

        // Default case.
        let mut parser = Parser::default();
        let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

        let mi = crate::blocks::SectionBlock::parse(
            &BlockMetadata::new("== Section Title"),
            &mut parser,
            &mut warnings,
        )
        .unwrap()
        .item;

        assert_eq!(mi.section_id().unwrap(), "_section_title");

        // Change the prefix.
        let mut parser = Parser::default().with_intrinsic_attribute(
            "idprefix",
            "~",
            ModificationContext::Anywhere,
        );
        let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

        let mi = crate::blocks::SectionBlock::parse(
            &BlockMetadata::new("== Section Title"),
            &mut parser,
            &mut warnings,
        )
        .unwrap()
        .item;

        assert_eq!(mi.section_id().unwrap(), "~section_title");
    }

    #[test]
    fn char_references_etc_removed() {
        verifies!(
            r#"
* Character references, HTML/XML tags (not their contents), and non-word characters (except for space, hyphen, and period) are removed.
"#
        );

        // Character references.
        let mut parser = Parser::default();
        let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

        let mi = crate::blocks::SectionBlock::parse(
            &BlockMetadata::new("== T&iacute;tulo de la secci&oacute;n"),
            &mut parser,
            &mut warnings,
        )
        .unwrap()
        .item;

        assert_eq!(mi.section_id().unwrap(), "_ttulo_de_la_seccin");

        // HTML tags.
        let mut parser = Parser::default();
        let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

        let mi = crate::blocks::SectionBlock::parse(
            &BlockMetadata::new("== Section <i>title</i>"),
            &mut parser,
            &mut warnings,
        )
        .unwrap()
        .item;

        assert_eq!(mi.section_id().unwrap(), "_section_title");

        // Non-word characters.
        let mut parser = Parser::default();
        let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

        let mi = crate::blocks::SectionBlock::parse(
            &BlockMetadata::new("== Section @ title @#!*$("),
            &mut parser,
            &mut warnings,
        )
        .unwrap()
        .item;

        assert_eq!(mi.section_id().unwrap(), "_section_title");
    }

    #[test]
    fn id_separator_applied() {
        verifies!(
            r#"
* Spaces, hyphens, and periods are replaced with the value of the xref:id-prefix-and-separator.adoc#separator[idseparator attribute] (`+_+` by default)
"#
        );

        // Default case.
        let mut parser = Parser::default();
        let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

        let mi = crate::blocks::SectionBlock::parse(
            &BlockMetadata::new("== Extra-long section.title"),
            &mut parser,
            &mut warnings,
        )
        .unwrap()
        .item;

        assert_eq!(mi.section_id().unwrap(), "_extra_long_section_title");

        // Change the separator.
        let mut parser = Parser::default().with_intrinsic_attribute(
            "idseparator",
            ".",
            ModificationContext::Anywhere,
        );
        let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

        let mi = crate::blocks::SectionBlock::parse(
            &BlockMetadata::new("== Extra-long section.title"),
            &mut parser,
            &mut warnings,
        )
        .unwrap()
        .item;

        assert_eq!(mi.section_id().unwrap(), "_extra.long.section.title");
    }

    #[test]
    fn repeating_separators_condensed() {
        verifies!(
            r#"
* Repeating separator characters are condensed.
"#
        );

        // Default case.
        let mut parser = Parser::default();
        let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

        let mi = crate::blocks::SectionBlock::parse(
            &BlockMetadata::new("== Section.- title"),
            &mut parser,
            &mut warnings,
        )
        .unwrap()
        .item;

        assert_eq!(mi.section_id().unwrap(), "_section_title");

        // Change the separator.
        let mut parser = Parser::default().with_intrinsic_attribute(
            "idseparator",
            ".",
            ModificationContext::Anywhere,
        );
        let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

        let mi = crate::blocks::SectionBlock::parse(
            &BlockMetadata::new("== Section.- title"),
            &mut parser,
            &mut warnings,
        )
        .unwrap()
        .item;

        assert_eq!(mi.section_id().unwrap(), "_section.title");
    }

    #[test]
    fn sequence_number_appended() {
        verifies!(
            r#"
* If necessary, a sequence number is appended until the ID is unique within the document.

"#
        );

        let doc = Parser::default().parse("== Duplicate Title\n\nOne\n\n== Duplicate Title\n\nTwo");

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
                    Block::Section(SectionBlock {
                        level: 1,
                        section_title: Content {
                            original: Span {
                                data: "Duplicate Title",
                                line: 1,
                                col: 4,
                                offset: 3,
                            },
                            rendered: "Duplicate Title",
                        },
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "One",
                                    line: 3,
                                    col: 1,
                                    offset: 20,
                                },
                                rendered: "One",
                            },
                            source: Span {
                                data: "One",
                                line: 3,
                                col: 1,
                                offset: 20,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "== Duplicate Title\n\nOne",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                        section_type: SectionType::Normal,
                        section_id: Some("_duplicate_title",),
                        section_number: None,
                    },),
                    Block::Section(SectionBlock {
                        level: 1,
                        section_title: Content {
                            original: Span {
                                data: "Duplicate Title",
                                line: 5,
                                col: 4,
                                offset: 28,
                            },
                            rendered: "Duplicate Title",
                        },
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "Two",
                                    line: 7,
                                    col: 1,
                                    offset: 45,
                                },
                                rendered: "Two",
                            },
                            source: Span {
                                data: "Two",
                                line: 7,
                                col: 1,
                                offset: 45,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "== Duplicate Title\n\nTwo",
                            line: 5,
                            col: 1,
                            offset: 25,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                        section_type: SectionType::Normal,
                        section_id: Some("_duplicate_title-2",),
                        section_number: None,
                    },),
                ],
                source: Span {
                    data: "== Duplicate Title\n\nOne\n\n== Duplicate Title\n\nTwo",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([
                        (
                            "_duplicate_title-2",
                            RefEntry {
                                id: "_duplicate_title-2",
                                reftext: Some("Duplicate Title",),
                                ref_type: RefType::Section,
                            }
                        ),
                        (
                            "_duplicate_title",
                            RefEntry {
                                id: "_duplicate_title",
                                reftext: Some("Duplicate Title",),
                                ref_type: RefType::Section,
                            }
                        ),
                    ]),
                    reftext_to_id: HashMap::from([("Duplicate Title", "_duplicate_title"),]),
                },
            }
        );
    }

    non_normative!(
        r#"
The generated ID can be expected to be safe to use in an HTML document.
However, it's important to understand that the generated ID does not necessarily conform to an {url-ntname}[NT-Name^], as required by the XML specification.
If you intend to produce DocBook from your AsciiDoc document(s), and your section titles uses word characters that are not permitted in an XML ID, the onus is on you to either provide an explicit ID that is conforming, or encode those invalid ID characters using a character reference (e.g., `\&#x2161;`).

"#
    );

    #[test]
    fn ex_wiley_sons_inc() {
        verifies!(
            r#"
With those rules in mind, given the following section title:

----
== Wiley & Sons, Inc.
----

the processor will produce the following ID:

....
_wiley_sons_inc
....

"#
        );

        // Default case.
        let mut parser = Parser::default();
        let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

        let mi = crate::blocks::SectionBlock::parse(
            &BlockMetadata::new("== Wiley & Sons, Inc."),
            &mut parser,
            &mut warnings,
        )
        .unwrap()
        .item;

        assert_eq!(mi.section_id().unwrap(), "_wiley_sons_inc");
    }

    non_normative!(
        r#"
You can toggle ID autogeneration on and off using `sectids` and xref:id-prefix-and-separator.adoc[customize the ID prefix and word separator].

CAUTION: If the section title contains a forward looking xref (i.e., an xref to an element that comes later in document order), you must either assign a custom ID to the block or <<disable,disable ID generation>> around the title.
Otherwise, the AsciiDoc processor may warn that the reference is invalid.
This happens because, in order to generate an ID, the processor must convert the title.
This conversion happens before the processor has visited the target element.
As a result, the processor is not able to lookup the reference and therefore must consider it invalid.

"#
    );
}
mod disable_automatic_section_id_generation {
    use std::collections::HashMap;

    use crate::{Parser, document::RefType, tests::prelude::*};

    non_normative!(
        r#"
[#disable]
== Disable automatic section ID generation

To disable autogeneration of section and discrete heading IDs, unset the `sectids` attribute.

----
:!sectids:
----

xref:custom-ids.adoc[Custom IDs] are still used even when automatic section IDs are disabled.

"#
    );

    #[test]
    fn disable_via_sectids() {
        verifies!(
            r#"
You can unset this attribute anywhere that attribute entries are permitted in the document.
By doing so, you can disable ID generation for only certain sections and discrete headings.

----
== ID generation on

:!sectids:
== ID generation off
:sectids:

== ID generation on again
----

"#
        );

        let doc = Parser::default().parse("== ID generation on\n\n:!sectids:\n== ID generation off\n:sectids:\n\n== ID generation on again");

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
                    Block::Section(SectionBlock {
                        level: 1,
                        section_title: Content {
                            original: Span {
                                data: "ID generation on",
                                line: 1,
                                col: 4,
                                offset: 3,
                            },
                            rendered: "ID generation on",
                        },
                        blocks: &[Block::DocumentAttribute(Attribute {
                            name: Span {
                                data: "sectids",
                                line: 3,
                                col: 3,
                                offset: 23,
                            },
                            value_source: None,
                            value: InterpretedValue::Unset,
                            source: Span {
                                data: ":!sectids:",
                                line: 3,
                                col: 1,
                                offset: 21,
                            },
                        },),],
                        source: Span {
                            data: "== ID generation on\n\n:!sectids:",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                        section_type: SectionType::Normal,
                        section_id: Some("_id_generation_on"),
                        section_number: None,
                    },),
                    Block::Section(SectionBlock {
                        level: 1,
                        section_title: Content {
                            original: Span {
                                data: "ID generation off",
                                line: 4,
                                col: 4,
                                offset: 35,
                            },
                            rendered: "ID generation off",
                        },
                        blocks: &[Block::DocumentAttribute(Attribute {
                            name: Span {
                                data: "sectids",
                                line: 5,
                                col: 2,
                                offset: 54,
                            },
                            value_source: None,
                            value: InterpretedValue::Set,
                            source: Span {
                                data: ":sectids:",
                                line: 5,
                                col: 1,
                                offset: 53,
                            },
                        },),],
                        source: Span {
                            data: "== ID generation off\n:sectids:",
                            line: 4,
                            col: 1,
                            offset: 32,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                        section_type: SectionType::Normal,
                        section_id: None,
                        section_number: None,
                    },),
                    Block::Section(SectionBlock {
                        level: 1,
                        section_title: Content {
                            original: Span {
                                data: "ID generation on again",
                                line: 7,
                                col: 4,
                                offset: 67,
                            },
                            rendered: "ID generation on again",
                        },
                        blocks: &[],
                        source: Span {
                            data: "== ID generation on again",
                            line: 7,
                            col: 1,
                            offset: 64,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                        section_type: SectionType::Normal,
                        section_id: Some("_id_generation_on_again",),
                        section_number: None,
                    },),
                ],
                source: Span {
                    data: "== ID generation on\n\n:!sectids:\n== ID generation off\n:sectids:\n\n== ID generation on again",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([
                        (
                            "_id_generation_on_again",
                            RefEntry {
                                id: "_id_generation_on_again",
                                reftext: Some("ID generation on again",),
                                ref_type: RefType::Section,
                            }
                        ),
                        (
                            "_id_generation_on",
                            RefEntry {
                                id: "_id_generation_on",
                                reftext: Some("ID generation on",),
                                ref_type: RefType::Section,
                            }
                        ),
                    ]),
                    reftext_to_id: HashMap::from([
                        ("ID generation on again", "_id_generation_on_again"),
                        ("ID generation on", "_id_generation_on"),
                    ]),
                },
            }
        );
    }

    non_normative!(
        r#"
If you disable autogenerated section IDs, and you don't assign a custom ID to a section or discrete headings, you won't be able to create cross references to that element.
"#
    );
}
