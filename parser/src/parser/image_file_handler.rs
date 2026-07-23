use std::fmt::Debug;

use crate::Parser;

/// An `ImageFileHandler` is responsible for providing the raw bytes of an image
/// file when a referenced image must be embedded directly in the output as a
/// `data:` URI (i.e. when the `data-uri` document attribute is set and the safe
/// mode is below [`SafeMode::Secure`]).
///
/// This crate is a parser, not a converter, and never reads from the filesystem
/// itself. A client of [`Parser`] that wants images embedded as `data:` URIs
/// must provide an `ImageFileHandler` (analogous to [`SvgFileHandler`],
/// [`IncludeFileHandler`], and [`DocinfoFileHandler`]) that maps a resolved
/// image path to its bytes. If no handler is provided (or the handler cannot
/// find the file), the image degrades to an ordinary web path – the same output
/// as when `data-uri` is not set – matching this crate's convention that a
/// missing I/O handler is a silent, graceful degradation.
///
/// [`Parser`]: crate::Parser
/// [`SafeMode::Secure`]: crate::SafeMode::Secure
/// [`SvgFileHandler`]: crate::parser::SvgFileHandler
/// [`IncludeFileHandler`]: crate::parser::IncludeFileHandler
/// [`DocinfoFileHandler`]: crate::parser::DocinfoFileHandler
pub trait ImageFileHandler: Debug {
    /// Provide the raw bytes of an image file, if available.
    ///
    /// # Parameters
    /// - `target`: The resolved path to the image file, already prefixed with
    ///   the value of the relevant asset-directory attribute (`imagesdir` or
    ///   `iconsdir`, as appropriate). This is the same value that would appear
    ///   in the `src` attribute of the image were it *not* embedded.
    /// - `parser`: An implementation may read document attribute values from
    ///   the [`Parser`] state.
    ///
    /// Return the bytes of the image file if found. If no file is found (or it
    /// is not readable), return `None`; the image will then fall back to
    /// rendering an ordinary web path.
    ///
    /// [`Parser`]: crate::Parser
    fn resolve_image(&self, target: &str, parser: &Parser) -> Option<Vec<u8>>;
}
