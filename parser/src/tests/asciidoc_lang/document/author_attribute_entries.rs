use crate::tests::prelude::*;

track_file!("ref/asciidoc-lang/docs/modules/document/pages/author-attribute-entries.adoc");

// Treating the entire file as non-normative because asciidoc-parser doesn't
// _consume_ the author attribute entries anywhere. There's nothing special
// about this syntax that we need to verify.
non_normative!(
    r#"
= Assign Author and Email with Attribute Entries

Instead of using an author line, a single author's information can be set and assigned with attribute entries in the document header.

== author and email attribute syntax

The built-in attributes `author` and `email` can be explicitly set and assigned values in the document header using attribute entries.

.Set author and email attributes
[source#ex-entries]
----
= The Intrepid Chronicles
:author: Kismet R. Lee <.>
:email: kismet@asciidoctor.org <.>
----
<.> The author's name is assigned to the built-in attribute `author`
<.> The author's email is assigned to the built-in attribute `email`

When the default stylesheet is applied, the author information assigned to these attributes is displayed on the byline.
The result of <<ex-entries>> is displayed below.

image::author-and-email-attributes.png["Byline containing author information from the explicitly set author and email attributes",role=screenshot]

NOTE: You can't set the built-in attributes for multiple authors (e.g., `author_2`, `email_3`) using attribute entries.
xref:multiple-authors.adoc[Multiple authors] can only be set using the author line.

These attributes can also be xref:reference-author-attributes.adoc[referenced in the document].
"#
);
