// Adapted from Asciidoctor's parser test suite, found in
// https://github.com/asciidoctor/asciidoctor/blob/main/test/parser_test.rb.
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
//! Port of Asciidoctor's `parser_test.rb`.
//!
//! Unlike the other suites ported here, `parser_test.rb` is almost entirely a
//! unit test of `Asciidoctor::Parser`'s **internal static helpers**
//! (`is_section_title?`, `sanitize_attribute_name`, `store_attribute`,
//! `parse_style_attribute`, `parse_header_metadata`, `adjust_indentation!`).
//! This crate does not expose those helpers by name, but most of the *behavior*
//! they implement is observable through the public API, so those tests are
//! `verifies!` and driven through the real path. The helpers with no public
//! analog at all (`is_section_title?`, `adjust_indentation!`) stay
//! `non_normative!`: the Ruby lines are reproduced verbatim to keep the `sdd`
//! coverage tool aligned, but no crate behavior is asserted against them.
//!
//! The `verifies!` mappings:
//!
//! * **`store_attribute`** — a `:name: value` attribute entry drives
//!   [`Parser::attribute_value`]; a negated name (`:name!:` / `:!name:`) unsets
//!   it; and the accessible/inaccessible distinction (may a document entry
//!   override an API-supplied value?) is modeled by
//!   [`ModificationContext`](crate::parser::ModificationContext) — `Anywhere`
//!   is accessible, `ApiOnly` is inaccessible (a rejected override is reported
//!   as [`WarningType::AttributeValueIsLocked`]).
//! * **`sanitize_attribute_name`** — an attribute entry is stored under its
//!   sanitized name (lower-cased, characters outside `[a-z0-9_-]` dropped), so
//!   the tests define a `:Raw Name:` entry and read it back through
//!   [`Parser::attribute_value`] under its sanitized key.
//! * **`parse_style_attribute`** — the shorthand `style#id.role%opt` parsing is
//!   exactly what [`Attrlist`](crate::attributes::Attrlist) does, so those
//!   tests drive [`Attrlist::parse`] in a block context and assert on
//!   `block_style()` / `id()` / `roles()` / `options()`. (The two cases that
//!   pre-seed a `style`/`option` into the attribute hash before calling the
//!   helper have no attrlist-syntax equivalent and stay `non_normative!`.)
//! * **`parse_header_metadata`** author parsing — driven through
//!   [`AuthorLine::parse`](crate::document::AuthorLine), asserting on the
//!   parsed [`Author`](crate::document::Author) list (name / firstname /
//!   middlename / lastname / email / initials). This crate does not derive
//!   Ruby's `authors` (comma-joined) or `authorcount` document attributes, and
//!   numbers the first author's attributes unsuffixed (`author`, not
//!   `author_1`), so assertions on those are dropped. The Ruby `metadata.size`
//!   (hash cardinality) has no crate analog and is likewise not asserted.
//! * **`parse_header_metadata`** revision parsing — driven through a full
//!   [`Parser::parse`] over the header (title, author line, revision line) and
//!   asserting on `doc.header().revision_line()`'s `revnumber()` / `revdate()`
//!   / `revremark()`. Parsing the whole header (rather than calling
//!   [`RevisionLine::parse`](crate::document::RevisionLine) on an
//!   already-chosen string) also covers selection of the revision line as the
//!   line after the author line.
//! * A few header-structure tests (comment skipping, attribute-entry
//!   interactions) are driven through a full [`Parser::parse`]. Because this
//!   crate only recognizes an author/revision line when a document title is
//!   present, those inputs are given a synthetic `= Document Title` line; this
//!   is noted inline.
//!
//! **Incompatibilities surfaced.** Several tests are `non_normative!` not
//! because the API is unimplemented but because this crate's observable
//! behavior genuinely differs from Asciidoctor. Each is annotated inline with
//! its tracking issue; the notable ones:
//!
//! * The author-name fallback does not condense interior whitespace and
//!   HTML-encodes angle brackets, so `author` differs from Ruby's (issue #756).
//! * Implicit author lines are split on `"; "` (semicolon + space) only, so a
//!   trailing bare `;` mangles the final author (issue #757).
//! * The `:author:` attribute-entry path does not partition a 4+-part name
//!   (issue #758).
//! * Block comments (`////`) in the header are not skipped ahead of the author
//!   or revision line (issue #760).
//! * A duplicate id introduced by an *inline* anchor (`[[id]]`) does not emit a
//!   `DuplicateId` warning (the error is discarded on the inline path), unlike
//!   a duplicate block anchor (issue #762).

use crate::tests::prelude::*;

track_file!("ref/asciidoctor/test/parser_test.rb");

/// The shorthand style/id/role/option attributes parsed from a raw attrlist
/// string, collected into owned data. Mirrors what Asciidoctor's
/// `parse_style_attribute` extracts from positional attribute 1.
struct ParsedStyle {
    style: Option<String>,
    id: Option<String>,
    roles: Vec<String>,
    options: Vec<String>,
}

/// Parse a raw attrlist string in a block context (as Asciidoctor does when it
/// calls `parse_style_attribute` on a block's attribute line) and collect the
/// shorthand results.
fn parse_style(line: &str) -> ParsedStyle {
    let parser = Parser::default();
    let maw = crate::attributes::Attrlist::parse(
        crate::Span::new(line),
        &parser,
        crate::attributes::AttrlistContext::Block,
    );
    let attrlist = &maw.item.item;

    ParsedStyle {
        style: attrlist.block_style().map(str::to_string),
        id: attrlist.id().map(str::to_string),
        roles: attrlist.roles().iter().map(|r| r.to_string()).collect(),
        options: attrlist.options().iter().map(|o| o.to_string()).collect(),
    }
}

/// A single parsed author, collected into owned data so the borrowed
/// `AuthorLine` can be dropped before the caller asserts.
struct ParsedAuthor {
    name: String,
    firstname: String,
    middlename: Option<String>,
    lastname: Option<String>,
    email: Option<String>,
    initials: String,
}

/// Parse an author line the way Asciidoctor's `parse_header_metadata` does for
/// the implicit author line: through [`AuthorLine::parse`].
fn parse_authors(line: &str) -> Vec<ParsedAuthor> {
    let mut parser = Parser::default();
    let author_line = crate::document::AuthorLine::parse(crate::Span::new(line), &mut parser);

    author_line
        .authors()
        .map(|a| ParsedAuthor {
            name: a.name().to_string(),
            firstname: a.firstname().to_string(),
            middlename: a.middlename().map(str::to_string),
            lastname: a.lastname().map(str::to_string),
            email: a.email().map(str::to_string),
            initials: a.initials(),
        })
        .collect()
}

/// Define a document attribute entry whose raw (as-written) name is `raw_name`
/// and return the value stored under the canonical name `canonical`. This
/// drives Asciidoctor's `sanitize_attribute_name` through the public
/// attribute-entry path: the entry is only observable under its sanitized name.
fn entry_value_under(raw_name: &str, canonical: &str) -> crate::document::InterpretedValue {
    let mut parser = Parser::default();
    let _doc = parser.parse(&format!(":{raw_name}: sentinel"));
    parser.attribute_value(canonical)
}

/// Parse a full document header — `= Document Title`, the author line, and the
/// revision line — and return the parsed revision line's number / date / remark
/// as owned data. This drives the same header path Asciidoctor's
/// `parse_header_metadata` exercises, including *selection* of the revision
/// line as the line after the author line (not just parsing of an
/// already-isolated revision string). A synthetic document title is prepended
/// because this crate only recognizes an author/revision line when a title is
/// present.
fn header_rev(author_and_revision: &str) -> (Option<String>, String, Option<String>) {
    let input = format!("= Document Title\n{author_and_revision}");
    let mut parser = Parser::default();
    let doc = parser.parse(&input);
    let rev = doc
        .header()
        .revision_line()
        .expect("expected the header to yield a revision line");
    (
        rev.revnumber().map(str::to_string),
        rev.revdate().to_string(),
        rev.revremark().map(str::to_string),
    )
}

non_normative!(
    r#"
# frozen_string_literal: true
require_relative 'test_helper'

context "Parser" do
"#
);

// `is_section_title?` is an internal `Asciidoctor::Parser` predicate that this
// crate does not expose; the two-line (setext) form it exercises is not
// supported here either. Section recognition is covered by the `sections`
// suites.
non_normative!(
    r#"
  test "is_section_title?" do
    assert Asciidoctor::Parser.is_section_title?('AsciiDoc Home Page', '==================')
    assert Asciidoctor::Parser.is_section_title?('=== AsciiDoc Home Page')
  end

"#
);

