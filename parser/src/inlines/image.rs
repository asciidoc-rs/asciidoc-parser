use crate::{HasSpan, Span, attributes::Attrlist, strings::CowStr};

/// An inline image (`image:target[…]`) or icon (`icon:target[…]`).
///
/// The two share one macro syntax and one node; [`is_icon`](Self::is_icon)
/// distinguishes them, because they render through different renderer methods
/// (an icon consults the `icons`/`icontype` document attributes and carries a
/// `size` rather than a `width`/`height`).
///
/// Field set is provisional (Phase 0) and will be refined against the first
/// consumer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Image<'src> {
    /// `true` for an `icon:` macro, `false` for an `image:` macro.
    pub is_icon: bool,

    /// The image target (the reference to the image), as written.
    pub target: CowStr<'src>,

    /// The alt text, explicit or defaulted.
    pub alt: Option<CowStr<'src>>,

    /// The requested width, if any. Not validated as a number.
    pub width: Option<CowStr<'src>>,

    /// The requested height, if any. Not validated as a number.
    pub height: Option<CowStr<'src>>,

    /// The full attribute list, when the image macro carried one.
    pub attrs: Option<Attrlist<'src>>,

    /// The source location of the whole image macro.
    pub location: Span<'src>,
}

impl<'src> HasSpan<'src> for Image<'src> {
    fn span(&self) -> Span<'src> {
        self.location
    }
}
