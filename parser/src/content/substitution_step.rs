use std::{borrow::Cow, sync::LazyLock};

use regex::{Captures, Regex, RegexBuilder, Replacer};

use crate::{
    Parser, Span,
    attributes::{Attrlist, AttrlistContext},
    content::Content,
    document::{InterpretedValue, RefType},
    internal::{LookaheadReplacer, LookaheadResult, replace_with_lookahead},
    parser::{
        CalloutGuard, CalloutRenderParams, CharacterReplacementType, InlineSubstitutionRenderer,
        QuoteScope, QuoteType, SpecialCharacter,
    },
    strings::CowStr,
    warnings::WarningType,
};

/// Each substitution type replaces characters, markup, attribute references,
/// and macros in text with the appropriate output for a given converter. When a
/// document is processed, up to six substitution types may be carried out
/// depending on the block or inline element’s assigned substitution group. The
/// processor runs the substitutions in the following order:
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
        attrlist: Option<&Attrlist<'_>>,
    ) {
        match self {
            Self::SpecialCharacters => {
                apply_special_characters(content, &*parser.renderer);
            }
            Self::Quotes => {
                apply_quotes(content, parser);
            }
            Self::AttributeReferences => {
                apply_attributes(content, parser);
            }
            Self::CharacterReplacements => {
                apply_character_replacements(content, &*parser.renderer);
            }
            Self::Macros => {
                super::macros::apply_macros(content, parser);
            }
            Self::PostReplacement => {
                apply_post_replacements(content, parser, attrlist);
            }
            Self::Callouts => {
                apply_callouts(content, parser, attrlist);
            }
        }
    }
}

fn apply_special_characters(content: &mut Content<'_>, renderer: &dyn InlineSubstitutionRenderer) {
    if !content.rendered.contains(['<', '>', '&']) {
        return;
    }

    let mut result: Cow<'_, str> = content.rendered.to_string().into();
    let replacer = SpecialCharacterReplacer { renderer };

    if let Cow::Owned(new_result) = SPECIAL_CHARS.replace_all(&result, replacer) {
        result = new_result.into();
    }

    content.rendered = result.into();
}

static SPECIAL_CHARS: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new("[<>&]").unwrap()
});

#[derive(Debug)]
struct SpecialCharacterReplacer<'r> {
    renderer: &'r dyn InlineSubstitutionRenderer,
}

