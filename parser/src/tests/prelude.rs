#![allow(unused)]
// I'm generally not a fan of preludes, but for repetitive test infrastructure,
// I'm making an exception.

pub(crate) use std::collections::HashMap;

pub(crate) use crate::{
    ASCIIDOCTOR_VERSION, HasSpan, Parser, SafeMode,
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

/// Returns the document's first top-level block as a [`Break`], panicking if it
/// is absent or not a break.
///
/// [`Break`]: crate::blocks::Break
pub(crate) fn first_break<'src>(doc: &'src crate::Document) -> &'src crate::blocks::Break<'src> {
    let block = doc
        .nested_blocks()
        .next()
        .expect("expected at least one block");

    let crate::blocks::Block::Break(brk) = block else {
        panic!("Wrong block type: {block:#?}");
    };

    brk
}

/// Interprets `block` as a [`SectionBlock`], panicking if it is any other block
/// type. Mirrors Ruby Asciidoctor's habit of indexing `doc.blocks[n]` and
/// reading section-specific attributes off the result.
///
/// [`SectionBlock`]: crate::blocks::SectionBlock
pub(crate) fn as_section<'src>(
    block: &'src crate::blocks::Block<'src>,
) -> &'src crate::blocks::SectionBlock<'src> {
    let crate::blocks::Block::Section(section) = block else {
        panic!("Wrong block type: {block:#?}");
    };

    section
}

/// Returns the document's top-level blocks as a `Vec`, for indexed access in
/// the style of Ruby Asciidoctor's `doc.blocks[n]`.
pub(crate) fn top_blocks<'src>(
    doc: &'src crate::Document<'src>,
) -> Vec<&'src crate::blocks::Block<'src>> {
    doc.nested_blocks().collect()
}

/// Returns the first [`SectionBlock`] in document order, recursing into nested
/// blocks. Mirrors Ruby Asciidoctor's `block_from_string('== ...')`, which
/// returns the parsed section.
///
/// [`SectionBlock`]: crate::blocks::SectionBlock
pub(crate) fn first_section<'src>(
    doc: &'src crate::Document<'src>,
) -> &'src crate::blocks::SectionBlock<'src> {
    fn walk<'src>(
        mut blocks: impl Iterator<Item = &'src crate::blocks::Block<'src>>,
    ) -> Option<&'src crate::blocks::SectionBlock<'src>> {
        blocks.find_map(|block| {
            if let crate::blocks::Block::Section(section) = block {
                Some(section)
            } else {
                walk(block.nested_blocks())
            }
        })
    }

    walk(doc.nested_blocks()).expect("expected at least one section block")
}

/// Returns every [`SectionBlock`] in the document, in document order, recursing
/// into nested blocks.
///
/// [`SectionBlock`]: crate::blocks::SectionBlock
pub(crate) fn all_sections<'src>(
    doc: &'src crate::Document<'src>,
) -> Vec<&'src crate::blocks::SectionBlock<'src>> {
    fn walk<'src>(
        blocks: impl Iterator<Item = &'src crate::blocks::Block<'src>>,
        out: &mut Vec<&'src crate::blocks::SectionBlock<'src>>,
    ) {
        for block in blocks {
            if let crate::blocks::Block::Section(section) = block {
                out.push(section);
            }
            walk(block.nested_blocks(), out);
        }
    }

    let mut out = vec![];
    walk(doc.nested_blocks(), &mut out);
    out
}
