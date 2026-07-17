//! Behavioral tests ported 1:1 from Ruby Asciidoctor's `test/*_test.rb` suites.
//!
//! Porting conventions (see the existing modules for worked examples):
//!
//! * One Rust module per Ruby file (`<suite>_test.rs`); nested Ruby `context`
//!   blocks become nested `mod`s, and each Ruby `test 'name'` becomes a
//!   `#[test] fn name()`.
//! * Ruby `assert_xpath` / `assert_css` / `assert_output_contains` /
//!   `refute_output_contains` map to the same-named helpers in
//!   [`crate::tests::assert_dom`], run against `doc.to_virtual_dom()`. Use a
//!   full-AST `assert_eq!(doc, Document { .. })` where the Ruby test inspects
//!   parser state (warnings, source spans, catalog) rather than HTML.
//! * DocBook and other non-HTML backends are out of scope: such tests are
//!   omitted with a `// Backend-specific test omitted: DocBook.` note. Compat
//!   mode is likewise disregarded.
//! * Tests blocked on an unimplemented (but planned) feature are kept as
//!   `#[ignore]`d stubs whose body preserves the Ruby assertions in
//!   `todo!("..")` calls, tagged with a tracking-issue `TODO`. Features that
//!   are explicitly out of scope are omitted with a one-line `// NOTE:`.
//! * Expected values track the crate's *actual* output, not the Ruby HTML
//!   string, when the two differ.

// The tests in this tree are adapted from the Ruby implementation of
// Asciidoctor, which comes with the following license:

// MIT License

// Copyright (C) 2012-present Dan Allen, Sarah White, Ryan Waldron, and the
// individual contributors to Asciidoctor.

// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:

// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

mod attribute_list_test;
mod converter_test;
mod lists_test;
mod options_test;
mod paragraphs_test;
mod substitutions_test;
mod tables_test;
