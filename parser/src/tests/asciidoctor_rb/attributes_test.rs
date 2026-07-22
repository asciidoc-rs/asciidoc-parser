// Adapted from Asciidoctor's attributes test suite, found in
// https://github.com/asciidoctor/asciidoctor/blob/main/test/attributes_test.rb.
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

//! Port of Asciidoctor's `attributes_test.rb`.
//!
//! Each Ruby `test '..'` becomes a `#[test] fn`, driven through
//! [`Parser`](crate::Parser) and asserted with the `assert_*` DOM helpers or
//! the document/block attribute accessors. `document_from_string(input,
//! attributes: {..})` maps to `Parser::default().with_intrinsic_attribute(..)`
//! (the Ruby `@` soft-set / `!` unset modifiers map to
//! [`ModificationContext`](crate::parser::ModificationContext) variants and the
//! `_bool` setters), and `safe:` maps to
//! [`with_safe_mode`](crate::Parser::with_safe_mode).
//!
//! Tests that exercise Ruby-only surface are reproduced verbatim in
//! `non_normative!` blocks (no `#[test]`), each tagged with a one-line reason:
//! the mutable node/document attribute API (`Block.new` / `set_attr` /
//! `remove_attr` / `set_attribute` / `role=` / `add_role` / `remove_role`), the
//! DocBook backend, and the `Asciidoctor::INTRINSIC_ATTRIBUTES` constant
//! enumeration.
//!
//! A number of tests surface behavior differences from Asciidoctor. These keep
//! their `#[test]` but move the Ruby source into a `non_normative!` block and
//! assert this crate's *actual* output, each annotated inline.
//!
//! Two of these are **intentional** divergences this crate does not plan to
//! match:
//!
//! * The `{set:..}` inline attribute assignment/unassignment reference is not
//!   processed (left literal).
//! * Compat mode does not change monospaced-span substitution (`+..+` /
//!   backtick) or the legacy syntax it enables.
//!
//! The remainder are tracked as issues (see the `sddbugfind` label). Each such
//! test carries an inline `Divergence:` note; the notable ones are:
//!
//! * Attribute *references* are case-sensitive, so `{He-Man}` does not resolve
//!   the lowercased `:He-Man:` entry.
//! * Multi-line attribute values fuse only the modern backslash continuation,
//!   not the legacy `+`, so a legacy `+`-continued value keeps only its first
//!   line.
//! * Non-ASCII attribute names and names with spaces are not accepted.
//! * A counter modifies a locked (API-set or built-in) attribute rather than
//!   leaving it unchanged.
//! * Safe-mode masking of `docdir`/`docfile`, the unterminated-header-comment
//!   behavior, and the transfer of trailing anchors/attributes onto a following
//!   section all differ.
//!
//! Separately, block captions/titles are exposed via `caption()` / `title()`
//! rather than as `<div class="title">` DOM nodes — a documented architectural
//! choice (this crate does not render whole blocks to HTML), not a divergence.

use crate::tests::sdd::*;

track_file!("ref/asciidoctor/test/attributes_test.rb");

non_normative!(
    r#"
# frozen_string_literal: true
require_relative 'test_helper'

context 'Attributes' do
  default_logger = Asciidoctor::LoggerManager.logger

  setup do
    Asciidoctor::LoggerManager.logger = (@logger = Asciidoctor::MemoryLogger.new)
  end

  teardown do
    Asciidoctor::LoggerManager.logger = default_logger
  end

  context 'Assignment' do
"#
);
mod assignment {
    use crate::tests::prelude::*;

    #[test]
    fn creates_an_attribute() {
        verifies!(
            r#"
    test 'creates an attribute' do
      doc = document_from_string(':frog: Tanglefoot')
      assert_equal 'Tanglefoot', doc.attributes['frog']
    end

"#
        );

        let doc = Parser::default().parse(":frog: Tanglefoot");
        assert_eq!(
            doc.attribute_value("frog"),
            InterpretedValue::Value("Tanglefoot")
        );
    }

    #[test]
    fn requires_a_space_after_colon_following_attribute_name() {
        verifies!(
            r#"
    test 'requires a space after colon following attribute name' do
      doc = document_from_string 'foo:bar'
      assert_nil doc.attributes['foo']
    end

"#
        );

        let doc = Parser::default().parse("foo:bar");
        assert_eq!(doc.attribute_value("foo"), InterpretedValue::Unset);
    }

    non_normative!(
        r#"
    # NOTE AsciiDoc.py recognizes this entry
"#
    );
    // Tracked in #728.
    #[test]
    fn does_not_recognize_attribute_entry_if_name_contains_colon() {
        non_normative!(
            r#"
    test 'does not recognize attribute entry if name contains colon' do
      input = ':foo:bar: baz'
      doc = document_from_string input
      refute doc.attr?('foo:bar')
      assert_equal 1, doc.blocks.size
      assert_equal :paragraph, doc.blocks[0].context
    end

"#
        );

        // Divergence: Asciidoctor renders `:foo:bar: baz` as a paragraph (a name
        // containing a colon isn't a valid attribute entry). This crate also
        // rejects `foo:bar` as an attribute, but consumes the line into the
        // document header, yielding no blocks — so the block count/context
        // assertions can't be verified against it.
        let doc = Parser::default().parse(":foo:bar: baz");
        assert!(!doc.has_attribute("foo:bar"));
        assert_eq!(doc.nested_blocks().count(), 0);
    }

    non_normative!(
        r#"
    # NOTE AsciiDoc.py recognizes this entry
"#
    );
    // Tracked in #728.
    #[test]
    fn does_not_recognize_attribute_entry_if_name_ends_with_colon() {
        non_normative!(
            r#"
    test 'does not recognize attribute entry if name ends with colon' do
      input = ':foo:: bar'
      doc = document_from_string input
      refute doc.attr?('foo:')
      assert_equal 1, doc.blocks.size
      assert_equal :dlist, doc.blocks[0].context
    end

"#
        );

        // Divergence: Asciidoctor renders `:foo:: bar` as a description list (a
        // name ending with a colon isn't a valid attribute entry). This crate
        // consumes the line into the document header, yielding no blocks.
        let doc = Parser::default().parse(":foo:: bar");
        assert!(!doc.has_attribute("foo:"));
        assert_eq!(doc.nested_blocks().count(), 0);
    }

