use std::{slice::Iter, sync::Arc};

use self_cell::self_cell;

use crate::{
    HasSpan, Parser, Span,
    attributes::Attrlist,
    blocks::{
        Block, CompoundDelimitedBlock, ContentModel, IsBlock, ListItemMarker, RawDelimitedBlock,
        SimpleBlock, TableBlock, metadata::BlockMetadata, parse_utils::parse_blocks_until,
    },
    content::{Content, SubstitutionGroup},
    internal::debug::DebugSliceReference,
    parser::{InlineSubstitutionRenderer, ReferenceResolver, ReferenceWarning},
    span::MatchedItem,
    strings::CowStr,
    warnings::{MatchAndWarnings, Warning, WarningType},
};

self_cell! {
    /// A Markdown-style blockquote's nested blocks, which borrow the owned
    /// (`>`-stripped) source the block carries.
    pub struct OwnedQuoteBlocks {
        owner: String,

        #[covariant]
        dependent: OwnedQuoteBlocksInner,
    }

    impl {Debug, Eq, PartialEq}
}

/// The parsed blocks of an [`OwnedQuoteBlocks`], borrowing its owned source.
#[derive(Debug, Eq, PartialEq)]
struct OwnedQuoteBlocksInner<'src> {
    blocks: Vec<Block<'src>>,
}

/// Distinguishes the two block types that share the blockquote syntax.
///
/// Prose excerpts and quotes use the [`Quote`](Self::Quote) type, which does
/// not preserve line breaks. Verses (e.g., poems or song lyrics) use the
/// [`Verse`](Self::Verse) type, which preserves line breaks in the output.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum QuoteType {
    /// A prose excerpt or quote. Line breaks are not preserved.
    Quote,

    /// A verse (e.g., a poem or song lyric). Line breaks are preserved.
    Verse,
}

impl QuoteType {
    /// Returns the lowercase name for this type (e.g., `quote`).
    ///
    /// This is the block's context and the basis for its CSS class
    /// (`quoteblock` or `verseblock`).
    pub fn name(self) -> &'static str {
        match self {
            Self::Quote => "quote",
            Self::Verse => "verse",
        }
    }
}

impl std::fmt::Debug for QuoteType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Quote => write!(f, "QuoteType::Quote"),
            Self::Verse => write!(f, "QuoteType::Verse"),
        }
    }
}

/// A blockquote: a quote, prose excerpt, or verse, optionally attributed to a
/// person and a source citation.
///
/// A blockquote can be written in several ways, all of which produce this
/// block:
///
/// * A **delimited block** bounded by lines of four underscores (`____`),
///   optionally with a `[quote,…]` or `[verse,…]` attribute list. A `quote`
///   block has the [`Compound`](ContentModel::Compound) content model (it
///   contains other blocks); a `verse` block has the
///   [`Simple`](ContentModel::Simple) content model and preserves its line
///   breaks.
/// * A **styled paragraph** introduced by a `[quote,…]` or `[verse,…]`
///   attribute list. This produces the [`Simple`](ContentModel::Simple) content
///   model.
/// * A **quoted paragraph**: a paragraph wrapped in double quotes and followed
///   by an attribution line introduced by two hyphens (`-- …`). This produces a
///   `quote` block with the [`Simple`](ContentModel::Simple) content model.
#[derive(Clone, Eq, PartialEq)]
pub struct QuoteBlock<'src> {
    type_: QuoteType,
    content_model: ContentModel,
    content: Option<Content<'src>>,
    blocks: Vec<Block<'src>>,
    markdown_blocks: Option<Arc<OwnedQuoteBlocks>>,
    attribution: Option<String>,
    citetitle: Option<String>,
    source: Span<'src>,
    title_source: Option<Span<'src>>,
    title: Option<String>,
    anchor: Option<Span<'src>>,
    anchor_reftext: Option<Span<'src>>,
    attrlist: Option<Attrlist<'src>>,
}

