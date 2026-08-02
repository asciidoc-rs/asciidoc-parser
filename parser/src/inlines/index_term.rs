use crate::{Span, strings::CowStr};

/// An index term (`((term))`, `(((primary, secondary, tertiary)))`,
/// `indexterm:[…]`, or `indexterm2:[…]`).
///
/// Field set is provisional (Phase 0) and will be refined against the first
/// consumer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexTerm<'src> {
    /// The term levels, from primary to tertiary.
    pub terms: Vec<CowStr<'src>>,

    /// `true` for a *flow* term (`((term))` / `indexterm2:[…]`), whose primary
    /// term is also shown in the flow of text; `false` for a *concealed* term,
    /// which produces no visible output.
    pub visible: bool,

    /// The source location of the whole index term.
    pub location: Span<'src>,
}
