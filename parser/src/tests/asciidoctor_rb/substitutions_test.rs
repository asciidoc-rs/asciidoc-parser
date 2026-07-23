// Adapted from Asciidoctor's substitutions test suite, found in
// https://github.com/asciidoctor/asciidoctor/blob/main/test/substitutions_test.rb.
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
//! Port of Asciidoctor's `substitutions_test.rb`.
//!
//! Each Ruby `context` becomes a Rust `mod` and each `test '..'` becomes a
//! `#[test] fn`, driven through [`Parser`](crate::Parser) (or the lower-level
//! substitution APIs) and asserted with the `assert_*` DOM helpers or a
//! full-AST `assert_eq!`. Every `test '..'` block is reproduced verbatim in a
//! `verifies!` block so the `sdd` coverage tool can measure which lines of the
//! Ruby suite are verified line-by-line.
//!
//! Compatibility mode and non-HTML backends (DocBook, etc.) are out of scope
//! for this crate, as is source-highlighter integration (CodeRay/Pygments) and
//! several Ruby-internal substitution APIs. Ruby tests exercising those, and a
//! handful of tests with no crate-level analogue, are reproduced verbatim in
//! `non_normative!` blocks (they account for those Ruby lines without asserting
//! behavior this crate does not model). A number of `#[test] fn`s extend the
//! Ruby coverage with crate-specific boundary cases and so carry no `verifies!`
//! block.

use crate::tests::sdd::*;

track_file!("ref/asciidoctor/test/substitutions_test.rb");

non_normative!(
    r#"
# frozen_string_literal: true
require_relative 'test_helper'

# TODO
# - test negatives
# - test role on every quote type
context 'Substitutions' do
  BACKSLASH = ?\\
  context 'Dispatcher' do
"#
);

mod dispatcher {
    use crate::tests::prelude::*;

    #[test]
    fn apply_normal_substitutions() {
        verifies!(
            r#"
    test 'apply normal substitutions' do
      para = block_from_string("[blue]_http://asciidoc.org[AsciiDoc]_ & [red]*Ruby*\n&#167; Making +++<u>documentation</u>+++ together +\nsince (C) {inception_year}.")
      para.document.attributes['inception_year'] = '2012'
      result = para.apply_subs(para.source)
      assert_equal %{<em class="blue"><a href="http://asciidoc.org">AsciiDoc</a></em> &amp; <strong class="red">Ruby</strong>\n&#167; Making <u>documentation</u> together<br>\nsince &#169; 2012.}, result
    end

"#
        );

        let mut p = Parser::default().with_intrinsic_attribute(
            "inception_year",
            "2012",
            ModificationContext::Anywhere,
        );

        let maw = crate::blocks::Block::parse(
            crate::Span::new(
                "[blue]_http://asciidoc.org[AsciiDoc]_ & [red]*Ruby*\n&#167; Making +++<u>documentation</u>+++ together +\nsince (C) {inception_year}.",
            ),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "[blue]_http://asciidoc.org[AsciiDoc]_ & [red]*Ruby*\n&#167; Making +++<u>documentation</u>+++ together +\nsince (C) {inception_year}.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "<em class=\"blue\"><a href=\"http://asciidoc.org\">AsciiDoc</a></em> &amp; <strong class=\"red\">Ruby</strong>\n&#167; Making <u>documentation</u> together<br>\nsince &#169; 2012.",
                },
                source: Span {
                    data: "[blue]_http://asciidoc.org[AsciiDoc]_ & [red]*Ruby*\n&#167; Making +++<u>documentation</u>+++ together +\nsince (C) {inception_year}.",
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
            },)
        );
    }

    #[test]
    fn apply_subs_should_not_modify_string_directly() {
        verifies!(
            r#"
    test 'apply_subs should not modify string directly' do
      input = '<html> -- the root of all web'
      para = block_from_string input
      para_source = para.source
      result = para.apply_subs para_source
      assert_equal '&lt;html&gt;&#8201;&#8212;&#8201;the root of all web', result
      assert_equal input, para_source
    end

"#
        );

        // The crate models `apply_subs` as immutable substitution: the block's
        // `original` span is preserved while `rendered` holds the result, so the
        // upstream "source string is not mutated" guarantee holds by construction.
        let mut p = Parser::default();

        let maw =
            crate::blocks::Block::parse(crate::Span::new("<html> -- the root of all web"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "<html> -- the root of all web",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "&lt;html&gt;&#8201;&#8212;&#8201;the root of all web",
                },
                source: Span {
                    data: "<html> -- the root of all web",
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
            },)
        );
    }

    // Not ported to this crate:
    // - 'should not drop trailing blank lines when performing substitutions':
    //   para.lines trailing-blank round-trip: no crate API to inject trailing blank
    //   lines.
    // - 'should expand subs passed to expand_subs': expand_subs Ruby API; sub-list
    //   resolution is covered by content::substitution_group tests.
    non_normative!(
        r#"
    test 'should not drop trailing blank lines when performing substitutions' do
      para = block_from_string %([%hardbreaks]\nthis\nis\n-> {program})
      para.lines << ''
      para.lines << ''
      para.document.attributes['program'] = 'Asciidoctor'
      result = para.apply_subs(para.lines)
      assert_equal ['this<br>', 'is<br>', '&#8594; Asciidoctor<br>', '<br>', ''], result
      result = para.apply_subs(para.lines * "\n")
      assert_equal %(this<br>\nis<br>\n&#8594; Asciidoctor<br>\n<br>\n), result
    end

    test 'should expand subs passed to expand_subs' do
      para = block_from_string %({program}\n*bold*\n2 > 1)
      para.document.attributes['program'] = 'Asciidoctor'
      assert_equal [:specialcharacters], (para.expand_subs [:specialchars])
      refute para.expand_subs([:none])
      assert_equal [:specialcharacters, :quotes, :attributes, :replacements, :macros, :post_replacements], (para.expand_subs [:normal])
    end

"#
    );

    #[test]
    fn apply_subs_should_allow_the_subs_argument_to_be_nil() {
        verifies!(
            r#"
    test 'apply_subs should allow the subs argument to be nil' do
      block = block_from_string %([pass]\n*raw*)
      result = block.apply_subs block.source, nil
      assert_equal '*raw*', result
    end
"#
        );

        // Upstream passes an explicit `nil` subs list; this crate resolves a
        // `[pass]` block to no substitutions at parse time, so its raw content is
        // emitted verbatim.
        let doc = Parser::default().parse("[pass]\n*raw*");
        assert_eq!(rendered_paragraphs(&doc), &["*raw*"]);
    }
}

non_normative!(
    r#"
  end

  context 'Quotes' do
"#
);

mod quotes {
    use crate::{content::SubstitutionStep, strings::CowStr, tests::prelude::*};

