//! Builds the inline AST **directly from source in a single forward pass**.
//!
//! This is the first brick of "Strategy B" (design §4.1): rather than recover
//! the tree from a post-substitution *marked string* – the
//! [`inline_tree`](crate::content::inline_tree) recorder's "Strategy A" – each
//! substitution step is recast as a **transducer** over a node list,
//! `Vec<InlineNode<'src>> -> Vec<InlineNode<'src>>`, that refines the tree in
//! place. Two properties fall out that Strategy A cannot offer:
//!
//! 1. **Honest per-node spans.** A node is sliced straight from the source
//!    [`Span`], so its `location` reports the real `line`/`col`/`offset` of the
//!    construct (issue #944), instead of every node carrying the whole-content
//!    span.
//! 2. **`'src` borrowing by construction.** A verbatim text run's `value`
//!    borrows the very bytes its `location` covers, so the common case does not
//!    allocate.
//!
//! # Status
//!
//! Strategy B "touches every step," so it lands incrementally under the
//! golden-HTML oracle. This module currently implements the **foundation** plus
//! these refinements:
//!
//! - [`build`] seeds a single borrowed whole-source [`Text`](InlineNode::Text)
//!   node and threads it through the steps.
//! - [`apply_special_characters`] splits each `Text` run on `<`/`>`/`&` into
//!   precise-span [`Text`](InlineNode::Text) and
//!   [`CharRef`](InlineNode::CharRef) nodes.
//! - [`apply_quotes`] recognizes [quoted text] and wraps each matched run in a
//!   [`Styled`] span, **introducing nesting** – `*a _b_ c*` becomes a tree, not
//!   a flat run. It reuses the *exact* [`quote_subs`] the string pipeline
//!   matches with (changing the recognition *sink*, not the recognition), so
//!   its fold is byte-identical to the string step.
//! - [`apply_character_replacements`] recognizes [character replacements] –
//!   `(C)`, `--`, `...`, arrows, apostrophes, and restored entities – replacing
//!   each with a [`CharRef::Replacement`] or [`CharRef::Entity`] leaf. It
//!   reuses the shared [`character_replacements`] rules and, like the string
//!   step, matches over the *escaped* text so an arrow (`-&gt;`) or entity
//!   (`&amp;copy;`) can straddle a `Text`/`CharRef` boundary.
//! - [`apply_macros`] recognizes **image and icon macros** (`image:target[…]`,
//!   `icon:target[…]`), replacing each with an [`Image`](InlineNode::Image)
//!   node that captures its own owned [`Attrlist`] – the step that makes a
//!   macro node *self-describing*. It reuses the shared [`INLINE_IMAGE_MACRO`]
//!   pattern and builds `'src`-borrowing nodes for verbatim macros only (see
//!   [`apply_macros`] for the boundary the escaped-content case defers). The
//!   other macro families are later increments.
//! - [`apply_post_replacements`] turns a trailing ` +` at the end of a line
//!   into a [`LineBreak`](InlineNode::LineBreak) leaf.
//! - [`fold_html`] folds the resulting leaves and spans back to output bytes
//!   through an [`InlineSubstitutionRenderer`] – the first fold over the
//!   *public* [`InlineNode`] tree (the recorder's [`fold_into`] folds an
//!   intermediate representation, not the public tree).
//!
//! [quoted text]: https://docs.asciidoctor.org/asciidoc/latest/subs/quotes/
//! [character replacements]:
//!     https://docs.asciidoctor.org/asciidoc/latest/subs/replacements/
//!
//! It is **additive and non-regressing**: nothing here is wired into the parse
//! path yet, so the authoritative string pipeline and the Strategy-A
//! [`Content::inlines`](crate::content::Content::inlines) tree are untouched.
//! Later increments extend the transducer to the remaining macro families,
//! attribute expansion, and passthroughs, at which point it can replace the
//! recorder, make `rendered_html()` a fold, and retire the sentinel systems.
//!
//! # A note on quote nesting
//!
//! The string pipeline realizes nesting by running the ordered [`quote_subs`]
//! over one growing string: an earlier sub renders `<strong>…</strong>` and a
//! later sub matches around (or inside) it. [`apply_quotes`] reproduces that by
//! applying each sub to the node tree in turn and, before matching at a level,
//! descending into the [`Styled`] spans earlier subs created – so `*a `b` c*`
//! (strong containing a later-recognized monospace) and `*a _b_ c*` nest
//! correctly. A [`Styled`] span an earlier sub produced is otherwise **opaque**
//! to a later sub at the same level (represented by a single placeholder while
//! matching), which is where this single-pass recognition can, in principle,
//! diverge from the string pipeline's match-through-rendered-tags behavior for
//! pathological cross-span inputs; the differential corpus (§5.3) pins the
//! cases this increment claims.
//!
//! [`fold_into`]: crate::content::inline_tree
//! [`Text`]: InlineNode::Text
//! [`quote_subs`]: crate::content::quote_subs

// The transducer framework is deliberately broader than the single step wired
// up so far; later Strategy-B increments consume the rest.
#![allow(dead_code)]

use crate::{
    HasSpan, Parser, Span,
    attributes::{Attrlist, AttrlistContext},
    content::{
        CharacterReplacement, INLINE_IMAGE_MACRO, QuoteSub, basename, character_replacements,
        hard_line_break_pattern, maybe_has_quotes, maybe_has_replacements,
        normalize_text_lf_escaped_bracket, quote_subs,
    },
    inlines::{CharRef, Image, InlineNode, SpanForm, StyleVariant, Styled},
    parser::{
        CharacterReplacementType, IconRenderParams, ImageRenderParams, InlineSubstitutionRenderer,
        QuoteScope, QuoteType, SpecialCharacter,
    },
    strings::CowStr,
};

/// Builds the inline tree for `source` in a single forward pass.
///
/// The tree is seeded as one borrowed whole-source [`Text`](InlineNode::Text)
/// node and refined by each substitution step in turn. `source` is the exact
/// text to process, so a caller controls precisely what is built; reconciling
/// with a block's line filtering and joining is a later increment's concern.
///
/// `parser` is consulted only to parse the attribute list of an attributed
/// quote (`[.role]#…#`); a caller with no document context can pass a default
/// [`Parser`].
pub(crate) fn build<'src>(source: Span<'src>, parser: &Parser) -> Vec<InlineNode<'src>> {
    let seed = vec![InlineNode::Text {
        value: CowStr::from(source.data()),
        location: source,
    }];

    let nodes = apply_special_characters(seed);
    let nodes = apply_quotes(nodes, source, parser);
    let nodes = apply_character_replacements(nodes, source);
    let nodes = apply_macros(nodes, source, parser);

    apply_post_replacements(nodes, source)
}

/// Reports whether `c` is one of the three characters the special-characters
/// substitution acts on.
fn is_special(c: char) -> bool {
    matches!(c, '<' | '>' | '&')
}

/// The special-characters substitution, as a node transducer: every
/// [`Text`](InlineNode::Text) run is split on `<`/`>`/`&` into
/// [`Text`](InlineNode::Text) and [`CharRef`](InlineNode::CharRef) nodes, and
/// every other node passes through (recursing into parent nodes' children).
///
/// The split is driven by the node's **logical `value`**, not by its source
/// span, so a *synthesized* value – an attribute expansion or a joined
/// multi-line run that a later step may feed in under a custom `subs` order –
/// is preserved rather than replaced by its source spelling. Precise spans are
/// kept for the common verbatim run, where the value coincides with the source
/// its `location` covers; see [`split_text`].
fn apply_special_characters<'src>(nodes: Vec<InlineNode<'src>>) -> Vec<InlineNode<'src>> {
    let mut out = Vec::with_capacity(nodes.len());

    for node in nodes {
        match node {
            InlineNode::Text { value, location } => {
                split_text(value, location, &mut out);
            }

            InlineNode::Styled(mut styled) => {
                styled.children = apply_special_characters(styled.children);
                out.push(InlineNode::Styled(styled));
            }

            InlineNode::Ref(mut reference) => {
                reference.children = apply_special_characters(reference.children);
                out.push(InlineNode::Ref(reference));
            }

            other => out.push(other),
        }
    }

    out
}

