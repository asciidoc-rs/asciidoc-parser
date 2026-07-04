use crate::tests::prelude::*;

track_file!("ref/asciidoc-lang/docs/modules/attributes/pages/unresolved-references.adoc");

non_normative!(
    r#"
= Handle Unresolved References

When you reference a missing attribute (e.g., `+{does-not-exist}+`), the AsciiDoc processor will leave the attribute reference behind.
If you undefine an attribute on the same line as other text (e.g., `+{set:attribute-no-more!}+`), the processor will drop the whole line.
You can tailor these behaviors using the `attribute-missing` and `attribute-undefined` attributes.
You'll want to think about how you want the processor to handle these situations and configure the processor accordingly.

"#
);

/// Renders `source` and returns the rendered content of its first block, which
/// is what the `attribute-missing` table in the spec describes.
fn render_first_block(source: &str) -> String {
    let doc = Parser::default().parse(source);
    doc.nested_blocks()
        .next()
        .and_then(|b| b.rendered_content())
        .unwrap_or_default()
        .to_string()
}

#[test]
fn missing() {
    verifies!(
        r#"
[#missing]
== Missing attribute

The `attribute-missing` attribute controls how missing references are handled.
By default, missing references are left behind so the integrity of the document is preserved and it's easy for the author to track down.

This attribute has four possible values:

`skip`:: leave the reference in place (default setting)
`drop`:: drop the reference, but not the line
`drop-line`:: drop the line on which the reference occurs (matches behavior of AsciiDoc.py)
`warn`:: print a warning about the missing attribute

The setting you might find of most interest is `warn`, which gives you a warning whenever the processor encounters an attribute reference that cannot be resolved, but otherwise leaves the line alone.

Consider the following line:

[source]
Hello, {name}!

Here's how the line is handled in each case, assuming the `name` attribute is not defined:

[%autowidth]
|===
|`attribute-missing` value |Result

|`skip` |Hello, \{name}!

|`drop` |Hello, !

|`drop-line` |{empty}

|`warn` |`asciidoctor: WARNING: skipping reference to missing attribute: XYZ`
|===

"#
    );

    // `skip` (the default) leaves the reference in place.
    assert_eq!(render_first_block("Hello, {name}!"), "Hello, {name}!");
    assert_eq!(
        render_first_block(":attribute-missing: skip\n\nHello, {name}!"),
        "Hello, {name}!"
    );

    // `drop` removes the reference but keeps the line.
    assert_eq!(
        render_first_block(":attribute-missing: drop\n\nHello, {name}!"),
        "Hello, !"
    );

    // `drop-line` drops the entire line, leaving the (single-line) block empty.
    assert_eq!(
        render_first_block(":attribute-missing: drop-line\n\nHello, {name}!"),
        ""
    );

    // `warn` leaves the line alone but records a warning (see `warn_*` tests
    // below for the warning itself).
    assert_eq!(
        render_first_block(":attribute-missing: warn\n\nHello, {name}!"),
        "Hello, {name}!"
    );
}

#[test]
fn warn_records_a_warning_but_leaves_the_line_alone() {
    let doc = Parser::default().parse(":attribute-missing: warn\n\nHello, {name}!");

    let warnings: Vec<_> = doc.warnings().collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0].warning,
        WarningType::SkippingReferenceToMissingAttribute("name".to_string())
    );
    assert_eq!(warnings[0].source.data(), "Hello, {name}!");
}

#[test]
fn warn_only_warns_about_truly_missing_attributes() {
    // A resolvable reference produces no warning.
    let doc = Parser::default().parse(":attribute-missing: warn\n:name: Bob\n\nHello, {name}!");
    assert!(doc.warnings().next().is_none());
    assert_eq!(
        render_first_block(":name: Bob\n\nHello, {name}!"),
        "Hello, Bob!"
    );
}

#[test]
fn drop_line_only_drops_the_offending_line() {
    // Only the line carrying the missing reference is dropped; the surrounding
    // lines (including one with a resolvable reference) are preserved.
    assert_eq!(
        render_first_block(
            ":attribute-missing: drop-line\n:name: Bob\n\nfirst {name} line\nsecond {missing} line\nthird line"
        ),
        "first Bob line\nthird line"
    );
}

#[test]
fn attribute_missing_can_be_changed_mid_document() {
    // The setting is read each time references are replaced, so it can be
    // changed partway through a document.
    let doc = Parser::default().parse("one {x}\n\n:attribute-missing: drop\n\ntwo {y}");

    let rendered: Vec<&str> = doc
        .nested_blocks()
        .filter_map(|b| b.rendered_content())
        .collect();

    assert_eq!(rendered, vec!["one {x}", "two "]);
}

non_normative!(
    r#"
.History
NOTE: AsciiDoc.py always drops the line that contains a reference to a missing attribute (effectively `attribute-missing=drop-line`).
This "`feature`" was a side effect of how the processor was implemented and not designed with the writer in mind.
The behavior is frustrating for the writer because it's hard to detect where its occurring and can result in loss of important content.
That's why Asciidoctor uses a different default behavior and, further, allows the behavior to be customized.

There are a few cases where the `attribute-missing` attribute is not strictly honored.
One of those cases is the include directive.
If a missing attribute is found in the target of an include directive, the processor will issue a warning about the missing attribute and leave behind the same warning message in the converted document.

Another case is the `ifeval` directive.
A missing attribute reference can safely be used in the clause of the `ifeval` directive without any side effects (i.e., `drop`) since the purpose of that statement is to determine whether an attribute resolves to a value.

=== Forcing failure

If you want the processor to fail when the document contains a missing attribute, set the `attribute-missing` attribute to `warn` and pass the `--failure-level=WARN` option to the CLI.

 $ asciidoctor -a attribute-missing=warn --failure-level=WARN doc.adoc

The processor will convert the entire document, but the application will complete with a non-zero exit status.

When using the API, you can consult the logger for the max severity of all messages reported or look for specific messages in the stack.
It's up to the application code to decide how and when to terminate the application.

"#
);

// IMPORTANT: The `attribute-undefined` attribute governs how an inline
// undefine expression (e.g. `{set:name!}`) is handled. The asciidoc-parser
// crate does not (and likely never will) implement inline attribute entries —
// see the "CAUTION" paragraph and the note in `inline_attribute_entries.rs`
// explaining that the core AsciiDoc team are moving to remove that syntax. With
// no undefine expressions to process, the `attribute-undefined` attribute has
// no observable effect here, so the section below is recorded as non-normative.
non_normative!(
    r#"
[#undefined]
== Undefined attribute

The `attribute-undefined` attribute controls how an expression that undefines an attribute (e.g., `+{set:name!}+`) are handled.
By default, the line containing the expression is dropped since the expression is intended to be a statement, not a content reference.

This attribute has two possible values:

`drop`:: substitute the expression with an empty string after processing it
`drop-line`:: drop the line that contains this expression (default setting; matches behavior of AsciiDoc.py)

The option `skip` doesn't make sense here since the statement is not intended to produce content.

Consider the following declaration:

[source]
----
{set:name!}
----

Depending on whether `attribute-undefined` is `drop` or `drop-line`, either the statement or the line that contains it will be discarded.
It's reasonable to stick with the compliant behavior, drop-line, in this case.

TIP: We recommend putting any statement that undefines an attribute on a line by itself.
"#
);
