// Adapted from Asciidoctor's text test suite, found in
// https://github.com/asciidoctor/asciidoctor/blob/main/test/text_test.rb.
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
//! Port of Asciidoctor's `text_test.rb`.
//!
//! Each Ruby `test '..'` becomes a `#[test] fn`, driven through
//! [`Parser`](crate::Parser) and asserted with the `assert_*` DOM helpers.
//! Every ported `test '..'` block is reproduced verbatim in a `verifies!` block
//! so the `sdd` coverage tool can measure which lines of the Ruby suite are
//! verified line-by-line.
//!
//! Several areas are out of scope for this crate and are reproduced verbatim in
//! `non_normative!` blocks (accounting for those Ruby lines without asserting
//! behavior this crate does not model); each carries a comment noting why:
//!
//! * The UTF-8 *encoding* tests exercise Ruby `String` encoding, fixture files
//!   (`fixtures/encoding.adoc`, not vendored), the DocBook backend, and the
//!   `PreprocessorReader` / include machinery. This crate parses `&str` that is
//!   already valid UTF-8, so encoding is not a parser concern here, and full
//!   document conversion / other backends are downstream responsibilities.
//! * *Compat mode* is not implemented (see `content/passthroughs.rs`). Tests
//!   that carry both a compat-mode and a modern assertion are ported as
//!   `verifies!` with the modern assertion driven in Rust; the compat-mode
//!   assertion remains in the reproduced Ruby text only.
//! * Markdown-style thematic breaks: per the AsciiDoc language spec
//!   (`blocks/partials/thematic-breaks.adoc`), only the three-character `-` and
//!   `*` forms are recognized. This crate also accepts Asciidoctor's `_` forms
//!   (`___`, `_ _ _`) at column 1 as a compatibility extension. It
//!   intentionally does *not* adopt Asciidoctor's 0–3 leading-space tolerance;
//!   that is a deliberate, settled divergence (the marker must start at column
//!   1), noted where it arises.

use crate::tests::prelude::*;

track_file!("ref/asciidoctor/test/text_test.rb");

non_normative!(
    r#"
# frozen_string_literal: true
require_relative 'test_helper'

context "Text" do
"#
);

// UTF-8 encoding test: loads the `:encoding` sample document (fixture not
// vendored) and converts it with the HTML backend. Ruby `String` encoding is
// not a parser concern for this crate, which parses `&str` that is already
// valid UTF-8; full standalone document conversion is a downstream concern.
non_normative!(
    r#"
  test "proper encoding to handle utf8 characters in document using html backend" do
    output = example_document(:encoding).convert
    assert_xpath '//p', output, 4
    assert_xpath '//a', output, 1
  end

"#
);

// As above, for the embedded (`standalone: false`) HTML conversion.
non_normative!(
    r#"
  test "proper encoding to handle utf8 characters in embedded document using html backend" do
    output = example_document(:encoding, standalone: false).convert
    assert_xpath '//p', output, 4
    assert_xpath '//a', output, 1
  end

"#
);

// DocBook backend is out of scope for this crate.
non_normative!(
    r#"
  test 'proper encoding to handle utf8 characters in document using docbook backend' do
    output = example_document(:encoding, attributes: { 'backend' => 'docbook', 'xmlns' => '' }).convert
    assert_xpath '//xmlns:simpara', output, 4
    assert_xpath '//xmlns:link', output, 1
  end

"#
);

// DocBook backend is out of scope for this crate.
non_normative!(
    r#"
  test 'proper encoding to handle utf8 characters in embedded document using docbook backend' do
    output = example_document(:encoding, standalone: false, attributes: { 'backend' => 'docbook' }).convert
    assert_xpath '//simpara', output, 4
    assert_xpath '//link', output, 1
  end

"#
);

// Reads the `:encoding` sample document (fixture not vendored) through the
// `PreprocessorReader` and `Parser.next_block` Ruby APIs, which this crate does
// not expose. Encoding is not a parser concern here (see module docs).
non_normative!(
    r#"
  # NOTE this test ensures we have the encoding line on block templates too
  test 'proper encoding to handle utf8 characters in arbitrary block' do
    input = []
    input << "[verse]\n"
    input += (File.readlines (sample_doc_path :encoding), mode: Asciidoctor::FILE_READ_MODE)
    doc = empty_document
    reader = Asciidoctor::PreprocessorReader.new doc, input, nil, normalize: true
    block = Asciidoctor::Parser.next_block(reader, doc)
    assert_xpath '//pre', block.convert.gsub(/^\s*\n/, ''), 1
  end

"#
);

