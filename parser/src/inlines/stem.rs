use crate::{HasSpan, Span, content::SubstitutionGroup, strings::CowStr};

/// Inline STEM content (`stem:[…]`, `asciimath:[…]`, `latexmath:[…]`).
///
/// Field set is provisional (Phase 0) and will be refined against the first
/// consumer.
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
    /// `render_quoted_substitution`.
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
