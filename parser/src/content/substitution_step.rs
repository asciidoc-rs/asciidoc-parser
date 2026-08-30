use std::{borrow::Cow, sync::LazyLock};

use regex::{Captures, Regex, RegexBuilder, Replacer};

use crate::{
    Parser, Span,
    attributes::Attrlist,
    content::Content,
    document::InterpretedValue,
    parser::{
        CharacterReplacementType, InlineRenderer, QuoteScope, QuoteType, SpecialCharacter,
        attribute_lookup_name,
    },
    strings::CowStr,
    warnings::WarningType,
};

/// Each substitution type replaces characters, markup, attribute references,
/// and macros in text with the appropriate output for a given converter. When a
/// document is processed, up to six substitution types may be carried out
/// depending on the block or inline element’s assigned substitution group. The
/// processor runs the substitutions in the following order:
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SubstitutionStep {
    /// Searches for three characters (`<`, `>`, `&`) and replaces them with
    /// their named character references.
    SpecialCharacters,

    /// Replacement of formatting markup on inline elements.
    Quotes,

    /// Replacement of attribute references by the values they reference.
    AttributeReferences,

    /// Replaces textual characters such as marks, arrows, and dashes and
    /// replaces them with the decimal format of their Unicode code point, i.e.,
    /// a numeric character reference.
    CharacterReplacements,

    /// Replaces a macro’s content with the appropriate built-in and
    /// user-defined configuration.
    Macros,

    /// Replaces the line break character, `+` with a line-end marker.
    PostReplacement,

    /// Processes callouts in literal, listing, and source blocks.
    Callouts,
}

impl SubstitutionStep {
    pub(crate) fn apply(
        &self,
        content: &mut Content<'_>,
        parser: &Parser,
        _attrlist: Option<&Attrlist<'_>>,
    ) {
        match self {
            Self::SpecialCharacters => {
                apply_special_characters(content, &*parser.renderer);
            }
            Self::AttributeReferences => {
                apply_attributes(content, parser);
            }
            // The five steps whose string implementations went with the
            // pipeline (design §5.2 step 6's tail). Their tree
            // implementations live in
            // [`inline_builder`](crate::content::inline_builder) and run
            // through [`SubstitutionGroup::apply`](super::SubstitutionGroup);
            // nothing applies one directly any more, so this arm exists only
            // to satisfy match exhaustiveness.
            step => unreachable!(
                "the string implementation of {step:?} is deleted; apply the step through a \
                 SubstitutionGroup"
            ),
        }
    }
}

fn apply_special_characters(content: &mut Content<'_>, renderer: &dyn InlineRenderer) {
    if !content.rendered.contains(['<', '>', '&']) {
        return;
    }

    let replacer = SpecialCharacterReplacer { renderer };

    // The guard above guarantees at least one of `<`, `>`, `&` is present, so
    // `replace_all` always rewrites the text and returns `Cow::Owned`, which
    // `into_owned` then unwraps without copying. Seeding a working buffer with
    // `to_string()` first would be a second, wholly redundant heap allocation.
    // (A `Cow::Borrowed` cannot occur here; were it ever to, `into_owned` would
    // clone the unchanged text, which is still correct.)
    let rendered = SPECIAL_CHARS
        .replace_all(content.rendered.as_ref(), replacer)
        .into_owned();

    content.rendered = rendered.into();
}

static SPECIAL_CHARS: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new("[<>&]").unwrap()
});

#[derive(Debug)]
struct SpecialCharacterReplacer<'r> {
    renderer: &'r dyn InlineRenderer,
}

impl Replacer for SpecialCharacterReplacer<'_> {
    fn replace_append(&mut self, caps: &Captures<'_>, dest: &mut String) {
        // The SPECIAL_CHARS regex only matches '<', '>', and '&'. This sequence
        // is specifically constructed to avoid having any unreachable
        // code.
        let ch = &caps[0];

        if ch == "<" {
            self.renderer
                .render_special_character(SpecialCharacter::Lt, dest);
        } else if ch == ">" {
            self.renderer
                .render_special_character(SpecialCharacter::Gt, dest);
        } else if ch == "&" {
            self.renderer
                .render_special_character(SpecialCharacter::Ampersand, dest);
        }

        // No other cases _should_ occur, but if they do, we'll fail safely by
        // not writing anything into dest.
    }
}

static QUOTED_TEXT_SNIFF: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new("[*_`#^~]").unwrap()
});

/// One quoted-text recognition rule: a [`QuoteType`]/[`QuoteScope`] pairing and
/// the [`Regex`] that recognizes it.
///
/// The rules are `pub(crate)` (via [`quote_subs`]) so the single-pass
/// [`inline_builder`](crate::content::inline_builder) reuses the *exact* same
/// patterns the string pipeline matches with — the design's core principle of
/// changing the recognition *sink*, not the recognition itself (§4.1).
pub(crate) struct QuoteSub {
    pub(crate) type_: QuoteType,
    pub(crate) scope: QuoteScope,
    pub(crate) pattern: Regex,
}

/// The ordered quoted-text recognition rules, shared with the single-pass
/// [`inline_builder`](crate::content::inline_builder). The order is
/// significant: it encodes Asciidoctor's precedence (see [`QUOTE_SUBS`]).
pub(crate) fn quote_subs() -> &'static [QuoteSub] {
    &QUOTE_SUBS
}

/// Reports whether `text` contains any character that could open a quoted-text
/// construct. A cheap pre-filter (shared with the single-pass builder) that
/// lets a caller skip the full pattern sweep when nothing quote-like is
/// present.
pub(crate) fn maybe_has_quotes(text: &str) -> bool {
    QUOTED_TEXT_SNIFF.is_match(text)
}

