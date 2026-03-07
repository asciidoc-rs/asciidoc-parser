use std::collections::HashMap;

use crate::{
    document::InterpretedValue,
    parser::{AllowableValue, AttributeValue, ModificationContext},
};

pub(super) fn built_in_attrs() -> HashMap<String, AttributeValue> {
    let mut attrs: HashMap<String, AttributeValue> = HashMap::new();

    attrs.insert(
        "blank".to_owned(),
        AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context: ModificationContext::ApiOnly,
            value: InterpretedValue::Value("".into()),
        },
    );

    attrs.insert(
        "empty".to_owned(),
        AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context: ModificationContext::ApiOnly,
            value: InterpretedValue::Value("".into()),
        },
    );

    attrs.insert(
        "sp".to_owned(),
        AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context: ModificationContext::ApiOnly,
            value: InterpretedValue::Value(" ".into()),
        },
    );

    attrs.insert(
        "vbar".to_owned(),
        AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context: ModificationContext::ApiOnly,
            value: InterpretedValue::Value("|".into()),
        },
    );

    attrs.insert(
        "deg".to_owned(),
        AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context: ModificationContext::ApiOnly,
            value: InterpretedValue::Value("&#176;".into()),
        },
    );

    attrs.insert(
        "plus".to_owned(),
        AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context: ModificationContext::ApiOnly,
            value: InterpretedValue::Value("&#43;".into()),
        },
    );

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

    // TO DO: Replace ./images with value of imagesdir if that is non-default.
    attrs.insert(
        "iconsdir".to_owned(),
        AttributeValue {
            allowable_value: AllowableValue::Any,
            modification_context: ModificationContext::Anywhere,
            value: InterpretedValue::Set,
        },
    );

    attrs
}

pub(super) fn built_in_default_values() -> HashMap<String, String> {
    let mut defaults: HashMap<String, String> = HashMap::new();

    defaults.insert("example-caption".to_owned(), "Example".to_owned());
    defaults.insert("iconsdir".to_owned(), "./images/icons".to_owned());
    defaults.insert("sectnums".to_owned(), "all".to_owned());
    defaults.insert("toc".to_owned(), "auto".to_owned());

    defaults
}
