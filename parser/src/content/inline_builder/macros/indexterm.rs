//! Index-term recognition (`((term))`, `(((primary, secondary)))`,
//! `indexterm:[…]`, `indexterm2:[…]`).

use super::{MacroMatch, MacroMatchKind, rebuild_macro_level};
use crate::{
    Span,
    content::{
        INLINE_INDEXTERM,
        inline_builder::quotes::{Piece, SPAN_PLACEHOLDER, build_match_string, source_slice},
        normalize_index_text, strip_see_and_seealso,
    },
    inlines::{IndexTerm, InlineNode},
    strings::CowStr,
};

/// Matches [`INLINE_INDEXTERM`] at this level's escaped text, replacing each
/// recognized index term – the `((term))` / `(((primary, secondary,
/// tertiary)))` shorthand and the `indexterm:[…]` / `indexterm2:[…]` macro –
/// with the [`IndexTerm`](InlineNode::IndexTerm) node it produces and leaving
/// everything else in place.
pub(super) fn indexterm_macros_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes);

    // Cheap pre-filter mirroring the string step's guard: a shorthand needs a
    // `((` … `))` pair (its parens are not special, so they reach the macros
    // step intact), and a macro form needs a `:[` and `dexterm` (matching both
    // `indexterm:` and `indexterm2:`).
    if !((s.contains("((") && s.contains("))")) || (s.contains(":[") && s.contains("dexterm"))) {
        return nodes;
    }

    let matches = find_indexterm_matches(&s, &pieces, root);

    if matches.is_empty() {
        return nodes;
    }

    // The string replacer runs the shorthand through [`replace_with_lookahead`],
    // whose look-ahead retry (a shorthand absorbing trailing parens) has a subtle
    // consequence: if the whole substitution accumulates *no* output and the last
    // event is such a retry, the helper returns `Cow::Borrowed` and the caller
    // keeps the original text **unchanged**. Concretely, content that is nothing
    // but concealed shorthand terms (`(((coffee)))`, `(((a)))(((b)))`) is left
    // *literal*, where the same terms with any surrounding output render to
    // nothing. Detect that no-op and mirror it: leave the level untouched.
    //
    // This mirrors a **known string-pipeline bug**
    // (asciidoc-rs/asciidoc-parser#1123): a whole-content concealed term should
    // render empty, not literal. The additive builder reproduces it here to
    // keep byte-for-byte parity (design §5.3); the fix for both is to drop this
    // call at the cutover (design §5.2, Phase 4, step 6),
    // where `rendered_html()` becomes the fold and the golden output is updated.
    if indexterm_substitution_is_a_noop(&matches) {
        return nodes;
    }

    let macro_matches = matches.into_iter().map(|m| m.macro_match).collect();

    rebuild_macro_level(&nodes, &pieces, &s, macro_matches)
}

/// A recognized index-term match, plus the two facts the string replacer's
/// look-ahead loop needs to reproduce its `Cow::Borrowed` (no-op) return (see
/// [`indexterm_substitution_is_a_noop`]).
struct RecognizedIndexterm<'src> {
    macro_match: MacroMatch<'src>,

    /// Whether this match renders any output (a shown term, a kept parenthesis,
    /// or an unescaped literal). A concealed term renders nothing.
    rendered_nonempty: bool,

    /// Whether recognizing this match advances the string replacer via a
    /// look-ahead retry (`SkipAheadAndRetry`) – true only for a shorthand that
    /// absorbed trailing parens. The macro forms always `Continue`.
    is_skip: bool,
}