impl Replacer for SpecialCharacterReplacer<'_> {
    fn replace_append(&mut self, caps: &Captures<'_>, dest: &mut String) {
        // The SPECIAL_CHARS regex only matches '<', '>', and '&'. This sequence is
        // specifically constructed to avoid having any unreachable code.
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

struct QuoteSub {
    type_: QuoteType,
    scope: QuoteScope,
    pattern: Regex,
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

#[derive(Debug)]
struct QuoteReplacer<'r> {
    type_: QuoteType,
    scope: QuoteScope,
    parser: &'r Parser,
}

impl LookaheadReplacer for QuoteReplacer<'_> {
    fn replace_append(
        &mut self,
        caps: &Captures<'_>,
        dest: &mut String,
        after: &str,
    ) -> LookaheadResult {
        // Adapted from Asciidoctor#convert_quoted_text, found in
        // https://github.com/asciidoctor/asciidoctor/blob/main/lib/asciidoctor/substitutors.rb#L1419-L1445.

        // The regex crate doesn't have a sophisticated lookahead mode, so we patch
        // it up here.

        if self.type_ == QuoteType::Monospaced
            && self.scope == QuoteScope::Constrained
            && after.starts_with(['"', '\'', '`'])
        {
            let skip_ahead = if caps[0].starts_with('\\') { 2 } else { 1 };
            dest.push_str(&caps[0][0..skip_ahead]);
            return LookaheadResult::SkipAheadAndRetry(skip_ahead);
        }

        let unescaped_attrs: Option<String> = if caps[0].starts_with('\\') {
            let maybe_attrs = caps.get(2).map(|a| a.as_str());
            if self.scope == QuoteScope::Constrained && maybe_attrs.is_some() {
                Some(format!(
                    "[{attrs}]",
                    attrs = maybe_attrs.unwrap_or_default()
                ))
            } else {
                dest.push_str(&caps[0][1..]);
                return LookaheadResult::Continue;
            }
        } else {
            None
        };

        match self.scope {
            QuoteScope::Constrained => {
                if let Some(attrs) = unescaped_attrs {
                    dest.push_str(&attrs);
                    self.parser.renderer.render_quoted_substitition(
                        self.type_, self.scope, None, None, &caps[3], dest,
                    );
                } else {
                    let (attrlist, type_): (Option<Attrlist<'_>>, QuoteType) =
                        if let Some(attrlist) = caps.get(2) {
                            let type_ = if self.type_ == QuoteType::Mark {
                                QuoteType::Unquoted
                            } else {
                                self.type_
                            };

                            (
                                Some(
                                    Attrlist::parse(
                                        crate::Span::new(attrlist.as_str()),
                                        self.parser,
                                        AttrlistContext::Inline,
                                    )
                                    .item
                                    .item,
                                ),
                                type_,
                            )
                        } else {
                            (None, self.type_)
                        };

                    if let Some(prefix) = caps.get(1) {
                        dest.push_str(prefix.as_str());
                    }

                    let id = attrlist
                        .as_ref()
                        .and_then(|a| a.id().map(|s| s.to_string()));

                    // Assigning an ID to inline quoted text (e.g.,
                    // `[#free_the_world]#free the world#`) makes that phrase
                    // referenceable, so register it in the catalog. A duplicate
                    // ID here is non-fatal (first registration wins).
                    if let Some(id) = &id {
                        let _ = self.parser.register_ref(id, None, RefType::Anchor);
                    }

                    self.parser.renderer.render_quoted_substitition(
                        type_, self.scope, attrlist, id, &caps[3], dest,
                    );
                }
            }

            QuoteScope::Unconstrained => {
                let (attrlist, type_): (Option<Attrlist<'_>>, QuoteType) =
                    if let Some(attrlist) = caps.get(1) {
                        let type_ = if self.type_ == QuoteType::Mark {
                            QuoteType::Unquoted
                        } else {
                            self.type_
                        };

                        (
                            Some(
                                Attrlist::parse(
                                    crate::Span::new(attrlist.as_str()),
                                    self.parser,
                                    AttrlistContext::Inline,
                                )
                                .item
                                .item,
                            ),
                            type_,
                        )
                    } else {
                        (None, self.type_)
                    };

                let id = attrlist
                    .as_ref()
                    .and_then(|a| a.id().map(|s| s.to_string()));

                // Assigning an ID to inline quoted text (e.g.,
                // `[#free_the_world]#free the world#`) makes that phrase
                // referenceable, so register it in the catalog. A duplicate ID
                // here is non-fatal (first registration wins).
                if let Some(id) = &id {
                    let _ = self.parser.register_ref(id, None, RefType::Anchor);
                }

                self.parser
                    .renderer
                    .render_quoted_substitition(type_, self.scope, attrlist, id, &caps[2], dest);
            }
        }

        LookaheadResult::Continue
    }
}

fn apply_quotes(content: &mut Content<'_>, parser: &Parser) {
    if !QUOTED_TEXT_SNIFF.is_match(content.rendered.as_ref()) {
        return;
    }

    let mut result: Cow<'_, str> = content.rendered.to_string().into();

    for sub in &*QUOTE_SUBS {
        let replacer = QuoteReplacer {
            type_: sub.type_,
            scope: sub.scope,
            parser,
        };

        if let Cow::Owned(new_result) = replace_with_lookahead(&sub.pattern, &result, replacer) {
            result = new_result.into();
        }
        // If it's Cow::Borrowed, there was no match for this pattern, so no
        // need to pay for a new string allocation.
    }

    content.rendered = result.into();
}

static ATTRIBUTE_REFERENCE: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r#"\\?\{([A-Za-z0-9_][A-Za-z0-9_-]*)\}"#).unwrap()
});

/// How the processor handles a reference to a missing attribute, controlled by
/// the [`attribute-missing`] document attribute.
///
/// [`attribute-missing`]: https://docs.asciidoctor.org/asciidoc/latest/attributes/unresolved-references/#missing
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AttributeMissing {
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
    fn from_parser(parser: &Parser) -> Self {
        match parser.attribute_value("attribute-missing").as_maybe_str() {
            Some("drop") => Self::Drop,
            Some("drop-line") => Self::DropLine,
            Some("warn") => Self::Warn,
            _ => Self::Skip,
        }
    }
}

