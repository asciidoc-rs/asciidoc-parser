# Asciidoctor reference test suite (in progress)

This directory vendors test files from
[Asciidoctor](https://github.com/asciidoctor/asciidoctor), the reference
Ruby implementation of the AsciiDoc language, so that `asciidoc-parser` can be
validated against Asciidoctor's own behavior using this repository's
spec-driven-development coverage tooling (see `sdd/`).

We're porting the Asciidoctor test suite **one file at a time**. Each file is
vendored here verbatim and covered line-by-line by a matching Rust test module
under `parser/src/tests/asciidoctor_rb/`, so the `sdd` tool can report exactly
which lines of the Ruby suite are verified. Files are added as they are ported,
building toward covering the suite in full.

`test/attribute_list_test.rb` is where this started. (Some Asciidoctor tests
were hand-ported into `parser/src/tests/asciidoctor_rb/` before this
line-by-line approach; those will be migrated to it over time.)

Asciidoctor is distributed under the MIT License; see [`LICENSE`](LICENSE).
The vendored files are unmodified copies of their upstream originals.
