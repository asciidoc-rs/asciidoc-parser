// Adapted from Asciidoctor's links test suite, found in
// https://github.com/asciidoctor/asciidoctor/blob/main/test/links_test.rb.
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
//! Port of Asciidoctor's `links_test.rb`.
//!
//! Each Ruby `test '..'` in the single flat `context 'Links'` becomes a
//! `#[test] fn`, driven through [`Parser`](crate::Parser) and asserted on the
//! rendered inline content of each block ([`rendered_paragraphs`]) or on the
//! document [`Catalog`](crate::document::Catalog). Every `test '..'` block is
//! reproduced verbatim in a `verifies!` block so the `sdd` coverage tool can
//! measure which lines of the Ruby suite are verified line-by-line.
//!
//! Where the crate's observable output matches Asciidoctor, the test is a
//! `verifies!`. The following are reproduced in `non_normative!` blocks
//! instead, each annotated with the reason it is not ported:
//!
//! * **DocBook backend** output — out of scope for this parser crate.
//! * **Compat mode** — disregarded per the porting conventions.
//! * **Include processing** (`include::[]`, `catalog[:includes]`) and
//!   Ruby-internal APIs (`doc.register`, `using_memory_logger`) — not modeled
//!   the same way here.
//! * **Inter-document self-xref fallback text** – a target that points back at
//!   the current document (`xref:test.adoc[]` where `docname` is `test`) is
//!   rewritten to that document's output path rather than collapsing to `#`
//!   with the doctitle as its link text.
//! * A number of **other surfaced incompatibilities**, each carrying a `//
//!   NOTE:` describing how the crate diverges from Asciidoctor.

use crate::tests::prelude::*;

track_file!("ref/asciidoctor/test/links_test.rb");

/// Parses `src` with the given intrinsic attributes applied.
fn doc_with(src: &str, attrs: &[(&str, &str)]) -> crate::Document<'static> {
    let mut parser = Parser::default();
    for (name, value) in attrs {
        parser = parser.with_intrinsic_attribute(*name, *value, ModificationContext::Anywhere);
    }
    parser.parse(src)
}

non_normative!(
    r###"
# frozen_string_literal: true
require_relative 'test_helper'

context 'Links' do

"###
);

#[test]
fn qualified_url_inline_with_text() {
    verifies!(
        r###"
  test 'qualified url inline with text' do
    assert_xpath "//a[@href='http://asciidoc.org'][@class='bare'][text() = 'http://asciidoc.org']", convert_string("The AsciiDoc project is located at http://asciidoc.org.")
  end

"###
    );

    let doc =
        Parser::default().parse(r##"The AsciiDoc project is located at http://asciidoc.org."##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"The AsciiDoc project is located at <a href="http://asciidoc.org" class="bare">http://asciidoc.org</a>."##
    );
}

#[test]
fn qualified_url_with_role_inline_with_text() {
    verifies!(
        r###"
  test 'qualified url with role inline with text' do
    assert_xpath "//a[@href='http://asciidoc.org'][@class='bare project'][text() = 'http://asciidoc.org']", convert_string("The AsciiDoc project is located at http://asciidoc.org[role=project].")
  end

"###
    );

    let doc = Parser::default()
        .parse(r##"The AsciiDoc project is located at http://asciidoc.org[role=project]."##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"The AsciiDoc project is located at <a href="http://asciidoc.org" class="bare project">http://asciidoc.org</a>."##
    );
}

#[test]
fn qualified_http_url_inline_with_hide_uri_scheme_set() {
    verifies!(
        r###"
  test 'qualified http url inline with hide-uri-scheme set' do
    assert_xpath "//a[@href='http://asciidoc.org'][@class='bare'][text() = 'asciidoc.org']", convert_string("The AsciiDoc project is located at http://asciidoc.org.", attributes: { 'hide-uri-scheme' => '' })
  end

"###
    );

    let doc = doc_with(
        r##"The AsciiDoc project is located at http://asciidoc.org."##,
        &[("hide-uri-scheme", "")],
    );
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"The AsciiDoc project is located at <a href="http://asciidoc.org" class="bare">asciidoc.org</a>."##
    );
}

#[test]
fn qualified_file_url_inline_with_label() {
    verifies!(
        r###"
  test 'qualified file url inline with label' do
    assert_xpath "//a[@href='file:///home/user/bookmarks.html'][text() = 'My Bookmarks']", convert_string_to_embedded('file:///home/user/bookmarks.html[My Bookmarks]')
  end

"###
    );

    let doc = Parser::default().parse(r##"file:///home/user/bookmarks.html[My Bookmarks]"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="file:///home/user/bookmarks.html">My Bookmarks</a>"##
    );
}

#[test]
fn qualified_file_url_inline_with_hide_uri_scheme_set() {
    verifies!(
        r###"
  test 'qualified file url inline with hide-uri-scheme set' do
    assert_xpath "//a[@href='file:///etc/app.conf'][text() = '/etc/app.conf']", convert_string('Edit the configuration file link:file:///etc/app.conf[]', attributes: { 'hide-uri-scheme' => '' })
  end

"###
    );

    let doc = doc_with(
        r##"Edit the configuration file link:file:///etc/app.conf[]"##,
        &[("hide-uri-scheme", "")],
    );
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"Edit the configuration file <a href="file:///etc/app.conf" class="bare">/etc/app.conf</a>"##
    );
}

#[test]
fn should_not_hide_bare_uri_scheme_in_implicit_text_of_link_macro_when_hide_uri_scheme_is_set() {
    verifies!(
        r###"
  test 'should not hide bare URI scheme in implicit text of link macro when hide-uri-scheme is set' do
    {
      'link:https://[]' => 'https://',
      'link:ssh://[]' => 'ssh://',
    }.each do |input, expected|
      assert_xpath %(/a[text() = "#{expected}"]), (convert_inline_string input, attributes: { 'hide-uri-scheme' => '' })
    end
  end

"###
    );

    let doc = doc_with(r##"link:https://[]"##, &[("hide-uri-scheme", "")]);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="https://" class="bare">https://</a>"##
    );
    let doc = doc_with(r##"link:ssh://[]"##, &[("hide-uri-scheme", "")]);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="ssh://" class="bare">ssh://</a>"##
    );
}

#[test]
fn qualified_url_with_label() {
    verifies!(
        r###"
  test 'qualified url with label' do
    assert_xpath "//a[@href='http://asciidoc.org'][text() = 'AsciiDoc']", convert_string("We're parsing http://asciidoc.org[AsciiDoc] markup")
  end

"###
    );

    let doc = Parser::default().parse(r##"We're parsing http://asciidoc.org[AsciiDoc] markup"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"We&#8217;re parsing <a href="http://asciidoc.org">AsciiDoc</a> markup"##
    );
}

#[test]
fn qualified_url_with_label_containing_escaped_right_square_bracket() {
    verifies!(
        r###"
  test 'qualified url with label containing escaped right square bracket' do
    assert_xpath "//a[@href='http://asciidoc.org'][text() = '[Ascii]Doc']", convert_string("We're parsing http://asciidoc.org[[Ascii\\]Doc] markup")
  end

"###
    );

    let doc = Parser::default().parse(r##"We're parsing http://asciidoc.org[[Ascii\]Doc] markup"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"We&#8217;re parsing <a href="http://asciidoc.org">[Ascii]Doc</a> markup"##
    );
}

#[test]
fn qualified_url_with_backslash_label() {
    verifies!(
        r###"
  test 'qualified url with backslash label' do
    assert_xpath "//a[@href='https://google.com'][text() = 'Google for \\']", convert_string("I advise you to https://google.com[Google for +\\+]")
  end

"###
    );

    let doc = Parser::default().parse(r##"I advise you to https://google.com[Google for +\+]"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"I advise you to <a href="https://google.com">Google for \</a>"##
    );
}

#[test]
fn qualified_url_with_label_using_link_macro() {
    verifies!(
        r###"
  test 'qualified url with label using link macro' do
    assert_xpath "//a[@href='http://asciidoc.org'][text() = 'AsciiDoc']", convert_string("We're parsing link:http://asciidoc.org[AsciiDoc] markup")
  end

"###
    );

    let doc =
        Parser::default().parse(r##"We're parsing link:http://asciidoc.org[AsciiDoc] markup"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"We&#8217;re parsing <a href="http://asciidoc.org">AsciiDoc</a> markup"##
    );
}

#[test]
fn qualified_url_with_role_using_link_macro() {
    verifies!(
        r###"
  test 'qualified url with role using link macro' do
    assert_xpath "//a[@href='http://asciidoc.org'][@class='bare project'][text() = 'http://asciidoc.org']", convert_string("We're parsing link:http://asciidoc.org[role=project] markup")
  end

"###
    );

    let doc =
        Parser::default().parse(r##"We're parsing link:http://asciidoc.org[role=project] markup"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"We&#8217;re parsing <a href="http://asciidoc.org" class="bare project">http://asciidoc.org</a> markup"##
    );
}

#[test]
fn qualified_url_using_macro_syntax_with_multi_line_label_inline_with_text() {
    verifies!(
        r###"
  test 'qualified url using macro syntax with multi-line label inline with text' do
    assert_xpath %{//a[@href='http://asciidoc.org'][text() = 'AsciiDoc\nmarkup']}, convert_string("We're parsing link:http://asciidoc.org[AsciiDoc\nmarkup]")
  end

"###
    );

    let doc = Parser::default().parse("We're parsing link:http://asciidoc.org[AsciiDoc\nmarkup]");
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        "We&#8217;re parsing <a href=\"http://asciidoc.org\">AsciiDoc\nmarkup</a>"
    );
}

#[test]
fn qualified_url_with_label_containing_square_brackets_using_link_macro() {
    verifies!(
        r###"
  test 'qualified url with label containing square brackets using link macro' do
    str = 'http://example.com[[bracket1\]]'
    doc = document_from_string str, standalone: false, doctype: 'inline'
    assert_match '<a href="http://example.com">[bracket1]</a>', doc.convert, 1
    doc = document_from_string str, standalone: false, backend: 'docbook', doctype: 'inline'
    assert_match '<link xl:href="http://example.com">[bracket1]</link>', doc.convert, 1
  end

"###
    );

    // Only the HTML rendering is verified here; the DocBook sub-case is out
    // of scope for this crate.
    let doc = Parser::default().parse(r##"http://example.com[[bracket1\]]"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="http://example.com">[bracket1]</a>"##
    );
}

#[test]
fn link_macro_with_empty_target() {
    verifies!(
        r###"
  test 'link macro with empty target' do
    input = 'Link to link:[this page].'
    output = convert_string_to_embedded input
    assert_xpath '//a', output, 1
    assert_xpath '//a[@href=""]', output, 1
  end

"###
    );

    let doc = Parser::default().parse("Link to link:[this page].");
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"Link to <a href="">this page</a>."##
    );
}

#[test]
fn should_not_recognize_link_macro_with_double_colons() {
    verifies!(
        r###"
  test 'should not recognize link macro with double colons' do
    input = 'The link::http://example.org[example domain] is reserved for tests and documentation.'
    output = convert_string_to_embedded input
    assert_includes output, 'link::http://example.org[example domain]'
  end

"###
    );

    let doc = Parser::default().parse(r##"The link::http://example.org[example domain] is reserved for tests and documentation."##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"The link::http://example.org[example domain] is reserved for tests and documentation."##
    );
}

#[test]
fn qualified_url_surrounded_by_angled_brackets() {
    verifies!(
        r###"
  test 'qualified url surrounded by angled brackets' do
    assert_xpath '//a[@href="http://asciidoc.org"][@class="bare"][text()="http://asciidoc.org"]', convert_string('<http://asciidoc.org> is the project page for AsciiDoc.'), 1
  end

"###
    );

    let doc =
        Parser::default().parse(r##"<http://asciidoc.org> is the project page for AsciiDoc."##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="http://asciidoc.org" class="bare">http://asciidoc.org</a> is the project page for AsciiDoc."##
    );
}

#[test]
fn qualified_url_surrounded_by_double_angled_brackets_should_preserve_outer_angled_brackets() {
    verifies!(
        r###"
  test 'qualified url surrounded by double angled brackets should preserve outer angled brackets' do
    assert_includes convert_string_to_embedded('<<https://asciidoc.org>>'), '&lt;<a href="https://asciidoc.org" class="bare">https://asciidoc.org</a>&gt;'
  end

"###
    );

    let doc = Parser::default().parse(r##"<<https://asciidoc.org>>"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"&lt;<a href="https://asciidoc.org" class="bare">https://asciidoc.org</a>&gt;"##
    );
}

#[test]
fn qualified_url_macro_inside_angled_brackets() {
    verifies!(
        r###"
  test 'qualified url macro inside angled brackets' do
    assert_includes convert_string_to_embedded('<https://asciidoc.org[]>'), '&lt;<a href="https://asciidoc.org" class="bare">https://asciidoc.org</a>&gt;'
  end

"###
    );

    let doc = Parser::default().parse(r##"<https://asciidoc.org[]>"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"&lt;<a href="https://asciidoc.org" class="bare">https://asciidoc.org</a>&gt;"##
    );
}

#[test]
fn qualified_url_surrounded_by_angled_brackets_in_unconstrained_context() {
    verifies!(
        r###"
  test 'qualified url surrounded by angled brackets in unconstrained context' do
    assert_xpath '//a[@href="http://asciidoc.org"][@class="bare"][text()="http://asciidoc.org"]', convert_string('URLは<http://asciidoc.org>。fin'), 1
  end

"###
    );

    let doc = Parser::default().parse(r##"URLは<http://asciidoc.org>。fin"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"URLは<a href="http://asciidoc.org" class="bare">http://asciidoc.org</a>。fin"##
    );
}

#[test]
fn multiple_qualified_urls_surrounded_by_angled_brackets_in_unconstrained_context() {
    verifies!(
        r###"
  test 'multiple qualified urls surrounded by angled brackets in unconstrained context' do
    assert_xpath '//a[@href="http://asciidoc.org"][@class="bare"][text()="http://asciidoc.org"]', convert_string('URLは<http://asciidoc.org>。URLは<http://asciidoc.org>。'), 2
  end

"###
    );

    let doc =
        Parser::default().parse(r##"URLは<http://asciidoc.org>。URLは<http://asciidoc.org>。"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"URLは<a href="http://asciidoc.org" class="bare">http://asciidoc.org</a>。URLは<a href="http://asciidoc.org" class="bare">http://asciidoc.org</a>。"##
    );
}