// Adapted from QUOTE_SUBS in Ruby Asciidoctor implementation,
// found in https://github.com/asciidoctor/asciidoctor/blob/main/lib/asciidoctor.rb#L440.
//
// Translation notes:
// * The `\m` modifier on Ruby regex means the `.` pattern *can* match a new
//   line. We use the `.dot_matches_new_line(true)` option on `RegexBuilder` to
//   implement this instead.
// * The `(?!#{CG_WORD})` look-ahead syntax is not available in Rust regex. It
//   looks like the `\b{end-half}` pattern can take its place. (This pattern
//   requires that a non-word character or end of haystack follow the match
//   point.)
// * `#{CC_ALL}` just means any character (`.`).
// * Replace `#{QuoteAttributeListRxt}` with `\\[([^\\[\\]]+)\\]`. (This seems
//   preferable to having yet another level of backslash escaping.)
//
// Notes from the original Ruby implementation:
// * Unconstrained quotes can appear anywhere.
// * Constrained quotes must be bordered by non-word characters.
// * NOTE: These substitutions are processed in the order they appear here and
//   the order in which they are replaced is important.
static QUOTE_SUBS: LazyLock<Vec<QuoteSub>> = LazyLock::new(|| {
    vec![
        QuoteSub {
            // **strong**
            type_: QuoteType::Strong,
            scope: QuoteScope::Unconstrained,
            #[allow(clippy::unwrap_used)]
            pattern: RegexBuilder::new(r#"\\?(?:\[([^\[\]]+)\])?\*\*(.+?)\*\*"#)
                .dot_matches_new_line(true)
                .build()
                .unwrap(),
        },
        QuoteSub {
            // *strong*
            type_: QuoteType::Strong,
            scope: QuoteScope::Constrained,
            #[allow(clippy::unwrap_used)]
            pattern: RegexBuilder::new(
                r#"(^|[^\w&;:}])(?:\[([^\[\]]+)\])?\*(\S|\S.*?\S)\*\b{end-half}"#,
            )
            .dot_matches_new_line(true)
            .build()
            .unwrap(),
        },
        QuoteSub {
            // "`double-quoted`"
            type_: QuoteType::DoubleQuote,
            scope: QuoteScope::Constrained,
            #[allow(clippy::unwrap_used)]
            pattern: RegexBuilder::new(
                r#"(^|[^\w&;:}])(?:\[([^\[\]]+)\])?"`(\S|\S.*?\S)`"\b{end-half}"#,
            )
            .dot_matches_new_line(true)
            .build()
            .unwrap(),
        },
        QuoteSub {
            // '`single-quoted`'
            type_: QuoteType::SingleQuote,
            scope: QuoteScope::Constrained,
            #[allow(clippy::unwrap_used)]
            pattern: RegexBuilder::new(
                r#"(^|[^\w&;:}])(?:\[([^\[\]]+)\])?'`(\S|\S.*?\S)`'\b{end-half}"#,
            )
            .dot_matches_new_line(true)
            .build()
            .unwrap(),
        },
        QuoteSub {
            // ``monospaced``
            type_: QuoteType::Monospaced,
            scope: QuoteScope::Unconstrained,
            #[allow(clippy::unwrap_used)]
            pattern: RegexBuilder::new(r#"\\?(?:\[([^\[\]]+)\])?``(.+?)``"#)
                .dot_matches_new_line(true)
                .build()
                .unwrap(),
        },
        QuoteSub {
            // `monospaced`
            type_: QuoteType::Monospaced,
            scope: QuoteScope::Constrained,
            #[allow(clippy::unwrap_used)]
            pattern: RegexBuilder::new(
                r#"(^|[^\w&;:"'`}])(?:\[([^\[\]]+)\])?`(\S|\S.*?\S)`\b{end-half}"#,
                // NB: We don't have look-ahead in Rust Regex, so we might miss some edge cases
                // because Ruby's version matches `(?![#{CC_WORD}"'`])` which is slightly more
                // detailed than our `\b{end-half}`.
            )
            .dot_matches_new_line(true)
            .build()
            .unwrap(),
        },
        QuoteSub {
            // __emphasis__
            type_: QuoteType::Emphasis,
            scope: QuoteScope::Unconstrained,
            #[allow(clippy::unwrap_used)]
            pattern: RegexBuilder::new(r#"\\?(?:\[([^\[\]]+)\])?__(.+?)__"#)
                .dot_matches_new_line(true)
                .build()
                .unwrap(),
        },
        QuoteSub {
            // _emphasis_
            type_: QuoteType::Emphasis,
            scope: QuoteScope::Constrained,
            #[allow(clippy::unwrap_used)]
            pattern: RegexBuilder::new(
                r#"(^|[^\w&;:}])(?:\[([^\[\]]+)\])?_(\S|\S.*?\S)_\b{end-half}"#,
            )
            .dot_matches_new_line(true)
            .build()
            .unwrap(),
        },
        QuoteSub {
            // ##mark##
            type_: QuoteType::Mark,
            scope: QuoteScope::Unconstrained,
            #[allow(clippy::unwrap_used)]
            pattern: RegexBuilder::new(r#"\\?(?:\[([^\[\]]+)\])?##(.+?)##"#)
                .dot_matches_new_line(true)
                .build()
                .unwrap(),
        },
        QuoteSub {
            // #mark#
            type_: QuoteType::Mark,
            scope: QuoteScope::Constrained,
            #[allow(clippy::unwrap_used)]
            pattern: RegexBuilder::new(
                r#"(^|[^\w&;:}])(?:\[([^\[\]]+)\])?#(\S|\S.*?\S)#\b{end-half}"#,
            )
            .dot_matches_new_line(true)
            .build()
            .unwrap(),
        },
        QuoteSub {
            // ^superscript^
            type_: QuoteType::Superscript,
            scope: QuoteScope::Unconstrained,
            #[allow(clippy::unwrap_used)]
            pattern: Regex::new(r#"\\?(?:\[([^\[\]]+)\])?\^(\S+?)\^"#).unwrap(),
        },
        QuoteSub {
            // ~subscript~
            type_: QuoteType::Subscript,
            scope: QuoteScope::Unconstrained,
            #[allow(clippy::unwrap_used)]
            pattern: Regex::new(r#"\\?(?:\[([^\[\]]+)\])?~(\S+?)~"#).unwrap(),
        },
    ]
});

pub(crate) static ATTRIBUTE_REFERENCE: LazyLock<Regex> = LazyLock::new(|| {
    // Either a `counter`/`counter2` directive (group 2) with its `name[:seed]`
    // expression (group 3), or a plain attribute name (group 4). This mirrors
    // the `counter2?:` branch of Asciidoctor's `AttributeReferenceRx`.
    //
    // Groups 1 and 5 capture the optional escaping backslash before the opening
    // (`\{name}`) and closing (`{name\}`) brace, respectively; either one marks
    // the reference escaped. This mirrors Asciidoctor's
    // `(\\)?\{…(\\)?\}`, whose `$1`/`$4` capture the same two backslashes.
    //
    // The counter expression is matched non-greedily (`+?`) so a trailing
    // escape backslash (`{counter:n\}`) is left for group 5 rather than being
    // swallowed into the expression, again matching Asciidoctor's
    // `#{CC_ANY}+?`.
    //
    // The attribute-name class `\w` (Unicode `\p{Word}`) accepts any Unicode
    // word character, matching Asciidoctor's `#{CG_WORD}[#{CC_WORD}-]*`, so
    // references such as `{café}` and `{سمن}` resolve. It is the same class
    // used to recognize and sanitize an attribute-entry name (see
    // `is_word_char`), so a name and a reference to it always agree.
    #[allow(clippy::unwrap_used)]
    Regex::new(r#"(\\)?\{(?:(counter2?):([^{}]+?)|(\w[\w-]*))(\\)?\}"#).unwrap()
});

/// How the processor handles a reference to a missing attribute, controlled by
/// the [`attribute-missing`] document attribute.
///
/// [`attribute-missing`]: https://docs.asciidoctor.org/asciidoc/latest/attributes/unresolved-references/#missing
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AttributeMissing {
    /// Leave the reference in place (the default).
    Skip,

    /// Drop the reference, but not the line that contains it.
    Drop,

    /// Drop the entire line on which the reference occurs.
    DropLine,

    /// Leave the reference in place and record a warning.
    Warn,
}

impl AttributeMissing {
    /// Resolves the `attribute-missing` setting from `parser`. An absent or
    /// unrecognized value falls back to [`Skip`](Self::Skip), matching
    /// Asciidoctor.
    pub(crate) fn from_parser(parser: &Parser) -> Self {
        match parser.attribute_value("attribute-missing").as_maybe_str() {
            Some("drop") => Self::Drop,
            Some("drop-line") => Self::DropLine,
            Some("warn") => Self::Warn,
            _ => Self::Skip,
        }
    }
}

/// Locates `attribute-missing=warn` warnings within a single line.
///
/// # Why a per-line, positional correlation
///
/// Attribute references are replaced during the *attributes* substitution,
/// which operates on [`Content::rendered`] — text that earlier steps (special
/// characters, quotes) have already transformed, and from which passthroughs
/// have been masked to placeholder tokens. A byte offset in that rendered text
/// therefore has no constant delta back to the original source `Span`, so a
/// warning cannot simply slice `content.original()` at the rendered offset.
///
/// The positional per-line correlation this once carried to recover a precise
/// span from a rendered offset (issue #564's approach 3) is **retired**: block
/// content is substituted by the single-pass builder now, which recognizes
/// each reference against `'src` and hands its own node span to
/// [`record_builder_diagnostic`](crate::Parser) — the honest answer the
/// correlation only approximated.
///
/// What is left is the two shapes this replacer still serves, and neither
/// needs it:
///
/// - A haystack that **is** its own source text, which is the macro-target path
///   ([`substitute_attributes_in_macro_target`]): a match's own offsets are
///   source offsets, so the span is sliced directly (see
///   [`over_its_own_source`](Self::over_its_own_source)).
/// - A haystack rendered from somewhere else — an author line, a docinfo file,
///   a `Custom` group applied to a caller's string — where no source mapping
///   exists to recover and the warning names
///   [`fallback_source`](Self::fallback_source), exactly as it did before #564.
#[derive(Debug)]
struct AttributeReplacer<'p> {
    parser: &'p Parser,

    /// How to handle a reference to a missing attribute.
    mode: AttributeMissing,

    /// Source span used to locate a recorded warning when a precise
    /// per-reference span cannot be recovered. This is the whole content (or
    /// line/target) span — the coarse fallback described in the type-level
    /// docs.
    fallback_source: Span<'p>,

    /// Whether the haystack being replaced is exactly
    /// [`fallback_source`](Self::fallback_source)'s own text, so a match's
    /// offsets are source offsets and a warning can name the precise
    /// reference. See [`over_its_own_source`](Self::over_its_own_source).
    haystack_is_source: bool,

    /// Set to `true` when a (non-escaped) reference to a missing attribute is
    /// dropped, under either [`AttributeMissing::Drop`] or
    /// [`AttributeMissing::DropLine`], so the caller can drop the line: the
    /// whole line in `drop-line` mode, or a line the dropped reference left
    /// empty in `drop` mode (Asciidoctor's `reject_if_empty`).
    missing_on_line: bool,
}

impl<'p> AttributeReplacer<'p> {
    /// Builds the replacer over a haystack rendered from somewhere other than
    /// `fallback_source` itself, so a recorded warning names that coarse span.
    fn new(parser: &'p Parser, mode: AttributeMissing, fallback_source: Span<'p>) -> Self {
        Self {
            parser,
            mode,
            fallback_source,
            haystack_is_source: false,
            missing_on_line: false,
        }
    }

    /// Marks that the haystack being replaced **is** `fallback_source`'s own
    /// text, which lets a warning name the exact reference rather than the
    /// whole span: the match offsets the regex reports are source offsets.
    fn over_its_own_source(mut self) -> Self {
        self.haystack_is_source = true;
        self
    }

    /// Records this step's `attribute-missing` diagnostic — unless the
    /// single-pass builder is going to record it instead.
    ///
    /// The fifth and last of the recognition diagnostics the tree-walk replay
    /// cannot carry (design §5.2 Phase 4, step 6): a dropped or warned-about
    /// reference leaves no node to hang a diagnostic on, so the builder records
    /// it at its own recognition site (see
    /// [`apply_attribute_references`](crate::content::inline_builder)) and it
    /// is carried onto the real parser afterwards. A direct
    /// [`SubstitutionStep::AttributeReferences`] call never runs inside a
    /// build, so this copy diagnoses unconditionally — which is what lets this
    /// step go on being tested in isolation.
    fn record_missing_reference(&self, caps: &Captures<'_>, attr_name: &str) {
        self.parser.record_substitution_warning(
            self.warning_source(caps),
            WarningType::SkippingReferenceToMissingAttribute(attr_name.to_string()),
        );
    }

    /// The source span to attribute a recorded warning to: the offending
    /// reference itself where the haystack is its own source, and the coarse
    /// [`fallback_source`](Self::fallback_source) otherwise.
    fn warning_source(&self, caps: &Captures<'_>) -> Span<'p> {
        if !self.haystack_is_source {
            return self.fallback_source;
        }

        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let range = caps.get(0).unwrap().range();

        self.fallback_source.slice(range)
    }
}

impl Replacer for AttributeReplacer<'_> {
    fn replace_append(&mut self, caps: &Captures<'_>, dest: &mut String) {
        // A backslash immediately before the opening brace (`\{name}`) or
        // before the closing brace (`{name\}`) — or both, as in
        // `\{name\}` — escapes the reference: it is emitted literally
        // with the escaping backslash(es) removed and left unexpanded,
        // whether or not the attribute is set. An escaped reference is
        // never treated as a missing reference, so it neither drops the
        // line nor warns, and an escaped counter directive
        // does not advance the counter. This mirrors Asciidoctor, whose
        // `sub_attributes` returns `{#{name}}` when either its leading (`$1`)
        // or trailing (`$4`) backslash capture is present, before any
        // counter, missing-attribute, or resolution handling runs.
        if caps.get(1).is_some() || caps.get(5).is_some() {
            dest.push('{');

            // Groups 2 (the `counter`/`counter2` directive) and 3 (its
            // expression) participate together; a plain reference is group 4.
            if let Some(directive) = caps.get(2) {
                dest.push_str(directive.as_str());
                dest.push(':');
                dest.push_str(&caps[3]);
            } else {
                dest.push_str(&caps[4]);
            }

            dest.push('}');
            return;
        }

        // A `counter`/`counter2` directive resolves (and advances) a counter
        // rather than looking up an existing attribute.
        if let Some(directive) = caps.get(2) {
            // Group 3 always participates when group 2 does (same alternation
            // branch). The expression is `name` or `name:seed`.
            let mut parts = caps[3].splitn(2, ':');
            let name = parts.next().unwrap_or_default();
            let seed = parts.next();

            let value = self.parser.counter(name, seed);

            // `counter` displays the new value; `counter2` advances silently.
            if directive.as_str() == "counter" {
                dest.push_str(&value);
            }
            return;
        }

        // Otherwise this is a plain attribute reference (group 4).
        let attr_name = &caps[4];

        // Resolve the reference case-insensitively: attribute names are stored
        // lower-cased (both an attribute-entry definition and an API-supplied
        // attribute fold their name), so the lookup name is folded the same way
        // here. This mirrors Asciidoctor's `sub_attributes`, which looks up
        // `key = $2.downcase`. The original spelling is still what is emitted
        // literally for a skipped or missing reference below.
        let lookup_name = attribute_lookup_name(attr_name);
        let value = self.parser.attribute_value(&lookup_name);

        // A reference is "missing" for `attribute-missing` purposes both when
        // the attribute was never assigned at all and when it was explicitly
        // unset (a document `:name!:` entry or an API override that unsets
        // it) — both resolve to `InterpretedValue::Unset`. Only a value-less
        // `Set` attribute or a concrete `Value` counts as present.
        if !self.parser.has_attribute(&lookup_name) || matches!(value, InterpretedValue::Unset) {
            match self.mode {
                AttributeMissing::Skip => dest.push_str(&caps[0]),
                AttributeMissing::Drop => {
                    // Drop the reference, leaving the rest of the line intact.
                    // Flag that a missing reference was dropped here so the
                    // caller can remove the line if the drop emptied it
                    // (Asciidoctor's `reject_if_empty`).
                    self.missing_on_line = true;
                }
                AttributeMissing::DropLine => {
                    // Mark the line for removal; whatever is written to `dest`
                    // here is discarded with it. Asciidoctor logs an `INFO`
                    // message ("dropping line containing reference to missing
                    // attribute") for each reference that triggers a drop, so
                    // record the matching diagnostic here.
                    self.missing_on_line = true;
                    self.record_missing_reference(caps, attr_name);
                }
                AttributeMissing::Warn => {
                    dest.push_str(&caps[0]);
                    self.record_missing_reference(caps, attr_name);
                }
            }
            return;
        }

        // A value-less `Set` attribute (e.g. `:foo:` with no `=value`)
        // substitutes to an empty string, matching Asciidoctor.
        if let InterpretedValue::Value(value) = value {
            dest.push_str(value.as_ref());
        }
    }
}

/// Whether a line that dropped a missing reference under
/// [`AttributeMissing::Drop`] should be treated as emptied (and therefore
/// removed, per Asciidoctor's `reject_if_empty`).
///
/// A trailing `\r` left from a CRLF terminator is part of the line ending, not
/// content: a line the drop reduced to just `\r` still counts as empty. The
/// block pipeline strips `\r` before content is assembled, but free-standing
/// text (a docinfo file) is split on `\n` with the `\r` intact, so this guard
/// is what makes a CRLF reference-only line drop there.
fn drop_emptied_line(replaced: &str) -> bool {
    replaced.strip_suffix('\r').unwrap_or(replaced).is_empty()
}

fn apply_attributes(content: &mut Content<'_>, parser: &Parser) {
    if !content.rendered.contains('{') {
        return;
    }

    let mode = AttributeMissing::from_parser(parser);
    let source = content.original();

    // Attribute references are replaced line by line so that, in `drop-line`
    // mode, an individual line carrying a missing reference can be removed
    // without disturbing the lines around it. A reference cannot span a line
    // break, so this matches what a single whole-text pass would produce for
    // every other mode.
    let mut out = String::with_capacity(content.rendered.len());
    let mut changed = false;
    let mut wrote_line = false;

    for line in content.rendered.split('\n') {
        if !line.contains('{') {
            if wrote_line {
                out.push('\n');
            }
            out.push_str(line);
            wrote_line = true;
            continue;
        }

        let mut replacer = AttributeReplacer::new(parser, mode, source);

        let replaced = ATTRIBUTE_REFERENCE.replace_all(line, replacer.by_ref());

        if replacer.missing_on_line
            && (mode == AttributeMissing::DropLine
                || (mode == AttributeMissing::Drop && drop_emptied_line(&replaced)))
        {
            // Drop the entire line, including its line break: unconditionally
            // in `drop-line` mode, or in `drop` mode when the dropped
            // reference was all the line contained (Asciidoctor's
            // `reject_if_empty`).
            changed = true;
            continue;
        }

        if let Cow::Owned(_) = replaced {
            changed = true;
        }

        if wrote_line {
            out.push('\n');
        }
        out.push_str(&replaced);
        wrote_line = true;
    }

    // If nothing was replaced or dropped, leave the (borrowed) rendering as-is
    // rather than paying for the rebuilt string.
    if changed {
        content.rendered = out.into();
    }
}

/// Applies the attribute-references substitution to a block macro target (the
/// portion between the `::` and the `[` of an `image::`, `video::`, or
/// `audio::` macro), honoring the [`attribute-missing`] document attribute.
///
/// Block macro targets are always a single line, so (unlike
/// [`apply_attributes`]) there is no line splitting. Returns `None` when the
/// target references a missing attribute under
/// [`AttributeMissing::DropLine`] — signaling that the entire block should be
/// dropped — and otherwise returns the substituted target.
///
/// [`attribute-missing`]: https://docs.asciidoctor.org/asciidoc/latest/attributes/unresolved-references/#missing
pub(crate) fn substitute_attributes_in_macro_target<'src>(
    target: Span<'src>,
    parser: &Parser,
) -> Option<CowStr<'src>> {
    let text = target.data();

    // Without a reference there is nothing to substitute (and nothing that
    // could trigger a drop), so the borrowed target is returned as-is.
    if !text.contains('{') {
        return Some(text.into());
    }

    let mode = AttributeMissing::from_parser(parser);

    // The haystack is the target's own text, so a match's offsets are source
    // offsets and a warning names the exact reference.
    let mut replacer = AttributeReplacer::new(parser, mode, target).over_its_own_source();

    let replaced = ATTRIBUTE_REFERENCE.replace_all(text, replacer.by_ref());

    if replacer.missing_on_line && mode == AttributeMissing::DropLine {
        return None;
    }

    Some(replaced.into())
}

/// Substitutes attribute references in a block anchor's reftext
/// (`[[id,reftext]]`) against the attributes in effect where the anchor
/// appears, returning the resolved text. This mirrors how attribute references
/// in a block ID (`[#install-{platform-id}]`) or a `reftext=` attribute are
/// resolved when the attribute list is parsed, so the reftext is registered in
/// the catalog with its attributes already expanded and a cross reference by
/// that text resolves.
///
/// The borrowed source text is returned unchanged when it holds no attribute
/// reference, avoiding an allocation in the common case.
pub(crate) fn substitute_attributes_in_reftext<'src>(
    reftext: Span<'src>,
    parser: &Parser,
) -> CowStr<'src> {
    if !reftext.data().contains('{') {
        return reftext.data().into();
    }

    let mut content = Content::from(reftext);
    SubstitutionStep::AttributeReferences.apply(&mut content, parser, None);
    CowStr::from(content.rendered.to_string())
}

/// One character-replacement recognition rule: a
/// [`CharacterReplacementType`] and the [`Regex`] that recognizes it.
///
/// The rules are `pub(crate)` (via [`character_replacements`]) so the
/// single-pass [`inline_builder`](crate::content::inline_builder) reuses the
/// *exact* same patterns the string pipeline matches with — the design's core
/// principle of changing the recognition *sink*, not the recognition itself
/// (§4.1). This mirrors how [`quote_subs`] is shared.
pub(crate) struct CharacterReplacement {
    pub(crate) type_: CharacterReplacementType,
    pub(crate) pattern: Regex,
}

/// The ordered character-replacement recognition rules, shared with the
/// single-pass [`inline_builder`](crate::content::inline_builder). The order is
/// significant: it encodes Asciidoctor's precedence (see [`REPLACEMENTS`]).
pub(crate) fn character_replacements() -> &'static [CharacterReplacement] {
    &REPLACEMENTS
}

/// Reports whether `text` contains any character that could open a
/// character-replacement construct. A cheap pre-filter (shared with the
/// single-pass builder) that lets a caller skip the full pattern sweep when
/// nothing replaceable is present.
pub(crate) fn maybe_has_replacements(text: &str) -> bool {
    REPLACEABLE_TEXT_SNIFF.is_match(text)
}

/// The hard-line-break recognition pattern (a line ending in ` +`), shared with
/// the single-pass [`inline_builder`](crate::content::inline_builder).
pub(crate) fn hard_line_break_pattern() -> &'static Regex {
    &HARD_LINE_BREAK
}

