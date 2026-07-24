use crate::tests::fixtures::parser::SourceLine;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct SourceMap(pub &'static [(usize, SourceLine)]);

impl PartialEq<crate::parser::SourceMap> for SourceMap {
    fn eq(&self, other: &crate::parser::SourceMap) -> bool {
        fixture_eq_observed(self, other)
    }
}

impl PartialEq<SourceMap> for crate::parser::SourceMap {
    fn eq(&self, other: &SourceMap) -> bool {
        fixture_eq_observed(other, self)
    }
}

fn fixture_eq_observed(fixture: &SourceMap, observed: &crate::parser::SourceMap) -> bool {
    let observed: Vec<_> = observed.anchors().collect();

    if fixture.0.len() != observed.len() {
        return false;
    }

    for (fixture_entry, (output_line, file, source_line)) in fixture.0.iter().zip(observed) {
        if fixture_entry.0 != output_line
            || fixture_entry.1.0 != file
            || fixture_entry.1.1 != source_line
        {
            return false;
        }
    }

    true
}