#[derive(Debug)]
struct AttributeReplacer<'p> {
    parser: &'p Parser,

    /// How to handle a reference to a missing attribute.
    mode: AttributeMissing,

    /// Source span of the content being processed, used to locate any `warn`
    /// warning that is recorded.
    ///
    /// TO DO (<https://github.com/asciidoc-rs/asciidoc-parser/issues/564>): This
    /// is the whole content span, not the span of the individual reference, so
    /// every `warn` warning in a block points at the same (coarse) location.
    /// Replacement happens on already-substituted `rendered` text, which has no
    /// reliable mapping back to the original source offset of each reference.
    source: Span<'p>,

    /// Set to `true` when a (non-escaped) reference to a missing attribute is
    /// encountered, so the caller can drop the whole line in
    /// [`AttributeMissing::DropLine`] mode.
    missing_on_line: bool,
}

impl Replacer for AttributeReplacer<'_> {
    fn replace_append(&mut self, caps: &Captures<'_>, dest: &mut String) {
        let escaped = caps[0].starts_with('\\');
        let attr_name = &caps[1];

        if !self.parser.has_attribute(attr_name) {
            // An escaped reference (e.g. `\{id}`) to an attribute that isn't set
            // is left exactly as written and is never treated as a missing
            // reference, so it neither drops the line nor warns.
            if escaped {
                dest.push_str(&caps[0]);
                return;
            }

            match self.mode {
                AttributeMissing::Skip => dest.push_str(&caps[0]),
                AttributeMissing::Drop => {
                    // Drop the reference, leaving the rest of the line intact.
                }
                AttributeMissing::DropLine => {
                    // Mark the line for removal; whatever is written to `dest`
                    // here is discarded with it.
                    self.missing_on_line = true;
                }
                AttributeMissing::Warn => {
                    dest.push_str(&caps[0]);
                    self.parser.record_substitution_warning(
                        self.source,
                        WarningType::SkippingReferenceToMissingAttribute(attr_name.to_string()),
                    );
                }
            }
            return;
        }

        if escaped {
            dest.push_str(&caps[0][1..]);
            return;
        }

        if let InterpretedValue::Value(value) = self.parser.attribute_value(attr_name) {
            dest.push_str(value.as_ref());
        }
        // Language description is unclear as to what happens for "set" and
        // "unset" attribute values. For now, we'll replace those with nothing.
    }
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

        let mut replacer = AttributeReplacer {
            parser,
            mode,
            source,
            missing_on_line: false,
        };

        let replaced = ATTRIBUTE_REFERENCE.replace_all(line, replacer.by_ref());

        if replacer.missing_on_line && mode == AttributeMissing::DropLine {
            // Drop the entire line, including its line break.
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

    let mut replacer = AttributeReplacer {
        parser,
        mode,
        source: target,
        missing_on_line: false,
    };

    let replaced = ATTRIBUTE_REFERENCE.replace_all(text, replacer.by_ref());

    if replacer.missing_on_line && mode == AttributeMissing::DropLine {
        return None;
    }

    Some(replaced.into())
}

/// Applies the attribute-references substitution to free-standing text (such as
/// the content of a [docinfo file]), honoring the [`attribute-missing`]
/// document attribute, and returns the substituted result.
///
/// Unlike [`apply_attributes`], this operates on owned text that is not part of
/// the document source. Substitution is performed line by line so that, in
/// `drop-line` mode, an individual line carrying a missing reference can be
/// removed without disturbing the lines around it.
///
/// Any `warn`-mode warnings it records on `parser` refer to offsets within
/// `text` (not the document source); callers that do not want such warnings
/// surfaced should discard them via
/// [`Parser::truncate_substitution_warnings`](crate::Parser).
///
/// [docinfo file]: https://docs.asciidoctor.org/asciidoc/latest/docinfo/
/// [`attribute-missing`]: https://docs.asciidoctor.org/asciidoc/latest/attributes/unresolved-references/#missing
pub(crate) fn substitute_attributes_in_text(text: &str, parser: &Parser) -> String {
    if !text.contains('{') {
        return text.to_string();
    }

    let mode = AttributeMissing::from_parser(parser);
    let source = Span::new(text);

    let mut out = String::with_capacity(text.len());
    let mut wrote_line = false;

    for line in text.split('\n') {
        if !line.contains('{') {
            if wrote_line {
                out.push('\n');
            }
            out.push_str(line);
            wrote_line = true;
            continue;
        }

        let mut replacer = AttributeReplacer {
            parser,
            mode,
            source,
            missing_on_line: false,
        };

        let replaced = ATTRIBUTE_REFERENCE.replace_all(line, replacer.by_ref());

        if replacer.missing_on_line && mode == AttributeMissing::DropLine {
            // Drop the entire line, including its line break.
            continue;
        }

        if wrote_line {
            out.push('\n');
        }
        out.push_str(&replaced);
        wrote_line = true;
    }

    out
}

