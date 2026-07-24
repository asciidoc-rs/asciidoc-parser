use crate::{
    HasSpan, Parser,
    attributes::Attrlist,
    content::{Content, Passthroughs, SubstitutionStep},
    warnings::WarningType,
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

/// The substitution steps applied by the normal substitution group.
const NORMAL_STEPS: &[SubstitutionStep] = &[
    SubstitutionStep::SpecialCharacters,
    SubstitutionStep::Quotes,
    SubstitutionStep::AttributeReferences,
    SubstitutionStep::CharacterReplacements,
    SubstitutionStep::Macros,
    SubstitutionStep::PostReplacement,
];

/// The substitution steps applied by the verbatim substitution group.
const VERBATIM_STEPS: &[SubstitutionStep] = &[
    SubstitutionStep::SpecialCharacters,
    SubstitutionStep::Callouts,
];

impl SubstitutionGroup {
    /// Parse the custom substitution group syntax defined in [Custom
    /// substitutions].
    ///
    /// Returns the resolved substitution group and the list of substitution
    /// names that were not recognized. Mirroring Asciidoctor's `resolve_subs`,
    /// an unrecognized name is skipped rather than invalidating the whole
    /// list; callers that can record warnings should report the returned
    /// invalid names.
    ///
    /// [Custom substitutions]: https://docs.asciidoctor.org/asciidoc/latest/pass/pass-macro/#custom-substitutions
    pub(crate) fn from_custom_string(
        start_from: Option<&Self>,
        mut custom: &str,
    ) -> (Self, Vec<String>) {
        custom = custom.trim();

        if custom == "none" {
            return (Self::None, vec![]);
        }

        if custom == "n" || custom == "normal" {
            return (Self::Normal, vec![]);
        }

        if custom == "v" || custom == "verbatim" {
            return (Self::Verbatim, vec![]);
        }

        let mut tokens: Vec<&str> = custom.split(',').map(str::trim).collect();

        // Ruby's `split(',')` drops trailing empty entries, so an empty string
        // or a list of only separators (e.g. `subs=","`) yields no tokens at
        // all and resolves to an empty substitution list, matching
        // Asciidoctor's `resolve_subs` (which returns no subs without warning
        // in that case).
        while tokens.last() == Some(&"") {
            tokens.pop();
        }

        let mut steps: Vec<SubstitutionStep> = vec![];
        let mut invalid: Vec<String> = vec![];
        let mut first = true;

        for mut step in tokens {
            let is_first = first;
            first = false;

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

            // Each name resolves to a list of steps, so a group name mid-list
            // contributes its constituent steps like any other token. This
            // matches Asciidoctor's `resolve_subs`, where every key resolves
            // to an array of substitutions before the modifier is applied.
            let resolved: &[SubstitutionStep] = match step {
                "none" => &[],
                "n" | "normal" => NORMAL_STEPS,
                "v" | "verbatim" => VERBATIM_STEPS,
                "c" | "specialcharacters" | "specialchars" => {
                    &[SubstitutionStep::SpecialCharacters]
                }
                "q" | "quotes" => &[SubstitutionStep::Quotes],
                "a" | "attributes" => &[SubstitutionStep::AttributeReferences],
                "r" | "replacements" => &[SubstitutionStep::CharacterReplacements],
                "m" | "macros" => &[SubstitutionStep::Macros],
                "p" | "post_replacements" => &[SubstitutionStep::PostReplacement],
                "callouts" => &[SubstitutionStep::Callouts],
                _ => {
                    // Removing an unrecognized name is a no-op rather than an
                    // error: in Asciidoctor's `resolve_subs`, a `-` modifier
                    // never adds the name to the candidate list, so it is
                    // never reported as invalid.
                    if !subtract {
                        invalid.push(step.to_owned());
                    }

                    continue;
                }
            };

            if prepend {
                for (index, step) in resolved.iter().enumerate() {
                    steps.insert(index, *step);
                }
            } else if subtract {
                steps.retain(|s| !resolved.contains(s));
            } else {
                steps.extend_from_slice(resolved);
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

        (Self::Custom(deduped), invalid)
    }

    pub(crate) fn apply(
        &self,
        content: &mut Content<'_>,
        parser: &Parser,
        attrlist: Option<&Attrlist>,
    ) {
        // Capture the inline AST first, from the pre-substitution state, when
        // the parser opts in and we are not already inside a capture pass. The
        // guard flag stays set for the rest of this `apply` so nested
        // substitutions (passthrough restore, table cells) do not recursively
        // capture. See `capture_inline_nodes`.
        let capturing = parser.capture_inline_ast && !parser.inline_ast_capturing.get();

        let captured = if capturing {
            parser.inline_ast_capturing.set(true);
            Some(crate::content::capture_inline_nodes(
                content, self, parser, attrlist,
            ))
        } else {
            None
        };

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

        if capturing {
            parser.inline_ast_capturing.set(false);
        }

        if let Some(nodes) = captured {
            content.set_inline_nodes(nodes);
        }
    }

    /// Produces the semantic inline AST (issue #892) for `span` under this
    /// substitution group, instead of a rendered string.
    ///
    /// This runs the same ordered substitution pipeline as the rendering path
    /// but with a recording renderer, then parses the result into a tree of
    /// [`InlineNode`](crate::content::InlineNode)s. It is additive and does not
    /// affect the rendered output
    /// ([`Content::rendered`](crate::content::Content::rendered)).
    /// Cross-references are captured but not resolved (there is no document
    /// context here); for resolved cross-references, enable
    /// [`Parser::with_inline_ast_capture`](crate::Parser::with_inline_ast_capture)
    /// and read
    /// [`Content::inline_nodes`](crate::content::Content::inline_nodes) after
    /// parsing.
    pub fn inline_nodes(
        &self,
        span: crate::Span<'_>,
        parser: &Parser,
        attrlist: Option<&Attrlist>,
    ) -> Vec<crate::content::InlineNode> {
        crate::content::capture_inline_nodes(&Content::from(span), self, parser, attrlist)
    }

    /// Applies any block style masquerade and `subs` attribute override from
    /// the block's attribute list.
    ///
    /// When `parser` is provided, unrecognized substitution names in the
    /// `subs` attribute are recorded as warnings. Parse-time callers should
    /// pass the parser; accessors that re-derive the group after parsing
    /// should pass `None` so the warning is only recorded once.
    pub(crate) fn override_via_attrlist(
        &self,
        attrlist: Option<&Attrlist>,
        parser: Option<&Parser>,
    ) -> Self {
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

            if let Some(subs) = attrlist.named_attribute("subs").map(|attr| attr.value()) {
                let (sub_group, invalid) = Self::from_custom_string(Some(self), subs);

                if !invalid.is_empty()
                    && let Some(parser) = parser
                {
                    parser.record_substitution_warning(
                        attrlist.span(),
                        WarningType::InvalidSubstitutionTypeForBlock(invalid.join(", ")),
                    );
                }

                result = sub_group;
            }
        }

        result
    }

    fn steps(&self) -> &[SubstitutionStep] {
        match self {
            Self::Normal | Self::Title => NORMAL_STEPS,

            Self::Header | Self::AttributeEntryValue => &[
                SubstitutionStep::SpecialCharacters,
                SubstitutionStep::AttributeReferences,
            ],

            Self::Verbatim => VERBATIM_STEPS,

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
            // An empty `subs` value resolves to an empty substitution list,
            // matching Asciidoctor's `resolve_subs` (which returns no subs for
            // a nil or empty string).
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, ""),
                (SubstitutionGroup::Custom(vec![]), vec![])
            );
        }

        #[test]
        fn empty_entries() {
            // A list containing only separators resolves to an empty
            // substitution list, without reporting any invalid names: Ruby's
            // `split(',')` drops trailing empty entries, so `","` yields no
            // tokens at all (issue #784).
            assert_eq!(
                SubstitutionGroup::from_custom_string(Some(&SubstitutionGroup::Verbatim), ","),
                (SubstitutionGroup::Custom(vec![]), vec![])
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, " , ,"),
                (SubstitutionGroup::Custom(vec![]), vec![])
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "quotes,"),
                (
                    SubstitutionGroup::Custom(vec![SubstitutionStep::Quotes]),
                    vec![]
                )
            );

            // A leading or interior empty entry is not dropped by Ruby's
            // `split(',')`; it resolves like any other unrecognized name, so
            // it is skipped and reported (with an empty name), matching
            // Asciidoctor's `resolve_subs`.
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "quotes,,macros"),
                (
                    SubstitutionGroup::Custom(vec![
                        SubstitutionStep::Quotes,
                        SubstitutionStep::Macros
                    ]),
                    vec!["".to_owned()]
                )
            );

            // A leading empty entry counts as a (failed) first token, so a
            // modifier that follows it does not start from the base group,
            // matching Asciidoctor (where the empty entry makes the candidate
            // list non-nil before the modifier is seen).
            assert_eq!(
                SubstitutionGroup::from_custom_string(
                    Some(&SubstitutionGroup::Verbatim),
                    ",+quotes"
                ),
                (
                    SubstitutionGroup::Custom(vec![SubstitutionStep::Quotes]),
                    vec!["".to_owned()]
                )
            );
        }

        #[test]
        fn invalid_names() {
            // An unrecognized name is skipped and reported; recognized names
            // in the same list are still honored.
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "bogus"),
                (SubstitutionGroup::Custom(vec![]), vec!["bogus".to_owned()])
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "bogus,quotes"),
                (
                    SubstitutionGroup::Custom(vec![SubstitutionStep::Quotes]),
                    vec!["bogus".to_owned()]
                )
            );

            // An appended unrecognized name still seeds the list from the
            // base group before being skipped, matching Asciidoctor.
            assert_eq!(
                SubstitutionGroup::from_custom_string(
                    Some(&SubstitutionGroup::Verbatim),
                    "+bogus,quotes"
                ),
                (
                    SubstitutionGroup::Custom(vec![
                        SubstitutionStep::SpecialCharacters,
                        SubstitutionStep::Callouts,
                        SubstitutionStep::Quotes,
                    ]),
                    vec!["bogus".to_owned()]
                )
            );

            // Removing an unrecognized name is a no-op, not an error: in
            // Asciidoctor's `resolve_subs`, a `-` modifier never adds the
            // name to the candidate list, so it is never reported as invalid.
            assert_eq!(
                SubstitutionGroup::from_custom_string(Some(&SubstitutionGroup::Verbatim), "-bogus"),
                (
                    SubstitutionGroup::Custom(vec![
                        SubstitutionStep::SpecialCharacters,
                        SubstitutionStep::Callouts,
                    ]),
                    vec![]
                )
            );
        }

        #[test]
        fn none() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "none"),
                (SubstitutionGroup::None, vec![])
            );

            // `none` mid-list resolves to an empty step list, like
            // Asciidoctor's `SUB_GROUPS[:none]`.
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "quotes,none"),
                (
                    SubstitutionGroup::Custom(vec![SubstitutionStep::Quotes]),
                    vec![]
                )
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "nermal"),
                (SubstitutionGroup::Custom(vec![]), vec!["nermal".to_owned()])
            );
        }

        #[test]
        fn normal() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "n"),
                (SubstitutionGroup::Normal, vec![])
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "normal"),
                (SubstitutionGroup::Normal, vec![])
            );
        }

        #[test]
        fn verbatim() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "v"),
                (SubstitutionGroup::Verbatim, vec![])
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "verbatim"),
                (SubstitutionGroup::Verbatim, vec![])
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "verboten"),
                (
                    SubstitutionGroup::Custom(vec![]),
                    vec!["verboten".to_owned()]
                )
            );
        }

        #[test]
        fn special_chars() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "c"),
                (
                    SubstitutionGroup::Custom(vec![SubstitutionStep::SpecialCharacters]),
                    vec![]
                )
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "specialchars"),
                (
                    SubstitutionGroup::Custom(vec![SubstitutionStep::SpecialCharacters]),
                    vec![]
                )
            );
        }

        #[test]
        fn quotes() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "q"),
                (
                    SubstitutionGroup::Custom(vec![SubstitutionStep::Quotes]),
                    vec![]
                )
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "quotes"),
                (
                    SubstitutionGroup::Custom(vec![SubstitutionStep::Quotes]),
                    vec![]
                )
            );
        }

        #[test]
        fn attributes() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "a"),
                (
                    SubstitutionGroup::Custom(vec![SubstitutionStep::AttributeReferences]),
                    vec![]
                )
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "attributes"),
                (
                    SubstitutionGroup::Custom(vec![SubstitutionStep::AttributeReferences]),
                    vec![]
                )
            );
        }

        #[test]
        fn replacements() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "r"),
                (
                    SubstitutionGroup::Custom(vec![SubstitutionStep::CharacterReplacements]),
                    vec![]
                )
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "replacements"),
                (
                    SubstitutionGroup::Custom(vec![SubstitutionStep::CharacterReplacements]),
                    vec![]
                )
            );
        }

        #[test]
        fn macros() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "m"),
                (
                    SubstitutionGroup::Custom(vec![SubstitutionStep::Macros]),
                    vec![]
                )
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "macros"),
                (
                    SubstitutionGroup::Custom(vec![SubstitutionStep::Macros]),
                    vec![]
                )
            );
        }

        #[test]
        fn post_replacements() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "p"),
                (
                    SubstitutionGroup::Custom(vec![SubstitutionStep::PostReplacement]),
                    vec![]
                )
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "post_replacements"),
                (
                    SubstitutionGroup::Custom(vec![SubstitutionStep::PostReplacement]),
                    vec![]
                )
            );
        }

        #[test]
        fn multiple() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "q,a"),
                (
                    SubstitutionGroup::Custom(vec![
                        SubstitutionStep::Quotes,
                        SubstitutionStep::AttributeReferences
                    ]),
                    vec![]
                )
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "q, a"),
                (
                    SubstitutionGroup::Custom(vec![
                        SubstitutionStep::Quotes,
                        SubstitutionStep::AttributeReferences
                    ]),
                    vec![]
                )
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "quotes,attributes"),
                (
                    SubstitutionGroup::Custom(vec![
                        SubstitutionStep::Quotes,
                        SubstitutionStep::AttributeReferences
                    ]),
                    vec![]
                )
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "x,bogus,no such step"),
                (
                    SubstitutionGroup::Custom(vec![]),
                    vec![
                        "x".to_owned(),
                        "bogus".to_owned(),
                        "no such step".to_owned()
                    ]
                )
            );
        }

        #[test]
        fn subtraction() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "n,-r"),
                (
                    SubstitutionGroup::Custom(vec![
                        SubstitutionStep::SpecialCharacters,
                        SubstitutionStep::Quotes,
                        SubstitutionStep::AttributeReferences,
                        SubstitutionStep::Macros,
                        SubstitutionStep::PostReplacement,
                    ]),
                    vec![]
                )
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "n,-r,-r,-m"),
                (
                    SubstitutionGroup::Custom(vec![
                        SubstitutionStep::SpecialCharacters,
                        SubstitutionStep::Quotes,
                        SubstitutionStep::AttributeReferences,
                        SubstitutionStep::PostReplacement,
                    ]),
                    vec![]
                )
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "v,-r"),
                (
                    SubstitutionGroup::Custom(vec![
                        SubstitutionStep::SpecialCharacters,
                        SubstitutionStep::Callouts,
                    ]),
                    vec![]
                )
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "v,-c"),
                (
                    SubstitutionGroup::Custom(vec![SubstitutionStep::Callouts]),
                    vec![]
                )
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "v,-callouts"),
                (
                    SubstitutionGroup::Custom(vec![SubstitutionStep::SpecialCharacters,]),
                    vec![]
                )
            );
        }

        #[test]
        fn addition() {
            // `n` expands to normal's steps (which already include
            // replacements); the trailing `r` is de-duplicated away, matching
            // Asciidoctor.
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "n,r"),
                (
                    SubstitutionGroup::Custom(vec![
                        SubstitutionStep::SpecialCharacters,
                        SubstitutionStep::Quotes,
                        SubstitutionStep::AttributeReferences,
                        SubstitutionStep::CharacterReplacements,
                        SubstitutionStep::Macros,
                        SubstitutionStep::PostReplacement,
                    ]),
                    vec![]
                )
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "v,m"),
                (
                    SubstitutionGroup::Custom(vec![
                        SubstitutionStep::SpecialCharacters,
                        SubstitutionStep::Callouts,
                        SubstitutionStep::Macros,
                    ]),
                    vec![]
                )
            );
        }

        #[test]
        fn incremental() {
            // `n` expands to normal's steps (which already include
            // replacements); the trailing `r` is de-duplicated away, matching
            // Asciidoctor.
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "n,r"),
                (
                    SubstitutionGroup::Custom(vec![
                        SubstitutionStep::SpecialCharacters,
                        SubstitutionStep::Quotes,
                        SubstitutionStep::AttributeReferences,
                        SubstitutionStep::CharacterReplacements,
                        SubstitutionStep::Macros,
                        SubstitutionStep::PostReplacement,
                    ]),
                    vec![]
                )
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "v,m"),
                (
                    SubstitutionGroup::Custom(vec![
                        SubstitutionStep::SpecialCharacters,
                        SubstitutionStep::Callouts,
                        SubstitutionStep::Macros,
                    ]),
                    vec![]
                )
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
                (
                    SubstitutionGroup::Custom(vec![
                        SubstitutionStep::Quotes,
                        SubstitutionStep::SpecialCharacters,
                        SubstitutionStep::AttributeReferences,
                        SubstitutionStep::CharacterReplacements,
                        SubstitutionStep::Macros,
                        SubstitutionStep::PostReplacement,
                    ]),
                    vec![]
                )
            );

            // Same behavior for the shorthand `v` group name mid-list.
            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "m,v"),
                (
                    SubstitutionGroup::Custom(vec![
                        SubstitutionStep::Macros,
                        SubstitutionStep::SpecialCharacters,
                        SubstitutionStep::Callouts,
                    ]),
                    vec![]
                )
            );
        }

        #[test]
        fn prepend() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(
                    Some(&SubstitutionGroup::Verbatim),
                    "attributes+"
                ),
                (
                    SubstitutionGroup::Custom(vec![
                        SubstitutionStep::AttributeReferences,
                        SubstitutionStep::SpecialCharacters,
                        SubstitutionStep::Callouts,
                    ]),
                    vec![]
                )
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "attributes+"),
                (
                    SubstitutionGroup::Custom(vec![SubstitutionStep::AttributeReferences,]),
                    vec![]
                )
            );
        }

        #[test]
        fn append() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(
                    Some(&SubstitutionGroup::Verbatim),
                    "+attributes"
                ),
                (
                    SubstitutionGroup::Custom(vec![
                        SubstitutionStep::SpecialCharacters,
                        SubstitutionStep::Callouts,
                        SubstitutionStep::AttributeReferences,
                    ]),
                    vec![]
                )
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "attributes+"),
                (
                    SubstitutionGroup::Custom(vec![SubstitutionStep::AttributeReferences,]),
                    vec![]
                )
            );
        }

        #[test]
        fn subtract() {
            assert_eq!(
                SubstitutionGroup::from_custom_string(
                    Some(&SubstitutionGroup::Normal),
                    "-attributes"
                ),
                (
                    SubstitutionGroup::Custom(vec![
                        SubstitutionStep::SpecialCharacters,
                        SubstitutionStep::Quotes,
                        SubstitutionStep::CharacterReplacements,
                        SubstitutionStep::Macros,
                        SubstitutionStep::PostReplacement,
                    ]),
                    vec![]
                )
            );

            assert_eq!(
                SubstitutionGroup::from_custom_string(None, "-attributes"),
                (SubstitutionGroup::Custom(vec![]), vec![])
            );
        }

        #[test]
        fn custom_group_with_macros_preserves_passthroughs() {
            let custom_group = SubstitutionGroup::from_custom_string(None, "q,m").0;

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

            base.override_via_attrlist(Some(&attrlist), None)
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

        #[test]
        fn warns_on_invalid_subs_name_and_honors_valid_names() {
            // Verified against Asciidoctor 2.0.26: the unrecognized name is
            // warned about and skipped, while the recognized `quotes` sub is
            // still applied. The `&` is left unescaped because
            // `specialchars` is not in the resolved list.
            let mut p = Parser::default();
            let doc = p.parse("[subs=\"bogus,quotes\"]\nabc *bold* &\ndef");

            let block = doc.nested_blocks().next().unwrap();
            assert_eq!(
                block.rendered_content(),
                Some("abc <strong>bold</strong> &\ndef")
            );

            let warnings: Vec<_> = doc.warnings().collect();
            assert_eq!(warnings.len(), 1);
            assert_eq!(
                warnings.first().unwrap().warning,
                crate::warnings::WarningType::InvalidSubstitutionTypeForBlock("bogus".to_owned())
            );
        }

        #[test]
        fn no_warning_for_empty_subs_list() {
            // An empty list (`subs=","`) resolves to no substitutions without
            // any warning, matching Asciidoctor (issue #784).
            let mut p = Parser::default();
            let doc = p.parse("[subs=\",\"]\n....\ncontent <here>\n....");

            let block = doc.nested_blocks().next().unwrap();
            assert_eq!(block.rendered_content(), Some("content <here>"));

            assert_eq!(doc.warnings().count(), 0);
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
