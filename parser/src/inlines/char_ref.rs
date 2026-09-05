use crate::strings::CowStr;

/// The logical payload of an [`InlineNode::CharRef`]: a character reference or
/// typographic replacement whose concrete output entity is chosen by the fold
/// (the renderer), not stored in the tree.
///
/// [`InlineNode::CharRef`]: super::InlineNode::CharRef
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharRef<'src> {
    /// A special character that the HTML fold escapes (`<` → `&lt;`, `>` →
    /// `&gt;`, `&` → `&amp;`).
    Special(char),

    /// A replacement whose logical value is one or more characters — for
    /// example `(C)` → `©`, `--` → an em dash, or `...` → an ellipsis. The
    /// stored string is the logical character(s); the fold decides how to
    /// encode them.
    Replacement(&'static str),

    /// An explicit numeric or named entity written by the author (for example
    /// `&#8217;` or `&hellip;`), carried through as written.
    Entity(CowStr<'src>),
}
