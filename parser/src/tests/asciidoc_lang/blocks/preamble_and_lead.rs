use crate::{blocks::SimpleBlockStyle, document::RefType, tests::prelude::*};

track_file!("docs/modules/blocks/pages/preamble-and-lead.adoc");

non_normative!(
    r#"
= Preamble and Lead Style

"#
);

#[test]
fn preamble_style() {
    verifies!(
        r#"
[#preamble-style]
== Preamble

Content between the end of the xref:document:header.adoc[document header] and the first section title in the document body is called the preamble.
A preamble is optional.

.Preamble
[#ex-preamble]
----
include::example$preamble.adoc[]
----

"#
    );

    let doc = Parser::default().parse(
        "= The Intrepid Chronicles\n\nThis adventure begins on a frigid morning.\nWe've run out of coffee beans, but leaving our office means venturing into certain peril.\nYesterday, a colony of ravenous Wolpertingers descended from the foothills.\nNo one can find the defensive operations manual, and our security experts are on an off-the-grid team-building retreat in Katchanga.\n\nWhat are we going to do?\n\n== Certain Peril\n\nDaylight trickles across the cobblestones..."
    );

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
                author_line: None,
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= The Intrepid Chronicles",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[
                Block::Preamble(Preamble {
                    blocks: &[
                        Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "This adventure begins on a frigid morning.\nWe've run out of coffee beans, but leaving our office means venturing into certain peril.\nYesterday, a colony of ravenous Wolpertingers descended from the foothills.\nNo one can find the defensive operations manual, and our security experts are on an off-the-grid team-building retreat in Katchanga.",
                                    line: 3,
                                    col: 1,
                                    offset: 27,
                                },
                                rendered: "This adventure begins on a frigid morning.\nWe&#8217;ve run out of coffee beans, but leaving our office means venturing into certain peril.\nYesterday, a colony of ravenous Wolpertingers descended from the foothills.\nNo one can find the defensive operations manual, and our security experts are on an off-the-grid team-building retreat in Katchanga.",
                            },
                            source: Span {
                                data: "This adventure begins on a frigid morning.\nWe've run out of coffee beans, but leaving our office means venturing into certain peril.\nYesterday, a colony of ravenous Wolpertingers descended from the foothills.\nNo one can find the defensive operations manual, and our security experts are on an off-the-grid team-building retreat in Katchanga.",
                                line: 3,
                                col: 1,
                                offset: 27,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "What are we going to do?",
                                    line: 8,
                                    col: 1,
                                    offset: 370,
                                },
                                rendered: "What are we going to do?",
                            },
                            source: Span {
                                data: "What are we going to do?",
                                line: 8,
                                col: 1,
                                offset: 370,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                    ],
                    source: Span {
                        data: "This adventure begins on a frigid morning.\nWe've run out of coffee beans, but leaving our office means venturing into certain peril.\nYesterday, a colony of ravenous Wolpertingers descended from the foothills.\nNo one can find the defensive operations manual, and our security experts are on an off-the-grid team-building retreat in Katchanga.\n\nWhat are we going to do?",
                        line: 3,
                        col: 1,
                        offset: 27,
                    },
                },),
                Block::Section(SectionBlock {
                    level: 1,
                    section_title: Content {
                        original: Span {
                            data: "Certain Peril",
                            line: 10,
                            col: 4,
                            offset: 399,
                        },
                        rendered: "Certain Peril",
                    },
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "Daylight trickles across the cobblestones...",
                                line: 12,
                                col: 1,
                                offset: 414,
                            },
                            rendered: "Daylight trickles across the cobblestones&#8230;&#8203;",
                        },
                        source: Span {
                            data: "Daylight trickles across the cobblestones...",
                            line: 12,
                            col: 1,
                            offset: 414,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "== Certain Peril\n\nDaylight trickles across the cobblestones...",
                        line: 10,
                        col: 1,
                        offset: 396,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    section_type: SectionType::Normal,
                    section_id: Some("_certain_peril",),
                    section_number: None,
                },),
            ],
            source: Span {
                data: "= The Intrepid Chronicles\n\nThis adventure begins on a frigid morning.\nWe've run out of coffee beans, but leaving our office means venturing into certain peril.\nYesterday, a colony of ravenous Wolpertingers descended from the foothills.\nNo one can find the defensive operations manual, and our security experts are on an off-the-grid team-building retreat in Katchanga.\n\nWhat are we going to do?\n\n== Certain Peril\n\nDaylight trickles across the cobblestones...",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog {
                refs: HashMap::from([(
                    "_certain_peril",
                    RefEntry {
                        id: "_certain_peril",
                        reftext: Some("Certain Peril",),
                        ref_type: RefType::Section,
                    },
                ),]),
                reftext_to_id: HashMap::from([("Certain Peril", "_certain_peril",),]),
            },
        }
    );
}

// Treating the rest of this page as non-normative. Asciidoctor's implementation
// of lead role is handled entirely by CSS treatment; asciidoc-parser does not
// generate renderings, so this part of the language description does not apply.
non_normative!(
    r#"
When using the default Asciidoctor stylesheet, if the first paragraph does not have an explicit role, it is styled as if it has the <<lead-role,lead role>>.
The result of <<ex-preamble>> is displayed below.

image::preamble.png[The preamble of a document,role=screenshot]

[#lead-role]
== Lead role

Apply the `lead` role to any paragraph, and it will render using a larger font size.
The lead role is assigned to the xref:attributes:role.adoc[role attribute].
You can set `role` using the classic or shorthand method.

.Setting role to lead using the shorthand syntax
[#ex-lead]
----
include::example$paragraph.adoc[tag=lead]
----

The result of <<ex-lead>> is displayed below.

image::lead.png[Paragraph styled with the lead role value,role=screenshot]

When you convert a document to HTML using the default stylesheet, the first paragraph of the <<preamble-style,preamble>> is automatically styled as a lead paragraph.
To disable this behavior, assign any role to the first paragraph.

.Disabling the automatic lead paragraph styling
----
include::example$paragraph.adoc[tag=no-lead]
----

The presence of the custom role (`normal`) informs the CSS not to style it as a lead paragraph.
"#
);
