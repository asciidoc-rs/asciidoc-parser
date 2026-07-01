use std::collections::HashMap;

use crate::{parser::SvgFileHandler, tests::prelude::*};

/// A test [`SvgFileHandler`] that resolves a fixed set of resolved image paths
/// to SVG file contents.
#[derive(Debug)]
pub(crate) struct SvgFileHandlerFixture(HashMap<&'static str, &'static str>);

impl SvgFileHandlerFixture {
    pub(crate) fn from_pairs<const N: usize>(pairs: [(&'static str, &'static str); N]) -> Self {
        Self(pairs.into_iter().collect())
    }
}

impl SvgFileHandler for SvgFileHandlerFixture {
    fn resolve_svg(&self, target: &str, _parser: &Parser) -> Option<String> {
        self.0.get(target).map(|v| v.to_string())
    }
}