// `sanitize_attribute_name` is an internal static helper not exposed by this
// crate by name, but its behavior is observable: an attribute entry is stored
// under its sanitized name, so a `:Raw Name:` entry is only reachable through
// the sanitized key. See
// https://github.com/asciidoc-rs/asciidoc-parser/issues/761.
#[test]
fn sanitize_attribute_name() {
    verifies!(
        r#"
  test 'sanitize attribute name' do
    assert_equal 'foobar', Asciidoctor::Parser.sanitize_attribute_name("Foo Bar")
    assert_equal 'foo', Asciidoctor::Parser.sanitize_attribute_name("foo")
    assert_equal 'foo3-bar', Asciidoctor::Parser.sanitize_attribute_name("Foo 3^ # - Bar[")
  end

"#
    );

    // `Foo Bar` -> `foobar`
    assert_eq!(
        entry_value_under("Foo Bar", "foobar"),
        InterpretedValue::Value("sentinel")
    );

    // `foo` -> `foo`
    assert_eq!(
        entry_value_under("foo", "foo"),
        InterpretedValue::Value("sentinel")
    );

    // `Foo 3^ # - Bar[` -> `foo3-bar`
    assert_eq!(
        entry_value_under("Foo 3^ # - Bar[", "foo3-bar"),
        InterpretedValue::Value("sentinel")
    );
}

// The `store_attribute` group tests the internal static helper that stores an
// attribute (optionally on a document). This crate exposes the same behavior
// through its public attribute API: [`Parser::with_intrinsic_attribute`] stores
// a value and [`Parser::with_intrinsic_attribute_bool`] with `false` stores it
// unset (the negated-name case). The *accessible* vs *inaccessible* distinction
// (whether a later document entry may override the stored value) is modeled by
// [`ModificationContext`](crate::parser::ModificationContext) — `Anywhere` is
// accessible, `ApiOnly` is inaccessible (and a rejected override is reported as
// [`WarningType::AttributeValueIsLocked`]). The Ruby helper's return tuple and
// its `:attribute_entries` accumulator are Ruby-internal bookkeeping and are
// not asserted.

#[test]
fn store_attribute_with_value() {
    verifies!(
        r#"
  test 'store attribute with value' do
    attr_name, attr_value = Asciidoctor::Parser.store_attribute 'foo', 'bar'
    assert_equal 'foo', attr_name
    assert_equal 'bar', attr_value
  end

"#
    );

    // Constructing the parser with the attribute makes it active for a parsed
    // document: `foo` is present on the resulting `Document` and resolves when a
    // `{foo}` reference is substituted.
    let mut parser =
        Parser::default().with_intrinsic_attribute("foo", "bar", ModificationContext::Anywhere);
    let doc = parser.parse("{foo}");
    assert_eq!(doc.attribute_value("foo"), InterpretedValue::Value("bar"));
    assert_eq!(rendered_paragraphs(&doc), vec!["bar".to_string()]);
}

#[test]
fn store_attribute_with_negated_value() {
    verifies!(
        r#"
  test 'store attribute with negated value' do
    { 'foo!' => nil, '!foo' => nil, 'foo' => nil }.each do |name, value|
      attr_name, attr_value = Asciidoctor::Parser.store_attribute name, value
      assert_equal name.sub('!', ''), attr_name
      assert_nil attr_value
    end
  end

"#
    );

    // A negated name (`foo!` or `!foo`) stores the attribute as unset; the API
    // models this with `with_intrinsic_attribute_bool(name, false, ..)`. The
    // unset value carries into a parsed document, and a `{foo}` reference
    // resolves to nothing.
    let mut parser = Parser::default().with_intrinsic_attribute_bool(
        "foo",
        false,
        ModificationContext::Anywhere,
    );
    let doc = parser.parse("{foo}");
    assert_eq!(doc.attribute_value("foo"), InterpretedValue::Unset);
    assert_eq!(rendered_paragraphs(&doc), vec![String::new()]);
}

#[test]
fn store_accessible_attribute_on_document_with_value() {
    verifies!(
        r#"
  test 'store accessible attribute on document with value' do
    doc = empty_document
    doc.set_attribute 'foo', 'baz'
    attrs = {}
    attr_name, attr_value = Asciidoctor::Parser.store_attribute 'foo', 'bar', doc, attrs
    assert_equal 'foo', attr_name
    assert_equal 'bar', attr_value
    assert_equal 'bar', (doc.attr 'foo')
    assert attrs.key?(:attribute_entries)
    assert_equal 1, attrs[:attribute_entries].size
    assert_equal 'foo', attrs[:attribute_entries][0].name
    assert_equal 'bar', attrs[:attribute_entries][0].value
  end

"#
    );

    // An accessible (document-modifiable) attribute is overridden by a later
    // `:foo:` entry.
    let mut parser =
        Parser::default().with_intrinsic_attribute("foo", "baz", ModificationContext::Anywhere);
    parser.parse(":foo: bar");
    assert_eq!(
        parser.attribute_value("foo"),
        InterpretedValue::Value("bar")
    );
}

#[test]
fn store_accessible_attribute_on_document_with_value_that_contains_attribute_reference() {
    verifies!(
        r#"
  test 'store accessible attribute on document with value that contains attribute reference' do
    doc = empty_document
    doc.set_attribute 'foo', 'baz'
    doc.set_attribute 'release', 'ultramega'
    attrs = {}
    attr_name, attr_value = Asciidoctor::Parser.store_attribute 'foo', '{release}', doc, attrs
    assert_equal 'foo', attr_name
    assert_equal 'ultramega', attr_value
    assert_equal 'ultramega', (doc.attr 'foo')
    assert attrs.key?(:attribute_entries)
    assert_equal 1, attrs[:attribute_entries].size
    assert_equal 'foo', attrs[:attribute_entries][0].name
    assert_equal 'ultramega', attrs[:attribute_entries][0].value
  end

"#
    );

    // The attribute reference in the value is resolved when the entry is
    // applied.
    let mut parser = Parser::default()
        .with_intrinsic_attribute("foo", "baz", ModificationContext::Anywhere)
        .with_intrinsic_attribute("release", "ultramega", ModificationContext::Anywhere);
    parser.parse(":foo: {release}");
    assert_eq!(
        parser.attribute_value("foo"),
        InterpretedValue::Value("ultramega")
    );
}

#[test]
fn store_inaccessible_attribute_on_document_with_value() {
    verifies!(
        r#"
  test 'store inaccessible attribute on document with value' do
    doc = empty_document attributes: { 'foo' => 'baz' }
    attrs = {}
    attr_name, attr_value = Asciidoctor::Parser.store_attribute 'foo', 'bar', doc, attrs
    assert_equal 'foo', attr_name
    assert_equal 'bar', attr_value
    assert_equal 'baz', (doc.attr 'foo')
    refute attrs.key?(:attribute_entries)
  end

"#
    );

    // An inaccessible (API-locked) attribute can't be overridden by a document
    // entry; the value stays and the rejected override is reported.
    let mut parser =
        Parser::default().with_intrinsic_attribute("foo", "baz", ModificationContext::ApiOnly);
    let doc = parser.parse(":foo: bar");
    assert_eq!(
        parser.attribute_value("foo"),
        InterpretedValue::Value("baz")
    );
    assert!(
        doc.warnings()
            .any(|w| w.warning == WarningType::AttributeValueIsLocked("foo".to_string()))
    );
}

#[test]
fn store_accessible_attribute_on_document_with_negated_value() {
    verifies!(
        r#"
  test 'store accessible attribute on document with negated value' do
    { 'foo!' => nil, '!foo' => nil, 'foo' => nil }.each do |name, value|
      doc = empty_document
      doc.set_attribute 'foo', 'baz'
      attrs = {}
      attr_name, attr_value = Asciidoctor::Parser.store_attribute name, value, doc, attrs
      assert_equal name.sub('!', ''), attr_name
      assert_nil attr_value
      assert attrs.key?(:attribute_entries)
      assert_equal 1, attrs[:attribute_entries].size
      assert_equal 'foo', attrs[:attribute_entries][0].name
      assert_nil attrs[:attribute_entries][0].value
    end
  end

"#
    );

    // A negated entry unsets an accessible attribute.
    for input in [":foo!:", ":!foo:"] {
        let mut parser =
            Parser::default().with_intrinsic_attribute("foo", "baz", ModificationContext::Anywhere);
        parser.parse(input);
        assert_eq!(parser.attribute_value("foo"), InterpretedValue::Unset);
    }
}

