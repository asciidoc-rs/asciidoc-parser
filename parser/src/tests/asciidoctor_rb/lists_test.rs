// Adapted from Asciidoctor's lists test suite, found in
// https://github.com/asciidoctor/asciidoctor/blob/main/test/lists_test.rb.
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
// IMPORTANT: In porting this, I've disregarded compatibility mode (stated
// limitation of `asciidoc-parser` crate) and alternate (non-HTML) back ends.

#![allow(unreachable_code)]
#![allow(unused_imports)]

mod bulleted_lists {
    use crate::{Parser, tests::prelude::*};

    mod simple_lists {
        use crate::{
            Parser,
            blocks::{ListType, SimpleBlockStyle},
            document::RefType,
            tests::prelude::*,
        };

        #[test]
        fn dash_elements_with_no_blank_lines() {
            let doc = Parser::default().parse("List\n====\n\n- Foo\n- Boo\n- Blech\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li", 3);
        }

        #[test]
        fn indented_dash_elements_using_spaces() {
            let doc = Parser::default().parse(" - Foo\n - Boo\n - Blech\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li", 3);
        }

        #[test]
        fn indented_dash_elements_using_tabs() {
            let doc = Parser::default().parse("\t-\tFoo\n\t-\tBoo\n\t-\tBlech\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li", 3);
        }

        #[test]
        fn dash_elements_separated_by_blank_lines_should_merge_lists() {
            let doc = Parser::default().parse("List\n====\n\n- Foo\n\n- Boo\n\n\n- Blech\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li", 3);
        }

        #[test]
        fn dash_elements_with_interspersed_line_comments_should_be_skipped_and_not_break_list() {
            let doc = Parser::default().parse("== List\n\n- Foo\n// line comment\n// another line comment\n- Boo\n// line comment\nmore text\n// another line comment\n- Blech\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li", 3);
            assert_xpath(&doc, "(//ul/li)[2]/p[text()=\"Boo\\nmore text\"]", 1);
        }

        #[test]
        fn dash_elements_separated_by_a_line_comment_offset_by_blank_lines_should_not_merge_lists()
        {
            let doc = Parser::default().parse("List\n====\n\n- Foo\n- Boo\n\n//\n\n- Blech\n");

            assert_xpath(&doc, "//ul", 2);
            assert_xpath(&doc, "(//ul)[1]/li", 2);
            assert_xpath(&doc, "(//ul)[2]/li", 1);
        }

        #[test]
        fn dash_elements_separated_by_a_block_title_offset_by_a_blank_line_should_not_merge_lists()
        {
            let doc = Parser::default().parse("== List\n\n- Foo\n- Boo\n\n.Also\n- Blech\n");

            assert_xpath(&doc, "//ul", 2);
            assert_xpath(&doc, "(//ul)[1]/li", 2);
            assert_xpath(&doc, "(//ul)[2]/li", 1);

            assert_xpath(
                &doc,
                "(//ul)[2]/preceding-sibling::*[@class = \"title\"][text() = \"Also\"]",
                1,
            );
        }

        #[test]
        fn dash_elements_separated_by_an_attribute_entry_offset_by_a_blank_line_should_not_merge_lists()
         {
            let doc = Parser::default().parse("== List\n\n- Foo\n- Boo\n\n:foo: bar\n- Blech\n");

            assert_xpath(&doc, "//ul", 2);
            assert_xpath(&doc, "(//ul)[1]/li", 2);
            assert_xpath(&doc, "(//ul)[2]/li", 1);
        }

        #[test]
        fn a_non_indented_wrapped_line_is_folded_into_text_of_list_item() {
            let doc =
                Parser::default().parse("List\n====\n\n- Foo\nwrapped content\n- Boo\n- Blech\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li[1]/*", 1);
            assert_xpath(&doc, "//ul/li[1]/p[text() = 'Foo\\nwrapped content']", 1);
        }

        #[test]
        fn a_non_indented_wrapped_line_that_resembles_a_block_title_is_folded_into_text_of_list_item()
         {
            let doc =
                Parser::default().parse("== List\n\n- Foo\n.wrapped content\n- Boo\n- Blech\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li[1]/*", 1);
            assert_xpath(&doc, "//ul/li[1]/p[text() = 'Foo\\n.wrapped content']", 1);
        }

        #[test]
        fn a_non_indented_wrapped_line_that_resembles_an_attribute_entry_is_folded_into_text_of_list_item()
         {
            let doc = Parser::default().parse("== List\n\n- Foo\n:foo: bar\n- Boo\n- Blech\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li[1]/*", 1);
            assert_xpath(&doc, "//ul/li[1]/p[text() = 'Foo\\n:foo: bar']", 1);
        }

        #[test]
        fn a_list_item_with_a_nested_marker_terminates_non_indented_paragraph_for_text_of_list_item()
         {
            let doc = Parser::default().parse("- Foo\nBar\n* Foo\n");

            assert_css(&doc, "ul ul", 1);
            refute_output_contains(&doc, "* Foo");
        }

        #[test]
        fn a_list_item_for_a_different_list_terminates_non_indented_paragraph_for_text_of_list_item()
         {
            let doc = Parser::default().parse(
                "== Example 1\n\n- Foo\nBar\n. Foo\n\n== Example 2\n\n* Item\ntext\nterm:: def\n",
            );

            assert_css(&doc, "ul ol", 1);
            refute_output_contains(&doc, "* Foo");
            assert_css(&doc, "ul dl", 1);
            refute_output_contains(&doc, "term:: def");
        }

        #[test]
        fn an_indented_wrapped_line_is_unindented_and_folded_into_text_of_list_item() {
            let doc =
                Parser::default().parse("== List\n\n- Foo\n  wrapped content\n- Boo\n- Blech\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li[1]/*", 1);
            assert_xpath(&doc, "//ul/li[1]/p[text() = \'Foo\nwrapped content\']", 1);
        }

