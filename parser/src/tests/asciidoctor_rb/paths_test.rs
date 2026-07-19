// Adapted from Asciidoctor's paths test suite, found in
// https://github.com/asciidoctor/asciidoctor/blob/main/test/paths_test.rb.
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
//! Port of Asciidoctor's `paths_test.rb`.
//!
//! These are unit tests of Asciidoctor's `PathResolver` class. This crate's
//! [`PathResolver`](crate::parser::PathResolver) is the direct analog, so each
//! ported test drives it directly.
//!
//! **Web Paths** (in scope). Every test in this context exercises
//! [`PathResolver::web_path`], which this crate implements. Ruby's
//! `web_path(target, start = nil)` maps to `web_path(target, start)` where a
//! Ruby `nil` (and an omitted argument) becomes `None` and a Ruby string
//! becomes `Some(..)`. Ruby posixifies a `nil` target to `''`, so
//! `web_path(nil, start)` is ported as `web_path("", Some(start))`.
//!
//! Two behavior gaps this port surfaced were fixed in `web_path` rather than
//! left non-normative, so every Web Paths test verifies:
//!
//! * An empty `start` string is now treated like `None` (Ruby's
//!   `start.nil_or_empty?`), so a relative target is returned as-is instead of
//!   being rooted at `/`.
//! * A trailing slash in the joined path no longer survives as a spurious empty
//!   final segment: Ruby's `String#split` drops trailing empty fields, and
//!   `partition_path` now matches.
//!
//! **System Paths** (out of scope). Every test in this context exercises
//! `PathResolver#system_path` / `#relative_path` (or
//! `Document#normalize_system_path`), none of which this crate ports:
//! `system_path` is the filesystem-resolution and safe-mode-jail routine, and
//! this crate performs no filesystem I/O — `include::` and asset resolution are
//! delegated to the client via `IncludeFileHandler` (see the note on
//! [`PathResolver`](crate::parser::PathResolver)). The whole context is
//! therefore `non_normative!`.

use crate::{parser::PathResolver, tests::sdd::*};

track_file!("ref/asciidoctor/test/paths_test.rb");

non_normative!(
    r#"
# frozen_string_literal: true
require_relative 'test_helper'

context 'Path Resolver' do
  context 'Web Paths' do
    def setup
      @resolver = Asciidoctor::PathResolver.new
    end

"#
);

#[test]
fn target_with_absolute_path() {
    verifies!(
        r#"
    test 'target with absolute path' do
      assert_equal '/images', @resolver.web_path('/images')
      assert_equal '/images', @resolver.web_path('/images', '')
      assert_equal '/images', @resolver.web_path('/images', nil)
    end

"#
    );

    let resolver = PathResolver::default();

    assert_eq!(resolver.web_path("/images", None), "/images");
    assert_eq!(resolver.web_path("/images", Some("")), "/images");
    assert_eq!(resolver.web_path("/images", None), "/images");
}

#[test]
fn target_with_relative_path() {
    verifies!(
        r#"
    test 'target with relative path' do
      assert_equal 'images', @resolver.web_path('images')
      assert_equal 'images', @resolver.web_path('images', '')
      assert_equal 'images', @resolver.web_path('images', nil)
    end

"#
    );

    let resolver = PathResolver::default();

    assert_eq!(resolver.web_path("images", None), "images");
    assert_eq!(resolver.web_path("images", Some("")), "images");
    assert_eq!(resolver.web_path("images", None), "images");
}

#[test]
fn target_with_hidden_relative_path() {
    verifies!(
        r#"
    test 'target with hidden relative path' do
      assert_equal '.images', @resolver.web_path('.images')
      assert_equal '.images', @resolver.web_path('.images', '')
      assert_equal '.images', @resolver.web_path('.images', nil)
    end

"#
    );

    let resolver = PathResolver::default();

    assert_eq!(resolver.web_path(".images", None), ".images");
    assert_eq!(resolver.web_path(".images", Some("")), ".images");
    assert_eq!(resolver.web_path(".images", None), ".images");
}

#[test]
fn target_with_path_relative_to_current_directory() {
    verifies!(
        r#"
    test 'target with path relative to current directory' do
      assert_equal './images', @resolver.web_path('./images')
      assert_equal './images', @resolver.web_path('./images', '')
      assert_equal './images', @resolver.web_path('./images', nil)
    end

"#
    );

    let resolver = PathResolver::default();

    assert_eq!(resolver.web_path("./images", None), "./images");
    assert_eq!(resolver.web_path("./images", Some("")), "./images");
    assert_eq!(resolver.web_path("./images", None), "./images");
}

