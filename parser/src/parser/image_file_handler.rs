use std::{fmt::Debug, rc::Rc};

use crate::parser::RenderContext;

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
/// find the file), the image degrades to an ordinary web path — the same output
/// as when `data-uri` is not set — matching this crate's convention that a
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
    /// - `context`: The document state as of the point in the document this
    ///   element came from. An implementation may read document attribute
    ///   values from it. See [`RenderContext`].
    ///
    /// Return the bytes of the image file if found. If no file is found (or it
    /// is not readable), return `None`; the image will then fall back to
    /// rendering an ordinary web path.
    fn resolve_image(&self, target: &str, context: &RenderContext) -> Option<Vec<u8>>;
}

/// An `Rc<T>` wrapping any `ImageFileHandler` (including an unsized `Rc<dyn
/// ImageFileHandler>`) is itself an `ImageFileHandler`, delegating to the
/// wrapped handler.
///
/// See the analogous impl on
/// [`IncludeFileHandler`](crate::parser::IncludeFileHandler) for why this is
/// useful: it lets a handler already held behind an `Rc` be passed straight to
/// [`Parser::with_image_file_handler`], whose `Sized` bound a trait object
/// cannot otherwise satisfy.
///
/// [`Parser::with_image_file_handler`]: crate::Parser::with_image_file_handler
impl<T: ImageFileHandler + ?Sized> ImageFileHandler for Rc<T> {
    fn resolve_image(&self, target: &str, context: &RenderContext) -> Option<Vec<u8>> {
        (**self).resolve_image(target, context)
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::ImageFileHandler;
    use crate::{
        Parser, SafeMode,
        parser::{ModificationContext, RenderContext},
        tests::prelude::rendered_paragraphs,
    };

    // The blanket `impl<T: ImageFileHandler + ?Sized> ImageFileHandler for
    // Rc<T>` above lets `Rc<dyn ImageFileHandler>` be handed straight to
    // `Parser::with_image_file_handler` -- no delegating newtype required.
    #[test]
    fn rc_dyn_image_file_handler_resolves_through_the_parser() {
        #[derive(Debug)]
        struct Fixed;

        impl ImageFileHandler for Fixed {
            fn resolve_image(&self, target: &str, _context: &RenderContext) -> Option<Vec<u8>> {
                (target == "circle.png").then(|| b"fake-bytes".to_vec())
            }
        }

        let handler: Rc<dyn ImageFileHandler> = Rc::new(Fixed);

        // Below `Secure`, with `data-uri` set, the default HTML renderer
        // embeds the image bytes -- read through the registered
        // `ImageFileHandler` -- as a `data:` URI.
        let doc = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_intrinsic_attribute_bool("data-uri", true, ModificationContext::Anywhere)
            .with_image_file_handler(handler)
            .parse("image:circle.png[Tiger]");

        let paragraphs = rendered_paragraphs(&doc);
        assert!(
            paragraphs
                .first()
                .is_some_and(|p| p.contains("data:image/png;base64,ZmFrZS1ieXRlcw==")),
            "{paragraphs:?}"
        );
    }
}