/// Reports whether the string replacer's index-term substitution over this
/// level would be a **no-op** – returning `Cow::Borrowed` and so leaving the
/// content unchanged (see [`indexterm_macros_level`]).
///
/// That happens exactly when the accumulated output is empty *and* the last
/// recognized match advanced via a look-ahead retry: an empty gap before every
/// match, every match rendering nothing, and the final match a paren-absorbing
/// shorthand. Any non-empty gap or shown term – or a trailing macro form, which
/// `Continue`s instead of retrying – makes the substitution `Cow::Owned` and
/// the terms are recognized normally.
fn indexterm_substitution_is_a_noop(matches: &[RecognizedIndexterm]) -> bool {
    let mut emitted_nonempty = false;
    let mut prev_end = 0;
    let mut last_is_skip = false;

    for m in matches {
        let full = &m.macro_match.full;

        // A non-empty gap before the match is literal text the replacer pushes,
        // making the accumulated output non-empty.
        if full.start > prev_end {
            emitted_nonempty = true;
        }

        if m.rendered_nonempty {
            emitted_nonempty = true;
        }

        prev_end = full.end;
        last_is_skip = m.is_skip;
    }

    last_is_skip && !emitted_nonempty
}

/// Finds every recognized index term at this level – both the macro forms
/// (`indexterm:[…]`, `indexterm2:[…]`) and the shorthand forms (`((term))`,
/// `(((primary, secondary, tertiary)))`) – as a [`MacroMatch`].
///
/// # Concealed vs. visible, and the recognition boundary
///
/// A **concealed** term (`indexterm:[…]`, `(((…)))`) renders to *nothing* –
/// [`render_index_term`](crate::parser::InlineSubstitutionRenderer::render_index_term) emits
/// no output for it – so, much like an inline anchor whose output is a function
/// of its id alone (see
/// [`find_anchor_matches`](super::anchors::find_anchor_matches)), it is
/// recognized regardless of what its argument crosses; the node simply carries
/// an empty `terms`. (The one exception is the string replacer's
/// whole-substitution no-op for a level of *only* concealed shorthand terms,
/// which [`indexterm_macros_level`] mirrors by leaving the level literal.)
///
/// A **visible** (flow) term (`indexterm2:[…]`, `((term))`) shows its term text
/// in the flow, so that text must be reconstructible from this level's escaped
/// match string. It is – with the *same* entity bytes the string pipeline sees,
/// since a [`CharRef`](InlineNode::CharRef) contributes its canonical entity to
/// the match string – whenever the term crosses no *opaque span* (an
/// earlier-recognized
/// [`Styled`](crate::inlines::Styled)/[`Ref`](crate::inlines::Ref), carried
/// here as a single [`SPAN_PLACEHOLDER`] rather than its rendered markup). A
/// visible term crossing such a span, or an `indexterm2:[…]` term carrying an
/// attribute list (an `=`, whose first positional attribute becomes the shown
/// term), is **deferred** – the match is left as literal source for a later
/// increment, each pinned by a divergence test.
///
/// A term crossing a [`synthesized`](Piece::synthesized) run (an attribute
/// expansion, or – reached at a tree's root – a filtered multi-line block's
/// own joined seed) is **not** deferred. The match string carries such a run's
/// bytes exactly, and this family reads its term from nowhere else – it never
/// slices `'src`, and an [`IndexTerm`] node carries no `Span`-typed field – so
/// the shown text is recovered precisely; only the node's `location` takes
/// design §4.4's coarse fallback. This is the same lift the anchor and
/// bare-e-mail families already made, for the same reason.
///
/// As in the additive builder generally, this performs *no* recognition side
/// effect; the string replacer records nothing in a catalog either (the HTML
/// backend generates no index), so there is none to skip here.
fn find_indexterm_matches<'src>(
    s: &str,
    pieces: &[Piece],
    root: Span<'src>,
) -> Vec<RecognizedIndexterm<'src>> {
    let mut matches = Vec::new();

    for caps in INLINE_INDEXTERM.captures_iter(s) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();

        let escaped = whole.as_str().starts_with('\\');

        // Macro form: group 1 is the name (`indexterm` / `indexterm2`), group 2
        // its argument.
        if let Some(name) = caps.get(1) {
            let full = whole.start()..whole.end();
            let is_visible = name.as_str() == "indexterm2";

            #[allow(clippy::unwrap_used)]
            let arg = caps.get(2).unwrap().as_str();

            if let Some(m) =
                build_indexterm_macro_match(is_visible, arg, full, escaped, pieces, root)
            {
                matches.push(m);
            }

            continue;
        }

        // Shorthand form: group 3 is the text enclosed by the outermost `((` …
        // `))`. Absorb any `)` immediately after the matched `))` so the closing
        // pair is the *last* in the run, mirroring the string replacer's
        // `(?!\))` look-ahead re-creation. `captures_iter` cannot skip ahead, so
        // the absorbed parens are folded into this match's `full` range instead;
        // a run of `)` never starts a new match, so the next `captures_iter`
        // match still lands past them.
        #[allow(clippy::unwrap_used)]
        let inner = caps.get(3).unwrap().as_str();

        let extra = s[whole.end()..].bytes().take_while(|b| *b == b')').count();
        let full = whole.start()..(whole.end() + extra);

        if let Some(m) = build_indexterm_shorthand_match(inner, extra, full, escaped, pieces, root)
        {
            matches.push(m);
        }
    }

    matches
}