    #[test]
    fn single_line_double_quoted_string() {
        verifies!(
            r#"
    test 'single-line double-quoted string' do
      para = block_from_string(%q{``a few quoted words''}, attributes: { 'compat-mode' => '' })
      assert_equal '&#8220;a few quoted words&#8221;', para.sub_quotes(para.source)

      para = block_from_string(%q{"`a few quoted words`"})
      assert_equal '&#8220;a few quoted words&#8221;', para.sub_quotes(para.source)

      para = block_from_string(%q{"`a few quoted words`"}, backend: 'docbook')
      assert_equal '<quote>a few quoted words</quote>', para.sub_quotes(para.source)
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new(r#""`a few quoted words`""#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"&#8220;a few quoted words&#8221;"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn escaped_single_line_double_quoted_string() {
        verifies!(
            r#"
    test 'escaped single-line double-quoted string' do
      para = block_from_string %(#{BACKSLASH}``a few quoted words''), attributes: { 'compat-mode' => '' }
      assert_equal %q(&#8216;`a few quoted words&#8217;'), para.sub_quotes(para.source)

      para = block_from_string %(#{BACKSLASH * 2}``a few quoted words''), attributes: { 'compat-mode' => '' }
      assert_equal %q(``a few quoted words''), para.sub_quotes(para.source)

      para = block_from_string(%(#{BACKSLASH}"`a few quoted words`"))
      assert_equal %q("`a few quoted words`"), para.sub_quotes(para.source)

      para = block_from_string(%(#{BACKSLASH * 2}"`a few quoted words`"))
      assert_equal %(#{BACKSLASH}"`a few quoted words`"), para.sub_quotes(para.source)
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new(r#"\"`a few quoted words`""#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#""`a few quoted words`""#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn multi_line_double_quoted_string() {
        verifies!(
            r#"
    test 'multi-line double-quoted string' do
      para = block_from_string(%Q{``a few\nquoted words''}, attributes: { 'compat-mode' => '' })
      assert_equal "&#8220;a few\nquoted words&#8221;", para.sub_quotes(para.source)

      para = block_from_string(%Q{"`a few\nquoted words`"})
      assert_equal "&#8220;a few\nquoted words&#8221;", para.sub_quotes(para.source)
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new("\"`a few\nquoted words`\""));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(
                "&#8220;a few\nquoted words&#8221;"
                    .to_string()
                    .into_boxed_str()
            )
        );
    }

    #[test]
    fn double_quoted_string_with_inline_single_quote() {
        verifies!(
            r#"
    test 'double-quoted string with inline single quote' do
      para = block_from_string(%q{``Here's Johnny!''}, attributes: { 'compat-mode' => '' })
      assert_equal %q{&#8220;Here's Johnny!&#8221;}, para.sub_quotes(para.source)

      para = block_from_string(%q{"`Here's Johnny!`"})
      assert_equal %q{&#8220;Here's Johnny!&#8221;}, para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(r#""`Here's Johnny!`""#));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"&#8220;Here's Johnny!&#8221;"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn double_quoted_string_with_inline_backquote() {
        verifies!(
            r#"
    test 'double-quoted string with inline backquote' do
      para = block_from_string(%q{``Here`s Johnny!''}, attributes: { 'compat-mode' => '' })
      assert_equal %q{&#8220;Here`s Johnny!&#8221;}, para.sub_quotes(para.source)

      para = block_from_string(%q{"`Here`s Johnny!`"})
      assert_equal %q{&#8220;Here`s Johnny!&#8221;}, para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(r#""`Here`s Johnny!`""#));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"&#8220;Here`s Johnny!&#8221;"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn double_quoted_string_around_monospaced_text() {
        verifies!(
            r#"
    test 'double-quoted string around monospaced text' do
      para = block_from_string(%q("``E=mc^2^` is the solution!`"))
      assert_equal %q(&#8220;`E=mc<sup>2</sup>` is the solution!&#8221;), para.apply_subs(para.source);

      para = block_from_string(%q("```E=mc^2^`` is the solution!`"))
      assert_equal %q(&#8220;<code>E=mc<sup>2</sup></code> is the solution!&#8221;), para.apply_subs(para.source);
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new(r#""```E=mc^2^`` is the solution!`""#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(
                r#"&#8220;<code>E=mc<sup>2</sup></code> is the solution!&#8221;"#
                    .to_string()
                    .into_boxed_str()
            )
        );
    }

    #[test]
    fn double_quoted_string_around_almost_monospaced_text() {
        let mut content =
            crate::content::Content::from(crate::Span::new(r#""``E=mc^2^` is the solution!`""#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(
                r#"&#8220;`E=mc<sup>2</sup>` is the solution!&#8221;"#
                    .to_string()
                    .into_boxed_str()
            )
        );
    }

    #[test]
    fn single_line_single_quoted_string() {
        verifies!(
            r#"
    test 'single-line single-quoted string' do
      para = block_from_string(%q{`a few quoted words'}, attributes: { 'compat-mode' => '' })
      assert_equal '&#8216;a few quoted words&#8217;', para.sub_quotes(para.source)

      para = block_from_string(%q{'`a few quoted words`'})
      assert_equal '&#8216;a few quoted words&#8217;', para.sub_quotes(para.source)

      para = block_from_string(%q{'`a few quoted words`'}, backend: 'docbook')
      assert_equal '<quote>a few quoted words</quote>', para.sub_quotes(para.source)
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new(r#"'`a few quoted words`'"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"&#8216;a few quoted words&#8217;"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn escaped_single_line_single_quoted_string() {
        verifies!(
            r#"
    test 'escaped single-line single-quoted string' do
      para = block_from_string(%(#{BACKSLASH}`a few quoted words'), attributes: { 'compat-mode' => '' })
      assert_equal %(`a few quoted words'), para.sub_quotes(para.source)

      para = block_from_string(%(#{BACKSLASH}'`a few quoted words`'))
      assert_equal %('`a few quoted words`'), para.sub_quotes(para.source)
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new(r#"\'`a few quoted words`'"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"'`a few quoted words`'"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn multi_line_single_quoted_string() {
        verifies!(
            r#"
    test 'multi-line single-quoted string' do
      para = block_from_string(%Q{`a few\nquoted words'}, attributes: { 'compat-mode' => '' })
      assert_equal "&#8216;a few\nquoted words&#8217;", para.sub_quotes(para.source)

      para = block_from_string(%Q{'`a few\nquoted words`'})
      assert_equal "&#8216;a few\nquoted words&#8217;", para.sub_quotes(para.source)
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new("'`a few\nquoted words`'"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(
                "&#8216;a few\nquoted words&#8217;"
                    .to_string()
                    .into_boxed_str()
            )
        );
    }

    #[test]
    fn single_quoted_string_with_inline_single_quote() {
        verifies!(
            r#"
    test 'single-quoted string with inline single quote' do
      para = block_from_string(%q{`That isn't what I did.'}, attributes: { 'compat-mode' => '' })
      assert_equal %q{&#8216;That isn't what I did.&#8217;}, para.sub_quotes(para.source)

      para = block_from_string(%q{'`That isn't what I did.`'})
      assert_equal %q{&#8216;That isn't what I did.&#8217;}, para.sub_quotes(para.source)
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new(r#"'`That isn't what I did.`'"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"&#8216;That isn't what I did.&#8217;"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn single_quoted_string_with_inline_backquote() {
        verifies!(
            r#"
    test 'single-quoted string with inline backquote' do
      para = block_from_string(%q{`Here`s Johnny!'}, attributes: { 'compat-mode' => '' })
      assert_equal %q{&#8216;Here`s Johnny!&#8217;}, para.sub_quotes(para.source)

      para = block_from_string(%q{'`Here`s Johnny!`'})
      assert_equal %q{&#8216;Here`s Johnny!&#8217;}, para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(r#"'`Here`s Johnny!`'"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"&#8216;Here`s Johnny!&#8217;"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn single_line_constrained_marked_string() {
        verifies!(
            r#"
    test 'single-line constrained marked string' do
      #para = block_from_string(%q{#a few words#}, attributes: { 'compat-mode' => '' })
      #assert_equal 'a few words', para.sub_quotes(para.source)

      para = block_from_string(%q{#a few words#})
      assert_equal '<mark>a few words</mark>', para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(r#"#a few words#"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"<mark>a few words</mark>"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn escaped_single_line_constrained_marked_string() {
        verifies!(
            r#"
    test 'escaped single-line constrained marked string' do
      para = block_from_string(%(#{BACKSLASH}#a few words#))
      assert_equal '#a few words#', para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(r#"\#a few words#"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"#a few words#"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn multi_line_constrained_marked_string() {
        verifies!(
            r#"
    test 'multi-line constrained marked string' do
      #para = block_from_string(%Q{#a few\nwords#}, attributes: { 'compat-mode' => '' })
      #assert_equal "a few\nwords", para.sub_quotes(para.source)

      para = block_from_string(%Q{#a few\nwords#})
      assert_equal "<mark>a few\nwords</mark>", para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new("#a few\nwords#"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed("<mark>a few\nwords</mark>".to_string().into_boxed_str())
        );
    }

    #[test]
    fn constrained_marked_string_should_not_match_entity_references() {
        verifies!(
            r#"
    test 'constrained marked string should not match entity references' do
      para = block_from_string('111 #mark a# 222 "`quote a`" 333 #mark b# 444')
      assert_equal %(111 <mark>mark a</mark> 222 &#8220;quote a&#8221; 333 <mark>mark b</mark> 444), para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(
            r##"111 #mark a# 222 "`quote a`" 333 #mark b# 444"##,
        ));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r##"111 <mark>mark a</mark> 222 &#8220;quote a&#8221; 333 <mark>mark b</mark> 444"##.to_string().into_boxed_str())
        );
    }

    #[test]
    fn single_line_unconstrained_marked_string() {
        verifies!(
            r#"
    test 'single-line unconstrained marked string' do
      #para = block_from_string(%q{##--anything goes ##}, attributes: { 'compat-mode' => '' })
      #assert_equal '--anything goes ', para.sub_quotes(para.source)

      para = block_from_string(%q{##--anything goes ##})
      assert_equal '<mark>--anything goes </mark>', para.sub_quotes(para.source)
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new(r###"##--anything goes ##"###));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(
                r###"<mark>--anything goes </mark>"###
                    .to_string()
                    .into_boxed_str()
            )
        );
    }

    #[test]
    fn escaped_single_line_unconstrained_marked_string() {
        verifies!(
            r#"
    test 'escaped single-line unconstrained marked string' do
      para = block_from_string(%(#{BACKSLASH}#{BACKSLASH}##--anything goes ##))
      assert_equal '##--anything goes ##', para.sub_quotes(para.source)
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new(r###"\\##--anything goes ##"###));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r###"##--anything goes ##"###.to_string().into_boxed_str())
        );
    }

    #[test]
    fn multi_line_unconstrained_marked_string() {
        verifies!(
            r#"
    test 'multi-line unconstrained marked string' do
      #para = block_from_string(%Q{##--anything\ngoes ##}, attributes: { 'compat-mode' => '' })
      #assert_equal "--anything\ngoes ", para.sub_quotes(para.source)

      para = block_from_string(%Q{##--anything\ngoes ##})
      assert_equal "<mark>--anything\ngoes </mark>", para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new("##--anything\ngoes ##"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(
                "<mark>--anything\ngoes </mark>"
                    .to_string()
                    .into_boxed_str()
            )
        );
    }

    #[test]
    fn single_line_constrained_marked_string_with_role() {
        verifies!(
            r#"
    test 'single-line constrained marked string with role' do
      para = block_from_string(%q{[statement]#a few words#})
      assert_equal '<span class="statement">a few words</span>', para.sub_quotes(para.source)
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new(r##"[statement]#a few words#"##));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(
                r#"<span class="statement">a few words</span>"#
                    .to_string()
                    .into_boxed_str()
            )
        );
    }

    #[test]
    fn single_line_constrained_strong_string() {
        verifies!(
            r#"
    test 'single-line constrained strong string' do
      para = block_from_string(%q{*a few strong words*})
      assert_equal '<strong>a few strong words</strong>', para.sub_quotes(para.source)
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new(r#"*a few strong words*"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"<strong>a few strong words</strong>"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn escaped_single_line_constrained_strong_string() {
        verifies!(
            r#"
    test 'escaped single-line constrained strong string' do
      para = block_from_string(%(#{BACKSLASH}*a few strong words*))
      assert_equal '*a few strong words*', para.sub_quotes(para.source)
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new(r#"\*a few strong words*"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"*a few strong words*"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn multi_line_constrained_strong_string() {
        verifies!(
            r#"
    test 'multi-line constrained strong string' do
      para = block_from_string(%Q{*a few\nstrong words*})
      assert_equal "<strong>a few\nstrong words</strong>", para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new("*a few\nstrong words*"));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(
                "<strong>a few\nstrong words</strong>"
                    .to_string()
                    .into_boxed_str()
            )
        );
    }

    #[test]
    fn constrained_strong_string_containing_an_asterisk() {
        verifies!(
            r#"
    test 'constrained strong string containing an asterisk' do
      para = block_from_string(%q{*bl*ck*-eye})
      assert_equal '<strong>bl*ck</strong>-eye', para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new("*bl*ck*-eye"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(
            content.rendered,
            CowStr::Boxed("<strong>bl*ck</strong>-eye".to_string().into_boxed_str())
        );
    }

    #[test]
    fn constrained_strong_string_containing_an_asterisk_and_multibyte_word_chars() {
        verifies!(
            r#"
    test 'constrained strong string containing an asterisk and multibyte word chars' do
      para = block_from_string(%q{*黑*眼圈*})
      assert_equal '<strong>黑*眼圈</strong>', para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new("*黑*眼圈*"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed("<strong>黑*眼圈</strong>".to_string().into_boxed_str())
        );
    }

    #[test]
    fn single_line_constrained_quote_variation_emphasized_string() {
        verifies!(
            r#"
    test 'single-line constrained quote variation emphasized string' do
      para = block_from_string(%q{_a few emphasized words_})
      assert_equal '<em>a few emphasized words</em>', para.sub_quotes(para.source)
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new("_a few emphasized words_"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(
                "<em>a few emphasized words</em>"
                    .to_string()
                    .into_boxed_str()
            )
        );
    }

    #[test]
    fn escaped_single_line_constrained_quote_variation_emphasized_string() {
        verifies!(
            r#"
    test 'escaped single-line constrained quote variation emphasized string' do
      para = block_from_string(%(#{BACKSLASH}_a few emphasized words_))
      assert_equal %q(_a few emphasized words_), para.sub_quotes(para.source)
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new("\\_a few emphasized words_"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed("_a few emphasized words_".to_string().into_boxed_str())
        );
    }

    #[test]
    fn escaped_single_quoted_string() {
        verifies!(
            r#"
    test 'escaped single quoted string' do
      para = block_from_string(%(#{BACKSLASH}'a few emphasized words'))
      # NOTE the \' is replaced with ' by the :replacements substitution, later in the substitution pipeline
      assert_equal %(#{BACKSLASH}'a few emphasized words'), para.sub_quotes(para.source)
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new(r#"\'a few emphasized words'"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"\'a few emphasized words'"#)
        );
    }

    #[test]
    fn multi_line_constrained_emphasized_quote_variation_string() {
        verifies!(
            r#"
    test 'multi-line constrained emphasized quote variation string' do
      para = block_from_string(%Q{_a few\nemphasized words_})
      assert_equal "<em>a few\nemphasized words</em>", para.sub_quotes(para.source)
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new("_a few\nemphasized words_"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed("<em>a few\nemphasized words</em>")
        );
    }

    #[test]
    fn single_quoted_string_containing_an_emphasized_phrase() {
        verifies!(
            r#"
    test 'single-quoted string containing an emphasized phrase' do
      para = block_from_string(%q{`I told him, 'Just go for it!''}, attributes: { 'compat-mode' => '' })
      assert_equal '&#8216;I told him, <em>Just go for it!</em>&#8217;', para.sub_quotes(para.source)

      para = block_from_string(%q{'`I told him, 'Just go for it!'`'})
      assert_equal %q(&#8216;I told him, 'Just go for it!'&#8217;), para.sub_quotes(para.source)
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new(r#"'`I told him, 'Just go for it!'`'"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"&#8216;I told him, 'Just go for it!'&#8217;"#)
        );
    }

    #[test]
    fn escaped_single_quotes_inside_emphasized_words_are_restored() {
        verifies!(
            r#"
    test 'escaped single-quotes inside emphasized words are restored' do
      para = block_from_string(%('Here#{BACKSLASH}'s Johnny!'), attributes: { 'compat-mode' => '' })
      assert_equal %q(<em>Here's Johnny!</em>), para.apply_subs(para.source)

      para = block_from_string(%('Here#{BACKSLASH}'s Johnny!'))
      assert_equal %q('Here's Johnny!'), para.apply_subs(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(r#"'Here\'s Johnny!'"#));
        let p = Parser::default();
        SubstitutionGroup::Normal.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed(r#"'Here's Johnny!'"#));
    }

    #[test]
    fn single_line_constrained_emphasized_underline_variation_string() {
        verifies!(
            r#"
    test 'single-line constrained emphasized underline variation string' do
      para = block_from_string(%q{_a few emphasized words_})
      assert_equal '<em>a few emphasized words</em>', para.sub_quotes(para.source)
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new(r#"_a few emphasized words_"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"<em>a few emphasized words</em>"#)
        );
    }

    #[test]
    fn escaped_single_line_constrained_emphasized_underline_variation_string() {
        verifies!(
            r#"
    test 'escaped single-line constrained emphasized underline variation string' do
      para = block_from_string(%(#{BACKSLASH}_a few emphasized words_))
      assert_equal '_a few emphasized words_', para.sub_quotes(para.source)
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new(r#"\_a few emphasized words_"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"_a few emphasized words_"#)
        );
    }

    #[test]
    fn multi_line_constrained_emphasized_underline_variation_string() {
        verifies!(
            r#"
    test 'multi-line constrained emphasized underline variation string' do
      para = block_from_string(%Q{_a few\nemphasized words_})
      assert_equal "<em>a few\nemphasized words</em>", para.sub_quotes(para.source)
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new("_a few\nemphasized words_"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed("<em>a few\nemphasized words</em>")
        );
    }

    non_normative!(
        r#"
    # NOTE must use apply_subs because constrained monospaced is handled as a passthrough
"#
    );

    #[test]
    fn single_line_constrained_monospaced_string() {
        // The upstream test's first sub-case exercises
        // compatibility mode, which this crate does not
        // support; it is reproduced non-normatively. Only
        // the modern sub-case below is verified.
        non_normative!(
            r#"
    test 'single-line constrained monospaced string' do
      para = block_from_string(%(`a few <{monospaced}> words`), attributes: { 'monospaced' => 'monospaced', 'compat-mode' => '' })
      assert_equal '<code>a few &lt;{monospaced}&gt; words</code>', para.apply_subs(para.source)

"#
        );

        verifies!(
            r#"
      para = block_from_string(%(`a few <{monospaced}> words`), attributes: { 'monospaced' => 'monospaced' })
      assert_equal '<code>a few &lt;monospaced&gt; words</code>', para.apply_subs(para.source)
    end

"#
        );

        let mut p = Parser::default().with_intrinsic_attribute(
            "monospaced",
            "monospaced",
            ModificationContext::Anywhere,
        );

        let maw =
            crate::blocks::Block::parse(crate::Span::new("`a few <{monospaced}> words`"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "`a few <{monospaced}> words`",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "<code>a few &lt;monospaced&gt; words</code>",
                },
                source: Span {
                    data: "`a few <{monospaced}> words`",
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
            },)
        );
    }

    non_normative!(
        r#"
    # NOTE must use apply_subs because constrained monospaced is handled as a passthrough
"#
    );

    #[test]
    fn single_line_constrained_monospaced_string_with_role() {
        // The upstream test's first sub-case exercises
        // compatibility mode, which this crate does not
        // support; it is reproduced non-normatively. Only
        // the modern sub-case below is verified.
        non_normative!(
            r#"
    test 'single-line constrained monospaced string with role' do
      para = block_from_string(%([input]`a few <{monospaced}> words`), attributes: { 'monospaced' => 'monospaced', 'compat-mode' => '' })
      assert_equal '<code class="input">a few &lt;{monospaced}&gt; words</code>', para.apply_subs(para.source)

"#
        );

        verifies!(
            r#"
      para = block_from_string(%([input]`a few <{monospaced}> words`), attributes: { 'monospaced' => 'monospaced' })
      assert_equal '<code class="input">a few &lt;monospaced&gt; words</code>', para.apply_subs(para.source)
    end

"#
        );

        let mut p = Parser::default().with_intrinsic_attribute(
            "monospaced",
            "monospaced",
            ModificationContext::Anywhere,
        );

        let maw = crate::blocks::Block::parse(
            crate::Span::new("[input]`a few <{monospaced}> words`"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "[input]`a few <{monospaced}> words`",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "<code class=\"input\">a few &lt;monospaced&gt; words</code>",
                },
                source: Span {
                    data: "[input]`a few <{monospaced}> words`",
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
            },)
        );
    }

    non_normative!(
        r#"
    # NOTE must use apply_subs because constrained monospaced is handled as a passthrough
"#
    );

    #[test]
    fn escaped_single_line_constrained_monospaced_string() {
        // The upstream test's first sub-case exercises
        // compatibility mode, which this crate does not
        // support; it is reproduced non-normatively. Only
        // the modern sub-case below is verified.
        non_normative!(
            r#"
    test 'escaped single-line constrained monospaced string' do
      para = block_from_string(%(#{BACKSLASH}`a few <monospaced> words`), attributes: { 'compat-mode' => '' })
      assert_equal '`a few &lt;monospaced&gt; words`', para.apply_subs(para.source)

"#
        );

        verifies!(
            r#"
      para = block_from_string(%(#{BACKSLASH}`a few <monospaced> words`))
      assert_equal '`a few &lt;monospaced&gt; words`', para.apply_subs(para.source)
    end

"#
        );

        let mut p = Parser::default().with_intrinsic_attribute(
            "monospaced",
            "monospaced",
            ModificationContext::Anywhere,
        );

        let maw =
            crate::blocks::Block::parse(crate::Span::new("\\`a few <monospaced> words`"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "\\`a few <monospaced> words`",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "`a few &lt;monospaced&gt; words`",
                },
                source: Span {
                    data: "\\`a few <monospaced> words`",
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
            },)
        );
    }

    non_normative!(
        r#"
    # NOTE must use apply_subs because constrained monospaced is handled as a passthrough
"#
    );

    #[test]
    fn escaped_single_line_constrained_monospaced_string_with_role() {
        // The upstream test's first sub-case exercises
        // compatibility mode, which this crate does not
        // support; it is reproduced non-normatively. Only
        // the modern sub-case below is verified.
        non_normative!(
            r#"
    test 'escaped single-line constrained monospaced string with role' do
      para = block_from_string(%([input]#{BACKSLASH}`a few <monospaced> words`), attributes: { 'compat-mode' => '' })
      assert_equal '[input]`a few &lt;monospaced&gt; words`', para.apply_subs(para.source)

"#
        );

        verifies!(
            r#"
      para = block_from_string(%([input]#{BACKSLASH}`a few <monospaced> words`))
      assert_equal '[input]`a few &lt;monospaced&gt; words`', para.apply_subs(para.source)
    end

"#
        );

        let mut p = Parser::default().with_intrinsic_attribute(
            "monospaced",
            "monospaced",
            ModificationContext::Anywhere,
        );

        let maw = crate::blocks::Block::parse(
            crate::Span::new("[input]\\`a few <monospaced> words`"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "[input]\\`a few <monospaced> words`",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "[input]`a few &lt;monospaced&gt; words`",
                },
                source: Span {
                    data: "[input]\\`a few <monospaced> words`",
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
            },)
        );
    }

    non_normative!(
        r#"
    # NOTE must use apply_subs because constrained monospaced is handled as a passthrough
"#
    );

    #[test]
    fn escaped_role_on_single_line_constrained_monospaced_string() {
        // The upstream test's first sub-case exercises
        // compatibility mode, which this crate does not
        // support; it is reproduced non-normatively. Only
        // the modern sub-case below is verified.
        non_normative!(
            r#"
    test 'escaped role on single-line constrained monospaced string' do
      para = block_from_string(%(#{BACKSLASH}[input]`a few <monospaced> words`), attributes: { 'compat-mode' => '' })
      assert_equal '[input]<code>a few &lt;monospaced&gt; words</code>', para.apply_subs(para.source)

"#
        );

        verifies!(
            r#"
      para = block_from_string(%(#{BACKSLASH}[input]`a few <monospaced> words`))
      assert_equal '[input]<code>a few &lt;monospaced&gt; words</code>', para.apply_subs(para.source)
    end

"#
        );

        let mut p = Parser::default().with_intrinsic_attribute(
            "monospaced",
            "monospaced",
            ModificationContext::Anywhere,
        );

        let maw = crate::blocks::Block::parse(
            crate::Span::new("\\[input]`a few <monospaced> words`"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "\\[input]`a few <monospaced> words`",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "[input]<code>a few &lt;monospaced&gt; words</code>",
                },
                source: Span {
                    data: "\\[input]`a few <monospaced> words`",
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
            },)
        );
    }

    non_normative!(
        r#"
    # NOTE must use apply_subs because constrained monospaced is handled as a passthrough
"#
    );

    #[test]
    fn escaped_role_on_escaped_single_line_constrained_monospaced_string() {
        // The upstream test's first sub-case exercises
        // compatibility mode, which this crate does not
        // support; it is reproduced non-normatively. Only
        // the modern sub-case below is verified.
        non_normative!(
            r#"
    test 'escaped role on escaped single-line constrained monospaced string' do
      para = block_from_string(%(#{BACKSLASH}[input]#{BACKSLASH}`a few <monospaced> words`), attributes: { 'compat-mode' => '' })
      assert_equal %(#{BACKSLASH}[input]`a few &lt;monospaced&gt; words`), para.apply_subs(para.source)

"#
        );

        verifies!(
            r#"
      para = block_from_string(%(#{BACKSLASH}[input]#{BACKSLASH}`a few <monospaced> words`))
      assert_equal %(#{BACKSLASH}[input]`a few &lt;monospaced&gt; words`), para.apply_subs(para.source)
    end

"#
        );

        let mut p = Parser::default().with_intrinsic_attribute(
            "monospaced",
            "monospaced",
            ModificationContext::Anywhere,
        );

        let maw = crate::blocks::Block::parse(
            crate::Span::new("\\[input]\\`a few <monospaced> words`"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "\\[input]\\`a few <monospaced> words`",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "\\[input]`a few &lt;monospaced&gt; words`",
                },
                source: Span {
                    data: "\\[input]\\`a few <monospaced> words`",
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
            },)
        );
    }

    non_normative!(
        r#"
    # NOTE must use apply_subs because constrained monospaced is handled as a passthrough
"#
    );

    #[test]
    fn should_ignore_role_that_ends_with_transitional_role_on_constrained_monospace_span() {
        verifies!(
            r#"
    test 'should ignore role that ends with transitional role on constrained monospace span' do
      para = block_from_string %([foox-]`leave it alone`)
      assert_equal '<code class="foox-">leave it alone</code>', para.apply_subs(para.source)
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new("[foox-]`leave it alone`"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "[foox-]`leave it alone`",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<code class="foox-">leave it alone</code>"#,
                },
                source: Span {
                    data: "[foox-]`leave it alone`",
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
            },)
        );
    }

    non_normative!(
        r#"
    # NOTE must use apply_subs because constrained monospaced is handled as a passthrough
"#
    );

    #[test]
    fn escaped_single_line_constrained_monospace_string_with_forced_compat_role() {
        verifies!(
            r#"
    test 'escaped single-line constrained monospace string with forced compat role' do
      para = block_from_string %([x-]#{BACKSLASH}`leave it alone`)
      assert_equal '[x-]`leave it alone`', para.apply_subs(para.source)
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new(r#"[x-]\`leave it alone`"#), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"[x-]\`leave it alone`"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "[x-]`leave it alone`",
                },
                source: Span {
                    data: r#"[x-]\`leave it alone`"#,
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
            },)
        );
    }

    non_normative!(
        r#"
    # NOTE must use apply_subs because constrained monospaced is handled as a passthrough
"#
    );

    #[test]
    fn escaped_forced_compat_role_on_single_line_constrained_monospace_string() {
        verifies!(
            r#"
    test 'escaped forced compat role on single-line constrained monospace string' do
      para = block_from_string %(#{BACKSLASH}[x-]`just *mono*`)
      assert_equal '[x-]<code>just <strong>mono</strong></code>', para.apply_subs(para.source)
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new(r#"\[x-]`just *mono*`"#), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"\[x-]`just *mono*`"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "[x-]<code>just <strong>mono</strong></code>",
                },
                source: Span {
                    data: r#"\[x-]`just *mono*`"#,
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
            },)
        );
    }

    non_normative!(
        r#"
    # NOTE must use apply_subs because constrained monospaced is handled as a passthrough
"#
    );

    #[test]
    fn multi_line_constrained_monospaced_string() {
        // The upstream test's first sub-case exercises
        // compatibility mode, which this crate does not
        // support; it is reproduced non-normatively. Only
        // the modern sub-case below is verified.
        non_normative!(
            r#"
    test 'multi-line constrained monospaced string' do
      para = block_from_string(%(`a few\n<{monospaced}> words`), attributes: { 'monospaced' => 'monospaced', 'compat-mode' => '' })
      assert_equal "<code>a few\n&lt;{monospaced}&gt; words</code>", para.apply_subs(para.source)

"#
        );

        verifies!(
            r#"
      para = block_from_string(%(`a few\n<{monospaced}> words`), attributes: { 'monospaced' => 'monospaced' })
      assert_equal "<code>a few\n&lt;monospaced&gt; words</code>", para.apply_subs(para.source)
    end

"#
        );

        let mut p = Parser::default().with_intrinsic_attribute(
            "monospaced",
            "monospaced",
            ModificationContext::Anywhere,
        );

        let maw =
            crate::blocks::Block::parse(crate::Span::new("`a few\n<{monospaced}> words`"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "`a few\n<{monospaced}> words`",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "<code>a few\n&lt;monospaced&gt; words</code>",
                },
                source: Span {
                    data: "`a few\n<{monospaced}> words`",
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
            },)
        );
    }

    #[test]
    fn single_line_unconstrained_strong_chars() {
        verifies!(
            r#"
    test 'single-line unconstrained strong chars' do
      para = block_from_string(%q{**Git**Hub})
      assert_equal '<strong>Git</strong>Hub', para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(r#"**Git**Hub"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed("<strong>Git</strong>Hub")
        );
    }

    #[test]
    fn escaped_single_line_unconstrained_strong_chars() {
        verifies!(
            r#"
    test 'escaped single-line unconstrained strong chars' do
      para = block_from_string(%(#{BACKSLASH}**Git**Hub))
      assert_equal '<strong>*Git</strong>*Hub', para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(r#"\**Git**Hub"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed("<strong>*Git</strong>*Hub")
        );
    }

    #[test]
    fn multi_line_unconstrained_strong_chars() {
        verifies!(
            r#"
    test 'multi-line unconstrained strong chars' do
      para = block_from_string(%Q{**G\ni\nt\n**Hub})
      assert_equal "<strong>G\ni\nt\n</strong>Hub", para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new("**G\ni\nt\n**Hub"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed("<strong>G\ni\nt\n</strong>Hub")
        );
    }

    #[test]
    fn unconstrained_strong_chars_with_inline_asterisk() {
        verifies!(
            r#"
    test 'unconstrained strong chars with inline asterisk' do
      para = block_from_string(%q{**bl*ck**-eye})
      assert_equal '<strong>bl*ck</strong>-eye', para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new("**bl*ck**-eye"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed("<strong>bl*ck</strong>-eye")
        );
    }

    #[test]
    fn unconstrained_strong_chars_with_role() {
        verifies!(
            r#"
    test 'unconstrained strong chars with role' do
      para = block_from_string(%q{Git[blue]**Hub**})
      assert_equal %q{Git<strong class="blue">Hub</strong>}, para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new("Git[blue]**Hub**"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"Git<strong class="blue">Hub</strong>"#)
        );
    }

    non_normative!(
        r#"
    # TODO this is not the same result as AsciiDoc, though I don't understand why AsciiDoc gets what it gets
"#
    );

    #[test]
    fn escaped_unconstrained_strong_chars_with_role() {
        verifies!(
            r#"
    test 'escaped unconstrained strong chars with role' do
      para = block_from_string(%(Git#{BACKSLASH}[blue]**Hub**))
      assert_equal %q{Git[blue]<strong>*Hub</strong>*}, para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(r#"Git\[blue]**Hub**"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"Git[blue]<strong>*Hub</strong>*"#)
        );
    }

    #[test]
    fn single_line_unconstrained_emphasized_characters() {
        verifies!(
            r#"
    test 'single-line unconstrained emphasized chars' do
      para = block_from_string(%q{__Git__Hub})
      assert_equal '<em>Git</em>Hub', para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new("__Git__Hub"));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed("<em>Git</em>Hub"));
    }

    #[test]
    fn escaped_single_line_unconstrained_emphasized_characters() {
        verifies!(
            r#"
    test 'escaped single-line unconstrained emphasized chars' do
      para = block_from_string(%(#{BACKSLASH}__Git__Hub))
      assert_equal '__Git__Hub', para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(r#"\__Git__Hub"#));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed("__Git__Hub"));
    }

    #[test]
    fn escaped_single_line_unconstrained_emphasized_characters_around_word() {
        verifies!(
            r#"
    test 'escaped single-line unconstrained emphasized chars around word' do
      para = block_from_string(%(#{BACKSLASH}#{BACKSLASH}__GitHub__))
      assert_equal '__GitHub__', para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(r#"\\__GitHub__"#));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed("__GitHub__"));
    }

    #[test]
    fn multi_line_unconstrained_emphasized_chars() {
        verifies!(
            r#"
    test 'multi-line unconstrained emphasized chars' do
      para = block_from_string(%Q{__G\ni\nt\n__Hub})
      assert_equal "<em>G\ni\nt\n</em>Hub", para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new("__G\ni\nt\n__Hub"));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed("<em>G\ni\nt\n</em>Hub"));
    }

    #[test]
    fn unconstrained_emphasis_chars_with_role() {
        verifies!(
            r#"
    test 'unconstrained emphasis chars with role' do
      para = block_from_string(%q{[gray]__Git__Hub})
      assert_equal %q{<em class="gray">Git</em>Hub}, para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new("[gray]__Git__Hub"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"<em class="gray">Git</em>Hub"#)
        );
    }

    #[test]
    fn escaped_unconstrained_emphasis_chars_with_role() {
        verifies!(
            r#"
    test 'escaped unconstrained emphasis chars with role' do
      para = block_from_string(%(#{BACKSLASH}[gray]__Git__Hub))
      assert_equal %q{[gray]__Git__Hub}, para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new("\\[gray]__Git__Hub"));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed(r#"[gray]__Git__Hub"#));
    }

    #[test]
    fn single_line_constrained_monospaced_chars_1() {
        verifies!(
            r#"
    test 'single-line constrained monospaced chars' do
      para = block_from_string(%q{call +save()+ to persist the changes}, attributes: { 'compat-mode' => '' })
      assert_equal 'call <code>save()</code> to persist the changes', para.sub_quotes(para.source)

      para = block_from_string(%q{call [x-]+save()+ to persist the changes})
      assert_equal 'call <code>save()</code> to persist the changes', para.apply_subs(para.source)

      para = block_from_string(%q{call `save()` to persist the changes})
      assert_equal 'call <code>save()</code> to persist the changes', para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(
            "call [x-]+save()+ to persist the changes",
        ));

        let p = Parser::default();
        SubstitutionGroup::Normal.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"call <code>save()</code> to persist the changes"#)
        );
    }

    #[test]
    fn single_line_constrained_monospaced_chars_2() {
        let mut content =
            crate::content::Content::from(crate::Span::new("call `save()` to persist the changes"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"call <code>save()</code> to persist the changes"#)
        );
    }

    #[test]
    fn single_line_constrained_monospaced_chars_with_role_1() {
        verifies!(
            r#"
    test 'single-line constrained monospaced chars with role' do
      para = block_from_string(%q{call [method]+save()+ to persist the changes}, attributes: { 'compat-mode' => '' })
      assert_equal 'call <code class="method">save()</code> to persist the changes', para.sub_quotes(para.source)

      para = block_from_string(%q{call [method x-]+save()+ to persist the changes})
      assert_equal 'call <code class="method">save()</code> to persist the changes', para.apply_subs(para.source)

      para = block_from_string(%q{call [method]`save()` to persist the changes})
      assert_equal 'call <code class="method">save()</code> to persist the changes', para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(
            "call [method x-]+save()+ to persist the changes",
        ));

        let p = Parser::default();
        SubstitutionGroup::Normal.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"call <code class="method">save()</code> to persist the changes"#)
        );
    }

    #[test]
    fn single_line_constrained_monospaced_chars_with_role_2() {
        let mut content = crate::content::Content::from(crate::Span::new(
            "call [method]`save()` to persist the changes",
        ));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"call <code class="method">save()</code> to persist the changes"#)
        );
    }

    #[test]
    fn escaped_single_line_constrained_monospaced_chars() {
        verifies!(
            r#"
    test 'escaped single-line constrained monospaced chars' do
      para = block_from_string(%(call #{BACKSLASH}+save()+ to persist the changes), attributes: { 'compat-mode' => '' })
      assert_equal 'call +save()+ to persist the changes', para.sub_quotes(para.source)

      para = block_from_string(%(call #{BACKSLASH}`save()` to persist the changes))
      assert_equal 'call `save()` to persist the changes', para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(
            r#"call \`save()` to persist the changes"#,
        ));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"call `save()` to persist the changes"#)
        );
    }

    #[test]
    fn escaped_single_line_constrained_monospaced_chars_with_role() {
        verifies!(
            r#"
    test 'escaped single-line constrained monospaced chars with role' do
      para = block_from_string(%(call [method]#{BACKSLASH}+save()+ to persist the changes), attributes: { 'compat-mode' => '' })
      assert_equal 'call [method]+save()+ to persist the changes', para.sub_quotes(para.source)

      para = block_from_string(%(call [method]#{BACKSLASH}`save()` to persist the changes))
      assert_equal 'call [method]`save()` to persist the changes', para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(
            r#"call [method]\`save()` to persist the changes"#,
        ));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"call [method]`save()` to persist the changes"#)
        );
    }

    #[test]
    fn escaped_role_on_single_line_constrained_monospaced_chars() {
        verifies!(
            r#"
    test 'escaped role on single-line constrained monospaced chars' do
      para = block_from_string(%(call #{BACKSLASH}[method]+save()+ to persist the changes), attributes: { 'compat-mode' => '' })
      assert_equal 'call [method]<code>save()</code> to persist the changes', para.sub_quotes(para.source)

      para = block_from_string(%(call #{BACKSLASH}[method]`save()` to persist the changes))
      assert_equal 'call [method]<code>save()</code> to persist the changes', para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(
            r#"call \[method]`save()` to persist the changes"#,
        ));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"call [method]<code>save()</code> to persist the changes"#)
        );
    }

    #[test]
    fn escaped_role_on_escaped_single_line_constrained_monospaced_chars() {
        verifies!(
            r#"
    test 'escaped role on escaped single-line constrained monospaced chars' do
      para = block_from_string(%(call #{BACKSLASH}[method]#{BACKSLASH}+save()+ to persist the changes), attributes: { 'compat-mode' => '' })
      assert_equal %(call #{BACKSLASH}[method]+save()+ to persist the changes), para.sub_quotes(para.source)

      para = block_from_string(%(call #{BACKSLASH}[method]#{BACKSLASH}`save()` to persist the changes))
      assert_equal %(call #{BACKSLASH}[method]`save()` to persist the changes), para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(
            r#"call \[method]\`save()` to persist the changes"#,
        ));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"call \[method]`save()` to persist the changes"#)
        );
    }

    non_normative!(
        r#"
    # NOTE must use apply_subs because constrained monospaced is handled as a passthrough
"#
    );

    #[test]
    fn escaped_single_line_constrained_passthrough_string() {
        verifies!(
            r#"
    test 'escaped single-line constrained passthrough string with forced compat role' do
      para = block_from_string %([x-]#{BACKSLASH}+leave it alone+)
      assert_equal '[x-]+leave it alone+', para.apply_subs(para.source)
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new(r#"[x-]\+leave it alone+"#));

        let p = Parser::default();
        SubstitutionGroup::Normal.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"[x-]+leave it alone+"#)
        );
    }

    #[test]
    fn single_line_unconstrained_monospaced_chars_1() {
        verifies!(
            r#"
    test 'single-line unconstrained monospaced chars' do
      para = block_from_string(%q{Git++Hub++}, attributes: { 'compat-mode' => '' })
      assert_equal 'Git<code>Hub</code>', para.sub_quotes(para.source)

      para = block_from_string(%q{Git[x-]++Hub++})
      assert_equal 'Git<code>Hub</code>', para.apply_subs(para.source)

      para = block_from_string(%q{Git``Hub``})
      assert_equal 'Git<code>Hub</code>', para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new("Git[x-]++Hub++"));
        let p = Parser::default();
        SubstitutionGroup::Normal.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed(r#"Git<code>Hub</code>"#));
    }

    #[test]
    fn single_line_unconstrained_monospaced_chars_2() {
        let mut content = crate::content::Content::from(crate::Span::new("Git``Hub``"));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed(r#"Git<code>Hub</code>"#));
    }

    #[test]
    fn single_line_unconstrained_monospaced_chars_with_old_behavior_and_role() {
        // NOTE: Not in the Ruby test suite.
        let mut content = crate::content::Content::from(crate::Span::new("Git[test x-]++Hub++"));

        let p = Parser::default();
        SubstitutionGroup::Normal.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"Git<code class="test">Hub</code>"#)
        );
    }

    #[test]
    fn escaped_single_line_unconstrained_monospaced_chars() {
        verifies!(
            r#"
    test 'escaped single-line unconstrained monospaced chars' do
      para = block_from_string(%(Git#{BACKSLASH}++Hub++), attributes: { 'compat-mode' => '' })
      assert_equal 'Git+<code>Hub</code>+', para.sub_quotes(para.source)

      para = block_from_string(%(Git#{BACKSLASH * 2}++Hub++), attributes: { 'compat-mode' => '' })
      assert_equal 'Git++Hub++', para.sub_quotes(para.source)

      para = block_from_string(%(Git#{BACKSLASH}``Hub``))
      assert_equal 'Git``Hub``', para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(r#"Git\``Hub``"#));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed(r#"Git``Hub``"#));
    }

    #[test]
    fn multi_line_unconstrained_monospaced_chars_1() {
        verifies!(
            r#"
    test 'multi-line unconstrained monospaced chars' do
      para = block_from_string(%Q{Git++\nH\nu\nb++}, attributes: { 'compat-mode' => '' })
      assert_equal "Git<code>\nH\nu\nb</code>", para.sub_quotes(para.source)

      para = block_from_string(%Q{Git[x-]++\nH\nu\nb++})
      assert_equal %(Git<code>\nH\nu\nb</code>), para.apply_subs(para.source)

      para = block_from_string(%Q{Git``\nH\nu\nb``})
      assert_equal "Git<code>\nH\nu\nb</code>", para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new("Git[x-]++\nH\nu\nb++"));

        let p = Parser::default();
        SubstitutionGroup::Normal.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed("Git<code>\nH\nu\nb</code>")
        );
    }

    #[test]
    fn multi_line_unconstrained_monospaced_chars_2() {
        let mut content = crate::content::Content::from(crate::Span::new("Git``\nH\nu\nb``"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed("Git<code>\nH\nu\nb</code>")
        );
    }

    #[test]
    fn single_line_superscript_chars() {
        verifies!(
            r#"
    test 'single-line superscript chars' do
      para = block_from_string(%(x^2^ = x * x, e = mc^2^, there's a 1^st^ time for everything))
      assert_equal %(x<sup>2</sup> = x * x, e = mc<sup>2</sup>, there\'s a 1<sup>st</sup> time for everything), para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(
            "x^2^ = x * x, e = mc^2^, there's a 1^st^ time for everything",
        ));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(
                "x<sup>2</sup> = x * x, e = mc<sup>2</sup>, there\'s a 1<sup>st</sup> time for everything"
            )
        );
    }

    #[test]
    fn escaped_single_line_superscript_chars() {
        verifies!(
            r#"
    test 'escaped single-line superscript chars' do
      para = block_from_string(%(x#{BACKSLASH}^2^ = x * x))
      assert_equal 'x^2^ = x * x', para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(r#"x\^2^ = x * x"#));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed("x^2^ = x * x"));
    }

    #[test]
    fn does_not_match_superscript_across_whitespace() {
        verifies!(
            r#"
    test 'does not match superscript across whitespace' do
      para = block_from_string(%Q{x^(n\n-\n1)^})
      assert_equal para.source, para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new("x^(n\n-\n1)^"));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed("x^(n\n-\n1)^"));
    }

    #[test]
    fn allow_spaces_in_superscript_if_spaces_are_inserted_using_an_attribute_reference() {
        verifies!(
            r#"
    test 'allow spaces in superscript if spaces are inserted using an attribute reference' do
      para = block_from_string 'Night ^A{sp}poem{sp}by{sp}Jane{sp}Kondo^.'
      assert_equal 'Night <sup>A poem by Jane Kondo</sup>.', para.apply_subs(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(
            "Night ^A{sp}poem{sp}by{sp}Jane{sp}Kondo^.",
        ));

        let p = Parser::default();

        SubstitutionGroup::Normal.apply(&mut content, &p, None);

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"Night <sup>A poem by Jane Kondo</sup>."#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn allow_spaces_in_superscript_if_text_is_wrapped_in_a_passthrough() {
        verifies!(
            r#"
    test 'allow spaces in superscript if text is wrapped in a passthrough' do
      para = block_from_string 'Night ^+A poem by Jane Kondo+^.'
      assert_equal 'Night <sup>A poem by Jane Kondo</sup>.', para.apply_subs(para.source)
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new("Night ^+A poem by Jane Kondo+^."));

        let p = Parser::default();
        SubstitutionGroup::Normal.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed("Night <sup>A poem by Jane Kondo</sup>.")
        );
    }

    #[test]
    fn does_not_match_adjacent_superscript_chars() {
        verifies!(
            r#"
    test 'does not match adjacent superscript chars' do
      para = block_from_string 'a ^^ b'
      assert_equal 'a ^^ b', para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new("a ^^ b"));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed("a ^^ b"));
    }

    #[ignore]
    #[test]
    fn does_not_confuse_superscript_and_links_with_blank_window_shorthand() {
        verifies!(
            r#"
    test 'does not confuse superscript and links with blank window shorthand' do
      para = block_from_string(%Q{http://localhost[Text^] on the 21^st^ and 22^nd^})
      assert_equal '<a href="http://localhost" target="_blank" rel="noopener">Text</a> on the 21<sup>st</sup> and 22<sup>nd</sup>', para.content
    end

"#
        );

        // TO DO: Enable when macro substitution is implemented.
        let mut content = crate::content::Content::from(crate::Span::new(
            "http://localhost[Text^] on the 21^st^ and 22^nd^",
        ));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        // ^^^ TO DO: This needs to be the full substitution group, not just the Quotes
        // substition.

        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(
                r#"<a href="http://localhost" target="_blank" rel="noopener">Text</a> on the 21<sup>st</sup> and 22<sup>nd</sup>"#
            )
        );
    }

    #[test]
    fn single_line_subscript_chars() {
        verifies!(
            r#"
    test 'single-line subscript chars' do
      para = block_from_string(%q{H~2~O})
      assert_equal 'H<sub>2</sub>O', para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new("H~2~O"));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed("H<sub>2</sub>O"));
    }

    #[test]
    fn escaped_single_line_subscript_chars() {
        verifies!(
            r#"
    test 'escaped single-line subscript chars' do
      para = block_from_string(%(H#{BACKSLASH}~2~O))
      assert_equal 'H~2~O', para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(r#"H\~2~O"#));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed("H~2~O"));
    }

    #[test]
    fn does_not_match_subscript_across_whitespace() {
        verifies!(
            r#"
    test 'does not match subscript across whitespace' do
      para = block_from_string(%Q{project~ view\non\nGitHub~})
      assert_equal para.source, para.sub_quotes(para.source)
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new("project~ view\non\nGitHub~"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed("project~ view\non\nGitHub~")
        );
    }

    #[test]
    fn does_not_match_adjacent_subscript_chars() {
        verifies!(
            r#"
    test 'does not match adjacent subscript chars' do
      para = block_from_string 'a ~~ b'
      assert_equal 'a ~~ b', para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new("a ~~ b"));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed("a ~~ b"));
    }

    #[test]
    fn does_not_match_subscript_across_distinct_urls() {
        verifies!(
            r#"
    test 'does not match subscript across distinct URLs' do
      para = block_from_string(%Q{http://www.abc.com/~def[DEF] and http://www.abc.com/~ghi[GHI]})
      assert_equal para.source, para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(
            "http://www.abc.com/~def[DEF] and http://www.abc.com/~ghi[GHI]",
        ));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed("http://www.abc.com/~def[DEF] and http://www.abc.com/~ghi[GHI]")
        );
    }

    #[test]
    fn quoted_text_with_role_shorthand() {
        verifies!(
            r#"
    test 'quoted text with role shorthand' do
      para = block_from_string(%q{[.white.red-background]#alert#})
      assert_equal '<span class="white red-background">alert</span>', para.sub_quotes(para.source)
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new("[.white.red-background]#alert#"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"<span class="white red-background">alert</span>"#)
        );
    }

    #[test]
    fn quoted_text_with_id_shorthand() {
        verifies!(
            r#"
    test 'quoted text with id shorthand' do
      para = block_from_string(%q{[#bond]#007#})
      assert_equal '<span id="bond">007</span>', para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new("[#bond]#007#"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"<span id="bond">007</span>"#)
        );
    }

    #[test]
    fn quoted_text_with_id_and_role_shorthand() {
        verifies!(
            r#"
    test 'quoted text with id and role shorthand' do
      para = block_from_string(%q{[#bond.white.red-background]#007#})
      assert_equal '<span id="bond" class="white red-background">007</span>', para.sub_quotes(para.source)
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new("[#bond.white.red-background]#007#"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"<span id="bond" class="white red-background">007</span>"#)
        );
    }

    #[test]
    fn quoted_text_with_id_and_role_shorthand_with_roles_before_id() {
        verifies!(
            r#"
    test 'quoted text with id and role shorthand with roles before id' do
      para = block_from_string(%q{[.white.red-background#bond]#007#})
      assert_equal '<span id="bond" class="white red-background">007</span>', para.sub_quotes(para.source)
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new("[.white.red-background#bond]#007#"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"<span id="bond" class="white red-background">007</span>"#)
        );
    }

    #[test]
    fn quoted_text_with_id_and_role_shorthand_with_roles_around_id() {
        verifies!(
            r#"
    test 'quoted text with id and role shorthand with roles around id' do
      para = block_from_string(%q{[.white#bond.red-background]#007#})
      assert_equal '<span id="bond" class="white red-background">007</span>', para.sub_quotes(para.source)
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new("[.white#bond.red-background]#007#"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"<span id="bond" class="white red-background">007</span>"#)
        );
    }

    // Not ported to this crate:
    // - 'quoted text with id and role shorthand using docbook backend': DocBook
    //   backend not supported.
    non_normative!(
        r#"
    test 'quoted text with id and role shorthand using docbook backend' do
      para = block_from_string '[#bond.white.red-background]#007#', backend: 'docbook'
      assert_equal '<anchor xml:id="bond"/><phrase role="white red-background">007</phrase>', para.sub_quotes(para.source)
    end

"#
    );

    #[test]
    fn should_not_assign_role_attribute_if_shorthand_style_has_no_roles() {
        verifies!(
            r#"
    test 'should not assign role attribute if shorthand style has no roles' do
      para = block_from_string '[#idname]*blah*'
      assert_equal '<strong id="idname">blah</strong>', para.content
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new("[#idname]*blah*"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"<strong id="idname">blah</strong>"#)
        );
    }

    #[test]
    fn should_remove_trailing_spaces_from_role_defined_using_shorthand() {
        verifies!(
            r#"
    test 'should remove trailing spaces from role defined using shorthand' do
      para = block_from_string '[.rolename ]*blah*'
      assert_equal '<strong class="rolename">blah</strong>', para.content
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new("[.rolename ]*blah*"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"<strong class="rolename">blah</strong>"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn should_ignore_attributes_after_comma() {
        verifies!(
            r#"
    test 'should ignore attributes after comma' do
      para = block_from_string(%q{[red, foobar]#alert#})
      assert_equal '<span class="red">alert</span>', para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new("[red, foobar]#alert#"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"<span class="red">alert</span>"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn should_remove_leading_and_trailing_spaces_around_role_after_ignoring_attributes_after_comma()
    {
        verifies!(
            r#"
    test 'should remove leading and trailing spaces around role after ignoring attributes after comma' do
      para = block_from_string(%q{[ red , foobar]#alert#})
      assert_equal '<span class="red">alert</span>', para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new("[ red , foobar]#alert#"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"<span class="red">alert</span>"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn should_not_assign_role_if_value_before_comma_is_empty() {
        verifies!(
            r#"
    test 'should not assign role if value before comma is empty' do
      para = block_from_string(%q{[,]#anonymous#})
      assert_equal 'anonymous', para.sub_quotes(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new("[,]#anonymous#"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed("anonymous".to_string().into_boxed_str())
        );
    }

    #[test]
    fn inline_passthrough_with_id_and_role_set_using_shorthand_1() {
        verifies!(
            r#"
    test 'inline passthrough with id and role set using shorthand' do
      %w(#idname.rolename .rolename#idname).each do |attrlist|
        para = block_from_string %([#{attrlist}]+pass+)
        assert_equal '<span id="idname" class="rolename">pass</span>', para.content
      end
    end
"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new("[#idname.rolename]+pass+"));

        let p = Parser::default();
        SubstitutionGroup::Normal.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(
                r#"<span id="idname" class="rolename">pass</span>"#
                    .to_string()
                    .into_boxed_str()
            )
        );
    }

    #[test]
    fn inline_passthrough_with_id_and_role_set_using_shorthand_2() {
        let mut content =
            crate::content::Content::from(crate::Span::new("[.rolename#idname]+pass+"));

        let p = Parser::default();
        SubstitutionGroup::Normal.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(
                r#"<span id="idname" class="rolename">pass</span>"#
                    .to_string()
                    .into_boxed_str()
            )
        );
    }

    #[test]
    fn does_not_recognize_attribute_list_with_left_square_bracket_on_formatted_text() {
        let mut content = crate::content::Content::from(crate::Span::new(
            r##"key: [ *before [.redacted]#redacted# after* ]"##,
        ));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(
                r#"key: [ <strong>before <span class="redacted">redacted</span> after</strong> ]"#
                    .to_string()
                    .into_boxed_str()
            )
        );
    }

    #[test]
    fn should_ignore_enclosing_square_brackets_when_processing_formatted_text_with_attribute() {
        let mut p = Parser::default();
        let maw =
            crate::blocks::Block::parse(crate::Span::new("nums = [1, 2, 3, [.blue]#4#]"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "nums = [1, 2, 3, [.blue]#4#]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"nums = [1, 2, 3, <span class="blue">4</span>]"#,
                },
                source: Span {
                    data: "nums = [1, 2, 3, [.blue]#4#]",
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
            },)
        );
    }

    #[test]
    fn should_allow_role_to_be_defined_using_attribute_reference() {
        let mut content = crate::content::Content::from(crate::Span::new("[{rolename}]#phrase#"));

        let p = Parser::default().with_intrinsic_attribute(
            "rolename",
            "red",
            ModificationContext::Anywhere,
        );

        SubstitutionGroup::Normal.apply(&mut content, &p, None);

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"<span class="red">phrase</span>"#.to_string().into_boxed_str())
        );
    }
}

non_normative!(
    r#"
  end

  context 'Macros' do
"#
);

mod macros {
    use crate::{content::SubstitutionStep, strings::CowStr, tests::prelude::*};

    const EMAIL_ADDRESSES: &[(&str, &str)] = &[
        (
            "doc.writer@asciidoc.org",
            r#"<a href="mailto:doc.writer@asciidoc.org">doc.writer@asciidoc.org</a>"#,
        ),
        (
            "author+website@4fs.no",
            r#"<a href="mailto:author+website@4fs.no">author+website@4fs.no</a>"#,
        ),
        (
            "john@domain.uk.co",
            r#"<a href="mailto:john@domain.uk.co">john@domain.uk.co</a>"#,
        ),
        (
            "name@somewhere.else.com",
            r#"<a href="mailto:name@somewhere.else.com">name@somewhere.else.com</a>"#,
        ),
        (
            "joe_bloggs@mail_server.com",
            r#"<a href="mailto:joe_bloggs@mail_server.com">joe_bloggs@mail_server.com</a>"#,
        ),
        (
            "joe-bloggs@mail-server.com",
            r#"<a href="mailto:joe-bloggs@mail-server.com">joe-bloggs@mail-server.com</a>"#,
        ),
        (
            "joe.bloggs@mail.server.com",
            r#"<a href="mailto:joe.bloggs@mail.server.com">joe.bloggs@mail.server.com</a>"#,
        ),
        (
            "FOO@BAR.COM",
            r#"<a href="mailto:FOO@BAR.COM">FOO@BAR.COM</a>"#,
        ),
        (
            "docs@writing.ninja",
            r#"<a href="mailto:docs@writing.ninja">docs@writing.ninja</a>"#,
        ),
    ];

    #[test]
    fn a_single_line_link_macro_should_be_interpreted_as_a_link() {
        verifies!(
            r#"
    test 'a single-line link macro should be interpreted as a link' do
      para = block_from_string('link:/home.html[]')
      assert_equal %q{<a href="/home.html" class="bare">/home.html</a>}, para.sub_macros(para.source)
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new("link:/home.html[]"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "link:/home.html[]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="/home.html" class="bare">/home.html</a>"#,
                },
                source: Span {
                    data: "link:/home.html[]",
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
            },)
        );
    }

    #[test]
    fn a_single_line_link_macro_with_text_should_be_interpreted_as_a_link() {
        verifies!(
            r#"
    test 'a single-line link macro with text should be interpreted as a link' do
      para = block_from_string('link:/home.html[Home]')
      assert_equal %q{<a href="/home.html">Home</a>}, para.sub_macros(para.source)
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new("link:/home.html[Home]"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "link:/home.html[Home]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="/home.html">Home</a>"#,
                },
                source: Span {
                    data: "link:/home.html[Home]",
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
            },)
        );
    }

    #[test]
    fn a_mailto_macro_should_be_interpreted_as_a_mailto_link() {
        verifies!(
            r#"
    test 'a mailto macro should be interpreted as a mailto link' do
      para = block_from_string('mailto:doc.writer@asciidoc.org[]')
      assert_equal %q{<a href="mailto:doc.writer@asciidoc.org">doc.writer@asciidoc.org</a>}, para.sub_macros(para.source)
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("mailto:doc.writer@asciidoc.org[]"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "mailto:doc.writer@asciidoc.org[]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="mailto:doc.writer@asciidoc.org">doc.writer@asciidoc.org</a>"#,
                },
                source: Span {
                    data: "mailto:doc.writer@asciidoc.org[]",
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
            },)
        );
    }

    #[test]
    fn a_mailto_macro_with_text_should_be_interpreted_as_a_mailto_link() {
        verifies!(
            r#"
    test 'a mailto macro with text should be interpreted as a mailto link' do
      para = block_from_string('mailto:doc.writer@asciidoc.org[Doc Writer]')
      assert_equal %q{<a href="mailto:doc.writer@asciidoc.org">Doc Writer</a>}, para.sub_macros(para.source)
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("mailto:doc.writer@asciidoc.org[Doc Writer]"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "mailto:doc.writer@asciidoc.org[Doc Writer]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="mailto:doc.writer@asciidoc.org">Doc Writer</a>"#,
                },
                source: Span {
                    data: "mailto:doc.writer@asciidoc.org[Doc Writer]",
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
            },)
        );
    }

    #[test]
    fn a_mailto_macro_with_text_and_subject_should_be_interpreted_as_a_mailto_link() {
        verifies!(
            r#"
    test 'a mailto macro with text and subject should be interpreted as a mailto link' do
      para = block_from_string 'mailto:doc.writer@asciidoc.org[Doc Writer, Pull request]'
      assert_equal '<a href="mailto:doc.writer@asciidoc.org?subject=Pull%20request">Doc Writer</a>', para.sub_macros(para.source)
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("mailto:doc.writer@asciidoc.org[Doc Writer, Pull request]"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "mailto:doc.writer@asciidoc.org[Doc Writer, Pull request]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="mailto:doc.writer@asciidoc.org?subject=Pull%20request">Doc Writer</a>"#,
                },
                source: Span {
                    data: "mailto:doc.writer@asciidoc.org[Doc Writer, Pull request]",
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
            },)
        );
    }

    #[test]
    fn a_mailto_macro_with_text_subject_and_body_should_be_interpreted_as_a_mailto_link() {
        verifies!(
            r#"
    test 'a mailto macro with text, subject and body should be interpreted as a mailto link' do
      para = block_from_string 'mailto:doc.writer@asciidoc.org[Doc Writer, Pull request, Please accept my pull request]'
      assert_equal '<a href="mailto:doc.writer@asciidoc.org?subject=Pull%20request&amp;body=Please%20accept%20my%20pull%20request">Doc Writer</a>', para.sub_macros(para.source)
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new(
                "mailto:doc.writer@asciidoc.org[Doc Writer, Pull request, Please accept my pull request]",
            ),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "mailto:doc.writer@asciidoc.org[Doc Writer, Pull request, Please accept my pull request]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="mailto:doc.writer@asciidoc.org?subject=Pull%20request&amp;body=Please%20accept%20my%20pull%20request">Doc Writer</a>"#,
                },
                source: Span {
                    data: "mailto:doc.writer@asciidoc.org[Doc Writer, Pull request, Please accept my pull request]",
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
            },)
        );
    }

    #[test]
    fn a_mailto_macro_with_subject_and_body_only_should_use_e_mail_as_text() {
        verifies!(
            r#"
    test 'a mailto macro with subject and body only should use e-mail as text' do
      para = block_from_string 'mailto:doc.writer@asciidoc.org[,Pull request,Please accept my pull request]'
      assert_equal '<a href="mailto:doc.writer@asciidoc.org?subject=Pull%20request&amp;body=Please%20accept%20my%20pull%20request">doc.writer@asciidoc.org</a>', para.sub_macros(para.source)
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new(
                "mailto:doc.writer@asciidoc.org[,Pull request,Please accept my pull request]",
            ),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "mailto:doc.writer@asciidoc.org[,Pull request,Please accept my pull request]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="mailto:doc.writer@asciidoc.org?subject=Pull%20request&amp;body=Please%20accept%20my%20pull%20request">doc.writer@asciidoc.org</a>"#,
                },
                source: Span {
                    data: "mailto:doc.writer@asciidoc.org[,Pull request,Please accept my pull request]",
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
            },)
        );
    }

    #[test]
    fn a_mailto_macro_supports_id_and_role_attributes() {
        verifies!(
            r#"
    test 'a mailto macro supports id and role attributes' do
      para = block_from_string('mailto:doc.writer@asciidoc.org[,id=contact,role=icon]')
      assert_equal %q{<a href="mailto:doc.writer@asciidoc.org" id="contact" class="icon">doc.writer@asciidoc.org</a>}, para.sub_macros(para.source)
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("mailto:doc.writer@asciidoc.org[,id=contact,role=icon]"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "mailto:doc.writer@asciidoc.org[,id=contact,role=icon]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="mailto:doc.writer@asciidoc.org" id="contact" class="icon">doc.writer@asciidoc.org</a>"#,
                },
                source: Span {
                    data: "mailto:doc.writer@asciidoc.org[,id=contact,role=icon]",
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
            },)
        );
    }

    #[test]
    fn should_recognize_inline_email_addresses() {
        verifies!(
            r#"
    test 'should recognize inline email addresses' do
      %w(
        doc.writer@asciidoc.org
        author+website@4fs.no
        john@domain.uk.co
        name@somewhere.else.com
        joe_bloggs@mail_server.com
        joe-bloggs@mail-server.com
        joe.bloggs@mail.server.com
        FOO@BAR.COM
        docs@writing.ninja
      ).each do |input|
        para = block_from_string input
        assert_equal %(<a href="mailto:#{input}">#{input}</a>), (para.sub_macros para.source)
      end
    end

"#
        );

        for (input, expected) in EMAIL_ADDRESSES {
            let mut p = Parser::default();
            let maw = crate::blocks::Block::parse(crate::Span::new(input), &mut p);

            let block = maw.item.unwrap().item;

            assert_eq!(
                block,
                Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: input,
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: expected,
                    },
                    source: Span {
                        data: input,
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
                },)
            );
        }
    }

    #[test]
    fn should_recognize_inline_email_address_containing_an_ampersand() {
        verifies!(
            r#"
    test 'should recognize inline email address containing an ampersand' do
      para = block_from_string('bert&ernie@sesamestreet.com')
      assert_equal %q{<a href="mailto:bert&amp;ernie@sesamestreet.com">bert&amp;ernie@sesamestreet.com</a>}, para.apply_subs(para.source)
    end

"#
        );

        let mut p = Parser::default();
        let maw =
            crate::blocks::Block::parse(crate::Span::new("bert&ernie@sesamestreet.com"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "bert&ernie@sesamestreet.com",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="mailto:bert&amp;ernie@sesamestreet.com">bert&amp;ernie@sesamestreet.com</a>"#,
                },
                source: Span {
                    data: "bert&ernie@sesamestreet.com",
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
            },)
        );
    }

    #[test]
    fn should_recognize_inline_email_address_surrounded_by_angle_brackets() {
        verifies!(
            r#"
    test 'should recognize inline email address surrounded by angle brackets' do
      para = block_from_string('<doc.writer@asciidoc.org>')
      assert_equal %q{&lt;<a href="mailto:doc.writer@asciidoc.org">doc.writer@asciidoc.org</a>&gt;}, para.apply_subs(para.source)
    end

"#
        );

        let mut p = Parser::default();
        let maw =
            crate::blocks::Block::parse(crate::Span::new("<doc.writer@asciidoc.org>"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "<doc.writer@asciidoc.org>",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"&lt;<a href="mailto:doc.writer@asciidoc.org">doc.writer@asciidoc.org</a>&gt;"#,
                },
                source: Span {
                    data: "<doc.writer@asciidoc.org>",
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
            },)
        );
    }

    #[test]
    fn should_ignore_escaped_inline_email_address() {
        verifies!(
            r#"
    test 'should ignore escaped inline email address' do
      para = block_from_string(%(#{BACKSLASH}doc.writer@asciidoc.org))
      assert_equal %q{doc.writer@asciidoc.org}, para.sub_macros(para.source)
    end

"#
        );

        let mut p = Parser::default();
        let maw =
            crate::blocks::Block::parse(crate::Span::new("\\doc.writer@asciidoc.org"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "\\doc.writer@asciidoc.org",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"doc.writer@asciidoc.org"#,
                },
                source: Span {
                    data: "\\doc.writer@asciidoc.org",
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
            },)
        );
    }

    #[test]
    fn a_single_line_raw_url_should_be_interpreted_as_a_link() {
        verifies!(
            r#"
    test 'a single-line raw url should be interpreted as a link' do
      para = block_from_string('http://google.com')
      assert_equal %q{<a href="http://google.com" class="bare">http://google.com</a>}, para.sub_macros(para.source)
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new("http://google.com"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "http://google.com",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="http://google.com" class="bare">http://google.com</a>"#,
                },
                source: Span {
                    data: "http://google.com",
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
            },)
        );
    }

    #[test]
    fn a_single_line_raw_url_with_text_should_be_interpreted_as_a_link() {
        verifies!(
            r#"
    test 'a single-line raw url with text should be interpreted as a link' do
      para = block_from_string('http://google.com[Google]')
      assert_equal %q{<a href="http://google.com">Google</a>}, para.sub_macros(para.source)
    end

"#
        );

        let mut p = Parser::default();
        let maw =
            crate::blocks::Block::parse(crate::Span::new("http://google.com[Google]"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "http://google.com[Google]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="http://google.com">Google</a>"#,
                },
                source: Span {
                    data: "http://google.com[Google]",
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
            },)
        );
    }

    #[test]
    fn a_multi_line_raw_url_with_text_should_be_interpreted_as_a_link() {
        verifies!(
            r#"
    test 'a multi-line raw url with text should be interpreted as a link' do
      para = block_from_string("http://google.com[Google\nHomepage]")
      assert_equal %{<a href="http://google.com">Google\nHomepage</a>}, para.sub_macros(para.source)
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("http://google.com[Google\nHomepage]"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "http://google.com[Google\nHomepage]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "<a href=\"http://google.com\">Google\nHomepage</a>",
                },
                source: Span {
                    data: "http://google.com[Google\nHomepage]",
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
            },)
        );
    }

    #[test]
    fn a_single_line_raw_url_with_attribute_as_text_should_be_interpreted_as_a_link_with_resolved_attribute()
     {
        verifies!(
            r#"
    test 'a single-line raw url with attribute as text should be interpreted as a link with resolved attribute' do
      para = block_from_string("http://google.com[{google_homepage}]")
      para.document.attributes['google_homepage'] = 'Google Homepage'
      assert_equal %q{<a href="http://google.com">Google Homepage</a>}, para.sub_macros(para.sub_attributes(para.source))
    end

"#
        );

        let mut p = Parser::default().with_intrinsic_attribute(
            "google_homepage",
            "Google Homepage",
            ModificationContext::Anywhere,
        );

        let maw = crate::blocks::Block::parse(
            crate::Span::new("http://google.com[{google_homepage}]"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "http://google.com[{google_homepage}]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="http://google.com">Google Homepage</a>"#,
                },
                source: Span {
                    data: "http://google.com[{google_homepage}]",
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
            },)
        );
    }

    #[test]
    fn should_not_resolve_an_escaped_attribute_in_link_text_1() {
        verifies!(
            r##"
    test 'should not resolve an escaped attribute in link text' do
      {
        'http://google.com' => "http://google.com[#{BACKSLASH}{google_homepage}]",
        'http://google.com?q=,' => "link:http://google.com?q=,[#{BACKSLASH}{google_homepage}]",
      }.each do |uri, macro|
        para = block_from_string macro
        para.document.attributes['google_homepage'] = 'Google Homepage'
        assert_equal %(<a href="#{uri}">{google_homepage}</a>), para.sub_macros(para.sub_attributes(para.source))
      end
    end

"##
        );

        let mut p = Parser::default().with_intrinsic_attribute(
            "google_homepage",
            "Google Homepage",
            ModificationContext::Anywhere,
        );

        let maw = crate::blocks::Block::parse(
            crate::Span::new("http://google.com[\\{google_homepage}]"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "http://google.com[\\{google_homepage}]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="http://google.com">{google_homepage}</a>"#,
                },
                source: Span {
                    data: "http://google.com[\\{google_homepage}]",
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
            },)
        );
    }

    #[test]
    fn should_not_resolve_an_escaped_attribute_in_link_text_2() {
        let mut p = Parser::default().with_intrinsic_attribute(
            "google_homepage",
            "Google Homepage",
            ModificationContext::Anywhere,
        );

        let maw = crate::blocks::Block::parse(
            crate::Span::new("http://google.com?q=,[\\{google_homepage}]"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "http://google.com?q=,[\\{google_homepage}]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="http://google.com?q=,">{google_homepage}</a>"#,
                },
                source: Span {
                    data: "http://google.com?q=,[\\{google_homepage}]",
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
            },)
        );
    }

    #[test]
    fn a_single_line_escaped_raw_url_should_not_be_interpreted_as_a_link() {
        verifies!(
            r#"
    test 'a single-line escaped raw url should not be interpreted as a link' do
      para = block_from_string(%(#{BACKSLASH}http://google.com))
      assert_equal %q{http://google.com}, para.sub_macros(para.source)
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new("\\http://google.com"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "\\http://google.com",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"http://google.com"#,
                },
                source: Span {
                    data: "\\http://google.com",
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
            },)
        );
    }

    #[test]
    fn a_comma_separated_list_of_links_should_not_include_commas_in_links() {
        verifies!(
            r#"
    test 'a comma separated list of links should not include commas in links' do
      para = block_from_string('http://foo.com, http://bar.com, http://example.org')
      assert_equal %q{<a href="http://foo.com" class="bare">http://foo.com</a>, <a href="http://bar.com" class="bare">http://bar.com</a>, <a href="http://example.org" class="bare">http://example.org</a>}, para.sub_macros(para.source)
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("http://foo.com, http://bar.com, http://example.org"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "http://foo.com, http://bar.com, http://example.org",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="http://foo.com" class="bare">http://foo.com</a>, <a href="http://bar.com" class="bare">http://bar.com</a>, <a href="http://example.org" class="bare">http://example.org</a>"#,
                },
                source: Span {
                    data: "http://foo.com, http://bar.com, http://example.org",
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
            },)
        );
    }

    #[test]
    fn a_single_line_image_macro_should_be_interpreted_as_an_image() {
        verifies!(
            r#"
    test 'a single-line image macro should be interpreted as an image' do
      para = block_from_string('image:tiger.png[]')
      assert_equal %{<span class="image"><img src="tiger.png" alt="tiger"></span>}, para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new("image:tiger.png[]"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "image:tiger.png[]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<span class="image"><img src="tiger.png" alt="tiger"></span>"#,
                },
                source: Span {
                    data: "image:tiger.png[]",
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
            },)
        );
    }

    #[test]
    fn should_replace_underscore_and_hyphen_with_space_in_generated_alt_text_for_an_inline_image() {
        verifies!(
            r#"
    test 'should replace underscore and hyphen with space in generated alt text for an inline image' do
      para = block_from_string('image:tiger-with-family_1.png[]')
      assert_equal %{<span class="image"><img src="tiger-with-family_1.png" alt="tiger with family 1"></span>}, para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("image:tiger-with-family_1.png[]"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "image:tiger-with-family_1.png[]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<span class="image"><img src="tiger-with-family_1.png" alt="tiger with family 1"></span>"#,
                },
                source: Span {
                    data: "image:tiger-with-family_1.png[]",
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
            },)
        );
    }

    #[test]
    fn a_single_line_image_macro_with_text_should_be_interpreted_as_an_image_with_alt_text() {
        verifies!(
            r#"
    test 'a single-line image macro with text should be interpreted as an image with alt text' do
      para = block_from_string('image:tiger.png[Tiger]')
      assert_equal %{<span class="image"><img src="tiger.png" alt="Tiger"></span>}, para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new("image:tiger.png[Tiger]"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "image:tiger.png[Tiger]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<span class="image"><img src="tiger.png" alt="Tiger"></span>"#,
                },
                source: Span {
                    data: "image:tiger.png[Tiger]",
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
            },)
        );
    }

    #[test]
    fn should_encode_special_characters_in_alt_text_of_inline_image() {
        verifies!(
            r##"
    test 'should encode special characters in alt text of inline image' do
      input = 'A tiger\'s "roar" is < a bear\'s "growl"'
      expected = 'A tiger&#8217;s &quot;roar&quot; is &lt; a bear&#8217;s &quot;growl&quot;'
      output = (convert_inline_string %(image:tiger-roar.png[#{input}])).gsub(/>\s+</, '><')
      assert_equal %(<span class="image"><img src="tiger-roar.png" alt="#{expected}"></span>), output
    end

"##
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new(r#"image:tiger-roar.png[A tiger's "roar" is < a bear's "growl"]"#),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"image:tiger-roar.png[A tiger's "roar" is < a bear's "growl"]"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<span class="image"><img src="tiger-roar.png" alt="A tiger&#8217;s &quot;roar&quot; is &lt; a bear&#8217;s &quot;growl&quot;"></span>"#,
                },
                source: Span {
                    data: r#"image:tiger-roar.png[A tiger's "roar" is < a bear's "growl"]"#,
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
            },)
        );
    }

    #[test]
    fn an_image_macro_with_svg_image_and_text_should_be_interpreted_as_an_image_with_alt_text() {
        verifies!(
            r#"
    test 'an image macro with SVG image and text should be interpreted as an image with alt text' do
      para = block_from_string('image:tiger.svg[Tiger]')
      assert_equal %{<span class="image"><img src="tiger.svg" alt="Tiger"></span>}, para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new("image:tiger.svg[Tiger]"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "image:tiger.svg[Tiger]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<span class="image"><img src="tiger.svg" alt="Tiger"></span>"#,
                },
                source: Span {
                    data: "image:tiger.svg[Tiger]",
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
            },)
        );
    }

    mod svg_image_options {
        //! The `inline` and `interactive` options for SVG images (issue #272).
        //!
        //! Ported from Asciidoctor's `substitutions_test.rb`. In this crate the
        //! SVG contents that Asciidoctor reads from disk are supplied instead
        //! by a [`SvgFileHandler`](crate::parser::SvgFileHandler)
        //! fixture, and the safe mode (which defaults to secure) is set
        //! explicitly.

        use crate::{
            SafeMode,
            tests::{fixtures::svg_file_handler::SvgFileHandlerFixture, prelude::*},
        };

        /// The raw contents that the SVG file handler returns for `circle.svg`.
        /// It carries `width`, `height`, and `style` attributes (which inline
        /// rendering must strip when an explicit width is supplied) plus an XML
        /// preamble (which must be dropped).
        const CIRCLE_SVG: &str = concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"500\" height=\"500\" ",
            "style=\"fill:red\" viewBox=\"0 0 500 500\">",
            "<circle cx=\"250\" cy=\"250\" r=\"200\"/></svg>",
        );

        /// The expected inline rendering of `circle.svg` at width 100: preamble
        /// removed, original width/height/style removed, and `width="100"`
        /// appended to the opening tag.
        const CIRCLE_SVG_INLINE: &str = concat!(
            r#"<span class="image">"#,
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 500 500" width="100">"#,
            r#"<circle cx="250" cy="250" r="200"/></svg></span>"#,
        );

        fn rendered(doc: &crate::Document<'_>) -> String {
            rendered_paragraphs(doc).join("\n")
        }

        #[test]
        fn an_interactive_svg_image_with_alt_text_should_be_converted_to_an_object_element() {
            verifies!(
                r#"
    test 'an image macro with an interactive SVG image and alt text should be converted to an object element' do
      para = block_from_string('image:tiger.svg[Tiger,opts=interactive]', safe: Asciidoctor::SafeMode::SERVER, attributes: { 'imagesdir' => 'images' })
      assert_equal %{<span class="image"><object type="image/svg+xml" data="images/tiger.svg"><span class="alt">Tiger</span></object></span>}, para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
            );

            let doc = Parser::default()
                .with_safe_mode(SafeMode::Server)
                .with_intrinsic_attribute("imagesdir", "images", ModificationContext::Anywhere)
                .parse("image:tiger.svg[Tiger,opts=interactive]");

            assert_eq!(
                rendered(&doc),
                r#"<span class="image"><object type="image/svg+xml" data="images/tiger.svg"><span class="alt">Tiger</span></object></span>"#
            );
        }

        #[test]
        fn an_interactive_svg_image_with_fallback_and_alt_text_should_be_converted_to_an_object_element()
         {
            verifies!(
                r#"
    test 'an image macro with an interactive SVG image, fallback and alt text should be converted to an object element' do
      para = block_from_string('image:tiger.svg[Tiger,fallback=tiger.png,opts=interactive]', safe: Asciidoctor::SafeMode::SERVER, attributes: { 'imagesdir' => 'images' })
      assert_equal %{<span class="image"><object type="image/svg+xml" data="images/tiger.svg"><img src="images/tiger.png" alt="Tiger"></object></span>}, para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
            );

            let doc = Parser::default()
                .with_safe_mode(SafeMode::Server)
                .with_intrinsic_attribute("imagesdir", "images", ModificationContext::Anywhere)
                .parse("image:tiger.svg[Tiger,fallback=tiger.png,opts=interactive]");

            assert_eq!(
                rendered(&doc),
                r#"<span class="image"><object type="image/svg+xml" data="images/tiger.svg"><img src="images/tiger.png" alt="Tiger"></object></span>"#
            );
        }

        #[test]
        fn an_inline_svg_image_should_be_converted_to_an_svg_element() {
            verifies!(
                r#"
    test 'an image macro with an inline SVG image should be converted to an svg element' do
      para = block_from_string('image:circle.svg[Tiger,100,opts=inline]', safe: Asciidoctor::SafeMode::SERVER, attributes: { 'imagesdir' => 'fixtures', 'docdir' => testdir })
      result = para.sub_macros(para.source).gsub(/>\s+</, '><')
      assert_match(/<svg\s[^>]*width="100"[^>]*>/, result)
      refute_match(/<svg\s[^>]*width="500"[^>]*>/, result)
      refute_match(/<svg\s[^>]*height="500"[^>]*>/, result)
      refute_match(/<svg\s[^>]*style="[^>]*>/, result)
    end

"#
            );

            let doc = Parser::default()
                .with_safe_mode(SafeMode::Server)
                .with_intrinsic_attribute("imagesdir", "fixtures", ModificationContext::Anywhere)
                .with_svg_file_handler(SvgFileHandlerFixture::from_pairs([(
                    "fixtures/circle.svg",
                    CIRCLE_SVG,
                )]))
                .parse("image:circle.svg[Tiger,100,opts=inline]");

            assert_eq!(rendered(&doc), CIRCLE_SVG_INLINE);
        }

        #[test]
        fn should_render_link_attribute_verbatim_when_value_is_self_and_image_target_is_inline_svg()
        {
            let doc = Parser::default()
                .with_safe_mode(SafeMode::Server)
                .with_intrinsic_attribute("imagesdir", "fixtures", ModificationContext::Anywhere)
                .with_svg_file_handler(SvgFileHandlerFixture::from_pairs([(
                    "fixtures/circle.svg",
                    CIRCLE_SVG,
                )]))
                .parse("image:circle.svg[Tiger,100,opts=inline,link=self]");

            // The `link` value is used verbatim: `self` is not resolved to the
            // image's own URI (which would be meaningless for an inline SVG), so
            // the anchor's `href` is the literal string `self`.
            assert_eq!(
                rendered(&doc),
                concat!(
                    r#"<span class="image"><a class="image" href="self">"#,
                    r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 500 500" width="100">"#,
                    r#"<circle cx="250" cy="250" r="200"/></svg></a></span>"#,
                )
            );
        }

        #[test]
        fn an_inline_svg_image_should_be_converted_to_an_svg_element_even_when_data_uri_is_set() {
            verifies!(
                r#"
    test 'an image macro with an inline SVG image should be converted to an svg element even when data-uri is set' do
      para = block_from_string('image:circle.svg[Tiger,100,opts=inline]', safe: Asciidoctor::SafeMode::SERVER, attributes: { 'data-uri' => '', 'imagesdir' => 'fixtures', 'docdir' => testdir })
      assert_match(/<svg\s[^>]*width="100">/, para.sub_macros(para.source).gsub(/>\s+</, '><'))
    end

"#
            );

            let doc = Parser::default()
                .with_safe_mode(SafeMode::Server)
                .with_intrinsic_attribute_bool("data-uri", true, ModificationContext::Anywhere)
                .with_intrinsic_attribute("imagesdir", "fixtures", ModificationContext::Anywhere)
                .with_svg_file_handler(SvgFileHandlerFixture::from_pairs([(
                    "fixtures/circle.svg",
                    CIRCLE_SVG,
                )]))
                .parse("image:circle.svg[Tiger,100,opts=inline]");

            assert_eq!(rendered(&doc), CIRCLE_SVG_INLINE);
        }

        #[test]
        fn an_svg_image_should_not_use_an_object_element_when_safe_mode_is_secure() {
            verifies!(
                r#"
    test 'an image macro with an SVG image should not use an object element when safe mode is secure' do
      para = block_from_string('image:tiger.svg[Tiger,opts=interactive]', attributes: { 'imagesdir' => 'images' })
      assert_equal %{<span class="image"><img src="images/tiger.svg" alt="Tiger"></span>}, para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
            );

            // `SafeMode::Secure` is the default; the `interactive` option is
            // ignored and a plain `<img>` is emitted.
            let doc = Parser::default()
                .with_intrinsic_attribute("imagesdir", "images", ModificationContext::Anywhere)
                .parse("image:tiger.svg[Tiger,opts=interactive]");

            assert_eq!(
                rendered(&doc),
                r#"<span class="image"><img src="images/tiger.svg" alt="Tiger"></span>"#
            );
        }

        #[test]
        fn an_inline_svg_image_falls_back_to_alt_text_when_no_handler_is_registered() {
            // With no `SvgFileHandler`, the SVG contents can't be read, so the
            // inline image degrades to its alt text.
            let doc = Parser::default()
                .with_safe_mode(SafeMode::Server)
                .with_intrinsic_attribute("imagesdir", "fixtures", ModificationContext::Anywhere)
                .parse("image:circle.svg[Tiger,100,opts=inline]");

            assert_eq!(
                rendered(&doc),
                r#"<span class="image"><span class="alt">Tiger</span></span>"#
            );
        }

        #[test]
        fn an_inline_svg_image_should_render_as_a_plain_img_when_safe_mode_is_secure() {
            // Like `interactive`, the `inline` option is disabled in `Secure`
            // mode (the default), so the SVG contents are never read and a plain
            // `<img>` is emitted. The registered handler is intentionally
            // ignored.
            let doc = Parser::default()
                .with_intrinsic_attribute("imagesdir", "fixtures", ModificationContext::Anywhere)
                .with_svg_file_handler(SvgFileHandlerFixture::from_pairs([(
                    "fixtures/circle.svg",
                    CIRCLE_SVG,
                )]))
                .parse("image:circle.svg[Tiger,100,opts=inline]");

            assert_eq!(
                rendered(&doc),
                r#"<span class="image"><img src="fixtures/circle.svg" alt="Tiger" width="100"></span>"#
            );
        }

        #[test]
        fn an_inline_svg_image_appends_both_width_and_height_to_the_opening_tag() {
            // When both a width and a height are supplied, both are appended to
            // the opening `<svg>` tag (after the original width/height/style are
            // stripped).
            let doc = Parser::default()
                .with_safe_mode(SafeMode::Server)
                .with_intrinsic_attribute("imagesdir", "fixtures", ModificationContext::Anywhere)
                .with_svg_file_handler(SvgFileHandlerFixture::from_pairs([(
                    "fixtures/circle.svg",
                    CIRCLE_SVG,
                )]))
                .parse("image:circle.svg[Tiger,100,200,opts=inline]");

            assert_eq!(
                rendered(&doc),
                concat!(
                    r#"<span class="image"><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 500 500" width="100" height="200">"#,
                    r#"<circle cx="250" cy="250" r="200"/></svg></span>"#,
                )
            );
        }

        #[test]
        fn an_inline_svg_image_without_dimensions_keeps_the_original_opening_tag() {
            // With neither a width nor a height supplied, the opening `<svg>` tag
            // is left untouched; only the XML preamble is stripped.
            let doc = Parser::default()
                .with_safe_mode(SafeMode::Server)
                .with_intrinsic_attribute("imagesdir", "fixtures", ModificationContext::Anywhere)
                .with_svg_file_handler(SvgFileHandlerFixture::from_pairs([(
                    "fixtures/circle.svg",
                    CIRCLE_SVG,
                )]))
                .parse("image:circle.svg[Tiger,opts=inline]");

            assert_eq!(
                rendered(&doc),
                concat!(
                    r#"<span class="image"><svg xmlns="http://www.w3.org/2000/svg" width="500" height="500" style="fill:red" viewBox="0 0 500 500">"#,
                    r#"<circle cx="250" cy="250" r="200"/></svg></span>"#,
                )
            );
        }

        #[test]
        fn special_characters_in_alt_text_are_encoded_in_the_span_fallback() {
            // The alt text used in a `<span class="alt">` fallback is already
            // HTML-encoded by the time the image macro is substituted: the
            // special-characters step (which runs before macros) has turned
            // `<`, `>`, and `&` into entities. A literal double quote is left
            // as-is in element-body content (it is only encoded in attribute
            // context, such as an `alt="…"` attribute), matching Ruby
            // Asciidoctor. This holds for both the interactive `<object>`
            // fallback and the inline no-handler fallback.
            let interactive = Parser::default()
                .with_safe_mode(SafeMode::Server)
                .with_intrinsic_attribute("imagesdir", "images", ModificationContext::Anywhere)
                .parse(r#"image:tiger.svg[A <b> & "c",opts=interactive]"#);

            assert_eq!(
                rendered(&interactive),
                concat!(
                    r#"<span class="image"><object type="image/svg+xml" data="images/tiger.svg">"#,
                    r#"<span class="alt">A &lt;b&gt; &amp; "c"</span></object></span>"#,
                )
            );

            let inline = Parser::default()
                .with_safe_mode(SafeMode::Server)
                .with_intrinsic_attribute("imagesdir", "images", ModificationContext::Anywhere)
                .parse(r#"image:missing.svg[A <b> & "c",opts=inline]"#);

            assert_eq!(
                rendered(&inline),
                r#"<span class="image"><span class="alt">A &lt;b&gt; &amp; "c"</span></span>"#
            );
        }
    }

    #[test]
    fn a_single_line_image_macro_with_text_containing_escaped_square_bracket_should_be_interpreted_as_an_image_with_alt_text()
     {
        verifies!(
            r#"
    test 'a single-line image macro with text containing escaped square bracket should be interpreted as an image with alt text' do
      para = block_from_string(%(image:tiger.png[[Another#{BACKSLASH}] Tiger]))
      assert_equal %{<span class="image"><img src="tiger.png" alt="[Another] Tiger"></span>}, para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new(r#"image:tiger.png[[Another\] Tiger]"#),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"image:tiger.png[[Another\] Tiger]"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<span class="image"><img src="tiger.png" alt="[Another] Tiger"></span>"#,
                },
                source: Span {
                    data: r#"image:tiger.png[[Another\] Tiger]"#,
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
            },)
        );
    }

    #[test]
    fn an_escaped_image_macro_should_not_be_interpreted_as_an_image() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new(r#"\image:tiger.png[]"#), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"\image:tiger.png[]"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"image:tiger.png[]"#,
                },
                source: Span {
                    data: r#"\image:tiger.png[]"#,
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
            },)
        );
    }

    #[test]
    fn a_single_line_image_macro_with_text_and_dimensions_should_be_interpreted_as_an_image_with_alt_text_and_dimensions()
     {
        verifies!(
            r#"
    test 'a single-line image macro with text and dimensions should be interpreted as an image with alt text and dimensions' do
      para = block_from_string('image:tiger.png[Tiger, 200, 100]')
      assert_equal %{<span class="image"><img src="tiger.png" alt="Tiger" width="200" height="100"></span>},
          para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new(r#"image:tiger.png[Tiger, 200, 100]"#),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"image:tiger.png[Tiger, 200, 100]"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<span class="image"><img src="tiger.png" alt="Tiger" width="200" height="100"></span>"#,
                },
                source: Span {
                    data: r#"image:tiger.png[Tiger, 200, 100]"#,
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
            },)
        );
    }

    // Not ported to this crate:
    // - 'a single-line image macro with text and dimensions should be interpreted
    //   as an image with alt text and dimensions in docbook': DocBook backend not
    //   supported (grouped Rust stub covers all four DocBook image tests).
    // - 'a single-line image macro with scaledwidth attribute should be supported
    //   in docbook': DocBook backend not supported.
    // - 'a single-line image macro with scaled attribute should be supported in
    //   docbook': DocBook backend not supported.
    // - 'should pass through role on image macro to DocBook output': DocBook
    //   backend not supported.
    non_normative!(
        r#"
    test 'a single-line image macro with text and dimensions should be interpreted as an image with alt text and dimensions in docbook' do
      para = block_from_string 'image:tiger.png[Tiger, 200, 100]', backend: 'docbook'
      assert_equal %{<inlinemediaobject><imageobject><imagedata fileref="tiger.png" contentwidth="200" contentdepth="100"/></imageobject><textobject><phrase>Tiger</phrase></textobject></inlinemediaobject>},
          para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

    test 'a single-line image macro with scaledwidth attribute should be supported in docbook' do
      para = block_from_string 'image:tiger.png[Tiger,scaledwidth=25%]', backend: 'docbook'
      assert_equal '<inlinemediaobject><imageobject><imagedata fileref="tiger.png" width="25%"/></imageobject><textobject><phrase>Tiger</phrase></textobject></inlinemediaobject>',
        para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

    test 'a single-line image macro with scaled attribute should be supported in docbook' do
      para = block_from_string 'image:tiger.png[Tiger,scale=200]', backend: 'docbook'
      assert_equal '<inlinemediaobject><imageobject><imagedata fileref="tiger.png" scale="200"/></imageobject><textobject><phrase>Tiger</phrase></textobject></inlinemediaobject>',
        para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

    test 'should pass through role on image macro to DocBook output' do
      para = block_from_string 'image:tiger.png[Tiger,200,role=animal]', backend: 'docbook'
      result = para.sub_macros(para.source)
      assert_includes result, '<inlinemediaobject role="animal">'
    end

"#
    );

    #[ignore]
    #[test]
    fn docbook_support_for_image() {
        // The following tests from Ruby have not been ported because Docbook is
        // not in scope for this crate.
        //
        // * test 'a single-line image macro with text and dimensions should be
        //   interpreted as an image with alt text and dimensions in docbook'
        // * test 'a single-line image macro with scaledwidth attribute should
        //   be supported in docbook'
        // * test 'a single-line image macro with scaled attribute should be
        //   supported in docbook'
        // * test 'should pass through role on image macro to DocBook output'
    }

    #[test]
    fn a_single_line_image_macro_with_text_and_link_should_be_interpreted_as_a_linked_image_with_alt_text()
     {
        verifies!(
            r#"
    test 'a single-line image macro with text and link should be interpreted as a linked image with alt text' do
      para = block_from_string('image:tiger.png[Tiger, link="http://en.wikipedia.org/wiki/Tiger"]')
      assert_equal %{<span class="image"><a class="image" href="http://en.wikipedia.org/wiki/Tiger"><img src="tiger.png" alt="Tiger"></a></span>},
          para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new(
                r#"image:tiger.png[Tiger, link="http://en.wikipedia.org/wiki/Tiger"]"#,
            ),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"image:tiger.png[Tiger, link="http://en.wikipedia.org/wiki/Tiger"]"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<span class="image"><a class="image" href="http://en.wikipedia.org/wiki/Tiger"><img src="tiger.png" alt="Tiger"></a></span>"#,
                },
                source: Span {
                    data: r#"image:tiger.png[Tiger, link="http://en.wikipedia.org/wiki/Tiger"]"#,
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
            },)
        );
    }

    #[test]
    fn a_single_line_image_macro_with_text_and_link_to_self_should_be_interpreted_as_a_self_referencing_image_with_alt_text()
     {
        verifies!(
            r#"
    test 'a single-line image macro with text and link to self should be interpreted as a self-referencing image with alt text' do
      para = block_from_string 'image:tiger.png[Tiger, link=self]', attributes: { 'imagesdir' => 'img' }
      assert_equal '<span class="image"><a class="image" href="img/tiger.png"><img src="img/tiger.png" alt="Tiger"></a></span>',
        para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
        );

        let mut p = Parser::default().with_intrinsic_attribute(
            "imagesdir",
            "img",
            ModificationContext::Anywhere,
        );

        let maw = crate::blocks::Block::parse(
            crate::Span::new(r#"image:tiger.png[Tiger, link=self]"#),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        // `link=self` resolves to the image's own resolved `src`
        // (`img/tiger.png`), not the literal string `self`.
        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"image:tiger.png[Tiger, link=self]"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<span class="image"><a class="image" href="img/tiger.png"><img src="img/tiger.png" alt="Tiger"></a></span>"#,
                },
                source: Span {
                    data: r#"image:tiger.png[Tiger, link=self]"#,
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
            },)
        );
    }

    #[test]
    fn should_link_to_data_uri_if_value_of_link_attribute_is_self_and_inline_image_is_embedded() {
        verifies!(
            r###"
        test 'should link to data URI if value of link attribute is self and inline image is embedded' do
            para = block_from_string 'image:circle.svg[Tiger,100,link=self]', safe: Asciidoctor::SafeMode::SERVER, attributes: { 'data-uri' => '', 'imagesdir' => 'fixtures', 'docdir' => testdir }
            output = para.sub_macros(para.source).gsub(/>\s+</, '><')
            assert_xpath '//a[starts-with(@href,"data:image/svg+xml;base64,")]', output, 1
            assert_xpath '//img[starts-with(@src,"data:image/svg+xml;base64,")]', output, 1
        end
            "###
        );

        use crate::tests::fixtures::image_file_handler::ImageFileHandlerFixture;

        // The bytes the image file handler returns for `circle.svg`. This crate
        // reads no files itself, so the handler supplies what Asciidoctor would
        // read from disk; `data-uri` embedding base64-encodes exactly these
        // bytes.
        const CIRCLE_SVG: &str = concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"500\" height=\"500\" ",
            "style=\"fill:red\" viewBox=\"0 0 500 500\">",
            "<circle cx=\"250\" cy=\"250\" r=\"200\"/></svg>",
        );

        // Base64 (strict, `pack 'm0'`) of `CIRCLE_SVG`, the `data:` URI payload.
        const CIRCLE_SVG_BASE64: &str = concat!(
            "PD94bWwgdmVyc2lvbj0iMS4wIiBlbmNvZGluZz0iVVRGLTgiPz4KPHN2ZyB4bWxucz0iaHR0",
            "cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSI1MDAiIGhlaWdodD0iNTAwIiBzdHls",
            "ZT0iZmlsbDpyZWQiIHZpZXdCb3g9IjAgMCA1MDAgNTAwIj48Y2lyY2xlIGN4PSIyNTAiIGN5",
            "PSIyNTAiIHI9IjIwMCIvPjwvc3ZnPg==",
        );

        let mut content = crate::content::Content::from(crate::Span::new(
            "image:circle.svg[Tiger,100,link=self]",
        ));

        // Below `Secure`, with `data-uri` set, the (non-inline) SVG image is
        // embedded as a data URI, and `link=self` resolves to that same data
        // URI for the anchor's `href`.
        let p = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_intrinsic_attribute_bool("data-uri", true, ModificationContext::Anywhere)
            .with_intrinsic_attribute("imagesdir", "fixtures", ModificationContext::Anywhere)
            .with_image_file_handler(ImageFileHandlerFixture::from_pairs([(
                "fixtures/circle.svg",
                CIRCLE_SVG.as_bytes(),
            )]));

        SubstitutionStep::Macros.apply(&mut content, &p, None);

        let expected = format!(
            concat!(
                r#"<span class="image"><a class="image" href="data:image/svg+xml;base64,{b64}">"#,
                r#"<img src="data:image/svg+xml;base64,{b64}" alt="Tiger" width="100"></a></span>"#,
            ),
            b64 = CIRCLE_SVG_BASE64,
        );

        assert_eq!(content.rendered, CowStr::Boxed(expected.into_boxed_str()));
    }

    // Not ported to this crate:
    // - 'an inline image macro with link should be interpreted as a linked image in
    //   docbook': DocBook backend not supported.
    non_normative!(
        r#"
    test 'an inline image macro with link should be interpreted as a linked image in docbook' do
      para = block_from_string 'image:apache license 2_0.png[Apache License 2.0,link=http://www.apache.org/licenses/LICENSE-2.0]', backend: 'docbook'
      assert_equal '<link xl:href="http://www.apache.org/licenses/LICENSE-2.0"><inlinemediaobject><imageobject><imagedata fileref="apache%20license%202_0.png"/></imageobject><textobject><phrase>Apache License 2.0</phrase></textobject></inlinemediaobject></link>',
        para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
    );

    #[test]
    fn rel_noopener_should_be_added_to_an_image_with_a_link_that_targets_the_blank_window() {
        verifies!(
            r#"
    test 'rel=noopener should be added to an image with a link that targets the _blank window' do
      para = block_from_string 'image:tiger.png[Tiger,link=http://en.wikipedia.org/wiki/Tiger,window=_blank]'
      assert_equal %{<span class="image"><a class="image" href="http://en.wikipedia.org/wiki/Tiger" target="_blank" rel="noopener"><img src="tiger.png" alt="Tiger"></a></span>},
          para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
        );

        let mut p = Parser::default();

        let maw = crate::blocks::Block::parse(
            crate::Span::new(
                r#"image:tiger.png[Tiger,link=http://en.wikipedia.org/wiki/Tiger,window=_blank]"#,
            ),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"image:tiger.png[Tiger,link=http://en.wikipedia.org/wiki/Tiger,window=_blank]"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<span class="image"><a class="image" href="http://en.wikipedia.org/wiki/Tiger" target="_blank" rel="noopener"><img src="tiger.png" alt="Tiger"></a></span>"#,
                },
                source: Span {
                    data: r#"image:tiger.png[Tiger,link=http://en.wikipedia.org/wiki/Tiger,window=_blank]"#,
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
            },)
        );
    }

    #[test]
    fn rel_noopener_should_be_added_to_an_image_with_a_link_that_targets_a_named_window_when_the_noopener_option_is_set()
     {
        verifies!(
            r#"
    test 'rel=noopener should be added to an image with a link that targets a named window when the noopener option is set' do
      para = block_from_string 'image:tiger.png[Tiger,link=http://en.wikipedia.org/wiki/Tiger,window=name,opts=noopener]'
      assert_equal %{<span class="image"><a class="image" href="http://en.wikipedia.org/wiki/Tiger" target="name" rel="noopener"><img src="tiger.png" alt="Tiger"></a></span>},
          para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
        );

        let mut p = Parser::default();

        let maw = crate::blocks::Block::parse(
            crate::Span::new(
                r#"image:tiger.png[Tiger,link=http://en.wikipedia.org/wiki/Tiger,window=name,opts=noopener]"#,
            ),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"image:tiger.png[Tiger,link=http://en.wikipedia.org/wiki/Tiger,window=name,opts=noopener]"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<span class="image"><a class="image" href="http://en.wikipedia.org/wiki/Tiger" target="name" rel="noopener"><img src="tiger.png" alt="Tiger"></a></span>"#,
                },
                source: Span {
                    data: r#"image:tiger.png[Tiger,link=http://en.wikipedia.org/wiki/Tiger,window=name,opts=noopener]"#,
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
            },)
        );
    }

    #[test]
    fn rel_nofollow_should_be_added_to_an_image_with_a_link_when_the_nofollow_option_is_set() {
        verifies!(
            r#"
    test 'rel=nofollow should be added to an image with a link when the nofollow option is set' do
      para = block_from_string 'image:tiger.png[Tiger,link=http://en.wikipedia.org/wiki/Tiger,opts=nofollow]'
      assert_equal %{<span class="image"><a class="image" href="http://en.wikipedia.org/wiki/Tiger" rel="nofollow"><img src="tiger.png" alt="Tiger"></a></span>},
          para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
        );

        let mut p = Parser::default();

        let maw = crate::blocks::Block::parse(
            crate::Span::new(
                r#"image:tiger.png[Tiger,link=http://en.wikipedia.org/wiki/Tiger,opts=nofollow]"#,
            ),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"image:tiger.png[Tiger,link=http://en.wikipedia.org/wiki/Tiger,opts=nofollow]"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<span class="image"><a class="image" href="http://en.wikipedia.org/wiki/Tiger" rel="nofollow"><img src="tiger.png" alt="Tiger"></a></span>"#,
                },
                source: Span {
                    data: r#"image:tiger.png[Tiger,link=http://en.wikipedia.org/wiki/Tiger,opts=nofollow]"#,
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
            },)
        );
    }

    #[test]
    fn a_multi_line_image_macro_with_text_and_dimensions_should_be_interpreted_as_an_image_with_alt_text_and_dimensions()
     {
        verifies!(
            r#"
    test 'a multi-line image macro with text and dimensions should be interpreted as an image with alt text and dimensions' do
      para = block_from_string(%(image:tiger.png[Another\nAwesome\nTiger, 200,\n100]))
      assert_equal %{<span class="image"><img src="tiger.png" alt="Another Awesome Tiger" width="200" height="100"></span>},
          para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
        );

        let mut p = Parser::default();

        let maw = crate::blocks::Block::parse(
            crate::Span::new("image:tiger.png[Another\nAwesome\nTiger, 200,\n100]"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "image:tiger.png[Another\nAwesome\nTiger, 200,\n100]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<span class="image"><img src="tiger.png" alt="Another Awesome Tiger" width="200" height="100"></span>"#,
                },
                source: Span {
                    data: "image:tiger.png[Another\nAwesome\nTiger, 200,\n100]",
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
            },)
        );
    }

    #[test]
    fn an_inline_image_macro_with_a_url_target_should_be_interpreted_as_an_image() {
        verifies!(
            r#"
    test 'an inline image macro with a url target should be interpreted as an image' do
      para = block_from_string %(Beware of the image:http://example.com/images/tiger.png[tiger].)
      assert_equal %{Beware of the <span class="image"><img src="http://example.com/images/tiger.png" alt="tiger"></span>.},
          para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
        );

        let mut p = Parser::default();

        let maw = crate::blocks::Block::parse(
            crate::Span::new(r#"Beware of the image:http://example.com/images/tiger.png[tiger]."#),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"Beware of the image:http://example.com/images/tiger.png[tiger]."#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"Beware of the <span class="image"><img src="http://example.com/images/tiger.png" alt="tiger"></span>."#,
                },
                source: Span {
                    data: r#"Beware of the image:http://example.com/images/tiger.png[tiger]."#,
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
            },)
        );
    }

    #[test]
    fn an_inline_image_macro_with_a_float_attribute_should_be_interpreted_as_a_floating_image() {
        verifies!(
            r#"
    test 'an inline image macro with a float attribute should be interpreted as a floating image' do
      para = block_from_string %(image:http://example.com/images/tiger.png[tiger, float="right"] Beware of the tigers!)
      assert_equal %{<span class="image right"><img src="http://example.com/images/tiger.png" alt="tiger"></span> Beware of the tigers!},
          para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
        );

        let mut p = Parser::default();

        let maw = crate::blocks::Block::parse(
            crate::Span::new(
                r#"image:http://example.com/images/tiger.png[tiger, float="right"] Beware of the tigers!"#,
            ),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"image:http://example.com/images/tiger.png[tiger, float="right"] Beware of the tigers!"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<span class="image right"><img src="http://example.com/images/tiger.png" alt="tiger"></span> Beware of the tigers!"#,
                },
                source: Span {
                    data: r#"image:http://example.com/images/tiger.png[tiger, float="right"] Beware of the tigers!"#,
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
            },)
        );
    }

    #[test]
    fn should_prepend_value_of_imagesdir_attribute_to_inline_image_target_if_target_is_relative_path()
     {
        verifies!(
            r#"
    test 'should prepend value of imagesdir attribute to inline image target if target is relative path' do
      para = block_from_string %(Beware of the image:tiger.png[tiger].), attributes: { 'imagesdir' => './images' }
      assert_equal %{Beware of the <span class="image"><img src="./images/tiger.png" alt="tiger"></span>.},
          para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
        );

        let mut p = Parser::default().with_intrinsic_attribute(
            "imagesdir",
            "./images",
            ModificationContext::Anywhere,
        );

        let maw = crate::blocks::Block::parse(
            crate::Span::new(r#"Beware of the image:tiger.png[tiger]."#),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"Beware of the image:tiger.png[tiger]."#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"Beware of the <span class="image"><img src="./images/tiger.png" alt="tiger"></span>."#,
                },
                source: Span {
                    data: r#"Beware of the image:tiger.png[tiger]."#,
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
            },)
        );
    }

    #[test]
    fn should_not_prepend_value_of_imagesdir_attribute_to_inline_image_target_if_target_is_absolute_path()
     {
        verifies!(
            r#"
    test 'should not prepend value of imagesdir attribute to inline image target if target is absolute path' do
      para = block_from_string %(Beware of the image:/tiger.png[tiger].), attributes: { 'imagesdir' => './images' }
      assert_equal %{Beware of the <span class="image"><img src="/tiger.png" alt="tiger"></span>.},
          para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
        );

        let mut p = Parser::default().with_intrinsic_attribute(
            "imagesdir",
            "./images",
            ModificationContext::Anywhere,
        );

        let maw = crate::blocks::Block::parse(
            crate::Span::new(r#"Beware of the image:/tiger.png[tiger]."#),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"Beware of the image:/tiger.png[tiger]."#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"Beware of the <span class="image"><img src="/tiger.png" alt="tiger"></span>."#,
                },
                source: Span {
                    data: r#"Beware of the image:/tiger.png[tiger]."#,
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
            },)
        );
    }

    #[test]
    fn should_not_prepend_value_of_imagesdir_attribute_to_inline_image_target_if_target_is_url() {
        verifies!(
            r#"
    test 'should not prepend value of imagesdir attribute to inline image target if target is url' do
      para = block_from_string %(Beware of the image:http://example.com/images/tiger.png[tiger].), attributes: { 'imagesdir' => './images' }
      assert_equal %{Beware of the <span class="image"><img src="http://example.com/images/tiger.png" alt="tiger"></span>.},
          para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
        );

        let mut p = Parser::default().with_intrinsic_attribute(
            "imagesdir",
            "./images",
            ModificationContext::Anywhere,
        );

        let maw = crate::blocks::Block::parse(
            crate::Span::new(r#"Beware of the image:http://example.com/images/tiger.png[tiger]."#),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"Beware of the image:http://example.com/images/tiger.png[tiger]."#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"Beware of the <span class="image"><img src="http://example.com/images/tiger.png" alt="tiger"></span>."#,
                },
                source: Span {
                    data: r#"Beware of the image:http://example.com/images/tiger.png[tiger]."#,
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
            },)
        );
    }

    #[test]
    fn should_match_an_inline_image_macro_if_target_contains_a_space_character() {
        verifies!(
            r#"
    test 'should match an inline image macro if target contains a space character' do
      para = block_from_string(%(Beware of the image:big cats.png[] around here.))
      assert_equal %(Beware of the <span class="image"><img src="big%20cats.png" alt="big cats"></span> around here.),
          para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
        );

        let mut p = Parser::default();

        let maw = crate::blocks::Block::parse(
            crate::Span::new(r#"Beware of the image:big cats.png[] around here."#),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"Beware of the image:big cats.png[] around here."#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"Beware of the <span class="image"><img src="big%20cats.png" alt="big cats"></span> around here."#,
                },
                source: Span {
                    data: r#"Beware of the image:big cats.png[] around here."#,
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
            },)
        );
    }

    #[test]
    fn should_not_match_an_inline_image_macro_if_target_contains_a_newline_character() {
        verifies!(
            r#"
    test 'should not match an inline image macro if target contains a newline character' do
      para = block_from_string(%(Fear not. There are no image:big\ncats.png[] around here.))
      result = para.sub_macros(para.source)
      refute_includes result, '<img '
      assert_includes result, %(image:big\ncats.png[])
    end

"#
        );

        let mut p = Parser::default();

        let maw = crate::blocks::Block::parse(
            crate::Span::new("Fear not. There are no image:big\ncats.png[] around here."),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "Fear not. There are no image:big\ncats.png[] around here.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "Fear not. There are no image:big\ncats.png[] around here.",
                },
                source: Span {
                    data: "Fear not. There are no image:big\ncats.png[] around here.",
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
            },)
        );
    }

    #[test]
    fn should_not_match_an_inline_image_macro_if_target_begins_with_space_character() {
        verifies!(
            r#"
    test 'should not match an inline image macro if target begins or ends with space character' do
      ['image: big cats.png[]', 'image:big cats.png []'].each do |input|
        para = block_from_string %(Fear not. There are no #{input} around here.)
        result = para.sub_macros(para.source)
        refute_includes result, '<img '
        assert_includes result, input
      end
    end

"#
        );

        let mut p = Parser::default();

        let maw = crate::blocks::Block::parse(
            crate::Span::new("Fear not. There are no image: big cats.png[] around here."),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "Fear not. There are no image: big cats.png[] around here.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "Fear not. There are no image: big cats.png[] around here.",
                },
                source: Span {
                    data: "Fear not. There are no image: big cats.png[] around here.",
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
            },)
        );
    }

    #[test]
    fn should_not_match_an_inline_image_macro_if_target_ends_with_space_character() {
        let mut p = Parser::default();

        let maw = crate::blocks::Block::parse(
            crate::Span::new("Fear not. There are no image:big cats.png [] around here."),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "Fear not. There are no image:big cats.png [] around here.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "Fear not. There are no image:big cats.png [] around here.",
                },
                source: Span {
                    data: "Fear not. There are no image:big cats.png [] around here.",
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
            },)
        );
    }

    #[test]
    fn should_not_detect_a_block_image_macro_found_inline() {
        verifies!(
            r#"
    test 'should not detect a block image macro found inline' do
      para = block_from_string(%(Not an inline image macro image::tiger.png[].))
      result = para.sub_macros(para.source)
      refute_includes result, '<img '
      assert_includes result, 'image::tiger.png[]'
    end

"#
        );

        let mut p = Parser::default();

        let maw = crate::blocks::Block::parse(
            crate::Span::new("Not an inline image macro image::tiger.png[]."),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "Not an inline image macro image::tiger.png[].",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "Not an inline image macro image::tiger.png[].",
                },
                source: Span {
                    data: "Not an inline image macro image::tiger.png[].",
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
            },)
        );
    }

    non_normative!(
        r#"
    # NOTE this test verifies attributes get substituted eagerly in target of image in title
"#
    );

    #[test]
    fn should_substitute_attributes_in_target_of_inline_image_in_section_title() {
        verifies!(
            r#"
    test 'should substitute attributes in target of inline image in section title' do
      input = '== image:{iconsdir}/dot.gif[dot] Title'

      using_memory_logger do |logger|
        sect = block_from_string input, attributes: { 'data-uri' => '', 'iconsdir' => 'fixtures', 'docdir' => testdir }, safe: :server, catalog_assets: true
        assert 1, sect.document.catalog[:images].size
        assert_equal 'fixtures/dot.gif', sect.document.catalog[:images][0].to_s
        assert_nil sect.document.catalog[:images][0].imagesdir
        assert logger.empty?
      end
    end

"#
        );

        // With `catalog_assets` enabled, the `image:` macro in the section title
        // records its target in the document catalog. The `{iconsdir}` reference
        // in the target is resolved (to `fixtures`) by the attribute-references
        // step, which runs before the macros step, so the catalog records the
        // resolved `fixtures/dot.gif`. `imagesdir` is unset here, so the
        // reference carries no `imagesdir`.
        let doc = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_intrinsic_attribute_bool("data-uri", true, ModificationContext::Anywhere)
            .with_intrinsic_attribute("iconsdir", "fixtures", ModificationContext::Anywhere)
            .with_catalog_assets(true)
            .parse("== image:{iconsdir}/dot.gif[dot] Title");

        let images = doc.catalog().images();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].to_string(), "fixtures/dot.gif");
        assert_eq!(images[0].imagesdir, None);

        // `data-uri` is set but no image file handler is registered, so the
        // image degrades silently to a web path – no warning is emitted
        // (mirroring `assert logger.empty?`).
        assert_eq!(doc.warnings().count(), 0);
    }

    #[test]
    fn an_icon_macro_should_be_interpreted_as_an_icon_if_icons_are_enabled() {
        verifies!(
            r#"
    test 'an icon macro should be interpreted as an icon if icons are enabled' do
      para = block_from_string 'icon:github[]', attributes: { 'icons' => '' }
      assert_equal %{<span class="icon"><img src="./images/icons/github.png" alt="github"></span>}, para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new("icon:github[]"));

        let expected =
            r#"<span class="icon"><img src="./images/icons/github.png" alt="github"></span>"#;

        let p = Parser::default().with_intrinsic_attribute_bool(
            "icons",
            true,
            ModificationContext::ApiOnly,
        );

        SubstitutionStep::Macros.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(expected.to_string().into_boxed_str())
        );
    }

    #[test]
    fn an_icon_macro_should_be_interpreted_as_alt_text_if_icons_are_disabled() {
        verifies!(
            r#"
    test 'an icon macro should be interpreted as alt text if icons are disabled' do
      para = block_from_string 'icon:github[]'
      assert_equal %{<span class="icon">[github&#93;</span>}, para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new("icon:github[]"));

        let expected = r#"<span class="icon">[github&#93;</span>"#;

        let p = Parser::default();

        SubstitutionStep::Macros.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(expected.to_string().into_boxed_str())
        );
    }

    #[test]
    fn should_not_mangle_icon_with_link_if_icons_are_disabled() {
        verifies!(
            r#"
    test 'should not mangle icon with link if icons are disabled' do
      para = block_from_string 'icon:github[link=https://github.com]'
      assert_equal '<span class="icon"><a class="image" href="https://github.com">[github&#93;</a></span>', para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new("icon:github[link=https://github.com]"));

        let expected = r#"<span class="icon"><a class="image" href="https://github.com">[github&#93;</a></span>"#;

        let p = Parser::default();

        SubstitutionStep::Macros.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(expected.to_string().into_boxed_str())
        );
    }

    #[ignore]
    #[test]
    fn should_not_mangle_icon_inside_link_if_icons_are_disabled() {
        verifies!(
            r#"
    test 'should not mangle icon inside link if icons are disabled' do
      para = block_from_string 'https://github.com[icon:github[] GitHub]'
      assert_equal '<a href="https://github.com"><span class="icon">[github&#93;</span> GitHub</a>', para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
        );

        // TO DO: Enable this test once http: links are supported.
        let mut content = crate::content::Content::from(crate::Span::new(
            "https://github.com[icon:github[] GitHub]",
        ));

        let expected =
            r#"<a href="https://github.com"><span class="icon">[github&#93;</span> GitHub</a>"#;

        let p = Parser::default();

        SubstitutionStep::Macros.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(expected.to_string().into_boxed_str())
        );
    }

    #[test]
    fn an_icon_macro_should_output_alt_text_if_icons_are_disabled_and_alt_is_given() {
        verifies!(
            r#"
    test 'an icon macro should output alt text if icons are disabled and alt is given' do
      para = block_from_string 'icon:github[alt="GitHub"]'
      assert_equal %{<span class="icon">[GitHub&#93;</span>}, para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new(r#"icon:github[alt="GitHub"]"#));

        let expected = r#"<span class="icon">[GitHub&#93;</span>"#;

        let p = Parser::default();

        SubstitutionStep::Macros.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(expected.to_string().into_boxed_str())
        );
    }

    #[test]
    fn an_icon_macro_should_be_interpreted_as_a_font_based_icon_when_icons_eq_font() {
        verifies!(
            r#"
    test 'an icon macro should be interpreted as a font-based icon when icons=font' do
      para = block_from_string 'icon:github[]', attributes: { 'icons' => 'font' }
      assert_equal %{<span class="icon"><i class="fa fa-github"></i></span>}, para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(r#"icon:github[]"#));

        let expected = r#"<span class="icon"><i class="fa fa-github"></i></span>"#;

        let p = Parser::default().with_intrinsic_attribute(
            "icons",
            "font",
            ModificationContext::ApiOnly,
        );

        SubstitutionStep::Macros.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(expected.to_string().into_boxed_str())
        );
    }

    #[test]
    fn an_icon_macro_with_a_size_should_be_interpreted_as_a_font_based_icon_with_a_size_when_icons_eq_font()
     {
        verifies!(
            r#"
    test 'an icon macro with a size should be interpreted as a font-based icon with a size when icons=font' do
      para = block_from_string 'icon:github[4x]', attributes: { 'icons' => 'font' }
      assert_equal %{<span class="icon"><i class="fa fa-github fa-4x"></i></span>}, para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(r#"icon:github[4x]"#));

        let expected = r#"<span class="icon"><i class="fa fa-github fa-4x"></i></span>"#;

        let p = Parser::default().with_intrinsic_attribute(
            "icons",
            "font",
            ModificationContext::ApiOnly,
        );

        SubstitutionStep::Macros.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(expected.to_string().into_boxed_str())
        );
    }

    #[test]
    fn an_icon_macro_with_flip_should_be_interpreted_as_a_flipped_font_based_icon_when_icons_eq_font()
     {
        verifies!(
            r#"
    test 'an icon macro with flip should be interpreted as a flipped font-based icon when icons=font' do
      para = block_from_string 'icon:shield[fw,flip=horizontal]', attributes: { 'icons' => 'font' }
      assert_equal '<span class="icon"><i class="fa fa-shield fa-fw fa-flip-horizontal"></i></span>', para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new(r#"icon:shield[fw,flip=horizontal]"#));

        let expected =
            r#"<span class="icon"><i class="fa fa-shield fa-fw fa-flip-horizontal"></i></span>"#;

        let p = Parser::default().with_intrinsic_attribute(
            "icons",
            "font",
            ModificationContext::ApiOnly,
        );

        SubstitutionStep::Macros.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(expected.to_string().into_boxed_str())
        );
    }

    #[test]
    fn an_icon_macro_with_rotate_should_be_interpreted_as_a_rotated_font_based_icon_when_icons_eq_font()
     {
        verifies!(
            r#"
    test 'an icon macro with rotate should be interpreted as a rotated font-based icon when icons=font' do
      para = block_from_string 'icon:shield[fw,rotate=90]', attributes: { 'icons' => 'font' }
      assert_equal '<span class="icon"><i class="fa fa-shield fa-fw fa-rotate-90"></i></span>', para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new(r#"icon:shield[fw,rotate=90]"#));

        let expected =
            r#"<span class="icon"><i class="fa fa-shield fa-fw fa-rotate-90"></i></span>"#;

        let p = Parser::default().with_intrinsic_attribute(
            "icons",
            "font",
            ModificationContext::ApiOnly,
        );

        SubstitutionStep::Macros.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(expected.to_string().into_boxed_str())
        );
    }

    #[test]
    fn an_icon_macro_with_a_role_and_title_should_be_interpreted_as_a_font_based_icon_with_a_class_and_title_when_icons_eq_font()
     {
        verifies!(
            r#"
    test 'an icon macro with a role and title should be interpreted as a font-based icon with a class and title when icons=font' do
      para = block_from_string 'icon:heart[role="red", title="Heart me"]', attributes: { 'icons' => 'font' }
      assert_equal %{<span class="icon red"><i class="fa fa-heart" title="Heart me"></i></span>}, para.sub_macros(para.source).gsub(/>\s+</, '><')
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(
            r#"icon:heart[role="red", title="Heart me"]"#,
        ));

        let expected =
            r#"<span class="icon red"><i class="fa fa-heart" title="Heart me"></i></span>"#;

        let p = Parser::default().with_intrinsic_attribute(
            "icons",
            "font",
            ModificationContext::ApiOnly,
        );

        SubstitutionStep::Macros.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(expected.to_string().into_boxed_str())
        );
    }

    mod footnotes {
        use crate::tests::prelude::*;

        #[test]
        fn single_line_footnote_macro_is_registered_and_output() {
            verifies!(
                r##"
    test 'a single-line footnote macro should be registered and output as a footnote' do
      para = block_from_string('Sentence text footnote:[An example footnote.].')
      assert_equal %(Sentence text <sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>.), para.sub_macros(para.source)
      assert_equal 1, para.document.catalog[:footnotes].size
      footnote = para.document.catalog[:footnotes].first
      assert_equal 1, footnote.index
      assert_nil footnote.id
      assert_equal 'An example footnote.', footnote.text
    end

"##
            );

            let doc = Parser::default().parse("Sentence text footnote:[An example footnote.].");
            assert_eq!(
                rendered_paragraphs(&doc),
                &[
                    r##"Sentence text <sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>."##
                ]
            );

            let footnotes = doc.catalog().footnotes();
            assert_eq!(footnotes.len(), 1);
            assert_eq!(footnotes[0].index, "1");
            assert_eq!(footnotes[0].id, None);
            assert_eq!(footnotes[0].text, "An example footnote.");
        }

        #[test]
        fn multi_line_footnote_macro_is_output_without_newline() {
            verifies!(
                r##"
    test 'a multi-line footnote macro should be registered and output as a footnote without newline' do
      para = block_from_string("Sentence text footnote:[An example footnote\nwith wrapped text.].")
      assert_equal %(Sentence text <sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>.), para.sub_macros(para.source)
      assert_equal 1, para.document.catalog[:footnotes].size
      footnote = para.document.catalog[:footnotes].first
      assert_equal 1, footnote.index
      assert_nil footnote.id
      assert_equal "An example footnote with wrapped text.", footnote.text
    end

"##
            );

            let doc = Parser::default()
                .parse("Sentence text footnote:[An example footnote\nwith wrapped text.].");
            assert_eq!(
                rendered_paragraphs(&doc),
                &[
                    r##"Sentence text <sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>."##
                ]
            );

            let footnotes = doc.catalog().footnotes();
            assert_eq!(footnotes.len(), 1);
            assert_eq!(footnotes[0].index, "1");
            assert_eq!(footnotes[0].id, None);
            assert_eq!(footnotes[0].text, "An example footnote with wrapped text.");
        }

        #[test]
        fn escaped_closing_square_bracket_is_unescaped_when_converted() {
            verifies!(
                r##"
    test 'an escaped closing square bracket in a footnote should be unescaped when converted' do
      para = block_from_string(%(footnote:[a #{BACKSLASH}] b].))
      assert_equal %(<sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>.), para.sub_macros(para.source)
      assert_equal 1, para.document.catalog[:footnotes].size
      footnote = para.document.catalog[:footnotes].first
      assert_equal "a ] b", footnote.text
    end

"##
            );

            let doc = Parser::default().parse("footnote:[a \\] b].");
            assert_eq!(
                rendered_paragraphs(&doc),
                &[
                    r##"<sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>."##
                ]
            );

            let footnotes = doc.catalog().footnotes();
            assert_eq!(footnotes.len(), 1);
            assert_eq!(footnotes[0].text, "a ] b");
        }

        #[test]
        fn footnote_macro_can_be_directly_adjacent_to_preceding_word() {
            verifies!(
                r##"
    test 'a footnote macro can be directly adjacent to preceding word' do
      para = block_from_string('Sentence textfootnote:[An example footnote.].')
      assert_equal %(Sentence text<sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>.), para.sub_macros(para.source)
    end

"##
            );

            let doc = Parser::default().parse("Sentence textfootnote:[An example footnote.].");
            assert_eq!(
                rendered_paragraphs(&doc),
                &[
                    r##"Sentence text<sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>."##
                ]
            );
        }

        #[test]
        fn footnote_macro_may_contain_an_escaped_backslash() {
            verifies!(
                r#"
    test 'a footnote macro may contain an escaped backslash' do
      para = block_from_string("footnote:[\\]]\nfootnote:[a \\] b]\nfootnote:[a \\]\\] b]")
      para.sub_macros(para.source)
      assert_equal 3, para.document.catalog[:footnotes].size
      footnote1 = para.document.catalog[:footnotes][0]
      assert_equal ']', footnote1.text
      footnote2 = para.document.catalog[:footnotes][1]
      assert_equal 'a ] b', footnote2.text
      footnote3 = para.document.catalog[:footnotes][2]
      assert_equal 'a ]] b', footnote3.text
    end

"#
            );

            let doc = Parser::default()
                .parse("footnote:[\\]]\nfootnote:[a \\] b]\nfootnote:[a \\]\\] b]");

            let footnotes = doc.catalog().footnotes();
            assert_eq!(footnotes.len(), 3);
            assert_eq!(footnotes[0].text, "]");
            assert_eq!(footnotes[1].text, "a ] b");
            assert_eq!(footnotes[2].text, "a ]] b");
        }

        #[test]
        fn footnote_macro_may_contain_a_link_macro() {
            verifies!(
                r##"
    test 'a footnote macro may contain a link macro' do
      para = block_from_string('Share your code. footnote:[https://github.com[GitHub]]')
      assert_equal %(Share your code. <sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>), para.sub_macros(para.source)
      assert_equal 1, para.document.catalog[:footnotes].size
      footnote1 = para.document.catalog[:footnotes][0]
      assert_equal '<a href="https://github.com">GitHub</a>', footnote1.text
    end

"##
            );

            let doc =
                Parser::default().parse("Share your code. footnote:[https://github.com[GitHub]]");
            assert_eq!(
                rendered_paragraphs(&doc),
                &[
                    r##"Share your code. <sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>"##
                ]
            );

            let footnotes = doc.catalog().footnotes();
            assert_eq!(footnotes.len(), 1);
            assert_eq!(
                footnotes[0].text,
                r#"<a href="https://github.com">GitHub</a>"#
            );
        }

        #[test]
        fn footnote_macro_may_contain_a_plain_url() {
            verifies!(
                r##"
    test 'a footnote macro may contain a plain URL' do
      para = block_from_string %(the JLine footnote:[https://github.com/jline/jline2]\nlibrary.)
      result = para.sub_macros para.source
      assert_equal %(the JLine <sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>\nlibrary.), result
      assert_equal 1, para.document.catalog[:footnotes].size
      fn1 = para.document.catalog[:footnotes].first
      assert_equal '<a href="https://github.com/jline/jline2" class="bare">https://github.com/jline/jline2</a>', fn1.text
    end

"##
            );

            let doc = Parser::default()
                .parse("the JLine footnote:[https://github.com/jline/jline2]\nlibrary.");
            assert_eq!(
                rendered_paragraphs(&doc),
                &[
                    "the JLine <sup class=\"footnote\">[<a id=\"_footnoteref_1\" class=\"footnote\" href=\"#_footnotedef_1\" title=\"View footnote.\">1</a>]</sup>\nlibrary."
                ]
            );

            let footnotes = doc.catalog().footnotes();
            assert_eq!(footnotes.len(), 1);
            assert_eq!(
                footnotes[0].text,
                r#"<a href="https://github.com/jline/jline2" class="bare">https://github.com/jline/jline2</a>"#
            );
        }

        #[test]
        fn footnote_macro_followed_by_a_semicolon_may_contain_a_plain_url() {
            verifies!(
                r##"
    test 'a footnote macro followed by a semi-colon may contain a plain URL' do
      para = block_from_string %(the JLine footnote:[https://github.com/jline/jline2];\nlibrary.)
      result = para.sub_macros para.source
      assert_equal %(the JLine <sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>;\nlibrary.), result
      assert_equal 1, para.document.catalog[:footnotes].size
      fn1 = para.document.catalog[:footnotes].first
      assert_equal '<a href="https://github.com/jline/jline2" class="bare">https://github.com/jline/jline2</a>', fn1.text
    end

"##
            );

            let doc = Parser::default()
                .parse("the JLine footnote:[https://github.com/jline/jline2];\nlibrary.");
            assert_eq!(
                rendered_paragraphs(&doc),
                &[
                    "the JLine <sup class=\"footnote\">[<a id=\"_footnoteref_1\" class=\"footnote\" href=\"#_footnotedef_1\" title=\"View footnote.\">1</a>]</sup>;\nlibrary."
                ]
            );

            let footnotes = doc.catalog().footnotes();
            assert_eq!(footnotes.len(), 1);
            assert_eq!(
                footnotes[0].text,
                r#"<a href="https://github.com/jline/jline2" class="bare">https://github.com/jline/jline2</a>"#
            );
        }

        #[test]
        fn footnote_macro_may_contain_text_formatting() {
            verifies!(
                r#"
    test 'a footnote macro may contain text formatting' do
      para = block_from_string 'You can download patches from the product page.footnote:[Only available with an _active_ subscription.]'
      para.convert
      footnotes = para.document.catalog[:footnotes]
      assert_equal 1, footnotes.size
      assert_equal 'Only available with an <em>active</em> subscription.', footnotes[0].text
    end

"#
            );

            let doc = Parser::default().parse(
                "You can download patches from the product page.footnote:[Only available with an _active_ subscription.]",
            );

            let footnotes = doc.catalog().footnotes();
            assert_eq!(footnotes.len(), 1);
            assert_eq!(
                footnotes[0].text,
                "Only available with an <em>active</em> subscription."
            );
        }

        #[test]
        fn externalized_footnote_macro_may_contain_text_formatting() {
            verifies!(
                r#"
    test 'an externalized footnote macro may contain text formatting' do
      input = <<~'EOS'
      :fn-disclaimer: pass:q[footnote:[Only available with an _active_ subscription.]]

      You can download patches from the production page.{fn-disclaimer}
      EOS
      doc = document_from_string input
      doc.convert
      footnotes = doc.catalog[:footnotes]
      assert_equal 1, footnotes.size
      assert_equal 'Only available with an <em>active</em> subscription.', footnotes[0].text
    end

"#
            );

            let doc = Parser::default().parse(
                ":fn-disclaimer: pass:q[footnote:[Only available with an _active_ subscription.]]\n\nYou can download patches from the production page.{fn-disclaimer}",
            );

            let footnotes = doc.catalog().footnotes();
            assert_eq!(footnotes.len(), 1);
            assert_eq!(
                footnotes[0].text,
                "Only available with an <em>active</em> subscription."
            );
        }

        #[test]
        fn footnote_macro_may_contain_a_shorthand_xref() {
            verifies!(
                r##"
    test 'a footnote macro may contain a shorthand xref' do
      # specialcharacters escaping is simulated
      para = block_from_string('text footnote:[&lt;&lt;_install,install&gt;&gt;]')
      doc = para.document
      doc.register :refs, ['_install', (Asciidoctor::Inline.new doc, :anchor, 'Install', type: :ref, target: '_install'), 'Install']
      catalog = doc.catalog
      assert_equal %(text <sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>), para.sub_macros(para.source)
      assert_equal 1, catalog[:footnotes].size
      footnote1 = catalog[:footnotes][0]
      assert_equal '<a href="#_install">install</a>', footnote1.text
    end

"##
            );

            // A cross-reference inside a footnote is captured as a placeholder
            // before the footnote text is extracted, then resolved in the
            // document-level pass so the stored footnote text holds the link.
            let doc = Parser::default()
                .parse("Sentence text.footnote:[See <<usage>>.]\n\n[#usage]\n== Usage\n");

            let footnotes = doc.catalog().footnotes();
            assert_eq!(footnotes.len(), 1);
            assert_eq!(footnotes[0].text, r##"See <a href="#usage">Usage</a>."##);
        }

        #[test]
        fn footnote_macro_may_contain_an_xref_macro() {
            verifies!(
                r##"
    test 'a footnote macro may contain an xref macro' do
      para = block_from_string('text footnote:[xref:_install[install]]')
      doc = para.document
      doc.register :refs, ['_install', (Asciidoctor::Inline.new doc, :anchor, 'Install', type: :ref, target: '_install'), 'Install']
      catalog = doc.catalog
      assert_equal %(text <sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>), para.sub_macros(para.source)
      assert_equal 1, catalog[:footnotes].size
      footnote1 = catalog[:footnotes][0]
      assert_equal '<a href="#_install">install</a>', footnote1.text
    end

"##
            );

            // The `xref:id[]` macro form works too, even though its literal `]`
            // would otherwise truncate the footnote: the cross-reference pass
            // turns it into a bracket-free placeholder before the footnote text
            // is captured.
            let doc = Parser::default()
                .parse("Sentence text.footnote:[See xref:usage[].]\n\n[#usage]\n== Usage\n");

            let footnotes = doc.catalog().footnotes();
            assert_eq!(footnotes.len(), 1);
            assert_eq!(footnotes[0].text, r##"See <a href="#usage">Usage</a>."##);
        }

        #[test]
        fn footnote_macro_may_contain_an_anchor_macro() {
            verifies!(
                r##"
    test 'a footnote macro may contain an anchor macro' do
      para = block_from_string('text footnote:[a [[b]] [[c\]\] d]')
      assert_equal %(text <sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>), para.sub_macros(para.source)
      assert_equal 1, para.document.catalog[:footnotes].size
      footnote1 = para.document.catalog[:footnotes][0]
      assert_equal 'a <a id="b"></a> [[c]] d', footnote1.text
    end

"##
            );

            let doc = Parser::default().parse("text footnote:[a [[b]] [[c\\]\\] d]");
            assert_eq!(
                rendered_paragraphs(&doc),
                &[
                    r##"text <sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>"##
                ]
            );

            let footnotes = doc.catalog().footnotes();
            assert_eq!(footnotes.len(), 1);
            assert_eq!(footnotes[0].text, r#"a <a id="b"></a> [[c]] d"#);
        }

        // Not ported to this crate:
        // - 'subsequent footnote macros with escaped URLs should be restored in
        //   DocBook': DocBook backend not supported.
        non_normative!(
            r#"
    test 'subsequent footnote macros with escaped URLs should be restored in DocBook' do
      input = 'foofootnote:[+http://example.com+]barfootnote:[+http://acme.com+]baz'

      result = convert_string_to_embedded input, doctype: 'inline', backend: 'docbook'
      assert_equal 'foo<footnote><simpara>http://example.com</simpara></footnote>bar<footnote><simpara>http://acme.com</simpara></footnote>baz', result
    end

"#
        );

        #[test]
        fn should_increment_index_of_subsequent_footnote_macros() {
            verifies!(
                r##"
    test 'should increment index of subsequent footnote macros' do
      para = block_from_string("Sentence text footnote:[An example footnote.]. Sentence text footnote:[Another footnote.].")
      assert_equal %(Sentence text <sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>. Sentence text <sup class="footnote">[<a id="_footnoteref_2" class="footnote" href="#_footnotedef_2" title="View footnote.">2</a>]</sup>.), para.sub_macros(para.source)
      assert_equal 2, para.document.catalog[:footnotes].size
      footnote1 = para.document.catalog[:footnotes][0]
      assert_equal 1, footnote1.index
      assert_nil footnote1.id
      assert_equal "An example footnote.", footnote1.text
      footnote2 = para.document.catalog[:footnotes][1]
      assert_equal 2, footnote2.index
      assert_nil footnote2.id
      assert_equal "Another footnote.", footnote2.text
    end

"##
            );

            let doc = Parser::default().parse(
                "Sentence text footnote:[An example footnote.]. Sentence text footnote:[Another footnote.].",
            );
            assert_eq!(
                rendered_paragraphs(&doc),
                &[
                    r##"Sentence text <sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>. Sentence text <sup class="footnote">[<a id="_footnoteref_2" class="footnote" href="#_footnotedef_2" title="View footnote.">2</a>]</sup>."##
                ]
            );

            let footnotes = doc.catalog().footnotes();
            assert_eq!(footnotes.len(), 2);
            assert_eq!(footnotes[0].index, "1");
            assert_eq!(footnotes[0].id, None);
            assert_eq!(footnotes[0].text, "An example footnote.");
            assert_eq!(footnotes[1].index, "2");
            assert_eq!(footnotes[1].id, None);
            assert_eq!(footnotes[1].text, "Another footnote.");
        }

        #[test]
        fn footnoteref_macro_with_id_and_single_line_text_is_registered() {
            verifies!(
                r##"
    test 'a footnoteref macro with id and single-line text should be registered and output as a footnote' do
      para = block_from_string 'Sentence text footnoteref:[ex1, An example footnote.].', attributes: { 'compat-mode' => '' }
      assert_equal %(Sentence text <sup class="footnote" id="_footnote_ex1">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>.), para.sub_macros(para.source)
      assert_equal 1, para.document.catalog[:footnotes].size
      footnote = para.document.catalog[:footnotes].first
      assert_equal 1, footnote.index
      assert_equal 'ex1', footnote.id
      assert_equal 'An example footnote.', footnote.text
    end

"##
            );

            let doc = Parser::default()
                .parse(":compat-mode:\n\nSentence text footnoteref:[ex1, An example footnote.].");
            assert_eq!(
                rendered_paragraphs(&doc),
                &[
                    r##"Sentence text <sup class="footnote" id="_footnote_ex1">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>."##
                ]
            );

            let footnotes = doc.catalog().footnotes();
            assert_eq!(footnotes.len(), 1);
            assert_eq!(footnotes[0].index, "1");
            assert_eq!(footnotes[0].id.as_deref(), Some("ex1"));
            assert_eq!(footnotes[0].text, "An example footnote.");
        }

        #[test]
        fn footnoteref_macro_with_id_and_multi_line_text_is_registered_without_newlines() {
            verifies!(
                r##"
    test 'a footnoteref macro with id and multi-line text should be registered and output as a footnote without newlines' do
      para = block_from_string "Sentence text footnoteref:[ex1, An example footnote\nwith wrapped text.].", attributes: { 'compat-mode' => '' }
      assert_equal %(Sentence text <sup class="footnote" id="_footnote_ex1">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>.), para.sub_macros(para.source)
      assert_equal 1, para.document.catalog[:footnotes].size
      footnote = para.document.catalog[:footnotes].first
      assert_equal 1, footnote.index
      assert_equal 'ex1', footnote.id
      assert_equal "An example footnote with wrapped text.", footnote.text
    end

"##
            );

            let doc = Parser::default().parse(
                ":compat-mode:\n\nSentence text footnoteref:[ex1, An example footnote\nwith wrapped text.].",
            );
            assert_eq!(
                rendered_paragraphs(&doc),
                &[
                    r##"Sentence text <sup class="footnote" id="_footnote_ex1">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>."##
                ]
            );

            let footnotes = doc.catalog().footnotes();
            assert_eq!(footnotes.len(), 1);
            assert_eq!(footnotes[0].id.as_deref(), Some("ex1"));
            assert_eq!(footnotes[0].text, "An example footnote with wrapped text.");
        }

        #[test]
        fn footnoteref_macro_with_id_should_refer_to_footnoteref_with_same_id() {
            verifies!(
                r##"
    test 'a footnoteref macro with id should refer to footnoteref with same id' do
      para = block_from_string 'Sentence text footnoteref:[ex1, An example footnote.]. Sentence text footnoteref:[ex1].', attributes: { 'compat-mode' => '' }
      assert_equal %(Sentence text <sup class="footnote" id="_footnote_ex1">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>. Sentence text <sup class="footnoteref">[<a class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>.), para.sub_macros(para.source)
      assert_equal 1, para.document.catalog[:footnotes].size
      footnote = para.document.catalog[:footnotes].first
      assert_equal 1, footnote.index
      assert_equal 'ex1', footnote.id
      assert_equal 'An example footnote.', footnote.text
    end

"##
            );

            let doc = Parser::default().parse(
                ":compat-mode:\n\nSentence text footnoteref:[ex1, An example footnote.]. Sentence text footnoteref:[ex1].",
            );
            assert_eq!(
                rendered_paragraphs(&doc),
                &[
                    r##"Sentence text <sup class="footnote" id="_footnote_ex1">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>. Sentence text <sup class="footnoteref">[<a class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>."##
                ]
            );

            let footnotes = doc.catalog().footnotes();
            assert_eq!(footnotes.len(), 1);
            assert_eq!(footnotes[0].index, "1");
            assert_eq!(footnotes[0].id.as_deref(), Some("ex1"));
            assert_eq!(footnotes[0].text, "An example footnote.");
        }

        #[test]
        fn unresolved_footnote_reference_produces_a_warning_and_red_fallback() {
            verifies!(
                r#"
    test 'an unresolved footnote reference should produce a warning message and output fallback text in red' do
      input = 'Sentence text.footnote:ex1[]'
      using_memory_logger do |logger|
        para = block_from_string input
        output = para.sub_macros para.source
        assert_equal 'Sentence text.<sup class="footnoteref red" title="Unresolved footnote reference.">[ex1]</sup>', output
        assert_message logger, :WARN, 'invalid footnote reference: ex1'
      end
    end

"#
            );

            let doc = Parser::default().parse("Sentence text.footnote:ex1[]");
            assert_eq!(
                rendered_paragraphs(&doc),
                &[
                    r#"Sentence text.<sup class="footnoteref red" title="Unresolved footnote reference.">[ex1]</sup>"#
                ]
            );

            let warnings: Vec<_> = doc.warnings().collect();
            assert_eq!(warnings.len(), 1);
            assert_eq!(
                warnings[0].warning,
                WarningType::InvalidFootnoteReference("ex1".to_string())
            );
        }

        #[test]
        fn footnoteref_macro_warns_when_compat_mode_is_not_enabled() {
            verifies!(
                r#"
    test 'using a footnoteref macro should generate a warning when compat mode is not enabled' do
      input = 'Sentence text.footnoteref:[fn1,Commentary on this sentence.]'
      using_memory_logger do |logger|
        para = block_from_string input
        para.sub_macros para.source
        assert_message logger, :WARN, 'found deprecated footnoteref macro: footnoteref:[fn1,Commentary on this sentence.]; use footnote macro with target instead'
      end
    end

"#
            );

            let doc = Parser::default()
                .parse("Sentence text.footnoteref:[fn1,Commentary on this sentence.]");

            let warnings: Vec<_> = doc.warnings().collect();
            assert_eq!(warnings.len(), 1);
            assert_eq!(
                warnings[0].warning,
                WarningType::DeprecatedFootnorefMacro(
                    "footnoteref:[fn1,Commentary on this sentence.]".to_string()
                )
            );
        }

        #[test]
        fn inline_footnote_macro_can_define_and_reference_a_footnote() {
            verifies!(
                r##"
    test 'inline footnote macro can be used to define and reference a footnote reference' do
      input = <<~'EOS'
      You can download the software from the product page.footnote:sub[Option only available if you have an active subscription.]

      You can also file a support request.footnote:sub[]

      If all else fails, you can give us a call.footnoteref:[sub]
      EOS

      using_memory_logger do |logger|
        output = convert_string_to_embedded input, attributes: { 'compat-mode' => '' }
        assert_css '#_footnotedef_1', output, 1
        assert_css 'p a[href="#_footnotedef_1"]', output, 3
        assert_css '#footnotes .footnote', output, 1
        assert logger.empty?
      end
    end

"##
            );

            let doc = Parser::default().parse(
                ":compat-mode:\n\nYou can download the software from the product page.footnote:sub[Option only available if you have an active subscription.]\n\nYou can also file a support request.footnote:sub[]\n\nIf all else fails, you can give us a call.footnoteref:[sub]",
            );

            // The footnote is defined once and referenced twice.
            assert_eq!(doc.catalog().footnotes().len(), 1);
            // Every occurrence links to the single footnote definition.
            assert_css(&doc, r##"a[href="#_footnotedef_1"]"##, 3);
            // Setting compat mode suppresses the footnoteref deprecation warning.
            assert_eq!(doc.warnings().count(), 0);
        }

        #[test]
        fn should_parse_multiple_footnote_references_in_a_single_line() {
            verifies!(
                r#"
    test 'should parse multiple footnote references in a single line' do
      input = 'notable text.footnote:id[about this [text\]], footnote:id[], footnote:id[]'
      output = convert_string_to_embedded input
      assert_xpath '(//p)[1]/sup[starts-with(@class,"footnote")]', output, 3
      assert_xpath '(//p)[1]/sup[@class="footnote"]', output, 1
      assert_xpath '(//p)[1]/sup[@class="footnoteref"]', output, 2
      assert_xpath '(//p)[1]/sup[starts-with(@class,"footnote")]/a[@class="footnote"][text()="1"]', output, 3
      assert_css '#footnotes .footnote', output, 1
    end

"#
            );

            let doc = Parser::default().parse(
                "notable text.footnote:id[about this [text\\]], footnote:id[], footnote:id[]",
            );

            // One defining occurrence and two references, all numbered 1.
            assert_css(&doc, "sup.footnote", 1);
            assert_css(&doc, "sup.footnoteref", 2);
            assert_xpath(&doc, r#"//a[@class="footnote"]"#, 3);

            let footnotes = doc.catalog().footnotes();
            assert_eq!(footnotes.len(), 1);
            assert_eq!(footnotes[0].text, "about this [text]");
        }

        #[test]
        fn should_not_register_footnote_with_id_and_text_if_id_already_registered() {
            verifies!(
                r#"
    test 'should not register footnote with id and text if id already registered' do
      input = <<~'EOS'
      :fn-notable-text: footnote:id[about this text]

      notable text.{fn-notable-text}

      more notable text.{fn-notable-text}
      EOS
      output = convert_string_to_embedded input
      assert_xpath '(//p)[1]/sup[@class="footnote"]', output, 1
      assert_xpath '(//p)[2]/sup[@class="footnoteref"]', output, 1
      assert_css '#footnotes .footnote', output, 1
    end

"#
            );

            let doc = Parser::default().parse(
                ":fn-notable-text: footnote:id[about this text]\n\nnotable text.{fn-notable-text}\n\nmore notable text.{fn-notable-text}",
            );

            assert_css(&doc, "sup.footnote", 1);
            assert_css(&doc, "sup.footnoteref", 1);
            assert_eq!(doc.catalog().footnotes().len(), 1);
        }

        #[test]
        fn should_not_resolve_an_inline_footnote_macro_missing_both_id_and_text() {
            verifies!(
                r#"
    test 'should not resolve an inline footnote macro missing both id and text' do
      input = <<~'EOS'
      The footnote:[] macro can be used for defining and referencing footnotes.

      The footnoteref:[] macro is now deprecated.
      EOS

      output = convert_string_to_embedded input
      assert_includes output, 'The footnote:[] macro'
      assert_includes output, 'The footnoteref:[] macro'
    end

"#
            );

            let doc = Parser::default().parse(
                "The footnote:[] macro can be used for defining and referencing footnotes.\n\nThe footnoteref:[] macro is now deprecated.",
            );

            assert_rendered_contains(&doc, "The footnote:[] macro");
            assert_rendered_contains(&doc, "The footnoteref:[] macro");
            assert!(doc.catalog().footnotes().is_empty());
        }

        #[test]
        fn inline_footnote_macro_can_define_a_numeric_id() {
            verifies!(
                r##"
    test 'inline footnote macro can define a numeric id without conflicting with auto-generated ID' do
      input = 'You can download the software from the product page.footnote:1[Option only available if you have an active subscription.]'

      output = convert_string_to_embedded input
      assert_css '#_footnote_1', output, 1
      assert_css 'p sup#_footnote_1', output, 1
      assert_css 'p a#_footnoteref_1', output, 1
      assert_css 'p a[href="#_footnotedef_1"]', output, 1
      assert_css '#footnotes #_footnotedef_1', output, 1
    end

"##
            );

            let doc = Parser::default().parse(
                "You can download the software from the product page.footnote:1[Option only available if you have an active subscription.]",
            );

            assert_css(&doc, "sup#_footnote_1", 1);
            assert_css(&doc, "a#_footnoteref_1", 1);
            assert_css(&doc, r##"a[href="#_footnotedef_1"]"##, 1);

            let footnotes = doc.catalog().footnotes();
            assert_eq!(footnotes.len(), 1);
            assert_eq!(footnotes[0].id.as_deref(), Some("1"));
        }

        #[test]
        fn inline_footnote_macro_can_define_an_id_with_unicode_word_characters() {
            verifies!(
                r#"
    test 'inline footnote macro can define an id that uses any word characters in Unicode' do
      input = <<~'EOS'
      L'origine du mot forêt{blank}footnote:forêt[un massif forestier] est complexe.

      Qu'est-ce qu'une forêt ?{blank}footnote:forêt[]
      EOS
      output = convert_string_to_embedded input
      assert_css '#_footnote_forêt', output, 1
      assert_css '#_footnotedef_1', output, 1
      assert_xpath '//a[@class="footnote"][text()="1"]', output, 2
    end

"#
            );

            let doc = Parser::default().parse(
                "L'origine du mot forêt{blank}footnote:forêt[un massif forestier] est complexe.\n\nQu'est-ce qu'une forêt ?{blank}footnote:forêt[]",
            );

            // (The DOM-based assert helpers can't slice the multi-byte `ê`, so
            // the rendered markup is checked directly.)
            let paras = rendered_paragraphs(&doc);
            assert!(
                paras[0].contains(r#"<sup class="footnote" id="_footnote_forêt">"#),
                "{}",
                paras[0]
            );
            // Both occurrences link to footnote 1.
            assert!(paras[0].contains(r##"href="#_footnotedef_1""##));
            assert!(paras[1].contains(r##"href="#_footnotedef_1""##));

            let footnotes = doc.catalog().footnotes();
            assert_eq!(footnotes.len(), 1);
            assert_eq!(footnotes[0].id.as_deref(), Some("forêt"));
            assert_eq!(footnotes[0].text, "un massif forestier");
        }

        #[test]
        fn should_be_able_to_reference_a_bibliography_entry_in_a_footnote() {
            verifies!(
                r##"
    test 'should be able to reference a bibliography entry in a footnote' do
      input = <<~'EOS'
      Choose a design pattern.footnote:[See <<gof>> to find a collection of design patterns.]

      [bibliography]
      == Bibliography

      * [[[gof]]] Erich Gamma, et al. _Design Patterns: Elements of Reusable Object-Oriented Software._ Addison-Wesley. 1994.
      EOS

      result = convert_string_to_embedded input
      assert_include '<a href="#_footnoteref_1">1</a>. See <a href="#gof">[gof]</a> to find a collection of design patterns.', result
    end

"##
            );

            let doc = Parser::default().parse(
                "Sentence text.footnote:[See <<taoup>>.]\n\n[bibliography]\n== References\n\n* [[[taoup]]] Eric Steven Raymond. _The Art of Unix Programming_. Addison-Wesley. ISBN 0-13-142901-9.\n",
            );

            let footnotes = doc.catalog().footnotes();
            assert_eq!(footnotes.len(), 1);
            // The bibliography anchor's reference text is its bracketed label.
            assert_eq!(footnotes[0].text, r##"See <a href="#taoup">[taoup]</a>."##);
        }

        #[test]
        #[ignore]
        // Intentional divergence (https://github.com/asciidoc-rs/asciidoc-parser/issues/594):
        // Asciidoctor converts section titles eagerly and out of document order
        // (to generate IDs and cross-reference text), so footnotes in headings
        // are numbered out of sequence — this test asserts that quirk. The crate
        // instead applies a title's substitutions before parsing its section
        // body, numbering heading footnotes in straightforward document order.
        // That in-order behavior is covered by
        // `asciidoc_lang::macros::footnote::footnotes_in_headings_are_numbered_in_document_order`,
        // so this Asciidoctor-fidelity test stays ignored.
        fn footnotes_in_headings_are_numbered_out_of_sequence() {
            verifies!(
                r#"
    test 'footnotes in headings are expected to be numbered out of sequence' do
      input = <<~'EOS'
      == Section 1

      para.footnote:[first footnote]

      == Section 2footnote:[second footnote]

      para.footnote:[third footnote]
      EOS

      result = convert_string_to_embedded input
      footnote_refs = xmlnodes_at_css 'a.footnote', result
      footnote_defs = xmlnodes_at_css 'div.footnote', result
      assert_equal 3, footnote_refs.length
      assert_equal %w(1 1 2), footnote_refs.map(&:text)
      assert_equal 3, footnote_defs.length
      assert_equal ['1. second footnote', '1. first footnote', '2. third footnote'], footnote_defs.map(&:text).map(&:strip)
    end

"#
            );
        }

        #[test]
        fn an_escaped_footnote_macro_is_emitted_literally() {
            // A leading backslash escapes the macro: the backslash is dropped
            // and the macro text is emitted verbatim, defining no footnote.
            let doc = Parser::default().parse("A \\footnote:[not a footnote] here.");
            assert_eq!(
                rendered_paragraphs(&doc),
                &["A footnote:[not a footnote] here."]
            );
            assert!(doc.catalog().footnotes().is_empty());
        }

        #[test]
        fn footnote_number_honors_a_non_integer_counter_seed() {
            // The `footnote-number` counter honors whatever seed the document
            // sets. A non-integer seed advances like Ruby's `String#succ`
            // (`z` -> `aa` -> `ab`), and the footnote number is rendered
            // verbatim, matching Asciidoctor rather than collapsing to `0`.
            let doc = Parser::default()
                .parse(":footnote-number: z\n\nOne.footnote:[first] two.footnote:[second]");

            let footnotes = doc.catalog().footnotes();
            assert_eq!(footnotes.len(), 2);
            assert_eq!(footnotes[0].index, "aa");
            assert_eq!(footnotes[1].index, "ab");

            let paras = rendered_paragraphs(&doc);
            assert!(
                paras[0].contains(
                    r##"<a id="_footnoteref_aa" class="footnote" href="#_footnotedef_aa" title="View footnote.">aa</a>"##
                ),
                "{}",
                paras[0]
            );
            assert!(paras[0].contains(r##"id="_footnoteref_ab""##));
        }

        #[test]
        fn text_matching_the_footnote_gate_but_not_the_macro_is_left_untouched() {
            // The footnote pass is gated on the text merely containing `tnote`
            // (a substring of `footnote`). A macro-like token that trips the
            // gate without being a footnote macro produces no match and is
            // emitted unchanged, defining no footnote.
            let doc = Parser::default().parse("A tnote:[x] here.");
            assert_eq!(rendered_paragraphs(&doc), &["A tnote:[x] here."]);
            assert!(doc.catalog().footnotes().is_empty());
        }

        #[test]
        fn a_footnote_macro_immediately_before_a_closing_anchor_tag_is_not_matched() {
            // Re-creates Asciidoctor's `(?!</a>)` look-ahead: a footnote macro
            // whose closing bracket is immediately followed by `</a>` (i.e. it
            // sits at the end of an already-rendered link's text) is left
            // untouched rather than being processed a second time.
            let mut content =
                crate::content::Content::from(crate::Span::new("x footnote:[note]</a>"));
            let parser = Parser::default();
            crate::content::SubstitutionStep::Macros.apply(&mut content, &parser, None);

            assert_eq!(content.rendered(), "x footnote:[note]</a>");
        }
    }

    mod index_terms {
        //! Ported from Asciidoctor's `substitutions_test.rs` index-term cases.
        //!
        //! These were previously stubbed (un-ported Ruby) because index terms
        //! weren't implemented. The crate targets HTML5 output, which never
        //! builds an index catalog, so the original `catalog[:indexterms]`
        //! assertions (commented out even in Ruby) and the DocBook-only
        //! `see`/`see-also` cases are intentionally omitted. What remains is
        //! the inline rendering each macro produces: a concealed term
        //! (`indexterm:[…]` / `(((…)))`) disappears, and a flow term
        //! (`indexterm2:[…]` / `((…))`) leaves its primary term in the text.

        use crate::tests::prelude::*;

        const SENTENCE: &str = "The tiger (Panthera tigris) is the largest cat species.";

        #[test]
        fn concealed_macro_with_primary_term() {
            verifies!(
                r##"
    test 'a single-line index term macro with a primary term should be registered as an index reference' do
      sentence = "The tiger (Panthera tigris) is the largest cat species.\n"
      macros = ['indexterm:[Tigers]', '(((Tigers)))']
      macros.each do |macro|
        para = block_from_string("#{sentence}#{macro}")
        output = para.sub_macros(para.source)
        assert_equal sentence, output
        #assert_equal 1, para.document.catalog[:indexterms].size
        #assert_equal ['Tigers'], para.document.catalog[:indexterms].first
      end
    end

"##
            );

            // 'a single-line index term macro with a primary term should be
            // registered as an index reference'
            for macro_ in ["indexterm:[Tigers]", "(((Tigers)))"] {
                let doc = Parser::default().parse(&format!("{SENTENCE}\n{macro_}"));
                assert_eq!(rendered_paragraphs(&doc), &[format!("{SENTENCE}\n")]);
            }
        }

        #[test]
        fn concealed_macro_with_primary_and_secondary_terms() {
            verifies!(
                r##"
    test 'a single-line index term macro with primary and secondary terms should be registered as an index reference' do
      sentence = "The tiger (Panthera tigris) is the largest cat species.\n"
      macros = ['indexterm:[Big cats, Tigers]', '(((Big cats, Tigers)))']
      macros.each do |macro|
        para = block_from_string("#{sentence}#{macro}")
        output = para.sub_macros(para.source)
        assert_equal sentence, output
        #assert_equal 1, para.document.catalog[:indexterms].size
        #assert_equal ['Big cats', 'Tigers'], para.document.catalog[:indexterms].first
      end
    end

"##
            );

            // 'a single-line index term macro with primary and secondary terms ...'
            for macro_ in ["indexterm:[Big cats, Tigers]", "(((Big cats, Tigers)))"] {
                let doc = Parser::default().parse(&format!("{SENTENCE}\n{macro_}"));
                assert_eq!(rendered_paragraphs(&doc), &[format!("{SENTENCE}\n")]);
            }
        }

        #[test]
        fn concealed_macro_with_primary_secondary_and_tertiary_terms() {
            verifies!(
                r##"
    test 'a single-line index term macro with primary, secondary and tertiary terms should be registered as an index reference' do
      sentence = "The tiger (Panthera tigris) is the largest cat species.\n"
      macros = ['indexterm:[Big cats,Tigers , Panthera tigris]', '(((Big cats,Tigers , Panthera tigris)))']
      macros.each do |macro|
        para = block_from_string("#{sentence}#{macro}")
        output = para.sub_macros(para.source)
        assert_equal sentence, output
        #assert_equal 1, para.document.catalog[:indexterms].size
        #assert_equal ['Big cats', 'Tigers', 'Panthera tigris'], para.document.catalog[:indexterms].first
      end
    end

"##
            );

            // 'a single-line index term macro with primary, secondary and
            // tertiary terms ...'
            for macro_ in [
                "indexterm:[Big cats,Tigers , Panthera tigris]",
                "(((Big cats,Tigers , Panthera tigris)))",
            ] {
                let doc = Parser::default().parse(&format!("{SENTENCE}\n{macro_}"));
                assert_eq!(rendered_paragraphs(&doc), &[format!("{SENTENCE}\n")]);
            }
        }

        #[test]
        fn multiline_concealed_macro_is_compacted() {
            verifies!(
                r##"
    test 'a multi-line index term macro should be compacted and registered as an index reference' do
      sentence = "The tiger (Panthera tigris) is the largest cat species.\n"
      macros = ["indexterm:[Panthera\ntigris]", "(((Panthera\ntigris)))"]
      macros.each do |macro|
        para = block_from_string("#{sentence}#{macro}")
        output = para.sub_macros(para.source)
        assert_equal sentence, output
        #assert_equal 1, para.document.catalog[:indexterms].size
        #assert_equal ['Panthera tigris'], para.document.catalog[:indexterms].first
      end
    end

"##
            );

            // 'a multi-line index term macro should be compacted and registered ...'
            for macro_ in ["indexterm:[Panthera\ntigris]", "(((Panthera\ntigris)))"] {
                let doc = Parser::default().parse(&format!("{SENTENCE}\n{macro_}"));
                assert_eq!(rendered_paragraphs(&doc), &[format!("{SENTENCE}\n")]);
            }
        }

        #[test]
        fn escape_concealed_term_when_second_bracket_escaped() {
            verifies!(
                r#"
    test 'should escape concealed index term if second bracket is preceded by a backslash' do
      input = %[National Institute of Science and Technology (#{BACKSLASH}((NIST)))]
      doc = document_from_string input, standalone: false
      output = doc.convert
      assert_xpath '//p[text()="National Institute of Science and Technology (((NIST)))"]', output, 1
      #assert doc.catalog[:indexterms].empty?
    end

"#
            );

            // 'should escape concealed index term if second bracket is preceded
            // by a backslash'
            let doc = Parser::default()
                .parse("National Institute of Science and Technology (\\((NIST)))");
            assert_eq!(
                rendered_paragraphs(&doc),
                &["National Institute of Science and Technology (((NIST)))"]
            );
        }

        #[test]
        fn escape_only_enclosing_brackets() {
            verifies!(
                r#"
    test 'should only escape enclosing brackets if concealed index term is preceded by a backslash' do
      input = %[National Institute of Science and Technology #{BACKSLASH}(((NIST)))]
      doc = document_from_string input, standalone: false
      output = doc.convert
      assert_xpath '//p[text()="National Institute of Science and Technology (NIST)"]', output, 1
      #term = doc.catalog[:indexterms].first
      #assert_equal 1, term.size
      #assert_equal 'NIST', term.first
    end

"#
            );

            // 'should only escape enclosing brackets if concealed index term is
            // preceded by a backslash'
            let doc = Parser::default()
                .parse("National Institute of Science and Technology \\(((NIST)))");
            assert_eq!(
                rendered_paragraphs(&doc),
                &["National Institute of Science and Technology (NIST)"]
            );
        }

        #[test]
        fn does_not_split_terms_on_commas_inside_quotes() {
            verifies!(
                r#"
    test 'should not split index terms on commas inside of quoted terms' do
      inputs = []
      inputs.push <<~'EOS'
      Tigers are big, scary cats.
      indexterm:[Tigers, "[Big\],
      scary cats"]
      EOS
      inputs.push <<~'EOS'
      Tigers are big, scary cats.
      (((Tigers, "[Big],
      scary cats")))
      EOS

      inputs.each do |input|
        para = block_from_string input
        output = para.sub_macros(para.source)
        assert_equal input.lines.first, output
        #assert_equal 1, para.document.catalog[:indexterms].size
        #terms = para.document.catalog[:indexterms].first
        #assert_equal 2, terms.size
        #assert_equal 'Tigers', terms.first
        #assert_equal '[Big], scary cats', terms.last
      end
    end

"#
            );

            // 'should not split index terms on commas inside of quoted terms'
            // The quoted comma keeps the term intact; the concealed term renders
            // nothing, so only the leading sentence remains.
            let macro_form =
                "Tigers are big, scary cats.\nindexterm:[Tigers, \"[Big\\],\nscary cats\"]";
            let shorthand = "Tigers are big, scary cats.\n(((Tigers, \"[Big],\nscary cats\")))";
            for input in [macro_form, shorthand] {
                let doc = Parser::default().parse(input);
                assert_eq!(
                    rendered_paragraphs(&doc),
                    &["Tigers are big, scary cats.\n"]
                );
            }
        }

        #[test]
        fn normal_substitutions_applied_to_concealed_macro() {
            verifies!(
                r##"
    test 'normal substitutions are performed on an index term macro' do
      sentence = "The tiger (Panthera tigris) is the largest cat species.\n"
      macros = ['indexterm:[*Tigers*]', '(((*Tigers*)))']
      macros.each do |macro|
        para = block_from_string("#{sentence}#{macro}")
        output = para.apply_subs(para.source)
        assert_equal sentence, output
        #assert_equal 1, para.document.catalog[:indexterms].size
        #assert_equal ['<strong>Tigers</strong>'], para.document.catalog[:indexterms].first
      end
    end

"##
            );

            // 'normal substitutions are performed on an index term macro'
            // The concealed term renders nothing regardless of inner formatting.
            for macro_ in ["indexterm:[*Tigers*]", "(((*Tigers*)))"] {
                let doc = Parser::default().parse(&format!("{SENTENCE}\n{macro_}"));
                assert_eq!(rendered_paragraphs(&doc), &[format!("{SENTENCE}\n")]);
            }
        }

        #[test]
        fn registers_multiple_concealed_macros() {
            verifies!(
                r##"
    test 'registers multiple index term macros' do
      sentence = "The tiger (Panthera tigris) is the largest cat species."
      macros = "(((Tigers)))\n(((Animals,Cats)))"
      para = block_from_string("#{sentence}\n#{macros}")
      output = para.sub_macros(para.source)
      assert_equal sentence, output.rstrip
      #assert_equal 2, para.document.catalog[:indexterms].size
      #assert_equal ['Tigers'], para.document.catalog[:indexterms][0]
      #assert_equal ['Animals', 'Cats'], para.document.catalog[:indexterms][1]
    end

"##
            );

            // 'registers multiple index term macros'
            let doc =
                Parser::default().parse(&format!("{SENTENCE}\n(((Tigers)))\n(((Animals,Cats)))"));
            assert_eq!(rendered_paragraphs(&doc), &[format!("{SENTENCE}\n\n")]);
        }

        #[test]
        fn concealed_term_may_contain_round_brackets() {
            verifies!(
                r##"
    test 'an index term macro with round bracket syntax may contain round brackets in term' do
      sentence = "The tiger (Panthera tigris) is the largest cat species.\n"
      macro = '(((Tiger (Panthera tigris))))'
      para = block_from_string("#{sentence}#{macro}")
      output = para.sub_macros(para.source)
      assert_equal sentence, output
      #assert_equal 1, para.document.catalog[:indexterms].size
      #assert_equal ['Tiger (Panthera tigris)'], para.document.catalog[:indexterms].first
    end

"##
            );

            // 'an index term macro with round bracket syntax may contain round
            // brackets in term'
            let doc =
                Parser::default().parse(&format!("{SENTENCE}\n(((Tiger (Panthera tigris))))"));
            assert_eq!(rendered_paragraphs(&doc), &[format!("{SENTENCE}\n")]);
        }

        #[test]
        fn shorthand_does_not_consume_trailing_round_bracket() {
            verifies!(
                r#"
    test 'visible shorthand index term macro should not consume trailing round bracket' do
      input = '(text with ((index term)))'
      expected = <<~'EOS'.chop
      (text with <indexterm>
      <primary>index term</primary>
      </indexterm>index term)
      EOS
      #expected_term = ['index term']
      para = block_from_string input, backend: :docbook
      output = para.sub_macros para.source
      assert_equal expected, output
      #indexterms_table = para.document.catalog[:indexterms]
      #assert_equal 1, indexterms_table.size
      #assert_equal expected_term, indexterms_table[0]
    end

"#
            );

            // HTML5 analogue of the DocBook-only test 'visible shorthand index
            // term macro should not consume trailing round bracket'.
            let doc = Parser::default().parse("(text with ((index term)))");
            assert_eq!(rendered_paragraphs(&doc), &["(text with index term)"]);
        }

        #[test]
        fn shorthand_does_not_consume_leading_round_bracket() {
            verifies!(
                r#"
    test 'visible shorthand index term macro should not consume leading round bracket' do
      input = '(((index term)) for text)'
      expected = <<~'EOS'.chop
      (<indexterm>
      <primary>index term</primary>
      </indexterm>index term for text)
      EOS
      #expected_term = ['index term']
      para = block_from_string input, backend: :docbook
      output = para.sub_macros para.source
      assert_equal expected, output
      #indexterms_table = para.document.catalog[:indexterms]
      #assert_equal 1, indexterms_table.size
      #assert_equal expected_term, indexterms_table[0]
    end

"#
            );

            // HTML5 analogue of 'visible shorthand index term macro should not
            // consume leading round bracket'.
            let doc = Parser::default().parse("(((index term)) for text)");
            assert_eq!(rendered_paragraphs(&doc), &["(index term for text)"]);
        }

        #[test]
        fn concealed_term_may_contain_square_brackets() {
            verifies!(
                r##"
    test 'an index term macro with square bracket syntax may contain square brackets in term' do
      sentence = "The tiger (Panthera tigris) is the largest cat species.\n"
      macro = 'indexterm:[Tiger [Panthera tigris\\]]'
      para = block_from_string("#{sentence}#{macro}")
      output = para.sub_macros(para.source)
      assert_equal sentence, output
      #assert_equal 1, para.document.catalog[:indexterms].size
      #assert_equal ['Tiger [Panthera tigris]'], para.document.catalog[:indexterms].first
    end

"##
            );

            // 'an index term macro with square bracket syntax may contain square
            // brackets in term'
            let doc = Parser::default().parse(&format!(
                "{SENTENCE}\nindexterm:[Tiger [Panthera tigris\\]]"
            ));
            assert_eq!(rendered_paragraphs(&doc), &[format!("{SENTENCE}\n")]);
        }

        #[test]
        fn flow_macro_retains_term_inline() {
            verifies!(
                r#"
    test 'a single-line index term 2 macro should be registered as an index reference and retain term inline' do
      sentence = 'The tiger (Panthera tigris) is the largest cat species.'
      macros = ['The indexterm2:[tiger] (Panthera tigris) is the largest cat species.', 'The ((tiger)) (Panthera tigris) is the largest cat species.']
      macros.each do |macro|
        para = block_from_string(macro)
        output = para.sub_macros(para.source)
        assert_equal sentence, output
        #assert_equal 1, para.document.catalog[:indexterms].size
        #assert_equal ['tiger'], para.document.catalog[:indexterms].first
      end
    end

"#
            );

            // 'a single-line index term 2 macro should be registered ... and
            // retain term inline'
            for input in [
                "The indexterm2:[tiger] (Panthera tigris) is the largest cat species.",
                "The ((tiger)) (Panthera tigris) is the largest cat species.",
            ] {
                let doc = Parser::default().parse(input);
                assert_eq!(rendered_paragraphs(&doc), &[SENTENCE.to_string()]);
            }
        }

        #[test]
        fn multiline_flow_macro_is_compacted() {
            verifies!(
                r#"
    test 'a multi-line index term 2 macro should be compacted and registered as an index reference and retain term inline' do
      sentence = 'The panthera tigris is the largest cat species.'
      macros = ["The indexterm2:[ panthera\ntigris ] is the largest cat species.", "The (( panthera\ntigris )) is the largest cat species."]
      macros.each do |macro|
        para = block_from_string(macro)
        output = para.sub_macros(para.source)
        assert_equal sentence, output
        #assert_equal 1, para.document.catalog[:indexterms].size
        #assert_equal ['panthera tigris'], para.document.catalog[:indexterms].first
      end
    end

"#
            );

            // 'a multi-line index term 2 macro should be compacted ... and retain
            // term inline'
            let expected = "The panthera tigris is the largest cat species.";
            for input in [
                "The indexterm2:[ panthera\ntigris ] is the largest cat species.",
                "The (( panthera\ntigris )) is the largest cat species.",
            ] {
                let doc = Parser::default().parse(input);
                assert_eq!(rendered_paragraphs(&doc), &[expected.to_string()]);
            }
        }

        #[test]
        fn registers_multiple_flow_macros() {
            verifies!(
                r#"
    test 'registers multiple index term 2 macros' do
      sentence = "The ((tiger)) (Panthera tigris) is the largest ((cat)) species."
      para = block_from_string(sentence)
      output = para.sub_macros(para.source)
      assert_equal 'The tiger (Panthera tigris) is the largest cat species.', output
      #assert_equal 2, para.document.catalog[:indexterms].size
      #assert_equal ['tiger'], para.document.catalog[:indexterms][0]
      #assert_equal ['cat'], para.document.catalog[:indexterms][1]
    end

"#
            );

            // 'registers multiple index term 2 macros'
            let doc = Parser::default()
                .parse("The ((tiger)) (Panthera tigris) is the largest ((cat)) species.");
            assert_eq!(rendered_paragraphs(&doc), &[SENTENCE.to_string()]);
        }

        #[test]
        fn escape_visible_term() {
            verifies!(
                r#"
    test 'should escape visible index term if preceded by a backslash' do
      sentence = "The #{BACKSLASH}((tiger)) (Panthera tigris) is the largest #{BACKSLASH}((cat)) species."
      para = block_from_string(sentence)
      output = para.sub_macros(para.source)
      assert_equal 'The ((tiger)) (Panthera tigris) is the largest ((cat)) species.', output
      #assert para.document.catalog[:indexterms].empty?
    end

"#
            );

            // 'should escape visible index term if preceded by a backslash'
            let doc = Parser::default()
                .parse("The \\((tiger)) (Panthera tigris) is the largest \\((cat)) species.");
            assert_eq!(
                rendered_paragraphs(&doc),
                &["The ((tiger)) (Panthera tigris) is the largest ((cat)) species."]
            );
        }

        #[test]
        fn normal_substitutions_applied_to_flow_macro() {
            verifies!(
                r#"
    test 'normal substitutions are performed on an index term 2 macro' do
      sentence = 'The ((*tiger*)) (Panthera tigris) is the largest cat species.'
      para = block_from_string sentence
      output = para.apply_subs(para.source)
      assert_equal 'The <strong>tiger</strong> (Panthera tigris) is the largest cat species.', output
      #assert_equal 1, para.document.catalog[:indexterms].size
      #assert_equal ['<strong>tiger</strong>'], para.document.catalog[:indexterms].first
    end

"#
            );

            // 'normal substitutions are performed on an index term 2 macro'
            let doc = Parser::default()
                .parse("The ((*tiger*)) (Panthera tigris) is the largest cat species.");
            assert_eq!(
                rendered_paragraphs(&doc),
                &["The <strong>tiger</strong> (Panthera tigris) is the largest cat species."]
            );
        }

        #[test]
        fn flow_and_concealed_shorthand_do_not_interfere() {
            verifies!(
                r#"
    test 'index term 2 macro with round bracket syntex should not interfer with index term macro with round bracket syntax' do
      sentence = "The ((panthera tigris)) is the largest cat species.\n(((Big cats,Tigers)))"
      para = block_from_string sentence
      output = para.sub_macros(para.source)
      assert_equal "The panthera tigris is the largest cat species.\n", output
      #terms = para.document.catalog[:indexterms]
      #assert_equal 2, terms.size
      #assert_equal ['panthera tigris'], terms[0]
      #assert_equal ['Big cats', 'Tigers'], terms[1]
    end

"#
            );

            // 'index term 2 macro with round bracket syntex should not interfer
            // with index term macro with round bracket syntax'
            let doc = Parser::default().parse(
                "The ((panthera tigris)) is the largest cat species.\n(((Big cats,Tigers)))",
            );
            assert_eq!(
                rendered_paragraphs(&doc),
                &["The panthera tigris is the largest cat species.\n"]
            );
        }

        #[test]
        fn flow_shorthand_strips_see_and_seealso() {
            verifies!(
                r#"
    test 'should parse visible shorthand index term with see and seealso' do
      sentence = '((Flash >> HTML 5)) has been supplanted by ((HTML 5 &> CSS 3 &> SVG)).'
      output = convert_string_to_embedded sentence, backend: 'docbook'
      indexterm_flash = <<~'EOS'.chop
      <indexterm>
      <primary>Flash</primary>
      <see>HTML 5</see>
      </indexterm>
      EOS
      indexterm_html5 = <<~'EOS'.chop
      <indexterm>
      <primary>HTML 5</primary>
      <seealso>CSS 3</seealso>
      <seealso>SVG</seealso>
      </indexterm>
      EOS
      assert_includes output, indexterm_flash
      assert_includes output, indexterm_html5
    end

"#
            );

            // The DocBook-only `see`/`see-also` structure is not modeled, but
            // Asciidoctor strips the `see` (` >> `) and `see-also` (` &> `)
            // clauses from a *visible* term's inline text regardless of backend,
            // leaving only the primary term in the flow of text.
            let doc =
                Parser::default().parse("((Flash >> HTML 5)) and ((HTML 5 &> CSS 3 &> SVG)) done.");
            assert_eq!(rendered_paragraphs(&doc), &["Flash and HTML 5 done."]);
        }

        // Not ported to this crate:
        // - 'should parse concealed shorthand index term with see and seealso':
        //   DocBook-only see/seealso structure; concealed shorthand form has no HTML5
        //   analogue (concealed term renders nothing).
        non_normative!(
            r#"
    test 'should parse concealed shorthand index term with see and seealso' do
      sentence = 'Flash(((Flash >> HTML 5))) has been supplanted by HTML 5(((HTML 5 &> CSS 3 &> SVG))).'
      output = convert_string_to_embedded sentence, backend: 'docbook'
      indexterm_flash = <<~'EOS'.chop
      <indexterm>
      <primary>Flash</primary>
      <see>HTML 5</see>
      </indexterm>
      EOS
      indexterm_html5 = <<~'EOS'.chop
      <indexterm>
      <primary>HTML 5</primary>
      <seealso>CSS 3</seealso>
      <seealso>SVG</seealso>
      </indexterm>
      EOS
      assert_includes output, indexterm_flash
      assert_includes output, indexterm_html5
    end

"#
        );

        #[test]
        fn flow_macro_strips_see_and_seealso_attributes() {
            verifies!(
                r#"
    test 'should parse visible index term macro with see and seealso' do
      sentence = 'indexterm2:[Flash,see=HTML 5] has been supplanted by indexterm2:[HTML 5,see-also="CSS 3, SVG"].'
      output = convert_string_to_embedded sentence, backend: 'docbook'
      indexterm_flash = <<~'EOS'.chop
      <indexterm>
      <primary>Flash</primary>
      <see>HTML 5</see>
      </indexterm>
      EOS
      indexterm_html5 = <<~'EOS'.chop
      <indexterm>
      <primary>HTML 5</primary>
      <seealso>CSS 3</seealso>
      <seealso>SVG</seealso>
      </indexterm>
      EOS
      assert_includes output, indexterm_flash
      assert_includes output, indexterm_html5
    end

"#
            );

            // For the macro form, `see`/`see-also` are named attributes, so the
            // visible inline text is just the first positional attribute.
            let doc = Parser::default().parse(
                "indexterm2:[Flash,see=HTML 5] and indexterm2:[HTML 5,see-also=\"CSS 3, SVG\"] done.",
            );
            assert_eq!(rendered_paragraphs(&doc), &["Flash and HTML 5 done."]);
        }

        #[test]
        fn flow_term_with_entities_but_no_see_clause_is_preserved() {
            // A visible term may contain `>`/`&` (rendered as entities) without
            // forming a `see`/`see-also` clause; the whole term is then kept.
            let doc = Parser::default().parse("A ((tom >& jerry)) term.");
            assert_eq!(rendered_paragraphs(&doc), &["A tom &gt;&amp; jerry term."]);
        }

        #[test]
        fn flow_macro_without_positional_term_falls_back_to_argument() {
            // When the argument carries only named attributes (no positional
            // primary term), Asciidoctor displays the raw argument text.
            let doc = Parser::default().parse("Only named indexterm2:[see=HTML 5] here.");
            assert_eq!(rendered_paragraphs(&doc), &["Only named see=HTML 5 here."]);
        }

        #[test]
        fn escape_macro_forms() {
            // A backslash before either macro form is honored: the macro is
            // emitted literally (without the backslash) and not interpreted.
            let doc =
                Parser::default().parse("A \\indexterm:[Tigers] and \\indexterm2:[Lancelot] here.");
            assert_eq!(
                rendered_paragraphs(&doc),
                &["A indexterm:[Tigers] and indexterm2:[Lancelot] here."]
            );
        }

        #[test]
        fn empty_macro_argument_is_passed_through() {
            // A truly empty argument does not match the macro (Asciidoctor
            // requires at least one character), so it is emitted verbatim.
            let doc = Parser::default().parse("an indexterm:[] here");
            assert_eq!(rendered_paragraphs(&doc), &["an indexterm:[] here"]);
        }
    }

    // Not ported to this crate:
    // - 'should parse concealed index term macro with see and seealso':
    //   DocBook-only see/seealso structure; concealed macro form has no HTML5
    //   analogue (concealed term renders nothing).
    // - 'should honor secondary and tertiary index terms when primary index term is
    //   quoted and contains equals sign': DocBook backend not supported; asserts
    //   <indexterm> primary/secondary/tertiary markup.
    non_normative!(
        r#"
    test 'should parse concealed index term macro with see and seealso' do
      sentence = 'Flashindexterm:[Flash,see=HTML 5] has been supplanted by HTML 5indexterm:[HTML 5,see-also="CSS 3, SVG"].'
      output = convert_string_to_embedded sentence, backend: 'docbook'
      indexterm_flash = <<~'EOS'.chop
      <indexterm>
      <primary>Flash</primary>
      <see>HTML 5</see>
      </indexterm>
      EOS
      indexterm_html5 = <<~'EOS'.chop
      <indexterm>
      <primary>HTML 5</primary>
      <seealso>CSS 3</seealso>
      <seealso>SVG</seealso>
      </indexterm>
      EOS
      assert_includes output, indexterm_flash
      assert_includes output, indexterm_html5
    end

    test 'should honor secondary and tertiary index terms when primary index term is quoted and contains equals sign' do
      sentence = 'Assigning variables.'
      expected = %(#{sentence}<indexterm><primary>name=value</primary><secondary>variable</secondary><tertiary>assignment</tertiary></indexterm>)
      macros = ['indexterm:["name=value",variable,assignment]', '(((name=value,variable,assignment)))']
      macros.each do |macro|
        para = block_from_string %(#{sentence}#{macro}), backend: 'docbook'
        output = (para.sub_macros para.source).tr ?\n, ''
        assert_equal expected, output
      end
    end

    context 'Button macro' do
"#
    );

    mod ui_macros {
        //! Ported from Asciidoctor's `substitutions_test.rb` Button, Keyboard,
        //! and Menu macro contexts.
        //!
        //! The crate targets HTML5 output, so the DocBook-backend cases are
        //! intentionally omitted, as is the shorthand menu syntax (`"File >
        //! Save"`), which per the spec is not on a standards track.
        //!
        //! Each UI macro is recognized only when the `experimental` document
        //! attribute is set, so every input is parsed with `:experimental:`.

        use crate::tests::prelude::*;

        /// Renders `input` as the body of a document that sets `experimental`,
        /// returning the rendered (post-substitution) inline text.
        fn render(input: &str) -> String {
            render_with_attributes(&[], input)
        }

        /// As [`render`], but sets additional document attributes, each
        /// supplied as a header line (e.g. `":icons: font"`).
        fn render_with_attributes(attribute_lines: &[&str], input: &str) -> String {
            let mut header = String::from(":experimental:\n");
            for line in attribute_lines {
                header.push_str(line);
                header.push('\n');
            }

            let doc = Parser::default().parse(&format!("{header}\n{input}"));
            rendered_paragraphs(&doc).join("")
        }

        // -- Button macro ----------------------------------------------------

        #[test]
        fn button_single_word() {
            verifies!(
                r#"
      test 'btn macro' do
        para = block_from_string('btn:[Save]', attributes: { 'experimental' => '' })
        assert_equal %q{<b class="button">Save</b>}, para.sub_macros(para.source)
      end

"#
            );

            // 'btn macro'
            assert_eq!(render("btn:[Save]"), r#"<b class="button">Save</b>"#);
        }

        #[test]
        fn button_spans_multiple_lines() {
            verifies!(
                r#"
      test 'btn macro that spans multiple lines' do
        para = block_from_string(%(btn:[Rebase and\nmerge]), attributes: { 'experimental' => '' })
        assert_equal %q{<b class="button">Rebase and merge</b>}, para.sub_macros(para.source)
      end

"#
            );

            // 'btn macro that spans multiple lines'
            assert_eq!(
                render("btn:[Rebase and\nmerge]"),
                r#"<b class="button">Rebase and merge</b>"#
            );
        }

        // Not ported to this crate:
        // - 'btn macro for docbook backend': DocBook backend not supported.
        non_normative!(
            r#"
      test 'btn macro for docbook backend' do
        para = block_from_string('btn:[Save]', backend: 'docbook', attributes: { 'experimental' => '' })
        assert_equal %q{<guibutton>Save</guibutton>}, para.sub_macros(para.source)
      end
    end

    context 'Keyboard macro' do
"#
        );

        #[test]
        fn kbd_single_key() {
            verifies!(
                r#"
      test 'kbd macro with single key' do
        para = block_from_string('kbd:[F3]', attributes: { 'experimental' => '' })
        assert_equal %q{<kbd>F3</kbd>}, para.sub_macros(para.source)
      end

"#
            );

            // 'kbd macro with single key'
            assert_eq!(render("kbd:[F3]"), "<kbd>F3</kbd>");
        }

        #[test]
        fn kbd_single_backslash_key() {
            verifies!(
                r#"
      test 'kbd macro with single backslash key' do
        para = block_from_string("kbd:[#{BACKSLASH} ]", attributes: { 'experimental' => '' })
        assert_equal %q(<kbd>\</kbd>), para.sub_macros(para.source)
      end

"#
            );

            // 'kbd macro with single backslash key'
            assert_eq!(render("kbd:[\\ ]"), "<kbd>\\</kbd>");
        }

        // Not ported to this crate:
        // - 'kbd macro with single key, docbook backend': DocBook backend not
        //   supported.
        non_normative!(
            r#"
      test 'kbd macro with single key, docbook backend' do
        para = block_from_string('kbd:[F3]', backend: 'docbook', attributes: { 'experimental' => '' })
        assert_equal %q{<keycap>F3</keycap>}, para.sub_macros(para.source)
      end

"#
        );

        #[test]
        fn kbd_key_combination() {
            verifies!(
                r#"
      test 'kbd macro with key combination' do
        para = block_from_string('kbd:[Ctrl+Shift+T]', attributes: { 'experimental' => '' })
        assert_equal %q{<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>T</kbd></span>}, para.sub_macros(para.source)
      end

"#
            );

            // 'kbd macro with key combination'
            assert_eq!(
                render("kbd:[Ctrl+Shift+T]"),
                r#"<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>T</kbd></span>"#
            );
        }

        #[test]
        fn kbd_key_combination_spans_multiple_lines() {
            verifies!(
                r#"
      test 'kbd macro with key combination that spans multiple lines' do
        para = block_from_string(%(kbd:[Ctrl +\nT]), attributes: { 'experimental' => '' })
        assert_equal %q{<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>T</kbd></span>}, para.sub_macros(para.source)
      end

"#
            );

            // 'kbd macro with key combination that spans multiple lines'
            assert_eq!(
                render("kbd:[Ctrl +\nT]"),
                r#"<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>T</kbd></span>"#
            );
        }

        // Not ported to this crate:
        // - 'kbd macro with key combination, docbook backend': DocBook backend not
        //   supported.
        non_normative!(
            r#"
      test 'kbd macro with key combination, docbook backend' do
        para = block_from_string('kbd:[Ctrl+Shift+T]', backend: 'docbook', attributes: { 'experimental' => '' })
        assert_equal %q{<keycombo><keycap>Ctrl</keycap><keycap>Shift</keycap><keycap>T</keycap></keycombo>}, para.sub_macros(para.source)
      end

"#
        );

        #[test]
        fn kbd_key_combination_pluses_with_spaces() {
            verifies!(
                r#"
      test 'kbd macro with key combination delimited by pluses with spaces' do
        para = block_from_string('kbd:[Ctrl + Shift + T]', attributes: { 'experimental' => '' })
        assert_equal %q{<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>T</kbd></span>}, para.sub_macros(para.source)
      end

"#
            );

            // 'kbd macro with key combination delimited by pluses with spaces'
            assert_eq!(
                render("kbd:[Ctrl + Shift + T]"),
                r#"<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>T</kbd></span>"#
            );
        }

        #[test]
        fn kbd_key_combination_commas() {
            verifies!(
                r#"
      test 'kbd macro with key combination delimited by commas' do
        para = block_from_string('kbd:[Ctrl,Shift,T]', attributes: { 'experimental' => '' })
        assert_equal %q{<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>T</kbd></span>}, para.sub_macros(para.source)
      end

"#
            );

            // 'kbd macro with key combination delimited by commas'
            assert_eq!(
                render("kbd:[Ctrl,Shift,T]"),
                r#"<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>T</kbd></span>"#
            );
        }

        #[test]
        fn kbd_key_combination_commas_with_spaces() {
            verifies!(
                r#"
      test 'kbd macro with key combination delimited by commas with spaces' do
        para = block_from_string('kbd:[Ctrl, Shift, T]', attributes: { 'experimental' => '' })
        assert_equal %q{<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>T</kbd></span>}, para.sub_macros(para.source)
      end

"#
            );

            // 'kbd macro with key combination delimited by commas with spaces'
            assert_eq!(
                render("kbd:[Ctrl, Shift, T]"),
                r#"<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>T</kbd></span>"#
            );
        }

        #[test]
        fn kbd_pluses_containing_comma_key() {
            verifies!(
                r#"
      test 'kbd macro with key combination delimited by plus containing a comma key' do
        para = block_from_string('kbd:[Ctrl+,]', attributes: { 'experimental' => '' })
        assert_equal %q{<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>,</kbd></span>}, para.sub_macros(para.source)
      end

"#
            );

            // 'kbd macro with key combination delimited by plus containing a comma key'
            assert_eq!(
                render("kbd:[Ctrl+,]"),
                r#"<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>,</kbd></span>"#
            );
        }

        #[test]
        fn kbd_commas_containing_plus_key() {
            verifies!(
                r#"
      test 'kbd macro with key combination delimited by commas containing a plus key' do
        para = block_from_string('kbd:[Ctrl, +, Shift]', attributes: { 'experimental' => '' })
        assert_equal %q{<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>+</kbd>+<kbd>Shift</kbd></span>}, para.sub_macros(para.source)
      end

"#
            );

            // 'kbd macro with key combination delimited by commas containing a plus key'
            assert_eq!(
                render("kbd:[Ctrl, +, Shift]"),
                r#"<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>+</kbd>+<kbd>Shift</kbd></span>"#
            );
        }

        #[test]
        fn kbd_last_key_matches_plus_delimiter() {
            verifies!(
                r#"
      test 'kbd macro with key combination where last key matches plus delimiter' do
        para = block_from_string('kbd:[Ctrl + +]', attributes: { 'experimental' => '' })
        assert_equal %q{<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>+</kbd></span>}, para.sub_macros(para.source)
      end

"#
            );

            // 'kbd macro with key combination where last key matches plus delimiter'
            assert_eq!(
                render("kbd:[Ctrl + +]"),
                r#"<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>+</kbd></span>"#
            );
        }

        #[test]
        fn kbd_last_key_matches_comma_delimiter() {
            verifies!(
                r#"
      test 'kbd macro with key combination where last key matches comma delimiter' do
        para = block_from_string('kbd:[Ctrl, ,]', attributes: { 'experimental' => '' })
        assert_equal %q{<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>,</kbd></span>}, para.sub_macros(para.source)
      end

"#
            );

            // 'kbd macro with key combination where last key matches comma delimiter'
            assert_eq!(
                render("kbd:[Ctrl, ,]"),
                r#"<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>,</kbd></span>"#
            );
        }

        #[test]
        fn kbd_combination_containing_escaped_bracket() {
            verifies!(
                r#"
      test 'kbd macro with key combination containing escaped bracket' do
        para = block_from_string('kbd:[Ctrl + \]]', attributes: { 'experimental' => '' })
        assert_equal %q{<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>]</kbd></span>}, para.sub_macros(para.source)
      end

"#
            );

            // 'kbd macro with key combination containing escaped bracket'
            assert_eq!(
                render("kbd:[Ctrl + \\]]"),
                r#"<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>]</kbd></span>"#
            );
        }

        #[test]
        fn kbd_combination_ending_in_backslash() {
            verifies!(
                r#"
      test 'kbd macro with key combination ending in backslash' do
        para = block_from_string("kbd:[Ctrl + #{BACKSLASH} ]", attributes: { 'experimental' => '' })
        assert_equal %q(<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>\\</kbd></span>), para.sub_macros(para.source)
      end

"#
            );

            // 'kbd macro with key combination ending in backslash'
            assert_eq!(
                render("kbd:[Ctrl + \\ ]"),
                r#"<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>\</kbd></span>"#
            );
        }

        #[test]
        fn kbd_looks_for_delimiter_beyond_first_character() {
            verifies!(
                r#"
      test 'kbd macro looks for delimiter beyond first character' do
        para = block_from_string('kbd:[,te]', attributes: { 'experimental' => '' })
        assert_equal %q(<kbd>,te</kbd>), para.sub_macros(para.source)
      end

"#
            );

            // 'kbd macro looks for delimiter beyond first character'
            assert_eq!(render("kbd:[,te]"), "<kbd>,te</kbd>");
        }

        #[test]
        fn kbd_restores_trailing_delimiter_as_key_value() {
            verifies!(
                r#"
      test 'kbd macro restores trailing delimiter as key value' do
        para = block_from_string('kbd:[te,]', attributes: { 'experimental' => '' })
        assert_equal %q(<kbd>te,</kbd>), para.sub_macros(para.source)
      end
"#
            );

            // 'kbd macro restores trailing delimiter as key value'
            assert_eq!(render("kbd:[te,]"), "<kbd>te,</kbd>");
        }

        non_normative!(
            r#"
    end

    context 'Menu macro' do
"#
        );

        #[test]
        fn menu_macro_syntax() {
            verifies!(
                r#"
      test 'should process menu using macro sytnax' do
        para = block_from_string('menu:File[]', attributes: { 'experimental' => '' })
        assert_equal %q{<b class="menuref">File</b>}, para.sub_macros(para.source)
      end

"#
            );

            // 'should process menu using macro sytnax'
            assert_eq!(render("menu:File[]"), r#"<b class="menuref">File</b>"#);
        }

        // Not ported to this crate:
        // - 'should process menu for docbook backend': DocBook backend not supported.
        non_normative!(
            r#"
      test 'should process menu for docbook backend' do
        para = block_from_string('menu:File[]', backend: 'docbook', attributes: { 'experimental' => '' })
        assert_equal %q{<guimenu>File</guimenu>}, para.sub_macros(para.source)
      end

"#
        );

        #[test]
        fn menu_multiple_macros_same_line() {
            verifies!(
                r#"
      test 'should process multiple menu macros in same line' do
        para = block_from_string('menu:File[] and menu:Edit[]', attributes: { 'experimental' => '' })
        assert_equal '<b class="menuref">File</b> and <b class="menuref">Edit</b>', para.sub_macros(para.source)
      end

"#
            );

            // 'should process multiple menu macros in same line'
            assert_eq!(
                render("menu:File[] and menu:Edit[]"),
                r#"<b class="menuref">File</b> and <b class="menuref">Edit</b>"#
            );
        }

        #[test]
        fn menu_with_menu_item_macro_syntax() {
            verifies!(
                r#"
      test 'should process menu with menu item using macro syntax' do
        para = block_from_string('menu:File[Save As&#8230;]', attributes: { 'experimental' => '' })
        assert_equal %q{<span class="menuseq"><b class="menu">File</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">Save As&#8230;</b></span>}, para.sub_macros(para.source)
      end

"#
            );

            // 'should process menu with menu item using macro syntax'
            assert_eq!(
                render("menu:File[Save As&#8230;]"),
                r#"<span class="menuseq"><b class="menu">File</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">Save As&#8230;</b></span>"#
            );
        }

        #[test]
        fn menu_macro_spans_multiple_lines() {
            verifies!(
                r#"
      test 'should process menu macro that spans multiple lines' do
        input = %(menu:Preferences[Compile\non\nSave])
        para = block_from_string input, attributes: { 'experimental' => '' }
        assert_equal %(<span class="menuseq"><b class="menu">Preferences</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">Compile\non\nSave</b></span>), para.sub_macros(para.source)
      end

"#
            );

            // 'should process menu macro that spans multiple lines'
            assert_eq!(
                render("menu:Preferences[Compile\non\nSave]"),
                "<span class=\"menuseq\"><b class=\"menu\">Preferences</b>&#160;<b class=\"caret\">&#8250;</b> <b class=\"menuitem\">Compile\non\nSave</b></span>"
            );
        }

        #[test]
        fn menu_unescape_escaped_closing_bracket() {
            verifies!(
                r#"
      test 'should unescape escaped closing bracket in menu macro' do
        input = 'menu:Preferences[Compile [on\\] Save]'
        para = block_from_string input, attributes: { 'experimental' => '' }
        assert_equal %q(<span class="menuseq"><b class="menu">Preferences</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">Compile [on] Save</b></span>), para.sub_macros(para.source)
      end

"#
            );

            // 'should unescape escaped closing bracket in menu macro'
            assert_eq!(
                render("menu:Preferences[Compile [on\\] Save]"),
                r#"<span class="menuseq"><b class="menu">Preferences</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">Compile [on] Save</b></span>"#
            );
        }

        #[test]
        fn menu_with_menu_item_when_font_icons_enabled() {
            verifies!(
                r#"
      test 'should process menu with menu item using macro syntax when fonts icons are enabled' do
        para = block_from_string('menu:Tools[More Tools &gt; Extensions]', attributes: { 'experimental' => '', 'icons' => 'font' })
        assert_equal %q{<span class="menuseq"><b class="menu">Tools</b>&#160;<i class="fa fa-angle-right caret"></i> <b class="submenu">More Tools</b>&#160;<i class="fa fa-angle-right caret"></i> <b class="menuitem">Extensions</b></span>}, para.sub_macros(para.source)
      end

"#
            );

            // 'should process menu with menu item using macro syntax when fonts icons are
            // enabled'
            assert_eq!(
                render_with_attributes(&[":icons: font"], "menu:Tools[More Tools &gt; Extensions]"),
                r#"<span class="menuseq"><b class="menu">Tools</b>&#160;<i class="fa fa-angle-right caret"></i> <b class="submenu">More Tools</b>&#160;<i class="fa fa-angle-right caret"></i> <b class="menuitem">Extensions</b></span>"#
            );
        }

        // Not ported to this crate:
        // - 'should process menu with menu item for docbook backend': DocBook backend
        //   not supported.
        non_normative!(
            r#"
      test 'should process menu with menu item for docbook backend' do
        para = block_from_string('menu:File[Save As&#8230;]', backend: 'docbook', attributes: { 'experimental' => '' })
        assert_equal %q{<menuchoice><guimenu>File</guimenu> <guimenuitem>Save As&#8230;</guimenuitem></menuchoice>}, para.sub_macros(para.source)
      end

"#
        );

        #[test]
        fn menu_with_menu_item_in_submenu_macro_syntax() {
            verifies!(
                r#"
      test 'should process menu with menu item in submenu using macro syntax' do
        para = block_from_string('menu:Tools[Project &gt; Build]', attributes: { 'experimental' => '' })
        assert_equal %q{<span class="menuseq"><b class="menu">Tools</b>&#160;<b class="caret">&#8250;</b> <b class="submenu">Project</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">Build</b></span>}, para.sub_macros(para.source)
      end

"#
            );

            // 'should process menu with menu item in submenu using macro syntax'
            assert_eq!(
                render("menu:Tools[Project &gt; Build]"),
                r#"<span class="menuseq"><b class="menu">Tools</b>&#160;<b class="caret">&#8250;</b> <b class="submenu">Project</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">Build</b></span>"#
            );
        }

        // Not ported to this crate:
        // - 'should process menu with menu item in submenu for docbook backend':
        //   DocBook backend not supported.
        non_normative!(
            r#"
      test 'should process menu with menu item in submenu for docbook backend' do
        para = block_from_string('menu:Tools[Project &gt; Build]', backend: 'docbook', attributes: { 'experimental' => '' })
        assert_equal %q{<menuchoice><guimenu>Tools</guimenu> <guisubmenu>Project</guisubmenu> <guimenuitem>Build</guimenuitem></menuchoice>}, para.sub_macros(para.source)
      end

"#
        );

        #[test]
        fn menu_with_menu_item_in_submenu_comma_delimiter() {
            verifies!(
                r#"
      test 'should process menu with menu item in submenu using macro syntax and comma delimiter' do
        para = block_from_string('menu:Tools[Project, Build]', attributes: { 'experimental' => '' })
        assert_equal %q{<span class="menuseq"><b class="menu">Tools</b>&#160;<b class="caret">&#8250;</b> <b class="submenu">Project</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">Build</b></span>}, para.sub_macros(para.source)
      end

"#
            );

            // 'should process menu with menu item in submenu using macro syntax and comma
            // delimiter'
            assert_eq!(
                render("menu:Tools[Project, Build]"),
                r#"<span class="menuseq"><b class="menu">Tools</b>&#160;<b class="caret">&#8250;</b> <b class="submenu">Project</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">Build</b></span>"#
            );
        }

        // Not ported to this crate:
        // - 'should process menu with menu item using inline syntax': Inline
        //   (shorthand) menu syntax not ported.
        // - 'should process menu with menu item in submenu using inline syntax': Inline
        //   (shorthand) menu syntax not ported.
        // - 'inline menu syntax should not match closing quote of XML attribute':
        //   Inline (shorthand) menu syntax not ported.
        non_normative!(
            r#"
      test 'should process menu with menu item using inline syntax' do
        para = block_from_string('"File &gt; Save As&#8230;"', attributes: { 'experimental' => '' })
        assert_equal %q{<span class="menuseq"><b class="menu">File</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">Save As&#8230;</b></span>}, para.sub_macros(para.source)
      end

      test 'should process menu with menu item in submenu using inline syntax' do
        para = block_from_string('"Tools &gt; Project &gt; Build"', attributes: { 'experimental' => '' })
        assert_equal %q{<span class="menuseq"><b class="menu">Tools</b>&#160;<b class="caret">&#8250;</b> <b class="submenu">Project</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">Build</b></span>}, para.sub_macros(para.source)
      end

      test 'inline menu syntax should not match closing quote of XML attribute' do
        para = block_from_string('<span class="xmltag">&lt;node&gt;</span><span class="classname">r</span>', attributes: { 'experimental' => '' })
        assert_equal %q{<span class="xmltag">&lt;node&gt;</span><span class="classname">r</span>}, para.sub_macros(para.source)
      end

"#
        );

        #[test]
        fn menu_macro_with_multibyte_characters() {
            verifies!(
                r#"
      test 'should process menu macro with items containing multibyte characters' do
        para = block_from_string('menu:视图[放大, 重置]', attributes: { 'experimental' => '' })
        assert_equal %q{<span class="menuseq"><b class="menu">视图</b>&#160;<b class="caret">&#8250;</b> <b class="submenu">放大</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">重置</b></span>}, para.sub_macros(para.source)
      end

"#
            );

            // 'should process menu macro with items containing multibyte characters'
            assert_eq!(
                render("menu:视图[放大, 重置]"),
                r#"<span class="menuseq"><b class="menu">视图</b>&#160;<b class="caret">&#8250;</b> <b class="submenu">放大</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">重置</b></span>"#
            );
        }

        // Not ported to this crate:
        // - 'should process inline menu with items containing multibyte characters':
        //   Inline (shorthand) menu syntax not ported.
        non_normative!(
            r#"
      test 'should process inline menu with items containing multibyte characters' do
        para = block_from_string('"视图 &gt; 放大 &gt; 重置"', attributes: { 'experimental' => '' })
        assert_equal %q{<span class="menuseq"><b class="menu">视图</b>&#160;<b class="caret">&#8250;</b> <b class="submenu">放大</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">重置</b></span>}, para.sub_macros(para.source)
      end

"#
        );

        #[test]
        fn menu_macro_target_begins_with_character_reference() {
            verifies!(
                r#"
      test 'should process a menu macro with a target that begins with a character reference' do
        para = block_from_string('menu:&#8942;[More Tools, Extensions]', attributes: { 'experimental' => '' })
        assert_equal %q{<span class="menuseq"><b class="menu">&#8942;</b>&#160;<b class="caret">&#8250;</b> <b class="submenu">More Tools</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">Extensions</b></span>}, para.sub_macros(para.source)
      end

"#
            );

            // 'should process a menu macro with a target that begins with a character
            // reference'
            assert_eq!(
                render("menu:&#8942;[More Tools, Extensions]"),
                r#"<span class="menuseq"><b class="menu">&#8942;</b>&#160;<b class="caret">&#8250;</b> <b class="submenu">More Tools</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">Extensions</b></span>"#
            );
        }

        #[test]
        fn menu_macro_target_ends_with_space_not_processed() {
            verifies!(
                r#"
      test 'should not process a menu macro with a target that ends with a space' do
        input = 'menu:foo [bar] menu:File[Save]'
        para = block_from_string input, attributes: { 'experimental' => '' }
        result = para.sub_macros para.source
        assert_xpath '/span[@class="menuseq"]', result, 1
        assert_xpath '//b[@class="menu"][text()="File"]', result, 1
      end

"#
            );

            // 'should not process a menu macro with a target that ends with a space'
            // Only the second (well-formed) macro is processed; the first is
            // left as literal text.
            assert_eq!(
                render("menu:foo [bar] menu:File[Save]"),
                r#"menu:foo [bar] <span class="menuseq"><b class="menu">File</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">Save</b></span>"#
            );
        }

        // Not ported to this crate:
        // - 'should process an inline menu that begins with a character reference':
        //   Inline (shorthand) menu syntax not ported.
        non_normative!(
            r#"
      test 'should process an inline menu that begins with a character reference' do
        para = block_from_string('"&#8942; &gt; More Tools &gt; Extensions"', attributes: { 'experimental' => '' })
        assert_equal %q{<span class="menuseq"><b class="menu">&#8942;</b>&#160;<b class="caret">&#8250;</b> <b class="submenu">More Tools</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">Extensions</b></span>}, para.sub_macros(para.source)
      end
"#
        );

        #[test]
        fn escaped_macros_are_emitted_verbatim() {
            // A leading backslash escapes each UI macro, emitting the macro text
            // without the backslash.
            assert_eq!(render("\\kbd:[F3]"), "kbd:[F3]");
            assert_eq!(render("\\btn:[Save]"), "btn:[Save]");
            assert_eq!(render("\\menu:File[Save]"), "menu:File[Save]");
        }
    }

    non_normative!(
        r#"
    end
"#
    );

    #[test]
    fn an_icon_macro_with_width_should_be_interpreted_as_an_icon_with_width_if_icons_are_enabled() {
        let mut content =
            crate::content::Content::from(crate::Span::new(r#"icon:github[width=32]"#));

        let expected = r#"<span class="icon"><img src="./images/icons/github.png" alt="github" width="32"></span>"#;

        let p = Parser::default().with_intrinsic_attribute_bool(
            "icons",
            true,
            ModificationContext::ApiOnly,
        );

        SubstitutionStep::Macros.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(expected.to_string().into_boxed_str())
        );
    }

    #[test]
    fn an_icon_macro_with_height_should_be_interpreted_as_an_icon_with_height_if_icons_are_enabled()
    {
        let mut content =
            crate::content::Content::from(crate::Span::new(r#"icon:github[height=24]"#));

        let expected = r#"<span class="icon"><img src="./images/icons/github.png" alt="github" height="24"></span>"#;

        let p = Parser::default().with_intrinsic_attribute_bool(
            "icons",
            true,
            ModificationContext::ApiOnly,
        );

        SubstitutionStep::Macros.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(expected.to_string().into_boxed_str())
        );
    }

    #[test]
    fn an_icon_macro_with_title_should_be_interpreted_as_an_icon_with_title_if_icons_are_enabled() {
        let mut content =
            crate::content::Content::from(crate::Span::new(r#"icon:github[title="GitHub Icon"]"#));

        let expected = r#"<span class="icon"><img src="./images/icons/github.png" alt="github" title="GitHub Icon"></span>"#;

        let p = Parser::default().with_intrinsic_attribute_bool(
            "icons",
            true,
            ModificationContext::ApiOnly,
        );

        SubstitutionStep::Macros.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(expected.to_string().into_boxed_str())
        );
    }
}

non_normative!(
    r#"
  end

  context 'Passthroughs' do
"#
);

mod passthroughs {
    use crate::{
        content::{Passthroughs, SubstitutionStep, passthroughs::Passthrough},
        parser::{ModificationContext, QuoteType},
        tests::prelude::*,
    };

    #[test]
    fn collect_inline_triple_plus_passthroughs() {
        verifies!(
            r#"
    test 'collect inline triple plus passthroughs' do
      para = block_from_string('+++<code>inline code</code>+++')
      result = para.extract_passthroughs(para.source)
      passthroughs = para.instance_variable_get :@passthroughs
      assert_equal Asciidoctor::Substitutors::PASS_START + '0' + Asciidoctor::Substitutors::PASS_END, result
      assert_equal 1, passthroughs.size
      assert_equal '<code>inline code</code>', passthroughs[0][:text]
      assert_empty passthroughs[0][:subs]
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new("+++<code>inline code</code>+++"));

        let pt = Passthroughs::extract_from(&mut content, &Parser::default());

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "+++<code>inline code</code>+++",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "\u{96}0\u{97}",
            }
        );

        assert_eq!(
            pt,
            Passthroughs(vec![Passthrough {
                text: "<code>inline code</code>".to_owned(),
                subs: SubstitutionGroup::None,
                type_: None,
                attrlist: None,
            },],)
        );
    }

    #[test]
    fn collect_inline_triple_plus_passthroughs_with_attrlist() {
        // NOTE: Not in the Ruby test suite.
        let mut content =
            crate::content::Content::from(crate::Span::new("[role]+++<code>inline code</code>+++"));

        let pt = Passthroughs::extract_from(&mut content, &Parser::default());

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "[role]+++<code>inline code</code>+++",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "\u{96}0\u{97}",
            }
        );

        assert_eq!(
            pt,
            Passthroughs(vec![Passthrough {
                text: "<code>inline code</code>".to_owned(),
                subs: SubstitutionGroup::None,
                type_: Some(QuoteType::Unquoted,),
                attrlist: Some("role".to_owned(),),
            },],)
        );
    }

    #[test]
    fn collect_multiline_inline_triple_plus_passthroughs() {
        verifies!(
            r#"
    test 'collect multi-line inline triple plus passthroughs' do
      para = block_from_string("+++<code>inline\ncode</code>+++")
      result = para.extract_passthroughs(para.source)
      passthroughs = para.instance_variable_get :@passthroughs
      assert_equal Asciidoctor::Substitutors::PASS_START + '0' + Asciidoctor::Substitutors::PASS_END, result
      assert_equal 1, passthroughs.size
      assert_equal "<code>inline\ncode</code>", passthroughs[0][:text]
      assert_empty passthroughs[0][:subs]
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new("+++<code>inline\ncode</code>+++"));

        let pt = Passthroughs::extract_from(&mut content, &Parser::default());

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "+++<code>inline\ncode</code>+++",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "\u{96}0\u{97}",
            }
        );

        assert_eq!(
            pt,
            Passthroughs(vec![Passthrough {
                text: "<code>inline\ncode</code>".to_owned(),
                subs: SubstitutionGroup::None,
                type_: None,
                attrlist: None,
            },],)
        );
    }

    #[test]
    fn collect_inline_double_dollar_passthroughs() {
        verifies!(
            r#"
    test 'collect inline double dollar passthroughs' do
      para = block_from_string('$$<code>{code}</code>$$')
      result = para.extract_passthroughs(para.source)
      passthroughs = para.instance_variable_get :@passthroughs
      assert_equal Asciidoctor::Substitutors::PASS_START + '0' + Asciidoctor::Substitutors::PASS_END, result
      assert_equal 1, passthroughs.size
      assert_equal '<code>{code}</code>', passthroughs[0][:text]
      assert_equal [:specialcharacters], passthroughs[0][:subs]
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new("$$<code>{code}</code>$$"));

        let pt = Passthroughs::extract_from(&mut content, &Parser::default());

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "$$<code>{code}</code>$$",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "\u{96}0\u{97}",
            }
        );

        assert_eq!(
            pt,
            Passthroughs(vec![Passthrough {
                text: "<code>{code}</code>".to_owned(),
                subs: SubstitutionGroup::Verbatim,
                type_: None,
                attrlist: None,
            },],)
        );
    }

    #[test]
    fn collect_inline_double_plus_passthroughs() {
        verifies!(
            r#"
    test 'collect inline double plus passthroughs' do
      para = block_from_string('++<code>{code}</code>++')
      result = para.extract_passthroughs(para.source)
      passthroughs = para.instance_variable_get :@passthroughs
      assert_equal Asciidoctor::Substitutors::PASS_START + '0' + Asciidoctor::Substitutors::PASS_END, result
      assert_equal 1, passthroughs.size
      assert_equal '<code>{code}</code>', passthroughs[0][:text]
      assert_equal [:specialcharacters], passthroughs[0][:subs]
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new("++<code>{code}</code>++"));

        let pt = Passthroughs::extract_from(&mut content, &Parser::default());

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "++<code>{code}</code>++",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "\u{96}0\u{97}",
            }
        );

        assert_eq!(
            pt,
            Passthroughs(vec![Passthrough {
                text: "<code>{code}</code>".to_owned(),
                subs: SubstitutionGroup::Verbatim,
                type_: None,
                attrlist: None,
            },],)
        );
    }

    #[test]
    fn should_not_crash_if_role_on_passthrough_is_enclosed_in_quotes_1() {
        verifies!(
            r#"
    test 'should not crash if role on passthrough is enclosed in quotes' do
      %W(
        ['role']#{BACKSLASH}++This++++++++++++
        ['role']#{BACKSLASH}+++++++++This++++++++++++
      ).each do |input|
        para = block_from_string input
        assert_includes para.content, %(<span class="'role'">)
      end
    end

"#
        );

        let mut p = Parser::default();
        let maw =
            crate::blocks::Block::parse(crate::Span::new("['role']\\++This++++++++++++"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "['role']\\++This++++++++++++",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "+This+",
                },
                source: Span {
                    data: "['role']\\++This++++++++++++",
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
            },)
        );
    }

    #[test]
    fn should_not_crash_if_role_on_passthrough_is_enclosed_in_quotes_2() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("['role']\\+++++++++This++++++++++++"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "['role']\\+++++++++This++++++++++++",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "++This+",
                },
                source: Span {
                    data: "['role']\\+++++++++This++++++++++++",
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
            },)
        );
    }

    #[test]
    fn should_allow_inline_double_plus_passthrough_to_be_escaped_using_backslash() {
        verifies!(
            r#"
    test 'should allow inline double plus passthrough to be escaped using backslash' do
      para = block_from_string("you need to replace `int a = n#{BACKSLASH}++;` with `int a = ++n;`!")
      result = para.apply_subs para.source
      assert_equal 'you need to replace <code>int a = n++;</code> with <code>int a = ++n;</code>!', result
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("you need to replace `int a = n\\++;` with `int a = ++n;`!"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "you need to replace `int a = n\\++;` with `int a = ++n;`!",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "you need to replace <code>int a = n++;</code> with <code>int a = ++n;</code>!",
                },
                source: Span {
                    data: "you need to replace `int a = n\\++;` with `int a = ++n;`!",
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
            },)
        );
    }

    #[test]
    fn should_allow_inline_double_plus_passthrough_with_attributes_to_be_escaped_using_backslash() {
        verifies!(
            r#"
    test 'should allow inline double plus passthrough with attributes to be escaped using backslash' do
      para = block_from_string("=[attrs]#{BACKSLASH}#{BACKSLASH}++text++")
      result = para.apply_subs para.source
      assert_equal '=[attrs]++text++', result
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new(r#"=[attrs]\\++text++"#), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"=[attrs]\\++text++"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "=[attrs]++text++",
                },
                source: Span {
                    data: r#"=[attrs]\\++text++"#,
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
            },)
        );
    }

    #[test]
    fn collect_multiline_inline_double_dollar_passthroughs() {
        verifies!(
            r#"
    test 'collect multi-line inline double dollar passthroughs' do
      para = block_from_string("$$<code>\n{code}\n</code>$$")
      result = para.extract_passthroughs(para.source)
      passthroughs = para.instance_variable_get :@passthroughs
      assert_equal Asciidoctor::Substitutors::PASS_START + '0' + Asciidoctor::Substitutors::PASS_END, result
      assert_equal 1, passthroughs.size
      assert_equal "<code>\n{code}\n</code>", passthroughs[0][:text]
      assert_equal [:specialcharacters], passthroughs[0][:subs]
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new("$$<code>\n{code}\n</code>$$"));

        let pt = Passthroughs::extract_from(&mut content, &Parser::default());

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "$$<code>\n{code}\n</code>$$",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "\u{96}0\u{97}",
            }
        );

        assert_eq!(
            pt,
            Passthroughs(vec![Passthrough {
                text: "<code>\n{code}\n</code>".to_owned(),
                subs: SubstitutionGroup::Verbatim,
                type_: None,
                attrlist: None,
            },],)
        );
    }

    #[test]
    fn collect_multiline_inline_double_plus_passthroughs() {
        verifies!(
            r#"
    test 'collect multi-line inline double plus passthroughs' do
      para = block_from_string("++<code>\n{code}\n</code>++")
      result = para.extract_passthroughs(para.source)
      passthroughs = para.instance_variable_get :@passthroughs
      assert_equal Asciidoctor::Substitutors::PASS_START + '0' + Asciidoctor::Substitutors::PASS_END, result
      assert_equal 1, passthroughs.size
      assert_equal "<code>\n{code}\n</code>", passthroughs[0][:text]
      assert_equal [:specialcharacters], passthroughs[0][:subs]
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new("++<code>\n{code}\n</code>++"));

        let pt = Passthroughs::extract_from(&mut content, &Parser::default());

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "++<code>\n{code}\n</code>++",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "\u{96}0\u{97}",
            }
        );

        assert_eq!(
            pt,
            Passthroughs(vec![Passthrough {
                text: "<code>\n{code}\n</code>".to_owned(),
                subs: SubstitutionGroup::Verbatim,
                type_: None,
                attrlist: None,
            },],)
        );
    }

    #[test]
    fn collect_passthroughs_from_inline_pass_macro() {
        verifies!(
            r#"
    test 'collect passthroughs from inline pass macro' do
      para = block_from_string(%Q{pass:specialcharacters,quotes[<code>['code'\\]</code>]})
      result = para.extract_passthroughs(para.source)
      passthroughs = para.instance_variable_get :@passthroughs
      assert_equal Asciidoctor::Substitutors::PASS_START + '0' + Asciidoctor::Substitutors::PASS_END, result
      assert_equal 1, passthroughs.size
      assert_equal %q{<code>['code']</code>}, passthroughs[0][:text]
      assert_equal [:specialcharacters, :quotes], passthroughs[0][:subs]
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(
            "pass:specialcharacters,quotes[<code>['code'\\]</code>]",
        ));

        let pt = Passthroughs::extract_from(&mut content, &Parser::default());

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "pass:specialcharacters,quotes[<code>['code'\\]</code>]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "\u{96}0\u{97}",
            }
        );

        assert_eq!(
            pt,
            Passthroughs(vec![Passthrough {
                text: "<code>['code']</code>".to_owned(),
                subs: SubstitutionGroup::Custom(vec![
                    SubstitutionStep::SpecialCharacters,
                    SubstitutionStep::Quotes,
                ]),
                type_: None,
                attrlist: None,
            },],)
        );
    }

    #[test]
    fn collect_multiline_passthroughs_from_inline_pass_macro() {
        verifies!(
            r#"
    test 'collect multi-line passthroughs from inline pass macro' do
      para = block_from_string(%Q{pass:specialcharacters,quotes[<code>['more\ncode'\\]</code>]})
      result = para.extract_passthroughs(para.source)
      passthroughs = para.instance_variable_get :@passthroughs
      assert_equal Asciidoctor::Substitutors::PASS_START + '0' + Asciidoctor::Substitutors::PASS_END, result
      assert_equal 1, passthroughs.size
      assert_equal %Q{<code>['more\ncode']</code>}, passthroughs[0][:text]
      assert_equal [:specialcharacters, :quotes], passthroughs[0][:subs]
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(
            "pass:specialcharacters,quotes[<code>['more\ncode'\\]</code>]",
        ));

        let pt = Passthroughs::extract_from(&mut content, &Parser::default());

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "pass:specialcharacters,quotes[<code>['more\ncode'\\]</code>]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "\u{96}0\u{97}",
            }
        );

        assert_eq!(
            pt,
            Passthroughs(vec![Passthrough {
                text: "<code>['more\ncode']</code>".to_owned(),
                subs: SubstitutionGroup::Custom(vec![
                    SubstitutionStep::SpecialCharacters,
                    SubstitutionStep::Quotes,
                ]),
                type_: None,
                attrlist: None,
            },],)
        );
    }

    #[ignore]
    #[test]
    fn should_find_and_replace_placeholder_duplicated_by_substitution() {
        verifies!(
            r#"
    test 'should find and replace placeholder duplicated by substitution' do
      input = %q(+first passthrough+ followed by link:$$http://example.com/__u_no_format_me__$$[] with passthrough)
      result = convert_inline_string input
      assert_equal 'first passthrough followed by <a href="http://example.com/__u_no_format_me__" class="bare">http://example.com/__u_no_format_me__</a> with passthrough', result
    end

"#
        );

        // TO DO: Restore test when macros are supported.

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new(
                "+first passthrough+ followed by link:$$http://example.com/__u_no_format_me__$$[] with passthrough",
            ),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "+first passthrough+ followed by link:$$http://example.com/__u_no_format_me__$$[] with passthrough",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"first passthrough followed by <a href="http://example.com/__u_no_format_me__" class="bare">http://example.com/__u_no_format_me__</a> with passthrough"#,
                },
                source: Span {
                    data: "+first passthrough+ followed by link:$$http://example.com/__u_no_format_me__$$[] with passthrough",
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
            },)
        );
    }

    #[test]
    fn resolves_sub_shorthands_on_inline_pass_macro() {
        verifies!(
            r#"
    test 'resolves sub shorthands on inline pass macro' do
      para = block_from_string 'pass:q,a[*<{backend}>*]'
      result = para.extract_passthroughs para.source
      passthroughs = para.instance_variable_get :@passthroughs
      assert_equal 1, passthroughs.size
      assert_equal [:quotes, :attributes], passthroughs[0][:subs]
      result = para.restore_passthroughs result
      assert_equal '<strong><html5></strong>', result
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new("pass:q,a[*<{backend}>*]"));

        let pt = Passthroughs::extract_from(&mut content, &Parser::default());

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "pass:q,a[*<{backend}>*]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "\u{96}0\u{97}",
            }
        );

        assert_eq!(
            pt,
            Passthroughs(vec![Passthrough {
                text: "*<{backend}>*".to_owned(),
                subs: SubstitutionGroup::Custom(vec![
                    SubstitutionStep::Quotes,
                    SubstitutionStep::AttributeReferences,
                ]),
                type_: None,
                attrlist: None,
            },],)
        );

        let parser = Parser::default().with_intrinsic_attribute(
            "backend",
            "html5",
            ModificationContext::ApiOnly,
        );

        pt.0[0].subs.apply(&mut content, &parser, None);

        pt.restore_to(&mut content, &parser);

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "pass:q,a[*<{backend}>*]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "<strong><html5></strong>",
            }
        );
    }

    #[ignore]
    #[test]
    fn inline_pass_macro_supports_incremental_subs() {
        verifies!(
            r#"
    test 'inline pass macro supports incremental subs' do
      para = block_from_string 'pass:n,-a[<{backend}>]'
      result = para.extract_passthroughs para.source
      passthroughs = para.instance_variable_get :@passthroughs
      assert_equal 1, passthroughs.size
      result = para.restore_passthroughs result
      assert_equal '&lt;{backend}&gt;', result
    end

"#
        );

        // TO DO: Restore this test once macro substitutions are implemented.
        let mut content = crate::content::Content::from(crate::Span::new("pass:n,-a[<{backend}>]"));
        let pt = Passthroughs::extract_from(&mut content, &Parser::default());

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "pass:n,-a[<{backend}>]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "\u{96}0\u{97}",
            }
        );

        assert_eq!(
            pt,
            Passthroughs(vec![Passthrough {
                text: "<{backend}>".to_owned(),
                subs: SubstitutionGroup::Custom(vec![
                    SubstitutionStep::SpecialCharacters,
                    SubstitutionStep::Quotes,
                    SubstitutionStep::CharacterReplacements,
                    SubstitutionStep::Macros,
                    SubstitutionStep::PostReplacement,
                ]),
                type_: None,
                attrlist: None,
            },],)
        );

        let parser = Parser::default().with_intrinsic_attribute(
            "backend",
            "html5",
            ModificationContext::ApiOnly,
        );

        pt.0[0].subs.apply(&mut content, &parser, None);

        pt.restore_to(&mut content, &parser);

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "pass:q,a[*<{backend}>*]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "&lt;{backend}&gt;",
            }
        );
    }

    #[test]
    fn should_not_recognize_pass_macro_with_invalid_substitution_list_1() {
        verifies!(
            r#"
    test 'should not recognize pass macro with invalid substitution list' do
      [',', '42', 'a,'].each do |subs|
        para = block_from_string %(pass:#{subs}[foobar])
        result = para.extract_passthroughs para.source
        assert_equal %(pass:#{subs}[foobar]), result
      end
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new("pass:,[foobar]"));
        let pt = Passthroughs::extract_from(&mut content, &Parser::default());

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "pass:,[foobar]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "pass:,[foobar]",
            }
        );

        assert!(pt.0.is_empty());
    }

    #[test]
    fn should_not_recognize_pass_macro_with_invalid_substitution_list_2() {
        let mut content = crate::content::Content::from(crate::Span::new("pass:42[foobar]"));
        let pt = Passthroughs::extract_from(&mut content, &Parser::default());

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "pass:42[foobar]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "pass:42[foobar]",
            }
        );

        assert!(pt.0.is_empty());
    }

    #[test]
    fn should_not_recognize_pass_macro_with_invalid_substitution_list_3() {
        let mut content = crate::content::Content::from(crate::Span::new("pass:a,[foobar]"));
        let pt = Passthroughs::extract_from(&mut content, &Parser::default());

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "pass:a,[foobar]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "pass:a,[foobar]",
            }
        );

        assert!(pt.0.is_empty());
    }

    #[test]
    fn should_warn_if_substitutions_on_pass_macro_are_invalid() {
        verifies!(
            r#"
    test 'should warn if substitutions on pass macro are invalid' do
      subs = 'bogus'
      input = %(pass:#{subs}[++])
      using_memory_logger do |logger|
        para = block_from_string input, attributes: { 'stem' => 'asciimath' }
        assert_equal '++', para.content
        assert_message logger, :WARN, %(invalid substitution type for passthrough macro: #{subs})
      end
    end

"#
        );

        let mut p = Parser::default().with_intrinsic_attribute(
            "stem",
            "asciimath",
            ModificationContext::Anywhere,
        );
        let doc = p.parse("pass:bogus[++]");
        assert_eq!(
            doc.nested_blocks().next().unwrap().rendered_content(),
            Some("++")
        );

        let warnings: Vec<_> = doc.warnings().collect();
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::InvalidSubstitutionTypeForPassthroughMacro("bogus".to_string())
        );
    }

    #[test]
    fn should_allow_content_of_inline_pass_macro_to_be_empty() {
        verifies!(
            r#"
    test 'should allow content of inline pass macro to be empty' do
      para = block_from_string 'pass:[]'
      result = para.extract_passthroughs para.source
      passthroughs = para.instance_variable_get :@passthroughs
      assert_equal 1, passthroughs.size
      assert_equal '', para.restore_passthroughs(result)
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new("pass:[]"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "pass:[]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "",
                },
                source: Span {
                    data: "pass:[]",
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
            },)
        );
    }

    non_normative!(
        r#"
    # NOTE placeholder is surrounded by text to prevent reader from stripping trailing boundary char (unique to test scenario)
"#
    );

    #[test]
    fn restore_inline_passthroughs_without_subs() {
        verifies!(
            r##"
    test 'restore inline passthroughs without subs' do
      para = block_from_string("some #{Asciidoctor::Substitutors::PASS_START}" + '0' + "#{Asciidoctor::Substitutors::PASS_END} to study")
      para.extract_passthroughs ''
      passthroughs = para.instance_variable_get :@passthroughs
      passthroughs[0] = { text: '<code>inline code</code>', subs: [] }
      result = para.restore_passthroughs(para.source)
      assert_equal "some <code>inline code</code> to study", result
    end

"##
        );

        // NOTE: Placeholder is surrounded by text to prevent reader from stripping
        // trailing boundary char (unique to test scenario).
        let mut content =
            crate::content::Content::from(crate::Span::new("some \u{96}0\u{97} to study"));

        let pt = Passthroughs(vec![Passthrough {
            text: "<code>inline code</code>".to_owned(),
            subs: SubstitutionGroup::None,
            type_: None,
            attrlist: None,
        }]);

        let parser = Parser::default();
        pt.restore_to(&mut content, &parser);

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "some \u{96}0\u{97} to study",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "some <code>inline code</code> to study",
            }
        );
    }

    non_normative!(
        r#"
    # NOTE placeholder is surrounded by text to prevent reader from stripping trailing boundary char (unique to test scenario)
"#
    );

    #[test]
    fn restore_inline_passthroughs_with_subs() {
        verifies!(
            r##"
    test 'restore inline passthroughs with subs' do
      para = block_from_string("some #{Asciidoctor::Substitutors::PASS_START}" + '0' + "#{Asciidoctor::Substitutors::PASS_END} to study in the #{Asciidoctor::Substitutors::PASS_START}" + '1' + "#{Asciidoctor::Substitutors::PASS_END} programming language")
      para.extract_passthroughs ''
      passthroughs = para.instance_variable_get :@passthroughs
      passthroughs[0] = { text: '<code>{code}</code>', subs: [:specialcharacters] }
      passthroughs[1] = { text: '{language}', subs: [:specialcharacters] }
      result = para.restore_passthroughs(para.source)
      assert_equal 'some &lt;code&gt;{code}&lt;/code&gt; to study in the {language} programming language', result
    end

"##
        );

        // NOTE: Placeholder is surrounded by text to prevent reader from stripping
        // trailing boundary char (unique to test scenario).
        let mut content = crate::content::Content::from(crate::Span::new(
            "some \u{96}0\u{97} to study in the \u{96}1\u{97} programming language",
        ));

        let pt = Passthroughs(vec![
            Passthrough {
                text: "<code>{code}</code>".to_owned(),
                subs: SubstitutionGroup::Custom(vec![SubstitutionStep::SpecialCharacters]),
                type_: None,
                attrlist: None,
            },
            Passthrough {
                text: "{language}".to_owned(),
                subs: SubstitutionGroup::Custom(vec![SubstitutionStep::SpecialCharacters]),
                type_: None,
                attrlist: None,
            },
        ]);

        let parser = Parser::default();
        pt.restore_to(&mut content, &parser);

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "some \u{96}0\u{97} to study in the \u{96}1\u{97} programming language",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "some &lt;code&gt;{code}&lt;/code&gt; to study in the {language} programming language",
            }
        );
    }

    #[test]
    fn should_restore_nested_passthroughs() {
        verifies!(
            r#"
    test 'should restore nested passthroughs' do
      result = convert_inline_string %q(+Sometimes you feel pass:q[`mono`].+ Sometimes you +$$don't$$+.)
      assert_equal %q(Sometimes you feel <code>mono</code>. Sometimes you don't.), result
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("+Sometimes you feel pass:q[`mono`].+ Sometimes you +$$don't$$+."),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "+Sometimes you feel pass:q[`mono`].+ Sometimes you +$$don't$$+.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "Sometimes you feel <code>mono</code>. Sometimes you don't.",
                },
                source: Span {
                    data: "+Sometimes you feel pass:q[`mono`].+ Sometimes you +$$don't$$+.",
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
            },)
        );
    }

    #[ignore]
    #[test]
    fn should_not_fail_to_restore_remaining_passthroughs_after_processing_inline_passthrough_with_macro_substitution()
     {
        verifies!(
            r#"
    test 'should not fail to restore remaining passthroughs after processing inline passthrough with macro substitution' do
      input = 'pass:m[.] pass:[.]'
      assert_equal '. .', (convert_inline_string input)
    end

"#
        );

        // TO DO: Enable this test when macro substitution is implemented.
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new("pass:m[.] pass:[.]"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "pass:m[.] pass:[.]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: ". .",
                },
                source: Span {
                    data: "pass:m[.] pass:[.]",
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
            },)
        );
    }

    #[test]
    fn should_honor_role_on_double_plus_passthrough() {
        verifies!(
            r#"
    test 'should honor role on double plus passthrough' do
      result = convert_inline_string 'Print the version using [var]++{asciidoctor-version}++.'
      assert_equal 'Print the version using <span class="var">{asciidoctor-version}</span>.', result
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("Print the version using [var]++{asciidoctor-version}++."),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "Print the version using [var]++{asciidoctor-version}++.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"Print the version using <span class="var">{asciidoctor-version}</span>."#,
                },
                source: Span {
                    data: "Print the version using [var]++{asciidoctor-version}++.",
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
            },)
        );
    }

    #[test]
    fn should_honor_role_on_double_dollar_passthrough() {
        // NOTE: Not in the Ruby test suite.
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("Print the version using [var]$${asciidoctor-version}$$."),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "Print the version using [var]$${asciidoctor-version}$$.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"Print the version using <span class="var">{asciidoctor-version}</span>."#,
                },
                source: Span {
                    data: "Print the version using [var]$${asciidoctor-version}$$.",
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
            },)
        );
    }

    #[test]
    fn complex_inline_passthrough_macro_1() {
        verifies!(
            r#"
    test 'complex inline passthrough macro' do
      text_to_escape = %q{[(] <'basic form'> <'logical operator'> <'basic form'> [)]}
      para = block_from_string %($$#{text_to_escape}$$)
      para.extract_passthroughs(para.source)
      passthroughs = para.instance_variable_get :@passthroughs
      assert_equal 1, passthroughs.size
      assert_equal text_to_escape, passthroughs[0][:text]

      text_to_escape_escaped = %q{[(\] <'basic form'> <'logical operator'> <'basic form'> [)\]}
      para = block_from_string %(pass:specialcharacters[#{text_to_escape_escaped}])
      para.extract_passthroughs(para.source)
      passthroughs = para.instance_variable_get :@passthroughs
      assert_equal 1, passthroughs.size
      assert_equal text_to_escape, passthroughs[0][:text]
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(
            "$$[(] <'basic form'> <'logical operator'> <'basic form'> [)]$$",
        ));

        let pt = Passthroughs::extract_from(&mut content, &Parser::default());

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "$$[(] <'basic form'> <'logical operator'> <'basic form'> [)]$$",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "\u{96}0\u{97}",
            }
        );

        assert_eq!(
            pt,
            Passthroughs(vec![Passthrough {
                text: "[(] <'basic form'> <'logical operator'> <'basic form'> [)]".to_owned(),
                subs: SubstitutionGroup::Verbatim,
                type_: None,
                attrlist: None,
            },],)
        );
    }

    #[test]
    fn complex_inline_passthrough_macro_2() {
        let mut content = crate::content::Content::from(crate::Span::new(
            r#"pass:specialcharacters[[(\] <'basic form'> <'logical operator'> <'basic form'> [)\]]"#,
        ));

        let pt = Passthroughs::extract_from(&mut content, &Parser::default());

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: r#"pass:specialcharacters[[(\] <'basic form'> <'logical operator'> <'basic form'> [)\]]"#,
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "\u{96}0\u{97}",
            }
        );

        assert_eq!(
            pt,
            Passthroughs(vec![Passthrough {
                text: r#"[(] <'basic form'> <'logical operator'> <'basic form'> [)]"#.to_owned(),
                subs: SubstitutionGroup::Custom(vec![SubstitutionStep::SpecialCharacters,],),
                type_: None,
                attrlist: None,
            },],)
        );
    }

    #[test]
    fn inline_pass_macro_with_a_composite_sub() {
        verifies!(
            r#"
    test 'inline pass macro with a composite sub' do
      para = block_from_string %(pass:verbatim[<{backend}>])
      assert_equal '&lt;{backend}&gt;', para.content
    end

"#
        );

        let mut p = Parser::default();
        let maw =
            crate::blocks::Block::parse(crate::Span::new("pass:verbatim[<{backend}>]"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "pass:verbatim[<{backend}>]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"&lt;{backend}&gt;"#,
                },
                source: Span {
                    data: "pass:verbatim[<{backend}>]",
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
            },)
        );
    }

    #[test]
    fn should_support_constrained_passthrough_in_middle_of_monospace_span() {
        verifies!(
            r#"
    test 'should support constrained passthrough in middle of monospace span' do
      input = 'a `foo +bar+ baz` kind of thing'
      para = block_from_string input
      assert_equal 'a <code>foo bar baz</code> kind of thing', para.content
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("a `foo +bar+ baz` kind of thing"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "a `foo +bar+ baz` kind of thing",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "a <code>foo bar baz</code> kind of thing",
                },
                source: Span {
                    data: "a `foo +bar+ baz` kind of thing",
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
            },)
        );
    }

    #[test]
    fn should_support_constrained_passthrough_in_monospace_span_preceded_by_escaped_boxed_attrlist_with_transitional_role()
     {
        verifies!(
            r#"
    test 'should support constrained passthrough in monospace span preceded by escaped boxed attrlist with transitional role' do
      input = %(#{BACKSLASH}[x-]`foo +bar+ baz`)
      para = block_from_string input
      assert_equal '[x-]<code>foo bar baz</code>', para.content
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new(r#"\[x-]`foo +bar+ baz`"#), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"\[x-]`foo +bar+ baz`"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "[x-]<code>foo bar baz</code>",
                },
                source: Span {
                    data: r#"\[x-]`foo +bar+ baz`"#,
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
            },)
        );
    }

    #[test]
    fn should_treat_monospace_phrase_with_escaped_boxed_attrlist_with_transitional_role_as_monospace()
     {
        verifies!(
            r#"
    test 'should treat monospace phrase with escaped boxed attrlist with transitional role as monospace' do
      input = %(#{BACKSLASH}[x-]`*foo* +bar+ baz`)
      para = block_from_string input
      assert_equal '[x-]<code><strong>foo</strong> bar baz</code>', para.content
    end

"#
        );

        let mut p = Parser::default();
        let maw =
            crate::blocks::Block::parse(crate::Span::new(r#"\[x-]`*foo* +bar+ baz`"#), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"\[x-]`*foo* +bar+ baz`"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "[x-]<code><strong>foo</strong> bar baz</code>",
                },
                source: Span {
                    data: r#"\[x-]`*foo* +bar+ baz`"#,
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
            },)
        );
    }

    #[test]
    fn should_ignore_escaped_attrlist_with_transitional_role_on_monospace_phrase_if_not_proceeded_by_bracket()
     {
        verifies!(
            r#"
    test 'should ignore escaped attrlist with transitional role on monospace phrase if not proceeded by [' do
      input = %(#{BACKSLASH}x-]`*foo* +bar+ baz`)
      para = block_from_string input
      assert_equal %(#{BACKSLASH}x-]<code><strong>foo</strong> bar baz</code>), para.content
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new(r#"\x-]`*foo* +bar+ baz`"#), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"\x-]`*foo* +bar+ baz`"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"\x-]<code><strong>foo</strong> bar baz</code>"#,
                },
                source: Span {
                    data: r#"\x-]`*foo* +bar+ baz`"#,
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
            },)
        );
    }

    #[test]
    fn should_not_process_passthrough_inside_transitional_literal_monospace_span() {
        verifies!(
            r#"
    test 'should not process passthrough inside transitional literal monospace span' do
      input = 'a [x-]`foo +bar+ baz` kind of thing'
      para = block_from_string input
      assert_equal 'a <code>foo +bar+ baz</code> kind of thing', para.content
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("a [x-]`foo +bar+ baz` kind of thing"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "a [x-]`foo +bar+ baz` kind of thing",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "a <code>foo +bar+ baz</code> kind of thing",
                },
                source: Span {
                    data: "a [x-]`foo +bar+ baz` kind of thing",
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
            },)
        );
    }

    #[test]
    fn should_support_constrained_passthrough_in_monospace_phrase_with_attrlist() {
        verifies!(
            r#"
    test 'should support constrained passthrough in monospace phrase with attrlist' do
      input = '[.role]`foo +bar+ baz`'
      para = block_from_string input
      assert_equal '<code class="role">foo bar baz</code>', para.content
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new("[.role]`foo +bar+ baz`"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "[.role]`foo +bar+ baz`",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<code class="role">foo bar baz</code>"#,
                },
                source: Span {
                    data: "[.role]`foo +bar+ baz`",
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
            },)
        );
    }

    #[test]
    fn should_support_attrlist_on_a_literal_monospace_phrase() {
        verifies!(
            r#"
    test 'should support attrlist on a literal monospace phrase' do
      input = '[.baz]`+foo--bar+`'
      para = block_from_string input
      assert_equal '<code class="baz">foo--bar</code>', para.content
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new("[.baz]`+foo--bar+`"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "[.baz]`+foo--bar+`",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<code class="baz">foo--bar</code>"#,
                },
                source: Span {
                    data: "[.baz]`+foo--bar+`",
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
            },)
        );
    }

    #[test]
    fn should_not_process_an_escaped_passthrough_macro_inside_a_monospaced_phrase() {
        verifies!(
            r#"
    test 'should not process an escaped passthrough macro inside a monospaced phrase' do
      input = 'use the `\pass:c[]` macro'
      para = block_from_string input
      assert_equal 'use the <code>pass:c[]</code> macro', para.content
    end

"#
        );

        let mut p = Parser::default();
        let maw =
            crate::blocks::Block::parse(crate::Span::new(r#"use the `\pass:c[]` macro"#), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"use the `\pass:c[]` macro"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "use the <code>pass:c[]</code> macro",
                },
                source: Span {
                    data: r#"use the `\pass:c[]` macro"#,
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
            },)
        );
    }

    #[test]
    fn should_not_process_an_escaped_passthrough_macro_inside_a_monospaced_phrase_with_attributes()
    {
        verifies!(
            r#"
    test 'should not process an escaped passthrough macro inside a monospaced phrase with attributes' do
      input = 'use the [syntax]`\pass:c[]` macro'
      para = block_from_string input
      assert_equal 'use the <code class="syntax">pass:c[]</code> macro', para.content
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new(r#"use the [syntax]`\pass:c[]` macro"#),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"use the [syntax]`\pass:c[]` macro"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"use the <code class="syntax">pass:c[]</code> macro"#,
                },
                source: Span {
                    data: r#"use the [syntax]`\pass:c[]` macro"#,
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
            },)
        );
    }

    #[test]
    fn should_honor_an_escaped_single_plus_passthrough_inside_a_monospaced_phrase() {
        verifies!(
            r#"
    test 'should honor an escaped single plus passthrough inside a monospaced phrase' do
      input = 'use `\+{author}+` to show an attribute reference'
      para = block_from_string input, attributes: { 'author' => 'Dan' }
      assert_equal 'use <code>+Dan+</code> to show an attribute reference', para.content
    end

"#
        );

        let mut p = Parser::default().with_intrinsic_attribute(
            "author",
            "Dan",
            ModificationContext::Anywhere,
        );

        let maw = crate::blocks::Block::parse(
            crate::Span::new(r#"use `\+{author}+` to show an attribute reference"#),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"use `\+{author}+` to show an attribute reference"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"use <code>+Dan+</code> to show an attribute reference"#,
                },
                source: Span {
                    data: r#"use `\+{author}+` to show an attribute reference"#,
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
            },)
        );
    }

    non_normative!(
        r#"
    context 'Math macros' do
"#
    );

    mod math_macros {
        use crate::tests::prelude::*;

        /// Assert that the first block of `input` (parsed with `parser`) has
        /// the given rendered content (the equivalent of Asciidoctor's
        /// `para.content`) and that no warnings were raised.
        fn assert_content(parser: Parser, input: &str, expected: &str) {
            let mut parser = parser;
            let doc = parser.parse(input);
            assert_eq!(
                doc.nested_blocks().next().unwrap().rendered_content(),
                Some(expected),
                "input = {input:?}"
            );
            assert!(doc.warnings().next().is_none(), "input = {input:?}");
        }

        #[test]
        fn should_passthrough_text_in_asciimath_macro_and_surround_with_asciimath_delimiters() {
            verifies!(
                r#"
      test 'should passthrough text in asciimath macro and surround with AsciiMath delimiters' do
        using_memory_logger do |logger|
          input = 'asciimath:[x/x={(1,if x!=0),(text{undefined},if x=0):}]'
          para = block_from_string input, attributes: { 'attribute-missing' => 'warn' }
          assert_equal '\$x/x={(1,if x!=0),(text{undefined},if x=0):}\$', para.content
          assert logger.empty?
        end
      end

"#
            );

            assert_content(
                Parser::default(),
                "asciimath:[x/x={(1,if x!=0),(text{undefined},if x=0):}]",
                r"\$x/x={(1,if x!=0),(text{undefined},if x=0):}\$",
            );
        }

        #[test]
        fn should_not_recognize_asciimath_macro_with_no_content() {
            verifies!(
                r#"
      test 'should not recognize asciimath macro with no content' do
        input = 'asciimath:[]'
        para = block_from_string input
        assert_equal 'asciimath:[]', para.content
      end

"#
            );

            assert_content(Parser::default(), "asciimath:[]", "asciimath:[]");
        }

        #[test]
        fn should_perform_specialcharacters_subs_on_asciimath_macro_content_in_html_backend_by_default()
         {
            verifies!(
                r#"
      test 'should perform specialcharacters subs on asciimath macro content in html backend by default' do
        input = 'asciimath:[a < b]'
        para = block_from_string input
        assert_equal '\$a &lt; b\$', para.content
      end

"#
            );

            assert_content(Parser::default(), "asciimath:[a < b]", r"\$a &lt; b\$");
        }

        // Not ported to this crate:
        // - 'should convert contents of asciimath macro to MathML in DocBook output if
        //   asciimath gem is available': DocBook backend / asciimath gem not supported.
        // - 'should not perform specialcharacters subs on asciimath macro content in
        //   Docbook output if asciimath gem not available': DocBook backend / asciimath
        //   gem not supported.
        non_normative!(
            r#"
      test 'should convert contents of asciimath macro to MathML in DocBook output if asciimath gem is available' do
        asciimath_available = !(Asciidoctor::Helpers.require_library 'asciimath', true, :ignore).nil?
        input = 'asciimath:[a < b]'
        expected = '<inlineequation><mml:math xmlns:mml="http://www.w3.org/1998/Math/MathML"><mml:mi>a</mml:mi><mml:mo>&lt;</mml:mo><mml:mi>b</mml:mi></mml:math></inlineequation>'
        using_memory_logger do |logger|
          para = block_from_string input, backend: :docbook
          actual = para.content
          if asciimath_available
            assert_equal expected, actual
            assert_equal :loaded, para.document.converter.instance_variable_get(:@asciimath_status)
          else
            assert_message logger, :WARN, 'optional gem \'asciimath\' is not available. Functionality disabled.'
            assert_equal :unavailable, para.document.converter.instance_variable_get(:@asciimath_status)
          end
        end
      end

      test 'should not perform specialcharacters subs on asciimath macro content in Docbook output if asciimath gem not available' do
        asciimath_available = !(Asciidoctor::Helpers.require_library 'asciimath', true, :ignore).nil?
        input = 'asciimath:[a < b]'
        para = block_from_string input, backend: :docbook
        para.document.converter.instance_variable_set :@asciimath_status, :unavailable
        if asciimath_available
          old_asciimath = ::AsciiMath
          Object.send :remove_const, 'AsciiMath'
        end
        assert_equal '<inlineequation><mathphrase><![CDATA[a < b]]></mathphrase></inlineequation>', para.content
        ::AsciiMath = old_asciimath if asciimath_available
      end

"#
        );

        #[test]
        fn should_honor_explicit_subslist_on_asciimath_macro() {
            verifies!(
                r#"
      test 'should honor explicit subslist on asciimath macro' do
        input = 'asciimath:attributes[{expr}]'
        para = block_from_string input, attributes: { 'expr' => 'x != 0' }
        assert_equal '\$x != 0\$', para.content
      end

"#
            );

            let p = Parser::default().with_intrinsic_attribute(
                "expr",
                "x != 0",
                ModificationContext::Anywhere,
            );
            assert_content(p, "asciimath:attributes[{expr}]", r"\$x != 0\$");
        }

        #[test]
        fn should_passthrough_text_in_latexmath_macro_and_surround_with_latex_math_delimiters() {
            verifies!(
                r#"
      test 'should passthrough text in latexmath macro and surround with LaTeX math delimiters' do
        input = 'latexmath:[C = \alpha + \beta Y^{\gamma} + \epsilon]'
        para = block_from_string input
        assert_equal '\(C = \alpha + \beta Y^{\gamma} + \epsilon\)', para.content
      end

"#
            );

            assert_content(
                Parser::default(),
                r"latexmath:[C = \alpha + \beta Y^{\gamma} + \epsilon]",
                r"\(C = \alpha + \beta Y^{\gamma} + \epsilon\)",
            );
        }

        #[test]
        fn should_strip_legacy_latex_math_delimiters_around_latexmath_content_if_present() {
            verifies!(
                r#"
      test 'should strip legacy LaTeX math delimiters around latexmath content if present' do
        input = 'latexmath:[$C = \alpha + \beta Y^{\gamma} + \epsilon$]'
        para = block_from_string input
        assert_equal '\(C = \alpha + \beta Y^{\gamma} + \epsilon\)', para.content
      end

"#
            );

            assert_content(
                Parser::default(),
                r"latexmath:[$C = \alpha + \beta Y^{\gamma} + \epsilon$]",
                r"\(C = \alpha + \beta Y^{\gamma} + \epsilon\)",
            );
        }

        #[test]
        fn should_not_recognize_latexmath_macro_with_no_content() {
            verifies!(
                r#"
      test 'should not recognize latexmath macro with no content' do
        input = 'latexmath:[]'
        para = block_from_string input
        assert_equal 'latexmath:[]', para.content
      end

"#
            );

            assert_content(Parser::default(), "latexmath:[]", "latexmath:[]");
        }

        #[test]
        fn should_unescape_escaped_square_bracket_in_equation() {
            verifies!(
                r#"
      test 'should unescape escaped square bracket in equation' do
        input = 'latexmath:[\sqrt[3\]{x}]'
        para = block_from_string input
        assert_equal '\(\sqrt[3]{x}\)', para.content
      end

"#
            );

            assert_content(
                Parser::default(),
                r"latexmath:[\sqrt[3\]{x}]",
                r"\(\sqrt[3]{x}\)",
            );
        }

        #[test]
        fn should_perform_specialcharacters_subs_on_latexmath_macro_in_html_backend_by_default() {
            verifies!(
                r#"
      test 'should perform specialcharacters subs on latexmath macro in html backend by default' do
        input = 'latexmath:[a < b]'
        para = block_from_string input
        assert_equal '\(a &lt; b\)', para.content
      end

"#
            );

            assert_content(Parser::default(), "latexmath:[a < b]", r"\(a &lt; b\)");
        }

        // Not ported to this crate:
        // - 'should not perform specialcharacters subs on latexmath macro content in
        //   docbook backend by default': DocBook backend not supported.
        non_normative!(
            r#"
      test 'should not perform specialcharacters subs on latexmath macro content in docbook backend by default' do
        input = 'latexmath:[a < b]'
        para = block_from_string input, backend: :docbook
        assert_equal '<inlineequation><alt><![CDATA[a < b]]></alt><mathphrase><![CDATA[a < b]]></mathphrase></inlineequation>', para.content
      end

"#
        );

        #[test]
        fn should_honor_explicit_subslist_on_latexmath_macro() {
            verifies!(
                r#"
      test 'should honor explicit subslist on latexmath macro' do
        input = 'latexmath:attributes[{expr}]'
        para = block_from_string input, attributes: { 'expr' => '\sqrt{4} = 2' }
        assert_equal '\(\sqrt{4} = 2\)', para.content
      end

"#
            );

            let p = Parser::default().with_intrinsic_attribute(
                "expr",
                r"\sqrt{4} = 2",
                ModificationContext::Anywhere,
            );
            assert_content(p, "latexmath:attributes[{expr}]", r"\(\sqrt{4} = 2\)");
        }

        #[test]
        fn should_passthrough_math_macro_inside_another_passthrough() {
            verifies!(
                r#"
      test 'should passthrough math macro inside another passthrough' do
        input = 'the text `asciimath:[x = y]` should be passed through as +literal+ text'
        para = block_from_string input, attributes: { 'compat-mode' => '' }
        assert_equal 'the text <code>asciimath:[x = y]</code> should be passed through as <code>literal</code> text', para.content

        input = 'the text [x-]`asciimath:[x = y]` should be passed through as `literal` text'
        para = block_from_string input
        assert_equal 'the text <code>asciimath:[x = y]</code> should be passed through as <code>literal</code> text', para.content

        input = 'the text `+asciimath:[x = y]+` should be passed through as `literal` text'
        para = block_from_string input
        assert_equal 'the text <code>asciimath:[x = y]</code> should be passed through as <code>literal</code> text', para.content
      end

"#
            );

            assert_content(
                Parser::default(),
                "the text [x-]`asciimath:[x = y]` should be passed through as `literal` text",
                "the text <code>asciimath:[x = y]</code> should be passed through as <code>literal</code> text",
            );

            assert_content(
                Parser::default(),
                "the text `+asciimath:[x = y]+` should be passed through as `literal` text",
                "the text <code>asciimath:[x = y]</code> should be passed through as <code>literal</code> text",
            );
        }

        #[test]
        fn should_not_recognize_stem_macro_with_no_content() {
            verifies!(
                r#"
      test 'should not recognize stem macro with no content' do
        input = 'stem:[]'
        para = block_from_string input
        assert_equal input, para.content
      end

"#
            );

            assert_content(Parser::default(), "stem:[]", "stem:[]");
        }

        #[test]
        fn should_passthrough_text_in_stem_macro_and_surround_with_asciimath_delimiters_by_default()
        {
            verifies!(
                r#"
      test 'should passthrough text in stem macro and surround with AsciiMath delimiters if stem attribute is asciimath, empty, or not set' do
        [
          {},
          { 'stem' => '' },
          { 'stem' => 'asciimath' },
          { 'stem' => 'bogus' },
        ].each do |attributes|
          using_memory_logger do |logger|
            input = 'stem:[x/x={(1,if x!=0),(text{undefined},if x=0):}]'
            para = block_from_string input, attributes: (attributes.merge 'attribute-missing' => 'warn')
            assert_equal '\$x/x={(1,if x!=0),(text{undefined},if x=0):}\$', para.content
            assert logger.empty?
          end
        end
      end

"#
            );

            for stem in ["__unset__", "", "asciimath", "bogus"] {
                let mut p = Parser::default();
                if stem != "__unset__" {
                    p = p.with_intrinsic_attribute("stem", stem, ModificationContext::Anywhere);
                }
                assert_content(
                    p,
                    "stem:[x/x={(1,if x!=0),(text{undefined},if x=0):}]",
                    r"\$x/x={(1,if x!=0),(text{undefined},if x=0):}\$",
                );
            }
        }

        #[test]
        fn should_passthrough_text_in_stem_macro_and_surround_with_latex_math_delimiters() {
            verifies!(
                r#"
      test 'should passthrough text in stem macro and surround with LaTeX math delimiters if stem attribute is latexmath, latex, or tex' do
        [
          { 'stem' => 'latexmath' },
          { 'stem' => 'latex' },
          { 'stem' => 'tex' },
        ].each do |attributes|
          input = 'stem:[C = \alpha + \beta Y^{\gamma} + \epsilon]'
          para = block_from_string input, attributes: attributes
          assert_equal '\(C = \alpha + \beta Y^{\gamma} + \epsilon\)', para.content
        end
      end

"#
            );

            for stem in ["latexmath", "latex", "tex"] {
                let p = Parser::default().with_intrinsic_attribute(
                    "stem",
                    stem,
                    ModificationContext::Anywhere,
                );
                assert_content(
                    p,
                    r"stem:[C = \alpha + \beta Y^{\gamma} + \epsilon]",
                    r"\(C = \alpha + \beta Y^{\gamma} + \epsilon\)",
                );
            }
        }

        #[test]
        fn should_apply_substitutions_specified_on_stem_macro() {
            verifies!(
                r#"
      test 'should apply substitutions specified on stem macro' do
        ['stem:c,a[sqrt(x) <=> {solve-for-x}]', 'stem:n,-r[sqrt(x) <=> {solve-for-x}]'].each do |input|
          para = block_from_string input, attributes: { 'stem' => 'asciimath', 'solve-for-x' => '13' }
          assert_equal '\$sqrt(x) &lt;=&gt; 13\$', para.content
        end
      end

"#
            );

            for input in [
                "stem:c,a[sqrt(x) <=> {solve-for-x}]",
                "stem:n,-r[sqrt(x) <=> {solve-for-x}]",
            ] {
                let p = Parser::default()
                    .with_intrinsic_attribute("stem", "asciimath", ModificationContext::Anywhere)
                    .with_intrinsic_attribute("solve-for-x", "13", ModificationContext::Anywhere);
                assert_content(p, input, r"\$sqrt(x) &lt;=&gt; 13\$");
            }
        }

        #[test]
        fn should_replace_passthroughs_inside_stem_expression() {
            verifies!(
                r#"
      test 'should replace passthroughs inside stem expression' do
        [
          ['stem:[+1+]', '\$1\$'],
          ['stem:[+\infty-(+\infty)]', '\$\infty-(\infty)\$'],
          ['stem:[+++\infty-(+\infty)++]', '\$+\infty-(+\infty)\$'],
        ].each do |input, expected|
          para = block_from_string input, attributes: { 'stem' => '', }
          assert_equal expected, para.content
        end
      end

"#
            );

            for (input, expected) in [
                ("stem:[+1+]", r"\$1\$"),
                (r"stem:[+\infty-(+\infty)]", r"\$\infty-(\infty)\$"),
                (r"stem:[+++\infty-(+\infty)++]", r"\$+\infty-(+\infty)\$"),
            ] {
                let p = Parser::default().with_intrinsic_attribute(
                    "stem",
                    "",
                    ModificationContext::Anywhere,
                );
                assert_content(p, input, expected);
            }
        }

        #[test]
        fn should_allow_passthrough_inside_stem_expression_to_be_escaped() {
            verifies!(
                r#"
      test 'should allow passthrough inside stem expression to be escaped' do
        [
          ['stem:[\+] and stem:[+]', '\$+\$ and \$+\$'],
          ['stem:[\+1+]', '\$+1+\$'],
        ].each do |input, expected|
          para = block_from_string input, attributes: { 'stem' => '', }
          assert_equal expected, para.content
        end
      end

"#
            );

            for (input, expected) in [
                (r"stem:[\+] and stem:[+]", r"\$+\$ and \$+\$"),
                (r"stem:[\+1+]", r"\$+1+\$"),
            ] {
                let p = Parser::default().with_intrinsic_attribute(
                    "stem",
                    "",
                    ModificationContext::Anywhere,
                );
                assert_content(p, input, expected);
            }
        }

        #[test]
        fn should_not_recognize_stem_macro_with_invalid_substitution_list() {
            verifies!(
                r#"
      test 'should not recognize stem macro with invalid substitution list' do
        [',', '42', 'a,'].each do |subs|
          input = %(stem:#{subs}[x^2])
          para = block_from_string input, attributes: { 'stem' => 'asciimath' }
          assert_equal %(stem:#{subs}[x^2]), para.content
        end
      end

"#
            );

            for subs in [",", "42", "a,"] {
                let p = Parser::default().with_intrinsic_attribute(
                    "stem",
                    "asciimath",
                    ModificationContext::Anywhere,
                );
                let input = format!("stem:{subs}[x^2]");
                assert_content(p, &input, &input);
            }
        }

        #[test]
        fn should_warn_if_substitutions_on_stem_macro_are_invalid() {
            verifies!(
                r#"
      test 'should warn if substitutions on stem macro are invalid' do
        subs = 'bogus'
        input = %(stem:#{subs}[x^2])
        using_memory_logger do |logger|
          para = block_from_string input, attributes: { 'stem' => 'asciimath' }
          assert_equal '\\$x^2\\$', para.content
          assert_message logger, :WARN, %(invalid substitution type for stem macro: #{subs})
        end
      end
"#
            );

            let mut p = Parser::default().with_intrinsic_attribute(
                "stem",
                "asciimath",
                ModificationContext::Anywhere,
            );
            let doc = p.parse("stem:bogus[x^2]");
            assert_eq!(
                doc.nested_blocks().next().unwrap().rendered_content(),
                Some(r"\$x^2\$")
            );

            let warnings: Vec<_> = doc.warnings().collect();
            assert_eq!(warnings.len(), 1);
            assert_eq!(
                warnings[0].warning,
                WarningType::InvalidSubstitutionTypeForStemMacro("bogus".to_string())
            );
        }

        #[test]
        fn should_not_process_escaped_stem_macro() {
            // A leading backslash escapes the entire macro: the backslash is
            // dropped and the macro text is emitted verbatim.
            assert_content(
                Parser::default(),
                r"The \stem:[x^2] macro is escaped.",
                "The stem:[x^2] macro is escaped.",
            );
        }
    }

    non_normative!(
        r#"
    end
"#
    );
}

non_normative!(
    r#"
  end

  context 'Replacements' do
"#
);

mod replacements {
    use crate::{content::SubstitutionStep, strings::CowStr, tests::prelude::*};

    #[test]
    fn unescapes_xml_entities() {
        verifies!(
            r#"
    test 'unescapes XML entities' do
      para = block_from_string '< &quot; &there4; &#34; &#x22; >'
      assert_equal '&lt; &quot; &there4; &#34; &#x22; &gt;', para.apply_subs(para.source)
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("< &quot; &there4; &#34; &#x22; >"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "< &quot; &there4; &#34; &#x22; >",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "&lt; &quot; &there4; &#34; &#x22; &gt;",
                },
                source: Span {
                    data: "< &quot; &there4; &#34; &#x22; >",
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
            },)
        );
    }

    #[test]
    fn replaces_arrows() {
        verifies!(
            r#"
    test 'replaces arrows' do
      para = block_from_string '<- -> <= => \<- \-> \<= \=>'
      assert_equal '&#8592; &#8594; &#8656; &#8658; &lt;- -&gt; &lt;= =&gt;', para.apply_subs(para.source)
    end

"#
        );

        let mut p = Parser::default();
        let maw =
            crate::blocks::Block::parse(crate::Span::new(r#"<- -> <= => \<- \-> \<= \=>"#), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"<- -> <= => \<- \-> \<= \=>"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "&#8592; &#8594; &#8656; &#8658; &lt;- -&gt; &lt;= =&gt;",
                },
                source: Span {
                    data: r#"<- -> <= => \<- \-> \<= \=>"#,
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
            },)
        );
    }

    #[test]
    fn replaces_dashes() {
        verifies!(
            r#"
    test 'replaces dashes' do
      input = <<~'EOS'
      -- foo foo--bar foo\--bar foo -- bar foo \-- bar
      stuff in between
      -- foo
      stuff in between
      foo --
      stuff in between
      foo --
      EOS
      expected = <<~'EOS'.chop
      &#8201;&#8212;&#8201;foo foo&#8212;&#8203;bar foo--bar foo&#8201;&#8212;&#8201;bar foo -- bar
      stuff in between&#8201;&#8212;&#8201;foo
      stuff in between
      foo&#8201;&#8212;&#8201;stuff in between
      foo&#8201;&#8212;&#8201;
      EOS
      para = block_from_string input
      assert_equal expected, para.sub_replacements(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new(
            r#"-- foo foo--bar foo\--bar foo -- bar foo \-- bar
stuff in between
-- foo
stuff in between
foo --
stuff in between
foo --
"#,
        ));

        let expected = r#"&#8201;&#8212;&#8201;foo foo&#8212;&#8203;bar foo--bar foo&#8201;&#8212;&#8201;bar foo -- bar
stuff in between&#8201;&#8212;&#8201;foo
stuff in between
foo&#8201;&#8212;&#8201;stuff in between
foo&#8201;&#8212;&#8201;"#;

        let p = Parser::default();
        SubstitutionStep::CharacterReplacements.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(expected.to_string().into_boxed_str())
        );
    }

    #[test]
    fn replaces_dashes_between_multibyte_word_characters() {
        verifies!(
            r#"
    test 'replaces dashes between multibyte word characters' do
      para = block_from_string %(富--巴)
      expected = '富&#8212;&#8203;巴'
      assert_equal expected, para.sub_replacements(para.source)
    end

"#
        );

        let mut content = crate::content::Content::from(crate::Span::new("富--巴"));

        let p = Parser::default();
        SubstitutionStep::CharacterReplacements.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed("富&#8212;&#8203;巴".to_string().into_boxed_str())
        );
    }

    #[test]
    fn replaces_marks() {
        verifies!(
            r#"
    test 'replaces marks' do
      para = block_from_string '(C) (R) (TM) \(C) \(R) \(TM)'
      assert_equal '&#169; &#174; &#8482; (C) (R) (TM)', para.sub_replacements(para.source)
    end

"#
        );

        let mut content =
            crate::content::Content::from(crate::Span::new(r#"(C) (R) (TM) \(C) \(R) \(TM)"#));

        let p = Parser::default();
        SubstitutionStep::CharacterReplacements.apply(&mut content, &p, None);

        assert_eq!(
            content.rendered,
            CowStr::Boxed(
                "&#169; &#174; &#8482; (C) (R) (TM)"
                    .to_string()
                    .into_boxed_str()
            )
        );
    }

    #[test]
    fn preserves_entity_references() {
        verifies!(
            r#"
    test 'preserves entity references' do
      input = '&amp; &#169; &#10004; &#128512; &#x2022; &#x1f600;'
      result = convert_inline_string input
      assert_equal input, result
    end

"#
        );

        let input = "&amp; &#169; &#10004; &#128512; &#x2022; &#x1f600;";

        let mut content = crate::content::Content::from(crate::Span::new(input));
        let p = Parser::default();
        SubstitutionStep::CharacterReplacements.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(input.to_string().into_boxed_str())
        );
    }

    #[test]
    fn only_preserves_named_entities_with_two_or_more_letters() {
        verifies!(
            r#"
    test 'only preserves named entities with two or more letters' do
      input = '&amp; &a; &gt;'
      result = convert_inline_string input
      assert_equal '&amp; &amp;a; &gt;', result
    end

"#
        );

        let input = "&amp; &a; &gt;";

        let mut content = crate::content::Content::from(crate::Span::new(input));
        let p = Parser::default();
        SubstitutionStep::CharacterReplacements.apply(&mut content, &p, None);

        assert_eq!(
            content.rendered,
            CowStr::Boxed(input.to_string().into_boxed_str())
        );
    }

    #[test]
    fn replaces_punctuation() {
        verifies!(
            r#"
    test 'replaces punctuation' do
      para = block_from_string %(John's Hideout is the Whites`' place... foo\\'bar)
      assert_equal "John&#8217;s Hideout is the Whites&#8217; place&#8230;&#8203; foo'bar", para.sub_replacements(para.source)
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new(r#"John's Hideout is the Whites`' place... foo\'bar"#),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"John's Hideout is the Whites`' place... foo\'bar"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "John&#8217;s Hideout is the Whites&#8217; place&#8230;&#8203; foo'bar",
                },
                source: Span {
                    data: r#"John's Hideout is the Whites`' place... foo\'bar"#,
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
            },)
        );
    }

    #[test]
    fn should_replace_right_single_quote_marks() {
        verifies!(
            r#"
    test 'should replace right single quote marks' do
      given = [
        %(`'Twas the night),
        %(a `'57 Chevy!),
        %(the whites`' place),
        %(the whites`'.),
        %(the whites`'--where the wild things are),
        %(the whites`'\nhave),
        %(It's Mary`'s little lamb.),
        %(consecutive single quotes '' are not modified),
        %(he is 6' tall),
        %(\\`')
      ]
      expected = [
        %(&#8217;Twas the night),
        %(a &#8217;57 Chevy!),
        %(the whites&#8217; place),
        %(the whites&#8217;.),
        %(the whites&#8217;--where the wild things are),
        %(the whites&#8217;\nhave),
        %(It&#8217;s Mary&#8217;s little lamb.),
        %(consecutive single quotes '' are not modified),
        %(he is 6' tall),
        %(`')
      ]
      given.size.times do |i|
        para = block_from_string given[i]
        assert_equal expected[i], para.sub_replacements(para.source)
      end
    end
"#
        );

        // Ported from Asciidoctor's `should replace right single quote marks`.
        // In Asciidoctor this exercises `sub_replacements`, which corresponds to
        // the `CharacterReplacements` substitution step here.
        let cases = [
            (r#"`'Twas the night"#, "&#8217;Twas the night"),
            (r#"a `'57 Chevy!"#, "a &#8217;57 Chevy!"),
            (r#"the whites`' place"#, "the whites&#8217; place"),
            (r#"the whites`'."#, "the whites&#8217;."),
            (
                r#"the whites`'--where the wild things are"#,
                "the whites&#8217;--where the wild things are",
            ),
            ("the whites`'\nhave", "the whites&#8217;\nhave"),
            (
                r#"It's Mary`'s little lamb."#,
                "It&#8217;s Mary&#8217;s little lamb.",
            ),
            (
                r#"consecutive single quotes '' are not modified"#,
                "consecutive single quotes '' are not modified",
            ),
            (r#"he is 6' tall"#, "he is 6' tall"),
            (r#"\`'"#, "`'"),
        ];

        let p = Parser::default();

        for (given, expected) in cases {
            let mut content = crate::content::Content::from(crate::Span::new(given));
            SubstitutionStep::CharacterReplacements.apply(&mut content, &p, None);
            assert_eq!(content.rendered, CowStr::from(expected), "input: {given:?}");
        }
    }
}

non_normative!(
    r#"
  end

  context 'Post replacements' do
"#
);

mod post_replacements {
    use crate::tests::prelude::*;

    #[test]
    fn line_break_inserted_after_line_with_line_break_character() {
        verifies!(
            r#"
    test 'line break inserted after line with line break character' do
      para = block_from_string("First line +\nSecond line")
      result = para.apply_subs para.lines, (para.expand_subs :post_replacements)
      assert_equal 'First line<br>', result.first
    end

"#
        );

        let mut p = Parser::default();
        let maw =
            crate::blocks::Block::parse(crate::Span::new("First line +\nSecond line"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "First line +\nSecond line",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "First line<br>\nSecond line",
                },
                source: Span {
                    data: "First line +\nSecond line",
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
            },)
        );
    }

    #[test]
    fn line_break_inserted_after_line_wrap_with_hardbreaks_enabled() {
        verifies!(
            r#"
    test 'line break inserted after line wrap with hardbreaks enabled' do
      para = block_from_string("First line\nSecond line", attributes: { 'hardbreaks' => '' })
      result = para.apply_subs para.lines, (para.expand_subs :post_replacements)
      assert_equal 'First line<br>', result.first
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("[%hardbreaks]\nFirst line\nSecond line"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "First line\nSecond line",
                        line: 2,
                        col: 1,
                        offset: 14,
                    },
                    rendered: "First line<br>\nSecond line",
                },
                source: Span {
                    data: "[%hardbreaks]\nFirst line\nSecond line",
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
                        shorthand_items: &["%hardbreaks"],
                        value: "%hardbreaks"
                    },],
                    anchor: None,
                    source: Span {
                        data: "%hardbreaks",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },)
        );
    }

    #[test]
    fn line_break_character_stripped_from_end_of_line_with_hardbreaks_enabled() {
        verifies!(
            r#"
    test 'line break character stripped from end of line with hardbreaks enabled' do
      para = block_from_string("First line +\nSecond line", attributes: { 'hardbreaks' => '' })
      result = para.apply_subs para.lines, (para.expand_subs :post_replacements)
      assert_equal 'First line<br>', result.first
    end

"#
        );

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("[%hardbreaks]\nFirst line +\nSecond line"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "First line +\nSecond line",
                        line: 2,
                        col: 1,
                        offset: 14,
                    },
                    rendered: "First line<br>\nSecond line",
                },
                source: Span {
                    data: "[%hardbreaks]\nFirst line +\nSecond line",
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
                        shorthand_items: &["%hardbreaks"],
                        value: "%hardbreaks"
                    },],
                    anchor: None,
                    source: Span {
                        data: "%hardbreaks",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },)
        );
    }

    #[test]
    fn line_break_not_inserted_for_single_line_with_hardbreaks_enabled() {
        verifies!(
            r#"
    test 'line break not inserted for single line with hardbreaks enabled' do
      para = block_from_string('First line', attributes: { 'hardbreaks' => '' })
      result = para.apply_subs para.lines, (para.expand_subs :post_replacements)
      assert_equal 'First line', result.first
    end
"#
        );

        let mut p = Parser::default();
        let maw =
            crate::blocks::Block::parse(crate::Span::new("[%hardbreaks]\nFirst line"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "First line",
                        line: 2,
                        col: 1,
                        offset: 14,
                    },
                    rendered: "First line",
                },
                source: Span {
                    data: "[%hardbreaks]\nFirst line",
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
                        shorthand_items: &["%hardbreaks"],
                        value: "%hardbreaks"
                    },],
                    anchor: None,
                    source: Span {
                        data: "%hardbreaks",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },)
        );
    }
}

// Not ported to this crate:
// - 'should resolve subs for block': Block#commit_subs resolution API; sub-list
//   expansion is covered by content::substitution_group.
// - 'should resolve specialcharacters sub as highlight for source block when
//   source highlighter is coderay': source-highlighter (CodeRay) integration is
//   out of scope for this crate.
// - 'should resolve specialcharacters sub as highlight for source block when
//   source highlighter is pygments': source-highlighter (Pygments) integration
//   is out of scope for this crate.
// - 'should not replace specialcharacters sub with highlight for source block
//   when source highlighter is not set': source-highlighter integration is out
//   of scope for this crate.
// - 'should not use subs if subs option passed to block constructor is nil':
//   Block-constructor subs option has no crate equivalent.
// - 'should not use subs if subs option passed to block constructor is empty
//   array': Block-constructor subs option has no crate equivalent.
// - 'should use subs from subs option passed to block constructor':
//   Block-constructor subs option has no crate equivalent.
// - 'should use subs from subs attribute if subs option is not passed to block
//   constructor': Block-constructor subs option has no crate equivalent.
// - 'should use subs from subs attribute if subs option passed to block
//   constructor is :default': Block-constructor subs option has no crate
//   equivalent.
// - 'should use built-in subs if subs option passed to block constructor is
//   :default and subs attribute is absent': Block-constructor subs option has
//   no crate equivalent.
non_normative!(
    r#"
  end

  context 'Resolve subs' do
    test 'should resolve subs for block' do
      doc = empty_document parse: true
      block = Asciidoctor::Block.new doc, :paragraph
      block.attributes['subs'] = 'quotes,normal'
      block.commit_subs
      assert_equal [:quotes, :specialcharacters, :attributes, :replacements, :macros, :post_replacements], block.subs
    end

    test 'should resolve specialcharacters sub as highlight for source block when source highlighter is coderay' do
      doc = empty_document attributes: { 'source-highlighter' => 'coderay' }, parse: true
      block = Asciidoctor::Block.new doc, :listing, content_model: :verbatim
      block.style = 'source'
      block.attributes['subs'] = 'specialcharacters'
      block.attributes['language'] = 'ruby'
      block.commit_subs
      assert_equal [:highlight], block.subs
    end

    test 'should resolve specialcharacters sub as highlight for source block when source highlighter is pygments', if: ENV['PYGMENTS_VERSION'] do
      doc = empty_document attributes: { 'source-highlighter' => 'pygments' }, parse: true
      block = Asciidoctor::Block.new doc, :listing, content_model: :verbatim
      block.style = 'source'
      block.attributes['subs'] = 'specialcharacters'
      block.attributes['language'] = 'ruby'
      block.commit_subs
      assert_equal [:highlight], block.subs
    end

    test 'should not replace specialcharacters sub with highlight for source block when source highlighter is not set' do
      doc = empty_document parse: true
      block = Asciidoctor::Block.new doc, :listing, content_model: :verbatim
      block.style = 'source'
      block.attributes['subs'] = 'specialcharacters'
      block.attributes['language'] = 'ruby'
      block.commit_subs
      assert_equal [:specialcharacters], block.subs
    end

    test 'should not use subs if subs option passed to block constructor is nil' do
      doc = empty_document parse: true
      block = Asciidoctor::Block.new doc, :paragraph, source: '*bold* _italic_', subs: nil, attributes: { 'subs' => 'quotes' }
      assert_empty block.subs
      block.commit_subs
      assert_empty block.subs
    end

    test 'should not use subs if subs option passed to block constructor is empty array' do
      doc = empty_document parse: true
      block = Asciidoctor::Block.new doc, :paragraph, source: '*bold* _italic_', subs: [], attributes: { 'subs' => 'quotes' }
      assert_empty block.subs
      block.commit_subs
      assert_empty block.subs
    end

    test 'should use subs from subs option passed to block constructor' do
      doc = empty_document parse: true
      block = Asciidoctor::Block.new doc, :paragraph, source: '*bold* _italic_', subs: [:specialcharacters], attributes: { 'subs' => 'quotes' }
      assert_equal [:specialcharacters], block.subs
      block.commit_subs
      assert_equal [:specialcharacters], block.subs
    end

    test 'should use subs from subs attribute if subs option is not passed to block constructor' do
      doc = empty_document parse: true
      block = Asciidoctor::Block.new doc, :paragraph, source: '*bold* _italic_', attributes: { 'subs' => 'quotes' }
      assert_empty block.subs
      # in this case, we have to call commit_subs to resolve the subs
      block.commit_subs
      assert_equal [:quotes], block.subs
    end

    test 'should use subs from subs attribute if subs option passed to block constructor is :default' do
      doc = empty_document parse: true
      block = Asciidoctor::Block.new doc, :paragraph, source: '*bold* _italic_', subs: :default, attributes: { 'subs' => 'quotes' }
      assert_equal [:quotes], block.subs
      block.commit_subs
      assert_equal [:quotes], block.subs
    end

    test 'should use built-in subs if subs option passed to block constructor is :default and subs attribute is absent' do
      doc = empty_document parse: true
      block = Asciidoctor::Block.new doc, :paragraph, source: '*bold* _italic_', subs: :default
      assert_equal [:specialcharacters, :quotes, :attributes, :replacements, :macros, :post_replacements], block.subs
      block.commit_subs
      assert_equal [:specialcharacters, :quotes, :attributes, :replacements, :macros, :post_replacements], block.subs
    end
  end
end

"#
);
