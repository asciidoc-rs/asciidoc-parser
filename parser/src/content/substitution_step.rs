use std::{borrow::Cow, sync::LazyLock};

use regex::{Captures, Regex, RegexBuilder, Replacer};

use crate::{
    Parser,
    attributes::{Attrlist, AttrlistContext},
    content::Content,
    document::InterpretedValue,
    internal::{LookaheadReplacer, LookaheadResult, replace_with_lookahead},
    parser::{
        CharacterReplacementType, InlineSubstitutionRenderer, QuoteScope, QuoteType,
        SpecialCharacter,
    },
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
            _ => {
                todo!("Implement apply for SubstitutionStep::{self:?}");
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

#[derive(Debug)]
struct AttributeReplacer<'p>(&'p Parser);

impl Replacer for AttributeReplacer<'_> {
    fn replace_append(&mut self, caps: &Captures<'_>, dest: &mut String) {
        let attr_name = &caps[1];

        // TO DO: Handle alternative responses ('skip', etc.) for missing attributes.
        if !self.0.has_attribute(attr_name) {
            dest.push_str(&caps[0]);
            return;
        }

        if caps[0].starts_with('\\') {
            dest.push_str(&caps[0][1..]);
            return;
        }

        if let InterpretedValue::Value(value) = self.0.attribute_value(attr_name) {
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

    let mut result: Cow<'_, str> = content.rendered.to_string().into();

    if let Cow::Owned(new_result) =
        ATTRIBUTE_REFERENCE.replace_all(&result, AttributeReplacer(parser))
    {
        result = new_result.into();
    }
    // If it's Cow::Borrowed, there was no match for this pattern, so no
    // need to pay for a new string allocation.

    content.rendered = result.into();
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
    }

    mod callouts {
        use crate::{
            content::{Content, SubstitutionStep},
            strings::CowStr,
            tests::prelude::*,
        };

        #[test]
        #[should_panic]
        fn not_yet_implemented() {
            let mut content = Content::from(crate::Span::default());
            let p = Parser::default();
            SubstitutionStep::Callouts.apply(&mut content, &p, None);
            assert!(content.is_empty());
            assert_eq!(content.rendered, CowStr::Borrowed(""));
        }
    }
}
