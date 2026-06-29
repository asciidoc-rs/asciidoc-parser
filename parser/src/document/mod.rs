//! Describes the top-level document structure.

mod attribute;
pub use attribute::{Attribute, InterpretedValue};

mod author;
pub use author::Author;

mod author_line;
pub use author_line::AuthorLine;

mod catalog;
#[allow(unused)] // TEMPORARY while building
pub(crate) use catalog::DuplicateIdError;
pub use catalog::{Catalog, RefEntry, RefType};

mod document;
pub use document::Document;

mod header;
pub use header::Header;

mod revision_line;
pub use revision_line::RevisionLine;

mod toc;
pub use toc::TocMode;
