use std::{fmt::Debug, rc::Rc};

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

/// An `Rc<T>` wrapping any `DocinfoFileHandler` (including an unsized
/// `Rc<dyn DocinfoFileHandler>`) is itself a `DocinfoFileHandler`, delegating
/// to the wrapped handler.
///
/// See the analogous impl on
/// [`IncludeFileHandler`](crate::parser::IncludeFileHandler) for why this is
/// useful: it lets a handler already held behind an `Rc` be passed straight to
/// [`Parser::with_docinfo_file_handler`], whose `Sized` bound a trait object
/// cannot otherwise satisfy.
///
/// [`Parser::with_docinfo_file_handler`]: crate::Parser::with_docinfo_file_handler
impl<T: DocinfoFileHandler + ?Sized> DocinfoFileHandler for Rc<T> {
    fn resolve_docinfo(
        &self,
        docinfodir: Option<&str>,
        file_name: &str,
        parser: &Parser,
    ) -> Option<String> {
        (**self).resolve_docinfo(docinfodir, file_name, parser)
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::DocinfoFileHandler;
    use crate::{Parser, SafeMode, document::DocinfoLocation};

    // The blanket `impl<T: DocinfoFileHandler + ?Sized> DocinfoFileHandler for
    // Rc<T>` above lets `Rc<dyn DocinfoFileHandler>` be handed straight to
    // `Parser::with_docinfo_file_handler` -- no delegating newtype required.
    #[test]
    fn rc_dyn_docinfo_file_handler_resolves_through_the_parser() {
        #[derive(Debug)]
        struct Fixed;

        impl DocinfoFileHandler for Fixed {
            fn resolve_docinfo(
                &self,
                _docinfodir: Option<&str>,
                file_name: &str,
                _parser: &Parser,
            ) -> Option<String> {
                (file_name == "docinfo.html").then(|| "<meta name=\"via-rc\">".to_owned())
            }
        }

        let handler: Rc<dyn DocinfoFileHandler> = Rc::new(Fixed);

        // Docinfo is resolved below `Secure`; `Server` also exercises the
        // safe-mode lock's own docinfo handling.
        let head = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_primary_file_name("mydoc.adoc")
            .with_docinfo_file_handler(handler)
            .parse("= Doc\n:docinfo: shared-head\n\nBody.")
            .docinfo(DocinfoLocation::Head)
            .to_string();

        assert_eq!(head, "<meta name=\"via-rc\">");
    }
}
