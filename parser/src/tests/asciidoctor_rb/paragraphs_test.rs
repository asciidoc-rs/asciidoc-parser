// Adapted from Asciidoctor's paragraphs test suite, found in
// https://github.com/asciidoctor/asciidoctor/blob/main/test/paragraphs_test.rb.
//
// The tests in this tree are adapted from the Ruby implementation of
// Asciidoctor, which comes with the following license:
//
// MIT License
//
// Copyright (C) 2012-present Dan Allen, Sarah White, Ryan Waldron, and the
// individual contributors to Asciidoctor.
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.
//
//! Port of Asciidoctor's `paragraphs_test.rb`.
//!
//! Each Ruby `context` becomes a Rust `mod` and each `test '..'` becomes a
//! `#[test] fn`, driven through [`Parser`](crate::Parser) and asserted with the
//! `assert_*` DOM helpers (or a full-AST `assert_eq!`) — the same conventions
//! documented in [`super`].
//!
//! Compatibility mode and non-HTML backends are out of scope for this crate, so
//! the DocBook `simpara`/`indexterm` tests are reproduced verbatim in
//! `non_normative!` blocks (they account for those Ruby lines without asserting
//! behavior this crate does not model). A handful of `inline_doctype` tests
//! have no Ruby
//! counterpart in v2.0.26 — they extend coverage of the boundary cases and so
//! carry no `verifies!` block.

use crate::tests::sdd::*;

track_file!("ref/asciidoctor/test/paragraphs_test.rb");

non_normative!(
    r#"
# frozen_string_literal: true
require_relative 'test_helper'

context 'Paragraphs' do
  context 'Normal' do
"#
);

mod normal {
    use crate::{document::RefType, tests::prelude::*};

