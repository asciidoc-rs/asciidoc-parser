//! The character-replacements substitution step.

use super::{
    quotes::{
        LevelContext, Piece, build_match_string, emit_range, level_may_have_replacements,
        source_slice,
    },
    special_chars::Masked,
};
use crate::{
    Span,
    content::{CharacterReplacement, character_replacements},
    inlines::{CharRef, InlineNode},
    parser::CharacterReplacementType,
    strings::CowStr,
};

/// The character-replacements substitution, as a node transducer: each shared
/// [`character_replacements`] rule is applied to the tree in order (its order
/// encodes Asciidoctor's precedence), replacing every matched construct with a
/// [`CharRef::Replacement`] (a typographic replacement such as `(C)` → `©`) or
/// a [`CharRef::Entity`] (a restored named/numeric entity such as `&amp;copy;`
/// → `&copy;`) leaf.
///
/// The rules match over the level's **escaped** text (built by
/// [`build_match_string`], where a [`CharRef::Special`] contributes its
/// canonical entity) — which is exactly why the arrow (`-&gt;`, `&lt;-`) and
/// entity (`&amp;copy;`) rules can straddle a `Text`/`CharRef` boundary, and,
/// since a [`synthesized`](super::quotes::Piece::synthesized) run (an
/// attribute-expanded value) contributes its own `value` there too, why they
/// can straddle a `Text`/synthesized-`Text` boundary as well — pinned by
/// `attribute_refs::tests::a_replacement_straddling_a_synthesized_and_a_real_piece_is_recognized`.
/// `root` is the whole-content source span; every leaf's precise `location`
/// is sliced from it — or, for a leaf recognized *inside* a synthesized run,
/// falls back to that run's own whole span, since its bytes have no honest
/// source counterpart of their own.
pub(super) fn apply_character_replacements<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
) -> Vec<InlineNode<'src>> {
    apply_replacements_recursive(nodes, root, LevelContext::ROOT)
}

/// Applies every [`character_replacements`] rule to `nodes`, descending into
/// the [`Styled`](crate::inlines::Styled)/[`Ref`](InlineNode::Ref) children
/// earlier steps created exactly once (a replacement inside a span is
/// recognized against the *inside* of that span's own rendered tag) rather
/// than once per rule: every rule shares the same
/// [`level_may_have_replacements`] sniff, so descending, and sniffing, once
/// per level reaches the same leaves whichever rule matches there, without
/// redoing either for every rule in the `character_replacements` list in
/// turn.
///
/// `ctx` is the boundary context this level sits in ([`LevelContext`]):
/// recognition inside a span is scoped to the *inside* of its tag, which only
/// opens up once the tag's own characters are there to be read — that is what
/// decides an em dash written against either edge of a span (`*x --*` renders
/// `<strong>x --</strong>`, the `--` staying literal because `<` follows it in
/// that haystack, not the end of a line).
fn apply_replacements_recursive<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    ctx: LevelContext,
) -> Vec<InlineNode<'src>> {
    // A level with no parent node to descend into — the common leaf-only
    // case — skips the rebuild of its node vector entirely.
    let nodes = if nodes
        .iter()
        .any(|node| matches!(node, InlineNode::Styled(_) | InlineNode::Ref(_)))
    {
        nodes
            .into_iter()
            .map(|node| match node {
                InlineNode::Styled(mut styled) => {
                    let inner = LevelContext::inside_styled(&styled, ctx);
                    styled.children = apply_replacements_recursive(styled.children, root, inner);
                    InlineNode::Styled(styled)
                }

                InlineNode::Ref(mut reference) => {
                    reference.children = apply_replacements_recursive(
                        reference.children,
                        root,
                        LevelContext::INSIDE_REF,
                    );
                    InlineNode::Ref(reference)
                }

                other => other,
            })
            .collect()
    } else {
        nodes
    };

    // Cheap pre-filter, shared by every rule below: none of them can match
    // when this level has nothing replaceable at all, so this skips all of
    // them at once rather than letting each rediscover the same answer over
    // `nodes` on its own turn. `replace_level` does not repeat this check
    // itself — see its own doc comment for why one pass here already covers
    // every rule's turn.
    if !level_may_have_replacements(&nodes) {
        return nodes;
    }

    let mut nodes = nodes;

    for repl in character_replacements() {
        nodes = replace_level(repl, nodes, root, ctx);
    }

    nodes
}

