use crate::{HasSpan, Span, inlines::InlineNode, strings::CowStr};

/// An index term (`((term))`, `(((primary, secondary, tertiary)))`,
/// `indexterm:[…]`, or `indexterm2:[…]`).
///
/// Field set is provisional (Phase 0) and will be refined against the first
/// consumer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexTerm<'src> {
    /// The term levels, from primary to tertiary.
    ///
    /// Empty when the shown text is carried by [`children`](Self::children)
    /// instead.
    pub terms: Vec<CowStr<'src>>,

    /// The shown text of a *visible* term, as child inline nodes — used when
    /// that text crosses a construct whose own rendering it contains, which an
    /// already-substituted string cannot express.
    ///
    /// A visible term's text is normally the single already-substituted string
    /// in [`terms`](Self::terms), because that is what
    /// [`IndexTermRenderParams`](crate::parser::IndexTermRenderParams) takes
    /// and what the term's own source spells. But a term may enclose an
    /// earlier-recognized construct — `((*tiger*))`, whose primary term is a
    /// bold span — and *that* construct's rendering exists only when the tree
    /// is folded, so it cannot be baked into a string at parse time. Such a
    /// term carries its text here instead, as the nodes it encloses, and the
    /// fold renders them with the same renderer as the surrounding flow.
    ///
    /// Empty for a concealed term (which shows nothing) and for every visible
    /// term whose text [`terms`](Self::terms) already carries; the two are
    /// never both populated.
    pub children: Vec<InlineNode<'src>>,

    /// `true` for a *flow* term (`((term))` / `indexterm2:[…]`), whose primary
    /// term is also shown in the flow of text; `false` for a *concealed* term,
    /// which produces no visible output.
    pub visible: bool,

    /// The source location of the whole index term.
    pub location: Span<'src>,
}

impl<'src> HasSpan<'src> for IndexTerm<'src> {
    fn span(&self) -> Span<'src> {
        self.location
    }
}
