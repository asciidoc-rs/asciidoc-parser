# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

As of January 2026 and until the 1.0.0 version is released, I will only make minor version changes (incrementing the x in 0.x.0) if breaking changes are made (including changing the minimum supported Rust version). Features will now result in a patch version change (incrementing the y in 0.x.y). This brings us into closer compliance with typical SemVer practice (and follows the default behavior of release-plz).

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

### Other

* Close two line-coverage gaps in parser.rs ([#645](https://github.com/asciidoc-rs/asciidoc-parser/pull/645))

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
