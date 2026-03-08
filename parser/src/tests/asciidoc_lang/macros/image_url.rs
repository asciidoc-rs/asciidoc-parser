use crate::tests::prelude::*;

track_file!("docs/modules/macros/pages/image-url.adoc");

non_normative!(
    r#"
= Insert Images from a URL

//(i.e., images with a URL target)
You can reference images served from any URL (e.g., your blog, an image hosting service, your server, etc.) and never have to worry about downloading the images and putting them somewhere locally.

"#
);

mod image_url_targets {
    use crate::{
        blocks::{MediaType, SimpleBlockStyle},
        tests::prelude::*,
    };

    non_normative!(
        r#"
== Image URL targets

Here are a few examples of images that have a URL target:

"#
    );

    #[test]
    fn block_image_with_a_url_target() {
        verifies!(
            r#"
.Block image with a URL target
[source]
----
include::example$image.adoc[tag=url]
----

include::example$image.adoc[tag=url]

"#
        );

        let doc = Parser::default().parse(
            "image::https://upload.wikimedia.org/wikipedia/commons/3/35/Tux.svg[Tux,250,350]",
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
                blocks: &[Block::Media(MediaBlock {
                    type_: MediaType::Image,
                    target: Span {
                        data: "https://upload.wikimedia.org/wikipedia/commons/3/35/Tux.svg",
                        line: 1,
                        col: 8,
                        offset: 7,
                    },
                    macro_attrlist: Attrlist {
                        attributes: &[
                            ElementAttribute {
                                name: None,
                                value: "Tux",
                                shorthand_items: &["Tux"],
                            },
                            ElementAttribute {
                                name: None,
                                value: "250",
                                shorthand_items: &[],
                            },
                            ElementAttribute {
                                name: None,
                                value: "350",
                                shorthand_items: &[],
                            },
                        ],
                        anchor: None,
                        source: Span {
                            data: "Tux,250,350",
                            line: 1,
                            col: 68,
                            offset: 67,
                        },
                    },
                    source: Span {
                        data: "image::https://upload.wikimedia.org/wikipedia/commons/3/35/Tux.svg[Tux,250,350]",
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
                    data: "image::https://upload.wikimedia.org/wikipedia/commons/3/35/Tux.svg[Tux,250,350]",
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
include::partial$image-target.adoc[]

"#
    );

    #[test]
    fn inline_image_with_a_url_target() {
        verifies!(
            r#"
.Inline image with a URL target
[source]
----
include::example$image.adoc[tag=in-url]
----

include::example$image.adoc[tag=in-url]

"#
        );

        let doc = Parser::default().parse("You can find image:https://upload.wikimedia.org/wikipedia/commons/3/35/Tux.svg[Linux,25,35] everywhere these days.");

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
                            data: "You can find image:https://upload.wikimedia.org/wikipedia/commons/3/35/Tux.svg[Linux,25,35] everywhere these days.",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "You can find <span class=\"image\"><img src=\"https://upload.wikimedia.org/wikipedia/commons/3/35/Tux.svg\" alt=\"Linux\" width=\"25\" height=\"35\"></span> everywhere these days.",
                    },
                    source: Span {
                        data: "You can find image:https://upload.wikimedia.org/wikipedia/commons/3/35/Tux.svg[Linux,25,35] everywhere these days.",
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
                    data: "You can find image:https://upload.wikimedia.org/wikipedia/commons/3/35/Tux.svg[Linux,25,35] everywhere these days.",
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

    #[ignore]
    #[test]
    fn using_a_url_as_the_base_url_for_images() {
        to_do_verifies!(
            r#"
NOTE: The value of `imagesdir` is ignored when the image target is a URL.

If you want to avoid typing the URL prefix for every image, and all the images are located on the same server, you can use the `imagesdir` attribute to set the base URL:

.Using a URL as the base URL for images
[source]
----
include::example$image.adoc[tag=base-url]
----

This time, `imagesdir` is used since the image target is not a URL (the value of `imagesdir` just happens to be one).
"#
        );

        let _doc = Parser::default().parse(":imagesdir-old: {imagesdir}\n:imagesdir: https://upload.wikimedia.org/wikipedia/commons\n\nimage::3/35/Tux.svg[Tux,250,350]\n\n:imagesdir: {imagesdir-old}");
    }
}