#[test]
fn store_inaccessible_attribute_on_document_with_negated_value() {
    verifies!(
        r#"
  test 'store inaccessible attribute on document with negated value' do
    { 'foo!' => nil, '!foo' => nil, 'foo' => nil }.each do |name, value|
      doc = empty_document attributes: { 'foo' => 'baz' }
      attrs = {}
      attr_name, attr_value = Asciidoctor::Parser.store_attribute name, value, doc, attrs
      assert_equal name.sub('!', ''), attr_name
      assert_nil attr_value
      refute attrs.key?(:attribute_entries)
    end
  end

"#
    );

    // A negated entry can't unset an inaccessible (API-locked) attribute.
    for input in [":foo!:", ":!foo:"] {
        let mut parser =
            Parser::default().with_intrinsic_attribute("foo", "baz", ModificationContext::ApiOnly);
        let doc = parser.parse(input);
        assert_eq!(
            parser.attribute_value("foo"),
            InterpretedValue::Value("baz")
        );
        assert!(
            doc.warnings()
                .any(|w| w.warning == WarningType::AttributeValueIsLocked("foo".to_string()))
        );
    }
}

#[test]
fn parse_style_attribute_with_id_and_role() {
    verifies!(
        r#"
  test 'parse style attribute with id and role' do
    attributes = { 1 => 'style#id.role' }
    style = Asciidoctor::Parser.parse_style_attribute(attributes)
    assert_equal 'style', style
    assert_equal 'style', attributes['style']
    assert_equal 'id', attributes['id']
    assert_equal 'role', attributes['role']
    assert_equal 'style#id.role', attributes[1]
  end

"#
    );

    let s = parse_style("style#id.role");
    assert_eq!(s.style.as_deref(), Some("style"));
    assert_eq!(s.id.as_deref(), Some("id"));
    assert_eq!(s.roles, vec!["role"]);
}

#[test]
fn parse_style_attribute_with_style_role_id_and_option() {
    verifies!(
        r#"
  test 'parse style attribute with style, role, id and option' do
    attributes = { 1 => 'style.role#id%fragment' }
    style = Asciidoctor::Parser.parse_style_attribute(attributes)
    assert_equal 'style', style
    assert_equal 'style', attributes['style']
    assert_equal 'id', attributes['id']
    assert_equal 'role', attributes['role']
    assert_equal '', attributes['fragment-option']
    assert_equal 'style.role#id%fragment', attributes[1]
    refute attributes.key? 'options'
  end

"#
    );

    let s = parse_style("style.role#id%fragment");
    assert_eq!(s.style.as_deref(), Some("style"));
    assert_eq!(s.id.as_deref(), Some("id"));
    assert_eq!(s.roles, vec!["role"]);
    assert_eq!(s.options, vec!["fragment"]);
}

#[test]
fn parse_style_attribute_with_style_id_and_multiple_roles() {
    verifies!(
        r#"
  test 'parse style attribute with style, id and multiple roles' do
    attributes = { 1 => 'style#id.role1.role2' }
    style = Asciidoctor::Parser.parse_style_attribute(attributes)
    assert_equal 'style', style
    assert_equal 'style', attributes['style']
    assert_equal 'id', attributes['id']
    assert_equal 'role1 role2', attributes['role']
    assert_equal 'style#id.role1.role2', attributes[1]
  end

"#
    );

    let s = parse_style("style#id.role1.role2");
    assert_eq!(s.style.as_deref(), Some("style"));
    assert_eq!(s.id.as_deref(), Some("id"));
    assert_eq!(s.roles, vec!["role1", "role2"]);
}

#[test]
fn parse_style_attribute_with_style_multiple_roles_and_id() {
    verifies!(
        r#"
  test 'parse style attribute with style, multiple roles and id' do
    attributes = { 1 => 'style.role1.role2#id' }
    style = Asciidoctor::Parser.parse_style_attribute(attributes)
    assert_equal 'style', style
    assert_equal 'style', attributes['style']
    assert_equal 'id', attributes['id']
    assert_equal 'role1 role2', attributes['role']
    assert_equal 'style.role1.role2#id', attributes[1]
  end

"#
    );

    let s = parse_style("style.role1.role2#id");
    assert_eq!(s.style.as_deref(), Some("style"));
    assert_eq!(s.id.as_deref(), Some("id"));
    assert_eq!(s.roles, vec!["role1", "role2"]);
}

// This case pre-seeds a `style` named entry into the attribute hash before
// calling `parse_style_attribute`, exercising the helper's "positional
// overrides an existing style" branch. There is no attrlist-syntax equivalent
// to a pre-existing named `style` attribute, so this stays non-normative.
non_normative!(
    r#"
  test 'parse style attribute with positional and original style' do
    attributes = { 1 => 'new_style', 'style' => 'original_style' }
    style = Asciidoctor::Parser.parse_style_attribute(attributes)
    assert_equal 'new_style', style
    assert_equal 'new_style', attributes['style']
    assert_equal 'new_style', attributes[1]
  end

"#
);

#[test]
fn parse_style_attribute_with_id_and_role_only() {
    verifies!(
        r#"
  test 'parse style attribute with id and role only' do
    attributes = { 1 => '#id.role' }
    style = Asciidoctor::Parser.parse_style_attribute(attributes)
    assert_nil style
    assert_equal 'id', attributes['id']
    assert_equal 'role', attributes['role']
    assert_equal '#id.role', attributes[1]
  end

"#
    );

    let s = parse_style("#id.role");
    assert_eq!(s.style, None);
    assert_eq!(s.id.as_deref(), Some("id"));
    assert_eq!(s.roles, vec!["role"]);
}

#[test]
fn parse_empty_style_attribute() {
    verifies!(
        r#"
  test 'parse empty style attribute' do
    attributes = { 1 => nil }
    style = Asciidoctor::Parser.parse_style_attribute(attributes)
    assert_nil style
    assert_nil attributes['id']
    assert_nil attributes['role']
    assert_nil attributes[1]
  end

"#
    );

    let s = parse_style("");
    assert_eq!(s.style, None);
    assert_eq!(s.id, None);
    assert!(s.roles.is_empty());
}

// This case pre-seeds a `footer-option` entry into the attribute hash and
// checks that `%header` shorthand does not clobber it. This crate's
// `Attrlist::parse` has no way to pre-seed an existing option, so the
// "preserve existing options" behavior can't be exercised; the `%header`
// shorthand itself is covered by the attribute suites.
non_normative!(
    r#"
  test 'parse style attribute with option should preserve existing options' do
    attributes = { 1 => '%header', 'footer-option' => '' }
    style = Asciidoctor::Parser.parse_style_attribute(attributes)
    assert_nil style
    assert_equal '', attributes['header-option']
    assert_equal '', attributes['footer-option']
  end

"#
);

#[test]
fn parse_author_first() {
    verifies!(
        r#"
  test "parse author first" do
    metadata, _ = parse_header_metadata 'Stuart'
    assert_equal 5, metadata.size
    assert_equal 1, metadata['authorcount']
    assert_equal metadata['author'], metadata['authors']
    assert_equal 'Stuart', metadata['firstname']
    assert_equal 'S', metadata['authorinitials']
  end

"#
    );

    let a = parse_authors("Stuart");
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, "Stuart");
    assert_eq!(a[0].firstname, "Stuart");
    assert_eq!(a[0].initials, "S");
}

#[test]
fn parse_author_first_last() {
    verifies!(
        r#"
  test "parse author first last" do
    metadata, _ = parse_header_metadata 'Yukihiro Matsumoto'
    assert_equal 6, metadata.size
    assert_equal 1, metadata['authorcount']
    assert_equal 'Yukihiro Matsumoto', metadata['author']
    assert_equal metadata['author'], metadata['authors']
    assert_equal 'Yukihiro', metadata['firstname']
    assert_equal 'Matsumoto', metadata['lastname']
    assert_equal 'YM', metadata['authorinitials']
  end

"#
    );

    let a = parse_authors("Yukihiro Matsumoto");
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, "Yukihiro Matsumoto");
    assert_eq!(a[0].firstname, "Yukihiro");
    assert_eq!(a[0].lastname.as_deref(), Some("Matsumoto"));
    assert_eq!(a[0].initials, "YM");
}

