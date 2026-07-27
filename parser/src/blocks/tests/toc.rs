use std::ops::Deref;

use crate::{blocks::ContentModel, tests::prelude::*};

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