// Exercises `include::` preprocessing of a fixture file (not vendored) via the
// `PreprocessorReader` Ruby API. Encoding is not a parser concern here.
non_normative!(
    r#"
  test 'proper encoding to handle utf8 characters from included file' do
    input = 'include::fixtures/encoding.adoc[tags=romé]'
    doc = empty_safe_document base_dir: testdir
    reader = Asciidoctor::PreprocessorReader.new doc, input, nil, normalize: true
    block = Asciidoctor::Parser.next_block(reader, doc)
    output = block.convert
    assert_css '.paragraph', output, 1
  end

"#
);

#[test]
fn escaped_text_markup() {
    verifies!(
        r#"
  test 'escaped text markup' do
    assert_match(/All your &lt;em&gt;inline&lt;\/em&gt; markup belongs to &lt;strong&gt;us&lt;\/strong&gt;!/,
        convert_string('All your <em>inline</em> markup belongs to <strong>us</strong>!'))
  end

"#
    );

    let doc =
        Parser::default().parse("All your <em>inline</em> markup belongs to <strong>us</strong>!");
    assert_output_contains(
        &doc,
        "All your &lt;em&gt;inline&lt;/em&gt; markup belongs to &lt;strong&gt;us&lt;/strong&gt;!",
    );
}

#[test]
fn line_breaks() {
    verifies!(
        r#"
  test "line breaks" do
    assert_xpath "//br", convert_string("Well this is +\njust fine and dandy, isn't it?"), 1
  end

"#
    );

    let doc = Parser::default().parse("Well this is +\njust fine and dandy, isn't it?");
    assert_xpath(&doc, "//br", 1);
}

