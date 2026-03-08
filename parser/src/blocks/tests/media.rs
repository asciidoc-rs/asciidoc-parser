use std::ops::Deref;

use crate::{
    HasSpan,
    blocks::{ContentModel, IsBlock, MediaType, SimpleBlockStyle},
    content::SubstitutionGroup,
    tests::prelude::*,
    warnings::WarningType,
};

// NOTE: The "error" cases from the MediaBlock parser test suite are not
// necessarily error cases here because we can reparse as SimpleBlock.

#[test]
fn err_inline_syntax() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("foo:bar[]"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(
        mi.item,
        Block::Simple(SimpleBlock {
            content: Content {
                original: Span {
                    data: "foo:bar[]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "foo:bar[]",
            },
            source: Span {
                data: "foo:bar[]",
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
        }),
    );

    assert_eq!(
        mi.item.span(),
        Span {
            data: "foo:bar[]",
            line: 1,
            col: 1,
            offset: 0,
        }
    );

    assert_eq!(
        mi.after,
        Span {
            data: "",
            line: 1,
            col: 10,
            offset: 9
        }
    );
}

#[test]
fn err_no_attr_list() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("image::bar"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(
        mi.item,
        Block::Simple(SimpleBlock {
            content: Content {
                original: Span {
                    data: "image::bar",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "image::bar",
            },
            source: Span {
                data: "image::bar",
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
        }),
    );

    assert_eq!(
        mi.item.span(),
        Span {
            data: "image::bar",
            line: 1,
            col: 1,
            offset: 0,
        }
    );

    assert_eq!(
        mi.after,
        Span {
            data: "",
            line: 1,
            col: 11,
            offset: 10
        }
    );
}

#[test]
fn err_attr_list_not_closed() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("image::bar[blah"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(
        mi.item,
        Block::Simple(SimpleBlock {
            content: Content {
                original: Span {
                    data: "image::bar[blah",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "image::bar[blah",
            },
            source: Span {
                data: "image::bar[blah",
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
        })
    );

    assert_eq!(
        mi.item.span(),
        Span {
            data: "image::bar[blah",
            line: 1,
            col: 1,
            offset: 0,
        }
    );

    assert_eq!(
        mi.after,
        Span {
            data: "",
            line: 1,
            col: 16,
            offset: 15
        }
    );
}

#[test]
fn err_unexpected_after_attr_list() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("image::bar[blah]bonus"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(
        mi.item,
        Block::Simple(SimpleBlock {
            content: Content {
                original: Span {
                    data: "image::bar[blah]bonus",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "image::bar[blah]bonus",
            },
            source: Span {
                data: "image::bar[blah]bonus",
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
        })
    );

    assert_eq!(
        mi.item.span(),
        Span {
            data: "image::bar[blah]bonus",
            line: 1,
            col: 1,
            offset: 0,
        }
    );

    assert_eq!(
        mi.after,
        Span {
            data: "",
            line: 1,
            col: 22,
            offset: 21,
        }
    );
}

#[test]
fn rejects_image_with_no_target() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("image::[]"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(
        mi.item,
        Block::Simple(SimpleBlock {
            content: Content {
                original: Span {
                    data: "image::[]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "image::[]",
            },
            source: Span {
                data: "image::[]",
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
        },)
    );

    assert_eq!(
        mi.item.span(),
        Span {
            data: "image::[]",
            line: 1,
            col: 1,
            offset: 0,
        }
    );

    assert_eq!(
        mi.after,
        Span {
            data: "",
            line: 1,
            col: 10,
            offset: 9
        }
    );
}

#[test]
fn has_target() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("image::bar[]"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(
        mi.item,
        Block::Media(MediaBlock {
            type_: MediaType::Image,
            target: Span {
                data: "bar",
                line: 1,
                col: 8,
                offset: 7,
            },
            macro_attrlist: Attrlist {
                attributes: &[],
                anchor: None,
                source: Span {
                    data: "",
                    line: 1,
                    col: 12,
                    offset: 11,
                }
            },
            source: Span {
                data: "image::bar[]",
                line: 1,
                col: 1,
                offset: 0,
            },
            title_source: None,
            title: None,
            anchor: None,
            anchor_reftext: None,
            attrlist: None,
        })
    );

    assert_eq!(mi.item.content_model(), ContentModel::Empty);
    assert!(mi.item.rendered_content().is_none());
    assert_eq!(mi.item.raw_context().deref(), "image");
    assert!(mi.item.nested_blocks().next().is_none());
    assert!(mi.item.title_source().is_none());
    assert!(mi.item.title().is_none());
    assert!(mi.item.anchor().is_none());
    assert!(mi.item.anchor_reftext().is_none());
    assert!(mi.item.attrlist().is_none());
    assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

    assert_eq!(
        mi.item.span(),
        Span {
            data: "image::bar[]",
            line: 1,
            col: 1,
            offset: 0,
        }
    );

    assert_eq!(
        mi.after,
        Span {
            data: "",
            line: 1,
            col: 13,
            offset: 12
        }
    );
}

#[test]
fn has_target_and_macro_attrlist() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("image::bar[blah]"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(
        mi.item,
        Block::Media(MediaBlock {
            type_: MediaType::Image,
            target: Span {
                data: "bar",
                line: 1,
                col: 8,
                offset: 7,
            },
            macro_attrlist: Attrlist {
                attributes: &[ElementAttribute {
                    name: None,
                    shorthand_items: &["blah"],
                    value: "blah"
                }],
                anchor: None,
                source: Span {
                    data: "blah",
                    line: 1,
                    col: 12,
                    offset: 11,
                }
            },
            source: Span {
                data: "image::bar[blah]",
                line: 1,
                col: 1,
                offset: 0,
            },
            title_source: None,
            title: None,
            anchor: None,
            anchor_reftext: None,
            attrlist: None,
        })
    );

    assert_eq!(
        mi.item.span(),
        Span {
            data: "image::bar[blah]",
            line: 1,
            col: 1,
            offset: 0,
        }
    );

    assert_eq!(
        mi.after,
        Span {
            data: "",
            line: 1,
            col: 17,
            offset: 16
        }
    );
}

#[test]
fn warn_macro_attrlist_has_extra_comma() {
    let mut parser = Parser::default();

    let maw = crate::blocks::Block::parse(
        crate::Span::new("image::bar[alt=Sunset,width=300,,height=400]"),
        &mut parser,
    );

    let mi = maw.item.as_ref().unwrap().clone();

    assert_eq!(
        mi.item,
        Block::Media(MediaBlock {
            type_: MediaType::Image,
            target: Span {
                data: "bar",
                line: 1,
                col: 8,
                offset: 7,
            },
            macro_attrlist: Attrlist {
                attributes: &[
                    ElementAttribute {
                        name: Some("alt"),
                        shorthand_items: &[],
                        value: "Sunset"
                    },
                    ElementAttribute {
                        name: Some("width"),
                        shorthand_items: &[],
                        value: "300"
                    },
                    ElementAttribute {
                        name: Some("height"),
                        shorthand_items: &[],
                        value: "400"
                    }
                ],
                anchor: None,
                source: Span {
                    data: "alt=Sunset,width=300,,height=400",
                    line: 1,
                    col: 12,
                    offset: 11,
                }
            },
            source: Span {
                data: "image::bar[alt=Sunset,width=300,,height=400]",
                line: 1,
                col: 1,
                offset: 0,
            },
            title_source: None,
            title: None,
            anchor: None,
            anchor_reftext: None,
            attrlist: None,
        })
    );

    assert_eq!(
        mi.item.span(),
        Span {
            data: "image::bar[alt=Sunset,width=300,,height=400]",
            line: 1,
            col: 1,
            offset: 0,
        }
    );

    assert_eq!(
        mi.after,
        Span {
            data: "",
            line: 1,
            col: 45,
            offset: 44
        }
    );
    assert_eq!(
        maw.warnings,
        vec![Warning {
            source: Span {
                data: "alt=Sunset,width=300,,height=400",
                line: 1,
                col: 12,
                offset: 11,
            },
            warning: WarningType::EmptyAttributeValue,
        }]
    );
}

#[test]
fn has_title() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(
        crate::Span::new(".macro title\nimage::bar[]\n"),
        &mut parser,
    )
    .unwrap_if_no_warnings()
    .unwrap();

    assert_eq!(
        mi.item,
        Block::Media(MediaBlock {
            type_: MediaType::Image,
            target: Span {
                data: "bar",
                line: 2,
                col: 8,
                offset: 20,
            },
            macro_attrlist: Attrlist {
                attributes: &[],
                anchor: None,
                source: Span {
                    data: "",
                    line: 2,
                    col: 12,
                    offset: 24,
                }
            },
            source: Span {
                data: ".macro title\nimage::bar[]",
                line: 1,
                col: 1,
                offset: 0,
            },
            title_source: Some(Span {
                data: "macro title",
                line: 1,
                col: 2,
                offset: 1,
            },),
            title: Some("macro title"),
            anchor: None,
            anchor_reftext: None,
            attrlist: None,
        })
    );

    assert_eq!(
        mi.item.span(),
        Span {
            data: ".macro title\nimage::bar[]",
            line: 1,
            col: 1,
            offset: 0,
        }
    );

    assert_eq!(
        mi.after,
        Span {
            data: "",
            line: 3,
            col: 1,
            offset: 26
        }
    );
}
