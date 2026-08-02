use crate::{Span, content::inlines::InlineNode, strings::CowStr};

/// An inline anchor (`[[id]]`, `[[id,reftext]]`, or `anchor:id[reftext]`).
///
/// Field set is provisional (Phase 0) and will be refined against the first
/// consumer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Anchor<'src> {
    /// The anchor's ID.
    pub id: CowStr<'src>,

    /// The optional reference text, as child inline nodes. `None` when the
    /// anchor supplied no reftext.
    pub reftext: Option<Vec<InlineNode<'src>>>,

    /// The source location of the whole anchor.
    pub location: Span<'src>,
}
