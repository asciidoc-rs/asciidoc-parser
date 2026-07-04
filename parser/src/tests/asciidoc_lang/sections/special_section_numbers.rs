use crate::tests::prelude::*;

track_file!("ref/asciidoc-lang/docs/modules/sections/pages/special-section-numbers.adoc");

// Treating the entire file as non-normative because we don't support `doctype`.

non_normative!(
    r#"
= Number Special Sections

Sections that are assigned a built-in special style aren't numbered by default.
To number regular sections as well as special sections, set `sectnums` and assign it a value of `all`.

[source]
----
= Title
:sectnums: all
----

The assignment of appendix numbers isn't affected by `sectnums` as their section title prefixes are controlled by the attribute xref:appendix.adoc#caption[appendix-caption].
Book parts aren't numbered by `sectnums` either, instead, they're xref:part-numbers-and-labels.adoc[controlled by `partnums`].
"#
);