impl<'src> QuoteBlock<'src> {
    /// Parse a blockquote, if the given metadata and content describe one.
    ///
    /// Returns `None` (without consuming the block) when the content is not a
    /// blockquote, so that the caller can fall through to other block parsers.
    pub(crate) fn parse(
        metadata: &BlockMetadata<'src>,
        parser: &mut Parser,
    ) -> Option<MatchAndWarnings<'src, Option<MatchedItem<'src, Self>>>> {
        let style = metadata.attrlist.as_ref().and_then(|a| a.block_style());
        let styled_type = match style {
            Some("quote") => Some(QuoteType::Quote),
            Some("verse") => Some(QuoteType::Verse),
            _ => None,
        };

        let first_line = metadata.block_start.take_normalized_line().item;
        let is_quote_delimiter = is_quote_verse_delimiter(&first_line);

        // A `[quote]` or `[verse]` style masquerades over a quote-delimited
        // block or a paragraph.
        if let Some(type_) = styled_type {
            if is_quote_delimiter {
                return Some(Self::parse_delimited(metadata, parser, type_));
            }

            // The style is set on some other structural container (example,
            // open, sidebar, listing, literal, passthrough, table). That
            // container keeps its own context and the style is ignored, so this
            // is not a blockquote.
            if RawDelimitedBlock::is_valid_delimiter(&first_line)
                || CompoundDelimitedBlock::is_valid_delimiter(&first_line)
                || TableBlock::is_table_delimiter(&first_line)
            {
                return None;
            }

            return Self::parse_styled_paragraph(metadata, parser, type_);
        }

        // A bare quote-delimited block (no `[quote]`/`[verse]` style) is a quote
        // block. Any other style on it (e.g. an admonition label) is ignored.
        if is_quote_delimiter {
            return Some(Self::parse_delimited(metadata, parser, QuoteType::Quote));
        }

        // A Markdown-style blockquote, introduced by a `>` marker.
        if first_line.data().starts_with('>') {
            return Self::parse_markdown(metadata, parser);
        }

        // A quoted paragraph: text wrapped in double quotes followed by a `-- `
        // attribution line.
        Self::parse_quoted_paragraph(metadata, parser)
    }

    /// Parse a quote- or verse-delimited block (bounded by `____` lines).
    fn parse_delimited(
        metadata: &BlockMetadata<'src>,
        parser: &mut Parser,
        type_: QuoteType,
    ) -> MatchAndWarnings<'src, Option<MatchedItem<'src, Self>>> {
        let delimiter = metadata.block_start.take_normalized_line();

        let mut next = delimiter.after;
        let (closing_delimiter, after) = loop {
            if next.is_empty() {
                break (next, next);
            }

            let line = next.take_normalized_line();
            if line.item.data() == delimiter.item.data() {
                break (line.item, line.after);
            }
            next = line.after;
        };

        let inside_delimiters = delimiter.after.trim_remainder(closing_delimiter);

        let (attribution, citetitle) = extract_attribution(metadata.attrlist.as_ref());

        let (content_model, content, blocks, mut warnings) = match type_ {
            // A quote block contains other blocks.
            QuoteType::Quote => {
                let maw_blocks = parse_blocks_until(inside_delimiters, |_| false, parser);
                (
                    ContentModel::Compound,
                    None,
                    maw_blocks.item.item,
                    maw_blocks.warnings,
                )
            }

            // A verse block preserves its content verbatim (line breaks
            // included), with normal substitutions applied.
            QuoteType::Verse => {
                let content = render_verbatim(inside_delimiters, parser);
                (ContentModel::Simple, Some(content), vec![], vec![])
            }
        };

        let source = metadata
            .source
            .trim_remainder(closing_delimiter.discard_all())
            .trim_trailing_whitespace();

        if closing_delimiter.is_empty() {
            warnings.insert(
                0,
                Warning {
                    source: delimiter.item,
                    warning: WarningType::UnterminatedDelimitedBlock,
                },
            );
        }

        MatchAndWarnings {
            item: Some(MatchedItem {
                item: Self {
                    type_,
                    content_model,
                    content,
                    blocks,
                    markdown_blocks: None,
                    attribution,
                    citetitle,
                    source,
                    title_source: metadata.title_source,
                    title: metadata.title.clone(),
                    anchor: metadata.anchor,
                    anchor_reftext: metadata.anchor_reftext,
                    attrlist: metadata.attrlist.clone(),
                },
                after,
            }),
            warnings,
        }
    }

    /// Parse a `[quote,…]` or `[verse,…]` styled paragraph.
    fn parse_styled_paragraph(
        metadata: &BlockMetadata<'src>,
        parser: &mut Parser,
        type_: QuoteType,
    ) -> Option<MatchAndWarnings<'src, Option<MatchedItem<'src, Self>>>> {
        // The paragraph text is read as a simple block. The `quote`/`verse`
        // block style is not one of the verbatim styles, so the content is
        // parsed as a normal paragraph (normal substitutions, line breaks
        // preserved in the rendered text).
        let inner = SimpleBlock::parse(metadata, parser)?;

        let (attribution, citetitle) = extract_attribution(metadata.attrlist.as_ref());

        let source = metadata
            .source
            .trim_remainder(inner.after)
            .trim_trailing_whitespace();

        Some(MatchAndWarnings {
            item: Some(MatchedItem {
                item: Self {
                    type_,
                    content_model: ContentModel::Simple,
                    content: Some(inner.item.content().clone()),
                    blocks: vec![],
                    markdown_blocks: None,
                    attribution,
                    citetitle,
                    source,
                    title_source: metadata.title_source,
                    title: metadata.title.clone(),
                    anchor: metadata.anchor,
                    anchor_reftext: metadata.anchor_reftext,
                    attrlist: metadata.attrlist.clone(),
                },
                after: inner.after,
            }),
            warnings: vec![],
        })
    }

