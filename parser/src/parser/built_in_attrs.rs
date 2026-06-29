use std::collections::HashMap;

use crate::{
    document::InterpretedValue,
    parser::{AllowableValue, AttributeValue, ModificationContext},
};

pub(super) fn built_in_attrs() -> HashMap<String, AttributeValue> {
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
    // These configure processor behavior rather than performing character
    // replacement. Order is not significant.

    attrs.insert(
        "toc".to_owned(),
        AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context: ModificationContext::ApiOrHeader,
            value: InterpretedValue::Unset,
        },
    );

    attrs.insert(
        "sectids".to_owned(),
        AttributeValue {
            allowable_value: AllowableValue::Empty,
            modification_context: ModificationContext::Anywhere,
            value: InterpretedValue::Set,
        },
    );

    attrs.insert(
        "sectnums".to_owned(),
        AttributeValue {
            allowable_value: AllowableValue::Empty,
            modification_context: ModificationContext::Anywhere,
            value: InterpretedValue::Unset,
        },
    );

    attrs.insert(
        "sectnumlevels".to_owned(),
        AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context: ModificationContext::ApiOrHeader,
            value: InterpretedValue::Value("3".into()),
        },
    );

    attrs.insert(
        "idprefix".to_owned(),
        AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context: ModificationContext::Anywhere,
            value: InterpretedValue::Value("_".into()),
        },
    );

    attrs.insert(
        "idseparator".to_owned(),
        AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context: ModificationContext::Anywhere,
            value: InterpretedValue::Value("_".into()),
        },
    );

    attrs.insert(
        "example-caption".to_owned(),
        AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context: ModificationContext::Anywhere,
            value: InterpretedValue::Set,
        },
    );

    attrs.insert(
        "table-caption".to_owned(),
        AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context: ModificationContext::Anywhere,
            value: InterpretedValue::Set,
        },
    );

    // TO DO: Replace ./images with value of imagesdir if that is non-default.
    attrs.insert(
        "iconsdir".to_owned(),
        AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context: ModificationContext::Anywhere,
            value: InterpretedValue::Set,
        },
    );

    // The document type defaults to `article` and may be set in the header or
    // via the API. The derived `backend-html5-doctype-{doctype}` attribute is
    // defined (empty) only for the active doctype; it is kept in sync by
    // `Parser::refresh_doctype_derived_attr` whenever `doctype` changes.
    attrs.insert(
        "doctype".to_owned(),
        AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context: ModificationContext::ApiOrHeader,
            value: InterpretedValue::Value("article".to_owned()),
        },
    );
    attrs.insert(
        "backend-html5-doctype-article".to_owned(),
        AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context: ModificationContext::Anywhere,
            value: InterpretedValue::Value(String::new()),
        },
    );

    attrs
}

pub(super) fn built_in_default_values() -> HashMap<String, String> {
    let mut defaults: HashMap<String, String> = HashMap::new();

    defaults.insert("example-caption".to_owned(), "Example".to_owned());
    defaults.insert("table-caption".to_owned(), "Table".to_owned());
    defaults.insert("iconsdir".to_owned(), "./images/icons".to_owned());
    defaults.insert("sectnums".to_owned(), "all".to_owned());
    defaults.insert("toc".to_owned(), "auto".to_owned());

    defaults
}
