//! The attribute-references substitution step.

use std::collections::HashMap;

use super::{
    passthrough_step::is_special,
    quotes::{Piece, build_match_string, emit_range, source_slice},
};
use crate::{
    Parser, Span, content::ATTRIBUTE_REFERENCE, document::InterpretedValue, inlines::InlineNode,
    parser::attribute_lookup_name, strings::CowStr,
};

/// The attribute-references substitution, as a node transducer: descends into
/// [`Styled`](crate::inlines::Styled) children (a reference inside a rendered
/// span is recognized just as the string pipeline recognizes one inside
/// rendered markup), then matches and splices at this level.
///
/// It reuses the string pipeline's *exact* recognition –
/// [`ATTRIBUTE_REFERENCE`] is now shared `pub(crate)` – so only the
/// recognition *sink* differs (§4.1): a resolved reference's value is spliced
/// into the node stream, classified by [`split_attribute_value`] (design
/// §3.4.1) rather than written into a `&mut String`.
///
/// A `counter`/`counter2` directive (`{counter:name}`, `{counter2:name:seed}`)
/// is recognized like any other reference: it resolves *and advances* the
/// named document counter via [`Parser::counter`] – the same required side
/// effect [`apply_footnotes`](super::footnotes::apply_footnotes) performs for
/// footnote numbering, and for the same reason it cannot be deferred to the
/// cutover the way every other macro family's catalog/warning side effect is
/// (skipping it would leave the directive's own digits wrong, not just an
/// absent catalog entry). `counter` splices the advanced value in (classified
/// by [`split_attribute_value`] exactly like a plain reference's value);
/// `counter2` advances silently and splices nothing.
///
/// One form is still deferred, documented and pinned by a divergence test: a
/// reference to a **missing** attribute under [`AttributeMissing::Drop`] /
/// [`AttributeMissing::DropLine`] (whose output *removes* content, unlike
/// leaving the reference literal – the behavior this step *does* reproduce,
/// since it is also what [`AttributeMissing::Skip`] (the default) and
/// [`AttributeMissing::Warn`] do). It is left **unrecognized**: no match is
/// produced, so the surrounding gap logic reproduces the source text
/// unchanged, exactly as an unrecognized macro is left for a later increment.
///
/// A **character replacement** *inside* an expanded value ((C) → © and
/// friends) is, by contrast, recognized: [`build_match_string`] (shared by
/// [`apply_character_replacements`](super::char_replacements::apply_character_replacements),
/// [`apply_macros`](super::macros::apply_macros), and
/// [`apply_quotes`](super::quotes::apply_quotes)) now contributes a
/// synthesized run's own `value` to the match string too – flagged
/// [`synthesized`](super::quotes::Piece::synthesized) rather than opaque –
/// so `apply_character_replacements`'s pattern sweep, still ahead in the
/// effective order, can match inside it exactly as it would over any other
/// run (a follow-up to this step, closing the gap this doc comment used to
/// describe). A **macro** *inside* an expanded value remains deferred,
/// though: a macro node bakes its target/attribute list straight from an
/// honest `'src` slice, which a synthesized run – its bytes have no source
/// counterpart of their own – cannot supply; see
/// [`range_is_verbatim`](super::macros::image::range_is_verbatim), which
/// every macro family gates recognition on, and which rejects a synthesized
/// piece for exactly this reason.
///
/// [`AttributeMissing::Drop`]: crate::content::AttributeMissing::Drop
/// [`AttributeMissing::DropLine`]: crate::content::AttributeMissing::DropLine
/// [`AttributeMissing::Skip`]: crate::content::AttributeMissing::Skip
/// [`AttributeMissing::Warn`]: crate::content::AttributeMissing::Warn
///
/// A `counter`/`counter2` directive's advance must happen in true left-to-right
/// *document* order even though the splicing recursion below visits a
/// [`Styled`](crate::inlines::Styled) child's content *before* its own level
/// (so a later sub can match *inside* an earlier span – design note on
/// [`apply_quotes`](super::quotes::apply_quotes)). Left uncorrected, that
/// would advance a directive nested in an earlier-positioned span *after* one
/// that sits later in the same source but outside any span.
/// [`resolve_counters`] runs first, as a dedicated pass that interleaves this
/// level's own matches with a `Styled` sibling's nested ones by source
/// position, and records each directive's resolved value keyed by its absolute
/// source byte offset; the splicing recursion then looks values up by that same
/// key regardless of the order it happens to visit levels in.
pub(super) fn apply_attribute_references<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    let mut counters = HashMap::new();
    resolve_counters(&nodes, root, parser, &mut counters);

    apply_attribute_references_recursive(nodes, root, parser, &counters)
}

