//! Describes the top-level document structure.

use std::{marker::PhantomData, rc::Rc, slice::Iter};

use self_cell::self_cell;

use crate::{
    HasSpan, Parser, Span,
    attributes::Attrlist,
    blocks::{Block, ContentModel, IsBlock, Preamble, parse_utils::parse_blocks_until},
    document::{
        Author, Catalog, Docinfo, DocinfoLocation, Header, InterpretedValue, TocConfig, TocMode,
    },
    internal::debug::DebugSliceReference,
    parser::{
        CatalogResolver, DeferredWarning, InlineSubstitutionRenderer, ReferenceResolver,
        ReferenceWarning, ReferenceWarnings, ResolvedAttributes, SourceMap,
    },
    strings::CowStr,
    warnings::{Warning, WarningType},
};

/// A document represents the top-level block element in AsciiDoc. It consists
/// of an optional document header and either a) one or more sections preceded
/// by an optional preamble or b) a sequence of top-level blocks only.
///
/// The document can be configured using a document header. The header is not a
/// block itself, but contributes metadata to the document, such as the document
/// title and document attributes.
///
/// The `Document` structure is a self-contained package of the original content
/// that was parsed and the data structures that describe that parsed content.
/// The API functions on this struct can be used to understand the parse
/// results.
#[derive(Eq, PartialEq)]
pub struct Document<'src> {
    internal: Internal,
    _phantom: PhantomData<&'src ()>,
}

/// Internal dependent struct containing the actual data members that reference
/// the owned source.
#[derive(Debug, Eq, PartialEq)]
struct InternalDependent<'src> {
    header: Header<'src>,
    blocks: Vec<Block<'src>>,
    source: Span<'src>,
    warnings: Vec<Warning<'src>>,
    source_map: SourceMap,
    catalog: Catalog,
    attributes: ResolvedAttributes,
    toc: TocConfig,
    docinfo: Docinfo,
}

self_cell! {
    /// Internal implementation struct containing the actual data members.
    struct Internal {
        owner: String,
        #[covariant]
        dependent: InternalDependent,
    }
    impl {Debug, Eq, PartialEq}
}

