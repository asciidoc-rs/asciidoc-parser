// Adapted from Asciidoctor's preamble test suite, found in
// https://github.com/asciidoctor/asciidoctor/blob/main/test/preamble_test.rb.
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
//! Port of Asciidoctor's `preamble_test.rb`.
//!
//! Each Ruby `test '..'` becomes a `#[test] fn`, driven through
//! [`Parser`](crate::Parser) and asserted with the `assert_xpath` DOM helper
//! against [`to_virtual_dom`](crate::Document::to_virtual_dom) — the same
//! conventions documented in [`super`]. The preamble tests are all about
//! rendered structure (the `id="preamble"` wrapper, section siblings, generated
//! section ids), so `assert_xpath` is the natural fit here.
//!
//! **Embedded-output model.** This crate's virtual DOM models Asciidoctor's
//! *embedded* output, whose root `<div class="document">` plays the role of the
//! standalone document's `<div id="content">` body wrapper. The Ruby tests use
//! `convert_string` (standalone output), so their `//*[@id="content"]/...`
//! assertions are translated to the equivalent `//*[@class="document"]/...`
//! here (noted inline where it applies).
//!
//! Non-HTML backends and multiple level-0 headings are out of scope for this
//! crate, so the five DocBook `preface`/`abstract` tests and the book-doctype
//! multi-part test are reproduced verbatim in `non_normative!` blocks (see
//! #380 for level-0 sections). Each such block is annotated inline.

use crate::tests::prelude::*;

track_file!("ref/asciidoctor/test/preamble_test.rb");

non_normative!(
    r#"
# frozen_string_literal: true
require_relative 'test_helper'

context 'Preamble' do
"#
);