fn apply_attribute_references_recursive<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
    counters: &HashMap<usize, String>,
) -> Vec<InlineNode<'src>> {
    let nodes: Vec<InlineNode<'src>> = nodes
        .into_iter()
        .map(|node| match node {
            InlineNode::Styled(mut styled) => {
                styled.children =
                    apply_attribute_references_recursive(styled.children, root, parser, counters);
                InlineNode::Styled(styled)
            }

            other => other,
        })
        .collect();

    attribute_references_level(nodes, root, parser, counters)
}

/// Resolves every `counter`/`counter2` directive in `nodes` (recursively,
/// including inside a [`Styled`](crate::inlines::Styled) child's own content)
/// via [`Parser::counter`], in genuine left-to-right document order, and
/// records each one's advanced value in `out`, keyed by the directive's
/// absolute source byte offset.
///
/// At each level this merges two kinds of event by source position – a
/// counter match found directly at this level, and a `Styled` sibling's own
/// placeholder position (a recursion point) – so a directive nested inside an
/// *earlier* sibling span is resolved before a *later* plain-text directive
/// at this same level, and vice versa. See [`apply_attribute_references`]'s
/// doc comment for why this must be a separate pass from the splicing
/// recursion.
fn resolve_counters<'nodes, 'src>(
    nodes: &'nodes [InlineNode<'src>],
    root: Span<'src>,
    parser: &Parser,
    out: &mut HashMap<usize, String>,
) {
    let (s, pieces) = build_match_string(nodes);

    // A counter match's `name`/`seed` borrow from `s`, a local match string
    // that does not outlive this call, so they are owned rather than
    // borrowed; a `Recurse` event's `children` borrows from `nodes` itself
    // (`'nodes`), a distinct, longer-lived borrow.
    enum Event<'nodes, 'src> {
        Counter {
            start: usize,
            name: String,
            seed: Option<String>,
        },
        Recurse {
            start: usize,
            children: &'nodes [InlineNode<'src>],
        },
    }

    let mut events: Vec<Event<'nodes, 'src>> = Vec::new();

    if s.contains('{') {
        for caps in ATTRIBUTE_REFERENCE.captures_iter(&s) {
            // An escaped directive (`\{counter:n}`) does not advance.
            if caps.get(1).is_some() || caps.get(5).is_some() {
                continue;
            }

            if caps.get(2).is_none() {
                continue;
            }

            #[allow(clippy::unwrap_used)]
            let expr = caps.get(3).unwrap().as_str();
            #[allow(clippy::unwrap_used)]
            let start = caps.get(0).unwrap().start();

            let mut parts = expr.splitn(2, ':');
            let name = parts.next().unwrap_or_default().to_string();
            let seed = parts.next().map(str::to_string);

            events.push(Event::Counter { start, name, seed });
        }
    }

    for (node, piece) in nodes.iter().zip(&pieces) {
        if let InlineNode::Styled(styled) = node {
            events.push(Event::Recurse {
                start: piece.s_start,
                children: &styled.children,
            });
        }
    }

    // Both sources are already individually sorted by position (regex matches
    // are found left to right; a piece's `s_start` strictly increases as
    // `nodes` is walked in order), so this merges rather than reorders either.
    events.sort_by_key(|event| match event {
        Event::Counter { start, .. } | Event::Recurse { start, .. } => *start,
    });

    for event in events {
        match event {
            Event::Counter { start, name, seed } => {
                let value = parser.counter(&name, seed.as_deref());
                let offset = source_slice(&pieces, start..start, root).byte_offset();
                out.insert(offset, value);
            }

            Event::Recurse { children, .. } => {
                resolve_counters(children, root, parser, out);
            }
        }
    }
}

/// One attribute-reference match at a level, in absolute match-string byte
/// offsets.
struct AttributeMatch {
    /// The whole match, `[start, end)`.
    full: std::ops::Range<usize>,

    kind: AttributeMatchKind,
}

