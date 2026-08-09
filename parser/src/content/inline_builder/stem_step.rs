//! The inline-STEM extraction step.

use regex::Captures;

use super::{
    macros::{MacroMatch, MacroMatchKind, rebuild_macro_level},
    passthrough_step::passthrough_text,
    quotes::{Piece, build_match_string, emit_range, source_slice},
};
use crate::{
    Parser, Span,
    content::{INLINE_STEM_MACRO, SubstitutionGroup, stem_notation},
    inlines::{InlineNode, Stem, StemNotation},
    parser::QuoteType,
    strings::CowStr,
};

/// Resolves the [`SubstitutionGroup`] a STEM macro's expression runs
/// through: [`SubstitutionGroup::Stem`] (special characters only) for a bare
/// macro, or the group an explicit substitution list (`stem:c,q[…]`)
/// resolves to – mirroring [`SubstitutionGroup::from_custom_string`]/
/// [`InlineStemMacroReplacer`](crate::content::passthroughs)'s own
/// resolution exactly, including its "skip and keep going" handling of an
/// unrecognized name. As throughout this module, this does *not* raise the
/// string pipeline's own `InvalidSubstitutionTypeForStemMacro` warning for
/// an invalid name, deferring that side effect to the cutover (design §5.2
/// Phase 4, step 6), since it does not change the fold's output bytes.
fn resolve_stem_subs(subs_list: Option<&str>) -> SubstitutionGroup {
    match subs_list {
        None => SubstitutionGroup::Stem,
        Some(subs_list) => SubstitutionGroup::from_custom_string(None, subs_list).0,
    }
}

/// Recognizes inline STEM macros (`stem:[…]`, `asciimath:[…]`,
/// `latexmath:[…]`), replacing each with a [`Stem`](InlineNode::Stem) leaf.
///
/// STEM is an **implicit passthrough**: [`Passthroughs::extract_from`]
/// extracts it last, after both passthrough-macro passes, *specifically so
/// that a passthrough placeholder nested inside a STEM expression survives
/// and is recursively restored* (its own doc comment). [`apply_stem`] mirrors
/// that ordering – [`build`](super::build) runs it immediately after
/// [`apply_passthroughs`](super::passthrough_step::apply_passthroughs) and
/// ahead of every other step – so a STEM expression's content is never
/// touched by specialcharacters, quotes, replacements, or macros, exactly
/// like a `+++…+++`/`++…++`/`$$…$$`/`pass:[…]` passthrough. It reuses the
/// string pipeline's *exact* recognition – [`INLINE_STEM_MACRO`] is now
/// shared `pub(crate)`, alongside the [`stem_notation`] helper that resolves
/// a bare `stem:[…]` macro's notation from the `stem` document attribute –
/// so only the recognition *sink* differs (§4.1).
///
/// Because it runs immediately after `apply_passthroughs`, the *only* node
/// kinds `apply_stem` can ever see are `Text` and [`Raw`](InlineNode::Raw)
/// (a nested, already-extracted passthrough) – no other step has run yet to
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
/// group – [`SubstitutionGroup::Stem`] (special characters only) for a bare
/// macro – via [`passthrough_text`], so a custom
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
/// of the bare macro's [`SubstitutionGroup::Stem`] – the analogous
/// `pass:c,q[…]` form takes the same approach (see
/// `passthrough_step::apply_passthroughs`'s doc comment), except a `Stem`
/// node already has a single `value` field to hold the substituted result,
/// so no richer subtree is needed here. This step is **additive**: nothing
/// is wired into the parse path.
///
/// [`Passthroughs::extract_from`]: crate::content::Passthroughs::extract_from
/// [`InlineStemMacroReplacer`]: crate::content::passthroughs
pub(super) fn apply_stem<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes);

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

/// Finds every STEM macro at this level – both the bare form and one
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

        let node = build_stem_node(&caps, &full, nodes, pieces, root, parser);

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

/// Builds one [`Stem`](InlineNode::Stem) node from a STEM macro match – see
/// [`apply_stem`] for how the expression is unescaped, has its legacy `$…$`
/// wrapper dropped, and is substituted into `value`.
///
/// The expression body (capture group 4) is recovered via
/// [`emit_range`] rather than sliced as one literal string, because it may
/// embed an already-extracted [`Raw`](InlineNode::Raw) passthrough
/// (`stem:[+++<b>x</b>+++]`) – the case `apply_stem`'s own doc comment
/// explains the extraction ordering exists to support.
/// [`stem_expression_value`] does the actual reconstruction.
fn build_stem_node<'src>(
    caps: &Captures<'_>,
    full: &std::ops::Range<usize>,
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> InlineNode<'src> {
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

    let subs = resolve_stem_subs(caps.get(3).map(|m| m.as_str()));
    let value = stem_expression_value(&emitted, notation, &subs, parser);

    InlineNode::Stem(Stem {
        notation,
        value: CowStr::from(value),
        location,
    })
}

/// Computes a [`Stem`](InlineNode::Stem) node's `value` from the expression
/// body's [`emit_range`]-recovered nodes.
///
/// The common case – the whole expression is one `Text` run, no nested
/// passthrough – matches the string pipeline exactly: the raw source is
/// unescaped (`\]` → `]`), has its legacy `latexmath` `$…$` wrapper dropped
/// (AsciiDoc.py backward-compat), and is run through the resolved
/// substitution group ([`SubstitutionGroup::Stem`] for a bare macro).
///
/// When the expression embeds one or more already-extracted
/// [`Raw`](InlineNode::Raw) passthroughs, each `Text` run around them is
/// unescaped and substituted the same way and each `Raw` run is spliced in
/// **verbatim, with no further substitution** – mirroring how the string
/// pipeline's passthrough-restore recursion re-splices a nested passthrough
/// into an outer construct's already-substituted text
/// (`PassthroughRestoreReplacer`'s own recursive `if … contains('\u{96}')`
/// branch). The legacy `$…$` wrapper is dropped only in the single-`Text`-run
/// case; a `$` immediately beside a nested passthrough is a narrower
/// divergence, documented and pinned by a test.
///
/// `subs` is the group the expression's `Text` runs are substituted through –
/// [`SubstitutionGroup::Stem`] for a bare macro, or the group an explicit
/// substitution list resolves to (see [`resolve_stem_subs`]).
fn stem_expression_value(
    emitted: &[InlineNode<'_>],
    notation: StemNotation,
    subs: &SubstitutionGroup,
    parser: &Parser,
) -> String {
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

        return passthrough_text(&expr, subs, parser);
    }

    let mut value = String::new();

    for node in emitted {
        match node {
            InlineNode::Text { value: text, .. } => {
                let mut text = text.to_string();

                if text.contains("\\]") {
                    text = text.replace("\\]", "]");
                }

                value.push_str(&passthrough_text(&text, subs, parser));
            }

            InlineNode::Raw { value: raw, .. } => {
                value.push_str(raw);
            }

            // Unreachable: `apply_stem` runs immediately after
            // `apply_passthroughs`, whose output is `Text`/`Raw` only – no
            // other step has run yet to build any other kind.
            _ => {}
        }
    }

    value
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
        // `stem:c[…]` names an explicit substitution list – here, special
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
        // `stem:q,c[…]` resolves to `Custom([Quotes, SpecialCharacters])` –
        // the order written, not the *normal* effective order – mirroring
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
            // A nested, already-extracted passthrough inside the expression –
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
}
