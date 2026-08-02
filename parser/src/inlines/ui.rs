use crate::{HasSpan, Span, strings::CowStr};

/// A UI macro: `kbd:`, `btn:`, or `menu:`.
///
/// Field set is provisional (Phase 0) and will be refined against the first
/// consumer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ui<'src> {
    /// Which UI macro this is, with its parsed contents.
    pub kind: UiKind<'src>,

    /// The source location of the whole UI macro.
    pub location: Span<'src>,
}

impl<'src> HasSpan<'src> for Ui<'src> {
    fn span(&self) -> Span<'src> {
        self.location
    }
}

/// The kind of a [`Ui`] macro, with its parsed contents.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UiKind<'src> {
    /// A keyboard macro (`kbd:[Ctrl+T]`), holding one or more keys.
    Keyboard(Vec<CowStr<'src>>),

    /// A button macro (`btn:[Save]`), holding the button label.
    Button(CowStr<'src>),

    /// A menu macro (`menu:File[Save As]`).
    Menu {
        /// The top-level menu name.
        menu: CowStr<'src>,

        /// Zero or more intermediate submenu names, outermost first.
        submenus: Vec<CowStr<'src>>,

        /// The final menu item, or `None` for a bare menu reference.
        item: Option<CowStr<'src>>,
    },
}
