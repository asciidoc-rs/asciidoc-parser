use std::fmt;

use crate::{
    blocks::{IsBlock, SectionType},
    tests::{
        fixtures::{
            Span,
            attributes::Attrlist,
            blocks::{Block, SectionNumber},
            content::Content,
        },
        prelude::*,
    },
};

#[derive(Eq, PartialEq)]
pub(crate) struct SectionBlock {
    pub level: usize,
    pub section_title: Content,
    pub blocks: &'static [Block],
    pub source: Span,
    pub title_source: Option<Span>,
    pub title: Option<&'static str>,
    pub anchor: Option<Span>,
    pub anchor_reftext: Option<Span>,
    pub attrlist: Option<Attrlist>,
    pub section_type: SectionType,
    pub section_id: Option<&'static str>,
    pub section_number: Option<SectionNumber>,
}

impl fmt::Debug for SectionBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SectionBlock")
            .field("level", &self.level)
            .field("section_title", &self.section_title)
            .field("blocks", &self.blocks)
            .field("source", &self.source)
            .field("title_source", &self.title_source)
            .field("title", &self.title)
            .field("anchor", &self.anchor)
            .field("anchor_reftext", &self.anchor_reftext)
            .field("attrlist", &self.attrlist)
            .field("section_type", &self.section_type)
            .field("section_id", &self.section_id)
            .field("section_number", &self.section_number)
            .finish()
    }
}

impl<'src> PartialEq<crate::blocks::SectionBlock<'src>> for SectionBlock {
    fn eq(&self, other: &crate::blocks::SectionBlock<'src>) -> bool {
        fixture_eq_observed(self, other)
    }
}

impl PartialEq<SectionBlock> for crate::blocks::SectionBlock<'_> {
    fn eq(&self, other: &SectionBlock) -> bool {
        fixture_eq_observed(other, self)
    }
}

fn fixture_eq_observed(fixture: &SectionBlock, observed: &crate::blocks::SectionBlock) -> bool {
    if fixture.level != observed.level() {
        return false;
    }

    if fixture.section_title.original != observed.section_title_source() {
        return false;
    }

    if fixture.section_title.rendered != observed.section_title() {
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

    if fixture.section_id.is_some() != observed.section_id().is_some() {
        return false;
    }

    if let Some(ref fixture_section_id) = fixture.section_id
        && let Some(ref observed_section_id) = observed.section_id()
        && fixture_section_id != observed_section_id
    {
        return false;
    }

    if fixture.section_number.is_some() != observed.section_number().is_some() {
        return false;
    }

    if let Some(ref fixture_section_number) = fixture.section_number
        && let Some(observed_section_number) = observed.section_number()
        && *fixture_section_number != *observed_section_number
    {
        return false;
    }

    fixture.source == observed.span() && fixture.section_type == observed.section_type()
}
