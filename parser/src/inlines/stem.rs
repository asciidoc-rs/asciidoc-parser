use crate::{HasSpan, Span, strings::CowStr};

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
