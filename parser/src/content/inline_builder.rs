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
//!   `icon:target[…]`), the **UI macros** (`kbd:[…]`, `btn:[…]`, `menu:…[…]`),
//!   the **`link:`/`mailto:` macro** (`link:target[…]`, `mailto:addr[…]`),
//!   **auto-links and formal-URL links** (`https://example.org`,
//!   `https://example.org[text]`), **cross-references** in both the
//!   `xref:` macro form (`xref:id[text]`) and the `<<id>>` shorthand,
//!   **inline anchors** (`[[id]]`, `[[id,reftext]]`, `anchor:id[reftext]`), and
//!   **index terms** (`((term))`, `(((primary, secondary)))`, `indexterm:[…]`,
//!   `indexterm2:[…]`), replacing each with an [`Image`](InlineNode::Image),
//!   [`Ui`](InlineNode::Ui), [`Ref`](InlineNode::Ref),
//!   [`Anchor`](InlineNode::Anchor), or [`IndexTerm`](InlineNode::IndexTerm)
//!   node. An image node
//!   captures its own owned [`Attrlist`] – the step that makes a macro node
//!   *self-describing*; a link or cross-reference node bakes its computed display
//!   text into [`Text`](InlineNode::Text) children so its fold needs no
//!   build-time state. Each family reuses the shared pattern the string step
//!   matches with ([`INLINE_IMAGE_MACRO`], [`INLINE_KBD_BTN_MACRO`],
//!   [`INLINE_MENU_MACRO`], [`INLINE_LINK_MACRO`], [`INLINE_LINK`],
//!   [`INLINE_XREF`], [`INLINE_ANCHOR`], [`INLINE_INDEXTERM`]), builds
//!   `'src`-borrowing nodes for verbatim macros only (see [`apply_macros`] for
//!   the boundary the escaped-content case defers), and – for the UI macros – is
//!   recognized only under the `experimental` document attribute, exactly as the
//!   string step gates them. The cross-reference pass claims the same-document
//!   form in both spellings (`xref:id[text]` and `<<id>>` / `<<id,text>>`);
//!   inter-document targets, a document-as-a-whole reference, and an
//!   attribute-list text are deferred. An anchor renders from its id alone, so it
//!   is *always* recognized (never deferred): only a non-verbatim reference text
//!   – which does not reach the flow – leaves the node's `reftext` unpopulated.
//!   Likewise a *concealed* index term (`indexterm:[…]`, `(((…)))`) renders
//!   nothing, so it too is always recognized; a *visible* term (`indexterm2:[…]`,
//!   `((term))`) is deferred only when its shown text crosses a rendered span or
//!   carries an attribute list. The remaining macro family (footnotes), inline
//!   STEM (a passthrough-time construct), and the bibliography-anchor form are
//!   later increments.
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
        CharacterReplacement, INLINE_ANCHOR, INLINE_IMAGE_MACRO, INLINE_INDEXTERM,
        INLINE_KBD_BTN_MACRO, INLINE_LINK, INLINE_LINK_MACRO, INLINE_MENU_MACRO, INLINE_XREF,
        NormalizedCaps, QuoteSub, URI_SNIFF, basename, character_replacements,
        hard_line_break_pattern, maybe_has_quotes, maybe_has_replacements, normalize_index_text,
        normalize_text_lf_escaped_bracket, quote_subs, split_kbd_keys, strip_see_and_seealso,
        xref_target::{XrefTarget, interpret_xref_target},
    },
    inlines::{
        Anchor, CharRef, Image, IndexTerm, InlineNode, Ref, RefVariant, SpanForm, StyleVariant,
        Styled, Ui, UiKind,
    },
    parser::{
        CharacterReplacementType, IconRenderParams, ImageRenderParams, IndexTermRenderParams,
        InlineSubstitutionRenderer, LinkRenderParams, MenuRenderParams, QuoteScope, QuoteType,
        SpecialCharacter, XrefRenderParams, has_dangerous_scheme,
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
/// This increment recognizes **image and icon macros** (`image:target[…]`,
/// `icon:target[…]`), the **UI macros** (`kbd:[…]`, `btn:[…]`, `menu:…[…]`),
/// the **`link:`/`mailto:` macro** (`link:target[…]`, `mailto:addr[…]`), and
/// **auto-links and formal-URL links** (`https://example.org`,
/// `https://example.org[text]`), replacing each with an
/// [`Image`](InlineNode::Image), [`Ui`](InlineNode::Ui), or
/// [`Ref`](InlineNode::Ref) node. An image node carries its own owned
/// [`Attrlist`] – the step that makes a macro node *self-describing*, so the
/// fold reconstructs the render parameters and calls the same
/// `render_image`/`render_icon` the string step calls; a UI node carries the
/// keys / label / menu path the string replacer computed, so its fold calls the
/// same `render_keyboard`/`render_button`/`render_menu`; a link node (whether a
/// `link:`/`mailto:` macro or an auto-link) carries the computed target,
/// display text (as [`Text`](InlineNode::Text) children), and roles/window, so
/// its fold calls the same `render_link`. The remaining macro families
/// (cross-references, footnotes, index terms, anchors, STEM) are later
/// increments (see [`link_macro_level`] and [`inline_link_level`] for the link
/// forms this increment defers).
///
/// Each family is applied at each level in the **same order the string step
/// applies them** – keyboard/button, then menu, then image/icon, then
/// auto-links (`INLINE_LINK`), then the `link:`/`mailto:` macro
/// (`INLINE_LINK_MACRO`) – so a level's overlapping constructs resolve
/// identically. Like the other steps it descends
/// into the [`Styled`]/[`Ref`](InlineNode::Ref) children earlier steps created
/// – a macro can appear inside a rendered span (`*image:x[]*`), just as the
/// string pipeline matches one inside a rendered `<strong>` tag – then matches
/// at each level.
///
/// # The UI macros are gated on `experimental`
///
/// The string step recognizes `kbd:`/`btn:`/`menu:` only when the
/// `experimental` document attribute is set (an optimization that skips the
/// work in the common case); this transducer mirrors that gate exactly, so with
/// `experimental` off a `kbd:[…]` stays literal here just as it does in the
/// string output.
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
/// delimiters). For a menu this notably defers the `&gt;`-submenu form
/// (`menu:View[Zoom > Reset]`), whose `>` is always an escaped
/// [`CharRef`](InlineNode::CharRef) by the time macros run; the comma-delimited
/// and bare/single-item forms are verbatim and covered. The differential corpus
/// pins the verbatim cases this increment claims.
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

    // The UI macros run before image/icon and only under `experimental`,
    // mirroring the string step's order and gate.
    let nodes = if parser.is_attribute_set("experimental") {
        let nodes = kbd_btn_macros_level(nodes, root);
        menu_macros_level(nodes, root)
    } else {
        nodes
    };

    let nodes = image_macros_level(nodes, root, parser);

    // Index terms (`((term))`, `(((primary, secondary)))`, `indexterm:[…]`,
    // `indexterm2:[…]`) run after image/icon and before the link families,
    // mirroring the string step's order.
    let nodes = indexterm_macros_level(nodes, root);

    // Auto-links and formal-URL links (`INLINE_LINK`) run after the index-term
    // pass and before the `link:`/`mailto:` macro, mirroring the string step's
    // order (`INLINE_LINK` precedes `INLINE_LINK_MACRO`).
    let nodes = inline_link_level(nodes, root, parser);

    // The `link:`/`mailto:` macro runs after the auto-link pass, mirroring the
    // string step's order.
    let nodes = link_macro_level(nodes, root, parser);

    // Inline anchors (`[[id]]`, `anchor:id[…]`) run after the link families and
    // before cross-references, mirroring the string step's order. The e-mail
    // pass the string step runs between the link macro and the anchor is a later
    // increment, so with it absent this preserves the relative order for the
    // constructs the builder recognizes so far. The bibliography-anchor pass the
    // string step runs *first* (a `^`-anchored `[[[id]]]`) fires only inside a
    // bibliography list item – a context the additive builder is not wired into –
    // so it is a cutover concern, not this pass's.
    let nodes = anchor_macros_level(nodes, root);

    // Cross-references (`xref:id[…]`) run last, after the anchor pass, mirroring
    // the string step's order.
    xref_macros_level(nodes, root)
}

/// One recognized macro match at a level, in absolute match-string byte
/// offsets. Shared across the macro families (image/icon, UI, and later
/// increments), which differ only in how they *build* a match's node.
struct MacroMatch<'src> {
    /// The whole match, `[start, end)`.
    full: std::ops::Range<usize>,

    /// What to emit in place of `full`.
    kind: MacroMatchKind<'src>,
}

enum MacroMatchKind<'src> {
    /// An escaped macro (`\image:…`, `\kbd:[…]`, `\https://…`): drop the single
    /// backslash at `backslash` and keep the rest of the match as literal
    /// nodes, replacing nothing – mirroring the string replacer's
    /// `caps[0][1..]`. The backslash is at the match start for the
    /// prefix-less macros, and at the scheme for an escaped auto-link whose
    /// match carries a boundary prefix.
    Unescape { backslash: usize },

    /// A recognized macro, built into its node ([`Image`](InlineNode::Image),
    /// [`Ui`](InlineNode::Ui), [`Ref`](InlineNode::Ref), …). Boxed to keep this
    /// enum small – a macro node is far larger than the
    /// [`Unescape`](Self::Unescape) variant.
    ///
    /// The node replaces only the `consumed` sub-range of the match; the match
    /// text before it (`[full.start, consumed.start)`, a boundary prefix a
    /// non-angle auto-link keeps) and after it (`[consumed.end, full.end)`, the
    /// trailing punctuation a bare URL strips) is kept as literal. For the
    /// macro families that consume their whole match (image, UI,
    /// `link:`/`mailto:`), `consumed` equals the full match, so no prefix
    /// or suffix is kept.
    Node {
        consumed: std::ops::Range<usize>,
        node: Box<InlineNode<'src>>,
    },
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

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every image/icon macro at this level, skipping any whose match is not
/// wholly verbatim source (see [`apply_macros`]).
fn find_image_matches<'src>(
    s: &str,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Vec<MacroMatch<'src>> {
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
            matches.push(MacroMatch {
                kind: MacroMatchKind::Unescape {
                    backslash: full.start,
                },
                full,
            });

            continue;
        }

        let node = build_image_node(&caps, &full, pieces, root, parser);

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

/// Rebuilds a level's node list from its macro matches: each gap keeps its
/// original nodes; each match becomes either its literal text (an escape, with
/// the leading backslash dropped) or the built macro node. Shared across the
/// macro families, which differ only in how they produce the [`MacroMatch`]
/// list.
fn rebuild_macro_level<'src>(
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    s: &str,
    matches: Vec<MacroMatch<'src>>,
) -> Vec<InlineNode<'src>> {
    let mut out = Vec::new();
    let mut cursor = 0usize;

    for m in matches {
        let MacroMatch { full, kind } = m;

        match kind {
            MacroMatchKind::Unescape { backslash } => {
                // Keep the whole match with the single backslash dropped.
                emit_range(nodes, pieces, cursor..backslash, &mut out);
                emit_range(nodes, pieces, (backslash + 1)..full.end, &mut out);
            }

            MacroMatchKind::Node { consumed, node } => {
                // The gap runs to the node, absorbing any kept boundary prefix;
                // the node replaces `consumed`; any stripped trailing
                // punctuation after it is kept as literal.
                emit_range(nodes, pieces, cursor..consumed.start, &mut out);
                out.push(*node);
                emit_range(nodes, pieces, consumed.end..full.end, &mut out);
            }
        }

        cursor = full.end;
    }

    if cursor < s.len() {
        emit_range(nodes, pieces, cursor..s.len(), &mut out);
    }

    out
}

// ─── UI macros (kbd: / btn: / menu:) ──────────────────────────────────────

/// The keyboard/button UI macro pass at a level: matches
/// [`INLINE_KBD_BTN_MACRO`] over the level's escaped text and replaces each
/// verbatim match with the [`Ui`](InlineNode::Ui) node it produces.
///
/// The caller runs this only under the `experimental` document attribute (see
/// [`apply_macros`]); a cheap prefilter still skips the pattern sweep when no
/// `kbd:`/`btn:` prefix with a `:[` is present, mirroring the string step's
/// `found_macroish_short` guard.
fn kbd_btn_macros_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes);

    if !(s.contains(":[") && (s.contains("kbd:") || s.contains("btn:"))) {
        return nodes;
    }

    let matches = find_kbd_btn_matches(&s, &pieces, root);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every keyboard/button macro at this level, skipping any whose match is
/// not wholly verbatim source (see [`apply_macros`]).
fn find_kbd_btn_matches<'src>(
    s: &str,
    pieces: &[Piece],
    root: Span<'src>,
) -> Vec<MacroMatch<'src>> {
    let mut matches = Vec::new();

    for caps in INLINE_KBD_BTN_MACRO.captures_iter(s) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();

        let full = whole.start()..whole.end();

        // Only a wholly-verbatim match maps its bracket content 1:1 onto source;
        // a match crossing an escaped special or a rendered span is left for a
        // later increment.
        if !range_is_verbatim(pieces, &full) {
            continue;
        }

        // Group 1 is the (optional) escape backslash.
        if caps.get(1).is_some() {
            matches.push(MacroMatch {
                kind: MacroMatchKind::Unescape {
                    backslash: full.start,
                },
                full,
            });

            continue;
        }

        let node = build_kbd_btn_node(&caps, &full, pieces, root);

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

