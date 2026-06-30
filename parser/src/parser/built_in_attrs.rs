use std::{collections::HashMap, sync::LazyLock};

use crate::{
    document::InterpretedValue,
    parser::{AllowableValue, AttributeValue, ModificationContext},
};

/// The built-in attribute table is identical for every parser, so build it
/// once and hand out cheap clones. Cloning copies the hash table layout
/// without re-hashing each key, which matters because [`Parser::default`]
/// (and therefore this function) is called once per parse.
///
/// [`Parser::default`]: crate::Parser::default
static BUILT_IN_ATTRS: LazyLock<HashMap<String, AttributeValue>> =
    LazyLock::new(build_built_in_attrs);

static BUILT_IN_DEFAULT_VALUES: LazyLock<HashMap<String, String>> =
    LazyLock::new(build_built_in_default_values);

pub(super) fn built_in_attrs() -> HashMap<String, AttributeValue> {
    BUILT_IN_ATTRS.clone()
}

pub(super) fn built_in_default_values() -> HashMap<String, String> {
    BUILT_IN_DEFAULT_VALUES.clone()
}

fn build_built_in_attrs() -> HashMap<String, AttributeValue> {
    let mut attrs: HashMap<String, AttributeValue> = HashMap::new();

    // ## Character replacement attributes
    //
    // These provide portable replacements for common typographical marks,
    // non-visible characters, escapes for characters with special meaning in
    // AsciiDoc, and passthroughs for characters that get encoded by default.
    // See `docs/modules/attributes/pages/character-replacement-ref.adoc`.
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
    // `docs/modules/attributes/pages/document-attributes-ref.adoc`. Order is not
    // significant. Default values match Ruby Asciidoctor.
    //
    // Modeling of the reference page's notation:
    //
    // * Attributes that are *set by default* store [`Set`](InterpretedValue::Set)
    //   (or a concrete [`Value`](InterpretedValue::Value)). When the page shows a
    //   bold default value, that value is recorded in
    //   [`build_built_in_default_values`] so the attribute resolves to it; a bold
    //   `_empty_` default resolves to an empty value.
    // * An *implied* value `(x)` (used when the attribute is not set) is recorded
    //   as [`AllowableValue::Implied`], and an *effective* value `_empty_[=x]` as
    //   [`AllowableValue::Effective`]. Per the reference page, neither can be
    //   resolved through an attribute reference, so these attributes are
    //   [`Unset`](InterpretedValue::Unset) by default and carry no entry in
    //   [`build_built_in_default_values`]. The `allowable_value` field records the
    //   documented default but is not otherwise consulted.
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
    // Not set by default; has an implied value `(default)` when unset.
    let implied = |ctx, default: &'static str| AttributeValue {
        allowable_value: AllowableValue::Implied(default),
        modification_context: ctx,
        value: Unset,
    };
    // Not set by default; an empty value is interpreted as `default`
    // (`_empty_[=default]`).
    let effective = |ctx, default: &str| AttributeValue {
        allowable_value: AllowableValue::Effective(Value(default.to_owned())),
        modification_context: ctx,
        value: Unset,
    };

    // ### Compliance attributes
    attrs.insert("attribute-missing".to_owned(), set(Anywhere, "skip"));
    attrs.insert("attribute-undefined".to_owned(), set(Anywhere, "drop-line"));

    // ### Localization and numbering attributes
    attrs.insert("appendix-caption".to_owned(), set(Anywhere, "Appendix"));
    attrs.insert("appendix-number".to_owned(), implied(Anywhere, "@"));
    attrs.insert("appendix-refsig".to_owned(), set(Anywhere, "Appendix"));
    attrs.insert("caution-caption".to_owned(), set(Anywhere, "Caution"));
    attrs.insert("chapter-number".to_owned(), implied(Anywhere, "0"));
    attrs.insert("chapter-refsig".to_owned(), set(Anywhere, "Chapter"));
    attrs.insert("example-caption".to_owned(), set(Anywhere, "Example"));
    attrs.insert("example-number".to_owned(), implied(Anywhere, "0"));
    attrs.insert("figure-caption".to_owned(), set(Anywhere, "Figure"));
    attrs.insert("figure-number".to_owned(), implied(Anywhere, "0"));
    attrs.insert("footnote-number".to_owned(), implied(Anywhere, "0"));
    attrs.insert("important-caption".to_owned(), set(Anywhere, "Important"));
    attrs.insert("lang".to_owned(), implied(ApiOrHeader, "en"));
    attrs.insert(
        "last-update-label".to_owned(),
        set(ApiOrHeader, "Last updated"),
    );
    attrs.insert("listing-number".to_owned(), implied(Anywhere, "0"));
    attrs.insert("manname-title".to_owned(), implied(ApiOrHeader, "Name"));
    attrs.insert("note-caption".to_owned(), set(Anywhere, "Note"));
    attrs.insert("part-refsig".to_owned(), set(Anywhere, "Part"));
    attrs.insert("section-refsig".to_owned(), set(Anywhere, "Section"));
    attrs.insert("table-caption".to_owned(), set(Anywhere, "Table"));
    attrs.insert("table-number".to_owned(), implied(Anywhere, "0"));
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
    attrs.insert("toclevels".to_owned(), implied(ApiOrHeader, "2"));

    // ### General content and formatting attributes
    attrs.insert("asset-uri-scheme".to_owned(), implied(ApiOrHeader, "https"));
    attrs.insert("docinfo".to_owned(), effective(ApiOrHeader, "private"));
    // `docinfosubs` has an implied default of `attributes`, but that default is
    // applied directly where docinfo substitution is resolved (an unset value
    // means "apply attribute substitution"), so it is recorded here only as
    // metadata.
    attrs.insert("docinfosubs".to_owned(), implied(ApiOrHeader, "attributes"));
    // The document type defaults to `article` and may be set in the header or
    // via the API. The derived `backend-html5-doctype-{doctype}` attribute
    // (defined below) is kept in sync by `Parser::refresh_doctype_derived_attr`
    // whenever `doctype` changes.
    attrs.insert(
        "doctype".to_owned(),
        any(ApiOrHeader, Value("article".into())),
    );
    attrs.insert("eqnums".to_owned(), effective(ApiOrHeader, "AMS"));
    attrs.insert("media".to_owned(), implied(ApiOrHeader, "screen"));
    // The file extension of the output file (always begins with a period),
    // defaulting to `.html`. Docinfo file names are built from this suffix.
    attrs.insert(
        "outfilesuffix".to_owned(),
        any(ApiOrHeader, Value(".html".into())),
    );
    attrs.insert("pagewidth".to_owned(), implied(ApiOrHeader, "425"));
    attrs.insert("relfilesuffix".to_owned(), set(Anywhere, ".html"));
    attrs.insert("stem".to_owned(), effective(ApiOrHeader, "asciimath"));
    attrs.insert("table-frame".to_owned(), implied(Anywhere, "all"));
    attrs.insert("table-grid".to_owned(), implied(Anywhere, "all"));
    attrs.insert("table-stripes".to_owned(), implied(Anywhere, "none"));
    attrs.insert("webfonts".to_owned(), empty(ApiOrHeader, Set));

    // ### Image and icon attributes
    attrs.insert(
        "iconfont-name".to_owned(),
        implied(ApiOrHeader, "font-awesome"),
    );
    attrs.insert("iconfont-remote".to_owned(), empty(ApiOrHeader, Set));
    attrs.insert("icons".to_owned(), effective(ApiOrHeader, "image"));
    // TO DO: Replace ./images/icons with value of imagesdir if that is non-default.
    attrs.insert("iconsdir".to_owned(), set(Anywhere, "./images/icons"));
    attrs.insert("icontype".to_owned(), implied(Anywhere, "png"));
    attrs.insert("imagesdir".to_owned(), any(Anywhere, Set));

    // ### Source highlighting and formatting attributes
    attrs.insert("coderay-css".to_owned(), implied(ApiOrHeader, "class"));
    attrs.insert(
        "coderay-linenums-mode".to_owned(),
        implied(Anywhere, "table"),
    );
    attrs.insert(
        "highlightjs-theme".to_owned(),
        implied(ApiOrHeader, "github"),
    );
    attrs.insert(
        "prettify-theme".to_owned(),
        implied(ApiOrHeader, "prettify"),
    );
    attrs.insert("prewrap".to_owned(), empty(Anywhere, Set));
    attrs.insert("pygments-css".to_owned(), implied(ApiOrHeader, "class"));
    attrs.insert(
        "pygments-linenums-mode".to_owned(),
        implied(Anywhere, "table"),
    );
    attrs.insert("pygments-style".to_owned(), implied(ApiOrHeader, "default"));
    attrs.insert("rouge-css".to_owned(), implied(ApiOrHeader, "class"));
    attrs.insert("rouge-linenums-mode".to_owned(), implied(Anywhere, "table"));
    attrs.insert("rouge-style".to_owned(), implied(ApiOrHeader, "github"));

    // ### HTML styling attributes
    attrs.insert("copycss".to_owned(), any(ApiOrHeader, Set));
    attrs.insert("stylesdir".to_owned(), set(ApiOrHeader, "."));
    attrs.insert("stylesheet".to_owned(), any(ApiOrHeader, Set));
    attrs.insert("toc-class".to_owned(), implied(ApiOrHeader, "toc"));

    // ### Manpage attributes
    attrs.insert(
        "man-linkstyle".to_owned(),
        implied(ApiOrHeader, "blue R <>"),
    );

    // ### Security attributes
    // `max-attribute-value-size` only has a default value (`4096`) when the safe
    // mode is SECURE, which is not yet implemented
    // (<https://github.com/asciidoc-rs/asciidoc-parser/issues/277>), so it is
    // unset here.
    attrs.insert("max-attribute-value-size".to_owned(), any(ApiOnly, Unset));
    attrs.insert("max-include-depth".to_owned(), set(ApiOnly, "64"));

    // Derived doctype attribute (see `doctype` above): defined (empty) only for
    // the active doctype.
    attrs.insert(
        "backend-html5-doctype-article".to_owned(),
        any(Anywhere, Value(String::new())),
    );

    attrs
}

fn build_built_in_default_values() -> HashMap<String, String> {
    // The value assigned to a built-in attribute when it is *turned on* with an
    // empty value (e.g. a bare `:toc:` resolves to `auto`). This applies only to
    // attributes that are not set by default: setting them activates the default.
    //
    // Attributes that *are* set by default store their value directly (see the
    // `set` helper in [`build_built_in_attrs`]), so that setting them with an
    // empty value overrides them with an empty value rather than re-applying the
    // default. Implied/effective defaults are recorded on the attribute's
    // `allowable_value` and are intentionally not resolvable here.
    let mut defaults: HashMap<String, String> = HashMap::new();

    defaults.insert("sectnums".to_owned(), "all".to_owned());
    defaults.insert("toc".to_owned(), "auto".to_owned());

    defaults
}
