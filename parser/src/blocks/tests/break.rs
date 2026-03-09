use std::ops::Deref;

use crate::{
    blocks::{BreakType, ContentModel},
    tests::prelude::*,
};

#[test]
fn err_unknown_break_pattern() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("==="), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    // Unknown pattern becomes a simple block.
    assert_eq!(
        mi.item,
        Block::Simple(SimpleBlock {
            content: Content {
                original: Span {
                    data: "===",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "===",
            },
            source: Span {
                data: "===",
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
            data: "===",
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
            col: 4,
            offset: 3
        }
    );
}

#[test]
fn thematic_break_triple_apostrophe() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("'''"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(
        mi.item,
        Block::Break(Break {
            type_: BreakType::Thematic,
            source: Span {
                data: "'''",
                line: 1,
                col: 1,
                offset: 0,
            },
            title_source: None,
            title: None,
            anchor: None,
            attrlist: None,
        })
    );

    assert_eq!(mi.item.content_model(), ContentModel::Empty);
    assert!(mi.item.rendered_content().is_none());
    assert_eq!(mi.item.raw_context().deref(), "thematic_break");
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
            data: "'''",
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
            col: 4,
            offset: 3
        }
    );
}

#[test]
fn thematic_break_triple_hyphen() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("---"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(
        mi.item,
        Block::Break(Break {
            type_: BreakType::Thematic,
            source: Span {
                data: "---",
                line: 1,
                col: 1,
                offset: 0,
            },
            title_source: None,
            title: None,
            anchor: None,
            attrlist: None,
        })
    );

    assert_eq!(mi.item.content_model(), ContentModel::Empty);
    assert!(mi.item.rendered_content().is_none());
    assert_eq!(mi.item.raw_context().deref(), "thematic_break");
}

#[test]
fn thematic_break_spaced_hyphen() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("- - -"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(
        mi.item,
        Block::Break(Break {
            type_: BreakType::Thematic,
            source: Span {
                data: "- - -",
                line: 1,
                col: 1,
                offset: 0,
            },
            title_source: None,
            title: None,
            anchor: None,
            attrlist: None,
        })
    );

    assert_eq!(mi.item.content_model(), ContentModel::Empty);
    assert!(mi.item.rendered_content().is_none());
    assert_eq!(mi.item.raw_context().deref(), "thematic_break");
}

#[test]
fn thematic_break_triple_asterisk() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("***"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(
        mi.item,
        Block::Break(Break {
            type_: BreakType::Thematic,
            source: Span {
                data: "***",
                line: 1,
                col: 1,
                offset: 0,
            },
            title_source: None,
            title: None,
            anchor: None,
            attrlist: None,
        })
    );

    assert_eq!(mi.item.content_model(), ContentModel::Empty);
    assert!(mi.item.rendered_content().is_none());
    assert_eq!(mi.item.raw_context().deref(), "thematic_break");
}

#[test]
fn thematic_break_spaced_asterisk() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("* * *"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(
        mi.item,
        Block::Break(Break {
            type_: BreakType::Thematic,
            source: Span {
                data: "* * *",
                line: 1,
                col: 1,
                offset: 0,
            },
            title_source: None,
            title: None,
            anchor: None,
            attrlist: None,
        })
    );

    assert_eq!(mi.item.content_model(), ContentModel::Empty);
    assert!(mi.item.rendered_content().is_none());
    assert_eq!(mi.item.raw_context().deref(), "thematic_break");
}

#[test]
fn page_break() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("<<<"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(
        mi.item,
        Block::Break(Break {
            type_: BreakType::Page,
            source: Span {
                data: "<<<",
                line: 1,
                col: 1,
                offset: 0,
            },
            title_source: None,
            title: None,
            anchor: None,
            attrlist: None,
        })
    );

    assert_eq!(
        mi.after,
        Span {
            data: "",
            line: 1,
            col: 4,
            offset: 3
        }
    );

    assert_eq!(mi.item.content_model(), ContentModel::Empty);
    assert!(mi.item.rendered_content().is_none());
    assert_eq!(mi.item.raw_context().deref(), "page_break");
    assert!(mi.item.nested_blocks().next().is_none());
    assert!(mi.item.title_source().is_none());
    assert!(mi.item.title().is_none());
    assert!(mi.item.anchor().is_none());
    assert!(mi.item.anchor_reftext().is_none());
    assert!(mi.item.attrlist().is_none());
    assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);
}

#[test]
fn thematic_break_with_trailing_whitespace() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("'''   "), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(
        mi.item,
        Block::Break(Break {
            type_: BreakType::Thematic,
            source: Span {
                data: "'''",
                line: 1,
                col: 1,
                offset: 0,
            },
            title_source: None,
            title: None,
            anchor: None,
            attrlist: None,
        })
    );

    assert_eq!(mi.item.raw_context().deref(), "thematic_break");
}