fn apply_character_replacements(
    content: &mut Content<'_>,
    renderer: &dyn InlineSubstitutionRenderer,
) {
    if !REPLACEABLE_TEXT_SNIFF.is_match(content.rendered.as_ref()) {
        return;
    }

    let mut result: Cow<'_, str> = content.rendered.to_string().into();

    for repl in &*REPLACEMENTS {
        let replacer = CharacterReplacer {
            type_: repl.type_.clone(),
            renderer,
        };

        if let Cow::Owned(new_result) = repl.pattern.replace_all(&result, replacer) {
            result = new_result.into();
        }
        // If it's Cow::Borrowed, there was no match for this pattern, so no
        // need to pay for a new string allocation.
    }

    content.rendered = result.into();
}

struct CharacterReplacement {
    type_: CharacterReplacementType,
    pattern: Regex,
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
            pattern: Regex::new(r#"\\?&amp;((?:[a-zA-Z][a-zA-Z]+\d{0,2}|#\d\d\d{0,4}|#x[\da-fA-F][\da-fA-F][\da-fA-F]{0,3}));"#).unwrap(),
        },
    ]
});

#[derive(Debug)]
struct CharacterReplacer<'r> {
    type_: CharacterReplacementType,
    renderer: &'r dyn InlineSubstitutionRenderer,
}

impl Replacer for CharacterReplacer<'_> {
    fn replace_append(&mut self, caps: &Captures<'_>, dest: &mut String) {
        if caps[0].contains('\\') {
            // We have to replace since we aren't sure the backslash is the first char.
            let unescaped = &caps[0].replace("\\", "");
            dest.push_str(unescaped);
            return;
        }

        match self.type_ {
            CharacterReplacementType::Copyright
            | CharacterReplacementType::Registered
            | CharacterReplacementType::Trademark
            | CharacterReplacementType::EmDashSurroundedBySpaces
            | CharacterReplacementType::Ellipsis
            | CharacterReplacementType::SingleLeftArrow
            | CharacterReplacementType::DoubleLeftArrow
            | CharacterReplacementType::SingleRightArrow
            | CharacterReplacementType::DoubleRightArrow => {
                self.renderer
                    .render_character_replacement(self.type_.clone(), dest);
            }

            CharacterReplacementType::EmDashWithoutSpace => {
                dest.push_str(&caps[1]);
                self.renderer.render_character_replacement(
                    CharacterReplacementType::EmDashWithoutSpace,
                    dest,
                );
            }

            CharacterReplacementType::TypographicApostrophe => {
                if let Some(before) = caps.get(1) {
                    dest.push_str(before.as_str());
                }

                self.renderer.render_character_replacement(
                    CharacterReplacementType::TypographicApostrophe,
                    dest,
                );

                if let Some(after) = caps.get(2) {
                    dest.push_str(after.as_str());
                }
            }

            CharacterReplacementType::CharacterReference(_) => {
                self.renderer.render_character_replacement(
                    CharacterReplacementType::CharacterReference(caps[1].to_string()),
                    dest,
                );
            }
        }
    }
}

fn apply_post_replacements(
    content: &mut Content<'_>,
    parser: &Parser,
    attrlist: Option<&Attrlist<'_>>,
) {
    if parser.is_attribute_set("hardbreaks-option")
        || attrlist.is_some_and(|attrlist| attrlist.has_option("hardbreaks"))
    {
        let text = content.rendered.as_ref();
        if !text.contains('\n') {
            return;
        }

        let mut lines: Vec<&str> = content.rendered.as_ref().lines().collect();
        let last = lines.pop().unwrap_or_default();

        let mut lines: Vec<String> = lines
            .iter()
            .map(|line| {
                let line = if line.ends_with(" +") {
                    &line[0..line.len() - 2]
                } else {
                    *line
                };

                let mut line = line.to_owned();
                parser.renderer.render_line_break(&mut line);
                line
            })
            .collect();

        lines.push(last.to_owned());

        let new_result = lines.join("\n");
        content.rendered = new_result.into();
    } else {
        let rendered = content.rendered.as_ref();
        if !(rendered.contains('+') && rendered.contains('\n')) {
            return;
        }

        let replacer = PostReplacementReplacer(&*parser.renderer);

        if let Cow::Owned(new_result) = HARD_LINE_BREAK.replace_all(rendered, replacer) {
            content.rendered = new_result.into();
        }
    }
}

