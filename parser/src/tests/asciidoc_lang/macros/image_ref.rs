use crate::tests::prelude::*;

track_file!("docs/modules/macros/pages/image-ref.adoc");

non_normative!(
    r#"
= Images Reference

.Document attributes and values
[cols=2;2;3;3]
|===
|Attribute |Value(s) |Example Syntax |Comments

"#
);

#[test]
fn document_imagesdir() {
    verifies!(
        r#"
|`imagesdir`
|empty, filesystem path, or base URL
|`:imagesdir: images`
|Added in front of a relative image target, joined using a file separator if needed.
Not used if the image target is an absolute URL or path.
Default value is empty.
"#
    );

    // A relative target is prefixed with `imagesdir`, joined with `/`.
    let doc = Parser::default()
        .with_intrinsic_attribute("imagesdir", "images", ModificationContext::Anywhere)
        .parse("image:sunset.jpg[Sunset]");
    assert_eq!(
        rendered_paragraphs(&doc).join("\n"),
        r#"<span class="image"><img src="images/sunset.jpg" alt="Sunset"></span>"#
    );

    // An absolute-URL target ignores `imagesdir` entirely.
    let doc = Parser::default()
        .with_intrinsic_attribute("imagesdir", "images", ModificationContext::Anywhere)
        .parse("image:https://example.org/sunset.jpg[Sunset]");
    assert_eq!(
        rendered_paragraphs(&doc).join("\n"),
        r#"<span class="image"><img src="https://example.org/sunset.jpg" alt="Sunset"></span>"#
    );
}

non_normative!(
    r#"
|===

.Block and inline image attributes and values
[cols=2;2;3;3]
|===
|Attribute |Value(s) |Example Syntax |Comments

"#
);

#[test]
fn id() {
    verifies!(
        r#"
|`id`
|User defined text
|`id=sunset-img` +
(or `+[[sunset-img]]+` or `[#sunset-img]` above block macro)
|

"#
    );

    // The named `id=` attribute inside a block image macro sets the block's ID
    // (and registers it in the document catalog so it can be cross-referenced),
    // exactly like the `[[sunset-img]]` / `[#sunset-img]` block-anchor forms.
    let doc = Parser::default().parse("image::sunset.jpg[Sunset,id=sunset-img]");
    let block = doc.nested_blocks().next().unwrap();
    assert_eq!(block.id(), Some("sunset-img"));
}

#[test]
fn alt() {
    verifies!(
        r#"
|`alt`
|User defined text in first position of attribute list or named attribute
|`image::sunset.jpg[Brilliant sunset]` +
(or `alt=Sunset`)
|

"#
    );

    // The first positional attribute supplies the alt text.
    let doc = Parser::default().parse("image:sunset.jpg[Brilliant sunset]");
    assert_eq!(
        rendered_paragraphs(&doc).join("\n"),
        r#"<span class="image"><img src="sunset.jpg" alt="Brilliant sunset"></span>"#
    );
}

#[test]
fn fallback() {
    verifies!(
        r#"
|`fallback`
|Image path relative to `imagesdir` or an absolute path or URL
|`image::tiger.svg[fallback=tiger.png]`
|Only applicable if target is an SVG and opts=interactive

"#
    );

    // The `fallback` image is nested inside the interactive SVG `<object>` for
    // user agents that cannot render the object. (Interactive SVG handling is
    // security sensitive, so it only takes effect below the `Secure` safe mode.)
    let doc = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .parse("image:tiger.svg[Tiger,fallback=tiger.png,opts=interactive]");
    assert_eq!(
        rendered_paragraphs(&doc).join("\n"),
        concat!(
            r#"<span class="image"><object type="image/svg+xml" data="tiger.svg">"#,
            r#"<img src="tiger.png" alt="Tiger"></object></span>"#,
        )
    );
}