#[test]
fn qualified_url_surrounded_by_escaped_angled_brackets_should_escape_form() {
    verifies!(
        r###"
  test 'qualified url surrounded by escaped angled brackets should escape form' do
    assert_xpath '//p[text()="<http://asciidoc.org>"]', convert_string('\\<http://asciidoc.org>'), 1
  end

"###
    );

    let doc = Parser::default().parse(r##"\<http://asciidoc.org>"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"&lt;http://asciidoc.org&gt;"##
    );
}

#[test]
fn escaped_qualified_url_surrounded_by_angled_brackets_should_escape_autolink() {
    verifies!(
        r###"
  test 'escaped qualified url surrounded by angled brackets should escape autolink' do
    assert_xpath '//p[text()="<http://asciidoc.org>"]', convert_string('<\\http://asciidoc.org>'), 1
  end

"###
    );

    let doc = Parser::default().parse(r##"<\http://asciidoc.org>"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"&lt;http://asciidoc.org&gt;"##
    );
}

#[test]
fn xref_shorthand_with_target_that_starts_with_url_protocol_and_has_space_after_comma_should_not_crash_parser()
 {
    verifies!(
        r###"
  test 'xref shorthand with target that starts with URL protocol and has space after comma should not crash parser' do
    output = convert_string_to_embedded '<<https://example.com, Example>>'
    assert_include '<a href="#https://example.com">Example</a>', output
  end

"###
    );

    let doc = Parser::default().parse(r##"<<https://example.com, Example>>"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="#https://example.com">Example</a>"##
    );
}

#[test]
fn xref_shorthand_with_link_macro_as_target_should_be_ignored() {
    verifies!(
        r###"
  test 'xref shorthand with link macro as target should be ignored' do
    output = convert_string_to_embedded '<<link:https://example.com[], Example>>'
    assert_include '&lt;&lt;<a href="https://example.com" class="bare">https://example.com</a>, Example&gt;&gt;', output
  end

"###
    );

    let doc = Parser::default().parse("<<link:https://example.com[], Example>>");
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"&lt;&lt;<a href="https://example.com" class="bare">https://example.com</a>, Example&gt;&gt;"##
    );
}

#[test]
fn autolink_containing_text_enclosed_in_angle_brackets() {
    verifies!(
        r###"
  test 'autolink containing text enclosed in angle brackets' do
    output = convert_string_to_embedded 'https://github.com/<org>/'
    assert_include '<a href="https://github.com/&lt;org&gt;/" class="bare">https://github.com/&lt;org&gt;/</a>', output
  end

"###
    );

    let doc = Parser::default().parse(r##"https://github.com/<org>/"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="https://github.com/&lt;org&gt;/" class="bare">https://github.com/&lt;org&gt;/</a>"##
    );
}

#[test]
fn qualified_url_surrounded_by_round_brackets() {
    verifies!(
        r###"
  test 'qualified url surrounded by round brackets' do
    assert_xpath '//a[@href="http://asciidoc.org"][text()="http://asciidoc.org"]', convert_string('(http://asciidoc.org) is the project page for AsciiDoc.'), 1
  end

"###
    );

    let doc =
        Parser::default().parse(r##"(http://asciidoc.org) is the project page for AsciiDoc."##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"(<a href="http://asciidoc.org" class="bare">http://asciidoc.org</a>) is the project page for AsciiDoc."##
    );
}

#[test]
fn qualified_url_with_trailing_period() {
    verifies!(
        r###"
  test 'qualified url with trailing period' do
    result = convert_string_to_embedded 'The homepage for Asciidoctor is https://asciidoctor.org.'
    assert_xpath '//a[@href="https://asciidoctor.org"][text()="https://asciidoctor.org"]', result, 1
    assert_xpath '//a[@href="https://asciidoctor.org"][text()="https://asciidoctor.org"]/following-sibling::text()[starts-with(.,".")]', result, 1
  end

"###
    );

    let doc =
        Parser::default().parse(r##"The homepage for Asciidoctor is https://asciidoctor.org."##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"The homepage for Asciidoctor is <a href="https://asciidoctor.org" class="bare">https://asciidoctor.org</a>."##
    );
}

#[test]
fn qualified_url_with_trailing_explanation_point() {
    verifies!(
        r###"
  test 'qualified url with trailing explanation point' do
    result = convert_string_to_embedded 'Check out https://asciidoctor.org!'
    assert_xpath '//a[@href="https://asciidoctor.org"][text()="https://asciidoctor.org"]', result, 1
    assert_xpath '//a[@href="https://asciidoctor.org"][text()="https://asciidoctor.org"]/following-sibling::text()[starts-with(.,"!")]', result, 1
  end

"###
    );

    let doc = Parser::default().parse(r##"Check out https://asciidoctor.org!"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"Check out <a href="https://asciidoctor.org" class="bare">https://asciidoctor.org</a>!"##
    );
}

#[test]
fn qualified_url_with_trailing_question_mark() {
    verifies!(
        r###"
  test 'qualified url with trailing question mark' do
    result = convert_string_to_embedded 'Is the homepage for Asciidoctor https://asciidoctor.org?'
    assert_xpath '//a[@href="https://asciidoctor.org"][text()="https://asciidoctor.org"]', result, 1
    assert_xpath '//a[@href="https://asciidoctor.org"][text()="https://asciidoctor.org"]/following-sibling::text()[starts-with(.,"?")]', result, 1
  end

"###
    );

    let doc =
        Parser::default().parse(r##"Is the homepage for Asciidoctor https://asciidoctor.org?"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"Is the homepage for Asciidoctor <a href="https://asciidoctor.org" class="bare">https://asciidoctor.org</a>?"##
    );
}

#[test]
fn qualified_url_with_trailing_round_bracket() {
    verifies!(
        r###"
  test 'qualified url with trailing round bracket' do
    result = convert_string_to_embedded 'Asciidoctor is a Ruby-based AsciiDoc processor (see https://asciidoctor.org)'
    assert_xpath '//a[@href="https://asciidoctor.org"][text()="https://asciidoctor.org"]', result, 1
    assert_xpath '//a[@href="https://asciidoctor.org"][text()="https://asciidoctor.org"]/following-sibling::text()[starts-with(.,")")]', result, 1
  end

"###
    );

    let doc = Parser::default()
        .parse(r##"Asciidoctor is a Ruby-based AsciiDoc processor (see https://asciidoctor.org)"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"Asciidoctor is a Ruby-based AsciiDoc processor (see <a href="https://asciidoctor.org" class="bare">https://asciidoctor.org</a>)"##
    );
}

#[test]
fn qualified_url_with_trailing_period_followed_by_round_bracket() {
    verifies!(
        r###"
  test 'qualified url with trailing period followed by round bracket' do
    result = convert_string_to_embedded '(The homepage for Asciidoctor is https://asciidoctor.org.)'
    assert_xpath '//a[@href="https://asciidoctor.org"][text()="https://asciidoctor.org"]', result, 1
    assert_xpath '//a[@href="https://asciidoctor.org"][text()="https://asciidoctor.org"]/following-sibling::text()[starts-with(.,".)")]', result, 1
  end

"###
    );

    let doc =
        Parser::default().parse(r##"(The homepage for Asciidoctor is https://asciidoctor.org.)"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"(The homepage for Asciidoctor is <a href="https://asciidoctor.org" class="bare">https://asciidoctor.org</a>.)"##
    );
}

#[test]
fn qualified_url_with_trailing_exclamation_point_followed_by_round_bracket() {
    verifies!(
        r###"
  test 'qualified url with trailing exclamation point followed by round bracket' do
    result = convert_string_to_embedded '(Check out https://asciidoctor.org!)'
    assert_xpath '//a[@href="https://asciidoctor.org"][text()="https://asciidoctor.org"]', result, 1
    assert_xpath '//a[@href="https://asciidoctor.org"][text()="https://asciidoctor.org"]/following-sibling::text()[starts-with(.,"!)")]', result, 1
  end

"###
    );

    let doc = Parser::default().parse(r##"(Check out https://asciidoctor.org!)"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"(Check out <a href="https://asciidoctor.org" class="bare">https://asciidoctor.org</a>!)"##
    );
}

#[test]
fn qualified_url_with_trailing_question_mark_followed_by_round_bracket() {
    verifies!(
        r###"
  test 'qualified url with trailing question mark followed by round bracket' do
    result = convert_string_to_embedded '(Is the homepage for Asciidoctor https://asciidoctor.org?)'
    assert_xpath '//a[@href="https://asciidoctor.org"][text()="https://asciidoctor.org"]', result, 1
    assert_xpath '//a[@href="https://asciidoctor.org"][text()="https://asciidoctor.org"]/following-sibling::text()[starts-with(.,"?)")]', result, 1
  end

"###
    );

    let doc =
        Parser::default().parse(r##"(Is the homepage for Asciidoctor https://asciidoctor.org?)"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"(Is the homepage for Asciidoctor <a href="https://asciidoctor.org" class="bare">https://asciidoctor.org</a>?)"##
    );
}

#[test]
fn qualified_url_with_trailing_semi_colon() {
    verifies!(
        r###"
  test 'qualified url with trailing semi-colon' do
    result = convert_string_to_embedded 'https://asciidoctor.org; where text gets parsed'
    assert_xpath '//a[@href="https://asciidoctor.org"][text()="https://asciidoctor.org"]', result, 1
    assert_xpath '//a[@href="https://asciidoctor.org"][text()="https://asciidoctor.org"]/following-sibling::text()[starts-with(.,";")]', result, 1
  end

"###
    );

    let doc = Parser::default().parse(r##"https://asciidoctor.org; where text gets parsed"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="https://asciidoctor.org" class="bare">https://asciidoctor.org</a>; where text gets parsed"##
    );
}

#[test]
fn qualified_url_with_trailing_colon() {
    verifies!(
        r###"
  test 'qualified url with trailing colon' do
    result = convert_string_to_embedded 'https://asciidoctor.org: where text gets parsed'
    assert_xpath '//a[@href="https://asciidoctor.org"][text()="https://asciidoctor.org"]', result, 1
    assert_xpath '//a[@href="https://asciidoctor.org"][text()="https://asciidoctor.org"]/following-sibling::text()[starts-with(.,":")]', result, 1
  end

"###
    );

    let doc = Parser::default().parse(r##"https://asciidoctor.org: where text gets parsed"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="https://asciidoctor.org" class="bare">https://asciidoctor.org</a>: where text gets parsed"##
    );
}

#[test]
fn qualified_url_in_round_brackets_with_trailing_colon() {
    verifies!(
        r###"
  test 'qualified url in round brackets with trailing colon' do
    result = convert_string_to_embedded '(https://asciidoctor.org): where text gets parsed'
    assert_xpath '//a[@href="https://asciidoctor.org"][text()="https://asciidoctor.org"]', result, 1
    assert_xpath '//a[@href="https://asciidoctor.org"][text()="https://asciidoctor.org"]/following-sibling::text()[starts-with(.,"):")]', result, 1
  end

"###
    );

    let doc = Parser::default().parse(r##"(https://asciidoctor.org): where text gets parsed"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"(<a href="https://asciidoctor.org" class="bare">https://asciidoctor.org</a>): where text gets parsed"##
    );
}

#[test]
fn qualified_url_with_trailing_round_bracket_followed_by_colon() {
    verifies!(
        r###"
  test 'qualified url with trailing round bracket followed by colon' do
    result = convert_string_to_embedded '(from https://asciidoctor.org): where text gets parsed'
    assert_xpath '//a[@href="https://asciidoctor.org"][text()="https://asciidoctor.org"]', result, 1
    assert_xpath '//a[@href="https://asciidoctor.org"][text()="https://asciidoctor.org"]/following-sibling::text()[starts-with(., "):")]', result, 1
  end

"###
    );

    let doc =
        Parser::default().parse(r##"(from https://asciidoctor.org): where text gets parsed"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"(from <a href="https://asciidoctor.org" class="bare">https://asciidoctor.org</a>): where text gets parsed"##
    );
}

#[test]
fn qualified_url_in_round_brackets_with_trailing_semi_colon() {
    verifies!(
        r###"
  test 'qualified url in round brackets with trailing semi-colon' do
    result = convert_string_to_embedded '(https://asciidoctor.org); where text gets parsed'
    assert_xpath '//a[@href="https://asciidoctor.org"][text()="https://asciidoctor.org"]', result, 1
    assert_xpath '//a[@href="https://asciidoctor.org"][text()="https://asciidoctor.org"]/following-sibling::text()[starts-with(., ");")]', result, 1
  end

"###
    );

    let doc = Parser::default().parse(r##"(https://asciidoctor.org); where text gets parsed"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"(<a href="https://asciidoctor.org" class="bare">https://asciidoctor.org</a>); where text gets parsed"##
    );
}

#[test]
fn qualified_url_with_trailing_round_bracket_followed_by_semi_colon() {
    verifies!(
        r###"
  test 'qualified url with trailing round bracket followed by semi-colon' do
    result = convert_string_to_embedded '(from https://asciidoctor.org); where text gets parsed'
    assert_xpath '//a[@href="https://asciidoctor.org"][text()="https://asciidoctor.org"]', result, 1
    assert_xpath '//a[@href="https://asciidoctor.org"][text()="https://asciidoctor.org"]/following-sibling::text()[starts-with(., ");")]', result, 1
  end

"###
    );

    let doc =
        Parser::default().parse(r##"(from https://asciidoctor.org); where text gets parsed"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"(from <a href="https://asciidoctor.org" class="bare">https://asciidoctor.org</a>); where text gets parsed"##
    );
}

#[test]
fn uri_scheme_with_trailing_characters_should_not_be_converted_to_a_link() {
    verifies!(
        r###"
  test 'URI scheme with trailing characters should not be converted to a link' do
    comma = ','
    input_sources = %W(
      http://;
      file://:
      irc://#{comma}
    )
    expected_outputs = %W(
      http://;
      file://:
      irc://#{comma}
    )
    input_sources.each_with_index do |input_source, i|
      expected_output = expected_outputs[i]
      actual = block_from_string input_source
      assert_equal expected_output, actual.content
    end
  end

"###
    );

    for (src, expected) in [
        ("http://;", "http://;"),
        ("file://:", "file://:"),
        ("irc://,", "irc://,"),
    ] {
        let doc = Parser::default().parse(src);
        assert_eq!(rendered_paragraphs(&doc)[0], expected);
    }
}

