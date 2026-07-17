// Adapted from Asciidoctor's helpers test suite, found in
// https://github.com/asciidoctor/asciidoctor/blob/main/test/helpers_test.rb.
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
//! Port of Asciidoctor's `helpers_test.rb`.
//!
//! This suite exercises Asciidoctor's `Asciidoctor::Helpers` utility module and
//! the `Asciidoctor::UriSniffRx` constant — internal Ruby helpers for URI
//! encoding, path/URI classification, and reflective class resolution. None of
//! these is part of `asciidoc-parser`'s public surface: the crate is a parser,
//! and per its README, URI/URL construction and backend conversion are the
//! responsibility of downstream renderer crates.
//!
//! As the user who requested this port expected, the file is therefore **almost
//! entirely non-normative** for this crate. The one exception is Asciidoctor's
//! `Helpers.extname?`, whose logic this crate reproduces (privately, as
//! `has_extname`) inside the inline renderer's icon macro; that behavior is
//! observable through rendering, so it is ported as a `verifies!` block.
//!
//! Disposition, context by context:
//!
//! * **URI Encoding** (`Helpers.encode_uri_component`) — the crate does have a
//!   private `encode_uri_component`, but it is reachable only through the
//!   `mailto:` subject/body rendering path, where the input is first mangled by
//!   the special-characters substitution (`&` → `&amp;`) and attribute-list
//!   whitespace trimming. The helper's isolated contract asserted by these
//!   tests cannot be reproduced through any public API, so both cases are
//!   `non_normative!`. Exercising that path *did* surface one genuine
//!   discrepancy: the crate's `CGI_ESCAPE_SET` does not encode `*`, so
//!   `encode_uri_component` yields a literal `*` where Asciidoctor (via
//!   `CGI.escape`) yields `%2A`. (It correctly leaves `~` unescaped, matching
//!   Ruby 2.5+.) This only affects `*` inside a `mailto:` subject/body.
//! * **URIs and Paths** — `rootname` has no analog in this crate (extension
//!   stripping for includes/paths is downstream); `UriSniffRx` exists here as
//!   the private `URI_SNIFF` regex but is used only to strip a scheme for
//!   `hide-uri-scheme`, not to *detect* arbitrary-scheme URIs (autolink
//!   detection hardcodes `https?|file|ftp|irc`), so its detection semantics are
//!   not mirrored as observable behavior; `uriish?` is JRuby classloader-path
//!   handling, out of scope. Only `extname?` is verifiable (see above).
//! * **Type Resolution** (`Helpers.class_for_name` / `resolve_class`) — Ruby
//!   reflection used to load converter/extension classes by name. This crate
//!   has no dynamic class registry; entirely out of scope.

use crate::tests::sdd::*;

track_file!("ref/asciidoctor/test/helpers_test.rb");

// `context 'URI Encoding'` — `Helpers.encode_uri_component`. The crate's
// private analog is reachable only through `mailto:` subject/body rendering,
// where the input is pre-transformed by other substitutions, so the isolated
// helper contract these tests assert is not observable. (This path also
// surfaced the `*` → `%2A` gap noted in the module docs.)
non_normative!(
    r#"
# frozen_string_literal: true
require_relative 'test_helper'

context 'Helpers' do
  context 'URI Encoding' do
    test 'should URI encode non-word characters generally' do
      given = ' !*/%&?\\='
      expect = '%20%21%2A%2F%25%26%3F%5C%3D'
      assert_equal expect, (Asciidoctor::Helpers.encode_uri_component given)
    end

    test 'should not URI encode select non-word characters' do
      # NOTE Ruby 2.5 and up stopped encoding ~
      given = '-.'
      expect = given
      assert_equal expect, (Asciidoctor::Helpers.encode_uri_component given)
    end
  end

"#
);

