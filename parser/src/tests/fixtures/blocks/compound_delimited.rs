use std::fmt;

use crate::{
    HasSpan,
    blocks::IsBlock,
    tests::fixtures::{Span, attributes::Attrlist, blocks::Block},
};

#[derive(Eq, PartialEq)]
pub(crate) struct CompoundDelimitedBlock {
    pub blocks: &'static [Block],
    pub context: &'static str,
    pub source: Span,
    pub title_source: Option<Span>,
    pub title: Option<&'static str>,
    pub caption: Option<&'static str>,
    pub number: Option<usize>,
    pub anchor: Option<Span>,
    pub anchor_reftext: Option<Span>,
    pub attrlist: Option<Attrlist>,
}

impl fmt::Debug for CompoundDelimitedBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompoundDelimitedBlock")
            .field("blocks", &self.blocks)
            .field("context", &self.context)
            .field("source", &self.source)
            .field("title_source", &self.title_source)
            .field("title", &self.title)
            .field("caption", &self.caption)
            .field("number", &self.number)
            .field("anchor", &self.anchor)
            .field("anchor_reftext", &self.anchor_reftext)
            .field("attrlist", &self.attrlist)
            .finish()
    }
}

impl<'src> PartialEq<crate::blocks::CompoundDelimitedBlock<'src>> for CompoundDelimitedBlock {
    fn eq(&self, other: &crate::blocks::CompoundDelimitedBlock<'src>) -> bool {
        fixture_eq_observed(self, other)
    }
}

impl PartialEq<CompoundDelimitedBlock> for crate::blocks::CompoundDelimitedBlock<'_> {
    fn eq(&self, other: &CompoundDelimitedBlock) -> bool {
        fixture_eq_observed(other, self)
    }
}

fn fixture_eq_observed(
    fixture: &CompoundDelimitedBlock,
    observed: &crate::blocks::CompoundDelimitedBlock,
) -> bool {
    if fixture.blocks.len() != observed.nested_blocks().len() {
        return false;
    }

    for (fixture_block, observed_block) in fixture.blocks.iter().zip(observed.nested_blocks()) {
        if fixture_block != observed_block {
            return false;
        }
    }

    if fixture.context != observed.raw_context().as_ref() {
        return false;
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

    if fixture.caption.is_some() != observed.caption().is_some() {
        return false;
    }

    if let Some(ref fixture_caption) = fixture.caption
        && let Some(ref observed_caption) = observed.caption()
        && fixture_caption != observed_caption
    {
        return false;
    }

    if fixture.number != observed.number() {
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

    if let Some(ref tcdb_attrlist) = fixture.attrlist
        && let Some(ref cdb_attrlist) = observed.attrlist()
        && &tcdb_attrlist != cdb_attrlist
    {
        return false;
    }

    fixture.source == observed.span()
}