/// Splits a [`Text`](InlineNode::Text) node's logical `value` into alternating
/// text runs and `<`/`>`/`&` [`CharRef`](InlineNode::CharRef) specials.
///
/// When `value` is exactly the source its `location` covers – the common
/// verbatim run – each sub-node is sliced from `location`, so its
/// `line`/`col`/`offset` stay honest (issue #944) and its run text borrows from
/// `'src`. When `value` is *synthesized* – it has no source of its own – the
/// runs are owned slices of the value and every sub-node falls back to the
/// whole `location` span, the documented coarse fallback (design §4.4).
fn split_text<'src>(value: CowStr<'src>, location: Span<'src>, out: &mut Vec<InlineNode<'src>>) {
    if value.as_ref() == location.data() {
        split_verbatim(location, out);
    } else {
        split_synthesized(value.as_ref(), location, out);
    }
}

/// Splits a verbatim run – text that coincides with the source `location`
/// covers – slicing each sub-span from `location` with the crate's span
/// primitives so `line`/`col`/`offset` stay honest; a run is never emitted
/// empty.
fn split_verbatim<'src>(location: Span<'src>, out: &mut Vec<InlineNode<'src>>) {
    let mut rest = location;

    while let Some(pos) = rest.position(is_special) {
        // Emit the borrowed text run preceding the special, when non-empty.
        if pos > 0 {
            let text = rest.slice_to(..pos);

            out.push(InlineNode::Text {
                value: CowStr::from(text.data()),
                location: text,
            });
        }

        // The three specials are ASCII, so the match is exactly one byte wide.
        let ch_span = rest.slice(pos..pos + 1);

        let ch = ch_span.data().chars().next().unwrap_or('\u{FFFD}');

        out.push(InlineNode::CharRef {
            value: CharRef::Special(ch),
            location: ch_span,
        });

        rest = rest.slice_from(pos + 1..);
    }

    if !rest.data().is_empty() {
        out.push(InlineNode::Text {
            value: CowStr::from(rest.data()),
            location: rest,
        });
    }
}

/// Splits a synthesized `value` – text with no source span of its own – into
/// owned [`Text`](InlineNode::Text) runs and [`CharRef`](InlineNode::CharRef)
/// specials, each carrying the whole `location` as its coarse fallback span; a
/// run is never emitted empty.
fn split_synthesized<'src>(value: &str, location: Span<'src>, out: &mut Vec<InlineNode<'src>>) {
    let mut rest = value;

    while let Some(pos) = rest.find(is_special) {
        // Emit the owned text run preceding the special, when non-empty.
        if pos > 0 {
            out.push(InlineNode::Text {
                value: CowStr::from(rest[..pos].to_string()),
                location,
            });
        }

        // The three specials are ASCII, so the match is exactly one byte wide.
        let ch = rest[pos..].chars().next().unwrap_or('\u{FFFD}');

        out.push(InlineNode::CharRef {
            value: CharRef::Special(ch),
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

// ─── Quotes ─────────────────────────────────────────────────────────────────

/// A single opaque codepoint standing in for a whole [`Styled`] span (produced
/// by an earlier sub) while a later sub matches at that span's level. It is in
/// the Unicode Private Use Area, so – like a rendered `<strong>…</strong>` in
/// the string pipeline – it is a single non-word, non-space boundary character
/// that a quote pattern treats as opaque content.
const SPAN_PLACEHOLDER: char = '\u{E0F0}';

/// The quoted-text substitution, as a node transducer: each shared
/// [`quote_subs`] rule is applied to the tree in order (its order encodes
/// Asciidoctor's precedence), wrapping every matched run in a [`Styled`] span.
///
/// `root` is the whole-content source span; every node's precise `location` is
/// sliced from it. `parser` parses an attributed quote's attribute list.
fn apply_quotes<'src>(
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
struct Piece {
    /// Index of the node in the level's node vector.
    node_index: usize,

    /// Byte offset where this piece begins in the match string.
    s_start: usize,

    /// Byte length of this piece in the match string.
    s_len: usize,

    /// Absolute source byte offset of the node's `location`.
    src_offset: usize,

    /// Byte length of the node's `location` source.
    src_len: usize,

    /// Whether the piece is indivisible. Only a verbatim
    /// [`Text`](InlineNode::Text) run can be split by a match boundary;
    /// everything else ([`CharRef`](InlineNode::CharRef) entities, opaque
    /// spans) is atomic.
    atomic: bool,
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
/// A [`Text`](InlineNode::Text) run contributes its (special-free) value; a
/// [`CharRef`](InlineNode::CharRef) contributes its canonical entity, so the
/// boundary classes the quote patterns key off (`&`, `;`) see exactly what the
/// string pipeline's escaped text presents; every other node contributes a
/// single opaque [`SPAN_PLACEHOLDER`].
fn build_match_string(nodes: &[InlineNode<'_>]) -> (String, Vec<Piece>) {
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
                });
            }

            other => {
                // A span from an earlier sub (or any node with a synthesized
                // value) is opaque: a single placeholder that a quote pattern
                // sees as one boundary character.
                s.push(SPAN_PLACEHOLDER);

                let location = other.span();

                pieces.push(Piece {
                    node_index,
                    s_start,
                    s_len: SPAN_PLACEHOLDER.len_utf8(),
                    src_offset: location.byte_offset(),
                    src_len: location.data().len(),
                    atomic: true,
                });
            }
        }
    }

    (s, pieces)
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
fn attributes_of<'src>(
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
fn emit_range<'src>(
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    range: std::ops::Range<usize>,
    out: &mut Vec<InlineNode<'src>>,
) {
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

        // A verbatim text run: slice it to the overlap.
        let lo = range.start.max(p_start) - p_start;
        let hi = range.end.min(p_end) - p_start;

        if let InlineNode::Text { location, .. } = node {
            let sliced = location.slice(lo..hi);

            out.push(InlineNode::Text {
                value: CowStr::from(sliced.data()),
                location: sliced,
            });
        }
    }
}

/// Maps a match-string range back to its source [`Span`], sliced from `root`.
///
/// A boundary inside a verbatim [`Text`](InlineNode::Text) run maps one-to-one
/// (its match text is its source text); a boundary inside an atomic piece snaps
/// to the nearer edge (it never legitimately falls there).
fn source_slice<'src>(
    pieces: &[Piece],
    range: std::ops::Range<usize>,
    root: Span<'src>,
) -> Span<'src> {
    let start = s_to_src(pieces, range.start);
    let end = s_to_src(pieces, range.end);

    let base = root.byte_offset();
    root.slice(start.saturating_sub(base)..end.saturating_sub(base))
}

/// Maps a single match-string byte offset back to an absolute source byte
/// offset.
fn s_to_src(pieces: &[Piece], x: usize) -> usize {
    for piece in pieces {
        let p_start = piece.s_start;
        let p_end = piece.s_start + piece.s_len;

        if x < p_start {
            break;
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
fn quote_type_of(variant: StyleVariant) -> QuoteType {
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

// ─── Character replacements ───────────────────────────────────────────────

/// The character-replacements substitution, as a node transducer: each shared
/// [`character_replacements`] rule is applied to the tree in order (its order
/// encodes Asciidoctor's precedence), replacing every matched construct with a
/// [`CharRef::Replacement`] (a typographic replacement such as `(C)` → `©`) or
/// a [`CharRef::Entity`] (a restored named/numeric entity such as `&amp;copy;`
/// → `&copy;`) leaf.
///
/// Like the string pipeline's step, the rules match over the level's
/// **escaped** text (built by [`build_match_string`], where a
/// [`CharRef::Special`] contributes its canonical entity) – which is exactly
/// why the arrow (`-&gt;`, `&lt;-`) and entity (`&amp;copy;`) rules can
/// straddle a `Text`/`CharRef` boundary. `root` is the whole-content source
/// span; every leaf's precise `location` is sliced from it.
fn apply_character_replacements<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
) -> Vec<InlineNode<'src>> {
    let mut nodes = nodes;

    for repl in character_replacements() {
        nodes = apply_one_replacement(repl, nodes, root);
    }

    nodes
}

/// Applies one [`CharacterReplacement`] rule to `nodes`, first descending into
/// the [`Styled`]/[`Ref`](InlineNode::Ref) children earlier steps created (a
/// replacement inside a span is recognized just as the string pipeline
/// recognizes one inside a rendered tag), then matching and replacing at this
/// level.
fn apply_one_replacement<'src>(
    repl: &CharacterReplacement,
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
) -> Vec<InlineNode<'src>> {
    let nodes: Vec<InlineNode<'src>> = nodes
        .into_iter()
        .map(|node| match node {
            InlineNode::Styled(mut styled) => {
                styled.children = apply_one_replacement(repl, styled.children, root);
                InlineNode::Styled(styled)
            }

            InlineNode::Ref(mut reference) => {
                reference.children = apply_one_replacement(repl, reference.children, root);
                InlineNode::Ref(reference)
            }

            other => other,
        })
        .collect();

    replace_level(repl, nodes, root)
}

