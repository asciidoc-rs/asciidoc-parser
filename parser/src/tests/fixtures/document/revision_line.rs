use std::{cmp::PartialEq, fmt};

use crate::tests::{fixtures::Span, prelude::*};

#[derive(Eq, PartialEq)]
pub(crate) struct RevisionLine {
    pub(crate) revnumber: Option<&'static str>,
    pub(crate) revdate: &'static str,
    pub(crate) revremark: Option<&'static str>,
    pub(crate) source: Span,
}

impl fmt::Debug for RevisionLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("crate::document::RevisionLine")
            .field("revnumber", &self.revnumber)
            .field("revdate", &self.revdate)
            .field("revremark", &self.revremark)
            .field("source", &self.source)
            .finish()
    }
}

impl PartialEq<crate::document::RevisionLine<'_>> for RevisionLine {
    fn eq(&self, other: &crate::document::RevisionLine) -> bool {
        fixture_eq_observed(self, other)
    }
}

impl PartialEq<RevisionLine> for crate::document::RevisionLine<'_> {
    fn eq(&self, other: &RevisionLine) -> bool {
        fixture_eq_observed(other, self)
    }
}

impl PartialEq<RevisionLine> for &crate::document::RevisionLine<'_> {
    fn eq(&self, other: &RevisionLine) -> bool {
        fixture_eq_observed(other, self)
    }
}

fn fixture_eq_observed(fixture: &RevisionLine, observed: &crate::document::RevisionLine) -> bool {
    fixture.revnumber == observed.revnumber()
        && fixture.revdate == observed.revdate()
        && fixture.revremark == observed.revremark()
        && fixture.source == observed.span()
}
