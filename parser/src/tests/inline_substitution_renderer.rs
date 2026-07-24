//! Tests for the public [`InlineSubstitutionRenderer`] extension surface: a
//! downstream renderer that overrides only the substitutions it cares about and
//! inherits the built-in HTML behavior (including `data-uri` embedding) for the
//! rest, plus the [`Parser`] accessors that expose the registered file
//! handlers.
//!
//! [`InlineSubstitutionRenderer`]: crate::parser::InlineSubstitutionRenderer

use crate::{
    Span,
    content::{Content, SubstitutionStep},
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

/// A renderer that overrides nothing, so every substitution falls through to
/// the inherited default. Its output must match the built-in
/// [`HtmlSubstitutionRenderer`] exactly.
///
/// [`HtmlSubstitutionRenderer`]: crate::parser::HtmlSubstitutionRenderer
#[derive(Debug)]
struct InheritEverything;

impl InlineSubstitutionRenderer for InheritEverything {}

/// Applies `step` to `source` twice – once through the default (HTML) renderer
/// and once through [`InheritEverything`] – with the same parser configuration,
/// and asserts the two rendered outputs are identical. Because every default
/// method body delegates to the built-in HTML renderer, a mismatch means a
/// default no longer reproduces the HTML output it promises.
fn assert_inherits_html(
    source: &str,
    step: SubstitutionStep,
    configure: impl Fn(Parser) -> Parser,
) {
    let render = |parser: &Parser| {
        let mut content = Content::from(Span::new(source));
        step.apply(&mut content, parser, None);
        content.rendered().to_string()
    };

    let expected = render(&configure(Parser::default()));
    let actual =
        render(&configure(Parser::default()).with_inline_substitution_renderer(InheritEverything));

    assert_eq!(
        actual, expected,
        "mismatch rendering {source:?} via {step:?}"
    );
}

#[test]
fn an_empty_renderer_matches_the_html_renderer_for_every_substitution() {
    let plain = |p: Parser| p;

    // Set the `experimental` attribute so the UI macros (`btn`/`kbd`/`menu`)
    // are recognized rather than passed through literally.
    let experimental = |p: Parser| {
        p.with_intrinsic_attribute_bool("experimental", true, ModificationContext::Anywhere)
    };

    // One case per default method body, driving the substitution step that
    // reaches it. Each exercises the inherited default and confirms it delegates
    // to the built-in HTML renderer.
    assert_inherits_html("a < b & c > d", SubstitutionStep::SpecialCharacters, plain);
    assert_inherits_html(
        "(C) (R) (TM) -- ... -> <- => <= it's",
        SubstitutionStep::CharacterReplacements,
        plain,
    );

    // `#alert#` with a role takes the `<span>` branch of the `Mark` quote type
    // (a bare `#marked#` would use `<mark>`), covering both.
    assert_inherits_html(
        "plain *bold* _em_ `code` #marked# [red]#alert#",
        SubstitutionStep::Quotes,
        plain,
    );

    // A trailing ` +` forces a hard line break, handled in post-replacement.
    assert_inherits_html(
        "first line +\nsecond line",
        SubstitutionStep::PostReplacement,
        plain,
    );

    // A `<N>` callout marker arrives here already special-character-escaped.
    assert_inherits_html("code &lt;1&gt;", SubstitutionStep::Callouts, plain);

    // The macros step reaches the image, icon, link, anchor, index-term, and
    // footnote renderers.
    assert_inherits_html(
        "image:foo.png[Alt,200,100]",
        SubstitutionStep::Macros,
        plain,
    );
    assert_inherits_html("icon:home[]", SubstitutionStep::Macros, plain);
    assert_inherits_html(
        "link:https://example.org[text]",
        SubstitutionStep::Macros,
        plain,
    );
    assert_inherits_html("[[an-anchor]]", SubstitutionStep::Macros, plain);
    assert_inherits_html(
        "a ((visible term)) and (((concealed,term)))",
        SubstitutionStep::Macros,
        plain,
    );
    assert_inherits_html(
        "a footnote:[the note here]",
        SubstitutionStep::Macros,
        plain,
    );

    // The UI macros require the `experimental` attribute.
    assert_inherits_html("press btn:[OK]", SubstitutionStep::Macros, experimental);
    assert_inherits_html("hit kbd:[Ctrl+T]", SubstitutionStep::Macros, experimental);
    assert_inherits_html(
        "open menu:File[Save > As]",
        SubstitutionStep::Macros,
        experimental,
    );
}

#[test]
fn an_empty_renderer_matches_the_html_renderer_for_cross_references() {
    // Cross-reference rendering is deferred past the macros step to reference
    // resolution, so it is exercised through a full parse rather than a single
    // substitution step. The document has one resolved reference (to the
    // anchored target) and one unresolved reference, covering both branches.
    let source = concat!(
        "[[the-target]]The target paragraph.\n\n",
        "See <<the-target>> and <<missing>>.\n",
    );

    let default_doc = Parser::default().parse(source);

    let inherit_doc = Parser::default()
        .with_inline_substitution_renderer(InheritEverything)
        .parse(source);

    assert_eq!(
        rendered_paragraphs(&inherit_doc),
        rendered_paragraphs(&default_doc)
    );
}
