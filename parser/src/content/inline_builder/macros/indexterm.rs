//! Index-term recognition (`((term))`, `(((primary, secondary)))`,
//! `indexterm:[…]`, `indexterm2:[…]`).

use super::{MacroMatch, MacroMatchKind, rebuild_macro_level};
use crate::{
    Parser, Span,
    attributes::{Attrlist, AttrlistContext},
    content::{
        INLINE_INDEXTERM,
        inline_builder::{
            quotes::{LevelContext, Piece, SPAN_PLACEHOLDER, build_match_string, source_slice},
            special_chars::Masked,
        },
        normalize_index_text, strip_see_and_seealso,
    },
    inlines::{IndexTerm, InlineNode},
    strings::CowStr,
};

/// Matches [`INLINE_INDEXTERM`] at this level's escaped text, replacing each
/// recognized index term — the `((term))` / `(((primary, secondary,
/// tertiary)))` shorthand and the `indexterm:[…]` / `indexterm2:[…]` macro —
/// with the [`IndexTerm`](InlineNode::IndexTerm) node it produces and leaving
/// everything else in place.
pub(super) fn indexterm_macros_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
    ctx: LevelContext,
    masked: Masked<'_>,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes, masked);

    // Cheap pre-filter mirroring the string step's guard: a shorthand needs a
    // `((` … `))` pair (its parens are not special, so they reach the macros
    // step intact), and a macro form needs a `:[` and `dexterm` (matching both
    // `indexterm:` and `indexterm2:`).
    if !((s.contains("((") && s.contains("))")) || (s.contains(":[") && s.contains("dexterm"))) {
        return nodes;
    }

    // Matched over the level wrapped in the boundary character its enclosing
    // construct presents, with the level's own pieces moved into that string's
    // coordinates — see `apply_macro_families`'s own doc comment.
    let (s, pieces) = ctx.shift(s, pieces);

    let matches = find_indexterm_matches(&s, &pieces, root, parser);

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
    /// look-ahead retry (`SkipAheadAndRetry`) — true only for a shorthand that
    /// absorbed trailing parens. The macro forms always `Continue`.
    is_skip: bool,
}

/// Reports whether the string replacer's index-term substitution over this
/// level would be a **no-op** — returning `Cow::Borrowed` and so leaving the
/// content unchanged (see [`indexterm_macros_level`]).
///
/// That happens exactly when the accumulated output is empty *and* the last
/// recognized match advanced via a look-ahead retry: an empty gap before every
/// match, every match rendering nothing, and the final match a paren-absorbing
/// shorthand. Any non-empty gap or shown term — or a trailing macro form, which
/// `Continue`s instead of retrying — makes the substitution `Cow::Owned` and
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

/// Finds every recognized index term at this level — both the macro forms
/// (`indexterm:[…]`, `indexterm2:[…]`) and the shorthand forms (`((term))`,
/// `(((primary, secondary, tertiary)))`) — as a [`MacroMatch`].
///
/// # Concealed vs. visible, and the recognition boundary
///
/// A **concealed** term (`indexterm:[…]`, `(((…)))`) renders to *nothing* —
/// [`render_index_term`](crate::parser::InlineSubstitutionRenderer::render_index_term) emits
/// no output for it — so, much like an inline anchor whose output is a function
/// of its id alone (see
/// [`find_anchor_matches`](super::anchors::find_anchor_matches)), it is
/// recognized regardless of what its argument crosses; the node simply carries
/// an empty `terms`. (The one exception is the string replacer's
/// whole-substitution no-op for a level of *only* concealed shorthand terms,
/// which [`indexterm_macros_level`] mirrors by leaving the level literal.)
///
/// A **visible** (flow) term (`indexterm2:[…]`, `((term))`) shows its term text
/// in the flow, so that text must be reconstructible from this level's escaped
/// match string. It is — with the *same* entity bytes the string pipeline sees,
/// since a [`CharRef`](InlineNode::CharRef) contributes its canonical entity to
/// the match string — whenever the term crosses no *opaque span* (an
/// earlier-recognized
/// [`Styled`](crate::inlines::Styled)/[`Ref`](crate::inlines::Ref), carried
/// here as a single [`SPAN_PLACEHOLDER`] rather than its rendered markup). A
/// visible term crossing such a span is **deferred** — the match is left as
/// literal source for a later increment, pinned by a divergence test. An
/// `indexterm2:[…]` term carrying an attribute list (an `=`, whose first
/// positional attribute becomes the shown term) is *not*: the list decides
/// only which of the argument's own bytes are shown, so
/// [`shown_macro_term`] consumes it here and the node carries the same
/// already-substituted text every other visible spelling gives it.
///
/// A term crossing a [`synthesized`](Piece::synthesized) run (an attribute
/// expansion, or — reached at a tree's root — a filtered multi-line block's
/// own joined seed) is **not** deferred. The match string carries such a run's
/// bytes exactly, and this family reads its term from nowhere else — it never
/// slices `'src`, and an [`IndexTerm`] node carries no `Span`-typed field — so
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
    parser: &Parser,
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
                build_indexterm_macro_match(is_visible, arg, full, escaped, pieces, root, parser)
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

        push_indexterm_shorthand_matches(inner, extra, full, escaped, pieces, root, &mut matches);
    }

    matches
}

