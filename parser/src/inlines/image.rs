use crate::{HasSpan, Span, attributes::Attrlist, strings::CowStr};

/// An inline image (`image:target[…]`) or icon (`icon:target[…]`).
///
/// The two share one macro syntax and one node; [`is_icon`](Self::is_icon)
/// distinguishes them, because they render through different renderer methods
/// (an icon consults the `icons`/`icontype` document attributes and carries a
/// `size` rather than a `width`/`height`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Image<'src> {
    /// `true` for an `icon:` macro, `false` for an `image:` macro.
    pub is_icon: bool,

    /// The image target (the reference to the image), as written.
    pub target: CowStr<'src>,

    /// Byte ranges of [`target`](Self::target) whose content was restored from
    /// a masked passthrough or STEM expression (`image:++a b++[]`,
    /// `image:stem:[x].png[]`), in ascending order; empty when the target
    /// crossed none.
    ///
    /// Path resolution must treat each such range as one opaque run rather
    /// than as path syntax, so that none of its bytes — a space `web_path`
    /// would percent-encode, a backslash it would posixify, a `/` or `.` its
    /// segment arithmetic would read — ever reaches the resolver. The
    /// built-in renderer masks these ranges around its own `web_path` call —
    /// see [`image_uri`](crate::parser::InlineRenderer::image_uri), which
    /// receives them off this node.
    pub restored_target_ranges: Vec<std::ops::Range<usize>>,

    /// The alt text, explicit or defaulted.
    pub alt: Option<CowStr<'src>>,

    /// The requested width, if any. Not validated as a number.
    pub width: Option<CowStr<'src>>,

    /// The requested height, if any. Not validated as a number.
    pub height: Option<CowStr<'src>>,

    /// The image macro's full attribute list —
    /// [`Attrlist::empty`](crate::attributes::Attrlist::empty) when it carried
    /// none, so a consumer reads attributes the same way either way.
    pub attrs: Attrlist<'src>,

    /// The source location of the whole image macro.
    pub location: Span<'src>,
}

impl<'src> HasSpan<'src> for Image<'src> {
    fn span(&self) -> Span<'src> {
        self.location
    }
}
