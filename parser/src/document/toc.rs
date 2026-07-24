//! Describes where (and whether) a document's table of contents is rendered,
//! together with the resolved depth, title, and CSS class.

use crate::{Parser, document::InterpretedValue};

/// Where (and whether) a document's table of contents (TOC) is generated,
/// resolved from the [`toc` attribute] and, when `toc` carries no placement
/// keyword, the `toc-placement` attribute.
///
/// The `toc`/`toc-placement` attributes are header-only, so this value is fixed
/// once a document's header has been processed. A nested [AsciiDoc table cell]
/// behaves as its own standalone document and resolves its own [`TocMode`]
/// independently – it does **not** inherit the parent document's setting.
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
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum TocMode {
    /// The `toc` attribute is unset: no table of contents is generated.
    Disabled,

    /// The `toc` attribute is empty (the value an empty `:toc:` resolves to) or
    /// set to `auto`. The TOC is generated automatically near the top of the
    /// document.
    Auto,

    /// The `toc` attribute is set to `left` (or the legacy `toc2` alias is
    /// set): an automatically placed TOC that, in standalone HTML, is rendered
    /// as a fixed left-hand side column.
    Left,

    /// The `toc` attribute is set to `right`: an automatically placed TOC that,
    /// in standalone HTML, is rendered as a fixed right-hand side column.
    Right,

    /// The placement resolves to `preamble` (via `:toc: preamble` or
    /// `:toc-placement: preamble`): the TOC is generated immediately below the
    /// document's preamble.
    Preamble,

    /// The placement resolves to `macro` (via `:toc: macro` or
    /// `:toc-placement: macro`): the table of contents is generated only where
    /// a `toc::[]` block macro appears.
    Macro,
}

impl TocMode {
    /// Resolves the table-of-contents placement from a parser's current `toc`
    /// attribute state.
    ///
    /// This reads the raw stored `toc-placement`, distinguishing a never-set
    /// attribute from an explicit unset tombstone. The derived `toc-position` /
    /// `toc-placement` / `toc-class` document attributes are materialized from
    /// the resolved placement *after* this runs – see
    /// [`Parser::materialize_toc_attributes`](crate::Parser).
    pub(crate) fn from_parser(parser: &Parser) -> Self {
        let value = parser.attribute_value("toc");
        if value == InterpretedValue::Unset {
            // `toc2` is a legacy alias that enables a left-positioned table of
            // contents (equivalent to `:toc: left`). When the `toc` attribute
            // itself is unset, a set `toc2` still turns the TOC on. (A soft-unset
            // `toc2!` records an `Unset` tombstone, which does not enable it.)
            if parser.attribute_value("toc2") != InterpretedValue::Unset {
                return Self::Left;
            }
            return Self::Disabled;
        }

        // `toc` has a built-in default of `auto`, so a bare `:toc:` resolves to
        // `Value("auto")` (never `Set`). A placement keyword in the `toc` value
        // itself is a shorthand that wins outright. Otherwise (`auto`, empty, or
        // any unrecognized value) the placement comes from the separate
        // `toc-placement` attribute, matching Asciidoctor, which folds the `toc`
        // shorthand into `toc-placement` and treats the latter as the source of
        // truth. A bogus `toc-placement` – like a bogus `toc` – falls back to an
        // automatic placement.
        match value.as_maybe_str().map(str::trim) {
            Some("macro") => Self::Macro,
            Some("left") => Self::Left,
            Some("right") => Self::Right,
            Some("preamble") => Self::Preamble,
            _ => {
                let placement = parser.attribute_value("toc-placement");
                match placement.as_maybe_str().map(str::trim) {
                    Some("macro") => Self::Macro,
                    Some("left") => Self::Left,
                    Some("right") => Self::Right,
                    Some("preamble") => Self::Preamble,

                    // `toc-placement` carries no recognized placement keyword.
                    // When it has been *explicitly unset* (via `:toc-placement!:`
                    // or an API unset) while `toc` is enabled, Asciidoctor's
                    // placement lookup falls through its `auto` default to a
                    // `macro` fetch fallback, so the TOC defers to a `toc::[]`
                    // block macro. (Asciidoctor deletes the attribute's default;
                    // this crate records the unset as an `Unset` tombstone, which
                    // `has_attribute` still reports as present – distinguishing it
                    // from a `toc-placement` that was never set.) A `toc-placement`
                    // that was never set, or set to any other (bogus) value, falls
                    // back to an automatic placement.
                    _ if placement == InterpretedValue::Unset
                        && parser.has_attribute("toc-placement") =>
                    {
                        Self::Macro
                    }
                    _ => Self::Auto,
                }
            }
        }
    }

