//! The passthrough-extraction substitution step.

use super::{
    macros::{MacroMatch, MacroMatchKind, image::range_is_verbatim, rebuild_macro_level},
    quotes::{Piece, build_match_string, source_slice},
};
use crate::{
    Parser, Span,
    content::{Content, INLINE_PASS_MACRO, SubstitutionGroup},
    inlines::InlineNode,
    strings::CowStr,
};

/// The passthrough-extraction step, as a node transducer: replaces each
/// recognized passthrough with a [`Raw`](InlineNode::Raw) leaf and leaves
/// everything else as the whole-source seed [`Text`](InlineNode::Text) node,
/// for [`apply_special_characters`](super::special_chars::apply_special_characters) and the later steps to refine.
///
/// This is the **first** step [`build`](super::build) runs – mirroring
/// [`Passthroughs::extract_from`](crate::content::Passthroughs::extract_from),
/// which the string pipeline runs *before* its own step loop – so a
/// passthrough's content is never touched by specialcharacters, quotes,
/// replacements, or macros: it is a leaf, and every later step's
/// [`build_match_string`] already treats a node it does not specifically
/// handle (an already-built [`Styled`](crate::inlines::Styled) span, and now a
/// [`Raw`](InlineNode::Raw) leaf) as a
/// single opaque placeholder.
///
/// It reuses the string pipeline's *exact* recognition –
/// [`INLINE_PASS_MACRO`] is now shared `pub(crate)` – so only the recognition
/// *sink* differs (§4.1). Two forms fold through a [`Raw`](InlineNode::Raw)
/// node whose `value`
/// is the *content itself*, since their substitution list applies nothing:
/// the triple-plus (`+++text+++`) and bare `pass:[…]` macro (with no
/// substitution list) both resolve to [`SubstitutionGroup::None`], and the
/// content borrows `'src` directly (a `pass:[…]` body unescapes an escaped
/// `\]`, as every other macro family's bracket content does, which makes the
/// unescaped case owned). The double-plus (`++text++`) and double-dollar
/// (`$$text$$`) forms resolve to [`SubstitutionGroup::Verbatim`] (special
/// characters only); rather than hand-escape `<`/`>`/`&`, their content is
/// run through the real substitution pipeline
/// ([`passthrough_text`]) so a custom
/// [`InlineSubstitutionRenderer`](crate::parser::InlineSubstitutionRenderer)'s
/// escaping is honored, exactly as it would be for the string pipeline's own
/// restore step – the cost is an owned [`Raw`](InlineNode::Raw) value rather
/// than a `'src` borrow, since the pipeline's output is not guaranteed to
/// coincide with the source.
///
/// Three forms are deferred, each documented and pinned by a divergence test:
/// an **attribute-list-prefixed** passthrough (`[quotes]++text++`,
/// `` [x-]`text` ``, `[attrs]+text+`), a **`pass:` macro carrying an explicit
/// substitution list** (`pass:c,q[…]`, whose content would need a richer
/// subtree than a single `Raw` leaf – the same reason a footnote's content
/// is structured children rather than a literal value), and the **bare
/// unconstrained form** (`+text+`, matched by [`INLINE_PASS`] rather than
/// [`INLINE_PASS_MACRO`] – its "must not follow a word" boundary needs a
/// lookbehind Rust's regex engine cannot express, which the string
/// replacer works around with a retry loop this increment does not yet
/// reproduce). Inline STEM (`stem:[…]`, `asciimath:[…]`, `latexmath:[…]`) is
/// an implicit passthrough too, but folds through its own
/// [`Stem`](InlineNode::Stem) node rather than `Raw`, so it is recognized by
/// its own step, [`apply_stem`](super::stem_step::apply_stem), run
/// immediately after this one (mirroring `Passthroughs::extract_from`, which
/// extracts STEM macros last, after both passthrough passes). This step is
/// **additive**: nothing is wired into the parse path.
///
/// The same deferred bare-form boundary shows up once more, indirectly: an
/// **escaped triple- or double-plus** (`\+++text+++`, `\++text++`) drops its
/// backslash and keeps the delimited text literal here, but the string
/// pipeline's *second* extraction pass ([`INLINE_PASS`]) re-scans that same
/// de-escaped text and consumes its leading `+++`/`++` as a bare passthrough
/// wrapping a shorter run – so these two escape forms are pinned as
/// divergences (`an_escaped_triple_plus_stays_literal`,
/// `an_escaped_double_plus_stays_literal`) rather than folded into the main
/// parity corpus. An escaped `$$…$$` or `pass:[…]` has no such residue and
/// stays parity, since [`INLINE_PASS`] never matches `$$` or `pass:` syntax.
///
/// [`INLINE_PASS`]: crate::content::passthroughs
/// [`InlineSubstitutionRenderer`](crate::parser::InlineSubstitutionRenderer):
/// crate::parser::InlineSubstitutionRenderer
pub(super) fn apply_passthroughs<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes);

    // Cheap pre-filter mirroring `Passthroughs::extract_from`'s own guard: a
    // recognized form always contains `++`, `$$`, or (for `pass:`) `ss:`.
    if !(s.contains("++") || s.contains("$$") || s.contains("ss:")) {
        return nodes;
    }

    let matches = find_passthrough_matches(&s, &pieces, root, parser);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every passthrough at this level, skipping the deferred forms
