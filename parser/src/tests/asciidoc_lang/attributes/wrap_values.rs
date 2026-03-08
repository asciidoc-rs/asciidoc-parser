use crate::{Parser, tests::prelude::*};

track_file!("docs/modules/attributes/pages/wrap-values.adoc");

non_normative!(
    r#"
= Wrap Attribute Entry Values

"#
);

#[test]
fn soft_wrap_attribute_values() {
    verifies!(
        r#"
== Soft wrap attribute values

If the value of a document attribute is too long to fit on the screen, you can split the value across multiple lines with a line continuation to make it easier to read.

A [.term]*line continuation* consists of a space followed by a backslash character (`\`) at the end of the line.
The line continuation must be placed on every line of a multiline value except for the last line.
Lines that follow a line continuation character may be indented, but that indentation will not be included in the value.

When the processor reads the attribute value, it folds the line continuation, the newline, and any ensuing indentation into a single space.
In this case, we can say that the attribute value has soft wraps.

Let's assume we want to define an attribute named `description` that has a very long value.
We can split this attribute up across multiple lines by placing a line continuation at the end of each line of the value except for the last.

.A multiline attribute value with soft wraps
[source]
----
:description: If you have a very long line of text \
that you need to substitute regularly in a document, \
you may find it easier to split the value neatly in the header \
so it remains readable to folks looking at the AsciiDoc source.
----

If the line continuation is missing, the processor will assume it has found the end of the value and will not include subsequent lines in the value of the attribute.

"#
    );

    let mi = crate::document::Attribute::parse(crate::Span::new(":description: If you have a very long line of text \\\nthat you need to substitute regularly in a document, \\\nyou may find it easier to split the value neatly in the header \\\nso it remains readable to folks looking at the AsciiDoc source."), &Parser::default()).unwrap();

    assert_eq!(
        mi.item,
        Attribute {
            name: Span {
                data: "description",
                line: 1,
                col: 2,
                offset: 1,
            },
            value_source: Some(Span {
                data: "If you have a very long line of text \\\nthat you need to substitute regularly in a document, \\\nyou may find it easier to split the value neatly in the header \\\nso it remains readable to folks looking at the AsciiDoc source.",
                line: 1,
                col: 15,
                offset: 14,
            }),
            value: InterpretedValue::Value(
                "If you have a very long line of text that you need to substitute regularly in a document, you may find it easier to split the value neatly in the header so it remains readable to folks looking at the AsciiDoc source."
            ),
            source: Span {
                data: ":description: If you have a very long line of text \\\nthat you need to substitute regularly in a document, \\\nyou may find it easier to split the value neatly in the header \\\nso it remains readable to folks looking at the AsciiDoc source.",
                line: 1,
                col: 1,
                offset: 0,
            },
        }
    );

    assert_eq!(
        mi.item.value(),
        &crate::document::InterpretedValue::Value(
            "If you have a very long line of text that you need to substitute regularly in a document, you may find it easier to split the value neatly in the header so it remains readable to folks looking at the AsciiDoc source.".to_string()
        )
    );
}

#[test]
fn hard_wrap_attribute_values() {
    verifies!(
        r#"
[#hard]
== Hard wrap attribute values

You can force an attribute value to hard wrap by inserting a hard line break replacement in front of the line continuation.
A hard line break replace is a space followed by a plus character (`+`).

As described in the previous section, the line continuation, newline, and ensuing indentation is normally replaced with a space.
This would prevent the hard line break replacement from being recognized.
However, the processor accounts for this scenario and leaves the newline intact.

Let's assume we want to define an attribute named `haiku` that requires hard line breaks.
We can split this attribute up across multiple lines and preserve those line breaks by placing a hard line break replacement followed by a line continuation at the end of each line of the value except for the last.

.A multiline attribute value with hard wraps
[source]
----
:haiku: Write your docs in text, + \
AsciiDoc makes it easy, + \
Now get back to work!
----

This syntax ensures that the newlines are preserved in the output as hard line breaks.
"#
    );

    let mi = crate::document::Attribute::parse(crate::Span::new(":haiku: Write your docs in text, + \\\nAsciiDoc makes it easy, + \\\nNow get back to work!"), &Parser::default()).unwrap();

    assert_eq!(
        mi.item,
        Attribute {
            name: Span {
                data: "haiku",
                line: 1,
                col: 2,
                offset: 1,
            },
            value_source: Some(Span {
                data: "Write your docs in text, + \\\nAsciiDoc makes it easy, + \\\nNow get back to work!",
                line: 1,
                col: 9,
                offset: 8,
            }),
            value: InterpretedValue::Value(
                "Write your docs in text,\nAsciiDoc makes it easy,\nNow get back to work!"
            ),
            source: Span {
                data: ":haiku: Write your docs in text, + \\\nAsciiDoc makes it easy, + \\\nNow get back to work!",
                line: 1,
                col: 1,
                offset: 0,
            },
        }
    );

    assert_eq!(
        mi.item.value(),
        &crate::document::InterpretedValue::Value(
            "Write your docs in text,\nAsciiDoc makes it easy,\nNow get back to work!".to_string()
        ),
    );
}