/// Builds one [`Ui`](InlineNode::Ui) node from a verbatim keyboard/button
/// match, splitting the keys / normalizing the label exactly as the string
/// replacer does so the fold reproduces the same bytes. On a verbatim match the
/// bracket content is source text, so this splits/normalizes precisely what the
/// string step splits/normalizes from the (identical, un-escaped) rendered
/// text.
fn build_kbd_btn_node<'src>(
    caps: &regex::Captures<'_>,
    full: &std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
) -> InlineNode<'src> {
    let location = source_slice(pieces, full.clone(), root);

    // Group 2 is the macro name; group 3 the bracketed content. Both always
    // participate in a non-escaped match.
    let content = caps.get(3).map_or("", |m| m.as_str());

    let kind = if caps.get(2).map(|m| m.as_str()) == Some("kbd") {
        let keys = split_kbd_keys(content)
            .into_iter()
            .map(CowStr::from)
            .collect();

        UiKind::Keyboard(keys)
    } else {
        UiKind::Button(CowStr::from(normalize_index_text(content, true)))
    };

    InlineNode::Ui(Ui { kind, location })
}

/// The menu UI macro pass at a level: matches [`INLINE_MENU_MACRO`] over the
/// level's escaped text and replaces each verbatim match with the
/// [`Ui`](InlineNode::Ui) node it produces.
///
/// The caller runs this only under the `experimental` document attribute (see
/// [`apply_macros`]); a cheap prefilter still skips the pattern sweep when no
/// `menu:` prefix with an opening bracket is present. The `&gt;`-submenu form
/// is always non-verbatim (its `>` is an escaped
/// [`CharRef`](InlineNode::CharRef) by the time macros run) and is therefore
/// deferred; the comma-delimited and bare/single-item forms are verbatim and
/// handled here.
fn menu_macros_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes);

    if !(s.contains("menu:") && s.contains('[')) {
        return nodes;
    }

    let matches = find_menu_matches(&s, &pieces, root);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every menu macro at this level, skipping any whose match is not wholly
/// verbatim source (see [`apply_macros`]).
fn find_menu_matches<'src>(s: &str, pieces: &[Piece], root: Span<'src>) -> Vec<MacroMatch<'src>> {
    let mut matches = Vec::new();

    for caps in INLINE_MENU_MACRO.captures_iter(s) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();

        let full = whole.start()..whole.end();

        if !range_is_verbatim(pieces, &full) {
            continue;
        }

        // The menu pattern's escape is an uncaptured leading `\?`.
        if whole.as_str().starts_with('\\') {
            matches.push(MacroMatch {
                kind: MacroMatchKind::Unescape {
                    backslash: full.start,
                },
                full,
            });

            continue;
        }

        let node = build_menu_node(&caps, &full, pieces, root);

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

/// Builds one [`Ui`](InlineNode::Ui) menu node from a verbatim match. The menu
/// name borrows from `'src`; the submenu path and trailing item are split
/// exactly as the string replacer splits them (owned, because a split part is
/// trimmed).
fn build_menu_node<'src>(
    caps: &regex::Captures<'_>,
    full: &std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
) -> InlineNode<'src> {
    let location = source_slice(pieces, full.clone(), root);

    // Group 1 (the menu name) is mandatory in the pattern, and on a verbatim
    // match it slices straight from `'src`, so the name borrows.
    // `unwrap` is safe: the pattern cannot match without group 1.
    #[allow(clippy::unwrap_used)]
    let name = caps.get(1).unwrap();

    let menu = CowStr::from(source_slice(pieces, name.start()..name.end(), root).data());

    // Group 2 (the items) is optional.
    let (submenus, item) = split_menu_items(caps.get(2).map(|m| m.as_str()));

    InlineNode::Ui(Ui {
        kind: UiKind::Menu {
            menu,
            submenus,
            item,
        },
        location,
    })
}

/// Splits a menu macro's item list into its submenu path and trailing item,
/// reproducing the string replacer's delimiter handling: a `&gt;` (from a
/// source `>`) takes precedence over a comma, the last part is the menu item,
/// and any earlier parts are submenus. With no delimiter the whole
/// (right-trimmed) list is a single item, and an absent list (an empty `[]`) is
/// a bare menu reference. Because a `&gt;` never survives into a *verbatim*
/// match (see [`menu_macros_level`]), only the comma / single-item / bare
/// branches run in practice; the `&gt;` branch is kept so the split stays
/// faithful.
fn split_menu_items<'src>(items: Option<&str>) -> (Vec<CowStr<'src>>, Option<CowStr<'src>>) {
    let Some(items) = items else {
        return (vec![], None);
    };

    let mut items = items.to_string();
    if items.contains(']') {
        items = items.replace("\\]", "]");
    }

    let delim = if items.contains("&gt;") {
        Some("&gt;")
    } else if items.contains(',') {
        Some(",")
    } else {
        None
    };

    if let Some(delim) = delim {
        let mut parts: Vec<String> = items.split(delim).map(|i| i.trim().to_string()).collect();
        let item = parts.pop().map(CowStr::from);

        (parts.into_iter().map(CowStr::from).collect(), item)
    } else {
        (vec![], Some(CowStr::from(items.trim_end().to_string())))
    }
}

// ─── Auto-links and formal-URL links (INLINE_LINK) ────────────────────────

/// The auto-link / formal-URL-link pass at a level: matches [`INLINE_LINK`]
/// over the level's escaped text and replaces each verbatim, recognized match
/// with the [`Ref`](InlineNode::Ref) link node it produces.
///
/// # Scope
///
/// This increment covers [`INLINE_LINK`]'s **non-angle branch** – a bare
/// auto-linked URL (`https://example.org`) and a formal URL link
/// (`https://example.org[text]`, `https://example.org[]`) – in its verbatim,
/// attribute-list-free forms, reproducing the string replacer's boundary-prefix
/// preservation, bare-URL trailing-punctuation stripping, `^` new-window
/// suffix, `hide-uri-scheme` display text, and `\` scheme escape. It
/// deliberately leaves several forms **unrecognized** for a later increment,
/// each left as literal source here (so the differential corpus only pins the
/// forms this increment claims):
///
/// - The **angle-bracketed URL** form (`<https://example.org>`) requires a
///   leading `&lt;`, which is always an escaped
///   [`CharRef`](InlineNode::CharRef) by the time macros run – so its match is
///   never verbatim, exactly the boundary the image increment documents.
/// - The **`link:` URL macro** form (`link:https://example.org[text]`, the
///   pattern's LINK-MACRO branch) is left to [`link_macro_level`], which folds
///   the identical node; running that pass second mirrors the string step's
///   order.
/// - A **formal text carrying an attribute list** (an `=` selecting roles / id
///   / title / window) is deferred, exactly as [`link_macro_level`] defers it:
///   the attribute list is parsed from a newline-normalized *copy* of the text,
///   so it cannot ride on the node as an [`Attrlist`]`<'src>` yet.
/// - A **non-verbatim match** – a URL crossing an escaped special
///   ([`CharRef`](InlineNode::CharRef)) or a rendered [`Styled`] span – is
///   deferred exactly as the image increment defers `image:a&b.png[]`.
///
/// An invalid quoted bare URL (`"https://example.org`) and a bare scheme with no
/// body (`http://;`) are left literal by the string step *and* the builder, so
/// they render identically and are covered by the differential corpus rather
/// than a divergence test.
fn inline_link_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes);

    // Cheap pre-filter mirroring the string step's guard: an auto-link needs a
    // `://` scheme separator somewhere in the level.
    if !s.contains("://") {
        return nodes;
    }

    let matches = find_inline_link_matches(&s, &pieces, root, parser);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every recognized auto-link / formal-URL link at this level, skipping
/// an angle-bracketed or `link:` macro match, a non-verbatim match, and any
/// form [`build_inline_link_node`] defers (see [`inline_link_level`]).
fn find_inline_link_matches<'src>(
    s: &str,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Vec<MacroMatch<'src>> {
    let mut matches = Vec::new();

    for caps in INLINE_LINK.captures_iter(s) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();

        let full = whole.start()..whole.end();

        let n = NormalizedCaps::new(&caps);

        // The angle-bracketed form always crosses a `&lt;` `CharRef` (so it is
        // never verbatim), and the `link:` URL macro form is left to
        // `link_macro_level`, which folds the identical node.
        if n.is_angle() || n.is_link_macro() {
            continue;
        }

        // Only a wholly-verbatim match can slice its target/text from `'src`; a
        // match crossing an escaped special or a rendered span is left for a
        // later increment.
        if !range_is_verbatim(pieces, &full) {
            continue;
        }

        match build_inline_link_node(&n, &full, pieces, root, parser) {
            Some(m) => matches.push(m),

            // A deferred or invalid form (an escaped scheme is handled inside as
            // an `Unescape`; a quoted bare URL, a bare scheme, or an
            // attribute-list text is left as literal source).
            None => continue,
        }
    }

    matches
}

/// Builds one [`MacroMatch`] for a verbatim non-angle [`INLINE_LINK`] match: a
/// [`Ref`](InlineNode::Ref) link node (with the boundary prefix kept before it
/// and any stripped trailing punctuation kept after it), or an
/// [`Unescape`](MacroMatchKind::Unescape) for an escaped scheme. Returns `None`
/// for a form this increment defers or that the string step leaves literal (see
/// [`inline_link_level`]).
///
/// The value computation mirrors [`InlineLinkReplacer`] exactly – target, bare
/// vs. labeled display text, `hide-uri-scheme`, the `^` window suffix, and the
/// trailing-punctuation strip – so the fold reproduces the same bytes through
/// the same `render_link` [`link_macro_level`]'s nodes fold through.
///
/// Like every macro family in this additive builder, it deliberately performs
/// *no* recognition side effect: it does **not** `register_link` the target in
/// the document's asset catalog, because the builder is not yet the
/// authoritative recognition sink – the string pipeline still registers it, and
/// registering it here too would double-count it. The cutover (design §5.2
/// Phase 4, step 6) re-attaches this registration, so
/// `Document::catalog().links()` stays populated by the string pipeline until
/// then. (The same applies to the `link:`/`mailto:` macro node built by
/// [`build_link_node`].)
///
/// [`InlineLinkReplacer`]: crate::content::macros
fn build_inline_link_node<'src>(
    n: &NormalizedCaps<'_, '_>,
    full: &std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Option<MacroMatch<'src>> {
    // `scheme_match` is always present in a non-angle match (the branch cannot
    // match without it); fall through defensively if it is somehow absent.
    let scheme_m = n.scheme_match()?;
    let scheme = scheme_m.as_str();

    // An escaped scheme (`\https://…`) keeps the boundary prefix and drops the
    // single backslash, leaving the rest of the match literal – no link node.
    if scheme.starts_with('\\') {
        return Some(MacroMatch {
            kind: MacroMatchKind::Unescape {
                backslash: scheme_m.start(),
            },
            full: full.clone(),
        });
    }

    let prefix = n.prefix_str();

    // The URL body is the formal-macro target group or, for a bare link, the
    // bare group; the two are mutually exclusive.
    let url_part = n.target().or_else(|| n.bare()).map_or("", |m| m.as_str());
    let mut target = format!("{scheme}{url_part}");

    // The node consumes the URL (and, for a formal macro, its `[…]` attrlist)
    // but not the boundary prefix; a bare URL additionally stops short of any
    // trailing punctuation the string step strips out.
    let mut consumed_end = full.end;

    let mut link_text: Option<String> = None;

    if let Some(attrlist_m) = n.attrlist() {
        // A formal URL link: the bracketed text is the display text (empty means
        // a bare link, handled by the shared post-processing below).
        if !attrlist_m.is_empty() {
            link_text = Some(attrlist_m.as_str().to_string());
        }
    } else {
        // A bare auto-link.

        // A URL wrapped in quotes with no brackets is invalid macro syntax; the
        // string step leaves the whole match literal.
        if prefix == "\"" || prefix == "'" {
            return None;
        }

        // Strip a trailing ';' or ':' (and an adjacent ')') off a bare URL,
        // keeping it as literal text after the link – mirroring the string
        // replacer, which keys off the target's final character.
        let mut stripped = 0usize;

        if target.ends_with([';', ':']) {
            target.truncate(target.len() - 1);
            stripped += 1;

            if target.ends_with(')') {
                target.truncate(target.len() - 1);
                stripped += 1;
            }
        }

        // A bare scheme with nothing left after trimming is not a link; leave it
        // literal, exactly as the string step does.
        if target.ends_with("://") {
            return None;
        }

        // The bare group is the last group in the match, so the URL ends at
        // `full.end`; the node stops short of the stripped punctuation, which the
        // stripped bytes (ASCII) place `stripped` bytes back.
        consumed_end = full.end - stripped;
    }

    // The display text becomes the node's children, located at the bracketed
    // text (a formal link) or the node itself (a bare link). Its value is a
    // computed (owned) string, so this is a *synthesized* `Text`.
    let text_location_range = n.attrlist().map(|m| m.start()..m.end());

    let mut window: Option<CowStr<'src>> = None;
    let mut bare = false;

    let link_text = if let Some(mut link_text) = link_text {
        link_text = link_text.replace("\\]", "]");

        // A text carrying an `=` splits into an attribute list, parsed from a
        // newline-normalized copy of the text (not from `'src`); defer the whole
        // macro until the node can carry an `Attrlist<'src>`.
        if link_text.contains('=') {
            return None;
        }

        if link_text.ends_with('^') {
            link_text.truncate(link_text.len() - 1);
            window = Some(CowStr::from("_blank"));
        }

        if link_text.is_empty() {
            bare = true;
            hide_uri_scheme_text(&target, parser)
        } else {
            link_text
        }
    } else {
        bare = true;
        hide_uri_scheme_text(&target, parser)
    };

    let mut roles: Vec<CowStr<'src>> = vec![];
    if bare {
        roles.push(CowStr::from("bare"));
    }

    let consumed = scheme_m.start()..consumed_end;
    let location = source_slice(pieces, consumed.clone(), root);

    let text_location =
        text_location_range.map_or(location, |range| source_slice(pieces, range, root));

    let children = vec![InlineNode::Text {
        value: CowStr::from(link_text),
        location: text_location,
    }];

    let node = InlineNode::Ref(Ref {
        variant: RefVariant::Link,
        target: CowStr::from(target),
        children,
        roles,
        window,
        resolved: None,
        location,
    });

    Some(MacroMatch {
        kind: MacroMatchKind::Node {
            consumed,
            node: Box::new(node),
        },
        full: full.clone(),
    })
}

