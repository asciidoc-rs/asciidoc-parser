use crate::tests::prelude::*;

track_file!("docs/modules/attributes/pages/attribute-entry-substitutions.adoc");

non_normative!(
    r#"
= Attribute Entry Substitutions

The AsciiDoc processor automatically applies substitutions from the header substitution group to the value of an attribute entry prior to the assignment, regardless of where the attribute entry is declared in the document.
"#
);

#[test]
fn applies_header_subs() {
    verifies!(
        r#"
The header substitution group, which replaces xref:subs:special-characters.adoc[special characters] followed by xref:subs:attributes.adoc[attribute references], is applied to the values of attribute entries, regardless of whether the entries are defined in the header or in the document body.
This is the same group that gets applied to metadata lines (author and revision information) in the document header.

That means that any inline formatting in an attribute value isn't interpreted because:

. inline formatting is not applied when the AsciiDoc processor sets an attribute, and
. inline formatting is not applied when an attribute is referenced since the relevant substitutions come before attributes are resolved.

"#
    );

    let mut parser = Parser::default();
    parser.parse(":special_chars: <tag_this>\n:y: yes\n:answer: {y}\n:format: *bold*");

    assert_eq!(
        parser
            .attribute_value("special_chars")
            .as_maybe_str()
            .unwrap(),
        "&lt;tag_this&gt;"
    );

    assert_eq!(
        parser.attribute_value("answer").as_maybe_str().unwrap(),
        "yes"
    );

    assert_eq!(
        parser.attribute_value("format").as_maybe_str().unwrap(),
        "*bold*"
    );
}

mod change_subs_when_assigning {
    use crate::{blocks::ContentModel, content::SubstitutionStep, tests::prelude::*};

