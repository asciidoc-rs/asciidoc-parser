use crate::tests::prelude::*;

track_file!("ref/asciidoc-lang/docs/modules/macros/pages/icon-macro.adoc");

non_normative!(
    r#"
= Icon Macro

In addition to built-in icons, you can add icons anywhere in your content where macros are substituted using the icon macro.
This page covers the anatomy of the icon macro, how the target is resolved, and what features it support (subject to the icon mode).

"#
);

mod anatomy {
    use crate::tests::prelude::*;

    non_normative!(
        r#"
== Anatomy

The icon macro is an inline macro.
Like other inline macros, its syntax follows the familiar pattern of the macro name and target separated by a colon followed by an attribute list enclosed in square brackets.

[source]
----
icon:<target>[<attrlist>]
----

The `<target>` is the icon name or path.
The `<attrlist>` specifies various named attributes to configure how the icon is displayed.

"#
    );

    #[test]
    fn example() {
        verifies!(
            r#"
For example:

[source]
----
icon:heart[2x,role=red]
----

"#
        );

        let doc = Parser::default().parse("icon:heart[2x,role=red]");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                },
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "icon:heart[2x,role=red]",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<span class=\"icon red\">[heart&#93;</span>",
                    },
                    source: Span {
                        data: "icon:heart[2x,role=red]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: "icon:heart[2x,role=red]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }
}

mod example {
    use crate::tests::prelude::*;

    #[test]
    fn tags_example() {
        verifies!(
            r#"
== Example

Here's an example that shows how to inserts an icon named _tags_ in front of a list of tag names.

[source]
----
icon:tags[] ruby, asciidoctor
----

Here's how the HTML converter converts an icon macro when the `icons` attribute is not set or empty.

.Result: HTML output
[source,html]
----
<div class="paragraph">
  <p><span class="image"><img src="./images/icons/tags.png" alt="tags"></span> ruby, asciidoctor</p>
</div>
----

"#
        );

        // The spec's "not set or empty" wording is loose: with the `icons`
        // attribute genuinely unset the icon renders as bracketed fallback text
        // (see `how_the_icon_is_resolved` below); the image `<img>` output shown
        // here is produced when `icons` is *empty* (i.e. image mode). This crate
        // renders only the inline fragment, so the `<span class="icon">` wrapper
        // appears without the enclosing `<div class="paragraph"><p>` block markup
        // shown above. It also uses `class="icon"` (matching Asciidoctor's actual
        // output) rather than the `class="image"` printed in this spec example.
        let doc = Parser::default().parse(":icons:\n\nicon:tags[] ruby, asciidoctor");

        assert_eq!(
            rendered_paragraphs(&doc).join("\n"),
            r#"<span class="icon"><img src="./images/icons/tags.png" alt="tags"></span> ruby, asciidoctor"#
        );
    }

    // The DocBook converter is not implemented by this crate, so the
    // `<inlinemediaobject>` rendering below is not verified here. Likewise, the
    // fallback behavior when an icon image can't be located requires file-system
    // access this crate leaves to the caller.
    non_normative!(
        r#"
Here's how the DocBook converter converts an icon macro.

[source,xml]
----
<inlinemediaobject>
  <imageobject>
    <imagedata fileref="./images/icons/tags.png"/>
  </imageobject>
  <textobject><phrase>tags</phrase></textobject>
</inlinemediaobject> ruby, asciidoctor
----

When the image for an icon can't be located in the icons directory, the AsciiDoc processor displays the icon macro's alt (i.e. fallback) text.

"#
    );
}

mod how_the_icon_is_resolved {
    use crate::tests::prelude::*;

    #[test]
    fn resolves_by_mode() {
        verifies!(
            r#"
== How the icon is resolved

The target of the icon macro is an icon name (or path).
How that target is resolved depends on the icon mode assigned to the xref:icons.adoc#icons-attribute[icons attribute].

text::
The icon name will be enclosed in square brackets (e.g., `[heart]`).

image::
The icon name will be resolved to a file in the `iconsdir` with the file extension specified by `icontype` (e.g., [.path]_./images/icons/heart.png_).

font::
The icon name will be resolved to a glyph in an icon font (as mapped by a CSS class) (e.g., `fa fa-heart`).

WARNING: If you include a file extension in the image target, the icon macro will not work correctly when using the font icon mode (i.e., `icons=font`).

"#
        );

        // text mode (the `icons` attribute is not set): the icon name is
        // enclosed in square brackets (the `]` is emitted as `&#93;`).
        let doc = Parser::default().parse("icon:heart[]");
        assert_eq!(
            rendered_paragraphs(&doc).join("\n"),
            r#"<span class="icon">[heart&#93;</span>"#
        );

        // image mode: the name resolves to a file in `iconsdir` with the
        // `icontype` extension (default `png`).
        let doc = Parser::default().parse(":icons: image\n\nicon:heart[]");
        assert_eq!(
            rendered_paragraphs(&doc).join("\n"),
            r#"<span class="icon"><img src="./images/icons/heart.png" alt="heart"></span>"#
        );

        // font mode: the name resolves to a `fa fa-<name>` glyph class.
        let doc = Parser::default().parse(":icons: font\n\nicon:heart[]");
        assert_eq!(
            rendered_paragraphs(&doc).join("\n"),
            r#"<span class="icon"><i class="fa fa-heart"></i></span>"#
        );

        // WARNING: a file extension in the target breaks font icon mode, because
        // the extension is carried into the glyph class (`fa-heart.png`).
        let doc = Parser::default().parse(":icons: font\n\nicon:heart.png[]");
        assert_eq!(
            rendered_paragraphs(&doc).join("\n"),
            r#"<span class="icon"><i class="fa fa-heart.png"></i></span>"#
        );
    }
}