/// The display text for a bare link, dropping the URI scheme under
/// `hide-uri-scheme` exactly as the string replacer's `URI_SNIFF` strip does.
/// Unlike a bare `link:`/`mailto:` macro, [`INLINE_LINK`]'s bare branch does
/// *not* fall back to the whole target when the strip leaves nothing – it
/// cannot leave nothing, because a bare scheme with no body is rejected
/// upstream.
fn hide_uri_scheme_text(target: &str, parser: &Parser) -> String {
    if parser.is_attribute_set("hide-uri-scheme") {
        URI_SNIFF.replace_all(target, "").into_owned()
    } else {
        target.to_string()
    }
}

// ─── Link and mailto macros (link: / mailto:) ─────────────────────────────

/// The `link:`/`mailto:` macro pass at a level: matches [`INLINE_LINK_MACRO`]
/// over the level's escaped text and replaces each verbatim, recognized match
/// with the [`Ref`](InlineNode::Ref) link node it produces.
///
/// # Scope
///
/// This increment covers the **explicit `link:`/`mailto:` macro** in its
/// verbatim, attribute-list-free forms: `link:target[text]`, `link:target[]`
/// (a bare link), `mailto:addr[text]`, and `mailto:addr[]`, plus the `^`
/// new-window suffix and the `\` escape. It deliberately leaves several forms
/// **unrecognized** for a later increment, each left as literal source here (so
/// the differential corpus only pins the forms this increment claims):
///
/// - **Auto-links and formal-URL links** (`https://example.org`, `https://example.org[text]`)
///   are matched by a *different* pattern (`INLINE_LINK`, with its bare-URL
///   trailing-punctuation handling) and are a separate later increment.
/// - **A link text that carries an attribute list** – a `,` in a `mailto:` text
///   (its `subject`/`body`) or an `=` in a `link:` text (roles / id / title /
///   window) – is deferred, because that attribute list is parsed from a
///   newline-normalized *copy* of the text (not from `'src`) and so cannot be
///   carried as an [`Attrlist`]`<'src>` on the node yet.
/// - **A non-verbatim match** – a macro whose target or text crosses an escaped
///   special ([`CharRef`](InlineNode::CharRef)) or a rendered [`Styled`] span
///   (`link:a&b[]`, `link:x[*bold*]`) – is deferred exactly as the image
///   increment defers `image:a&b.png[]`.
///
/// A `link:` (not `mailto:`) target whose scheme could execute script
/// (`javascript:`, `data:`, `vbscript:`) is likewise left literal – matching
/// the string step, which neutralizes it, so it renders identically; the
/// additive builder simply skips the `record_substitution_warning` side effect
/// the string step performs there.
fn link_macro_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes);

    // Cheap pre-filter mirroring the string step's guard: a link/mailto macro
    // needs its prefix and an opening bracket.
    if !((s.contains("link:") || s.contains("mailto:")) && s.contains('[')) {
        return nodes;
    }

    let matches = find_link_macro_matches(&s, &pieces, root, parser);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every recognized `link:`/`mailto:` macro at this level, skipping any
/// match that is not wholly verbatim source or that this increment defers (see
/// [`link_macro_level`]).
fn find_link_macro_matches<'src>(
    s: &str,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Vec<MacroMatch<'src>> {
    let mut matches = Vec::new();

    for caps in INLINE_LINK_MACRO.captures_iter(s) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();

        let full = whole.start()..whole.end();

        // Only a wholly-verbatim match can slice its target/text from `'src`; a
        // match crossing an escaped special or a rendered span is left for a
        // later increment.
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

        match build_link_node(&caps, &full, pieces, root, parser) {
            Some(node) => matches.push(MacroMatch {
                kind: MacroMatchKind::Node {
                    consumed: full.clone(),
                    node: Box::new(node),
                },
                full,
            }),

            // A deferred form (an attribute-list-in-text macro, or a rejected
            // dangerous `link:` scheme) is left as literal source: parity with
            // the string step for the dangerous case, a documented divergence
            // for the attribute-list case.
            None => continue,
        }
    }

    matches
}

/// Builds one [`Ref`](InlineNode::Ref) link node from a verbatim
/// `link:`/`mailto:` match, computing the target, display text, window, and
/// roles exactly as the string replacer does so the fold reproduces the same
/// bytes. Returns `None` for a form this increment defers (see
/// [`link_macro_level`]): a rejected dangerous `link:` scheme, or a link text
/// that carries an attribute list.
///
/// The display text is baked into a single [`Text`](InlineNode::Text) child, so
/// the fold recovers `link_text` by folding the children and needs no
/// build-time state (bare-vs-labeled, `hide-uri-scheme`, `mailto:`) at fold
/// time; the `bare` role, when the string step would add one, rides on the
/// node's `roles`.
///
/// As in the additive builder generally, this performs *no* recognition side
/// effect – notably it does **not** `register_link` the target in the asset
/// catalog, which the string replacer does; the cutover (design §5.2 Phase 4,
/// step 6) re-attaches that (see [`build_inline_link_node`]).
fn build_link_node<'src>(
    caps: &regex::Captures<'_>,
    full: &std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Option<InlineNode<'src>> {
    let location = source_slice(pieces, full.clone(), root);

    // Group 1 present ⟺ a `mailto:` macro; group 3 is the (optional) target;
    // group 5 is the (optional) bracketed text.
    let is_mailto = caps.get(1).is_some();
    let target_str = caps.get(3).map_or("", |m| m.as_str());

    let target = if is_mailto {
        format!("mailto:{target_str}")
    } else {
        target_str.to_string()
    };

    // A `link:` target whose scheme could execute script is neutralized by the
    // string step (left literal); mirror that by deferring – the node would
    // otherwise render a live, dangerous link. `mailto:` carries its own safe
    // scheme and is exempt.
    if !is_mailto && has_dangerous_scheme(&target) {
        return None;
    }

    let mut window: Option<CowStr<'src>> = None;

    let mut link_text = caps
        .get(5)
        .map_or_else(String::new, |m| m.as_str().to_string());

    if !link_text.is_empty() {
        // An attribute list embedded in the text (`mailto:` subject/body via a
        // comma, or `link:` roles/id/title via an `=`) is parsed from a
        // newline-normalized copy of the text, so it cannot be carried as an
        // `Attrlist<'src>` on the node yet; defer the whole macro.
        if (is_mailto && link_text.contains(',')) || (!is_mailto && link_text.contains('=')) {
            return None;
        }

        link_text = link_text.replace("\\]", "]");

        if link_text.ends_with('^') {
            link_text.truncate(link_text.len() - 1);
            window = Some(CowStr::from("_blank"));
        }
    }

    let mut roles: Vec<CowStr<'src>> = vec![];

    if link_text.is_empty() {
        if is_mailto {
            // A bare `mailto:` shows the address (group 3) and takes no `bare`
            // role.
            link_text = target_str.to_string();
        } else {
            // A bare `link:` shows the target (with the scheme optionally hidden)
            // and takes the `bare` role.
            link_text = if parser.is_attribute_set("hide-uri-scheme") {
                let stripped = URI_SNIFF.replace_all(&target, "").into_owned();

                if stripped.is_empty() {
                    target.clone()
                } else {
                    stripped
                }
            } else {
                target.clone()
            };

            roles.push(CowStr::from("bare"));
        }
    }

    // The display text becomes the node's children, located at the bracketed
    // text (or the whole macro when there is none). `link_text` is a computed
    // (owned) value, so this is a *synthesized* `Text` whose value need not
    // coincide with its source.
    let text_location = caps
        .get(5)
        .map_or(location, |m| source_slice(pieces, m.start()..m.end(), root));

    let children = if link_text.is_empty() {
        vec![]
    } else {
        vec![InlineNode::Text {
            value: CowStr::from(link_text),
            location: text_location,
        }]
    };

    Some(InlineNode::Ref(Ref {
        variant: RefVariant::Link,
        target: CowStr::from(target),
        children,
        roles,
        window,
        resolved: None,
        location,
    }))
}

// ─── Index terms (((term)) / (((primary, secondary))) / indexterm:[…]) ─────

/// Matches [`INLINE_INDEXTERM`] at this level's escaped text, replacing each
/// recognized index term – the `((term))` / `(((primary, secondary,
/// tertiary)))` shorthand and the `indexterm:[…]` / `indexterm2:[…]` macro –
/// with the [`IndexTerm`](InlineNode::IndexTerm) node it produces and leaving
/// everything else in place.
fn indexterm_macros_level<'src>(
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
/// [`render_index_term`](InlineSubstitutionRenderer::render_index_term) emits
/// no output for it – so, much like an inline anchor whose output is a function
/// of its id alone (see [`find_anchor_matches`]), it is recognized regardless
/// of what its argument crosses; the node simply carries an empty `terms`. (The
/// one exception is the string replacer's whole-substitution no-op for a level
/// of *only* concealed shorthand terms, which [`indexterm_macros_level`]
/// mirrors by leaving the level literal.)
///
/// A **visible** (flow) term (`indexterm2:[…]`, `((term))`) shows its term text
/// in the flow, so that text must be reconstructible from this level's escaped
/// match string. It is – with the *same* entity bytes the string pipeline sees,
/// since a [`CharRef`](InlineNode::CharRef) contributes its canonical entity to
/// the match string – whenever the term crosses no *opaque span* (an
/// earlier-recognized [`Styled`]/[`Ref`], carried here as a single
/// [`SPAN_PLACEHOLDER`] rather than its rendered markup). A visible term
/// crossing such a span, or an `indexterm2:[…]` term carrying an attribute list
/// (an `=`, whose first positional attribute becomes the shown term), is
/// **deferred** – the match is left as literal source for a later increment,
/// each pinned by a divergence test.
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
/// [`fold_index_term`] feeds back to `render_index_term` for byte-identical
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
        // its markup), so it is deferred.
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
        // the escaped string; defer it (see [`find_indexterm_matches`]).
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

// ─── Inline anchors ([[id]] / anchor:id[…]) ───────────────────────────────

/// Matches `INLINE_ANCHOR` at this level's escaped text, replacing each
/// recognized inline anchor – the `[[id]]` / `[[id,reftext]]` shorthand and the
/// `anchor:id[reftext]` macro – with the [`Anchor`](InlineNode::Anchor) node it
/// produces and leaving everything else in place.
fn anchor_macros_level<'src>(
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
fn find_anchor_matches<'src>(s: &str, pieces: &[Piece], root: Span<'src>) -> Vec<MacroMatch<'src>> {
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

// ─── Cross-references (xref: / <<id>>) ─────────────────────────────────────

/// Matches `INLINE_XREF` at this level's escaped text, replacing each
/// recognized `xref:` macro with the [`Ref`](InlineNode::Ref)`{Xref}` node it
/// produces and leaving everything else in place.
fn xref_macros_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes);

    // Cheap pre-filter: both the `xref:` macro form and the `<<id>>` shorthand
    // (seen here as `&lt;&lt;id&gt;&gt;`, since specials run before macros) are
    // recognized. The prefilter triggers on either the macro prefix or the
    // shorthand's `&lt;&lt;` opener, mirroring the string step's
    // `text.contains("&lt;&lt;") || (found_macroish && text.contains("xref:"))`
    // guard.
    if !s.contains("xref:") && !s.contains("&lt;&lt;") {
        return nodes;
    }

    let matches = find_xref_matches(&s, &pieces, root);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every recognized cross-reference at this level – the `xref:` macro