#[test]
fn parse_author_first_middle_last() {
    verifies!(
        r#"
  test "parse author first middle last" do
    metadata, _ = parse_header_metadata 'David Heinemeier Hansson'
    assert_equal 7, metadata.size
    assert_equal 1, metadata['authorcount']
    assert_equal 'David Heinemeier Hansson', metadata['author']
    assert_equal metadata['author'], metadata['authors']
    assert_equal 'David', metadata['firstname']
    assert_equal 'Heinemeier', metadata['middlename']
    assert_equal 'Hansson', metadata['lastname']
    assert_equal 'DHH', metadata['authorinitials']
  end

"#
    );

    let a = parse_authors("David Heinemeier Hansson");
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, "David Heinemeier Hansson");
    assert_eq!(a[0].firstname, "David");
    assert_eq!(a[0].middlename.as_deref(), Some("Heinemeier"));
    assert_eq!(a[0].lastname.as_deref(), Some("Hansson"));
    assert_eq!(a[0].initials, "DHH");
}

#[test]
fn parse_author_first_middle_last_email() {
    verifies!(
        r#"
  test "parse author first middle last email" do
    metadata, _ = parse_header_metadata 'David Heinemeier Hansson <rails@ruby-lang.org>'
    assert_equal 8, metadata.size
    assert_equal 1, metadata['authorcount']
    assert_equal 'David Heinemeier Hansson', metadata['author']
    assert_equal metadata['author'], metadata['authors']
    assert_equal 'David', metadata['firstname']
    assert_equal 'Heinemeier', metadata['middlename']
    assert_equal 'Hansson', metadata['lastname']
    assert_equal 'rails@ruby-lang.org', metadata['email']
    assert_equal 'DHH', metadata['authorinitials']
  end

"#
    );

    let a = parse_authors("David Heinemeier Hansson <rails@ruby-lang.org>");
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, "David Heinemeier Hansson");
    assert_eq!(a[0].firstname, "David");
    assert_eq!(a[0].middlename.as_deref(), Some("Heinemeier"));
    assert_eq!(a[0].lastname.as_deref(), Some("Hansson"));
    assert_eq!(a[0].email.as_deref(), Some("rails@ruby-lang.org"));
    assert_eq!(a[0].initials, "DHH");
}

#[test]
fn parse_author_first_email() {
    verifies!(
        r#"
  test "parse author first email" do
    metadata, _ = parse_header_metadata 'Stuart <founder@asciidoc.org>'
    assert_equal 6, metadata.size
    assert_equal 1, metadata['authorcount']
    assert_equal 'Stuart', metadata['author']
    assert_equal metadata['author'], metadata['authors']
    assert_equal 'Stuart', metadata['firstname']
    assert_equal 'founder@asciidoc.org', metadata['email']
    assert_equal 'S', metadata['authorinitials']
  end

"#
    );

    let a = parse_authors("Stuart <founder@asciidoc.org>");
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, "Stuart");
    assert_eq!(a[0].firstname, "Stuart");
    assert_eq!(a[0].email.as_deref(), Some("founder@asciidoc.org"));
    assert_eq!(a[0].initials, "S");
}

#[test]
fn parse_author_first_last_email() {
    verifies!(
        r#"
  test "parse author first last email" do
    metadata, _ = parse_header_metadata 'Stuart Rackham <founder@asciidoc.org>'
    assert_equal 7, metadata.size
    assert_equal 1, metadata['authorcount']
    assert_equal 'Stuart Rackham', metadata['author']
    assert_equal metadata['author'], metadata['authors']
    assert_equal 'Stuart', metadata['firstname']
    assert_equal 'Rackham', metadata['lastname']
    assert_equal 'founder@asciidoc.org', metadata['email']
    assert_equal 'SR', metadata['authorinitials']
  end

"#
    );

    let a = parse_authors("Stuart Rackham <founder@asciidoc.org>");
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, "Stuart Rackham");
    assert_eq!(a[0].firstname, "Stuart");
    assert_eq!(a[0].lastname.as_deref(), Some("Rackham"));
    assert_eq!(a[0].email.as_deref(), Some("founder@asciidoc.org"));
    assert_eq!(a[0].initials, "SR");
}

#[test]
fn parse_author_with_hyphen() {
    verifies!(
        r#"
  test "parse author with hyphen" do
    metadata, _ = parse_header_metadata 'Tim Berners-Lee <founder@www.org>'
    assert_equal 7, metadata.size
    assert_equal 1, metadata['authorcount']
    assert_equal 'Tim Berners-Lee', metadata['author']
    assert_equal metadata['author'], metadata['authors']
    assert_equal 'Tim', metadata['firstname']
    assert_equal 'Berners-Lee', metadata['lastname']
    assert_equal 'founder@www.org', metadata['email']
    assert_equal 'TB', metadata['authorinitials']
  end

"#
    );

    let a = parse_authors("Tim Berners-Lee <founder@www.org>");
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, "Tim Berners-Lee");
    assert_eq!(a[0].firstname, "Tim");
    assert_eq!(a[0].lastname.as_deref(), Some("Berners-Lee"));
    assert_eq!(a[0].email.as_deref(), Some("founder@www.org"));
    assert_eq!(a[0].initials, "TB");
}

#[test]
fn parse_author_with_single_quote() {
    verifies!(
        r#"
  test "parse author with single quote" do
    metadata, _ = parse_header_metadata 'Stephen O\'Grady <founder@redmonk.com>'
    assert_equal 7, metadata.size
    assert_equal 1, metadata['authorcount']
    assert_equal 'Stephen O\'Grady', metadata['author']
    assert_equal metadata['author'], metadata['authors']
    assert_equal 'Stephen', metadata['firstname']
    assert_equal 'O\'Grady', metadata['lastname']
    assert_equal 'founder@redmonk.com', metadata['email']
    assert_equal 'SO', metadata['authorinitials']
  end

"#
    );

    let a = parse_authors("Stephen O'Grady <founder@redmonk.com>");
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, "Stephen O'Grady");
    assert_eq!(a[0].firstname, "Stephen");
    assert_eq!(a[0].lastname.as_deref(), Some("O'Grady"));
    assert_eq!(a[0].email.as_deref(), Some("founder@redmonk.com"));
    assert_eq!(a[0].initials, "SO");
}

#[test]
fn parse_author_with_dotted_initial() {
    verifies!(
        r#"
  test "parse author with dotted initial" do
    metadata, _ = parse_header_metadata 'Heiko W. Rupp <hwr@example.de>'
    assert_equal 8, metadata.size
    assert_equal 1, metadata['authorcount']
    assert_equal 'Heiko W. Rupp', metadata['author']
    assert_equal metadata['author'], metadata['authors']
    assert_equal 'Heiko', metadata['firstname']
    assert_equal 'W.', metadata['middlename']
    assert_equal 'Rupp', metadata['lastname']
    assert_equal 'hwr@example.de', metadata['email']
    assert_equal 'HWR', metadata['authorinitials']
  end

"#
    );

    let a = parse_authors("Heiko W. Rupp <hwr@example.de>");
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, "Heiko W. Rupp");
    assert_eq!(a[0].firstname, "Heiko");
    assert_eq!(a[0].middlename.as_deref(), Some("W."));
    assert_eq!(a[0].lastname.as_deref(), Some("Rupp"));
    assert_eq!(a[0].email.as_deref(), Some("hwr@example.de"));
    assert_eq!(a[0].initials, "HWR");
}

#[test]
fn parse_author_with_underscore() {
    verifies!(
        r#"
  test "parse author with underscore" do
    metadata, _ = parse_header_metadata 'Tim_E Fella'
    assert_equal 6, metadata.size
    assert_equal 1, metadata['authorcount']
    assert_equal 'Tim E Fella', metadata['author']
    assert_equal metadata['author'], metadata['authors']
    assert_equal 'Tim E', metadata['firstname']
    assert_equal 'Fella', metadata['lastname']
    assert_equal 'TF', metadata['authorinitials']
  end

"#
    );

    let a = parse_authors("Tim_E Fella");
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, "Tim E Fella");
    assert_eq!(a[0].firstname, "Tim E");
    assert_eq!(a[0].lastname.as_deref(), Some("Fella"));
    assert_eq!(a[0].initials, "TF");
}