#[derive(Debug)]
struct PostReplacementReplacer<'r>(&'r dyn InlineSubstitutionRenderer);

impl Replacer for PostReplacementReplacer<'_> {
    fn replace_append(&mut self, caps: &Captures<'_>, dest: &mut String) {
        dest.push_str(&caps[1]);
        self.0.render_line_break(dest);
    }
}

static HARD_LINE_BREAK: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r#"(?m)^(.*) \+$"#).unwrap()
});

/// Processes [callouts] in literal, listing, and source blocks.
///
/// Callout numbers (`<1>`, `<.>`, or `<!--1-->` for XML) that appear at the end
/// of a line are replaced with the renderer's callout markup. Callouts may be
/// tucked behind a line comment (`//`, `#`, `--`, or `;;` by default, or a
/// custom prefix specified by the `line-comment` attribute), and a callout may
/// be escaped with a leading backslash to render it literally.
///
/// This substitution runs after [special characters] have been replaced, so the
/// angle brackets that delimit a callout appear in `content.rendered` as
/// `&lt;` and `&gt;`. This mirrors Asciidoctor's `sub_callouts` /
/// `CalloutSourceRx`.
///
/// [callouts]: https://docs.asciidoctor.org/asciidoc/latest/verbatim/callouts/
/// [special characters]: https://docs.asciidoctor.org/asciidoc/latest/subs/special-characters/
fn apply_callouts(content: &mut Content<'_>, parser: &Parser, attrlist: Option<&Attrlist<'_>>) {
    // A callout's opening bracket is always rendered as `&lt;` by the special
    // characters substitution, so we can cheaply skip content without any.
    if !content.rendered.contains("&lt;") {
        return;
    }

    // The `line-comment` attribute (block-level, falling back to document-level)
    // customizes or disables line-comment recognition:
    //
    // * absent -> default prefixes (`//`, `#`, `--`, `;;`) and XML callouts are
    //   recognized.
    // * present (custom) -> only the given prefix is recognized; XML callouts are
    //   not.
    // * present but empty -> no line-comment prefix is recognized; XML callouts are
    //   not.
    let line_comment: Option<String> = attrlist
        .and_then(|a| a.named_attribute("line-comment"))
        .map(|a| a.value().to_string())
        .or_else(|| {
            if parser.has_attribute("line-comment") {
                Some(
                    parser
                        .attribute_value("line-comment")
                        .as_maybe_str()
                        .unwrap_or("")
                        .to_string(),
                )
            } else {
                None
            }
        });

    let (callout_rx, tail_rx) = build_callout_regexes(line_comment.as_deref());

    let replacer = CalloutReplacer {
        renderer: &*parser.renderer,
        parser,
        autonum: 0,
        tail: tail_rx,
    };

    if let Cow::Owned(new_result) =
        replace_with_lookahead(&callout_rx, content.rendered.as_ref(), replacer)
    {
        content.rendered = new_result.into();
    }
}

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
fn build_callout_regexes(line_comment: Option<&str>) -> (Cow<'static, Regex>, &'static Regex) {
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

/// Replacer that renders each trailing callout token, emulating Asciidoctor's
/// `sub_callouts`.
struct CalloutReplacer<'r> {
    renderer: &'r dyn InlineSubstitutionRenderer,
    parser: &'r Parser,

    /// Running counter for automatically-numbered (`<.>`) callouts, scoped to a
    /// single block.
    autonum: u32,

    /// Trailing-position lookahead regex (see [`build_callout_regexes`]).
    tail: &'r Regex,
}

