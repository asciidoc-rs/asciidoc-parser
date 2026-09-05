use std::cmp::PartialEq;

use crate::tests::fixtures::Span;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Content {
    pub original: Span,
    pub rendered: &'static str,
}

impl<'src> PartialEq<crate::content::Content<'src>> for Content {
    fn eq(&self, other: &crate::content::Content<'src>) -> bool {
        fixture_eq_observed(self, other)
    }
}

impl PartialEq<Content> for crate::content::Content<'_> {
    fn eq(&self, other: &Content) -> bool {
        fixture_eq_observed(other, self)
    }
}

impl PartialEq<Content> for &crate::content::Content<'_> {
    fn eq(&self, other: &Content) -> bool {
        fixture_eq_observed(other, self)
    }
}

fn fixture_eq_observed(fixture: &Content, observed: &crate::content::Content) -> bool {
    fixture.original == observed.original() && fixture.rendered == observed.rendered_html()
}
