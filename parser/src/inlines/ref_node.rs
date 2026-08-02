use crate::{HasSpan, Span, inlines::InlineNode, parser::ResolvedReference, strings::CowStr};

/// A link or cross-reference. ASG: `inlineRef`.
///
/// The display text is held as child [`InlineNode`]s, so a formatted link text
/// (`link:x[*bold*]`) is a tree. For a cross-reference, [`resolved`] is filled
/// in by the resolution pass and remains `None` while unresolved or for a
/// standalone (document-less) parse.
///
/// [`resolved`]: Self::resolved
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ref<'src> {
    /// Whether this is a link or a cross-reference.
    pub variant: RefVariant,

    /// The raw target as written in the source.
    pub target: CowStr<'src>,

    /// The display text, as child inline nodes. Empty when none was supplied.
    pub children: Vec<InlineNode<'src>>,

    /// The roles (CSS classes) assigned to the reference.
    pub roles: Vec<CowStr<'src>>,

    /// The target window selection (for example `_blank`), if any.
    pub window: Option<CowStr<'src>>,

    /// For a cross-reference, the resolved destination, filled in by the
    /// resolution pass. `None` while unresolved, or for a standalone parse with
    /// no catalog.
    pub resolved: Option<ResolvedReference>,

    /// The source location of the whole reference.
    pub location: Span<'src>,
}

impl<'src> HasSpan<'src> for Ref<'src> {
    fn span(&self) -> Span<'src> {
        self.location
    }
}

/// Whether a [`Ref`] is a link or a cross-reference. ASG: `variant`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefVariant {
    /// A link to an external target (`link:`, `https:`, and similar). ASG
    /// `variant="link"`.
    Link,

    /// A cross-reference to an element within the document set (`xref:` or
    /// `<<id>>`). ASG `variant="xref"`.
    Xref,
}
