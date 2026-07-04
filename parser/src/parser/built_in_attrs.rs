use std::{
    collections::HashMap,
    sync::{Arc, LazyLock},
};

use crate::{
    document::InterpretedValue,
    parser::{AllowableValue, AttributeValue, ModificationContext},
};

/// The built-in default value of the `iconsdir` attribute, used when neither
/// `iconsdir` nor `imagesdir` has been configured. When `imagesdir` is set to a
/// non-empty value and `iconsdir` is left at this default, the icons directory
/// is instead derived as `{imagesdir}/icons` (see
/// [`Document::parse`](crate::document::Document)).
pub(crate) const DEFAULT_ICONSDIR: &str = "./images/icons";

/// The built-in attribute table is identical for every parser, so build it
/// once and share it behind an [`Arc`]. A [`Parser`] holds this shared table
/// and copies it (via `Arc::make_mut`) only when it first modifies an
/// attribute, so the (large) built-in table is never deep-cloned just to create
/// or clone a parser. This matters because [`Parser::default`] (and parser
/// cloning, e.g. for nested AsciiDoc table cells) happens frequently.
///
/// [`Parser`]: crate::Parser
/// [`Parser::default`]: crate::Parser::default
static BUILT_IN_ATTRS: LazyLock<Arc<HashMap<String, AttributeValue>>> =
    LazyLock::new(|| Arc::new(build_built_in_attrs()));

static BUILT_IN_DEFAULT_VALUES: LazyLock<Arc<HashMap<String, String>>> =
    LazyLock::new(|| Arc::new(build_built_in_default_values()));

pub(super) fn built_in_attrs() -> Arc<HashMap<String, AttributeValue>> {
    BUILT_IN_ATTRS.clone()
}

pub(super) fn built_in_default_values() -> Arc<HashMap<String, String>> {
    BUILT_IN_DEFAULT_VALUES.clone()
}

