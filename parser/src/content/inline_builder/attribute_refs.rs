//! The attribute-references substitution step.

use std::collections::HashMap;

use super::{
    passthrough_step::is_special,
    quotes::{Piece, build_match_string, emit_range, source_slice},
    special_chars::Masked,
};
use crate::{
    Parser, Span,
    content::{ATTRIBUTE_REFERENCE, AttributeMissing},
    document::InterpretedValue,
    inlines::{InlineNode, RawForm},
    parser::attribute_lookup_name,
    strings::CowStr,
};

/// The attribute-references substitution, as a node transducer: descends into
/// [`Styled`](crate::inlines::Styled) children (a reference inside a rendered
/// span is recognized just as the string pipeline recognizes one inside
/// rendered markup), then matches and splices at this level.
///
/// It reuses the string pipeline's *exact* recognition —
/// [`ATTRIBUTE_REFERENCE`] is now shared `pub(crate)` — so only the
/// recognition *sink* differs (§4.1): a resolved reference's value is spliced
/// into the node stream, classified by [`split_attribute_value`] (design
/// §3.4.1) rather than written into a `&mut String`.
///
/// A `counter`/`counter2` directive (`{counter:name}`, `{counter2:name:seed}`)
/// is recognized like any other reference: it resolves *and advances* the
/// named document counter via [`Parser::counter`] — the same required side
/// effect [`apply_footnotes`](super::footnotes::apply_footnotes) performs for
/// footnote numbering, and for the same reason it cannot be deferred to the
/// cutover the way every other macro family's catalog/warning side effect is
/// (skipping it would leave the directive's own digits wrong, not just an
/// absent catalog entry). `counter` splices the advanced value in (classified
/// by [`split_attribute_value`] exactly like a plain reference's value);
/// `counter2` advances silently and splices nothing.
///
/// A reference to a **missing** attribute is handled per the document's
/// [`attribute-missing`] mode, exactly as `AttributeReplacer` handles it.
/// [`AttributeMissing::Skip`] (the default) and [`AttributeMissing::Warn`]
/// leave the reference literal, which this step reproduces by recording no
/// match at all — the surrounding gap logic then emits the source text
/// unchanged. [`AttributeMissing::Drop`] and [`AttributeMissing::DropLine`]
/// *remove* content instead, so they need the line granularity the string
/// pipeline's own `apply_attributes` gets from its line loop: see
/// [`MissingHandling`] and [`surviving_lines`] for how this transducer
/// reproduces it, and for the two shapes it defers.
///
/// The `DropLine` mode's own diagnostic (Asciidoctor's "dropping line
/// containing reference to missing attribute", recorded as a
/// [`SkippingReferenceToMissingAttribute`] warning) is **not** raised here: it
/// does not change the fold's output bytes, so — like every macro family's own
/// catalog/warning side effect — it is deferred to the cutover (design §5.2
/// Phase 4, step 6). The same applies to `Warn` mode's warning, whose output
/// this step already reproduces.
///
/// A literal `<`, `>`, or `&` **in the expanded value** is classified by
/// design §3.4.1 — "the kind a fragment becomes is decided by which
/// substitution steps still act on it under the group's effective order" —
/// which for this step means one question: does a
/// [`SpecialCharacters`](crate::content::SubstitutionStep::SpecialCharacters)
/// step still run *after* this one? [`SplicedSpecials`] carries the answer,
/// and [`split_attribute_value`] acts on it.
///
/// A **character replacement** *inside* an expanded value ((C) → © and
/// friends) is, by contrast, recognized: [`build_match_string`] (shared by
/// [`apply_character_replacements`](super::char_replacements::apply_character_replacements),
/// [`apply_macros`](super::macros::apply_macros), and
/// [`apply_quotes`](super::quotes::apply_quotes)) now contributes a
/// synthesized run's own `value` to the match string too — flagged
/// [`synthesized`](super::quotes::Piece::synthesized) rather than opaque —
/// so `apply_character_replacements`'s pattern sweep, still ahead in the
/// effective order, can match inside it exactly as it would over any other
/// run (a follow-up to this step, closing the gap this doc comment used to
/// describe). Most **macro** families are recognized inside an expanded value
/// too, for the same reason and by the same mechanism: a family that computes
/// every value it holds from the match string gates recognition on
/// [`range_is_verbatim_or_synthesized`](super::macros::image::range_is_verbatim_or_synthesized)
/// rather than
/// [`range_is_verbatim`](super::macros::image::range_is_verbatim), rejecting
/// only an [`atomic`](super::quotes::Piece::atomic) piece. What still needs an
/// honest `'src` slice, and so still defers, is an
/// [`Attrlist`](crate::attributes::Attrlist)`<'src>`, which reads its own
/// source span's bytes *as content* rather than merely as a location tag: an
/// image macro's non-empty attribute list (`image:sunset.jpg[{caption}]`) and
/// a link's attribute-list-bearing display text (`link:x[{text},role=hl]`).
/// A *wholly* expanded `link:`/`mailto:` macro defers for a second reason of
/// its own — see `link_macro_level`'s own scope note in
/// [`macros::links`](super::macros).
///
/// [`AttributeMissing::Drop`]: crate::content::AttributeMissing::Drop
/// [`AttributeMissing::DropLine`]: crate::content::AttributeMissing::DropLine
/// [`AttributeMissing::Skip`]: crate::content::AttributeMissing::Skip
/// [`AttributeMissing::Warn`]: crate::content::AttributeMissing::Warn
/// [`attribute-missing`]: https://docs.asciidoctor.org/asciidoc/latest/attributes/unresolved-references/#missing
/// [`SkippingReferenceToMissingAttribute`]: crate::warnings::WarningType::SkippingReferenceToMissingAttribute
///
/// A `counter`/`counter2` directive's advance must happen in true left-to-right
/// *document* order even though the splicing recursion below visits a
/// [`Styled`](crate::inlines::Styled) child's content *before* its own level
/// (so a later sub can match *inside* an earlier span — design note on
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
    specials: SplicedSpecials,
) -> Vec<InlineNode<'src>> {
    let mut counters = HashMap::new();
    resolve_counters(&nodes, root, parser, &mut counters);

    let missing = MissingHandling::for_content(&nodes, parser);

    // Which top-level nodes carry a missing reference *inside a span* — found
    // here, ahead of the recursion below, precisely because it must read the
    // content's own **pre-expansion** text (see [`styled_drop_indices`]).
    let span_drops = if missing == MissingHandling::DropLine {
        styled_drop_indices(&nodes, parser)
    } else {
        Vec::new()
    };

    apply_attribute_references_recursive(
        nodes,
        root,
        parser,
        &counters,
        missing,
        &span_drops,
        specials,
    )
}

