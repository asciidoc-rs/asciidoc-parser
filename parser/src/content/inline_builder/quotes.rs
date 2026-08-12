//! The quoted-text substitution step.

use crate::{
    HasSpan, Parser, Span,
    attributes::{Attrlist, AttrlistContext},
    content::{QuoteSub, maybe_has_quotes, quote_subs},
    inlines::{CharRef, InlineNode, SpanForm, StyleVariant, Styled},
    parser::{QuoteScope, QuoteType},
    strings::CowStr,
};

/// A single opaque codepoint standing in for a whole [`Styled`] span (produced
/// by an earlier sub) while a later sub matches at that span's level. It is in
/// the Unicode Private Use Area, so – like a rendered `<strong>…</strong>` in
/// the string pipeline – it is a single non-word, non-space boundary character
/// that a quote pattern treats as opaque content.
pub(super) const SPAN_PLACEHOLDER: char = '\u{E0F0}';

/// The quoted-text substitution, as a node transducer: each shared
/// [`quote_subs`] rule is applied to the tree in order (its order encodes
/// Asciidoctor's precedence), wrapping every matched run in a [`Styled`] span.
///
/// `root` is the whole-content source span; every node's precise `location` is
/// sliced from it. `parser` parses an attributed quote's attribute list.
pub(super) fn apply_quotes<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    let mut nodes = nodes;

    for sub in quote_subs() {
        nodes = apply_quote_sub(sub, nodes, root, parser);
    }

    nodes
}

/// Applies one [`QuoteSub`] to `nodes`, first descending into the [`Styled`]
/// spans earlier subs created (so this sub can match *inside* them – the
/// nesting case), then matching and wrapping at this level.
fn apply_quote_sub<'src>(
    sub: &QuoteSub,
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    // Recurse into the spans produced by earlier subs *before* matching at this
    // level. A span this sub itself creates below is therefore never revisited
    // by the same sub, matching the string pipeline (a sub runs once).
    let nodes: Vec<InlineNode<'src>> = nodes
        .into_iter()
        .map(|node| match node {
            InlineNode::Styled(mut styled) => {
                styled.children = apply_quote_sub(sub, styled.children, root, parser);
                InlineNode::Styled(styled)
            }

            other => other,
        })
        .collect();

    match_level(sub, nodes, root, parser)
}

/// One leaf/opaque node's placement in a level's reconstructed match string.
pub(super) struct Piece {
    /// Index of the node in the level's node vector.
    pub(super) node_index: usize,

    /// Byte offset where this piece begins in the match string.
    pub(super) s_start: usize,

    /// Byte length of this piece in the match string.
    pub(super) s_len: usize,

    /// Absolute source byte offset of the node's `location`.
    pub(super) src_offset: usize,

    /// Byte length of the node's `location` source.
    pub(super) src_len: usize,

    /// Whether the piece is indivisible. Only a verbatim
    /// [`Text`](InlineNode::Text) run or a [`synthesized`](Self::synthesized)
    /// one can be split by a match boundary; everything else
    /// ([`CharRef`](InlineNode::CharRef) entities, opaque spans) is atomic.
    pub(super) atomic: bool,

    /// Whether the piece is a [`Text`](InlineNode::Text) run whose `value`
    /// was synthesized (an attribute expansion, a `counter` directive, …) –
    /// its `value` differs from `location.data()`, so unlike a verbatim run
    /// its match-string bytes do **not** correspond one-to-one with source
    /// bytes. It contributes its `value` to the match string so a later step
    /// (design §3.4.1: character replacements, macros) can still recognize a
    /// construct inside it, but a match landing here has no honest `'src`
    /// slice: [`emit_range`] slices the node's *value* instead of its
    /// location, and [`s_to_src`] falls back to the piece's whole node span
    /// (design §4.4's coarse fallback) rather than a proportional one.
    pub(super) synthesized: bool,
}

/// Matches `sub` once at this level, wrapping each accepted match in a
/// [`Styled`] span and leaving everything else in place.
fn match_level<'src>(
    sub: &QuoteSub,
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes);

    // Cheap pre-filter: if nothing quote-like is present, no sub can match, so
    // skip building the (owned) result vector entirely.
    if !maybe_has_quotes(&s) {
        return nodes;
    }

    let matches = find_matches(sub, &s);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_level(&nodes, &pieces, &s, &matches, root, parser)
}

