use crate::{
    HasSpan, Parser,
    attributes::Attrlist,
    content::{Content, Passthrough, Passthroughs, SubstitutionStep},
    document::RefType,
    warnings::WarningType,
};

/// Each block and inline element has a default substitution group that is
/// applied unless you customize the substitutions for a particular element.
///
/// `SubstitutionGroup` specifies the default or overridden substitution group
/// to be applied.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
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

    pub(crate) fn apply<'src>(
        &self,
        content: &mut Content<'src>,
        parser: &Parser,
        attrlist: Option<&Attrlist<'src>>,
    ) {
        self.apply_inner(content, parser, attrlist, None);
    }

    /// [`apply`](Self::apply) for a description-list **term**, which is the one
    /// content with a registration rule of its own: a leading `[[id]]` /
    /// `[[id,reftext]]` at the very start of the term is registered with the
    /// **rest of the term** as its default reference text, so `[[cpu]]CPU::`
    /// makes `<<cpu>>` display *CPU*.
    ///
    /// That rule cannot live in
    /// [`apply_ref_side_effects`](crate::content::inline_builder): a leading
    /// anchor is only special because of where it sits in a *term*, which is a
    /// fact about the caller, not about the node. So the term passes its
    /// warnings list here, the rule runs between the tree build and the replay
    /// — the one point where the tree exists and nothing has registered from it
    /// yet — and the replay is then told the anchor is already registered, so
    /// it does not raise a second duplicate-id warning for it.
    ///
    /// Before this branch's step 6 the term ran the steps directly and
    /// registered from the string pipeline, which made it the last content
    /// doing so. It no longer is.
    pub(crate) fn apply_to_description_list_term<'src>(
        &self,
        content: &mut Content<'src>,
        parser: &Parser,
        warnings: &mut Vec<crate::warnings::Warning<'src>>,
    ) {
        self.apply_inner(content, parser, None, Some(warnings));
    }

    fn apply_inner<'src>(
        &self,
        content: &mut Content<'src>,
        parser: &Parser,
        attrlist: Option<&Attrlist<'src>>,
        term_warnings: Option<&mut Vec<crate::warnings::Warning<'src>>>,
    ) {
        // Snapshot the pre-substitution value *before* either pass runs: both
        // are seeded from it, so the authoritative one sees exactly what the
        // oracle does.
        //
        // Unconditional. Every parse builds the tree — the `with_inline_tree`
        // opt-in and the `build_inline_tree` field that outlived it are both
        // retired — and the reentrancy guard that was the last remaining reason
        // to skip one is retired here: nothing re-enters this seam during a
        // build. A passthrough carrying its own substitution list was the only
        // caller that ever did, and `passthrough_text` builds and folds its
        // body's own tree directly through `build_for_group` now. No production
        // code under `content::inline_builder` calls `SubstitutionGroup::apply`
        // at all (every such call in that module is inside a `mod tests`), and
        // the guard was observed set for 0 of the 13,299 parses the suite
        // reaches this seam with.
        let tree_seed = content.rendered.clone();

        // **The inversion** (design §5.2 Phase 4, step 6). The string pipeline
        // used to run on the real parser with its recognition side effects
        // suppressed, while the builder ran on a counter-safe clone whose
        // mutations were thrown away. The two have swapped places: the string
        // pipeline runs on the clone — so its counters, its catalog
        // registrations and its warnings are *all* discarded with it, and there
        // is nothing left to suppress — and the builder runs on the real
        // parser, where what it recognizes is simply kept.
        //
        // That is what makes `run_pipeline` a pure oracle: it computes a string
        // and writes nothing anyone reads. Deleting it — and with it the three
        // sentinel systems and the now-vestigial
        // `suppress_recognition_side_effects` window — is the next increment;
        // this one only turns the seam around, so that deletion is a deletion
        // rather than a rewrite.
        //
        // The clone is taken here, immediately before the pass it feeds, so it
        // carries every document counter (footnote and callout numbers,
        // `{counter:…}` values) at its pre-substitution value — the same value
        // the builder then advances on the real parser, exactly once.
        self.run_pipeline(content, &parser.clone(), attrlist);

        {
            let value = tree_seed;

            // Where this build's own diagnostics start — see
            // `Parser::drain_builder_diagnostics_since` for why a mark rather
            // than the whole buffer.
            let diagnostics_before_build = parser.builder_diagnostics_len();

            // And where the *substitution* warning buffer stands, so anything
            // the build records into it can be discarded below.
            //
            // The build's deliberate diagnostics go through
            // `record_builder_diagnostic`, never here. What reaches this buffer
            // during a build is incidental: an `Attrlist` parsed out of a match
            // string records its own `attribute-missing` warning at an offset
            // into that string, which is not a position in the document source
            // — `['{missing}']++x++` is the shape, and surfacing it would
            // anchor a warning nowhere. Before the inversion these were
            // discarded by being recorded onto the clone; the build holds the
            // real parser now, so the discard has to be said out loud. It is
            // the same `substitution_warnings_len`/`truncate` idiom every other
            // owned-source substitution in the crate already uses.
            let warnings_before_build = parser.substitution_warnings_len();

            let tree = crate::content::inline_builder::build_for_group(
                self,
                value,
                content.original(),
                parser,
                attrlist,
            );

            // Everything this build recorded into the substitution-warning
            // buffer is incidental (see `warnings_before_build`), and all of it
            // is discarded. There is no longer an exception: the one that stood
            // here put back what a *nested authoritative pass* had moved aside,
            // and no pass runs nested any more — that mechanism's two ends went
            // with the branch that fed it.
            parser.truncate_substitution_warnings(warnings_before_build);

            // The recognition **diagnostics** the string pipeline's copy of this
            // content raised into the discarded clone, raised again here where
            // they are kept — the warning half of "re-attach the recognition
            // side effects" (design §5.2's step 6).
            //
            // A registration has a node to hang on, so it is replayed from the
            // tree below. These five do not: `attribute-missing` drops a
            // reference and leaves nothing behind, a `link:` macro with a
            // dangerous scheme stays literal, and an invalid substitution name
            // in a `pass:`/`stem:` list is simply skipped. So they are recorded
            // where they are *recognized* and carried across here — which is
            // what `Parser::record_builder_diagnostic` and
            // `push_substitution_warnings` are for. The builder's own buffer is
            // still used rather than the parser's warning buffer, and now for
            // the only reason that was ever load-bearing: a warning the build
            // records merely *incidentally* (an `Attrlist` parse over a match
            // string) must not be swept up with them. Before the inversion that
            // separation also fell out of the buffer sitting on a clone nobody
            // read; it does not any more, so this buffer is the whole of it.
            //
            // Before `apply_macro_side_effects`, deliberately: the string
            // pipeline raised these during its own pass, ahead of the
            // registrations the replay performs, and that relative order is
            // what `inline_builder_side_effect_parity` compares.
            parser.push_substitution_warnings(
                parser.drain_builder_diagnostics_since(diagnostics_before_build),
            );

            // The deferred cross-references are the **tree's**, not the string
            // pipeline's — design §5.2's survey item, wired. The two staged
            // walks read them off the tree already partitioned into the
            // block-level ones and the ones this content's footnotes carry,
            // where the string pipeline produced one flat list that had to be
            // split by asking which of its placeholders survived. What is kept
            // from the pipeline's own answer is the placeholder template, and
            // only for the one content that renders from one — see
            // `Content::set_tree_xrefs`.
            //
            // The fold below reads `content.deferred_parts()` for nothing, so
            // the order of these two is a matter of reading rather than of
            // correctness; deriving first keeps `rendered` and `deferred`
            // describing the same tree at every point in this function.
            let render_context = parser.render_context();

            content.set_tree_xrefs(&tree, &*parser.renderer, &render_context);

            // The tree is **authoritative** for the rendered string: what
            // `rendered_html()` returns is a fold of it, not the string
            // pipeline's own output (design §5.2 Phase 4, step 6).
            //
            // Unconditional now, including for content carrying a deferred
            // cross-reference. Such a content's rendering is taken *again* at
            // the end of resolution (`Content::refold`), once the destinations
            // are known; what it holds until then is the fold of an unresolved
            // tree, which is the same unresolved-fallback answer the template
            // gave and is the honest one for a document that has not settled
            // its references yet.
            let folded = crate::content::inline_builder::fold_html(
                &tree,
                &*parser.renderer,
                &render_context,
            );

            content.rendered = crate::strings::CowStr::from(folded);

            // The recognition side effects the string pipeline just skipped,
            // replayed from the tree — design §5.2's step 6, "re-attach the
            // recognition side effects". They run here, after the whole
            // pipeline rather than during its macros step, which keeps them in
            // document order across contents and in the string pipeline's own
            // pass order within one (see `apply_macro_side_effects`).
            //
            // A description-list term's own leading-anchor rule runs first,
            // between the build and the replay — see
            // `apply_to_description_list_term`. Every other content passes
            // `false`: it has no anchor registered ahead of the replay.
            let leading_anchor_registered = term_warnings.is_some_and(|warnings| {
                Self::register_term_leading_anchor(&tree, parser, content, warnings)
            });

            crate::content::inline_builder::apply_macro_side_effects(
                &tree,
                parser,
                content.original(),
                leading_anchor_registered,
            );

            // The callouts step's own registration, replayed the same way. It
            // is not a macro family — callouts are recognized in verbatim
            // content, where the macros step does not run at all — so it is a
            // sibling call rather than part of the composition above.
            crate::content::inline_builder::apply_callout_side_effects(&tree, parser);

            // `Content::passthroughs()` is a **view over the tree**, the last
            // of design §5.2's six things `run_pipeline` solely owned. The
            // extraction pass still builds its own list — the restore pass
            // indexes into it by sentinel — but that list is now private to
            // this one pipeline run, and what a caller observes is read back
            // off the tree in document order. See `Passthrough::from_tree`.
            content.set_passthroughs(Passthrough::from_tree(&tree));

            content.set_inlines(tree);

            // Content carrying a deferred cross-reference is the only content
            // whose rendering is rebuilt after the parse, so it is the only
            // content that is *folded* after the parse — and a fold needs the
            // document attributes this content was written under, which by then
            // the parse has moved on from. Retain them here, where "now" is
            // still that point in the document. `Content::refold` reads them
            // back.
            if content.deferred_parts().is_some() {
                content.set_render_attributes(parser.snapshot_attributes());
            }
        }
    }

    /// Registers a description-list term's leading inline anchor, from the
    /// term's own tree, and answers whether it did.
    ///
    /// The rule mirrors what the term used to read out of its half-substituted
    /// string with a regex: the anchor must be the term's **first** node and
    /// must start at the term's own first byte, and its reference text is its
    /// own `[[id,reftext]]` text when it has one, or else the **rest of the
    /// term**, trimmed.
    ///
    /// Reading "the rest of the term" from the tree rather than from that
    /// string is one deliberate difference. The regex ran *before* the macros
    /// step, so a term whose remainder held a macro registered the macro's
    /// **source** as its reference text (`[[x]]image:a.png[]Term` registered
    /// `image:a.png[]Term`); the fold of the same nodes gives the rendering
    /// instead, which is what every other reference text on this branch is.
    ///
    /// Being the tree's **first** node is the whole of "at the start of the
    /// term": that is what the regex's `^` anchor said, and the two agree
    /// because a term's own source begins at its first non-space character.
    /// There is deliberately no second test of the node's byte offset, and no
    /// [`is_bibliography`](crate::inlines::Anchor) check either — a
    /// bibliography anchor registers under its own
    /// [`RefType`] from its own earlier pass, but it cannot lead a term in the
    /// first place, since a bibliography list item's principal text is never
    /// parsed as one. Both would be branches no input can take.
    fn register_term_leading_anchor<'src>(
        tree: &[crate::inlines::InlineNode<'src>],
        parser: &Parser,
        content: &Content<'src>,
        warnings: &mut Vec<crate::warnings::Warning<'src>>,
    ) -> bool {
        use crate::inlines::InlineNode;

        let Some(InlineNode::Anchor(anchor)) = tree.first() else {
            return false;
        };

        let reftext = match &anchor.reftext {
            Some(reftext) => Some(crate::content::inline_builder::fold_html(
                reftext,
                parser.renderer.as_ref(),
                &parser.render_context(),
            )),

            None => {
                let rest = crate::content::inline_builder::fold_html(
                    tree.get(1..).unwrap_or_default(),
                    parser.renderer.as_ref(),
                    &parser.render_context(),
                );

                Some(rest.trim().to_string())
            }
        };

        if parser
            .register_ref(&anchor.id, reftext.as_deref(), RefType::Anchor)
            .is_err()
        {
            warnings.push(crate::warnings::Warning::new(
                content.original(),
                WarningType::DuplicateId(anchor.id.to_string()),
            ));
        }

        true
    }

    /// Runs **only** the string pipeline over `content` — no inline tree, no
    /// fold — which is the golden-HTML oracle (§5.3) as a callable.
    ///
    /// Every differential corpus on this branch takes its golden by rendering a
    /// fixture through the string pipeline and comparing it against the tree's
    /// fold. Taking that golden from [`apply`](Self::apply) works only while
    /// `rendered` *is* the string pipeline's output. The step 6 cutover makes
    /// `rendered` a fold of the tree, at which point such a corpus compares the
    /// fold against itself and passes for that reason, with nothing failing to
    /// say so — see
    /// [`snapshot`](crate::content::inline_builder) for the demonstration.
    ///
    /// So the corpora take their golden from here instead, and go on
    /// differentiating for real. It was landed one increment *before* the
    /// cutover, while it was still byte-identical to `apply` — the tree being
    /// additive then, the only difference was work the golden never reads — so
    /// that the rewiring could be checked by the whole suite staying green,
    /// which is a claim the cutover itself could no longer make. As of this
    /// increment the two genuinely differ, and this is the only remaining way
    /// to reach the string pipeline's own output.
    ///
    /// The ~277 golden-HTML assertions deliberately do **not** take it: their
    /// subject is `rendered_html()` itself, so they must go on exercising the
    /// production entry point, and after the cutover they are precisely what
    /// validates the fold.
    #[cfg(test)]
    pub(crate) fn apply_string_pipeline<'src>(
        &self,
        content: &mut Content<'src>,
        parser: &Parser,
        attrlist: Option<&Attrlist<'src>>,
    ) {
        self.run_pipeline(content, parser, attrlist);
    }

    /// Runs the substitution pipeline for this group over `content`: extract
    /// passthroughs (when the group includes them), apply each step in order,
    /// restore the passthroughs, and finalize any deferred cross-references.
    fn run_pipeline(
        &self,
        content: &mut Content<'_>,
        parser: &Parser,
        attrlist: Option<&Attrlist>,
    ) {
        // The steps below mark their work **in band**, with sentinel codepoints
        // spliced into the same string as the document's text. A document can
        // type those codepoints itself, so its own copies are escaped out of
        // the way first; otherwise they are read back as the parser's own
        // control sequences (forging, for instance, a second cross-reference
        // into the output).
        //
        // The escaping is this pipeline's alone. The single-pass builder needs
        // none: it recognizes constructs by *range* over the source rather than
        // by scanning a rendered string for its own marks, so a codepoint the
        // document typed is never a mark. What escaped form still reaches past
        // this call is the deferred placeholder template and, where the
        // carve-out keeps them, this pipeline's own cross-reference segments —
        // each unescaped by the reader that makes it user-facing (see
        // `Content::resolve_references` and `catalog_target`).
        content.escape_sentinels();

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
        // render the unresolved fallback, so `rendered_html()` is clean even before
        // references are resolved. This is a no-op when no cross-references were
        // found.
        content.finalize_deferred(&*parser.renderer);

        // Hand back the document's own text: the sentinel codepoints escaped on
        // the way in are restored now that every pass that reads them has run.
        // The template `finalize_deferred` just captured stays escaped — it is
        // an internal representation, re-rendered each time references are
        // resolved.
        //
        // Gated, because that re-rendering is the *other* way out of escaped
        // form: a content that deferred anything has already had `rendered`
        // rebuilt by `finalize_deferred`, through a `render_template` that
        // leaves escaped form run by run (so the resolver's own answer is never
        // decoded with it — see `Content::render_template`). Decoding that
        // result again would read one of the document's own restored escapes a
        // second time.
        if content.deferred_parts().is_none() {
            content.unescape_sentinels();
        }
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

    /// Returns the ordered list of substitution [steps](SubstitutionStep) this
    /// group applies, in the order they run.
    ///
    /// This is the resolved, expanded form of the group: a named group (e.g.
    /// [`Normal`](Self::Normal) or [`Verbatim`](Self::Verbatim)) expands to its
    /// fixed step sequence, and a [`Custom`](Self::Custom) group returns its
    /// own steps. Useful for inspecting the substitutions in effect for a
    /// block or an extracted [`Passthrough`].
    pub fn steps(&self) -> &[SubstitutionStep] {
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

    mod sentinel_escaping {
        use super::super::SubstitutionGroup;
        use crate::{Parser, Span, content::Content};

        #[test]
        fn a_deferred_content_leaves_escaped_form_exactly_once() {
            // The string pipeline has two ways out of escaped form, and a
            // content that defers a cross-reference takes the *first*:
            // `finalize_deferred` rebuilds `rendered` through `render_template`,
            // which leaves escaped form run by run. Decoding the result again at
            // the end of the pipeline would read the document's own restored
            // escape introducer a second time — here turning `\u{e004}b` into
            // `\u{e001}` — so the tail decode is gated on there being nothing
            // deferred.
            //
            // Driven through `apply_string_pipeline` because that pipeline's
            // output is the differential corpora's oracle rather than the
            // production rendering, which is a fold of the tree.
            let mut content = Content::from(Span::new("x\u{e004}by <<a>>"));

            SubstitutionGroup::Normal.apply_string_pipeline(&mut content, &Parser::default(), None);

            assert_eq!(
                content.rendered_html(),
                "x\u{e004}by <a href=\"#a\">[a]</a>",
                "the typed escape introducer was decoded twice"
            );
        }

        #[test]
        fn a_content_with_nothing_deferred_still_leaves_escaped_form() {
            // The complement: no template was rebuilt, so the tail decode is
            // the only one and must run.
            let mut content = Content::from(Span::new("x\u{e004}by"));

            SubstitutionGroup::Normal.apply_string_pipeline(&mut content, &Parser::default(), None);

            assert_eq!(content.rendered_html(), "x\u{e004}by");
        }
    }

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

            let block = doc.child_blocks().next().unwrap();
            assert_eq!(
                block.rendered_html_content(),
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

            let block = doc.child_blocks().next().unwrap();
            assert_eq!(block.rendered_html_content(), Some("content <here>"));

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

            // Title should produce exactly the same result as Normal.
            assert_eq!(title_content.rendered, normal_content.rendered);
        }
    }
}
