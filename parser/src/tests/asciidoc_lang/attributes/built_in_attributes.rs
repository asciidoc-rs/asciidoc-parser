use crate::tests::prelude::*;

track_file!("docs/modules/attributes/pages/built-in-attributes.adoc");

non_normative!(
    r#"
= Declare Built-In Document Attributes
:navtitle: Declare Built-In Attributes

An AsciiDoc processor has numerous attributes reserved for special purposes.
*Built-in attributes* add, configure, and control common features in a document.
Many built-in attributes only take effect when defined in the document header with an attribute entry.

"#
);

#[test]
fn use_default_value() {
    verifies!(
        r#"
== Use an attribute's default value

Many built-in attributes have a default value.
When you want to activate a built-in attribute and assign it its default value, you can leave the value in the attribute entry empty.

For example, to turn on the xref:toc:index.adoc[Table of Contents for a document], you set the `toc` attribute using an attribute entry in the document header.

[source]
----
= Title of Document
:toc:
----

The default value of an activated attribute will be assigned at processing time, if:

. it has a default value, and
. the value in the attribute entry is left empty

In the example above, the default value of `auto` will be assigned to `toc` since the value was left empty in the attribute entry.

"#
    );

    let mut parser = Parser::default();
    parser.parse("= Title of Document\n:toc:");

    assert_eq!(
        parser.attribute_value("toc").as_maybe_str().unwrap(),
        "auto"
    );
}

#[test]
fn override_default_value() {
    verifies!(
        r#"
== Override an attribute's default value

You may not want to use the default value of a built-in attribute.
In the next example, we'll override the default value of an attribute that the AsciiDoc processor sets automatically.
The built-in attribute `doctype` is automatically set and assigned a value of `article` at processing time.
However, if you want to use AsciiDoc's book features, the `doctype` attribute needs to be assigned the `book` value.

[source]
----
= Title of My Document
:doctype: book <.>
----
<.> Set `doctype` in the document header and assign it the value `book`.
Explicit values must be offset from the closing colon (`:`) by at least one space.

To override an attribute's default value, you have to explicitly assign a value when you set the attribute.
The value assigned to an attribute in the document header replaces the default value (assuming the attribute is not locked via the CLI or API).

"#
    );

    let mut parser = Parser::default();
    parser.parse("= Title of My Document\n:doctype: book");

    assert_eq!(
        parser.attribute_value("doctype").as_maybe_str().unwrap(),
        "book"
    );
}

#[test]
fn override_default_asset_directories() {
    verifies!(
        r#"
//Change to override a default value with a user-defined value
=== Override a default asset directory value

You can also use the built-in asset directory attributes to customize the base path to images (default: `_empty_`), icons (default: `./images/icons`), stylesheets (default: `./stylesheets`) and JavaScript files (default: `./javascripts`).

.Replace the default values of the built-in asset directory attributes
[source]
----
= My Document
:imagesdir: ./images
:iconsdir: ./icons
:stylesdir: ./styles
:scriptsdir: ./js
----

The four built-in attributes in the example above have default values that are automatically set at processing time.
However, in the example, they're being set and assigned explicit values in the document header.
This explicit user-defined value replaces the default value (assuming the attribute is not locked via the CLI or API).

"#
    );

    let mut parser = Parser::default();
    parser.parse("= My Document\n:imagesdir: ./images\n:iconsdir: ./icons\n:stylesdir: ./styles\n:scriptsdir: ./js");

    assert_eq!(
        parser.attribute_value("imagesdir").as_maybe_str().unwrap(),
        "./images"
    );

    assert_eq!(
        parser.attribute_value("iconsdir").as_maybe_str().unwrap(),
        "./icons"
    );

    assert_eq!(
        parser.attribute_value("stylesdir").as_maybe_str().unwrap(),
        "./styles"
    );

    assert_eq!(
        parser.attribute_value("scriptsdir").as_maybe_str().unwrap(),
        "./js"
    );
}

// Non-normative because this section is commented out.
non_normative!(
    r#"
////
Many built-in attributes have a built-in value that is designated as the default value.
This default value is assigned when the attribute is set and its value is left empty.
For example, the xref:sections:id.adoc#separator[ID word separator attribute] can accept <<user-values,user-defined values>> and it has one default value.
If you set `idseparator` and leave the value empty, the default value will be assigned automatically when the document is processed.

[source]
----
:idseparator: <1>
----
<1> The words in automatically generated IDs will be separated with an underscore (`_`), the attribute's default value, because the value is empty.

To override the default value of an attribute, you have to explicitly assign a new value when you set the attribute.

[source]
----
:idseparator: - <1>
----
<1> The words in automatically generated IDs will be separated with a hyphen (`-`).
The value must be offset from the attribute's name by a space.
////
"#
);
