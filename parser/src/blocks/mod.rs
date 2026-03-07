//! **Block elements** form the main structure of an AsciiDoc document, starting
//! with the document itself.
//!
//! A block element (aka **block**) is a discrete, line-oriented chunk of
//! content in an AsciiDoc document. Once parsed, that chunk of content becomes
//! a block element in the parsed document model. Certain blocks may contain
//! other blocks, so we say that blocks can be nested. The converter visits each
//! block in turn, in document order, converting it to a corresponding chunk of
//! output.
//!
//! ## Design discussion
//!
//! There are two data structures that express blocks in the asciidoc-parser
//! crate:
//!
//! * [`Block`] is an enum that contains the common built-in block types
//!   understood by this parser
//! * [`IsBlock`] is a trait that describes the common properties of a block
//!   data structure that might be provided outside of this crate
//!
//! This duality exists because we sought to avoid the overhead of `Box<dyn
//! Block>` throughout the codebase, but also needed to provide for
//! externally-described block types.

mod block;
pub use block::Block;

mod r#break;
pub use r#break::{Break, BreakType};

mod compound_delimited;
pub use compound_delimited::CompoundDelimitedBlock;

mod context;
pub(crate) use context::is_built_in_context;

mod is_block;
pub use is_block::{ContentModel, IsBlock};

mod list;
pub use list::{ListBlock, ListType};

mod list_item;
pub use list_item::ListItem;

mod list_item_marker;
pub use list_item_marker::ListItemMarker;

mod media;
pub use media::{MediaBlock, MediaType};

pub(crate) mod metadata;
pub(crate) mod parse_utils;

mod preamble;
pub use preamble::Preamble;

mod raw_delimited;
pub use raw_delimited::RawDelimitedBlock;

mod section;
pub use section::{SectionBlock, SectionNumber, SectionType};

mod simple;
pub use simple::{SimpleBlock, SimpleBlockStyle};

#[cfg(test)]
mod tests;