    /// Parse a quoted paragraph: text wrapped in double quotes followed by a
    /// `-- ` attribution line.
    ///
    /// Returns `None` if the content does not match this shape.
    fn parse_quoted_paragraph(
        metadata: &BlockMetadata<'src>,
        parser: &mut Parser,
    ) -> Option<MatchAndWarnings<'src, Option<MatchedItem<'src, Self>>>> {
        // The paragraph extends to the first blank line.
        let para = read_paragraph(metadata.block_start);
        let data = para.data();

        if !data.starts_with('"') {
            return None;
        }

        // Locate the attribution line: the first line that begins with `--`
        // followed by whitespace and at least one more character. Splits the
        // paragraph into the quoted text and the attribution text.
        let (quoted, attribution_text) = split_at_attribution_line(data)?;

        // The quoted text, with surrounding whitespace removed, must be wrapped
        // in double quotes.
        let inner = quoted
            .trim_end()
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .filter(|inner| !inner.is_empty())?;

        // The quoted text, with the surrounding quotes removed, becomes the
        // blockquote content. Build a span that points just past the opening
        // quote so source positions stay meaningful.
        let inner_span = para.slice(1..1 + inner.len());
        let mut content = Content::from(inner_span);
        SubstitutionGroup::Normal.apply(&mut content, parser, None);

        // The attribution line provides the attribution and (optional) citation.
        let (attribution, citetitle) = split_attribution_line(attribution_text.trim(), parser);

        let source = metadata
            .source
            .trim_remainder(read_paragraph_after(metadata.block_start))
            .trim_trailing_whitespace();

        Some(MatchAndWarnings {
            item: Some(MatchedItem {
                item: Self {
                    type_: QuoteType::Quote,
                    content_model: ContentModel::Simple,
                    content: Some(content),
                    blocks: vec![],
                    markdown_blocks: None,
                    attribution,
                    citetitle,
                    source,
                    title_source: metadata.title_source,
                    title: metadata.title.clone(),
                    anchor: metadata.anchor,
                    anchor_reftext: metadata.anchor_reftext,
                    attrlist: metadata.attrlist.clone(),
                },
                after: read_paragraph_after(metadata.block_start).discard_empty_lines(),
            }),
            warnings: vec![],
        })
    }

    /// Parse a Markdown-style blockquote: a run of lines introduced by a `>`
    /// marker.
    ///
    /// Returns `None` when the content is not a Markdown-style blockquote,
    /// including the case where it is a description list (which takes
    /// precedence, since the `>` marker is a valid description-list term
    /// prefix).
    fn parse_markdown(
        metadata: &BlockMetadata<'src>,
        parser: &mut Parser,
    ) -> Option<MatchAndWarnings<'src, Option<MatchedItem<'src, Self>>>> {
        let first_line = metadata.block_start.take_normalized_line().item;

        // The first line must begin the Markdown blockquote: a `>` followed by a
        // space, or a bare `>`. A `>` immediately followed by other text (e.g.
        // `>foo`) is not a blockquote.
        if !is_markdown_marker_line(first_line.data()) {
            return None;
        }

        // A description list takes precedence over a Markdown-style blockquote:
        // the `>` marker is a valid prefix for a description-list term, so a
        // line like `> term:: def` is a description list, not a blockquote.
        if let Some(MatchedItem {
            item: ListItemMarker::DefinedTerm { .. },
            ..
        }) = ListItemMarker::parse(metadata.block_start, parser)
        {
            return None;
        }

        // The blockquote spans the paragraph: every line up to the first blank
        // line. Within it, each line's `>` marker is stripped (a bare `>`
        // becomes a blank line); a line with no marker is a lazy continuation
        // and is kept as-is.
        let chunk = read_paragraph(metadata.block_start);
        let after = read_paragraph_after(metadata.block_start);

        let mut lines: Vec<String> = chunk
            .data()
            .split('\n')
            .map(|line| {
                if line == ">" {
                    String::new()
                } else if let Some(rest) = line.strip_prefix("> ") {
                    rest.to_string()
                } else {
                    line.to_string()
                }
            })
            .collect();

        // A trailing attribution line (introduced by `--`) is pulled out of the
        // content and used as the attribution and citation.
        let (attribution, citetitle) = take_trailing_attribution(&mut lines, parser);

        let body = lines.join("\n");

        // The stripped body owns its source; its blocks borrow from it.
        let owned = OwnedQuoteBlocks::new(body, |source| {
            // Warnings from the owned parse borrow the owned source and so
            // cannot escape it. Markdown-style blockquotes are used for simple
            // content, so this path is currently warning-free; the
            // `debug_assert` turns any future warning into a loud test failure
            // rather than a silent loss.
            let maw = parse_blocks_until(Span::new(source), |_| false, parser);
            debug_assert!(
                maw.warnings.is_empty(),
                "warnings from a Markdown-style blockquote are dropped; \
                 propagate them before adding any to this path"
            );
            OwnedQuoteBlocksInner {
                blocks: maw.item.item,
            }
        });

        let source = metadata
            .source
            .trim_remainder(after)
            .trim_trailing_whitespace();

        Some(MatchAndWarnings {
            item: Some(MatchedItem {
                item: Self {
                    type_: QuoteType::Quote,
                    content_model: ContentModel::Compound,
                    content: None,
                    blocks: vec![],
                    markdown_blocks: Some(Arc::new(owned)),
                    attribution,
                    citetitle,
                    source,
                    title_source: metadata.title_source,
                    title: metadata.title.clone(),
                    anchor: metadata.anchor,
                    anchor_reftext: metadata.anchor_reftext,
                    attrlist: metadata.attrlist.clone(),
                },
                after: after.discard_empty_lines(),
            }),
            warnings: vec![],
        })
    }

