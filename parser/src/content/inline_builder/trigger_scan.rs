//! One fused trigger scan for the whole substitution pipeline.
//!
//! Nearly every step opens with its own cheap sniff over a lone
//! [`Text`](InlineNode::Text) node's value — quotes' marker mask, the macros
//! step's trigger-byte gate, the attribute step's `{` probe, and so on. For
//! the overwhelmingly common content shape (a plain paragraph that no step
//! touches), that means walking the same bytes once per step just to conclude
//! "nothing here" each time.
//!
//! [`LoneTextSniff`] collapses those walks into **one** pass: [`trigger_mask`]
//! classifies each byte into the union of every step's
//! trigger bytes, ORing the classes together, and
//! [`build_for_group`](super::build_for_group) then skips a step's whole call
//! when the mask holds none of that step's bytes. The gate fires only while
//! the tree is a single `Text` node — the one shape whose match string *is*
//! its value — and every step that does run invalidates the cached mask (see
//! [`LoneTextSniff`] for when a rescan is really needed), so a step that
//! transforms the tree can never leave a stale answer behind and every later
//! step runs normally.
//!
//! Each step's class here must stay **conservative** against that step's own
//! sniff: it may name more bytes than the step needs, never fewer. A byte the
//! class over-names costs one redundant step call (the step's own sniff then
//! declines); a byte it under-named would silently disable a construct. The
//! per-step reasoning lives on each constant below, and
//! `crate::tests::inline_builder_trigger_scan` pins every class against its
//! step's own recognizers, so a future needle cannot drift out of its class
//! unnoticed.

use crate::{inlines::InlineNode, strings::CowStr};

/// The passthrough-extraction pass. Union of its two phases' own sniffs: the
/// macro/doubled forms (`++`, `$$`, and `pass:`/`ss:`) and the bare or
/// attrlisted single-`+` form, whose `-]` needle carries the `]` here.
pub(crate) const PASSTHROUGH_TRIGGERS: u32 = bit(b'+') | bit(b'$') | bit(b':') | bit(b']');

/// Inline STEM extraction: every spelling its sniff admits (`stem:`,
/// `asciimath:`, `latexmath:`) requires the colon.
pub(crate) const STEM_TRIGGERS: u32 = bit(b':');

/// The specialcharacters step (and the end-of-group
/// [`classify_unescaped_specials`](super::special_chars::classify_unescaped_specials)
/// sweep): the three characters it escapes or classifies.
pub(crate) const SPECIAL_TRIGGERS: u32 = bit(b'<') | bit(b'>') | bit(b'&');

/// The quotes step: the eight `sub_markers` characters (see that function in
/// the quotes module). Every quote sub requires *all* of its own markers, so
/// a value holding none of the eight can satisfy no sub at all.
pub(crate) const QUOTE_TRIGGERS: u32 =
    bit(b'*') | bit(b'"') | bit(b'\'') | bit(b'`') | bit(b'_') | bit(b'#') | bit(b'^') | bit(b'~');

/// The attribute-references step: a reference or counter directive is always
/// spelled `{…}`.
pub(crate) const ATTRIBUTE_TRIGGERS: u32 = bit(b'{');

/// The character-replacements step: one byte from each alternative of its own
/// sniff (`[&']|--|\.\.\.|\([CRT]M?\)` — see
/// [`maybe_has_replacements`](crate::content::maybe_has_replacements)).
pub(crate) const REPLACEMENT_TRIGGERS: u32 =
    bit(b'&') | bit(b'\'') | bit(b'-') | bit(b'.') | bit(b'(');

/// The macros step: the same five bytes as
/// [`level_may_have_macros`](super::macros::level_may_have_macros), which
/// documents why they cover every macro family. The footnotes pass rides on
/// this same gate: its constructs (`footnote:[…]`, `footnoteref:[…]`) always
/// carry the colon.
pub(crate) const MACRO_TRIGGERS: u32 = bit(b':') | bit(b'[') | bit(b'(') | bit(b'@') | bit(b'&');

