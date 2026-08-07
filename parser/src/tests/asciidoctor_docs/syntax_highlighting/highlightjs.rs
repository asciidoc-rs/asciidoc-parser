use crate::{document::InterpretedValue, tests::prelude::*};

track_file!("ref/asciidoctor/docs/modules/syntax-highlighting/pages/highlightjs.adoc");

// Highlight.js is a client-side syntax highlighter. asciidoc-parser performs no
// rendering, so the activation, CDN-loading, language-bundle, and
// custom-library mechanics on this page are non-normative here. The one
// exception is the documented default value of the `highlightjs-theme` document
// attribute, which the parser knows as a built-in default value (see
// `built_in_attrs.rs`); that is verified below.
non_normative!(
    r##"
= Highlight.js
:url-highlightjs: https://highlightjs.org/
:url-highlightjs-lang: https://highlightjs.org/download/
:url-highlightjs-cdn: https://cdnjs.com/libraries/highlight.js

{url-highlightjs}[Highlight.js^] is a popular client-side syntax highlighter that supports a broad range of {url-highlightjs-lang}[languages^].

== Activate highlight.js

To activate highlight.js, add the following attribute entry to the header of your AsciiDoc file:

[,asciidoc]
----
:source-highlighter: highlight.js
----

By default, Asciidoctor will link to the highlight.js library and stylesheet hosted on {url-highlightjs-cdn}[cdnjs^].
The version of the highlight.js library Asciidoctor loads from the CDN only includes support for languages in the common language bundle (apache, bash, coffeescript, cpp, cs, css, diff, http, ini, java, javascript, json, makefile, markdown, nginx, objectivec, perl, php, properties, python, ruby, shell, sql, xml, and yaml).

== Change the theme

The theme controls the colors that are used for the tokens (keywords, strings, methods, etc.) in the highlighted code.
"##
);

#[test]
fn highlightjs_theme_defaults_to_github() {
    verifies!(
        r##"
By default, highlight.js is configured to use the github theme.
"##
    );

    // `github` is the parser's built-in default value for `highlightjs-theme`:
    // setting the attribute with an empty value (a bare `:highlightjs-theme:`)
    // resolves to `github`. The parser does no rendering, so this is the extent
    // to which it models the documented default theme.
    let doc = Parser::default().parse(":highlightjs-theme:\n");

    assert_eq!(
        doc.attribute_value("highlightjs-theme"),
        InterpretedValue::Value("github".to_string()),
    );
}

non_normative!(
    r##"
You can change the theme used by highlight.js by setting the `highlightjs-theme` attribute.

[,asciidoc]
----
:source-highlighter: highlight.js
:highlightjs-theme: monokai
----

The theme is loaded from the CDN, so any theme supported by the version of highlight.js that Asciidoctor uses is supported.
Refer to https://cdnjs.com/libraries/highlight.js/9.18.3 for a list of themes (filter by *Asset Type: Styling*).
The value of the `highlightjs-theme` attribute is the basename of the file minus the _.min.css_ file extension.

== Load support for additional languages

To load additional languages supported by highlight.js, list them in the value of the `highlightjs-languages` document attribute.
Separate each language by a comma followed by an optional space.

The common highlight.js bundle does not include support for Rust and Swift.
Let's set the `highlightjs-languages` attribute so the HTML converter loads support for them into the HTML page.

[,asciidoc]
----
:source-highlighter: highlight.js
:highlightjs-languages: rust, swift
----

The `highlightjs-languages` attribute only applies when generating a standalone HTML document (i.e., backend: html, standalone: true).
It does not work when generating embedded HTML, which is used by site generator integrations such as Antora.

== Use a custom highlight.js library

If you'd rather use a personal copy of highlight.js instead of the one hosted on the CDN, follow these steps:

. Create your custom bundle on the {url-highlightjs-lang}[download page^].
. Download and unpack the zip into a folder called [.path]_highlight_ adjacent to your AsciiDoc file (or in the output directory, if different)
. Rename [.path]_highlight/highlight.pack.js_ to [.path]_highlight/highlight.min.js_
. Rename [.path]_highlight/styles/github.css_ to [.path]_highlight/styles/github.min.css_
** Replace `github` with the name of the `highlightjs-theme` you are using, if different.
. Add the attribute entry `:highlightjsdir: highlight` to the header of your AsciiDoc file.
** Alternatively, you can pass the `-a highlightjsdir=highlight` flag when invoking the Asciidoctor CLI.

The output file will use your personal copy of the highlight.js library and stylesheet instead of the one hosted on cdnjs.
"##
);