    /// Returns the blockquote type (quote or verse).
    pub fn type_(&self) -> QuoteType {
        self.type_
    }

    /// Returns the rendered attribution (the person the content is attributed
    /// to), if any.
    pub fn attribution(&self) -> Option<&str> {
        self.attribution.as_deref()
    }

    /// Returns the rendered citation title (the work the content is drawn
    /// from), if any.
    pub fn citetitle(&self) -> Option<&str> {
        self.citetitle.as_deref()
    }

    /// Returns the simple content of this blockquote, if it has the
    /// [`Simple`](ContentModel::Simple) content model.
    pub fn content(&self) -> Option<&Content<'src>> {
        self.content.as_ref()
    }

    /// Returns the nested blocks of a compound blockquote.
    ///
    /// Unlike [`nested_blocks()`](IsBlock::nested_blocks), this also returns
    /// the blocks of a Markdown-style blockquote, which borrow the block's
    /// own owned source rather than the document source and so are not
    /// exposed through the `'src`-bound trait method.
    pub fn blocks(&self) -> &[Block<'_>] {
        match &self.markdown_blocks {
            Some(owned) => &owned.borrow_dependent().blocks,
            None => &self.blocks,
        }
    }

    /// Resolves any deferred cross-references in this blockquote's nested
    /// blocks.
    ///
    /// The blocks of a `____`-delimited quote are reached through
    /// [`nested_blocks_mut()`](IsBlock::nested_blocks_mut) by the generic block
    /// walker; this method handles the Markdown-style case, whose blocks borrow
    /// the block's own owned source.
    pub(crate) fn resolve_references(
        &mut self,
        resolver: &dyn ReferenceResolver,
        renderer: &dyn InlineSubstitutionRenderer,
        warnings: &mut Vec<ReferenceWarning>,
    ) {
        // The owned store is shared behind an `Arc`, but references are resolved
        // immediately after parsing while the block is still its sole owner, so
        // `get_mut` succeeds.
        if let Some(owned) = self.markdown_blocks.as_mut()
            && let Some(owned) = Arc::get_mut(owned)
        {
            owned.with_dependent_mut(|_, dependent| {
                for block in &mut dependent.blocks {
                    block.resolve_references(resolver, renderer, warnings);
                }
            });
        }
    }
}

/// Returns `true` if `line` introduces a Markdown-style blockquote: a `>`
/// followed by a space, or a bare `>`.
fn is_markdown_marker_line(line: &str) -> bool {
    line == ">" || line.starts_with("> ")
}

/// Removes a trailing attribution line (introduced by `--`) from `lines`,
/// returning the rendered attribution and citation.
///
/// Trailing blank lines are discarded first. If the last remaining line begins
/// with `--` followed by whitespace and text, it is consumed and split into the
/// attribution and (optional) citation.
fn take_trailing_attribution(
    lines: &mut Vec<String>,
    parser: &Parser,
) -> (Option<String>, Option<String>) {
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }

    if let Some(last) = lines.last()
        && let Some(rest) = last.strip_prefix("--")
        && (rest.starts_with(' ') || rest.starts_with('\t'))
    {
        let result = split_attribution_line(rest.trim(), parser);
        lines.pop();
        while lines.last().is_some_and(|line| line.is_empty()) {
            lines.pop();
        }
        return result;
    }

    (None, None)
}

/// Returns `true` if `line` is a quote/verse delimiter (a line of four or more
/// underscores).
pub(crate) fn is_quote_verse_delimiter(line: &Span<'_>) -> bool {
    let data = line.data();
    data.len() >= 4 && data.starts_with("____") && data.chars().all(|c| c == '_')
}

/// Builds a verse block's verbatim content: the text between the delimiters
/// with normal substitutions applied and line breaks preserved.
fn render_verbatim<'src>(inside: Span<'src>, parser: &Parser) -> Content<'src> {
    let trimmed = inside.discard_empty_lines().trim_trailing_whitespace();
    let mut content = Content::from(trimmed);
    SubstitutionGroup::Normal.apply(&mut content, parser, None);
    content
}

