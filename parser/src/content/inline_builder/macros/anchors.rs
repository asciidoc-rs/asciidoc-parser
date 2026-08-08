//! Inline anchor recognition (`[[id]]`, `[[id,reftext]]`, `anchor:id[…]`).

use super::{MacroMatch, MacroMatchKind, image::range_is_verbatim, rebuild_macro_level};
use crate::{
    Span,
    content::{
        INLINE_ANCHOR,
        inline_builder::quotes::{Piece, build_match_string, source_slice},
    },
    inlines::{Anchor, InlineNode},
    strings::CowStr,
};

/// Matches `INLINE_ANCHOR` at this level's escaped text, replacing each
/// recognized inline anchor – the `[[id]]` / `[[id,reftext]]` shorthand and the
/// `anchor:id[reftext]` macro – with the [`Anchor`](InlineNode::Anchor) node it
/// produces and leaving everything else in place.
pub(super) fn anchor_macros_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes);

    // Cheap pre-filter: an anchor needs either the shorthand `[[` opener or the
    // `anchor:` macro prefix. The `[` characters are not special, so a shorthand
    // reaches the macros step with its `[[` intact.
    if !s.contains("[[") && !s.contains("anchor:") {
        return nodes;
    }

    let matches = find_anchor_matches(&s, &pieces, root);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every recognized inline anchor at this level – both spellings – as a
/// [`MacroMatch`].
///
/// An anchor's HTML rendering (`<a id="…"></a>`) is a function of its **id
/// alone**, and an id admits no special character (the pattern's id class is
/// letters/digits/`_`/`-`/`:`/`.`), so an id is always verbatim and an anchor
/// is *always* recognized – unlike the link/xref families, an anchor is never
/// deferred on a non-verbatim boundary. A non-verbatim *reference text* (one
/// carrying a rendered span or an escaped special) does not reach the flow, so
/// it only leaves the node's `reftext` unpopulated (see [`build_anchor_node`]).
pub(super) fn find_anchor_matches<'src>(
    s: &str,
    pieces: &[Piece],
    root: Span<'src>,
) -> Vec<MacroMatch<'src>> {
    let mut matches = Vec::new();

    for caps in INLINE_ANCHOR.captures_iter(s) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();

        let full = whole.start()..whole.end();

        // An escape (`\[[…` / `\anchor:…`) is honored by dropping the backslash
        // and keeping the rest literal, mirroring the string replacer's leading
        // `caps.get(1)` check. [`rebuild_macro_level`] emits the kept range with
        // [`emit_range`], which clones an atomic piece (a rendered-span reference
        // text) whole, so the unescape works even across a non-verbatim reference
        // text – just as the id itself is always verbatim, the whole anchor
        // unescapes regardless.
        if caps.get(1).is_some() {
            matches.push(MacroMatch {
                kind: MacroMatchKind::Unescape {
                    backslash: full.start,
                },
                full,
            });

            continue;
        }

        let node = build_anchor_node(&caps, &full, pieces, root);

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