/// Whether a
/// [`SpecialCharacters`](crate::content::SubstitutionStep::SpecialCharacters)
/// step still runs **after** the attribute-references step under the group's
/// effective order — the one thing that decides what a literal `<`, `>`, or
/// `&` in a spliced attribute value becomes (design §3.4.1).
///
/// In every *built-in* group that runs both steps the answer is
/// [`Verbatim`](Self::Verbatim):
/// [`Normal`](crate::content::SubstitutionGroup::Normal),
/// [`Title`](crate::content::SubstitutionGroup::Title),
/// [`Header`](crate::content::SubstitutionGroup::Header), and
/// [`AttributeEntryValue`](crate::content::SubstitutionGroup::AttributeEntryValue)
/// all escape first and expand later. Only a
/// [`Custom`](crate::content::SubstitutionGroup::Custom) `subs=` list can put
/// the two the other way round — and the shorthand `subs=attributes+`, which
/// *prepends* the step, is the documented AsciiDoc idiom for inspecting a
/// stored attribute value ("Internally, the value is stored as
/// `MyApp<sup>2</sup>`"), so this is a real order, not a corner case.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SplicedSpecials {
    /// A `SpecialCharacters` step is still ahead, so it will escape the
    /// spliced value exactly as it escapes the author's own text: the value is
    /// spliced as one ordinary [`Text`](InlineNode::Text) run and left for that
    /// step to split.
    EscapedLater,

    /// No `SpecialCharacters` step runs after this one — because it already
    /// ran (the built-in orders) or because the group never runs it at all —
    /// so the string pipeline emits the value's specials untouched, and each
    /// becomes a [`Raw`](InlineNode::Raw) leaf the fold emits verbatim.
    Verbatim,
}

/// `span_drops` names this level's own [`Styled`](crate::inlines::Styled)
/// nodes that force a line drop (see [`styled_drop_indices`]); a nested level
/// never has any of its own, since a drop is only ever decided at the
/// content's top level.
#[allow(clippy::too_many_arguments)]
fn apply_attribute_references_recursive<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
    counters: &HashMap<usize, String>,
    missing: MissingHandling,
    span_drops: &[usize],
    specials: SplicedSpecials,
) -> Vec<InlineNode<'src>> {
    let nodes: Vec<InlineNode<'src>> = nodes
        .into_iter()
        .map(|node| match node {
            InlineNode::Styled(mut styled) => {
                styled.children = apply_attribute_references_recursive(
                    styled.children,
                    root,
                    parser,
                    counters,
                    missing.nested(),
                    &[],
                    specials,
                );
                InlineNode::Styled(styled)
            }

            other => other,
        })
        .collect();

    attribute_references_level(nodes, root, parser, counters, missing, span_drops, specials)
}

/// How a level treats a reference to a **missing** attribute — this module's
/// node-transducer counterpart of the [`AttributeMissing`] mode the string
/// pipeline's own `AttributeReplacer` reads.
///
/// The two modes that *remove* content need line granularity the node stream
/// does not have on its own: the string pipeline gets it by splitting
/// `content.rendered` on `\n` and processing (and possibly discarding) one
/// line at a time. [`surviving_lines`] reproduces that split over a level's
/// own match string, whose `\n` bytes are the same ones the rendered string
/// carries — for the two shapes below, and only those.
///
/// # The shapes this defers
///
/// Both are pinned by their own divergence tests, and both fall back to
/// [`Literal`](Self::Literal) — the reference stays in place, exactly as it
/// did before this increment.
///
/// - A **`Styled` span straddling a line break** at the content's top level. A
///   span is one opaque [`SPAN_PLACEHOLDER`](super::quotes::SPAN_PLACEHOLDER)
///   piece in the match string, so its interior `\n`s are invisible here —
///   while the string pipeline, which by this point holds the span's rendered
///   markup inline, still sees them and splits on them. The line correspondence
///   the drop rests on would therefore be wrong, so
///   [`for_content`](Self::for_content) disables dropping for the whole content
///   when it finds one. (A masked passthrough or STEM expression is *not*
///   affected: the string pipeline holds a sentinel for it at this point, so a
///   multi-line one collapses its lines there too, exactly as the placeholder
///   does here.)
///
/// - A missing reference nested inside a `Styled` span, under
///   [`DropLine`](Self::DropLine). Dropping the *enclosing* line is what the
///   string pipeline does, which this level cannot see from inside the span;
///   [`nested`](Self::nested) therefore leaves such a reference literal. Under
///   [`DropReference`](Self::DropReference) the nested case *is* handled,
///   because removing the reference is a purely local edit and the span keeps
///   its enclosing line non-empty either way.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MissingHandling {
    /// Leave the reference in place, recording no match at all
    /// ([`AttributeMissing::Skip`], [`AttributeMissing::Warn`], and every
    /// deferred shape above).
    Literal,

    /// Drop the reference ([`AttributeMissing::Drop`]), and with it the whole
    /// line when the drop is all the line contained (Asciidoctor's
    /// `reject_if_empty`).
    DropReference,

    /// Drop the whole line the reference sits on
    /// ([`AttributeMissing::DropLine`]).
    DropLine,
}

