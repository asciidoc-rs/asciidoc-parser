// Adapted from Asciidoctor's attribute-list test suite, found in
// https://github.com/asciidoctor/asciidoctor/blob/main/test/attribute_list_test.rb.
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
//! Port of Asciidoctor's `attribute_list_test.rb`.
//!
//! These are unit tests of Asciidoctor's parser-internal `AttributeList` class:
//! each builds an `AttributeList` from a raw attrlist string, calls
//! `parse_into`, and asserts on the resulting positional/named attribute map.
//! `asciidoc-parser`'s [`Attrlist`](crate::attributes::Attrlist) is the direct
//! analog, so each test drives [`Attrlist::parse`] on the raw string (in a
//! block context, matching how a block attribute line is parsed) and asserts on
//! the parsed attributes via [`nth_attribute`](crate::attributes::Attrlist)
//! and [`named_attribute`](crate::attributes::Attrlist).
//!
//! **Positional numbering.** Like Asciidoctor, positions count every
//! comma-delimited entry — named entries and blank (`nil`) slots included — so
//! `nth(n)` here is Asciidoctor's `n`-keyed positional, and a blank slot yields
//! `None` (Asciidoctor's `nil`).
//!
//! A handful of tests stay `non_normative!` because they exercise behavior this
//! crate models differently, each annotated inline:
//!
//! * The `apply_subs`-is-not-called guarantee combined with a *defined*
//!   attribute reference (`name='{val}`): this crate resolves attribute
//!   references at the whole-attrlist level before parsing, so a defined
//!   `{val}` resolves where the no-document Ruby path keeps it literal.
//! * Leading, non-semantic whitespace before the first attribute name.
//! * `parse_into`'s positional *rekeying* and the static `AttributeList.rekey`
//!   helper, neither of which this crate exposes.
//!
//! The Ruby unit tests pass no document (or stub `apply_subs` to raise), so
//! they never apply substitutions. Driving [`Attrlist::parse`] in a block
//! context *does* apply the normal substitution group to single-quoted values —
//! which is exactly what Asciidoctor does when a real block is present (see
//! `attribute_list.rb`'s `single_quoted && @block` branch). So a single-quoted
//! value like `'ba\'zaar'` verifies against the substituted output
//! (`ba&#8217;zaar`), matching real Asciidoctor rather than the stubbed
//! literal.

use crate::{
    Parser, Span,
    attributes::{Attrlist, AttrlistContext},
    tests::sdd::*,
};

track_file!("ref/asciidoctor/test/attribute_list_test.rb");

/// The positional and named attributes parsed from a raw attrlist string, plus
/// its option list — collected into owned data so the borrowed `Attrlist` can
/// be dropped before the caller asserts.
struct ParsedAttrlist {
    positional: Vec<(usize, String)>,
    named: Vec<(String, String)>,
    option_list: Vec<String>,
}

impl ParsedAttrlist {
    /// Value of the positional attribute at Asciidoctor position `n`, if that
    /// slot holds one (a blank/`nil` slot or a named slot yields `None`).
    fn nth(&self, n: usize) -> Option<&str> {
        self.positional
            .iter()
            .find(|(p, _)| *p == n)
            .map(|(_, v)| v.as_str())
    }

