use std::{fmt::Debug, rc::Rc};

use crate::parser::RenderContext;

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
    /// - `context`: The document state as of the point in the document this
    ///   element came from. An implementation may read document attribute
    ///   values from it. See [`RenderContext`].
    ///
    /// Return the string content of the SVG file if found. If no file is found
    /// (or it is not readable), return `None`; the inline image will then fall
    /// back to rendering its alt text.
    ///
    /// # Encoding
    /// If a `Some` result is provided, it is a typical Rust [`String`] and
    /// therefore must be encoded as UTF-8.
    fn resolve_svg(&self, target: &str, context: &RenderContext) -> Option<String>;
}

/// An `Rc<T>` wrapping any `SvgFileHandler` (including an unsized `Rc<dyn
/// SvgFileHandler>`) is itself an `SvgFileHandler`, delegating to the wrapped
/// handler.
///
/// See the analogous impl on
/// [`IncludeFileHandler`](crate::parser::IncludeFileHandler) for why this is
/// useful: it lets a handler already held behind an `Rc` be passed straight to
/// [`Parser::with_svg_file_handler`], whose `Sized` bound a trait object
/// cannot otherwise satisfy.
///
/// [`Parser::with_svg_file_handler`]: crate::Parser::with_svg_file_handler
impl<T: SvgFileHandler + ?Sized> SvgFileHandler for Rc<T> {
    fn resolve_svg(&self, target: &str, context: &RenderContext) -> Option<String> {
        (**self).resolve_svg(target, context)
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::SvgFileHandler;
    use crate::{Parser, SafeMode, parser::RenderContext, tests::prelude::rendered_paragraphs};

    const SAMPLE_SVG: &str = concat!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 500 500\">",
        "<circle cx=\"250\" cy=\"250\" r=\"200\"/></svg>",
    );

    // The blanket `impl<T: SvgFileHandler + ?Sized> SvgFileHandler for Rc<T>`
    // above lets `Rc<dyn SvgFileHandler>` be handed straight to
    // `Parser::with_svg_file_handler` -- no delegating newtype required.
    #[test]
    fn rc_dyn_svg_file_handler_resolves_through_the_parser() {
        #[derive(Debug)]
        struct Fixed;

        impl SvgFileHandler for Fixed {
            fn resolve_svg(&self, target: &str, _context: &RenderContext) -> Option<String> {
                (target == "sample.svg").then(|| SAMPLE_SVG.to_owned())
            }
        }

        let handler: Rc<dyn SvgFileHandler> = Rc::new(Fixed);

        let doc = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_svg_file_handler(handler)
            .parse("image:sample.svg[opts=inline]");

        let paragraphs = rendered_paragraphs(&doc);
        assert!(
            paragraphs
                .first()
                .is_some_and(|p| p.contains("<circle cx=\"250\" cy=\"250\" r=\"200\"/>")),
            "{paragraphs:?}"
        );
    }
}
