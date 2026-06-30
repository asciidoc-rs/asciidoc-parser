//! Verifies the docinfo specification page, line by line.

#![allow(clippy::unwrap_used)]

use std::{cell::RefCell, rc::Rc};

use crate::{document::DocinfoLocation, parser::DocinfoFileHandler, tests::prelude::*};

track_file!("docs/modules/docinfo/pages/index.adoc");

/// A log of the docinfo files the parser requested: each entry is the
/// `docinfodir` (if any) and the requested file name.
type DocinfoRequests = Rc<RefCell<Vec<(Option<String>, String)>>>;

/// A [`DocinfoFileHandler`] backed by a fixed map of file name to content. It
/// records each request so tests can assert what the parser asked for.
#[derive(Clone, Debug)]
struct TestDocinfo {
    files: Rc<HashMap<String, String>>,
    requests: DocinfoRequests,
}

impl TestDocinfo {
    fn new(files: &[(&str, &str)]) -> Self {
        Self {
            files: Rc::new(
                files
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            ),
            requests: Rc::new(RefCell::new(vec![])),
        }
    }
}

impl DocinfoFileHandler for TestDocinfo {
    fn resolve_docinfo(
        &self,
        docinfodir: Option<&str>,
        file_name: &str,
        _parser: &Parser,
    ) -> Option<String> {
        self.requests
            .borrow_mut()
            .push((docinfodir.map(|s| s.to_string()), file_name.to_string()));
        self.files.get(file_name).cloned()
    }
}

/// A handler stocked with all six (shared and private) docinfo files for a
/// document named `mydoc`.
fn all_six() -> TestDocinfo {
    TestDocinfo::new(&[
        ("docinfo.html", "S-HEAD"),
        ("docinfo-header.html", "S-HEADER"),
        ("docinfo-footer.html", "S-FOOTER"),
        ("mydoc-docinfo.html", "P-HEAD"),
        ("mydoc-docinfo-header.html", "P-HEADER"),
        ("mydoc-docinfo-footer.html", "P-FOOTER"),
    ])
}

/// Parses a minimal `mydoc.adoc` carrying the given header line, with a handler
/// stocked with every docinfo file.
fn parse_docinfo(header_line: &str) -> crate::Document<'static> {
    let src = format!("= Doc\n{header_line}\n\nBody.");
    Parser::default()
        .with_primary_file_name("mydoc.adoc")
        .with_docinfo_file_handler(all_six())
        .parse(&src)
}

non_normative!(
    r#"
= Docinfo Files
:url-docbook-info-ref: https://tdg.docbook.org/tdg/5.0/info.html
:url-docinfo-example: https://github.com/clojure-cookbook/clojure-cookbook/blob/master/book-docinfo.xml

Docinfo is a feature of AsciiDoc that allows you to insert custom content into the head, header, or footer of the output document.
This custom content is read from files known as docinfo files by the converter.
Docinfo files are intended as convenient way to supplement the output produced by a converter.
Examples include injecting auxiliary metadata, stylesheets, and scripting logic not already provided by the converter.

The docinfo feature does not apply to all backends.
While it works when converting to output formats such as HTML and DocBook, it does not work when converting to PDF using Asciidoctor PDF.

The docinfo feature must be explicitly enabled using the `docinfo` attribute (see <<enable>>).
Which docinfo files are consumed depends on the value of the `docinfo` attribute as well as the backend.

"#
);