#[test]
fn title_and_single_paragraph_preamble_before_section() {
    verifies!(
        r#"
  test 'title and single paragraph preamble before section' do
    input = <<~'EOS'
    = Title

    Preamble paragraph 1.

    == First Section

    Section paragraph 1.
    EOS
    result = convert_string(input)
    assert_xpath '//p', result, 2
    assert_xpath '//*[@id="preamble"]', result, 1
    assert_xpath '//*[@id="preamble"]//p', result, 1
    assert_xpath '//*[@id="preamble"]/following-sibling::*//h2[@id="_first_section"]', result, 1
    assert_xpath '//*[@id="preamble"]/following-sibling::*//p', result, 1
  end

"#
    );

    let doc = Parser::default()
        .parse("= Title\n\nPreamble paragraph 1.\n\n== First Section\n\nSection paragraph 1.");

    assert_xpath(&doc, "//p", 2);
    assert_xpath(&doc, r#"//*[@id="preamble"]"#, 1);
    assert_xpath(&doc, r#"//*[@id="preamble"]//p"#, 1);
    assert_xpath(
        &doc,
        r#"//*[@id="preamble"]/following-sibling::*//h2[@id="_first_section"]"#,
        1,
    );
    assert_xpath(&doc, r#"//*[@id="preamble"]/following-sibling::*//p"#, 1);
}

non_normative!(
    r#"
  test 'title of preface is blank by default in DocBook output' do
    input = <<~'EOS'
    = Document Title
    :doctype: book

    Preface content.

    == First Section

    Section content.
    EOS
    result = convert_string input, backend: :docbook
    assert_xpath '//preface/title', result, 1
    title_node = xmlnodes_at_xpath '//preface/title', result, 1
    assert_equal '', title_node.text
  end

  test 'preface-title attribute is assigned as title of preface in DocBook output' do
    input = <<~'EOS'
    = Document Title
    :doctype: book
    :preface-title: Preface

    Preface content.

    == First Section

    Section content.
    EOS
    result = convert_string input, backend: :docbook
    assert_xpath '//preface/title[text()="Preface"]', result, 1
  end

"#
);

#[test]
fn title_and_multi_paragraph_preamble_before_section() {
    verifies!(
        r#"
  test 'title and multi-paragraph preamble before section' do
    input = <<~'EOS'
    = Title

    Preamble paragraph 1.

    Preamble paragraph 2.

    == First Section

    Section paragraph 1.
    EOS
    result = convert_string(input)
    assert_xpath '//p', result, 3
    assert_xpath '//*[@id="preamble"]', result, 1
    assert_xpath '//*[@id="preamble"]//p', result, 2
    assert_xpath '//*[@id="preamble"]/following-sibling::*//h2[@id="_first_section"]', result, 1
    assert_xpath '//*[@id="preamble"]/following-sibling::*//p', result, 1
  end

"#
    );

    let doc = Parser::default().parse(
        "= Title\n\nPreamble paragraph 1.\n\nPreamble paragraph 2.\n\n== First Section\n\nSection paragraph 1.",
    );

    assert_xpath(&doc, "//p", 3);
    assert_xpath(&doc, r#"//*[@id="preamble"]"#, 1);
    assert_xpath(&doc, r#"//*[@id="preamble"]//p"#, 2);
    assert_xpath(
        &doc,
        r#"//*[@id="preamble"]/following-sibling::*//h2[@id="_first_section"]"#,
        1,
    );
    assert_xpath(&doc, r#"//*[@id="preamble"]/following-sibling::*//p"#, 1);
}

#[test]
fn should_not_wrap_content_in_preamble_if_document_has_title_but_no_sections() {
    verifies!(
        r#"
  test 'should not wrap content in preamble if document has title but no sections' do
    input = <<~'EOS'
    = Title

    paragraph
    EOS
    result = convert_string(input)
    assert_xpath '//p', result, 1
    assert_xpath '//*[@id="content"]/*[@class="paragraph"]/p', result, 1
    assert_xpath '//*[@id="content"]/*[@class="paragraph"]/following-sibling::*', result, 0
  end

"#
    );

    let doc = Parser::default().parse("= Title\n\nparagraph");

    assert_xpath(&doc, "//p", 1);

    // In the embedded model the `<div class="document">` root is the content
    // body wrapper (Ruby's `id="content"`): the lone paragraph is a direct
    // child of it, not wrapped in a preamble, and has no following sibling.
    assert_xpath(&doc, r#"//*[@id="preamble"]"#, 0);
    assert_xpath(&doc, r#"//*[@class="paragraph"]/p"#, 1);
    assert_xpath(&doc, r#"//*[@class="paragraph"]/following-sibling::*"#, 0);
}

#[test]
fn title_and_section_without_preamble() {
    verifies!(
        r#"
  test 'title and section without preamble' do
    input = <<~'EOS'
    = Title

    == First Section

    Section paragraph 1.
    EOS
    result = convert_string(input)
    assert_xpath '//p', result, 1
    assert_xpath '//*[@id="preamble"]', result, 0
    assert_xpath '//h2[@id="_first_section"]', result, 1
  end

"#
    );

    let doc = Parser::default().parse("= Title\n\n== First Section\n\nSection paragraph 1.");

    assert_xpath(&doc, "//p", 1);
    assert_xpath(&doc, r#"//*[@id="preamble"]"#, 0);
    assert_xpath(&doc, r#"//h2[@id="_first_section"]"#, 1);
}

#[test]
fn no_title_with_preamble_and_section() {
    verifies!(
        r#"
  test 'no title with preamble and section' do
    input = <<~'EOS'
    Preamble paragraph 1.

    == First Section

    Section paragraph 1.
    EOS
    result = convert_string(input)
    assert_xpath '//p', result, 2
    assert_xpath '//*[@id="preamble"]', result, 0
    assert_xpath '//h2[@id="_first_section"]/preceding::p', result, 1
  end

"#
    );

    let doc = Parser::default()
        .parse("Preamble paragraph 1.\n\n== First Section\n\nSection paragraph 1.");

    assert_xpath(&doc, "//p", 2);
    assert_xpath(&doc, r#"//*[@id="preamble"]"#, 0);

    // Ruby's `preceding::p` (all paragraphs before the heading in document
    // order) counts the single top-level paragraph that precedes the section.
    // With no document title it is a plain paragraph, not a preamble, so it is
    // a preceding *sibling* of the section div here.
    assert_xpath(&doc, r#"//*[@class="sect1"]/preceding-sibling::*//p"#, 1);
}

non_normative!(
    r#"
  test 'preamble in book doctype' do
      input = <<~'EOS'
      = Book
      :doctype: book

      Back then...

      = Chapter One

      [partintro]
      It was a dark and stormy night...

      == Scene One

      Someone's gonna get axed.

      = Chapter Two

      [partintro]
      They couldn't believe their eyes when...

      == Scene One

      The axe came swinging.
      EOS

      d = document_from_string(input)
      assert_equal 'book', d.doctype
      output = d.convert
      assert_xpath '//h1', output, 3
      assert_xpath %{//*[@id="preamble"]//p[text() = "Back then#{decode_char 8230}#{decode_char 8203}"]}, output, 1
  end

"#
);

#[test]
fn should_output_table_of_contents_in_preamble_if_toc_placement_attribute_value_is_preamble() {
    verifies!(
        r#"
  test 'should output table of contents in preamble if toc-placement attribute value is preamble' do
    input = <<~'EOS'
    = Article
    :toc:
    :toc-placement: preamble

    Once upon a time...

    == Section One

    It was a dark and stormy night...

    == Section Two

    They couldn't believe their eyes when...
    EOS

    output = convert_string input
    assert_xpath '//*[@id="preamble"]/*[@id="toc"]', output, 1
  end

"#
    );

    let doc = Parser::default().parse(
        "= Article\n:toc:\n:toc-placement: preamble\n\nOnce upon a time...\n\n== Section One\n\nIt was a dark and stormy night...\n\n== Section Two\n\nThey couldn't believe their eyes when...",
    );

    assert_xpath(&doc, r#"//*[@id="preamble"]/*[@id="toc"]"#, 1);
}

non_normative!(
    r#"
  test 'should move abstract in implicit preface to info tag when converting to DocBook' do
    input = <<~'EOS'
    = Document Title

    [abstract]
    This is the abstract.

    == Fin
    EOS

    %w(article book).each do |doctype|
      output = convert_string input, backend: 'docbook', doctype: doctype
      assert_xpath '//abstract', output, 1
      assert_xpath %(/#{doctype}/info/abstract), output, 1
    end
  end

  test 'should move abstract as first section to info tag when converting to DocBook' do
    input = <<~'EOS'
    = Document Title

    [abstract]
    == Abstract

    This is the abstract.

    == Fin
    EOS

    output = convert_string input, backend: 'docbook'
    assert_xpath '//abstract', output, 1
    assert_xpath '/article/info/abstract', output, 1
  end

  test 'should move abstract in preface section to info tag when converting to DocBook' do
    input = <<~'EOS'
    = Document Title
    :doctype: book

    [preface]
    == Preface

    [abstract]
    This is the abstract.

    == Fin
    EOS

    output = convert_string input, backend: 'docbook'
    assert_xpath '//abstract', output, 1
    assert_xpath '/book/info/abstract', output, 1
    assert_xpath '//preface', output, 0
  end
end
"#
);
