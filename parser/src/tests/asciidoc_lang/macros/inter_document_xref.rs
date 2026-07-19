use crate::tests::prelude::*;

track_file!("ref/asciidoc-lang/docs/modules/macros/pages/inter-document-xref.adoc");

non_normative!(
    r##"
= Document to Document Cross References

The inline xref macro can also link to IDs in other AsciiDoc documents.
This eliminates the need to use direct links between documents that are coupled to a particular converter (e.g., HTML links).
It also captures the intent of the author to establish a reference to a section in another document.

Here's how a cross reference is normally defined in AsciiDoc:

[source]
----
include::example$xref.adoc[tag=base]
----

This cross reference creates a link to the section with the ID _anchors_.

Let's assume the cross reference is defined in the document [.path]_document-a.adoc_.
If the target section is in a separate document, [.path]_document-b.adoc_, the author may be tempted to write:

[source]
----
include::example$xref.adoc[tag=bad]
----

However, this link is coupled to HTML output.
What's worse, if [.path]_document-b.adoc_ is included in the same document as [.path]_document-a.adoc_, the link will refer to a document that doesn't even exist!

These problems can be alleviated by using an inter-document xref:

[source]
----
include::example$xref.adoc[tag=base-inter]
----

The ID of the target is now placed behind a hash symbol (`#`).
Preceding the hash is the name of the reference document (the file extension is optional).
We've also added link text since an AsciiDoc processor is not (yet) required to resolve the section title in a separate document.

TIP: While the link text for a local (i.e., intradocument) cross reference is optional, the link text for an interdocument cross reference is (currently) required.

"##
);

#[test]
fn generates_a_link_to_the_output_file() {
    verifies!(
        r##"
When the AsciiDoc processor generates the link for this cross reference, it first checks to see if [.path]_document-b.adoc_ is included in the same document as [.path]_document-a.doc_ (by comparing the xref target to the include targets relative to the outermost document).
If not, it will generate a link to [.path]_document-b.html_, intelligently substituting the original file extension with the file extension of the output file.

[source,html]
----
<a href="document-b.html#section-b">Section B</a>
----

"##
    );

    // The AsciiDoc file extension of the target is replaced by the file
    // extension of the output file (`outfilesuffix`, `.html` by default).
    let doc = Parser::default()
        .parse("Refer to xref:document-b.adoc#section-b[Section B] for more information.");

    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"Refer to <a href="document-b.html#section-b">Section B</a> for more information."##
    );
}

// This crate parses one document at a time and does not track which files were
// included into it, so a cross reference to an included document is not
// collapsed to a same-document reference. Tracked in #773.
non_normative!(
    r##"
If [.path]_document-b.adoc_ is included in the same document as [.path]_document-a.doc_, then the document will be dropped in the link target and look like the output of a normal cross reference:

[source,html]
----
<a href="#section-b">Section B</a>
----

Now you can create inter-document cross references without the headache.

"##
);

non_normative!(
    r##"
== Navigating between source files

In certain environments, such as a web interface for a source repository or an editor preview, you might see the generated HTML when you visit the URL of the AsciiDoc source file.
If not accounted for, this has consequences for inter-document cross references.

Since the default suffix for inter-document cross references in the `html5` backend is `.html`, the resulting link created in these environments may end up pointing to non-existent HTML files.
In this case, you need to change the inter-document cross references to refer to other AsciiDoc source files instead.

"##
);

#[test]
fn relfilesuffix_controls_the_file_extension() {
    verifies!(
        r##"
The file extension chosen for inter-document cross references is controlled by the `relfilesuffix` attribute.
By default, this attribute is not set and the value of the `outfilesuffix` is used instead.
If you want to change the file extension that gets used, you can do so by setting the `relfilesuffix` attribute.

The following example demonstrates how to use the `relfilesuffix` attribute to control the file extension for inter-document cross references when you want to create a source-to-source reference.
The assignment is hidden behind a check for `env-name`, where `env-name` is an attribute that is only set in an environment where you need to make this type of reference.

[source]
----
= Document Title
\ifdef::env-name[:relfilesuffix: .adoc]

See the xref:README.adoc[README].

We could also write the link as link:README{relfilesuffix}[README].
----

The links in the generated document will now point to [.path]_README.adoc_ instead of [.path]_README.html_.

"##
    );

    // Without `relfilesuffix`, the reference takes the value of
    // `outfilesuffix`.
    let doc = Parser::default().parse("= Document Title\n\nSee the xref:README.adoc[README].");

    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"See the <a href="README.html">README</a>."##
    );

    // Setting `relfilesuffix` overrides it, here making the reference point at
    // the AsciiDoc source file instead.
    let doc = Parser::default()
        .with_intrinsic_attribute("env-name", "", ModificationContext::Anywhere)
        .parse(
            "= Document Title\nifdef::env-name[:relfilesuffix: .adoc]\n\nSee the xref:README.adoc[README].\n\nWe could also write the link as link:README{relfilesuffix}[README].",
        );

    assert_eq!(
        rendered_paragraphs(&doc),
        vec![
            r##"See the <a href="README.adoc">README</a>."##,
            r##"We could also write the link as <a href="README.adoc">README</a>."##,
        ]
    );
}

non_normative!(
    r##"
TIP: This configuration is not actually necessary on GitHub, GitLab, or the browser preview extension since those environments automatically set the value of `relfilesuffix` to match the file extension of the source file.
However, this setting may still be required for other environments, so it's worth knowing.

"##
);

#[test]
fn relfileprefix_is_prepended_to_the_path() {
    verifies!(
        r##"
== Mapping references to a different structure

While `relfilesuffix` gives you control over the end of the resolved path for an inter-document cross reference, the `relfileprefix` attribute gives you control over the beginning of the path.
When resolving the path of an inter-document cross reference, if the `relfileprefix` attribute is set, the value of this attribute gets prepended to the path.
Let's look at an example of when these two attributes are used together.

A common practice in website architecture is to move files into their own folder to make the path format agnostic (called "`indexify`").
For example, the path [.path]_filename.html_ becomes [.path]_filename_ (which targets [.path]_filename/index.html_).
However, this is problematic for inter-document cross references.
Any cross reference that resolves to the path [.path]_filename.html_ is now invalid since the file has moved to a subfolder (and thus no longer a sibling of the referencing document).

To solve this problem, you can define the following two attributes:

[source]
----
:relfileprefix: ../
:relfilesuffix: /
----

Now, the cross reference `+<<filename.adoc,link text>>+` will resolve to [.path]_../filename_ instead of [.path]_filename.html_.
Since this change is specific to the website architecture described, you want to be sure to only set these attributes in that particular environment (either using an ifdef directive or via the API).
"##
    );

    let doc = Parser::default()
        .parse(":relfileprefix: ../\n:relfilesuffix: /\n\n<<filename.adoc#,link text>>");

    assert_eq!(
        rendered_paragraphs(&doc)[0],
        r##"<a href="../filename/">link text</a>"##
    );
}