// `context 'URIs and Paths'` — `rootname` (no analog here), `UriSniffRx`
// (present as `URI_SNIFF`, but used only for `hide-uri-scheme` stripping, not
// URI detection), and `uriish?` (JRuby classloader paths, out of scope) are all
// non-normative. `extname?` is the lone verifiable behavior and appears as a
// `verifies!` block immediately below this one, in source order.
non_normative!(
    r#"
  context 'URIs and Paths' do
    test 'rootname should return file name without extension' do
      assert_equal 'main', Asciidoctor::Helpers.rootname('main.adoc')
      assert_equal 'docs/main', Asciidoctor::Helpers.rootname('docs/main.adoc')
    end

    test 'rootname should file name if it has no extension' do
      assert_equal 'main', Asciidoctor::Helpers.rootname('main')
      assert_equal 'docs/main', Asciidoctor::Helpers.rootname('docs/main')
    end

    test 'rootname should ignore dot not in last segment' do
      assert_equal 'include.d/main', Asciidoctor::Helpers.rootname('include.d/main')
      assert_equal 'include.d/main', Asciidoctor::Helpers.rootname('include.d/main.adoc')
    end

"#
);

// `Helpers.extname?` reports whether the final path segment carries a file
// extension. This crate reproduces exactly that logic in the inline renderer's
// private `has_extname`, which the icon macro consults to decide whether to
// append the `icontype` (default `png`) to a bare icon name. Rendering
// `icon:<name>[]` with `icons=image` therefore makes each of the four Ruby
// assertions observable: a name the helper considers to *have* an extension is
// used verbatim, while one it considers extension-less gets `.png` appended.
#[test]
fn extname_should_return_whether_path_contains_an_extname() {
    verifies!(
        r#"
    test 'extname? should return whether path contains an extname' do
      assert Asciidoctor::Helpers.extname?('document.adoc')
      assert Asciidoctor::Helpers.extname?('path/to/document.adoc')
      assert_nil Asciidoctor::Helpers.extname?('basename')
      refute Asciidoctor::Helpers.extname?('include.d/basename')
    end

"#
    );

    use crate::{Span, blocks::Block, tests::prelude::*};

    fn icon_src(name: &str) -> String {
        let mut parser = Parser::default().with_intrinsic_attribute(
            "icons",
            "image",
            ModificationContext::Anywhere,
        );

        let src = format!("icon:{name}[]");
        let maw = Block::parse(Span::new(&src), &mut parser);

        let Block::Simple(block) = maw.item.unwrap().item else {
            panic!("expected a simple block");
        };

        block.content().rendered().to_string()
    }

    // `assert Asciidoctor::Helpers.extname?('document.adoc')`: has an extension,
    // so the name is used verbatim (no `.png` appended).
    assert_eq!(
        icon_src("document.adoc"),
        r#"<span class="icon"><img src="./images/icons/document.adoc" alt="document"></span>"#
    );

    // `assert Asciidoctor::Helpers.extname?('path/to/document.adoc')`: the
    // extension lives in the final segment, so again used verbatim.
    assert_eq!(
        icon_src("path/to/document.adoc"),
        r#"<span class="icon"><img src="./images/icons/path/to/document.adoc" alt="document"></span>"#
    );

    // `assert_nil Asciidoctor::Helpers.extname?('basename')`: no extension, so
    // `icontype` (`png`) is appended.
    assert_eq!(
        icon_src("basename"),
        r#"<span class="icon"><img src="./images/icons/basename.png" alt="basename"></span>"#
    );

    // `refute Asciidoctor::Helpers.extname?('include.d/basename')`: the only dot
    // is in a non-final segment, so the final segment has no extension and
    // `png` is appended.
    assert_eq!(
        icon_src("include.d/basename"),
        r#"<span class="icon"><img src="./images/icons/include.d/basename.png" alt="basename"></span>"#
    );
}

