use crate::{
    Parser,
    attributes::Attrlist,
    content::{Content, Passthroughs, SubstitutionStep},
};

/// Each block and inline element has a default substitution group that is
/// applied unless you customize the substitutions for a particular element.
///
/// `SubstitutionGroup` specifies the default or overridden substitution group
/// to be applied.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SubstitutionGroup {
    /// The normal substitution group is applied to the majority of the AsciiDoc
    /// block and inline elements except for specific elements described in the
    /// next sections.
    Normal,

    /// The title substitution group is applied to section and block titles.
    /// It uses the same substitution steps as Normal.
    Title,

    /// The header substitution group is applied to metadata lines (author and
    /// revision information) in the document header. It’s also applied to the
    /// values of attribute entries, regardless of whether those entries are
    /// defined in the document header or body. Only special characters,
    /// attribute references, and the inline pass macro are replaced in elements
    /// that fall under the header group.
    ///
    /// You can use the inline pass macro in attribute entries to customize the
    /// substitution types applied to the attribute’s value.
    Header,

    /// Literal, listing, and source blocks are processed using the verbatim
    /// substitution group. Only special characters are replaced in these
    /// blocks.
    Verbatim,

    /// No substitutions are applied to three of the elements in the pass
    /// substitution group. These elements include the passthrough block, inline
    /// pass macro, and triple plus macro.
    ///
    /// The inline single plus and double plus macros also belong to the pass
    /// group. Only the special characters substitution is applied to these
    /// elements.
    Pass,

    /// The none substitution group is applied to comment blocks. No
    /// substitutions are applied to comments.
    None,

    /// The attribute entry value substitution group is applied to attribute
    /// values. Only special characters and attribute references are applied to
    /// these values.
    AttributeEntryValue,

    /// You can customize the substitutions applied to the content of an inline
    /// pass macro by specifying one or more substitution values. Multiple
    /// values must be separated by commas and may not contain any spaces. The
    /// substitution value is either the formal name of a substitution type or
    /// group, or its shorthand.
    ///
    /// See [Custom substitutions].
    ///
    /// [Custom substitutions]: https://docs.asciidoctor.org/asciidoc/latest/pass/pass-macro/#custom-substitutions
    Custom(Vec<SubstitutionStep>),
}

impl SubstitutionGroup {
    /// Parse the custom substitution group syntax defined in [Custom
    /// substitutions].
    ///
    /// [Custom substitutions]: https://docs.asciidoctor.org/asciidoc/latest/pass/pass-macro/#custom-substitutions
    pub(crate) fn from_custom_string(start_from: Option<&Self>, mut custom: &str) -> Option<Self> {
        custom = custom.trim();

        if custom == "none" {
            return Some(Self::None);
        }

        if custom == "n" || custom == "normal" {
            return Some(Self::Normal);
        }

        if custom == "v" || custom == "verbatim" {
            return Some(Self::Verbatim);
        }

        let mut steps: Vec<SubstitutionStep> = vec![];

        for (count, mut step) in custom.split(",").enumerate() {
            step = step.trim();

            if step == "n" || step == "normal" {
                steps = vec![
                    SubstitutionStep::SpecialCharacters,
                    SubstitutionStep::Quotes,
                    SubstitutionStep::AttributeReferences,
                    SubstitutionStep::CharacterReplacements,
                    SubstitutionStep::Macros,
                    SubstitutionStep::PostReplacement,
                ];
                continue;
            }

            if step == "v" || step == "verbatim" {
                steps = vec![SubstitutionStep::SpecialCharacters];
                continue;
            }

            let append = if step.starts_with('+') {
                step = &step[1..];
                true
            } else {
                false
            };

            let prepend = if !append && step.ends_with('+') {
                step = &step[0..step.len() - 1];
                true
            } else {
                false
            };

            let subtract = if !append && !prepend && step.starts_with('-') {
                step = &step[1..];
                true
            } else {
                false
            };

            if count == 0
                && let Some(start_from) = start_from
                && (append || prepend || subtract)
            {
                steps = start_from.steps().to_owned();
            }

            let step = match step {
                "c" | "specialcharacters" | "specialchars" => SubstitutionStep::SpecialCharacters,
                "q" | "quotes" => SubstitutionStep::Quotes,
                "a" | "attributes" => SubstitutionStep::AttributeReferences,
                "r" | "replacements" => SubstitutionStep::CharacterReplacements,
                "m" | "macros" => SubstitutionStep::Macros,
                "p" | "post_replacements" => SubstitutionStep::PostReplacement,
                _ => {
                    return None;
                }
            };

            if prepend {
                steps.insert(0, step);
            } else if append {
                steps.push(step);
            } else if subtract {
                steps.retain(|s| s != &step);
            } else {
                steps.push(step);
            }
        }

        Some(Self::Custom(steps))
    }

