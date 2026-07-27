use std::ops::Deref;

use crate::{blocks::ContentModel, tests::prelude::*, warnings::MatchAndWarnings};

#[test]
fn simplest_toc_macro() {
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("toc::[]"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(
        mi.item,
        Block::Toc(TocBlock {
            macro_attrlist: Attrlist {
                attributes: &[],
                anchor: None,
                source: Span {
                    data: "",
                    line: 1,
                    col: 7,
                    offset: 6,
                }
            },
            source: Span {
                data: "toc::[]",
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
            data: "toc::[]",
            line: 1,
            col: 1,
            offset: 0,
        }
    );

    assert_eq!(mi.item.content_model(), ContentModel::Empty);
    assert_eq!(mi.item.raw_context().deref(), "toc");
    assert_eq!(mi.item.resolved_context().deref(), "toc");

    // Exercise the remaining `Block::Toc` accessor delegations.
    assert!(mi.item.rendered_content().is_none());
    assert!(mi.item.declared_style().is_none());
    assert!(mi.item.title_source().is_none());
    assert!(mi.item.title().is_none());
    assert!(mi.item.caption().is_none());
    assert!(mi.item.number().is_none());
    assert!(mi.item.id().is_none());
    assert!(mi.item.roles().is_empty());
    assert!(mi.item.options().is_empty());
    assert!(mi.item.anchor().is_none());
    assert!(mi.item.anchor_reftext().is_none());
    assert!(mi.item.attrlist().is_none());
    assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);
}

#[test]
fn not_a_toc_macro_becomes_paragraph() {
    // `toc::foo[]` carries a target, which the `toc` block macro does not take,
    // so it is not recognized as a TOC block and falls through to a paragraph.
    let mut parser = Parser::default();

    let mi = crate::blocks::Block::parse(crate::Span::new("toc::foo[]"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap();

    assert_eq!(mi.item.raw_context().deref(), "paragraph");
}

#[test]
fn macro_attrlist_warning_is_surfaced() {
    // A warning raised while parsing the macro's attribute list (here, an empty
    // attribute value from the doubled comma) is propagated even though the TOC
    // block still parses.
    let mut parser = Parser::default();

    let MatchAndWarnings { item: mi, warnings } =
        crate::blocks::Block::parse(crate::Span::new("toc::[blah,,blap]"), &mut parser);

    let mi = mi.unwrap();
    assert_eq!(mi.item.raw_context().deref(), "toc");

    assert_eq!(
        warnings,
        vec![Warning {
            source: Span {
                data: "blah,,blap",
                line: 1,
                col: 7,
                offset: 6,
            },
            warning: WarningType::EmptyAttributeValue,
        }]
    );
}

#[test]
fn parses_in_preamble() {
    // `toc::[]` on its own line parses to a block whose resolved context is
    // `toc`, rather than a paragraph containing the literal text `toc::[]`.
    let doc = Parser::default()
        .parse("= Article\n:toc: macro\n\npre\n\ntoc::[]\n\n== Section One\n\nbody\n");

    let toc_contexts: Vec<String> = doc
        .descendant_blocks()
        .map(|b| b.resolved_context().as_ref().to_string())
        .filter(|c| c == "toc")
        .collect();

    assert_eq!(toc_contexts, vec!["toc".to_string()]);
}

#[test]
fn block_title_overrides_toc_title() {
    // A block title above the macro is surfaced through `title()` so a backend
    // can use it to override `toc-title`.
    let doc = Parser::default().parse("= Article\n:toc: macro\n\n.Contents\ntoc::[]\n");

    let toc = doc
        .descendant_blocks()
        .find(|b| b.resolved_context().as_ref() == "toc")
        .unwrap();

    assert_eq!(toc.title(), Some("Contents"));
}