/// Reconstructs the escaped match string for a level and the [`Piece`] map back
/// to its nodes.
///
/// A verbatim [`Text`](InlineNode::Text) run contributes its (special-free)
/// value; a [`CharRef`](InlineNode::CharRef) contributes its canonical entity,
/// so the boundary classes the quote patterns key off (`&`, `;`) see exactly
/// what the string pipeline's escaped text presents; a *synthesized*
/// `Text` run (design §3.4.1 – an attribute expansion, a `counter` directive)
/// contributes its `value` too, so [`apply_character_replacements`]
/// (design §3.4.1: `replacements` still runs over an expanded value) can
/// recognize a construct inside it, but is flagged
/// [`synthesized`](Piece::synthesized) since those bytes have no honest
/// `'src` counterpart; every other node contributes a single opaque
/// [`SPAN_PLACEHOLDER`].
///
/// [`apply_character_replacements`]: super::char_replacements::apply_character_replacements
pub(super) fn build_match_string(nodes: &[InlineNode<'_>]) -> (String, Vec<Piece>) {
    let mut s = String::new();
    let mut pieces = Vec::with_capacity(nodes.len());

    for (node_index, node) in nodes.iter().enumerate() {
        let s_start = s.len();

        match node {
            InlineNode::Text { value, location } if value.as_ref() == location.data() => {
                s.push_str(value);

                pieces.push(Piece {
                    node_index,
                    s_start,
                    s_len: value.len(),
                    src_offset: location.byte_offset(),
                    src_len: location.data().len(),
                    atomic: false,
                    synthesized: false,
                });
            }

            InlineNode::Text { value, location } => {
                // A synthesized run (its value has no `'src` slice of its
                // own): still splittable for matching purposes, but any
                // resulting node falls back to this node's whole `location`
                // (design §4.4) rather than a proportional slice of it.
                s.push_str(value);

                pieces.push(Piece {
                    node_index,
                    s_start,
                    s_len: value.len(),
                    src_offset: location.byte_offset(),
                    src_len: location.data().len(),
                    atomic: false,
                    synthesized: true,
                });
            }

            InlineNode::CharRef {
                value: CharRef::Special(ch),
                location,
            } => {
                let entity = special_entity(*ch);
                s.push_str(entity);

                pieces.push(Piece {
                    node_index,
                    s_start,
                    s_len: entity.len(),
                    src_offset: location.byte_offset(),
                    src_len: location.data().len(),
                    atomic: true,
                    synthesized: false,
                });
            }

            other => {
                // A span from an earlier sub (or any other synthesized-value
                // node, e.g. a `Raw` leaf) is opaque: a single placeholder
                // that a quote pattern sees as one boundary character.
                s.push(SPAN_PLACEHOLDER);

                let location = other.span();

                pieces.push(Piece {
                    node_index,
                    s_start,
                    s_len: SPAN_PLACEHOLDER.len_utf8(),
                    src_offset: location.byte_offset(),
                    src_len: location.data().len(),
                    atomic: true,
                    synthesized: false,
                });
            }
        }
    }

    (s, pieces)
}

/// Reports whether any piece overlapping the match-string range `range` is
/// [`synthesized`](Piece::synthesized). A macro family that reconstructs its
/// *shown* text straight from the match string (rather than needing an honest
/// `'src` slice – e.g. an index term's `arg`/`term_src`, already checked
/// against [`SPAN_PLACEHOLDER`] for a crossed span) still needs this check
/// too: a synthesized run's bytes have no source counterpart, and design
/// §3.4.1 leaves recognizing a macro *inside* one for a later increment (see
/// [`apply_attribute_references`](super::attribute_refs::apply_attribute_references)'s
/// doc comment) – distinct from
/// [`range_is_verbatim`](super::macros::image::range_is_verbatim),
/// which a family needing to *slice* `'src` (a target, an `Attrlist<'src>`)
/// uses instead and which already rejects a synthesized piece outright.
pub(in crate::content::inline_builder) fn range_overlaps_synthesized(
    pieces: &[Piece],
    range: &std::ops::Range<usize>,
) -> bool {
    pieces.iter().any(|piece| {
        let p_start = piece.s_start;
        let p_end = piece.s_start + piece.s_len;

        piece.synthesized && p_end > range.start && p_start < range.end
    })
}

/// The canonical special-character entity a [`CharRef::Special`] contributes to
/// the match string. These are the AsciiDoc-standard escapes the quote patterns
/// were written against, independent of the render-time backend.
fn special_entity(ch: char) -> &'static str {
    match ch {
        '<' => "&lt;",
        '>' => "&gt;",

        // `&` and any other special the step recognizes.
        _ => "&amp;",
    }
}

/// One accepted quote match at a level, in absolute match-string byte offsets.
struct QuoteMatch {
    /// The whole match, `[start, end)`.
    full: std::ops::Range<usize>,

    /// What to emit in place of `full`.
    kind: QuoteMatchKind,
}

enum QuoteMatchKind {
    /// An escaped construct (`\*x*`): drop the leading backslash, keep the rest
    /// as literal text, wrap nothing.
    Unescape,

    /// A recognized construct to wrap in a [`Styled`] span.
    Wrap {
        /// For the escaped-with-attributes case (`\[a]*x*`), the literal text
        /// (`[a]`) to keep before the span even though the construct is still
        /// wrapped. `None` for the ordinary case, where the text before the
        /// span (any boundary prefix) is emitted as part of the preceding gap.
        keep_literal: Option<std::ops::Range<usize>>,

        /// The span's body (its children), `[start, end)`.
        body: std::ops::Range<usize>,

        /// The full construct (delimiters included, boundary prefix excluded),
        /// used as the span's `location`. Its start is where the kept text
        /// ends, so a gap that runs to `construct.start` absorbs the
        /// prefix.
        construct: std::ops::Range<usize>,

        /// The attribute list's inner source range (`[…]` without brackets), if
        /// one was present.
        attrlist: Option<std::ops::Range<usize>>,

        variant: StyleVariant,
        form: SpanForm,
    },
}

