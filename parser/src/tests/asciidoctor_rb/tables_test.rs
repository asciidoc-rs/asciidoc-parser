// Adapted from Asciidoctor's tables test suite, found in
// https://github.com/asciidoctor/asciidoctor/blob/main/test/tables_test.rb.
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
//! Port of Asciidoctor's `tables_test.rb`.
//!
//! Each Ruby `context` becomes a Rust `mod` and each `test '..'` becomes a
//! `#[test] fn`, driven through [`Parser`](crate::Parser) and asserted with the
//! `assert_*` DOM helpers. Every ported `test '..'` block is reproduced
//! verbatim in a `verifies!` block so the `sdd` coverage tool can measure which
//! lines of the Ruby suite are verified line-by-line.
//!
//! DocBook and other non-HTML backends are out of scope for this crate, as is
//! compatibility mode and the discouraged `{set:cellbgcolor:..}`
//! cell-background syntax. Ruby tests exercising those are reproduced verbatim
//! in `non_normative!` blocks (accounting for those Ruby lines without
//! asserting behavior this crate does not model); each carries a comment noting
//! why it is not ported. A few `#[test] fn`s extend the Ruby coverage with
//! crate-specific cases and so carry no `verifies!` block.

use crate::tests::sdd::*;

track_file!("ref/asciidoctor/test/tables_test.rb");

non_normative!(
    r#"
# frozen_string_literal: true
require_relative 'test_helper'

context 'Tables' do
"#
);

mod psv {
    use crate::tests::prelude::{inline_file_handler::InlineFileHandler, *};

    non_normative!(
        r#"
  context 'PSV' do
"#
    );

    #[test]
    fn converts_simple_psv_table() {
        verifies!(
            r#"
    test 'converts simple psv table' do
      input = <<~'EOS'
      |=======
      |A |B |C
      |a |b |c
      |1 |2 |3
      |=======
      EOS
      cells = [%w(A B C), %w(a b c), %w(1 2 3)]
      doc = document_from_string input, standalone: false
      table = doc.blocks[0]
      assert_equal 100, table.columns.map {|col| col.attributes['colpcwidth'] }.reduce(:+)
      output = doc.convert
      assert_css 'table', output, 1
      assert_css 'table.tableblock.frame-all.grid-all.stretch', output, 1
      assert_css 'table > colgroup > col[style*="width: 33.3333%"]', output, 2
      assert_css 'table > colgroup > col:last-of-type[style*="width: 33.3334%"]', output, 1
      assert_css 'table tr', output, 3
      assert_css 'table > tbody > tr', output, 3
      assert_css 'table td', output, 9
      assert_css 'table > tbody > tr > td.tableblock.halign-left.valign-top > p.tableblock', output, 9
      cells.each_with_index do |row, rowi|
        assert_css "table > tbody > tr:nth-child(#{rowi + 1}) > td", output, row.size
        assert_css "table > tbody > tr:nth-child(#{rowi + 1}) > td > p", output, row.size
        row.each_with_index do |cell, celli|
          assert_xpath "(//tr)[#{rowi + 1}]/td[#{celli + 1}]/p[text()='#{cell}']", output, 1
        end
      end
    end

"#
        );

        let doc = Parser::default().parse("|=======\n|A |B |C\n|a |b |c\n|1 |2 |3\n|=======");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table.tableblock.frame-all.grid-all.stretch", 1);
        assert_css(&doc, "table > colgroup > col[width=\"33.3333%\"]", 2);
        // Ruby uses `col:last-of-type`; the indexed XPath form is equivalent.
        assert_xpath(&doc, "(/table/colgroup/col)[3][@width=\"33.3334%\"]", 1);
        assert_css(&doc, "table tr", 3);
        assert_css(&doc, "table > tbody > tr", 3);
        assert_css(&doc, "table td", 9);
        assert_css(
            &doc,
            "table > tbody > tr > td.tableblock.halign-left.valign-top > p.tableblock",
            9,
        );

        let cells = [["A", "B", "C"], ["a", "b", "c"], ["1", "2", "3"]];
        for (rowi, row) in cells.iter().enumerate() {
            assert_xpath(
                &doc,
                &format!("(/table/tbody/tr)[{}]/td", rowi + 1),
                row.len(),
            );
            assert_xpath(
                &doc,
                &format!("(/table/tbody/tr)[{}]/td/p", rowi + 1),
                row.len(),
            );
            for (celli, cell) in row.iter().enumerate() {
                assert_xpath(
                    &doc,
                    &format!("(//tr)[{}]/td[{}]/p[text()='{cell}']", rowi + 1, celli + 1),
                    1,
                );
            }
        }
    }

    #[test]
    fn should_add_direction_css_class_if_float_attribute_is_set_on_table() {
        verifies!(
            r#"
    test 'should add direction CSS class if float attribute is set on table' do
      input = <<~'EOS'
      [float=left]
      |=======
      |A |B |C
      |a |b |c
      |1 |2 |3
      |=======
      EOS

      output = convert_string_to_embedded input
      assert_css 'table.left', output, 1
    end

"#
        );

        let doc = Parser::default()
            .parse("[float=left]\n|=======\n|A |B |C\n|a |b |c\n|1 |2 |3\n|=======");

        assert_css(&doc, "table.left", 1);
    }

    #[test]
    fn should_set_stripes_class_if_stripes_option_is_set() {
        verifies!(
            r#"
    test 'should set stripes class if stripes option is set' do
      input = <<~'EOS'
      [stripes=odd]
      |=======
      |A |B |C
      |a |b |c
      |1 |2 |3
      |=======
      EOS

      output = convert_string_to_embedded input
      assert_css 'table.stripes-odd', output, 1
    end

"#
        );

        let doc = Parser::default()
            .parse("[stripes=odd]\n|=======\n|A |B |C\n|a |b |c\n|1 |2 |3\n|=======");

        assert_css(&doc, "table.stripes-odd", 1);
    }

    #[test]
    fn outputs_a_caption_on_simple_psv_table() {
        verifies!(
            r#"
    test 'outputs a caption on simple psv table' do
      input = <<~'EOS'
      .Simple psv table
      |=======
      |A |B |C
      |a |b |c
      |1 |2 |3
      |=======
      EOS
      output = convert_string_to_embedded input
      assert_xpath '/table/caption[@class="title"][text()="Table 1. Simple psv table"]', output, 1
      assert_xpath '/table/caption/following-sibling::colgroup', output, 1
    end

"#
        );

        let doc = Parser::default()
            .parse(".Simple psv table\n|=======\n|A |B |C\n|a |b |c\n|1 |2 |3\n|=======");

        assert_xpath(
            &doc,
            "/table/caption[@class=\"title\"][text()=\"Table 1. Simple psv table\"]",
            1,
        );
        assert_xpath(&doc, "/table/caption/following-sibling::colgroup", 1);
    }

    #[test]
    fn only_increments_table_counter_for_tables_that_have_a_title() {
        verifies!(
            r#"
    test 'only increments table counter for tables that have a title' do
      input = <<~'EOS'
      .First numbered table
      |=======
      |1 |2 |3
      |=======

      |=======
      |4 |5 |6
      |=======

      .Second numbered table
      |=======
      |7 |8 |9
      |=======
      EOS
      output = convert_string_to_embedded input
      assert_css 'table:root', output, 3
      assert_xpath '(/table)[1]/caption', output, 1
      assert_xpath '(/table)[1]/caption[text()="Table 1. First numbered table"]', output, 1
      assert_xpath '(/table)[2]/caption', output, 0
      assert_xpath '(/table)[3]/caption', output, 1
      assert_xpath '(/table)[3]/caption[text()="Table 2. Second numbered table"]', output, 1
    end

"#
        );

        let doc = Parser::default().parse(
            ".First numbered table\n|=======\n|1 |2 |3\n|=======\n\n|=======\n|4 |5 |6\n|=======\n\n.Second numbered table\n|=======\n|7 |8 |9\n|=======",
        );

        assert_xpath(&doc, "/table", 3);
        assert_xpath(&doc, "(/table)[1]/caption", 1);
        assert_xpath(
            &doc,
            "(/table)[1]/caption[text()=\"Table 1. First numbered table\"]",
            1,
        );
        assert_xpath(&doc, "(/table)[2]/caption", 0);
        assert_xpath(&doc, "(/table)[3]/caption", 1);
        assert_xpath(
            &doc,
            "(/table)[3]/caption[text()=\"Table 2. Second numbered table\"]",
            1,
        );
    }

    #[test]
    fn uses_explicit_caption_in_front_of_title_in_place_of_default_caption_and_number() {
        verifies!(
            r#"
    test 'uses explicit caption in front of title in place of default caption and number' do
      input = <<~'EOS'
      [caption="All the Data. "]
      .Simple psv table
      |=======
      |A |B |C
      |a |b |c
      |1 |2 |3
      |=======
      EOS
      output = convert_string_to_embedded input
      assert_xpath '/table/caption[@class="title"][text()="All the Data. Simple psv table"]', output, 1
      assert_xpath '/table/caption/following-sibling::colgroup', output, 1
    end

"#
        );

        let doc = Parser::default().parse(
            "[caption=\"All the Data. \"]\n.Simple psv table\n|=======\n|A |B |C\n|a |b |c\n|1 |2 |3\n|=======",
        );

        assert_xpath(
            &doc,
            "/table/caption[@class=\"title\"][text()=\"All the Data. Simple psv table\"]",
            1,
        );
        assert_xpath(&doc, "/table/caption/following-sibling::colgroup", 1);
    }

    #[test]
    fn disables_caption_when_caption_attribute_on_table_is_empty() {
        verifies!(
            r#"
    test 'disables caption when caption attribute on table is empty' do
      input = <<~'EOS'
      [caption=]
      .Simple psv table
      |=======
      |A |B |C
      |a |b |c
      |1 |2 |3
      |=======
      EOS
      output = convert_string_to_embedded input
      assert_xpath '/table/caption[@class="title"][text()="Simple psv table"]', output, 1
      assert_xpath '/table/caption/following-sibling::colgroup', output, 1
    end

"#
        );

        let doc = Parser::default().parse(
            "[caption=]\n.Simple psv table\n|=======\n|A |B |C\n|a |b |c\n|1 |2 |3\n|=======",
        );

        assert_xpath(
            &doc,
            "/table/caption[@class=\"title\"][text()=\"Simple psv table\"]",
            1,
        );
        assert_xpath(&doc, "/table/caption/following-sibling::colgroup", 1);
    }

    #[test]
    fn disables_caption_when_caption_attribute_on_table_is_empty_string() {
        verifies!(
            r#"
    test 'disables caption when caption attribute on table is empty string' do
      input = <<~'EOS'
      [caption=""]
      .Simple psv table
      |=======
      |A |B |C
      |a |b |c
      |1 |2 |3
      |=======
      EOS
      output = convert_string_to_embedded input
      assert_xpath '/table/caption[@class="title"][text()="Simple psv table"]', output, 1
      assert_xpath '/table/caption/following-sibling::colgroup', output, 1
    end

"#
        );

        let doc = Parser::default().parse(
            "[caption=\"\"]\n.Simple psv table\n|=======\n|A |B |C\n|a |b |c\n|1 |2 |3\n|=======",
        );

        assert_xpath(
            &doc,
            "/table/caption[@class=\"title\"][text()=\"Simple psv table\"]",
            1,
        );
        assert_xpath(&doc, "/table/caption/following-sibling::colgroup", 1);
    }

    #[test]
    fn disables_caption_on_table_when_table_caption_document_attribute_is_unset() {
        verifies!(
            r#"
    test 'disables caption on table when table-caption document attribute is unset' do
      input = <<~'EOS'
      :!table-caption:

      .Simple psv table
      |=======
      |A |B |C
      |a |b |c
      |1 |2 |3
      |=======
      EOS
      output = convert_string_to_embedded input
      assert_xpath '/table/caption[@class="title"][text()="Simple psv table"]', output, 1
      assert_xpath '/table/caption/following-sibling::colgroup', output, 1
    end

"#
        );

        let doc = Parser::default().parse(
            ":!table-caption:\n\n.Simple psv table\n|=======\n|A |B |C\n|a |b |c\n|1 |2 |3\n|=======",
        );

        assert_xpath(
            &doc,
            "/table/caption[@class=\"title\"][text()=\"Simple psv table\"]",
            1,
        );
        assert_xpath(&doc, "/table/caption/following-sibling::colgroup", 1);
    }

    #[test]
    fn ignores_escaped_separators() {
        verifies!(
            r#"
    test 'ignores escaped separators' do
      input = <<~'EOS'
      |===
      |A \| here| a \| there
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 2
      assert_css 'table > tbody > tr', output, 1
      assert_css 'table > tbody > tr > td', output, 2
      assert_xpath '/table/tbody/tr/td[1]/p[text()="A | here"]', output, 1
      assert_xpath '/table/tbody/tr/td[2]/p[text()="a | there"]', output, 1
    end

"#
        );

        let doc = Parser::default().parse("|===\n|A \\| here| a \\| there\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > tbody > tr", 1);
        assert_css(&doc, "table > tbody > tr > td", 2);
        assert_xpath(&doc, "/table/tbody/tr/td[1]/p[text()=\"A | here\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr/td[2]/p[text()=\"a | there\"]", 1);
    }

    #[test]
    fn preserves_escaped_delimiters_at_the_end_of_the_line() {
        verifies!(
            r#"
    test 'preserves escaped delimiters at the end of the line' do
      input = <<~'EOS'
      [%header,cols="1,1"]
      |===
      |A |B\|
      |A1 |B1\|
      |A2 |B2\|
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 2
      assert_css 'table > thead > tr', output, 1
      assert_css 'table > thead > tr:nth-child(1) > th', output, 2
      assert_xpath '/table/thead/tr[1]/th[2][text()="B|"]', output, 1
      assert_css 'table > tbody > tr', output, 2
      assert_css 'table > tbody > tr:nth-child(1) > td', output, 2
      assert_xpath '/table/tbody/tr[1]/td[2]/p[text()="B1|"]', output, 1
      assert_css 'table > tbody > tr:nth-child(2) > td', output, 2
      assert_xpath '/table/tbody/tr[2]/td[2]/p[text()="B2|"]', output, 1
    end

"#
        );

        let doc = Parser::default()
            .parse("[%header,cols=\"1,1\"]\n|===\n|A |B\\|\n|A1 |B1\\|\n|A2 |B2\\|\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > thead > tr", 1);
        assert_xpath(&doc, "(/table/thead/tr)[1]/th", 2);
        assert_xpath(&doc, "/table/thead/tr[1]/th[2][text()=\"B|\"]", 1);
        assert_css(&doc, "table > tbody > tr", 2);
        assert_xpath(&doc, "(/table/tbody/tr)[1]/td", 2);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[2]/p[text()=\"B1|\"]", 1);
        assert_xpath(&doc, "(/table/tbody/tr)[2]/td", 2);
        assert_xpath(&doc, "/table/tbody/tr[2]/td[2]/p[text()=\"B2|\"]", 1);
    }

    #[test]
    fn should_treat_trailing_pipe_as_an_empty_cell() {
        verifies!(
            r#"
    test 'should treat trailing pipe as an empty cell' do
      input = <<~'EOS'
      |===
      |A1 |
      |B1 |B2
      |C1 |C2
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 2
      assert_css 'table > tbody > tr', output, 3
      assert_xpath '/table/tbody/tr[1]/td', output, 2
      assert_xpath '/table/tbody/tr[1]/td[1]/p[text()="A1"]', output, 1
      assert_xpath '/table/tbody/tr[1]/td[2]/p', output, 0
      assert_xpath '/table/tbody/tr[2]/td[1]/p[text()="B1"]', output, 1
    end

"#
        );

        let doc = Parser::default().parse("|===\n|A1 |\n|B1 |B2\n|C1 |C2\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > tbody > tr", 3);
        assert_xpath(&doc, "/table/tbody/tr[1]/td", 2);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[1]/p[text()=\"A1\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[2]/p", 0);
        assert_xpath(&doc, "/table/tbody/tr[2]/td[1]/p[text()=\"B1\"]", 1);
    }

