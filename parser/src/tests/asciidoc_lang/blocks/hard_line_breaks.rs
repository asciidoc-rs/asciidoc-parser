use crate::{blocks::SimpleBlockStyle, tests::prelude::*};

track_file!("docs/modules/blocks/pages/hard-line-breaks.adoc");

non_normative!(
    r#"
= Hard Line Breaks

Adjacent lines of regular text in AsciiDoc are combined into a single paragraph when converted.
That means you can wrap paragraph text in the source document, either at a specific column or by putting each sentence or phrase on its own line.
The line breaks separating adjacent lines won't appear in the output.
Instead, the line break will be (effectively) converted to a single space.
(In fact, all repeating space characters are reduced to a single space, just like in HTML.)

TIP: Hard line breaks are automatically retained in xref:verbatim:literal-blocks.adoc[literal], xref:verbatim:listing-blocks.adoc[listing], xref:verbatim:source-blocks.adoc[source], and xref:verses.adoc[verse] blocks and paragraphs.

If you want line breaks in a paragraph to be preserved, there are several techniques you can use.
"#
);

#[test]
fn space_plus_syntax() {
    verifies!(
        r#"
For any single line, you can terminate it with a space followed by a plus sign.
This syntax signals to the processor to end the line in the output with a hard line break.

[source]
----
line one +
line two
----

"#
    );

    let doc = Parser::default().parse("line one +\nline two");

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
            blocks: &[Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "line one +\nline two",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "line one<br>\nline two",
                },
                source: Span {
                    data: "line one +\nline two",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },),],
            source: Span {
                data: "line one +\nline two",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog {
                refs: HashMap::from([]),
                reftext_to_id: HashMap::from([]),
            },
        }
    );
}

#[test]
fn hardbreaks_option_paragraph() {
    verifies!(
        r#"
To add this behavior to every line in the paragraph, set the `hardbreaks` option on the paragraph instead.

[source]
----
[%hardbreaks]
line one
line two
----

"#
    );

    let doc = Parser::default().parse("[%hardbreaks]\nline one\nline two");

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
            blocks: &[Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "line one\nline two",
                        line: 2,
                        col: 1,
                        offset: 14,
                    },
                    rendered: "line one<br>\nline two",
                },
                source: Span {
                    data: "[%hardbreaks]\nline one\nline two",
                    line: 1,
                    col: 1,
                    offset: 0,
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
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },),],
            source: Span {
                data: "[%hardbreaks]\nline one\nline two",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog {
                refs: HashMap::from([]),
                reftext_to_id: HashMap::from([]),
            },
        }
    );
}

#[test]
fn hardbreaks_option_document_attribute() {
    verifies!(
        r#"
Alternately, you can tell the processor to preserve all line breaks in every paragraph in the document by setting the `hardbreaks-option` document attribute, though this option should be used wisely.

[source]
----
:hardbreaks-option:

line one
line two
----

"#
    );

    let doc = Parser::default().parse(":hardbreaks-option:\n\nline one\nline two");

    assert_eq!(
        doc,
        Document {
            header: Header {
                title_source: None,
                title: None,
                attributes: &[Attribute {
                    name: Span {
                        data: "hardbreaks-option",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                    value_source: None,
                    value: InterpretedValue::Set,
                    source: Span {
                        data: ":hardbreaks-option:",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },],
                author_line: None,
                revision_line: None,
                comments: &[],
                source: Span {
                    data: ":hardbreaks-option:",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "line one\nline two",
                        line: 3,
                        col: 1,
                        offset: 21,
                    },
                    rendered: "line one<br>\nline two",
                },
                source: Span {
                    data: "line one\nline two",
                    line: 3,
                    col: 1,
                    offset: 21,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },),],
            source: Span {
                data: ":hardbreaks-option:\n\nline one\nline two",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog {
                refs: HashMap::from([]),
                reftext_to_id: HashMap::from([]),
            },
        }
    );
}

#[test]
fn hard_line_break_solo() {
    verifies!(
        r#"
To insert an empty line in the middle of the paragraph, you can use the hard line break syntax on a line by itself.
This allows you to insert space between lines in the output without introducing separate paragraphs.

[source]
----
line one +
 +
line three
----

"#
    );

    let doc = Parser::default().parse("line one +\n +\nline three");

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
            blocks: &[Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "line one +\n +\nline three",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "line one<br>\n<br>\nline three",
                },
                source: Span {
                    data: "line one +\n +\nline three",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },),],
            source: Span {
                data: "line one +\n +\nline three",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog {
                refs: HashMap::from([]),
                reftext_to_id: HashMap::from([]),
            },
        }
    );
}

#[test]
fn start_with_hard_line_break() {
    verifies!(
        r#"
If you want the paragraph to start with a hard line break, you need to place an `\{empty}` attribute reference at the start of the line.
That's because a line that starts with a space has a different meaning.
The `\{empty}` attribute reference allows you to insert nothing at the start of the line.

[source]
----
{empty} +
line two
----

"#
    );

    let doc = Parser::default().parse("{empty} +\nline two");

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
            blocks: &[Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "{empty} +\nline two",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "<br>\nline two",
                },
                source: Span {
                    data: "{empty} +\nline two",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },),],
            source: Span {
                data: "{empty} +\nline two",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog {
                refs: HashMap::from([]),
                reftext_to_id: HashMap::from([]),
            },
        }
    );
}

