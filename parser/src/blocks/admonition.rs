use std::slice::Iter;

use crate::{
    HasSpan, Parser, Span,
    attributes::Attrlist,
    blocks::{
        AdmonitionVariant, Block, CompoundDelimitedBlock, ContentModel, IsBlock, RawDelimitedBlock,
        SimpleBlock, TableBlock, metadata::BlockMetadata,
    },
    content::Content,
    document::InterpretedValue,
    internal::debug::DebugSliceReference,
    span::MatchedItem,
    strings::CowStr,
    warnings::MatchAndWarnings,
};

/// An admonition draws attention to a statement by taking it out of the
/// content's flow and labeling it with a priority (its type).
///
/// An admonition can be written in two ways:
///
/// * As a **paragraph** whose first line begins with one of the five admonition
///   labels followed by a colon (e.g., `NOTE: This is a note.`). This produces
///   an admonition with the [`Simple`] content model.
/// * As a **block** by setting one of the labels as the block style in an
///   attribute list (e.g., `[NOTE]`). This is referred to as *masquerading*.
///   When the style is set on a delimited block that can contain other blocks
///   (such as an example block), the admonition uses the [`Compound`] content
///   model; otherwise it uses the [`Simple`] content model.
///
/// [`Simple`]: ContentModel::Simple
/// [`Compound`]: ContentModel::Compound
#[derive(Clone, Eq, PartialEq)]
pub struct AdmonitionBlock<'src> {
    variant: AdmonitionVariant,
    label: String,
    icons_font: bool,
    content_model: ContentModel,
    content: Option<Content<'src>>,
    blocks: Vec<Block<'src>>,
    source: Span<'src>,
    title_source: Option<Span<'src>>,
    title: Option<Content<'src>>,
    anchor: Option<Span<'src>>,
    anchor_reftext: Option<Span<'src>>,
    attrlist: Option<Attrlist<'src>>,
}

