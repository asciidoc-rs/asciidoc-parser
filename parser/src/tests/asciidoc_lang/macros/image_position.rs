use crate::tests::prelude::*;

track_file!("docs/modules/macros/pages/image-position.adoc");

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
    use crate::{
        Parser,
        blocks::{MediaType, SimpleBlockStyle},
        tests::prelude::*,
    };

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
    use crate::{
        Parser,
        blocks::{MediaType, SimpleBlockStyle},
        tests::prelude::*,
    };

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
}

// TO DO: Test all of the roles for positioning images.

// TO DO: Framing roles and float controls.
