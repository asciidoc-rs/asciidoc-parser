//! Describes where (and whether) a document's table of contents is rendered,
//! together with the resolved depth, title, and CSS class.

use crate::{Parser, document::InterpretedValue};

/// Where (and whether) a document's table of contents (TOC) is generated,
/// resolved from the [`toc` attribute].
///
/// The `toc` attribute is header-only, so this value is fixed once a document's
/// header has been processed. A nested [AsciiDoc table cell] behaves as its own
/// standalone document and resolves its own [`TocMode`] independently — it does
/// **not** inherit the parent document's setting.
///
/// The `auto`, `left`, and `right` placements all render the TOC automatically
/// near the top of the document; `left` and `right` additionally request a
/// fixed side column when converting to standalone HTML (a presentation detail
/// outside this crate's scope). `preamble` places the TOC immediately below the
/// preamble, and `macro` defers placement to a `toc::[]` block macro.
///
/// [`toc` attribute]: https://docs.asciidoctor.org/asciidoc/latest/toc/
/// [AsciiDoc table cell]: crate::blocks::TableCellContent::AsciiDoc
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TocMode {
    /// The `toc` attribute is unset: no table of contents is generated.
    Disabled,

    /// The `toc` attribute is empty (the value an empty `:toc:` resolves to) or
    /// set to `auto`. The TOC is generated automatically near the top of the
    /// document.
    Auto,

    /// The `toc` attribute is set to `left`: an automatically placed TOC that,
    /// in standalone HTML, is rendered as a fixed left-hand side column.
    Left,

    /// The `toc` attribute is set to `right`: an automatically placed TOC that,
    /// in standalone HTML, is rendered as a fixed right-hand side column.
    Right,

    /// The `toc` attribute is set to `preamble`: the TOC is generated
    /// immediately below the document's preamble.
    Preamble,

    /// The `toc` attribute is set to `macro`: the table of contents is
    /// generated only where a `toc::[]` block macro appears.
    Macro,
}

impl TocMode {
    /// Resolves the table-of-contents placement from a parser's current `toc`
    /// attribute state.
    pub(crate) fn from_parser(parser: &Parser) -> Self {
        match parser.attribute_value("toc") {
            InterpretedValue::Unset => Self::Disabled,
            InterpretedValue::Value(ref v) => match v.trim() {
                "macro" => Self::Macro,
                "left" => Self::Left,
                "right" => Self::Right,
                "preamble" => Self::Preamble,
                // An explicit `auto`, or any other (unrecognized) value, is
                // treated as an automatic placement, matching Asciidoctor (which
                // renders an auto-placed TOC for any non-positional value).
                _ => Self::Auto,
            },
            // An empty `:toc:` resolves to `auto`.
            InterpretedValue::Set => Self::Auto,
        }
    }

    /// Returns `true` unless the `toc` attribute is unset (i.e. a table of
    /// contents is generated somewhere in the document).
    pub fn is_enabled(self) -> bool {
        self != Self::Disabled
    }
}

/// The depth of section levels included in a table of contents when the
/// `toclevels` attribute is not set. Matches Asciidoctor's default of 2
/// (sections up to and including `===`).
pub(crate) const DEFAULT_TOCLEVELS: usize = 2;

/// The title of the table of contents when the `toc-title` attribute is not
/// set. Matches Asciidoctor's default.
pub(crate) const DEFAULT_TOC_TITLE: &str = "Table of Contents";

/// The CSS class applied to the table of contents container when the
/// `toc-class` attribute is not set. Matches Asciidoctor's default.
pub(crate) const DEFAULT_TOC_CLASS: &str = "toc";

/// The resolved table-of-contents configuration for a document (or a nested
/// AsciiDoc table cell, which resolves its own configuration independently).
///
/// Like [`TocMode`], the underlying attributes (`toc`, `toclevels`,
/// `toc-title`, `toc-class`) are header-only, so this value is captured once
/// the header has been processed and the parser still holds the document's
/// resolved attribute state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TocConfig {
    /// Where (and whether) the TOC is placed.
    pub(crate) mode: TocMode,

    /// The depth of section levels included in the TOC, from the `toclevels`
    /// attribute (default [`DEFAULT_TOCLEVELS`]).
    pub(crate) levels: usize,

    /// The TOC title, from the `toc-title` attribute (default
    /// [`DEFAULT_TOC_TITLE`]).
    pub(crate) title: String,

    /// The CSS class applied to the TOC container, from the `toc-class`
    /// attribute (default [`DEFAULT_TOC_CLASS`]).
    pub(crate) class: String,
}