mod shared_attributes {
    use crate::tests::prelude::*;

    #[test]
    fn title() {
        verifies!(
            r#"
== Icon macro attributes (shared)

The following attributes of the icon macro are shared for all icon modes.

`role`::
The role applied to the element that surrounds the icon.

`title`::
The title of the image displayed when the mouse hovers over it.

`link`::
The URI target used for the icon, which will wrap the converted icon in a link.

`window`::
The target window of the link (when the `link` attribute is specified).

"#
        );

        // `role`, `link`, and `window` are each demonstrated by their own
        // example below; here we exercise `title`, which adds a `title`
        // attribute to the rendered image.
        let doc = Parser::default().parse(":icons: image\n\nicon:tags[title=Tags]");
        assert_eq!(
            rendered_paragraphs(&doc).join("\n"),
            r#"<span class="icon"><img src="./images/icons/tags.png" alt="tags" title="Tags"></span>"#
        );
    }

    #[test]
    fn role() {
        verifies!(
            r#"
=== Role

Here's an example of an icon that uses a role to specify the color.

[source]
----
icon:tags[role=blue] ruby, asciidoctor
----

"#
        );

        // The role is appended to the class of the `<span>` surrounding the icon.
        let doc =
            Parser::default().parse(":icons: image\n\nicon:tags[role=blue] ruby, asciidoctor");
        assert_eq!(
            rendered_paragraphs(&doc).join("\n"),
            r#"<span class="icon blue"><img src="./images/icons/tags.png" alt="tags"></span> ruby, asciidoctor"#
        );
    }

    #[test]
    fn link_and_window() {
        verifies!(
            r#"
=== Link and window

Here's an example of an icon with a link that targets a separate window:

[source]
----
icon:download[link=https://rubygems.org/downloads/whizbang-1.0.0.gem, window=_blank]
----

"#
        );

        // `link` wraps the icon in an `<a class="image">`; `window=_blank` sets
        // the link's `target` and adds `rel="noopener"`.
        let doc = Parser::default().parse(
            ":icons: image\n\nicon:download[link=https://rubygems.org/downloads/whizbang-1.0.0.gem, window=_blank]",
        );
        assert_eq!(
            rendered_paragraphs(&doc).join("\n"),
            concat!(
                r#"<span class="icon">"#,
                r#"<a class="image" href="https://rubygems.org/downloads/whizbang-1.0.0.gem" target="_blank" rel="noopener">"#,
                r#"<img src="./images/icons/download.png" alt="download"></a></span>"#,
            )
        );
    }
}

mod image_mode_attributes {
    use crate::tests::prelude::*;

    #[test]
    fn alt_and_width() {
        verifies!(
            r#"
== Icon macro attributes (image mode only)

The attributes listed below only apply when using the image icon mode.

`alt`::
The alternative text on the `<img>` element (HTML output) or text of `<inlinemediaobject>` element (DocBook output)

`width`::
The width applied to the image.

For example, here's how to control the icon alt text and width when using the image icon mode:

[source]
----
icon:tags[Tags,width=16] ruby, asciidoctor
----

"#
        );

        // The `alt` named attribute sets the `<img>` alt text in image mode.
        let doc = Parser::default().parse(":icons: image\n\nicon:tags[alt=Tags]");
        assert_eq!(
            rendered_paragraphs(&doc).join("\n"),
            r#"<span class="icon"><img src="./images/icons/tags.png" alt="Tags"></span>"#
        );

        // The `width` named attribute sets the `<img>` width.
        let doc = Parser::default().parse(":icons: image\n\nicon:tags[width=16]");
        assert_eq!(
            rendered_paragraphs(&doc).join("\n"),
            r#"<span class="icon"><img src="./images/icons/tags.png" alt="tags" width="16"></span>"#
        );

        // NOTE: The icon macro's sole positional attribute is `size` (see the
        // font-mode section below), so in `icon:tags[Tags,width=16]` the leading
        // `Tags` is parsed as `size`, not `alt`. Because `size` has no effect in
        // image mode, the alt text falls back to the target-derived default
        // (`tags`) while `width=16` applies. To set the alt text explicitly, use
        // the named `alt` attribute as shown above.
        let doc =
            Parser::default().parse(":icons: image\n\nicon:tags[Tags,width=16] ruby, asciidoctor");
        assert_eq!(
            rendered_paragraphs(&doc).join("\n"),
            r#"<span class="icon"><img src="./images/icons/tags.png" alt="tags" width="16"></span> ruby, asciidoctor"#
        );
    }