#[test]
fn parse_author_name_with_letters_outside_basic_latin() {
    verifies!(
        r#"
  test 'parse author name with letters outside basic latin' do
    metadata, _ = parse_header_metadata 'Stéphane Brontë'
    assert_equal 6, metadata.size
    assert_equal 1, metadata['authorcount']
    assert_equal 'Stéphane Brontë', metadata['author']
    assert_equal metadata['author'], metadata['authors']
    assert_equal 'Stéphane', metadata['firstname']
    assert_equal 'Brontë', metadata['lastname']
    assert_equal 'SB', metadata['authorinitials']
  end

"#
    );

    let a = parse_authors("Stéphane Brontë");
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, "Stéphane Brontë");
    assert_eq!(a[0].firstname, "Stéphane");
    assert_eq!(a[0].lastname.as_deref(), Some("Brontë"));
    assert_eq!(a[0].initials, "SB");
}

#[test]
fn parse_ideographic_author_names() {
    verifies!(
        r#"
  test 'parse ideographic author names' do
    metadata, _ = parse_header_metadata '李 四 <si.li@example.com>'
    assert_equal 7, metadata.size
    assert_equal 1, metadata['authorcount']
    assert_equal '李 四', metadata['author']
    assert_equal metadata['author'], metadata['authors']
    assert_equal '李', metadata['firstname']
    assert_equal '四', metadata['lastname']
    assert_equal 'si.li@example.com', metadata['email']
    assert_equal '李四', metadata['authorinitials']
  end

"#
    );

    let a = parse_authors("李 四 <si.li@example.com>");
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, "李 四");
    assert_eq!(a[0].firstname, "李");
    assert_eq!(a[0].lastname.as_deref(), Some("四"));
    assert_eq!(a[0].email.as_deref(), Some("si.li@example.com"));
    assert_eq!(a[0].initials, "李四");
}

#[test]
fn parse_author_condenses_whitespace() {
    verifies!(
        r#"
  test "parse author condenses whitespace" do
    metadata, _ = parse_header_metadata 'Stuart       Rackham     <founder@asciidoc.org>'
    assert_equal 7, metadata.size
    assert_equal 1, metadata['authorcount']
    assert_equal 'Stuart Rackham', metadata['author']
    assert_equal metadata['author'], metadata['authors']
    assert_equal 'Stuart', metadata['firstname']
    assert_equal 'Rackham', metadata['lastname']
    assert_equal 'founder@asciidoc.org', metadata['email']
    assert_equal 'SR', metadata['authorinitials']
  end

"#
    );

    // This crate does not derive the `authors` or `authorcount` attributes, so we
    // assert on the parsed author directly. The interior whitespace between the
    // names is condensed to a single space (matching `metadata['author']`).
    let a = parse_authors("Stuart       Rackham     <founder@asciidoc.org>");
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, "Stuart Rackham");
    assert_eq!(a[0].firstname, "Stuart");
    assert_eq!(a[0].lastname.as_deref(), Some("Rackham"));
    assert_eq!(a[0].email.as_deref(), Some("founder@asciidoc.org"));
    assert_eq!(a[0].initials, "SR");
}

#[test]
fn parse_invalid_author_line_becomes_author() {
    verifies!(
        r#"
  test "parse invalid author line becomes author" do
    metadata, _ = parse_header_metadata '   Stuart       Rackham, founder of AsciiDoc   <founder@asciidoc.org>'
    assert_equal 5, metadata.size
    assert_equal 1, metadata['authorcount']
    assert_equal 'Stuart Rackham, founder of AsciiDoc <founder@asciidoc.org>', metadata['author']
    assert_equal metadata['author'], metadata['authors']
    assert_equal 'Stuart Rackham, founder of AsciiDoc <founder@asciidoc.org>', metadata['firstname']
    assert_equal 'S', metadata['authorinitials']
  end

"#
    );

    // The author line doesn't match the author pattern (because of the embedded
    // comma), so Asciidoctor condenses the interior whitespace and stores the whole
    // line verbatim as the author, keeping the angle brackets literal.
    let a = parse_authors("   Stuart       Rackham, founder of AsciiDoc   <founder@asciidoc.org>");
    assert_eq!(a.len(), 1);
    assert_eq!(
        a[0].name,
        "Stuart Rackham, founder of AsciiDoc <founder@asciidoc.org>"
    );
    assert_eq!(
        a[0].firstname,
        "Stuart Rackham, founder of AsciiDoc <founder@asciidoc.org>"
    );
    assert_eq!(a[0].middlename, None);
    assert_eq!(a[0].lastname, None);
    assert_eq!(a[0].email, None);
    assert_eq!(a[0].initials, "S");
}

#[test]
fn parse_multiple_authors() {
    verifies!(
        r#"
  test 'parse multiple authors' do
    metadata, _ = parse_header_metadata 'Doc Writer <doc.writer@asciidoc.org>; John Smith <john.smith@asciidoc.org>'
    assert_equal 2, metadata['authorcount']
    assert_equal 'Doc Writer, John Smith', metadata['authors']
    assert_equal 'Doc Writer', metadata['author']
    assert_equal 'Doc Writer', metadata['author_1']
    assert_equal 'John Smith', metadata['author_2']
  end

"#
    );

    // This crate does not derive the `authors` (comma-joined) or `authorcount`
    // attributes and numbers the first author unsuffixed, so we assert on the
    // parsed author list directly.
    let a =
        parse_authors("Doc Writer <doc.writer@asciidoc.org>; John Smith <john.smith@asciidoc.org>");
    assert_eq!(a.len(), 2);
    assert_eq!(a[0].name, "Doc Writer");
    assert_eq!(a[0].email.as_deref(), Some("doc.writer@asciidoc.org"));
    assert_eq!(a[1].name, "John Smith");
    assert_eq!(a[1].email.as_deref(), Some("john.smith@asciidoc.org"));
}

#[test]
fn should_not_parse_multiple_authors_if_semi_colon_is_not_followed_by_space() {
    verifies!(
        r#"
  test 'should not parse multiple authors if semi-colon is not followed by space' do
    metadata, _ = parse_header_metadata 'Joe Doe;Smith Johnson'
    assert_equal 1, metadata['authorcount']
  end

"#
    );

    // Authors are split on `"; "` (semicolon + space) only, so this is one
    // author.
    let a = parse_authors("Joe Doe;Smith Johnson");
    assert_eq!(a.len(), 1);
}

// Incompatibility (https://github.com/asciidoc-rs/asciidoc-parser/issues/757):
// Asciidoctor splits the implicit author line on `;` and trims each entry, so
// the trailing bare `;` and the blank middle entry are dropped, yielding
// `John Smith` as the second author. This crate splits on `"; "` only, so the
// trailing `;` stays attached to the final entry (breaking its email match and
// HTML-encoding the angle brackets). The blank middle entry is dropped as
// expected.
non_normative!(
    r#"
  test 'skips blank author entries in implicit author line' do
    metadata, _ = parse_header_metadata 'Doc Writer; ; John Smith <john.smith@asciidoc.org>;'
    assert_equal 2, metadata['authorcount']
    assert_equal 'Doc Writer', metadata['author_1']
    assert_equal 'John Smith', metadata['author_2']
  end

"#
);

// Incompatibility (https://github.com/asciidoc-rs/asciidoc-parser/issues/758):
// the `:author:` attribute-entry path in this crate does not partition a
// 4+-part name (it stores the whole value as `firstname` without condensing
// whitespace), whereas Asciidoctor assigns the trailing parts to `lastname`.
non_normative!(
    r#"
  test 'parse name with more than 3 parts in author attribute' do
    doc = empty_document
    parse_header_metadata ':author: Leroy  Harold  Scherer,  Jr.', doc
    assert_equal 'Leroy Harold Scherer, Jr.', doc.attributes['author']
    assert_equal 'Leroy', doc.attributes['firstname']
    assert_equal 'Harold', doc.attributes['middlename']
    assert_equal 'Scherer, Jr.', doc.attributes['lastname']
  end

"#
);

#[test]
fn use_explicit_authorinitials_if_set_after_implicit_author_line() {
    verifies!(
        r#"
  test 'use explicit authorinitials if set after implicit author line' do
    input = <<~'EOS'
    Jean-Claude Van Damme
    :authorinitials: JCVD
    EOS
    doc = empty_document
    parse_header_metadata input, doc
    assert_equal 'JCVD', doc.attributes['authorinitials']
  end

"#
    );

    // This crate recognizes an author line only when a document title is
    // present, so the input is given a synthetic title.
    let mut parser = Parser::default();
    parser.parse("= Document Title\nJean-Claude Van Damme\n:authorinitials: JCVD");
    assert_eq!(
        parser.attribute_value("authorinitials"),
        InterpretedValue::Value("JCVD")
    );
}