    /// Returns `true` unless the `toc` attribute is unset (i.e. a table of
    /// contents is generated somewhere in the document).
    pub fn is_enabled(self) -> bool {
        self != Self::Disabled
    }

    /// Returns the value of the derived `toc-position` document attribute for
    /// this placement, or `None` when Asciidoctor leaves it unset (its `nil`
    /// default). The side-column placements report their side (`left` /
    /// `right`); the content-flow placements (`preamble` / `macro`) report
    /// `content`; an automatic top TOC (and a disabled TOC) leave it unset.
    ///
    /// See [`Parser::materialize_toc_attributes`](crate::Parser).
    pub(crate) fn derived_toc_position(self) -> Option<&'static str> {
        match self {
            Self::Left => Some("left"),
            Self::Right => Some("right"),
            Self::Preamble | Self::Macro => Some("content"),
            Self::Auto | Self::Disabled => None,
        }
    }

    /// Returns the value of the derived `toc-placement` document attribute for
    /// this placement, or `None` when no TOC is generated. The automatic and
    /// side-column placements all fold to `auto`; `preamble` / `macro` report
    /// themselves. This is the placement keyword Asciidoctor exposes once the
    /// `toc` shorthand has been folded into `toc-placement`.
    pub(crate) fn derived_toc_placement(self) -> Option<&'static str> {
        match self {
            Self::Disabled => None,
            Self::Preamble => Some("preamble"),
            Self::Macro => Some("macro"),
            Self::Auto | Self::Left | Self::Right => Some("auto"),
        }
    }

    /// Returns the value the derived `toc-class` document attribute defaults to
    /// for this placement when the author has not set `toc-class`, or `None`
    /// when Asciidoctor leaves it unset (its `nil` default). Only a `left` /
    /// `right` side-column TOC introduces a default (`toc2`, the side-column
    /// class).
    pub(crate) fn derived_toc_class(self) -> Option<&'static str> {
        match self {
            Self::Left | Self::Right => Some(DEFAULT_TOC_CLASS_SIDE),
            _ => None,
        }
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

/// The CSS class applied to the table of contents container for a
/// `left`/`right` side-column TOC when the `toc-class` attribute is not set.
/// This is the class that drives the fixed side-column styling in Asciidoctor's
/// standalone HTML.
pub(crate) const DEFAULT_TOC_CLASS_SIDE: &str = "toc2";

