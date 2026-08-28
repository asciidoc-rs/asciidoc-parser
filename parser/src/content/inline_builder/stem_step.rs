//! The inline-STEM extraction step.

use regex::Captures;

use super::{
    macros::{MacroMatch, MacroMatchKind, rebuild_macro_level},
    passthrough_step::passthrough_text,
    quotes::{Piece, build_match_string, emit_range, source_slice},
    special_chars::Masked,
};
use crate::{
    Parser, Span,
    content::{INLINE_STEM_MACRO, SubstitutionGroup, SubstitutionStep, stem_notation},
    inlines::{InlineNode, Stem, StemNotation},
    parser::QuoteType,
    strings::CowStr,
};

/// Resolves the [`SubstitutionGroup`] a STEM macro's expression runs
/// through: [`SubstitutionGroup::Stem`] (special characters only) for a bare
/// macro, or the group an explicit substitution list (`stem:c,q[…]`)
/// resolves to — mirroring [`SubstitutionGroup::from_custom_string`]/
/// [`InlineStemMacroReplacer`](crate::content::passthroughs)'s own
/// resolution exactly, including its "skip and keep going" handling of an
/// unrecognized name. As throughout this module, this does *not* raise the
/// string pipeline's own `InvalidSubstitutionTypeForStemMacro` warning for
/// an invalid name, deferring that side effect to the cutover (design §5.2
/// Phase 4, step 6), since it does not change the fold's output bytes.
fn resolve_stem_subs(
    subs_list: Option<&str>,
    root: Span<'_>,
    parser: &Parser,
) -> SubstitutionGroup {
    match subs_list {
        None => SubstitutionGroup::Stem,

        Some(subs_list) => {
            let (group, invalid) = SubstitutionGroup::from_custom_string(None, subs_list);

            // Reported exactly where `InlineStemMacroReplacer` reports it, and
            // recorded rather than replayed for the same reason its `pass:`
            // sibling is: an invalid name is skipped, so the node it produces
            // carries no trace of one.
            if !invalid.is_empty() {
                parser.record_builder_diagnostic(
                    root,
                    crate::warnings::WarningType::InvalidSubstitutionTypeForStemMacro(
                        invalid.join(", "),
                    ),
                );
            }

            group
        }
    }
}

/// Reports whether `subs` is safe to apply independently to each `Text` run
/// around an embedded, already-extracted [`Raw`](InlineNode::Raw) passthrough
/// — the splicing [`stem_expression_value`] does when the expression embeds
/// one.
///
/// [`SubstitutionStep::SpecialCharacters`] (the *only* step
/// [`SubstitutionGroup::Stem`] — a bare macro's default — ever runs) escapes
/// each character in isolation, so running it per-fragment and splicing the
/// `Raw` back in unprocessed reproduces exactly what running it once over the
/// whole expression (with the `Raw`'s content protected) would. Every other
/// step this crate's steps can resolve to needs more than one character of
/// context to recognize its construct — a quote pair, a `{name}` reference, a
/// `--`/arrow replacement, or a macro's own delimiters — so a construct whose
/// halves fall on either side of the `Raw` (`stem:q[*a +++x+++ b*]`) would
/// escape recognition when matched against each fragment separately, even
/// though the string pipeline (which substitutes the whole expression as one
/// string, the `Raw` content merely *protected* rather than *absent*) finds
/// it. An empty step list (`SubstitutionGroup::None`, or an explicit list
/// naming only unrecognized steps) is trivially safe too: with nothing to
/// match, per-fragment and whole-string substitution agree by construction.
fn subs_are_local(subs: &SubstitutionGroup) -> bool {
    subs.steps()
        .iter()
        .all(|step| *step == SubstitutionStep::SpecialCharacters)
}