impl<'src> AdmonitionBlock<'src> {
    /// Returns the block's title as a mutable [`Content`], if the block has
    /// one.
    ///
    /// This narrow seam exists for the document-order title resolution pass
    /// (see `document::title_refs`), which installs the re-rendered title
    /// after resolving any cross-references embedded in it. All other access
    /// goes through the read-only [`IsBlock::title`] accessor.
    pub(crate) fn title_content_mut(&mut self) -> Option<&mut Content<'src>> {
        self.title.as_mut()
    }

    /// Parse an admonition block, if the given metadata and content describe
    /// one.
    ///
    /// Returns `None` (without consuming the block) when the content is not an
    /// admonition, so that the caller can fall through to other block parsers.
    pub(crate) fn parse(
        metadata: &BlockMetadata<'src>,
        parser: &mut Parser,
    ) -> Option<MatchAndWarnings<'src, Option<MatchedItem<'src, Self>>>> {
        // Masquerade form: the block style names an admonition type.
        if let Some(style) = metadata.attrlist.as_ref().and_then(|a| a.block_style())
            && let Some(variant) = AdmonitionVariant::from_style(style)
        {
            return Some(Self::parse_masquerade(metadata, parser, variant));
        }

        // Paragraph form: the first line begins with `LABEL: `. This form does
        // not apply when a block style was declared (that would be a
        // masquerade), so it is only reached when there is no admonition style.
        if metadata
            .attrlist
            .as_ref()
            .and_then(|a| a.block_style())
            .is_none()
            && let Some((variant, content_start)) =
                admonition_paragraph_prefix(metadata.block_start)
        {
            return Some(Self::parse_paragraph(
                metadata,
                parser,
                variant,
                content_start,
            ));
        }

        None
    }

    /// Parse the masquerade form, where the admonition style is set on a block.
    fn parse_masquerade(
        metadata: &BlockMetadata<'src>,
        parser: &mut Parser,
        variant: AdmonitionVariant,
    ) -> MatchAndWarnings<'src, Option<MatchedItem<'src, Self>>> {
        let first_line = metadata.block_start.take_normalized_line().item;

        // An admonition style masquerades only over an example block, an open
        // block, or a paragraph. Over an example or open block it yields
        // compound content.
        //
        // For a valid example/open delimiter, `CompoundDelimitedBlock::parse`
        // always returns `item: Some(..)` — even for an unterminated block, in
        // which case it also reports an `UnterminatedDelimitedBlock` warning.
        // Those `warnings` are bound here and forwarded through `finish`, so no
        // diagnostic is lost on the masquerade path.
        if is_example_or_open_delimiter(&first_line)
            && let Some(MatchAndWarnings {
                item: Some(inner),
                warnings,
            }) = CompoundDelimitedBlock::parse(metadata, parser)
        {
            let after = inner.after;
            let blocks = inner.item.into_nested_blocks();
            return Self::finish(
                metadata,
                parser,
                variant,
                ContentModel::Compound,
                None,
                blocks,
                after,
                warnings,
            );
        }

        // Any other structural container (sidebar, quote, listing, literal,
        // passthrough, table, comment) keeps its own context and ignores the
        // admonition style, so this is not an admonition.
        if RawDelimitedBlock::is_valid_delimiter(&first_line)
            || CompoundDelimitedBlock::is_valid_delimiter(&first_line)
            || TableBlock::is_table_delimiter(&first_line)
        {
            return MatchAndWarnings {
                item: None,
                warnings: vec![],
            };
        }

        // Otherwise the style is applied to a paragraph: simple content.
        if let Some(inner) = SimpleBlock::parse(metadata, parser) {
            return Self::finish(
                metadata,
                parser,
                variant,
                ContentModel::Simple,
                Some(inner.item.content().clone()),
                vec![],
                inner.after,
                vec![],
            );
        }

        MatchAndWarnings {
            item: None,
            warnings: vec![],
        }
    }

    /// Parse the paragraph form, where the first line begins with `LABEL: `.
    fn parse_paragraph(
        metadata: &BlockMetadata<'src>,
        parser: &mut Parser,
        variant: AdmonitionVariant,
        content_start: Span<'src>,
    ) -> MatchAndWarnings<'src, Option<MatchedItem<'src, Self>>> {
        // Build a metadata that begins after the stripped label so that the
        // paragraph content is parsed without it. The admonition retains the
        // outer block's title, anchor, and attribute list.
        let inner_metadata = BlockMetadata {
            title_source: None,
            title: None,
            anchor: None,
            anchor_reftext: None,
            attrlist: None,
            source: content_start,
            block_start: content_start,
        };

        if let Some(inner) = SimpleBlock::parse(&inner_metadata, parser) {
            return Self::finish(
                metadata,
                parser,
                variant,
                ContentModel::Simple,
                Some(inner.item.content().clone()),
                vec![],
                inner.after,
                vec![],
            );
        }

        // Unreachable in practice: `parse_paragraph` is only called after
        // `admonition_paragraph_prefix` matched, which requires a non-whitespace
        // character after the label on the first line (trailing whitespace is
        // trimmed before the match). `content_start` therefore points at
        // non-empty content, so `SimpleBlock::parse` always returns `Some`. This
        // fall-through is kept as a defensive default that mirrors
        // `parse_masquerade`.
        MatchAndWarnings {
            item: None,
            warnings: vec![],
        }
    }

    /// Assemble the final admonition block from its parsed parts, resolving the
    /// caption and icon state from the current document attributes.
    #[allow(clippy::too_many_arguments)]
    fn finish(
        metadata: &BlockMetadata<'src>,
        parser: &Parser,
        variant: AdmonitionVariant,
        content_model: ContentModel,
        content: Option<Content<'src>>,
        blocks: Vec<Block<'src>>,
        after: Span<'src>,
        warnings: Vec<crate::warnings::Warning<'src>>,
    ) -> MatchAndWarnings<'src, Option<MatchedItem<'src, Self>>> {
        let source = metadata
            .source
            .trim_remainder(after)
            .trim_trailing_whitespace();

        MatchAndWarnings {
            item: Some(MatchedItem {
                item: Self {
                    variant,
                    label: resolve_caption(variant, parser),
                    icons_font: icons_are_font(parser),
                    content_model,
                    content,
                    blocks,
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

    /// Returns the admonition type (e.g., [`AdmonitionVariant::Note`]).
    pub fn variant(&self) -> AdmonitionVariant {
        self.variant
    }

    /// Returns the lowercase name for this admonition (e.g., `note`).
    pub fn name(&self) -> &'static str {
        self.variant.name()
    }

    /// Returns the caption (label) text shown for this admonition (e.g.,
    /// `Note`).
    ///
    /// This is the value of the `<type>-caption` document attribute if set, or
    /// the default caption for the admonition type otherwise.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns `true` if font-based icons are enabled (i.e., the `icons`
    /// document attribute is set to `font`).
    pub fn icons_font(&self) -> bool {
        self.icons_font
    }

    /// Returns the simple content of this admonition, if it has the
    /// [`Simple`](ContentModel::Simple) content model.
    pub fn content(&self) -> Option<&Content<'src>> {
        self.content.as_ref()
    }
}

/// Returns `true` if `line` is the opening delimiter of an example block
/// (`====`) or an open block (`--`).
///
/// These are the only delimited blocks over which an admonition style may
/// masquerade.
fn is_example_or_open_delimiter(line: &Span<'_>) -> bool {
    let data = line.data();

    if data == "--" {
        return true;
    }

    data.len() >= 4 && data.chars().all(|c| c == '=')
}

/// Resolve the caption (label) text for an admonition variant from the current
/// document attributes, falling back to the variant's default caption.
fn resolve_caption(variant: AdmonitionVariant, parser: &Parser) -> String {
    let key = format!("{}-caption", variant.name());
    match parser.attribute_value(key) {
        InterpretedValue::Value(value) => value,
        _ => variant.default_caption().to_string(),
    }
}

/// Returns `true` if the `icons` document attribute is set to `font`.
fn icons_are_font(parser: &Parser) -> bool {
    matches!(parser.attribute_value("icons"), InterpretedValue::Value(value) if value == "font")
}

/// If the first line of `block_start` begins with an admonition label followed
/// by a colon and at least one space (e.g., `NOTE: `), return the admonition
/// variant and a span positioned at the start of the paragraph content (after
/// the stripped label).
pub(crate) fn admonition_paragraph_prefix(
    block_start: Span<'_>,
) -> Option<(AdmonitionVariant, Span<'_>)> {
    let line = block_start.take_normalized_line().item;
    let data = line.data();

    for variant in [
        AdmonitionVariant::Note,
        AdmonitionVariant::Tip,
        AdmonitionVariant::Important,
        AdmonitionVariant::Caution,
        AdmonitionVariant::Warning,
    ] {
        if let Some(rest) = data.strip_prefix(variant.style())
            && let Some(rest) = rest.strip_prefix(':')
            && (rest.starts_with(' ') || rest.starts_with('\t'))
        {
            let whitespace_len = rest.len() - rest.trim_start_matches([' ', '\t']).len();
            let prefix_len = variant.style().len() + 1 + whitespace_len;
            return Some((variant, block_start.discard(prefix_len)));
        }
    }

    None
}

/// Returns `true` if the first line of `line` begins with an admonition
/// paragraph label (e.g., `NOTE: `).
pub(crate) fn starts_with_admonition_label(line: Span<'_>) -> bool {
    admonition_paragraph_prefix(line).is_some()
}

impl<'src> IsBlock<'src> for AdmonitionBlock<'src> {
    fn content_model(&self) -> ContentModel {
        self.content_model
    }

    fn raw_context(&self) -> CowStr<'src> {
        "admonition".into()
    }

    fn declared_style(&'src self) -> Option<&'src str> {
        Some(self.variant.style())
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
        self.title.as_ref().map(Content::rendered_str)
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

impl<'src> HasSpan<'src> for AdmonitionBlock<'src> {
    fn span(&self) -> Span<'src> {
        self.source
    }
}

impl std::fmt::Debug for AdmonitionBlock<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdmonitionBlock")
            .field("variant", &self.variant)
            .field("label", &self.label)
            .field("icons_font", &self.icons_font)
            .field("content_model", &self.content_model)
            .field("content", &self.content)
            .field("blocks", &DebugSliceReference(&self.blocks))
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
        blocks::{AdmonitionVariant, Block, ContentModel, IsBlock},
        tests::prelude::*,
    };

    fn parse_one(input: &'static str) -> Block<'static> {
        let mut parser = Parser::default();
        Block::parse(crate::Span::new(input), &mut parser)
            .unwrap_if_no_warnings()
            .unwrap()
            .item
    }

    fn as_admonition<'a>(block: &'a Block<'a>) -> &'a crate::blocks::AdmonitionBlock<'a> {
        match block {
            Block::Admonition(admonition) => admonition,
            // Only reached if a test parses an input that is not an admonition;
            // it exists to fail that test loudly, so it is uncovered while the
            // tests pass.
            other => panic!("expected an admonition block, got {other:?}"),
        }
    }

    mod admonition_variant {
        use crate::blocks::AdmonitionVariant;

        #[test]
        fn style() {
            assert_eq!(AdmonitionVariant::Note.style(), "NOTE");
            assert_eq!(AdmonitionVariant::Tip.style(), "TIP");
            assert_eq!(AdmonitionVariant::Important.style(), "IMPORTANT");
            assert_eq!(AdmonitionVariant::Caution.style(), "CAUTION");
            assert_eq!(AdmonitionVariant::Warning.style(), "WARNING");
        }

        #[test]
        fn name() {
            assert_eq!(AdmonitionVariant::Note.name(), "note");
            assert_eq!(AdmonitionVariant::Tip.name(), "tip");
            assert_eq!(AdmonitionVariant::Important.name(), "important");
            assert_eq!(AdmonitionVariant::Caution.name(), "caution");
            assert_eq!(AdmonitionVariant::Warning.name(), "warning");
        }

        #[test]
        fn default_caption() {
            assert_eq!(AdmonitionVariant::Note.default_caption(), "Note");
            assert_eq!(AdmonitionVariant::Tip.default_caption(), "Tip");
            assert_eq!(AdmonitionVariant::Important.default_caption(), "Important");
            assert_eq!(AdmonitionVariant::Caution.default_caption(), "Caution");
            assert_eq!(AdmonitionVariant::Warning.default_caption(), "Warning");
        }

        #[test]
        fn from_style() {
            assert_eq!(
                AdmonitionVariant::from_style("NOTE"),
                Some(AdmonitionVariant::Note)
            );
            assert_eq!(
                AdmonitionVariant::from_style("TIP"),
                Some(AdmonitionVariant::Tip)
            );
            assert_eq!(
                AdmonitionVariant::from_style("IMPORTANT"),
                Some(AdmonitionVariant::Important)
            );
            assert_eq!(
                AdmonitionVariant::from_style("CAUTION"),
                Some(AdmonitionVariant::Caution)
            );
            assert_eq!(
                AdmonitionVariant::from_style("WARNING"),
                Some(AdmonitionVariant::Warning)
            );

            // The match is case-sensitive and rejects non-labels.
            assert_eq!(AdmonitionVariant::from_style("note"), None);
            assert_eq!(AdmonitionVariant::from_style("Note"), None);
            assert_eq!(AdmonitionVariant::from_style("example"), None);
        }

        #[test]
        fn impl_debug() {
            assert_eq!(
                format!("{:?}", AdmonitionVariant::Note),
                "AdmonitionVariant::Note"
            );
            assert_eq!(
                format!("{:?}", AdmonitionVariant::Tip),
                "AdmonitionVariant::Tip"
            );
            assert_eq!(
                format!("{:?}", AdmonitionVariant::Important),
                "AdmonitionVariant::Important"
            );
            assert_eq!(
                format!("{:?}", AdmonitionVariant::Caution),
                "AdmonitionVariant::Caution"
            );
            assert_eq!(
                format!("{:?}", AdmonitionVariant::Warning),
                "AdmonitionVariant::Warning"
            );
        }

        #[test]
        fn impl_clone() {
            // Silly test to mark the #[derive(...)] line as covered.
            let v1 = AdmonitionVariant::Note;
            let v2 = v1;
            assert_eq!(v1, v2);
        }
    }

    #[test]
    fn paragraph_form() {
        let block = parse_one("NOTE: This is a note.");
        let admonition = as_admonition(&block);

        assert_eq!(admonition.variant(), AdmonitionVariant::Note);
        assert_eq!(admonition.name(), "note");
        assert_eq!(admonition.label(), "Note");
        assert!(!admonition.icons_font());
        assert_eq!(admonition.content_model(), ContentModel::Simple);
        assert_eq!(admonition.content().unwrap().rendered(), "This is a note.");
        assert_eq!(admonition.rendered_content(), Some("This is a note."));
        assert_eq!(admonition.raw_context().deref(), "admonition");
        assert_eq!(admonition.resolved_context().deref(), "admonition");
        assert_eq!(admonition.declared_style(), Some("NOTE"));
        assert!(admonition.nested_blocks().next().is_none());
        assert!(admonition.title().is_none());
        assert!(admonition.attrlist().is_none());
    }

    #[test]
    fn paragraph_form_all_variants() {
        for (input, variant, name) in [
            ("NOTE: x", AdmonitionVariant::Note, "note"),
            ("TIP: x", AdmonitionVariant::Tip, "tip"),
            ("IMPORTANT: x", AdmonitionVariant::Important, "important"),
            ("CAUTION: x", AdmonitionVariant::Caution, "caution"),
            ("WARNING: x", AdmonitionVariant::Warning, "warning"),
        ] {
            let block = parse_one(input);
            let admonition = as_admonition(&block);
            assert_eq!(admonition.variant(), variant);
            assert_eq!(admonition.name(), name);
        }
    }

    #[test]
    fn paragraph_form_multiline() {
        let block = parse_one("NOTE: first line\nsecond line");
        let admonition = as_admonition(&block);
        assert_eq!(
            admonition.content().unwrap().rendered(),
            "first line\nsecond line"
        );
    }

    #[test]
    fn paragraph_form_tab_separator() {
        let block = parse_one("NOTE:\tindented with a tab");
        let admonition = as_admonition(&block);
        assert_eq!(admonition.variant(), AdmonitionVariant::Note);
        assert_eq!(
            admonition.content().unwrap().rendered(),
            "indented with a tab"
        );
    }

    #[test]
    fn not_an_admonition_when_lowercase_label() {
        let block = parse_one("note: not an admonition");
        assert_eq!(block.raw_context().deref(), "paragraph");
    }

    #[test]
    fn not_an_admonition_without_separating_space() {
        let block = parse_one("NOTE:no space");
        assert_eq!(block.raw_context().deref(), "paragraph");
    }

    #[test]
    fn not_an_admonition_for_partial_label() {
        let block = parse_one("NOTES: a longer word");
        assert_eq!(block.raw_context().deref(), "paragraph");
    }

    #[test]
    fn indented_label_is_a_literal_block_not_an_admonition() {
        // Indented content is a literal block; the leading whitespace means the
        // line is not an admonition paragraph.
        let block = parse_one("  NOTE: indented text");
        assert_eq!(block.raw_context().deref(), "paragraph");
    }

    #[test]
    fn masquerade_on_example_block_is_compound() {
        let block = parse_one("[NOTE]\n====\nCompound content.\n====");
        let admonition = as_admonition(&block);
        assert_eq!(admonition.variant(), AdmonitionVariant::Note);
        assert_eq!(admonition.content_model(), ContentModel::Compound);
        assert!(admonition.content().is_none());
        assert!(admonition.rendered_content().is_none());
        assert_eq!(admonition.nested_blocks().count(), 1);
    }

    #[test]
    fn masquerade_propagates_unterminated_warning() {
        // An unterminated example block under an admonition style still parses
        // as a (compound) admonition, and its `UnterminatedDelimitedBlock`
        // warning is not swallowed by the masquerade path.
        let mut parser = Parser::default();
        let maw = Block::parse(crate::Span::new("[NOTE]\n====\nunclosed"), &mut parser);

        let block = maw.item.unwrap().item;
        assert_eq!(block.raw_context().deref(), "admonition");
        assert_eq!(maw.warnings.len(), 1);
        assert_eq!(
            maw.warnings.first().unwrap().warning,
            crate::warnings::WarningType::UnterminatedDelimitedBlock
        );
    }

    #[test]
    fn masquerade_on_open_block_is_compound() {
        let block = parse_one("[WARNING]\n--\npara one\n\npara two\n--");
        let admonition = as_admonition(&block);
        assert_eq!(admonition.variant(), AdmonitionVariant::Warning);
        assert_eq!(admonition.content_model(), ContentModel::Compound);
        assert_eq!(admonition.nested_blocks().count(), 2);
    }

    #[test]
    fn masquerade_on_paragraph_is_simple() {
        let block = parse_one("[TIP]\nA single paragraph.");
        let admonition = as_admonition(&block);
        assert_eq!(admonition.variant(), AdmonitionVariant::Tip);
        assert_eq!(admonition.content_model(), ContentModel::Simple);
        assert_eq!(
            admonition.content().unwrap().rendered(),
            "A single paragraph."
        );
    }

    #[test]
    fn masquerade_with_title() {
        let block = parse_one("[NOTE]\n.A title\n====\nContent.\n====");
        let admonition = as_admonition(&block);
        assert_eq!(admonition.title(), Some("A title"));
    }

    #[test]
    fn masquerade_does_not_apply_to_other_containers() {
        // Sidebar, quote, listing, literal, and passthrough blocks keep their
        // own context; the admonition style is ignored.
        assert_eq!(
            parse_one("[NOTE]\n****\nx\n****").raw_context().deref(),
            "sidebar"
        );
        assert_eq!(
            parse_one("[NOTE]\n____\nx\n____").raw_context().deref(),
            "quote"
        );
        assert_eq!(
            parse_one("[NOTE]\n----\nx\n----").raw_context().deref(),
            "listing"
        );
        assert_eq!(
            parse_one("[NOTE]\n....\nx\n....").raw_context().deref(),
            "literal"
        );
        assert_eq!(
            parse_one("[NOTE]\n++++\nx\n++++").raw_context().deref(),
            "pass"
        );
    }

    #[test]
    fn impl_debug() {
        let block = parse_one("NOTE: hi");
        let admonition = as_admonition(&block);
        let debug = format!("{admonition:?}");
        assert!(debug.starts_with("AdmonitionBlock {"));
        assert!(debug.contains("variant: AdmonitionVariant::Note"));
    }

    #[test]
    fn impl_clone() {
        // Silly test to mark the #[derive(...)] line as covered.
        let block = parse_one("NOTE: clone me");
        let admonition = as_admonition(&block).clone();
        assert_eq!(admonition.variant(), AdmonitionVariant::Note);
    }

    #[test]
    fn masquerade_without_content_is_not_an_admonition() {
        // A masquerade style with no following block content cannot form an
        // admonition; the masquerade parser reports no block and the line is
        // treated as an ordinary block with a missing-block warning.
        let mut parser = Parser::default();
        let maw = Block::parse(crate::Span::new("[NOTE]\n"), &mut parser);

        // The lone attribute list has no block to attach to: a missing-block
        // warning is emitted and the line is treated as an ordinary paragraph,
        // not an admonition.
        let block = maw.item.unwrap().item;
        assert_eq!(block.raw_context().deref(), "paragraph");
        assert_eq!(maw.warnings.len(), 1);
        assert_eq!(
            maw.warnings.first().unwrap().warning,
            crate::warnings::WarningType::MissingBlockAfterTitleOrAttributeList
        );
    }

    #[test]
    fn block_enum_delegates_to_admonition() {
        // Exercise the `Block`-level `IsBlock`/`Debug` arms for an admonition
        // (rather than calling through the unwrapped `AdmonitionBlock`).
        let simple = parse_one("NOTE: text");
        assert_eq!(simple.content_model(), ContentModel::Simple);
        assert_eq!(simple.rendered_content(), Some("text"));
        assert_eq!(simple.raw_context().deref(), "admonition");
        assert_eq!(simple.declared_style(), Some("NOTE"));
        assert!(simple.title_source().is_none());
        assert!(simple.anchor_reftext().is_none());
        assert_eq!(simple.substitution_group(), SubstitutionGroup::Normal);
        assert!(simple.nested_blocks().next().is_none());
        assert!(format!("{simple:?}").starts_with("Block::Admonition"));

        let compound = parse_one("[NOTE]\n====\nx\n====");
        assert_eq!(compound.content_model(), ContentModel::Compound);
        assert_eq!(compound.nested_blocks().count(), 1);
    }

    #[test]
    fn block_declared_style_for_non_admonition_kinds() {
        // The `Block::declared_style` delegation returns `None` for block kinds
        // that have no positional style: a media block, a list item, and a
        // document-attribute block.
        let media = parse_one("image::a.png[]");
        assert_eq!(media.raw_context().deref(), "image");
        assert!(media.declared_style().is_none());

        let list = parse_one("* item");
        let item = list.nested_blocks().next().unwrap();
        assert_eq!(item.raw_context().deref(), "list_item");
        assert!(item.declared_style().is_none());

        let attribute = parse_one(":foo: bar");
        assert_eq!(attribute.raw_context().deref(), "attribute");
        assert!(attribute.declared_style().is_none());
    }

    mod caption_and_icons {
        use crate::tests::prelude::*;

        #[test]
        fn default_caption_when_icons_unset() {
            let doc = Parser::default().parse("CAUTION: Slippery when wet.");
            assert_xpath(
                &doc,
                "//td[@class=\"icon\"]/div[@class=\"title\"][text()=\"Caution\"]",
                1,
            );
        }

        #[test]
        fn caption_override_via_attribute() {
            let doc = Parser::default().parse(":note-caption: Remarque\n\nNOTE: Bonjour.");
            assert_xpath(
                &doc,
                "//td[@class=\"icon\"]/div[@class=\"title\"][text()=\"Remarque\"]",
                1,
            );
        }

        #[test]
        fn font_icons_when_enabled() {
            let doc = Parser::default().parse(":icons: font\n\nNOTE: This is a note.");
            assert_css(&doc, "td.icon i.fa.icon-note", 1);
            assert_css(&doc, "td.icon .title", 0);
        }
    }
}