    #[test]
    fn should_treat_plain_text_separated_by_blank_lines_as_paragraphs() {
        verifies!(
            r#"
    test 'should treat plain text separated by blank lines as paragraphs' do
      input = <<~'EOS'
      Plain text for the win!

      Yep. Text. Plain and simple.
      EOS
      output = convert_string_to_embedded input
      assert_css 'p', output, 2
      assert_xpath '(//p)[1][text() = "Plain text for the win!"]', output, 1
      assert_xpath '(//p)[2][text() = "Yep. Text. Plain and simple."]', output, 1
    end

"#
        );

        let doc =
            Parser::default().parse("Plain text for the win!\n\nYep. Text. Plain and simple.");

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
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "Plain text for the win!",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "Plain text for the win!",
                        },
                        source: Span {
                            data: "Plain text for the win!",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "Yep. Text. Plain and simple.",
                                line: 3,
                                col: 1,
                                offset: 25,
                            },
                            rendered: "Yep. Text. Plain and simple.",
                        },
                        source: Span {
                            data: "Yep. Text. Plain and simple.",
                            line: 3,
                            col: 1,
                            offset: 25,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: "Plain text for the win!\n\nYep. Text. Plain and simple.",
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
    fn should_associate_block_title_with_paragraph() {
        verifies!(
            r#"
    test 'should associate block title with paragraph' do
      input = <<~'EOS'
      .Titled
      Paragraph.

      Winning.
      EOS
      output = convert_string_to_embedded input

      assert_css 'p', output, 2
      assert_xpath '(//p)[1]/preceding-sibling::*[@class = "title"]', output, 1
      assert_xpath '(//p)[1]/preceding-sibling::*[@class = "title"][text() = "Titled"]', output, 1
      assert_xpath '(//p)[2]/preceding-sibling::*[@class = "title"]', output, 0
    end

"#
        );

        let doc = Parser::default().parse(".Titled\nParagraph.\n\nWinning.");

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
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "Paragraph.",
                                line: 2,
                                col: 1,
                                offset: 8,
                            },
                            rendered: "Paragraph.",
                        },
                        source: Span {
                            data: ".Titled\nParagraph.",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: Some(Span {
                            data: "Titled",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },),
                        title: Some("Titled",),
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "Winning.",
                                line: 4,
                                col: 1,
                                offset: 20,
                            },
                            rendered: "Winning.",
                        },
                        source: Span {
                            data: "Winning.",
                            line: 4,
                            col: 1,
                            offset: 20,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: ".Titled\nParagraph.\n\nWinning.",
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
    fn no_duplicate_block_before_next_section() {
        verifies!(
            r#"
    test 'no duplicate block before next section' do
      input = <<~'EOS'
      = Title

      Preamble

      == First Section

      Paragraph 1

      Paragraph 2

      == Second Section

      Last words
      EOS

      output = convert_string input
      assert_xpath '//p[text() = "Paragraph 2"]', output, 1
    end

"#
        );

        let doc = Parser::default().parse("= Title\n\nPreamble\n\n== First Section\n\nParagraph 1\n\nParagraph 2\n\n== Second Section\n\nLast words");

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
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "= Title",
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
                                    data: "Preamble",
                                    line: 3,
                                    col: 1,
                                    offset: 9,
                                },
                                rendered: "Preamble",
                            },
                            source: Span {
                                data: "Preamble",
                                line: 3,
                                col: 1,
                                offset: 9,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            caption: None,
                            number: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "Preamble",
                            line: 3,
                            col: 1,
                            offset: 9,
                        },
                    },),
                    Block::Section(SectionBlock {
                        level: 1,
                        section_title: Content {
                            original: Span {
                                data: "First Section",
                                line: 5,
                                col: 4,
                                offset: 22,
                            },
                            rendered: "First Section",
                        },
                        blocks: &[
                            Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Paragraph 1",
                                        line: 7,
                                        col: 1,
                                        offset: 37,
                                    },
                                    rendered: "Paragraph 1",
                                },
                                source: Span {
                                    data: "Paragraph 1",
                                    line: 7,
                                    col: 1,
                                    offset: 37,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                caption: None,
                                number: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),
                            Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "Paragraph 2",
                                        line: 9,
                                        col: 1,
                                        offset: 50,
                                    },
                                    rendered: "Paragraph 2",
                                },
                                source: Span {
                                    data: "Paragraph 2",
                                    line: 9,
                                    col: 1,
                                    offset: 50,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                caption: None,
                                number: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),
                        ],
                        source: Span {
                            data: "== First Section\n\nParagraph 1\n\nParagraph 2",
                            line: 5,
                            col: 1,
                            offset: 19,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                        section_type: SectionType::Normal,
                        section_id: Some("_first_section",),
                        caption: None,
                        section_number: None,
                    },),
                    Block::Section(SectionBlock {
                        level: 1,
                        section_title: Content {
                            original: Span {
                                data: "Second Section",
                                line: 11,
                                col: 4,
                                offset: 66,
                            },
                            rendered: "Second Section",
                        },
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "Last words",
                                    line: 13,
                                    col: 1,
                                    offset: 82,
                                },
                                rendered: "Last words",
                            },
                            source: Span {
                                data: "Last words",
                                line: 13,
                                col: 1,
                                offset: 82,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            caption: None,
                            number: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "== Second Section\n\nLast words",
                            line: 11,
                            col: 1,
                            offset: 63,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                        section_type: SectionType::Normal,
                        section_id: Some("_second_section",),
                        caption: None,
                        section_number: None,
                    },),
                ],
                source: Span {
                    data: "= Title\n\nPreamble\n\n== First Section\n\nParagraph 1\n\nParagraph 2\n\n== Second Section\n\nLast words",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([
                        (
                            "_first_section",
                            RefEntry {
                                id: "_first_section",
                                reftext: Some("First Section",),
                                ref_type: RefType::Section,
                            },
                        ),
                        (
                            "_second_section",
                            RefEntry {
                                id: "_second_section",
                                reftext: Some("Second Section",),
                                ref_type: RefType::Section,
                            },
                        ),
                    ]),
                    reftext_to_id: HashMap::from([
                        ("First Section", "_first_section",),
                        ("Second Section", "_second_section",),
                    ]),
                },
            }
        );
    }

    #[test]
    fn does_not_treat_wrapped_line_as_a_list_item() {
        verifies!(
            r#"
    test 'does not treat wrapped line as a list item' do
      input = <<~'EOS'
      paragraph
      . wrapped line
      EOS

      output = convert_string_to_embedded input
      assert_css 'p', output, 1
      assert_xpath %(//p[text()="paragraph\n. wrapped line"]), output, 1
    end

"#
        );

        let doc = Parser::default().parse("paragraph\n. wrapped line");

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
                            data: "paragraph\n. wrapped line",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "paragraph\n. wrapped line",
                    },
                    source: Span {
                        data: "paragraph\n. wrapped line",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: "paragraph\n. wrapped line",
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
    fn does_not_treat_wrapped_line_as_a_block_title() {
        verifies!(
            r#"
    test 'does not treat wrapped line as a block title' do
      input = <<~'EOS'
      paragraph
      .wrapped line
      EOS

      output = convert_string_to_embedded input
      assert_css 'p', output, 1
      assert_xpath %(//p[text()="paragraph\n.wrapped line"]), output, 1
    end

"#
        );

        let doc = Parser::default().parse("paragraph\n.wrapped line");

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
                            data: "paragraph\n.wrapped line",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "paragraph\n.wrapped line",
                    },
                    source: Span {
                        data: "paragraph\n.wrapped line",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: "paragraph\n.wrapped line",
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
    fn interprets_normal_paragraph_style_as_normal_paragraph() {
        verifies!(
            r#"
    test 'interprets normal paragraph style as normal paragraph' do
      input = <<~'EOS'
      [normal]
      Normal paragraph.
      Nothing special.
      EOS

      output = convert_string_to_embedded input
      assert_css 'p', output, 1
    end

"#
        );

        let doc = Parser::default().parse("[normal]\nNormal paragraph.\nNothing special.");

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
                            data: "Normal paragraph.\nNothing special.",
                            line: 2,
                            col: 1,
                            offset: 9,
                        },
                        rendered: "Normal paragraph.\nNothing special.",
                    },
                    source: Span {
                        data: "[normal]\nNormal paragraph.\nNothing special.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: None,
                            value: "normal",
                            shorthand_items: &["normal",],
                        },],
                        anchor: None,
                        source: Span {
                            data: "normal",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[normal]\nNormal paragraph.\nNothing special.",
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
    fn removes_indentation_from_literal_paragraph_marked_as_normal() {
        verifies!(
            r#"
    test 'removes indentation from literal paragraph marked as normal' do
      # NOTE cannot use single-quoted heredoc because of https://github.com/jruby/jruby/issues/4260
      input = <<~EOS
      [normal]
        Normal paragraph.
          Nothing special.
        Last line.
      EOS

      output = convert_string_to_embedded input
      assert_css 'p', output, 1
      assert_xpath %(//p[text()="Normal paragraph.\n  Nothing special.\nLast line."]), output, 1
    end

"#
        );

        let doc = Parser::default()
            .parse("[normal]\n Normal paragraph.\n  Nothing special.\n Last line.");

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
                            data: " Normal paragraph.\n  Nothing special.\n Last line.",
                            line: 2,
                            col: 1,
                            offset: 9,
                        },
                        rendered: "Normal paragraph.\n Nothing special.\nLast line.",
                    },
                    source: Span {
                        data: "[normal]\n Normal paragraph.\n  Nothing special.\n Last line.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: None,
                            value: "normal",
                            shorthand_items: &["normal"],
                        },],
                        anchor: None,
                        source: Span {
                            data: "normal",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[normal]\n Normal paragraph.\n  Nothing special.\n Last line.",
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
    fn normal_paragraph_terminates_at_block_attribute_list() {
        verifies!(
            r#"
    test 'normal paragraph terminates at block attribute list' do
      input = <<~'EOS'
      normal text
      [literal]
      literal text
      EOS
      output = convert_string_to_embedded input
      assert_css '.paragraph:root', output, 1
      assert_css '.literalblock:root', output, 1
    end

"#
        );

        let doc = Parser::default().parse("normal text\n[literal]\nliteral text");

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
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "normal text",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "normal text",
                        },
                        source: Span {
                            data: "normal text",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "literal text",
                                line: 3,
                                col: 1,
                                offset: 22,
                            },
                            rendered: "literal text",
                        },
                        source: Span {
                            data: "[literal]\nliteral text",
                            line: 2,
                            col: 1,
                            offset: 12,
                        },
                        style: SimpleBlockStyle::Literal,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: Some(Attrlist {
                            attributes: &[ElementAttribute {
                                name: None,
                                value: "literal",
                                shorthand_items: &["literal"],
                            },],
                            anchor: None,
                            source: Span {
                                data: "literal",
                                line: 2,
                                col: 2,
                                offset: 13,
                            },
                        },),
                    },),
                ],
                source: Span {
                    data: "normal text\n[literal]\nliteral text",
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
    fn normal_paragraph_terminates_at_block_delimiter() {
        verifies!(
            r#"
    test 'normal paragraph terminates at block delimiter' do
      input = <<~'EOS'
      normal text
      --
      text in open block
      --
      EOS
      output = convert_string_to_embedded input
      assert_css '.paragraph:root', output, 1
      assert_css '.openblock:root', output, 1
    end

"#
        );

        let doc = Parser::default().parse("normal text\n--\ntext in open block\n--");

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
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "normal text",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "normal text",
                        },
                        source: Span {
                            data: "normal text",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::CompoundDelimited(CompoundDelimitedBlock {
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "text in open block",
                                    line: 3,
                                    col: 1,
                                    offset: 15,
                                },
                                rendered: "text in open block",
                            },
                            source: Span {
                                data: "text in open block",
                                line: 3,
                                col: 1,
                                offset: 15,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            caption: None,
                            number: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        context: "open",
                        source: Span {
                            data: "--\ntext in open block\n--",
                            line: 2,
                            col: 1,
                            offset: 12,
                        },
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: "normal text\n--\ntext in open block\n--",
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
    fn normal_paragraph_terminates_at_list_continuation() {
        verifies!(
            r#"
    test 'normal paragraph terminates at list continuation' do
      input = <<~'EOS'
      normal text
      +
      EOS
      output = convert_string_to_embedded input
      assert_css '.paragraph:root', output, 2
      assert_xpath %((/*[@class="paragraph"])[1]/p[text() = "normal text"]), output, 1
      assert_xpath %((/*[@class="paragraph"])[2]/p[text() = "+"]), output, 1
    end

"#
        );

        let doc = Parser::default().parse("normal text\n+");

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
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "normal text",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "normal text",
                        },
                        source: Span {
                            data: "normal text",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "+",
                                line: 2,
                                col: 1,
                                offset: 12,
                            },
                            rendered: "+",
                        },
                        source: Span {
                            data: "+",
                            line: 2,
                            col: 1,
                            offset: 12,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: "normal text\n+",
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
    fn normal_style_turns_literal_paragraph_into_normal_paragraph() {
        verifies!(
            r#"
    test 'normal style turns literal paragraph into normal paragraph' do
      input = <<~'EOS'
      [normal]
       normal paragraph,
       despite the leading indent
      EOS

      output = convert_string_to_embedded input
      assert_css '.paragraph:root > p', output, 1
    end

"#
        );

        let doc =
            Parser::default().parse("[normal]\n normal paragraph,\n despite the leading indent");

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
                            data: " normal paragraph,\n despite the leading indent",
                            line: 2,
                            col: 1,
                            offset: 9,
                        },
                        rendered: "normal paragraph,\ndespite the leading indent",
                    },
                    source: Span {
                        data: "[normal]\n normal paragraph,\n despite the leading indent",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: None,
                            value: "normal",
                            shorthand_items: &["normal"],
                        },],
                        anchor: None,
                        source: Span {
                            data: "normal",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[normal]\n normal paragraph,\n despite the leading indent",
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

    // No planned support for DocBook output, so these tests were not ported from
    // asciidoctor_rb:
    //
    // * test 'automatically promotes index terms in DocBook output if
    //   indexterm-promotion-option is set'
    // * test 'does not automatically promote index terms in DocBook output if
    //   indexterm-promotion-option is not set'

    // Backend-specific tests omitted: DocBook index-term promotion.
    non_normative!(
        r#"
    test 'automatically promotes index terms in DocBook output if indexterm-promotion-option is set' do
      input = <<~'EOS'
      Here is an index entry for ((tigers)).
      indexterm:[Big cats,Tigers,Siberian Tiger]
      Here is an index entry for indexterm2:[Linux].
      (((Operating Systems,Linux)))
      Note that multi-entry terms generate separate index entries.
      EOS

      output = convert_string_to_embedded input, backend: 'docbook', attributes: { 'indexterm-promotion-option' => '' }
      assert_xpath '/simpara', output, 1
      term1 = xmlnodes_at_xpath '(//indexterm)[1]', output, 1
      assert_equal %(<indexterm>\n<primary>tigers</primary>\n</indexterm>), term1.to_s
      assert term1.next.content.start_with?('tigers')

      term2 = xmlnodes_at_xpath '(//indexterm)[2]', output, 1
      term2_elements = term2.elements
      assert_equal 3, term2_elements.size
      assert_equal '<primary>Big cats</primary>', term2_elements[0].to_s
      assert_equal '<secondary>Tigers</secondary>', term2_elements[1].to_s
      assert_equal '<tertiary>Siberian Tiger</tertiary>', term2_elements[2].to_s

      term3 = xmlnodes_at_xpath '(//indexterm)[3]', output, 1
      term3_elements = term3.elements
      assert_equal 2, term3_elements.size
      assert_equal '<primary>Tigers</primary>', term3_elements[0].to_s
      assert_equal '<secondary>Siberian Tiger</secondary>', term3_elements[1].to_s

      term4 = xmlnodes_at_xpath '(//indexterm)[4]', output, 1
      term4_elements = term4.elements
      assert_equal 1, term4_elements.size
      assert_equal '<primary>Siberian Tiger</primary>', term4_elements[0].to_s

      term5 = xmlnodes_at_xpath '(//indexterm)[5]', output, 1
      assert_equal %(<indexterm>\n<primary>Linux</primary>\n</indexterm>), term5.to_s
      assert term5.next.content.start_with?('Linux')

      assert_xpath '(//indexterm)[6]/*', output, 2
      assert_xpath '(//indexterm)[7]/*', output, 1
    end

    test 'does not automatically promote index terms in DocBook output if indexterm-promotion-option is not set' do
      input = <<~'EOS'
      The Siberian Tiger is one of the biggest living cats.
      indexterm:[Big cats,Tigers,Siberian Tiger]
      Note that multi-entry terms generate separate index entries.
      (((Operating Systems,Linux)))
      EOS

      output = convert_string_to_embedded input, backend: 'docbook'

      assert_css 'indexterm', output, 2

      terms = xmlnodes_at_css 'indexterm', output, 2
      term1 = terms[0]
      term1_elements = term1.elements
      assert_equal 3, term1_elements.size
      assert_equal '<primary>Big cats</primary>', term1_elements[0].to_s
      assert_equal '<secondary>Tigers</secondary>', term1_elements[1].to_s
      assert_equal '<tertiary>Siberian Tiger</tertiary>', term1_elements[2].to_s
      term2 = terms[1]
      term2_elements = term2.elements
      assert_equal 2, term2_elements.size
      assert_equal '<primary>Operating Systems</primary>', term2_elements[0].to_s
      assert_equal '<secondary>Linux</secondary>', term2_elements[1].to_s
    end

"#
    );

    #[test]
    fn normal_paragraph_should_honor_explicit_subs_list() {
        verifies!(
            r#"
    test 'normal paragraph should honor explicit subs list' do
      input = <<~'EOS'
      [subs="specialcharacters"]
      *<Hey Jude>*
      EOS

      output = convert_string_to_embedded input
      assert_includes output, '*&lt;Hey Jude&gt;*'
    end

"#
        );

        let doc = Parser::default().parse("[subs=\"specialcharacters\"]\n*<Hey Jude>*");

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
                            data: "*<Hey Jude>*",
                            line: 2,
                            col: 1,
                            offset: 27,
                        },
                        rendered: "*&lt;Hey Jude&gt;*",
                    },
                    source: Span {
                        data: "[subs=\"specialcharacters\"]\n*<Hey Jude>*",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: Some("subs",),
                            value: "specialcharacters",
                            shorthand_items: &[],
                        },],
                        anchor: None,
                        source: Span {
                            data: "subs=\"specialcharacters\"",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[subs=\"specialcharacters\"]\n*<Hey Jude>*",
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
    fn normal_paragraph_should_honor_specialchars_shorthand() {
        verifies!(
            r#"
    test 'normal paragraph should honor specialchars shorthand' do
      input = <<~'EOS'
      [subs="specialchars"]
      *<Hey Jude>*
      EOS

      output = convert_string_to_embedded input
      assert_includes output, '*&lt;Hey Jude&gt;*'
    end

"#
        );

        let doc = Parser::default().parse("[subs=\"specialchars\"]\n*<Hey Jude>*");

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
                            data: "*<Hey Jude>*",
                            line: 2,
                            col: 1,
                            offset: 22,
                        },
                        rendered: "*&lt;Hey Jude&gt;*",
                    },
                    source: Span {
                        data: "[subs=\"specialchars\"]\n*<Hey Jude>*",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: Some("subs",),
                            value: "specialchars",
                            shorthand_items: &[],
                        },],
                        anchor: None,
                        source: Span {
                            data: "subs=\"specialchars\"",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[subs=\"specialchars\"]\n*<Hey Jude>*",
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
    fn should_add_a_hardbreak_at_end_of_each_line_when_hardbreaks_option_is_set() {
        verifies!(
            r#"
    test 'should add a hardbreak at end of each line when hardbreaks option is set' do
      input = <<~'EOS'
      [%hardbreaks]
      read
      my
      lips
      EOS

      output = convert_string_to_embedded input
      assert_css 'br', output, 2
      assert_xpath '//p', output, 1
      assert_includes output, "<p>read<br>\nmy<br>\nlips</p>"
    end

"#
        );

        let doc = Parser::default().parse("[%hardbreaks]\nread\nmy\nlips");

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
                            data: "read\nmy\nlips",
                            line: 2,
                            col: 1,
                            offset: 14,
                        },
                        rendered: "read<br>\nmy<br>\nlips",
                    },
                    source: Span {
                        data: "[%hardbreaks]\nread\nmy\nlips",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
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
                    data: "[%hardbreaks]\nread\nmy\nlips",
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

    // This test stays `non_normative!`: the upstream `input` ends with the
    // single-line paragraph `roll it back`, which renders no `<br>` regardless
    // of the `hardbreaks` setting, so the upstream `(//p)[2]/br == 0` assertion
    // can't actually demonstrate that `:!hardbreaks:` took effect (see
    // https://github.com/asciidoctor/asciidoctor/issues/4818). The Rust test
    // below instead feeds a multi-line final paragraph (`roll\nit\nback`), so
    // the absence of `<br>` genuinely proves the toggle. Because the parser
    // never receives the verbatim upstream input, we don't claim line coverage
    // of it.
    #[test]
    fn should_be_able_to_toggle_hardbreaks_by_setting_hardbreaks_option_on_document() {
        non_normative!(
            r#"
    test 'should be able to toggle hardbreaks by setting hardbreaks-option on document' do
      input = <<~'EOS'
      :hardbreaks-option:

      make
      it
      so

      :!hardbreaks:

      roll it back
      EOS

      output = convert_string_to_embedded input
      assert_xpath '(//p)[1]/br', output, 2
      assert_xpath '(//p)[2]/br', output, 0
    end
"#
        );

        let doc = Parser::default()
            .parse(":hardbreaks-option:\n\nmake\nit\nso\n\n:!hardbreaks:\n\nroll\nit\nback");

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
                blocks: &[
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "make\nit\nso",
                                line: 3,
                                col: 1,
                                offset: 21,
                            },
                            rendered: "make<br>\nit<br>\nso",
                        },
                        source: Span {
                            data: "make\nit\nso",
                            line: 3,
                            col: 1,
                            offset: 21,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::DocumentAttribute(Attribute {
                        name: Span {
                            data: "hardbreaks",
                            line: 7,
                            col: 3,
                            offset: 35,
                        },
                        value_source: None,
                        value: InterpretedValue::Unset,
                        source: Span {
                            data: ":!hardbreaks:",
                            line: 7,
                            col: 1,
                            offset: 33,
                        },
                    },),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "roll\nit\nback",
                                line: 9,
                                col: 1,
                                offset: 48,
                            },
                            rendered: "roll\nit\nback",
                        },
                        source: Span {
                            data: "roll\nit\nback",
                            line: 9,
                            col: 1,
                            offset: 48,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: ":hardbreaks-option:\n\nmake\nit\nso\n\n:!hardbreaks:\n\nroll\nit\nback",
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
}

non_normative!(
    r#"
  end

  context 'Literal' do
"#
);

mod literal {
    use crate::tests::prelude::*;

    #[test]
    fn single_line_literal_paragraphs() {
        verifies!(
            r#"
    test 'single-line literal paragraphs' do
      # NOTE cannot use single-quoted heredoc because of https://github.com/jruby/jruby/issues/4260
      input = <<~EOS
      you know what?

       LITERALS

       ARE LITERALLY

       AWESOME!
      EOS
      output = convert_string_to_embedded input
      assert_xpath '//pre', output, 3
    end

"#
        );

        let doc =
            Parser::default().parse("you know what?\n\n LITERALS\n\n ARE LITERALLY\n\n AWESOME!");

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
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "you know what?",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "you know what?",
                        },
                        source: Span {
                            data: "you know what?",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: " LITERALS",
                                line: 3,
                                col: 1,
                                offset: 16,
                            },
                            rendered: "LITERALS",
                        },
                        source: Span {
                            data: " LITERALS",
                            line: 3,
                            col: 1,
                            offset: 16,
                        },
                        style: SimpleBlockStyle::Literal,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: " ARE LITERALLY",
                                line: 5,
                                col: 1,
                                offset: 27,
                            },
                            rendered: "ARE LITERALLY",
                        },
                        source: Span {
                            data: " ARE LITERALLY",
                            line: 5,
                            col: 1,
                            offset: 27,
                        },
                        style: SimpleBlockStyle::Literal,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: " AWESOME!",
                                line: 7,
                                col: 1,
                                offset: 43,
                            },
                            rendered: "AWESOME!",
                        },
                        source: Span {
                            data: " AWESOME!",
                            line: 7,
                            col: 1,
                            offset: 43,
                        },
                        style: SimpleBlockStyle::Literal,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: "you know what?\n\n LITERALS\n\n ARE LITERALLY\n\n AWESOME!",
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
    fn multi_line_literal_paragraphs() {
        verifies!(
            r#"
    test 'multi-line literal paragraph' do
      # NOTE cannot use single-quoted heredoc because of https://github.com/jruby/jruby/issues/4260
      input = <<~EOS
      Install instructions:

       yum install ruby rubygems
       gem install asciidoctor

      You're good to go!
      EOS
      output = convert_string_to_embedded input
      assert_xpath '//pre', output, 1
      # indentation should be trimmed from literal block
      assert_xpath %(//pre[text() = "yum install ruby rubygems\ngem install asciidoctor"]), output, 1
    end

"#
        );

        let doc =
            Parser::default().parse("Install instructions:\n\n yum install ruby rubygems\n gem install asciidoctor\n\nYou're good to go!");

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
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "Install instructions:",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "Install instructions:",
                        },
                        source: Span {
                            data: "Install instructions:",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: " yum install ruby rubygems\n gem install asciidoctor",
                                line: 3,
                                col: 1,
                                offset: 23,
                            },
                            rendered: "yum install ruby rubygems\ngem install asciidoctor",
                        },
                        source: Span {
                            data: " yum install ruby rubygems\n gem install asciidoctor",
                            line: 3,
                            col: 1,
                            offset: 23,
                        },
                        style: SimpleBlockStyle::Literal,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "You're good to go!",
                                line: 6,
                                col: 1,
                                offset: 76,
                            },
                            rendered: "You&#8217;re good to go!",
                        },
                        source: Span {
                            data: "You're good to go!",
                            line: 6,
                            col: 1,
                            offset: 76,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: "Install instructions:\n\n yum install ruby rubygems\n gem install asciidoctor\n\nYou're good to go!",
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
    fn literal_paragraph() {
        verifies!(
            r#"
    test 'literal paragraph' do
      input = <<~'EOS'
      [literal]
      this text is literally literal
      EOS
      output = convert_string_to_embedded input
      assert_xpath %(/*[@class="literalblock"]//pre[text()="this text is literally literal"]), output, 1
    end

"#
        );

        let doc = Parser::default().parse("[literal]\nthis text is literally literal");

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
                            data: "this text is literally literal",
                            line: 2,
                            col: 1,
                            offset: 10,
                        },
                        rendered: "this text is literally literal",
                    },
                    source: Span {
                        data: "[literal]\nthis text is literally literal",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Literal,
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: None,
                            value: "literal",
                            shorthand_items: &["literal"],
                        },],
                        anchor: None,
                        source: Span {
                            data: "literal",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[literal]\nthis text is literally literal",
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
    fn should_read_content_below_literal_style_verbatim() {
        verifies!(
            r#"
    test 'should read content below literal style verbatim' do
      input = <<~'EOS'
      [literal]
      image::not-an-image-block[]
      EOS
      output = convert_string_to_embedded input
      assert_xpath %(/*[@class="literalblock"]//pre[text()="image::not-an-image-block[]"]), output, 1
      assert_css 'img', output, 0
    end

"#
        );

        let doc = Parser::default().parse("[literal]\nimage::not-an-image-block[]");

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
                            data: "image::not-an-image-block[]",
                            line: 2,
                            col: 1,
                            offset: 10,
                        },
                        rendered: "image::not-an-image-block[]",
                    },
                    source: Span {
                        data: "[literal]\nimage::not-an-image-block[]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Literal,
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: None,
                            value: "literal",
                            shorthand_items: &["literal"],
                        },],
                        anchor: None,
                        source: Span {
                            data: "literal",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[literal]\nimage::not-an-image-block[]",
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
    fn listing_paragraph() {
        verifies!(
            r#"
    test 'listing paragraph' do
      input = <<~'EOS'
      [listing]
      this text is a listing
      EOS
      output = convert_string_to_embedded input
      assert_xpath %(/*[@class="listingblock"]//pre[text()="this text is a listing"]), output, 1
    end

"#
        );

        let doc = Parser::default().parse("[listing]\nthis text is a listing");

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
                            data: "this text is a listing",
                            line: 2,
                            col: 1,
                            offset: 10,
                        },
                        rendered: "this text is a listing",
                    },
                    source: Span {
                        data: "[listing]\nthis text is a listing",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Listing,
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: None,
                            value: "listing",
                            shorthand_items: &["listing"],
                        },],
                        anchor: None,
                        source: Span {
                            data: "listing",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[listing]\nthis text is a listing",
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
    fn source_paragraph() {
        verifies!(
            r#"
    test 'source paragraph' do
      input = <<~'EOS'
      [source]
      use the source, luke!
      EOS
      block = block_from_string input
      assert_equal :listing, block.context
      assert_equal 'source', (block.attr 'style')
      assert_equal :paragraph, (block.attr 'cloaked-context')
      assert_nil (block.attr 'language')
      output = convert_string_to_embedded input
      assert_xpath %(/*[@class="listingblock"]//pre[@class="highlight"]/code[text()="use the source, luke!"]), output, 1
    end

"#
        );

        let doc = Parser::default().parse("[source]\nuse the source, luke!");

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
                            data: "use the source, luke!",
                            line: 2,
                            col: 1,
                            offset: 9,
                        },
                        rendered: "use the source, luke!",
                    },
                    source: Span {
                        data: "[source]\nuse the source, luke!",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Source,
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: None,
                            value: "source",
                            shorthand_items: &["source"],
                        },],
                        anchor: None,
                        source: Span {
                            data: "source",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[source]\nuse the source, luke!",
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
    fn source_code_paragraph_with_language() {
        verifies!(
            r#"
    test 'source code paragraph with language' do
      input = <<~'EOS'
      [source, perl]
      die 'zomg perl is tough';
      EOS
      block = block_from_string input
      assert_equal :listing, block.context
      assert_equal 'source', (block.attr 'style')
      assert_equal :paragraph, (block.attr 'cloaked-context')
      assert_equal 'perl', (block.attr 'language')
      output = convert_string_to_embedded input
      assert_xpath %(/*[@class="listingblock"]//pre[@class="highlight"]/code[@class="language-perl"][@data-lang="perl"][text()="die 'zomg perl is tough';"]), output, 1
    end

"#
        );

        let doc = Parser::default().parse("[source, perl]\ndie 'zomg perl is tough';");

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
                            data: "die 'zomg perl is tough';",
                            line: 2,
                            col: 1,
                            offset: 15,
                        },
                        rendered: "die 'zomg perl is tough';",
                    },
                    source: Span {
                        data: "[source, perl]\ndie 'zomg perl is tough';",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Source,
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[
                            ElementAttribute {
                                name: None,
                                value: "source",
                                shorthand_items: &["source"],
                            },
                            ElementAttribute {
                                name: None,
                                value: "perl",
                                shorthand_items: &[],
                            },
                        ],
                        anchor: None,
                        source: Span {
                            data: "source, perl",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[source, perl]\ndie 'zomg perl is tough';",
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
    fn literal_paragraph_terminates_at_block_attribute_list() {
        verifies!(
            r#"
    test 'literal paragraph terminates at block attribute list' do
      # NOTE cannot use single-quoted heredoc because of https://github.com/jruby/jruby/issues/4260
      input = <<~EOS
       literal text
      [normal]
      normal text
      EOS
      output = convert_string_to_embedded input
      assert_xpath %(/*[@class="literalblock"]), output, 1
      assert_xpath %(/*[@class="paragraph"]), output, 1
    end

"#
        );

        let doc = Parser::default().parse(" literal text\n[normal]\nnormal text");

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
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: " literal text",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "literal text",
                        },
                        source: Span {
                            data: " literal text",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Literal,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "normal text",
                                line: 3,
                                col: 1,
                                offset: 23,
                            },
                            rendered: "normal text",
                        },
                        source: Span {
                            data: "[normal]\nnormal text",
                            line: 2,
                            col: 1,
                            offset: 14,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: Some(Attrlist {
                            attributes: &[ElementAttribute {
                                name: None,
                                value: "normal",
                                shorthand_items: &["normal"],
                            },],
                            anchor: None,
                            source: Span {
                                data: "normal",
                                line: 2,
                                col: 2,
                                offset: 15,
                            },
                        },),
                    },),
                ],
                source: Span {
                    data: " literal text\n[normal]\nnormal text",
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
    fn literal_paragraph_terminates_at_block_delimiter() {
        verifies!(
            r#"
    test 'literal paragraph terminates at block delimiter' do
      # NOTE cannot use single-quoted heredoc because of https://github.com/jruby/jruby/issues/4260
      input = <<~EOS
       literal text
      --
      normal text
      --
      EOS
      output = convert_string_to_embedded input
      assert_xpath %(/*[@class="literalblock"]), output, 1
      assert_xpath %(/*[@class="openblock"]), output, 1
    end

"#
        );

        let doc = Parser::default().parse(" literal text\n--\nnormal text\n--");

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
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: " literal text",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "literal text",
                        },
                        source: Span {
                            data: " literal text",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Literal,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::CompoundDelimited(CompoundDelimitedBlock {
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "normal text",
                                    line: 3,
                                    col: 1,
                                    offset: 17,
                                },
                                rendered: "normal text",
                            },
                            source: Span {
                                data: "normal text",
                                line: 3,
                                col: 1,
                                offset: 17,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            caption: None,
                            number: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        context: "open",
                        source: Span {
                            data: "--\nnormal text\n--",
                            line: 2,
                            col: 1,
                            offset: 14,
                        },
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: " literal text\n--\nnormal text\n--",
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
    fn literal_paragraph_terminates_at_list_continuation() {
        verifies!(
            r#"
    test 'literal paragraph terminates at list continuation' do
      # NOTE cannot use single-quoted heredoc because of https://github.com/jruby/jruby/issues/4260
      input = <<~EOS
       literal text
      +
      EOS
      output = convert_string_to_embedded input
      assert_xpath %(/*[@class="literalblock"]), output, 1
      assert_xpath %(/*[@class="literalblock"]//pre[text() = "literal text"]), output, 1
      assert_xpath %(/*[@class="paragraph"]), output, 1
      assert_xpath %(/*[@class="paragraph"]/p[text() = "+"]), output, 1
    end
"#
        );

        let doc = Parser::default().parse(" literal text\n+");

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
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: " literal text",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "literal text",
                        },
                        source: Span {
                            data: " literal text",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Literal,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "+",
                                line: 2,
                                col: 1,
                                offset: 14,
                            },
                            rendered: "+",
                        },
                        source: Span {
                            data: "+",
                            line: 2,
                            col: 1,
                            offset: 14,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: " literal text\n+",
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
}

non_normative!(
    r#"
  end

  context 'Quote' do
"#
);

mod quote {
    use crate::tests::prelude::*;

    #[test]
    fn single_line_quote_paragraph() {
        verifies!(
            r#"
    test "single-line quote paragraph" do
      input = <<~'EOS'
      [quote]
      Famous quote.
      EOS
      output = convert_string input
      assert_xpath '//*[@class = "quoteblock"]', output, 1
      assert_xpath '//*[@class = "quoteblock"]//p', output, 0
      assert_xpath '//*[@class = "quoteblock"]//*[contains(text(), "Famous quote.")]', output, 1
    end

"#
        );

        let doc = Parser::default().parse("[quote]\nFamous quote.");
        assert_xpath(&doc, "//*[@class = \"quoteblock\"]", 1);
        // A styled quote paragraph renders its text directly inside the
        // blockquote, not wrapped in a `<p>`.
        assert_xpath(&doc, "//*[@class = \"quoteblock\"]//p", 0);
        assert_rendered_contains(&doc, "Famous quote.");
    }

    #[test]
    fn quote_paragraph_terminates_at_list_continuation() {
        verifies!(
            r#"
    test 'quote paragraph terminates at list continuation' do
      input = <<~'EOS'
      [quote]
      A famouse quote.
      +
      EOS
      output = convert_string_to_embedded input
      assert_css '.quoteblock:root', output, 1
      assert_css '.paragraph:root', output, 1
      assert_xpath %(/*[@class="paragraph"]/p[text() = "+"]), output, 1
    end

"#
        );

        let doc = Parser::default().parse("[quote]\nA famouse quote.\n+");
        assert_css(&doc, ".quoteblock", 1);
        // The list-continuation marker (`+`) terminates the quote paragraph and
        // becomes its own paragraph.
        assert_css(&doc, ".paragraph", 1);
        assert_xpath(&doc, "//*[@class=\"paragraph\"]/p[text() = \"+\"]", 1);
    }

    #[test]
    fn verse_paragraph() {
        verifies!(
            r#"
    test "verse paragraph" do
      output = convert_string("[verse]\nFamous verse.")
      assert_xpath '//*[@class = "verseblock"]', output, 1
      assert_xpath '//*[@class = "verseblock"]/pre', output, 1
      assert_xpath '//*[@class = "verseblock"]//p', output, 0
      assert_xpath '//*[@class = "verseblock"]/pre[normalize-space(text()) = "Famous verse."]', output, 1
    end

"#
        );

        let doc = Parser::default().parse("[verse]\nFamous verse.");
        assert_xpath(&doc, "//*[@class = \"verseblock\"]", 1);
        assert_xpath(&doc, "//*[@class = \"verseblock\"]/pre", 1);
        assert_xpath(&doc, "//*[@class = \"verseblock\"]//p", 0);
        assert_xpath(
            &doc,
            "//*[@class = \"verseblock\"]/pre[normalize-space(text()) = \"Famous verse.\"]",
            1,
        );
    }

    #[test]
    fn should_perform_normal_subs_on_a_verse_paragraph() {
        verifies!(
            r##"
    test 'should perform normal subs on a verse paragraph' do
      input = <<~'EOS'
      [verse]
      _GET /groups/link:#group-id[\{group-id\}]_
      EOS

      output = convert_string_to_embedded input
      assert_includes output, '<pre class="content"><em>GET /groups/<a href="#group-id">{group-id}</a></em></pre>'
    end

"##
        );

        let doc = Parser::default().parse("[verse]\n_GET /groups/link:#group-id[\\{group-id\\}]_");
        let block = doc.child_blocks().next().unwrap();
        assert_eq!(
            block.rendered_content(),
            Some("<em>GET /groups/<a href=\"#group-id\">{group-id}</a></em>")
        );
    }

    #[test]
    fn quote_paragraph_should_honor_explicit_subs_list() {
        verifies!(
            r#"
    test 'quote paragraph should honor explicit subs list' do
      input = <<~'EOS'
      [subs="specialcharacters"]
      [quote]
      *Hey Jude*
      EOS

      output = convert_string_to_embedded input
      assert_includes output, '*Hey Jude*'
    end
"#
        );

        let doc = Parser::default().parse("[subs=\"specialcharacters\"]\n[quote]\n*Hey Jude*");
        // Only special-character substitution runs, so the `*` is left intact
        // (not converted to a `<strong>`).
        assert_rendered_contains(&doc, "*Hey Jude*");
    }
}

non_normative!(
    r#"
  end

  context "special" do
"#
);

mod special {
    use crate::tests::prelude::*;

    // Asciidoctor's `ADMONITION_STYLES`: the five built-in admonition labels.
    const ADMONITION_STYLES: [&str; 5] = ["NOTE", "TIP", "IMPORTANT", "WARNING", "CAUTION"];

    // A styled paragraph whose style is an admonition label renders as an
    // admonition block.
    #[test]
    fn note_multiline_syntax() {
        verifies!(
            r#"
    test "note multiline syntax" do
      Asciidoctor::ADMONITION_STYLES.each do |style|
        assert_xpath "//div[@class='admonitionblock #{style.downcase}']", convert_string("[#{style}]\nThis is a winner.")
      end
    end

"#
        );

        for style in ADMONITION_STYLES {
            let doc = Parser::default().parse(&format!("[{style}]\nThis is a winner."));
            assert_xpath(
                &doc,
                &format!(
                    "//div[@class = \"admonitionblock {}\"]",
                    style.to_lowercase()
                ),
                1,
            );
            assert_rendered_contains(&doc, "This is a winner.");
        }
    }

    // An example-delimited block carrying an admonition style renders as an
    // admonition block.
    #[test]
    fn note_block_syntax() {
        verifies!(
            r#"
    test "note block syntax" do
      Asciidoctor::ADMONITION_STYLES.each do |style|
        assert_xpath "//div[@class='admonitionblock #{style.downcase}']", convert_string("[#{style}]\n====\nThis is a winner.\n====")
      end
    end

"#
        );

        for style in ADMONITION_STYLES {
            let doc = Parser::default().parse(&format!("[{style}]\n====\nThis is a winner.\n===="));
            assert_xpath(
                &doc,
                &format!(
                    "//div[@class = \"admonitionblock {}\"]",
                    style.to_lowercase()
                ),
                1,
            );
            assert_rendered_contains(&doc, "This is a winner.");
        }
    }

    // The inline label form (e.g. `NOTE: ...`) renders as an admonition block.
    #[test]
    fn note_inline_syntax() {
        verifies!(
            r##"
    test "note inline syntax" do
      Asciidoctor::ADMONITION_STYLES.each do |style|
        assert_xpath "//div[@class='admonitionblock #{style.downcase}']", convert_string("#{style}: This is important, fool!")
      end
    end

"##
        );

        for style in ADMONITION_STYLES {
            let doc = Parser::default().parse(&format!("{style}: This is important, fool!"));
            assert_xpath(
                &doc,
                &format!(
                    "//div[@class = \"admonitionblock {}\"]",
                    style.to_lowercase()
                ),
                1,
            );
            assert_rendered_contains(&doc, "This is important, fool!");
        }
    }

    #[test]
    fn should_process_preprocessor_conditional_in_paragraph_content() {
        verifies!(
            r#"
    test 'should process preprocessor conditional in paragraph content' do
      input = <<~'EOS'
      ifdef::asciidoctor-version[]
      [sidebar]
      First line of sidebar.
      ifdef::backend[The backend is {backend}.]
      Last line of sidebar.
      endif::[]
      EOS

      expected = <<~'EOS'.chop
      <div class="sidebarblock">
      <div class="content">
      First line of sidebar.
      The backend is html5.
      Last line of sidebar.
      </div>
      </div>
      EOS

      result = convert_string_to_embedded input
      assert_equal expected, result
    end

"#
        );

        // `asciidoctor-version` is predefined by this crate; `backend` is not
        // set by default, so it is supplied explicitly to mirror the Ruby
        // environment.
        let doc = Parser::default()
            .with_intrinsic_attribute("backend", "html5", ModificationContext::Anywhere)
            .parse(
                "ifdef::asciidoctor-version[]\n[sidebar]\nFirst line of sidebar.\nifdef::backend[The backend is {backend}.]\nLast line of sidebar.\nendif::[]",
            );

        // The outer `ifdef` includes the sidebar; the inner single-line `ifdef`
        // resolves to the backend value.
        assert_output_contains(&doc, "First line of sidebar.");
        assert_output_contains(&doc, "The backend is html5.");
        assert_output_contains(&doc, "Last line of sidebar.");
    }

    // Backend-specific test omitted: DocBook `simpara` wrapping of styled
    // paragraphs.
    non_normative!(
        r#"
    context 'Styled Paragraphs' do
      test 'should wrap text in simpara for styled paragraphs when converted to DocBook' do
        input = <<~'EOS'
        = Book
        :doctype: book

        [preface]
        = About this book

        [abstract]
        An abstract for the book.

        = Part 1

        [partintro]
        An intro to this part.

        == Chapter 1

        [sidebar]
        Just a side note.

        [example]
        As you can see here.

        [quote]
        Wise words from a wise person.

        [open]
        Make it what you want.
        EOS

        output = convert_string input, backend: 'docbook'
        assert_css 'abstract > simpara', output, 1
        assert_css 'partintro > simpara', output, 1
        assert_css 'sidebar > simpara', output, 1
        assert_css 'informalexample > simpara', output, 1
        assert_css 'blockquote > simpara', output, 1
        assert_css 'chapter > simpara', output, 1
      end

"#
    );

    // An `[open]` styled paragraph renders as an open block whose inline content
    // sits directly in the content wrapper, with no wrapping `<p>`.
    #[test]
    fn should_convert_open_paragraph_to_open_block() {
        verifies!(
            r#"
      test 'should convert open paragraph to open block' do
        input = <<~'EOS'
        [open]
        Make it what you want.
        EOS

        output = convert_string_to_embedded input
        assert_css '.openblock', output, 1
        assert_css '.openblock p', output, 0
      end

"#
        );

        let doc = Parser::default().parse("[open]\nMake it what you want.");
        assert_css(&doc, ".openblock", 1);
        assert_css(&doc, ".openblock p", 0);

        // The inline content sits directly in the `div.content` wrapper (no
        // wrapping `<p>`), so assert the wrapper structure and that the text
        // lives inside it — not merely somewhere in the rendered tree.
        assert_css(&doc, "div.openblock > div.content", 1);
        assert_xpath(
            &doc,
            "//div[@class = \"openblock\"]/div[@class = \"content\"][contains(text(), \"Make it what you want.\")]",
            1,
        );
    }

    // Backend-specific test omitted: DocBook `simpara` wrapping of titled styled
    // paragraphs. (This block also closes the Ruby `Styled Paragraphs` context
    // and opens `Inline doctype`.)
    non_normative!(
        r#"
      test 'should wrap text in simpara for styled paragraphs with title when converted to DocBook' do
        input = <<~'EOS'
        = Book
        :doctype: book

        [preface]
        = About this book

        [abstract]
        .Abstract title
        An abstract for the book.

        = Part 1

        [partintro]
        .Part intro title
        An intro to this part.

        == Chapter 1

        [sidebar]
        .Sidebar title
        Just a side note.

        [example]
        .Example title
        As you can see here.

        [quote]
        .Quote title
        Wise words from a wise person.
        EOS

        output = convert_string input, backend: 'docbook'
        assert_css 'abstract > title', output, 1
        assert_xpath '//abstract/title[text() = "Abstract title"]', output, 1
        assert_css 'abstract > title + simpara', output, 1
        assert_css 'partintro > title', output, 1
        assert_xpath '//partintro/title[text() = "Part intro title"]', output, 1
        assert_css 'partintro > title + simpara', output, 1
        assert_css 'sidebar > title', output, 1
        assert_xpath '//sidebar/title[text() = "Sidebar title"]', output, 1
        assert_css 'sidebar > title + simpara', output, 1
        assert_css 'example > title', output, 1
        assert_xpath '//example/title[text() = "Example title"]', output, 1
        assert_css 'example > title + simpara', output, 1
        assert_css 'blockquote > title', output, 1
        assert_xpath '//blockquote/title[text() = "Quote title"]', output, 1
        assert_css 'blockquote > title + simpara', output, 1
      end
    end

    context 'Inline doctype' do
"#
    );

    // Ported from Ruby Asciidoctor's paragraphs_test.rb, `context 'Inline
    // doctype'`. Under `doctype: inline` the document converts only its first
    // block, as bare inline content; a compound (or empty) first block has no
    // inline candidate and is reported.
    mod inline_doctype {
        use crate::tests::prelude::*;

        // Ported from 'should only format and output text in first paragraph
        // when doctype is inline'.
        #[test]
        fn should_only_output_first_paragraph() {
            verifies!(
                r#"
      test 'should only format and output text in first paragraph when doctype is inline' do
        input = "http://asciidoc.org[AsciiDoc] is a _lightweight_ markup language...\n\nignored"
        output = convert_string input, doctype: 'inline'
        assert_equal '<a href="http://asciidoc.org">AsciiDoc</a> is a <em>lightweight</em> markup language&#8230;&#8203;', output
      end

"#
            );

            let doc = Parser::default()
                .with_intrinsic_attribute("doctype", "inline", ModificationContext::Anywhere)
                .parse(
                    "http://asciidoc.org[AsciiDoc] is a _lightweight_ markup language...\n\nignored",
                );

            // The first paragraph is rendered as bare inline content, with the
            // full inline substitutions applied and no `<div class="paragraph">
            // <p>` wrapper.
            let first = doc.child_blocks().next().unwrap();
            assert_eq!(
                first.rendered_content(),
                Some(
                    "<a href=\"http://asciidoc.org\">AsciiDoc</a> is a <em>lightweight</em> markup language&#8230;&#8203;"
                )
            );
            assert_css(&doc, ".paragraph", 0);
            assert_xpath(&doc, "//a[@href = \"http://asciidoc.org\"]", 1);
            assert_xpath(&doc, "//em[text() = \"lightweight\"]", 1);

            // The second paragraph (`ignored`) is dropped from the output.
            refute_rendered_contains(&doc, "ignored");

            // A valid inline candidate produces no warning.
            assert!(doc.warnings().next().is_none());
        }

        // Ported from 'should output nil and warn if first block is not a
        // paragraph'. A list is a compound block, so it is not an inline
        // candidate: nothing is emitted and a warning is logged.
        #[test]
        fn should_warn_if_first_block_is_not_a_paragraph() {
            verifies!(
                r#"
      test 'should output nil and warn if first block is not a paragraph' do
        input = '* bullet'
        using_memory_logger do |logger|
          output = convert_string input, doctype: 'inline'
          assert_nil output
          assert_message logger, :WARN, '~no inline candidate'
        end
      end
"#
            );

            let doc = Parser::default()
                .with_intrinsic_attribute("doctype", "inline", ModificationContext::Anywhere)
                .parse("* bullet");

            let warnings: Vec<_> = doc.warnings().collect();
            assert_eq!(warnings.len(), 1);
            assert_eq!(warnings[0].warning, WarningType::NoInlineDoctypeCandidate);

            // Nothing is rendered (the list is not an inline candidate).
            refute_rendered_contains(&doc, "bullet");
        }

        // A leading `[comment]`-styled paragraph produces no output, so it is
        // transparent: the inline candidate is the first *following* block, and
        // the comment text is never emitted.
        #[test]
        fn should_skip_leading_comment_styled_paragraph() {
            let doc = Parser::default()
                .with_intrinsic_attribute("doctype", "inline", ModificationContext::Anywhere)
                .parse("[comment]\nthis is a comment\n\nvisible text");

            assert!(doc.warnings().next().is_none());
            assert_rendered_contains(&doc, "visible text");
            refute_rendered_contains(&doc, "this is a comment");
        }

        // A leading `////` comment block is likewise transparent: its raw
        // content is never emitted, and the following paragraph is the
        // candidate.
        #[test]
        fn should_skip_leading_comment_block() {
            let doc = Parser::default()
                .with_intrinsic_attribute("doctype", "inline", ModificationContext::Anywhere)
                .parse("////\nhidden comment\n////\n\nvisible text");

            assert!(doc.warnings().next().is_none());
            assert_rendered_contains(&doc, "visible text");
            refute_rendered_contains(&doc, "hidden comment");
        }

        // A document title followed by a paragraph and a section wraps the
        // paragraph in a compound preamble, which is the first block. A compound
        // candidate has no inline content, so the parse-time warning and the
        // (empty) rendered output stay in agreement — the preamble never masks a
        // silently-dropped candidate.
        //
        // This matches Asciidoctor 2.0.26 exactly: `= Title\n\nfirst
        // paragraph\n\n== Section` under `-d inline` logs `no inline candidate`
        // and emits nothing (the preamble is `@blocks[0]`, and its content model
        // is compound). Rendering the inner paragraph instead would diverge from
        // Asciidoctor.
        #[test]
        fn preamble_is_a_compound_candidate() {
            let doc = Parser::default()
                .with_intrinsic_attribute("doctype", "inline", ModificationContext::Anywhere)
                .parse("= Title\n\nfirst paragraph\n\n== Section\n\nbody");

            let warnings: Vec<_> = doc.warnings().collect();
            assert_eq!(warnings.len(), 1);
            assert_eq!(warnings[0].warning, WarningType::NoInlineDoctypeCandidate);

            refute_rendered_contains(&doc, "first paragraph");
            refute_rendered_contains(&doc, "body");
        }

        // A title with a paragraph but *no* section creates no preamble (this
        // crate, like Asciidoctor, only wraps a preamble when a section follows),
        // so the paragraph is itself the first block and is rendered as inline
        // content with no warning. Asciidoctor 2.0.26 emits `first paragraph`
        // for `= Title\n\nfirst paragraph` under `-d inline`. This is the
        // boundary that distinguishes the candidate from the compound-preamble
        // case above.
        #[test]
        fn titled_paragraph_without_section_is_rendered() {
            let doc = Parser::default()
                .with_intrinsic_attribute("doctype", "inline", ModificationContext::Anywhere)
                .parse("= Title\n\nfirst paragraph");

            assert!(doc.warnings().next().is_none());
            assert_rendered_contains(&doc, "first paragraph");
        }
    }
}

non_normative!(
    r#"
    end
  end

  context 'Custom' do
"#
);

// Covers how an unregistered (unknown) paragraph style is handled.
mod custom {
    use crate::tests::prelude::*;

    // An unknown block style such as `[foo]` on a paragraph is accepted
    // silently: the style is retained on the block, the content is parsed as an
    // ordinary paragraph, and no warning is emitted.
    #[test]
    fn should_not_warn_if_paragraph_style_is_unregistered() {
        verifies!(
            r#"
    test 'should not warn if paragraph style is unregisted' do
      input = <<~'EOS'
      [foo]
      bar
      EOS
      using_memory_logger do |logger|
        convert_string_to_embedded input
        assert_empty logger.messages
      end
    end

"#
        );

        let doc = Parser::default().parse("[foo]\nbar");

        // The unknown style is retained on the block, and its content is parsed
        // as an ordinary paragraph.
        let block = doc.child_blocks().next().unwrap();
        assert_eq!(block.declared_style(), Some("foo"));
        assert_eq!(block.rendered_content(), Some("bar"));

        // Asciidoctor asserts no messages at its *default* (WARN) severity. This
        // crate surfaces the unknown style as a `WarningSeverity::Debug`
        // diagnostic (see the debug-level test below), which a host suppresses
        // by default, so the default-severity view is empty.
        assert_eq!(
            doc.warnings()
                .filter(|w| w.severity >= WarningSeverity::Warning)
                .count(),
            0
        );
    }

    #[test]
    fn should_log_debug_message_if_paragraph_style_is_unknown_and_debug_level_is_enabled() {
        verifies!(
            r#"
    test 'should log debug message if paragraph style is unknown and debug level is enabled' do
      input = <<~'EOS'
      [foo]
      bar
      EOS
      using_memory_logger Logger::Severity::DEBUG do |logger|
        convert_string_to_embedded input
        assert_message logger, :DEBUG, '<stdin>: line 2: unknown style for paragraph: foo', Hash
      end
    end
"#
        );

        // Asciidoctor logs this only at DEBUG severity (below its default WARN
        // threshold). This crate records it as a single `WarningSeverity::Debug`
        // warning, observable via `Document::warnings()`. The paragraph keeps
        // its default context; the style `foo` is retained but otherwise
        // ignored.
        let doc = Parser::default().parse("[foo]\nbar");

        let warning = doc.warnings().next().unwrap();
        assert_eq!(warning.severity, WarningSeverity::Debug);
        assert_eq!(
            warning,
            &Warning {
                source: Span {
                    data: "[foo]\nbar",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warning: WarningType::UnknownBlockStyle("paragraph".to_string(), "foo".to_string()),
            }
        );
        assert_eq!(doc.warnings().count(), 1);
    }
}

non_normative!(
    r#"
  end
end
"#
);
