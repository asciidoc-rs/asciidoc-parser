use crate::tests::prelude::*;

track_file!("docs/modules/attributes/pages/unset-attributes.adoc");

non_normative!(
    r#"
= Unset Document Attributes
:navtitle: Unset Attributes

Document attributes--built-in, boolean, and custom--can be unset in the document header and document body.

"#
);

mod unset_in_header {
    use crate::{
        Parser, document::InterpretedValue, parser::ModificationContext, tests::prelude::*,
    };

    non_normative!(
        r#"
== Unset a document attribute in the header

"#
    );

    #[test]
    fn unset_syntax() {
        verifies!(
            r#"
Document attributes are unset by adding a bang symbol (`!`) directly in front of (preferred) or after the attribute's name.
Like when setting an attribute in a document header, the attribute entry must be on its own line.
Don't add a value to the entry.

[source]
----
= Title
:!name: <.>
:name!: <.>
----
<.> An attribute is unset when a `!` is prefixed to its name (preferred).
<.> An attribute is unset when a `!` is appended to its name.

"#
        );

        let mut parser = Parser::default()
            .with_intrinsic_attribute("name1", "name1", ModificationContext::Anywhere)
            .with_intrinsic_attribute("name2", "name2", ModificationContext::Anywhere)
            .with_intrinsic_attribute("name3", "name3", ModificationContext::Anywhere);

        parser.parse("= Title\n:!name1:\n:name2!:");

        assert_eq!(parser.attribute_value("name1"), InterpretedValue::Unset);

        assert_eq!(parser.attribute_value("name2"), InterpretedValue::Unset);

        assert_eq!(
            parser.attribute_value("name3"),
            InterpretedValue::Value("name3".to_owned())
        );
    }

    #[test]
    fn sectids() {
        verifies!(
            r#"
Let's use an attribute entry to turn off the built-in boolean attribute named `sectids`.
The AsciiDoc processor automatically sets `sectids` at processing time unless you unset it.
The `sectids` attribute xref:sections:auto-ids.adoc[generates an ID for each section] from the section's title.

.Unset a boolean attribute
[source#ex-unset-boolean]
----
≈----
<.> On a new line, type a colon (`:`), directly followed by a bang symbol (`!`), the attribute's name, and then another colon (`:`).
After the closing colon, press kbd:[Enter].
The attribute is now unset and its behavior won't be applied to the document.

Once an attribute is unset, its behavior is deactivated.
When `sectids` is unset, the AsciiDoc processor will not generate IDs from section titles at processing time.

"#
        );

        let mut parser = Parser::default();

        parser.parse("= Document Title\n:!sectids:");

        assert_eq!(parser.attribute_value("sectids"), InterpretedValue::Unset);
    }

    #[test]
    fn example_caption() {
        verifies!(
            r#"
Let's unset the built-in attribute `example-caption`.
This is an attribute that is set and assigned a default value of `Example` automatically by the AsciiDoc processor when you use an example block.

.Unset an automatically declared attribute
[source#ex-unset-built-in]
----
= Title
:!example-caption: <.>
----
<.> Example blocks won't be labeled and numbered, e.g., Example 1, because the attribute controlling that behavior is unset with the leading `!`.

"#
        );

        let mut parser = Parser::default();

        parser.parse("= Title\n:!example-caption:");

        assert_eq!(
            parser.attribute_value("example-caption"),
            InterpretedValue::Unset
        );
    }
}

mod unset_in_body {
    use crate::{Parser, document::InterpretedValue, tests::prelude::*};

    non_normative!(
        r#"
== Unset a document attribute in the header

"#
    );

    #[ignore]
    #[test]
    fn sectnums_example() {
        to_do_verifies!(
            r#"
== Unset a document attribute in the body

Custom document attributes and some built-in document attributes can be turned off in the body of the document using an attribute entry and the bang symbol (`!`) as described in the previous section.
For example, let's say you set the section numbering attribute in the header of your document; however, you don't want the two sections midway through the document to be numbered.
To disable the numbering on these two sections, you'd unset `sectnums` before the first section you didn't want numbered and then reset it when you wanted the numbering to start again.

[source]
----
= Title
:sectnums: <.>

== Section Title

:!sectnums: <.>
== Section Title

=== Section Title

:sectnums: <.>
== Section Title
----
<.> The `sectnums` attribute is set in the header to activate section numbering throughout the document.
<.> `sectnums` is unset by adding a `!` to it's name.
The `!` can be placed either before or after the attribute's name.
The attribute entry must be placed on its own line.
All of the sections below where the attribute is unset will not be numbered.
<.> `sectnums` is set and all subsequent sections will be numbered.
"#
        );

        let mut parser = Parser::default();

        parser.parse("= Title\n:sectnums:\n\n== Section Title\n\nsectnums = {sectnums}\n\n:!sectnums:\n\nsectnums = {sectnums}\n\n== Section Title\n\n=== Section Title\n\n:sectnums:\n\n== Section Title\n\nsectnums = {sectnums}");

        assert_eq!(
            parser.attribute_value("sectnums"),
            InterpretedValue::Value("all".to_owned())
        );

        // TO DO (https://github.com/asciidoc-rs/asciidoc-parser/issues/328):
        // Differentiate between numbers on and numbers off when we can.
    }
}