/// Builds the [`MacroMatch`] for an index-term **macro** (`indexterm:[…]` /
/// `indexterm2:[…]`), or `None` when this increment defers it.
///
/// A concealed `indexterm:[…]` is always recognized (it renders nothing, so its
/// argument is never reconstructed). A visible `indexterm2:[…]` is deferred
/// only when its argument crosses an opaque span (unreconstructable from the
/// escaped string) — see [`find_indexterm_matches`]. The visible term is
/// normalized exactly as the string replacer does ([`normalize_index_text`]
/// with bracket-unescaping), reduced through [`shown_macro_term`] when the
/// argument is an attribute list, and baked into the node's `terms` in its
/// already-substituted form, which is what `fold_index_term` feeds back to
/// `render_index_term` for byte-identical output.
fn build_indexterm_macro_match<'src>(
    is_visible: bool,
    arg: &str,
    full: std::ops::Range<usize>,
    escaped: bool,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
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

        let term = shown_macro_term(normalize_index_text(arg, true), parser);

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

/// The text an `indexterm2:[…]` shows in the flow, given its already
/// [`normalize_index_text`]-ed argument.
///
/// An argument carrying an `=` is an **attribute list**, whose first
/// *positional* attribute is the shown term (`indexterm2:[Coffee,
/// region=Kona]` shows `Coffee`); everything else it holds — a `see`, a
/// `see-also`, a `region` — names the entry in an index this crate's HTML
/// backend does not build, so it reaches the flow through nothing. Where the
/// list has no positional attribute at all (`indexterm2:[see=HTML 5]`), the
/// whole argument is shown verbatim, which is also the answer for an `=` that
/// was never an attribute list to begin with.
///
/// This is [`InlineIndextermReplacer`]'s own arithmetic, reached with the same
/// bytes: the argument is read from this level's escaped match string, which
/// holds exactly what the string pipeline's flat haystack holds at that
/// position — and the caller has already refused an argument crossing an
/// opaque span, the one case where the two differ.
///
/// # Why the node needs no `Attrlist`
///
/// The link and cross-reference families capture the
/// [`Attrlist<'src>`](Attrlist) their own bracket parses, because a role, an
/// id, or a `window=` there changes what the fold emits. An index term's whole
/// render surface is
/// [`IndexTermRenderParams`](crate::parser::IndexTermRenderParams), which
/// carries the shown term and nothing else — so the list is *consumed* here
/// rather than carried, and the [`IndexTerm`] node goes on holding the same
/// already-substituted shown text every other visible spelling gives it. That
/// is what the deferral this closes was waiting on, and the answer is that it
/// was waiting on nothing.
///
/// [`InlineIndextermReplacer`]: crate::content::macros
fn shown_macro_term(arg: String, parser: &Parser) -> String {
    if !arg.contains('=') {
        return arg;
    }

    // Parsed over the normalized copy, exactly as the string replacer parses
    // its own (`Attrlist::parse(Span::new(&arg), …)`): the value read back out
    // is owned, so nothing borrows `'src` and the node keeps the coarse
    // whole-match location design §4.4 gives a computed value.
    let attrlist = Attrlist::parse(Span::new(&arg), parser, AttrlistContext::Inline)
        .item
        .item;

    let positional = attrlist.nth_attribute(1).map(|a| a.value().to_string());

    positional.unwrap_or(arg)
}