/// form and the `<<id>>` shorthand – skipping any match that is not verbatim
/// enough to slice from `'src` or that this increment defers (see
/// [`build_xref_node`] and [`build_xref_shorthand_node`]).
fn find_xref_matches<'src>(s: &str, pieces: &[Piece], root: Span<'src>) -> Vec<MacroMatch<'src>> {
    let mut matches = Vec::new();

    for caps in INLINE_XREF.captures_iter(s) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();

        let full = whole.start()..whole.end();

        // The `xref:` macro (group 3) and the `<<…>>` shorthand (group 2) differ
        // in what must be verbatim to build a node. The macro's whole match is
        // sliced from `'src`, so all of it must be verbatim. The shorthand's
        // `&lt;&lt;` / `&gt;&gt;` delimiters are always `CharRef`s that the node
        // *consumes* rather than slices, so only its inner text (group 2) – the
        // id and any reference text – need be verbatim.
        let shorthand_inner = caps.get(2).map(|inner| inner.start()..inner.end());

        let verbatim = match &shorthand_inner {
            Some(inner) => range_is_verbatim(pieces, inner),
            None => range_is_verbatim(pieces, &full),
        };

        // An escape (`\xref:` / `\<<`) is honored by dropping the backslash and
        // keeping the rest literal, mirroring the string replacer's leading
        // `caps.get(1)` check. For the shorthand the unescape runs even when the
        // inner is not verbatim, because [`rebuild_macro_level`] emits the
        // `CharRef` delimiters (and any rendered span between them) whole; the
        // macro form keeps its established verbatim-first order.
        if whole.as_str().starts_with('\\') {
            if shorthand_inner.is_some() || verbatim {
                matches.push(MacroMatch {
                    kind: MacroMatchKind::Unescape {
                        backslash: full.start,
                    },
                    full,
                });
            }

            continue;
        }

        if !verbatim {
            continue;
        }

        let node = match &shorthand_inner {
            Some(inner) => build_xref_shorthand_node(inner.clone(), &full, pieces, root),
            None => build_xref_node(&caps, &full, pieces, root),
        };

        match node {
            Some(node) => matches.push(MacroMatch {
                kind: MacroMatchKind::Node {
                    consumed: full.clone(),
                    node: Box::new(node),
                },
                full,
            }),

            // A form this increment defers (an inter-document target, an
            // attribute-list-in-text macro, or a degenerate shorthand – see the
            // two builders) is left as literal source for a later increment.
            None => continue,
        }
    }

    matches
}

/// Builds one [`Ref`](InlineNode::Ref)`{Xref}` node from a verbatim `xref:`
/// macro match, computing the target and display text exactly as the string
/// replacer does so the fold reproduces the same bytes. Returns `None` for a
/// form this increment defers.
///
/// The scope this builder claims is the **same-document** `xref:` macro form
/// (`xref:id[]`, `xref:id[Reference Text]`); the `<<id>>` shorthand is built by
/// [`build_xref_shorthand_node`]. Two macro-form targets are deferred, each to
/// a later increment:
///
/// - an **inter-document** target (`xref:other.adoc#frag[]`): its rendering
///   needs a *derived* destination the [`Ref`] node does not carry;
/// - a **text carrying an attribute list** (an `=`, for `window`/`role`/
///   `xrefstyle`): it is parsed as an [`Attrlist`] the node cannot hold yet,
///   exactly as [`build_link_node`] defers the analogous link form.
///
/// The display text becomes the node's children as a single
/// [`Text`](InlineNode::Text), so the fold recovers the provided text by
/// folding the children and needs no build-time state; an empty text yields no
/// children, which the fold reads as "no text provided" (the bracketed `[id]`
/// fallback).
///
/// As in the additive builder generally, this performs *no* recognition side
/// effect – notably it does **not** register the reference for resolution,
/// which the string replacer does by recording a deferred `XrefSegment`; the
/// cutover (design §5.2 Phase 4, step 6) wires resolution to the tree.
fn build_xref_node<'src>(
    caps: &regex::Captures<'_>,
    full: &std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
) -> Option<InlineNode<'src>> {
    // Group 3 is the `xref:` macro target; when it is absent the match is the
    // shorthand form, which this increment defers.
    let raw_target = caps.get(3)?.as_str();

    // This increment recognizes only same-document references. An empty target
    // (`xref:#[]`) points at the document as a whole – which carries a derived
    // destination – and an inter-document target needs one too; both are
    // deferred.
    //
    // The node's `target` is the *interpreted* id (a leading `#` stripped:
    // `#install` → `install`), not the raw source spelling. This is deliberate:
    // it is the value the renderer builds the `href` from and the value
    // resolution keys on, so it matches both the string pipeline's rendering and
    // the recorder tree's `target` (which stores the same interpreted value) –
    // storing the raw `#install` would fold to `href="##install"` and break
    // parity. See the `Ref::target` field docs.
    let target = match interpret_xref_target(raw_target, true) {
        XrefTarget::SameDocument(id) if !id.is_empty() => id,
        _ => return None,
    };

    let raw_text = caps.get(4).map_or("", |m| m.as_str());

    // A text carrying an attribute list (an `=`) is parsed from a
    // newline-normalized copy of the text into named attributes (`window`,
    // `role`, `xrefstyle`); it cannot be carried on the node yet, so defer the
    // whole macro, mirroring the string replacer's `raw_text.contains('=')`
    // branch and [`build_link_node`].
    if raw_text.contains('=') {
        return None;
    }

    let location = source_slice(pieces, full.clone(), root);

    let children = if raw_text.is_empty() {
        vec![]
    } else {
        // The provided text becomes the node's children, located at the
        // bracketed text.
        #[allow(clippy::unwrap_used)]
        let text_span = caps.get(4).unwrap();

        let text_location = source_slice(pieces, text_span.start()..text_span.end(), root);

        // An escaped bracket (`\]`) makes the logical text a computed (owned)
        // value – a *synthesized* `Text` whose value need not coincide with its
        // source, mirroring the string replacer's `raw_text.replace`. Without
        // one the text is verbatim, so it borrows the very bytes its location
        // covers (the builder's `'src`-borrowing goal).
        let value = if raw_text.contains("\\]") {
            CowStr::from(raw_text.replace("\\]", "]"))
        } else {
            CowStr::from(text_location.data())
        };

        vec![InlineNode::Text {
            value,
            location: text_location,
        }]
    };

    Some(InlineNode::Ref(Ref {
        variant: RefVariant::Xref,
        target: CowStr::from(target),
        children,
        roles: vec![],
        window: None,
        resolved: None,
        location,
    }))
}