impl<'src> Document<'src> {
    pub(crate) fn parse(
        source: &str,
        source_map: SourceMap,
        preprocessor_warnings: Vec<DeferredWarning>,
        parser: &mut Parser,
    ) -> Self {
        let owned_source = source.to_string();

        // Publish the source map on the parser for the duration of the parse so
        // an AsciiDoc table cell can map a position in this (preprocessed)
        // source back to the file and line it originally came from — needed to
        // report an unresolved `include::` directive inside such a cell against
        // the correct cursor. The document keeps its own copy of the map, so
        // clear the parser's reference once parsing completes.
        let source_map = Rc::new(source_map);
        parser.source_map = Some(Rc::clone(&source_map));

        let internal = Internal::new(owned_source, |owned_src| {
            let source = Span::new(owned_src);

            let mi = Header::parse(source, parser);
            let after_header = mi.item.after;

            parser.sectnumlevels = parser
                .attribute_value("sectnumlevels")
                .as_maybe_str()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(3);

            let header = mi.item.item;
            let mut warnings = mi.warnings;

            // Derive the `iconsdir` default from `imagesdir` (`{imagesdir}/icons`)
            // now that the header is fully parsed, unless the author set
            // `iconsdir` explicitly in the header (in which case it wins).
            let iconsdir_set_in_header = header.attributes().any(|a| a.name().data() == "iconsdir");
            parser.apply_iconsdir_default(iconsdir_set_in_header);

            let mut maw_blocks = parse_blocks_until(after_header, |_, _| false, parser);

            if !maw_blocks.warnings.is_empty() {
                warnings.append(&mut maw_blocks.warnings);
            }

            // A top-level section that skips level 1 (e.g. a `= Document Title`
            // followed directly by a level-2 heading) is out of sequence, but
            // the section-child boundary check only sees sections nested under
            // another section; flag the document-root case here.
            //
            // Skipped for a title-less document or when `fragment` is set — both
            // are treated as section fragments with no level-0 root to sequence
            // against — and when `leveloffset` is in effect, since a shifted (or
            // clamped) effective level no longer reflects the authored level
            // relationship and any degenerate offset is reported on its own.
            if header.title_source().is_some()
                && !parser.is_attribute_set("fragment")
                && parser.level_offset() == 0
            {
                warnings.append(&mut crate::blocks::root_section_sequence_warnings(
                    &maw_blocks.item.item,
                ));
            }

            // Warnings recorded while replacing attribute references (e.g. a
            // reference to a missing attribute under `attribute-missing=warn`)
            // are collected on the parser, where only owned offsets — not
            // borrowed spans — can live. Now that the document's owned source is
            // available, turn each one back into a spanned `Warning`.
            let root = Span::new(owned_src);

            // Warnings raised during preprocessing (e.g. an unresolved include
            // directive) are carried the same way and reconstituted here.
            for pw in preprocessor_warnings {
                warnings.push(Warning {
                    source: root.slice(pw.offset..pw.offset + pw.len),
                    warning: pw.warning,
                    origin: pw.origin,
                });
            }

            for sw in parser.take_substitution_warnings() {
                warnings.push(Warning {
                    source: root.slice(sw.offset..sw.offset + sw.len),
                    warning: sw.warning,
                    origin: None,
                });
            }

            let mut blocks = maw_blocks.item.item;
            let mut has_content_blocks = false;
            let mut preamble_split_index: Option<usize> = None;

            // Only look for preamble content if document has a title.
            // Asciidoctor only creates a preamble when there's a document title.
            if header.title().is_some() {
                for (index, block) in blocks.iter().enumerate() {
                    match block {
                        Block::DocumentAttribute(_) => (),
                        Block::Section(_) => {
                            if has_content_blocks {
                                preamble_split_index = Some(index);
                            }
                            break;
                        }
                        _ => {
                            has_content_blocks = true;
                        }
                    }
                }
            }

            if let Some(index) = preamble_split_index {
                let mut section_blocks = blocks.split_off(index);

                let preamble = Preamble::from_blocks(blocks, after_header);

                section_blocks.insert(0, Block::Preamble(preamble));
                blocks = section_blocks;
            }

            // An abstract block is not permitted as a direct child of a book
            // document without a doctitle. Asciidoctor's converter excludes
            // such a block's content and warns; the parser keeps the block in
            // the AST (as Asciidoctor does) and records the warning here, for
            // a renderer to act on.
            if matches!(
                parser.attribute_value("doctype"),
                InterpretedValue::Value(ref v) if v == "book"
            ) && header.title().is_none()
            {
                for block in &blocks {
                    if block.declared_style() == Some("abstract")
                        && block.resolved_context().as_ref() == "open"
                    {
                        warnings.push(Warning {
                            source: block.span(),
                            warning: WarningType::AbstractBlockInBookWithoutDoctitle,
                            origin: None,
                        });
                    }
                }
            }

            // Under `doctype: inline`, only the first eligible block is converted,
            // as bare inline content, and everything after it is dropped (the
            // rendering lives on the embed path). A compound or empty candidate
            // has no inline content to emit, so warn here — matching
            // Asciidoctor's `Document#convert` — and let the embed path render
            // nothing. This runs on the final block list (after any preamble
            // split) and uses the same candidate selection as the renderer, so
            // the two always agree on which block is the candidate.
            if matches!(
                parser.attribute_value("doctype"),
                InterpretedValue::Value(ref v) if v == "inline"
            ) && let Some(first) = first_inline_candidate(blocks.iter())
                && matches!(
                    first.content_model(),
                    ContentModel::Compound | ContentModel::Empty
                )
            {
                warnings.push(Warning {
                    source: first.span(),
                    warning: WarningType::NoInlineDoctypeCandidate,
                    origin: None,
                });
            }

            // Capture the parser's fully-resolved attribute state so it can be
            // read back through the `Document` (via `attribute_value`,
            // `has_attribute`, and `is_attribute_set`) without a `Parser` in
            // hand — the embed path a renderer uses for `convert_document`.
            let attributes = parser.snapshot_attributes();

            // The `toc` family of attributes is header-only, so the resolved
            // placement, depth, title, and class are fixed once the header (and
            // body) have been processed. Capture them here, while the parser
            // still holds the document's resolved attribute state.
            let toc = TocConfig::from_parser(parser);

            // Resolve docinfo from the final attribute state and the parser's
            // configured docinfo file handler (empty when no handler is set).
            let docinfo = Docinfo::resolve(parser);

            InternalDependent {
                header,
                blocks,
                source: source.trim_trailing_whitespace(),
                warnings,
                source_map: (*source_map).clone(),
                catalog: parser.take_catalog(),
                attributes,
                toc,
                docinfo,
            }
        });

        // The parse is complete; the document now owns its source map.
        parser.source_map = None;

        Self {
            internal,
            _phantom: PhantomData,
        }
    }

