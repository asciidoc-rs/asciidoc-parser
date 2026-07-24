use std::{cmp::PartialEq, fmt};

use crate::{
    blocks::FindBlocks,
    tests::fixtures::{
        Span,
        blocks::Block,
        document::{Catalog, Header},
        parser::SourceMap,
        warnings::Warning,
    },
};

#[derive(Eq, PartialEq)]
pub(crate) struct Document {
    pub header: Header,
    pub blocks: &'static [Block],
    pub source: Span,
    pub warnings: &'static [Warning],
    pub source_map: SourceMap,
    pub catalog: Catalog,
}

impl fmt::Debug for Document {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Document")
            .field("header", &self.header)
            .field("blocks", &self.blocks)
            .field("source", &self.source)
            .field("warnings", &self.warnings)
            .field("source_map", &self.source_map)
            .field("catalog", &self.catalog)
            .finish()
    }
}

impl<'src> PartialEq<crate::Document<'src>> for Document {
    fn eq(&self, other: &crate::Document<'src>) -> bool {
        fixture_eq_observed(self, other)
    }
}

impl PartialEq<Document> for crate::Document<'_> {
    fn eq(&self, other: &Document) -> bool {
        fixture_eq_observed(other, self)
    }
}

impl PartialEq<Document> for &crate::Document<'_> {
    fn eq(&self, other: &Document) -> bool {
        fixture_eq_observed(other, self)
    }
}

fn fixture_eq_observed(fixture: &Document, observed: &crate::Document) -> bool {
    if fixture.source != observed.span() {
        return false;
    }

    if &fixture.header != observed.header() {
        return false;
    }

    if fixture.blocks.len() != observed.child_blocks().count() {
        return false;
    }

    for (fixture_block, observed_block) in fixture.blocks.iter().zip(observed.child_blocks()) {
        if fixture_block != observed_block {
            return false;
        }
    }

    if fixture.warnings.len() != observed.warnings().len() {
        return false;
    }

    for (fixture_warning, observed_warning) in fixture.warnings.iter().zip(observed.warnings()) {
        if fixture_warning != observed_warning {
            return false;
        }
    }

    &fixture.source_map == observed.source_map() && &fixture.catalog == observed.catalog()
}