impl MissingHandling {
    /// The handling the **content's own top level** takes: the document's
    /// [`AttributeMissing`] mode, unless one of `nodes`'s spans straddles a
    /// line break (see the type-level docs).
    fn for_content(nodes: &[InlineNode<'_>], parser: &Parser) -> Self {
        let mode = match AttributeMissing::from_parser(parser) {
            AttributeMissing::Skip | AttributeMissing::Warn => Self::Literal,
            AttributeMissing::Drop => Self::DropReference,
            AttributeMissing::DropLine => Self::DropLine,
        };

        if mode.drops_missing() && !line_structure_is_faithful(nodes) {
            return Self::Literal;
        }

        mode
    }

    /// The handling a `Styled` span's own children take (see the type-level
    /// docs).
    fn nested(self) -> Self {
        match self {
            Self::DropLine => Self::Literal,
            other => other,
        }
    }

    /// Whether a missing reference is removed rather than left literal.
    fn drops_missing(self) -> bool {
        self != Self::Literal
    }
}

/// Reports whether the match string of a level made of `nodes` preserves the
/// content's own line structure — i.e. no [`Styled`](crate::inlines::Styled)
/// span at this level straddles a line break. See [`MissingHandling`]'s own
/// docs for why a span that does disables the line-dropping modes.
///
/// The check is on the span's *source* span rather than on what it renders to,
/// which is sound in the direction that matters: a span whose source carries
/// no `\n` cannot render one, and building the tree must not invoke a renderer
/// to find out.
fn line_structure_is_faithful(nodes: &[InlineNode<'_>]) -> bool {
    !nodes.iter().any(|node| match node {
        InlineNode::Styled(styled) => styled.location.data().contains('\n'),
        _ => false,
    })
}

/// Resolves every `counter`/`counter2` directive in `nodes` (recursively,
/// including inside a [`Styled`](crate::inlines::Styled) child's own content)
/// via [`Parser::counter`], in genuine left-to-right document order, and
/// records each one's advanced value in `out`, keyed by the directive's
/// absolute source byte offset.
///
/// At each level this merges two kinds of event by source position — a
/// counter match found directly at this level, and a `Styled` sibling's own
/// placeholder position (a recursion point) — so a directive nested inside an
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
    let (s, pieces) = build_match_string(nodes, Masked::UNKNOWN);

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
    /// text, replacing nothing — mirroring the string replacer's
    /// `caps[1]`/`caps[5]` branch. One or two absolute offsets, ascending.
    Unescape { backslashes: Vec<usize> },

    /// A reference to a set attribute: its `value` (already resolved from
    /// `parser`) is spliced in, classified by [`split_attribute_value`]. A
    /// value-less `InterpretedValue::Set` attribute resolves to an empty
    /// `value`, mirroring the string replacer (see `AttributeReplacer`); an
    /// `InterpretedValue::Unset` one never becomes an `Expand` at all,
    /// counting as missing instead.
    Expand { value: String },

    /// A `counter`/`counter2` directive: the named counter's advanced value is
    /// looked up from [`resolve_counters`]'s output (keyed by this match's
    /// absolute source offset), *not* resolved here — see
    /// [`apply_attribute_references`]'s doc comment for why resolution must
    /// happen as a separate, document-order pass. `counter` splices the
    /// looked-up value in, classified by [`split_attribute_value`];
    /// `counter2` (`display: false`) advances silently and splices nothing.
    Counter { display: bool },

    /// A reference to a **missing** attribute under a line-dropping mode
    /// ([`MissingHandling::DropReference`] or
    /// [`MissingHandling::DropLine`]): the reference itself is replaced with
    /// nothing, and its presence is what marks its line as a drop candidate
    /// (see [`surviving_lines`]). Under [`MissingHandling::Literal`] no such
    /// match is recorded at all, so the reference survives as literal text.
    DropMissing,
}

/// Matches [`ATTRIBUTE_REFERENCE`] over this level's escaped text, replacing
/// each recognized match with the node(s) it produces and leaving everything
/// else in place. `counters` supplies each `counter`/`counter2` directive's
/// already-resolved value (see [`resolve_counters`]); `missing` decides what a
/// reference to an unset attribute becomes (see [`MissingHandling`]), and
/// `span_drops` names the nodes whose spans force a line drop (see
/// [`styled_drop_indices`]).
#[allow(clippy::too_many_arguments)]
fn attribute_references_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
    counters: &HashMap<usize, String>,
    missing: MissingHandling,
    span_drops: &[usize],
    specials: SplicedSpecials,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes, Masked::UNKNOWN);

    // A span that forces a line drop is named by node index; the line decision
    // works in match-string offsets. Translating here (rather than in
    // `styled_drop_indices`, which must run before the recursion) is safe
    // because that recursion only ever rewrites a `Styled` node's *children*,
    // and a `Styled` node contributes exactly one placeholder piece whatever
    // they are — so this level's piece layout is the same before and after it.
    let span_drop_offsets: Vec<usize> = pieces
        .iter()
        .filter(|piece| span_drops.contains(&piece.node_index))
        .map(|piece| piece.s_start)
        .collect();

    // Cheap pre-filter: skip the pattern sweep when no reference can be
    // present at this level. A span drop is still work even with no reference
    // of this level's own.
    if !s.contains('{') && span_drop_offsets.is_empty() {
        return nodes;
    }

    let matches = if s.contains('{') {
        find_attribute_matches(&s, parser, missing)
    } else {
        Vec::new()
    };

    let lines = surviving_lines(
        &s,
        &matches,
        &span_drop_offsets,
        missing,
        &pieces,
        root,
        counters,
    );

    if matches.is_empty() && lines.is_none() {
        return nodes;
    }

    // The whole level as one "line" when nothing was dropped: the rebuild's
    // flat, line-agnostic walk. (Spelled with `iter::once` rather than a
    // one-element `vec![…]`, which `clippy::single_range_in_vec_init` reads as
    // a mistyped `(0..n).collect()`.)
    let lines = lines.unwrap_or_else(|| std::iter::once(0..s.len()).collect());

    rebuild_attribute_level(&nodes, &pieces, &matches, root, counters, &lines, specials)
}

/// The indices into `nodes` of every [`Styled`](crate::inlines::Styled) span
/// whose own subtree carries a live reference to a missing attribute — the
/// spans that force their enclosing line to be dropped under
/// [`MissingHandling::DropLine`], since the string pipeline drops the line the
/// span's rendered markup sits on.
///
/// # Why this runs before the splicing recursion
///
/// It must read the content's **pre-expansion** text. The string pipeline
/// replaces every reference on a line in one `replace_all` pass, which never
/// re-scans its own replacements: a value that happens to expand *to*
/// `{something}` leaves that text in the output literally, and it neither is
/// nor arms a missing reference. Run after the recursion, this walk would see
/// the already-spliced value and read it as one — dropping a line the string
/// pipeline keeps. Hoisting the whole detection to
/// [`apply_attribute_references`], before anything is spliced, is what keeps
/// the two in step; it also means a synthesized *seed* (a filtered multi-line
/// block, reached through `build_from_value`) is still scanned, since its text
/// is pre-expansion content in its own right.
fn styled_drop_indices(nodes: &[InlineNode<'_>], parser: &Parser) -> Vec<usize> {
    nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| match node {
            InlineNode::Styled(styled) => subtree_has_missing_reference(&styled.children, parser),
            _ => false,
        })
        .map(|(index, _)| index)
        .collect()
}