impl TocConfig {
    /// Resolves the full table-of-contents configuration from a parser's
    /// current attribute state.
    pub(crate) fn from_parser(parser: &Parser) -> Self {
        Self {
            mode: TocMode::from_parser(parser),
            levels: resolve_levels(parser),
            title: resolve_title(parser),
            class: resolve_class(parser),
        }
    }

    /// Returns a configuration with no table of contents, used as the default
    /// for a structure that has not enabled a TOC.
    #[cfg(test)]
    pub(crate) fn disabled() -> Self {
        Self {
            mode: TocMode::Disabled,
            levels: DEFAULT_TOCLEVELS,
            title: DEFAULT_TOC_TITLE.to_string(),
            class: DEFAULT_TOC_CLASS.to_string(),
        }
    }
}

/// Resolves the `toclevels` depth. Accepted values are the integers 0 through
/// 5; the value `0` is coerced to `1` (this crate has no multipart-book parts,
/// so level 0 sections never appear), and any unparseable value falls back to
/// the default of 2.
fn resolve_levels(parser: &Parser) -> usize {
    parser
        .attribute_value("toclevels")
        .as_maybe_str()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .map(|n| n.max(1))
        .unwrap_or(DEFAULT_TOCLEVELS)
}

/// Resolves the `toc-title`. An empty `:toc-title:` (set but with no value)
/// yields an empty title, matching Asciidoctor; an unset attribute falls back
/// to the default.
fn resolve_title(parser: &Parser) -> String {
    match parser.attribute_value("toc-title") {
        InterpretedValue::Value(v) => v,
        InterpretedValue::Set => String::new(),
        InterpretedValue::Unset => DEFAULT_TOC_TITLE.to_string(),
    }
}

/// Resolves the `toc-class`, falling back to the default when unset or empty.
fn resolve_class(parser: &Parser) -> String {
    match parser.attribute_value("toc-class") {
        InterpretedValue::Value(v) if !v.trim().is_empty() => v,
        _ => DEFAULT_TOC_CLASS.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use crate::{Parser, document::TocMode};

    /// Parses a minimal document with the given header attribute lines and
    /// returns the parsed [`Document`](crate::Document) for inspection.
    fn doc_with(header: &str) -> crate::Document<'static> {
        let src = format!("= Title\n{header}\n\n== Section\n\ncontent");
        Parser::default().parse(&src)
    }

    #[test]
    fn mode_is_disabled_when_unset() {
        assert_eq!(doc_with("").toc_mode(), TocMode::Disabled);
        assert!(!TocMode::Disabled.is_enabled());
    }

    #[test]
    fn mode_resolves_each_placement() {
        assert_eq!(doc_with(":toc:").toc_mode(), TocMode::Auto);
        assert_eq!(doc_with(":toc: auto").toc_mode(), TocMode::Auto);
        assert_eq!(doc_with(":toc: left").toc_mode(), TocMode::Left);
        assert_eq!(doc_with(":toc: right").toc_mode(), TocMode::Right);
        assert_eq!(doc_with(":toc: preamble").toc_mode(), TocMode::Preamble);
        assert_eq!(doc_with(":toc: macro").toc_mode(), TocMode::Macro);

        for mode in [
            TocMode::Auto,
            TocMode::Left,
            TocMode::Right,
            TocMode::Preamble,
            TocMode::Macro,
        ] {
            assert!(mode.is_enabled());
        }
    }

    #[test]
    fn unrecognized_mode_is_treated_as_auto() {
        assert_eq!(doc_with(":toc: bogus").toc_mode(), TocMode::Auto);
    }

    #[test]
    fn levels_default_and_overrides() {
        assert_eq!(doc_with(":toc:").toc_levels(), 2);
        assert_eq!(doc_with(":toc:\n:toclevels: 5").toc_levels(), 5);
        // `0` is coerced to `1`; an unparseable value falls back to the default.
        assert_eq!(doc_with(":toc:\n:toclevels: 0").toc_levels(), 1);
        assert_eq!(doc_with(":toc:\n:toclevels: nope").toc_levels(), 2);
    }

    #[test]
    fn title_default_value_and_empty() {
        assert_eq!(doc_with(":toc:").toc_title(), "Table of Contents");
        assert_eq!(doc_with(":toc:\n:toc-title: My TOC").toc_title(), "My TOC");
        assert_eq!(doc_with(":toc:\n:toc-title:").toc_title(), "");
    }

    #[test]
    fn class_default_value_and_empty() {
        assert_eq!(doc_with(":toc:").toc_class(), "toc");
        assert_eq!(doc_with(":toc:\n:toc-class: floaty").toc_class(), "floaty");
        // An empty `:toc-class:` falls back to the default.
        assert_eq!(doc_with(":toc:\n:toc-class:").toc_class(), "toc");
    }
}