/// The post-replacements step: a hard break needs the ` +` marker — except
/// under the `hardbreaks` option, where every `\n` becomes one, which is why
/// the newline is in this class rather than only `+`.
pub(crate) const POST_REPLACEMENT_TRIGGERS: u32 = bit(b'+') | bit(b'\n');

/// The bit for one trigger byte — the single source both the step classes
/// above and [`trigger_mask`]'s per-byte classification are built from. A
/// byte outside every class answers `0`; the classification is per **byte**,
/// which cannot miss a trigger inside a multi-byte character since every
/// trigger byte is ASCII and never occurs in a multi-byte character's
/// encoding.
const fn bit(b: u8) -> u32 {
    match b {
        b'+' => 1 << 0,
        b'$' => 1 << 1,
        b':' => 1 << 2,
        b']' => 1 << 3,
        b'<' => 1 << 4,
        b'>' => 1 << 5,
        b'&' => 1 << 6,
        b'*' => 1 << 7,
        b'"' => 1 << 8,
        b'\'' => 1 << 9,
        b'`' => 1 << 10,
        b'_' => 1 << 11,
        b'#' => 1 << 12,
        b'^' => 1 << 13,
        b'~' => 1 << 14,
        b'{' => 1 << 15,
        b'-' => 1 << 16,
        b'.' => 1 << 17,
        b'(' => 1 << 18,
        b'[' => 1 << 19,
        b'@' => 1 << 20,
        b'\n' => 1 << 21,

        _ => 0,
    }
}

/// The fused scan itself: the trigger classes present anywhere in `value`.
pub(crate) fn trigger_mask(value: &str) -> u32 {
    let mut mask = 0u32;

    for b in value.bytes() {
        mask |= bit(b);
    }

    mask
}

/// The one-pass replacement for each step's own lone-`Text` sniff, cached
/// across the step loop.
///
/// [`may`](Self::may) answers whether a step whose trigger class is
/// `triggers` could possibly act on `nodes`: `false` only when the tree is a
/// single `Text` node whose value holds none of the class's bytes — the
/// caller then skips the step's call entirely. Any other tree shape answers
/// `true`, leaving the decision to the step's own machinery.
///
/// The cached mask is reused on two grounds, each sound on its own:
///
/// - **Freshness** — no step has run since the scan
///   ([`invalidate`](Self::invalidate) is called after every step that does),
///   so the node the mask describes is untouched, whatever its [`CowStr`]
///   variant.
/// - **A matching borrowed identity** — a [`Borrowed`](CowStr::Borrowed)
///   value's bytes live in the source, which outlives the whole build, so the
///   same address and length can only ever name the same bytes again. An
///   *owned* value earns no such key: a step can free it and the allocator can
///   hand its address, at the same length, to a different replacement value
///   (and an inlined value's address is just its slot in the node vector) —
///   those rescan once a step has run, which only ever happens on a level that
///   carries trigger bytes anyway.
#[derive(Default)]
pub(crate) struct LoneTextSniff {
    /// The address/length identity of the **borrowed** lone value `mask`
    /// describes; `None` when the last scanned value was not borrowed.
    borrowed_key: Option<(usize, usize)>,

    /// [`trigger_mask`] of the last-scanned lone value.
    mask: u32,

    /// Whether no step has run since `mask` was computed.
    fresh: bool,
}

impl LoneTextSniff {
    pub(crate) fn may(&mut self, nodes: &[InlineNode<'_>], triggers: u32) -> bool {
        let [InlineNode::Text { value, .. }] = nodes else {
            return true;
        };

        let borrowed_key = match value {
            CowStr::Borrowed(text) => Some((text.as_ptr() as usize, text.len())),
            _ => None,
        };

        let reusable = self.fresh || (borrowed_key.is_some() && borrowed_key == self.borrowed_key);

        if !reusable {
            self.mask = trigger_mask(value.as_ref());
            self.borrowed_key = borrowed_key;
        }

        self.fresh = true;
        self.mask & triggers != 0
    }

    /// Records that a step ran, whether or not it changed the tree; the next
    /// [`may`](Self::may) then rescans unless the value's borrowed identity
    /// vouches for the cached mask.
    pub(crate) fn invalidate(&mut self) {
        self.fresh = false;
    }
}