/// Pushes the [`MacroMatch`]es for an index-term **shorthand** (`((term))`,
/// `(((primary, secondary, tertiary)))`) — none at all when the visible term
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
/// [`rebuild_macro_level`] emits that edge `(`/`)` as literal text — the same
/// sub-range mechanism the auto-link pass uses for a bare URL's kept prefix.
///
/// # The two escaped spellings
///
/// An **escaped** shorthand (`\((…))`) drops its backslash and keeps the rest —
/// absorbed parens included — literal, an
/// [`Unescape`](MacroMatchKind::Unescape) matching the string replacer's plain
/// `caps[0][1..]` byte-for-byte.
///
/// An escaped **paren-wrapped** one (`\(((x)))`) is the exception, and the
/// replacer's own branch says why: *an escaped concealed term still processes a
/// nested flow term*. It strips the wrapping parentheses off `encl_text` and
/// renders what is left as a **visible** term between two literal parens, so
/// `\(((x)))` collapses to `(x)` rather than staying literal. That is one
/// match's source doing two things, which is exactly what a **pair** of matches
/// expresses — an [`Unescape`](MacroMatchKind::Unescape) over the backslash
/// alone, then a [`Node`](MacroMatchKind::Node) over the rest with a kept paren
/// at *each* end of its `consumed` sub-range — and
/// [`rebuild_macro_level`] composes the pair as it composes any two adjacent
/// matches, so neither [`MacroMatchKind`] variant grows a new shape. It is the
/// same composition the escaped-attribute-list bracket
/// ([`passthrough_step`](super::super::passthrough_step)) reaches from the
/// other direction; only the kept-paren narrowing is this family's own, and it
/// is the narrowing the unescaped spellings above already use, applied to both
/// ends at once.
fn push_indexterm_shorthand_matches<'src>(
    inner: &str,
    extra: usize,
    full: std::ops::Range<usize>,
    escaped: bool,
    pieces: &[Piece],
    root: Span<'src>,
    matches: &mut Vec<RecognizedIndexterm<'src>>,
) {
    // `encl_text` = the enclosed text plus any absorbed trailing parens, exactly
    // as the string replacer builds it before classifying.
    let mut encl_text = inner.to_string();
    for _ in 0..extra {
        encl_text.push(')');
    }

    if escaped {
        // The escaped branch's own classification: a paren-wrapped `encl_text`
        // is the nested flow term the replacer still renders (see this
        // function's doc comment); anything else stays literal.
        let nested = encl_text
            .strip_prefix('(')
            .and_then(|t| t.strip_suffix(')'))
            // A shown term crossing an opaque span is unreconstructable from
            // this level's escaped string, so the family's own recognition
            // boundary decides first and the match keeps the literal shape —
            // the same deferral every other visible spelling takes, pinned by
            // its own divergence test.
            .filter(|nested| !nested.contains(SPAN_PLACEHOLDER));

        let Some(nested) = nested else {
            matches.push(RecognizedIndexterm {
                macro_match: MacroMatch {
                    kind: MacroMatchKind::Unescape {
                        backslash: full.start,
                    },
                    full,
                },
                rendered_nonempty: true,
                is_skip: extra > 0,
            });

            return;
        };

        // The backslash alone. An `Unescape` whose match *is* the character it
        // drops emits nothing, which is what the replacer does with this one.
        matches.push(RecognizedIndexterm {
            macro_match: MacroMatch {
                kind: MacroMatchKind::Unescape {
                    backslash: full.start,
                },
                full: full.start..(full.start + 1),
            },
            rendered_nonempty: false,
            is_skip: false,
        });

        // Everything after it: the nested term, with the wrapping parenthesis at
        // each end left outside `consumed` so `rebuild_macro_level` emits both
        // as the literal text the replacer pushes around the rendered term.
        let term_full = (full.start + 1)..full.end;
        let consumed = (term_full.start + 1)..(term_full.end - 1);

        let node = InlineNode::IndexTerm(IndexTerm {
            terms: vec![CowStr::from(strip_see_and_seealso(&normalize_index_text(
                nested, false,
            )))],
            visible: true,
            location: source_slice(pieces, term_full.clone(), root),
        });

        matches.push(RecognizedIndexterm {
            macro_match: MacroMatch {
                kind: MacroMatchKind::Node {
                    consumed,
                    node: Box::new(node),
                },
                full: term_full,
            },
            // The two kept parentheses are output whatever the term renders to.
            rendered_nonempty: true,
            is_skip: extra > 0,
        });

        return;
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
            return;
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

    matches.push(RecognizedIndexterm {
        macro_match: MacroMatch {
            kind: MacroMatchKind::Node {
                consumed,
                node: Box::new(node),
            },
            full,
        },
        rendered_nonempty,
        is_skip: extra > 0,
    });
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
            // A visible term crossing a *typographic replacement* — the third
            // recoverable piece, whose match-string bytes are the entity the
            // built-in backend renders it as, so the shown text is
            // reconstructed exactly as the string replacer computes it.
            "((Coffee (C) Beans))",
            "((O'Reilly))",
            "an indexterm2:[Tom (C) Jerry] in the flow",
            "(((Coffee (C) Beans, robusta)))",
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
        // unrecognized — the `((` / `))` stay literal around the span. The string
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
    fn fold_matches_the_string_pipeline_through_indexterm2_attribute_lists() {
        // An `indexterm2:[…]` argument carrying an `=` splits into an
        // attribute list whose first *positional* attribute is the shown term;
        // everything else it holds names an entry in an index the HTML backend
        // does not build, so it reaches the flow through nothing. The builder
        // reads the argument from this level's escaped match string — the same
        // bytes the string pipeline's flat haystack holds there — and runs the
        // replacer's own arithmetic over them ([`shown_macro_term`]).
        for fixture in [
            // The shown term is the first positional attribute.
            "indexterm2:[Coffee, region=Kona]",
            "a indexterm2:[Coffee, region=Kona] term",
            // The two golden spellings of the `see` / `see-also` named
            // attributes, which the macro form carries as attributes rather
            // than as the shorthand's ` >> ` / ` &> ` clause.
            "indexterm2:[Flash,see=HTML 5] and indexterm2:[HTML 5,see-also=\"CSS 3, SVG\"] done.",
            // No positional attribute at all: the whole argument is shown
            // verbatim, `=` included.
            "Only named indexterm2:[see=HTML 5] here.",
            "indexterm2:[region=Kona]",
            // A quoted positional attribute, whose own comma is not a
            // separator.
            "indexterm2:[\"Coffee, Kona\",region=x]",
            // An empty first positional attribute, which shows nothing.
            "x indexterm2:[,region=Kona] y",
            // A special character in either half: by macro time it is an
            // entity in both pipelines, so the attribute list is parsed from
            // the same escaped bytes on both sides.
            "indexterm2:[a < b,region=Kona]",
            "indexterm2:[Coffee,region=a < b]",
            "indexterm2:[a &copy; b,region=Kona]",
            // A typographic replacement inside the shown half — the third
            // recoverable piece, whose match-string bytes are the entity the
            // built-in backend renders it as.
            "indexterm2:[Tom (C) Jerry,region=Kona]",
            "indexterm2:[O'Reilly,see=Books]",
            // In flow beside other constructs, twice in one flow, and inside a
            // rendered span (the macros step descends into one).
            "*bold* then indexterm2:[Coffee,region=Kona] here",
            "indexterm2:[a,x=1] and indexterm2:[b,y=2]",
            "_indexterm2:[Coffee,region=Kona] in em_",
            // Spanning a newline, which `normalize_index_text` collapses to a
            // space before either side parses the list.
            "indexterm2:[Coffee,\nregion=Kona] end",
            // An escaped attribute-list form still drops its backslash and
            // stays literal.
            "\\indexterm2:[Coffee,region=Kona]",
            // The concealed spelling ignores its argument entirely, `=` or
            // not.
            "x indexterm:[Coffee,region=Kona] y",
        ] {
            assert_eq!(
                fold_html(&build_src(Span::new(fixture)), &HtmlSubstitutionRenderer {}),
                golden_macros(fixture),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn an_indexterm2_attribute_list_becomes_a_node_carrying_only_its_shown_term() {
        // The shape the increment asserts: the attribute list is *consumed*
        // rather than carried. An index term's whole render surface is its
        // shown term, so the node holds the same already-substituted text
        // every other visible spelling gives it — and no `Attrlist`, which is
        // what the deferral this closes had been waiting on.
        let source = "indexterm2:[Coffee, region=Kona]";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        let index_term = assert_index_term(&nodes[0]);

        assert!(index_term.visible);
        assert_eq!(index_term.terms.len(), 1);
        assert_eq!(index_term.terms[0].as_ref(), "Coffee");

        // The location covers the whole macro, list included.
        assert_eq!(index_term.location.data(), source);
        assert_eq!(index_term.location.line(), 1);
        assert_eq!(index_term.location.col(), 1);

        // With no positional attribute the whole argument is the shown term.
        let nodes = build_src(Span::new("indexterm2:[see=HTML 5]"));
        let index_term = assert_index_term(&nodes[0]);

        assert!(index_term.visible);
        assert_eq!(index_term.terms[0].as_ref(), "see=HTML 5");
    }

    #[test]
    fn a_real_documents_indexterm2_attribute_lists_fold_to_their_rendered_strings() {
        // End-to-end, through the real parse path, on the shape that named
        // this increment: an `indexterm2:[…]` whose argument is an attribute
        // list must show the same term the rendered string carries — in a
        // heading's own title as well as in block content — so a tree that
        // left the macro literal would regress the moment `rendered_html()`
        // becomes a fold of this tree.
        use crate::blocks::{FindBlocks, IsBlock};

        let doc = crate::Parser::default()
            .with_inline_tree(true)
            .parse(concat!(
                "== A indexterm2:[Coffee,region=Kona] heading\n",
                "\n",
                "indexterm2:[Flash,see=HTML 5] has been supplanted by\n",
                "indexterm2:[HTML 5,see-also=\"CSS 3, SVG\"].\n",
                "\n",
                "Only named indexterm2:[see=HTML 5] here.\n",
            ));

        let mut folded_blocks = 0;

        for block in doc.descendant_blocks() {
            let (Some(rendered), Some(inlines)) = (block.rendered_html_content(), block.inlines())
            else {
                continue;
            };

            assert_eq!(
                crate::content::inline_builder::fold_html(
                    inlines,
                    &HtmlSubstitutionRenderer {},
                    &crate::Parser::default()
                ),
                rendered,
                "fold diverged from the rendered string for {inlines:?}"
            );

            folded_blocks += 1;
        }

        assert_eq!(folded_blocks, 2, "expected every paragraph to carry a tree");

        // The heading's own title takes the same seam (its `Title` group runs
        // the same macros step), so the shown term reaches a section title
        // too, and that title's own tree folds to the bytes it rendered.
        let mut folded_titles = 0;

        for block in doc.descendant_blocks() {
            let crate::blocks::Block::Section(section) = block else {
                continue;
            };

            assert_eq!(section.section_title(), "A Coffee heading");

            assert_eq!(
                crate::content::inline_builder::fold_html(
                    section.section_title_inlines(),
                    &HtmlSubstitutionRenderer {},
                    &crate::Parser::default()
                ),
                section.section_title(),
                "fold diverged from the rendered section title"
            );

            folded_titles += 1;
        }

        assert_eq!(folded_titles, 1, "expected the heading to carry a tree");
    }

    #[test]
    fn an_attribute_list_term_over_a_span_is_a_documented_divergence() {
        // The recognition boundary the attribute list does *not* move: a shown
        // term crossing a rendered span is unreconstructable from this level's
        // escaped match string (the span is one placeholder here, not its
        // markup), so the macro is still deferred — the `=` half is decided
        // only once the argument's own bytes are in hand.
        let source = "indexterm2:[*bold* term,region=Kona]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::IndexTerm(_))),
            "a term crossing a span must be left unrecognized: {nodes:?}"
        );

        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});
        assert!(folded.contains("indexterm2:["));
        assert_eq!(golden_macros(source), "<strong>bold</strong> term");
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_index_terms_inside_expanded_values() {
        // A term whose shown text crosses a *synthesized* run (an attribute
        // expansion) is now recognized: this family reads a term from the
        // match string alone — which carries such a run's bytes exactly — and
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
            .with_intrinsic_attribute("shorthand", "((tea))", ModificationContext::Anywhere)
            .with_intrinsic_attribute("escaped", "\\(((tea)))", ModificationContext::Anywhere);

        let fixtures = [
            // The visible shorthand and macro spellings, whole and partial.
            "x (({term})) y",
            "x ((hot {term})) y",
            "x indexterm2:[{term}] y",
            "x indexterm2:[hot {term}] y",
            // The macro spelling's attribute list, whose shown term arrives
            // from an expanded value — and whose *named* half does too, so the
            // list is parsed from the synthesized run's own bytes on both
            // sides.
            "x indexterm2:[{term},region={second}] y",
            "x indexterm2:[region={second}] y",
            // The concealed spellings (always recognized, but now over an
            // expanded value too).
            "x ((({term}, {second}))) y",
            "x indexterm:[{term}, {second}] y",
            // The whole construct arriving from an expanded value.
            "x {shorthand} y",
            // A kept literal parenthesis beside an expanded term.
            "x (((({term}))) y",
            // The escaped paren-wrapped spelling, whose nested term is the same
            // shown text read from the same synthesized run — the two kept
            // parens are the level's own bytes either side of it.
            "x \\((({term}))) y",
            // The same spelling arriving *whole* from an expanded value, so the
            // two kept parens are themselves sliced out of the synthesized run
            // (as is the backslash the pair's first match drops).
            "x {escaped} y",
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
        // The shown term is exact — and necessarily owned, since an expanded
        // value's bytes have no `'src` counterpart — while the node's
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
    fn fold_matches_the_string_pipeline_through_escaped_paren_wrapped_shorthands() {
        // The one escaped shorthand the string replacer re-renders rather than
        // leaving literal: a paren-wrapped `\(((x)))`, whose wrapping parens it
        // strips off `encl_text` before rendering what is left as a **visible**
        // term between two literal parens ("an escaped concealed term still
        // processes a nested flow term"), so the whole thing collapses to
        // `(x)`. The builder expresses that as a pair of matches — an
        // `Unescape` over the backslash alone, then a `Node` whose `consumed`
        // sub-range keeps a paren at each end — and folds to the same bytes.
        for fixture in [
            // The shape itself, alone and in flow.
            r"\(((x)))",
            r"a \(((x))) b",
            r"\(((a, b, c)))",
            // The nested term takes the *visible* branch's whole arithmetic: a
            // `see` / `see-also` clause is stripped to the primary term (the
            // separators are `&gt;&gt;` / `&amp;&gt;` entities by macro time)…
            r"\(((Coffee >> Beans)))",
            r"\(((Coffee &> Tea)))",
            // …a newline is collapsed to a space and the term is trimmed…
            "\\(((multi\nline)))",
            r"\((( spaced )))",
            // …and a special character, a restored entity, and a typographic
            // replacement all reach the shown text as the same entity bytes the
            // string pipeline holds there.
            r"\(((a < b)))",
            r"\(((a &copy; b)))",
            r"\(((Tom (C) Jerry)))",
            r"\(((O'Reilly)))",
            // An empty nested term shows nothing between the kept parens.
            r"\((()))",
            r"\((( )))",
            // The trailing-paren absorption still runs first, so what is
            // wrapped can itself carry parens — the extra `)` that the closing
            // pair absorbed is the wrapper's own right half.
            r"\(((x))))",
            r"\((((x))))",
            r"\(((x)))))",
            r"\((((x)))",
            // Beside its own literal-staying twins, whose `encl_text` is not
            // paren-wrapped and which therefore only drop a backslash.
            r"\((x))\(((y)))",
            r"\indexterm:[x]\(((y)))",
            // Twice in one flow, and beside constructs the earlier steps
            // recognized.
            r"\(((x))) \(((y)))",
            r"*bold* \(((x))) _em_",
            // Recognized inside a rendered span (the macros step descends).
            r"_\(((x))) in em_",
            // The shown term makes the substitution's output non-empty, so a
            // concealed term beside it is consumed rather than left literal by
            // the `Cow::Borrowed` no-op the level would otherwise be.
            r"\(((x)))(((y)))",
            r"\(((x))) and (((y)))",
            r"(((y))) and \(((x)))",
        ] {
            assert_eq!(
                fold_html(&build_src(Span::new(fixture)), &HtmlSubstitutionRenderer {}),
                golden_macros(fixture),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn an_escaped_paren_wrapped_shorthand_becomes_a_term_between_two_literal_parens() {
        // The shape the pair of matches produces: the backslash is gone, the
        // wrapping parentheses ride beside the node as literal text, and the
        // node is an ordinary visible term. Its location covers the match
        // *minus* the backslash the pair's first match drops — the kept parens
        // included, as every other shorthand node's location includes its own.
        let nodes = build_src(Span::new("x \\(((coffee))) y"));

        assert_eq!(nodes.len(), 5);
        assert_text(&nodes[0], "x ", 1, 1);
        assert_text(&nodes[1], "(", 1, 4);

        let index_term = assert_index_term(&nodes[2]);
        assert!(index_term.visible);
        assert_eq!(index_term.terms.len(), 1);
        assert_eq!(index_term.terms[0].as_ref(), "coffee");
        assert_eq!(index_term.location.data(), "(((coffee)))");
        assert_eq!(index_term.location.line(), 1);
        assert_eq!(index_term.location.col(), 4);

        assert_text(&nodes[3], ")", 1, 15);
        assert_text(&nodes[4], " y", 1, 16);
    }

    #[test]
    fn a_real_documents_escaped_paren_wrapped_shorthands_fold_to_their_rendered_strings() {
        // End-to-end, through the real parse path, on the shape that named this
        // increment: an escaped paren-wrapped shorthand renders as its nested
        // term between two literal parens, so a tree that left the whole match
        // literal would regress the moment `rendered_html()` becomes a fold of
        // this tree. Driven in block content and in a heading's own title,
        // whose `Title` group runs the same macros step.
        use crate::blocks::{FindBlocks, IsBlock};

        let doc = crate::Parser::default()
            .with_inline_tree(true)
            .parse(concat!(
                "== A \\(((Coffee))) heading\n",
                "\n",
                "An escaped \\(((Coffee >> Beans))) term keeps its parens,\n",
                "and \\((()))  shows nothing between them.\n",
            ));

        let mut folded_blocks = 0;

        for block in doc.descendant_blocks() {
            let (Some(rendered), Some(inlines)) = (block.rendered_html_content(), block.inlines())
            else {
                continue;
            };

            assert!(rendered.contains("(Coffee)"), "rendered: {rendered:?}");

            assert_eq!(
                crate::content::inline_builder::fold_html(
                    inlines,
                    &HtmlSubstitutionRenderer {},
                    &crate::Parser::default()
                ),
                rendered,
                "fold diverged from the rendered string for {inlines:?}"
            );

            folded_blocks += 1;
        }

        assert_eq!(folded_blocks, 1, "expected the paragraph to carry a tree");

        let mut folded_titles = 0;

        for block in doc.descendant_blocks() {
            let crate::blocks::Block::Section(section) = block else {
                continue;
            };

            assert_eq!(section.section_title(), "A (Coffee) heading");

            assert_eq!(
                crate::content::inline_builder::fold_html(
                    section.section_title_inlines(),
                    &HtmlSubstitutionRenderer {},
                    &crate::Parser::default()
                ),
                section.section_title(),
                "fold diverged from the rendered section title"
            );

            folded_titles += 1;
        }

        assert_eq!(folded_titles, 1, "expected the heading to carry a tree");
    }

    #[test]
    fn an_escaped_paren_wrapped_term_over_a_span_is_a_documented_divergence() {
        // The recognition boundary this spelling does *not* move: the nested
        // term is a shown one, so a term crossing a rendered span is
        // unreconstructable from this level's escaped match string and the
        // family's own deferral decides first — the match keeps the plain
        // escaped shape (backslash dropped, the rest literal) that every other
        // escaped spelling takes. The string pipeline, by contrast, folds the
        // span markup into the shown term and keeps only the wrapping parens.
        let source = "\\(((*bold* term)))";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::IndexTerm(_))),
            "a nested term crossing a span must be left unrecognized: {nodes:?}"
        );

        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});
        assert_eq!(folded, "(((<strong>bold</strong> term)))");
        assert_eq!(golden_macros(source), "(<strong>bold</strong> term)");
    }
}