    /// Return the document header.
    pub fn header(&self) -> &Header<'_> {
        &self.internal.borrow_dependent().header
    }

    /// Return the document's authors.
    ///
    /// Authors may be declared on the [author line] or via the `author` /
    /// `author_N` document attributes; this returns the resolved list
    /// regardless of which mechanism was used. See [`Header::authors`].
    ///
    /// [author line]: https://docs.asciidoctor.org/asciidoc/latest/document/author-line/
    pub fn authors(&self) -> &[Author] {
        self.header().authors()
    }

    /// Return the document title (the level-0 `= Title`), if there was one.
    ///
    /// If the title contains a subtitle, this returns the full, combined title.
    /// Use [`Header::main_title`] and [`Header::subtitle`] (via [`header`]) to
    /// access the partitioned title.
    ///
    /// [`header`]: Self::header
    pub fn doctitle(&self) -> Option<&str> {
        self.header().title()
    }

    /// Return the document subtitle, if the document title contained one.
    ///
    /// A subtitle is the text following the final subtitle separator (a colon
    /// followed by a space, by default) in the document title. See
    /// [`Header::subtitle`].
    pub fn subtitle(&self) -> Option<&str> {
        self.header().subtitle()
    }

    /// Returns the resolved interpreted value of the named [document
    /// attribute], as of the end of parsing.
    ///
    /// This mirrors [`Parser::attribute_value`] and is the accessor to use on
    /// the *embed* path — rendering a [`Document`] you already hold, without a
    /// [`Parser`] in hand. The value reflects the document's final attribute
    /// state: built-in defaults, values set in the header or body, and the
    /// current value of any counter of the same name. An attribute that is not
    /// present, or is present but explicitly [unset], resolves to
    /// [`InterpretedValue::Unset`].
    ///
    /// [document attribute]: https://docs.asciidoctor.org/asciidoc/latest/attributes/document-attributes/
    /// [unset]: https://docs.asciidoctor.org/asciidoc/latest/attributes/unset-attributes/
    /// [`Parser::attribute_value`]: crate::Parser::attribute_value
    pub fn attribute_value<N: AsRef<str>>(&self, name: N) -> InterpretedValue {
        self.internal
            .borrow_dependent()
            .attributes
            .attribute_value(name)
    }

    /// Returns `true` if the document has a [document attribute] by this name
    /// (whether or not it is set), as of the end of parsing.
    ///
    /// This mirrors [`Parser::has_attribute`].
    ///
    /// [document attribute]: https://docs.asciidoctor.org/asciidoc/latest/attributes/document-attributes/
    /// [`Parser::has_attribute`]: crate::Parser::has_attribute
    pub fn has_attribute<N: AsRef<str>>(&self, name: N) -> bool {
        self.internal
            .borrow_dependent()
            .attributes
            .has_attribute(name)
    }

    /// Returns `true` if the document has a [document attribute] by this name
    /// which has been set (i.e. is present and not [unset]), as of the end of
    /// parsing.
    ///
    /// This mirrors [`Parser::is_attribute_set`].
    ///
    /// [document attribute]: https://docs.asciidoctor.org/asciidoc/latest/attributes/document-attributes/
    /// [unset]: https://docs.asciidoctor.org/asciidoc/latest/attributes/unset-attributes/
    /// [`Parser::is_attribute_set`]: crate::Parser::is_attribute_set
    pub fn is_attribute_set<N: AsRef<str>>(&self, name: N) -> bool {
        self.internal
            .borrow_dependent()
            .attributes
            .is_attribute_set(name)
    }

    /// Return where (and whether) this document's table of contents is
    /// generated, resolved from the [`toc` attribute].
    ///
    /// [`toc` attribute]: https://docs.asciidoctor.org/asciidoc/latest/toc/
    pub fn toc_mode(&self) -> TocMode {
        self.internal.borrow_dependent().toc.mode
    }

    /// Return the depth of section levels included in this document's table of
    /// contents, resolved from the [`toclevels` attribute] (default `2`).
    ///
    /// [`toclevels` attribute]: https://docs.asciidoctor.org/asciidoc/latest/toc/levels/
    pub fn toc_levels(&self) -> usize {
        self.internal.borrow_dependent().toc.levels
    }

    /// Return the title of this document's table of contents, resolved from the
    /// [`toc-title` attribute] (default _Table of Contents_).
    ///
    /// [`toc-title` attribute]: https://docs.asciidoctor.org/asciidoc/latest/toc/title/
    pub fn toc_title(&self) -> &str {
        &self.internal.borrow_dependent().toc.title
    }

    /// Return the CSS class applied to this document's table of contents
    /// container, resolved from the [`toc-class` attribute] (default `toc`).
    ///
    /// [`toc-class` attribute]: https://docs.asciidoctor.org/asciidoc/latest/toc/
    pub fn toc_class(&self) -> &str {
        &self.internal.borrow_dependent().toc.class
    }

    /// Return this document's resolved [docinfo] content for `location`.
    ///
    /// [Docinfo] is custom content read from external *docinfo files* and
    /// injected into the head, header, or footer of the converted output. The
    /// returned string is the concatenation of the applicable shared and
    /// private docinfo files (shared first, matching Asciidoctor), with
    /// `docinfosubs` substitutions already applied.
    ///
    /// An empty string is returned when no docinfo applies to the location —
    /// for example when no [`DocinfoFileHandler`] was configured on the parser,
    /// the `docinfo` attribute did not enable that scope/location, or no
    /// matching file was found. Docinfo files are resolved through a
    /// caller-supplied [`DocinfoFileHandler`], since this crate does not read
    /// from the filesystem itself.
    ///
    /// [docinfo]: https://docs.asciidoctor.org/asciidoc/latest/docinfo/
    /// [Docinfo]: https://docs.asciidoctor.org/asciidoc/latest/docinfo/
    /// [`DocinfoFileHandler`]: crate::parser::DocinfoFileHandler
    pub fn docinfo(&self, location: DocinfoLocation) -> &str {
        self.internal.borrow_dependent().docinfo.content(location)
    }

    /// Return an iterator over any warnings found during parsing.
    pub fn warnings(&self) -> Iter<'_, Warning<'_>> {
        self.internal.borrow_dependent().warnings.iter()
    }

    /// Return a [`Span`] describing the entire document source.
    pub fn span(&self) -> Span<'_> {
        self.internal.borrow_dependent().source
    }

    /// Return the source map that tracks original file locations.
    pub fn source_map(&self) -> &SourceMap {
        &self.internal.borrow_dependent().source_map
    }

    /// Return the document catalog for accessing referenceable elements.
    pub fn catalog(&self) -> &Catalog {
        &self.internal.borrow_dependent().catalog
    }

    /// Resolve the document's deferred cross-references using a caller-supplied
    /// [`ReferenceResolver`] and [`InlineSubstitutionRenderer`].
    ///
    /// This is the entry point for multi-document workflows: parse each
    /// document with [`Parser::parse_deferred`], then call this with a
    /// resolver that resolves targets against whatever combined index the
    /// caller has built (this crate does not merge catalogs). The resolver
    /// binds the "from" document, so a single shared resolver can be
    /// parametrized per call site.
    ///
    /// Resolution is non-destructive and may be repeated (e.g. for incremental
    /// builds or multiple output targets): the original target text is
    /// retained, so re-resolving is always possible.
    ///
    /// Each call is a **full, independent resolution sweep**. Every
    /// cross-reference is re-resolved against `resolver`, overwriting any
    /// result from a previous pass, and the returned [`ReferenceWarning`]s
    /// reflect only what *this* `resolver` could not resolve — a prior pass
    /// having resolved a target does not suppress a warning here.
    /// Consequently, resolving with a resolver that knows fewer targets
    /// than an earlier pass (for example, calling this after
    /// [`Parser::parse`] has already auto-resolved against the document's
    /// own catalog) will re-report those now-unknown targets as unresolved.
    /// Multi-document pipelines should therefore start from
    /// [`Parser::parse_deferred`], which does not auto-resolve.
    ///
    /// Each unresolved target is also recorded on the document as a
    /// [`WarningType::PossibleInvalidReference`] warning, so a host that reads
    /// [`warnings()`](Self::warnings) sees it alongside every other parse-time
    /// diagnostic. Because each sweep is independent, those warnings replace
    /// (rather than accumulate on top of) any left by an earlier sweep.
    pub fn resolve_references(
        &mut self,
        resolver: &dyn ReferenceResolver,
        renderer: &dyn InlineSubstitutionRenderer,
    ) -> Vec<ReferenceWarning> {
        self.internal.with_dependent_mut(|_owner, dependent| {
            let source = dependent.source;
            let mut warnings = ReferenceWarnings::default();

            for block in dependent.blocks.iter_mut() {
                block.resolve_references(resolver, renderer, &mut warnings);
            }

            // Footnote text is extracted out of block content, so its
            // cross-references are resolved here rather than by the block pass
            // above. The host resolver does not alias the catalog, so the
            // footnotes can be borrowed mutably in place.
            for footnote in dependent.catalog.footnotes.iter_mut() {
                footnote.resolve_references(resolver, renderer, &mut warnings, source);
            }

            replace_reference_warnings(&mut dependent.warnings, &mut warnings.doc);

            warnings.host
        })
    }

    /// Resolve the document's deferred cross-references against its own
    /// catalog.
    ///
    /// This is the single-document convenience path used by [`Parser::parse`].
    pub(crate) fn resolve_against_own_catalog(
        &mut self,
        renderer: &dyn InlineSubstitutionRenderer,
    ) -> Vec<ReferenceWarning> {
        self.internal.with_dependent_mut(|_owner, dependent| {
            let source = dependent.source;
            let mut warnings = ReferenceWarnings::default();

            // The footnotes are moved out of the catalog so they can be resolved
            // mutably while the `CatalogResolver` borrows the (footnote-free)
            // catalog. Footnotes are never cross-reference *targets*, so their
            // absence does not affect resolution.
            let mut footnotes = dependent.catalog.take_footnotes();

            let resolver = CatalogResolver::new(&dependent.catalog);
            for block in dependent.blocks.iter_mut() {
                block.resolve_references(&resolver, renderer, &mut warnings);
            }

            // Footnote text is extracted out of block content, so its
            // cross-references are resolved here rather than by the block pass
            // above.
            for footnote in footnotes.iter_mut() {
                footnote.resolve_references(&resolver, renderer, &mut warnings, source);
            }

            dependent.catalog.restore_footnotes(footnotes);

            replace_reference_warnings(&mut dependent.warnings, &mut warnings.doc);

            warnings.host
        })
    }
}

