use std::fmt;

use crate::{
    HasSpan,
    tests::fixtures::{Span, blocks::Block},
};

#[derive(Eq, PartialEq)]
pub(crate) struct Preamble {
    pub blocks: &'static [Block],
    pub source: Span,
}

impl fmt::Debug for Preamble {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Preamble")
            .field("blocks", &self.blocks)
            .field("source", &self.source)
            .finish()
    }
}

impl<'src> PartialEq<crate::blocks::Preamble<'src>> for Preamble {
    fn eq(&self, other: &crate::blocks::Preamble<'src>) -> bool {
        fixture_eq_observed(self, other)
    }
}

impl PartialEq<Preamble> for crate::blocks::Preamble<'_> {
    fn eq(&self, other: &Preamble) -> bool {
        fixture_eq_observed(other, self)
    }
}

fn fixture_eq_observed(fixture: &Preamble, observed: &crate::blocks::Preamble) -> bool {
    if fixture.blocks.len() != observed.child_blocks().count() {
        return false;
    }

    for (fixture_block, observed_block) in fixture.blocks.iter().zip(observed.child_blocks()) {
        if fixture_block != observed_block {
            return false;
        }
    }

    fixture.source == observed.span()
}
