# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

As of January 2026 and until the 1.0.0 version is released, I will only make minor version changes (incrementing the x in 0.x.0) if breaking changes are made (including changing the minimum supported Rust version). Features will now result in a patch version change (incrementing the y in 0.x.y). This brings us into closer compliance with typical SemVer practice (and follows the default behavior of release-plz).

## [0.29.3](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.29.2...v0.29.3)
_01 August 2026_

### Fixed

* Set the combined `authors` attribute from a single `:author:` entry ([#1032](https://github.com/asciidoc-rs/asciidoc-parser/pull/1032))
* Treat a section heading inside a delimited block as literal content ([#1030](https://github.com/asciidoc-rs/asciidoc-parser/pull/1030))
* Coerce a leveloffset-shifted level-0 heading to the document title ([#1028](https://github.com/asciidoc-rs/asciidoc-parser/pull/1028))
* Ignore an empty block anchor `[[]]` instead of rendering it literally ([#1026](https://github.com/asciidoc-rs/asciidoc-parser/pull/1026))
* Recognize a block title above the document title as document metadata ([#1029](https://github.com/asciidoc-rs/asciidoc-parser/pull/1029))

## [0.29.2](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.29.1...v0.29.2)
_01 August 2026_

### Added

* Add IncludeResolution::NotDecodable for a found-but-undecodable include ([#1021](https://github.com/asciidoc-rs/asciidoc-parser/pull/1021))

### Fixed

* Warn when a special section that doesn't support subsections contains a nested section ([#1017](https://github.com/asciidoc-rs/asciidoc-parser/pull/1017))
* Treat a `[float]`/`[discrete]` level-0 (`=`) heading as a discrete heading ([#1018](https://github.com/asciidoc-rs/asciidoc-parser/pull/1018))
* Reflect an indexed :author_N: override in the combined authors string ([#1016](https://github.com/asciidoc-rs/asciidoc-parser/pull/1016))

## [0.29.1](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.29.0...v0.29.1)
_31 July 2026_

### Added

* Expose the inline passthrough collection on a block's content ([#1008](https://github.com/asciidoc-rs/asciidoc-parser/pull/1008))

### Fixed

* Run :author: attribute value through substitutions before name partitioning ([#1009](https://github.com/asciidoc-rs/asciidoc-parser/pull/1009))
* Emit a diagnostic for lines dropped under attribute-missing=drop-line ([#1012](https://github.com/asciidoc-rs/asciidoc-parser/pull/1012))
* Populate combined `authors` and `author_1` attributes from the implicit author line ([#1007](https://github.com/asciidoc-rs/asciidoc-parser/pull/1007))
* Reconcile an explicit `:authors:` entry against the implicit author line ([#1006](https://github.com/asciidoc-rs/asciidoc-parser/pull/1006))

## [0.29.0](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.28.1...v0.29.0)
_30 July 2026_

### Added

* [**breaking**] Distinguish an unreadable include file from a missing one ([#999](https://github.com/asciidoc-rs/asciidoc-parser/pull/999))

### Breaking changes

* **`IncludeFileHandler`** ([#999](https://github.com/asciidoc-rs/asciidoc-parser/pull/999)) — `IncludeFileHandler::resolve_target` now returns the new `IncludeResolution` enum instead of `Option<IncludeContent>`, so a handler can signal *why* resolution failed. A file that exists but cannot be read (e.g. a permission error) now reports the new `WarningType::IncludeFileNotReadable` warning, matching Asciidoctor's `include file not readable`, rather than being conflated with a missing file. To migrate an `IncludeFileHandler` implementation: map what was `Some(content)` to `IncludeResolution::Found(content)` (or `content.into()`, via the provided `From<IncludeContent>` conversion), and what was `None` to `IncludeResolution::NotFound`; a handler that reads from the filesystem should additionally return `IncludeResolution::NotReadable` when the file exists but the read fails (e.g. distinguish `io::ErrorKind::NotFound` from other IO errors). The rendered `Unresolved directive` replacement and the `opts=optional` short-circuit are unchanged for both failure reasons — only the emitted warning differs. `IncludeResolution` is `#[non_exhaustive]`, leaving room for future reasons.

## [0.28.1](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.28.0...v0.28.1)
_29 July 2026_

### Fixed

* Keep block metadata attached when a comment sits directly above it ([#996](https://github.com/asciidoc-rs/asciidoc-parser/pull/996))

## [0.28.0](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.27.4...v0.28.0)
_29 July 2026_

### Fixed

* Mirror Asciidoctor TOC position normalization ([#993](https://github.com/asciidoc-rs/asciidoc-parser/pull/993))

## [0.27.4](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.27.3...v0.27.4)
_27 July 2026_

### Fixed

* Unlock an API-enabled `numbered` so a body toggle still applies ([#990](https://github.com/asciidoc-rs/asciidoc-parser/pull/990))

## [0.27.3](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.27.2...v0.27.3)
_27 July 2026_

### Added

* Expose an AsciiDoc table cell's own footnotes ([#977](https://github.com/asciidoc-rs/asciidoc-parser/pull/977))

### Fixed

* Tokenize a run of `+` around a passthrough like Asciidoctor ([#988](https://github.com/asciidoc-rs/asciidoc-parser/pull/988))
* Preserve a quoted role on an inline passthrough span ([#973](https://github.com/asciidoc-rs/asciidoc-parser/pull/973)) ([#978](https://github.com/asciidoc-rs/asciidoc-parser/pull/978))
* Honor an empty document `caption` attribute to suppress a block's caption ([#984](https://github.com/asciidoc-rs/asciidoc-parser/pull/984))
* Fall back to link macro for remote include under non-secure safe mode ([#985](https://github.com/asciidoc-rs/asciidoc-parser/pull/985))
* Parse the `toc::[]` block macro as a `toc`-context block ([#986](https://github.com/asciidoc-rs/asciidoc-parser/pull/986))
* Honor API-supplied `hardbreaks` document attribute for line breaks ([#976](https://github.com/asciidoc-rs/asciidoc-parser/pull/976))

## [0.27.2](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.27.1...v0.27.2)
_26 July 2026_

### Fixed

* Resolve cross-references to the document title (issues #965, #968) ([#970](https://github.com/asciidoc-rs/asciidoc-parser/pull/970))
* Report `floating_title` context for discrete headings ([#967](https://github.com/asciidoc-rs/asciidoc-parser/pull/967))

## [0.27.1](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.27.0...v0.27.1)
_25 July 2026_

### Fixed

* Unescape `\{` and `\}` in attribute reference substitution ([#963](https://github.com/asciidoc-rs/asciidoc-parser/pull/963))
* Recognize block title with a leading period (`..name`) ([#958](https://github.com/asciidoc-rs/asciidoc-parser/pull/958))

## [0.27.0](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.26.0...v0.27.0)
_25 July 2026_

### Added

* [**breaking**] Derive Hash on the public AST/output types; mark Warning non_exhaustive ([#940](https://github.com/asciidoc-rs/asciidoc-parser/pull/940))

### Fixed

* Capture block metadata on list items instead of dropping it ([#954](https://github.com/asciidoc-rs/asciidoc-parser/pull/954))

### Other

* Replace Span::take_ident with spec-grounded take_block_macro_name ([#953](https://github.com/asciidoc-rs/asciidoc-parser/pull/953))

## [0.26.0](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.25.0...v0.26.0)
_24 July 2026_

### Added

* Let a custom InlineSubstitutionRenderer override only what differs (close #896) ([#933](https://github.com/asciidoc-rs/asciidoc-parser/pull/933))
* [**breaking**] Replace IsBlock::nested_blocks with a complete child_blocks API (close #894) ([#930](https://github.com/asciidoc-rs/asciidoc-parser/pull/930))
* Map preprocessed spans back to origin file, line, column, and fidelity ([#917](https://github.com/asciidoc-rs/asciidoc-parser/pull/917))

### Documented

* Complete the truncated doc comment on `LookaheadReplacer` (close #906) ([#925](https://github.com/asciidoc-rs/asciidoc-parser/pull/925))
* Document that untrusted AsciiDoc output requires HTML sanitization (close #889) ([#920](https://github.com/asciidoc-rs/asciidoc-parser/pull/920))
* Normalize comment and doc prose style across the codebase ([#915](https://github.com/asciidoc-rs/asciidoc-parser/pull/915))
* Correct SafeMode Safe/Server docstrings to match enforced protection (close #890) ([#913](https://github.com/asciidoc-rs/asciidoc-parser/pull/913))

### Fixed

* Emit document warnings in source order (close #901) ([#932](https://github.com/asciidoc-rs/asciidoc-parser/pull/932))
* Repair stale nested_blocks() call left by concurrent PR landing ([#936](https://github.com/asciidoc-rs/asciidoc-parser/pull/936))
* Reject a dangerous image/icon target promoted to an href via link=self (close #919) ([#927](https://github.com/asciidoc-rs/asciidoc-parser/pull/927))
* Enforce SourceMap append ordering with a debug assertion (close #903) ([#931](https://github.com/asciidoc-rs/asciidoc-parser/pull/931))
* Make catalog ID iteration deterministic across process runs (close #899) ([#926](https://github.com/asciidoc-rs/asciidoc-parser/pull/926))
* Reduce allocations in the inline substitution pipeline (close #905) ([#924](https://github.com/asciidoc-rs/asciidoc-parser/pull/924))
* Fall back to an empty span instead of the whole span on out-of-bounds slice (close #902) ([#923](https://github.com/asciidoc-rs/asciidoc-parser/pull/923))
* Escape href/target/icon attributes and reject script URIs in link macro (close #888) ([#911](https://github.com/asciidoc-rs/asciidoc-parser/pull/911))
* Strip a leading UTF-8 BOM before parsing (close #900) ([#918](https://github.com/asciidoc-rs/asciidoc-parser/pull/918))
* Cap block nesting depth to prevent stack overflow on untrusted input (close #885) ([#910](https://github.com/asciidoc-rs/asciidoc-parser/pull/910))
* Reset document attribute values between parses on a reused Parser (close #893) ([#909](https://github.com/asciidoc-rs/asciidoc-parser/pull/909))
* Honor char boundary when skipping constrained-monospace look-ahead (close #887) ([#912](https://github.com/asciidoc-rs/asciidoc-parser/pull/912))
* Avoid O(n²) parse time on many repeated block delimiters (close #886) ([#914](https://github.com/asciidoc-rs/asciidoc-parser/pull/914))

### Other

* [**breaking**] Make PathResolver a trait with a with_path_resolver builder (close #898) ([#934](https://github.com/asciidoc-rs/asciidoc-parser/pull/934))
* Give the `link:` prefix its own bracket-required INLINE_LINK branch (close #908) ([#928](https://github.com/asciidoc-rs/asciidoc-parser/pull/928))
* [**breaking**] Polish public rendering API ahead of 1.0 (close #897) ([#929](https://github.com/asciidoc-rs/asciidoc-parser/pull/929))
* Avoid per-cell String allocations when locking attributes in AsciiDoc table cells (close #904) ([#922](https://github.com/asciidoc-rs/asciidoc-parser/pull/922))

### Breaking changes

This release settles several pre-1.0 public-API commitments. Each is a source-level rename or reshape of the extension surface; the parser's behavior is unchanged. Downstream code updates as follows:

* **Block traversal** ([#930](https://github.com/asciidoc-rs/asciidoc-parser/pull/930)) — `IsBlock::nested_blocks()` is removed. Use `FindBlocks::child_blocks()` on `Document`/`Block` (or the inherent `child_blocks()` on a concrete block type) as the single, discoverable direct-children accessor. Unlike the old method it is blockquote-complete, and — like `descendant_blocks()` — it does not descend into AsciiDoc table cells unless you opt in with `BlockSelector::traverse_documents`. It returns the new opaque `ChildBlocks` iterator. The internal write-back hook `IsBlock::nested_blocks_mut()` is renamed to `child_blocks_mut()`. Several public iterators — `Document::warnings()`, `AuthorLine::authors()`, `Header::attributes()` / `Header::comments()`, and `Attrlist::attributes()` — no longer leak `std::slice::Iter<…>` in their signatures; they now return named opaque iterator types that remain `ExactSizeIterator` + `DoubleEndedIterator` (so `.len()` and reverse iteration keep working). Only code that named the old `std::slice::Iter` return type explicitly needs to change.
* **`PathResolver`** ([#934](https://github.com/asciidoc-rs/asciidoc-parser/pull/934)) — `PathResolver` is now a trait, matching the crate's other `Rc<dyn …>` handler seams, so a host can override path/URL resolution. The former concrete struct is now `DefaultPathResolver` (behavior unchanged); replace `PathResolver::default()` or `PathResolver { file_separator }` with `DefaultPathResolver`. `Parser::path_resolver` is no longer a public field — configure a custom resolver via the new `Parser::with_path_resolver(...)` builder, consistent with `with_image_file_handler` and the rest.
* **Rendering API** ([#929](https://github.com/asciidoc-rs/asciidoc-parser/pull/929)) — the misspelled trait method `InlineSubstitutionRenderer::render_quoted_substitition` is renamed to `render_quoted_substitution`; every implementor must update the method name. The single-variant `LinkRenderType` enum and the `LinkRenderParams::type_` field it backed are removed (the field carried no information; a link-type distinction can be reintroduced deliberately later). `LinkRenderParams::window` changes from `Option<&'static str>` to `Option<&'a str>` to match `XrefRenderParams::window` — a pure signature change, since the only value ever passed is the `"_blank"` literal.
* **`SourceMap`** ([#917](https://github.com/asciidoc-rs/asciidoc-parser/pull/917)) — `SourceMap` changed from a tuple struct (`SourceMap(pub Vec<(usize, SourceLine)>)`) to a plain struct with private fields, to hold the new interned-origin representation behind the added `origin_at`/`origin_of` lookups. It is obtained from `Document`, not constructed directly, and `original_file_and_line()` and `SourceLine` are unchanged; only code that built a `SourceMap` struct literal or read its `.0` field needs to update.

## [0.25.0](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.24.0...v0.25.0)
_23 July 2026_

### Added

* Populate the document catalog's link registry (close #335) ([#872](https://github.com/asciidoc-rs/asciidoc-parser/pull/872))

### Fixed

* Verify link attrlist enclosure rule for invalid attribute names (close #871) ([#883](https://github.com/asciidoc-rs/asciidoc-parser/pull/883))
* Expand tabs from `tabsize` independently of `indent` on included content (close #877) ([#881](https://github.com/asciidoc-rs/asciidoc-parser/pull/881))
* Match Asciidoctor's first-delimiter rule for mixed ifdef/ifndef combinators (close #866) ([#868](https://github.com/asciidoc-rs/asciidoc-parser/pull/868))

### Other

* Verify document-title & lines= selection for includes with leading blank lines (close #876) ([#882](https://github.com/asciidoc-rs/asciidoc-parser/pull/882))
* *(warnings)* [**breaking**] Review WarningType variant names and messages (close #801) ([#880](https://github.com/asciidoc-rs/asciidoc-parser/pull/880))
* Resolve remaining to_do_verifies! blocks from the #628 sweep ([#878](https://github.com/asciidoc-rs/asciidoc-parser/pull/878))
* Cover inline-macro spec rules hidden behind non_normative! (close #865) ([#875](https://github.com/asciidoc-rs/asciidoc-parser/pull/875))

## [0.24.0](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.23.4...v0.24.0)
_23 July 2026_

### Added

* Drop YAML-style front matter when `skip-front-matter` is set (close #745) ([#848](https://github.com/asciidoc-rs/asciidoc-parser/pull/848))
* Embed images as `data-uri` and catalog image assets (close #697) ([#854](https://github.com/asciidoc-rs/asciidoc-parser/pull/854))
* Define the `user-home` intrinsic attribute (close #737) ([#843](https://github.com/asciidoc-rs/asciidoc-parser/pull/843))
* Materialize derived backend/basebackend/filetype/doctype attributes ([#738](https://github.com/asciidoc-rs/asciidoc-parser/pull/738)) ([#839](https://github.com/asciidoc-rs/asciidoc-parser/pull/839))
* Implement max-attribute-value-size limit (close #736) ([#835](https://github.com/asciidoc-rs/asciidoc-parser/pull/835))
* Define a section ID via an anchor embedded in the section title ([#751](https://github.com/asciidoc-rs/asciidoc-parser/pull/751)) ([#827](https://github.com/asciidoc-rs/asciidoc-parser/pull/827))
* Implement time-dependent document attributes (docdate/doctime/docdatetime/docyear) ([#819](https://github.com/asciidoc-rs/asciidoc-parser/pull/819))
* Resolve an inter-document xref to an included file as an internal anchor ([#808](https://github.com/asciidoc-rs/asciidoc-parser/pull/808)) ([#822](https://github.com/asciidoc-rs/asciidoc-parser/pull/822))
* Predefine the `asciidoctor-version` attribute ([#809](https://github.com/asciidoc-rs/asciidoc-parser/pull/809))
* [**breaking**] Resolve inter-document xref targets to output paths ([#773](https://github.com/asciidoc-rs/asciidoc-parser/pull/773)) ([#803](https://github.com/asciidoc-rs/asciidoc-parser/pull/803))
* Support the Markdown-style (#) document title ([#774](https://github.com/asciidoc-rs/asciidoc-parser/pull/774)) ([#796](https://github.com/asciidoc-rs/asciidoc-parser/pull/796))
* Honor `numbered`/`numbered!` as a legacy alias for `sectnums` ([#793](https://github.com/asciidoc-rs/asciidoc-parser/pull/793))
* Define a predefined `asciidoc-parser-version` attribute ([#778](https://github.com/asciidoc-rs/asciidoc-parser/pull/778)) ([#787](https://github.com/asciidoc-rs/asciidoc-parser/pull/787))
* Honor the appendix-number attribute for appendix lettering ([#789](https://github.com/asciidoc-rs/asciidoc-parser/pull/789))
* Track Asciidoctor's links_test.rb via SDD (and fix the link/xref incompatibilities it surfaces) ([#743](https://github.com/asciidoc-rs/asciidoc-parser/pull/743))
* Track Asciidoctor's sections_test.rb via SDD ([#747](https://github.com/asciidoc-rs/asciidoc-parser/pull/747))
* Track Asciidoctor's blocks_test.rb via SDD ([#767](https://github.com/asciidoc-rs/asciidoc-parser/pull/767))
* Track Asciidoctor's reader_test.rb via SDD ([#744](https://github.com/asciidoc-rs/asciidoc-parser/pull/744))

### Documented

* Mark non-`article` doctypes (incl. `manpage`) as out of scope (close #721) ([#858](https://github.com/asciidoc-rs/asciidoc-parser/pull/858))

### Fixed

* Fuse legacy `+`-continued multi-line attribute values (close #729) ([#852](https://github.com/asciidoc-rs/asciidoc-parser/pull/852))
* Formal `role=` replaces roles set via shorthand `.role` (close #732) ([#850](https://github.com/asciidoc-rs/asciidoc-parser/pull/850))
* Accept Asciidoctor `_` markdown-style thematic breaks (close #723) ([#860](https://github.com/asciidoc-rs/asciidoc-parser/pull/860))
* Derive author attributes from `:author:`/`:authors:`/`author_N` entries and set `authorcount` (close #718) ([#855](https://github.com/asciidoc-rs/asciidoc-parser/pull/855))
* Resolve attribute references case-insensitively (close #724) ([#859](https://github.com/asciidoc-rs/asciidoc-parser/pull/859))
* Do not let counters modify locked (API-set / built-in) attributes (close #725) ([#857](https://github.com/asciidoc-rs/asciidoc-parser/pull/857))
* Derive and override the doctitle from `:doctitle:`/`:title:` attribute entries (close #716) ([#853](https://github.com/asciidoc-rs/asciidoc-parser/pull/853))
* Accept Unicode word characters in attribute-entry names (close #726) ([#849](https://github.com/asciidoc-rs/asciidoc-parser/pull/849))
* Last-wins block anchors and trailing metadata transfer across comments to a following section (close #733) ([#851](https://github.com/asciidoc-rs/asciidoc-parser/pull/851))
* Warn on an unterminated `////` comment block in the document header (close #731) ([#846](https://github.com/asciidoc-rs/asciidoc-parser/pull/846))
* Reject attribute entry whose name contains or ends with a colon (close #728) ([#847](https://github.com/asciidoc-rs/asciidoc-parser/pull/847))
* Mask docdir and relativize docfile under SERVER safe mode (close #735) ([#844](https://github.com/asciidoc-rs/asciidoc-parser/pull/844))
* Materialize derived toc-position/toc-placement/toc-class attributes (close #840) ([#845](https://github.com/asciidoc-rs/asciidoc-parser/pull/845))
* Suppress preprocessor directives inside comment blocks and define the `asciidoctor` flag (close #810) ([#837](https://github.com/asciidoc-rs/asciidoc-parser/pull/837))
* Promote a level-0 heading after a comment line under an active leveloffset (close #746) ([#838](https://github.com/asciidoc-rs/asciidoc-parser/pull/838))
* Partition a 4+-part name supplied via the :author: attribute entry ([#758](https://github.com/asciidoc-rs/asciidoc-parser/pull/758)) ([#836](https://github.com/asciidoc-rs/asciidoc-parser/pull/836))
* Fold stacked block attribute lines above the document title (close #821) ([#841](https://github.com/asciidoc-rs/asciidoc-parser/pull/841))
* Emit auto-generated section id only on the heading, not the wrapper (close #734) ([#842](https://github.com/asciidoc-rs/asciidoc-parser/pull/842))
* Drop a line emptied by an unresolved reference under attribute-missing=drop (close #730) ([#831](https://github.com/asciidoc-rs/asciidoc-parser/pull/831))
* Recognize the legacy `toc2` attribute alias as a left-placed TOC (close #748) ([#829](https://github.com/asciidoc-rs/asciidoc-parser/pull/829))
* Surface a document role through Document::roles() (close #820) ([#830](https://github.com/asciidoc-rs/asciidoc-parser/pull/830))
* Accept `_`-prefixed and dotted named tokens in attribute lists (close #727) ([#834](https://github.com/asciidoc-rs/asciidoc-parser/pull/834))
* Switch toc-class default to toc2 for left/right TOC placement (close #749) ([#832](https://github.com/asciidoc-rs/asciidoc-parser/pull/832))
* Substitute attribute references in a block anchor's reftext when registering it ([#753](https://github.com/asciidoc-rs/asciidoc-parser/pull/753)) ([#828](https://github.com/asciidoc-rs/asciidoc-parser/pull/828))
* Split the implicit author line on `;` followed by space or end of line ([#757](https://github.com/asciidoc-rs/asciidoc-parser/pull/757)) ([#824](https://github.com/asciidoc-rs/asciidoc-parser/pull/824))
* Resolve soft-unset `toc-placement!` with `toc` set to macro (close #750) ([#826](https://github.com/asciidoc-rs/asciidoc-parser/pull/826))
* Condense author-line whitespace and keep angle brackets literal ([#756](https://github.com/asciidoc-rs/asciidoc-parser/pull/756)) ([#825](https://github.com/asciidoc-rs/asciidoc-parser/pull/825))
* Resolve cross-references embedded in section titles ([#770](https://github.com/asciidoc-rs/asciidoc-parser/pull/770)) ([#817](https://github.com/asciidoc-rs/asciidoc-parser/pull/817))
* Resolve revision components given as attribute references ([#759](https://github.com/asciidoc-rs/asciidoc-parser/pull/759)) ([#823](https://github.com/asciidoc-rs/asciidoc-parser/pull/823))
* Anchor footnote cross-reference warnings at the footnote ([#804](https://github.com/asciidoc-rs/asciidoc-parser/pull/804)) ([#815](https://github.com/asciidoc-rs/asciidoc-parser/pull/815))
* Do not register a bibliography anchor found in prose ([#769](https://github.com/asciidoc-rs/asciidoc-parser/pull/769)) ([#813](https://github.com/asciidoc-rs/asciidoc-parser/pull/813))
* Parse a block attribute line above the document title as document metadata ([#805](https://github.com/asciidoc-rs/asciidoc-parser/pull/805)) ([#814](https://github.com/asciidoc-rs/asciidoc-parser/pull/814))
* Recognize a URL macro preceded by a no-break space ([#768](https://github.com/asciidoc-rs/asciidoc-parser/pull/768)) ([#811](https://github.com/asciidoc-rs/asciidoc-parser/pull/811))
* Skip `////` block comments in the header before the author or revision line ([#816](https://github.com/asciidoc-rs/asciidoc-parser/pull/816))
* Sanitize attribute-entry names (close #761) ([#818](https://github.com/asciidoc-rs/asciidoc-parser/pull/818))
* Do not treat a numeric character reference as an xref path separator ([#797](https://github.com/asciidoc-rs/asciidoc-parser/pull/797))
* Apply attribute-missing policy to include directive targets ([#798](https://github.com/asciidoc-rs/asciidoc-parser/pull/798))
* Report an unresolved cross-reference as a document warning ([#772](https://github.com/asciidoc-rs/asciidoc-parser/pull/772)) ([#799](https://github.com/asciidoc-rs/asciidoc-parser/pull/799))
* Enforce max-include-depth in the preprocessor ([#792](https://github.com/asciidoc-rs/asciidoc-parser/pull/792))
* Resolve unset attribute references to empty in ifeval expressions ([#788](https://github.com/asciidoc-rs/asciidoc-parser/pull/788))
* Carry a block title above a section heading over to the section's first block ([#791](https://github.com/asciidoc-rs/asciidoc-parser/pull/791))
* Implement the abstract block style (close #783) ([#790](https://github.com/asciidoc-rs/asciidoc-parser/pull/790))
* Honor an empty `subs` list (e.g. `[subs=","]`) ([#786](https://github.com/asciidoc-rs/asciidoc-parser/pull/786))

### Other

* Cover counter deferral for a rejected metadata run above the title ([#862](https://github.com/asciidoc-rs/asciidoc-parser/pull/862))
* Remove resolved #308 reference from post_replacements titles stub ([#861](https://github.com/asciidoc-rs/asciidoc-parser/pull/861))

## [0.23.4](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.23.3...v0.23.4)
_18 July 2026_

### Added

* Track Asciidoctor's parser_test.rb via SDD ([#763](https://github.com/asciidoc-rs/asciidoc-parser/pull/763))
* Track Asciidoctor's document_test.rb via SDD ([#715](https://github.com/asciidoc-rs/asciidoc-parser/pull/715))
* Track Asciidoctor's paths_test.rb via SDD ([#755](https://github.com/asciidoc-rs/asciidoc-parser/pull/755))
* Track Asciidoctor's syntax_highlighter_test.rb via SDD ([#764](https://github.com/asciidoc-rs/asciidoc-parser/pull/764))
* Track Asciidoctor's manpage_test.rb via SDD ([#752](https://github.com/asciidoc-rs/asciidoc-parser/pull/752))
* Track Asciidoctor's preamble_test.rb via SDD ([#742](https://github.com/asciidoc-rs/asciidoc-parser/pull/742))
* Track Asciidoctor's logger_test.rb via SDD ([#741](https://github.com/asciidoc-rs/asciidoc-parser/pull/741))
* Track Asciidoctor's text_test.rb via SDD ([#739](https://github.com/asciidoc-rs/asciidoc-parser/pull/739))
* Track Asciidoctor's attributes_test.rb via SDD ([#722](https://github.com/asciidoc-rs/asciidoc-parser/pull/722))
* Track Asciidoctor's api_test.rb via SDD (and resolve authors from attributes) ([#713](https://github.com/asciidoc-rs/asciidoc-parser/pull/713))
* Track Asciidoctor's options_test.rb via SDD ([#740](https://github.com/asciidoc-rs/asciidoc-parser/pull/740))
* Track Asciidoctor's test_helper.rb via SDD ([#712](https://github.com/asciidoc-rs/asciidoc-parser/pull/712))
* Track Asciidoctor's helpers_test.rb via SDD ([#717](https://github.com/asciidoc-rs/asciidoc-parser/pull/717))
* Track Asciidoctor's invoker_test.rb via SDD ([#714](https://github.com/asciidoc-rs/asciidoc-parser/pull/714))
* Track Asciidoctor's extensions_test.rb via SDD ([#711](https://github.com/asciidoc-rs/asciidoc-parser/pull/711))
* Track Asciidoctor's converter_test.rb via SDD ([#710](https://github.com/asciidoc-rs/asciidoc-parser/pull/710))
* Track Asciidoctor's lists_test.rb via SDD ([#707](https://github.com/asciidoc-rs/asciidoc-parser/pull/707))
* Track Asciidoctor's tables_test.rb via SDD ([#706](https://github.com/asciidoc-rs/asciidoc-parser/pull/706))
* Track Asciidoctor's substitutions_test.rb via SDD ([#703](https://github.com/asciidoc-rs/asciidoc-parser/pull/703))

### Documented

* Resolve issue #146 — open blocks are delimited only by `--` (close #146) ([#702](https://github.com/asciidoc-rs/asciidoc-parser/pull/702))
* Resolve spec question on delimited block marker char matching ([#145](https://github.com/asciidoc-rs/asciidoc-parser/pull/145)) ([#700](https://github.com/asciidoc-rs/asciidoc-parser/pull/700))

### Fixed

* Warn on duplicate inline anchors ([#765](https://github.com/asciidoc-rs/asciidoc-parser/pull/765) by @pollychen-lab -- thank you!)
* Strip trailing ` +` from single-line attribute values ([#709](https://github.com/asciidoc-rs/asciidoc-parser/pull/709))
* Preserve hard line break marker in attribute entry values (close #307) ([#708](https://github.com/asciidoc-rs/asciidoc-parser/pull/708))
* Support macros nested in link/xref text (close #305) ([#704](https://github.com/asciidoc-rs/asciidoc-parser/pull/704))

## [0.23.3](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.23.2...v0.23.3)
_16 July 2026_

### Added

* Track Asciidoctor's paragraphs_test.rb via SDD ([#696](https://github.com/asciidoc-rs/asciidoc-parser/pull/696))

### Fixed

* Safe-mode handling in `InlineSubstitutionRenderer` is complete (close #277) ([#698](https://github.com/asciidoc-rs/asciidoc-parser/pull/698))

## [0.23.2](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.23.1...v0.23.2)
_15 July 2026_

### Documented

* Note extensions are not planned for 1.0 and remove #262 references ([#695](https://github.com/asciidoc-rs/asciidoc-parser/pull/695))

### Fixed

* Track Asciidoctor's attribute_list_test.rb via SDD, and fix the parser differences it surfaces ([#693](https://github.com/asciidoc-rs/asciidoc-parser/pull/693))

## [0.23.1](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.23.0...v0.23.1)
_15 July 2026_

### Added

* Render document-level `doctype: inline` ([#680](https://github.com/asciidoc-rs/asciidoc-parser/pull/680)) ([#690](https://github.com/asciidoc-rs/asciidoc-parser/pull/690))
* Render [open] styled paragraph as an open block ([#679](https://github.com/asciidoc-rs/asciidoc-parser/pull/679)) ([#688](https://github.com/asciidoc-rs/asciidoc-parser/pull/688))
* Handle unknown/custom paragraph styles ([#681](https://github.com/asciidoc-rs/asciidoc-parser/pull/681)) ([#687](https://github.com/asciidoc-rs/asciidoc-parser/pull/687))

### Fixed

* Default `relfilesuffix` to `outfilesuffix` instead of hardcoded `.html` ([#657](https://github.com/asciidoc-rs/asciidoc-parser/pull/657)) ([#689](https://github.com/asciidoc-rs/asciidoc-parser/pull/689))

### Other

* Mark multiple level-0 heading support as out of scope ([#380](https://github.com/asciidoc-rs/asciidoc-parser/pull/380)) ([#692](https://github.com/asciidoc-rs/asciidoc-parser/pull/692))

## [0.23.0](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.22.1...v0.23.0)
_14 July 2026_

### Added

* Honor cell-level alignment in table header row ([#654](https://github.com/asciidoc-rs/asciidoc-parser/pull/654)) ([#685](https://github.com/asciidoc-rs/asciidoc-parser/pull/685))
* Implement remaining style-specific substitution groups ([#682](https://github.com/asciidoc-rs/asciidoc-parser/pull/682))
* Link notitle and showtitle as inverse title-visibility toggles ([#677](https://github.com/asciidoc-rs/asciidoc-parser/pull/677)) ([#678](https://github.com/asciidoc-rs/asciidoc-parser/pull/678))
* Port system-path root handling in PathResolver::partition_path ([#653](https://github.com/asciidoc-rs/asciidoc-parser/pull/653)) ([#674](https://github.com/asciidoc-rs/asciidoc-parser/pull/674))

### Documented

* Clarify multi-line attribute soft-wrap folding is spec-correct ([#658](https://github.com/asciidoc-rs/asciidoc-parser/pull/658)) ([#671](https://github.com/asciidoc-rs/asciidoc-parser/pull/671))

### Fixed

* Attach block metadata separated from its block by a blank line ([#664](https://github.com/asciidoc-rs/asciidoc-parser/pull/664)) ([#686](https://github.com/asciidoc-rs/asciidoc-parser/pull/686))
* Treat a bare trailing backslash as literal, not a line continuation ([#666](https://github.com/asciidoc-rs/asciidoc-parser/pull/666)) ([#684](https://github.com/asciidoc-rs/asciidoc-parser/pull/684))
* Drop backslash when escaping an attribute reference ([#667](https://github.com/asciidoc-rs/asciidoc-parser/pull/667)) ([#676](https://github.com/asciidoc-rs/asciidoc-parser/pull/676))
* Expand mid-list subs group name in place and de-dup ([#673](https://github.com/asciidoc-rs/asciidoc-parser/pull/673))
* Verify section numbering on/off for sectnums unset in body ([#328](https://github.com/asciidoc-rs/asciidoc-parser/pull/328)) ([#668](https://github.com/asciidoc-rs/asciidoc-parser/pull/668))

### Breaking changes

Linking `notitle` and `showtitle` ([#678](https://github.com/asciidoc-rs/asciidoc-parser/pull/678)) also added a `Copy` derive to the public `ModificationContext` enum. `cargo-semver-checks` classifies any newly added `Copy` impl as technically breaking, which is why this is a minor (`0.x.0`) release. Almost no downstream code needs to change:

* `ModificationContext` — the field-less enum passed to `Parser::with_intrinsic_attribute()` and its variants — now derives `Copy` in addition to `Clone`. Constructing it, passing it to those builder methods, and matching on it all continue to work unchanged. The lint fires only because a `Copy` value is captured by reference (rather than moved) in a **non-`move`** closure ([rust-lang/rust#100905](https://github.com/rust-lang/rust/issues/100905)); the sole way to be affected is code that relied on such a closure *moving* the value to end the original binding's lifetime. If you hit a borrow-checker error along those lines, add `move` to the closure so it captures its own copy. No signature, variant, or field changed.

## [0.22.1](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.22.0...v0.22.1)
_13 July 2026_

### Added

* Add block search API (find_by equivalent) to Document and Block ([#660](https://github.com/asciidoc-rs/asciidoc-parser/pull/660))

## [0.22.0](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.21.0...v0.22.0)
_13 July 2026_

### Added

* Parse and expose document subtitle from the title line ([#649](https://github.com/asciidoc-rs/asciidoc-parser/pull/649))
* Expose AsciiDoc table cell as introspectable nested document ([#545](https://github.com/asciidoc-rs/asciidoc-parser/pull/545)) ([#648](https://github.com/asciidoc-rs/asciidoc-parser/pull/648))

### Fixed

* [**breaking**] Report cursor for unresolved include in owned table cell ([#641](https://github.com/asciidoc-rs/asciidoc-parser/pull/641)) ([#651](https://github.com/asciidoc-rs/asciidoc-parser/pull/651))

### Breaking changes

Reporting the originating cursor for an unresolved include in an owned table cell ([#651](https://github.com/asciidoc-rs/asciidoc-parser/pull/651)) adds a new public field to an externally-constructible struct. Code that builds it with a struct literal must add the new field (`None` preserves the previous behavior):

* `Warning` (`warnings`) gains `origin: Option<SourceLine>`. This carries a pre-resolved originating `(file, line)` for warnings that arise from privately expanded content — an `include::` directive inside an include-expanded table cell — which no document span maps to. Code that only reads warnings (the common case, via `Document::warnings()`) is unaffected.

## [0.21.0](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.20.0...v0.21.0)
_12 July 2026_

### Added

* Apply leveloffset attribute to section levels ([#609](https://github.com/asciidoc-rs/asciidoc-parser/pull/609)) ([#642](https://github.com/asciidoc-rs/asciidoc-parser/pull/642))
* Support the `title=` block attribute ([#578](https://github.com/asciidoc-rs/asciidoc-parser/pull/578)) ([#643](https://github.com/asciidoc-rs/asciidoc-parser/pull/643))
* [**breaking**] Support xrefstyle full/short cross-reference text formatting ([#640](https://github.com/asciidoc-rs/asciidoc-parser/pull/640))

### Fixed

* Number footnotes in section titles in document order ([#594](https://github.com/asciidoc-rs/asciidoc-parser/pull/594)) ([#646](https://github.com/asciidoc-rs/asciidoc-parser/pull/646))

### Breaking changes

The `full`/`short` `xrefstyle` support ([#640](https://github.com/asciidoc-rs/asciidoc-parser/pull/640)) adds a new public field to three externally-constructible structs. Code that builds any of these with a struct literal must add the new field (all three accept `None` to preserve the previous behavior):

* `RefEntry` (`document::catalog`) gains `signifier: Option<XrefSignifier>`.
* `ResolvedReference` (`parser::reference_resolver`) gains `signifier: Option<XrefSignifier>`. Prefer the `ResolvedReference::new`, `from_entry`, or `with_signifier` constructors over a struct literal to remain source-compatible across future field additions.
* `XrefRenderParams` (`parser::inline_substitution_renderer`) gains `xrefstyle: Option<XrefStyle>`.

## [0.20.0](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.19.2...v0.20.0)
_12 July 2026_

### Added

* Report the originating cursor for an unresolved include in an AsciiDoc table cell ([#639](https://github.com/asciidoc-rs/asciidoc-parser/pull/639))
* Point attribute-missing=warn at the precise reference ([#637](https://github.com/asciidoc-rs/asciidoc-parser/pull/637))
* [**breaking**] Let IncludeFileHandler transcode non-UTF-8 include content ([#633](https://github.com/asciidoc-rs/asciidoc-parser/pull/633))

### Fixed

* Gate inline autolink `&gt;` alternative to the `&lt;` context ([#503](https://github.com/asciidoc-rs/asciidoc-parser/pull/503)) ([#638](https://github.com/asciidoc-rs/asciidoc-parser/pull/638))
* Improve performance by falling back to shared built-in attributes instead of copying them ([#634](https://github.com/asciidoc-rs/asciidoc-parser/pull/634))

## [0.19.2](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.19.1...v0.19.2)
_11 July 2026_

### Added

* Recognize language-aware fenced code blocks ([#630](https://github.com/asciidoc-rs/asciidoc-parser/pull/630))

## [0.19.1](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.19.0...v0.19.1)
_10 July 2026_

### Added

* Seed intrinsic attributes that lock silently ([#626](https://github.com/asciidoc-rs/asciidoc-parser/pull/626))

## [0.19.0](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.18.0...v0.19.0)
_05 July 2026_

### Added

* Add renderer ergonomics accessors to the public API ([#622](https://github.com/asciidoc-rs/asciidoc-parser/pull/622))
* [**breaking**] Expose resolved document attributes on `Document` ([#623](https://github.com/asciidoc-rs/asciidoc-parser/pull/623))

## [0.18.0](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.17.0...v0.18.0)
_04 July 2026_

### Added

* Update README to reflect feature-complete status ([#616](https://github.com/asciidoc-rs/asciidoc-parser/pull/616))
* Support fenced code blocks (``` delimiter) ([#599](https://github.com/asciidoc-rs/asciidoc-parser/pull/599)) ([#613](https://github.com/asciidoc-rs/asciidoc-parser/pull/613))
* Retain comments in the data model; implement [comment] block styles ([#612](https://github.com/asciidoc-rs/asciidoc-parser/pull/612))
* Implement include directive lines/tags/indent/leveloffset and URI gating ([#608](https://github.com/asciidoc-rs/asciidoc-parser/pull/608))
* Implement conditional preprocessor directives (ifdef/ifndef/ifeval) ([#606](https://github.com/asciidoc-rs/asciidoc-parser/pull/606))
* Implement cross-reference features and add SDD coverage for xref pages ([#604](https://github.com/asciidoc-rs/asciidoc-parser/pull/604))
* Implement icon macro features and add SDD coverage for icon pages ([#603](https://github.com/asciidoc-rs/asciidoc-parser/pull/603))
* Implement image id= and macro imagesdir= attributes; add SDD coverage for image-format and image-ref ([#602](https://github.com/asciidoc-rs/asciidoc-parser/pull/602))
* Implement safe mode handling and safe-mode-* attributes ([#277](https://github.com/asciidoc-rs/asciidoc-parser/pull/277)) ([#598](https://github.com/asciidoc-rs/asciidoc-parser/pull/598))
* Implement inline and interactive SVG image options ([#272](https://github.com/asciidoc-rs/asciidoc-parser/pull/272)) ([#596](https://github.com/asciidoc-rs/asciidoc-parser/pull/596))
* Implement keyboard, button, and menu UI macros ([#263](https://github.com/asciidoc-rs/asciidoc-parser/pull/263)) ([#595](https://github.com/asciidoc-rs/asciidoc-parser/pull/595))
* Implement footnote inline macro ([#591](https://github.com/asciidoc-rs/asciidoc-parser/pull/591))
* Add include directive spec coverage; match Asciidoctor escaping ([#588](https://github.com/asciidoc-rs/asciidoc-parser/pull/588))
* Implement captioned titles for listing/source and image blocks ([#587](https://github.com/asciidoc-rs/asciidoc-parser/pull/587))
* Implement index terms (user-index) ([#586](https://github.com/asciidoc-rs/asciidoc-parser/pull/586))
* Implement block masquerading ([#584](https://github.com/asciidoc-rs/asciidoc-parser/pull/584))

### Documented

* Formally decline inline attribute entries / cellbgcolor ([#547](https://github.com/asciidoc-rs/asciidoc-parser/pull/547)) ([#607](https://github.com/asciidoc-rs/asciidoc-parser/pull/607))

### Other

* Add SDD coverage for faq and glossary pages ([#610](https://github.com/asciidoc-rs/asciidoc-parser/pull/610))

## [0.17.0](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.16.0...v0.17.0)
_30 June 2026_

### Added

* Implement predefined document attributes (document-attributes-ref) ([#577](https://github.com/asciidoc-rs/asciidoc-parser/pull/577))
* Implement bibliography sections, entries, and anchors ([#479](https://github.com/asciidoc-rs/asciidoc-parser/pull/479)) ([#580](https://github.com/asciidoc-rs/asciidoc-parser/pull/580))
* Implement STEM (stem/asciimath/latexmath) inline and block support ([#261](https://github.com/asciidoc-rs/asciidoc-parser/pull/261)) ([#576](https://github.com/asciidoc-rs/asciidoc-parser/pull/576))
* Implement docinfo files (head/header/footer resolution) ([#574](https://github.com/asciidoc-rs/asciidoc-parser/pull/574))
* Implement appendix caption/label (appendix-caption) ([#575](https://github.com/asciidoc-rs/asciidoc-parser/pull/575))
* Implement counter attribute references ([#569](https://github.com/asciidoc-rs/asciidoc-parser/pull/569))
* Add support for checklists (task lists) ([#572](https://github.com/asciidoc-rs/asciidoc-parser/pull/572))
* Complete ID-attribute features (xreflabel + inline shorthand ID registration) ([#567](https://github.com/asciidoc-rs/asciidoc-parser/pull/567))
* Implement all predefined character replacement attributes ([#568](https://github.com/asciidoc-rs/asciidoc-parser/pull/568))
* Drop blocks whose macro target references a missing attribute ([#566](https://github.com/asciidoc-rs/asciidoc-parser/pull/566))
* Handle unresolved attribute references (attribute-missing) ([#563](https://github.com/asciidoc-rs/asciidoc-parser/pull/563))

### Other

* Enable callout-list tests blocked on callout support ([#311](https://github.com/asciidoc-rs/asciidoc-parser/pull/311)) ([#570](https://github.com/asciidoc-rs/asciidoc-parser/pull/570))

## [0.16.0](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.15.2...v0.16.0)
_29 June 2026_

### Added

* Finish implementation of TOC parsing ([#561](https://github.com/asciidoc-rs/asciidoc-parser/pull/561))
* Add table of contents (toc) rendering ([#560](https://github.com/asciidoc-rs/asciidoc-parser/pull/560))
* Add support for callouts ([#559](https://github.com/asciidoc-rs/asciidoc-parser/pull/559))
* Add support for open blocks ([#556](https://github.com/asciidoc-rs/asciidoc-parser/pull/556))
* Add support for example blocks ([#555](https://github.com/asciidoc-rs/asciidoc-parser/pull/555))
* Add support for sidebars ([#554](https://github.com/asciidoc-rs/asciidoc-parser/pull/554))
* Add support for collapsible blocks ([#553](https://github.com/asciidoc-rs/asciidoc-parser/pull/553))
* Add support for blockquotes ([#552](https://github.com/asciidoc-rs/asciidoc-parser/pull/552))
* Add support for admonitions ([#551](https://github.com/asciidoc-rs/asciidoc-parser/pull/551))

### Other

* Add spec coverage for archived blocks pages (styles.adoc, nest.adoc) ([#558](https://github.com/asciidoc-rs/asciidoc-parser/pull/558))
* Add spec coverage for verse blocks (verses.adoc) ([#557](https://github.com/asciidoc-rs/asciidoc-parser/pull/557))

## [0.15.2](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.15.1...v0.15.2)
_26 June 2026_

### Added

* Add support for tables ([#508](https://github.com/asciidoc-rs/asciidoc-parser/pull/508))

### Fixed

* Only honor attrlist shorthand in the first attribute position ([#524](https://github.com/asciidoc-rs/asciidoc-parser/pull/524))

### Updated dependencies

* Update codspeed-criterion-compat requirement in /parser ([#538](https://github.com/asciidoc-rs/asciidoc-parser/pull/538))

## [0.15.1](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.15.0...v0.15.1)
_20 June 2026_

### Fixed

* Merge multiple block attribute-list lines ([#511](https://github.com/asciidoc-rs/asciidoc-parser/pull/511))

### Other

* Sync asciidoc_lang tests with merged spec text

## [0.15.0](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.14.5...v0.15.0)
_19 June 2026_

### Added

* Split parsing from inline cross-reference resolution ([#461](https://github.com/asciidoc-rs/asciidoc-parser/pull/461)) ([#505](https://github.com/asciidoc-rs/asciidoc-parser/pull/505))

## [0.14.5](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.14.4...v0.14.5)
_18 June 2026_

### Fixed

* URL capture group panic/wrong-URL and SkipAheadAndRetry infinite-loop ([#498](https://github.com/asciidoc-rs/asciidoc-parser/pull/498))

## [0.14.4](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.14.3...v0.14.4)
_07 March 2026_

### Added

* Parse lists of all types ([#458](https://github.com/asciidoc-rs/asciidoc-parser/pull/458))

## [0.14.3](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.14.2...v0.14.3)
_25 January 2026_

### Fixed

* Unicode safety: `str.split_at` used without regard to character offsets ([#467](https://github.com/asciidoc-rs/asciidoc-parser/pull/467))

## [0.14.2](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.14.1...v0.14.2)
_17 January 2026_

### Fixed

* Revised the `BlockMetadata::parse` function to accept block metadata items in any order ([#463](https://github.com/asciidoc-rs/asciidoc-parser/pull/463))

## [0.14.1](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.14.0...v0.14.1)
_02 January 2026_

### Added

* Add new method `IsBlock::rendered_content` ([#459](https://github.com/asciidoc-rs/asciidoc-parser/pull/459))

## [0.14.0](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.13.1...v0.14.0)
_08 December 2025_

### Added

* Improve handling of paragraph, listing, and literal blocks ([#451](https://github.com/asciidoc-rs/asciidoc-parser/pull/451))

### Updated dependencies

* Update criterion requirement from 0.7.0 to 0.8.0 in /parser ([#452](https://github.com/asciidoc-rs/asciidoc-parser/pull/452))

## [0.13.1](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.13.0...v0.13.1)
_17 November 2025_

### Fixed

* Review SDD for text formatting and punctuation pages ([#446](https://github.com/asciidoc-rs/asciidoc-parser/pull/446))

## [0.13.0](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.12.0...v0.13.0)
_16 November 2025_

### Added

* Parse thematic and page breaks ([#445](https://github.com/asciidoc-rs/asciidoc-parser/pull/445))
* Implement discrete headings ([#443](https://github.com/asciidoc-rs/asciidoc-parser/pull/443))

## [0.12.0](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.11.0...v0.12.0)
_15 November 2025_

### Added

* Recognize preamble when relevant ([#441](https://github.com/asciidoc-rs/asciidoc-parser/pull/441))
* Implement `hardbreaks-option` document attribute ([#437](https://github.com/asciidoc-rs/asciidoc-parser/pull/437))

## [0.11.0](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.10.0...v0.11.0)
_02 November 2025_

The major theme for this release is support for **[Sections](https://docs.asciidoctor.org/asciidoc/latest/sections/titles-and-levels/).**

### Added

* Add support for appendix section type ([#435](https://github.com/asciidoc-rs/asciidoc-parser/pull/435))
* Add `block_style` accessor to `Attrlist` ([#434](https://github.com/asciidoc-rs/asciidoc-parser/pull/434))
* Assign section numbers when parsing ([#429](https://github.com/asciidoc-rs/asciidoc-parser/pull/429))
* Add support for reftext throughout block data model ([#421](https://github.com/asciidoc-rs/asciidoc-parser/pull/421))
* Parse `reftext` attribute on section blocks ([#420](https://github.com/asciidoc-rs/asciidoc-parser/pull/420))
* Implement auto-generation of section IDs when appropriate ([#412](https://github.com/asciidoc-rs/asciidoc-parser/pull/412))
* Implement a document catalog ([#414](https://github.com/asciidoc-rs/asciidoc-parser/pull/414))
* Support Markdown-style (`##`, etc) section headings ([#406](https://github.com/asciidoc-rs/asciidoc-parser/pull/406))

### Fixed

* Fix internal docs ([#419](https://github.com/asciidoc-rs/asciidoc-parser/pull/419))
* `Section::parse` should register manual IDs ([#418](https://github.com/asciidoc-rs/asciidoc-parser/pull/418))
* Don't warn on skipped section level at root of document ([#407](https://github.com/asciidoc-rs/asciidoc-parser/pull/407))
* Enforce limits on section levels ([#404](https://github.com/asciidoc-rs/asciidoc-parser/pull/404))

## [0.10.0](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.9.0...v0.10.0)
_14 October 2025_

### Added

* Implement basic `include::` directive handling with a preprocessor ([#397](https://github.com/asciidoc-rs/asciidoc-parser/pull/397))

## [0.9.0](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.8.0...v0.9.0)
_12 October 2025_

### Added

* Add new function `Parser::with_inline_substitution_renderer` ([#394](https://github.com/asciidoc-rs/asciidoc-parser/pull/394))

### Fixed

* Revise `Parser::with_inline_substitution_renderer` to return a modified Self ([#396](https://github.com/asciidoc-rs/asciidoc-parser/pull/396))

## [0.8.0](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.7.0...v0.8.0)
_10 October 2025_

### Added

* Support compound names in author line ([#391](https://github.com/asciidoc-rs/asciidoc-parser/pull/391))
* Apply title substitutions when parsing section titles ([#390](https://github.com/asciidoc-rs/asciidoc-parser/pull/390))
* Set derived author name attributes when :author: attribute is set in document header ([#388](https://github.com/asciidoc-rs/asciidoc-parser/pull/388))
* Parse revision line in document header ([#377](https://github.com/asciidoc-rs/asciidoc-parser/pull/377))
* Refactor document header parsing ([#375](https://github.com/asciidoc-rs/asciidoc-parser/pull/375))
* Implement parsing for author line ([#374](https://github.com/asciidoc-rs/asciidoc-parser/pull/374))

### Fixed

* Set document attributes from revision line ([#392](https://github.com/asciidoc-rs/asciidoc-parser/pull/392))
* Author line parsing had several bugs ([#387](https://github.com/asciidoc-rs/asciidoc-parser/pull/387))
* Set document attributes from author line ([#384](https://github.com/asciidoc-rs/asciidoc-parser/pull/384))
* Set `doctitle` attribute from document header title line ([#381](https://github.com/asciidoc-rs/asciidoc-parser/pull/381))
* Allow comment lines between document start and title ([#368](https://github.com/asciidoc-rs/asciidoc-parser/pull/368))

### Updated dependencies

* Update codspeed-criterion-compat requirement in /parser ([#389](https://github.com/asciidoc-rs/asciidoc-parser/pull/389))

## [0.7.0](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.6.0...v0.7.0)
_18 September 2025_

### Added

* Implement inline anchor macro substitution ([#363](https://github.com/asciidoc-rs/asciidoc-parser/pull/363))
* Recognize anchor syntax when parsing `Attrlist` ([#362](https://github.com/asciidoc-rs/asciidoc-parser/pull/362))
* Implement `Default` for `Span` ([#355](https://github.com/asciidoc-rs/asciidoc-parser/pull/355))
* Add `Default` implementation to `Attrlist` ([#353](https://github.com/asciidoc-rs/asciidoc-parser/pull/353))

### Fixed

* Block metadata should ignore block anchor if the anchor name is invalid ([#366](https://github.com/asciidoc-rs/asciidoc-parser/pull/366))
* Apply normal substitutions in `ElementAttribute::parse` but only when parsing attrlists for blocks and only when the value is single-quoted ([#361](https://github.com/asciidoc-rs/asciidoc-parser/pull/361))
* Attribute value with unmatched initial quote ends at next comma or EOF instead ([#359](https://github.com/asciidoc-rs/asciidoc-parser/pull/359))
* Trim trailing whitespace from attrlist values ([#358](https://github.com/asciidoc-rs/asciidoc-parser/pull/358))
* A named attribute with the exact value "None" should be ignored ([#350](https://github.com/asciidoc-rs/asciidoc-parser/pull/350))
* Quoted attribute value should unescape quotes inside the value ([#348](https://github.com/asciidoc-rs/asciidoc-parser/pull/348))

### Other

* Improve SDD coverage for ID page ([#364](https://github.com/asciidoc-rs/asciidoc-parser/pull/364))

## [0.6.0](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.5.0...v0.6.0)
_08 September 2025_

### Added

* Implement `link:` macros ([#330](https://github.com/asciidoc-rs/asciidoc-parser/pull/330))
* Add new function `Parser::is_attribute_set` ([#332](https://github.com/asciidoc-rs/asciidoc-parser/pull/332))
* Do not support setting document attributes inline ([#324](https://github.com/asciidoc-rs/asciidoc-parser/pull/324))
* Parse document attributes and record them as "blocks" ([#320](https://github.com/asciidoc-rs/asciidoc-parser/pull/320))

### Fixed

* Attrlist should not look for shorthand values if first value is entirely quoted ([#334](https://github.com/asciidoc-rs/asciidoc-parser/pull/334))
* Allow empty positional attribute when parsing attrlist ([#331](https://github.com/asciidoc-rs/asciidoc-parser/pull/331))
* Delimited block should return a delimited block even if end delimiter is not found ([#318](https://github.com/asciidoc-rs/asciidoc-parser/pull/318))

## [0.5.0](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.4.0...v0.5.0)
_16 August 2025_

### Added

* Make `SubstitutionGroup` and `SubstitutionStep` public ([#302](https://github.com/asciidoc-rs/asciidoc-parser/pull/302))
* [**breaking**] Move `Content` into its own module ([#301](https://github.com/asciidoc-rs/asciidoc-parser/pull/301))
* [**breaking**] All block types now apply normal substitutions to their title ([#299](https://github.com/asciidoc-rs/asciidoc-parser/pull/299))
* [**breaking**] Apply header substitution group when parsing title in `Header` ([#295](https://github.com/asciidoc-rs/asciidoc-parser/pull/295))
* Attribute values set in document header can be used later ([#293](https://github.com/asciidoc-rs/asciidoc-parser/pull/293))
* [**breaking**] Rework document attribute parsing ([#292](https://github.com/asciidoc-rs/asciidoc-parser/pull/292))
* [**breaking**] Remove lifetime from `InterpretedValue` and `AllowableValue` ([#291](https://github.com/asciidoc-rs/asciidoc-parser/pull/291))
* [**breaking**] Revise HasSpan::span() to return Span by value not reference ([#290](https://github.com/asciidoc-rs/asciidoc-parser/pull/290))
* [**breaking**] Change `Parser::parse` so that parser state is available after the fact ([#289](https://github.com/asciidoc-rs/asciidoc-parser/pull/289))
* [**breaking**] Replace `MacroBlock` with `MediaBlock` ([#284](https://github.com/asciidoc-rs/asciidoc-parser/pull/284))
* Implement `image:` and `icon:` macro substitutions ([#264](https://github.com/asciidoc-rs/asciidoc-parser/pull/264))
* Add `path_resolver` member to `Parser` ([#275](https://github.com/asciidoc-rs/asciidoc-parser/pull/275))
* Implement (part of) `PathResolver` struct ([#273](https://github.com/asciidoc-rs/asciidoc-parser/pull/273))
* Adopt Rust edition 2024 and bump MSRV to 1.88 ([#274](https://github.com/asciidoc-rs/asciidoc-parser/pull/274))
* [**breaking**] Attribute entry values should have special chars and document attribute substitutions applied ([#268](https://github.com/asciidoc-rs/asciidoc-parser/pull/268))
* Implement passthroughs ([#259](https://github.com/asciidoc-rs/asciidoc-parser/pull/259))
* Implement post-replacement substitution ([#257](https://github.com/asciidoc-rs/asciidoc-parser/pull/257))
* Add `has_option` accessor to `IsBlock` and `Attrlist` ([#258](https://github.com/asciidoc-rs/asciidoc-parser/pull/258))
* Implement character replacement substitutions ([#256](https://github.com/asciidoc-rs/asciidoc-parser/pull/256))
* Implement attribute substitution ([#255](https://github.com/asciidoc-rs/asciidoc-parser/pull/255))
* Apply substitutions when parsing simple and raw-delimited blocks ([#253](https://github.com/asciidoc-rs/asciidoc-parser/pull/253))
* Add `substitution_group` accessor to `IsBlock` trait ([#252](https://github.com/asciidoc-rs/asciidoc-parser/pull/252))
* Implement `SubstitutionGroup` ([#251](https://github.com/asciidoc-rs/asciidoc-parser/pull/251))
* Add a reference to `InlineSubstitutionRenderer` to `Parser` ([#250](https://github.com/asciidoc-rs/asciidoc-parser/pull/250))
* [**breaking**] Revise `Content` to be a simple text container with copy-on-write for substitutions ([#241](https://github.com/asciidoc-rs/asciidoc-parser/pull/241))

### Fixed

* Look for correct name `post_replacements` in `SubstitutionsGroup::from_custom_string` ([#312](https://github.com/asciidoc-rs/asciidoc-parser/pull/312))
* `SubstitutionGroup::from_custom_string` should recognize the name `none` ([#309](https://github.com/asciidoc-rs/asciidoc-parser/pull/309))
* Allow substition group for simple and raw delimited blocks to be overridden by `subs` attribute ([#303](https://github.com/asciidoc-rs/asciidoc-parser/pull/303))
* Use new partial lookahead replacer to fix monospace parsing edge case ([#282](https://github.com/asciidoc-rs/asciidoc-parser/pull/282))
* Apply attribute value substitution before parsing `Attrlist` ([#271](https://github.com/asciidoc-rs/asciidoc-parser/pull/271))
* `Attrlist` should trim trailing whitespace from shorthand items ([#245](https://github.com/asciidoc-rs/asciidoc-parser/pull/245))

### Updated dependencies

* Update criterion requirement from 0.6.0 to 0.7.0 in /parser ([#286](https://github.com/asciidoc-rs/asciidoc-parser/pull/286))
* Update codspeed-criterion-compat requirement in /parser ([#267](https://github.com/asciidoc-rs/asciidoc-parser/pull/267))
* Update criterion requirement from 0.5.1 to 0.6.0 in /parser ([#248](https://github.com/asciidoc-rs/asciidoc-parser/pull/248))

## [0.4.0](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.3.1...v0.4.0)
_27 April 2025_

### Major change

In this release, I replaced the previous "inline content" model with a new `Content` model which more accurately matches the manner in which Asciidoc handles [content substitutions](https://docs.asciidoctor.org/asciidoc/latest/subs/).

### Added

* Change `RawDelimitedBlock` to use `Content` for its inner body ([#238](https://github.com/asciidoc-rs/asciidoc-parser/pull/238))
* Introduce `Content` model for rendered block content ([#236](https://github.com/asciidoc-rs/asciidoc-parser/pull/236))
* Plumb `&mut Parser` through to the block-level parsers ([#235](https://github.com/asciidoc-rs/asciidoc-parser/pull/235))
* Add internal `AttributeValue` struct for a single document attribute value ([#232](https://github.com/asciidoc-rs/asciidoc-parser/pull/232))
* [**breaking**] Introduce new `Parser` struct which can configure and initiate parsing ([#233](https://github.com/asciidoc-rs/asciidoc-parser/pull/233))
* [**breaking**] Rename `AttributeValue` to `InterpretedValue` ([#229](https://github.com/asciidoc-rs/asciidoc-parser/pull/229))
* [**breaking**] Remove inline content model ([#228](https://github.com/asciidoc-rs/asciidoc-parser/pull/228))
* Add new fn `Span::take_non_empty_lines` ([#225](https://github.com/asciidoc-rs/asciidoc-parser/pull/225))

## [0.3.1](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.3.0...v0.3.1)
_14 April 2025_

### Fixed

* Document attribute values that continue with `+` should include a line-end ([#221](https://github.com/asciidoc-rs/asciidoc-parser/pull/221))
* User-defined attribute names may start with a digit ([#220](https://github.com/asciidoc-rs/asciidoc-parser/pull/220))
* Enforce document attribute name restrictions (revert most of #215) ([#218](https://github.com/asciidoc-rs/asciidoc-parser/pull/218))
* Document attribute names are free-form ([#215](https://github.com/asciidoc-rs/asciidoc-parser/pull/215))

## [0.3.0](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.2.0...v0.3.0)
_11 April 2025_

### Added

* Check block anchor name for valid XML name characters ([#208](https://github.com/asciidoc-rs/asciidoc-parser/pull/208))
* Add support for block anchor syntax ([#205](https://github.com/asciidoc-rs/asciidoc-parser/pull/205))
* Add `options` accessor to `IsBlock` trait ([#198](https://github.com/asciidoc-rs/asciidoc-parser/pull/198))
* Add `options` accessor to `Attrlist` ([#197](https://github.com/asciidoc-rs/asciidoc-parser/pull/197))
* Add `roles` accessor to `IsBlock` trait ([#195](https://github.com/asciidoc-rs/asciidoc-parser/pull/195))
* Add `roles` accessor to `Attrlist` ([#193](https://github.com/asciidoc-rs/asciidoc-parser/pull/193))
* Bump MSRV to 1.81.0 ([#194](https://github.com/asciidoc-rs/asciidoc-parser/pull/194))
* Add method `IsBlock::id()` ([#184](https://github.com/asciidoc-rs/asciidoc-parser/pull/184))
* Add new trait function `IsBlock::resolved_style` ([#182](https://github.com/asciidoc-rs/asciidoc-parser/pull/182))
* Add new method `IsBlock::declared_style` ([#179](https://github.com/asciidoc-rs/asciidoc-parser/pull/179))
* Rename `IsBlock::context` to `raw_context` ([#178](https://github.com/asciidoc-rs/asciidoc-parser/pull/178))

### Fixed

* Add coverage for positional attributes in spec ([#202](https://github.com/asciidoc-rs/asciidoc-parser/pull/202))

### Other

* Fix link to AsciiDoc repo
* Add license info for AsciiDoc language snapshot

## [0.2.0](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.1.1...v0.2.0)
_30 November 2024_

### Added

* Parse attribute lists for blocks ([#164](https://github.com/asciidoc-rs/asciidoc-parser/pull/164))
* Add support for block titles using `.(title)` syntax ([#158](https://github.com/asciidoc-rs/asciidoc-parser/pull/158))
* SDD: Delimited blocks ([#157](https://github.com/asciidoc-rs/asciidoc-parser/pull/157))
* Add support for compound delimited blocks ([#150](https://github.com/asciidoc-rs/asciidoc-parser/pull/150))

### Fixed

* Add coverage for a missing case of `TInline::Span`
* Resolve new Clippy warnings for Rust 1.83 ([#161](https://github.com/asciidoc-rs/asciidoc-parser/pull/161))
* Do not treat triple-hyphen as a delimiter for open block ([#156](https://github.com/asciidoc-rs/asciidoc-parser/pull/156))
* `Span.trim_remainder` gave incorrect result if `after` was incomplete subset of `self` ([#147](https://github.com/asciidoc-rs/asciidoc-parser/pull/147))

### Updated dependencies

* Update thiserror requirement from 1.0.63 to 2.0.1 ([#152](https://github.com/asciidoc-rs/asciidoc-parser/pull/152))

## [0.1.1](https://github.com/asciidoc-rs/asciidoc-parser/compare/v0.1.0...v0.1.1)
_26 October 2024_

### Fixed

* Copy/paste error in crate description

## [0.1.0](https://github.com/asciidoc-rs/asciidoc-parser/releases/tag/v0.1.0)
_26 October 2024_

* Initial public release of this crate. (Still very much a work-in-progress.)
