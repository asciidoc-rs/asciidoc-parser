//! The callouts substitution step.

use regex::Regex;

use super::{
    quotes::{Piece, build_match_string, emit_range, source_slice},
    special_chars::Masked,
};
use crate::{
    Parser, Span,
    attributes::Attrlist,
    content::build_callout_regexes,
    inlines::{Callout, CalloutGuard, InlineNode},
    parser::CharacterReplacementType,
    strings::CowStr,
};

/// Registers every callout the tree carries with `parser`, so the callout list
/// that annotates the block can be validated against them.
///
/// The callouts step's own recognition side effect, replayed from the tree —
/// the counterpart of
/// [`apply_macro_side_effects`](super::macros::apply_macro_side_effects) for
/// the one family that is not a macro. It runs on the real parse path from
/// [`SubstitutionGroup::apply`](crate::content::SubstitutionGroup).
///
/// A callout number is registered in document order, which is what
/// [`Parser::callout_defined`](crate::Parser) reads when the following callout
/// list is parsed — a *later block*, so this runs early enough for it.
///
/// A [`Callout`](InlineNode::Callout) node's `number` is already the resolved
/// sequential value for an auto-numbered `<.>`, so no counter is consulted
/// here. A number that does not parse as a `u32` is skipped, exactly as the
/// string pipeline skips it.
///
/// The walk recurses into every container a node can nest inside. Callouts are
/// recognized in **verbatim** content, where no quotes or macros step runs, so
/// in practice they are always top-level; recursing costs nothing and keeps
/// this walk from silently narrowing if that ever stops being true.
pub(crate) fn apply_callout_side_effects(nodes: &[InlineNode<'_>], parser: &Parser) {
    for node in nodes {
        match node {
            InlineNode::Callout(callout) => {
                if let Ok(number) = callout.number.parse::<u32>() {
                    parser.register_callout(number);
                }
            }

            InlineNode::Styled(styled) => apply_callout_side_effects(&styled.children, parser),
            InlineNode::Ref(reference) => apply_callout_side_effects(&reference.children, parser),
            InlineNode::Footnote(footnote) => {
                apply_callout_side_effects(&footnote.children, parser);
            }
            InlineNode::IndexTerm(index_term) => {
                apply_callout_side_effects(&index_term.children, parser);
            }
            InlineNode::Anchor(anchor) => {
                if let Some(reftext) = anchor.reftext.as_ref() {
                    apply_callout_side_effects(reftext, parser);
                }
            }

            _ => {}
        }
    }
}

/// The callouts substitution, as a node transducer: a trailing callout token
/// (`<1>`, `<.>`, or `<!--1-->` for XML) is replaced with a
/// [`Callout`](InlineNode::Callout) leaf.
///
/// Unlike every other step in this module, this is not a step of the
/// *normal* effective order [`build`](super::build) runs: callouts belong to
/// [`SubstitutionGroup::Verbatim`](crate::content::SubstitutionGroup::Verbatim)
/// (literal, listing, and source blocks), whose only steps are
/// `SpecialCharacters` then `Callouts` — so `nodes` here can never contain a
/// [`Styled`](crate::inlines::Styled) or [`Ref`](crate::inlines::Ref) span
/// (`Quotes` and `Macros` never ran) and this function need not descend into
/// anything; the level it matches against is the only level.
/// [`build_for_group`](super::build_for_group) runs it for the groups that
/// carry the step, directly after
/// [`apply_special_characters`](super::special_chars::apply_special_characters).
///
/// It reuses the string pipeline's *exact* recognition —
/// [`build_callout_regexes`] is now shared `pub(crate)` — so only the
/// recognition *sink* differs (§4.1): a matched, trailing-position callout
/// becomes a node instead of rendered markup, folding through the same
/// `render_callout` the string step calls (see `fold_callout`) so the
/// output is byte-for-byte identical. `attrlist` is the block's own
/// attribute list (`None` when the caller has none); together with `parser`
/// it resolves the `line-comment` attribute exactly as
/// [`apply_callouts`](crate::content::substitution_step)'s string-pipeline
/// counterpart does: absent, the default line-comment prefixes (`//`, `#`,
/// `--`, `;;`) and XML callouts are recognized; present (non-empty), only
/// that prefix is; present but empty, no prefix is recognized (and neither
/// are XML callouts).
///
/// An escaped callout (`\<1>`) drops its backslash and stays literal,
/// mirroring every other macro family's escape handling. This pass performs
/// **no** recognition side effect of its own: `Parser::register_callout` is
/// the replay's to call, from the tree, in
/// [`apply_callout_side_effects`] — exactly as the macro families leave
/// their registrations to
/// [`apply_macro_side_effects`](super::macros::apply_macro_side_effects).
pub(super) fn apply_callouts<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
    attrlist: Option<&Attrlist<'_>>,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes, Masked::UNKNOWN);

    // A callout's opening bracket is always rendered as `&lt;` by
    // `apply_special_characters`, so we can cheaply skip content without any
    // (mirrors the string pipeline's own guard).
    if !s.contains("&lt;") {
        return nodes;
    }

    // The `line-comment` attribute (block-level, falling back to
    // document-level) customizes or disables line-comment recognition; see
    // this function's doc comment.
    let line_comment: Option<String> = attrlist
        .and_then(|a| a.named_attribute("line-comment"))
        .map(|a| a.value().to_string())
        .or_else(|| {
            if parser.has_attribute("line-comment") {
                Some(
                    parser
                        .attribute_value("line-comment")
                        .as_maybe_str()
                        .unwrap_or("")
                        .to_string(),
                )
            } else {
                None
            }
        });

    let (callout_rx, tail_rx) = build_callout_regexes(line_comment.as_deref());

    let matches = find_callout_matches(&s, &callout_rx, tail_rx);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_callout_level(&nodes, &pieces, &s, &matches, root)
}