#[test]
fn title() {
    verifies!(
        r#"
|`title`
|User defined text
|in attrlist: `title="A mountain sunset"` (enclosing quotes only required if value contains a comma) +
above block macro: `.A mountain sunset`
|Blocks: title displayed below image +
Inline: title displayed as tooltip

"#
    );

    // For an inline image the title becomes the `title` attribute on the
    // `<img>` (rendered by browsers as a tooltip).
    let doc = Parser::default().parse(r#"image:sunset.jpg[Sunset,title="A mountain sunset"]"#);
    assert_eq!(
        rendered_paragraphs(&doc).join("\n"),
        r#"<span class="image"><img src="sunset.jpg" alt="Sunset" title="A mountain sunset"></span>"#
    );
}

// The `format` attribute is verified normatively on the dedicated
// xref:image-format.adoc page (see `image_format.rs`).
non_normative!(
    r#"
|`format`
|The format of the image, specified as a sub-MIME type (except in the case of an SVG, which is specified as `svg`).
|`format=svg`
|Only necessary when the converter needs to know the format of the image and the target does not end in a file extension (or otherwise cannot be detected).

"#
);

#[test]
fn caption() {
    verifies!(
        r#"
|`caption`
|User defined text
|`caption="Figure 8: "`
|Only applies to block images.

"#
    );

    // An explicit `caption` on a block image supplies a verbatim, unnumbered
    // caption prefix (it wins over the automatic "Figure N. " label).
    let doc = Parser::default()
        .parse(".A mountain sunset\nimage::sunset.jpg[Sunset,caption=\"Figure 8: \"]");
    let block = doc.nested_blocks().next().unwrap();
    assert_eq!(block.caption(), Some("Figure 8: "));
    assert_eq!(block.number(), None);
}

#[test]
fn width() {
    verifies!(
        r#"
|`width`
|User defined size in pixels
|`image::sunset.jpg[Sunset,300]` +
(or `width=300`)
|

"#
    );

    let doc = Parser::default().parse("image:sunset.jpg[Sunset,300]");
    assert_eq!(
        rendered_paragraphs(&doc).join("\n"),
        r#"<span class="image"><img src="sunset.jpg" alt="Sunset" width="300"></span>"#
    );
}

#[test]
fn height() {
    verifies!(
        r#"
|`height`
|User defined size in pixels
|`image::sunset.jpg[Sunset,300,200]` +
(or `height=200`)
|The height should only be set if the width attribute is set and must respect the aspect ratio of the image.

"#
    );

    let doc = Parser::default().parse("image:sunset.jpg[Sunset,300,200]");
    assert_eq!(
        rendered_paragraphs(&doc).join("\n"),
        r#"<span class="image"><img src="sunset.jpg" alt="Sunset" width="300" height="200"></span>"#
    );
}

#[test]
fn link() {
    verifies!(
        r#"
|`link`
|User defined location of external URI
|`link=https://www.flickr.com/photos/javh/5448336655`
|

"#
    );

    // The `link` attribute wraps the image in an `<a class="image">`.
    let doc = Parser::default()
        .parse("image:sunset.jpg[Sunset,link=https://www.flickr.com/photos/javh/5448336655]");
    assert_eq!(
        rendered_paragraphs(&doc).join("\n"),
        concat!(
            r#"<span class="image">"#,
            r#"<a class="image" href="https://www.flickr.com/photos/javh/5448336655">"#,
            r#"<img src="sunset.jpg" alt="Sunset"></a></span>"#,
        )
    );
}

#[test]
fn window() {
    verifies!(
        r#"
|`window`
|User defined window target for the `link` attribute
|`window=_blank`
|

"#
    );

    // `window` sets the link's `target`; `_blank` additionally adds
    // `rel="noopener"`.
    let doc =
        Parser::default().parse("image:sunset.jpg[Sunset,link=https://example.org,window=_blank]");
    assert_eq!(
        rendered_paragraphs(&doc).join("\n"),
        concat!(
            r#"<span class="image">"#,
            r#"<a class="image" href="https://example.org" target="_blank" rel="noopener">"#,
            r#"<img src="sunset.jpg" alt="Sunset"></a></span>"#,
        )
    );
}

// `scale`, `scaledwidth`, and `pdfwidth` size images only for the DocBook
// and/or Asciidoctor PDF backends; they have no effect in this HTML crate.
non_normative!(
    r#"
|`scale`
|A scaling factor to apply to the intrinsic image dimensions
|`scale=80`
|DocBook only

|`scaledwidth`
|User defined width for block images
|`scaledwidth=25%`
|DocBook and Asciidoctor PDF only

|`pdfwidth`
|User defined width for images in a PDF
|`pdfwidth=80vw`
|Asciidoctor PDF only

"#
);

// `align` and `float` position a *block* image; whole-block HTML rendering
// is not produced by this crate, so there is no rendered output to assert.
non_normative!(
    r#"
|`align`
|`left`, `center`, `right`
|`align=left`
|Block images only.
`align` and `float` attributes are mutually exclusive.

|`float`
|`left`, `right`
|`float=right`
|Block images only.
`float` and `align` attributes are mutually exclusive.
To scope the float, use a xref:image-position.adoc#control-float[float group].

"#
);

#[test]
fn role() {
    verifies!(
        r#"
|`role`
|user-defined, `left`, `right`, `th`, `thumb`, `related`, `rel`
|`role="thumb right"` +
(or `[.thumb.right]` above block macro)
|The role is preferred to specify the float position for an image.
Role shorthand (`.`) can only be used in block attribute list above a block image.

"#
    );

    // Roles are appended to the wrapper's `class` after the `image` class.
    let doc = Parser::default().parse(r#"image:sunset.jpg[Sunset,role="thumb right"]"#);
    assert_eq!(
        rendered_paragraphs(&doc).join("\n"),
        r#"<span class="image thumb right"><img src="sunset.jpg" alt="Sunset"></span>"#
    );
}

#[test]
fn macro_imagesdir() {
    verifies!(
        r#"
|`imagesdir`
|empty, filesystem path, or base URL
|`imagesdir=ch1/images`
|Overrides the `imagesdir` set on the document.
If not specified, the `imagesdir` from the document is used.
(Not supported until Asciidoctor 2.1).

"#
    );

    // A macro-level `imagesdir` overrides the document `imagesdir` for this one
    // image.
    let doc = Parser::default()
        .with_intrinsic_attribute("imagesdir", "images", ModificationContext::Anywhere)
        .parse("image:sunset.jpg[Sunset,imagesdir=ch1/images]");
    assert_eq!(
        rendered_paragraphs(&doc).join("\n"),
        r#"<span class="image"><img src="ch1/images/sunset.jpg" alt="Sunset"></span>"#
    );
}

#[test]
fn opts() {
    verifies!(
        r#"
|`opts`
|Additional options for link creation and SVG targets.
|`image::sunset.jpg[Sunset, link=https://example.org, opts=nofollow]`

`image::chart.svg[opts=inline]`
|Option names include: `nofollow`, `noopener`, `inline` (SVG only), `interactive` (SVG only)
|===
"#
    );

    // `opts=nofollow` adds `rel="nofollow"` to the image's link. (The SVG-only
    // `inline` and `interactive` options are verified on the image-svg page.)
    let doc = Parser::default()
        .parse("image:sunset.jpg[Sunset, link=https://example.org, opts=nofollow]");
    assert_eq!(
        rendered_paragraphs(&doc).join("\n"),
        concat!(
            r#"<span class="image">"#,
            r#"<a class="image" href="https://example.org" rel="nofollow">"#,
            r#"<img src="sunset.jpg" alt="Sunset"></a></span>"#,
        )
    );
}
