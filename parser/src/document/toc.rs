//! Describes where (and whether) a document's table of contents is rendered.

use crate::{Parser, document::InterpretedValue};

/// Where (and whether) a document's table of contents (TOC) is generated,
/// resolved from the [`toc` attribute].
///
/// The `toc` attribute is header-only, so this value is fixed once a document's
/// header has been processed. A nested [AsciiDoc table cell] behaves as its own
/// standalone document and resolves its own [`TocMode`] independently — it does
/// **not** inherit the parent document's setting.
///
/// [`toc` attribute]: https://docs.asciidoctor.org/asciidoc/latest/toc/
/// [AsciiDoc table cell]: crate::blocks::TableCellContent::AsciiDoc
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TocMode {
    /// The `toc` attribute is unset: no table of contents is generated.
    Disabled,

    /// The `toc` attribute names an automatic placement (`auto` — the value an
    /// empty `:toc:` resolves to — or `left`, `right`, `preamble`). The table
    /// of contents is generated automatically near the top of the document.
    Auto,

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
            InterpretedValue::Value(ref v) if v == "macro" => Self::Macro,
            // An empty `:toc:` resolves to `auto`; `left`, `right`, and
            // `preamble` are likewise automatic placements. Any other value is
            // treated as automatic, matching Asciidoctor (which renders an
            // auto-placed TOC for any non-`macro` value).
            _ => Self::Auto,
        }
    }
}
