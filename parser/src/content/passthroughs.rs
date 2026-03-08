use std::{borrow::Cow, sync::LazyLock};

use regex::{Captures, Regex, Replacer};

use crate::{
    Parser, Span,
    attributes::{Attrlist, AttrlistContext},
    content::{Content, SubstitutionGroup},
    parser::{QuoteScope, QuoteType},
};

/// Saves the content of one passthrough (`+++` or similarly bracketed) passage
/// for later re-expansion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Passthrough {
    pub(crate) text: String,
    pub(crate) subs: SubstitutionGroup,
    pub(crate) type_: Option<QuoteType>,
    pub(crate) attrlist: Option<String>,
}

/// Saves content of passthrough (`+++`-bracketed) passages for later
/// re-expansion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Passthroughs(pub(crate) Vec<Passthrough>);

impl Passthroughs {
    pub(crate) fn extract_from(content: &mut Content<'_>) -> Self {
        let mut passthroughs = Self(vec![]);

        // TRANSLATION GUIDE:
        // * compat_mode => always false
        // * passthroughs => self.saved_spans
        // * old_behavior => appears to affect the entire span

        {
            let text = content.rendered.as_ref();
            if text.contains("++") || text.contains("$$") || text.contains("ss:") {
                let replacer = InlinePassMacroReplacer(&mut passthroughs);

                if let Cow::Owned(new_result) = INLINE_PASS_MACRO.replace_all(text, replacer) {
                    content.rendered = new_result.into();
                }
            }
        }

        {
            let text = content.rendered.as_ref();
            if text.contains('+') || text.contains("-]") {
                let replacer = InlinePassReplacer(&mut passthroughs);

                if let Cow::Owned(new_result) = INLINE_PASS.replace_all(text, replacer) {
                    content.rendered = new_result.into();
                }
            }
        }

        // TO DO (#261): When implementing STEM macros, look for the block that starts
        // with `text.gsub InlineStemMacroRx do` in Ruby Asciidoctor's substitutors.rb
        // file.

        passthroughs
    }

    pub(crate) fn restore_to(&self, content: &mut Content<'_>, parser: &Parser) {
        if self.0.is_empty() {
            return;
        }

        let replacer = PassthroughRestoreReplacer(self, parser);

        if let Cow::Owned(new_result) =
            PASS_WITH_INDEX.replace_all(content.rendered().as_ref(), replacer)
        {
            content.rendered = new_result.into();
        }
    }

    pub(super) fn push(&mut self, passthrough: Passthrough, dest: &mut String) {
        let index = self.0.len();
        self.0.push(passthrough);

        dest.push('\u{96}');
        dest.push_str(&format!("{index}"));
        dest.push('\u{97}');
    }
}

/// Matches several variants of the passthrough inline macro, which may span
/// multiple lines.
///
/// ## Examples
///
/// * `+++text+++`
/// * `$$text$$`
/// * `pass:quotes[text]`
///
/// NOTE: We have to support an empty `pass:[]` for compatibility with
/// AsciiDoc.py.
static INLINE_PASS_MACRO: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(
        r#"(?xs)
        (?:
            # Optional: attrlist
            (?:
                (\\?)              # Group 1: optional backslash before [
                \[
                    ([^\[\]]+)     # Group 2: attrlist contents
                \]
            )?
            
            (\\{0,2})              # Group 3: optional escape prefix (e.g., \ or \\)

            # Passthrough span delimiters: +++, ++, or $$
            (?:
                (\+\+\+) (.*?) (\+\+\+) |   # Groups 4,5,6: triple plus
                (\+\+)   (.*?) (\+\+)   |   # Groups 7,8,9: double plus
                (\$\$)   (.*?) (\$\$)       # Groups 10,11,12: double dollar
            )

        |

            # Alternative: pass-through directive
            (\\?)                       # Group 13: optional escape before pass
            pass:
                ([a-z]+(?:,[a-z-]+)*)?  # Group 14: optional substitution step list
                \[
                     (|.*?[^\\])        # Group 15: optional content
                                        # (avoiding escape of trailing bracket)
                \]
        )"#,
    )
    .unwrap()
});

#[derive(Debug)]
struct InlinePassMacroReplacer<'p>(&'p mut Passthroughs);