/// Reports whether `nodes` — at this level, or inside a
/// [`Styled`](crate::inlines::Styled) child at any depth — carries a live
/// (non-escaped, non-directive) reference to an attribute that is not set.
///
/// Recognition is [`find_attribute_matches`]'s own, so the two cannot disagree
/// about what counts as a missing reference.
fn subtree_has_missing_reference(nodes: &[InlineNode<'_>], parser: &Parser) -> bool {
    let (s, _) = build_match_string(nodes, Masked::UNKNOWN);

    if s.contains('{')
        && find_attribute_matches(&s, parser, MissingHandling::DropLine)
            .iter()
            .any(|m| matches!(m.kind, AttributeMatchKind::DropMissing))
    {
        return true;
    }

    nodes.iter().any(|node| match node {
        InlineNode::Styled(styled) => subtree_has_missing_reference(&styled.children, parser),
        _ => false,
    })
}

/// The ranges of `s` this level still emits, one per surviving line — or
/// `None` when no line is dropped, which is every level under
/// [`MissingHandling::Literal`] and the overwhelming majority of levels under
/// the other two.
///
/// This mirrors the string pipeline's own line loop (`apply_attributes`
/// splits `content.rendered` on `\n`, replaces within each line, and skips a
/// line the mode calls for). A returned range covers a line's *content* only:
/// [`rebuild_attribute_level`] re-emits one `\n` between consecutive
/// survivors, which reproduces the string pipeline's "join the kept lines"
/// shape for a drop anywhere — first line, last line, or a run in the middle.
#[allow(clippy::too_many_arguments)]
fn surviving_lines(
    s: &str,
    matches: &[AttributeMatch],
    span_drops: &[usize],
    missing: MissingHandling,
    pieces: &[Piece],
    root: Span<'_>,
    counters: &HashMap<usize, String>,
) -> Option<Vec<std::ops::Range<usize>>> {
    if !missing.drops_missing() {
        return None;
    }

    let mut kept = Vec::new();
    let mut dropped_any = false;
    let mut start = 0usize;

    for line in s.split('\n') {
        let range = start..start + line.len();

        // Past the last line this runs off the end, which is harmless: it is
        // never read again.
        start = range.end + 1;

        let dropped = line_is_dropped(
            s,
            &range,
            matches,
            span_drops,
            missing == MissingHandling::DropLine,
            pieces,
            root,
            counters,
        );

        if dropped {
            dropped_any = true;
        } else {
            kept.push(range);
        }
    }

    dropped_any.then_some(kept)
}

/// Whether the line covering `range` is dropped whole: unconditionally when
/// `unconditional` ([`MissingHandling::DropLine`]) once it carries a missing
/// reference, and otherwise ([`MissingHandling::DropReference`]) only when
/// removing that reference is all it took to empty the line (Asciidoctor's
/// `reject_if_empty`, which the string pipeline reproduces in
/// `drop_emptied_line`).
///
/// Only [`surviving_lines`] calls this, and only once it has established that
/// the level drops missing references at all, so
/// [`MissingHandling::Literal`] never reaches here — hence the plain flag
/// rather than the mode itself.
#[allow(clippy::too_many_arguments)]
fn line_is_dropped(
    s: &str,
    range: &std::ops::Range<usize>,
    matches: &[AttributeMatch],
    span_drops: &[usize],
    unconditional: bool,
    pieces: &[Piece],
    root: Span<'_>,
    counters: &HashMap<usize, String>,
) -> bool {
    let has_missing = matches_in(matches, range)
        .any(|m| matches!(m.kind, AttributeMatchKind::DropMissing))
        || span_drops
            .iter()
            .any(|start| *start >= range.start && *start < range.end);

    if !has_missing {
        return false;
    }

    if unconditional {
        return true;
    }

    let replaced = line_replacement(s, range, matches, pieces, root, counters);

    // A trailing `\r` left from a CRLF terminator is part of the line ending,
    // not content — the same guard `drop_emptied_line` applies on the string
    // side.
    replaced.strip_suffix('\r').unwrap_or(&replaced).is_empty()
}

/// The text the line covering `range` would become once every match in it is
/// replaced — the string pipeline's `replaced` for that line, reconstructed
/// from this level's own matches so the emptied-line test above sees what
/// `apply_attributes` sees.
///
/// An opaque piece (a rendered span, a masked passthrough) contributes its
/// single [`SPAN_PLACEHOLDER`](super::quotes::SPAN_PLACEHOLDER) here where the
/// string pipeline holds its markup or sentinel; the two differ in length but
/// agree on the only thing this test asks, which is whether anything is left.
fn line_replacement(
    s: &str,
    range: &std::ops::Range<usize>,
    matches: &[AttributeMatch],
    pieces: &[Piece],
    root: Span<'_>,
    counters: &HashMap<usize, String>,
) -> String {
    let mut out = String::new();
    let mut cursor = range.start;

    for m in matches_in(matches, range) {
        out.push_str(s.get(cursor..m.full.start).unwrap_or_default());

        match &m.kind {
            AttributeMatchKind::Unescape { backslashes } => {
                let mut piece_cursor = m.full.start;

                for &backslash in backslashes {
                    out.push_str(s.get(piece_cursor..backslash).unwrap_or_default());
                    piece_cursor = backslash + 1;
                }

                out.push_str(s.get(piece_cursor..m.full.end).unwrap_or_default());
            }

            AttributeMatchKind::Expand { value } => out.push_str(value),

            AttributeMatchKind::Counter { display } => {
                if *display {
                    out.push_str(counter_value(pieces, &m.full, root, counters));
                }
            }

            AttributeMatchKind::DropMissing => {}
        }

        cursor = m.full.end;
    }

    out.push_str(s.get(cursor..range.end).unwrap_or_default());
    out
}

/// The matches falling wholly inside `range`. A match never straddles a line
/// break (an [`ATTRIBUTE_REFERENCE`] cannot contain one), so every match is
/// either wholly inside a line or wholly outside it.
fn matches_in<'m>(
    matches: &'m [AttributeMatch],
    range: &'m std::ops::Range<usize>,
) -> impl Iterator<Item = &'m AttributeMatch> {
    matches
        .iter()
        .filter(move |m| m.full.start >= range.start && m.full.end <= range.end)
}

/// The value [`resolve_counters`] recorded for the directive matched at
/// `full`, keyed by its absolute source byte offset.
///
/// `resolve_counters` records a value for every counter match this module's
/// own regex sweep finds, keyed by this exact offset, so the lookup always
/// hits; the empty fallback only guards against the two passes somehow
/// disagreeing, which would itself be a bug.
fn counter_value<'c>(
    pieces: &[Piece],
    full: &std::ops::Range<usize>,
    root: Span<'_>,
    counters: &'c HashMap<usize, String>,
) -> &'c str {
    let offset = source_slice(pieces, full.clone(), root).byte_offset();

    counters
        .get(&offset)
        .map(String::as_str)
        .unwrap_or_default()
}

