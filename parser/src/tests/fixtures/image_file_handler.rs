use std::collections::HashMap;

use crate::{parser::ImageFileHandler, tests::prelude::*};

/// A test [`ImageFileHandler`] that resolves a fixed set of resolved image
/// paths to raw image bytes.
#[derive(Debug)]
pub(crate) struct ImageFileHandlerFixture(HashMap<&'static str, &'static [u8]>);

impl ImageFileHandlerFixture {
    pub(crate) fn from_pairs<const N: usize>(pairs: [(&'static str, &'static [u8]); N]) -> Self {
        Self(pairs.into_iter().collect())
    }
}

impl ImageFileHandler for ImageFileHandlerFixture {
    fn resolve_image(&self, target: &str, _parser: &Parser) -> Option<Vec<u8>> {
        self.0.get(target).map(|v| v.to_vec())
    }
}