#[test]
fn bare_uri_scheme_enclosed_in_brackets_should_not_be_converted_to_link() {
    verifies!(
        r###"
  test 'bare URI scheme enclosed in brackets should not be converted to link' do
    input_sources = %w(
      (https://)
      <ftp://>
    )
    expected_outputs = %w(
      (https://)
      &lt;ftp://&gt;
    )
    input_sources.each_with_index do |input_source, i|
      expected_output = expected_outputs[i]
      actual = block_from_string input_source
      assert_equal expected_output, actual.content
    end
  end

"###
    );

    let doc = Parser::default().parse("(https://)");
    assert_eq!(rendered_paragraphs(&doc)[0], "(https://)");
    let doc = Parser::default().parse("<ftp://>");
    assert_eq!(rendered_paragraphs(&doc)[0], "&lt;ftp://&gt;");
}

#[test]
fn qualified_url_containing_round_brackets() {
    verifies!(
        r###"
  test 'qualified url containing round brackets' do
    assert_xpath '//a[@href="http://jruby.org/apidocs/org/jruby/Ruby.html#addModule(org.jruby.RubyModule)"][text()="addModule() adds a Ruby module"]', convert_string('http://jruby.org/apidocs/org/jruby/Ruby.html#addModule(org.jruby.RubyModule)[addModule() adds a Ruby module]'), 1
  end

"###
    );

    let doc = Parser::default().parse(r##"http://jruby.org/apidocs/org/jruby/Ruby.html#addModule(org.jruby.RubyModule)[addModule() adds a Ruby module]"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="http://jruby.org/apidocs/org/jruby/Ruby.html#addModule(org.jruby.RubyModule)">addModule() adds a Ruby module</a>"##
    );
}

#[test]
fn qualified_url_adjacent_to_text_in_square_brackets() {
    verifies!(
        r###"
  test 'qualified url adjacent to text in square brackets' do
    assert_xpath '//a[@href="http://asciidoc.org"][text()="AsciiDoc"]', convert_string(']http://asciidoc.org[AsciiDoc] project page.'), 1
  end

"###
    );

    let doc = Parser::default().parse(r##"]http://asciidoc.org[AsciiDoc] project page."##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"]<a href="http://asciidoc.org">AsciiDoc</a> project page."##
    );
}

#[test]
fn qualified_url_adjacent_to_text_in_round_brackets() {
    verifies!(
        r###"
  test 'qualified url adjacent to text in round brackets' do
    assert_xpath '//a[@href="http://asciidoc.org"][text()="AsciiDoc"]', convert_string(')http://asciidoc.org[AsciiDoc] project page.'), 1
  end

"###
    );

    let doc = Parser::default().parse(r##")http://asciidoc.org[AsciiDoc] project page."##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##")<a href="http://asciidoc.org">AsciiDoc</a> project page."##
    );
}

// Surfaced incompatibility: Asciidoctor recognizes a URL macro preceded by a
// no-break space; this crate does not treat U+00A0 as a leading boundary and
// leaves the macro as literal text. Tracked in #768.
non_normative!(
    r###"
  test 'qualified url following no-break space' do
    assert_xpath '//a[@href="http://asciidoc.org"][text()="AsciiDoc"]', convert_string(%(#{[0xa0].pack 'U1'}http://asciidoc.org[AsciiDoc] project page.)), 1
  end

"###
);

#[test]
fn qualified_url_following_smart_apostrophe() {
    verifies!(
        r###"
  test 'qualified url following smart apostrophe' do
    output = convert_string_to_embedded("l&#8217;http://www.irit.fr[IRIT]")
    assert_match(/l&#8217;<a href=/, output)
  end

"###
    );

    let doc = Parser::default().parse(r##"l&#8217;http://www.irit.fr[IRIT]"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"l&#8217;<a href="http://www.irit.fr">IRIT</a>"##
    );
}

#[test]
fn should_convert_qualified_url_as_macro_enclosed_in_double_quotes() {
    verifies!(
        r###"
  test 'should convert qualified url as macro enclosed in double quotes' do
    output = convert_string_to_embedded('"https://asciidoctor.org[]"')
    assert_include '"<a href="https://asciidoctor.org" class="bare">https://asciidoctor.org</a>"', output
  end

"###
    );

    let doc = Parser::default().parse(r##""https://asciidoctor.org[]""##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##""<a href="https://asciidoctor.org" class="bare">https://asciidoctor.org</a>""##
    );
}

#[test]
fn should_convert_qualified_url_as_macro_enclosed_in_single_quotes() {
    verifies!(
        r###"
  test 'should convert qualified url as macro enclosed in single quotes' do
    output = convert_string_to_embedded('\'https://asciidoctor.org[]\'')
    assert_include '\'<a href="https://asciidoctor.org" class="bare">https://asciidoctor.org</a>\'', output
  end

"###
    );

    let doc = Parser::default().parse("'https://asciidoctor.org[]'");
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"'<a href="https://asciidoctor.org" class="bare">https://asciidoctor.org</a>'"##
    );
}

#[test]
fn should_convert_qualified_url_as_macro_with_trailing_period() {
    verifies!(
        r###"
  test 'should convert qualified url as macro with trailing period' do
    result = convert_string_to_embedded 'Information about the https://symbols.example.org/.[.] character.'
    assert_xpath '//a[@href="https://symbols.example.org/."][text()="."]', result, 1
  end

"###
    );

    let doc = Parser::default()
        .parse(r##"Information about the https://symbols.example.org/.[.] character."##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"Information about the <a href="https://symbols.example.org/.">.</a> character."##
    );
}

#[test]
fn qualified_url_using_invalid_link_macro_should_not_create_link() {
    verifies!(
        r###"
  test 'qualified url using invalid link macro should not create link' do
    assert_xpath '//a', convert_string('link:http://asciidoc.org is the project page for AsciiDoc.'), 0
  end

"###
    );

    let doc = Parser::default().parse("link:http://asciidoc.org is the project page for AsciiDoc.");
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        "link:http://asciidoc.org is the project page for AsciiDoc."
    );
}

#[test]
fn escaped_inline_qualified_url_should_not_create_link() {
    verifies!(
        r###"
  test 'escaped inline qualified url should not create link' do
    assert_xpath '//a', convert_string('\http://asciidoc.org is the project page for AsciiDoc.'), 0
  end

"###
    );

    let doc =
        Parser::default().parse(r##"\http://asciidoc.org is the project page for AsciiDoc."##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"http://asciidoc.org is the project page for AsciiDoc."##
    );
}

#[test]
fn escaped_inline_qualified_url_as_macro_should_not_create_link() {
    verifies!(
        r###"
  test 'escaped inline qualified url as macro should not create link' do
    output = convert_string '\http://asciidoc.org[asciidoc.org] is the project page for AsciiDoc.'
    assert_xpath '//a', output, 0
    assert_xpath '//p[starts-with(text(), "http://asciidoc.org[asciidoc.org]")]', output, 1
  end

"###
    );

    let doc = Parser::default()
        .parse(r##"\http://asciidoc.org[asciidoc.org] is the project page for AsciiDoc."##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"http://asciidoc.org[asciidoc.org] is the project page for AsciiDoc."##
    );
}

#[test]
fn url_in_link_macro_with_at_sign_should_not_create_mailto_link() {
    verifies!(
        r###"
  test 'url in link macro with at (@) sign should not create mailto link' do
    assert_xpath '//a[@href="http://xircles.codehaus.org/lists/dev@geb.codehaus.org"][text()="subscribe"]', convert_string('http://xircles.codehaus.org/lists/dev@geb.codehaus.org[subscribe]')
  end

"###
    );

    let doc = Parser::default()
        .parse(r##"http://xircles.codehaus.org/lists/dev@geb.codehaus.org[subscribe]"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="http://xircles.codehaus.org/lists/dev@geb.codehaus.org">subscribe</a>"##
    );
}

#[test]
fn implicit_url_with_at_sign_should_not_create_mailto_link() {
    verifies!(
        r###"
  test 'implicit url with at (@) sign should not create mailto link' do
    assert_xpath '//a[@href="http://xircles.codehaus.org/lists/dev@geb.codehaus.org"][text()="http://xircles.codehaus.org/lists/dev@geb.codehaus.org"]', convert_string('http://xircles.codehaus.org/lists/dev@geb.codehaus.org')
  end

"###
    );

    let doc =
        Parser::default().parse(r##"http://xircles.codehaus.org/lists/dev@geb.codehaus.org"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="http://xircles.codehaus.org/lists/dev@geb.codehaus.org" class="bare">http://xircles.codehaus.org/lists/dev@geb.codehaus.org</a>"##
    );
}

#[test]
fn escaped_inline_qualified_url_using_macro_syntax_should_not_create_link() {
    verifies!(
        r###"
  test 'escaped inline qualified url using macro syntax should not create link' do
    assert_xpath '//a', convert_string('\http://asciidoc.org[AsciiDoc] is the key to good docs.'), 0
  end

"###
    );

    let doc =
        Parser::default().parse(r##"\http://asciidoc.org[AsciiDoc] is the key to good docs."##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"http://asciidoc.org[AsciiDoc] is the key to good docs."##
    );
}

#[test]
fn inline_qualified_url_followed_by_a_newline_should_not_include_newline_in_link() {
    verifies!(
        r###"
  test 'inline qualified url followed by a newline should not include newline in link' do
    assert_xpath '//a[@href="https://github.com/asciidoctor"]', convert_string("The source code for Asciidoctor can be found at https://github.com/asciidoctor\nwhich is a GitHub organization."), 1
  end

"###
    );

    let doc = Parser::default().parse(
        "The source code for Asciidoctor can be found at https://github.com/asciidoctor\nwhich is a GitHub organization.",
    );
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        "The source code for Asciidoctor can be found at <a href=\"https://github.com/asciidoctor\" class=\"bare\">https://github.com/asciidoctor</a>\nwhich is a GitHub organization."
    );
}

#[test]
fn qualified_url_divided_by_newline_using_macro_syntax_should_not_create_link() {
    verifies!(
        r###"
  test 'qualified url divided by newline using macro syntax should not create link' do
    assert_xpath '//a', convert_string("The source code for Asciidoctor can be found at link:https://github.com/asciidoctor\n[]which is a GitHub organization."), 0
  end

"###
    );

    let doc = Parser::default().parse(
        "The source code for Asciidoctor can be found at link:https://github.com/asciidoctor\n[]which is a GitHub organization.",
    );
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        "The source code for Asciidoctor can be found at link:https://github.com/asciidoctor\n[]which is a GitHub organization."
    );
}

#[test]
fn qualified_url_containing_whitespace_using_macro_syntax_should_not_create_link() {
    verifies!(
        r###"
  test 'qualified url containing whitespace using macro syntax should not create link' do
    assert_xpath '//a', convert_string('I often need to refer to the chapter on link:http://asciidoc.org?q=attribute references[Attribute References].'), 0
  end

"###
    );

    let doc = Parser::default().parse(
        "I often need to refer to the chapter on link:http://asciidoc.org?q=attribute references[Attribute References].",
    );
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        "I often need to refer to the chapter on link:http://asciidoc.org?q=attribute references[Attribute References]."
    );
}

#[test]
fn qualified_url_containing_an_encoded_space_using_macro_syntax_should_create_a_link() {
    verifies!(
        r###"
  test 'qualified url containing an encoded space using macro syntax should create a link' do
    assert_xpath '//a', convert_string('I often need to refer to the chapter on link:http://asciidoc.org?q=attribute%20references[Attribute References].'), 1
  end

"###
    );

    let doc = Parser::default().parse(r##"I often need to refer to the chapter on link:http://asciidoc.org?q=attribute%20references[Attribute References]."##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"I often need to refer to the chapter on <a href="http://asciidoc.org?q=attribute%20references">Attribute References</a>."##
    );
}

#[test]
fn inline_quoted_qualified_url_should_not_consume_surrounding_angled_brackets() {
    verifies!(
        r###"
  test 'inline quoted qualified url should not consume surrounding angled brackets' do
    assert_xpath '//a[@href="https://github.com/asciidoctor"]', convert_string('Asciidoctor GitHub organization: <**https://github.com/asciidoctor**>'), 1
  end

"###
    );

    let doc = Parser::default()
        .parse(r##"Asciidoctor GitHub organization: <**https://github.com/asciidoctor**>"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"Asciidoctor GitHub organization: &lt;<strong><a href="https://github.com/asciidoctor" class="bare">https://github.com/asciidoctor</a></strong>&gt;"##
    );
}

#[test]
fn link_with_quoted_text_should_not_be_separated_into_attributes_when_text_contains_an_equal_sign()
{
    verifies!(
        r###"
  test 'link with quoted text should not be separated into attributes when text contains an equal sign' do
    assert_xpath '//a[@href="http://search.example.com"][text()="Google, Yahoo, Bing = Search Engines"]', convert_string_to_embedded('http://search.example.com["Google, Yahoo, Bing = Search Engines"]'), 1
  end

"###
    );

    let doc = Parser::default()
        .parse(r##"http://search.example.com["Google, Yahoo, Bing = Search Engines"]"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="http://search.example.com">Google, Yahoo, Bing = Search Engines</a>"##
    );
}

#[test]
fn should_leave_link_text_as_is_if_it_contains_an_equals_sign_but_no_attributes_are_found() {
    verifies!(
        r###"
  test 'should leave link text as is if it contains an equals sign but no attributes are found' do
    assert_xpath %(//a[@href="https://example.com"][text()="What You Need\n= What You Get"]), convert_string_to_embedded(%(https://example.com[What You Need\n= What You Get])), 1
  end

"###
    );

    let doc = Parser::default().parse("https://example.com[What You Need\n= What You Get]");
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        "<a href=\"https://example.com\">What You Need\n= What You Get</a>"
    );
}