impl Replacer for InlinePassMacroReplacer<'_> {
    fn replace_append(&mut self, caps: &Captures<'_>, dest: &mut String) {
        if caps.get(4).is_some() {
            // +++
            self.handle_quoted_text(caps, 5, dest);
        } else if caps.get(7).is_some() {
            // ++
            self.handle_quoted_text(caps, 8, dest);
        } else if caps.get(10).is_some() {
            // %%
            self.handle_quoted_text(caps, 11, dest);
        } else {
            // NOTE: We don't look for nested `pass:[]` macros.

            if caps.get(13).is_some_and(|m| !m.as_str().is_empty()) {
                // Honor escape of `pass:` macro.
                dest.push_str("pass:");
                if let Some(subs) = caps.get(14) {
                    dest.push_str(subs.as_str());
                }
                dest.push('[');
                dest.push_str(&caps[15]);
                dest.push(']');
                return;
            }

            let subs = caps
                .get(14)
                .and_then(|m| SubstitutionGroup::from_custom_string(None, m.as_str()))
                .unwrap_or(SubstitutionGroup::None);

            let mut text = caps[15].to_string();
            if !text.is_empty() {
                text = text.replace("\\]", "]");
            }

            self.0.push(
                Passthrough {
                    text,
                    subs,
                    type_: None,
                    attrlist: None,
                },
                dest,
            );
        }
    }
}

impl InlinePassMacroReplacer<'_> {
    fn handle_quoted_text(
        &mut self,
        caps: &Captures<'_>,
        quoted_text_index: usize,
        dest: &mut String,
    ) {
        let escape_count = caps.get(3).map_or(0, |m| m.len());

        let boundary = caps.get(4).or_else(|| caps.get(7)).or_else(|| caps.get(10));
        let boundary = boundary.map(|m| m.as_str()).unwrap_or_default();

        let quoted_text = caps.get(5).or_else(|| caps.get(8)).or_else(|| caps.get(11));
        let quoted_text = quoted_text.map(|m| m.as_str()).unwrap_or_default();

        let mut old_behavior = false;

        let attrlist: Option<String> = if let Some(attrlist) = caps.get(2) {
            let attrlist = attrlist.as_str();

            if escape_count > 0 {
                dest.push_str(caps[1].as_ref());
                dest.push('[');
                dest.push_str(caps[2].as_ref());
                dest.push(']');
                dest.push_str(&("\\".repeat(escape_count - 1)));
                dest.push_str(caps[quoted_text_index - 1].as_ref());
                dest.push_str(caps[quoted_text_index].as_ref());
                dest.push_str(caps[quoted_text_index - 1].as_ref());
                return;
            }

            if &caps[1] == "\\" {
                dest.push_str(&format!("[{attrlist}]", attrlist = &caps[2]));
                None
            } else if boundary == "++" {
                if attrlist == "x-" {
                    old_behavior = true;
                    Some("".to_owned())
                } else if attrlist.ends_with(" x-") {
                    old_behavior = true;
                    Some(attrlist[0..attrlist.len() - 3].to_owned())
                } else {
                    Some(attrlist.to_owned())
                }
            } else {
                Some(attrlist.to_owned())
            }
        } else if escape_count > 0 {
            // NOTE: We don't look for nested unconstrained pass macros.
            dest.push_str(&("\\".repeat(escape_count - 1)));
            dest.push_str(boundary);
            dest.push_str(quoted_text);
            dest.push_str(boundary);
            return;
        } else {
            None
        };

        let passthrough = if let Some(attrlist) = attrlist {
            if old_behavior {
                Passthrough {
                    text: caps
                        .get(quoted_text_index)
                        .map(|m| m.as_str().to_owned())
                        .unwrap_or_default(),
                    subs: SubstitutionGroup::Normal,
                    type_: Some(QuoteType::Monospaced),
                    attrlist: Some(attrlist),
                }
            } else {
                Passthrough {
                    text: caps
                        .get(quoted_text_index)
                        .map(|m| m.as_str().to_owned())
                        .unwrap_or_default(),
                    subs: if boundary == "+++" {
                        SubstitutionGroup::None
                    } else {
                        SubstitutionGroup::Verbatim
                    },
                    type_: Some(QuoteType::Unquoted),
                    attrlist: Some(attrlist),
                }
            }
        } else {
            Passthrough {
                text: caps
                    .get(quoted_text_index)
                    .map(|m| m.as_str().to_owned())
                    .unwrap_or_default(),
                subs: if boundary == "+++" {
                    SubstitutionGroup::None
                } else {
                    SubstitutionGroup::Verbatim
                },
                type_: None,
                attrlist: None,
            }
        };

        self.0.push(passthrough, dest);
    }
}