    /// Value of the named attribute `name`, if present.
    fn named(&self, name: &str) -> Option<&str> {
        self.named
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// The parsed option list (`opts=` / `options=`).
    fn options(&self) -> Vec<&str> {
        self.option_list.iter().map(String::as_str).collect()
    }
}

/// Parse a raw attrlist string the way Asciidoctor's `AttributeList.new(line)`
/// does: through [`Attrlist::parse`] in a block context. Warnings are
/// disregarded — these tests assert on the parsed attributes, not diagnostics.
fn parse_attrlist(line: &str) -> ParsedAttrlist {
    let parser = Parser::default();
    let maw = Attrlist::parse(Span::new(line), &parser, AttrlistContext::Block);
    let attrlist = &maw.item.item;

    let mut positional = vec![];
    let mut named = vec![];

    for attr in attrlist.attributes() {
        match attr.name() {
            Some(name) => named.push((name.to_string(), attr.value().to_string())),
            None => positional.push((
                attr.positional_index().unwrap_or(0),
                attr.value().to_string(),
            )),
        }
    }

    let option_list = attrlist.options().iter().map(|o| o.to_string()).collect();

    ParsedAttrlist {
        positional,
        named,
        option_list,
    }
}

non_normative!(
    r#"
# frozen_string_literal: true
require_relative 'test_helper'

context 'AttributeList' do
"#
);

#[test]
fn collect_unnamed_attribute() {
    verifies!(
        r#"
  test 'collect unnamed attribute' do
    attributes = {}
    line = 'quote'
    expected = { 1 => 'quote' }
    Asciidoctor::AttributeList.new(line).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
    );

    let a = parse_attrlist("quote");
    assert_eq!(a.nth(1), Some("quote"));
    assert!(a.named.is_empty());
}

#[test]
fn collect_unnamed_attribute_double_quoted() {
    verifies!(
        r#"
  test 'collect unnamed attribute double-quoted' do
    attributes = {}
    line = '"quote"'
    expected = { 1 => 'quote' }
    Asciidoctor::AttributeList.new(line).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
    );

    let a = parse_attrlist(r#""quote""#);
    assert_eq!(a.nth(1), Some("quote"));
    assert!(a.named.is_empty());
}

#[test]
fn collect_empty_unnamed_attribute_double_quoted() {
    verifies!(
        r#"
  test 'collect empty unnamed attribute double-quoted' do
    attributes = {}
    line = '""'
    expected = { 1 => '' }
    Asciidoctor::AttributeList.new(line).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
    );

    let a = parse_attrlist(r#""""#);
    assert_eq!(a.nth(1), Some(""));
    assert!(a.named.is_empty());
}

#[test]
fn collect_unnamed_attribute_double_quoted_containing_escaped_quote() {
    verifies!(
        r#"
  test 'collect unnamed attribute double-quoted containing escaped quote' do
    attributes = {}
    line = '"ba\"zaar"'
    expected = { 1 => 'ba"zaar' }
    Asciidoctor::AttributeList.new(line).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
    );

    let a = parse_attrlist(r#""ba\"zaar""#);
    assert_eq!(a.nth(1), Some(r#"ba"zaar"#));
    assert!(a.named.is_empty());
}

#[test]
fn collect_unnamed_attribute_single_quoted() {
    verifies!(
        r#"
  test 'collect unnamed attribute single-quoted' do
    attributes = {}
    line = '\'quote\''
    expected = { 1 => 'quote' }
    Asciidoctor::AttributeList.new(line).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
    );

    let a = parse_attrlist("'quote'");
    assert_eq!(a.nth(1), Some("quote"));
    assert!(a.named.is_empty());
}

#[test]
fn collect_empty_unnamed_attribute_single_quoted() {
    verifies!(
        r#"
  test 'collect empty unnamed attribute single-quoted' do
    attributes = {}
    line = '\'\''
    expected = { 1 => '' }
    Asciidoctor::AttributeList.new(line).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
    );

    let a = parse_attrlist("''");
    assert_eq!(a.nth(1), Some(""));
    assert!(a.named.is_empty());
}

#[test]
fn collect_isolated_single_quote_positional_attribute() {
    verifies!(
        r#"
  test 'collect isolated single quote positional attribute' do
    attributes = {}
    line = '\''
    expected = { 1 => '\'' }
    doc = empty_document
    def doc.apply_subs *args
      raise 'apply_subs should not be called'
    end
    Asciidoctor::AttributeList.new(line, doc).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
    );

    // A lone single quote has no terminator, so it is a literal positional value
    // (`'`), not a quoted string. `asciidoc-parser` additionally emits a lenient
    // "missing terminating quote" warning, which these value-focused tests
    // disregard.
    let a = parse_attrlist("'");
    assert_eq!(a.nth(1), Some("'"));
    assert!(a.named.is_empty());
}

#[test]
fn collect_isolated_single_quote_attribute_value() {
    verifies!(
        r#"
  test 'collect isolated single quote attribute value' do
    attributes = {}
    line = 'name=\''
    expected = { 'name' => '\'' }
    doc = empty_document
    def doc.apply_subs *args
      raise 'apply_subs should not be called'
    end
    Asciidoctor::AttributeList.new(line, doc).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
    );

    let a = parse_attrlist("name='");
    assert_eq!(a.named("name"), Some("'"));
    assert!(a.positional.is_empty());
}

// This crate resolves attribute references at the whole-attrlist level before
// parsing, so `name='{val}` with `val` defined resolves to `'val` rather than
// staying literal. Asciidoctor keeps it literal here only because the value is
// a leading-quote-only literal (not single-quoted) *and* the test stubs
// `apply_subs`. This is a substitution-timing difference, not single-quote
// handling.
non_normative!(
    r#"
  test 'collect attribute value as is if it has only leading single quote' do
    attributes = {}
    line = 'name=\'{val}'
    expected = { 'name' => '\'{val}' }
    doc = empty_document attributes: { 'val' => 'val' }
    def doc.apply_subs *args
      raise 'apply_subs should not be called'
    end
    Asciidoctor::AttributeList.new(line, doc).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
);

#[test]
fn collect_unnamed_attribute_single_quoted_containing_escaped_quote() {
    verifies!(
        r#"
  test 'collect unnamed attribute single-quoted containing escaped quote' do
    attributes = {}
    line = '\'ba\\\'zaar\''
    expected = { 1 => 'ba\'zaar' }
    Asciidoctor::AttributeList.new(line).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
    );

    // A genuinely single-quoted value has the normal substitution group applied
    // in a block context — exactly Asciidoctor's behavior when a block is present
    // (`single_quoted && @block`). So the unescaped `ba'zaar` becomes
    // `ba&#8217;zaar` (typographic apostrophe), where the Ruby unit test, which
    // stubs `apply_subs`, sees the literal `ba'zaar`.
    let a = parse_attrlist("'ba\\'zaar'");
    assert_eq!(a.nth(1), Some("ba&#8217;zaar"));
    assert!(a.named.is_empty());
}

#[test]
fn collect_unnamed_attribute_with_dangling_delimiter() {
    verifies!(
        r#"
  test 'collect unnamed attribute with dangling delimiter' do
    attributes = {}
    line = 'quote , '
    expected = { 1 => 'quote', 2 => nil }
    Asciidoctor::AttributeList.new(line).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
    );

    // The dangling delimiter leaves position 2 blank (`nil` → `None`).
    let a = parse_attrlist("quote , ");
    assert_eq!(a.nth(1), Some("quote"));
    assert_eq!(a.nth(2), None);
}

#[test]
fn collect_unnamed_attribute_in_second_position_after_empty_attribute() {
    verifies!(
        r#"
  test 'collect unnamed attribute in second position after empty attribute' do
    attributes = {}
    line = ', John Smith'
    expected = { 1 => nil, 2 => 'John Smith' }
    Asciidoctor::AttributeList.new(line).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
    );

    // The leading delimiter leaves position 1 blank (`nil` → `None`).
    let a = parse_attrlist(", John Smith");
    assert_eq!(a.nth(1), None);
    assert_eq!(a.nth(2), Some("John Smith"));
}

#[test]
fn collect_unnamed_attributes() {
    verifies!(
        r#"
  test 'collect unnamed attributes' do
    attributes = {}
    line = 'first, second one, third'
    expected = { 1 => 'first', 2 => 'second one', 3 => 'third' }
    Asciidoctor::AttributeList.new(line).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
    );

    let a = parse_attrlist("first, second one, third");
    assert_eq!(a.nth(1), Some("first"));
    assert_eq!(a.nth(2), Some("second one"));
    assert_eq!(a.nth(3), Some("third"));
    assert!(a.named.is_empty());
}

#[test]
fn collect_blank_unnamed_attributes() {
    verifies!(
        r#"
  test 'collect blank unnamed attributes' do
    attributes = {}
    line = 'first,,third,'
    expected = { 1 => 'first', 2 => nil, 3 => 'third', 4 => nil }
    Asciidoctor::AttributeList.new(line).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
    );

    // The blank middle and trailing slots are `nil` (→ `None`); `first` and
    // `third` keep positions 1 and 3.
    let a = parse_attrlist("first,,third,");
    assert_eq!(a.nth(1), Some("first"));
    assert_eq!(a.nth(2), None);
    assert_eq!(a.nth(3), Some("third"));
    assert_eq!(a.nth(4), None);
    assert!(a.named.is_empty());
}

#[test]
fn collect_unnamed_attribute_enclosed_in_equal_signs() {
    verifies!(
        r#"
  test 'collect unnamed attribute enclosed in equal signs' do
    attributes = {}
    line = '=foo='
    expected = { 1 => '=foo=' }
    Asciidoctor::AttributeList.new(line).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
    );

    let a = parse_attrlist("=foo=");
    assert_eq!(a.nth(1), Some("=foo="));
    assert!(a.named.is_empty());
}

#[test]
fn collect_named_attribute() {
    verifies!(
        r#"
  test 'collect named attribute' do
    attributes = {}
    line = 'foo=bar'
    expected = { 'foo' => 'bar' }
    Asciidoctor::AttributeList.new(line).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
    );

    let a = parse_attrlist("foo=bar");
    assert_eq!(a.named("foo"), Some("bar"));
    assert!(a.positional.is_empty());
}

#[test]
fn collect_named_attribute_double_quoted() {
    verifies!(
        r#"
  test 'collect named attribute double-quoted' do
    attributes = {}
    line = 'foo="bar"'
    expected = { 'foo' => 'bar' }
    Asciidoctor::AttributeList.new(line).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
    );

    let a = parse_attrlist(r#"foo="bar""#);
    assert_eq!(a.named("foo"), Some("bar"));
    assert!(a.positional.is_empty());
}

#[test]
fn collect_named_attribute_with_double_quoted_empty_value() {
    verifies!(
        r#"
  test 'collect named attribute with double-quoted empty value' do
    attributes = {}
    line = 'height=100,caption="",link="images/octocat.png"'
    expected = { 'height' => '100', 'caption' => '', 'link' => 'images/octocat.png' }
    Asciidoctor::AttributeList.new(line).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
    );

    let a = parse_attrlist(r#"height=100,caption="",link="images/octocat.png""#);
    assert_eq!(a.named("height"), Some("100"));
    assert_eq!(a.named("caption"), Some(""));
    assert_eq!(a.named("link"), Some("images/octocat.png"));
}

#[test]
fn collect_named_attribute_single_quoted() {
    verifies!(
        r#"
  test 'collect named attribute single-quoted' do
    attributes = {}
    line = 'foo=\'bar\''
    expected = { 'foo' => 'bar' }
    Asciidoctor::AttributeList.new(line).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
    );

    let a = parse_attrlist("foo='bar'");
    assert_eq!(a.named("foo"), Some("bar"));
    assert!(a.positional.is_empty());
}

#[test]
fn collect_named_attribute_with_single_quoted_empty_value() {
    verifies!(
        r#"
  test 'collect named attribute with single-quoted empty value' do
    attributes = {}
    line = %(height=100,caption='',link='images/octocat.png')
    expected = { 'height' => '100', 'caption' => '', 'link' => 'images/octocat.png' }
    Asciidoctor::AttributeList.new(line).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
    );

    let a = parse_attrlist("height=100,caption='',link='images/octocat.png'");
    assert_eq!(a.named("height"), Some("100"));
    assert_eq!(a.named("caption"), Some(""));
    assert_eq!(a.named("link"), Some("images/octocat.png"));
}

#[test]
fn collect_single_named_attribute_with_empty_value() {
    verifies!(
        r#"
  test 'collect single named attribute with empty value' do
    attributes = {}
    line = 'foo='
    expected = { 'foo' => '' }
    Asciidoctor::AttributeList.new(line).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
    );

    let a = parse_attrlist("foo=");
    assert_eq!(a.named("foo"), Some(""));
    assert!(a.positional.is_empty());
}

#[test]
fn collect_single_named_attribute_with_empty_value_when_followed_by_other_attributes() {
    verifies!(
        r#"
  test 'collect single named attribute with empty value when followed by other attributes' do
    attributes = {}
    line = 'foo=,bar=baz'
    expected = { 'foo' => '', 'bar' => 'baz' }
    Asciidoctor::AttributeList.new(line).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
    );

    let a = parse_attrlist("foo=,bar=baz");
    assert_eq!(a.named("foo"), Some(""));
    assert_eq!(a.named("bar"), Some("baz"));
}

#[test]
fn collect_named_attributes_unquoted() {
    verifies!(
        r#"
  test 'collect named attributes unquoted' do
    attributes = {}
    line = 'first=value, second=two, third=3'
    expected = { 'first' => 'value', 'second' => 'two', 'third' => '3' }
    Asciidoctor::AttributeList.new(line).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
    );

    let a = parse_attrlist("first=value, second=two, third=3");
    assert_eq!(a.named("first"), Some("value"));
    assert_eq!(a.named("second"), Some("two"));
    assert_eq!(a.named("third"), Some("3"));
}

#[test]
fn collect_named_attributes_quoted() {
    verifies!(
        r#"
  test 'collect named attributes quoted' do
    attributes = {}
    line = %(first='value', second="value two", third=three)
    expected = { 'first' => 'value', 'second' => 'value two', 'third' => 'three' }
    Asciidoctor::AttributeList.new(line).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
    );

    let a = parse_attrlist(r#"first='value', second="value two", third=three"#);
    assert_eq!(a.named("first"), Some("value"));
    assert_eq!(a.named("second"), Some("value two"));
    assert_eq!(a.named("third"), Some("three"));
}

// `asciidoc-parser` does not strip leading, non-semantic whitespace before the
// first attribute name, so `     first    = ...` is not recognized as a named
// attribute (it becomes a positional literal). Asciidoctor skips that leading
// blank. Kept `non_normative!`.
non_normative!(
    r#"
  test 'collect named attributes quoted containing non-semantic spaces' do
    attributes = {}
    line = %(     first    =     'value', second     ="value two"     , third=       three      )
    expected = { 'first' => 'value', 'second' => 'value two', 'third' => 'three' }
    Asciidoctor::AttributeList.new(line).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
);

#[test]
fn collect_mixed_named_and_unnamed_attributes() {
    verifies!(
        r#"
  test 'collect mixed named and unnamed attributes' do
    attributes = {}
    line = %(first, second="value two", third=three, Sherlock Holmes)
    expected = { 1 => 'first', 'second' => 'value two', 'third' => 'three', 4 => 'Sherlock Holmes' }
    Asciidoctor::AttributeList.new(line).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
    );

    // The two interleaved named entries occupy positions 2 and 3, so the
    // trailing positional is at position 4 (as in Asciidoctor).
    let a = parse_attrlist(r#"first, second="value two", third=three, Sherlock Holmes"#);
    assert_eq!(a.nth(1), Some("first"));
    assert_eq!(a.named("second"), Some("value two"));
    assert_eq!(a.named("third"), Some("three"));
    assert_eq!(a.nth(4), Some("Sherlock Holmes"));
}

#[test]
fn collect_mixed_empty_named_and_blank_unnamed_attributes() {
    verifies!(
        r#"
  test 'collect mixed empty named and blank unnamed attributes' do
    attributes = {}
    line = 'first,,third=,,fifth=five'
    expected = { 1 => 'first', 2 => nil, 'third' => '', 4 => nil, 'fifth' => 'five' }
    Asciidoctor::AttributeList.new(line).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
    );

    // Positions: 1 = `first`, 2 = blank (`nil`), 3 = named `third`, 4 = blank
    // (`nil`), 5 = named `fifth`.
    let a = parse_attrlist("first,,third=,,fifth=five");
    assert_eq!(a.nth(1), Some("first"));
    assert_eq!(a.nth(2), None);
    assert_eq!(a.named("third"), Some(""));
    assert_eq!(a.nth(4), None);
    assert_eq!(a.named("fifth"), Some("five"));
}

#[test]
fn collect_options_attribute() {
    verifies!(
        r#"
  test 'collect options attribute' do
    attributes = {}
    line = %(quote, options='opt1,,opt2 , opt3')
    expected = { 1 => 'quote', 'opt1-option' => '', 'opt2-option' => '', 'opt3-option' => '' }
    Asciidoctor::AttributeList.new(line).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
    );

    // Option tokens are split on commas, trimmed, and empties dropped.
    let a = parse_attrlist("quote, options='opt1,,opt2 , opt3'");
    assert_eq!(a.nth(1), Some("quote"));
    assert_eq!(a.options(), ["opt1", "opt2", "opt3"]);
}

#[test]
fn collect_opts_attribute_as_options() {
    verifies!(
        r#"
  test 'collect opts attribute as options' do
    attributes = {}
    line = %(quote, opts='opt1,,opt2 , opt3')
    expected = { 1 => 'quote', 'opt1-option' => '', 'opt2-option' => '', 'opt3-option' => '' }
    Asciidoctor::AttributeList.new(line).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
    );

    let a = parse_attrlist("quote, opts='opt1,,opt2 , opt3'");
    assert_eq!(a.nth(1), Some("quote"));
    assert_eq!(a.options(), ["opt1", "opt2", "opt3"]);
}

#[test]
fn should_ignore_options_attribute_if_empty() {
    verifies!(
        r#"
  test 'should ignore options attribute if empty' do
    attributes = {}
    line = %(quote, opts=)
    expected = { 1 => 'quote' }
    Asciidoctor::AttributeList.new(line).parse_into(attributes)
    assert_equal expected, attributes
  end

"#
    );

    // An empty `opts=` contributes no options.
    let a = parse_attrlist("quote, opts=");
    assert_eq!(a.nth(1), Some("quote"));
    assert!(a.options().is_empty());
}

// The remaining tests exercise `parse_into`'s positional rekeying (mapping
// positional attributes onto supplied names) and the static
// `AttributeList.rekey` helper. `asciidoc-parser` exposes neither — it offers
// only per-lookup name-or-position resolution
// (`named_or_positional_attribute`), not a rekey that folds positional names
// into the attribute set. Kept `non_normative!`.
non_normative!(
    r#"
  test 'collect and rekey unnamed attributes' do
    attributes = {}
    line = 'first, second one, third, fourth'
    expected = { 1 => 'first', 2 => 'second one', 3 => 'third', 4 => 'fourth', 'a' => 'first', 'b' => 'second one', 'c' => 'third' }
    Asciidoctor::AttributeList.new(line).parse_into(attributes, ['a', 'b', 'c'])
    assert_equal expected, attributes
  end

  test 'should not assign nil to attribute mapped to missing positional attribute' do
    attributes = {}
    line = 'alt text,,100'
    expected = { 1 => 'alt text', 2 => nil, 3 => '100', 'alt' => 'alt text', 'height' => '100' }
    Asciidoctor::AttributeList.new(line).parse_into(attributes, %w(alt width height))
    assert_equal expected, attributes
  end

  test 'rekey positional attributes' do
    attributes = { 1 => 'source', 2 => 'java' }
    expected = { 1 => 'source', 2 => 'java', 'style' => 'source', 'language' => 'java' }
    Asciidoctor::AttributeList.rekey(attributes, ['style', 'language', 'linenums'])
    assert_equal expected, attributes
  end
end
"#
);