    non_normative!(
        r#"
    # NOTE AsciiDoc.py does not recognize this entry
"#
    );
    // Tracked in #726.
    #[test]
    fn allows_any_word_character_defined_by_unicode_in_an_attribute_name() {
        non_normative!(
            r#"
    test 'allows any word character defined by Unicode in an attribute name' do
      [['café', 'a coffee shop'], ['سمن', %(سازمان مردمنهاد)]].each do |(name, value)|
        str = <<~EOS
        :#{name}: #{value}

        {#{name}}
        EOS
        result = convert_string_to_embedded str
        assert_includes result, %(<p>#{value}</p>)
      end
    end

"#
        );

        // Divergence: Asciidoctor accepts any Unicode word character in an
        // attribute name; this crate restricts names to ASCII word characters,
        // so `:café: …` / `:سمن: …` are treated as literal paragraphs and the
        // `{café}` / `{سمن}` references are left unresolved.
        for (name, value) in [("café", "a coffee shop"), ("سمن", "سازمان مردمنهاد")]
        {
            let _ = value;
            let doc = Parser::default().parse(&format!(":{name}: {value}\n\n{{{name}}}"));
            assert_xpath(&doc, &format!("//p[text()=\"{{{name}}}\"]"), 1);
        }
    }

    // Tracked in #729.
    #[test]
    fn creates_an_attribute_by_fusing_a_legacy_multi_line_value() {
        non_normative!(
            r#"
    test 'creates an attribute by fusing a legacy multi-line value' do
      str = <<~'EOS'
      :description: This is the first      +
                    Ruby implementation of +
                    AsciiDoc.
      EOS
      doc = document_from_string(str)
      assert_equal 'This is the first Ruby implementation of AsciiDoc.', doc.attributes['description']
    end

"#
        );

        // Divergence: Asciidoctor fuses a legacy `+`-terminated multi-line
        // attribute value; this crate only fuses the modern backslash
        // continuation, so the `+`-continued lines are not joined, and the
        // trailing `+` is treated as a hard-break marker and stripped, leaving
        // just the first line's text.
        let doc = Parser::default().parse(
            ":description: This is the first      +\n              Ruby implementation of +\n              AsciiDoc.",
        );
        assert_eq!(
            doc.attribute_value("description"),
            InterpretedValue::Value("This is the first")
        );
    }

    #[test]
    fn creates_an_attribute_by_fusing_a_multi_line_value() {
        verifies!(
            r#"
    test 'creates an attribute by fusing a multi-line value' do
      str = <<~'EOS'
      :description: This is the first \
                    Ruby implementation of \
                    AsciiDoc.
      EOS
      doc = document_from_string(str)
      assert_equal 'This is the first Ruby implementation of AsciiDoc.', doc.attributes['description']
    end

"#
        );

        let doc = Parser::default().parse(
            ":description: This is the first \\\n              Ruby implementation of \\\n              AsciiDoc.",
        );
        assert_eq!(
            doc.attribute_value("description"),
            InterpretedValue::Value("This is the first Ruby implementation of AsciiDoc.")
        );
    }

    #[test]
    fn honors_line_break_characters_in_multi_line_values() {
        verifies!(
            r#"
    test 'honors line break characters in multi-line values' do
      str = <<~'EOS'
      :signature: Linus Torvalds + \
      Linux Hacker + \
      linus.torvalds@example.com
      EOS
      doc = document_from_string(str)
      assert_equal %(Linus Torvalds +\nLinux Hacker +\nlinus.torvalds@example.com), doc.attributes['signature']
    end

"#
        );

        let doc = Parser::default().parse(
            ":signature: Linus Torvalds + \\\nLinux Hacker + \\\nlinus.torvalds@example.com",
        );
        assert_eq!(
            doc.attribute_value("signature"),
            InterpretedValue::Value("Linus Torvalds +\nLinux Hacker +\nlinus.torvalds@example.com")
        );
    }

    #[test]
    fn should_allow_pass_macro_to_surround_a_multi_line_value_that_contains_line_breaks() {
        verifies!(
            r#"
    test 'should allow pass macro to surround a multi-line value that contains line breaks' do
      str = <<~'EOS'
      :signature: pass:a[{author} + \
      {title} + \
      {email}]
      EOS
      doc = document_from_string str, attributes: { 'author' => 'Linus Torvalds', 'title' => 'Linux Hacker', 'email' => 'linus.torvalds@example.com' }
      assert_equal %(Linus Torvalds +\nLinux Hacker +\nlinus.torvalds@example.com), (doc.attr 'signature')
    end

"#
        );

        let doc = Parser::default()
            .with_intrinsic_attribute("author", "Linus Torvalds", ModificationContext::ApiOnly)
            .with_intrinsic_attribute("title", "Linux Hacker", ModificationContext::ApiOnly)
            .with_intrinsic_attribute(
                "email",
                "linus.torvalds@example.com",
                ModificationContext::ApiOnly,
            )
            .parse(":signature: pass:a[{author} + \\\n{title} + \\\n{email}]");
        assert_eq!(
            doc.attribute_value("signature"),
            InterpretedValue::Value("Linus Torvalds +\nLinux Hacker +\nlinus.torvalds@example.com")
        );
    }

    #[test]
    fn should_delete_an_attribute_that_ends_with() {
        verifies!(
            r#"
    test 'should delete an attribute that ends with !' do
      doc = document_from_string(":frog: Tanglefoot\n:frog!:")
      assert_nil doc.attributes['frog']
    end

"#
        );

        let doc = Parser::default().parse(":frog: Tanglefoot\n:frog!:");
        assert_eq!(doc.attribute_value("frog"), InterpretedValue::Unset);
    }

    #[test]
    fn should_delete_an_attribute_that_ends_with_set_via_api() {
        verifies!(
            r#"
    test 'should delete an attribute that ends with ! set via API' do
      doc = document_from_string(":frog: Tanglefoot", attributes: { 'frog!' => '' })
      assert_nil doc.attributes['frog']
    end

"#
        );

        let doc = Parser::default()
            .with_intrinsic_attribute_bool("frog", false, ModificationContext::ApiOnly)
            .parse(":frog: Tanglefoot");
        assert_eq!(doc.attribute_value("frog"), InterpretedValue::Unset);
    }

    #[test]
    fn should_delete_an_attribute_that_begins_with() {
        verifies!(
            r#"
    test 'should delete an attribute that begins with !' do
      doc = document_from_string(":frog: Tanglefoot\n:!frog:")
      assert_nil doc.attributes['frog']
    end

"#
        );

        let doc = Parser::default().parse(":frog: Tanglefoot\n:!frog:");
        assert_eq!(doc.attribute_value("frog"), InterpretedValue::Unset);
    }

    #[test]
    fn should_delete_an_attribute_that_begins_with_set_via_api() {
        verifies!(
            r#"
    test 'should delete an attribute that begins with ! set via API' do
      doc = document_from_string(":frog: Tanglefoot", attributes: { '!frog' => '' })
      assert_nil doc.attributes['frog']
    end

"#
        );

        let doc = Parser::default()
            .with_intrinsic_attribute_bool("frog", false, ModificationContext::ApiOnly)
            .parse(":frog: Tanglefoot");
        assert_eq!(doc.attribute_value("frog"), InterpretedValue::Unset);
    }

    #[test]
    fn should_delete_an_attribute_set_via_api_to_nil_value() {
        verifies!(
            r#"
    test 'should delete an attribute set via API to nil value' do
      doc = document_from_string(":frog: Tanglefoot", attributes: { 'frog' => nil })
      assert_nil doc.attributes['frog']
    end

"#
        );

        let doc = Parser::default()
            .with_intrinsic_attribute_bool("frog", false, ModificationContext::ApiOnly)
            .parse(":frog: Tanglefoot");
        assert_eq!(doc.attribute_value("frog"), InterpretedValue::Unset);
    }

    #[test]
    fn doesn_t_choke_when_deleting_a_non_existing_attribute() {
        verifies!(
            r#"
    test "doesn't choke when deleting a non-existing attribute" do
      doc = document_from_string(':frog!:')
      assert_nil doc.attributes['frog']
    end

"#
        );

        let doc = Parser::default().parse(":frog!:");
        assert_eq!(doc.attribute_value("frog"), InterpretedValue::Unset);
    }

    #[test]
    fn replaces_special_characters_in_attribute_value() {
        verifies!(
            r#"
    test "replaces special characters in attribute value" do
      doc = document_from_string(":xml-busters: <>&")
      assert_equal '&lt;&gt;&amp;', doc.attributes['xml-busters']
    end

"#
        );

        let doc = Parser::default().parse(":xml-busters: <>&");
        assert_eq!(
            doc.attribute_value("xml-busters"),
            InterpretedValue::Value("&lt;&gt;&amp;")
        );
    }

    #[test]
    fn performs_attribute_substitution_on_attribute_value() {
        verifies!(
            r#"
    test "performs attribute substitution on attribute value" do
      doc = document_from_string(":version: 1.0\n:release: Asciidoctor {version}")
      assert_equal 'Asciidoctor 1.0', doc.attributes['release']
    end

"#
        );

        let doc = Parser::default().parse(":version: 1.0\n:release: Asciidoctor {version}");
        assert_eq!(
            doc.attribute_value("release"),
            InterpretedValue::Value("Asciidoctor 1.0")
        );
    }

    #[test]
    fn assigns_attribute_to_empty_string_if_substitution_fails_to_resolve_attribute() {
        verifies!(
            r#"
    test 'assigns attribute to empty string if substitution fails to resolve attribute' do
      input = ':release: Asciidoctor {version}'
      document_from_string input, attributes: { 'attribute-missing' => 'drop-line' }
      assert_message @logger, :INFO, 'dropping line containing reference to missing attribute: version'
    end

"#
        );

        // This crate does not surface the INFO "dropping line…" log message, so
        // only the observable result (the missing reference resolves to empty)
        // is checked.
        let doc = Parser::default()
            .with_intrinsic_attribute(
                "attribute-missing",
                "drop-line",
                ModificationContext::ApiOnly,
            )
            .parse(":release: Asciidoctor {version}");
        assert_eq!(doc.attribute_value("release"), InterpretedValue::Value(""));
    }

    // Tracked in #729.
    #[test]
    fn assigns_multi_line_attribute_to_empty_string_if_substitution_fails_to_resolve_attribute() {
        non_normative!(
            r#"
    test 'assigns multi-line attribute to empty string if substitution fails to resolve attribute' do
      input = <<~'EOS'
      :release: Asciidoctor +
                {version}
      EOS
      doc = document_from_string input, attributes: { 'attribute-missing' => 'drop-line' }
      assert_equal '', doc.attributes['release']
      assert_message @logger, :INFO, 'dropping line containing reference to missing attribute: version'
    end

"#
        );

        // Divergence: this crate only fuses the modern backslash continuation,
        // not the legacy `+` continuation, so the `{version}` line never
        // participates in the header value; the trailing `+` is treated as a
        // hard-break marker and stripped, leaving just `Asciidoctor`. (The INFO
        // drop-line log is also not modeled.)
        let doc = Parser::default()
            .with_intrinsic_attribute(
                "attribute-missing",
                "drop-line",
                ModificationContext::ApiOnly,
            )
            .parse(":release: Asciidoctor +\n          {version}\n");
        assert_eq!(
            doc.attribute_value("release"),
            InterpretedValue::Value("Asciidoctor")
        );
    }

    #[test]
    fn resolves_attributes_inside_attribute_value_within_header() {
        verifies!(
            r#"
    test 'resolves attributes inside attribute value within header' do
      input = <<~'EOS'
      = Document Title
      :big: big
      :bigfoot: {big}foot

      {bigfoot}
      EOS

      result = convert_string_to_embedded input
      assert_includes result, 'bigfoot'
    end

"#
        );

        let doc = Parser::default()
            .parse("= Document Title\n:big: big\n:bigfoot: {big}foot\n\n{bigfoot}");
        assert_xpath(&doc, "//p[text()=\"bigfoot\"]", 1);
    }

    #[test]
    fn resolves_attributes_and_pass_macro_inside_attribute_value_outside_header() {
        verifies!(
            r#"
    test 'resolves attributes and pass macro inside attribute value outside header' do
      input = <<~'EOS'
      = Document Title

      content

      :big: pass:a,q[_big_]
      :bigfoot: {big}foot
      {bigfoot}
      EOS

      result = convert_string_to_embedded input
      assert_includes result, '<em>big</em>foot'
    end

"#
        );

        let doc = Parser::default().parse(
            "= Document Title\n\ncontent\n\n:big: pass:a,q[_big_]\n:bigfoot: {big}foot\n{bigfoot}",
        );
        assert_eq!(
            doc.attribute_value("bigfoot"),
            InterpretedValue::Value("<em>big</em>foot")
        );
        assert_xpath(&doc, "//em[text()=\"big\"]", 1);
    }

    #[test]
    fn should_limit_maximum_size_of_attribute_value_if_safe_mode_is_secure() {
        verifies!(
            r#"
    test 'should limit maximum size of attribute value if safe mode is SECURE' do
      expected = 'a' * 4096
      input = <<~EOS
      :name: #{'a' * 5000}

      {name}
      EOS

      result = convert_inline_string input
      assert_equal expected, result
      assert_equal 4096, result.bytesize
    end

"#
        );

        // `Parser::default()` runs under `SafeMode::Secure`, where
        // `max-attribute-value-size` defaults to 4096. The 5000-byte value is
        // truncated to 4096 bytes.
        let input = format!(":name: {}\n\n{{name}}", "a".repeat(5000));
        let doc = Parser::default().parse(&input);

        let value = doc.attribute_value("name");
        assert_eq!(value.as_maybe_str(), Some("a".repeat(4096).as_str()));
        assert_eq!(value.as_maybe_str().map(str::len), Some(4096));
    }

    #[test]
    fn should_handle_multibyte_characters_when_limiting_attribute_value_size() {
        verifies!(
            r#"
    test 'should handle multibyte characters when limiting attribute value size' do
      expected = '日本'
      input = <<~'EOS'
      :name: 日本語

      {name}
      EOS

      result = convert_inline_string input, attributes: { 'max-attribute-value-size' => 6 }
      assert_equal expected, result
      assert_equal 6, result.bytesize
    end

"#
        );

        // Each of these characters is 3 bytes, so the limit of 6 lands exactly
        // on a character boundary, keeping the first two characters.
        let doc = Parser::default()
            .with_intrinsic_attribute(
                "max-attribute-value-size",
                "6",
                ModificationContext::ApiOnly,
            )
            .parse(":name: 日本語\n\n{name}");

        let value = doc.attribute_value("name");
        assert_eq!(value.as_maybe_str(), Some("日本"));
        assert_eq!(value.as_maybe_str().map(str::len), Some(6));
    }

    #[test]
    fn should_not_mangle_multibyte_characters_when_limiting_attribute_value_size() {
        verifies!(
            r#"
    test 'should not mangle multibyte characters when limiting attribute value size' do
      expected = '日本'
      input = <<~'EOS'
      :name: 日本語

      {name}
      EOS

      result = convert_inline_string input, attributes: { 'max-attribute-value-size' => 8 }
      assert_equal expected, result
      assert_equal 6, result.bytesize
    end

"#
        );

        // The limit of 8 falls in the middle of the third (3-byte) character, so
        // that character is dropped whole rather than split: the truncated value
        // is 6 bytes, not 8.
        let doc = Parser::default()
            .with_intrinsic_attribute(
                "max-attribute-value-size",
                "8",
                ModificationContext::ApiOnly,
            )
            .parse(":name: 日本語\n\n{name}");

        let value = doc.attribute_value("name");
        assert_eq!(value.as_maybe_str(), Some("日本"));
        assert_eq!(value.as_maybe_str().map(str::len), Some(6));
    }

    #[test]
    fn should_allow_maximize_size_of_attribute_value_to_be_disabled() {
        verifies!(
            r#"
    test 'should allow maximize size of attribute value to be disabled' do
      expected = 'a' * 5000
      input = <<~EOS
      :name: #{'a' * 5000}

      {name}
      EOS

      result = convert_inline_string input, attributes: { 'max-attribute-value-size' => nil }
      assert_equal expected, result
      assert_equal 5000, result.bytesize
    end

"#
        );

        // Asciidoctor disables the limit with a `nil` value; this crate models a
        // disabled limit as a non-positive value (Ruby's `String#to_i` coerces
        // `nil`/empty to `0`), so `0` turns it off and the full 5000-byte value
        // is preserved.
        let input = format!(":name: {}\n\n{{name}}", "a".repeat(5000));
        let doc = Parser::default()
            .with_intrinsic_attribute(
                "max-attribute-value-size",
                "0",
                ModificationContext::ApiOnly,
            )
            .parse(&input);

        let value = doc.attribute_value("name");
        assert_eq!(value.as_maybe_str(), Some("a".repeat(5000).as_str()));
        assert_eq!(value.as_maybe_str().map(str::len), Some(5000));
    }

    #[test]
    fn resolves_user_home_attribute_if_safe_mode_is_less_than_server() {
        verifies!(
            r#"
    test 'resolves user-home attribute if safe mode is less than SERVER' do
      input = <<~'EOS'
      :imagesdir: {user-home}/etc/images

      {imagesdir}
      EOS
      output = convert_inline_string input, safe: :safe
      assert_equal %(#{Asciidoctor::USER_HOME}/etc/images), output
    end

"#
        );

        // Below `SafeMode::Server`, the built-in `user-home` intrinsic resolves
        // to the user's home directory, the counterpart of Ruby Asciidoctor's
        // `USER_HOME` (resolved from the same environment).
        let home = std::env::home_dir()
            .expect("home directory should resolve in the test environment")
            .to_string_lossy()
            .into_owned();
        let doc = Parser::default()
            .with_safe_mode(SafeMode::Safe)
            .parse(":imagesdir: {user-home}/etc/images\n\n{imagesdir}");
        assert_eq!(
            rendered_paragraphs(&doc),
            vec![format!("{home}/etc/images")]
        );

        // The intrinsic is also queryable on the completed document (mirroring
        // Ruby's `doc.attributes['user-home']`), not only during substitution.
        assert_eq!(
            doc.attribute_value("user-home").as_maybe_str(),
            Some(home.as_str())
        );
    }

    #[test]
    fn user_home_attribute_resolves_to_if_safe_mode_is_server_or_greater() {
        verifies!(
            r#"
    test 'user-home attribute resolves to . if safe mode is SERVER or greater' do
      input = <<~'EOS'
      :imagesdir: {user-home}/etc/images

      {imagesdir}
      EOS
      output = convert_inline_string input, safe: :server
      assert_equal './etc/images', output
    end

"#
        );

        // At `SafeMode::Server` (and above, including the default `Secure`), the
        // real home path is masked and `user-home` resolves to `.`, so the
        // reference expands to a relative path.
        let doc = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .parse(":imagesdir: {user-home}/etc/images\n\n{imagesdir}");
        assert_eq!(rendered_paragraphs(&doc), vec!["./etc/images".to_string()]);

        // The masked value is likewise queryable on the completed document.
        assert_eq!(doc.attribute_value("user-home").as_maybe_str(), Some("."));
    }

    #[test]
    fn user_home_attribute_can_be_overridden_by_api_if_safe_mode_is_less_than_server() {
        verifies!(
            r#"
    test 'user-home attribute can be overridden by API if safe mode is less than SERVER' do
      input = <<~'EOS'
      Go {user-home}!
      EOS
      output = convert_inline_string input, attributes: { 'user-home' => '/home' }
      assert_equal 'Go /home!', output
    end

"#
        );

        let doc = Parser::default()
            .with_safe_mode(SafeMode::Safe)
            .with_intrinsic_attribute("user-home", "/home", ModificationContext::ApiOnly)
            .parse("Go {user-home}!");
        assert_xpath(&doc, "//p[text()=\"Go /home!\"]", 1);
    }

    #[test]
    fn user_home_attribute_can_be_overridden_by_api_if_safe_mode_is_server_or_greater() {
        verifies!(
            r#"
    test 'user-home attribute can be overridden by API if safe mode is SERVER or greater' do
      input = <<~'EOS'
      Go {user-home}!
      EOS
      output = convert_inline_string input, safe: :server, attributes: { 'user-home' => '/home' }
      assert_equal 'Go /home!', output
    end

"#
        );

        let doc = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_intrinsic_attribute("user-home", "/home", ModificationContext::ApiOnly)
            .parse("Go {user-home}!");
        assert_xpath(&doc, "//p[text()=\"Go /home!\"]", 1);
    }

    #[test]
    fn apply_custom_substitutions_to_text_in_passthrough_macro_and_assign_to_attribute() {
        verifies!(
            r#"
    test "apply custom substitutions to text in passthrough macro and assign to attribute" do
      doc = document_from_string(":xml-busters: pass:[<>&]")
      assert_equal '<>&', doc.attributes['xml-busters']
      doc = document_from_string(":xml-busters: pass:none[<>&]")
      assert_equal '<>&', doc.attributes['xml-busters']
      doc = document_from_string(":xml-busters: pass:specialcharacters[<>&]")
      assert_equal '&lt;&gt;&amp;', doc.attributes['xml-busters']
      doc = document_from_string(":xml-busters: pass:n,-c[<(C)>]")
      assert_equal '<&#169;>', doc.attributes['xml-busters']
    end

"#
        );

        let doc = Parser::default().parse(":xml-busters: pass:[<>&]");
        assert_eq!(
            doc.attribute_value("xml-busters"),
            InterpretedValue::Value("<>&")
        );
        let doc = Parser::default().parse(":xml-busters: pass:none[<>&]");
        assert_eq!(
            doc.attribute_value("xml-busters"),
            InterpretedValue::Value("<>&")
        );
        let doc = Parser::default().parse(":xml-busters: pass:specialcharacters[<>&]");
        assert_eq!(
            doc.attribute_value("xml-busters"),
            InterpretedValue::Value("&lt;&gt;&amp;")
        );
        let doc = Parser::default().parse(":xml-busters: pass:n,-c[<(C)>]");
        assert_eq!(
            doc.attribute_value("xml-busters"),
            InterpretedValue::Value("<&#169;>")
        );
    }

    #[test]
    fn should_not_recognize_pass_macro_with_invalid_substitution_list_in_attribute_value() {
        verifies!(
            r#"
    test 'should not recognize pass macro with invalid substitution list in attribute value' do
      [',', '42', 'a,'].each do |subs|
        doc = document_from_string %(:pass-fail: pass:#{subs}[whale])
        assert_equal %(pass:#{subs}[whale]), doc.attributes['pass-fail']
      end
    end

"#
        );

        let doc = Parser::default().parse(":pass-fail: pass:,[whale]");
        assert_eq!(
            doc.attribute_value("pass-fail"),
            InterpretedValue::Value("pass:,[whale]")
        );
        let doc = Parser::default().parse(":pass-fail: pass:42[whale]");
        assert_eq!(
            doc.attribute_value("pass-fail"),
            InterpretedValue::Value("pass:42[whale]")
        );
        let doc = Parser::default().parse(":pass-fail: pass:a,[whale]");
        assert_eq!(
            doc.attribute_value("pass-fail"),
            InterpretedValue::Value("pass:a,[whale]")
        );
    }

    #[test]
    fn attribute_is_treated_as_defined_until_it_s_not() {
        verifies!(
            r#"
    test "attribute is treated as defined until it's not" do
      input = <<~'EOS'
      :holygrail:
      ifdef::holygrail[]
      The holy grail has been found!
      endif::holygrail[]

      :holygrail!:
      ifndef::holygrail[]
      Buggers! What happened to the grail?
      endif::holygrail[]
      EOS
      output = convert_string input
      assert_xpath '//p', output, 2
      assert_xpath '(//p)[1][text() = "The holy grail has been found!"]', output, 1
      assert_xpath '(//p)[2][text() = "Buggers! What happened to the grail?"]', output, 1
    end

"#
        );

        let doc = Parser::default().parse(
            ":holygrail:\nifdef::holygrail[]\nThe holy grail has been found!\nendif::holygrail[]\n\n:holygrail!:\nifndef::holygrail[]\nBuggers! What happened to the grail?\nendif::holygrail[]",
        );
        assert_xpath(&doc, "//p", 2);
        assert_xpath(
            &doc,
            "(//p)[1][text() = \"The holy grail has been found!\"]",
            1,
        );
        assert_xpath(
            &doc,
            "(//p)[2][text() = \"Buggers! What happened to the grail?\"]",
            1,
        );
    }

    #[test]
    fn attribute_set_via_api_overrides_attribute_set_in_document() {
        verifies!(
            r#"
    test 'attribute set via API overrides attribute set in document' do
      doc = document_from_string(':cash: money', attributes: { 'cash' => 'heroes' })
      assert_equal 'heroes', doc.attributes['cash']
    end

"#
        );

        let doc = Parser::default()
            .with_intrinsic_attribute("cash", "heroes", ModificationContext::ApiOnly)
            .parse(":cash: money");
        assert_eq!(
            doc.attribute_value("cash"),
            InterpretedValue::Value("heroes")
        );
    }

    #[test]
    fn attribute_set_via_api_cannot_be_unset_by_document() {
        verifies!(
            r#"
    test 'attribute set via API cannot be unset by document' do
      doc = document_from_string(':cash!:', attributes: { 'cash' => 'heroes' })
      assert_equal 'heroes', doc.attributes['cash']
    end

"#
        );

        let doc = Parser::default()
            .with_intrinsic_attribute("cash", "heroes", ModificationContext::ApiOnly)
            .parse(":cash!:");
        assert_eq!(
            doc.attribute_value("cash"),
            InterpretedValue::Value("heroes")
        );
    }

    #[test]
    fn attribute_soft_set_via_api_using_modifier_on_name_can_be_overridden_by_document() {
        verifies!(
            r#"
    test 'attribute soft set via API using modifier on name can be overridden by document' do
      doc = document_from_string(':cash: money', attributes: { 'cash@' => 'heroes' })
      assert_equal 'money', doc.attributes['cash']
    end

"#
        );

        // The Ruby `@` modifier (soft set) maps to a value that the document may
        // override anywhere, i.e. `ModificationContext::Anywhere`.
        let doc = Parser::default()
            .with_intrinsic_attribute("cash", "heroes", ModificationContext::Anywhere)
            .parse(":cash: money");
        assert_eq!(
            doc.attribute_value("cash"),
            InterpretedValue::Value("money")
        );
    }

    #[test]
    fn attribute_soft_set_via_api_using_modifier_on_value_can_be_overridden_by_document() {
        verifies!(
            r#"
    test 'attribute soft set via API using modifier on value can be overridden by document' do
      doc = document_from_string(':cash: money', attributes: { 'cash' => 'heroes@' })
      assert_equal 'money', doc.attributes['cash']
    end

"#
        );

        let doc = Parser::default()
            .with_intrinsic_attribute("cash", "heroes", ModificationContext::Anywhere)
            .parse(":cash: money");
        assert_eq!(
            doc.attribute_value("cash"),
            InterpretedValue::Value("money")
        );
    }

    #[test]
    fn attribute_soft_set_via_api_using_modifier_on_name_can_be_unset_by_document() {
        verifies!(
            r#"
    test 'attribute soft set via API using modifier on name can be unset by document' do
      doc = document_from_string(':cash!:', attributes: { 'cash@' => 'heroes' })
      assert_nil doc.attributes['cash']
      doc = document_from_string(':cash!:', attributes: { 'cash@' => true })
      assert_nil doc.attributes['cash']
    end

"#
        );

        let doc = Parser::default()
            .with_intrinsic_attribute("cash", "heroes", ModificationContext::Anywhere)
            .parse(":cash!:");
        assert_eq!(doc.attribute_value("cash"), InterpretedValue::Unset);
        let doc = Parser::default()
            .with_intrinsic_attribute_bool("cash", true, ModificationContext::Anywhere)
            .parse(":cash!:");
        assert_eq!(doc.attribute_value("cash"), InterpretedValue::Unset);
    }

    #[test]
    fn attribute_soft_set_via_api_using_modifier_on_value_can_be_unset_by_document() {
        verifies!(
            r#"
    test 'attribute soft set via API using modifier on value can be unset by document' do
      doc = document_from_string(':cash!:', attributes: { 'cash' => 'heroes@' })
      assert_nil doc.attributes['cash']
    end

"#
        );

        let doc = Parser::default()
            .with_intrinsic_attribute("cash", "heroes", ModificationContext::Anywhere)
            .parse(":cash!:");
        assert_eq!(doc.attribute_value("cash"), InterpretedValue::Unset);
    }

    #[test]
    fn attribute_unset_via_api_cannot_be_set_by_document() {
        verifies!(
            r#"
    test 'attribute unset via API cannot be set by document' do
      [
        { 'cash!' => '' },
        { '!cash' => '' },
        { 'cash' => nil },
      ].each do |attributes|
        doc = document_from_string(':cash: money', attributes: attributes)
        assert_nil doc.attributes['cash']
      end
    end

"#
        );

        // The three Ruby forms (`cash!`, `!cash`, `cash => nil`) all mean "unset
        // via API, locked", which maps to a single locked boolean-false intrinsic.
        let doc = Parser::default()
            .with_intrinsic_attribute_bool("cash", false, ModificationContext::ApiOnly)
            .parse(":cash: money");
        assert_eq!(doc.attribute_value("cash"), InterpretedValue::Unset);
    }

    #[test]
    fn attribute_soft_unset_via_api_can_be_set_by_document() {
        verifies!(
            r#"
    test 'attribute soft unset via API can be set by document' do
      [
        { 'cash!@' => '' },
        { '!cash@' => '' },
        { 'cash!' => '@' },
        { '!cash' => '@' },
        { 'cash' => false },
      ].each do |attributes|
        doc = document_from_string(':cash: money', attributes: attributes)
        assert_equal 'money', doc.attributes['cash']
      end
    end

"#
        );

        // All five Ruby forms are a soft unset (unset via API, overridable by the
        // document), which maps to a boolean-false intrinsic modifiable anywhere.
        let doc = Parser::default()
            .with_intrinsic_attribute_bool("cash", false, ModificationContext::Anywhere)
            .parse(":cash: money");
        assert_eq!(
            doc.attribute_value("cash"),
            InterpretedValue::Value("money")
        );
    }

    // Tracked in #734.
    #[test]
    fn can_soft_unset_built_in_attribute_from_api_and_still_override_in_document() {
        verifies!(
            r#"
    test 'can soft unset built-in attribute from API and still override in document' do
      [
        { 'sectids!@' => '' },
        { '!sectids@' => '' },
        { 'sectids!' => '@' },
        { '!sectids' => '@' },
        { 'sectids' => false },
      ].each do |attributes|
        doc = document_from_string '== Heading', attributes: attributes
        refute doc.attr?('sectids')
        assert_css '#_heading', (doc.convert standalone: false), 0
        doc = document_from_string %(:sectids:\n\n== Heading), attributes: attributes
        assert doc.attr?('sectids')
        assert_css '#_heading', (doc.convert standalone: false), 1
      end
    end

"#
        );

        // All five Ruby forms are a soft unset of the built-in `sectids`
        // attribute (unset via API, overridable by the document), which maps to
        // a boolean-false intrinsic modifiable anywhere.
        let doc = Parser::default()
            .with_intrinsic_attribute_bool("sectids", false, ModificationContext::Anywhere)
            .parse("== Heading");
        assert!(!doc.is_attribute_set("sectids"));
        assert_css(&doc, "#_heading", 0);

        let doc = Parser::default()
            .with_intrinsic_attribute_bool("sectids", false, ModificationContext::Anywhere)
            .parse(":sectids:\n\n== Heading");
        assert!(doc.is_attribute_set("sectids"));
        // The generated section id lands on the heading `h2` only, not on the
        // section wrapper `div`, so a bare `#_heading` matches exactly once.
        assert_css(&doc, "#_heading", 1);
        assert_css(&doc, "h2#_heading", 1);
    }

    #[test]
    fn backend_and_doctype_attributes_are_set_by_default_in_default_configuration() {
        verifies!(
            r#"
    test 'backend and doctype attributes are set by default in default configuration' do
      input = <<~'EOS'
      = Document Title
      Author Name

      content
      EOS

      doc = document_from_string input
      expect = {
        'backend' => 'html5',
        'backend-html5' => '',
        'backend-html5-doctype-article' => '',
        'outfilesuffix' => '.html',
        'basebackend' => 'html',
        'basebackend-html' => '',
        'basebackend-html-doctype-article' => '',
        'doctype' => 'article',
        'doctype-article' => '',
        'filetype' => 'html',
        'filetype-html' => '',
      }
      expect.each do |key, val|
        assert doc.attributes.key? key
        assert_equal val, doc.attributes[key]
      end
    end

"#
        );

        // The derived backend / basebackend / filetype / doctype family is
        // materialized as queryable document attributes. A `''` Ruby value maps
        // to an empty [`InterpretedValue::Value`] here; each key must be present
        // (`has_attribute`) and resolve to the expected value.
        let doc = Parser::default().parse("= Document Title\nAuthor Name\n\ncontent");
        for (key, value) in [
            ("backend", "html5"),
            ("backend-html5", ""),
            ("backend-html5-doctype-article", ""),
            ("outfilesuffix", ".html"),
            ("basebackend", "html"),
            ("basebackend-html", ""),
            ("basebackend-html-doctype-article", ""),
            ("doctype", "article"),
            ("doctype-article", ""),
            ("filetype", "html"),
            ("filetype-html", ""),
        ] {
            assert!(doc.has_attribute(key), "missing attribute {key:?}");
            assert_eq!(
                doc.attribute_value(key),
                InterpretedValue::Value(value),
                "unexpected value for {key:?}"
            );
        }
    }

    // Out of scope: targets the DocBook backend, which is out of scope for this
    // crate.
    non_normative!(
        r#"
    test 'backend and doctype attributes are set by default in custom configuration' do
      input = <<~'EOS'
      = Document Title
      Author Name

      content
      EOS

      doc = document_from_string input, doctype: 'book', backend: 'docbook'
      expect = {
        'backend' => 'docbook5',
        'backend-docbook5' => '',
        'backend-docbook5-doctype-book' => '',
        'outfilesuffix' => '.xml',
        'basebackend' => 'docbook',
        'basebackend-docbook' => '',
        'basebackend-docbook-doctype-book' => '',
        'doctype' => 'book',
        'doctype-book' => '',
        'filetype' => 'xml',
        'filetype-xml' => '',
      }
      expect.each do |key, val|
        assert doc.attributes.key? key
        assert_equal val, doc.attributes[key]
      end
    end

"#
    );

    // Out of scope: targets the DocBook backend, which is out of scope for this
    // crate.
    non_normative!(
        r#"
    test 'backend attributes are updated if backend attribute is defined in document and safe mode is less than SERVER' do
      input = <<~'EOS'
      = Document Title
      Author Name
      :backend: docbook
      :doctype: book

      content
      EOS

      doc = document_from_string input, safe: Asciidoctor::SafeMode::SAFE
      expect = {
        'backend' => 'docbook5',
        'backend-docbook5' => '',
        'backend-docbook5-doctype-book' => '',
        'outfilesuffix' => '.xml',
        'basebackend' => 'docbook',
        'basebackend-docbook' => '',
        'basebackend-docbook-doctype-book' => '',
        'doctype' => 'book',
        'doctype-book' => '',
        'filetype' => 'xml',
        'filetype-xml' => '',
      }
      expect.each do |key, val|
        assert doc.attributes.key?(key)
        assert_equal val, doc.attributes[key]
      end

      refute doc.attributes.key?('backend-html5')
      refute doc.attributes.key?('backend-html5-doctype-article')
      refute doc.attributes.key?('basebackend-html')
      refute doc.attributes.key?('basebackend-html-doctype-article')
      refute doc.attributes.key?('doctype-article')
      refute doc.attributes.key?('filetype-html')
    end

"#
    );

    #[test]
    fn backend_attributes_defined_in_document_options_overrides_backend_attribute_in_document() {
        verifies!(
            r#"
    test 'backend attributes defined in document options overrides backend attribute in document' do
      doc = document_from_string(':backend: docbook5', safe: Asciidoctor::SafeMode::SAFE, attributes: { 'backend' => 'html5' })
      assert_equal 'html5', doc.attributes['backend']
      assert doc.attributes.key? 'backend-html5'
      assert_equal 'html', doc.attributes['basebackend']
      assert doc.attributes.key? 'basebackend-html'
    end

"#
        );

        // The API-set `backend` value (locked, `ApiOnly`) wins over the document
        // `:backend: docbook5` assignment, and the derived `backend-html5` /
        // `basebackend` / `basebackend-html` attributes are synthesized from the
        // winning value.
        let doc = Parser::default()
            .with_safe_mode(SafeMode::Safe)
            .with_intrinsic_attribute("backend", "html5", ModificationContext::ApiOnly)
            .parse(":backend: docbook5");
        assert_eq!(
            doc.attribute_value("backend"),
            InterpretedValue::Value("html5")
        );
        assert!(doc.has_attribute("backend-html5"));
        assert_eq!(
            doc.attribute_value("basebackend"),
            InterpretedValue::Value("html")
        );
        assert!(doc.has_attribute("basebackend-html"));
    }

    // Out of scope: exercises Ruby's mutable node attribute API (`Block.new` /
    // positional `attr`), which this crate does not expose.
    non_normative!(
        r#"
    test 'can only access a positional attribute from the attributes hash' do
      node = Asciidoctor::Block.new nil, :paragraph, attributes: { 1 => 'position 1' }
      assert_nil node.attr(1)
      refute node.attr?(1)
      assert_equal 'position 1', node.attributes[1]
    end

"#
    );

    // Out of scope: exercises Ruby's node `attr` document-fallback API, which this
    // crate does not expose.
    non_normative!(
        r#"
    test 'attr should not retrieve attribute from document if not set on block' do
      doc = document_from_string 'paragraph', attributes: { 'name' => 'value' }
      para = doc.blocks[0]
      assert_nil para.attr 'name'
    end

"#
    );

    // Out of scope: exercises Ruby's node `attr` document-fallback API, which this
    // crate does not expose.
    non_normative!(
        r#"
    test 'attr looks for attribute on document if fallback name is true' do
      doc = document_from_string 'paragraph', attributes: { 'name' => 'value' }
      para = doc.blocks[0]
      assert_equal 'value', (para.attr 'name', nil, true)
    end

"#
    );

    // Out of scope: exercises Ruby's node `attr` document-fallback API, which this
    // crate does not expose.
    non_normative!(
        r#"
    test 'attr uses fallback name when looking for attribute on document' do
      doc = document_from_string 'paragraph', attributes: { 'alt-name' => 'value' }
      para = doc.blocks[0]
      assert_equal 'value', (para.attr 'name', nil, 'alt-name')
    end

"#
    );

    // Out of scope: exercises Ruby's node `attr?` document-fallback API, which this
    // crate does not expose.
    non_normative!(
        r#"
    test 'attr? should not check for attribute on document if not set on block' do
      doc = document_from_string 'paragraph', attributes: { 'name' => 'value' }
      para = doc.blocks[0]
      refute para.attr? 'name'
    end

"#
    );

    // Out of scope: exercises Ruby's node `attr?` document-fallback API, which this
    // crate does not expose.
    non_normative!(
        r#"
    test 'attr? checks for attribute on document if fallback name is true' do
      doc = document_from_string 'paragraph', attributes: { 'name' => 'value' }
      para = doc.blocks[0]
      assert para.attr? 'name', nil, true
    end

"#
    );

    // Out of scope: exercises Ruby's node `attr?` document-fallback API, which this
    // crate does not expose.
    non_normative!(
        r#"
    test 'attr? checks for fallback name when looking for attribute on document' do
      doc = document_from_string 'paragraph', attributes: { 'alt-name' => 'value' }
      para = doc.blocks[0]
      assert para.attr? 'name', nil, 'alt-name'
    end

"#
    );

    // Out of scope: exercises Ruby's mutable node `set_attr` API, which this crate
    // does not expose.
    non_normative!(
        r#"
    test 'set_attr should set value to empty string if no value is specified' do
      node = Asciidoctor::Block.new nil, :paragraph, attributes: {}
      node.set_attr 'foo'
      assert_equal '', (node.attr 'foo')
    end

"#
    );

    // Out of scope: exercises Ruby's mutable node `remove_attr` API, which this
    // crate does not expose.
    non_normative!(
        r#"
    test 'remove_attr should remove attribute and return previous value' do
      doc = empty_document
      node = Asciidoctor::Block.new doc, :paragraph, attributes: { 'foo' => 'bar' }
      assert_equal 'bar', (node.remove_attr 'foo')
      assert_nil node.attr('foo')
    end

"#
    );

    // Out of scope: exercises Ruby's mutable node `set_attr` API, which this crate
    // does not expose.
    non_normative!(
        r#"
    test 'set_attr should not overwrite existing key if overwrite is false' do
      node = Asciidoctor::Block.new nil, :paragraph, attributes: { 'foo' => 'bar' }
      assert_equal 'bar', (node.attr 'foo')
      node.set_attr 'foo', 'baz', false
      assert_equal 'bar', (node.attr 'foo')
    end

"#
    );

    // Out of scope: exercises Ruby's mutable node `set_attr` API, which this crate
    // does not expose.
    non_normative!(
        r#"
    test 'set_attr should overwrite existing key by default' do
      node = Asciidoctor::Block.new nil, :paragraph, attributes: { 'foo' => 'bar' }
      assert_equal 'bar', (node.attr 'foo')
      node.set_attr 'foo', 'baz'
      assert_equal 'baz', (node.attr 'foo')
    end

"#
    );

    // Out of scope: exercises Ruby's post-load `set_attr` mutation and
    // re-conversion, which this crate does not expose.
    non_normative!(
        r#"
    test 'set_attr should set header attribute in loaded document' do
      input = <<~'EOS'
      :uri: http://example.org

      {uri}
      EOS

      doc = Asciidoctor.load input, attributes: { 'uri' => 'https://github.com' }
      doc.set_attr 'uri', 'https://google.com'
      output = doc.convert
      assert_xpath '//a[@href="https://google.com"]', output, 1
    end

"#
    );

    // Out of scope: exercises Ruby's mutable `Document#set_attribute` API, which
    // this crate does not expose.
    non_normative!(
        r#"
    test 'set_attribute should set attribute if key is not locked' do
      doc = empty_document
      refute doc.attr? 'foo'
      res = doc.set_attribute 'foo', 'baz'
      assert res
      assert_equal 'baz', (doc.attr 'foo')
    end

"#
    );

    // Out of scope: exercises Ruby's mutable `Document#set_attribute` API, which
    // this crate does not expose.
    non_normative!(
        r#"
    test 'set_attribute should not set key if key is locked' do
      doc = empty_document attributes: { 'foo' => 'bar' }
      assert_equal 'bar', (doc.attr 'foo')
      res = doc.set_attribute 'foo', 'baz'
      refute res
      assert_equal 'bar', (doc.attr 'foo')
    end

"#
    );

    // Out of scope: exercises Ruby's mutable `Document#set_attribute` API, which
    // this crate does not expose.
    non_normative!(
        r#"
    test 'set_attribute should update backend attributes' do
      doc = empty_document attributes: { 'backend' => 'html5@' }
      assert_equal '', (doc.attr 'backend-html5')
      res = doc.set_attribute 'backend', 'docbook5'
      assert res
      refute doc.attr? 'backend-html5'
      assert_equal '', (doc.attr 'backend-docbook5')
    end

"#
    );

    #[test]
    fn verify_toc_attribute_matrix() {
        verifies!(
            r#"
    test 'verify toc attribute matrix' do
      expected_data = <<~'EOS'
      #attributes                               |toc|toc-position|toc-placement|toc-class
      toc                                       |   |nil         |auto         |nil
      toc=header                                |   |nil         |auto         |nil
      toc=beeboo                                |   |nil         |auto         |nil
      toc=left                                  |   |left        |auto         |toc2
      toc2                                      |   |left        |auto         |toc2
      toc=right                                 |   |right       |auto         |toc2
      toc=preamble                              |   |content     |preamble     |nil
      toc=macro                                 |   |content     |macro        |nil
      toc toc-placement=macro toc-position=left |   |content     |macro        |nil
      toc toc-placement!                        |   |content     |macro        |nil
      EOS

      expected = expected_data.lines.map do |l|
        next if l.start_with? '#'
        l.split('|').map {|e| (e = e.strip) == 'nil' ? nil : e }
      end.compact

      expected.each do |expect|
        raw_attrs, toc, toc_position, toc_placement, toc_class = expect
        attrs = Hash[*raw_attrs.split.map {|e| e.include?('=') ? e.split('=', 2) : [e, ''] }.flatten]
        doc = document_from_string '', attributes: attrs
        toc ? (assert doc.attr?('toc', toc)) : (refute doc.attr?('toc'))
        toc_position ? (assert doc.attr?('toc-position', toc_position)) : (refute doc.attr?('toc-position'))
        toc_placement ? (assert doc.attr?('toc-placement', toc_placement)) : (refute doc.attr?('toc-placement'))
        toc_class ? (assert doc.attr?('toc-class', toc_class)) : (refute doc.attr?('toc-class'))
      end
    end
"#
        );

        // This crate now materializes the derived `toc-position` /
        // `toc-placement` / `toc-class` document attributes from the resolved
        // placement (close #840), so each matrix row is asserted directly
        // through `attribute_value` / `has_attribute`, exactly as Asciidoctor's
        // `doc.attr?` checks do. A `None` column marks a value Asciidoctor
        // leaves unset (its `nil`).
        //
        // The one remaining modeling difference is the `toc` column itself:
        // Asciidoctor resets `toc` to an empty value (so `doc.attr?('toc', '')`
        // holds) once it has folded the placement keyword out into
        // `toc-placement` / `toc-position`, and materializes it even for the
        // legacy `toc2` alias. This crate instead keeps the author's raw `toc`
        // value (and never materializes `toc` for the `toc2` alias), exposing
        // the placement through `toc_mode()`. Every row enables a TOC — what the
        // matrix's `toc` column asserts — so that column is checked here via the
        // resolved `toc_mode()`.
        use crate::document::TocMode;

        // (spec, toc-position, toc-placement, toc-class, toc_mode). Each spec is
        // parsed the way Ruby parses the vendored matrix: space-separated
        // entries, `name=value`, or `name!` for a soft unset. A `None` column is
        // an attribute Asciidoctor leaves unset (its `nil`).
        type Row = (
            &'static str,
            Option<&'static str>,
            Option<&'static str>,
            Option<&'static str>,
            TocMode,
        );
        let rows: &[Row] = &[
            ("toc", None, Some("auto"), None, TocMode::Auto),
            ("toc=header", None, Some("auto"), None, TocMode::Auto),
            ("toc=beeboo", None, Some("auto"), None, TocMode::Auto),
            (
                "toc=left",
                Some("left"),
                Some("auto"),
                Some("toc2"),
                TocMode::Left,
            ),
            // `toc2` is a legacy alias for a left-placed TOC (#748); its
            // side-column placement derives `toc-position=left` and defaults
            // `toc-class` to `toc2` (#749), matching Asciidoctor.
            (
                "toc2",
                Some("left"),
                Some("auto"),
                Some("toc2"),
                TocMode::Left,
            ),
            (
                "toc=right",
                Some("right"),
                Some("auto"),
                Some("toc2"),
                TocMode::Right,
            ),
            (
                "toc=preamble",
                Some("content"),
                Some("preamble"),
                None,
                TocMode::Preamble,
            ),
            (
                "toc=macro",
                Some("content"),
                Some("macro"),
                None,
                TocMode::Macro,
            ),
            // The fully-derived `toc-position` overwrites the author's explicit
            // `toc-position=left`: the `macro` placement forces `content`.
            (
                "toc toc-placement=macro toc-position=left",
                Some("content"),
                Some("macro"),
                None,
                TocMode::Macro,
            ),
            // Soft-unset `toc-placement!` with `toc` set defers to a `toc::[]`
            // macro (#750); the derived `toc-placement` is re-materialized as
            // `macro`, overwriting the unset tombstone.
            (
                "toc toc-placement!",
                Some("content"),
                Some("macro"),
                None,
                TocMode::Macro,
            ),
        ];

        for (spec, toc_position, toc_placement, toc_class, expected_mode) in rows {
            let mut parser = Parser::default();
            for entry in spec.split(' ') {
                parser = if let Some(name) = entry.strip_suffix('!') {
                    parser.with_intrinsic_attribute_bool(name, false, ModificationContext::Anywhere)
                } else if let Some((name, value)) = entry.split_once('=') {
                    parser.with_intrinsic_attribute(name, value, ModificationContext::Anywhere)
                } else {
                    parser.with_intrinsic_attribute(entry, "", ModificationContext::Anywhere)
                };
            }
            let doc = parser.parse("");

            // Every row enables a TOC; the placement lives in `toc_mode()`.
            assert_eq!(doc.toc_mode(), *expected_mode, "toc_mode for {spec:?}");
            assert!(doc.toc_mode().is_enabled(), "toc enabled for {spec:?}");

            for (name, expected) in [
                ("toc-position", *toc_position),
                ("toc-placement", *toc_placement),
                ("toc-class", *toc_class),
            ] {
                match expected {
                    Some(value) => assert_eq!(
                        doc.attribute_value(name),
                        InterpretedValue::Value(value),
                        "{name} for {spec:?}"
                    ),
                    None => assert!(
                        !doc.has_attribute(name),
                        "{name} should be unset for {spec:?}"
                    ),
                }
            }
        }
    }
}

non_normative!(
    r#"
  end

  context 'Interpolation' do
"#
);
mod interpolation {
    use crate::tests::prelude::*;

    non_normative!(
        r#"

"#
    );
    #[test]
    fn convert_properly_with_simple_names() {
        verifies!(
            r#"
    test "convert properly with simple names" do
      html = convert_string(":frog: Tanglefoot\n:my_super-hero: Spiderman\n\nYo, {frog}!\nBeat {my_super-hero}!")
      assert_xpath %(//p[text()="Yo, Tanglefoot!\nBeat Spiderman!"]), html, 1
    end

"#
        );

        let doc = Parser::default().parse(
            ":frog: Tanglefoot\n:my_super-hero: Spiderman\n\nYo, {frog}!\nBeat {my_super-hero}!",
        );
        assert_xpath(&doc, "//p[text()=\"Yo, Tanglefoot!\nBeat Spiderman!\"]", 1);
    }

    // Tracked in #724.
    #[test]
    fn attribute_lookup_is_not_case_sensitive() {
        non_normative!(
            r#"
    test 'attribute lookup is not case sensitive' do
      input = <<~'EOS'
      :He-Man: The most powerful man in the universe

      He-Man: {He-Man}

      She-Ra: {She-Ra}
      EOS
      result = convert_string_to_embedded input, attributes: { 'She-Ra' => 'The Princess of Power' }
      assert_xpath '//p[text()="He-Man: The most powerful man in the universe"]', result, 1
      assert_xpath '//p[text()="She-Ra: The Princess of Power"]', result, 1
    end

"#
        );

        let doc = Parser::default()
            .with_intrinsic_attribute(
                "She-Ra",
                "The Princess of Power",
                ModificationContext::ApiOnly,
            )
            .parse(
                ":He-Man: The most powerful man in the universe\n\nHe-Man: {He-Man}\n\nShe-Ra: {She-Ra}",
            );
        // Divergence: attribute *references* are case-sensitive in this crate,
        // so `{He-Man}` does not resolve against the lowercased `:He-Man:` entry
        // and `{She-Ra}` does not match the API attribute stored as `she-ra`;
        // both are left literal. Asciidoctor performs a case-insensitive lookup.
        assert_xpath(&doc, "//p[text()=\"He-Man: {He-Man}\"]", 1);
        assert_xpath(&doc, "//p[text()=\"She-Ra: {She-Ra}\"]", 1);
    }

    #[test]
    fn convert_properly_with_single_character_name() {
        verifies!(
            r#"
    test "convert properly with single character name" do
      html = convert_string(":r: Ruby\n\nR is for {r}!")
      assert_xpath %(//p[text()="R is for Ruby!"]), html, 1
    end

"#
        );

        let doc = Parser::default().parse(":r: Ruby\n\nR is for {r}!");
        assert_xpath(&doc, "//p[text()=\"R is for Ruby!\"]", 1);
    }

    #[test]
    fn collapses_spaces_in_attribute_names() {
        verifies!(
            r#"
    test "collapses spaces in attribute names" do
      input = <<~'EOS'
      Main Header
      ===========
      :My frog: Tanglefoot

      Yo, {myfrog}!
      EOS
      output = convert_string input
      assert_xpath '(//p)[1][text()="Yo, Tanglefoot!"]', output, 1
    end

"#
        );

        // The attribute-entry name `My frog` is sanitized to `myfrog` (spaces
        // dropped, lower-cased), so the `{myfrog}` reference resolves. See
        // https://github.com/asciidoc-rs/asciidoc-parser/issues/761 (previously
        // tracked as a divergence in #726).
        let doc = Parser::default()
            .parse("Main Header\n===========\n:My frog: Tanglefoot\n\nYo, {myfrog}!");
        assert_xpath(&doc, "//p[text()=\"Yo, Tanglefoot!\"]", 1);
    }

    #[test]
    fn ignores_lines_with_bad_attributes_if_attribute_missing_is_drop_line() {
        verifies!(
            r#"
    test 'ignores lines with bad attributes if attribute-missing is drop-line' do
      input = <<~'EOS'
      :attribute-missing: drop-line

      This is
      blah blah {foobarbaz}
      all there is.
      EOS
      output = convert_string_to_embedded input
      para = xmlnodes_at_css 'p', output, 1
      refute_includes 'blah blah', para.content
      assert_message @logger, :INFO, 'dropping line containing reference to missing attribute: foobarbaz'
    end

"#
        );

        // Only the observable line drop is checked; the INFO log is not modeled.
        let doc = Parser::default().parse(
            ":attribute-missing: drop-line\n\nThis is\nblah blah {foobarbaz}\nall there is.",
        );
        assert_xpath(&doc, "//p[text()=\"This is\nall there is.\"]", 1);
    }

    #[test]
    fn attribute_value_gets_interpreted_when_converting() {
        verifies!(
            r#"
    test 'attribute value gets interpreted when converting' do
      doc = document_from_string(":google: http://google.com[Google]\n\n{google}")
      assert_equal 'http://google.com[Google]', doc.attributes['google']
      output = doc.convert
      assert_xpath '//a[@href="http://google.com"][text() = "Google"]', output, 1
    end

"#
        );

        let doc = Parser::default().parse(":google: http://google.com[Google]\n\n{google}");
        assert_eq!(
            doc.attribute_value("google"),
            InterpretedValue::Value("http://google.com[Google]")
        );
        assert_xpath(
            &doc,
            "//a[@href=\"http://google.com\"][text() = \"Google\"]",
            1,
        );
    }

    #[test]
    fn should_drop_line_with_reference_to_missing_attribute_if_attribute_missing_attribute_is_drop_line()
     {
        verifies!(
            r#"
    test 'should drop line with reference to missing attribute if attribute-missing attribute is drop-line' do
      input = <<~'EOS'
      :attribute-missing: drop-line

      Line 1: This line should appear in the output.
      Line 2: Oh no, a {bogus-attribute}! This line should not appear in the output.
      EOS

      output = convert_string_to_embedded input
      assert_match(/Line 1/, output)
      refute_match(/Line 2/, output)
      assert_message @logger, :INFO, 'dropping line containing reference to missing attribute: bogus-attribute'
    end

"#
        );

        let doc = Parser::default().parse(
            ":attribute-missing: drop-line\n\nLine 1: This line should appear in the output.\nLine 2: Oh no, a {bogus-attribute}! This line should not appear in the output.",
        );
        assert_rendered_contains(&doc, "Line 1");
        refute_rendered_contains(&doc, "Line 2");
    }

    #[test]
    fn should_not_drop_line_with_reference_to_missing_attribute_by_default() {
        verifies!(
            r#"
    test 'should not drop line with reference to missing attribute by default' do
      input = <<~'EOS'
      Line 1: This line should appear in the output.
      Line 2: A {bogus-attribute}! This time, this line should appear in the output.
      EOS

      output = convert_string_to_embedded input
      assert_match(/Line 1/, output)
      assert_match(/Line 2/, output)
      assert_match(/\{bogus-attribute\}/, output)
    end

"#
        );

        let doc = Parser::default().parse(
            "Line 1: This line should appear in the output.\nLine 2: A {bogus-attribute}! This time, this line should appear in the output.",
        );
        assert_rendered_contains(&doc, "Line 1");
        assert_rendered_contains(&doc, "Line 2");
        assert_rendered_contains(&doc, "{bogus-attribute}");
    }

    #[test]
    fn should_drop_line_with_attribute_unassignment_by_default() {
        non_normative!(
            r#"
    test 'should drop line with attribute unassignment by default' do
      input = <<~'EOS'
      :a:

      Line 1: This line should appear in the output.
      Line 2: {set:a!}This line should not appear in the output.
      EOS

      output = convert_string_to_embedded input
      assert_match(/Line 1/, output)
      refute_match(/Line 2/, output)
    end

"#
        );

        // Intentional divergence: this crate does not implement the `{set:…}` attribute
        // assignment/unassignment reference, so `{set:a!}` is left literal and
        // its line is not dropped.
        let doc = Parser::default().parse(
            ":a:\n\nLine 1: This line should appear in the output.\nLine 2: {set:a!}This line should not appear in the output.",
        );
        assert_rendered_contains(&doc, "Line 1");
        assert_rendered_contains(&doc, "Line 2");
    }

    #[test]
    fn should_not_drop_line_with_attribute_unassignment_if_attribute_undefined_is_drop() {
        non_normative!(
            r#"
    test 'should not drop line with attribute unassignment if attribute-undefined is drop' do
      input = <<~'EOS'
      :attribute-undefined: drop
      :a:

      Line 1: This line should appear in the output.
      Line 2: {set:a!}This line should appear in the output.
      EOS

      output = convert_string_to_embedded input
      assert_match(/Line 1/, output)
      assert_match(/Line 2/, output)
      refute_match(/\{set:a!\}/, output)
    end

"#
        );

        // Intentional divergence: `{set:…}` is not implemented, so `{set:a!}` remains
        // literal in the output rather than being consumed.
        let doc = Parser::default().parse(
            ":attribute-undefined: drop\n:a:\n\nLine 1: This line should appear in the output.\nLine 2: {set:a!}This line should appear in the output.",
        );
        assert_rendered_contains(&doc, "Line 1");
        assert_rendered_contains(&doc, "Line 2");
        assert_rendered_contains(&doc, "{set:a!}");
    }

    #[test]
    fn should_drop_line_that_only_contains_attribute_assignment() {
        non_normative!(
            r#"
    test 'should drop line that only contains attribute assignment' do
      input = <<~'EOS'
      Line 1
      {set:a}
      Line 2
      EOS

      output = convert_string_to_embedded input
      assert_xpath %(//p[text()="Line 1\nLine 2"]), output, 1
    end

"#
        );

        // Intentional divergence: `{set:…}` is not implemented, so the `{set:a}` line
        // is kept literally instead of being dropped.
        let doc = Parser::default().parse("Line 1\n{set:a}\nLine 2");
        assert_xpath(&doc, "//p[text()=\"Line 1\n{set:a}\nLine 2\"]", 1);
    }

    #[test]
    fn should_drop_line_that_only_contains_unresolved_attribute_when_attribute_missing_is_drop() {
        verifies!(
            r#"
    test 'should drop line that only contains unresolved attribute when attribute-missing is drop' do
      input = <<~'EOS'
      Line 1
      {unresolved}
      Line 2
      EOS

      output = convert_string_to_embedded input, attributes: { 'attribute-missing' => 'drop' }
      assert_xpath %(//p[text()="Line 1\nLine 2"]), output, 1
    end

"#
        );

        // With `attribute-missing=drop`, a line that contains only an
        // unresolved reference is dropped entirely (the reference removal
        // empties the line), matching Asciidoctor's `reject_if_empty` (#730).
        let doc = Parser::default()
            .with_intrinsic_attribute("attribute-missing", "drop", ModificationContext::ApiOnly)
            .parse("Line 1\n{unresolved}\nLine 2");
        assert_eq!(
            rendered_paragraphs(&doc),
            vec!["Line 1\nLine 2".to_string()]
        );
    }

    #[test]
    fn substitutes_inside_unordered_list_items() {
        verifies!(
            r#"
    test "substitutes inside unordered list items" do
      html = convert_string(":foo: bar\n* snort at the {foo}\n* yawn")
      assert_xpath %(//li/p[text()="snort at the bar"]), html, 1
    end

"#
        );

        let doc = Parser::default().parse(":foo: bar\n* snort at the {foo}\n* yawn");
        assert_xpath(&doc, "//li/p[text()=\"snort at the bar\"]", 1);
    }

    #[test]
    fn substitutes_inside_section_title() {
        verifies!(
            r#"
    test 'substitutes inside section title' do
      output = convert_string(":prefix: Cool\n\n== {prefix} Title\n\ncontent")
      assert_xpath '//h2[text()="Cool Title"]', output, 1
      assert_css 'h2#_cool_title', output, 1
    end

"#
        );

        let doc = Parser::default().parse(":prefix: Cool\n\n== {prefix} Title\n\ncontent");
        assert_xpath(&doc, "//h2[text()=\"Cool Title\"]", 1);
        assert_css(&doc, "h2#_cool_title", 1);
    }

    #[test]
    fn interpolates_attribute_defined_in_header_inside_attribute_entry_in_header() {
        verifies!(
            r#"
    test 'interpolates attribute defined in header inside attribute entry in header' do
      input = <<~'EOS'
      = Title
      Author Name
      :attribute-a: value
      :attribute-b: {attribute-a}

      preamble
      EOS
      doc = document_from_string(input, parse_header_only: true)
      assert_equal 'value', doc.attributes['attribute-b']
    end

"#
        );

        // `parse_header_only` isn't needed here: a full parse resolves the same
        // header attribute value.
        let doc = Parser::default().parse(
            "= Title\nAuthor Name\n:attribute-a: value\n:attribute-b: {attribute-a}\n\npreamble",
        );
        assert_eq!(
            doc.attribute_value("attribute-b"),
            InterpretedValue::Value("value")
        );
    }

    #[test]
    fn interpolates_author_attribute_inside_attribute_entry_in_header() {
        verifies!(
            r#"
    test 'interpolates author attribute inside attribute entry in header' do
      input = <<~'EOS'
      = Title
      Author Name
      :name: {author}

      preamble
      EOS
      doc = document_from_string(input, parse_header_only: true)
      assert_equal 'Author Name', doc.attributes['name']
    end

"#
        );

        let doc = Parser::default().parse("= Title\nAuthor Name\n:name: {author}\n\npreamble");
        assert_eq!(
            doc.attribute_value("name"),
            InterpretedValue::Value("Author Name")
        );
    }

    #[test]
    fn interpolates_revinfo_attribute_inside_attribute_entry_in_header() {
        verifies!(
            r#"
    test 'interpolates revinfo attribute inside attribute entry in header' do
      input = <<~'EOS'
      = Title
      Author Name
      2013-01-01
      :date: {revdate}

      preamble
      EOS
      doc = document_from_string(input, parse_header_only: true)
      assert_equal '2013-01-01', doc.attributes['date']
    end

"#
        );

        let doc = Parser::default()
            .parse("= Title\nAuthor Name\n2013-01-01\n:date: {revdate}\n\npreamble");
        assert_eq!(
            doc.attribute_value("date"),
            InterpretedValue::Value("2013-01-01")
        );
    }

    #[test]
    fn attribute_entries_can_resolve_previously_defined_attributes() {
        verifies!(
            r#"
    test 'attribute entries can resolve previously defined attributes' do
      input = <<~'EOS'
      = Title
      Author Name
      v1.0, 2010-01-01: First release!
      :a: value
      :a2: {a}
      :revdate2: {revdate}

      {a} == {a2}

      {revdate} == {revdate2}
      EOS

      doc = document_from_string input
      assert_equal '2010-01-01', doc.attr('revdate')
      assert_equal '2010-01-01', doc.attr('revdate2')
      assert_equal 'value', doc.attr('a')
      assert_equal 'value', doc.attr('a2')

      output = doc.convert
      assert_includes output, 'value == value'
      assert_includes output, '2010-01-01 == 2010-01-01'
    end

"#
        );

        let doc = Parser::default().parse(
            "= Title\nAuthor Name\nv1.0, 2010-01-01: First release!\n:a: value\n:a2: {a}\n:revdate2: {revdate}\n\n{a} == {a2}\n\n{revdate} == {revdate2}",
        );
        assert_eq!(
            doc.attribute_value("revdate"),
            InterpretedValue::Value("2010-01-01")
        );
        assert_eq!(
            doc.attribute_value("revdate2"),
            InterpretedValue::Value("2010-01-01")
        );
        assert_eq!(doc.attribute_value("a"), InterpretedValue::Value("value"));
        assert_eq!(doc.attribute_value("a2"), InterpretedValue::Value("value"));
        assert_rendered_contains(&doc, "value == value");
        assert_rendered_contains(&doc, "2010-01-01 == 2010-01-01");
    }

    // Tracked in #731.
    #[test]
    fn should_warn_if_unterminated_block_comment_is_detected_in_document_header() {
        non_normative!(
            r#"
    test 'should warn if unterminated block comment is detected in document header' do
      input = <<~'EOS'
      = Document Title
      :foo: bar
      ////
      :hey: there

      content
      EOS
      doc = document_from_string input
      assert_nil doc.attr('hey')
      assert_message @logger, :WARN, '<stdin>: line 3: unterminated comment block', Hash
    end

"#
        );

        // As in Asciidoctor, the unterminated `////` opens a comment block that
        // swallows the rest of the header, so `:hey: there` is never applied
        // (see https://github.com/asciidoc-rs/asciidoc-parser/issues/760).
        // Remaining divergence: the `unterminated comment block` warning is not
        // yet modeled (tracked in #731).
        let doc =
            Parser::default().parse("= Document Title\n:foo: bar\n////\n:hey: there\n\ncontent");
        assert_eq!(doc.attribute_value("hey"), InterpretedValue::Unset);
    }

    #[test]
    fn substitutes_inside_block_title() {
        non_normative!(
            r#"
    test 'substitutes inside block title' do
      input = <<~'EOS'
      :gem_name: asciidoctor

      .Require the +{gem_name}+ gem
      To use {gem_name}, the first thing to do is to import it in your Ruby source file.
      EOS
      output = convert_string_to_embedded input, attributes: { 'compat-mode' => '' }
      assert_xpath '//*[@class="title"]/code[text()="asciidoctor"]', output, 1

      input = <<~'EOS'
      :gem_name: asciidoctor

      .Require the `{gem_name}` gem
      To use {gem_name}, the first thing to do is to import it in your Ruby source file.
      EOS
      output = convert_string_to_embedded input
      assert_xpath '//*[@class="title"]/code[text()="asciidoctor"]', output, 1
    end

"#
        );

        // A block title is exposed via the `title()` accessor (rendered with
        // inline substitutions applied) rather than as a queryable DOM node.
        //
        // The modern backtick form (`` `{gem_name}` ``) substitutes and wraps in
        // `<code>` as Asciidoctor does. The compat-mode `+…+` monospace form is
        // an intentional divergence — compat mode is not supported, so the `+…+`
        // markers are dropped and the attribute reference is left unsubstituted;
        // that first case is recorded as the crate's actual output.
        let doc = Parser::default()
            .with_intrinsic_attribute("compat-mode", "", ModificationContext::Anywhere)
            .parse(
                ":gem_name: asciidoctor\n\n.Require the +{gem_name}+ gem\nTo use {gem_name}, the first thing to do is to import it in your Ruby source file.",
            );
        assert_eq!(
            doc.nested_blocks().next().unwrap().title(),
            Some("Require the {gem_name} gem")
        );

        let doc = Parser::default().parse(
            ":gem_name: asciidoctor\n\n.Require the `{gem_name}` gem\nTo use {gem_name}, the first thing to do is to import it in your Ruby source file.",
        );
        assert_eq!(
            doc.nested_blocks().next().unwrap().title(),
            Some("Require the <code>asciidoctor</code> gem")
        );
    }

    #[test]
    fn sets_attribute_until_it_is_deleted() {
        verifies!(
            r#"
    test 'sets attribute until it is deleted' do
      input = <<~'EOS'
      :foo: bar

      Crossing the {foo}.

      :foo!:

      Belly up to the {foo}.
      EOS
      output = convert_string_to_embedded input
      assert_xpath '//p[text()="Crossing the bar."]', output, 1
      assert_xpath '//p[text()="Belly up to the bar."]', output, 0
    end

"#
        );

        let doc = Parser::default()
            .parse(":foo: bar\n\nCrossing the {foo}.\n\n:foo!:\n\nBelly up to the {foo}.");
        assert_xpath(&doc, "//p[text()=\"Crossing the bar.\"]", 1);
        assert_xpath(&doc, "//p[text()=\"Belly up to the bar.\"]", 0);
    }

    #[test]
    fn should_allow_compat_mode_to_be_set_and_unset_in_middle_of_document() {
        non_normative!(
            r#"
    test 'should allow compat-mode to be set and unset in middle of document' do
      input = <<~'EOS'
      :foo: bar

      [[paragraph-a]]
      `{foo}`

      :compat-mode!:

      [[paragraph-b]]
      `{foo}`

      :compat-mode:

      [[paragraph-c]]
      `{foo}`
      EOS

      result = convert_string_to_embedded input, attributes: { 'compat-mode' => '@' }
      assert_xpath '/*[@id="paragraph-a"]//code[text()="{foo}"]', result, 1
      assert_xpath '/*[@id="paragraph-b"]//code[text()="bar"]', result, 1
      assert_xpath '/*[@id="paragraph-c"]//code[text()="{foo}"]', result, 1
    end

"#
        );

        let doc = Parser::default()
            .with_intrinsic_attribute("compat-mode", "", ModificationContext::Anywhere)
            .parse(
                ":foo: bar\n\n[[paragraph-a]]\n`{foo}`\n\n:compat-mode!:\n\n[[paragraph-b]]\n`{foo}`\n\n:compat-mode:\n\n[[paragraph-c]]\n`{foo}`",
            );
        // Intentional divergence: this crate does not change backtick/monospace
        // attribute substitution based on compat-mode, so `{foo}` resolves to `bar` in
        // all three paragraphs (Asciidoctor keeps it literal where compat-mode
        // is on, i.e. paragraphs a and c).
        assert_xpath(&doc, "//*[@id=\"paragraph-a\"]//code[text()=\"bar\"]", 1);
        assert_xpath(&doc, "//*[@id=\"paragraph-b\"]//code[text()=\"bar\"]", 1);
        assert_xpath(&doc, "//*[@id=\"paragraph-c\"]//code[text()=\"bar\"]", 1);
    }

    #[test]
    fn does_not_disturb_attribute_looking_things_escaped_with_backslash() {
        verifies!(
            r#"
    test 'does not disturb attribute-looking things escaped with backslash' do
      html = convert_string(":foo: bar\nThis is a \\{foo} day.")
      assert_xpath '//p[text()="This is a {foo} day."]', html, 1
    end

"#
        );

        let doc = Parser::default().parse(":foo: bar\nThis is a \\{foo} day.");
        assert_xpath(&doc, "//p[text()=\"This is a {foo} day.\"]", 1);
    }

    #[test]
    fn does_not_disturb_attribute_looking_things_escaped_with_literals() {
        verifies!(
            r#"
    test 'does not disturb attribute-looking things escaped with literals' do
      html = convert_string(":foo: bar\nThis is a +++{foo}+++ day.")
      assert_xpath '//p[text()="This is a {foo} day."]', html, 1
    end

"#
        );

        let doc = Parser::default().parse(":foo: bar\nThis is a +++{foo}+++ day.");
        assert_xpath(&doc, "//p[text()=\"This is a {foo} day.\"]", 1);
    }

    #[test]
    fn does_not_substitute_attributes_inside_listing_blocks() {
        verifies!(
            r#"
    test 'does not substitute attributes inside listing blocks' do
      input = <<~'EOS'
      :forecast: snow

      ----
      puts 'The forecast for today is {forecast}'
      ----
      EOS
      output = convert_string(input)
      assert_match(/\{forecast\}/, output)
    end

"#
        );

        let doc = Parser::default()
            .parse(":forecast: snow\n\n----\nputs 'The forecast for today is {forecast}'\n----");
        assert_rendered_contains(&doc, "{forecast}");
    }

    #[test]
    fn does_not_substitute_attributes_inside_literal_blocks() {
        verifies!(
            r#"
    test 'does not substitute attributes inside literal blocks' do
      input = <<~'EOS'
      :foo: bar

      ....
      You insert the text {foo} to expand the value
      of the attribute named foo in your document.
      ....
      EOS
      output = convert_string(input)
      assert_match(/\{foo\}/, output)
    end

"#
        );

        let doc = Parser::default().parse(
            ":foo: bar\n\n....\nYou insert the text {foo} to expand the value\nof the attribute named foo in your document.\n....",
        );
        assert_rendered_contains(&doc, "{foo}");
    }

    // Tracked in #735.
    #[test]
    fn does_not_show_docdir_and_shows_relative_docfile_if_safe_mode_is_server_or_greater() {
        non_normative!(
            r#"
    test 'does not show docdir and shows relative docfile if safe mode is SERVER or greater' do
      input = <<~'EOS'
      * docdir: {docdir}
      * docfile: {docfile}
      EOS

      docdir = Dir.pwd
      docfile = File.join(docdir, 'sample.adoc')
      output = convert_string_to_embedded input, safe: Asciidoctor::SafeMode::SERVER, attributes: { 'docdir' => docdir, 'docfile' => docfile }
      assert_xpath '//li[1]/p[text()="docdir: "]', output, 1
      assert_xpath '//li[2]/p[text()="docfile: sample.adoc"]', output, 1
    end

"#
        );

        let doc = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_intrinsic_attribute("docdir", "/some/dir", ModificationContext::ApiOnly)
            .with_intrinsic_attribute(
                "docfile",
                "/some/dir/sample.adoc",
                ModificationContext::ApiOnly,
            )
            .parse("* docdir: {docdir}\n* docfile: {docfile}");
        // Divergence: this crate does not apply Asciidoctor's SERVER-safe-mode
        // masking of `docdir` (blanked) and `docfile` (relativized); the
        // API-provided values are substituted verbatim.
        assert_eq!(
            rendered_paragraphs(&doc),
            vec![
                "docdir: /some/dir".to_string(),
                "docfile: /some/dir/sample.adoc".to_string(),
            ]
        );
    }

    #[test]
    fn shows_absolute_docdir_and_docfile_paths_if_safe_mode_is_less_than_server() {
        verifies!(
            r#"
    test 'shows absolute docdir and docfile paths if safe mode is less than SERVER' do
      input = <<~'EOS'
      * docdir: {docdir}
      * docfile: {docfile}
      EOS

      docdir = Dir.pwd
      docfile = File.join(docdir, 'sample.adoc')
      output = convert_string_to_embedded input, safe: Asciidoctor::SafeMode::SAFE, attributes: { 'docdir' => docdir, 'docfile' => docfile }
      assert_xpath %(//li[1]/p[text()="docdir: #{docdir}"]), output, 1
      assert_xpath %(//li[2]/p[text()="docfile: #{docfile}"]), output, 1
    end

"#
        );

        let doc = Parser::default()
            .with_safe_mode(SafeMode::Safe)
            .with_intrinsic_attribute("docdir", "/some/dir", ModificationContext::ApiOnly)
            .with_intrinsic_attribute(
                "docfile",
                "/some/dir/sample.adoc",
                ModificationContext::ApiOnly,
            )
            .parse("* docdir: {docdir}\n* docfile: {docfile}");
        assert_eq!(
            rendered_paragraphs(&doc),
            vec![
                "docdir: /some/dir".to_string(),
                "docfile: /some/dir/sample.adoc".to_string(),
            ]
        );
    }

    #[test]
    fn assigns_attribute_defined_in_attribute_reference_with_set_prefix_and_value() {
        non_normative!(
            r#"
    test 'assigns attribute defined in attribute reference with set prefix and value' do
      input = '{set:foo:bar}{foo}'
      output = convert_string_to_embedded input
      assert_xpath '//p', output, 1
      assert_xpath '//p[text()="bar"]', output, 1
    end

"#
        );

        // Intentional divergence: this crate does not implement the `{set:…}` attribute
        // assignment reference, so it stays literal and `{foo}` is never
        // assigned (it remains an unresolved reference).
        let doc = Parser::default().parse("{set:foo:bar}{foo}");
        assert_xpath(&doc, "//p", 1);
        assert_xpath(&doc, "//p[text()=\"{set:foo:bar}{foo}\"]", 1);
    }

    #[test]
    fn assigns_attribute_defined_in_attribute_reference_with_set_prefix_and_no_value() {
        non_normative!(
            r#"
    test 'assigns attribute defined in attribute reference with set prefix and no value' do
      input = "{set:foo}\n{foo}yes"
      output = convert_string_to_embedded input
      assert_xpath '//p', output, 1
      assert_xpath '//p[normalize-space(text())="yes"]', output, 1
    end

"#
        );

        // Intentional divergence: `{set:…}` is not implemented (see the
        // `…_set_prefix_and_value` test), so it is left literal.
        let doc = Parser::default().parse("{set:foo}\n{foo}yes");
        assert_xpath(&doc, "//p", 1);
        assert_xpath(&doc, "//p[text()=\"{set:foo}\n{foo}yes\"]", 1);
    }

    #[test]
    fn assigns_attribute_defined_in_attribute_reference_with_set_prefix_and_empty_value() {
        non_normative!(
            r#"
    test 'assigns attribute defined in attribute reference with set prefix and empty value' do
      input = "{set:foo:}\n{foo}yes"
      output = convert_string_to_embedded input
      assert_xpath '//p', output, 1
      assert_xpath '//p[normalize-space(text())="yes"]', output, 1
    end

"#
        );

        // Intentional divergence: `{set:…}` is not implemented (see the
        // `…_set_prefix_and_value` test), so it is left literal.
        let doc = Parser::default().parse("{set:foo:}\n{foo}yes");
        assert_xpath(&doc, "//p", 1);
        assert_xpath(&doc, "//p[text()=\"{set:foo:}\n{foo}yes\"]", 1);
    }

    #[test]
    fn unassigns_attribute_defined_in_attribute_reference_with_set_prefix() {
        non_normative!(
            r#"
    test 'unassigns attribute defined in attribute reference with set prefix' do
      input = <<~'EOS'
      :attribute-missing: drop-line
      :foo:

      {set:foo!}
      {foo}yes
      EOS
      output = convert_string_to_embedded input
      assert_xpath '//p', output, 1
      assert_xpath '//p/child::text()', output, 0
      assert_message @logger, :INFO, 'dropping line containing reference to missing attribute: foo'
    end
"#
        );

        // Intentional divergence: `{set:foo!}` unassignment is not implemented; it
        // stays literal, and because `foo` is still set (to empty) `{foo}`
        // resolves to nothing, so no line is dropped.
        let doc =
            Parser::default().parse(":attribute-missing: drop-line\n:foo:\n\n{set:foo!}\n{foo}yes");
        assert_eq!(
            rendered_paragraphs(&doc),
            vec!["{set:foo!}\nyes".to_string()]
        );
    }
}

non_normative!(
    r#"
  end

  context "Intrinsic attributes" do
"#
);
mod intrinsic_attributes {
    use crate::tests::prelude::*;

    /// The rendered lines of the document's first raw (passthrough) block. The
    /// counter-increment tests wrap their references in a `[subs=attributes]`
    /// passthrough block, so the output is that block's substituted text split
    /// into lines (Asciidoctor's `output.lines.map(&:rstrip)`).
    fn raw_block_lines(doc: &crate::Document<'_>) -> Vec<String> {
        for b in doc.nested_blocks() {
            if let crate::blocks::Block::RawDelimited(raw) = b {
                return raw
                    .content()
                    .rendered()
                    .lines()
                    .map(str::to_string)
                    .collect();
            }
        }
        vec![]
    }

    non_normative!(
        r#"

"#
    );
    // Out of scope: iterates Ruby's `Asciidoctor::INTRINSIC_ATTRIBUTES` constant,
    // which this crate does not expose as an enumerable set.
    non_normative!(
        r#"
    test "substitute intrinsics" do
      Asciidoctor::INTRINSIC_ATTRIBUTES.each_pair do |key, value|
        html = convert_string("Look, a {#{key}} is here")
        # can't use Nokogiri because it interprets the HTML entities and we can't match them
        assert_match(/Look, a #{Regexp.escape(value)} is here/, html)
      end
    end

"#
    );

    #[test]
    fn don_t_escape_intrinsic_substitutions() {
        verifies!(
            r#"
    test "don't escape intrinsic substitutions" do
      html = convert_string('happy{nbsp}together')
      assert_match(/happy&#160;together/, html)
    end

"#
        );

        let doc = Parser::default().parse("happy{nbsp}together");
        assert!(
            rendered_paragraphs(&doc)
                .iter()
                .any(|p| p.contains("happy&#160;together"))
        );
    }

    #[test]
    fn escape_special_characters() {
        verifies!(
            r#"
    test "escape special characters" do
      html = convert_string('<node>&</node>')
      assert_match(/&lt;node&gt;&amp;&lt;\/node&gt;/, html)
    end

"#
        );

        let doc = Parser::default().parse("<node>&</node>");
        assert!(
            rendered_paragraphs(&doc)
                .iter()
                .any(|p| p.contains("&lt;node&gt;&amp;&lt;/node&gt;"))
        );
    }

    #[test]
    fn creates_counter() {
        verifies!(
            r#"
    test 'creates counter' do
      input = '{counter:mycounter}'

      doc = document_from_string input
      output = doc.convert
      assert_equal 1, doc.attributes['mycounter']
      assert_xpath '//p[text()="1"]', output, 1
    end

"#
        );

        let doc = Parser::default().parse("{counter:mycounter}");
        assert_eq!(
            doc.attribute_value("mycounter"),
            InterpretedValue::Value("1")
        );
        assert_xpath(&doc, "//p[text()=\"1\"]", 1);
    }

    #[test]
    fn creates_counter_silently() {
        verifies!(
            r#"
    test 'creates counter silently' do
      input = '{counter2:mycounter}'

      doc = document_from_string input
      output = doc.convert
      assert_equal 1, doc.attributes['mycounter']
      assert_xpath '//p[text()="1"]', output, 0
    end

"#
        );

        let doc = Parser::default().parse("{counter2:mycounter}");
        assert_eq!(
            doc.attribute_value("mycounter"),
            InterpretedValue::Value("1")
        );
        assert_xpath(&doc, "//p[text()=\"1\"]", 0);
    }

    #[test]
    fn creates_counter_with_numeric_seed_value() {
        verifies!(
            r#"
    test 'creates counter with numeric seed value' do
      input = '{counter2:mycounter:10}'

      doc = document_from_string input
      doc.convert
      assert_equal 10, doc.attributes['mycounter']
    end

"#
        );

        let doc = Parser::default().parse("{counter2:mycounter:10}");
        assert_eq!(
            doc.attribute_value("mycounter"),
            InterpretedValue::Value("10")
        );
    }

    #[test]
    fn creates_counter_with_character_seed_value() {
        verifies!(
            r#"
    test 'creates counter with character seed value' do
      input = '{counter2:mycounter:A}'

      doc = document_from_string input
      doc.convert
      assert_equal 'A', doc.attributes['mycounter']
    end

"#
        );

        let doc = Parser::default().parse("{counter2:mycounter:A}");
        assert_eq!(
            doc.attribute_value("mycounter"),
            InterpretedValue::Value("A")
        );
    }

    #[test]
    fn can_seed_counter_to_start_at_1() {
        verifies!(
            r#"
    test 'can seed counter to start at 1' do
      input = <<~'EOS'
      :mycounter: 0

      {counter:mycounter}
      EOS

      output = convert_string_to_embedded input
      assert_xpath '//p[text()="1"]', output, 1
    end

"#
        );

        let doc = Parser::default().parse(":mycounter: 0\n\n{counter:mycounter}");
        assert_xpath(&doc, "//p[text()=\"1\"]", 1);
    }

    #[test]
    fn can_seed_counter_to_start_at_a() {
        verifies!(
            r#"
    test 'can seed counter to start at A' do
      input = <<~'EOS'
      :mycounter: @

      {counter:mycounter}
      EOS

      output = convert_string_to_embedded input
      assert_xpath '//p[text()="A"]', output, 1
    end

"#
        );

        let doc = Parser::default().parse(":mycounter: @\n\n{counter:mycounter}");
        assert_xpath(&doc, "//p[text()=\"A\"]", 1);
    }

    #[test]
    fn increments_counter_with_positive_numeric_value() {
        verifies!(
            r#"
    test 'increments counter with positive numeric value' do
      input = <<~'EOS'
      [subs=attributes]
      ++++
      {counter:mycounter:1}
      {counter:mycounter}
      {counter:mycounter}
      {mycounter}
      ++++
      EOS

      doc = document_from_string input, standalone: false
      output = doc.convert
      assert_equal 3, doc.attributes['mycounter']
      assert_equal %w(1 2 3 3), output.lines.map {|l| l.rstrip }
    end

"#
        );

        let doc = Parser::default().parse(
            "[subs=attributes]\n++++\n{counter:mycounter:1}\n{counter:mycounter}\n{counter:mycounter}\n{mycounter}\n++++",
        );
        assert_eq!(
            doc.attribute_value("mycounter"),
            InterpretedValue::Value("3")
        );
        assert_eq!(raw_block_lines(&doc), vec!["1", "2", "3", "3"]);
    }

    #[test]
    fn increments_counter_with_negative_numeric_value() {
        verifies!(
            r#"
    test 'increments counter with negative numeric value' do
      input = <<~'EOS'
      [subs=attributes]
      ++++
      {counter:mycounter:-2}
      {counter:mycounter}
      {counter:mycounter}
      {mycounter}
      ++++
      EOS

      doc = document_from_string input, standalone: false
      output = doc.convert
      assert_equal 0, doc.attributes['mycounter']
      assert_equal %w(-2 -1 0 0), output.lines.map {|l| l.rstrip }
    end

"#
        );

        let doc = Parser::default().parse(
            "[subs=attributes]\n++++\n{counter:mycounter:-2}\n{counter:mycounter}\n{counter:mycounter}\n{mycounter}\n++++",
        );
        assert_eq!(
            doc.attribute_value("mycounter"),
            InterpretedValue::Value("0")
        );
        assert_eq!(raw_block_lines(&doc), vec!["-2", "-1", "0", "0"]);
    }

    #[test]
    fn increments_counter_with_ascii_character_value() {
        verifies!(
            r#"
    test 'increments counter with ASCII character value' do
      input = <<~'EOS'
      [subs=attributes]
      ++++
      {counter:mycounter:A}
      {counter:mycounter}
      {counter:mycounter}
      {mycounter}
      ++++
      EOS

      output = convert_string_to_embedded input
      assert_equal %w(A B C C), output.lines.map {|l| l.rstrip }
    end

"#
        );

        let doc = Parser::default().parse(
            "[subs=attributes]\n++++\n{counter:mycounter:A}\n{counter:mycounter}\n{counter:mycounter}\n{mycounter}\n++++",
        );
        assert_eq!(raw_block_lines(&doc), vec!["A", "B", "C", "C"]);
    }

    #[test]
    fn increments_counter_with_non_ascii_character_value() {
        verifies!(
            r#"
    test 'increments counter with non-ASCII character value' do
      input = <<~'EOS'
      [subs=attributes]
      ++++
      {counter:mycounter:é}
      {counter:mycounter}
      {counter:mycounter}
      {mycounter}
      ++++
      EOS

      output = convert_string_to_embedded input
      assert_equal %w(é ê ë ë), output.lines.map {|l| l.rstrip }
    end

"#
        );

        let doc = Parser::default().parse(
            "[subs=attributes]\n++++\n{counter:mycounter:é}\n{counter:mycounter}\n{counter:mycounter}\n{mycounter}\n++++",
        );
        assert_eq!(raw_block_lines(&doc), vec!["é", "ê", "ë", "ë"]);
    }

    #[test]
    fn increments_counter_with_emoji_character_value() {
        verifies!(
            r#"
    test 'increments counter with emoji character value' do
      input = <<~'EOS'
      [subs=attributes]
      ++++
      {counter:smiley:😋}
      {counter:smiley}
      {counter:smiley}
      {smiley}
      ++++
      EOS

      output = convert_string_to_embedded input
      assert_equal %w(😋 😌 😍 😍), output.lines.map {|l| l.rstrip }
    end

"#
        );

        let doc = Parser::default().parse(
            "[subs=attributes]\n++++\n{counter:smiley:😋}\n{counter:smiley}\n{counter:smiley}\n{smiley}\n++++",
        );
        assert_eq!(raw_block_lines(&doc), vec!["😋", "😌", "😍", "😍"]);
    }

    #[test]
    fn increments_counter_with_multi_character_value() {
        verifies!(
            r#"
    test 'increments counter with multi-character value' do
      input = <<~'EOS'
      [subs=attributes]
      ++++
      {counter:math:1x}
      {counter:math}
      {counter:math}
      {math}
      ++++
      EOS

      output = convert_string_to_embedded input
      assert_equal %w(1x 1y 1z 1z), output.lines.map {|l| l.rstrip }
    end

"#
        );

        let doc = Parser::default().parse(
            "[subs=attributes]\n++++\n{counter:math:1x}\n{counter:math}\n{counter:math}\n{math}\n++++",
        );
        assert_eq!(raw_block_lines(&doc), vec!["1x", "1y", "1z", "1z"]);
    }

    #[test]
    fn counter_uses_0_as_seed_value_if_seed_attribute_is_nil() {
        verifies!(
            r#"
    test 'counter uses 0 as seed value if seed attribute is nil' do
      input = <<~'EOS'
      :mycounter:

      {counter:mycounter}

      {mycounter}
      EOS

      doc = document_from_string input
      output = doc.convert standalone: false
      assert_equal 1, doc.attributes['mycounter']
      assert_xpath '//p[text()="1"]', output, 2
    end

"#
        );

        let doc = Parser::default().parse(":mycounter:\n\n{counter:mycounter}\n\n{mycounter}");
        assert_eq!(
            doc.attribute_value("mycounter"),
            InterpretedValue::Value("1")
        );
        assert_xpath(&doc, "//p[text()=\"1\"]", 2);
    }

    #[test]
    fn counter_value_can_be_reset_by_attribute_entry() {
        verifies!(
            r#"
    test 'counter value can be reset by attribute entry' do
      input = <<~'EOS'
      :mycounter:

      before: {counter:mycounter} {counter:mycounter} {counter:mycounter}

      :mycounter!:

      after: {counter:mycounter}
      EOS

      doc = document_from_string input
      output = doc.convert standalone: false
      assert_equal 1, doc.attributes['mycounter']
      assert_xpath '//p[text()="before: 1 2 3"]', output, 1
      assert_xpath '//p[text()="after: 1"]', output, 1
    end

"#
        );

        let doc = Parser::default().parse(
            ":mycounter:\n\nbefore: {counter:mycounter} {counter:mycounter} {counter:mycounter}\n\n:mycounter!:\n\nafter: {counter:mycounter}",
        );
        assert_eq!(
            doc.attribute_value("mycounter"),
            InterpretedValue::Value("1")
        );
        assert_xpath(&doc, "//p[text()=\"before: 1 2 3\"]", 1);
        assert_xpath(&doc, "//p[text()=\"after: 1\"]", 1);
    }

    #[test]
    fn counter_value_can_be_advanced_by_attribute_entry() {
        verifies!(
            r#"
    test 'counter value can be advanced by attribute entry' do
      input = <<~'EOS'
      before: {counter:mycounter}

      :mycounter: 10

      after: {counter:mycounter}
      EOS

      doc = document_from_string input
      output = doc.convert standalone: false
      assert_equal 11, doc.attributes['mycounter']
      assert_xpath '//p[text()="before: 1"]', output, 1
      assert_xpath '//p[text()="after: 11"]', output, 1
    end

"#
        );

        let doc = Parser::default()
            .parse("before: {counter:mycounter}\n\n:mycounter: 10\n\nafter: {counter:mycounter}");
        assert_eq!(
            doc.attribute_value("mycounter"),
            InterpretedValue::Value("11")
        );
        assert_xpath(&doc, "//p[text()=\"before: 1\"]", 1);
        assert_xpath(&doc, "//p[text()=\"after: 11\"]", 1);
    }

    #[test]
    fn nested_document_should_use_counter_from_parent_document() {
        non_normative!(
            r#"
    test 'nested document should use counter from parent document' do
      input = <<~'EOS'
      .Title for Foo
      image::foo.jpg[]

      [cols="2*a"]
      |===
      |
      .Title for Bar
      image::bar.jpg[]

      |
      .Title for Baz
      image::baz.jpg[]
      |===

      .Title for Qux
      image::qux.jpg[]
      EOS

      output = convert_string_to_embedded input
      assert_xpath '//div[@class="title"]', output, 4
      assert_xpath '//div[@class="title"][text() = "Figure 1. Title for Foo"]', output, 1
      assert_xpath '//div[@class="title"][text() = "Figure 2. Title for Bar"]', output, 1
      assert_xpath '//div[@class="title"][text() = "Figure 3. Title for Baz"]', output, 1
      assert_xpath '//div[@class="title"][text() = "Figure 4. Title for Qux"]', output, 1
    end

"#
        );

        let doc = Parser::default().parse(
            ".Title for Foo\nimage::foo.jpg[]\n\n[cols=\"2*a\"]\n|===\n|\n.Title for Bar\nimage::bar.jpg[]\n\n|\n.Title for Baz\nimage::baz.jpg[]\n|===\n\n.Title for Qux\nimage::qux.jpg[]",
        );
        // This crate exposes a block's caption/number via the `caption()` /
        // `number()` accessors rather than as a `<div class="title">` DOM node,
        // so the shared-counter behavior is verified through those accessors.
        // The figure counter is shared with the nested table-cell documents: the
        // two top-level images are numbered 1 and 4, leaving 2 and 3 for the
        // images inside the table cells.
        let blocks: Vec<_> = doc.nested_blocks().collect();
        assert_eq!(blocks[0].title(), Some("Title for Foo"));
        assert_eq!(blocks[0].caption(), Some("Figure 1. "));
        assert_eq!(blocks[2].title(), Some("Title for Qux"));
        assert_eq!(blocks[2].caption(), Some("Figure 4. "));
    }

    // Tracked in #725.
    #[test]
    fn should_not_allow_counter_to_modify_locked_attribute() {
        non_normative!(
            r#"
    test 'should not allow counter to modify locked attribute' do
      input = <<~'EOS'
      {counter:foo:ignored} is not {foo}
      EOS

      output = convert_string_to_embedded input, attributes: { 'foo' => 'bar' }
      assert_xpath '//p[text()="bas is not bar"]', output, 1
    end

"#
        );

        // Divergence: Asciidoctor does not let a counter modify a locked
        // (API-set) attribute, so `{foo}` stays `bar`. This crate's counter
        // reads and increments the current value (`bar` → `bas`) and also stores
        // it, so both references render `bas`.
        let doc = Parser::default()
            .with_intrinsic_attribute("foo", "bar", ModificationContext::ApiOnly)
            .parse("{counter:foo:ignored} is not {foo}");
        assert_xpath(&doc, "//p[text()=\"bas is not bas\"]", 1);
    }

    // Tracked in #725.
    #[test]
    fn should_not_allow_counter2_to_modify_locked_attribute() {
        non_normative!(
            r#"
    test 'should not allow counter2 to modify locked attribute' do
      input = <<~'EOS'
      {counter2:foo:ignored}{foo}
      EOS

      output = convert_string_to_embedded input, attributes: { 'foo' => 'bar' }
      assert_xpath '//p[text()="bar"]', output, 1
    end

"#
        );

        // Divergence: as with `counter`, this crate's `counter2` modifies the
        // locked `foo` (Asciidoctor leaves it `bar`), so `{foo}` renders `bas`.
        let doc = Parser::default()
            .with_intrinsic_attribute("foo", "bar", ModificationContext::ApiOnly)
            .parse("{counter2:foo:ignored}{foo}");
        assert_xpath(&doc, "//p[text()=\"bas\"]", 1);
    }

    // Tracked in #725.
    #[test]
    fn should_not_allow_counter_to_modify_built_in_locked_attribute() {
        non_normative!(
            r#"
    test 'should not allow counter to modify built-in locked attribute' do
      input = <<~'EOS'
      {counter:max-include-depth:128} is one more than {max-include-depth}
      EOS

      doc = document_from_string input, standalone: false
      output = doc.convert
      assert_xpath '//p[text()="65 is one more than 64"]', output, 1
      assert_equal 64, doc.attributes['max-include-depth']
    end

"#
        );

        // Divergence: Asciidoctor does not let a counter modify the built-in
        // locked `max-include-depth`, so it stays 64. This crate increments and
        // stores it, so both the counter output and the later reference read 65.
        let doc = Parser::default()
            .parse("{counter:max-include-depth:128} is one more than {max-include-depth}");
        assert_xpath(&doc, "//p[text()=\"65 is one more than 65\"]", 1);
        assert_eq!(
            doc.attribute_value("max-include-depth"),
            InterpretedValue::Value("65")
        );
    }

    // Tracked in #725.
    #[test]
    fn should_not_allow_counter2_to_modify_built_in_locked_attribute() {
        non_normative!(
            r#"
    test 'should not allow counter2 to modify built-in locked attribute' do
      input = <<~'EOS'
      {counter2:max-include-depth:128}{max-include-depth}
      EOS

      doc = document_from_string input, standalone: false
      output = doc.convert
      assert_xpath '//p[text()="64"]', output, 1
      assert_equal 64, doc.attributes['max-include-depth']
    end
"#
        );

        // Divergence: as with `counter`, this crate's `counter2` modifies the
        // built-in locked `max-include-depth` (Asciidoctor leaves it 64).
        let doc = Parser::default().parse("{counter2:max-include-depth:128}{max-include-depth}");
        assert_xpath(&doc, "//p[text()=\"65\"]", 1);
        assert_eq!(
            doc.attribute_value("max-include-depth"),
            InterpretedValue::Value("65")
        );
    }
}

non_normative!(
    r#"
  end

  context 'Block attributes' do
"#
);
mod block_attributes {
    use crate::tests::prelude::*;

    /// The document's first top-level block.
    fn first_block<'src>(doc: &'src crate::Document) -> &'src crate::blocks::Block<'src> {
        doc.nested_blocks().next().expect("expected a block")
    }

    /// The document's first top-level block, as a [`QuoteBlock`].
    ///
    /// [`QuoteBlock`]: crate::blocks::QuoteBlock
    fn first_quote<'src>(doc: &'src crate::Document) -> &'src crate::blocks::QuoteBlock<'src> {
        match first_block(doc) {
            crate::blocks::Block::Quote(q) => q,
            b => panic!("expected a quote block, got: {b:?}"),
        }
    }

    /// Value of a block's named attribute, if present.
    fn named<'src>(block: &'src crate::blocks::Block<'src>, name: &str) -> Option<&'src str> {
        block
            .attrlist()
            .and_then(|al| al.named_attribute(name))
            .map(|a| a.value())
    }

    #[test]
    fn parses_attribute_names_as_name_token() {
        verifies!(
            r#"
    test 'parses attribute names as name token' do
      input = <<~'EOS'
      [normal,foo="bar",_foo="_bar",foo1="bar1",foo-foo="bar-bar",foo.foo="bar.bar"]
      content
      EOS

      block = block_from_string input
      assert_equal 'bar', block.attr('foo')
      assert_equal '_bar', block.attr('_foo')
      assert_equal 'bar1', block.attr('foo1')
      assert_equal 'bar-bar', block.attr('foo-foo')
      assert_equal 'bar.bar', block.attr('foo.foo')
    end

"#
        );

        let doc = Parser::default().parse(
            "[normal,foo=\"bar\",_foo=\"_bar\",foo1=\"bar1\",foo-foo=\"bar-bar\",foo.foo=\"bar.bar\"]\ncontent",
        );
        let block = first_block(&doc);
        assert_eq!(named(block, "foo"), Some("bar"));
        assert_eq!(named(block, "_foo"), Some("_bar"));
        assert_eq!(named(block, "foo1"), Some("bar1"));
        assert_eq!(named(block, "foo-foo"), Some("bar-bar"));
        assert_eq!(named(block, "foo.foo"), Some("bar.bar"));
    }

    #[test]
    fn positional_attributes_assigned_to_block() {
        verifies!(
            r#"
    test 'positional attributes assigned to block' do
      input = <<~'EOS'
      [quote, author, source]
      ____
      A famous quote.
      ____
      EOS
      doc = document_from_string(input)
      qb = doc.blocks.first
      assert_equal 'quote', qb.style
      assert_equal 'author', qb.attr('attribution')
      assert_equal 'author', qb.attr(:attribution)
      assert_equal 'author', qb.attributes['attribution']
      assert_equal 'source', qb.attributes['citetitle']
    end

"#
        );

        let doc = Parser::default().parse("[quote, author, source]\n____\nA famous quote.\n____");
        let qb = first_quote(&doc);
        assert_eq!(qb.declared_style(), Some("quote"));
        assert_eq!(qb.attribution(), Some("author"));
        assert_eq!(qb.citetitle(), Some("source"));
    }

    #[test]
    fn normal_substitutions_are_performed_on_single_quoted_positional_attribute() {
        verifies!(
            r#"
    test 'normal substitutions are performed on single-quoted positional attribute' do
      input = <<~'EOS'
      [quote, author, 'http://wikipedia.org[source]']
      ____
      A famous quote.
      ____
      EOS
      doc = document_from_string(input)
      qb = doc.blocks.first
      assert_equal 'quote', qb.style
      assert_equal 'author', qb.attr('attribution')
      assert_equal 'author', qb.attr(:attribution)
      assert_equal 'author', qb.attributes['attribution']
      assert_equal '<a href="http://wikipedia.org">source</a>', qb.attributes['citetitle']
    end

"#
        );

        let doc = Parser::default()
            .parse("[quote, author, 'http://wikipedia.org[source]']\n____\nA famous quote.\n____");
        let qb = first_quote(&doc);
        assert_eq!(qb.declared_style(), Some("quote"));
        assert_eq!(qb.attribution(), Some("author"));
        assert_eq!(
            qb.citetitle(),
            Some("<a href=\"http://wikipedia.org\">source</a>")
        );
    }

    #[test]
    fn normal_substitutions_are_performed_on_single_quoted_named_attribute() {
        verifies!(
            r#"
    test 'normal substitutions are performed on single-quoted named attribute' do
      input = <<~'EOS'
      [quote, author, citetitle='http://wikipedia.org[source]']
      ____
      A famous quote.
      ____
      EOS
      doc = document_from_string(input)
      qb = doc.blocks.first
      assert_equal 'quote', qb.style
      assert_equal 'author', qb.attr('attribution')
      assert_equal 'author', qb.attr(:attribution)
      assert_equal 'author', qb.attributes['attribution']
      assert_equal '<a href="http://wikipedia.org">source</a>', qb.attributes['citetitle']
    end

"#
        );

        let doc = Parser::default().parse(
            "[quote, author, citetitle='http://wikipedia.org[source]']\n____\nA famous quote.\n____",
        );
        let qb = first_quote(&doc);
        assert_eq!(qb.declared_style(), Some("quote"));
        assert_eq!(qb.attribution(), Some("author"));
        assert_eq!(
            qb.citetitle(),
            Some("<a href=\"http://wikipedia.org\">source</a>")
        );
    }

    #[test]
    fn normal_substitutions_are_performed_once_on_single_quoted_named_title_attribute() {
        verifies!(
            r#"
    test 'normal substitutions are performed once on single-quoted named title attribute' do
      input = <<~'EOS'
      [title='*title*']
      content
      EOS
      output = convert_string_to_embedded input
      assert_xpath '//*[@class="title"]/strong[text()="title"]', output, 1
    end

"#
        );

        // The block title is exposed via the `title()` accessor rather than as a
        // `<div class="title">` DOM node; the single-quoted title has normal
        // substitutions applied once (`*title*` → `<strong>title</strong>`).
        let doc = Parser::default().parse("[title='*title*']\ncontent");
        assert_eq!(first_block(&doc).title(), Some("<strong>title</strong>"));
    }

    #[test]
    fn attribute_list_may_not_begin_with_space() {
        verifies!(
            r#"
    test 'attribute list may not begin with space' do
      input = <<~'EOS'
      [ quote]
      ____
      A famous quote.
      ____
      EOS

      doc = document_from_string input
      b1 = doc.blocks.first
      assert_equal ['[ quote]'], b1.lines
    end

"#
        );

        // `[ quote]` (leading space) is not a valid attribute list, so the line
        // is kept as literal paragraph text.
        let doc = Parser::default().parse("[ quote]\n____\nA famous quote.\n____");
        assert_rendered_contains(&doc, "[ quote]");
    }

    #[test]
    fn attribute_list_may_begin_with_comma() {
        verifies!(
            r#"
    test 'attribute list may begin with comma' do
      input = <<~'EOS'
      [, author, source]
      ____
      A famous quote.
      ____
      EOS

      doc = document_from_string input
      qb = doc.blocks.first
      assert_equal 'quote', qb.style
      assert_equal 'author', qb.attributes['attribution']
      assert_equal 'source', qb.attributes['citetitle']
    end

"#
        );

        // The block is a quote (delimited by `____`) even though the style
        // positional is empty; `declared_style()` reflects only an explicitly
        // declared bare style, so it is `None` here — the positional
        // attribution/citetitle are what this test cares about.
        let doc = Parser::default().parse("[, author, source]\n____\nA famous quote.\n____");
        let qb = first_quote(&doc);
        assert_eq!(qb.attribution(), Some("author"));
        assert_eq!(qb.citetitle(), Some("source"));
    }

    #[test]
    fn first_attribute_in_list_may_be_double_quoted() {
        verifies!(
            r#"
    test 'first attribute in list may be double quoted' do
      input = <<~'EOS'
      ["quote", "author", "source", role="famous"]
      ____
      A famous quote.
      ____
      EOS

      doc = document_from_string input
      qb = doc.blocks.first
      assert_equal 'quote', qb.style
      assert_equal 'author', qb.attributes['attribution']
      assert_equal 'source', qb.attributes['citetitle']
      assert_equal 'famous', qb.attributes['role']
    end

"#
        );

        // A quoted style token (`"quote"`) still produces a quote block, but
        // `declared_style()` reports `None` (it reflects only a bare declared
        // style); the positional attribution/citetitle and role are verified.
        let doc = Parser::default().parse(
            "[\"quote\", \"author\", \"source\", role=\"famous\"]\n____\nA famous quote.\n____",
        );
        let qb = first_quote(&doc);
        assert_eq!(qb.attribution(), Some("author"));
        assert_eq!(qb.citetitle(), Some("source"));
        assert_eq!(qb.roles(), vec!["famous"]);
    }

    #[test]
    fn first_attribute_in_list_may_be_single_quoted() {
        verifies!(
            r#"
    test 'first attribute in list may be single quoted' do
      input = <<~'EOS'
      ['quote', 'author', 'source', role='famous']
      ____
      A famous quote.
      ____
      EOS

      doc = document_from_string input
      qb = doc.blocks.first
      assert_equal 'quote', qb.style
      assert_equal 'author', qb.attributes['attribution']
      assert_equal 'source', qb.attributes['citetitle']
      assert_equal 'famous', qb.attributes['role']
    end

"#
        );

        // As with the double-quoted form, a single-quoted style token produces a
        // quote block whose `declared_style()` is `None`.
        let doc = Parser::default()
            .parse("['quote', 'author', 'source', role='famous']\n____\nA famous quote.\n____");
        let qb = first_quote(&doc);
        assert_eq!(qb.attribution(), Some("author"));
        assert_eq!(qb.citetitle(), Some("source"));
        assert_eq!(qb.roles(), vec!["famous"]);
    }

    #[test]
    fn attribute_with_value_none_without_quotes_is_ignored() {
        verifies!(
            r#"
    test 'attribute with value None without quotes is ignored' do
      input = <<~'EOS'
      [id=None]
      paragraph
      EOS

      doc = document_from_string input
      para = doc.blocks.first
      refute para.attributes.key?('id')
    end

"#
        );

        let doc = Parser::default().parse("[id=None]\nparagraph");
        assert_eq!(first_block(&doc).id(), None);
    }

    #[test]
    fn role_returns_true_if_role_is_assigned() {
        verifies!(
            r#"
    test 'role? returns true if role is assigned' do
      input = <<~'EOS'
      [role="lead"]
      A paragraph
      EOS

      doc = document_from_string input
      p = doc.blocks.first
      assert p.role?
    end

"#
        );

        // Ruby's `role?` (no argument) maps to "has any role" here.
        let doc = Parser::default().parse("[role=\"lead\"]\nA paragraph");
        assert!(!first_block(&doc).roles().is_empty());
    }

    #[test]
    fn role_does_not_return_true_if_role_attribute_is_set_on_document() {
        verifies!(
            r#"
    test 'role? does not return true if role attribute is set on document' do
      input = <<~'EOS'
      :role: lead

      A paragraph
      EOS

      doc = document_from_string input
      p = doc.blocks.first
      refute p.role?
    end

"#
        );

        let doc = Parser::default().parse(":role: lead\n\nA paragraph");
        assert!(first_block(&doc).roles().is_empty());
    }

    #[test]
    fn role_can_check_for_exact_role_name_match() {
        verifies!(
            r#"
    test 'role? can check for exact role name match' do
      input = <<~'EOS'
      [role="lead"]
      A paragraph
      EOS

      doc = document_from_string input
      p = doc.blocks.first
      assert p.role?('lead')
      p2 = doc.blocks.last
      refute p2.role?('final')
    end

"#
        );

        // Ruby's `role?(name)` matches when the whole `role` attribute equals
        // `name`; here that is `roles() == [name]`.
        let doc = Parser::default().parse("[role=\"lead\"]\nA paragraph");
        let p = first_block(&doc);
        assert_eq!(p.roles(), vec!["lead"]);
        assert_ne!(p.roles(), vec!["final"]);
    }

    #[test]
    fn has_role_can_check_for_presence_of_role_name() {
        verifies!(
            r#"
    test 'has_role? can check for presence of role name' do
      input = <<~'EOS'
      [role="lead abstract"]
      A paragraph
      EOS

      doc = document_from_string input
      p = doc.blocks.first
      refute p.role?('lead')
      assert p.has_role?('lead')
    end

"#
        );

        // `role?('lead')` is false (the whole role string is "lead abstract"),
        // but `has_role?('lead')` (membership) is true.
        let doc = Parser::default().parse("[role=\"lead abstract\"]\nA paragraph");
        let p = first_block(&doc);
        assert_ne!(p.roles().join(" "), "lead");
        assert!(p.roles().contains(&"lead"));
    }

    #[test]
    fn has_role_does_not_look_for_role_defined_as_document_attribute() {
        verifies!(
            r#"
    test 'has_role? does not look for role defined as document attribute' do
      input = <<~'EOS'
      :role: lead abstract

      A paragraph
      EOS

      doc = document_from_string input
      p = doc.blocks.first
      refute p.has_role?('lead')
    end

"#
        );

        let doc = Parser::default().parse(":role: lead abstract\n\nA paragraph");
        assert!(!first_block(&doc).roles().contains(&"lead"));
    }

    #[test]
    fn roles_returns_array_of_role_names() {
        verifies!(
            r#"
    test 'roles returns array of role names' do
      input = <<~'EOS'
      [role="story lead"]
      A paragraph
      EOS

      doc = document_from_string input
      p = doc.blocks.first
      assert_equal ['story', 'lead'], p.roles
    end

"#
        );

        let doc = Parser::default().parse("[role=\"story lead\"]\nA paragraph");
        assert_eq!(first_block(&doc).roles(), vec!["story", "lead"]);
    }

    #[test]
    fn roles_returns_empty_array_if_role_attribute_is_not_set() {
        verifies!(
            r#"
    test 'roles returns empty array if role attribute is not set' do
      input = 'a paragraph'

      doc = document_from_string input
      p = doc.blocks.first
      assert_equal [], p.roles
    end

"#
        );

        let doc = Parser::default().parse("a paragraph");
        assert!(first_block(&doc).roles().is_empty());
    }

    #[test]
    fn roles_does_not_return_value_of_roles_document_attribute() {
        verifies!(
            r#"
    test 'roles does not return value of roles document attribute' do
      input = <<~'EOS'
      :role: story lead

      A paragraph
      EOS

      doc = document_from_string input
      p = doc.blocks.first
      assert_equal [], p.roles
    end

"#
        );

        let doc = Parser::default().parse(":role: story lead\n\nA paragraph");
        assert!(first_block(&doc).roles().is_empty());
    }

    // Out of scope: exercises Ruby's mutable `role=` setter on a node, which this
    // crate does not expose.
    non_normative!(
        r#"
    test 'roles= sets the role attribute on the node' do
      doc = document_from_string 'a paragraph'
      p = doc.blocks.first
      p.role = 'foobar'
      assert_equal 'foobar', (p.attr 'role')
    end

"#
    );

    // Out of scope: exercises Ruby's mutable `role=` setter on a node, which this
    // crate does not expose.
    non_normative!(
        r#"
    test 'roles= coerces array value to a space-separated string' do
      doc = document_from_string 'a paragraph'
      p = doc.blocks.first
      p.role = %w(foo bar)
      assert_equal 'foo bar', (p.attr 'role')
    end

"#
    );

    #[test]
    fn attribute_substitutions_are_performed_on_attribute_list_before_parsing_attributes() {
        verifies!(
            r#"
    test "Attribute substitutions are performed on attribute list before parsing attributes" do
      input = <<~'EOS'
      :lead: role="lead"

      [{lead}]
      A paragraph
      EOS
      doc = document_from_string(input)
      para = doc.blocks.first
      assert_equal 'lead', para.attributes['role']
    end

"#
        );

        let doc = Parser::default().parse(":lead: role=\"lead\"\n\n[{lead}]\nA paragraph");
        assert_eq!(first_block(&doc).roles(), vec!["lead"]);
    }

    #[test]
    fn id_role_and_options_attributes_can_be_specified_on_block_style_using_shorthand_syntax() {
        verifies!(
            r#"
    test 'id, role and options attributes can be specified on block style using shorthand syntax' do
      input = <<~'EOS'
      [literal#first.lead%step]
      A literal paragraph.
      EOS
      doc = document_from_string(input)
      para = doc.blocks.first
      assert_equal :literal, para.context
      assert_equal 'first', para.attributes['id']
      assert_equal 'lead', para.attributes['role']
      assert para.attributes.key?('step-option')
      refute para.attributes.key?('options')
    end

"#
        );

        let doc = Parser::default().parse("[literal#first.lead%step]\nA literal paragraph.");
        let p = first_block(&doc);
        assert_eq!(p.declared_style(), Some("literal"));
        assert_eq!(p.id(), Some("first"));
        assert_eq!(p.roles(), vec!["lead"]);
        assert!(p.has_option("step"));
        assert_eq!(named(p, "options"), None);
    }

    #[test]
    fn id_role_and_options_attributes_can_be_specified_using_shorthand_syntax_on_block_style_using_multiple_block_attribute_lines()
     {
        verifies!(
            r#"
    test 'id, role and options attributes can be specified using shorthand syntax on block style using multiple block attribute lines' do
      input = <<~'EOS'
      [literal]
      [#first]
      [.lead]
      [%step]
      A literal paragraph.
      EOS
      doc = document_from_string(input)
      para = doc.blocks.first
      assert_equal :literal, para.context
      assert_equal 'first', para.attributes['id']
      assert_equal 'lead', para.attributes['role']
      assert para.attributes.key?('step-option')
      refute para.attributes.key?('options')
    end

"#
        );

        let doc =
            Parser::default().parse("[literal]\n[#first]\n[.lead]\n[%step]\nA literal paragraph.");
        let p = first_block(&doc);
        assert_eq!(p.declared_style(), Some("literal"));
        assert_eq!(p.id(), Some("first"));
        assert_eq!(p.roles(), vec!["lead"]);
        assert!(p.has_option("step"));
        assert_eq!(named(p, "options"), None);
    }

    #[test]
    fn multiple_roles_and_options_can_be_specified_in_block_style_using_shorthand_syntax() {
        verifies!(
            r#"
    test 'multiple roles and options can be specified in block style using shorthand syntax' do
      input = <<~'EOS'
      [.role1%option1.role2%option2]
      Text
      EOS

      doc = document_from_string input
      para = doc.blocks.first
      assert_equal 'role1 role2', para.attributes['role']
      assert para.attributes.key?('option1-option')
      assert para.attributes.key?('option2-option')
      refute para.attributes.key?('options')
    end

"#
        );

        let doc = Parser::default().parse("[.role1%option1.role2%option2]\nText");
        let p = first_block(&doc);
        assert_eq!(p.roles(), vec!["role1", "role2"]);
        assert!(p.has_option("option1"));
        assert!(p.has_option("option2"));
        assert_eq!(named(p, "options"), None);
    }

    #[test]
    fn options_specified_using_shorthand_syntax_on_block_style_across_multiple_lines_should_be_additive()
     {
        verifies!(
            r#"
    test 'options specified using shorthand syntax on block style across multiple lines should be additive' do
      input = <<~'EOS'
      [%option1]
      [%option2]
      Text
      EOS

      doc = document_from_string input
      para = doc.blocks.first
      assert para.attributes.key?('option1-option')
      assert para.attributes.key?('option2-option')
      refute para.attributes.key?('options')
    end

"#
        );

        let doc = Parser::default().parse("[%option1]\n[%option2]\nText");
        let p = first_block(&doc);
        assert!(p.has_option("option1"));
        assert!(p.has_option("option2"));
        assert_eq!(named(p, "options"), None);
    }

    #[test]
    fn roles_specified_using_shorthand_syntax_on_block_style_across_multiple_lines_should_be_additive()
     {
        verifies!(
            r#"
    test 'roles specified using shorthand syntax on block style across multiple lines should be additive' do
      input = <<~'EOS'
      [.role1]
      [.role2.role3]
      Text
      EOS

      doc = document_from_string input
      para = doc.blocks.first
      assert_equal 'role1 role2 role3', para.attributes['role']
    end

"#
        );

        let doc = Parser::default().parse("[.role1]\n[.role2.role3]\nText");
        assert_eq!(first_block(&doc).roles(), vec!["role1", "role2", "role3"]);
    }

    // Tracked in #732.
    #[test]
    fn setting_a_role_using_the_role_attribute_replaces_any_existing_roles() {
        non_normative!(
            r#"
    test 'setting a role using the role attribute replaces any existing roles' do
      input = <<~'EOS'
      [.role1]
      [role=role2]
      [.role3]
      Text
      EOS

      doc = document_from_string input
      para = doc.blocks.first
      assert_equal 'role2 role3', para.attributes['role']
    end

"#
        );

        // Divergence: in Asciidoctor a formal `role=` entry replaces any roles
        // already set by shorthand `.role`, so `.role1` is cleared and the
        // result is `role2 role3`. This crate treats every role source as
        // additive, so all three accumulate (in encounter order).
        let doc = Parser::default().parse("[.role1]\n[role=role2]\n[.role3]\nText");
        assert_eq!(first_block(&doc).roles(), vec!["role1", "role3", "role2"]);
    }

    #[test]
    fn setting_a_role_using_the_shorthand_syntax_on_block_style_should_not_clear_the_id() {
        verifies!(
            r#"
    test 'setting a role using the shorthand syntax on block style should not clear the ID' do
      input = <<~'EOS'
      [#id]
      [.role]
      Text
      EOS

      doc = document_from_string input
      para = doc.blocks.first
      assert_equal 'id', para.id
      assert_equal 'role', para.role
    end

"#
        );

        let doc = Parser::default().parse("[#id]\n[.role]\nText");
        let p = first_block(&doc);
        assert_eq!(p.id(), Some("id"));
        assert_eq!(p.roles(), vec!["role"]);
    }

    // Out of scope: exercises Ruby's mutable `add_role` API, which this crate does
    // not expose.
    non_normative!(
        r#"
    test 'a role can be added using add_role when the node has no roles' do
      input = 'A normal paragraph'
      doc = document_from_string(input)
      para = doc.blocks.first
      res = para.add_role 'role1'
      assert res
      assert_equal 'role1', para.attributes['role']
      assert para.has_role? 'role1'
    end

"#
    );

    // Out of scope: exercises Ruby's mutable `add_role` API, which this crate does
    // not expose.
    non_normative!(
        r#"
    test 'a role can be added using add_role when the node already has a role' do
      input = <<~'EOS'
      [.role1]
      A normal paragraph
      EOS
      doc = document_from_string(input)
      para = doc.blocks.first
      res = para.add_role 'role2'
      assert res
      assert_equal 'role1 role2', para.attributes['role']
      assert para.has_role? 'role1'
      assert para.has_role? 'role2'
    end

"#
    );

    // Out of scope: exercises Ruby's mutable `add_role` API, which this crate does
    // not expose.
    non_normative!(
        r#"
    test 'a role is not added using add_role if the node already has that role' do
      input = <<~'EOS'
      [.role1]
      A normal paragraph
      EOS
      doc = document_from_string(input)
      para = doc.blocks.first
      res = para.add_role 'role1'
      refute res
      assert_equal 'role1', para.attributes['role']
      assert para.has_role? 'role1'
    end

"#
    );

    // Out of scope: exercises Ruby's mutable `remove_role` API, which this crate
    // does not expose.
    non_normative!(
        r#"
    test 'an existing role can be removed using remove_role' do
      input = <<~'EOS'
      [.role1.role2]
      A normal paragraph
      EOS
      doc = document_from_string(input)
      para = doc.blocks.first
      res = para.remove_role 'role1'
      assert res
      assert_equal 'role2', para.attributes['role']
      assert para.has_role? 'role2'
      refute para.has_role?('role1')
    end

"#
    );

    // Out of scope: exercises Ruby's mutable `remove_role` API, which this crate
    // does not expose.
    non_normative!(
        r#"
    test 'roles are removed when last role is removed using remove_role' do
      input = <<~'EOS'
      [.role1]
      A normal paragraph
      EOS
      doc = document_from_string(input)
      para = doc.blocks.first
      res = para.remove_role 'role1'
      assert res
      refute para.role?
      assert_nil para.attributes['role']
      refute para.has_role? 'role1'
    end

"#
    );

    // Out of scope: exercises Ruby's mutable `remove_role` API, which this crate
    // does not expose.
    non_normative!(
        r#"
    test 'roles are not changed when a non-existent role is removed using remove_role' do
      input = <<~'EOS'
      [.role1]
      A normal paragraph
      EOS
      doc = document_from_string(input)
      para = doc.blocks.first
      res = para.remove_role 'role2'
      refute res
      assert_equal 'role1', para.attributes['role']
      assert para.has_role? 'role1'
      refute para.has_role?('role2')
    end

"#
    );

    // Out of scope: exercises Ruby's mutable `remove_role` API, which this crate
    // does not expose.
    non_normative!(
        r#"
    test 'roles are not changed when using remove_role if the node has no roles' do
      input = 'A normal paragraph'
      doc = document_from_string(input)
      para = doc.blocks.first
      res = para.remove_role 'role1'
      refute res
      assert_nil para.attributes['role']
      refute para.has_role?('role1')
    end

"#
    );

    #[test]
    fn option_can_be_specified_in_first_position_of_block_style_using_shorthand_syntax() {
        verifies!(
            r#"
    test 'option can be specified in first position of block style using shorthand syntax' do
      input = <<~'EOS'
      [%interactive]
      - [x] checked
      EOS

      doc = document_from_string input
      list = doc.blocks.first
      assert list.attributes.key? 'interactive-option'
      refute list.attributes.key? 'options'
    end

"#
        );

        let doc = Parser::default().parse("[%interactive]\n- [x] checked");
        let list = first_block(&doc);
        assert!(list.has_option("interactive"));
        assert_eq!(named(list, "options"), None);
    }

    #[test]
    fn id_and_role_attributes_can_be_specified_on_section_style_using_shorthand_syntax() {
        verifies!(
            r#"
    test 'id and role attributes can be specified on section style using shorthand syntax' do
      input = <<~'EOS'
      [dedication#dedication.small]
      == Section
      Content.
      EOS
      output = convert_string_to_embedded input
      assert_xpath '/div[@class="sect1 small"]', output, 1
      assert_xpath '/div[@class="sect1 small"]/h2[@id="dedication"]', output, 1
    end

"#
        );

        let doc = Parser::default().parse("[dedication#dedication.small]\n== Section\nContent.");
        assert_css(&doc, "div.sect1.small", 1);
        assert_css(&doc, "div.sect1.small h2#dedication", 1);
    }

    // Out of scope: targets the DocBook backend, which is out of scope for this
    // crate.
    non_normative!(
        r#"
    test 'id attribute specified using shorthand syntax should not create a special section' do
      input = <<~'EOS'
      [#idname]
      == Section

      content
      EOS

      doc = document_from_string input, backend: 'docbook'
      section = doc.blocks[0]
      refute_nil section
      assert_equal :section, section.context
      refute section.special
      output = doc.convert
      assert_css 'article:root > section', output, 1
      assert_css 'article:root > section[xml|id="idname"]', output, 1
    end

"#
    );

    #[test]
    fn block_attributes_are_additive() {
        verifies!(
            r#"
    test "Block attributes are additive" do
      input = <<~'EOS'
      [id='foo']
      [role='lead']
      A paragraph.
      EOS
      doc = document_from_string(input)
      para = doc.blocks.first
      assert_equal 'foo', para.id
      assert_equal 'lead', para.attributes['role']
    end

"#
        );

        let doc = Parser::default().parse("[id='foo']\n[role='lead']\nA paragraph.");
        let p = first_block(&doc);
        assert_eq!(p.id(), Some("foo"));
        assert_eq!(p.roles(), vec!["lead"]);
    }

    // Tracked in #733.
    #[test]
    fn last_wins_for_id_attribute() {
        non_normative!(
            r#"
    test "Last wins for id attribute" do
      input = <<~'EOS'
      [[bar]]
      [[foo]]
      == Section

      paragraph

      [[baz]]
      [id='coolio']
      === Section
      EOS
      doc = document_from_string(input)
      sec = doc.first_section
      assert_equal 'foo', sec.id
      subsec = sec.blocks.last
      assert_equal 'coolio', subsec.id
    end

"#
        );

        // Divergence: Asciidoctor keeps the *last* of consecutive block anchors,
        // so the section id resolves to `foo`. This crate keeps the *first*
        // anchor, so the id is `bar`, and it does not collapse the stacked
        // anchors/attributes onto the following section the same way.
        let doc = Parser::default().parse(
            "[[bar]]\n[[foo]]\n== Section\n\nparagraph\n\n[[baz]]\n[id='coolio']\n=== Section",
        );
        assert_eq!(first_block(&doc).id(), Some("bar"));
    }

    // Tracked in #733.
    #[test]
    fn trailing_block_attributes_transfer_to_the_following_section() {
        non_normative!(
            r#"
    test "trailing block attributes transfer to the following section" do
      input = <<~'EOS'
      [[one]]

      == Section One

      paragraph

      [[sub]]
      // try to mess this up!

      === Sub-section

      paragraph

      [role='classy']

      ////
      block comment
      ////

      == Section Two

      content
      EOS
      doc = document_from_string(input)
      section_one = doc.blocks.first
      assert_equal 'one', section_one.id
      subsection = section_one.blocks.last
      assert_equal 'sub', subsection.id
      section_two = doc.blocks.last
      assert_equal 'classy', section_two.attr(:role)
    end
"#
        );

        // Divergence: Asciidoctor transfers a trailing block anchor / attribute
        // list (separated from the heading by a comment or block) to the
        // *following* section, so the sub-section gets id `sub` and Section Two
        // gets role `classy`. This crate attaches them to standalone blocks
        // instead, so the sub-section keeps its auto-generated id and Section Two
        // has no role. The top-level section ids that this crate does assign are
        // verified here.
        let doc = Parser::default().parse(
            "[[one]]\n\n== Section One\n\nparagraph\n\n[[sub]]\n// try to mess this up!\n\n=== Sub-section\n\nparagraph\n\n[role='classy']\n\n////\nblock comment\n////\n\n== Section Two\n\ncontent",
        );
        let blocks: Vec<_> = doc.nested_blocks().collect();
        assert_eq!(blocks[0].id(), Some("one"));
        assert_eq!(blocks.last().unwrap().id(), Some("_section_two"));
        assert!(blocks.last().unwrap().roles().is_empty());
    }
}

non_normative!(
    r#"
  end

end
"#
);
