use crate::{HasSpan, Span, strings::CowStr};

/// A callout number in verbatim content (`<1>`, `<.>`).
///
/// Field set is provisional (Phase 0) and will be refined against the first
/// consumer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Callout<'src> {
    /// The callout number to display. For an automatically-numbered callout
    /// (`<.>`) this is the resolved sequential number.
    pub number: CowStr<'src>,

    /// The source location of the callout.
    pub location: Span<'src>,
}

impl<'src> HasSpan<'src> for Callout<'src> {
    fn span(&self) -> Span<'src> {
        self.location
    }
}
