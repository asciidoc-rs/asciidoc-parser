//! Describes the top-level document structure.

mod attribute;
pub use attribute::{Attribute, InterpretedValue};

mod author;
pub use author::Author;
pub(crate) use author::matches_author_pattern;

mod author_line;
pub use author_line::{AuthorLine, Authors};

mod catalog;
#[allow(unused)] // TODO: Remove once `DuplicateIdError` is consumed (#949).
pub(crate) use catalog::DuplicateIdError;
pub use catalog::{Catalog, Footnote, ImageReference, RefEntry, RefType};

mod docinfo;
pub(crate) use docinfo::Docinfo;
pub use docinfo::DocinfoLocation;

mod document;
#[cfg(test)]
pub(crate) use document::first_inline_candidate;
pub use document::{Document, Warnings};

mod header;
pub use header::{Comments, Header, HeaderAttributes};

mod revision_line;
pub use revision_line::RevisionLine;

mod title_refs;

mod toc;
pub(crate) use toc::TocConfig;
pub use toc::TocMode;
