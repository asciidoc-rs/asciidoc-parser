//! This module quotes from the Asciidoctor *processor* documentation with the
//! intent of verifying, line-by-line, compliance with that documentation.
//!
//! Unlike the AsciiDoc language documentation ported under
//! [`super::asciidoc_lang`], these pages describe behavior specific to the
//! Asciidoctor processor (e.g. how it renders a source block through a syntax
//! highlighter). This crate implements the parsing half of AsciiDoc and
//! performs no rendering, so much of this material is non-normative here; it is
//! reproduced so the `sdd` coverage tool can account for every line.
//!
//! The quoted documentation can be found in rendered form here:
//! https://docs.asciidoctor.org/asciidoctor/latest/
//!
//! and in source form here:
//! https://github.com/asciidoctor/asciidoctor/tree/v2.0.26/docs/modules
//!
//! The vendored copies under `ref/asciidoctor/docs` are unmodified and taken
//! from Asciidoctor v2.0.26, matching the reference test suite vendored under
//! `ref/asciidoctor/test` (see `ref/asciidoctor/README.md`).
//!
//! Asciidoctor is distributed under the MIT License; see
//! `ref/asciidoctor/LICENSE`.

mod syntax_highlighting;