/// One character-replacement match at a level, in absolute match-string byte
/// offsets.
struct ReplacementMatch {
    /// The whole match, `[start, end)`.
    full: std::ops::Range<usize>,

    /// What to emit in place of `full`.
    kind: ReplacementKind,
}

enum ReplacementKind {
    /// An escaped construct (`\(C)`, `\-&gt;`, …): drop the single backslash at
    /// this offset and keep the rest of the match as literal nodes, replacing
    /// nothing – mirroring the string replacer's `caps[0].replace("\\", "")`.
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
fn replace_level<'src>(
    repl: &CharacterReplacement,
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes);

    // Cheap pre-filter: skip the pattern sweep when nothing replaceable is
    // present at this level.
    if !maybe_has_replacements(&s) {
        return nodes;
    }

    let matches = find_replacement_matches(repl, &s);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_replacements(&nodes, &pieces, &s, &matches, root)
}

/// Finds every non-overlapping match of `repl` in the escaped match string `s`,
/// left to right, exactly as the string pipeline's `replace_all` does.
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
                // The gap runs up to `consumed`, absorbing any kept leading word
                // character; the cursor stops at `consumed.end`, so a kept
                // trailing letter (the second letter of a `w'w` apostrophe) is
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

// ─── Macros ───────────────────────────────────────────────────────────────

/// The macros substitution, as a node transducer.
///
/// This increment recognizes **image and icon macros** only
/// (`image:target[…]`, `icon:target[…]`), replacing each with an
/// [`Image`](InlineNode::Image) node that carries its own owned
/// [`Attrlist`] – the step that makes a macro node *self-describing*,
/// so the fold reconstructs the render parameters and calls the same
/// `render_image`/`render_icon` the string step calls. The remaining macro
/// families (links, cross-references, footnotes, UI macros, index terms,
/// anchors, STEM) are later increments.
///
/// Like the other steps it descends into the
/// [`Styled`]/[`Ref`](InlineNode::Ref) children earlier steps created – a macro
/// can appear inside a rendered span (`*image:x[]*`), just as the string
/// pipeline matches one inside a rendered `<strong>` tag – then matches at each
/// level.
///
/// # Scope: verbatim macros only
///
/// A recognized macro is built into an `'src`-borrowing node only when its
/// whole match is **verbatim source** – no special character (`< > &`, an
/// atomic [`CharRef`](InlineNode::CharRef)) and no rendered [`Styled`] span
/// falls inside it. The string pipeline matches macros over *escaped,
/// already-rendered* text, so a macro containing (say) a `&` sees `&amp;` in
/// its target/attrlist, and a self-describing node cannot carry that escaped
/// text as an `'src` slice. Such a macro is therefore **left unrecognized**
/// here for a later increment (the attribute-references step and the cutover),
/// mirroring how the quotes step documents its own cross-span boundary (crossed
/// delimiters). The differential corpus pins the verbatim cases this increment
/// claims.
fn apply_macros<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    // Recurse into spans/refs first, matching the string pipeline's
    // whole-string pass.
    let nodes: Vec<InlineNode<'src>> = nodes
        .into_iter()
        .map(|node| match node {
            InlineNode::Styled(mut styled) => {
                styled.children = apply_macros(styled.children, root, parser);
                InlineNode::Styled(styled)
            }

            InlineNode::Ref(mut reference) => {
                reference.children = apply_macros(reference.children, root, parser);
                InlineNode::Ref(reference)
            }

            other => other,
        })
        .collect();

    image_macros_level(nodes, root, parser)
}

/// One image/icon match at a level, in absolute match-string byte offsets.
struct ImageMatch<'src> {
    /// The whole match, `[start, end)`.
    full: std::ops::Range<usize>,

    /// What to emit in place of `full`.
    kind: ImageMatchKind<'src>,
}

enum ImageMatchKind<'src> {
    /// An escaped macro (`\image:…`): drop the leading backslash and keep the
    /// rest as literal nodes, replacing nothing – mirroring the string
    /// replacer's `caps[0][1..]`.
    Unescape,

    /// A recognized macro, built into an [`Image`](InlineNode::Image) node.
    /// Boxed to keep this enum small – the [`Image`](InlineNode::Image) node is
    /// far larger than the empty [`Unescape`](Self::Unescape) variant.
    Node(Box<InlineNode<'src>>),
}

/// Matches `INLINE_IMAGE_MACRO` at this level's escaped text, replacing each
/// verbatim match with the [`Image`](InlineNode::Image) node it produces and
/// leaving everything else in place.
fn image_macros_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes);

    // Cheap pre-filter mirroring the string step's `found_macroish`: an image
    // or icon macro needs its name prefix and an opening bracket.
    if !((s.contains("image:") || s.contains("icon:")) && s.contains('[')) {
        return nodes;
    }

    let matches = find_image_matches(&s, &pieces, root, parser);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_image_level(&nodes, &pieces, &s, matches)
}

/// Finds every image/icon macro at this level, skipping any whose match is not
/// wholly verbatim source (see [`apply_macros`]).
fn find_image_matches<'src>(
    s: &str,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Vec<ImageMatch<'src>> {
    let mut matches = Vec::new();

    for caps in INLINE_IMAGE_MACRO.captures_iter(s) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();

        let full = whole.start()..whole.end();

        // Only a wholly-verbatim match can slice its target/attrlist from
        // `'src`; a match crossing an escaped special or a rendered span is left
        // for a later increment.
        if !range_is_verbatim(pieces, &full) {
            continue;
        }

        if whole.as_str().starts_with('\\') {
            matches.push(ImageMatch {
                full,
                kind: ImageMatchKind::Unescape,
            });

            continue;
        }

        let node = build_image_node(&caps, &full, pieces, root, parser);

        matches.push(ImageMatch {
            full,
            kind: ImageMatchKind::Node(Box::new(node)),
        });
    }

    matches
}

/// Reports whether every piece overlapping the match-string range `range` is a
/// verbatim [`Text`](InlineNode::Text) run (non-atomic). Only then does the
/// range map one-to-one onto contiguous source, so its captures can slice
/// `'src` directly.
fn range_is_verbatim(pieces: &[Piece], range: &std::ops::Range<usize>) -> bool {
    for piece in pieces {
        let p_start = piece.s_start;
        let p_end = piece.s_start + piece.s_len;

        // Skip pieces that do not overlap the range.
        if p_end <= range.start || p_start >= range.end {
            continue;
        }

        if piece.atomic {
            return false;
        }
    }

    true
}

