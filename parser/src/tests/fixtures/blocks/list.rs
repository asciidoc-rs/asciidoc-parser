use std::fmt;

use crate::{
    HasSpan,
    blocks::IsBlock,
    tests::fixtures::{Span, attributes::Attrlist, blocks::Block},
};

#[derive(Eq, PartialEq)]
pub(crate) struct ListBlock {
    pub type_: crate::blocks::ListType,
    pub items: &'static [Block],
    pub source: Span,
    pub title_source: Option<Span>,
    pub title: Option<&'static str>,
    pub anchor: Option<Span>,
    pub anchor_reftext: Option<Span>,
    pub attrlist: Option<Attrlist>,
}

impl fmt::Debug for ListBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ListBlock")
            .field("type_", &self.type_)
            .field("items", &self.items)
            .field("source", &self.source)
            .field("title_source", &self.title_source)
            .field("title", &self.title)
            .field("anchor", &self.anchor)
            .field("anchor_reftext", &self.anchor_reftext)
            .field("attrlist", &self.attrlist)
            .finish()
    }
}

impl<'src> PartialEq<crate::blocks::ListBlock<'src>> for ListBlock {
    fn eq(&self, other: &crate::blocks::ListBlock<'src>) -> bool {
        fixture_eq_observed(self, other)
    }
}

impl PartialEq<ListBlock> for crate::blocks::ListBlock<'_> {
    fn eq(&self, other: &ListBlock) -> bool {
        fixture_eq_observed(other, self)
    }
}

fn fixture_eq_observed(fixture: &ListBlock, observed: &crate::blocks::ListBlock) -> bool {
    if fixture.type_ != observed.type_() {
        return false;
    }

    if fixture.items.len() != observed.nested_blocks().len() {
        return false;
    }

    for (fixture_block, observed_block) in fixture.items.iter().zip(observed.nested_blocks()) {
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
