use crate::{Span, strings::CowStr};

/// Inline STEM content (`stem:[…]`, `asciimath:[…]`, `latexmath:[…]`).
///
/// Field set is provisional (Phase 0) and will be refined against the first
/// consumer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stem<'src> {
    /// The notation the expression is written in.
    pub notation: StemNotation,

    /// The raw expression, carried through verbatim.
    pub value: CowStr<'src>,

    /// The source location of the whole STEM macro.
    pub location: Span<'src>,
}

/// The notation of an inline [`Stem`] expression.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StemNotation {
    /// AsciiMath notation (the default, and `asciimath:[…]`).
    AsciiMath,

    /// LaTeX notation (`latexmath:[…]`).
    LatexMath,
}
