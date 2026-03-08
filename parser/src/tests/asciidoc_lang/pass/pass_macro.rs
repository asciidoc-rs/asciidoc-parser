use crate::{
    Parser,
    blocks::{Block, IsBlock},
    tests::prelude::*,
};
track_file!("docs/modules/pass/pages/pass-macro.adoc");

non_normative!(
    r#"
= Inline Passthroughs

AsciiDoc supports several inline passthrough macros and shorthands.
Inline passthroughs are designed to prevent subsitutions for regions of text, or to give you more fine-grained control over which substitutions are applied.

WARNING: Due to the fact that inline syntax in AsciiDoc is processed using substitutions rather than a descending grammar, it's possible to create invalid interleaving of inline elements, or other adverse interactions, that leads to invalid or illogical output.
The inline passthrough provides a bailout option to mitigate these entanglements.
This problem is expected to be resolved properly by the AsciiDoc Language Specification, which will mandate that inline syntax is parsed as a tree rather than through substitutions (to the degree possible).

"#
);

#[test]
fn inline_passthrough_macros() {
    verifies!(
        r#"
== Inline passthrough macros

[[def-plus]]single and double plus:: A special syntax for preventing inline text from being formatted.
Only xref:subs:special-characters.adoc[special characters] are replaced in the output format.
The substitutions can't be modified for this type of passthrough.

triple plus:: A special inline syntax for designating passthrough content.
No substitutions are applied nor can they be added using the step and group substitution values.

inline pass macro:: An inline macro named `pass` that can be used to passthrough content.
You can apply specific substitutions to the macro's target using substitution types and groups.
+
[source]
----
pass:[content like #{variable} passed directly to the output] followed by normal content.

content with only select substitutions applied: pass:c,a[__<{email}>__]
----

TIP: When you need to prevent or control the substitutions on one or more blocks of content, use a xref:pass-block.adoc[delimited passthrough block or the pass block style].

"#
    );

    let doc = Parser::default().parse(":email: foo@example.com\n\npass:[content like #{variable} passed directly to the output] followed by normal content.\n\ncontent with only select substitutions applied: pass:c,a[__<{email}>__]");

    let block1 = doc.nested_blocks().next().unwrap();

    let Block::Simple(sb1) = block1 else {
        panic!("Unexpected block type: {block1:?}");
    };

    assert_eq!(
        sb1.content().rendered(),
        "content like #{variable} passed directly to the output followed by normal content."
    );
}

#[test]
fn single_double_plus_1() {
    verifies!(
        r#"
[#single-double-plus]
== Single and double plus

The single and double plus passthroughs prevent text enclosed in either a pair of single pluses (`{plus}`) or a pair of double pluses (`{pp}`) from being formatted.

----
A +word+, a +sequence of words+, or ++char++acters that are escaped from formatting.
----

The single and double pluses represent the constrained and unconstrained passthrough, respectively.
They have boundaries that match the xref:text:index.adoc#formatting-marks-and-pairs[constrained and unconstrained formatting marks].
The main difference, however, is that they are applied first to suppress formatting.

"#
    );

    let doc = Parser::default().parse(
        "A +word+, a +sequence of words+, or ++char++acters that are escaped from formatting.",
    );

    let block1 = doc.nested_blocks().next().unwrap();

    let Block::Simple(sb1) = block1 else {
        panic!("Unexpected block type: {block1:?}");
    };

    assert_eq!(
        sb1.content().rendered(),
        "A word, a sequence of words, or characters that are escaped from formatting."
    );
}

#[test]
fn single_double_plus_2() {
    verifies!(
        r#"
This type of passthrough is intended to suppress any special meaning of the source text itself.
This passthrough type still ensures, however, that the content is properly escaped in the output.
That means the xref:subs:special-characters.adoc[special characters] substitution is still applied.

As with all constrained pairs, the single plus passthrough is designed to be used around a word or phrase.

----
A word or phrase between single pluses, such as +/document/{id}+, is not substituted.
However, the special characters +<+ and +>+ are still escaped in the output.

You can also escape formatting marks, like +``+.
----

"#
    );

    let doc = Parser::default().parse("A word or phrase between single pluses, such as +/document/{id}+, is not substituted.\nHowever, the special characters +<+ and +>+ are still escaped in the output.\n\nYou can also escape formatting marks, like +``+.");

    let mut blocks = doc.nested_blocks();

    let block1 = blocks.next().unwrap();

    let Block::Simple(sb1) = block1 else {
        panic!("Unexpected block type: {block1:?}");
    };

    assert_eq!(
        sb1.content().rendered(),
        "A word or phrase between single pluses, such as /document/{id}, is not substituted.\nHowever, the special characters &lt; and &gt; are still escaped in the output."
    );

    let block2 = blocks.next().unwrap();

    let Block::Simple(sb2) = block2 else {
        panic!("Unexpected block type: {block1:?}");
    };

    assert_eq!(
        sb2.content().rendered(),
        "You can also escape formatting marks, like ``."
    );
}

#[test]
fn single_double_plus_3() {
    verifies!(
        r#"
Being an unconstrained pair, the double plus passthrough can be used anywhere in the text.

----
Text formatting is not applied to a link target if it is surrounded by double pluses.
For example, link:++https://example.org/now_this__link_works.html++[].

You can also escape formatting marks, like all-natural++*++.

An attribute reference within a word, such as dev++{conf}++, is not replaced.
----

The single and plus passthroughs are a surefire alternative to backslash escaping.

Note that the single and plus passthroughs only prevent substitutions.
They do not format the text in monospace.
If you want to do both, you must enclose the pair in a monospace formatting pair, known as xref:text:literal-monospace.adoc[literal monospace].

"#
    );

    let doc = Parser::default().parse(":conf: /prod\n\nText formatting is not applied to a link target if it is surrounded by double pluses.\nFor example, link:++https://example.org/now_this__link_works.html++[].\n\nYou can also escape formatting marks, like all-natural++*++.\n\nAn attribute reference within a word, such as dev++{conf}++, is not replaced.");

    let mut blocks = doc.nested_blocks();

    let block1 = blocks.next().unwrap();

    let Block::Simple(sb1) = block1 else {
        panic!("Unexpected block type: {block1:?}");
    };

    assert_eq!(
        sb1.content().rendered(),
        "Text formatting is not applied to a link target if it is surrounded by double pluses.\nFor example, <a href=\"https://example.org/now_this__link_works.html\" class=\"bare\">https://example.org/now_this__link_works.html</a>."
    );

    let block2 = blocks.next().unwrap();

    let Block::Simple(sb2) = block2 else {
        panic!("Unexpected block type: {block1:?}");
    };

    assert_eq!(
        sb2.content().rendered(),
        "You can also escape formatting marks, like all-natural*."
    );

    let block3 = blocks.next().unwrap();

    let Block::Simple(sb3) = block3 else {
        panic!("Unexpected block type: {block1:?}");
    };

    assert_eq!(
        sb3.content().rendered(),
        "An attribute reference within a word, such as dev{conf}, is not replaced."
    );
}

#[test]
fn triple_plus() {
    verifies!(
        r#"
[#triple-plus]
== Triple plus

The triple plus passthrough excludes content enclosed in a pair of triple pluses (pass:[+++]) from all substitutions.

 +++content passed directly to the output+++ followed by normal content.

The triple plus macro is often used to output custom HTML or XML.

[source]
----
include::example$pass.adoc[tag=3p]
----

====
include::example$pass.adoc[tag=3p]
====

"#
    );

    let doc =
        Parser::default().parse("The text +++<del>strike this</del>+++ is marked as deleted.");

    let block1 = doc.nested_blocks().next().unwrap();

    let Block::Simple(sb1) = block1 else {
        panic!("Unexpected block type: {block1:?}");
    };

    assert_eq!(
        sb1.content().rendered(),
        "The text <del>strike this</del> is marked as deleted."
    );
}

#[test]
fn inline_pass_macro() {
    verifies!(
        r#"
[#inline-pass]
== Inline pass macro

Like other inline passthroughs, the inline pass macro can be used to control the substitutions applied to a run of text.
To exclude inline content from all of the substitutions, enclose it in the inline pass macro.

Here's one way to format text as underline when generating HTML from AsciiDoc:

[source]
----
include::example$pass.adoc[tag=in-macro]
----

And here's the result.

====
include::example$pass.adoc[tag=in-macro]
====

WARNING: Using passthroughs to send content directly to the output can couple your content to a specific output format, such as HTML.
To avoid this risk, you should consider using conditional preprocessor directives to select content for different output formats based on the current backend.

What sets the inline pass macro apart from the alternatives is that it allows the substitutions to be customized.
The inline pass macro also plays a critical role in the document header.
In fact, it's the only macro that is processed in the document header by default as part of the xref:subs:index.adoc#header-group[header substitution group] (though it can be used to enable other substitutions, as demonstrated in this section).

Let's look at how to use the inline pass macro to hand select substitutions.

"#
    );

    let doc = Parser::default().parse(
        "The text pass:[<del>strike this</del>] is marked as deleted.
",
    );

    let block1 = doc.nested_blocks().next().unwrap();

    let Block::Simple(sb1) = block1 else {
        panic!("Unexpected block type: {block1:?}");
    };

    assert_eq!(
        sb1.content().rendered(),
        "The text <del>strike this</del> is marked as deleted."
    );
}

#[test]
fn custom_substitutions_1() {
    verifies!(
        r#"
=== Custom substitutions

You can customize the substitutions applied to the content of an inline pass macro by specifying one or more substitution values in the target of the macro.
Multiple values must be separated by commas and may not contain any spaces.
The substitution value is either the formal name of a substitution type or group, or its shorthand.

The following table lists the allowable substitution values:

.Substitution values accepted by the inline pass macro
[cols="1m,3m",width=50%]
|===
|Shorthand | Substitution Type

|c
|specialchars

|q
|quotes

|a
|attributes

|r
|replacements

|m
|macros

|p
|post replacements

h|Shorthand
h| Substitution Group

|n
|normal

|v
|verbatim
|===

For example, the quotes substitution (i.e., `q` or `quotes`) is enabled on the inline passthrough macro as follows:

[source]
----
include::example$pass.adoc[tag=in-macro-sub]
----

Here's the result.

====
include::example$pass.adoc[tag=in-macro-sub]
====

"#
    );

    let doc =
        Parser::default().parse("The text pass:q[<del>strike *this*</del>] is marked as deleted.");

    let block1 = doc.nested_blocks().next().unwrap();

    let Block::Simple(sb1) = block1 else {
        panic!("Unexpected block type: {block1:?}");
    };

    assert_eq!(
        sb1.content().rendered(),
        "The text <del>strike <strong>this</strong></del> is marked as deleted."
    );
}

#[test]
fn custom_substitutions_2() {
    verifies!(
        r#"
To enable multiple substitution groups, separate each value in the macro target by a comma:

[source]
----
include::example$pass.adoc[tag=in-macro-subs]
----

Here's the result.

====
include::example$pass.adoc[tag=in-macro-subs]
====

"#
    );

    let doc = Parser::default().parse(":docname: untitled document 1\n\nThe text pass:q,a[<del>strike _{docname}_</del>] is marked as deleted.");

    let block1 = doc.nested_blocks().next().unwrap();

    let Block::Simple(sb1) = block1 else {
        panic!("Unexpected block type: {block1:?}");
    };

    assert_eq!(
        sb1.content().rendered(),
        "The text <del>strike <em>untitled document 1</em></del> is marked as deleted."
    );
}

#[test]
fn nesting_blocks_and_paragraphs() {
    verifies!(
        r#"
== Nesting blocks and passthroughs

When you're using passthroughs inside literal and listing blocks, it can be easy to forget that the single plus and triple plus passthroughs are xref:subs:macros.adoc[macros substitutions].
To enable the passthroughs, assign the `macros` value to the `subs` attribute.

....
[source,java,subs="+quotes,+macros"]
----
protected void configure(HttpSecurity http) throws Exception {
    http
        .authorizeRequests()
            **.antMatchers("/resources/+++**+++").permitAll()**
            .anyRequest().authenticated()
            .and()
        .formLogin()
            .loginPage("/login")
            .permitAll();
----
....

To learn more about applying substitutions to blocks, see xref:subs:apply-subs-to-blocks.adoc[].

[source,java,subs="+quotes,+macros"]
----
protected void configure(HttpSecurity http) throws Exception {
    http
        .authorizeRequests()
            **.antMatchers("/resources/+++**+++").permitAll()**
            .anyRequest().authenticated()
            .and()
        .formLogin()
            .loginPage("/login")
            .permitAll();
----
"#
    );

    let doc = Parser::default().parse("[source,java,subs=\"+quotes,+macros\"]\n----\nprotected void configure(HttpSecurity http) throws Exception {\n    http\n        .authorizeRequests()\n            **.antMatchers(\"/resources/+++**+++\").permitAll()**\n            .anyRequest().authenticated()\n            .and()\n        .formLogin()\n            .loginPage(\"/login\")\n            .permitAll();\n----");

    let block1 = doc.nested_blocks().next().unwrap();

    let Block::RawDelimited(rdb1) = block1 else {
        panic!("Unexpected block type: {block1:?}");
    };

    assert_eq!(
        rdb1.content().rendered(),
        "protected void configure(HttpSecurity http) throws Exception {\n    http\n        .authorizeRequests()\n            <strong>.antMatchers(\"/resources/**\").permitAll()</strong>\n            .anyRequest().authenticated()\n            .and()\n        .formLogin()\n            .loginPage(\"/login\")\n            .permitAll();"
    );
}
