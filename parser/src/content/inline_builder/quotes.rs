//! The quoted-text substitution step.

use std::{borrow::Cow, ops::Range};

use super::{
    macros::image::{range_has_no_opaque_piece, range_is_verbatim},
    special_chars::Masked,
};
use crate::{
    HasSpan, Parser, Span,
    attributes::{Attrlist, AttrlistContext},
    content::{QuoteSub, maybe_has_replacements, quote_subs},
    inlines::{CharRef, InlineNode, RawForm, RawOrigin, SpanForm, StyleVariant, Styled},
    parser::{HtmlInlineRenderer, InlineRenderer, QuoteScope, QuoteType},
    strings::CowStr,
};

/// A single opaque codepoint standing in for a whole [`Styled`] span (produced
/// by an earlier sub) while a later sub matches at that span's level. Like the
/// `<strong>…</strong>` markup a completed span folds to, it is a single
/// non-word, non-space boundary character that a quote pattern treats as
/// opaque content.
///
/// The codepoint is an **ASCII** control character (U+0010 DATA LINK ESCAPE),
/// not a Private Use Area one, and the choice is load-bearing for throughput:
/// several shared patterns carry Unicode word-boundary assertions
/// (`\b{start-half}` / `\b{end-half}`), which the regex crate's lazy DFA
/// supports only while the haystack is pure ASCII — its first non-ASCII byte
/// makes the DFA quit and the whole search rerun on an engine that is more
/// than an order of magnitude slower. A PUA placeholder (three UTF-8 bytes)
/// put that byte in every level that contained any recognized construct, which
/// is exactly the text these patterns sweep hardest. Semantically the two are
/// interchangeable — the placeholder is located through each level's
/// [`Piece`] table, never by scanning for it, and both codepoints are
/// non-word, non-space, and match no pattern's own character classes — so the
/// ASCII pick costs nothing and keeps the sweep on the fast engine.
pub(super) const SPAN_PLACEHOLDER: char = '\u{10}';

/// The characters an enclosing construct's own rendering presents to the level
/// nested inside it — the bytes immediately before and after that level's own
/// text once the enclosing markup is rendered.
///
/// A quote pattern's boundary classes (`^`, `$`, and a `(^|[^\w&;:}])`-style
/// group) need to read the same characters a whole-content match would see at
/// that position, but a transducer matches one level at a time, where the same
/// position is instead the very start (or end) of that level's own haystack.
/// The two agree for a construct written at the content's own top level and
/// diverge for one written *inside* a span, where the enclosing rendering is
/// the span's opening `<strong>` (or `&#8220;`) rather than a start anchor:
/// ``` `"``end points``"` ``` renders
/// ``` `&#8220;`end points`&#8221;` ``` there — the inner backticks stay
/// literal, because the `;` ending the entity fails the monospace sub's own
/// boundary class — where a level matched in isolation sees `^` and wraps them
/// in a `<code>` span.
///
/// A `LevelContext` restores those two characters: the level's match string is
/// wrapped in them before a pattern is run over it, and every resulting offset
/// is mapped back with [`LevelContext::unshift`], so only *recognition*
/// changes and every range a caller goes on to slice stays in the level's own
/// coordinates.
///
/// [`post_replacements`](super::post_replacements) already reasoned this way
/// for its own `$` — a nested level "is always followed by its own closing
/// markup … so a ` +` ending a span is not at a line end there" — with a
/// boolean; this generalizes that one step's flag into the pair of characters
/// every step's patterns can read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct LevelContext {
    /// The last character of the enclosing construct's *opening* markup, or
    /// `None` at the content's own top level (where a pattern's `^` is
    /// exactly right).
    before: Option<char>,

    /// The first character of the enclosing construct's *closing* markup, or
    /// `None` at the content's own top level.
    after: Option<char>,
}

impl LevelContext {
    /// The context a [`Ref`](InlineNode::Ref) node presents to its own display
    /// children: every reference the fold renders — a link, and a
    /// cross-reference in each of its resolved and unresolved forms — wraps
    /// them in an `<a …>…</a>` element.
    ///
    /// Only an order that runs `macros` *before* a step that descends into a
    /// reference's children reaches this at all (the built-in orders run it
    /// last but one, ahead of `post_replacements` alone). A deferred
    /// cross-reference — one whose target resolution has not yet filled the
    /// node's rendered text in — presents its own placeholder characters at
    /// that moment rather than the element's, which read the same to every
    /// boundary class in play: both are non-word, and neither is one of the
    /// `&;:}` a constrained quote excludes nor the space or line end a spaced
    /// em dash requires.
    pub(super) const INSIDE_REF: Self = Self {
        before: Some('>'),
        after: Some('<'),
    };
    /// The content's own top level: nothing encloses it, so a pattern's `^`
    /// and `$` anchor exactly where they should.
    pub(super) const ROOT: Self = Self {
        before: None,
        after: None,
    };

    /// The context a [`Styled`] span presents to its own children, given the
    /// context that span itself sits in.
    ///
    /// A span the built-in backend renders with **no markup of its own** — an
    /// unquoted span that ends up carrying neither a role nor an id, whose
    /// rendering is its body and nothing else — is transparent, so its
    /// children see whatever the span itself sees. That is right whenever the
    /// span is all its level holds; [`child_contexts`](Self::child_contexts)
    /// is the same answer sharpened by what stands *beside* it, for the two
    /// steps that can take it.
    pub(super) fn inside_styled(styled: &Styled<'_>, enclosing: Self) -> Self {
        match styled_boundaries(styled) {
            Some((before, after)) => Self {
                before: Some(before),
                after: Some(after),
            },

            None => enclosing,
        }
    }

    /// The context each node in `nodes` presents to its own children, one
    /// entry per node, given the context the level itself sits in.
    ///
    /// A [`Ref`](InlineNode::Ref) always presents
    /// [`INSIDE_REF`](Self::INSIDE_REF) and a [`Styled`] span whatever
    /// [`inside_styled`](Self::inside_styled) says; every other node kind has
    /// no children to match, and its entry — the enclosing context, unchanged
    /// — is never read.
    ///
    /// # A transparent span reads its siblings
    ///
    /// A **transparent** span wraps its children in nothing, so what they read
    /// is not the enclosing construct's markup but whatever stands *beside the
    /// span itself* once this level is rendered. Inheriting the
    /// enclosing context — what [`inside_styled`](Self::inside_styled) does on
    /// its own — is right only while the span is all its level holds; the
    /// moment a sibling precedes it, the haystack shows what that sibling
    /// rendered (`*x [width=10]#doc@example.org#*` presents the space `x `
    /// ends with, where the enclosing `<strong>`'s own `>` is one of the bare
    /// e-mail pattern's mismatch characters).
    ///
    /// The level's own **match string** already spells out what every node
    /// kind presents — a text run's bytes, a [`CharRef`]'s entity, and, for an
    /// opaque node, the [`SPAN_PLACEHOLDER`] wrapped in whatever
    /// [`styled_sibling_boundaries`] can say — so the character beside the
    /// span's own [`Piece`] is read straight out of it rather than recomputed
    /// per node kind, and the two cannot drift. Building that match string is
    /// worth it only when such a span is there, so it is built on demand:
    /// every other level answers from [`styled_boundaries`] alone, exactly as
    /// before.
    ///
    /// # Only the opening character
    ///
    /// A sibling supplies the **opening** character alone, for the reason
    /// [`shift`](Self::shift) gives one level in and then some. A pattern's
    /// *boundary* class reads one character and — where it consumes one, as a
    /// constrained quote's `(^|[^\w&;:}])` does — the replacer writes it back,
    /// which [`unshift`](Self::unshift)'s own clip reproduces by leaving the
    /// character with the sibling that owns it. A *delimiter* or a greedy body
    /// class swallows it instead, and what the replacer swallows it **deletes**
    /// — a character another level's node owns, which this level's rebuild
    /// cannot delete. So the closing half is dropped rather than
    /// half-supplied, leaving a construct whose own closing delimiter fell
    /// beside the span (`x[width=10]##d #c###`) exactly as divergent as it
    /// already was, never newly wrong.
    ///
    /// The same reasoning keeps the
    /// [`character replacements`](super::char_replacements) step off this
    /// entirely: its one boundary-reading rule is the spaced em dash, whose
    /// replacement *consumes* the spaces it matches on both sides, so even an
    /// opening character a sibling owns would be deleted here and written
    /// twice there. That step goes on inheriting through
    /// [`inside_styled`](Self::inside_styled), and
    /// `a_replacement_beside_a_transparent_span_is_a_documented_divergence`
    /// keeps its shape.
    ///
    /// # What a neighbour can be read as
    ///
    /// Because the answer is read out of the match string, it is exactly as
    /// good as what [`build_match_string`] could write there — so `masked` is
    /// passed straight through. A caller holding the extraction pass's
    /// identity gets the `>` a tag-rendered neighbour's own closing markup
    /// ends with, or the last character of a **transparent** neighbour's own
    /// body ([`transparent_sibling_boundaries`]); one that does not gets the
    /// bare [`SPAN_PLACEHOLDER`], which [`preceding_character`] reports as
    /// *nothing* rather than as a character. Both are right for what their
    /// caller knows, and the second is what this function answered before the
    /// identity reached it at all.
    pub(super) fn child_contexts(
        nodes: &[InlineNode<'_>],
        enclosing: Self,
        masked: Masked<'_>,
    ) -> Vec<Self> {
        let mut has_transparent = false;

        let mut contexts: Vec<Self> = nodes
            .iter()
            .map(|node| match node {
                InlineNode::Styled(styled) => {
                    has_transparent |= styled_boundaries(styled).is_none();
                    Self::inside_styled(styled, enclosing)
                }

                InlineNode::Ref(_) => Self::INSIDE_REF,

                _ => enclosing,
            })
            .collect();

        if !has_transparent {
            return contexts;
        }

        let (s, pieces) = build_match_string(nodes, masked);

        // `build_match_string` contributes exactly one [`Piece`] per node, in
        // order, so the three sequences line up and no lookup is needed.
        for ((context, piece), node) in contexts.iter_mut().zip(&pieces).zip(nodes) {
            let InlineNode::Styled(styled) = node else {
                continue;
            };

            if styled_boundaries(styled).is_some() {
                continue;
            }

            // Read from before the span's **own** opening character. A
            // transparent span presents its body's first character to whatever
            // precedes it ([`transparent_sibling_boundaries`]), and that
            // character sits between the neighbour and this piece — so the
            // lookup steps back over it to reach what the span's *children*
            // read, which is what precedes the span itself.
            let own = styled_sibling_boundaries(styled, masked)
                .0
                .map_or(0, char::len_utf8);

            context.before =
                preceding_character(&s, piece.s_start.saturating_sub(own)).or(context.before);
        }

        contexts
    }

    /// The haystack a pattern should be matched against for a level whose own
    /// match string is `s`, together with the byte length of the prefix
    /// [`unshift`](Self::unshift) removes again.
    ///
    /// At the top level this borrows `s` untouched, so the overwhelmingly
    /// common case allocates nothing.
    ///
    /// The two halves are applied independently, because a level can have an
    /// opening character without a closing one: an enclosing construct always
    /// presents both of its own or neither, but a **transparent** span's
    /// opening character comes from a *sibling*
    /// ([`child_contexts`](Self::child_contexts)) while its closing one stays
    /// the enclosing construct's — so `x[width=10]###c# d##` at the content's
    /// own top level carries the `x` its sibling ends with and still anchors
    /// `$` where the content itself ends. The reverse never arises, which is
    /// what lets the opening character gate the wrap.
    pub(super) fn haystack<'a>(&self, s: &'a str) -> (Cow<'a, str>, usize) {
        let Some(before) = self.before else {
            return (Cow::Borrowed(s), 0);
        };

        let prefix = before.len_utf8();

        let mut hay =
            String::with_capacity(s.len() + prefix + self.after.map_or(0, char::len_utf8));

        hay.push(before);
        hay.push_str(s);

        if let Some(after) = self.after {
            hay.push(after);
        }

        (Cow::Owned(hay), prefix)
    }

    /// [`haystack`](Self::haystack)'s counterpart for a step that maps no
    /// offsets back: it moves the level's own **pieces** into the haystack's
    /// coordinates instead, so every offset a caller goes on to use — a match
    /// range, a gate, a slice — is already in the one coordinate system the
    /// haystack itself is in.
    ///
    /// This is what the [`macros`](super::macros) step takes, where
    /// [`unshift`](Self::unshift) would not do: a macro family does not merely
    /// *report* ranges, it reads the match string's own bytes through
    /// [`Piece`]s ([`emit_range`], [`source_slice`],
    /// [`range_has_no_opaque_piece`]), so haystack offsets and level offsets
    /// cannot be allowed to coexist. Shifting the pieces removes the second
    /// coordinate system rather than translating between them.
    ///
    /// The context character belongs to no piece — it is the *enclosing*
    /// construct's, not this level's — so a range reaching it contributes
    /// nothing: [`emit_range`] finds no piece overlapping it (which is exactly
    /// [`unshift`](Self::unshift)'s own clip: a boundary prefix is text the
    /// enclosing span already carries), and every gate skips it as
    /// non-overlapping.
    ///
    /// # Only the opening character
    ///
    /// Unlike [`haystack`](Self::haystack), this applies the **opening** half
    /// alone, and the asymmetry is the point. A pattern's *boundary class*
    /// reads exactly one character, so one is all a level needs to answer it:
    /// `<strong>` ends in `>` and `&#8220;` in `;`, which is precisely what
    /// the auto-link's own `( ^ | [\ \t\p{Zs}] | [>\(\)\[\];"'] )` prefix
    /// group and the bare e-mail's `([\\>:/]?)` mismatch-prefix group inspect
    /// there.
    ///
    /// A macro *body* class, by contrast, consumes greedily rather than
    /// reading one character: the auto-link's own bare-URL body
    /// (`[^\s\[\]<]*`) excludes a `<` but not an `&`, so where the string
    /// pipeline's haystack presents a whole closing `&#8221;` for it to
    /// swallow, a level carrying one `&` would build a *different* wrong
    /// target rather than the same one. The closing character is therefore
    /// dropped rather than half-supplied — leaving that shape exactly as
    /// divergent as it already is (see
    /// `a_bare_url_at_an_entity_rendered_spans_closing_edge_is_a_documented_divergence`),
    /// never newly wrong.
    pub(super) fn shift(self, mut s: String, mut pieces: Vec<Piece>) -> (String, Vec<Piece>) {
        let Some(before) = self.before else {
            return (s, pieces);
        };

        // Wrapped in place rather than into a second string: the level owns
        // its match string here, so this reuses that allocation.
        s.insert(0, before);

        for piece in &mut pieces {
            piece.s_start += before.len_utf8();
        }

        (s, pieces)
    }

    /// Maps a range of the [`haystack`](Self::haystack) back into the level's
    /// own match string, clamping it to the level.
    ///
    /// A pattern may legitimately *consume* a context character — a
    /// constrained sub's boundary group is exactly that — but the character is
    /// not part of the level and has no node behind it, so it is clipped away
    /// rather than emitted: the boundary group is text the sub keeps anyway,
    /// and here the enclosing span already carries it.
    pub(super) fn unshift(prefix: usize, len: usize, range: Range<usize>) -> Range<usize> {
        let start = range.start.saturating_sub(prefix).min(len);
        let end = range.end.saturating_sub(prefix).min(len);

        start..end
    }
}

/// The two characters the **built-in** HTML backend places immediately around
/// a [`Styled`] span's body, or `None` when it wraps the body in nothing at
/// all.
///
/// Reading the *built-in* backend here is the same deliberate compromise
/// [`special_entity`] and [`replacement_entity`] already make: a custom
/// backend changes what the fold *emits*, not the recognition the AsciiDoc
/// patterns were written against, so recognition stays backend-independent.
/// Every variant but the unquoted one is decided by its own rendering shape —
/// a tag (`<strong>…</strong>`) or an entity pair (`&#8220;…&#8221;`) — rather
/// than by rendering it, which `styled_boundaries_match_the_built_in_renderer`
/// pins against the renderer itself; an unquoted span is the one whose shape
/// depends on what its attribute list resolves to, so it is rendered.
fn styled_boundaries(styled: &Styled<'_>) -> Option<(char, char)> {
    match styled.variant {
        // `&#8220;…&#8221;` / `&#8216;…&#8217;`.
        StyleVariant::DoubleQuote | StyleVariant::SingleQuote => Some((';', '&')),

        // A `<span …>` when the attribute list resolves to a role or an id,
        // and the body alone when it does not.
        StyleVariant::Unquoted => probe_styled_boundaries(styled),

        // Every other variant wraps the body in an HTML tag.
        _ => Some(('>', '<')),
    }
}

