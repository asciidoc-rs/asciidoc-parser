pub(crate) fn is_built_in_context(context: &str) -> bool {
    BuiltInContext::from_str(context).is_some()
}

/// The set of built-in block contexts defined by the AsciiDoc language.
///
/// A block's context (see [`IsBlock::resolved_context`]) is a stringly-typed
/// value so that third-party extensions can introduce their own contexts. The
/// contexts understood natively by this parser, however, are drawn from a fixed
/// vocabulary. This enum names that vocabulary so a downstream consumer (for
/// example, a converter that dispatches on
/// [`resolved_context`](crate::blocks::IsBlock::resolved_context)) can match
/// against a shared definition rather than hard-coding and maintaining its own
/// copy of the context strings.
///
/// Use [`from_str`](Self::from_str) to classify a context string (such as the
/// value returned by
/// [`resolved_context`](crate::blocks::IsBlock::resolved_context)) and
/// [`as_str`](Self::as_str) to recover the canonical string for a variant.
///
/// [`IsBlock::resolved_context`]: crate::blocks::IsBlock::resolved_context
/// [`resolved_context`]: crate::blocks::IsBlock::resolved_context
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum BuiltInContext {
    /// The `admonition` context.
    Admonition,

    /// The `audio` context.
    Audio,

    /// The `colist` (callout list) context.
    Colist,

    /// The `dlist` (description list) context.
    Dlist,

    /// The `document` context.
    Document,

    /// The `example` context.
    Example,

    /// The `floating_title` (discrete heading) context.
    FloatingTitle,

    /// The `image` context.
    Image,

    /// The `list_item` context.
    ListItem,

    /// The `listing` context.
    Listing,

    /// The `literal` context.
    Literal,

    /// The `olist` (ordered list) context.
    Olist,

    /// The `open` context.
    Open,

    /// The `page_break` context.
    PageBreak,

    /// The `paragraph` context.
    Paragraph,

    /// The `pass` (passthrough) context.
    Pass,

    /// The `preamble` context.
    Preamble,

    /// The `quote` context.
    Quote,

    /// The `section` context.
    Section,

    /// The `sidebar` context.
    Sidebar,

    /// The `stem` context.
    Stem,

    /// The `table` context.
    Table,

    /// The `table_cell` context.
    TableCell,

    /// The `thematic_break` context.
    ThematicBreak,

    /// The `toc` context.
    Toc,

    /// The `ulist` (unordered list) context.
    Ulist,

    /// The `verse` context.
    Verse,

    /// The `video` context.
    Video,
}

impl BuiltInContext {
    /// Every built-in context, in a stable order.
    ///
    /// This lets a consumer enumerate the full vocabulary (for example, to
    /// build a dispatch table keyed by [`as_str`](Self::as_str)).
    pub const ALL: &'static [BuiltInContext] = &[
        Self::Admonition,
        Self::Audio,
        Self::Colist,
        Self::Dlist,
        Self::Document,
        Self::Example,
        Self::FloatingTitle,
        Self::Image,
        Self::ListItem,
        Self::Listing,
        Self::Literal,
        Self::Olist,
        Self::Open,
        Self::PageBreak,
        Self::Paragraph,
        Self::Pass,
        Self::Preamble,
        Self::Quote,
        Self::Section,
        Self::Sidebar,
        Self::Stem,
        Self::Table,
        Self::TableCell,
        Self::ThematicBreak,
        Self::Toc,
        Self::Ulist,
        Self::Verse,
        Self::Video,
    ];

    /// Returns the canonical context string for this variant (e.g.,
    /// `"paragraph"`).
    ///
    /// This is the same string that [`resolved_context`] yields for a block of
    /// this context.
    ///
    /// [`resolved_context`]: crate::blocks::IsBlock::resolved_context
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Admonition => "admonition",
            Self::Audio => "audio",
            Self::Colist => "colist",
            Self::Dlist => "dlist",
            Self::Document => "document",
            Self::Example => "example",
            Self::FloatingTitle => "floating_title",
            Self::Image => "image",
            Self::ListItem => "list_item",
            Self::Listing => "listing",
            Self::Literal => "literal",
            Self::Olist => "olist",
            Self::Open => "open",
            Self::PageBreak => "page_break",
            Self::Paragraph => "paragraph",
            Self::Pass => "pass",
            Self::Preamble => "preamble",
            Self::Quote => "quote",
            Self::Section => "section",
            Self::Sidebar => "sidebar",
            Self::Stem => "stem",
            Self::Table => "table",
            Self::TableCell => "table_cell",
            Self::ThematicBreak => "thematic_break",
            Self::Toc => "toc",
            Self::Ulist => "ulist",
            Self::Verse => "verse",
            Self::Video => "video",
        }
    }

    /// Resolves a context string to its [`BuiltInContext`] variant, or `None`
    /// if the string is not a built-in context.
    ///
    /// The match is case-sensitive: built-in contexts are always lowercase.
    // This is deliberately an inherent method returning `Option` (rather than an
    // implementation of `std::str::FromStr`, which would return `Result`): a
    // non-built-in context is an ordinary, expected outcome here, not an error.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(context: &str) -> Option<Self> {
        Some(match context {
            "admonition" => Self::Admonition,
            "audio" => Self::Audio,
            "colist" => Self::Colist,
            "dlist" => Self::Dlist,
            "document" => Self::Document,
            "example" => Self::Example,
            "floating_title" => Self::FloatingTitle,
            "image" => Self::Image,
            "list_item" => Self::ListItem,
            "listing" => Self::Listing,
            "literal" => Self::Literal,
            "olist" => Self::Olist,
            "open" => Self::Open,
            "page_break" => Self::PageBreak,
            "paragraph" => Self::Paragraph,
            "pass" => Self::Pass,
            "preamble" => Self::Preamble,
            "quote" => Self::Quote,
            "section" => Self::Section,
            "sidebar" => Self::Sidebar,
            "stem" => Self::Stem,
            "table" => Self::Table,
            "table_cell" => Self::TableCell,
            "thematic_break" => Self::ThematicBreak,
            "toc" => Self::Toc,
            "ulist" => Self::Ulist,
            "verse" => Self::Verse,
            "video" => Self::Video,
            _ => return None,
        })
    }
}