#[test]
fn use_explicit_authorinitials_if_set_after_author_attribute() {
    verifies!(
        r#"
  test 'use explicit authorinitials if set after author attribute' do
    input = <<~'EOS'
    :author: Jean-Claude Van Damme
    :authorinitials: JCVD
    EOS
    doc = empty_document
    parse_header_metadata input, doc
    assert_equal 'JCVD', doc.attributes['authorinitials']
  end

"#
    );

    let mut parser = Parser::default();
    parser.parse("= Document Title\n:author: Jean-Claude Van Damme\n:authorinitials: JCVD");
    assert_eq!(
        parser.attribute_value("authorinitials"),
        InterpretedValue::Value("JCVD")
    );
}

// Incompatibility: this crate does not implement Asciidoctor's reconciliation
// of an explicit `:authors:` entry against the computed value; it simply stores
// the `:authors:` entry verbatim and never re-derives `author_1`/`authorcount`
// from it.
non_normative!(
    r#"
  test 'use implicit authors if value of authors attribute matches computed value' do
    input = <<~'EOS'
    Doc Writer; Junior Writer
    :authors: Doc Writer, Junior Writer
    EOS
    doc = empty_document
    parse_header_metadata input, doc
    assert_equal 'Doc Writer, Junior Writer', doc.attributes['authors']
    assert_equal 'Doc Writer', doc.attributes['author_1']
    assert_equal 'Junior Writer', doc.attributes['author_2']
  end

  test 'replace implicit authors if value of authors attribute does not match computed value' do
    input = <<~'EOS'
    Doc Writer; Junior Writer
    :authors: Stuart Rackham; Dan Allen; Sarah White
    EOS
    doc = empty_document
    metadata, _ = parse_header_metadata input, doc
    assert_equal 3, metadata['authorcount']
    assert_equal 3, doc.attributes['authorcount']
    assert_equal 'Stuart Rackham, Dan Allen, Sarah White', doc.attributes['authors']
    assert_equal 'Stuart Rackham', doc.attributes['author_1']
    assert_equal 'Dan Allen', doc.attributes['author_2']
    assert_equal 'Sarah White', doc.attributes['author_3']
  end

"#
);

// This crate does not derive an `authorcount` document attribute.
non_normative!(
    r#"
  test 'sets authorcount to 0 if document has no authors' do
    input = ''
    doc = empty_document
    metadata, _ = parse_header_metadata input, doc
    assert_equal 0, doc.attributes['authorcount']
    assert_equal 0, metadata['authorcount']
  end

  test 'returns empty hash if document has no authors and invoked without document' do
    metadata, _ = parse_header_metadata ''
    assert_empty metadata
  end

"#
);

#[test]
fn does_not_drop_name_joiner_when_using_multiple_authors() {
    verifies!(
        r#"
  test 'does not drop name joiner when using multiple authors' do
    input = 'Kismet Chameleon; Lazarus het_Draeke'
    doc = empty_document
    parse_header_metadata input, doc
    assert_equal 2, doc.attributes['authorcount']
    assert_equal 'Kismet Chameleon, Lazarus het Draeke', doc.attributes['authors']
    assert_equal 'Kismet Chameleon', doc.attributes['author_1']
    assert_equal 'Lazarus het Draeke', doc.attributes['author_2']
    assert_equal 'het Draeke', doc.attributes['lastname_2']
  end

"#
    );

    // The `het_Draeke` name-joiner underscore becomes a space (`het Draeke`)
    // rather than being dropped.
    let a = parse_authors("Kismet Chameleon; Lazarus het_Draeke");
    assert_eq!(a.len(), 2);
    assert_eq!(a[0].name, "Kismet Chameleon");
    assert_eq!(a[1].name, "Lazarus het Draeke");
    assert_eq!(a[1].lastname.as_deref(), Some("het Draeke"));
}

#[test]
fn allows_authors_to_be_overridden_using_explicit_author_attributes() {
    verifies!(
        r#"
  test 'allows authors to be overridden using explicit author attributes' do
    input = <<~'EOS'
    Kismet Chameleon; Johnny Bravo; Lazarus het_Draeke
    :author_2: Danger Mouse
    EOS
    doc = empty_document
    parse_header_metadata input, doc
    assert_equal 3, doc.attributes['authorcount']
    assert_equal 'Kismet Chameleon, Danger Mouse, Lazarus het Draeke', doc.attributes['authors']
    assert_equal 'Kismet Chameleon', doc.attributes['author_1']
    assert_equal 'Danger Mouse', doc.attributes['author_2']
    assert_equal 'Lazarus het Draeke', doc.attributes['author_3']
    assert_equal 'het Draeke', doc.attributes['lastname_3']
  end

"#
    );

    // The `:author_2:` entry overrides the second author computed from the
    // implicit author line. (This crate names the first author unsuffixed
    // `author` rather than `author_1`, and does not derive `authorcount` /
    // `authors`.)
    let mut parser = Parser::default();
    parser.parse(
        "= Document Title\nKismet Chameleon; Johnny Bravo; Lazarus het_Draeke\n:author_2: Danger Mouse",
    );
    assert_eq!(
        parser.attribute_value("author"),
        InterpretedValue::Value("Kismet Chameleon")
    );
    assert_eq!(
        parser.attribute_value("author_2"),
        InterpretedValue::Value("Danger Mouse")
    );
    assert_eq!(
        parser.attribute_value("author_3"),
        InterpretedValue::Value("Lazarus het Draeke")
    );
    assert_eq!(
        parser.attribute_value("lastname_3"),
        InterpretedValue::Value("het Draeke")
    );
}

// Incompatibility: this crate does not strip inline formatting (here a
// `pass:n[...]` macro) from an `:author:` attribute value before partitioning
// it into name parts, so the derived `authors`/`firstname`/`lastname` differ.
non_normative!(
    r#"
  test 'removes formatting before partitioning author defined using author attribute' do
    input = ':author: pass:n[http://example.org/community/team.html[Ze_**Project** team]]'

    doc = empty_document
    parse_header_metadata input, doc
    assert_equal 1, doc.attributes['authorcount']
    assert_equal '<a href="http://example.org/community/team.html">Ze <strong>Project</strong> team</a>', doc.attributes['authors']
    assert_equal 'Ze Project', doc.attributes['firstname']
    assert_equal 'team', doc.attributes['lastname']
  end

"#
);

#[test]
fn parse_rev_number_date_remark() {
    verifies!(
        r#"
  test "parse rev number date remark" do
    input = <<~'EOS'
    Ryan Waldron
    v0.0.7, 2013-12-18: The first release you can stand on
    EOS
    metadata, _ = parse_header_metadata input
    assert_equal 9, metadata.size
    assert_equal '0.0.7', metadata['revnumber']
    assert_equal '2013-12-18', metadata['revdate']
    assert_equal 'The first release you can stand on', metadata['revremark']
  end

"#
    );

    let (revnumber, revdate, revremark) =
        header_rev("Ryan Waldron\nv0.0.7, 2013-12-18: The first release you can stand on");
    assert_eq!(revnumber.as_deref(), Some("0.0.7"));
    assert_eq!(revdate, "2013-12-18");
    assert_eq!(
        revremark.as_deref(),
        Some("The first release you can stand on")
    );
}

#[test]
fn parse_rev_number_date_and_remark_as_attribute_references() {
    verifies!(
        r#"
  test 'parse rev number, data, and remark as attribute references' do
    input = <<~'EOS'
    Author Name
    v{project-version}, {release-date}: {release-summary}
    EOS
    metadata, _ = parse_header_metadata input
    assert_equal 9, metadata.size
    assert_equal '{project-version}', metadata['revnumber']
    assert_equal '{release-date}', metadata['revdate']
    assert_equal '{release-summary}', metadata['revremark']
  end

"#
    );

    // With none of the referenced attributes defined, the references survive
    // verbatim: the `v` prefix is stripped from the revision number, but
    // `{project-version}` is preserved rather than dropped for lack of a literal
    // digit. (This crate does not surface the full header-metadata hash, so the
    // `metadata.size` assertion is not modeled.)
    let (revnumber, revdate, revremark) =
        header_rev("Author Name\nv{project-version}, {release-date}: {release-summary}");
    assert_eq!(revnumber.as_deref(), Some("{project-version}"));
    assert_eq!(revdate, "{release-date}");
    assert_eq!(revremark.as_deref(), Some("{release-summary}"));
}