    pub(crate) fn apply(
        &self,
        content: &mut Content<'_>,
        parser: &Parser,
        attrlist: Option<&Attrlist>,
    ) {
        let steps = self.steps();

        let passthroughs: Option<Passthroughs> =
            if steps.contains(&SubstitutionStep::Macros) || self == &Self::Header {
                Some(Passthroughs::extract_from(content))
            } else {
                None
            };

        for step in steps {
            step.apply(content, parser, attrlist);
        }

        if let Some(passthroughs) = passthroughs {
            passthroughs.restore_to(content, parser);
        }
    }

    pub(crate) fn override_via_attrlist(&self, attrlist: Option<&Attrlist>) -> Self {
        let mut result = self.clone();

        if let Some(attrlist) = attrlist {
            if let Some(block_style) = attrlist.nth_attribute(1).and_then(|a| a.block_style()) {
                result = match block_style {
                    // TO DO: Many other style-specific substitution groups.
                    "pass" => SubstitutionGroup::None,
                    _ => result,
                };
            }

            if let Some(sub_group) = attrlist
                .named_attribute("subs")
                .map(|attr| attr.value())
                .and_then(|s| Self::from_custom_string(Some(self), s))
            {
                result = sub_group;
            }
        }

        result
    }

    fn steps(&self) -> &[SubstitutionStep] {
        match self {
            Self::Normal | Self::Title => &[
                SubstitutionStep::SpecialCharacters,
                SubstitutionStep::Quotes,
                SubstitutionStep::AttributeReferences,
                SubstitutionStep::CharacterReplacements,
                SubstitutionStep::Macros,
                SubstitutionStep::PostReplacement,
            ],

            Self::Header | Self::AttributeEntryValue => &[
                SubstitutionStep::SpecialCharacters,
                SubstitutionStep::AttributeReferences,
            ],

            Self::Verbatim => &[SubstitutionStep::SpecialCharacters],

            Self::Pass | Self::None => &[],

            Self::Custom(steps) => steps,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    mod from_custom_string {
        use crate::{
            Parser,
            content::{Content, SubstitutionGroup, SubstitutionStep},
            strings::CowStr,
        };

        #[test]
        fn empty() {
            assert_eq!(SubstitutionGroup::from_custom_string(None, ""), None);
        }

        #[test]
        fn none() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "none"),
                Some(SubstitutionGroup::None)
            );

            assert_eq!(SubstitutionGroup::from_custom_string(None, "nermal"), None);
        }

