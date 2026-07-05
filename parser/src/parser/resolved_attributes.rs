use std::{collections::HashMap, sync::Arc};

use crate::{document::InterpretedValue, parser::AttributeValue};

/// A snapshot of a [`Parser`]'s fully-resolved document-attribute state, taken
/// at the end of parsing so it can be retained on a [`Document`] and queried
/// without a [`Parser`] in hand.
///
/// The attribute tables are shared from the parser by [`Arc`] rather than
/// copied, so taking a snapshot is cheap (the large built-in attribute table is
/// never deep-cloned). Only the small set of active counter values is copied.
///
/// [`attribute_value`](Self::attribute_value),
/// [`has_attribute`](Self::has_attribute), and
/// [`is_attribute_set`](Self::is_attribute_set) mirror the identically-named
/// [`Parser`] methods exactly, so a lookup here returns the same value the
/// parser would report after `parse`.
///
/// [`Parser`]: crate::Parser
/// [`Document`]: crate::Document
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedAttributes {
    /// Attribute values as of the end of parsing (shared with the parser via
    /// [`Arc`]).
    attribute_values: Arc<HashMap<String, AttributeValue>>,

    /// Default values applied to attributes that are "set" with an empty value
    /// (shared with the parser via [`Arc`]).
    default_attribute_values: Arc<HashMap<String, String>>,

    /// Current value of each counter as of the end of parsing. A counter value
    /// supersedes any like-named attribute.
    counter_values: HashMap<String, String>,
}

impl ResolvedAttributes {
    pub(crate) fn new(
        attribute_values: Arc<HashMap<String, AttributeValue>>,
        default_attribute_values: Arc<HashMap<String, String>>,
        counter_values: HashMap<String, String>,
    ) -> Self {
        Self {
            attribute_values,
            default_attribute_values,
            counter_values,
        }
    }

    /// Returns the resolved interpreted value of the named document attribute.
    ///
    /// Mirrors [`Parser::attribute_value`](crate::Parser::attribute_value).
    pub(crate) fn attribute_value<N: AsRef<str>>(&self, name: N) -> InterpretedValue {
        // A counter's current value supersedes any earlier value of the
        // attribute of the same name.
        if let Some(value) = self.counter_values.get(name.as_ref()) {
            return InterpretedValue::Value(value.clone());
        }

        self.attribute_values
            .get(name.as_ref())
            .map(|av| av.value.clone())
            .map(|av| {
                if let InterpretedValue::Set = av
                    && let Some(default) = self.default_attribute_values.get(name.as_ref())
                {
                    InterpretedValue::Value(default.clone())
                } else {
                    av
                }
            })
            .unwrap_or(InterpretedValue::Unset)
    }

    /// Returns `true` if the named document attribute is present (whether or
    /// not it is set).
    ///
    /// Mirrors [`Parser::has_attribute`](crate::Parser::has_attribute).
    pub(crate) fn has_attribute<N: AsRef<str>>(&self, name: N) -> bool {
        self.counter_values.contains_key(name.as_ref())
            || self.attribute_values.contains_key(name.as_ref())
    }

    /// Returns `true` if the named document attribute is present and set (i.e.
    /// not [unset]).
    ///
    /// Mirrors [`Parser::is_attribute_set`](crate::Parser::is_attribute_set).
    ///
    /// [unset]: https://docs.asciidoctor.org/asciidoc/latest/attributes/unset-attributes/
    pub(crate) fn is_attribute_set<N: AsRef<str>>(&self, name: N) -> bool {
        // A counter always holds a concrete (set) value.
        if self.counter_values.contains_key(name.as_ref()) {
            return true;
        }

        self.attribute_values
            .get(name.as_ref())
            .map(|a| a.value != InterpretedValue::Unset)
            .unwrap_or(false)
    }
}