#[test]
fn should_resolve_attribute_references_in_rev_number_date_and_remark() {
    verifies!(
        r#"
  test 'should resolve attribute references in rev number, data, and remark' do
    input = <<~'EOS'
    = Document Title
    Author Name
    {project-version}, {release-date}: {release-summary}
    EOS
    doc = document_from_string input, attributes: {
      'project-version' => '1.0.1',
      'release-date' => '2018-05-15',
      'release-summary' => 'The one you can count on!',
    }
    assert_equal '1.0.1', (doc.attr 'revnumber')
    assert_equal '2018-05-15', (doc.attr 'revdate')
    assert_equal 'The one you can count on!', (doc.attr 'revremark')
  end

"#
    );

    // The referenced attributes are resolved before the built-in revision
    // document attributes are recorded, so `doc.attr` holds the substituted
    // values rather than the raw `{...}` references.
    let mut parser = Parser::default()
        .with_intrinsic_attribute("project-version", "1.0.1", ModificationContext::Anywhere)
        .with_intrinsic_attribute("release-date", "2018-05-15", ModificationContext::Anywhere)
        .with_intrinsic_attribute(
            "release-summary",
            "The one you can count on!",
            ModificationContext::Anywhere,
        );

    let _doc = parser.parse(
        "= Document Title\nAuthor Name\n{project-version}, {release-date}: {release-summary}",
    );

    assert_eq!(
        parser.attribute_value("revnumber").as_maybe_str(),
        Some("1.0.1")
    );
    assert_eq!(
        parser.attribute_value("revdate").as_maybe_str(),
        Some("2018-05-15")
    );
    assert_eq!(
        parser.attribute_value("revremark").as_maybe_str(),
        Some("The one you can count on!")
    );
}

#[test]
fn parse_rev_date() {
    verifies!(
        r#"
  test "parse rev date" do
    input = <<~'EOS'
    Ryan Waldron
    2013-12-18
    EOS
    metadata, _ = parse_header_metadata input
    assert_equal 7, metadata.size
    assert_equal '2013-12-18', metadata['revdate']
  end

"#
    );

    let (revnumber, revdate, revremark) = header_rev("Ryan Waldron\n2013-12-18");
    assert_eq!(revnumber, None);
    assert_eq!(revdate, "2013-12-18");
    assert_eq!(revremark, None);
}

#[test]
fn parse_rev_number_with_trailing_comma() {
    verifies!(
        r#"
  test 'parse rev number with trailing comma' do
    input = <<~'EOS'
    Stuart Rackham
    v8.6.8,
    EOS
    metadata, _ = parse_header_metadata input
    assert_equal 7, metadata.size
    assert_equal '8.6.8', metadata['revnumber']
    refute metadata.key?('revdate')
  end

"#
    );

    let (revnumber, revdate, revremark) = header_rev("Stuart Rackham\nv8.6.8,");
    assert_eq!(revnumber.as_deref(), Some("8.6.8"));
    // No date follows the trailing comma.
    assert_eq!(revdate, "");
    assert_eq!(revremark, None);
}

#[test]
fn parse_rev_number() {
    verifies!(
        r#"
  # Asciidoctor recognizes a standalone revision without a trailing comma
  test 'parse rev number' do
    input = <<~'EOS'
    Stuart Rackham
    v8.6.8
    EOS
    metadata, _ = parse_header_metadata input
    assert_equal 7, metadata.size
    assert_equal '8.6.8', metadata['revnumber']
    refute metadata.key?('revdate')
  end

"#
    );

    let (revnumber, revdate, revremark) = header_rev("Stuart Rackham\nv8.6.8");
    assert_eq!(revnumber.as_deref(), Some("8.6.8"));
    assert_eq!(revdate, "");
    assert_eq!(revremark, None);
}

#[test]
fn treats_arbitrary_text_on_rev_line_as_revdate() {
    verifies!(
        r#"
  # while compliant w/ AsciiDoc, this is just sloppy parsing
  test "treats arbitrary text on rev line as revdate" do
    input = <<~'EOS'
    Ryan Waldron
    foobar
    EOS
    metadata, _ = parse_header_metadata input
    assert_equal 7, metadata.size
    assert_equal 'foobar', metadata['revdate']
  end

"#
    );

    let (revnumber, revdate, revremark) = header_rev("Ryan Waldron\nfoobar");
    assert_eq!(revnumber, None);
    assert_eq!(revdate, "foobar");
    assert_eq!(revremark, None);
}

#[test]
fn parse_rev_date_remark() {
    verifies!(
        r#"
  test "parse rev date remark" do
    input = <<~'EOS'
    Ryan Waldron
    2013-12-18:  The first release you can stand on
    EOS
    metadata, _ = parse_header_metadata input
    assert_equal 8, metadata.size
    assert_equal '2013-12-18', metadata['revdate']
    assert_equal 'The first release you can stand on', metadata['revremark']
  end

"#
    );

    let (revnumber, revdate, revremark) =
        header_rev("Ryan Waldron\n2013-12-18:  The first release you can stand on");
    assert_eq!(revnumber, None);
    assert_eq!(revdate, "2013-12-18");
    assert_eq!(
        revremark.as_deref(),
        Some("The first release you can stand on")
    );
}

#[test]
fn should_not_mistake_attribute_entry_as_rev_remark() {
    verifies!(
        r#"
  test "should not mistake attribute entry as rev remark" do
    input = <<~'EOS'
    Joe Cool
    :page-layout: post
    EOS
    metadata, _ = parse_header_metadata input
    refute_equal 'page-layout: post', metadata['revremark']
    refute metadata.key?('revdate')
  end

"#
    );

    // The `:page-layout:` line after the author is an attribute entry, not a
    // revision line, so no revision line is parsed.
    let mut parser = Parser::default();
    let doc = parser.parse("= Document Title\nJoe Cool\n:page-layout: post");
    assert!(doc.header().revision_line().is_none());
    assert_eq!(parser.attribute_value("revremark"), InterpretedValue::Unset);
}

#[test]
fn parse_rev_remark_only() {
    verifies!(
        r#"
  test "parse rev remark only" do
    # NOTE cannot use single-quoted heredoc because of https://github.com/jruby/jruby/issues/4260
    input = <<~EOS
    Joe Cool
     :Must start revremark-only line with space
    EOS
    metadata, _ = parse_header_metadata input
    assert_equal 'Must start revremark-only line with space', metadata['revremark']
    refute metadata.key?('revdate')
  end

"#
    );

    let mut parser = Parser::default();
    let doc =
        parser.parse("= Document Title\nJoe Cool\n :Must start revremark-only line with space");
    let rev = doc
        .header()
        .revision_line()
        .expect("expected a revision line");
    assert_eq!(
        rev.revremark(),
        Some("Must start revremark-only line with space")
    );
    assert_eq!(rev.revnumber(), None);
}

#[test]
fn skip_line_comments_before_author() {
    verifies!(
        r#"
  test "skip line comments before author" do
    input = <<~'EOS'
    // Asciidoctor
    // release artist
    Ryan Waldron
    EOS
    metadata, _ = parse_header_metadata input
    assert_equal 6, metadata.size
    assert_equal 1, metadata['authorcount']
    assert_equal 'Ryan Waldron', metadata['author']
    assert_equal 'Ryan', metadata['firstname']
    assert_equal 'Waldron', metadata['lastname']
    assert_equal 'RW', metadata['authorinitials']
  end

"#
    );

    // Line comments preceding the author line are skipped (synthetic title
    // added so the author line is recognized).
    let mut parser = Parser::default();
    parser.parse("= Document Title\n// Asciidoctor\n// release artist\nRyan Waldron");
    assert_eq!(
        parser.attribute_value("author"),
        InterpretedValue::Value("Ryan Waldron")
    );
    assert_eq!(
        parser.attribute_value("firstname"),
        InterpretedValue::Value("Ryan")
    );
    assert_eq!(
        parser.attribute_value("lastname"),
        InterpretedValue::Value("Waldron")
    );
    assert_eq!(
        parser.attribute_value("authorinitials"),
        InterpretedValue::Value("RW")
    );
}