/// The two characters a [`Styled`] span's own rendering presents to its
/// **siblings** — the first character of its opening markup and the last of
/// its closing markup — or `None` when this module cannot say what a sibling
/// reads there.
///
/// This is [`styled_boundaries`]'s mirror image, one level out. That function
/// answers what a span presents to the level *inside* it (the last character
/// of its opening markup and the first of its closing one, carried by a
/// [`LevelContext`]); this one answers what the same span presents to the
/// nodes *beside* it at its own level, where
/// [`build_match_string`] otherwise stands the whole span in as one opaque
/// [`SPAN_PLACEHOLDER`] belonging to no boundary class at all. A rendered
/// span's own markup is what a following/preceding construct actually reads
/// there — a following one reads `>` where the span rendered a tag and `;`
/// where it rendered a smart quote's `&#8221;`, and a preceding one reads `<`
/// or `&`.
///
/// # An extraction-pass wrapper is not a rendered span
///
/// A [`Styled`] node reaching [`build_match_string`] is not necessarily one
/// whose markup has actually been rendered: the passthrough-extraction pass
/// builds one of its own for an attribute-list-prefixed passthrough
/// (`[quotes]++text++`, `` [x-]`text` ``), standing in as a **placeholder**
/// for content masked out and restored later, rather than as markup, for
/// every step this module runs. A sibling reads that placeholder's own
/// characters, which are exactly what the bare [`SPAN_PLACEHOLDER`] already
/// reads as to every class in play (both are non-word, in none of `&;:}`,
/// `[>\(\)\[\];"']`, or `[\\>:/]`), so such a wrapper keeps the bare
/// placeholder — not as an approximation, but because that is the right
/// answer.
///
/// Telling one apart from a genuinely rendered span needs the *identity*
/// [`masked_locations`](super::special_chars::masked_locations) collects
/// before any step runs, which `masked` carries here. Where a caller does not
/// hold it ([`Masked::UNKNOWN`]), no tag-rendered span is classified — the
/// answer this function gave before the identity reached it.
///
/// The two **entity**-rendered variants need no such check and are answered
/// either way: the extraction pass builds neither smart-quote variant, so a
/// [`DoubleQuote`](StyleVariant::DoubleQuote) or
/// [`SingleQuote`](StyleVariant::SingleQuote) node can only have come from the
/// quotes step, which really does render `&#8220;…&#8221;`.
///
/// # A transparent span presents its own body
///
/// A span the built-in backend renders with **no markup of its own** — an
/// unquoted span whose attribute list resolves to neither a role nor an id —
/// wraps its body in nothing, so what a sibling reads there is that *body*:
/// rendering `[width=10]##x ##https://example.org` holds `x ` where the span
/// stands as one opaque placeholder, and links on the space the body ends
/// with.
/// [`transparent_sibling_boundaries`] answers that pair from the span's own
/// children, and the identity gates it for exactly the reason it gates a tag:
/// `[width=10]++x ++` is an extraction wrapper that renders its body and
/// nothing else, and what a sibling reads *there* is the same placeholder a
/// bare, unclassified span already reads as.
///
/// # Two halves, answered independently
///
/// The pair is two [`Option`]s rather than one option of a pair, because a
/// transparent span's halves are read from opposite ends of its body and
/// either can be a character this module cannot describe while the other is
/// not. Every markup-rendering variant goes on answering both or neither.
fn styled_sibling_boundaries(
    styled: &Styled<'_>,
    masked: Masked<'_>,
) -> (Option<char>, Option<char>) {
    match styled.variant {
        // `&#8220;…&#8221;` / `&#8216;…&#8217;`, whichever step built them —
        // and only the quotes step can. See this function's own scope note.
        StyleVariant::DoubleQuote | StyleVariant::SingleQuote => (Some('&'), Some(';')),

        // Every other variant wraps its body in a tag, which is also the shape
        // the passthrough-extraction pass's own wrapper takes — so answering
        // one turns on whether this node is such a wrapper, which only the
        // identity `masked` carries can say. A wrapper that renders its body
        // and nothing else is covered by the same guard, for the same reason.
        _ if !masked.renders_to_its_siblings(styled.location) => (None, None),

        // The one variant whose rendering shape its attribute list decides:
        // a `<span …>` when that resolves to a role or an id, and the body
        // alone when it does not — which presents the body's own two outer
        // characters.
        StyleVariant::Unquoted => match probe_styled_sibling_boundaries(styled) {
            Some((before, after)) => (Some(before), Some(after)),

            None => transparent_sibling_boundaries(&styled.children, masked),
        },

        _ => (Some('<'), Some('>')),
    }
}

/// [`styled_sibling_boundaries`] for a **transparent** span — one whose
/// rendering is its body and nothing else — whose two outer characters are its
/// children's rather than any markup of its own.
///
/// Those characters are already spelled out in the children's own **match
/// string**, which is the one place every node kind's presented bytes are
/// written down (a text run's, a [`CharRef`]'s entity, and a nested opaque
/// node's placeholder wrapped in whatever this function's own caller can say)
/// — the same reason [`LevelContext::child_contexts`] reads a level's siblings
/// out of it rather than recomputing them per node kind, and the reason the two
/// cannot drift. The recursion terminates on the tree: a transparent span
/// nested inside this one answers from *its* children.
///
/// A [`SPAN_PLACEHOLDER`] at either edge reports **nothing** rather than a
/// character, the line [`preceding_character`] draws for the same reason: it is
/// what [`build_match_string`] writes for a node this module cannot describe,
/// so reporting it would manufacture an answer rather than sharpen one.
fn transparent_sibling_boundaries(
    children: &[InlineNode<'_>],
    masked: Masked<'_>,
) -> (Option<char>, Option<char>) {
    let (s, _) = build_match_string(children, masked);

    let mut chars = s.chars();

    let before = chars.next().filter(|ch| *ch != SPAN_PLACEHOLDER);

    // A one-character body presents that same character on both sides: `next`
    // has already consumed it, so the closing half falls back to what the
    // opening one read.
    let after = chars
        .next_back()
        .filter(|ch| *ch != SPAN_PLACEHOLDER)
        .or(before);

    (before, after)
}

/// [`styled_sibling_boundaries`] for a span whose rendering shape is not
/// decided by its variant alone: renders it through the built-in backend
/// around a probe body and reads the characters that land at the two outer
/// edges. A span that wraps its body in nothing has neither.
fn probe_styled_sibling_boundaries(styled: &Styled<'_>) -> Option<(char, char)> {
    let (before, after) = probe_styled_boundaries_markup(styled);

    before.chars().next().zip(after.chars().next_back())
}

/// The character a level's own match string `s` holds immediately before byte
/// offset `at`, or `None` where this module cannot say what character is
/// there — because nothing precedes that offset, or because what does is
/// an unclassified opaque node.
///
/// This is how a **transparent** span's children learn what precedes the span
/// (see [`LevelContext::child_contexts`]): the match string is the one place
/// every node kind's presented bytes are already spelled out, so reading the
/// character out of it cannot drift from what a pattern matching at this level
/// would read there.
///
/// # A bare placeholder is not an answer
///
/// [`SPAN_PLACEHOLDER`] is what [`build_match_string`] writes for a node whose
/// rendering this module cannot describe — everything
/// [`styled_sibling_boundaries`] declines to classify, which with the
/// extraction pass's identity in hand is the pass's own wrapper (tag-rendered
/// or transparent alike) and, without it, every span but the two
/// entity-rendered ones. Reporting it here would *manufacture* a
/// character where the level previously read its own start anchor, which is a
/// different answer rather than a better one — and for a wrapper it would be
/// the wrong one, since a sibling there reads the wrapper's own placeholder
/// character, which the auto-link's own prefix group rejects exactly as `^`
/// is accepted.
/// So an unclassified neighbour reports nothing and the span goes on
/// inheriting, leaving that shape exactly as it already was — the same line
/// [`styled_sibling_boundaries`] draws, for the same reason.
fn preceding_character(s: &str, at: usize) -> Option<char> {
    // Every offset a caller passes is one this module itself pushed into `s`,
    // so the lookup always lands on a character boundary within it; the crate
    // forbids `unwrap`, so that is spelled `unwrap_or_default` rather than as a
    // branch no input reaches.
    s.get(..at)
        .unwrap_or_default()
        .chars()
        .next_back()
        .filter(|ch| *ch != SPAN_PLACEHOLDER)
}

/// [`styled_boundaries`] for a span whose rendering shape is not decided by
/// its variant alone: renders it through the built-in backend around a probe
/// body and reads the characters that land beside it.
fn probe_styled_boundaries(styled: &Styled<'_>) -> Option<(char, char)> {
    let (before, after) = probe_styled_boundaries_markup(styled);

    before.chars().next_back().zip(after.chars().next())
}

/// The **opening** and **closing** markup the built-in backend places around a
/// [`Styled`] span's body, recovered by rendering the span around a probe body
/// and splitting on it.
///
/// Both boundary functions read this one pair from opposite ends:
/// [`styled_boundaries`] takes the last character of the opening run and the
/// first of the closing one (what the level *inside* the span sees), and
/// [`styled_sibling_boundaries`] takes the first character of the opening run
/// and the last of the closing one (what a *sibling* sees).
fn probe_styled_boundaries_markup(styled: &Styled<'_>) -> (String, String) {
    // A NUL never appears in a rendered attribute value, so the probe is
    // unambiguous.
    const PROBE: &str = "\u{0}";

    let scope = match styled.form {
        SpanForm::Constrained => QuoteScope::Constrained,
        SpanForm::Unconstrained => QuoteScope::Unconstrained,
    };

    let mut rendered = String::new();

    HtmlInlineRenderer {}.render_styled(
        quote_type_of(styled.variant),
        scope,
        &styled.attrs,
        styled.id.as_ref().map(|id| id.to_string()),
        PROBE,
        &mut rendered,
    );

    // The renderer always emits the body, so the probe is always there; the
    // `unwrap_or_default` spells that lookup without a branch no input reaches,
    // and an absent probe would read as "no markup of its own" — the same
    // answer a span that wraps its body in nothing already gives.
    let (before, after) = rendered.split_once(PROBE).unwrap_or_default();

    (before.to_string(), after.to_string())
}

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
        nodes = apply_quote_sub(sub, nodes, root, parser, LevelContext::ROOT);
    }

    nodes
}

/// Applies one [`QuoteSub`] to `nodes`, first descending into the [`Styled`]
/// spans earlier subs created (so this sub can match *inside* them — the
/// nesting case), then matching and wrapping at this level.
///
/// `ctx` is the boundary context this level sits in (see [`LevelContext`]);
/// a span's own children are matched in the context that span's rendering
/// presents — or, for a span that renders no markup of its own, the one its
/// **siblings** present ([`LevelContext::child_contexts`]) — which is what
/// keeps a sub matching *inside* an earlier sub's span reading the same
/// characters a whole-content match would see there.
fn apply_quote_sub<'src>(
    sub: &QuoteSub,
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
    ctx: LevelContext,
) -> Vec<InlineNode<'src>> {
    // Recurse into the spans produced by earlier subs *before* matching at this
    // level. A span this sub itself creates below is therefore never revisited
    // by the same sub (a sub runs once per level). A level
    // with no such span — the common leaf-only case, visited once per sub —
    // has nothing to descend into, so it skips the context derivation and the
    // rebuild of its node vector entirely.
    let nodes = if nodes
        .iter()
        .any(|node| matches!(node, InlineNode::Styled(_)))
    {
        let contexts = LevelContext::child_contexts(&nodes, ctx, Masked::UNKNOWN);

        nodes
            .into_iter()
            .zip(contexts)
            .map(|(node, inner)| match node {
                InlineNode::Styled(mut styled) => {
                    styled.children = apply_quote_sub(sub, styled.children, root, parser, inner);
                    InlineNode::Styled(styled)
                }

                other => other,
            })
            .collect()
    } else {
        nodes
    };

    match_level(sub, nodes, root, parser, ctx)
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
    /// was synthesized (an attribute expansion, a `counter` directive, …) —
    /// its `value` differs from `location.data()`, so unlike a verbatim run
    /// its match-string bytes do **not** correspond one-to-one with source
    /// bytes. It contributes its `value` to the match string so a later step
    /// (character replacements, macros) can still recognize a
    /// construct inside it, but a match landing here has no honest `'src`
    /// slice: [`emit_range`] slices the node's *value* instead of its
    /// location, and [`s_to_src`] falls back to the piece's whole node span
    /// (its coarse fallback) rather than a proportional one.
    pub(super) synthesized: bool,
}

/// Matches `sub` once at this level, wrapping each accepted match in a
/// [`Styled`] span and leaving everything else in place.
fn match_level<'src>(
    sub: &QuoteSub,
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
    ctx: LevelContext,
) -> Vec<InlineNode<'src>> {
    // Cheap pre-filter, taken *before* the match string is materialized: if
    // this sub's own marker character(s) (see `sub_markers`) are not present
    // anywhere at this level, `sub` cannot possibly match, so skip the build
    // (and its allocations) entirely — see `level_may_match_sub`'s own doc
    // comment for why this is sub-specific rather than one gate shared by
    // every sub in `quote_subs`.
    if !level_may_match_sub(&nodes, sub) {
        return nodes;
    }

    let (s, pieces) = build_match_string(&nodes, Masked::UNKNOWN);

    // The pattern runs over the level wrapped in its enclosing construct's own
    // boundary characters, and every offset it reports is mapped back into the
    // level's own coordinates (see [`LevelContext`]).
    let (haystack, prefix) = ctx.haystack(&s);

    let matches: Vec<QuoteMatch> = find_matches(sub, &haystack)
        .into_iter()
        .map(|m| m.unshift(prefix, s.len()))
        .filter(|m| attrlist_is_readable(&nodes, &pieces, m))
        .collect();

    if matches.is_empty() {
        return nodes;
    }

    rebuild_level(&nodes, &pieces, &s, &matches, root, parser)
}

