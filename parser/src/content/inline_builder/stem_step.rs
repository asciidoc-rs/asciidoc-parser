//! The inline-STEM extraction step.

use regex::Captures;

use super::{
    macros::{MacroMatch, MacroMatchKind, image::range_is_verbatim, rebuild_macro_level},
    passthrough_step::passthrough_text,
    quotes::{Piece, build_match_string, source_slice},
};
use crate::{
    Parser, Span,
    content::{INLINE_STEM_MACRO, SubstitutionGroup, stem_notation},
    inlines::{InlineNode, Stem, StemNotation},
    parser::QuoteType,
    strings::CowStr,
};

/// Recognizes inline STEM macros (`stem:[…]`, `asciimath:[…]`,
/// `latexmath:[…]`), replacing each with a [`Stem`](InlineNode::Stem) leaf.
///
/// STEM is an **implicit passthrough**: [`Passthroughs::extract_from`]
/// extracts it last, after both passthrough-macro passes, so that any
/// passthrough placeholder nested inside a STEM expression survives.
/// [`apply_stem`] mirrors that ordering – [`build`](super::build) runs it
/// immediately after
/// [`apply_passthroughs`](super::passthrough_step::apply_passthroughs)
/// and ahead of every other step – so a STEM expression's content is never
/// touched by specialcharacters, quotes, replacements, or macros, exactly
/// like a `+++…+++`/`++…++`/`$$…$$`/`pass:[…]` passthrough. It reuses the
/// string pipeline's *exact* recognition – [`INLINE_STEM_MACRO`] is now
/// shared `pub(crate)`, alongside the [`stem_notation`] helper that resolves
/// a bare `stem:[…]` macro's notation from the `stem` document attribute –
/// so only the recognition *sink* differs (§4.1).
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
/// Two forms are deferred, each documented and pinned by a divergence test:
/// a macro carrying an **explicit substitution list** (`stem:c,q[…]`, whose
/// content would need a richer subtree than a single `Stem` leaf can hold –
/// the same reason a `pass:` macro with an explicit list is deferred), and a
/// match whose expression **crosses an already-recognized construct**
/// (non-verbatim – in practice this cannot yet happen, since `apply_stem`
/// only ever runs on the seed
/// [`apply_passthroughs`](super::passthrough_step::apply_passthroughs) has
/// already refined into `Text`/[`Raw`](InlineNode::Raw) leaves, but the
/// check is kept for the same reason every other family keeps it). This
/// step is **additive**: nothing is wired into the parse path.
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

    let matches = find_stem_matches(&s, &pieces, root, parser);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every STEM macro at this level, skipping the deferred form
/// [`apply_stem`] documents: a macro carrying an explicit substitution list
/// (an optional group [`INLINE_STEM_MACRO`] captures ahead of the bracket).
fn find_stem_matches<'src>(
    s: &str,
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

        // An explicit substitution list (`stem:c,q[…]`) is deferred.
        if caps.get(3).is_some() {
            continue;
        }

        if !range_is_verbatim(pieces, &full) {
            continue;
        }

        if whole.as_str().starts_with('\\') {
            matches.push(MacroMatch {
                kind: MacroMatchKind::Unescape {
                    backslash: full.start,
                },
                full,
            });

            continue;
        }

        let node = build_stem_node(&caps, &full, pieces, root, parser);

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

/// Builds one [`Stem`](InlineNode::Stem) node from a verbatim, unescaped STEM
/// macro match – see [`apply_stem`] for how the expression is unescaped, has
/// its legacy `$…$` wrapper dropped, and is substituted into `value`.
fn build_stem_node<'src>(
    caps: &Captures<'_>,
    full: &std::ops::Range<usize>,
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

    // Unescape any escaped closing brackets in the expression.
    let mut expr = caps[4].to_string();
    if expr.contains("\\]") {
        expr = expr.replace("\\]", "]");
    }

    // Drop legacy enclosing `$…$` around latexmath content (for backwards
    // compatibility with AsciiDoc.py).
    if notation == StemNotation::LatexMath
        && expr.len() >= 2
        && expr.starts_with('$')
        && expr.ends_with('$')
    {
        expr = expr[1..expr.len() - 1].to_string();
    }

    let value = passthrough_text(&expr, &SubstitutionGroup::Stem, parser);

    InlineNode::Stem(Stem {
        notation,
        value: CowStr::from(value),
        location,
    })
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
        inlines::{InlineNode, SpanForm, Stem, StemNotation, StyleVariant, Styled},
        parser::HtmlSubstitutionRenderer,
        strings::CowStr,
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
    fn a_match_whose_content_crosses_an_already_built_node_is_deferred() {
        // In practice `apply_stem` only ever runs on the level
        // `apply_passthroughs` has already refined into `Text`/`Raw` leaves
        // (never a `Styled` node), so `range_is_verbatim`'s false branch is
        // defensive – kept for the same reason every other macro family keeps
        // it (see `passthrough_step`'s own
        // `a_match_whose_content_crosses_an_already_built_node_is_deferred`).
        // Exercise it directly, feeding a hand-built level whose STEM match
        // spans an already-built `Styled` node, to document the intended
        // fallback: the whole match is left unrecognized rather than
        // mis-sliced.
        // `build_match_string` only treats a `Text` node as verbatim (rather
        // than an opaque placeholder) when its `value` equals its own
        // `location.data()` – so, unlike the passthrough version of this test
        // (whose `+++` delimiter is the same literal on both sides), each
        // `Text` node here needs its *own* matching span.
        let prefix_location = Span::new("stem:[x");
        let styled_location = Span::new("*b*");
        let suffix_location = Span::new("]");

        let nodes = vec![
            InlineNode::Text {
                value: CowStr::from("stem:[x"),
                location: prefix_location,
            },
            InlineNode::Styled(Styled {
                variant: StyleVariant::Strong,
                form: SpanForm::Constrained,
                id: None,
                roles: vec![],
                attrs: None,
                children: vec![],
                location: styled_location,
            }),
            InlineNode::Text {
                value: CowStr::from("]"),
                location: suffix_location,
            },
        ];

        let root = Span::new("stem:[x]");
        let result = apply_stem(nodes.clone(), root, &Parser::default());

        assert_eq!(
            result, nodes,
            "a non-verbatim match must be left unrecognized"
        );
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
    fn a_stem_macro_with_an_explicit_subs_list_is_a_documented_divergence() {
        // `stem:c[…]` names an explicit substitution list, which would need a
        // richer subtree than a single `Stem` leaf can hold (the same reason
        // `pass:c[…]` is deferred) – deferred to a later increment.
        let source = "stem:c[a < b]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Stem(_))),
            "a stem: macro with an explicit subs list must be left unrecognized: {nodes:?}"
        );

        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});
        let golden = golden_passthroughs(source);

        assert_ne!(folded, golden);
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