/// Folds the document warnings raised by a resolution sweep into the document's
/// own warning list.
///
/// Each sweep is a full, independent pass, so any unresolved-reference warning
/// left by an earlier sweep is discarded first; otherwise resolving a document
/// twice would report every still-unresolved reference twice.
fn replace_reference_warnings<'src>(
    document_warnings: &mut Vec<Warning<'src>>,
    sweep_warnings: &mut Vec<Warning<'src>>,
) {
    document_warnings
        .retain(|warning| !matches!(warning.warning, WarningType::PossibleInvalidReference(_)));

    document_warnings.append(sweep_warnings);
}

impl<'src> IsBlock<'src> for Document<'src> {
    fn content_model(&self) -> ContentModel {
        ContentModel::Compound
    }

    fn raw_context(&self) -> CowStr<'src> {
        "document".into()
    }

    fn nested_blocks(&'src self) -> Iter<'src, Block<'src>> {
        self.internal.borrow_dependent().blocks.iter()
    }

    fn title_source(&'src self) -> Option<Span<'src>> {
        // Document title is reflected in the Header.
        None
    }

    fn title(&self) -> Option<&str> {
        // Document title is reflected in the Header.
        None
    }

    fn anchor(&'src self) -> Option<Span<'src>> {
        None
    }

    fn anchor_reftext(&'src self) -> Option<Span<'src>> {
        None
    }

    fn attrlist(&'src self) -> Option<&'src Attrlist<'src>> {
        // Document attributes are reflected in the Header.
        None
    }
}

impl std::fmt::Debug for Document<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let dependent = self.internal.borrow_dependent();
        f.debug_struct("Document")
            .field("header", &dependent.header)
            .field("blocks", &DebugSliceReference(&dependent.blocks))
            .field("source", &dependent.source)
            .field("warnings", &DebugSliceReference(&dependent.warnings))
            .field("source_map", &dependent.source_map)
            .field("catalog", &dependent.catalog)
            .finish()
    }
}

