use std::collections::HashMap;

use crate::{
    attributes::Attrlist,
    parser::{IncludeContent, IncludeFileHandler, IncludeResolution},
    tests::prelude::*,
};

#[derive(Debug)]
pub(crate) struct InlineFileHandler(HashMap<&'static str, &'static str>);

impl InlineFileHandler {
    pub(crate) fn from_pairs<const N: usize>(pairs: [(&'static str, &'static str); N]) -> Self {
        Self(pairs.into_iter().collect())
    }
}

impl IncludeFileHandler for InlineFileHandler {
    fn resolve_target<'src>(
        &self,
        _source: Option<&str>,
        target: &str,
        _attrlist: &Attrlist<'src>,
        _parser: &Parser,
    ) -> IncludeResolution {
        self.0.get(target).map_or(IncludeResolution::NotFound, |v| {
            IncludeResolution::Found(IncludeContent::new(*v))
        })
    }
}