/// One character-replacement match at a level, in absolute match-string byte
/// offsets.
struct ReplacementMatch {
    /// The whole match, `[start, end)`.
    full: std::ops::Range<usize>,

    /// What to emit in place of `full`.
    kind: ReplacementKind,
}

impl ReplacementMatch {
    /// Maps every offset in this match out of the
    /// [`haystack`](LevelContext::haystack) it was found in and back into the
    /// level's own match string (see [`LevelContext::unshift`]).
    fn unshift(self, prefix: usize, len: usize) -> Self {
        let map = |range: std::ops::Range<usize>| LevelContext::unshift(prefix, len, range);

        Self {
            full: map(self.full),

            kind: match self.kind {
                ReplacementKind::Unescape { backslash } => ReplacementKind::Unescape {
                    backslash: map(backslash..backslash).start,
                },

                ReplacementKind::Replace { consumed, value } => ReplacementKind::Replace {
                    consumed: map(consumed),
                    value,
                },

                ReplacementKind::Entity { name } => ReplacementKind::Entity { name: map(name) },
            },
        }
    }
}

enum ReplacementKind {
    /// An escaped construct (`\(C)`, `\-&gt;`, …): drop the single backslash at
    /// this offset and keep the rest of the match as literal nodes, replacing
    /// nothing.
    Unescape { backslash: usize },

    /// A recognized typographic replacement. Only the `consumed` sub-range
    /// becomes a [`CharRef::Replacement`] leaf carrying `value` (the logical
    /// character(s)); any word character the pattern anchors on (the `w` in
    /// `w--`, or the letters around a `w'w` apostrophe) lies outside `consumed`
    /// and is kept as literal text by the surrounding gaps.
    Replace {
        consumed: std::ops::Range<usize>,
        value: &'static str,
    },

    /// A restored character reference (`&amp;copy;`): the whole match becomes a
    /// [`CharRef::Entity`] leaf whose value is the named/numeric entity `name`
    /// wrapped as `&name;`.
    Entity { name: std::ops::Range<usize> },
}

/// Matches `repl` over this level's escaped text, replacing each match with the
/// leaf node(s) it produces and leaving everything else in place.
///
/// Takes no [`level_may_have_replacements`] pre-filter of its own: the caller
/// ([`apply_replacements_recursive`]) already took it once for `nodes` before
/// entering the rule loop this is called from, and every rule's own leaf —
/// [`Replace`](ReplacementKind::Replace),
/// [`Unescape`](ReplacementKind::Unescape),
/// or [`Entity`](ReplacementKind::Entity) alike — either keeps the matched
/// text's own trigger characters in place or produces a [`CharRef`] whose
/// [`charref_entity`](super::quotes::charref_entity) form always starts with
/// `&`, which the sniff's own `[&']` alternative always answers `true` for.
/// So once true for a level, the sniff cannot go false again for the rest of
/// that level's rule loop, and re-taking it on every rule's own turn would
/// only ever confirm what the caller already established.
fn replace_level<'src>(
    repl: &CharacterReplacement,
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    ctx: LevelContext,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes, Masked::UNKNOWN);

    // The rule runs over the level wrapped in its enclosing construct's own
    // boundary characters, and every offset it reports is mapped back into the
    // level's own coordinates (see [`LevelContext`]).
    let (haystack, prefix) = ctx.haystack(&s);

    // A match the clip emptied kept nothing of the level itself, so there is
    // nothing here to replace. No rule's pattern can produce one — each needs
    // at least two characters, and a context is one — so this is the clip's own
    // invariant rather than a case any fixture reaches.
    let matches: Vec<ReplacementMatch> = find_replacement_matches(repl, &haystack)
        .into_iter()
        .map(|m| m.unshift(prefix, s.len()))
        .filter(|m| !m.full.is_empty())
        .collect();

    if matches.is_empty() {
        return nodes;
    }

    rebuild_replacements(&nodes, &pieces, &s, &matches, root)
}

