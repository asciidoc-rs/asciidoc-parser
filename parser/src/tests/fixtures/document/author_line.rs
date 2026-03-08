use std::{cmp::PartialEq, fmt};

use crate::tests::{
    fixtures::{Span, document::Author},
    prelude::*,
};

#[derive(Eq, PartialEq)]
pub(crate) struct AuthorLine {
    pub(crate) authors: &'static [Author],
    pub(crate) source: Span,
}

impl fmt::Debug for AuthorLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("crate::document::AuthorLine")
            .field("authors", &self.authors)
            .field("source", &self.source)
            .finish()
    }
}

impl PartialEq<crate::document::AuthorLine<'_>> for AuthorLine {
    fn eq(&self, other: &crate::document::AuthorLine) -> bool {
        fixture_eq_observed(self, other)
    }
}

impl PartialEq<AuthorLine> for crate::document::AuthorLine<'_> {
    fn eq(&self, other: &AuthorLine) -> bool {
        fixture_eq_observed(other, self)
    }
}

impl PartialEq<AuthorLine> for &crate::document::AuthorLine<'_> {
    fn eq(&self, other: &AuthorLine) -> bool {
        fixture_eq_observed(other, self)
    }
}

fn fixture_eq_observed(fixture: &AuthorLine, observed: &crate::document::AuthorLine) -> bool {
    if fixture.source != observed.span() {
        return false;
    }

    if fixture.authors.len() != observed.authors().len() {
        return false;
    }

    for (fixture_author, observed_author) in fixture.authors.iter().zip(observed.authors()) {
        if fixture_author != observed_author {
            return false;
        }
    }

    true
}