        #[test]
        fn normal() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "n"),
                Some(SubstitutionGroup::Normal)
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "normal"),
                Some(SubstitutionGroup::Normal)
            );

            assert_eq!(SubstitutionGroup::from_custom_string(None, "nermal"), None);
        }

        #[test]
        fn verbatim() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "v"),
                Some(SubstitutionGroup::Verbatim)
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "verbatim"),
                Some(SubstitutionGroup::Verbatim)
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "verboten"),
                None
            );
        }

        #[test]
        fn special_chars() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "c"),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::SpecialCharacters
                ]))
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "specialchars"),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::SpecialCharacters
                ]))
            );
        }

        #[test]
        fn quotes() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "q"),
                Some(SubstitutionGroup::Custom(vec![SubstitutionStep::Quotes]))
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "quotes"),
                Some(SubstitutionGroup::Custom(vec![SubstitutionStep::Quotes]))
            );
        }

        #[test]
        fn attributes() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "a"),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::AttributeReferences
                ]))
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "attributes"),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::AttributeReferences
                ]))
            );
        }

        #[test]
        fn replacements() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "r"),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::CharacterReplacements
                ]))
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "replacements"),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::CharacterReplacements
                ]))
            );
        }

        #[test]
        fn macros() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "m"),
                Some(SubstitutionGroup::Custom(vec![SubstitutionStep::Macros]))
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "macros"),
                Some(SubstitutionGroup::Custom(vec![SubstitutionStep::Macros]))
            );
        }

        #[test]
        fn post_replacements() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "p"),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::PostReplacement
                ]))
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "post_replacements"),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::PostReplacement
                ]))
            );
        }

        #[test]
        fn multiple() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "q,a"),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::Quotes,
                    SubstitutionStep::AttributeReferences
                ]))
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "q, a"),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::Quotes,
                    SubstitutionStep::AttributeReferences
                ]))
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "quotes,attributes"),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::Quotes,
                    SubstitutionStep::AttributeReferences
                ]))
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "x,bogus,no such step"),
                None
            );
        }

        #[test]
        fn subtraction() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "n,-r"),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::SpecialCharacters,
                    SubstitutionStep::Quotes,
                    SubstitutionStep::AttributeReferences,
                    SubstitutionStep::Macros,
                    SubstitutionStep::PostReplacement,
                ]))
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "n,-r,-r,-m"),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::SpecialCharacters,
                    SubstitutionStep::Quotes,
                    SubstitutionStep::AttributeReferences,
                    SubstitutionStep::PostReplacement,
                ]))
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "v,-r"),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::SpecialCharacters,
                ]))
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "v,-c"),
                Some(SubstitutionGroup::Custom(vec![]))
            );
        }

        #[test]
        fn addition() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "n,r"),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::SpecialCharacters,
                    SubstitutionStep::Quotes,
                    SubstitutionStep::AttributeReferences,
                    SubstitutionStep::CharacterReplacements,
                    SubstitutionStep::Macros,
                    SubstitutionStep::PostReplacement,
                    SubstitutionStep::CharacterReplacements,
                ]))
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "v,m"),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::SpecialCharacters,
                    SubstitutionStep::Macros,
                ]))
            );
        }

        #[test]
        fn incremental() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "n,r"),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::SpecialCharacters,
                    SubstitutionStep::Quotes,
                    SubstitutionStep::AttributeReferences,
                    SubstitutionStep::CharacterReplacements,
                    SubstitutionStep::Macros,
                    SubstitutionStep::PostReplacement,
                    SubstitutionStep::CharacterReplacements,
                ]))
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "v,m"),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::SpecialCharacters,
                    SubstitutionStep::Macros,
                ]))
            );
        }

        #[test]
        fn prepend() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(
                    Some(&SubstitutionGroup::Verbatim),
                    "attributes+"
                ),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::AttributeReferences,
                    SubstitutionStep::SpecialCharacters,
                ]))
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "attributes+"),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::AttributeReferences,
                ]))
            );
        }

        #[test]
        fn append() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(
                    Some(&SubstitutionGroup::Verbatim),
                    "+attributes"
                ),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::SpecialCharacters,
                    SubstitutionStep::AttributeReferences,
                ]))
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "attributes+"),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::AttributeReferences,
                ]))
            );
        }

        #[test]
        fn subtract() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(
                    Some(&SubstitutionGroup::Normal),
                    "-attributes"
                ),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::SpecialCharacters,
                    SubstitutionStep::Quotes,
                    SubstitutionStep::CharacterReplacements,
                    SubstitutionStep::Macros,
                    SubstitutionStep::PostReplacement,
                ]))
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "-attributes"),
                Some(SubstitutionGroup::Custom(vec![]))
            );
        }

        #[test]
        fn custom_group_with_macros_preserves_passthroughs() {
            let custom_group = SubstitutionGroup::from_custom_string(None, "q,m").unwrap();

            let mut content = Content::from(crate::Span::new(
                "Text with +++pass<through>+++ icon:github[] content.",
            ));
            let p = Parser::default();
            custom_group.apply(&mut content, &p, None);

            assert!(!content.is_empty());
            assert_eq!(
                content.rendered,
                CowStr::Boxed(
                    "Text with pass<through> <span class=\"icon\">[github&#93;</span> content."
                        .to_string()
                        .into_boxed_str()
                )
            );
        }
    }

    mod normal {
        use crate::{
            Parser,
            content::{Content, SubstitutionGroup},
            strings::CowStr,
        };

        #[test]
        fn empty() {
            let mut content = Content::from(crate::Span::default());
            let p = Parser::default();
            SubstitutionGroup::Normal.apply(&mut content, &p, None);
            assert!(content.is_empty());
            assert_eq!(content.rendered, CowStr::Borrowed(""));
        }

        #[test]
        fn basic_non_empty_span() {
            let mut content = Content::from(crate::Span::new("blah"));
            let p = Parser::default();
            SubstitutionGroup::Normal.apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(content.rendered, CowStr::Borrowed("blah"));
        }

        #[test]
        fn match_lt_and_gt() {
            let mut content = Content::from(crate::Span::new("bl<ah>"));
            let p = Parser::default();
            SubstitutionGroup::Normal.apply(&mut content, &p, None);
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
            SubstitutionGroup::Normal.apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(
                content.rendered,
                CowStr::Boxed("bl&lt;a&amp;h&gt;".to_string().into_boxed_str())
            );
        }

        #[test]
        fn strong_word() {
            let mut content = Content::from(crate::Span::new("One *word* is strong."));
            let p = Parser::default();
            SubstitutionGroup::Normal.apply(&mut content, &p, None);
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
        fn strong_word_with_special_chars() {
            let mut content = Content::from(crate::Span::new("One *wo<r>d* is strong."));
            let p = Parser::default();
            SubstitutionGroup::Normal.apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(
                content.rendered,
                CowStr::Boxed(
                    "One <strong>wo&lt;r&gt;d</strong> is strong."
                        .to_string()
                        .into_boxed_str()
                )
            );
        }

        #[test]
        fn marked_string_with_id() {
            let mut content = Content::from(crate::Span::new(r#"[#id]#a few words#"#));
            let p = Parser::default();
            SubstitutionGroup::Normal.apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(
                content.rendered,
                CowStr::Boxed(r#"<span id="id">a few words</span>"#.to_string().into_boxed_str())
            );
        }
    }

    mod attribute_entry_value {
        use crate::{
            Parser,
            content::{Content, SubstitutionGroup},
            parser::ModificationContext,
            strings::CowStr,
        };

        #[test]
        fn empty() {
            let mut content = Content::from(crate::Span::default());
            let p = Parser::default();
            SubstitutionGroup::AttributeEntryValue.apply(&mut content, &p, None);
            assert!(content.is_empty());
            assert_eq!(content.rendered, CowStr::Borrowed(""));
        }

        #[test]
        fn basic_non_empty_span() {
            let mut content = Content::from(crate::Span::new("blah"));
            let p = Parser::default();
            SubstitutionGroup::AttributeEntryValue.apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(content.rendered, CowStr::Borrowed("blah"));
        }

        #[test]
        fn match_lt_and_gt() {
            let mut content = Content::from(crate::Span::new("bl<ah>"));
            let p = Parser::default();
            SubstitutionGroup::Normal.apply(&mut content, &p, None);
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
            SubstitutionGroup::AttributeEntryValue.apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(
                content.rendered,
                CowStr::Boxed("bl&lt;a&amp;h&gt;".to_string().into_boxed_str())
            );
        }

        #[test]
        fn ignores_strong_word() {
            let mut content = Content::from(crate::Span::new("One *word* is strong."));
            let p = Parser::default();
            SubstitutionGroup::AttributeEntryValue.apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(
                content.rendered,
                CowStr::Boxed("One *word* is strong.".to_string().into_boxed_str())
            );
        }

        #[test]
        fn special_chars_and_attributes() {
            let mut content = Content::from(crate::Span::new("bl<ah> {color}"));

            let p = Parser::default().with_intrinsic_attribute(
                "color",
                "red",
                ModificationContext::Anywhere,
            );

            SubstitutionGroup::AttributeEntryValue.apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(
                content.rendered,
                CowStr::Boxed("bl&lt;ah&gt; red".to_string().into_boxed_str())
            );
        }
    }

    mod header {
        use crate::{
            Parser,
            content::{Content, SubstitutionGroup},
            strings::CowStr,
        };

        #[test]
        fn empty() {
            let mut content = Content::from(crate::Span::default());
            let p = Parser::default();
            SubstitutionGroup::Header.apply(&mut content, &p, None);
            assert!(content.is_empty());
            assert_eq!(content.rendered, CowStr::Borrowed(""));
        }

        #[test]
        fn basic_non_empty_span() {
            let mut content = Content::from(crate::Span::new("blah"));
            let p = Parser::default();
            SubstitutionGroup::Header.apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(content.rendered, CowStr::Borrowed("blah"));
        }

        #[test]
        fn match_lt_and_gt() {
            let mut content = Content::from(crate::Span::new("bl<ah>"));
            let p = Parser::default();
            SubstitutionGroup::Header.apply(&mut content, &p, None);
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
            SubstitutionGroup::Header.apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(
                content.rendered,
                CowStr::Boxed("bl&lt;a&amp;h&gt;".to_string().into_boxed_str())
            );
        }

        #[test]
        fn ignores_strong_word() {
            let mut content = Content::from(crate::Span::new("One *word* is strong."));
            let p = Parser::default();
            SubstitutionGroup::Header.apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(content.rendered, CowStr::Borrowed("One *word* is strong."));
        }

        #[test]
        fn ignores_strong_word_with_special_chars() {
            let mut content = Content::from(crate::Span::new("One *wo<r>d* is strong."));
            let p = Parser::default();
            SubstitutionGroup::Header.apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(
                content.rendered,
                CowStr::Boxed("One *wo&lt;r&gt;d* is strong.".to_string().into_boxed_str())
            );
        }

        #[test]
        fn ignores_marked_string_with_id() {
            let mut content = Content::from(crate::Span::new(r#"[#id]#a few words#"#));
            let p = Parser::default();
            SubstitutionGroup::Header.apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(content.rendered, CowStr::Borrowed("[#id]#a few words#"));
        }
    }

    mod title {
        use crate::{
            Parser,
            content::{Content, SubstitutionGroup},
            strings::CowStr,
        };

        #[test]
        fn empty() {
            let mut content = Content::from(crate::Span::default());
            let p = Parser::default();
            SubstitutionGroup::Title.apply(&mut content, &p, None);
            assert!(content.is_empty());
            assert_eq!(content.rendered, CowStr::Borrowed(""));
        }

        #[test]
        fn basic_non_empty_span() {
            let mut content = Content::from(crate::Span::new("blah"));
            let p = Parser::default();
            SubstitutionGroup::Title.apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(content.rendered, CowStr::Borrowed("blah"));
        }

        #[test]
        fn match_lt_and_gt() {
            let mut content = Content::from(crate::Span::new("bl<ah>"));
            let p = Parser::default();
            SubstitutionGroup::Title.apply(&mut content, &p, None);
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
            SubstitutionGroup::Title.apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(
                content.rendered,
                CowStr::Boxed("bl&lt;a&amp;h&gt;".to_string().into_boxed_str())
            );
        }

        #[test]
        fn strong_word() {
            let mut content = Content::from(crate::Span::new("One *word* is strong."));
            let p = Parser::default();
            SubstitutionGroup::Title.apply(&mut content, &p, None);
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
        fn strong_word_with_special_chars() {
            let mut content = Content::from(crate::Span::new("One *wo<r>d* is strong."));
            let p = Parser::default();
            SubstitutionGroup::Title.apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(
                content.rendered,
                CowStr::Boxed(
                    "One <strong>wo&lt;r&gt;d</strong> is strong."
                        .to_string()
                        .into_boxed_str()
                )
            );
        }

        #[test]
        fn marked_string_with_id() {
            let mut content = Content::from(crate::Span::new(r#"[#id]#a few words#"#));
            let p = Parser::default();
            SubstitutionGroup::Title.apply(&mut content, &p, None);
            assert!(!content.is_empty());
            assert_eq!(
                content.rendered,
                CowStr::Boxed(r#"<span id="id">a few words</span>"#.to_string().into_boxed_str())
            );
        }

        #[test]
        fn title_behaves_same_as_normal() {
            let test_input = "One *wo<r>d* is strong with [#id]#marked text#.";

            let mut title_content = Content::from(crate::Span::new(test_input));
            let mut normal_content = Content::from(crate::Span::new(test_input));
            let p = Parser::default();

            SubstitutionGroup::Title.apply(&mut title_content, &p, None);
            SubstitutionGroup::Normal.apply(&mut normal_content, &p, None);

            // Title should produce exactly the same result as Normal
            assert_eq!(title_content.rendered, normal_content.rendered);
        }
    }
}
