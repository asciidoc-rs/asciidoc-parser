#![allow(clippy::module_inception)]
#![deny(clippy::expect_used)]
#![deny(clippy::indexing_slicing)]
#![deny(clippy::panic)]
#![deny(clippy::unwrap_used)]
#![deny(missing_docs)]
#![deny(warnings)]
#![doc = include_str!(concat!("../", std::env!("CARGO_PKG_README")))]

pub mod attributes;
pub mod blocks;
pub mod content;

pub mod document;
pub use document::Document;

pub(crate) mod internal;

pub mod parser;
pub use parser::Parser;

mod span;
pub use span::{HasSpan, Span};

pub mod strings;

#[cfg(test)]
mod tests;

mod warnings;
