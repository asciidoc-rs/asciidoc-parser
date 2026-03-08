use std::collections::HashMap;

use crate::{Parser, blocks::SimpleBlockStyle, document::RefType, tests::prelude::*};

track_file!("docs/modules/document/pages/reference-author-attributes.adoc");

non_normative!(
    r#"
= Reference the Author Information

"#
);

#[test]
fn referencing_single_author_attributes() {
    non_normative!(
        r#"
[#reference-author]
== Referencing the author attributes

"#
    );

    verifies!(
        r#"
You can reference the built-in author attributes in your document regardless of whether they're set via the author line or attribute entries.
In <<ex-reference>>, the `author` and `email` attributes are assigned using attribute entries.

.Reference the author attributes
[source#ex-reference]
----
include::example$reference-author.adoc[]
----

The result of <<ex-reference>> is displayed below.

image::reference-author.png[Reference the built-in attributes for an author,role=screenshot]

"#
    );

    let mut parser = Parser::default();
    let doc = parser.parse("= The Intrepid Chronicles\n:author: Kismet R. Lee\n:email: kismet@asciidoctor.org\n\n== About {author}\n\nYou can contact {firstname} at {email}.\n\nP.S. Don't ask what the {middlename} stands for; it's a secret.\n{authorinitials}");

    assert_eq!(
        parser.attribute_value("author"),
        InterpretedValue::Value("Kismet R. Lee")
    );

    assert_eq!(
        parser.attribute_value("firstname"),
        InterpretedValue::Value("Kismet")
    );

    assert_eq!(
        parser.attribute_value("middlename"),
        InterpretedValue::Value("R.")
    );

    assert_eq!(
        parser.attribute_value("lastname"),
        InterpretedValue::Value("Lee")
    );

    assert_eq!(
        parser.attribute_value("authorinitials"),
        InterpretedValue::Value("KRL")
    );

    assert_eq!(
        parser.attribute_value("email"),
        InterpretedValue::Value("kismet@asciidoctor.org")
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
                attributes: &[
                    Attribute {
                        name: Span {
                            data: "author",
                            line: 2,
                            col: 2,
                            offset: 27,
                        },
                        value_source: Some(Span {
                            data: "Kismet R. Lee",
                            line: 2,
                            col: 10,
                            offset: 35,
                        },),
                        value: InterpretedValue::Value("Kismet R. Lee",),
                        source: Span {
                            data: ":author: Kismet R. Lee",
                            line: 2,
                            col: 1,
                            offset: 26,
                        },
                    },
                    Attribute {
                        name: Span {
                            data: "email",
                            line: 3,
                            col: 2,
                            offset: 50,
                        },
                        value_source: Some(Span {
                            data: "kismet@asciidoctor.org",
                            line: 3,
                            col: 9,
                            offset: 57,
                        },),
                        value: InterpretedValue::Value("kismet@asciidoctor.org",),
                        source: Span {
                            data: ":email: kismet@asciidoctor.org",
                            line: 3,
                            col: 1,
                            offset: 49,
                        },
                    },
                ],
                author_line: None,
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= The Intrepid Chronicles\n:author: Kismet R. Lee\n:email: kismet@asciidoctor.org",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[Block::Section(SectionBlock {
                level: 1,
                section_title: Content {
                    original: Span {
                        data: "About {author}",
                        line: 5,
                        col: 4,
                        offset: 84,
                    },
                    rendered: "About Kismet R. Lee",
                },
                blocks: &[
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "You can contact {firstname} at {email}.",
                                line: 7,
                                col: 1,
                                offset: 100,
                            },
                            rendered: "You can contact Kismet at <a href=\"mailto:kismet@asciidoctor.org\">kismet@asciidoctor.org</a>.",
                        },
                        source: Span {
                            data: "You can contact {firstname} at {email}.",
                            line: 7,
                            col: 1,
                            offset: 100,
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
                                data: "P.S. Don't ask what the {middlename} stands for; it's a secret.\n{authorinitials}",
                                line: 9,
                                col: 1,
                                offset: 141,
                            },
                            rendered: "P.S. Don&#8217;t ask what the R. stands for; it&#8217;s a secret.\nKRL",
                        },
                        source: Span {
                            data: "P.S. Don't ask what the {middlename} stands for; it's a secret.\n{authorinitials}",
                            line: 9,
                            col: 1,
                            offset: 141,
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
                    data: "== About {author}\n\nYou can contact {firstname} at {email}.\n\nP.S. Don't ask what the {middlename} stands for; it's a secret.\n{authorinitials}",
                    line: 5,
                    col: 1,
                    offset: 81,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
                section_type: SectionType::Normal,
                section_id: Some("_about_kismet_r_lee"),
                section_number: None,
            },),],
            source: Span {
                data: "= The Intrepid Chronicles\n:author: Kismet R. Lee\n:email: kismet@asciidoctor.org\n\n== About {author}\n\nYou can contact {firstname} at {email}.\n\nP.S. Don't ask what the {middlename} stands for; it's a secret.\n{authorinitials}",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog {
                refs: HashMap::from([(
                    "_about_kismet_r_lee",
                    RefEntry {
                        id: "_about_kismet_r_lee",
                        reftext: Some("About Kismet R. Lee",),
                        ref_type: RefType::Section,
                    }
                ),]),
                reftext_to_id: HashMap::from([("About Kismet R. Lee", "_about_kismet_r_lee"),]),
            },
        }
    );
}

