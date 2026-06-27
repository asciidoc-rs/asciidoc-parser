pub(crate) fn is_built_in_context(context: &str) -> bool {
    matches!(
        context,
        "admonition"
            | "audio"
            | "colist"
            | "dlist"
            | "document"
            | "example"
            | "floating_title"
            | "image"
            | "list_item"
            | "listing"
            | "literal"
            | "olist"
            | "open"
            | "page_break"
            | "paragraph"
            | "pass"
            | "preamble"
            | "quote"
            | "section"
            | "sidebar"
            | "table"
            | "table_cell"
            | "thematic_break"
            | "toc"
            | "ulist"
            | "verse"
            | "video"
    )
}

/// The five admonition types provided by the AsciiDoc language.
///
/// An admonition draws attention to a statement by taking it out of the
/// content's flow and labeling it with a priority. The variant is determined by
/// the assigned type (i.e., the uppercase label), which is specified either as
/// a special paragraph prefix (e.g., `NOTE:`) or as the block style in an
/// attribute list (e.g., `[NOTE]`).
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum AdmonitionVariant {
    /// The `NOTE` admonition type.
    Note,

    /// The `TIP` admonition type.
    Tip,

    /// The `IMPORTANT` admonition type.
    Important,

    /// The `CAUTION` admonition type.
    Caution,

    /// The `WARNING` admonition type.
    Warning,
}

impl AdmonitionVariant {
    /// Resolve an uppercase style label (e.g., `NOTE`) to its admonition
    /// variant, if it names one.
    ///
    /// The match is case-sensitive: the label must be uppercase, per the
    /// AsciiDoc language specification.
    pub(crate) fn from_style(style: &str) -> Option<Self> {
        Some(match style {
            "NOTE" => Self::Note,
            "TIP" => Self::Tip,
            "IMPORTANT" => Self::Important,
            "CAUTION" => Self::Caution,
            "WARNING" => Self::Warning,
            _ => return None,
        })
    }

    /// Returns the uppercase style label for this variant (e.g., `NOTE`).
    pub fn style(self) -> &'static str {
        match self {
            Self::Note => "NOTE",
            Self::Tip => "TIP",
            Self::Important => "IMPORTANT",
            Self::Caution => "CAUTION",
            Self::Warning => "WARNING",
        }
    }

    /// Returns the lowercase name for this variant (e.g., `note`).
    ///
    /// This is the name used for the block's CSS class and for the
    /// `<type>-caption` document attribute that controls the label text.
    pub fn name(self) -> &'static str {
        match self {
            Self::Note => "note",
            Self::Tip => "tip",
            Self::Important => "important",
            Self::Caution => "caution",
            Self::Warning => "warning",
        }
    }

    /// Returns the default caption (label) text for this variant (e.g.,
    /// `Note`).
    ///
    /// This text is shown to the reader (in place of an icon) unless it is
    /// overridden by setting the `<type>-caption` document attribute.
    pub fn default_caption(self) -> &'static str {
        match self {
            Self::Note => "Note",
            Self::Tip => "Tip",
            Self::Important => "Important",
            Self::Caution => "Caution",
            Self::Warning => "Warning",
        }
    }
}

impl std::fmt::Debug for AdmonitionVariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Note => write!(f, "AdmonitionVariant::Note"),
            Self::Tip => write!(f, "AdmonitionVariant::Tip"),
            Self::Important => write!(f, "AdmonitionVariant::Important"),
            Self::Caution => write!(f, "AdmonitionVariant::Caution"),
            Self::Warning => write!(f, "AdmonitionVariant::Warning"),
        }
    }
}
