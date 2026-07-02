use crate::tests::prelude::*;

track_file!("docs/modules/macros/pages/image-svg.adoc");

non_normative!(
    r#"
= SVG Images
:url-svg-editor: https://petercollingridge.appspot.com/svg-editor
:url-svgo: https://github.com/svg/svgo

Both block and inline image macros have built-in support for scalable vector graphics (SVGs).
But there's more than one way to include an SVG into a web page, and the strategy used can affect how the SVG behaves (or misbehaves).
Therefore, these macros provide additional options to control how the SVG is included (i.e., referenced).

"#
);

mod svg_dimensions {
    use crate::tests::prelude::*;

    non_normative!(
        r#"
== SVG dimensions

The viewBox attribute on the root `<svg>` element is required.
The viewBox establishes the coordinate space on to which x and y values inside the SVG data are placed.
Without this information, converters cannot properly interpret the SVG data and translate it to the canvas.

We strongly recommend not using the width and height attributes on the root `<svg>` element.
You will find that SVGs that only specify a viewBox work best in a document.
That's because the SVG data itself is infinitely scalable.
By assigning an explicit width and height, you may end up limiting how the SVG can be sized or positioned in the document.
It's better to specify the width (or similar, such as pdfwidth) on the image macro instead, or using CSS.

You particularly want to avoid using a percentage width, such as `width="100%"`.
According to the SVG spec, this means using all available space, but without altering the aspect ratio.
As a result, you can get a large gap above and below the SVG in page-oriented media, such as PDF.
If you do specify a width and height, at least make sure the values are fixed and that they respect the aspect ratio of the data.

"#
    );
}

mod options_for_svg_images {
    use crate::{
        SafeMode,
        tests::{fixtures::svg_file_handler::SvgFileHandlerFixture, prelude::*},
    };