/// [`apply_passthroughs`] documents: an attribute-list-prefixed match (an
/// optional group [`INLINE_PASS_MACRO`] captures ahead of the delimiters) and
/// a `pass:` macro carrying an explicit substitution list.
fn find_passthrough_matches<'src>(
    s: &str,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Vec<MacroMatch<'src>> {
    let mut matches = Vec::new();

    for caps in INLINE_PASS_MACRO.captures_iter(s) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();

        let full = whole.start()..whole.end();

        // An attribute list ahead of the delimiters (`[quotes]++text++`,
        // `` [x-]`text` ``) is deferred.
        if caps.get(2).is_some() {
            continue;
        }

        // A `pass:` macro carrying an explicit substitution list
        // (`pass:c,q[…]`) is deferred.
        if caps.get(14).is_some() {
            continue;
        }

        // Only a wholly-verbatim match can slice its content from `'src`; a
        // match crossing an already-recognized construct cannot occur here –
        // this is the very first step – but the check is kept for the same
        // reason every other family keeps it: a future caller of this
        // function over a non-seed level must not silently mis-slice.
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

        let node = build_passthrough_node(&caps, &full, pieces, root, parser);

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

/// Builds one [`Raw`](InlineNode::Raw) node from a verbatim, unescaped
/// passthrough match – see [`apply_passthroughs`] for how each delimiter form
/// maps to its `value`.
fn build_passthrough_node<'src>(
    caps: &regex::Captures<'_>,
    full: &std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> InlineNode<'src> {
    let location = source_slice(pieces, full.clone(), root);

    if let Some(m) = caps.get(5) {
        // `+++text+++`: `SubstitutionGroup::None` applies nothing, so the
        // content is genuinely raw and borrows `'src` directly.
        let content = source_slice(pieces, m.start()..m.end(), root);

        return InlineNode::Raw {
            value: CowStr::from(content.data()),
            location,
        };
    }

    if let Some(m) = caps.get(8).or_else(|| caps.get(11)) {
        // `++text++` / `$$text$$`: `SubstitutionGroup::Verbatim` applies only
        // special characters. Computed through the real substitution
        // pipeline (rather than hand-escaping `<`/`>`/`&`) so a custom
        // `InlineSubstitutionRenderer`'s escaping is honored.
        let content = source_slice(pieces, m.start()..m.end(), root);
        let value = passthrough_text(content.data(), &SubstitutionGroup::Verbatim, parser);

        return InlineNode::Raw {
            value: CowStr::from(value),
            location,
        };
    }

    // The bare `pass:[…]` macro (no explicit substitution list):
    // `SubstitutionGroup::None` applies nothing, and an escaped closing
    // bracket (`\]`) unescapes, mirroring the string replacer's
    // `text.replace("\\]", "]")` – the same treatment every other macro
    // family's bracket content gets.
    #[allow(clippy::unwrap_used)]
    let m = caps.get(15).unwrap();
    let content = source_slice(pieces, m.start()..m.end(), root);
    let raw = content.data();

    let value = if raw.contains("\\]") {
        CowStr::from(raw.replace("\\]", "]"))
    } else {
        CowStr::from(raw)
    };

    InlineNode::Raw { value, location }
}