/// Builds one [`Image`](InlineNode::Image) node from a verbatim image/icon
/// match: it slices the target and attribute list straight from `'src` and
/// pre-extracts the alt/width/height the way the string replacer does, so the
/// fold reproduces the same bytes.
fn build_image_node<'src>(
    caps: &regex::Captures<'_>,
    full: &std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> InlineNode<'src> {
    let location = source_slice(pieces, full.clone(), root);

    // The macro name (past any – here absent – escape) is `image:` or `icon:`.
    let is_icon = !location.data().starts_with("image:");

    // Group 1 is the (optional) target; group 2 is the bracket text, which
    // always participates (it may be empty).
    let target = caps
        .get(1)
        .map(|m| source_slice(pieces, m.start()..m.end(), root))
        .map_or_else(|| CowStr::from(""), |sp| CowStr::from(sp.data()));

    let bracket = caps.get(2).map_or_else(
        || location.slice(0..0),
        |m| source_slice(pieces, m.start()..m.end(), root),
    );

    let attrlist = Attrlist::parse(bracket, parser, AttrlistContext::Inline)
        .item
        .item;

    // The default alt text derives from the target's basename, with `_`/`-`
    // read as spaces – exactly the string replacer's `default_alt`.
    let default_alt = basename(&target.replace(['_', '-'], " "));

    // Pre-extract the resolved alt/width/height into owned values, ending the
    // `&'src self`-tied borrows before the attribute list is moved into the node
    // (the same read-then-move shape [`attributes_of`] uses). An icon carries a
    // `size` rather than width/height, recomputed at fold time from `attrs`.
    let (alt, width, height) = if is_icon {
        let alt = attrlist.named_attribute("alt").map_or(default_alt, |a| {
            normalize_text_lf_escaped_bracket(a.value())
        });

        (Some(CowStr::from(alt)), None, None)
    } else {
        let alt = attrlist
            .named_or_positional_attribute("alt", 1)
            .map_or(default_alt, |a| {
                normalize_text_lf_escaped_bracket(a.value())
            });

        let width = attrlist
            .named_or_positional_attribute("width", 2)
            .map(|a| CowStr::from(a.value().to_string()));

        let height = attrlist
            .named_or_positional_attribute("height", 3)
            .map(|a| CowStr::from(a.value().to_string()));

        (Some(CowStr::from(alt)), width, height)
    };

    InlineNode::Image(Image {
        is_icon,
        target,
        alt,
        width,
        height,
        attrs: Some(attrlist),
        location,
    })
}

/// Rebuilds a level's node list from its image/icon matches: each gap keeps its
/// original nodes; each match becomes either its literal text (an escape, with
/// the leading backslash dropped) or the built [`Image`](InlineNode::Image)
/// node.
fn rebuild_image_level<'src>(
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    s: &str,
    matches: Vec<ImageMatch<'src>>,
) -> Vec<InlineNode<'src>> {
    let mut out = Vec::new();
    let mut cursor = 0usize;

    for m in matches {
        match m.kind {
            ImageMatchKind::Unescape => {
                // Keep the whole match with the single leading backslash dropped.
                emit_range(nodes, pieces, cursor..m.full.start, &mut out);
                emit_range(nodes, pieces, (m.full.start + 1)..m.full.end, &mut out);
            }

            ImageMatchKind::Node(node) => {
                emit_range(nodes, pieces, cursor..m.full.start, &mut out);
                out.push(*node);
            }
        }

        cursor = m.full.end;
    }

    if cursor < s.len() {
        emit_range(nodes, pieces, cursor..s.len(), &mut out);
    }

    out
}

// ─── Post replacements (hard line breaks) ─────────────────────────────────

/// The post-replacement substitution, as a node transducer: a line ending in
/// ` +` has that ` +` replaced by a [`LineBreak`](InlineNode::LineBreak) leaf,
/// the line content before it staying in place.
///
/// Only the default hard-line-break form is handled here. The block-wide
/// `hardbreaks` option – which turns *every* line ending into a break – needs
/// the block's attribute list and the document's `hardbreaks-option` attribute,
/// neither yet threaded into the builder, so it is deferred to the cutover
/// (design §5.2 Phase 4, step 6).
fn apply_post_replacements<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
) -> Vec<InlineNode<'src>> {
    // Descend into spans/refs first, matching the string pipeline's
    // whole-string pass.
    let nodes: Vec<InlineNode<'src>> = nodes
        .into_iter()
        .map(|node| match node {
            InlineNode::Styled(mut styled) => {
                styled.children = apply_post_replacements(styled.children, root);
                InlineNode::Styled(styled)
            }

            InlineNode::Ref(mut reference) => {
                reference.children = apply_post_replacements(reference.children, root);
                InlineNode::Ref(reference)
            }

            other => other,
        })
        .collect();

    let (s, pieces) = build_match_string(&nodes);

    // The string pipeline guards on both a `+` and a newline being present.
    if !(s.contains('+') && s.contains('\n')) {
        return nodes;
    }

    // Each match's `[content.end, whole.end)` is the trailing ` +` the break
    // replaces; the line content before it is kept. Group 0 (the whole match)
    // and group 1 (the `(.*)` line content) always participate in this pattern.
    let breaks: Vec<std::ops::Range<usize>> = hard_line_break_pattern()
        .captures_iter(&s)
        .map(|caps| {
            #[allow(clippy::unwrap_used)]
            let whole = caps.get(0).unwrap();

            #[allow(clippy::unwrap_used)]
            let content = caps.get(1).unwrap();

            content.end()..whole.end()
        })
        .collect();

    if breaks.is_empty() {
        return nodes;
    }

    let mut out = Vec::new();
    let mut cursor = 0usize;

    for br in breaks {
        emit_range(&nodes, &pieces, cursor..br.start, &mut out);

        out.push(InlineNode::LineBreak {
            location: source_slice(&pieces, br.clone(), root),
        });

        cursor = br.end;
    }

    if cursor < s.len() {
        emit_range(&nodes, &pieces, cursor..s.len(), &mut out);
    }

    out
}

