#![allow(unused)]
// I'm generally not a fan of preludes, but for repetitive test infrastructure,
// I'm making an exception.

pub(crate) use std::collections::HashMap;

pub(crate) use crate::{
    HasSpan, Parser, SafeMode,
    blocks::{
        AdmonitionVariant, ColumnStyle, ContentModel, Frame, Grid, HorizontalAlignment, IsBlock,
        QuoteType, SectionType, SimpleBlockStyle, Stripes, VerticalAlignment,
    },
    content::SubstitutionGroup,
    parser::ModificationContext,
    tests::{
        assert_dom::*,
        fixtures::{attributes::*, blocks::*, content::*, document::*, parser::*, warnings::*, *},
        sdd::*,
    },
    warnings::WarningType,
};

/// Collects the rendered (post-substitution) text of every simple block
/// (paragraph) in the document, in document order.
///
/// This crate produces inline content at parse time but does not render whole
/// blocks to HTML, so tests that need to inspect the inline rendering of a
/// block (e.g. a bibliography anchor or a cross-reference) read it from here.
pub(crate) fn rendered_paragraphs(doc: &crate::Document<'_>) -> Vec<String> {
    fn walk(block: &crate::blocks::Block<'_>, out: &mut Vec<String>) {
        if let crate::blocks::Block::Simple(simple) = block {
            out.push(simple.content().rendered().to_string());
        }
        for child in block.nested_blocks() {
            walk(child, out);
        }
    }

    let mut out = vec![];
    for block in doc.nested_blocks() {
        walk(block, &mut out);
    }
    out
}
