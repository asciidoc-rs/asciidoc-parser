use crate::tests::prelude::{inline_file_handler::InlineFileHandler, *};

track_file!("ref/asciidoc-lang/docs/modules/directives/pages/include-uri.adoc");

const URI: &str = "https://example.org/doc.adoc";

/// Resolve a URI include with the given safe mode and (optionally) the
/// `allow-uri-read` permission, returning the rendered paragraphs. The handler
/// resolves the URI target to `content`.
fn uri_include(safe: SafeMode, allow_uri_read: bool, content: &'static str) -> Vec<String> {
    let handler = InlineFileHandler::from_pairs([(URI, content)]);
    let mut parser = Parser::default()
        .with_safe_mode(safe)
        .with_primary_file_name("main.adoc")
        .with_include_file_handler(handler);
    if allow_uri_read {
        parser =
            parser.with_intrinsic_attribute("allow-uri-read", "", ModificationContext::Anywhere);
    }
    let source = format!("include::{URI}[]");
    let doc = parser.parse(&source);
    rendered_paragraphs(&doc)
}

non_normative!(
    r#"
= Include Content by URI
//aka Include Content from a URI
:url-http: https://www.w3.org/Protocols/rfc2616/rfc2616-sec13.html
:url-uri: https://en.wikipedia.org/wiki/Uniform_Resource_Identifier

== Reference include content by URI

"#
);

#[test]
fn recognizes_uri_target() {
    verifies!(
        r#"
The include directive recognizes when the target is a URI and can include the content referenced by that URI.
This example demonstrates how to include an AsciiDoc file from a GitHub repository directly into your document.

----
include::example$include.adoc[tag=uri]
----

"#
    );

    // With the URI read permission granted (and safe mode below secure), the
    // URI target is resolved and its content is included.
    let paras = uri_include(SafeMode::Server, true, "Remote paragraph.");
    let paras: Vec<&str> = paras.iter().map(|s| s.as_str()).collect();
    assert_eq!(paras, vec!["Remote paragraph."]);
}

#[test]
fn not_enabled_by_default() {
    verifies!(
        r#"
For security reasons, this capability is *not enabled by default*.
To allow content to be read from a URI, you must enable the URI read permission by:

. running Asciidoctor in `SERVER` mode or less and
. setting the `allow-uri-read` attribute securely from the CLI or API

"#
    );

    // Without `allow-uri-read`, a URI target is not fetched even in server
    // mode; the handler is not consulted. Rather than report an unresolved
    // directive, the directive falls back to a `link:` macro to the target
    // (matching Asciidoctor at safe mode `SERVER` or below), so its content is
    // never read.
    let paras = uri_include(SafeMode::Server, false, "SHOULD NOT APPEAR");
    assert_eq!(paras.len(), 1);
    assert!(
        !paras[0].contains("SHOULD NOT APPEAR"),
        "URI content was fetched: {paras:?}"
    );
    assert!(
        paras[0].contains(&format!("href=\"{URI}\"")) && paras[0].contains("include"),
        "expected a link fallback, was: {paras:?}"
    );
}

non_normative!(
    r#"
Here's an example that shows how to run Asciidoctor from the console so it can read content from a URI:

 $ asciidoctor -a allow-uri-read filename.adoc

Remember that Asciidoctor executes in `UNSAFE` mode by default when run from the command line.

Here's an example that shows how to run Asciidoctor from the API so it can read content from a URI:

[source,ruby]
----
Asciidoctor.convert_file 'filename.adoc', safe: :safe, attributes: { 'allow-uri-read' => '' }
----

"#
);

#[test]
fn forcefully_disabled_in_secure_mode() {
    verifies!(
        r#"
WARNING: Including content from sources outside your control carries certain risks, including the potential to introduce malicious behavior into your documentation.
Because `allow-uri-read` is a potentially dangerous feature, it is forcefully disabled when the safe mode is `SECURE` or higher.

"#
    );

    // In secure mode the include is converted to a link even when
    // `allow-uri-read` is set, so the URI content is never read.
    let paras = uri_include(SafeMode::Secure, true, "SECRET CONTENT");
    assert!(
        paras.iter().all(|p| !p.contains("SECRET CONTENT")),
        "URI content leaked in secure mode: {paras:?}"
    );
}

non_normative!(
    r#"
.URI vs URL
****
URI stands for {url-uri}[Uniform Resource Identifier^].
When we talk about a URI, we're usually talking about a URL, or Uniform Resource Locator.
A URL is simply a URI that points to a resource over a network, or web address.

As far as Asciidoctor is concerned, all URIs share the same restriction, whether or not it's actually local or remote, or whether it points to a web address (http or https prefix), FTP address (ftp prefix), or some other addressing scheme.
****

The same restriction described in this section applies when embedding an image referenced from a URI, such as when `data-uri` is set or when converting to PDF using Asciidoctor PDF.

=== Caching URI content

Reading content from a URI is obviously much slower than reading it from a local file.

Asciidoctor provides a way for the content read from a URI to be cached, which is highly recommended.

To enable the built-in cache, you must:

. Install the `open-uri-cached` gem.
. Set the `cache-uri` attribute in the document.

When these two conditions are satisfied, Asciidoctor caches content read from a URI according the to {url-http}[HTTP caching recommendations^].
"#
);

// A remote target containing a space is wrapped in `pass:c[…]` by the
// preprocessor fallback (see `reader_test.rs`); this confirms that wrapper is
// extracted correctly downstream, so the rendered link carries the full URI as
// its `href` (spaces and all) rather than leaving `pass:c` as the destination.
#[test]
fn space_in_remote_target_renders_full_uri() {
    const SPACE_URI: &str = "https://example.org/no such file.adoc";

    let mut parser = Parser::default()
        .with_safe_mode(SafeMode::Safe)
        .with_primary_file_name("main.adoc");

    let source = format!("include::{SPACE_URI}[]");
    let doc = parser.parse(&source);
    let paras = rendered_paragraphs(&doc);

    assert_eq!(
        paras,
        vec![format!(
            "<a href=\"{SPACE_URI}\" class=\"bare include\">{SPACE_URI}</a>"
        )]
    );
}
