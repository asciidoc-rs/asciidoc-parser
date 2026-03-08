use crate::{Parser, blocks::SimpleBlockStyle, document::RefType, tests::prelude::*};

track_file!("docs/modules/document/pages/compound-author-name.adoc");

non_normative!(
    r#"
= Compound Author Names

When a name consists of multiple parts, such as a compound or composite surname, or a double middle name, the processor needs to be explicitly told which words should be assigned to a specific attribute.

"#
);

#[test]
fn connecting_compound_author_names() {
    non_normative!(
        r#"
== Connecting compound author names

"#
    );

    verifies!(
        r#"
If the parts of an author's name aren't assigned to the correct built-in attributes, they may output the wrong information if they're referenced in the body of the document.
For instance, if the name _Ann Marie Jenson_ was entered on the author line or assigned to the attribute `author`, the processor would assign _Ann_ to `firstname`, _Marie_ to `middlename`, and _Jenson_ to `lastname` based on the location and order of each word.
This assignment would be incorrect because the author's first name is _Ann Marie_.

When part of an author's name consists of more than one word, use an underscore (`+_+`) between the words to connect them.

.Compound name syntax
[source]
----
= Document Title
firstname_firstname lastname; firstname middlename_middlename lastname
----

If the more than three space-separated names (or initials) are entered in the implicit author line, the entire line (including the email portion) will be used as the author's full name and first name.
Thus, it's important to use the underscore separator to ensure there are no more than three space-separated names.

"#
    );

    let mut parser = Parser::default();
    let doc = parser.parse(
        "= Document Title\nfirstname_firstname lastname; firstname middlename_middlename lastname",
    );

    assert_eq!(
        doc,
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
                author_line: Some(AuthorLine {
                    authors: &[
                        Author {
                            name: "firstname firstname lastname",
                            firstname: "firstname firstname",
                            middlename: None,
                            lastname: Some("lastname",),
                            email: None,
                        },
                        Author {
                            name: "firstname middlename middlename lastname",
                            firstname: "firstname",
                            middlename: Some("middlename middlename",),
                            lastname: Some("lastname",),
                            email: None,
                        },
                    ],
                    source: Span {
                        data: "firstname_firstname lastname; firstname middlename_middlename lastname",
                        line: 2,
                        col: 1,
                        offset: 17,
                    },
                },),
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= Document Title\nfirstname_firstname lastname; firstname middlename_middlename lastname",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[],
            source: Span {
                data: "= Document Title\nfirstname_firstname lastname; firstname middlename_middlename lastname",
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

#[test]
fn ex_line_compound() {
    non_normative!(
        r#"
== Compound names in the author line

"#
    );

    verifies!(
        r#"
In <<ex-line-compound>>, the first author has a compound first name and the second author has a compound surname.

.Assign compound names in the author line
[source#ex-line-compound]
----
= Drum and Bass Breakbeats
Ann_Marie Jenson; Tomás López_del_Toro <.> <.>
----
<.> To signal to the processor that _Ann Marie_ is the author's first name (instead of their first and middle names), type an underscore (`+_+`) between each part of the author's first name.
<.> The second author's last name consists of three words.
Type an underscore (`+_+`) between each word of the author's last name.

The result of <<ex-line-compound>> is displayed below.
Notice that the underscores (`+_+`) aren't displayed when the document is rendered.

image::author-line-with-compound-names.png[Compound author names displayed in the byline,role=screenshot]

"#
    );

    let mut parser = Parser::default();
    let doc = parser.parse("= Drum and Bass Breakbeats\nAnn_Marie Jenson; Tomás López_del_Toro");

    assert_eq!(
        doc,
        Document {
            header: Header {
                title_source: Some(Span {
                    data: "Drum and Bass Breakbeats",
                    line: 1,
                    col: 3,
                    offset: 2,
                },),
                title: Some("Drum and Bass Breakbeats",),
                attributes: &[],
                author_line: Some(AuthorLine {
                    authors: &[
                        Author {
                            name: "Ann Marie Jenson",
                            firstname: "Ann Marie",
                            middlename: None,
                            lastname: Some("Jenson",),
                            email: None,
                        },
                        Author {
                            name: "Tomás López del Toro",
                            firstname: "Tomás",
                            middlename: None,
                            lastname: Some("López del Toro",),
                            email: None,
                        },
                    ],
                    source: Span {
                        data: "Ann_Marie Jenson; Tomás López_del_Toro",
                        line: 2,
                        col: 1,
                        offset: 27,
                    },
                },),
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= Drum and Bass Breakbeats\nAnn_Marie Jenson; Tomás López_del_Toro",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[],
            source: Span {
                data: "= Drum and Bass Breakbeats\nAnn_Marie Jenson; Tomás López_del_Toro",
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

#[test]
fn ex_reference_compound() {
    verifies!(
        r#"
The underscore between each word in a compound name ensures that the parts of an author's name are assigned correctly to the corresponding built-in attributes.
If you were to reference the first author's first name or the second author's last name in the document body, as shown in <<ex-reference-compound>>, the correct values would be displayed.

.Reference authors with compound names
[source#ex-reference-compound]
----
= Drum and Bass Breakbeats
Ann_Marie Jenson; Tomás López_del_Toro

The first author's first name is {firstname}.

The second author's last name is {lastname_2}.
----

Like in the byline, the underscores (`+_+`) aren't displayed when the document is rendered.

image::reference-compound-names.png[Compound author names displayed in the document body when referenced,role=screenshot]

"#
    );

    let mut parser = Parser::default();
    let doc = parser.parse(
        "= Drum and Bass Breakbeats\nAnn_Marie Jenson; Tomás López_del_Toro\n\nThe first author's first name is {firstname}.\n\nThe second author's last name is {lastname_2}.",
    );

    assert_eq!(
        doc,
        Document {
            header: Header {
                title_source: Some(Span {
                    data: "Drum and Bass Breakbeats",
                    line: 1,
                    col: 3,
                    offset: 2,
                },),
                title: Some("Drum and Bass Breakbeats",),
                attributes: &[],
                author_line: Some(AuthorLine {
                    authors: &[
                        Author {
                            name: "Ann Marie Jenson",
                            firstname: "Ann Marie",
                            middlename: None,
                            lastname: Some("Jenson",),
                            email: None,
                        },
                        Author {
                            name: "Tomás López del Toro",
                            firstname: "Tomás",
                            middlename: None,
                            lastname: Some("López del Toro",),
                            email: None,
                        },
                    ],
                    source: Span {
                        data: "Ann_Marie Jenson; Tomás López_del_Toro",
                        line: 2,
                        col: 1,
                        offset: 27,
                    },
                },),
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= Drum and Bass Breakbeats\nAnn_Marie Jenson; Tomás López_del_Toro",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[
                Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "The first author's first name is {firstname}.",
                            line: 4,
                            col: 1,
                            offset: 69,
                        },
                        rendered: "The first author&#8217;s first name is Ann Marie.",
                    },
                    source: Span {
                        data: "The first author's first name is {firstname}.",
                        line: 4,
                        col: 1,
                        offset: 69,
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
                            data: "The second author's last name is {lastname_2}.",
                            line: 6,
                            col: 1,
                            offset: 116,
                        },
                        rendered: "The second author&#8217;s last name is López del Toro.",
                    },
                    source: Span {
                        data: "The second author's last name is {lastname_2}.",
                        line: 6,
                        col: 1,
                        offset: 116,
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
                data: "= Drum and Bass Breakbeats\nAnn_Marie Jenson; Tomás López_del_Toro\n\nThe first author's first name is {firstname}.\n\nThe second author's last name is {lastname_2}.",
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

#[test]
fn ex_compound() {
    non_normative!(
        r#"
== Compound names in the author attribute

"#
    );

    verifies!(
        r#"
An underscore (`+_+`) should also be placed between each part of a compound name when the author is assigned using the `author` attribute.

.Assign a compound name using the author attribute
[source#ex-compound]
----
= Quantum Networks
:author: Mara_Moss Wirribi <.>

== About {author}

{firstname} lives on the Bellarine Peninsula near Geelong, Australia. <.>
----
<.> Assign the author's name to the `author` attribute.
Enter an underscore (`+_+`) between each part of the author's first name.
This ensures that their full first name is correct when it's automatically assigned to `firstname` by the processor.
<.> The built-in attribute `firstname` is referenced in the document's body.
The author's first name is automatically extracted from the value of `author` and assigned to `firstname`.

The result of <<ex-compound>>, displayed below, shows that the processor assigned the correct words to the built-in attribute `firstname` since the author's full first name, _Mara Moss_, is displayed where `firstname` was referenced.

image::author-attribute-with-compound-name.png[Compound author name displayed in the byline using the author attribute,role=screenshot]
"#
    );

    let mut parser = Parser::default();
    let doc = parser.parse(
        "= Quantum Networks\n:author: Mara_Moss Wirribi\n\n== About {author}\n\n{firstname} lives on the Bellarine Peninsula near Geelong, Australia.",
    );

    assert_eq!(
        doc,
        Document {
            header: Header {
                title_source: Some(Span {
                    data: "Quantum Networks",
                    line: 1,
                    col: 3,
                    offset: 2,
                },),
                title: Some("Quantum Networks",),
                attributes: &[Attribute {
                    name: Span {
                        data: "author",
                        line: 2,
                        col: 2,
                        offset: 20,
                    },
                    value_source: Some(Span {
                        data: "Mara_Moss Wirribi",
                        line: 2,
                        col: 10,
                        offset: 28,
                    },),
                    value: InterpretedValue::Value("Mara_Moss Wirribi",),
                    source: Span {
                        data: ":author: Mara_Moss Wirribi",
                        line: 2,
                        col: 1,
                        offset: 19,
                    },
                },],
                author_line: None,
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= Quantum Networks\n:author: Mara_Moss Wirribi",
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
                        line: 4,
                        col: 4,
                        offset: 50,
                    },
                    rendered: "About Mara_Moss Wirribi",
                },
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "{firstname} lives on the Bellarine Peninsula near Geelong, Australia.",
                            line: 6,
                            col: 1,
                            offset: 66,
                        },
                        rendered: "Mara Moss lives on the Bellarine Peninsula near Geelong, Australia.",
                    },
                    source: Span {
                        data: "{firstname} lives on the Bellarine Peninsula near Geelong, Australia.",
                        line: 6,
                        col: 1,
                        offset: 66,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: "== About {author}\n\n{firstname} lives on the Bellarine Peninsula near Geelong, Australia.",
                    line: 4,
                    col: 1,
                    offset: 47,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
                section_type: SectionType::Normal,
                section_id: Some("_about_mara_moss_wirribi"),
                section_number: None,
            },),],
            source: Span {
                data: "= Quantum Networks\n:author: Mara_Moss Wirribi\n\n== About {author}\n\n{firstname} lives on the Bellarine Peninsula near Geelong, Australia.",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog {
                refs: HashMap::from([(
                    "_about_mara_moss_wirribi",
                    RefEntry {
                        id: "_about_mara_moss_wirribi",
                        reftext: Some("About Mara_Moss Wirribi",),
                        ref_type: RefType::Section,
                    }
                ),]),
                reftext_to_id: HashMap::from([(
                    "About Mara_Moss Wirribi",
                    "_about_mara_moss_wirribi"
                ),]),
            },
        }
    );
}