/// Runs `text` through the real substitution pipeline under `subs`, returning
/// the resulting owned string. Used to compute a [`Raw`](InlineNode::Raw)
/// passthrough's `value` under [`SubstitutionGroup::Verbatim`] – mirroring
/// `PassthroughRestoreReplacer`'s own `pass.subs.apply(…)` call in the string
/// pipeline's restore step – so the result honors whatever
/// [`InlineSubstitutionRenderer`](crate::parser::InlineSubstitutionRenderer)
/// `parser` carries rather than a hand-rolled, always-default escaping.
pub(super) fn passthrough_text(text: &str, subs: &SubstitutionGroup, parser: &Parser) -> String {
    let mut content = Content::from(Span::new(text));
    subs.apply(&mut content, parser, None);
    content.rendered_str().to_string()
}

/// Reports whether `c` is one of the three characters the special-characters
/// substitution acts on.
pub(super) fn is_special(c: char) -> bool {
    matches!(c, '<' | '>' | '&')
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use super::{
        super::test_support::{assert_raw, build_src, fold_html, golden_passthroughs, seed},
        apply_passthroughs,
    };
    use crate::{
        Parser, Span,
        inlines::{InlineNode, SpanForm, StyleVariant, Styled},
        parser::HtmlSubstitutionRenderer,
        strings::CowStr,
    };

    #[test]
    fn apply_passthroughs_is_a_noop_without_passthrough_syntax() {
        // The cheap pre-filter returns the seed unchanged when no `++`, `$$`, or
        // `ss:` substring is present, so ordinary content never pays for the
        // level-building machinery.
        let source = Span::new("plain text, no passthrough syntax here");
        let seeded = seed(source);
        let nodes = apply_passthroughs(seeded.clone(), source, &Parser::default());

        assert_eq!(nodes, seeded);
    }

    #[test]
    fn a_match_whose_content_crosses_an_already_built_node_is_deferred() {
        // In practice `apply_passthroughs` only ever runs on the pristine
        // whole-source seed (it is `build`'s first step, ahead of every node
        // that could make a range non-verbatim), so `range_is_verbatim`'s
        // false branch is defensive – kept for the same reason every other
        // macro family keeps the check. Exercise it directly, feeding a
        // hand-built level whose triple-plus content spans an already-built
        // `Styled` node, to document the intended fallback: the whole match
        // is left unrecognized rather than mis-sliced.
        let location = Span::new("+++");

        let nodes = vec![
            InlineNode::Text {
                value: CowStr::from("+++"),
                location,
            },
            InlineNode::Styled(Styled {
                variant: StyleVariant::Strong,
                form: SpanForm::Constrained,
                id: None,
                roles: vec![],
                attrs: None,
                children: vec![],
                location,
            }),
            InlineNode::Text {
                value: CowStr::from("+++"),
                location,
            },
        ];

        let root = Span::new("+++");
        let result = apply_passthroughs(nodes.clone(), root, &Parser::default());

        assert_eq!(
            result, nodes,
            "a non-verbatim match must be left unrecognized"
        );
    }

    #[test]
    fn triple_plus_borrows_its_content_unescaped() {
        // `SubstitutionGroup::None` applies nothing, so the content is genuinely
        // raw – not even special characters are escaped – and borrows `'src`.
        let nodes = build_src(Span::new("+++<b>*not quotes*</b>+++"));

        assert_eq!(nodes.len(), 1);

        // `location` covers the whole `+++…+++` construct (delimiters
        // included), matching every other macro node; only `value` is the
        // unwrapped content.
        let location = assert_raw(&nodes[0], "<b>*not quotes*</b>");
        assert_eq!(location.data(), "+++<b>*not quotes*</b>+++");

        match &nodes[0] {
            InlineNode::Raw { value, .. } => {
                assert!(
                    matches!(value, CowStr::Borrowed(_)),
                    "triple-plus content should borrow from source, got {value:?}"
                );
            }
            other => panic!("expected Raw, got {other:?}"),
        }
    }

    #[test]
    fn double_plus_and_double_dollar_escape_specials_only() {
        // `SubstitutionGroup::Verbatim` applies only special characters: `<`/`>`
        // are escaped, but `*not quotes*` is left alone (quotes never runs over
        // passthrough content).
        for source in ["++<b>*not quotes*</b>++", "$$<b>*not quotes*</b>$$"] {
            let nodes = build_src(Span::new(source));

            assert_eq!(nodes.len(), 1, "for {source:?}: {nodes:?}");
            assert_raw(&nodes[0], "&lt;b&gt;*not quotes*&lt;/b&gt;");
        }
    }

    #[test]
    fn bare_pass_macro_borrows_unescaped_content() {
        let nodes = build_src(Span::new("pass:[<b>*not quotes*</b>]"));

        assert_eq!(nodes.len(), 1);
        let location = assert_raw(&nodes[0], "<b>*not quotes*</b>");
        assert_eq!(location.data(), "pass:[<b>*not quotes*</b>]");
    }

    #[test]
    fn an_empty_pass_macro_builds_an_empty_raw_node() {
        let nodes = build_src(Span::new("pass:[]"));

        assert_eq!(nodes.len(), 1);
        assert_raw(&nodes[0], "");
    }

    #[test]
    fn a_pass_macro_unescapes_an_escaped_closing_bracket() {
        // Mirrors the string replacer's `text.replace("\\]", "]")`, the same
        // treatment every other macro family's bracket content gets. The
        // unescape makes the value owned, unlike the no-escape case above.
        let nodes = build_src(Span::new(r"pass:[a\]b]"));

        assert_eq!(nodes.len(), 1);
        assert_raw(&nodes[0], "a]b");

        match &nodes[0] {
            InlineNode::Raw { value, .. } => {
                assert!(matches!(value, CowStr::Boxed(_)), "got {value:?}");
            }
            other => panic!("expected Raw, got {other:?}"),
        }
    }

    #[test]
    fn an_escaped_triple_plus_stays_literal() {
        // The builder drops the single backslash and keeps `+++text+++`
        // literal, mirroring every other family's `\image:…` escape handling.
        // The string pipeline instead runs a *second* pass (`INLINE_PASS`,
        // the deferred bare `+text+` form) over that same de-escaped text,
        // which re-consumes its leading `+++` as a bare passthrough wrapping
        // a single `+` – so this is a documented divergence, not parity, and
        // stems from the same deferred boundary as the bare unconstrained
        // form.
        let source = r"\+++text+++";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Raw { .. })),
            "an escaped passthrough must not build a Raw node: {nodes:?}"
        );

        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});
        assert_eq!(folded, "+++text+++");
        assert_ne!(folded, golden_passthroughs(source));
    }

    #[test]
    fn an_escaped_double_plus_stays_literal() {
        // Same divergence as `an_escaped_triple_plus_stays_literal`, one
        // delimiter layer down: de-escaping `\++text++` to `++text++` leaves
        // a leading `++` the deferred bare-form pass also re-consumes.
        let source = r"\++text++";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Raw { .. })),
            "an escaped passthrough must not build a Raw node: {nodes:?}"
        );

        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});
        assert_eq!(folded, "++text++");
        assert_ne!(folded, golden_passthroughs(source));
    }

    #[test]
    fn an_escaped_pass_macro_stays_literal() {
        let nodes = build_src(Span::new(r"\pass:[x]"));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Raw { .. })),
            "an escaped passthrough must not build a Raw node: {nodes:?}"
        );

        assert_eq!(fold_html(&nodes, &HtmlSubstitutionRenderer {}), "pass:[x]");
    }

    #[test]
    fn fold_matches_the_string_pipeline_through_passthroughs() {
        // For each fixture, folding the single-pass tree (all six steps,
        // passthroughs included) reproduces the string pipeline's output
        // byte-for-byte. This is the differential corpus (design §5.3) that
        // pins the passthrough increment.
        let fixtures = [
            // No passthrough despite passthrough-ish characters.
            "plain text",
            "a colon : and a plus + apart",
            "one + two - not a passthrough",
            // Triple-plus: no substitutions at all.
            "+++<b>bold</b>+++",
            "+++*not quotes*+++",
            "prefix +++<raw/>+++ suffix",
            "+++line one\nline two+++",
            // Double-plus / double-dollar: special characters only.
            "++<b>++",
            "++*not quotes*++",
            "$$<b>$$",
            "$$*not quotes*$$",
            // The bare `pass:[…]` macro.
            "pass:[<b>]",
            "pass:[]",
            r"pass:[a\]b]",
            "pass:[*not quotes*]",
            // Multiple passthroughs, and passthroughs beside ordinary quoted
            // text (proving the surrounding text is still substituted
            // normally).
            "a +++x+++ and a ++y++ and a $$z$$ and a pass:[w]",
            "*bold* +++<raw/>+++ *more bold*",
            // Escapes. The `+++`/`++` forms are handled elsewhere (see
            // `an_escaped_triple_plus_stays_literal` and
            // `an_escaped_double_plus_stays_literal`), since de-escaping them
            // can leave a residue the deferred bare `+text+` pass would also
            // consume – a documented divergence, not parity.
            r"\$$text$$",
            r"\pass:[x]",
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
    fn an_attribute_list_prefixed_passthrough_is_a_documented_divergence() {
        // `[.role]++text++` splices an attribute list ahead of the delimiters,
        // wrapping the restored content in a `<span>` at restore time. The
        // builder cannot yet carry that attribute list, so the whole match is
        // left unrecognized (no `Raw` node; `++`/`++` stay literal, and the
        // surrounding steps do not otherwise recognize them either).
        let source = "[.role]++text++";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Raw { .. })),
            "an attribute-list-prefixed passthrough must be left unrecognized: {nodes:?}"
        );

        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});
        let golden = golden_passthroughs(source);

        assert_ne!(folded, golden);
        assert!(golden.contains("role"), "golden: {golden:?}");
    }

    #[test]
    fn a_pass_macro_with_an_explicit_subs_list_is_a_documented_divergence() {
        // `pass:c[…]` names an explicit substitution list, which would need a
        // richer subtree than a single `Raw` leaf can hold (the same reason a
        // footnote's content is structured children rather than a literal
        // value) – deferred to a later increment.
        let source = "pass:c[<b>]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Raw { .. })),
            "a pass: macro with an explicit subs list must be left unrecognized: {nodes:?}"
        );

        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});
        let golden = golden_passthroughs(source);

        assert_ne!(folded, golden);
    }

    #[test]
    fn a_bare_unconstrained_passthrough_is_a_documented_divergence() {
        // The bare `+text+` form is matched by `INLINE_PASS`, not
        // `INLINE_PASS_MACRO` – its "must not follow a word" boundary needs a
        // lookbehind Rust's regex engine cannot express, which the string
        // replacer works around with a retry loop this increment does not
        // reproduce. Left unrecognized: the plus signs stay literal.
        let source = "a +text+ b";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Raw { .. })),
            "a bare unconstrained passthrough must be left unrecognized: {nodes:?}"
        );

        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});
        let golden = golden_passthroughs(source);

        assert_ne!(folded, golden);
        assert_eq!(folded, "a +text+ b");
    }
}