enum AttributeMatchKind {
    /// An escaped reference (`\{name}`, `{name\}`, `\{name\}`): drop the
    /// escaping backslash(es) and keep the rest of the match as literal
    /// text, replacing nothing – mirroring the string replacer's
    /// `caps[1]`/`caps[5]` branch. One or two absolute offsets, ascending.
    Unescape { backslashes: Vec<usize> },

    /// A reference to a set attribute: its `value` (already resolved from
    /// `parser`) is spliced in, classified by [`split_attribute_value`]. An
    /// `InterpretedValue::Set`/`::Unset` attribute resolves to an empty
    /// `value`, mirroring the string replacer (whose behavior for those two
    /// kinds the language leaves unclear – see `AttributeReplacer`).
    Expand { value: String },

    /// A `counter`/`counter2` directive: the named counter's advanced value is
    /// looked up from [`resolve_counters`]'s output (keyed by this match's
    /// absolute source offset), *not* resolved here – see
    /// [`apply_attribute_references`]'s doc comment for why resolution must
    /// happen as a separate, document-order pass. `counter` splices the
    /// looked-up value in, classified by [`split_attribute_value`];
    /// `counter2` (`display: false`) advances silently and splices nothing.
    Counter { display: bool },
}

/// Matches [`ATTRIBUTE_REFERENCE`] over this level's escaped text, replacing
/// each recognized match with the node(s) it produces and leaving everything
/// else in place. `counters` supplies each `counter`/`counter2` directive's
/// already-resolved value (see [`resolve_counters`]).
fn attribute_references_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
    counters: &HashMap<usize, String>,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes);

    // Cheap pre-filter: skip the pattern sweep when no reference can be
    // present at this level.
    if !s.contains('{') {
        return nodes;
    }

    let matches = find_attribute_matches(&s, parser);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_attribute_level(&nodes, &pieces, &s, &matches, root, counters)
}

/// Finds every non-overlapping [`ATTRIBUTE_REFERENCE`] match in the escaped
/// match string `s`, left to right, exactly as the string pipeline's
/// `replace_all` does. A `counter`/`counter2` directive is recorded as a
/// match here but not yet resolved (see [`AttributeMatchKind::Counter`]); a
/// reference to a missing attribute is left out of the returned list
/// entirely – see the deferred forms documented on
/// [`apply_attribute_references`].
fn find_attribute_matches(s: &str, parser: &Parser) -> Vec<AttributeMatch> {
    let mut matches = Vec::new();

    for caps in ATTRIBUTE_REFERENCE.captures_iter(s) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();
        let full = whole.start()..whole.end();

        let backslashes: Vec<usize> = [caps.get(1), caps.get(5)]
            .into_iter()
            .flatten()
            .map(|m| m.start())
            .collect();

        if !backslashes.is_empty() {
            matches.push(AttributeMatch {
                full,
                kind: AttributeMatchKind::Unescape { backslashes },
            });

            continue;
        }

        // A `counter`/`counter2` directive resolves *and advances* the named
        // counter rather than looking up an existing attribute – mirroring
        // `AttributeReplacer`'s own counter branch exactly, including which
        // directive spelling displays the new value. The value itself was
        // already resolved by `resolve_counters`.
        if let Some(directive) = caps.get(2) {
            matches.push(AttributeMatch {
                full,
                kind: AttributeMatchKind::Counter {
                    display: directive.as_str() == "counter",
                },
            });

            continue;
        }

        // Otherwise this is a plain attribute reference (group 4).
        #[allow(clippy::unwrap_used)]
        let attr_name = caps.get(4).unwrap().as_str();
        let lookup_name = attribute_lookup_name(attr_name);

        if !parser.has_attribute(&lookup_name) {
            // Left unrecognized – correct parity under the default
            // (`AttributeMissing::Skip`) and `AttributeMissing::Warn` modes,
            // a documented divergence under `AttributeMissing::Drop` /
            // `AttributeMissing::DropLine` (see the doc comment above).
            continue;
        }

        let value = match parser.attribute_value(&lookup_name) {
            InterpretedValue::Value(value) => value,
            InterpretedValue::Set | InterpretedValue::Unset => String::new(),
        };

        matches.push(AttributeMatch {
            full,
            kind: AttributeMatchKind::Expand { value },
        });
    }

    matches
}