static PASS_WITH_INDEX: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new("\u{96}(\\d+)\u{97}").unwrap()
});

/// Matches an inline passthrough, which may span multiple lines.
///
/// ## Examples
///
/// * `+text+`
/// * `[x-]+text+`
/// * `[x-]\`text\``
///
/// NOTE: We do not support compat-mode in the Rust implementation.
static INLINE_PASS: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(
        r#"(?xs)
            \b{start-half}              # Must not follow a word

            (?:
                                        # Option 1: [... x-] followed by `xxx`
                \[(x-|[^\[\]]+\ x-)\]       # Group 1: [attrlist] with x- suffix
                \`(\S(?:.*?\S)??)\`         # Group 2: `...` content
            
            |                           # --OR--
                                        # Option 2: [...] followed by +xxx+
                \[([^\[\]]+)\]              # Group 3: [attrlist]
                (\\{0,2})                   # Group 4: optional escapes
                \+(\S(?:.*?\S)??)\+         # Group 5: +...+ content (surrounded by non-space)

            |                           # --OR--
                                        # Option 3: +xxx+ without attrlist
                (\\)?                       # Group 6: optional escape
                \+(\S(?:.*?\S)??)\+         # Group 7: +...+ content (surrounded by non-space)

            )

            \b{end-half}            # Must not be followed by a word character
        "#,
    )
    .unwrap()
});

#[derive(Debug)]
struct InlinePassReplacer<'p>(&'p mut Passthroughs);

impl Replacer for InlinePassReplacer<'_> {
    fn replace_append(&mut self, caps: &Captures<'_>, dest: &mut String) {
        if dest.ends_with('\\') || dest.ends_with(':') || dest.ends_with(';') {
            // Honor the prohibited prefix.
            // let dest_ended_with_backslash = dest.ends_with('\\');
            // if dest_ended_with_backslash {
            //     dest.truncate(dest.len() - 1);
            // }

            // EDGE CASE: Since we don't have lookarounds in Rust's regex, we have to retry
            // the inline pass replacement here. Possible it might miss a few very obscure
            // cases, but this should cover most cases where the attrlist is off-limits, but
            // the quoted text is still subject to inline pass replacement.
            let replacer = InlinePassReplacer(self.0);

            let (first, rem) = &caps[0].split_at(1);
            dest.push_str(first);

            let new_result = INLINE_PASS.replace_all(rem, replacer);
            dest.push_str(&new_result);

            return;
        }

        let escapes = caps.get(4).or_else(|| caps.get(6));
        let escape_count = escapes.map_or(0, |m| m.len());

        let format_mark = if caps.get(2).is_some() { '`' } else { '+' };
        let orig_attrlist_body = caps.get(1).or_else(|| caps.get(3)).map(|m| m.as_str());

        let (attrlist_body, old_behavior) = orig_attrlist_body.map_or((None, false), |m| {
            if m == "x-" {
                (Some("".to_string()), true)
            } else if m.ends_with(" x-") {
                (Some(m[0..m.len() - 3].to_string()), true)
            } else {
                (Some(m.to_string()), false)
            }
        });

        let quoted_text = caps.get(2).or_else(|| caps.get(5)).or_else(|| caps.get(7));
        let quoted_text = quoted_text.map_or("", |m| m.as_str());

        if let Some(orig_attrlist_body) = orig_attrlist_body {
            if escape_count > 0 {
                // Honor the escape of the formatting mark.
                dest.push('[');
                dest.push_str(orig_attrlist_body);
                dest.push(']');
                dest.push_str(&("\\".repeat(escape_count - 1)));
                dest.push(format_mark);
                dest.push_str(quoted_text);
                dest.push(format_mark);
                return;
            }
        } else if escape_count > 0 {
            // Honor the escape of the formatting mark.
            dest.push_str(&("\\".repeat(escape_count - 1)));
            dest.push(format_mark);
            dest.push_str(quoted_text);
            dest.push(format_mark);
            return;
        };

        let subs = if attrlist_body.is_some() && old_behavior && format_mark != '`' {
            SubstitutionGroup::Normal
        } else {
            SubstitutionGroup::Verbatim
        };

        let type_ = if attrlist_body.is_some() {
            if old_behavior {
                Some(QuoteType::Monospaced)
            } else {
                Some(QuoteType::Unquoted)
            }
        } else {
            None
        };

        self.0.push(
            Passthrough {
                text: quoted_text.to_string(),
                subs,
                type_,
                attrlist: attrlist_body,
            },
            dest,
        );
    }
}