/// Drives `sub` over the match string, mirroring the string pipeline's
/// look-ahead retry: a rejected monospace-before-quote match slices the
/// haystack forward and re-searches, exactly as [`replace_with_lookahead`]
/// does.
///
/// [`replace_with_lookahead`]: crate::internal::replace_with_lookahead
fn find_matches(sub: &QuoteSub, s: &str) -> Vec<QuoteMatch> {
    let mut matches = Vec::new();

    // Absolute offset of the current (possibly sliced) haystack within `s`.
    let mut base = 0usize;

    'retry: loop {
        let haystack = &s[base..];

        for caps in sub.pattern.captures_iter(haystack) {
            // `unwrap` on group 0 is safe: a capture always has an overall
            // match.
            #[allow(clippy::unwrap_used)]
            let whole = caps.get(0).unwrap();

            let full = (base + whole.start())..(base + whole.end());
            let after = &haystack[whole.end()..];

            // The monospace-constrained-before-quote look-ahead: reject and
            // resume a few bytes in, so the following quote can be recognized.
            if sub.type_ == QuoteType::Monospaced
                && sub.scope == QuoteScope::Constrained
                && after.starts_with(['"', '\'', '`'])
            {
                let skip = if whole.as_str().starts_with('\\') {
                    2
                } else {
                    whole.as_str().chars().next().map_or(1, char::len_utf8)
                };

                base = full.start + skip;
                continue 'retry;
            }

            if let Some(m) = classify_match(sub, &caps, base) {
                matches.push(QuoteMatch { full, kind: m });
            } else {
                matches.push(QuoteMatch {
                    full,
                    kind: QuoteMatchKind::Unescape,
                });
            }
        }

        break;
    }

    matches
}

/// Classifies one raw capture into the [`Styled`] span it produces, or `None`
/// when it is an escaped construct that wraps nothing.
///
/// Group numbering follows the shared patterns: a *constrained* sub captures
/// `(prefix)(attrlist?)(body)`; an *unconstrained* sub captures
/// `(attrlist?)(body)`. `base` maps a capture offset (into the current
/// haystack) back to an absolute offset in the whole match string.
fn classify_match(
    sub: &QuoteSub,
    caps: &regex::Captures<'_>,
    base: usize,
) -> Option<QuoteMatchKind> {
    let abs = |r: std::ops::Range<usize>| (base + r.start)..(base + r.end);

    // `unwrap` on group 0 is safe: a capture always has an overall match.
    #[allow(clippy::unwrap_used)]
    let whole = caps.get(0).unwrap();

    let escaped = whole.as_str().starts_with('\\');

    match sub.scope {
        QuoteScope::Constrained => {
            let prefix_end = caps.get(1).map_or(base, |m| base + m.end());
            let attrlist = caps.get(2).map(|m| abs(m.range()));
            let body = caps.get(3).map(|m| abs(m.range()))?;

            if escaped {
                // `\[a]*x*`: the escape keeps `[a]` literal but still wraps the
                // body with no attribute list. `\*x*` (no attrs) wraps nothing.
                let attrlist = attrlist?;

                return Some(QuoteMatchKind::Wrap {
                    // `[a]` including the brackets: one byte before the inner
                    // capture through one byte after it.
                    keep_literal: Some((attrlist.start - 1)..(attrlist.end + 1)),
                    body,
                    construct: (attrlist.end + 1)..(base + whole.end()),
                    attrlist: None,
                    variant: style_variant(sub.type_, false),
                    form: SpanForm::Constrained,
                });
            }

            let has_attrs = attrlist.is_some();

            // The construct (for the span's `location`) is everything the match
            // consumed except the kept boundary prefix: `[attrs]delim body
            // delim`. Its start is `prefix_end`, so the preceding gap absorbs
            // the prefix into one contiguous run of kept text.
            Some(QuoteMatchKind::Wrap {
                keep_literal: None,
                construct: prefix_end..(base + whole.end()),
                body,
                attrlist,
                variant: style_variant(sub.type_, has_attrs),
                form: SpanForm::Constrained,
            })
        }

        QuoteScope::Unconstrained => {
            // An escaped unconstrained construct always wraps nothing (the
            // attribute-list group is never treated as attrs here).
            if escaped {
                return None;
            }

            let attrlist = caps.get(1).map(|m| abs(m.range()));
            let body = caps.get(2).map(|m| abs(m.range()))?;
            let has_attrs = attrlist.is_some();

            Some(QuoteMatchKind::Wrap {
                keep_literal: None,
                construct: (base + whole.start())..(base + whole.end()),
                body,
                attrlist,
                variant: style_variant(sub.type_, has_attrs),
                form: SpanForm::Unconstrained,
            })
        }
    }
}

/// Rebuilds a level's node list from its matches: unmatched gaps keep their
/// original nodes; each match becomes kept prefix/literal text plus (for a
/// wrap) a [`Styled`] span over the mapped-back body nodes.
fn rebuild_level<'src>(
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    s: &str,
    matches: &[QuoteMatch],
    root: Span<'src>,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    let mut out = Vec::new();
    let mut cursor = 0usize;

    for m in matches {
        match &m.kind {
            QuoteMatchKind::Unescape => {
                // Emit the gap, then drop the leading backslash and keep the
                // remainder as literal text.
                emit_range(nodes, pieces, cursor..m.full.start, &mut out);
                emit_range(nodes, pieces, (m.full.start + 1)..m.full.end, &mut out);
            }

            QuoteMatchKind::Wrap {
                keep_literal,
                body,
                construct,
                attrlist,
                variant,
                form,
            } => {
                match keep_literal {
                    // Ordinary case: the gap runs all the way to the construct,
                    // absorbing any boundary prefix into one contiguous run of
                    // kept text.
                    None => emit_range(nodes, pieces, cursor..construct.start, &mut out),

                    // Escaped-with-attributes: the gap stops at the backslash,
                    // then the `[a]` literal (skipping the backslash) is kept.
                    Some(literal) => {
                        emit_range(nodes, pieces, cursor..m.full.start, &mut out);
                        emit_range(nodes, pieces, literal.clone(), &mut out);
                    }
                }

                let mut children = Vec::new();
                emit_range(nodes, pieces, body.clone(), &mut children);

                let (id, roles, attrs) = match attrlist {
                    Some(range) => attributes_of(source_slice(pieces, range.clone(), root), parser),

                    None => (None, Vec::new(), None),
                };

                out.push(InlineNode::Styled(Styled {
                    variant: *variant,
                    form: *form,
                    id,
                    roles,
                    attrs,
                    children,
                    location: source_slice(pieces, construct.clone(), root),
                }));
            }
        }

        cursor = m.full.end;
    }

    // Emit the trailing gap.
    if cursor < s.len() {
        emit_range(nodes, pieces, cursor..s.len(), &mut out);
    }

    out
}

