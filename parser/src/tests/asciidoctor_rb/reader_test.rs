// Adapted from Asciidoctor's reader test suite, found in
// https://github.com/asciidoctor/asciidoctor/blob/main/test/reader_test.rb.
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
//! Port of Asciidoctor's `reader_test.rb`.
//!
//! Asciidoctor's `Reader` / `PreprocessorReader` are a stateful, line-at-a-time
//! cursor over the source: tests construct one directly and drive its
//! imperative API (`read_line`, `peek_line(s)`, `has_more_lines?`, `unshift`,
//! `terminate`, `skip_blank_lines`, `read_lines_until`, `cursor`, `lineno`,
//! `file`/`dir`/`path`, `push_include`, `source_lines`, `catalog[:includes]`,
//! …). `asciidoc-parser` has no such public type: input is a `&str`, and the
//! reader's responsibilities live inside the crate-private
//! [`preprocess`](crate::parser) pass (include expansion + conditional
//! directives) whose result is handed straight to `Document::parse`. The line
//! cursor itself is an internal [`Span`](crate::Span) walk.
//!
//! So the large blocks of this suite that assert on the reader *object* — the
//! `Reader` API (`Prepare lines`, `With empty data`, `With data`, `Line
//! context`, `Read lines until`), the `PreprocessorReader` type hierarchy and
//! `push_include` include stack, and every case that inspects `reader.lines` /
//! `reader.read` / a cursor / `doc.catalog[:includes]` — have no analog here
//! and are reproduced verbatim in `non_normative!` blocks, each annotated with
//! the reason it is not ported. UTF-8 BOM stripping, UTF-16 transcoding,
//! `uri:classloader:` includes, and remote (URL) includes are likewise out of
//! scope for this crate (see the crate README) and stay non-normative.
//! Front-matter skipping (the `---` fence gated on `skip-front-matter`) *is*
//! supported and is ported below.
//!
//! What *is* normative and supported — and therefore ported to `verifies!`
//! blocks driving [`Parser`](crate::Parser) — is the observable preprocessor
//! behavior: the include directive (link-macro replacement under
//! [`SafeMode::Secure`], unresolved-directive handling, tag/line/leveloffset
//! selection, and the include-tag diagnostics) and the conditional directives
//! (`ifdef` / `ifndef` / `ifeval` / `endif`, including nesting, the single-line
//! and long forms, escaping, and the malformed-directive diagnostics). Several
//! of those diagnostics did not previously exist in the crate; they were added
//! (see [`WarningType`](crate::warnings::WarningType)) as part of this port so
//! the warning cases could be verified rather than skipped.
//!
//! Where the crate's observable result differs from Asciidoctor's, the
//! `verifies!` body asserts the crate's actual behavior and a comment explains
//! the difference (most notably: an unterminated conditional is always reported
//! at the directive's own line, as Asciidoctor does only when `sourcemap` is
//! enabled, because this crate always maintains a source map).

use std::{cell::RefCell, rc::Rc};

use crate::{
    attributes::Attrlist,
    parser::{IncludeContent, IncludeFileHandler},
    tests::prelude::{inline_file_handler::InlineFileHandler, *},
};

track_file!("ref/asciidoctor/test/reader_test.rb");

/// A recorded `resolve_target` call: the `(source, target, encoding)` the
/// parser handed to the [`IncludeFileHandler`] — where `encoding` is the value
/// of the directive's `encoding` attribute, if any.
type RecordedInclude = (Option<String>, String, Option<String>);

/// A mock [`IncludeFileHandler`] that records the `(source, target, encoding)`
/// of every `resolve_target` call and returns the same fixed content for each.
///
/// The real file-system lookup (and any transcoding) is downstream of this
/// crate, but the parser is still responsible for the *plumbing*: resolving
/// attribute references (and otherwise cleaning up) the directive's target,
/// naming the including file as `source`, and forwarding the `encoding`
/// attribute — before delegating. This handler lets a test assert exactly what
/// the parser hands off.
#[derive(Clone, Debug)]
struct RecordingIncludeFileHandler {
    calls: Rc<RefCell<Vec<RecordedInclude>>>,
    /// What the handler returns from `resolve_target`: `Some((content,
    /// transcoded))` for a resolved file (returned via
    /// [`IncludeContent::transcoded`] when `transcoded` is set, otherwise
    /// [`IncludeContent::new`]), or `None` for a file that could not be found.
    result: Option<(&'static str, bool)>,
}

impl RecordingIncludeFileHandler {
    fn new(content: &'static str) -> Self {
        Self {
            calls: Rc::new(RefCell::new(Vec::new())),
            result: Some((content, false)),
        }
    }

    /// Like [`new`](Self::new), but returns its content as
    /// [`IncludeContent::transcoded`] — i.e. as a handler that read a non-UTF-8
    /// file and reencoded it per the `encoding` attribute.
    fn transcoding(content: &'static str) -> Self {
        Self {
            result: Some((content, true)),
            ..Self::new(content)
        }
    }

    /// A handler that records the call but reports the file as not found
    /// (returns `None`), as a real handler would for a missing target.
    fn missing() -> Self {
        Self {
            result: None,
            ..Self::new("")
        }
    }

    /// The `(source, target, encoding)` of every recorded call, in order. A
    /// clone of the handler shares this record with the copy handed to the
    /// parser.
    fn calls(&self) -> Vec<RecordedInclude> {
        self.calls.borrow().clone()
    }
}

impl IncludeFileHandler for RecordingIncludeFileHandler {
    fn resolve_target<'src>(
        &self,
        source: Option<&str>,
        target: &str,
        attrlist: &Attrlist<'src>,
        _parser: &Parser,
    ) -> Option<IncludeContent> {
        let encoding = attrlist
            .named_attribute("encoding")
            .map(|a| a.value().to_string());
        self.calls
            .borrow_mut()
            .push((source.map(str::to_owned), target.to_owned(), encoding));
        self.result.map(|(content, transcoded)| {
            if transcoded {
                IncludeContent::transcoded(content)
            } else {
                IncludeContent::new(content)
            }
        })
    }
}

/// The crate's analog of Asciidoctor's `doc.reader.read`: run `input` through
/// the preprocessor (include expansion + conditional directives) with `parser`
/// and return the resulting lines joined by `\n`, exactly as Asciidoctor's
/// reader would surface them. (Ruby's `reader.read` joins the reader lines with
/// no trailing newline, so the single trailing newline the preprocessor emits
/// is stripped.)
fn reader_read(parser: &Parser, input: &str) -> String {
    let (output, _source_map, _warnings, _includes) =
        crate::parser::preprocessor::preprocess(input, parser);
    output
        .strip_suffix('\n')
        .map(str::to_owned)
        .unwrap_or(output)
}

/// Verbatim copy of Asciidoctor's `test/fixtures/include-file.adoc`, used by
/// the tag-selection tests below (which the crate feeds through an
/// [`InlineFileHandler`] rather than reading from disk).
const INCLUDE_FILE_ADOC: &str = "\
first line of included content
second line of included content
third line of included content
fourth line of included content
fifth line of included content
sixth line of included content
seventh line of included content
eighth line of included content

// tag::snippet[]
// tag::snippetA[]
snippetA content
// end::snippetA[]

non-tagged content

// tag::snippetB[]
snippetB content
// end::snippetB[]
// end::snippet[]

more non-tagged content

last line of included content";

/// Verbatim copy of Asciidoctor's `test/fixtures/tagged-class-enclosed.rb`.
const TAGGED_CLASS_ENCLOSED_RB: &str = "\
#tag::all[]
class Dog
  #tag::init[]
  def initialize breed
    @breed = breed
  end
  #end::init[]
  #tag::bark[]

  def bark
    #tag::bark-beagle[]
    if @breed == 'beagle'
      'woof woof woof woof woof'
    #end::bark-beagle[]
    #tag::bark-other[]
    else
      'woof woof'
    #end::bark-other[]
    #tag::bark-all[]
    end
    #end::bark-all[]
  end
  #end::bark[]
end
#end::all[]";

/// Verbatim copy of Asciidoctor's `test/fixtures/tagged-class.rb` (like
/// [`TAGGED_CLASS_ENCLOSED_RB`], but with no enclosing `all` tag).
const TAGGED_CLASS_RB: &str = "\
class Dog
  #tag::init[]
  def initialize breed
    @breed = breed
  end
  #end::init[]
  #tag::bark[]

  def bark
    #tag::bark-beagle[]
    if @breed == 'beagle'
      'woof woof woof woof woof'
    #end::bark-beagle[]
    #tag::bark-other[]
    else
      'woof woof'
    #end::bark-other[]
    #tag::bark-all[]
    end
    #end::bark-all[]
  end
  #end::bark[]
end";

/// Verbatim copy of Asciidoctor's `test/fixtures/include-file.xml` — a tagged
/// region delimited by XML circumfix comments.
const INCLUDE_FILE_XML: &str = "\
<root>
  <!-- tag::snippet[] -->
  <snippet>content</snippet>
  <!-- end::snippet[] -->
</root>";

/// Verbatim copy of Asciidoctor's `test/fixtures/include-file.ml` — a tagged
/// region delimited by OCaml circumfix comments.
const INCLUDE_FILE_ML: &str = "\
(* tag::snippet[] *)
let s = SS.empty;;
(* end::snippet[] *)";

/// Verbatim copy of Asciidoctor's `test/fixtures/include-file.jsx`.
const INCLUDE_FILE_JSX: &str = "\
const element = (
  <div>
    <h1>Hello, Programmer!</h1>
    <!-- tag::snippet[] -->
    <p>Welcome to the club.</p>
    <!-- end::snippet[] -->
  </div>
)";

/// Verbatim copy of Asciidoctor's `test/fixtures/basic-docinfo.xml` (the inner
/// elements are indented, so an `indent=0` include can reset that indentation).
const BASIC_DOCINFO_XML: &str = "\
<copyright><!-- don't remove the indent! -->
    <year>2013</year>
    <holder>Acme\u{2122}, Inc.</holder>
</copyright>";

non_normative!(
    r#"
# frozen_string_literal: true
require_relative 'test_helper'

class ReaderTest < Minitest::Test
  DIRNAME = ASCIIDOCTOR_TEST_DIR

  SAMPLE_DATA = ['first line', 'second line', 'third line']

  context 'Reader' do
    context 'Prepare lines' do
"#
);

