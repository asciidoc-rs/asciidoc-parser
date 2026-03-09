use std::ops::Deref;

use crate::{
    blocks::{ContentModel, MediaType},
    tests::prelude::*,
    warnings::MatchAndWarnings,
};

#[test]
fn err_missing_space_before_title() {
    let mut parser = Parser::default();

    let MatchAndWarnings { warnings, item } =
        crate::blocks::Block::parse(crate::Span::new("=blah blah"), &mut parser);

    let mi = item.unwrap();

    assert_eq!(
        mi.item,
        Block::Simple(SimpleBlock {
            content: Content {
                original: Span {
                    data: "=blah blah",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "=blah blah",
            },
            source: Span {
                data: "=blah blah",
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
            data: "=blah blah",
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

    assert_eq!(
        warnings,
        [Warning {
            source: Span {
                data: "=blah blah",
                line: 1,
                col: 1,
                offset: 0,
            },
            warning: WarningType::Level0SectionHeadingNotSupported,
        },]
    );
}

#[test]
fn simplest_section_block() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("== Section Title"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(mi.item.content_model(), ContentModel::Compound);
    assert!(mi.item.rendered_content().is_none());
    assert_eq!(mi.item.raw_context().deref(), "section");
    assert_eq!(mi.item.resolved_context().deref(), "section");
    assert!(mi.item.declared_style().is_none());
    assert!(mi.item.id().is_none());
    assert!(mi.item.roles().is_empty());
    assert!(mi.item.options().is_empty());
    assert!(mi.item.title_source().is_none());
    assert!(mi.item.title().is_none());
    assert!(mi.item.anchor().is_none());
    assert!(mi.item.anchor_reftext().is_none());
    assert!(mi.item.attrlist().is_none());
    assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

    assert_eq!(
        mi.item,
        Block::Section(SectionBlock {
            level: 1,
            section_title: Content {
                original: Span {
                    data: "Section Title",
                    line: 1,
                    col: 4,
                    offset: 3,
                },
                rendered: "Section Title",
            },
            blocks: &[],
            source: Span {
                data: "== Section Title",
                line: 1,
                col: 1,
                offset: 0,
            },
            title_source: None,
            title: None,
            anchor: None,
            anchor_reftext: None,
            attrlist: None,
            section_type: SectionType::Normal,
            section_id: Some("_section_title"),
            section_number: None,
        })
    );

    assert_eq!(mi.item.nested_blocks().next(), None);

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
fn has_child_block() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("== Section Title\n\nabc"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(mi.item.content_model(), ContentModel::Compound);
    assert!(mi.item.rendered_content().is_none());
    assert_eq!(mi.item.raw_context().deref(), "section");
    assert_eq!(mi.item.resolved_context().deref(), "section");
    assert!(mi.item.declared_style().is_none());
    assert!(mi.item.id().is_none());
    assert!(mi.item.roles().is_empty());
    assert!(mi.item.options().is_empty());
    assert!(mi.item.title_source().is_none());
    assert!(mi.item.title().is_none());
    assert!(mi.item.anchor().is_none());
    assert!(mi.item.anchor_reftext().is_none());
    assert!(mi.item.attrlist().is_none());
    assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

    assert_eq!(
        mi.item,
        Block::Section(SectionBlock {
            level: 1,
            section_title: Content {
                original: Span {
                    data: "Section Title",
                    line: 1,
                    col: 4,
                    offset: 3,
                },
                rendered: "Section Title",
            },
            blocks: &[Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "abc",
                        line: 3,
                        col: 1,
                        offset: 18,
                    },
                    rendered: "abc",
                },
                source: Span {
                    data: "abc",
                    line: 3,
                    col: 1,
                    offset: 18,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            })],
            source: Span {
                data: "== Section Title\n\nabc",
                line: 1,
                col: 1,
                offset: 0,
            },
            title_source: None,
            title: None,
            anchor: None,
            anchor_reftext: None,
            attrlist: None,
            section_type: SectionType::Normal,
            section_id: Some("_section_title"),
            section_number: None,
        })
    );

    let mut nested_blocks = mi.item.nested_blocks();

    assert_eq!(
        nested_blocks.next().unwrap(),
        &Block::Simple(SimpleBlock {
            content: Content {
                original: Span {
                    data: "abc",
                    line: 3,
                    col: 1,
                    offset: 18,
                },
                rendered: "abc",
            },
            source: Span {
                data: "abc",
                line: 3,
                col: 1,
                offset: 18,
            },
            style: SimpleBlockStyle::Paragraph,
            title_source: None,
            title: None,
            anchor: None,
            anchor_reftext: None,
            attrlist: None,
        })
    );

    assert_eq!(nested_blocks.next(), None);

    assert_eq!(
        mi.item.span(),
        Span {
            data: "== Section Title\n\nabc",
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
            col: 4,
            offset: 21
        }
    );
}

#[test]
fn title() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(
        crate::Span::new(".other section title\n== Section Title\n\nabc"),
        &mut parser,
    )
    .unwrap_if_no_warnings()
    .unwrap();

    assert_eq!(mi.item.content_model(), ContentModel::Compound);
    assert!(mi.item.rendered_content().is_none());
    assert_eq!(mi.item.raw_context().deref(), "section");
    assert_eq!(mi.item.resolved_context().deref(), "section");
    assert!(mi.item.declared_style().is_none());
    assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

    assert_eq!(
        mi.item,
        Block::Section(SectionBlock {
            level: 1,
            section_title: Content {
                original: Span {
                    data: "Section Title",
                    line: 2,
                    col: 4,
                    offset: 24,
                },
                rendered: "Section Title",
            },
            blocks: &[Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "abc",
                        line: 4,
                        col: 1,
                        offset: 39,
                    },
                    rendered: "abc",
                },
                source: Span {
                    data: "abc",
                    line: 4,
                    col: 1,
                    offset: 39,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            })],
            source: Span {
                data: ".other section title\n== Section Title\n\nabc",
                line: 1,
                col: 1,
                offset: 0,
            },
            title_source: Some(Span {
                data: "other section title",
                line: 1,
                col: 2,
                offset: 1,
            },),
            title: Some("other section title"),
            anchor: None,
            anchor_reftext: None,
            attrlist: None,
            section_type: SectionType::Normal,
            section_id: Some("_section_title"),
            section_number: None,
        })
    );

    assert!(mi.item.id().is_none());
    assert!(mi.item.roles().is_empty());
    assert!(mi.item.options().is_empty());

    assert_eq!(
        mi.item.title_source().unwrap(),
        Span {
            data: "other section title",
            line: 1,
            col: 2,
            offset: 1,
        }
    );

    assert_eq!(
        mi.item.title_source().unwrap().data(),
        "other section title"
    );
    assert_eq!(mi.item.title().unwrap(), "other section title");

    assert!(mi.item.anchor().is_none());
    assert!(mi.item.anchor_reftext().is_none());
    assert!(mi.item.attrlist().is_none());

    let mut nested_blocks = mi.item.nested_blocks();

    assert_eq!(
        nested_blocks.next().unwrap(),
        &Block::Simple(SimpleBlock {
            content: Content {
                original: Span {
                    data: "abc",
                    line: 4,
                    col: 1,
                    offset: 39,
                },
                rendered: "abc",
            },
            source: Span {
                data: "abc",
                line: 4,
                col: 1,
                offset: 39,
            },
            style: SimpleBlockStyle::Paragraph,
            title_source: None,
            title: None,
            anchor: None,
            anchor_reftext: None,
            attrlist: None,
        })
    );

    assert_eq!(nested_blocks.next(), None);

    assert_eq!(
        mi.item.span(),
        Span {
            data: ".other section title\n== Section Title\n\nabc",
            line: 1,
            col: 1,
            offset: 0,
        }
    );

    assert_eq!(
        mi.after,
        Span {
            data: "",
            line: 4,
            col: 4,
            offset: 42
        }
    );
}