/// The resolved table-of-contents configuration for a document (or a nested
/// AsciiDoc table cell, which resolves its own configuration independently).
///
/// Like [`TocMode`], the underlying attributes (`toc`, `toclevels`,
/// `toc-title`, `toc-class`) are header-only, so this value is captured once
/// the header has been processed and the parser still holds the document's
/// resolved attribute state.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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
        let mode = TocMode::from_parser(parser);
        Self {
            mode,
            levels: resolve_levels(parser),
            title: resolve_title(parser),
            class: resolve_class(parser, mode),
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
/// 5: the value `0` is coerced to `1` (this crate has no multipart-book parts,
/// so level 0 sections never appear) and values above `5` are clamped to `5`,
/// matching the documented range. Any unparseable value falls back to the
/// default of 2.
fn resolve_levels(parser: &Parser) -> usize {
    parser
        .attribute_value("toclevels")
        .as_maybe_str()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .map(|n| n.clamp(1, 5))
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

/// Resolves the `toc-class`. An explicit, non-empty `toc-class` wins outright.
/// Otherwise the default depends on placement: a `left`/`right` side-column TOC
/// uses `toc2` (the class that drives the side-column styling), matching
/// Asciidoctor, which switches the default `toc-class` to `toc2` for those
/// placements; every other placement uses the plain `toc`.
fn resolve_class(parser: &Parser, mode: TocMode) -> String {
    match parser.attribute_value("toc-class") {
        InterpretedValue::Value(v) if !v.trim().is_empty() => v,
        _ => default_toc_class(mode).to_string(),
    }
}

/// Returns the default `toc-class` for a resolved placement: `toc2` for a
/// `left`/`right` side-column TOC, and the plain `toc` for every other
/// placement.
fn default_toc_class(mode: TocMode) -> &'static str {
    match mode {
        TocMode::Left | TocMode::Right => DEFAULT_TOC_CLASS_SIDE,
        _ => DEFAULT_TOC_CLASS,
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Parser,
        document::{InterpretedValue, TocMode},
    };

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
    fn toc2_alias_enables_a_left_placed_toc() {
        // `toc2` is a legacy alias for `:toc: left`: setting the bare attribute
        // enables a left-positioned table of contents even though the `toc`
        // attribute itself is unset.
        assert_eq!(doc_with(":toc2:").toc_mode(), TocMode::Left);
        assert!(doc_with(":toc2:").toc_mode().is_enabled());

        // Its side-column placement also switches the default `toc-class` to
        // `toc2`, like any other `left`/`right` TOC.
        assert_eq!(doc_with(":toc2:").toc_class(), "toc2");

        // A soft-unset `toc2!` does not enable the TOC.
        assert_eq!(doc_with(":toc2!:").toc_mode(), TocMode::Disabled);
    }

    #[test]
    fn placement_falls_back_to_toc_placement_attribute() {
        // When the `toc` value carries no placement keyword, the separate
        // `toc-placement` attribute determines the placement.
        assert_eq!(
            doc_with(":toc:\n:toc-placement: preamble").toc_mode(),
            TocMode::Preamble
        );
        assert_eq!(
            doc_with(":toc:\n:toc-placement: macro").toc_mode(),
            TocMode::Macro
        );
        assert_eq!(
            doc_with(":toc: auto\n:toc-placement: preamble").toc_mode(),
            TocMode::Preamble
        );

        // A placement keyword in the `toc` value wins over `toc-placement`.
        assert_eq!(
            doc_with(":toc: macro\n:toc-placement: preamble").toc_mode(),
            TocMode::Macro
        );

        // A bogus `toc-placement` falls back to an automatic placement.
        assert_eq!(
            doc_with(":toc:\n:toc-placement: bogus").toc_mode(),
            TocMode::Auto
        );
    }

    #[test]
    fn soft_unset_toc_placement_resolves_to_macro() {
        // With `toc` enabled, explicitly unsetting `toc-placement` defers the
        // TOC to a `toc::[]` block macro (Asciidoctor's `macro` fetch fallback),
        // whereas a `toc-placement` that was never set stays automatic.
        assert_eq!(doc_with(":toc:").toc_mode(), TocMode::Auto);
        assert_eq!(
            doc_with(":toc:\n:toc-placement!:").toc_mode(),
            TocMode::Macro
        );
        assert_eq!(
            doc_with(":toc:\n:!toc-placement:").toc_mode(),
            TocMode::Macro
        );

        // A placement keyword in the `toc` value still wins over an unset
        // `toc-placement`.
        assert_eq!(
            doc_with(":toc: preamble\n:toc-placement!:").toc_mode(),
            TocMode::Preamble
        );
    }

    #[test]
    fn levels_default_and_overrides() {
        assert_eq!(doc_with(":toc:").toc_levels(), 2);
        assert_eq!(doc_with(":toc:\n:toclevels: 5").toc_levels(), 5);

        // `0` is coerced to `1`, values above `5` are clamped to `5`, and an
        // unparseable value falls back to the default.
        assert_eq!(doc_with(":toc:\n:toclevels: 0").toc_levels(), 1);
        assert_eq!(doc_with(":toc:\n:toclevels: 6").toc_levels(), 5);
        assert_eq!(doc_with(":toc:\n:toclevels: nope").toc_levels(), 2);
    }

    #[test]
    fn title_default_value_and_empty() {
        assert_eq!(doc_with(":toc:").toc_title(), "Table of Contents");
        assert_eq!(doc_with(":toc:\n:toc-title: My TOC").toc_title(), "My TOC");

        // A `:toc-title:` set with no value yields an empty title.
        assert_eq!(doc_with(":toc:\n:toc-title:").toc_title(), "");

        // An explicitly unset `:toc-title!:` falls back to the built-in default.
        assert_eq!(
            doc_with(":toc:\n:toc-title!:").toc_title(),
            "Table of Contents"
        );
    }

    #[test]
    fn class_default_value_and_empty() {
        assert_eq!(doc_with(":toc:").toc_class(), "toc");
        assert_eq!(doc_with(":toc:\n:toc-class: floaty").toc_class(), "floaty");

        // An empty `:toc-class:` falls back to the default.
        assert_eq!(doc_with(":toc:\n:toc-class:").toc_class(), "toc");
    }

    #[test]
    fn class_defaults_to_toc2_for_side_column_placement() {
        // A `left`/`right` side-column TOC switches the default class to `toc2`,
        // matching Asciidoctor; every other placement keeps the plain `toc`.
        assert_eq!(doc_with(":toc: left").toc_class(), "toc2");
        assert_eq!(doc_with(":toc: right").toc_class(), "toc2");
        assert_eq!(doc_with(":toc:\n:toc-placement: left").toc_class(), "toc2");
        assert_eq!(doc_with(":toc:\n:toc-placement: right").toc_class(), "toc2");
        assert_eq!(doc_with(":toc: preamble").toc_class(), "toc");
        assert_eq!(doc_with(":toc: macro").toc_class(), "toc");

        // An explicit `toc-class` still wins over the side-column default.
        assert_eq!(
            doc_with(":toc: left\n:toc-class: floaty").toc_class(),
            "floaty"
        );

        // An explicit but empty `:toc-class:` resolves to the built-in `toc-class`
        // default (`toc`) at the attribute layer, so the placement-derived `toc2`
        // default only applies when `toc-class` is left entirely unset.
        assert_eq!(doc_with(":toc: left\n:toc-class:").toc_class(), "toc");
    }

    /// The derived `toc-position` / `toc-placement` / `toc-class` attributes
    /// are materialized so they are queryable via
    /// `Document::attribute_value`, matching Asciidoctor (see the `verify
    /// toc attribute matrix` upstream test).
    #[test]
    fn derived_attributes_are_materialized() {
        use InterpretedValue::{Unset, Value};

        // Reads the derived (`toc-position`, `toc-placement`, `toc-class`)
        // document attributes as a `(position, placement, class)` triple.
        let derived = |header: &str| {
            let doc = doc_with(header);
            (
                doc.attribute_value("toc-position"),
                doc.attribute_value("toc-placement"),
                doc.attribute_value("toc-class"),
            )
        };

        // An automatic top TOC: position and class stay unset, placement `auto`.
        assert_eq!(derived(":toc:"), (Unset, Value("auto".into()), Unset));

        // An unrecognized `toc` value still resolves to an automatic placement.
        assert_eq!(
            derived(":toc: beeboo"),
            (Unset, Value("auto".into()), Unset)
        );

        // Side-column placements derive their side, keep placement `auto`, and
        // default the class to `toc2`.
        assert_eq!(
            derived(":toc: left"),
            (
                Value("left".into()),
                Value("auto".into()),
                Value("toc2".into())
            )
        );
        assert_eq!(
            derived(":toc: right"),
            (
                Value("right".into()),
                Value("auto".into()),
                Value("toc2".into())
            )
        );

        // The legacy `toc2` alias behaves like `toc=left`.
        assert_eq!(
            derived(":toc2:"),
            (
                Value("left".into()),
                Value("auto".into()),
                Value("toc2".into())
            )
        );

        // Content-flow placements report `content` and leave the class unset.
        assert_eq!(
            derived(":toc: preamble"),
            (Value("content".into()), Value("preamble".into()), Unset)
        );
        assert_eq!(
            derived(":toc: macro"),
            (Value("content".into()), Value("macro".into()), Unset)
        );
    }

    #[test]
    fn derived_placement_overwrites_author_supplied_values() {
        use InterpretedValue::Value;

        // A `toc-position` the resolved placement contradicts is overwritten:
        // the `macro` placement forces `content`.
        let doc = doc_with(":toc:\n:toc-placement: macro\n:toc-position: left");
        assert_eq!(doc.attribute_value("toc-position"), Value("content".into()));
        assert_eq!(doc.attribute_value("toc-placement"), Value("macro".into()));

        // A soft-unset `toc-placement!` with `toc` set defers to a `toc::[]`
        // macro; the derived `toc-placement` is re-materialized as `macro`,
        // overwriting the unset tombstone.
        let doc = doc_with(":toc:\n:toc-placement!:");
        assert_eq!(doc.attribute_value("toc-position"), Value("content".into()));
        assert_eq!(doc.attribute_value("toc-placement"), Value("macro".into()));
    }

    #[test]
    fn explicit_toc_class_survives_side_column_default() {
        // The side-column `toc2` default only fills in an *unset* `toc-class`;
        // an explicit author value is left untouched.
        assert_eq!(
            doc_with(":toc: left\n:toc-class: floaty").attribute_value("toc-class"),
            InterpretedValue::Value("floaty".into())
        );
    }

    #[test]
    fn derived_attributes_do_not_leak_across_parses() {
        // The derived attributes live on each document's snapshot, not on the
        // parser, so reusing a parser must not carry one document's TOC state
        // into the next.
        let mut parser = Parser::default();

        // A first document with a `macro` placement materializes its derived
        // attributes on its own snapshot.
        let doc1 = parser.parse("= One\n:toc: macro\n\n== S\n\nx");
        assert_eq!(doc1.toc_mode(), TocMode::Macro);
        assert_eq!(
            doc1.attribute_value("toc-placement"),
            InterpretedValue::Value("macro".into())
        );

        // A second document that enables an automatic TOC must resolve `Auto` –
        // if the first document's derived `toc-placement: macro` had leaked onto
        // the parser, `TocMode::from_parser` would read it back and wrongly
        // resolve `Macro` here.
        let doc2 = parser.parse("= Two\n:toc:\n\n== S\n\nx");
        assert_eq!(doc2.toc_mode(), TocMode::Auto);
        assert_eq!(
            doc2.attribute_value("toc-placement"),
            InterpretedValue::Value("auto".into())
        );

        // The first document's derived `toc-position` (`content`) likewise does
        // not linger: an automatic TOC leaves it unset.
        assert!(!doc2.has_attribute("toc-position"));
    }

    #[test]
    fn disabled_toc_materializes_no_derived_attributes() {
        // With no TOC enabled, the whole family stays unset (Asciidoctor's
        // defaults), so none of the attributes is even present.
        let doc = doc_with("");
        for name in ["toc-position", "toc-placement", "toc-class"] {
            assert!(!doc.has_attribute(name), "{name} should be absent");
        }
    }

    /// Exercises the derived-attribute mapping directly for every [`TocMode`]
    /// variant, including [`TocMode::Disabled`] – which the parse path never
    /// feeds to these helpers (materialization returns early for it), but whose
    /// arms must still map to "no attribute".
    #[test]
    fn derived_attribute_mapping_covers_every_mode() {
        for (mode, position, placement, class) in [
            (TocMode::Disabled, None, None, None),
            (TocMode::Auto, None, Some("auto"), None),
            (TocMode::Left, Some("left"), Some("auto"), Some("toc2")),
            (TocMode::Right, Some("right"), Some("auto"), Some("toc2")),
            (TocMode::Preamble, Some("content"), Some("preamble"), None),
            (TocMode::Macro, Some("content"), Some("macro"), None),
        ] {
            assert_eq!(
                mode.derived_toc_position(),
                position,
                "position for {mode:?}"
            );
            assert_eq!(
                mode.derived_toc_placement(),
                placement,
                "placement for {mode:?}"
            );
            assert_eq!(mode.derived_toc_class(), class, "class for {mode:?}");
        }
    }
}