/// Parses an attributed quote's attribute list, returning the id, roles, and
/// the full [`Attrlist`] (kept so the fold renders exactly as the string
/// pipeline).
///
/// Shared with the attribute-list-prefixed passthrough forms
/// ([`passthrough_step`](super::passthrough_step)), which parse their own
/// attrlist the same way and fold through the same
/// [`Styled`] node.
pub(super) fn attributes_of<'src>(
    source: Span<'src>,
    parser: &Parser,
) -> (
    Option<CowStr<'src>>,
    Vec<CowStr<'src>>,
    Option<Attrlist<'src>>,
) {
    let attrlist = Attrlist::parse(source, parser, AttrlistContext::Inline)
        .item
        .item;

    // Extract owned id/roles before the attrlist is moved into the node, exactly
    // as the string pipeline's quote replacer does.
    //
    // Unlike that replacer, this deliberately performs *no* side effect: it does
    // not `register_ref` an assigned id in the catalog, because the builder is
    // additive and not yet the recognition sink – the authoritative string
    // pipeline still registers it. The cutover (design §5.2 Phase 4, step 6)
    // must add that registration so cross-references to an inline id resolve
    // (tracked by #1087).
    let id = attrlist.id().map(|id| CowStr::from(id.to_string()));

    let roles = attrlist
        .roles()
        .into_iter()
        .map(|role| CowStr::from(role.to_string()))
        .collect();

    (id, roles, Some(attrlist))
}

/// Emits the original nodes covering the match-string range `[range.start,
/// range.end)` into `out`, slicing a verbatim [`Text`](InlineNode::Text) run at
/// the boundaries and cloning any atomic piece that falls inside.
pub(super) fn emit_range<'src>(
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    range: std::ops::Range<usize>,
    out: &mut Vec<InlineNode<'src>>,
) {
    // An empty range (e.g. a macro whose node consumes its whole match, so the
    // kept-suffix range is zero-width) emits nothing – never a spurious empty
    // `Text` node sliced from a piece the range merely touches.
    if range.start >= range.end {
        return;
    }

    for piece in pieces {
        let p_start = piece.s_start;
        let p_end = piece.s_start + piece.s_len;

        // Skip pieces that do not overlap the requested range.
        if p_end <= range.start || p_start >= range.end {
            continue;
        }

        let Some(node) = nodes.get(piece.node_index) else {
            continue;
        };

        if piece.atomic {
            // An atomic piece should fall wholly inside the range; a boundary
            // that splits one is a recognition edge this increment does not
            // claim, so clone the whole piece and let the differential corpus
            // catch any real divergence.
            out.push(node.clone());
            continue;
        }

        // A verbatim or synthesized text run: slice it to the overlap.
        let lo = range.start.max(p_start) - p_start;
        let hi = range.end.min(p_end) - p_start;

        if let InlineNode::Text { value, location } = node {
            if piece.synthesized {
                // No `'src` slice exists for these bytes: slice the node's
                // *value* instead, keeping the whole original `location` as
                // the coarse fallback span (design §4.4) – the same policy
                // `split_attribute_value` already applies to every fragment
                // of an expanded value.
                let Some(sliced) = value.get(lo..hi) else {
                    continue;
                };

                out.push(InlineNode::Text {
                    value: CowStr::from(sliced.to_string()),
                    location: *location,
                });
            } else {
                let sliced = location.slice(lo..hi);

                out.push(InlineNode::Text {
                    value: CowStr::from(sliced.data()),
                    location: sliced,
                });
            }
        }
    }
}

/// Maps a match-string range back to its source [`Span`], sliced from `root`.
///
/// A boundary inside a verbatim [`Text`](InlineNode::Text) run maps one-to-one
/// (its match text is its source text); a boundary inside an atomic or
/// [`synthesized`](Piece::synthesized) piece has no such honest source
/// position, so it falls back to that piece's own edges (design §4.4) –
/// snapping to the *nearer* one for an atomic piece (it never legitimately
/// falls there), or to the edge [`Bias`] names for a synthesized one, so a
/// range wholly inside a synthesized run maps to that run's *whole* node span
/// regardless of exactly where the range's boundaries land in it – the same
/// coarse policy `split_attribute_value` (in
/// [`attribute_refs`](super::attribute_refs)) already gives every fragment of
/// an expanded value.
pub(super) fn source_slice<'src>(
    pieces: &[Piece],
    range: std::ops::Range<usize>,
    root: Span<'src>,
) -> Span<'src> {
    let start = s_to_src(pieces, range.start, Bias::Start);
    let end = s_to_src(pieces, range.end, Bias::End);

    let base = root.byte_offset();
    root.slice(start.saturating_sub(base)..end.saturating_sub(base))
}