/// Builds one [`Ref`](InlineNode::Ref)`{Xref}` node from a `<<id>>` shorthand
/// cross-reference, computing the target and display text exactly as the string
/// replacer's shorthand branch does so the fold reproduces the same bytes.
/// Returns `None` for a form this increment defers.
///
/// `inner` is the shorthand's inner text (`INLINE_XREF` group 2) in
/// match-string coordinates; the caller guarantees it is verbatim, so its
/// match-string bytes coincide with source. It is split on the first `,` into
/// an id and an optional reference text, each trimmed – mirroring the string
/// replacer's `inner.split_once(',')` with `id.trim()` / `text.trim()`. The
/// reference text becomes the node's single [`Text`](InlineNode::Text) child
/// (an empty text yields no children, which the fold reads as "no text
/// provided" – the bracketed `[id]` fallback), and the whole `<<…>>` – its
/// `CharRef` delimiters included – is the node's `location`.
///
/// The scope this builder claims is the **same-document** shorthand
/// (`<<id>>`, `<<id,Reference Text>>`). Three forms are deferred, each left as
/// literal source for a later increment and pinned by a divergence test:
///
/// - an **inter-document** shorthand (`<<other#frag>>`): like the macro form,
///   it needs a *derived* destination the [`Ref`] node does not carry;
/// - a **document-as-a-whole** shorthand (`<<>>`, an empty id): it too resolves
///   through a derived destination;
/// - a **`<<id,>>` with an empty reference text**: the string replacer records
///   this as a *present-but-empty* text (rendering an empty `<a>…</a>`), which
///   an empty child vector cannot distinguish from "no text provided" – so the
///   whole shorthand is deferred rather than rendered with the wrong fallback.
///
/// A shorthand whose id already carries a rendered `<` (an earlier-substituted
/// macro, e.g. `<<link:https://example.com[], Example>>`) – which the string
/// replacer leaves untouched – cannot reach here at all: the `<` is a
/// `CharRef`, so the inner is not verbatim and the caller never calls this
/// builder.
///
/// As in the additive builder generally, this performs *no* recognition side
/// effect – notably it does **not** register the reference for resolution; the
/// cutover (design §5.2 Phase 4, step 6) wires resolution to the tree.
fn build_xref_shorthand_node<'src>(
    inner: std::ops::Range<usize>,
    full: &std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
) -> Option<InlineNode<'src>> {
    // The inner is verbatim (the caller checked), so its source slice's bytes
    // coincide with the match string's – a byte offset within `inner_data` maps
    // to a match-string offset by adding `inner.start`.
    let inner_span = source_slice(pieces, inner.clone(), root);
    let inner_data = inner_span.data();

    // Split an optional ", reference text" off the id at the first comma.
    let comma = inner_data.find(',');

    let raw_id = match comma {
        Some(index) => &inner_data[..index],
        None => inner_data,
    };

    // Only same-document shorthands are claimed. An inter-document target
    // (`<<other#frag>>`) carries a derived destination, and an empty id
    // (`<<>>`) names the document as a whole (also derived); both are deferred.
    let target = match interpret_xref_target(raw_id.trim(), false) {
        XrefTarget::SameDocument(id) if !id.is_empty() => id,
        _ => return None,
    };

    let location = source_slice(pieces, full.clone(), root);

    let children = match comma {
        None => vec![],

        Some(index) => {
            let raw_text = &inner_data[index + 1..];
            let trimmed = raw_text.trim();

            // A `<<id,>>` with an empty (or whitespace-only) reference text is a
            // present-but-empty text the node cannot represent (see the doc
            // comment); defer the whole shorthand.
            if trimmed.is_empty() {
                return None;
            }

            // Locate the trimmed reference text at its source. It is verbatim, so
            // the `Text` child borrows the very bytes its location covers.
            let lead = raw_text.len() - raw_text.trim_start().len();

            let text_start = inner.start + index + 1 + lead;
            let text_location = source_slice(pieces, text_start..text_start + trimmed.len(), root);

            vec![InlineNode::Text {
                value: CowStr::from(text_location.data()),
                location: text_location,
            }]
        }
    };

    Some(InlineNode::Ref(Ref {
        variant: RefVariant::Xref,
        target: CowStr::from(target),
        children,
        roles: vec![],
        window: None,
        resolved: None,
        location,
    }))
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
/// [`Ui`](InlineNode::Ui), [`Ref`](InlineNode::Ref) (both link and
/// cross-reference), and [`LineBreak`](InlineNode::LineBreak), plus the
/// design-legal [`Raw`](InlineNode::Raw) leaf; a later increment extends it as
/// the transducer grows new kinds.
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

            InlineNode::Ui(ui) => {
                fold_ui(ui, renderer, parser, out);
            }

            InlineNode::Ref(reference) if reference.variant == RefVariant::Link => {
                fold_link(reference, renderer, parser, out);
            }

            InlineNode::Ref(reference) if reference.variant == RefVariant::Xref => {
                fold_xref(reference, renderer, parser, out);
            }

            InlineNode::Anchor(anchor) => {
                fold_anchor(anchor, renderer, parser, out);
            }

            InlineNode::IndexTerm(index_term) => {
                fold_index_term(index_term, renderer, out);
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
                // `CharRef::Special`, `Styled`, `Image`, `Ui`, `Ref` (link and
                // cross-reference), `Anchor`, `IndexTerm`, and `LineBreak`
                // nodes, and this fold additionally emits the design-legal `Raw`
                // leaf; no other node kind reaches the fold in this increment.
                // A later increment
                // fills in the arms above as the transducer grows new kinds.
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

/// Folds a [`Ui`](InlineNode::Ui) node through the same
/// `render_keyboard`/`render_button`/`render_menu` the string pipeline's macros
/// step calls, reconstructing the render parameters from the keys / label /
/// menu path the macro step captured, so the output is byte-for-byte identical.
/// `parser` is threaded through because rendering a menu reads the document's
/// `icons` attribute to choose the caret between menu levels.
fn fold_ui(
    ui: &Ui<'_>,
    renderer: &dyn InlineSubstitutionRenderer,
    parser: &Parser,
    out: &mut String,
) {
    match &ui.kind {
        UiKind::Keyboard(keys) => {
            let keys: Vec<String> = keys.iter().map(|k| k.to_string()).collect();
            renderer.render_keyboard(&keys, out);
        }

        UiKind::Button(text) => {
            renderer.render_button(text.as_ref(), out);
        }

        UiKind::Menu {
            menu,
            submenus,
            item,
        } => {
            let submenus: Vec<String> = submenus.iter().map(|s| s.to_string()).collect();

            let params = MenuRenderParams {
                menu: menu.as_ref(),
                submenus: &submenus,
                menuitem: item.as_deref(),
                parser,
            };

            renderer.render_menu(&params, out);
        }
    }
}

/// Folds a link [`Ref`](InlineNode::Ref) node through the same `render_link`
/// the string pipeline's macros step calls, reconstructing the
/// [`LinkRenderParams`] from the node: the display text is the fold of the
/// children, the extra roles (`bare`) ride on the node's `roles`, and the
/// target/window come straight off the node. The attribute list is empty
/// because this increment recognizes only the attribute-list-free link forms
/// (see [`link_macro_level`]).
fn fold_link(
    reference: &Ref<'_>,
    renderer: &dyn InlineSubstitutionRenderer,
    parser: &Parser,
    out: &mut String,
) {
    let mut link_text = String::new();
    fold_into_html(&reference.children, renderer, parser, &mut link_text);

    // A link built by this increment carries no attribute list; fold through an
    // empty one, sliced (empty) from the node's own location so its lifetime
    // matches.
    let attrlist = Attrlist::parse(
        reference.location.slice(0..0),
        parser,
        AttrlistContext::Inline,
    )
    .item
    .item;

    let extra_roles: Vec<&str> = reference.roles.iter().map(|r| r.as_ref()).collect();

    let params = LinkRenderParams {
        target: reference.target.to_string(),
        link_text,
        extra_roles,
        window: reference.window.as_deref(),
        attrlist: &attrlist,
        parser,
    };

    renderer.render_link(&params, out);
}

/// Folds a cross-reference [`Ref`](InlineNode::Ref) node through the same
/// `render_xref` the string pipeline's macros step feeds at resolution time,
/// reconstructing the [`XrefRenderParams`] from the node: the provided text is
/// the fold of the children (empty children ⇒ no text, so the renderer emits
/// the bracketed `[id]` fallback), and the target/window/roles come straight
/// off the node.
///
/// This increment recognizes only same-document references, which the additive
/// builder leaves **unresolved** (no catalog-resolution pass runs) and which
/// carry no *derived* destination – so the fold always takes `render_xref`'s
/// unresolved branch, where `xrefstyle` and `derived` play no part. The cutover
/// (design §5.2 Phase 4, step 6) wires resolution to the tree, at which point a
/// resolved reference renders through its resolved destination.
fn fold_xref(
    reference: &Ref<'_>,
    renderer: &dyn InlineSubstitutionRenderer,
    parser: &Parser,
    out: &mut String,
) {
    let mut provided = String::new();
    fold_into_html(&reference.children, renderer, parser, &mut provided);

    let provided_text = if provided.is_empty() {
        None
    } else {
        Some(provided.as_str())
    };

    // `XrefRenderParams::roles` is `&[String]`; the node's `CowStr` roles are
    // materialized into a `String` vector for the borrow.
    let roles: Vec<String> = reference.roles.iter().map(|r| r.to_string()).collect();

    let params = XrefRenderParams {
        target: reference.target.as_ref(),
        provided_text,
        window: reference.window.as_deref(),
        roles: &roles,
        xrefstyle: None,
        derived: None,
        resolved: reference.resolved.as_ref(),
    };

    renderer.render_xref(&params, out);
}

/// Folds an [`Anchor`](InlineNode::Anchor) through the same `render_anchor` the
/// string step calls. The built-in HTML backend emits only `<a id="…"></a>`, so
/// the reference text never reaches the flow; a custom backend that consults it
/// (e.g. one using it as an `xreflabel`) receives the folded reference text
/// **when the node captured it**.
///
/// That capture is verbatim-only: a *non-verbatim* reference text (a rendered
/// span or an escaped special) is `None` on the node (see
/// [`build_anchor_reftext`]), so `render_anchor` receives `None` here where the
/// string replacer would have passed the substituted text. This changes no HTML
/// output – the built-in backend ignores the reference text entirely – and is
/// the same verbatim boundary the node documents; a custom backend that needs
/// the full reference text unconditionally will get it once a re-flow consumer
/// pins richer `reftext` population.
fn fold_anchor(
    anchor: &Anchor<'_>,
    renderer: &dyn InlineSubstitutionRenderer,
    parser: &Parser,
    out: &mut String,
) {
    let reftext = anchor.reftext.as_ref().map(|children| {
        let mut s = String::new();
        fold_into_html(children, renderer, parser, &mut s);
        s
    });

    renderer.render_anchor(&anchor.id, reftext, out);
}

/// Folds an [`IndexTerm`](InlineNode::IndexTerm) through the same
/// `render_index_term` the string step's macros pass calls.
///
/// A **concealed** term (`visible == false`) has an empty `terms` and renders
/// nothing – the HTML backend generates no index. A **visible** (flow) term
/// carries its shown text as `terms[0]` in the *already-substituted* form the
/// recognizer computed (matching the seam's "already-substituted visible term"
/// contract), so `render_index_term` emits it verbatim and the fold reproduces
/// the string pipeline's bytes exactly.
///
/// The node's `terms` mirrors what the [`inline_tree`](super::inline_tree)
/// recorder stores – the single shown term for a visible node, empty for a
/// concealed one; the richer primary/secondary/tertiary structure the field can
/// hold is left to a re-flow consumer to pin (the field is provisional, per the
/// node's Phase-0 note), exactly as an anchor's `reftext` is.
fn fold_index_term(
    index_term: &IndexTerm<'_>,
    renderer: &dyn InlineSubstitutionRenderer,
    out: &mut String,
) {
    let visible_term = if index_term.visible {
        Some(index_term.terms.first().map_or("", CowStr::as_ref))
    } else {
        None
    };

    renderer.render_index_term(&IndexTermRenderParams { visible_term }, out);
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
        inlines::{
            Anchor, CharRef, Image, IndexTerm, InlineNode, Ref, RefVariant, SpanForm, StyleVariant,
            Styled, Ui, UiKind,
        },
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
            // An icon's named `alt` attribute (its value normalized, escaped
            // brackets unescaped).
            "icon:home[alt=Home Page]",
            "icon:home[alt=a\\]b]",
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

    // ─── Macros (UI: kbd / btn / menu) ────────────────────────────────────

    /// A parser with the `experimental` attribute set, so the UI macros are
    /// recognized (the string step gates them on it, and the builder mirrors
    /// that gate).
    fn experimental_parser() -> Parser {
        use crate::parser::ModificationContext;

        Parser::default().with_intrinsic_attribute_bool(
            "experimental",
            true,
            ModificationContext::Anywhere,
        )
    }

    /// Builds the single-pass tree for `source` under [`experimental_parser`].
    fn build_ui(source: Span<'_>) -> Vec<InlineNode<'_>> {
        build(source, &experimental_parser())
    }

    #[test]
    fn fold_matches_the_string_pipeline_through_ui_macros() {
        // For each fixture, folding the single-pass tree (all five steps, under
        // `experimental`) reproduces the string pipeline's output byte-for-byte.
        // This is the differential corpus (design §5.3) that pins the UI-macro
        // increment. Every fixture is deliberately *verbatim* (no `<`/`>`/`&`
        // inside a macro), the boundary this increment claims.
        let fixtures = [
            // No UI macro despite macro-ish characters.
            "kbd is a word, not a macro",
            "a menu without a bracket menu:File stays literal",
            // Passes the `kbd:`/`:[` prefilter but never forms a `kbd:[…]`, so
            // the pattern sweep finds nothing and the level returns unchanged.
            "note kbd: and a :[ bracket here",
            // Keyboard: single key and sequences (both delimiters).
            "kbd:[Enter]",
            "press kbd:[F11] to go full screen",
            "kbd:[Ctrl+T]",
            "kbd:[Ctrl+Shift+N]",
            "kbd:[Ctrl,T]",
            // A key whose value is the delimiter, and an escaped bracket.
            "kbd:[Ctrl + +]",
            "kbd:[Ctrl+\\]]",
            // Button: plain, embedded, and whitespace-normalized.
            "btn:[Save]",
            "click btn:[OK] to continue",
            "btn:[ Trim Me ]",
            // Menu: bare, single item, multi-word item, comma submenu path.
            "menu:File[]",
            "menu:File[Save]",
            "menu:File[Save As]",
            "menu:Tools[Project, Build]",
            "menu:View[Tool Windows, Project, Structure]",
            // Several UI macros together, and next to another macro family.
            "See kbd:[F1] for help and btn:[Go] to run.",
            "kbd:[A] then image:x.png[X]",
            // A UI macro inside a rendered span.
            "*press kbd:[Esc]*",
            "_click btn:[OK] now_",
            // Escapes: the macro stays literal, minus the backslash.
            "\\kbd:[X]",
            "\\btn:[Y]",
            "\\menu:File[Save]",
        ];

        let renderer = HtmlSubstitutionRenderer {};
        let parser = experimental_parser();

        for fixture in fixtures {
            let folded = super::fold_html(&build(Span::new(fixture), &parser), &renderer, &parser);

            assert_eq!(
                folded,
                golden_macros_with(fixture, &parser),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn ui_macros_are_literal_without_experimental() {
        // Without `experimental`, the string step does not recognize the UI
        // macros, and neither does the builder: the fold reproduces the literal
        // (default-parser) output byte-for-byte.
        let fixtures = ["kbd:[Ctrl+T]", "btn:[Save]", "menu:File[Save]"];

        let renderer = HtmlSubstitutionRenderer {};

        for fixture in fixtures {
            let nodes = build_src(Span::new(fixture));

            assert!(
                nodes.iter().all(|n| !matches!(n, InlineNode::Ui(_))),
                "no UI node without experimental: {nodes:?}"
            );

            assert_eq!(
                fold_html(&nodes, &renderer),
                golden_macros(fixture),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    /// Asserts that `node` is a [`Ui`], returning it for further inspection.
    fn assert_ui<'a, 'src>(node: &'a InlineNode<'src>) -> &'a Ui<'src> {
        match node {
            InlineNode::Ui(ui) => ui,

            other => panic!("expected Ui, got {other:?}"),
        }
    }

    #[test]
    fn a_kbd_macro_becomes_a_ui_node() {
        let nodes = build_ui(Span::new("kbd:[Enter]"));

        assert_eq!(nodes.len(), 1);
        let ui = assert_ui(&nodes[0]);

        match &ui.kind {
            UiKind::Keyboard(keys) => assert_eq!(keys, &[CowStr::from("Enter")]),

            other => panic!("expected Keyboard, got {other:?}"),
        }

        // Its location covers the whole macro, delimiters included.
        assert_eq!(ui.location.data(), "kbd:[Enter]");
        assert_eq!(ui.location.line(), 1);
        assert_eq!(ui.location.col(), 1);
    }

    #[test]
    fn a_kbd_sequence_splits_into_keys() {
        // The `+`/`,` delimiter selects how the sequence is split into keys,
        // exactly as the string replacer's `split_kbd_keys` does.
        let nodes = build_ui(Span::new("kbd:[Ctrl+Shift+N]"));

        match &assert_ui(&nodes[0]).kind {
            UiKind::Keyboard(keys) => assert_eq!(
                keys,
                &[
                    CowStr::from("Ctrl"),
                    CowStr::from("Shift"),
                    CowStr::from("N"),
                ]
            ),

            other => panic!("expected Keyboard, got {other:?}"),
        }
    }

    #[test]
    fn a_btn_macro_becomes_a_ui_node() {
        // The label is normalized (surrounding whitespace folded) like the
        // string replacer's `normalize_index_text`.
        let nodes = build_ui(Span::new("btn:[ Save ]"));

        match &assert_ui(&nodes[0]).kind {
            UiKind::Button(text) => assert_eq!(text.as_ref(), "Save"),

            other => panic!("expected Button, got {other:?}"),
        }
    }

    #[test]
    fn a_menu_with_a_comma_path_splits_submenu_and_item() {
        // The last comma-separated part is the menu item; earlier parts are the
        // submenu path. The menu name borrows from source.
        let nodes = build_ui(Span::new("menu:View[Tool Windows, Project, Structure]"));

        match &assert_ui(&nodes[0]).kind {
            UiKind::Menu {
                menu,
                submenus,
                item,
            } => {
                assert!(matches!(menu, CowStr::Borrowed(_)));
                assert_eq!(menu.as_ref(), "View");
                assert_eq!(
                    submenus,
                    &[CowStr::from("Tool Windows"), CowStr::from("Project")]
                );
                assert_eq!(item.as_deref(), Some("Structure"));
            }

            other => panic!("expected Menu, got {other:?}"),
        }
    }

    #[test]
    fn a_bare_menu_reference_has_no_item() {
        // `menu:File[]` is a bare reference: no submenus and no item.
        let nodes = build_ui(Span::new("menu:File[]"));

        match &assert_ui(&nodes[0]).kind {
            UiKind::Menu {
                menu,
                submenus,
                item,
            } => {
                assert_eq!(menu.as_ref(), "File");
                assert!(submenus.is_empty());
                assert_eq!(item.as_deref(), None);
            }

            other => panic!("expected Menu, got {other:?}"),
        }
    }

    #[test]
    fn an_escaped_ui_macro_stays_literal() {
        // `\kbd:[X]` drops the backslash and keeps the macro as literal text –
        // no UI node.
        let nodes = build_ui(Span::new("\\kbd:[X]"));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ui(_))),
            "an escaped UI macro must not produce a UI node: {nodes:?}"
        );

        assert_eq!(
            super::fold_html(&nodes, &HtmlSubstitutionRenderer {}, &experimental_parser()),
            golden_macros_with("\\kbd:[X]", &experimental_parser())
        );
    }

    #[test]
    fn a_ui_macro_is_recognized_inside_a_span() {
        // A UI macro can appear inside a rendered span; the transducer descends
        // into the span body and builds the node there.
        let nodes = build_ui(Span::new("*press kbd:[Esc]*"));

        let children = assert_styled(&nodes[0], StyleVariant::Strong, SpanForm::Constrained);
        assert_eq!(children.len(), 2);
        assert_text(&children[0], "press ", 1, 2);

        match &assert_ui(&children[1]).kind {
            UiKind::Keyboard(keys) => assert_eq!(keys, &[CowStr::from("Esc")]),

            other => panic!("expected Keyboard, got {other:?}"),
        }
    }

    #[test]
    fn a_menu_with_a_submenu_caret_is_a_documented_divergence() {
        // The submenu form `menu:View[Zoom > Reset]` uses `>` as the level
        // delimiter, but by the time macros run the special-characters step has
        // turned `>` into a `&gt;` `CharRef`. A self-describing node cannot carry
        // that escaped text as an `'src` slice, so the single-pass builder leaves
        // the macro *unrecognized* here (deferred to a later increment), exactly
        // as the image increment defers a macro over a special character.
        let source = "menu:View[Zoom > Reset]";
        let nodes = build_ui(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ui(_))),
            "a menu crossing an escaped `>` must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* build a menuseq with a
        // submenu here – the divergence this test documents.
        assert!(golden_macros_with(source, &experimental_parser()).contains(r#"class="submenu""#));
    }

    #[test]
    fn a_kbd_macro_over_a_special_character_is_a_documented_divergence() {
        // A `kbd:` whose bracket content contains a special character (`<`) is
        // matched by the string pipeline over the *escaped* text (`kbd:[a&lt;b]`),
        // but a self-describing node cannot carry that escaped text as an `'src`
        // slice, so the single-pass builder leaves the macro *unrecognized* here
        // (deferred to a later increment), exactly as it defers the `&gt;`-submenu
        // menu form and the image increment defers a macro over a special.
        let nodes = build_ui(Span::new("kbd:[a<b]"));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ui(_))),
            "a kbd macro crossing an escaped special must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* build a keyboard macro here.
        assert!(golden_macros_with("kbd:[a<b]", &experimental_parser()).contains("<kbd>"));
    }

    #[test]
    fn split_menu_items_reproduces_the_delimiter_handling() {
        use super::split_menu_items;

        // Helper to compare against owned expectations.
        let go = |items: Option<&str>| {
            let (submenus, item) = split_menu_items(items);

            (
                submenus.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                item.map(|i| i.to_string()),
            )
        };

        // An absent list (an empty `[]`) is a bare menu reference.
        assert_eq!(go(None), (vec![], None));

        // No delimiter: the whole (right-trimmed) list is a single item.
        assert_eq!(go(Some("Save As ")), (vec![], Some("Save As".to_string())));

        // Comma: the last part is the item, earlier parts are submenus.
        assert_eq!(
            go(Some("Project, Build")),
            (vec!["Project".to_string()], Some("Build".to_string()))
        );

        // An escaped `]` inside the list is unescaped before splitting.
        assert_eq!(
            go(Some("a\\]b, c")),
            (vec!["a]b".to_string()], Some("c".to_string()))
        );

        // The `&gt;` submenu delimiter takes precedence over a comma. This form
        // never reaches [`build_menu_node`] through the verbatim boundary (its
        // source `>` is an escaped `CharRef` by macro time), so it is exercised
        // directly here to keep [`split_menu_items`] a faithful reproduction of
        // the string replacer's splitting.
        assert_eq!(
            go(Some("View &gt; Zoom &gt; Reset")),
            (
                vec!["View".to_string(), "Zoom".to_string()],
                Some("Reset".to_string())
            )
        );
    }

    // ─── Macros (link / mailto) ───────────────────────────────────────────

    #[test]
    fn fold_matches_the_string_pipeline_through_link_macros() {
        // For each fixture, folding the single-pass tree (all five steps)
        // reproduces the string pipeline's output byte-for-byte. This is the
        // differential corpus (design §5.3) that pins the `link:`/`mailto:`
        // macro increment. Every fixture is a *verbatim*, attribute-list-free
        // `link:`/`mailto:` macro – the boundary this increment claims (a URL
        // target, an attribute list in the text, or a special character inside
        // the macro is deferred and lives in a divergence test below).
        let fixtures = [
            // No link macro despite macro-ish characters.
            "plain text with a colon: but no bracket",
            "a link without a bracket link:index.html stays literal",
            // Link macro: labeled, bare, relative and pathed targets.
            "link:index.html[Docs]",
            "link:downloads/report.pdf[Report]",
            "link:index.html[]",
            "link:index.html[Read the docs]",
            // The `^` suffix opens the link in a new window.
            "link:index.html[Open^]",
            // An escaped `]` inside the text is unescaped.
            "link:index.html[a\\]b]",
            // mailto: labeled and bare (bare shows the address).
            "mailto:hello@example.org[Email us]",
            "mailto:hello@example.org[]",
            // Degenerate empty targets: a bare link/mailto with no display text.
            "link:[]",
            "mailto:[]",
            // A macro embedded in surrounding flow, and next to other constructs.
            "See link:about.html[about] for details.",
            "*bold* then link:x.html[X] and _em_",
            "a copyright (C) then link:x.html[X]",
            // Escapes: the macro stays literal, minus the backslash.
            "\\link:index.html[Docs]",
            "\\mailto:hello@example.org[Email]",
            // A `link:` target whose scheme could execute script is left literal
            // by the string step *and* the builder, so it renders identically.
            "link:javascript:alert(1)[Click]",
            // A macro inside a rendered span (recognized inside the span body).
            "*see link:x.html[X]*",
            "_link:y.html[Y] in em_",
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
    fn fold_matches_the_string_pipeline_with_hide_uri_scheme() {
        // Under `hide-uri-scheme`, a bare link's display text drops the URI
        // scheme; the builder reproduces the string step's `URI_SNIFF` stripping,
        // including the fall-back to the whole target when the strip leaves
        // nothing. Every fixture is a non-`://` `link:` target, so the string
        // pipeline routes it through the same `INLINE_LINK_MACRO` the builder
        // uses (a `://` target is `INLINE_LINK`'s territory, a later increment).
        use crate::parser::ModificationContext;

        let parser = Parser::default().with_intrinsic_attribute_bool(
            "hide-uri-scheme",
            true,
            ModificationContext::Anywhere,
        );

        let fixtures = [
            // A scheme prefix is stripped, leaving the remainder as the text.
            "link:foo:bar[]",
            // The whole target is a scheme: the strip leaves nothing, so the
            // text falls back to the target itself.
            "link:foo:[]",
            // No scheme to strip: the target shows unchanged.
            "link:index.html[]",
            // A bare mailto shows the address regardless of `hide-uri-scheme`.
            "mailto:hello@example.org[]",
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

    /// Asserts that `node` is a link [`Ref`], returning it for further
    /// inspection.
    fn assert_link<'a, 'src>(node: &'a InlineNode<'src>) -> &'a Ref<'src> {
        match node {
            InlineNode::Ref(reference) if reference.variant == RefVariant::Link => reference,

            other => panic!("expected a link Ref, got {other:?}"),
        }
    }

    /// The concatenated value of a link node's [`Text`](InlineNode::Text)
    /// display children (the reconstructed `link_text`).
    fn link_text_of(reference: &Ref<'_>) -> String {
        let mut s = String::new();

        for child in &reference.children {
            if let InlineNode::Text { value, .. } = child {
                s.push_str(value);
            }
        }

        s
    }

    #[test]
    fn a_link_macro_becomes_a_ref_node() {
        let nodes = build_src(Span::new("link:index.html[Docs]"));

        assert_eq!(nodes.len(), 1);
        let reference = assert_link(&nodes[0]);

        assert_eq!(reference.target.as_ref(), "index.html");
        assert_eq!(link_text_of(reference), "Docs");
        assert!(reference.roles.is_empty());
        assert_eq!(reference.window, None);
        assert_eq!(reference.resolved, None);

        // Its location covers the whole macro, delimiters included.
        assert_eq!(reference.location.data(), "link:index.html[Docs]");
        assert_eq!(reference.location.line(), 1);
        assert_eq!(reference.location.col(), 1);
    }

    #[test]
    fn a_bare_link_takes_the_bare_role() {
        // An empty text is a bare link: the display text is the target, and the
        // `bare` role rides on the node so the fold reproduces `class="bare"`.
        let nodes = build_src(Span::new("link:index.html[]"));

        let reference = assert_link(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "index.html");
        assert_eq!(link_text_of(reference), "index.html");
        assert_eq!(reference.roles, [CowStr::from("bare")]);
    }

    #[test]
    fn a_mailto_macro_targets_the_address() {
        // A labeled mailto prefixes the address with `mailto:` and shows the
        // label; a bare mailto shows the address itself and takes no `bare` role.
        let labeled = build_src(Span::new("mailto:hello@example.org[Email us]"));
        let reference = assert_link(&labeled[0]);
        assert_eq!(reference.target.as_ref(), "mailto:hello@example.org");
        assert_eq!(link_text_of(reference), "Email us");
        assert!(reference.roles.is_empty());

        let bare = build_src(Span::new("mailto:hello@example.org[]"));
        let reference = assert_link(&bare[0]);
        assert_eq!(reference.target.as_ref(), "mailto:hello@example.org");
        assert_eq!(link_text_of(reference), "hello@example.org");
        assert!(
            reference.roles.is_empty(),
            "a bare mailto takes no `bare` role"
        );
    }

    #[test]
    fn a_link_window_suffix_opens_blank() {
        // A trailing `^` in the text is stripped and selects the `_blank`
        // window, exactly as the string replacer does.
        let nodes = build_src(Span::new("link:index.html[Open^]"));

        let reference = assert_link(&nodes[0]);
        assert_eq!(link_text_of(reference), "Open");
        assert_eq!(reference.window.as_deref(), Some("_blank"));
    }

    #[test]
    fn an_escaped_link_macro_stays_literal() {
        // `\link:…` drops the backslash and keeps the macro as literal text – no
        // link node.
        let nodes = build_src(Span::new("\\link:index.html[Docs]"));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an escaped macro must not produce a link node: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros("\\link:index.html[Docs]")
        );
    }

    #[test]
    fn a_dangerous_link_scheme_is_left_literal() {
        // A `link:` target whose scheme could execute script is neutralized by
        // the string step (left literal); the builder mirrors that by leaving
        // the macro unrecognized, so the two render identically. (The builder
        // additionally skips the warning side effect the string step records.)
        let source = "link:javascript:alert(1)[Click]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "a dangerous link scheme must not produce a link node: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn a_link_over_a_special_character_is_a_documented_divergence() {
        // The string pipeline matches macros over *escaped* text, so a target
        // containing `&` is matched as `a&amp;b.html`. A self-describing node
        // cannot carry that escaped text as an `'src` slice, so the single-pass
        // builder leaves such a macro *unrecognized* for a later increment,
        // exactly as the image increment defers `image:a&b.png[]`.
        let nodes = build_src(Span::new("link:a&b.html[x]"));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "a macro crossing an escaped special must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* build a link here.
        assert!(golden_macros("link:a&b.html[x]").contains("<a href"));
    }

    #[test]
    fn a_link_text_attribute_list_is_a_documented_divergence() {
        // A `link:` text carrying an `=` splits into an attribute list (here a
        // role), which the string replacer parses from a newline-normalized copy
        // of the text – not from `'src`. The builder cannot carry that as an
        // `Attrlist<'src>` yet, so it defers the whole macro (left literal),
        // pending the increment that adds an attribute list to the link node.
        let source = "link:index.html[Docs,role=hl]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an attribute-list-in-text link must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, applies the role.
        assert!(golden_macros(source).contains(r#"class="hl""#));
    }

    #[test]
    fn a_mailto_subject_is_a_documented_divergence() {
        // A `mailto:` text carrying a `,` encodes a `subject` (and optional
        // `body`) into the target – the same attribute-list-from-a-copy handling
        // the builder defers.
        let source = "mailto:team@example.org[Team,Hello there]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "a mailto with a subject must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, encodes the subject into the href.
        assert!(golden_macros(source).contains("subject="));
    }

    #[test]
    fn a_link_is_recognized_inside_a_span() {
        // A macro can appear inside a rendered span; the transducer descends into
        // the span body and builds the node there.
        let nodes = build_src(Span::new("*see link:x.html[X]*"));

        let children = assert_styled(&nodes[0], StyleVariant::Strong, SpanForm::Constrained);
        assert_eq!(children.len(), 2);
        assert_text(&children[0], "see ", 1, 2);

        let reference = assert_link(&children[1]);
        assert_eq!(reference.target.as_ref(), "x.html");
        assert_eq!(link_text_of(reference), "X");
    }

    // ─── Auto-links and formal-URL links (INLINE_LINK) ────────────────────

    #[test]
    fn fold_matches_the_string_pipeline_through_inline_links() {
        // For each fixture, folding the single-pass tree (all five steps)
        // reproduces the string pipeline's output byte-for-byte. This is the
        // differential corpus (design §5.3) that pins the auto-link / formal-URL
        // link increment. Every fixture is a *verbatim*, attribute-list-free
        // link – the boundary this increment claims (an angle form, a URL
        // crossing a special, or an attribute-list text is deferred and lives in
        // a divergence test below).
        let fixtures = [
            // No auto-link despite a colon or a `//`.
            "plain text with a colon: but no scheme",
            "a bare host example.org stays literal",
            // Bare auto-links: at the start, mid-flow, and with a path/query.
            "https://example.org",
            "Visit https://example.org for details.",
            "https://example.org/path/to/page.html",
            "https://example.org/search?q=rust",
            // Other schemes the pattern recognizes.
            "ftp://ftp.example.org/pub",
            "irc://irc.example.org/channel",
            "file:///etc/hosts here",
            // A trailing sentence period is left outside the link (the pattern
            // stops before it); a trailing ';'/':' is stripped back out.
            "See https://example.org. Next sentence.",
            "https://example.org; and more",
            "read https://example.org: really",
            // A bracketed / parenthesized URL keeps the surrounding punctuation.
            "(see https://example.org)",
            "[https://example.org]",
            // A ')' adjacent to a stripped ';'/':' is stripped out with it,
            // keeping both as literal text after the link.
            "See (https://example.org); done.",
            "read (https://example.org): really",
            // Formal URL links: labeled, bare, and the `^` new-window suffix.
            "https://example.org[Example]",
            "https://example.org[]",
            "https://example.org[Open^]",
            // A formal text that is only the `^` suffix becomes an empty, bare,
            // new-window link (the display text falls back to the target).
            "https://example.org[^]",
            // An escaped `]` inside the text is unescaped.
            "https://example.org[a\\]b]",
            // A bare scheme with nothing left after trimming is left literal by
            // both (a `://`-only rejection).
            "http://; is not a link",
            // A URL wrapped in quotes with no brackets is invalid macro syntax:
            // left literal by both.
            "\"https://example.org\" in quotes",
            // Escapes: the scheme's backslash is dropped and the URL stays
            // literal (no link), the boundary prefix preserved.
            "\\https://example.org",
            "see \\https://example.org here",
            "\\https://example.org[text]",
            // Next to other constructs, and inside a rendered span.
            "*bold* then https://example.org and _em_",
            "a copyright (C) then https://example.org",
            "*see https://example.org*",
            "_https://example.org in em_",
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
    fn fold_matches_the_string_pipeline_for_auto_links_with_hide_uri_scheme() {
        // Under `hide-uri-scheme`, a bare auto-link's display text drops the URI
        // scheme; the builder reproduces the string step's `URI_SNIFF` stripping.
        use crate::parser::ModificationContext;

        let parser = Parser::default().with_intrinsic_attribute_bool(
            "hide-uri-scheme",
            true,
            ModificationContext::Anywhere,
        );

        let fixtures = [
            // A bare auto-link shows the scheme-stripped URL.
            "https://example.org",
            "Visit https://example.org for details.",
            // A formal bare link (`[]`) likewise drops the scheme.
            "https://example.org[]",
            // A labeled link keeps its explicit text unchanged.
            "https://example.org[Example]",
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

    #[test]
    fn a_bare_auto_link_becomes_a_ref_node() {
        // A bare URL is a link whose display text is the target and which takes
        // the `bare` role, so the fold reproduces `class="bare"`.
        let nodes = build_src(Span::new("https://example.org"));

        assert_eq!(nodes.len(), 1);
        let reference = assert_link(&nodes[0]);

        assert_eq!(reference.target.as_ref(), "https://example.org");
        assert_eq!(link_text_of(reference), "https://example.org");
        assert_eq!(reference.roles, [CowStr::from("bare")]);
        assert_eq!(reference.window, None);
        assert_eq!(reference.resolved, None);

        // Its location covers the whole URL.
        assert_eq!(reference.location.data(), "https://example.org");
        assert_eq!(reference.location.line(), 1);
        assert_eq!(reference.location.col(), 1);
    }

    #[test]
    fn a_bare_auto_link_keeps_its_boundary_prefix() {
        // The boundary character before the URL (here a space) is not part of
        // the link – it stays as literal text before the node.
        let nodes = build_src(Span::new("Visit https://example.org now"));

        // Text "Visit ", the link, then Text " now".
        assert_eq!(nodes.len(), 3);
        assert_text(&nodes[0], "Visit ", 1, 1);

        let reference = assert_link(&nodes[1]);
        assert_eq!(reference.target.as_ref(), "https://example.org");
        assert_eq!(reference.location.col(), 7);

        assert_text(&nodes[2], " now", 1, 26);
    }

    #[test]
    fn a_bare_auto_link_strips_trailing_punctuation() {
        // A trailing ';' is stripped off the target and kept as literal text
        // after the link, exactly as the string replacer does.
        let nodes = build_src(Span::new("https://example.org; done"));

        let reference = assert_link(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "https://example.org");
        assert_eq!(reference.location.data(), "https://example.org");

        // The stripped ';' is kept as its own literal run, then the rest.
        assert_text(&nodes[1], ";", 1, 20);
        assert_text(&nodes[2], " done", 1, 21);
    }

    #[test]
    fn a_bare_auto_link_strips_a_trailing_paren_before_punctuation() {
        // A ')' adjacent to a stripped ';' is stripped out with it: the link
        // covers only the URL, and both trailing characters are kept as literal
        // text after it (a single run).
        let nodes = build_src(Span::new("(https://example.org);"));

        // Text "(", the link, then Text ");".
        assert_eq!(nodes.len(), 3);
        assert_text(&nodes[0], "(", 1, 1);

        let reference = assert_link(&nodes[1]);
        assert_eq!(reference.target.as_ref(), "https://example.org");
        assert_eq!(reference.location.data(), "https://example.org");

        assert_text(&nodes[2], ");", 1, 21);
    }

    #[test]
    fn a_formal_url_link_that_is_only_a_window_suffix_is_bare() {
        // A `[^]` text is empty once the `^` is stripped, so the link is bare
        // (its display text falls back to the target) *and* opens a new window.
        let nodes = build_src(Span::new("https://example.org[^]"));

        let reference = assert_link(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "https://example.org");
        assert_eq!(link_text_of(reference), "https://example.org");
        assert_eq!(reference.roles, [CowStr::from("bare")]);
        assert_eq!(reference.window.as_deref(), Some("_blank"));
    }

    #[test]
    fn a_formal_url_link_becomes_a_ref_node() {
        // A URL with a `[…]` text is a labeled link: no `bare` role.
        let nodes = build_src(Span::new("https://example.org[Example]"));

        let reference = assert_link(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "https://example.org");
        assert_eq!(link_text_of(reference), "Example");
        assert!(reference.roles.is_empty());

        // Its location covers the whole macro, the `[…]` included.
        assert_eq!(reference.location.data(), "https://example.org[Example]");
    }

    #[test]
    fn a_formal_url_window_suffix_opens_blank() {
        // A trailing `^` in the text is stripped and selects the `_blank`
        // window.
        let nodes = build_src(Span::new("https://example.org[Open^]"));

        let reference = assert_link(&nodes[0]);
        assert_eq!(link_text_of(reference), "Open");
        assert_eq!(reference.window.as_deref(), Some("_blank"));
    }

    #[test]
    fn an_escaped_auto_link_stays_literal() {
        // `\https://…` drops the backslash and keeps the URL as literal text – no
        // link node – with the boundary prefix preserved.
        let nodes = build_src(Span::new("see \\https://example.org here"));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an escaped auto-link must not produce a link node: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros("see \\https://example.org here")
        );
    }

    #[test]
    fn a_link_macro_url_target_is_left_for_the_link_macro_pass() {
        // `link:https://…[…]` is the pattern's LINK-MACRO branch; the auto-link
        // pass leaves it, and `link_macro_level` builds the identical node, so
        // the fold still matches the string pipeline byte-for-byte.
        let source = "link:https://example.org[Example]";
        let nodes = build_src(Span::new(source));

        let reference = assert_link(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "https://example.org");
        assert_eq!(link_text_of(reference), "Example");

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn an_auto_link_is_recognized_inside_a_span() {
        // A URL can appear inside a rendered span; the transducer descends into
        // the span body and builds the node there.
        let nodes = build_src(Span::new("*see https://example.org*"));

        let children = assert_styled(&nodes[0], StyleVariant::Strong, SpanForm::Constrained);
        assert_eq!(children.len(), 2);
        assert_text(&children[0], "see ", 1, 2);

        let reference = assert_link(&children[1]);
        assert_eq!(reference.target.as_ref(), "https://example.org");
        assert_eq!(link_text_of(reference), "https://example.org");
    }

    #[test]
    fn an_angle_bracketed_url_is_a_documented_divergence() {
        // `<https://example.org>` requires a leading `&lt;`, which is an escaped
        // `CharRef` by the time macros run, so the match is never verbatim and
        // the single-pass builder leaves it unrecognized for a later increment.
        let nodes = build_src(Span::new("<https://example.org>"));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an angle-bracketed URL must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* build a link here.
        assert!(golden_macros("<https://example.org>").contains("<a href"));
    }

    #[test]
    fn an_auto_link_over_a_special_character_is_a_documented_divergence() {
        // A URL whose body contains `&` is matched by the string pipeline over
        // the *escaped* text (`…?a=1&amp;b=2`). A self-describing node cannot
        // carry that escaped text as an `'src` slice, so the single-pass builder
        // leaves such a link *unrecognized* for a later increment, exactly as the
        // image increment defers `image:a&b.png[]`.
        let nodes = build_src(Span::new("https://example.org/?a=1&b=2"));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "a URL crossing an escaped special must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* build a link here.
        assert!(golden_macros("https://example.org/?a=1&b=2").contains("<a href"));
    }

    #[test]
    fn a_formal_url_link_attribute_list_is_a_documented_divergence() {
        // A formal URL text carrying an `=` splits into an attribute list (here a
        // role), which the string replacer parses from a newline-normalized copy
        // of the text – not from `'src`. The builder cannot carry that as an
        // `Attrlist<'src>` yet, so it defers the whole macro (left literal).
        let source = "https://example.org[Example,role=hl]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an attribute-list-in-text link must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, applies the role.
        assert!(golden_macros(source).contains(r#"class="hl""#));
    }

    // ─── Macros (cross-references) ────────────────────────────────────────

    /// The string pipeline's output through the **macros** step for `source`,
    /// with any deferred cross-references finalized to their unresolved
    /// fallback. Unlike [`golden_macros`], the macros step defers a
    /// cross-reference to a placeholder rather than rendering it, so the
    /// placeholder must be finalized – no catalog resolution runs, so the
    /// result is the unresolved-fallback rendering the additive builder's
    /// fold (always unresolved) must reproduce.
    fn golden_xref_with(source: &str, parser: &Parser) -> String {
        let mut content = Content::from(Span::new(source));
        SubstitutionStep::SpecialCharacters.apply(&mut content, parser, None);
        SubstitutionStep::Quotes.apply(&mut content, parser, None);
        SubstitutionStep::CharacterReplacements.apply(&mut content, parser, None);
        SubstitutionStep::Macros.apply(&mut content, parser, None);
        SubstitutionStep::PostReplacement.apply(&mut content, parser, None);

        // Finalize as the real pipeline does after the last step, capturing the
        // placeholder template and rebuilding the unresolved fallback.
        content.finalize_deferred(&HtmlSubstitutionRenderer {});
        content.rendered_str().to_string()
    }

    /// [`golden_xref_with`] with a default parser.
    fn golden_xref(source: &str) -> String {
        golden_xref_with(source, &Parser::default())
    }

    /// Asserts that `node` is a cross-reference [`Ref`](InlineNode::Ref), and
    /// returns it.
    fn assert_xref<'a, 'src>(node: &'a InlineNode<'src>) -> &'a Ref<'src> {
        match node {
            InlineNode::Ref(reference) if reference.variant == RefVariant::Xref => reference,

            other => panic!("expected an xref Ref, got {other:?}"),
        }
    }

    #[test]
    fn fold_matches_the_string_pipeline_through_xrefs() {
        // For each fixture, folding the single-pass tree (all five steps)
        // reproduces the string pipeline's output byte-for-byte. This is the
        // differential corpus (design §5.3) that pins the cross-reference
        // increment. Every fixture is a *verbatim*, same-document cross-reference
        // in either spelling – the boundary this increment claims (an
        // inter-document target, a document-as-a-whole reference, an
        // attribute-list text, and a shorthand crossing a special/span are
        // deferred and live in divergence tests below).
        let fixtures = [
            // No cross-reference despite macro-ish characters.
            "plain text without a reference",
            "an xref without a bracket xref:foo stays literal",
            // Macro form: bracketed reference text, and empty (bracketed
            // fallback).
            "xref:install[Installation]",
            "xref:install[]",
            "xref:sect-one[Section One]",
            // An explicit same-document reference (`#id`).
            "xref:#install[Install]",
            // An escaped `]` inside the text is unescaped.
            "xref:foo[a\\]b]",
            // A macro embedded in surrounding flow, and next to other constructs.
            "See xref:install[the guide] for details.",
            "*bold* then xref:x[X] and _em_",
            "a copyright (C) then xref:x[X]",
            // Escapes: the macro stays literal, minus the backslash.
            "\\xref:install[Installation]",
            "\\xref:install[]",
            // A macro inside a rendered span (recognized inside the span body).
            "*see xref:x[X]*",
            "_xref:y[Y] in em_",
            // Shorthand form: bare id (bracketed fallback) and with reference
            // text, seen post-special-chars as `&lt;&lt;id&gt;&gt;`.
            "<<install>>",
            "<<install,Install Now>>",
            "<<sect-one,Section One>>",
            // The shorthand reads a dotted target as an id (unlike the macro).
            "<<a.b.c>>",
            // The id and reference text are each trimmed around the comma.
            "<< spaced , Trimmed Text >>",
            // A shorthand embedded in surrounding flow, and next to other
            // constructs; and both spellings together.
            "See <<install>> now.",
            "*bold* then <<x,X>> and _em_",
            "<<install>> and xref:install[Installation]",
            // Escapes: the shorthand stays literal, minus the backslash.
            "\\<<install>>",
            "\\<<install,Install Now>>",
            // A shorthand inside a rendered span (recognized inside the body).
            "*see <<x>>*",
            "_<<y,Y>> in em_",
        ];

        let renderer = HtmlSubstitutionRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_src(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden_xref(fixture),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn an_xref_macro_becomes_a_ref_node() {
        let nodes = build_src(Span::new("xref:install[Installation]"));

        assert_eq!(nodes.len(), 1);
        let reference = assert_xref(&nodes[0]);

        assert_eq!(reference.target.as_ref(), "install");
        assert_eq!(link_text_of(reference), "Installation");
        assert!(reference.roles.is_empty());
        assert_eq!(reference.window, None);
        assert_eq!(reference.resolved, None);

        // Its location covers the whole macro, the `[…]` included.
        assert_eq!(reference.location.data(), "xref:install[Installation]");
        assert_eq!(reference.location.line(), 1);
        assert_eq!(reference.location.col(), 1);
    }

    #[test]
    fn an_empty_xref_macro_has_no_children() {
        // An empty text yields no children; the fold reads that as "no text
        // provided" and renders the bracketed `[id]` fallback.
        let nodes = build_src(Span::new("xref:install[]"));

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "install");
        assert!(reference.children.is_empty());

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_xref("xref:install[]")
        );
    }

    #[test]
    fn an_xref_display_text_is_located_at_its_source() {
        // The display text's `Text` child locates at the bracketed text, not the
        // whole macro.
        let nodes = build_src(Span::new("xref:install[Installation]"));

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.children.len(), 1);
        assert_text(&reference.children[0], "Installation", 1, 14);
    }

    #[test]
    fn an_explicit_same_document_xref_stores_the_interpreted_id() {
        // `xref:#install[]` uses the explicit-`#` same-document form. The node's
        // `target` is the *interpreted* id (`install`), not the raw `#install`:
        // it is the value the renderer builds the `href` from and resolution
        // keys on, matching the string pipeline and the recorder tree (see the
        // `Ref::target` field docs). Storing `#install` would fold to
        // `href="##install"` and break parity.
        let nodes = build_src(Span::new("xref:#install[Install]"));

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "install");
        assert_eq!(link_text_of(reference), "Install");

        // The fold reproduces the string pipeline's `href="#install"` exactly.
        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});
        assert!(folded.contains(r##"href="#install""##), "folded: {folded}");
        assert_eq!(folded, golden_xref("xref:#install[Install]"));
    }

    #[test]
    fn an_xref_is_recognized_inside_a_span() {
        // A cross-reference can appear inside a rendered span; the transducer
        // descends into the span body and builds the node there.
        let nodes = build_src(Span::new("*see xref:x[X]*"));

        let children = assert_styled(&nodes[0], StyleVariant::Strong, SpanForm::Constrained);
        assert_eq!(children.len(), 2);
        assert_text(&children[0], "see ", 1, 2);

        let reference = assert_xref(&children[1]);
        assert_eq!(reference.target.as_ref(), "x");
        assert_eq!(link_text_of(reference), "X");
    }

    #[test]
    fn an_escaped_xref_stays_literal() {
        // `\xref:…` drops the backslash and keeps the macro as literal text – no
        // reference node – exactly as the string replacer's escape branch does.
        let source = "\\xref:install[Installation]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an escaped xref must not produce a reference node: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_xref(source)
        );
    }

    #[test]
    fn an_xref_shorthand_becomes_a_ref_node() {
        // The `<<id,text>>` shorthand builds the same `Ref{Xref}` node the
        // `xref:` macro does, even though its `&lt;&lt;` / `&gt;&gt;` delimiters
        // are `CharRef`s: the node consumes them and slices its verbatim inner.
        let nodes = build_src(Span::new("<<install,Install Now>>"));

        assert_eq!(nodes.len(), 1);
        let reference = assert_xref(&nodes[0]);

        assert_eq!(reference.target.as_ref(), "install");
        assert_eq!(link_text_of(reference), "Install Now");
        assert!(reference.roles.is_empty());
        assert_eq!(reference.window, None);
        assert_eq!(reference.resolved, None);

        // Its location covers the whole shorthand, the `<<` / `>>` included.
        assert_eq!(reference.location.data(), "<<install,Install Now>>");
        assert_eq!(reference.location.line(), 1);
        assert_eq!(reference.location.col(), 1);
    }

    #[test]
    fn a_bare_xref_shorthand_has_no_children() {
        // A shorthand without a `, reference text` yields no children; the fold
        // reads that as "no text provided" and renders the bracketed `[id]`
        // fallback, exactly as the empty `xref:id[]` macro does.
        let nodes = build_src(Span::new("<<install>>"));

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "install");
        assert!(reference.children.is_empty());

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_xref("<<install>>")
        );
    }

    #[test]
    fn an_xref_shorthand_display_text_is_located_at_its_trimmed_source() {
        // The reference text's `Text` child locates at the *trimmed* text within
        // the shorthand, not at the whole shorthand and not including the
        // surrounding whitespace the string replacer trims.
        let nodes = build_src(Span::new("<<install, Install Now >>"));

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "install");
        assert_eq!(reference.children.len(), 1);

        // `<<install, ` is 11 characters, so the text starts at column 12.
        assert_text(&reference.children[0], "Install Now", 1, 12);
    }

    #[test]
    fn an_xref_shorthand_is_recognized_inside_a_span() {
        // A shorthand can appear inside a rendered span; the transducer descends
        // into the span body and builds the node there.
        let nodes = build_src(Span::new("*see <<x,X>>*"));

        let children = assert_styled(&nodes[0], StyleVariant::Strong, SpanForm::Constrained);
        assert_eq!(children.len(), 2);
        assert_text(&children[0], "see ", 1, 2);

        let reference = assert_xref(&children[1]);
        assert_eq!(reference.target.as_ref(), "x");
        assert_eq!(link_text_of(reference), "X");
    }

    #[test]
    fn an_escaped_xref_shorthand_stays_literal() {
        // `\<<id>>` drops the backslash and keeps the shorthand as literal text –
        // no reference node – exactly as the string replacer's escape branch
        // does. Its delimiters are non-verbatim `CharRef`s, so this also exercises
        // the escape path that does not require a verbatim inner.
        let source = "\\<<install,Install Now>>";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an escaped shorthand must not produce a reference node: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_xref(source)
        );
    }

    #[test]
    fn an_inter_document_xref_shorthand_is_a_documented_divergence() {
        // An inter-document shorthand target (`other#frag`) renders through a
        // *derived* destination the `Ref` node does not carry, so the builder
        // defers it (left literal), exactly as it defers the inter-document
        // `xref:` macro form.
        let source = "<<other#frag,Elsewhere>>";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an inter-document shorthand must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* build a reference here.
        assert!(golden_xref(source).contains("<a href"));
    }

    #[test]
    fn an_empty_xref_shorthand_is_a_documented_divergence() {
        // `<<>>` names the document as a whole: an empty id that resolves through
        // a *derived* destination the `Ref` node does not carry, so the builder
        // defers it (left literal), exactly as it defers the empty `xref:#[]`
        // macro form.
        let source = "<<>>";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an empty shorthand must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* build a reference here.
        assert!(golden_xref(source).contains("<a href"));
    }

    #[test]
    fn an_xref_shorthand_with_empty_text_is_a_documented_divergence() {
        // `<<id,>>` records a *present-but-empty* reference text: the string
        // replacer renders an empty `<a href="#id"></a>`, whereas an empty child
        // vector is indistinguishable from "no text provided" (the `[id]`
        // fallback). The builder cannot represent the distinction, so it defers
        // the whole shorthand (left literal).
        let source = "<<install,>>";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "a shorthand with an empty text must be left unrecognized: {nodes:?}"
        );

        // The string pipeline builds an anchor with an empty body, which the
        // bracketed fallback the builder would produce does not match.
        assert!(golden_xref(source).contains(r##"href="#install">"##));
        assert!(!golden_xref(source).contains("[install]"));
    }

    #[test]
    fn an_xref_shorthand_over_a_rendered_span_is_a_documented_divergence() {
        // A shorthand whose reference text is a rendered span (`<<x,*bold*>>`) has
        // a non-verbatim inner – the span is opaque – so the builder cannot slice
        // its text from `'src` and leaves the shorthand unrecognized, exactly as
        // it defers a macro crossing a rendered span.
        let source = "<<x,*bold*>>";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "a shorthand crossing a rendered span must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* build a reference here.
        assert!(golden_xref(source).contains("<a href"));
    }

    #[test]
    fn an_inter_document_xref_is_a_documented_divergence() {
        // An inter-document target (`other.adoc#frag`) renders through a
        // *derived* destination the `Ref` node does not carry, so the single-pass
        // builder defers it to a later increment (left literal).
        let source = "xref:other.adoc#frag[Elsewhere]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an inter-document xref must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* build a reference here.
        assert!(golden_xref(source).contains("<a href"));
    }

    #[test]
    fn an_xref_over_a_special_character_is_a_documented_divergence() {
        // A cross-reference whose text contains `<` is matched by the string
        // pipeline over the *escaped* text (`xref:foo[a&lt;b]`). A self-describing
        // node cannot carry that escaped text as an `'src` slice, so the
        // single-pass builder leaves such a macro *unrecognized* for a later
        // increment, exactly as the image and auto-link increments defer a macro
        // crossing a special character.
        let source = "xref:foo[a<b]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an xref crossing an escaped special must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* build a reference here.
        assert!(golden_xref(source).contains("<a href"));
    }

    #[test]
    fn an_xref_attribute_list_is_a_documented_divergence() {
        // An `xref:` text carrying an `=` splits into an attribute list (here a
        // role), which the string replacer parses from a newline-normalized copy
        // of the text – not from `'src`. The builder cannot carry that as an
        // `Attrlist<'src>` yet, so it defers the whole macro (left literal).
        let source = "xref:install[Installation,role=hl]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an attribute-list-in-text xref must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, applies the role.
        assert!(golden_xref(source).contains(r#"class="hl""#));
    }

    #[test]
    fn an_empty_same_document_xref_is_a_documented_divergence() {
        // `xref:#[]` names the document as a whole: an empty same-document id
        // that resolves through a *derived* destination (`this_document_
        // reference`) the `Ref` node does not carry, so the builder defers it to
        // a later increment (left literal) exactly as it defers an inter-document
        // target.
        let source = "xref:#[]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an empty same-document xref must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* build a reference here.
        assert!(golden_xref(source).contains("<a href"));
    }

    // ─── Macros (inline anchors) ──────────────────────────────────────────

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

    // ─── Fold robustness on hand-built nodes ──────────────────────────────

    #[test]
    fn fold_folds_a_hand_built_image_without_an_attribute_list() {
        // The node types are public, so a consumer may hand-build an
        // [`Image`](InlineNode::Image) directly rather than through the macro
        // step – with no [`Attrlist`]. The fold handles that by folding through
        // an empty attribute list sliced from the node's own location, so a
        // hand-built image renders like the same macro would. (The macro step
        // always attaches an attribute list, so only a hand-built node reaches
        // this branch, exactly as [`fold_encodes_an_unknown_replacement_value_
        // per_character`] exercises a hand-built replacement.)
        let location = Span::new("image:sunset.jpg[Sunset]");

        let hand_built = InlineNode::Image(Image {
            is_icon: false,
            target: CowStr::from("sunset.jpg"),
            alt: Some(CowStr::from("Sunset")),
            width: None,
            height: None,
            attrs: None,
            location,
        });

        // The macro-built equivalent (which carries an attribute list) is the
        // oracle: the two must fold identically.
        let renderer = HtmlSubstitutionRenderer {};
        let macro_built = fold_html(&build_src(location), &renderer);

        assert_eq!(fold_html(&[hand_built], &renderer), macro_built);
    }

    #[test]
    fn apply_macros_recognizes_a_macro_inside_reference_children() {
        // The macros step descends into a [`Ref`](InlineNode::Ref)'s display
        // children before matching at its own level (mirroring the string
        // pipeline's whole-string pass). The builder never leaves an
        // unrecognized macro inside freshly-built reference children, so this
        // descent is exercised directly: a hand-built `Ref` whose child text is
        // an `image:` macro has that macro recognized inside it.
        let root = Span::new("image:x.png[X]");

        let reference = InlineNode::Ref(Ref {
            variant: RefVariant::Link,
            target: CowStr::from("https://example.org"),
            children: vec![InlineNode::Text {
                value: CowStr::from(root.data()),
                location: root,
            }],
            roles: vec![],
            window: None,
            resolved: None,
            location: root,
        });

        let out = apply_macros(vec![reference], root, &Parser::default());

        // The reference survives, and its single child is now the recognized
        // image node.
        assert_eq!(out.len(), 1);

        match &out[0] {
            InlineNode::Ref(reference) => {
                assert_eq!(reference.children.len(), 1);

                assert!(
                    matches!(reference.children[0], InlineNode::Image(_)),
                    "expected the child macro to be recognized, got {:?}",
                    reference.children[0]
                );
            }

            other => panic!("expected the Ref to survive, got {other:?}"),
        }
    }

    // ─── Macros (index terms) ─────────────────────────────────────────────

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
        // A level that is *only* concealed shorthand terms accumulates no output
        // and ends in a look-ahead retry, so the string replacer returns
        // `Cow::Borrowed` and leaves it literal. The builder mirrors that: the
        // terms are left unrecognized (no `IndexTerm` node), byte-for-byte.
        for source in ["(((coffee)))", "(((a)))(((b)))", "indexterm:[x](((y)))"] {
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
