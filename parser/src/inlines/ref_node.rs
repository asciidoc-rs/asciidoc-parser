use crate::{
    HasSpan, Span,
    inlines::InlineNode,
    parser::{DerivedReference, ResolvedReference},
    strings::CowStr,
};

/// A link or cross-reference. ASG: `inlineRef`.
///
/// The display text is held as child [`InlineNode`]s, so a formatted link text
/// (`link:x[*bold*]`) is a tree. For a cross-reference, [`resolved`] is filled
/// in by the resolution pass and remains `None` while unresolved or for a
/// standalone (document-less) parse; [`derived`] is filled in at build time
/// instead, for a target that carries its own destination without any catalog.
///
/// [`resolved`]: Self::resolved
/// [`derived`]: Self::derived
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ref<'src> {
    /// Whether this is a link or a cross-reference.
    pub variant: RefVariant,

    /// The reference target.
    ///
    /// For a [`Link`](RefVariant::Link) this is the raw target as written in
    /// the source (for example `https://example.org` or `index.html`).
    ///
    /// For an [`Xref`](RefVariant::Xref) this is the *interpreted* target that
    /// drives the rendered `href` and the catalog resolution, not the verbatim
    /// source spelling: a same-document reference's leading `#` is removed
    /// (`xref:#install[]` is stored as `install`), while an inter-document
    /// target is kept as written (`other.adoc#frag`). This mirrors the
    /// value the renderer and the resolution pass use, so the two
    /// representations cannot disagree.
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

    /// For a cross-reference whose target names another document, or the
    /// empty target naming the current document as a whole, the destination
    /// derived from the target itself — computed once, at build time, without
    /// consulting any catalog (mirroring the string pipeline's own target
    /// interpretation). `None` for a same-document reference to a specific id,
    /// which resolves through [`resolved`](Self::resolved) instead, and always
    /// `None` for a [`Link`](RefVariant::Link).
    pub derived: Option<DerivedReference>,

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
