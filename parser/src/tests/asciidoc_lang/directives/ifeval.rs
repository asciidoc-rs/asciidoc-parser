use crate::tests::prelude::*;

track_file!("docs/modules/directives/pages/ifeval.adoc");

non_normative!(
    r#"
= ifeval Directive

"#
);

#[test]
fn ifeval_included_when_expression_is_true() {
    verifies!(
        r#"
Lines enclosed by an `ifeval` directive (i.e., between the `ifeval` and `endif` directives) are included if the expression inside the square brackets of the `ifeval` directive evaluates to true.

.ifeval example
----
\ifeval::[{sectnumlevels} == 3]
If the `sectnumlevels` attribute has the value 3, this sentence is included.
\endif::[]
----

"#
    );

    // `sectnumlevels` defaults to 3, so the expression is true and the sentence
    // is included.
    let doc = Parser::default().parse(
        "ifeval::[{sectnumlevels} == 3]\nIf the sectnumlevels attribute has the value 3, this sentence is included.\nendif::[]",
    );
    assert_eq!(
        rendered_paragraphs(&doc),
        vec!["If the sectnumlevels attribute has the value 3, this sentence is included."]
    );

    // When the expression is false the enclosed lines are skipped.
    let doc =
        Parser::default().parse("ifeval::[{sectnumlevels} == 5]\nHidden.\nendif::[]\n\nTail.");
    assert_eq!(rendered_paragraphs(&doc), vec!["Tail."]);
}

#[test]
fn ifeval_terminated_only_by_anonymous_endif() {
    verifies!(
        r#"
The `ifeval` directive does not have a single-line or long-form variant like `ifdef` and `ifndef`.

Unlike `ifdef` and `ifndef`, you cannot terminate a specific `ifeval` directive using its complement.
For example, the following `ifeval` block is not valid:

.Invalid ifeval terminator
----
\ifeval::[<condition>]
conditional content
\endif::[<condition>]
----

You can only terminate the previous `ifeval` directive using an anonymous `endif::[]` directive, as shown here:

.Valid ineval terminator
----
\ifeval::[<condition>]
conditional content
\endif::[]
----

"#
    );

    // An anonymous `endif::[]` terminates the `ifeval` directive.
    let doc = Parser::default().parse("ifeval::[1 == 1]\nconditional content\nendif::[]");
    assert_eq!(rendered_paragraphs(&doc), vec!["conditional content"]);
}

non_normative!(
    r#"
If you're mixing `ifeval` directives with `ifdef` or `ifndef` directives, you should always close multiline `ifdef` and `ifndef` directives by name (`endif::name-of-attribute[]`) so the `ifeval` directive does not end prematurely.

== Anatomy

The expression of an `ifeval` directive consists of a left-hand value and a right-hand value with an operator in between.
It's customary to include a single space on either side of the operator.

.ifeval expression examples
----
\ifeval::[2 > 1]
...
\endif::[]

\ifeval::["{backend}" == "html5"]
...
\endif::[]

\ifeval::[{sectnumlevels} == 3]
...
\endif::[]

// the value of outfilesuffix includes a leading period (e.g., .html)
\ifeval::["{docname}{outfilesuffix}" == "main.html"]
...
\endif::[]
----

"#
);

#[test]
fn values() {
    verifies!(
        r#"
== Values

Each expression value can reference the name of zero or more AsciiDoc attributes using the attribute reference syntax (for example, `+{backend}+`).

Attribute references are resolved (i.e., substituted) first.
Once attributes references have been resolved, each value is coerced to a recognized type.

When you expect the attribute reference to resolve to a string, that is, a sequence of characters, enclose that side of the expression in quotes.
For example:

.ifeval that compares two string expressions
----
\ifeval::["{backend}" == "html5"]
----

If you expect the attribute to resolve to a number, you do not need to enclose the expression in quotes.
In this case, the values will be compared as numbers.
The same rule applies to boolean values.

You should not attempt to mix value types in a comparison.
For example, the following expression is not valid:

.Invalid ifeval expression
----
\ifeval::["{sectnumlevels}" > 3]
----

The following values types are recognized:

number:: Either an integer or floating-point value.
quoted string:: Enclosed in either single (`'`) or double (`"`) quotes.
boolean:: Literal value of `true` or `false`.

"#
    );

    // Attribute references are resolved first; a quoted side is compared as a
    // string.
    let doc = Parser::default()
        .parse(":backend: html5\n\nifeval::[\"{backend}\" == \"html5\"]\nMatched.\nendif::[]");
    assert_eq!(rendered_paragraphs(&doc), vec!["Matched."]);

    // Unquoted numeric values are compared as numbers.
    let doc = Parser::default().parse("ifeval::[10 > 9]\nMatched.\nendif::[]");
    assert_eq!(rendered_paragraphs(&doc), vec!["Matched."]);

    // Boolean values are recognized.
    let doc = Parser::default().parse("ifeval::[true != false]\nMatched.\nendif::[]");
    assert_eq!(rendered_paragraphs(&doc), vec!["Matched."]);

    // Mixing value types (a quoted string against a number) is not valid: the
    // comparison fails, so the content is skipped.
    let doc = Parser::default()
        .parse(":backend: html5\n\nifeval::[\"{sectnumlevels}\" > 3]\nHidden.\nendif::[]\n\nTail.");
    assert_eq!(rendered_paragraphs(&doc), vec!["Tail."]);
}

