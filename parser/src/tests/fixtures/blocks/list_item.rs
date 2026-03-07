use std::fmt;

use crate::{
    HasSpan,
    blocks::IsBlock,
    tests::fixtures::{
        Span,
        attributes::Attrlist,
        blocks::{Block, ListItemMarker},
    },
};

#[derive(Eq, PartialEq)]
pub(crate) struct ListItem {
    pub marker: ListItemMarker,
    pub blocks: &'static [Block],
    pub source: Span,
    pub anchor: Option<Span>,
    pub anchor_reftext: Option<Span>,
    pub attrlist: Option<Attrlist>,
}

impl fmt::Debug for ListItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ListItem")
            .field("marker", &self.marker)
            .field("blocks", &self.blocks)
            .field("source", &self.source)
            .field("anchor", &self.anchor)
            .field("anchor_reftext", &self.anchor_reftext)
            .field("attrlist", &self.attrlist)
            .finish()
    }
}

impl<'src> PartialEq<crate::blocks::ListItem<'src>> for ListItem {
    fn eq(&self, other: &crate::blocks::ListItem<'src>) -> bool {
        fixture_eq_observed(self, other)
    }
}

impl PartialEq<ListItem> for crate::blocks::ListItem<'_> {
    fn eq(&self, other: &ListItem) -> bool {
        fixture_eq_observed(other, self)
    }
}

fn fixture_eq_observed(fixture: &ListItem, observed: &crate::blocks::ListItem) -> bool {
    if fixture.marker != observed.list_item_marker() {
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
