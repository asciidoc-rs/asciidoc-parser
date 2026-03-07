use crate::tests::fixtures::{
    blocks::{
        Break, CompoundDelimitedBlock, ListBlock, ListItem, MediaBlock, Preamble,
        RawDelimitedBlock, SectionBlock, SimpleBlock,
    },
    document::Attribute,
};

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Block {
    Simple(SimpleBlock),
    Media(MediaBlock),
    Section(SectionBlock),
    List(ListBlock),
    ListItem(ListItem),
    RawDelimited(RawDelimitedBlock),
    CompoundDelimited(CompoundDelimitedBlock),
    Preamble(Preamble),
    Break(Break),
    DocumentAttribute(Attribute),
}

impl<'src> PartialEq<crate::blocks::Block<'src>> for Block {
    fn eq(&self, other: &crate::blocks::Block<'src>) -> bool {
        fixture_eq_observed(self, other)
    }
}

impl PartialEq<Block> for crate::blocks::Block<'_> {
    fn eq(&self, other: &Block) -> bool {
        fixture_eq_observed(other, self)
    }
}

fn fixture_eq_observed(fixture: &Block, observed: &crate::blocks::Block) -> bool {
    match fixture {
        Block::Simple(simple_fixture) => match observed {
            crate::blocks::Block::Simple(simple_observed) => simple_fixture == simple_observed,
            _ => false,
        },

        Block::Media(macro_fixture) => match observed {
            crate::blocks::Block::Media(macro_observed) => macro_fixture == macro_observed,
            _ => false,
        },

        Block::Section(section_fixture) => match observed {
            crate::blocks::Block::Section(section_observed) => section_fixture == section_observed,
            _ => false,
        },

        Block::List(list_fixture) => match observed {
            crate::blocks::Block::List(list_item_observed) => list_fixture == list_item_observed,
            _ => false,
        },

        Block::ListItem(list_item_fixture) => match observed {
            crate::blocks::Block::ListItem(list_item_observed) => {
                list_item_fixture == list_item_observed
            }
            _ => false,
        },

        Block::RawDelimited(rdb_fixture) => match observed {
            crate::blocks::Block::RawDelimited(rdb_observed) => rdb_fixture == rdb_observed,
            _ => false,
        },

        Block::CompoundDelimited(cdb_fixture) => match observed {
            crate::blocks::Block::CompoundDelimited(cdb_observed) => cdb_fixture == cdb_observed,
            _ => false,
        },

        Block::Preamble(preamble_fixture) => match observed {
            crate::blocks::Block::Preamble(preamble_observed) => {
                preamble_fixture == preamble_observed
            }
            _ => false,
        },

        Block::Break(break_fixture) => match observed {
            crate::blocks::Block::Break(break_observed) => break_fixture == break_observed,
            _ => false,
        },

        Block::DocumentAttribute(attr_fixture) => match observed {
            crate::blocks::Block::DocumentAttribute(attr_observed) => attr_fixture == attr_observed,
            _ => false,
        },
    }
}