/// Finds every non-overlapping [`ATTRIBUTE_REFERENCE`] match in the escaped
/// match string `s`, left to right, exactly as the string pipeline's
/// `replace_all` does. A `counter`/`counter2` directive is recorded as a
/// match here but not yet resolved (see [`AttributeMatchKind::Counter`]); a
/// reference to a missing attribute becomes a
/// [`DropMissing`](AttributeMatchKind::DropMissing) match under a
/// line-dropping `missing` handling, and is left out of the returned list
/// entirely under [`MissingHandling::Literal`].
fn find_attribute_matches(
    s: &str,
    parser: &Parser,
    missing: MissingHandling,
) -> Vec<AttributeMatch> {
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
        // counter rather than looking up an existing attribute — mirroring
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

        let interpreted_value = parser.attribute_value(&lookup_name);

        // A reference is "missing" for `attribute-missing` purposes both when
        // the attribute was never assigned at all and when it was explicitly
        // unset (a document `:name!:` entry or an API override that unsets
        // it) — both resolve to `InterpretedValue::Unset`. Only a value-less
        // `Set` attribute or a concrete `Value` counts as present, mirroring
        // `AttributeReplacer::replace_append`.
        if !parser.has_attribute(&lookup_name)
            || matches!(interpreted_value, InterpretedValue::Unset)
        {
            if missing.drops_missing() {
                matches.push(AttributeMatch {
                    full,
                    kind: AttributeMatchKind::DropMissing,
                });
            }

            // Otherwise left unrecognized, so the surrounding gap logic emits
            // the reference as literal text — what `AttributeMissing::Skip`
            // (the default) and `AttributeMissing::Warn` both do, and what the
            // shapes `MissingHandling` defers fall back to.
            continue;
        }

        let value = match interpreted_value {
            InterpretedValue::Value(value) => value,

            // A value-less `Set` attribute substitutes to an empty string; an
            // `Unset` one never reaches here, having been treated as missing
            // above.
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
///
/// `lines` is the level's surviving line ranges (see [`surviving_lines`]).
/// When no line was dropped it is the single whole-level range `0..s.len()`,
/// which makes this the flat, line-agnostic walk it has always been; when
/// lines *were* dropped, each survivor is emitted in turn with one `\n` — the
/// very byte that terminated the previous *survivor* in the source — re-emitted
/// between consecutive survivors.
#[allow(clippy::too_many_arguments)]
fn rebuild_attribute_level<'src>(
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    matches: &[AttributeMatch],
    root: Span<'src>,
    counters: &HashMap<usize, String>,
    lines: &[std::ops::Range<usize>],
    specials: SplicedSpecials,
) -> Vec<InlineNode<'src>> {
    let mut out = Vec::new();
    let mut previous_end: Option<usize> = None;

    for line in lines {
        // One separator between consecutive survivors — the string pipeline's
        // own "join the kept lines with `\n`" shape. The byte emitted is the
        // `\n` that terminated the *previous survivor*, which is always a real
        // one: a line range excludes its terminator, and a line followed by
        // another always has one. Whether lines were dropped in between makes
        // no difference; exactly one separator belongs here either way.
        if let Some(end) = previous_end {
            emit_range(nodes, pieces, end..end + 1, &mut out);
        }

        previous_end = Some(line.end);

        let mut cursor = line.start;

        for m in matches_in(matches, line) {
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
                }

                AttributeMatchKind::Expand { value } => {
                    emit_range(nodes, pieces, cursor..m.full.start, &mut out);

                    let location = source_slice(pieces, m.full.clone(), root);
                    split_attribute_value(value, location, specials, &mut out);
                }

                AttributeMatchKind::Counter { display } => {
                    emit_range(nodes, pieces, cursor..m.full.start, &mut out);

                    // `counter2` advances silently: the match is consumed but
                    // splices no node, mirroring `AttributeReplacer`'s own
                    // directive-name check.
                    if *display {
                        let location = source_slice(pieces, m.full.clone(), root);
                        let value = counter_value(pieces, &m.full, root, counters);
                        split_attribute_value(value, location, specials, &mut out);
                    }
                }

                // A missing reference under a line-dropping mode: the match is
                // consumed and splices nothing. Its line survived, so only the
                // reference itself goes (`AttributeMissing::Drop`'s own
                // behavior; under `DropLine` a line carrying one is never a
                // survivor in the first place).
                AttributeMatchKind::DropMissing => {
                    emit_range(nodes, pieces, cursor..m.full.start, &mut out);
                }
            }

            cursor = m.full.end;
        }

        if cursor < line.end {
            emit_range(nodes, pieces, cursor..line.end, &mut out);
        }
    }

    out
}