/// One callout match at a level, in absolute match-string byte offsets.
struct CalloutMatch {
    /// The whole match, `[start, end)`.
    full: std::ops::Range<usize>,

    kind: CalloutMatchKind,
}

enum CalloutMatchKind {
    /// An escaped callout (`\<1>`): drop the escaping backslash and keep the
    /// rest of the match as literal text, replacing nothing.
    Unescape { backslash: usize },

    /// A recognized, trailing-position callout token.
    Callout {
        /// The callout number to display (already resolved for an
        /// automatically-numbered `<.>`).
        number: String,

        /// Whether this is the XML comment form (`<!--N-->`).
        is_xml: bool,

        /// The captured line-comment prefix, if any (its absence does not by
        /// itself mean "no guard" — see [`rebuild_callout_level`], which also
        /// consults `is_xml`).
        prefix: Option<std::ops::Range<usize>>,
    },
}

/// Finds every [`build_callout_regexes`] match in the match string `s`, left
/// to right, honoring the trailing-position lookahead (`tail_rx`) exactly as
/// the string pipeline's `CalloutReplacer` does: a callout is only recognized
/// when nothing but further callouts follows it up to the end of the line.
/// Unlike the string pipeline (whose `LookaheadReplacer` never skips ahead
/// and retries for this step), a match that fails the lookahead is simply
/// left out of the returned list — the surrounding gap then reproduces its
/// original nodes unchanged, exactly the same outcome as the string
/// pipeline's `dest.push_str(&caps[0])` fallback.
fn find_callout_matches(s: &str, callout_rx: &Regex, tail_rx: &Regex) -> Vec<CalloutMatch> {
    let mut matches = Vec::new();
    let mut autonum = 0u32;

    for caps in callout_rx.captures_iter(s) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();

        if !tail_rx.is_match(&s[whole.end()..]) {
            continue;
        }

        let full = whole.start()..whole.end();

        if let Some(esc) = caps.name("esc") {
            matches.push(CalloutMatch {
                full,
                kind: CalloutMatchKind::Unescape {
                    backslash: esc.start(),
                },
            });

            continue;
        }

        let (number_raw, is_xml) = if let Some(xnum) = caps.name("xnum") {
            (xnum.as_str(), true)
        } else {
            // The regex guarantees one of `xnum` or `num` is present.
            #[allow(clippy::unwrap_used)]
            (caps.name("num").unwrap().as_str(), false)
        };

        // Auto-numbering (`<.>`) is scoped to this level's whole scan, exactly
        // as the string pipeline's `CalloutReplacer::autonum` is scoped to one
        // block, and only advances for a match that reaches this point (past
        // the lookahead and escape checks).
        let number = if number_raw == "." {
            autonum += 1;
            autonum.to_string()
        } else {
            number_raw.to_string()
        };

        let prefix = caps.name("prefix").map(|m| m.start()..m.end());

        matches.push(CalloutMatch {
            full,
            kind: CalloutMatchKind::Callout {
                number,
                is_xml,
                prefix,
            },
        });
    }

    matches
}