// The entire `Reader` context is out of scope for this crate. Asciidoctor's
// `Reader` is a public, stateful, line-at-a-time cursor object; these tests
// construct one directly and drive its imperative API — `lines`, `read_line`,
// `read_lines`, `read`, `peek_line(s)`, `has_more_lines?`, `empty?`,
// `next_line_empty?`, `advance`, `unshift(_all)`, `terminate`,
// `skip_blank_lines`, `skip_comment_lines`, `source(_lines)`, `cursor` /
// `line_info` / `cursor_at_prev_line`, and `read_lines_until`.
// `asciidoc-parser` exposes no such type: input is a `&str`, the line cursor is
// an internal `Span` walk, and there is no user-visible reader object to assert
// against. UTF-8/UTF-16 BOM handling and encoding transcoding (the `Prepare
// lines` cases) are likewise out of scope — the crate requires UTF-8 input (see
// the crate README).
non_normative!(
    r#"
      test 'should prepare lines from Array data' do
        reader = Asciidoctor::Reader.new SAMPLE_DATA
        assert_equal SAMPLE_DATA, reader.lines
      end

      test 'should prepare lines from String data' do
        reader = Asciidoctor::Reader.new SAMPLE_DATA.join(Asciidoctor::LF)
        assert_equal SAMPLE_DATA, reader.lines
      end

      test 'should prepare lines from String data with trailing newline' do
        reader = Asciidoctor::Reader.new SAMPLE_DATA.join(Asciidoctor::LF) + Asciidoctor::LF
        assert_equal SAMPLE_DATA, reader.lines
      end

      test 'should remove UTF-8 BOM from first line of String data' do
        ['UTF-8', 'ASCII-8BIT'].each do |start_encoding|
          data = String.new %(\xef\xbb\xbf#{SAMPLE_DATA.join ::Asciidoctor::LF}), encoding: start_encoding
          reader = Asciidoctor::Reader.new data, nil, normalize: true
          assert_equal Encoding::UTF_8, reader.lines[0].encoding
          assert_equal 'f', reader.lines[0].chr
          assert_equal SAMPLE_DATA, reader.lines
        end
      end

      test 'should remove UTF-8 BOM from first line of Array data' do
        ['UTF-8', 'ASCII-8BIT'].each do |start_encoding|
          data = SAMPLE_DATA.drop 0
          data[0] = String.new %(\xef\xbb\xbf#{data.first}), encoding: start_encoding
          reader = Asciidoctor::Reader.new data, nil, normalize: true
          assert_equal Encoding::UTF_8, reader.lines[0].encoding
          assert_equal 'f', reader.lines[0].chr
          assert_equal SAMPLE_DATA, reader.lines
        end
      end

      test 'should encode UTF-16LE string to UTF-8 when BOM is found' do
        ['UTF-8', 'ASCII-8BIT'].each do |start_encoding|
          data = "\ufeff#{SAMPLE_DATA.join ::Asciidoctor::LF}".encode('UTF-16LE').force_encoding(start_encoding)
          reader = Asciidoctor::Reader.new data, nil, normalize: true
          assert_equal Encoding::UTF_8, reader.lines[0].encoding
          assert_equal 'f', reader.lines[0].chr
          assert_equal SAMPLE_DATA, reader.lines
        end
      end

      test 'should encode UTF-16LE string array to UTF-8 when BOM is found' do
        ['UTF-8', 'ASCII-8BIT'].each do |start_encoding|
          # NOTE can't split a UTF-16LE string using .lines when encoding is set to UTF-8
          data = SAMPLE_DATA.drop 0
          data.unshift %(\ufeff#{data.shift})
          data.each {|line| (line.encode 'UTF-16LE').force_encoding start_encoding }
          reader = Asciidoctor::Reader.new data, nil, normalize: true
          assert_equal Encoding::UTF_8, reader.lines[0].encoding
          assert_equal 'f', reader.lines[0].chr
          assert_equal SAMPLE_DATA, reader.lines
        end
      end

      test 'should encode UTF-16BE string to UTF-8 when BOM is found' do
        ['UTF-8', 'ASCII-8BIT'].each do |start_encoding|
          data = "\ufeff#{SAMPLE_DATA.join ::Asciidoctor::LF}".encode('UTF-16BE').force_encoding(start_encoding)
          reader = Asciidoctor::Reader.new data, nil, normalize: true
          assert_equal Encoding::UTF_8, reader.lines[0].encoding
          assert_equal 'f', reader.lines[0].chr
          assert_equal SAMPLE_DATA, reader.lines
        end
      end

      test 'should encode UTF-16BE string array to UTF-8 when BOM is found' do
        ['UTF-8', 'ASCII-8BIT'].each do |start_encoding|
          data = SAMPLE_DATA.drop 0
          data.unshift %(\ufeff#{data.shift})
          data = data.map {|line| (line.encode 'UTF-16BE').force_encoding start_encoding }
          reader = Asciidoctor::Reader.new data, nil, normalize: true
          assert_equal Encoding::UTF_8, reader.lines[0].encoding
          assert_equal 'f', reader.lines[0].chr
          assert_equal SAMPLE_DATA, reader.lines
        end
      end
    end

    context 'With empty data' do
      test 'has_more_lines? should return false with empty data' do
        refute Asciidoctor::Reader.new.has_more_lines?
      end

      test 'empty? should return true with empty data' do
        assert Asciidoctor::Reader.new.empty?
        assert Asciidoctor::Reader.new.eof?
      end

      test 'next_line_empty? should return true with empty data' do
        assert Asciidoctor::Reader.new.next_line_empty?
      end

      test 'peek_line should return nil with empty data' do
        assert_nil Asciidoctor::Reader.new.peek_line
      end

      test 'peek_lines should return empty Array with empty data' do
        assert_equal [], Asciidoctor::Reader.new.peek_lines(1)
      end

      test 'read_line should return nil with empty data' do
        assert_nil Asciidoctor::Reader.new.read_line
        #assert_nil Asciidoctor::Reader.new.get_line
      end

      test 'read_lines should return empty Array with empty data' do
        assert_equal [], Asciidoctor::Reader.new.read_lines
        #assert_equal [], Asciidoctor::Reader.new.get_lines
      end
    end

    context 'With data' do
      test 'has_more_lines? should return true if there are lines remaining' do
        reader = Asciidoctor::Reader.new SAMPLE_DATA
        assert reader.has_more_lines?
      end

      test 'empty? should return false if there are lines remaining' do
        reader = Asciidoctor::Reader.new SAMPLE_DATA
        refute reader.empty?
        refute reader.eof?
      end

      test 'next_line_empty? should return false if next line is not blank' do
        reader = Asciidoctor::Reader.new SAMPLE_DATA
        refute reader.next_line_empty?
      end

      test 'next_line_empty? should return true if next line is blank' do
        reader = Asciidoctor::Reader.new ['', 'second line']
        assert reader.next_line_empty?
      end

      test 'peek_line should return nil if next entry is nil' do
        assert_nil (Asciidoctor::Reader.new [nil]).peek_line
      end

      test 'peek_line should return next line if there are lines remaining' do
        reader = Asciidoctor::Reader.new SAMPLE_DATA
        assert_equal SAMPLE_DATA.first, reader.peek_line
      end

      test 'peek_line should not consume line or increment line number' do
        reader = Asciidoctor::Reader.new SAMPLE_DATA
        assert_equal SAMPLE_DATA.first, reader.peek_line
        assert_equal SAMPLE_DATA.first, reader.peek_line
        assert_equal 1, reader.lineno
      end

      test 'peek_line should return next lines if there are lines remaining' do
        reader = Asciidoctor::Reader.new SAMPLE_DATA
        assert_equal SAMPLE_DATA[0..1], reader.peek_lines(2)
      end

      test 'peek_lines should not consume lines or increment line number' do
        reader = Asciidoctor::Reader.new SAMPLE_DATA
        assert_equal SAMPLE_DATA[0..1], reader.peek_lines(2)
        assert_equal SAMPLE_DATA[0..1], reader.peek_lines(2)
        assert_equal 1, reader.lineno
      end

      test 'peek_lines should not increment line number if reader overruns buffer' do
        reader = Asciidoctor::Reader.new SAMPLE_DATA
        assert_equal SAMPLE_DATA, (reader.peek_lines SAMPLE_DATA.size * 2)
        assert_equal 1, reader.lineno
      end

      test 'peek_lines should peek all lines if no arguments are given' do
        reader = Asciidoctor::Reader.new SAMPLE_DATA
        assert_equal SAMPLE_DATA, reader.peek_lines
        assert_equal 1, reader.lineno
      end

      test 'peek_lines should not invert order of lines' do
        reader = Asciidoctor::Reader.new SAMPLE_DATA
        assert_equal SAMPLE_DATA, reader.lines
        reader.peek_lines 3
        assert_equal SAMPLE_DATA, reader.lines
      end

      test 'read_line should return next line if there are lines remaining' do
        reader = Asciidoctor::Reader.new SAMPLE_DATA
        assert_equal SAMPLE_DATA.first, reader.read_line
      end

      test 'read_line should consume next line and increment line number' do
        reader = Asciidoctor::Reader.new SAMPLE_DATA
        assert_equal SAMPLE_DATA[0], reader.read_line
        assert_equal SAMPLE_DATA[1], reader.read_line
        assert_equal 3, reader.lineno
      end

      test 'advance should consume next line and return a Boolean indicating if a line was consumed' do
        reader = Asciidoctor::Reader.new SAMPLE_DATA
        assert reader.advance
        assert reader.advance
        assert reader.advance
        refute reader.advance
      end

      test 'read_lines should return all lines' do
        reader = Asciidoctor::Reader.new SAMPLE_DATA
        assert_equal SAMPLE_DATA, reader.read_lines
      end

      test 'read should return all lines joined as String' do
        reader = Asciidoctor::Reader.new SAMPLE_DATA
        assert_equal SAMPLE_DATA.join(::Asciidoctor::LF), reader.read
      end

      test 'has_more_lines? should return false after read_lines is invoked' do
        reader = Asciidoctor::Reader.new SAMPLE_DATA
        reader.read_lines
        refute reader.has_more_lines?
      end

      test 'unshift puts line onto Reader as next line to read' do
        reader = Asciidoctor::Reader.new SAMPLE_DATA, nil, normalize: true
        reader.unshift 'line zero'
        assert_equal 'line zero', reader.peek_line
        assert_equal 'line zero', reader.read_line
        assert_equal 1, reader.lineno
      end

      test 'terminate should consume all lines and update line number' do
        reader = Asciidoctor::Reader.new SAMPLE_DATA
        reader.terminate
        assert reader.eof?
        assert_equal 4, reader.lineno
      end

      test 'skip_blank_lines should skip blank lines' do
        reader = Asciidoctor::Reader.new ['', ''].concat(SAMPLE_DATA)
        reader.skip_blank_lines
        assert_equal SAMPLE_DATA.first, reader.peek_line
      end

      test 'lines should return remaining lines' do
        reader = Asciidoctor::Reader.new SAMPLE_DATA
        reader.read_line
        assert_equal SAMPLE_DATA[1..-1], reader.lines
      end

      test 'source_lines should return copy of original data Array' do
        reader = Asciidoctor::Reader.new SAMPLE_DATA
        reader.read_lines
        assert_equal SAMPLE_DATA, reader.source_lines
      end

      test 'source should return original data Array joined as String' do
        reader = Asciidoctor::Reader.new SAMPLE_DATA
        reader.read_lines
        assert_equal SAMPLE_DATA.join(::Asciidoctor::LF), reader.source
      end

    end

    context 'Line context' do
      test 'cursor.to_s should return file name and line number of current line' do
        reader = Asciidoctor::Reader.new SAMPLE_DATA, 'sample.adoc'
        reader.read_line
        assert_equal 'sample.adoc: line 2', reader.cursor.to_s
      end

      test 'line_info should return file name and line number of current line' do
        reader = Asciidoctor::Reader.new SAMPLE_DATA, 'sample.adoc'
        reader.read_line
        assert_equal 'sample.adoc: line 2', reader.line_info
      end

      test 'cursor_at_prev_line should return file name and line number of previous line read' do
        reader = Asciidoctor::Reader.new SAMPLE_DATA, 'sample.adoc'
        reader.read_line
        assert_equal 'sample.adoc: line 1', reader.cursor_at_prev_line.to_s
      end
    end

    context 'Read lines until' do
      test 'Read lines until until end' do
        lines = <<~'EOS'.lines
        This is one paragraph.

        This is another paragraph.
        EOS

        reader = Asciidoctor::Reader.new lines, nil, normalize: true
        result = reader.read_lines_until
        assert_equal 3, result.size
        assert_equal lines.map(&:chomp), result
        refute reader.has_more_lines?
        assert reader.eof?
      end

      test 'Read lines until until blank line' do
        lines = <<~'EOS'.lines
        This is one paragraph.

        This is another paragraph.
        EOS

        reader = Asciidoctor::Reader.new lines, nil, normalize: true
        result = reader.read_lines_until break_on_blank_lines: true
        assert_equal 1, result.size
        assert_equal lines.first.chomp, result.first
        assert_equal lines.last.chomp, reader.peek_line
      end

      test 'Read lines until until blank line preserving last line' do
        lines = <<~'EOS'.split ::Asciidoctor::LF
        This is one paragraph.

        This is another paragraph.
        EOS

        reader = Asciidoctor::Reader.new lines
        result = reader.read_lines_until break_on_blank_lines: true, preserve_last_line: true
        assert_equal 1, result.size
        assert_equal lines.first.chomp, result.first
        assert reader.next_line_empty?
      end

      test 'Read lines until until condition is true' do
        lines = <<~'EOS'.split ::Asciidoctor::LF
        --
        This is one paragraph inside the block.

        This is another paragraph inside the block.
        --

        This is a paragraph outside the block.
        EOS

        reader = Asciidoctor::Reader.new lines
        reader.read_line
        result = reader.read_lines_until {|line| line == '--' }
        assert_equal 3, result.size
        assert_equal lines[1, 3], result
        assert reader.next_line_empty?
      end

      test 'Read lines until until condition is true, taking last line' do
        lines = <<~'EOS'.split ::Asciidoctor::LF
        --
        This is one paragraph inside the block.

        This is another paragraph inside the block.
        --

        This is a paragraph outside the block.
        EOS

        reader = Asciidoctor::Reader.new lines
        reader.read_line
        result = reader.read_lines_until(read_last_line: true) {|line| line == '--' }
        assert_equal 4, result.size
        assert_equal lines[1, 4], result
        assert reader.next_line_empty?
      end

      test 'Read lines until until condition is true, taking and preserving last line' do
        lines = <<~'EOS'.split ::Asciidoctor::LF
        --
        This is one paragraph inside the block.

        This is another paragraph inside the block.
        --

        This is a paragraph outside the block.
        EOS

        reader = Asciidoctor::Reader.new lines
        reader.read_line
        result = reader.read_lines_until(read_last_line: true, preserve_last_line: true) {|line| line == '--' }
        assert_equal 4, result.size
        assert_equal lines[1, 4], result
        assert_equal '--', reader.peek_line
      end

      test 'read lines until terminator' do
        lines = <<~'EOS'.lines
        ****
        captured

        also captured
        ****

        not captured
        EOS

        expected = ['captured', '', 'also captured']

        doc = empty_safe_document base_dir: DIRNAME
        reader = Asciidoctor::PreprocessorReader.new doc, lines, nil, normalize: true
        terminator = reader.read_line
        result = reader.read_lines_until terminator: terminator, skip_processing: true
        assert_equal expected, result
        refute reader.unterminated
      end

      test 'should flag reader as unterminated if reader reaches end of source without finding terminator' do
        lines = <<~'EOS'.lines
        ****
        captured

        also captured

        captured yet again
        EOS

        expected = lines[1..-1].map(&:chomp)

        using_memory_logger do |logger|
          doc = empty_safe_document base_dir: DIRNAME
          reader = Asciidoctor::PreprocessorReader.new doc, lines, nil, normalize: true
          terminator = reader.peek_line
          result = reader.read_lines_until terminator: terminator, skip_first_line: true, skip_processing: true
          assert_equal expected, result
          assert reader.unterminated
          assert_message logger, :WARN, '<stdin>: line 1: unterminated **** block', Hash
        end
      end
"#
);

non_normative!(
    r#"
    end
  end

  context 'PreprocessorReader' do
    context 'Type hierarchy' do
"#
);

// The `PreprocessorReader` type-hierarchy tests assert on the Ruby class
// relationship (`PreprocessorReader < Reader`) and its initializer via
// `doc.reader` / `reader.lineno`. No such type exists in this crate.
non_normative!(
    r#"
      test 'PreprocessorReader should extend from Reader' do
        reader = empty_document.reader
        assert_kind_of Asciidoctor::PreprocessorReader, reader
      end

      test 'PreprocessorReader should invoke or emulate Reader initializer' do
        doc = Asciidoctor::Document.new SAMPLE_DATA
        reader = doc.reader
        assert_equal SAMPLE_DATA, reader.lines
        assert_equal 1, reader.lineno
      end
"#
);

non_normative!(
    r#"
    end

    context 'Prepare lines' do
"#
);

// Out of scope (Reader API): these assert on `reader.lines` after Asciidoctor's
// leading/trailing blank-line normalization of Array/String input. This crate
// takes a `&str` and has no reader-line array to inspect.
non_normative!(
    r#"
      test 'should prepare and normalize lines from Array data' do
        data = SAMPLE_DATA.drop 0
        data.unshift ''
        data.push ''
        doc = Asciidoctor::Document.new data
        reader = doc.reader
        assert_equal [''] + SAMPLE_DATA, reader.lines
      end

      test 'should prepare and normalize lines from String data' do
        data = SAMPLE_DATA.drop 0
        data.unshift ' '
        data.push ' '
        data_as_string = data * ::Asciidoctor::LF
        doc = Asciidoctor::Document.new data_as_string
        reader = doc.reader
        assert_equal [''] + SAMPLE_DATA, reader.lines
      end

      test 'should drop all lines if all lines are empty' do
        data = ['', ' ', '', ' ']
        doc = Asciidoctor::Document.new data
        reader = doc.reader
        assert reader.lines.empty?
      end

"#
);

// Normative and supported: `\r\n` line endings are cleaned to `\n`. (The Ruby
// test inspects `reader.lines`; the crate normalizes CRLF per line as it walks
// the source, so the parsed block content carries no carriage returns.)
#[test]
fn should_clean_crlf_from_end_of_lines() {
    verifies!(
        r#"
      test 'should clean CRLF from end of lines' do
        input = <<~EOS
        source\r
        with\r
        CRLF\r
        line endings\r
        EOS

        [input, input.lines, input.split(::Asciidoctor::LF), input.split(::Asciidoctor::LF).join(::Asciidoctor::LF)].each do |lines|
          doc = Asciidoctor::Document.new lines
          reader = doc.reader
          reader.lines.each do |line|
            refute line.end_with?("\r"), "CRLF not properly cleaned for source lines: #{lines.inspect}"
            refute line.end_with?("\r\n"), "CRLF not properly cleaned for source lines: #{lines.inspect}"
            refute line.end_with?("\n"), "CRLF not properly cleaned for source lines: #{lines.inspect}"
          end
        end
      end

"#
    );

    let doc = Parser::default().parse("source\r\nwith\r\nCRLF\r\nline endings\r\n");

    // The four adjacent lines form one paragraph, with every CRLF cleaned to a
    // bare LF (no `\r` survives).
    let paras = rendered_paragraphs(&doc);
    assert_eq!(paras, vec!["source\nwith\nCRLF\nline endings"]);
    assert!(!paras[0].contains('\r'));
}

// Front matter support (https://github.com/asciidoc-rs/asciidoc-parser/issues/745):
// a `---`-fenced YAML/TOML block at the very top of the document is dropped –
// and captured verbatim in the `front-matter` attribute – but only when the
// `skip-front-matter` attribute is set. This crate has no public reader, so
// where Asciidoctor inspects `reader.peek_line` / `reader.lineno`, these ports
// observe the parsed document instead: the `front-matter` attribute, the
// recognized document title, and the title's (preserved) line number.
#[test]
fn should_not_skip_front_matter_by_default() {
    verifies!(
        r#"
      test 'should not skip front matter by default' do
        input = <<~'EOS'
        ---
        layout: post
        title: Document Title
        author: username
        tags: [ first, second ]
        ---
        = Document Title
        Author Name

        preamble
        EOS

        doc = Asciidoctor::Document.new input
        reader = doc.reader
        refute doc.attributes.key?('front-matter')
        assert_equal '---', reader.peek_line
        assert_equal 1, reader.lineno
      end

"#
    );

    // With `skip-front-matter` unset, the leading `---` keeps its ordinary
    // meaning: nothing is stripped, no `front-matter` attribute is recorded, and
    // the `= Document Title` line – no longer the first line – is not read as the
    // document title.
    let doc = Parser::default().parse(
        "---\nlayout: post\ntitle: Document Title\nauthor: username\ntags: [ first, second ]\n---\n= Document Title\nAuthor Name\n\npreamble\n",
    );

    assert_eq!(doc.attribute_value("front-matter"), InterpretedValue::Unset);
    assert_eq!(doc.header().title(), None);
}

#[test]
fn should_not_skip_front_matter_if_ending_delimiter_is_not_found() {
    verifies!(
        r#"
      test 'should not skip front matter if ending delimiter is not found' do
        input = <<~'EOS'
        ---
        title: Document Title
        tags: [ first, second ]
        = Document Title
        Author Name

        preamble
        EOS

        doc = Asciidoctor::Document.new input, attributes: { 'skip-front-matter' => '' }
        reader = doc.reader
        assert_equal '---', reader.peek_line
        refute doc.attributes.key? 'front-matter'
        assert_equal 1, reader.lineno
      end

"#
    );

    // `skip-front-matter` is set, but the block never closes with a second
    // `---`, so it is malformed: the source is left untouched and no
    // `front-matter` attribute is recorded.
    let doc = Parser::default()
        .with_intrinsic_attribute_bool("skip-front-matter", true, ModificationContext::ApiOnly)
        .parse(
            "---\ntitle: Document Title\ntags: [ first, second ]\n= Document Title\nAuthor Name\n\npreamble\n",
        );

    assert_eq!(doc.attribute_value("front-matter"), InterpretedValue::Unset);
    assert_eq!(doc.header().title(), None);
}

#[test]
fn should_skip_front_matter_if_specified_by_skip_front_matter_attribute() {
    verifies!(
        r#"
      test 'should skip front matter if specified by skip-front-matter attribute' do
        front_matter = <<~'EOS'.chop
        layout: post
        title: Document Title
        author: username
        tags: [ first, second ]
        EOS

        input = <<~EOS
        ---
        #{front_matter}
        ---
        = Document Title
        Author Name

        preamble
        EOS

        doc = Asciidoctor::Document.new input, attributes: { 'skip-front-matter' => '' }
        reader = doc.reader
        assert_equal '= Document Title', reader.peek_line
        assert_equal front_matter, doc.attributes['front-matter']
        assert_equal 7, reader.lineno
      end
"#
    );

    // `skip-front-matter` is set and the block is well-formed: its content
    // (delimiters excluded, lines joined by LF) is captured in the
    // `front-matter` attribute, and parsing resumes at `= Document Title`, which
    // – thanks to the blank lines left in place of the removed block – is still
    // reported at its original line 7.
    let doc = Parser::default()
        .with_intrinsic_attribute_bool("skip-front-matter", true, ModificationContext::ApiOnly)
        .parse(
            "---\nlayout: post\ntitle: Document Title\nauthor: username\ntags: [ first, second ]\n---\n= Document Title\nAuthor Name\n\npreamble\n",
        );

    assert_eq!(
        doc.attribute_value("front-matter"),
        InterpretedValue::Value(
            "layout: post\ntitle: Document Title\nauthor: username\ntags: [ first, second ]"
        )
    );
    assert_eq!(doc.header().title(), Some("Document Title"));
    assert_eq!(doc.header().title_source().unwrap().line(), 7);
}

// Crate-native (no Asciidoctor analog): the front-matter delimiters are matched
// after a CRLF line ending is stripped, and the captured `front-matter` value
// is likewise chomped, so a document with `\r\n` line endings is handled the
// same as one with bare `\n`.
#[test]
fn should_skip_front_matter_with_crlf_line_endings() {
    let doc = Parser::default()
        .with_intrinsic_attribute_bool("skip-front-matter", true, ModificationContext::ApiOnly)
        .parse("---\r\nlayout: post\r\ntitle: Document Title\r\n---\r\n= Document Title\r\nAuthor Name\r\n\r\npreamble\r\n");

    assert_eq!(
        doc.attribute_value("front-matter"),
        InterpretedValue::Value("layout: post\ntitle: Document Title")
    );
    assert_eq!(doc.header().title(), Some("Document Title"));
    assert_eq!(doc.header().title_source().unwrap().line(), 5);
}

// Crate-native (no Asciidoctor analog): with `skip-front-matter` set but no
// opening `---` on the first line, there is nothing to skip – the document
// parses normally and no `front-matter` attribute is recorded.
#[test]
fn should_not_skip_front_matter_when_first_line_is_not_a_delimiter() {
    let doc = Parser::default()
        .with_intrinsic_attribute_bool("skip-front-matter", true, ModificationContext::ApiOnly)
        .parse("= Document Title\nAuthor Name\n\npreamble\n");

    assert_eq!(doc.attribute_value("front-matter"), InterpretedValue::Unset);
    assert_eq!(doc.header().title(), Some("Document Title"));
    assert_eq!(doc.header().title_source().unwrap().line(), 1);
}

non_normative!(
    r#"
    end

    context 'Include Stack' do
"#
);

// The `Include Stack` context drives `PreprocessorReader#push_include` directly
// — pushing line arrays onto the reader and inspecting `reader.file` / `dir` /
// `path` and `doc.catalog[:includes]`. This crate has no public reader to push
// onto and does not expose an include catalog; include resolution is handled
// internally by the preprocessor and an `IncludeFileHandler`. (Observable
// include *behavior* is covered by the `Include Directive` cases below and by
// the crate's own `directives/include*` spec tests.)
non_normative!(
    r#"
      test 'PreprocessorReader#push_include method should return reader' do
        reader = empty_document.reader
        append_lines = %w(one two three)
        result = reader.push_include append_lines, '<stdin>', '<stdin>'
        assert_equal reader, result
      end

      test 'PreprocessorReader#push_include method should put lines on top of stack' do
        lines = %w(a b c)
        doc = Asciidoctor::Document.new lines
        reader = doc.reader
        append_lines = %w(one two three)
        reader.push_include append_lines, '', '<stdin>'
        assert_equal 1, reader.include_stack.size
        assert_equal 'one', reader.read_line.rstrip
      end

      test 'PreprocessorReader#push_include method should gracefully handle file and path' do
        lines = %w(a b c)
        doc = Asciidoctor::Document.new lines
        reader = doc.reader
        append_lines = %w(one two three)
        reader.push_include append_lines
        assert_equal 1, reader.include_stack.size
        assert_equal 'one', reader.read_line.rstrip
        assert_nil reader.file
        assert_equal '<stdin>', reader.path
      end

      test 'PreprocessorReader#push_include method should set path from file automatically if not specified' do
        lines = %w(a b c)
        doc = Asciidoctor::Document.new lines
        reader = doc.reader
        append_lines = %w(one two three)
        reader.push_include append_lines, '/tmp/lines.adoc'
        assert_equal '/tmp/lines.adoc', reader.file
        assert_equal 'lines.adoc', reader.path
        assert doc.catalog[:includes]['lines']
      end

      test 'PreprocessorReader#push_include method should accept file as a URI and compute dir and path' do
        file_uri = ::URI.parse 'http://example.com/docs/file.adoc'
        dir_uri = ::URI.parse 'http://example.com/docs'
        reader = empty_document.reader
        reader.push_include %w(one two three), file_uri
        assert_same file_uri, reader.file
        assert_equal dir_uri, reader.dir
        assert_equal 'file.adoc', reader.path
      end

      test 'PreprocessorReader#push_include method should accept file as a top-level URI and compute dir and path' do
        file_uri = ::URI.parse 'http://example.com/index.adoc'
        dir_uri = ::URI.parse 'http://example.com'
        reader = empty_document.reader
        reader.push_include %w(one two three), file_uri
        assert_same file_uri, reader.file
        assert_equal dir_uri, reader.dir
        assert_equal 'index.adoc', reader.path
      end

      test 'PreprocessorReader#push_include method should not fail if data is nil' do
        lines = %w(a b c)
        doc = Asciidoctor::Document.new lines
        reader = doc.reader
        reader.push_include nil, '', '<stdin>'
        assert_equal 0, reader.include_stack.size
        assert_equal 'a', reader.read_line.rstrip
      end

      test 'PreprocessorReader#push_include method should ignore dot in directory name when computing include path' do
        lines = %w(a b c)
        doc = Asciidoctor::Document.new lines
        reader = doc.reader
        append_lines = %w(one two three)
        reader.push_include append_lines, nil, 'include.d/data'
        assert_nil reader.file
        assert_equal 'include.d/data', reader.path
        assert doc.catalog[:includes]['include.d/data']
      end
"#
);

non_normative!(
    r#"
    end

    context 'Include Directive' do
"#
);

// Section note — `Include Directive`.
//
// The distinct *preprocessor* behaviors of the include directive are ported to
// `verifies!` below: the secure-mode link-macro replacement (with `pass:c`
// space escaping), the "target does not match" cases, the escaped directive,
// and an indented (non-directive) line. The include *file-selection* engine —
// `lines=` ranges and `tag(s)=` region filtering (including the wildcard and
// negation forms), `indent`, and `leveloffset` — is verified comprehensively by
// this crate's own spec suites (`tests::asciidoc_lang::directives::include`,
// `include_lines`, `include_tagged_regions`, `include_with_indent`,
// `include_with_leveloffset`) and, for the tag engine specifically, by the
// include-tag warning cases already ported above; the many Ruby cases that
// re-exercise that same engine are therefore left `non_normative!` rather than
// duplicated here.
//
// The remaining cases are out of scope for this crate and stay `non_normative!`
// for the reasons noted inline or here: compat mode; UTF-8 BOM stripping;
// `uri:classloader:` and remote (URL) includes; the `doc.catalog[:includes]`
// registry; filesystem-relative / absolute-path / spaces-in-filename resolution
// and the `base_dir` sandbox; non-UTF-8 `encoding` handling; the blank-target
// reader diagnostic; and the Reader-object APIs (`read_lines_until`,
// `skip_comment_lines`).

// At the default safe mode (secure) the include directive is not expanded;
// it is replaced with a link to the target carrying the `include` role.
#[test]
fn should_replace_include_directive_with_link_macro_in_default_safe_mode() {
    verifies!(
        r#"
      test 'should replace include directive with link macro in default safe mode' do
        input = 'include::include-file.adoc[]'
        doc = Asciidoctor::Document.new input
        reader = doc.reader
        assert_equal 'link:include-file.adoc[role=include]', reader.read_line
      end

"#
    );

    assert_eq!(
        reader_read(&Parser::default(), "include::include-file.adoc[]"),
        "link:include-file.adoc[role=include]"
    );
}

non_normative!(
    r#"
      test 'should not add role to link macro used to replace include directive in compat mode' do
        input = 'include::include-file.adoc[]'
        doc = Asciidoctor::Document.new input, attributes: { 'compat-mode' => '' }
        reader = doc.reader
        assert_equal 'link:include-file.adoc[]', reader.read_line
      end

"#
);

// A target with spaces is wrapped in `pass:c[…]` so the space cannot break
// the generated link macro.
#[test]
fn should_escape_spaces_in_target_when_generating_link_from_include_directive() {
    verifies!(
        r#"
      test 'should escape spaces in target when generating link from include directive' do
        input = 'include::foo bar baz.adoc[]'
        doc = Asciidoctor::Document.new input
        reader = doc.reader
        assert_equal 'link:pass:c[foo bar baz.adoc][role=include]', reader.read_line
      end

"#
    );

    assert_eq!(
        reader_read(&Parser::default(), "include::foo bar baz.adoc[]"),
        "link:pass:c[foo bar baz.adoc][role=include]"
    );
}

// A remote (URI) target is link-replaced the same way at secure safe mode.
#[test]
fn should_replace_include_directive_with_link_macro_if_safe_mode_allows_it_but_allow_uri_read_is_not_set()
 {
    verifies!(
        r#"
      test 'should replace include directive with link macro if safe mode allows it, but allow-uri-read is not set' do
        using_memory_logger do |logger|
          input = 'include::https://example.org/dist/info.adoc[]'
          doc = Asciidoctor::Document.new input, safe: :safe
          reader = doc.reader
          assert_equal 'link:https://example.org/dist/info.adoc[role=include]', reader.read_line
          assert_empty logger
        end
      end

"#
    );

    assert_eq!(
        reader_read(
            &Parser::default(),
            "include::https://example.org/dist/info.adoc[]"
        ),
        "link:https://example.org/dist/info.adoc[role=include]"
    );
}

non_normative!(
    r#"
      test 'should not add role to link macro that replaces include directive with remote target in compat mode' do
        input = 'include::https://example.org/dist/info.adoc[]'
        doc = Asciidoctor::Document.new input, safe: :safe, attributes: { 'compat-mode' => '' }
        reader = doc.reader
        assert_equal 'link:https://example.org/dist/info.adoc[]', reader.read_line
      end

"#
);

#[test]
fn should_escape_spaces_in_target_when_generating_link_from_remote_include_directive() {
    verifies!(
        r#"
      test 'should escape spaces in target when generating link from remote include directive' do
        input = 'include::https://example.org/no such file.adoc[]'
        doc = Asciidoctor::Document.new input, safe: :safe
        reader = doc.reader
        assert_equal 'link:pass:c[https://example.org/no such file.adoc][role=include]', reader.read_line
      end

"#
    );

    assert_eq!(
        reader_read(
            &Parser::default(),
            "include::https://example.org/no such file.adoc[]"
        ),
        "link:pass:c[https://example.org/no such file.adoc][role=include]"
    );
}

// Below `SafeMode::Secure` the directive is *not* link-replaced (contrast the
// default-safe-mode case above): the handler is consulted and its content is
// merged in place. (The `doc.catalog[:includes]` assertion is the include
// registry, tracked separately by
// https://github.com/asciidoc-rs/asciidoc-parser/issues/335.)
#[test]
fn include_directive_is_enabled_when_safe_mode_is_less_than_secure() {
    verifies!(
        r#"
      test 'include directive is enabled when safe mode is less than SECURE' do
        input = 'include::fixtures/include-file.adoc[]'
        doc = document_from_string input, safe: :safe, standalone: false, base_dir: DIRNAME
        output = doc.convert
        assert_match(/included content/, output)
        assert doc.catalog[:includes]['fixtures/include-file']
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new("included content");
    let probe = handler.clone();
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);

    assert_eq!(
        reader_read(&parser, "include::fixtures/include-file.adoc[]"),
        "included content"
    );
    assert_eq!(
        probe.calls(),
        vec![(None, "fixtures/include-file.adoc".to_owned(), None)]
    );
}

non_normative!(
    r#"
      test 'should strip BOM from include file' do
        input = %(:showtitle:\ninclude::fixtures/file-with-utf8-bom.adoc[])
        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        assert_css '.paragraph', output, 0
        assert_css 'h1', output, 1
        assert_match(/<h1>人<\/h1>/, output)
      end

"#
);

non_normative!(
    r#"
      test 'should include content from a file on the classloader', if: jruby? do
        require fixture_path 'assets.jar'
        input = 'include::uri:classloader:/includes-in-jar/include-file.adoc[]'
        doc = document_from_string input, safe: :unsafe, standalone: false, base_dir: DIRNAME
        output = doc.convert
        assert_match(/included from a file/, output)
        assert doc.catalog[:includes]['uri:classloader:/includes-in-jar/include-file']
      end

"#
);

// Out of scope for now: asserts on `doc.catalog[:includes]`, the include/link
// registry. This crate does not yet maintain such a registry; it is tracked by
// https://github.com/asciidoc-rs/asciidoc-parser/issues/335.
non_normative!(
    r#"
      test 'should not track include in catalog for non-AsciiDoc include files' do
        input = <<~'EOS'
        ----
        include::fixtures/circle.svg[]
        ----
        EOS

        doc = document_from_string input, safe: :safe, standalone: false, base_dir: DIRNAME
        assert doc.catalog[:includes].empty?
      end

"#
);

// The actual file lookup (here, a file whose name contains a space) is
// downstream of this crate, so a mock handler stands in for it: the directive
// is expanded with the handler's content, and the parser hands the handler the
// verbatim target — spaces preserved.
#[test]
fn include_directive_should_resolve_file_with_spaces_in_name() {
    verifies!(
        r#"
      test 'include directive should resolve file with spaces in name' do
        input = 'include::fixtures/include file.adoc[]'
        include_file = File.join DIRNAME, 'fixtures', 'include-file.adoc'
        include_file_with_sp = File.join DIRNAME, 'fixtures', 'include file.adoc'
        begin
          FileUtils.cp include_file, include_file_with_sp
          doc = document_from_string input, safe: :safe, standalone: false, base_dir: DIRNAME
          output = doc.convert
          assert_match(/included content/, output)
        ensure
          FileUtils.rm include_file_with_sp
        end
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new("included content");
    let probe = handler.clone();
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);

    assert_eq!(
        reader_read(&parser, "include::fixtures/include file.adoc[]"),
        "included content"
    );
    assert_eq!(
        probe.calls(),
        vec![(None, "fixtures/include file.adoc".to_owned(), None)]
    );
}

// Same plumbing check, but the target contains a `{sp}` attribute reference:
// the parser resolves it (to a space) before handing the target to the handler,
// so the handler sees `fixtures/include file.adoc`.
#[test]
fn include_directive_should_resolve_file_with_sp_in_name() {
    verifies!(
        r#"
      test 'include directive should resolve file with {sp} in name' do
        input = 'include::fixtures/include{sp}file.adoc[]'
        include_file = File.join DIRNAME, 'fixtures', 'include-file.adoc'
        include_file_with_sp = File.join DIRNAME, 'fixtures', 'include file.adoc'
        begin
          FileUtils.cp include_file, include_file_with_sp
          doc = document_from_string input, safe: :safe, standalone: false, base_dir: DIRNAME
          output = doc.convert
          assert_match(/included content/, output)
        ensure
          FileUtils.rm include_file_with_sp
        end
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new("included content");
    let probe = handler.clone();
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);

    assert_eq!(
        reader_read(&parser, "include::fixtures/include{sp}file.adoc[]"),
        "included content"
    );
    assert_eq!(
        probe.calls(),
        vec![(None, "fixtures/include file.adoc".to_owned(), None)]
    );
}

// An `include::` directive whose target is empty or has a leading/trailing
// space does not match, so the line is left untouched.
#[test]
fn include_directive_should_not_match_if_target_is_empty_or_starts_or_ends_with_space() {
    verifies!(
        r#"
      test 'include directive should not match if target is empty or starts or ends with space' do
        ['include::[]', 'include:: []', 'include:: not-include[]', 'include::not-include []'].each do |input|
          doc = Asciidoctor::Document.new input
          reader = doc.reader
          assert_equal input, reader.read_line
        end
      end

"#
    );

    for input in [
        "include::[]",
        "include:: []",
        "include:: not-include[]",
        "include::not-include []",
    ] {
        assert_eq!(reader_read(&Parser::default(), input), input);
    }
}

non_normative!(
    r#"
      test 'include directive should not attempt to resolve target as remote if allow-uri-read is set and URL is not on first line' do
        using_memory_logger do |logger|
          input = <<~'EOS'
          :target: not-a-file.adoc + \
          http://example.org/team.adoc

          include::{target}[]
          EOS
          doc = Asciidoctor.load input, safe: :safe, base_dir: fixturedir
          lines = doc.blocks[0].lines
          assert_equal [%(Unresolved directive in <stdin> - include::not-a-file.adoc +\nhttp://example.org/team.adoc[])], lines
          assert_message logger, :ERROR, %(<stdin>: line 4: include file not found: #{fixture_path 'not-a-file.adoc'} +\nhttp://example.org/team.adoc), Hash
        end
      end

"#
);

non_normative!(
    r#"
      test 'include directive should resolve file relative to current include' do
        input = 'include::fixtures/parent-include.adoc[]'
        pseudo_docfile = File.join DIRNAME, 'main.adoc'
        fixtures_dir = File.join DIRNAME, 'fixtures'
        parent_include_docfile = File.join fixtures_dir, 'parent-include.adoc'
        child_include_docfile = File.join fixtures_dir, 'child-include.adoc'
        grandchild_include_docfile = File.join fixtures_dir, 'grandchild-include.adoc'

        doc = empty_safe_document base_dir: DIRNAME
        reader = Asciidoctor::PreprocessorReader.new doc, input, pseudo_docfile, normalize: true

        assert_equal pseudo_docfile, reader.file
        assert_equal DIRNAME, reader.dir
        assert_equal 'main.adoc', reader.path

        assert_equal 'first line of parent', reader.read_line

        assert_equal 'fixtures/parent-include.adoc: line 1', reader.cursor_at_prev_line.to_s
        assert_equal parent_include_docfile, reader.file
        assert_equal fixtures_dir, reader.dir
        assert_equal 'fixtures/parent-include.adoc', reader.path

        reader.skip_blank_lines

        assert_equal 'first line of child', reader.read_line

        assert_equal 'fixtures/child-include.adoc: line 1', reader.cursor_at_prev_line.to_s
        assert_equal child_include_docfile, reader.file
        assert_equal fixtures_dir, reader.dir
        assert_equal 'fixtures/child-include.adoc', reader.path

        reader.skip_blank_lines

        assert_equal 'first line of grandchild', reader.read_line

        assert_equal 'fixtures/grandchild-include.adoc: line 1', reader.cursor_at_prev_line.to_s
        assert_equal grandchild_include_docfile, reader.file
        assert_equal fixtures_dir, reader.dir
        assert_equal 'fixtures/grandchild-include.adoc', reader.path

        reader.skip_blank_lines

        assert_equal 'last line of grandchild', reader.read_line

        reader.skip_blank_lines

        assert_equal 'last line of child', reader.read_line

        reader.skip_blank_lines

        assert_equal 'last line of parent', reader.read_line

        assert_equal 'fixtures/parent-include.adoc: line 5', reader.cursor_at_prev_line.to_s
        assert_equal parent_include_docfile, reader.file
        assert_equal fixtures_dir, reader.dir
        assert_equal 'fixtures/parent-include.adoc', reader.path
      end

"#
);

non_normative!(
    r#"
      test 'include directive should process lines when file extension of target is .asciidoc' do
        input = 'include::fixtures/include-alt-extension.asciidoc[]'
        doc = document_from_string input, safe: :safe, base_dir: DIRNAME
        assert_equal 3, doc.blocks.size
        assert_equal ['first line'], doc.blocks[0].lines
        assert_equal ['Asciidoctor!'], doc.blocks[1].lines
        assert_equal ['last line'], doc.blocks[2].lines
      end

"#
);

non_normative!(
    r#"
      test 'should only strip trailing newlines, not trailing whitespace, if include file is not AsciiDoc' do
        input = <<~'EOS'
        ....
        include::fixtures/data.tsv[]
        ....
        EOS

        doc = document_from_string input, safe: :safe, base_dir: DIRNAME
        assert_equal 1, doc.blocks.size
        assert doc.blocks[0].lines[2].end_with? ?\t
      end

"#
);

non_normative!(
    r#"
      test 'should fail to read include file if not UTF-8 encoded and encoding is not specified' do
        input = <<~'EOS'
        ....
        include::fixtures/iso-8859-1.txt[]
        ....
        EOS

        assert_raises StandardError, 'invalid byte sequence in UTF-8' do
          doc = document_from_string input, safe: :safe, base_dir: DIRNAME
          assert_equal 1, doc.blocks.size
          refute_equal ['Où est l\'hôpital ?'], doc.blocks[0].lines
          doc.convert
        end
      end

"#
);

non_normative!(
    r#"
      test 'should ignore encoding attribute if value is not a valid encoding' do
        input = <<~'EOS'
        ....
        include::fixtures/encoding.adoc[tag=romé,encoding=iso-1000-1]
        ....
        EOS

        doc = document_from_string input, safe: :safe, base_dir: DIRNAME
        assert_equal 1, doc.blocks.size
        assert_equal doc.blocks[0].lines[0].encoding, Encoding::UTF_8
        assert_equal ['Gregory Romé has written an AsciiDoc plugin for the Redmine project management application.'], doc.blocks[0].lines
      end

"#
);

// The actual transcoding is the handler's job, but the parser must forward the
// `encoding` attribute so the handler knows the source format. The mock records
// that it received `encoding=iso-8859-1` and returns its (already UTF-8)
// content via `IncludeContent::transcoded`; the parser merges it and — because
// the handler honored the encoding — raises no non-UTF-8 warning.
#[test]
fn should_use_encoding_specified_by_encoding_attribute_when_reading_include_file() {
    verifies!(
        r#"
      test 'should use encoding specified by encoding attribute when reading include file' do
        input = <<~'EOS'
        ....
        include::fixtures/iso-8859-1.txt[encoding=iso-8859-1]
        ....
        EOS

        doc = document_from_string input, safe: :safe, base_dir: DIRNAME
        assert_equal 1, doc.blocks.size
        assert_equal doc.blocks[0].lines[0].encoding, Encoding::UTF_8
        assert_equal ['Où est l\'hôpital ?'], doc.blocks[0].lines
      end

"#
    );

    let handler = RecordingIncludeFileHandler::transcoding("Où est l'hôpital ?");
    let probe = handler.clone();
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);

    let (output, _source_map, warnings, _includes) = crate::parser::preprocessor::preprocess(
        "include::fixtures/iso-8859-1.txt[encoding=iso-8859-1]",
        &parser,
    );
    assert_eq!(output, "Où est l'hôpital ?\n");

    // The `encoding` attribute was forwarded to the handler ...
    assert_eq!(
        probe.calls(),
        vec![(
            None,
            "fixtures/iso-8859-1.txt".to_owned(),
            Some("iso-8859-1".to_owned())
        )]
    );

    // ... and because the handler reported the content as transcoded, no
    // non-UTF-8 include-encoding warning is raised.
    assert!(warnings.is_empty());
}

// With `opts=optional`, a target the handler cannot resolve (here `None`) is
// dropped silently — no "Unresolved directive" text and no warning — leaving
// the following content in place. (Asciidoctor additionally logs an INFO-level
// notice that the optional include was dropped; this crate's warning mechanism
// is reserved for parse-affecting conditions and does not model that notice.)
#[test]
fn unresolved_target_referenced_by_include_directive_is_skipped_when_optional_option_is_set() {
    verifies!(
        r#"
      test 'unresolved target referenced by include directive is skipped when optional option is set' do
        input = <<~'EOS'
        include::fixtures/{no-such-file}[opts=optional]

        trailing content
        EOS

        begin
          using_memory_logger do |logger|
            doc = document_from_string input, safe: :safe, base_dir: DIRNAME
            assert_equal 1, doc.blocks.size
            assert_equal ['trailing content'], doc.blocks[0].lines
            assert_message logger, :INFO, '~<stdin>: line 1: optional include dropped because include file not found', Hash
          end
        rescue
          flunk 'include directive should not raise exception on unresolved target'
        end
      end

"#
    );

    let handler = RecordingIncludeFileHandler::missing();
    let probe = handler.clone();
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);

    let (output, _source_map, warnings, _includes) = crate::parser::preprocessor::preprocess(
        "include::fixtures/{no-such-file}[opts=optional]\n\ntrailing content",
        &parser,
    );

    // The unresolvable optional include leaves only the trailing content.
    assert_eq!(output, "\ntrailing content\n");

    // The handler was consulted (and reported the file missing) ...
    assert_eq!(
        probe.calls(),
        vec![(None, "fixtures/{no-such-file}".to_owned(), None)]
    );

    // ... and because the option was `optional`, no warning was raised.
    assert!(warnings.is_empty());
}

// Twin of the previous case with a concrete filename: an unresolvable
// `opts=optional` include is dropped silently.
#[test]
fn should_skip_include_directive_that_references_missing_file_if_optional_option_is_set() {
    verifies!(
        r#"
      test 'should skip include directive that references missing file if optional option is set' do
        input = <<~'EOS'
        include::fixtures/no-such-file.adoc[opts=optional]

        trailing content
        EOS

        begin
          using_memory_logger do |logger|
            doc = document_from_string input, safe: :safe, base_dir: DIRNAME
            assert_equal 1, doc.blocks.size
            assert_equal ['trailing content'], doc.blocks[0].lines
            assert_message logger, :INFO, '~<stdin>: line 1: optional include dropped because include file not found', Hash
          end
        rescue
          flunk 'include directive should not raise exception on missing file'
        end
      end

"#
    );

    let handler = RecordingIncludeFileHandler::missing();
    let probe = handler.clone();
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    let (output, _source_map, warnings, _includes) = crate::parser::preprocessor::preprocess(
        "include::fixtures/no-such-file.adoc[opts=optional]\n\ntrailing content",
        &parser,
    );
    assert_eq!(output, "\ntrailing content\n");
    assert_eq!(
        probe.calls(),
        vec![(None, "fixtures/no-such-file.adoc".to_owned(), None)]
    );
    assert!(warnings.is_empty());
}

// Without `optional`, an unresolvable include is replaced by an "Unresolved
// directive" message and an `IncludeFileNotFound` warning. (Asciidoctor names
// the including file `<stdin>`; with no primary file name this crate writes
// `(root file)`.)
#[test]
fn should_replace_include_directive_that_references_missing_file_with_message() {
    verifies!(
        r#"
      test 'should replace include directive that references missing file with message' do
        input = <<~'EOS'
        include::fixtures/no-such-file.adoc[]

        trailing content
        EOS

        begin
          using_memory_logger do |logger|
            doc = document_from_string input, safe: :safe, base_dir: DIRNAME
            assert_equal 2, doc.blocks.size
            assert_equal ['Unresolved directive in <stdin> - include::fixtures/no-such-file.adoc[]'], doc.blocks[0].lines
            assert_equal ['trailing content'], doc.blocks[1].lines
            assert_message logger, :ERROR, '~<stdin>: line 1: include file not found', Hash
          end
        rescue
          flunk 'include directive should not raise exception on missing file'
        end
      end

"#
    );

    let handler = RecordingIncludeFileHandler::missing();
    let probe = handler.clone();
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    let (output, _source_map, warnings, _includes) = crate::parser::preprocessor::preprocess(
        "include::fixtures/no-such-file.adoc[]\n\ntrailing content",
        &parser,
    );
    assert_eq!(
        output,
        "Unresolved directive in (root file) - include::fixtures/no-such-file.adoc[]\n\ntrailing content\n"
    );
    assert_eq!(
        probe.calls(),
        vec![(None, "fixtures/no-such-file.adoc".to_owned(), None)]
    );
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0].warning,
        WarningType::IncludeFileNotFound("fixtures/no-such-file.adoc".to_owned())
    );
}

// This crate delegates file access to the handler, which signals both a
// missing and an unreadable file the same way (by returning `None`), so an
// unreadable target is reported with the same `IncludeFileNotFound` warning
// as a missing one — it does not distinguish Asciidoctor's separate "include
// file not readable" message.
#[test]
fn should_replace_include_directive_that_references_unreadable_file_with_message() {
    verifies!(
        r#"
      test 'should replace include directive that references unreadable file with message', unless: (windows? || Process.euid == 0) do
        include_file = File.join DIRNAME, 'fixtures', 'chapter-a.adoc'
        old_mode = (File.stat include_file).mode
        FileUtils.chmod 0o000, include_file
        input = <<~'EOS'
        include::fixtures/chapter-a.adoc[]

        trailing content
        EOS

        begin
          using_memory_logger do |logger|
            doc = document_from_string input, safe: :safe, base_dir: DIRNAME
            assert_equal 2, doc.blocks.size
            assert_equal ['Unresolved directive in <stdin> - include::fixtures/chapter-a.adoc[]'], doc.blocks[0].lines
            assert_equal ['trailing content'], doc.blocks[1].lines
            assert_message logger, :ERROR, '~<stdin>: line 1: include file not readable', Hash
          end
        rescue
          flunk 'include directive should not raise exception on missing file'
        ensure
          FileUtils.chmod old_mode, include_file
        end
      end
"#
    );

    let handler = RecordingIncludeFileHandler::missing();
    let probe = handler.clone();
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    let (output, _source_map, warnings, _includes) = crate::parser::preprocessor::preprocess(
        "include::fixtures/chapter-a.adoc[]\n\ntrailing content",
        &parser,
    );
    assert_eq!(
        output,
        "Unresolved directive in (root file) - include::fixtures/chapter-a.adoc[]\n\ntrailing content\n"
    );
    assert_eq!(
        probe.calls(),
        vec![(None, "fixtures/chapter-a.adoc".to_owned(), None)]
    );
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0].warning,
        WarningType::IncludeFileNotFound("fixtures/chapter-a.adoc".to_owned())
    );
}

non_normative!(
    r#"

      # IMPORTANT this test needs to be run on Windows to verify proper behavior in Windows
"#
);

// An absolute target is passed through to the handler verbatim (path
// resolution is the handler's concern).
#[test]
fn can_resolve_include_directive_with_absolute_path() {
    verifies!(
        r#"
      test 'can resolve include directive with absolute path' do
        include_path = ::File.join DIRNAME, 'fixtures', 'chapter-a.adoc'
        input = %(include::#{include_path}[])
        result = document_from_string input, safe: :safe
        assert_equal 'Chapter A', result.doctitle

        result = document_from_string input, safe: :unsafe, base_dir: ::Dir.tmpdir
        assert_equal 'Chapter A', result.doctitle
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new("= Chapter A");
    let probe = handler.clone();
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    assert_eq!(
        reader_read(&parser, "include::/abs/fixtures/chapter-a.adoc[]"),
        "= Chapter A"
    );
    assert_eq!(
        probe.calls(),
        vec![(None, "/abs/fixtures/chapter-a.adoc".to_owned(), None)]
    );
}

non_normative!(
    r#"
      test 'include directive can retrieve data from uri' do
        url = %(http://#{resolve_localhost}:9876/name/asciidoctor)
        input = <<~EOS
        ....
        include::#{url}[]
        ....
        EOS
        expect = /\{"name": "asciidoctor"\}/
        output = using_test_webserver do
          convert_string_to_embedded input, safe: :safe, attributes: { 'allow-uri-read' => '' }
        end

        refute_nil output
        assert_match(expect, output)
      end

"#
);

non_normative!(
    r#"
      test 'nested include directives are resolved relative to current file' do
        input = <<~'EOS'
        ....
        include::fixtures/outer-include.adoc[]
        ....
        EOS

        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        expected = <<~'EOS'.chop
        first line of outer

        first line of middle

        first line of inner

        last line of inner

        last line of middle

        last line of outer
        EOS
        assert_includes output, expected
      end

"#
);

non_normative!(
    r#"
      test 'nested remote include directive is resolved relative to uri of current file' do
        url = %(http://#{resolve_localhost}:9876/fixtures/outer-include.adoc)
        input = <<~EOS
        ....
        include::#{url}[]
        ....
        EOS
        output = using_test_webserver do
          convert_string_to_embedded input, safe: :safe, attributes: { 'allow-uri-read' => '' }
        end

        expected = <<~'EOS'.chop
        first line of outer

        first line of middle

        first line of inner

        last line of inner

        last line of middle

        last line of outer
        EOS
        assert_includes output, expected
      end

"#
);

non_normative!(
    r#"
      test 'nested remote include directive that cannot be resolved does not crash processor' do
        include_url = %(http://#{resolve_localhost}:9876/fixtures/file-with-missing-include.adoc)
        nested_include_url = 'no-such-file.adoc'
        input = <<~EOS
        ....
        include::#{include_url}[]
        ....
        EOS
        begin
          using_memory_logger do |logger|
            result = using_test_webserver do
              convert_string_to_embedded input, safe: :safe, attributes: { 'allow-uri-read' => '' }
            end
            assert_includes result, %(Unresolved directive in #{include_url} - include::#{nested_include_url}[])
            assert_message logger, :ERROR, %(#{include_url}: line 1: include uri not readable: http://#{resolve_localhost}:9876/fixtures/#{nested_include_url}), Hash
          end
        rescue
          flunk 'include directive should not raise exception on missing file'
        end
      end

"#
);

non_normative!(
    r#"
      test 'should support tag filtering for remote includes' do
        url = %(http://#{resolve_localhost}:9876/fixtures/tagged-class.rb)
        input = <<~EOS
        [source,ruby]
        ----
        include::#{url}[tag=init,indent=0]
        ----
        EOS
        output = using_test_webserver do
          convert_string_to_embedded input, safe: :safe, attributes: { 'allow-uri-read' => '' }
        end

        # NOTE cannot use single-quoted heredoc because of https://github.com/jruby/jruby/issues/4260
        expected = <<~EOS.chop
        <code class="language-ruby" data-lang="ruby">def initialize breed
          @breed = breed
        end</code>
        EOS
        assert_includes output, expected
      end

"#
);

non_normative!(
    r#"
      test 'should not crash if include directive references inaccessible uri' do
        url = %(http://#{resolve_localhost}:9876/no_such_file)
        input = <<~EOS
        ....
        include::#{url}[]
        ....
        EOS

        begin
          using_memory_logger do |logger|
            output = using_test_webserver do
              convert_string_to_embedded input, safe: :safe, attributes: { 'allow-uri-read' => '' }
            end
            refute_nil output
            assert_match(/Unresolved directive/, output)
            assert_message logger, :ERROR, %(<stdin>: line 2: include uri not readable: #{url}), Hash
          end
        rescue
          flunk 'include directive should not raise exception on inaccessible uri'
        end
      end

"#
);

// Line selection: `lines=1;3..4;6..-1` keeps line 1, lines 3-4, and line 6
// through the end of the file (the mock supplies the fixture text). The parser
// applies the selection to the handler's content; the handler itself sees only
// the resolved target.
#[test]
fn include_directive_supports_selecting_lines_by_line_number() {
    verifies!(
        r#"
      test 'include directive supports selecting lines by line number' do
        input = 'include::fixtures/include-file.adoc[lines=1;3..4;6..-1]'
        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        assert_match(/first line/, output)
        refute_match(/second line/, output)
        assert_match(/third line/, output)
        assert_match(/fourth line/, output)
        refute_match(/fifth line/, output)
        assert_match(/sixth line/, output)
        assert_match(/seventh line/, output)
        assert_match(/eighth line/, output)
        assert_match(/last line of included content/, output)
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(INCLUDE_FILE_ADOC);
    let probe = handler.clone();
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);

    let output = reader_read(
        &parser,
        "include::fixtures/include-file.adoc[lines=1;3..4;6..-1]",
    );

    assert!(output.contains("first line"));
    assert!(!output.contains("second line"));
    assert!(output.contains("third line"));
    assert!(output.contains("fourth line"));
    assert!(!output.contains("fifth line"));
    assert!(output.contains("sixth line"));
    assert!(output.contains("seventh line"));
    assert!(output.contains("eighth line"));
    assert!(output.contains("last line of included content"));

    assert_eq!(
        probe.calls(),
        vec![(None, "fixtures/include-file.adoc".to_owned(), None)]
    );
}

// A quoted `lines` value may use commas between ranges (same selection as
// the semicolon form).
#[test]
fn include_directive_supports_line_ranges_separated_by_commas_in_quoted_attribute_value() {
    verifies!(
        r#"
      test 'include directive supports line ranges separated by commas in quoted attribute value' do
        input = 'include::fixtures/include-file.adoc[lines="1,3..4,6..-1"]'
        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        assert_match(/first line/, output)
        refute_match(/second line/, output)
        assert_match(/third line/, output)
        assert_match(/fourth line/, output)
        refute_match(/fifth line/, output)
        assert_match(/sixth line/, output)
        assert_match(/seventh line/, output)
        assert_match(/eighth line/, output)
        assert_match(/last line of included content/, output)
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(INCLUDE_FILE_ADOC);
    let probe = handler.clone();
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    let output = reader_read(
        &parser,
        "include::fixtures/include-file.adoc[lines=\"1,3..4,6..-1\"]",
    );
    assert!(output.contains("first line"));
    assert!(!output.contains("second line"));
    assert!(output.contains("third line"));
    assert!(output.contains("fourth line"));
    assert!(!output.contains("fifth line"));
    assert!(output.contains("sixth line"));
    assert!(output.contains("seventh line"));
    assert!(output.contains("eighth line"));
    assert!(output.contains("last line of included content"));
    assert_eq!(
        probe.calls(),
        vec![(None, "fixtures/include-file.adoc".to_owned(), None)]
    );
}

// Spaces around the range separators and the `..` operator are ignored.
#[test]
fn include_directive_ignores_spaces_between_line_ranges_in_quoted_attribute_value() {
    verifies!(
        r#"
      test 'include directive ignores spaces between line ranges in quoted attribute value' do
        input = 'include::fixtures/include-file.adoc[lines="1, 3..4 , 6 .. -1"]'
        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        assert_match(/first line/, output)
        refute_match(/second line/, output)
        assert_match(/third line/, output)
        assert_match(/fourth line/, output)
        refute_match(/fifth line/, output)
        assert_match(/sixth line/, output)
        assert_match(/seventh line/, output)
        assert_match(/eighth line/, output)
        assert_match(/last line of included content/, output)
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(INCLUDE_FILE_ADOC);
    let probe = handler.clone();
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    let output = reader_read(
        &parser,
        "include::fixtures/include-file.adoc[lines=\"1, 3..4 , 6 .. -1\"]",
    );
    assert!(output.contains("first line"));
    assert!(!output.contains("second line"));
    assert!(output.contains("third line"));
    assert!(output.contains("fourth line"));
    assert!(!output.contains("fifth line"));
    assert!(output.contains("sixth line"));
    assert!(output.contains("seventh line"));
    assert!(output.contains("eighth line"));
    assert!(output.contains("last line of included content"));
    assert_eq!(
        probe.calls(),
        vec![(None, "fixtures/include-file.adoc".to_owned(), None)]
    );
}

// `6..` (no end) selects from line 6 to the end of the file.
#[test]
fn include_directive_supports_implicit_endless_range() {
    verifies!(
        r#"
      test 'include directive supports implicit endless range' do
        input = 'include::fixtures/include-file.adoc[lines=6..]'
        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        refute_match(/first line/, output)
        refute_match(/second line/, output)
        refute_match(/third line/, output)
        refute_match(/fourth line/, output)
        refute_match(/fifth line/, output)
        assert_match(/sixth line/, output)
        assert_match(/seventh line/, output)
        assert_match(/eighth line/, output)
        assert_match(/last line of included content/, output)
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(INCLUDE_FILE_ADOC);
    let probe = handler.clone();
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    let output = reader_read(&parser, "include::fixtures/include-file.adoc[lines=6..]");
    assert!(!output.contains("first line"));
    assert!(!output.contains("second line"));
    assert!(!output.contains("third line"));
    assert!(!output.contains("fourth line"));
    assert!(!output.contains("fifth line"));
    assert!(output.contains("sixth line"));
    assert!(output.contains("seventh line"));
    assert!(output.contains("eighth line"));
    assert!(output.contains("last line of included content"));
    assert_eq!(
        probe.calls(),
        vec![(None, "fixtures/include-file.adoc".to_owned(), None)]
    );
}

// An empty `lines=` applies no selection, so the whole file is included.
#[test]
fn include_directive_ignores_lines_attribute_if_empty() {
    verifies!(
        r#"
      test 'include directive ignores lines attribute if empty' do
        input = <<~'EOS'
        ++++
        include::fixtures/include-file.adoc[lines=]
        ++++
        EOS

        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        assert_includes output, 'first line of included content'
        assert_includes output, 'last line of included content'
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(INCLUDE_FILE_ADOC);
    let probe = handler.clone();
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    let output = reader_read(
        &parser,
        "++++\ninclude::fixtures/include-file.adoc[lines=]\n++++",
    );
    assert!(output.contains("first line of included content"));
    assert!(output.contains("last line of included content"));
    assert_eq!(
        probe.calls(),
        vec![(None, "fixtures/include-file.adoc".to_owned(), None)]
    );
}

// A reversed range like `10..5` selects no lines, so it is treated as
// invalid and the `lines` attribute is ignored — the whole file is
// included. (This crate previously applied the reversed range and produced
// empty output; it now matches Asciidoctor.)
#[test]
fn include_directive_ignores_lines_attribute_with_invalid_range() {
    verifies!(
        r#"
      test 'include directive ignores lines attribute with invalid range' do
        input = <<~'EOS'
        ++++
        include::fixtures/include-file.adoc[lines=10..5]
        ++++
        EOS

        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        assert_includes output, 'first line of included content'
        assert_includes output, 'last line of included content'
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(INCLUDE_FILE_ADOC);
    let probe = handler.clone();
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    let output = reader_read(
        &parser,
        "++++\ninclude::fixtures/include-file.adoc[lines=10..5]\n++++",
    );
    assert!(output.contains("first line of included content"));
    assert!(output.contains("last line of included content"));
    assert_eq!(
        probe.calls(),
        vec![(None, "fixtures/include-file.adoc".to_owned(), None)]
    );
}

// A single `tag=` selects only that region's content.
#[test]
fn include_directive_supports_selecting_lines_by_tag() {
    verifies!(
        r#"
      test 'include directive supports selecting lines by tag' do
        input = 'include::fixtures/include-file.adoc[tag=snippetA]'
        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        assert_match(/snippetA content/, output)
        refute_match(/snippetB content/, output)
        refute_match(/non-tagged content/, output)
        refute_match(/included content/, output)
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(INCLUDE_FILE_ADOC);
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    assert_eq!(
        reader_read(&parser, "include::fixtures/include-file.adoc[tag=snippetA]"),
        "snippetA content"
    );
}

// `tags=a;b` selects both regions.
#[test]
fn include_directive_supports_selecting_lines_by_tags() {
    verifies!(
        r#"
      test 'include directive supports selecting lines by tags' do
        input = 'include::fixtures/include-file.adoc[tags=snippetA;snippetB]'
        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        assert_match(/snippetA content/, output)
        assert_match(/snippetB content/, output)
        refute_match(/non-tagged content/, output)
        refute_match(/included content/, output)
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(INCLUDE_FILE_ADOC);
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    assert_eq!(
        reader_read(
            &parser,
            "include::fixtures/include-file.adoc[tags=snippetA;snippetB]"
        ),
        "snippetA content\nsnippetB content"
    );
}

// Tag directives are recognized inside circumfix comments (XML, OCaml, JSX),
// and `indent=0` on the directive strips the region's indentation.
#[test]
fn include_directive_supports_selecting_lines_by_tag_in_language_that_uses_circumfix_comments() {
    verifies!(
        r#"
      test 'include directive supports selecting lines by tag in language that uses circumfix comments' do
        {
          'include-file.xml' => '<snippet>content</snippet>',
          'include-file.ml' => 'let s = SS.empty;;',
          'include-file.jsx' => '<p>Welcome to the club.</p>',
        }.each do |filename, expect|
          input = <<~EOS
          [source,xml]
          ----
          include::fixtures/#{filename}[tag=snippet,indent=0]
          ----
          EOS

          doc = document_from_string input, safe: :safe, base_dir: DIRNAME
          assert_equal expect, doc.blocks[0].source
        end
      end

"#
    );

    for (content, input, expected) in [
        (
            INCLUDE_FILE_XML,
            "[source,xml]\n----\ninclude::fixtures/include-file.xml[tag=snippet,indent=0]\n----",
            "[source,xml]\n----\n<snippet>content</snippet>\n----",
        ),
        (
            INCLUDE_FILE_ML,
            "[source,xml]\n----\ninclude::fixtures/include-file.ml[tag=snippet,indent=0]\n----",
            "[source,xml]\n----\nlet s = SS.empty;;\n----",
        ),
        (
            INCLUDE_FILE_JSX,
            "[source,xml]\n----\ninclude::fixtures/include-file.jsx[tag=snippet,indent=0]\n----",
            "[source,xml]\n----\n<p>Welcome to the club.</p>\n----",
        ),
    ] {
        let handler = RecordingIncludeFileHandler::new(content);
        let parser = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_include_file_handler(handler);
        assert_eq!(reader_read(&parser, input), expected);
    }
}

// Tag directives are matched even when the include file has CRLF endings.
#[test]
fn include_directive_supports_selecting_lines_by_tag_in_file_that_has_crlf_line_endings() {
    verifies!(
        r#"
      test 'include directive supports selecting lines by tag in file that has CRLF line endings' do
        begin
          tmp_include = Tempfile.new %w(include- .adoc)
          tmp_include_dir, tmp_include_path = File.split tmp_include.path
          tmp_include.write %(do not include\r\ntag::include-me[]\r\nincluded line\r\nend::include-me[]\r\ndo not include\r\n)
          tmp_include.close
          input = %(include::#{tmp_include_path}[tag=include-me])
          output = convert_string_to_embedded input, safe: :safe, base_dir: tmp_include_dir
          assert_includes output, 'included line'
          refute_includes output, 'do not include'
        ensure
          tmp_include.close!
        end
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(
        "do not include\r\ntag::include-me[]\r\nincluded line\r\nend::include-me[]\r\ndo not include\r\n",
    );
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    assert_eq!(
        reader_read(&parser, "include::fixtures/include.adoc[tag=include-me]"),
        "included line"
    );
}

// A closing tag on the final line (no trailing newline) is still recognized.
#[test]
fn include_directive_finds_closing_tag_on_last_line_of_file_without_a_trailing_newline() {
    verifies!(
        r#"
      test 'include directive finds closing tag on last line of file without a trailing newline' do
        begin
          tmp_include = Tempfile.new %w(include- .adoc)
          tmp_include_dir, tmp_include_path = File.split tmp_include.path
          tmp_include.write %(line not included\ntag::include-me[]\nline included\nend::include-me[])
          tmp_include.close
          input = %(include::#{tmp_include_path}[tag=include-me])
          using_memory_logger do |logger|
            output = convert_string_to_embedded input, safe: :safe, base_dir: tmp_include_dir
            assert_empty logger.messages
            assert_includes output, 'line included'
            refute_includes output, 'line not included'
          end
        ensure
          tmp_include.close!
        end
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(
        "line not included\ntag::include-me[]\nline included\nend::include-me[]",
    );
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    assert_eq!(
        reader_read(&parser, "include::fixtures/include.adoc[tag=include-me]"),
        "line included"
    );
}

// The tag-directive lines within a selected region are themselves dropped.
#[test]
fn include_directive_does_not_select_lines_containing_tag_directives_within_selected_tag_region() {
    verifies!(
        r#"
      test 'include directive does not select lines containing tag directives within selected tag region' do
        input = <<~'EOS'
        ++++
        include::fixtures/include-file.adoc[tags=snippet]
        ++++
        EOS

        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        expected = <<~'EOS'.chop
        snippetA content

        non-tagged content

        snippetB content
        EOS
        assert_equal expected, output
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(INCLUDE_FILE_ADOC);
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    assert_eq!(
        reader_read(
            &parser,
            "++++\ninclude::fixtures/include-file.adoc[tags=snippet]\n++++"
        ),
        "++++\nsnippetA content\n\nnon-tagged content\n\nsnippetB content\n++++"
    );
}

// `reader_read` is the raw preprocessed text, so it keeps the block
// delimiters and any leading/trailing blank line that the enclosing block
// trims when rendered; the tag *selection* is what is verified here.
#[test]
fn include_directive_skips_lines_inside_tag_which_is_negated() {
    verifies!(
        r#"
      test 'include directive skips lines inside tag which is negated' do
        input = <<~'EOS'
        ----
        include::fixtures/tagged-class-enclosed.rb[tags=all;!bark]
        ----
        EOS

        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        # NOTE cannot use single-quoted heredoc because of https://github.com/jruby/jruby/issues/4260
        expected = <<~EOS.chop
        class Dog
          def initialize breed
            @breed = breed
          end
        end
        EOS
        assert_includes output, %(<pre>#{expected}</pre>)
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(TAGGED_CLASS_ENCLOSED_RB);
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    assert_eq!(
        reader_read(
            &parser,
            "----\ninclude::fixtures/tagged-class-enclosed.rb[tags=all;!bark]\n----"
        ),
        "----\nclass Dog\n  def initialize breed\n    @breed = breed\n  end\nend\n----"
    );
}

// `reader_read` is the raw preprocessed text, so it keeps the block
// delimiters and any leading/trailing blank line that the enclosing block
// trims when rendered; the tag *selection* is what is verified here.
#[test]
fn include_directive_selects_all_lines_without_a_tag_directive_when_value_is_double_asterisk() {
    verifies!(
        r#"
      test 'include directive selects all lines without a tag directive when value is double asterisk' do
        input = <<~'EOS'
        ----
        include::fixtures/tagged-class.rb[tags=**]
        ----
        EOS

        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        # NOTE cannot use single-quoted heredoc because of https://github.com/jruby/jruby/issues/4260
        expected = <<~EOS.chop
        class Dog
          def initialize breed
            @breed = breed
          end

          def bark
            if @breed == 'beagle'
              'woof woof woof woof woof'
            else
              'woof woof'
            end
          end
        end
        EOS
        assert_includes output, %(<pre>#{expected}</pre>)
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(TAGGED_CLASS_RB);
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    assert_eq!(
        reader_read(
            &parser,
            "----\ninclude::fixtures/tagged-class.rb[tags=**]\n----"
        ),
        "----\nclass Dog\n  def initialize breed\n    @breed = breed\n  end\n\n  def bark\n    if @breed == 'beagle'\n      'woof woof woof woof woof'\n    else\n      'woof woof'\n    end\n  end\nend\n----"
    );
}

// `reader_read` is the raw preprocessed text, so it keeps the block
// delimiters and any leading/trailing blank line that the enclosing block
// trims when rendered; the tag *selection* is what is verified here.
#[test]
fn include_directive_selects_all_lines_except_lines_inside_tag_which_is_negated_when_value_starts_with_double_asterisk()
 {
    verifies!(
        r#"
      test 'include directive selects all lines except lines inside tag which is negated when value starts with double asterisk' do
        input = <<~'EOS'
        ----
        include::fixtures/tagged-class.rb[tags=**;!bark]
        ----
        EOS

        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        # NOTE cannot use single-quoted heredoc because of https://github.com/jruby/jruby/issues/4260
        expected = <<~EOS.chop
        class Dog
          def initialize breed
            @breed = breed
          end
        end
        EOS
        assert_includes output, %(<pre>#{expected}</pre>)
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(TAGGED_CLASS_RB);
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    assert_eq!(
        reader_read(
            &parser,
            "----\ninclude::fixtures/tagged-class.rb[tags=**;!bark]\n----"
        ),
        "----\nclass Dog\n  def initialize breed\n    @breed = breed\n  end\nend\n----"
    );
}

// `reader_read` is the raw preprocessed text, so it keeps the block
// delimiters and any leading/trailing blank line that the enclosing block
// trims when rendered; the tag *selection* is what is verified here.
#[test]
fn include_directive_selects_all_lines_including_lines_inside_nested_tags_except_lines_inside_tag_which_is_negated_when_value_starts_with_double_asterisk()
 {
    verifies!(
        r#"
      test 'include directive selects all lines, including lines inside nested tags, except lines inside tag which is negated when value starts with double asterisk' do
        input = <<~'EOS'
        ----
        include::fixtures/tagged-class.rb[tags=**;!init]
        ----
        EOS

        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        # NOTE cannot use single-quoted heredoc because of https://github.com/jruby/jruby/issues/4260
        expected = <<~EOS.chop
        class Dog

          def bark
            if @breed == 'beagle'
              'woof woof woof woof woof'
            else
              'woof woof'
            end
          end
        end
        EOS
        assert_includes output, %(<pre>#{expected}</pre>)
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(TAGGED_CLASS_RB);
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    assert_eq!(
        reader_read(
            &parser,
            "----\ninclude::fixtures/tagged-class.rb[tags=**;!init]\n----"
        ),
        "----\nclass Dog\n\n  def bark\n    if @breed == 'beagle'\n      'woof woof woof woof woof'\n    else\n      'woof woof'\n    end\n  end\nend\n----"
    );
}

// `reader_read` is the raw preprocessed text, so it keeps the block
// delimiters and any leading/trailing blank line that the enclosing block
// trims when rendered; the tag *selection* is what is verified here.
#[test]
fn include_directive_selects_all_lines_outside_of_tags_when_value_is_double_asterisk_followed_by_negated_wildcard()
 {
    verifies!(
        r#"
      test 'include directive selects all lines outside of tags when value is double asterisk followed by negated wildcard' do
        input = <<~'EOS'
        ----
        include::fixtures/tagged-class.rb[tags=**;!*]
        ----
        EOS

        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        expected = <<~'EOS'.chop
        class Dog
        end
        EOS
        assert_includes output, %(<pre>#{expected}</pre>)
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(TAGGED_CLASS_RB);
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    assert_eq!(
        reader_read(
            &parser,
            "----\ninclude::fixtures/tagged-class.rb[tags=**;!*]\n----"
        ),
        "----\nclass Dog\nend\n----"
    );
}

// `reader_read` is the raw preprocessed text, so it keeps the block
// delimiters and any leading/trailing blank line that the enclosing block
// trims when rendered; the tag *selection* is what is verified here.
#[test]
fn include_directive_skips_all_tagged_regions_when_value_of_tags_attribute_is_negated_wildcard() {
    verifies!(
        r#"
      test 'include directive skips all tagged regions when value of tags attribute is negated wildcard' do
        input = <<~'EOS'
        ----
        include::fixtures/tagged-class.rb[tags=!*]
        ----
        EOS

        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        expected = %(class Dog\nend)
        assert_includes output, %(<pre>#{expected}</pre>)
      end
"#
    );

    let handler = RecordingIncludeFileHandler::new(TAGGED_CLASS_RB);
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    assert_eq!(
        reader_read(
            &parser,
            "----\ninclude::fixtures/tagged-class.rb[tags=!*]\n----"
        ),
        "----\nclass Dog\nend\n----"
    );
}

non_normative!(
    r#"

      # FIXME this is a weird one since we'd expect it to only select the specified tags; but it's always been this way
"#
);

// `reader_read` is the raw preprocessed text, so it keeps the block
// delimiters and any leading/trailing blank line that the enclosing block
// trims when rendered; the tag *selection* is what is verified here.
#[test]
fn include_directive_selects_all_lines_except_for_lines_containing_tag_directive_if_value_is_double_asterisk_followed_by_nested_tag_names()
 {
    verifies!(
        r#"
      test 'include directive selects all lines except for lines containing tag directive if value is double asterisk followed by nested tag names' do
        input = <<~'EOS'
        ----
        include::fixtures/tagged-class.rb[tags=**;bark-beagle;bark-all]
        ----
        EOS

        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        # NOTE cannot use single-quoted heredoc because of https://github.com/jruby/jruby/issues/4260
        expected = <<~EOS.chop
        class Dog
          def initialize breed
            @breed = breed
          end

          def bark
            if @breed == 'beagle'
              'woof woof woof woof woof'
            else
              'woof woof'
            end
          end
        end
        EOS
        assert_includes output, %(<pre>#{expected}</pre>)
      end
"#
    );

    let handler = RecordingIncludeFileHandler::new(TAGGED_CLASS_RB);
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    assert_eq!(
        reader_read(
            &parser,
            "----\ninclude::fixtures/tagged-class.rb[tags=**;bark-beagle;bark-all]\n----"
        ),
        "----\nclass Dog\n  def initialize breed\n    @breed = breed\n  end\n\n  def bark\n    if @breed == 'beagle'\n      'woof woof woof woof woof'\n    else\n      'woof woof'\n    end\n  end\nend\n----"
    );
}

non_normative!(
    r#"

      # FIXME this is a weird one since we'd expect it to only select the specified tags; but it's always been this way
"#
);

// `reader_read` is the raw preprocessed text, so it keeps the block
// delimiters and any leading/trailing blank line that the enclosing block
// trims when rendered; the tag *selection* is what is verified here.
#[test]
fn include_directive_selects_all_lines_except_for_lines_containing_tag_directive_when_value_is_double_asterisk_followed_by_outer_tag_name()
 {
    verifies!(
        r#"
      test 'include directive selects all lines except for lines containing tag directive when value is double asterisk followed by outer tag name' do
        input = <<~'EOS'
        ----
        include::fixtures/tagged-class.rb[tags=**;bark]
        ----
        EOS

        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        # NOTE cannot use single-quoted heredoc because of https://github.com/jruby/jruby/issues/4260
        expected = <<~EOS.chop
        class Dog
          def initialize breed
            @breed = breed
          end

          def bark
            if @breed == 'beagle'
              'woof woof woof woof woof'
            else
              'woof woof'
            end
          end
        end
        EOS
        assert_includes output, %(<pre>#{expected}</pre>)
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(TAGGED_CLASS_RB);
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    assert_eq!(
        reader_read(
            &parser,
            "----\ninclude::fixtures/tagged-class.rb[tags=**;bark]\n----"
        ),
        "----\nclass Dog\n  def initialize breed\n    @breed = breed\n  end\n\n  def bark\n    if @breed == 'beagle'\n      'woof woof woof woof woof'\n    else\n      'woof woof'\n    end\n  end\nend\n----"
    );
}

// `reader_read` is the raw preprocessed text, so it keeps the block
// delimiters and any leading/trailing blank line that the enclosing block
// trims when rendered; the tag *selection* is what is verified here.
#[test]
fn include_directive_selects_all_lines_inside_unspecified_tags_when_value_is_negated_double_asterisk_followed_by_negated_tags()
 {
    verifies!(
        r#"
      test 'include directive selects all lines inside unspecified tags when value is negated double asterisk followed by negated tags' do
        input = <<~'EOS'
        ----
        include::fixtures/tagged-class.rb[tags=!**;!init]
        ----
        EOS

        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        expected = <<~EOS.chop
        \x20 def bark
        \x20   if @breed == 'beagle'
        \x20     'woof woof woof woof woof'
        \x20   else
        \x20     'woof woof'
        \x20   end
        \x20 end
        EOS
        assert_includes output, %(<pre>#{expected}</pre>)
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(TAGGED_CLASS_RB);
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    assert_eq!(
        reader_read(
            &parser,
            "----\ninclude::fixtures/tagged-class.rb[tags=!**;!init]\n----"
        ),
        "----\n\n  def bark\n    if @breed == 'beagle'\n      'woof woof woof woof woof'\n    else\n      'woof woof'\n    end\n  end\n----"
    );
}

// `reader_read` is the raw preprocessed text, so it keeps the block
// delimiters and any leading/trailing blank line that the enclosing block
// trims when rendered; the tag *selection* is what is verified here.
#[test]
fn include_directive_selects_all_lines_except_tag_which_is_negated_when_value_only_contains_negated_tag()
 {
    verifies!(
        r#"
      test 'include directive selects all lines except tag which is negated when value only contains negated tag' do
        input = <<~'EOS'
        ----
        include::fixtures/tagged-class.rb[tag=!bark]
        ----
        EOS

        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        # NOTE cannot use single-quoted heredoc because of https://github.com/jruby/jruby/issues/4260
        expected = <<~EOS.chop
        class Dog
          def initialize breed
            @breed = breed
          end
        end
        EOS
        assert_includes output, %(<pre>#{expected}</pre>)
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(TAGGED_CLASS_RB);
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    assert_eq!(
        reader_read(
            &parser,
            "----\ninclude::fixtures/tagged-class.rb[tag=!bark]\n----"
        ),
        "----\nclass Dog\n  def initialize breed\n    @breed = breed\n  end\nend\n----"
    );
}

// `reader_read` is the raw preprocessed text, so it keeps the block
// delimiters and any leading/trailing blank line that the enclosing block
// trims when rendered; the tag *selection* is what is verified here.
#[test]
fn include_directive_selects_all_lines_except_tags_which_are_negated_when_value_only_contains_negated_tags()
 {
    verifies!(
        r#"
      test 'include directive selects all lines except tags which are negated when value only contains negated tags' do
        input = <<~'EOS'
        ----
        include::fixtures/tagged-class.rb[tags=!bark;!init]
        ----
        EOS

        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        expected = <<~'EOS'.chop
        class Dog
        end
        EOS
        assert_includes output, %(<pre>#{expected}</pre>)
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(TAGGED_CLASS_RB);
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    assert_eq!(
        reader_read(
            &parser,
            "----\ninclude::fixtures/tagged-class.rb[tags=!bark;!init]\n----"
        ),
        "----\nclass Dog\nend\n----"
    );
}

// `reader_read` is the raw preprocessed text, so it keeps the block
// delimiters and any leading/trailing blank line that the enclosing block
// trims when rendered; the tag *selection* is what is verified here.
#[test]
fn should_recognize_tag_wildcard_if_not_at_start_of_tags_list() {
    verifies!(
        r#"
      test 'should recognize tag wildcard if not at start of tags list' do
        input = <<~'EOS'
        ----
        include::fixtures/tagged-class.rb[tags=init;**;*;!bark-other]
        ----
        EOS

        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        # NOTE cannot use single-quoted heredoc because of https://github.com/jruby/jruby/issues/4260
        expected = <<~EOS.chop
        class Dog
          def initialize breed
            @breed = breed
          end

          def bark
            if @breed == 'beagle'
              'woof woof woof woof woof'
            end
          end
        end
        EOS
        assert_includes output, %(<pre>#{expected}</pre>)
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(TAGGED_CLASS_RB);
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    assert_eq!(
        reader_read(
            &parser,
            "----\ninclude::fixtures/tagged-class.rb[tags=init;**;*;!bark-other]\n----"
        ),
        "----\nclass Dog\n  def initialize breed\n    @breed = breed\n  end\n\n  def bark\n    if @breed == 'beagle'\n      'woof woof woof woof woof'\n    end\n  end\nend\n----"
    );
}

// `reader_read` is the raw preprocessed text, so it keeps the block
// delimiters and any leading/trailing blank line that the enclosing block
// trims when rendered; the tag *selection* is what is verified here.
#[test]
fn include_directive_selects_lines_between_tags_when_value_of_tags_attribute_is_wildcard() {
    verifies!(
        r#"
      test 'include directive selects lines between tags when value of tags attribute is wildcard' do
        input = <<~'EOS'
        ----
        include::fixtures/tagged-class.rb[tags=*]
        ----
        EOS

        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        expected = <<~EOS.chop
        \x20 def initialize breed
        \x20   @breed = breed
        \x20 end

        \x20 def bark
        \x20   if @breed == 'beagle'
        \x20     'woof woof woof woof woof'
        \x20   else
        \x20     'woof woof'
        \x20   end
        \x20 end
        EOS
        assert_includes output, %(<pre>#{expected}</pre>)
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(TAGGED_CLASS_RB);
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    assert_eq!(
        reader_read(
            &parser,
            "----\ninclude::fixtures/tagged-class.rb[tags=*]\n----"
        ),
        "----\n  def initialize breed\n    @breed = breed\n  end\n\n  def bark\n    if @breed == 'beagle'\n      'woof woof woof woof woof'\n    else\n      'woof woof'\n    end\n  end\n----"
    );
}

// `reader_read` is the raw preprocessed text, so it keeps the block
// delimiters and any leading/trailing blank line that the enclosing block
// trims when rendered; the tag *selection* is what is verified here.
#[test]
fn include_directive_selects_lines_inside_tags_when_value_of_tags_attribute_is_wildcard_and_tag_surrounds_content()
 {
    verifies!(
        r#"
      test 'include directive selects lines inside tags when value of tags attribute is wildcard and tag surrounds content' do
        input = <<~'EOS'
        ----
        include::fixtures/tagged-class-enclosed.rb[tags=*]
        ----
        EOS

        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        # NOTE cannot use single-quoted heredoc because of https://github.com/jruby/jruby/issues/4260
        expected = <<~EOS.chop
        class Dog
          def initialize breed
            @breed = breed
          end

          def bark
            if @breed == 'beagle'
              'woof woof woof woof woof'
            else
              'woof woof'
            end
          end
        end
        EOS
        assert_includes output, %(<pre>#{expected}</pre>)
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(TAGGED_CLASS_ENCLOSED_RB);
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    assert_eq!(
        reader_read(
            &parser,
            "----\ninclude::fixtures/tagged-class-enclosed.rb[tags=*]\n----"
        ),
        "----\nclass Dog\n  def initialize breed\n    @breed = breed\n  end\n\n  def bark\n    if @breed == 'beagle'\n      'woof woof woof woof woof'\n    else\n      'woof woof'\n    end\n  end\nend\n----"
    );
}

// `reader_read` is the raw preprocessed text, so it keeps the block
// delimiters and any leading/trailing blank line that the enclosing block
// trims when rendered; the tag *selection* is what is verified here.
#[test]
fn include_directive_selects_lines_inside_all_tags_except_tag_which_is_negated_when_value_of_tags_attribute_is_wildcard_followed_by_negated_tag()
 {
    verifies!(
        r#"
      test 'include directive selects lines inside all tags except tag which is negated when value of tags attribute is wildcard followed by negated tag' do
        input = <<~'EOS'
        ----
        include::fixtures/tagged-class-enclosed.rb[tags=*;!init]
        ----
        EOS

        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        # NOTE cannot use single-quoted heredoc because of https://github.com/jruby/jruby/issues/4260
        expected = <<~EOS.chop
        class Dog

          def bark
            if @breed == 'beagle'
              'woof woof woof woof woof'
            else
              'woof woof'
            end
          end
        end
        EOS
        assert_includes output, %(<pre>#{expected}</pre>)
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(TAGGED_CLASS_ENCLOSED_RB);
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    assert_eq!(
        reader_read(
            &parser,
            "----\ninclude::fixtures/tagged-class-enclosed.rb[tags=*;!init]\n----"
        ),
        "----\nclass Dog\n\n  def bark\n    if @breed == 'beagle'\n      'woof woof woof woof woof'\n    else\n      'woof woof'\n    end\n  end\nend\n----"
    );
}

// `reader_read` is the raw preprocessed text, so it keeps the block
// delimiters and any leading/trailing blank line that the enclosing block
// trims when rendered; the tag *selection* is what is verified here.
#[test]
fn include_directive_skips_all_tagged_regions_except_ones_re_enabled_when_value_of_tags_attribute_is_negated_wildcard_followed_by_tag_name()
 {
    verifies!(
        r#"
      test 'include directive skips all tagged regions except ones re-enabled when value of tags attribute is negated wildcard followed by tag name' do
        ['!*;init', '**;!*;init'].each do |pattern|
          input = <<~EOS
          ----
          include::fixtures/tagged-class.rb[tags=#{pattern}]
          ----
          EOS

          output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
          # NOTE cannot use single-quoted heredoc because of https://github.com/jruby/jruby/issues/4260
          expected = <<~EOS.chop
          class Dog
            def initialize breed
              @breed = breed
            end
          end
          EOS
          assert_includes output, %(<pre>#{expected}</pre>)
        end
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(TAGGED_CLASS_RB);
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    for input in [
        "----\ninclude::fixtures/tagged-class.rb[tags=!*;init]\n----",
        "----\ninclude::fixtures/tagged-class.rb[tags=**;!*;init]\n----",
    ] {
        assert_eq!(
            reader_read(&parser, input),
            "----\nclass Dog\n  def initialize breed\n    @breed = breed\n  end\nend\n----"
        );
    }
}

// `reader_read` is the raw preprocessed text, so it keeps the block
// delimiters and any leading/trailing blank line that the enclosing block
// trims when rendered; the tag *selection* is what is verified here.
#[test]
fn include_directive_includes_regions_outside_tags_and_inside_specified_tags_when_value_begins_with_negated_wildcard()
 {
    verifies!(
        r#"
      test 'include directive includes regions outside tags and inside specified tags when value begins with negated wildcard' do
        input = <<~'EOS'
        ----
        include::fixtures/tagged-class.rb[tags=!*;bark]
        ----
        EOS

        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        # NOTE cannot use single-quoted heredoc because of https://github.com/jruby/jruby/issues/4260
        expected = <<~EOS.chop
        class Dog

          def bark
          end
        end
        EOS
        assert_includes output, %(<pre>#{expected}</pre>)
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(TAGGED_CLASS_RB);
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    assert_eq!(
        reader_read(
            &parser,
            "----\ninclude::fixtures/tagged-class.rb[tags=!*;bark]\n----"
        ),
        "----\nclass Dog\n\n  def bark\n  end\nend\n----"
    );
}

// `reader_read` is the raw preprocessed text, so it keeps the block
// delimiters and any leading/trailing blank line that the enclosing block
// trims when rendered; the tag *selection* is what is verified here.
#[test]
fn include_directive_includes_lines_inside_tag_except_for_lines_inside_nested_tags_when_tag_is_followed_by_negated_wildcard()
 {
    verifies!(
        r#"
      test 'include directive includes lines inside tag except for lines inside nested tags when tag is followed by negated wildcard' do
        ['bark;!*', '!**;bark;!*', '!**;!*;bark'].each do |pattern|
          input = <<~EOS
          ----
          include::fixtures/tagged-class.rb[tags=#{pattern}]
          ----
          EOS

          output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
          expected = <<~EOS.chop
          \x20 def bark
          \x20 end
          EOS
          assert_includes output, %(<pre>#{expected}</pre>)
        end
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(TAGGED_CLASS_RB);
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    for input in [
        "----\ninclude::fixtures/tagged-class.rb[tags=bark;!*]\n----",
        "----\ninclude::fixtures/tagged-class.rb[tags=!**;bark;!*]\n----",
        "----\ninclude::fixtures/tagged-class.rb[tags=!**;!*;bark]\n----",
    ] {
        assert_eq!(
            reader_read(&parser, input),
            "----\n\n  def bark\n  end\n----"
        );
    }
}

// `reader_read` is the raw preprocessed text, so it keeps the block
// delimiters and any leading/trailing blank line that the enclosing block
// trims when rendered; the tag *selection* is what is verified here.
#[test]
fn include_directive_selects_lines_inside_tag_except_for_lines_inside_nested_tags_when_tag_is_preceded_by_negated_double_asterisk_and_negated_wildcard()
 {
    verifies!(
        r#"
      test 'include directive selects lines inside tag except for lines inside nested tags when tag is preceded by negated double asterisk and negated wildcard' do
        input = <<~'EOS'
        ----
        include::fixtures/tagged-class.rb[tags=!**;!*;bark]
        ----
        EOS

        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        expected = <<~EOS.chop
        \x20 def bark
        \x20 end
        EOS
        assert_includes output, %(<pre>#{expected}</pre>)
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(TAGGED_CLASS_RB);
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    assert_eq!(
        reader_read(
            &parser,
            "----\ninclude::fixtures/tagged-class.rb[tags=!**;!*;bark]\n----"
        ),
        "----\n\n  def bark\n  end\n----"
    );
}

// `reader_read` is the raw preprocessed text, so it keeps the block
// delimiters and any leading/trailing blank line that the enclosing block
// trims when rendered; the tag *selection* is what is verified here.
#[test]
fn include_directive_does_not_select_lines_inside_tag_that_has_been_included_then_excluded() {
    verifies!(
        r#"
      test 'include directive does not select lines inside tag that has been included then excluded' do
        input = <<~'EOS'
        ----
        include::fixtures/tagged-class.rb[tags=!*;init;!init]
        ----
        EOS

        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        expected = <<~'EOS'.chop
        class Dog
        end
        EOS
        assert_includes output, %(<pre>#{expected}</pre>)
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(TAGGED_CLASS_RB);
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    assert_eq!(
        reader_read(
            &parser,
            "----\ninclude::fixtures/tagged-class.rb[tags=!*;init;!init]\n----"
        ),
        "----\nclass Dog\nend\n----"
    );
}

// `reader_read` is the raw preprocessed text, so it keeps the block
// delimiters and any leading/trailing blank line that the enclosing block
// trims when rendered; the tag *selection* is what is verified here.
#[test]
fn include_directive_only_selects_lines_inside_specified_tag_even_if_proceeded_by_negated_double_asterisk()
 {
    verifies!(
        r#"
      test 'include directive only selects lines inside specified tag, even if proceeded by negated double asterisk' do
        ['bark', '!**;bark'].each do |pattern|
          input = <<~EOS
          ----
          include::fixtures/tagged-class.rb[tags=#{pattern}]
          ----
          EOS

          output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
          expected = <<~EOS.chop
          \x20 def bark
          \x20   if @breed == 'beagle'
          \x20     'woof woof woof woof woof'
          \x20   else
          \x20     'woof woof'
          \x20   end
          \x20 end
          EOS
          assert_includes output, %(<pre>#{expected}</pre>)
        end
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(TAGGED_CLASS_RB);
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    for input in [
        "----\ninclude::fixtures/tagged-class.rb[tags=bark]\n----",
        "----\ninclude::fixtures/tagged-class.rb[tags=!**;bark]\n----",
    ] {
        assert_eq!(
            reader_read(&parser, input),
            "----\n\n  def bark\n    if @breed == 'beagle'\n      'woof woof woof woof woof'\n    else\n      'woof woof'\n    end\n  end\n----"
        );
    }
}

// `reader_read` is the raw preprocessed text, so it keeps the block
// delimiters and any leading/trailing blank line that the enclosing block
// trims when rendered; the tag *selection* is what is verified here.
// (The block-level `[indent=0]` is applied when the listing block is
// rendered, not by the preprocessor, so the raw text keeps its indentation.)
#[test]
fn include_directive_selects_lines_inside_specified_tag_and_ignores_lines_inside_a_negated_tag() {
    verifies!(
        r#"
      test 'include directive selects lines inside specified tag and ignores lines inside a negated tag' do
        input = <<~'EOS'
        [indent=0]
        ----
        include::fixtures/tagged-class.rb[tags=bark;!bark-other]
        ----
        EOS

        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        # NOTE cannot use single-quoted heredoc because of https://github.com/jruby/jruby/issues/4260
        expected = <<~EOS.chop
        def bark
          if @breed == 'beagle'
            'woof woof woof woof woof'
          end
        end
        EOS
        assert_includes output, %(<pre>#{expected}</pre>)
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(TAGGED_CLASS_RB);
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    assert_eq!(
        reader_read(
            &parser,
            "[indent=0]\n----\ninclude::fixtures/tagged-class.rb[tags=bark;!bark-other]\n----"
        ),
        "[indent=0]\n----\n\n  def bark\n    if @breed == 'beagle'\n      'woof woof woof woof woof'\n    end\n  end\n----"
    );
}

// A requested (non-negated) tag that never appears in the include file is
// reported, located at the include directive's line. (This crate carries the
// missing tag name; it does not reproduce Asciidoctor's trailing include-file
// path in the message.)
#[test]
fn should_warn_if_specified_tag_is_not_found_in_include_file() {
    verifies!(
        r#"
      test 'should warn if specified tag is not found in include file' do
        input = 'include::fixtures/include-file.adoc[tag=no-such-tag]'
        using_memory_logger do |logger|
          convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
          assert_message logger, :WARN, %(~<stdin>: line 1: tag 'no-such-tag' not found in include file), Hash
        end
      end

"#
    );

    let handler = InlineFileHandler::from_pairs([("include-file.adoc", INCLUDE_FILE_ADOC)]);
    let doc = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler)
        .parse("include::include-file.adoc[tag=no-such-tag]");

    let warnings: Vec<_> = doc.warnings().collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0].warning,
        WarningType::IncludeTagNotFound("tag 'no-such-tag'".to_owned())
    );
    assert_eq!(warnings[0].origin, Some(crate::parser::SourceLine(None, 1)));
}

// A *negated* tag that is not found is not reported (only the presence of a
// requested inclusion tag is checked), and the whole file is included with its
// tag directives stripped.
#[test]
fn should_not_warn_if_specified_negated_tag_is_not_found_in_include_file() {
    verifies!(
        r#"
      test 'should not warn if specified negated tag is not found in include file' do
        input = <<~'EOS'
        ----
        include::fixtures/tagged-class-enclosed.rb[tag=!no-such-tag]
        ----
        EOS
        expected = <<~EOS.chop
        class Dog
          def initialize breed
            @breed = breed
          end

          def bark
            if @breed == 'beagle'
              'woof woof woof woof woof'
            else
              'woof woof'
            end
          end
        end
        EOS
        using_memory_logger do |logger|
          output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
          assert_includes output, %(<pre>#{expected}</pre>)
          assert_empty logger.messages
        end
      end

"#
    );

    let handler =
        InlineFileHandler::from_pairs([("tagged-class-enclosed.rb", TAGGED_CLASS_ENCLOSED_RB)]);
    let doc = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler)
        .parse("----\ninclude::tagged-class-enclosed.rb[tag=!no-such-tag]\n----");

    assert_eq!(doc.warnings().count(), 0);

    // The whole class is included, with the tag-directive lines removed.
    let block = doc.nested_blocks().next().unwrap();
    let content = block.span().data();
    assert!(content.contains("class Dog\n  def initialize breed"));
    assert!(content.contains("      'woof woof woof woof woof'\n    else"));
    assert!(!content.contains("tag::"));
}

// Several missing tags are reported together, pluralized and comma-joined in
// the order they were requested.
#[test]
fn should_warn_if_specified_tags_are_not_found_in_include_file() {
    verifies!(
        r#"
      test 'should warn if specified tags are not found in include file' do
        input = <<~'EOS'
        ++++
        include::fixtures/include-file.adoc[tags=no-such-tag-b;no-such-tag-a]
        ++++
        EOS

        using_memory_logger do |logger|
          convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
          expected_tags = 'no-such-tag-b, no-such-tag-a'
          assert_message logger, :WARN, %(~<stdin>: line 2: tags '#{expected_tags}' not found in include file), Hash
        end
      end

"#
    );

    let handler = InlineFileHandler::from_pairs([("include-file.adoc", INCLUDE_FILE_ADOC)]);
    let doc = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler)
        .parse("++++\ninclude::include-file.adoc[tags=no-such-tag-b;no-such-tag-a]\n++++");

    let warnings: Vec<_> = doc.warnings().collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0].warning,
        WarningType::IncludeTagNotFound("tags 'no-such-tag-b, no-such-tag-a'".to_owned())
    );
    assert_eq!(warnings[0].origin, Some(crate::parser::SourceLine(None, 2)));
}

// The negated tags are again not reported; only the (found) `all` inclusion
// governs the selection, and the whole class is emitted.
#[test]
fn should_not_warn_if_specified_negated_tags_are_not_found_in_include_file() {
    verifies!(
        r#"
      test 'should not warn if specified negated tags are not found in include file' do
        input = <<~'EOS'
        ----
        include::fixtures/tagged-class-enclosed.rb[tags=all;!no-such-tag;!unknown-tag]
        ----
        EOS
        expected = <<~EOS.chop
        class Dog
          def initialize breed
            @breed = breed
          end

          def bark
            if @breed == 'beagle'
              'woof woof woof woof woof'
            else
              'woof woof'
            end
          end
        end
        EOS
        using_memory_logger do |logger|
          output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
          assert_includes output, %(<pre>#{expected}</pre>)
          assert_empty logger.messages
        end
      end

"#
    );

    let handler =
        InlineFileHandler::from_pairs([("tagged-class-enclosed.rb", TAGGED_CLASS_ENCLOSED_RB)]);
    let doc = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler)
        .parse("----\ninclude::tagged-class-enclosed.rb[tags=all;!no-such-tag;!unknown-tag]\n----");

    assert_eq!(doc.warnings().count(), 0);

    let block = doc.nested_blocks().next().unwrap();
    let content = block.span().data();
    assert!(content.contains("class Dog\n  def initialize breed"));
    assert!(!content.contains("tag::"));
}

// A tag region opened but never closed before end of file is reported as
// unclosed, and the region's content (to end of file) is still selected. (The
// crate carries the tag name; it does not reproduce the "starting at line N of
// include file" suffix.)
#[test]
fn should_warn_if_specified_tag_in_include_file_is_not_closed() {
    verifies!(
        r#"
      test 'should warn if specified tag in include file is not closed' do
        input = <<~'EOS'
        ++++
        include::fixtures/unclosed-tag.adoc[tag=a]
        ++++
        EOS

        using_memory_logger do |logger|
          result = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
          assert_equal 'a', result
          assert_message logger, :WARN, %(~<stdin>: line 2: detected unclosed tag 'a' starting at line 2 of include file), Hash
          refute_nil logger.messages[0][:message][:include_location]
        end
      end

"#
    );

    let handler = InlineFileHandler::from_pairs([("unclosed-tag.adoc", "x\n// tag::a[]\na")]);
    let doc = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler)
        .parse("++++\ninclude::unclosed-tag.adoc[tag=a]\n++++");

    let block = doc.nested_blocks().next().unwrap();
    assert_eq!(block.span().data(), "++++\na\n++++");

    let warnings: Vec<_> = doc.warnings().collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0].warning,
        WarningType::IncludeTagUnclosed("'a'".to_owned())
    );
    assert_eq!(warnings[0].origin, Some(crate::parser::SourceLine(None, 2)));
}

// An `end::` directive that closes an outer region while an inner region is
// still open is reported as a mismatch (expected the inner tag, found the outer
// one). The two selected regions' content still survives.
#[test]
fn should_warn_if_end_tag_in_included_file_is_mismatched() {
    verifies!(
        r#"
      test 'should warn if end tag in included file is mismatched' do
        input = <<~'EOS'
        ++++
        include::fixtures/mismatched-end-tag.adoc[tags=a;b]
        ++++
        EOS

        inc_path = File.join DIRNAME, 'fixtures/mismatched-end-tag.adoc'
        using_memory_logger do |logger|
          result = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
          assert_equal %(a\nb), result
          assert_message logger, :WARN, %(<stdin>: line 2: mismatched end tag (expected 'b' but found 'a') at line 5 of include file: #{inc_path}), Hash
          refute_nil logger.messages[0][:message][:include_location]
        end
      end

"#
    );

    let handler = InlineFileHandler::from_pairs([(
        "mismatched-end-tag.adoc",
        "//tag::a[]\na\n//tag::b[]\nb\n//end::a[]\n//end::b[]\nc",
    )]);
    let doc = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler)
        .parse("++++\ninclude::mismatched-end-tag.adoc[tags=a;b]\n++++");

    let block = doc.nested_blocks().next().unwrap();
    assert_eq!(block.span().data(), "++++\na\nb\n++++");

    let warnings: Vec<_> = doc.warnings().collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0].warning,
        WarningType::IncludeTagMismatchedEnd("'b'".to_owned(), "'a'".to_owned())
    );
    assert_eq!(warnings[0].origin, Some(crate::parser::SourceLine(None, 2)));
}

// An `end::` directive with no matching open region is reported as unexpected,
// and the (already-closed) region's content still survives.
#[test]
fn should_warn_if_unexpected_end_tag_is_found_in_included_file() {
    verifies!(
        r#"
      test 'should warn if unexpected end tag is found in included file' do
        input = <<~'EOS'
        ++++
        include::fixtures/unexpected-end-tag.adoc[tags=a]
        ++++
        EOS

        inc_path = File.join DIRNAME, 'fixtures/unexpected-end-tag.adoc'
        using_memory_logger do |logger|
          result = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
          assert_equal 'a', result
          assert_message logger, :WARN, %(<stdin>: line 2: unexpected end tag 'a' at line 4 of include file: #{inc_path}), Hash
          refute_nil logger.messages[0][:message][:include_location]
        end
      end

"#
    );

    let handler = InlineFileHandler::from_pairs([(
        "unexpected-end-tag.adoc",
        "// tag::a[]\na\n// end::a[]\n// end::a[]",
    )]);
    let doc = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler)
        .parse("++++\ninclude::unexpected-end-tag.adoc[tags=a]\n++++");

    let block = doc.nested_blocks().next().unwrap();
    assert_eq!(block.span().data(), "++++\na\n++++");

    let warnings: Vec<_> = doc.warnings().collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0].warning,
        WarningType::IncludeTagUnexpectedEnd("'a'".to_owned())
    );
    assert_eq!(warnings[0].origin, Some(crate::parser::SourceLine(None, 2)));
}

// An empty `tag=` / `tags=` value applies no filtering, so the whole file —
// including its tag-directive lines — is included.
#[test]
fn include_directive_ignores_tags_attribute_when_empty() {
    verifies!(
        r#"
      test 'include directive ignores tags attribute when empty' do
        ['tag', 'tags'].each do |attr_name|
          input = <<~EOS
          ++++
          include::fixtures/include-file.xml[#{attr_name}=]
          ++++
          EOS

          output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
          assert_match(/(?:tag|end)::/, output, 2)
        end
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new("// tag::a[]\nbody\n// end::a[]");
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    for attr_name in ["tag", "tags"] {
        let input = format!("++++\ninclude::x.xml[{attr_name}=]\n++++");
        assert_eq!(
            reader_read(&parser, &input),
            "++++\n// tag::a[]\nbody\n// end::a[]\n++++"
        );
    }
}

// When both are given, `lines` wins over `tags`: only the first line is
// selected, not the `snippetA`/`snippetB` regions.
#[test]
fn lines_attribute_takes_precedence_over_tags_attribute_in_include_directive() {
    verifies!(
        r#"
      test 'lines attribute takes precedence over tags attribute in include directive' do
        input = 'include::fixtures/include-file.adoc[lines=1, tags=snippetA;snippetB]'
        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        assert_match(/first line of included content/, output)
        refute_match(/snippetA content/, output)
        refute_match(/snippetB content/, output)
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(INCLUDE_FILE_ADOC);
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    assert_eq!(
        reader_read(
            &parser,
            "include::fixtures/include-file.adoc[lines=1, tags=snippetA;snippetB]"
        ),
        "first line of included content"
    );
}

// `indent=0` on the include directive strips the common indentation of the
// selected lines. Here lines 2-3 are both indented four spaces, so the
// preprocessor reindents them to column 0.
#[test]
fn indent_of_included_file_can_be_reset_to_size_of_indent_attribute() {
    verifies!(
        r#"
      test 'indent of included file can be reset to size of indent attribute' do
        input = <<~'EOS'
        [source, xml]
        ----
        include::fixtures/basic-docinfo.xml[lines=2..3, indent=0]
        ----
        EOS

        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        result = xmlnodes_at_xpath('//pre', output, 1).text
        assert_equal "<year>2013</year>\n<holder>Acme™, Inc.</holder>", result
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(BASIC_DOCINFO_XML);
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);

    assert_eq!(
        reader_read(
            &parser,
            "[source, xml]\n----\ninclude::fixtures/basic-docinfo.xml[lines=2..3, indent=0]\n----"
        ),
        "[source, xml]\n----\n<year>2013</year>\n<holder>Acme\u{2122}, Inc.</holder>\n----"
    );
}

// Attribute references in the attribute list are resolved too, so
// `tag={name-of-tag}` selects the `snippetA` region.
#[test]
fn should_substitute_attribute_references_in_attrlist() {
    verifies!(
        r#"
      test 'should substitute attribute references in attrlist' do
        input = <<~'EOS'
        :name-of-tag: snippetA
        include::fixtures/include-file.adoc[tag={name-of-tag}]
        EOS

        output = convert_string_to_embedded input, safe: :safe, base_dir: DIRNAME
        assert_match(/snippetA content/, output)
        refute_match(/snippetB content/, output)
        refute_match(/non-tagged content/, output)
        refute_match(/included content/, output)
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new(INCLUDE_FILE_ADOC);
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    assert_eq!(
        reader_read(
            &parser,
            ":name-of-tag: snippetA\ninclude::fixtures/include-file.adoc[tag={name-of-tag}]"
        ),
        ":name-of-tag: snippetA\nsnippetA content"
    );
}

non_normative!(
    r#"
      test 'should fall back to built-in include directive behavior when not handled by include processor' do
        input = 'include::fixtures/include-file.adoc[]'
        include_processor = Class.new do
          def initialize document; end

          def handles? target
            false
          end

          def process reader, target, attributes
            raise 'TestIncludeHandler should not have been invoked'
          end
        end

        document = empty_safe_document base_dir: DIRNAME
        reader = Asciidoctor::PreprocessorReader.new document, input, nil, normalize: true
        reader.instance_variable_set '@include_processors', [include_processor.new(document)]
        lines = reader.read_lines
        source = lines * ::Asciidoctor::LF
        assert_match(/included content/, source)
      end

"#
);

non_normative!(
    r#"
      test 'leveloffset attribute entries should be added to content if leveloffset attribute is specified' do
        input = 'include::fixtures/main.adoc[]'
        expected = <<~'EOS'.split ::Asciidoctor::LF
        = Main Document

        preamble

        :leveloffset: +1

        = Chapter A

        content

        :leveloffset!:
        EOS

        document = Asciidoctor.load input, safe: :safe, base_dir: DIRNAME, parse: false
        assert_equal expected, document.reader.read_lines
      end

"#
);

// Attribute references in the target are resolved before the handler is
// consulted: `{fixturesdir}/include-file.{ext}` becomes
// `fixtures/include-file.adoc`.
#[test]
fn attributes_are_substituted_in_target_of_include_directive() {
    verifies!(
        r#"
      test 'attributes are substituted in target of include directive' do
        input = <<~'EOS'
        :fixturesdir: fixtures
        :ext: adoc

        include::{fixturesdir}/include-file.{ext}[]
        EOS

        doc = document_from_string input, safe: :safe, base_dir: DIRNAME
        output = doc.convert
        assert_match(/included content/, output)
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new("included content");
    let probe = handler.clone();
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);
    assert_eq!(
        reader_read(
            &parser,
            ":fixturesdir: fixtures\n:ext: adoc\n\ninclude::{fixturesdir}/include-file.{ext}[]"
        ),
        ":fixturesdir: fixtures\n:ext: adoc\n\nincluded content"
    );
    assert_eq!(
        probe.calls(),
        vec![(None, "fixtures/include-file.adoc".to_owned(), None)]
    );
}

// `{blank}` resolves to the empty string, so the directive's target is blank.
// This crate hands even a blank target to the handler (a real handler reports
// no such file by returning `None`), so the directive is replaced with the
// "Unresolved directive" message and an `IncludeFileNotFound` warning — rather
// than Asciidoctor's distinct "resolved target is blank" notice, and naming the
// root as `(root file)` rather than `<stdin>`.
#[test]
fn line_is_skipped_by_default_if_target_of_include_directive_resolves_to_empty() {
    verifies!(
        r#"
      test 'line is skipped by default if target of include directive resolves to empty' do
        input = 'include::{blank}[]'
        using_memory_logger do |logger|
          doc = empty_safe_document base_dir: DIRNAME
          reader = Asciidoctor::PreprocessorReader.new doc, input, nil, normalize: true
          line = reader.read_line
          assert_equal 'Unresolved directive in <stdin> - include::{blank}[]', line
          assert_message logger, :WARN, '<stdin>: line 1: include dropped because resolved target is blank: include::{blank}[]', Hash
        end
      end

"#
    );

    let handler = RecordingIncludeFileHandler::missing();
    let probe = handler.clone();
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler);

    let (output, _source_map, warnings, _includes) =
        crate::parser::preprocessor::preprocess("include::{blank}[]", &parser);

    assert_eq!(
        output,
        "Unresolved directive in (root file) - include::{blank}[]\n"
    );
    // The blank target was still handed to the handler (as the empty string) ...
    assert_eq!(probe.calls(), vec![(None, "".to_owned(), None)]);
    // ... which reported no such file, yielding the not-found warning.
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0].warning,
        WarningType::IncludeFileNotFound("".to_owned())
    );
}

// Under `attribute-missing=drop-line`, a missing attribute reference in an
// include target drops the entire directive line: nothing is emitted in its
// place and the include file handler is never consulted. Asciidoctor logs both
// of its messages here at INFO level; this crate has no INFO channel, so — as
// everywhere else `drop-line` applies — the line is dropped silently.
#[test]
fn include_is_dropped_if_target_contains_missing_attribute_and_attribute_missing_is_drop_line() {
    verifies!(
        r#"
      test 'include is dropped if target contains missing attribute and attribute-missing is drop-line' do
        input = 'include::{foodir}/include-file.adoc[]'
        using_memory_logger Logger::INFO do |logger|
          doc = empty_safe_document base_dir: DIRNAME, attributes: { 'attribute-missing' => 'drop-line' }
          reader = Asciidoctor::PreprocessorReader.new doc, input, nil, normalize: true
          line = reader.read_line
          assert_nil line
          assert_messages logger, [
            [:INFO, 'dropping line containing reference to missing attribute: foodir'],
            [:INFO, '<stdin>: line 1: include dropped due to missing attribute: include::{foodir}/include-file.adoc[]', Hash],
          ]
        end
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new("included content");
    let probe = handler.clone();

    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_intrinsic_attribute(
            "attribute-missing",
            "drop-line",
            ModificationContext::Anywhere,
        )
        .with_include_file_handler(handler);

    let (output, _source_map, warnings, _includes) =
        crate::parser::preprocessor::preprocess("include::{foodir}/include-file.adoc[]", &parser);

    assert_eq!(output, "");
    assert!(warnings.is_empty());
    assert!(probe.calls().is_empty());
}

// The directive line is dropped on its own: the line that follows it survives.
// Under `attribute-missing=warn` the dropped directive leaves the "Unresolved
// directive" message in its place (naming the root as `(root file)` rather than
// `<stdin>`) together with a single warning naming the whole directive.
#[test]
fn line_following_dropped_include_is_not_dropped() {
    verifies!(
        r#"
      test 'line following dropped include is not dropped' do
        input = <<~'EOS'
        include::{foodir}/include-file.adoc[]
        yo
        EOS

        using_memory_logger do |logger|
          doc = empty_safe_document base_dir: DIRNAME, attributes: { 'attribute-missing' => 'warn' }
          reader = Asciidoctor::PreprocessorReader.new doc, input, nil, normalize: true
          line = reader.read_line
          assert_equal 'Unresolved directive in <stdin> - include::{foodir}/include-file.adoc[]', line
          line = reader.read_line
          assert_equal 'yo', line
          assert_messages logger, [
            [:INFO, 'dropping line containing reference to missing attribute: foodir'],
            [:WARN, '<stdin>: line 1: include dropped due to missing attribute: include::{foodir}/include-file.adoc[]', Hash],
          ]
        end
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new("included content");
    let probe = handler.clone();

    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_intrinsic_attribute("attribute-missing", "warn", ModificationContext::Anywhere)
        .with_include_file_handler(handler);

    let (output, _source_map, warnings, _includes) = crate::parser::preprocessor::preprocess(
        "include::{foodir}/include-file.adoc[]\nyo",
        &parser,
    );

    assert_eq!(
        output,
        "Unresolved directive in (root file) - include::{foodir}/include-file.adoc[]\nyo\n"
    );

    assert!(probe.calls().is_empty());

    assert_eq!(warnings.len(), 1);

    assert_eq!(
        warnings[0].warning,
        WarningType::IncludeDroppedDueToMissingAttribute(
            "include::{foodir}/include-file.adoc[]".to_owned()
        )
    );
}

// A backslash-escaped include directive keeps its place with the backslash
// removed and is not expanded; an unrelated leading backslash is preserved.
#[test]
fn escaped_include_directive_is_left_unprocessed() {
    verifies!(
        r#"
      test 'escaped include directive is left unprocessed' do
        input = <<~'EOS'
        \include::fixtures/include-file.adoc[]
        \escape preserved here
        EOS
        doc = empty_safe_document base_dir: DIRNAME
        reader = Asciidoctor::PreprocessorReader.new doc, input, nil, normalize: true
        # we should be able to peek it multiple times and still have the backslash preserved
        # this is the test for @unescape_next_line
        assert_equal 'include::fixtures/include-file.adoc[]', reader.peek_line
        assert_equal 'include::fixtures/include-file.adoc[]', reader.peek_line
        assert_equal 'include::fixtures/include-file.adoc[]', reader.read_line
        assert_equal '\\escape preserved here', reader.read_line
      end

"#
    );

    assert_eq!(
        reader_read(
            &Parser::default(),
            "\\include::fixtures/include-file.adoc[]\n\\escape preserved here"
        ),
        "include::fixtures/include-file.adoc[]\n\\escape preserved here"
    );
}

// An include directive must start at column 0; an indented one is ordinary
// content and is left untouched by the preprocessor.
#[test]
fn include_directive_not_at_start_of_line_is_ignored() {
    verifies!(
        r#"
      test 'include directive not at start of line is ignored' do
        input = ' include::include-file.adoc[]'
        para = block_from_string input
        assert_equal 1, para.lines.size
        # NOTE the space gets stripped because the line is treated as an inline literal
        assert_equal :literal, para.context
        assert_equal 'include::include-file.adoc[]', para.source
      end

"#
    );

    assert_eq!(
        reader_read(&Parser::default(), " include::include-file.adoc[]"),
        " include::include-file.adoc[]"
    );
}

// `max-include-depth=0` disables the include directive entirely: the directive
// line is left in the output verbatim, with no diagnostic, and the include
// file handler is never consulted.
#[test]
fn include_directive_is_disabled_when_max_include_depth_attribute_is_0() {
    verifies!(
        r#"
      test 'include directive is disabled when max-include-depth attribute is 0' do
        input = 'include::include-file.adoc[]'
        para = block_from_string input, safe: :safe, attributes: { 'max-include-depth' => 0 }
        assert_equal 1, para.lines.size
        assert_equal 'include::include-file.adoc[]', para.source
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new("included content");
    let probe = handler.clone();
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_intrinsic_attribute("max-include-depth", "0", ModificationContext::ApiOnly)
        .with_include_file_handler(handler);

    let (output, _source_map, warnings, _includes) =
        crate::parser::preprocessor::preprocess("include::include-file.adoc[]", &parser);

    assert_eq!(output, "include::include-file.adoc[]\n");
    assert!(probe.calls().is_empty());
    assert!(warnings.is_empty());
}

// `max-include-depth` is an API-only attribute (see `built_in_attrs.rs`), so
// the document's attempt to raise it to 1 is ignored and the API-set value of
// 0 still disables the include directive.
#[test]
fn max_include_depth_cannot_be_set_by_document() {
    verifies!(
        r#"
      test 'max-include-depth cannot be set by document' do
        input = <<~'EOS'
        :max-include-depth: 1

        include::include-file.adoc[]
        EOS
        para = block_from_string input, safe: :safe, attributes: { 'max-include-depth' => 0 }
        assert_equal 1, para.lines.size
        assert_equal 'include::include-file.adoc[]', para.source
      end

"#
    );

    let handler = RecordingIncludeFileHandler::new("included content");
    let probe = handler.clone();
    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_intrinsic_attribute("max-include-depth", "0", ModificationContext::ApiOnly)
        .with_include_file_handler(handler);

    let (output, _source_map, warnings, _includes) = crate::parser::preprocessor::preprocess(
        ":max-include-depth: 1\n\ninclude::include-file.adoc[]",
        &parser,
    );

    assert_eq!(
        output,
        ":max-include-depth: 1\n\ninclude::include-file.adoc[]\n"
    );
    assert!(probe.calls().is_empty());
    assert!(warnings.is_empty());
}

/// Verbatim copy of Asciidoctor's `test/fixtures/parent-include.adoc`.
const PARENT_INCLUDE_ADOC: &str =
    "first line of parent\n\ninclude::child-include.adoc[]\n\nlast line of parent\n";

/// Verbatim copy of Asciidoctor's
/// `test/fixtures/parent-include-restricted.adoc`.
const PARENT_INCLUDE_RESTRICTED_ADOC: &str =
    "first line of parent\n\ninclude::child-include.adoc[depth=0]\n\nlast line of parent\n";

/// Verbatim copy of Asciidoctor's `test/fixtures/child-include.adoc`.
const CHILD_INCLUDE_ADOC: &str =
    "first line of child\n\ninclude::grandchild-include.adoc[]\n\nlast line of child\n";

/// Verbatim copy of Asciidoctor's `test/fixtures/grandchild-include.adoc`.
const GRANDCHILD_INCLUDE_ADOC: &str = "first line of grandchild\n\nlast line of grandchild\n";

// The `depth` attribute on an include directive bounds how many further levels
// of include nesting are permitted beneath the included file. An include
// directive in a file that already sits at the limit is left verbatim, with a
// "maximum include depth exceeded" error at the directive's own file and line.
//
// (Asciidoctor resolves each nested target against the including file's
// directory, so its message names `fixtures/child-include.adoc`; this crate
// delegates path resolution to the include file handler and names the target
// as written, `child-include.adoc`.)
#[test]
fn include_directive_should_be_disabled_if_max_include_depth_has_been_exceeded() {
    verifies!(
        r#"
      test 'include directive should be disabled if max include depth has been exceeded' do
        input = 'include::fixtures/parent-include.adoc[depth=1]'
        using_memory_logger do |logger|
          pseudo_docfile = File.join DIRNAME, 'main.adoc'
          doc = empty_safe_document base_dir: DIRNAME
          reader = Asciidoctor::PreprocessorReader.new doc, input, Asciidoctor::Reader::Cursor.new(pseudo_docfile), normalize: true
          lines = reader.readlines
          assert_includes lines, 'include::grandchild-include.adoc[]'
          assert_message logger, :ERROR, 'fixtures/child-include.adoc: line 3: maximum include depth of 1 exceeded', Hash
        end
      end

"#
    );

    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(InlineFileHandler::from_pairs([
            ("fixtures/parent-include.adoc", PARENT_INCLUDE_ADOC),
            ("child-include.adoc", CHILD_INCLUDE_ADOC),
            ("grandchild-include.adoc", GRANDCHILD_INCLUDE_ADOC),
        ]));

    let (output, source_map, warnings, _includes) = crate::parser::preprocessor::preprocess(
        "include::fixtures/parent-include.adoc[depth=1]",
        &parser,
    );

    assert_eq!(
        output,
        "first line of parent\n\nfirst line of child\n\ninclude::grandchild-include.adoc[]\n\nlast line of child\n\nlast line of parent\n"
    );

    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].warning, WarningType::MaxIncludeDepthExceeded(1));

    // The warning's span covers the verbatim directive line, which the source
    // map places at line 3 of `child-include.adoc`.
    assert_eq!(
        &output[warnings[0].offset..(warnings[0].offset + warnings[0].len)],
        "include::grandchild-include.adoc[]"
    );

    let output_line = output[..warnings[0].offset]
        .bytes()
        .filter(|&b| b == b'\n')
        .count()
        + 1;
    assert_eq!(
        source_map.original_file_and_line(output_line),
        Some(crate::parser::SourceLine(
            Some("child-include.adoc".to_owned()),
            3
        ))
    );
}

// A `depth` limit established by a nested include directive (here `depth=0` on
// the child include) is enforced within that include even though the outer
// directive permitted more nesting, and is restored once the include has been
// merged (so `last line of parent` still follows).
#[test]
fn include_directive_should_be_disabled_if_max_include_depth_set_in_nested_context_has_been_exceeded()
 {
    verifies!(
        r#"
      test 'include directive should be disabled if max include depth set in nested context has been exceeded' do
        input = 'include::fixtures/parent-include-restricted.adoc[depth=3]'
        using_memory_logger do |logger|
          pseudo_docfile = File.join DIRNAME, 'main.adoc'
          doc = empty_safe_document base_dir: DIRNAME
          reader = Asciidoctor::PreprocessorReader.new doc, input, Asciidoctor::Reader::Cursor.new(pseudo_docfile), normalize: true
          lines = reader.readlines
          assert_includes lines, 'first line of child'
          assert_includes lines, 'include::grandchild-include.adoc[]'
          assert_message logger, :ERROR, 'fixtures/child-include.adoc: line 3: maximum include depth of 0 exceeded', Hash
        end
      end

"#
    );

    let parser = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(InlineFileHandler::from_pairs([
            (
                "fixtures/parent-include-restricted.adoc",
                PARENT_INCLUDE_RESTRICTED_ADOC,
            ),
            ("child-include.adoc", CHILD_INCLUDE_ADOC),
            ("grandchild-include.adoc", GRANDCHILD_INCLUDE_ADOC),
        ]));

    let (output, _source_map, warnings, _includes) = crate::parser::preprocessor::preprocess(
        "include::fixtures/parent-include-restricted.adoc[depth=3]",
        &parser,
    );

    assert_eq!(
        output,
        "first line of parent\n\nfirst line of child\n\ninclude::grandchild-include.adoc[]\n\nlast line of child\n\nlast line of parent\n"
    );

    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].warning, WarningType::MaxIncludeDepthExceeded(0));
}

non_normative!(
    r#"
      test 'read_lines_until should not process lines if process option is false' do
        lines = <<~'EOS'.lines
        ////
        include::fixtures/no-such-file.adoc[]
        ////
        EOS

        doc = empty_safe_document base_dir: DIRNAME
        reader = Asciidoctor::PreprocessorReader.new doc, lines, nil, normalize: true
        reader.read_line
        result = reader.read_lines_until(terminator: '////', skip_processing: true)
        assert_equal lines.map(&:chomp)[1..1], result
      end

"#
);

non_normative!(
    r#"
      test 'skip_comment_lines should not process lines read' do
        lines = <<~'EOS'.lines
        ////
        include::fixtures/no-such-file.adoc[]
        ////
        EOS

        using_memory_logger do |logger|
          doc = empty_safe_document base_dir: DIRNAME
          reader = Asciidoctor::PreprocessorReader.new doc, lines, nil, normalize: true
          reader.skip_comment_lines
          assert reader.empty?
          assert logger.empty?
        end
      end
"#
);

non_normative!(
    r#"
    end

    context 'Conditional Inclusions' do
"#
);

// Out of scope (Reader API): these drive the reader's incremental conditional
// processing directly — `process_line`, `peek_line`/`peek_lines` (with the
// `direct` option), and the cursor/`lineno` advancement as conditional lines
// are consumed one at a time. This crate resolves all conditionals in a single
// preprocessing pass with no incremental cursor to observe. The observable
// results (which lines survive) are covered by the `ifdef`/`ifndef`/`ifeval`
// cases below.
non_normative!(
    r#"
      test 'process_line returns nil if cursor advanced' do
        input = <<~'EOS'
        ifdef::asciidoctor[]
        Asciidoctor!
        endif::asciidoctor[]
        EOS

        doc = Asciidoctor::Document.new input
        reader = doc.reader
        assert_nil reader.send :process_line, reader.lines.first
      end

      test 'peek_line advances cursor to next conditional line of content' do
        input = <<~'EOS'
        ifdef::asciidoctor[]
        Asciidoctor!
        endif::asciidoctor[]
        EOS

        doc = Asciidoctor::Document.new input
        reader = doc.reader
        assert_equal 1, reader.lineno
        assert_equal 'Asciidoctor!', reader.peek_line
        assert_equal 2, reader.lineno
      end

      test 'peek_lines should preprocess lines if direct is false' do
        input = <<~'EOS'
        The Asciidoctor
        ifdef::asciidoctor[is in.]
        EOS
        doc = Asciidoctor::Document.new input
        reader = doc.reader
        result = reader.peek_lines 2, false
        assert_equal ['The Asciidoctor', 'is in.'], result
      end

      test 'peek_lines should not preprocess lines if direct is true' do
        input = <<~'EOS'
        The Asciidoctor
        ifdef::asciidoctor[is in.]
        EOS
        doc = Asciidoctor::Document.new input
        reader = doc.reader
        result = reader.peek_lines 2, true
        assert_equal ['The Asciidoctor', 'ifdef::asciidoctor[is in.]'], result
      end

      test 'peek_lines should not prevent subsequent preprocessing of peeked lines' do
        input = <<~'EOS'
        The Asciidoctor
        ifdef::asciidoctor[is in.]
        EOS
        doc = Asciidoctor::Document.new input
        reader = doc.reader
        result = reader.peek_lines 2, true
        result = reader.peek_lines 2, false
        assert_equal ['The Asciidoctor', 'is in.'], result
      end

      test 'process_line returns line if cursor not advanced' do
        input = <<~'EOS'
        content
        ifdef::asciidoctor[]
        Asciidoctor!
        endif::asciidoctor[]
        EOS

        doc = Asciidoctor::Document.new input
        reader = doc.reader
        refute_nil reader.send :process_line, reader.lines.first
      end

      test 'peek_line does not advance cursor when on a regular content line' do
        input = <<~'EOS'
        content
        ifdef::asciidoctor[]
        Asciidoctor!
        endif::asciidoctor[]
        EOS

        doc = Asciidoctor::Document.new input
        reader = doc.reader
        assert_equal 1, reader.lineno
        assert_equal 'content', reader.peek_line
        assert_equal 1, reader.lineno
      end

      test 'peek_line returns nil if cursor advances past end of source' do
        input = <<~'EOS'
        ifdef::foobar[]
        swallowed content
        endif::foobar[]
        EOS

        doc = Asciidoctor::Document.new input
        reader = doc.reader
        assert_equal 1, reader.lineno
        assert_nil reader.peek_line
        assert_equal 4, reader.lineno
      end

      test 'peek_line returns nil if contents of skipped conditional is empty line' do
        input = <<~'EOS'
        ifdef::foobar[]

        endif::foobar[]
        EOS

        doc = Asciidoctor::Document.new input
        reader = doc.reader
        assert_equal 1, reader.lineno
        assert_nil reader.peek_line
      end

"#
);

#[test]
fn ifdef_with_defined_attribute_includes_content() {
    verifies!(
        r#"
      test 'ifdef with defined attribute includes content' do
        input = <<~'EOS'
        ifdef::holygrail[]
        There is a holy grail!
        endif::holygrail[]
        EOS

        doc = Asciidoctor::Document.new input, attributes: { 'holygrail' => '' }
        reader = doc.reader
        lines = []
        while reader.has_more_lines?
          lines << reader.read_line
        end
        assert_equal 'There is a holy grail!', (lines * ::Asciidoctor::LF)
      end

"#
    );

    let parser =
        Parser::default().with_intrinsic_attribute("holygrail", "", ModificationContext::Anywhere);
    assert_eq!(
        reader_read(
            &parser,
            "ifdef::holygrail[]\nThere is a holy grail!\nendif::holygrail[]"
        ),
        "There is a holy grail!"
    );
}

// The single-line form emits its bracketed text in place when the condition
// holds.
#[test]
fn ifdef_with_defined_attribute_includes_text_in_brackets() {
    verifies!(
        r#"
      test 'ifdef with defined attribute includes text in brackets' do
        input = <<~'EOS'
        On our quest we go...
        ifdef::holygrail[There is a holy grail!]
        There was much rejoicing.
        EOS

        doc = Asciidoctor::Document.new input, attributes: { 'holygrail' => '' }
        reader = doc.reader
        lines = []
        while reader.has_more_lines?
          lines << reader.read_line
        end
        assert_equal "On our quest we go...\nThere is a holy grail!\nThere was much rejoicing.", (lines * ::Asciidoctor::LF)
      end

"#
    );

    let parser =
        Parser::default().with_intrinsic_attribute("holygrail", "", ModificationContext::Anywhere);
    assert_eq!(
        reader_read(
            &parser,
            "On our quest we go...\nifdef::holygrail[There is a holy grail!]\nThere was much rejoicing."
        ),
        "On our quest we go...\nThere is a holy grail!\nThere was much rejoicing."
    );
}

// Out of scope: asserts that the single-line form's bracketed content — here an
// `include::` directive — is itself preprocessed. This crate emits
// single-line conditional content for normal parsing and does not recursively
// run the preprocessor over it, so a nested include there is not expanded.
non_normative!(
    r#"
      test 'ifdef with defined attribute processes include directive in brackets' do
        input = 'ifdef::asciidoctor-version[include::fixtures/include-file.adoc[tag=snippetA]]'
        doc = Asciidoctor::Document.new input, safe: :safe, base_dir: DIRNAME
        reader = doc.reader
        lines = []
        while reader.has_more_lines?
          lines << reader.read_line
        end
        assert_equal 'snippetA content', lines[0]
      end

"#
);

// The attribute name in the directive is matched case-insensitively.
#[test]
fn ifdef_attribute_name_is_not_case_sensitive() {
    verifies!(
        r#"
      test 'ifdef attribute name is not case sensitive' do
        input = <<~'EOS'
        ifdef::showScript[]
        The script is shown!
        endif::showScript[]
        EOS

        doc = Asciidoctor::Document.new input, attributes: { 'showscript' => '' }
        result = doc.reader.read
        assert_equal 'The script is shown!', result
      end

"#
    );

    let parser =
        Parser::default().with_intrinsic_attribute("showscript", "", ModificationContext::Anywhere);
    assert_eq!(
        reader_read(
            &parser,
            "ifdef::showScript[]\nThe script is shown!\nendif::showScript[]"
        ),
        "The script is shown!"
    );
}

#[test]
fn ifndef_with_defined_attribute_does_not_include_text_in_brackets() {
    verifies!(
        r#"
      test 'ifndef with defined attribute does not include text in brackets' do
        input = <<~'EOS'
        On our quest we go...
        ifndef::hardships[There is a holy grail!]
        There was no rejoicing.
        EOS

        doc = Asciidoctor::Document.new input, attributes: { 'hardships' => '' }
        reader = doc.reader
        lines = []
        while reader.has_more_lines?
          lines << reader.read_line
        end
        assert_equal "On our quest we go...\nThere was no rejoicing.", (lines * ::Asciidoctor::LF)
      end

"#
    );

    let parser =
        Parser::default().with_intrinsic_attribute("hardships", "", ModificationContext::Anywhere);
    assert_eq!(
        reader_read(
            &parser,
            "On our quest we go...\nifndef::hardships[There is a holy grail!]\nThere was no rejoicing."
        ),
        "On our quest we go...\nThere was no rejoicing."
    );
}

#[test]
fn include_with_non_matching_nested_exclude() {
    verifies!(
        r#"
      test 'include with non-matching nested exclude' do
        input = <<~'EOS'
        ifdef::grail[]
        holy
        ifdef::swallow[]
        swallow
        endif::swallow[]
        grail
        endif::grail[]
        EOS

        doc = Asciidoctor::Document.new input, attributes: { 'grail' => '' }
        reader = doc.reader
        lines = []
        while reader.has_more_lines?
          lines << reader.read_line
        end
        assert_equal "holy\ngrail", (lines * ::Asciidoctor::LF)
      end

"#
    );

    let parser =
        Parser::default().with_intrinsic_attribute("grail", "", ModificationContext::Anywhere);
    assert_eq!(
        reader_read(
            &parser,
            "ifdef::grail[]\nholy\nifdef::swallow[]\nswallow\nendif::swallow[]\ngrail\nendif::grail[]"
        ),
        "holy\ngrail"
    );
}

#[test]
fn nested_excludes_with_same_condition() {
    verifies!(
        r#"
      test 'nested excludes with same condition' do
        input = <<~'EOS'
        ifndef::grail[]
        ifndef::grail[]
        not here
        endif::grail[]
        endif::grail[]
        EOS

        doc = Asciidoctor::Document.new input, attributes: { 'grail' => '' }
        reader = doc.reader
        lines = []
        while reader.has_more_lines?
          lines << reader.read_line
        end
        assert_equal '', (lines * ::Asciidoctor::LF)
      end

"#
    );

    let parser =
        Parser::default().with_intrinsic_attribute("grail", "", ModificationContext::Anywhere);
    assert_eq!(
        reader_read(
            &parser,
            "ifndef::grail[]\nifndef::grail[]\nnot here\nendif::grail[]\nendif::grail[]"
        ),
        ""
    );
}

#[test]
fn include_with_nested_exclude_of_inverted_condition() {
    verifies!(
        r#"
      test 'include with nested exclude of inverted condition' do
        input = <<~'EOS'
        ifdef::grail[]
        holy
        ifndef::grail[]
        not here
        endif::grail[]
        grail
        endif::grail[]
        EOS

        doc = Asciidoctor::Document.new input, attributes: { 'grail' => '' }
        reader = doc.reader
        lines = []
        while reader.has_more_lines?
          lines << reader.read_line
        end
        assert_equal "holy\ngrail", (lines * ::Asciidoctor::LF)
      end

"#
    );

    let parser =
        Parser::default().with_intrinsic_attribute("grail", "", ModificationContext::Anywhere);
    assert_eq!(
        reader_read(
            &parser,
            "ifdef::grail[]\nholy\nifndef::grail[]\nnot here\nendif::grail[]\ngrail\nendif::grail[]"
        ),
        "holy\ngrail"
    );
}

#[test]
fn exclude_with_matching_nested_exclude() {
    verifies!(
        r#"
      test 'exclude with matching nested exclude' do
        input = <<~'EOS'
        poof
        ifdef::swallow[]
        no
        ifdef::swallow[]
        swallow
        endif::swallow[]
        here
        endif::swallow[]
        gone
        EOS

        doc = Asciidoctor::Document.new input, attributes: { 'grail' => '' }
        reader = doc.reader
        lines = []
        while reader.has_more_lines?
          lines << reader.read_line
        end
        assert_equal "poof\ngone", (lines * ::Asciidoctor::LF)
      end

"#
    );

    let parser =
        Parser::default().with_intrinsic_attribute("grail", "", ModificationContext::Anywhere);
    assert_eq!(
        reader_read(
            &parser,
            "poof\nifdef::swallow[]\nno\nifdef::swallow[]\nswallow\nendif::swallow[]\nhere\nendif::swallow[]\ngone"
        ),
        "poof\ngone"
    );
}

#[test]
fn exclude_with_nested_include_using_shorthand_end() {
    verifies!(
        r#"
      test 'exclude with nested include using shorthand end' do
        input = <<~'EOS'
        poof
        ifndef::grail[]
        no grail
        ifndef::swallow[]
        or swallow
        endif::[]
        in here
        endif::[]
        gone
        EOS

        doc = Asciidoctor::Document.new input, attributes: { 'grail' => '' }
        reader = doc.reader
        lines = []
        while reader.has_more_lines?
          lines << reader.read_line
        end
        assert_equal "poof\ngone", (lines * ::Asciidoctor::LF)
      end

"#
    );

    let parser =
        Parser::default().with_intrinsic_attribute("grail", "", ModificationContext::Anywhere);
    assert_eq!(
        reader_read(
            &parser,
            "poof\nifndef::grail[]\nno grail\nifndef::swallow[]\nor swallow\nendif::[]\nin here\nendif::[]\ngone"
        ),
        "poof\ngone"
    );
}

// `,` combines alternatives with logical OR: any one set includes the content.
#[test]
fn ifdef_with_one_alternative_attribute_set_includes_content() {
    verifies!(
        r#"
      test 'ifdef with one alternative attribute set includes content' do
        input = <<~'EOS'
        ifdef::holygrail,swallow[]
        Our quest is complete!
        endif::holygrail,swallow[]
        EOS

        doc = Asciidoctor::Document.new input, attributes: { 'swallow' => '' }
        reader = doc.reader
        lines = []
        while reader.has_more_lines?
          lines << reader.read_line
        end
        assert_equal 'Our quest is complete!', (lines * ::Asciidoctor::LF)
      end

"#
    );

    let parser =
        Parser::default().with_intrinsic_attribute("swallow", "", ModificationContext::Anywhere);
    assert_eq!(
        reader_read(
            &parser,
            "ifdef::holygrail,swallow[]\nOur quest is complete!\nendif::holygrail,swallow[]"
        ),
        "Our quest is complete!"
    );
}

#[test]
fn ifdef_with_no_alternative_attributes_set_does_not_include_content() {
    verifies!(
        r#"
      test 'ifdef with no alternative attributes set does not include content' do
        input = <<~'EOS'
        ifdef::holygrail,swallow[]
        Our quest is complete!
        endif::holygrail,swallow[]
        EOS

        doc = Asciidoctor::Document.new input
        reader = doc.reader
        lines = []
        while reader.has_more_lines?
          lines << reader.read_line
        end
        assert_equal '', (lines * ::Asciidoctor::LF)
      end

"#
    );

    let parser = Parser::default();
    assert_eq!(
        reader_read(
            &parser,
            "ifdef::holygrail,swallow[]\nOur quest is complete!\nendif::holygrail,swallow[]"
        ),
        ""
    );
}

// `+` combines requirements with logical AND: all must be set.
#[test]
fn ifdef_with_all_required_attributes_set_includes_content() {
    verifies!(
        r#"
      test 'ifdef with all required attributes set includes content' do
        input = <<~'EOS'
        ifdef::holygrail+swallow[]
        Our quest is complete!
        endif::holygrail+swallow[]
        EOS

        doc = Asciidoctor::Document.new input, attributes: { 'holygrail' => '', 'swallow' => '' }
        reader = doc.reader
        lines = []
        while reader.has_more_lines?
          lines << reader.read_line
        end
        assert_equal 'Our quest is complete!', (lines * ::Asciidoctor::LF)
      end

"#
    );

    let parser = Parser::default()
        .with_intrinsic_attribute("holygrail", "", ModificationContext::Anywhere)
        .with_intrinsic_attribute("swallow", "", ModificationContext::Anywhere);
    assert_eq!(
        reader_read(
            &parser,
            "ifdef::holygrail+swallow[]\nOur quest is complete!\nendif::holygrail+swallow[]"
        ),
        "Our quest is complete!"
    );
}

#[test]
fn ifdef_with_missing_required_attributes_does_not_include_content() {
    verifies!(
        r#"
      test 'ifdef with missing required attributes does not include content' do
        input = <<~'EOS'
        ifdef::holygrail+swallow[]
        Our quest is complete!
        endif::holygrail+swallow[]
        EOS

        doc = Asciidoctor::Document.new input, attributes: { 'holygrail' => '' }
        reader = doc.reader
        lines = []
        while reader.has_more_lines?
          lines << reader.read_line
        end
        assert_equal '', (lines * ::Asciidoctor::LF)
      end

"#
    );

    let parser =
        Parser::default().with_intrinsic_attribute("holygrail", "", ModificationContext::Anywhere);
    assert_eq!(
        reader_read(
            &parser,
            "ifdef::holygrail+swallow[]\nOur quest is complete!\nendif::holygrail+swallow[]"
        ),
        ""
    );
}

// Leading, trailing, and repeated `,`/`+` operators are tolerated: an empty
// name between operators is simply an unset attribute (so it never satisfies an
// `all` requirement but is harmless in an `any` alternative). (`asciidoctor` is
// an always-set intrinsic in Asciidoctor; it is set explicitly here.)
#[test]
fn ifdef_should_permit_leading_trailing_and_repeat_operators() {
    verifies!(
        r#"
      test 'ifdef should permit leading, trailing, and repeat operators' do
        {
          'asciidoctor,' => 'content',
          ',asciidoctor' => 'content',
          'asciidoctor+' => '',
          '+asciidoctor' => '',
          'asciidoctor,,asciidoctor-version' => 'content',
          'asciidoctor++asciidoctor-version' => '',
        }.each do |condition, expected|
          input = <<~EOS
          ifdef::#{condition}[]
          content
          endif::[]
          EOS
          assert_equal expected, (document_from_string input, parse: false).reader.read
        end
      end

"#
    );

    let parser = Parser::default().with_intrinsic_attribute(
        "asciidoctor",
        "",
        ModificationContext::Anywhere,
    );
    for (condition, expected) in [
        ("asciidoctor,", "content"),
        (",asciidoctor", "content"),
        ("asciidoctor+", ""),
        ("+asciidoctor", ""),
        ("asciidoctor,,asciidoctor-version", "content"),
        ("asciidoctor++asciidoctor-version", ""),
    ] {
        let input = format!("ifdef::{condition}[]\ncontent\nendif::[]");
        assert_eq!(
            reader_read(&parser, &input),
            expected,
            "condition: {condition}"
        );
    }
}

#[test]
fn ifndef_with_undefined_attribute_includes_block() {
    verifies!(
        r#"
      test 'ifndef with undefined attribute includes block' do
        input = <<~'EOS'
        ifndef::holygrail[]
        Our quest continues to find the holy grail!
        endif::holygrail[]
        EOS

        doc = Asciidoctor::Document.new input
        reader = doc.reader
        lines = []
        while reader.has_more_lines?
          lines << reader.read_line
        end
        assert_equal 'Our quest continues to find the holy grail!', (lines * ::Asciidoctor::LF)
      end

"#
    );

    let parser = Parser::default();
    assert_eq!(
        reader_read(
            &parser,
            "ifndef::holygrail[]\nOur quest continues to find the holy grail!\nendif::holygrail[]"
        ),
        "Our quest continues to find the holy grail!"
    );
}

#[test]
fn ifndef_with_one_alternative_attribute_set_does_not_include_content() {
    verifies!(
        r#"
      test 'ifndef with one alternative attribute set does not include content' do
        input = <<~'EOS'
        ifndef::holygrail,swallow[]
        Our quest is complete!
        endif::holygrail,swallow[]
        EOS

        result = (Asciidoctor::Document.new input, attributes: { 'swallow' => '' }).reader.read
        assert_empty result
      end

"#
    );

    let parser =
        Parser::default().with_intrinsic_attribute("swallow", "", ModificationContext::Anywhere);
    assert_eq!(
        reader_read(
            &parser,
            "ifndef::holygrail,swallow[]\nOur quest is complete!\nendif::holygrail,swallow[]"
        ),
        ""
    );
}

#[test]
fn ifndef_with_both_alternative_attributes_set_does_not_include_content() {
    verifies!(
        r#"
      test 'ifndef with both alternative attributes set does not include content' do
        input = <<~'EOS'
        ifndef::holygrail,swallow[]
        Our quest is complete!
        endif::holygrail,swallow[]
        EOS

        result = (Asciidoctor::Document.new input, attributes: { 'swallow' => '', 'holygrail' => '' }).reader.read
        assert_empty result
      end

"#
    );

    let parser = Parser::default()
        .with_intrinsic_attribute("swallow", "", ModificationContext::Anywhere)
        .with_intrinsic_attribute("holygrail", "", ModificationContext::Anywhere);
    assert_eq!(
        reader_read(
            &parser,
            "ifndef::holygrail,swallow[]\nOur quest is complete!\nendif::holygrail,swallow[]"
        ),
        ""
    );
}

#[test]
fn ifndef_with_no_alternative_attributes_set_includes_content() {
    verifies!(
        r#"
      test 'ifndef with no alternative attributes set includes content' do
        input = <<~'EOS'
        ifndef::holygrail,swallow[]
        Our quest is complete!
        endif::holygrail,swallow[]
        EOS

        result = (Asciidoctor::Document.new input).reader.read
        assert_equal 'Our quest is complete!', result
      end

"#
    );

    let parser = Parser::default();
    assert_eq!(
        reader_read(
            &parser,
            "ifndef::holygrail,swallow[]\nOur quest is complete!\nendif::holygrail,swallow[]"
        ),
        "Our quest is complete!"
    );
}

#[test]
fn ifndef_with_no_required_attributes_set_includes_content() {
    verifies!(
        r#"
      test 'ifndef with no required attributes set includes content' do
        input = <<~'EOS'
        ifndef::holygrail+swallow[]
        Our quest is complete!
        endif::holygrail+swallow[]
        EOS

        result = (Asciidoctor::Document.new input).reader.read
        assert_equal 'Our quest is complete!', result
      end

"#
    );

    let parser = Parser::default();
    assert_eq!(
        reader_read(
            &parser,
            "ifndef::holygrail+swallow[]\nOur quest is complete!\nendif::holygrail+swallow[]"
        ),
        "Our quest is complete!"
    );
}

#[test]
fn ifndef_with_all_required_attributes_set_does_not_include_content() {
    verifies!(
        r#"
      test 'ifndef with all required attributes set does not include content' do
        input = <<~'EOS'
        ifndef::holygrail+swallow[]
        Our quest is complete!
        endif::holygrail+swallow[]
        EOS

        result = (Asciidoctor::Document.new input, attributes: { 'swallow' => '', 'holygrail' => '' }).reader.read
        assert_empty result
      end

"#
    );

    let parser = Parser::default()
        .with_intrinsic_attribute("swallow", "", ModificationContext::Anywhere)
        .with_intrinsic_attribute("holygrail", "", ModificationContext::Anywhere);
    assert_eq!(
        reader_read(
            &parser,
            "ifndef::holygrail+swallow[]\nOur quest is complete!\nendif::holygrail+swallow[]"
        ),
        ""
    );
}

#[test]
fn ifndef_with_at_least_one_required_attributes_set_does_not_include_content() {
    verifies!(
        r#"
      test 'ifndef with at least one required attributes set does not include content' do
        input = <<~'EOS'
        ifndef::holygrail+swallow[]
        Our quest is complete!
        endif::holygrail+swallow[]
        EOS

        result = (Asciidoctor::Document.new input, attributes: { 'swallow' => '' }).reader.read
        assert_equal 'Our quest is complete!', result
      end

"#
    );

    let parser =
        Parser::default().with_intrinsic_attribute("swallow", "", ModificationContext::Anywhere);
    assert_eq!(
        reader_read(
            &parser,
            "ifndef::holygrail+swallow[]\nOur quest is complete!\nendif::holygrail+swallow[]"
        ),
        "Our quest is complete!"
    );
}

// A skipped block that wraps a blank line does not leave a stray blank behind.
#[test]
fn ifdef_around_empty_line_does_not_introduce_extra_line() {
    verifies!(
        r#"
      test 'ifdef around empty line does not introduce extra line' do
        input = <<~'EOS'
        before
        ifdef::no-such-attribute[]

        endif::[]
        after
        EOS

        result = (Asciidoctor::Document.new input).reader.read
        assert_equal %(before\nafter), result
      end

"#
    );

    let parser = Parser::default();
    assert_eq!(
        reader_read(
            &parser,
            "before\nifdef::no-such-attribute[]\n\nendif::[]\nafter"
        ),
        "before\nafter"
    );
}

// An `endif` with no matching open conditional is discarded (the enclosed
// content survives) and reported as an unmatched-directive error, located at
// the stray `endif`'s own line.
#[test]
fn should_log_warning_if_endif_is_unmatched() {
    verifies!(
        r#"
      test 'should log warning if endif is unmatched' do
        input = <<~'EOS'
        Our quest is complete!
        endif::on-quest[]
        EOS

        using_memory_logger do |logger|
          result = (Asciidoctor::Document.new input, attributes: { 'on-quest' => '' }).reader.read
          assert_equal 'Our quest is complete!', result
          assert_message logger, :ERROR, '~<stdin>: line 2: unmatched preprocessor directive: endif::on-quest[]', Hash
        end
      end

"#
    );

    let doc = Parser::default()
        .with_intrinsic_attribute("on-quest", "", ModificationContext::Anywhere)
        .parse("Our quest is complete!\nendif::on-quest[]");

    assert_eq!(rendered_paragraphs(&doc), vec!["Our quest is complete!"]);

    let warnings: Vec<_> = doc.warnings().collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0].warning,
        WarningType::UnmatchedConditionalDirective("endif::on-quest[]".to_owned())
    );
    assert_eq!(warnings[0].origin, Some(crate::parser::SourceLine(None, 2)));
}

// An `endif` naming a different target than the open conditional is
// discarded (it closes nothing), so the opening `ifdef` is left unterminated:
// both the mismatch (at the `endif`'s line) and the unterminated conditional
// (at the `ifdef`'s line) are reported.
#[test]
fn should_log_warning_if_endif_is_mismatched() {
    verifies!(
        r#"
      test 'should log warning if endif is mismatched' do
        input = <<~'EOS'
        ifdef::on-quest[]
        Our quest is complete!
        endif::on-journey[]
        EOS

        using_memory_logger do |logger|
          result = (Asciidoctor::Document.new input, attributes: { 'on-quest' => '' }, sourcemap: true).reader.read
          assert_equal 'Our quest is complete!', result
          assert_messages logger, [
            [:ERROR, '~<stdin>: line 3: mismatched preprocessor directive: endif::on-journey[]', Hash],
            [:ERROR, '~<stdin>: line 1: detected unterminated preprocessor conditional directive: ifdef::on-quest[]', Hash],
          ]
        end
      end

"#
    );

    let doc = Parser::default()
        .with_intrinsic_attribute("on-quest", "", ModificationContext::Anywhere)
        .parse("ifdef::on-quest[]\nOur quest is complete!\nendif::on-journey[]");

    assert_eq!(rendered_paragraphs(&doc), vec!["Our quest is complete!"]);

    let warnings: Vec<_> = doc.warnings().collect();
    assert_eq!(warnings.len(), 2);
    assert_eq!(
        warnings[0].warning,
        WarningType::MismatchedConditionalDirective("endif::on-journey[]".to_owned())
    );
    assert_eq!(warnings[0].origin, Some(crate::parser::SourceLine(None, 3)));
    assert_eq!(
        warnings[1].warning,
        WarningType::UnterminatedConditionalDirective("ifdef::on-quest[]".to_owned())
    );
    assert_eq!(warnings[1].origin, Some(crate::parser::SourceLine(None, 1)));
}

// An `endif` with bracketed text is malformed (text is not permitted) and
// closes nothing, so the opening `ifdef` is again left unterminated. Both the
// malformed-directive error and the unterminated-conditional error are
// reported, and the surrounding content flows normally.
#[test]
fn should_log_warning_if_endif_contains_text() {
    verifies!(
        r#"
      test 'should log warning if endif contains text' do
        input = <<~'EOS'
        ifdef::on-quest[]
        Our quest is complete!
        endif::on-quest[complete!]
        fin
        EOS

        using_memory_logger do |logger|
          result = (Asciidoctor::Document.new input, attributes: { 'on-quest' => '' }, sourcemap: true).reader.read
          assert_equal %(Our quest is complete!\nfin), result
          assert_messages logger, [
            [:ERROR, '~<stdin>: line 3: malformed preprocessor directive - text not permitted: endif::on-quest[complete!]', Hash],
            [:ERROR, '~<stdin>: line 1: detected unterminated preprocessor conditional directive: ifdef::on-quest[]', Hash],
          ]
        end
      end

"#
    );

    let doc = Parser::default()
        .with_intrinsic_attribute("on-quest", "", ModificationContext::Anywhere)
        .parse("ifdef::on-quest[]\nOur quest is complete!\nendif::on-quest[complete!]\nfin");

    // The two surviving lines are adjacent, so they form a single paragraph.
    assert_eq!(
        rendered_paragraphs(&doc),
        vec!["Our quest is complete!\nfin"]
    );

    let warnings: Vec<_> = doc.warnings().collect();
    assert_eq!(warnings.len(), 2);
    assert_eq!(
        warnings[0].warning,
        WarningType::MalformedConditionalDirective(
            "text not permitted".to_owned(),
            "endif::on-quest[complete!]".to_owned()
        )
    );
    assert_eq!(warnings[0].origin, Some(crate::parser::SourceLine(None, 3)));
    assert_eq!(
        warnings[1].warning,
        WarningType::UnterminatedConditionalDirective("ifdef::on-quest[]".to_owned())
    );
    assert_eq!(warnings[1].origin, Some(crate::parser::SourceLine(None, 1)));
}

// An escaped conditional directive is passed through with its backslash
// removed and is not processed.
#[test]
fn escaped_ifdef_is_unescaped_and_ignored() {
    verifies!(
        r#"
      test 'escaped ifdef is unescaped and ignored' do
        input = <<~'EOS'
        \ifdef::holygrail[]
        content
        \endif::holygrail[]
        EOS

        doc = Asciidoctor::Document.new input
        reader = doc.reader
        lines = []
        while reader.has_more_lines?
          lines << reader.read_line
        end
        assert_equal "ifdef::holygrail[]\ncontent\nendif::holygrail[]", (lines * ::Asciidoctor::LF)
      end

"#
    );

    let parser = Parser::default();
    assert_eq!(
        reader_read(
            &parser,
            "\\ifdef::holygrail[]\ncontent\n\\endif::holygrail[]"
        ),
        "ifdef::holygrail[]\ncontent\nendif::holygrail[]"
    );
}

// A reference to an unset attribute resolves to the empty string in an
// `ifeval` operand (see issue #779), so `'{foo}' == ''` is true and the
// content is included.
#[test]
fn ifeval_comparing_missing_attribute_to_nil_includes_content() {
    verifies!(
        r#"
      test 'ifeval comparing missing attribute to nil includes content' do
        input = <<~'EOS'
        ifeval::['{foo}' == '']
        No foo for you!
        endif::[]
        EOS

        doc = Asciidoctor::Document.new input
        reader = doc.reader
        lines = []
        while reader.has_more_lines?
          lines << reader.read_line
        end
        assert_equal 'No foo for you!', (lines * ::Asciidoctor::LF)
      end

"#
    );

    let parser = Parser::default();
    assert_eq!(
        reader_read(
            &parser,
            "ifeval::['{foo}' == '']\nNo foo for you!\nendif::[]"
        ),
        "No foo for you!"
    );
}

// The unset (and unquoted) `{leveloffset}` reference resolves to empty and
// thus coerces to nil, which is not equal to 0, so the content is dropped.
#[test]
fn ifeval_comparing_missing_attribute_to_0_drops_content() {
    verifies!(
        r#"
      test 'ifeval comparing missing attribute to 0 drops content' do
        input = <<~'EOS'
        ifeval::[{leveloffset} == 0]
        I didn't make the cut!
        endif::[]
        EOS

        doc = Asciidoctor::Document.new input
        reader = doc.reader
        lines = []
        while reader.has_more_lines?
          lines << reader.read_line
        end
        assert_equal '', (lines * ::Asciidoctor::LF)
      end

"#
    );

    let parser = Parser::default();
    assert_eq!(
        reader_read(
            &parser,
            "ifeval::[{leveloffset} == 0]\nI didn't make the cut!\nendif::[]"
        ),
        ""
    );
}

#[test]
fn ifeval_running_unsupported_operation_on_missing_attribute_drops_content() {
    verifies!(
        r#"
      test 'ifeval running unsupported operation on missing attribute drops content' do
        input = <<~'EOS'
        ifeval::[{leveloffset} >= 3]
        I didn't make the cut!
        endif::[]
        EOS

        doc = Asciidoctor::Document.new input
        reader = doc.reader
        lines = []
        while reader.has_more_lines?
          lines << reader.read_line
        end
        assert_equal '', (lines * ::Asciidoctor::LF)
      end

"#
    );

    let parser = Parser::default();
    assert_eq!(
        reader_read(
            &parser,
            "ifeval::[{leveloffset} >= 3]\nI didn't make the cut!\nendif::[]"
        ),
        ""
    );
}

#[test]
fn ifeval_running_invalid_operation_drops_content() {
    verifies!(
        r#"
      test 'ifeval running invalid operation drops content' do
        input = <<~'EOS'
        ifeval::[{asciidoctor-version} > true]
        I didn't make the cut!
        endif::[]
        EOS

        doc = Asciidoctor::Document.new input
        reader = doc.reader
        lines = []
        while reader.has_more_lines?
          lines << reader.read_line
        end
        assert_equal '', (lines * ::Asciidoctor::LF)
      end

"#
    );

    // `asciidoctor-version` is predefined by this crate, so – as in Ruby – the
    // invalid operation is a number compared against a boolean.
    assert_eq!(
        reader_read(
            &Parser::default(),
            "ifeval::[{asciidoctor-version} > true]\nI didn't make the cut!\nendif::[]"
        ),
        ""
    );
}

#[test]
fn ifeval_comparing_double_quoted_attribute_to_matching_string_includes_content() {
    verifies!(
        r#"
      test 'ifeval comparing double-quoted attribute to matching string includes content' do
        input = <<~'EOS'
        ifeval::["{gem}" == "asciidoctor"]
        Asciidoctor it is!
        endif::[]
        EOS

        doc = Asciidoctor::Document.new input, attributes: { 'gem' => 'asciidoctor' }
        reader = doc.reader
        lines = []
        while reader.has_more_lines?
          lines << reader.read_line
        end
        assert_equal 'Asciidoctor it is!', (lines * ::Asciidoctor::LF)
      end

"#
    );

    let parser = Parser::default().with_intrinsic_attribute(
        "gem",
        "asciidoctor",
        ModificationContext::Anywhere,
    );
    assert_eq!(
        reader_read(
            &parser,
            "ifeval::[\"{gem}\" == \"asciidoctor\"]\nAsciidoctor it is!\nendif::[]"
        ),
        "Asciidoctor it is!"
    );
}

#[test]
fn ifeval_comparing_single_quoted_attribute_to_matching_string_includes_content() {
    verifies!(
        r#"
      test 'ifeval comparing single-quoted attribute to matching string includes content' do
        input = <<~'EOS'
        ifeval::['{gem}' == 'asciidoctor']
        Asciidoctor it is!
        endif::[]
        EOS

        doc = Asciidoctor::Document.new input, attributes: { 'gem' => 'asciidoctor' }
        reader = doc.reader
        lines = []
        while reader.has_more_lines?
          lines << reader.read_line
        end
        assert_equal 'Asciidoctor it is!', (lines * ::Asciidoctor::LF)
      end

"#
    );

    let parser = Parser::default().with_intrinsic_attribute(
        "gem",
        "asciidoctor",
        ModificationContext::Anywhere,
    );
    assert_eq!(
        reader_read(
            &parser,
            "ifeval::['{gem}' == 'asciidoctor']\nAsciidoctor it is!\nendif::[]"
        ),
        "Asciidoctor it is!"
    );
}

#[test]
fn ifeval_comparing_quoted_attribute_to_non_matching_string_drops_content() {
    verifies!(
        r#"
      test 'ifeval comparing quoted attribute to non-matching string drops content' do
        input = <<~'EOS'
        ifeval::['{gem}' == 'asciidoctor']
        Asciidoctor it is!
        endif::[]
        EOS

        doc = Asciidoctor::Document.new input, attributes: { 'gem' => 'tilt' }
        reader = doc.reader
        lines = []
        while reader.has_more_lines?
          lines << reader.read_line
        end
        assert_equal '', (lines * ::Asciidoctor::LF)
      end

"#
    );

    let parser =
        Parser::default().with_intrinsic_attribute("gem", "tilt", ModificationContext::Anywhere);
    assert_eq!(
        reader_read(
            &parser,
            "ifeval::['{gem}' == 'asciidoctor']\nAsciidoctor it is!\nendif::[]"
        ),
        ""
    );
}

#[test]
fn ifeval_comparing_attribute_to_lower_version_number_includes_content() {
    verifies!(
        r#"
      test 'ifeval comparing attribute to lower version number includes content' do
        input = <<~'EOS'
        ifeval::['{asciidoctor-version}' >= '0.1.0']
        That version will do!
        endif::[]
        EOS

        doc = Asciidoctor::Document.new input
        reader = doc.reader
        lines = []
        while reader.has_more_lines?
          lines << reader.read_line
        end
        assert_equal 'That version will do!', (lines * ::Asciidoctor::LF)
      end

"#
    );

    assert_eq!(
        reader_read(
            &Parser::default(),
            "ifeval::['{asciidoctor-version}' >= '0.1.0']\nThat version will do!\nendif::[]"
        ),
        "That version will do!"
    );
}

// A reference always equals itself, so the content is included. (Here the
// attribute is unset, so both sides resolve to the empty string.)
#[test]
fn ifeval_comparing_attribute_to_self_includes_content() {
    verifies!(
        r#"
      test 'ifeval comparing attribute to self includes content' do
        input = <<~'EOS'
        ifeval::['{asciidoctor-version}' == '{asciidoctor-version}']
        Of course it's the same!
        endif::[]
        EOS

        doc = Asciidoctor::Document.new input
        reader = doc.reader
        lines = []
        while reader.has_more_lines?
          lines << reader.read_line
        end
        assert_equal 'Of course it\'s the same!', (lines * ::Asciidoctor::LF)
      end

"#
    );

    let parser = Parser::default();
    assert_eq!(
        reader_read(
            &parser,
            "ifeval::['{asciidoctor-version}' == '{asciidoctor-version}']\nOf course it's the same!\nendif::[]"
        ),
        "Of course it's the same!"
    );
}

// The operands may be given in either order.
#[test]
fn ifeval_arguments_can_be_transposed() {
    verifies!(
        r#"
      test 'ifeval arguments can be transposed' do
        input = <<~'EOS'
        ifeval::['0.1.0' <= '{asciidoctor-version}']
        That version will do!
        endif::[]
        EOS

        doc = Asciidoctor::Document.new input
        reader = doc.reader
        lines = []
        while reader.has_more_lines?
          lines << reader.read_line
        end
        assert_equal 'That version will do!', (lines * ::Asciidoctor::LF)
      end

"#
    );

    assert_eq!(
        reader_read(
            &Parser::default(),
            "ifeval::['0.1.0' <= '{asciidoctor-version}']\nThat version will do!\nendif::[]"
        ),
        "That version will do!"
    );
}

#[test]
fn ifeval_matching_numeric_equality_includes_content() {
    verifies!(
        r#"
      test 'ifeval matching numeric equality includes content' do
        input = <<~'EOS'
        ifeval::[{rings} == 1]
        One ring to rule them all!
        endif::[]
        EOS

        doc = Asciidoctor::Document.new input, attributes: { 'rings' => '1' }
        reader = doc.reader
        lines = []
        while reader.has_more_lines?
          lines << reader.read_line
        end
        assert_equal 'One ring to rule them all!', (lines * ::Asciidoctor::LF)
      end

"#
    );

    let parser =
        Parser::default().with_intrinsic_attribute("rings", "1", ModificationContext::Anywhere);
    assert_eq!(
        reader_read(
            &parser,
            "ifeval::[{rings} == 1]\nOne ring to rule them all!\nendif::[]"
        ),
        "One ring to rule them all!"
    );
}

#[test]
fn ifeval_matching_numeric_inequality_includes_content() {
    verifies!(
        r#"
      test 'ifeval matching numeric inequality includes content' do
        input = <<~'EOS'
        ifeval::[{rings} != 0]
        One ring to rule them all!
        endif::[]
        EOS

        doc = Asciidoctor::Document.new input, attributes: { 'rings' => '1' }
        reader = doc.reader
        lines = []
        while reader.has_more_lines?
          lines << reader.read_line
        end
        assert_equal 'One ring to rule them all!', (lines * ::Asciidoctor::LF)
      end

"#
    );

    let parser =
        Parser::default().with_intrinsic_attribute("rings", "1", ModificationContext::Anywhere);
    assert_eq!(
        reader_read(
            &parser,
            "ifeval::[{rings} != 0]\nOne ring to rule them all!\nendif::[]"
        ),
        "One ring to rule them all!"
    );
}

// `ifeval` requires an empty target; a non-empty one is malformed. The
// directive is dropped (opening no conditional) and the following content flows
// normally, with a target-not-permitted error reported at the directive's line.
#[test]
fn should_warn_if_ifeval_has_target() {
    verifies!(
        r#"
      test 'should warn if ifeval has target' do
        input = <<~'EOS'
        ifeval::target[1 == 1]
        content
        EOS

        using_memory_logger do |logger|
          doc = Asciidoctor::Document.new input
          reader = doc.reader
          lines = []
          lines << reader.read_line while reader.has_more_lines?
          assert_equal 'content', (lines * ::Asciidoctor::LF)
          assert_message logger, :ERROR, '~<stdin>: line 1: malformed preprocessor directive - target not permitted: ifeval::target[1 == 1]', Hash
        end
      end

"#
    );

    let doc = Parser::default().parse("ifeval::target[1 == 1]\ncontent");

    assert_eq!(rendered_paragraphs(&doc), vec!["content"]);

    let warnings: Vec<_> = doc.warnings().collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0].warning,
        WarningType::MalformedConditionalDirective(
            "target not permitted".to_owned(),
            "ifeval::target[1 == 1]".to_owned()
        )
    );
    assert_eq!(warnings[0].origin, Some(crate::parser::SourceLine(None, 1)));
}

// A bracketed expression with no comparison operator cannot be parsed: the
// directive is malformed, dropped, and reported as an invalid expression.
#[test]
fn should_warn_if_ifeval_has_invalid_expression() {
    verifies!(
        r#"
      test 'should warn if ifeval has invalid expression' do
        input = <<~'EOS'
        ifeval::[1 | 2]
        content
        EOS

        using_memory_logger do |logger|
          doc = Asciidoctor::Document.new input
          reader = doc.reader
          lines = []
          lines << reader.read_line while reader.has_more_lines?
          assert_equal 'content', (lines * ::Asciidoctor::LF)
          assert_message logger, :ERROR, '~<stdin>: line 1: malformed preprocessor directive - invalid expression: ifeval::[1 | 2]', Hash
        end
      end

"#
    );

    let doc = Parser::default().parse("ifeval::[1 | 2]\ncontent");

    assert_eq!(rendered_paragraphs(&doc), vec!["content"]);

    let warnings: Vec<_> = doc.warnings().collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0].warning,
        WarningType::MalformedConditionalDirective(
            "invalid expression".to_owned(),
            "ifeval::[1 | 2]".to_owned()
        )
    );
    assert_eq!(warnings[0].origin, Some(crate::parser::SourceLine(None, 1)));
}

// An empty bracketed expression is malformed (missing expression): the
// directive is dropped and reported, and the content flows normally. (The old
// crate behavior — treating an empty expression as false and *enclosing* the
// following lines — was incorrect; see the preprocessor unit tests.)
#[test]
fn should_warn_if_ifeval_is_missing_expression() {
    verifies!(
        r#"
      test 'should warn if ifeval is missing expression' do
        input = <<~'EOS'
        ifeval::[]
        content
        EOS

        using_memory_logger do |logger|
          doc = Asciidoctor::Document.new input
          reader = doc.reader
          lines = []
          lines << reader.read_line while reader.has_more_lines?
          assert_equal 'content', (lines * ::Asciidoctor::LF)
          assert_message logger, :ERROR, '~<stdin>: line 1: malformed preprocessor directive - missing expression: ifeval::[]', Hash
        end
      end

"#
    );

    let doc = Parser::default().parse("ifeval::[]\ncontent");

    assert_eq!(rendered_paragraphs(&doc), vec!["content"]);

    let warnings: Vec<_> = doc.warnings().collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0].warning,
        WarningType::MalformedConditionalDirective(
            "missing expression".to_owned(),
            "ifeval::[]".to_owned()
        )
    );
    assert_eq!(warnings[0].origin, Some(crate::parser::SourceLine(None, 1)));
}

// `ifdef`/`ifndef` require a target (attribute name); an empty one is malformed
// (missing target). The directive is dropped and reported, and the content
// flows normally.
#[test]
fn ifdef_with_no_target_is_ignored() {
    verifies!(
        r#"
      test 'ifdef with no target is ignored' do
        input = <<~'EOS'
        ifdef::[]
        content
        EOS

        using_memory_logger do |logger|
          doc = Asciidoctor::Document.new input
          reader = doc.reader
          lines = []
          lines << reader.read_line while reader.has_more_lines?
          assert_equal 'content', (lines * ::Asciidoctor::LF)
          assert_message logger, :ERROR, '~<stdin>: line 1: malformed preprocessor directive - missing target: ifdef::[]', Hash
        end
      end

"#
    );

    let doc = Parser::default().parse("ifdef::[]\ncontent");

    assert_eq!(rendered_paragraphs(&doc), vec!["content"]);

    let warnings: Vec<_> = doc.warnings().collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0].warning,
        WarningType::MalformedConditionalDirective(
            "missing target".to_owned(),
            "ifdef::[]".to_owned()
        )
    );
    assert_eq!(warnings[0].origin, Some(crate::parser::SourceLine(None, 1)));
}

// A malformed `ifdef` inside an already-skipping region is silently dropped: it
// neither warns nor opens a conditional, so the anonymous `endif` still closes
// the outer skipped region and `baz` is the only surviving content.
#[test]
fn should_not_warn_about_invalid_ifdef_preprocessor_directive_if_already_skipping() {
    verifies!(
        r#"
      test 'should not warn about invalid ifdef preprocessor directive if already skipping' do
        input = <<~'EOS'
        ifdef::attribute-not-set[]
        foo
        ifdef::[]
        bar
        endif::[]
        baz
        EOS

        using_memory_logger do |logger|
          result = (Asciidoctor::Document.new input).reader.read
          assert_equal 'baz', result
          assert_empty logger
        end
      end

"#
    );

    let doc =
        Parser::default().parse("ifdef::attribute-not-set[]\nfoo\nifdef::[]\nbar\nendif::[]\nbaz");

    assert_eq!(rendered_paragraphs(&doc), vec!["baz"]);
    assert_eq!(doc.warnings().count(), 0);
}

// Likewise a malformed `ifeval` inside an already-skipping region is silently
// dropped.
#[test]
fn should_not_warn_about_invalid_ifeval_preprocessor_directive_if_already_skipping() {
    verifies!(
        r#"
      test 'should not warn about invalid ifeval preprocessor directive if already skipping' do
        input = <<~'EOS'
        ifdef::attribute-not-set[]
        foo
        ifeval::[]
        bar
        endif::[]
        baz
        EOS

        using_memory_logger do |logger|
          result = (Asciidoctor::Document.new input).reader.read
          assert_equal 'baz', result
          assert_empty logger
        end
      end

"#
    );

    let doc =
        Parser::default().parse("ifdef::attribute-not-set[]\nfoo\nifeval::[]\nbar\nendif::[]\nbaz");

    assert_eq!(rendered_paragraphs(&doc), vec!["baz"]);
    assert_eq!(doc.warnings().count(), 0);
}

// An `ifdef` that is never closed skips everything to the end of the source and
// is reported as unterminated. Asciidoctor, with no source map, reports it at
// the *end* of the reader (line 6); this crate always maintains a source map,
// so — like Asciidoctor *with* `sourcemap` (see the next test) — it reports the
// directive at its own line (line 2).
#[test]
fn should_log_error_with_end_position_if_preprocessor_conditional_directive_is_unterminated() {
    verifies!(
        r#"
      test 'should log error with end position if preprocessor conditional directive is unterminated' do
        input = <<~'EOS'
        before
        ifdef::not-set[]
        skip
        these
        lines
        fin
        EOS

        using_memory_logger do |logger|
          doc = Asciidoctor::Document.new input
          reader = doc.reader
          lines = []
          lines << reader.read_line while reader.has_more_lines?
          assert_equal 'before', (lines * Asciidoctor::LF)
          assert_message logger, :ERROR, '~<stdin>: line 6: detected unterminated preprocessor conditional directive: ifdef::not-set[]', Hash
        end
      end

"#
    );

    let doc = Parser::default().parse("before\nifdef::not-set[]\nskip\nthese\nlines\nfin");

    assert_eq!(rendered_paragraphs(&doc), vec!["before"]);

    let warnings: Vec<_> = doc.warnings().collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0].warning,
        WarningType::UnterminatedConditionalDirective("ifdef::not-set[]".to_owned())
    );
    // Reported at the directive's own line (line 2), not the end of the source.
    assert_eq!(warnings[0].origin, Some(crate::parser::SourceLine(None, 2)));
}

// With a source map (always, for this crate), the unterminated conditional is
// reported at its opening line (line 2) — matching Asciidoctor's `sourcemap`
// behavior exactly.
#[test]
fn should_log_error_with_start_location_if_preprocessor_conditional_directive_is_unterminated_and_sourcemap_is_set()
 {
    verifies!(
        r#"
      test 'should log error with start location if preprocessor conditional directive is unterminated and sourcemap is set' do
        input = <<~'EOS'
        before
        ifdef::not-set[]
        skip
        these
        lines
        fin
        EOS

        using_memory_logger do |logger|
          doc = Asciidoctor::Document.new input, sourcemap: true
          reader = doc.reader
          lines = []
          lines << reader.read_line while reader.has_more_lines?
          assert_equal 'before', (lines * Asciidoctor::LF)
          assert_message logger, :ERROR, '~<stdin>: line 2: detected unterminated preprocessor conditional directive: ifdef::not-set[]', Hash
        end
      end

"#
    );

    let doc = Parser::default().parse("before\nifdef::not-set[]\nskip\nthese\nlines\nfin");

    assert_eq!(rendered_paragraphs(&doc), vec!["before"]);

    let warnings: Vec<_> = doc.warnings().collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0].warning,
        WarningType::UnterminatedConditionalDirective("ifdef::not-set[]".to_owned())
    );
    assert_eq!(warnings[0].origin, Some(crate::parser::SourceLine(None, 2)));
}

// Every conditional still open at end of source is reported, in the order the
// directives were opened, each at its own line. Here the never-closed
// `ifdef::not-set[]` (line 2) skips to the end, and the `ifeval::[1 == 2]`
// opened within that skipped region (line 6) is likewise unterminated.
#[test]
fn should_log_error_if_multiple_preprocessor_conditional_directives_are_unterminated() {
    verifies!(
        r#"
      test 'should log error if multiple preprocessor conditional directives are unterminated' do
        input = <<~'EOS'
        before
        ifdef::not-set[]
        skip
        these
        lines
        ifeval::[1 == 2]
        {asciidoctor-version}
        fin
        EOS

        using_memory_logger do |logger|
          doc = Asciidoctor::Document.new input, sourcemap: true
          reader = doc.reader
          lines = []
          lines << reader.read_line while reader.has_more_lines?
          assert_equal 'before', (lines * Asciidoctor::LF)
          assert_messages logger, [
            [:ERROR, '~<stdin>: line 2: detected unterminated preprocessor conditional directive: ifdef::not-set[]', Hash],
            [:ERROR, '~<stdin>: line 6: detected unterminated preprocessor conditional directive: ifeval::[1 == 2]', Hash],
          ]
        end
      end

"#
    );

    let doc = Parser::default()
        .parse("before\nifdef::not-set[]\nskip\nthese\nlines\nifeval::[1 == 2]\n{asciidoctor-version}\nfin");

    assert_eq!(rendered_paragraphs(&doc), vec!["before"]);

    let warnings: Vec<_> = doc.warnings().collect();
    assert_eq!(warnings.len(), 2);
    assert_eq!(
        warnings[0].warning,
        WarningType::UnterminatedConditionalDirective("ifdef::not-set[]".to_owned())
    );
    assert_eq!(warnings[0].origin, Some(crate::parser::SourceLine(None, 2)));
    assert_eq!(
        warnings[1].warning,
        WarningType::UnterminatedConditionalDirective("ifeval::[1 == 2]".to_owned())
    );
    assert_eq!(warnings[1].origin, Some(crate::parser::SourceLine(None, 6)));
}

// A properly-closed, false conditional enclosing a very large block is skipped
// wholesale, leaving just the surrounding paragraphs (and no warning).
#[test]
fn should_not_fail_to_process_preprocessor_directive_that_evaluates_to_false_and_has_a_large_number_of_lines()
 {
    verifies!(
        r#"
      test 'should not fail to process preprocessor directive that evaluates to false and has a large number of lines' do
        lines = (%w(data) * 5000) * ?\n
        input = <<~EOS
        before

        ifdef::attribute-not-set[]
        #{lines}
        endif::attribute-not-set[]

        after
        EOS

        doc = Asciidoctor.load input
        assert_equal 2, doc.blocks.size
        assert_equal 'before', doc.blocks[0].source
        assert_equal 'after', doc.blocks[1].source
      end

"#
    );

    let big = vec!["data"; 5000].join("\n");
    let input =
        format!("before\n\nifdef::attribute-not-set[]\n{big}\nendif::attribute-not-set[]\n\nafter");

    let doc = Parser::default().parse(&input);

    assert_eq!(rendered_paragraphs(&doc), vec!["before", "after"]);
    assert_eq!(doc.warnings().count(), 0);
}

// Out of scope: drives a custom `preprocessor` extension that pokes a `nil`
// into the reader's `source_lines`. This crate has no extension API (see the
// crate README) and no mutable reader line array.
non_normative!(
    r#"
      test 'should not fail to process lines if reader contains a nil entry' do
        input = ['before', '', '', '', 'after']
        doc = Asciidoctor.load input, extensions: proc {
          preprocessor do
            process do |_, reader|
              reader.source_lines[2] = nil
              nil
            end
          end
        }
        assert_equal 2, doc.blocks.size
        assert_equal 'before', doc.blocks[0].source
        assert_equal 'after', doc.blocks[1].source
      end
"#
);

non_normative!(
    r#"
    end
  end
end
"#
);
