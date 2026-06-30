use std::fmt::Debug;

use crate::Parser;

/// A `DocinfoFileHandler` is responsible for providing the text content of a
/// [docinfo file] when one is requested while resolving a document's docinfo.
///
/// This crate is a parser, not a converter, and never reads from the
/// filesystem itself. A client of [`Parser`] that wants docinfo files to be
/// applied must provide a `DocinfoFileHandler` (analogous to
/// [`IncludeFileHandler`]) that maps a computed docinfo file name to its
/// content. If no handler is provided, no docinfo content is resolved.
///
/// [docinfo file]: https://docs.asciidoctor.org/asciidoc/latest/docinfo/
/// [`Parser`]: crate::Parser
/// [`IncludeFileHandler`]: crate::parser::IncludeFileHandler
pub trait DocinfoFileHandler: Debug {
    /// Provide the content of a docinfo file, if available.
    ///
    /// # Parameters
    /// - `docinfodir`: The value of the `docinfodir` attribute, if set. When
    ///   `Some`, docinfo files should be resolved relative to this directory
    ///   only (a relative value is appended to the document directory; an
    ///   absolute value is used as-is). When `None`, the implementation should
    ///   resolve relative to the document directory.
    /// - `file_name`: The computed docinfo file name to resolve, for example
    ///   `docinfo-header.html` (shared) or `mydoc-docinfo.html` (private). The
    ///   parser determines this name from the docinfo scope, location, the
    ///   document name, and the `outfilesuffix` attribute.
    /// - `parser`: An implementation may read document attribute values from
    ///   the [`Parser`] state.
    ///
    /// Return the string content of the docinfo file if found. If no file is
    /// found (or it is not readable), return `None`; the requested location's
    /// content simply omits this file.
    ///
    /// # Encoding
    /// If a `Some` result is provided, it is a typical Rust [`String`] and
    /// therefore must be encoded as UTF-8.
    ///
    /// [`Parser`]: crate::Parser
    fn resolve_docinfo(
        &self,
        docinfodir: Option<&str>,
        file_name: &str,
        parser: &Parser,
    ) -> Option<String>;
}
