use std::fmt;

use crate::{blocks::SectionType, internal::debug::DebugSliceReference};

#[derive(Eq, PartialEq)]
pub(crate) struct SectionNumber {
    pub section_type: SectionType,
    pub components: &'static [usize],
    pub appendix_letter: Option<&'static str>,
}

impl fmt::Debug for SectionNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SectionNumber")
            .field("section_type", &self.section_type)
            .field("components", &DebugSliceReference(self.components))
            .field("appendix_letter", &self.appendix_letter)
            .finish()
    }
}

impl PartialEq<crate::blocks::SectionNumber> for SectionNumber {
    fn eq(&self, other: &crate::blocks::SectionNumber) -> bool {
        fixture_eq_observed(self, other)
    }
}

impl PartialEq<SectionNumber> for crate::blocks::SectionNumber {
    fn eq(&self, other: &SectionNumber) -> bool {
        fixture_eq_observed(other, self)
    }
}

fn fixture_eq_observed(fixture: &SectionNumber, observed: &crate::blocks::SectionNumber) -> bool {
    fixture.section_type == observed.section_type
        && fixture.components == observed.components()
        && fixture.appendix_letter == observed.appendix_letter()
}
