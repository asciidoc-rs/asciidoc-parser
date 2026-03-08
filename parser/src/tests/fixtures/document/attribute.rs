use std::{cmp::PartialEq, fmt};

use crate::tests::{
    fixtures::{Span, document::InterpretedValue},
    prelude::*,
};

#[derive(Eq, PartialEq)]
pub(crate) struct Attribute {
    pub name: Span,
    pub value_source: Option<Span>,
    pub value: InterpretedValue,
    pub source: Span,
}

impl fmt::Debug for Attribute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("crate::document::Attribute")
            .field("name", &self.name)
            .field("value_source", &self.value_source)
            .field("value", &self.value)
            .field("source", &self.source)
            .finish()
    }
}

impl<'src> PartialEq<crate::document::Attribute<'src>> for Attribute {
    fn eq(&self, other: &crate::document::Attribute<'src>) -> bool {
        fixture_eq_observed(self, other)
    }
}

impl PartialEq<Attribute> for crate::document::Attribute<'_> {
    fn eq(&self, other: &Attribute) -> bool {
        fixture_eq_observed(other, self)
    }
}

impl PartialEq<Attribute> for &crate::document::Attribute<'_> {
    fn eq(&self, other: &Attribute) -> bool {
        fixture_eq_observed(other, self)
    }
}

fn fixture_eq_observed(fixture: &Attribute, observed: &crate::document::Attribute) -> bool {
    if fixture.source != observed.span() {
        return false;
    }

    if &fixture.name != observed.name() {
        return false;
    }

    if let Some(ref fixture_value_source) = fixture.value_source
        && let Some(observed_raw_value) = observed.raw_value()
        && fixture_value_source != &observed_raw_value
    {
        return false;
    }

    if fixture.value_source.is_some() != observed.raw_value().is_some() {
        return false;
    }

    if &fixture.value != observed.value() {
        return false;
    }

    true
}