    non_normative!(
        r#"
The icon macro doesn't support any options to change its physical position (such as alignment).

"#
    );
}

mod font_mode_attributes {
    use crate::tests::prelude::*;

    #[test]
    fn size_rotate_flip() {
        verifies!(
            r#"
== Icon macro attributes (font mode only)

The icon macro has a few attributes that modify the size and orientation of a font-based icon.
These attributes are only recognized in the font icon mode.

`size`::
First positional attribute; scales the icon; values: `1x` (default), `2x`, `3x`, `4x`, `5x`, `lg`, `fw`

`rotate`::
Rotates the icon; values: `90`, `180`, `270`

`flip`::
Flips the icon; values: `horizontal`, `vertical`

"#
        );

        // `size` scales the glyph via a `fa-<size>` class.
        let doc = Parser::default().parse(":icons: font\n\nicon:heart[2x]");
        assert_eq!(
            rendered_paragraphs(&doc).join("\n"),
            r#"<span class="icon"><i class="fa fa-heart fa-2x"></i></span>"#
        );

        // `rotate` rotates the glyph via a `fa-rotate-<deg>` class.
        let doc = Parser::default().parse(":icons: font\n\nicon:shield[rotate=90]");
        assert_eq!(
            rendered_paragraphs(&doc).join("\n"),
            r#"<span class="icon"><i class="fa fa-shield fa-rotate-90"></i></span>"#
        );

        // `flip` flips the glyph via a `fa-flip-<dir>` class.
        let doc = Parser::default().parse(":icons: font\n\nicon:shield[flip=vertical]");
        assert_eq!(
            rendered_paragraphs(&doc).join("\n"),
            r#"<span class="icon"><i class="fa fa-shield fa-flip-vertical"></i></span>"#
        );
    }

    #[test]
    fn size() {
        verifies!(
            r#"
=== Size

To make the icon twice the size as the default, enter `2x` inside the square brackets.

[source]
----
icon:heart[2x]
----

or

[source]
----
icon:heart[size=2x]
----

"#
        );

        // The positional form (`2x`) and the named form (`size=2x`) are
        // equivalent, both adding the `fa-2x` class.
        let expected = r#"<span class="icon"><i class="fa fa-heart fa-2x"></i></span>"#;

        let doc = Parser::default().parse(":icons: font\n\nicon:heart[2x]");
        assert_eq!(rendered_paragraphs(&doc).join("\n"), expected);

        let doc = Parser::default().parse(":icons: font\n\nicon:heart[size=2x]");
        assert_eq!(rendered_paragraphs(&doc).join("\n"), expected);
    }

    #[test]
    fn fixed_width() {
        verifies!(
            r#"
[TIP]
====
If you want to line up icons so that you can use them as bullets in a list, use the `fw` size as follows:

----
[%hardbreaks]
icon:bolt[fw] bolt
icon:heart[fw] heart
----
====

"#
        );

        // The `fw` (fixed-width) size adds the `fa-fw` class.
        let doc = Parser::default().parse(":icons: font\n\nicon:bolt[fw] bolt");
        assert_eq!(
            rendered_paragraphs(&doc).join("\n"),
            r#"<span class="icon"><i class="fa fa-bolt fa-fw"></i></span> bolt"#
        );

        let doc = Parser::default().parse(":icons: font\n\nicon:heart[fw] heart");
        assert_eq!(
            rendered_paragraphs(&doc).join("\n"),
            r#"<span class="icon"><i class="fa fa-heart fa-fw"></i></span> heart"#
        );
    }

    #[test]
    fn rotate_and_flip() {
        verifies!(
            r#"
=== Rotate and flip

To rotate and flip an icon, specify these options using named attributes:

[source]
----
icon:shield[rotate=90, flip=vertical]
----
"#
        );

        // When both `rotate` and `flip` are supplied, `flip` takes precedence
        // (matching Asciidoctor, which selects the flip class and ignores the
        // rotate class).
        let doc = Parser::default().parse(":icons: font\n\nicon:shield[rotate=90, flip=vertical]");
        assert_eq!(
            rendered_paragraphs(&doc).join("\n"),
            r#"<span class="icon"><i class="fa fa-shield fa-flip-vertical"></i></span>"#
        );
    }
}