#[test]
fn target_with_absolute_path_ignores_start_path() {
    verifies!(
        r#"
    test 'target with absolute path ignores start path' do
      assert_equal '/images', @resolver.web_path('/images', 'foo')
      assert_equal '/images', @resolver.web_path('/images', '/foo')
      assert_equal '/images', @resolver.web_path('/images', './foo')
    end

"#
    );

    let resolver = PathResolver::default();

    assert_eq!(resolver.web_path("/images", Some("foo")), "/images");
    assert_eq!(resolver.web_path("/images", Some("/foo")), "/images");
    assert_eq!(resolver.web_path("/images", Some("./foo")), "/images");
}

#[test]
fn target_with_relative_path_appended_to_start_path() {
    verifies!(
        r#"
    test 'target with relative path appended to start path' do
      assert_equal 'assets/images', @resolver.web_path('images', 'assets')
      assert_equal '/assets/images', @resolver.web_path('images', '/assets')
      #assert_equal '/assets/images/tiger.png', @resolver.web_path('tiger.png', '/assets//images')
      assert_equal './assets/images', @resolver.web_path('images', './assets')
      assert_equal '/theme.css', @resolver.web_path('theme.css', '/')
      assert_equal '/css/theme.css', @resolver.web_path('theme.css', '/css/')
    end

"#
    );

    let resolver = PathResolver::default();

    assert_eq!(resolver.web_path("images", Some("assets")), "assets/images");
    assert_eq!(
        resolver.web_path("images", Some("/assets")),
        "/assets/images"
    );
    assert_eq!(
        resolver.web_path("images", Some("./assets")),
        "./assets/images"
    );
    assert_eq!(resolver.web_path("theme.css", Some("/")), "/theme.css");
    assert_eq!(
        resolver.web_path("theme.css", Some("/css/")),
        "/css/theme.css"
    );
}

#[test]
fn target_with_path_relative_to_current_directory_appended_to_start_path() {
    verifies!(
        r#"
    test 'target with path relative to current directory appended to start path' do
      assert_equal 'assets/images', @resolver.web_path('./images', 'assets')
      assert_equal '/assets/images', @resolver.web_path('./images', '/assets')
      assert_equal './assets/images', @resolver.web_path('./images', './assets')
    end

"#
    );

    let resolver = PathResolver::default();

    assert_eq!(
        resolver.web_path("./images", Some("assets")),
        "assets/images"
    );
    assert_eq!(
        resolver.web_path("./images", Some("/assets")),
        "/assets/images"
    );
    assert_eq!(
        resolver.web_path("./images", Some("./assets")),
        "./assets/images"
    );
}

#[test]
fn target_with_relative_path_appended_to_url_start_path() {
    verifies!(
        r#"
    test 'target with relative path appended to url start path' do
      assert_equal 'http://www.example.com/assets/images', @resolver.web_path('images', 'http://www.example.com/assets')
    end

"#
    );

    let resolver = PathResolver::default();

    assert_eq!(
        resolver.web_path("images", Some("http://www.example.com/assets")),
        "http://www.example.com/assets/images"
    );
}

// Two upstream tests are commented out (they would only apply if `web_path`
// grew the ability to detect and preserve a target URI); nothing to port.
non_normative!(
    r#"
    # enable if we want to allow web_path to detect and preserve a target URI
    #test 'target with file url appended to relative path' do
    #  assert_equal 'file:///home/username/styles/asciidoctor.css', @resolver.web_path('file:///home/username/styles/asciidoctor.css', '.')
    #end

    # enable if we want to allow web_path to detect and preserve a target URI
    #test 'target with http url appended to relative path' do
    #  assert_equal 'http://example.com/asciidoctor.css', @resolver.web_path('http://example.com/asciidoctor.css', '.')
    #end

"#
);

#[test]
fn normalize_target() {
    verifies!(
        r#"
    test 'normalize target' do
      assert_equal '../images', @resolver.web_path('../images/../images')
    end

"#
    );

    let resolver = PathResolver::default();

    assert_eq!(resolver.web_path("../images/../images", None), "../images");
}

