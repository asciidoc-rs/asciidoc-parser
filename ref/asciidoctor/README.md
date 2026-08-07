# Asciidoctor reference material (in progress)

This directory vendors material from
[Asciidoctor](https://github.com/asciidoctor/asciidoctor), the reference
Ruby implementation of the AsciiDoc language, so that `asciidoc-parser` can be
validated against Asciidoctor's own behavior and documentation using this
repository's spec-driven-development coverage tooling (see `sdd/`).

Two kinds of material are vendored:

- **`test/`** — the Asciidoctor test suite.
- **`docs/`** — the Asciidoctor processor documentation (Antora modules).

## Test suite (`test/`)

We're porting the Asciidoctor test suite **one file at a time**. Each file is
vendored here verbatim and covered line-by-line by a matching Rust test module
under `parser/src/tests/asciidoctor_rb/`, so the `sdd` tool can report exactly
which lines of the Ruby suite are verified. Files are added as they are ported,
building toward covering the suite in full.

`test/attribute_list_test.rb` is where this started. (Some Asciidoctor tests
were hand-ported into `parser/src/tests/asciidoctor_rb/` before this
line-by-line approach; those will be migrated to it over time.)

## Processor documentation (`docs/`)

We're porting the Asciidoctor processor documentation **one module at a time**,
mirrored line-by-line by Rust test modules under
`parser/src/tests/asciidoctor_docs/`. Because these pages largely describe
rendering behavior that `asciidoc-parser` does not implement (it parses AsciiDoc
but performs no output rendering), most of the ported prose is marked
non-normative.

`docs/modules/syntax-highlighting` is the first module ported.

## Provenance

The vendored files are unmodified copies of their upstream originals, taken from
Asciidoctor **v2.0.26**.

Asciidoctor is distributed under the MIT License; see [`LICENSE`](LICENSE).