    #[test]
    fn should_auto_recover_with_warning_if_missing_leading_separator_on_first_cell() {
        verifies!(
            r#"
    test 'should auto recover with warning if missing leading separator on first cell' do
      input = <<~'EOS'
      |===
      A | here| a | there
      | x
      | y
      | z
      | end
      |===
      EOS
      using_memory_logger do |logger|
        output = convert_string_to_embedded input
        assert_css 'table', output, 1
        assert_css 'table > tbody > tr', output, 2
        assert_css 'table > tbody > tr > td', output, 8
        assert_xpath '/table/tbody/tr[1]/td[1]/p[text()="A"]', output, 1
        assert_xpath '/table/tbody/tr[1]/td[2]/p[text()="here"]', output, 1
        assert_xpath '/table/tbody/tr[1]/td[3]/p[text()="a"]', output, 1
        assert_xpath '/table/tbody/tr[1]/td[4]/p[text()="there"]', output, 1
        assert_message logger, :ERROR, '<stdin>: line 2: table missing leading separator; recovering automatically', Hash
      end
    end

"#
        );

        let doc = Parser::default().parse("|===\nA | here| a | there\n| x\n| y\n| z\n| end\n|===");

        // The content before the first separator (`A`) is recovered as the first
        // cell, so the first row has four cells and the table four columns.
        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > tbody > tr", 2);
        assert_css(&doc, "table > tbody > tr > td", 8);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[1]/p[text()=\"A\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[2]/p[text()=\"here\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[3]/p[text()=\"a\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[4]/p[text()=\"there\"]", 1);

        // The recovery logs an error pointing at the offending line (line 2).
        let warnings: Vec<_> = doc.warnings().collect();
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::TableMissingLeadingSeparator
        );
        assert_eq!(warnings[0].source.line(), 2);
    }

    #[test]
    fn performs_normal_substitutions_on_cell_content() {
        verifies!(
            r#"
    test 'performs normal substitutions on cell content' do
      input = <<~'EOS'
      :show_title: Cool new show
      |===
      |{show_title} |Coming soon...
      |===
      EOS
      output = convert_string_to_embedded input
      assert_xpath '//tbody/tr/td[1]/p[text()="Cool new show"]', output, 1
      assert_xpath %(//tbody/tr/td[2]/p[text()='Coming soon#{decode_char 8230}#{decode_char 8203}']), output, 1
    end

"#
        );

        let doc = Parser::default()
            .parse(":show_title: Cool new show\n|===\n|{show_title} |Coming soon...\n|===");

        assert_xpath(&doc, "//tbody/tr/td[1]/p[text()=\"Cool new show\"]", 1);
        assert_xpath(
            &doc,
            "//tbody/tr/td[2]/p[text()='Coming soon\u{2026}\u{200b}']",
            1,
        );
    }

    #[test]
    fn should_only_substitute_specialchars_for_literal_table_cells() {
        verifies!(
            r#"
    test 'should only substitute specialchars for literal table cells' do
      input = <<~'EOS'
      |===
      l|one
      *two*
      three
      <four>
      |===
      EOS
      output = convert_string_to_embedded input
      result = xmlnodes_at_xpath('/table//pre', output, 1)
      assert_equal %(<pre>one\n*two*\nthree\n&lt;four&gt;</pre>), result.to_s
    end

"#
        );

        let doc = Parser::default().parse("|===\nl|one\n*two*\nthree\n<four>\n|===");

        // Ruby compares the serialized `<pre>one\n*two*\nthree\n&lt;four&gt;</pre>`;
        // the test DOM decodes entities in `text()`, so this asserts the decoded
        // content (formatting markup left literal, specialchars escaped then
        // decoded back).
        assert_css(&doc, "table pre", 1);
        assert_xpath(&doc, "/table//pre[text()=\"one\n*two*\nthree\n<four>\"]", 1);
    }

    #[test]
    fn should_preserve_leading_spaces_but_not_leading_newlines_or_trailing_spaces_in_literal_table_cells()
     {
        verifies!(
            r#"
    test 'should preserving leading spaces but not leading newlines or trailing spaces in literal table cells' do
      # NOTE cannot use single-quoted heredoc because of https://github.com/jruby/jruby/issues/4260
      input = <<~EOS
      [cols=2*]
      |===
      l|
        one
        two
      three

        | normal
      |===
      EOS
      output = convert_string_to_embedded input
      result = xmlnodes_at_xpath('/table//pre', output, 1)
      assert_equal %(<pre>  one\n  two\nthree</pre>), result.to_s
    end

"#
        );

        let doc =
            Parser::default().parse("[cols=2*]\n|===\nl|\n  one\n  two\nthree\n\n  | normal\n|===");

        assert_css(&doc, "table pre", 1);
        assert_xpath(&doc, "/table//pre[text()=\"  one\n  two\nthree\"]", 1);
    }

    #[test]
    fn should_ignore_v_table_cell_style() {
        verifies!(
            r#"
    test 'should ignore v table cell style' do
      # NOTE cannot use single-quoted heredoc because of https://github.com/jruby/jruby/issues/4260
      input = <<~EOS
      [cols=2*]
      |===
      v|
        one
        two
      three

        | normal
      |===
      EOS
      output = convert_string_to_embedded input
      result = xmlnodes_at_xpath('/table//p[@class="tableblock"]', output, 1)
      assert_equal %(<p class="tableblock">one\n  two\nthree</p>), result.to_s
    end

"#
        );

        let doc =
            Parser::default().parse("[cols=2*]\n|===\nv|\n  one\n  two\nthree\n\n  | normal\n|===");

        // The unrecognized `v` style is ignored, so the cell renders as a normal
        // paragraph (`p.tableblock`) with leading newlines and trailing spaces
        // stripped but interior indentation preserved.
        assert_xpath(
            &doc,
            "(/table/tbody/tr/td)[1]/p[@class=\"tableblock\"][text()=\"one\n  two\nthree\"]",
            1,
        );
    }

    #[test]
    fn table_and_column_width_not_assigned_when_autowidth_option_is_specified() {
        verifies!(
            r#"
    test 'table and column width not assigned when autowidth option is specified' do
      input = <<~'EOS'
      [options="autowidth"]
      |=======
      |A |B |C
      |a |b |c
      |1 |2 |3
      |=======
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table.fit-content', output, 1
      assert_css 'table[style*="width"]', output, 0
      assert_css 'table colgroup col', output, 3
      assert_css 'table colgroup col[style*="width"]', output, 0
    end

"#
        );

        let doc = Parser::default()
            .parse("[options=\"autowidth\"]\n|=======\n|A |B |C\n|a |b |c\n|1 |2 |3\n|=======");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table.fit-content", 1);
        assert_css(&doc, "table[style*=\"width\"]", 0);
        assert_css(&doc, "table colgroup col", 3);
        assert_css(&doc, "table colgroup col[style*=\"width\"]", 0);
    }

    #[test]
    fn does_not_assign_column_width_for_autowidth_columns_in_html_output() {
        verifies!(
            r#"
    test 'does not assign column width for autowidth columns in HTML output' do
      input = <<~'EOS'
      [cols="15%,3*~"]
      |=======
      |A |B |C |D
      |a |b |c |d
      |1 |2 |3 |4
      |=======
      EOS
      doc = document_from_string input
      table_row0 = doc.blocks[0].rows.body[0]
      assert_equal 15, table_row0[0].attributes['width']
      assert_equal 15, table_row0[0].attributes['colpcwidth']
      refute_equal '', table_row0[0].attributes['autowidth-option']
      expected_pcwidths = { 1 => 28.3333, 2 => 28.3333, 3 => 28.3334 }
      (1..3).each do |i|
        assert_equal 28.3333, table_row0[i].attributes['width']
        assert_equal expected_pcwidths[i], table_row0[i].attributes['colpcwidth']
        assert_equal '', table_row0[i].attributes['autowidth-option']
      end
      output = doc.convert standalone: false
      assert_css 'table', output, 1
      assert_css 'table colgroup col', output, 4
      assert_css 'table colgroup col[style]', output, 1
      assert_css 'table colgroup col[style*="width: 15%"]', output, 1
    end

"#
        );

        let doc = Parser::default()
            .parse("[cols=\"15%,3*~\"]\n|=======\n|A |B |C |D\n|a |b |c |d\n|1 |2 |3 |4\n|=======");

        // The fixed first column keeps its computed percentage in `colpcwidth`
        // and carries the HTML `width` attribute; it is not autowidth.
        assert_xpath(&doc, "(/table/colgroup/col)[1][@colpcwidth=\"15\"]", 1);
        assert_xpath(&doc, "(/table/colgroup/col)[1][@width=\"15%\"]", 1);
        assert_xpath(&doc, "(/table/colgroup/col)[1][@autowidth-option]", 0);

        // Each autowidth column still has a computed `colpcwidth` (the remaining
        // space shared three ways, truncated to four places with the balance
        // donated to the last column) and the `autowidth-option` marker, but no
        // HTML `width` attribute.
        for i in 2..=3 {
            assert_xpath(
                &doc,
                &format!("(/table/colgroup/col)[{i}][@colpcwidth=\"28.3333\"]"),
                1,
            );
        }
        assert_xpath(&doc, "(/table/colgroup/col)[4][@colpcwidth=\"28.3334\"]", 1);
        for i in 2..=4 {
            assert_xpath(
                &doc,
                &format!("(/table/colgroup/col)[{i}][@autowidth-option]"),
                1,
            );
            assert_xpath(&doc, &format!("(/table/colgroup/col)[{i}][@width]"), 0);
        }

        // The HTML-output expectations from the Ruby test: every column renders a
        // `<col>`, but only the single fixed column carries a `width` attribute.
        assert_css(&doc, "table", 1);
        assert_css(&doc, "table colgroup col", 4);
        assert_css(&doc, "table colgroup col[width]", 1);
        assert_css(&doc, "table colgroup col[width=\"15%\"]", 1);
        assert_css(&doc, "table colgroup col[autowidth-option]", 3);
    }

    #[test]
    fn can_assign_autowidth_to_all_columns_even_when_table_has_a_width() {
        verifies!(
            r#"
    test 'can assign autowidth to all columns even when table has a width' do
      input = <<~'EOS'
      [cols="4*~",width=50%]
      |=======
      |A |B |C |D
      |a |b |c |d
      |1 |2 |3 |4
      |=======
      EOS
      doc = document_from_string input
      table_row0 = doc.blocks[0].rows.body[0]
      (0..3).each do |i|
        assert_equal 25, table_row0[i].attributes['width']
        assert_equal 25, table_row0[i].attributes['colpcwidth']
        assert_equal '', table_row0[i].attributes['autowidth-option']
      end
      output = doc.convert standalone: false
      assert_css 'table', output, 1
      assert_css 'table[style*="width: 50%"]', output, 1
      assert_css 'table colgroup col', output, 4
      assert_css 'table colgroup col[style]', output, 0
    end

"#
        );

        let doc = Parser::default().parse(
            "[cols=\"4*~\",width=50%]\n|=======\n|A |B |C |D\n|a |b |c |d\n|1 |2 |3 |4\n|=======",
        );

        // Every column is autowidth, so each takes an equal 25% share in
        // `colpcwidth` and carries the `autowidth-option` marker; none carries an
        // HTML `width` attribute even though the table itself has a width.
        for i in 1..=4 {
            assert_xpath(
                &doc,
                &format!("(/table/colgroup/col)[{i}][@colpcwidth=\"25\"]"),
                1,
            );
            assert_xpath(
                &doc,
                &format!("(/table/colgroup/col)[{i}][@autowidth-option]"),
                1,
            );
            assert_xpath(&doc, &format!("(/table/colgroup/col)[{i}][@width]"), 0);
        }

        // The explicit table width is still rendered, and no column carries a
        // width-bearing `style` (the Ruby HTML-output expectations).
        assert_css(&doc, "table", 1);
        assert_css(&doc, "table[width=\"50%\"]", 1);
        assert_css(&doc, "table colgroup col", 4);
        assert_css(&doc, "table colgroup col[style]", 0);
        assert_css(&doc, "table colgroup col[autowidth-option]", 4);
    }

    // Out of scope: DocBook backend. ("equally distributes remaining column
    // width to autowidth columns in DocBook output")
    non_normative!(
        r#"
    test 'equally distributes remaining column width to autowidth columns in DocBook output' do
      input = <<~'EOS'
      [cols="15%,3*~"]
      |=======
      |A |B |C |D
      |a |b |c |d
      |1 |2 |3 |4
      |=======
      EOS
      output = convert_string_to_embedded input, backend: 'docbook5'
      assert_css 'tgroup[cols="4"]', output, 1
      assert_css 'tgroup colspec', output, 4
      assert_css 'tgroup colspec[colwidth]', output, 4
      assert_css 'tgroup colspec[colwidth="15*"]', output, 1
      assert_css 'tgroup colspec[colwidth="28.3333*"]', output, 2
      assert_css 'tgroup colspec[colwidth="28.3334*"]', output, 1
    end

"#
    );

    // Out of scope: DocBook backend. ("should compute column widths based on
    // pagewidth when width is set on table in DocBook output")
    non_normative!(
        r#"
    test 'should compute column widths based on pagewidth when width is set on table in DocBook output' do
      input = <<~'EOS'
      :pagewidth: 500

      [width=50%]
      |=======
      |A |B |C |D

      |a |b |c |d
      |1 |2 |3 |4
      |=======
      EOS
      output = convert_string_to_embedded input, backend: 'docbook5'
      assert_css 'tgroup[cols="4"]', output, 1
      assert_css 'tgroup colspec', output, 4
      assert_css 'tgroup colspec[colwidth]', output, 4
      assert_css 'tgroup colspec[colwidth="62.5*"]', output, 4
    end

"#
    );

    #[test]
    fn explicit_table_width_is_used_even_when_autowidth_option_is_specified() {
        verifies!(
            r#"
    test 'explicit table width is used even when autowidth option is specified' do
      input = <<~'EOS'
      [%autowidth,width=75%]
      |=======
      |A |B |C
      |a |b |c
      |1 |2 |3
      |=======
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table[style*="width"]', output, 1
      assert_css 'table colgroup col', output, 3
      assert_css 'table colgroup col[style*="width"]', output, 0
    end

"#
        );

        let doc = Parser::default()
            .parse("[%autowidth,width=75%]\n|=======\n|A |B |C\n|a |b |c\n|1 |2 |3\n|=======");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table[width]", 1);
        assert_css(&doc, "table colgroup col", 3);
        assert_css(&doc, "table colgroup col[style*=\"width\"]", 0);
    }

    #[test]
    fn first_row_sets_number_of_columns_when_not_specified() {
        verifies!(
            r#"
    test 'first row sets number of columns when not specified' do
      input = <<~'EOS'
      |===
      |first |second |third |fourth
      |1 |2 |3
      |4
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 4
      assert_css 'table > tbody > tr', output, 2
      assert_css 'table > tbody > tr:nth-child(1) > td', output, 4
      assert_css 'table > tbody > tr:nth-child(2) > td', output, 4
    end

"#
        );

        let doc =
            Parser::default().parse("|===\n|first |second |third |fourth\n|1 |2 |3\n|4\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 4);
        assert_css(&doc, "table > tbody > tr", 2);
        assert_xpath(&doc, "(/table/tbody/tr)[1]/td", 4);
        assert_xpath(&doc, "(/table/tbody/tr)[2]/td", 4);
    }

    #[test]
    fn colspec_attribute_using_asterisk_syntax_sets_number_of_columns() {
        verifies!(
            r#"
    test 'colspec attribute using asterisk syntax sets number of columns' do
      input = <<~'EOS'
      [cols="3*"]
      |===
      |A |B |C |a |b |c |1 |2 |3
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > tbody > tr', output, 3
    end

"#
        );

        let doc = Parser::default().parse("[cols=\"3*\"]\n|===\n|A |B |C |a |b |c |1 |2 |3\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > tbody > tr", 3);
    }

    #[test]
    fn table_with_explicit_column_count_can_have_multiple_rows_on_a_single_line() {
        verifies!(
            r#"
    test 'table with explicit column count can have multiple rows on a single line' do
      input = <<~'EOS'
      [cols="3*"]
      |===
      |one |two
      |1 |2 |a |b
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 3
      assert_css 'table > tbody > tr', output, 2
    end

"#
        );

        let doc = Parser::default().parse("[cols=\"3*\"]\n|===\n|one |two\n|1 |2 |a |b\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 3);
        assert_css(&doc, "table > tbody > tr", 2);
    }

    #[test]
    fn table_with_explicit_deprecated_colspec_syntax_can_have_multiple_rows_on_a_single_line() {
        verifies!(
            r#"
    test 'table with explicit deprecated colspec syntax can have multiple rows on a single line' do
      input = <<~'EOS'
      [cols="3"]
      |===
      |one |two
      |1 |2 |a |b
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 3
      assert_css 'table > tbody > tr', output, 2
    end

"#
        );

        let doc = Parser::default().parse("[cols=\"3\"]\n|===\n|one |two\n|1 |2 |a |b\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 3);
        assert_css(&doc, "table > tbody > tr", 2);
    }

    #[test]
    fn columns_are_added_for_empty_records_in_colspec_attribute() {
        verifies!(
            r#"
    test 'columns are added for empty records in colspec attribute' do
      input = <<~'EOS'
      [cols="<,"]
      |===
      |one |two
      |1 |2 |a |b
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 2
      assert_css 'table > tbody > tr', output, 3
    end

"#
        );

        let doc = Parser::default().parse("[cols=\"<,\"]\n|===\n|one |two\n|1 |2 |a |b\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > tbody > tr", 3);
    }

    #[test]
    fn cols_may_be_separated_by_semi_colon_instead_of_comma() {
        verifies!(
            r#"
    test 'cols may be separated by semi-colon instead of comma' do
      input = <<~'EOS'
      [cols="1s;3m"]
      |===
      | strong
      | mono
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 2
      assert_css 'col[style="width: 25%;"]', output, 1
      assert_css 'col[style="width: 75%;"]', output, 1
      assert_xpath '(//td)[1]//strong', output, 1
      assert_xpath '(//td)[2]//code', output, 1
    end

"#
        );

        let doc = Parser::default().parse("[cols=\"1s;3m\"]\n|===\n| strong\n| mono\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "col[width=\"25%\"]", 1);
        assert_css(&doc, "col[width=\"75%\"]", 1);
        assert_xpath(&doc, "(//td)[1]//strong", 1);
        assert_xpath(&doc, "(//td)[2]//code", 1);
    }

    #[test]
    fn cols_attribute_may_include_spaces() {
        verifies!(
            r#"
    test 'cols attribute may include spaces' do
      input = <<~'EOS'
      [cols=" 1, 1 "]
      |===
      |one |two |1 |2 |a |b
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 2
      assert_css 'col[style="width: 50%;"]', output, 2
      assert_css 'table > tbody > tr', output, 3
    end

"#
        );

        let doc = Parser::default().parse("[cols=\" 1, 1 \"]\n|===\n|one |two |1 |2 |a |b\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "col[width=\"50%\"]", 2);
        assert_css(&doc, "table > tbody > tr", 3);
    }

    #[test]
    fn blank_cols_attribute_should_be_ignored() {
        verifies!(
            r#"
    test 'blank cols attribute should be ignored' do
      input = <<~'EOS'
      [cols=" "]
      |===
      |one |two
      |1 |2 |a |b
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 2
      assert_css 'col[style="width: 50%;"]', output, 2
      assert_css 'table > tbody > tr', output, 3
    end

"#
        );

        let doc = Parser::default().parse("[cols=\" \"]\n|===\n|one |two\n|1 |2 |a |b\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "col[width=\"50%\"]", 2);
        assert_css(&doc, "table > tbody > tr", 3);
    }

    #[test]
    fn empty_cols_attribute_should_be_ignored() {
        verifies!(
            r#"
    test 'empty cols attribute should be ignored' do
      input = <<~'EOS'
      [cols=""]
      |===
      |one |two
      |1 |2 |a |b
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 2
      assert_css 'col[style="width: 50%;"]', output, 2
      assert_css 'table > tbody > tr', output, 3
    end

"#
        );

        let doc = Parser::default().parse("[cols=\"\"]\n|===\n|one |two\n|1 |2 |a |b\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "col[width=\"50%\"]", 2);
        assert_css(&doc, "table > tbody > tr", 3);
    }

    #[test]
    fn table_with_header_and_footer() {
        verifies!(
            r#"
    test 'table with header and footer' do
      input = <<~'EOS'
      [options="header,footer"]
      |===
      |Item       |Quantity
      |Item 1     |1
      |Item 2     |2
      |Item 3     |3
      |Total      |6
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 2
      assert_css 'table > thead', output, 1
      assert_css 'table > thead > tr', output, 1
      assert_css 'table > thead > tr > th', output, 2
      assert_css 'table > tfoot', output, 1
      assert_css 'table > tfoot > tr', output, 1
      assert_css 'table > tfoot > tr > td', output, 2
      assert_css 'table > tbody', output, 1
      assert_css 'table > tbody > tr', output, 3
      table_section_names = (xmlnodes_at_css 'table > *', output).map(&:node_name).select {|n| n.start_with? 't' }
      assert_equal %w(thead tbody tfoot), table_section_names
    end

"#
        );

        let doc = Parser::default().parse(
            "[options=\"header,footer\"]\n|===\n|Item       |Quantity\n|Item 1     |1\n|Item 2     |2\n|Item 3     |3\n|Total      |6\n|===",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > thead", 1);
        assert_css(&doc, "table > thead > tr", 1);
        assert_css(&doc, "table > thead > tr > th", 2);
        assert_css(&doc, "table > tfoot", 1);
        assert_css(&doc, "table > tfoot > tr", 1);
        assert_css(&doc, "table > tfoot > tr > td", 2);
        assert_css(&doc, "table > tbody", 1);
        assert_css(&doc, "table > tbody > tr", 3);

        // Ruby additionally asserts the section order is thead, tbody, tfoot;
        // the renderer emits them in that order.
        assert_xpath(&doc, "/table/thead/following-sibling::tbody", 1);
        assert_xpath(&doc, "/table/tbody/following-sibling::tfoot", 1);
    }

    // Out of scope: DocBook backend. ("table with header and footer docbook")
    non_normative!(
        r#"
    test 'table with header and footer docbook' do
      input = <<~'EOS'
      .Table with header, body and footer
      [options="header,footer"]
      |===
      |Item       |Quantity
      |Item 1     |1
      |Item 2     |2
      |Item 3     |3
      |Total      |6
      |===
      EOS
      output = convert_string_to_embedded input, backend: 'docbook'
      assert_css 'table', output, 1
      assert_css 'table > title', output, 1
      assert_css 'table > tgroup', output, 1
      assert_css 'table > tgroup[cols="2"]', output, 1
      assert_css 'table > tgroup[cols="2"] > colspec', output, 2
      assert_css 'table > tgroup[cols="2"] > colspec[colwidth="50*"]', output, 2
      assert_css 'table > tgroup > thead', output, 1
      assert_css 'table > tgroup > thead > row', output, 1
      assert_css 'table > tgroup > thead > row > entry', output, 2
      assert_css 'table > tgroup > thead > row > entry > simpara', output, 0
      assert_css 'table > tgroup > tfoot', output, 1
      assert_css 'table > tgroup > tfoot > row', output, 1
      assert_css 'table > tgroup > tfoot > row > entry', output, 2
      assert_css 'table > tgroup > tfoot > row > entry > simpara', output, 2
      assert_css 'table > tgroup > tbody', output, 1
      assert_css 'table > tgroup > tbody > row', output, 3
      assert_css 'table > tgroup > tbody > row', output, 3
      table_section_names = (xmlnodes_at_css 'table > tgroup > *', output).map(&:node_name).select {|n| n.start_with? 't' }
      assert_equal %w(thead tbody tfoot), table_section_names
    end

"#
    );

    // Out of scope: DocBook backend. ("should set horizontal and vertical
    // alignment when converting to DocBook")
    non_normative!(
        r#"
    test 'should set horizontal and vertical alignment when converting to DocBook' do
      input = <<~'EOS'
      |===
      |A ^.^|B >|C

      |A1
      ^.^|B1
      >|C1
      |===
      EOS
      output = convert_string input, backend: 'docbook'
      assert_css 'informaltable', output, 1
      assert_css 'informaltable thead > row > entry[align="left"][valign="top"]', output, 1
      assert_css 'informaltable thead > row > entry[align="center"][valign="middle"]', output, 1
      assert_css 'informaltable thead > row > entry[align="right"][valign="top"]', output, 1
      assert_css 'informaltable tbody > row > entry[align="left"][valign="top"]', output, 1
      assert_css 'informaltable tbody > row > entry[align="center"][valign="middle"]', output, 1
      assert_css 'informaltable tbody > row > entry[align="right"][valign="top"]', output, 1
    end

"#
    );

    #[test]
    fn should_preserve_frame_value_ends_when_converting_to_html() {
        verifies!(
            r#"
    test 'should preserve frame value ends when converting to HTML' do
      input = <<~'EOS'
      [frame=ends]
      |===
      |A |B |C
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table.frame-ends', output, 1
    end

"#
        );

        let doc = Parser::default().parse("[frame=ends]\n|===\n|A |B |C\n|===");

        assert_css(&doc, "table.frame-ends", 1);
    }

    #[test]
    fn should_normalize_frame_value_topbot_as_ends_when_converting_to_html() {
        verifies!(
            r#"
    test 'should normalize frame value topbot as ends when converting to HTML' do
      input = <<~'EOS'
      [frame=topbot]
      |===
      |A |B |C
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table.frame-ends', output, 1
    end

"#
        );

        let doc = Parser::default().parse("[frame=topbot]\n|===\n|A |B |C\n|===");

        assert_css(&doc, "table.frame-ends", 1);
    }

    // Out of scope: DocBook backend. ("should preserve frame value topbot when
    // converting to DocBook")
    non_normative!(
        r#"
    test 'should preserve frame value topbot when converting to DocBook' do
      input = <<~'EOS'
      [frame=topbot]
      |===
      |A |B |C
      |===
      EOS
      output = convert_string_to_embedded input, backend: 'docbook'
      assert_css 'informaltable[frame="topbot"]', output, 1
    end

"#
    );

    // Out of scope: DocBook backend. ("should convert frame value ends to topbot
    // when converting to DocBook")
    non_normative!(
        r#"
    test 'should convert frame value ends to topbot when converting to DocBook' do
      input = <<~'EOS'
      [frame=ends]
      |===
      |A |B |C
      |===
      EOS
      output = convert_string_to_embedded input, backend: 'docbook'
      assert_css 'informaltable[frame="topbot"]', output, 1
    end

"#
    );

    // Out of scope: DocBook backend. ("table with landscape orientation in
    // DocBook")
    non_normative!(
        r#"
    test 'table with landscape orientation in DocBook' do
      ['orientation=landscape', '%rotate'].each do |attrs|
        input = <<~EOS
        [#{attrs}]
        |===
        |Column A | Column B | Column C
        |===
        EOS

        output = convert_string_to_embedded input, backend: 'docbook'
        assert_css 'informaltable', output, 1
        assert_css 'informaltable[orient="land"]', output, 1
      end
    end

"#
    );

    #[test]
    fn table_with_implicit_header_row() {
        verifies!(
            r#"
    test 'table with implicit header row' do
      input = <<~'EOS'
      |===
      |Column 1 |Column 2

      |Data A1
      |Data B1

      |Data A2
      |Data B2
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 2
      assert_css 'table > thead', output, 1
      assert_css 'table > thead > tr', output, 1
      assert_css 'table > thead > tr > th', output, 2
      assert_css 'table > tbody', output, 1
      assert_css 'table > tbody > tr', output, 2
    end

"#
        );

        let doc = Parser::default()
            .parse("|===\n|Column 1 |Column 2\n\n|Data A1\n|Data B1\n\n|Data A2\n|Data B2\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > thead", 1);
        assert_css(&doc, "table > thead > tr", 1);
        assert_css(&doc, "table > thead > tr > th", 2);
        assert_css(&doc, "table > tbody", 1);
        assert_css(&doc, "table > tbody > tr", 2);
    }

    #[test]
    fn table_with_implicit_header_row_only() {
        verifies!(
            r#"
    test 'table with implicit header row only' do
      input = <<~'EOS'
      |===
      |Column 1 |Column 2

      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 2
      assert_css 'table > thead', output, 1
      assert_css 'table > thead > tr', output, 1
      assert_css 'table > thead > tr > th', output, 2
      assert_css 'table > tbody', output, 0
    end

"#
        );

        let doc = Parser::default().parse("|===\n|Column 1 |Column 2\n\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > thead", 1);
        assert_css(&doc, "table > thead > tr", 1);
        assert_css(&doc, "table > thead > tr > th", 2);
        assert_css(&doc, "table > tbody", 0);
    }

    #[test]
    fn table_with_implicit_header_row_when_other_options_set() {
        verifies!(
            r#"
    test 'table with implicit header row when other options set' do
      input = <<~'EOS'
      [%autowidth]
      |===
      |Column 1 |Column 2

      |Data A1
      |Data B1
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table[style*="width"]', output, 0
      assert_css 'table > colgroup > col', output, 2
      assert_css 'table > thead', output, 1
      assert_css 'table > thead > tr', output, 1
      assert_css 'table > thead > tr > th', output, 2
      assert_css 'table > tbody', output, 1
      assert_css 'table > tbody > tr', output, 1
    end

"#
        );

        let doc = Parser::default()
            .parse("[%autowidth]\n|===\n|Column 1 |Column 2\n\n|Data A1\n|Data B1\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table[style*=\"width\"]", 0);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > thead", 1);
        assert_css(&doc, "table > thead > tr", 1);
        assert_css(&doc, "table > thead > tr > th", 2);
        assert_css(&doc, "table > tbody", 1);
        assert_css(&doc, "table > tbody > tr", 1);
    }

    #[test]
    fn no_implicit_header_row_if_second_line_not_blank() {
        verifies!(
            r#"
    test 'no implicit header row if second line not blank' do
      input = <<~'EOS'
      |===
      |Column 1 |Column 2
      |Data A1
      |Data B1

      |Data A2
      |Data B2
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 2
      assert_css 'table > thead', output, 0
      assert_css 'table > tbody', output, 1
      assert_css 'table > tbody > tr', output, 3
    end

"#
        );

        let doc = Parser::default()
            .parse("|===\n|Column 1 |Column 2\n|Data A1\n|Data B1\n\n|Data A2\n|Data B2\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > thead", 0);
        assert_css(&doc, "table > tbody", 1);
        assert_css(&doc, "table > tbody > tr", 3);
    }

    #[test]
    fn no_implicit_header_row_if_cell_in_first_line_spans_multiple_lines() {
        verifies!(
            r#"
    test 'no implicit header row if cell in first line spans multiple lines' do
      input = <<~'EOS'
      [cols=2*]
      |===
      |A1


      A1 continued|B1

      |A2
      |B2
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 2
      assert_css 'table > thead', output, 0
      assert_css 'table > tbody', output, 1
      assert_css 'table > tbody > tr', output, 2
      assert_xpath '(//td)[1]/p', output, 2
    end

"#
        );

        let doc =
            Parser::default().parse("[cols=2*]\n|===\n|A1\n\n\nA1 continued|B1\n\n|A2\n|B2\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > thead", 0);
        assert_css(&doc, "table > tbody", 1);
        assert_css(&doc, "table > tbody > tr", 2);
        assert_xpath(&doc, "(//td)[1]/p", 2);
    }

    #[test]
    fn should_format_first_cell_as_literal_if_there_is_no_implicit_header_row_and_column_has_l_style()
     {
        verifies!(
            r#"
    test 'should format first cell as literal if there is no implicit header row and column has l style' do
      input = <<~'EOS'
      [cols="1l,1"]
      |===
      |literal
      |normal
      |===
      EOS

      output = convert_string_to_embedded input
      assert_css 'tbody pre', output, 1
      assert_css 'tbody p.tableblock', output, 1
    end

"#
        );

        let doc = Parser::default().parse("[cols=\"1l,1\"]\n|===\n|literal\n|normal\n|===");

        assert_css(&doc, "tbody pre", 1);
        assert_css(&doc, "tbody p.tableblock", 1);
    }

    #[test]
    fn should_format_first_cell_as_asciidoc_if_there_is_no_implicit_header_row_and_column_has_a_style()
     {
        verifies!(
            r#"
    test 'should format first cell as AsciiDoc if there is no implicit header row and column has a style' do
      input = <<~'EOS'
      [cols="1a,1"]
      |===
      | * list
      | normal
      |===
      EOS

      output = convert_string_to_embedded input
      assert_css 'tbody .ulist', output, 1
      assert_css 'tbody p.tableblock', output, 1
    end

"#
        );

        let doc = Parser::default().parse("[cols=\"1a,1\"]\n|===\n| * list\n| normal\n|===");

        assert_css(&doc, "tbody .ulist", 1);
        assert_css(&doc, "tbody p.tableblock", 1);
    }

    #[test]
    fn should_interpret_leading_indent_if_first_cell_is_asciidoc_and_there_is_no_implicit_header_row()
     {
        verifies!(
            r#"
    test 'should interpret leading indent if first cell is AsciiDoc and there is no implicit header row' do
      # NOTE cannot use single-quoted heredoc because of https://github.com/jruby/jruby/issues/4260
      input = <<~EOS
      [cols="1a,1"]
      |===
      |
        literal
      | normal
      |===
      EOS

      output = convert_string_to_embedded input
      assert_css 'tbody pre', output, 1
      assert_css 'tbody p.tableblock', output, 1
    end

"#
        );

        let doc = Parser::default().parse("[cols=\"1a,1\"]\n|===\n|\n  literal\n| normal\n|===");

        assert_css(&doc, "tbody pre", 1);
        assert_css(&doc, "tbody p.tableblock", 1);
    }

    #[test]
    fn should_format_first_cell_as_asciidoc_if_there_is_no_implicit_header_row_and_cell_has_a_style()
     {
        verifies!(
            r#"
    test 'should format first cell as AsciiDoc if there is no implicit header row and cell has a style' do
      input = <<~'EOS'
      |===
      a| * list
      | normal
      |===
      EOS

      output = convert_string_to_embedded input
      assert_css 'tbody .ulist', output, 1
      assert_css 'tbody p.tableblock', output, 1
    end

"#
        );

        let doc = Parser::default().parse("|===\na| * list\n| normal\n|===");

        assert_css(&doc, "tbody .ulist", 1);
        assert_css(&doc, "tbody p.tableblock", 1);
    }

    #[test]
    fn no_implicit_header_row_if_asciidoc_cell_in_first_line_spans_multiple_lines() {
        verifies!(
            r#"
    test 'no implicit header row if AsciiDoc cell in first line spans multiple lines' do
      input = <<~'EOS'
      [cols=2*]
      |===
      a|contains AsciiDoc content

      * a
      * b
      * c
      a|contains no AsciiDoc content

      just text
      |A2
      |B2
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 2
      assert_css 'table > thead', output, 0
      assert_css 'table > tbody', output, 1
      assert_css 'table > tbody > tr', output, 2
      assert_xpath '(//td)[1]//ul', output, 1
    end

"#
        );

        let doc = Parser::default().parse(
            "[cols=2*]\n|===\na|contains AsciiDoc content\n\n* a\n* b\n* c\na|contains no AsciiDoc content\n\njust text\n|A2\n|B2\n|===",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > thead", 0);
        assert_css(&doc, "table > tbody", 1);
        assert_css(&doc, "table > tbody > tr", 2);
        assert_xpath(&doc, "(//td)[1]//ul", 1);
    }

    #[test]
    fn no_implicit_header_row_if_first_line_blank() {
        verifies!(
            r#"
    test 'no implicit header row if first line blank' do
      input = <<~'EOS'
      |===

      |Column 1 |Column 2

      |Data A1
      |Data B1

      |Data A2
      |Data B2

      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 2
      assert_css 'table > thead', output, 0
      assert_css 'table > tbody', output, 1
      assert_css 'table > tbody > tr', output, 3
    end

"#
        );

        let doc = Parser::default().parse(
            "|===\n\n|Column 1 |Column 2\n\n|Data A1\n|Data B1\n\n|Data A2\n|Data B2\n\n|===",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > thead", 0);
        assert_css(&doc, "table > tbody", 1);
        assert_css(&doc, "table > tbody > tr", 3);
    }

    #[test]
    fn no_implicit_header_row_if_noheader_option_is_specified() {
        verifies!(
            r#"
    test 'no implicit header row if noheader option is specified' do
      input = <<~'EOS'
      [%noheader]
      |===
      |Column 1 |Column 2

      |Data A1
      |Data B1

      |Data A2
      |Data B2
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 2
      assert_css 'table > thead', output, 0
      assert_css 'table > tbody', output, 1
      assert_css 'table > tbody > tr', output, 3
    end

"#
        );

        let doc = Parser::default().parse(
            "[%noheader]\n|===\n|Column 1 |Column 2\n\n|Data A1\n|Data B1\n\n|Data A2\n|Data B2\n|===",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > thead", 0);
        assert_css(&doc, "table > tbody", 1);
        assert_css(&doc, "table > tbody > tr", 3);
    }

    #[test]
    fn styles_not_applied_to_header_cells() {
        verifies!(
            r#"
    test 'styles not applied to header cells' do
      input = <<~'EOS'
      [cols="1h,1s,1e",options="header,footer"]
      |===
      |Name |Occupation| Website
      |Octocat |Social coding| https://github.com
      |Name |Occupation| Website
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > thead > tr > th', output, 3
      assert_css 'table > thead > tr > th > *', output, 0

      assert_css 'table > tfoot > tr > th', output, 1
      assert_css 'table > tfoot > tr > td', output, 2
      assert_css 'table > tfoot > tr > td > p > strong', output, 1
      assert_css 'table > tfoot > tr > td > p > em', output, 1

      assert_css 'table > tbody > tr > th', output, 1
      assert_css 'table > tbody > tr > td', output, 2
      assert_css 'table > tbody > tr > td > p.header', output, 0
      assert_css 'table > tbody > tr > td > p > strong', output, 1
      assert_css 'table > tbody > tr > td > p > em > a', output, 1
    end

"#
        );

        let doc = Parser::default().parse(
            "[cols=\"1h,1s,1e\",options=\"header,footer\"]\n|===\n|Name |Occupation| Website\n|Octocat |Social coding| https://github.com\n|Name |Occupation| Website\n|===",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > thead > tr > th", 3);
        assert_css(&doc, "table > thead > tr > th > *", 0);

        assert_css(&doc, "table > tfoot > tr > th", 1);
        assert_css(&doc, "table > tfoot > tr > td", 2);
        assert_css(&doc, "table > tfoot > tr > td > p > strong", 1);
        assert_css(&doc, "table > tfoot > tr > td > p > em", 1);

        assert_css(&doc, "table > tbody > tr > th", 1);
        assert_css(&doc, "table > tbody > tr > td", 2);
        assert_css(&doc, "table > tbody > tr > td > p.header", 0);
        assert_css(&doc, "table > tbody > tr > td > p > strong", 1);
        assert_css(&doc, "table > tbody > tr > td > p > em > a", 1);
    }

    #[test]
    fn should_apply_text_formatting_to_cells_in_implicit_header_row_when_column_has_a_style() {
        verifies!(
            r#"
    test 'should apply text formatting to cells in implicit header row when column has a style' do
      input = <<~'EOS'
      [cols="2*a"]
      |===
      | _foo_ | *bar*

      | * list item
      | paragraph
      |===
      EOS
      output = convert_string_to_embedded input
      assert_xpath '(//thead/tr/th)[1]/em[text()="foo"]', output, 1
      assert_xpath '(//thead/tr/th)[2]/strong[text()="bar"]', output, 1
      assert_css 'tbody .ulist', output, 1
      assert_css 'tbody .paragraph', output, 1
    end

"#
        );

        let doc = Parser::default()
            .parse("[cols=\"2*a\"]\n|===\n| _foo_ | *bar*\n\n| * list item\n| paragraph\n|===");

        assert_xpath(&doc, "(//thead/tr/th)[1]/em[text()=\"foo\"]", 1);
        assert_xpath(&doc, "(//thead/tr/th)[2]/strong[text()=\"bar\"]", 1);
        assert_css(&doc, "tbody .ulist", 1);
        assert_css(&doc, "tbody .paragraph", 1);
    }

    #[test]
    fn should_apply_style_and_text_formatting_to_cells_in_first_row_if_no_implicit_header() {
        verifies!(
            r#"
    test 'should apply style and text formatting to cells in first row if no implicit header' do
      input = <<~'EOS'
      [cols="s,e"]
      |===
      | _strong_ | *emphasis*
      | strong
      | emphasis
      |===
      EOS
      output = convert_string_to_embedded input
      assert_xpath '((//tbody/tr)[1]/td)[1]//strong/em[text()="strong"]', output, 1
      assert_xpath '((//tbody/tr)[1]/td)[2]//em/strong[text()="emphasis"]', output, 1
      assert_xpath '((//tbody/tr)[2]/td)[1]//strong[text()="strong"]', output, 1
      assert_xpath '((//tbody/tr)[2]/td)[2]//em[text()="emphasis"]', output, 1
    end

"#
        );

        let doc = Parser::default()
            .parse("[cols=\"s,e\"]\n|===\n| _strong_ | *emphasis*\n| strong\n| emphasis\n|===");

        assert_xpath(
            &doc,
            "((//tbody/tr)[1]/td)[1]//strong/em[text()=\"strong\"]",
            1,
        );
        assert_xpath(
            &doc,
            "((//tbody/tr)[1]/td)[2]//em/strong[text()=\"emphasis\"]",
            1,
        );
        assert_xpath(
            &doc,
            "((//tbody/tr)[2]/td)[1]//strong[text()=\"strong\"]",
            1,
        );
        assert_xpath(&doc, "((//tbody/tr)[2]/td)[2]//em[text()=\"emphasis\"]", 1);
    }

    #[test]
    fn vertical_table_headers_use_th_element_instead_of_header_class() {
        verifies!(
            r#"
    test 'vertical table headers use th element instead of header class' do
      input = <<~'EOS'
      [cols="1h,1s,1e"]
      |===

      |Name |Occupation| Website

      |Octocat |Social coding| https://github.com

      |Name |Occupation| Website

      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > tbody > tr > th', output, 3
      assert_css 'table > tbody > tr > td', output, 6
      assert_css 'table > tbody > tr .header', output, 0
      assert_css 'table > tbody > tr > td > p > strong', output, 3
      assert_css 'table > tbody > tr > td > p > em', output, 3
      assert_css 'table > tbody > tr > td > p > em > a', output, 1
    end

"#
        );

        let doc = Parser::default().parse(
            "[cols=\"1h,1s,1e\"]\n|===\n\n|Name |Occupation| Website\n\n|Octocat |Social coding| https://github.com\n\n|Name |Occupation| Website\n\n|===",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > tbody > tr > th", 3);
        assert_css(&doc, "table > tbody > tr > td", 6);
        assert_css(&doc, "table > tbody > tr .header", 0);
        assert_css(&doc, "table > tbody > tr > td > p > strong", 3);
        assert_css(&doc, "table > tbody > tr > td > p > em", 3);
        assert_css(&doc, "table > tbody > tr > td > p > em > a", 1);
    }

    #[test]
    fn supports_horizontal_and_vertical_source_data_with_blank_lines_and_table_header() {
        verifies!(
            r#"
    test 'supports horizontal and vertical source data with blank lines and table header' do
      input = <<~'EOS'
      .Horizontal and vertical source data
      [width="80%",cols="3,^2,^2,10",options="header"]
      |===
      |Date |Duration |Avg HR |Notes

      |22-Aug-08 |10:24 | 157 |
      Worked out MSHR (max sustainable heart rate) by going hard
      for this interval.

      |22-Aug-08 |23:03 | 152 |
      Back-to-back with previous interval.

      |24-Aug-08 |40:00 | 145 |
      Moderately hard interspersed with 3x 3min intervals (2 min
      hard + 1 min really hard taking the HR up to 160).

      I am getting in shape!

      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table[style*="width: 80%"]', output, 1
      assert_xpath '/table/caption[@class="title"][text()="Table 1. Horizontal and vertical source data"]', output, 1
      assert_css 'table > colgroup > col', output, 4
      assert_css 'table > colgroup > col:nth-child(1)[style*="width: 17.647%"]', output, 1
      assert_css 'table > colgroup > col:nth-child(2)[style*="width: 11.7647%"]', output, 1
      assert_css 'table > colgroup > col:nth-child(3)[style*="width: 11.7647%"]', output, 1
      assert_css 'table > colgroup > col:nth-child(4)[style*="width: 58.8236%"]', output, 1
      assert_css 'table > thead', output, 1
      assert_css 'table > thead > tr', output, 1
      assert_css 'table > thead > tr > th', output, 4
      assert_css 'table > tbody > tr', output, 3
      assert_css 'table > tbody > tr:nth-child(1) > td', output, 4
      assert_css 'table > tbody > tr:nth-child(2) > td', output, 4
      assert_css 'table > tbody > tr:nth-child(3) > td', output, 4
      assert_xpath "/table/tbody/tr[1]/td[4]/p[text()='Worked out MSHR (max sustainable heart rate) by going hard\nfor this interval.']", output, 1
      assert_css 'table > tbody > tr:nth-child(3) > td:nth-child(4) > p', output, 2
      assert_xpath '/table/tbody/tr[3]/td[4]/p[2][text()="I am getting in shape!"]', output, 1
    end

"#
        );

        let doc = Parser::default().parse(
            ".Horizontal and vertical source data\n[width=\"80%\",cols=\"3,^2,^2,10\",options=\"header\"]\n|===\n|Date |Duration |Avg HR |Notes\n\n|22-Aug-08 |10:24 | 157 |\nWorked out MSHR (max sustainable heart rate) by going hard\nfor this interval.\n\n|22-Aug-08 |23:03 | 152 |\nBack-to-back with previous interval.\n\n|24-Aug-08 |40:00 | 145 |\nModerately hard interspersed with 3x 3min intervals (2 min\nhard + 1 min really hard taking the HR up to 160).\n\nI am getting in shape!\n\n|===",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table[width=\"80%\"]", 1);
        assert_xpath(
            &doc,
            "/table/caption[@class=\"title\"][text()=\"Table 1. Horizontal and vertical source data\"]",
            1,
        );
        assert_css(&doc, "table > colgroup > col", 4);
        // Ruby uses `col:nth-child(N)`; the indexed XPath form is equivalent.
        assert_xpath(&doc, "(/table/colgroup/col)[1][@width=\"17.647%\"]", 1);
        assert_xpath(&doc, "(/table/colgroup/col)[2][@width=\"11.7647%\"]", 1);
        assert_xpath(&doc, "(/table/colgroup/col)[3][@width=\"11.7647%\"]", 1);
        assert_xpath(&doc, "(/table/colgroup/col)[4][@width=\"58.8236%\"]", 1);
        assert_css(&doc, "table > thead", 1);
        assert_css(&doc, "table > thead > tr", 1);
        assert_css(&doc, "table > thead > tr > th", 4);
        assert_css(&doc, "table > tbody > tr", 3);
        assert_xpath(&doc, "(/table/tbody/tr)[1]/td", 4);
        assert_xpath(&doc, "(/table/tbody/tr)[2]/td", 4);
        assert_xpath(&doc, "(/table/tbody/tr)[3]/td", 4);
        assert_xpath(
            &doc,
            "/table/tbody/tr[1]/td[4]/p[text()='Worked out MSHR (max sustainable heart rate) by going hard\nfor this interval.']",
            1,
        );
        assert_xpath(&doc, "(/table/tbody/tr)[3]/td[4]/p", 2);
        assert_xpath(
            &doc,
            "/table/tbody/tr[3]/td[4]/p[2][text()=\"I am getting in shape!\"]",
            1,
        );
    }

    #[test]
    fn percentages_as_column_widths() {
        verifies!(
            r#"
    test 'percentages as column widths' do
      input = <<~'EOS'
      [cols="<.^10%,<90%"]
      |===
      |column A |column B
      |===
      EOS

      output = convert_string_to_embedded input
      assert_xpath '/table/colgroup/col', output, 2
      assert_xpath '(/table/colgroup/col)[1][@style="width: 10%;"]', output, 1
      assert_xpath '(/table/colgroup/col)[2][@style="width: 90%;"]', output, 1
    end

"#
        );

        let doc =
            Parser::default().parse("[cols=\"<.^10%,<90%\"]\n|===\n|column A |column B\n|===");

        assert_xpath(&doc, "/table/colgroup/col", 2);
        assert_xpath(&doc, "(/table/colgroup/col)[1][@width=\"10%\"]", 1);
        assert_xpath(&doc, "(/table/colgroup/col)[2][@width=\"90%\"]", 1);
    }

    #[test]
    fn spans_alignments_and_styles() {
        verifies!(
            r#"
    test 'spans, alignments and styles' do
      input = <<~'EOS'
      [cols="e,m,^,>s",width="25%"]
      |===
      |1 >s|2 |3 |4
      ^|5 2.2+^.^|6 .3+<.>m|7
      ^|8
      d|9 2+>|10
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col[style*="width: 25%"]', output, 4
      assert_css 'table > tbody > tr', output, 4
      assert_css 'table > tbody > tr > td', output, 10
      assert_css 'table > tbody > tr:nth-child(1) > td', output, 4
      assert_css 'table > tbody > tr:nth-child(2) > td', output, 3
      assert_css 'table > tbody > tr:nth-child(3) > td', output, 1
      assert_css 'table > tbody > tr:nth-child(4) > td', output, 2

      assert_css 'table > tbody > tr:nth-child(1) > td:nth-child(1).halign-left.valign-top p em', output, 1
      assert_css 'table > tbody > tr:nth-child(1) > td:nth-child(2).halign-right.valign-top p strong', output, 1
      assert_css 'table > tbody > tr:nth-child(1) > td:nth-child(3).halign-center.valign-top p', output, 1
      assert_css 'table > tbody > tr:nth-child(1) > td:nth-child(3).halign-center.valign-top p *', output, 0
      assert_css 'table > tbody > tr:nth-child(1) > td:nth-child(4).halign-right.valign-top p strong', output, 1

      assert_css 'table > tbody > tr:nth-child(2) > td:nth-child(1).halign-center.valign-top p em', output, 1
      assert_css 'table > tbody > tr:nth-child(2) > td:nth-child(2).halign-center.valign-middle[colspan="2"][rowspan="2"] p code', output, 1
      assert_css 'table > tbody > tr:nth-child(2) > td:nth-child(3).halign-left.valign-bottom[rowspan="3"] p code', output, 1

      assert_css 'table > tbody > tr:nth-child(3) > td:nth-child(1).halign-center.valign-top p em', output, 1

      assert_css 'table > tbody > tr:nth-child(4) > td:nth-child(1).halign-left.valign-top p', output, 1
      assert_css 'table > tbody > tr:nth-child(4) > td:nth-child(1).halign-left.valign-top p em', output, 0
      assert_css 'table > tbody > tr:nth-child(4) > td:nth-child(2).halign-right.valign-top[colspan="2"] p code', output, 1
    end

"#
        );

        let doc = Parser::default().parse(
            "[cols=\"e,m,^,>s\",width=\"25%\"]\n|===\n|1 >s|2 |3 |4\n^|5 2.2+^.^|6 .3+<.>m|7\n^|8\nd|9 2+>|10\n|===",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col[width=\"25%\"]", 4);
        assert_css(&doc, "table > tbody > tr", 4);
        assert_css(&doc, "table > tbody > tr > td", 10);

        // Ruby uses `tr:nth-child(N)` / `td:nth-child(N)`; the indexed XPath form
        // is equivalent and supported by the test DOM.
        assert_xpath(&doc, "(/table/tbody/tr)[1]/td", 4);
        assert_xpath(&doc, "(/table/tbody/tr)[2]/td", 3);
        assert_xpath(&doc, "(/table/tbody/tr)[3]/td", 1);
        assert_xpath(&doc, "(/table/tbody/tr)[4]/td", 2);

        assert_xpath(
            &doc,
            "((/table/tbody/tr)[1]/td)[1][@class=\"halign-left valign-top\"]/p/em",
            1,
        );
        assert_xpath(
            &doc,
            "((/table/tbody/tr)[1]/td)[2][@class=\"halign-right valign-top\"]/p/strong",
            1,
        );
        assert_xpath(
            &doc,
            "((/table/tbody/tr)[1]/td)[3][@class=\"halign-center valign-top\"]/p",
            1,
        );
        assert_xpath(
            &doc,
            "((/table/tbody/tr)[1]/td)[3][@class=\"halign-center valign-top\"]/p/*",
            0,
        );
        assert_xpath(
            &doc,
            "((/table/tbody/tr)[1]/td)[4][@class=\"halign-right valign-top\"]/p/strong",
            1,
        );

        assert_xpath(
            &doc,
            "((/table/tbody/tr)[2]/td)[1][@class=\"halign-center valign-top\"]/p/em",
            1,
        );
        assert_xpath(
            &doc,
            "((/table/tbody/tr)[2]/td)[2][@class=\"halign-center valign-middle\"][@colspan=\"2\"][@rowspan=\"2\"]/p/code",
            1,
        );
        assert_xpath(
            &doc,
            "((/table/tbody/tr)[2]/td)[3][@class=\"halign-left valign-bottom\"][@rowspan=\"3\"]/p/code",
            1,
        );

        assert_xpath(
            &doc,
            "((/table/tbody/tr)[3]/td)[1][@class=\"halign-center valign-top\"]/p/em",
            1,
        );

        assert_xpath(
            &doc,
            "((/table/tbody/tr)[4]/td)[1][@class=\"halign-left valign-top\"]/p",
            1,
        );
        assert_xpath(
            &doc,
            "((/table/tbody/tr)[4]/td)[1][@class=\"halign-left valign-top\"]/p/em",
            0,
        );
        assert_xpath(
            &doc,
            "((/table/tbody/tr)[4]/td)[2][@class=\"halign-right valign-top\"][@colspan=\"2\"]/p/code",
            1,
        );
    }

    #[test]
    fn sets_up_columns_correctly_if_first_row_has_cell_that_spans_columns() {
        verifies!(
            r#"
    test 'sets up columns correctly if first row has cell that spans columns' do
      input = <<~'EOS'
      |===
      2+^|AAA |CCC
      |AAA |BBB |CCC
      |AAA |BBB |CCC
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table > tbody > tr:nth-child(1) > td', output, 2
      assert_css 'table > tbody > tr:nth-child(1) > td:nth-child(1)[colspan="2"]', output, 1
      assert_css 'table > tbody > tr:nth-child(1) > td:nth-child(2):not([colspan])', output, 1
      assert_css 'table > tbody > tr:nth-child(2) > td:not([colspan])', output, 3
      assert_css 'table > tbody > tr:nth-child(3) > td:not([colspan])', output, 3
    end

"#
        );

        let doc =
            Parser::default().parse("|===\n2+^|AAA |CCC\n|AAA |BBB |CCC\n|AAA |BBB |CCC\n|===");

        assert_xpath(&doc, "(/table/tbody/tr)[1]/td", 2);
        assert_xpath(&doc, "((/table/tbody/tr)[1]/td)[1][@colspan=\"2\"]", 1);
        // Ruby uses `td:nth-child(1)[colspan]` / `td:nth-child(2):not([colspan])`;
        // since row 1 has exactly one cell carrying a colspan, asserting the row
        // has a single colspanned cell is equivalent.
        assert_xpath(&doc, "(/table/tbody/tr)[1]/td[@colspan]", 1);
        assert_xpath(&doc, "(/table/tbody/tr)[2]/td", 3);
        assert_xpath(&doc, "(/table/tbody/tr)[2]/td[@colspan]", 0);
        assert_xpath(&doc, "(/table/tbody/tr)[3]/td", 3);
        assert_xpath(&doc, "(/table/tbody/tr)[3]/td[@colspan]", 0);
    }

    #[test]
    fn supports_repeating_cells() {
        verifies!(
            r#"
    test 'supports repeating cells' do
      input = <<~'EOS'
      |===
      3*|A
      |1 3*|2
      |b |c
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 3
      assert_css 'table > tbody > tr', output, 3
      assert_css 'table > tbody > tr:nth-child(1) > td', output, 3
      assert_css 'table > tbody > tr:nth-child(2) > td', output, 3
      assert_css 'table > tbody > tr:nth-child(3) > td', output, 3

      assert_xpath '/table/tbody/tr[1]/td[1]/p[text()="A"]', output, 1
      assert_xpath '/table/tbody/tr[1]/td[2]/p[text()="A"]', output, 1
      assert_xpath '/table/tbody/tr[1]/td[3]/p[text()="A"]', output, 1

      assert_xpath '/table/tbody/tr[2]/td[1]/p[text()="1"]', output, 1
      assert_xpath '/table/tbody/tr[2]/td[2]/p[text()="2"]', output, 1
      assert_xpath '/table/tbody/tr[2]/td[3]/p[text()="2"]', output, 1

      assert_xpath '/table/tbody/tr[3]/td[1]/p[text()="2"]', output, 1
      assert_xpath '/table/tbody/tr[3]/td[2]/p[text()="b"]', output, 1
      assert_xpath '/table/tbody/tr[3]/td[3]/p[text()="c"]', output, 1
    end

"#
        );

        let doc = Parser::default().parse("|===\n3*|A\n|1 3*|2\n|b |c\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 3);
        assert_css(&doc, "table > tbody > tr", 3);
        assert_xpath(&doc, "(/table/tbody/tr)[1]/td", 3);
        assert_xpath(&doc, "(/table/tbody/tr)[2]/td", 3);
        assert_xpath(&doc, "(/table/tbody/tr)[3]/td", 3);

        assert_xpath(&doc, "/table/tbody/tr[1]/td[1]/p[text()=\"A\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[2]/p[text()=\"A\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[3]/p[text()=\"A\"]", 1);

        assert_xpath(&doc, "/table/tbody/tr[2]/td[1]/p[text()=\"1\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[2]/td[2]/p[text()=\"2\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[2]/td[3]/p[text()=\"2\"]", 1);

        assert_xpath(&doc, "/table/tbody/tr[3]/td[1]/p[text()=\"2\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[3]/td[2]/p[text()=\"b\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[3]/td[3]/p[text()=\"c\"]", 1);
    }

    // Out of scope: DocBook backend. ("calculates colnames correctly when using
    // implicit column count and single cell with colspan")
    non_normative!(
        r#"
    test 'calculates colnames correctly when using implicit column count and single cell with colspan' do
      input = <<~'EOS'
      |===
      2+|Two Columns
      |One Column |One Column
      |===
      EOS

      output = convert_string_to_embedded input, backend: 'docbook'
      assert_xpath '//colspec', output, 2
      assert_xpath '(//colspec)[1][@colname="col_1"]', output, 1
      assert_xpath '(//colspec)[2][@colname="col_2"]', output, 1
      assert_xpath '//row', output, 2
      assert_xpath '(//row)[1]/entry', output, 1
      assert_xpath '(//row)[1]/entry[@namest="col_1"][@nameend="col_2"]', output, 1
    end

"#
    );

    // Out of scope: DocBook backend. ("calculates colnames correctly when using
    // implicit column count and cells with mixed colspans")
    non_normative!(
        r#"
    test 'calculates colnames correctly when using implicit column count and cells with mixed colspans' do
      input = <<~'EOS'
      |===
      2+|Two Columns | One Column
      |One Column |One Column |One Column
      |===
      EOS

      output = convert_string_to_embedded input, backend: 'docbook'
      assert_xpath '//colspec', output, 3
      assert_xpath '(//colspec)[1][@colname="col_1"]', output, 1
      assert_xpath '(//colspec)[2][@colname="col_2"]', output, 1
      assert_xpath '(//colspec)[3][@colname="col_3"]', output, 1
      assert_xpath '//row', output, 2
      assert_xpath '(//row)[1]/entry', output, 2
      assert_xpath '(//row)[1]/entry[@namest="col_1"][@nameend="col_2"]', output, 1
      assert_xpath '(//row)[2]/entry[@namest]', output, 0
      assert_xpath '(//row)[2]/entry[@nameend]', output, 0
    end

"#
    );

    // Out of scope: DocBook backend. ("assigns unique column names for table
    // with implicit column count and colspans in first row")
    non_normative!(
        r#"
    test 'assigns unique column names for table with implicit column count and colspans in first row' do
      input = <<~'EOS'
      |===
      |                 2+| Node 0          2+| Node 1

      | Host processes    | Core 0 | Core 1   | Core 4 | Core 5
      | Guest processes   | Core 2 | Core 3   | Core 6 | Core 7
      |===
      EOS

      output = convert_string_to_embedded input, backend: 'docbook'
      assert_xpath '//colspec', output, 5
      (1..5).each do |n|
        assert_xpath %((//colspec)[#{n}][@colname="col_#{n}"]), output, 1
      end
      assert_xpath '(//row)[1]/entry', output, 3
      assert_xpath '((//row)[1]/entry)[1][@namest]', output, 0
      assert_xpath '((//row)[1]/entry)[1][@namend]', output, 0
      assert_xpath '((//row)[1]/entry)[2][@namest="col_2"][@nameend="col_3"]', output, 1
      assert_xpath '((//row)[1]/entry)[3][@namest="col_4"][@nameend="col_5"]', output, 1
    end

"#
    );

    #[test]
    fn should_drop_row_but_preserve_remaining_rows_after_cell_with_colspan_exceeds_number_of_columns()
     {
        verifies!(
            r#"
    test 'should drop row but preserve remaining rows after cell with colspan exceeds number of columns' do
      input = <<~'EOS'
      [cols=2*]
      |===
      3+|A
      |B
      a|C

      more C
      |===
      EOS
      using_memory_logger do |logger|
        output = convert_string_to_embedded input
        assert_css 'table', output, 1
        assert_css 'table tr', output, 1
        assert_xpath '/table/tbody/tr/td[1]/p[text()="B"]', output, 1
        assert_message logger, :ERROR, '<stdin>: line 3: dropping cell because it exceeds specified number of columns', Hash
      end
    end

"#
        );

        let doc = Parser::default().parse("[cols=2*]\n|===\n3+|A\n|B\na|C\n\nmore C\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table tr", 1);
        assert_xpath(&doc, "/table/tbody/tr/td[1]/p[text()=\"B\"]", 1);

        let warnings: Vec<_> = doc.warnings().collect();
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::TableCellExceedsColumnCount
        );
        assert_eq!(warnings[0].source.line(), 3);
    }

    #[test]
    fn should_drop_last_row_if_last_cell_in_table_has_colspan_that_exceeds_specified_number_of_columns()
     {
        verifies!(
            r#"
    test 'should drop last row if last cell in table has colspan that exceeds specified number of columns' do
      input = <<~'EOS'
      [cols=2*]
      |===
      |a 2+|b
      |===
      EOS
      using_memory_logger do |logger|
        output = convert_string_to_embedded input
        assert_css 'table', output, 1
        assert_css 'table *', output, 0
        assert_message logger, :ERROR, '<stdin>: line 3: dropping cell because it exceeds specified number of columns', Hash
      end
    end

"#
        );

        let doc = Parser::default().parse("[cols=2*]\n|===\n|a 2+|b\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table *", 0);

        let warnings: Vec<_> = doc.warnings().collect();
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::TableCellExceedsColumnCount
        );
        assert_eq!(warnings[0].source.line(), 3);
    }

    #[test]
    fn should_drop_last_row_if_last_cell_in_table_has_colspan_that_exceeds_implicit_number_of_columns()
     {
        verifies!(
            r#"
    test 'should drop last row if last cell in table has colspan that exceeds implicit number of columns' do
      input = <<~'EOS'
      |===
      |a |b
      |c 2+|d
      |===
      EOS
      using_memory_logger do |logger|
        output = convert_string_to_embedded input
        assert_css 'table', output, 1
        assert_css 'table tr', output, 1
        assert_xpath '/table/tbody/tr/td[1]/p[text()="a"]', output, 1
        assert_message logger, :ERROR, '<stdin>: line 3: dropping cell because it exceeds specified number of columns', Hash
      end
    end

"#
        );

        let doc = Parser::default().parse("|===\n|a |b\n|c 2+|d\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table tr", 1);
        assert_xpath(&doc, "/table/tbody/tr/td[1]/p[text()=\"a\"]", 1);

        let warnings: Vec<_> = doc.warnings().collect();
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::TableCellExceedsColumnCount
        );
        assert_eq!(warnings[0].source.line(), 3);
    }

    #[test]
    fn should_take_colspan_into_account_when_taking_cells_for_row() {
        verifies!(
            r#"
    test 'should take colspan into account when taking cells for row' do
      input = <<~'EOS'
      [cols=7]
      |===
      2+|a 2+|b 2+|c 2+|d
      |e |f |g |h |i |j |k
      |===
      EOS
      using_memory_logger do |logger|
        output = convert_string_to_embedded input
        assert_css 'table', output, 1
        assert_css 'table tr', output, 1
        assert_css 'table tr td', output, 7
        assert_message logger, :ERROR, '<stdin>: line 3: dropping cell because it exceeds specified number of columns', Hash
      end
    end

"#
        );

        let doc = Parser::default()
            .parse("[cols=7]\n|===\n2+|a 2+|b 2+|c 2+|d\n|e |f |g |h |i |j |k\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table tr", 1);
        assert_css(&doc, "table tr td", 7);

        let warnings: Vec<_> = doc.warnings().collect();
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::TableCellExceedsColumnCount
        );

        // The warning is logged against the overrunning first row (line 3).
        assert_eq!(warnings[0].source.line(), 3);
    }

    #[test]
    fn should_drop_incomplete_row_at_end_of_table_and_log_an_error() {
        verifies!(
            r#"
    test 'should drop incomplete row at end of table and log an error' do
      input = <<~'EOS'
      [cols=2*]
      |===
      |a |b
      |c |d
      |e
      |===
      EOS
      using_memory_logger do |logger|
        output = convert_string_to_embedded input
        assert_css 'table', output, 1
        assert_css 'table tr', output, 2
        assert_message logger, :ERROR, '<stdin>: line 5: dropping cells from incomplete row detected end of table', Hash
      end
    end

"#
        );

        let doc = Parser::default().parse("[cols=2*]\n|===\n|a |b\n|c |d\n|e\n|===");

        // The incomplete `|e` row is dropped, leaving two rows, and an error is
        // logged against its line (line 5).
        assert_css(&doc, "table", 1);
        assert_css(&doc, "table tr", 2);

        let warnings: Vec<_> = doc.warnings().collect();
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::TableIncompleteRowAtEndOfTable
        );
        assert_eq!(warnings[0].source.line(), 5);
    }

    #[test]
    fn should_apply_cell_style_for_column_to_repeated_content() {
        verifies!(
            r#"
    test 'should apply cell style for column to repeated content' do
      input = <<~'EOS'
      [cols=",^l"]
      |===
      |Paragraphs |Literal

      2*|The discussion about what is good,
      what is beautiful, what is noble,
      what is pure, and what is true
      could always go on.

      Why is that important?
      Why would I like to do that?

      Because that's the only conversation worth having.

      And whether it goes on or not after I die, I don't know.
      But, I do know that it is the conversation I want to have while I am still alive.

      Which means that to me the offer of certainty,
      the offer of complete security,
      the offer of an impermeable faith that can't give way
      is an offer of something not worth having.

      I want to live my life taking the risk all the time
      that I don't know anything like enough yet...
      that I haven't understood enough...
      that I can't know enough...
      that I am always hungrily operating on the margins
      of a potentially great harvest of future knowledge and wisdom.

      I wouldn't have it any other way.
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 2
      assert_css 'table > thead', output, 1
      assert_css 'table > thead > tr', output, 1
      assert_css 'table > thead > tr > th', output, 2
      assert_css 'table > tbody', output, 1
      assert_css 'table > tbody > tr', output, 1
      assert_css 'table > tbody > tr > td', output, 2
      assert_css 'table > tbody > tr > td:nth-child(1).halign-left.valign-top > p.tableblock', output, 7
      assert_css 'table > tbody > tr > td:nth-child(2).halign-center.valign-top > div.literal > pre', output, 1
      literal = xmlnodes_at_css 'table > tbody > tr > td:nth-child(2).halign-center.valign-top > div.literal > pre', output, 1
      assert_equal 26, literal.text.lines.size
    end

"#
        );

        let doc = Parser::default().parse(
            "[cols=\",^l\"]\n|===\n|Paragraphs |Literal\n\n2*|The discussion about what is good,\nwhat is beautiful, what is noble,\nwhat is pure, and what is true\ncould always go on.\n\nWhy is that important?\nWhy would I like to do that?\n\nBecause that's the only conversation worth having.\n\nAnd whether it goes on or not after I die, I don't know.\nBut, I do know that it is the conversation I want to have while I am still alive.\n\nWhich means that to me the offer of certainty,\nthe offer of complete security,\nthe offer of an impermeable faith that can't give way\nis an offer of something not worth having.\n\nI want to live my life taking the risk all the time\nthat I don't know anything like enough yet...\nthat I haven't understood enough...\nthat I can't know enough...\nthat I am always hungrily operating on the margins\nof a potentially great harvest of future knowledge and wisdom.\n\nI wouldn't have it any other way.\n|===",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > thead", 1);
        assert_css(&doc, "table > thead > tr", 1);
        assert_css(&doc, "table > thead > tr > th", 2);
        assert_css(&doc, "table > tbody", 1);
        assert_css(&doc, "table > tbody > tr", 1);
        assert_css(&doc, "table > tbody > tr > td", 2);

        // The duplicated cell (`2*|`) is cloned into both columns, and each clone
        // takes its own column's style: the first (default) column renders the
        // content as seven paragraphs, the second (`^l`) column as a single
        // centered literal block. (Ruby uses `td:nth-child(N)`; the equivalent
        // positional `td[N]` is expressed in XPath here.)
        assert_xpath(
            &doc,
            "/table/tbody/tr/td[1][@class=\"halign-left valign-top\"]/p[@class=\"tableblock\"]",
            7,
        );
        assert_xpath(
            &doc,
            "/table/tbody/tr/td[2][@class=\"halign-center valign-top\"]/div[@class=\"literal\"]/pre",
            1,
        );

        // The literal block preserves every line of the cell, including the blank
        // lines between paragraphs (26 lines total).
        let vdom = doc.to_virtual_dom();
        let pre = query_xpath(&vdom, "/table/tbody/tr/td[2]//pre");
        assert_eq!(pre.len(), 1);
        assert_eq!(
            pre[0].text.as_deref().unwrap_or_default().lines().count(),
            26
        );
    }

    #[test]
    fn should_not_split_paragraph_at_line_containing_only_blank_that_is_directly_adjacent_to_non_blank_lines()
     {
        verifies!(
            r#"
    test 'should not split paragraph at line containing only {blank} that is directly adjacent to non-blank lines' do
      input = <<~'EOS'
      |===
      |paragraph
      {blank}
      still one paragraph
      {blank}
      still one paragraph
      |===
      EOS

      result = convert_string_to_embedded input
      assert_css 'p.tableblock', result, 1
    end

"#
        );

        let doc = Parser::default().parse(
            "|===\n|paragraph\n{blank}\nstill one paragraph\n{blank}\nstill one paragraph\n|===",
        );

        // Each `{blank}` line renders as an empty line but is not blank in the
        // source, so the cell stays a single paragraph.
        assert_css(&doc, "p.tableblock", 1);
    }

    #[test]
    fn should_strip_trailing_newlines_when_splitting_paragraphs() {
        verifies!(
            r#"
    test 'should strip trailing newlines when splitting paragraphs' do
      input = <<~'EOS'
      |===
      |first wrapped
      paragraph

      second paragraph

      third paragraph
      |===
      EOS

      result = convert_string_to_embedded input
      assert_xpath %((//p[@class="tableblock"])[1][text()="first wrapped\nparagraph"]), result, 1
      assert_xpath %((//p[@class="tableblock"])[2][text()="second paragraph"]), result, 1
      assert_xpath %((//p[@class="tableblock"])[3][text()="third paragraph"]), result, 1
    end

"#
        );

        let doc = Parser::default()
            .parse("|===\n|first wrapped\nparagraph\n\nsecond paragraph\n\nthird paragraph\n|===");

        assert_xpath(
            &doc,
            "(//p[@class=\"tableblock\"])[1][text()=\"first wrapped\nparagraph\"]",
            1,
        );
        assert_xpath(
            &doc,
            "(//p[@class=\"tableblock\"])[2][text()=\"second paragraph\"]",
            1,
        );
        assert_xpath(
            &doc,
            "(//p[@class=\"tableblock\"])[3][text()=\"third paragraph\"]",
            1,
        );
    }

    #[test]
    fn basic_asciidoc_cell() {
        verifies!(
            r#"
    test 'basic AsciiDoc cell' do
      input = <<~'EOS'
      |===
      a|--
      NOTE: content

      content
      --
      |===
      EOS

      result = convert_string_to_embedded input
      assert_css 'table.tableblock', result, 1
      assert_css 'table.tableblock td.tableblock', result, 1
      assert_css 'table.tableblock td.tableblock .openblock', result, 1
      assert_css 'table.tableblock td.tableblock .openblock .admonitionblock', result, 1
      assert_css 'table.tableblock td.tableblock .openblock .paragraph', result, 1
    end

"#
        );

        let doc = Parser::default().parse("|===\na|--\nNOTE: content\n\ncontent\n--\n|===");

        assert_css(&doc, "table.tableblock", 1);
        assert_css(&doc, "table.tableblock td.tableblock", 1);
        assert_css(&doc, "table.tableblock td.tableblock .openblock", 1);
        assert_css(
            &doc,
            "table.tableblock td.tableblock .openblock .admonitionblock",
            1,
        );
        assert_css(
            &doc,
            "table.tableblock td.tableblock .openblock .paragraph",
            1,
        );
    }

    #[test]
    fn asciidoc_table_cell_should_be_wrapped_in_div_with_class_content() {
        verifies!(
            r#"
    test 'AsciiDoc table cell should be wrapped in div with class "content"' do
      input = <<~'EOS'
      |===
      a|AsciiDoc table cell
      |===
      EOS

      result = convert_string_to_embedded input
      assert_css 'table.tableblock td.tableblock > div.content', result, 1
      assert_css 'table.tableblock td.tableblock > div.content > div.paragraph', result, 1
    end

"#
        );

        let doc = Parser::default().parse("|===\na|AsciiDoc table cell\n|===");

        assert_css(&doc, "table.tableblock td.tableblock > div.content", 1);
        assert_css(
            &doc,
            "table.tableblock td.tableblock > div.content > div.paragraph",
            1,
        );
    }

    #[test]
    fn doctype_can_be_set_in_asciidoc_table_cell() {
        verifies!(
            r#"
    test 'doctype can be set in AsciiDoc table cell' do
      input = <<~'EOS'
      |===
      a|
      :doctype: inline

      content
      |===
      EOS

      result = convert_string_to_embedded input
      assert_css 'table.tableblock', result, 1
      assert_css 'table.tableblock .paragraph', result, 0
    end

"#
        );

        let doc = Parser::default().parse("|===\na|\n:doctype: inline\n\ncontent\n|===");

        // An `inline` doctype renders the cell's lone paragraph as bare inline
        // content, with no `.paragraph` wrapper.
        assert_css(&doc, "table.tableblock", 1);
        assert_css(&doc, "table.tableblock .paragraph", 0);
    }

    #[test]
    fn should_reset_doctype_to_default_in_asciidoc_table_cell() {
        verifies!(
            r#"
    test 'should reset doctype to default in AsciiDoc table cell' do
      input = <<~'EOS'
      = Book Title
      :doctype: book

      == Chapter 1

      |===
      a|
      = AsciiDoc Table Cell

      doctype={doctype}
      {backend-html5-doctype-article}
      {backend-html5-doctype-book}
      |===
      EOS

      result = convert_string_to_embedded input, attributes: { 'attribute-missing' => 'skip' }
      assert_includes result, 'doctype=article'
      refute_includes result, '{backend-html5-doctype-article}'
      assert_includes result, '{backend-html5-doctype-book}'
    end

"#
        );

        // The parent is a `book`, but a cell resets to the default `article`
        // doctype, so `{doctype}` is `article` and only the
        // `backend-html5-doctype-article` derived attribute is defined (an
        // undefined reference is left literal).
        let doc = Parser::default().parse(
            "= Book Title\n:doctype: book\n\n== Chapter 1\n\n|===\na|\n= AsciiDoc Table Cell\n\ndoctype={doctype}\n{backend-html5-doctype-article}\n{backend-html5-doctype-book}\n|===",
        );

        assert_rendered_contains(&doc, "doctype=article");
        refute_rendered_contains(&doc, "{backend-html5-doctype-article}");
        assert_rendered_contains(&doc, "{backend-html5-doctype-book}");
    }

    #[test]
    fn asciidoc_table_cell_cannot_override_the_derived_doctype_flag() {
        // The active `backend-html5-doctype-*` flag is a read-only intrinsic
        // (empty value), so a cell-body assignment to it is silently ignored:
        // `{backend-html5-doctype-article}` still resolves to the empty intrinsic
        // value rather than the cell's attempted override.
        //
        // The parent is a `book`, so the cell's active flag
        // (`backend-html5-doctype-article`, after the cell resets to the default
        // doctype) differs from the parent's — the case a lock computed from the
        // parent's doctype would miss.
        let doc = Parser::default().parse(
            "= Book Title\n:doctype: book\n\n== Chapter 1\n\n|===\na|\n= AsciiDoc Table Cell\n:backend-html5-doctype-article: custom\n\nflag=[{backend-html5-doctype-article}]\n|===",
        );

        assert_rendered_contains(&doc, "flag=[]");
        refute_rendered_contains(&doc, "custom");
    }

    #[test]
    fn asciidoc_table_cell_cannot_stash_a_derived_doctype_flag_while_it_is_inactive() {
        // Assigning `:backend-html5-doctype-article:` while the cell's doctype is
        // `book` (so that flag is not the active synthesized intrinsic) must not
        // stash a per-parser override that later shadows the intrinsic once the
        // cell switches its doctype back to `article`. The whole
        // `backend-html5-doctype-*` namespace is read-only, so the assignment is
        // ignored and `{backend-html5-doctype-article}` still resolves to the
        // empty intrinsic value.
        let doc = Parser::default().parse(
            "= Doc Title\n\n== Chapter 1\n\n|===\na|\n= AsciiDoc Table Cell\n:doctype: book\n:backend-html5-doctype-article: custom\n:doctype: article\n\nflag=[{backend-html5-doctype-article}]\n|===",
        );

        assert_rendered_contains(&doc, "flag=[]");
        refute_rendered_contains(&doc, "custom");
    }

    #[test]
    fn should_update_doctype_related_attributes_in_asciidoc_table_cell_when_doctype_is_set() {
        verifies!(
            r#"
    test 'should update doctype-related attributes in AsciiDoc table cell when doctype is set' do
      input = <<~'EOS'
      = Document Title
      :doctype: article

      == Chapter 1

      |===
      a|
      = AsciiDoc Table Cell
      :doctype: book

      doctype={doctype}
      {backend-html5-doctype-book}
      {backend-html5-doctype-article}
      |===
      EOS

      result = convert_string_to_embedded input, attributes: { 'attribute-missing' => 'skip' }
      assert_includes result, 'doctype=book'
      refute_includes result, '{backend-html5-doctype-book}'
      assert_includes result, '{backend-html5-doctype-article}'
    end

"#
        );

        // Setting `:doctype: book` in the cell updates `{doctype}` and the
        // derived `backend-html5-doctype-*` attribute accordingly.
        let doc = Parser::default().parse(
            "= Document Title\n:doctype: article\n\n== Chapter 1\n\n|===\na|\n= AsciiDoc Table Cell\n:doctype: book\n\ndoctype={doctype}\n{backend-html5-doctype-book}\n{backend-html5-doctype-article}\n|===",
        );

        assert_rendered_contains(&doc, "doctype=book");
        refute_rendered_contains(&doc, "{backend-html5-doctype-book}");
        assert_rendered_contains(&doc, "{backend-html5-doctype-article}");
    }

    #[test]
    fn should_not_allow_asciidoc_table_cell_to_set_a_document_attribute_that_was_hard_set_by_the_api()
     {
        verifies!(
            r#"
    test 'should not allow AsciiDoc table cell to set a document attribute that was hard set by the API' do
      input = <<~'EOS'
      |===
      a|
      :icons:

      NOTE: This admonition does not have a font-based icon.
      |===
      EOS

      result = convert_string_to_embedded input, safe: :safe, attributes: { 'icons' => 'font' }
      assert_css 'td.icon .title', result, 0
      assert_css 'td.icon i.icon-note', result, 1
    end

"#
        );

        // `icons` is hard set to `font` by the API, so the cell's `:icons:`
        // (which would otherwise change it) is ignored: the cell's NOTE
        // admonition still renders with a font-based icon.
        let doc = Parser::default()
            .with_intrinsic_attribute("icons", "font", ModificationContext::ApiOnly)
            .parse(
                "|===\na|\n:icons:\n\nNOTE: This admonition does not have a font-based icon.\n|===",
            );

        assert_css(&doc, "td.icon .title", 0);
        assert_css(&doc, "td.icon i.icon-note", 1);
    }

    #[test]
    fn should_not_allow_asciidoc_table_cell_to_set_a_document_attribute_that_was_hard_unset_by_the_api()
     {
        verifies!(
            r#"
    test 'should not allow AsciiDoc table cell to set a document attribute that was hard unset by the API' do
      input = <<~'EOS'
      |===
      a|
      :icons: font

      NOTE: This admonition does not have a font-based icon.
      |===
      EOS

      result = convert_string_to_embedded input, safe: :safe, attributes: { 'icons' => nil }
      assert_css 'td.icon .title', result, 1
      assert_css 'td.icon i.icon-note', result, 0
      assert_xpath '//td[@class="icon"]/*[@class="title"][text()="Note"]', result, 1
    end

"#
        );

        // `icons` is hard unset by the API, so the cell's `:icons: font` is
        // ignored: the cell's NOTE admonition renders with a text label.
        let doc = Parser::default()
            .with_intrinsic_attribute_bool("icons", false, ModificationContext::ApiOnly)
            .parse(
                "|===\na|\n:icons: font\n\nNOTE: This admonition does not have a font-based icon.\n|===",
            );

        assert_css(&doc, "td.icon .title", 1);
        assert_css(&doc, "td.icon i.icon-note", 0);
        assert_xpath(
            &doc,
            "//td[@class=\"icon\"]/*[@class=\"title\"][text()=\"Note\"]",
            1,
        );
    }

    #[test]
    fn should_keep_attribute_unset_in_asciidoc_table_cell_if_unset_in_parent_document() {
        verifies!(
            r#"
    test 'should keep attribute unset in AsciiDoc table cell if unset in parent document' do
      input = <<~'EOS'
      :!sectids:
      :!table-caption:

      == Outer Heading

      .Outer Table
      |===
      a|

      == Inner Heading

      .Inner Table
      !===
      ! table cell
      !===
      |===
      EOS

      result = convert_string_to_embedded input
      assert_xpath 'h2[id]', result, 0
      assert_xpath '//caption[text()="Outer Table"]', result, 1
      assert_xpath '//caption[text()="Inner Table"]', result, 1
    end

"#
        );

        // `sectids` and `table-caption` are unset in the parent and stay unset
        // in the cell: the headings get no id, and both the outer table and the
        // nested table render their title as a bare `<caption>`.
        let doc = Parser::default().parse(
            ":!sectids:\n:!table-caption:\n\n== Outer Heading\n\n.Outer Table\n|===\na|\n\n== Inner Heading\n\n.Inner Table\n!===\n! table cell\n!===\n|===",
        );

        assert_xpath(&doc, "//h2[@id]", 0);
        assert_xpath(&doc, "//caption[text()=\"Outer Table\"]", 1);
        assert_xpath(&doc, "//caption[text()=\"Inner Table\"]", 1);
    }

    #[test]
    fn should_allow_attribute_unset_in_parent_document_to_be_set_in_asciidoc_table_cell() {
        verifies!(
            r#"
    test 'should allow attribute unset in parent document to be set in AsciiDoc table cell' do
      input = <<~'EOS'
      :!sectids:

      == No ID

      |===
      a|

      == No ID

      :sectids:

      == Has ID
      |===
      EOS

      result = convert_string_to_embedded input
      headings = xmlnodes_at_css 'h2', result
      assert_equal 3, headings.size
      assert_nil headings[0].attr :id
      assert_nil headings[1].attr :id
      assert_equal '_has_id', (headings[2].attr :id)
    end

"#
        );

        // `sectids` is unset in the parent header (not locked), so the cell may
        // set it: the heading after `:sectids:` in the cell gets an id.
        let doc = Parser::default().parse(
            ":!sectids:\n\n== No ID\n\n|===\na|\n\n== No ID\n\n:sectids:\n\n== Has ID\n|===",
        );

        assert_css(&doc, "h2", 3);
        assert_xpath(&doc, "(//h2)[1][@id]", 0);
        assert_xpath(&doc, "(//h2)[2][@id]", 0);
        assert_xpath(&doc, "(//h2)[3][@id=\"_has_id\"]", 1);
    }

    #[test]
    fn should_not_allow_locked_attribute_unset_in_parent_document_to_be_set_in_asciidoc_table_cell()
    {
        verifies!(
            r#"
    test 'should not allow locked attribute unset in parent document to be set in AsciiDoc table cell' do
      input = <<~'EOS'
      == No ID

      |===
      a|

      == No ID

      :sectids:

      == Has ID
      |===
      EOS

      result = convert_string_to_embedded input, attributes: { 'sectids' => nil }
      headings = xmlnodes_at_css 'h2', result
      assert_equal 3, headings.size
      headings.each {|heading| assert_nil heading.attr :id }
    end

"#
        );

        // `sectids` is hard unset by the API, so the cell's `:sectids:` is
        // ignored and no heading gets an id.
        let doc = Parser::default()
            .with_intrinsic_attribute_bool("sectids", false, ModificationContext::ApiOnly)
            .parse("== No ID\n\n|===\na|\n\n== No ID\n\n:sectids:\n\n== Has ID\n|===");

        assert_css(&doc, "h2", 3);
        assert_xpath(&doc, "//h2[@id]", 0);
    }

    #[test]
    fn showtitle_can_be_enabled_in_asciidoc_table_cell_if_unset_in_parent_document() {
        verifies!(
            r#"
    test 'showtitle can be enabled in AsciiDoc table cell if unset in parent document' do
      %w(showtitle notitle).each do |name|
        input = <<~EOS
        = Document Title
        :#{name == 'showtitle' ? '!' : ''}#{name}:

        |===
        a|
        = Nested Document Title
        :#{name == 'showtitle' ? '' : '!'}#{name}:

        content
        |===
        EOS

        result = convert_string_to_embedded input
        assert_css 'h1', result, 1
        assert_css '.tableblock h1', result, 1
      end
    end

"#
        );

        // The parent hides its title; the cell shows its own. (`showtitle` and
        // `notitle` are complements, so both spellings are exercised.)
        for (parent, cell) in [(":!showtitle:", ":showtitle:"), (":notitle:", ":!notitle:")] {
            let doc = Parser::default().parse(&format!(
                "= Document Title\n{parent}\n\n|===\na|\n= Nested Document Title\n{cell}\n\ncontent\n|==="
            ));
            assert_css(&doc, "h1", 1);
            assert_css(&doc, ".tableblock h1", 1);
        }
    }

    #[test]
    fn showtitle_can_be_enabled_in_asciidoc_table_cell_if_unset_by_api() {
        verifies!(
            r#"
    test 'showtitle can be enabled in AsciiDoc table cell if unset by API' do
      %w(showtitle notitle).each do |name|
        input = <<~EOS
        = Document Title

        |===
        a|
        = Nested Document Title
        :#{name == 'showtitle' ? '' : '!'}#{name}:

        content
        |===
        EOS

        result = convert_string_to_embedded input, attributes: { name => (name == 'showtitle' ? nil : '') }
        assert_css 'h1', result, 1
        assert_css '.tableblock h1', result, 1
      end
    end

"#
        );

        for (name, value, cell) in [
            ("showtitle", false, ":showtitle:"),
            ("notitle", true, ":!notitle:"),
        ] {
            let doc = Parser::default()
                .with_intrinsic_attribute_bool(name, value, ModificationContext::ApiOnly)
                .parse(&format!(
                    "= Document Title\n\n|===\na|\n= Nested Document Title\n{cell}\n\ncontent\n|==="
                ));
            assert_css(&doc, "h1", 1);
            assert_css(&doc, ".tableblock h1", 1);
        }
    }

    #[test]
    fn showtitle_can_be_disabled_in_asciidoc_table_cell_if_set_in_parent_document() {
        verifies!(
            r#"
    test 'showtitle can be disabled in AsciiDoc table cell if set in parent document' do
      %w(showtitle notitle).each do |name|
        input = <<~EOS
        = Document Title
        :#{name == 'showtitle' ? '' : '!'}#{name}:

        |===
        a|
        = Nested Document Title
        :#{name == 'showtitle' ? '!' : ''}#{name}:

        content
        |===
        EOS

        result = convert_string_to_embedded input
        assert_css 'h1', result, 1
        assert_css '.tableblock h1', result, 0
      end
    end

"#
        );

        // The parent shows its title; the cell hides its own.
        for (parent, cell) in [(":showtitle:", ":!showtitle:"), (":!notitle:", ":notitle:")] {
            let doc = Parser::default().parse(&format!(
                "= Document Title\n{parent}\n\n|===\na|\n= Nested Document Title\n{cell}\n\ncontent\n|==="
            ));
            assert_css(&doc, "h1", 1);
            assert_css(&doc, ".tableblock h1", 0);
        }
    }

    #[test]
    fn showtitle_can_be_disabled_in_asciidoc_table_cell_if_set_by_api() {
        verifies!(
            r#"
    test 'showtitle can be disabled in AsciiDoc table cell if set by API' do
      %w(showtitle notitle).each do |name|
        input = <<~EOS
        = Document Title

        |===
        a|
        = Nested Document Title
        :#{name == 'showtitle' ? '!' : ''}#{name}:

        content
        |===
        EOS

        result = convert_string_to_embedded input, attributes: { name => (name == 'showtitle' ? '' : nil) }
        assert_css 'h1', result, 1
        assert_css '.tableblock h1', result, 0
      end
    end

"#
        );

        for (name, value, cell) in [
            ("showtitle", true, ":!showtitle:"),
            ("notitle", false, ":notitle:"),
        ] {
            let doc = Parser::default()
                .with_intrinsic_attribute_bool(name, value, ModificationContext::ApiOnly)
                .parse(&format!(
                    "= Document Title\n\n|===\na|\n= Nested Document Title\n{cell}\n\ncontent\n|==="
                ));
            assert_css(&doc, "h1", 1);
            assert_css(&doc, ".tableblock h1", 0);
        }
    }

    #[test]
    fn asciidoc_content() {
        verifies!(
            r#"
    test 'AsciiDoc content' do
      input = <<~'EOS'
      [cols="1e,1,5a"]
      |===
      |Name |Backends |Description

      |badges |xhtml11, html5 |
      Link badges ('XHTML 1.1' and 'CSS') in document footers.

      [NOTE]
      ====
      The path names of images, icons and scripts are relative path
      names to the output document not the source document.
      ====
      |[[X97]] docinfo, docinfo1, docinfo2 |All backends |
      These three attributes control which document information
      files will be included in the the header of the output file:

      docinfo:: Include `<filename>-docinfo.<ext>`
      docinfo1:: Include `docinfo.<ext>`
      docinfo2:: Include `docinfo.<ext>` and `<filename>-docinfo.<ext>`

      Where `<filename>` is the file name (sans extension) of the AsciiDoc
      input file and `<ext>` is `.html` for HTML outputs or `.xml` for
      DocBook outputs. If the input file is the standard input then the
      output file name is used.
      |===
      EOS
      doc = document_from_string input, sourcemap: true
      table = doc.blocks.first
      refute_nil table
      tbody = table.rows.body
      assert_equal 2, tbody.size
      body_cell_1_2 = tbody[0][1]
      assert_equal 5, body_cell_1_2.lineno
      body_cell_1_3 = tbody[0][2]
      refute_nil body_cell_1_3.inner_document
      assert body_cell_1_3.inner_document.nested?
      assert_equal doc, body_cell_1_3.inner_document.parent_document
      assert_equal doc.converter, body_cell_1_3.inner_document.converter
      assert_equal 5, body_cell_1_3.lineno
      assert_equal 6, body_cell_1_3.inner_document.lineno
      note = (body_cell_1_3.inner_document.find_by context: :admonition)[0]
      assert_equal 9, note.lineno
      output = doc.convert standalone: false

      # NOTE JRuby matches the table inside the admonition block if the class is not specified on the table
      assert_css 'table.tableblock > tbody > tr', output, 2
      assert_css 'table.tableblock > tbody > tr:nth-child(1) > td:nth-child(3) div.admonitionblock', output, 1
      assert_css 'table.tableblock > tbody > tr:nth-child(2) > td:nth-child(3) div.dlist', output, 1
    end

"#
        );

        let doc = Parser::default().parse(
            "[cols=\"1e,1,5a\"]\n|===\n|Name |Backends |Description\n\n|badges |xhtml11, html5 |\nLink badges.\n\n[NOTE]\n====\nThe path names are relative.\n====\n|docinfo |All backends |\nThese attributes control docinfo:\n\ndocinfo:: Include x\ndocinfo1:: Include y\n|===",
        );

        assert_css(&doc, "table.tableblock > tbody > tr", 2);
        assert_xpath(
            &doc,
            "((/table/tbody/tr)[1]/td)[3]//*[@class=\"admonitionblock\"]",
            1,
        );
        assert_xpath(&doc, "((/table/tbody/tr)[2]/td)[3]//*[@class=\"dlist\"]", 1);
    }

    #[test]
    fn should_preserve_leading_indentation_in_contents_of_asciidoc_table_cell_if_contents_starts_with_newline()
     {
        verifies!(
            r#"
    test 'should preserve leading indentation in contents of AsciiDoc table cell if contents starts with newline' do
      # NOTE cannot use single-quoted heredoc because of https://github.com/jruby/jruby/issues/4260
      input = <<~EOS
      |===
      a|
       $ command
      a| paragraph
      |===
      EOS
      doc = document_from_string input, sourcemap: true
      table = doc.blocks[0]
      tbody = table.rows.body
      assert_equal 1, table.lineno
      assert_equal 2, tbody[0][0].lineno
      assert_equal 3, tbody[0][0].inner_document.lineno
      assert_equal 4, tbody[1][0].lineno
      output = doc.convert standalone: false
      assert_css 'td', output, 2
      assert_xpath '(//td)[1]//*[@class="literalblock"]', output, 1
      assert_xpath '(//td)[2]//*[@class="paragraph"]', output, 1
      assert_xpath '(//pre)[1][text()="$ command"]', output, 1
      assert_xpath '(//p)[1][text()="paragraph"]', output, 1
    end

"#
        );

        let doc = Parser::default().parse("|===\na|\n $ command\na| paragraph\n|===");

        // Source-map line numbers: the table starts on line 1; cell 1's
        // separator is on line 2 and its inner document begins on line 3 (the
        // indented `$ command`); cell 2 is on line 4.
        let table = match doc.child_blocks().next() {
            Some(crate::blocks::Block::Table(table)) => table,
            other => panic!("expected a table block, got {other:?}"),
        };
        assert_eq!(table.span().line(), 1);

        let cell1 = &table.body_rows()[0].cells()[0];
        assert_eq!(cell1.span().line(), 2);
        let crate::blocks::TableCellContent::AsciiDoc(inner) = cell1.content() else {
            panic!("expected an AsciiDoc cell");
        };
        assert_eq!(inner.blocks()[0].span().line(), 3);

        let cell2 = &table.body_rows()[1].cells()[0];
        assert_eq!(cell2.span().line(), 4);

        assert_css(&doc, "td", 2);
        assert_xpath(&doc, "(//td)[1]//*[@class=\"literalblock\"]", 1);
        assert_xpath(&doc, "(//td)[2]//*[@class=\"paragraph\"]", 1);
        assert_xpath(&doc, "(//pre)[1][text()=\"$ command\"]", 1);
        assert_xpath(&doc, "(//p)[1][text()=\"paragraph\"]", 1);
    }

    #[test]
    fn preprocessor_directive_on_first_line_of_an_asciidoc_table_cell_should_be_processed() {
        verifies!(
            r#"
    test 'preprocessor directive on first line of an AsciiDoc table cell should be processed' do
      input = <<~'EOS'
      |===
      a|include::fixtures/include-file.adoc[]
      |===
      EOS

      output = convert_string_to_embedded input, safe: :safe, base_dir: testdir
      assert_match(/included content/, output)
    end

"#
        );

        let handler =
            InlineFileHandler::from_pairs([("fixtures/include-file.adoc", "included content")]);
        let mut parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_include_file_handler(handler);
        let doc = parser.parse("|===\na|include::fixtures/include-file.adoc[]\n|===");

        // The `include::` on the cell's first line is expanded, so the cell holds
        // the included content rather than the literal directive.
        assert_rendered_contains(&doc, "included content");
    }

    #[test]
    fn error_about_unresolved_preprocessor_directive_on_first_line_of_an_asciidoc_table_cell_should_have_correct_cursor()
     {
        verifies!(
            r#"
    test 'error about unresolved preprocessor directive on first line of an AsciiDoc table cell should have correct cursor' do
      begin
        tmp_include = Tempfile.new %w(include- .adoc)
        tmp_include_dir, tmp_include_path = File.split tmp_include.path
        tmp_include.write <<~'EOS'
        |===
        |A |B

        |text
        a|include::does-not-exist.adoc[]
        |===
        EOS
        tmp_include.close
        input = <<~EOS
        first

        include::#{tmp_include_path}[]

        last
        EOS
        using_memory_logger do |logger|
          output = convert_string_to_embedded input, safe: :safe, base_dir: tmp_include_dir
          assert_includes output, %(Unresolved directive in #{tmp_include_path})
          assert_message logger, :ERROR, %(#{tmp_include_path}: line 5: include file not found: #{File.join tmp_include_dir, 'does-not-exist.adoc'}), Hash
        end
      ensure
        tmp_include.close!
      end
    end

"#
        );

        // The table (with a cell whose first line is an unresolvable `include::`)
        // lives in `outer.adoc`, which the primary document reaches via its own
        // `include::`. The error about the unresolved cell directive must be
        // attributed to `outer.adoc` at the line the directive appears on there
        // (line 5), not to the primary document or the synthesized cell content.
        let handler = InlineFileHandler::from_pairs([(
            "outer.adoc",
            "|===\n|A |B\n\n|text\na|include::does-not-exist.adoc[]\n|===",
        )]);
        let mut parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_include_file_handler(handler);
        let doc = parser.parse("first\n\ninclude::outer.adoc[]\n\nlast");

        // The unresolved directive is replaced (in the cell) with a message that
        // names the file the directive came from.
        assert_rendered_contains(&doc, "Unresolved directive in outer.adoc");

        // A single warning is reported, and its cursor maps — through the
        // document source map — back to `outer.adoc` line 5.
        let warnings: Vec<_> = doc.warnings().collect();
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::IncludeFileNotFound("does-not-exist.adoc".to_string())
        );
        assert_eq!(
            doc.source_map()
                .original_file_and_line(warnings[0].source.line()),
            Some(crate::parser::SourceLine(Some("outer.adoc".to_string()), 5))
        );
    }

    #[test]
    fn cross_reference_link_in_an_asciidoc_table_cell_should_resolve_to_reference_in_main_document()
    {
        verifies!(
            r##"
    test 'cross reference link in an AsciiDoc table cell should resolve to reference in main document' do
      input = <<~'EOS'
      == Some

      |===
      a|See <<_more>>
      |===

      == More

      content
      EOS

      result = convert_string input
      assert_xpath '//a[@href="#_more"]', result, 1
      assert_xpath '//a[@href="#_more"][text()="More"]', result, 1
    end

"##
        );

        let doc =
            Parser::default().parse("== Some\n\n|===\na|See <<_more>>\n|===\n\n== More\n\ncontent");

        // The cross-reference inside the AsciiDoc cell resolves against the main
        // document's catalog, even though its target section is defined *after*
        // the table.
        assert_xpath(&doc, "//a[@href=\"#_more\"]", 1);
        assert_xpath(&doc, "//a[@href=\"#_more\"][text()=\"More\"]", 1);
    }

    #[test]
    fn should_discover_anchor_at_start_of_cell_and_register_it_as_a_reference() {
        verifies!(
            r##"
    test 'should discover anchor at start of cell and register it as a reference' do
      input = <<~'EOS'
      The highest peak in the Front Range is <<grays-peak>>, which tops <<mount-evans>> by just a few feet.

      [cols="1s,1"]
      |===
      |[[mount-evans,Mount Evans]]Mount Evans
      |14,271 feet

      h|[[grays-peak,Grays Peak]]
      Grays Peak
      |14,278 feet
      |===
      EOS
      doc = document_from_string input
      refs = doc.catalog[:refs]
      assert refs.key?('mount-evans')
      assert refs.key?('grays-peak')
      output = doc.convert standalone: false
      assert_xpath '(//p)[1]/a[@href="#grays-peak"][text()="Grays Peak"]', output, 1
      assert_xpath '(//p)[1]/a[@href="#mount-evans"][text()="Mount Evans"]', output, 1
      assert_xpath '(//table/tbody/tr)[1]//td//a[@id="mount-evans"]', output, 1
      assert_xpath '(//table/tbody/tr)[2]//th//a[@id="grays-peak"]', output, 1
    end

"##
        );

        let doc = Parser::default().parse(
            "The highest peak in the Front Range is <<grays-peak>>, which tops <<mount-evans>> by just a few feet.\n\n[cols=\"1s,1\"]\n|===\n|[[mount-evans,Mount Evans]]Mount Evans\n|14,271 feet\n\nh|[[grays-peak,Grays Peak]]\nGrays Peak\n|14,278 feet\n|===",
        );

        // The anchors at the start of the cells are cataloged as document
        // references.
        let catalog = doc.catalog();
        assert!(catalog.contains_id("mount-evans"));
        assert!(catalog.contains_id("grays-peak"));

        // The cross-references in the introductory paragraph resolve against
        // those cataloged anchors, using each anchor's reftext.
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

        // The anchors render as targets within their respective cells.
        assert_xpath(&doc, "(//table/tbody/tr)[1]//td//a[@id=\"mount-evans\"]", 1);
        assert_xpath(&doc, "(//table/tbody/tr)[2]//th//a[@id=\"grays-peak\"]", 1);
    }

    #[test]
    fn should_catalog_anchor_at_start_of_cell_in_implicit_header_row_when_column_has_a_style() {
        verifies!(
            r#"
    test 'should catalog anchor at start of cell in implicit header row when column has a style' do
      input = <<~'EOS'
      [cols=1a]
      |===
      |[[foo,Foo]]* not AsciiDoc

      | AsciiDoc
      |===
      EOS
      doc = document_from_string input
      refs = doc.catalog[:refs]
      assert refs.key?('foo')
    end

"#
        );

        let doc = Parser::default()
            .parse("[cols=1a]\n|===\n|[[foo,Foo]]* not AsciiDoc\n\n| AsciiDoc\n|===");

        assert!(doc.catalog().contains_id("foo"));
    }

    #[test]
    fn should_catalog_anchor_at_start_of_cell_in_explicit_header_row_when_column_has_a_style() {
        verifies!(
            r#"
    test 'should catalog anchor at start of cell in explicit header row when column has a style' do
      input = <<~'EOS'
      [%header,cols=1a]
      |===
      |[[foo,Foo]]* not AsciiDoc
      | AsciiDoc
      |===
      EOS
      doc = document_from_string input
      refs = doc.catalog[:refs]
      assert refs.key?('foo')
    end

"#
        );

        let doc = Parser::default()
            .parse("[%header,cols=1a]\n|===\n|[[foo,Foo]]* not AsciiDoc\n| AsciiDoc\n|===");

        assert!(doc.catalog().contains_id("foo"));
    }

    #[test]
    fn should_catalog_anchor_at_start_of_cell_in_first_row() {
        verifies!(
            r#"
    test 'should catalog anchor at start of cell in first row' do
      input = <<~'EOS'
      |===
      |[[foo,Foo]]foo
      | bar
      |===
      EOS
      doc = document_from_string input
      refs = doc.catalog[:refs]
      assert refs.key?('foo')
    end

"#
        );

        let doc = Parser::default().parse("|===\n|[[foo,Foo]]foo\n| bar\n|===");

        assert!(doc.catalog().contains_id("foo"));
    }

    #[test]
    fn footnotes_should_not_be_shared_between_an_asciidoc_table_cell_and_the_main_document() {
        verifies!(
            r#"
    test 'footnotes should not be shared between an AsciiDoc table cell and the main document' do
      input = <<~'EOS'
      |===
      a|AsciiDoc footnote:[A lightweight markup language.]
      |===
      EOS

      result = convert_string input
      assert_css '#_footnotedef_1', result, 1
    end

"#
        );

        let doc = Parser::default()
            .parse("|===\na|AsciiDoc footnote:[A lightweight markup language.]\n|===");

        // The footnote defined inside the cell is numbered and rendered within
        // the cell's own (nested) document.
        assert_css(&doc, "sup.footnote", 1);
        assert_css(&doc, "a#_footnoteref_1", 1);

        // It is *not* shared with the enclosing document: the main document's
        // footnote registry stays empty, so the footnote would not appear in the
        // main document's footnote list.
        assert!(
            doc.catalog().footnotes().is_empty(),
            "cell footnote leaked into the main document's registry"
        );

        // The cell instead retains and exposes its own footnote, so a renderer
        // can emit the cell-local `#footnotes` block (`#_footnotedef_1` in the
        // Ruby assertion above).
        let table = doc
            .child_blocks()
            .find_map(|block| match block {
                crate::blocks::Block::Table(table) => Some(table),
                _ => None,
            })
            .expect("expected a table block");

        let crate::blocks::TableCellContent::AsciiDoc(cell) =
            table.body_rows()[0].cells()[0].content()
        else {
            panic!("expected an AsciiDoc cell");
        };

        let footnotes = cell.footnotes();
        assert_eq!(footnotes.len(), 1);
        assert_eq!(footnotes[0].index, "1");
        assert_eq!(footnotes[0].text, "A lightweight markup language.");
    }

    // Out of scope: DocBook backend. ("callout numbers should be globally unique,
    // including AsciiDoc table cells" — asserts only against DocBook `//co` /
    // `//callout` output)
    non_normative!(
        r#"
    test 'callout numbers should be globally unique, including AsciiDoc table cells' do
      input = <<~'EOS'
      = Document Title

      == Section 1

      |===
      a|
      [source, yaml]
      ----
      key: value <1>
      ----
      <1> First callout
      |===

      == Section 2

      |===
      a|
      [source, yaml]
      ----
      key: value <1>
      ----
      <1> Second callout
      |===

      == Section 3

      [source, yaml]
      ----
      key: value <1>
      ----
      <1> Third callout
      EOS

      result = convert_string_to_embedded input, backend: 'docbook'
      conums = xmlnodes_at_xpath '//co', result
      assert_equal 3, conums.size
      ['CO1-1', 'CO2-1', 'CO3-1'].each_with_index do |conum, idx|
        assert_equal conum, conums[idx].attribute('xml:id').value
      end
      callouts = xmlnodes_at_xpath '//callout', result
      assert_equal 3, callouts.size
      ['CO1-1', 'CO2-1', 'CO3-1'].each_with_index do |callout, idx|
        assert_equal callout, callouts[idx].attribute('arearefs').value
      end
    end

"#
    );

    // Out of scope: compatibility mode is a stated limitation of this crate.
    // ("compat mode can be activated in AsciiDoc table cell")
    non_normative!(
        r#"
    test 'compat mode can be activated in AsciiDoc table cell' do
      input = <<~'EOS'
      |===
      a|
      :compat-mode:

      The word 'italic' is emphasized.
      |===
      EOS

      result = convert_string_to_embedded input
      assert_xpath '//em[text()="italic"]', result, 1
    end

"#
    );

    // Out of scope: compatibility mode is a stated limitation of this crate.
    // ("compat mode in AsciiDoc table cell inherits from parent document")
    non_normative!(
        r#"
    test 'compat mode in AsciiDoc table cell inherits from parent document' do
      input = <<~'EOS'
      :compat-mode:

      The word 'italic' is emphasized.

      [cols=1*]
      |===
      |The word 'oblique' is emphasized.
      a|
      The word 'slanted' is emphasized.
      |===

      The word 'askew' is emphasized.
      EOS

      result = convert_string_to_embedded input
      assert_xpath '//em[text()="italic"]', result, 1
      assert_xpath '//em[text()="oblique"]', result, 1
      assert_xpath '//em[text()="slanted"]', result, 1
      assert_xpath '//em[text()="askew"]', result, 1
    end

"#
    );

    // Out of scope: compatibility mode is a stated limitation of this crate.
    // ("compat mode in AsciiDoc table cell can be unset if set in parent
    // document")
    non_normative!(
        r#"
    test 'compat mode in AsciiDoc table cell can be unset if set in parent document' do
      input = <<~'EOS'
      :compat-mode:

      The word 'italic' is emphasized.

      [cols=1*]
      |===
      |The word 'oblique' is emphasized.
      a|
      :!compat-mode:

      The word 'slanted' is not emphasized.
      |===

      The word 'askew' is emphasized.
      EOS

      result = convert_string_to_embedded input
      assert_xpath '//em[text()="italic"]', result, 1
      assert_xpath '//em[text()="oblique"]', result, 1
      assert_xpath '//em[text()="slanted"]', result, 0
      assert_xpath '//em[text()="askew"]', result, 1
    end

"#
    );

    #[test]
    fn nested_table() {
        verifies!(
            r#"
    test 'nested table' do
      input = <<~'EOS'
      [cols="1,2a"]
      |===
      |Normal cell
      |Cell with nested table
      [cols="2,1"]
      !===
      !Nested table cell 1 !Nested table cell 2
      !===
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 2
      assert_css 'table table', output, 1
      assert_css 'table > tbody > tr > td:nth-child(2) table', output, 1
      assert_css 'table > tbody > tr > td:nth-child(2) table > tbody > tr > td', output, 2
    end

"#
        );

        let doc = Parser::default().parse(
            "[cols=\"1,2a\"]\n|===\n|Normal cell\n|Cell with nested table\n[cols=\"2,1\"]\n!===\n!Nested table cell 1 !Nested table cell 2\n!===\n|===",
        );

        assert_css(&doc, "table", 2);
        assert_css(&doc, "table table", 1);
        // Ruby uses `td:nth-child(2) table`; the nested table sits in the second
        // cell of the (single) outer row.
        assert_xpath(&doc, "/table/tbody/tr/td[2]//table", 1);
        assert_xpath(&doc, "/table/tbody/tr/td[2]//table/tbody/tr/td", 2);
    }

    #[test]
    fn can_set_format_of_nested_table_to_psv() {
        verifies!(
            r#"
    test 'can set format of nested table to psv' do
      input = <<~'EOS'
      [cols="2*"]
      |===
      |normal cell
      a|
      [format=psv]
      !===
      !nested cell
      !===
      |===
      EOS

      output = convert_string_to_embedded input
      assert_css 'table', output, 2
      assert_css 'table table', output, 1
      assert_css 'table > tbody > tr > td:nth-child(2) table', output, 1
      assert_css 'table > tbody > tr > td:nth-child(2) table > tbody > tr > td', output, 1
    end

"#
        );

        let doc = Parser::default().parse(
            "[cols=\"2*\"]\n|===\n|normal cell\na|\n[format=psv]\n!===\n!nested cell\n!===\n|===",
        );

        assert_css(&doc, "table", 2);
        assert_css(&doc, "table table", 1);
        assert_xpath(&doc, "/table/tbody/tr/td[2]//table", 1);
        assert_xpath(&doc, "/table/tbody/tr/td[2]//table/tbody/tr/td", 1);
    }

    #[test]
    fn asciidoc_table_cell_should_inherit_to_dir_option_from_parent_document() {
        verifies!(
            r#"
    test 'AsciiDoc table cell should inherit to_dir option from parent document' do
      doc = document_from_string <<~'EOS', parse: true, to_dir: testdir
      |===
      a|
      AsciiDoc table cell
      |===
      EOS

      nested_doc = (doc.blocks[0].find_by context: :document, traverse_documents: true)[0]
      assert nested_doc.nested?
      assert_equal doc.options[:to_dir], nested_doc.options[:to_dir]
    end

"#
        );

        // Adapted from Asciidoctor's "AsciiDoc table cell should inherit to_dir
        // option from parent document". Asciidoctor passes a `to_dir` *option*
        // when building the document and, after locating the nested cell
        // document, asserts it is `nested?` and that its `options[:to_dir]`
        // matches the parent's.
        //
        // This crate has no options bag: a document's configuration is carried by
        // its attributes, and Asciidoctor itself surfaces the `to_dir` option as
        // the `outdir` document attribute. So the faithful analog is to configure
        // the parent with an `outdir` attribute (as if from the API, the way an
        // option is supplied) and confirm the nested AsciiDoc-cell document both
        // reports itself as nested and has inherited that value.
        let doc = Parser::default()
            .with_intrinsic_attribute("outdir", "/path/to/output", ModificationContext::ApiOnly)
            .parse("|===\na|\nAsciiDoc table cell\n|===");

        // Locate the nested document: the single table's single cell, which is an
        // AsciiDoc cell (`a|`) and therefore a nested, standalone document. This
        // is the crate's equivalent of Ruby's
        // `doc.blocks[0].find_by context: :document, traverse_documents: true`.
        let table = match doc.child_blocks().next() {
            Some(crate::blocks::Block::Table(table)) => table,
            other => panic!("expected a table block, got {other:?}"),
        };
        let crate::blocks::TableCellContent::AsciiDoc(nested_doc) =
            table.body_rows()[0].cells()[0].content()
        else {
            panic!("expected an AsciiDoc cell");
        };

        // `assert nested_doc.nested?`
        assert!(nested_doc.is_nested());

        // `assert_equal doc.options[:to_dir], nested_doc.options[:to_dir]` — the
        // inherited option (here the `outdir` attribute) is visible on the nested
        // document and equals the parent's.
        assert!(nested_doc.is_attribute_set("outdir"));
        assert_eq!(
            nested_doc.attribute_value("outdir"),
            doc.attribute_value("outdir")
        );
    }

    #[test]
    fn asciidoc_table_cell_should_not_inherit_toc_setting_from_parent_document() {
        verifies!(
            r#"
    test 'AsciiDoc table cell should not inherit toc setting from parent document' do
      input = <<~'EOS'
      = Document Title
      :toc:

      == Section

      |===
      a|
      == Section in Nested Document

      content
      |===
      EOS

      output = convert_string input
      assert_css '.toc', output, 1
      assert_css 'table .toc', output, 0
    end

"#
        );

        let doc = Parser::default().parse(
            "= Document Title\n:toc:\n\n== Section\n\n|===\na|\n== Section in Nested Document\n\ncontent\n|===",
        );

        // The outer document renders one TOC; the nested cell does not inherit
        // the `toc` setting, so no TOC appears inside the table.
        assert_css(&doc, ".toc", 1);
        assert_css(&doc, "table .toc", 0);
    }

    #[test]
    fn should_be_able_to_enable_toc_in_an_asciidoc_table_cell() {
        verifies!(
            r#"
    test 'should be able to enable toc in an AsciiDoc table cell' do
      input = <<~'EOS'
      = Document Title

      == Section A

      |===
      a|
      = Subdocument Title
      :toc:

      == Subdocument Section A

      content
      |===
      EOS

      output = convert_string input
      assert_css '.toc', output, 1
      assert_css 'table .toc', output, 1
    end

"#
        );

        let doc = Parser::default().parse(
            "= Document Title\n\n== Section A\n\n|===\na|\n= Subdocument Title\n:toc:\n\n== Subdocument Section A\n\ncontent\n|===",
        );

        // The outer document has no TOC; the cell enables its own, so the single
        // TOC is the one inside the table.
        assert_css(&doc, ".toc", 1);
        assert_css(&doc, "table .toc", 1);
    }

    #[test]
    fn should_be_able_to_enable_toc_in_an_asciidoc_table_cell_even_if_hard_unset_by_api() {
        verifies!(
            r#"
    test 'should be able to enable toc in an AsciiDoc table cell even if hard unset by API' do
      input = <<~'EOS'
      = Document Title

      == Section A

      |===
      a|
      = Subdocument Title
      :toc:

      == Subdocument Section A

      content
      |===
      EOS

      output = convert_string input, attributes: { 'toc' => nil }
      assert_css '.toc', output, 1
      assert_css 'table .toc', output, 1
    end

"#
        );

        // Hard-unsetting `toc` through the API (Ruby's `toc => nil`) suppresses
        // the outer document's TOC but does not prevent the cell — a standalone
        // nested document, and `toc` is one of the attributes a cell may set —
        // from enabling its own.
        let doc = Parser::default()
            .with_intrinsic_attribute_bool("toc", false, ModificationContext::ApiOnly)
            .parse(
                "= Document Title\n\n== Section A\n\n|===\na|\n= Subdocument Title\n:toc:\n\n== Subdocument Section A\n\ncontent\n|===",
            );

        assert_css(&doc, ".toc", 1);
        assert_css(&doc, "table .toc", 1);
    }

    #[test]
    fn should_be_able_to_enable_toc_in_both_outer_document_and_in_an_asciidoc_table_cell() {
        verifies!(
            r#"
    test 'should be able to enable toc in both outer document and in an AsciiDoc table cell' do
      input = <<~'EOS'
      = Document Title
      :toc:

      == Section A

      |===
      a|
      = Subdocument Title
      :toc: macro

      [#table-cell-toc]
      toc::[]

      == Subdocument Section A

      content
      |===
      EOS

      output = convert_string input
      assert_css '.toc', output, 2
      assert_css '#toc', output, 1
      assert_css 'table .toc', output, 1
      assert_css 'table #table-cell-toc', output, 1
    end

"#
        );

        let doc = Parser::default().parse(
            "= Document Title\n:toc:\n\n== Section A\n\n|===\na|\n= Subdocument Title\n:toc: macro\n\n[#table-cell-toc]\ntoc::[]\n\n== Subdocument Section A\n\ncontent\n|===",
        );

        // Two TOCs: the outer document's automatic TOC (`#toc`) and the cell's
        // `toc::[]` macro TOC (carrying the `#table-cell-toc` id).
        assert_css(&doc, ".toc", 2);
        assert_css(&doc, "#toc", 1);
        assert_css(&doc, "table .toc", 1);
        assert_css(&doc, "table #table-cell-toc", 1);
    }

    #[test]
    fn document_in_an_asciidoc_table_cell_should_not_see_doctitle_of_parent() {
        verifies!(
            r#"
    test 'document in an AsciiDoc table cell should not see doctitle of parent' do
      input = <<~'EOS'
      = Document Title

      [cols="1a"]
      |===
      |AsciiDoc content
      |===
      EOS

      output = convert_string input
      assert_css 'table', output, 1
      assert_css 'table > tbody > tr > td', output, 1
      assert_css 'table > tbody > tr > td #preamble', output, 0
      assert_css 'table > tbody > tr > td .paragraph', output, 1
    end

"#
        );

        let doc = Parser::default()
            .parse("= Document Title\n\n[cols=\"1a\"]\n|===\n|AsciiDoc content\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > tbody > tr > td", 1);

        // The cell is its own (nested) document and does not inherit the parent's
        // doctitle, so its lone paragraph is not promoted into a preamble: the
        // cell renders a bare `.paragraph`, never a `#preamble`.
        assert_css(&doc, "table > tbody > tr > td #preamble", 0);
        assert_css(&doc, "table > tbody > tr > td .paragraph", 1);
    }

    // Not implemented (by design): per-cell background colors depend on the
    // `{set:cellbgcolor:...}` / `{set:cellbgcolor!}` inline attribute-entry
    // reference, a discouraged syntax likely to be removed from the AsciiDoc
    // language; this crate has decided not to implement it (see the "No planned
    // support for some AsciiDoc features" section of the README). ("cell
    // background color")
    non_normative!(
        r#"
    test 'cell background color' do
      input = <<~'EOS'
      [cols="1e,1", options="header"]
      |===
      |{set:cellbgcolor:green}green
      |{set:cellbgcolor!}
      plain
      |{set:cellbgcolor:red}red
      |{set:cellbgcolor!}
      plain
      |===
      EOS

      output = convert_string_to_embedded input
      assert_xpath '(/table/thead/tr/th)[1][@style="background-color: green;"]', output, 1
      assert_xpath '(/table/thead/tr/th)[2][@style="background-color: green;"]', output, 0
      assert_xpath '(/table/tbody/tr/td)[1][@style="background-color: red;"]', output, 1
      assert_xpath '(/table/tbody/tr/td)[2][@style="background-color: green;"]', output, 0
    end

"#
    );

    #[test]
    fn should_warn_if_table_block_is_not_terminated() {
        verifies!(
            r#"
    test 'should warn if table block is not terminated' do
      input = <<~'EOS'
      outside

      |===
      |
      inside

      still inside

      eof
      EOS

      using_memory_logger do |logger|
        output = convert_string_to_embedded input
        assert_xpath '/table', output, 1
        assert_message logger, :WARN, '<stdin>: line 3: unterminated table block', Hash
      end
    end

"#
        );

        let doc = Parser::default().parse("outside\n\n|===\n|\ninside\n\nstill inside\n\neof");

        assert_xpath(&doc, "/table", 1);

        let warnings: Vec<_> = doc.warnings().collect();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].warning, WarningType::UnterminatedDelimitedBlock);
        assert_eq!(warnings[0].source.line(), 3);
    }

    #[test]
    // An unterminated example block inside an AsciiDoc table cell that is itself
    // attached to a list item — Asciidoctor reports the warning at the inner
    // block's line (9). The cursor/line of nested-cell warnings is tracked via
    // the nested-cell source-map work landed for #542.
    fn should_show_correct_line_number_in_warning_about_unterminated_block_inside_asciidoc_table_cell()
     {
        verifies!(
            r#"
    test 'should show correct line number in warning about unterminated block inside AsciiDoc table cell' do
      input = <<~'EOS'
      outside

      * list item
      +
      |===
      |cell
      a|inside

      ====
      unterminated example block
      |===

      eof
      EOS

      using_memory_logger do |logger|
        output = convert_string_to_embedded input
        assert_xpath '//ul//table', output, 1
        assert_message logger, :WARN, '<stdin>: line 9: unterminated example block', Hash
      end
    end

"#
        );

        let doc = Parser::default().parse(
            "outside\n\n* list item\n+\n|===\n|cell\na|inside\n\n====\nunterminated example block\n|===\n\neof",
        );

        assert_xpath(&doc, "//ul//table", 1);

        // Exactly one warning is expected: the unterminated example block,
        // attributed to line 9 (where `====` opens inside the cell).
        let warnings: Vec<_> = doc.warnings().collect();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].warning, WarningType::UnterminatedDelimitedBlock);
        assert_eq!(warnings[0].source.line(), 9);
    }

    #[test]
    fn custom_separator_for_an_asciidoc_table_cell() {
        verifies!(
            r#"
    test 'custom separator for an AsciiDoc table cell' do
      input = <<~'EOS'
      [cols=2,separator=!]
      |===
      !Pipe output to vim
      a!
      ----
      asciidoctor -o - -s test.adoc | view -
      ----
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 2
      assert_css 'table > tbody > tr', output, 1
      assert_css 'table > tbody > tr:nth-child(1) > td', output, 2
      assert_css 'table > tbody > tr:nth-child(1) > td:nth-child(1) p', output, 1
      assert_css 'table > tbody > tr:nth-child(1) > td:nth-child(2) .listingblock', output, 1
    end

"#
        );

        let doc = Parser::default().parse(
            "[cols=2,separator=!]\n|===\n!Pipe output to vim\na!\n----\nasciidoctor -o - -s test.adoc | view -\n----\n|===",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > tbody > tr", 1);
        assert_xpath(&doc, "(/table/tbody/tr)[1]/td", 2);
        assert_xpath(&doc, "((/table/tbody/tr)[1]/td)[1]//p", 1);
        assert_xpath(
            &doc,
            "((/table/tbody/tr)[1]/td)[2]//*[@class=\"listingblock\"]",
            1,
        );
    }

    // Out of scope: DocBook backend. ("table with breakable option docbook 5")
    non_normative!(
        r#"
    test 'table with breakable option docbook 5' do
      input = <<~'EOS'
      .Table with breakable
      [%breakable]
      |===
      |Item       |Quantity
      |Item 1     |1
      |===
      EOS
      output = convert_string_to_embedded input, backend: 'docbook5'
      assert_includes output, '<?dbfo keep-together="auto"?>'
    end

"#
    );

    // Out of scope: DocBook backend. ("table with unbreakable option docbook 5")
    non_normative!(
        r#"
    test 'table with unbreakable option docbook 5' do
      input = <<~'EOS'
      .Table with unbreakable
      [%unbreakable]
      |===
      |Item       |Quantity
      |Item 1     |1
      |===
      EOS
      output = convert_string_to_embedded input, backend: 'docbook5'
      assert_includes output, '<?dbfo keep-together="always"?>'
    end

"#
    );

    #[test]
    fn no_implicit_header_row_if_cell_in_first_line_is_quoted_and_spans_multiple_lines() {
        verifies!(
            r#"
    test 'no implicit header row if cell in first line is quoted and spans multiple lines' do
      input = <<~'EOS'
      [cols=2*l]
      ,===
      "A1

      A1 continued",B1
      A2,B2
      ,===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 2
      assert_css 'table > thead', output, 0
      assert_css 'table > tbody', output, 1
      assert_css 'table > tbody > tr', output, 2
      assert_xpath %((//td)[1]//pre[text()="A1\n\nA1 continued"]), output, 1
    end
"#
        );

        let doc =
            Parser::default().parse("[cols=2*l]\n,===\n\"A1\n\nA1 continued\",B1\nA2,B2\n,===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > thead", 0);
        assert_css(&doc, "table > tbody", 1);
        assert_css(&doc, "table > tbody > tr", 2);
        assert_xpath(&doc, "(//td)[1]//pre[text()=\"A1\n\nA1 continued\"]", 1);
    }

    non_normative!(
        r#"
  end
"#
    );
}

mod dsv {
    use crate::tests::prelude::*;

    non_normative!(
        r#"

  context 'DSV' do
"#
    );

    #[test]
    fn converts_simple_dsv_table() {
        verifies!(
            r#"
    test 'converts simple dsv table' do
      input = <<~'EOS'
      [width="75%",format="dsv"]
      |===
      root:x:0:0:root:/root:/bin/bash
      bin:x:1:1:bin:/bin:/sbin/nologin
      mysql:x:27:27:MySQL\:Server:/var/lib/mysql:/bin/bash
      gdm:x:42:42::/var/lib/gdm:/sbin/nologin
      sshd:x:74:74:Privilege-separated SSH:/var/empty/sshd:/sbin/nologin
      nobody:x:99:99:Nobody:/:/sbin/nologin
      |===
      EOS
      doc = document_from_string input, standalone: false
      table = doc.blocks[0]
      assert_equal 100, table.columns.map {|col| col.attributes['colpcwidth'] }.reduce(:+)
      output = doc.convert
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col[style*="width: 14.2857%"]', output, 6
      assert_css 'table > colgroup > col:last-of-type[style*="width: 14.2858%"]', output, 1
      assert_css 'table > tbody > tr', output, 6
      assert_xpath '//tr[4]/td[5]/p/text()', output, 0
      assert_xpath '//tr[3]/td[5]/p[text()="MySQL:Server"]', output, 1
    end

"#
        );

        let doc = Parser::default().parse(
            "[width=\"75%\",format=\"dsv\"]\n|===\nroot:x:0:0:root:/root:/bin/bash\nbin:x:1:1:bin:/bin:/sbin/nologin\nmysql:x:27:27:MySQL\\:Server:/var/lib/mysql:/bin/bash\ngdm:x:42:42::/var/lib/gdm:/sbin/nologin\nsshd:x:74:74:Privilege-separated SSH:/var/empty/sshd:/sbin/nologin\nnobody:x:99:99:Nobody:/:/sbin/nologin\n|===",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col[width=\"14.2857%\"]", 6);
        // Ruby uses `col:last-of-type`; the indexed XPath form is equivalent.
        assert_xpath(&doc, "(/table/colgroup/col)[7][@width=\"14.2858%\"]", 1);
        assert_css(&doc, "table > tbody > tr", 6);
        assert_xpath(&doc, "//tr[4]/td[5]/p/text()", 0);
        assert_xpath(&doc, "//tr[3]/td[5]/p[text()=\"MySQL:Server\"]", 1);
    }

    #[test]
    fn dsv_format_shorthand() {
        verifies!(
            r#"
    test 'dsv format shorthand' do
      input = <<~'EOS'
      :===
      a:b:c
      1:2:3
      :===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 3
      assert_css 'table > tbody > tr', output, 2
      assert_css 'table > tbody > tr:nth-child(1) > td', output, 3
      assert_css 'table > tbody > tr:nth-child(2) > td', output, 3
    end

"#
        );

        let doc = Parser::default().parse(":===\na:b:c\n1:2:3\n:===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 3);
        assert_css(&doc, "table > tbody > tr", 2);
        assert_xpath(&doc, "(/table/tbody/tr)[1]/td", 3);
        assert_xpath(&doc, "(/table/tbody/tr)[2]/td", 3);
    }

    #[test]
    fn single_cell_in_dsv_table_should_only_produce_single_row() {
        verifies!(
            r#"
    test 'single cell in DSV table should only produce single row' do
      input = <<~'EOS'
      :===
      single cell
      :===
      EOS

      output = convert_string_to_embedded input
      assert_css 'table td', output, 1
    end

"#
        );

        let doc = Parser::default().parse(":===\nsingle cell\n:===");

        assert_css(&doc, "table td", 1);
    }

    #[test]
    fn should_treat_trailing_colon_as_an_empty_cell() {
        verifies!(
            r#"
    test 'should treat trailing colon as an empty cell' do
      input = <<~'EOS'
      :===
      A1:
      B1:B2
      C1:C2
      :===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 2
      assert_css 'table > tbody > tr', output, 3
      assert_xpath '/table/tbody/tr[1]/td', output, 2
      assert_xpath '/table/tbody/tr[1]/td[1]/p[text()="A1"]', output, 1
      assert_xpath '/table/tbody/tr[1]/td[2]/p', output, 0
      assert_xpath '/table/tbody/tr[2]/td[1]/p[text()="B1"]', output, 1
    end
"#
        );

        let doc = Parser::default().parse(":===\nA1:\nB1:B2\nC1:C2\n:===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > tbody > tr", 3);
        assert_xpath(&doc, "/table/tbody/tr[1]/td", 2);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[1]/p[text()=\"A1\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[2]/p", 0);
        assert_xpath(&doc, "/table/tbody/tr[2]/td[1]/p[text()=\"B1\"]", 1);
    }

    non_normative!(
        r#"
  end
"#
    );
}

mod csv {
    use crate::tests::prelude::{inline_file_handler::InlineFileHandler, *};

    non_normative!(
        r#"

  context 'CSV' do
"#
    );

    #[test]
    fn should_treat_trailing_comma_as_an_empty_cell() {
        verifies!(
            r#"
    test 'should treat trailing comma as an empty cell' do
      input = <<~'EOS'
      ,===
      A1,
      B1,B2
      C1,C2
      ,===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 2
      assert_css 'table > tbody > tr', output, 3
      assert_xpath '/table/tbody/tr[1]/td', output, 2
      assert_xpath '/table/tbody/tr[1]/td[1]/p[text()="A1"]', output, 1
      assert_xpath '/table/tbody/tr[1]/td[2]/p', output, 0
      assert_xpath '/table/tbody/tr[2]/td[1]/p[text()="B1"]', output, 1
    end

"#
        );

        let doc = Parser::default().parse(",===\nA1,\nB1,B2\nC1,C2\n,===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > tbody > tr", 3);
        assert_xpath(&doc, "/table/tbody/tr[1]/td", 2);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[1]/p[text()=\"A1\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[2]/p", 0);
        assert_xpath(&doc, "/table/tbody/tr[2]/td[1]/p[text()=\"B1\"]", 1);
    }

    #[test]
    fn should_log_error_but_not_crash_if_cell_data_has_unclosed_quote() {
        verifies!(
            r#"
    test 'should log error but not crash if cell data has unclosed quote' do
      input = <<~'EOS'
      ,===
      a,b
      c,"
      ,===
      EOS
      using_memory_logger do |logger|
        output = convert_string_to_embedded input
        assert_css 'table', output, 1
        assert_css 'table td', output, 4
        assert_xpath '(/table/td)[4]/p', output, 0
        assert_message logger, :ERROR, '<stdin>: line 3: unclosed quote in CSV data; setting cell to empty', Hash
      end
    end

"#
        );

        let doc = Parser::default().parse(",===\na,b\nc,\"\n,===");

        // The unclosed quote recovers to an empty cell without crashing, and an
        // error is logged for line 3 (the `c,"` line).
        assert_css(&doc, "table", 1);
        assert_css(&doc, "table td", 4);
        assert_xpath(&doc, "(//td)[4]/p", 0);

        let warnings: Vec<_> = doc.warnings().collect();
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::TableCsvDataHasUnclosedQuote
        );
        assert_eq!(warnings[0].source.line(), 3);
    }

    #[test]
    fn should_preserve_newlines_in_quoted_csv_values() {
        verifies!(
            r#"
    test 'should preserve newlines in quoted CSV values' do
      input = <<~'EOS'
      [cols="1,1,1l"]
      ,===
      "A
      B
      C","one

      two

      three","do

      re

      me"
      ,===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 3
      assert_css 'table > tbody > tr', output, 1
      assert_xpath '/table/tbody/tr[1]/td', output, 3
      assert_xpath %(/table/tbody/tr[1]/td[1]/p[text()="A\nB\nC"]), output, 1
      assert_xpath '/table/tbody/tr[1]/td[2]/p', output, 3
      assert_xpath '/table/tbody/tr[1]/td[2]/p[1][text()="one"]', output, 1
      assert_xpath '/table/tbody/tr[1]/td[2]/p[2][text()="two"]', output, 1
      assert_xpath '/table/tbody/tr[1]/td[2]/p[3][text()="three"]', output, 1
      assert_xpath %(/table/tbody/tr[1]/td[3]//pre[text()="do\n\nre\n\nme"]), output, 1
    end

"#
        );

        let doc = Parser::default().parse(
            "[cols=\"1,1,1l\"]\n,===\n\"A\nB\nC\",\"one\n\ntwo\n\nthree\",\"do\n\nre\n\nme\"\n,===",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 3);
        assert_css(&doc, "table > tbody > tr", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td", 3);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[1]/p[text()=\"A\nB\nC\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[2]/p", 3);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[2]/p[1][text()=\"one\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[2]/p[2][text()=\"two\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[2]/p[3][text()=\"three\"]", 1);
        assert_xpath(
            &doc,
            "/table/tbody/tr[1]/td[3]//pre[text()=\"do\n\nre\n\nme\"]",
            1,
        );
    }

    #[test]
    fn should_not_drop_trailing_empty_cell_in_tsv_data_when_loaded_from_an_include_file() {
        verifies!(
            r#"
    test 'should not drop trailing empty cell in TSV data when loaded from an include file' do
      input  = <<~'EOS'
      [%header,format=tsv]
      |===
      include::fixtures/data.tsv[]
      |===
      EOS
      output = convert_string_to_embedded input, safe: :safe, base_dir: ASCIIDOCTOR_TEST_DIR
      assert_css 'table > tbody > tr', output, 3
      assert_css 'table > tbody > tr:nth-child(1) > td', output, 3
      assert_css 'table > tbody > tr:nth-child(2) > td', output, 3
      assert_css 'table > tbody > tr:nth-child(3) > td', output, 3
      assert_css 'table > tbody > tr:nth-child(2) > td:nth-child(3):empty', output, 1
    end

"#
        );

        // The `1\t2\t` record ends with a tab, so its third field is empty. The
        // include is the delivery mechanism; the behavior under test is that the
        // trailing empty cell survives.
        let handler = InlineFileHandler::from_pairs([(
            "fixtures/data.tsv",
            "First\tSecond\tThird\na\tb\tc\n1\t2\t\nx\ty\tz\n",
        )]);
        let mut parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_include_file_handler(handler);
        let doc = parser.parse("[%header,format=tsv]\n|===\ninclude::fixtures/data.tsv[]\n|===");

        assert_css(&doc, "table > tbody > tr", 3);
        assert_css(&doc, "table > tbody > tr:nth-child(1) > td", 3);
        assert_css(&doc, "table > tbody > tr:nth-child(2) > td", 3);
        assert_css(&doc, "table > tbody > tr:nth-child(3) > td", 3);
        assert_css(
            &doc,
            "table > tbody > tr:nth-child(2) > td:nth-child(3):empty",
            1,
        );
    }

    #[test]
    fn mixed_unquoted_records_and_quoted_records_with_escaped_quotes_commas_and_wrapped_lines() {
        verifies!(
            r#"
    test 'mixed unquoted records and quoted records with escaped quotes, commas, and wrapped lines' do
      input = <<~'EOS'
      [format="csv",options="header"]
      |===
      Year,Make,Model,Description,Price
      1997,Ford,E350,"ac, abs, moon",3000.00
      1999,Chevy,"Venture ""Extended Edition""","",4900.00
      1999,Chevy,"Venture ""Extended Edition, Very Large""",,5000.00
      1996,Jeep,Grand Cherokee,"MUST SELL!
      air, moon roof, loaded",4799.00
      2000,Toyota,Tundra,"""This one's gonna to blow you're socks off,"" per the sticker",10000.00
      2000,Toyota,Tundra,"Check it, ""this one's gonna to blow you're socks off"", per the sticker",10000.00
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col[style*="width: 20%"]', output, 5
      assert_css 'table > thead > tr', output, 1
      assert_css 'table > tbody > tr', output, 6
      assert_xpath '((//tbody/tr)[1]/td)[4]/p[text()="ac, abs, moon"]', output, 1
      assert_xpath %(((//tbody/tr)[2]/td)[3]/p[text()='Venture "Extended Edition"']), output, 1
      assert_xpath %(((//tbody/tr)[4]/td)[4]/p[text()="MUST SELL!\nair, moon roof, loaded"]), output, 1
      assert_xpath %(((//tbody/tr)[5]/td)[4]/p[text()='"This one#{decode_char 8217}s gonna to blow you#{decode_char 8217}re socks off," per the sticker']), output, 1
      assert_xpath %(((//tbody/tr)[6]/td)[4]/p[text()='Check it, "this one#{decode_char 8217}s gonna to blow you#{decode_char 8217}re socks off", per the sticker']), output, 1
    end

"#
        );

        let doc = Parser::default().parse(
            "[format=\"csv\",options=\"header\"]\n|===\nYear,Make,Model,Description,Price\n1997,Ford,E350,\"ac, abs, moon\",3000.00\n1999,Chevy,\"Venture \"\"Extended Edition\"\"\",\"\",4900.00\n1999,Chevy,\"Venture \"\"Extended Edition, Very Large\"\"\",,5000.00\n1996,Jeep,Grand Cherokee,\"MUST SELL!\nair, moon roof, loaded\",4799.00\n2000,Toyota,Tundra,\"\"\"This one's gonna to blow you're socks off,\"\" per the sticker\",10000.00\n2000,Toyota,Tundra,\"Check it, \"\"this one's gonna to blow you're socks off\"\", per the sticker\",10000.00\n|===",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col[width=\"20%\"]", 5);
        assert_css(&doc, "table > thead > tr", 1);
        assert_css(&doc, "table > tbody > tr", 6);
        assert_xpath(
            &doc,
            "((//tbody/tr)[1]/td)[4]/p[text()=\"ac, abs, moon\"]",
            1,
        );
        assert_xpath(
            &doc,
            "((//tbody/tr)[2]/td)[3]/p[text()='Venture \"Extended Edition\"']",
            1,
        );
        assert_xpath(
            &doc,
            "((//tbody/tr)[4]/td)[4]/p[text()=\"MUST SELL!\nair, moon roof, loaded\"]",
            1,
        );
        assert_xpath(
            &doc,
            "((//tbody/tr)[5]/td)[4]/p[text()='\"This one\u{2019}s gonna to blow you\u{2019}re socks off,\" per the sticker']",
            1,
        );
        assert_xpath(
            &doc,
            "((//tbody/tr)[6]/td)[4]/p[text()='Check it, \"this one\u{2019}s gonna to blow you\u{2019}re socks off\", per the sticker']",
            1,
        );
    }

    #[test]
    fn should_allow_quotes_around_a_csv_value_to_be_on_their_own_lines() {
        verifies!(
            r#"
    test 'should allow quotes around a CSV value to be on their own lines' do
      input = <<~'EOS'
      [cols=2*]
      ,===
      "
      A
      ","
      B
      "
      ,===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 2
      assert_css 'table > tbody > tr', output, 1
      assert_xpath '/table/tbody/tr[1]/td', output, 2
      assert_xpath '/table/tbody/tr[1]/td[1]/p[text()="A"]', output, 1
      assert_xpath '/table/tbody/tr[1]/td[2]/p[text()="B"]', output, 1
    end

"#
        );

        let doc = Parser::default().parse("[cols=2*]\n,===\n\"\nA\n\",\"\nB\n\"\n,===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > tbody > tr", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td", 2);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[1]/p[text()=\"A\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[2]/p[text()=\"B\"]", 1);
    }

    #[test]
    fn csv_format_shorthand() {
        verifies!(
            r#"
    test 'csv format shorthand' do
      input = <<~'EOS'
      ,===
      a,b,c
      1,2,3
      ,===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 3
      assert_css 'table > tbody > tr', output, 2
      assert_css 'table > tbody > tr:nth-child(1) > td', output, 3
      assert_css 'table > tbody > tr:nth-child(2) > td', output, 3
    end

"#
        );

        let doc = Parser::default().parse(",===\na,b,c\n1,2,3\n,===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 3);
        assert_css(&doc, "table > tbody > tr", 2);
        assert_xpath(&doc, "(/table/tbody/tr)[1]/td", 3);
        assert_xpath(&doc, "(/table/tbody/tr)[2]/td", 3);
    }

    #[test]
    fn tsv_as_format() {
        verifies!(
            r#"
    test 'tsv as format' do
      input = <<~EOS
      [format=tsv]
      ,===
      a\tb\tc
      1\t2\t3
      ,===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 3
      assert_css 'table > tbody > tr', output, 2
      assert_css 'table > tbody > tr:nth-child(1) > td', output, 3
      assert_css 'table > tbody > tr:nth-child(2) > td', output, 3
    end

"#
        );

        let doc = Parser::default().parse("[format=tsv]\n,===\na\tb\tc\n1\t2\t3\n,===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 3);
        assert_css(&doc, "table > tbody > tr", 2);
        assert_xpath(&doc, "(/table/tbody/tr)[1]/td", 3);
        assert_xpath(&doc, "(/table/tbody/tr)[2]/td", 3);
    }

    #[test]
    fn custom_csv_separator() {
        verifies!(
            r#"
    test 'custom csv separator' do
      input = <<~'EOS'
      [format=csv,separator=;]
      |===
      a;b;c
      1;2;3
      |===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 3
      assert_css 'table > tbody > tr', output, 2
      assert_css 'table > tbody > tr:nth-child(1) > td', output, 3
      assert_css 'table > tbody > tr:nth-child(2) > td', output, 3
    end

"#
        );

        let doc = Parser::default().parse("[format=csv,separator=;]\n|===\na;b;c\n1;2;3\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 3);
        assert_css(&doc, "table > tbody > tr", 2);
        assert_xpath(&doc, "(/table/tbody/tr)[1]/td", 3);
        assert_xpath(&doc, "(/table/tbody/tr)[2]/td", 3);
    }

    #[test]
    fn tab_as_separator() {
        verifies!(
            r#"
    test 'tab as separator' do
      input = <<~EOS
      [separator=\\t]
      ,===
      a\tb\tc
      1\t2\t3
      ,===
      EOS
      output = convert_string_to_embedded input
      assert_css 'table', output, 1
      assert_css 'table > colgroup > col', output, 3
      assert_css 'table > tbody > tr', output, 2
      assert_css 'table > tbody > tr:nth-child(1) > td', output, 3
      assert_css 'table > tbody > tr:nth-child(2) > td', output, 3
    end

"#
        );

        let doc = Parser::default().parse("[separator=\\t]\n,===\na\tb\tc\n1\t2\t3\n,===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 3);
        assert_css(&doc, "table > tbody > tr", 2);
        assert_xpath(&doc, "(/table/tbody/tr)[1]/td", 3);
        assert_xpath(&doc, "(/table/tbody/tr)[2]/td", 3);
    }

    #[test]
    fn single_cell_in_csv_table_should_only_produce_single_row() {
        verifies!(
            r#"
    test 'single cell in CSV table should only produce single row' do
      input = <<~'EOS'
      ,===
      single cell
      ,===
      EOS

      output = convert_string_to_embedded input
      assert_css 'table td', output, 1
    end

"#
        );

        let doc = Parser::default().parse(",===\nsingle cell\n,===");

        assert_css(&doc, "table td", 1);
    }

    #[test]
    fn cell_formatted_with_asciidoc_style() {
        verifies!(
            r#"
    test 'cell formatted with AsciiDoc style' do
      input = <<~'EOS'
      [cols="1,1,1a",separator=;]
      ,===
      element;description;example

      thematic break,a visible break; also known as a horizontal rule;---
      ,===
      EOS

      output = convert_string_to_embedded input
      assert_css 'table tbody hr', output, 1
    end

"#
        );

        let doc = Parser::default().parse(
            "[cols=\"1,1,1a\",separator=;]\n,===\nelement;description;example\n\nthematic break,a visible break; also known as a horizontal rule;---\n,===",
        );

        assert_css(&doc, "table tbody hr", 1);
    }

    #[test]
    fn should_strip_whitespace_around_contents_of_asciidoc_cell() {
        verifies!(
            r#"
    test 'should strip whitespace around contents of AsciiDoc cell' do
      input = <<~'EOS'
      [cols="1,1,1a",separator=;]
      ,===
      element;description;example

      paragraph;contiguous lines of words and phrases;"
        one sentence, one line
        "
      ,===
      EOS

      output = convert_string_to_embedded input
      assert_xpath '/table/tbody//*[@class="paragraph"]/p[text()="one sentence, one line"]', output, 1
    end
"#
        );

        let doc = Parser::default().parse(
            "[cols=\"1,1,1a\",separator=;]\n,===\nelement;description;example\n\nparagraph;contiguous lines of words and phrases;\"\n  one sentence, one line\n  \"\n,===",
        );

        assert_xpath(
            &doc,
            "/table/tbody//*[@class=\"paragraph\"]/p[text()=\"one sentence, one line\"]",
            1,
        );
    }

    non_normative!(
        r#"
  end
end
"#
    );
}
