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

    /// The STEM substitution group is applied to STEM (`stem`, `asciimath`, and
    /// `latexmath`) content when no explicit substitution list is given. Only
    /// the special characters substitution is applied (Asciidoctor's basic subs
    /// for HTML output). Used by both the inline STEM macro and the STEM block.
    Stem,

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

        // An entirely empty string is not a substitution list; leave it to the
        // caller to decide what that means (warn for a pass macro, keep the
        // default group for a block).
        if custom.is_empty() {
            return None;
        }

        let mut steps: Vec<SubstitutionStep> = vec![];
        let mut first = true;

        for mut step in custom.split(",") {
            step = step.trim();

            // An empty entry contributes nothing, so a list of only separators
            // (e.g. `subs=","`) resolves to an empty substitution list. This
            // matches Asciidoctor's `resolve_subs`, where `','.split(',')`
            // yields no tokens.
            if step.is_empty() {
                continue;
            }

            let is_first = first;
            first = false;

            // A group name (`normal`/`verbatim`) is expanded *in place*: its
            // constituent steps are appended to the running list rather than
            // replacing it. This matches Asciidoctor's `resolve_subs`, where a
            // group name mid-list contributes its steps like any other token.
            if step == "n" || step == "normal" {
                steps.extend([
                    SubstitutionStep::SpecialCharacters,
                    SubstitutionStep::Quotes,
                    SubstitutionStep::AttributeReferences,
                    SubstitutionStep::CharacterReplacements,
                    SubstitutionStep::Macros,
                    SubstitutionStep::PostReplacement,
                ]);
                continue;
            }

            if step == "v" || step == "verbatim" {
                steps.extend([
                    SubstitutionStep::SpecialCharacters,
                    SubstitutionStep::Callouts,
                ]);
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

            if is_first
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
                "callouts" => SubstitutionStep::Callouts,
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

        // De-duplicate the final list, first occurrence winning. Asciidoctor
        // ensures each substitution runs at most once, so a step contributed by
        // more than one token (e.g. the `quotes` in `quotes,normal`) is kept
        // only in its earliest position.
        let mut deduped: Vec<SubstitutionStep> = Vec::with_capacity(steps.len());
        for step in steps {
            if !deduped.contains(&step) {
                deduped.push(step);
            }
        }

        Some(Self::Custom(deduped))
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
                Some(Passthroughs::extract_from(content, parser))
            } else {
                None
            };

        for step in steps {
            step.apply(content, parser, attrlist);
        }

        if let Some(passthroughs) = passthroughs {
            passthroughs.restore_to(content, parser);
        }

        // Capture any deferred cross-references as a placeholder template and
        // render the unresolved fallback, so `rendered()` is clean even before
        // references are resolved. This is a no-op when no cross-references were
        // found.
        content.finalize_deferred(&*parser.renderer);
    }

    pub(crate) fn override_via_attrlist(&self, attrlist: Option<&Attrlist>) -> Self {
        let mut result = self.clone();

        if let Some(attrlist) = attrlist {
            // A declared block style reinterprets a simple-content (paragraph)
            // block as another context, which can change the substitution group
            // that applies. This masquerade only affects blocks whose default
            // group is `Normal`: a delimited block's delimiter already fixes its
            // group (verbatim, pass, stem, etc.), and Asciidoctor does not let a
            // style keyword override it. So the mapping below is scoped to
            // `Normal` blocks, matching Asciidoctor's parser.
            if result == SubstitutionGroup::Normal
                && let Some(block_style) = attrlist.nth_attribute(1).and_then(|a| a.block_style())
            {
                result = match block_style {
                    // The verbatim masquerade styles (`literal`, `listing`, and
                    // `source`) apply only special characters and callouts.
                    "literal" | "listing" | "source" => SubstitutionGroup::Verbatim,

                    // The `pass` style excludes the content from all
                    // substitutions.
                    "pass" => SubstitutionGroup::None,

                    // Every other style (`normal`, `verse`, `quote`, `sidebar`,
                    // `example`, admonitions, …) keeps the normal substitution
                    // group.
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

            Self::Verbatim => &[
                SubstitutionStep::SpecialCharacters,
                SubstitutionStep::Callouts,
            ],

            Self::Stem => &[SubstitutionStep::SpecialCharacters],

            Self::Pass | Self::None => &[],

            Self::Custom(steps) => steps,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    mod stem {
        use crate::{content::Content, strings::CowStr, tests::prelude::*};

        #[test]
        fn applies_special_characters_only() {
            // The `Stem` group applies only the special characters substitution:
            // `<` is escaped, but quotes (`*bold*`) and attribute references
            // (`{color}`) are left untouched.
            let mut content = Content::from(crate::Span::new("*a* < {color}"));
            let p = Parser::default();
            SubstitutionGroup::Stem.apply(&mut content, &p, None);
            assert_eq!(
                content.rendered,
                CowStr::Boxed("*a* &lt; {color}".to_string().into_boxed_str())
            );
        }
    }

    mod from_custom_string {
        use crate::{
            content::{Content, SubstitutionStep},
            strings::CowStr,
            tests::prelude::*,
        };

        #[test]
        fn empty() {
            assert_eq!(SubstitutionGroup::from_custom_string(None, ""), None);
        }

        #[test]
        fn empty_entries() {
            // A list containing only separators resolves to an empty
            // substitution list (issue #784).
            assert_eq!(
                SubstitutionGroup::from_custom_string(Some(&SubstitutionGroup::Verbatim), ","),
                Some(SubstitutionGroup::Custom(vec![]))
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, " , ,"),
                Some(SubstitutionGroup::Custom(vec![]))
            );

            // Empty entries elsewhere in the list are skipped.
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "quotes,"),
                Some(SubstitutionGroup::Custom(vec![SubstitutionStep::Quotes]))
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "quotes,,macros"),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::Quotes,
                    SubstitutionStep::Macros
                ]))
            );

            // A leading empty entry doesn't count as the first token, so a
            // modifier that follows it still starts from the base group.
            assert_eq!(
                SubstitutionGroup::from_custom_string(
                    Some(&SubstitutionGroup::Verbatim),
                    ",+quotes"
                ),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::SpecialCharacters,
                    SubstitutionStep::Callouts,
                    SubstitutionStep::Quotes
                ]))
            );
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
                    SubstitutionStep::Callouts,
                ]))
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "v,-c"),
                Some(SubstitutionGroup::Custom(vec![SubstitutionStep::Callouts]))
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "v,-callouts"),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::SpecialCharacters,
                ]))
            );
        }

        #[test]
        fn addition() {
            // `n` expands to normal's steps (which already include
            // replacements); the trailing `r` is de-duplicated away, matching
            // Asciidoctor.
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "n,r"),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::SpecialCharacters,
                    SubstitutionStep::Quotes,
                    SubstitutionStep::AttributeReferences,
                    SubstitutionStep::CharacterReplacements,
                    SubstitutionStep::Macros,
                    SubstitutionStep::PostReplacement,
                ]))
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "v,m"),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::SpecialCharacters,
                    SubstitutionStep::Callouts,
                    SubstitutionStep::Macros,
                ]))
            );
        }

        #[test]
        fn incremental() {
            // `n` expands to normal's steps (which already include
            // replacements); the trailing `r` is de-duplicated away, matching
            // Asciidoctor.
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "n,r"),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::SpecialCharacters,
                    SubstitutionStep::Quotes,
                    SubstitutionStep::AttributeReferences,
                    SubstitutionStep::CharacterReplacements,
                    SubstitutionStep::Macros,
                    SubstitutionStep::PostReplacement,
                ]))
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "v,m"),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::SpecialCharacters,
                    SubstitutionStep::Callouts,
                    SubstitutionStep::Macros,
                ]))
            );
        }

        #[test]
        fn group_name_mid_list_expands_in_place_and_dedups() {
            // A group name (`normal`) appearing mid-list is expanded in place
            // and appended to what came before, rather than resetting the
            // accumulated steps. The leading `quotes` is preserved, and the
            // redundant `quotes` from `normal`'s expansion is de-duplicated
            // away, matching Asciidoctor's `resolve_subs`.
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "quotes,normal"),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::Quotes,
                    SubstitutionStep::SpecialCharacters,
                    SubstitutionStep::AttributeReferences,
                    SubstitutionStep::CharacterReplacements,
                    SubstitutionStep::Macros,
                    SubstitutionStep::PostReplacement,
                ]))
            );

            // Same behavior for the shorthand `v` group name mid-list.
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "m,v"),
                Some(SubstitutionGroup::Custom(vec![
                    SubstitutionStep::Macros,
                    SubstitutionStep::SpecialCharacters,
                    SubstitutionStep::Callouts,
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
                    SubstitutionStep::Callouts,
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
                    SubstitutionStep::Callouts,
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

    mod override_via_attrlist {
        use crate::{
            attributes::{Attrlist, AttrlistContext},
            tests::prelude::*,
        };

        /// Resolve the substitution group that `base` maps to when the given
        /// attribute list (block style, `subs=`, …) is applied.
        fn resolve(base: SubstitutionGroup, attrlist: &str) -> SubstitutionGroup {
            let p = Parser::default();
            let attrlist = Attrlist::parse(crate::Span::new(attrlist), &p, AttrlistContext::Block)
                .item
                .item;

            base.override_via_attrlist(Some(&attrlist))
        }

        #[test]
        fn verbatim_masquerade_styles_promote_normal_to_verbatim() {
            // On a simple-content (paragraph) block, the `literal`, `listing`,
            // and `source` styles switch the substitution group to verbatim.
            for style in ["literal", "listing", "source"] {
                assert_eq!(
                    resolve(SubstitutionGroup::Normal, style),
                    SubstitutionGroup::Verbatim,
                    "style `{style}` should map Normal to Verbatim"
                );
            }
        }

        #[test]
        fn pass_style_suppresses_substitutions_on_normal() {
            assert_eq!(
                resolve(SubstitutionGroup::Normal, "pass"),
                SubstitutionGroup::None
            );
        }

        #[test]
        fn non_masquerade_styles_keep_normal() {
            // Styles whose content model is simple (or compound) keep the normal
            // substitution group; e.g. `verse` uses normal subs even though its
            // content model is verbatim.
            for style in ["normal", "verse", "quote", "sidebar", "example"] {
                assert_eq!(
                    resolve(SubstitutionGroup::Normal, style),
                    SubstitutionGroup::Normal,
                    "style `{style}` should keep Normal"
                );
            }
        }

        #[test]
        fn style_does_not_override_a_delimited_block_group() {
            // A delimited block's delimiter fixes its substitution group; a style
            // keyword must not override it (matching Asciidoctor). A `[pass]`
            // style on a `----`/`....` verbatim block keeps verbatim subs, and a
            // `[source]` style on a `++++` pass block keeps the pass group.
            assert_eq!(
                resolve(SubstitutionGroup::Verbatim, "pass"),
                SubstitutionGroup::Verbatim
            );

            assert_eq!(
                resolve(SubstitutionGroup::Pass, "source"),
                SubstitutionGroup::Pass
            );

            assert_eq!(
                resolve(SubstitutionGroup::Stem, "source"),
                SubstitutionGroup::Stem
            );
        }

        #[test]
        fn subs_attribute_still_overrides() {
            // An explicit `subs=` attribute overrides the group regardless of the
            // block style, and takes precedence over the style masquerade.
            assert_eq!(
                resolve(SubstitutionGroup::Normal, "listing,subs=normal"),
                SubstitutionGroup::Normal
            );

            assert_eq!(
                resolve(SubstitutionGroup::Verbatim, "subs=none"),
                SubstitutionGroup::None
            );
        }
    }

    mod normal {
        use crate::{content::Content, strings::CowStr, tests::prelude::*};

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
            content::Content, parser::ModificationContext, strings::CowStr, tests::prelude::*,
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
        use crate::{content::Content, strings::CowStr, tests::prelude::*};

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
        use crate::{content::Content, strings::CowStr, tests::prelude::*};

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