static REPLACEABLE_TEXT_SNIFF: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r#"[&']|--|\.\.\.|\([CRT]M?\)"#).unwrap()
});

// Adapted from REPLACEMENTS in Ruby Asciidoctor implementation,
// found in https://github.com/asciidoctor/asciidoctor/blob/main/lib/asciidoctor.rb#L490.
//
// * NOTE: These substitutions are processed in the order they appear here and
//   the order in which they are replaced is important.
static REPLACEMENTS: LazyLock<Vec<CharacterReplacement>> = LazyLock::new(|| {
    vec![
        CharacterReplacement {
            // Copyright `(C)`
            type_: CharacterReplacementType::Copyright,
            #[allow(clippy::unwrap_used)]
            pattern: Regex::new(r#"\\?\(C\)"#).unwrap(),
        },
        CharacterReplacement {
            // Registered `(R)`
            type_: CharacterReplacementType::Registered,
            #[allow(clippy::unwrap_used)]
            pattern: Regex::new(r#"\\?\(R\)"#).unwrap(),
        },
        CharacterReplacement {
            // Trademark `(TM)`
            type_: CharacterReplacementType::Trademark,
            #[allow(clippy::unwrap_used)]
            pattern: Regex::new(r#"\\?\(TM\)"#).unwrap(),
        },
        CharacterReplacement {
            // Em dash surrounded by spaces ` -- `
            type_: CharacterReplacementType::EmDashSurroundedBySpaces,
            #[allow(clippy::unwrap_used)]
            pattern: Regex::new(r#"(?: |\n|^|\\)--(?: |\n|$)"#).unwrap(),
        },
        CharacterReplacement {
            // Em dash without spaces `--`
            type_: CharacterReplacementType::EmDashWithoutSpace,
            #[allow(clippy::unwrap_used)]
            pattern: Regex::new(r#"(\w)\\?--\b{start-half}"#).unwrap(),
        },
        CharacterReplacement {
            // Ellipsis `...`
            type_: CharacterReplacementType::Ellipsis,
            #[allow(clippy::unwrap_used)]
            pattern: Regex::new(r#"\\?\.\.\."#).unwrap(),
        },
        CharacterReplacement {
            // Right single quote `\`'`
            type_: CharacterReplacementType::TypographicApostrophe,
            #[allow(clippy::unwrap_used)]
            pattern: Regex::new(r#"\\?`'"#).unwrap(),
        },
        CharacterReplacement {
            // Apostrophe (inside a word)
            type_: CharacterReplacementType::TypographicApostrophe,
            #[allow(clippy::unwrap_used)]
            pattern: Regex::new(r#"([[:alnum:]])\\?'([[:alpha:]])"#).unwrap(),
        },
        CharacterReplacement {
            // Right arrow `->`
            type_: CharacterReplacementType::SingleRightArrow,
            #[allow(clippy::unwrap_used)]
            pattern: Regex::new(r#"\\?-&gt;"#).unwrap(),
        },
        CharacterReplacement {
            // Right double arrow `=>`
            type_: CharacterReplacementType::DoubleRightArrow,
            #[allow(clippy::unwrap_used)]
            pattern: Regex::new(r#"\\?=&gt;"#).unwrap(),
        },
        CharacterReplacement {
            // Left arrow `<-`
            type_: CharacterReplacementType::SingleLeftArrow,
            #[allow(clippy::unwrap_used)]
            pattern: Regex::new(r#"\\?&lt;-"#).unwrap(),
        },
        CharacterReplacement {
            // Left double arrow `<=`
            type_: CharacterReplacementType::DoubleLeftArrow,
            #[allow(clippy::unwrap_used)]
            pattern: Regex::new(r#"\\?&lt;="#).unwrap(),
        },
        CharacterReplacement {
            // Restore entities
            type_: CharacterReplacementType::CharacterReference("".to_owned()),
            #[allow(clippy::unwrap_used)]
            pattern: Regex::new(&format!(r#"\\?&amp;({ENTITY_NAME});"#)).unwrap(),
        },
    ]
});

/// The name an entity reference must carry for the **restore entities**
/// replacement rule above to recognize it: a named entity (`copy`, `hellip`,
/// `frac12`), a decimal numeric reference (`#8217`), or a hexadecimal one
/// (`#x2014`).
///
/// Shared with [`restored_entity_pattern`] so the two spellings of the same
/// class — the rule that *produces* a restored entity, and the pattern that
/// recognizes one already produced — cannot drift.
const ENTITY_NAME: &str =
    r#"(?:[a-zA-Z][a-zA-Z]+\d{0,2}|#\d\d\d{0,4}|#x[\da-fA-F][\da-fA-F][\da-fA-F]{0,3})"#;

/// A **restored** entity reference (`&copy;`, `&#8217;`) as it appears once the
/// **restore entities** replacement rule above has un-escaped it — that is, the
/// same class that rule matches, minus the `&amp;` escaping its `&` had before
/// the rule ran.
///
/// Shared with the single-pass
/// [`inline_builder`](crate::content::inline_builder), which needs to find such
/// an entity inside a value a macro family *computed* off an already-escaped
/// string, so the entity becomes its own
/// [`CharRef`](crate::inlines::InlineNode::CharRef)`::Entity` child rather than
/// text a fold would escape a second time.
pub(crate) fn restored_entity_pattern() -> &'static Regex {
    &RESTORED_ENTITY
}

static RESTORED_ENTITY: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(&format!(r#"^&{ENTITY_NAME};"#)).unwrap()
});

static HARD_LINE_BREAK: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r#"(?m)^(.*) \+$"#).unwrap()
});

/// Callout regex for the default `line-comment` mode: recognizes the common
/// line-comment prefixes and XML callouts.
static DEFAULT_CALLOUT_RX: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(
        r"(?P<prefix>(?://|#|--|;;) ?)?(?P<esc>\\)?(?:&lt;!--(?P<xnum>\d+|\.)--&gt;|&lt;(?P<num>\d+|\.)&gt;)",
    )
    .unwrap()
});

/// Trailing-position lookahead regex for the default `line-comment` mode.
static DEFAULT_CALLOUT_TAIL_RX: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r"^(?: ?\\?(?:&lt;!--(?:\d+|\.)--&gt;|&lt;(?:\d+|\.)&gt;))*(?:\n|$)").unwrap()
});

/// Trailing-position lookahead regex for a custom or empty `line-comment` mode
/// (no XML callout form).
static CUSTOM_CALLOUT_TAIL_RX: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r"^(?: ?\\?&lt;(?:\d+|\.)&gt;)*(?:\n|$)").unwrap()
});

/// Builds the `(callout, tail)` regex pair for the given `line-comment` mode.
///
/// The `callout` regex matches a single callout token (with the optional
/// line-comment prefix and escape that may precede it). The `tail` regex is
/// used to emulate Asciidoctor's trailing-position lookahead: a matched callout
/// is only honored when the remainder of its line consists solely of further
/// callouts. Rust's regex engine supports neither lookahead nor backreferences,
/// so the lookahead is applied manually against the post-match text.
///
/// The default-mode regexes and both tail regexes are constant, so they are
/// built once. Only a custom (non-empty) prefix requires building a regex from
/// the attribute value, which is borrowed otherwise.
pub(crate) fn build_callout_regexes(
    line_comment: Option<&str>,
) -> (Cow<'static, Regex>, &'static Regex) {
    match line_comment {
        // Default: recognize the common line-comment prefixes and XML callouts.
        None => (Cow::Borrowed(&DEFAULT_CALLOUT_RX), &DEFAULT_CALLOUT_TAIL_RX),

        // A custom or empty `line-comment`: only the bare (non-XML) callout form
        // is recognized, optionally behind the custom prefix.
        Some(prefix) => {
            let prefix_pattern = if prefix.is_empty() {
                String::new()
            } else {
                format!(r"(?P<prefix>{} ?)?", regex::escape(prefix))
            };

            #[allow(clippy::unwrap_used)]
            let callout = Regex::new(&format!(
                r"{prefix_pattern}(?P<esc>\\)?&lt;(?P<num>\d+|\.)&gt;"
            ))
            .unwrap();

            (Cow::Owned(callout), &CUSTOM_CALLOUT_TAIL_RX)
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    // Pins (and covers) the exhaustiveness arm in `SubstitutionStep::apply`:
    // the five steps whose string implementations went with the pipeline
    // refuse direct application rather than silently doing nothing.
    #[test]
    #[should_panic(expected = "the string implementation of Quotes is deleted")]
    fn a_deleted_steps_direct_application_is_refused() {
        let mut content = crate::content::Content::from(crate::Span::new("x"));

        crate::content::SubstitutionStep::Quotes.apply(
            &mut content,
            &crate::Parser::default(),
            None,
        );
    }

    mod special_characters {
        use crate::{
            content::{Content, SubstitutionStep},
            strings::CowStr,
            tests::prelude::*,
        };

        #[test]
        fn empty() {
            let mut content = Content::from(crate::Span::default());
            let p = Parser::default();
            SubstitutionStep::SpecialCharacters.apply(&mut content, &p, None);
            assert!(content.is_empty());
            assert_eq!(content.rendered, CowStr::Borrowed(""));
        }

        #[test]
        fn basic_non_empty_span() {
            let mut content = Content::from(crate::Span::new("blah"));
            let p = Parser::default();
            SubstitutionStep::SpecialCharacters.apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(content.rendered, CowStr::Borrowed("blah"));
        }

        #[test]
        fn match_lt_and_gt() {
            let mut content = Content::from(crate::Span::new("bl<ah>"));
            let p = Parser::default();
            SubstitutionStep::SpecialCharacters.apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(
                content.rendered,
                CowStr::Boxed("bl&lt;ah&gt;".to_string().into_boxed_str())
            );
        }

        #[test]
        fn match_amp() {
            let mut content = Content::from(crate::Span::new("bl<a&h>"));
            let p = Parser::default();
            SubstitutionStep::SpecialCharacters.apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(
                content.rendered,
                CowStr::Boxed("bl&lt;a&amp;h&gt;".to_string().into_boxed_str())
            );
        }
    }

    mod quotes {
        use crate::{
            content::{Content, SubstitutionStep},
            strings::CowStr,
            tests::prelude::*,
        };

        #[test]
        fn empty() {
            let mut content = Content::from(crate::Span::default());
            let p = Parser::default();
            SubstitutionGroup::Custom(vec![SubstitutionStep::Quotes]).apply(&mut content, &p, None);
            assert!(content.is_empty());
            assert_eq!(content.rendered, CowStr::Borrowed(""));
        }

        #[test]
        fn basic_non_empty_span() {
            let mut content = Content::from(crate::Span::new("blah"));
            let p = Parser::default();
            SubstitutionGroup::Custom(vec![SubstitutionStep::Quotes]).apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(content.rendered, CowStr::Borrowed("blah"));
        }

        #[test]
        fn ignore_lt_and_gt() {
            let mut content = Content::from(crate::Span::new("bl<ah>"));
            let p = Parser::default();
            SubstitutionGroup::Custom(vec![SubstitutionStep::Quotes]).apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(
                content.rendered,
                CowStr::Boxed("bl<ah>".to_string().into_boxed_str())
            );
        }

        #[test]
        fn strong_word() {
            let mut content = Content::from(crate::Span::new("One *word* is strong."));
            let p = Parser::default();
            SubstitutionGroup::Custom(vec![SubstitutionStep::Quotes]).apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(
                content.rendered,
                CowStr::Boxed(
                    "One <strong>word</strong> is strong."
                        .to_string()
                        .into_boxed_str()
                )
            );
        }

        #[test]
        fn marked_string_with_id() {
            let mut content = Content::from(crate::Span::new(r#"[#id]#a few words#"#));
            let p = Parser::default();
            SubstitutionGroup::Custom(vec![SubstitutionStep::Quotes]).apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(
                content.rendered,
                CowStr::Boxed(r#"<span id="id">a few words</span>"#.to_string().into_boxed_str())
            );
        }

        #[test]
        fn unconstrained_marked_string_with_id_is_registered() {
            // An ID assigned to *unconstrained* quoted text (here, `##...##`)
            // is rendered as the element's `id` and registered in the catalog
            // so the phrase can be the target of a cross reference.
            let doc = Parser::default().parse(r#"[#the_id]##marked text##"#);

            assert_eq!(
                doc.child_blocks()
                    .next()
                    .unwrap()
                    .rendered_html_content()
                    .unwrap(),
                r#"<span id="the_id">marked text</span>"#
            );

            assert!(doc.catalog().contains_id("the_id"));
        }

        #[test]
        fn multibyte_leading_char_before_constrained_monospace() {
            // The constrained-monospace leading boundary group matches any
            // non-word Unicode scalar, so it can begin with a multi-byte
            // character. When the failed look-ahead skips past that leading
            // character, the skip width must honor the character boundary
            // rather than assuming a single byte.
            for leading in ["€", "中", "🎉"] {
                let source = format!("{leading}`code``");
                let mut content = Content::from(crate::Span::new(&source));
                let p = Parser::default();

                // Must not panic on the multi-byte leading character.
                SubstitutionGroup::Custom(vec![SubstitutionStep::Quotes]).apply(
                    &mut content,
                    &p,
                    None,
                );

                assert!(content.rendered.starts_with(leading));
            }
        }

        #[test]
        fn escaped_leading_backtick_before_constrained_monospace() {
            // When the leading boundary character is a backslash, the failed
            // look-ahead skips the backslash plus the following backtick (both
            // ASCII, so two bytes). Exercises the escape branch of the skip
            // width and confirms the escaped text is preserved verbatim.
            let mut content = Content::from(crate::Span::new(r"\`code``"));
            let p = Parser::default();

            SubstitutionGroup::Custom(vec![SubstitutionStep::Quotes]).apply(&mut content, &p, None);

            assert_eq!(
                content.rendered,
                CowStr::Boxed(r"\`code``".to_string().into_boxed_str())
            );
        }
    }

    mod attribute_references {
        use crate::{
            content::{Content, SubstitutionStep},
            strings::CowStr,
            tests::prelude::*,
        };

        #[test]
        fn empty() {
            let mut content = Content::from(crate::Span::default());
            let p = Parser::default();
            SubstitutionStep::AttributeReferences.apply(&mut content, &p, None);
            assert!(content.is_empty());
            assert_eq!(content.rendered, CowStr::Borrowed(""));
        }

        #[test]
        fn basic_non_empty_span() {
            let mut content = Content::from(crate::Span::new("blah"));
            let p = Parser::default();
            SubstitutionStep::AttributeReferences.apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(content.rendered, CowStr::Borrowed("blah"));
        }

        #[test]
        fn ignore_non_match() {
            let mut content = Content::from(crate::Span::new("bl{ah}"));
            let p = Parser::default();
            SubstitutionStep::AttributeReferences.apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(
                content.rendered,
                CowStr::Boxed("bl{ah}".to_string().into_boxed_str())
            );
        }

        #[test]
        fn escaped_reference_to_unset_attribute_drops_backslash() {
            // `ah` is a valid attribute name but is unset. An escaped reference
            // still has its backslash removed and is passed through literally,
            // matching Asciidoctor (see issue #667).
            let mut content = Content::from(crate::Span::new("bl\\{ah}"));
            let p = Parser::default();
            SubstitutionStep::AttributeReferences.apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(
                content.rendered,
                CowStr::Boxed("bl{ah}".to_string().into_boxed_str())
            );
        }

        #[test]
        fn replace_sp_match() {
            let mut content = Content::from(crate::Span::new("bl{sp}ah"));
            let p = Parser::default();
            SubstitutionStep::AttributeReferences.apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(
                content.rendered,
                CowStr::Boxed("bl ah".to_string().into_boxed_str())
            );
        }

        #[test]
        fn ignore_escaped_sp_match() {
            let mut content = Content::from(crate::Span::new("bl\\{sp}ah"));
            let p = Parser::default();
            SubstitutionStep::AttributeReferences.apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(
                content.rendered,
                CowStr::Boxed("bl{sp}ah".to_string().into_boxed_str())
            );
        }

        #[test]
        fn counter_directive_displays_and_advances() {
            let mut content = Content::from(crate::Span::new("{counter:n}-{counter:n}"));
            let p = Parser::default();
            SubstitutionStep::AttributeReferences.apply(&mut content, &p, None);
            assert_eq!(
                content.rendered,
                CowStr::Boxed("1-2".to_string().into_boxed_str())
            );
        }

        #[test]
        fn counter2_directive_advances_silently() {
            let mut content = Content::from(crate::Span::new("{counter2:n}{counter:n}"));
            let p = Parser::default();
            SubstitutionStep::AttributeReferences.apply(&mut content, &p, None);
            assert_eq!(
                content.rendered,
                CowStr::Boxed("2".to_string().into_boxed_str())
            );
        }

        #[test]
        fn escaped_counter_directive_is_literal_and_does_not_advance() {
            let mut content = Content::from(crate::Span::new("\\{counter:n} {counter:n}"));
            let p = Parser::default();
            SubstitutionStep::AttributeReferences.apply(&mut content, &p, None);
            assert_eq!(
                content.rendered,
                CowStr::Boxed("{counter:n} 1".to_string().into_boxed_str())
            );
        }

        #[test]
        fn escaped_reference_with_both_braces_escaped_drops_backslashes() {
            // `\{name\}` escapes the reference the same way `\{name}` does:
            // both backslashes are removed and the reference is
            // left unexpanded, even when the attribute is set. This
            // is the form Asciidoctor produces for `\{group-id\}`.
            let p = Parser::default().with_intrinsic_attribute(
                "group-id",
                "42",
                crate::parser::ModificationContext::Anywhere,
            );

            let mut content = Content::from(crate::Span::new("\\{group-id\\}"));
            SubstitutionStep::AttributeReferences.apply(&mut content, &p, None);
            assert_eq!(
                content.rendered,
                CowStr::Boxed("{group-id}".to_string().into_boxed_str())
            );
        }

        #[test]
        fn escaped_reference_with_only_trailing_brace_escaped_drops_backslash() {
            // A backslash before the closing brace alone (`{name\}`) also
            // escapes the reference, matching Asciidoctor's
            // trailing-backslash capture.
            let p = Parser::default().with_intrinsic_attribute(
                "group-id",
                "42",
                crate::parser::ModificationContext::Anywhere,
            );

            let mut content = Content::from(crate::Span::new("{group-id\\}"));
            SubstitutionStep::AttributeReferences.apply(&mut content, &p, None);
            assert_eq!(
                content.rendered,
                CowStr::Boxed("{group-id}".to_string().into_boxed_str())
            );
        }

        #[test]
        fn escaped_counter_with_trailing_backslash_is_literal_and_does_not_advance() {
            // A trailing escape backslash on a counter directive
            // (`{counter:n\}`) emits the reference literally
            // (without the backslash) and does not advance the
            // counter, so the following unescaped reference is `1`.
            let mut content = Content::from(crate::Span::new("{counter:n\\} {counter:n}"));
            let p = Parser::default();
            SubstitutionStep::AttributeReferences.apply(&mut content, &p, None);
            assert_eq!(
                content.rendered,
                CowStr::Boxed("{counter:n} 1".to_string().into_boxed_str())
            );
        }

        mod attribute_missing {
            #![allow(clippy::indexing_slicing)]

            use crate::{
                Span,
                content::{Content, SubstitutionGroup, SubstitutionStep},
                parser::ModificationContext,
                tests::prelude::*,
                warnings::WarningType,
            };

            fn parser_with_mode(mode: &str) -> Parser {
                Parser::default().with_intrinsic_attribute(
                    "attribute-missing",
                    mode,
                    ModificationContext::Anywhere,
                )
            }

            fn render(text: &str, parser: &Parser) -> String {
                let mut content = Content::from(crate::Span::new(text));
                SubstitutionStep::AttributeReferences.apply(&mut content, parser, None);
                content.rendered.to_string()
            }

            /// Asserts that `warning`'s recorded offset/length select exactly
            /// `expected` out of `text`, i.e. the warning points at that
            /// precise reference in the original source.
            fn assert_spans(warning: &crate::parser::DeferredWarning, text: &str, expected: &str) {
                assert_eq!(
                    &text[warning.offset..warning.offset + warning.len],
                    expected
                );
            }

            #[test]
            fn skip_is_default() {
                let p = Parser::default();
                assert_eq!(render("Hello, {name}!", &p), "Hello, {name}!");
                assert!(p.take_substitution_warnings().is_empty());
            }

            #[test]
            fn skip_explicit() {
                let p = parser_with_mode("skip");
                assert_eq!(render("Hello, {name}!", &p), "Hello, {name}!");
            }

            #[test]
            fn unknown_value_falls_back_to_skip() {
                let p = parser_with_mode("bogus");
                assert_eq!(render("Hello, {name}!", &p), "Hello, {name}!");
            }

            #[test]
            fn drop_removes_only_the_reference() {
                let p = parser_with_mode("drop");
                assert_eq!(render("Hello, {name}!", &p), "Hello, !");
            }

            #[test]
            fn drop_keeps_resolvable_references() {
                let p = parser_with_mode("drop");
                assert_eq!(render("a {sp}b {missing} c", &p), "a  b  c");
            }

            #[test]
            fn drop_removes_line_that_only_contained_the_reference() {
                // A line consisting solely of an unresolved reference is
                // dropped entirely, not left as a blank line (issue #730).
                let p = parser_with_mode("drop");
                assert_eq!(render("Line 1\n{missing}\nLine 2", &p), "Line 1\nLine 2");
            }

            #[test]
            fn drop_keeps_a_line_the_reference_did_not_empty() {
                // The line still has other content after the reference is
                // dropped, so it survives (only the reference is removed).
                let p = parser_with_mode("drop");
                assert_eq!(
                    render("Line 1\ntext {missing}\nLine 2", &p),
                    "Line 1\ntext \nLine 2"
                );
            }

            #[test]
            fn drop_removes_a_leading_or_trailing_reference_only_line() {
                let p = parser_with_mode("drop");
                assert_eq!(render("{missing}\nLine 2", &p), "Line 2");
                assert_eq!(render("Line 1\n{missing}", &p), "Line 1");
            }

            #[test]
            fn drop_can_empty_the_content() {
                // A single line that is only an unresolved reference drops to
                // empty content, mirroring `drop-line`.
                let p = parser_with_mode("drop");
                assert_eq!(render("{missing}", &p), "");
            }

            #[test]
            fn drop_keeps_a_line_emptied_by_a_resolvable_reference() {
                // The line becomes empty, but not because a *missing* reference
                // was dropped, so it is retained.
                let p = parser_with_mode("drop");
                assert_eq!(render("Line 1\n{empty}\nLine 2", &p), "Line 1\n\nLine 2");
            }

            #[test]
            fn drop_line_removes_the_whole_line() {
                let p = parser_with_mode("drop-line");
                assert_eq!(render("Hello, {name}!\nSecond line.", &p), "Second line.");
            }

            #[test]
            fn drop_line_only_drops_lines_with_a_missing_reference() {
                let p = parser_with_mode("drop-line");
                assert_eq!(
                    render("first {sp}line\nsecond {missing} line\nthird line", &p),
                    "first  line\nthird line"
                );
            }

            #[test]
            fn drop_line_can_empty_the_content() {
                let p = parser_with_mode("drop-line");
                assert_eq!(render("{missing}", &p), "");
            }

            #[test]
            fn drop_line_removes_a_line_referencing_an_api_unset_attribute() {
                // An attribute explicitly unset via the API (as opposed to one
                // that was never assigned at all) still counts as "missing" for
                // `attribute-missing` purposes (issue #1117).
                let p = parser_with_mode("drop-line").with_intrinsic_attribute_bool(
                    "version",
                    false,
                    ModificationContext::ApiOnly,
                );
                assert_eq!(render("bootstrap.{version}.min.js", &p), "");
            }

            #[test]
            fn drop_line_records_a_warning_for_the_dropped_reference() {
                // Dropping the line is silent in Asciidoctor's output, but it
                // logs an `INFO` diagnostic naming the missing attribute; the
                // parser records the matching warning.
                let p = parser_with_mode("drop-line");
                assert_eq!(render("Hello, {name}!\nSecond line.", &p), "Second line.");

                let warnings = p.take_substitution_warnings();
                assert_eq!(warnings.len(), 1);
                assert_eq!(
                    warnings[0].warning,
                    WarningType::SkippingReferenceToMissingAttribute("name".to_string())
                );
            }

            #[test]
            fn drop_line_records_one_warning_per_missing_reference() {
                // Two missing references on the same dropped line each produce
                // a diagnostic, matching Asciidoctor's
                // per-reference logging.
                let p = parser_with_mode("drop-line");
                assert_eq!(render("a {x} b {y} c\ntail", &p), "tail");
                assert_eq!(p.take_substitution_warnings().len(), 2);
            }

            #[test]
            fn drop_line_does_not_warn_for_a_line_without_a_missing_reference() {
                // Only the line carrying the missing reference is dropped and
                // warned about; a line whose references all resolve is
                // untouched.
                let p = parser_with_mode("drop-line");
                assert_eq!(
                    render("first {sp}line\nsecond {missing} line\nthird line", &p),
                    "first  line\nthird line"
                );

                let warnings = p.take_substitution_warnings();
                assert_eq!(warnings.len(), 1);
                assert_eq!(
                    warnings[0].warning,
                    WarningType::SkippingReferenceToMissingAttribute("missing".to_string())
                );
            }

            #[test]
            fn drop_line_points_at_the_precise_reference() {
                // With per-line source spans retained, the drop-line diagnostic
                // names the exact offending reference rather than the whole
                // line.
                let p = parser_with_mode("drop-line");
                let text = "first {alpha} line\nsecond {beta} line";
                let mut content = Content::from(Span::new(text));
                SubstitutionGroup::Normal.apply(&mut content, &p, None);

                let warnings = p.take_substitution_warnings();
                assert_eq!(warnings.len(), 2);
                assert_spans(&warnings[0], text, "{alpha}");
                assert_spans(&warnings[1], text, "{beta}");
            }

            #[test]
            fn drop_line_falls_back_to_whole_span_on_a_direct_step_call() {
                // A direct step call substitutes a haystack rendered from
                // somewhere other than its own source, so the drop-line
                // diagnostic names the whole-content span rather than
                // misreporting a location.
                let p = parser_with_mode("drop-line");
                let text = "x {foo} y";
                let mut content = Content::from(Span::new(text));
                SubstitutionStep::AttributeReferences.apply(&mut content, &p, None);

                let warnings = p.take_substitution_warnings();
                assert_eq!(warnings.len(), 1);
                assert_eq!(warnings[0].offset, 0);
                assert_eq!(warnings[0].len, text.len());
            }

            #[test]
            fn warn_leaves_the_reference_and_records_a_warning() {
                let p = parser_with_mode("warn");
                assert_eq!(render("Hello, {name}!", &p), "Hello, {name}!");

                let warnings = p.take_substitution_warnings();
                assert_eq!(warnings.len(), 1);
                assert_eq!(
                    warnings[0].warning,
                    WarningType::SkippingReferenceToMissingAttribute("name".to_string())
                );
            }

            #[test]
            fn warn_records_one_warning_per_missing_reference() {
                let p = parser_with_mode("warn");
                assert_eq!(render("a {x} b {y} c", &p), "a {x} b {y} c");
                assert_eq!(p.take_substitution_warnings().len(), 2);
            }

            #[test]
            fn escaped_missing_reference_drops_the_backslash_and_never_drops_the_line() {
                // An escaped reference has its backslash removed and is passed
                // through literally; it is never treated as a missing
                // reference, so even under `drop-line` the line
                // survives and no warning is recorded.
                let p = parser_with_mode("drop-line");
                assert_eq!(
                    render("In the path /items/\\{id}, x.", &p),
                    "In the path /items/{id}, x."
                );
                assert!(p.take_substitution_warnings().is_empty());
            }

            // The tests below drive `SubstitutionGroup::Normal` — the
            // production seam — so the precise per-reference `warn` location
            // (issue #564) is exercised where it is actually produced: the
            // single-pass builder recognizes each reference against `'src` and
            // records its own node span. The `render`-based tests above call
            // the string step directly, which has no source mapping to recover
            // and so names the whole-content span.

            #[test]
            fn warn_points_at_the_precise_reference() {
                let p = parser_with_mode("warn");
                let text = "Hello, {name}!";
                let mut content = Content::from(Span::new(text));
                SubstitutionGroup::Normal.apply(&mut content, &p, None);

                let warnings = p.take_substitution_warnings();
                assert_eq!(warnings.len(), 1);
                assert_spans(&warnings[0], text, "{name}");
            }

            #[test]
            fn warn_locates_multiple_references_on_one_line() {
                let p = parser_with_mode("warn");
                let text = "a {x} b {y} c";
                let mut content = Content::from(Span::new(text));
                SubstitutionGroup::Normal.apply(&mut content, &p, None);

                let warnings = p.take_substitution_warnings();
                assert_eq!(warnings.len(), 2);
                assert_spans(&warnings[0], text, "{x}");
                assert_spans(&warnings[1], text, "{y}");

                // The two references must resolve to distinct offsets.
                assert_ne!(warnings[0].offset, warnings[1].offset);
            }

            #[test]
            fn warn_locates_references_across_multiple_lines() {
                // The acceptance case from issue #564: several distinct
                // references on different lines of one block, each pointed at
                // individually rather than at the shared whole-block span.
                let p = parser_with_mode("warn");
                let text = "first {alpha} line\nsecond {beta} line\nthird {gamma} line";
                let mut content = Content::from(Span::new(text));
                SubstitutionGroup::Normal.apply(&mut content, &p, None);

                let warnings = p.take_substitution_warnings();
                assert_eq!(warnings.len(), 3);
                assert_spans(&warnings[0], text, "{alpha}");
                assert_spans(&warnings[1], text, "{beta}");
                assert_spans(&warnings[2], text, "{gamma}");
            }

            #[test]
            fn warn_distinguishes_repeated_reference_occurrences() {
                let p = parser_with_mode("warn");
                let text = "{dup} and again {dup}";
                let mut content = Content::from(Span::new(text));
                SubstitutionGroup::Normal.apply(&mut content, &p, None);

                let warnings = p.take_substitution_warnings();
                assert_eq!(warnings.len(), 2);
                assert_spans(&warnings[0], text, "{dup}");
                assert_spans(&warnings[1], text, "{dup}");

                // Same text, but the two occurrences are at different offsets.
                assert_eq!(warnings[0].offset, 0);
                assert_eq!(warnings[1].offset, text.rfind("{dup}").unwrap());
            }

            #[test]
            fn warn_span_survives_earlier_special_character_expansion() {
                // The key regression guard: special characters run before the
                // attributes step and lengthen the rendered text (`<` ->
                // `&lt;`), so a naive rendered-offset would be
                // wrong. The warning must still name the
                // reference's *original* source offset.
                let p = parser_with_mode("warn");
                let text = "a < b {foo} c";
                let mut content = Content::from(Span::new(text));
                SubstitutionGroup::Normal.apply(&mut content, &p, None);

                // Sanity check that the earlier step really did shift offsets.
                assert!(content.rendered_html().contains("&lt;"));

                let warnings = p.take_substitution_warnings();
                assert_eq!(warnings.len(), 1);
                assert_spans(&warnings[0], text, "{foo}");
                assert_eq!(warnings[0].offset, text.find("{foo}").unwrap());
            }

            #[test]
            fn warn_span_survives_earlier_quote_expansion() {
                // Same guard as above, but for the quotes step, which wraps
                // `*bold*` in markup ahead of the attributes step.
                let p = parser_with_mode("warn");
                let text = "*bold* {foo}";
                let mut content = Content::from(Span::new(text));
                SubstitutionGroup::Normal.apply(&mut content, &p, None);

                assert!(content.rendered_html().contains("<strong>"));

                let warnings = p.take_substitution_warnings();
                assert_eq!(warnings.len(), 1);
                assert_spans(&warnings[0], text, "{foo}");
                assert_eq!(warnings[0].offset, text.find("{foo}").unwrap());
            }

            #[test]
            fn warn_falls_back_to_whole_span_on_a_direct_step_call() {
                // A direct step call has no source mapping to recover, so the
                // warning degrades to the whole-content span (the pre-#564
                // behavior) rather than misreporting a location.
                let p = parser_with_mode("warn");
                let text = "x {foo} y";
                let mut content = Content::from(Span::new(text));
                SubstitutionStep::AttributeReferences.apply(&mut content, &p, None);

                let warnings = p.take_substitution_warnings();
                assert_eq!(warnings.len(), 1);
                assert_eq!(warnings[0].offset, 0);
                assert_eq!(warnings[0].len, text.len());
            }
        }
    }

    mod callouts {
        use crate::{
            content::{Content, SubstitutionStep},
            parser::ModificationContext,
            strings::CowStr,
            tests::prelude::*,
        };

        /// Builds a `Content` whose `rendered` text is `text` (as if special
        /// characters had already been substituted), applies the callouts step,
        /// and returns the resulting rendered text.
        fn render_callouts(text: &str, parser: &Parser) -> String {
            let mut content = Content::from(crate::Span::new(text));

            // `Content::from` copies the source verbatim into `rendered`, which
            // is exactly the post-special-characters state we want to exercise.
            SubstitutionGroup::Custom(vec![SubstitutionStep::Callouts]).apply(
                &mut content,
                parser,
                None,
            );
            content.rendered.to_string()
        }

        #[test]
        fn empty() {
            let mut content = Content::from(crate::Span::default());
            let p = Parser::default();
            SubstitutionGroup::Custom(vec![SubstitutionStep::Callouts]).apply(
                &mut content,
                &p,
                None,
            );
            assert!(content.is_empty());
            assert_eq!(content.rendered, CowStr::Borrowed(""));
        }

        #[test]
        fn no_callouts() {
            let p = Parser::default();
            assert_eq!(render_callouts("just some text", &p), "just some text");
        }

        #[test]
        fn lt_without_callout_is_untouched() {
            let p = Parser::default();
            assert_eq!(render_callouts("a &lt;b&gt; c", &p), "a &lt;b&gt; c");
        }

        #[test]
        fn basic_explicit() {
            let p = Parser::default();
            assert_eq!(
                render_callouts("require 'x' &lt;1&gt;", &p),
                r#"require 'x' <b class="conum">(1)</b>"#
            );
        }

        #[test]
        fn line_comment_prefix_preserved() {
            let p = Parser::default();
            assert_eq!(
                render_callouts("puts 'x' # &lt;1&gt;", &p),
                r#"puts 'x' # <b class="conum">(1)</b>"#
            );
        }

        #[test]
        fn multiple_on_one_line() {
            let p = Parser::default();
            assert_eq!(
                render_callouts("puts x &lt;5&gt;&lt;6&gt;", &p),
                r#"puts x <b class="conum">(5)</b><b class="conum">(6)</b>"#
            );
        }

        #[test]
        fn not_at_end_of_line() {
            let p = Parser::default();
            assert_eq!(
                render_callouts("puts \"&lt;1&gt; in the middle\"", &p),
                "puts \"&lt;1&gt; in the middle\""
            );
        }

        #[test]
        fn auto_numbering() {
            let p = Parser::default();
            assert_eq!(
                render_callouts("a &lt;.&gt;\nb &lt;.&gt;\nc &lt;.&gt;", &p),
                "a <b class=\"conum\">(1)</b>\nb <b class=\"conum\">(2)</b>\nc <b class=\"conum\">(3)</b>"
            );
        }

        #[test]
        fn mixed_numbering_ignores_explicit() {
            // Auto-numbering is not aware of explicit numbers.
            let p = Parser::default();
            assert_eq!(
                render_callouts("a &lt;.&gt;\nb &lt;1&gt;\nc &lt;.&gt;", &p),
                "a <b class=\"conum\">(1)</b>\nb <b class=\"conum\">(1)</b>\nc <b class=\"conum\">(2)</b>"
            );
        }

        #[test]
        fn xml_callout() {
            let p = Parser::default();
            assert_eq!(
                render_callouts("&lt;child/&gt; &lt;!--1--&gt;", &p),
                r#"&lt;child/&gt; &lt;!--<b class="conum">(1)</b>--&gt;"#
            );
        }

        #[test]
        fn half_xml_comment_is_not_a_callout() {
            let p = Parser::default();
            assert_eq!(
                render_callouts("First line &lt;1--&gt;", &p),
                "First line &lt;1--&gt;"
            );
        }

        #[test]
        fn escaped_callout() {
            let p = Parser::default();
            assert_eq!(
                render_callouts("require 'x' # \\&lt;1&gt;", &p),
                "require 'x' # &lt;1&gt;"
            );
        }

        #[test]
        fn icons_font() {
            let p = Parser::default().with_intrinsic_attribute(
                "icons",
                "font",
                ModificationContext::Anywhere,
            );
            assert_eq!(
                render_callouts("puts x # &lt;1&gt;", &p),
                r#"puts x <i class="conum" data-value="1"></i><b>(1)</b>"#
            );
        }

        #[test]
        fn icons_image() {
            let p = Parser::default().with_intrinsic_attribute(
                "icons",
                "",
                ModificationContext::Anywhere,
            );
            assert_eq!(
                render_callouts("puts x &lt;1&gt;", &p),
                r#"puts x <img src="./images/icons/callouts/1.png" alt="1">"#
            );
        }

        #[test]
        fn custom_line_comment_prefix() {
            // `line-comment=%` (Erlang). Only `%` is recognized as a prefix.
            let mut content = Content::from(crate::Span::new("hello() -> % &lt;1&gt;"));
            let attrlist = crate::attributes::Attrlist::parse(
                crate::Span::new("source,erlang,line-comment=%"),
                &Parser::default(),
                crate::attributes::AttrlistContext::Block,
            )
            .item
            .item;
            let p = Parser::default();
            SubstitutionGroup::Custom(vec![SubstitutionStep::Callouts]).apply(
                &mut content,
                &p,
                Some(&attrlist),
            );
            assert_eq!(
                content.rendered.to_string(),
                r#"hello() -> % <b class="conum">(1)</b>"#
            );
        }

        #[test]
        fn disabled_line_comment_preserves_leading_chars() {
            // `line-comment=` (empty) disables prefix recognition, so the `--`
            // before the callout is preserved verbatim.
            let mut content = Content::from(crate::Span::new("-- &lt;1&gt;"));
            let attrlist = crate::attributes::Attrlist::parse(
                crate::Span::new("source,asciidoc,line-comment="),
                &Parser::default(),
                crate::attributes::AttrlistContext::Block,
            )
            .item
            .item;
            let p = Parser::default();
            SubstitutionGroup::Custom(vec![SubstitutionStep::Callouts]).apply(
                &mut content,
                &p,
                Some(&attrlist),
            );
            assert_eq!(
                content.rendered.to_string(),
                r#"-- <b class="conum">(1)</b>"#
            );
        }

        #[test]
        fn document_line_comment_attribute() {
            // The `line-comment` attribute can be set at the document level
            // (here, with no block attrlist), and is honored as a fallback.
            let p = Parser::default().with_intrinsic_attribute(
                "line-comment",
                "%",
                ModificationContext::Anywhere,
            );
            assert_eq!(
                render_callouts("hello() -> % &lt;1&gt;", &p),
                r#"hello() -> % <b class="conum">(1)</b>"#
            );
        }
    }
}