        #[test]
        fn wrapped_list_item_with_hanging_indent_followed_by_non_indented_line() {
            let doc = Parser::default().parse("== Lists\n\n- list item 1\n  // not line comment\nsecond wrapped line\n- list item 2\n");

            assert_css(&doc, "ul", 1);
            assert_css(&doc, "ul li", 2);

            // Asciidoctor test on which this is based comes with the following comment:
            // > NOTE: for some reason, we're getting an extra line after the indented line.
            //
            // Looks like we're not having that problem in the Rust port. Use this detailed
            // comparison to ensure that remains true. todo!("xmlnodes_at_xpath
            // check for 3 lines");

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
                    blocks: &[Block::Section(SectionBlock {
                        level: 1,
                        section_title: Content {
                            original: Span {
                                data: "Lists",
                                line: 1,
                                col: 4,
                                offset: 3,
                            },
                            rendered: "Lists",
                        },
                        blocks: &[Block::List(ListBlock {
                            type_: ListType::Unordered,
                            items: &[
                                Block::ListItem(ListItem {
                                    marker: ListItemMarker::Hyphen(Span {
                                        data: "-",
                                        line: 3,
                                        col: 1,
                                        offset: 10,
                                    },),
                                    blocks: &[Block::Simple(SimpleBlock {
                                        content: Content {
                                            original: Span {
                                                data: "list item 1\n  // not line comment\nsecond wrapped line",
                                                line: 3,
                                                col: 3,
                                                offset: 12,
                                            },
                                            rendered: "list item 1\n// not line comment\nsecond wrapped line",
                                        },
                                        source: Span {
                                            data: "list item 1\n  // not line comment\nsecond wrapped line",
                                            line: 3,
                                            col: 3,
                                            offset: 12,
                                        },
                                        style: SimpleBlockStyle::Paragraph,
                                        title_source: None,
                                        title: None,
                                        anchor: None,
                                        anchor_reftext: None,
                                        attrlist: None,
                                    },),],
                                    source: Span {
                                        data: "- list item 1\n  // not line comment\nsecond wrapped line",
                                        line: 3,
                                        col: 1,
                                        offset: 10,
                                    },
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                                Block::ListItem(ListItem {
                                    marker: ListItemMarker::Hyphen(Span {
                                        data: "-",
                                        line: 6,
                                        col: 1,
                                        offset: 66,
                                    },),
                                    blocks: &[Block::Simple(SimpleBlock {
                                        content: Content {
                                            original: Span {
                                                data: "list item 2",
                                                line: 6,
                                                col: 3,
                                                offset: 68,
                                            },
                                            rendered: "list item 2",
                                        },
                                        source: Span {
                                            data: "list item 2",
                                            line: 6,
                                            col: 3,
                                            offset: 68,
                                        },
                                        style: SimpleBlockStyle::Paragraph,
                                        title_source: None,
                                        title: None,
                                        anchor: None,
                                        anchor_reftext: None,
                                        attrlist: None,
                                    },),],
                                    source: Span {
                                        data: "- list item 2",
                                        line: 6,
                                        col: 1,
                                        offset: 66,
                                    },
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                            ],
                            source: Span {
                                data: "- list item 1\n  // not line comment\nsecond wrapped line\n- list item 2",
                                line: 3,
                                col: 1,
                                offset: 10,
                            },
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "== Lists\n\n- list item 1\n  // not line comment\nsecond wrapped line\n- list item 2",
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
                        section_id: Some("_lists",),
                        section_number: None,
                    },),],
                    source: Span {
                        data: "== Lists\n\n- list item 1\n  // not line comment\nsecond wrapped line\n- list item 2",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    warnings: &[],
                    source_map: SourceMap(&[]),
                    catalog: Catalog {
                        refs: HashMap::from([(
                            "_lists",
                            RefEntry {
                                id: "_lists",
                                reftext: Some("Lists",),
                                ref_type: RefType::Section,
                            },
                        ),]),
                        reftext_to_id: HashMap::from([("Lists", "_lists",),]),
                    },
                }
            );
        }

        #[test]
        fn a_list_item_with_a_nested_marker_terminates_indented_paragraph_for_text_of_list_item() {
            let doc = Parser::default().parse("- Foo\n  Bar\n* Foo\n");

            assert_css(&doc, "ul ul", 1);
            refute_output_contains(&doc, "* Foo");
        }

        #[test]
        fn a_list_item_that_starts_with_a_sequence_of_list_markers_characters_should_not_match_a_nested_list()
         {
            let doc = Parser::default().parse(" * first item\n *. normal text\n");

            assert_css(&doc, "ul", 1);
            assert_css(&doc, "ul li", 1);
            assert_xpath(&doc, "//ul/li/p[text()='first item\n*. normal text\']", 1);
        }

        #[test]
        fn a_list_item_for_a_different_list_terminates_indented_paragraph_for_text_of_list_item() {
            let doc = Parser::default().parse("== Example 1\n\n- Foo\n  Bar\n. Foo\n\n== Example 2\n\n* Item\n  text\nterm:: def\n");

            assert_css(&doc, "ul ol", 1);
            refute_output_contains(&doc, "* Foo");
            assert_css(&doc, "ul dl", 1);
            refute_output_contains(&doc, "term:: def");
        }

        #[test]
        fn a_literal_paragraph_offset_by_blank_lines_in_list_content_is_appended_as_a_literal_block()
         {
            let doc = Parser::default().parse("== List\n\n- Foo\n\n  literal\n\n- Boo\n- Blech\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li", 3);
            assert_xpath(&doc, "(//ul/li)[1]/p[text() = \"Foo\"]", 1);
            assert_xpath(&doc, "(//ul/li)[1]/*[@class=\"literalblock\"]", 1);

            assert_xpath(
                &doc,
                "(//ul/li)[1]/p/following-sibling::*[@class=\"literalblock\"]",
                1,
            );

            assert_xpath(
                &doc,
                "((//ul/li)[1]/*[@class=\"literalblock\"])[1]//pre[text() = \"literal\"]",
                1,
            );
        }

        #[test]
        fn should_escape_special_characters_in_all_literal_paragraphs_attached_to_list_item() {
            let doc = Parser::default().parse("* first item\n\n  <code>text</code>\n\n  more <code>text</code>\n\n* second item\n");

            assert_css(&doc, "li", 2);
            assert_css(&doc, "code", 0);
            assert_css(&doc, "li:first-of-type > *", 3);
            assert_css(&doc, "li:first-of-type pre", 2);
            assert_xpath(&doc, "((//li)[1]//pre)[1][text()=\"<code>text</code>\"]", 1);

            assert_xpath(
                &doc,
                "((//li)[1]//pre)[2][text()=\"more <code>text</code>\"]",
                1,
            );
        }

        #[test]
        fn a_literal_paragraph_offset_by_a_blank_line_in_list_content_followed_by_line_with_continuation_is_appended_as_two_blocks()
         {
            let doc = Parser::default()
                .parse("== List\n\n- Foo\n\n  literal\n+\npara\n\n- Boo\n- Blech\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li", 3);
            assert_xpath(&doc, "(//ul/li)[1]/p[text() = \"Foo\"]", 1);
            assert_xpath(&doc, "(//ul/li)[1]/*[@class=\"literalblock\"]", 1);

            assert_xpath(
                &doc,
                "(//ul/li)[1]/p/following-sibling::*[@class=\"literalblock\"]",
                1,
            );

            assert_xpath(
                &doc,
                "((//ul/li)[1]/*[@class=\"literalblock\"])[1]//pre[text() = \"literal\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//ul/li)[1]/*[@class=\"literalblock\"]/following-sibling::*[@class=\"paragraph\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//ul/li)[1]/*[@class=\"literalblock\"]/following-sibling::*[@class=\"paragraph\"]/p[text()=\"para\"]",
                1,
            );
        }

        #[test]
        #[ignore]
        // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/456):
        // Enable this test when admonitions are implemented.
        fn an_admonition_paragraph_attached_by_a_line_continuation_to_a_list_item_with_wrapped_text_should_produce_admonition()
         {
            let _doc = Parser::default()
                .parse("- first-line text\n  wrapped text\n+\nNOTE: This is a note.\n");
            todo!("assert_css: 'ul', output, 1");
            todo!("assert_css: 'ul > li', output, 1");
            todo!("assert_css: 'ul > li > p', output, 1");
            todo!(
                "assert_xpath: '//ul/li/p[text()=\"first-line text\\nwrapped text\"]', output, 1"
            );
            todo!("assert_css: 'ul > li > p + .admonitionblock.note', output, 1");
            todo!(
                "assert_xpath: '//ul/li/*[@class=\"admonitionblock note\"]//td[@class=\"content\"][normalize-space(text())=\"This is a note.\"]', output, 1"
            );
        }

        #[test]
        #[ignore]
        // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/456):
        // Enable this test when admonitions are implemented.
        fn paragraph_like_blocks_attached_to_an_ancestor_list_item_by_a_list_continuation_should_produce_blocks()
         {
            let _doc = Parser::default().parse("* parent\n ** child\n\n+\nNOTE: This is a note.\n\n* another parent\n ** another child\n\n+\n'''\n");
            todo!("assert_css: 'ul ul .admonitionblock.note', output, 0");
            todo!("assert_xpath: '(//ul)[1]/li/*[@class=\"admonitionblock note\"]', output, 1");
            todo!("assert_css: 'ul ul hr', output, 0");
            todo!("assert_xpath: '(//ul)[1]/li/hr', output, 1");
        }

        #[test]
        #[ignore]
        // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/311):
        // Enable this test when callouts are implemented.
        fn should_not_inherit_block_attributes_from_previous_block_when_block_is_attached_using_a_list_continuation()
         {
            let _doc = Parser::default().parse("* complex list item\n+\n[source,xml]\n----\n<name>value</name> <!--1-->\n----\n<1> a configuration value\n");
            todo!("doc.blocks[0].items[0].blocks[-1] check");
            todo!("assert_css: 'ul', output, 1");
            todo!("assert_css: 'ul > li', output, 1");
            todo!("assert_css: 'ul > li > p', output, 1");
            todo!("assert_css: 'ul > li > .listingblock', output, 1");
            todo!("assert_css: 'ul > li > .colist', output, 1");
        }

        #[test]
        fn should_continue_to_parse_blocks_attached_by_a_list_continuation_after_block_is_dropped()
        {
            let doc = Parser::default().parse(
                "* item\n+\nparagraph\n+\n[comment]\ncomment\n+\n====\nexample\n====\n'''\n",
            );

            assert_css(&doc, "ul > li > .paragraph", 1);
            assert_css(&doc, "ul > li > .exampleblock", 1);
        }

        #[test]
        fn appends_line_as_paragraph_if_attached_by_continuation_following_line_comment() {
            let doc = Parser::default().parse(
                "- list item 1\n// line comment\n+\nparagraph in list item 1\n\n- list item 2\n",
            );

            assert_css(&doc, "ul", 1);
            assert_css(&doc, "ul li", 2);

            assert_xpath(&doc, "(//ul/li)[1]/p[text()=\"list item 1\"]", 1);

            assert_xpath(
                &doc,
                "(//ul/li)[1]/p/following-sibling::*[@class=\"paragraph\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//ul/li)[1]/p/following-sibling::*[@class=\"paragraph\"]/p[text()=\"paragraph in list item 1\"]",
                1,
            );

            assert_xpath(&doc, "(//ul/li)[2]/p[text()=\"list item 2\"]", 1);
        }

        #[test]
        fn a_literal_paragraph_with_a_line_that_appears_as_a_list_item_that_is_followed_by_a_continuation_should_create_two_blocks()
         {
            let doc =
                Parser::default().parse("* Foo\n+\n  literal\n. still literal\n+\npara\n\n* Bar\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li", 2);
            assert_xpath(&doc, "(//ul/li)[1]/p[text() = \"Foo\"]", 1);
            assert_xpath(&doc, "(//ul/li)[1]/*[@class=\"literalblock\"]", 1);

            assert_xpath(
                &doc,
                "(//ul/li)[1]/p/following-sibling::*[@class=\"literalblock\"]",
                1,
            );

            assert_xpath(
                &doc,
                "((//ul/li)[1]/*[@class=\"literalblock\"])[1]//pre[text() = \"literal\n. still literal\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//ul/li)[1]/*[@class=\"literalblock\"]/following-sibling::*[@class=\"paragraph\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//ul/li)[1]/*[@class=\"literalblock\"]/following-sibling::*[@class=\"paragraph\"]/p[text()=\"para\"]",
                1,
            );
        }

        #[test]
        fn consecutive_literal_paragraph_offset_by_blank_lines_in_list_content_are_appended_as_a_literal_blocks()
         {
            let doc = Parser::default()
                .parse("List\n====\n\n- Foo\n\n  literal\n\n  more\n  literal\n\n- Boo\n- Blech\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li", 3);
            assert_xpath(&doc, "(//ul/li)[1]/p[text() = \"Foo\"]", 1);
            assert_xpath(&doc, "(//ul/li)[1]/*[@class=\"literalblock\"]", 2);

            assert_xpath(
                &doc,
                "(//ul/li)[1]/p/following-sibling::*[@class=\"literalblock\"]",
                2,
            );

            assert_xpath(
                &doc,
                "((//ul/li)[1]/*[@class=\"literalblock\"])[1]//pre[text()=\"literal\"]",
                1,
            );

            assert_xpath(
                &doc,
                "((//ul/li)[1]/*[@class=\"literalblock\"])[2]//pre[text()=\"more\nliteral\"]",
                1,
            );
        }

        #[test]
        fn a_literal_paragraph_without_a_trailing_blank_line_consumes_following_list_items() {
            let doc = Parser::default().parse("List\n====\n\n- Foo\n\n  literal\n- Boo\n- Blech\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li", 1);
            assert_xpath(&doc, "(//ul/li)[1]/p[text() = \"Foo\"]", 1);
            assert_xpath(&doc, "(//ul/li)[1]/*[@class=\"literalblock\"]", 1);

            assert_xpath(
                &doc,
                "(//ul/li)[1]/p/following-sibling::*[@class=\"literalblock\"]",
                1,
            );

            assert_xpath(
                &doc,
                "((//ul/li)[1]/*[@class=\"literalblock\"])[1]//pre[text() = \"literal\\n- Boo\\n- Blech\"]",
                1,
            );
        }

        #[test]
        fn asterisk_elements_with_no_blank_lines() {
            let doc = Parser::default().parse("== List\n\n* Foo\n* Boo\n* Blech\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li", 3);
        }

        #[test]
        fn indented_asterisk_elements_using_spaces() {
            let doc = Parser::default().parse(" * Foo\n * Boo\n * Blech\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li", 3);
        }

        #[test]
        fn indented_unicode_bullet_elements_using_spaces() {
            let doc = Parser::default().parse(" • Foo\n • Boo\n • Blech\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li", 3);
        }

        #[test]
        fn indented_asterisk_elements_using_tabs() {
            let doc = Parser::default().parse("\t*\tFoo\n\t*\tBoo\n\t*\tBlech\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li", 3);
        }

        #[test]
        fn should_represent_block_style_as_style_class() {
            let doc = Parser::default().parse("[disc]\n* a\n* b\n* c\n");

            assert_css(&doc, ".ulist.disc", 1);
            assert_css(&doc, ".ulist.disc ul.disc", 1);
            // NOTE: Ruby test loops over %w(disc square circle), testing each.
        }

        #[test]
        fn asterisk_elements_separated_by_blank_lines_should_merge_lists() {
            let doc = Parser::default().parse("== List\n\n* Foo\n\n* Boo\n\n\n* Blech\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li", 3);
        }

        #[test]
        fn asterisk_elements_with_interspersed_line_comments_should_be_skipped_and_not_break_list()
        {
            let doc = Parser::default().parse("== List\n\n* Foo\n// line comment\n// another line comment\n* Boo\n// line comment\nmore text\n// another line comment\n* Blech\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li", 3);

            assert_xpath(&doc, "(//ul/li)[2]/p[text()=\"Boo\nmore text\"]", 1);
        }

        #[test]
        fn asterisk_elements_separated_by_a_line_comment_offset_by_blank_lines_should_not_merge_lists()
         {
            let doc = Parser::default().parse("== List\n\n* Foo\n* Boo\n\n//\n\n* Blech\n");

            assert_xpath(&doc, "//ul", 2);
            assert_xpath(&doc, "(//ul)[1]/li", 2);
            assert_xpath(&doc, "(//ul)[2]/li", 1);
        }

        #[test]
        fn asterisk_elements_separated_by_a_block_title_offset_by_a_blank_line_should_not_merge_lists()
         {
            let doc = Parser::default().parse("List\n====\n\n* Foo\n* Boo\n\n.Also\n* Blech\n");

            assert_xpath(&doc, "//ul", 2);
            assert_xpath(&doc, "(//ul)[1]/li", 2);
            assert_xpath(&doc, "(//ul)[2]/li", 1);

            assert_xpath(
                &doc,
                "(//ul)[2]/preceding-sibling::*[@class = \"title\"][text() = \"Also\"]",
                1,
            );
        }

        #[test]
        fn asterisk_elements_separated_by_an_attribute_entry_offset_by_a_blank_line_should_not_merge_lists()
         {
            let doc = Parser::default().parse("== List\n\n* Foo\n* Boo\n\n:foo: bar\n* Blech\n");

            assert_xpath(&doc, "//ul", 2);
            assert_xpath(&doc, "(//ul)[1]/li", 2);
            assert_xpath(&doc, "(//ul)[2]/li", 1);
        }

        #[test]
        fn list_should_terminate_before_next_lower_section_heading() {
            let doc =
                Parser::default().parse("== List\n\n* first\nitem\n* second\nitem\n\n== Section\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li", 2);
            assert_xpath(&doc, "//h2[text() = \"Section\"]", 1);
        }

        #[test]
        fn list_should_terminate_before_next_lower_section_heading_with_implicit_id() {
            let doc = Parser::default()
                .parse("== List\n\n* first\nitem\n* second\nitem\n\n[[sec]]\n== Section\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li", 2);
            assert_xpath(&doc, "//h2[@id = \"sec\"][text() = \"Section\"]", 1);
        }

        #[test]
        fn should_not_find_section_title_immediately_below_last_list_item() {
            let doc = Parser::default().parse("* first\n* second\n== Not a section\n");

            assert_css(&doc, "ul", 1);
            assert_css(&doc, "ul > li", 2);
            assert_css(&doc, "h2", 0);
            assert_output_contains(&doc, "== Not a section");
            assert_xpath(
                &doc,
                "(//li)[2]/p[text() = \"second\\n== Not a section\"]",
                1,
            );
        }

        #[test]
        fn should_match_trailing_line_separator_in_text_of_list_item() {
            let doc = Parser::default().parse("* a\n* b\u{2028}\n* c");

            assert_css(&doc, "li", 3);
            assert_xpath(&doc, "(//li)[2]/p[text()=\"b\u{2028}\"]", 1);
        }

        #[test]
        fn should_match_line_separator_in_text_of_list_item() {
            let doc = Parser::default().parse("* a\n* b\u{2028}b\n* c");

            assert_css(&doc, "li", 3);
            assert_xpath(&doc, "(//li)[2]/p[text()=\"b\u{2028}b\"]", 1);
        }
    }

    mod lists_with_inline_markup {
        use super::*;

        #[test]
        fn quoted_text() {
            let doc = Parser::default()
                .parse("List\n====\n\n- I am *strong*.\n- I am _stressed_.\n- I am `flexible`.\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li", 3);
            assert_xpath(&doc, "(//ul/li)[1]//strong", 1);
            assert_xpath(&doc, "(//ul/li)[2]//em", 1);
            assert_xpath(&doc, "(//ul/li)[3]//code", 1);
        }

        #[test]
        fn attribute_substitutions() {
            let doc = Parser::default()
                .parse("List\n====\n:foo: bar\n\n- side a {vbar} side b\n- Take me to a {foo}.\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li", 2);
            assert_xpath(&doc, "(//ul/li)[1]//p[text() = \"side a | side b\"]", 1);
            assert_xpath(&doc, "(//ul/li)[2]//p[text() = \"Take me to a bar.\"]", 1);
        }

        #[test]
        fn leading_dot_is_treated_as_text_not_block_title() {
            let doc = Parser::default().parse("* .first\n* .second\n* .third\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li", 3);
            assert_xpath(&doc, "(//ul/li)[1]//p[text() = \".first\"]", 1);
            assert_xpath(&doc, "(//ul/li)[2]//p[text() = \".second\"]", 1);
            assert_xpath(&doc, "(//ul/li)[3]//p[text() = \".third\"]", 1);
        }

        #[test]
        fn word_ending_sentence_on_continuing_line_not_treated_as_a_list_item() {
            let doc = Parser::default().parse(
                "A. This is the story about\n   AsciiDoc. It begins here.\nB. And it ends here.\n",
            );

            assert_xpath(&doc, "//ol", 1);
            assert_xpath(&doc, "//ol/li", 2);
        }

        #[test]
        #[ignore]
        // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/461):
        // Enable this test when xref replacement refactoring is complete.
        fn should_discover_anchor_at_start_of_unordered_list_item_text_and_register_it_as_a_reference()
         {
            let _doc = Parser::default().parse("The highest peak in the Front Range is <<grays-peak>>, which tops <<mount-evans>> by just a few feet.\n\n* [[mount-evans,Mount Evans]]At 14,271 feet, Mount Evans is the highest summit of the Chicago Peaks in the Front Range of the Rocky Mountains.\n* [[grays-peak,Grays Peak]]\nGrays Peak rises to 14,278 feet, making it the highest summit in the Front Range of the Rocky Mountains.\n* Longs Peak is a 14,259-foot high, prominent mountain summit in the northern Front Range of the Rocky Mountains.\n* Pikes Peak is the highest summit of the southern Front Range of the Rocky Mountains at 14,115 feet.\n");
            todo!("doc.catalog[:refs] check");
            todo!(
                "assert_xpath: '(//p)[1]/a[@href=\"#grays-peak\"][text()=\"Grays Peak\"]', output, 1"
            );
            todo!(
                "assert_xpath: '(//p)[1]/a[@href=\"#mount-evans\"][text()=\"Mount Evans\"]', output, 1"
            );
        }

        #[test]
        #[ignore]
        // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/461):
        // Enable this test when xref replacement refactoring is complete.
        fn should_discover_anchor_at_start_of_ordered_list_item_text_and_register_it_as_a_reference()
         {
            let _doc = Parser::default().parse("This is a cross-reference to <<step-2>>.\nThis is a cross-reference to <<step-4>>.\n\n. Ordered list, item 1, without anchor\n. [[step-2,Step 2]]Ordered list, item 2, with anchor\n. Ordered list, item 3, without anchor\n. [[step-4,Step 4]]Ordered list, item 4, with anchor\n");
            todo!("doc.catalog[:refs] check");
            todo!("assert_xpath: '(//p)[1]/a[@href=\"#step-2\"][text()=\"Step 2\"]', output, 1");
            todo!("assert_xpath: '(//p)[1]/a[@href=\"#step-4\"][text()=\"Step 4\"]', output, 1");
        }

        #[test]
        #[ignore]
        // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/461):
        // Enable this test when xref replacement refactoring is complete.
        // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/311):
        // Enable this test when callouts are implemented.
        fn should_discover_anchor_at_start_of_callout_list_item_text_and_register_it_as_a_reference()
         {
            let _doc = Parser::default().parse("This is a cross-reference to <<url-mapping>>.\n\n[source,ruby]\n----\nrequire 'sinatra' <1>\n\nget '/hi' do <2> <3>\n  \"Hello World!\"\nend\n----\n<1> Library import\n<2> [[url-mapping,url mapping]]URL mapping\n<3> Response block\n");
            todo!("doc.catalog[:refs] check");
            todo!(
                "assert_xpath: '(//p)[1]/a[@href=\"#url-mapping\"][text()=\"url mapping\"]', output, 1"
            );
        }
    }

    mod nested_lists {
        use super::*;
        use crate::blocks::IsBlock;

        #[test]
        fn asterisk_element_mixed_with_dash_elements_should_be_nested() {
            let doc = Parser::default().parse("== List\n\n- Foo\n* Boo\n- Blech\n");

            assert_xpath(&doc, "//ul", 2);
            assert_xpath(&doc, "//ul/li", 3);
            assert_xpath(&doc, "(//ul)[1]/li", 2);
            assert_xpath(&doc, "(//ul)[1]/li//ul/li", 1);
        }

        #[test]
        fn dash_element_mixed_with_asterisks_elements_should_be_nested() {
            let doc = Parser::default().parse("List\n====\n\n* Foo\n- Boo\n* Blech\n");

            assert_xpath(&doc, "//ul", 2);
            assert_xpath(&doc, "//ul/li", 3);
            assert_xpath(&doc, "(//ul)[1]/li", 2);
            assert_xpath(&doc, "(//ul)[1]/li//ul/li", 1);
        }

        #[test]
        fn lines_prefixed_with_alternating_list_markers_separated_by_blank_lines_should_be_nested()
        {
            let doc = Parser::default().parse("List\n====\n\n- Foo\n\n* Boo\n\n\n- Blech\n");

            assert_xpath(&doc, "//ul", 2);
            assert_xpath(&doc, "//ul/li", 3);
            assert_xpath(&doc, "(//ul)[1]/li", 2);
            assert_xpath(&doc, "(//ul)[1]/li//ul/li", 1);
        }

        #[test]
        fn nested_elements_2_with_asterisks() {
            let doc = Parser::default().parse("List\n====\n\n* Foo\n** Boo\n* Blech\n");

            assert_xpath(&doc, "//ul", 2);
            assert_xpath(&doc, "//ul/li", 3);
            assert_xpath(&doc, "(//ul)[1]/li", 2);
            assert_xpath(&doc, "(//ul)[1]/li//ul/li", 1);
        }

        #[test]
        fn nested_elements_3_with_asterisks() {
            let doc = Parser::default().parse("List\n====\n\n* Foo\n** Boo\n*** Snoo\n* Blech\n");

            assert_xpath(&doc, "//ul", 3);
            assert_xpath(&doc, "(//ul)[1]/li", 2);
            assert_xpath(&doc, "((//ul)[1]/li//ul)[1]/li", 1);
            assert_xpath(&doc, "(((//ul)[1]/li//ul)[1]/li//ul)[1]/li", 1);
        }

        #[test]
        fn nested_elements_4_with_asterisks() {
            let doc =
                Parser::default().parse("== List\n\n* Foo\n** Boo\n*** Snoo\n**** Froo\n* Blech\n");

            assert_xpath(&doc, "//ul", 4);
            assert_xpath(&doc, "(//ul)[1]/li", 2);
            assert_xpath(&doc, "((//ul)[1]/li//ul)[1]/li", 1);
            assert_xpath(&doc, "(((//ul)[1]/li//ul)[1]/li//ul)[1]/li", 1);
            assert_xpath(&doc, "((((//ul)[1]/li//ul)[1]/li//ul)[1]/li//ul)[1]/li", 1);
        }

        #[test]
        fn nested_elements_5_with_asterisks() {
            let doc = Parser::default()
                .parse("== List\n\n* Foo\n** Boo\n*** Snoo\n**** Froo\n***** Groo\n* Blech\n");

            assert_xpath(&doc, "//ul", 5);
            assert_xpath(&doc, "(//ul)[1]/li", 2);
            assert_xpath(&doc, "((//ul)[1]/li//ul)[1]/li", 1);
            assert_xpath(&doc, "(((//ul)[1]/li//ul)[1]/li//ul)[1]/li", 1);
            assert_xpath(&doc, "((((//ul)[1]/li//ul)[1]/li//ul)[1]/li//ul)[1]/li", 1);

            assert_xpath(
                &doc,
                "(((((//ul)[1]/li//ul)[1]/li//ul)[1]/li//ul)[1]/li//ul)[1]/li",
                1,
            );
        }

        #[test]
        fn nested_arbitrary_depth_with_asterisks() {
            let doc = Parser::default().parse("* a\n** b\n*** c\n**** d\n***** e\n****** f\n******* g\n******** h\n********* i\n********** j\n*********** k\n************ l\n************* m\n************** n\n*************** o\n**************** p\n***************** q\n****************** r\n******************* s\n******************** t\n********************* u\n********************** v\n*********************** w\n************************ x\n************************* y\n************************** z\n");

            refute_output_contains(&doc, "*");
            assert_css(&doc, "li", 26);
        }

        // NOTE: Skipped test named "level of unordered list should match section level"
        // because we don't store level for parsed items except in the section data
        // structure.

        #[test]
        fn does_not_recognize_lists_with_repeating_unicode_bullets() {
            let doc = Parser::default().parse("•• Boo");

            assert_xpath(&doc, "//ul", 0);
            assert_output_contains(&doc, "•");
        }

        #[test]
        fn nested_ordered_elements_2() {
            let doc = Parser::default().parse("List\n====\n\n. Foo\n.. Boo\n. Blech\n");

            assert_xpath(&doc, "//ol", 2);
            assert_xpath(&doc, "//ol/li", 3);
            assert_xpath(&doc, "(//ol)[1]/li", 2);
            assert_xpath(&doc, "(//ol)[1]/li//ol/li", 1);
        }

        #[test]
        fn nested_ordered_elements_3() {
            let doc = Parser::default().parse("List\n====\n\n. Foo\n.. Boo\n... Snoo\n. Blech\n");

            assert_xpath(&doc, "//ol", 3);
            assert_xpath(&doc, "(//ol)[1]/li", 2);
            assert_xpath(&doc, "((//ol)[1]/li//ol)[1]/li", 1);
            assert_xpath(&doc, "(((//ol)[1]/li//ol)[1]/li//ol)[1]/li", 1);
        }

        #[test]
        fn nested_arbitrary_depth_with_dot_marker() {
            let doc = Parser::default().parse(". a\n.. b\n... c\n.... d\n..... e\n...... f\n....... g\n........ h\n......... i\n.......... j\n........... k\n............ l\n............. m\n.............. n\n............... o\n................ p\n................. q\n.................. r\n................... s\n.................... t\n..................... u\n...................... v\n....................... w\n........................ x\n......................... y\n.......................... z\n");

            refute_output_contains(&doc, ".");
            assert_css(&doc, "li", 26);
        }

        // NOTE: Skipped test named "level of ordered list should match section level"
        // because we don't store level for parsed items except in the section data
        // structure.

        #[test]
        fn nested_unordered_inside_ordered_elements() {
            let doc = Parser::default().parse("List\n====\n\n. Foo\n* Boo\n. Blech\n");

            assert_xpath(&doc, "//ol", 1);
            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "(//ol)[1]/li", 2);
            assert_xpath(&doc, "((//ol)[1]/li//ul)[1]/li", 1);
        }

        #[test]
        fn nested_ordered_inside_unordered_elements() {
            let doc = Parser::default().parse("List\n====\n\n* Foo\n. Boo\n* Blech\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ol", 1);
            assert_xpath(&doc, "(//ul)[1]/li", 2);
            assert_xpath(&doc, "((//ul)[1]/li//ol)[1]/li", 1);
        }

        #[test]
        fn three_levels_of_alternating_unordered_and_ordered_elements() {
            let doc = Parser::default()
                .parse("== Lists\n\n* bullet 1\n. numbered 1.1\n** bullet 1.1.1\n* bullet 2\n");

            assert_css(&doc, ".ulist", 2);
            assert_css(&doc, ".olist", 1);
            assert_css(&doc, ".ulist > ul > li > p", 3);
            assert_css(&doc, ".ulist > ul > li > p + .olist", 1);

            assert_css(&doc, ".ulist > ul > li > p + .olist > ol > li > p", 1);

            assert_css(
                &doc,
                ".ulist > ul > li > p + .olist > ol > li > p + .ulist",
                1,
            );

            assert_css(
                &doc,
                ".ulist > ul > li > p + .olist > ol > li > p + .ulist > ul > li > p",
                1,
            );

            assert_css(&doc, ".ulist > ul > li + li > p", 1);
        }

        #[test]
        fn lines_with_alternating_markers_of_unordered_and_ordered_list_types_separated_by_blank_lines_should_be_nested()
         {
            let doc = Parser::default().parse("== List\n\n* Foo\n\n. Boo\n\n\n* Blech\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ol", 1);
            assert_xpath(&doc, "(//ul)[1]/li", 2);
            assert_xpath(&doc, "((//ul)[1]/li//ol)[1]/li", 1);
        }

        #[test]
        fn list_item_with_literal_content_should_not_consume_nested_list_of_different_type() {
            let doc = Parser::default()
                .parse("== List\n\n- bullet\n\n  literal\n  but not\n  hungry\n\n. numbered\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//li", 2);
            assert_xpath(&doc, "//ul//ol", 1);
            assert_xpath(&doc, "//ul/li/p", 1);
            assert_xpath(&doc, "//ul/li/p[text()=\"bullet\"]", 1);

            assert_xpath(
                &doc,
                "//ul/li/p/following-sibling::*[@class=\"literalblock\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//ul/li/p/following-sibling::*[@class=\"literalblock\"]//pre[text()=\"literal\\nbut not\\nhungry\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"literalblock\"]/following-sibling::*[@class=\"olist arabic\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"literalblock\"]/following-sibling::*[@class=\"olist arabic\"]//p[text()=\"numbered\"]",
                1,
            );
        }

        #[test]
        fn nested_list_item_does_not_eat_the_title_of_the_following_detached_block() {
            let doc = Parser::default().parse("List\n====\n\n- bullet\n  * nested bullet 1\n  * nested bullet 2\n\n.Title\n....\nliteral\n....\n");

            assert_xpath(&doc, "//*[@class=\"ulist\"]/ul", 2);

            assert_xpath(
                &doc,
                "(//*[@class=\"ulist\"])[1]/following-sibling::*[@class=\"literalblock\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//*[@class=\"ulist\"])[1]/following-sibling::*[@class=\"literalblock\"]/*[@class=\"title\"]",
                1,
            );
        }

        #[test]
        fn lines_with_alternating_markers_of_bulleted_and_description_list_types_separated_by_blank_lines_should_be_nested()
         {
            let doc = Parser::default().parse("== List\n\n* Foo\n\nterm1:: def1\n\n* Blech\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//dl", 1);
            assert_xpath(&doc, "//ul[1]/li", 2);
            assert_xpath(&doc, "//ul[1]/li//dl[1]/dt", 1);
            assert_xpath(&doc, "//ul[1]/li//dl[1]/dd", 1);
        }

        #[test]
        fn nested_ordered_with_attribute_inside_unordered_elements() {
            let doc = Parser::default().parse("Blah\n====\n\n* Foo\n[start=2]\n. Boo\n* Blech\n");
            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ol", 1);
            assert_xpath(&doc, "(//ul)[1]/li", 2);
            assert_xpath(&doc, "((//ul)[1]/li//ol)[1][@start = 2]/li", 1);
        }
    }

    mod list_continuations {
        use super::*;

        #[test]
        fn adjacent_list_continuation_line_attaches_following_paragraph() {
            let doc = Parser::default().parse("== Lists\n\n* Item one, paragraph one\n+\nItem one, paragraph two\n+\n* Item two\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li", 2);
            assert_xpath(&doc, "//ul/li[1]/p", 1);
            assert_xpath(&doc, "//ul/li[1]//p", 2);

            assert_xpath(
                &doc,
                "//ul/li[1]/p[text() = \"Item one, paragraph one\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//ul/li[1]/*[@class = \"paragraph\"]/p[text() = \"Item one, paragraph two\"]",
                1,
            );
        }

        #[test]
        fn adjacent_list_continuation_line_attaches_following_block() {
            let doc = Parser::default().parse("== Lists\n\n* Item one, paragraph one\n+\n....\nItem one, literal block\n....\n+\n* Item two\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li", 2);
            assert_xpath(&doc, "//ul/li[1]/p", 1);

            assert_xpath(
                &doc,
                "(//ul/li[1]/p/following-sibling::*)[1][@class = \"literalblock\"]",
                1,
            );
        }

        #[test]
        fn adjacent_list_continuation_line_attaches_following_block_with_block_attributes() {
            let doc = Parser::default().parse("== Lists\n\n* Item one, paragraph one\n+\n:foo: bar\n[[beck]]\n.Read the following aloud to yourself\n[source, ruby]\n----\n5.times { print \"Odelay!\" }\n----\n\n* Item two\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li", 2);
            assert_xpath(&doc, "//ul/li[1]/p", 1);

            assert_xpath(
                &doc,
                "(//ul/li[1]/p/following-sibling::*)[1][@id=\"beck\"][@class = \"listingblock\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//ul/li[1]/p/following-sibling::*)[1][@id=\"beck\"]/div[@class=\"title\"][starts-with(text(),\"Read\")]",
                1,
            );

            assert_xpath(
                &doc,
                "(//ul/li[1]/p/following-sibling::*)[1][@id=\"beck\"]//code[@data-lang=\"ruby\"][starts-with(text(),\"5.times\")]",
                1,
            );
        }

        #[test]
        fn trailing_block_attribute_line_attached_by_continuation_should_not_create_block() {
            let doc = Parser::default()
                .parse("== Lists\n\n* Item one, paragraph one\n+\n[source]\n\n* Item two\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li", 2);
            assert_xpath(&doc, "//ul/li[1]/*", 1);
            assert_xpath(&doc, "//ul/li//*[@class=\"listingblock\"]", 0);
        }

        #[test]
        fn trailing_block_title_line_attached_by_continuation_should_not_create_block() {
            let doc = Parser::default().parse("== Lists\n\n* Item one, paragraph one\n+\n.Disappears into the ether\n\n* Item two\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li", 2);
            assert_xpath(&doc, "//ul/li[1]/*", 1);
        }

        #[test]
        fn consecutive_blocks_in_list_continuation_attach_to_list_item() {
            let doc = Parser::default().parse("== Lists\n\n* Item one, paragraph one\n+\n....\nItem one, literal block\n....\n+\n____\nItem one, quote block\n____\n+\n* Item two\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li", 2);
            assert_xpath(&doc, "//ul/li[1]/p", 1);

            assert_xpath(
                &doc,
                "(//ul/li[1]/p/following-sibling::*)[1][@class = \"literalblock\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//ul/li[1]/p/following-sibling::*)[2][@class = \"quoteblock\"]",
                1,
            );
        }

        #[test]
        fn list_item_with_hanging_indent_followed_by_block_attached_by_list_continuation() {
            let doc = Parser::default().parse("== Lists\n\n. list item 1\n  continued\n+\n--\nopen block in list item 1\n--\n\n. list item 2\n");

            assert_css(&doc, "ol", 1);
            assert_css(&doc, "ol li", 2);

            assert_xpath(
                &doc,
                "(//ol/li)[1]/p[text()=\"list item 1\\ncontinued\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//ol/li)[1]/p/following-sibling::*[@class=\"openblock\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//ol/li)[1]/p/following-sibling::*[@class=\"openblock\"]//p[text()=\"open block in list item 1\"]",
                1,
            );

            assert_xpath(&doc, "(//ol/li)[2]/p[text()=\"list item 2\"]", 1);
        }

        #[test]
        fn list_item_paragraph_in_list_item_and_nested_list_item() {
            let doc = Parser::default().parse("== Lists\n\n. list item 1\n+\nlist item 1 paragraph\n\n* nested list item\n+\nnested list item paragraph\n\n. list item 2\n");

            assert_css(&doc, ".olist ol", 1);
            assert_css(&doc, ".olist ol > li", 2);
            assert_css(&doc, ".ulist ul", 1);
            assert_css(&doc, ".ulist ul > li", 1);

            assert_xpath(&doc, "(//ol/li)[1]/*", 3);
            assert_xpath(&doc, "((//ol/li)[1]/*)[1]/self::p", 1);

            assert_xpath(
                &doc,
                "((//ol/li)[1]/*)[1]/self::p[text()=\"list item 1\"]",
                1,
            );

            assert_xpath(
                &doc,
                "((//ol/li)[1]/*)[2]/self::div[@class=\"paragraph\"]",
                1,
            );

            assert_xpath(&doc, "((//ol/li)[1]/*)[3]/self::div[@class=\"ulist\"]", 1);

            assert_xpath(
                &doc,
                "((//ol/li)[1]/*)[3]/self::div[@class=\"ulist\"]/ul/li",
                1,
            );

            assert_xpath(
                &doc,
                "((//ol/li)[1]/*)[3]/self::div[@class=\"ulist\"]/ul/li/p[text()=\"nested list item\"]",
                1,
            );

            assert_xpath(
                &doc,
                "((//ol/li)[1]/*)[3]/self::div[@class=\"ulist\"]/ul/li/p/following-sibling::div[@class=\"paragraph\"]",
                1,
            );
        }

        #[test]
        fn trailing_list_continuations_should_attach_to_list_items_at_respective_levels() {
            let doc = Parser::default().parse("== Lists\n\n. list item 1\n+\n* nested list item 1\n* nested list item 2\n+\nparagraph for nested list item 2\n\n+\nparagraph for list item 1\n\n. list item 2\n");

            assert_css(&doc, ".olist ol", 1);
            assert_css(&doc, ".olist ol > li", 2);
            assert_css(&doc, ".ulist ul", 1);
            assert_css(&doc, ".ulist ul > li", 2);
            assert_css(&doc, ".olist .ulist", 1);

            assert_xpath(&doc, "(//ol/li)[1]/*", 3);
            assert_xpath(&doc, "((//ol/li)[1]/*)[1]/self::p", 1);

            assert_xpath(
                &doc,
                "((//ol/li)[1]/*)[1]/self::p[text()=\"list item 1\"]",
                1,
            );

            assert_xpath(&doc, "((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]", 1);

            assert_xpath(
                &doc,
                "((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]/ul/li",
                2,
            );

            assert_xpath(
                &doc,
                "(((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]/ul/li)[2]/*",
                2,
            );

            assert_xpath(
                &doc,
                "(((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]/ul/li)[2]/p",
                1,
            );

            assert_xpath(
                &doc,
                "(((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]/ul/li)[2]/div[@class=\"paragraph\"]",
                1,
            );

            assert_xpath(
                &doc,
                "((//ol/li)[1]/*)[3]/self::div[@class=\"paragraph\"]",
                1,
            );
        }

        #[test]
        fn trailing_list_continuations_should_attach_to_list_items_of_different_types_at_respective_levels()
         {
            let doc = Parser::default().parse("== Lists\n\n* bullet 1\n. numbered 1.1\n** bullet 1.1.1\n\n+\nnumbered 1.1 paragraph\n\n+\nbullet 1 paragraph\n\n* bullet 2\n");

            assert_xpath(&doc, "(//ul)[1]/li", 2);

            assert_xpath(&doc, "((//ul)[1]/li[1])/*", 3);

            assert_xpath(
                &doc,
                "(((//ul)[1]/li[1])/*)[1]/self::p[text()=\"bullet 1\"]",
                1,
            );

            assert_xpath(&doc, "(((//ul)[1]/li[1])/*)[2]/ol", 1);

            assert_xpath(
                &doc,
                "(((//ul)[1]/li[1])/*)[3]/self::div[@class=\"paragraph\"]/p[text()=\"bullet 1 paragraph\"]",
                1,
            );

            assert_xpath(&doc, "((//ul)[1]/li)[1]/div/ol/li", 1);
            assert_xpath(&doc, "((//ul)[1]/li)[1]/div/ol/li/*", 3);

            assert_xpath(
                &doc,
                "(((//ul)[1]/li)[1]/div/ol/li/*)[1]/self::p[text()=\"numbered 1.1\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(((//ul)[1]/li)[1]/div/ol/li/*)[2]/self::div[@class=\"ulist\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(((//ul)[1]/li)[1]/div/ol/li/*)[3]/self::div[@class=\"paragraph\"]/p[text()=\"numbered 1.1 paragraph\"]",
                1,
            );

            assert_xpath(
                &doc,
                "((//ul)[1]/li)[1]/div/ol/li/div[@class=\"ulist\"]/ul/li",
                1,
            );

            assert_xpath(
                &doc,
                "((//ul)[1]/li)[1]/div/ol/li/div[@class=\"ulist\"]/ul/li/*",
                1,
            );

            assert_xpath(
                &doc,
                "((//ul)[1]/li)[1]/div/ol/li/div[@class=\"ulist\"]/ul/li/p[text()=\"bullet 1.1.1\"]",
                1,
            );
        }

        #[test]
        fn repeated_list_continuations_should_attach_to_list_items_at_respective_levels() {
            let doc = Parser::default().parse("== Lists\n\n. list item 1\n\n* nested list item 1\n+\n--\nopen block for nested list item 1\n--\n+\n* nested list item 2\n+\nparagraph for nested list item 2\n\n+\nparagraph for list item 1\n\n. list item 2\n");

            assert_css(&doc, ".olist ol", 1);
            assert_css(&doc, ".olist ol > li", 2);
            assert_css(&doc, ".ulist ul", 1);
            assert_css(&doc, ".ulist ul > li", 2);
            assert_css(&doc, ".olist .ulist", 1);

            assert_xpath(&doc, "(//ol/li)[1]/*", 3);
            assert_xpath(&doc, "((//ol/li)[1]/*)[1]/self::p", 1);

            assert_xpath(
                &doc,
                "((//ol/li)[1]/*)[1]/self::p[text()=\"list item 1\"]",
                1,
            );

            assert_xpath(&doc, "((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]", 1);

            assert_xpath(
                &doc,
                "((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]/ul/li",
                2,
            );

            assert_xpath(
                &doc,
                "(((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]/ul/li)[1]/*",
                2,
            );

            assert_xpath(
                &doc,
                "(((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]/ul/li)[1]/p",
                1,
            );

            assert_xpath(
                &doc,
                "(((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]/ul/li)[1]/div[@class=\"openblock\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]/ul/li)[2]/*",
                2,
            );

            assert_xpath(
                &doc,
                "(((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]/ul/li)[2]/p",
                1,
            );

            assert_xpath(
                &doc,
                "(((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]/ul/li)[2]/div[@class=\"paragraph\"]",
                1,
            );

            assert_xpath(
                &doc,
                "((//ol/li)[1]/*)[3]/self::div[@class=\"paragraph\"]",
                1,
            );
        }

        #[test]
        fn repeated_list_continuations_attached_directly_to_list_item_should_attach_to_list_items_at_respective_levels()
         {
            let doc = Parser::default().parse("== Lists\n\n. list item 1\n+\n* nested list item 1\n+\n--\nopen block for nested list item 1\n--\n+\n* nested list item 2\n+\nparagraph for nested list item 2\n\n+\nparagraph for list item 1\n\n. list item 2\n");

            assert_css(&doc, ".olist ol", 1);
            assert_css(&doc, ".olist ol > li", 2);
            assert_css(&doc, ".ulist ul", 1);
            assert_css(&doc, ".ulist ul > li", 2);
            assert_css(&doc, ".olist .ulist", 1);
            assert_xpath(&doc, "(//ol/li)[1]/*", 3);
            assert_xpath(&doc, "((//ol/li)[1]/*)[1]/self::p", 1);
            assert_xpath(
                &doc,
                "((//ol/li)[1]/*)[1]/self::p[text()=\"list item 1\"]",
                1,
            );
            assert_xpath(&doc, "((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]", 1);
            assert_xpath(
                &doc,
                "((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]/ul/li",
                2,
            );
            assert_xpath(
                &doc,
                "(((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]/ul/li)[1]/*",
                2,
            );
            assert_xpath(
                &doc,
                "(((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]/ul/li)[1]/p",
                1,
            );
            assert_xpath(
                &doc,
                "(((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]/ul/li)[1]/div[@class=\"openblock\"]",
                1,
            );
            assert_xpath(
                &doc,
                "(((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]/ul/li)[2]/*",
                2,
            );
            assert_xpath(
                &doc,
                "(((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]/ul/li)[2]/p",
                1,
            );
            assert_xpath(
                &doc,
                "(((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]/ul/li)[2]/div[@class=\"paragraph\"]",
                1,
            );
            assert_xpath(
                &doc,
                "((//ol/li)[1]/*)[3]/self::div[@class=\"paragraph\"]",
                1,
            );
        }

        #[test]
        fn repeated_list_continuations_should_attach_to_list_items_at_respective_levels_ignoring_blank_lines()
         {
            let doc = Parser::default().parse("== Lists\n\n. list item 1\n+\n* nested list item 1\n+\n--\nopen block for nested list item 1\n--\n+\n* nested list item 2\n+\nparagraph for nested list item 2\n\n\n+\nparagraph for list item 1\n\n. list item 2\n");

            assert_css(&doc, ".olist ol", 1);
            assert_css(&doc, ".olist ol > li", 2);
            assert_css(&doc, ".ulist ul", 1);
            assert_css(&doc, ".ulist ul > li", 2);
            assert_css(&doc, ".olist .ulist", 1);

            assert_xpath(&doc, "(//ol/li)[1]/*", 3);
            assert_xpath(&doc, "((//ol/li)[1]/*)[1]/self::p", 1);

            assert_xpath(
                &doc,
                "((//ol/li)[1]/*)[1]/self::p[text()=\"list item 1\"]",
                1,
            );

            assert_xpath(&doc, "((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]", 1);

            assert_xpath(
                &doc,
                "((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]/ul/li",
                2,
            );

            assert_xpath(
                &doc,
                "(((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]/ul/li)[1]/*",
                2,
            );

            assert_xpath(
                &doc,
                "(((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]/ul/li)[1]/p",
                1,
            );

            assert_xpath(
                &doc,
                "(((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]/ul/li)[1]/div[@class=\"openblock\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]/ul/li)[2]/*",
                2,
            );

            assert_xpath(
                &doc,
                "(((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]/ul/li)[2]/p",
                1,
            );

            assert_xpath(
                &doc,
                "(((//ol/li)[1]/*)[2]/self::div[@class=\"ulist\"]/ul/li)[2]/div[@class=\"paragraph\"]",
                1,
            );

            assert_xpath(
                &doc,
                "((//ol/li)[1]/*)[3]/self::div[@class=\"paragraph\"]",
                1,
            );
        }

        #[test]
        fn trailing_list_continuations_should_ignore_preceding_blank_lines() {
            let doc = Parser::default().parse("== Lists\n\n* bullet 1\n** bullet 1.1\n*** bullet 1.1.1\n+\n--\nopen block\n--\n\n\n+\nbullet 1.1 paragraph\n\n\n+\nbullet 1 paragraph\n\n* bullet 2\n");

            assert_xpath(&doc, "((//ul)[1]/li[1])/*", 3);
            assert_xpath(
                &doc,
                "(((//ul)[1]/li[1])/*)[1]/self::p[text()=\"bullet 1\"]",
                1,
            );
            assert_xpath(
                &doc,
                "(((//ul)[1]/li[1])/*)[2]/self::div[@class=\"ulist\"]",
                1,
            );
            assert_xpath(
                &doc,
                "(((//ul)[1]/li[1])/*)[3]/self::div[@class=\"paragraph\"]/p[text()=\"bullet 1 paragraph\"]",
                1,
            );

            assert_xpath(&doc, "((//ul)[1]/li)[1]/div[@class=\"ulist\"]/ul/li", 1);
            assert_xpath(&doc, "((//ul)[1]/li)[1]/div[@class=\"ulist\"]/ul/li/*", 3);
            assert_xpath(
                &doc,
                "(((//ul)[1]/li)[1]/div[@class=\"ulist\"]/ul/li/*)[1]/self::p[text()=\"bullet 1.1\"]",
                1,
            );
            assert_xpath(
                &doc,
                "(((//ul)[1]/li)[1]/div[@class=\"ulist\"]/ul/li/*)[2]/self::div[@class=\"ulist\"]",
                1,
            );
            assert_xpath(
                &doc,
                "(((//ul)[1]/li)[1]/div[@class=\"ulist\"]/ul/li/*)[3]/self::div[@class=\"paragraph\"]/p[text()=\"bullet 1.1 paragraph\"]",
                1,
            );

            assert_xpath(
                &doc,
                "((//ul)[1]/li)[1]/div[@class=\"ulist\"]/ul/li/div[@class=\"ulist\"]/ul/li",
                1,
            );
            assert_xpath(
                &doc,
                "((//ul)[1]/li)[1]/div[@class=\"ulist\"]/ul/li/div[@class=\"ulist\"]/ul/li/*",
                2,
            );
            assert_xpath(
                &doc,
                "(((//ul)[1]/li)[1]/div[@class=\"ulist\"]/ul/li/div[@class=\"ulist\"]/ul/li/*)[1]/self::p",
                1,
            );
            assert_xpath(
                &doc,
                "(((//ul)[1]/li)[1]/div[@class=\"ulist\"]/ul/li/div[@class=\"ulist\"]/ul/li/*)[2]/self::div[@class=\"openblock\"]",
                1,
            );
        }

        #[test]
        fn indented_outline_list_item_with_different_marker_offset_by_a_blank_line_should_be_recognized_as_a_nested_list()
         {
            let doc = Parser::default().parse("* item 1\n\n  . item 1.1\n+\nattached paragraph\n\n  . item 1.2\n+\nattached paragraph\n\n* item 2\n");

            assert_css(&doc, "ul", 1);
            assert_css(&doc, "ol", 1);
            assert_css(&doc, "ul ol", 1);
            assert_css(&doc, "ul > li", 2);

            assert_xpath(&doc, "((//ul/li)[1]/*)", 2);
            assert_xpath(&doc, "((//ul/li)[1]/*)[1]/self::p", 1);
            assert_xpath(&doc, "((//ul/li)[1]/*)[2]/self::div/ol", 1);
            assert_xpath(&doc, "((//ul/li)[1]/*)[2]/self::div/ol/li", 2);

            assert_xpath(&doc, "(((//ul/li)[1]/*)[2]/self::div/ol/li)[1]/*", 2);

            assert_xpath(
                &doc,
                "((((//ul/li)[1]/*)[2]/self::div/ol/li)[1]/*)[1]/self::p",
                1,
            );
            assert_xpath(
                &doc,
                "((((//ul/li)[1]/*)[2]/self::div/ol/li)[1]/*)[2]/self::div[@class=\"paragraph\"]",
                1,
            );

            assert_xpath(&doc, "(((//ul/li)[1]/*)[2]/self::div/ol/li)[2]/*", 2);

            assert_xpath(
                &doc,
                "((((//ul/li)[1]/*)[2]/self::div/ol/li)[2]/*)[1]/self::p",
                1,
            );
            assert_xpath(
                &doc,
                "((((//ul/li)[1]/*)[2]/self::div/ol/li)[2]/*)[2]/self::div[@class=\"paragraph\"]",
                1,
            );
        }

        #[test]
        fn indented_description_list_item_inside_outline_list_item_offset_by_a_blank_line_should_be_recognized_as_a_nested_list()
         {
            let doc = Parser::default().parse("* item 1\n\n  term a:: description a\n+\nattached paragraph\n\n  term b:: description b\n+\nattached paragraph\n\n* item 2\n");

            assert_css(&doc, "ul", 1);
            assert_css(&doc, "dl", 1);
            assert_css(&doc, "ul dl", 1);
            assert_css(&doc, "ul > li", 2);

            assert_xpath(&doc, "((//ul/li)[1]/*)", 2);
            assert_xpath(&doc, "((//ul/li)[1]/*)[1]/self::p", 1);
            assert_xpath(&doc, "((//ul/li)[1]/*)[2]/self::div/dl", 1);
            assert_xpath(&doc, "((//ul/li)[1]/*)[2]/self::div/dl/dt", 2);
            assert_xpath(&doc, "((//ul/li)[1]/*)[2]/self::div/dl/dd", 2);

            assert_xpath(&doc, "(((//ul/li)[1]/*)[2]/self::div/dl/dd)[1]/*", 2);
            assert_xpath(
                &doc,
                "((((//ul/li)[1]/*)[2]/self::div/dl/dd)[1]/*)[1]/self::p",
                1,
            );
            assert_xpath(
                &doc,
                "((((//ul/li)[1]/*)[2]/self::div/dl/dd)[1]/*)[2]/self::div[@class=\"paragraph\"]",
                1,
            );

            assert_xpath(&doc, "(((//ul/li)[1]/*)[2]/self::div/dl/dd)[2]/*", 2);
            assert_xpath(
                &doc,
                "((((//ul/li)[1]/*)[2]/self::div/dl/dd)[2]/*)[1]/self::p",
                1,
            );
            assert_xpath(
                &doc,
                "((((//ul/li)[1]/*)[2]/self::div/dl/dd)[2]/*)[2]/self::div[@class=\"paragraph\"]",
                1,
            );
        }

        #[test]
        fn consecutive_list_continuation_lines_are_folded() {
            // The Ruby asciidoctor implementation on which this is based comes with the
            // following comment:

            // NOTE: This is not consistent w/ AsciiDoc.py, but this is some screwy input
            // anyway. FIXME: one list continuation is left behind.

            let doc = Parser::default().parse("== Lists\n\n* Item one, paragraph one\n+\n+\nItem one, paragraph two\n+\n+\n* Item two\n+\n+\n");

            assert_xpath(&doc, "//ul", 1);
            assert_xpath(&doc, "//ul/li", 2);
            assert_xpath(&doc, "//ul/li[1]/p", 1);
            assert_xpath(&doc, "//ul/li[1]/div/p", 1);

            assert_xpath(
                &doc,
                "//ul/li[1]//p[text() = \"Item one, paragraph one\"]",
                1,
            );

            // NOTE: This is a negative assertion.
            assert_xpath(
                &doc,
                "//ul/li[1]//p[text() = \"+\nItem one, paragraph two\"]",
                1,
            );
        }

        #[test]
        fn should_warn_if_unterminated_block_is_detected_in_list_item() {
            let doc = Parser::default().parse("* item\n+\n====\nexample\n* swallowed item\n");

            assert_xpath(&doc, "//ul/li", 1);
            assert_xpath(&doc, "//ul/li/*[@class=\"exampleblock\"]", 1);
            assert_xpath(&doc, "//p[text()=\"example\n* swallowed item\"]", 1);

            let mut warnings = doc.warnings();
            assert!(warnings.next().is_some());
        }
    }
}

mod ordered_lists {
    use crate::{Parser, tests::prelude::*};

    mod simple_lists {
        use super::*;

        #[test]
        fn dot_elements_with_no_blank_lines() {
            let doc = Parser::default().parse("List\n====\n\n. Foo\n. Boo\n. Blech\n");

            assert_xpath(&doc, "//ol", 1);
            assert_css(&doc, "ol[start]", 0);
            assert_xpath(&doc, "//ol/li", 3);
        }

        #[test]
        fn indented_dot_elements_using_spaces() {
            let doc = Parser::default().parse(" . Foo\n . Boo\n . Blech\n");

            assert_xpath(&doc, "//ol", 1);
            assert_xpath(&doc, "//ol/li", 3);
        }

        #[test]
        fn indented_dot_elements_using_tabs() {
            let doc = Parser::default().parse("\t.\tFoo\n\t.\tBoo\n\t.\tBlech\n");

            assert_xpath(&doc, "//ol", 1);
            assert_xpath(&doc, "//ol/li", 3);
        }

        #[test]
        fn should_represent_explicit_role_attribute_as_style_class() {
            let doc = Parser::default().parse("[role=\"dry\"]\n. Once\n. Again\n. Refactor!\n");

            assert_css(&doc, ".olist.arabic.dry", 1);
            assert_css(&doc, ".olist ol.arabic", 1);
        }

        #[test]
        fn should_base_list_style_on_marker_length_rather_than_list_depth() {
            let doc = Parser::default().parse("... parent\n.. child\n. grandchild\n");

            assert_css(&doc, ".olist.lowerroman", 1);
            assert_css(&doc, ".olist.lowerroman .olist.loweralpha", 1);
            assert_css(&doc, ".olist.lowerroman .olist.loweralpha .olist.arabic", 1);
        }

        #[test]
        fn should_allow_list_style_to_be_specified_explicitly_when_using_markers_with_implicit_style()
         {
            let doc = Parser::default().parse("[loweralpha]\ni) 1\nii) 2\niii) 3\n");

            assert_css(&doc, ".olist.loweralpha", 1);
            assert_css(&doc, ".olist.lowerroman", 0);
        }

        #[test]
        fn should_represent_custom_numbering_and_explicit_role_attribute_as_style_classes() {
            let doc = Parser::default()
                .parse("[loweralpha, role=\"dry\"]\n. Once\n. Again\n. Refactor!\n");

            assert_css(&doc, ".olist.loweralpha.dry", 1);
            assert_css(&doc, ".olist ol.loweralpha", 1);
        }

        #[test]
        fn should_set_reversed_attribute_on_list_if_reversed_option_is_set() {
            let doc = Parser::default()
                .parse("[%reversed, start=3]\n. three\n. two\n. one\n. blast off!\n");

            assert_css(&doc, "ol[reversed][start=\"3\"]", 1);
        }

        #[test]
        fn should_represent_implicit_role_attribute_as_style_class() {
            let doc = Parser::default().parse("[.dry]\n. Once\n. Again\n. Refactor!\n");

            assert_css(&doc, ".olist.arabic.dry", 1);
            assert_css(&doc, ".olist ol.arabic", 1);
        }

        #[test]
        fn should_represent_custom_numbering_and_implicit_role_attribute_as_style_classes() {
            let doc = Parser::default().parse("[loweralpha.dry]\n. Once\n. Again\n. Refactor!\n");

            assert_css(&doc, ".olist.loweralpha.dry", 1);
            assert_css(&doc, ".olist ol.loweralpha", 1);
        }

        #[test]
        fn dot_elements_separated_by_blank_lines_should_merge_lists() {
            let doc = Parser::default().parse("List\n====\n\n. Foo\n\n. Boo\n\n\n. Blech\n");

            assert_xpath(&doc, "//ol", 1);
            assert_xpath(&doc, "//ol/li", 3);
        }

        #[test]
        fn should_escape_special_characters_in_all_literal_paragraphs_attached_to_list_item() {
            let doc = Parser::default().parse(". first item\n\n  <code>text</code>\n\n  more <code>text</code>\n\n. second item\n");

            assert_css(&doc, "li", 2);
            assert_css(&doc, "code", 0);
            assert_css(&doc, "li:first-of-type > *", 3);
            assert_css(&doc, "li:first-of-type pre", 2);

            assert_xpath(&doc, "((//li)[1]//pre)[1][text()=\"<code>text</code>\"]", 1);

            assert_xpath(
                &doc,
                "((//li)[1]//pre)[2][text()=\"more <code>text</code>\"]",
                1,
            );
        }

        #[test]
        fn dot_elements_with_interspersed_line_comments_should_be_skipped_and_not_break_list() {
            let doc = Parser::default().parse("== List\n\n. Foo\n// line comment\n// another line comment\n. Boo\n// line comment\nmore text\n// another line comment\n. Blech\n");

            assert_xpath(&doc, "//ol", 1);
            assert_xpath(&doc, "//ol/li", 3);
            assert_xpath(&doc, "(//ol/li)[2]/p[text()=\"Boo\nmore text\"]", 1);
        }

        #[test]
        fn dot_elements_separated_by_line_comment_offset_by_blank_lines_should_not_merge_lists() {
            let doc = Parser::default().parse("List\n====\n\n. Foo\n. Boo\n\n//\n\n. Blech\n");

            assert_xpath(&doc, "//ol", 2);
            assert_xpath(&doc, "(//ol)[1]/li", 2);
            assert_xpath(&doc, "(//ol)[2]/li", 1);
        }

        #[test]
        fn dot_elements_separated_by_a_block_title_offset_by_a_blank_line_should_not_merge_lists() {
            let doc = Parser::default().parse("List\n====\n\n. Foo\n. Boo\n\n.Also\n. Blech\n");

            assert_xpath(&doc, "//ol", 2);
            assert_xpath(&doc, "(//ol)[1]/li", 2);
            assert_xpath(&doc, "(//ol)[2]/li", 1);
            assert_xpath(
                &doc,
                "(//ol)[2]/preceding-sibling::*[@class = \"title\"][text() = \"Also\"]",
                1,
            );
        }

        #[test]
        fn dot_elements_separated_by_an_attribute_entry_offset_by_a_blank_line_should_not_merge_lists()
         {
            let doc = Parser::default().parse("== List\n\n. Foo\n. Boo\n\n:foo: bar\n. Blech\n");

            assert_xpath(&doc, "//ol", 2);
            assert_xpath(&doc, "(//ol)[1]/li", 2);
            assert_xpath(&doc, "(//ol)[2]/li", 1);
        }

        #[test]
        fn should_honor_start_attribute_on_ordered_list() {
            let doc = Parser::default().parse("== List\n\n[start=7]\n. item 7\n. item 8\n");

            assert_css(&doc, "ol.arabic", 1);
            assert_css(&doc, "ol[start=7]", 1);
        }

        #[test]
        fn should_allow_value_of_start_attribute_to_be_0() {
            let doc =
                Parser::default().parse("== List\n\n[start=0]\n. item 0\n. item 1\n. item 2\n");

            assert_css(&doc, "ol.arabic", 1);
            assert_css(&doc, "ol[start=0]", 1);
        }

        #[test]
        fn should_allow_value_of_start_attribute_to_be_negative() {
            let doc = Parser::default().parse("== List\n\n[start=-10]\n. -10\n. -9\n. -8\n");

            assert_css(&doc, "ol.arabic", 1);
            assert_css(&doc, "ol[start=-10]", 1);
        }

        // Backend-specific test omitted: DocBook.

        #[test]
        fn should_not_warn_if_explicit_numbering_starts_at_value_of_start_attribute() {
            let doc = Parser::default().parse("== List\n\n[start=7]\n7. item 7\n8. item 8\n");

            assert_css(&doc, "ol[start=7]", 1);
            assert_css(&doc, "ol.arabic", 1);
            assert_css(&doc, "ol li", 2);

            assert!(doc.warnings().next().is_none());
        }

        #[test]
        fn should_implicitly_set_start_on_ordered_list_if_explicit_arabic_numbering_does_not_start_at_1()
         {
            let doc = Parser::default().parse("== List\n\n7. item 7\n8. item 8\n");

            assert_css(&doc, "ol[start=7]", 1);
            assert_css(&doc, "ol.arabic", 1);
            assert_css(&doc, "ol li", 2);

            assert!(doc.warnings().next().is_none());
        }

        #[test]
        fn should_implicitly_set_start_on_ordered_list_if_explicit_roman_numbering_does_not_start_at_1()
         {
            let doc = Parser::default().parse("== List\n\nIV) item 4\nV) item 5\n");

            assert_css(&doc, "ol[start=4]", 1);
            assert_css(&doc, "ol.upperroman", 1);
            assert_css(&doc, "ol li", 2);

            assert!(doc.warnings().next().is_none());
        }

        #[test]
        fn should_warn_if_item_with_explicit_numbering_in_ordered_list_is_out_of_sequence() {
            let doc = Parser::default().parse("== List\n\nx. x\nz. z\n");

            assert_css(&doc, "ol[start=24]", 1);
            assert_css(&doc, "ol.loweralpha", 1);
            assert_css(&doc, "ol li", 2);

            let warnings: Vec<_> = doc.warnings().collect();
            assert_eq!(warnings.len(), 1);

            assert_eq!(
                warnings[0].warning,
                crate::warnings::WarningType::ListItemOutOfSequence(
                    "y".to_string(),
                    "z".to_string()
                )
            );

            // Warning should point to line 4 (the "z. z" line).
            assert_eq!(warnings[0].source.line(), 4);
        }

        #[test]
        fn should_match_trailing_line_separator_in_text_of_list_item() {
            let doc = Parser::default().parse(". a\n. b\u{2028}\n. c");

            assert_css(&doc, "li", 3);
            assert_xpath(&doc, "(//li)[2]/p[text()=\"b\u{2028}\"]", 1);
        }

        #[test]
        fn should_match_line_separator_in_text_of_list_item() {
            let doc = Parser::default().parse(". a\n. b\u{2028}b\n. c");

            assert_css(&doc, "li", 3);
            assert_xpath(&doc, "(//li)[2]/p[text()=\"b\u{2028}b\"]", 1);
        }
    }

    #[test]
    fn should_warn_if_explicit_uppercase_roman_numerals_in_list_are_out_of_sequence() {
        let doc = Parser::default().parse("I) one\nIII) three\n");

        assert_xpath(&doc, "//ol/li", 2);

        let warnings: Vec<_> = doc.warnings().collect();
        assert_eq!(warnings.len(), 1);

        assert_eq!(
            warnings[0].warning,
            crate::warnings::WarningType::ListItemOutOfSequence(
                "II".to_string(),
                "III".to_string()
            )
        );

        // Warning should point to line 2 (the "III) three" line).
        assert_eq!(warnings[0].source.line(), 2);
    }

    #[test]
    fn should_warn_if_explicit_lowercase_roman_numerals_in_list_are_out_of_sequence() {
        let doc = Parser::default().parse("i) one\niii) three\n");

        assert_xpath(&doc, "//ol/li", 2);

        let warnings: Vec<_> = doc.warnings().collect();
        assert_eq!(warnings.len(), 1);

        assert_eq!(
            warnings[0].warning,
            crate::warnings::WarningType::ListItemOutOfSequence(
                "ii".to_string(),
                "iii".to_string()
            )
        );

        // Warning should point to line 2 (the "iii) three" line).
        assert_eq!(warnings[0].source.line(), 2);
    }
}

mod description_lists_dlist {
    use crate::{Parser, tests::prelude::*};

    mod simple_lists {
        use super::*;
        use crate::blocks::{ListType, SimpleBlockStyle};

        #[test]
        fn should_not_parse_a_bare_dlist_delimiter_as_a_dlist() {
            let doc = Parser::default().parse("::");

            assert_css(&doc, "dl", 0);
            assert_xpath(&doc, "//p[text()=\"::\"]", 1);
        }

        #[test]
        fn should_not_parse_an_indented_bare_dlist_delimiter_as_a_dlist() {
            let doc = Parser::default().parse(" ::");

            assert_css(&doc, "dl", 0);
            assert_xpath(&doc, "//pre[text()=\"::\"]", 1);
        }

        #[test]
        fn should_parse_a_dlist_delimiter_preceded_by_a_blank_attribute_as_a_dlist() {
            let doc = Parser::default().parse("{blank}::");

            assert_css(&doc, "dl", 1);
            assert_css(&doc, "dl > dt", 1);
            assert_css(&doc, "dl > dt:empty", 1);
        }

        #[test]
        fn should_parse_a_dlist_if_term_is_include_and_principal_text_is_brackets() {
            let doc = Parser::default().parse("include:: []");

            assert_css(&doc, "dl", 1);
            assert_css(&doc, "dl > dt", 1);
            assert_xpath(
                &doc,
                "(//dl/dt)[1]/following-sibling::dd/p[text() = \"[]\"]",
                1,
            );
        }

        #[test]
        fn should_parse_a_dlist_if_term_is_include_and_principal_text_matches_macro_form() {
            let doc = Parser::default().parse("include:: pass:[${placeholder}]");

            assert_css(&doc, "dl", 1);
            assert_css(&doc, "dl > dt", 1);

            assert_xpath(
                &doc,
                "(//dl/dt)[1]/following-sibling::dd/p[text() = \"${placeholder}\"]",
                1,
            );
        }

        #[test]
        fn single_line_adjacent_elements() {
            let doc = Parser::default().parse("term1:: def1\nterm2:: def2");

            assert_xpath(&doc, "//dl", 1);
            assert_xpath(&doc, "//dl/dt", 2);
            assert_xpath(&doc, "//dl/dt/following-sibling::dd", 2);

            assert_xpath(&doc, "(//dl/dt)[1][normalize-space(text()) = \"term1\"]", 1);

            assert_xpath(
                &doc,
                "(//dl/dt)[1]/following-sibling::dd/p[text() = \"def1\"]",
                1,
            );

            assert_xpath(&doc, "(//dl/dt)[2][normalize-space(text()) = \"term2\"]", 1);

            assert_xpath(
                &doc,
                "(//dl/dt)[2]/following-sibling::dd/p[text() = \"def2\"]",
                1,
            );
        }

        #[test]
        fn should_parse_sibling_items_using_same_rules() {
            let doc = Parser::default().parse("term1;; ;; def1\nterm2;; ;; def2\n");

            assert_xpath(&doc, "//dl", 1);
            assert_xpath(&doc, "//dl/dt", 2);
            assert_xpath(&doc, "//dl/dt/following-sibling::dd", 2);
            assert_xpath(&doc, "(//dl/dt)[1][normalize-space(text()) = \"term1\"]", 1);

            assert_xpath(
                &doc,
                "(//dl/dt)[1]/following-sibling::dd/p[text() = \";; def1\"]",
                1,
            );

            assert_xpath(&doc, "(//dl/dt)[2][normalize-space(text()) = \"term2\"]", 1);

            assert_xpath(
                &doc,
                "(//dl/dt)[2]/following-sibling::dd/p[text() = \";; def2\"]",
                1,
            );
        }

        #[test]
        fn should_allow_term_to_end_with_a_semicolon_when_using_double_semicolon_delimiter() {
            let doc = Parser::default().parse("term;;; def\n");

            assert_css(&doc, "dl", 1);
            assert_css(&doc, "dl > dt", 1);
            assert_xpath(&doc, "(//dl/dt)[1][text() = \"term;\"]", 1);
            assert_xpath(
                &doc,
                "(//dl/dt)[1]/following-sibling::dd/p[text() = \"def\"]",
                1,
            );
        }

        #[test]
        fn single_line_indented_adjacent_elements() {
            let doc = Parser::default().parse("term1:: def1\n term2:: def2\n");

            assert_xpath(&doc, "//dl", 1);
            assert_xpath(&doc, "//dl/dt", 2);
            assert_xpath(&doc, "//dl/dt/following-sibling::dd", 2);
            assert_xpath(&doc, "(//dl/dt)[1][normalize-space(text()) = \"term1\"]", 1);
            assert_xpath(
                &doc,
                "(//dl/dt)[1]/following-sibling::dd/p[text() = \"def1\"]",
                1,
            );
            assert_xpath(&doc, "(//dl/dt)[2][normalize-space(text()) = \"term2\"]", 1);
            assert_xpath(
                &doc,
                "(//dl/dt)[2]/following-sibling::dd/p[text() = \"def2\"]",
                1,
            );
        }

        #[test]
        fn single_line_indented_adjacent_elements_with_tabs() {
            let doc = Parser::default().parse("term1::\tdef1\n\tterm2::\tdef2\n");

            assert_xpath(&doc, "//dl", 1);
            assert_xpath(&doc, "//dl/dt", 2);
            assert_xpath(&doc, "//dl/dt/following-sibling::dd", 2);
            assert_xpath(&doc, "(//dl/dt)[1][normalize-space(text()) = \"term1\"]", 1);
            assert_xpath(
                &doc,
                "(//dl/dt)[1]/following-sibling::dd/p[text() = \"def1\"]",
                1,
            );
            assert_xpath(&doc, "(//dl/dt)[2][normalize-space(text()) = \"term2\"]", 1);
            assert_xpath(
                &doc,
                "(//dl/dt)[2]/following-sibling::dd/p[text() = \"def2\"]",
                1,
            );
        }

        #[test]
        fn single_line_elements_separated_by_blank_line_should_create_a_single_list() {
            let doc = Parser::default().parse("term1:: def1\n\nterm2:: def2\n");

            assert_xpath(&doc, "//dl", 1);
            assert_xpath(&doc, "//dl/dt", 2);
            assert_xpath(&doc, "//dl/dt/following-sibling::dd", 2);
        }

        #[test]
        fn a_line_comment_between_elements_should_divide_them_into_separate_lists() {
            let doc = Parser::default().parse("term1:: def1\n\n//\n\nterm2:: def2\n");

            assert_xpath(&doc, "//dl", 2);
            assert_xpath(&doc, "//dl/dt", 2);
            assert_xpath(&doc, "(//dl)[1]/dt", 1);
            assert_xpath(&doc, "(//dl)[2]/dt", 1);
        }

        #[test]
        #[ignore]
        // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/474):
        // Enable this test when rulers are implemented.
        fn a_ruler_between_elements_should_divide_them_into_separate_lists() {
            let _doc = Parser::default().parse("term1:: def1\n\n'''\n\nterm2:: def2\n");
            todo!("assert_xpath: '//dl', output, 2");
            todo!("assert_xpath: '//dl/dt', output, 2");
            todo!("assert_xpath: '//dl//hr', output, 0");
            todo!("assert_xpath: '(//dl)[1]/dt', output, 1");
            todo!("assert_xpath: '(//dl)[2]/dt', output, 1");
        }

        #[test]
        fn a_block_title_between_elements_should_divide_them_into_separate_lists() {
            let doc = Parser::default().parse("term1:: def1\n\n.Some more\nterm2:: def2\n");

            assert_xpath(&doc, "//dl", 2);
            assert_xpath(&doc, "//dl/dt", 2);
            assert_xpath(&doc, "(//dl)[1]/dt", 1);
            assert_xpath(&doc, "(//dl)[2]/dt", 1);

            assert_xpath(
                &doc,
                "(//dl)[2]/preceding-sibling::*[@class=\"title\"][text() = \"Some more\"]",
                1,
            );
        }

        #[test]
        fn multi_line_elements_with_paragraph_content() {
            let doc = Parser::default().parse("term1::\ndef1\nterm2::\ndef2\n");

            assert_xpath(&doc, "//dl", 1);
            assert_xpath(&doc, "//dl/dt", 2);
            assert_xpath(&doc, "//dl/dt/following-sibling::dd", 2);
            assert_xpath(&doc, "(//dl/dt)[1][normalize-space(text()) = \"term1\"]", 1);

            assert_xpath(
                &doc,
                "(//dl/dt)[1]/following-sibling::dd/p[text() = \"def1\"]",
                1,
            );

            assert_xpath(&doc, "(//dl/dt)[2][normalize-space(text()) = \"term2\"]", 1);

            assert_xpath(
                &doc,
                "(//dl/dt)[2]/following-sibling::dd/p[text() = \"def2\"]",
                1,
            );
        }

        #[test]
        fn multi_line_elements_with_indented_paragraph_content() {
            let doc = Parser::default().parse("term1::\n def1\nterm2::\n  def2\n");

            assert_xpath(&doc, "//dl", 1);
            assert_xpath(&doc, "//dl/dt", 2);
            assert_xpath(&doc, "//dl/dt/following-sibling::dd", 2);
            assert_xpath(&doc, "(//dl/dt)[1][normalize-space(text()) = \"term1\"]", 1);

            assert_xpath(
                &doc,
                "(//dl/dt)[1]/following-sibling::dd/p[text() = \"def1\"]",
                1,
            );

            assert_xpath(&doc, "(//dl/dt)[2][normalize-space(text()) = \"term2\"]", 1);

            assert_xpath(
                &doc,
                "(//dl/dt)[2]/following-sibling::dd/p[text() = \"def2\"]",
                1,
            );
        }

        #[test]
        fn multi_line_elements_with_indented_paragraph_content_that_includes_comment_lines() {
            let doc = Parser::default().parse(
                "term1::\n def1\n// comment\nterm2::\n  def2\n// comment\n  def2 continued\n",
            );

            assert_xpath(&doc, "//dl", 1);
            assert_xpath(&doc, "//dl/dt", 2);
            assert_xpath(&doc, "//dl/dt/following-sibling::dd", 2);
            assert_xpath(&doc, "(//dl/dt)[1][normalize-space(text()) = \"term1\"]", 1);

            assert_xpath(
                &doc,
                "(//dl/dt)[1]/following-sibling::dd/p[text() = \"def1\"]",
                1,
            );

            assert_xpath(&doc, "(//dl/dt)[2][normalize-space(text()) = \"term2\"]", 1);

            assert_xpath(
                &doc,
                "(//dl/dt)[2]/following-sibling::dd/p[text() = \"def2\ndef2 continued\"]",
                1,
            );
        }

        #[test]
        fn should_not_strip_comment_line_in_literal_paragraph_block_attached_to_list_item() {
            let doc = Parser::default().parse("term1::\n+\n line 1\n// not a comment\n line 3\n");

            assert_xpath(&doc, "//*[@class=\"literalblock\"]", 1);

            assert_xpath(
                &doc,
                "//*[@class=\"literalblock\"]//pre[text()=\" line 1\n// not a comment\n line 3\"]",
                1,
            );
        }

        #[test]
        fn should_escape_special_characters_in_all_literal_paragraphs_attached_to_list_item() {
            let doc = Parser::default().parse("term:: desc\n\n  <code>text</code>\n\n  more <code>text</code>\n\nanother term::\n\n  <code>text</code> in a paragraph\n");

            assert_css(&doc, "dt", 2);
            assert_css(&doc, "code", 0);
            assert_css(&doc, "dd:first-of-type > *", 3);
            assert_css(&doc, "dd:first-of-type pre", 2);

            assert_xpath(&doc, "((//dd)[1]//pre)[1][text()=\"<code>text</code>\"]", 1);

            assert_xpath(
                &doc,
                "((//dd)[1]//pre)[2][text()=\"more <code>text</code>\"]",
                1,
            );

            assert_xpath(
                &doc,
                "((//dd)[2]//p)[1][text()=\"<code>text</code> in a paragraph\"]",
                1,
            );
        }

        #[test]
        fn multi_line_element_with_paragraph_starting_with_multiple_dashes_should_not_be_seen_as_list()
         {
            let doc =
                Parser::default().parse("term1::\n  def1\n  -- and a note\n\nterm2::\n  def2\n");

            assert_xpath(&doc, "//dl", 1);
            assert_xpath(&doc, "//dl/dt", 2);
            assert_xpath(&doc, "//dl/dt/following-sibling::dd", 2);
            assert_xpath(&doc, "(//dl/dt)[1][normalize-space(text()) = \"term1\"]", 1);

            assert_xpath(
                &doc,
                "(//dl/dt)[1]/following-sibling::dd/p[text() = \"def1&#8201;&#8212;&#8201;and a note\"]",
                1,
            );

            assert_xpath(&doc, "(//dl/dt)[2][normalize-space(text()) = \"term2\"]", 1);

            assert_xpath(
                &doc,
                "(//dl/dt)[2]/following-sibling::dd/p[text() = \"def2\"]",
                1,
            );
        }

        #[test]
        fn multi_line_element_with_multiple_terms() {
            let doc = Parser::default().parse("term1::\nterm2::\ndef2\n");

            assert_xpath(&doc, "//dl", 1);
            assert_xpath(&doc, "//dl/dt", 2);
            assert_xpath(&doc, "//dl/dd", 1);
            assert_xpath(&doc, "(//dl/dt)[1]/following-sibling::dt", 1);
            assert_xpath(&doc, "(//dl/dt)[1][normalize-space(text()) = \"term1\"]", 1);
            assert_xpath(&doc, "(//dl/dt)[2]/following-sibling::dd", 1);

            assert_xpath(
                &doc,
                "(//dl/dt)[2]/following-sibling::dd/p[text() = \"def2\"]",
                1,
            );
        }

        // Backend-specific test omitted: DocBook.

        #[test]
        fn multi_line_elements_with_blank_line_before_paragraph_content() {
            let doc = Parser::default().parse("term1::\n\ndef1\nterm2::\n\ndef2\n");

            assert_xpath(&doc, "//dl", 1);
            assert_xpath(&doc, "//dl/dt", 2);
            assert_xpath(&doc, "//dl/dt/following-sibling::dd", 2);
            assert_xpath(&doc, "(//dl/dt)[1][normalize-space(text()) = \"term1\"]", 1);

            assert_xpath(
                &doc,
                "(//dl/dt)[1]/following-sibling::dd/p[text() = \"def1\"]",
                1,
            );

            assert_xpath(&doc, "(//dl/dt)[2][normalize-space(text()) = \"term2\"]", 1);

            assert_xpath(
                &doc,
                "(//dl/dt)[2]/following-sibling::dd/p[text() = \"def2\"]",
                1,
            );
        }

        #[test]
        fn multi_line_elements_with_paragraph_and_literal_content() {
            // NOTE: blank line following literal paragraph is required or else it will
            // gobble up the second term.
            let doc = Parser::default().parse("term1::\ndef1\n\n  literal\n\nterm2::\n  def2\n");

            assert_xpath(&doc, "//dl", 1);
            assert_xpath(&doc, "//dl/dt", 2);
            assert_xpath(&doc, "//dl/dt/following-sibling::dd", 2);
            assert_xpath(&doc, "//dl/dt/following-sibling::dd//pre", 1);
            assert_xpath(&doc, "(//dl/dt)[1][normalize-space(text()) = \"term1\"]", 1);

            assert_xpath(
                &doc,
                "(//dl/dt)[1]/following-sibling::dd/p[text() = \"def1\"]",
                1,
            );

            assert_xpath(&doc, "(//dl/dt)[2][normalize-space(text()) = \"term2\"]", 1);

            assert_xpath(
                &doc,
                "(//dl/dt)[2]/following-sibling::dd/p[text() = \"def2\"]",
                1,
            );
        }

        #[test]
        fn mixed_single_and_multi_line_adjacent_elements() {
            let doc = Parser::default().parse("term1:: def1\nterm2::\ndef2\n");

            assert_xpath(&doc, "//dl", 1);
            assert_xpath(&doc, "//dl/dt", 2);
            assert_xpath(&doc, "//dl/dt/following-sibling::dd", 2);
            assert_xpath(&doc, "(//dl/dt)[1][normalize-space(text()) = \"term1\"]", 1);

            assert_xpath(
                &doc,
                "(//dl/dt)[1]/following-sibling::dd/p[text() = \"def1\"]",
                1,
            );

            assert_xpath(&doc, "(//dl/dt)[2][normalize-space(text()) = \"term2\"]", 1);

            assert_xpath(
                &doc,
                "(//dl/dt)[2]/following-sibling::dd/p[text() = \"def2\"]",
                1,
            );
        }

        #[test]
        #[ignore]
        // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/476):
        // Enable this test when << >> xrefs are implemented.
        fn should_discover_anchor_at_start_of_description_term_text_and_register_it_as_a_reference()
        {
            let doc = Parser::default().parse("The highest peak in the Front Range is <<grays-peak>>, which tops <<mount-evans>> by just a few feet.\n\n[[mount-evans,Mount Evans]]Mount Evans:: 14,271 feet\n[[grays-peak]]Grays Peak:: 14,278 feet\n");

            dbg!(&doc);
            // refs = doc.catalog[:refs]
            // assert refs.key?('mount-evans')
            // assert refs.key?('grays-peak')

            assert_xpath(
                &doc,
                "(//p)[1]/a[@href=\"#grays-peak\"][text()=\"Grays Peak\"]",
                1,
            );
            assert_xpath(
                &doc,
                "(//p)[1]/a[@href=\"#mount-evans\"][text()=\"Mount Evans\"]",
                1,
            );
            assert_xpath(&doc, "//dl", 1);
            assert_xpath(&doc, "//dl/dt", 2);
            assert_xpath(&doc, "(//dl/dt)[1]/a[@id=\"mount-evans\"]", 1);
            assert_xpath(&doc, "(//dl/dt)[2]/a[@id=\"grays-peak\"]", 1);
        }

        #[test]
        fn missing_space_before_term_does_not_produce_description_list() {
            let doc = Parser::default().parse("term1::def1\nterm2::def2\n");

            assert_xpath(&doc, "//dl", 0);
        }

        #[test]
        fn literal_block_inside_description_list() {
            let doc = Parser::default().parse(
                "term::\n+\n....\nliteral, line 1\n\nliteral, line 2\n....\nanotherterm:: def\n",
            );

            assert_xpath(&doc, "//dl/dt", 2);
            assert_xpath(&doc, "//dl/dd", 2);
            assert_xpath(&doc, "//dl/dd//pre", 1);
            assert_xpath(&doc, "(//dl/dd)[1]/*[@class=\"literalblock\"]//pre", 1);
            assert_xpath(&doc, "(//dl/dd)[2]/p[text() = \"def\"]", 1);
        }

        #[test]
        fn literal_block_inside_description_list_with_trailing_line_continuation() {
            let doc = Parser::default().parse(
                "term::\n+\n....\nliteral, line 1\n\nliteral, line 2\n....\n+\nanotherterm:: def\n",
            );

            assert_xpath(&doc, "//dl/dt", 2);
            assert_xpath(&doc, "//dl/dd", 2);
            assert_xpath(&doc, "//dl/dd//pre", 1);
            assert_xpath(&doc, "(//dl/dd)[1]/*[@class=\"literalblock\"]//pre", 1);
            assert_xpath(&doc, "(//dl/dd)[2]/p[text() = \"def\"]", 1);
        }

        #[test]
        fn multiple_listing_blocks_inside_description_list() {
            let doc = Parser::default().parse("term::\n+\n----\nlisting, line 1\n\nlisting, line 2\n----\n+\n----\nlisting, line 1\n\nlisting, line 2\n----\nanotherterm:: def\n");

            assert_xpath(&doc, "//dl/dt", 2);
            assert_xpath(&doc, "//dl/dd", 2);
            assert_xpath(&doc, "//dl/dd//pre", 2);
            assert_xpath(&doc, "(//dl/dd)[1]/*[@class=\"listingblock\"]//pre", 2);
            assert_xpath(&doc, "(//dl/dd)[2]/p[text() = \"def\"]", 1);
        }

        #[test]
        fn open_block_inside_description_list() {
            let doc = Parser::default().parse("term::\n+\n--\nOpen block as description of term.\n\nAnd some more detail...\n--\nanotherterm:: def\n");

            assert_xpath(&doc, "//dl/dd//p", 3);
            assert_xpath(&doc, "(//dl/dd)[1]//*[@class=\"openblock\"]//p", 2);
        }

        #[test]
        fn paragraph_attached_by_a_list_continuation_on_either_side_in_a_description_list() {
            let doc = Parser::default().parse("term1:: def1\n+\nmore detail\n+\nterm2:: def2\n");

            assert_xpath(&doc, "(//dl/dt)[1][normalize-space(text())=\"term1\"]", 1);
            assert_xpath(&doc, "(//dl/dt)[2][normalize-space(text())=\"term2\"]", 1);
            assert_xpath(&doc, "(//dl/dd)[1]//p", 2);
            assert_xpath(&doc, "((//dl/dd)[1]//p)[1][text()=\"def1\"]", 1);

            assert_xpath(
                &doc,
                "(//dl/dd)[1]/p/following-sibling::*[@class=\"paragraph\"]/p[text() = \"more detail\"]",
                1,
            );
        }

        #[test]
        fn paragraph_attached_by_a_list_continuation_on_either_side_to_a_multi_line_element_in_a_description_list()
         {
            let doc = Parser::default().parse("term1::\ndef1\n+\nmore detail\n+\nterm2:: def2\n");

            assert_xpath(&doc, "(//dl/dt)[1][normalize-space(text())=\"term1\"]", 1);
            assert_xpath(&doc, "(//dl/dt)[2][normalize-space(text())=\"term2\"]", 1);
            assert_xpath(&doc, "(//dl/dd)[1]//p", 2);
            assert_xpath(&doc, "((//dl/dd)[1]//p)[1][text()=\"def1\"]", 1);

            assert_xpath(
                &doc,
                "(//dl/dd)[1]/p/following-sibling::*[@class=\"paragraph\"]/p[text() = \"more detail\"]",
                1,
            );
        }

        #[test]
        #[ignore]
        // TO DO (https://github.com/asciidoc-rs/asciidoc-parser/issues/478):
        // Enable this test after `attribute-missing` is handled.
        fn should_continue_to_parse_subsequent_blocks_attached_to_list_item_after_first_block_is_dropped()
         {
            let _doc = Parser::default().parse(
                ":attribute-missing: drop-line\n\nterm::\n+\nimage::{unresolved}[]\n+\nparagraph\n",
            );
            todo!("assert_css: 'dl', output, 1");
            todo!("assert_css: 'dl > dt', output, 1");
            todo!("assert_css: 'dl > dt + dd', output, 1");
            todo!("assert_css: 'dl > dt + dd > .imageblock', output, 0");
            todo!("assert_css: 'dl > dt + dd > .paragraph', output, 1");
        }

        #[test]
        fn verse_paragraph_inside_a_description_list() {
            let doc = Parser::default().parse("term1:: def\n+\n[verse]\nla la la\n\nterm2:: def\n");

            assert_xpath(&doc, "//dl/dd//p", 2);

            assert_xpath(
                &doc,
                "(//dl/dd)[1]/*[@class=\"verseblock\"]/pre[text() = \"la la la\"]",
                1,
            );
        }

        #[test]
        fn list_inside_a_description_list() {
            let doc =
                Parser::default().parse("term1::\n* level 1\n** level 2\n* level 1\nterm2:: def\n");

            assert_xpath(&doc, "//dl/dd", 2);
            assert_xpath(&doc, "//dl/dd/p", 1);
            assert_xpath(&doc, "(//dl/dd)[1]//ul", 2);
            assert_xpath(&doc, "((//dl/dd)[1]//ul)[1]//ul", 1);
        }

        #[test]
        fn list_inside_a_description_list_offset_by_blank_lines() {
            let doc = Parser::default()
                .parse("term1::\n\n* level 1\n** level 2\n* level 1\n\nterm2:: def\n");

            assert_xpath(&doc, "//dl/dd", 2);
            assert_xpath(&doc, "//dl/dd/p", 1);
            assert_xpath(&doc, "(//dl/dd)[1]//ul", 2);
            assert_xpath(&doc, "((//dl/dd)[1]//ul)[1]//ul", 1);
        }

        #[test]
        fn should_only_grab_one_line_following_last_item_if_item_has_no_inline_description() {
            let doc = Parser::default().parse(
                "term1::\n\ndef1\n\nterm2::\n\ndef2\n\nA new paragraph\n\nAnother new paragraph\n",
            );

            assert_xpath(&doc, "//dl", 1);
            assert_xpath(&doc, "//dl/dd", 2);
            assert_xpath(&doc, "(//dl/dd)[1]/p[text() = \"def1\"]", 1);
            assert_xpath(&doc, "(//dl/dd)[2]/p[text() = \"def2\"]", 1);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]/following-sibling::*[@class=\"paragraph\"]",
                2,
            );

            assert_xpath(
                &doc,
                "(//*[@class=\"dlist\"]/following-sibling::*[@class=\"paragraph\"])[1]/p[text() = \"A new paragraph\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//*[@class=\"dlist\"]/following-sibling::*[@class=\"paragraph\"])[2]/p[text() = \"Another new paragraph\"]",
                1,
            );
        }

        #[test]
        fn should_only_grab_one_literal_line_following_last_item_if_item_has_no_inline_description()
        {
            let doc = Parser::default().parse("term1::\n\ndef1\n\nterm2::\n\n  def2\n\nA new paragraph\n\nAnother new paragraph\n");

            assert_xpath(&doc, "//dl", 1);
            assert_xpath(&doc, "//dl/dd", 2);
            assert_xpath(&doc, "(//dl/dd)[1]/p[text() = \"def1\"]", 1);
            assert_xpath(&doc, "(//dl/dd)[2]/p[text() = \"def2\"]", 1);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]/following-sibling::*[@class=\"paragraph\"]",
                2,
            );

            assert_xpath(
                &doc,
                "(//*[@class=\"dlist\"]/following-sibling::*[@class=\"paragraph\"])[1]/p[text() = \"A new paragraph\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//*[@class=\"dlist\"]/following-sibling::*[@class=\"paragraph\"])[2]/p[text() = \"Another new paragraph\"]",
                1,
            );
        }

        #[test]
        fn should_append_subsequent_paragraph_literals_to_list_item_as_block_content() {
            let doc = Parser::default()
                .parse("term1::\n\ndef1\n\nterm2::\n\n  def2\n\n  literal\n\nA new paragraph.\n");

            assert_xpath(&doc, "//dl", 1);
            assert_xpath(&doc, "//dl/dd", 2);
            assert_xpath(&doc, "(//dl/dd)[1]/p[text() = \"def1\"]", 1);
            assert_xpath(&doc, "(//dl/dd)[2]/p[text() = \"def2\"]", 1);

            assert_xpath(
                &doc,
                "(//dl/dd)[2]/p/following-sibling::*[@class=\"literalblock\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//dl/dd)[2]/p/following-sibling::*[@class=\"literalblock\"]//pre[text() = \"literal\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]/following-sibling::*[@class=\"paragraph\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//*[@class=\"dlist\"]/following-sibling::*[@class=\"paragraph\"])[1]/p[text() = \"A new paragraph.\"]",
                1,
            );
        }

        #[test]
        fn should_not_match_comment_line_that_looks_like_description_list_term() {
            let doc = Parser::default().parse("before\n\n//key:: val\n\nafter\n");

            assert_css(&doc, "dl", 0);
        }

        #[test]
        fn should_not_match_comment_line_following_list_that_looks_like_description_list_term() {
            let doc =
                Parser::default().parse("* item\n\n//term:: desc\n== Section\n\nsection text\n");

            assert_xpath(&doc, "/*[@class=\"ulist\"]", 1);
            assert_xpath(&doc, "/*[@class=\"sect1\"]", 1);
            assert_xpath(&doc, "/*[@class=\"sect1\"]/h2[text()=\"Section\"]", 1);

            assert_xpath(
                &doc,
                "/*[@class=\"ulist\"]/following-sibling::*[@class=\"sect1\"]",
                1,
            );
        }

        #[test]
        fn should_not_match_comment_line_that_looks_like_sibling_description_list_term() {
            let doc = Parser::default().parse("before\n\nfoo:: bar\n//yin:: yang\n\nafter\n");

            assert_css(&doc, ".dlist", 1);
            assert_css(&doc, ".dlist dt", 1);
            refute_output_contains(&doc, "yin");
        }

        #[test]
        fn should_not_hang_on_description_list_item_in_list_that_begins_with_triple_slash() {
            let doc = Parser::default().parse("* a\n///b::\nc\n");

            assert_css(&doc, "ul", 1);
            assert_css(&doc, "ul li dl", 1);
            assert_xpath(&doc, "//ul/li/p[text()=\"a\"]", 1);
            assert_xpath(&doc, "//dt[text()=\"///b\"]", 1);
            assert_xpath(&doc, "//dd/p[text()=\"c\"]", 1);
        }

        #[test]
        fn should_not_hang_on_sibling_description_list_item_that_begins_with_triple_slash() {
            let doc = Parser::default().parse("a::\n///b::\nc\n");

            assert_css(&doc, "dl", 1);
            assert_xpath(&doc, "(//dl/dt)[1][text()=\"a\"]", 1);
            assert_xpath(&doc, "(//dl/dt)[2][text()=\"///b\"]", 1);
            assert_xpath(&doc, "//dl/dd/p[text()=\"c\"]", 1);
        }

        #[test]
        fn should_skip_dlist_term_that_begins_with_double_slash_unless_it_begins_with_triple_slash()
        {
            let doc = Parser::default()
                .parse("category a::\n//ignored term:: def\n\ncategory b::\n///term:: def\n");

            refute_output_contains(&doc, "ignored term");
            assert_xpath(&doc, "//dt[text()=\"///term\"]", 1);
        }

        #[test]
        fn more_than_4_consecutive_colons_should_become_part_of_description_list_term() {
            let doc = Parser::default().parse("A term::::: a description\n");

            assert_css(&doc, "dl", 1);
            assert_css(&doc, "dl > dt", 1);
            assert_xpath(&doc, "//dl/dt[text()=\"A term:\"]", 1);
            assert_xpath(&doc, "//dl/dd/p[text()=\"a description\"]", 1);
        }

        #[test]
        fn text_method_of_dd_node_should_return_nil_if_dd_node_only_contains_blocks() {
            let doc = Parser::default().parse("term::\n+\nparagraph\n");

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
                    blocks: &[Block::List(ListBlock {
                        type_: ListType::Description,
                        items: &[Block::ListItem(ListItem {
                            marker: ListItemMarker::DefinedTerm {
                                term: Content {
                                    original: Span {
                                        data: "term",
                                        line: 1,
                                        col: 1,
                                        offset: 0,
                                    },
                                    rendered: "term",
                                },
                                marker: Span {
                                    data: "::",
                                    line: 1,
                                    col: 5,
                                    offset: 4,
                                },
                                source: Span {
                                    data: "term::",
                                    line: 1,
                                    col: 1,
                                    offset: 0,
                                },
                            },
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "paragraph",
                                        line: 3,
                                        col: 1,
                                        offset: 9,
                                    },
                                    rendered: "paragraph",
                                },
                                source: Span {
                                    data: "paragraph",
                                    line: 3,
                                    col: 1,
                                    offset: 9,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: "term::\n+\nparagraph",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "term::\n+\nparagraph",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "term::\n+\nparagraph",
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
        fn should_match_trailing_line_separator_in_text_of_list_item() {
            let doc = Parser::default().parse("A:: a\nB:: b\u{2028}\nC:: c");

            assert_css(&doc, "dd", 3);
            assert_xpath(&doc, "(//dd)[2]/p[text()=\"b\u{2028}\"]", 1);
        }

        #[test]
        fn should_match_line_separator_in_text_of_list_item() {
            let doc = Parser::default().parse("A:: a\nB:: b\u{2028}b\nC:: c");

            assert_css(&doc, "dd", 3);
            assert_xpath(&doc, "(//dd)[2]/p[text()=\"b\u{2028}b\"]", 1);
        }
    }

    mod nested_lists {
        use super::*;

        #[test]
        fn should_not_parse_a_nested_dlist_delimiter_without_a_term_as_a_dlist() {
            let doc = Parser::default().parse("t::\n;;\n");

            assert_xpath(&doc, "//dl", 1);
            assert_xpath(&doc, "//dl/dd/p[text()=\";;\"]", 1);
        }

        #[test]
        fn should_not_parse_a_nested_indented_dlist_delimiter_without_a_term_as_a_dlist() {
            let doc = Parser::default().parse("t::\ndesc\n  ;;\n");

            assert_xpath(&doc, "//dl", 1);
            assert_xpath(&doc, "//dl/dd/p[text()=\"desc\n  ;;\"]", 1);
        }

        #[test]
        fn single_line_adjacent_nested_elements() {
            let doc = Parser::default().parse("term1:: def1\nlabel1::: detail1\nterm2:: def2\n");

            assert_xpath(&doc, "//dl", 2);
            assert_xpath(&doc, "//dl//dl", 1);

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[1][normalize-space(text()) = \"term1\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[1]/following-sibling::dd/p[text() = \"def1\"]",
                1,
            );

            assert_xpath(&doc, "//dl//dl/dt[normalize-space(text()) = \"label1\"]", 1);

            assert_xpath(
                &doc,
                "//dl//dl/dt/following-sibling::dd/p[text() = \"detail1\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[2][normalize-space(text()) = \"term2\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[2]/following-sibling::dd/p[text() = \"def2\"]",
                1,
            );
        }

        #[test]
        fn single_line_adjacent_maximum_nested_elements() {
            let doc = Parser::default().parse(
                "term1:: def1\nlabel1::: detail1\nname1:::: value1\nitem1;; price1\nterm2:: def2\n",
            );

            assert_xpath(&doc, "//dl", 4);
            assert_xpath(&doc, "//dl//dl//dl//dl", 1);
        }

        #[test]
        fn single_line_nested_elements_separated_by_blank_line_at_top_level() {
            let doc =
                Parser::default().parse("term1:: def1\n\nlabel1::: detail1\n\nterm2:: def2\n");

            assert_xpath(&doc, "//dl", 2);
            assert_xpath(&doc, "//dl//dl", 1);

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[1][normalize-space(text()) = \"term1\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[1]/following-sibling::dd/p[text() = \"def1\"]",
                1,
            );

            assert_xpath(&doc, "//dl//dl/dt[normalize-space(text()) = \"label1\"]", 1);

            assert_xpath(
                &doc,
                "//dl//dl/dt/following-sibling::dd/p[text() = \"detail1\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[2][normalize-space(text()) = \"term2\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[2]/following-sibling::dd/p[text() = \"def2\"]",
                1,
            );
        }

        #[test]
        fn single_line_nested_elements_separated_by_blank_line_at_nested_level() {
            let doc = Parser::default()
                .parse("term1:: def1\nlabel1::: detail1\n\nlabel2::: detail2\nterm2:: def2\n");

            assert_xpath(&doc, "//dl", 2);
            assert_xpath(&doc, "//dl//dl", 1);

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[1][normalize-space(text()) = \"term1\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[1]/following-sibling::dd/p[text() = \"def1\"]",
                1,
            );

            assert_xpath(&doc, "//dl//dl/dt[normalize-space(text()) = \"label1\"]", 1);

            assert_xpath(
                &doc,
                "//dl//dl/dt/following-sibling::dd/p[text() = \"detail1\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[2][normalize-space(text()) = \"term2\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[2]/following-sibling::dd/p[text() = \"def2\"]",
                1,
            );
        }

        #[test]
        fn single_line_adjacent_nested_elements_with_alternate_delimiters() {
            let doc = Parser::default().parse("term1:: def1\nlabel1;; detail1\nterm2:: def2\n");

            assert_xpath(&doc, "//dl", 2);
            assert_xpath(&doc, "//dl//dl", 1);

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[1][normalize-space(text()) = \"term1\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[1]/following-sibling::dd/p[text() = \"def1\"]",
                1,
            );

            assert_xpath(&doc, "//dl//dl/dt[normalize-space(text()) = \"label1\"]", 1);

            assert_xpath(
                &doc,
                "//dl//dl/dt/following-sibling::dd/p[text() = \"detail1\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[2][normalize-space(text()) = \"term2\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[2]/following-sibling::dd/p[text() = \"def2\"]",
                1,
            );
        }

        #[test]
        fn multi_line_adjacent_nested_elements() {
            let doc = Parser::default().parse("term1::\ndef1\nlabel1:::\ndetail1\nterm2::\ndef2\n");

            assert_xpath(&doc, "//dl", 2);
            assert_xpath(&doc, "//dl//dl", 1);

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[1][normalize-space(text()) = \"term1\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[1]/following-sibling::dd/p[text() = \"def1\"]",
                1,
            );

            assert_xpath(&doc, "//dl//dl/dt[normalize-space(text()) = \"label1\"]", 1);

            assert_xpath(
                &doc,
                "//dl//dl/dt/following-sibling::dd/p[text() = \"detail1\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[2][normalize-space(text()) = \"term2\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[2]/following-sibling::dd/p[text() = \"def2\"]",
                1,
            );
        }

        #[test]
        fn multi_line_nested_elements_separated_by_blank_line_at_nested_level_repeated() {
            let doc = Parser::default()
                .parse("term1::\ndef1\nlabel1:::\n\ndetail1\nlabel2:::\ndetail2\n\nterm2:: def2\n");

            assert_xpath(&doc, "//dl", 2);
            assert_xpath(&doc, "//dl//dl", 1);

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[1][normalize-space(text()) = \"term1\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[1]/following-sibling::dd/p[text() = \"def1\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//dl//dl/dt)[1][normalize-space(text()) = \"label1\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//dl//dl/dt)[1]/following-sibling::dd/p[text() = \"detail1\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//dl//dl/dt)[2][normalize-space(text()) = \"label2\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//dl//dl/dt)[2]/following-sibling::dd/p[text() = \"detail2\"]",
                1,
            );
        }

        #[test]
        fn multi_line_element_with_indented_nested_element() {
            let doc = Parser::default()
                .parse("term1::\n  def1\n  label1;;\n   detail1\nterm2::\n  def2\n");

            assert_xpath(&doc, "//dl", 2);
            assert_xpath(&doc, "//dl//dl", 1);
            assert_xpath(&doc, "(//dl)[1]/dt", 2);
            assert_xpath(&doc, "(//dl)[1]/dd", 2);

            assert_xpath(
                &doc,
                "((//dl)[1]/dt)[1][normalize-space(text()) = \"term1\"]",
                1,
            );

            assert_xpath(
                &doc,
                "((//dl)[1]/dt)[1]/following-sibling::dd/p[text() = \"def1\"]",
                1,
            );

            assert_xpath(&doc, "//dl//dl/dt", 1);
            assert_xpath(&doc, "//dl//dl/dt[normalize-space(text()) = \"label1\"]", 1);

            assert_xpath(
                &doc,
                "//dl//dl/dt/following-sibling::dd/p[text() = \"detail1\"]",
                1,
            );

            assert_xpath(
                &doc,
                "((//dl)[1]/dt)[2][normalize-space(text()) = \"term2\"]",
                1,
            );

            assert_xpath(
                &doc,
                "((//dl)[1]/dt)[2]/following-sibling::dd/p[text() = \"def2\"]",
                1,
            );
        }

        #[test]
        fn mixed_single_and_multi_line_elements_with_indented_nested_elements() {
            let doc =
                Parser::default().parse("term1:: def1\n  label1:::\n   detail1\nterm2:: def2\n");

            assert_xpath(&doc, "//dl", 2);
            assert_xpath(&doc, "//dl//dl", 1);

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[1][normalize-space(text()) = \"term1\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[1]/following-sibling::dd/p[text() = \"def1\"]",
                1,
            );

            assert_xpath(&doc, "//dl//dl/dt[normalize-space(text()) = \"label1\"]", 1);

            assert_xpath(
                &doc,
                "//dl//dl/dt/following-sibling::dd/p[text() = \"detail1\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[2][normalize-space(text()) = \"term2\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[2]/following-sibling::dd/p[text() = \"def2\"]",
                1,
            );
        }

        #[test]
        fn multi_line_elements_with_first_paragraph_folded_to_text_with_adjacent_nested_element() {
            let doc = Parser::default().parse("term1:: def1\ncontinued\nlabel1:::\ndetail1\n");

            assert_xpath(&doc, "//dl", 2);
            assert_xpath(&doc, "//dl//dl", 1);

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[1][normalize-space(text()) = \"term1\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[1]/following-sibling::dd/p[starts-with(text(), \"def1\")]",
                1,
            );

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[1]/following-sibling::dd/p[contains(text(), \"continued\")]",
                1,
            );

            assert_xpath(&doc, "//dl//dl/dt[normalize-space(text()) = \"label1\"]", 1);

            assert_xpath(
                &doc,
                "//dl//dl/dt/following-sibling::dd/p[text() = \"detail1\"]",
                1,
            );
        }

        #[test]
        fn nested_dlist_attached_by_list_continuation_should_not_consume_detached_paragraph() {
            let doc = Parser::default().parse("term:: text\n+\nnested term::: text\n\nparagraph\n");

            assert_xpath(&doc, "//dl", 2);
            assert_xpath(&doc, "//dl//dl", 1);

            assert_css(&doc, ".dlist .paragraph", 0);
            assert_css(&doc, ".dlist + .paragraph", 1);
        }

        #[test]
        fn nested_dlist_with_attached_block_offset_by_empty_line() {
            let doc = Parser::default().parse("category::\n\nterm 1:::\n+\n--\ndef 1\n--\n");

            assert_xpath(&doc, "//dl", 2);
            assert_xpath(&doc, "//dl//dl", 1);

            assert_xpath(
                &doc,
                "(//dl)[1]/dt[1][normalize-space(text()) = \"category\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//dl)[1]//dl/dt[1][normalize-space(text()) = \"term 1\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//dl)[1]//dl/dt[1]/following-sibling::dd//p[starts-with(text(), \"def 1\")]",
                1,
            );
        }
    }

    mod special_lists {
        use super::*;

        #[test]
        fn should_convert_glossary_list_with_proper_semantics() {
            let doc = Parser::default().parse("[glossary]\nterm 1:: def 1\nterm 2:: def 2\n");

            assert_css(&doc, ".dlist.glossary", 1);
            assert_css(&doc, ".dlist dt:not([class])", 2);
        }

        // Backend-specific test omitted: DocBook.

        // Skipping tests 'should convert horizontal list with proper markup' and
        // 'should set col widths of item and label if specified' because we don't do
        // HTML rendering from this crate. The role should appear automatically.

        // Backend-specific test omitted: DocBook.

        // Skipping tests 'should add strong class to label if strong option is set' and
        // 'consecutive terms in horizontal list should share same cell' because we
        // don't do HTML rendering from this crate.

        // Backend-specific test omitted: DocBook.
        // Backend-specific test omitted: DocBook.

        // Skipping test 'should convert qanda list in HTML with proper semantics'
        // because we don't do HTML rendering from this crate.

        // Backend-specific test omitted: DocBook.
        // Backend-specific test omitted: DocBook.

        #[test]
        #[ignore]
        // TO DO (https://github.com/asciidoc-rs/asciidoc-parser/issues/479):
        // Enable this test when bibliography parsing is enabled.
        fn should_convert_bibliography_list_with_proper_semantics() {
            let _doc = Parser::default().parse("[bibliography]\n- [[[taoup]]] Eric Steven Raymond. _The Art of Unix\n  Programming_. Addison-Wesley. ISBN 0-13-142901-9.\n- [[[walsh-muellner]]] Norman Walsh & Leonard Muellner.\n  _DocBook - The Definitive Guide_. O'Reilly & Associates. 1999.\n  ISBN 1-56592-580-7.\n");
            todo!("assert_css: '.ulist.bibliography', output, 1");
            todo!("assert_css: '.ulist.bibliography ul', output, 1");
            todo!("assert_css: '.ulist.bibliography ul li', output, 2");
            todo!("assert_css: '.ulist.bibliography ul li p', output, 2");
            todo!("assert_css: '.ulist.bibliography ul li:nth-child(1) p a#taoup', output, 1");
            todo!("assert_xpath: '//a/*', output, 0");
            todo!(
                "assert_xpath: '(//a)[1][starts-with(following-sibling::text(), \"[taoup] \")]', output, 1"
            );
        }

        // Backend-specific test omitted: DocBook.

        #[test]
        #[ignore]
        // TO DO (https://github.com/asciidoc-rs/asciidoc-parser/issues/479):
        // Enable this test when bibliography parsing is enabled.
        fn should_warn_if_a_bibliography_id_is_already_in_use() {
            let _doc = Parser::default().parse("[bibliography]\n* [[[Fowler]]] Fowler M. _Analysis Patterns: Reusable Object Models_.\nAddison-Wesley. 1997.\n* [[[Fowler]]] Fowler M. _Analysis Patterns: Reusable Object Models_.\nAddison-Wesley. 1997.\n");
            todo!("memory logger test");
        }

        #[test]
        #[ignore]
        // TO DO (https://github.com/asciidoc-rs/asciidoc-parser/issues/479):
        // Enable this test when bibliography parsing is enabled.
        fn should_automatically_add_bibliography_style_to_top_level_lists_in_bibliography_section()
        {
            let _doc = Parser::default().parse("[bibliography]\n== Bibliography\n\n.Books\n* [[[taoup]]] Eric Steven Raymond. _The Art of Unix\n  Programming_. Addison-Wesley. ISBN 0-13-142901-9.\n* [[[walsh-muellner]]] Norman Walsh & Leonard Muellner.\n  _DocBook - The Definitive Guide_. O'Reilly & Associates. 1999.\n  ISBN 1-56592-580-7.\n\n.Periodicals\n* [[[doc-writer]]] Doc Writer. _Documentation As Code_. Static Times, 54. August 2016.\n");
            todo!("document_from_string test");
        }

        #[test]
        #[ignore]
        // TO DO (https://github.com/asciidoc-rs/asciidoc-parser/issues/479):
        // Enable this test when bibliography parsing is enabled.
        fn should_not_recognize_bibliography_anchor_that_begins_with_a_digit() {
            let _doc = Parser::default().parse(
                "[bibliography]\n- [[[1984]]] George Orwell. _1984_. New American Library. 1950.\n",
            );
            todo!("assert_includes output, '[[[1984]]]'");
            todo!("assert_xpath: '//a[@id=\"1984\"]', output, 0");
        }

        #[test]
        #[ignore]
        // TO DO (https://github.com/asciidoc-rs/asciidoc-parser/issues/479):
        // Enable this test when bibliography parsing is enabled.
        fn should_recognize_bibliography_anchor_that_contains_a_digit_but_does_not_start_with_one()
        {
            let _doc = Parser::default().parse("[bibliography]\n- [[[_1984]]] George Orwell. __1984__. New American Library. 1950.\n");
            todo!("refute_includes output, '[[[_1984]]]'");
            todo!("assert_includes output, '[_1984]'");
            todo!("assert_xpath: '//a[@id=\"_1984\"]', output, 1");
        }

        #[test]
        #[ignore]
        // TO DO (https://github.com/asciidoc-rs/asciidoc-parser/issues/479):
        // Enable this test when bibliography parsing is enabled.
        fn should_catalog_bibliography_anchors_in_bibliography_list() {
            let _doc = Parser::default().parse("= Article Title\n\nPlease read <<Fowler_1997>>.\n\n[bibliography]\n== References\n\n* [[[Fowler_1997]]] Fowler M. _Analysis Patterns: Reusable Object Models_. Addison-Wesley. 1997.\n");
            todo!("document_from_string test");
        }

        #[test]
        #[ignore]
        // TO DO (https://github.com/asciidoc-rs/asciidoc-parser/issues/479):
        // Enable this test when bibliography parsing is enabled.
        fn should_use_reftext_from_bibliography_anchor_at_xref_and_entry() {
            let _doc = Parser::default().parse("= Article Title\n\nBegin with <<TMMM>>.\nThen move on to <<Fowler_1997>>.\n\n[bibliography]\n== References\n\n* [[[TMMM]]] Brooks F. _The Mythical Man-Month_. Addison-Wesley. 1975.\n* [[[Fowler_1997,1]]] Fowler M. _Analysis Patterns: Reusable Object Models_. Addison-Wesley. 1997.\n");
            todo!("document_from_string test");
        }

        // Backend-specific test omitted: DocBook.
    }
}

mod description_lists_redux {
    use crate::{Parser, tests::prelude::*};

    mod label_without_text_on_same_line {
        use super::*;

        #[test]
        fn folds_text_from_subsequent_line() {
            let doc = Parser::default().parse("== Lists\n\nterm1::\ndef1\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p[text()=\"def1\"]", 1);
        }

        #[test]
        fn folds_text_from_first_line_after_blank_lines() {
            let doc = Parser::default().parse("== Lists\n\nterm1::\n\n\ndef1\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p[text()=\"def1\"]", 1);
        }

        #[test]
        fn folds_text_from_first_line_after_blank_line_and_immediately_preceding_next_item() {
            let doc = Parser::default().parse("== Lists\n\nterm1::\n\ndef1\nterm2:: def2\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 2);
            assert_xpath(&doc, "(//*[@class=\"dlist\"]//dd)[1]/p[text()=\"def1\"]", 1);
        }

        #[test]
        fn paragraph_offset_by_blank_lines_does_not_break_list_if_label_does_not_have_inline_text()
        {
            let doc = Parser::default().parse("== Lists\n\nterm1::\n\ndef1\n\nterm2:: def2\n");

            assert_css(&doc, "dl", 1);
            assert_css(&doc, "dl > dt", 2);
            assert_css(&doc, "dl > dd", 2);

            assert_xpath(&doc, "(//dl/dd)[1]/p[text()=\"def1\"]", 1);
        }

        #[test]
        fn folds_text_from_first_line_after_comment_line() {
            let doc = Parser::default().parse("== Lists\n\nterm1::\n// comment\ndef1\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p[text()=\"def1\"]", 1);
        }

        #[test]
        fn folds_text_from_line_following_comment_line_offset_by_blank_line() {
            let doc = Parser::default().parse("== Lists\n\nterm1::\n\n// comment\ndef1\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p[text()=\"def1\"]", 1);
        }

        #[test]
        fn folds_text_from_subsequent_indented_line() {
            let doc = Parser::default().parse("== Lists\n\nterm1::\n  def1\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p[text()=\"def1\"]", 1);
        }

        #[test]
        fn folds_text_from_indented_line_after_blank_line() {
            let doc = Parser::default().parse("== Lists\n\nterm1::\n\n  def1\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p[text()=\"def1\"]", 1);
        }

        #[test]
        fn folds_text_that_looks_like_ruler_offset_by_blank_line() {
            let doc = Parser::default().parse("== Lists\n\nterm1::\n\n'''\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p[text()=\"'''\"]", 1);
        }

        #[test]
        #[ignore]
        // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/474):
        // Enable this test when rulers are implemented.
        fn folds_text_that_looks_like_ruler_offset_by_blank_line_and_line_comment() {
            let doc = Parser::default().parse("== Lists\n\nterm1::\n\n// comment\n'''\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p[text()=\"'''\"]", 1);
        }

        #[test]
        fn folds_text_that_looks_like_ruler_and_the_line_following_it_offset_by_blank_line() {
            let doc = Parser::default().parse("== Lists\n\nterm1::\n\n'''\ncontinued\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/p[normalize-space(text())=\"''' continued\"]",
                1,
            );
        }

        #[test]
        fn folds_text_that_looks_like_title_offset_by_blank_line() {
            let doc = Parser::default().parse("== Lists\n\nterm1::\n\n.def1\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p[text()=\".def1\"]", 1);
        }

        #[test]
        fn folds_text_that_looks_like_title_offset_by_blank_line_and_line_comment() {
            let doc = Parser::default().parse("== Lists\n\nterm1::\n\n// comment\n.def1\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p[text()=\".def1\"]", 1);
        }

        #[test]
        fn folds_text_that_looks_like_admonition_offset_by_blank_line() {
            let doc = Parser::default().parse("== Lists\n\nterm1::\n\nNOTE: def1\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/p[text()=\"NOTE: def1\"]",
                1,
            );
        }

        #[test]
        fn folds_text_that_looks_like_section_title_offset_by_blank_line() {
            let doc = Parser::default().parse("== Lists\n\nterm1::\n\n== Another Section\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/p[text()=\"== Another Section\"]",
                1,
            );

            assert_xpath(&doc, "//h2", 1);
        }

        #[test]
        fn folds_text_of_first_literal_line_offset_by_blank_line_appends_subsequent_literals_offset_by_blank_line_as_blocks()
         {
            let doc = Parser::default()
                .parse("== Lists\n\nterm1::\n\n  def1\n\n  literal\n\n\n  literal\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p[text()=\"def1\"]", 1);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/p/following-sibling::*[@class=\"literalblock\"]",
                2,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/p/following-sibling::*[@class=\"literalblock\"]//pre[text()=\"literal\"]",
                2,
            );
        }

        #[test]
        fn folds_text_of_subsequent_line_and_appends_following_literal_line_offset_by_blank_line_as_block_if_term_has_no_inline_description()
         {
            let doc =
                Parser::default().parse("== Lists\n\nterm1::\ndef1\n\n  literal\n\nterm2:: def2\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 2);
            assert_xpath(&doc, "(//*[@class=\"dlist\"]//dd)[1]/p[text()=\"def1\"]", 1);

            assert_xpath(
                &doc,
                "(//*[@class=\"dlist\"]//dd)[1]/p/following-sibling::*[@class=\"literalblock\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//*[@class=\"dlist\"]//dd)[1]/p/following-sibling::*[@class=\"literalblock\"]//pre[text()=\"literal\"]",
                1,
            );
        }

        #[test]
        fn appends_literal_line_attached_by_continuation_as_block_if_item_has_no_inline_description()
         {
            let doc = Parser::default().parse("== Lists\n\nterm1::\n+\n  literal\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p", 0);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/*[@class=\"literalblock\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/*[@class=\"literalblock\"]//pre[text()=\"literal\"]",
                1,
            );
        }

        #[test]
        fn appends_literal_line_attached_by_continuation_as_block_if_item_has_no_inline_description_followed_by_ruler()
         {
            let doc = Parser::default().parse("== Lists\n\nterm1::\n+\n  literal\n\n'''\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p", 0);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/*[@class=\"literalblock\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/*[@class=\"literalblock\"]//pre[text()=\"literal\"]",
                1,
            );

            assert_xpath(&doc, "//*[@class=\"dlist\"]/following-sibling::hr", 1);
        }

        #[test]
        #[ignore]
        // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/474):
        // Enable this test when rulers are implemented.
        fn appends_line_attached_by_continuation_as_block_if_item_has_no_inline_description_followed_by_ruler()
         {
            let doc = Parser::default().parse("== Lists\n\nterm1::\n+\npara\n\n'''\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p", 0);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/*[@class=\"paragraph\"]", 1);
            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/*[@class=\"paragraph\"]/p[text()=\"para\"]",
                1,
            );
            assert_xpath(&doc, "//*[@class=\"dlist\"]/following-sibling::hr", 1);
        }

        #[test]
        fn appends_line_attached_by_continuation_as_block_if_item_has_no_inline_description_followed_by_block()
         {
            let doc =
                Parser::default().parse("== Lists\n\nterm1::\n+\npara\n\n....\nliteral\n....\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p", 0);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/*[@class=\"paragraph\"]", 1);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/*[@class=\"paragraph\"]/p[text()=\"para\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]/following-sibling::*[@class=\"literalblock\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]/following-sibling::*[@class=\"literalblock\"]//pre[text()=\"literal\"]",
                1,
            );
        }

        #[test]
        fn appends_block_attached_by_continuation_but_not_subsequent_block_not_attached_by_continuation()
         {
            let doc = Parser::default()
                .parse("== Lists\n\nterm1::\n+\n....\nliteral\n....\n....\ndetached\n....\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p", 0);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/*[@class=\"literalblock\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/*[@class=\"literalblock\"]//pre[text()=\"literal\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]/following-sibling::*[@class=\"literalblock\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]/following-sibling::*[@class=\"literalblock\"]//pre[text()=\"detached\"]",
                1,
            );
        }

        #[test]
        fn appends_list_if_item_has_no_inline_description() {
            let doc = Parser::default().parse("== Lists\n\nterm1::\n\n* one\n* two\n* three\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p", 0);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd//ul/li", 3);
        }

        #[test]
        fn appends_list_to_first_term_when_followed_immediately_by_second_term() {
            let doc = Parser::default()
                .parse("== Lists\n\nterm1::\n\n* one\n* two\n* three\nterm2:: def2\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 2);
            assert_xpath(&doc, "(//*[@class=\"dlist\"]//dd)[1]/p", 0);
            assert_xpath(&doc, "(//*[@class=\"dlist\"]//dd)[1]//ul/li", 3);
            assert_xpath(&doc, "(//*[@class=\"dlist\"]//dd)[2]/p[text()=\"def2\"]", 1);
        }

        #[test]
        fn appends_indented_list_to_first_term_that_is_adjacent_to_second_term() {
            let doc = Parser::default().parse("== Lists\n\nlabel 1::\n  description 1\n\n  * one\n  * two\n  * three\nlabel 2::\n  description 2\n\nparagraph\n");

            assert_css(&doc, ".dlist > dl", 1);
            assert_css(&doc, ".dlist dt", 2);

            assert_xpath(
                &doc,
                "(//*[@class=\"dlist\"]//dt)[1][normalize-space(text())=\"label 1\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//*[@class=\"dlist\"]//dt)[2][normalize-space(text())=\"label 2\"]",
                1,
            );

            assert_css(&doc, ".dlist dd", 2);

            assert_xpath(
                &doc,
                "(//*[@class=\"dlist\"]//dd)[1]/p[text()=\"description 1\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//*[@class=\"dlist\"]//dd)[2]/p[text()=\"description 2\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//*[@class=\"dlist\"]//dd)[1]/p/following-sibling::*[@class=\"ulist\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//*[@class=\"dlist\"]//dd)[1]/p/following-sibling::*[@class=\"ulist\"]//li",
                3,
            );

            assert_css(&doc, ".dlist + .paragraph", 1);
        }

        #[test]
        fn appends_indented_list_to_first_term_that_is_attached_by_a_continuation_and_adjacent_to_second_term()
         {
            let doc = Parser::default().parse("== Lists\n\nlabel 1::\n  description 1\n+\n  * one\n  * two\n  * three\nlabel 2::\n  description 2\n\nparagraph\n");

            assert_css(&doc, ".dlist > dl", 1);
            assert_css(&doc, ".dlist dt", 2);

            assert_xpath(
                &doc,
                "(//*[@class=\"dlist\"]//dt)[1][normalize-space(text())=\"label 1\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//*[@class=\"dlist\"]//dt)[2][normalize-space(text())=\"label 2\"]",
                1,
            );

            assert_css(&doc, ".dlist dd", 2);

            assert_xpath(
                &doc,
                "(//*[@class=\"dlist\"]//dd)[1]/p[text()=\"description 1\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//*[@class=\"dlist\"]//dd)[2]/p[text()=\"description 2\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//*[@class=\"dlist\"]//dd)[1]/p/following-sibling::*[@class=\"ulist\"]",
                1,
            );

            assert_xpath(
                &doc,
                "(//*[@class=\"dlist\"]//dd)[1]/p/following-sibling::*[@class=\"ulist\"]//li",
                3,
            );

            assert_css(&doc, ".dlist + .paragraph", 1);
        }

        #[test]
        fn appends_list_and_paragraph_block_when_line_following_list_attached_by_continuation() {
            let doc = Parser::default()
                .parse("== Lists\n\nterm1::\n\n* one\n* two\n* three\n\n+\npara\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p", 0);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/*[@class=\"ulist\"]", 1);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/*[@class=\"ulist\"]/ul/li",
                3,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/*[@class=\"ulist\"]/following-sibling::*[@class=\"paragraph\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/*[@class=\"ulist\"]/following-sibling::*[@class=\"paragraph\"]/p[text()=\"para\"]",
                1,
            );
        }

        #[test]
        fn first_continued_line_associated_with_nested_list_item_and_second_continued_line_associated_with_term()
         {
            let doc = Parser::default()
                .parse("== Lists\n\nterm1::\n* one\n+\nnested list para\n\n+\nterm1 para\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p", 0);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/*[@class=\"ulist\"]", 1);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/*[@class=\"ulist\"]/ul/li",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/*[@class=\"ulist\"]/ul/li/*[@class=\"paragraph\"]/p[text()=\"nested list para\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/*[@class=\"ulist\"]/following-sibling::*[@class=\"paragraph\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/*[@class=\"ulist\"]/following-sibling::*[@class=\"paragraph\"]/p[text()=\"term1 para\"]",
                1,
            );
        }

        #[test]
        fn literal_line_attached_by_continuation_swallows_adjacent_line_that_looks_like_term() {
            let doc = Parser::default().parse("== Lists\n\nterm1::\n+\n  literal\nnotnestedterm:::\n+\n  literal\nnotnestedterm:::\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p", 0);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/*[@class=\"literalblock\"]",
                2,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/*[@class=\"literalblock\"]//pre[text()=\"  literal\nnotnestedterm:::\"]",
                2,
            );
        }

        #[test]
        fn line_attached_by_continuation_is_appended_as_paragraph_if_term_has_no_inline_description()
         {
            let doc = Parser::default().parse("== Lists\n\nterm1::\n+\npara\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p", 0);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/*[@class=\"paragraph\"]", 1);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/*[@class=\"paragraph\"]/p[text()=\"para\"]",
                1,
            );
        }

        #[test]
        fn attached_paragraph_does_not_break_on_adjacent_nested_description_list_term() {
            let doc =
                Parser::default().parse("term1:: def\n+\nmore description\nnot a term::: def\n");

            assert_css(&doc, ".dlist > dl > dt", 1);
            assert_css(&doc, ".dlist > dl > dd", 1);
            assert_css(&doc, ".dlist > dl > dd > .paragraph", 1);

            assert_output_contains(&doc, "not a term::: def");
        }

        #[test]
        fn attached_paragraph_is_terminated_by_adjacent_sibling_description_list_term() {
            // Comment ported from Ruby Asciidoctor tests:
            // FIXME: this is a negative test; the behavior should be the other way around.
            let doc =
                Parser::default().parse("term1:: def\n+\nmore description\nnot a term:: def\n");

            assert_css(&doc, ".dlist > dl > dt", 2);
            assert_css(&doc, ".dlist > dl > dd", 2);
            assert_css(&doc, ".dlist > dl > dd > .paragraph", 1);

            refute_output_contains(&doc, "not a term:: def");
        }

        #[test]
        fn attached_styled_paragraph_does_not_break_on_adjacent_nested_description_list_term() {
            let doc = Parser::default()
                .parse("term1:: def\n+\n[quote]\nmore description\nnot a term::: def\n");

            assert_css(&doc, ".dlist > dl > dt", 1);
            assert_css(&doc, ".dlist > dl > dd", 1);
            assert_css(&doc, ".dlist > dl > dd > .quoteblock", 1);

            assert_output_contains(&doc, "not a term::: def");
        }

        #[test]
        fn appends_line_as_paragraph_if_attached_by_continuation_following_blank_line_and_line_comment_when_term_has_no_inline_description()
         {
            let doc = Parser::default().parse("== Lists\n\nterm1::\n\n// comment\n+\npara\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p", 0);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/*[@class=\"paragraph\"]", 1);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/*[@class=\"paragraph\"]/p[text()=\"para\"]",
                1,
            );
        }

        #[test]
        fn line_attached_by_continuation_offset_by_blank_line_is_appended_as_paragraph_if_term_has_no_inline_description()
         {
            let doc = Parser::default().parse("== Lists\n\nterm1::\n\n+\npara\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p", 0);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/*[@class=\"paragraph\"]", 1);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/*[@class=\"paragraph\"]/p[text()=\"para\"]",
                1,
            );
        }

        #[test]
        fn delimited_block_breaks_list_even_when_term_has_no_inline_description() {
            let doc = Parser::default().parse("== Lists\n\nterm1::\n====\ndetached\n====\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 0);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]/following-sibling::*[@class=\"exampleblock\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]/following-sibling::*[@class=\"exampleblock\"]//p[text()=\"detached\"]",
                1,
            );
        }

        #[test]
        fn block_attribute_line_above_delimited_block_that_breaks_a_dlist_is_not_duplicated() {
            let doc = Parser::default()
                .parse("== Lists\n\nterm:: desc\n[.rolename]\n----\ndetached\n----\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]/following-sibling::*[@class=\"listingblock rolename\"]",
                1,
            );
        }

        #[test]
        fn block_attribute_line_above_paragraph_breaks_list_even_when_term_has_no_inline_description()
         {
            let doc = Parser::default().parse("== Lists\n\nterm1::\n[verse]\ndetached\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 0);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]/following-sibling::*[@class=\"verseblock\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]/following-sibling::*[@class=\"verseblock\"]/pre[text()=\"detached\"]",
                1,
            );
        }

        #[test]
        fn block_attribute_line_above_paragraph_that_breaks_a_dlist_is_not_duplicated() {
            let doc = Parser::default().parse("== Lists\n\nterm:: desc\n[.rolename]\ndetached\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]/following-sibling::*[@class=\"paragraph rolename\"]",
                1,
            );
        }

        #[test]
        fn block_anchor_line_breaks_list_even_when_term_has_no_inline_description() {
            let doc = Parser::default().parse("== Lists\n\nterm1::\n[[id]]\ndetached\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 0);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]/following-sibling::*[@class=\"paragraph\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]/following-sibling::*[@class=\"paragraph\"]/p[text()=\"detached\"]",
                1,
            );
        }

        #[test]
        fn block_attribute_lines_above_nested_horizontal_list_does_not_break_list() {
            let doc = Parser::default().parse("Operating Systems::\n[horizontal]\n  Linux::: Fedora\n  BSD::: OpenBSD\n\nCloud Providers::\n  PaaS::: OpenShift\n  IaaS::: AWS\n");

            assert_xpath(&doc, "//dl", 2);
            assert_xpath(&doc, "/*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "(//dl)[1]/dd", 2);
            assert_xpath(&doc, "((//dl)[1]/dd)[1]//table", 1);
            assert_xpath(&doc, "((//dl)[1]/dd)[2]//table", 0);
        }

        #[test]
        fn block_attribute_lines_above_nested_list_with_style_does_not_break_list() {
            let doc = Parser::default().parse("TODO List::\n* get groceries\nGrocery List::\n[square]\n* bread\n* milk\n* lettuce\n");

            assert_xpath(&doc, "//dl", 1);
            assert_xpath(&doc, "(//dl)[1]/dd", 2);
            assert_xpath(&doc, "((//dl)[1]/dd)[2]//ul[@class=\"square\"]", 1);
        }

        #[test]
        fn multiple_block_attribute_lines_above_nested_list_does_not_break_list() {
            let doc = Parser::default().parse("Operating Systems::\n[[variants]]\n[horizontal]\n  Linux::: Fedora\n  BSD::: OpenBSD\n\nCloud Providers::\n  PaaS::: OpenShift\n  IaaS::: AWS\n");

            assert_xpath(&doc, "//dl", 2);
            assert_xpath(&doc, "/*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "(//dl)[1]/dd", 2);
            assert_xpath(&doc, "(//dl)[1]/dd/*[@id=\"variants\"]", 1);
            assert_xpath(&doc, "((//dl)[1]/dd)[1]//table", 1);
            assert_xpath(&doc, "((//dl)[1]/dd)[2]//table", 0);
        }

        #[test]
        fn multiple_block_attribute_lines_separated_by_empty_line_above_nested_list_does_not_break_list()
         {
            let doc = Parser::default().parse("Operating Systems::\n[[variants]]\n\n[horizontal]\n\n  Linux::: Fedora\n  BSD::: OpenBSD\n\nCloud Providers::\n  PaaS::: OpenShift\n  IaaS::: AWS\n");

            assert_xpath(&doc, "//dl", 2);
            assert_xpath(&doc, "/*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "(//dl)[1]/dd", 2);
            assert_xpath(&doc, "(//dl)[1]/dd/*[@id=\"variants\"]", 1);
            assert_xpath(&doc, "((//dl)[1]/dd)[1]//table", 1);
            assert_xpath(&doc, "((//dl)[1]/dd)[2]//table", 0);
        }
    }

    mod item_with_text_inline {
        use super::*;

        #[test]
        fn folds_text_from_inline_description_and_subsequent_line() {
            let doc = Parser::default().parse("== Lists\n\nterm1:: def1\ncontinued\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/p[text()=\"def1\ncontinued\"]",
                1,
            );
        }

        #[test]
        fn folds_text_from_inline_description_and_subsequent_lines() {
            let doc = Parser::default().parse("== Lists\n\nterm1:: def1\ncontinued\ncontinued\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/p[text()=\"def1\ncontinued\ncontinued\"]",
                1,
            );
        }

        #[test]
        fn folds_text_from_inline_description_and_line_following_comment_line() {
            let doc = Parser::default().parse("== Lists\n\nterm1:: def1\n// comment\ncontinued\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/p[text()=\"def1\ncontinued\"]",
                1,
            );
        }

        #[test]
        fn folds_text_from_inline_description_and_subsequent_indented_line() {
            let doc = Parser::default().parse("== List\n\nterm1:: def1\n  continued\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/p[text()=\"def1\ncontinued\"]",
                1,
            );
        }

        #[test]
        fn appends_literal_line_offset_by_blank_line_as_block_if_item_has_inline_description() {
            let doc = Parser::default().parse("== Lists\n\nterm1:: def1\n\n  literal\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p[text()=\"def1\"]", 1);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/p/following-sibling::*[@class=\"literalblock\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/p/following-sibling::*[@class=\"literalblock\"]//pre[text()=\"literal\"]",
                1,
            );
        }

        #[test]
        fn appends_literal_line_offset_by_blank_line_as_block_and_appends_line_after_continuation_as_block_if_item_has_inline_description()
         {
            let doc = Parser::default().parse("== Lists\n\nterm1:: def1\n\n  literal\n+\npara\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p[text()=\"def1\"]", 1);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/p/following-sibling::*[@class=\"literalblock\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/p/following-sibling::*[@class=\"literalblock\"]//pre[text()=\"literal\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/*[@class=\"literalblock\"]/following-sibling::*[@class=\"paragraph\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/*[@class=\"literalblock\"]/following-sibling::*[@class=\"paragraph\"]/p[text()=\"para\"]",
                1,
            );
        }

        #[test]
        fn appends_line_after_continuation_as_block_and_literal_line_offset_by_blank_line_as_block_if_item_has_inline_description()
         {
            let doc = Parser::default().parse("== Lists\n\nterm1:: def1\n+\npara\n\n  literal\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p[text()=\"def1\"]", 1);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/p/following-sibling::*[@class=\"paragraph\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/p/following-sibling::*[@class=\"paragraph\"]/p[text()=\"para\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/*[@class=\"paragraph\"]/following-sibling::*[@class=\"literalblock\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/*[@class=\"paragraph\"]/following-sibling::*[@class=\"literalblock\"]//pre[text()=\"literal\"]",
                1,
            );
        }

        #[test]
        fn appends_list_if_item_has_inline_description() {
            let doc =
                Parser::default().parse("== Lists\n\nterm1:: def1\n\n* one\n* two\n* three\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p[text()=\"def1\"]", 1);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/p/following-sibling::*[@class=\"ulist\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/p/following-sibling::*[@class=\"ulist\"]/ul/li",
                3,
            );
        }

        #[test]
        #[ignore]
        // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/474):
        // Enable this test when rulers are implemented.
        fn appends_literal_line_attached_by_continuation_as_block_if_item_has_inline_description_followed_by_ruler()
         {
            let _doc = Parser::default().parse("== Lists\n\nterm1:: def1\n+\n  literal\n\n'''\n");
            todo!("assert_xpath: '//*[@class=\"dlist\"]/dl', output, 1");
            todo!("assert_xpath: '//*[@class=\"dlist\"]//dd', output, 1");
            todo!("assert_xpath: '//*[@class=\"dlist\"]//dd/p[text()=\"def1\"]', output, 1");
            todo!(
                "assert_xpath: '//*[@class=\"dlist\"]//dd/p/following-sibling::*[@class=\"literalblock\"]', output, 1"
            );
            todo!(
                "assert_xpath: '//*[@class=\"dlist\"]//dd/p/following-sibling::*[@class=\"literalblock\"]//pre[text()=\"literal\"]', output, 1"
            );
            todo!("assert_xpath: '//*[@class=\"dlist\"]/following-sibling::hr', output, 1");
        }

        #[test]
        fn line_offset_by_blank_line_breaks_list_if_term_has_inline_description() {
            let doc = Parser::default().parse("== Lists\n\nterm1:: def1\n\ndetached\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p[text()=\"def1\"]", 1);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]/following-sibling::*[@class=\"paragraph\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]/following-sibling::*[@class=\"paragraph\"]/p[text()=\"detached\"]",
                1,
            );
        }

        #[test]
        fn nested_term_with_description_does_not_consume_following_heading() {
            let doc = Parser::default()
                .parse("== Lists\n\nterm::\n  def\n  nestedterm;;\n    nesteddef\n\n=== Detached");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 2);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 2);
            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl//dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl//dl/dt", 1);

            assert_xpath(
                &doc,
                "((//*[@class=\"dlist\"])[1]//dd)[1]/p[text()=\"def\"]",
                1,
            );

            assert_xpath(
                &doc,
                "((//*[@class=\"dlist\"])[1]//dd)[1]/p/following-sibling::*[@class=\"dlist\"]",
                1,
            );

            assert_xpath(
                &doc,
                "((//*[@class=\"dlist\"])[1]//dd)[1]/p/following-sibling::*[@class=\"dlist\"]//dd/p[text()=\"nesteddef\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]/following-sibling::*[@class=\"sect2\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]/following-sibling::*[@class=\"sect2\"]/h3[text()=\"Detached\"]",
                1,
            );
        }

        #[test]
        fn line_attached_by_continuation_is_appended_as_paragraph_if_term_has_inline_description_followed_by_detached_paragraph()
         {
            let doc = Parser::default().parse("== Lists\n\nterm1:: def1\n+\npara\n\ndetached\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p[text()=\"def1\"]", 1);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/p/following-sibling::*[@class=\"paragraph\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/p/following-sibling::*[@class=\"paragraph\"]/p[text()=\"para\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]/following-sibling::*[@class=\"paragraph\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]/following-sibling::*[@class=\"paragraph\"]/p[text()=\"detached\"]",
                1,
            );
        }

        #[test]
        fn line_attached_by_continuation_is_appended_as_paragraph_if_term_has_inline_description_followed_by_detached_block()
         {
            let doc = Parser::default()
                .parse("== Lists\n\nterm1:: def1\n+\npara\n\n****\ndetached\n****\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p[text()=\"def1\"]", 1);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/p/following-sibling::*[@class=\"paragraph\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/p/following-sibling::*[@class=\"paragraph\"]/p[text()=\"para\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]/following-sibling::*[@class=\"sidebarblock\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]/following-sibling::*[@class=\"sidebarblock\"]//p[text()=\"detached\"]",
                1,
            );
        }

        #[test]
        fn line_attached_by_continuation_offset_by_line_comment_is_appended_as_paragraph_if_term_has_inline_description()
         {
            let doc = Parser::default().parse("== Lists\n\nterm1:: def1\n// comment\n+\npara\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p[text()=\"def1\"]", 1);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/p/following-sibling::*[@class=\"paragraph\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/p/following-sibling::*[@class=\"paragraph\"]/p[text()=\"para\"]",
                1,
            );
        }

        #[test]
        fn line_attached_by_continuation_offset_by_blank_line_is_appended_as_paragraph_if_term_has_inline_description()
         {
            let doc = Parser::default().parse("== Lists\n\nterm1:: def1\n\n+\npara\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd", 1);
            assert_xpath(&doc, "//*[@class=\"dlist\"]//dd/p[text()=\"def1\"]", 1);

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/p/following-sibling::*[@class=\"paragraph\"]",
                1,
            );

            assert_xpath(
                &doc,
                "//*[@class=\"dlist\"]//dd/p/following-sibling::*[@class=\"paragraph\"]/p[text()=\"para\"]",
                1,
            );
        }

        #[test]
        fn line_comment_offset_by_blank_line_divides_lists_because_item_has_text() {
            let doc = Parser::default().parse("== Lists\n\nterm1:: def1\n\n//\n\nterm2:: def2\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 2);
        }

        #[test]
        #[ignore]
        // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/474):
        // Enable this test when rulers are implemented.
        fn ruler_offset_by_blank_line_divides_lists_because_item_has_text() {
            let _doc = Parser::default().parse("== Lists\n\nterm1:: def1\n\n'''\n\nterm2:: def2\n");
            todo!("assert_xpath: '//*[@class=\"dlist\"]/dl', output, 2");
        }

        #[test]
        #[ignore]
        fn block_title_offset_by_blank_line_divides_lists_and_becomes_title_of_second_list_because_item_has_text()
         {
            let doc =
                Parser::default().parse("== Lists\n\nterm1:: def1\n\n.title\n\nterm2:: def2\n");

            assert_xpath(&doc, "//*[@class=\"dlist\"]/dl", 2);

            assert_xpath(
                &doc,
                "(//*[@class=\"dlist\"])[2]/*[@class=\"title\"][text()=\"title\"]",
                1,
            );
        }
    }
}

mod callout_lists {
    // TO DO (https://github.com/asciidoc-rs/asciidoc-parser/issues/311):
    // Implement these tests once callouts are implemented.
    use crate::Parser;

    #[test]
    #[ignore]
    fn does_not_recognize_callout_list_denoted_by_markers_that_only_have_a_trailing_bracket() {
        let _doc = Parser::default()
            .parse("----\nrequire 'asciidoctor' # <1>\n----\n1> Not a callout list item\n");
        todo!("assert_css: '.colist', output, 0");
    }

    #[test]
    #[ignore]
    fn should_not_hang_if_obsolete_callout_list_is_found_inside_list_item() {
        let _doc = Parser::default().parse("* foo\n1> bar\n");
        todo!("assert_css: '.colist', output, 0");
    }

    #[test]
    #[ignore]
    fn should_not_hang_if_obsolete_callout_list_is_found_inside_dlist_item() {
        let _doc = Parser::default().parse("foo::\n1> bar\n");
        todo!("assert_css: '.colist', output, 0");
    }

    #[test]
    #[ignore]
    fn should_recognize_auto_numberd_callout_list_inside_list() {
        let _doc =
            Parser::default().parse("----\nrequire 'asciidoctor' # <1>\n----\n* foo\n<.> bar\n");
        todo!("assert_css: '.colist', output, 1");
    }

    #[test]
    #[ignore]
    fn listing_block_with_sequential_callouts_followed_by_adjacent_callout_list() {
        // Backend-specific test omitted: DocBook.
    }

    #[test]
    #[ignore]
    fn listing_block_with_sequential_callouts_followed_by_non_adjacent_callout_list() {
        // Backend-specific test omitted: DocBook.
    }

    #[test]
    #[ignore]
    fn listing_block_with_a_callout_that_refers_to_two_different_lines() {
        // Backend-specific test omitted: DocBook.
    }

    #[test]
    #[ignore]
    fn source_block_with_non_sequential_callouts_followed_by_adjacent_callout_list() {
        // Backend-specific test omitted: DocBook.
    }

    #[test]
    #[ignore]
    fn two_listing_blocks_can_share_the_same_callout_list() {
        // Backend-specific test omitted: DocBook.
    }

    #[test]
    #[ignore]
    fn two_listing_blocks_each_followed_by_an_adjacent_callout_list() {
        // Backend-specific test omitted: DocBook.
    }

    #[test]
    #[ignore]
    fn callout_list_retains_block_content() {
        let _doc = Parser::default().parse("[source, ruby]\n----\nrequire 'asciidoctor' # <1>\ndoc = Asciidoctor::Document.new('Hello, World!') # <2>\nputs doc.convert # <3>\n----\n<1> Imports the library\nas a RubyGem\n<2> Creates a new document\n* Scans the lines for known blocks\n* Converts the lines into blocks\n<3> Renders the document\n+\nYou can write this to file rather than printing to stdout.\n");
        todo!("assert_xpath: '//ol/li', output, 3");
        todo!(
            "assert_xpath: '((//ol/li)[1]/p[text()=\"Imports the library\\nas a RubyGem\"])', output, 1"
        );
        todo!("assert_xpath: '((//ol/li)[2]//ul)', output, 1");
        todo!("assert_xpath: '((//ol/li)[2]//ul/li)', output, 2");
        todo!("assert_xpath: '((//ol/li)[3]//p)', output, 2");
    }

    #[test]
    #[ignore]
    fn callout_list_retains_block_content_when_converted_to_docbook() {
        // Backend-specific test omitted: DocBook.
    }

    #[test]
    #[ignore]
    fn escaped_callout_should_not_be_interpreted_as_a_callout() {
        let _doc = Parser::default().parse("[source,text]\n----\nrequire 'asciidoctor' # \\<1>\nAsciidoctor.convert 'convert me!' \\<2>\n----\n");
        todo!("assert_css: 'pre b', output, 0");
        todo!("assert_includes output, ' # &lt;1&gt;'");
        todo!("assert_includes output, ' &lt;2&gt;'");
    }

    #[test]
    #[ignore]
    fn should_autonumber_dot_callouts() {
        let _doc = Parser::default().parse("[source, ruby]\n----\nrequire 'asciidoctor' # <.>\ndoc = Asciidoctor::Document.new('Hello, World!') # <.>\nputs doc.convert # <.>\n----\n<.> Describe the first line\n<.> Describe the second line\n<.> Describe the third line\n");
        todo!("xmlnodes_at_css 'pre'");
        todo!("assert_includes pre_html, '(1)'");
        todo!("assert_includes pre_html, '(2)'");
        todo!("assert_includes pre_html, '(3)'");
        todo!("assert_css: '.colist ol', output, 1");
        todo!("assert_css: '.colist ol li', output, 3");
    }

    #[test]
    #[ignore]
    fn should_not_recognize_callouts_in_middle_of_line() {
        let _doc = Parser::default().parse("[source, ruby]\n----\nputs \"The syntax <1> at the end of the line makes a code callout\"\n----\n");
        todo!("assert_xpath: '//b', output, 0");
    }

    #[test]
    #[ignore]
    fn should_allow_multiple_callouts_on_the_same_line() {
        let _doc = Parser::default().parse("[source, ruby]\n----\nrequire 'asciidoctor' <1>\ndoc = Asciidoctor.load('Hello, World!') # <2> <3> <4>\nputs doc.convert <5><6>\nexit 0\n----\n<1> Require library\n<2> Load document from String\n<3> Uses default backend and doctype\n<4> One more for good luck\n<5> Renders document to String\n<6> Prints output to stdout\n");
        todo!("assert_xpath: '//code/b', output, 6");
        todo!("assert_match: / <b class=\"conum\">\\(1\\)<\\/b>$/");
        todo!(
            "assert_match: / <b class=\"conum\">\\(2\\)<\\/b> <b class=\"conum\">\\(3\\)<\\/b> <b class=\"conum\">\\(4\\)<\\/b>$/"
        );
        todo!("assert_match: / <b class=\"conum\">\\(5\\)<\\/b><b class=\"conum\">\\(6\\)<\\/b>$/");
    }

    #[test]
    #[ignore]
    fn should_allow_xml_comment_style_callouts() {
        let _doc = Parser::default().parse("[source, xml]\n----\n<section>\n  <title>Section Title</title> <!--1-->\n  <simpara>Just a paragraph</simpara> <!--2-->\n</section>\n----\n<1> The title is required\n<2> The content isn't\n");
        todo!("assert_xpath: '//b', output, 2");
        todo!("assert_xpath: '//b[text()=\"(1)\"]', output, 1");
        todo!("assert_xpath: '//b[text()=\"(2)\"]', output, 1");
    }

    #[test]
    #[ignore]
    fn should_not_allow_callouts_with_half_an_xml_comment() {
        let _doc = Parser::default().parse("----\nFirst line <1-->\nSecond line <2-->\n----\n");
        todo!("assert_xpath: '//b', output, 0");
    }

    #[test]
    #[ignore]
    fn should_not_recognize_callouts_in_an_indented_description_list_paragraph() {
        let _doc = Parser::default().parse("foo::\n  bar <1>\n\n<1> Not pointing to a callout\n");
        todo!("memory logger test");
        todo!("assert_xpath: '//dl//b', output, 0");
        todo!("assert_xpath: '//dl/dd/p[text()=\"bar <1>\"]', output, 1");
        todo!("assert_xpath: '//ol/li/p[text()=\"Not pointing to a callout\"]', output, 1");
        todo!("assert_message logger, :WARN, '<stdin>: line 4: no callout found for <1>'");
    }

    #[test]
    #[ignore]
    fn should_not_recognize_callouts_in_an_indented_outline_list_paragraph() {
        let _doc = Parser::default().parse("* foo\n  bar <1>\n\n<1> Not pointing to a callout\n");
        todo!("memory logger test");
        todo!("assert_xpath: '//ul//b', output, 0");
        todo!("assert_xpath: '//ul/li/p[text()=\"foo\\nbar <1>\"]', output, 1");
        todo!("assert_xpath: '//ol/li/p[text()=\"Not pointing to a callout\"]', output, 1");
        todo!("assert_message logger, :WARN, '<stdin>: line 4: no callout found for <1>'");
    }

    #[test]
    #[ignore]
    fn should_warn_if_numbers_in_callout_list_are_out_of_sequence() {
        let _doc = Parser::default().parse("----\n<beans> <1>\n  <bean class=\"com.example.HelloWorld\"/>\n</beans>\n----\n<1> Container of beans.\nBeans are fun.\n<3> An actual bean.\n");
        todo!("memory logger test");
        todo!("assert_xpath: '//ol/li', output, 2");
        todo!(
            "assert_messages logger, [[:WARN, '<stdin>: line 8: callout list item index: expected 2, got 3'], [:WARN, '<stdin>: line 8: no callout found for <2>']]"
        );
    }

    #[test]
    #[ignore]
    fn should_preserve_line_comment_chars_that_precede_callout_number_if_icons_is_not_set() {
        let _doc = Parser::default().parse("[source,ruby]\n----\nputs 'Hello, world!' # <1>\n----\n<1> Ruby\n\n[source,groovy]\n----\nprintln 'Hello, world!' // <1>\n----\n<1> Groovy\n\n[source,clojure]\n----\n(def hello (fn [] \"Hello, world!\")) ;; <1>\n(hello)\n----\n<1> Clojure\n\n[source,haskell]\n----\nmain = putStrLn \"Hello, World!\" -- <1>\n----\n<1> Haskell\n");
        todo!("xmlnodes_at_css 'pre'");
        todo!("assert_xpath: '//b', output, 4");
        todo!("assert_equal: nodes[0].text, 'puts 'Hello, world!' # (1)'");
        todo!("assert_equal: nodes[1].text, 'println 'Hello, world!' // (1)'");
        todo!(
            "assert_equal: nodes[2].text, '(def hello (fn [] \"Hello, world!\")) ;; (1)\\n(hello)'"
        );
        todo!("assert_equal: nodes[3].text, 'main = putStrLn \"Hello, World!\" -- (1)'");
    }

    #[test]
    #[ignore]
    fn should_remove_line_comment_chars_that_precede_callout_number_if_icons_is_font() {
        let _doc = Parser::default().parse("[source,ruby]\n----\nputs 'Hello, world!' # <1>\n----\n<1> Ruby\n\n[source,groovy]\n----\nprintln 'Hello, world!' // <1>\n----\n<1> Groovy\n\n[source,clojure]\n----\n(def hello (fn [] \"Hello, world!\")) ;; <1>\n(hello)\n----\n<1> Clojure\n\n[source,haskell]\n----\nmain = putStrLn \"Hello, World!\" -- <1>\n----\n<1> Haskell\n");
        todo!("xmlnodes_at_css 'pre'");
        todo!("assert_css: 'pre b', output, 4");
        todo!("assert_css: 'pre i.conum', output, 4");
        todo!("assert_equal: nodes[0].text, 'puts 'Hello, world!' (1)'");
        todo!("assert_equal: nodes[1].text, 'println 'Hello, world!' (1)'");
        todo!("assert_equal: nodes[2].text, '(def hello (fn [] \"Hello, world!\")) (1)\\n(hello)'");
        todo!("assert_equal: nodes[3].text, 'main = putStrLn \"Hello, World!\" (1)'");
    }

    #[test]
    #[ignore]
    fn should_allow_line_comment_chars_that_precede_callout_number_to_be_specified() {
        let _doc = Parser::default().parse("[source,erlang,line-comment=%]\n----\nhello_world() -> % <1>\n  io:fwrite(\"hello, world~n\"). %<2>\n----\n<1> Erlang function clause head.\n<2> ~n adds a new line to the output.\n");
        todo!("xmlnodes_at_css 'pre'");
        todo!("assert_xpath: '//b', output, 2");
        todo!(
            "assert_equal: nodes[0].text, 'hello_world() -> % (1)\\n  io:fwrite(\"hello, world~n\"). %(2)'"
        );
    }

    #[test]
    #[ignore]
    fn should_allow_line_comment_chars_preceding_callout_number_to_be_configurable_when_source_highlighter_is_coderay()
     {
        let _doc = Parser::default().parse("[source,html,line-comment=-#]\n----\n-# <1>\n%p Hello\n----\n<1> Prints a paragraph with the text \"Hello\"\n");
        todo!("xmlnodes_at_css 'pre'");
        todo!("assert_xpath: '//b', output, 1");
        todo!("assert_equal: nodes[0].text, '-# (1)\\n%p Hello'");
    }

    #[test]
    #[ignore]
    fn should_not_eat_whitespace_before_callout_number_if_line_comment_attribute_is_empty() {
        let _doc = Parser::default().parse("[source,asciidoc,line-comment=]\n----\n-- <1>\n----\n<1> The start of an open block.\n");
        todo!("assert_includes output, '-- <i class=\"conum\"'");
    }

    #[test]
    #[ignore]
    fn literal_block_with_callouts() {
        // Backend-specific test omitted: DocBook.
    }

    #[test]
    #[ignore]
    fn callout_list_with_icons_enabled() {
        let _doc = Parser::default().parse("[source, ruby]\n----\nrequire 'asciidoctor' # <1>\ndoc = Asciidoctor::Document.new('Hello, World!') # <2>\nputs doc.convert # <3>\n----\n<1> Describe the first line\n<2> Describe the second line\n<3> Describe the third line\n");
        todo!("assert_css: '.listingblock code > img', output, 3");
        todo!(
            "assert_xpath: '(/div[@class=\"listingblock\"]//code/img)[1][@src=\"./images/icons/callouts/1.png\"][@alt=\"1\"]', output, 1"
        );
        todo!("additional assertions");
        todo!("assert_css: '.colist table td img', output, 3");
        todo!(
            "assert_xpath: '(/div[@class=\"colist arabic\"]//td/img)[1][@src=\"./images/icons/callouts/1.png\"][@alt=\"1\"]', output, 1"
        );
        todo!("additional assertions");
    }

    #[test]
    #[ignore]
    fn callout_list_with_font_based_icons_enabled() {
        let _doc = Parser::default().parse("[source]\n----\nrequire 'asciidoctor' # <1>\ndoc = Asciidoctor::Document.new('Hello, World!') #<2>\nputs doc.convert #<3>\n----\n<1> Describe the first line\n<2> Describe the second line\n<3> Describe the third line\n");
        todo!("assert_css: '.listingblock code > i', output, 3");
        todo!("assert_xpath: '(/div[@class=\"listingblock\"]//code/i)[1]', output, 1");
        todo!(
            "assert_xpath: '(/div[@class=\"listingblock\"]//code/i)[1][@class=\"conum\"][@data-value=\"1\"]', output, 1"
        );
        todo!(
            "assert_xpath: '(/div[@class=\"listingblock\"]//code/i)[1]/following-sibling::b[text()=\"(1)\"]', output, 1"
        );
        todo!("additional assertions");
        todo!("assert_css: '.colist table td i', output, 3");
        todo!("assert_xpath: '(/div[@class=\"colist arabic\"]//td/i)[1]', output, 1");
        todo!(
            "assert_xpath: '(/div[@class=\"colist arabic\"]//td/i)[1][@class=\"conum\"][@data-value=\"1\"]', output, 1"
        );
        todo!(
            "assert_xpath: '(/div[@class=\"colist arabic\"]//td/i)[1]/following-sibling::b[text()=\"1\"]', output, 1"
        );
        todo!("additional assertions");
    }

    #[test]
    #[ignore]
    fn should_match_trailing_line_separator_in_text_of_list_item() {
        let _doc =
            Parser::default().parse("----\nA <1>\nB <2>\nC <3>\n----\n<1> a\n<2> b\u{2028}\n<3> c");
        todo!("Unicode line separator (U+2028) in test input");
        todo!("assert_css: 'li', output, 3");
        todo!("assert_xpath: '((//li)[2]/p[text()=\"b\u{2028}\"])', output, 1");
    }

    #[test]
    #[ignore]
    fn should_match_line_separator_in_text_of_list_item() {
        let _doc = Parser::default()
            .parse("----\nA <1>\nB <2>\nC <3>\n----\n<1> a\n<2> b\u{2028}b\n<3> c");
        todo!("Unicode line separator (U+2028) in test input");
        todo!("assert_css: 'li', output, 3");
        todo!("assert_xpath: '((//li)[2]/p[text()=\"b\u{2028}b\"])', output, 1");
    }
}

mod checklists {
    // TO DO (https://github.com/asciidoc-rs/asciidoc-parser/issues/481):
    // Enable these tests when checklists are implemented.
    use crate::Parser;

    #[test]
    #[ignore]
    fn should_create_checklist_if_at_least_one_item_has_checkbox_syntax() {
        let _doc = Parser::default()
            .parse("- [ ] todo\n- [x] done\n- [ ] another todo\n- [*] another done\n- plain\n");
        todo!("document_from_string test");
    }

    #[test]
    #[ignore]
    fn entry_is_not_a_checklist_item_if_the_closing_bracket_is_not_immediately_followed_by_the_space_character()
     {
        let _doc = Parser::default()
            .parse("- [ ]    todo\n- [x] \t done\n- [ ]\t  another todo\n- [x]\t  another done\n");
        todo!("document_from_string test");
    }

    #[test]
    #[ignore]
    fn should_create_checklist_with_font_icons_if_at_least_one_item_has_checkbox_syntax_and_icons_attribute_is_font()
     {
        let _doc = Parser::default().parse("- [ ] todo\n- [x] done\n- plain\n");
        todo!("assert_css: '.ulist.checklist', output, 1");
        todo!("assert_css: '.ulist.checklist li i.fa-check-square-o', output, 1");
        todo!("assert_css: '.ulist.checklist li i.fa-square-o', output, 1");
        todo!(
            "assert_xpath: '(/*[@class=\"ulist checklist\"]/ul/li)[3]/p[text()=\"plain\"]', output, 1"
        );
    }

    #[test]
    #[ignore]
    fn should_create_interactive_checklist_if_interactive_option_is_set_even_with_icons_attribute_is_font()
     {
        let _doc =
            Parser::default().parse(":icons: font\n\n[%interactive]\n- [ ] todo\n- [x] done\n");
        todo!("document_from_string test");
    }

    #[test]
    #[ignore]
    fn should_not_create_checklist_if_checkbox_on_item_is_followed_by_a_tab() {
        let _doc = Parser::default().parse("- [ ]\ttodo\n");
        todo!("document_from_string test");
    }
}

mod lists_model {
    use crate::{
        Parser,
        blocks::{ContentModel, ListType, SimpleBlockStyle},
        content::SubstitutionGroup,
        tests::prelude::*,
    };

    #[test]
    fn content_should_return_items_in_list() {
        let doc = Parser::default().parse("* one\n* two\n* three\n");

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
                blocks: &[Block::List(ListBlock {
                    type_: ListType::Unordered,
                    items: &[
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Asterisks(Span {
                                data: "*",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "one",
                                        line: 1,
                                        col: 3,
                                        offset: 2,
                                    },
                                    rendered: "one",
                                },
                                source: Span {
                                    data: "one",
                                    line: 1,
                                    col: 3,
                                    offset: 2,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: "* one",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Asterisks(Span {
                                data: "*",
                                line: 2,
                                col: 1,
                                offset: 6,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "two",
                                        line: 2,
                                        col: 3,
                                        offset: 8,
                                    },
                                    rendered: "two",
                                },
                                source: Span {
                                    data: "two",
                                    line: 2,
                                    col: 3,
                                    offset: 8,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: "* two",
                                line: 2,
                                col: 1,
                                offset: 6,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Asterisks(Span {
                                data: "*",
                                line: 3,
                                col: 1,
                                offset: 12,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "three",
                                        line: 3,
                                        col: 3,
                                        offset: 14,
                                    },
                                    rendered: "three",
                                },
                                source: Span {
                                    data: "three",
                                    line: 3,
                                    col: 3,
                                    offset: 14,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            source: Span {
                                data: "* three",
                                line: 3,
                                col: 1,
                                offset: 12,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                    ],
                    source: Span {
                        data: "* one\n* two\n* three",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: "* one\n* two\n* three",
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
    fn list_item_should_be_the_parent_of_block_attached_to_a_list_item() {
        let doc =
            Parser::default().parse("* list item 1\n+\n----\nlisting block in list item 1\n----\n");

        assert_eq!(
            &doc,
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
                blocks: &[Block::List(ListBlock {
                    type_: ListType::Unordered,
                    items: &[Block::ListItem(ListItem {
                        marker: ListItemMarker::Asterisks(Span {
                            data: "*",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },),
                        blocks: &[
                            Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "list item 1",
                                        line: 1,
                                        col: 3,
                                        offset: 2,
                                    },
                                    rendered: "list item 1",
                                },
                                source: Span {
                                    data: "list item 1",
                                    line: 1,
                                    col: 3,
                                    offset: 2,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),
                            Block::RawDelimited(RawDelimitedBlock {
                                content: Content {
                                    original: Span {
                                        data: "listing block in list item 1",
                                        line: 4,
                                        col: 1,
                                        offset: 21,
                                    },
                                    rendered: "listing block in list item 1",
                                },
                                content_model: ContentModel::Verbatim,
                                context: "listing",
                                source: Span {
                                    data: "----\nlisting block in list item 1\n----",
                                    line: 3,
                                    col: 1,
                                    offset: 16,
                                },
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                                substitution_group: SubstitutionGroup::Verbatim,
                            },),
                        ],
                        source: Span {
                            data: "* list item 1\n+\n----\nlisting block in list item 1\n----",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    source: Span {
                        data: "* list item 1\n+\n----\nlisting block in list item 1\n----",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: "* list item 1\n+\n----\nlisting block in list item 1\n----",
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

    // Skip tests related to outline? and simple? methods in Ruby interface.

    // Skip test describing mutability of data model. The Rust parser's data
    // structures are intentionally immutable.

    #[test]
    fn should_set_lineno_to_line_number_in_source_where_list_starts() {
        let doc =
            Parser::default().parse("* bullet 1\n** bullet 1.1\n*** bullet 1.1.1\n* bullet 2\n");

        assert_eq!(
            &doc,
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
                blocks: &[Block::List(ListBlock {
                    type_: ListType::Unordered,
                    items: &[
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Asterisks(Span {
                                data: "*",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },),
                            blocks: &[
                                Block::Simple(SimpleBlock {
                                    content: Content {
                                        original: Span {
                                            data: "bullet 1",
                                            line: 1,
                                            col: 3,
                                            offset: 2,
                                        },
                                        rendered: "bullet 1",
                                    },
                                    source: Span {
                                        data: "bullet 1",
                                        line: 1,
                                        col: 3,
                                        offset: 2,
                                    },
                                    style: SimpleBlockStyle::Paragraph,
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                                Block::List(ListBlock {
                                    type_: ListType::Unordered,
                                    items: &[Block::ListItem(ListItem {
                                        marker: ListItemMarker::Asterisks(Span {
                                            data: "**",
                                            line: 2,
                                            col: 1,
                                            offset: 11,
                                        },),
                                        blocks: &[
                                            Block::Simple(SimpleBlock {
                                                content: Content {
                                                    original: Span {
                                                        data: "bullet 1.1",
                                                        line: 2,
                                                        col: 4,
                                                        offset: 14,
                                                    },
                                                    rendered: "bullet 1.1",
                                                },
                                                source: Span {
                                                    data: "bullet 1.1",
                                                    line: 2,
                                                    col: 4,
                                                    offset: 14,
                                                },
                                                style: SimpleBlockStyle::Paragraph,
                                                title_source: None,
                                                title: None,
                                                anchor: None,
                                                anchor_reftext: None,
                                                attrlist: None,
                                            },),
                                            Block::List(ListBlock {
                                                type_: ListType::Unordered,
                                                items: &[Block::ListItem(ListItem {
                                                    marker: ListItemMarker::Asterisks(Span {
                                                        data: "***",
                                                        line: 3,
                                                        col: 1,
                                                        offset: 25,
                                                    },),
                                                    blocks: &[Block::Simple(SimpleBlock {
                                                        content: Content {
                                                            original: Span {
                                                                data: "bullet 1.1.1",
                                                                line: 3,
                                                                col: 5,
                                                                offset: 29,
                                                            },
                                                            rendered: "bullet 1.1.1",
                                                        },
                                                        source: Span {
                                                            data: "bullet 1.1.1",
                                                            line: 3,
                                                            col: 5,
                                                            offset: 29,
                                                        },
                                                        style: SimpleBlockStyle::Paragraph,
                                                        title_source: None,
                                                        title: None,
                                                        anchor: None,
                                                        anchor_reftext: None,
                                                        attrlist: None,
                                                    },),],
                                                    source: Span {
                                                        data: "*** bullet 1.1.1",
                                                        line: 3,
                                                        col: 1,
                                                        offset: 25,
                                                    },
                                                    anchor: None,
                                                    anchor_reftext: None,
                                                    attrlist: None,
                                                },),],
                                                source: Span {
                                                    data: "*** bullet 1.1.1",
                                                    line: 3,
                                                    col: 1,
                                                    offset: 25,
                                                },
                                                title_source: None,
                                                title: None,
                                                anchor: None,
                                                anchor_reftext: None,
                                                attrlist: None,
                                            },),
                                        ],
                                        source: Span {
                                            data: "** bullet 1.1\n*** bullet 1.1.1",
                                            line: 2,
                                            col: 1,
                                            offset: 11,
                                        },
                                        anchor: None,
                                        anchor_reftext: None,
                                        attrlist: None,
                                    },),],
                                    source: Span {
                                        data: "** bullet 1.1\n*** bullet 1.1.1",
                                        line: 2,
                                        col: 1,
                                        offset: 11,
                                    },
                                    title_source: None,
                                    title: None,
                                    anchor: None,
                                    anchor_reftext: None,
                                    attrlist: None,
                                },),
                            ],
                            source: Span {
                                data: "* bullet 1\n** bullet 1.1\n*** bullet 1.1.1",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::ListItem(ListItem {
                            marker: ListItemMarker::Asterisks(Span {
                                data: "*",
                                line: 4,
                                col: 1,
                                offset: 42,
                            },),
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "bullet 2",
                                        line: 4,
                                        col: 3,
                                        offset: 44,
                                    },
                                    rendered: "bullet 2",
                                },
                                source: Span {
                                    data: "bullet 2",
                                    line: 4,
                                    col: 3,
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
                                data: "* bullet 2",
                                line: 4,
                                col: 1,
                                offset: 42,
                            },
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                    ],
                    source: Span {
                        data: "* bullet 1\n** bullet 1.1\n*** bullet 1.1.1\n* bullet 2",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: "* bullet 1\n** bullet 1.1\n*** bullet 1.1.1\n* bullet 2",
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