/// Builds one [`Anchor`](InlineNode::Anchor) node from an inline-anchor match,
/// slicing the id straight from `'src` (an id is always verbatim) so the fold
/// reproduces the string replacer's `<a id="…"></a>` exactly.
///
/// Two spellings share this builder: the `[[id,reftext]]` shorthand (groups
/// 2/3) and the `anchor:id[reftext]` macro (groups 4/5). Exactly one id group
/// matches.
///
/// The optional reference text is captured as the node's `reftext` – a single
/// [`Text`](InlineNode::Text) child – **when it is verbatim** (the common case,
/// borrowing `'src`; a shorthand's trailing whitespace is trimmed and a macro's
/// escaped `\]` is unescaped into an owned value, mirroring the string
/// replacer). A reference text that carries a rendered span or an escaped
/// special is non-verbatim; because it never reaches the flow (the anchor
/// renders from its id alone), the anchor is still recognized but its `reftext`
/// is left `None` rather than sliced wrongly from `'src` – the same verbatim
/// boundary the other macro families document, and a shape a re-flow consumer
/// can refine later (the field is provisional, per the node's Phase-0 note).
///
/// As in the additive builder generally, this performs *no* recognition side
/// effect – notably it does **not** `register_ref` the id in the catalog (so a
/// cross-reference can resolve against it), nor emit the duplicate-id warning
/// the string replacer raises; the cutover (design §5.2 Phase 4, step 6) wires
/// those in.
fn build_anchor_node<'src>(
    caps: &regex::Captures<'_>,
    full: &std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
) -> InlineNode<'src> {
    let location = source_slice(pieces, full.clone(), root);

    // Exactly one id group matches: group 2 for the `[[…]]` shorthand (with its
    // reference text in group 3), else group 4 for the `anchor:…[…]` macro (with
    // its reference text in group 5).
    #[allow(clippy::unwrap_used)]
    let (id_match, reftext_match, is_shorthand) = if let Some(id) = caps.get(2) {
        (id, caps.get(3), true)
    } else {
        // Group 4 always matches when group 2 does not; the alternation admits
        // no third form.
        (caps.get(4).unwrap(), caps.get(5), false)
    };

    // An id admits no special character, so it is verbatim and borrows `'src`.
    let id_span = source_slice(pieces, id_match.start()..id_match.end(), root);
    let id = CowStr::from(id_span.data());

    let reftext = reftext_match
        .and_then(|m| build_anchor_reftext(m.start()..m.end(), pieces, root, is_shorthand));

    InlineNode::Anchor(Anchor {
        id,
        reftext,
        location,
    })
}