#[test]
fn referencing_multiple_author_attributes() {
    non_normative!(
        r#"
[#reference-multiple-authors]
== Referencing information for multiple authors

"#
    );

    verifies!(
        r#"
The first author in an author line is assigned to the built-in attributes `author`, `email`, `firstname`, etc.
Subsequent authors are assigned to the built-in author attributes, but the attribute names are appended with an underscore (`+_+`) and the numeric position of the author in the author line.
For instance, the author _B. Steppenwolf_ in <<ex-reference-multiple>> is the second author in the author line.
The built-in attributes used to reference their information are appended with the number _2_, e.g., `author_2`, `email_2`, `lastname_2`, etc.

.Reference the built-in attributes for multiple authors
[source#ex-reference-multiple]
----
include::example$multiple-authors.adoc[tag=doc]
----

The result of <<ex-reference-multiple>> is displayed below.

image::reference-multiple-authors.png[Reference the built-in attributes for multiple author,role=screenshot]

"#
    );

    let mut parser = Parser::default();
    let doc = parser.parse("= The Intrepid Chronicles\nKismet R. Lee <kismet@asciidoctor.org>; B. Steppenwolf; Pax Draeke <pax@asciidoctor.org>\n\n.About {author_2}\nMr. {lastname_2} lives in the Rocky Mountains.\n\n.About {author_3}\n{firstname_3}, also known as {authorinitials_3}, loves to surf.\n\n.About {author}\nYou can contact {firstname} at {email}.");

    assert_eq!(
        parser.attribute_value("author"),
        InterpretedValue::Value("Kismet R. Lee")
    );

    assert_eq!(
        parser.attribute_value("firstname"),
        InterpretedValue::Value("Kismet")
    );

    assert_eq!(
        parser.attribute_value("middlename"),
        InterpretedValue::Value("R.")
    );

    assert_eq!(
        parser.attribute_value("lastname"),
        InterpretedValue::Value("Lee")
    );

    assert_eq!(
        parser.attribute_value("authorinitials"),
        InterpretedValue::Value("KRL")
    );

    assert_eq!(
        parser.attribute_value("email"),
        InterpretedValue::Value("kismet@asciidoctor.org")
    );

    assert_eq!(
        parser.attribute_value("author_2"),
        InterpretedValue::Value("B. Steppenwolf")
    );

    assert_eq!(
        parser.attribute_value("firstname_2"),
        InterpretedValue::Value("B.")
    );

    assert_eq!(
        parser.attribute_value("middlename_2"),
        InterpretedValue::Unset
    );

    assert_eq!(
        parser.attribute_value("lastname_2"),
        InterpretedValue::Value("Steppenwolf")
    );

    assert_eq!(
        parser.attribute_value("authorinitials_2"),
        InterpretedValue::Value("BS")
    );

    assert_eq!(parser.attribute_value("email_2"), InterpretedValue::Unset);

    assert_eq!(
        parser.attribute_value("author_3"),
        InterpretedValue::Value("Pax Draeke")
    );

    assert_eq!(
        parser.attribute_value("firstname_3"),
        InterpretedValue::Value("Pax")
    );

    assert_eq!(
        parser.attribute_value("middlename_3"),
        InterpretedValue::Unset
    );

    assert_eq!(
        parser.attribute_value("lastname_3"),
        InterpretedValue::Value("Draeke")
    );

    assert_eq!(
        parser.attribute_value("authorinitials_3"),
        InterpretedValue::Value("PD")
    );

    assert_eq!(
        parser.attribute_value("email_3"),
        InterpretedValue::Value("pax@asciidoctor.org")
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
                author_line: Some(AuthorLine {
                    authors: &[
                        Author {
                            name: "Kismet R. Lee",
                            firstname: "Kismet",
                            middlename: Some("R.",),
                            lastname: Some("Lee",),
                            email: Some("kismet@asciidoctor.org",),
                        },
                        Author {
                            name: "B. Steppenwolf",
                            firstname: "B.",
                            middlename: None,
                            lastname: Some("Steppenwolf",),
                            email: None,
                        },
                        Author {
                            name: "Pax Draeke",
                            firstname: "Pax",
                            middlename: None,
                            lastname: Some("Draeke",),
                            email: Some("pax@asciidoctor.org",),
                        },
                    ],
                    source: Span {
                        data: "Kismet R. Lee <kismet@asciidoctor.org>; B. Steppenwolf; Pax Draeke <pax@asciidoctor.org>",
                        line: 2,
                        col: 1,
                        offset: 26,
                    },
                },),
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= The Intrepid Chronicles\nKismet R. Lee <kismet@asciidoctor.org>; B. Steppenwolf; Pax Draeke <pax@asciidoctor.org>",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[
                Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "Mr. {lastname_2} lives in the Rocky Mountains.",
                            line: 5,
                            col: 1,
                            offset: 134,
                        },
                        rendered: "Mr. Steppenwolf lives in the Rocky Mountains.",
                    },
                    source: Span {
                        data: ".About {author_2}\nMr. {lastname_2} lives in the Rocky Mountains.",
                        line: 4,
                        col: 1,
                        offset: 116,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: Some(Span {
                        data: "About {author_2}",
                        line: 4,
                        col: 2,
                        offset: 117,
                    },),
                    title: Some("About B. Steppenwolf",),
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),
                Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "{firstname_3}, also known as {authorinitials_3}, loves to surf.",
                            line: 8,
                            col: 1,
                            offset: 200,
                        },
                        rendered: "Pax, also known as PD, loves to surf.",
                    },
                    source: Span {
                        data: ".About {author_3}\n{firstname_3}, also known as {authorinitials_3}, loves to surf.",
                        line: 7,
                        col: 1,
                        offset: 182,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: Some(Span {
                        data: "About {author_3}",
                        line: 7,
                        col: 2,
                        offset: 183,
                    },),
                    title: Some("About Pax Draeke",),
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),
                Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "You can contact {firstname} at {email}.",
                            line: 11,
                            col: 1,
                            offset: 281,
                        },
                        rendered: "You can contact Kismet at <a href=\"mailto:kismet@asciidoctor.org\">kismet@asciidoctor.org</a>.",
                    },
                    source: Span {
                        data: ".About {author}\nYou can contact {firstname} at {email}.",
                        line: 10,
                        col: 1,
                        offset: 265,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: Some(Span {
                        data: "About {author}",
                        line: 10,
                        col: 2,
                        offset: 266,
                    },),
                    title: Some("About Kismet R. Lee",),
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),
            ],
            source: Span {
                data: "= The Intrepid Chronicles\nKismet R. Lee <kismet@asciidoctor.org>; B. Steppenwolf; Pax Draeke <pax@asciidoctor.org>\n\n.About {author_2}\nMr. {lastname_2} lives in the Rocky Mountains.\n\n.About {author_3}\n{firstname_3}, also known as {authorinitials_3}, loves to surf.\n\n.About {author}\nYou can contact {firstname} at {email}.",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog::default(),
        }
    );
}

non_normative!(
    r#"
////
.Set the author and email attributes in the document header
[source]
----
= The Intrepid Chronicles
:author: Kismet R. Lee <1>
:email: kismet@asciidoctor.org <2>
:author_2: Mara_Moss Wirribi <3>
:email_2: mmw@asciidoctor.org <4>
:author_3: B. Steppenwolf <5>
:email_3: https://twitter.com/asciidoctor[@asciidoctor] <6>
----
<1> The first author is assigned to the built-in attribute `author`.
<2> The first author's email is assigned to the built-in attribute `email`.
<3> The second author is assigned to the built-in attribute `author_2`.
The underscore (`+_+`) between _Mara_ and _Moss_ tells the processor that the xref:compound-author-name.adoc[author has a double first name].
<4> The second author's email is assigned to the built-in attribute `email_2`.
<5> The third author is assigned to the built-in attribute `author_3`.
<6> The value of an author's email attribute, such as `email_3`, can contain an active URL and link text.
////
"#
);