/// Which edge of a piece [`s_to_src`] falls back to when a boundary lands
/// inside a [`synthesized`](Piece::synthesized) piece, matching whether the
/// boundary is a range's start or end.
#[derive(Clone, Copy)]
enum Bias {
    Start,
    End,
}

/// Maps a single match-string byte offset back to an absolute source byte
/// offset. `bias` only matters for a boundary landing inside a `synthesized`
/// piece; an atomic piece keeps its own nearer-edge snap regardless of it.
fn s_to_src(pieces: &[Piece], x: usize, bias: Bias) -> usize {
    for piece in pieces {
        let p_start = piece.s_start;
        let p_end = piece.s_start + piece.s_len;

        if x < p_start {
            break;
        }

        // A boundary exactly at the *end* of a synthesized piece belongs to
        // whatever comes next, not to this piece: unlike a verbatim piece
        // (whose `s_len` and `src_len` always agree, so its own end edge and
        // the next piece's start edge are numerically the same value either
        // way), a synthesized piece's `value` generally has a *different*
        // byte length than its source span, so the plain linear mapping below
        // is only honest at this piece's own `p_start` (a zero delta) – never
        // at `p_end`. Skipping to the next piece (or the past-the-last-piece
        // fallback, if there is none) lets that boundary resolve correctly
        // instead.
        if piece.synthesized && x == p_end {
            continue;
        }

        if x <= p_end {
            if piece.atomic {
                // Snap to the nearer edge; a boundary never legitimately lands
                // inside an atomic piece.
                return if x - p_start <= p_end - x {
                    piece.src_offset
                } else {
                    piece.src_offset + piece.src_len
                };
            }

            // A boundary strictly *inside* a synthesized piece (not on its
            // `p_start` edge – already excluded `p_end` above, and `p_start`
            // is exact via the plain mapping just like a verbatim piece) has
            // no honest source position, so it falls back to the edge `bias`
            // names (design §4.4's coarse fallback).
            if piece.synthesized && x > p_start {
                return match bias {
                    Bias::Start => piece.src_offset,
                    Bias::End => piece.src_offset + piece.src_len,
                };
            }

            return piece.src_offset + (x - p_start);
        }
    }

    // Past the last piece: the end of the source the pieces cover, or the anchor
    // for an empty level.
    pieces
        .last()
        .map_or(0, |last| last.src_offset + last.src_len)
}

/// Maps a [`QuoteType`] to its [`Styled`] variant, downgrading an attributed
/// `mark` to an unquoted span exactly as the string pipeline does.
fn style_variant(type_: QuoteType, has_attrlist: bool) -> StyleVariant {
    match type_ {
        QuoteType::Strong => StyleVariant::Strong,
        QuoteType::Emphasis => StyleVariant::Emphasis,
        QuoteType::Monospaced => StyleVariant::Code,
        QuoteType::Mark if has_attrlist => StyleVariant::Unquoted,
        QuoteType::Mark => StyleVariant::Mark,
        QuoteType::Superscript => StyleVariant::Superscript,
        QuoteType::Subscript => StyleVariant::Subscript,
        QuoteType::DoubleQuote => StyleVariant::DoubleQuote,
        QuoteType::SingleQuote => StyleVariant::SingleQuote,

        // AsciiMath/LatexMath/Unquoted are not produced by `quote_subs`; map any
        // residue to the unquoted span.
        _ => StyleVariant::Unquoted,
    }
}

