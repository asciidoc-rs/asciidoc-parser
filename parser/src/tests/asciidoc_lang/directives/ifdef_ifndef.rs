use crate::tests::prelude::*;

track_file!("ref/asciidoc-lang/docs/modules/directives/pages/ifdef-ifndef.adoc");

non_normative!(
    r#"
= ifdef and ifndef Directives

"#
);

#[test]
fn ifdef_directive() {
    verifies!(
        r#"
[#ifdef]
== ifdef directive

Content between the `ifdef` and `endif` directives gets included if the specified attribute is set:

.ifdef example
----
\ifdef::env-github[]
This content is for GitHub only.
\endif::[]
----

The syntax of the start directive is `ifdef::<attribute>[]`, where `<attribute>` is the name of an attribute.

Keep in mind that the content is not limited to a single line.
You can have any amount of content between the `ifdef` and `endif` directives.

"#
    );

    // The attribute is set, so the enclosed content is included.
    let doc = Parser::default()
        .parse(":env-github:\n\nifdef::env-github[]\nThis content is for GitHub only.\nendif::[]");
    assert_eq!(
        rendered_paragraphs(&doc),
        vec!["This content is for GitHub only."]
    );

    // The attribute is not set, so the enclosed content is excluded.
    let doc = Parser::default()
        .parse("ifdef::env-github[]\nThis content is for GitHub only.\nendif::[]\n\nTail.");
    assert_eq!(rendered_paragraphs(&doc), vec!["Tail."]);
}

#[test]
fn ifdef_long_form() {
    verifies!(
        r#"
If you have a large amount of content inside the `ifdef` directive, you may find it more readable to use the long-form version of the directive, in which the attribute (aka condition) is referenced again in the `endif` directive.

.ifdef long-form example
----
\ifdef::env-github[]
This content is for GitHub only.

So much content in this section, I'd get confused reading the source without the closing `ifdef` directive.

It isn't necessary for short blocks, but if you are conditionally including a section it may be something worth considering.

Other readers reviewing your docs source code may go cross-eyed when reading your source docs if you don't.
\endif::env-github[]
----

"#
    );

    // The long form names the attribute again on the `endif` directive; the
    // content may span multiple paragraphs.
    let doc = Parser::default().parse(
        ":env-github:\n\nifdef::env-github[]\nFirst paragraph.\n\nSecond paragraph.\nendif::env-github[]",
    );
    assert_eq!(
        rendered_paragraphs(&doc),
        vec!["First paragraph.", "Second paragraph."]
    );
}

#[test]
fn ifdef_single_line() {
    verifies!(
        r#"
If you're only dealing with a single line of text, you can put the content directly inside the square brackets and drop the `endif` directive.

.ifdef single line example
----
\ifdef::revnumber[This document has a version number of {revnumber}.]
----

The single-line block above is equivalent to this formal `ifdef` directive:

----
\ifdef::revnumber[]
This document has a version number of {revnumber}.
\endif::[]
----

"#
    );

    // The single-line form puts the content directly inside the brackets and
    // drops the `endif` directive.
    let doc = Parser::default().parse(
        ":revnumber: 2.0\n\nifdef::revnumber[This document has a version number of {revnumber}.]",
    );
    assert_eq!(
        rendered_paragraphs(&doc),
        vec!["This document has a version number of 2.0."]
    );

    // It is equivalent to the formal (block) form.
    let doc = Parser::default().parse(
        ":revnumber: 2.0\n\nifdef::revnumber[]\nThis document has a version number of {revnumber}.\nendif::[]",
    );
    assert_eq!(
        rendered_paragraphs(&doc),
        vec!["This document has a version number of 2.0."]
    );
}

#[test]
fn ifndef_directive() {
    verifies!(
        r#"
[#ifndef]
== ifndef directive

`ifndef` is the logical opposite of `ifdef`.
Content between `ifndef` and `endif` gets included only if the specified attribute is _not_ set:

.ifndef example
----
\ifndef::env-github[]
This content is not shown on GitHub.
\endif::[]
----

The syntax of the start directive is `ifndef::<attribute>[]`, where `<attribute>` is the name of an attribute.

The `ifndef` directive supports the same single-line and long-form variants as `ifdef`.

"#
    );

    // The attribute is not set, so the enclosed content is included.
    let doc = Parser::default()
        .parse("ifndef::env-github[]\nThis content is not shown on GitHub.\nendif::[]");
    assert_eq!(
        rendered_paragraphs(&doc),
        vec!["This content is not shown on GitHub."]
    );

    // The attribute is set, so the enclosed content is excluded.
    let doc = Parser::default()
        .parse(":env-github:\n\nifndef::env-github[]\nHidden.\nendif::[]\n\nTail.");
    assert_eq!(rendered_paragraphs(&doc), vec!["Tail."]);
}

non_normative!(
    r#"
== Checking multiple attributes

Both the `ifdef` and `ifndef` directives accept multiple attribute names.
The combinator can be "`and`" or "`or`".
"#
);

#[test]
fn combinators_cannot_be_mixed() {
    verifies!(
        r#"
The two combinators cannot be combined in the same expression.

"#
    );

    // The spec forbids mixing the `,` (or) and `+` (and) combinators in a single
    // expression, but the parser (like Asciidoctor) neither rejects nor warns
    // about a mixed expression. Instead, whichever combinator appears *first*
    // governs the whole expression, and the target is split on that delimiter
    // alone — so the other delimiter becomes part of a single literal attribute
    // name. Since no attribute is ever named (for example) `b+c`, such a segment
    // can never match.

    // `,` appears first, so this is an "`or`": the `a` segment is set, so the
    // content is included (the `b+c` segment is never consulted).
    let doc = Parser::default().parse(":a:\n:b:\n:c:\n\nifdef::a,b+c[Shown.]");
    assert_eq!(rendered_paragraphs(&doc), vec!["Shown."]);

    // Still an "`or`" (`,` first): with `a` unset, the `b+c` segment is a single
    // never-set attribute name — the `+` is not honored as an "`and`" — so the
    // content is excluded even though both `b` and `c` are set.
    let doc = Parser::default().parse(":b:\n:c:\n\nifdef::a,b+c[Shown.]\n\nTail.");
    assert_eq!(rendered_paragraphs(&doc), vec!["Tail."]);

    // `+` appears first, so this is an "`and`": the `b,c` segment is a single
    // never-set attribute name — the `,` is not honored as an "`or`" — so the
    // content is excluded even though every real attribute (`a`, `b`, `c`) is
    // set.
    let doc = Parser::default().parse(":a:\n:b:\n:c:\n\nifdef::a+b,c[Shown.]\n\nTail.");
    assert_eq!(rendered_paragraphs(&doc), vec!["Tail."]);
}

#[test]
fn ifdef_with_multiple_attributes() {
    verifies!(
        r#"
=== ifdef with multiple attributes

If any attribute is set (or)::
Multiple attribute names must be separated by commas (`,`).
If one or more of the attributes are set, the content is included.
Otherwise, the content is not included.
+
.If any attribute example
----
\ifdef::backend-html5,backend-docbook5[Only shown if converting to HTML (backend-html5 is set) or DocBook (backend-docbook5 is set).]
----

If all attributes are set (and)::
Multiple attribute names must be separated by pluses (`+`).
If all the attributes are set, the content is included.
Otherwise, the content is not included.
+
.If all attributes example
----
\ifdef::backend-html5+env-github[Only shown when converting to HTML (backend-html5 is set) on GitHub (env-github is set).]
----

"#
    );

    // OR (comma): the default backend is `html5`, so the derived `backend-html5`
    // flag is set and the content is included.
    let doc = Parser::default().parse("ifdef::backend-html5,backend-docbook5[Shown.]");
    assert_eq!(rendered_paragraphs(&doc), vec!["Shown."]);

    // OR (comma): with a different backend neither flag is set, so the content
    // is not included.
    let doc = Parser::default()
        .parse(":backend: pdf\n\nifdef::backend-html5,backend-docbook5[Shown.]\n\nTail.");
    assert_eq!(rendered_paragraphs(&doc), vec!["Tail."]);

    // AND (plus): all attributes are set, so the content is included.
    let doc = Parser::default()
        .parse(":backend-html5:\n:env-github:\n\nifdef::backend-html5+env-github[Shown.]");
    assert_eq!(rendered_paragraphs(&doc), vec!["Shown."]);

    // AND (plus): only one attribute is set, so the content is not included.
    let doc = Parser::default()
        .parse(":backend-html5:\n\nifdef::backend-html5+env-github[Shown.]\n\nTail.");
    assert_eq!(rendered_paragraphs(&doc), vec!["Tail."]);
}

non_normative!(
    r#"
=== ifndef with multiple attributes

The `ifndef` directive negates the results of the expression.
When using the `ifndef` directive, the expression should be read with the prefix "`unless`".

"#
);

#[test]
fn ifndef_with_multiple_attributes() {
    verifies!(
        r#"
Unless any attribute is set (or)::
Multiple attribute names must be separated by commas (`,`).
If one or more of the attributes are set, the content is not included.
Otherwise, the content is included.
+
.Unless any attribute example
----
\ifndef::profile-production,env-site[Not shown if profile-production or env-site is set.]
----

Unless all attributes are set (and)::
Multiple attribute names must be separated by pluses (`+`).
If all of the attributes are set, the content is not included.
Otherwise, the content is included.
+
.Unless all attributes example
----
\ifndef::profile-staging+env-site[Not shown if profile-staging and env-site are set.]
----
"#
    );

    // "Unless any" (comma): neither attribute is set, so the content is included.
    let doc = Parser::default().parse("ifndef::profile-production,env-site[Shown.]");
    assert_eq!(rendered_paragraphs(&doc), vec!["Shown."]);

    // "Unless any" (comma): one attribute is set, so the content is not included.
    let doc = Parser::default()
        .parse(":env-site:\n\nifndef::profile-production,env-site[Shown.]\n\nTail.");
    assert_eq!(rendered_paragraphs(&doc), vec!["Tail."]);

    // "Unless all" (plus): all attributes are set, so the content is not included.
    let doc = Parser::default().parse(
        ":profile-staging:\n:env-site:\n\nifndef::profile-staging+env-site[Shown.]\n\nTail.",
    );
    assert_eq!(rendered_paragraphs(&doc), vec!["Tail."]);

    // "Unless all" (plus): only one attribute is set, so the content is included.
    let doc = Parser::default().parse(":env-site:\n\nifndef::profile-staging+env-site[Shown.]");
    assert_eq!(rendered_paragraphs(&doc), vec!["Shown."]);
}