/// Reports whether a match's attribute list, if it has one, is readable from
/// the level's own match string without crossing an **opaque** piece (see
/// [`range_has_no_opaque_piece`]).
///
/// A match with no attribute list is always readable, which is the
/// overwhelmingly common case.
///
/// The one shape this rejects is an attribute list crossing a piece whose
/// bytes exist only at fold time or behind a placeholder — a rendered span
/// from an earlier sub, or a masked passthrough or STEM expression
/// (`[.a+++x+++b]#y#`). A passthrough's own placeholder holds no bytes to
/// splice an attribute list around: reading the passthrough's real text back
/// through the placeholder and splicing it into the parsed attribute list
/// would let a comma inside that text split the list, since the placeholder
/// is one atomic piece and cannot be read into partway. Leaving the whole
/// construct unrecognized — literal text, never a *wrong* node — is the same
/// boundary
/// every macro family draws, and puts this match back in the same position a
/// rejected look-ahead leaves one: out of the match list, so the surrounding
/// gap reproduces its original nodes and a later sub may still match there.
fn attrlist_is_readable(nodes: &[InlineNode<'_>], pieces: &[Piece], m: &QuoteMatch) -> bool {
    let QuoteMatchKind::Wrap {
        attrlist: Some(range),
        ..
    } = &m.kind
    else {
        return true;
    };

    range_has_no_opaque_piece(nodes, pieces, range)
}

/// Reconstructs the escaped match string for a level and the [`Piece`] map back
/// to its nodes.
///
/// A verbatim [`Text`](InlineNode::Text) run contributes its (special-free)
/// value; a [`CharRef`](InlineNode::CharRef) contributes its canonical entity
/// (a [`Special`](CharRef::Special)), the entity itself (a *restored*
/// [`Entity`](CharRef::Entity)), or the entity the built-in backend renders it
/// as (a typographic [`Replacement`](CharRef::Replacement), via
/// [`replacement_entity`]), so the boundary classes the quote patterns key off
/// (`&`, `;`) see exactly the bytes this content's own escaped rendering
/// presents — and so does a later step reading a value across one; a
/// *synthesized* `Text` run (an attribute expansion, a `counter` directive)
/// contributes its `value` too, so [`apply_character_replacements`]
/// (character replacements still runs over an expanded value) can
/// recognize a construct inside it, but is flagged
/// [`synthesized`](Piece::synthesized) since those bytes have no honest
/// `'src` counterpart; every other node contributes a single opaque
/// [`SPAN_PLACEHOLDER`], wrapped in the two characters its own rendering
/// presents to a sibling wherever [`styled_sibling_boundaries`] can say what
/// those are — which for a span rendering to its body and nothing else is that
/// **body**'s own two outer characters, since it wraps them in no markup.
///
/// `masked` is what the caller can say about which [`Styled`] nodes are the
/// passthrough-extraction pass's own wrappers, which is what that last
/// question turns on for every tag-rendered span; see [`Masked`].
///
/// [`apply_character_replacements`]: super::char_replacements::apply_character_replacements
/// Reports whether this level's match string *could* satisfy `predicate`,
/// without materializing the string.
///
/// This probes exactly the contributions [`build_match_string`] would write —
/// a `Text` run's value, a `CharRef`'s entity bytes ([`charref_entity`]), and
/// the up-to-two sibling-boundary characters wrapped around an opaque node's
/// placeholder (the placeholder itself matches no predicate a caller here
/// passes) — so its answer equals testing the built string with `predicate`,
/// *provided* `predicate` cannot itself be satisfied by two contributions
/// straddled together (single-character predicates never can; see
/// [`level_may_have_replacements`] for the one caller whose predicate can, and
/// how it stays conservative instead). Probing under [`Masked::UNKNOWN`]
/// mirrors the build [`match_level`] would do.
fn level_may_contain(nodes: &[InlineNode<'_>], predicate: impl Fn(&str) -> bool) -> bool {
    let mut buf = [0u8; 4];

    nodes.iter().any(|node| match node {
        InlineNode::Text { value, .. } => predicate(value),

        node => {
            if let Some(entity) = charref_entity(node) {
                predicate(entity)
            } else {
                let (before, after) = match node {
                    InlineNode::Styled(styled) => {
                        styled_sibling_boundaries(styled, Masked::UNKNOWN)
                    }
                    _ => (None, None),
                };

                before.is_some_and(|ch| predicate(ch.encode_utf8(&mut buf)))
                    || after.is_some_and(|ch| predicate(ch.encode_utf8(&mut buf)))
            }
        }
    })
}

/// The character(s) a [`QuoteSub`] of the given [`QuoteType`] requires
/// *somewhere* in its match to have any chance of matching — [`Strong`]'s
/// `*`, [`Monospaced`]'s `` ` ``, and so on — read straight off `QUOTE_SUBS`'s
/// own patterns. [`DoubleQuote`] and [`SingleQuote`] need *two*: their quote
/// character and the backtick immediately beside it.
///
/// [`Strong`]: QuoteType::Strong
/// [`Monospaced`]: QuoteType::Monospaced
/// [`DoubleQuote`]: QuoteType::DoubleQuote
/// [`SingleQuote`]: QuoteType::SingleQuote
fn sub_markers(type_: QuoteType) -> &'static [char] {
    match type_ {
        QuoteType::Strong => &['*'],
        QuoteType::DoubleQuote => &['"', '`'],
        QuoteType::SingleQuote => &['\'', '`'],
        QuoteType::Monospaced => &['`'],
        QuoteType::Emphasis => &['_'],
        QuoteType::Mark => &['#'],
        QuoteType::Superscript => &['^'],
        QuoteType::Subscript => &['~'],

        // None of these three is ever a `QuoteSub`'s own `type_` — `Unquoted`
        // is a rendering *variant* [`style_variant`] maps `Mark` to, never a
        // recognition rule of its own, and the two math types are recognized
        // by [`apply_stem`](super::stem_step::apply_stem) instead of by any
        // sub in [`quote_subs`]. An empty marker list is a safe (if
        // unhelpful) answer for a caller that somehow asked anyway — spelled
        // out per variant, rather than behind a wildcard, so each one stays a
        // choice a future match-exhaustiveness error surfaces rather than a
        // case silently falling through; see
        // `every_quote_sub_has_specific_markers`.
        QuoteType::Unquoted | QuoteType::AsciiMath | QuoteType::LatexMath => &[],
    }
}

/// Reports whether this level could possibly satisfy `sub`'s own pattern —
/// every one of its [`sub_markers`] present *somewhere* at this level (not
/// necessarily adjacent, nor in one node: each is a single ASCII character,
/// so none can be split across two contributions) — without materializing the
/// match string.
///
/// A sub-specific tightening of what used to be one gate shared by every sub
/// ("could *any* quoted-text construct start here?"): a level containing only
/// `*bold*` still has every other sub in [`quote_subs`] pay for that shared
/// sniff passing, and, worse, for the match-string build it could not by
/// itself prevent. Checking the one or two characters *this* sub's own
/// pattern needs — instead of the six-character union the group's own shared
/// `maybe_has_quotes` sniff answers for as a whole — lets a level using only
/// a couple of quote families skip the build for every sub that could never
/// have matched there in the first place.
fn level_may_match_sub(nodes: &[InlineNode<'_>], sub: &QuoteSub) -> bool {
    sub_markers(sub.type_)
        .iter()
        .all(|&marker| level_may_contain(nodes, |s| s.contains(marker)))
}

/// Reports whether this level's match string *could* contain text the
/// character-replacements sniff ([`maybe_has_replacements`]) looks for,
/// without materializing the string — the replacements counterpart of
/// [`level_may_match_sub`], probing the same contributions.
///
/// Unlike the quotes sniff, this one holds multi-character alternatives
/// (`--`, `...`, `(C)`…), which *can* straddle two adjacent contributions the
/// piecewise probe checks separately. The probe stays conservative rather
/// than exact: any non-final contribution whose last character could end a
/// proper prefix of such an alternative (`-`, `.`, `(`, `C`, `R`, `T`, `M`)
/// reports `true`, since a straddling match necessarily consumes that
/// character. A false positive only means the level builds its match string
/// and sniffs it exactly as before; a miss is impossible, because a match
/// lying inside one contribution is found by that contribution's own sniff
/// and a straddling one trips the last-character rule.
///
/// Lives here rather than beside its caller
/// ([`apply_character_replacements`](super::char_replacements)) because the
/// probe reads [`styled_sibling_boundaries`], which is this module's own.
pub(super) fn level_may_have_replacements(nodes: &[InlineNode<'_>]) -> bool {
    // The last characters of every proper prefix of the sniff's
    // multi-character alternatives; see the doc comment.
    const STRADDLE_ENDINGS: [char; 7] = ['-', '.', '(', 'C', 'R', 'T', 'M'];

    let mut buf = [0u8; 4];

    for (index, node) in nodes.iter().enumerate() {
        // Whether some contribution follows this node's own, making a
        // straddling match possible.
        let followed = index + 1 != nodes.len();

        match node {
            InlineNode::Text { value, .. } => {
                if maybe_has_replacements(value) || (followed && value.ends_with(STRADDLE_ENDINGS))
                {
                    return true;
                }
            }

            node => {
                if let Some(entity) = charref_entity(node) {
                    // Every entity's own `&` is in the sniff's class, so this
                    // arm always reports `true` in practice; it is spelled as
                    // the sniff itself for the same no-drift reason the rest
                    // of the probe is. It takes no straddle clause: an entity
                    // always ends with `;`, which ends no alternative's
                    // proper prefix.
                    if maybe_has_replacements(entity) {
                        return true;
                    }
                } else {
                    let (before, after) = match node {
                        InlineNode::Styled(styled) => {
                            styled_sibling_boundaries(styled, Masked::UNKNOWN)
                        }
                        _ => (None, None),
                    };

                    // A boundary character takes the sniff alone, no straddle
                    // rule: under [`Masked::UNKNOWN`] — the identity-less
                    // answer this probe's callers run with — the only pair
                    // [`styled_sibling_boundaries`] classifies is the
                    // smart-quote span's `&`/`;` (its `&#8220;`/`&#8221;`
                    // edges), and neither character ends an alternative's
                    // proper prefix (pinned by
                    // `the_replacements_pre_filter_reads_every_contribution_kind`).
                    let mut hits = |ch: char| maybe_has_replacements(ch.encode_utf8(&mut buf));

                    if before.is_some_and(&mut hits) || after.is_some_and(&mut hits) {
                        return true;
                    }
                }
            }
        }
    }

    false
}

/// The level's own text, when `nodes` is *exactly* one verbatim or
/// synthesized [`Text`](InlineNode::Text) node — the shape
/// [`build_match_string`] leaves untouched: with nothing else in the level to
/// contribute a boundary character or a placeholder, its match string is that
/// one node's `value`, byte for byte (see `build_match_string`'s own `Text`
/// arms, which just `push_str` it either way).
///
/// This is the shape a paragraph is in until *something* has split it —
/// `specialcharacters` finding a `<`/`>`/`&`, `quotes` wrapping a span,
/// `attributes` splicing in an expanded value — so a family whose own cheap
/// `.contains(...)` sniff would otherwise run against the *built* string can
/// run it against this instead and skip the build (and its `String`/`Vec`
/// allocations) for the overwhelmingly common level that has nothing for it
/// to find. A level already split into more than one node falls back to
/// `None`: multiple nodes are exactly the case a caller's needle can straddle
/// a boundary the single-node case cannot, so nothing here tries to answer
/// for it — see each caller's own site for how it stays safe there instead.
pub(super) fn single_text_value<'src, 'a>(nodes: &'a [InlineNode<'src>]) -> Option<&'a str> {
    match nodes {
        [InlineNode::Text { value, .. }] => Some(value.as_ref()),
        _ => None,
    }
}

pub(super) fn build_match_string(
    nodes: &[InlineNode<'_>],
    masked: Masked<'_>,
) -> (String, Vec<Piece>) {
    // Sized for the common all-text case (each node's own value), plus the
    // few bytes an entity or a boundary-wrapped placeholder adds, so the
    // build appends without growth reallocations.
    let capacity: usize = nodes
        .iter()
        .map(|node| match node {
            InlineNode::Text { value, .. } => value.len(),
            _ => 8,
        })
        .sum();

    let mut s = String::with_capacity(capacity);
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
                // (its coarse fallback) rather than a proportional slice of it.
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

            InlineNode::CharRef {
                value: CharRef::Entity(entity),
                location,
            } => {
                // A *restored* entity (`&amp;copy;` un-escaped back to
                // `&copy;` by the character-replacements step) contributes its
                // own bytes for the same reason a `Special` contributes its
                // canonical entity: those are the bytes this position holds
                // from the replacements step onward, and the fold emits them
                // verbatim (see `fold`'s `CharRef::Entity`
                // arm), so the two agree with no renderer
                // involved. It stays `atomic` — the leaf is one
                // indivisible node, never sliced — but is *recoverable*, which
                // is the distinction
                // [`range_has_no_opaque_piece`](super::macros::image::range_has_no_opaque_piece)
                // draws.
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

            InlineNode::CharRef {
                value: CharRef::Replacement(value),
                location,
            } if replacement_entity(value).is_some() => {
                // A *typographic replacement* (`(C)` and `'`, which the
                // replacements step turned into a copyright sign and a
                // typographic apostrophe) contributes the entity the built-in
                // backend renders it as, for the same reason the two other
                // `CharRef` leaves contribute theirs: those are the bytes this
                // position holds from the replacements step onward (`&#169;`,
                // `&#8217;`), so a later step matching across one — or reading
                // a value out of the match string — sees exactly the same
                // bytes. It stays `atomic` (the leaf is one indivisible node,
                // never sliced) but is *recoverable*, which is the distinction
                // [`range_has_no_opaque_piece`](super::macros::image::range_has_no_opaque_piece)
                // draws.
                //
                // The guard has already established `Some`; the crate forbids
                // `unwrap`, so the same lookup is spelled `unwrap_or_default`.
                let entity = replacement_entity(value).unwrap_or_default();
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
                // that a quote pattern sees as one boundary character —
                // wrapped, when this module can say what the string
                // pipeline's own haystack holds *beside* the construct, in
                // the two characters its rendering presents to a sibling (see
                // [`styled_sibling_boundaries`]).
                //
                // Those two characters belong to no piece — they are the
                // opaque node's own markup, and the node is already
                // represented by the placeholder's piece — so a range
                // reaching one contributes nothing, exactly as a
                // [`LevelContext`]'s do: [`emit_range`] finds no piece
                // overlapping it (which is right, since a boundary character
                // a pattern keeps is markup the span itself carries), and
                // every gate skips it as non-overlapping.
                let (before, after) = match other {
                    InlineNode::Styled(styled) => styled_sibling_boundaries(styled, masked),
                    _ => (None, None),
                };

                if let Some(before) = before {
                    s.push(before);
                }

                // Re-taken after the opening character: the piece covers the
                // placeholder alone.
                let s_start = s.len();
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

                if let Some(after) = after {
                    s.push(after);
                }
            }
        }
    }

    (s, pieces)
}

/// Reports whether any piece overlapping the match-string range `range` is
/// [`synthesized`](Piece::synthesized). A macro family that reconstructs its
/// *shown* text straight from the match string (rather than needing an honest
/// `'src` slice — e.g. an index term's `arg`/`term_src`, already checked
/// against [`SPAN_PLACEHOLDER`] for a crossed span) still needs this check
/// too: a synthesized run's bytes have no source counterpart, so even once a
/// construct inside it is recognized, the match still needs the coarse
/// `location` fallback
/// [`apply_attribute_references`](super::attribute_refs::apply_attribute_references)'s
/// doc comment describes — distinct from
/// [`range_is_verbatim`],
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
///
/// Shared with the macro families, which need the same mapping in reverse to
/// take an already-escaped *computed* value apart (see
/// `xref::escaped_value_children`), so the two directions cannot drift.
pub(super) fn special_entity(ch: char) -> &'static str {
    match ch {
        '<' => "&lt;",
        '>' => "&gt;",

        // `&` and any other special the step recognizes.
        _ => "&amp;",
    }
}

/// The entity a [`CharRef::Replacement`] contributes to the match string: the
/// bytes the **built-in** HTML backend renders that replacement as, which are
/// exactly what this position holds from the replacements
/// step onward. Returns `None` for a value no replacement rule produces (only a
/// hand-built node can carry one), which [`build_match_string`] then stands in
/// as one opaque [`SPAN_PLACEHOLDER`], as it did for every replacement before
/// this.
///
/// Like [`special_entity`], this is deliberately the *canonical* rendering
/// rather than a call into the configured renderer: a custom backend changes
/// what the fold emits, not the recognition the AsciiDoc patterns were written
/// against. `replacement_entity_matches_the_built_in_renderer` pins the two
/// against each other so they cannot drift.
pub(super) fn replacement_entity(value: &str) -> Option<&'static str> {
    // Keyed on the value rather than on
    // [`replacement_type_of`](super::callouts::replacement_type_of)'s type so
    // the two tables cover exactly the same set of values — this one's `None`
    // and that one's are the same case, a value no replacement rule produces.
    Some(match value {
        "\u{a9}" => "&#169;",
        "\u{ae}" => "&#174;",
        "\u{2122}" => "&#8482;",
        "\u{2009}\u{2014}\u{2009}" => "&#8201;&#8212;&#8201;",
        "\u{2014}\u{200b}" => "&#8212;&#8203;",
        "\u{2026}\u{200b}" => "&#8230;&#8203;",
        "\u{2019}" => "&#8217;",
        "\u{2190}" => "&#8592;",
        "\u{21d0}" => "&#8656;",
        "\u{2192}" => "&#8594;",
        "\u{21d2}" => "&#8658;",

        _ => return None,
    })
}

/// The match-string bytes a [`CharRef`] leaf contributes — a
/// [`Special`](CharRef::Special)'s canonical entity, a restored
/// [`Entity`](CharRef::Entity)'s own text, or a typographic
/// [`Replacement`](CharRef::Replacement)'s built-in rendering — or `None` for
/// every other node, whose output bytes exist only at fold time and which
/// [`build_match_string`] therefore stands in as one [`SPAN_PLACEHOLDER`].
///
/// These are the leaves whose match-string bytes are *exactly* the bytes their
/// own fold emits, which is what lets a caller read them as bytes:
/// [`range_has_no_opaque_piece`] admits a range crossing one, and
/// [`emit_range`] slices one a range only partly covers. That equality is the
/// whole content of this function, so it reads the same three arms
/// [`build_match_string`] writes them with;
/// `charref_entity_matches_the_match_strings_own_bytes` pins the two against
/// each other so they cannot drift.
///
/// [`range_has_no_opaque_piece`]: super::macros::image::range_has_no_opaque_piece
pub(super) fn charref_entity<'a>(node: &'a InlineNode<'_>) -> Option<&'a str> {
    match node {
        InlineNode::CharRef {
            value: CharRef::Special(ch),
            ..
        } => Some(special_entity(*ch)),

        InlineNode::CharRef {
            value: CharRef::Entity(entity),
            ..
        } => Some(entity.as_ref()),

        InlineNode::CharRef {
            value: CharRef::Replacement(value),
            ..
        } => replacement_entity(value),

        _ => None,
    }
}