impl std::fmt::Debug for BuiltInContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BuiltInContext::{}", {
            match self {
                Self::Admonition => "Admonition",
                Self::Audio => "Audio",
                Self::Colist => "Colist",
                Self::Dlist => "Dlist",
                Self::Document => "Document",
                Self::Example => "Example",
                Self::FloatingTitle => "FloatingTitle",
                Self::Image => "Image",
                Self::ListItem => "ListItem",
                Self::Listing => "Listing",
                Self::Literal => "Literal",
                Self::Olist => "Olist",
                Self::Open => "Open",
                Self::PageBreak => "PageBreak",
                Self::Paragraph => "Paragraph",
                Self::Pass => "Pass",
                Self::Preamble => "Preamble",
                Self::Quote => "Quote",
                Self::Section => "Section",
                Self::Sidebar => "Sidebar",
                Self::Stem => "Stem",
                Self::Table => "Table",
                Self::TableCell => "TableCell",
                Self::ThematicBreak => "ThematicBreak",
                Self::Toc => "Toc",
                Self::Ulist => "Ulist",
                Self::Verse => "Verse",
                Self::Video => "Video",
            }
        })
    }
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

#[cfg(test)]
mod tests {
    mod built_in_context {
        use crate::blocks::{BuiltInContext, context::is_built_in_context};

        #[test]
        fn round_trips_every_variant() {
            // Every variant's `as_str` must resolve back to the same variant,
            // and each canonical string is recognized as a built-in context.
            for &context in BuiltInContext::ALL {
                assert_eq!(BuiltInContext::from_str(context.as_str()), Some(context));
                assert!(is_built_in_context(context.as_str()));
            }
        }

        #[test]
        fn all_is_complete_and_deduplicated() {
            // `ALL` should list each of the 28 built-in contexts exactly once.
            assert_eq!(BuiltInContext::ALL.len(), 28);

            let mut seen: Vec<&str> = BuiltInContext::ALL.iter().map(|c| c.as_str()).collect();
            seen.sort_unstable();
            seen.dedup();
            assert_eq!(seen.len(), BuiltInContext::ALL.len());
        }

        #[test]
        fn known_contexts() {
            assert_eq!(
                BuiltInContext::from_str("paragraph"),
                Some(BuiltInContext::Paragraph)
            );
            assert_eq!(BuiltInContext::Listing.as_str(), "listing");
            assert_eq!(BuiltInContext::TableCell.as_str(), "table_cell");
        }

        #[test]
        fn unknown_context_is_none() {
            assert_eq!(BuiltInContext::from_str("verbatim"), None);
            // The match is case-sensitive.
            assert_eq!(BuiltInContext::from_str("Paragraph"), None);
            assert!(!is_built_in_context("not-a-context"));
        }

        #[test]
        fn impl_debug() {
            // Spot-check one exact rendering...
            assert_eq!(
                format!("{:?}", BuiltInContext::FloatingTitle),
                "BuiltInContext::FloatingTitle"
            );

            // ...then exercise every variant's `Debug` arm, checking each
            // renders as a distinct `BuiltInContext::`-prefixed name.
            let mut seen = std::collections::HashSet::new();
            for &context in BuiltInContext::ALL {
                let debug = format!("{context:?}");
                assert!(debug.starts_with("BuiltInContext::"));
                assert!(!debug.contains(' '));
                assert!(seen.insert(debug), "duplicate Debug rendering");
            }
            assert_eq!(seen.len(), BuiltInContext::ALL.len());
        }
    }
}
