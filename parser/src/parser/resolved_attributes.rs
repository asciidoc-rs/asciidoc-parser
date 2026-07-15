use std::{collections::HashMap, sync::Arc};

use crate::{
    document::InterpretedValue,
    parser::{
        AttributeValue,
        built_in_attrs::{built_in_attr, synthesized_attr},
    },
};

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
#[derive(Clone, Debug, Default, Eq, PartialEq)]
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
        let name = name.as_ref();

        // A counter's current value supersedes any earlier value of the
        // attribute of the same name.
        if let Some(value) = self.counter_values.get(name) {
            return InterpretedValue::Value(value.clone());
        }

        // An unset `relfilesuffix` reads as the *effective* value of
        // `outfilesuffix` — routed through this same reader so an
        // `outfilesuffix` counter overlay is honored too (see
        // [`tracks_outfilesuffix`](Self::tracks_outfilesuffix)).
        if self.tracks_outfilesuffix(name) {
            return self.attribute_value("outfilesuffix");
        }

        match self.effective_attribute(name) {
            Some(av) => {
                if let InterpretedValue::Set = av.value
                    && let Some(default) = self.default_attribute_values.get(name)
                {
                    InterpretedValue::Value(default.clone())
                } else {
                    av.value.clone()
                }
            }
            None => InterpretedValue::Unset,
        }
    }

    /// Returns the effective attribute definition for `name`, falling back to
    /// the shared built-in defaults (and the synthesized derived doctype
    /// attribute) exactly as [`Parser::effective_attribute`] does.
    ///
    /// [`Parser::effective_attribute`]: crate::Parser::effective_attribute
    fn effective_attribute(&self, name: &str) -> Option<&AttributeValue> {
        if let Some(av) = self.attribute_values.get(name) {
            return Some(av);
        }
        if let Some(av) = built_in_attr(name) {
            return Some(av);
        }
        synthesized_attr(name, &self.attribute_values)
    }

    /// Reports whether `name` is `relfilesuffix` in its unset state, in which
    /// case a *read* resolves it to the current value of `outfilesuffix`.
    /// Mirrors [`Parser::tracks_outfilesuffix`], so a lookup here returns the
    /// same value the parser would report after `parse`. Callers apply a
    /// like-named counter overlay first, so a `{counter:relfilesuffix}` still
    /// wins over the tracked default.
    ///
    /// [`Parser::tracks_outfilesuffix`]: crate::Parser::tracks_outfilesuffix
    fn tracks_outfilesuffix(&self, name: &str) -> bool {
        name == "relfilesuffix" && !self.attribute_values.contains_key(name)
    }

    /// Returns `true` if the named document attribute is present (whether or
    /// not it is set).
    ///
    /// Mirrors [`Parser::has_attribute`](crate::Parser::has_attribute).
    pub(crate) fn has_attribute<N: AsRef<str>>(&self, name: N) -> bool {
        let name = name.as_ref();
        if self.counter_values.contains_key(name) {
            return true;
        }
        if self.tracks_outfilesuffix(name) {
            return self.has_attribute("outfilesuffix");
        }
        self.effective_attribute(name).is_some()
    }

    /// Returns `true` if the named document attribute is present and set (i.e.
    /// not [unset]).
    ///
    /// Mirrors [`Parser::is_attribute_set`](crate::Parser::is_attribute_set).
    ///
    /// [unset]: https://docs.asciidoctor.org/asciidoc/latest/attributes/unset-attributes/
    pub(crate) fn is_attribute_set<N: AsRef<str>>(&self, name: N) -> bool {
        let name = name.as_ref();

        // A counter always holds a concrete (set) value.
        if self.counter_values.contains_key(name) {
            return true;
        }

        if self.tracks_outfilesuffix(name) {
            return self.is_attribute_set("outfilesuffix");
        }

        self.effective_attribute(name)
            .map(|a| a.value != InterpretedValue::Unset)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use crate::{
        document::InterpretedValue,
        parser::{AllowableValue, AttributeValue, ModificationContext, ResolvedAttributes},
    };

    fn attr(value: InterpretedValue) -> AttributeValue {
        AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context: ModificationContext::Anywhere,
            silent_when_locked: false,
            value,
        }
    }

    /// Builds a snapshot exercising each attribute shape — an explicit value, a
    /// `Set` with a registered default, a `Set` with no default, and an
    /// explicitly unset attribute — plus a counter that shadows a like-named
    /// attribute.
    fn sample() -> ResolvedAttributes {
        let mut attribute_values: HashMap<String, AttributeValue> = HashMap::new();
        attribute_values.insert(
            "value".to_string(),
            attr(InterpretedValue::Value("v".to_string())),
        );
        attribute_values.insert("set-with-default".to_string(), attr(InterpretedValue::Set));
        attribute_values.insert("set-no-default".to_string(), attr(InterpretedValue::Set));
        attribute_values.insert("unset".to_string(), attr(InterpretedValue::Unset));
        attribute_values.insert(
            "shadowed".to_string(),
            attr(InterpretedValue::Value("attr".to_string())),
        );

        let mut default_attribute_values: HashMap<String, String> = HashMap::new();
        default_attribute_values.insert("set-with-default".to_string(), "d".to_string());

        let mut counter_values: HashMap<String, String> = HashMap::new();
        counter_values.insert("count".to_string(), "3".to_string());
        counter_values.insert("shadowed".to_string(), "counter".to_string());

        ResolvedAttributes::new(
            Arc::new(attribute_values),
            Arc::new(default_attribute_values),
            counter_values,
        )
    }

    #[test]
    fn attribute_value_resolves_each_shape() {
        let attrs = sample();

        // Explicit value.
        assert_eq!(
            attrs.attribute_value("value"),
            InterpretedValue::Value("v".to_string())
        );

        // `Set` with a default resolves to that default.
        assert_eq!(
            attrs.attribute_value("set-with-default"),
            InterpretedValue::Value("d".to_string())
        );

        // `Set` with no default stays `Set`.
        assert_eq!(
            attrs.attribute_value("set-no-default"),
            InterpretedValue::Set
        );

        // Explicitly unset resolves to `Unset`.
        assert_eq!(attrs.attribute_value("unset"), InterpretedValue::Unset);

        // Absent resolves to `Unset`.
        assert_eq!(attrs.attribute_value("absent"), InterpretedValue::Unset);

        // A counter reads back its current value.
        assert_eq!(
            attrs.attribute_value("count"),
            InterpretedValue::Value("3".to_string())
        );

        // A counter supersedes a like-named attribute.
        assert_eq!(
            attrs.attribute_value("shadowed"),
            InterpretedValue::Value("counter".to_string())
        );
    }

    #[test]
    fn has_attribute_reports_presence() {
        let attrs = sample();

        assert!(attrs.has_attribute("value"));
        assert!(attrs.has_attribute("unset"));
        assert!(attrs.has_attribute("count"));
        assert!(!attrs.has_attribute("absent"));
    }

    #[test]
    fn is_attribute_set_reports_set_state() {
        let attrs = sample();

        assert!(attrs.is_attribute_set("value"));
        assert!(attrs.is_attribute_set("set-no-default"));
        assert!(!attrs.is_attribute_set("unset"));
        assert!(!attrs.is_attribute_set("absent"));

        // A counter always holds a concrete (set) value.
        assert!(attrs.is_attribute_set("count"));
    }
}