/// The inverse of the classifier's value assignment: the
/// [`CharacterReplacementType`] the fold renders a [`CharRef::Replacement`]
/// value with. The mapping is a bijection – each replacement type produces a
/// distinct logical string – so the fold reproduces the string pipeline's bytes
/// through the same [`render_character_replacement`] the step calls.
///
/// [`render_character_replacement`]:
///     crate::parser::InlineSubstitutionRenderer::render_character_replacement
fn replacement_type_of(value: &str) -> Option<CharacterReplacementType> {
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

/// Folds an inline node tree to output bytes through `renderer`.
///
/// This is the fold over the *public* [`InlineNode`] tree. It handles the node
/// kinds the transducer steps produce so far – [`Text`](InlineNode::Text),
/// [`CharRef`](InlineNode::CharRef), [`Styled`], [`Image`](InlineNode::Image),
/// and [`LineBreak`](InlineNode::LineBreak), plus the design-legal
/// [`Raw`](InlineNode::Raw) leaf; a later increment extends it as the
/// transducer grows new kinds.
pub(crate) fn fold_html(
    nodes: &[InlineNode<'_>],
    renderer: &dyn InlineSubstitutionRenderer,
    parser: &Parser,
) -> String {
    let mut out = String::new();
    fold_into_html(nodes, renderer, parser, &mut out);
    out
}

/// Appends the fold of `nodes` to `out` (the recursive worker for
/// [`fold_html`]).
///
/// `parser` is threaded through because rendering some nodes – an
/// [`Image`](InlineNode::Image), whose `render_image`/`render_icon` reads the
/// document's safe mode, `data-uri`, and `icons`/`icontype` attributes – is a
/// function of document context, not of the node alone.
fn fold_into_html(
    nodes: &[InlineNode<'_>],
    renderer: &dyn InlineSubstitutionRenderer,
    parser: &Parser,
    out: &mut String,
) {
    for node in nodes {
        match node {
            InlineNode::Text { value, .. } => {
                // `Text` is logical (un-escaped) text; the fold escapes it. The
                // builder never leaves a special inside a `Text`, so this is
                // belt-and-suspenders, but it keeps the fold correct in its own
                // right.
                for ch in value.chars() {
                    render_char(ch, renderer, out);
                }
            }

            InlineNode::Raw { value, .. } => {
                out.push_str(value);
            }

            InlineNode::CharRef {
                value: CharRef::Special(ch),
                ..
            } => {
                render_char(*ch, renderer, out);
            }

            InlineNode::CharRef {
                value: CharRef::Replacement(value),
                ..
            } => match replacement_type_of(value) {
                // A replacement the builder produced routes through the renderer,
                // so a custom backend can override the entity it emits.
                Some(type_) => renderer.render_character_replacement(type_, out),

                // A value the builder never produces (only a hand-built node can
                // reach here) still folds losslessly: the general rule every
                // known replacement follows is one decimal numeric entity per
                // logical character.
                None => {
                    for ch in value.chars() {
                        out.push_str(&format!("&#{};", ch as u32));
                    }
                }
            },

            InlineNode::CharRef {
                value: CharRef::Entity(entity),
                ..
            } => {
                // `Entity` carries the character reference exactly as written
                // (`&copy;`); it is already a valid entity, emitted verbatim.
                out.push_str(entity);
            }

            InlineNode::LineBreak { .. } => {
                renderer.render_line_break(out);
            }

            InlineNode::Image(image) => {
                fold_image(image, renderer, parser, out);
            }

            InlineNode::Styled(styled) => {
                // Fold the children to the body, then wrap it exactly as the
                // string pipeline's quotes step did: the same `QuoteType`,
                // attribute list, and id it recognized, so the bytes match.
                let mut body = String::new();
                fold_into_html(&styled.children, renderer, parser, &mut body);

                let scope = match styled.form {
                    SpanForm::Constrained => QuoteScope::Constrained,
                    SpanForm::Unconstrained => QuoteScope::Unconstrained,
                };

                renderer.render_quoted_substitution(
                    quote_type_of(styled.variant),
                    scope,
                    styled.attrs.clone(),
                    styled.id.as_ref().map(|id| id.to_string()),
                    &body,
                    out,
                );
            }

            other => {
                // The steps wired up so far produce only `Text`,
                // `CharRef::Special`, `Styled`, `Image`, and `LineBreak` nodes,
                // and this fold additionally emits the design-legal `Raw` leaf;
                // no other node kind reaches the fold in this increment. A later
                // increment fills in the arms above as the transducer grows new
                // kinds.
                // Guard against a premature caller in debug builds and emit
                // nothing in release, mirroring the safe defensive fallback in
                // [`content`](super::content).
                debug_assert!(
                    false,
                    "inline_builder::fold_html reached an unsupported node kind: {other:?}"
                );
            }
        }
    }
}

/// Appends `ch` to `out`, routing the three special characters through
/// `renderer` (so a custom renderer's escaping is honored) and pushing any
/// other character verbatim.
fn render_char(ch: char, renderer: &dyn InlineSubstitutionRenderer, out: &mut String) {
    let type_ = match ch {
        '<' => SpecialCharacter::Lt,
        '>' => SpecialCharacter::Gt,
        '&' => SpecialCharacter::Ampersand,

        _ => {
            out.push(ch);
            return;
        }
    };

    renderer.render_special_character(type_, out);
}

/// Folds an [`Image`](InlineNode::Image) node, reconstructing the
/// [`ImageRenderParams`]/[`IconRenderParams`] the string pipeline built and
/// calling the same `render_image`/`render_icon`, so the output is
/// byte-for-byte identical. The pre-extracted `alt` (and, for an image,
/// `width`/`height`) come straight off the node; an icon's `size` and every
/// other rendered attribute (`title`, `link`, `format`, roles, …) are read back
/// from the node's own [`Attrlist`] – which is why the macro step
/// captured it.
fn fold_image(
    image: &Image<'_>,
    renderer: &dyn InlineSubstitutionRenderer,
    parser: &Parser,
    out: &mut String,
) {
    // A macro-built image always carries its attribute list; a node hand-built
    // without one folds through an empty list, sliced (empty) from the node's
    // own `'src` location so its lifetime matches.
    let empty;
    let attrlist = match &image.attrs {
        Some(attrlist) => attrlist,

        None => {
            empty = Attrlist::parse(image.location.slice(0..0), parser, AttrlistContext::Inline)
                .item
                .item;

            &empty
        }
    };

    let alt = image
        .alt
        .as_ref()
        .map(|a| a.to_string())
        .unwrap_or_default();

    if image.is_icon {
        let params = IconRenderParams {
            target: image.target.as_ref(),
            alt,
            size: attrlist
                .named_or_positional_attribute("size", 1)
                .map(|a| a.value()),
            attrlist,
            parser,
        };

        renderer.render_icon(&params, out);
    } else {
        let params = ImageRenderParams {
            target: image.target.as_ref(),
            alt,
            width: image.width.as_deref(),
            height: image.height.as_deref(),
            attrlist,
            parser,
        };

        renderer.render_image(&params, out);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use super::{
        apply_character_replacements, apply_macros, apply_post_replacements, apply_quotes,
        apply_special_characters, build,
    };
    use crate::{
        HasSpan, Parser, Span,
        content::{Content, SubstitutionStep},
        inlines::{CharRef, Image, InlineNode, Ref, RefVariant, SpanForm, StyleVariant, Styled},
        parser::HtmlSubstitutionRenderer,
        strings::CowStr,
    };

    /// Folds the tree with the built-in HTML renderer and a default parser. The
    /// parser is consulted only when the tree contains an
    /// [`Image`](InlineNode::Image) node (for the document's safe mode,
    /// `data-uri`, and `icons` attributes); tests that need a non-default
    /// document call [`super::fold_html`] directly with their own parser.
    fn fold_html(nodes: &[InlineNode<'_>], renderer: &HtmlSubstitutionRenderer) -> String {
        super::fold_html(nodes, renderer, &Parser::default())
    }

    /// Builds the single-pass tree for `source` with a default parser (the
    /// parser is consulted only for attributed-quote attribute lists).
    fn build_src(source: Span<'_>) -> Vec<InlineNode<'_>> {
        build(source, &Parser::default())
    }

    /// Seeds the whole-source [`Text`](InlineNode::Text) node the transducer
    /// steps refine, exactly as [`build`] does before running them.
    fn seed(source: Span<'_>) -> Vec<InlineNode<'_>> {
        vec![InlineNode::Text {
            value: CowStr::from(source.data()),
            location: source,
        }]
    }

    /// Builds the tree **through the special-characters step only**, so a
    /// staged differential test compares against the matching partial golden
    /// (the full [`build`] runs later steps that would perturb it).
    fn build_through_special(source: Span<'_>) -> Vec<InlineNode<'_>> {
        apply_special_characters(seed(source))
    }

    /// Builds the tree **through the quotes step**, for the quotes-stage
    /// differential (see [`build_through_special`]).
    fn build_through_quotes(source: Span<'_>) -> Vec<InlineNode<'_>> {
        apply_quotes(build_through_special(source), source, &Parser::default())
    }

    /// Asserts that `node` is a [`Text`](InlineNode::Text) whose `value`
    /// borrows (does not allocate) and whose `location` selects `data` at
    /// `line`/`col`.
    fn assert_text(node: &InlineNode<'_>, data: &str, line: usize, col: usize) {
        match node {
            InlineNode::Text { value, location } => {
                assert!(
                    matches!(value, CowStr::Borrowed(_)),
                    "text value should borrow from source, got {value:?}"
                );
                assert_eq!(value.as_ref(), data);
                assert_eq!(location.data(), data);
                assert_eq!(location.line(), line, "line for {data:?}");
                assert_eq!(location.col(), col, "col for {data:?}");
            }

            other => panic!("expected Text({data:?}), got {other:?}"),
        }
    }

    /// Asserts that `node` is a special [`CharRef`](InlineNode::CharRef) for
    /// `ch`, located at `col` on line 1 with `offset`.
    fn assert_special(node: &InlineNode<'_>, ch: char, col: usize, offset: usize) {
        match node {
            InlineNode::CharRef {
                value: CharRef::Special(got),
                location,
            } => {
                assert_eq!(*got, ch);
                assert_eq!(location.data(), ch.to_string());
                assert_eq!(location.col(), col, "col for {ch:?}");
                assert_eq!(location.byte_offset(), offset, "offset for {ch:?}");
            }

            other => panic!("expected CharRef::Special({ch:?}), got {other:?}"),
        }
    }

    #[test]
    fn splits_text_and_specials_with_precise_spans() {
        let nodes = build_src(Span::new("a<b>c&d"));

        assert_eq!(nodes.len(), 7);
        assert_text(&nodes[0], "a", 1, 1);
        assert_special(&nodes[1], '<', 2, 1);
        assert_text(&nodes[2], "b", 1, 3);
        assert_special(&nodes[3], '>', 4, 3);
        assert_text(&nodes[4], "c", 1, 5);
        assert_special(&nodes[5], '&', 6, 5);
        assert_text(&nodes[6], "d", 1, 7);
    }

    #[test]
    fn all_specials_yield_only_char_refs() {
        let nodes = build_src(Span::new("<>&"));

        assert_eq!(nodes.len(), 3);
        assert_special(&nodes[0], '<', 1, 0);
        assert_special(&nodes[1], '>', 2, 1);
        assert_special(&nodes[2], '&', 3, 2);
    }

    #[test]
    fn adjacent_specials_produce_no_empty_runs() {
        let nodes = build_src(Span::new("<<"));

        assert_eq!(nodes.len(), 2);
        assert_special(&nodes[0], '<', 1, 0);
        assert_special(&nodes[1], '<', 2, 1);
    }

    #[test]
    fn plain_text_is_a_single_borrowed_node() {
        let nodes = build_src(Span::new("hello"));

        assert_eq!(nodes.len(), 1);
        assert_text(&nodes[0], "hello", 1, 1);
    }

    #[test]
    fn empty_source_yields_no_nodes() {
        assert!(build_src(Span::new("")).is_empty());
    }

    #[test]
    fn a_run_spanning_a_newline_tracks_line_and_col() {
        // The text run between the two specials includes the newline, so the
        // node after it is located on line 2.
        let nodes = build_src(Span::new("a<\nb>"));

        assert_eq!(nodes.len(), 4);
        assert_text(&nodes[0], "a", 1, 1);
        assert_special(&nodes[1], '<', 2, 1);

        // The middle run is "\nb": it starts right after `<` (line 1, col 3) and
        // carries into line 2.
        assert_text(&nodes[2], "\nb", 1, 3);

        // The closing `>` lands on line 2.
        match &nodes[3] {
            InlineNode::CharRef {
                value: CharRef::Special('>'),
                location,
            } => {
                assert_eq!(location.line(), 2);
                assert_eq!(location.col(), 2);
            }

            other => panic!("expected CharRef::Special('>'), got {other:?}"),
        }
    }

    #[test]
    fn special_characters_recurses_into_styled_children() {
        // A custom `subs` order can run quotes before special characters, so the
        // step must descend into a `Styled` span's children.
        let loc = Span::new("a<b");

        let styled = InlineNode::Styled(Styled {
            variant: StyleVariant::Strong,
            form: SpanForm::Constrained,
            id: None,
            roles: vec![],
            attrs: None,
            children: vec![InlineNode::Text {
                value: CowStr::from(loc.data()),
                location: loc,
            }],
            location: loc,
        });

        let out = apply_special_characters(vec![styled]);

        assert_eq!(out.len(), 1);

        match &out[0] {
            InlineNode::Styled(styled) => {
                assert_eq!(styled.children.len(), 3);
                assert_text(&styled.children[0], "a", 1, 1);
                assert_special(&styled.children[1], '<', 2, 1);
                assert_text(&styled.children[2], "b", 1, 3);
            }

            other => panic!("expected Styled, got {other:?}"),
        }
    }

    #[test]
    fn special_characters_recurses_into_ref_children() {
        // A reference's display text is likewise refined in place.
        let loc = Span::new("x&y");

        let reference = InlineNode::Ref(Ref {
            variant: RefVariant::Link,
            target: CowStr::from("https://example.com"),
            children: vec![InlineNode::Text {
                value: CowStr::from(loc.data()),
                location: loc,
            }],
            roles: vec![],
            window: None,
            resolved: None,
            location: loc,
        });

        let out = apply_special_characters(vec![reference]);

        assert_eq!(out.len(), 1);

        match &out[0] {
            InlineNode::Ref(reference) => {
                assert_eq!(reference.children.len(), 3);
                assert_text(&reference.children[0], "x", 1, 1);
                assert_special(&reference.children[1], '&', 2, 1);
                assert_text(&reference.children[2], "y", 1, 3);
            }

            other => panic!("expected Ref, got {other:?}"),
        }
    }

    #[test]
    fn special_characters_passes_other_nodes_through() {
        // A node kind the step does not split (here a line break) is forwarded
        // unchanged.
        let location = Span::new("");
        let out = apply_special_characters(vec![InlineNode::LineBreak { location }]);

        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], InlineNode::LineBreak { .. }));
    }

    #[test]
    fn special_characters_preserves_a_synthesized_text_value() {
        // A synthesized `value` (standing in for an attribute expansion) does
        // not coincide with the source its `location` covers. The step must
        // split the *logical value* – not re-derive text from the span – so the
        // expansion survives, with the whole `location` kept as each sub-node's
        // coarse fallback span.
        let location = Span::new("{x}");

        let text = InlineNode::Text {
            value: CowStr::from("a<b".to_string()),
            location,
        };

        let out = apply_special_characters(vec![text]);

        assert_eq!(out.len(), 3);

        // Leading run: the value's text, not the span's, and the coarse span.
        match &out[0] {
            InlineNode::Text { value, location } => {
                assert_eq!(value.as_ref(), "a");
                assert_eq!(location.data(), "{x}");
            }

            other => panic!("expected Text, got {other:?}"),
        }

        match &out[1] {
            InlineNode::CharRef {
                value: CharRef::Special(ch),
                location,
            } => {
                assert_eq!(*ch, '<');
                assert_eq!(location.data(), "{x}");
            }

            other => panic!("expected CharRef::Special, got {other:?}"),
        }

        // Trailing run, exercising the loop's post-special tail.
        match &out[2] {
            InlineNode::Text { value, location } => {
                assert_eq!(value.as_ref(), "b");
                assert_eq!(location.data(), "{x}");
            }

            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn fold_emits_raw_verbatim() {
        // A `Raw` leaf is emitted without HTML-escaping, unlike `Text`; its `<`,
        // `>`, and `&` pass straight through.
        let location = Span::new("<b>raw &amp;</b>");

        let raw = InlineNode::Raw {
            value: CowStr::from(location.data()),
            location,
        };

        assert_eq!(
            fold_html(&[raw], &HtmlSubstitutionRenderer {}),
            "<b>raw &amp;</b>"
        );
    }

    /// The string pipeline's special-characters output for `source`, used as
    /// the golden oracle: `Content::from` then the `SpecialCharacters`
    /// step.
    fn golden(source: &str) -> String {
        let parser = crate::Parser::default();
        let mut content = Content::from(Span::new(source));
        SubstitutionStep::SpecialCharacters.apply(&mut content, &parser, None);
        content.rendered_str().to_string()
    }

    #[test]
    fn fold_matches_the_string_pipeline_byte_for_byte() {
        // Special-characters-only fixtures: for these, folding the single-pass
        // tree reproduces the string pipeline's escaped output exactly.
        let fixtures = [
            "",
            "plain text",
            "a<b>c&d",
            "<>&",
            "<<",
            "&<>&",
            "less < and & more >",
            "trailing &",
            "multi\nline < with & specials >",
            "unicode π < ω &",
        ];

        let renderer = HtmlSubstitutionRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_through_special(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden(fixture),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

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
        use super::{Piece, s_to_src};

        // In practice every boundary `s_to_src` maps falls on a literal
        // delimiter (a text position), so the atomic-snap, before-first, and
        // past-last branches are defensive. Exercise them directly to document
        // the intended fallback.
        let atomic = Piece {
            node_index: 0,
            s_start: 0,
            s_len: 4,
            src_offset: 10,
            src_len: 1,
            atomic: true,
        };

        // A boundary inside an atomic piece snaps to the nearer edge.
        assert_eq!(s_to_src(std::slice::from_ref(&atomic), 1), 10);
        assert_eq!(s_to_src(std::slice::from_ref(&atomic), 3), 11);

        // A boundary past the last piece falls back to the source end.
        assert_eq!(s_to_src(std::slice::from_ref(&atomic), 9), 11);

        // A boundary before the first piece begins breaks out to the same
        // fallback.
        let offset = Piece {
            s_start: 2,
            ..atomic
        };

        assert_eq!(s_to_src(std::slice::from_ref(&offset), 0), 11);

        // No pieces (an empty level) anchors at the source start.
        assert_eq!(s_to_src(&[], 0), 0);
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
        };

        let mut out = Vec::new();
        emit_range(&[], std::slice::from_ref(&piece), 0..1, &mut out);
        assert!(out.is_empty());
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

    /// Asserts that `node` is a [`Styled`] span of `variant`/`form` with
    /// `children` children, and returns those children for further inspection.
    fn assert_styled<'a, 'src>(
        node: &'a InlineNode<'src>,
        variant: StyleVariant,
        form: SpanForm,
    ) -> &'a [InlineNode<'src>] {
        match node {
            InlineNode::Styled(styled) => {
                assert_eq!(styled.variant, variant, "variant");
                assert_eq!(styled.form, form, "form");
                &styled.children
            }

            other => panic!("expected Styled({variant:?}), got {other:?}"),
        }
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

    /// The string pipeline's output through the **post-replacement** step for
    /// `source`, used as the golden oracle: the four steps [`build`] runs, in
    /// order (special characters, quotes, character replacements, post
    /// replacement). Attribute references and macros are skipped – exactly as
    /// the additive builder skips them – so the fixtures deliberately contain
    /// neither.
    fn golden_replacements(source: &str) -> String {
        let parser = crate::Parser::default();
        let mut content = Content::from(Span::new(source));
        SubstitutionStep::SpecialCharacters.apply(&mut content, &parser, None);
        SubstitutionStep::Quotes.apply(&mut content, &parser, None);
        SubstitutionStep::CharacterReplacements.apply(&mut content, &parser, None);
        SubstitutionStep::PostReplacement.apply(&mut content, &parser, None);
        content.rendered_str().to_string()
    }

    #[test]
    fn fold_matches_the_string_pipeline_through_replacements() {
        // For each fixture, folding the single-pass tree (special characters +
        // quotes + character replacements + post replacement) reproduces the
        // string pipeline's output byte-for-byte. This is the differential
        // corpus (design §5.3) that pins this increment.
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

        let renderer = HtmlSubstitutionRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_src(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden_replacements(fixture),
                "fold diverged from the string pipeline for {fixture:?}"
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
        // the numeric entity the string pipeline emits.
        assert_replacement(&nodes[0], "\u{a9}", "(C)");

        assert_eq!(fold_html(&nodes, &HtmlSubstitutionRenderer {}), "&#169;");
    }

    #[test]
    fn an_arrow_replacement_spans_a_text_and_charref_boundary() {
        // `->` is `-` (text) followed by `&gt;` (a `CharRef::Special` from the
        // special-characters step), so the arrow rule must match across the two.
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

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            "&copy; 2026"
        );
    }

    #[test]
    fn an_escaped_replacement_stays_literal() {
        // `\(C)` drops the backslash and keeps `(C)` as literal text – no
        // replacement leaf.
        let nodes = build_src(Span::new("\\(C)"));

        assert!(
            nodes
                .iter()
                .all(|n| !matches!(n, InlineNode::CharRef { .. })),
            "an escaped replacement must not produce a char-ref leaf: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_replacements("\\(C)")
        );
    }

    #[test]
    fn a_replacement_inside_a_span_is_recognized() {
        // A `(C)` inside a strong span is replaced just as the string pipeline
        // replaces one inside a rendered `<strong>` tag.
        let nodes = build_src(Span::new("*Acme (C)*"));

        let children = assert_styled(&nodes[0], StyleVariant::Strong, SpanForm::Constrained);
        assert_eq!(children.len(), 2);
        assert_text(&children[0], "Acme ", 1, 2);
        assert_replacement(&children[1], "\u{a9}", "(C)");
    }

    #[test]
    fn a_hard_line_break_becomes_a_line_break_leaf() {
        // A line ending in ` +` yields a `LineBreak` leaf in place of the ` +`;
        // the newline and following line stay as text.
        let nodes = build_src(Span::new("foo +\nbar"));

        assert_eq!(nodes.len(), 3);
        assert_text(&nodes[0], "foo", 1, 1);

        match &nodes[1] {
            InlineNode::LineBreak { location } => {
                assert_eq!(location.data(), " +");
            }

            other => panic!("expected LineBreak, got {other:?}"),
        }

        assert_text(&nodes[2], "\nbar", 1, 6);

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            "foo<br>\nbar"
        );
    }

    #[test]
    fn character_replacements_recurse_into_ref_children() {
        // A reference's display text is subject to replacements, just as the
        // string pipeline processes it inside the rendered anchor. (No macros
        // step yet builds `Ref` nodes, so this drives the recursion directly.)
        let loc = Span::new("(C)");

        let reference = InlineNode::Ref(Ref {
            variant: RefVariant::Link,
            target: CowStr::from("https://example.com"),
            children: vec![InlineNode::Text {
                value: CowStr::from(loc.data()),
                location: loc,
            }],
            roles: vec![],
            window: None,
            resolved: None,
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
    fn post_replacements_recurse_into_ref_children() {
        // A hard line break inside a reference's display text is likewise
        // recognized (driven directly, as above).
        let loc = Span::new("x +\ny");

        let reference = InlineNode::Ref(Ref {
            variant: RefVariant::Link,
            target: CowStr::from("https://example.com"),
            children: vec![InlineNode::Text {
                value: CowStr::from(loc.data()),
                location: loc,
            }],
            roles: vec![],
            window: None,
            resolved: None,
            location: loc,
        });

        let out = apply_post_replacements(vec![reference], loc);

        match &out[0] {
            InlineNode::Ref(reference) => {
                assert_eq!(reference.children.len(), 3);
                assert_text(&reference.children[0], "x", 1, 1);
                assert!(matches!(
                    reference.children[1],
                    InlineNode::LineBreak { .. }
                ));
                assert_text(&reference.children[2], "\ny", 1, 4);
            }

            other => panic!("expected Ref, got {other:?}"),
        }
    }

    #[test]
    fn fold_encodes_an_unknown_replacement_value_per_character() {
        // A `CharRef::Replacement` value the builder never produces (only a
        // hand-built node can carry one) still folds losslessly: one decimal
        // numeric entity per logical character. Here `z` is U+007A.
        let location = Span::new("z");

        let node = InlineNode::CharRef {
            value: CharRef::Replacement("z"),
            location,
        };

        assert_eq!(fold_html(&[node], &HtmlSubstitutionRenderer {}), "&#122;");
    }

    #[test]
    fn replacement_type_of_round_trips_every_variant() {
        use super::replacement_type_of;

        // Every replacement value the classifier assigns maps back to a type, so
        // the fold always routes through the renderer. An unknown value is the
        // defensive `None`.
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

    // ─── Macros (image / icon) ────────────────────────────────────────────

    /// The string pipeline's output through the **macros** step for `source`,
    /// used as the golden oracle: the five steps [`build`] runs, in order
    /// (special characters, quotes, character replacements, macros, post
    /// replacement), with `parser` as the document context. Attribute
    /// references are skipped – exactly as the additive builder skips them – so
    /// the fixtures deliberately contain none.
    fn golden_macros_with(source: &str, parser: &Parser) -> String {
        let mut content = Content::from(Span::new(source));
        SubstitutionStep::SpecialCharacters.apply(&mut content, parser, None);
        SubstitutionStep::Quotes.apply(&mut content, parser, None);
        SubstitutionStep::CharacterReplacements.apply(&mut content, parser, None);
        SubstitutionStep::Macros.apply(&mut content, parser, None);
        SubstitutionStep::PostReplacement.apply(&mut content, parser, None);
        content.rendered_str().to_string()
    }

    /// [`golden_macros_with`] with a default parser.
    fn golden_macros(source: &str) -> String {
        golden_macros_with(source, &Parser::default())
    }

    #[test]
    fn fold_matches_the_string_pipeline_through_macros() {
        // For each fixture, folding the single-pass tree (all five steps)
        // reproduces the string pipeline's output byte-for-byte. This is the
        // differential corpus (design §5.3) that pins the image/icon increment.
        let fixtures = [
            // No macro despite macro-ish characters.
            "plain text",
            "a colon : and a bracket [ apart",
            "image without a bracket image:foo.png stays literal",
            // Images: empty, alt, defaulted alt, dimensions, named attrs.
            "image:sunset.jpg[]",
            "image:sunset.jpg[Sunset]",
            "image:sunset.jpg[Sunset Mountain]",
            "image:photo.png[Alt Text,200,100]",
            "image:photo.png[alt=Alt Text,width=200,height=100]",
            "image:a_b-c.png[]",
            "image:d/e/f.png[]",
            "image:logo.png[Logo,role=thumb]",
            "image:logo.png[title=Hover text]",
            "image:logo.png[link=https://example.org]",
            "image:logo.png[float=left]",
            // Icons.
            "icon:tags[]",
            "icon:home[Home]",
            "icon:home[size=2x]",
            // A macro embedded in surrounding flow, and next to other constructs.
            "See image:sunset.jpg[Sunset] here.",
            "*bold* then image:x.png[X] and _em_",
            "before image:a.png[A] middle image:b.png[B] after",
            "a copyright (C) then image:x.png[X]",
            // Escapes: the macro stays literal, minus the backslash.
            "\\image:sunset.jpg[]",
            "\\image:sunset.jpg[Sunset]",
            "\\icon:home[]",
            // A macro inside a rendered span (recognized inside the span body).
            "*see image:x.png[X]*",
            "_image:y.png[Y] in em_",
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
    fn fold_matches_the_string_pipeline_with_document_context() {
        // The image/icon fold reads document attributes (an icon's `icons`
        // mode, an image's `imagesdir`), so the parity must hold under a
        // non-default document too. Build and fold with the *same* parser the
        // golden uses.
        use crate::parser::ModificationContext;

        let parser = Parser::default()
            .with_intrinsic_attribute("imagesdir", "assets/img", ModificationContext::Anywhere)
            .with_intrinsic_attribute("icons", "font", ModificationContext::Anywhere)
            .with_intrinsic_attribute("icontype", "svg", ModificationContext::Anywhere);

        let fixtures = [
            "image:sunset.jpg[Sunset]",
            "image:sub/dir/pic.png[Pic,320]",
            "icon:heart[]",
            "icon:heart[2x]",
            "icon:heart[size=lg,role=fav]",
            "text with icon:star[] inline",
        ];

        let renderer = HtmlSubstitutionRenderer {};

        for fixture in fixtures {
            let folded = super::fold_html(&build(Span::new(fixture), &parser), &renderer, &parser);

            assert_eq!(
                folded,
                golden_macros_with(fixture, &parser),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    /// Asserts that `node` is an [`Image`](InlineNode::Image), returning it for
    /// further inspection.
    fn assert_image<'a, 'src>(node: &'a InlineNode<'src>) -> &'a Image<'src> {
        match node {
            InlineNode::Image(image) => image,

            other => panic!("expected Image, got {other:?}"),
        }
    }

    #[test]
    fn an_image_macro_becomes_a_self_describing_node() {
        let nodes = build_src(Span::new("image:sunset.jpg[Sunset]"));

        assert_eq!(nodes.len(), 1);
        let image = assert_image(&nodes[0]);

        assert!(!image.is_icon);

        // The target borrows from source (no allocation), and the alt is the
        // supplied positional value.
        assert!(matches!(image.target, CowStr::Borrowed(_)));
        assert_eq!(image.target.as_ref(), "sunset.jpg");
        assert_eq!(image.alt.as_deref(), Some("Sunset"));
        assert_eq!(image.width, None);
        assert_eq!(image.height, None);

        // The node captures its own attribute list – the property that makes it
        // self-describing (and unblocks a faithful fold).
        assert!(image.attrs.is_some(), "the attribute list is retained");

        // Its location covers the whole macro, delimiters included.
        assert_eq!(image.location.data(), "image:sunset.jpg[Sunset]");
        assert_eq!(image.location.line(), 1);
        assert_eq!(image.location.col(), 1);
    }

    #[test]
    fn image_dimensions_are_captured_positionally() {
        let nodes = build_src(Span::new("image:p.png[Alt,200,100]"));

        let image = assert_image(&nodes[0]);
        assert_eq!(image.alt.as_deref(), Some("Alt"));
        assert_eq!(image.width.as_deref(), Some("200"));
        assert_eq!(image.height.as_deref(), Some("100"));
    }

    #[test]
    fn image_default_alt_derives_from_the_basename() {
        // With no alt, the default is the target's basename with `_`/`-` read as
        // spaces and the extension dropped.
        let nodes = build_src(Span::new("image:a_b-c.png[]"));

        let image = assert_image(&nodes[0]);
        assert_eq!(image.alt.as_deref(), Some("a b c"));
    }

    #[test]
    fn an_icon_macro_becomes_an_icon_node() {
        let nodes = build_src(Span::new("icon:home[size=2x]"));

        let image = assert_image(&nodes[0]);
        assert!(image.is_icon);
        assert_eq!(image.target.as_ref(), "home");

        // An icon has no positional width/height; its `size` lives in the
        // attribute list (read back at fold time), and its default alt is the
        // target itself.
        assert_eq!(image.width, None);
        assert_eq!(image.height, None);
        assert_eq!(image.alt.as_deref(), Some("home"));
    }

    #[test]
    fn an_image_macro_is_recognized_inside_a_span() {
        // A macro can appear inside a rendered span; the transducer descends
        // into the span body and builds the node there.
        let nodes = build_src(Span::new("*see image:x.png[X]*"));

        let children = assert_styled(&nodes[0], StyleVariant::Strong, SpanForm::Constrained);
        assert_eq!(children.len(), 2);
        assert_text(&children[0], "see ", 1, 2);

        let image = assert_image(&children[1]);
        assert_eq!(image.target.as_ref(), "x.png");
        assert_eq!(image.alt.as_deref(), Some("X"));
    }

    #[test]
    fn an_escaped_image_macro_stays_literal() {
        // `\image:…` drops the backslash and keeps the macro as literal text –
        // no image node.
        let nodes = build_src(Span::new("\\image:sunset.jpg[Sunset]"));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Image(_))),
            "an escaped macro must not produce an image node: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros("\\image:sunset.jpg[Sunset]")
        );
    }

    #[test]
    fn a_macro_over_a_special_character_is_a_documented_divergence() {
        // The string pipeline matches macros over *escaped* text, so a target
        // containing `&` is matched as `a&amp;b.png`. A self-describing node
        // cannot carry that escaped text as an `'src` slice, so the single-pass
        // builder leaves such a macro *unrecognized* for a later increment (the
        // attribute-references step and the cutover). This is the documented
        // boundary of the additive image increment; the differential corpus
        // above deliberately excludes it.
        let nodes = apply_macros(
            build_through_special_and_replacements(Span::new("image:a&b.png[]")),
            Span::new("image:a&b.png[]"),
            &Parser::default(),
        );

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Image(_))),
            "a macro crossing an escaped special must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* build an image here.
        assert!(golden_macros("image:a&b.png[]").contains("<img"));
    }

    /// Builds the tree **through character replacements** (special characters,
    /// quotes, character replacements) – the state the macros step consumes –
    /// so a test can drive [`apply_macros`] directly.
    fn build_through_special_and_replacements(source: Span<'_>) -> Vec<InlineNode<'_>> {
        apply_character_replacements(build_through_quotes(source), source)
    }
}
