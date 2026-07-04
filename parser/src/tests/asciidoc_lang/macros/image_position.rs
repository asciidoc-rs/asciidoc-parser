use crate::tests::prelude::*;

track_file!("ref/asciidoc-lang/docs/modules/macros/pages/image-position.adoc");

non_normative!(
    r#"
= Position and Frame Images
:y: Yes
:n: No

Images are a great way to enhance the text, whether to illustrate an idea, show rather than tell, or just help the reader connect with the text.

Out of the box, images and text behave like oil and water.
Images don't like to share space with text.
They are kind of "`pushy`" about it.
That's why we focused on tuning the controls in the image macros so you can get the images and the text to flow together.

There are two approaches you can take when positioning your images:

. Named attributes
. Roles

"#
);

mod positioning_attributes {
    use crate::{blocks::MediaType, tests::prelude::*};

    non_normative!(
        r#"
== Positioning attributes

AsciiDoc supports the `align` attribute on block images to align the image within the block (e.g., left, right or center).
The named attribute `float` can be applied to both the block and inline image macros.
Float pulls the image to one side of the page or the other and wraps block or inline content around it, respectively.

"#
    );

    #[test]
    fn floating_block_image() {
        verifies!(
            r#"
Here's an example of a floating block image.
The paragraphs or other blocks that follow the image will float up into the available space next to the image.
The image will also be positioned horizontally in the center of the image block.

.A block image pulled to the right and centered within the block
[source]
----
include::example$image.adoc[tag=float]
----

"#
        );

        let doc = Parser::default()
            .parse(r#"image::tiger.png[Tiger,200,200,float="right",align="center"]"#);

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
                blocks: &[Block::Media(MediaBlock {
                    type_: MediaType::Image,
                    target: Span {
                        data: "tiger.png",
                        line: 1,
                        col: 8,
                        offset: 7,
                    },
                    macro_attrlist: Attrlist {
                        attributes: &[
                            ElementAttribute {
                                name: None,
                                value: "Tiger",
                                shorthand_items: &["Tiger"],
                            },
                            ElementAttribute {
                                name: None,
                                value: "200",
                                shorthand_items: &[],
                            },
                            ElementAttribute {
                                name: None,
                                value: "200",
                                shorthand_items: &[],
                            },
                            ElementAttribute {
                                name: Some("float",),
                                value: "right",
                                shorthand_items: &[],
                            },
                            ElementAttribute {
                                name: Some("align",),
                                value: "center",
                                shorthand_items: &[],
                            },
                        ],
                        anchor: None,
                        source: Span {
                            data: "Tiger,200,200,float=\"right\",align=\"center\"",
                            line: 1,
                            col: 18,
                            offset: 17,
                        },
                    },
                    source: Span {
                        data: "image::tiger.png[Tiger,200,200,float=\"right\",align=\"center\"]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },),],
                source: Span {
                    data: "image::tiger.png[Tiger,200,200,float=\"right\",align=\"center\"]",
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

    #[test]
    fn floating_inline_image() {
        verifies!(
            r#"
Here's an example of a floating inline image.
The image will float into the upper-right corner of the paragraph text.

.An inline image pulled to the right of the paragraph text
[source]
----
include::example$image.adoc[tag=in-float]
----

"#
        );

        let doc = Parser::default().parse(
            "image:linux.png[Linux,150,150,float=\"right\"]\nYou can find Linux everywhere these days!",
        );

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
                            data: "image:linux.png[Linux,150,150,float=\"right\"]\nYou can find Linux everywhere these days!",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<span class=\"image right\"><img src=\"linux.png\" alt=\"Linux\" width=\"150\" height=\"150\"></span>\nYou can find Linux everywhere these days!",
                    },
                    source: Span {
                        data: "image:linux.png[Linux,150,150,float=\"right\"]\nYou can find Linux everywhere these days!",
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
                    data: "image:linux.png[Linux,150,150,float=\"right\"]\nYou can find Linux everywhere these days!",
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

    non_normative!(
        r#"
When you use the named attributes, CSS gets added inline (e.g., `style="float: left"`).
That's bad practice because it can make the page harder to style when you want to customize the theme.
It's far better to use CSS classes for these sorts of things, which map to roles in AsciiDoc terminology.

"#
    );
}

mod positioning_roles {
    use crate::{blocks::MediaType, tests::prelude::*};

    non_normative!(
        r#"
== Positioning roles

Here are the examples from above, now configured to use roles that map to CSS classes in the default Asciidoctor stylesheet:

"#
    );

    #[test]
    fn floating_block_image() {
        verifies!(
            r#"
.Block image macro using positioning roles
[source]
----
include::example$image.adoc[tag=role]
----

"#
        );

        let doc = Parser::default().parse("[.right.text-center]\nimage::tiger.png[Tiger,200,200]");

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
                blocks: &[Block::Media(MediaBlock {
                    type_: MediaType::Image,
                    target: Span {
                        data: "tiger.png",
                        line: 2,
                        col: 8,
                        offset: 28,
                    },
                    macro_attrlist: Attrlist {
                        attributes: &[
                            ElementAttribute {
                                name: None,
                                value: "Tiger",
                                shorthand_items: &["Tiger",],
                            },
                            ElementAttribute {
                                name: None,
                                value: "200",
                                shorthand_items: &[],
                            },
                            ElementAttribute {
                                name: None,
                                value: "200",
                                shorthand_items: &[],
                            },
                        ],
                        anchor: None,
                        source: Span {
                            data: "Tiger,200,200",
                            line: 2,
                            col: 18,
                            offset: 38,
                        },
                    },
                    source: Span {
                        data: "[.right.text-center]\nimage::tiger.png[Tiger,200,200]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: None,
                            value: ".right.text-center",
                            shorthand_items: &[".right", ".text-center"],
                        },],
                        anchor: None,
                        source: Span {
                            data: ".right.text-center",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                },),],
                source: Span {
                    data: "[.right.text-center]\nimage::tiger.png[Tiger,200,200]",
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

    #[test]
    fn floating_inline_image() {
        verifies!(
            r#"
.Inline image macro using positioning role
[source]
----
include::example$image.adoc[tag=in-role]
----

"#
        );

        let doc = Parser::default()
            .parse(r#"image:sunset.jpg[Sunset,150,150,role=right] What a beautiful sunset!"#);

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
                            data: "image:sunset.jpg[Sunset,150,150,role=right] What a beautiful sunset!",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "<span class=\"image right\"><img src=\"sunset.jpg\" alt=\"Sunset\" width=\"150\" height=\"150\"></span> What a beautiful sunset!",
                    },
                    source: Span {
                        data: "image:sunset.jpg[Sunset,150,150,role=right] What a beautiful sunset!",
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
                    data: "image:sunset.jpg[Sunset,150,150,role=right] What a beautiful sunset!",
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

    non_normative!(
        r#"
The following table lists all the roles available out of the box for positioning images.

.Roles for positioning images
[cols="1h,5*^"]
|===
|{empty} 2+|Float 3+|Align

|Role
|left
|right
|text-left
|text-right
|text-center

|Block Image
|{y}
|{y}
|{y}
|{y}
|{y}

|Inline Image
|{y}
|{y}
|{n}
|{n}
|{n}
|===

Merely setting the float direction on an image is not sufficient for proper positioning.
That's because, by default, no space is left between the image and the text.
To alleviate this problem, we've added sensible margins to images that use either the positioning named attributes or roles.

If you want to customize the image styles, perhaps to customize the margins, you can provide your own additions to the stylesheet (either by using your own stylesheet that builds on the default stylesheet or by adding the styles to a docinfo file).

WARNING: The shorthand syntax for a role (`.`) can not yet be used with image styles.

"#
    );
}

mod framing_roles {
    use crate::tests::prelude::*;

    #[test]
    fn thumb_role() {
        verifies!(
            r#"
== Framing roles

It's common to frame the image in a border to further offset it from the text.
You can style any block or inline image to appear as a thumbnail using the `thumb` role (or `th` for short).

NOTE: The `thumb` role doesn't alter the dimensions of the image.
For that, you need to assign the image a height and width.

Here's a common example for adding an image to a blog post.
The image floats to the right and is framed to make it stand out more from the text.

[source]
----
include::example$image.adoc[tag=frame]
----

////
====
image:logo.png[role="related thumb right"] Here's text that will wrap around the image to the left.
====
////

Notice we added the `related` role to the image.
This role isn't technically required, but it gives the image semantic meaning.

"#
        );

        // The `thumb`/`related`/`right` roles become CSS classes on the inline
        // image's wrapping `<span>` (in the order they were written, after the
        // implicit `image` class).
        let doc = Parser::default().parse(
            r#"image:logo.png[role="related thumb right"] Here's text that will wrap around the image to the left."#,
        );

        assert_eq!(
            rendered_paragraphs(&doc),
            vec![
                r#"<span class="image related thumb right"><img src="logo.png" alt="logo"></span> Here&#8217;s text that will wrap around the image to the left."#
                    .to_string()
            ]
        );
    }
}

mod control_the_float {
    // This section describes the browser layout behavior of the `float-group`
    // role, which the parser records as an ordinary role on the enclosing block.
    // No parser-specific behavior is normative here (block images are not
    // rendered to HTML by this crate), so the section is covered as
    // non-normative.
    use crate::tests::prelude::*;

    non_normative!(
        r#"
[#control-float]
== Control the float

When you start floating images, you may discover that too much content is floating around the image.
What you need is a way to clear the float.
That is provided using another role, `float-group`.

Let's assume that we've floated two images so that they are positioned next to each other and we want the next paragraph to appear below them.

[source]
----
[.left]
.Image A
image::a.png[A,240,180]

[.left]
.Image B
image::b.png[B,240,180,title=Image B]

Text below images.
----

When this example is converted, then viewed in a browser, the paragraph text appears to the right of the images.
To fix this behavior, you just need to "`group`" the images together in a block with self-contained floats.
Here's how it's done:

[source]
----
[.float-group]
--
[.left]
.Image A
image::a.png[A,240,180]

[.left]
.Image B
image::b.png[B,240,180]
--

Text below images.
----

This time, the text will appear below the images where we want it.
"#
    );
}
