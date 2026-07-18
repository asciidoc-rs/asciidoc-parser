// Adapted from Asciidoctor's logger test suite, found in
// https://github.com/asciidoctor/asciidoctor/blob/main/test/logger_test.rb.
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
//! Port of Asciidoctor's `logger_test.rb`.
//!
//! This suite exercises Asciidoctor's *logging* subsystem: the
//! `Asciidoctor::LoggerManager` static accessor and its swappable
//! `logger` / `logger_class` properties, the `Asciidoctor::Logger` and
//! `Asciidoctor::NullLogger` classes (progname, level, `$stderr` logdev,
//! traditional message formatter), the `:logger` API option accepted by the
//! `load` / `load_file` / `convert` / `convert_file` entry points, and the
//! `Asciidoctor::Logging` mixin (`logger` accessor + `message_with_context`).
//!
//! None of that infrastructure lives in this crate. `asciidoc-parser` does not
//! own a pluggable logger, a `LoggerManager`, or the top-level `load` /
//! `convert` APIs; it reports parse-time conditions as structured
//! [`Warning`](crate::warnings::Warning) values via [`Document::warnings`],
//! rather than by emitting formatted text to a Ruby `Logger`. So the file is
//! almost entirely non-normative for this crate, confirming the expectation
//! that `logger_test.rb` tests APIs intentionally outside its scope.
//!
//! The one exception is the final test, which drives a document whose two
//! blocks claim the same explicit id. This crate models that condition (it is
//! the observable behavior behind Asciidoctor's log message), so it is ported
//! as a `verifies!` block that asserts a [`WarningType::DuplicateId`] warning.
//!
//! [`Document::warnings`]: crate::Document::warnings

use crate::tests::prelude::*;

track_file!("ref/asciidoctor/test/logger_test.rb");

// `context 'Logger'` opens the suite and defines a `MyLogger` subclass of
// Ruby's stdlib `Logger` used throughout. Ruby test scaffolding, not parser
// behavior.
non_normative!(
    r#"
# frozen_string_literal: true
require_relative 'test_helper'

context 'Logger' do
  MyLogger = Class.new Logger

"#
);

// `context 'LoggerManager'` — the static `Asciidoctor::LoggerManager.logger`
// accessor: default instance, swapping the instance, resetting to default on a
// falsy value, and instantiating from `logger_class`. This crate has no
// equivalent global logger registry.
non_normative!(
    r#"
  context 'LoggerManager' do
    test 'provides access to logger via static logger method' do
      logger = Asciidoctor::LoggerManager.logger
      refute_nil logger
      assert_kind_of Logger, logger
    end

    test 'allows logger instance to be changed' do
      old_logger = Asciidoctor::LoggerManager.logger
      new_logger = MyLogger.new $stdout
      begin
        Asciidoctor::LoggerManager.logger = new_logger
        assert_same new_logger, Asciidoctor::LoggerManager.logger
      ensure
        Asciidoctor::LoggerManager.logger = old_logger
      end
    end

    test 'setting logger instance to falsy value resets instance to default logger' do
      old_logger = Asciidoctor::LoggerManager.logger
      begin
        Asciidoctor::LoggerManager.logger = MyLogger.new $stdout
        Asciidoctor::LoggerManager.logger = nil
        refute_nil Asciidoctor::LoggerManager.logger
        assert_kind_of Logger, Asciidoctor::LoggerManager.logger
      ensure
        Asciidoctor::LoggerManager.logger = old_logger
      end
    end

    test 'creates logger instance from static logger_class property' do
      old_logger_class = Asciidoctor::LoggerManager.logger_class
      old_logger = Asciidoctor::LoggerManager.logger
      begin
        Asciidoctor::LoggerManager.logger_class = MyLogger
        Asciidoctor::LoggerManager.logger = nil
        refute_nil Asciidoctor::LoggerManager.logger
        assert_kind_of MyLogger, Asciidoctor::LoggerManager.logger
      ensure
        Asciidoctor::LoggerManager.logger_class = old_logger_class
        Asciidoctor::LoggerManager.logger = old_logger
      end
    end
  end

"#
);

