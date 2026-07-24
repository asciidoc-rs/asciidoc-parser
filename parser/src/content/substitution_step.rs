use std::{borrow::Cow, ops::Range, sync::LazyLock};

use regex::{Captures, Regex, RegexBuilder, Replacer};

use crate::{
    Parser, Span,
    attributes::{Attrlist, AttrlistContext},
    content::Content,
    document::{InterpretedValue, RefType},
    internal::{LookaheadReplacer, LookaheadResult, replace_with_lookahead},
    parser::{
        CalloutGuard, CalloutRenderParams, CharacterReplacementType, InlineSubstitutionRenderer,
        QuoteScope, QuoteType, SpecialCharacter, attribute_lookup_name,
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

    let replacer = SpecialCharacterReplacer { renderer };

    // The guard above guarantees at least one of `<`, `>`, `&` is present, so
    // `replace_all` always rewrites the text and returns `Cow::Owned`. Bind that
    // owned string directly – seeding a working buffer with `to_string()` first
    // would be a second, wholly redundant heap allocation. A `Cow::Borrowed`
    // result cannot occur, but if it somehow did we leave the text unchanged.
    let rendered = match SPECIAL_CHARS.replace_all(content.rendered.as_ref(), replacer) {
        Cow::Owned(rendered) => rendered,
        Cow::Borrowed(_) => return,
    };

    content.rendered = rendered.into();
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
            // The leading boundary group `[^\w&;:"'`}]` matches any non-word
            // Unicode scalar, so it can be a multi-byte character. Skip the full
            // width of that leading character rather than assuming one byte;
            // otherwise the slice below (and the matching offset in
            // `SkipAheadAndRetry`) would land inside the character and panic.
            let skip_ahead = if caps[0].starts_with('\\') {
                // Escape case: skip the backslash plus the following byte, which
                // is always an ASCII `[` or `` ` ``.
                2
            } else {
                caps[0].chars().next().map_or(1, char::len_utf8)
            };

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

    // Start borrowed: the sniff above only proves a quote-like character is
    // present, not that any pattern actually matches, so seeding an owned
    // working buffer up front would allocate even when nothing is rewritten
    // (a false-positive sniff). `owned` is materialized only once a pattern
    // first produces `Cow::Owned`, and reused thereafter.
    let mut owned: Option<String> = None;

    for sub in &*QUOTE_SUBS {
        let replacer = QuoteReplacer {
            type_: sub.type_,
            scope: sub.scope,
            parser,
        };

        let replaced = {
            let haystack = owned
                .as_deref()
                .unwrap_or_else(|| content.rendered.as_ref());

            match replace_with_lookahead(&sub.pattern, haystack, replacer) {
                Cow::Owned(new_result) => Some(new_result),

                // A borrowed result means this pattern did not match, so no need
                // to pay for a new string allocation.
                Cow::Borrowed(_) => None,
            }
        };

        if let Some(new_result) = replaced {
            owned = Some(new_result);
        }
    }

    if let Some(rendered) = owned {
        content.rendered = rendered.into();
    }
}

static ATTRIBUTE_REFERENCE: LazyLock<Regex> = LazyLock::new(|| {
    // Either a `counter`/`counter2` directive (group 1) with its `name[:seed]`
    // expression (group 2), or a plain attribute name (group 3). This mirrors
    // the `counter2?:` branch of Asciidoctor's `AttributeReferenceRx`.
    //
    // The attribute-name class `\w` (Unicode `\p{Word}`) accepts any Unicode
    // word character, matching Asciidoctor's `#{CG_WORD}[#{CC_WORD}-]*`, so
    // references such as `{café}` and `{سمن}` resolve. It is the same class
    // used to recognize and sanitize an attribute-entry name (see
    // `is_word_char`), so a name and a reference to it always agree.
    #[allow(clippy::unwrap_used)]
    Regex::new(r#"\\?\{(?:(counter2?):([^{}]+)|(\w[\w-]*))\}"#).unwrap()
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
/// which operates on [`Content::rendered`] – text that earlier steps (special
/// characters, quotes) have already transformed, and from which passthroughs
/// have been masked to placeholder tokens. A byte offset in that rendered text
/// therefore has no constant delta back to the original source `Span`, so a
/// warning cannot simply slice `content.original()` at the rendered offset.
///
/// Three approaches were considered (see issue #564):
///
/// 1. **Thread a source-offset map through every substitution step.** Fully
///    general, but adds offset-tracking state to `Content` and every mutating
///    step – a large surface and regression risk disproportionate to a
///    diagnostic-only refinement.
/// 2. **Scan the whole original source positionally.** Pair the *k*-th
///    reference found in `rendered` with the *k*-th `{name}` in
///    `content.original()`. Simple, but the raw source still contains `{name}`
///    tokens that never reach substitution – inside removed comment lines and
///    inside passthroughs – so the pairing drifts out of alignment.
/// 3. **Per-line positional correlation (chosen).** Anchor each rendered line
///    to the source `Span` of the line it came from (retained at construction,
///    see [`Content::from_filtered_lines`]), then pair the *k*-th reference on
///    the rendered line with the *k*-th `{name}` in that source line.
///
/// Approach 3 works because the two length-changing steps that run before
/// attributes (special characters, quotes) neither add, remove, nor reorder
/// `{…}`-shaped tokens and never introduce or remove a newline, so a rendered
/// line and its source line carry the same reference tokens in the same order.
/// Anchoring per line (rather than per block) sidesteps the
/// removed-comment-line drift of approach 2 for free.
///
/// # Graceful degradation
///
/// The correlation is best-effort. When a precise span can't be trusted it
/// falls back to [`fallback_source`](Self::fallback_source) (the whole-content
/// span, i.e. the pre-#564 behavior):
///
/// - No source line is available ([`source_line`](Self::source_line) is
///   `None`), e.g. for content not built line-by-line from source.
/// - The retained line count no longer matches the rendered line count, e.g. a
///   multi-line passthrough collapsed lines during extraction. The caller
///   detects this and withholds the source line.
/// - The *k*-th source-line match's text does not equal the rendered match,
///   e.g. an inline passthrough on the same line masked an earlier reference
///   and shifted the count. The [text check](Self::warning_source) catches the
///   mismatch and degrades to the fallback rather than pointing at the wrong
///   token.
#[derive(Debug)]
struct AttributeReplacer<'p> {
    parser: &'p Parser,

    /// How to handle a reference to a missing attribute.
    mode: AttributeMissing,

    /// Source span used to locate a `warn` warning when a precise per-reference
    /// span cannot be recovered. This is the whole content (or line/target)
    /// span – the coarse fallback described in the type-level docs.
    fallback_source: Span<'p>,

    /// Source `Span` of the line currently being processed, when known. Every
    /// attribute reference on this line is located by slicing a subrange of
    /// this span. `None` disables precise location (the warning uses
    /// [`fallback_source`](Self::fallback_source)).
    source_line: Option<Span<'p>>,

    /// Byte ranges (into [`source_line`](Self::source_line)'s data) of every
    /// `ATTRIBUTE_REFERENCE` match on the source line, in order. Populated only
    /// in [`AttributeMissing::Warn`] mode and only when `source_line` is set.
    source_matches: Vec<Range<usize>>,

    /// Index of the next reference to be processed on this line, into
    /// [`source_matches`](Self::source_matches). The regex driver calls
    /// [`replace_append`](Replacer::replace_append) once per match, left to
    /// right, so this stays in step with the rendered matches.
    match_index: usize,

    /// Set to `true` when a (non-escaped) reference to a missing attribute is
    /// dropped, under either [`AttributeMissing::Drop`] or
    /// [`AttributeMissing::DropLine`], so the caller can drop the line: the
    /// whole line in `drop-line` mode, or a line the dropped reference left
    /// empty in `drop` mode (Asciidoctor's `reject_if_empty`).
    missing_on_line: bool,
}

impl<'p> AttributeReplacer<'p> {
    /// Builds the replacer for one line, precomputing the source-match ranges
    /// used to locate `warn` warnings precisely.
    ///
    /// `source_line` is the source span the line was rendered from, or `None`
    /// when no precise mapping is available. `fallback_source` is the coarse
    /// span used when a precise location cannot be recovered.
    fn new(
        parser: &'p Parser,
        mode: AttributeMissing,
        fallback_source: Span<'p>,
        source_line: Option<Span<'p>>,
    ) -> Self {
        // The per-reference ranges are only consulted in `warn` mode, so skip the
        // extra scan otherwise.
        let source_matches = match (mode, source_line) {
            (AttributeMissing::Warn, Some(line)) => ATTRIBUTE_REFERENCE
                .find_iter(line.data())
                .map(|m| m.range())
                .collect(),
            _ => Vec::new(),
        };

        Self {
            parser,
            mode,
            fallback_source,
            source_line,
            source_matches,
            match_index: 0,
            missing_on_line: false,
        }
    }

    /// Returns the source span to attribute a `warn` warning to for the
    /// reference at `index` on this line, whose matched text (including any
    /// leading escape backslash) is `matched`.
    ///
    /// Falls back to [`fallback_source`](Self::fallback_source) unless a
    /// retained source-line match at `index` exists *and* its text equals
    /// `matched` – the text check guards against a correlation that has
    /// drifted (see the type-level docs).
    fn warning_source(&self, index: usize, matched: &str) -> Span<'p> {
        if let Some(line) = self.source_line
            && let Some(range) = self.source_matches.get(index)
            && line.data().get(range.clone()) == Some(matched)
        {
            return line.slice(range.clone());
        }

        self.fallback_source
    }
}

impl Replacer for AttributeReplacer<'_> {
    fn replace_append(&mut self, caps: &Captures<'_>, dest: &mut String) {
        // Consume this reference's position in the line so the next call lines up
        // with the next source-line match, regardless of which branch handles it.
        let match_index = self.match_index;
        self.match_index += 1;

        let escaped = caps[0].starts_with('\\');

        // A `counter`/`counter2` directive resolves (and advances) a counter
        // rather than looking up an existing attribute.
        if let Some(directive) = caps.get(1) {
            if escaped {
                // An escaped counter reference is emitted literally (without the
                // backslash) and does not advance the counter.
                dest.push_str(&caps[0][1..]);
                return;
            }

            // Group 2 always participates when group 1 does (same alternation
            // branch). The expression is `name` or `name:seed`.
            let mut parts = caps[2].splitn(2, ':');
            let name = parts.next().unwrap_or_default();
            let seed = parts.next();

            let value = self.parser.counter(name, seed);

            // `counter` displays the new value; `counter2` advances silently.
            if directive.as_str() == "counter" {
                dest.push_str(&value);
            }
            return;
        }

        // Otherwise this is a plain attribute reference (group 3).
        let attr_name = &caps[3];

        // An escaped reference (e.g. `\{id}`) is emitted literally with the
        // escaping backslash removed, whether or not the attribute is set. It is
        // never treated as a missing reference, so it neither drops the line nor
        // warns. This mirrors Asciidoctor, whose `sub_attributes` returns the
        // match minus its leading backslash before any missing-attribute
        // handling runs.
        if escaped {
            dest.push_str(&caps[0][1..]);
            return;
        }

        // Resolve the reference case-insensitively: attribute names are stored
        // lower-cased (both an attribute-entry definition and an API-supplied
        // attribute fold their name), so the lookup name is folded the same way
        // here. This mirrors Asciidoctor's `sub_attributes`, which looks up
        // `key = $2.downcase`. The original spelling is still what is emitted
        // literally for a skipped or missing reference below.
        let lookup_name = attribute_lookup_name(attr_name);

        if !self.parser.has_attribute(&lookup_name) {
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
                    // here is discarded with it.
                    self.missing_on_line = true;
                }
                AttributeMissing::Warn => {
                    dest.push_str(&caps[0]);
                    self.parser.record_substitution_warning(
                        self.warning_source(match_index, &caps[0]),
                        WarningType::SkippingReferenceToMissingAttribute(attr_name.to_string()),
                    );
                }
            }
            return;
        }

        if let InterpretedValue::Value(value) = self.parser.attribute_value(&lookup_name) {
            dest.push_str(value.as_ref());
        }

        // Language description is unclear as to what happens for "set" and
        // "unset" attribute values. For now, we'll replace those with nothing.
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

    // In `warn` mode, anchor each rendered line to the source `Span` it came
    // from so a warning can name the precise offset of the offending reference
    // (see `AttributeReplacer`). The retained line spans are only trustworthy
    // when they still line up one-to-one with the rendered lines; a mismatch
    // (e.g. a multi-line passthrough that collapsed lines during extraction)
    // withholds them, falling back to the coarse whole-content span.
    let source_lines = if mode == AttributeMissing::Warn {
        content
            .source_lines()
            .filter(|lines| lines.len() == content.rendered.split('\n').count())
    } else {
        None
    };

    // Attribute references are replaced line by line so that, in `drop-line`
    // mode, an individual line carrying a missing reference can be removed
    // without disturbing the lines around it. A reference cannot span a line
    // break, so this matches what a single whole-text pass would produce for
    // every other mode.
    let mut out = String::with_capacity(content.rendered.len());
    let mut changed = false;
    let mut wrote_line = false;

    for (index, line) in content.rendered.split('\n').enumerate() {
        if !line.contains('{') {
            if wrote_line {
                out.push('\n');
            }
            out.push_str(line);
            wrote_line = true;
            continue;
        }

        // `index` enumerates the same split whose count the guard above matched
        // against `source_lines.len()`, so the entry is always present; `.get`
        // keeps the access panic-free regardless.
        let source_line = source_lines.and_then(|lines| lines.get(index).copied());
        let mut replacer = AttributeReplacer::new(parser, mode, source, source_line);

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
/// [`AttributeMissing::DropLine`] – signaling that the entire block should be
/// dropped – and otherwise returns the substituted target.
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

    // The target is a single source-backed line, so it doubles as both the
    // precise per-reference anchor and the coarse fallback.
    let mut replacer = AttributeReplacer::new(parser, mode, target, Some(target));

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

        // This text is not backed by the document source (offsets refer into
        // `text`, and callers discard these warnings), so no precise per-line
        // anchor is supplied: warnings fall back to the whole-text span.
        let mut replacer = AttributeReplacer::new(parser, mode, source, None);

        let replaced = ATTRIBUTE_REFERENCE.replace_all(line, replacer.by_ref());

        if replacer.missing_on_line
            && (mode == AttributeMissing::DropLine
                || (mode == AttributeMissing::Drop && drop_emptied_line(&replaced)))
        {
            // Drop the entire line, including its line break: unconditionally
            // in `drop-line` mode, or in `drop` mode when the dropped
            // reference was all the line contained (Asciidoctor's
            // `reject_if_empty`).
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

fn apply_character_replacements(
    content: &mut Content<'_>,
    renderer: &dyn InlineSubstitutionRenderer,
) {
    if !REPLACEABLE_TEXT_SNIFF.is_match(content.rendered.as_ref()) {
        return;
    }

    // Start borrowed: the sniff above only proves a replaceable character is
    // present, not that any pattern actually matches, so seeding an owned
    // working buffer up front would allocate even when nothing is rewritten
    // (a false-positive sniff). `owned` is materialized only once a pattern
    // first produces `Cow::Owned`, and reused thereafter.
    let mut owned: Option<String> = None;

    for repl in &*REPLACEMENTS {
        let replacer = CharacterReplacer {
            type_: repl.type_.clone(),
            renderer,
        };

        let replaced = {
            let haystack = owned
                .as_deref()
                .unwrap_or_else(|| content.rendered.as_ref());

            match repl.pattern.replace_all(haystack, replacer) {
                Cow::Owned(new_result) => Some(new_result),

                // A borrowed result means this pattern did not match, so no need
                // to pay for a new string allocation.
                Cow::Borrowed(_) => None,
            }
        };

        if let Some(new_result) = replaced {
            owned = Some(new_result);
        }
    }

    if let Some(rendered) = owned {
        content.rendered = rendered.into();
    }
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
                SubstitutionStep::Quotes.apply(&mut content, &p, None);

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

            SubstitutionStep::Quotes.apply(&mut content, &p, None);

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

            /// Builds a `Content` that carries the per-line source spans (as a
            /// real block would), so the precise `warn`-location correlation is
            /// exercised. Each line's span is a subrange of the root, mirroring
            /// what block construction retains via
            /// [`Content::from_filtered_lines`].
            fn content_with_source_lines(text: &'static str) -> Content<'static> {
                let root = Span::new(text);
                let lines: Vec<&str> = text.split('\n').collect();

                let mut spans = Vec::with_capacity(lines.len());
                let mut offset = 0;
                for line in &lines {
                    spans.push(root.slice(offset..offset + line.len()));

                    // Advance past the line and the '\n' that split consumed.
                    offset += line.len() + 1;
                }

                Content::from_filtered_lines(root, &lines, spans)
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

            /// Exercises the free-standing text path (used for docinfo file
            /// content), which applies the same `attribute-missing` handling as
            /// [`render`] but through [`substitute_attributes_in_text`] rather
            /// than the block substitution pipeline.
            mod free_standing_text {
                use super::parser_with_mode;
                use crate::content::substitute_attributes_in_text;

                #[test]
                fn drop_removes_line_that_only_contained_the_reference() {
                    let p = parser_with_mode("drop");
                    assert_eq!(
                        substitute_attributes_in_text("Line 1\n{missing}\nLine 2", &p),
                        "Line 1\nLine 2"
                    );
                }

                #[test]
                fn drop_keeps_a_line_the_reference_did_not_empty() {
                    let p = parser_with_mode("drop");
                    assert_eq!(
                        substitute_attributes_in_text("Line 1\ntext {missing}\nLine 2", &p),
                        "Line 1\ntext \nLine 2"
                    );
                }

                #[test]
                fn drop_keeps_a_line_emptied_by_a_resolvable_reference() {
                    let p = parser_with_mode("drop");
                    assert_eq!(
                        substitute_attributes_in_text("Line 1\n{empty}\nLine 2", &p),
                        "Line 1\n\nLine 2"
                    );
                }

                #[test]
                fn drop_line_removes_the_whole_line() {
                    let p = parser_with_mode("drop-line");
                    assert_eq!(
                        substitute_attributes_in_text("Line 1\n{missing} tail\nLine 2", &p),
                        "Line 1\nLine 2"
                    );
                }

                #[test]
                fn drop_removes_a_crlf_reference_only_line() {
                    // The `\r` left by a CRLF terminator does not keep the line
                    // from counting as emptied by the dropped reference, so the
                    // whole `\r\n` line is removed.
                    let p = parser_with_mode("drop");
                    assert_eq!(
                        substitute_attributes_in_text("Line 1\r\n{missing}\r\nLine 2", &p),
                        "Line 1\r\nLine 2"
                    );
                }

                #[test]
                fn drop_keeps_a_crlf_line_the_reference_did_not_empty() {
                    let p = parser_with_mode("drop");
                    assert_eq!(
                        substitute_attributes_in_text("Line 1\r\ntext {missing}\r\nLine 2", &p),
                        "Line 1\r\ntext \r\nLine 2"
                    );
                }
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
                // through literally; it is never treated as a missing reference,
                // so even under `drop-line` the line survives and no warning is
                // recorded.
                let p = parser_with_mode("drop-line");
                assert_eq!(
                    render("In the path /items/\\{id}, x.", &p),
                    "In the path /items/{id}, x."
                );
                assert!(p.take_substitution_warnings().is_empty());
            }

            // The tests below use `content_with_source_lines` so the precise
            // per-reference `warn` location (issue #564) is exercised; the
            // `render`-based tests above go through `Content::from`, which
            // retains no source lines and so falls back to the whole-content
            // span.

            #[test]
            fn warn_points_at_the_precise_reference() {
                let p = parser_with_mode("warn");
                let text = "Hello, {name}!";
                let mut content = content_with_source_lines(text);
                SubstitutionStep::AttributeReferences.apply(&mut content, &p, None);

                let warnings = p.take_substitution_warnings();
                assert_eq!(warnings.len(), 1);
                assert_spans(&warnings[0], text, "{name}");
            }

            #[test]
            fn warn_locates_multiple_references_on_one_line() {
                let p = parser_with_mode("warn");
                let text = "a {x} b {y} c";
                let mut content = content_with_source_lines(text);
                SubstitutionStep::AttributeReferences.apply(&mut content, &p, None);

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
                let mut content = content_with_source_lines(text);
                SubstitutionStep::AttributeReferences.apply(&mut content, &p, None);

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
                let mut content = content_with_source_lines(text);
                SubstitutionStep::AttributeReferences.apply(&mut content, &p, None);

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
                // attributes step and lengthen the rendered text (`<` -> `&lt;`),
                // so a naive rendered-offset would be wrong. The warning must
                // still name the reference's *original* source offset.
                let p = parser_with_mode("warn");
                let text = "a < b {foo} c";
                let mut content = content_with_source_lines(text);
                SubstitutionGroup::Normal.apply(&mut content, &p, None);

                // Sanity check that the earlier step really did shift offsets.
                assert!(content.rendered().contains("&lt;"));

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
                let mut content = content_with_source_lines(text);
                SubstitutionGroup::Normal.apply(&mut content, &p, None);

                assert!(content.rendered().contains("<strong>"));

                let warnings = p.take_substitution_warnings();
                assert_eq!(warnings.len(), 1);
                assert_spans(&warnings[0], text, "{foo}");
                assert_eq!(warnings[0].offset, text.find("{foo}").unwrap());
            }

            #[test]
            fn warn_falls_back_to_whole_span_without_source_lines() {
                // `Content::from` retains no per-line spans, so the warning
                // degrades to the whole-content span (the pre-#564 behavior)
                // rather than misreporting a location.
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
            SubstitutionStep::Callouts.apply(&mut content, &p, Some(&attrlist));
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
