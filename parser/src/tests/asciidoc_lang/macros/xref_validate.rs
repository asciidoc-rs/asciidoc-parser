use crate::{
    parser::{CatalogResolver, HtmlSubstitutionRenderer},
    tests::prelude::*,
};

track_file!("ref/asciidoc-lang/docs/modules/macros/pages/xref-validate.adoc");

non_normative!(
    r##"
= Validate Cross References

An AsciiDoc processor is only required to provide limited support for validating internal cross references.
Validation occurs when a cross reference is first visited.
Since there are still some references aren't stored in the parse tree (such as an anchor in the middle of a paragraph), which can lead to false positives, these validations are hidden behind a flag.

When using Asciidoctor, you can enable validation of cross references in several ways:

* when using the CLI, passing the `-v` CLI option
* when using the API, setting the global variable `$VERBOSE` to the value `true`
* when using the API, setting the level on the global logger to INFO (i.e., `Asciidoctor::LoggerManager.logger.level = :info`)

"##
);

#[test]
fn reports_an_invalid_reference() {
    verifies!(
        r##"
All of these adjustments put the processor into pedantic mode.
In this mode, the parser will immediately validate cross references, issuing a warning message if the reference is not valid.
If you set the global variable `$VERBOSE` to `true`, it will also enable warnings in Ruby, which may not be what you want.

Consider the following example:

----
See <<foobar>>.

[#foobaz]
== Foobaz
----

"##
    );

    // In pedantic (verbose) mode the resolution pass reports a reference it
    // cannot resolve within the same document. This crate surfaces that as a
    // `ReferenceWarning` returned from `resolve_references`.
    let mut doc = Parser::default().parse_deferred("See <<foobar>>.\n\n[#foobaz]\n== Foobaz\n");

    let catalog = doc.catalog().clone();
    let resolver = CatalogResolver::new(&catalog);
    let warnings = doc.resolve_references(&resolver, &HtmlSubstitutionRenderer {});

    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].target, "foobar");
}

non_normative!(
    r##"
If you run Asciidoctor in verbose/pedantic mode on this document (`-v`), it will send the following warning message to the logger.

....
asciidoctor: WARNING: invalid reference: foobar
....

An AsciiDoc processor is only required to validate references within the same document (after any includes are resolved).
"##
);