#[test]
fn value_type_coercion() {
    verifies!(
        r#"
=== How value type coercion works

If a value is enclosed in quotes, the characters between the quotes is used and always coerced to a string.

If a value is *not* enclosed in quotes, it's subject to the following type coercion rules:

* an empty value becomes nil (aka null) (and thus safe for use in a comparison).
* a value of `true` or `false` becomes a boolean value.
* a value of only repeating whitespace becomes a single whitespace string.
* a value containing a period becomes a floating-point number.
* any other value is coerced to an integer value.

"#
    );

    // An empty value becomes nil, and two nil values compare equal.
    let doc = Parser::default().parse("ifeval::[{empty} == {empty}]\nMatched.\nendif::[]");
    assert_eq!(rendered_paragraphs(&doc), vec!["Matched."]);

    // A value of `true` or `false` becomes a boolean value.
    let doc = Parser::default().parse("ifeval::[true == true]\nMatched.\nendif::[]");
    assert_eq!(rendered_paragraphs(&doc), vec!["Matched."]);

    // A value of only repeating whitespace becomes a single whitespace string.
    let doc = Parser::default().parse("ifeval::[{sp}{sp} == {sp}]\nMatched.\nendif::[]");
    assert_eq!(rendered_paragraphs(&doc), vec!["Matched."]);

    // A value containing a period becomes a floating-point number.
    let doc = Parser::default().parse("ifeval::[1.5 < 2]\nMatched.\nendif::[]");
    assert_eq!(rendered_paragraphs(&doc), vec!["Matched."]);

    // Any other value is coerced to an integer value.
    let doc = Parser::default().parse("ifeval::[10 == 10]\nMatched.\nendif::[]");
    assert_eq!(rendered_paragraphs(&doc), vec!["Matched."]);
}

#[test]
fn operators() {
    verifies!(
        r#"
== Operators

The value on each side is compared using the operator to derive an outcome.

`==`::
Checks if the two values are equal.
`!=`::
Checks if the two values are not equal.
`<`::
Checks whether the left-hand side is less than the right-hand side.
`+<=+`::
Checks whether the left-hand side is less than or equal to the right-hand side.
`>`::
Checks whether the left-hand side is greater than the right-hand side.
`+>=+`::
Checks whether the left-hand side is greater than or equal to the right-hand side.

Both sides should be of the same value type.
If they are not, the comparison will fail.
If the comparison fails, the condition will evaluate to false (i.e., the content inside the directive will be skipped).

The operators follow the same rules as operators in Ruby.
"#
    );

    // Each operator derives the expected outcome.
    for (expr, included) in [
        ("3 == 3", true),
        ("3 == 4", false),
        ("3 != 4", true),
        ("2 < 3", true),
        ("3 <= 3", true),
        ("4 > 3", true),
        ("3 >= 3", true),
    ] {
        let source = format!("ifeval::[{expr}]\nMatched.\nendif::[]\n\nTail.");
        let doc = Parser::default().parse(&source);
        let expected: Vec<&str> = if included {
            vec!["Matched.", "Tail."]
        } else {
            vec!["Tail."]
        };
        assert_eq!(rendered_paragraphs(&doc), expected, "expr: {expr}");
    }

    // When the two sides are not the same value type the comparison fails and
    // the content is skipped.
    let doc = Parser::default().parse("ifeval::[1 < \"a\"]\nHidden.\nendif::[]\n\nTail.");
    assert_eq!(rendered_paragraphs(&doc), vec!["Tail."]);
}