/// Returns the first block eligible to be the sole rendered block of an
/// `inline` document.
///
/// A document-attribute entry and a comment (either a `[comment]`-styled block
/// or a `////` comment block) produce no output, so they are transparent here
/// and skipped, mirroring how Asciidoctor drops them before taking `blocks[0]`.
/// The returned block is the one an `inline` document renders (when it holds
/// inline content) or reports as having *no inline candidate* (when it is
/// compound or empty).
///
/// Both the parse-time `no inline candidate` check and the embed-path renderer
/// select the candidate through this function so the two never disagree about
/// which block is the candidate.
pub(crate) fn first_inline_candidate<'a, 'src>(
    blocks: impl Iterator<Item = &'a Block<'src>>,
) -> Option<&'a Block<'src>>
where
    'src: 'a,
{
    blocks.into_iter().find(|b| {
        !matches!(b, Block::DocumentAttribute(_))
            && b.resolved_context().as_ref() != "comment"
            && b.declared_style() != Some("comment")
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::{collections::HashMap, ops::Deref};

    use crate::{
        blocks::{ContentModel, MediaType},
        document::RefType,
        tests::prelude::*,
    };

    #[test]
    fn empty_source() {
        let doc = Parser::default().parse("");

        assert_eq!(doc.content_model(), ContentModel::Compound);
        assert_eq!(doc.raw_context().deref(), "document");
        assert_eq!(doc.resolved_context().deref(), "document");
        assert!(doc.declared_style().is_none());
        assert!(doc.id().is_none());
        assert!(doc.roles().is_empty());
        assert!(doc.title_source().is_none());
        assert!(doc.title().is_none());
        assert!(doc.anchor().is_none());
        assert!(doc.anchor_reftext().is_none());
        assert!(doc.attrlist().is_none());
        assert_eq!(doc.substitution_group(), SubstitutionGroup::Normal);

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "",
                        line: 1,
                        col: 1,
                        offset: 0
                    },
                },
                source: Span {
                    data: "",
                    line: 1,
                    col: 1,
                    offset: 0
                },
                blocks: &[],
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }

    #[test]
    fn only_spaces() {
        assert_eq!(
            Parser::default().parse("    "),
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "",
                        line: 1,
                        col: 5,
                        offset: 4
                    },
                },
                source: Span {
                    data: "",
                    line: 1,
                    col: 1,
                    offset: 0
                },
                blocks: &[],
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }

    #[test]
    fn one_simple_block() {
        let doc = Parser::default().parse("abc");
        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "",
                        line: 1,
                        col: 1,
                        offset: 0
                    },
                },
                source: Span {
                    data: "abc",
                    line: 1,
                    col: 1,
                    offset: 0
                },
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "abc",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "abc",
                    },
                    source: Span {
                        data: "abc",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                })],
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );

        assert!(doc.anchor().is_none());
        assert!(doc.anchor_reftext().is_none());
    }

    #[test]
    fn two_simple_blocks() {
        assert_eq!(
            Parser::default().parse("abc\n\ndef"),
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "",
                        line: 1,
                        col: 1,
                        offset: 0
                    },
                },
                source: Span {
                    data: "abc\n\ndef",
                    line: 1,
                    col: 1,
                    offset: 0
                },
                blocks: &[
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "abc",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "abc",
                        },
                        source: Span {
                            data: "abc",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    }),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "def",
                                line: 3,
                                col: 1,
                                offset: 5,
                            },
                            rendered: "def",
                        },
                        source: Span {
                            data: "def",
                            line: 3,
                            col: 1,
                            offset: 5,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    })
                ],
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }

    #[test]
    fn two_blocks_and_title() {
        assert_eq!(
            Parser::default().parse("= Example Title\n\nabc\n\ndef"),
            Document {
                header: Header {
                    title_source: Some(Span {
                        data: "Example Title",
                        line: 1,
                        col: 3,
                        offset: 2,
                    }),
                    title: Some("Example Title"),
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "= Example Title",
                        line: 1,
                        col: 1,
                        offset: 0,
                    }
                },
                blocks: &[
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "abc",
                                line: 3,
                                col: 1,
                                offset: 17,
                            },
                            rendered: "abc",
                        },
                        source: Span {
                            data: "abc",
                            line: 3,
                            col: 1,
                            offset: 17,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    }),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "def",
                                line: 5,
                                col: 1,
                                offset: 22,
                            },
                            rendered: "def",
                        },
                        source: Span {
                            data: "def",
                            line: 5,
                            col: 1,
                            offset: 22,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    })
                ],
                source: Span {
                    data: "= Example Title\n\nabc\n\ndef",
                    line: 1,
                    col: 1,
                    offset: 0
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }

    #[test]
    fn blank_lines_before_header() {
        let doc = Parser::default().parse("\n\n= Example Title\n\nabc\n\ndef");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: Some(Span {
                        data: "Example Title",
                        line: 3,
                        col: 3,
                        offset: 4,
                    },),
                    title: Some("Example Title",),
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "= Example Title",
                        line: 3,
                        col: 1,
                        offset: 2,
                    },
                },
                blocks: &[
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "abc",
                                line: 5,
                                col: 1,
                                offset: 19,
                            },
                            rendered: "abc",
                        },
                        source: Span {
                            data: "abc",
                            line: 5,
                            col: 1,
                            offset: 19,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "def",
                                line: 7,
                                col: 1,
                                offset: 24,
                            },
                            rendered: "def",
                        },
                        source: Span {
                            data: "def",
                            line: 7,
                            col: 1,
                            offset: 24,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: "\n\n= Example Title\n\nabc\n\ndef",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }

    #[test]
    fn blank_lines_and_comment_before_header() {
        let doc =
            Parser::default().parse("\n// ignore this comment\n= Example Title\n\nabc\n\ndef");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: Some(Span {
                        data: "Example Title",
                        line: 3,
                        col: 3,
                        offset: 26,
                    },),
                    title: Some("Example Title",),
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[Span {
                        data: "// ignore this comment",
                        line: 2,
                        col: 1,
                        offset: 1,
                    },],
                    source: Span {
                        data: "// ignore this comment\n= Example Title",
                        line: 2,
                        col: 1,
                        offset: 1,
                    },
                },
                blocks: &[
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "abc",
                                line: 5,
                                col: 1,
                                offset: 41,
                            },
                            rendered: "abc",
                        },
                        source: Span {
                            data: "abc",
                            line: 5,
                            col: 1,
                            offset: 41,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "def",
                                line: 7,
                                col: 1,
                                offset: 46,
                            },
                            rendered: "def",
                        },
                        source: Span {
                            data: "def",
                            line: 7,
                            col: 1,
                            offset: 46,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: "\n// ignore this comment\n= Example Title\n\nabc\n\ndef",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }

    #[test]
    fn extra_space_before_title() {
        assert_eq!(
            Parser::default().parse("=   Example Title\n\nabc"),
            Document {
                header: Header {
                    title_source: Some(Span {
                        data: "Example Title",
                        line: 1,
                        col: 5,
                        offset: 4,
                    }),
                    title: Some("Example Title"),
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "=   Example Title",
                        line: 1,
                        col: 1,
                        offset: 0,
                    }
                },
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "abc",
                            line: 3,
                            col: 1,
                            offset: 19,
                        },
                        rendered: "abc",
                    },
                    source: Span {
                        data: "abc",
                        line: 3,
                        col: 1,
                        offset: 19,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                })],
                source: Span {
                    data: "=   Example Title\n\nabc",
                    line: 1,
                    col: 1,
                    offset: 0
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }

    #[test]
    fn err_bad_header() {
        assert_eq!(
            Parser::default().parse(
                "= Title\nJane Smith <jane@example.com>\nv1, 2025-09-28\nnot an attribute\n"
            ),
            Document {
                header: Header {
                    title_source: Some(Span {
                        data: "Title",
                        line: 1,
                        col: 3,
                        offset: 2,
                    }),
                    title: Some("Title"),
                    attributes: &[],
                    author_line: Some(AuthorLine {
                        authors: &[Author {
                            name: "Jane Smith",
                            firstname: "Jane",
                            middlename: None,
                            lastname: Some("Smith"),
                            email: Some("jane@example.com"),
                        }],
                        source: Span {
                            data: "Jane Smith <jane@example.com>",
                            line: 2,
                            col: 1,
                            offset: 8,
                        },
                    }),
                    revision_line: Some(RevisionLine {
                        revnumber: Some("1",),
                        revdate: "2025-09-28",
                        revremark: None,
                        source: Span {
                            data: "v1, 2025-09-28",
                            line: 3,
                            col: 1,
                            offset: 38,
                        },
                    },),
                    comments: &[],
                    source: Span {
                        data: "= Title\nJane Smith <jane@example.com>\nv1, 2025-09-28",
                        line: 1,
                        col: 1,
                        offset: 0,
                    }
                },
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "not an attribute",
                            line: 4,
                            col: 1,
                            offset: 53,
                        },
                        rendered: "not an attribute",
                    },
                    source: Span {
                        data: "not an attribute",
                        line: 4,
                        col: 1,
                        offset: 53,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                })],
                source: Span {
                    data: "= Title\nJane Smith <jane@example.com>\nv1, 2025-09-28\nnot an attribute",
                    line: 1,
                    col: 1,
                    offset: 0
                },
                warnings: &[Warning {
                    source: Span {
                        data: "not an attribute",
                        line: 4,
                        col: 1,
                        offset: 53,
                    },
                    warning: WarningType::DocumentHeaderNotTerminated,
                },],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }

    #[test]
    fn err_bad_header_and_bad_macro() {
        let doc = Parser::default().parse("= Title\nJane Smith <jane@example.com>\nv1, 2025-09-28\nnot an attribute\n\n== Section Title\n\nimage::bar[alt=Sunset,width=300,,height=400]");

        assert_eq!(
            Document {
                header: Header {
                    title_source: Some(Span {
                        data: "Title",
                        line: 1,
                        col: 3,
                        offset: 2,
                    }),
                    title: Some("Title"),
                    attributes: &[],
                    author_line: Some(AuthorLine {
                        authors: &[Author {
                            name: "Jane Smith",
                            firstname: "Jane",
                            middlename: None,
                            lastname: Some("Smith"),
                            email: Some("jane@example.com"),
                        }],
                        source: Span {
                            data: "Jane Smith <jane@example.com>",
                            line: 2,
                            col: 1,
                            offset: 8,
                        },
                    }),
                    revision_line: Some(RevisionLine {
                        revnumber: Some("1"),
                        revdate: "2025-09-28",
                        revremark: None,
                        source: Span {
                            data: "v1, 2025-09-28",
                            line: 3,
                            col: 1,
                            offset: 38,
                        },
                    },),
                    comments: &[],
                    source: Span {
                        data: "= Title\nJane Smith <jane@example.com>\nv1, 2025-09-28",
                        line: 1,
                        col: 1,
                        offset: 0,
                    }
                },
                blocks: &[
                    Block::Preamble(Preamble {
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "not an attribute",
                                    line: 4,
                                    col: 1,
                                    offset: 53,
                                },
                                rendered: "not an attribute",
                            },
                            source: Span {
                                data: "not an attribute",
                                line: 4,
                                col: 1,
                                offset: 53,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            caption: None,
                            number: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "not an attribute",
                            line: 4,
                            col: 1,
                            offset: 53,
                        },
                    },),
                    Block::Section(SectionBlock {
                        level: 1,
                        section_title: Content {
                            original: Span {
                                data: "Section Title",
                                line: 6,
                                col: 4,
                                offset: 74,
                            },
                            rendered: "Section Title",
                        },
                        blocks: &[Block::Media(MediaBlock {
                            type_: MediaType::Image,
                            target: Span {
                                data: "bar",
                                line: 8,
                                col: 8,
                                offset: 96,
                            },
                            macro_attrlist: Attrlist {
                                attributes: &[
                                    ElementAttribute {
                                        name: Some("alt"),
                                        shorthand_items: &[],
                                        value: "Sunset"
                                    },
                                    ElementAttribute {
                                        name: Some("width"),
                                        shorthand_items: &[],
                                        value: "300"
                                    },
                                    ElementAttribute {
                                        name: Some("height"),
                                        shorthand_items: &[],
                                        value: "400"
                                    },
                                ],
                                anchor: None,
                                source: Span {
                                    data: "alt=Sunset,width=300,,height=400",
                                    line: 8,
                                    col: 12,
                                    offset: 100,
                                },
                            },
                            source: Span {
                                data: "image::bar[alt=Sunset,width=300,,height=400]",
                                line: 8,
                                col: 1,
                                offset: 89,
                            },
                            title_source: None,
                            title: None,
                            caption: None,
                            number: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "== Section Title\n\nimage::bar[alt=Sunset,width=300,,height=400]",
                            line: 6,
                            col: 1,
                            offset: 71,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                        section_type: SectionType::Normal,
                        section_id: Some("_section_title"),
                        caption: None,
                        section_number: None,
                    },)
                ],
                source: Span {
                    data: "= Title\nJane Smith <jane@example.com>\nv1, 2025-09-28\nnot an attribute\n\n== Section Title\n\nimage::bar[alt=Sunset,width=300,,height=400]",
                    line: 1,
                    col: 1,
                    offset: 0
                },
                warnings: &[
                    Warning {
                        source: Span {
                            data: "not an attribute",
                            line: 4,
                            col: 1,
                            offset: 53,
                        },
                        warning: WarningType::DocumentHeaderNotTerminated,
                    },
                    Warning {
                        source: Span {
                            data: "alt=Sunset,width=300,,height=400",
                            line: 8,
                            col: 12,
                            offset: 100,
                        },
                        warning: WarningType::EmptyAttributeValue,
                    },
                ],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([(
                        "_section_title",
                        RefEntry {
                            id: "_section_title",
                            reftext: Some("Section Title",),
                            ref_type: RefType::Section,
                        }
                    ),]),
                    reftext_to_id: HashMap::from([("Section Title", "_section_title"),]),
                }
            },
            doc
        );
    }

    #[test]
    fn impl_debug() {
        let doc = Parser::default().parse("= Example Title\n\nabc\n\ndef");

        assert_eq!(
            format!("{doc:#?}"),
            r#"Document {
    header: Header {
        title_source: Some(
            Span {
                data: "Example Title",
                line: 1,
                col: 3,
                offset: 2,
            },
        ),
        title: Some(
            "Example Title",
        ),
        main_title: Some(
            "Example Title",
        ),
        subtitle: None,
        attributes: &[],
        author_line: None,
        authors: [],
        revision_line: None,
        comments: &[],
        source: Span {
            data: "= Example Title",
            line: 1,
            col: 1,
            offset: 0,
        },
    },
    blocks: &[
        Block::Simple(
            SimpleBlock {
                content: Content {
                    original: Span {
                        data: "abc",
                        line: 3,
                        col: 1,
                        offset: 17,
                    },
                    rendered: "abc",
                },
                source: Span {
                    data: "abc",
                    line: 3,
                    col: 1,
                    offset: 17,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                caption: None,
                number: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },
        ),
        Block::Simple(
            SimpleBlock {
                content: Content {
                    original: Span {
                        data: "def",
                        line: 5,
                        col: 1,
                        offset: 22,
                    },
                    rendered: "def",
                },
                source: Span {
                    data: "def",
                    line: 5,
                    col: 1,
                    offset: 22,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                caption: None,
                number: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },
        ),
    ],
    source: Span {
        data: "= Example Title\n\nabc\n\ndef",
        line: 1,
        col: 1,
        offset: 0,
    },
    warnings: &[],
    source_map: SourceMap(&[]),
    catalog: Catalog {
        refs: HashMap::from([]),
        reftext_to_id: HashMap::from([]),
        footnotes: [],
    },
}"#
        );
    }

    mod attribute_access {
        use crate::{document::InterpretedValue, tests::prelude::*};

        #[test]
        fn built_in_default() {
            // `doctype` is a built-in attribute with a default of `article`; it
            // should read back through the `Document` even though the source
            // never sets it.
            let doc = Parser::default().parse("Hello.");

            assert!(doc.has_attribute("doctype"));
            assert!(doc.is_attribute_set("doctype"));
            assert_eq!(
                doc.attribute_value("doctype"),
                InterpretedValue::Value("article".to_string())
            );
        }

        #[test]
        fn header_set_attribute() {
            let doc = Parser::default().parse("= Title\n:lang: fr\n\nBonjour.");

            assert!(doc.has_attribute("lang"));
            assert!(doc.is_attribute_set("lang"));
            assert_eq!(
                doc.attribute_value("lang"),
                InterpretedValue::Value("fr".to_string())
            );
        }

        #[test]
        fn body_set_attribute() {
            // An attribute set in the document body (not the header) is part of
            // the final resolved state and must be visible on the `Document`.
            let doc = Parser::default().parse("First paragraph.\n\n:foo: bar\n\nSecond paragraph.");

            assert!(doc.has_attribute("foo"));
            assert!(doc.is_attribute_set("foo"));
            assert_eq!(
                doc.attribute_value("foo"),
                InterpretedValue::Value("bar".to_string())
            );
        }

        #[test]
        fn set_flag_attribute() {
            // A bare `:sectnums:` turns the attribute on; its resolved value is
            // the built-in default `all`.
            let doc = Parser::default().parse("= Title\n:sectnums:\n\nBody.");

            assert!(doc.has_attribute("sectnums"));
            assert!(doc.is_attribute_set("sectnums"));
            assert_eq!(
                doc.attribute_value("sectnums"),
                InterpretedValue::Value("all".to_string())
            );
        }

        #[test]
        fn unset_attribute() {
            // `sectnums` exists in the built-in table but is unset by default.
            let doc = Parser::default().parse("Hello.");

            assert!(doc.has_attribute("sectnums"));
            assert!(!doc.is_attribute_set("sectnums"));
            assert_eq!(doc.attribute_value("sectnums"), InterpretedValue::Unset);
        }

        #[test]
        fn explicitly_unset_attribute() {
            // `:!sectnums:` explicitly unsets an otherwise-set attribute: it is
            // present but not set.
            let doc = Parser::default().parse("= Title\n:sectnums:\n:!sectnums:\n\nBody.");

            assert!(doc.has_attribute("sectnums"));
            assert!(!doc.is_attribute_set("sectnums"));
            assert_eq!(doc.attribute_value("sectnums"), InterpretedValue::Unset);
        }

        #[test]
        fn absent_attribute() {
            let doc = Parser::default().parse("Hello.");

            assert!(!doc.has_attribute("no-such-attribute"));
            assert!(!doc.is_attribute_set("no-such-attribute"));
            assert_eq!(
                doc.attribute_value("no-such-attribute"),
                InterpretedValue::Unset
            );
        }

        #[test]
        fn matches_parser_state() {
            // The values read back through the `Document` must equal what the
            // `Parser` itself reports after `parse`.
            let mut parser = Parser::default();
            let doc = parser.parse("= Title\n:lang: de\n:sectnums:\n\nBody.");

            for name in [
                "lang",
                "sectnums",
                "doctype",
                "notitle",
                "no-such-attribute",
            ] {
                assert_eq!(doc.attribute_value(name), parser.attribute_value(name));
                assert_eq!(doc.has_attribute(name), parser.has_attribute(name));
                assert_eq!(doc.is_attribute_set(name), parser.is_attribute_set(name));
            }
        }

        #[test]
        fn counter_value() {
            // A counter's current value is part of the resolved attribute state
            // and supersedes any like-named attribute.
            let doc = Parser::default().parse("{counter:my-counter}\n\n{counter:my-counter}");

            assert!(doc.has_attribute("my-counter"));
            assert!(doc.is_attribute_set("my-counter"));
            assert_eq!(
                doc.attribute_value("my-counter"),
                InterpretedValue::Value("2".to_string())
            );
        }
    }
}