#[test]
fn warn_child_attrlist_has_extra_comma() {
    let mut parser = Parser::default();

    let maw = crate::blocks::Block::parse(
        crate::Span::new("== Section Title\n\nimage::bar[alt=Sunset,width=300,,height=400]"),
        &mut parser,
    );

    let mi = maw.item.as_ref().unwrap().clone();

    assert_eq!(
        mi.item,
        Block::Section(SectionBlock {
            level: 1,
            section_title: Content {
                original: Span {
                    data: "Section Title",
                    line: 1,
                    col: 4,
                    offset: 3,
                },
                rendered: "Section Title",
            },
            blocks: &[Block::Media(MediaBlock {
                type_: MediaType::Image,
                target: Span {
                    data: "bar",
                    line: 3,
                    col: 8,
                    offset: 25,
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
                        line: 3,
                        col: 12,
                        offset: 29,
                    }
                },
                source: Span {
                    data: "image::bar[alt=Sunset,width=300,,height=400]",
                    line: 3,
                    col: 1,
                    offset: 18,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            })],
            source: Span {
                data: "== Section Title\n\nimage::bar[alt=Sunset,width=300,,height=400]",
                line: 1,
                col: 1,
                offset: 0,
            },
            title_source: None,
            title: None,
            anchor: None,
            anchor_reftext: None,
            attrlist: None,
            section_type: SectionType::Normal,
            section_id: Some("_section_title"),
            section_number: None,
        })
    );

    assert_eq!(
        mi.item.span(),
        Span {
            data: "== Section Title\n\nimage::bar[alt=Sunset,width=300,,height=400]",
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
            col: 45,
            offset: 62
        }
    );

    assert_eq!(
        maw.warnings,
        vec![Warning {
            source: Span {
                data: "alt=Sunset,width=300,,height=400",
                line: 3,
                col: 12,
                offset: 29,
            },
            warning: WarningType::EmptyAttributeValue,
        }]
    );
}