/// Renders a fragment of attribution text (the attribution or citation) by
/// applying normal inline substitutions, returning the owned result.
fn render_inline(parser: &Parser, text: &str) -> String {
    let span = Span::new(text);
    let mut content = Content::from(span);
    SubstitutionGroup::Normal.apply(&mut content, parser, None);
    content.rendered_owned()
}

/// Extracts the attribution and citation from a block's attribute list.
///
/// The attribution is the `attribution` named attribute or the second
/// positional attribute; the citation is the `citetitle` named attribute or the
/// third positional attribute. An empty value is treated as absent.
///
/// The values are used as-is: an attribute list value already has its
/// substitutions applied (e.g. a single-quoted value is rendered when the
/// attribute list is parsed), so they must not be re-rendered here.
fn extract_attribution(attrlist: Option<&Attrlist<'_>>) -> (Option<String>, Option<String>) {
    let Some(attrlist) = attrlist else {
        return (None, None);
    };

    let attribution = attrlist
        .named_or_positional_attribute("attribution", 2)
        .map(|a| a.value())
        .filter(|v| !v.is_empty())
        .map(str::to_string);

    let citetitle = attrlist
        .named_or_positional_attribute("citetitle", 3)
        .map(|a| a.value())
        .filter(|v| !v.is_empty())
        .map(str::to_string);

    (attribution, citetitle)
}

/// Splits an attribution line's text (everything after the leading `-- `) into
/// the attribution and the optional citation, separated by the first comma.
fn split_attribution_line(text: &str, parser: &Parser) -> (Option<String>, Option<String>) {
    match text.split_once(',') {
        Some((attribution, citetitle)) => {
            let attribution = attribution.trim();
            let citetitle = citetitle.trim();
            (
                non_empty(attribution).map(|v| render_inline(parser, v)),
                non_empty(citetitle).map(|v| render_inline(parser, v)),
            )
        }
        None => (
            non_empty(text.trim()).map(|v| render_inline(parser, v)),
            None,
        ),
    }
}

fn non_empty(s: &str) -> Option<&str> {
    if s.is_empty() { None } else { Some(s) }
}

/// Finds the attribution line in a quoted paragraph and splits it from the
/// quoted text.
///
/// The attribution line begins with `--` followed by at least one space or tab
/// and then more text. Returns the text before that line (the quoted text) and
/// the attribution text (everything after the `--` marker and its trailing
/// whitespace), or `None` if no attribution line is present.
fn split_at_attribution_line(data: &str) -> Option<(&str, &str)> {
    let mut line_start = 0;

    for line in data.split_inclusive('\n') {
        let trimmed = line.strip_suffix('\n').unwrap_or(line);

        if let Some(rest) = trimmed.strip_prefix("--")
            && (rest.starts_with(' ') || rest.starts_with('\t'))
        {
            let attribution_text = rest.trim_start_matches([' ', '\t']);
            // `line_start > 0` ensures there is at least one line of quoted text
            // before the attribution line; `line_start` is a line boundary, so
            // `split_at` never lands inside a character.
            if !attribution_text.is_empty() && line_start > 0 {
                let (quoted, _) = data.split_at(line_start);
                return Some((quoted, attribution_text));
            }
        }

        line_start += line.len();
    }

    None
}

