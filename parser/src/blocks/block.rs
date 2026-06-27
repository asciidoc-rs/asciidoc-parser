use std::slice::Iter;

use crate::{
    HasSpan, Parser, Span,
    attributes::Attrlist,
    blocks::{
        AdmonitionBlock, Break, CompoundDelimitedBlock, ContentModel, IsBlock, ListBlock, ListItem,
        ListItemMarker, MediaBlock, Preamble, QuoteBlock, RawDelimitedBlock, SectionBlock,
        SimpleBlock, TableBlock, metadata::BlockMetadata, starts_with_admonition_label,
    },
    content::{Content, SubstitutionGroup},
    document::{Attribute, RefType},
    parser::{InlineSubstitutionRenderer, ReferenceResolver, ReferenceWarning},
    span::MatchedItem,
    strings::CowStr,
    warnings::{MatchAndWarnings, Warning, WarningType},
};

/// **Block elements** form the main structure of an AsciiDoc document, starting
/// with the document itself.
///
/// A block element (aka **block**) is a discrete, line-oriented chunk of
/// content in an AsciiDoc document. Once parsed, that chunk of content becomes
/// a block element in the parsed document model. Certain blocks may contain
/// other blocks, so we say that blocks can be nested. The converter visits each
/// block in turn, in document order, converting it to a corresponding chunk of
/// output.
///
/// This enum represents all of the block types that are understood directly by
/// this parser and also implements the [`IsBlock`] trait.
#[derive(Clone, Eq, PartialEq)]
#[allow(clippy::large_enum_variant)] // TEMPORARY: review later
#[non_exhaustive]
pub enum Block<'src> {
    /// A block that’s treated as contiguous lines of paragraph text (and
    /// subject to normal substitutions) (e.g., a paragraph block).
    Simple(SimpleBlock<'src>),

    /// A media block is used to represent an image, video, or audio block
    /// macro.
    Media(MediaBlock<'src>),

    /// A section helps to partition the document into a content hierarchy.
    /// May also be a part, chapter, or special section.
    Section(SectionBlock<'src>),

    /// A list contains a sequence of items prefixed with symbol, such as a disc
    /// (aka bullet). Each individual item in the list is represented by a
    /// [`ListItem`].
    List(ListBlock<'src>),

    /// A list item is a special kind of block that is a member of a
    /// [`ListBlock`] and contains one or more blocks attached to it.
    ListItem(ListItem<'src>),

    /// A delimited block that contains verbatim, raw, or comment text. The
    /// content between the matching delimiters is not parsed for block
    /// syntax.
    RawDelimited(RawDelimitedBlock<'src>),

    /// A delimited block that can contain other blocks.
    CompoundDelimited(CompoundDelimitedBlock<'src>),

    /// An admonition draws attention to a statement by taking it out of the
    /// content's flow and labeling it with a priority (e.g., a note or a
    /// warning).
    Admonition(AdmonitionBlock<'src>),

    /// A blockquote: a quote, prose excerpt, or verse, optionally attributed to
    /// a person and a source citation.
    Quote(QuoteBlock<'src>),

    /// A table block arranges content into a grid of rows and columns.
    Table(TableBlock<'src>),

    /// Content between the end of the document header and the first section
    /// title in the document body is called the preamble.
    Preamble(Preamble<'src>),

    /// A thematic or page break.
    Break(Break<'src>),

    /// When an attribute is defined in the document body using an attribute
    /// entry, that’s simply referred to as a document attribute.
    DocumentAttribute(Attribute<'src>),
}

impl<'src> std::fmt::Debug for Block<'src> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Block::Simple(block) => f.debug_tuple("Block::Simple").field(block).finish(),
            Block::Media(block) => f.debug_tuple("Block::Media").field(block).finish(),
            Block::Section(block) => f.debug_tuple("Block::Section").field(block).finish(),
            Block::List(block) => f.debug_tuple("Block::List").field(block).finish(),
            Block::ListItem(block) => f.debug_tuple("Block::ListItem").field(block).finish(),

            Block::RawDelimited(block) => {
                f.debug_tuple("Block::RawDelimited").field(block).finish()
            }

            Block::CompoundDelimited(block) => f
                .debug_tuple("Block::CompoundDelimited")
                .field(block)
                .finish(),

            Block::Admonition(block) => f.debug_tuple("Block::Admonition").field(block).finish(),
            Block::Quote(block) => f.debug_tuple("Block::Quote").field(block).finish(),
            Block::Table(block) => f.debug_tuple("Block::Table").field(block).finish(),
            Block::Preamble(block) => f.debug_tuple("Block::Preamble").field(block).finish(),
            Block::Break(break_) => f.debug_tuple("Block::Break").field(break_).finish(),

            Block::DocumentAttribute(block) => f
                .debug_tuple("Block::DocumentAttribute")
                .field(block)
                .finish(),
        }
    }
}

impl<'src> Block<'src> {
    /// Parse a block of any type and return a `Block` that describes it.
    ///
    /// Consumes any blank lines before and after the block.
    pub(crate) fn parse(
        source: Span<'src>,
        parser: &mut Parser,
    ) -> MatchAndWarnings<'src, Option<MatchedItem<'src, Self>>> {
        Self::parse_internal(source, parser, None, false)
    }

    /// Parse a block of any type and return a `Block` that describes it.
    ///
    /// Will terminate early when parsing certain block types within a list
    /// context.
    ///
    /// Consumes any blank lines before and after the block.
    ///
    /// If `is_continuation` is true, this content was attached via a `+`
    /// continuation marker and literal blocks should preserve their
    /// indentation.
    pub(crate) fn parse_for_list_item(
        source: Span<'src>,
        parser: &mut Parser,
        parent_list_markers: &[ListItemMarker<'src>],
        is_continuation: bool,
    ) -> MatchAndWarnings<'src, Option<MatchedItem<'src, Self>>> {
        Self::parse_internal(source, parser, Some(parent_list_markers), is_continuation)
    }

    /// Shared parser for [`Block::parse`] and [`Block::parse_for_list_item`].
    fn parse_internal(
        source: Span<'src>,
        parser: &mut Parser,
        parent_list_markers: Option<&[ListItemMarker<'src>]>,
        is_continuation: bool,
    ) -> MatchAndWarnings<'src, Option<MatchedItem<'src, Self>>> {
        // Optimization: If the first line doesn't match any of the early indications
        // for delimited blocks, titles, or attrlists, we can skip directly to treating
        // this as a simple block. That saves quite a bit of parsing time.
        let first_line = source.take_line().item.discard_whitespace();

        // If it does contain any of those markers, we fall through to the more costly
        // tests below which can more accurately classify the upcoming block.
        if let Some(first_char) = first_line.chars().next()
            && !matches!(
                first_char,
                '.' | '#'
                    | '='
                    | '/'
                    | '-'
                    | '+'
                    | '*'
                    | '_'
                    | '['
                    | ':'
                    | '\''
                    | '<'
                    | '>'
                    | '"'
                    | '•'
            )
            && !first_line.contains("::")
            && !first_line.contains(";;")
            && !TableBlock::is_table_delimiter(&first_line)
            && !ListItemMarker::starts_with_marker(first_line)
            && !starts_with_admonition_label(first_line)
            && parent_list_markers.is_none()
            && let Some(MatchedItem {
                item: simple_block,
                after,
            }) = SimpleBlock::parse_fast(source, parser)
        {
            let mut warnings = vec![];
            let block = Self::Simple(simple_block);

            Self::register_block_id(
                block.id(),
                block.title(),
                block.span(),
                parser,
                &mut warnings,
            );

            return MatchAndWarnings {
                item: Some(MatchedItem { item: block, after }),
                warnings,
            };
        }

        // Look for document attributes first since these don't support block metadata.
        if first_line.starts_with(':')
            && (first_line.ends_with(':') || first_line.contains(": "))
            && let Some(attr) = Attribute::parse(source, parser)
        {
            let mut warnings: Vec<Warning<'src>> = vec![];
            parser.set_attribute_from_body(&attr.item, &mut warnings);

            return MatchAndWarnings {
                item: Some(MatchedItem {
                    item: Self::DocumentAttribute(attr.item),
                    after: attr.after,
                }),
                warnings,
            };
        }

        // Optimization not possible; start by looking for block metadata (title,
        // attrlist, etc.).
        let MatchAndWarnings {
            item: mut metadata,
            mut warnings,
        } = BlockMetadata::parse(source, parser);

        let is_literal =
            metadata.attrlist.as_ref().and_then(|a| a.block_style()) == Some("literal");

        // A simple block may be parsed speculatively inside the `!is_literal`
        // branch below (to detect the "metadata with no block" edge case). When
        // that speculative parse succeeds it is reused as the final result rather
        // than re-parsed, so that the captioning side effect of
        // `SimpleBlock::parse` (which can consume a caption counter) happens at
        // most once per block.
        let mut simple_block_mi = None;

        if !is_literal {
            if let Some(mut adm_maw) = AdmonitionBlock::parse(&metadata, parser)
                && let Some(adm) = adm_maw.item
            {
                if !adm_maw.warnings.is_empty() {
                    warnings.append(&mut adm_maw.warnings);
                }

                let block = Self::Admonition(adm.item);

                Self::register_block_id(
                    block.id(),
                    block.title(),
                    block.span(),
                    parser,
                    &mut warnings,
                );

                return MatchAndWarnings {
                    item: Some(MatchedItem {
                        item: block,
                        after: adm.after,
                    }),
                    warnings,
                };
            }

            if let Some(mut quote_maw) = QuoteBlock::parse(&metadata, parser)
                && let Some(quote) = quote_maw.item
            {
                if !quote_maw.warnings.is_empty() {
                    warnings.append(&mut quote_maw.warnings);
                }

                let block = Self::Quote(quote.item);

                Self::register_block_id(
                    block.id(),
                    block.title(),
                    block.span(),
                    parser,
                    &mut warnings,
                );

                return MatchAndWarnings {
                    item: Some(MatchedItem {
                        item: block,
                        after: quote.after,
                    }),
                    warnings,
                };
            }

            if let Some(mut rdb_maw) = RawDelimitedBlock::parse(&metadata, parser)
                && let Some(rdb) = rdb_maw.item
            {
                if !rdb_maw.warnings.is_empty() {
                    warnings.append(&mut rdb_maw.warnings);
                }

                let block = Self::RawDelimited(rdb.item);

                Self::register_block_id(
                    block.id(),
                    block.title(),
                    block.span(),
                    parser,
                    &mut warnings,
                );

                return MatchAndWarnings {
                    item: Some(MatchedItem {
                        item: block,
                        after: rdb.after,
                    }),
                    warnings,
                };
            }

            if let Some(mut cdb_maw) = CompoundDelimitedBlock::parse(&metadata, parser)
                && let Some(cdb) = cdb_maw.item
            {
                if !cdb_maw.warnings.is_empty() {
                    warnings.append(&mut cdb_maw.warnings);
                }

                let block = Self::CompoundDelimited(cdb.item);

                Self::register_block_id(
                    block.id(),
                    block.title(),
                    block.span(),
                    parser,
                    &mut warnings,
                );

                return MatchAndWarnings {
                    item: Some(MatchedItem {
                        item: block,
                        after: cdb.after,
                    }),
                    warnings,
                };
            }

            if let Some(mut table_maw) = TableBlock::parse(&metadata, parser)
                && let Some(table) = table_maw.item
            {
                if !table_maw.warnings.is_empty() {
                    warnings.append(&mut table_maw.warnings);
                }

                let block = Self::Table(table.item);

                Self::register_block_id(
                    block.id(),
                    block.title(),
                    block.span(),
                    parser,
                    &mut warnings,
                );

                return MatchAndWarnings {
                    item: Some(MatchedItem {
                        item: block,
                        after: table.after,
                    }),
                    warnings,
                };
            }

            // Try to discern the block type by scanning the first line.
            let line = metadata.block_start.take_normalized_line();

            if line.item.starts_with("image::")
                || line.item.starts_with("video::")
                || line.item.starts_with("video::")
            {
                let mut media_block_maw = MediaBlock::parse(&metadata, parser);

                if let Some(media_block) = media_block_maw.item {
                    // Only propagate warnings from media block parsing if we think this
                    // *is* a media block. Otherwise, there would likely be too many false
                    // positives.
                    if !media_block_maw.warnings.is_empty() {
                        warnings.append(&mut media_block_maw.warnings);
                    }

                    let block = Self::Media(media_block.item);

                    Self::register_block_id(
                        block.id(),
                        block.title(),
                        block.span(),
                        parser,
                        &mut warnings,
                    );

                    return MatchAndWarnings {
                        item: Some(MatchedItem {
                            item: block,
                            after: media_block.after,
                        }),
                        warnings,
                    };
                }

                // This might be some other kind of block, so we don't
                // automatically error out on a parse failure.
            }

            if (line.item.starts_with('=') || line.item.starts_with('#'))
                && let Some(mi_section_block) =
                    SectionBlock::parse(&metadata, parser, &mut warnings)
            {
                // A line starting with `=` or `#` might be some other kind of block, so we
                // continue quietly if `SectionBlock` parser rejects this block.

                return MatchAndWarnings {
                    item: Some(MatchedItem {
                        item: Self::Section(mi_section_block.item),
                        after: mi_section_block.after,
                    }),
                    warnings,
                };
            }

            if (line.item.starts_with('\'')
                || line.item.starts_with('-')
                || line.item.starts_with('*')
                || line.item.starts_with('<'))
                && let Some(mi_break) = Break::parse(&metadata, parser)
            {
                // Continue quietly if `Break` parser rejects this block.

                return MatchAndWarnings {
                    item: Some(MatchedItem {
                        item: Self::Break(mi_break.item),
                        after: mi_break.after,
                    }),
                    warnings,
                };
            }

            // Only try to parse as a new list if we're NOT inside a list item context.
            // If we are inside a list context, lists can only be created when the first
            // line is a list item marker (handled above).
            if parent_list_markers.is_none()
                && let Some(mi_list) = ListBlock::parse(&metadata, parser, &mut warnings)
            {
                return MatchAndWarnings {
                    item: Some(MatchedItem {
                        item: Self::List(mi_list.item),
                        after: mi_list.after,
                    }),
                    warnings,
                };
            }

            // First, let's look for a fun edge case. Perhaps the text contains block
            // metadata but no block immediately following. If we're not careful, we could
            // spin in a loop (for example, `parse_blocks_until`) thinking there will be
            // another block, but there isn't.

            // The following check disables that spin loop.
            simple_block_mi = if let Some(plm) = parent_list_markers {
                SimpleBlock::parse_for_list_item(&metadata, parser, is_continuation, plm)
            } else {
                SimpleBlock::parse(&metadata, parser)
            };

            if simple_block_mi.is_none() && !metadata.is_empty() {
                // We have a metadata with no block. Treat it as a simple block but issue a
                // warning.

                warnings.push(Warning {
                    source: metadata.source,
                    warning: WarningType::MissingBlockAfterTitleOrAttributeList,
                });

                // Remove the metadata content so that SimpleBlock will read the title/attrlist
                // line(s) as regular content. The speculative parse failed, so the
                // block is re-parsed below with this stripped metadata.
                metadata.title_source = None;
                metadata.title = None;
                metadata.anchor = None;
                metadata.attrlist = None;
                metadata.block_start = metadata.source;
            }
        }

        // If no other block kind matches, we can always use SimpleBlock. Reuse the
        // speculative parse from the `!is_literal` branch when it succeeded;
        // otherwise (a literal block, or metadata stripped above) parse now.
        let simple_block_mi = match simple_block_mi {
            Some(mi) => Some(mi),
            None => {
                if let Some(plm) = parent_list_markers {
                    SimpleBlock::parse_for_list_item(&metadata, parser, is_continuation, plm)
                } else {
                    SimpleBlock::parse(&metadata, parser)
                }
            }
        };

        let mut result = MatchAndWarnings {
            item: simple_block_mi.map(|mi| MatchedItem {
                item: Self::Simple(mi.item),
                after: mi.after,
            }),
            warnings,
        };

        if let Some(ref matched_item) = result.item {
            Self::register_block_id(
                matched_item.item.id(),
                matched_item.item.title(),
                matched_item.item.span(),
                parser,
                &mut result.warnings,
            );
        }

        result
    }

    /// Register a block's ID with the catalog if the block has an ID.
    ///
    /// This should be called for all block types except `SectionBlock`,
    /// which handles its own catalog registration.
    fn register_block_id(
        id: Option<&str>,
        title: Option<&str>,
        span: Span<'src>,
        parser: &mut Parser,
        warnings: &mut Vec<Warning<'src>>,
    ) {
        if let Some(id) = id
            && let Err(_duplicate_error) = parser.register_ref(
                id,
                title, // Use block title as reftext if available
                RefType::Anchor,
            )
        {
            // If registration fails due to duplicate ID, issue a warning.
            warnings.push(Warning {
                source: span,
                warning: WarningType::DuplicateId(id.to_string()),
            });
        }
    }

    /// Returns a reference to the inner [`ListItem`] if this is a
    /// `Block::ListItem`, or `None` otherwise.
    pub(crate) fn as_list_item(&self) -> Option<&ListItem<'src>> {
        match self {
            Self::ListItem(li) => Some(li),
            _ => None,
        }
    }

    /// Resolve any deferred cross-references in this block and its descendants,
    /// using `resolver` to map targets to destinations and `renderer` to render
    /// the resulting links. Unresolved targets are reported in `warnings`.
    ///
    /// This drives the recursion uniformly via the [`IsBlock::content_mut`] and
    /// [`IsBlock::nested_blocks_mut`] accessors, so it needs no per-block-type
    /// special casing.
    pub(crate) fn resolve_references(
        &mut self,
        resolver: &dyn ReferenceResolver,
        renderer: &dyn InlineSubstitutionRenderer,
        warnings: &mut Vec<ReferenceWarning>,
    ) {
        if let Some(content) = self.content_mut() {
            content.resolve_references(resolver, renderer, warnings);
        }

        // Tables hold their resolvable content in cells rather than in a single
        // `content_mut()` value, so they are resolved explicitly here.
        if let Self::Table(table) = self {
            table.resolve_references(resolver, renderer, warnings);
        }

        // A Markdown-style blockquote holds its nested blocks in its own owned
        // source, which the generic `nested_blocks_mut()` walk below does not
        // reach, so they are resolved explicitly here.
        if let Self::Quote(quote) = self {
            quote.resolve_references(resolver, renderer, warnings);
        }

        for child in self.nested_blocks_mut() {
            child.resolve_references(resolver, renderer, warnings);
        }
    }
}

impl<'src> IsBlock<'src> for Block<'src> {
    fn content_model(&self) -> ContentModel {
        match self {
            Self::Simple(_) => ContentModel::Simple,
            Self::Media(b) => b.content_model(),
            Self::Section(_) => ContentModel::Compound,
            Self::List(b) => b.content_model(),
            Self::ListItem(b) => b.content_model(),
            Self::RawDelimited(b) => b.content_model(),
            Self::CompoundDelimited(b) => b.content_model(),
            Self::Admonition(b) => b.content_model(),
            Self::Quote(b) => b.content_model(),
            Self::Table(b) => b.content_model(),
            Self::Preamble(b) => b.content_model(),
            Self::Break(b) => b.content_model(),
            Self::DocumentAttribute(b) => b.content_model(),
        }
    }

    fn declared_style(&'src self) -> Option<&'src str> {
        match self {
            Self::Simple(b) => b.declared_style(),
            Self::Media(b) => b.declared_style(),
            Self::Section(b) => b.declared_style(),
            Self::List(b) => b.declared_style(),
            Self::ListItem(b) => b.declared_style(),
            Self::RawDelimited(b) => b.declared_style(),
            Self::CompoundDelimited(b) => b.declared_style(),
            Self::Admonition(b) => b.declared_style(),
            Self::Quote(b) => b.declared_style(),
            Self::Table(b) => b.declared_style(),
            Self::Preamble(b) => b.declared_style(),
            Self::Break(b) => b.declared_style(),
            Self::DocumentAttribute(b) => b.declared_style(),
        }
    }

    fn rendered_content(&'src self) -> Option<&'src str> {
        match self {
            Self::Simple(b) => b.rendered_content(),
            Self::Media(b) => b.rendered_content(),
            Self::Section(b) => b.rendered_content(),
            Self::List(b) => b.rendered_content(),
            Self::ListItem(b) => b.rendered_content(),
            Self::RawDelimited(b) => b.rendered_content(),
            Self::CompoundDelimited(b) => b.rendered_content(),
            Self::Admonition(b) => b.rendered_content(),
            Self::Quote(b) => b.rendered_content(),
            Self::Table(b) => b.rendered_content(),
            Self::Preamble(b) => b.rendered_content(),
            Self::Break(b) => b.rendered_content(),
            Self::DocumentAttribute(b) => b.rendered_content(),
        }
    }

    fn raw_context(&self) -> CowStr<'src> {
        match self {
            Self::Simple(b) => b.raw_context(),
            Self::Media(b) => b.raw_context(),
            Self::Section(b) => b.raw_context(),
            Self::List(b) => b.raw_context(),
            Self::ListItem(b) => b.raw_context(),
            Self::RawDelimited(b) => b.raw_context(),
            Self::CompoundDelimited(b) => b.raw_context(),
            Self::Admonition(b) => b.raw_context(),
            Self::Quote(b) => b.raw_context(),
            Self::Table(b) => b.raw_context(),
            Self::Preamble(b) => b.raw_context(),
            Self::Break(b) => b.raw_context(),
            Self::DocumentAttribute(b) => b.raw_context(),
        }
    }

    fn nested_blocks(&'src self) -> Iter<'src, Block<'src>> {
        match self {
            Self::Simple(b) => b.nested_blocks(),
            Self::Media(b) => b.nested_blocks(),
            Self::Section(b) => b.nested_blocks(),
            Self::List(b) => b.nested_blocks(),
            Self::ListItem(b) => b.nested_blocks(),
            Self::RawDelimited(b) => b.nested_blocks(),
            Self::CompoundDelimited(b) => b.nested_blocks(),
            Self::Admonition(b) => b.nested_blocks(),
            Self::Quote(b) => b.nested_blocks(),
            Self::Table(b) => b.nested_blocks(),
            Self::Preamble(b) => b.nested_blocks(),
            Self::Break(b) => b.nested_blocks(),
            Self::DocumentAttribute(b) => b.nested_blocks(),
        }
    }

    fn nested_blocks_mut(&mut self) -> &mut [Block<'src>] {
        match self {
            Self::Simple(b) => b.nested_blocks_mut(),
            Self::Media(b) => b.nested_blocks_mut(),
            Self::Section(b) => b.nested_blocks_mut(),
            Self::List(b) => b.nested_blocks_mut(),
            Self::ListItem(b) => b.nested_blocks_mut(),
            Self::RawDelimited(b) => b.nested_blocks_mut(),
            Self::CompoundDelimited(b) => b.nested_blocks_mut(),
            Self::Admonition(b) => b.nested_blocks_mut(),
            Self::Quote(b) => b.nested_blocks_mut(),
            Self::Table(b) => b.nested_blocks_mut(),
            Self::Preamble(b) => b.nested_blocks_mut(),
            Self::Break(b) => b.nested_blocks_mut(),
            Self::DocumentAttribute(b) => b.nested_blocks_mut(),
        }
    }

    fn content_mut(&mut self) -> Option<&mut Content<'src>> {
        match self {
            Self::Simple(b) => b.content_mut(),
            Self::Media(b) => b.content_mut(),
            Self::Section(b) => b.content_mut(),
            Self::List(b) => b.content_mut(),
            Self::ListItem(b) => b.content_mut(),
            Self::RawDelimited(b) => b.content_mut(),
            Self::CompoundDelimited(b) => b.content_mut(),
            Self::Admonition(b) => b.content_mut(),
            Self::Quote(b) => b.content_mut(),
            Self::Table(b) => b.content_mut(),
            Self::Preamble(b) => b.content_mut(),
            Self::Break(b) => b.content_mut(),
            Self::DocumentAttribute(b) => b.content_mut(),
        }
    }

    fn title_source(&'src self) -> Option<Span<'src>> {
        match self {
            Self::Simple(b) => b.title_source(),
            Self::Media(b) => b.title_source(),
            Self::Section(b) => b.title_source(),
            Self::List(b) => b.title_source(),
            Self::ListItem(b) => b.title_source(),
            Self::RawDelimited(b) => b.title_source(),
            Self::CompoundDelimited(b) => b.title_source(),
            Self::Admonition(b) => b.title_source(),
            Self::Quote(b) => b.title_source(),
            Self::Table(b) => b.title_source(),
            Self::Preamble(b) => b.title_source(),
            Self::Break(b) => b.title_source(),
            Self::DocumentAttribute(b) => b.title_source(),
        }
    }

    fn title(&self) -> Option<&str> {
        match self {
            Self::Simple(b) => b.title(),
            Self::Media(b) => b.title(),
            Self::Section(b) => b.title(),
            Self::List(b) => b.title(),
            Self::ListItem(b) => b.title(),
            Self::RawDelimited(b) => b.title(),
            Self::CompoundDelimited(b) => b.title(),
            Self::Admonition(b) => b.title(),
            Self::Quote(b) => b.title(),
            Self::Table(b) => b.title(),
            Self::Preamble(b) => b.title(),
            Self::Break(b) => b.title(),
            Self::DocumentAttribute(b) => b.title(),
        }
    }

    fn caption(&self) -> Option<&str> {
        match self {
            Self::Simple(b) => b.caption(),
            Self::Media(b) => b.caption(),
            Self::Section(b) => b.caption(),
            Self::List(b) => b.caption(),
            Self::ListItem(b) => b.caption(),
            Self::RawDelimited(b) => b.caption(),
            Self::CompoundDelimited(b) => b.caption(),
            Self::Admonition(b) => b.caption(),
            Self::Quote(b) => b.caption(),
            Self::Table(b) => b.caption(),
            Self::Preamble(b) => b.caption(),
            Self::Break(b) => b.caption(),
            Self::DocumentAttribute(b) => b.caption(),
        }
    }

    fn number(&self) -> Option<usize> {
        match self {
            Self::Simple(b) => b.number(),
            Self::Media(b) => b.number(),
            Self::Section(b) => b.number(),
            Self::List(b) => b.number(),
            Self::ListItem(b) => b.number(),
            Self::RawDelimited(b) => b.number(),
            Self::CompoundDelimited(b) => b.number(),
            Self::Admonition(b) => b.number(),
            Self::Quote(b) => b.number(),
            Self::Table(b) => b.number(),
            Self::Preamble(b) => b.number(),
            Self::Break(b) => b.number(),
            Self::DocumentAttribute(b) => b.number(),
        }
    }

    fn anchor(&'src self) -> Option<Span<'src>> {
        match self {
            Self::Simple(b) => b.anchor(),
            Self::Media(b) => b.anchor(),
            Self::Section(b) => b.anchor(),
            Self::List(b) => b.anchor(),
            Self::ListItem(b) => b.anchor(),
            Self::RawDelimited(b) => b.anchor(),
            Self::CompoundDelimited(b) => b.anchor(),
            Self::Admonition(b) => b.anchor(),
            Self::Quote(b) => b.anchor(),
            Self::Table(b) => b.anchor(),
            Self::Preamble(b) => b.anchor(),
            Self::Break(b) => b.anchor(),
            Self::DocumentAttribute(b) => b.anchor(),
        }
    }

    fn anchor_reftext(&'src self) -> Option<Span<'src>> {
        match self {
            Self::Simple(b) => b.anchor_reftext(),
            Self::Media(b) => b.anchor_reftext(),
            Self::Section(b) => b.anchor_reftext(),
            Self::List(b) => b.anchor_reftext(),
            Self::ListItem(b) => b.anchor_reftext(),
            Self::RawDelimited(b) => b.anchor_reftext(),
            Self::CompoundDelimited(b) => b.anchor_reftext(),
            Self::Admonition(b) => b.anchor_reftext(),
            Self::Quote(b) => b.anchor_reftext(),
            Self::Table(b) => b.anchor_reftext(),
            Self::Preamble(b) => b.anchor_reftext(),
            Self::Break(b) => b.anchor_reftext(),
            Self::DocumentAttribute(b) => b.anchor_reftext(),
        }
    }

    fn attrlist(&'src self) -> Option<&'src Attrlist<'src>> {
        match self {
            Self::Simple(b) => b.attrlist(),
            Self::Media(b) => b.attrlist(),
            Self::Section(b) => b.attrlist(),
            Self::List(b) => b.attrlist(),
            Self::ListItem(b) => b.attrlist(),
            Self::RawDelimited(b) => b.attrlist(),
            Self::CompoundDelimited(b) => b.attrlist(),
            Self::Admonition(b) => b.attrlist(),
            Self::Quote(b) => b.attrlist(),
            Self::Table(b) => b.attrlist(),
            Self::Preamble(b) => b.attrlist(),
            Self::Break(b) => b.attrlist(),
            Self::DocumentAttribute(b) => b.attrlist(),
        }
    }

    fn substitution_group(&self) -> SubstitutionGroup {
        match self {
            Self::Simple(b) => b.substitution_group(),
            Self::Media(b) => b.substitution_group(),
            Self::Section(b) => b.substitution_group(),
            Self::List(b) => b.substitution_group(),
            Self::ListItem(b) => b.substitution_group(),
            Self::RawDelimited(b) => b.substitution_group(),
            Self::CompoundDelimited(b) => b.substitution_group(),
            Self::Admonition(b) => b.substitution_group(),
            Self::Quote(b) => b.substitution_group(),
            Self::Table(b) => b.substitution_group(),
            Self::Preamble(b) => b.substitution_group(),
            Self::Break(b) => b.substitution_group(),
            Self::DocumentAttribute(b) => b.substitution_group(),
        }
    }
}

impl<'src> HasSpan<'src> for Block<'src> {
    fn span(&self) -> Span<'src> {
        match self {
            Self::Simple(b) => b.span(),
            Self::Media(b) => b.span(),
            Self::Section(b) => b.span(),
            Self::List(b) => b.span(),
            Self::ListItem(b) => b.span(),
            Self::RawDelimited(b) => b.span(),
            Self::CompoundDelimited(b) => b.span(),
            Self::Admonition(b) => b.span(),
            Self::Quote(b) => b.span(),
            Self::Table(b) => b.span(),
            Self::Preamble(b) => b.span(),
            Self::Break(b) => b.span(),
            Self::DocumentAttribute(b) => b.span(),
        }
    }
}