/// Rebuilds a level's node list from its callout matches: each gap keeps its
/// original nodes; each match becomes its unescaped literal text (an
/// [`Unescape`](CalloutMatchKind::Unescape)) or a
/// [`Callout`](InlineNode::Callout) node (a
/// [`Callout`](CalloutMatchKind::Callout) match), whose guard mirrors
/// [`CalloutReplacer`](crate::content::substitution_step)'s resolution: a
/// captured line-comment prefix takes precedence; otherwise an XML callout
/// uses the XML guard; failing both, an empty line-comment guard (there is no
/// "no guard" state — the fold's non-icon fallback always guards with
/// *something*).
fn rebuild_callout_level<'src>(
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    s: &str,
    matches: &[CalloutMatch],
    root: Span<'src>,
) -> Vec<InlineNode<'src>> {
    let mut out = Vec::new();
    let mut cursor = 0usize;

    for m in matches {
        match &m.kind {
            CalloutMatchKind::Unescape { backslash } => {
                emit_range(nodes, pieces, cursor..*backslash, &mut out);
                emit_range(nodes, pieces, backslash + 1..m.full.end, &mut out);
                cursor = m.full.end;
            }

            CalloutMatchKind::Callout {
                number,
                is_xml,
                prefix,
            } => {
                emit_range(nodes, pieces, cursor..m.full.start, &mut out);

                let guard = match prefix {
                    Some(range) => {
                        let prefix_span = source_slice(pieces, range.clone(), root);
                        CalloutGuard::LineComment(CowStr::from(prefix_span.data()))
                    }

                    None if *is_xml => CalloutGuard::Xml,

                    None => CalloutGuard::LineComment(CowStr::Borrowed("")),
                };

                out.push(InlineNode::Callout(Callout {
                    number: CowStr::from(number.clone()),
                    guard,
                    location: source_slice(pieces, m.full.clone(), root),
                }));

                cursor = m.full.end;
            }
        }
    }

    if cursor < s.len() {
        emit_range(nodes, pieces, cursor..s.len(), &mut out);
    }

    out
}