/// One accepted quote match at a level, in absolute match-string byte offsets.
struct QuoteMatch {
    /// The whole match, `[start, end)`.
    full: Range<usize>,

    /// What to emit in place of `full`.
    kind: QuoteMatchKind,
}

impl QuoteMatch {
    /// Maps every range in this match out of the
    /// [`haystack`](LevelContext::haystack) it was found in and back into the
    /// level's own match string (see [`LevelContext::unshift`]).
    fn unshift(self, prefix: usize, len: usize) -> Self {
        let map = |range: Range<usize>| LevelContext::unshift(prefix, len, range);

        Self {
            full: map(self.full),

            kind: match self.kind {
                QuoteMatchKind::Unescape => QuoteMatchKind::Unescape,

                QuoteMatchKind::Wrap {
                    keep_literal,
                    body,
                    construct,
                    attrlist,
                    variant,
                    form,
                } => QuoteMatchKind::Wrap {
                    keep_literal: keep_literal.map(map),
                    body: map(body),
                    construct: map(construct),
                    attrlist: attrlist.map(map),
                    variant,
                    form,
                },
            },
        }
    }
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

/// Drives `sub` over the match string with a look-ahead retry: a rejected
/// monospace-before-quote match slices the haystack forward and re-searches,
/// rather than giving up on the level.
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

                let location = source_slice(pieces, construct.clone(), root);

                let (id, roles, attrs) = match attrlist {
                    Some(range) => quote_attributes(s, range.clone(), pieces, root, parser),

                    None => (None, Vec::new(), Attrlist::empty(location.slice(0..0))),
                };

                out.push(InlineNode::Styled(Styled {
                    variant: *variant,
                    form: *form,
                    id,
                    roles,
                    attrs,
                    children,
                    passthrough: None,
                    location,
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

/// Parses an **attributed quote's** attribute list out of the level's own
/// match string, returning the id, roles, and the full [`Attrlist`] the
/// node's fold renders from.
///
/// By the time the quotes step runs, the match string holds the *escaped*
/// text (`['a&lt;b&amp;c']*bold*`), so the role parsed out of it — and
/// rendered into the `class` attribute verbatim — carries the entity, not the
/// author's raw `<`. Parsing the source slice instead would put an unescaped
/// `<`/`&` into rendered markup, which is both a divergence from
/// Asciidoctor's own output and, for a `"`-bearing value, exactly the
/// injection the escaping is there to prevent (pinned by
/// `quoted_positional_role_class_does_not_double_escape_special_characters` in
/// the crate's own security tests).
///
/// A **verbatim** range's match-string bytes *are* its source bytes, so it
/// parses straight from `'src` and its attribute names and values borrow —
/// the shape every ordinary `[.role]#text#` takes. Any other range
/// (an escaped special, a restored entity or typographic replacement, or a
/// [`synthesized`](Piece::synthesized) expansion under an order that runs
/// `attributes` before `quotes`) has no `'src` slice whose bytes are the
/// attrlist text, so it parses from a [`Span::new`] over the match-string
/// slice — whose `line`/`col`/`offset` are meaningless and never escape this
/// function — and [`into_owned`](Attrlist::into_owned)s the result onto the
/// range's coarse source span, exactly as
/// [`bracket_attrlist`](super::macros::image) does for an image's bracket and
/// [`text_attrlist`](super::macros::links) for a link's display text.
///
/// A range crossing an *opaque* piece never reaches here: such a match is
/// dropped by [`attrlist_is_readable`] before the rebuild.
fn quote_attributes<'src>(
    s: &str,
    range: std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> (Option<CowStr<'src>>, Vec<CowStr<'src>>, Attrlist<'src>) {
    let source = source_slice(pieces, range.clone(), root);

    if range_is_verbatim(pieces, &range) {
        return attributes_of(source, parser);
    }

    // `s[range]` is in bounds and on char boundaries: every range here comes
    // from a capture over `s` itself.
    let escaped = s.get(range).unwrap_or_default();

    attributes_of_attrlist(parse_attrlist(Span::new(escaped), parser).into_owned(source))
}

/// Parses an attribute list from `source` and takes it apart, returning the
/// id, roles, and the full [`Attrlist`].
///
/// Shared with the attribute-list-prefixed passthrough forms
/// ([`passthrough_step`](super::passthrough_step)), which parse their own
/// attrlist the same way and fold through the same [`Styled`] node. Unlike an
/// attributed quote's (see [`quote_attributes`]), a passthrough's attrlist is
/// read from the source slice, and correctly so: the extraction pass that
/// recognizes it runs *before* the escaping step, so those are still the
/// author's raw bytes.
pub(super) fn attributes_of<'src>(
    source: Span<'src>,
    parser: &Parser,
) -> (Option<CowStr<'src>>, Vec<CowStr<'src>>, Attrlist<'src>) {
    attributes_of_attrlist(parse_attrlist(source, parser))
}

/// Parses one inline attribute list, discarding the warnings — the shared
/// spelling both of [`quote_attributes`]'s paths use.
fn parse_attrlist<'a>(source: Span<'a>, parser: &Parser) -> Attrlist<'a> {
    Attrlist::parse(source, parser, AttrlistContext::Inline)
        .item
        .item
}

/// Takes an already-parsed attribute list apart into the id, roles, and the
/// list itself.
fn attributes_of_attrlist<'src>(
    attrlist: Attrlist<'src>,
) -> (Option<CowStr<'src>>, Vec<CowStr<'src>>, Attrlist<'src>) {
    // Extract owned id/roles before the attrlist is moved into the node.
    //
    // This step performs no catalog side effect of its own: recognition and
    // registration are kept apart, so an assigned id is registered later,
    // once the tree is built and folded, by `apply_ref_side_effects` (see
    // `macros::anchors`) rather than here.
    let id = attrlist.id().map(|id| CowStr::from(id.to_string()));

    let roles = attrlist
        .roles()
        .into_iter()
        .map(|role| CowStr::from(role.to_string()))
        .collect();

    (id, roles, attrlist)
}

/// Emits the original nodes covering the match-string range `[range.start,
/// range.end)` into `out`, slicing a verbatim [`Text`](InlineNode::Text) run at
/// the boundaries and cloning any atomic piece that falls inside.
///
/// An atomic piece a boundary *splits* is sliced too, but only for the leaves
/// [`charref_entity`] names — the three [`CharRef`](InlineNode::CharRef)
/// leaves, whose match-string bytes are exactly the bytes their own fold emits
/// — each half emitted as a [`Raw`](InlineNode::Raw) leaf carrying those bytes
/// verbatim. Every other atomic piece stands in for markup that exists only at
/// fold time, so a boundary splitting one still clones it whole.
pub(super) fn emit_range<'src>(
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    range: std::ops::Range<usize>,
    out: &mut Vec<InlineNode<'src>>,
) {
    // An empty range (e.g. a macro whose node consumes its whole match, so the
    // kept-suffix range is zero-width) emits nothing — never a spurious empty
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
            // An atomic piece falling wholly inside the range is emitted as
            // its own node, which is the overwhelmingly common case.
            if p_start >= range.start && p_end <= range.end {
                out.push(node.clone());
                continue;
            }

            // A boundary that *splits* one is answerable for exactly the
            // leaves [`charref_entity`] names — the three
            // [`CharRef`](InlineNode::CharRef) leaves, whose match-string
            // bytes are the very bytes their own fold emits — because there
            // either half **is** those bytes: a [`Raw`](InlineNode::Raw) leaf
            // carrying them folds verbatim, so every partition of the entity
            // folds to the entity, and a caller cutting one (a bare URL whose
            // trailing-punctuation strip lands on an entity's own `;` — see
            // `build_inline_link_node`) can split it cleanly rather than
            // declining to handle it. Neither half has an honest
            // `'src` slice of its own (the source holds one character, or
            // `(C)`, where the match string holds an entity), so both keep the
            // leaf's whole `location` — its coarse fallback, the
            // same one a synthesized run's slices already take.
            //
            // Every other atomic piece stands in for markup that exists only
            // at fold time, one `SPAN_PLACEHOLDER` character with no bytes to
            // slice, so a partial overlap there stays what it was: clone the
            // whole piece and let the differential corpus catch any real
            // divergence.
            let Some(entity) = charref_entity(node) else {
                out.push(node.clone());
                continue;
            };

            let lo = range.start.max(p_start) - p_start;
            let hi = range.end.min(p_end) - p_start;

            // Every entity is ASCII, so both offsets are character boundaries
            // within it; the crate forbids `unwrap`, so that lookup is spelled
            // `unwrap_or_default` rather than as a branch no input reaches.
            let sliced = entity.get(lo..hi).unwrap_or_default();

            out.push(InlineNode::Raw {
                value: CowStr::from(sliced.to_string()),
                form: RawForm::AsIs,
                origin: RawOrigin::Substitution,
                location: node.span(),
            });

            continue;
        }

        // A verbatim or synthesized text run: slice it to the overlap.
        let lo = range.start.max(p_start) - p_start;
        let hi = range.end.min(p_end) - p_start;

        if let InlineNode::Text { value, location } = node {
            if piece.synthesized {
                // No `'src` slice exists for these bytes: slice the node's
                // *value* instead, keeping the whole original `location` as
                // the coarse fallback span — the same policy
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

/// Returns the literal text `range` covers, reusing [`emit_range`]'s own
/// verbatim/synthesized slicing to recover it *exactly* even when `range`
/// falls inside a [`synthesized`](Piece::synthesized) piece (an attribute
/// expansion, or — reached at a tree's root — a filtered multi-line block's
/// own joined seed): unlike [`source_slice`], which snaps a boundary landing
/// *inside* a synthesized piece to that piece's own coarse edge because it
/// must return an honest `'src` [`Span`], this slices the
/// piece's own `value` instead, so the returned text is precise rather than
/// approximate — the same recovery [`emit_range`] already gives a kept
/// [`Text`](InlineNode::Text) run, just concatenated into one value instead
/// of a node list. Still yields `None` when `range` touches an
/// [`atomic`](Piece::atomic) piece (an escaped special or a rendered span) —
/// that boundary is unchanged; a caller checks
/// [`range_is_verbatim_or_synthesized`](super::macros::image::range_is_verbatim_or_synthesized)
/// first and is expected to defer exactly as before when it fails.
pub(super) fn text_slice<'src>(
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    range: std::ops::Range<usize>,
) -> Option<CowStr<'src>> {
    if range.start >= range.end {
        return Some(CowStr::from(""));
    }

    let mut emitted = Vec::new();
    emit_range(nodes, pieces, range, &mut emitted);

    let mut parts = emitted.into_iter();

    let first = parts.next()?;
    let InlineNode::Text { value: first, .. } = first else {
        return None;
    };

    match parts.next() {
        None => Some(first),

        Some(second) => {
            let InlineNode::Text { value: second, .. } = second else {
                return None;
            };

            let mut owned = first.into_string();
            owned.push_str(&second);

            for part in parts {
                let InlineNode::Text { value, .. } = part else {
                    return None;
                };
                owned.push_str(&value);
            }

            Some(CowStr::from(owned))
        }
    }
}

/// Maps a match-string range back to its source [`Span`], sliced from `root`.
///
/// A boundary inside a verbatim [`Text`](InlineNode::Text) run maps one-to-one
/// (its match text is its source text); a boundary inside an atomic or
/// [`synthesized`](Piece::synthesized) piece has no such honest source
/// position, so it falls back to that piece's own edges —
/// snapping to the *nearer* one for an atomic piece (it never legitimately
/// falls there), or to the edge [`Bias`] names for a synthesized one, so a
/// range wholly inside a synthesized run maps to that run's *whole* node span
/// regardless of exactly where the range's boundaries land in it — the same
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

        // Before this piece begins: only a **boundary character** an opaque
        // node contributes (see [`build_match_string`]) lies outside every
        // piece, and only a leading one can be reached here — an offset
        // inside a trailing one still falls at or before its own placeholder
        // piece's end, and one between two pieces is resolved by the earlier
        // piece below. Such an offset belongs to this piece's own node, whose
        // source starts here.
        if x < p_start {
            return piece.src_offset;
        }

        // A boundary exactly at the *end* of a synthesized piece belongs to
        // whatever comes next, not to this piece: unlike a verbatim piece
        // (whose `s_len` and `src_len` always agree, so its own end edge and
        // the next piece's start edge are numerically the same value either
        // way), a synthesized piece's `value` generally has a *different*
        // byte length than its source span, so the plain linear mapping below
        // is only honest at this piece's own `p_start` (a zero delta) — never
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
            // `p_start` edge — already excluded `p_end` above, and `p_start`
            // is exact via the plain mapping just like a verbatim piece) has
            // no honest source position, so it falls back to the edge `bias`
            // names (its coarse fallback).
            if piece.synthesized && x > p_start {
                return match bias {
                    Bias::Start => piece.src_offset,
                    Bias::End => piece.src_offset + piece.src_len,
                };
            }

            return piece.src_offset + (x - p_start);
        }
    }

    // Past the last piece: the end of the source the pieces cover, or the
    // anchor for an empty level.
    pieces
        .last()
        .map_or(0, |last| last.src_offset + last.src_len)
}