/// Recognizes inline STEM macros (`stem:[…]`, `asciimath:[…]`,
/// `latexmath:[…]`), replacing each with a [`Stem`](InlineNode::Stem) leaf.
///
/// STEM is an **implicit passthrough**: `Passthroughs::extract_from`
/// extracts it last, after both passthrough-macro passes, *specifically so
/// that a passthrough placeholder nested inside a STEM expression survives
/// and is recursively restored* (its own doc comment). [`apply_stem`] mirrors
/// that ordering — [`build`](super::build) runs it immediately after
/// [`apply_passthroughs`](super::passthrough_step::apply_passthroughs) and
/// ahead of every other step — so a STEM expression's content is never
/// touched by specialcharacters, quotes, replacements, or macros, exactly
/// like a `+++…+++`/`++…++`/`$$…$$`/`pass:[…]` passthrough. It reuses the
/// string pipeline's *exact* recognition — [`INLINE_STEM_MACRO`] is now
/// shared `pub(crate)`, alongside the [`stem_notation`] helper that resolves
/// a bare `stem:[…]` macro's notation from the `stem` document attribute —
/// so only the recognition *sink* differs (§4.1).
///
/// Because it runs immediately after `apply_passthroughs`, the *only* node
/// kinds `apply_stem` can ever see are `Text` and [`Raw`](InlineNode::Raw)
/// (a nested, already-extracted passthrough) — no other step has run yet to
/// build anything else. So, unlike every other macro family in this module,
/// a STEM match is **never deferred** for crossing an already-recognized
/// construct: [`build_stem_node`] embeds a crossed `Raw` node directly (see
/// its doc comment), which is what makes the extraction ordering above
/// actually pay off, rather than merely being followed.
///
/// A recognized macro's expression is unescaped (`\]` → `]`), has its legacy
/// enclosing `$…$` dropped for `latexmath` (backwards compatibility with
/// AsciiDoc.py, mirroring [`InlineStemMacroReplacer`]), and is then run
/// through the real substitution pipeline under its resolved substitution
/// group — [`SubstitutionGroup::Stem`] (special characters only) for a bare
/// macro — via [`passthrough_text`], so a custom
/// [`InlineSubstitutionRenderer`](crate::parser::InlineSubstitutionRenderer)'s
/// escaping is honored exactly as it would be for the string pipeline's own
/// restore step. The result becomes the node's `value`; the cost is an owned
/// value rather than a `'src` borrow, since the pipeline's output is not
/// guaranteed to coincide with the source (the same trade-off
/// [`apply_passthroughs`](super::passthrough_step::apply_passthroughs) makes
/// for `++…++`/`$$…$$`).
///
/// An **escaped** macro (`\stem:[…]`) drops its backslash and stays literal,
/// mirroring every other macro family's escape handling.
///
/// A macro carrying an **explicit substitution list** (`stem:c,q[…]`) is
/// recognized too: [`resolve_stem_subs`] resolves the list to a
/// [`SubstitutionGroup`] exactly as [`InlineStemMacroReplacer`] does, and
/// [`stem_expression_value`] runs the expression through that group instead
/// of the bare macro's [`SubstitutionGroup::Stem`] — the analogous
/// `pass:c,q[…]` form takes the same approach (see
/// `passthrough_step::apply_passthroughs`'s doc comment), except a `Stem`
/// node already has a single `value` field to hold the substituted result,
/// so no richer subtree is needed here. This step is **additive**: nothing
/// is wired into the parse path.
///
/// [`InlineStemMacroReplacer`]: crate::content::passthroughs
pub(super) fn apply_stem<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes, Masked::UNKNOWN);

    // Cheap pre-filter mirroring `Passthroughs::extract_from`'s own guard.
    if !(s.contains(':') && (s.contains("stem:") || s.contains("math:"))) {
        return nodes;
    }

    let matches = find_stem_matches(&s, &nodes, &pieces, root, parser);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every STEM macro at this level — both the bare form and one
