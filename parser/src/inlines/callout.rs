use crate::{HasSpan, Span, strings::CowStr};

/// A callout number in verbatim content (`<1>`, `<.>`, or `<!--1-->` for
/// XML).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Callout<'src> {
    /// The callout number to display. For an automatically-numbered callout
    /// (`<.>`) this is the resolved sequential number.
    pub number: CowStr<'src>,

    /// The guard that hides the callout in the raw verbatim source, so the
    /// fold can preserve it when icons are not enabled (see
    /// [`CalloutGuard`]).
    pub guard: CalloutGuard<'src>,

    /// The source location of the callout.
    pub location: Span<'src>,
}

impl<'src> HasSpan<'src> for Callout<'src> {
    fn span(&self) -> Span<'src> {
        self.location
    }
}

/// Describes the characters that guard (hide) a callout number in verbatim
/// source. Mirrors [`crate::parser::CalloutGuard`], the render-time
/// counterpart the fold reconstructs this into, but is decoupled from it (as
/// every node in this module is decoupled from its `*RenderParams`
/// counterpart) so the node stays canonical, structured data rather than a
/// render-seam type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CalloutGuard<'src> {
    /// A line-comment (or absent) guard. Holds the line-comment prefix that
    /// precedes the callout in the source (e.g. `# `), or an empty string
    /// when the callout is not tucked behind a line comment.
    LineComment(CowStr<'src>),

    /// An XML comment guard (`<!--N-->`).
    Xml,
}