/// Rebuilds a level's node list from its attribute-reference matches: each gap
/// keeps its original nodes; each match becomes its unescaped literal text (an
/// [`Unescape`](AttributeMatchKind::Unescape)) or its classified expansion (an
/// [`Expand`](AttributeMatchKind::Expand)). `counters` supplies each
/// [`Counter`](AttributeMatchKind::Counter) match's already-resolved value.
fn rebuild_attribute_level<'src>(
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    s: &str,
    matches: &[AttributeMatch],
    root: Span<'src>,
    counters: &HashMap<usize, String>,
) -> Vec<InlineNode<'src>> {
    let mut out = Vec::new();
    let mut cursor = 0usize;

    for m in matches {
        match &m.kind {
            AttributeMatchKind::Unescape { backslashes } => {
                // Keep the match's literal text, dropping each escaping
                // backslash in turn. `piece_cursor` starts at `cursor`
                // (before the match), so the first `emit_range` call also
                // absorbs the untouched gap ahead of it, exactly as
                // `rebuild_replacements`'s `Unescape` arm does.
                let mut piece_cursor = cursor;

                for &backslash in backslashes {
                    emit_range(nodes, pieces, piece_cursor..backslash, &mut out);
                    piece_cursor = backslash + 1;
                }

                emit_range(nodes, pieces, piece_cursor..m.full.end, &mut out);
                cursor = m.full.end;
            }

            AttributeMatchKind::Expand { value } => {
                emit_range(nodes, pieces, cursor..m.full.start, &mut out);

                let location = source_slice(pieces, m.full.clone(), root);
                split_attribute_value(value, location, &mut out);

                cursor = m.full.end;
            }

            AttributeMatchKind::Counter { display } => {
                emit_range(nodes, pieces, cursor..m.full.start, &mut out);

                // `counter2` advances silently: the match is consumed but
                // splices no node, mirroring `AttributeReplacer`'s own
                // directive-name check.
                if *display {
                    let location = source_slice(pieces, m.full.clone(), root);

                    // `resolve_counters` records a value for every counter
                    // match this same regex finds, keyed by this exact
                    // absolute offset, so the lookup always hits; an empty
                    // fallback only guards against the two passes somehow
                    // disagreeing, which would itself be a bug.
                    let value = counters
                        .get(&location.byte_offset())
                        .map(String::as_str)
                        .unwrap_or_default();
                    split_attribute_value(value, location, &mut out);
                }

                cursor = m.full.end;
            }
        }
    }

    if cursor < s.len() {
        emit_range(nodes, pieces, cursor..s.len(), &mut out);
    }

    out
}