/// The inverse of [`style_variant`]: the [`QuoteType`] the fold renders a
/// [`Styled`] variant with.
pub(super) fn quote_type_of(variant: StyleVariant) -> QuoteType {
    match variant {
        StyleVariant::Strong => QuoteType::Strong,
        StyleVariant::Emphasis => QuoteType::Emphasis,
        StyleVariant::Code => QuoteType::Monospaced,
        StyleVariant::Mark => QuoteType::Mark,
        StyleVariant::Superscript => QuoteType::Superscript,
        StyleVariant::Subscript => QuoteType::Subscript,
        StyleVariant::DoubleQuote => QuoteType::DoubleQuote,
        StyleVariant::SingleQuote => QuoteType::SingleQuote,
        StyleVariant::Unquoted => QuoteType::Unquoted,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use super::super::test_support::{
        assert_styled, assert_text, build_src, build_through_quotes, fold_html,
    };
    use crate::{
        HasSpan, Span,
        content::{Content, SubstitutionStep},
        inlines::{CharRef, InlineNode, SpanForm, StyleVariant},
        parser::HtmlSubstitutionRenderer,
        strings::CowStr,
    };

    /// The string pipeline's output through the **quotes** step for `source`,
    /// used as the golden oracle: `Content::from` then `SpecialCharacters` then
    /// `Quotes`, exactly the order [`build`] runs them.
    fn golden_quotes(source: &str) -> String {
        let parser = crate::Parser::default();
        let mut content = Content::from(Span::new(source));
        SubstitutionStep::SpecialCharacters.apply(&mut content, &parser, None);
        SubstitutionStep::Quotes.apply(&mut content, &parser, None);
        content.rendered_str().to_string()
    }

    #[test]
    fn fold_matches_the_string_pipeline_through_quotes() {
        // For each fixture, folding the single-pass tree (special characters +
        // quotes) reproduces the string pipeline's output byte-for-byte. This is
        // the differential corpus (design §5.3) that pins the quotes increment.
        let fixtures = [
            // No quotes.
            "plain text",
            "a < b & c > d",
            // Constrained, the core variants.
            "*bold*",
            "_italic_",
            "`code`",
            "#mark#",
            "a *bold* here",
            "punctuation: *bold*, and _more_.",
            // Unconstrained.
            "**bo**ld",
            "__it__alic",
            "##ma##rk",
            "un**con**strained",
            // Superscript / subscript (always unconstrained, single char).
            "H~2~O",
            "E = mc^2^",
            // Smart quotes.
            "\"`double`\"",
            "'`single`'",
            // Nesting: a later sub matches inside an earlier span's body.
            "*a _b_ c*",
            "*a `b` c*",
            "_*strong* in em_",
            "`*b* and _i_`",
            // Specials adjacent to quotes exercise the escaped-boundary classes.
            "*a<b>c*",
            "before < *bold* > after",
            // Escapes: constrained, unconstrained, and escaped-with-attributes.
            "\\*not bold*",
            "\\_not italic_",
            "\\**not bold**",
            "\\[role]*bold*",
            // Escaped constrained quotes *after* a boundary character: the
            // backslash is the match's leading boundary group, so it is still
            // recognized as an escape (not wrapped).
            "a \\*x*",
            "foo \\_bar_",
            "x \\`code`",
            "word \\#m#",
            // Monospace-constrained followed by a quote character triggers the
            // look-ahead retry.
            "a `b`\" c",
            "x `y`' z",
            "\\`m`\" n",
            // Roles / ids on a span.
            "[.lead]#tagline#",
            "[#anchor]*bold*",
            "[.a.b]_x_",
            "['quoted role']#x#",
            "[.role1.role2]#x#",
            // Deeper nesting and repeated spans.
            "nested *_`all three`_* here",
            "*a* *b* *c*",
            "*x*y*z*",
            "**a *b* c**",
            "a `b` `c` `d` e",
            // A run that spans a newline.
            "multi\nline *strong\nspan* end",
            // Monospace-before-quote look-ahead.
            "{leading}`code``",
            "a `b\"c` d",
            // Specials interacting with the escaped boundary classes.
            "`<code>&amp;</code>`",
            "*bold with ` backtick*",
            // No match despite a quote-like character.
            "1 * 2 * 3",
            "a_b_c",
            "* not a list *",
        ];

        let renderer = HtmlSubstitutionRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_through_quotes(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden_quotes(fixture),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn s_to_src_guards_are_defensive() {
        use super::{Bias, Piece, s_to_src};

        // In practice every boundary `s_to_src` maps falls on a literal
        // delimiter (a text position), so the atomic-snap, before-first, and
        // past-last branches are defensive. Exercise them directly to document
        // the intended fallback. Bias is irrelevant to an atomic piece, so
        // `Bias::Start` is used throughout.
        let atomic = Piece {
            node_index: 0,
            s_start: 0,
            s_len: 4,
            src_offset: 10,
            src_len: 1,
            atomic: true,
            synthesized: false,
        };

        // A boundary inside an atomic piece snaps to the nearer edge.
        assert_eq!(s_to_src(std::slice::from_ref(&atomic), 1, Bias::Start), 10);
        assert_eq!(s_to_src(std::slice::from_ref(&atomic), 3, Bias::Start), 11);

        // A boundary past the last piece falls back to the source end.
        assert_eq!(s_to_src(std::slice::from_ref(&atomic), 9, Bias::Start), 11);

        // A boundary before the first piece begins breaks out to the same
        // fallback.
        let offset = Piece {
            s_start: 2,
            ..atomic
        };

        assert_eq!(s_to_src(std::slice::from_ref(&offset), 0, Bias::Start), 11);

        // No pieces (an empty level) anchors at the source start.
        assert_eq!(s_to_src(&[], 0, Bias::Start), 0);
    }

    #[test]
    fn s_to_src_biases_a_synthesized_piece_to_its_whole_node_span() {
        use super::{Bias, Piece, s_to_src};

        // A boundary landing *strictly inside* a synthesized piece (its
        // match-string bytes have no honest source counterpart, since its
        // `s_len` here – 9 – differs from its `src_len` – 10) falls back to
        // the whole node span: its start edge (100) for a `Bias::Start`
        // boundary, its end edge (110) for a `Bias::End` one.
        //
        // Its own two edges (`x == 0`, `x == 9`) are a distinct case, pinned
        // by `s_to_src_resolves_a_synthesized_pieces_own_edges_exactly`
        // below: `p_start` already has an honest position (delta zero) and
        // `p_end` is skipped to whatever comes next, so *neither* runs
        // through this bias fallback – only interior positions like `3` do.
        let synthesized = Piece {
            node_index: 0,
            s_start: 0,
            s_len: 9,
            src_offset: 100,
            src_len: 10,
            atomic: false,
            synthesized: true,
        };

        assert_eq!(
            s_to_src(std::slice::from_ref(&synthesized), 3, Bias::Start),
            100
        );
        assert_eq!(
            s_to_src(std::slice::from_ref(&synthesized), 3, Bias::End),
            110
        );
    }

    #[test]
    fn s_to_src_resolves_a_synthesized_pieces_own_edges_exactly() {
        use super::{Bias, Piece, s_to_src};

        // A boundary landing exactly on a synthesized piece's own start or
        // end edge has an honest position regardless of `bias` – unlike an
        // interior boundary (see the test above), it never falls back to the
        // coarse whole-node span. This is what keeps a construct recognized
        // *immediately after* a synthesized run (e.g. a second `image:` macro
        // right after an `{sp}` attribute reference) from having its node's
        // location wrongly swallow the synthesized run's own source bytes –
        // a real regression this test reproduces at the `Piece` level
        // (see `image::tests::matches_the_golden_pipelines_registration_for_a_broad_fixture_set`
        // for the end-to-end fixture that first caught it).
        fn synthesized_piece() -> Piece {
            Piece {
                node_index: 0,
                s_start: 5,
                s_len: 9, // match-string range [5, 14)
                src_offset: 100,
                src_len: 10, // source range [100, 110)
                atomic: false,
                synthesized: true,
            }
        }

        // A lone synthesized piece: its own start edge is exact for both
        // biases; its own end edge has no *next* piece to defer to, so it
        // falls back to the past-the-last-piece anchor, which for this piece
        // alone is its own end – numerically the same as the whole-node-span
        // fallback here, but arrived at without going through `Bias` at all.
        let pieces = [synthesized_piece()];
        assert_eq!(s_to_src(&pieces, 5, Bias::Start), 100);
        assert_eq!(s_to_src(&pieces, 5, Bias::End), 100);
        assert_eq!(s_to_src(&pieces, 14, Bias::Start), 110);
        assert_eq!(s_to_src(&pieces, 14, Bias::End), 110);

        // With a verbatim piece immediately following (the common case – a
        // recognized construct starting right where the synthesized run
        // ends), the shared boundary (`x == 14`) resolves through the
        // *following* piece's own honest linear mapping instead, giving the
        // same edge value (110) as the piece-alone case above, but arrived
        // at correctly rather than by the synthesized piece's own (invalid,
        // since `s_len != src_len` for it) linear mapping.
        let following = Piece {
            node_index: 1,
            s_start: 14,
            s_len: 4,
            src_offset: 110,
            src_len: 4,
            atomic: false,
            synthesized: false,
        };
        let pieces = [synthesized_piece(), following];
        assert_eq!(s_to_src(&pieces, 14, Bias::Start), 110);
        assert_eq!(s_to_src(&pieces, 14, Bias::End), 110);
        assert_eq!(s_to_src(&pieces, 16, Bias::Start), 112);
    }

    #[test]
    fn emit_range_skips_a_stale_piece_index() {
        use super::{Piece, emit_range};

        // A piece whose `node_index` no longer resolves is skipped rather than
        // panicking (defensive against an internal invariant slip).
        let piece = Piece {
            node_index: 9,
            s_start: 0,
            s_len: 1,
            src_offset: 0,
            src_len: 1,
            atomic: true,
            synthesized: false,
        };

        let mut out = Vec::new();
        emit_range(&[], std::slice::from_ref(&piece), 0..1, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn emit_range_skips_a_synthesized_piece_whose_declared_length_overruns_its_value() {
        use super::{Piece, emit_range};
        use crate::{Span, inlines::InlineNode, strings::CowStr};

        // A synthesized piece's `s_len` is expected to equal its node's
        // `value.len()` (how `build_match_string` constructs one); a piece
        // declaring a longer `s_len` than its node's actual `value` – an
        // internal invariant slip – is skipped rather than panicking, the
        // same defensive posture as a stale `node_index` above.
        let location = Span::new("{x}");

        let node = InlineNode::Text {
            value: CowStr::from("ab"),
            location,
        };

        let piece = Piece {
            node_index: 0,
            s_start: 0,
            s_len: 5, // overruns "ab"'s 2 bytes
            src_offset: location.byte_offset(),
            src_len: location.data().len(),
            atomic: false,
            synthesized: true,
        };

        let mut out = Vec::new();
        emit_range(
            std::slice::from_ref(&node),
            std::slice::from_ref(&piece),
            0..5,
            &mut out,
        );
        assert!(out.is_empty(), "{out:?}");
    }

    #[test]
    fn style_variant_maps_non_quote_types_to_unquoted() {
        use super::style_variant;

        // `quote_subs` never yields these, so the residue arm is defensive.
        assert_eq!(
            style_variant(crate::parser::QuoteType::AsciiMath, false),
            StyleVariant::Unquoted
        );
        assert_eq!(
            style_variant(crate::parser::QuoteType::LatexMath, true),
            StyleVariant::Unquoted
        );
        assert_eq!(
            style_variant(crate::parser::QuoteType::Unquoted, false),
            StyleVariant::Unquoted
        );
    }

    #[test]
    fn crossed_delimiters_are_a_documented_divergence() {
        // `` `a *b` c* `` interleaves a monospace and a strong span so their
        // ranges *overlap* rather than nest. The string pipeline, rewriting a
        // flat string, emits crossed – malformed – HTML tags (`<code>…<strong>…
        // </code>…</strong>`) that no tree can represent. The single-pass
        // builder instead treats an earlier span as opaque, so it produces a
        // well-formed tree (here, monospace wrapping a strong span). This is the
        // documented boundary of the single-pass recognition (see the module
        // docs): for pathological cross-span input the two intentionally differ.
        let source = "`a *b` c*";

        let folded = fold_html(
            &build_through_quotes(Span::new(source)),
            &HtmlSubstitutionRenderer {},
        );
        let golden = golden_quotes(source);

        // The string pipeline's crossed tags: monospace matched *through* the
        // rendered `<strong>` tag, so `</code>` closes before `</strong>`.
        assert_eq!(golden, "<code>a <strong>b</code> c</strong>");

        // The builder sealed the inner backtick inside the opaque strong span,
        // so no monospace is recognized; the leading backtick stays literal and
        // the tree stays well-formed. It deliberately differs from the crossed
        // golden output.
        assert_ne!(folded, golden);
        assert_eq!(folded, "`a <strong>b` c</strong>");
    }

    #[test]
    fn constrained_strong_is_a_span_over_its_body() {
        let nodes = build_src(Span::new("*bold*"));

        assert_eq!(nodes.len(), 1);
        let children = assert_styled(&nodes[0], StyleVariant::Strong, SpanForm::Constrained);

        // The delimiters are consumed; the child is the borrowed body, precisely
        // located just past the opening `*`.
        assert_eq!(children.len(), 1);
        assert_text(&children[0], "bold", 1, 2);

        // The span's own location covers the whole construct, delimiters
        // included.
        assert_eq!(nodes[0].span().data(), "*bold*");
    }

    #[test]
    fn a_boundary_prefix_is_kept_before_the_span() {
        let nodes = build_src(Span::new("a *bold*"));

        // "a " is kept as text (the boundary prefix `[^\w…]` is not consumed),
        // then the span.
        assert_eq!(nodes.len(), 2);
        assert_text(&nodes[0], "a ", 1, 1);

        let children = assert_styled(&nodes[1], StyleVariant::Strong, SpanForm::Constrained);
        assert_text(&children[0], "bold", 1, 4);
    }

    #[test]
    fn emphasis_nests_inside_strong() {
        // The canonical nesting example: constrained emphasis matches inside the
        // body of the strong span an earlier sub created.
        let nodes = build_src(Span::new("*a _b_ c*"));

        assert_eq!(nodes.len(), 1);
        let children = assert_styled(&nodes[0], StyleVariant::Strong, SpanForm::Constrained);

        assert_eq!(children.len(), 3);
        assert_text(&children[0], "a ", 1, 2);

        let inner = assert_styled(&children[1], StyleVariant::Emphasis, SpanForm::Constrained);
        assert_eq!(inner.len(), 1);
        assert_text(&inner[0], "b", 1, 5);

        assert_text(&children[2], " c", 1, 7);
    }

    #[test]
    fn monospace_nests_inside_strong() {
        // A later sub (constrained monospace) matches inside the strong body.
        let nodes = build_src(Span::new("*a `b` c*"));

        let children = assert_styled(&nodes[0], StyleVariant::Strong, SpanForm::Constrained);
        assert_eq!(children.len(), 3);
        assert_text(&children[0], "a ", 1, 2);

        let inner = assert_styled(&children[1], StyleVariant::Code, SpanForm::Constrained);
        assert_text(&inner[0], "b", 1, 5);
    }

    #[test]
    fn unconstrained_strong_is_recognized() {
        let nodes = build_src(Span::new("**bo**ld"));

        assert_eq!(nodes.len(), 2);
        let children = assert_styled(&nodes[0], StyleVariant::Strong, SpanForm::Unconstrained);
        assert_text(&children[0], "bo", 1, 3);
        assert_text(&nodes[1], "ld", 1, 7);
    }

    #[test]
    fn a_char_ref_inside_a_span_is_preserved_as_a_child() {
        // The special character splits into a `CharRef` child of the span; the
        // fold re-escapes it, matching the string pipeline (covered by the
        // corpus) while the structure exposes the entity.
        let nodes = build_src(Span::new("*a<b*"));

        let children = assert_styled(&nodes[0], StyleVariant::Strong, SpanForm::Constrained);
        assert_eq!(children.len(), 3);
        assert_text(&children[0], "a", 1, 2);

        match &children[1] {
            InlineNode::CharRef {
                value: CharRef::Special('<'),
                ..
            } => {}

            other => panic!("expected CharRef::Special('<'), got {other:?}"),
        }

        assert_text(&children[2], "b", 1, 4);
    }

    #[test]
    fn an_escaped_quote_wraps_nothing() {
        // `\*x*` drops the backslash and keeps the delimiters as literal text.
        let nodes = build_src(Span::new("\\*x*"));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Styled(_))),
            "an escaped quote must not produce a span: {nodes:?}"
        );

        // The fold reproduces the literal text (also covered by the corpus).
        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_quotes("\\*x*")
        );
    }

    #[test]
    fn an_escaped_quote_after_a_boundary_wraps_nothing() {
        // Regression guard for the escape-after-a-boundary case (`a \*x*`): the
        // backslash immediately before the delimiter is the constrained match's
        // *leading boundary group*, hence the first character of the whole
        // match, so it is still recognized as an escape. The construct must stay
        // literal rather than becoming a span.
        let nodes = build_src(Span::new("a \\*x*"));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Styled(_))),
            "an escaped quote after a boundary must not produce a span: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_quotes("a \\*x*")
        );
    }

    #[test]
    fn an_attributed_span_captures_its_roles() {
        let nodes = build_src(Span::new("[.lead]#tagline#"));

        match &nodes[0] {
            InlineNode::Styled(styled) => {
                // `#…#` with an attribute list downgrades from mark to an
                // unquoted span, exactly as the string pipeline does.
                assert_eq!(styled.variant, StyleVariant::Unquoted);
                assert_eq!(styled.roles, vec![CowStr::from("lead")]);
                assert!(styled.attrs.is_some(), "the attribute list is retained");
            }

            other => panic!("expected Styled, got {other:?}"),
        }
    }
}
