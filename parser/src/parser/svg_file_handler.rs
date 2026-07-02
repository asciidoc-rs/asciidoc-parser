use std::fmt::Debug;

use crate::Parser;

/// An `SvgFileHandler` is responsible for providing the raw contents of an SVG
/// file when an inline image macro requests that the SVG be embedded directly
/// in the output (`image:target.svg[opts=inline]`).
///
/// This crate is a parser, not a converter, and never reads from the
/// filesystem itself. A client of [`Parser`] that wants inline SVG images to be
/// embedded must provide an `SvgFileHandler` (analogous to
/// [`IncludeFileHandler`] and [`DocinfoFileHandler`]) that maps a resolved
/// image path to its content. If no handler is provided (or the handler cannot
/// find the file), the inline SVG image degrades to a `<span class="alt">`
/// element containing the alt text, matching Ruby Asciidoctor's behavior when
/// the SVG contents can't be read.
///
/// [`Parser`]: crate::Parser
/// [`IncludeFileHandler`]: crate::parser::IncludeFileHandler
/// [`DocinfoFileHandler`]: crate::parser::DocinfoFileHandler
pub trait SvgFileHandler: Debug {
    /// Provide the raw contents of an SVG file, if available.
    ///
    /// # Parameters
    /// - `target`: The resolved path to the SVG file, already prefixed with the
    ///   value of the `imagesdir` attribute (if any). This is the same value
    ///   that would appear in the `src` attribute of a non-inline image.
    /// - `parser`: An implementation may read document attribute values from
    ///   the [`Parser`] state.
    ///
    /// Return the string content of the SVG file if found. If no file is found
    /// (or it is not readable), return `None`; the inline image will then fall
    /// back to rendering its alt text.
    ///
    /// # Encoding
    /// If a `Some` result is provided, it is a typical Rust [`String`] and
    /// therefore must be encoded as UTF-8.
    ///
    /// [`Parser`]: crate::Parser
    fn resolve_svg(&self, target: &str, parser: &Parser) -> Option<String>;
}
