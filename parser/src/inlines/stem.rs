use crate::{HasSpan, Span, content::SubstitutionGroup, inlines::InlineNode, strings::CowStr};

/// Inline STEM content (`stem:[…]`, `asciimath:[…]`, `latexmath:[…]`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stem<'src> {
    /// The notation the expression is written in.
    pub notation: StemNotation,

    /// The expression, with its resolved substitution group already applied
    /// (special characters only, by default — [`SubstitutionGroup::Stem`]).
    /// This mirrors a passthrough [`Raw`](crate::inlines::InlineNode::Raw)
    /// node's `value`, which likewise holds already-substituted content
    /// rather than the untouched source slice: STEM is an implicit
    /// passthrough (extracted before every other step), so nothing further
    /// acts on this text before the fold wraps it via
    /// `render_styled`.
    ///
    /// [`SubstitutionGroup::Stem`]: crate::content::SubstitutionGroup::Stem
    pub value: CowStr<'src>,

    /// The substitution group this expression's body is restored under, as
    /// the extraction pass resolved it: [`SubstitutionGroup::Stem`] for a bare
    /// macro (special characters only), or whatever an explicit list spells
    /// out (`stem:c,q[…]`, `stem:n[…]`).
    ///
    /// STEM is an *implicit* passthrough, so this is the same fact
    /// [`RawOrigin::Passthrough`](crate::inlines::RawOrigin)'s own `subs`
    /// records for an explicit one, and what
    /// [`Passthrough::subs`](crate::content::Passthrough::subs) returns. The
    /// node kind alone cannot supply it, since the group varies by spelling.
    ///
    /// [`SubstitutionGroup::Stem`]: crate::content::SubstitutionGroup::Stem
    pub subs: SubstitutionGroup,

    /// The author's expression **before** [`subs`](Self::subs) was applied,
    /// when [`value`](Self::value) is not it.
    ///
    /// `value` holds already-substituted text — `p &lt; q` for `stem:[p < q]`
    /// — where
    /// [`Passthrough::text`](crate::content::Passthrough::text) returns the
    /// author's `p < q`. This records that input, exactly as
    /// [`RawOrigin::Passthrough`](crate::inlines::RawOrigin)'s own
    /// `source_text` does for a `pass:c,q[…]` body, and is `None` when the
    /// group changed nothing.
    pub source_text: Option<String>,

    /// The expression body's own nodes — the [`Text`](InlineNode::Text) runs it
    /// is written from, and any [`Raw`](InlineNode::Raw) passthrough the
    /// extraction pass had already pulled out of it before this macro was
    /// recognized (`stem:[x +++<b>+++ y]`).
    ///
    /// [`value`](Self::value) is the *rendering* of these, which is what the
    /// fold emits; this is what they **are**. The two are redundant for the
    /// overwhelmingly common flat body — one `Text` run — and differ exactly
    /// when the body embeds a passthrough, which is the case they exist for: an
    /// embedded one is a [`Passthrough`](crate::content::Passthrough) entry in
    /// its own right, and folding it into `value` (as this node used to) left a
    /// walk no way to reach it.
    ///
    /// A body whose expression this module declined to recognize builds no
    /// `Stem` at all, so there is no partial state here: a node either holds
    /// its whole body or does not exist.
    pub children: Vec<InlineNode<'src>>,

    /// The source location of the whole STEM macro.
    pub location: Span<'src>,
}

impl<'src> HasSpan<'src> for Stem<'src> {
    fn span(&self) -> Span<'src> {
        self.location
    }
}

/// The notation of an inline [`Stem`] expression.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StemNotation {
    /// AsciiMath notation (the default, and `asciimath:[…]`).
    AsciiMath,

    /// LaTeX notation (`latexmath:[…]`).
    LatexMath,
}