/// carrying an explicit substitution list (an optional group
/// [`INLINE_STEM_MACRO`] captures ahead of the bracket).
fn find_stem_matches<'src>(
    s: &str,
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Vec<MacroMatch<'src>> {
    let mut matches = Vec::new();

    for caps in INLINE_STEM_MACRO.captures_iter(s) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();

        let full = whole.start()..whole.end();

        if whole.as_str().starts_with('\\') {
            matches.push(MacroMatch {
                kind: MacroMatchKind::Unescape {
                    backslash: full.start,
                },
                full,
            });

            continue;
        }

        let node = match build_stem_node(&caps, &full, nodes, pieces, root, parser) {
            Some(node) => node,

            // A macro this increment defers — an explicit substitution list
            // whose steps need more context than one `Text` run at a time,
            // embedding an already-extracted passthrough (see
            // `subs_are_local`) — is left as literal source for a later
            // increment.
            None => continue,
        };

        matches.push(MacroMatch {
            kind: MacroMatchKind::Node {
                consumed: full.clone(),
                node: Box::new(node),
            },
            full,
        });
    }

    matches
}

/// Builds one [`Stem`](InlineNode::Stem) node from a STEM macro match — see
/// [`apply_stem`] for how the expression is unescaped, has its legacy `$…$`
/// wrapper dropped, and is substituted into `value`. Returns `None` for a
/// form this increment defers (see [`subs_are_local`]).
///
/// The expression body (capture group 4) is recovered via
/// [`emit_range`] rather than sliced as one literal string, because it may
/// embed an already-extracted [`Raw`](InlineNode::Raw) passthrough
/// (`stem:[+++<b>x</b>+++]`) — the case `apply_stem`'s own doc comment
/// explains the extraction ordering exists to support.
/// [`stem_expression_value`] does the actual reconstruction.
fn build_stem_node<'src>(
    caps: &Captures<'_>,
    full: &std::ops::Range<usize>,
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Option<InlineNode<'src>> {
    let location = source_slice(pieces, full.clone(), root);

    let notation = match &caps[2] {
        "latexmath" => StemNotation::LatexMath,

        "asciimath" => StemNotation::AsciiMath,

        // `stem`: the notation is resolved from the `stem` document
        // attribute (defaulting to AsciiMath).
        _ => match stem_notation(parser) {
            QuoteType::LatexMath => StemNotation::LatexMath,
            _ => StemNotation::AsciiMath,
        },
    };

    #[allow(clippy::unwrap_used)]
    let expr_match = caps.get(4).unwrap();
    let expr_range = expr_match.start()..expr_match.end();

    let mut emitted = Vec::new();
    emit_range(nodes, pieces, expr_range, &mut emitted);

    let subs = resolve_stem_subs(caps.get(3).map(|m| m.as_str()), root, parser);

    // An explicit substitution list naming a step that needs more than one
    // `Text` run of context (Quotes, AttributeReferences,
    // CharacterReplacements, Macros, PostReplacement) cannot be applied
    // fragment-by-fragment around an embedded passthrough without risking a
    // construct that spans the boundary going unrecognized (see
    // `subs_are_local`). The bare-macro default (`SubstitutionGroup::Stem`,
    // special characters only) is always local, so this only ever defers an
    // explicit list.
    if emitted.len() > 1 && !subs_are_local(&subs) {
        return None;
    }

    let (value, source) = stem_expression_value(&emitted, notation, &subs, parser);

    Some(InlineNode::Stem(Stem {
        notation,
        // The body's own nodes, kept rather than folded away: an embedded
        // passthrough is its own extraction entry, and `value` alone gives a
        // walk no way to reach it.
        children: emitted,
        // The author's expression is recorded only where the group changed it,
        // the same rule `RawOrigin::Passthrough`'s own `source_text` follows.
        source_text: (source != value).then_some(source),
        value: CowStr::from(value),
        subs,
        location,
    }))
}