/// Finds every non-overlapping match of `repl` in the escaped match string
/// `s`, left to right.
fn find_replacement_matches(repl: &CharacterReplacement, s: &str) -> Vec<ReplacementMatch> {
    let mut matches = Vec::new();

    for caps in repl.pattern.captures_iter(s) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();

        let full = whole.start()..whole.end();

        // An escaped construct keeps its literal text with the single backslash
        // dropped, and replaces nothing.
        if let Some(rel) = whole.as_str().find('\\') {
            matches.push(ReplacementMatch {
                full,
                kind: ReplacementKind::Unescape {
                    backslash: whole.start() + rel,
                },
            });

            continue;
        }

        matches.push(ReplacementMatch {
            full: full.clone(),
            kind: classify_replacement(repl, &caps, full),
        });
    }

    matches
}

/// Classifies one non-escaped capture into the leaf it produces. Group presence
/// distinguishes the two rules that share [`CharacterReplacementType`]: a bare
/// `` `' `` apostrophe (no groups) versus a word-internal `w'w` one (two).
fn classify_replacement(
    repl: &CharacterReplacement,
    caps: &regex::Captures<'_>,
    full: std::ops::Range<usize>,
) -> ReplacementKind {
    let group = |i: usize| caps.get(i).map(|m| m.range());

    // The whole match becomes a single replacement leaf with nothing kept.
    let whole = |value| ReplacementKind::Replace {
        consumed: full.clone(),
        value,
    };

    match repl.type_ {
        CharacterReplacementType::Copyright => whole("\u{a9}"),
        CharacterReplacementType::Registered => whole("\u{ae}"),
        CharacterReplacementType::Trademark => whole("\u{2122}"),
        CharacterReplacementType::EmDashSurroundedBySpaces => whole("\u{2009}\u{2014}\u{2009}"),
        CharacterReplacementType::Ellipsis => whole("\u{2026}\u{200b}"),
        CharacterReplacementType::SingleLeftArrow => whole("\u{2190}"),
        CharacterReplacementType::DoubleLeftArrow => whole("\u{21d0}"),
        CharacterReplacementType::SingleRightArrow => whole("\u{2192}"),
        CharacterReplacementType::DoubleRightArrow => whole("\u{21d2}"),

        CharacterReplacementType::EmDashWithoutSpace => {
            // `(\w)--`: the leading word character (group 1) stays; only the
            // `--` after it is consumed.
            let before = group(1).unwrap_or(full.start..full.start);

            ReplacementKind::Replace {
                consumed: before.end..full.end,
                value: "\u{2014}\u{200b}",
            }
        }

        CharacterReplacementType::TypographicApostrophe => match (group(1), group(2)) {
            // `w'w`: the surrounding letters stay; only the apostrophe between
            // them is consumed.
            (Some(before), Some(after)) => ReplacementKind::Replace {
                consumed: before.end..after.start,
                value: "\u{2019}",
            },

            // `` `' ``: the whole match is the apostrophe.
            _ => whole("\u{2019}"),
        },

        CharacterReplacementType::CharacterReference(_) => ReplacementKind::Entity {
            name: group(1).unwrap_or(full),
        },
    }
}