    non_normative!(
        r#"
[#pass-macro]
== Change substitutions when assigning a value

If you want the value of an attribute entry to be used *as is* (not subject to substitutions), or you want to alter the substitutions that are applied, you can enclose the value in the xref:pass:pass-macro.adoc[inline pass macro] (i.e., `\pass:[]`).
The inline pass macro accepts a list of zero or more substitutions in the target slot, which can be used to control which substitutions are applied to the value.
If no substitutions are specified, no substitutions will be applied.

In order for the inline macro to work in this context, it must completely surround the attribute value.
If it's used anywhere else in the value, it will be ignored.

"#
    );

    #[test]
    fn prevent_subs_on_assignment() {
        verifies!(
            r#"
Here's how to prevent substitutions from being applied to the value of an attribute entry:

[source]
----
:cols: pass:[.>2,.>4]
----

This might be useful if you're referencing the attribute in a place that depends on the unaltered text, such as the value of the `cols` attribute on a table.

"#
        );

        let mut parser = Parser::default();
        parser.parse(":cols: pass:[.>2,.>4]");

        assert_eq!(
            parser.attribute_value("cols").as_maybe_str().unwrap(),
            ".>2,.>4"
        );
    }

    #[test]
    fn apply_quotes_subs() {
        verifies!(
            r#"
Here's how we can apply the xref:subs:quotes.adoc[quotes substitution] to the value of an attribute entry:

[source]
----
:app-name: pass:quotes[MyApp^2^]
----

Internally, the value is stored as `MyApp<sup>2</sup>`.
You can inspect the value stored in an attribute using this trick:

[source]
----
[subs=attributes+]
------
{app-name}
------
----

"#
        );

        let mut parser = Parser::default();
        let doc = parser.parse(
            ":app-name: pass:quotes[MyApp^2^]\n\n[subs=attributes+]\n------\n{app-name}\n------",
        );

        assert_eq!(
            parser.attribute_value("app-name").as_maybe_str().unwrap(),
            "MyApp<sup>2</sup>"
        );

        let mut blocks = doc.nested_blocks();

        let block1 = blocks.next().unwrap();

        assert_eq!(
            block1,
            &Block::RawDelimited(RawDelimitedBlock {
                content: Content {
                    original: Span {
                        data: "{app-name}",
                        line: 5,
                        col: 1,
                        offset: 60,
                    },
                    rendered: "MyApp&lt;sup&gt;2&lt;/sup&gt;",
                },
                content_model: ContentModel::Verbatim,
                context: "listing",
                source: Span {
                    data: "[subs=attributes+]\n------\n{app-name}\n------",
                    line: 3,
                    col: 1,
                    offset: 34,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: Some(Attrlist {
                    attributes: &[ElementAttribute {
                        name: Some("subs"),
                        value: "attributes+",
                        shorthand_items: &[],
                    },],
                    anchor: None,
                    source: Span {
                        data: "subs=attributes+",
                        line: 3,
                        col: 2,
                        offset: 35,
                    },
                },),
                substitution_group: SubstitutionGroup::Custom(vec![
                    SubstitutionStep::AttributeReferences,
                    SubstitutionStep::SpecialCharacters,
                ],),
            },)
        );
    }

    #[test]
    fn apply_quotes_single_char_alias() {
        verifies!(
            r#"
You can also specify the substitution using the single-character alias, `q`.

[source]
----
:app-name: pass:q[MyApp^2^]
----

The inline pass macro kind of works like an attribute value preprocessor.
If the processor detects that an inline pass macro completely surrounds the attribute value, it:

. reads the list of substitutions from the target slot of the macro
. unwraps the value from the macro
. applies the substitutions to the value

If the macro is absent, the value is processed with the header substitution group.

"#
        );

        let mut parser = Parser::default();
        let doc = parser
            .parse(":app-name: pass:q[MyApp^2^]\n\n[subs=attributes+]\n------\n{app-name}\n------");

        assert_eq!(
            parser.attribute_value("app-name").as_maybe_str().unwrap(),
            "MyApp<sup>2</sup>"
        );

        let mut blocks = doc.nested_blocks();

        let block1 = blocks.next().unwrap();

        assert_eq!(
            block1,
            &Block::RawDelimited(RawDelimitedBlock {
                content: Content {
                    original: Span {
                        data: "{app-name}",
                        line: 5,
                        col: 1,
                        offset: 55,
                    },
                    rendered: "MyApp&lt;sup&gt;2&lt;/sup&gt;",
                },
                content_model: ContentModel::Verbatim,
                context: "listing",
                source: Span {
                    data: "[subs=attributes+]\n------\n{app-name}\n------",
                    line: 3,
                    col: 1,
                    offset: 29,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: Some(Attrlist {
                    attributes: &[ElementAttribute {
                        name: Some("subs"),
                        value: "attributes+",
                        shorthand_items: &[],
                    },],
                    anchor: None,
                    source: Span {
                        data: "subs=attributes+",
                        line: 3,
                        col: 2,
                        offset: 30,
                    },
                },),
                substitution_group: SubstitutionGroup::Custom(vec![
                    SubstitutionStep::AttributeReferences,
                    SubstitutionStep::SpecialCharacters,
                ],),
            },)
        );
    }
}

mod attributes_defined_outside_document {
    use crate::tests::prelude::*;

    // Non-normative because we have a different API and no CLI.
    non_normative!(
        r#"
== Substitutions for attributes defined outside the document

Unlike attribute entries, substitutions are *not* applied to the value of an attribute passed in to the AsciiDoc processor.
An attribute can be passed into the AsciiDoc processor using the `-a` CLI option or the `:attributes` API option.
When attributes are defined external to the document, the value must be prepared so it's ready to be referenced as is.
If the value contains XML special characters, that means those characters must be pre-escaped.
The exception would be if you intend for XML/HTML tags in the value to be preserved.
If the value needs to reference other attributes, those values must be pre-replaced.

"#
    );

    #[test]
    fn escape_ampersand_example() {
        verifies!(
            r#"
Let's consider the case when the value of an attribute defined external to the document contains an ampersand.
In order to reference this attribute safely in the AsciiDoc document, the ampersand must be escaped:

 $ asciidoctor -a equipment="a bat &amp; ball" document.adoc

You can reference the attribute as follows:

[,asciidoc]
----
To play, you'll need {equipment}.
----

"#
        );

        let mut parser = Parser::default().with_intrinsic_attribute(
            "equipment",
            "a bat &amp; ball",
            crate::parser::ModificationContext::Anywhere,
        );

        let doc = parser.parse("To play, you'll need {equipment}.");

        let mut blocks = doc.nested_blocks();

        let block1 = blocks.next().unwrap();

        assert_eq!(
            block1,
            &Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "To play, you'll need {equipment}.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "To play, you&#8217;ll need a bat &amp; ball.",
                },
                source: Span {
                    data: "To play, you'll need {equipment}.",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },)
        );
    }

    #[test]
    fn defined_inline_example() {
        verifies!(
            r#"
If the attribute were to be defined in the document, this escaping would not be necessary.

[,asciidoc]
----
:equipment: a bat & ball
----

That's because, in contrast, substitutions are applied to the value of an attribute entry.

"#
        );

        let mut parser = Parser::default();

        let doc = parser.parse(
            ":equipment: a bat & ball
\n\nTo play, you'll need {equipment}.",
        );

        let mut blocks = doc.nested_blocks();

        let block1 = blocks.next().unwrap();

        assert_eq!(
            block1,
            &Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "To play, you'll need {equipment}.",
                        line: 4,
                        col: 1,
                        offset: 27,
                    },
                    rendered: "To play, you&#8217;ll need a bat &amp; ball.",
                },
                source: Span {
                    data: "To play, you'll need {equipment}.",
                    line: 4,
                    col: 1,
                    offset: 27,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },)
        );
    }
}