/// Computes a [`Stem`](InlineNode::Stem) node's `value` from the expression
/// body's [`emit_range`]-recovered nodes.
///
/// The common case — the whole expression is one `Text` run, no nested
/// passthrough — matches the string pipeline exactly: the raw source is
/// unescaped (`\]` → `]`), has its legacy `latexmath` `$…$` wrapper dropped
/// (AsciiDoc.py backward-compat), and is run through the resolved
/// substitution group ([`SubstitutionGroup::Stem`] for a bare macro).
///
/// When the expression embeds one or more already-extracted
/// [`Raw`](InlineNode::Raw) passthroughs, each `Text` run around them is
/// unescaped and substituted the same way and each `Raw` run is spliced in
/// **verbatim, with no further substitution** — mirroring how the string
/// pipeline's passthrough-restore recursion re-splices a nested passthrough
/// into an outer construct's already-substituted text
/// (`PassthroughRestoreReplacer`'s own recursive `if … contains('\u{96}')`
/// branch). The legacy `$…$` wrapper is dropped only in the single-`Text`-run
/// case; a `$` immediately beside a nested passthrough is a narrower
/// divergence, documented and pinned by a test.
///
/// `subs` is the group the expression's `Text` runs are substituted through —
/// [`SubstitutionGroup::Stem`] for a bare macro, or the group an explicit
/// substitution list resolves to (see [`resolve_stem_subs`]).
fn stem_expression_value(
    emitted: &[InlineNode<'_>],
    notation: StemNotation,
    subs: &SubstitutionGroup,
    parser: &Parser,
) -> (String, String) {
    if let [InlineNode::Text { value: text, .. }] = emitted {
        let mut expr = text.to_string();

        if expr.contains("\\]") {
            expr = expr.replace("\\]", "]");
        }

        if notation == StemNotation::LatexMath
            && expr.len() >= 2
            && expr.starts_with('$')
            && expr.ends_with('$')
        {
            expr = expr[1..expr.len() - 1].to_string();
        }

        // Unanchored: `expr` is an owned copy the caller has already rewritten
        // (unescaped `\]`, stripped `$` delimiters), so its bytes no longer
        // line up with the document. A STEM body's substitution list records no
        // warning to locate anyway — see `passthrough_text`.
        return (passthrough_text(Span::new(&expr), subs, parser), expr);
    }

    let mut value = String::new();
    let mut source = String::new();

    for node in emitted {
        match node {
            InlineNode::Text { value: text, .. } => {
                let mut text = text.to_string();

                if text.contains("\\]") {
                    text = text.replace("\\]", "]");
                }

                value.push_str(&passthrough_text(Span::new(&text), subs, parser));
                source.push_str(&text);
            }

            InlineNode::Raw { value: raw, .. } => {
                value.push_str(raw);
                source.push_str(raw);
            }

            // Unreachable: `apply_stem` runs immediately after
            // `apply_passthroughs`, whose output is `Text`/`Raw` only — no
            // other step has run yet to build any other kind.
            _ => {}
        }
    }

    (value, source)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use super::{
        super::test_support::{build_src, fold_html, golden_passthroughs, seed},
        apply_stem,
    };
    use crate::{
        Parser, Span,
        inlines::{InlineNode, Stem, StemNotation},
        parser::HtmlSubstitutionRenderer,
    };

    /// Asserts that `node` is a [`Stem`](InlineNode::Stem) of `notation`
    /// with the given `value`.
    fn assert_stem(node: &InlineNode<'_>, notation: StemNotation, value: &str) {
        match node {
            InlineNode::Stem(Stem {
                notation: n,
                value: v,
                ..
            }) => {
                assert_eq!(*n, notation, "notation");
                assert_eq!(v.as_ref(), value, "value");
            }

            other => panic!("expected Stem({notation:?}, {value:?}), got {other:?}"),
        }
    }

    #[test]
    fn apply_stem_is_a_noop_without_stem_syntax() {
        let source = Span::new("plain text, no stem syntax here");
        let seeded = seed(source);
        let nodes = apply_stem(seeded.clone(), source, &Parser::default());

        assert_eq!(nodes, seeded);
    }

    #[test]
    fn a_nested_delimited_passthrough_is_embedded_verbatim() {
        // The whole reason `Passthroughs::extract_from` extracts STEM *after*
        // both passthrough passes: a `+++…+++` inside a STEM expression is
        // already a `Raw` node by the time `apply_stem` runs, and must be
        // spliced into the STEM's value verbatim (no specialcharacters
        // escaping) rather than deferred.
        let nodes = build_src(Span::new("stem:[+++<b>x</b>+++]"));

        assert_eq!(nodes.len(), 1);
        assert_stem(&nodes[0], StemNotation::AsciiMath, "<b>x</b>");
    }

    #[test]
    fn a_nested_passthrough_beside_ordinary_text_mixes_both() {
        // The surrounding text is still unescaped and substituted normally
        // (special characters only, under the default `SubstitutionGroup::
        // Stem`); only the passthrough's own content is exempt.
        let nodes = build_src(Span::new("stem:[a < b +++<i>or</i>+++ c]"));

        assert_eq!(nodes.len(), 1);
        assert_stem(&nodes[0], StemNotation::AsciiMath, "a &lt; b <i>or</i> c");
    }

    #[test]
    fn a_text_segment_beside_a_nested_passthrough_unescapes_a_closing_bracket() {
        // Each `Text` segment around a nested passthrough is unescaped
        // (`\]` → `]`) independently, exactly like the single-run case.
        let nodes = build_src(Span::new(r"stem:[+++<b>x</b>+++ a\]b]"));

        assert_eq!(nodes.len(), 1);
        assert_stem(&nodes[0], StemNotation::AsciiMath, "<b>x</b> a]b");
    }

    #[test]
    fn a_nested_bare_pass_macro_is_embedded_verbatim() {
        let nodes = build_src(Span::new("stem:[pass:[<b>x</b>]]"));

        assert_eq!(nodes.len(), 1);
        assert_stem(&nodes[0], StemNotation::AsciiMath, "<b>x</b>");
    }

    #[test]
    fn a_latexmath_dollar_wrapper_beside_a_nested_passthrough_is_a_documented_divergence() {
        // The legacy `$…$` wrapper is dropped only on the common
        // single-`Text`-run case; a `$` immediately beside a nested
        // passthrough is a narrower, honestly-labeled divergence from the
        // string pipeline (which strips it even here, since its `$…$` check
        // runs on the raw captured text before the nested passthrough's
        // sentinel is ever resolved).
        let source = "latexmath:[$+++x+++$]";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        // Not stripped here (divergence): the golden fold *does* strip it.
        assert_stem(&nodes[0], StemNotation::LatexMath, "$x$");

        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});
        let golden = golden_passthroughs(source);

        assert_ne!(folded, golden);
        assert_eq!(golden, r"\(x\)");
    }

    #[test]
    fn bare_stem_macro_defaults_to_asciimath() {
        let nodes = build_src(Span::new("stem:[x^2]"));

        assert_eq!(nodes.len(), 1);
        assert_stem(&nodes[0], StemNotation::AsciiMath, "x^2");
    }

    #[test]
    fn asciimath_macro_is_recognized_explicitly() {
        let nodes = build_src(Span::new("asciimath:[x != 0]"));

        assert_eq!(nodes.len(), 1);
        assert_stem(&nodes[0], StemNotation::AsciiMath, "x != 0");
    }

    #[test]
    fn latexmath_macro_is_recognized() {
        let nodes = build_src(Span::new(r"latexmath:[\sqrt{4} = 2]"));

        assert_eq!(nodes.len(), 1);
        assert_stem(&nodes[0], StemNotation::LatexMath, r"\sqrt{4} = 2");
    }

    #[test]
    fn latexmath_drops_a_legacy_dollar_wrapper() {
        let nodes = build_src(Span::new(r"latexmath:[$x = 1$]"));

        assert_eq!(nodes.len(), 1);
        assert_stem(&nodes[0], StemNotation::LatexMath, "x = 1");
    }

    #[test]
    fn a_bare_stem_macro_honors_the_stem_document_attribute() {
        use crate::parser::ModificationContext;

        let parser = Parser::default().with_intrinsic_attribute(
            "stem",
            "latexmath",
            ModificationContext::Anywhere,
        );

        let source = Span::new("stem:[x]");
        let nodes = apply_stem(seed(source), source, &parser);

        assert_eq!(nodes.len(), 1);
        assert_stem(&nodes[0], StemNotation::LatexMath, "x");
    }

    #[test]
    fn a_stem_macro_unescapes_an_escaped_closing_bracket() {
        let nodes = build_src(Span::new(r"stem:[a\]b]"));

        assert_eq!(nodes.len(), 1);
        assert_stem(&nodes[0], StemNotation::AsciiMath, "a]b");
    }

    #[test]
    fn a_stem_macro_escapes_special_characters() {
        // The default (no explicit substitution list) substitution group is
        // `SubstitutionGroup::Stem`, which applies only special characters.
        let nodes = build_src(Span::new("stem:[a < b]"));

        assert_eq!(nodes.len(), 1);
        assert_stem(&nodes[0], StemNotation::AsciiMath, "a &lt; b");
    }

    #[test]
    fn an_escaped_stem_macro_stays_literal() {
        let source = r"\stem:[x]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Stem(_))),
            "an escaped STEM macro must not build a Stem node: {nodes:?}"
        );

        assert_eq!(fold_html(&nodes, &HtmlSubstitutionRenderer {}), "stem:[x]");
    }

    #[test]
    fn a_stem_macro_with_an_explicit_subs_list_applies_only_the_named_steps() {
        // `stem:c[…]` names an explicit substitution list — here, special
        // characters only, the same result the default `SubstitutionGroup::
        // Stem` produces for this input, but taking the explicit-list path
        // through `resolve_stem_subs` rather than the bare-macro default.
        let source = "stem:c[a < b]";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        assert_stem(&nodes[0], StemNotation::AsciiMath, "a &lt; b");

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_passthroughs(source)
        );
    }

    #[test]
    fn a_stem_macro_with_multiple_subs_applies_them_in_the_order_given() {
        // `stem:q,c[…]` resolves to `Custom([Quotes, SpecialCharacters])` —
        // the order written, not the *normal* effective order — mirroring
        // the analogous `pass:q,c[…]` fixture exactly (see
        // `passthrough_step`'s own test of the same name).
        let source = "stem:q,c[<b> *bold*]";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        assert_stem(
            &nodes[0],
            StemNotation::AsciiMath,
            "&lt;b&gt; &lt;strong&gt;bold&lt;/strong&gt;",
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_passthroughs(source)
        );
    }

    #[test]
    fn a_stem_macro_with_an_unrecognized_subs_name_skips_it() {
        // An unrecognized name resolves to zero steps rather than
        // invalidating the whole list, mirroring `SubstitutionGroup::
        // from_custom_string`'s "skip and keep going" resolution: with no
        // steps at all the expression is substituted completely untouched.
        let source = "stem:bogus[a < b]";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        assert_stem(&nodes[0], StemNotation::AsciiMath, "a < b");

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_passthroughs(source)
        );
    }

    #[test]
    fn a_recognized_stem_subs_name_beside_an_unrecognized_one_is_still_honored() {
        // `stem:c,bogus[…]` resolves to the same `Custom([SpecialCharacters])`
        // as `stem:c[…]` alone.
        let nodes = build_src(Span::new("stem:c,bogus[a < b]"));

        assert_eq!(nodes.len(), 1);
        assert_stem(&nodes[0], StemNotation::AsciiMath, "a &lt; b");
    }

    #[test]
    fn an_escaped_stem_macro_with_a_subs_list_stays_literal() {
        // `\stem:c[…]` drops the single backslash and keeps the whole
        // `stem:c[…]` text literal, mirroring the escape branch every other
        // form of this macro takes.
        let source = r"\stem:c[a < b]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Stem(_))),
            "an escaped STEM macro must not build a Stem node: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_passthroughs(source)
        );
    }

    #[test]
    fn an_explicit_local_subs_list_still_applies_beside_a_nested_passthrough() {
        // `stem:c[…]` resolves to `Custom([SpecialCharacters])` — the same,
        // purely local step the bare macro's default `SubstitutionGroup::
        // Stem` already applies safely per `Text` run around a nested
        // passthrough (see `fold_matches_the_string_pipeline_through_stem`'s
        // own fixtures) — so `subs_are_local` does not defer it even though
        // the expression embeds a `Raw` node.
        let source = "stem:c[a < b +++<i>or</i>+++ c]";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        assert_stem(&nodes[0], StemNotation::AsciiMath, "a &lt; b <i>or</i> c");

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_passthroughs(source)
        );
    }

    #[test]
    fn a_non_local_explicit_subs_list_beside_a_nested_passthrough_is_a_documented_divergence() {
        // `stem:q[*a +++x+++ b*]`: the quote pair's delimiters fall on either
        // side of the embedded, already-extracted `+++x+++` passthrough. The
        // string pipeline substitutes the *whole* expression as one string
        // (the passthrough's content merely protected, not absent), so it
        // recognizes the pair; this builder would otherwise have to apply
        // `Quotes` to each `Text` run around the `Raw` independently, and
        // neither run contains a complete pair on its own. Rather than
        // silently diverge, `subs_are_local` rejects a non-local explicit
        // list here and the whole macro is left unrecognized (see
        // `build_stem_node`).
        let source = "stem:q[*a +++x+++ b*]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Stem(_))),
            "a non-local subs list beside a nested passthrough must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* recognize the quote pair.
        let golden = golden_passthroughs(source);
        assert!(golden.contains("<strong>"), "golden: {golden}");
        assert_ne!(fold_html(&nodes, &HtmlSubstitutionRenderer {}), golden);
    }

    #[test]
    fn fold_matches_the_string_pipeline_through_stem() {
        // For each fixture, folding the single-pass tree (all steps, STEM
        // included) reproduces the string pipeline's output byte-for-byte.
        // This is the differential corpus (design §5.3) that pins the STEM
        // increment.
        let fixtures = [
            "stem:[x^2]",
            "asciimath:[x != 0]",
            r"latexmath:[\sqrt{4} = 2]",
            r"latexmath:[$x = 1$]",
            "stem:[a < b]",
            r"stem:[a\]b]",
            "before stem:[x] after",
            "a stem:[x] and a stem:[y]",
            "*bold* stem:[x^2] *more bold*",
            r"\stem:[x]",
            // A nested, already-extracted passthrough inside the expression —
            // the case `Passthroughs::extract_from` orders STEM extraction
            // after both passthrough passes to support.
            "stem:[+++<b>x</b>+++]",
            "stem:[a < b +++<i>or</i>+++ c]",
            "stem:[pass:[<b>x</b>]]",
            "latexmath:[+++x+++]",
            // An explicit substitution list, in various shapes.
            "stem:c[a < b]",
            "stem:q,c[<b> *bold*]",
            "stem:bogus[a < b]",
            "stem:c,bogus[a < b]",
            r"\stem:c[a < b]",
            r"stem:c[a\]b]",
            // A *local* explicit list (special characters only) beside a
            // nested passthrough is still safe to apply per `Text` run.
            "stem:c[a < b +++<i>or</i>+++ c]",
        ];

        for source in fixtures {
            let nodes = build_src(Span::new(source));
            let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});

            assert_eq!(
                folded,
                golden_passthroughs(source),
                "mismatch for {source:?}"
            );
        }
    }

    #[test]
    fn a_stem_macro_inside_a_passthrough_is_not_re_extracted() {
        // `Passthroughs::extract_from` extracts STEM last, after the
        // passthrough-macro passes, precisely so a placeholder already
        // extracted (here, the whole `+++…+++` content) is not re-scanned for
        // STEM syntax. `apply_stem` reproduces this by running over the level
        // `apply_passthroughs` already refined into a `Raw` leaf, which is
        // opaque to `INLINE_STEM_MACRO`'s match string.
        let source = "+++stem:[x]+++";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Stem(_))),
            "STEM syntax inside a passthrough must not be re-extracted: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_passthroughs(source)
        );
    }

    #[test]
    fn a_stem_node_keeps_its_bodys_own_nodes() {
        // `children` is the body as it *is*, where `value` is its rendering.
        // They are redundant for a flat body and differ exactly when one
        // embeds a passthrough — the case the field exists for, since an
        // embedded passthrough is its own extraction entry and folding it into
        // `value` left a walk no way to reach it.
        use crate::inlines::{InlineNode, RawOrigin};

        // A flat body: one `Text` run, and `value` says the same thing.
        let nodes = build_src(Span::new("stem:[x^2]"));
        let InlineNode::Stem(stem) = &nodes[0] else {
            panic!("expected a Stem, got {:?}", nodes[0]);
        };

        assert_eq!(stem.children.len(), 1);
        assert!(matches!(stem.children[0], InlineNode::Text { .. }));
        assert_eq!(stem.value.as_ref(), "x^2");

        // An embedded passthrough: three nodes, the middle one the `Raw` leaf
        // carrying its own record — which is the whole point.
        let nodes = build_src(Span::new("stem:[x +++<b>+++ y]"));
        let InlineNode::Stem(stem) = &nodes[0] else {
            panic!("expected a Stem, got {:?}", nodes[0]);
        };

        assert_eq!(stem.children.len(), 3, "{:?}", stem.children);

        let InlineNode::Raw { value, origin, .. } = &stem.children[1] else {
            panic!("expected a Raw, got {:?}", stem.children[1]);
        };

        assert_eq!(value.as_ref(), "<b>");
        assert_eq!(
            *origin,
            RawOrigin::Passthrough {
                subs: crate::content::SubstitutionGroup::None,
                source_text: None,
            }
        );

        // The rendering is unchanged by keeping them — this increment moves no
        // byte, which is what lets the whole suite stay green.
        assert_eq!(stem.value.as_ref(), "x <b> y");
    }

    #[test]
    fn a_stem_bodys_nodes_are_only_text_and_raw() {
        // The invariant every *other* walk in the crate relies on without
        // saying so: `apply_stem` runs immediately after `apply_passthroughs`,
        // whose output is `Text`/`Raw` only, so no cross-reference, macro or
        // span can ever be nested inside a `Stem`'s body. That is why adding
        // `children` here obliges no other walk to descend into it.
        //
        // Asserted rather than assumed, because this branch has twice shipped a
        // walk that missed a container it should have descended into. If a
        // later step ever moves ahead of this one, this fails and names the
        // walks that then need revisiting.
        use crate::inlines::InlineNode;

        for source in [
            "stem:[x +++<b>+++ y]",
            "stem:[a $$lit$$ b]",
            "stem:[see *bold* and <<ref>> and image:i.png[]]",
            "stem:[{attr} and link:x.html[t]]",
            "latexmath:[e ++f++ g]",
        ] {
            let nodes = build_src(Span::new(source));

            let InlineNode::Stem(stem) = &nodes[0] else {
                panic!("{source:?} did not build a Stem; got {:?}", nodes[0]);
            };

            for child in &stem.children {
                assert!(
                    matches!(child, InlineNode::Text { .. } | InlineNode::Raw { .. }),
                    "{source:?} put a {child:?} in a STEM body; every walk that \
                     skips `Stem::children` now needs revisiting"
                );
            }
        }
    }
}