#[test]
fn append_target_to_start_path_and_normalize() {
    verifies!(
        r#"
    test 'append target to start path and normalize' do
      assert_equal '../images', @resolver.web_path('../images/../images', '../images')
      assert_equal '../../images', @resolver.web_path('../images', '..')
    end

"#
    );

    let resolver = PathResolver::default();

    assert_eq!(
        resolver.web_path("../images/../images", Some("../images")),
        "../images"
    );
    assert_eq!(resolver.web_path("../images", Some("..")), "../../images");
}

#[test]
fn normalize_parent_directory_that_follows_root() {
    verifies!(
        r#"
    test 'normalize parent directory that follows root' do
      assert_equal '/tiger.png', @resolver.web_path('/../tiger.png')
      assert_equal '/tiger.png', @resolver.web_path('/../../tiger.png')
    end

"#
    );

    let resolver = PathResolver::default();

    assert_eq!(resolver.web_path("/../tiger.png", None), "/tiger.png");
    assert_eq!(resolver.web_path("/../../tiger.png", None), "/tiger.png");
}

#[test]
fn uses_start_when_target_is_empty() {
    verifies!(
        r#"
    test 'uses start when target is empty' do
      assert_equal 'assets/images', @resolver.web_path('', 'assets/images')
      assert_equal 'assets/images', @resolver.web_path(nil, 'assets/images')
    end

"#
    );

    let resolver = PathResolver::default();

    assert_eq!(
        resolver.web_path("", Some("assets/images")),
        "assets/images"
    );

    // Ruby posixifies a `nil` target to `''`, so `web_path(nil, ..)` behaves
    // identically to `web_path('', ..)`.
    assert_eq!(
        resolver.web_path("", Some("assets/images")),
        "assets/images"
    );
}

#[test]
fn posixifies_windows_paths() {
    verifies!(
        r#"
    test 'posixifies windows paths' do
      @resolver.file_separator = '\\'
      assert_equal '/images', @resolver.web_path('\\images')
      assert_equal '../images', @resolver.web_path('..\\images')
      assert_equal '/images', @resolver.web_path('\\..\\images')
      assert_equal 'assets/images', @resolver.web_path('assets\\images')
      assert_equal '../assets/images', @resolver.web_path('assets\\images', '..\\images\\..')
    end

"#
    );

    let resolver = PathResolver {
        file_separator: '\\',
    };

    assert_eq!(resolver.web_path("\\images", None), "/images");
    assert_eq!(resolver.web_path("..\\images", None), "../images");
    assert_eq!(resolver.web_path("\\..\\images", None), "/images");
    assert_eq!(resolver.web_path("assets\\images", None), "assets/images");
    assert_eq!(
        resolver.web_path("assets\\images", Some("..\\images\\..")),
        "../assets/images"
    );
}

#[test]
fn url_encode_spaces_in_path() {
    verifies!(
        r#"
    test 'URL encode spaces in path' do
      assert_equal 'assets%20and%20stuff/lots%20of%20images', @resolver.web_path('lots of images', 'assets and stuff')
    end
"#
    );

    let resolver = PathResolver::default();

    assert_eq!(
        resolver.web_path("lots of images", Some("assets and stuff")),
        "assets%20and%20stuff/lots%20of%20images"
    );
}