/// Returns the span of the paragraph that begins at `source`: all lines up to
/// (but not including) the first blank line.
fn read_paragraph(source: Span<'_>) -> Span<'_> {
    source.trim_remainder(read_paragraph_after(source))
}

/// Returns the span that follows the paragraph beginning at `source` (starting
/// at the first blank line or end of input).
fn read_paragraph_after(source: Span<'_>) -> Span<'_> {
    let mut next = source;
    while let Some(line) = next.take_non_empty_line() {
        next = line.after;
    }
    next
}

impl<'src> IsBlock<'src> for QuoteBlock<'src> {
    fn content_model(&self) -> ContentModel {
        self.content_model
    }

    fn raw_context(&self) -> CowStr<'src> {
        self.type_.name().into()
    }

    fn declared_style(&'src self) -> Option<&'src str> {
        self.attrlist
            .as_ref()
            .and_then(|attrlist| attrlist.block_style())
    }

    fn rendered_content(&'src self) -> Option<&'src str> {
        self.content.as_ref().map(|content| content.rendered())
    }

    fn nested_blocks(&'src self) -> Iter<'src, Block<'src>> {
        self.blocks.iter()
    }

    fn nested_blocks_mut(&mut self) -> &mut [Block<'src>] {
        &mut self.blocks
    }

    fn content_mut(&mut self) -> Option<&mut Content<'src>> {
        self.content.as_mut()
    }

    fn title_source(&'src self) -> Option<Span<'src>> {
        self.title_source
    }

    fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    fn anchor(&'src self) -> Option<Span<'src>> {
        self.anchor
    }

    fn anchor_reftext(&'src self) -> Option<Span<'src>> {
        self.anchor_reftext
    }

    fn attrlist(&'src self) -> Option<&'src Attrlist<'src>> {
        self.attrlist.as_ref()
    }
}

impl<'src> HasSpan<'src> for QuoteBlock<'src> {
    fn span(&self) -> Span<'src> {
        self.source
    }
}

impl std::fmt::Debug for QuoteBlock<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuoteBlock")
            .field("type_", &self.type_)
            .field("content_model", &self.content_model)
            .field("content", &self.content)
            .field("blocks", &DebugSliceReference(&self.blocks))
            .field("attribution", &self.attribution)
            .field("citetitle", &self.citetitle)
            .field("source", &self.source)
            .field("title_source", &self.title_source)
            .field("title", &self.title)
            .field("anchor", &self.anchor)
            .field("anchor_reftext", &self.anchor_reftext)
            .field("attrlist", &self.attrlist)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::panic)]

    use std::ops::Deref;

    use crate::{
        blocks::{Block, ContentModel, IsBlock, QuoteType},
        tests::prelude::*,
    };

    fn parse_one(input: &'static str) -> Block<'static> {
        let mut parser = Parser::default();
        Block::parse(crate::Span::new(input), &mut parser)
            .unwrap_if_no_warnings()
            .unwrap()
            .item
    }

    fn as_quote<'a>(block: &'a Block<'a>) -> &'a crate::blocks::QuoteBlock<'a> {
        match block {
            Block::Quote(quote) => quote,
            // Only reached if a test parses an input that is not a quote block;
            // it exists to fail that test loudly, so it is uncovered while the
            // tests pass.
            other => panic!("expected a quote block, got {other:?}"),
        }
    }

    mod quote_type {
        use crate::blocks::QuoteType;

        #[test]
        fn name() {
            assert_eq!(QuoteType::Quote.name(), "quote");
            assert_eq!(QuoteType::Verse.name(), "verse");
        }

        #[test]
        fn impl_debug() {
            assert_eq!(format!("{:?}", QuoteType::Quote), "QuoteType::Quote");
            assert_eq!(format!("{:?}", QuoteType::Verse), "QuoteType::Verse");
        }

        #[test]
        fn impl_clone() {
            // Silly test to mark the #[derive(...)] line as covered.
            let v1 = QuoteType::Quote;
            let v2 = v1;
            assert_eq!(v1, v2);
        }
    }

    #[test]
    fn delimited_quote_is_compound() {
        let block = parse_one("____\nA quote.\n\nWith two paragraphs.\n____");
        let quote = as_quote(&block);

        assert_eq!(quote.type_(), QuoteType::Quote);
        assert_eq!(quote.content_model(), ContentModel::Compound);
        assert_eq!(quote.raw_context().deref(), "quote");
        assert!(quote.content().is_none());
        assert!(quote.attribution().is_none());
        assert!(quote.citetitle().is_none());
        assert_eq!(quote.blocks().len(), 2);
        assert_eq!(quote.nested_blocks().count(), 2);
    }

    #[test]
    fn delimited_quote_with_attribution_and_citation() {
        let block =
            parse_one("[quote,Abraham Lincoln,Gettysburg Address]\n____\nFour score.\n____");
        let quote = as_quote(&block);

        assert_eq!(quote.attribution(), Some("Abraham Lincoln"));
        assert_eq!(quote.citetitle(), Some("Gettysburg Address"));
        assert_eq!(quote.declared_style(), Some("quote"));
    }

    #[test]
    fn styled_paragraph_quote_is_simple() {
        let block = parse_one("[quote,Albert Einstein]\nA person who never made a mistake.");
        let quote = as_quote(&block);

        assert_eq!(quote.type_(), QuoteType::Quote);
        assert_eq!(quote.content_model(), ContentModel::Simple);
        assert_eq!(
            quote.content().unwrap().rendered(),
            "A person who never made a mistake."
        );
        assert_eq!(
            quote.rendered_content(),
            Some("A person who never made a mistake.")
        );
        assert_eq!(quote.attribution(), Some("Albert Einstein"));
        assert!(quote.citetitle().is_none());
    }

    #[test]
    fn verse_paragraph_is_simple() {
        let block = parse_one("[verse,Carl Sandburg,Fog]\nThe fog comes\non little cat feet.");
        let quote = as_quote(&block);

        assert_eq!(quote.type_(), QuoteType::Verse);
        assert_eq!(quote.content_model(), ContentModel::Simple);
        assert_eq!(quote.raw_context().deref(), "verse");
        assert_eq!(
            quote.content().unwrap().rendered(),
            "The fog comes\non little cat feet."
        );
        assert_eq!(quote.attribution(), Some("Carl Sandburg"));
        assert_eq!(quote.citetitle(), Some("Fog"));
    }

    #[test]
    fn verse_delimited_preserves_line_breaks() {
        let block = parse_one("[verse]\n____\nA verse\ndelimited block\n____");
        let quote = as_quote(&block);

        assert_eq!(quote.type_(), QuoteType::Verse);
        assert_eq!(quote.content_model(), ContentModel::Simple);
        assert_eq!(
            quote.content().unwrap().rendered(),
            "A verse\ndelimited block"
        );
        assert!(quote.nested_blocks().next().is_none());
    }

    #[test]
    fn quote_or_verse_style_over_other_container_is_not_a_quote() {
        // A `quote`/`verse` style over an example, sidebar, listing, literal, or
        // passthrough block keeps that container's own context.
        assert_eq!(
            parse_one("[quote]\n====\nx\n====").raw_context().deref(),
            "example"
        );
        assert_eq!(
            parse_one("[verse]\n****\nx\n****").raw_context().deref(),
            "sidebar"
        );
        assert_eq!(
            parse_one("[quote]\n----\nx\n----").raw_context().deref(),
            "listing"
        );
    }

    #[test]
    fn quoted_paragraph_with_attribution_and_citation() {
        let block = parse_one("\"A little rebellion is good.\"\n-- Thomas Jefferson, Volume 11");
        let quote = as_quote(&block);

        assert_eq!(quote.type_(), QuoteType::Quote);
        assert_eq!(quote.content_model(), ContentModel::Simple);
        assert_eq!(
            quote.content().unwrap().rendered(),
            "A little rebellion is good."
        );
        assert_eq!(quote.attribution(), Some("Thomas Jefferson"));
        assert_eq!(quote.citetitle(), Some("Volume 11"));
    }

    #[test]
    fn quoted_paragraph_without_citation() {
        let block = parse_one("\"A quote.\"\n-- Anonymous");
        let quote = as_quote(&block);

        assert_eq!(quote.attribution(), Some("Anonymous"));
        assert!(quote.citetitle().is_none());
    }

    #[test]
    fn quoted_paragraph_requires_attribution_line() {
        // Without a `-- ` attribution line, a quoted sentence is just a
        // paragraph.
        let block = parse_one("\"Just a quoted sentence.\"");
        assert_eq!(block.raw_context().deref(), "paragraph");
    }

    #[test]
    fn quoted_paragraph_requires_opening_quote() {
        // The attribution line alone, with no opening quote, is not a quoted
        // paragraph.
        let block = parse_one("Not quoted.\n-- Someone");
        assert_eq!(block.raw_context().deref(), "paragraph");
    }

    #[test]
    fn empty_quoted_paragraph_is_not_a_quote() {
        // A pair of quotes wrapping no text is not a quoted paragraph.
        let block = parse_one("\"\"\n-- Someone");
        assert_eq!(block.raw_context().deref(), "paragraph");
    }

    #[test]
    fn unclosed_quoted_paragraph_is_not_a_quote() {
        // An opening quote with no closing quote is not a quoted paragraph.
        let block = parse_one("\"no closing quote\n-- Someone");
        assert_eq!(block.raw_context().deref(), "paragraph");
    }

    #[test]
    fn dash_line_without_attribution_text_is_not_a_quote() {
        // A `-- ` line with no attribution text after it is not an attribution
        // line, so this is an ordinary paragraph.
        let block = parse_one("\"A quote.\"\n--  ");
        assert_eq!(block.raw_context().deref(), "paragraph");
    }

    #[test]
    fn quoted_paragraph_tab_separated_attribution() {
        // The `--` attribution marker may be separated from the text by a tab.
        let block = parse_one("\"A quote.\"\n--\tSomeone");
        let quote = as_quote(&block);
        assert_eq!(quote.attribution(), Some("Someone"));
    }

    #[test]
    fn attribution_with_empty_name_keeps_citation() {
        // A `--` line of the form `-- , citation` has no attribution but still
        // yields a citation.
        let block = parse_one("\"A quote.\"\n-- , Just a citation");
        let quote = as_quote(&block);
        assert!(quote.attribution().is_none());
        assert_eq!(quote.citetitle(), Some("Just a citation"));
    }

    #[test]
    fn styled_paragraph_with_no_content_is_not_a_quote() {
        // A `[quote]` style with no following content cannot form a styled
        // paragraph; the lone attribute list is treated as an ordinary block.
        let mut parser = Parser::default();
        let maw = Block::parse(crate::Span::new("[quote]\n"), &mut parser);
        let block = maw.item.unwrap().item;
        assert_eq!(block.raw_context().deref(), "paragraph");
    }

    #[test]
    fn markdown_blockquote_tab_attribution_after_blank() {
        // A trailing blank (`>`) line before the `--` attribution is discarded,
        // and the attribution marker may be tab-separated.
        let block = parse_one("> A quote.\n>\n> --\tSomeone");
        let quote = as_quote(&block);
        assert_eq!(quote.attribution(), Some("Someone"));
        assert_eq!(quote.blocks().len(), 1);
    }

    #[test]
    fn markdown_blockquote_double_dash_without_space_is_content() {
        // A trailing `--` not followed by whitespace is content, not an
        // attribution.
        let block = parse_one("> A quote.\n> --nospace");
        let quote = as_quote(&block);
        assert!(quote.attribution().is_none());
    }

    #[test]
    fn markdown_blockquote_basic() {
        let block = parse_one("> A markdown quote.");
        let quote = as_quote(&block);

        assert_eq!(quote.type_(), QuoteType::Quote);
        assert_eq!(quote.content_model(), ContentModel::Compound);
        assert_eq!(quote.blocks().len(), 1);
        // The nested blocks borrow the block's owned source, so they are not
        // exposed through the `'src`-bound trait accessor.
        assert!(quote.nested_blocks().next().is_none());
    }

    #[test]
    fn markdown_blockquote_with_attribution() {
        let block = parse_one("> A quote.\n> -- Someone");
        let quote = as_quote(&block);

        assert_eq!(quote.attribution(), Some("Someone"));
        assert_eq!(quote.blocks().len(), 1);
    }

    #[test]
    fn markdown_blockquote_lazy_continuation() {
        // A line without a `>` marker continues the current paragraph.
        let block = parse_one("> line one\nline two");
        let quote = as_quote(&block);

        assert_eq!(quote.blocks().len(), 1);
        let inner = quote.blocks().first().unwrap();
        assert_eq!(inner.rendered_content(), Some("line one\nline two"));
    }

    #[test]
    fn markdown_marker_requires_space() {
        // A `>` immediately followed by other text is not a blockquote.
        let block = parse_one(">foo bar");
        assert_eq!(block.raw_context().deref(), "paragraph");
    }

    #[test]
    fn markdown_yields_to_description_list() {
        // A `>`-prefixed description-list term is a description list, not a
        // Markdown-style blockquote.
        let block = parse_one("> term:: definition");
        assert_eq!(block.raw_context().deref(), "list");
    }

    #[test]
    fn unterminated_delimited_quote_warns() {
        let mut parser = Parser::default();
        let maw = Block::parse(crate::Span::new("____\nunclosed"), &mut parser);

        let block = maw.item.unwrap().item;
        assert_eq!(block.raw_context().deref(), "quote");
        assert_eq!(maw.warnings.len(), 1);
        assert_eq!(
            maw.warnings.first().unwrap().warning,
            WarningType::UnterminatedDelimitedBlock
        );
    }

    #[test]
    fn citation_receives_inline_substitutions() {
        // A link in the citation is rendered to an anchor.
        let block = parse_one(
            "[quote,Lewis Carroll,'See https://example.com/lc[the bio]']\n____\nAny road.\n____",
        );
        let quote = as_quote(&block);
        let citetitle = quote.citetitle().unwrap();
        assert!(
            citetitle.contains("<a href=\"https://example.com/lc\">the bio</a>"),
            "citation was: {citetitle}"
        );
    }

    #[test]
    fn block_enum_delegates_to_quote() {
        // Exercise the `Block`-level `IsBlock`/`Debug` arms for a quote block.
        let compound = parse_one("____\nx\n____");
        assert_eq!(compound.content_model(), ContentModel::Compound);
        assert_eq!(compound.raw_context().deref(), "quote");
        assert!(compound.title_source().is_none());
        assert!(compound.anchor().is_none());
        assert!(compound.anchor_reftext().is_none());
        assert!(compound.attrlist().is_none());
        assert_eq!(compound.substitution_group(), SubstitutionGroup::Normal);
        assert_eq!(compound.nested_blocks().count(), 1);
        assert!(compound.title().is_none());
        assert!(compound.declared_style().is_none());
        assert!(format!("{compound:?}").starts_with("Block::Quote"));

        let simple = parse_one("[verse]\nverse text");
        assert_eq!(simple.rendered_content(), Some("verse text"));
        assert_eq!(simple.content_model(), ContentModel::Simple);
    }

    #[test]
    fn impl_debug() {
        let block = parse_one("____\nx\n____");
        let quote = as_quote(&block);
        let debug = format!("{quote:?}");
        assert!(debug.starts_with("QuoteBlock {"));
        assert!(debug.contains("type_: QuoteType::Quote"));
    }

    #[test]
    fn impl_clone() {
        // Silly test to mark the #[derive(...)] line as covered.
        let block = parse_one("____\nclone me\n____");
        let quote = as_quote(&block).clone();
        assert_eq!(quote.type_(), QuoteType::Quote);
    }

    #[test]
    fn title_renders_inside_quote_block() {
        let doc = Parser::default()
            .parse(".A title\n[quote,Captain Kirk]\nEverybody remember where we parked.");
        let block = doc.nested_blocks().next().unwrap();
        let quote = as_quote(block);
        assert_eq!(quote.title(), Some("A title"));
    }
}
