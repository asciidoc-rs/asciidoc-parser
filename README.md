# AsciiDoc parser for Rust

[![CI](https://github.com/asciidoc-rs/asciidoc-parser/actions/workflows/ci.yml/badge.svg)](https://github.com/asciidoc-rs/asciidoc-parser/actions/workflows/ci.yml) [![Latest Version](https://img.shields.io/crates/v/asciidoc-parser.svg)](https://crates.io/crates/asciidoc-parser) [![docs.rs](https://img.shields.io/docsrs/asciidoc-parser)](https://docs.rs/asciidoc-parser/) [![Codecov](https://codecov.io/gh/asciidoc-rs/asciidoc-parser/graph/badge.svg?token=ULDZN1IUR9)](https://codecov.io/gh/asciidoc-rs/asciidoc-parser) [![CodSpeed](https://img.shields.io/endpoint?url=https://codspeed.io/badge.json)](https://codspeed.io/asciidoc-rs/asciidoc-parser)

This is a semantic parser for the [AsciiDoc language](https://docs.asciidoctor.org/asciidoc/latest/) written in the [Rust](https://rust-lang.org) language.

As of version 0.25.0 (July 2026) this crate is feature-complete, heavily tested, and ready to be used.

Now that the core is in place, I’ll be building the downstream projects described below. Expect the API to evolve slightly in the coming months as those projects mature. I do expect to publish a mature (1.0) release within the year.

## Security: rendering untrusted input

**If you render AsciiDoc from an untrusted source to HTML that is then served to other users, you must run the rendered HTML through an HTML sanitizer (such as [`ammonia`](https://crates.io/crates/ammonia)) before serving it.**

The HTML renderer in this crate is _not_ an HTML sanitizer. Like Ruby Asciidoctor, the AsciiDoc language intentionally allows a document to emit raw, unescaped HTML into the output. Two mechanisms do this **by design**:

* **Attribute-reference substitution.** Attribute substitution runs _after_ special-character escaping, so the `<`, `>`, and `&` characters in an attribute’s value are never escaped. A document that sets `:x: <script>alert(1)</script>` and then references `{x}` emits a live `<script>` element.
* **Passthroughs.** The passthrough forms (`+++…+++`, `pass:[…]`, and the `pass` block) re-emit their content verbatim, bypassing escaping – that is their entire purpose.

This behavior is faithful to Ruby Asciidoctor and is not a defect in this crate; it is a fundamental property of the language. It means that **rendered HTML from untrusted AsciiDoc cannot safely be served to other users without a post-rendering sanitization pass.**

Note that the [safe mode](https://docs.asciidoctor.org/asciidoc/latest/safe-modes/) (see [`SafeMode`](https://docs.rs/asciidoc-parser/latest/asciidoc_parser/parser/enum.SafeMode.html)) does **not** address this. Safe mode governs how far a document may reach outside of itself (file-system access, includes, and certain macros); it does not escape or sanitize the rendered HTML output.

## Why do this?

Most of all this is a fun project that exercises different architectural and project design skills from my [day job](https://opensource.contentauthenticity.org). As part of that work, I write [technical standards for the Creator Assertions Working Group](https://cawg.io/specs/) in Asciidoc and [Antora](https://antora.org).

There are a few projects that I’m now starting to build that depend on the parser:

* A version of Antora that highlights differences between versions of a spec/document, as in version to version or proposed updates in a pull request.
* A version of Antora or similar that shows what portions of a spec are tested/completed/known good. (See the following section on “spec-driven development.”)
* A version of [Zola](https://getzola.org), the static site generator that I use for most of my web sites, that accepts Asciidoc formatted text as input. (See [Project proposal: Asciidoc support in Zola](https://zola.discourse.group/t/project-proposal-asciidoc-support-in-zola/2867).)

## Spec-driven coverage

I value high code coverage. Code coverage for this crate is now extremely high (99.5%). But it’s not just code that I’m covering.

In this project, I also employ a technique I call **“spec-driven development.”** Since I started, that phrase has taken on a different and now more widely used meaning – [writing a structured specification up front so that an AI coding agent can implement it](https://github.blog/ai-and-ml/generative-ai/spec-driven-development-with-ai-get-started-with-a-new-open-source-toolkit/), as popularized by tooling such as [GitHub’s Spec Kit](https://github.com/github/spec-kit). That is _not_ what I mean here. In my sense the specification already exists – it’s the AsciiDoc language description – and I’m driving the implementation _toward_ it: not only am I monitoring [coverage of the _code_](https://app.codecov.io/gh/asciidoc-rs/asciidoc-parser/tree/main/parser%2Fsrc) but also [coverage of the _spec_](https://app.codecov.io/gh/asciidoc-rs/asciidoc-parser/tree/main/ref%2Fasciidoc-lang%2Fdocs%2Fmodules).

I’ve read the language definition and Asciidoctor’s extensive test suite page-by-page, line-by-line, and written tests to verify that this implementation matches the specification(*) and Asciidoctor’s behavior. This slowed progress considerably, but I believe it has resulted in an implementation that is far more solid than it would otherwise be.

(*) Yes, I’m aware that the Asciidoc language authors consider this a “language description,” not a specification. I’m splitting the difference here.

## Extensions

Extensions – custom block, block macro, inline macro, and similar processors, as provided by the [Ruby implementation of Asciidoctor](https://github.com/asciidoctor/asciidoctor) – are not planned for the 1.0 release of this crate. They may be added in a subsequent version.

## No planned support for some AsciiDoc features

The following features are supported in the [Ruby implementation of Asciidoctor](https://github.com/asciidoctor/asciidoctor), on which this project is based, but are not supported – and will likely never be supported – in this crate:

* Parsing UTF-16 content is not supported. (UTF-16 documents must be re-encoded to UTF-8 prior to parsing with this crate.)
* [Document types](https://docs.asciidoctor.org/asciidoc/latest/document/doctype/) other than `article` are not supported. Specifically, features which are enabled for the `book` doctype are not supported. These include [book parts](https://docs.asciidoctor.org/asciidoc/latest/sections/parts/) – level-0 section headings in the document body, which are reported via `WarningType::Level0SectionHeadingNotSupported` (see [#800](https://github.com/asciidoc-rs/asciidoc-parser/issues/800)) – and the `partintro` block style that goes with them (see [#794](https://github.com/asciidoc-rs/asciidoc-parser/issues/794)). A few behaviors that Asciidoctor gates on the `book` doctype are nonetheless implemented incidentally; they should not be relied upon.
* The document attribute [`compat-mode`](https://docs.asciidoctor.org/asciidoctor/latest/migrate/asciidoc-py/#compatibility-mode) is not supported.
* The legacy two-line (or _setext_) heading syntax – a line of title text underlined by a row of `=` or `-` characters – is not supported. It is not part of the [AsciiDoc language description](https://docs.asciidoctor.org/asciidoc/latest/), which defines only the single-line (ATX) form (`= Document Title`, `== Section Title`); only that form is recognized.
* The parser has built-in support for HTML5 rendering similar to what is provided in Asciidoctor. Other back ends could be supported by other crates by implementing the `InlineSubstitutionRenderer` trait. They will not be directly supported in this crate.
* Setting document attributes via the [inline attribute entry syntax](https://docs.asciidoctor.org/asciidoc/latest/attributes/inline-attribute-entries/) (`{set:name:value}` / `{set:name!}`) is not supported. (Note that this syntax is discouraged and may eventually be removed from the AsciiDoc language documentation.) As a consequence, per-cell table background colors set via the `{set:cellbgcolor:...}` document attribute are also not supported.
* [Retrieving include file content via URL](https://docs.asciidoctor.org/asciidoc/latest/directives/include-uri/) is not directly supported. An implementation could implement the [`IncludeFileHandler`](https://docs.rs/asciidoc-parser/latest/asciidoc_parser/parser/trait.IncludeFileHandler.html) trait to provide that behavior.
* The [shorthand menu syntax](https://docs.asciidoctor.org/asciidoc/latest/macros/ui-macros/) (`"File > Save"`) is not supported. Per the AsciiDoc language documentation, it is not on a standards track, so only the `menu:` macro form is implemented. (The `kbd:`, `btn:`, and `menu:` UI macros themselves _are_ supported.)

## Licenses

The `asciidoc-parser` crate is distributed under the terms of both the MIT license and the Apache License (Version 2.0).

See [LICENSE-APACHE](./LICENSE-APACHE) and [LICENSE-MIT](./LICENSE-MIT).

Note that some components and dependent crates may be licensed under different terms; please check the license terms for each crate and component for details.

IMPORTANT: This project is a personal project; it is known to my team at Adobe, but it is not an officially-sponsored project in any way.

### License for AsciiDoc language materials

IMPORTANT: This repository contains a snapshot of the AsciiDoc language description which comes with its own license terms. It is not the purpose of _this_ repository to supplant or replace that description; these documents are here as part of tooling to ensure that this crate follows the language description as closely as possible. Please consult [AsciiDoc Language @ Eclipse GitLab](https://gitlab.eclipse.org/eclipse/asciidoc-lang/asciidoc-lang) for the official language description.

The snapshot lives in [`ref/asciidoc-lang`](./ref/asciidoc-lang); see [`ref/asciidoc-lang/README.md`](./ref/asciidoc-lang/README.md) for the exact upstream commit it was taken from and how to refresh it.

The following applies to content in the `ref/asciidoc-lang/docs` folder:

> The user documentation for the AsciiDoc Language, located in the docs/ folder, is made available under the terms of a [Creative Commons Attribution 4.0 International License](https://creativecommons.org/licenses/by/4.0/) (CC-BY-4.0).

The AsciiDoc Language project as a whole is made available under the terms of the Eclipse Public License v 2.0 (EPL-2.0). See the [project LICENSE](https://gitlab.eclipse.org/eclipse/asciidoc-lang/asciidoc-lang/-/blob/main/LICENSE) for the full license text.
