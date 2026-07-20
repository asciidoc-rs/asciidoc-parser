use std::slice::Iter;

use crate::{
    HasSpan, Parser, Span,
    attributes::Attrlist,
    blocks::{
        AdmonitionBlock, Break, CompoundDelimitedBlock, ContentModel, IsBlock, ListBlock, ListItem,
        ListItemMarker, MediaBlock, Preamble, QuoteBlock, RawDelimitedBlock, SectionBlock,
        SimpleBlock, TableBlock, media::TargetResolution, metadata::BlockMetadata,
        starts_with_admonition_label,
    },
    content::{Content, SubstitutionGroup, substitute_attributes_in_reftext},
    document::{Attribute, InterpretedValue, RefType},
    parser::{InlineSubstitutionRenderer, ReferenceResolver, ReferenceWarnings, XrefSignifier},
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

/// Outcome of attempting to parse a single [`Block`].
///
/// Most blocks parse to [`Parsed`](Self::Parsed). [`Dropped`](Self::Dropped)
/// supports `attribute-missing=drop-line`: when a block-macro target references
/// a missing attribute, Asciidoctor discards the whole block, which the parser
/// must distinguish both from a successful parse and from "no block matched"
/// (so the block-collection loops advance past the dropped source rather than
/// spinning or mis-parsing it).
// `Parsed` embeds a `Block`, which is itself a large enum (see the matching
// allow on `Block`). This outcome is short-lived and returned by value on the
// hot parse path, so boxing it would just trade the size for an allocation.
#[allow(clippy::large_enum_variant)]
pub(crate) enum BlockParseOutcome<'src> {
    /// A block was parsed.
    Parsed(MatchedItem<'src, Block<'src>>),

    /// The input was recognized as a block macro but dropped at parse time
    /// because its target referenced a missing attribute under
    /// `attribute-missing=drop-line`. The contained span is where parsing
    /// should resume (the dropped block's `after`).
    Dropped(Span<'src>),

    /// No block matched. This happens only for empty or all-blank input.
    NoMatch,
}

impl<'src> Block<'src> {
    /// Parse a block of any type and return a `Block` that describes it.
    ///
    /// Consumes any blank lines before and after the block.
    ///
    /// This is a test-only convenience wrapper over
    /// [`parse_with_outcome`](Self::parse_with_outcome) that flattens the
    /// drop-line outcome to an `Option`; production code uses
    /// `parse_with_outcome` so it can react to a dropped block.
    #[cfg(test)]
    pub(crate) fn parse(
        source: Span<'src>,
        parser: &mut Parser,
    ) -> MatchAndWarnings<'src, Option<MatchedItem<'src, Self>>> {
        let MatchAndWarnings { item, warnings } = Self::parse_internal(source, parser, None, false);

        MatchAndWarnings {
            item: match item {
                BlockParseOutcome::Parsed(mi) => Some(mi),
                BlockParseOutcome::Dropped(_) | BlockParseOutcome::NoMatch => None,
            },
            warnings,
        }
    }

    /// Parse a block of any type, returning the full [`BlockParseOutcome`] so a
    /// block-collection loop can advance past a block that was dropped at parse
    /// time (`attribute-missing=drop-line`). Consumes any blank lines before
    /// and after the block.
    ///
    /// This is the entry point used by production block-collection loops.
    pub(crate) fn parse_with_outcome(
        source: Span<'src>,
        parser: &mut Parser,
    ) -> MatchAndWarnings<'src, BlockParseOutcome<'src>> {
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
    ) -> MatchAndWarnings<'src, BlockParseOutcome<'src>> {
        Self::parse_internal(source, parser, Some(parent_list_markers), is_continuation)
    }

    /// Shared parser for [`parse_with_outcome`](Self::parse_with_outcome) and
    /// [`parse_for_list_item`](Self::parse_for_list_item).
    fn parse_internal(
        source: Span<'src>,
        parser: &mut Parser,
        parent_list_markers: Option<&[ListItemMarker<'src>]>,
        is_continuation: bool,
    ) -> MatchAndWarnings<'src, BlockParseOutcome<'src>> {
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
                    | '`'
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
            && parser.pending_block_title.is_none()
            && let Some(MatchedItem {
                item: simple_block,
                after,
            }) = SimpleBlock::parse_fast(source, parser)
        {
            let mut warnings = vec![];
            let block = Self::Simple(simple_block);

            Self::register_block_id(
                block.id(),
                Self::block_reftext(&block, parser).as_deref(),
                Self::block_signifier(&block, parser),
                block.span(),
                parser,
                &mut warnings,
            );

            return MatchAndWarnings {
                item: BlockParseOutcome::Parsed(MatchedItem { item: block, after }),
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
                item: BlockParseOutcome::Parsed(MatchedItem {
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

        // A block title stashed by an enclosing section heading (see
        // `SectionBlock::parse`) is claimed by the next block parsed — this
        // one. A title of the block's own wins, discarding the carried title.
        // The carried title has no source line adjacent to this block, so
        // `title_source` stays `None` (the same shape as a `title=` attribute).
        if let Some(pending_title) = parser.pending_block_title.take()
            && metadata.title.is_none()
        {
            // The carried title arrives as an owned snapshot; rebuild it as a
            // `Content` anchored at the block's start, restoring any deferred
            // cross-references so the title pass can still resolve them.
            metadata.title = Some(crate::content::Content::from_owned_title(
                metadata.block_start,
                pending_title,
            ));
        }

        // Tolerate a blank line between a block's metadata (title, anchor, or
        // attribute list) and the block it decorates. Asciidoctor's
        // `parse_block_metadata_lines` skips blank lines after each metadata
        // line, so metadata separated from its block by one or more blank lines
        // still attaches to that block rather than dangling as a spurious
        // `MissingBlockAfterTitleOrAttributeList`. Advancing `block_start` past
        // the gap lets the block-type dispatch below see the content directly.
        //
        // This applies at the block level only. Inside a list item,
        // blank-separated metadata follows the list-continuation rules handled
        // in `ListItem::parse` (where such metadata is discarded), so leave
        // `block_start` pointing at the blank line for those callers. Likewise,
        // if only blank lines follow (no block content), leave it untouched so
        // the genuinely-dangling-metadata warning still fires.
        if parent_list_markers.is_none() && !metadata.is_empty() {
            let after_blanks = metadata.block_start.discard_empty_lines();
            if after_blanks != metadata.block_start && !after_blanks.is_empty() {
                metadata.block_start = after_blanks;
            }
        }

        // The `[literal]` block style normally marks a literal *paragraph*,
        // which is handled directly as a simple (literal) block below, bypassing
        // the delimited-block parsers. The exception is when `[literal]` is set
        // on the delimiter line of a structural container, where it masquerades
        // over that container (e.g. `[literal]` on a `----` listing, on a `....`
        // literal, or on a `--` open block); those cases must fall through to the
        // delimited-block parsers.
        let is_literal =
            metadata.attrlist.as_ref().and_then(|a| a.block_style()) == Some("literal") && {
                let first_line = metadata.block_start.take_normalized_line().item;
                !RawDelimitedBlock::is_valid_delimiter(&first_line)
                    && !CompoundDelimitedBlock::is_valid_delimiter(&first_line)
                    && !TableBlock::is_table_delimiter(&first_line)
            };

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
                    Self::block_reftext(&block, parser).as_deref(),
                    Self::block_signifier(&block, parser),
                    block.span(),
                    parser,
                    &mut warnings,
                );

                return MatchAndWarnings {
                    item: BlockParseOutcome::Parsed(MatchedItem {
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
                    Self::block_reftext(&block, parser).as_deref(),
                    Self::block_signifier(&block, parser),
                    block.span(),
                    parser,
                    &mut warnings,
                );

                return MatchAndWarnings {
                    item: BlockParseOutcome::Parsed(MatchedItem {
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
                    Self::block_reftext(&block, parser).as_deref(),
                    Self::block_signifier(&block, parser),
                    block.span(),
                    parser,
                    &mut warnings,
                );

                return MatchAndWarnings {
                    item: BlockParseOutcome::Parsed(MatchedItem {
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
                    Self::block_reftext(&block, parser).as_deref(),
                    Self::block_signifier(&block, parser),
                    block.span(),
                    parser,
                    &mut warnings,
                );

                return MatchAndWarnings {
                    item: BlockParseOutcome::Parsed(MatchedItem {
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
                    Self::block_reftext(&block, parser).as_deref(),
                    Self::block_signifier(&block, parser),
                    block.span(),
                    parser,
                    &mut warnings,
                );

                return MatchAndWarnings {
                    item: BlockParseOutcome::Parsed(MatchedItem {
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
                || line.item.starts_with("audio::")
            {
                let mut media_block_maw = MediaBlock::parse(&metadata, parser);

                if let Some(mut media_block) = media_block_maw.item {
                    // Only propagate warnings from media block parsing if we think this
                    // *is* a media block. Otherwise, there would likely be too many false
                    // positives.
                    if !media_block_maw.warnings.is_empty() {
                        warnings.append(&mut media_block_maw.warnings);
                    }

                    // Resolve attribute references in the macro target. Under
                    // `attribute-missing=drop-line`, a reference to a missing
                    // attribute drops the entire block (Asciidoctor behavior).
                    if media_block.item.resolve_target(parser) == TargetResolution::Drop {
                        return MatchAndWarnings {
                            item: BlockParseOutcome::Dropped(media_block.after),
                            warnings,
                        };
                    }

                    // Assign the caption only now that the block has survived
                    // `resolve_target`, so a dropped image does not consume the
                    // `figure-number` counter and leave a gap in the numbering.
                    media_block.item.assign_caption(parser);

                    let block = Self::Media(media_block.item);

                    Self::register_block_id(
                        block.id(),
                        Self::block_reftext(&block, parser).as_deref(),
                        Self::block_signifier(&block, parser),
                        block.span(),
                        parser,
                        &mut warnings,
                    );

                    return MatchAndWarnings {
                        item: BlockParseOutcome::Parsed(MatchedItem {
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
                    item: BlockParseOutcome::Parsed(MatchedItem {
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
                    item: BlockParseOutcome::Parsed(MatchedItem {
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
                    item: BlockParseOutcome::Parsed(MatchedItem {
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
                    origin: None,
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
            item: match simple_block_mi {
                Some(mi) => BlockParseOutcome::Parsed(MatchedItem {
                    item: Self::Simple(mi.item),
                    after: mi.after,
                }),
                None => BlockParseOutcome::NoMatch,
            },
            warnings,
        };

        if let BlockParseOutcome::Parsed(ref matched_item) = result.item {
            Self::register_block_id(
                matched_item.item.id(),
                Self::block_reftext(&matched_item.item, parser).as_deref(),
                Self::block_signifier(&matched_item.item, parser),
                matched_item.item.span(),
                parser,
                &mut result.warnings,
            );
        }

        result
    }

    /// Determine the [`XrefSignifier`] a cross-reference uses to build
    /// `full`/`short` [`xrefstyle`](crate::parser::XrefStyle) text when this
    /// block is the target.
    ///
    /// A signifier is produced only for an auto-numbered captioned block (e.g.
    /// an image → "Figure 1", a titled table → "Table 1") that has no explicit
    /// reftext. A block with an explicit `reftext` attribute or a
    /// `[[id,reftext]]` anchor reftext uses that text verbatim, so it gets no
    /// signifier; neither does an uncaptioned block or one whose caption was
    /// overridden with `[caption=...]` (which is not numbered).
    fn block_signifier<'a>(block: &'a Block<'a>, parser: &Parser) -> Option<XrefSignifier> {
        // Only captioned blocks are eligible.
        let caption = block.caption()?;

        let has_explicit_reftext = block
            .attrlist()
            .and_then(|attrlist| attrlist.named_attribute("reftext"))
            .is_some()
            || block.anchor_reftext().is_some();
        if has_explicit_reftext {
            return None;
        }

        // Exclude explicit caption overrides, which are not numbered. This is
        // *not* the same as `block.number().is_none()`: an auto-numbered block
        // whose context counter holds a non-integer value (e.g. `:figure-number:
        // A`, rendering "Figure B") also has no bare integer number, yet it is
        // genuinely numbered and must keep its signifier ("Figure B").
        if Self::has_caption_override(block, parser) {
            return None;
        }

        // The caption prefix is "<label> <n>. "; the xrefstyle label is that
        // prefix without its trailing ". " separator (e.g. "Figure 1").
        let label = caption.strip_suffix(". ").unwrap_or(caption).to_string();
        Some(XrefSignifier {
            label,
            emphasize: false,
        })
    }

    /// Whether a captioned block's caption comes from an explicit override
    /// rather than automatic numbering.
    ///
    /// An override is a `caption` attribute on the block (or, for an image, on
    /// the image macro), or a non-empty document-wide `caption` attribute. This
    /// mirrors the override detection in
    /// [`caption::assign_block_caption`](crate::blocks::caption) and
    /// [`MediaBlock::assign_caption`], so the two agree on which blocks are
    /// numbered.
    fn has_caption_override<'a>(block: &'a Block<'a>, parser: &Parser) -> bool {
        let attribute_override = block
            .attrlist()
            .and_then(|attrlist| attrlist.named_attribute("caption"))
            .is_some()
            || matches!(block, Block::Media(media)
                if media.macro_attrlist().named_attribute("caption").is_some());

        attribute_override
            || matches!(
                parser.attribute_value("caption"),
                InterpretedValue::Value(value) if !value.is_empty(),
            )
    }

    /// Determine the reftext (a.k.a. xreflabel) used as the link text when a
    /// block is the target of a cross reference. Asciidoctor's precedence is:
    /// an explicit `reftext` attribute, then the reftext supplied with a
    /// block anchor (`[[id,reftext]]`), and finally the block title.
    ///
    /// Attribute references in a `[[id,reftext]]` anchor reftext are resolved
    /// against the attributes in effect when the block is registered, matching
    /// how the anchor ID and a `reftext=` attribute (both substituted when the
    /// attribute list is parsed) are handled. The `reftext=` and title branches
    /// are already substituted, so only the anchor branch is resolved here.
    fn block_reftext<'a>(block: &'a Block<'a>, parser: &Parser) -> Option<CowStr<'a>> {
        if let Some(attr) = block
            .attrlist()
            .and_then(|attrlist| attrlist.named_attribute("reftext"))
        {
            return Some(CowStr::from(attr.value()));
        }

        if let Some(span) = block.anchor_reftext() {
            return Some(substitute_attributes_in_reftext(span, parser));
        }

        block.title().map(CowStr::from)
    }

    /// Register a block's ID with the catalog if the block has an ID.
    ///
    /// This should be called for all block types except `SectionBlock`,
    /// which handles its own catalog registration.
    fn register_block_id(
        id: Option<&str>,
        reftext: Option<&str>,
        signifier: Option<XrefSignifier>,
        span: Span<'src>,
        parser: &mut Parser,
        warnings: &mut Vec<Warning<'src>>,
    ) {
        if let Some(id) = id {
            match parser.register_ref(id, reftext, RefType::Anchor) {
                Ok(()) => {
                    if let Some(signifier) = signifier {
                        parser.set_ref_signifier(id, signifier);
                    }
                }
                Err(_duplicate_error) => {
                    // If registration fails due to duplicate ID, issue a warning.
                    warnings.push(Warning {
                        source: span,
                        warning: WarningType::DuplicateId(id.to_string()),
                        origin: None,
                    });
                }
            }
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
        warnings: &mut ReferenceWarnings<'src>,
    ) {
        // A section is not resolved here: its resolvable content is its
        // heading, which `content_mut` deliberately does not expose (see
        // `SectionBlock`). Headings are resolved by the document-order title
        // pass (`title_refs::resolve_title_references`), which coordinates
        // cross-references *between* titles (forward and circular) — something
        // per-content resolution cannot see.
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

    /// Returns this block's *block title* (`.Title`) as a mutable [`Content`],
    /// when the block has one.
    ///
    /// This is the decorative title carried above a block, distinct from a
    /// section's heading. Used only by the document-order title resolution
    /// pass, which reads a title's deferred cross-references and installs the
    /// re-rendered title once they are resolved. Blocks that never carry a
    /// title return `None`.
    pub(crate) fn block_title_content_mut(&mut self) -> Option<&mut Content<'src>> {
        match self {
            Self::Simple(b) => b.title_content_mut(),
            Self::Media(b) => b.title_content_mut(),
            Self::List(b) => b.title_content_mut(),
            Self::RawDelimited(b) => b.title_content_mut(),
            Self::CompoundDelimited(b) => b.title_content_mut(),
            Self::Admonition(b) => b.title_content_mut(),
            Self::Quote(b) => b.title_content_mut(),
            Self::Table(b) => b.title_content_mut(),
            Self::Break(b) => b.title_content_mut(),
            _ => None,
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

    fn id(&'src self) -> Option<&'src str> {
        // Two variants override the trait default:
        //
        // * A `MediaBlock` additionally recognizes a named `id=` _inside_ its macro
        //   attribute list (e.g. `image::sunset.jpg[id=sunset-img]`).
        //
        // * A `SectionBlock` falls back to its auto-generated (`_slug`) ID when no
        //   explicit ID was supplied, so `block.id()` yields the same ID the section is
        //   registered and cross-referenced under. Delegating here (rather than
        //   applying the trait default) avoids the footgun of `block.id()` silently
        //   returning `None` for a section that plainly has an ID.
        //
        // Every other variant keeps the trait default (explicit anchor or block
        // attribute list only).
        match self {
            Self::Media(b) => b.id(),
            Self::Section(b) => b.id(),
            _ => self
                .anchor()
                .map(|a| a.data())
                .or_else(|| self.attrlist().and_then(|attrlist| attrlist.id())),
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
