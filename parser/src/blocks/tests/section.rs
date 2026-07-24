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
    assert_eq!(mi.item.id().unwrap(), "_section_title");
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
            caption: None,
            section_number: None,
        })
    );

    assert_eq!(mi.item.child_blocks().next(), None);

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
    assert_eq!(mi.item.id().unwrap(), "_section_title");
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
                caption: None,
                number: None,
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
            caption: None,
            section_number: None,
        })
    );

    let mut nested_blocks = mi.item.child_blocks();

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
            caption: None,
            number: None,
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
    // A block title above a section heading does not title the section; it is
    // carried over to the first block inside the section (matching
    // Asciidoctor). The carried title has no source line adjacent to the
    // claiming block, so that block's `title_source` is `None`.
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
                title: Some("other section title"),
                caption: None,
                number: None,
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
            title_source: None,
            title: None,
            anchor: None,
            anchor_reftext: None,
            attrlist: None,
            section_type: SectionType::Normal,
            section_id: Some("_section_title"),
            caption: None,
            section_number: None,
        })
    );

    assert_eq!(mi.item.id().unwrap(), "_section_title");
    assert!(mi.item.roles().is_empty());
    assert!(mi.item.options().is_empty());

    assert!(mi.item.title_source().is_none());
    assert!(mi.item.title().is_none());

    assert!(mi.item.anchor().is_none());
    assert!(mi.item.anchor_reftext().is_none());
    assert!(mi.item.attrlist().is_none());

    let mut nested_blocks = mi.item.child_blocks();

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
            title: Some("other section title"),
            caption: None,
            number: None,
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
fn title_carries_into_nested_section_first_block() {
    // A carried block title passes through a nested section heading (which
    // doesn't claim it) and lands on that section's first block.
    let doc = Parser::default().parse(".carried title\n== Outer\n\n=== Inner\n\nparagraph");

    let Some(crate::blocks::Block::Section(outer)) = doc.child_blocks().next() else {
        panic!("expected outer section");
    };

    assert!(outer.title().is_none());

    let Some(crate::blocks::Block::Section(inner)) = outer.child_blocks().next() else {
        panic!("expected inner section");
    };

    assert!(inner.title().is_none());

    let paragraph = inner.child_blocks().next().unwrap();
    assert_eq!(paragraph.title().unwrap(), "carried title");
    assert!(paragraph.title_source().is_none());
}

#[test]
fn own_title_wins_over_carried_title() {
    // A block with a title of its own keeps it; the carried title is
    // discarded rather than cascading to a later block.
    let doc = Parser::default()
        .parse(".carried title\n== Section\n\n.own title\nparagraph 1\n\nparagraph 2");

    let Some(crate::blocks::Block::Section(section)) = doc.child_blocks().next() else {
        panic!("expected section");
    };

    assert!(section.title().is_none());

    let mut blocks = section.child_blocks();

    let paragraph_1 = blocks.next().unwrap();
    assert_eq!(paragraph_1.title().unwrap(), "own title");

    let paragraph_2 = blocks.next().unwrap();
    assert!(paragraph_2.title().is_none());
}

#[test]
fn title_carries_across_empty_section() {
    // When the titled section's body is empty, the carried title survives to
    // the next sibling section and lands on its first block (matching
    // Asciidoctor, where the block-attribute hash is passed along until a
    // block claims it).
    let doc = Parser::default().parse(".carried title\n== Empty\n\n== Sibling\n\nparagraph");

    let mut blocks = doc.child_blocks();

    let Some(crate::blocks::Block::Section(empty)) = blocks.next() else {
        panic!("expected first section");
    };

    assert!(empty.title().is_none());
    assert!(empty.child_blocks().next().is_none());

    let Some(crate::blocks::Block::Section(sibling)) = blocks.next() else {
        panic!("expected second section");
    };

    assert!(sibling.title().is_none());

    let paragraph = sibling.child_blocks().next().unwrap();
    assert_eq!(paragraph.title().unwrap(), "carried title");
}

#[test]
fn discrete_heading_keeps_its_title() {
    // A discrete heading is an ordinary block, not a section, so a block
    // title above it is not carried over.
    let doc = Parser::default().parse("[discrete]\n.kept title\n== Heading\n\nparagraph");

    let mut blocks = doc.child_blocks();

    let Some(crate::blocks::Block::Section(heading)) = blocks.next() else {
        panic!("expected discrete heading");
    };

    assert_eq!(heading.title().unwrap(), "kept title");

    let paragraph = blocks.next().unwrap();
    assert!(paragraph.title().is_none());
}

#[test]
fn carried_title_does_not_escape_a_table_cell() {
    // A titled empty section at the end of an AsciiDoc table cell leaves a
    // block title pending. The cell is a nested standalone document, so that
    // title must not leak out and be claimed by the next block in the enclosing
    // document.
    let doc = Parser::default().parse("|===\na|\n.Carried\n== Empty\n|===\n\nouter paragraph");

    // The outer paragraph following the table must not have claimed the cell's
    // carried title.
    let Some(outer_paragraph) = doc.child_blocks().find_map(|b| match b {
        crate::blocks::Block::Simple(s) => Some(s),
        _ => None,
    }) else {
        panic!("expected an outer paragraph after the table");
    };

    assert!(outer_paragraph.title().is_none());
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
                caption: None,
                number: None,
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
            caption: None,
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