#[test]
fn single_and_double_quoted_text() {
    verifies!(
        r#"
  test 'single- and double-quoted text' do
    output = convert_string_to_embedded(%q(``Where?,'' she said, flipping through her copy of `The New Yorker.'), attributes: { 'compat-mode' => '' })
    assert_match(/&#8220;Where\?,&#8221;/, output)
    assert_match(/&#8216;The New Yorker.&#8217;/, output)

    output = convert_string_to_embedded(%q("`Where?,`" she said, flipping through her copy of '`The New Yorker.`'))
    assert_match(/&#8220;Where\?,&#8221;/, output)
    assert_match(/&#8216;The New Yorker.&#8217;/, output)
  end

"#
    );

    // Compat mode is unsupported; only the modern (second) form is driven here.
    let doc = Parser::default()
        .parse(r#""`Where?,`" she said, flipping through her copy of '`The New Yorker.`'"#);
    assert_output_contains(&doc, "&#8220;Where?,&#8221;");
    assert_output_contains(&doc, "&#8216;The New Yorker.&#8217;");
}

#[test]
fn multiple_double_quoted_text_on_a_single_line() {
    verifies!(
        r#"
  test 'multiple double-quoted text on a single line' do
    assert_equal '&#8220;Our business is constantly changing&#8221; or &#8220;We need faster time to market.&#8221;',
        convert_inline_string(%q(``Our business is constantly changing'' or ``We need faster time to market.''), attributes: { 'compat-mode' => '' })
    assert_equal '&#8220;Our business is constantly changing&#8221; or &#8220;We need faster time to market.&#8221;',
        convert_inline_string(%q("`Our business is constantly changing`" or "`We need faster time to market.`"))
  end

"#
    );

    // Compat mode is unsupported; only the modern (second) form is driven here.
    let doc = Parser::default()
        .parse(r#""`Our business is constantly changing`" or "`We need faster time to market.`""#);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        "&#8220;Our business is constantly changing&#8221; or &#8220;We need faster time to market.&#8221;"
    );
}

#[test]
fn horizontal_rule() {
    verifies!(
        r#"
  test 'horizontal rule' do
    input = <<~'EOS'
    This line is separated by a horizontal rule...

    '''

    ...from this line.
    EOS
    output = convert_string_to_embedded input
    assert_xpath "//hr", output, 1
    assert_xpath "/*[@class='paragraph']", output, 2
    assert_xpath "(/*[@class='paragraph'])[1]/following-sibling::hr", output, 1
    assert_xpath "/hr/following-sibling::*[@class='paragraph']", output, 1
  end

"#
    );

    let doc = Parser::default()
        .parse("This line is separated by a horizontal rule...\n\n'''\n\n...from this line.");

    // The test xpath engine handles double-quoted attribute values only.
    assert_xpath(&doc, "//hr", 1);
    assert_xpath(&doc, "/*[@class=\"paragraph\"]", 2);
    assert_xpath(
        &doc,
        "(/*[@class=\"paragraph\"])[1]/following-sibling::hr",
        1,
    );
    assert_xpath(&doc, "/hr/following-sibling::*[@class=\"paragraph\"]", 1);
}

// The `markdown horizontal rules` (positive) test is out of scope as written,
// so it is reproduced non-normatively rather than verified. It asserts a
// thematic break for six variants at four leading offsets. This crate now
// recognizes all six variants (`---`, `- - -`, `***`, `* * *`, `___`, `_ _ _`)
// at column 1: the `-`/`*` forms are spec-recognized
// (`blocks/partials/thematic-breaks.adoc`), and the `_` forms are accepted as
// an Asciidoctor-compatibility extension. This crate intentionally diverges on
// the leading offset: Asciidoctor tolerates 0–3 leading spaces, while this
// crate requires the marker at column 1 (an indented line becomes a literal
// paragraph, per AsciiDoc's general treatment of leading whitespace). That
// divergence is a deliberate, settled decision, so the offset dimension of this
// test is left unsatisfied by design. The spec-recognized subset is verified
// directly against the language spec in
// `tests::asciidoc_lang::blocks::thematic_breaks::markdown_style_thematic_breaks`.
non_normative!(
    r#"
  test 'markdown horizontal rules' do
    variants = [
      '---',
      '- - -',
      '***',
      '* * *',
      '___',
      '_ _ _'
    ]

    offsets = [
      '',
      ' ',
      '  ',
      '   '
    ]

    variants.each do |variant|
      offsets.each do |offset|
        input = <<~EOS
        This line is separated by a horizontal rule...

        #{offset}#{variant}

        ...from this line.
        EOS
        output = convert_string_to_embedded input
        assert_xpath "//hr", output, 1
        assert_xpath "/*[@class='paragraph']", output, 2
        assert_xpath "(/*[@class='paragraph'])[1]/following-sibling::hr", output, 1
        assert_xpath "/hr/following-sibling::*[@class='paragraph']", output, 1
      end
    end
  end

"#
);

#[test]
fn markdown_horizontal_rules_negative_case() {
    verifies!(
        r#"
  test 'markdown horizontal rules negative case' do

    bad_variants = [
      '- - - -',
      '* * * *',
      '_ _ _ _'
    ]

    good_offsets = [
      '',
      ' ',
      '  ',
      '   '
    ]

    bad_variants.each do |variant|
      good_offsets.each do |offset|
        input = <<~EOS
        This line is separated by something that is not a horizontal rule...

        #{offset}#{variant}

        ...from this line.
        EOS
        output = convert_string_to_embedded input
        assert_xpath '//hr', output, 0
      end
    end

    good_variants = [
      '- - -',
      '* * *',
      '_ _ _'
    ]

    bad_offsets = [
      "\t",
      '    '
    ]

    good_variants.each do |variant|
      bad_offsets.each do |offset|
        input = <<~EOS
        This line is separated by something that is not a horizontal rule...

        #{offset}#{variant}

        ...from this line.
        EOS
        output = convert_string_to_embedded input
        assert_xpath '//hr', output, 0
      end
    end
  end

"#
    );

    // None of these produce a thematic break in this crate. The four-character
    // variants are rejected by both this crate and Asciidoctor; the tab- and
    // four-space-offset good variants are rejected here because this crate
    // intentionally accepts no leading offset at all (Asciidoctor rejects only
    // these two). That stricter behavior is a deliberate, settled divergence.
    for variant in ["- - - -", "* * * *", "_ _ _ _"] {
        for offset in ["", " ", "  ", "   "] {
            let input = format!(
                "This line is separated by something that is not a horizontal rule...\n\n{offset}{variant}\n\n...from this line."
            );
            let doc = Parser::default().parse(&input);
            assert_xpath(&doc, "//hr", 0);
        }
    }

    for variant in ["- - -", "* * *", "_ _ _"] {
        for offset in ["\t", "    "] {
            let input = format!(
                "This line is separated by something that is not a horizontal rule...\n\n{offset}{variant}\n\n...from this line."
            );
            let doc = Parser::default().parse(&input);
            assert_xpath(&doc, "//hr", 0);
        }
    }
}

#[test]
fn emphasized_text_using_underscore_characters() {
    verifies!(
        r#"
  test "emphasized text using underscore characters" do
    assert_xpath "//em", convert_string("An _emphatic_ no")
  end

"#
    );

    let doc = Parser::default().parse("An _emphatic_ no");
    assert_xpath(&doc, "//em", 1);
}

#[test]
fn emphasized_text_with_single_quote_using_apostrophe_characters() {
    verifies!(
        r#"
  test 'emphasized text with single quote using apostrophe characters' do
    rsquo = decode_char 8217
    assert_xpath %(//em[text()="Johnny#{rsquo}s"]), convert_string(%q(It's 'Johnny's' phone), attributes: { 'compat-mode' => '' })
    assert_xpath %(//p[text()="It#{rsquo}s 'Johnny#{rsquo}s' phone"]), convert_string(%q(It's 'Johnny's' phone))
  end

"#
    );

    // Compat mode is unsupported; only the modern (second) assertion is driven.
    // `#{rsquo}` decodes to U+2019 (right single quotation mark).
    let doc = Parser::default().parse("It's 'Johnny's' phone");
    assert_xpath(
        &doc,
        "//p[text()=\"It\u{2019}s 'Johnny\u{2019}s' phone\"]",
        1,
    );
}

#[test]
fn emphasized_text_with_escaped_single_quote_using_apostrophe_characters() {
    verifies!(
        r#"
  test 'emphasized text with escaped single quote using apostrophe characters' do
    assert_xpath %(//em[text()="Johnny's"]), convert_string(%q(It's 'Johnny\\'s' phone), attributes: { 'compat-mode' => '' })
    assert_xpath %(//p[text()="It's 'Johnny's' phone"]), convert_string(%q(It\\'s 'Johnny\\'s' phone))
  end

"#
    );

    // Compat mode is unsupported; only the modern (second) assertion is driven.
    // In the `%q(..)` Ruby literal, `\\'` is a single escaped apostrophe.
    let doc = Parser::default().parse("It\\'s 'Johnny\\'s' phone");
    assert_xpath(&doc, "//p[text()=\"It's 'Johnny's' phone\"]", 1);
}

#[test]
fn escaped_single_quote_is_restored_as_single_quote() {
    verifies!(
        r#"
  test "escaped single quote is restored as single quote" do
    assert_xpath "//p[contains(text(), \"Let's do it!\")]", convert_string("Let\\'s do it!")
  end

"#
    );

    // `contains(text(), ..)` is not supported by the test xpath engine; the
    // whole paragraph is `Let's do it!`, so an exact `text()` match is
    // equivalent here.
    let doc = Parser::default().parse("Let\\'s do it!");
    assert_xpath(&doc, "//p[text()=\"Let's do it!\"]", 1);
}

#[test]
fn unescape_escaped_single_quote_emphasis_in_compat_mode_only() {
    verifies!(
        r#"
  test 'unescape escaped single quote emphasis in compat mode only' do
    assert_xpath %(//p[text()="A 'single quoted string' example"]), convert_string_to_embedded(%(A \\'single quoted string' example), attributes: { 'compat-mode' => '' })
    assert_xpath %(//p[text()="'single quoted string'"]), convert_string_to_embedded(%(\\'single quoted string'), attributes: { 'compat-mode' => '' })

    assert_xpath %(//p[text()="A \\'single quoted string' example"]), convert_string_to_embedded(%(A \\'single quoted string' example))
    assert_xpath %(//p[text()="\\'single quoted string'"]), convert_string_to_embedded(%(\\'single quoted string'))
  end

"#
    );

    // Compat mode is unsupported; only the two modern assertions are driven.
    // Outside compat mode the leading escaped apostrophe keeps its backslash.
    let doc = Parser::default().parse("A \\'single quoted string' example");
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        "A \\'single quoted string' example"
    );

    let doc = Parser::default().parse("\\'single quoted string'");
    assert_eq!(rendered_paragraphs(&doc)[0], "\\'single quoted string'");
}

#[test]
fn emphasized_text_at_end_of_line() {
    verifies!(
        r#"
  test "emphasized text at end of line" do
    assert_xpath "//em", convert_string("This library is _awesome_")
  end

"#
    );

    let doc = Parser::default().parse("This library is _awesome_");
    assert_xpath(&doc, "//em", 1);
}

#[test]
fn emphasized_text_at_beginning_of_line() {
    verifies!(
        r#"
  test "emphasized text at beginning of line" do
    assert_xpath "//em", convert_string("_drop_ it")
  end

"#
    );

    let doc = Parser::default().parse("_drop_ it");
    assert_xpath(&doc, "//em", 1);
}

#[test]
fn emphasized_text_across_line() {
    verifies!(
        r#"
  test "emphasized text across line" do
    assert_xpath "//em", convert_string("_check it_")
  end

"#
    );

    let doc = Parser::default().parse("_check it_");
    assert_xpath(&doc, "//em", 1);
}

#[test]
fn unquoted_text() {
    verifies!(
        r#"
  test "unquoted text" do
    refute_match(/#/, convert_string("An #unquoted# word"))
  end

"#
    );

    let doc = Parser::default().parse("An #unquoted# word");
    refute_output_contains(&doc, "#");
}

#[test]
fn backticks_and_straight_quotes_in_text() {
    verifies!(
        r#"
  test 'backticks and straight quotes in text' do
    backslash = '\\'
    assert_equal %q(run <code>foo</code> <em>dog</em>), convert_inline_string(%q(run `foo` 'dog'), attributes: { 'compat-mode' => '' })
    assert_equal %q(run <code>foo</code> 'dog'), convert_inline_string(%q(run `foo` 'dog'))
    assert_equal %q(run `foo` 'dog'), convert_inline_string(%(run #{backslash}`foo` 'dog'))
    assert_equal %q(run &#8216;foo` 'dog&#8217;), convert_inline_string(%q(run '`foo` 'dog`'))
    assert_equal %q(run '`foo` 'dog`'), convert_inline_string(%(run #{backslash}'`foo` 'dog#{backslash}`'))
  end

"#
    );

    // Compat mode is unsupported; the four modern assertions are driven here.
    let doc = Parser::default().parse("run `foo` 'dog'");
    assert_eq!(rendered_paragraphs(&doc)[0], "run <code>foo</code> 'dog'");

    let doc = Parser::default().parse("run \\`foo` 'dog'");
    assert_eq!(rendered_paragraphs(&doc)[0], "run `foo` 'dog'");

    let doc = Parser::default().parse("run '`foo` 'dog`'");
    assert_eq!(rendered_paragraphs(&doc)[0], "run &#8216;foo` 'dog&#8217;");

    let doc = Parser::default().parse("run \\'`foo` 'dog\\`'");
    assert_eq!(rendered_paragraphs(&doc)[0], "run '`foo` 'dog`'");
}

#[test]
fn plus_characters_inside_single_plus_passthrough() {
    verifies!(
        r#"
  test 'plus characters inside single plus passthrough' do
    assert_xpath '//p[text()="+"]', convert_string_to_embedded('+++')
    assert_xpath '//p[text()="+="]', convert_string_to_embedded('++=+')
  end

"#
    );

    let doc = Parser::default().parse("+++");
    assert_xpath(&doc, "//p[text()=\"+\"]", 1);

    let doc = Parser::default().parse("++=+");
    assert_xpath(&doc, "//p[text()=\"+=\"]", 1);
}

#[test]
fn plus_passthrough_escapes_entity_reference() {
    verifies!(
        r#"
  test 'plus passthrough escapes entity reference' do
    assert_match(/&amp;#44;/, convert_string_to_embedded('+&#44;+'))
    assert_match(/one&amp;#44;two/, convert_string_to_embedded('one++&#44;++two'))
  end

"#
    );

    let doc = Parser::default().parse("+&#44;+");
    assert_output_contains(&doc, "&amp;#44;");

    let doc = Parser::default().parse("one++&#44;++two");
    assert_output_contains(&doc, "one&amp;#44;two");
}

mod basic_styling {
    use crate::tests::prelude::*;

    non_normative!(
        r#"
  context "basic styling" do
    setup do
      @output = convert_string("A *BOLD* word.  An _italic_ word.  A `mono` word.  ^superscript!^ and some ~subscript~.")
    end

"#
    );

    // The Ruby `setup` renders `@output` once for the whole context; each Rust
    // test reparses the same input.
    fn output() -> crate::Document<'static> {
        Parser::default().parse(
            "A *BOLD* word.  An _italic_ word.  A `mono` word.  ^superscript!^ and some ~subscript~.",
        )
    }

    #[test]
    fn strong() {
        verifies!(
            r#"
    test "strong" do
      assert_xpath "//strong", @output, 1
    end

"#
        );

        assert_xpath(&output(), "//strong", 1);
    }

    #[test]
    fn italic() {
        verifies!(
            r#"
    test "italic" do
      assert_xpath "//em", @output, 1
    end

"#
        );

        assert_xpath(&output(), "//em", 1);
    }

    #[test]
    fn monospaced() {
        verifies!(
            r#"
    test "monospaced" do
      assert_xpath "//code", @output, 1
    end

"#
        );

        assert_xpath(&output(), "//code", 1);
    }

    #[test]
    fn superscript() {
        verifies!(
            r#"
    test "superscript" do
      assert_xpath "//sup", @output, 1
    end

"#
        );

        assert_xpath(&output(), "//sup", 1);
    }

    #[test]
    fn subscript() {
        verifies!(
            r#"
    test "subscript" do
      assert_xpath "//sub", @output, 1
    end

"#
        );

        assert_xpath(&output(), "//sub", 1);
    }

    #[test]
    fn passthrough() {
        verifies!(
            r#"
    test "passthrough" do
      assert_xpath "//code", convert_string("This is +passed through+."), 0
      assert_xpath "//code", convert_string("This is +passed through and monospaced+.", attributes: { 'compat-mode' => '' }), 1
    end

"#
        );

        // Modern: single-plus is a passthrough, not a monospaced phrase. The
        // second assertion is compat-mode (unsupported) and is not driven.
        let doc = Parser::default().parse("This is +passed through+.");
        assert_xpath(&doc, "//code", 0);
    }

    #[test]
    fn nested_styles() {
        verifies!(
            r#"
    test "nested styles" do
      output = convert_string("Winning *big _time_* in the +city *boyeeee*+.", attributes: { 'compat-mode' => '' })

      assert_xpath "//strong/em", output
      assert_xpath "//code/strong", output

      output = convert_string("Winning *big _time_* in the `city *boyeeee*`.")

      assert_xpath "//strong/em", output
      assert_xpath "//code/strong", output
    end

"#
        );

        // Compat mode is unsupported; only the modern form is driven here.
        let doc = Parser::default().parse("Winning *big _time_* in the `city *boyeeee*`.");
        assert_xpath(&doc, "//strong/em", 1);
        assert_xpath(&doc, "//code/strong", 1);
    }

    #[test]
    fn unconstrained_quotes() {
        verifies!(
            r#"
    test 'unconstrained quotes' do
      output = convert_string('**B**__I__++M++[role]++M++', attributes: { 'compat-mode' => '' })
      assert_xpath '//strong', output, 1
      assert_xpath '//em', output, 1
      assert_xpath '//code[not(@class)]', output, 1
      assert_xpath '//code[@class="role"]', output, 1

      output = convert_string('**B**__I__``M``[role]``M``')
      assert_xpath '//strong', output, 1
      assert_xpath '//em', output, 1
      assert_xpath '//code[not(@class)]', output, 1
      assert_xpath '//code[@class="role"]', output, 1
    end
"#
        );

        // Compat mode is unsupported; only the modern form is driven here.
        let doc = Parser::default().parse("**B**__I__``M``[role]``M``");
        assert_xpath(&doc, "//strong", 1);
        assert_xpath(&doc, "//em", 1);
        assert_xpath(&doc, "//code[@class=\"role\"]", 1);

        // The test xpath engine has no `not(@class)`, so the Ruby assertion that
        // exactly one `<code>` has no class is instead pinned by asserting the
        // full rendered output (which also subsumes the checks above).
        assert_eq!(
            rendered_paragraphs(&doc)[0],
            r#"<strong>B</strong><em>I</em><code>M</code><code class="role">M</code>"#
        );
    }

    non_normative!(
        r#"
  end

"#
    );
}

#[test]
fn should_format_asian_characters_as_words() {
    verifies!(
        r#"
  test 'should format Asian characters as words' do
    assert_xpath '//strong', (convert_string_to_embedded 'bold *要* bold')
    assert_xpath '//strong', (convert_string_to_embedded 'bold *素* bold')
    assert_xpath '//strong', (convert_string_to_embedded 'bold *要素* bold')
  end
"#
    );

    assert_xpath(&Parser::default().parse("bold *要* bold"), "//strong", 1);
    assert_xpath(&Parser::default().parse("bold *素* bold"), "//strong", 1);
    assert_xpath(&Parser::default().parse("bold *要素* bold"), "//strong", 1);
}

non_normative!(
    r#"
end
"#
);