/// Builds the [`MacroMatch`] for an index-term **macro** (`indexterm:[…]` /
/// `indexterm2:[…]`), or `None` when this increment defers it.
///
/// A concealed `indexterm:[…]` is always recognized (it renders nothing, so its
/// argument is never reconstructed). A visible `indexterm2:[…]` is deferred
/// when its argument crosses an opaque span (unreconstructable from the escaped
/// string) or carries an attribute list (an `=` the node cannot hold yet) – see
/// [`find_indexterm_matches`]. The visible term is normalized exactly as the
/// string replacer does ([`normalize_index_text`] with bracket-unescaping) and
/// baked into the node's `terms` in its already-substituted form, which is what
/// `fold_index_term` feeds back to `render_index_term` for byte-identical
/// output.
fn build_indexterm_macro_match<'src>(
    is_visible: bool,
    arg: &str,
    full: std::ops::Range<usize>,
    escaped: bool,
    pieces: &[Piece],
    root: Span<'src>,
) -> Option<RecognizedIndexterm<'src>> {
    // An escape (`\indexterm:…`) drops the backslash and keeps the rest literal,
    // mirroring the string replacer's `caps[0][1..]`. A macro form always
    // `Continue`s (no look-ahead retry), and the unescaped literal is non-empty.
    if escaped {
        return Some(RecognizedIndexterm {
            macro_match: MacroMatch {
                kind: MacroMatchKind::Unescape {
                    backslash: full.start,
                },
                full,
            },
            rendered_nonempty: true,
            is_skip: false,
        });
    }

    let location = source_slice(pieces, full.clone(), root);

    let (node, rendered_nonempty) = if is_visible {
        // A visible flow term crossing an opaque span cannot be reconstructed
        // from this level's escaped string (a span is a placeholder here, not
        // its markup), so it is deferred. A term crossing a
        // [`synthesized`](Piece::synthesized) run is *not*: the match string
        // carries such a run's bytes exactly, which is the only thing this
        // family ever reads a term from (see [`find_indexterm_matches`]).
        if arg.contains(SPAN_PLACEHOLDER) {
            return None;
        }

        let term = normalize_index_text(arg, true);

        // A term carrying an attribute list (an `=`) is parsed as an `Attrlist`
        // the node cannot hold yet; deferred exactly as the link/xref macros
        // defer their attribute-list text.
        if term.contains('=') {
            return None;
        }

        let rendered_nonempty = !term.is_empty();

        (
            InlineNode::IndexTerm(IndexTerm {
                terms: vec![CowStr::from(term)],
                visible: true,
                location,
            }),
            rendered_nonempty,
        )
    } else {
        // A concealed term renders nothing, so it is always recognized; its
        // argument (which never reaches the flow) is not reconstructed.
        (
            InlineNode::IndexTerm(IndexTerm {
                terms: vec![],
                visible: false,
                location,
            }),
            false,
        )
    };

    Some(RecognizedIndexterm {
        macro_match: MacroMatch {
            kind: MacroMatchKind::Node {
                consumed: full.clone(),
                node: Box::new(node),
            },
            full,
        },
        rendered_nonempty,
        // The macro forms always `Continue`; only the shorthand can retry.
        is_skip: false,
    })
}