// Remainder of `context 'URIs and Paths'` — `UriSniffRx` detection and the
// JRuby-specific `uriish?` classloader case. `URI_SNIFF` exists in this crate
// but only for scheme stripping (`hide-uri-scheme`); the parser does not use a
// general URI-sniffing regex to *detect* arbitrary-scheme URIs, so none of
// these detection assertions maps to observable parser behavior.
non_normative!(
    r#"
    test 'UriSniffRx should detect URIs' do
      assert_match Asciidoctor::UriSniffRx, 'http://example.com'
      assert_match Asciidoctor::UriSniffRx, 'https://example.com'
      assert_match Asciidoctor::UriSniffRx, 'data:image/gif;base64,R0lGODlhAQABAIAAAAUEBAAAACwAAAAAAQABAAACAkQBADs='
    end

    test 'UriSniffRx should not detect an absolute Windows path as a URI' do
      refute_match Asciidoctor::UriSniffRx, 'c:/sample.adoc'
      refute_match Asciidoctor::UriSniffRx, 'c:\\sample.adoc'
    end

    test 'uriish? should not detect a classloader path as a URI on JRuby' do
      input = 'uri:classloader:/sample.png'
      assert_match Asciidoctor::UriSniffRx, input
      if jruby?
        refute Asciidoctor::Helpers.uriish? input
      else
        assert Asciidoctor::Helpers.uriish? input
      end
    end

    test 'UriSniffRx should not detect URI that does not start on first line' do
      refute_match Asciidoctor::UriSniffRx, %(text\nhttps://example.org)
    end
  end

"#
);

// `context 'Type Resolution'` — `Helpers.class_for_name` / `resolve_class`
// resolve a Ruby class from a name string (used to load converters and
// extensions by class name). This crate has no dynamic class registry and does
// not model Ruby reflection; the entire context is out of scope.
non_normative!(
    r#"
  context 'Type Resolution' do
    test 'should get class for top-level class name' do
      clazz = Asciidoctor::Helpers.class_for_name 'String'
      refute_nil clazz
      assert_equal String, clazz
    end

    test 'should get class for class name in module' do
      clazz = Asciidoctor::Helpers.class_for_name 'Asciidoctor::Document'
      refute_nil clazz
      assert_equal Asciidoctor::Document, clazz
    end

    test 'should get class for class name resolved from root' do
      clazz = Asciidoctor::Helpers.class_for_name '::Asciidoctor::Document'
      refute_nil clazz
      assert_equal Asciidoctor::Document, clazz
    end

    test 'should raise exception if cannot find class for name' do
      begin
        Asciidoctor::Helpers.class_for_name 'InvalidModule::InvalidClass'
        flunk 'Expecting RuntimeError to be raised'
      rescue NameError => e
        assert_match %r/^Could not resolve class for name: InvalidModule::InvalidClass$/, e.message
      end
    end

    test 'should raise exception if constant name is invalid' do
      begin
        Asciidoctor::Helpers.class_for_name 'foobar'
        flunk 'Expecting RuntimeError to be raised'
      rescue NameError => e
        assert_match %r/^Could not resolve class for name: foobar$/, e.message
      end
    end

    test 'should raise exception if class not found in scope' do
      begin
        Asciidoctor::Helpers.class_for_name 'Asciidoctor::Extensions::String'
        flunk 'Expecting RuntimeError to be raised'
      rescue NameError => e
        assert_match %r/^Could not resolve class for name: Asciidoctor::Extensions::String$/, e.message
      end
    end

    test 'should raise exception if name resolves to module' do
      begin
        Asciidoctor::Helpers.class_for_name 'Asciidoctor::Extensions'
        flunk 'Expecting RuntimeError to be raised'
      rescue NameError => e
        assert_match %r/^Could not resolve class for name: Asciidoctor::Extensions$/, e.message
      end
    end

    test 'should resolve class if class is given' do
      clazz = Asciidoctor::Helpers.resolve_class Asciidoctor::Document
      refute_nil clazz
      assert_equal Asciidoctor::Document, clazz
    end

    test 'should resolve class if class from string' do
      clazz = Asciidoctor::Helpers.resolve_class 'Asciidoctor::Document'
      refute_nil clazz
      assert_equal Asciidoctor::Document, clazz
    end

    test 'should not resolve class if not in scope' do
      begin
        Asciidoctor::Helpers.resolve_class 'Asciidoctor::Extensions::String'
        flunk 'Expecting RuntimeError to be raised'
      rescue NameError => e
        assert_match %r/^Could not resolve class for name: Asciidoctor::Extensions::String$/, e.message
      end
    end
  end
end
"#
);
