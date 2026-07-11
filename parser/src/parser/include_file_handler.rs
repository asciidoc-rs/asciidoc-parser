use std::fmt::Debug;

use crate::{Parser, attributes::Attrlist};

/// An `IncludeFileHandler` is responsible for providing the text content for an
/// `include::` directive when encountered.
///
/// A client of [`Parser`] may provide an `IncludeFileHandler` to customize how
/// include file resolution is handled.
///
/// [`Parser`]: crate::Parser
pub trait IncludeFileHandler: Debug {
    /// Provide the file content for an `include::` directive, if available.
    ///
    /// # Parameters
    /// - `source`: The path to the document that is including the file. A root
    ///   document may be signaled via `None` depending on how the parser was
    ///   invoked. This path should be considered when resolving relative paths.
    /// - `target`: The path to the document that was provided in the
    ///   `include::` directive.
    /// - `attrlist`: Any attributes specified on the include directive.
    /// - `parser`: An implementation may read document attribute values from
    ///   the [`Parser`] state.
    ///
    /// Return the content of the include file (wrapped in [`IncludeContent`])
    /// if found. If no file is found, return `None` and an appropriate warning
    /// message will be generated.
    ///
    /// # Options
    /// With the exception of `encoding` (see below), the implementation should
    /// not attempt to interpret any of the built-in attributes (i.e.
    /// `leveloffset`, `lines`, `tags`, or `indent`). Correct handling of these
    /// attributes will be provided by the parser itself.
    ///
    /// # Encoding
    /// The content returned in [`IncludeContent`] is a typical Rust [`String`]
    /// and therefore must be encoded as UTF-8.
    ///
    /// If the implementation is capable of transcoding from other formats, it
    /// may use the `encoding` attribute as a hint of the source format. When it
    /// transcodes the content to UTF-8, it should return the result via
    /// [`IncludeContent::transcoded`] so that the parser knows the requested
    /// encoding was honored and suppresses the non-UTF-8 include-encoding
    /// warning.
    ///
    /// An implementation that only deals in UTF-8 should return its content via
    /// [`IncludeContent::new`] (or the [`From`] conversions). If the directive
    /// requested a non-UTF-8 `encoding`, the parser will emit a non-UTF-8
    /// include-encoding warning in that case.
    ///
    /// If the implementation finds a file that is not encoded in UTF-8 and is
    /// incapable of transcoding it, it should return `None`.
    fn resolve_target<'src>(
        &self,
        source: Option<&str>,
        target: &str,
        attrlist: &Attrlist<'src>,
        parser: &Parser,
    ) -> Option<IncludeContent>;
}

/// The content returned by an [`IncludeFileHandler`] for an `include::`
/// directive, together with the metadata the parser needs to finish processing
/// the include.
///
/// Construct one via [`IncludeContent::new`] for UTF-8 content that was read
/// without interpreting the `encoding` attribute, or via
/// [`IncludeContent::transcoded`] for content the handler transcoded to UTF-8
/// while honoring a requested `encoding`. See the `# Encoding` section of
/// [`IncludeFileHandler::resolve_target`] for details.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IncludeContent {
    content: String,
    encoding_handled: bool,
}

impl IncludeContent {
    /// UTF-8 content that was read **without** interpreting the `encoding`
    /// attribute.
    ///
    /// If the `include::` directive requested a non-UTF-8 `encoding`, the
    /// parser will emit a non-UTF-8 include-encoding warning. This is the
    /// appropriate constructor for a handler that only deals in UTF-8.
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            encoding_handled: false,
        }
    }

    /// Content that the handler transcoded to UTF-8 while honoring the
    /// requested `encoding` attribute.
    ///
    /// The parser will **not** emit a non-UTF-8 include-encoding warning for
    /// content returned this way, since the handler has already reencoded it.
    pub fn transcoded(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            encoding_handled: true,
        }
    }

    /// Returns the UTF-8 content of the include.
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Returns `true` if the handler honored the requested `encoding` attribute
    /// (i.e. the content was created via [`IncludeContent::transcoded`]).
    pub fn encoding_handled(&self) -> bool {
        self.encoding_handled
    }

    /// Consumes the `IncludeContent`, returning the owned UTF-8 content.
    pub fn into_content(self) -> String {
        self.content
    }
}

impl From<String> for IncludeContent {
    fn from(content: String) -> Self {
        Self::new(content)
    }
}

impl From<&str> for IncludeContent {
    fn from(content: &str) -> Self {
        Self::new(content)
    }
}

#[cfg(test)]
mod tests {
    use super::IncludeContent;

    #[test]
    fn new_does_not_mark_encoding_handled() {
        let content = IncludeContent::new("Content.");
        assert_eq!(content.content(), "Content.");
        assert!(!content.encoding_handled());
        assert_eq!(content.into_content(), "Content.".to_owned());
    }

    #[test]
    fn transcoded_marks_encoding_handled() {
        let content = IncludeContent::transcoded("Résumé.");
        assert_eq!(content.content(), "Résumé.");
        assert!(content.encoding_handled());
        assert_eq!(content.into_content(), "Résumé.".to_owned());
    }

    #[test]
    fn from_string_and_str_do_not_mark_encoding_handled() {
        let from_string = IncludeContent::from("Content.".to_owned());
        assert_eq!(from_string, IncludeContent::new("Content."));
        assert!(!from_string.encoding_handled());

        let from_str: IncludeContent = "Content.".into();
        assert_eq!(from_str, IncludeContent::new("Content."));
        assert!(!from_str.encoding_handled());
    }
}