impl LookaheadReplacer for CalloutReplacer<'_> {
    fn replace_append(
        &mut self,
        caps: &Captures<'_>,
        dest: &mut String,
        after: &str,
    ) -> LookaheadResult {
        // Honor the trailing-position requirement: a callout is only recognized
        // when nothing but further callouts follows it on the line.
        if !self.tail.is_match(after) {
            dest.push_str(&caps[0]);
            return LookaheadResult::Continue;
        }

        // Honor the escape: emit the matched text with the escaping backslash
        // removed so the callout renders literally.
        if caps.name("esc").is_some() {
            dest.push_str(&caps[0].replacen('\\', "", 1));
            return LookaheadResult::Continue;
        }

        let (number_raw, is_xml) = if let Some(xnum) = caps.name("xnum") {
            (xnum.as_str(), true)
        } else {
            // The regex guarantees one of `xnum` or `num` is present.
            #[allow(clippy::unwrap_used)]
            (caps.name("num").unwrap().as_str(), false)
        };

        let number = if number_raw == "." {
            self.autonum += 1;
            self.autonum.to_string()
        } else {
            number_raw.to_string()
        };

        // Register this callout so the callout list that annotates this block
        // can be validated against the callouts it references.
        if let Ok(n) = number.parse::<u32>() {
            self.parser.register_callout(n);
        }

        // Mirror Asciidoctor's guard resolution: a captured line-comment prefix
        // takes precedence; otherwise an XML callout uses the XML guard; failing
        // both, there is no guard.
        let guard = match caps.name("prefix") {
            Some(prefix) => CalloutGuard::LineComment(prefix.as_str()),
            None if is_xml => CalloutGuard::Xml,
            None => CalloutGuard::LineComment(""),
        };

        self.renderer.render_callout(
            &CalloutRenderParams {
                number: &number,
                guard,
                parser: self.parser,
            },
            dest,
        );

        LookaheadResult::Continue
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

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
            SubstitutionStep::Quotes.apply(&mut content, &p, None);
            assert!(content.is_empty());
            assert_eq!(content.rendered, CowStr::Borrowed(""));
        }

        #[test]
        fn basic_non_empty_span() {
            let mut content = Content::from(crate::Span::new("blah"));
            let p = Parser::default();
            SubstitutionStep::Quotes.apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(content.rendered, CowStr::Borrowed("blah"));
        }

        #[test]
        fn ignore_lt_and_gt() {
            let mut content = Content::from(crate::Span::new("bl<ah>"));
            let p = Parser::default();
            SubstitutionStep::Quotes.apply(&mut content, &p, None);
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
            SubstitutionStep::Quotes.apply(&mut content, &p, None);
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
            SubstitutionStep::Quotes.apply(&mut content, &p, None);
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
                doc.nested_blocks()
                    .next()
                    .unwrap()
                    .rendered_content()
                    .unwrap(),
                r#"<span id="the_id">marked text</span>"#
            );

            assert!(doc.catalog().contains_id("the_id"));
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
        fn ignore_escaped_non_match() {
            let mut content = Content::from(crate::Span::new("bl\\{ah}"));
            let p = Parser::default();
            SubstitutionStep::AttributeReferences.apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(
                content.rendered,
                CowStr::Boxed("bl\\{ah}".to_string().into_boxed_str())
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

        mod attribute_missing {
            #![allow(clippy::indexing_slicing)]

            use crate::{
                content::{Content, SubstitutionStep},
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
            fn escaped_missing_reference_is_left_verbatim_and_never_dropped() {
                let p = parser_with_mode("drop-line");
                assert_eq!(
                    render("In the path /items/\\{id}, x.", &p),
                    "In the path /items/\\{id}, x."
                );
                assert!(p.take_substitution_warnings().is_empty());
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
            SubstitutionStep::Callouts.apply(&mut content, parser, None);
            content.rendered.to_string()
        }

        #[test]
        fn empty() {
            let mut content = Content::from(crate::Span::default());
            let p = Parser::default();
            SubstitutionStep::Callouts.apply(&mut content, &p, None);
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
            // line-comment=% (Erlang). Only `%` is recognized as a prefix.
            let mut content = Content::from(crate::Span::new("hello() -> % &lt;1&gt;"));
            let attrlist = crate::attributes::Attrlist::parse(
                crate::Span::new("source,erlang,line-comment=%"),
                &Parser::default(),
                crate::attributes::AttrlistContext::Block,
            )
            .item
            .item;
            let p = Parser::default();
            SubstitutionStep::Callouts.apply(&mut content, &p, Some(&attrlist));
            assert_eq!(
                content.rendered.to_string(),
                r#"hello() -> % <b class="conum">(1)</b>"#
            );
        }

        #[test]
        fn disabled_line_comment_preserves_leading_chars() {
            // line-comment= (empty) disables prefix recognition, so the `--`
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
            SubstitutionStep::Callouts.apply(&mut content, &p, Some(&attrlist));
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