#[test]
fn link_with_quoted_text_but_no_equal_sign_should_carry_quotes_over_to_output() {
    verifies!(
        r###"
  test 'link with quoted text but no equal sign should carry quotes over to output' do
    assert_xpath %(//a[@href="http://search.example.com"][text()='"Google, Yahoo, Bing"']), convert_string_to_embedded('http://search.example.com["Google, Yahoo, Bing"]'), 1
  end

"###
    );

    let doc = Parser::default().parse(r##"http://search.example.com["Google, Yahoo, Bing"]"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="http://search.example.com">"Google, Yahoo, Bing"</a>"##
    );
}

#[test]
fn link_with_comma_in_text_but_no_equal_sign_should_not_be_separated_into_attributes() {
    verifies!(
        r###"
  test 'link with comma in text but no equal sign should not be separated into attributes' do
    assert_xpath '//a[@href="http://search.example.com"][text()="Google, Yahoo, Bing"]', convert_string_to_embedded('http://search.example.com[Google, Yahoo, Bing]'), 1
  end

"###
    );

    let doc = Parser::default().parse(r##"http://search.example.com[Google, Yahoo, Bing]"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="http://search.example.com">Google, Yahoo, Bing</a>"##
    );
}

#[test]
fn link_with_formatted_wrapped_text_should_not_be_separated_into_attributes() {
    verifies!(
        r###"
  test 'link with formatted wrapped text should not be separated into attributes' do
    result = convert_string_to_embedded %(https://example.com[[.role]#Foo\nBar#])
    assert_include %(<a href="https://example.com"><span class="role">Foo\nBar</span></a>), result
  end

"###
    );

    let doc = Parser::default().parse("https://example.com[[.role]#Foo\nBar#]");
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        "<a href=\"https://example.com\"><span class=\"role\">Foo\nBar</span></a>"
    );
}

#[test]
fn should_process_role_and_window_attributes_on_link() {
    verifies!(
        r###"
  test 'should process role and window attributes on link' do
    assert_xpath '//a[@href="http://google.com"][@class="external"][@target="_blank"]', convert_string_to_embedded('http://google.com[Google, role=external, window="_blank"]'), 1
  end

"###
    );

    let doc =
        Parser::default().parse(r##"http://google.com[Google, role=external, window="_blank"]"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="http://google.com" class="external" target="_blank" rel="noopener">Google</a>"##
    );
}

#[test]
fn should_parse_link_with_wrapped_text_that_includes_attributes() {
    verifies!(
        r###"
  test 'should parse link with wrapped text that includes attributes' do
    result = convert_string_to_embedded %(https://example.com[Foo\nBar,role=foobar])
    assert_include %(<a href="https://example.com" class="foobar">Foo Bar</a>), result
  end

"###
    );

    let doc = Parser::default().parse("https://example.com[Foo\nBar,role=foobar]");
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="https://example.com" class="foobar">Foo Bar</a>"##
    );
}

#[test]
fn link_macro_with_attributes_but_no_text_should_use_url_as_text() {
    verifies!(
        r###"
  test 'link macro with attributes but no text should use URL as text' do
    url = 'https://fonts.googleapis.com/css?family=Roboto:400,400italic,'
    assert_xpath %(//a[@href="#{url}"][text()="#{url}"]), convert_string_to_embedded(%(link:#{url}[family=Roboto,weight=400])), 1
  end

"###
    );

    let doc = Parser::default().parse(r##"link:https://fonts.googleapis.com/css?family=Roboto:400,400italic,[family=Roboto,weight=400]"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="https://fonts.googleapis.com/css?family=Roboto:400,400italic," class="bare">https://fonts.googleapis.com/css?family=Roboto:400,400italic,</a>"##
    );
}

#[test]
fn link_macro_with_attributes_but_blank_text_should_use_url_as_text() {
    verifies!(
        r###"
  test 'link macro with attributes but blank text should use URL as text' do
    url = 'https://fonts.googleapis.com/css?family=Roboto:400,400italic,'
    assert_xpath %(//a[@href="#{url}"][text()="#{url}"]), convert_string_to_embedded(%(link:#{url}[,family=Roboto,weight=400])), 1
  end

"###
    );

    let doc = Parser::default().parse(r##"link:https://fonts.googleapis.com/css?family=Roboto:400,400italic,[,family=Roboto,weight=400]"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="https://fonts.googleapis.com/css?family=Roboto:400,400italic," class="bare">https://fonts.googleapis.com/css?family=Roboto:400,400italic,</a>"##
    );
}

#[test]
fn link_macro_with_comma_but_no_explicit_attributes_in_text_should_not_parse_text() {
    verifies!(
        r###"
  test 'link macro with comma but no explicit attributes in text should not parse text' do
    url = 'https://fonts.googleapis.com/css?family=Roboto:400,400italic,'
    assert_xpath %(//a[@href="#{url}"][text()="Roboto,400"]), convert_string_to_embedded(%(link:#{url}[Roboto,400])), 1
  end

"###
    );

    let doc = Parser::default().parse(
        r##"link:https://fonts.googleapis.com/css?family=Roboto:400,400italic,[Roboto,400]"##,
    );
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="https://fonts.googleapis.com/css?family=Roboto:400,400italic,">Roboto,400</a>"##
    );
}

#[test]
fn link_macro_should_support_id_and_role_attributes() {
    verifies!(
        r###"
  test 'link macro should support id and role attributes' do
    url = 'https://fonts.googleapis.com/css?family=Roboto:400'
    assert_xpath %(//a[@href="#{url}"][@id="roboto-regular"][@class="bare font"][text()="#{url}"]), convert_string_to_embedded(%(link:#{url}[,id=roboto-regular,role=font])), 1
  end

"###
    );

    let doc = Parser::default().parse(r##"link:https://fonts.googleapis.com/css?family=Roboto:400[,id=roboto-regular,role=font]"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="https://fonts.googleapis.com/css?family=Roboto:400" id="roboto-regular" class="bare font">https://fonts.googleapis.com/css?family=Roboto:400</a>"##
    );
}

#[test]
fn link_text_that_ends_in_should_set_link_window_to_blank() {
    verifies!(
        r###"
  test 'link text that ends in ^ should set link window to _blank' do
    assert_xpath '//a[@href="http://google.com"][@target="_blank"]', convert_string_to_embedded('http://google.com[Google^]'), 1
  end

"###
    );

    let doc = Parser::default().parse(r##"http://google.com[Google^]"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="http://google.com" target="_blank" rel="noopener">Google</a>"##
    );
}

#[test]
fn rel_noopener_should_be_added_to_a_link_that_targets_the_blank_window() {
    verifies!(
        r###"
  test 'rel=noopener should be added to a link that targets the _blank window' do
    assert_xpath '//a[@href="http://google.com"][@target="_blank"][@rel="noopener"]', convert_string_to_embedded('http://google.com[Google^]'), 1
  end

"###
    );

    let doc = Parser::default().parse(r##"http://google.com[Google^]"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="http://google.com" target="_blank" rel="noopener">Google</a>"##
    );
}

#[test]
fn rel_noopener_should_be_added_to_a_link_that_targets_a_named_window_when_the_noopener_option_is_set()
 {
    verifies!(
        r###"
  test 'rel=noopener should be added to a link that targets a named window when the noopener option is set' do
    assert_xpath '//a[@href="http://google.com"][@target="name"][@rel="noopener"]', convert_string_to_embedded('http://google.com[Google,window=name,opts=noopener]'), 1
  end

"###
    );

    let doc = Parser::default().parse(r##"http://google.com[Google,window=name,opts=noopener]"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="http://google.com" target="name" rel="noopener">Google</a>"##
    );
}

#[test]
fn rel_noopener_should_not_be_added_to_a_link_if_it_does_not_target_a_window() {
    verifies!(
        r###"
  test 'rel=noopener should not be added to a link if it does not target a window' do
    result = convert_string_to_embedded 'http://google.com[Google,opts=noopener]'
    assert_xpath '//a[@href="http://google.com"]', result, 1
    assert_xpath '//a[@href="http://google.com"][@rel="noopener"]', result, 0
  end

"###
    );

    let doc = Parser::default().parse(r##"http://google.com[Google,opts=noopener]"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="http://google.com">Google</a>"##
    );
}

#[test]
fn rel_nofollow_should_be_added_to_a_link_when_the_nofollow_option_is_set() {
    verifies!(
        r###"
  test 'rel=nofollow should be added to a link when the nofollow option is set' do
    assert_xpath '//a[@href="http://google.com"][@target="name"][@rel="nofollow noopener"]', convert_string_to_embedded('http://google.com[Google,window=name,opts="nofollow,noopener"]'), 1
  end

"###
    );

    let doc = Parser::default()
        .parse(r##"http://google.com[Google,window=name,opts="nofollow,noopener"]"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="http://google.com" target="name" rel="nofollow noopener">Google</a>"##
    );
}

#[test]
fn id_attribute_on_link_is_processed() {
    verifies!(
        r###"
  test 'id attribute on link is processed' do
    assert_xpath '//a[@href="http://google.com"][@id="link-1"]', convert_string_to_embedded('http://google.com[Google, id="link-1"]'), 1
  end

"###
    );

    let doc = Parser::default().parse(r##"http://google.com[Google, id="link-1"]"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="http://google.com" id="link-1">Google</a>"##
    );
}

#[test]
fn title_attribute_on_link_is_processed() {
    verifies!(
        r###"
  test 'title attribute on link is processed' do
    assert_xpath '//a[@href="http://google.com"][@title="title-1"]', convert_string_to_embedded('http://google.com[Google, title="title-1"]'), 1
  end

"###
    );

    let doc = Parser::default().parse(r##"http://google.com[Google, title="title-1"]"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="http://google.com" title="title-1">Google</a>"##
    );
}

#[test]
fn inline_irc_link() {
    verifies!(
        r###"
  test 'inline irc link' do
    assert_xpath '//a[@href="irc://irc.freenode.net"][text()="irc://irc.freenode.net"]', convert_string_to_embedded('irc://irc.freenode.net'), 1
  end

"###
    );

    let doc = Parser::default().parse(r##"irc://irc.freenode.net"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="irc://irc.freenode.net" class="bare">irc://irc.freenode.net</a>"##
    );
}

#[test]
fn inline_irc_link_with_text() {
    verifies!(
        r###"
  test 'inline irc link with text' do
    assert_xpath '//a[@href="irc://irc.freenode.net"][text()="Freenode IRC"]', convert_string_to_embedded('irc://irc.freenode.net[Freenode IRC]'), 1
  end

"###
    );

    let doc = Parser::default().parse(r##"irc://irc.freenode.net[Freenode IRC]"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="irc://irc.freenode.net">Freenode IRC</a>"##
    );
}

#[test]
fn inline_ref() {
    verifies!(
        r###"
  test 'inline ref' do
    variations = %w([[tigers]] anchor:tigers[])
    variations.each do |anchor|
      doc = document_from_string %(Here you can read about tigers.#{anchor})
      output = doc.convert
      assert_kind_of Asciidoctor::Inline, doc.catalog[:refs]['tigers']
      assert_nil doc.catalog[:refs]['tigers'].text
      assert_xpath '//a[@id="tigers"]', output, 1
      assert_xpath '//a[@id="tigers"]/child::text()', output, 0
    end
  end

"###
    );

    for anchor in ["[[tigers]]", "anchor:tigers[]"] {
        let src = ["Here you can read about tigers.", anchor].concat();
        let doc = Parser::default().parse(&src);
        assert_eq!(
            rendered_paragraphs(&doc)[0],
            r##"Here you can read about tigers.<a id="tigers"></a>"##
        );
        let entry = doc.catalog().get_ref("tigers").unwrap();
        assert_eq!(entry.reftext, None);
    }
}

#[test]
fn escaped_inline_ref() {
    verifies!(
        r###"
  test 'escaped inline ref' do
    variations = %w([[tigers]] anchor:tigers[])
    variations.each do |anchor|
      doc = document_from_string %(Here you can read about tigers.\\#{anchor})
      output = doc.convert
      refute doc.catalog[:refs].key?('tigers')
      assert_xpath '//a[@id="tigers"]', output, 0
    end
  end

"###
    );

    for anchor in ["[[tigers]]", "anchor:tigers[]"] {
        let src = ["Here you can read about tigers.\\", anchor].concat();
        let doc = Parser::default().parse(&src);
        assert!(doc.catalog().get_ref("tigers").is_none());
    }
}

#[test]
fn inline_ref_can_start_with_colon() {
    verifies!(
        r###"
  test 'inline ref can start with colon' do
    input = '[[:idname]] text'
    output = convert_string_to_embedded input
    assert_xpath '//a[@id=":idname"]', output, 1
  end

"###
    );

    let doc = Parser::default().parse("[[:idname]] text");
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a id=":idname"></a> text"##
    );
    assert!(doc.catalog().get_ref(":idname").is_some());
}

#[test]
fn inline_ref_cannot_start_with_digit() {
    verifies!(
        r###"
  test 'inline ref cannot start with digit' do
    input = '[[1-install]] text'
    output = convert_string_to_embedded input
    assert_includes output, '[[1-install]]'
    assert_xpath '//a[@id = "1-install"]', output, 0
  end

"###
    );

    let doc = Parser::default().parse("[[1-install]] text");
    assert_eq!(rendered_paragraphs(&doc)[0], "[[1-install]] text");
    assert!(doc.catalog().get_ref("1-install").is_none());
}

#[test]
fn reftext_of_shorthand_inline_ref_cannot_resolve_to_empty() {
    verifies!(
        r###"
  test 'reftext of shorthand inline ref cannot resolve to empty' do
    input = '[[no-such-id,{empty}]]text'
    doc = document_from_string input
    assert_empty doc.catalog[:refs]
    output = doc.convert standalone: false
    assert_includes output, (input.sub '{empty}', '')
  end

"###
    );

    let doc = Parser::default().parse("[[no-such-id,{empty}]]text");
    assert!(doc.catalog().is_empty());
    assert_eq!(rendered_paragraphs(&doc)[0], "[[no-such-id,]]text");
}

#[test]
fn reftext_of_macro_inline_ref_can_resolve_to_empty() {
    verifies!(
        r###"
  test 'reftext of macro inline ref can resolve to empty' do
    input = 'anchor:id-only[{empty}]text\n\nsee <<id-only>>'
    doc = document_from_string input
    assert doc.catalog[:refs].key? 'id-only'
    output = doc.convert standalone: false
    assert_xpath '//a[@id="id-only"]', output, 1
    assert_xpath '//a[@href="#id-only"]', output, 1
    assert_xpath '//a[@href="#id-only"][text()="[id-only]"]', output, 1
  end

"###
    );

    // The Ruby input is single-quoted, so the `\n` sequences are literal
    // (this is all one line).
    let doc = Parser::default().parse(r"anchor:id-only[{empty}]text\n\nsee <<id-only>>");
    assert!(doc.catalog().get_ref("id-only").is_some());
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a id="id-only"></a>text\n\nsee <a href="#id-only">[id-only]</a>"##
    );
}

#[test]
fn inline_ref_with_reftext() {
    verifies!(
        r###"
  test 'inline ref with reftext' do
    %w([[tigers,Tigers]] anchor:tigers[Tigers]).each do |anchor|
      doc = document_from_string %(Here you can read about tigers.#{anchor})
      output = doc.convert
      assert_kind_of Asciidoctor::Inline, doc.catalog[:refs]['tigers']
      assert_equal 'Tigers', doc.catalog[:refs]['tigers'].text
      assert_xpath '//a[@id="tigers"]', output, 1
      assert_xpath '//a[@id="tigers"]/child::text()', output, 0
    end
  end

"###
    );

    for anchor in ["[[tigers,Tigers]]", "anchor:tigers[Tigers]"] {
        let src = ["Here you can read about tigers.", anchor].concat();
        let doc = Parser::default().parse(&src);
        assert_eq!(
            rendered_paragraphs(&doc)[0],
            r##"Here you can read about tigers.<a id="tigers"></a>"##
        );
        let entry = doc.catalog().get_ref("tigers").unwrap();
        assert_eq!(entry.reftext.as_deref(), Some("Tigers"));
    }
}

// Backend-specific test: DocBook output is out of scope for this crate.
non_normative!(
    r###"
  test 'should encode double quotes in reftext of anchor macro in DocBook output' do
    input = 'anchor:uncola[the "un"-cola]'
    result = convert_inline_string input, backend: :docbook
    assert_equal '<anchor xml:id="uncola" xreflabel="the &quot;un&quot;-cola"/>', result
  end

"###
);

#[test]
fn should_substitute_attribute_references_in_reftext_when_registering_inline_ref() {
    verifies!(
        r###"
  test 'should substitute attribute references in reftext when registering inline ref' do
    %w([[tigers,{label-tigers}]] anchor:tigers[{label-tigers}]).each do |anchor|
      doc = document_from_string %(Here you can read about tigers.#{anchor}), attributes: { 'label-tigers' => 'Tigers' }
      doc.convert
      assert_kind_of Asciidoctor::Inline, doc.catalog[:refs]['tigers']
      assert_equal 'Tigers', doc.catalog[:refs]['tigers'].text
    end
  end

"###
    );

    for anchor in ["[[tigers,{label-tigers}]]", "anchor:tigers[{label-tigers}]"] {
        let src = ["Here you can read about tigers.", anchor].concat();
        let doc = doc_with(&src, &[("label-tigers", "Tigers")]);
        let entry = doc.catalog().get_ref("tigers").unwrap();
        assert_eq!(entry.reftext.as_deref(), Some("Tigers"));
    }
}

// Backend-specific test: DocBook output is out of scope for this crate.
non_normative!(
    r###"
  test 'inline ref with reftext converted to DocBook' do
    %w([[tigers,<Tigers>]] anchor:tigers[<Tigers>]).each do |anchor|
      doc = document_from_string %(Here you can read about tigers.#{anchor}), backend: :docbook
      output = doc.convert standalone: false
      assert_kind_of Asciidoctor::Inline, doc.catalog[:refs]['tigers']
      assert_equal '<Tigers>', doc.catalog[:refs]['tigers'].text
      assert_includes output, '<anchor xml:id="tigers" xreflabel="&lt;Tigers&gt;"/>'
    end
  end

"###
);

// Surfaced incompatibility: Asciidoctor does not register a bibliography anchor
// (`[[[label]]]`) found in prose; this crate registers `label`. Tracked in
// #769.
non_normative!(
    r###"
  test 'does not match bibliography anchor in prose when scanning for inline anchor' do
    doc = document_from_string 'Use [[[label]]] to assign a label to a bibliography entry, but not in a paragraph.'
    refute doc.catalog[:refs].key? 'label'
  end

"###
);

#[test]
fn repeating_inline_anchor_macro_with_empty_reftext() {
    verifies!(
        r###"
  test 'repeating inline anchor macro with empty reftext' do
    input = 'anchor:one[] anchor:two[] anchor:three[]'
    result = convert_inline_string input
    assert_equal '<a id="one"></a> <a id="two"></a> <a id="three"></a>', result
  end

"###
    );

    let doc = Parser::default().parse(r##"anchor:one[] anchor:two[] anchor:three[]"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a id="one"></a> <a id="two"></a> <a id="three"></a>"##
    );
}

#[test]
fn mixed_inline_anchor_macro_and_anchor_shorthand_with_empty_reftext() {
    verifies!(
        r###"
  test 'mixed inline anchor macro and anchor shorthand with empty reftext' do
    input = 'anchor:one[][[two]]anchor:three[][[four]]anchor:five[]'
    result = convert_inline_string input
    assert_equal '<a id="one"></a><a id="two"></a><a id="three"></a><a id="four"></a><a id="five"></a>', result
  end

"###
    );

    let doc =
        Parser::default().parse(r##"anchor:one[][[two]]anchor:three[][[four]]anchor:five[]"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a id="one"></a><a id="two"></a><a id="three"></a><a id="four"></a><a id="five"></a>"##
    );
}

// Backend-specific test: DocBook output is out of scope for this crate.
non_normative!(
    r###"
  test 'assigns xreflabel value for anchor macro without reftext in DocBook output' do
    ['anchor:foo[]bar', '[[foo]]bar'].each do |input|
      result = convert_inline_string input, backend: :docbook
      assert_equal '<anchor xml:id="foo" xreflabel="[foo]"/>bar', result
    end
  end

"###
);

#[test]
fn should_remove_trailing_space_on_reftext_of_inline_anchor_shorthand() {
    verifies!(
        r###"
  test 'should remove trailing space on reftext of inline anchor shorthand' do
    input = <<~'EOS'
    see <<foo>>

    [[foo,[FOO] ]]foo
    EOS
    result = convert_string_to_embedded input
    assert_includes result, 'see <a href="#foo">[FOO]</a>'
  end

"###
    );

    let doc = Parser::default().parse("see <<foo>>\n\n[[foo,[FOO] ]]foo");
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"see <a href="#foo">[FOO]</a>"##
    );
    assert_eq!(
        doc.catalog().get_ref("foo").unwrap().reftext.as_deref(),
        Some("[FOO]")
    );
}

// Backend-specific test: DocBook output is out of scope for this crate.
non_normative!(
    r###"
  test 'should remove trailing space on reftext of inline anchor shorthand when converting to DocBook' do
    input = <<~'EOS'
    see <<foo>>

    [[foo,[FOO] ]]foo
    EOS
    result = convert_string_to_embedded input, backend: 'docbook'
    assert_includes result, ' xreflabel="[FOO]"'
  end

"###
);

#[test]
fn unescapes_square_bracket_in_reftext_of_anchor_macro() {
    verifies!(
        r###"
  test 'unescapes square bracket in reftext of anchor macro' do
    input = <<~'EOS'
    see <<foo>>

    anchor:foo[b[a\]r]tex
    EOS
    result = convert_string_to_embedded input
    assert_includes result, 'see <a href="#foo">b[a]r</a>'
  end

"###
    );

    let doc = Parser::default().parse("see <<foo>>\n\nanchor:foo[b[a\\]r]tex");
    let paras = rendered_paragraphs(&doc);
    assert_eq!(paras[0], r##"see <a href="#foo">b[a]r</a>"##);
    assert_eq!(
        doc.catalog().get_ref("foo").unwrap().reftext.as_deref(),
        Some("b[a]r")
    );
}

// Backend-specific test: DocBook output is out of scope for this crate.
non_normative!(
    r###"
  test 'unescapes square bracket in reftext of anchor macro in DocBook output' do
    input = 'anchor:foo[b[a\]r]'
    result = convert_inline_string input, backend: :docbook
    assert_equal '<anchor xml:id="foo" xreflabel="b[a]r"/>', result
  end

"###
);

// Ruby-internal API: manually seeds a ref via `doc.register` /
// `Asciidoctor::Inline.new`, which this crate does not expose.
non_normative!(
    r###"
  test 'xref using angled bracket syntax' do
    doc = document_from_string '<<tigers>>'
    doc.register :refs, ['tigers', (Asciidoctor::Inline.new doc, :anchor, '[tigers]', type: :ref, target: 'tigers'), '[tigers]']
    assert_xpath '//a[@href="#tigers"][text() = "[tigers]"]', doc.convert, 1
  end

"###
);

// Ruby-internal API: manually seeds a ref via `doc.register` /
// `Asciidoctor::Inline.new`, which this crate does not expose.
non_normative!(
    r###"
  test 'xref using angled bracket syntax with explicit hash' do
    doc = document_from_string '<<#tigers>>'
    doc.register :refs, ['tigers', (Asciidoctor::Inline.new doc, :anchor, 'Tigers', type: :ref, target: 'tigers'), 'Tigers']
    assert_xpath '//a[@href="#tigers"][text() = "Tigers"]', doc.convert, 1
  end

"###
);

#[test]
fn xref_using_angled_bracket_syntax_with_label() {
    verifies!(
        r###"
  test 'xref using angled bracket syntax with label' do
    input = <<~'EOS'
    <<tigers,About Tigers>>

    [#tigers]
    == Tigers
    EOS
    assert_xpath '//a[@href="#tigers"][text() = "About Tigers"]', convert_string(input), 1
  end

"###
    );

    let doc = Parser::default().parse(
        r##"<<tigers,About Tigers>>

[#tigers]
== Tigers"##,
    );
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="#tigers">About Tigers</a>"##
    );
}

#[test]
fn xref_should_use_title_of_target_as_link_text_when_no_explicit_reftext_is_specified() {
    verifies!(
        r###"
  test 'xref should use title of target as link text when no explicit reftext is specified' do
    input = <<~'EOS'
    <<tigers>>

    [#tigers]
    == Tigers
    EOS
    assert_xpath '//a[@href="#tigers"][text() = "Tigers"]', convert_string(input), 1
  end

"###
    );

    let doc = Parser::default().parse(
        r##"<<tigers>>

[#tigers]
== Tigers"##,
    );
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="#tigers">Tigers</a>"##
    );
}

#[test]
fn xref_should_use_title_of_target_as_link_text_when_explicit_link_text_is_empty() {
    verifies!(
        r###"
  test 'xref should use title of target as link text when explicit link text is empty' do
    input = <<~'EOS'
    <<tigers,>>

    [#tigers]
    == Tigers
    EOS
    assert_xpath '//a[@href="#tigers"][text() = "Tigers"]', convert_string(input), 1
  end

"###
    );

    let doc = Parser::default().parse("<<tigers,>>\n\n[#tigers]\n== Tigers");
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="#tigers">Tigers</a>"##
    );
}

#[test]
fn xref_using_angled_bracket_syntax_with_quoted_label() {
    verifies!(
        r###"
  test 'xref using angled bracket syntax with quoted label' do
    input = <<~'EOS'
    <<tigers,"About Tigers">>

    [#tigers]
    == Tigers
    EOS
    assert_xpath %q(//a[@href="#tigers"][text() = '"About Tigers"']), convert_string(input), 1
  end

"###
    );

    let doc = Parser::default().parse(
        r##"<<tigers,"About Tigers">>

[#tigers]
== Tigers"##,
    );
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="#tigers">"About Tigers"</a>"##
    );
}

// Compat mode is out of scope for this crate.
non_normative!(
    r###"
  test 'should not interpret path sans extension in xref with angled bracket syntax in compat mode' do
    using_memory_logger do |logger|
      doc = document_from_string '<<tigers#>>', standalone: false, attributes: { 'compat-mode' => '' }
      assert_xpath '//a[@href="#tigers#"][text() = "[tigers#]"]', doc.convert, 1
    end
  end

"###
);

#[test]
fn xref_using_angled_bracket_syntax_with_path_sans_extension() {
    verifies!(
        r###"
  test 'xref using angled bracket syntax with path sans extension' do
    doc = document_from_string '<<tigers#>>', standalone: false
    assert_xpath '//a[@href="tigers.html"][text() = "tigers.html"]', doc.convert, 1
  end

"###
    );

    let doc = Parser::default().parse(r##"<<tigers#>>"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="tigers.html">tigers.html</a>"##
    );
}

#[test]
fn inter_document_xref_shorthand_syntax_should_assume_asciidoc_extension_if_asciidoc_extension_not_present()
 {
    verifies!(
        r###"
  test 'inter-document xref shorthand syntax should assume AsciiDoc extension if AsciiDoc extension not present' do
    {
      'using-.net-web-services#' => 'Using .NET web services',
      'asciidoctor.1#' => 'Asciidoctor Manual',
      'path/to/document#' => 'Document Title',
    }.each do |target, text|
      result = convert_string_to_embedded %(<<#{target},#{text}>>)
      assert_xpath %(//a[@href="#{target.chop}.html"][text()="#{text}"]), result, 1
    end
  end

"###
    );

    for (target, text) in [
        ("using-.net-web-services#", "Using .NET web services"),
        ("asciidoctor.1#", "Asciidoctor Manual"),
        ("path/to/document#", "Document Title"),
    ] {
        let doc = Parser::default().parse(&format!("<<{target},{text}>>"));

        assert_eq!(
            rendered_paragraphs(&doc)[0],
            format!(
                r##"<a href="{path}.html">{text}</a>"##,
                path = target.trim_end_matches('#')
            )
        );
    }
}

#[test]
fn xref_macro_with_explicit_inter_document_target_should_assume_implicit_asciidoc_file_extension_if_no_file_extension_is_present()
 {
    verifies!(
        r###"
  test 'xref macro with explicit inter-document target should assume implicit AsciiDoc file extension if no file extension is present' do
    {
      'using-.net-web-services#' => 'Using .NET web services',
      'asciidoctor.1#' => 'Asciidoctor Manual',
    }.each do |target, text|
      result = convert_string_to_embedded %(xref:#{target}[#{text}])
      assert_xpath %(//a[@href="#{target.chop}"][text()="#{text}"]), result, 1
    end
    {
      'document#' => 'Document Title',
      'path/to/document#' => 'Document Title',
      'include.d/document#' => 'Document Title',
    }.each do |target, text|
      result = convert_string_to_embedded %(xref:#{target}[#{text}])
      assert_xpath %(//a[@href="#{target.chop}.html"][text()="#{text}"]), result, 1
    end
  end

"###
    );

    // A target that already carries a file extension keeps it: the extension
    // marks a file that is not AsciiDoc source, so no output suffix is added.
    for (target, text) in [
        ("using-.net-web-services#", "Using .NET web services"),
        ("asciidoctor.1#", "Asciidoctor Manual"),
    ] {
        let doc = Parser::default().parse(&format!("xref:{target}[{text}]"));

        assert_eq!(
            rendered_paragraphs(&doc)[0],
            format!(
                r##"<a href="{path}">{text}</a>"##,
                path = target.trim_end_matches('#')
            )
        );
    }

    // A target with no file extension (a period in an earlier path segment is
    // not one) names an AsciiDoc document, so it takes the output suffix.
    for (target, text) in [
        ("document#", "Document Title"),
        ("path/to/document#", "Document Title"),
        ("include.d/document#", "Document Title"),
    ] {
        let doc = Parser::default().parse(&format!("xref:{target}[{text}]"));

        assert_eq!(
            rendered_paragraphs(&doc)[0],
            format!(
                r##"<a href="{path}.html">{text}</a>"##,
                path = target.trim_end_matches('#')
            )
        );
    }
}

#[test]
fn xref_macro_with_implicit_inter_document_target_should_preserve_path_with_file_extension() {
    verifies!(
        r###"
  test 'xref macro with implicit inter-document target should preserve path with file extension' do
    {
      'refcard.pdf' => 'Refcard',
      'asciidoctor.1' => 'Asciidoctor Manual',
    }.each do |path, text|
      result = convert_string_to_embedded %(xref:#{path}[#{text}])
      assert_xpath %(//a[@href="#{path}"][text()="#{text}"]), result, 1
    end
    {
      'sections.d/first' => 'First Section',
    }.each do |path, text|
      result = convert_string_to_embedded %(xref:#{path}[#{text}])
      assert_xpath %(//a[@href="##{path}"][text()="#{text}"]), result, 1
    end
  end

"###
    );

    for (path, text) in [
        ("refcard.pdf", "Refcard"),
        ("asciidoctor.1", "Asciidoctor Manual"),
    ] {
        let doc = Parser::default().parse(&format!("xref:{path}[{text}]"));

        assert_eq!(
            rendered_paragraphs(&doc)[0],
            format!(r##"<a href="{path}">{text}</a>"##)
        );
    }

    // The period here belongs to a directory name, not a file extension, so
    // the target is read as an ID within this document.
    let doc = Parser::default().parse(r##"xref:sections.d/first[First Section]"##);

    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="#sections.d/first">First Section</a>"##
    );
}

#[test]
fn inter_document_xref_should_only_remove_the_file_extension_part_if_the_path_contains_a_period_elsewhere()
 {
    verifies!(
        r###"
  test 'inter-document xref should only remove the file extension part if the path contains a period elsewhere' do
    result = convert_string_to_embedded '<<using-.net-web-services.adoc#,Using .NET web services>>'
    assert_xpath '//a[@href="using-.net-web-services.html"][text() = "Using .NET web services"]', result, 1
  end

"###
    );

    let doc =
        Parser::default().parse(r##"<<using-.net-web-services.adoc#,Using .NET web services>>"##);

    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="using-.net-web-services.html">Using .NET web services</a>"##
    );
}

#[test]
fn xref_macro_target_containing_dot_should_be_interpreted_as_a_path_unless_prefixed_by_hash() {
    verifies!(
        r###"
  test 'xref macro target containing dot should be interpreted as a path unless prefixed by #' do
    result = convert_string_to_embedded 'xref:using-.net-web-services[Using .NET web services]'
    assert_xpath '//a[@href="using-.net-web-services"][text() = "Using .NET web services"]', result, 1
    result = convert_string_to_embedded 'xref:#using-.net-web-services[Using .NET web services]'
    assert_xpath '//a[@href="#using-.net-web-services"][text() = "Using .NET web services"]', result, 1
  end

"###
    );

    let doc = Parser::default().parse(r##"xref:using-.net-web-services[Using .NET web services]"##);

    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="using-.net-web-services">Using .NET web services</a>"##
    );

    let doc =
        Parser::default().parse(r##"xref:#using-.net-web-services[Using .NET web services]"##);

    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="#using-.net-web-services">Using .NET web services</a>"##
    );
}

#[test]
fn should_not_interpret_double_underscore_in_target_of_xref_macro_if_sequence_is_preceded_by_a_backslash()
 {
    verifies!(
        r###"
  test 'should not interpret double underscore in target of xref macro if sequence is preceded by a backslash' do
    result = convert_string_to_embedded 'xref:doc\__with_double__underscore.adoc[text]'
    assert_xpath '//a[@href="doc__with_double__underscore.html"][text() = "text"]', result, 1
  end

"###
    );

    let doc = Parser::default().parse(r##"xref:doc\__with_double__underscore.adoc[text]"##);

    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="doc__with_double__underscore.html">text</a>"##
    );
}

#[test]
fn should_not_interpret_double_underscore_in_target_of_xref_shorthand_if_sequence_is_preceded_by_a_backslash()
 {
    verifies!(
        r###"
  test 'should not interpret double underscore in target of xref shorthand if sequence is preceded by a backslash' do
    result = convert_string_to_embedded '<<doc\__with_double__underscore.adoc#,text>>'
    assert_xpath '//a[@href="doc__with_double__underscore.html"][text() = "text"]', result, 1
  end

"###
    );

    let doc = Parser::default().parse(r##"<<doc\__with_double__underscore.adoc#,text>>"##);

    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="doc__with_double__underscore.html">text</a>"##
    );
}

// Backend-specific test: DocBook output is out of scope for this crate.
non_normative!(
    r###"
  test 'xref using angled bracket syntax with path sans extension using docbook backend' do
    doc = document_from_string '<<tigers#>>', standalone: false, backend: 'docbook'
    assert_match '<link xl:href="tigers.xml">tigers.xml</link>', doc.convert, 1
  end

"###
);

#[test]
fn xref_using_angled_bracket_syntax_with_ancestor_path_sans_extension() {
    verifies!(
        r###"
  test 'xref using angled bracket syntax with ancestor path sans extension' do
    doc = document_from_string '<<../tigers#,tigers>>', standalone: false
    assert_xpath '//a[@href="../tigers.html"][text() = "tigers"]', doc.convert, 1
  end

"###
    );

    let doc = Parser::default().parse(r##"<<../tigers#,tigers>>"##);

    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="../tigers.html">tigers</a>"##
    );
}

#[test]
fn xref_using_angled_bracket_syntax_with_absolute_path_sans_extension() {
    verifies!(
        r###"
  test 'xref using angled bracket syntax with absolute path sans extension' do
    doc = document_from_string '<</path/to/tigers#,tigers>>', standalone: false
    assert_xpath '//a[@href="/path/to/tigers.html"][text() = "tigers"]', doc.convert, 1
  end

"###
    );

    let doc = Parser::default().parse(r##"<</path/to/tigers#,tigers>>"##);

    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="/path/to/tigers.html">tigers</a>"##
    );
}

#[test]
fn xref_using_angled_bracket_syntax_with_path_and_extension() {
    verifies!(
        r###"
  test 'xref using angled bracket syntax with path and extension' do
    using_memory_logger do |logger|
      doc = document_from_string '<<tigers.adoc>>', standalone: false
      assert_xpath '//a[@href="#tigers.adoc"][text() = "[tigers.adoc]"]', doc.convert, 1
    end
  end

"###
    );

    let doc = Parser::default().parse(r##"<<tigers.adoc>>"##);
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="#tigers.adoc">[tigers.adoc]</a>"##
    );
}

#[test]
fn xref_using_angled_bracket_syntax_with_path_and_extension_with_hash() {
    verifies!(
        r###"
  test 'xref using angled bracket syntax with path and extension with hash' do
    doc = document_from_string '<<tigers.adoc#>>', standalone: false
    assert_xpath '//a[@href="tigers.html"][text() = "tigers.html"]', doc.convert, 1
  end

"###
    );

    let doc = Parser::default().parse(r##"<<tigers.adoc#>>"##);

    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="tigers.html">tigers.html</a>"##
    );
}

#[test]
fn xref_using_angled_bracket_syntax_with_path_and_extension_with_fragment() {
    verifies!(
        r###"
  test 'xref using angled bracket syntax with path and extension with fragment' do
    doc = document_from_string '<<tigers.adoc#id>>', standalone: false
    assert_xpath '//a[@href="tigers.html#id"][text() = "tigers.html"]', doc.convert, 1
  end

"###
    );

    let doc = Parser::default().parse(r##"<<tigers.adoc#id>>"##);

    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="tigers.html#id">tigers.html</a>"##
    );
}

// Compat mode is out of scope for this crate.
non_normative!(
    r###"
  test 'xref using macro syntax with path and extension in compat mode' do
    using_memory_logger do |logger|
      doc = document_from_string 'xref:tigers.adoc[]', standalone: false, attributes: { 'compat-mode' => '' }
      assert_xpath '//a[@href="#tigers.adoc"][text() = "[tigers.adoc]"]', doc.convert, 1
    end
  end

"###
);

#[test]
fn xref_using_macro_syntax_with_path_and_extension() {
    verifies!(
        r###"
  test 'xref using macro syntax with path and extension' do
    doc = document_from_string 'xref:tigers.adoc[]', standalone: false
    assert_xpath '//a[@href="tigers.html"][text() = "tigers.html"]', doc.convert, 1
  end

"###
    );

    let doc = Parser::default().parse(r##"xref:tigers.adoc[]"##);

    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="tigers.html">tigers.html</a>"##
    );
}

#[test]
fn xref_using_angled_bracket_syntax_with_path_and_fragment() {
    verifies!(
        r###"
  test 'xref using angled bracket syntax with path and fragment' do
    doc = document_from_string '<<tigers#about>>', standalone: false
    assert_xpath '//a[@href="tigers.html#about"][text() = "tigers.html"]', doc.convert, 1
  end

"###
    );

    let doc = Parser::default().parse(r##"<<tigers#about>>"##);

    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="tigers.html#about">tigers.html</a>"##
    );
}

#[test]
fn xref_using_angled_bracket_syntax_with_path_fragment_and_text() {
    verifies!(
        r###"
  test 'xref using angled bracket syntax with path, fragment and text' do
    doc = document_from_string '<<tigers#about,About Tigers>>', standalone: false
    assert_xpath '//a[@href="tigers.html#about"][text() = "About Tigers"]', doc.convert, 1
  end

"###
    );

    let doc = Parser::default().parse(r##"<<tigers#about,About Tigers>>"##);

    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="tigers.html#about">About Tigers</a>"##
    );
}

#[test]
fn xref_using_angled_bracket_syntax_with_path_and_custom_relfilesuffix_and_outfilesuffix() {
    verifies!(
        r###"
  test 'xref using angled bracket syntax with path and custom relfilesuffix and outfilesuffix' do
    attributes = { 'relfileprefix' => '../', 'outfilesuffix' => '/' }
    doc = document_from_string '<<tigers#about,About Tigers>>', standalone: false, attributes: attributes
    assert_xpath '//a[@href="../tigers/#about"][text() = "About Tigers"]', doc.convert, 1
  end

"###
    );

    // `relfilesuffix` is unset here, so it tracks `outfilesuffix`.
    let doc = doc_with(
        r##"<<tigers#about,About Tigers>>"##,
        &[("relfileprefix", "../"), ("outfilesuffix", "/")],
    );

    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="../tigers/#about">About Tigers</a>"##
    );
}

#[test]
fn xref_using_angled_bracket_syntax_with_path_and_custom_relfilesuffix() {
    verifies!(
        r###"
  test 'xref using angled bracket syntax with path and custom relfilesuffix' do
    attributes = { 'relfilesuffix' => '/' }
    doc = document_from_string '<<tigers#about,About Tigers>>', standalone: false, attributes: attributes
    assert_xpath '//a[@href="tigers/#about"][text() = "About Tigers"]', doc.convert, 1
  end

"###
    );

    let doc = doc_with(
        r##"<<tigers#about,About Tigers>>"##,
        &[("relfilesuffix", "/")],
    );

    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="tigers/#about">About Tigers</a>"##
    );
}

// Include processing / `catalog[:includes]` is out of scope for this crate.
non_normative!(
    r###"
  test 'xref using angled bracket syntax with path which has been included in this document' do
    using_memory_logger do |logger|
      in_verbose_mode do
        doc = document_from_string '<<tigers#about,About Tigers>>', standalone: false
        doc.catalog[:includes]['tigers'] = true
        output = doc.convert
        assert_xpath '//a[@href="#about"][text() = "About Tigers"]', output, 1
        assert_message logger, :INFO, 'possible invalid reference: about'
      end
    end
  end

"###
);

// Include processing / `catalog[:includes]` is out of scope for this crate.
non_normative!(
    r###"
  test 'xref using angled bracket syntax with nested path which has been included in this document' do
    using_memory_logger do |logger|
      in_verbose_mode do
        doc = document_from_string '<<part1/tigers#about,About Tigers>>', standalone: false
        doc.catalog[:includes]['part1/tigers'] = true
        output = doc.convert
        assert_xpath '//a[@href="#about"][text() = "About Tigers"]', output, 1
        assert_message logger, :INFO, 'possible invalid reference: about'
      end
    end
  end

"###
);

#[test]
fn xref_using_angled_bracket_syntax_inline_with_text() {
    verifies!(
        r###"
  test 'xref using angled bracket syntax inline with text' do
    input = <<~'EOS'
    Want to learn <<tigers,about tigers>>?

    [#tigers]
    == Tigers
    EOS
    assert_xpath '//a[@href="#tigers"][text() = "about tigers"]', convert_string(input), 1
  end

"###
    );

    let doc = Parser::default().parse(
        r##"Want to learn <<tigers,about tigers>>?

[#tigers]
== Tigers"##,
    );
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"Want to learn <a href="#tigers">about tigers</a>?"##
    );
}

#[test]
fn xref_using_angled_bracket_syntax_with_multi_line_label_inline_with_text() {
    verifies!(
        r###"
  test 'xref using angled bracket syntax with multi-line label inline with text' do
    input = <<~'EOS'
    Want to learn <<tigers,about
    tigers>>?

    [#tigers]
    == Tigers
    EOS
    assert_xpath %{//a[@href="#tigers"][normalize-space(text()) = "about tigers"]}, convert_string(input), 1
  end

"###
    );

    let doc =
        Parser::default().parse("Want to learn <<tigers,about\ntigers>>?\n\n[#tigers]\n== Tigers");
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        "Want to learn <a href=\"#tigers\">about\ntigers</a>?"
    );
}

#[test]
fn xref_with_escaped_text() {
    verifies!(
        r###"
  test 'xref with escaped text' do
    # when \x0 was used as boundary character for passthrough, it was getting stripped
    # now using unicode marks as boundary characters, which resolves issue
    input = <<~'EOS'
    See the <<tigers, `+[tigers]+`>> section for details about tigers.

    [#tigers]
    == Tigers
    EOS
    output = convert_string_to_embedded input
    assert_xpath %(//a[@href="#tigers"]/code[text()="[tigers]"]), output, 1
  end

"###
    );

    let doc = Parser::default().parse(
        "See the <<tigers, `+[tigers]+`>> section for details about tigers.\n\n[#tigers]\n== Tigers",
    );
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"See the <a href="#tigers"><code>[tigers]</code></a> section for details about tigers."##
    );
}

#[test]
fn xref_with_target_that_begins_with_attribute_reference_in_title() {
    verifies!(
        r###"
  test 'xref with target that begins with attribute reference in title' do
    ['<<{lessonsdir}/lesson-1#,Lesson 1>>', 'xref:{lessonsdir}/lesson-1.adoc[Lesson 1]'].each do |xref|
      input = <<~EOS
      :lessonsdir: lessons

      [#lesson-1-listing]
      == #{xref}

      A summary of the first lesson.
      EOS

      output = convert_string_to_embedded input
      assert_xpath '//h2/a[@href="lessons/lesson-1.html"]', output, 1
    end
  end

"###
    );

    for xref in [
        "<<{lessonsdir}/lesson-1#,Lesson 1>>",
        "xref:{lessonsdir}/lesson-1.adoc[Lesson 1]",
    ] {
        let doc = Parser::default().parse(&format!(
            ":lessonsdir: lessons\n\n[#lesson-1-listing]\n== {xref}\n\nA summary of the first lesson.\n"
        ));

        assert_eq!(
            first_section(&doc).section_title(),
            r##"<a href="lessons/lesson-1.html">Lesson 1</a>"##
        );
    }
}

// Ruby-internal API: manually seeds a ref via `doc.register` /
// `Asciidoctor::Inline.new`, which this crate does not expose.
non_normative!(
    r###"
  test 'xref using macro syntax' do
    doc = document_from_string 'xref:tigers[]'
    doc.register :refs, ['tigers', (Asciidoctor::Inline.new doc, :anchor, '[tigers]', type: :ref, target: 'tigers'), '[tigers]']
    assert_xpath '//a[@href="#tigers"][text() = "[tigers]"]', doc.convert, 1
  end

"###
);

#[test]
fn multiple_xref_macros_with_implicit_text_in_single_line() {
    verifies!(
        r###"
  test 'multiple xref macros with implicit text in single line' do
    input = <<~'EOS'
    This document has two sections, xref:sect-a[] and xref:sect-b[].

    [#sect-a]
    == Section A

    [#sect-b]
    == Section B
    EOS
    result = convert_string_to_embedded input
    assert_xpath '//a[@href="#sect-a"][text() = "Section A"]', result, 1
    assert_xpath '//a[@href="#sect-b"][text() = "Section B"]', result, 1
  end

"###
    );

    let doc = Parser::default().parse(
        r##"This document has two sections, xref:sect-a[] and xref:sect-b[].

[#sect-a]
== Section A

[#sect-b]
== Section B"##,
    );
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"This document has two sections, <a href="#sect-a">Section A</a> and <a href="#sect-b">Section B</a>."##
    );
}

// Ruby-internal API: manually seeds a ref via `doc.register` /
// `Asciidoctor::Inline.new`, which this crate does not expose.
non_normative!(
    r###"
  test 'xref using macro syntax with explicit hash' do
    doc = document_from_string 'xref:#tigers[]'
    doc.register :refs, ['tigers', (Asciidoctor::Inline.new doc, :anchor, 'Tigers', type: :ref, target: 'tigers'), 'Tigers']
    assert_xpath '//a[@href="#tigers"][text() = "Tigers"]', doc.convert, 1
  end

"###
);

#[test]
fn xref_using_macro_syntax_with_label() {
    verifies!(
        r###"
  test 'xref using macro syntax with label' do
    input = <<~'EOS'
    xref:tigers[About Tigers]

    [#tigers]
    == Tigers
    EOS
    assert_xpath '//a[@href="#tigers"][text() = "About Tigers"]', convert_string(input), 1
  end

"###
    );

    let doc = Parser::default().parse(
        r##"xref:tigers[About Tigers]

[#tigers]
== Tigers"##,
    );
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="#tigers">About Tigers</a>"##
    );
}

#[test]
fn xref_using_macro_syntax_inline_with_text() {
    verifies!(
        r###"
  test 'xref using macro syntax inline with text' do
    input = <<~'EOS'
    Want to learn xref:tigers[about tigers]?

    [#tigers]
    == Tigers
    EOS

    assert_xpath '//a[@href="#tigers"][text() = "about tigers"]', convert_string(input), 1
  end

"###
    );

    let doc = Parser::default().parse(
        r##"Want to learn xref:tigers[about tigers]?

[#tigers]
== Tigers"##,
    );
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"Want to learn <a href="#tigers">about tigers</a>?"##
    );
}

#[test]
fn xref_using_macro_syntax_with_multi_line_label_inline_with_text() {
    verifies!(
        r###"
  test 'xref using macro syntax with multi-line label inline with text' do
    input = <<~'EOS'
    Want to learn xref:tigers[about
    tigers]?

    [#tigers]
    == Tigers
    EOS
    assert_xpath %{//a[@href="#tigers"][normalize-space(text()) = "about tigers"]}, convert_string(input), 1
  end

"###
    );

    let doc = Parser::default()
        .parse("Want to learn xref:tigers[about\ntigers]?\n\n[#tigers]\n== Tigers");
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        "Want to learn <a href=\"#tigers\">about\ntigers</a>?"
    );
}

#[test]
fn xref_using_macro_syntax_with_text_that_ends_with_an_escaped_closing_bracket() {
    verifies!(
        r###"
  test 'xref using macro syntax with text that ends with an escaped closing bracket' do
    input = <<~'EOS'
    xref:tigers[[tigers\]]

    [#tigers]
    == Tigers
    EOS
    assert_xpath '//a[@href="#tigers"][text() = "[tigers]"]', convert_string_to_embedded(input), 1
  end

"###
    );

    let doc = Parser::default().parse(
        r##"xref:tigers[[tigers\]]

[#tigers]
== Tigers"##,
    );
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="#tigers">[tigers]</a>"##
    );
}

#[test]
fn xref_using_macro_syntax_with_text_that_contains_an_escaped_closing_bracket() {
    verifies!(
        r###"
  test 'xref using macro syntax with text that contains an escaped closing bracket' do
    input = <<~'EOS'
    xref:tigers[[tigers\] are cats]

    [#tigers]
    == Tigers
    EOS
    assert_xpath '//a[@href="#tigers"][text() = "[tigers] are cats"]', convert_string_to_embedded(input), 1
  end

"###
    );

    let doc = Parser::default().parse(
        r##"xref:tigers[[tigers\] are cats]

[#tigers]
== Tigers"##,
    );
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="#tigers">[tigers] are cats</a>"##
    );
}

#[test]
fn unescapes_square_bracket_in_reftext_used_by_xref() {
    verifies!(
        r###"
  test 'unescapes square bracket in reftext used by xref' do
    input = <<~'EOS'
    anchor:foo[b[a\]r]about

    see <<foo>>
    EOS
    result = convert_string_to_embedded input
    assert_xpath '//a[@href="#foo"]', result, 1
    assert_xpath '//a[@href="#foo"][text()="b[a]r"]', result, 1
  end

"###
    );

    let doc = Parser::default().parse("anchor:foo[b[a\\]r]about\n\nsee <<foo>>");
    let paras = rendered_paragraphs(&doc);
    assert_eq!(paras[1], r##"see <a href="#foo">b[a]r</a>"##);
    assert_eq!(
        doc.catalog().get_ref("foo").unwrap().reftext.as_deref(),
        Some("b[a]r")
    );
}

// Ruby-internal API: manually seeds a ref via `doc.register` /
// `Asciidoctor::Inline.new`, which this crate does not expose.
non_normative!(
    r###"
  test 'xref using invalid macro syntax does not create link' do
    doc = document_from_string 'xref:tigers'
    doc.register :refs, ['tigers', (Asciidoctor::Inline.new doc, :anchor, 'Tigers', type: :ref, target: 'tigers'), 'Tigers']
    assert_xpath '//a', doc.convert, 0
  end

"###
);

// Surfaced incompatibility: this crate creates the fallback link but does not
// emit the `possible invalid reference` log message that this test also
// asserts. Tracked in #772.
non_normative!(
    r###"
  test 'should warn and create link if verbose flag is set and reference is not found' do
    input = <<~'EOS'
    [#foobar]
    == Foobar

    == Section B

    See <<foobaz>>.
    EOS
    using_memory_logger do |logger|
      in_verbose_mode do
        output = convert_string_to_embedded input
        assert_xpath '//a[@href="#foobaz"][text() = "[foobaz]"]', output, 1
        assert_message logger, :INFO, 'possible invalid reference: foobaz'
      end
    end
  end

"###
);

// Compat mode is out of scope for this crate.
non_normative!(
    r###"
  test 'should not warn if verbose flag is set and reference is found in compat mode' do
    input = <<~'EOS'
    [[foobar]]
    == Foobar

    == Section B

    See <<foobar>>.
    EOS
    using_memory_logger do |logger|
      in_verbose_mode do
        output = convert_string_to_embedded input, attributes: { 'compat-mode' => '' }
        assert_xpath '//a[@href="#foobar"][text() = "Foobar"]', output, 1
        assert_empty logger
      end
    end
  end

"###
);

// Surfaced incompatibility: this crate creates the fallback link `<a
// href="#foobaz">[foobaz]</a>` but does not emit the `possible invalid
// reference` log message that this test also asserts (verbose logging). Tracked
// in #772.
non_normative!(
    r###"
  test 'should warn and create link if verbose flag is set and reference using # notation is not found' do
    input = <<~'EOS'
    [#foobar]
    == Foobar

    == Section B

    See <<#foobaz>>.
    EOS
    using_memory_logger do |logger|
      in_verbose_mode do
        output = convert_string_to_embedded input
        assert_xpath '//a[@href="#foobaz"][text() = "[foobaz]"]', output, 1
        assert_message logger, :INFO, 'possible invalid reference: foobaz'
      end
    end
  end

"###
);

// Include processing / `catalog[:includes]` is out of scope for this crate.
non_normative!(
    r###"
  test 'should produce an internal anchor from an inter-document xref to file included into current file' do
    input = <<~'EOS'
    = Book Title
    :doctype: book

    [#ch1]
    == Chapter 1

    So it begins.

    Read <<other-chapters.adoc#ch2>> to find out what happens next!

    include::other-chapters.adoc[]
    EOS

    doc = document_from_string input, safe: :safe, base_dir: fixturedir
    assert doc.catalog[:includes].key?('other-chapters')
    assert doc.catalog[:includes]['other-chapters']
    output = doc.convert
    assert_xpath '//a[@href="#ch2"][text()="Chapter 2"]', output, 1
  end

"###
);

// Include processing / `catalog[:includes]` is out of scope for this crate.
non_normative!(
    r###"
  test 'should produce an internal anchor from an inter-document xref to file included entirely into current file using tags' do
    input = <<~'EOS'
    = Book Title
    :doctype: book

    [#ch1]
    == Chapter 1

    So it begins.

    Read <<other-chapters.adoc#ch2>> to find out what happens next!

    include::other-chapters.adoc[tags=**]
    EOS

    output = convert_string_to_embedded input, safe: :safe, base_dir: fixturedir
    assert_xpath '//a[@href="#ch2"][text()="Chapter 2"]', output, 1
  end

"###
);

// Include processing / `catalog[:includes]` is out of scope for this crate.
non_normative!(
    r###"
  test 'should not produce an internal anchor for inter-document xref to file partially included into current file' do
    input = <<~'EOS'
    = Book Title
    :doctype: book

    [#ch1]
    == Chapter 1

    So it begins.

    Read <<other-chapters.adoc#ch2,the next chapter>> to find out what happens next!

    include::other-chapters.adoc[tags=ch2]
    EOS

    doc = document_from_string input, safe: :safe, base_dir: fixturedir
    assert doc.catalog[:includes].key?('other-chapters')
    refute doc.catalog[:includes]['other-chapters']
    output = doc.convert
    assert_xpath '//a[@href="other-chapters.html#ch2"][text()="the next chapter"]', output, 1
  end

"###
);

// Include processing / `catalog[:includes]` is out of scope for this crate.
non_normative!(
    r###"
  test 'should produce an internal anchor for inter-document xref to file included fully and partially' do
    input = <<~'EOS'
    = Book Title
    :doctype: book

    [#ch1]
    == Chapter 1

    So it begins.

    Read <<other-chapters.adoc#ch2,the next chapter>> to find out what happens next!

    include::other-chapters.adoc[]

    include::other-chapters.adoc[tag=ch2-noid]
    EOS

    doc = document_from_string input, safe: :safe, base_dir: fixturedir
    assert doc.catalog[:includes].key?('other-chapters')
    assert doc.catalog[:includes]['other-chapters']
    output = doc.convert
    assert_xpath '//a[@href="#ch2"][text()="the next chapter"]', output, 1
  end

"###
);

// Include processing / `catalog[:includes]` is out of scope for this crate.
non_normative!(
    r###"
  test 'should warn and create link if debug mode is enabled, inter-document xref points to current doc, and reference not found' do
    input = <<~'EOS'
    [#foobar]
    == Foobar

    == Section B

    See <<test.adoc#foobaz>>.
    EOS
    using_memory_logger do |logger|
      in_verbose_mode do
        output = convert_string_to_embedded input, attributes: { 'docname' => 'test' }
        assert_xpath '//a[@href="#foobaz"][text() = "[foobaz]"]', output, 1
        assert_message logger, :INFO, 'possible invalid reference: foobaz'
      end
    end
  end

"###
);

// Inter-document self-xref fallback link text is not implemented here. Tracked
// in #773.
non_normative!(
    r###"
  test 'should use doctitle as fallback link text if inter-document xref points to current doc and no link text is provided' do
    input = <<~'EOS'
    = Links & Stuff at https://example.org

    See xref:test.adoc[]
    EOS
    output = convert_string_to_embedded input, attributes: { 'docname' => 'test' }
    assert_include '<a href="#">Links &amp; Stuff at https://example.org</a>', output
  end

"###
);

// Inter-document xref fallback in an AsciiDoc table cell is out of scope here.
// Tracked in #773.
non_normative!(
    r###"
  test 'should use doctitle of root document as fallback link text for inter-document xref in AsciiDoc table cell that resolves to current doc' do
    input = <<~'EOS'
    = Document Title

    |===
    a|See xref:test.adoc[]
    |===
    EOS
    output = convert_string_to_embedded input, attributes: { 'docname' => 'test' }
    assert_include '<a href="#">Document Title</a>', output
  end

"###
);

// Inter-document self-xref fallback link text is not implemented here. Tracked
// in #773.
non_normative!(
    r###"
  test 'should use reftext on document as fallback link text if inter-document xref points to current doc and no link text is provided' do
    input = <<~'EOS'
    [reftext="Links and Stuff"]
    = Links & Stuff

    See xref:test.adoc[]
    EOS
    output = convert_string_to_embedded input, attributes: { 'docname' => 'test' }
    assert_include '<a href="#">Links and Stuff</a>', output
  end

"###
);

// Inter-document self-xref fallback link text is not implemented here. Tracked
// in #773.
non_normative!(
    r###"
  test 'should use reftext on document as fallback link text if xref points to empty fragment and no link text is provided' do
    input = <<~'EOS'
    [reftext="Links and Stuff"]
    = Links & Stuff

    See xref:#[]
    EOS
    output = convert_string_to_embedded input, attributes: { 'docname' => 'test' }
    assert_include '<a href="#">Links and Stuff</a>', output
  end

"###
);

// Inter-document self-xref fallback link text is not implemented here. Tracked
// in #773.
non_normative!(
    r###"
  test 'should use fallback link text if inter-document xref points to current doc without header and no link text is provided' do
    input = <<~'EOS'
    See xref:test.adoc[]
    EOS
    output = convert_string_to_embedded input, attributes: { 'docname' => 'test' }
    assert_include '<a href="#">[^top]</a>', output
  end

"###
);

// Inter-document self-xref fallback link text is not implemented here. Tracked
// in #773.
non_normative!(
    r###"
  test 'should use fallback link text if fragment of internal xref is empty and no link text is provided' do
    input = <<~'EOS'
    See xref:#[]
    EOS
    output = convert_string_to_embedded input, attributes: { 'docname' => 'test' }
    assert_include '<a href="#">[^top]</a>', output
  end

"###
);

// Backend-specific test: DocBook output is out of scope for this crate.
non_normative!(
    r###"
  test 'should use document id as linkend for self xref in DocBook backend' do
    input = <<~'EOS'
    [#docid]
    = Document Title

    See xref:test.adoc[]
    EOS
    output = convert_string_to_embedded input, backend: :docbook, attributes: { 'docname' => 'test' }
    assert_include '<xref linkend="docid"/>', output
  end

"###
);

// Backend-specific test: DocBook output is out of scope for this crate.
non_normative!(
    r###"
  test 'should auto-generate document id to use as linkend for self xref in DocBook backend' do
    input = <<~'EOS'
    = Document Title

    See xref:test.adoc[]
    EOS
    doc = document_from_string input, backend: :docbook, attributes: { 'docname' => 'test' }
    assert_nil doc.id
    output = doc.convert
    assert_nil doc.id
    assert_include ' xml:id="__article-root__"', output
    assert_include '<xref linkend="__article-root__"/>', output
  end

"###
);

// Include processing / `catalog[:includes]` is out of scope for this crate.
non_normative!(
    r###"
  test 'should produce an internal anchor for inter-document xref to file outside of base directory' do
    input = <<~'EOS'
    = Document Title

    See <<../section-a.adoc#section-a>>.

    include::../section-a.adoc[]
    EOS

    doc = document_from_string input, safe: :unsafe, base_dir: (File.join fixturedir, 'subdir')
    assert_includes doc.catalog[:includes], '../section-a'
    output = doc.convert standalone: false
    assert_xpath '//a[@href="#section-a"][text()="Section A"]', output, 1
  end

"###
);

#[test]
fn xref_uses_title_of_target_as_label_for_forward_and_backward_references_in_html_output() {
    verifies!(
        r###"
  test 'xref uses title of target as label for forward and backward references in html output' do
    input = <<~'EOS'
    == Section A

    <<_section_b>>

    == Section B

    <<_section_a>>
    EOS

    output = convert_string_to_embedded input
    assert_xpath '//h2[@id="_section_a"][text()="Section A"]', output, 1
    assert_xpath '//a[@href="#_section_a"][text()="Section A"]', output, 1
    assert_xpath '//h2[@id="_section_b"][text()="Section B"]', output, 1
    assert_xpath '//a[@href="#_section_b"][text()="Section B"]', output, 1
  end

"###
    );

    let doc =
        Parser::default().parse("== Section A\n\n<<_section_b>>\n\n== Section B\n\n<<_section_a>>");
    let paras = rendered_paragraphs(&doc);
    assert_eq!(paras[0], r##"<a href="#_section_b">Section B</a>"##);
    assert_eq!(paras[1], r##"<a href="#_section_a">Section A</a>"##);
    assert_eq!(
        doc.catalog()
            .get_ref("_section_a")
            .unwrap()
            .reftext
            .as_deref(),
        Some("Section A")
    );
    assert_eq!(
        doc.catalog()
            .get_ref("_section_b")
            .unwrap()
            .reftext
            .as_deref(),
        Some("Section B")
    );
}

#[test]
fn should_not_fail_to_resolve_broken_xref_in_title_of_block_with_id() {
    verifies!(
        r###"
  test 'should not fail to resolve broken xref in title of block with ID' do
    input = <<~'EOS'
    [#p1]
    .<<DNE>>
    paragraph text
    EOS

    output = convert_string_to_embedded input
    assert_xpath '//*[@class="title"]/a[@href="#DNE"][text()="[DNE]"]', output, 1
  end

"###
    );

    // The broken xref lives in the block title, whose rendered inline is
    // captured in the catalog reftext for the block ID.
    let doc = Parser::default().parse("[#p1]\n.<<DNE>>\nparagraph text");
    assert_eq!(
        doc.catalog().get_ref("p1").unwrap().reftext.as_deref(),
        Some("<a href=\"#DNE\">[DNE]</a>")
    );
}

// Surfaced incompatibility: Asciidoctor resolves the forward xref in a block
// title to the target title (`Conclusion`); this crate leaves it unresolved
// (`[conclusion]`). Tracked in #770.
non_normative!(
    r###"
  test 'should resolve forward xref in title of block with ID' do
    input = <<~'EOS'
    [#p1]
    .<<conclusion>>
    paragraph text

    [#conclusion]
    == Conclusion
    EOS

    output = convert_string_to_embedded input
    assert_xpath '//*[@class="title"]/a[@href="#conclusion"][text()="Conclusion"]', output, 1
  end

"###
);

// Surfaced incompatibility: Asciidoctor resolves `<<s1>>` to section s1's own
// xreflabel; this crate leaves the sibling xref unresolved (`[s1]`). Tracked in
// #770.
non_normative!(
    r###"
  test 'should not fail to resolve broken xref in section title' do
    input = <<~'EOS'
    [#s1]
    == <<DNE>>

    == <<s1>>
    EOS

    output = convert_string_to_embedded input
    assert_xpath '//h2[@id="s1"]/a[@href="#DNE"][text()="[DNE]"]', output, 1
    assert_xpath '//h2/a[@href="#s1"][text()="[DNE]"]', output, 1
  end

"###
);

// Surfaced incompatibility: Asciidoctor resolves an xref in a section title to
// the target section's title; this crate leaves it unresolved (`[b]`/`[a]`).
// Tracked in #770.
non_normative!(
    r###"
  test 'should break circular xref reference in section title' do
    input = <<~'EOS'
    [#a]
    == A <<b>>

    [#b]
    == B <<a>>
    EOS

    output = convert_string_to_embedded input
    assert_includes output, '<h2 id="a">A <a href="#b">B [a]</a></h2>'
    assert_includes output, '<h2 id="b">B <a href="#a">[a]</a></h2>'
  end

"###
);

// Surfaced incompatibility: same section-title xref resolution gap —
// Asciidoctor renders the target section's title (with the nested anchor
// dropped); this crate leaves `[b]` unresolved. Tracked in #770.
non_normative!(
    r###"
  test 'should drop nested anchor in xreftext' do
    input = <<~'EOS'
    [#a]
    == See <<b>>

    [#b]
    == Consult https://google.com[Google]
    EOS

    output = convert_string_to_embedded input
    assert_includes output, '<h2 id="a">See <a href="#b">Consult Google</a></h2>'
  end

"###
);

#[test]
fn should_not_resolve_forward_xref_evaluated_during_parsing() {
    verifies!(
        r###"
  test 'should not resolve forward xref evaluated during parsing' do
    input = <<~'EOS'
    [#s1]
    == <<forward>>

    == <<s1>>

    [#forward]
    == Forward
    EOS

    output = convert_string_to_embedded input
    assert_xpath '//a[@href="#forward"][text()="Forward"]', output, 0
  end

"###
    );

    // The forward xref sits in a section title; the crate does not resolve
    // it to `Forward` during parsing, so no resolved link is produced.
    let doc =
        Parser::default().parse("[#s1]\n== <<forward>>\n\n== <<s1>>\n\n[#forward]\n== Forward");
    assert_eq!(
        doc.catalog().get_ref("s1").unwrap().reftext.as_deref(),
        Some("<a href=\"#forward\">[forward]</a>")
    );
}

#[test]
fn should_not_resolve_forward_natural_xref_evaluated_during_parsing() {
    verifies!(
        r###"
  test 'should not resolve forward natural xref evaluated during parsing' do
    input = <<~'EOS'
    :idprefix:

    [#s1]
    == <<Forward>>

    == <<s1>>

    == Forward
    EOS

    output = convert_string_to_embedded input
    assert_xpath '//a[@href="#forward"][text()="Forward"]', output, 0
  end

"###
    );

    let doc =
        Parser::default().parse(":idprefix:\n\n[#s1]\n== <<Forward>>\n\n== <<s1>>\n\n== Forward");
    assert_eq!(
        doc.catalog().get_ref("s1").unwrap().reftext.as_deref(),
        Some("<a href=\"#Forward\">[Forward]</a>")
    );
}

#[test]
fn should_resolve_first_matching_natural_xref() {
    verifies!(
        r###"
  test 'should resolve first matching natural xref' do
    input = <<~'EOS'
    see <<Section Title>>

    [#s1]
    == Section Title

    [#s2]
    == Section Title
    EOS

    output = convert_string_to_embedded input
    assert_xpath '//a[@href="#s1"]', output, 1
    assert_xpath '//a[@href="#s1"][text()="Section Title"]', output, 1
  end

"###
    );

    let doc = Parser::default()
        .parse("see <<Section Title>>\n\n[#s1]\n== Section Title\n\n[#s2]\n== Section Title");
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"see <a href="#s1">Section Title</a>"##
    );
}

// Surfaced incompatibility: Asciidoctor resolves `<<Cub => Tiger>>` to the
// section id `_cub_tiger`; this crate applies replacement subs to the target
// and fails to match. Tracked in #771.
non_normative!(
    r###"
  test 'should not match numeric character references while searching for fragment in xref target' do
    input = <<~'EOS'
    see <<Cub => Tiger>>

    == Cub => Tiger
    EOS
    output = convert_string_to_embedded input
    assert_xpath '//a[@href="#_cub_tiger"]', output, 1
    assert_xpath %(//a[@href="#_cub_tiger"][text()="Cub #{decode_char 8658} Tiger"]), output, 1
  end

"###
);

#[test]
fn should_not_match_numeric_character_references_in_path_of_interdocument_xref() {
    verifies!(
        r###"
  test 'should not match numeric character references in path of interdocument xref' do
    input = <<~'EOS'
    see xref:{cpp}[{cpp}].
    EOS
    output = convert_string_to_embedded input
    assert_includes output, '<a href="#C&#43;&#43;">C&#43;&#43;</a>'
  end

"###
    );

    let doc = Parser::default().parse("see xref:{cpp}[{cpp}].");
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"see <a href="#C&#43;&#43;">C&#43;&#43;</a>."##
    );
}

#[test]
fn anchor_creates_reference() {
    verifies!(
        r###"
  test 'anchor creates reference' do
    doc = document_from_string '[[tigers]]Tigers roam here.'
    ref = doc.catalog[:refs]['tigers']
    refute_nil ref
    assert_nil ref.reftext
  end

"###
    );

    let doc = Parser::default().parse("[[tigers]]Tigers roam here.");
    let entry = doc.catalog().get_ref("tigers").unwrap();
    assert_eq!(entry.reftext, None);
}

#[test]
fn anchor_with_label_creates_reference() {
    verifies!(
        r###"
  test 'anchor with label creates reference' do
    doc = document_from_string '[[tigers,Tigers]]Tigers roam here.'
    ref = doc.catalog[:refs]['tigers']
    refute_nil ref
    assert_equal 'Tigers', ref.reftext
  end

"###
    );

    let doc = Parser::default().parse("[[tigers,Tigers]]Tigers roam here.");
    let entry = doc.catalog().get_ref("tigers").unwrap();
    assert_eq!(entry.reftext.as_deref(), Some("Tigers"));
}

#[test]
fn anchor_with_quoted_label_creates_reference_with_quoted_label_text() {
    verifies!(
        r###"
  test 'anchor with quoted label creates reference with quoted label text' do
    doc = document_from_string %([[tigers,"Tigers roam here"]]Tigers roam here.)
    ref = doc.catalog[:refs]['tigers']
    refute_nil ref
    assert_equal '"Tigers roam here"', ref.reftext
  end

"###
    );

    let doc = Parser::default().parse(r#"[[tigers,"Tigers roam here"]]Tigers roam here."#);
    let entry = doc.catalog().get_ref("tigers").unwrap();
    assert_eq!(entry.reftext.as_deref(), Some(r#""Tigers roam here""#));
}

#[test]
fn anchor_with_label_containing_a_comma_creates_reference() {
    verifies!(
        r###"
  test 'anchor with label containing a comma creates reference' do
    doc = document_from_string %([[tigers,Tigers, scary tigers, roam here]]Tigers roam here.)
    ref = doc.catalog[:refs]['tigers']
    refute_nil ref
    assert_equal 'Tigers, scary tigers, roam here', ref.reftext
  end
"###
    );

    let doc =
        Parser::default().parse("[[tigers,Tigers, scary tigers, roam here]]Tigers roam here.");
    let entry = doc.catalog().get_ref("tigers").unwrap();
    assert_eq!(
        entry.reftext.as_deref(),
        Some("Tigers, scary tigers, roam here")
    );
}

non_normative!(
    r###"
end
"###
);
