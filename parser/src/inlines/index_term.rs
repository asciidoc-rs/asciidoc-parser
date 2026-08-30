use crate::{HasSpan, Span, inlines::InlineNode, strings::CowStr};

/// An index term (`((term))`, `(((primary, secondary, tertiary)))`,
/// `indexterm:[…]`, or `indexterm2:[…]`).
///
/// The known gap is [`terms`](Self::terms), which holds less than its name
/// promises — see that field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexTerm<'src> {
    /// The term levels, from primary to tertiary — the field's *intent*, and
    /// more than it currently holds.
    ///
    /// What the builder records today is at most **one** level, and only for a
    /// **visible** term: its shown text, as a single already-substituted
    /// string. It is empty in two cases — when no such string can express the
    /// shown text (the term encloses a construct whose rendering exists only
    /// at fold time, `((*tiger*))`, and [`children`](Self::children) is the
    /// authoritative form instead), and for a **concealed** term, whose
    /// argument is not reconstructed at all because none of it reaches the
    /// flow.
    ///
    /// So a consumer building an index cannot yet recover the authored
    /// primary/secondary/tertiary levels of an `indexterm:[p, s, t]` or
    /// `(((p, s, t)))` — which are exactly the terms that exist *only* to be
    /// indexed. Recording them is a change to what the builder parses; the
    /// field is named and typed for it so that filling it in later is
    /// additive.
    pub terms: Vec<CowStr<'src>>,

    /// The shown text of a *visible* term, as the child inline nodes it
    /// encloses.
    ///
    /// This is what the fold renders, and it is the shown text's authoritative
    /// form: a term's text is a *region of the document*, not an opaque
    /// string, and anything written inside it — a link, an anchor, a
    /// cross-reference, an image, a formatted span — is a construct in its own
    /// right, recognized exactly as it would be outside the parentheses.
    ///
    /// [`terms`](Self::terms) carries the same text as the single
    /// already-substituted string
    /// [`render_index_term`](crate::parser::InlineRenderer::render_index_term)
    /// takes as its `visible_term`,
    /// whenever one can express it. One cannot when the term encloses a
    /// construct whose rendering exists only at fold time — `((*tiger*))`,
    /// whose primary term is a bold span — and `terms` is then empty. The two
    /// are built from the same source range and agree by construction, so a
    /// consumer wanting the plain text may read `terms` and fall back to
    /// folding these nodes.
    ///
    /// Empty for a concealed term, which shows nothing.
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