#[test]
fn use_empty_consistently() {
    verifies!(
        r#"
To be consistent, you can always start an empty line with `\{empty}`.

[source]
----
{empty} +
line two +
{empty} +
line four
----

Note that `empty` is a built-in document attribute in AsciiDoc.

"#
    );

    let doc = Parser::default().parse("{empty} +\nline two +\n{empty} +\nline four");

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
            blocks: &[Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "{empty} +\nline two +\n{empty} +\nline four",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "<br>\nline two<br>\n<br>\nline four",
                },
                source: Span {
                    data: "{empty} +\nline two +\n{empty} +\nline four",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },),],
            source: Span {
                data: "{empty} +\nline two +\n{empty} +\nline four",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog {
                refs: HashMap::from([]),
                reftext_to_id: HashMap::from([]),
            },
        }
    );
}

#[test]
#[ignore]
fn per_line_dialogue_syntax() {
    // TO DO (https://github.com/asciidoc-rs/asciidoc-parser/issues/440): Await spec clarity on correct behavior for this example.
    verifies!(
        r#"
If you're writing a story with dialogue, and you want to prefix the dialogue lines with `--`, the per-line syntax is the most appropriate choice.
For example:

[source]
----
-- Come here! -- I said. +
-- What is it? -- replied Lance.
----

If you were to use the `hardbreaks` option instead, the second `--` would not only be substituted with an endash, it would also consume the preceding newline.
As a result, both lines in the source would end up appearing on the same line in the output.

"#
    );

    let doc =
        Parser::default().parse("-- Come here! -- I said. +\n-- What is it? -- replied Lance.");

    dbg!(&doc);
    todo!("This appears to be generating wrong results");
}

#[test]
fn inline_line_break_syntax() {
    verifies!(
        r#"
[#per-line]
== Inline line break syntax

To preserve a line break in a paragraph, insert a space followed by a plus sign (`{plus}`) at the end of the line.
This results in a visible line break (e.g., `<br>`) following the line.

.Line breaks preserved using a space followed by the plus sign ({plus})
[#ex-plus]
----
include::example$paragraph.adoc[tag=hb]
----

The result of <<ex-plus>> is displayed below.

====
include::example$paragraph.adoc[tag=hb]
====

"#
    );

    let doc = Parser::default().parse("Rubies are red, +\nTopazes are blue.");

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
            blocks: &[Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "Rubies are red, +\nTopazes are blue.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "Rubies are red,<br>\nTopazes are blue.",
                },
                source: Span {
                    data: "Rubies are red, +\nTopazes are blue.",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },),],
            source: Span {
                data: "Rubies are red, +\nTopazes are blue.",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog {
                refs: HashMap::from([]),
                reftext_to_id: HashMap::from([]),
            },
        }
    );
}

#[test]
fn hardbreaks_option_syntax() {
    verifies!(
        r#"
[#per-block]
== hardbreaks option

To retain all of the line breaks in an entire paragraph, assign the `hardbreaks` option to the paragraph using an attribute list.

.Line breaks preserved using the hardbreaks option
[#ex-option]
----
include::example$paragraph.adoc[tag=hb-p]
----

The result of <<ex-option>> is displayed below.

====
include::example$paragraph.adoc[tag=hb-p]
====

"#
    );

    let doc = Parser::default().parse("[%hardbreaks]\nRuby is red.\nJava is beige.");

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
            blocks: &[Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "Ruby is red.\nJava is beige.",
                        line: 2,
                        col: 1,
                        offset: 14,
                    },
                    rendered: "Ruby is red.<br>\nJava is beige.",
                },
                source: Span {
                    data: "[%hardbreaks]\nRuby is red.\nJava is beige.",
                    line: 1,
                    col: 1,
                    offset: 0,
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
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },),],
            source: Span {
                data: "[%hardbreaks]\nRuby is red.\nJava is beige.",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog {
                refs: HashMap::from([]),
                reftext_to_id: HashMap::from([]),
            },
        }
    );
}

#[test]
fn hardbreaks_option_attribute() {
    verifies!(
        r#"
[#per-document]
== hardbreaks-option attribute

To preserve line breaks in all paragraphs throughout your entire document, set the `hardbreaks-option` document attribute in the document header.

.Line breaks preserved throughout the document using the hardbreaks-option attribute
[#ex-attribute]
----
include::example$paragraph.adoc[tag=hb-attr]
----
"#
    );

    let doc = Parser::default()
        .parse("= Line Break Doc Title\n:hardbreaks-option:\n\nRubies are red,\nTopazes are blue.");

    assert_eq!(
        doc,
        Document {
            header: Header {
                title_source: Some(Span {
                    data: "Line Break Doc Title",
                    line: 1,
                    col: 3,
                    offset: 2,
                },),
                title: Some("Line Break Doc Title",),
                attributes: &[Attribute {
                    name: Span {
                        data: "hardbreaks-option",
                        line: 2,
                        col: 2,
                        offset: 24,
                    },
                    value_source: None,
                    value: InterpretedValue::Set,
                    source: Span {
                        data: ":hardbreaks-option:",
                        line: 2,
                        col: 1,
                        offset: 23,
                    },
                },],
                author_line: None,
                revision_line: None,
                comments: &[],
                source: Span {
                    data: "= Line Break Doc Title\n:hardbreaks-option:",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
            },
            blocks: &[Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "Rubies are red,\nTopazes are blue.",
                        line: 4,
                        col: 1,
                        offset: 44,
                    },
                    rendered: "Rubies are red,<br>\nTopazes are blue.",
                },
                source: Span {
                    data: "Rubies are red,\nTopazes are blue.",
                    line: 4,
                    col: 1,
                    offset: 44,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },),],
            source: Span {
                data: "= Line Break Doc Title\n:hardbreaks-option:\n\nRubies are red,\nTopazes are blue.",
                line: 1,
                col: 1,
                offset: 0,
            },
            warnings: &[],
            source_map: SourceMap(&[]),
            catalog: Catalog {
                refs: HashMap::from([]),
                reftext_to_id: HashMap::from([]),
            },
        }
    );
}