// `context 'Logger'` — the `Asciidoctor::Logger` / `NullLogger` classes:
// progname `asciidoctor`, default `WARN` level, writing to `$stderr`, routing
// to a file logdev, and the traditional `asciidoctor: SEVERITY: message`
// formatter. This crate does not expose a `Logger` type or format log lines.
non_normative!(
    r#"
  context 'Logger' do
    test 'configures default logger with progname set to asciidoctor' do
      assert_equal 'asciidoctor', Asciidoctor::LoggerManager.logger.progname
    end

    test 'configures default logger with level set to WARN' do
      assert_equal Logger::Severity::WARN, Asciidoctor::LoggerManager.logger.level
    end

    test 'configures default logger to write messages to $stderr' do
      out_string, err_string = redirect_streams do |out, err|
        Asciidoctor::LoggerManager.logger.warn 'this is a call'
        [out.string, err.string]
      end
      assert_empty out_string
      refute_empty err_string
      assert_includes err_string, 'this is a call'
    end

    test 'should set logdev to specified file' do
      log_file = Tempfile.new %w(asciidoctor- .log)
      logger = Asciidoctor::Logger.new log_file
      logger.warn 'this is a call'
      logger.close
      log_file_contents = File.read log_file
      assert_equal 'asciidoctor: WARNING: this is a call', log_file_contents.lines.pop.chomp
    end

    test 'should set logdev to specified file with additional options' do
      log_file = Tempfile.new %w(asciidoctor- .log)
      logger = Asciidoctor::Logger.new log_file, formatter: nil, level: Asciidoctor::Logger::DEBUG
      logger.debug 'this is a sign of life'
      logger.close
      log_file_contents = File.read log_file
      assert_includes log_file_contents.lines.pop.chomp, 'DEBUG -- asciidoctor: this is a sign of life'
    end

    test 'configures default logger to use a formatter that matches traditional format' do
      err_string = redirect_streams do |_, err|
        Asciidoctor::LoggerManager.logger.warn 'this is a call'
        Asciidoctor::LoggerManager.logger.fatal 'it cannot be done'
        err.string
      end
      assert_includes err_string, %(asciidoctor: WARNING: this is a call)
      assert_includes err_string, %(asciidoctor: FAILED: it cannot be done)
    end

    test 'NullLogger level is not nil' do
      logger = Asciidoctor::NullLogger.new
      refute_nil logger.level
      assert_equal Logger::WARN, logger.level
    end
  end

"#
);

// `context ':logger API option'` — passing a `:logger` (or falsy value, for a
// `NullLogger`) to the `load` / `load_file` / `convert` / `convert_file` entry
// points. Those document-level APIs, and the option, are downstream of this
// parser crate.
non_normative!(
    r#"
  context ':logger API option' do
    test 'should be able to set logger when invoking load API' do
      old_logger = Asciidoctor::LoggerManager.logger
      new_logger = MyLogger.new $stdout
      begin
        Asciidoctor.load 'contents', logger: new_logger
        assert_same new_logger, Asciidoctor::LoggerManager.logger
      ensure
        Asciidoctor::LoggerManager.logger = old_logger
      end
    end

    test 'should be able to set logger when invoking load_file API' do
      old_logger = Asciidoctor::LoggerManager.logger
      new_logger = MyLogger.new $stdout
      begin
        Asciidoctor.load_file fixture_path('basic.adoc'), logger: new_logger
        assert_same new_logger, Asciidoctor::LoggerManager.logger
      ensure
        Asciidoctor::LoggerManager.logger = old_logger
      end
    end

    test 'should be able to set logger when invoking convert API' do
      old_logger = Asciidoctor::LoggerManager.logger
      new_logger = MyLogger.new $stdout
      begin
        Asciidoctor.convert 'contents', logger: new_logger
        assert_same new_logger, Asciidoctor::LoggerManager.logger
      ensure
        Asciidoctor::LoggerManager.logger = old_logger
      end
    end

    test 'should be able to set logger when invoking convert_file API' do
      old_logger = Asciidoctor::LoggerManager.logger
      new_logger = MyLogger.new $stdout
      begin
        Asciidoctor.convert_file fixture_path('basic.adoc'), to_file: false, logger: new_logger
        assert_same new_logger, Asciidoctor::LoggerManager.logger
      ensure
        Asciidoctor::LoggerManager.logger = old_logger
      end
    end

    test 'should be able to set logger to NullLogger by setting :logger option to a falsy value' do
      [nil, false].each do |falsy_val|
        old_logger = Asciidoctor::LoggerManager.logger
        begin
          Asciidoctor.load 'contents', logger: falsy_val
          assert_kind_of Asciidoctor::NullLogger, Asciidoctor::LoggerManager.logger
        ensure
          Asciidoctor::LoggerManager.logger = old_logger
        end
      end
    end
  end

"#
);

