use crate::{HasSpan, Span, inlines::InlineNode, strings::CowStr};

/// An inline anchor (`[[id]]`, `[[id,reftext]]`, or `anchor:id[reftext]`), or
/// the bibliography anchor (`[[[label]]]`, `[[[label,xreftext]]]`) that
/// prefixes a bibliography list item.
///
/// Field set is provisional (Phase 0) and will be refined against the first
/// consumer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Anchor<'src> {
    /// The anchor's ID.
    pub id: CowStr<'src>,

    /// The optional reference text, as child inline nodes. `None` when the
    /// anchor supplied no reftext.
    ///
    /// A **bibliography** anchor always carries one: the bracketed label
    /// (`[label]`, or `[xreftext]` when one was supplied) that a
    /// cross-reference to the entry displays. That same bracketed text is also
    /// rendered into the flow — as the sibling nodes that follow this one, not
    /// from this field (see [`is_bibliography`](Self::is_bibliography)).
    pub reftext: Option<Vec<InlineNode<'src>>>,

    /// True when this is a **bibliography** anchor (`[[[label]]]`) prefixing a
    /// bibliography list item, rather than an ordinary inline anchor.
    ///
    /// The two render differently — a bibliography anchor emits its empty `<a
    /// id="…"></a>` *followed by* the bracketed label, and registers the entry
    /// under [`RefType::Bibliography`](crate::document::RefType::Bibliography)
    /// rather than [`RefType::Anchor`](crate::document::RefType::Anchor) — so
    /// the two forms are told apart by this flag rather than by re-reading the
    /// node's source, exactly as an [`Image`](crate::inlines::Image) node's own
    /// `is_icon` tells its two forms apart.
    ///
    /// The bracketed label a bibliography anchor renders into the flow is
    /// **not** this node's own children: it is emitted as the sibling text
    /// nodes that follow, exactly as the string pipeline leaves that text in
    /// the flow for its own later passes to see.
    pub is_bibliography: bool,

    /// The source location of the whole anchor.
    pub location: Span<'src>,
}

impl<'src> HasSpan<'src> for Anchor<'src> {
    fn span(&self) -> Span<'src> {
        self.location
    }
}