fn build_built_in_attrs() -> HashMap<String, AttributeValue> {
    let mut attrs: HashMap<String, AttributeValue> = HashMap::new();

    // ## Character replacement attributes
    //
    // These provide portable replacements for common typographical marks,
    // non-visible characters, escapes for characters with special meaning in
    // AsciiDoc, and passthroughs for characters that get encoded by default.
    // See `ref/asciidoc-lang/docs/modules/attributes/pages/character-replacement-ref.adoc`.
    //
    // The entries below are listed in the same order they appear on that
    // reference page. The replacement values match Ruby Asciidoctor's
    // `INTRINSIC_ATTRIBUTES` table (e.g. `cpp` resolves to `C&#43;&#43;`, not a
    // literal `C++`).
    let char_replacement = |value: &str| AttributeValue {
        allowable_value: AllowableValue::Any,
        modification_context: ModificationContext::ApiOnly,
        value: InterpretedValue::Value(value.into()),
    };

    // `blank` is an alias for `empty` for those who find this terminology
    // clearer.
    attrs.insert("blank".to_owned(), char_replacement(""));
    attrs.insert("empty".to_owned(), char_replacement(""));
    attrs.insert("sp".to_owned(), char_replacement(" "));
    attrs.insert("nbsp".to_owned(), char_replacement("&#160;"));
    attrs.insert("zwsp".to_owned(), char_replacement("&#8203;"));
    attrs.insert("wj".to_owned(), char_replacement("&#8288;"));
    attrs.insert("apos".to_owned(), char_replacement("&#39;"));
    attrs.insert("quot".to_owned(), char_replacement("&#34;"));
    attrs.insert("lsquo".to_owned(), char_replacement("&#8216;"));
    attrs.insert("rsquo".to_owned(), char_replacement("&#8217;"));
    attrs.insert("ldquo".to_owned(), char_replacement("&#8220;"));
    attrs.insert("rdquo".to_owned(), char_replacement("&#8221;"));
    attrs.insert("deg".to_owned(), char_replacement("&#176;"));
    attrs.insert("plus".to_owned(), char_replacement("&#43;"));
    attrs.insert("brvbar".to_owned(), char_replacement("&#166;"));
    attrs.insert("vbar".to_owned(), char_replacement("|"));
    attrs.insert("amp".to_owned(), char_replacement("&"));
    attrs.insert("lt".to_owned(), char_replacement("<"));
    attrs.insert("gt".to_owned(), char_replacement(">"));
    attrs.insert("startsb".to_owned(), char_replacement("["));
    attrs.insert("endsb".to_owned(), char_replacement("]"));
    attrs.insert("caret".to_owned(), char_replacement("^"));
    attrs.insert("asterisk".to_owned(), char_replacement("*"));
    attrs.insert("tilde".to_owned(), char_replacement("~"));
    attrs.insert("backslash".to_owned(), char_replacement("\\"));
    attrs.insert("backtick".to_owned(), char_replacement("`"));
    attrs.insert("two-colons".to_owned(), char_replacement("::"));
    attrs.insert("two-semicolons".to_owned(), char_replacement(";;"));

    // `cpp` is deprecated in favor of `cxx`; both resolve to the same value.
    attrs.insert("cpp".to_owned(), char_replacement("C&#43;&#43;"));
    attrs.insert("cxx".to_owned(), char_replacement("C&#43;&#43;"));
    attrs.insert("pp".to_owned(), char_replacement("&#43;&#43;"));

    // ## Other predefined document attributes
    //
    // The groups below mirror the catalog in
    // `ref/asciidoc-lang/docs/modules/attributes/pages/document-attributes-ref.adoc`. Order is not
    // significant. Default values match Ruby Asciidoctor.
    //
    // Only attributes that are *set by default* are registered here, so they
    // resolve on a pristine parser. Attributes that are not set by default but
    // have a default value (the reference page's implied `(x)` and effective
    // `_empty_[=x]` values, e.g. `lang`, `toclevels`, `icons`) are recorded in
    // [`build_built_in_default_values`] instead: they stay absent (so an
    // attribute reference such as `{lang}` is treated as missing), but the
    // default value is applied when the attribute is later set with an empty
    // value.
    use InterpretedValue::{Set, Unset, Value};
    use ModificationContext::{Anywhere, ApiOnly, ApiOrHeader};

    // Holds a fixed value (`allowable_value` is `Any`).
    let any = |ctx, value| AttributeValue {
        allowable_value: AllowableValue::Any,
        modification_context: ctx,
        value,
    };

    // Set by default to a concrete `default` value. The value is stored directly
    // (not via [`build_built_in_default_values`]) so that setting the attribute
    // with an empty value overrides it with an empty value, rather than
    // re-applying the default.
    let set = |ctx, default: &str| AttributeValue {
        allowable_value: AllowableValue::Any,
        modification_context: ctx,
        value: Value(default.to_owned()),
    };

    // Set by default to an empty value (a boolean-style switch).
    let empty = |ctx, value| AttributeValue {
        allowable_value: AllowableValue::Empty,
        modification_context: ctx,
        value,
    };

    // ### Compliance attributes
    attrs.insert("attribute-missing".to_owned(), set(Anywhere, "skip"));
    attrs.insert("attribute-undefined".to_owned(), set(Anywhere, "drop-line"));

    // ### Localization and numbering attributes
    attrs.insert("appendix-caption".to_owned(), set(Anywhere, "Appendix"));
    attrs.insert("appendix-refsig".to_owned(), set(Anywhere, "Appendix"));
    attrs.insert("caution-caption".to_owned(), set(Anywhere, "Caution"));
    attrs.insert("chapter-refsig".to_owned(), set(Anywhere, "Chapter"));
    attrs.insert("example-caption".to_owned(), set(Anywhere, "Example"));
    attrs.insert("figure-caption".to_owned(), set(Anywhere, "Figure"));
    attrs.insert("important-caption".to_owned(), set(Anywhere, "Important"));
    attrs.insert(
        "last-update-label".to_owned(),
        set(ApiOrHeader, "Last updated"),
    );
    attrs.insert("note-caption".to_owned(), set(Anywhere, "Note"));
    attrs.insert("part-refsig".to_owned(), set(Anywhere, "Part"));
    attrs.insert("section-refsig".to_owned(), set(Anywhere, "Section"));
    attrs.insert("table-caption".to_owned(), set(Anywhere, "Table"));
    attrs.insert("tip-caption".to_owned(), set(Anywhere, "Tip"));
    attrs.insert(
        "toc-title".to_owned(),
        set(ApiOrHeader, "Table of Contents"),
    );
    attrs.insert("untitled-label".to_owned(), set(ApiOrHeader, "Untitled"));
    attrs.insert("version-label".to_owned(), set(ApiOrHeader, "Version"));
    attrs.insert("warning-caption".to_owned(), set(Anywhere, "Warning"));

    // ### Section title and table of contents attributes
    attrs.insert("idprefix".to_owned(), any(Anywhere, Value("_".into())));
    attrs.insert("idseparator".to_owned(), any(Anywhere, Value("_".into())));
    attrs.insert("sectids".to_owned(), empty(Anywhere, Set));
    attrs.insert("sectnums".to_owned(), empty(Anywhere, Unset));
    attrs.insert(
        "sectnumlevels".to_owned(),
        any(ApiOrHeader, Value("3".into())),
    );
    attrs.insert("toc".to_owned(), any(ApiOrHeader, Unset));

    // ### General content and formatting attributes
    //
    // The document type defaults to `article` and may be set in the header or
    // via the API. The derived `backend-html5-doctype-{doctype}` attribute
    // (defined below) is kept in sync by `Parser::refresh_doctype_derived_attr`
    // whenever `doctype` changes.
    attrs.insert(
        "doctype".to_owned(),
        any(ApiOrHeader, Value("article".into())),
    );

    // The file extension of the output file (always begins with a period),
    // defaulting to `.html`. Docinfo file names are built from this suffix.
    attrs.insert(
        "outfilesuffix".to_owned(),
        any(ApiOrHeader, Value(".html".into())),
    );

    // TO DO: `relfilesuffix` should default to the current value of
    // `outfilesuffix` rather than a hardcoded `.html`; they diverge for
    // non-HTML backends (e.g. `.xml` for DocBook).
    attrs.insert("relfilesuffix".to_owned(), set(Anywhere, ".html"));
    attrs.insert("webfonts".to_owned(), empty(ApiOrHeader, Set));

    // ### Image and icon attributes
    attrs.insert("iconfont-remote".to_owned(), empty(ApiOrHeader, Set));

    // The default is `{imagesdir}/icons`; when `imagesdir` is left empty this
    // resolves to `./images/icons`. The `imagesdir`-relative derivation for a
    // non-empty `imagesdir` is applied after the header is parsed (see
    // `Document::parse`).
    attrs.insert("iconsdir".to_owned(), set(Anywhere, DEFAULT_ICONSDIR));
    attrs.insert("imagesdir".to_owned(), any(Anywhere, Set));

    // ### Source highlighting and formatting attributes
    attrs.insert("prewrap".to_owned(), empty(Anywhere, Set));

    // ### HTML styling attributes
    attrs.insert("copycss".to_owned(), any(ApiOrHeader, Set));
    attrs.insert("stylesdir".to_owned(), set(ApiOrHeader, "."));
    attrs.insert("stylesheet".to_owned(), any(ApiOrHeader, Set));

    // ### Security attributes
    attrs.insert("max-include-depth".to_owned(), set(ApiOnly, "64"));

    // ### Safe-mode intrinsic attributes
    //
    // These describe the default safe mode (`SafeMode::Secure`).
    // `Parser::with_safe_mode` rewrites this family (via
    // `apply_safe_mode_attributes`) when the caller chooses a different mode;
    // exactly one `safe-mode-<name>` flag is set at a time.
    attrs.insert(
        "safe-mode-level".to_owned(),
        any(ApiOnly, Value("20".into())),
    );
    attrs.insert(
        "safe-mode-name".to_owned(),
        any(ApiOnly, Value("secure".into())),
    );
    attrs.insert("safe-mode-secure".to_owned(), any(ApiOnly, Set));

    // Derived doctype attribute (see `doctype` above): defined (empty) only for
    // the active doctype.
    attrs.insert(
        "backend-html5-doctype-article".to_owned(),
        any(Anywhere, Value(String::new())),
    );

    attrs
}