/// Splices a resolved attribute `value` into the node stream, classifying its
/// literal `<`, `>`, and `&` by design §3.4.1 — which, here, is exactly the
/// question [`SplicedSpecials`] answers.
///
/// Under [`SplicedSpecials::Verbatim`] — every built-in group that runs both
/// steps, plus any order that never runs `SpecialCharacters` at all —
/// [`apply_special_characters`](super::special_chars::apply_special_characters)
/// will not act on this spliced-in content, so a literal special must **not**
/// be escaped by the fold: it becomes a [`Raw`](InlineNode::Raw) leaf
/// (verbatim) and the surrounding runs stay [`Text`](InlineNode::Text). That is
/// the classification this step has always emitted.
///
/// Under [`SplicedSpecials::EscapedLater`] the step is still ahead, so the
/// string pipeline *does* escape the spliced value — `subs=attributes+` on a
/// verbatim block renders `pass:quotes[MyApp^2^]`'s stored
/// `MyApp<sup>2</sup>` as `MyApp&lt;sup&gt;2&lt;/sup&gt;`. The value is
/// therefore spliced as one ordinary `Text` run and **left unsplit**, for
/// `apply_special_characters` to split at its own position in the order. Doing
/// it there rather than here is what keeps the intervening steps faithful: the
/// string pipeline's own `quotes`/`replacements`/`macros` passes match over
/// text whose specials are still literal, and a `Raw` leaf is *opaque* to
/// [`build_match_string`] where a `Text` run's bytes are not — the same
/// argument
/// [`classify_unescaped_specials`](super::special_chars::classify_unescaped_specials)
/// makes for running last.
///
/// Every node emitted carries the reference's own `location` as its coarse
/// fallback span (design §4.4: a synthesized value has no source of its own).
/// A run is never emitted empty, and an empty `value` (a value-less
/// `InterpretedValue::Set` attribute) emits no node at all.
fn split_attribute_value<'src>(
    value: &str,
    location: Span<'src>,
    specials: SplicedSpecials,
    out: &mut Vec<InlineNode<'src>>,
) {
    if specials == SplicedSpecials::EscapedLater {
        if !value.is_empty() {
            out.push(InlineNode::Text {
                value: CowStr::from(value.to_string()),
                location,
            });
        }

        return;
    }

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
            form: RawForm::AsIs,
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

    use super::super::test_support::{
        assert_raw, assert_special_char, assert_styled, assert_text, fold_html,
    };
    use crate::{
        Parser, Span,
        content::{
            Content, SubstitutionGroup, SubstitutionStep,
            inline_builder::{build, build_for_group},
        },
        inlines::{InlineNode, SpanForm, StyleVariant},
        parser::HtmlSubstitutionRenderer,
        strings::CowStr,
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
            // Escapes: the reference stays literal, minus the backslash(es) —
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

        // A value-less `Set` attribute expands to nothing, while an explicitly
        // *unset* one counts as missing — so under the default
        // (`AttributeMissing::Skip`) mode its reference stays literal rather
        // than vanishing (issue #1117).
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
        // on the *fold*, not node count/shape — the same choice
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

        // The expansion splits into `Raw("<")`, `Text("b")`, `Raw(">")` —
        // design §3.4.1's mix of node kinds, since `specialcharacters` has
        // already run and will not re-escape this spliced-in content.
        assert_eq!(nodes.len(), 3);
        let raw_location = assert_raw(&nodes[0], "<");

        match &nodes[1] {
            InlineNode::Text { value, location } => {
                assert_eq!(value.as_ref(), "b");
                // A synthesized value has no source of its own, so every
                // sub-node falls back to the whole reference's span (design
                // §4.4) — the same coarse fallback as `raw_location`.
                assert_eq!(*location, raw_location);
            }

            other => panic!("expected Text(\"b\"), got {other:?}"),
        }

        assert_raw(&nodes[2], ">");

        // The fold emits the tag verbatim, unescaped.
        assert_eq!(fold_html(&nodes, &HtmlSubstitutionRenderer {}), "<b>");
    }

    #[test]
    fn a_reference_expanding_to_a_special_character_stays_text_when_escaping_follows() {
        // The mirror image of the test above, under the one order that
        // reverses the two steps: `subs=attributes+` on a verbatim block
        // resolves to `[attributes, specialcharacters, callouts]`, so the
        // `SpecialCharacters` step is still *ahead* when the value is spliced
        // and the string pipeline escapes it like any other text.
        //
        // Design §3.4.1: what the fragment becomes is decided by which steps
        // still act on it, so the value is spliced as one ordinary `Text` run
        // (`SplicedSpecials::EscapedLater`) and `apply_special_characters`
        // splits it at its own position in the order — leaving the `CharRef`
        // leaves an author-written `<b>` would get under the same order, not
        // the `Raw` leaves the built-in orders produce.
        let (group, invalid) = SubstitutionGroup::from_custom_string(
            Some(&SubstitutionGroup::Verbatim),
            "attributes+",
        );
        assert!(invalid.is_empty());
        assert_eq!(
            group,
            SubstitutionGroup::Custom(vec![
                SubstitutionStep::AttributeReferences,
                SubstitutionStep::SpecialCharacters,
                SubstitutionStep::Callouts,
            ])
        );

        let parser = parser_with_attribute("tag", "<b>");
        let source = "{tag}";
        let nodes = build_for_group(
            &group,
            CowStr::from(source),
            Span::new(source),
            &parser,
            None,
        );

        // `CharRef("<")`, `Text("b")`, `CharRef(">")` — not the `Raw` mix the
        // normal order yields for the same value.
        assert_eq!(nodes.len(), 3);
        let char_ref_location = assert_special_char(&nodes[0], '<');

        match &nodes[1] {
            InlineNode::Text { value, location } => {
                assert_eq!(value.as_ref(), "b");
                // Still the coarse §4.4 fallback: a synthesized value has no
                // source of its own, whichever leaf kind its specials take.
                assert_eq!(*location, char_ref_location);
            }

            other => panic!("expected Text(\"b\"), got {other:?}"),
        }

        assert_special_char(&nodes[2], '>');

        // And the fold escapes the tag, byte-for-byte as the real pipeline
        // does — the documented `subs=attributes+` trick for inspecting a
        // stored attribute value.
        let mut content = Content::from(Span::new(source));
        group.apply_string_pipeline(&mut content, &parser_with_attribute("tag", "<b>"), None);

        assert_eq!(content.rendered_str(), "&lt;b&gt;");
        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            content.rendered_str()
        );
    }

    #[test]
    fn a_replacement_inside_an_expanded_value_is_recognized() {
        // Design §3.4.1 says `replacements` still runs over an expanded
        // value, so a `(C)` inside it becomes a `CharRef` — closing the gap
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
        // character from one piece and its `--` from the next — exercised
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
    fn an_attrlist_bearing_display_text_inside_an_expanded_value_is_recognized() {
        // Every macro family is now recognized inside a spliced value (see
        // this module's own doc comment, and each family's own parity corpus),
        // and so is every attribute list one of them carries: a capture with no
        // `'src` slice is parsed from the level's match string — the same bytes
        // the string replacer parses — and owned off it, for an image's own
        // bracket (`macros/image.rs`) and for a link's display text
        // (`macros/links.rs`) alike.
        let parser = parser_with_attribute("label", "Docs");
        let source = "see link:index.html[{label},role=hl] now";
        let nodes = build(Span::new(source), &parser, None);

        assert!(
            nodes.iter().any(|n| matches!(n, InlineNode::Ref(_))),
            "an attrlist-bearing display text inside a spliced value should be recognized: \
             {nodes:?}"
        );
    }

    #[test]
    fn an_attrlist_bearing_image_inside_an_expanded_value_is_recognized() {
        // The counterpart of the divergence above, and the case this test used
        // to pin: an image macro's non-empty attribute list no longer needs an
        // `'src` slice, so a wholly-spliced `image:…[…]` is recognized.
        let parser = parser_with_attribute("imgtext", "image:sunset.jpg[Sunset]");
        let nodes = build(Span::new("see {imgtext} now"), &parser, None);

        assert!(
            nodes.iter().any(|n| matches!(n, InlineNode::Image(_))),
            "an image inside a spliced value should be recognized: {nodes:?}"
        );
    }

    #[test]
    fn a_macro_inside_an_expanded_value_is_recognized() {
        // The counterpart of the divergence above: a family that computes
        // every value it holds from the level's match string — here the
        // `link:` macro, whose own marker is written in the source — is
        // recognized inside a spliced value, with only the node's `location`
        // taking design §4.4's coarse fallback.
        let parser = parser_with_attribute("target", "https://example.org");
        let nodes = build(Span::new("see link:{target}[Example] now"), &parser, None);

        assert!(
            nodes.iter().any(|n| matches!(n, InlineNode::Ref(_))),
            "a link macro over a spliced target should be recognized: {nodes:?}"
        );
    }

    #[test]
    fn a_macro_crossing_a_replacement_from_an_expanded_value_is_recognized() {
        // The two lifts composed: the replacements step recognizes a `(C)`
        // *inside* a spliced value (above), and the macro families now read
        // across the `CharRef::Replacement` leaf it produces — so an image's
        // attribute list and a link's display text, each crossing a
        // replacement that exists only because an attribute expanded, are
        // recognized with the string replacer's own bytes.
        let parser = parser_with_attribute("note", "Pause (C) Resume");

        for source in ["image:x.png[title={note}]", "link:x.html[{note}]"] {
            let nodes = build(Span::new(source), &parser, None);

            assert_eq!(
                fold_html(&nodes, &HtmlSubstitutionRenderer {}),
                golden_attributes_with(source, &parser),
                "fold diverged from the string pipeline for {source:?}"
            );
        }
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
        // crosses over — the same test-independence footnote numbering needs
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
            // relative orders — regression coverage for `resolve_counters`'s
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
        // the source — reversing the numbering. `resolve_counters` fixes this
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

    // ---- `attribute-missing` drop / drop-line ------------------------------

    /// A default parser with `attribute-missing` set to `mode`, plus three
    /// resolvable attributes a fixture can mix with a missing reference:
    /// an ordinary one, one whose value carries a **newline**, and one whose
    /// value is itself **reference-shaped**.
    ///
    /// It also carries an explicitly **unset** attribute, which counts as
    /// missing just as a never-assigned one does (issue #1117), and a
    /// value-less **set** one, which does not.
    fn parser_with_missing_mode(mode: &str) -> Parser {
        use crate::parser::ModificationContext;

        Parser::default()
            .with_intrinsic_attribute("attribute-missing", mode, ModificationContext::Anywhere)
            .with_intrinsic_attribute("greeting", "Hello", ModificationContext::Anywhere)
            .with_intrinsic_attribute("two-lines", "a\nb", ModificationContext::Anywhere)
            .with_intrinsic_attribute("looks-like-a-ref", "{nope}", ModificationContext::Anywhere)
            .with_intrinsic_attribute_bool("unset-thing", false, ModificationContext::Anywhere)
            .with_intrinsic_attribute_bool("set-flag", true, ModificationContext::Anywhere)
    }

    /// Asserts that folding the single-pass tree for `source` reproduces the
    /// string pipeline's output byte-for-byte under `attribute-missing=mode`.
    ///
    /// Each side gets its own independently-built parser: a fixture carrying a
    /// `counter` directive advances a real document counter on both, so
    /// sharing one would number them twice (design §5.3's
    /// two-independent-parsers discipline).
    fn assert_missing_mode_parity(source: &str, mode: &str) {
        let nodes = build(Span::new(source), &parser_with_missing_mode(mode), None);

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_attributes_with(source, &parser_with_missing_mode(mode)),
            "fold diverged under attribute-missing={mode} for {source:?}"
        );
    }

    #[test]
    fn fold_matches_the_string_pipeline_under_attribute_missing_drop() {
        // `AttributeMissing::Drop` removes the reference and, when that was
        // all the line contained, the line with it (Asciidoctor's
        // `reject_if_empty`).
        let fixtures = [
            // The reference goes; the rest of the line stays.
            "before {undefined-thing} after",
            "{undefined-thing} leads the line",
            "the line trails with {undefined-thing}",
            "two {undefined-thing} on {also-undefined} one line",
            // A resolvable reference beside a missing one.
            "{greeting}, {undefined-thing}!",
            // A reference-only line is dropped entirely, wherever it sits.
            "{undefined-thing}",
            "{undefined-thing}\nsecond line",
            "first line\n{undefined-thing}",
            "first line\n{undefined-thing}\nthird line",
            "first\n{undefined-thing}\n{also-undefined}\nlast",
            // A line the drop did *not* empty survives.
            "first line\nx{undefined-thing}\nthird line",
            "first line\n{undefined-thing} \nthird line",
            // A resolvable reference that empties a line does *not* drop it:
            // only a *missing* one arms the check.
            "first line\n{empty-value}\nthird line",
            // An escaped reference is never a missing reference, so it neither
            // drops nor arms the line check.
            "\\{undefined-thing}",
            "first\n\\{undefined-thing}\nlast",
            // An escaped reference on the *same* line as a missing one: the
            // literal text it leaves behind is what keeps the line from
            // counting as emptied.
            "\\{undefined-thing}{undefined-thing}",
            "first\n\\{a\\}{undefined-thing}\nlast",
            // A `counter` directive likewise leaves its digits behind, so the
            // line survives — while `counter2`, which displays nothing, leaves
            // the line as empty as the dropped reference did.
            "{counter:n}{undefined-thing}",
            "first\n{counter2:n}{undefined-thing}\nlast",
            // Beside other constructs on the same line.
            "*bold* and {undefined-thing} here",
            "a link:index.html[Docs] then {undefined-thing}",
            // Inside a single-line span: the reference goes, the span (and so
            // the line) stays.
            "*{undefined-thing}*",
            "_text {undefined-thing} more_",
            "a line with *{undefined-thing}* in it",
            // An expanded value carrying a newline of its own: both pipelines
            // split into lines *before* expanding, so the newline the value
            // introduces is never a line boundary either drop decision sees.
            "{two-lines} {undefined-thing}",
            "{two-lines}\n{undefined-thing}",
            "{undefined-thing}\n{two-lines}",
            "*{two-lines}* {undefined-thing}",
            // A value that is itself reference-shaped: neither pipeline
            // re-scans its own replacement, so the spliced `{nope}` stays
            // literal and arms nothing.
            "{looks-like-a-ref}",
            "_{looks-like-a-ref}_",
            "keep\n_{looks-like-a-ref}_\nkeep",
            "_{looks-like-a-ref}_ and {undefined-thing}",
            // An *explicitly unset* attribute is missing too, so it drops — and
            // empties its line — exactly as a never-assigned one does (issue
            // #1117). A value-less *set* attribute is not missing: it expands
            // to nothing without arming the line check.
            "before {unset-thing} after",
            "{unset-thing}",
            "first line\n{unset-thing}\nthird line",
            "{unset-thing} and {undefined-thing} on one line",
            "*{unset-thing}*",
            "first line\n{set-flag}\nthird line",
            "{set-flag}{unset-thing}",
        ];

        for fixture in fixtures {
            assert_missing_mode_parity(fixture, "drop");
        }
    }

    #[test]
    fn fold_matches_the_string_pipeline_under_attribute_missing_drop_line() {
        // `AttributeMissing::DropLine` removes the whole line a missing
        // reference sits on, whatever else the line carried.
        let fixtures = [
            "a line with {undefined-thing} in it",
            "{undefined-thing}",
            "{undefined-thing}\nsecond line",
            "first line\n{undefined-thing}",
            "first line\nblah blah {undefined-thing}\nall there is",
            "first\n{undefined-thing}\n{also-undefined}\nlast",
            "first\n{undefined-thing}\nmiddle\n{also-undefined}\nlast",
            // Two missing references on one line drop it once, not twice.
            "keep\n{undefined-thing} and {also-undefined}\nkeep",
            // A resolvable reference on a surviving line still expands.
            "{greeting}\n{undefined-thing}\n{greeting} again",
            // An escaped reference is not a missing reference: the line stays.
            "\\{undefined-thing} stays",
            "keep\n\\{undefined-thing}\nkeep too",
            // Other constructs on the dropped line go with it.
            "keep\n*bold* and {undefined-thing}\nkeep too",
            "keep\nlink:index.html[Docs] {undefined-thing}\nkeep too",
            // A missing reference nested in a *single-line* span drops the
            // enclosing line, exactly as the string pipeline drops the line
            // the span's rendered markup sits on.
            "*{undefined-thing}*",
            "keep\na *{undefined-thing}* span\nkeep too",
            "keep\n_outer *{undefined-thing}* inner_\nkeep too",
            // An expanded value carrying a newline of its own: both pipelines
            // split into lines *before* expanding, so the newline the value
            // introduces is never a line boundary either drop decision sees.
            "{two-lines} {undefined-thing}",
            "{two-lines}\n{undefined-thing}",
            "{undefined-thing}\n{two-lines}",
            "*{two-lines}* {undefined-thing}",
            "keep\n{two-lines} {undefined-thing}\nkeep too",
            // A value that is itself reference-shaped. Neither pipeline
            // re-scans its own replacement — `replace_all` never does — so the
            // spliced `{nope}` stays literal and is *not* a missing reference:
            // it neither drops its own line nor, from inside a span, the
            // enclosing one. This is why the span-drop detection runs ahead of
            // the splicing recursion (see `styled_drop_indices`).
            "{looks-like-a-ref}",
            "_{looks-like-a-ref}_",
            "keep\n_{looks-like-a-ref}_\nkeep",
            "keep\n{looks-like-a-ref}\nkeep",
            "_{looks-like-a-ref}_ and {undefined-thing}",
            "keep\n_a {looks-like-a-ref} b_\nkeep too",
            // An *explicitly unset* attribute is missing too, so it takes its
            // whole line with it exactly as a never-assigned one does (issue
            // #1117). A value-less *set* attribute is not missing: its line
            // survives, with the reference expanded to nothing.
            "keep\na line with {unset-thing} in it\nkeep too",
            "{unset-thing}",
            "keep\n*{unset-thing}*\nkeep too",
            "keep\n{unset-thing} and {undefined-thing}\nkeep too",
            "keep\n{set-flag}\nkeep too",
            "keep\n{set-flag} and {unset-thing}\nkeep too",
        ];

        for fixture in fixtures {
            assert_missing_mode_parity(fixture, "drop-line");
        }
    }

    #[test]
    fn a_reference_shaped_expansion_inside_a_span_does_not_drop_its_line() {
        // The regression the span-drop detection's own ordering guards
        // against, asserted directly rather than only through the corpus: the
        // spliced `{nope}` must reach the output literally, with its line
        // intact, exactly as the string pipeline's single `replace_all` pass
        // leaves it.
        let source = "keep\n_{looks-like-a-ref}_\nkeep too";

        let nodes = build(
            Span::new(source),
            &parser_with_missing_mode("drop-line"),
            None,
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            "keep\n<em>{nope}</em>\nkeep too",
            "{nodes:?}"
        );
    }

    #[test]
    fn a_counter_directive_survives_a_dropped_neighbouring_line() {
        // A `counter` directive advances during `resolve_counters`, which runs
        // over the whole tree before any line is dropped — exactly as the
        // string pipeline advances a counter as its own line loop reaches it.
        // A directive on a *dropped* line has still advanced, so the survivor
        // after it numbers from there.
        let parser = parser_with_missing_mode("drop-line");
        let source = "{counter:n}\n{undefined-thing} {counter:n}\n{counter:n}";

        let nodes = build(Span::new(source), &parser, None);
        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});

        assert_eq!(folded, "1\n3");

        // The string pipeline, driven by its own independent parser (each side
        // advances the counter for real — design §5.3's two-independent-parsers
        // discipline), agrees.
        assert_eq!(
            golden_attributes_with(source, &parser_with_missing_mode("drop-line")),
            folded
        );
    }

    #[test]
    fn a_dropped_line_leaves_honest_spans_on_the_survivors() {
        // Dropping a line is a matter of *which* source ranges are emitted:
        // every surviving node still borrows from `'src` with its own precise
        // line/col, including the re-emitted `\n` separator — which is the
        // byte that terminated the previous survivor (line 1 here), not the
        // one that terminated the dropped line (design §4.4's precision stage
        // — the structural assertion the Strategy-A tree cannot make).
        let parser = parser_with_missing_mode("drop-line");
        let source = "first\n{undefined-thing}\nthird";

        let nodes = build(Span::new(source), &parser, None);

        assert_eq!(nodes.len(), 3, "{nodes:?}");
        assert_text(&nodes[0], "first", 1, 1);
        assert_text(&nodes[1], "\n", 1, 6);
        assert_text(&nodes[2], "third", 3, 1);
    }

    #[test]
    fn a_real_documents_dropped_line_reaches_its_tree() {
        // End-to-end, through the real parse path: `attribute-missing` is read
        // off the document's own header, and `SubstitutionGroup::apply` clones
        // the parser to build each content's tree — so a real paragraph whose
        // middle line the string pipeline dropped folds to exactly the
        // rendered string it produced. This is the shape that made this a
        // *blocker* for the authoritative fold rather than an unclaimed form:
        // golden tests already exercise it (design §5.3's oracle).
        use crate::blocks::{FindBlocks, IsBlock};

        let doc = Parser::default()
            .parse(":attribute-missing: drop-line\n\nFirst line.\nA {nope} line.\nThird line.\n");

        let blocks: Vec<_> = doc.descendant_blocks().collect();
        assert_eq!(blocks.len(), 1, "expected one paragraph: {blocks:?}");

        let rendered = blocks[0].rendered_html_content().unwrap();
        let inlines = blocks[0].inlines().unwrap();

        assert_eq!(rendered, "First line.\nThird line.");

        assert_eq!(
            super::super::fold_html(inlines, &HtmlSubstitutionRenderer {}, &Parser::default()),
            rendered,
            "fold diverged from the rendered string for {inlines:?}"
        );
    }

    #[test]
    fn a_real_documents_prepended_attributes_step_reaches_its_tree() {
        // End-to-end, through the real parse path, for the AsciiDoc docs' own
        // `subs=attributes+` trick: a listing block whose substitution list
        // puts `attributes` *first* renders `pass:quotes[…]`'s stored markup
        // escaped, and the tree `SubstitutionGroup::apply` builds alongside it
        // must fold to exactly those bytes. This is the shape that makes the
        // reversed order a *blocker* for the authoritative fold rather than an
        // unclaimed form: a golden test already exercises it (design §5.3's
        // oracle — see `attribute_entry_substitutions`).
        use crate::blocks::{FindBlocks, IsBlock};

        let doc = Parser::default().parse(
            ":app-name: pass:quotes[MyApp^2^]\n\n[subs=attributes+]\n------\n{app-name}\n------\n",
        );

        let blocks: Vec<_> = doc.descendant_blocks().collect();
        assert_eq!(blocks.len(), 1, "expected one listing block: {blocks:?}");

        let rendered = blocks[0].rendered_html_content().unwrap();
        let inlines = blocks[0].inlines().unwrap();

        assert_eq!(rendered, "MyApp&lt;sup&gt;2&lt;/sup&gt;");

        assert_eq!(
            super::super::fold_html(inlines, &HtmlSubstitutionRenderer {}, &Parser::default()),
            rendered,
            "fold diverged from the rendered string for {inlines:?}"
        );
    }

    #[test]
    fn a_missing_reference_inside_a_multi_line_span_is_a_documented_divergence() {
        // A `Styled` span is one opaque placeholder in the match string, so a
        // span straddling a line break hides the `\n`s the string pipeline
        // still sees in its own rendered markup — and with them the line
        // correspondence a drop rests on. `MissingHandling::for_content`
        // therefore disables dropping for the whole content, leaving every
        // reference literal (this step's pre-increment behavior).
        let parser = parser_with_missing_mode("drop-line");
        let source = "_a\nb_ {undefined-thing}";

        let nodes = build(Span::new(source), &parser, None);
        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});

        assert!(
            folded.contains("{undefined-thing}"),
            "the reference must be left literal: {folded:?}"
        );

        // The string pipeline, by contrast, drops the line the reference sits
        // on — which here is the span's own second half.
        assert_eq!(golden_attributes_with(source, &parser), "<em>a");
    }

    #[test]
    fn a_missing_reference_inside_a_multi_line_span_diverges_under_drop_too() {
        // The same guard applies under `drop`: it is the line *correspondence*
        // that is unsound, not the drop rule, so both modes fall back.
        let parser = parser_with_missing_mode("drop");
        let source = "_a\n{undefined-thing}\nb_";

        let nodes = build(Span::new(source), &parser, None);
        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});

        assert!(
            folded.contains("{undefined-thing}"),
            "the reference must be left literal: {folded:?}"
        );

        // The string pipeline drops the (now empty) middle line from inside
        // the span's rendered markup.
        assert_eq!(golden_attributes_with(source, &parser), "<em>a\nb</em>");
    }
}