// `context 'Logging'` — the `Asciidoctor::Logging` mixin, giving modules and
// classes a `logger` accessor plus `message_with_context` (whose result is
// formatted through `Reader::Cursor#inspect`). The mixin and its
// message-formatting helper are Ruby-internal and not part of this crate. The
// final test in this context (duplicate block id) is the sole normative case
// and is ported separately below.
non_normative!(
    r#"
  context 'Logging' do
    test 'including Logging gives instance methods on module access to logging infrastructure' do
      module SampleModuleA
        include Asciidoctor::Logging
        def get_logger
          logger
        end
      end

      class SampleClassA
        include SampleModuleA
      end
      assert_same Asciidoctor::LoggerManager.logger, SampleClassA.new.get_logger
      assert SampleClassA.public_method_defined? :logger
    end

    test 'including Logging gives static methods on module access to logging infrastructure' do
      module SampleModuleB
        include Asciidoctor::Logging
        def self.get_logger
          logger
        end
      end

      assert_same Asciidoctor::LoggerManager.logger, SampleModuleB.get_logger
    end

    test 'including Logging gives instance methods on class access to logging infrastructure' do
      class SampleClassC
        include Asciidoctor::Logging
        def get_logger
          logger
        end
      end

      assert_same Asciidoctor::LoggerManager.logger, SampleClassC.new.get_logger
      assert SampleClassC.public_method_defined? :logger
    end

    test 'including Logging gives static methods on class access to logging infrastructure' do
      class SampleClassD
        include Asciidoctor::Logging
        def self.get_logger
          logger
        end
      end

      assert_same Asciidoctor::LoggerManager.logger, SampleClassD.get_logger
    end

    test 'can create an auto-formatting message with context' do
      class SampleClassE
        include Asciidoctor::Logging
        def create_message cursor
          message_with_context 'Asciidoctor was here', source_location: cursor
        end
      end

      cursor = Asciidoctor::Reader::Cursor.new 'file.adoc', fixturedir, 'file.adoc', 5
      message = SampleClassE.new.create_message cursor
      assert_equal 'Asciidoctor was here', message[:text]
      assert_same cursor, message[:source_location]
      assert_equal 'file.adoc: line 5: Asciidoctor was here', message.inspect
    end

"#
);

// The final test drives a document with two blocks that claim the same
// explicit id (`[#first]`). Asciidoctor logs
// `<stdin>: line 5: id assigned to block already in use: first`; this crate
// surfaces the identical condition as a structured `DuplicateId` warning.
#[test]
fn writes_message_prefixed_with_program_name_and_source_location_to_stderr() {
    verifies!(
        r#"
    test 'writes message prefixed with program name and source location to stderr' do
      input = <<~'EOS'
      [#first]
      first paragraph

      [#first]
      another first paragraph
      EOS
      messages = redirect_streams do |_, err|
        convert_string_to_embedded input
        err.string.chomp
      end
      assert_equal 'asciidoctor: WARNING: <stdin>: line 5: id assigned to block already in use: first', messages
    end
"#
    );

    let doc =
        Parser::default().parse("[#first]\nfirst paragraph\n\n[#first]\nanother first paragraph\n");

    // Asciidoctor reports the duplicate id through its logger as
    // `<stdin>: line 5: id assigned to block already in use: first`. This
    // crate has no logger; it records the same condition as a document
    // warning of type `DuplicateId`.
    //
    // The line differs by design: Asciidoctor anchors the message at the
    // block's first content line (line 5), whereas this crate anchors the
    // warning at the block's metadata/anchor line (`[#first]`, line 4). We
    // still assert that line so an *unrelated* span regression is caught —
    // the divergence from Asciidoctor's reported line is the documented `4`
    // vs `5`, not a licence to attach the warning to any span.
    let warnings: Vec<_> = doc.warnings().collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0].warning,
        WarningType::DuplicateId("first".to_string())
    );

    // The second `[#first]` anchor is on line 4 (Asciidoctor reports line 5;
    // see above).
    assert_eq!(warnings[0].source.line(), 4);
}

// Closing `end`s for `context 'Logging'` and the outer `context 'Logger'`.
non_normative!(
    r#"
  end
end
"#
);
