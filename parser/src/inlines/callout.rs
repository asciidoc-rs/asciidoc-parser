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
/// source, so a fold can preserve them when icons are not enabled.
///
/// This used to be mirrored by a render-time `parser::CalloutGuard` that the
/// fold converted into, on the reasoning that a node should stay canonical
/// structured data rather than a render-seam type. That duplicate is retired:
/// [`render_callout`](crate::parser::InlineRenderer::render_callout) takes
/// the [`Callout`] node itself, so the node *is* what the seam carries, and a
/// second enum saying the same thing in `&str` where this one says
/// [`CowStr`] would buy nothing but a conversion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CalloutGuard<'src> {
    /// A line-comment (or absent) guard. Holds the line-comment prefix that
    /// precedes the callout in the source (e.g. `# `), or an empty string
    /// when the callout is not tucked behind a line comment.
    LineComment(CowStr<'src>),

    /// An XML comment guard (`<!--N-->`).
    Xml,
}