/// The inverse of the classifier's value assignment: the
/// [`CharacterReplacementType`] the fold renders a
/// [`CharRef::Replacement`](crate::inlines::CharRef::Replacement) value with.
/// The mapping is a bijection — each replacement type produces a
/// distinct logical string — so the fold reproduces the string pipeline's bytes
/// through the same [`render_character_replacement`] the step calls.
///
/// [`render_character_replacement`]:
///     crate::parser::InlineRenderer::render_character_replacement
pub(super) fn replacement_type_of(value: &str) -> Option<CharacterReplacementType> {
    Some(match value {
        "\u{a9}" => CharacterReplacementType::Copyright,
        "\u{ae}" => CharacterReplacementType::Registered,
        "\u{2122}" => CharacterReplacementType::Trademark,
        "\u{2009}\u{2014}\u{2009}" => CharacterReplacementType::EmDashSurroundedBySpaces,
        "\u{2014}\u{200b}" => CharacterReplacementType::EmDashWithoutSpace,
        "\u{2026}\u{200b}" => CharacterReplacementType::Ellipsis,
        "\u{2019}" => CharacterReplacementType::TypographicApostrophe,
        "\u{2190}" => CharacterReplacementType::SingleLeftArrow,
        "\u{21d0}" => CharacterReplacementType::DoubleLeftArrow,
        "\u{2192}" => CharacterReplacementType::SingleRightArrow,
        "\u{21d2}" => CharacterReplacementType::DoubleRightArrow,

        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use super::{
        super::{
            special_chars::apply_special_characters,
            test_support::{assert_text, fold_html, seed},
        },
        apply_callouts,
    };
    use crate::{
        Parser, Span,
        attributes::{Attrlist, AttrlistContext},
        inlines::{Callout, CalloutGuard, InlineNode},
        parser::HtmlInlineRenderer,
        strings::CowStr,
    };

    #[test]
    fn replacement_type_of_round_trips_every_variant() {
        use super::replacement_type_of;

        // Every replacement value the classifier assigns maps back to a type,
        // so the fold always routes through the renderer. An unknown
        // value is the defensive `None`.
        for value in [
            "\u{a9}",
            "\u{ae}",
            "\u{2122}",
            "\u{2009}\u{2014}\u{2009}",
            "\u{2014}\u{200b}",
            "\u{2026}\u{200b}",
            "\u{2019}",
            "\u{2190}",
            "\u{21d0}",
            "\u{2192}",
            "\u{21d2}",
        ] {
            assert!(
                replacement_type_of(value).is_some(),
                "no type for replacement value {value:?}"
            );
        }

        assert!(replacement_type_of("not a replacement").is_none());
    }

    /// Builds the tree through the **callouts** step for `source`: special
    /// characters, then callouts — the two (and only) steps
    /// [`SubstitutionGroup::Verbatim`](crate::content::SubstitutionGroup::Verbatim)(crate::content::SubstitutionGroup::Verbatim)
    /// runs.
    fn build_verbatim<'src>(
        source: Span<'src>,
        parser: &Parser,
        attrlist: Option<&Attrlist<'_>>,
    ) -> Vec<InlineNode<'src>> {
        apply_callouts(
            apply_special_characters(seed(source)),
            source,
            parser,
            attrlist,
        )
    }

    /// [`build_verbatim`] with a default parser and no attribute list.
    fn build_verbatim_src(source: Span<'_>) -> Vec<InlineNode<'_>> {
        build_verbatim(source, &Parser::default(), None)
    }

    /// The string pipeline's recorded output through the **callouts** step
    /// for `source` — what that pipeline produced while it existed:
    /// `SubstitutionGroup::Verbatim`'s two steps, in order (special
    /// characters, then callouts), frozen into `snapshots/callouts.txt`. The
    /// `_parser` and `_attrlist` no longer participate — a recording is keyed
    /// by source alone — but the parameters stay so the call sites that
    /// configured them do not churn.
    fn golden_callouts_with(
        source: &str,
        parser: &Parser,
        attrlist: Option<&Attrlist<'_>>,
    ) -> String {
        golden_callouts_in("callouts", source, parser, attrlist)
    }

    /// [`golden_callouts_with`], recording into a named corpus.
    ///
    /// One corpus is keyed by source alone, so a test whose parser makes a
    /// shared source render differently — a non-default `icons` is the one
    /// here — names its own.
    fn golden_callouts_in(
        corpus: &str,
        source: &str,
        _parser: &Parser,
        _attrlist: Option<&Attrlist<'_>>,
    ) -> String {
        crate::content::inline_builder::snapshot::recorded(corpus, source)
    }

    /// [`golden_callouts_with`] with a default parser and no attribute list.
    fn golden_callouts(source: &str) -> String {
        golden_callouts_with(source, &Parser::default(), None)
    }

    /// Collects every [`Callout`](InlineNode::Callout) node in `nodes`, in
    /// order.
    fn callouts<'a, 'src>(nodes: &'a [InlineNode<'src>]) -> Vec<&'a Callout<'src>> {
        nodes
            .iter()
            .filter_map(|n| match n {
                InlineNode::Callout(callout) => Some(callout),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn fold_matches_the_string_pipeline_through_callouts() {
        // For each fixture, folding the single-pass tree (special characters
        // then callouts) reproduces the string pipeline's output
        // byte-for-byte. This is the differential corpus (design §5.3) that
        // pins the callouts increment (part 5c).
        let fixtures = [
            "just some text",
            "a <b> c",
            "require 'x' <1>",
            "puts 'x' # <1>",
            "puts x <5><6>",
            "puts \"<1> in the middle\"",
            "a <.>\nb <.>\nc <.>",
            "a <.>\nb <1>\nc <.>",
            "<child/> <!--1-->",
            "First line <1-->",
            "require 'x' # \\<1>",
            // Trailing content survives a recognized callout that is not the
            // last thing in the level: the callout is still trailing-position
            // *on its own line* (immediately followed by a newline), but more
            // text follows on the next line, exercising the rebuild's final
            // gap after the last match.
            "line one <1>\nmore code after",
        ];

        for fixture in fixtures {
            let parser = Parser::default();

            let folded = fold_html(
                &build_verbatim(Span::new(fixture), &parser, None),
                &HtmlInlineRenderer {},
            );

            assert_eq!(
                folded,
                golden_callouts(fixture),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn fold_matches_the_string_pipeline_through_callouts_under_icons() {
        // Both non-default `icons` renderings (font and image) read `guard`
        // and `number` through the same `render_callout` the fold calls, so
        // parity holds under them too. `super::fold_html` is called directly
        // (not this module's `fold_html` test helper, which always uses a
        // default parser) so the custom parser reaches the renderer.
        use crate::parser::ModificationContext;

        let font_parser = Parser::default().with_intrinsic_attribute(
            "icons",
            "font",
            ModificationContext::Anywhere,
        );
        let image_parser =
            Parser::default().with_intrinsic_attribute("icons", "", ModificationContext::Anywhere);

        for (corpus, parser) in [
            ("callouts_icons_font", &font_parser),
            ("callouts_icons_image", &image_parser),
        ] {
            let fixture = "puts x # <1>";

            let folded = crate::content::inline_builder::fold_html(
                &build_verbatim(Span::new(fixture), parser, None),
                &HtmlInlineRenderer {},
                &parser.render_context(),
            );

            assert_eq!(
                folded,
                golden_callouts_in(corpus, fixture, parser, None),
                "fold diverged from the string pipeline under a custom `icons` attribute"
            );
        }
    }

    #[test]
    fn a_bare_callout_becomes_a_callout_node_with_an_empty_line_comment_guard() {
        let nodes = build_verbatim_src(Span::new("require 'x' <1>"));
        let found = callouts(&nodes);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].number.as_ref(), "1");
        assert_eq!(found[0].guard, CalloutGuard::LineComment(CowStr::from("")));
        assert_eq!(found[0].location.data(), "<1>");
    }

    #[test]
    fn a_line_comment_guarded_callout_captures_its_prefix() {
        let nodes = build_verbatim_src(Span::new("puts 'x' # <1>"));
        let found = callouts(&nodes);

        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].guard,
            CalloutGuard::LineComment(CowStr::from("# "))
        );
    }

    #[test]
    fn an_xml_callout_captures_the_xml_guard() {
        let nodes = build_verbatim_src(Span::new("<child/> <!--1-->"));
        let found = callouts(&nodes);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].number.as_ref(), "1");
        assert_eq!(found[0].guard, CalloutGuard::Xml);
    }

    #[test]
    fn multiple_callouts_on_one_line_are_each_their_own_node() {
        let nodes = build_verbatim_src(Span::new("puts x <5><6>"));
        let found = callouts(&nodes);

        assert_eq!(found.len(), 2);
        assert_eq!(found[0].number.as_ref(), "5");
        assert_eq!(found[1].number.as_ref(), "6");
    }

    #[test]
    fn content_after_the_last_callout_is_kept_as_a_trailing_node() {
        // The callout is trailing-position on its own line (immediately
        // followed by a newline), but more text follows on the next line, so
        // the rebuild's final gap (after the last match) is non-empty.
        let nodes = build_verbatim_src(Span::new("line one <1>\nmore code after"));
        let found = callouts(&nodes);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].number.as_ref(), "1");

        match nodes.last() {
            Some(InlineNode::Text { value, .. }) => {
                assert_eq!(value.as_ref(), "\nmore code after");
            }

            other => panic!("expected a trailing Text node, got {other:?}"),
        }
    }

    #[test]
    fn auto_numbering_advances_per_recognized_callout_in_source_order() {
        let nodes = build_verbatim_src(Span::new("a <.>\nb <.>\nc <.>"));
        let found = callouts(&nodes);

        assert_eq!(
            found.iter().map(|c| c.number.as_ref()).collect::<Vec<_>>(),
            vec!["1", "2", "3"]
        );
    }

    #[test]
    fn auto_numbering_ignores_explicit_numbers() {
        // Auto-numbering is not aware of explicit numbers.
        let nodes = build_verbatim_src(Span::new("a <.>\nb <1>\nc <.>"));
        let found = callouts(&nodes);

        assert_eq!(
            found.iter().map(|c| c.number.as_ref()).collect::<Vec<_>>(),
            vec!["1", "1", "2"]
        );
    }

    #[test]
    fn a_callout_not_at_the_trailing_position_is_left_unrecognized() {
        let nodes = build_verbatim_src(Span::new("puts \"<1> in the middle\""));

        assert!(
            callouts(&nodes).is_empty(),
            "a non-trailing callout token must not produce a node: {nodes:?}"
        );
    }

    #[test]
    fn a_half_xml_comment_is_not_a_callout() {
        let nodes = build_verbatim_src(Span::new("First line <1-->"));

        assert!(
            callouts(&nodes).is_empty(),
            "a half XML comment must not produce a callout node: {nodes:?}"
        );
    }

    #[test]
    fn an_escaped_callout_drops_its_backslash_and_stays_literal() {
        let nodes = build_verbatim_src(Span::new("require 'x' # \\<1>"));

        assert!(
            callouts(&nodes).is_empty(),
            "an escaped callout must not produce a node: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_callouts("require 'x' # \\<1>")
        );
    }

    #[test]
    fn a_custom_line_comment_prefix_from_the_attribute_list_is_honored() {
        // `line-comment=%` (Erlang). Only `%` is recognized as a prefix.
        let attrlist = Attrlist::parse(
            Span::new("source,erlang,line-comment=%"),
            &Parser::default(),
            AttrlistContext::Block,
        )
        .item
        .item;

        let source = "hello() -> % <1>";
        let nodes = build_verbatim(Span::new(source), &Parser::default(), Some(&attrlist));
        let found = callouts(&nodes);

        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].guard,
            CalloutGuard::LineComment(CowStr::from("% "))
        );

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_callouts_with(source, &Parser::default(), Some(&attrlist))
        );
    }

    #[test]
    fn an_empty_line_comment_attribute_disables_prefix_recognition() {
        // `line-comment=` (empty) disables prefix recognition, so the `--`
        // before the callout is preserved verbatim (not consumed as a
        // default line-comment prefix).
        let attrlist = Attrlist::parse(
            Span::new("source,asciidoc,line-comment="),
            &Parser::default(),
            AttrlistContext::Block,
        )
        .item
        .item;

        let source = "-- <1>";
        let nodes = build_verbatim(Span::new(source), &Parser::default(), Some(&attrlist));
        let found = callouts(&nodes);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].guard, CalloutGuard::LineComment(CowStr::from("")));
        assert_text(&nodes[0], "-- ", 1, 1);

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_callouts_with(source, &Parser::default(), Some(&attrlist))
        );
    }

    #[test]
    fn a_document_level_line_comment_attribute_is_a_fallback() {
        // The `line-comment` attribute can be set at the document level (here,
        // with no block attribute list), and is honored as a fallback.
        use crate::parser::ModificationContext;

        let parser = Parser::default().with_intrinsic_attribute(
            "line-comment",
            "%",
            ModificationContext::Anywhere,
        );

        let source = "hello() -> % <1>";
        let nodes = build_verbatim(Span::new(source), &parser, None);
        let found = callouts(&nodes);

        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].guard,
            CalloutGuard::LineComment(CowStr::from("% "))
        );

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_callouts_with(source, &parser, None)
        );
    }

    #[test]
    fn apply_callouts_is_a_noop_without_a_special_character() {
        // The cheap pre-filter returns the seed unchanged when no `&lt;` is
        // present at this level, so plain verbatim content never pays for the
        // level-building machinery.
        let source = Span::new("plain verbatim, no callouts here");
        let seeded = apply_special_characters(seed(source));

        let nodes = apply_callouts(seeded.clone(), source, &Parser::default(), None);

        assert_eq!(nodes, seeded);
    }
}