/// Builds an inline anchor's `reftext` – a single [`Text`](InlineNode::Text)
/// child – from the reference-text capture's match-string `range`, or `None`
/// when the reference text is non-verbatim or trims to empty (see
/// [`build_anchor_node`] for why a non-verbatim reference text is not an
/// error).
///
/// A `shorthand` reference text has its trailing whitespace stripped (the
/// string replacer's `trim_end`; leading whitespace was already excluded by the
/// pattern's `, \s*`). A macro reference text unescapes an escaped `\]` into an
/// owned value, mirroring the replacer's `replace("\\]", "]")`; without one it
/// borrows `'src`.
fn build_anchor_reftext<'src>(
    range: std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
    shorthand: bool,
) -> Option<Vec<InlineNode<'src>>> {
    if !range_is_verbatim(pieces, &range) {
        return None;
    }

    let span = source_slice(pieces, range, root);
    let raw = span.data();

    let child = if shorthand {
        let trimmed_len = raw.trim_end().len();

        if trimmed_len == 0 {
            return None;
        }

        let text_location = span.slice(0..trimmed_len);

        InlineNode::Text {
            value: CowStr::from(text_location.data()),
            location: text_location,
        }
    } else if raw.contains("\\]") {
        // An escaped bracket makes the logical text a synthesized (owned) value
        // whose `location` still covers the raw source it derives from.
        InlineNode::Text {
            value: CowStr::from(raw.replace("\\]", "]")),
            location: span,
        }
    } else {
        InlineNode::Text {
            value: CowStr::from(raw),
            location: span,
        }
    };

    Some(vec![child])
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use super::super::super::test_support::{
        assert_styled, assert_text, build_src, fold_html, golden_macros,
    };
    use crate::{
        Span,
        inlines::{Anchor, InlineNode, SpanForm, StyleVariant},
        parser::HtmlSubstitutionRenderer,
        strings::CowStr,
    };

    /// Asserts that `node` is an [`Anchor`](InlineNode::Anchor), returning it.
    fn assert_anchor<'a, 'src>(node: &'a InlineNode<'src>) -> &'a Anchor<'src> {
        match node {
            InlineNode::Anchor(anchor) => anchor,

            other => panic!("expected an Anchor, got {other:?}"),
        }
    }

    #[test]
    fn fold_matches_the_string_pipeline_through_anchors() {
        // For each fixture, folding the single-pass tree (all five steps)
        // reproduces the string pipeline's output byte-for-byte. This is the
        // differential corpus (design §5.3) that pins the inline-anchor
        // increment. An anchor renders from its id alone, so every fixture is
        // recognized (there is no deferred-output boundary for anchors); the
        // reference-text cases additionally exercise the node's `reftext`
        // population, which does not affect the flow.
        let fixtures = [
            // No anchor despite bracket/colon characters.
            "plain text without an anchor",
            "a lone [ bracket and a colon : apart",
            "an anchor without brackets anchor:foo stays literal",
            // Shorthand: bare id, and with a reference text.
            "[[install]]",
            "[[install,Installation]]",
            "[[sect-one,Section One]]",
            // Macro form: empty brackets, and with a reference text.
            "anchor:install[]",
            "anchor:install[Installation]",
            // Id character classes: `_`, `:`, `-`, `.`, digits.
            "[[_foo]]",
            "[[:bar]]",
            "[[a-b.c:d]]",
            "[[sect_1]]",
            "anchor:a.b-c:d[]",
            // A reference text with trailing whitespace (trimmed by the string
            // replacer) and an escaped `]` (unescaped by the macro form).
            "[[install, Installation ]]",
            "anchor:foo[a\\]b]",
            // A shorthand whose reference text is whitespace only trims to empty:
            // the node carries no reference text, exactly as the bare form.
            "[[install, ]]",
            // Embedded in surrounding flow, and next to other constructs.
            "See [[here]] for details.",
            "text anchor:x[X] more",
            "*bold* then [[x]] and _em_",
            "a copyright (C) then [[x]]",
            "[[a]] and [[b]] and anchor:c[C]",
            // Escapes: the anchor stays literal, minus the backslash.
            "\\[[install]]",
            "\\[[install,Installation]]",
            "\\anchor:install[]",
            "\\anchor:foo[Ref]",
            // Recognized inside a rendered span.
            "*see [[x]]*",
            "_anchor:y[] in em_",
            // A reference text that is a rendered span or a special: the anchor is
            // still recognized (its id alone renders), and the reference text –
            // which never reaches the flow – is consumed with the match.
            "[[id,*bold*]]",
            "anchor:id[*bold*]",
            "[[id,A & B]]",
            // A triple-bracket `[[[id]]]` outside a bibliography list item is not
            // a bibliography anchor (that pass fires only inside such an item); the
            // inner `[[id]]` is a plain anchor with literal outer brackets, exactly
            // as the string pipeline routes it here.
            "[[[id]]]",
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
    fn an_anchor_shorthand_becomes_a_node() {
        let nodes = build_src(Span::new("[[install,Installation]]"));

        assert_eq!(nodes.len(), 1);
        let anchor = assert_anchor(&nodes[0]);

        // The id borrows from source (no allocation).
        assert!(matches!(anchor.id, CowStr::Borrowed(_)));
        assert_eq!(anchor.id.as_ref(), "install");

        // Its location covers the whole anchor, the `[[` / `]]` included.
        assert_eq!(anchor.location.data(), "[[install,Installation]]");
        assert_eq!(anchor.location.line(), 1);
        assert_eq!(anchor.location.col(), 1);

        // The reference text is a single borrowed `Text` child located at its
        // source (`[[install,` is 10 characters, so it starts at column 11).
        let reftext = anchor.reftext.as_ref().unwrap();
        assert_eq!(reftext.len(), 1);
        assert_text(&reftext[0], "Installation", 1, 11);
    }

    #[test]
    fn an_anchor_macro_becomes_a_node() {
        let nodes = build_src(Span::new("anchor:install[Installation]"));

        let anchor = assert_anchor(&nodes[0]);
        assert!(matches!(anchor.id, CowStr::Borrowed(_)));
        assert_eq!(anchor.id.as_ref(), "install");
        assert_eq!(anchor.location.data(), "anchor:install[Installation]");

        // `anchor:install[` is 15 characters, so the text starts at column 16.
        let reftext = anchor.reftext.as_ref().unwrap();
        assert_text(&reftext[0], "Installation", 1, 16);
    }

    #[test]
    fn a_bare_anchor_has_no_reftext() {
        // A shorthand without a `, reference text` and an empty-bracket macro
        // both leave `reftext` `None`.
        for source in ["[[install]]", "anchor:install[]"] {
            let nodes = build_src(Span::new(source));
            let anchor = assert_anchor(&nodes[0]);
            assert_eq!(anchor.id.as_ref(), "install");
            assert!(anchor.reftext.is_none(), "no reftext for {source:?}");
        }
    }

    #[test]
    fn an_anchor_shorthand_reftext_is_trimmed() {
        // A shorthand's trailing whitespace is stripped (the string replacer's
        // `trim_end`); leading whitespace was already excluded by the pattern's
        // `, \s*`.
        let nodes = build_src(Span::new("[[install, Installation ]]"));

        let anchor = assert_anchor(&nodes[0]);
        let reftext = anchor.reftext.as_ref().unwrap();

        // `[[install, ` is 11 characters, so the trimmed text starts at column 12.
        assert_text(&reftext[0], "Installation", 1, 12);
    }

    #[test]
    fn an_anchor_shorthand_reftext_that_is_whitespace_only_has_no_reftext() {
        // A shorthand reference text that trims to empty (the pattern's `(.+?)`
        // matched only the whitespace the string replacer's `trim_end` strips)
        // leaves `reftext` `None`, the same shape as the bare `[[id]]` form – and
        // it still folds to the same `<a id="…"></a>` the string pipeline emits.
        let source = "[[install, ]]";
        let nodes = build_src(Span::new(source));

        let anchor = assert_anchor(&nodes[0]);
        assert_eq!(anchor.id.as_ref(), "install");
        assert!(
            anchor.reftext.is_none(),
            "a whitespace-only reftext trims away"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn an_anchor_macro_reftext_unescapes_a_bracket() {
        // A macro reference text unescapes `\]` into `]`, making the logical text
        // a synthesized (owned) value whose `location` still covers the raw
        // source it derives from.
        let nodes = build_src(Span::new("anchor:foo[a\\]b]"));

        let anchor = assert_anchor(&nodes[0]);
        let reftext = anchor.reftext.as_ref().unwrap();
        assert_eq!(reftext.len(), 1);

        match &reftext[0] {
            InlineNode::Text { value, location } => {
                assert_eq!(value.as_ref(), "a]b");
                assert!(
                    matches!(value, CowStr::Boxed(_)),
                    "an unescaped value is synthesized (owned), got {value:?}"
                );
                assert_eq!(location.data(), "a\\]b");
            }

            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn an_anchor_is_recognized_inside_a_span() {
        // An anchor can appear inside a rendered span; the transducer descends
        // into the span body and builds the node there.
        let nodes = build_src(Span::new("*see [[x]]*"));

        let children = assert_styled(&nodes[0], StyleVariant::Strong, SpanForm::Constrained);
        assert_eq!(children.len(), 2);
        assert_text(&children[0], "see ", 1, 2);

        let anchor = assert_anchor(&children[1]);
        assert_eq!(anchor.id.as_ref(), "x");
    }

    #[test]
    fn an_escaped_anchor_stays_literal() {
        // `\[[…]]` and `\anchor:…[…]` drop the backslash and keep the anchor as
        // literal text – no anchor node – exactly as the string replacer's escape
        // branch does.
        for source in ["\\[[install]]", "\\anchor:install[Installation]"] {
            let nodes = build_src(Span::new(source));

            assert!(
                nodes.iter().all(|n| !matches!(n, InlineNode::Anchor(_))),
                "an escaped anchor must not produce an anchor node: {nodes:?}"
            );

            assert_eq!(
                fold_html(&nodes, &HtmlSubstitutionRenderer {}),
                golden_macros(source),
                "fold diverged for {source:?}"
            );
        }
    }

    #[test]
    fn an_anchor_reference_text_over_a_span_is_consumed() {
        // A reference text that is a rendered span (`[[id,*bold*]]`) does not
        // reach the flow: the anchor is still recognized (its id alone renders),
        // the span is consumed with the match, and the node's `reftext` is left
        // `None` (a non-verbatim reference text the builder does not slice from
        // `'src`). The fold still matches the string pipeline byte-for-byte.
        let source = "[[id,*bold*]]";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        let anchor = assert_anchor(&nodes[0]);
        assert_eq!(anchor.id.as_ref(), "id");
        assert!(
            anchor.reftext.is_none(),
            "a non-verbatim reference text is left unpopulated"
        );
        assert_eq!(anchor.location.data(), "[[id,*bold*]]");

        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});
        assert_eq!(folded, golden_macros(source));

        // The consumed span does not render into the flow.
        assert!(!folded.contains("<strong>"), "folded: {folded}");
    }
}