fn build_built_in_default_values() -> HashMap<String, String> {
    // The value assigned to a built-in attribute when it is set (or turned on)
    // with an empty value (e.g. a bare `:toc:` resolves to `auto`, and a bare
    // `:lang:` resolves to `en`).
    //
    // This map holds the defaults for attributes that are *not* set by default:
    // the reference page's "turn-on" attributes (`toc`, `sectnums`) and its
    // implied `(x)` / effective `_empty_[=x]` values. Because these attributes
    // are absent from [`build_built_in_attrs`], they are treated as missing when
    // referenced while unset; their default is applied only once they are
    // explicitly set. Attributes that *are* set by default store their value
    // directly in [`build_built_in_attrs`] and do not appear here, so setting
    // one with an empty value overrides it with an empty value.
    //
    // `docinfosubs` is intentionally omitted: its implied default of
    // `attributes` is handled where docinfo substitution is resolved (an unset
    // value means "apply attribute substitution"), so a bare `:docinfosubs:`
    // must remain empty rather than resolving to `attributes`.
    let mut defaults: HashMap<String, String> = HashMap::new();

    // Turn-on attributes (reference page section: section title / TOC).
    defaults.insert("sectnums".to_owned(), "all".to_owned());
    defaults.insert("toc".to_owned(), "auto".to_owned());
    defaults.insert("toclevels".to_owned(), "2".to_owned());

    // Numbering seeds (localization and numbering attributes).
    defaults.insert("appendix-number".to_owned(), "@".to_owned());
    defaults.insert("chapter-number".to_owned(), "0".to_owned());
    defaults.insert("example-number".to_owned(), "0".to_owned());
    defaults.insert("figure-number".to_owned(), "0".to_owned());
    defaults.insert("footnote-number".to_owned(), "0".to_owned());
    defaults.insert("listing-number".to_owned(), "0".to_owned());
    defaults.insert("table-number".to_owned(), "0".to_owned());
    defaults.insert("lang".to_owned(), "en".to_owned());
    defaults.insert("manname-title".to_owned(), "Name".to_owned());

    // General content and formatting attributes.
    defaults.insert("asset-uri-scheme".to_owned(), "https".to_owned());
    defaults.insert("docinfo".to_owned(), "private".to_owned());
    defaults.insert("eqnums".to_owned(), "AMS".to_owned());
    defaults.insert("media".to_owned(), "screen".to_owned());
    defaults.insert("pagewidth".to_owned(), "425".to_owned());
    defaults.insert("stem".to_owned(), "asciimath".to_owned());
    defaults.insert("table-frame".to_owned(), "all".to_owned());
    defaults.insert("table-grid".to_owned(), "all".to_owned());
    defaults.insert("table-stripes".to_owned(), "none".to_owned());

    // Image and icon attributes.
    defaults.insert("iconfont-name".to_owned(), "font-awesome".to_owned());
    defaults.insert("icons".to_owned(), "image".to_owned());
    defaults.insert("icontype".to_owned(), "png".to_owned());

    // Source highlighting and formatting attributes.
    defaults.insert("coderay-css".to_owned(), "class".to_owned());
    defaults.insert("coderay-linenums-mode".to_owned(), "table".to_owned());
    defaults.insert("highlightjs-theme".to_owned(), "github".to_owned());
    defaults.insert("prettify-theme".to_owned(), "prettify".to_owned());
    defaults.insert("pygments-css".to_owned(), "class".to_owned());
    defaults.insert("pygments-linenums-mode".to_owned(), "table".to_owned());
    defaults.insert("pygments-style".to_owned(), "default".to_owned());
    defaults.insert("rouge-css".to_owned(), "class".to_owned());
    defaults.insert("rouge-linenums-mode".to_owned(), "table".to_owned());
    defaults.insert("rouge-style".to_owned(), "github".to_owned());

    // HTML styling attributes.
    defaults.insert("toc-class".to_owned(), "toc".to_owned());

    // Manpage attributes.
    defaults.insert("man-linkstyle".to_owned(), "blue R <>".to_owned());

    defaults
}