// The entire `System Paths` context exercises `system_path` / `relative_path`
// / `normalize_system_path`, which this crate does not port (no filesystem
// I/O; see the note on `PathResolver`). Line 100's `end` closes `Web Paths`.
non_normative!(
    r##"
  end

  context 'System Paths' do
    JAIL = '/home/doctor/docs'
    default_logger = Asciidoctor::LoggerManager.logger

    def setup
      @resolver = Asciidoctor::PathResolver.new
      @logger = (Asciidoctor::LoggerManager.logger = Asciidoctor::MemoryLogger.new)
    end

    teardown do
      Asciidoctor::LoggerManager.logger = default_logger
    end

    test 'raises security error if jail is not an absolute path' do
      begin
        @resolver.system_path('images/tiger.png', '/etc', 'foo')
        flunk 'Expecting SecurityError to be raised'
      rescue SecurityError
      end
    end

    #test 'raises security error if jail is not a canonical path' do
    #  begin
    #    @resolver.system_path('images/tiger.png', '/etc', %(#{JAIL}/../foo))
    #    flunk 'Expecting SecurityError to be raised'
    #  rescue SecurityError
    #  end
    #end

    test 'prevents access to paths outside of jail' do
      result = @resolver.system_path '../../../../../css', %(#{JAIL}/assets/stylesheets), JAIL
      assert_equal %(#{JAIL}/css), result
      assert_message @logger, :WARN, 'path has illegal reference to ancestor of jail; recovering automatically'

      @logger.clear
      result = @resolver.system_path '/../../../../../css', %(#{JAIL}/assets/stylesheets), JAIL
      assert_equal %(#{JAIL}/css), result
      assert_message @logger, :WARN, 'path is outside of jail; recovering automatically'

      @logger.clear
      result = @resolver.system_path '../../../css', '../../..', JAIL
      assert_equal %(#{JAIL}/css), result
      assert_message @logger, :WARN, 'path has illegal reference to ancestor of jail; recovering automatically'
    end

    test 'throws exception for illegal path access if recover is false' do
      begin
        @resolver.system_path('../../../../../css', "#{JAIL}/assets/stylesheets", JAIL, recover: false)
        flunk 'Expecting SecurityError to be raised'
      rescue SecurityError
      end
    end

    test 'resolves start path if target is empty' do
      assert_equal "#{JAIL}/assets/stylesheets", @resolver.system_path('', "#{JAIL}/assets/stylesheets", JAIL)
      assert_equal "#{JAIL}/assets/stylesheets", @resolver.system_path(nil, "#{JAIL}/assets/stylesheets", JAIL)
    end

    test 'expands parent references in start path if target is empty' do
      assert_equal "#{JAIL}/stylesheets", @resolver.system_path('', "#{JAIL}/assets/../stylesheets", JAIL)
    end

    test 'expands parent references in start path if target is not empty' do
      assert_equal "#{JAIL}/stylesheets/site.css", @resolver.system_path('site.css', "#{JAIL}/assets/../stylesheets", JAIL)
    end

    test 'resolves start path if target is dot' do
      assert_equal "#{JAIL}/assets/stylesheets", @resolver.system_path('.', "#{JAIL}/assets/stylesheets", JAIL)
      assert_equal "#{JAIL}/assets/stylesheets", @resolver.system_path('./', "#{JAIL}/assets/stylesheets", JAIL)
    end

    test 'treats absolute target outside of jail as relative when jail is specified' do
      result = @resolver.system_path '/', "#{JAIL}/assets/stylesheets", JAIL
      assert_equal JAIL, result
      assert_message @logger, :WARN, 'path is outside of jail; recovering automatically'

      @logger.clear
      result = @resolver.system_path '/foo', "#{JAIL}/assets/stylesheets", JAIL
      assert_equal "#{JAIL}/foo", result
      assert_message @logger, :WARN, 'path is outside of jail; recovering automatically'

      @logger.clear
      result = @resolver.system_path '/../foo', "#{JAIL}/assets/stylesheets", JAIL
      assert_equal "#{JAIL}/foo", result
      assert_message @logger, :WARN, 'path is outside of jail; recovering automatically'

      @logger.clear
      @resolver.file_separator = '\\'
      result = @resolver.system_path 'baz.adoc', 'C:/foo', 'C:/bar'
      assert_equal 'C:/bar/baz.adoc', result
      assert_message @logger, :WARN, 'path is outside of jail; recovering automatically'
    end

    test 'allows use of absolute target or start if resolved path is sub-path of jail' do
      assert_equal "#{JAIL}/my/path", @resolver.system_path("#{JAIL}/my/path", '', JAIL)
      assert_equal "#{JAIL}/my/path", @resolver.system_path("#{JAIL}/my/path", nil, JAIL)
      assert_equal "#{JAIL}/my/path", @resolver.system_path('', "#{JAIL}/my/path", JAIL)
      assert_equal "#{JAIL}/my/path", @resolver.system_path(nil, "#{JAIL}/my/path", JAIL)
      assert_equal "#{JAIL}/my/path", @resolver.system_path('path', "#{JAIL}/my", JAIL)
      assert_equal '/foo/bar/baz.adoc', @resolver.system_path('/foo/bar/baz.adoc', nil, '/')
      assert_equal '/foo/bar/baz.adoc', @resolver.system_path('baz.adoc', '/foo/bar', '/')
      assert_equal '/foo/bar/baz.adoc', @resolver.system_path('baz.adoc', 'foo/bar', '/')
    end

    test 'uses jail path if start path is empty' do
      assert_equal "#{JAIL}/images/tiger.png", @resolver.system_path('images/tiger.png', '', JAIL)
      assert_equal "#{JAIL}/images/tiger.png", @resolver.system_path('images/tiger.png', nil, JAIL)
    end

    test 'warns if start is not contained within jail' do
      result = @resolver.system_path 'images/tiger.png', '/etc', JAIL
      assert_equal %(#{JAIL}/images/tiger.png), result
      assert_message @logger, :WARN, 'path is outside of jail; recovering automatically'

      @logger.clear
      result = @resolver.system_path '.', '/etc', JAIL
      assert_equal JAIL, result
      assert_message @logger, :WARN, 'path is outside of jail; recovering automatically'

      @logger.clear
      @resolver.file_separator = '\\'
      result = @resolver.system_path '.', 'C:/foo', 'C:/bar'
      assert_equal 'C:/bar', result
      assert_message @logger, :WARN, 'path is outside of jail; recovering automatically'
    end

    test 'allows start path to be parent of jail if resolved target is inside jail' do
      assert_equal "#{JAIL}/foo/path", @resolver.system_path('foo/path', JAIL, "#{JAIL}/foo")
      @resolver.file_separator = '\\'
      assert_equal "C:/dev/project/README.adoc", @resolver.system_path('project/README.adoc', 'C:/dev', 'C:/dev/project')
    end

    test 'relocates target to jail if resolved value fails outside of jail' do
      result = @resolver.system_path 'bar/baz.adoc', JAIL, "#{JAIL}/foo"
      assert_equal %(#{JAIL}/foo/bar/baz.adoc), result
      assert_message @logger, :WARN, 'path is outside of jail; recovering automatically'

      @logger.clear
      @resolver.file_separator = '\\'
      result = @resolver.system_path 'bar/baz.adoc', 'D:/', 'C:/foo'
      assert_equal 'C:/foo/bar/baz.adoc', result
      assert_message @logger, :WARN, '~outside of jail root'
    end

    test 'raises security error if start is not contained within jail and recover is disabled' do
      begin
        @resolver.system_path('images/tiger.png', '/etc', JAIL, recover: false)
        flunk 'Expecting SecurityError to be raised'
      rescue SecurityError
      end

      begin
        @resolver.system_path('.', '/etc', JAIL, recover: false)
        flunk 'Expecting SecurityError to be raised'
      rescue SecurityError
      end
    end

    test 'expands parent references in absolute path if jail is not specified' do
      assert_equal '/etc/stylesheet.css', @resolver.system_path('/usr/share/../../etc/stylesheet.css')
    end

    test 'resolves absolute directory if jail is not specified' do
      assert_equal '/usr/share/stylesheet.css', @resolver.system_path('/usr/share/stylesheet.css', '/home/dallen/docs/assets/stylesheets')
    end

    test 'resolves ancestor directory of start if jail is not specified' do
      assert_equal '/usr/share/stylesheet.css', @resolver.system_path('../../../../../usr/share/stylesheet.css', '/home/dallen/docs/assets/stylesheets')
    end

    test 'resolves absolute path if start is absolute and target is relative' do
      assert_equal '/usr/share/assets/stylesheet.css', @resolver.system_path('assets/stylesheet.css', '/usr/share')
    end

    test 'File.dirname preserves UNC path root on Windows', if: windows? do
      assert_equal File.dirname('\\\\server\\docs\\file.html'), '\\\\server\\docs'
    end

    test 'File.dirname preserves posix-style UNC path root on Windows', if: windows? do
      assert_equal File.dirname('//server/docs/file.html'), '//server/docs'
    end

    test 'resolves UNC path if start is absolute and target is relative' do
      assert_equal '//QA/c$/users/asciidoctor/assets/stylesheet.css', @resolver.system_path('assets/stylesheet.css', '//QA/c$/users/asciidoctor')
    end

    test 'resolves UNC path if target is UNC path' do
      @resolver.file_separator = '\\'
      assert_equal '//server/docs/output.html', @resolver.system_path('\\\\server\\docs\\output.html')
    end

    test 'resolves UNC path if target is posix-style UNC path' do
      assert_equal '//server/docs/output.html', @resolver.system_path('//server/docs/output.html')
    end

    test 'resolves classloader path if start is classloader path and target is relative', if: jruby? do
      assert_equal 'uri:classloader:images/sample.png', @resolver.system_path('sample.png', 'uri:classloader:images')
    end

    test 'resolves classloader path if start is root-relative classloader path and target is relative', if: jruby? do
      assert_equal 'uri:classloader:/images/sample.png', @resolver.system_path('sample.png', 'uri:classloader:/images')
    end

    test 'preserves classloader path if start is absolute path and target is classloader path', if: jruby? do
      assert_equal 'uri:classloader:/images/sample.png', @resolver.system_path('uri:classloader:/images/sample.png', '/home/doctor/docs')
    end

    test 'resolves relative target relative to current directory if start is empty' do
      pwd = File.expand_path(Dir.pwd)
      assert_equal "#{pwd}/images/tiger.png", @resolver.system_path('images/tiger.png', '')
      assert_equal "#{pwd}/images/tiger.png", @resolver.system_path('images/tiger.png', nil)
      assert_equal "#{pwd}/images/tiger.png", @resolver.system_path('images/tiger.png')
    end

    test 'resolves relative hidden target relative to current directory if start is empty' do
      pwd = File.expand_path(Dir.pwd)
      assert_equal "#{pwd}/.images/tiger.png", @resolver.system_path('.images/tiger.png', '')
      assert_equal "#{pwd}/.images/tiger.png", @resolver.system_path('.images/tiger.png', nil)
    end

    test 'resolves and normalizes start when target is empty' do
      pwd = File.expand_path Dir.pwd
      assert_equal '/home/doctor/docs', (@resolver.system_path '', '/home/doctor/docs')
      assert_equal '/home/doctor/docs', (@resolver.system_path '', '/home/doctor/./docs')
      assert_equal '/home/doctor/docs', (@resolver.system_path nil, '/home/doctor/docs')
      assert_equal '/home/doctor/docs', (@resolver.system_path nil, '/home/doctor/./docs')
      assert_equal %(#{pwd}/assets/images), (@resolver.system_path nil, 'assets/images')
      @resolver.system_path '', '../assets/images', JAIL
      assert_message @logger, :WARN, 'path has illegal reference to ancestor of jail; recovering automatically'
    end

    test 'posixifies windows paths' do
      @resolver.file_separator = '\\'
      assert_equal "#{JAIL}/assets/css", @resolver.system_path('..\\css', 'assets\\stylesheets', JAIL)
    end

    test 'resolves windows paths when file separator is backlash' do
      @resolver.file_separator = '\\'

      assert_equal 'C:/data/docs', (@resolver.system_path '..', 'C:\\data\\docs\\assets', 'C:\\data\\docs')

      result = @resolver.system_path '..\\..', 'C:\\data\\docs\\assets', 'C:\\data\\docs'
      assert_equal 'C:/data/docs', result
      assert_message @logger, :WARN, 'path has illegal reference to ancestor of jail; recovering automatically'

      @logger.clear
      result = @resolver.system_path '..\\..\\css', 'C:\\data\\docs\\assets', 'C:\\data\\docs'
      assert_equal 'C:/data/docs/css', result
      assert_message @logger, :WARN, 'path has illegal reference to ancestor of jail; recovering automatically'
    end

    test 'should calculate relative path' do
      filename = @resolver.system_path('part1/chapter1/section1.adoc', nil, JAIL)
      assert_equal "#{JAIL}/part1/chapter1/section1.adoc", filename
      assert_equal 'part1/chapter1/section1.adoc', @resolver.relative_path(filename, JAIL)
    end

    test 'should resolve relative path to filename outside of base directory' do
      filename = '/home/shared/partials'
      base_dir = '/home/user/docs'
      result = @resolver.relative_path filename, base_dir
      assert_equal '../../shared/partials', result
    end

    test 'should return original path if relative path cannot be computed', if: windows? do
      filename = 'D:/path/to/include/file.txt'
      base_dir = 'C:/docs'
      result = @resolver.relative_path filename, base_dir
      assert_equal 'D:/path/to/include/file.txt', result
    end

    test 'should resolve relative path relative to base dir in unsafe mode' do
      base_dir = fixture_path 'base'
      doc = empty_document base_dir: base_dir, safe: Asciidoctor::SafeMode::UNSAFE
      expected = ::File.join base_dir, 'images', 'tiger.png'
      actual = doc.normalize_system_path 'tiger.png', 'images'
      assert_equal expected, actual
    end

    test 'should resolve absolute path as absolute in unsafe mode' do
      base_dir = fixture_path 'base'
      doc = empty_document base_dir: base_dir, safe: Asciidoctor::SafeMode::UNSAFE
      actual = doc.normalize_system_path 'tiger.png', '/etc/images'
      assert_equal '/etc/images/tiger.png', actual
    end
  end
end
"##
);
