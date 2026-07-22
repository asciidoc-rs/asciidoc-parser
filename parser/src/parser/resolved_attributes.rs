use std::{collections::HashMap, sync::Arc};

use crate::{
    SafeMode,
    document::InterpretedValue,
    parser::{
        AttributeValue, DatetimeContext, DatetimeInputs, ReferenceTime,
        built_in_attrs::{
            built_in_attr, derived_backend_value, max_attribute_value_size_default,
            synthesized_attr, user_home_default,
        },
        is_datetime_attribute,
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

    /// The safe mode the parser ran under. It is not stored in any attribute
    /// table, so the snapshot captures it here to resolve the mode-aware
    /// intrinsics (`max-attribute-value-size`, `user-home`) exactly as the
    /// parser does. Defaults to [`SafeMode::Secure`], matching the parser.
    safe: SafeMode,

    /// The parser's pinned reference-time configuration, if any, used to
    /// resolve the time-dependent attributes (`docdate` and its family) on
    /// demand. This is deterministic parser configuration (not a captured
    /// instant), so two snapshots taken from equally-configured parsers
    /// stay equal. It is boxed (and `None` unless a clock was pinned) so it
    /// costs only a pointer here — this snapshot is embedded in a
    /// size-sensitive cell enum.
    datetime_inputs: Option<Box<DatetimeInputs>>,
}

impl ResolvedAttributes {
    pub(crate) fn new(
        attribute_values: Arc<HashMap<String, AttributeValue>>,
        default_attribute_values: Arc<HashMap<String, String>>,
        counter_values: HashMap<String, String>,
        safe: SafeMode,
        reference_time: Option<ReferenceTime>,
        input_mtime: Option<ReferenceTime>,
    ) -> Self {
        Self {
            attribute_values,
            default_attribute_values,
            counter_values,
            safe,
            datetime_inputs: DatetimeInputs::new(reference_time, input_mtime),
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

        // `basebackend` / `filetype` are derived on the fly from the current
        // `backend` (see [`derived_backend_value`]) rather than stored.
        if let Some(value) = derived_backend_value(name, &self.attribute_values) {
            return value;
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
            // A time-dependent attribute is resolved on demand from the
            // reference-time configuration rather than stored in a table.
            None => self
                .resolve_datetime_attribute(name)
                .unwrap_or(InterpretedValue::Unset),
        }
    }

    /// Resolves a time-dependent document attribute (`docdate` and its family)
    /// on demand from the snapshot's reference-time configuration, mirroring
    /// [`Parser::attribute_value`](crate::Parser::attribute_value). Returns
    /// `None` for any other name.
    ///
    /// An explicit value stored during parsing still wins, since the readers
    /// consult [`effective_attribute`](Self::effective_attribute) first.
    ///
    /// # Consistency of an unpinned clock
    ///
    /// The reference instant is captured per call (these attributes are read
    /// rarely from a snapshot) rather than cached; within a single call it is
    /// consistent. When the clock is *pinned* — a
    /// [`reference_time`](crate::Parser::with_reference_time), an
    /// [`input_mtime`](crate::Parser::with_input_mtime), or `SOURCE_DATE_EPOCH`
    /// — every capture yields the same instant, so a snapshot lookup always
    /// agrees with the value substituted into content during parsing. This is
    /// the reproducible-build path and the intended way to consume these
    /// attributes.
    ///
    /// When the clock is *not* pinned, each capture reads the real wall clock
    /// (or a since-changed `SOURCE_DATE_EPOCH`) afresh, so a post-parse lookup
    /// can disagree with content the parser already rendered — e.g. `{docdate}`
    /// substituted just before midnight, then read back just after. The
    /// snapshot deliberately does *not* freeze the parser's capture here: doing
    /// so would either make snapshot equality depend on a wall-clock reading or
    /// require the parser to capture (a clock/environment read plus an
    /// allocation) on *every* parse, defeating the laziness that keeps a parse
    /// which never references one of these attributes free of that cost. The
    /// unpinned clock is inherently non-reproducible, so pin it (or set
    /// `SOURCE_DATE_EPOCH`) whenever stable, self-consistent output matters.
    fn resolve_datetime_attribute(&self, name: &str) -> Option<InterpretedValue> {
        if !is_datetime_attribute(name) {
            return None;
        }

        // Absent any pinned clock the inputs box is `None`; capture from the
        // defaults (SOURCE_DATE_EPOCH, then the real wall clock) in that case.
        // See the doc comment above on why an unpinned capture is intentionally
        // taken fresh here rather than frozen from the parse.
        let context = match &self.datetime_inputs {
            Some(inputs) => inputs.capture(),
            None => DatetimeContext::capture(None, None),
        };

        context
            .resolve(name, |sibling| self.stored_datetime_override(sibling))
            .map(InterpretedValue::Value)
    }

    /// Returns the *explicitly-set* value of `name` from the stored attribute
    /// map (a value-less "set" reads as an empty string), or `None`.
    ///
    /// Reads only the stored overrides — never the on-the-fly datetime
    /// resolution — so it can feed the explicit sibling values
    /// [`resolve_datetime_attribute`](Self::resolve_datetime_attribute) needs
    /// without recursing.
    fn stored_datetime_override(&self, name: &str) -> Option<String> {
        self.attribute_values
            .get(name)
            .and_then(|av| match &av.value {
                InterpretedValue::Value(value) => Some(value.clone()),
                InterpretedValue::Set => Some(String::new()),
                InterpretedValue::Unset => None,
            })
    }

    /// Returns the effective attribute definition for `name`, falling back to
    /// the mode-aware intrinsics (`max-attribute-value-size`, `user-home`), the
    /// shared built-in defaults, and the synthesized derived attributes exactly
    /// as [`Parser::effective_attribute`] does.
    ///
    /// [`Parser::effective_attribute`]: crate::Parser::effective_attribute
    fn effective_attribute(&self, name: &str) -> Option<&AttributeValue> {
        if let Some(av) = self.attribute_values.get(name) {
            return Some(av);
        }
        // Mirror the parser's mode-aware resolution of the two intrinsics whose
        // default depends on the safe mode rather than on either attribute
        // table (see [`Parser::effective_attribute`]). Consulted after the
        // per-parser map so a caller-supplied value still wins.
        if name == "max-attribute-value-size" {
            return Some(max_attribute_value_size_default(
                self.safe == SafeMode::Secure,
            ));
        }
        if name == "user-home" {
            return Some(user_home_default(self.safe < SafeMode::Server));
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
        // A derived `basebackend` / `filetype` is present only while `backend`
        // resolves to a non-empty value (see [`derived_backend_value`]).
        if derived_backend_value(name, &self.attribute_values).is_some() {
            return true;
        }
        self.effective_attribute(name).is_some() || self.resolve_datetime_attribute(name).is_some()
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

        // A derived `basebackend` / `filetype` holds a concrete (set) value
        // whenever it is present, i.e. while `backend` is non-empty (see
        // [`derived_backend_value`]).
        if derived_backend_value(name, &self.attribute_values).is_some() {
            return true;
        }

        self.effective_attribute(name)
            .map(|a| a.value != InterpretedValue::Unset)
            .unwrap_or_else(|| self.resolve_datetime_attribute(name).is_some())
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use crate::{
        SafeMode,
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
            SafeMode::Secure,
            None,
            None,
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
