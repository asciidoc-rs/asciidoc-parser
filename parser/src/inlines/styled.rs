use crate::{
    HasSpan, Span, attributes::Attrlist, content::SubstitutionGroup, inlines::InlineNode,
    strings::CowStr,
};

/// A formatted span, such as strong, emphasis, code, or mark. ASG:
/// `inlineSpan`.
///
/// The span's content is held as child [`InlineNode`]s, so nested formatting
/// (`*a _b_ c*`) is a tree rather than a flat string with tags.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Styled<'src> {
    /// Which kind of span this is.
    pub variant: StyleVariant,

    /// Whether the span was written in constrained or unconstrained form.
    pub form: SpanForm,

    /// The span's ID, if one was assigned via a shorthand or attribute.
    pub id: Option<CowStr<'src>>,

    /// The roles (CSS classes) assigned to the span.
    pub roles: Vec<CowStr<'src>>,

    /// The span's full attribute list —
    /// [`Attrlist::empty`](crate::attributes::Attrlist::empty) when the span
    /// carried none, so a consumer reads attributes the same way either way.
    pub attrs: Attrlist<'src>,

    /// The span's content, as child inline nodes.
    pub children: Vec<InlineNode<'src>>,

    /// Set when the **passthrough-extraction pass** built this span as the
    /// wrapper for an attribute-list-prefixed passthrough
    /// (`[.role]++text++`, `` [x-]`text` ``), and `None` for every ordinary
    /// span.
    ///
    /// Such a wrapper is what the extraction pass records as one
    /// [`Passthrough`](crate::content::Passthrough) entry — the *span*, not
    /// anything inside it — so the record belongs here rather than on a child.
    /// It has to, for one spelling: the `x-` compatibility marker sends its
    /// body through the **normal** substitutions as a subtree, leaving no
    /// [`Raw`](InlineNode::Raw) leaf to carry anything, which is why the
    /// entry's group is `Normal` where every other prefixed spelling reads its
    /// group off the delimiters.
    ///
    /// One `Option` rather than two fields, because the two facts are
    /// meaningless apart and absent from the overwhelming majority of spans.
    pub passthrough: Option<PassthroughWrapper>,

    /// The source location of the whole span.
    pub location: Span<'src>,
}

/// The passthrough record an attribute-list-prefixed wrapper carries — see
/// [`Styled::passthrough`].
///
/// This is [`Stem`](crate::inlines::Stem)'s and
/// [`RawOrigin::Passthrough`](crate::inlines::RawOrigin)'s pair in the one
/// shape a *wrapper* can hold it: a wrapper has no `value` of its own, so the
/// author's body is unconditional here rather than the "when `value` is not
/// it" `Option` those two carry.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PassthroughWrapper {
    /// The substitution group the extraction pass resolved for the body.
    pub subs: SubstitutionGroup,

    /// The author's body, before that group was applied.
    pub text: String,
}

impl<'src> HasSpan<'src> for Styled<'src> {
    fn span(&self) -> Span<'src> {
        self.location
    }
}

/// The kind of a formatted [`Styled`] span.
///
/// The first four variants are ASG-native; the remainder are crate extensions
/// that project to the nearest ASG-legal form (typically a `span` carrying a
/// role) when emitting conformant ASG.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StyleVariant {
    /// Strong (`*bold*`). ASG `variant="strong"`.
    Strong,

    /// Emphasis (`_italic_`). ASG `variant="emphasis"`.
    Emphasis,

    /// Monospace/code (`` `code` ``). ASG `variant="code"`.
    Code,

    /// Highlight/mark (`#marked#`). ASG `variant="mark"`.
    Mark,

    /// Superscript (`^sup^`). Crate extension.
    Superscript,

    /// Subscript (`~sub~`). Crate extension.
    Subscript,

    /// Double-quoted smart-quote span (`"`…`"`). Crate extension.
    DoubleQuote,

    /// Single-quoted smart-quote span (`'`…`'`). Crate extension.
    SingleQuote,

    /// An unquoted span carrying only id/roles (`[.role]#…#`). Crate extension.
    Unquoted,
}

/// Whether a [`Styled`] span was written in constrained or unconstrained form.
/// ASG: `form`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpanForm {
    /// Constrained form — the single-delimiter syntax (`*bold*`), which
    /// requires word boundaries around the span.
    Constrained,

    /// Unconstrained form — the doubled-delimiter syntax (`**bold**`), which
    /// may appear mid-word.
    Unconstrained,
}
