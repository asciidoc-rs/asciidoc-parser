use crate::{HasSpan, Span, inlines::InlineNode, strings::CowStr};

/// A footnote (`footnote:[…]`) or a reference to an earlier one
/// (`footnote:id[]`).
///
/// The footnote's number is resolved into the node at parse time, so rendering
/// stays a pure fold. Field set is provisional (Phase 0) and will be refined
/// against the first consumer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Footnote<'src> {
    /// The footnote's own ID, when it was given one.
    pub id: Option<CowStr<'src>>,

    /// The footnote's resolved number, or `None` for a reference whose ID was
    /// never defined.
    pub number: Option<CowStr<'src>>,

    /// `true` when this occurrence references an existing footnote (or fails to
    /// resolve one); `false` for the defining occurrence.
    pub is_reference: bool,

    /// The footnote's text, as child inline nodes. Empty for a bare reference.
    pub children: Vec<InlineNode<'src>>,

    /// The source location of the whole footnote macro.
    pub location: Span<'src>,
}

impl<'src> HasSpan<'src> for Footnote<'src> {
    fn span(&self) -> Span<'src> {
        self.location
    }
}