#[test]
fn page_break_with_trailing_whitespace() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("<<<   "), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(
        mi.item,
        Block::Break(Break {
            type_: BreakType::Page,
            source: Span {
                data: "<<<",
                line: 1,
                col: 1,
                offset: 0,
            },
            title_source: None,
            title: None,
            anchor: None,
            attrlist: None,
        })
    );

    assert_eq!(mi.item.raw_context().deref(), "page_break");
}

#[test]
fn thematic_break_with_title() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new(".My Break Title\n'''\n"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(
        mi.item,
        Block::Break(Break {
            type_: BreakType::Thematic,
            source: Span {
                data: ".My Break Title\n'''",
                line: 1,
                col: 1,
                offset: 0,
            },
            title_source: Some(Span {
                data: "My Break Title",
                line: 1,
                col: 2,
                offset: 1,
            }),
            title: Some("My Break Title".to_string()),
            anchor: None,
            attrlist: None,
        })
    );

    assert_eq!(
        mi.item.span(),
        Span {
            data: ".My Break Title\n'''",
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
            offset: 20
        }
    );
}

#[test]
fn page_break_with_title() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new(".Page Break\n<<<\n"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(
        mi.item,
        Block::Break(Break {
            type_: BreakType::Page,
            source: Span {
                data: ".Page Break\n<<<",
                line: 1,
                col: 1,
                offset: 0,
            },
            title_source: Some(Span {
                data: "Page Break",
                line: 1,
                col: 2,
                offset: 1,
            }),
            title: Some("Page Break".to_string()),
            anchor: None,
            attrlist: None,
        })
    );

    assert_eq!(mi.item.content_model(), ContentModel::Empty);
    assert_eq!(mi.item.raw_context().deref(), "page_break");
}

#[test]
fn break_with_anchor() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("[[my-break]]\n'''\n"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(
        mi.item,
        Block::Break(Break {
            type_: BreakType::Thematic,
            source: Span {
                data: "[[my-break]]\n'''",
                line: 1,
                col: 1,
                offset: 0,
            },
            title_source: None,
            title: None,
            anchor: Some(Span {
                data: "my-break",
                line: 1,
                col: 3,
                offset: 2,
            }),
            attrlist: None,
        })
    );

    assert_eq!(mi.item.anchor().unwrap().data(), "my-break");
    assert_eq!(mi.item.id(), Some("my-break"));
    assert!(mi.item.anchor_reftext().is_none());
}

#[test]
fn break_with_attrlist() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(
        crate::Span::new(
            "[#mybreak.role1.role2%option1]
'''\n",
        ),
        &mut parser,
    )
    .unwrap_if_no_warnings()
    .unwrap();

    // Verify it's a Break block and check its type.
    if let crate::blocks::Block::Break(ref brk) = mi.item {
        assert_eq!(brk.type_(), BreakType::Thematic);
    } else {
        panic!("Expected Break block");
    }

    // Test IsBlock trait methods with attrlist metadata.
    assert!(mi.item.attrlist().is_some());
    assert_eq!(mi.item.id(), Some("mybreak"));
    assert_eq!(mi.item.roles(), vec!["role1", "role2"]);
    assert_eq!(mi.item.options(), vec!["option1"]);
    assert!(mi.item.has_option("option1"));
    assert!(!mi.item.has_option("option2"));
    assert!(mi.item.declared_style().is_none());
    assert_eq!(mi.item.raw_context().deref(), "thematic_break");
    assert_eq!(mi.item.resolved_context().deref(), "thematic_break");
    assert!(mi.item.title_source().is_none());
    assert!(mi.item.title().is_none());
}

#[test]
fn break_trait_methods_without_metadata() {
    // Test that IsBlock trait methods work correctly for a break without metadata.
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("<<<"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    // Test all IsBlock trait methods.
    assert_eq!(mi.item.content_model(), ContentModel::Empty);
    assert_eq!(mi.item.raw_context().deref(), "page_break");
    assert_eq!(mi.item.resolved_context().deref(), "page_break");
    assert!(mi.item.declared_style().is_none());
    assert!(mi.item.nested_blocks().next().is_none());
    assert!(mi.item.id().is_none());
    assert!(mi.item.roles().is_empty());
    assert!(mi.item.options().is_empty());
    assert!(!mi.item.has_option("any_option"));
    assert!(mi.item.title_source().is_none());
    assert!(mi.item.title().is_none());
    assert!(mi.item.anchor().is_none());
    assert!(mi.item.anchor_reftext().is_none());
    assert!(mi.item.attrlist().is_none());
    assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

    // Test HasSpan trait method.
    assert_eq!(
        mi.item.span(),
        Span {
            data: "<<<",
            line: 1,
            col: 1,
            offset: 0,
        }
    );
}
