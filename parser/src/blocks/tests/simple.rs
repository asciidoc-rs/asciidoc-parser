use std::ops::Deref;

use crate::{blocks::ContentModel, tests::prelude::*};

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
            caption: None,
            number: None,
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
    assert_eq!(mi.item.child_blocks().next(), None);
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
            caption: None,
            number: None,
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
            caption: None,
            number: None,
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
            caption: None,
            number: None,
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
            caption: None,
            number: None,
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
            caption: None,
            number: None,
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
            caption: None,
            number: None,
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
    assert_eq!(mi.item.child_blocks().next(), None);
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
            caption: None,
            number: None,
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
    assert_eq!(mi.item.child_blocks().next(), None);
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
fn with_block_anchor_invalid_id_before_comma() {
    // A block anchor with a reftext (`[[id,reftext]]`) whose id part is not a
    // valid XML name is rejected: the warning points at the id (the text before
    // the comma), and the anchor is not applied to the block.
    let mut parser = Parser::default();

    let maw = crate::blocks::Block::parse(
        crate::Span::new("[[1bad,reftext]]\nThis paragraph gets a lot of attention.\n"),
        &mut parser,
    );

    assert_eq!(
        maw.warnings,
        [Warning {
            source: Span {
                data: "1bad",
                line: 1,
                col: 3,
                offset: 2,
            },
            warning: WarningType::InvalidBlockAnchorName,
        }]
    );

    // The block is still produced; the rejected anchor stays as literal text
    // and no id is registered.
    let mi = maw.item.unwrap();
    assert!(mi.item.anchor().is_none());
    assert!(mi.item.id().is_none());
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
            caption: None,
            number: None,
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
    assert_eq!(mi.item.child_blocks().next(), None);
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
                    data: "This paragraph gets a lot of attention.",
                    line: 2,
                    col: 1,
                    offset: 5,
                },
                rendered: "This paragraph gets a lot of attention.",
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
            caption: None,
            number: None,
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
        Some("This paragraph gets a lot of attention.")
    );
    assert_eq!(mi.item.raw_context().deref(), "paragraph");
    assert_eq!(mi.item.resolved_context().deref(), "paragraph");
    assert!(mi.item.declared_style().is_none());
    assert_eq!(mi.item.child_blocks().next(), None);
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
fn terminal_empty_block_anchor_does_not_spin() {
    // A lone empty `[[]]` anchor at the end of a block scope names nothing and
    // decorates no block, so it is consumed (dropped) and yields no block.
    // Regression: an empty anchor consumed into empty metadata with no block
    // following once returned `NoMatch` on a non-blank source, which the
    // block-collection loop leaves unadvanced – spinning forever.
    let doc = Parser::default().parse("--\nBlock content\n[[]]\n--\n");

    let block = doc.child_blocks().next().unwrap();
    assert_eq!(block.raw_context().deref(), "open");

    // Only the paragraph survives inside the open block; the trailing `[[]]`
    // produces nothing.
    let mut inner = block.child_blocks();
    assert_eq!(
        inner.next().unwrap().rendered_content(),
        Some("Block content")
    );
    assert!(inner.next().is_none());

    assert_eq!(doc.child_blocks().count(), 1);
}

