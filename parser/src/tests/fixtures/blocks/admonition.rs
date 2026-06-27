use std::fmt;

use crate::{
    HasSpan,
    blocks::{AdmonitionVariant, ContentModel, IsBlock},
    tests::fixtures::{Span, attributes::Attrlist, blocks::Block, content::Content},
};

#[derive(Eq, PartialEq)]
pub(crate) struct AdmonitionBlock {
    pub variant: AdmonitionVariant,
    pub label: &'static str,
    pub icons_font: bool,
    pub content_model: ContentModel,
    pub content: Option<Content>,
    pub blocks: &'static [Block],
    pub source: Span,
    pub title_source: Option<Span>,
    pub title: Option<&'static str>,
    pub anchor: Option<Span>,
    pub anchor_reftext: Option<Span>,
    pub attrlist: Option<Attrlist>,
}

impl fmt::Debug for AdmonitionBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AdmonitionBlock")
            .field("variant", &self.variant)
            .field("label", &self.label)
            .field("icons_font", &self.icons_font)
            .field("content_model", &self.content_model)
            .field("content", &self.content)
            .field("blocks", &self.blocks)
            .field("source", &self.source)
            .field("title_source", &self.title_source)
            .field("title", &self.title)
            .field("anchor", &self.anchor)
            .field("anchor_reftext", &self.anchor_reftext)
            .field("attrlist", &self.attrlist)
            .finish()
    }
}

impl<'src> PartialEq<crate::blocks::AdmonitionBlock<'src>> for AdmonitionBlock {
    fn eq(&self, other: &crate::blocks::AdmonitionBlock<'src>) -> bool {
        fixture_eq_observed(self, other)
    }
}

impl PartialEq<AdmonitionBlock> for crate::blocks::AdmonitionBlock<'_> {
    fn eq(&self, other: &AdmonitionBlock) -> bool {
        fixture_eq_observed(other, self)
    }
}

fn fixture_eq_observed(
    fixture: &AdmonitionBlock,
    observed: &crate::blocks::AdmonitionBlock,
) -> bool {
    if fixture.variant != observed.variant() {
        return false;
    }

    if fixture.label != observed.label() {
        return false;
    }

    if fixture.icons_font != observed.icons_font() {
        return false;
    }

    if fixture.content_model != observed.content_model() {
        return false;
    }

    if fixture.content.is_some() != observed.content().is_some() {
        return false;
    }

    if let Some(ref fixture_content) = fixture.content
        && let Some(observed_content) = observed.content()
        && fixture_content != observed_content
    {
        return false;
    }

    if fixture.blocks.len() != observed.nested_blocks().len() {
        return false;
    }

    for (fixture_block, observed_block) in fixture.blocks.iter().zip(observed.nested_blocks()) {
        if fixture_block != observed_block {
            return false;
        }
    }

    if fixture.title_source.is_some() != observed.title_source().is_some() {
        return false;
    }

    if let Some(ref fixture_title_source) = fixture.title_source
        && let Some(ref observed_title_source) = observed.title_source()
        && fixture_title_source != observed_title_source
    {
        return false;
    }

    if fixture.title.is_some() != observed.title().is_some() {
        return false;
    }

    if let Some(ref fixture_title) = fixture.title
        && let Some(ref observed_title) = observed.title()
        && fixture_title != observed_title
    {
        return false;
    }

    if fixture.anchor.is_some() != observed.anchor().is_some() {
        return false;
    }

    if let Some(ref fixture_anchor) = fixture.anchor
        && let Some(ref observed_anchor) = observed.anchor()
        && fixture_anchor != observed_anchor
    {
        return false;
    }

    if fixture.anchor_reftext.is_some() != observed.anchor_reftext().is_some() {
        return false;
    }

    if let Some(ref fixture_anchor_reftext) = fixture.anchor_reftext
        && let Some(ref observed_anchor_reftext) = observed.anchor_reftext()
        && fixture_anchor_reftext != observed_anchor_reftext
    {
        return false;
    }

    if fixture.attrlist.is_some() != observed.attrlist().is_some() {
        return false;
    }

    if let Some(ref fixture_attrlist) = fixture.attrlist
        && let Some(ref observed_attrlist) = observed.attrlist()
        && &fixture_attrlist != observed_attrlist
    {
        return false;
    }

    fixture.source == observed.span()
}