/// Maps a [`QuoteType`] to its [`Styled`] variant, downgrading an attributed
/// `mark` to an unquoted span, matching Asciidoctor's own behavior.
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

    use super::{
        super::test_support::{
            assert_raw, assert_special_char, assert_styled, assert_text, build_src,
            build_through_quotes, fold_html, golden_passthroughs,
        },
        Masked,
    };
    use crate::{
        HasSpan, Span,
        inlines::{CharRef, InlineNode, SpanForm, StyleVariant},
        parser::HtmlInlineRenderer,
        strings::CowStr,
    };

    #[test]
    fn single_text_value_answers_only_for_one_lone_text_node() {
        use super::single_text_value;

        let text = |value: &'static str| InlineNode::Text {
            value: CowStr::from(value),
            location: Span::new(value),
        };

        let charref = || InlineNode::CharRef {
            value: CharRef::Special('<'),
            location: Span::new("<"),
        };

        // The one shape it answers for: exactly one `Text` node.
        assert_eq!(single_text_value(&[text("abc")]), Some("abc"));

        // Every other shape — none, more than one (even two `Text` nodes),
        // or a single node of any other kind — answers `None`: a caller
        // must fall back to building the match string.
        assert_eq!(single_text_value(&[]), None);
        assert_eq!(single_text_value(&[text("a"), text("b")]), None);
        assert_eq!(single_text_value(&[charref()]), None);
        assert_eq!(single_text_value(&[text("a"), charref()]), None);
    }

    #[test]
    fn every_quote_sub_has_specific_markers() {
        // Every `QuoteType` a real `QuoteSub` carries has its own non-empty
        // marker list — the three that never appear in `quote_subs` at all
        // (`Unquoted`, `AsciiMath`, `LatexMath`) are the only ones
        // `sub_markers` answers "no markers" for, which
        // `sub_markers_answers_empty_for_every_type_no_quote_sub_carries`
        // pins directly.
        use super::{quote_subs, sub_markers};

        for sub in quote_subs() {
            assert!(
                !sub_markers(sub.type_).is_empty(),
                "{:?} has no specific markers",
                sub.type_
            );
        }
    }

    #[test]
    fn sub_markers_answers_empty_for_every_type_no_quote_sub_carries() {
        use super::sub_markers;
        use crate::parser::QuoteType;

        for type_ in [
            QuoteType::Unquoted,
            QuoteType::AsciiMath,
            QuoteType::LatexMath,
        ] {
            assert!(sub_markers(type_).is_empty(), "{type_:?}");
        }
    }

    #[test]
    fn level_may_match_sub_rules_out_a_level_missing_the_subs_own_marker() {
        use super::{level_may_match_sub, quote_subs};

        let text = |value: &'static str| InlineNode::Text {
            value: CowStr::from(value),
            location: Span::new(value),
        };

        // Plain prose carries none of the eight marker characters any
        // `QuoteSub` needs, so every sub is correctly ruled out.
        let nodes = [text("plain prose, nothing quote-like at all")];

        for sub in quote_subs() {
            assert!(
                !level_may_match_sub(&nodes, sub),
                "{:?} should not match plain prose",
                sub.type_
            );
        }

        // A level carrying only `*`, on the other hand, still rules out
        // every sub whose own marker is not `*` — the whole point of a
        // sub-specific gate over the old shared one.
        let starred = [text("*bold*")];

        for sub in quote_subs() {
            assert_eq!(
                level_may_match_sub(&starred, sub),
                sub.type_ == crate::parser::QuoteType::Strong,
                "{:?} disagreed on `*bold*`",
                sub.type_
            );
        }
    }

    #[test]
    fn styled_boundaries_match_the_built_in_renderer() {
        // [`styled_boundaries`] decides all but one variant from its rendering
        // *shape* rather than by rendering it. This pins that shape against the
        // renderer itself, exactly as
        // `replacement_entity_matches_the_built_in_renderer` pins the
        // replacement table, so the two cannot drift.
        use super::{LevelContext, probe_styled_boundaries, styled_boundaries};
        use crate::inlines::Styled;

        let span = |variant| Styled {
            variant,
            form: SpanForm::Constrained,
            id: None,
            roles: vec![],
            attrs: crate::attributes::Attrlist::empty(Span::new("x").slice(0..0)),
            children: vec![],
            passthrough: None,
            location: Span::new("x"),
        };

        for variant in [
            StyleVariant::Strong,
            StyleVariant::Emphasis,
            StyleVariant::Code,
            StyleVariant::Mark,
            StyleVariant::Superscript,
            StyleVariant::Subscript,
            StyleVariant::DoubleQuote,
            StyleVariant::SingleQuote,
        ] {
            let styled = span(variant);

            assert_eq!(
                styled_boundaries(&styled),
                probe_styled_boundaries(&styled),
                "boundary characters drifted from the built-in rendering of {variant:?}"
            );
        }

        // An unquoted span carrying neither a role nor an id renders to its
        // body and nothing else, so it presents no boundary of its own — and,
        // when it is all its level holds, its children fall back to whatever
        // the span itself sees on both sides.
        let bare = span(StyleVariant::Unquoted);
        assert_eq!(styled_boundaries(&bare), None);
        assert_eq!(
            LevelContext::child_contexts(
                &[InlineNode::Styled(bare)],
                LevelContext::INSIDE_REF,
                Masked::UNKNOWN
            ),
            vec![LevelContext::INSIDE_REF]
        );

        // One carrying an id renders a `<span …>`, like every other tag. (The
        // node's own `roles` are not consulted: the fold hands the renderer
        // the span's `attrs` and `id`, and the renderer derives its roles from
        // that attribute list — so the probe reads exactly what the fold
        // will.)
        let mut identified = span(StyleVariant::Unquoted);
        identified.id = Some(CowStr::from("anchor"));
        assert_eq!(styled_boundaries(&identified), Some(('>', '<')));
    }

    #[test]
    fn styled_sibling_boundaries_match_the_built_in_renderer() {
        // [`styled_sibling_boundaries`] is the mirror image of
        // [`styled_boundaries`], reading the *outer* end of the same two runs
        // of markup — the first character of the opening one and the last of
        // the closing one — so it is pinned against the same probe.
        use super::{probe_styled_boundaries_markup, styled_sibling_boundaries};
        use crate::inlines::Styled;

        let span = |variant| Styled {
            variant,
            form: SpanForm::Constrained,
            id: None,
            roles: vec![],
            attrs: crate::attributes::Attrlist::empty(Span::new("x").slice(0..0)),
            children: vec![],
            passthrough: None,
            location: Span::new("x"),
        };

        // With the identity in hand, every variant that renders markup of its
        // own is answered from that markup's two outer characters.
        for variant in [
            StyleVariant::DoubleQuote,
            StyleVariant::SingleQuote,
            StyleVariant::Strong,
            StyleVariant::Emphasis,
            StyleVariant::Code,
            StyleVariant::Mark,
            StyleVariant::Superscript,
            StyleVariant::Subscript,
        ] {
            let styled = span(variant);

            let (opening, closing) = probe_styled_boundaries_markup(&styled);

            assert_eq!(
                styled_sibling_boundaries(&styled, Masked::known(&[])),
                (opening.chars().next(), closing.chars().next_back()),
                "sibling boundary characters drifted from the built-in rendering of {variant:?}"
            );
        }

        // An unquoted span carrying neither a role nor an id renders to its
        // body and nothing else, so what a sibling reads is that body: its
        // first and last characters, read out of the children's own match
        // string. With no children there is nothing to read.
        assert_eq!(
            styled_sibling_boundaries(&span(StyleVariant::Unquoted), Masked::known(&[])),
            (None, None)
        );

        let mut transparent = span(StyleVariant::Unquoted);
        transparent.children = vec![InlineNode::Text {
            value: CowStr::from("ab "),
            location: Span::new("ab "),
        }];

        assert_eq!(
            styled_sibling_boundaries(&transparent, Masked::known(&[])),
            (Some('a'), Some(' '))
        );

        // A one-character body is the same character on both sides, and one
        // this module cannot describe — a nested span with no identity to
        // classify it by — is no character at all.
        transparent.children = vec![InlineNode::Text {
            value: CowStr::from("a"),
            location: Span::new("a"),
        }];

        assert_eq!(
            styled_sibling_boundaries(&transparent, Masked::known(&[])),
            (Some('a'), Some('a'))
        );

        transparent.children = vec![InlineNode::Styled(span(StyleVariant::Strong))];

        assert_eq!(
            styled_sibling_boundaries(&transparent, Masked::UNKNOWN),
            (None, None)
        );

        // The same body, with the identity in hand: the nested span's own
        // `<strong>…</strong>` is what a sibling reads at both edges.
        assert_eq!(
            styled_sibling_boundaries(&transparent, Masked::known(&[])),
            (Some('<'), Some('>'))
        );

        // One carrying an id renders a `<span …>`, like every other tag.
        let mut identified = span(StyleVariant::Unquoted);
        identified.id = Some(CowStr::from("anchor"));

        assert_eq!(
            styled_sibling_boundaries(&identified, Masked::known(&[])),
            (Some('<'), Some('>'))
        );

        // The two **entity**-rendered variants are answered whether or not the
        // caller holds the identity: the extraction pass builds neither, so a
        // smart-quote span can only have come from the quotes step.
        for variant in [StyleVariant::DoubleQuote, StyleVariant::SingleQuote] {
            assert_eq!(
                styled_sibling_boundaries(&span(variant), Masked::UNKNOWN),
                (Some('&'), Some(';'))
            );
        }

        // Every tag-rendered variant keeps the bare placeholder where the
        // identity is missing — not because its rendering has no outer
        // characters (it has `<` and `>`), but because such a node may be the
        // passthrough-extraction pass's own wrapper, which is standing in as
        // a placeholder rather than as markup. A **transparent** span takes
        // the same guard: `[width=10]++x ++` is a wrapper that renders its
        // body and nothing else. See [`styled_sibling_boundaries`]'s own
        // scope note.
        for variant in [
            StyleVariant::Strong,
            StyleVariant::Emphasis,
            StyleVariant::Code,
            StyleVariant::Mark,
            StyleVariant::Superscript,
            StyleVariant::Subscript,
            StyleVariant::Unquoted,
        ] {
            assert_eq!(
                styled_sibling_boundaries(&span(variant), Masked::UNKNOWN),
                (None, None)
            );
        }

        // And a node the identity *names* keeps it too, which is the whole
        // point of carrying the identity: `[quotes]++text++` renders a
        // `<span class="quotes">`, but this wrapper stands in as its own
        // placeholder for every step this module runs.
        let wrapper = span(StyleVariant::Code);
        let identity = (
            wrapper.location.byte_offset(),
            wrapper.location.data().len(),
        );

        assert_eq!(
            styled_sibling_boundaries(&wrapper, Masked::known(&[identity])),
            (None, None)
        );
    }

    #[test]
    fn level_context_wraps_and_unshifts() {
        use super::LevelContext;

        // At the content's own top level the haystack is the level's own match
        // string, borrowed rather than rebuilt, and every offset is its own.
        let (haystack, prefix) = LevelContext::ROOT.haystack("abc");
        assert!(matches!(haystack, std::borrow::Cow::Borrowed("abc")));
        assert_eq!(prefix, 0);
        assert_eq!(LevelContext::unshift(0, 3, 1..3), 1..3);

        // Inside a construct the level is wrapped in the two characters that
        // construct's rendering presents, and the prefix is mapped back off.
        let (haystack, prefix) = LevelContext::INSIDE_REF.haystack("abc");
        assert_eq!(haystack, ">abc<");
        assert_eq!(prefix, 1);
        assert_eq!(LevelContext::unshift(prefix, 3, 1..4), 0..3);

        // A range reaching into either context character is clipped to the
        // level: those characters belong to the enclosing construct, which
        // already carries them.
        assert_eq!(LevelContext::unshift(prefix, 3, 0..2), 0..1);
        assert_eq!(LevelContext::unshift(prefix, 3, 3..5), 2..3);
        assert_eq!(LevelContext::unshift(prefix, 3, 4..5), 3..3);
    }

    #[test]
    fn level_context_shifts_a_levels_pieces_into_the_haystack() {
        use super::{LevelContext, Piece};

        fn piece(s_start: usize, s_len: usize) -> Piece {
            Piece {
                node_index: 0,
                s_start,
                s_len,
                src_offset: 100,
                src_len: s_len,
                atomic: false,
                synthesized: false,
            }
        }

        // The macros step maps no offsets back: it moves the level's own
        // pieces into the haystack's coordinates instead, so one coordinate
        // system reaches every gate and slice. Only the opening character is
        // applied there, since a macro body class would swallow a
        // half-supplied closing one.
        let (haystack, pieces) =
            LevelContext::INSIDE_REF.shift("abc".to_string(), vec![piece(0, 1), piece(1, 2)]);

        assert_eq!(haystack, ">abc");
        assert_eq!(pieces[0].s_start, 1);
        assert_eq!(pieces[1].s_start, 2);

        // At the content's own top level the level is its own haystack and no
        // piece moves.
        let (haystack, pieces) = LevelContext::ROOT.shift("abc".to_string(), vec![piece(0, 3)]);

        assert_eq!(haystack, "abc");
        assert_eq!(pieces[0].s_start, 0);
    }

    #[test]
    fn child_contexts_read_a_transparent_spans_siblings() {
        use super::LevelContext;
        use crate::inlines::Styled;

        const TAG: LevelContext = LevelContext::INSIDE_REF;

        fn styled(variant: StyleVariant) -> InlineNode<'static> {
            InlineNode::Styled(Styled {
                variant,
                form: SpanForm::Constrained,
                id: None,
                roles: Vec::new(),
                attrs: crate::attributes::Attrlist::empty(Span::new("x").slice(0..0)),
                children: Vec::new(),
                passthrough: None,
                location: Span::new("x"),
            })
        }

        fn text(value: &'static str) -> InlineNode<'static> {
            InlineNode::Text {
                value: CowStr::from(value),
                location: Span::new(value),
            }
        }

        // A span that renders markup of its own answers from that markup and
        // never reads a sibling: `>`/`<` are what its children see whatever
        // precedes the span. A node with no children of its own carries the
        // enclosing context, which nothing reads.
        assert_eq!(
            LevelContext::child_contexts(
                &[text("a "), styled(StyleVariant::Strong)],
                LevelContext::ROOT,
                Masked::UNKNOWN
            ),
            vec![LevelContext::ROOT, TAG]
        );

        // A transparent span takes the last character of what precedes it,
        // while the closing half stays the enclosing context's.
        assert_eq!(
            LevelContext::child_contexts(
                &[text("a "), styled(StyleVariant::Unquoted)],
                TAG,
                Masked::UNKNOWN
            ),
            vec![
                TAG,
                LevelContext {
                    before: Some(' '),
                    after: Some('<'),
                }
            ]
        );

        // With nothing before it, both halves fall back to the enclosing
        // context — the answer inheriting alone always gave.
        assert_eq!(
            LevelContext::child_contexts(
                &[styled(StyleVariant::Unquoted), text(" a")],
                TAG,
                Masked::UNKNOWN
            ),
            vec![TAG, TAG]
        );

        // At the content's own top level that fallback is `^`, and a sibling
        // supplies an opening character without a closing one.
        assert_eq!(
            LevelContext::child_contexts(
                &[text("ab"), styled(StyleVariant::Unquoted)],
                LevelContext::ROOT,
                Masked::UNKNOWN
            ),
            vec![
                LevelContext::ROOT,
                LevelContext {
                    before: Some('b'),
                    after: None,
                }
            ]
        );

        // An entity-rendered span beside it presents the `;` its own
        // `&#8221;` ends in.
        assert_eq!(
            LevelContext::child_contexts(
                &[
                    styled(StyleVariant::DoubleQuote),
                    styled(StyleVariant::Unquoted)
                ],
                LevelContext::ROOT,
                Masked::UNKNOWN
            ),
            vec![
                LevelContext {
                    before: Some(';'),
                    after: Some('&'),
                },
                LevelContext {
                    before: Some(';'),
                    after: None,
                }
            ]
        );

        // A tag-rendered one contributes the bare placeholder where the caller
        // cannot say whether it is an extraction-pass wrapper, and the bare
        // placeholder says *nothing* rather than a character: the span goes on
        // inheriting.
        assert_eq!(
            LevelContext::child_contexts(
                &[styled(StyleVariant::Strong), styled(StyleVariant::Unquoted)],
                LevelContext::ROOT,
                Masked::UNKNOWN
            ),
            vec![TAG, LevelContext::ROOT]
        );

        // With the identity in hand it presents the `>` its own `<strong>`
        // ends in, exactly as a whole-content match would see there.
        assert_eq!(
            LevelContext::child_contexts(
                &[styled(StyleVariant::Strong), styled(StyleVariant::Unquoted)],
                LevelContext::ROOT,
                Masked::known(&[])
            ),
            vec![
                TAG,
                LevelContext {
                    before: Some('>'),
                    after: None,
                }
            ]
        );

        // And a node that identity *names* is an extraction-pass wrapper that
        // still stands in as a placeholder rather than as rendered markup, so
        // it goes back to contributing the bare one.
        assert_eq!(
            LevelContext::child_contexts(
                &[styled(StyleVariant::Strong), styled(StyleVariant::Unquoted)],
                LevelContext::ROOT,
                Masked::known(&[(0, 1)])
            ),
            vec![TAG, LevelContext::ROOT]
        );

        // A **transparent** span presents no markup either, but it does
        // present its own *body*: the second span here reads the space the
        // first one's body ends with, which is what a whole-content match
        // would read between the two.
        fn transparent(children: Vec<InlineNode<'static>>) -> InlineNode<'static> {
            InlineNode::Styled(Styled {
                variant: StyleVariant::Unquoted,
                form: SpanForm::Constrained,
                id: None,
                roles: Vec::new(),
                attrs: crate::attributes::Attrlist::empty(Span::new("x").slice(0..0)),
                children,
                passthrough: None,
                location: Span::new("x"),
            })
        }

        assert_eq!(
            LevelContext::child_contexts(
                &[transparent(vec![text("y ")]), transparent(vec![text("z")])],
                LevelContext::ROOT,
                Masked::known(&[])
            ),
            vec![
                LevelContext::ROOT,
                LevelContext {
                    before: Some(' '),
                    after: None,
                }
            ]
        );

        // The character a transparent span presents to its *neighbour* is not
        // the one its own children read: what they read is whatever precedes
        // the span, so the lookup steps back over the body's own first
        // character rather than reporting it.
        assert_eq!(
            LevelContext::child_contexts(
                &[text("a "), transparent(vec![text("bc")])],
                LevelContext::ROOT,
                Masked::known(&[])
            ),
            vec![
                LevelContext::ROOT,
                LevelContext {
                    before: Some(' '),
                    after: None,
                }
            ]
        );
    }

    #[test]
    fn a_sub_inside_a_span_reads_that_spans_own_boundary_characters() {
        // The nesting cases the enclosing span's rendering decides, each
        // pinned against the frozen golden recording (see `golden_quotes`).
        for source in [
            // The shape that named this: the double-quote sub runs *before*
            // the monospace one, so by the time monospace matches, the
            // enclosing rendering holds `&#8220;` — whose `;` its boundary
            // class excludes — where the level alone would show `^`.
            r#""``end points``""#,
            r#""`_e_`""#,
            r#""`#m#`""#,
            r#"'`_e_`'"#,
            // The same span, where the sub *does* match on both sides: a
            // boundary character of its own precedes the construct.
            r#""`x `code` y`""#,
            r#""`a _b_ c`""#,
            // A sub that runs *before* the smart-quote subs is unaffected: it
            // matched over the raw source, where no entity had been written
            // yet.
            r#""`*b*`""#,
            // A tag-rendered span presents `>` and `<`, which read exactly as
            // the start and end anchors they replace for these classes — so
            // every ordinary nesting fixture is unchanged.
            "*a `b` c*",
            "*a _b_ c*",
            "_`code` in em_",
            "#a *b* c#",
            "[.r]#a `b` c#",
            // An unquoted span that renders to its body alone presents no
            // boundary of its own.
            "[width=10]#a `b` c#",
        ] {
            assert_eq!(
                golden_quotes(source),
                fold_html(
                    &build_through_quotes(Span::new(source)),
                    &HtmlInlineRenderer {}
                ),
                "fold diverged from the golden recording for {source:?}"
            );
        }
    }

    #[test]
    fn a_sub_beside_a_span_reads_that_spans_own_sibling_boundary_characters() {
        // The mirror image of the fixtures above, one level out: a construct
        // written *beside* an entity-rendered span reads the last character of
        // that span's own closing markup
        // (`&#8221;`, whose `;` the monospace sub's boundary class excludes),
        // where [`build_match_string`] used to stand the whole span in as one
        // [`SPAN_PLACEHOLDER`] — a private-use codepoint that belongs to no
        // boundary class at all. It now wraps that placeholder in the two
        // characters the span's rendering presents to a sibling (see
        // [`styled_sibling_boundaries`]), so both pipelines read the same
        // character there.
        for source in [
            // The shapes that named this increment: a construct directly
            // after a smart-quote span, which both pipelines now leave
            // literal.
            r#""`a`"`code`"#,
            r#"'`a`'`code`"#,
            r##""`a`"#mark#"##,
            r#""`a`"_em_"#,
            // The same, one character further out: a space intervenes, so
            // both pipelines match.
            r#""`a`" `code`"#,
            r#"'`a`' _em_"#,
            // A construct directly *before* a smart-quote span reads that
            // span's opening `&`, which is non-word exactly as the
            // placeholder it replaces is — so both pipelines match, as they
            // did before.
            r#"`code`"`a`""#,
            r#"_em_'`a`'"#,
            // Two smart-quote spans side by side: the second sub reads the
            // first span's own closing `;`.
            r#""`a`"'`b`'"#,
            // A tag-rendered span keeps the bare placeholder, which reads as
            // `>` does to every quote boundary class (both are non-word and
            // in none of `&;:}`), so these are unchanged either way.
            "*a*`code`",
            "`code`*a*",
            "[.r]#a#`code`",
            // The same constructs at the content's own top level, where no
            // span is involved at all.
            "`code`",
            "#mark#",
        ] {
            assert_eq!(
                golden_quotes(source),
                fold_html(
                    &build_through_quotes(Span::new(source)),
                    &HtmlInlineRenderer {}
                ),
                "fold diverged from the golden recording for {source:?}"
            );
        }
    }

    #[test]
    fn a_sub_inside_a_transparent_span_reads_that_spans_own_siblings() {
        // The half both tests above name: a **transparent** span — an
        // unquoted span whose attribute list resolves to neither a role nor an
        // id — renders to its body and nothing else, so what its children read
        // is not the enclosing construct's markup but whatever stands *beside
        // the span itself*. Only one sub can reach this at all: the
        // constrained `#mark#` is the last boundary-reading sub in the list,
        // so the span it looks across must have been built by the
        // unconstrained `##mark##` one place ahead of it.
        //
        // `x[width=10]###c# d##` is the shape that names it. The string
        // pipeline's haystack after the unconstrained sub is `x#c# d`, where
        // the `#` is preceded by a word character and the constrained sub's
        // own `(^|[^\w&;:}])` rejects it; the level alone showed `^`, and
        // wrapped it.
        for source in [
            // A word character beside the span, which both pipelines now
            // reject.
            "x[width=10]###c# d##",
            "*x[width=10]###c# d##*",
            // The same sibling one character further out: a space, which both
            // pipelines accept.
            "x [width=10]###c# d##",
            "*x [width=10]###c# d##*",
            // No sibling at all, where the span really is all its level holds
            // and the enclosing context is the right answer — `^` at the
            // content's own top level, and the enclosing `<strong>`'s own `>`
            // inside one.
            "[width=10]###c# d##",
            "*[width=10]###c# d##*",
            // A tag-rendered span beside it contributes the bare placeholder,
            // which reads as its `>` does to this boundary class (both are
            // non-word and in none of `&;:}`).
            "*x*[width=10]###c# d##",
            // A construct away from the span's own edge, where the character
            // before it is the level's own text in both pipelines.
            "x[width=10]##d #c# e##",
            // And a sibling *after* the span, which supplies nothing: the
            // closing half is deliberately not carried (see
            // [`LevelContext::child_contexts`]).
            "[width=10]###c# d##x",
            "[width=10]###c# d## x",
        ] {
            assert_eq!(
                golden_quotes(source),
                fold_html(
                    &build_through_quotes(Span::new(source)),
                    &HtmlInlineRenderer {}
                ),
                "fold diverged from the golden recording for {source:?}"
            );
        }
    }

    #[test]
    fn a_sub_closing_on_a_transparent_spans_sibling_is_a_documented_divergence() {
        // The closing half [`LevelContext::child_contexts`] deliberately does
        // not carry. A boundary class reads one character and, where it
        // consumes one, the replacer writes it back — which
        // [`LevelContext::unshift`]'s clip reproduces by leaving the character
        // with the sibling that owns it. A *delimiter* swallows it instead,
        // and what the replacer swallows it deletes: here the constrained
        // `#mark#` sub's own closing `#` is the sibling, so a level given it
        // would build a span and still emit the `#` a level out.
        //
        // Supplying it would make this shape differently wrong rather than
        // right; closing it means letting one level's rebuild consume a node
        // another level owns.
        let source = "x[width=10]##d #c###";

        let folded = fold_html(
            &build_through_quotes(Span::new(source)),
            &HtmlInlineRenderer {},
        );

        assert_ne!(
            golden_quotes(source),
            folded,
            "expected the documented divergence to still reproduce"
        );

        // A whole-content match sees `xd #c#`, all of it one flat
        // string, so the sub wraps `c`; the tree holds `d #c` inside the span
        // and the closing `#` beside it, and leaves both literal.
        assert_eq!(golden_quotes(source), "xd <mark>c</mark>");
        assert_eq!(folded, "xd #c#");
    }

    #[test]
    fn a_sub_beside_a_masked_passthrough_wrapper_keeps_the_bare_placeholder() {
        // The one tag-rendered span that presents *no* markup to a sibling.
        // The passthrough-extraction pass builds a [`Styled`] wrapper of its
        // own for an attribute-list-prefixed passthrough, standing in as its
        // own placeholder rather than as markup for every step this module
        // runs — so a sibling reads that placeholder, which is exactly what
        // the bare placeholder reads as. The
        // identity `masked` carries is what tells one from a genuinely
        // rendered span of the identical shape.
        //
        // Asserted directly on the match string, since a quote boundary class
        // cannot tell a `>` from the placeholder in the first place (which is
        // why the quotes step goes on passing `Masked::UNKNOWN`).
        use super::{SPAN_PLACEHOLDER, build_match_string, styled_sibling_boundaries};
        use crate::inlines::Styled;

        // An id rather than a role, so the probe sees a `<span …>`: the fold
        // hands the renderer the span's `attrs` and `id` and lets it derive
        // the roles, so a `roles` field with no attribute list behind it
        // renders nothing (see
        // `styled_boundaries_match_the_built_in_renderer`).
        let styled = Styled {
            variant: StyleVariant::Unquoted,
            form: SpanForm::Unconstrained,
            id: Some(CowStr::from("x")),
            roles: Vec::new(),
            attrs: crate::attributes::Attrlist::empty(Span::new("[#x]##y##").slice(0..0)),
            children: Vec::new(),
            passthrough: None,
            location: Span::new("[#x]##y##"),
        };

        let identity = (styled.location.byte_offset(), styled.location.data().len());

        assert_eq!(
            styled_sibling_boundaries(&styled, Masked::known(&[identity])),
            (None, None)
        );

        let (s, pieces) = build_match_string(
            &[InlineNode::Styled(styled.clone())],
            Masked::known(&[identity]),
        );

        assert_eq!(s, SPAN_PLACEHOLDER.to_string());
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0].s_start, 0);

        // The same node, *not* named by the identity, is a span that really
        // has been rendered — and presents the two characters its
        // `<span class="x">…</span>` puts beside its siblings.
        let (s, pieces) = build_match_string(&[InlineNode::Styled(styled)], Masked::known(&[]));

        assert_eq!(s, format!("<{SPAN_PLACEHOLDER}>"));
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0].s_start, 1);
    }

    #[test]
    fn a_transparent_spans_placeholder_carries_its_own_body() {
        // A span that renders to its body and nothing else presents *that
        // body* to a sibling, where every other variant presents its markup —
        // so the placeholder standing in for it carries the body's own two
        // outer characters, read out of the children's own match string.
        //
        // Asserted directly on the match string for the reason the wrapper
        // test above is: the class that reads these characters is the macros
        // step's, and it is the only step holding the identity that gates
        // them.
        use super::{SPAN_PLACEHOLDER, build_match_string};
        use crate::inlines::Styled;

        let styled = |children| Styled {
            variant: StyleVariant::Unquoted,
            form: SpanForm::Unconstrained,
            id: None,
            roles: Vec::new(),
            attrs: crate::attributes::Attrlist::empty(Span::new("[width=10]##x ##").slice(0..0)),
            children,
            passthrough: None,
            location: Span::new("[width=10]##x ##"),
        };

        let text = |value: &'static str| InlineNode::Text {
            value: CowStr::from(value),
            location: Span::new(value),
        };

        let body = || vec![text("x ")];

        // With the identity in hand, the body's first and last characters land
        // on either side of the placeholder — and the piece still covers the
        // placeholder alone, so the two belong to no node and no range a
        // caller slices moves.
        let (s, pieces) =
            build_match_string(&[InlineNode::Styled(styled(body()))], Masked::known(&[]));

        assert_eq!(s, format!("x{SPAN_PLACEHOLDER} "));
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0].s_start, 1);

        // Without it — every step but `macros` — the span this module cannot
        // rule out being an extraction wrapper keeps the bare placeholder,
        // byte for byte what it carried before.
        let (s, pieces) =
            build_match_string(&[InlineNode::Styled(styled(body()))], Masked::UNKNOWN);

        assert_eq!(s, SPAN_PLACEHOLDER.to_string());
        assert_eq!(pieces[0].s_start, 0);

        // And a node the identity *names* is such a wrapper — `[width=10]++x
        // ++` renders its body and nothing else too, and it stands in
        // as its own placeholder there, which
        // is exactly what a bare placeholder reads as.
        let wrapper = styled(body());

        let identity = (
            wrapper.location.byte_offset(),
            wrapper.location.data().len(),
        );

        let (s, _) = build_match_string(&[InlineNode::Styled(wrapper)], Masked::known(&[identity]));

        assert_eq!(s, SPAN_PLACEHOLDER.to_string());
    }

    #[test]
    fn the_replacements_pre_filter_reads_every_contribution_kind() {
        // The piecewise pre-filter (`level_may_have_replacements`) must answer
        // exactly as sniffing the built match string would, per contribution
        // kind: a `CharRef`'s entity bytes (whose `&` is in the sniff's
        // class), a smart-quote span's presented boundary `&`, a straddle
        // ending on a non-final text run, and — negatively — a tag-rendered
        // span under `Masked::UNKNOWN`, which stands as the bare placeholder
        // and can satisfy nothing.
        use super::level_may_have_replacements;
        use crate::inlines::{CharRef, Styled};

        let text = |value: &'static str| InlineNode::Text {
            value: CowStr::from(value),
            location: Span::new(value),
        };

        let styled = |variant| {
            InlineNode::Styled(Styled {
                variant,
                form: SpanForm::Constrained,
                id: None,
                roles: Vec::new(),
                attrs: crate::attributes::Attrlist::empty(Span::new("").slice(0..0)),
                children: vec![text("x")],
                passthrough: None,
                location: Span::new("\"`x`\""),
            })
        };

        // A `CharRef` contributes its entity, whose `&` sniffs.
        assert!(level_may_have_replacements(&[InlineNode::CharRef {
            value: CharRef::Special('<'),
            location: Span::new("<"),
        }]));

        // A smart-quote span presents `&#8220;`'s own `&` to its siblings.
        assert!(level_may_have_replacements(&[styled(
            StyleVariant::DoubleQuote
        )]));

        // A non-final text run ending a proper prefix of `--` admits the
        // level through the straddle rule; the same run standing last cannot
        // straddle anything.
        assert!(level_may_have_replacements(&[text("x-"), text("y")]));
        assert!(!level_may_have_replacements(&[text("x-")]));

        // A tag-rendered span is unclassified without the extraction pass's
        // identity: a bare placeholder, sniffing nothing.
        assert!(!level_may_have_replacements(&[styled(
            StyleVariant::Strong
        )]));

        // An entity that carries no sniffable byte — a shape no producer
        // builds, since a real entity is always `&…;` — falls through to the
        // nodes after it rather than answering for the level.
        assert!(!level_may_have_replacements(&[InlineNode::CharRef {
            value: CharRef::Entity(CowStr::from("x")),
            location: Span::new("x"),
        }]));
    }

    #[test]
    fn a_real_documents_nested_span_tree_folds_to_its_rendered_string() {
        // End-to-end, through the real parse path, on the shape that named
        // this increment: `"``end points``"` is a golden fixture of its own
        // (`tests::asciidoc_lang::text::troubleshoot_unconstrained_formatting`
        // asserts the inner backticks stay literal), so a tree that recognized
        // a `<code>` span there would regress it the moment `rendered_html()`
        // becomes a fold of this tree.
        use crate::{
            Parser,
            blocks::{FindBlocks, IsBlock},
        };

        let doc = Parser::default().parse(concat!(
            "== A heading\n",
            "\n",
            "That only gives you \"``end points``\".\n",
            "\n",
            "A *span ending in --* here.\n",
        ));

        let mut folded_blocks = 0;

        for block in doc.descendant_blocks() {
            let (Some(rendered), Some(inlines)) = (block.rendered_html_content(), block.inlines())
            else {
                continue;
            };

            assert_eq!(
                super::super::fold_html(
                    inlines,
                    &HtmlInlineRenderer {},
                    &Parser::default().render_context()
                ),
                rendered,
                "fold diverged from the rendered string for {inlines:?}"
            );

            folded_blocks += 1;
        }

        // The two paragraphs; the section that contains them holds no inline
        // content of its own and is skipped.
        assert_eq!(folded_blocks, 2, "expected both paragraphs to be checked");
    }

    #[test]
    fn a_real_documents_sibling_span_tree_folds_to_its_rendered_string() {
        // End-to-end, through the real parse path, on the shape one level out
        // from the test above: a construct written *beside* an entity-rendered
        // span, whose rendering supplies a `;` from `&#8221;` where the
        // tree holds one placeholder.
        use crate::{
            Parser,
            blocks::{FindBlocks, IsBlock},
        };

        let doc = Parser::default().parse(concat!(
            "== A heading\n",
            "\n",
            "She said \"`hello`\"`code` and meant it.\n",
            "\n",
            "Then '`this`'`that` and \"`one`\"'`two`' in a row.\n",
            "\n",
            "And x[width=10]###c# d## beside x [width=10]###c# d## here.\n",
        ));

        let mut folded_blocks = 0;

        for block in doc.descendant_blocks() {
            let (Some(rendered), Some(inlines)) = (block.rendered_html_content(), block.inlines())
            else {
                continue;
            };

            assert_eq!(
                super::super::fold_html(
                    inlines,
                    &HtmlInlineRenderer {},
                    &Parser::default().render_context()
                ),
                rendered,
                "fold diverged from the rendered string for {inlines:?}"
            );

            folded_blocks += 1;
        }

        // The three paragraphs; the section that contains them holds no inline
        // content of its own and is skipped.
        assert_eq!(folded_blocks, 3, "expected every paragraph to be checked");
    }

    #[test]
    fn a_real_documents_tag_rendered_sibling_tree_folds_to_its_rendered_string() {
        // End-to-end, through the real parse path, on the **tag**-rendered half
        // of the shape above — the one the extraction pass's identity had to
        // reach recognition for. A URL written against a closing tag's own `>`
        // links; one written against the pass's own wrapper, which still
        // stands in as its own placeholder, stays literal.
        use crate::{
            Parser,
            blocks::{FindBlocks, IsBlock},
        };

        let doc = Parser::default().parse(concat!(
            "== A heading\n",
            "\n",
            "See **bold**https://example.org and __em__https://example.org here.\n",
            "\n",
            "But [quotes]++x++https://example.org stays literal.\n",
            "\n",
            "And *x*[width=10]#doc@example.org# beside *x [width=10]#doc@example.org#*.\n",
        ));

        let mut folded_blocks = 0;

        for block in doc.descendant_blocks() {
            let (Some(rendered), Some(inlines)) = (block.rendered_html_content(), block.inlines())
            else {
                continue;
            };

            assert_eq!(
                super::super::fold_html(
                    inlines,
                    &HtmlInlineRenderer {},
                    &Parser::default().render_context()
                ),
                rendered,
                "fold diverged from the rendered string for {inlines:?}"
            );

            folded_blocks += 1;
        }

        // The three paragraphs; the section that contains them holds no inline
        // content of its own and is skipped.
        assert_eq!(folded_blocks, 3, "expected every paragraph to be checked");
    }

    #[test]
    fn replacement_entity_matches_the_built_in_renderer() {
        // The table `build_match_string` reads is the built-in backend's own
        // rendering of each replacement — the bytes this position holds
        // from the replacements step onward — so the two cannot
        // be allowed to drift. Every value the classifier recognizes is
        // checked against the renderer that produces it, and a value no rule
        // produces has no entity at all (`build_match_string` stands such a
        // leaf in as one opaque placeholder, as it did for every replacement
        // before this).
        use super::super::callouts::replacement_type_of;
        use crate::parser::InlineRenderer;

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
            let type_ = replacement_type_of(value).unwrap();

            let mut rendered = String::new();
            HtmlInlineRenderer {}.render_character_replacement(type_, &mut rendered);

            assert_eq!(
                super::replacement_entity(value),
                Some(rendered.as_str()),
                "match-string bytes drifted from the built-in rendering of {value:?}"
            );
        }

        assert!(super::replacement_entity("not a replacement").is_none());
    }

    /// The frozen golden recording through the **quotes** step for `source`,
    /// used as the oracle: `Content::from` then `SpecialCharacters` then
    /// `Quotes`, exactly the order [`build`] runs them.
    fn golden_quotes(source: &str) -> String {
        crate::content::inline_builder::snapshot::recorded("quotes", source)
    }

    #[test]
    fn fold_matches_the_string_pipeline_through_quotes() {
        // For each fixture, folding the single-pass tree (special characters +
        // quotes) reproduces the golden recording's output byte-for-byte. This
        // is the differential corpus that pins the quotes
        // step.
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
            // An attribute list carrying a special character. The escaping
            // step runs *before* this one, so `quote_attributes` parses the
            // already-escaped text and renders the entity straight into the
            // `class`/`id` attribute, in every spelling an attribute list has.
            "[.a<b]*bold*",
            "[#a&b]#x#",
            "[a<b]#x#",
            // The same, on the *unconstrained* branch, whose attribute list is
            // its own capture group.
            "[.role]##x##",
            "[.a<b]##x##",
            "[role=\"a<b\"]*bold*",
            "['a<b&c']*bold*",
            "[.a&b.c<d]_x_",
            "[#i&d.r<le]*bold*",
            // Shorthand names that name *nothing*. Both pipelines read a span's
            // attribute list through the same `Attrlist`, so the rule that a
            // whitespace-only name is dropped exactly as a missing one is
            // reaches the tree with it: the first `#` item here no longer
            // shadows the real id behind it.
            "[x#\t#realid]#x#",
            "[x% ]#x#",
            "[x%\t]#x#",
            "[.\t.role]#x#",
            "[%%%%]#x#",
            "[.role]#a < b#",
            // The `"`-escaping the built-in backend adds on top (a quoted
            // positional role and a `role=` value alike) composes with it: the
            // specials stay singly escaped either way.
            "['x\" onmouseover=\"y']*bold*",
            "[role='a\"b']#x#",
            // Escaped-with-attributes: the `[…]` is kept literal (and escaped
            // as ordinary text by the step before this one) while the body is
            // still wrapped with no attribute list at all.
            "\\[a<b]*bold*",
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

        let renderer = HtmlInlineRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_through_quotes(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden_quotes(fixture),
                "fold diverged from the golden recording for {fixture:?}"
            );
        }
    }

    #[test]
    fn s_to_src_guards_are_defensive() {
        use super::{Bias, Piece, s_to_src};

        // In practice every boundary `s_to_src` maps falls on a literal
        // delimiter (a text position) or on a **boundary character** an opaque
        // node contributes, so the atomic-snap, before-first, and past-last
        // branches are defensive. Exercise them directly to document the
        // intended fallback. Bias is irrelevant to an atomic piece, so
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

        // A boundary before the first piece begins — the leading boundary
        // character an opaque node contributes, the one position no piece
        // covers — resolves to that piece's own source start.
        let offset = Piece {
            s_start: 2,
            ..atomic
        };

        assert_eq!(s_to_src(std::slice::from_ref(&offset), 0, Bias::Start), 10);

        // No pieces (an empty level) anchors at the source start.
        assert_eq!(s_to_src(&[], 0, Bias::Start), 0);
    }

    #[test]
    fn s_to_src_biases_a_synthesized_piece_to_its_whole_node_span() {
        use super::{Bias, Piece, s_to_src};

        // A boundary landing *strictly inside* a synthesized piece (its
        // match-string bytes have no honest source counterpart, since its
        // `s_len` here — 9 — differs from its `src_len` — 10) falls back to
        // the whole node span: its start edge (100) for a `Bias::Start`
        // boundary, its end edge (110) for a `Bias::End` one.
        //
        // Its own two edges (`x == 0`, `x == 9`) are a distinct case, pinned
        // by `s_to_src_resolves_a_synthesized_pieces_own_edges_exactly`
        // below: `p_start` already has an honest position (delta zero) and
        // `p_end` is skipped to whatever comes next, so *neither* runs
        // through this bias fallback — only interior positions like `3` do.
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
        // end edge has an honest position regardless of `bias` — unlike an
        // interior boundary (see the test above), it never falls back to the
        // coarse whole-node span. This is what keeps a construct recognized
        // *immediately after* a synthesized run (e.g. a second `image:` macro
        // right after an `{sp}` attribute reference) from having its node's
        // location wrongly swallow the synthesized run's own source bytes —
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
        // alone is its own end — numerically the same as the whole-node-span
        // fallback here, but arrived at without going through `Bias` at all.
        let pieces = [synthesized_piece()];
        assert_eq!(s_to_src(&pieces, 5, Bias::Start), 100);
        assert_eq!(s_to_src(&pieces, 5, Bias::End), 100);
        assert_eq!(s_to_src(&pieces, 14, Bias::Start), 110);
        assert_eq!(s_to_src(&pieces, 14, Bias::End), 110);

        // With a verbatim piece immediately following (the common case — a
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
    fn emit_range_cuts_a_charref_leaf_into_raw_halves() {
        use super::{build_match_string, emit_range};

        // A boundary inside a `CharRef` leaf cuts it, because either half of
        // its match-string bytes is what that half's own fold emits. The three
        // leaves are cut alike; each half keeps the leaf's whole location
        // (its coarse fallback), and the two concatenate back to the entity.
        let source = Span::new("&(C)\u{a9}");

        let nodes = vec![
            InlineNode::CharRef {
                value: CharRef::Special('&'),
                location: source.slice(0..1),
            },
            InlineNode::CharRef {
                value: CharRef::Replacement("\u{a9}"),
                location: source.slice(1..4),
            },
            InlineNode::CharRef {
                value: CharRef::Entity(CowStr::from("&copy;")),
                location: source.slice(4..6),
            },
        ];

        let (s, pieces) = build_match_string(&nodes, Masked::UNKNOWN);
        assert_eq!(s, "&amp;&#169;&copy;");

        // One cut in each leaf, at its own final `;` — the boundary a bare
        // auto-link's trailing-punctuation strip lands on.
        for (range, head, tail, location) in [
            (0..4, "&amp", ";", "&"),
            (5..10, "&#169", ";", "(C)"),
            (11..16, "&copy", ";", "\u{a9}"),
        ] {
            let mut out = Vec::new();
            emit_range(&nodes, &pieces, range.clone(), &mut out);
            assert_eq!(out.len(), 1, "{out:?}");
            assert_eq!(assert_raw(&out[0], head).data(), location);

            let mut out = Vec::new();
            emit_range(&nodes, &pieces, range.end..range.end + 1, &mut out);
            assert_eq!(out.len(), 1, "{out:?}");
            assert_eq!(assert_raw(&out[0], tail).data(), location);
        }

        // A range covering a leaf whole still emits the leaf itself, which is
        // what every caller before this one gets.
        let mut out = Vec::new();
        emit_range(&nodes, &pieces, 0..5, &mut out);
        assert_eq!(out.len(), 1, "{out:?}");
        assert_special_char(&out[0], '&');
    }

    #[test]
    fn emit_range_keeps_an_opaque_piece_whole_at_a_cut() {
        use super::{build_match_string, emit_range};
        use crate::inlines::Styled;

        // The other half of the rule: a piece standing in for markup that
        // exists only at fold time has no bytes to cut, so a boundary
        // splitting one clones it whole, exactly as before.
        let source = Span::new("*x*");

        let nodes = vec![InlineNode::Styled(Styled {
            variant: StyleVariant::Strong,
            form: SpanForm::Constrained,
            id: None,
            roles: vec![],
            attrs: crate::attributes::Attrlist::empty(source.slice(0..0)),
            children: vec![],
            passthrough: None,
            location: source,
        })];

        let (_, pieces) = build_match_string(&nodes, Masked::UNKNOWN);

        // The placeholder is one three-byte character; a range covering its
        // first byte alone still emits the span itself.
        let mut out = Vec::new();
        emit_range(&nodes, &pieces, 0..1, &mut out);
        assert_eq!(out.len(), 1, "{out:?}");
        assert_styled(&out[0], StyleVariant::Strong, SpanForm::Constrained);
    }

    #[test]
    fn charref_entity_matches_the_match_strings_own_bytes() {
        use super::{build_match_string, charref_entity};

        // [`charref_entity`] answers the bytes [`build_match_string`] writes
        // for the same node — the equality every caller reading a leaf as
        // bytes rests on, and the one thing that could drift between the two
        // lists of `CharRef` arms.
        let source = Span::new("<>&(C)'\u{a9}x");

        let nodes = vec![
            InlineNode::CharRef {
                value: CharRef::Special('<'),
                location: source.slice(0..1),
            },
            InlineNode::CharRef {
                value: CharRef::Special('>'),
                location: source.slice(1..2),
            },
            InlineNode::CharRef {
                value: CharRef::Special('&'),
                location: source.slice(2..3),
            },
            InlineNode::CharRef {
                value: CharRef::Replacement("\u{a9}"),
                location: source.slice(3..6),
            },
            InlineNode::CharRef {
                value: CharRef::Replacement("\u{2019}"),
                location: source.slice(6..7),
            },
            InlineNode::CharRef {
                value: CharRef::Entity(CowStr::from("&copy;")),
                location: source.slice(7..9),
            },
            InlineNode::Text {
                value: CowStr::from("x"),
                location: source.slice(9..10),
            },
        ];

        let (s, pieces) = build_match_string(&nodes, Masked::UNKNOWN);

        for piece in &pieces {
            let node = &nodes[piece.node_index];
            let bytes = &s[piece.s_start..piece.s_start + piece.s_len];

            match charref_entity(node) {
                // A leaf: its entity is exactly the piece's own bytes, and the
                // piece is atomic (it is one indivisible node, cut only where
                // both halves are its own bytes).
                Some(entity) => {
                    assert_eq!(entity, bytes, "{node:?}");
                    assert!(piece.atomic, "{node:?}");
                }

                // Everything else is not a leaf — here the `Text` run, which
                // is not atomic either.
                None => assert!(!piece.atomic, "{node:?}"),
            }
        }

        // A replacement carrying a value no rule produces is not a leaf: the
        // one shape whose two answers agree by *exclusion*, since
        // `build_match_string` stands it in as a placeholder.
        assert!(
            charref_entity(&InlineNode::CharRef {
                value: CharRef::Replacement("\u{2603}"),
                location: source,
            })
            .is_none()
        );
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
        // declaring a longer `s_len` than its node's actual `value` — an
        // internal invariant slip — is skipped rather than panicking, the
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
    fn text_slice_of_an_empty_range_is_empty() {
        use super::text_slice;

        assert_eq!(text_slice(&[], &[], 3..3).unwrap().as_ref(), "");
    }

    #[test]
    fn text_slice_borrows_from_a_single_verbatim_piece() {
        use super::{Piece, text_slice};
        use crate::{Span, inlines::InlineNode, strings::CowStr};

        let location = Span::new("hello");
        let node = InlineNode::Text {
            value: CowStr::from("hello"),
            location,
        };
        let piece = Piece {
            node_index: 0,
            s_start: 0,
            s_len: 5,
            src_offset: location.byte_offset(),
            src_len: location.data().len(),
            atomic: false,
            synthesized: false,
        };

        let result = text_slice(
            std::slice::from_ref(&node),
            std::slice::from_ref(&piece),
            1..4,
        )
        .unwrap();

        assert_eq!(result.as_ref(), "ell");
        assert!(matches!(result, CowStr::Borrowed(_)));
    }

    #[test]
    fn text_slice_recovers_exact_text_from_a_single_synthesized_piece() {
        use super::{Piece, text_slice};
        use crate::{Span, inlines::InlineNode, strings::CowStr};

        // The recovered text ("value") differs from `location`'s own source
        // bytes ("{x}") — the exact shape a spliced attribute expansion
        // produces — which is what `source_slice`'s coarse fallback cannot
        // recover but `text_slice` can.
        let location = Span::new("{x}");
        let node = InlineNode::Text {
            value: CowStr::from("expanded value"),
            location,
        };
        let piece = Piece {
            node_index: 0,
            s_start: 0,
            s_len: "expanded value".len(),
            src_offset: location.byte_offset(),
            src_len: location.data().len(),
            atomic: false,
            synthesized: true,
        };

        let result = text_slice(
            std::slice::from_ref(&node),
            std::slice::from_ref(&piece),
            9..14,
        )
        .unwrap();

        assert_eq!(result.as_ref(), "value");
        assert!(matches!(result, CowStr::Boxed(_)));
    }

    #[test]
    fn text_slice_concatenates_across_multiple_pieces() {
        use super::{Piece, text_slice};
        use crate::{Span, inlines::InlineNode, strings::CowStr};

        let loc_a = Span::new("ab");
        let node_a = InlineNode::Text {
            value: CowStr::from("ab"),
            location: loc_a,
        };
        let piece_a = Piece {
            node_index: 0,
            s_start: 0,
            s_len: 2,
            src_offset: loc_a.byte_offset(),
            src_len: loc_a.data().len(),
            atomic: false,
            synthesized: false,
        };

        let loc_b = Span::new("{y}");
        let node_b = InlineNode::Text {
            value: CowStr::from("cd"),
            location: loc_b,
        };
        let piece_b = Piece {
            node_index: 1,
            s_start: 2,
            s_len: 2,
            src_offset: loc_b.byte_offset(),
            src_len: loc_b.data().len(),
            atomic: false,
            synthesized: true,
        };

        let nodes = [node_a, node_b];
        let pieces = [piece_a, piece_b];

        let result = text_slice(&nodes, &pieces, 0..4).unwrap();

        assert_eq!(result.as_ref(), "abcd");
        assert!(
            matches!(result, CowStr::Boxed(_)),
            "a multi-piece result is always owned, even when every piece is verbatim"
        );
    }

    #[test]
    fn text_slice_concatenates_across_three_or_more_pieces() {
        use super::{Piece, text_slice};
        use crate::{Span, inlines::InlineNode, strings::CowStr};

        // Exercises the tail of the concatenation loop (past the first two
        // pieces), which the two-piece test above cannot reach.
        let text_piece = |node_index, s_start, data: &'static str| {
            let location = Span::new(data);
            (
                InlineNode::Text {
                    value: CowStr::from(data),
                    location,
                },
                Piece {
                    node_index,
                    s_start,
                    s_len: data.len(),
                    src_offset: location.byte_offset(),
                    src_len: location.data().len(),
                    atomic: false,
                    synthesized: false,
                },
            )
        };

        let (node_a, piece_a) = text_piece(0, 0, "a");
        let (node_b, piece_b) = text_piece(1, 1, "b");
        let (node_c, piece_c) = text_piece(2, 2, "c");

        let nodes = [node_a, node_b, node_c];
        let pieces = [piece_a, piece_b, piece_c];

        let result = text_slice(&nodes, &pieces, 0..3).unwrap();
        assert_eq!(result.as_ref(), "abc");
    }

    #[test]
    fn text_slice_returns_none_when_a_third_piece_is_atomic() {
        use super::{Piece, text_slice};
        use crate::{
            Span,
            inlines::{CharRef, InlineNode},
            strings::CowStr,
        };

        // Covers the concatenation loop's own atomic check for a piece past
        // the *second* one — distinct from
        // `text_slice_returns_none_when_a_later_piece_is_atomic`, which stops
        // at the second.
        let loc_a = Span::new("a");
        let node_a = InlineNode::Text {
            value: CowStr::from("a"),
            location: loc_a,
        };
        let piece_a = Piece {
            node_index: 0,
            s_start: 0,
            s_len: 1,
            src_offset: loc_a.byte_offset(),
            src_len: loc_a.data().len(),
            atomic: false,
            synthesized: false,
        };

        let loc_b = Span::new("b");
        let node_b = InlineNode::Text {
            value: CowStr::from("b"),
            location: loc_b,
        };
        let piece_b = Piece {
            node_index: 1,
            s_start: 1,
            s_len: 1,
            src_offset: loc_b.byte_offset(),
            src_len: loc_b.data().len(),
            atomic: false,
            synthesized: false,
        };

        let loc_c = Span::new("&amp;");
        let node_c = InlineNode::CharRef {
            value: CharRef::Special('&'),
            location: loc_c,
        };
        let piece_c = Piece {
            node_index: 2,
            s_start: 2,
            s_len: 5,
            src_offset: loc_c.byte_offset(),
            src_len: loc_c.data().len(),
            atomic: true,
            synthesized: false,
        };

        let nodes = [node_a, node_b, node_c];
        let pieces = [piece_a, piece_b, piece_c];

        assert!(text_slice(&nodes, &pieces, 0..7).is_none());
    }

    #[test]
    fn text_slice_returns_none_when_a_later_piece_is_atomic() {
        use super::{Piece, text_slice};
        use crate::{
            Span,
            inlines::{CharRef, InlineNode},
            strings::CowStr,
        };

        // The atomic-rejection test above hits it on the *first* piece; this
        // covers the loop's own atomic check for a piece after the first.
        let loc_a = Span::new("a");
        let node_a = InlineNode::Text {
            value: CowStr::from("a"),
            location: loc_a,
        };
        let piece_a = Piece {
            node_index: 0,
            s_start: 0,
            s_len: 1,
            src_offset: loc_a.byte_offset(),
            src_len: loc_a.data().len(),
            atomic: false,
            synthesized: false,
        };

        let loc_b = Span::new("&amp;");
        let node_b = InlineNode::CharRef {
            value: CharRef::Special('&'),
            location: loc_b,
        };
        let piece_b = Piece {
            node_index: 1,
            s_start: 1,
            s_len: 5,
            src_offset: loc_b.byte_offset(),
            src_len: loc_b.data().len(),
            atomic: true,
            synthesized: false,
        };

        let nodes = [node_a, node_b];
        let pieces = [piece_a, piece_b];

        assert!(text_slice(&nodes, &pieces, 0..6).is_none());
    }

    #[test]
    fn text_slice_returns_none_across_an_atomic_piece() {
        use super::{Piece, text_slice};
        use crate::{
            Span,
            inlines::{CharRef, InlineNode},
        };

        // Mirrors `range_is_verbatim_or_synthesized`'s own atomic rejection:
        // an escaped special (or a rendered span) is cloned whole by
        // `emit_range`, so it never reduces to a `Text` node `text_slice` can
        // return a value for.
        let location = Span::new("&amp;");
        let node = InlineNode::CharRef {
            value: CharRef::Special('&'),
            location,
        };
        let piece = Piece {
            node_index: 0,
            s_start: 0,
            s_len: 5,
            src_offset: location.byte_offset(),
            src_len: location.data().len(),
            atomic: true,
            synthesized: false,
        };

        assert!(
            text_slice(
                std::slice::from_ref(&node),
                std::slice::from_ref(&piece),
                0..5
            )
            .is_none()
        );
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
    fn a_restored_entity_contributes_its_own_bytes_to_the_match_string() {
        // The two `CharRef` leaves are the atomic pieces `build_match_string`
        // gives real bytes to: a `Special` its canonical entity, and an
        // `Entity` the entity itself. Both are the bytes this position holds,
        // which is what lets a family read a
        // value across one; both stay `atomic`, since a leaf is one
        // indivisible node.
        let nodes = vec![
            InlineNode::Text {
                value: CowStr::from("a"),
                location: Span::new("a"),
            },
            InlineNode::CharRef {
                value: CharRef::Entity(CowStr::from("&copy;")),
                location: Span::new("&copy;"),
            },
            InlineNode::CharRef {
                value: CharRef::Special('&'),
                location: Span::new("&"),
            },
        ];

        let (s, pieces) = super::build_match_string(&nodes, Masked::UNKNOWN);

        assert_eq!(s, "a&copy;&amp;");
        assert_eq!(pieces.len(), 3);

        assert!(!pieces[0].atomic);

        assert_eq!(pieces[1].s_start, 1);
        assert_eq!(pieces[1].s_len, "&copy;".len());
        assert!(pieces[1].atomic);
        assert!(!pieces[1].synthesized);

        assert_eq!(pieces[2].s_start, "a&copy;".len());
        assert_eq!(pieces[2].s_len, "&amp;".len());
        assert!(pieces[2].atomic);
    }

    #[test]
    fn crossed_delimiters_are_a_documented_divergence() {
        // `` `a *b` c* `` interleaves a monospace and a strong span so their
        // ranges *overlap* rather than nest. The old string-substitution
        // implementation, rewriting a flat string, emitted crossed —
        // malformed — HTML tags (`<code>…<strong>…</code>…</strong>`) that no
        // tree can represent, and that recording is still this test's golden
        // oracle. The single-pass
        // builder instead treats an earlier span as opaque, so it produces a
        // well-formed tree (here, monospace wrapping a strong span). This is
        // the documented boundary of the single-pass recognition (see
        // the module docs): for pathological cross-span input the two
        // intentionally differ.
        let source = "`a *b` c*";

        let folded = fold_html(
            &build_through_quotes(Span::new(source)),
            &HtmlInlineRenderer {},
        );
        let golden = golden_quotes(source);

        // The golden recording's crossed tags: monospace matched *through* the
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

        // The delimiters are consumed; the child is the borrowed body,
        // precisely located just past the opening `*`.
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
        // The canonical nesting example: constrained emphasis matches inside
        // the body of the strong span an earlier sub created.
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
        // fold re-escapes it, matching Asciidoctor's own output (covered by
        // the corpus) while the structure exposes the entity.
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
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_quotes("\\*x*")
        );
    }

    #[test]
    fn an_escaped_quote_after_a_boundary_wraps_nothing() {
        // Regression guard for the escape-after-a-boundary case (`a \*x*`): the
        // backslash immediately before the delimiter is the constrained match's
        // *leading boundary group*, hence the first character of the whole
        // match, so it is still recognized as an escape. The construct must
        // stay literal rather than becoming a span.
        let nodes = build_src(Span::new("a \\*x*"));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Styled(_))),
            "an escaped quote after a boundary must not produce a span: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_quotes("a \\*x*")
        );
    }

    #[test]
    fn an_attributed_span_captures_its_roles() {
        let nodes = build_src(Span::new("[.lead]#tagline#"));

        match &nodes[0] {
            InlineNode::Styled(styled) => {
                // `#…#` with an attribute list downgrades from mark to an
                // unquoted span, matching Asciidoctor's own behavior.
                assert_eq!(styled.variant, StyleVariant::Unquoted);
                assert_eq!(styled.roles, vec![CowStr::from("lead")]);
                assert_ne!(
                    styled.attrs.attributes().len(),
                    0,
                    "the attribute list is retained"
                );

                // A wholly verbatim attribute list is parsed from its own
                // `'src` slice, so the node's list is located exactly there
                // (its values borrow) rather than falling back to
                // the coarse span an owned one takes.
                let attrs = &styled.attrs;
                assert_eq!(attrs.span().data(), ".lead");
                assert_eq!(attrs.span().line(), 1);
                assert_eq!(attrs.span().col(), 2);
            }

            other => panic!("expected Styled, got {other:?}"),
        }
    }

    #[test]
    fn an_attributed_spans_attribute_list_is_parsed_from_the_escaped_text() {
        // The structural counterpart of the corpus fixtures above: the role
        // and id a special-carrying attribute list yields are the *escaped*
        // bytes — parsed out of the level's own (already-escaped) match
        // string and rendered verbatim into the `class`/`id` attribute — not
        // the author's raw `<`/`&`.
        let source = "[#a&b.c<d]*bold*";
        let nodes = build_src(Span::new(source));

        match &nodes[0] {
            InlineNode::Styled(styled) => {
                assert_eq!(styled.id, Some(CowStr::from("a&amp;b")));
                assert_eq!(styled.roles, vec![CowStr::from("c&lt;d")]);

                // Those bytes have no `'src` slice of their own, so the list
                // is owned and takes the bracket's coarse source span as its
                // location tag — the same fallback an image's
                // bracket and a link's display-text list already take.
                let attrs = &styled.attrs;
                assert_eq!(attrs.span().data(), "#a&b.c<d");
                assert_eq!(attrs.span().line(), 1);
                assert_eq!(attrs.span().col(), 2);

                // The span itself still covers the whole construct, sliced
                // from `'src`.
                assert_eq!(styled.location.data(), source);
            }

            other => panic!("expected Styled, got {other:?}"),
        }
    }

    #[test]
    fn an_attribute_list_crossing_an_opaque_piece_is_a_documented_divergence() {
        // An *opaque* piece inside an attribute list — a masked passthrough
        // (`[.a+++x+++b]#y#`) or a span an earlier sub already rendered
        // (`[.a**b**c]#y#`) — is the one shape `attrlist_is_readable` rejects.
        // Splicing a passthrough's real text back in at parse time would let
        // a comma inside it split the attribute list, where the placeholder —
        // one atomic character no comma can hide behind — never can; a
        // rendered span's markup, likewise, exists only at fold time and has
        // no source bytes to parse a list from. So the construct is left
        // unrecognized — literal text, never a *wrong* node (which is what
        // the raw source slice used to yield here: a `class` of `a**b**c`) —
        // exactly as every macro family leaves its own opaque-piece boundary.
        //
        // If that boundary is ever lifted, fold these fixtures into the
        // parity corpus above.
        let renderer = HtmlInlineRenderer {};

        for (source, golden_html) in [
            ("[.a+++x+++b]#y#", "<span class=\"axb\">y</span>"),
            ("[.a$$x$$b]#y#", "<span class=\"axb\">y</span>"),
            (
                "[.a**b**c]#y#",
                "<span class=\"a<strong>b</strong>c\">y</span>",
            ),
            (
                "[role=\"a *b* c\"]#y#",
                "<span class=\"a <strong>b</strong> c\">y</span>",
            ),
        ] {
            let nodes = build_src(Span::new(source));
            let folded = super::super::fold_html(
                &nodes,
                &renderer,
                &crate::Parser::default().render_context(),
            );
            let golden = golden_passthroughs(source);

            assert_eq!(golden, golden_html);
            assert_ne!(folded, golden, "expected a divergence for {source:?}");

            // The attributed construct itself is gone from the tree: what the
            // golden recording renders as an attributed `<span>` is left as
            // literal text (an earlier sub's own span, or the passthrough's
            // `Raw` leaf, still sits inside it).
            assert!(
                !folded.contains("<span"),
                "expected no attributed span for {source:?}, got {folded:?}"
            );
        }
    }

    #[test]
    fn a_quoted_role_reads_the_attribute_lists_substituted_text() {
        // A quote-delimited first positional is the one thing an attribute
        // list yields from its own *text* rather than from a parsed attribute
        // (`quoted_text_fallback_role`, mirroring the `else` branch of
        // Asciidoctor's `parse_quoted_text_attributes`, which takes the role
        // verbatim — quote characters included). `Attrlist::parse` expands
        // attribute references over the whole list before splitting it, so
        // that accessor reads the *expanded* text, exactly as
        // `parse_quoted_text_attributes` reads the string its own
        // `sub_attributes` returned.
        //
        // Every family that parses an attribute list therefore agrees on a
        // quoted role, whichever step recognized it: the quotes step, whose
        // list is a slice of the buffer (`['{myrole}']*bold*`), and the
        // passthrough-extraction step, whose list is substituted at
        // *restore* time instead (`['{myrole}']++text++`, see
        // `substitute_and_restore`). The unquoted spellings — a bare
        // positional, a shorthand role, an id — never took this path at all,
        // and are here to pin that they still do not.
        let parser = crate::Parser::default().with_intrinsic_attribute(
            "myrole",
            "highlight",
            crate::parser::ModificationContext::Anywhere,
        );

        for (source, golden_html) in [
            // The quoted positional, in each family that parses a list.
            (
                "['{myrole}']*bold*",
                "<strong class=\"'highlight'\">bold</strong>",
            ),
            (
                "['{myrole}']#text#",
                "<span class=\"'highlight'\">text</span>",
            ),
            (
                "['{myrole}']`code`",
                "<code class=\"'highlight'\">code</code>",
            ),
            (
                "['{myrole}']++text++",
                "<span class=\"'highlight'\">text</span>",
            ),
            (
                "['{myrole}']+text+",
                "<span class=\"'highlight'\">text</span>",
            ),
            // A named attribute after the quoted positional: the role is the
            // source up to the first comma, so the tail changes nothing.
            (
                "['{myrole}',foo]++text++",
                "<span class=\"'highlight'\">text</span>",
            ),
            // A comma *inside* the quotes still truncates the role there
            // (Asciidoctor's `str.slice 0, (str.index ',')` runs after the
            // substitution, so an expansion introducing one truncates too).
            ("['a,{myrole}']#text#", "<span class=\"'a\">text</span>"),
            // A missing attribute leaves the reference alone under the default
            // `attribute-missing=skip`, so the expansion is a no-op and the
            // raw text is what the role was already reading.
            (
                "['{missing}']#text#",
                "<span class=\"'{missing}'\">text</span>",
            ),
            // The unquoted spellings, which read a parsed attribute instead.
            (
                "[{myrole}]++text++",
                "<span class=\"highlight\">text</span>",
            ),
            (
                "[.{myrole}]++text++",
                "<span class=\"highlight\">text</span>",
            ),
            ("[#{myrole}]++text++", "<span id=\"highlight\">text</span>"),
            (
                "[{myrole}]*bold*",
                "<strong class=\"highlight\">bold</strong>",
            ),
        ] {
            // `golden_html` is the golden recording's rendering,
            // frozen in the fixture at the last differentially-verified
            // parity.
            let folded = super::super::fold_html(
                &super::super::build(Span::new(source), &parser, None),
                &HtmlInlineRenderer {},
                &parser.render_context(),
            );

            assert_eq!(folded, golden_html, "mismatch for {source:?}");
        }
    }

    #[test]
    fn an_attribute_list_rewritten_by_a_later_step_is_a_documented_divergence() {
        // The complement of the boundary above, and not one this step can
        // draw: under the *normal* order the steps that run **after** quotes
        // (character replacements) used to go on matching over the whole
        // rendered string — the markup the quotes step just wrote
        // included — so they rewrote bytes that live only inside a rendered
        // `class`/`id` attribute. A later *sub* of this same step did it too
        // (`[.a~b~c]#y#`, whose subscript sub runs after the unquoted one that
        // consumed the attribute list). A tree's markup exists at fold time
        // alone, and its later transducers see the nodes, not the tags, so an
        // attribute list is whatever the sub that recognized it parsed, and
        // nothing rewrites it afterwards.
        //
        // This is the same class as `flatten_prior_markup`'s own case
        // — a step acting on
        // another step's emitted markup — seen from the other side, and it
        // costs three shapes: a typographic replacement, a *restored* entity
        // (whose escaped `&amp;amp;` the replacements step unwinds one level
        // in the rendered markup), and a later sub's own span.
        //
        // The **attribute-references** step used to cost a fourth
        // (`['{myrole}']*bold*`), and no longer does: that one was never
        // really about a later step reading emitted markup, but about
        // `Attrlist::parse` discarding the substituted text one accessor
        // needs — see
        // `a_quoted_role_reads_the_attribute_lists_substituted_text`
        // above. What is left here is genuinely markup-reading.
        let parser = crate::Parser::default();

        for (source, golden_html) in [
            ("[.a(C)c]#x#", "<span class=\"a&#169;c\">x</span>"),
            (
                "[.a&amp;b]*bold*",
                "<strong class=\"a&amp;b\">bold</strong>",
            ),
            ("[.a~b~c]#y#", "<span class=\"a<sub>b</sub>c\">y</span>"),
        ] {
            // `golden_html` is what the old string-substitution
            // implementation rendered — the recorded half of the divergence,
            // frozen in the fixture now that it is gone.
            let folded = super::super::fold_html(
                &super::super::build(Span::new(source), &parser, None),
                &HtmlInlineRenderer {},
                &parser.render_context(),
            );

            assert_ne!(folded, golden_html, "expected a divergence for {source:?}");
        }
    }
}