#[test]
fn change_subs_when_referencing() {
    verifies!(
        r#"
== Change substitutions when referencing an attribute

You can also change the substitutions that are applied to an attribute at the time it is resolved.
This is done by manipulating the substitutions applied to the text where it is referenced.
For example, here's how we could get the processor to apply quote substitutions to the value of an attribute:

[source]
----
:app-name: MyApp^2^

[subs="specialchars,attributes,quotes,replacements,macros,post_replacements"]
The application is called {app-name}.
----

Notice that we've swapped the order of the `attributes` and `quotes` substitutions.
This strategy is akin to post-processing the attribute value.
"#
    );

    let mut parser = Parser::default();

    let doc = parser.parse(":app-name: MyApp^2^\n\n[subs=\"specialchars,attributes,quotes,replacements,macros,post_replacements\"]\nThe application is called {app-name}.");

    assert_eq!(
        parser.attribute_value("app-name").as_maybe_str().unwrap(),
        "MyApp^2^"
    );

    let mut blocks = doc.nested_blocks();

    let block1 = blocks.next().unwrap();

    assert_eq!(
        block1,
        &Block::Simple(SimpleBlock {
            content: Content {
                original: Span {
                    data: "The application is called {app-name}.",
                    line: 4,
                    col: 1,
                    offset: 99,
                },
                rendered: "The application is called MyApp<sup>2</sup>.",
            },
            source: Span {
                data: "[subs=\"specialchars,attributes,quotes,replacements,macros,post_replacements\"]\nThe application is called {app-name}.",
                line: 3,
                col: 1,
                offset: 21,
            },
            style: SimpleBlockStyle::Paragraph,
            title_source: None,
            title: None,
            anchor: None,
            anchor_reftext: None,
            attrlist: Some(Attrlist {
                attributes: &[ElementAttribute {
                    name: Some("subs"),
                    value: "specialchars,attributes,quotes,replacements,macros,post_replacements",
                    shorthand_items: &[],
                },],
                anchor: None,
                source: Span {
                    data: "subs=\"specialchars,attributes,quotes,replacements,macros,post_replacements\"",
                    line: 3,
                    col: 2,
                    offset: 22,
                },
            },),
        },)
    );
}