#[test]
fn skip_block_comment_before_author() {
    verifies!(
        r#"
  test "skip block comment before author" do
    input = <<~'EOS'
    ////
    Asciidoctor
    release artist
    ////
    Ryan Waldron
    EOS
    metadata, _ = parse_header_metadata input
    assert_equal 6, metadata.size
    assert_equal 1, metadata['authorcount']
    assert_equal 'Ryan Waldron', metadata['author']
    assert_equal 'Ryan', metadata['firstname']
    assert_equal 'Waldron', metadata['lastname']
    assert_equal 'RW', metadata['authorinitials']
  end

"#
    );

    // A `////` block comment preceding the author line is skipped in its
    // entirety (synthetic title added so the author line is recognized).
    let mut parser = Parser::default();
    parser.parse("= Document Title\n////\nAsciidoctor\nrelease artist\n////\nRyan Waldron");
    assert_eq!(
        parser.attribute_value("author"),
        InterpretedValue::Value("Ryan Waldron")
    );
    assert_eq!(
        parser.attribute_value("firstname"),
        InterpretedValue::Value("Ryan")
    );
    assert_eq!(
        parser.attribute_value("lastname"),
        InterpretedValue::Value("Waldron")
    );
    assert_eq!(
        parser.attribute_value("authorinitials"),
        InterpretedValue::Value("RW")
    );
}

#[test]
fn skip_block_comment_before_rev() {
    verifies!(
        r#"
  test "skip block comment before rev" do
    input = <<~'EOS'
    Ryan Waldron
    ////
    Asciidoctor
    release info
    ////
    v0.0.7, 2013-12-18
    EOS
    metadata, _ = parse_header_metadata input
    assert_equal 8, metadata.size
    assert_equal 1, metadata['authorcount']
    assert_equal 'Ryan Waldron', metadata['author']
    assert_equal '0.0.7', metadata['revnumber']
    assert_equal '2013-12-18', metadata['revdate']
  end

"#
    );

    // A `////` block comment between the author line and the revision line is
    // skipped, so the revision line that follows it is parsed (synthetic title
    // added so the author line is recognized).
    let mut parser = Parser::default();
    let doc = parser.parse(
        "= Document Title\nRyan Waldron\n////\nAsciidoctor\nrelease info\n////\nv0.0.7, 2013-12-18",
    );
    assert_eq!(
        parser.attribute_value("author"),
        InterpretedValue::Value("Ryan Waldron")
    );

    let rev = doc
        .header()
        .revision_line()
        .expect("expected a revision line");
    assert_eq!(rev.revnumber(), Some("0.0.7"));
    assert_eq!(rev.revdate(), "2013-12-18");
}

#[test]
fn break_header_at_line_with_three_forward_slashes() {
    verifies!(
        r#"
  test 'break header at line with three forward slashes' do
    input = <<~'EOS'
    Joe Cool
    v1.0
    ///
    stuff
    EOS
    metadata, _ = parse_header_metadata input
    assert_equal 7, metadata.size
    assert_equal 1, metadata['authorcount']
    assert_equal 'Joe Cool', metadata['author']
    assert_equal '1.0', metadata['revnumber']
  end

"#
    );

    // A `///` line breaks the header; the author and revision number before it
    // are captured (synthetic title added).
    let mut parser = Parser::default();
    parser.parse("= Document Title\nJoe Cool\nv1.0\n///\nstuff");
    assert_eq!(
        parser.attribute_value("author"),
        InterpretedValue::Value("Joe Cool")
    );
    assert_eq!(
        parser.attribute_value("revnumber"),
        InterpretedValue::Value("1.0")
    );
}

// This crate has no separate `metadata` hash: the author line's generated
// initials and a later `:Author Initials:` override both write the one
// `authorinitials` document attribute. So the generated value (`SR`) is
// asserted on the parsed author, and the override (`SJR`) on the resolved
// attribute.
#[test]
fn attribute_entry_overrides_generated_author_initials() {
    verifies!(
        r#"
  test 'attribute entry overrides generated author initials' do
    doc = empty_document
    metadata, _ = parse_header_metadata %(Stuart Rackham <founder@asciidoc.org>\n:Author Initials: SJR), doc
    assert_equal 'SR', metadata['authorinitials']
    assert_equal 'SJR', doc.attributes['authorinitials']
  end

"#
    );

    // The author line on its own generates the initials `SR`.
    let a = parse_authors("Stuart Rackham <founder@asciidoc.org>");
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].initials, "SR");

    // A `:Author Initials: SJR` entry below the author line is sanitized to the
    // `authorinitials` attribute name and overrides the generated value. (A
    // synthetic title is required because this crate only recognizes an author
    // line when a document title is present.)
    let mut parser = Parser::default();
    let _doc = parser
        .parse("= Document Title\nStuart Rackham <founder@asciidoc.org>\n:Author Initials: SJR");
    assert_eq!(
        parser.attribute_value("authorinitials"),
        InterpretedValue::Value("SJR")
    );
}

// The `adjust_indentation!` group tests an internal static helper (with
// explicit indent/tab-size arguments) that this crate does not expose. Verbatim
// indentation handling for verbatim blocks is covered by the block suites.
non_normative!(
    r#"
  test 'adjust indentation to 0' do
    input = <<~EOS
    \x20   def names

    \x20     @name.split

    \x20   end
    EOS

    # NOTE cannot use single-quoted heredoc because of https://github.com/jruby/jruby/issues/4260
    expected = <<~EOS.chop
    def names

      @name.split

    end
    EOS

    lines = input.split ?\n
    Asciidoctor::Parser.adjust_indentation! lines
    assert_equal expected, (lines * ?\n)
  end

  test 'adjust indentation mixed with tabs and spaces to 0' do
    input = <<~EOS
        def names

    \t  @name.split

        end
    EOS

    expected = <<~EOS.chop
    def names

      @name.split

    end
    EOS

    lines = input.split ?\n
    Asciidoctor::Parser.adjust_indentation! lines, 0, 4
    assert_equal expected, (lines * ?\n)
  end

  test 'expands tabs to spaces' do
    input = <<~'EOS'
    Filesystem				Size	Used	Avail	Use%	Mounted on
    Filesystem              Size    Used    Avail   Use%    Mounted on
    devtmpfs				3.9G	   0	 3.9G	  0%	/dev
    /dev/mapper/fedora-root	 48G	 18G	  29G	 39%	/
    EOS

    expected = <<~'EOS'.chop
    Filesystem              Size    Used    Avail   Use%    Mounted on
    Filesystem              Size    Used    Avail   Use%    Mounted on
    devtmpfs                3.9G       0     3.9G     0%    /dev
    /dev/mapper/fedora-root  48G     18G      29G    39%    /
    EOS

    lines = input.split ?\n
    Asciidoctor::Parser.adjust_indentation! lines, 0, 4
    assert_equal expected, (lines * ?\n)
  end

  test 'adjust indentation to non-zero' do
    input = <<~EOS
    \x20   def names

    \x20     @name.split

    \x20   end
    EOS

    expected = <<~EOS.chop
    \x20 def names

    \x20   @name.split

    \x20 end
    EOS

    lines = input.split ?\n
    Asciidoctor::Parser.adjust_indentation! lines, 2
    assert_equal expected, (lines * ?\n)
  end

  test 'preserve block indent if indent is -1' do
    input = <<~EOS
    \x20   def names

    \x20     @name.split

    \x20   end
    EOS

    expected = input

    lines = input.lines
    Asciidoctor::Parser.adjust_indentation! lines, -1
    assert_equal expected, lines.join
  end

  test 'adjust indentation handles empty lines gracefully' do
    input = []
    expected = input

    lines = input.dup
    Asciidoctor::Parser.adjust_indentation! lines
    assert_equal expected, lines
  end

"#
);

// Incompatibility (https://github.com/asciidoc-rs/asciidoc-parser/issues/762):
// a duplicate id introduced by an *inline* anchor (`[[in-use]]` in paragraph
// content) does not emit a `DuplicateId` warning — this crate discards the
// duplicate-id error on the inline-anchor substitution path (a duplicate
// *block* anchor does warn; see the `lists` and `blocks` suites).
non_normative!(
    r#"
  test 'should warn if inline anchor is already in use' do
    input = <<~'EOS'
    [#in-use]
    A paragraph with an id.

    Another paragraph
    [[in-use]]that uses an id
    which is already in use.
    EOS

    using_memory_logger do |logger|
      document_from_string input
      assert_message logger, :WARN, '<stdin>: line 5: id assigned to anchor already in use: in-use', Hash
    end
  end
"#
);

non_normative!(
    r#"
end
"#
);