non_normative!(
    r#"
[#head]
== Head docinfo files

The content of head docinfo files gets injected into the top of the document.
For HTML, the content is append to the `<head>` element.
For DocBook, the content is appended to the root `<info>` element.

The docinfo file for HTML output may contain valid elements to populate the HTML `<head>` element, including:

* `<base>`
* `<link>`
* `<meta>`
* `<noscript>`
* `<script>`
* `<style>`

CAUTION: Use of the `<title>` element is not recommended as it's already emitted by the converter.

You do not need to include the enclosing `<head>` element as it's assumed to be the envelope.

Here's an example:

.A head docinfo file for HTML output
[source,html]
----
<meta name="keywords" content="open source, documentation">
<meta name="description" content="The dangerous and thrilling adventures of an open source documentation team.">
<link rel="stylesheet" href="basejump.css">
<script src="map.js"></script>
----

Docinfo files for HTML output must be saved with the `.html` file extension.
See <<naming>> for more details.

The docinfo file for DocBook 5.0 output may include any of the {url-docbook-info-ref}[<info> element's children^], such as:

* `<address>`
* `<copyright>`
* `<edition>`
* `<keywordset>`
* `<publisher>`
* `<subtitle>`
* `<revhistory>`

The following example shows some of the content a docinfo file for DocBook might contain.

.A docinfo file for DocBook 5.0 output
[source,xml]
----
<author>
  <personname>
    <firstname>Karma</firstname>
    <surname>Chameleon</surname>
  </personname>
  <affiliation>
    <jobtitle>Word SWAT Team Leader</jobtitle>
  </affiliation>
</author>

<keywordset>
  <keyword>open source</keyword>
  <keyword>documentation</keyword>
  <keyword>adventure</keyword>
</keywordset>

<printhistory>
  <para>April, 2021. Twenty-sixth printing.</para>
</printhistory>
----

Docinfo files for DocBook output must be saved with the `.xml` file extension.
See <<naming>> for more details.

You can find a real-world example of a docinfo file for DocBook in the source of the {url-docinfo-example}[Clojure Cookbook^].

[#header]
== Header docinfo files

Header docinfo files are differentiated from head docinfo files by the addition of `-header` to the file name.
In the HTML output, the header content is inserted immediately before the header div (i.e., `<div id="header">`).
In the DocBook output, the header content is inserted immediately after the opening tag (e.g., `<article>` or `<book>`).

TIP: One possible use of the header docinfo file is to completely replace the default header in the standard stylesheet.
Just set the attribute `noheader`, then apply a custom header docinfo file.

[#footer]
== Footer docinfo files

Footer docinfo files are differentiated from head docinfo files by the addition of `-footer` to the file name.
In the HTML output, the footer content is inserted immediately after the footer div (i.e., `<div id="footer">`).
In the DocBook output, the footer content is inserted immediately before the ending tag (e.g., `</article>` or `</book>`).

TIP: One possible use of the footer docinfo file is to completely replace the default footer in the standard stylesheet.
Just set the attribute `nofooter`, then apply a custom footer docinfo file.

// Not here! Good info, but does nothing to clarify the previous paragraphs and could confuse.
////
TIP: To change the text in the "Last updated" line in the footer, set the text in the attribute `last-update-label` (for example, `:last-update-label: <your text> Last Updated`). +
To disable the "Last updated" line in the footer, unassign the attribute `last-update-label` (however, this leaves an empty footer div). +
To disable the footer completely, set the attribute `nofooter`. Then having a footer docinfo file effectively replaces the default footer with your custom footer.
////

"#
);

#[test]
fn naming_docinfo_files() {
    verifies!(
        r#"
[#naming]
== Naming docinfo files

The file that gets selected to provide the docinfo depends on which converter is in use (html, docbook, etc) and whether the docinfo scope is configured for a specific document ("`private`") or for all documents in the same directory ("`shared`").
The file extension of the docinfo file must match the file extension of the output file (as specified by the `outfilesuffix` attribute, a value which always begins with a period (`.`)).

.Docinfo file naming
[cols="<10,<20,<30,<30"]
|===
|Mode |Location |Behavior |Docinfo file name

.3+|Private
|Head
|Adds content to `<head>`/`<info>` for <docname>.adoc files.
|`<docname>-docinfo<outfilesuffix>`

|Header
|Adds content to start of document for <docname>.adoc files.
|`<docname>-docinfo-header<outfilesuffix>`

|Footer
|Adds content to end of document for <docname>.adoc files.
|`<docname>-docinfo-footer<outfilesuffix>`

.3+|Shared
|Head
|Adds content to `<head>`/`<info>` for any document in same directory.
|`docinfo<outfilesuffix>`

|Header
|Adds content to start of document for any document in same directory.
|`docinfo-header<outfilesuffix>`

|Footer
|Adds content to end of document for any document in same directory.
|`docinfo-footer<outfilesuffix>`
|===

"#
    );

    let handler = TestDocinfo::new(&[
        ("docinfo.html", "SHARED-HEAD"),
        ("docinfo-header.html", "SHARED-HEADER"),
        ("docinfo-footer.html", "SHARED-FOOTER"),
        ("mydoc-docinfo.html", "PRIVATE-HEAD"),
        ("mydoc-docinfo-header.html", "PRIVATE-HEADER"),
        ("mydoc-docinfo-footer.html", "PRIVATE-FOOTER"),
    ]);
    let doc = Parser::default()
        .with_primary_file_name("mydoc.adoc")
        .with_docinfo_file_handler(handler)
        .parse("= Doc\n:docinfo: shared,private\n\nBody.");

    // Each location draws from its own private and shared file names, with the
    // shared content concatenated before the private content.
    assert_eq!(
        doc.docinfo(DocinfoLocation::Head),
        "SHARED-HEAD\nPRIVATE-HEAD"
    );
    assert_eq!(
        doc.docinfo(DocinfoLocation::Header),
        "SHARED-HEADER\nPRIVATE-HEADER"
    );
    assert_eq!(
        doc.docinfo(DocinfoLocation::Footer),
        "SHARED-FOOTER\nPRIVATE-FOOTER"
    );

    // The docinfo file extension follows `outfilesuffix`.
    let xml = TestDocinfo::new(&[("docinfo.xml", "XML-HEAD")]);
    let doc = Parser::default()
        .with_primary_file_name("mydoc.adoc")
        .with_docinfo_file_handler(xml)
        .parse("= Doc\n:docinfo: shared-head\n:outfilesuffix: .xml\n\nBody.");
    assert_eq!(doc.docinfo(DocinfoLocation::Head), "XML-HEAD");

    // A private docinfo file is named for the document; a file named for a
    // different document is not used.
    let other = TestDocinfo::new(&[("otherdoc-docinfo.html", "NOPE")]);
    let doc = Parser::default()
        .with_primary_file_name("mydoc.adoc")
        .with_docinfo_file_handler(other)
        .parse("= Doc\n:docinfo: private-head\n\nBody.");
    assert_eq!(doc.docinfo(DocinfoLocation::Head), "");
}

#[test]
fn enabling_docinfo() {
    verifies!(
        r#"
[#enable]
== Enabling docinfo

To specify which file(s) you want to apply, set the `docinfo` attribute to any combination of these values:

* `private-head`
* `private-header`
* `private-footer`
* `private` (alias for `private-head,private-header,private-footer`)
* `shared-head`
* `shared-header`
* `shared-footer`
* `shared` (alias for `shared-head,shared-header,shared-footer`)

Setting `docinfo` with no value is equivalent to setting the value to `private`.

For example:

[source,asciidoc]
----
:docinfo: shared,private-footer
----

This docinfo configuration will apply the shared docinfo head, header and footer files, if they exist, as well as the private footer file, if it exists.

Let's apply this to an example:

You have two AsciiDoc documents, [.path]_adventure.adoc_ and [.path]_insurance.adoc_, saved in the same folder.
You want to add the same content to the head of both documents when they're converted to HTML.

. Create a docinfo file containing `<head>` elements.
. Save it as docinfo.html.
. Set the `docinfo` attribute in [.path]_adventure.adoc_ and [.path]_insurance.adoc_ to `shared`.

You also want to include some additional content, but only to the head of [.path]_adventure.adoc_.

. Create *another* docinfo file containing `<head>` elements.
. Save it as [.path]_adventure-docinfo.html_.
. Set the `docinfo` attribute in [.path]_adventure.adoc_ to `shared,private-head`

If other AsciiDoc files are added to the same folder, and `docinfo` is set to `shared` in those files, only the [.path]_docinfo.html_ file will be added when converting those files.

"#
    );

    // Each scope/location value selects exactly the matching file.
    let d = parse_docinfo(":docinfo: shared-head");
    assert_eq!(d.docinfo(DocinfoLocation::Head), "S-HEAD");
    assert_eq!(d.docinfo(DocinfoLocation::Header), "");
    assert_eq!(d.docinfo(DocinfoLocation::Footer), "");

    let d = parse_docinfo(":docinfo: private-header");
    assert_eq!(d.docinfo(DocinfoLocation::Header), "P-HEADER");
    assert_eq!(d.docinfo(DocinfoLocation::Head), "");

    let d = parse_docinfo(":docinfo: private-footer");
    assert_eq!(d.docinfo(DocinfoLocation::Footer), "P-FOOTER");

    // `private` is an alias for `private-head,private-header,private-footer`.
    let d = parse_docinfo(":docinfo: private");
    assert_eq!(d.docinfo(DocinfoLocation::Head), "P-HEAD");
    assert_eq!(d.docinfo(DocinfoLocation::Header), "P-HEADER");
    assert_eq!(d.docinfo(DocinfoLocation::Footer), "P-FOOTER");

    // `shared` is an alias for `shared-head,shared-header,shared-footer`.
    let d = parse_docinfo(":docinfo: shared");
    assert_eq!(d.docinfo(DocinfoLocation::Head), "S-HEAD");
    assert_eq!(d.docinfo(DocinfoLocation::Header), "S-HEADER");
    assert_eq!(d.docinfo(DocinfoLocation::Footer), "S-FOOTER");

    // Setting `docinfo` with no value is equivalent to setting it to `private`.
    let d = parse_docinfo(":docinfo:");
    assert_eq!(d.docinfo(DocinfoLocation::Head), "P-HEAD");
    assert_eq!(d.docinfo(DocinfoLocation::Header), "P-HEADER");
    assert_eq!(d.docinfo(DocinfoLocation::Footer), "P-FOOTER");

    // The combined example from the page: the shared head, header, and footer
    // files plus the private footer file.
    let d = parse_docinfo(":docinfo: shared,private-footer");
    assert_eq!(d.docinfo(DocinfoLocation::Head), "S-HEAD");
    assert_eq!(d.docinfo(DocinfoLocation::Header), "S-HEADER");
    assert_eq!(d.docinfo(DocinfoLocation::Footer), "S-FOOTER\nP-FOOTER");

    // With no `docinfo` attribute, no docinfo is applied.
    let d = Parser::default()
        .with_primary_file_name("mydoc.adoc")
        .with_docinfo_file_handler(all_six())
        .parse("= Doc\n\nBody.");
    assert_eq!(d.docinfo(DocinfoLocation::Head), "");

    // With no docinfo file handler, no docinfo is resolved even when enabled.
    let d = Parser::default()
        .with_primary_file_name("mydoc.adoc")
        .parse("= Doc\n:docinfo: shared\n\nBody.");
    assert_eq!(d.docinfo(DocinfoLocation::Head), "");
}

#[test]
fn locating_docinfo_files() {
    verifies!(
        r#"
[#resolving]
== Locating docinfo files

By default, docinfo files are searched for in the same directory as the document file (which can be overridden by setting the `:base_dir` API option / `--base-dir` CLI option).
If you want to load them from another location, set the `docinfodir` attribute to the directory where the files are located.
If the value of the `docinfodir` attribute is a relative path, that value is appended to the document directory.
If the value is an absolute path, that value is used as is.

[source,asciidoc]
----
:docinfodir: common/meta
----

Note that if you use this attribute, only the specified folder will be searched; docinfo files in the document directory will no longer be found.

"#
    );

    // By default no `docinfodir` is set, so the handler is asked with `None`
    // (it resolves relative to the document directory).
    let handler = TestDocinfo::new(&[("docinfo.html", "HEAD")]);
    let doc = Parser::default()
        .with_primary_file_name("mydoc.adoc")
        .with_docinfo_file_handler(handler.clone())
        .parse("= Doc\n:docinfo: shared-head\n\nBody.");
    assert_eq!(doc.docinfo(DocinfoLocation::Head), "HEAD");
    assert_eq!(
        *handler.requests.borrow(),
        vec![(None, "docinfo.html".to_string())]
    );

    // Setting `docinfodir` passes the directory through to the handler, which
    // is then responsible for searching only that folder.
    let handler = TestDocinfo::new(&[("docinfo.html", "HEAD")]);
    let doc = Parser::default()
        .with_primary_file_name("mydoc.adoc")
        .with_docinfo_file_handler(handler.clone())
        .parse("= Doc\n:docinfo: shared-head\n:docinfodir: common/meta\n\nBody.");
    assert_eq!(doc.docinfo(DocinfoLocation::Head), "HEAD");
    assert_eq!(
        *handler.requests.borrow(),
        vec![(Some("common/meta".to_string()), "docinfo.html".to_string())]
    );
}

#[test]
fn attribute_substitution_in_docinfo_files() {
    verifies!(
        r#"
[#attribute-substitution]
== Attribute substitution in docinfo files

Docinfo files may include attribute references.
Which substitutions get applied is controlled by the `docinfosubs` attribute, which takes a comma-separated list of substitution names.
If this attribute is not set, it has an implied default value of `attributes` (i.e., attribute references are resolved).

For example, if you created the following docinfo file:

.Docinfo file (docinfo.xml) containing a revnumber attribute reference
[source,xml]
----
<edition>{revnumber}</edition>
----

And this source document:

.Source document including a revision number
[,asciidoc]
----
= Document Title
Author Name
v1.0, 2020-12-30
:doctype: book
:backend: docbook
:docinfo: shared
----

Then the converted DocBook output would be:

.Converted DocBook containing the docinfo
[,xml]
----
<?xml version="1.0" encoding="UTF-8"?>
<book xmlns="http://docbook.org/ns/docbook" xmlns:xl="http://www.w3.org/1999/xlink" version="5.0" xml:lang="en">
<info>
<title>Document Title</title>
<date>2020-12-30</date>
<author>
<personname>
<firstname>Author</firstname>
<surname>Name</surname>
</personname>
</author>
<authorinitials>AN</authorinitials>
<revhistory>
<revision>
<revnumber>1.0</revnumber>
<date>2020-12-30</date>
<authorinitials>AN</authorinitials>
</revision>
</revhistory>
<edition>1.0</edition> <!--.-->
</info>
</book>
----
<.> The revnumber attribute reference in `docinfo.xml` was replaced by the source document's revision number in the converted output.

Another example is if you want to define the license link tag in the HTML head.

.Docinfo file (docinfo.html) containing a license meta tag
[,html]
----
<link rel="license" href="{license-url}" title="{license-title}">
----

Now define these attributes in your AsciiDoc source:

.Source document that defines license attributes
[,asciidoc]
----
= Document Title
:license-url: https://mit-license.org
:license-title: MIT
:docinfo: shared
----

Then the `<head>` tag in the converted HTML would include:

.Rendered license link tag in HTML output
[,html]
----
<link rel="license" href="https://mit-license.org" title="MIT">
----
"#
    );

    // Attribute references in docinfo content are resolved by default
    // (`docinfosubs` defaults to `attributes`).
    let handler = TestDocinfo::new(&[("docinfo.html", "<edition>{revnumber}</edition>")]);
    let doc = Parser::default()
        .with_primary_file_name("mydoc.adoc")
        .with_docinfo_file_handler(handler)
        .parse("= Doc\n:revnumber: 1.0\n:docinfo: shared-head\n\nBody.");
    assert_eq!(doc.docinfo(DocinfoLocation::Head), "<edition>1.0</edition>");

    // The license link example from the page.
    let handler = TestDocinfo::new(&[(
        "docinfo.html",
        "<link rel=\"license\" href=\"{license-url}\" title=\"{license-title}\">",
    )]);
    let doc = Parser::default()
        .with_primary_file_name("mydoc.adoc")
        .with_docinfo_file_handler(handler)
        .parse(
            "= Doc\n:license-url: https://mit-license.org\n:license-title: MIT\n:docinfo: shared-head\n\nBody.",
        );
    assert_eq!(
        doc.docinfo(DocinfoLocation::Head),
        "<link rel=\"license\" href=\"https://mit-license.org\" title=\"MIT\">"
    );

    // `docinfosubs` controls which substitutions apply: a list that omits
    // `attributes` leaves attribute references untouched.
    let handler = TestDocinfo::new(&[("docinfo.html", "{license-url}")]);
    let doc = Parser::default()
        .with_primary_file_name("mydoc.adoc")
        .with_docinfo_file_handler(handler)
        .parse(
            "= Doc\n:license-url: https://mit-license.org\n:docinfo: shared-head\n:docinfosubs: specialchars\n\nBody.",
        );
    assert_eq!(doc.docinfo(DocinfoLocation::Head), "{license-url}");
}