/// Builds the [`MacroMatch`] for an index-term **shorthand** (`((term))`,
/// `(((primary, secondary, tertiary)))`), or `None` when the visible term
/// crosses an opaque span (see [`find_indexterm_matches`]).
///
/// `inner` is the text between the outermost `((` and `))` (match group 3);
/// `extra` is the count of `)` absorbed after the closing pair. Together they
/// form the string replacer's `encl_text`, whose leading/trailing parentheses
/// classify the term as concealed vs. visible and carry any literal parenthesis
/// adjacent to (but not part of) the term:
///
/// - `(((x)))` → **concealed** `x` (the node consumes the whole match and
///   renders nothing);
/// - a leading extra paren → **visible** term preceded by a literal `(`;
/// - a trailing extra paren → **visible** term followed by a literal `)`;
/// - `((x))` → **visible** `x`.
///
/// A single literal parenthesis kept beside the term is expressed by pointing
/// the node's `consumed` sub-range one byte inside the match, so
/// [`rebuild_macro_level`] emits that edge `(`/`)` as literal text – the same
/// sub-range mechanism the auto-link pass uses for a bare URL's kept prefix.
///
/// An **escaped** shorthand (`\((…))`) drops its backslash and stays literal
/// (an [`Unescape`](MacroMatchKind::Unescape)); the one string-replacer form
/// that instead re-renders an escaped *paren-wrapped* term (`\(((x)))` →
/// `(x)`) is left literal here, a documented divergence pinned by a test.
fn build_indexterm_shorthand_match<'src>(
    inner: &str,
    extra: usize,
    full: std::ops::Range<usize>,
    escaped: bool,
    pieces: &[Piece],
    root: Span<'src>,
) -> Option<RecognizedIndexterm<'src>> {
    // An escaped shorthand drops its backslash and keeps the rest (including any
    // absorbed parens) literal. The one form the string replacer treats
    // specially – an escaped, paren-wrapped `\(((x)))`, which it collapses to
    // `(x)` – is left literal here, a divergence documented and pinned by a
    // test. Every other escaped spelling matches the string replacer's plain
    // `caps[0][1..]` byte-for-byte.
    if escaped {
        return Some(RecognizedIndexterm {
            macro_match: MacroMatch {
                kind: MacroMatchKind::Unescape {
                    backslash: full.start,
                },
                full,
            },
            rendered_nonempty: true,
            is_skip: extra > 0,
        });
    }

    // `encl_text` = the enclosed text plus any absorbed trailing parens, exactly
    // as the string replacer builds it before classifying.
    let mut encl_text = inner.to_string();
    for _ in 0..extra {
        encl_text.push(')');
    }

    // Classify the term, mirroring the string replacer's paren stripping. `term`
    // is the inner text whose primary term is shown (visible) or indexed only
    // (concealed); `before`/`after` flag a single literal parenthesis kept
    // beside the term.
    let (term_src, visible, before, after): (&str, bool, bool, bool) =
        if let Some(without_open) = encl_text.strip_prefix('(') {
            if let Some(inner) = without_open.strip_suffix(')') {
                (inner, false, false, false) // `(((concealed)))`
            } else {
                (without_open, true, true, false) // visible, kept `(`
            }
        } else if let Some(inner) = encl_text.strip_suffix(')') {
            (inner, true, false, true) // visible, kept `)`
        } else {
            (encl_text.as_str(), true, false, false) // `((visible))`
        };

    let location = source_slice(pieces, full.clone(), root);

    let (node, rendered_nonempty) = if visible {
        // A visible term crossing an opaque span cannot be reconstructed from
        // the escaped string; defer it (see [`find_indexterm_matches`]). One
        // crossing a synthesized run is recognized, exactly as in the macro
        // form's own check above.
        if term_src.contains(SPAN_PLACEHOLDER) {
            return None;
        }

        let term = strip_see_and_seealso(&normalize_index_text(term_src, false));

        // The match renders output when it shows a non-empty term or keeps a
        // literal parenthesis beside it.
        let rendered_nonempty = before || after || !term.is_empty();

        (
            InlineNode::IndexTerm(IndexTerm {
                terms: vec![CowStr::from(term)],
                visible: true,
                location,
            }),
            rendered_nonempty,
        )
    } else {
        (
            InlineNode::IndexTerm(IndexTerm {
                terms: vec![],
                visible: false,
                location,
            }),
            false,
        )
    };

    // A kept literal parenthesis is left outside the node's `consumed` sub-range
    // so [`rebuild_macro_level`] emits it as literal text: a `before` keeps the
    // match's first `(`; an `after` keeps its last `)`.
    let consumed = (full.start + usize::from(before))..(full.end - usize::from(after));

    Some(RecognizedIndexterm {
        macro_match: MacroMatch {
            kind: MacroMatchKind::Node {
                consumed,
                node: Box::new(node),
            },
            full,
        },
        rendered_nonempty,
        is_skip: extra > 0,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use super::super::super::test_support::{assert_text, build_src, fold_html, golden_macros};
    use crate::{
        Span,
        inlines::{IndexTerm, InlineNode},
        parser::HtmlSubstitutionRenderer,
    };

    /// Asserts that `node` is an [`IndexTerm`](InlineNode::IndexTerm),
    /// returning it.
    fn assert_index_term<'a, 'src>(node: &'a InlineNode<'src>) -> &'a IndexTerm<'src> {
        match node {
            InlineNode::IndexTerm(index_term) => index_term,

            other => panic!("expected an IndexTerm, got {other:?}"),
        }
    }

    #[test]
    fn fold_matches_the_string_pipeline_through_index_terms() {
        // For each fixture, folding the single-pass tree (all five steps)
        // reproduces the string pipeline's output byte-for-byte. This is the
        // differential corpus (design §5.3) that pins the index-term increment.
        // A concealed term renders nothing, a visible term renders its shown
        // text; both spellings and the trailing-paren / see-also handling are
        // exercised.
        let fixtures = [
            // No index term despite paren/colon/bracket characters.
            "plain text without a term",
            "a single ( paren and a lone ) paren",
            "indexterm without a bracket stays literal",
            "a colon : and brackets [] but no term",
            // Macro form: concealed (no output) and visible (flow) primary.
            "indexterm:[coffee]",
            "indexterm:[coffee, robusta]",
            "indexterm2:[coffee]",
            "an indexterm2:[Coffee] in the flow",
            // Shorthand: visible `((…))` and concealed `(((…)))`.
            "((coffee))",
            "See ((coffee)) for more.",
            "(((coffee)))",
            "(((coffee, robusta, arabica)))",
            // Trailing-paren absorption: the closing pair is the *last* in a run,
            // so an extra `)` is kept as literal flow text after the term.
            "((coffee)))",
            // A leading extra paren is kept as literal flow text before the term.
            "(((coffee))",
            // A visible term with a `see` / `see-also` clause shows only its
            // primary; the `>>` / `&>` separators are `&gt;`/`&amp;` entities by
            // macro time, but the term is still reconstructed from the escaped
            // match string, so this is parity (not a divergence).
            "((Coffee >> Beans))",
            "((Coffee &> Tea))",
            // A concealed term whose inner crosses a rendered span still renders
            // nothing on both paths (the span markup is inside the discarded
            // term), so it is parity.
            "(((*bold* term)))",
            // Escapes: the term stays literal, minus the backslash.
            "\\indexterm:[x]",
            "\\indexterm2:[x]",
            "\\((coffee))",
            // Embedded in surrounding flow and next to other constructs.
            "*bold* then ((x)) here",
            "a copyright (C) then ((x))",
            "((a)) and ((b)) and indexterm:[c]",
            "((a))((b))",
            // Recognized inside a rendered span (the macros step descends).
            "*see ((x))*",
            "_indexterm2:[y] in em_",
        ];

        let renderer = HtmlSubstitutionRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_src(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden_macros(fixture),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn a_concealed_indexterm_macro_becomes_an_empty_node() {
        let nodes = build_src(Span::new("indexterm:[coffee, robusta]"));

        assert_eq!(nodes.len(), 1);
        let index_term = assert_index_term(&nodes[0]);

        // A concealed term carries no shown text and renders nothing.
        assert!(!index_term.visible);
        assert!(index_term.terms.is_empty());

        // Its location covers the whole macro.
        assert_eq!(index_term.location.data(), "indexterm:[coffee, robusta]");
        assert_eq!(index_term.location.line(), 1);
        assert_eq!(index_term.location.col(), 1);
    }

    #[test]
    fn a_visible_indexterm_macro_becomes_a_flow_node() {
        let nodes = build_src(Span::new("indexterm2:[coffee]"));

        let index_term = assert_index_term(&nodes[0]);

        assert!(index_term.visible);
        assert_eq!(index_term.terms.len(), 1);
        assert_eq!(index_term.terms[0].as_ref(), "coffee");
        assert_eq!(index_term.location.data(), "indexterm2:[coffee]");
    }

    #[test]
    fn a_visible_shorthand_becomes_a_flow_node_with_precise_span() {
        // Embedded so the node's location is not at column 1.
        let nodes = build_src(Span::new("x ((coffee)) y"));

        // `x ` then the term then ` y`.
        assert_eq!(nodes.len(), 3);
        assert_text(&nodes[0], "x ", 1, 1);

        let index_term = assert_index_term(&nodes[1]);
        assert!(index_term.visible);
        assert_eq!(index_term.terms[0].as_ref(), "coffee");

        // The location covers the whole `((coffee))`, the delimiters included,
        // starting at column 3.
        assert_eq!(index_term.location.data(), "((coffee))");
        assert_eq!(index_term.location.col(), 3);

        assert_text(&nodes[2], " y", 1, 13);
    }

    #[test]
    fn a_concealed_shorthand_becomes_an_empty_node() {
        // Embedded after literal text so the level is not the all-concealed
        // no-op the string replacer leaves untouched (see the parity test
        // below); the concealed term is then recognized as an empty node.
        let nodes = build_src(Span::new("x (((coffee, robusta)))"));

        assert_eq!(nodes.len(), 2);
        assert_text(&nodes[0], "x ", 1, 1);

        let index_term = assert_index_term(&nodes[1]);
        assert!(!index_term.visible);
        assert!(index_term.terms.is_empty());
        assert_eq!(index_term.location.data(), "(((coffee, robusta)))");
        assert_eq!(index_term.location.col(), 3);
    }

    #[test]
    fn an_all_concealed_shorthand_level_stays_literal() {
        // A level whose only *output* would be from concealed shorthand terms
        // accumulates no output and ends in a look-ahead retry, so the string
        // replacer returns `Cow::Borrowed` and leaves it literal (the #1123
        // bug). The builder mirrors that byte-for-byte: the terms are left
        // unrecognized (no `IndexTerm` node).
        //
        // Note the `(((coffee))) trailing` case: **trailing** text does *not*
        // rescue recognition, because it is appended only on the replacer's
        // normal completion — the `Cow::Borrowed` early-return on the empty-`new`
        // retry happens first and discards it. Only output emitted *before* the
        // retry (a leading/between gap, or a shown term) makes `new` non-empty;
        // see `a_concealed_term_after_leading_output_is_consumed` for that
        // contrast. (Both mirror the string pipeline exactly — verified below.)
        for source in [
            "(((coffee)))",
            "(((a)))(((b)))",
            "indexterm:[x](((y)))",
            "(((coffee))) trailing",
        ] {
            let nodes = build_src(Span::new(source));

            assert!(
                nodes.iter().all(|n| !matches!(n, InlineNode::IndexTerm(_))),
                "an all-concealed level must be left literal for {source:?}: {nodes:?}"
            );

            let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});
            assert_eq!(folded, source, "left-literal fold for {source:?}");
            assert_eq!(
                golden_macros(source),
                source,
                "golden literal for {source:?}"
            );
        }
    }

    #[test]
    fn a_concealed_term_after_leading_output_is_consumed() {
        // The contrast to the all-concealed no-op: **leading** text makes the
        // replacer's accumulator non-empty *before* the look-ahead retry, so the
        // substitution is `Cow::Owned` and the concealed term is recognized and
        // consumed (renders nothing) — leaving only the leading text. The builder
        // reproduces this byte-for-byte, so `leading (((coffee)))` folds to
        // `leading `, not the literal source.
        for source in ["leading (((coffee)))", "((a)) (((b)))"] {
            let folded = fold_html(&build_src(Span::new(source)), &HtmlSubstitutionRenderer {});
            assert_eq!(
                folded,
                golden_macros(source),
                "fold diverged from the string pipeline for {source:?}"
            );
            assert_ne!(folded, source, "the term should be consumed for {source:?}");
        }
    }

    #[test]
    fn a_see_clause_leaves_only_the_primary_term() {
        // The `see` separator (` >> `) is `&gt;&gt;` by macro time; the term is
        // reconstructed from the escaped match string, so the node carries only
        // the primary `Coffee`, exactly as the string replacer renders.
        let nodes = build_src(Span::new("((Coffee >> Beans))"));

        let index_term = assert_index_term(&nodes[0]);
        assert!(index_term.visible);
        assert_eq!(index_term.terms[0].as_ref(), "Coffee");
    }

    #[test]
    fn a_kept_paren_rides_beside_the_term_as_literal_text() {
        // A leading extra paren is kept as flow text before the visible term.
        let nodes = build_src(Span::new("(((coffee))"));

        // A literal `(` then the term node.
        assert_eq!(nodes.len(), 2);
        assert_text(&nodes[0], "(", 1, 1);

        let index_term = assert_index_term(&nodes[1]);
        assert!(index_term.visible);
        assert_eq!(index_term.terms[0].as_ref(), "coffee");
    }

    #[test]
    fn an_index_term_is_recognized_inside_a_span() {
        // The macros step descends into a `Styled` span's children, so a term
        // inside a rendered span becomes an `IndexTerm` there.
        let nodes = build_src(Span::new("*a ((b)) c*"));

        assert_eq!(nodes.len(), 1);

        match &nodes[0] {
            InlineNode::Styled(styled) => {
                assert!(
                    styled
                        .children
                        .iter()
                        .any(|n| matches!(n, InlineNode::IndexTerm(_))),
                    "expected an IndexTerm inside the span, got {:?}",
                    styled.children
                );
            }

            other => panic!("expected a Styled span, got {other:?}"),
        }
    }

    #[test]
    fn a_visible_term_over_a_span_is_a_documented_divergence() {
        // A *visible* term whose shown text crosses a rendered span (`*bold*`
        // became a `Styled` placeholder by macro time) cannot be reconstructed
        // from this level's escaped match string, so the builder leaves it
        // unrecognized – the `((` / `))` stay literal around the span. The string
        // pipeline, by contrast, folds the span markup into the shown term and
        // consumes the delimiters.
        let source = "((*bold* term))";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::IndexTerm(_))),
            "a visible term crossing a span must be left unrecognized: {nodes:?}"
        );

        // The delimiters survive literally in the builder's output, but the
        // string pipeline consumes them.
        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});
        assert!(folded.contains("(("));
        assert!(!golden_macros(source).contains("(("));
    }

    #[test]
    fn a_visible_macro_term_over_a_span_is_a_documented_divergence() {
        // The same span boundary for the *macro* spelling: an `indexterm2:[…]`
        // whose shown text crosses a rendered span is left unrecognized (the
        // `indexterm2:[` stays literal), where the string pipeline folds the span
        // markup into the shown term and consumes the macro.
        let source = "indexterm2:[*bold* term]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::IndexTerm(_))),
            "a visible macro term crossing a span must be left unrecognized: {nodes:?}"
        );

        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});
        assert!(folded.contains("indexterm2:["));
        assert_eq!(golden_macros(source), "<strong>bold</strong> term");
    }

    #[test]
    fn an_indexterm2_attribute_list_is_a_documented_divergence() {
        // An `indexterm2:[…]` argument carrying an `=` splits into an attribute
        // list whose first positional attribute is the shown term. The builder
        // cannot carry that as an `Attrlist<'src>` yet, so it defers the whole
        // macro (left literal), exactly as the link/xref macros defer their
        // attribute-list text.
        let source = "indexterm2:[Coffee, region=Kona]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::IndexTerm(_))),
            "an attribute-list-in-text index term must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, shows the first positional term.
        assert_eq!(golden_macros(source), "Coffee");
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_index_terms_inside_expanded_values() {
        // A term whose shown text crosses a *synthesized* run (an attribute
        // expansion) is now recognized: this family reads a term from the
        // match string alone – which carries such a run's bytes exactly – and
        // an [`IndexTerm`] node carries no `Span`-typed field, so nothing on
        // it needs an `'src` slice. The same lift the anchor and bare-e-mail
        // families already made; it closes the two divergences
        // `a_visible_term_inside_an_expanded_value_is_a_documented_divergence`
        // and its `indexterm2:` twin used to pin.
        use crate::{
            Parser,
            content::{Content, SubstitutionGroup, inline_builder::build},
            parser::{HtmlSubstitutionRenderer, ModificationContext},
        };

        let parser = Parser::default()
            .with_intrinsic_attribute("term", "coffee", ModificationContext::Anywhere)
            .with_intrinsic_attribute("second", "brewing", ModificationContext::Anywhere)
            .with_intrinsic_attribute("shorthand", "((tea))", ModificationContext::Anywhere);

        let fixtures = [
            // The visible shorthand and macro spellings, whole and partial.
            "x (({term})) y",
            "x ((hot {term})) y",
            "x indexterm2:[{term}] y",
            "x indexterm2:[hot {term}] y",
            // The concealed spellings (always recognized, but now over an
            // expanded value too).
            "x ((({term}, {second}))) y",
            "x indexterm:[{term}, {second}] y",
            // The whole construct arriving from an expanded value.
            "x {shorthand} y",
            // A kept literal parenthesis beside an expanded term.
            "x (((({term}))) y",
        ];

        for source in fixtures {
            let nodes = build(Span::new(source), &parser, None);

            let mut golden = Content::from(Span::new(source));
            SubstitutionGroup::Normal.apply(&mut golden, &parser, None);

            assert_eq!(
                crate::content::inline_builder::fold_html(
                    &nodes,
                    &HtmlSubstitutionRenderer {},
                    &parser
                ),
                golden.rendered_str(),
                "fold diverged from the string pipeline for {source:?}"
            );
        }
    }

    #[test]
    fn a_term_inside_an_expanded_value_keeps_a_coarse_location() {
        // The shown term is exact – and necessarily owned, since an expanded
        // value's bytes have no `'src` counterpart – while the node's
        // `location` falls back to the whole match's source span (design
        // §4.4), exactly as an anchor's does.
        use crate::{
            Parser, content::inline_builder::build, parser::ModificationContext, strings::CowStr,
        };

        let parser = Parser::default().with_intrinsic_attribute(
            "term",
            "coffee",
            ModificationContext::Anywhere,
        );

        let source = "x (({term})) y";
        let nodes = build(Span::new(source), &parser, None);

        let term = nodes
            .iter()
            .find_map(|n| match n {
                InlineNode::IndexTerm(term) => Some(term),
                _ => None,
            })
            .unwrap_or_else(|| panic!("expected an IndexTerm node: {nodes:?}"));

        assert!(term.visible);
        assert_eq!(term.terms.len(), 1);
        assert_eq!(term.terms[0].as_ref(), "coffee");
        assert!(matches!(term.terms[0], CowStr::Boxed(_)), "{term:?}");

        assert_eq!(term.location.data(), "(({term}))");
        assert_eq!(term.location.line(), 1);
        assert_eq!(term.location.col(), 3);
    }

    #[test]
    fn an_escaped_paren_wrapped_shorthand_is_a_documented_divergence() {
        // The one escaped shorthand the string replacer re-renders rather than
        // leaving literal: an escaped, paren-wrapped `\(((x)))`, which it
        // collapses to `(x)`. The builder drops the backslash and keeps the rest
        // literal (`(((x)))`), a documented divergence – every other escaped
        // spelling matches byte-for-byte (pinned by the corpus above).
        let source = "\\(((x)))";
        let folded = fold_html(&build_src(Span::new(source)), &HtmlSubstitutionRenderer {});

        assert_eq!(folded, "(((x)))");
        assert_eq!(golden_macros(source), "(x)");
    }
}
