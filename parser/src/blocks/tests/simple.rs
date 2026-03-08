use std::ops::Deref;

use crate::{
    blocks::{ContentModel, IsBlock},
    tests::prelude::*,
};

#[test]
fn impl_clone() {
    // Silly test to mark the #[derive(...)] line as covered.
    let mut parser = Parser::default();

    let b1 = crate::blocks::Block::parse(crate::Span::new("abc"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    let b2 = b1.item.clone();
    assert_eq!(b1.item, b2);
}

#[test]
fn err_empty_source() {
    let mut parser = Parser::default();

    assert!(
        crate::blocks::Block::parse(crate::Span::default(), &mut parser)
            .unwrap_if_no_warnings()
            .is_none()
    );
}

#[test]
fn err_only_spaces() {
    let mut parser = Parser::default();

    assert!(
        crate::blocks::Block::parse(crate::Span::new("    "), &mut parser)
            .unwrap_if_no_warnings()
            .is_none()
    );
}

#[test]
fn single_line() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("abc"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(
        mi.item,
        Block::Simple(SimpleBlock {
            content: Content {
                original: Span {
                    data: "abc",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "abc",
            },
            source: Span {
                data: "abc",
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
            data: "abc",
            line: 1,
            col: 1,
            offset: 0,
        }
    );

    assert_eq!(mi.item.content_model(), ContentModel::Simple);
    assert_eq!(mi.item.rendered_content(), Some("abc"));
    assert_eq!(mi.item.raw_context().deref(), "paragraph");
    assert_eq!(mi.item.resolved_context().deref(), "paragraph");
    assert!(mi.item.declared_style().is_none());
    assert_eq!(mi.item.nested_blocks().next(), None);
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
        mi.after,
        Span {
            data: "",
            line: 1,
            col: 4,
            offset: 3
        }
    );
}

#[test]
fn multiple_lines() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("abc\ndef"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(
        mi.item,
        Block::Simple(SimpleBlock {
            content: Content {
                original: Span {
                    data: "abc\ndef",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "abc\ndef",
            },
            source: Span {
                data: "abc\ndef",
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
            data: "abc\ndef",
            line: 1,
            col: 1,
            offset: 0,
        }
    );

    assert_eq!(
        mi.after,
        Span {
            data: "",
            line: 2,
            col: 4,
            offset: 7
        }
    );
}

#[test]
fn title() {
    let mut parser = Parser::default();

    let mi =
        crate::blocks::Block::parse(crate::Span::new(".simple block\nabc\ndef\n"), &mut parser)
            .unwrap_if_no_warnings()
            .unwrap();

    assert_eq!(
        mi.item,
        Block::Simple(SimpleBlock {
            content: Content {
                original: Span {
                    data: "abc\ndef",
                    line: 2,
                    col: 1,
                    offset: 14,
                },
                rendered: "abc\ndef",
            },
            source: Span {
                data: ".simple block\nabc\ndef",
                line: 1,
                col: 1,
                offset: 0,
            },
            style: SimpleBlockStyle::Paragraph,
            title_source: Some(Span {
                data: "simple block",
                line: 1,
                col: 2,
                offset: 1,
            },),
            title: Some("simple block"),
            anchor: None,
            anchor_reftext: None,
            attrlist: None,
        })
    );
}

#[test]
fn attrlist() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("[sidebar]\nabc\ndef\n"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(
        mi.item,
        Block::Simple(SimpleBlock {
            content: Content {
                original: Span {
                    data: "abc\ndef",
                    line: 2,
                    col: 1,
                    offset: 10,
                },
                rendered: "abc\ndef",
            },
            source: Span {
                data: "[sidebar]\nabc\ndef",
                line: 1,
                col: 1,
                offset: 0,
            },
            style: SimpleBlockStyle::Paragraph,
            title_source: None,
            title: None,
            anchor: None,
            anchor_reftext: None,
            attrlist: Some(Attrlist {
                attributes: &[ElementAttribute {
                    name: None,
                    shorthand_items: &["sidebar"],
                    value: "sidebar"
                },],
                anchor: None,
                source: Span {
                    data: "sidebar",
                    line: 1,
                    col: 2,
                    offset: 1,
                },
            },),
        },)
    );

    assert_eq!(
        mi.item.span(),
        Span {
            data: "[sidebar]\nabc\ndef",
            line: 1,
            col: 1,
            offset: 0,
        }
    );

    assert!(mi.item.anchor().is_none());
    assert!(mi.item.anchor_reftext().is_none());

    assert_eq!(
        mi.item.attrlist().unwrap(),
        Attrlist {
            attributes: &[ElementAttribute {
                name: None,
                shorthand_items: &["sidebar"],
                value: "sidebar"
            },],
            anchor: None,
            source: Span {
                data: "sidebar",
                line: 1,
                col: 2,
                offset: 1,
            },
        }
    );

    assert_eq!(
        mi.after,
        Span {
            data: "",
            line: 4,
            col: 1,
            offset: 18,
        }
    );
}

#[test]
fn title_and_attrlist() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(
        crate::Span::new(".title\n[sidebar]\nabc\ndef\n"),
        &mut parser,
    )
    .unwrap_if_no_warnings()
    .unwrap();

    assert_eq!(
        mi.item,
        Block::Simple(SimpleBlock {
            content: Content {
                original: Span {
                    data: "abc\ndef",
                    line: 3,
                    col: 1,
                    offset: 17,
                },
                rendered: "abc\ndef",
            },
            source: Span {
                data: ".title\n[sidebar]\nabc\ndef",
                line: 1,
                col: 1,
                offset: 0,
            },
            style: SimpleBlockStyle::Paragraph,
            title_source: Some(Span {
                data: "title",
                line: 1,
                col: 2,
                offset: 1,
            },),
            title: Some("title"),
            anchor: None,
            anchor_reftext: None,
            attrlist: Some(Attrlist {
                attributes: &[ElementAttribute {
                    name: None,
                    shorthand_items: &["sidebar"],
                    value: "sidebar"
                },],
                anchor: None,
                source: Span {
                    data: "sidebar",
                    line: 2,
                    col: 2,
                    offset: 8,
                },
            },),
        },)
    );

    assert_eq!(
        mi.item.span(),
        Span {
            data: ".title\n[sidebar]\nabc\ndef",
            line: 1,
            col: 1,
            offset: 0,
        }
    );

    assert!(mi.item.anchor().is_none());
    assert!(mi.item.anchor_reftext().is_none());

    assert_eq!(
        mi.item.attrlist().unwrap(),
        Attrlist {
            attributes: &[ElementAttribute {
                name: None,
                shorthand_items: &["sidebar"],
                value: "sidebar"
            },],
            anchor: None,
            source: Span {
                data: "sidebar",
                line: 2,
                col: 2,
                offset: 8,
            },
        }
    );

    assert_eq!(
        mi.after,
        Span {
            data: "",
            line: 5,
            col: 1,
            offset: 25,
        }
    );
}

#[test]
fn consumes_blank_lines_after() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("abc\n\ndef"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(
        mi.item,
        Block::Simple(SimpleBlock {
            content: Content {
                original: Span {
                    data: "abc",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "abc",
            },
            source: Span {
                data: "abc",
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
            data: "abc",
            line: 1,
            col: 1,
            offset: 0,
        }
    );

    assert_eq!(
        mi.after,
        Span {
            data: "def",
            line: 3,
            col: 1,
            offset: 5
        }
    );
}

#[test]
fn with_block_anchor_only() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(
        crate::Span::new("[[notice]]\nThis paragraph gets a lot of attention.\n"),
        &mut parser,
    )
    .unwrap_if_no_warnings()
    .unwrap();

    assert_eq!(
        mi.item,
        Block::Simple(SimpleBlock {
            content: Content {
                original: Span {
                    data: "This paragraph gets a lot of attention.",
                    line: 2,
                    col: 1,
                    offset: 11,
                },
                rendered: "This paragraph gets a lot of attention.",
            },
            source: Span {
                data: "[[notice]]\nThis paragraph gets a lot of attention.",
                line: 1,
                col: 1,
                offset: 0,
            },
            style: SimpleBlockStyle::Paragraph,
            title_source: None,
            title: None,
            anchor: Some(Span {
                data: "notice",
                line: 1,
                col: 3,
                offset: 2,
            },),
            anchor_reftext: None,
            attrlist: None,
        })
    );

    assert_eq!(
        mi.item.span(),
        Span {
            data: "[[notice]]\nThis paragraph gets a lot of attention.",
            line: 1,
            col: 1,
            offset: 0,
        }
    );

    assert_eq!(mi.item.content_model(), ContentModel::Simple);
    assert_eq!(
        mi.item.rendered_content(),
        Some("This paragraph gets a lot of attention.")
    );
    assert_eq!(mi.item.raw_context().deref(), "paragraph");
    assert_eq!(mi.item.resolved_context().deref(), "paragraph");
    assert!(mi.item.declared_style().is_none());
    assert_eq!(mi.item.nested_blocks().next(), None);
    assert_eq!(mi.item.id().unwrap(), "notice");
    assert!(mi.item.roles().is_empty());
    assert!(mi.item.options().is_empty());
    assert!(mi.item.title_source().is_none());
    assert!(mi.item.title().is_none());
    assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

    assert_eq!(
        mi.item.anchor().unwrap(),
        Span {
            data: "notice",
            line: 1,
            col: 3,
            offset: 2,
        }
    );

    assert!(mi.item.anchor_reftext().is_none());
    assert!(mi.item.attrlist().is_none());

    assert_eq!(
        mi.after,
        Span {
            data: "",
            line: 3,
            col: 1,
            offset: 51
        }
    );
}

#[test]
fn with_block_anchor_trailing_comma() {
    let mut parser = Parser::default();

    let maw = crate::blocks::Block::parse(
        crate::Span::new("[[notice,]]\nThis paragraph gets a lot of attention.\n"),
        &mut parser,
    );

    assert_eq!(
        maw.warnings,
        [Warning {
            source: Span {
                data: "notice,",
                line: 1,
                col: 3,
                offset: 2,
            },
            warning: WarningType::InvalidBlockAnchorName,
        }]
    );

    let mi = maw.item.unwrap();

    assert_eq!(
        mi.item,
        Block::Simple(SimpleBlock {
            content: Content {
                original: Span {
                    data: "[[notice,]]\nThis paragraph gets a lot of attention.",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "[[notice,]]\nThis paragraph gets a lot of attention.",
            },
            source: Span {
                data: "[[notice,]]\nThis paragraph gets a lot of attention.",
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
            data: "[[notice,]]\nThis paragraph gets a lot of attention.",
            line: 1,
            col: 1,
            offset: 0,
        }
    );

    assert_eq!(mi.item.content_model(), ContentModel::Simple);
    assert_eq!(
        mi.item.rendered_content(),
        Some("[[notice,]]\nThis paragraph gets a lot of attention.")
    );
    assert_eq!(mi.item.raw_context().deref(), "paragraph");
    assert_eq!(mi.item.resolved_context().deref(), "paragraph");
    assert!(mi.item.declared_style().is_none());
    assert_eq!(mi.item.nested_blocks().next(), None);
    assert!(mi.item.id().is_none());
    assert!(mi.item.roles().is_empty());
    assert!(mi.item.options().is_empty());
    assert!(mi.item.title_source().is_none());
    assert!(mi.item.title().is_none());
    assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);
    assert!(mi.item.anchor().is_none());
    assert!(mi.item.anchor_reftext().is_none());
    assert!(mi.item.attrlist().is_none());

    assert_eq!(
        mi.after,
        Span {
            data: "",
            line: 3,
            col: 1,
            offset: 52
        }
    );
}

#[test]
fn with_block_anchor_and_reftext() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(
        crate::Span::new("[[notice,See Here!]]\nThis paragraph gets a lot of attention.\n"),
        &mut parser,
    )
    .unwrap_if_no_warnings()
    .unwrap();

    assert_eq!(
        mi.item,
        Block::Simple(SimpleBlock {
            content: Content {
                original: Span {
                    data: "This paragraph gets a lot of attention.",
                    line: 2,
                    col: 1,
                    offset: 21,
                },
                rendered: "This paragraph gets a lot of attention.",
            },
            source: Span {
                data: "[[notice,See Here!]]\nThis paragraph gets a lot of attention.",
                line: 1,
                col: 1,
                offset: 0,
            },
            style: SimpleBlockStyle::Paragraph,
            title_source: None,
            title: None,
            anchor: Some(Span {
                data: "notice",
                line: 1,
                col: 3,
                offset: 2,
            },),
            anchor_reftext: Some(Span {
                data: "See Here!",
                line: 1,
                col: 10,
                offset: 9,
            },),
            attrlist: None,
        })
    );

    assert_eq!(
        mi.item.span(),
        Span {
            data: "[[notice,See Here!]]\nThis paragraph gets a lot of attention.",
            line: 1,
            col: 1,
            offset: 0,
        }
    );

    assert_eq!(mi.item.content_model(), ContentModel::Simple);
    assert_eq!(
        mi.item.rendered_content(),
        Some("This paragraph gets a lot of attention.")
    );
    assert_eq!(mi.item.raw_context().deref(), "paragraph");
    assert_eq!(mi.item.resolved_context().deref(), "paragraph");
    assert!(mi.item.declared_style().is_none());
    assert_eq!(mi.item.nested_blocks().next(), None);
    assert_eq!(mi.item.id().unwrap(), "notice");
    assert!(mi.item.roles().is_empty());
    assert!(mi.item.options().is_empty());
    assert!(mi.item.title_source().is_none());
    assert!(mi.item.title().is_none());
    assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

    assert_eq!(
        mi.item.anchor().unwrap(),
        Span {
            data: "notice",
            line: 1,
            col: 3,
            offset: 2,
        }
    );

    assert_eq!(
        mi.item.anchor_reftext().unwrap(),
        Span {
            data: "See Here!",
            line: 1,
            col: 10,
            offset: 9,
        }
    );

    assert!(mi.item.attrlist().is_none());

    assert_eq!(
        mi.after,
        Span {
            data: "",
            line: 3,
            col: 1,
            offset: 61
        }
    );
}

#[test]
fn err_empty_block_anchor() {
    let mut parser = Parser::default();

    let maw = crate::blocks::Block::parse(
        crate::Span::new("[[]]\nThis paragraph gets a lot of attention.\n"),
        &mut parser,
    );

    assert_eq!(
        maw.warnings,
        vec![Warning {
            source: Span {
                data: "",
                line: 1,
                col: 3,
                offset: 2,
            },
            warning: WarningType::EmptyBlockAnchorName,
        },]
    );

    let mi = maw.item.unwrap();

    assert_eq!(
        mi.item,
        Block::Simple(SimpleBlock {
            content: Content {
                original: Span {
                    data: "[[]]\nThis paragraph gets a lot of attention.",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "[[]]\nThis paragraph gets a lot of attention.",
            },
            source: Span {
                data: "[[]]\nThis paragraph gets a lot of attention.",
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
            data: "[[]]\nThis paragraph gets a lot of attention.",
            line: 1,
            col: 1,
            offset: 0,
        }
    );

    assert_eq!(mi.item.content_model(), ContentModel::Simple);
    assert_eq!(
        mi.item.rendered_content(),
        Some("[[]]\nThis paragraph gets a lot of attention.")
    );
    assert_eq!(mi.item.raw_context().deref(), "paragraph");
    assert_eq!(mi.item.resolved_context().deref(), "paragraph");
    assert!(mi.item.declared_style().is_none());
    assert_eq!(mi.item.nested_blocks().next(), None);
    assert!(mi.item.id().is_none());
    assert!(mi.item.roles().is_empty());
    assert!(mi.item.options().is_empty());
    assert!(mi.item.title_source().is_none());
    assert!(mi.item.title().is_none());
    assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);
    assert!(mi.item.anchor().is_none());
    assert!(mi.item.anchor_reftext().is_none());
    assert!(mi.item.attrlist().is_none());

    assert_eq!(
        mi.after,
        Span {
            data: "",
            line: 3,
            col: 1,
            offset: 45
        }
    );
}

#[test]
fn err_invalid_block_anchor() {
    let mut parser = Parser::default();

    let maw = crate::blocks::Block::parse(
        crate::Span::new("[[3 blind mice]]\nThis paragraph gets a lot of attention.\n"),
        &mut parser,
    );

    assert_eq!(
        maw.warnings,
        vec![Warning {
            source: Span {
                data: "3 blind mice",
                line: 1,
                col: 3,
                offset: 2,
            },
            warning: WarningType::InvalidBlockAnchorName,
        },]
    );

    let mi = maw.item.unwrap();

    assert_eq!(
        mi.item,
        Block::Simple(SimpleBlock {
            content: Content {
                original: Span {
                    data: "[[3 blind mice]]\nThis paragraph gets a lot of attention.",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "[[3 blind mice]]\nThis paragraph gets a lot of attention.",
            },
            source: Span {
                data: "[[3 blind mice]]\nThis paragraph gets a lot of attention.",
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
            data: "[[3 blind mice]]\nThis paragraph gets a lot of attention.",
            line: 1,
            col: 1,
            offset: 0,
        }
    );

    assert_eq!(mi.item.content_model(), ContentModel::Simple);
    assert_eq!(
        mi.item.rendered_content(),
        Some("[[3 blind mice]]\nThis paragraph gets a lot of attention.")
    );
    assert_eq!(mi.item.raw_context().deref(), "paragraph");
    assert_eq!(mi.item.resolved_context().deref(), "paragraph");
    assert!(mi.item.declared_style().is_none());
    assert_eq!(mi.item.nested_blocks().next(), None);
    assert!(mi.item.id().is_none());
    assert!(mi.item.roles().is_empty());
    assert!(mi.item.options().is_empty());
    assert!(mi.item.title_source().is_none());
    assert!(mi.item.title().is_none());
    assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);
    assert!(mi.item.anchor().is_none());
    assert!(mi.item.anchor_reftext().is_none());
    assert!(mi.item.attrlist().is_none());

    assert_eq!(
        mi.after,
        Span {
            data: "",
            line: 3,
            col: 1,
            offset: 57
        }
    );
}

#[test]
fn unterminated_block_anchor() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(
        crate::Span::new("[[notice]\nThis paragraph gets a lot of attention.\n"),
        &mut parser,
    )
    .unwrap_if_no_warnings()
    .unwrap();

    assert_eq!(
        mi.item,
        Block::Simple(SimpleBlock {
            content: Content {
                original: Span {
                    data: "This paragraph gets a lot of attention.",
                    line: 2,
                    col: 1,
                    offset: 10,
                },
                rendered: "This paragraph gets a lot of attention.",
            },
            source: Span {
                data: "[[notice]\nThis paragraph gets a lot of attention.",
                line: 1,
                col: 1,
                offset: 0,
            },
            style: SimpleBlockStyle::Paragraph,
            title_source: None,
            title: None,
            anchor: None,
            anchor_reftext: None,
            attrlist: Some(Attrlist {
                attributes: &[ElementAttribute {
                    name: None,
                    shorthand_items: &["[notice",],
                    value: "[notice"
                },],
                anchor: None,
                source: Span {
                    data: "[notice",
                    line: 1,
                    col: 2,
                    offset: 1,
                },
            },),
        })
    );

    assert_eq!(
        mi.item.span(),
        Span {
            data: "[[notice]\nThis paragraph gets a lot of attention.",
            line: 1,
            col: 1,
            offset: 0,
        }
    );

    assert_eq!(mi.item.content_model(), ContentModel::Simple);
    assert_eq!(
        mi.item.rendered_content(),
        Some("This paragraph gets a lot of attention.")
    );
    assert_eq!(mi.item.raw_context().deref(), "paragraph");
    assert_eq!(mi.item.resolved_context().deref(), "paragraph");
    assert_eq!(mi.item.declared_style().unwrap(), "[notice");
    assert_eq!(mi.item.nested_blocks().next(), None);

    assert!(mi.item.id().is_none());
    assert!(mi.item.roles().is_empty());
    assert!(mi.item.options().is_empty());
    assert!(mi.item.title_source().is_none());
    assert!(mi.item.title().is_none());
    assert!(mi.item.anchor().is_none());
    assert!(mi.item.anchor_reftext().is_none());
    assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

    assert_eq!(
        mi.item.attrlist().unwrap(),
        Attrlist {
            attributes: &[ElementAttribute {
                name: None,
                shorthand_items: &["[notice"],
                value: "[notice"
            },],
            anchor: None,
            source: Span {
                data: "[notice",
                line: 1,
                col: 2,
                offset: 1,
            },
        },
    );

    assert_eq!(
        mi.after,
        Span {
            data: "",
            line: 3,
            col: 1,
            offset: 50
        }
    );
}

// The following tests exercise the `is_section_header` function
// (called from `parse_lines`) via full parse trees. A comment line
// (`// ...`) sets `skipped_comment_line = true`, and the next
// non-empty line is checked by `is_section_header`. If it looks
// like a section header, the paragraph terminates before that line.

#[test]
fn comment_then_asciidoc_level_2_header_terminates_paragraph() {
    // Exercises `is_section_header` for `=== ` (3 equals).
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(
        crate::Span::new("paragraph\n// comment\n=== Section\n\ncontent"),
        &mut parser,
    )
    .unwrap_if_no_warnings()
    .unwrap();

    assert_eq!(
        mi.item,
        Block::Simple(SimpleBlock {
            content: Content {
                original: Span {
                    data: "paragraph\n// comment",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "paragraph",
            },
            source: Span {
                data: "paragraph\n// comment",
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
        mi.after,
        Span {
            data: "=== Section\n\ncontent",
            line: 3,
            col: 1,
            offset: 21,
        }
    );
}

#[test]
fn comment_then_markdown_level_1_header_terminates_paragraph() {
    // Exercises `is_section_header` for `## ` (2 hashes).
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(
        crate::Span::new("paragraph\n// comment\n## Section\n\ncontent"),
        &mut parser,
    )
    .unwrap_if_no_warnings()
    .unwrap();

    assert_eq!(
        mi.item,
        Block::Simple(SimpleBlock {
            content: Content {
                original: Span {
                    data: "paragraph\n// comment",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "paragraph",
            },
            source: Span {
                data: "paragraph\n// comment",
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
        mi.after,
        Span {
            data: "## Section\n\ncontent",
            line: 3,
            col: 1,
            offset: 21,
        }
    );
}

#[test]
fn comment_then_markdown_level_2_header_terminates_paragraph() {
    // Exercises `is_section_header` for `### ` (3 hashes).
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(
        crate::Span::new("paragraph\n// comment\n### Section\n\ncontent"),
        &mut parser,
    )
    .unwrap_if_no_warnings()
    .unwrap();

    assert_eq!(
        mi.item,
        Block::Simple(SimpleBlock {
            content: Content {
                original: Span {
                    data: "paragraph\n// comment",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "paragraph",
            },
            source: Span {
                data: "paragraph\n// comment",
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
        mi.after,
        Span {
            data: "### Section\n\ncontent",
            line: 3,
            col: 1,
            offset: 21,
        }
    );
}

#[test]
fn comment_then_equals_without_space_does_not_terminate_paragraph() {
    // Exercises the `false` return from `is_section_header` when
    // `==` is not followed by a space.
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(
        crate::Span::new("paragraph\n// comment\n==nospace"),
        &mut parser,
    )
    .unwrap_if_no_warnings()
    .unwrap();

    assert_eq!(
        mi.item,
        Block::Simple(SimpleBlock {
            content: Content {
                original: Span {
                    data: "paragraph\n// comment\n==nospace",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "paragraph\n==nospace",
            },
            source: Span {
                data: "paragraph\n// comment\n==nospace",
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
        mi.after,
        Span {
            data: "",
            line: 3,
            col: 10,
            offset: 30,
        }
    );
}

#[test]
fn comment_then_hashes_without_space_does_not_terminate_paragraph() {
    // Exercises the `false` return from `is_section_header` when
    // `##` is not followed by a space.
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(
        crate::Span::new("paragraph\n// comment\n##nospace"),
        &mut parser,
    )
    .unwrap_if_no_warnings()
    .unwrap();

    assert_eq!(
        mi.item,
        Block::Simple(SimpleBlock {
            content: Content {
                original: Span {
                    data: "paragraph\n// comment\n##nospace",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "paragraph\n##nospace",
            },
            source: Span {
                data: "paragraph\n// comment\n##nospace",
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
        mi.after,
        Span {
            data: "",
            line: 3,
            col: 10,
            offset: 30,
        }
    );
}