    /// The raw contents that the SVG file handler returns for `sample.svg`. The
    /// opening `<svg>` tag carries `width`, `height`, and `style` attributes
    /// (which inline rendering strips when an explicit width is supplied), plus
    /// an XML preamble (which is dropped) and the required `viewBox`.
    const SAMPLE_SVG: &str = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"500\" height=\"500\" ",
        "style=\"fill:red\" viewBox=\"0 0 500 500\">",
        "<circle cx=\"250\" cy=\"250\" r=\"200\"/></svg>",
    );

    fn rendered(doc: &crate::Document<'_>) -> String {
        rendered_paragraphs(doc).join("\n")
    }

    non_normative!(
        r#"
== Options for SVG images

When the image target is an SVG, the `options` attribute (often abbreviated as `opts`) on the macro accepts one of the following values to control how the SVG is referenced:

* _none_ (default)
* `interactive`
* `inline`

The following table demonstrates the impact these options have.

.Demonstration of option values for SVG images
[cols=2*,frame=ends,grid=none]
|===
2+l|image::sample.svg[Static,300]
a|image::sample.svg[Static,300]
|Observe that the SVG does not respond to the hover event.

2+l|image::sample.svg[Interactive,300,opts=interactive]
a|image::sample.svg[Interactive,300,opts=interactive]
|Observe that the color changes when hovering over the SVG.

2+l|image::sample.svg[Embedded,300,opts=inline]
a|image::sample.svg[Embedded,300,opts=interactive]
// the output uses the interactive version as the documentation doesn't currently support the `inline` option.
|Observe that the color changes when hovering over the SVG.
The SVG also inherits CSS from the document stylesheets.
|===

How these options values work and when each should be used is described below:

.Option values that control how an SVG image is referenced in the HTML output
[%autowidth]
|===
|Option |HTML Element Used |Effect |When To Use

"#
    );

    #[test]
    fn none_option_uses_img_element() {
        verifies!(
            r#"
|_none_ (default)
|`<img>`
|Image is rasterized
|Static image, no interactivity, no custom fonts

"#
        );

        // With no option (and in the default `Secure` safe mode) an SVG image is
        // referenced with a plain `<img>` element, exactly like any other image.
        let doc = Parser::default().parse("image:sample.svg[Static,300]");

        assert_eq!(
            rendered(&doc),
            r#"<span class="image"><img src="sample.svg" alt="Static" width="300"></span>"#
        );
    }

    #[test]
    fn interactive_option_uses_object_element() {
        verifies!(
            r#"
|`interactive`
|`<object>`
|Image embedded as a live, interactive object
|For using CSS animations, scripting, webfonts, or when you want to specify a fallback image

"#
        );

        // The `interactive` option references the SVG with an `<object>` element
        // so its embedded scripting and links stay live. It is security
        // sensitive, so it only takes effect below the `Secure` safe mode; the
        // alt text becomes the object's fallback content.
        let doc = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .parse("image:sample.svg[Interactive,300,opts=interactive]");

        assert_eq!(
            rendered(&doc),
            r#"<span class="image"><object type="image/svg+xml" data="sample.svg" width="300"><span class="alt">Interactive</span></object></span>"#
        );
    }

    #[test]
    fn interactive_is_ignored_in_secure_mode() {
        // Because the `interactive` option is security sensitive, in the default
        // `Secure` safe mode it is ignored and the SVG is referenced with a plain
        // `<img>`, exactly as Ruby Asciidoctor does. (The page itself does not
        // discuss safe mode, so this is behavior coverage rather than a
        // `verifies!` of normative page text.)
        let doc = Parser::default().parse("image:sample.svg[Interactive,300,opts=interactive]");

        assert_eq!(
            rendered(&doc),
            r#"<span class="image"><img src="sample.svg" alt="Interactive" width="300"></span>"#
        );
    }

    #[test]
    fn inline_option_uses_svg_element() {
        verifies!(
            r#"
|`inline`
|`<svg>`
|The SVG is embedded directly into the HTML itself
|For using CSS animations, scripting, webfonts, when you require search engines to search the SVG content

To allow SVG content reachable by JavaScript in the main DOM or to inherit styles from the main DOM
|===

"#
        );

        // The `inline` option embeds the SVG contents directly into the HTML as
        // an `<svg>` element. The contents are supplied by the parser's
        // `SvgFileHandler`; like `interactive`, it only takes effect below the
        // `Secure` safe mode.
        let doc = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_svg_file_handler(SvgFileHandlerFixture::from_pairs([(
                "sample.svg",
                SAMPLE_SVG,
            )]))
            .parse("image:sample.svg[Embedded,300,opts=inline]");

        assert_eq!(
            rendered(&doc),
            concat!(
                r#"<span class="image">"#,
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 500 500" width="300">"#,
                r#"<circle cx="250" cy="250" r="200"/></svg></span>"#,
            )
        );
    }

    #[test]
    fn inline_is_ignored_in_secure_mode() {
        // Like `interactive`, the `inline` option is disabled in the default
        // `Secure` safe mode: the registered `SvgFileHandler` is never consulted
        // and a plain `<img>` is emitted instead of the embedded `<svg>`.
        let doc = Parser::default()
            .with_svg_file_handler(SvgFileHandlerFixture::from_pairs([(
                "sample.svg",
                SAMPLE_SVG,
            )]))
            .parse("image:sample.svg[Embedded,300,opts=inline]");

        assert_eq!(
            rendered(&doc),
            r#"<span class="image"><img src="sample.svg" alt="Embedded" width="300"></span>"#
        );
    }

    #[test]
    fn interactive_fallback_is_prefixed_with_imagesdir() {
        verifies!(
            r#"
When using the `interactive` option, you can specify a fallback image using the `fallback` attribute.
The fallback image is used if the browser does not support the `<object>` tag.
If the value of the fallback attribute is a relative path, it will be prefixed with the value of the `imagesdir` document attribute.

"#
        );

        // The `fallback` image nests inside the `<object>` as an `<img>`. Its
        // relative path (`fallback.png`) is prefixed with `imagesdir` (`images`),
        // just like the object's own `data` target.
        let doc = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_intrinsic_attribute("imagesdir", "images", ModificationContext::Anywhere)
            .parse("image:sample.svg[Diagram,fallback=fallback.png,opts=interactive]");

        assert_eq!(
            rendered(&doc),
            r#"<span class="image"><object type="image/svg+xml" data="images/sample.svg"><img src="images/fallback.png" alt="Diagram"></object></span>"#
        );
    }

    non_normative!(
        r#"
When using the `inline` or `interactive` options, the `viewBox` attribute must be defined on the root `<svg>` element in order for scaling to work properly.

"#
    );

    #[test]
    fn inline_strips_width_height_and_style() {
        verifies!(
            r#"
When using the `inline` option, if you specify a width or height on the image macro in AsciiDoc, the `width`, `height` and `style` attributes on the `<svg>` element will be removed. Additionally, when using `inline` the primary SVG elements (e.g., `<svg>`) cannot have a namespace.

"#
        );

        // The fixture's opening `<svg>` tag carries `width`, `height`, and
        // `style` attributes. Supplying a width (and here a height) on the image
        // macro removes all three from the tag and appends the requested
        // dimensions instead; the required `viewBox` is preserved.
        //
        // This test exercises only the attribute-stripping clause of the spec
        // line above. The trailing "cannot have a namespace" clause is an
        // authoring constraint on the SVG *source* — its element names must not
        // be namespace-prefixed (e.g. `<svg:svg>`/`<svg:circle>`) — not a parser
        // behavior: the parser embeds the SVG verbatim and never rewrites element
        // names. The preserved `xmlns="…"` here is a default-namespace
        // *declaration*, which is unrelated to (and not prohibited by) that
        // clause; Asciidoctor keeps it as well.
        let doc = Parser::default()
            .with_safe_mode(SafeMode::Server)
            .with_svg_file_handler(SvgFileHandlerFixture::from_pairs([(
                "sample.svg",
                SAMPLE_SVG,
            )]))
            .parse("image:sample.svg[Embedded,300,200,opts=inline]");

        assert_eq!(
            rendered(&doc),
            concat!(
                r#"<span class="image">"#,
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 500 500" width="300" height="200">"#,
                r#"<circle cx="250" cy="250" r="200"/></svg></span>"#,
            )
        );
    }

    non_normative!(
        r#"
If using the `interactive` option, you must link to the CSS that declares the fonts in the SVG file using an XML stylesheet declaration.

If you're inserting an SVG using either the `inline` or `interactive` options, we strongly recommend you optimize your SVG using a tool like {url-svgo}[svgo^] or {url-svg-editor}[SVG Editor^].

As you work with SVG, you'll become more comfortable making the decision about which method to employ given the circumstances.
It's only confusing when you first encounter the choice.
To learn more about using SVG on the web, consult the online book https://svgontheweb.com/[SVG on the Web: A Practical Guide^] as well as https://www.sarasoueidan.com/tags/svg/[these articles about SVG^].
"#
    );
}