#[test]
fn lone_empty_block_anchor_document_does_not_spin() {
    // An empty `[[]]` anchor as the entire document body is consumed and
    // produces no block (rather than hanging or rendering the literal `[[]]`).
    let doc = Parser::default().parse("[[]]\n");

    assert_eq!(doc.child_blocks().count(), 0);
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
            caption: None,
            number: None,
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
    assert_eq!(mi.item.child_blocks().next(), None);
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
            caption: None,
            number: None,
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
    assert_eq!(mi.item.child_blocks().next(), None);

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
            caption: None,
            number: None,
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
            caption: None,
            number: None,
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
            caption: None,
            number: None,
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
            caption: None,
            number: None,
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
            caption: None,
            number: None,
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

/// A single-line comment (`//`) placed directly above a block-metadata line –
/// an attribute list, an anchor, etc., with no blank line between them – must
/// not detach that metadata from the block it decorates. Regression tests for
/// the case where the first metadata line after the comment was surfaced as
/// paragraph text.
mod comment_directly_above_metadata {
    use crate::tests::prelude::*;

    #[test]
    fn attrlist_before_listing() {
        let doc = Parser::default().parse("= T\n\n// note\n[,ruby]\n----\nputs 1\n----\n");
        let blocks = top_blocks(&doc);

        // The comment is retained as its own (empty) paragraph block.
        assert_eq!(blocks[0].raw_context().as_ref(), "paragraph");
        assert_eq!(blocks[0].span().data(), "// note");

        // The `[,ruby]` metadata attaches to the following listing rather than
        // being consumed as paragraph text.
        assert_eq!(blocks[1].raw_context().as_ref(), "listing");
        assert_eq!(
            blocks[1]
                .attrlist()
                .and_then(|a| a.nth_attribute(2))
                .map(|a| a.value()),
            Some("ruby")
        );
        assert_eq!(doc.warnings().count(), 0);
    }

    #[test]
    fn stem_style_before_passthrough() {
        let doc = Parser::default().parse("= T\n:stem:\n\n// note\n[stem]\n++++\nx = y^2\n++++\n");
        let blocks = top_blocks(&doc);

        assert_eq!(blocks[0].raw_context().as_ref(), "paragraph");
        assert_eq!(blocks[0].span().data(), "// note");

        // The `[stem]` style attaches to the following delimited block.
        assert_eq!(blocks[1].declared_style(), Some("stem"));
        assert_eq!(doc.warnings().count(), 0);
    }

    #[test]
    fn anchor_and_attrlist_both_attach() {
        // Both the anchor directly under the comment and the attribute list
        // after it attach to the listing.
        let doc = Parser::default().parse("= T\n\n// note\n[[an]]\n[,ruby]\n----\nputs 1\n----\n");
        let blocks = top_blocks(&doc);

        assert_eq!(blocks[0].raw_context().as_ref(), "paragraph");
        assert_eq!(blocks[0].span().data(), "// note");

        assert_eq!(blocks[1].raw_context().as_ref(), "listing");
        assert_eq!(blocks[1].id(), Some("an"));
        assert_eq!(
            blocks[1]
                .attrlist()
                .and_then(|a| a.nth_attribute(2))
                .map(|a| a.value()),
            Some("ruby")
        );
    }

    #[test]
    fn title_before_listing() {
        // A `.Title` line directly under the comment attaches as the following
        // block's title rather than being surfaced as paragraph text.
        let doc = Parser::default().parse("= T\n\n// note\n.Title\n----\nputs 1\n----\n");
        let blocks = top_blocks(&doc);

        assert_eq!(blocks[0].raw_context().as_ref(), "paragraph");
        assert_eq!(blocks[0].span().data(), "// note");

        assert_eq!(blocks[1].raw_context().as_ref(), "listing");
        assert_eq!(blocks[1].title(), Some("Title"));
    }

    #[test]
    fn title_before_paragraph() {
        // The title also attaches when the decorated block is itself a
        // paragraph.
        let doc = Parser::default().parse("= T\n\nfirst\n\n// note\n.Title\ntext\n");
        let blocks = top_blocks(&doc);

        // blocks[0] is `first`; blocks[1] is the retained comment.
        assert_eq!(blocks[2].raw_context().as_ref(), "paragraph");
        assert_eq!(blocks[2].title(), Some("Title"));
        assert_eq!(blocks[2].rendered_content(), Some("text"));
    }

    #[test]
    fn bracketed_line_after_comment_is_a_block_boundary() {
        // A bracketed line that is not valid block metadata (a leading space is
        // rejected by attribute-line parsing) is still treated as a block
        // boundary, exactly as it is when it directly follows ordinary
        // paragraph text. The comment is retained as its own block and the
        // bracketed line renders as its own paragraph – the same rendered
        // output the pre-existing merged form produced, just as a distinct
        // block.
        let doc = Parser::default().parse("= T\n\nfirst\n\n// note\n[ foo]\n");
        let blocks = top_blocks(&doc);

        assert_eq!(blocks[1].raw_context().as_ref(), "paragraph");
        assert_eq!(blocks[1].span().data(), "// note");

        assert_eq!(blocks[2].raw_context().as_ref(), "paragraph");
        assert_eq!(blocks[2].rendered_content(), Some("[ foo]"));
    }

    #[test]
    fn bare_table_delimiter_after_comment() {
        // A comment directly above a bare table delimiter opens the table rather
        // than swallowing the whole table as paragraph text.
        let doc = Parser::default().parse("= T\n\n// note\n|===\n|a |b\n|===\n");
        let blocks = top_blocks(&doc);

        assert_eq!(blocks[0].raw_context().as_ref(), "paragraph");
        assert_eq!(blocks[0].span().data(), "// note");

        assert_eq!(blocks[1].raw_context().as_ref(), "table");
    }

    #[test]
    fn plain_text_after_comment_still_merges() {
        // A comment directly above ordinary paragraph text is still absorbed
        // into that single paragraph (the metadata-adjacency stop must not
        // fire for non-metadata content).
        let doc = Parser::default().parse("// note\nHello world\n");
        let blocks = top_blocks(&doc);

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].raw_context().as_ref(), "paragraph");
        assert_eq!(blocks[0].rendered_content(), Some("Hello world"));
    }
}