/// Rebuilds a level's node list from its character-replacement matches: each
/// gap keeps its original nodes; each match becomes its kept literal text plus
/// the replacement leaf.
fn rebuild_replacements<'src>(
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    s: &str,
    matches: &[ReplacementMatch],
    root: Span<'src>,
) -> Vec<InlineNode<'src>> {
    let mut out = Vec::new();
    let mut cursor = 0usize;

    for m in matches {
        match &m.kind {
            ReplacementKind::Unescape { backslash } => {
                // Keep the whole match with the single backslash dropped.
                emit_range(nodes, pieces, cursor..*backslash, &mut out);
                emit_range(nodes, pieces, (*backslash + 1)..m.full.end, &mut out);
                cursor = m.full.end;
            }

            ReplacementKind::Replace { consumed, value } => {
                // The gap runs up to `consumed`, absorbing any kept leading
                // word character; the cursor stops at
                // `consumed.end`, so a kept trailing letter
                // (the second letter of a `w'w` apostrophe) is
                // absorbed by the next gap.
                emit_range(nodes, pieces, cursor..consumed.start, &mut out);

                out.push(InlineNode::CharRef {
                    value: CharRef::Replacement(value),
                    location: source_slice(pieces, consumed.clone(), root),
                });

                cursor = consumed.end;
            }

            ReplacementKind::Entity { name } => {
                // The entity is emitted as written (`&copy;`); its `&`/`;` come
                // from the pattern, its name from the level's escaped text.
                emit_range(nodes, pieces, cursor..m.full.start, &mut out);

                let entity = format!("&{};", &s[name.clone()]);

                out.push(InlineNode::CharRef {
                    value: CharRef::Entity(CowStr::from(entity)),
                    location: source_slice(pieces, m.full.clone(), root),
                });

                cursor = m.full.end;
            }
        }
    }

    if cursor < s.len() {
        emit_range(nodes, pieces, cursor..s.len(), &mut out);
    }

    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use super::{
        super::test_support::{assert_styled, assert_text, build_src, fold_html},
        apply_character_replacements,
    };
    use crate::{
        Span,
        inlines::{CharRef, InlineNode, Ref, RefVariant, SpanForm, StyleVariant},
        parser::HtmlInlineRenderer,
        strings::CowStr,
    };

    /// The frozen recording (see `parser/snapshots/README.md`) through the
    /// **post-replacement** step for `source`: the four steps [`build`] runs,
    /// in order (special characters, quotes, character replacements, post
    /// replacement), frozen into `snapshots/char_replacements.txt`. Attribute
    /// references and macros were skipped — exactly as the additive builder
    /// skips them — so the fixtures deliberately contain neither.
    fn golden_replacements(source: &str) -> String {
        crate::content::inline_builder::snapshot::recorded("char_replacements", source)
    }

    #[test]
    fn a_replacement_at_a_spans_own_edge_reads_that_spans_boundary_characters() {
        // A rule whose pattern reads what surrounds its match — the spaced em
        // dash's `(^|\n| )--( |\n|$)` is the one in this step — must see the
        // enclosing span's own markup, not the start or end of a level. `*x
        // --*` renders `<strong>x --</strong>`: the `--` stays literal because
        // `<` follows it there, and a level matched in isolation would take
        // its own end as the line end the rule wants.
        for source in [
            // Against either edge, in each variant's own rendering shape.
            "*-- x*",
            "*x --*",
            "_x --_",
            "`x --`",
            "#x --#",
            "[.r]#x --#",
            "^x --^",
            "~x --~",
            "**x --**",
            r#""`-- x`""#,
            r#""`x --`""#,
            r#"'`x --`'"#,
            // Away from either edge, where the rule matches inside the span
            // regardless.
            "*a -- b*",
            "_a -- b_",
            r#""`a -- b`""#,
            // The rules that read no boundary of their own are unaffected.
            "*(C) x*",
            "*x ...*",
            "*w'w*",
            "*a -> b*",
            "*&copy; x*",
            // The word-anchored em dash keeps its own leading word character
            // rather than a boundary one.
            "*one--two*",
            // And the same constructs at the content's own top level, where a
            // pattern's `^`/`$` matches the line boundaries directly.
            "-- x",
            "x --",
            "a -- b",
            // A span *beside* a replacement, where the placeholder standing in
            // for it is not the space the rule requires.
            "*x*-- y",
            "*x* -- y",
        ] {
            assert_eq!(
                golden_replacements(source),
                fold_html(&build_src(Span::new(source)), &HtmlInlineRenderer {}),
                "fold diverged from golden for {source:?}"
            );
        }
    }

    #[test]
    fn a_replacement_beside_a_transparent_span_is_a_documented_divergence() {
        // An unquoted span whose attribute list resolves to neither a role nor
        // an id renders to its body and nothing else, so its children inherit
        // the context the span itself sits in
        // ([`LevelContext::inside_styled`]). That is right whenever the span
        // is all its parent's level holds — `[width=10]#x --#` at the top
        // level replaces the dash the same way in the tree and the frozen
        // recording — and wrong when a sibling follows it, because the
        // recording's flat haystack then shows what that sibling begins with
        // (here a space, which is exactly the em dash's own trailing class)
        // where the tree shows the parent's closing markup.
        //
        // [`LevelContext::child_contexts`] derives exactly that character from
        // a level's siblings for the two steps that can take it, and this step
        // is deliberately not one of them: its one boundary-reading rule is
        // the spaced em dash, whose replacement **consumes** the spaces it
        // matches rather than writing them back. A character a sibling owns
        // lives in another level's node, which this level's rebuild cannot
        // delete — so supplying it would emit the space here *and* leave it
        // there, a differently wrong answer rather than the right one.
        //
        // Closing it means letting one level's rebuild consume a node another
        // level owns. If that lands, fold this fixture into the corpus above.
        let source = "*[width=10]#x --# --*";

        let folded = fold_html(&build_src(Span::new(source)), &HtmlInlineRenderer {});

        assert_ne!(
            folded,
            golden_replacements(source),
            "expected the documented divergence to still reproduce for {source:?}"
        );

        // The frozen recording replaces the *first* dash (a space follows it,
        // the transparent span having rendered nothing); the tree leaves both
        // literal.
        assert_eq!(folded, "<strong>x -- --</strong>");

        // The same span with nothing after it agrees, since there is no
        // sibling for the inherited context to be wrong about.
        let source = "[width=10]#x --#";

        assert_eq!(
            golden_replacements(source),
            fold_html(&build_src(Span::new(source)), &HtmlInlineRenderer {}),
        );
    }

    #[test]
    fn fold_matches_the_string_pipeline_through_replacements() {
        // For each fixture, folding the single-pass tree (special characters +
        // quotes + character replacements + post replacement) reproduces the
        // frozen recording's output byte-for-byte. This is the differential
        // corpus that pins this step.
        let fixtures = [
            // No replacements.
            "plain text",
            "a < b & c > d",
            "*bold* and _italic_",
            // Symbols.
            "(C)",
            "(R)",
            "(TM)",
            "Copyright (C) 2026, Acme (R), Widget (TM)",
            // Em dashes.
            "a -- b",
            "one--two",
            "start -- of a thought",
            "-- leading",
            "trailing --",
            "a--b--c",
            // Ellipsis.
            "wait...",
            "a...b...c",
            "...",
            // Apostrophes.
            "Sam's book",
            "it's a girls' school",
            "He said `'hello",
            // Arrows (they straddle a Text/CharRef boundary once escaped).
            "a -> b",
            "a => b",
            "a <- b",
            "a <= b",
            "if x -> y then z",
            "->",
            "<-",
            // Entity restoration.
            "&copy; 2026",
            "&#8217;",
            "&#x2019;",
            "&hellip; and &mdash;",
            // Escapes suppress the replacement.
            "\\(C)",
            "\\--",
            "a\\--b",
            "\\...",
            "\\-> arrow",
            "\\&copy;",
            "It\\'s",
            // Replacements inside spans.
            "*Acme (C)*",
            "_wait..._",
            "`a -> b`",
            "*a -- b* and _x...y_",
            // Nesting with replacements.
            "*a _b (C) c_ d*",
            // Specials adjacent to replacement triggers.
            "a<b (C) c>d",
            "&amp; then (R)",
            // Hard line breaks.
            "foo +\nbar",
            "a +\nb +\nc",
            "no break here\nsecond line",
            "line one +\nline two\nline three +\nline four",
            "trailing space but no plus \nnext",
            // A `+` and a newline are both present, but no line ends in ` +`, so
            // no break is recognized.
            "a + b\nc",
            // The content ends exactly in a break, so nothing trails the last
            // one.
            "a\nfoo +",
            // Line breaks interacting with replacements and spans.
            "Acme (C) +\nnext line",
            "*bold* +\nplain",
            "a -> b +\nc <- d",
            // Combinations.
            "(C) 2026 -- Acme's widgets... see x -> y",
            // Quote-like / replacement-like characters that do not match.
            "1 -- 2 * 3",
            "a_b_c",
        ];

        let renderer = HtmlInlineRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_src(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden_replacements(fixture),
                "fold diverged from golden for {fixture:?}"
            );
        }
    }

    /// Asserts that `node` is a [`CharRef::Replacement`] carrying `value`,
    /// located over source `data`.
    fn assert_replacement(node: &InlineNode<'_>, value: &str, data: &str) {
        match node {
            InlineNode::CharRef {
                value: CharRef::Replacement(got),
                location,
            } => {
                assert_eq!(*got, value, "replacement value");
                assert_eq!(location.data(), data, "replacement location");
            }

            other => panic!("expected CharRef::Replacement({value:?}), got {other:?}"),
        }
    }

    #[test]
    fn copyright_becomes_a_replacement_leaf() {
        let nodes = build_src(Span::new("(C)"));

        assert_eq!(nodes.len(), 1);
        // The logical value is the copyright character; the fold encodes it as
        // a numeric entity.
        assert_replacement(&nodes[0], "\u{a9}", "(C)");

        assert_eq!(fold_html(&nodes, &HtmlInlineRenderer {}), "&#169;");
    }

    #[test]
    fn an_arrow_replacement_spans_a_text_and_charref_boundary() {
        // `->` is `-` (text) followed by `&gt;` (a `CharRef::Special` from the
        // special-characters step), so the arrow rule must match across the
        // two.
        let nodes = build_src(Span::new("a -> b"));

        // "a " kept, the arrow leaf over "->", then " b".
        assert_eq!(nodes.len(), 3);
        assert_text(&nodes[0], "a ", 1, 1);
        assert_replacement(&nodes[1], "\u{2192}", "->");
        assert_text(&nodes[2], " b", 1, 5);
    }

    #[test]
    fn an_em_dash_without_space_keeps_the_leading_word_char() {
        // `(\w)--`: the leading word character is kept, the `--` consumed.
        let nodes = build_src(Span::new("one--two"));

        assert_eq!(nodes.len(), 3);
        assert_text(&nodes[0], "one", 1, 1);
        assert_replacement(&nodes[1], "\u{2014}\u{200b}", "--");
        assert_text(&nodes[2], "two", 1, 6);
    }

    #[test]
    fn a_word_apostrophe_keeps_both_letters() {
        // `w'w`: the surrounding letters are kept, the apostrophe consumed.
        let nodes = build_src(Span::new("Sam's"));

        assert_eq!(nodes.len(), 3);
        assert_text(&nodes[0], "Sam", 1, 1);
        assert_replacement(&nodes[1], "\u{2019}", "'");
        assert_text(&nodes[2], "s", 1, 5);
    }

    #[test]
    fn a_restored_entity_becomes_an_entity_leaf() {
        let nodes = build_src(Span::new("&copy; 2026"));

        assert_eq!(nodes.len(), 2);

        match &nodes[0] {
            InlineNode::CharRef {
                value: CharRef::Entity(entity),
                location,
            } => {
                assert_eq!(entity.as_ref(), "&copy;");
                // The location covers the source the entity derives from.
                assert_eq!(location.data(), "&copy;");
            }

            other => panic!("expected CharRef::Entity, got {other:?}"),
        }

        assert_text(&nodes[1], " 2026", 1, 7);

        assert_eq!(fold_html(&nodes, &HtmlInlineRenderer {}), "&copy; 2026");
    }

    #[test]
    fn an_escaped_replacement_stays_literal() {
        // `\(C)` drops the backslash and keeps `(C)` as literal text — no
        // replacement leaf.
        let nodes = build_src(Span::new("\\(C)"));

        assert!(
            nodes
                .iter()
                .all(|n| !matches!(n, InlineNode::CharRef { .. })),
            "an escaped replacement must not produce a char-ref leaf: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_replacements("\\(C)")
        );
    }

    #[test]
    fn a_replacement_inside_a_span_is_recognized() {
        // A `(C)` inside a strong span is replaced the same way it would be at
        // the top level.
        let nodes = build_src(Span::new("*Acme (C)*"));

        let children = assert_styled(&nodes[0], StyleVariant::Strong, SpanForm::Constrained);
        assert_eq!(children.len(), 2);
        assert_text(&children[0], "Acme ", 1, 2);
        assert_replacement(&children[1], "\u{a9}", "(C)");
    }

    #[test]
    fn character_replacements_recurse_into_ref_children() {
        // A reference's display text is subject to replacements just like any
        // other span's. (This constructs the `Ref` node directly, without
        // running the macros step, to drive the recursion in isolation.)
        let loc = Span::new("(C)");

        let reference = InlineNode::Ref(Ref {
            variant: RefVariant::Link,
            link_form: Some(crate::inlines::LinkForm::Macro),
            target: CowStr::from("https://example.com"),
            children: vec![InlineNode::Text {
                value: CowStr::from(loc.data()),
                location: loc,
            }],
            roles: vec![],
            window: None,
            resolved: None,
            derived: None,
            xrefstyle: None,
            attrs: crate::attributes::Attrlist::empty(loc.slice(0..0)),
            location: loc,
        });

        let out = apply_character_replacements(vec![reference], loc);

        assert_eq!(out.len(), 1);

        match &out[0] {
            InlineNode::Ref(reference) => {
                assert_eq!(reference.children.len(), 1);
                assert_replacement(&reference.children[0], "\u{a9}", "(C)");
            }

            other => panic!("expected Ref, got {other:?}"),
        }
    }
    #[test]
    fn a_later_rule_never_splits_an_earlier_replacement_leaf() {
        // Now that a `CharRef::Replacement` leaf contributes its rendered
        // entity to the match string, a *later* rule in the ordered sweep sees
        // those bytes too. What such a rule must never do is match
        // *partially* into one: `rebuild_replacements`' gap would then clone
        // the whole atomic leaf and emit the new leaf beside it, duplicating
        // bytes that should appear once.
        //
        // It cannot, and the reason is structural. Every entity this table
        // produces is `&#…;` — only `&`, `#`, digits and a terminating `;` —
        // while every rule that could still run needs a character from outside
        // that set immediately adjacent to its anchor: `(\w)--` needs a word
        // character *directly* before the dashes (the entity's last byte is
        // `;`, which is not one), the copyright/registered/trademark rules need
        // parens, the ellipsis needs dots, the apostrophe rules need `'`, the
        // arrows need a `-`/`=` beside `&lt;`/`&gt;`, and the entity-restore
        // rule needs the literal `&amp;`. So a match can abut a replacement
        // leaf but never begin inside one.
        //
        // This pins that as behavior rather than as an argument: every
        // replacement-producing token is glued to every token that could
        // extend a match, in both orders and with a word character between, and
        // each pairing must fold to exactly what the frozen recording holds
        // for it.
        let replacements = [
            "(C)",
            "(R)",
            "(TM)",
            "a--",
            " -- ",
            "...",
            "x'y",
            "->",
            "=>",
            "<-",
            "<=",
            "&amp;copy;",
        ];

        let neighbors = [
            "--",
            "a--",
            " -- ",
            "...",
            "'",
            "x'y",
            "->",
            "<-",
            "&amp;copy;",
            "9",
            "a",
        ];

        for replacement in replacements {
            for neighbor in neighbors {
                for source in [
                    format!("x{replacement}{neighbor}y"),
                    format!("x{neighbor}{replacement}y"),
                    format!("x{replacement}z{neighbor}y"),
                ] {
                    let nodes = build_src(Span::new(&source));

                    assert_eq!(
                        fold_html(&nodes, &HtmlInlineRenderer {}),
                        golden_replacements(&source),
                        "fold diverged from golden for {source:?}"
                    );
                }
            }
        }
    }
}