#[derive(Debug)]
struct PassthroughRestoreReplacer<'p>(&'p Passthroughs, &'p Parser);

impl Replacer for PassthroughRestoreReplacer<'_> {
    fn replace_append(&mut self, caps: &Captures<'_>, dest: &mut String) {
        let index = caps[1].parse::<usize>().unwrap_or_default();

        let Some(pass) = self.0.0.get(index) else {
            dest.push_str(&format!(
                "(INTERNAL ERROR: Unresolved passthrough index {index})"
            ));
            return;
        };

        let span = Span::new(&pass.text);

        let mut subbed_text = Content::from(span);
        pass.subs.apply(&mut subbed_text, self.1, None);

        if let Some(type_) = pass.type_ {
            let attrlist = pass.attrlist.as_ref().map(|attrlist_body| {
                let span = Span::new(attrlist_body);
                let maw = Attrlist::parse(span, self.1, AttrlistContext::Inline);
                maw.item.item
            });

            let id = attrlist
                .as_ref()
                .and_then(|attrlist| attrlist.id().map(|id| id.to_string()));

            let mut new_text = String::default();
            self.1.renderer.render_quoted_substitition(
                type_,
                QuoteScope::Unconstrained,
                attrlist,
                id,
                subbed_text.rendered(),
                &mut new_text,
            );

            subbed_text.rendered = new_text.into();
        }

        if subbed_text.rendered().contains('\u{96}') {
            // Recursively apply passthrough replacement and write the result.
            let replacer = PassthroughRestoreReplacer(self.0, self.1);

            let new_result = PASS_WITH_INDEX.replace_all(subbed_text.rendered().as_ref(), replacer);

            dest.push_str(new_result.as_ref());
        } else {
            dest.push_str(subbed_text.rendered());
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::unwrap_used)]

    use crate::{
        blocks::SimpleBlockStyle,
        content::{Passthroughs, SubstitutionStep, passthroughs::Passthrough},
        parser::ModificationContext,
        tests::prelude::*,
    };

    #[test]
    fn inline_double_plus_with_escaped_attrlist() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new(r#"abc \[attrs]++text++"#), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"abc \[attrs]++text++"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "abc [attrs]text",
                },
                source: Span {
                    data: r#"abc \[attrs]++text++"#,
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },)
        );
    }

    #[test]
    fn adds_warning_text_for_unresolved_passthrough_id() {
        let mut content =
            crate::content::Content::from(crate::Span::new("pass:q,a[*<{backend}>*]"));
        let pt = Passthroughs::extract_from(&mut content);

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "pass:q,a[*<{backend}>*]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "\u{96}0\u{97}",
            }
        );

        assert_eq!(
            pt,
            Passthroughs(vec![Passthrough {
                text: "*<{backend}>*".to_owned(),
                subs: SubstitutionGroup::Custom(vec![
                    SubstitutionStep::Quotes,
                    SubstitutionStep::AttributeReferences,
                ]),
                type_: None,
                attrlist: None,
            },],)
        );

        let parser = Parser::default().with_intrinsic_attribute(
            "backend",
            "html5",
            ModificationContext::ApiOnly,
        );

        pt.0[0].subs.apply(&mut content, &parser, None);

        content.rendered = "\u{96}99\u{97}".into();

        pt.restore_to(&mut content, &parser);

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "pass:q,a[*<{backend}>*]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "(INTERNAL ERROR: Unresolved passthrough index 99)",
            }
        );
    }
}
