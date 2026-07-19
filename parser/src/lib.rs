#![allow(clippy::module_inception)]
#![deny(clippy::expect_used)]
#![deny(clippy::indexing_slicing)]
#![deny(clippy::panic)]
#![deny(clippy::unwrap_used)]
#![deny(missing_docs)]
#![deny(warnings)]
#![doc = include_str!(concat!("../", std::env!("CARGO_PKG_README")))]

/// The version of (Ruby) Asciidoctor whose behavior this crate implements.
///
/// This is the value of the predefined `asciidoctor-version` attribute, and it
/// is also the release that the reference test suite vendored in
/// `ref/asciidoctor` is taken from. Update it here – and re-vendor
/// `ref/asciidoctor` – when the crate starts tracking a newer Asciidoctor
/// release.
pub(crate) const ASCIIDOCTOR_VERSION: &str = "2.0.26";

pub mod attributes;
pub mod blocks;
pub mod content;

pub mod document;
pub use document::Document;

pub(crate) mod internal;

pub mod parser;
pub use parser::{Parser, SafeMode};

mod span;
pub use span::{HasSpan, Span};

pub mod strings;

#[cfg(test)]
mod tests;

pub mod warnings;

// Use `pretty_assertion_sorted`'s version of `assert_eq` across the board.
#[cfg(test)]
#[macro_use]
extern crate pretty_assertions_sorted;
