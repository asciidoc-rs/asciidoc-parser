use crate::{Span, attributes::Attrlist, content::inlines::InlineNode, strings::CowStr};

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

    /// The full attribute list, when the span carried one.
    pub attrs: Option<Attrlist<'src>>,

    /// The span's content, as child inline nodes.
    pub children: Vec<InlineNode<'src>>,

    /// The source location of the whole span.
    pub location: Span<'src>,
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
    /// Constrained form – the single-delimiter syntax (`*bold*`), which
    /// requires word boundaries around the span.
    Constrained,

    /// Unconstrained form – the doubled-delimiter syntax (`**bold**`), which
    /// may appear mid-word.
    Unconstrained,
}
