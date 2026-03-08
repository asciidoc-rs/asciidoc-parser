# AsciiDoc parser for Rust

[![CI](https://github.com/asciidoc-rs/asciidoc-parser/actions/workflows/ci.yml/badge.svg)](https://github.com/asciidoc-rs/asciidoc-parser/actions/workflows/ci.yml) [![Latest Version](https://img.shields.io/crates/v/asciidoc-parser.svg)](https://crates.io/crates/asciidoc-parser) [![docs.rs](https://img.shields.io/docsrs/asciidoc-parser)](https://docs.rs/asciidoc-parser/) [![Codecov](https://codecov.io/gh/asciidoc-rs/asciidoc-parser/graph/badge.svg?token=ULDZN1IUR9)](https://codecov.io/gh/asciidoc-rs/asciidoc-parser) [![CodSpeed](https://img.shields.io/endpoint?url=https://codspeed.io/badge.json)](https://codspeed.io/asciidoc-rs/asciidoc-parser)

This is an effort to write a semantic parser for the [AsciiDoc language](https://docs.asciidoctor.org/asciidoc/latest/) in the [Rust](https://rust-lang.org) language.

As of October 2025 this crate is ready for experimental projects, but still has significant gaps in language coverage that render it not yet suitable for production applications. I don’t list work items that I’ve not yet started; work items that are _partially_ implemented are documented with the [**to do** tag](https://github.com/asciidoc-rs/asciidoc-parser/issues?q=is%3Aissue%20state%3Aopen%20label%3Ato-do) in GitHub. Follow that for the most current status for planned development.

You’re welcome to follow along and contribute with the understanding that I may or may not drive this project to a mature (1.0) release.

## Why do this?

Most of all this is a fun project that exercises different architectural and project design skills from my [day job](https://opensource.contentauthenticity.org). As part of that work, I write [technical standards for the Creator Assertions Working Group](https://cawg.io/specs/) in Asciidoc and [Antora](https://antora.org).

Once the parser is sufficiently built out, I have a few projects I’d like to build out that depend on it:

* A version of Antora that highlights differences between versions of a spec/document, as in version to version or proposed updates in a pull request.
* A version of Antora or similar that shows what portions of a spec are tested/completed/known good. (See the following section on “spec-driven development.”)
* A version of [Zola](https://getzola.org), the static site generator that I use for most of my web sites, that accepts Asciidoc formatted text as input. (See [Project proposal: Asciidoc support in Zola](https://zola.discourse.group/t/project-proposal-asciidoc-support-in-zola/2867).)

For now I’m still driving the parser to being complete enough for those projects to start.

## Spec-driven coverage

_(aka Why is the code coverage so low?)_

If you know me from other projects, you know that I value high code coverage and that isn’t changed here, despite what the Codecov badge above might say.

With this project, I’m doing an experiment in what I call **“spec-driven development.”** That means not only am I monitoring [coverage of the _code_](https://app.codecov.io/gh/asciidoc-rs/asciidoc-parser/tree/main/parser%2Fsrc) (which is typically above 99%) but also [coverage of the _spec_](https://app.codecov.io/gh/asciidoc-rs/asciidoc-parser/tree/main/docs%2Fmodules).

I’m reading page-by-page, line-by-line, and writing tests to verify that the implementation matches the specification(*). This slows progress considerably, but I expect it to result in an implementation that is very solid once complete. I’ve started tracking bugs found via this approach with the tag [#sddbugfind](https://github.com/asciidoc-rs/asciidoc-parser/pulls?q=is%3Apr+label%3Asddbugfind+is%3Aclosed). There are many already since I started tracking in August 2025.

(*) Yes, I’m aware that the Asciidoc language authors consider this a “language description,” not a specification. Since I’m experimenting with and potentially introducing the term spec-driven development as a broader term, I’m splitting the difference here.

## No planned support for some AsciiDoc features

The following features are supported in the [Ruby implementation of Asciidoctor](https://github.com/asciidoctor/asciidoctor), on which this project is based, but are not supported – and will likely never be supported – in this crate:

* Parsing UTF-16 content is not supported. (UTF-16 documents must be re-encoded to UTF-8 prior to parsing with this crate.)
* [Document types](https://docs.asciidoctor.org/asciidoc/latest/document/doctype/) other than `article` are not supported. Specifically, features which are enabled for the `book` doctype are not supported.
* The document attribute [`compat-mode`](https://docs.asciidoctor.org/asciidoctor/latest/migrate/asciidoc-py/#compatibility-mode) is not supported.
* The parser has built-in support for HTML5 rendering similar to what is provided in Asciidoctor. Other back ends could be supported by other crates by implementing the `InlineSubstitutionRenderer` trait. They will not be directly supported in this crate.
* Setting document attributes via the [inline attribute entry syntax](https://docs.asciidoctor.org/asciidoc/latest/attributes/inline-attribute-entries/) is not supported. (Note that this syntax is discouraged and may eventually be removed from the AsciiDoc language documentation.)
* [Retrieving include file content via URL](https://docs.asciidoctor.org/asciidoc/latest/directives/include-uri/) is not directly supported. An implementation could implement the [`IncludeFileHandler`](https://docs.rs/asciidoc-parser/latest/asciidoc_parser/parser/trait.IncludeFileHandler.html) trait to provide that behavior.

## Licenses

The `asciidoc-parser` crate is distributed under the terms of both the MIT license and the Apache License (Version 2.0).

See [LICENSE-APACHE](./LICENSE-APACHE) and [LICENSE-MIT](./LICENSE-MIT).

Note that some components and dependent crates may be licensed under different terms; please check the license terms for each crate and component for details.

IMPORTANT: This project is a personal project; it is known to my team at Adobe, but it is not an officially-sponsored project in any way.

### License for AsciiDoc language materials

IMPORTANT: This repository contains a snapshot of the AsciiDoc language description which comes with its own license terms. It is not the purpose of _this_ repository to supplant or replace that description; these documents are here as part of tooling to ensure that this crate follows the language description as closely as possible. Please consult [AsciiDoc Language @ Eclipse GitLab](https://gitlab.eclipse.org/eclipse/asciidoc-lang/asciidoc-lang) for the official language description.

The following applies to content in the `asg` and `spec` folders:

> The AsciiDoc Language and the accompanying materials are made available under the terms of the Eclipse Public License v 2.0 (EPL-2.0). See [LICENSE](https://gitlab.eclipse.org/eclipse/asciidoc-lang/asciidoc-lang/-/blob/main/LICENSE) to find the full license text.

The following applies to content in the `docs` folder:

> The user documentation for the AsciiDoc Language, located in the docs/ folder, is made available under the terms of a [Creative Commons Attribution 4.0 International License](https://creativecommons.org/licenses/by/4.0/) (CC-BY-4.0).
