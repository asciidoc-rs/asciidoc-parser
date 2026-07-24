//! Tests for the public [`InlineSubstitutionRenderer`] extension surface: a
//! downstream renderer that overrides only the substitutions it cares about and
//! inherits the built-in HTML behavior (including `data-uri` embedding) for the
//! rest, plus the [`Parser`] accessors that expose the registered file
//! handlers.
//!
//! [`InlineSubstitutionRenderer`]: crate::parser::InlineSubstitutionRenderer

use crate::{
    parser::{ImageRenderParams, InlineSubstitutionRenderer, SpecialCharacter},
    tests::{
        fixtures::{
            image_file_handler::ImageFileHandlerFixture, svg_file_handler::SvgFileHandlerFixture,
        },
        prelude::*,
    },
};

/// The bytes the image file handler returns for `circle.svg`, and the strict
/// base64 of those bytes (the `data:` URI payload). Shared with the ported
/// Asciidoctor substitution tests.
const CIRCLE_SVG: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"500\" height=\"500\" ",
    "style=\"fill:red\" viewBox=\"0 0 500 500\">",
    "<circle cx=\"250\" cy=\"250\" r=\"200\"/></svg>",
);

const CIRCLE_SVG_BASE64: &str = concat!(
    "PD94bWwgdmVyc2lvbj0iMS4wIiBlbmNvZGluZz0iVVRGLTgiPz4KPHN2ZyB4bWxucz0iaHR0",
    "cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSI1MDAiIGhlaWdodD0iNTAwIiBzdHls",
    "ZT0iZmlsbDpyZWQiIHZpZXdCb3g9IjAgMCA1MDAgNTAwIj48Y2lyY2xlIGN4PSIyNTAiIGN5",
    "PSIyNTAiIHI9IjIwMCIvPjwvc3ZnPg==",
);

fn rendered(doc: &crate::Document<'_>) -> String {
    rendered_paragraphs(doc).join("\n")
}

/// A renderer that overrides only [`render_special_character`], emitting
/// bracketed placeholders instead of HTML entities. Every other substitution
/// falls through to the inherited default (the built-in HTML renderer).
///
/// [`render_special_character`]: InlineSubstitutionRenderer::render_special_character
#[derive(Debug)]
struct BracketSpecialChars;

impl InlineSubstitutionRenderer for BracketSpecialChars {
    fn render_special_character(&self, type_: SpecialCharacter, dest: &mut String) {
        match type_ {
            SpecialCharacter::Lt => dest.push_str("[LT]"),
            SpecialCharacter::Gt => dest.push_str("[GT]"),
            SpecialCharacter::Ampersand => dest.push_str("[AMP]"),
        }
    }
}

#[test]
fn overrides_one_method_and_inherits_the_rest() {
    // Only special-character rendering is customized; the `*strong*` quote
    // substitution is inherited unchanged from the built-in HTML renderer,
    // proving a consumer no longer faces an all-or-nothing implementation.
    let doc = Parser::default()
        .with_inline_substitution_renderer(BracketSpecialChars)
        .parse("a < b > c & d *bold*");

    assert_eq!(
        rendered(&doc),
        "a [LT] b [GT] c [AMP] d <strong>bold</strong>"
    );
}

/// A renderer that overrides [`render_image`] to emit its own markup but
/// reaches the built-in `data-uri` embedding through the inherited
/// [`image_uri`](InlineSubstitutionRenderer::image_uri).
///
/// [`render_image`]: InlineSubstitutionRenderer::render_image
#[derive(Debug)]
struct FigureImages;

impl InlineSubstitutionRenderer for FigureImages {
    fn render_image(&self, params: &ImageRenderParams, dest: &mut String) {
        // `image_uri` is not overridden, so this inherits the crate's data-uri
        // embedding, which reads the image bytes through the registered
        // `ImageFileHandler` – behavior a custom renderer previously could not
        // reproduce.
        let uri = self.image_uri(params.target, params.parser, None);

        dest.push_str(&format!(
            r#"<figure data-src="{uri}">{alt}</figure>"#,
            alt = params.alt
        ));
    }
}

#[test]
fn inherited_image_uri_embeds_data_uri_for_a_custom_renderer() {
    // Below `Secure`, with `data-uri` set and a handler registered, the
    // inherited `image_uri` embeds the image as a `data:` URI – so a custom
    // renderer that only reshapes the surrounding markup still gets embedding.
    let doc = Parser::default()
        .with_inline_substitution_renderer(FigureImages)
        .with_safe_mode(SafeMode::Server)
        .with_intrinsic_attribute_bool("data-uri", true, ModificationContext::Anywhere)
        .with_intrinsic_attribute("imagesdir", "fixtures", ModificationContext::Anywhere)
        .with_image_file_handler(ImageFileHandlerFixture::from_pairs([(
            "fixtures/circle.svg",
            CIRCLE_SVG.as_bytes(),
        )]))
        .parse("image:circle.svg[Tiger]");

    assert_eq!(
        rendered(&doc),
        format!(
            r#"<figure data-src="data:image/svg+xml;base64,{CIRCLE_SVG_BASE64}">Tiger</figure>"#
        )
    );
}

#[test]
fn file_handler_accessors_expose_registered_handlers() {
    // A parser with no handlers reports none.
    let bare = Parser::default();
    assert!(bare.image_file_handler().is_none());
    assert!(bare.svg_file_handler().is_none());

    // Once registered, the handlers are reachable – and usable – through the
    // public accessors, so a renderer that resolves asset URIs itself can read
    // the same bytes the built-in renderer would.
    let parser = Parser::default()
        .with_image_file_handler(ImageFileHandlerFixture::from_pairs([(
            "fixtures/circle.svg",
            CIRCLE_SVG.as_bytes(),
        )]))
        .with_svg_file_handler(SvgFileHandlerFixture::from_pairs([(
            "fixtures/circle.svg",
            CIRCLE_SVG,
        )]));

    let image_handler = parser
        .image_file_handler()
        .expect("image file handler should be registered");

    assert_eq!(
        image_handler.resolve_image("fixtures/circle.svg", &parser),
        Some(CIRCLE_SVG.as_bytes().to_vec())
    );

    let svg_handler = parser
        .svg_file_handler()
        .expect("SVG file handler should be registered");

    assert_eq!(
        svg_handler.resolve_svg("fixtures/circle.svg", &parser),
        Some(CIRCLE_SVG.to_string())
    );
}