/// Splits a resolved attribute `value` into [`Text`](InlineNode::Text) and
/// [`Raw`](InlineNode::Raw) runs (design §3.4.1). By the time this step runs,
/// [`apply_special_characters`](super::special_chars::apply_special_characters)
/// has already run and will not run again over spliced-in content, so a literal
/// `<`, `>`, or `&` in `value` must **not** be re-escaped by the fold: it
/// becomes a `Raw` leaf (verbatim). Everything else stays `Text` – logical
/// content that design §3.4.1 says
/// [`apply_character_replacements`](super::char_replacements::apply_character_replacements) and [`apply_macros`](super::macros::apply_macros) (still ahead in the
/// effective order) should recognize normally (a `(C)` in the value becomes
/// a `CharRef`, a `link:` in it becomes a `Ref`) – true today for the former
/// (a follow-up to this step extended [`build_match_string`] to look inside a
/// synthesized run, so no change was needed here), still deferred for the
/// latter (see [`apply_attribute_references`]'s doc comment for why).
///
/// Both node kinds carry the reference's own `location` as their coarse
/// fallback span (design §4.4: a synthesized value has no source of its own).
/// A run is never emitted empty, and an empty `value` (an
/// `InterpretedValue::Set`/`::Unset` attribute) emits no node at all.
fn split_attribute_value<'src>(value: &str, location: Span<'src>, out: &mut Vec<InlineNode<'src>>) {
    let mut rest = value;

    while let Some(pos) = rest.find(is_special) {
        if pos > 0 {
            out.push(InlineNode::Text {
                value: CowStr::from(rest[..pos].to_string()),
                location,
            });
        }

        // The three specials are ASCII, so the match is exactly one byte
        // wide.
        let ch = rest[pos..].chars().next().unwrap_or('\u{FFFD}');

        out.push(InlineNode::Raw {
            value: CowStr::from(ch.to_string()),
            location,
        });

        rest = &rest[pos + 1..];
    }

    if !rest.is_empty() {
        out.push(InlineNode::Text {
            value: CowStr::from(rest.to_string()),
            location,
        });
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use super::super::test_support::{assert_raw, assert_styled, assert_text, fold_html};
    use crate::{
        Parser, Span,
        content::{Content, SubstitutionStep, inline_builder::build},
        inlines::{InlineNode, SpanForm, StyleVariant},
        parser::HtmlSubstitutionRenderer,
    };

    /// A default parser with `name` set to `value` (an
    /// [`InterpretedValue::Value`]).
    fn parser_with_attribute(name: &str, value: &str) -> Parser {
        use crate::parser::ModificationContext;

        Parser::default().with_intrinsic_attribute(name, value, ModificationContext::Anywhere)
    }

    /// The string pipeline's output through the **attribute-references** step
    /// for `source`, run against `parser`, used as the golden oracle: the six
    /// steps [`build`] runs, in order (special characters, quotes, attribute
    /// references, character replacements, macros, post replacement).
    fn golden_attributes_with(source: &str, parser: &Parser) -> String {
        let mut content = Content::from(Span::new(source));
        SubstitutionStep::SpecialCharacters.apply(&mut content, parser, None);
        SubstitutionStep::Quotes.apply(&mut content, parser, None);
        SubstitutionStep::AttributeReferences.apply(&mut content, parser, None);
        SubstitutionStep::CharacterReplacements.apply(&mut content, parser, None);
        SubstitutionStep::Macros.apply(&mut content, parser, None);
        SubstitutionStep::PostReplacement.apply(&mut content, parser, None);
        content.rendered_str().to_string()
    }

    #[test]
    fn fold_matches_the_string_pipeline_through_attribute_references() {
        // For each fixture, folding the single-pass tree (all six steps)
        // reproduces the string pipeline's output byte-for-byte. This is the
        // differential corpus (design §5.3) that pins the attribute-references
        // increment (part 5b).
        let fixtures = [
            // No reference despite brace-ish characters.
            "plain text without a reference",
            "a lone { brace and a lone } brace",
            "an empty pair {}",
            // A reference to a missing attribute, under the default
            // (`AttributeMissing::Skip`) mode, stays literal.
            "a reference to {undefined-thing} here",
            // Escapes: the reference stays literal, minus the backslash(es) –
            // whether or not the attribute is set.
            "\\{set-name} stays literal",
            "{set-name\\} stays literal",
            "\\{set-name\\} stays literal",
            "\\{undefined-thing} stays literal",
            // Multiple references on one run, and one next to plain text.
            "{greeting}, {greeting}!",
            "before{greeting}after",
            // A reference inside an already-recognized quoted span.
            "*{greeting}*",
            "_{greeting} text_",
            // Character-class coverage matching `ATTRIBUTE_REFERENCE`'s `\w`:
            // digits, `-`, and a non-ASCII (Unicode word) name.
            "{a-name-2}",
        ];

        for fixture in fixtures {
            let parser = parser_with_attribute("greeting", "Hello")
                .with_intrinsic_attribute(
                    "set-name",
                    "value",
                    crate::parser::ModificationContext::Anywhere,
                )
                .with_intrinsic_attribute(
                    "a-name-2",
                    "value",
                    crate::parser::ModificationContext::Anywhere,
                );

            let folded = fold_html(
                &build(Span::new(fixture), &parser, None),
                &HtmlSubstitutionRenderer {},
            );

            assert_eq!(
                folded,
                golden_attributes_with(fixture, &parser),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }

        // A reference expanding to a literal special character emits it
        // unescaped (`Raw`), exactly as design §3.4.1 requires.
        let special_fixtures = [("tag", "<b>", "a {tag} value"), ("amp", "A & B", "{amp}")];

        for (name, value, fixture) in special_fixtures {
            let parser = parser_with_attribute(name, value);

            let folded = fold_html(
                &build(Span::new(fixture), &parser, None),
                &HtmlSubstitutionRenderer {},
            );

            assert_eq!(
                folded,
                golden_attributes_with(fixture, &parser),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }

        // A `Set`/`Unset` attribute (no textual value) expands to nothing.
        use crate::parser::ModificationContext;
        let bool_parser = Parser::default()
            .with_intrinsic_attribute_bool("flag-on", true, ModificationContext::Anywhere)
            .with_intrinsic_attribute_bool("flag-off", false, ModificationContext::Anywhere);

        for fixture in ["before{flag-on}after", "before{flag-off}after"] {
            let folded = fold_html(
                &build(Span::new(fixture), &bool_parser, None),
                &HtmlSubstitutionRenderer {},
            );

            assert_eq!(
                folded,
                golden_attributes_with(fixture, &bool_parser),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn a_reference_to_a_set_attribute_expands_to_a_text_node() {
        let parser = parser_with_attribute("greeting", "Hello");
        let nodes = build(Span::new("say {greeting}!"), &parser, None);

        // [Text("say "), Text("Hello") (synthesized), Text("!")].
        assert_eq!(nodes.len(), 3);
        assert_text(&nodes[0], "say ", 1, 1);

        match &nodes[1] {
            InlineNode::Text { value, location } => {
                assert_eq!(value.as_ref(), "Hello");
                // A synthesized value has no source of its own, so it falls
                // back to the whole reference's span (design §4.4).
                assert_eq!(location.data(), "{greeting}");
            }

            other => panic!("expected Text(\"Hello\"), got {other:?}"),
        }

        assert_text(&nodes[2], "!", 1, 15);
    }

    #[test]
    fn a_reference_to_a_missing_attribute_stays_literal() {
        let nodes = build(Span::new("{undefined-thing}"), &Parser::default(), None);

        assert_eq!(nodes.len(), 1);
        assert_text(&nodes[0], "{undefined-thing}", 1, 1);
    }

    #[test]
    fn an_escaped_reference_drops_the_backslash() {
        // The kept text is emitted as whatever nodes cover its (possibly
        // split, around the dropped backslash) source range, so this asserts
        // on the *fold*, not node count/shape – the same choice
        // `an_escaped_quote_wraps_nothing` makes for the quotes step.
        for (source, expected) in [
            ("\\{name}", "{name}"),
            ("{name\\}", "{name}"),
            ("\\{name\\}", "{name}"),
            ("\\{counter:x}", "{counter:x}"),
        ] {
            let nodes = build(Span::new(source), &Parser::default(), None);

            assert!(
                nodes.iter().all(|n| matches!(n, InlineNode::Text { .. })),
                "for {source:?}: {nodes:?}"
            );
            assert_eq!(
                fold_html(&nodes, &HtmlSubstitutionRenderer {}),
                expected,
                "for {source:?}"
            );
        }
    }

    #[test]
    fn a_reference_expanding_to_a_special_character_becomes_raw() {
        let parser = parser_with_attribute("tag", "<b>");
        let nodes = build(Span::new("{tag}"), &parser, None);

        // The expansion splits into `Raw("<")`, `Text("b")`, `Raw(">")` –
        // design §3.4.1's mix of node kinds, since `specialcharacters` has
        // already run and will not re-escape this spliced-in content.
        assert_eq!(nodes.len(), 3);
        let raw_location = assert_raw(&nodes[0], "<");

        match &nodes[1] {
            InlineNode::Text { value, location } => {
                assert_eq!(value.as_ref(), "b");
                // A synthesized value has no source of its own, so every
                // sub-node falls back to the whole reference's span (design
                // §4.4) – the same coarse fallback as `raw_location`.
                assert_eq!(*location, raw_location);
            }

            other => panic!("expected Text(\"b\"), got {other:?}"),
        }

        assert_raw(&nodes[2], ">");

        // The fold emits the tag verbatim, unescaped.
        assert_eq!(fold_html(&nodes, &HtmlSubstitutionRenderer {}), "<b>");
    }

    #[test]
    fn a_replacement_inside_an_expanded_value_is_recognized() {
        // Design §3.4.1 says `replacements` still runs over an expanded
        // value, so a `(C)` inside it becomes a `CharRef` – closing the gap
        // `build_match_string` documented as a follow-up: a synthesized
        // `Text` piece now contributes its own `value` to the match string
        // (flagged [`synthesized`](super::super::quotes::Piece::synthesized)
        // rather than opaque), so `apply_character_replacements`'s pattern
        // sweep can match inside it exactly as it would over any other run.
        let parser = parser_with_attribute("note", "(C) 2024");
        let nodes = build(Span::new("{note}"), &parser, None);

        assert!(
            nodes
                .iter()
                .any(|n| matches!(n, InlineNode::CharRef { .. })),
            "a replacement inside a spliced value must be recognized: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_attributes_with("{note}", &parser),
        );
    }

    #[test]
    fn a_replacement_straddling_a_synthesized_and_a_real_piece_is_recognized() {
        // The em-dash-without-space rule (`(\w)--`) needs its leading word
        // character from one piece and its `--` from the next – exercised
        // here across the boundary between a synthesized (attribute-expanded)
        // piece and the real, verbatim text that follows it, the case that
        // first exposed `s_to_src`'s own edge-vs-interior bug (see
        // `quotes::tests::s_to_src_resolves_a_synthesized_pieces_own_edges_exactly`).
        let parser = parser_with_attribute("word", "hello");
        let source = "{word}--world";
        let nodes = build(Span::new(source), &parser, None);

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_attributes_with(source, &parser),
        );
    }

    #[test]
    fn a_construct_immediately_after_a_synthesized_run_keeps_its_own_location() {
        // Regression coverage for the bug the `{sp}`-then-`image:` fixture in
        // `macros::image::tests::matches_the_golden_pipelines_registration_for_a_broad_fixture_set`
        // first caught: a construct recognized by a *later* step
        // (`apply_character_replacements`, standing in for any step that
        // shares `build_match_string`) immediately after a synthesized piece
        // must not have its own location swallow the synthesized run's
        // source bytes.
        let parser = parser_with_attribute("word", "hello");
        let nodes = build(Span::new("{word}(C)"), &parser, None);

        let replacement = nodes
            .iter()
            .find(|n| matches!(n, InlineNode::CharRef { .. }))
            .unwrap_or_else(|| panic!("expected a CharRef among {nodes:?}"));

        match replacement {
            InlineNode::CharRef { location, .. } => {
                assert_eq!(location.data(), "(C)", "{nodes:?}");
            }

            other => panic!("expected CharRef, got {other:?}"),
        }
    }

    #[test]
    fn a_macro_inside_an_expanded_value_is_a_documented_divergence() {
        // The same boundary as
        // `a_replacement_inside_an_expanded_value_is_a_documented_divergence`,
        // for the macros step.
        let parser = parser_with_attribute("linktext", "link:https://example.org[Example]");
        let nodes = build(Span::new("see {linktext} now"), &parser, None);

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "a macro inside a spliced value must not yet be recognized: {nodes:?}"
        );

        assert!(golden_attributes_with("see {linktext} now", &parser).contains("<a href"));
    }

    #[test]
    fn a_reference_inside_a_span_is_recognized() {
        let parser = parser_with_attribute("greeting", "Hello");
        let nodes = build(Span::new("*{greeting}*"), &parser, None);

        assert_eq!(nodes.len(), 1);
        let children = assert_styled(&nodes[0], StyleVariant::Strong, SpanForm::Constrained);

        assert_eq!(children.len(), 1);
        match &children[0] {
            InlineNode::Text { value, .. } => assert_eq!(value.as_ref(), "Hello"),
            other => panic!("expected Text(\"Hello\"), got {other:?}"),
        }
    }

    #[test]
    fn fold_matches_the_string_pipeline_through_counter_directives() {
        // For each fixture, folding the single-pass tree reproduces the
        // string pipeline's output byte-for-byte. Each fixture uses its own
        // pair of *independent* default parsers (one for `build`, one for
        // `golden_attributes_with`), so the counter each one advances never
        // crosses over – the same test-independence footnote numbering needs
        // (see `fold_matches_the_string_pipeline_through_footnotes` in
        // `footnotes.rs`). As long as both recognize the same occurrences in
        // the same left-to-right order, their numbering stays in lockstep.
        let fixtures = [
            // `counter` displays the advanced value; a repeat reference to the
            // same name keeps advancing.
            "{counter:n}",
            "{counter:n}-{counter:n}",
            // `counter2` advances silently.
            "{counter2:n}{counter:n}",
            "{counter2:n}",
            // A seed supplies the first value; a later reference ignores it
            // (the counter is already set).
            "{counter:n:9}",
            "{counter:n:9}-{counter:n:1}",
            // Independent counter names track separately.
            "{counter:a}-{counter:b}-{counter:a}",
            // Next to plain text, and inside a rendered span.
            "page {counter:page} of many",
            "*{counter:n}*",
            // A directive outside a span and one nested inside it, in both
            // relative orders – regression coverage for `resolve_counters`'s
            // document-order pass (see
            // `a_counter_directive_advances_in_true_document_order_across_a_span_boundary`).
            "{counter:n} *{counter:n}*",
            "*{counter:n}* {counter:n}",
            // An escaped directive stays literal and does not advance.
            "\\{counter:n} {counter:n}",
        ];

        for fixture in fixtures {
            let folded = fold_html(
                &build(Span::new(fixture), &Parser::default(), None),
                &HtmlSubstitutionRenderer {},
            );

            assert_eq!(
                folded,
                golden_attributes_with(fixture, &Parser::default()),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn a_counter_directive_advances_and_displays_the_new_value() {
        let nodes = build(
            Span::new("{counter:n}-{counter:n}"),
            &Parser::default(),
            None,
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            "1-2",
            "{nodes:?}"
        );
    }

    #[test]
    fn a_counter2_directive_advances_without_displaying() {
        let nodes = build(
            Span::new("{counter2:n}{counter:n}"),
            &Parser::default(),
            None,
        );

        // `counter2:n` advances the counter to `1` and splices no node;
        // `counter:n` then advances it again and displays `2`.
        assert_eq!(nodes.len(), 1);
        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            "2",
            "{nodes:?}"
        );
    }

    #[test]
    fn a_counter_directive_with_a_seed_starts_from_it() {
        let nodes = build(Span::new("{counter:n:9}"), &Parser::default(), None);

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            "9",
            "{nodes:?}"
        );
    }

    #[test]
    fn a_counter_directive_advances_in_true_document_order_across_a_span_boundary() {
        // Regression test: `apply_attribute_references` (called recursively
        // by `apply_attribute_references_recursive`) resolves a `Styled`
        // child's own content *before* its own level, so a naive
        // find-then-advance at match time would advance the directive nested
        // in the *later* span before the plain directive that precedes it in
        // the source – reversing the numbering. `resolve_counters` fixes this
        // by resolving every directive, across the whole tree, in one
        // document-order pass first.
        let nodes = build(
            Span::new("{counter:n} *{counter:n}*"),
            &Parser::default(),
            None,
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            "1 <strong>2</strong>",
            "{nodes:?}"
        );

        // The reverse arrangement (the span comes first in the source) must
        // stay correct too.
        let nodes = build(
            Span::new("*{counter:n}* {counter:n}"),
            &Parser::default(),
            None,
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            "<strong>1</strong> 2",
            "{nodes:?}"
        );
    }

    #[test]
    fn a_missing_attribute_under_drop_is_a_documented_divergence() {
        use crate::parser::ModificationContext;

        let parser = Parser::default().with_intrinsic_attribute(
            "attribute-missing",
            "drop",
            ModificationContext::Anywhere,
        );

        let source = "before {undefined-thing} after";
        let nodes = build(Span::new(source), &parser, None);

        // Left unrecognized: the reference stays literal rather than being
        // dropped.
        assert_eq!(nodes.len(), 1);
        assert_text(&nodes[0], source, 1, 1);

        // The string pipeline, by contrast, drops the reference (keeping the
        // rest of the line).
        assert_eq!(golden_attributes_with(source, &parser), "before  after");
    }

    #[test]
    fn a_missing_attribute_under_drop_line_is_a_documented_divergence() {
        use crate::parser::ModificationContext;

        let parser = Parser::default().with_intrinsic_attribute(
            "attribute-missing",
            "drop-line",
            ModificationContext::Anywhere,
        );

        let source = "a line with {undefined-thing} in it";
        let nodes = build(Span::new(source), &parser, None);

        // Left unrecognized: the whole line is not dropped (this step has no
        // line-granularity concept; the string pipeline's line-drop mode is
        // orthogonal to node splicing).
        assert_eq!(nodes.len(), 1);
        assert_text(&nodes[0], source, 1, 1);

        // The string pipeline, by contrast, drops the whole line.
        assert_eq!(golden_attributes_with(source, &parser), "");
    }
}
