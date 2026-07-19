use std::{fmt, slice::Iter, sync::LazyLock};

use regex::Regex;

use crate::{
    HasSpan, Parser, Span,
    attributes::Attrlist,
    blocks::{
        Block, ContentModel, IsBlock, metadata::BlockMetadata, parse_utils::parse_blocks_until,
    },
    content::{Content, SubstitutionGroup, strip_footnote_marker_spans},
    document::{InterpretedValue, RefType},
    internal::debug::DebugSliceReference,
    parser::XrefSignifier,
    span::MatchedItem,
    strings::CowStr,
    warnings::{Warning, WarningType},
};

/// Sections partition the document into a content hierarchy. A section is an
/// implicit enclosure. Each section begins with a title and ends at the next
/// sibling section, ancestor section, or end of document. Nested section levels
/// must be sequential.
#[derive(Clone, Eq, PartialEq)]
pub struct SectionBlock<'src> {
    level: usize,
    section_title: Content<'src>,
    blocks: Vec<Block<'src>>,
    source: Span<'src>,
    title_source: Option<Span<'src>>,
    title: Option<String>,
    anchor: Option<Span<'src>>,
    anchor_reftext: Option<Span<'src>>,
    attrlist: Option<Attrlist<'src>>,
    section_type: SectionType,
    section_id: Option<String>,
    caption: Option<String>,
    section_number: Option<SectionNumber>,
}

impl<'src> SectionBlock<'src> {
    pub(crate) fn parse(
        metadata: &BlockMetadata<'src>,
        parser: &mut Parser,
        warnings: &mut Vec<Warning<'src>>,
    ) -> Option<MatchedItem<'src, Self>> {
        let discrete = metadata.is_discrete();

        let source = metadata.block_start.discard_empty_lines();

        // The heading's effective level folds in the running `leveloffset`
        // document attribute. A positive offset (the usual case, from
        // `include::[leveloffset=+1]`) pushes headings down — notably promoting
        // an included file's level-0 document title (`=`) into a real section —
        // while a negative offset pulls them up. A heading whose effective level
        // is below 1 is rejected as an unsupported level-0 heading (the warning
        // is raised inside `parse_title_line`).
        let level_and_title = parse_title_line(source, parser.level_offset(), warnings)?;

        // Take a snapshot of `sectids` value before reading child blocks because
        // the value might be altered while parsing.
        let sectids = parser.is_attribute_set("sectids");

        let level = level_and_title.item.0;

        // Assign the section type. At level 1, we look for an `appendix` section style;
        // at all other levels, we inherit the section type from parent.
        let section_type = if discrete {
            SectionType::Discrete
        } else if level == 1 {
            let section_type = if let Some(ref attrlist) = metadata.attrlist
                && let Some(block_style) = attrlist.block_style()
                && block_style == "appendix"
            {
                SectionType::Appendix
            } else {
                SectionType::Normal
            };
            parser.topmost_section_type = section_type;
            section_type
        } else {
            parser.topmost_section_type
        };

        // Assign section number BEFORE parsing child blocks so that sections are
        // numbered in document order (parent before children).
        //
        // Appendix sections are lettered (A, B, ...) independently of `sectnums`
        // because their title prefix is governed by the `appendix-caption`
        // attribute (see the "Appendix label" section of the spec). An appendix
        // root — the section that directly carries the `appendix` style —
        // therefore always advances the appendix counter so it (and the numbering
        // of any subsection) can derive its letter, even when `sectnums` is unset.
        let sectnums_active =
            parser.is_attribute_set("sectnums") && level <= parser.sectnumlevels && !discrete;

        let is_appendix_root = !discrete && level == 1 && section_type == SectionType::Appendix;

        // A cross-reference builds `full`/`short` xrefstyle text from a section's
        // signifier and number, but only when the section has a number *and* no
        // explicit reftext (an explicit reftext is used verbatim instead). An
        // explicit reftext can come from a `reftext` attribute or the second
        // field of a `[[id,reftext]]` block anchor.
        let has_explicit_reftext = metadata
            .attrlist
            .as_ref()
            .and_then(|a| a.named_attribute("reftext"))
            .is_some()
            || metadata.anchor_reftext.is_some();

        let (section_number, caption, xref_signifier) = if is_appendix_root {
            // The appendix letter is resolved through the `appendix-number`
            // counter (mirroring Ruby Asciidoctor's `Document#counter
            // 'appendix-number', 'A'`): each appendix advances the counter, so
            // a document-set `appendix-number` value is the letter *before* the
            // first appendix (`:appendix-number: α` letters the appendices β,
            // γ, …) and the attribute always reads back as the current letter.
            let letter = parser.counter("appendix-number", Some("A"));

            parser
                .last_appendix_section_number
                .assign_next_number(level);
            parser.last_appendix_section_number.appendix_letter = Some(letter);

            let number = parser.last_appendix_section_number.clone();
            let caption = appendix_caption(parser, &number);

            // An appendix is always lettered and its title is emphasized, even
            // when `sectnums` is unset; its reference signifier is
            // `appendix-refsig`.
            let signifier = (!has_explicit_reftext).then(|| XrefSignifier {
                label: join_signifier(
                    parser.attribute_value("appendix-refsig").as_maybe_str(),
                    &number.to_string(),
                ),
                emphasize: true,
            });
            let section_number = if sectnums_active { Some(number) } else { None };
            (section_number, Some(caption), signifier)
        } else if sectnums_active {
            let number = parser.assign_section_number(level);
            let signifier = (!has_explicit_reftext).then(|| XrefSignifier {
                label: join_signifier(
                    parser.attribute_value("section-refsig").as_maybe_str(),
                    &number.to_string(),
                ),
                emphasize: false,
            });
            (Some(number), None, signifier)
        } else {
            (None, None, None)
        };

        let mut most_recent_level = level;

        // Apply the title's substitutions BEFORE parsing the section body, so
        // that a `footnote:[…]` macro in the title is numbered ahead of any
        // footnotes in the body (document order: the title precedes its body).
        // Substituting before the body also means the title only sees document
        // attributes defined ahead of it, not ones its body sets later.
        //
        // Asciidoctor instead converts headings eagerly and out of document
        // order (to build IDs and cross-reference text), which numbers heading
        // footnotes out of sequence. The crate deliberately diverges toward
        // straightforward document-order numbering; see
        // https://github.com/asciidoc-rs/asciidoc-parser/issues/594.
        //
        // A footnote in the title is a real, document-order footnote, but its
        // marker must not leak into the section's reference text (an xref's link
        // text) or auto-generated ID. Marking the title's footnote markers with
        // sentinels lets those be excised below from a single render — no second
        // substitution pass, so counters and attribute-expanded footnotes are
        // processed exactly once.
        let mut section_title = Content::from(level_and_title.item.1);
        parser.mark_footnote_spans.set(true);
        SubstitutionGroup::Title.apply(&mut section_title, parser, metadata.attrlist.as_ref());
        parser.mark_footnote_spans.set(false);

        // The footnote-free rendering of the title, for the reference text and
        // auto-generated ID; a no-op string copy when the title had no footnote.
        let title_reftext = strip_footnote_marker_spans(section_title.rendered());

        // Strip the now-consumed sentinels from the title itself, keeping the
        // footnote marker so the heading still renders it.
        section_title.remove_footnote_marker_sentinels();

        // A section carrying the `bibliography` style implicitly adds that style
        // to each top-level unordered list in its body (see the "Bibliography
        // section syntax" section of the spec). Record that we are parsing such a
        // section's body so `ListBlock::parse` can detect it, and restore the
        // previous value afterward so the style does not leak into sibling
        // sections (or, via a non-bibliography subsection, into its children).
        let is_bibliography_section = !discrete
            && metadata
                .attrlist
                .as_ref()
                .and_then(|attrlist| attrlist.block_style())
                == Some("bibliography");

        let previously_in_bibliography_section = parser.parsing_bibliography_section_body;
        parser.parsing_bibliography_section_body = is_bibliography_section;

        // A block title above a section heading does not become the section's
        // title; it is carried over to the first block inside the section
        // (matching Asciidoctor). Stash it on the parser: the next block parsed
        // claims it — usually the section's first child, or (when the section
        // body is empty) the sibling section that follows, which re-stashes it
        // for its own first block. A discrete heading is an ordinary block, not
        // a section, so it keeps its title. See `Block::parse_internal` for the
        // claiming side.
        if !discrete && metadata.title.is_some() {
            parser.pending_block_title = metadata.title.clone();
        }

        let mut maw_blocks = parse_blocks_until(
            level_and_title.after,
            |i, parser| {
                discrete
                    || peer_or_ancestor_section(*i, level, &mut most_recent_level, warnings, parser)
            },
            parser,
        );

        parser.parsing_bibliography_section_body = previously_in_bibliography_section;

        let blocks = maw_blocks.item;
        let source = metadata.source.trim_remainder(blocks.after);

        let proposed_base_id = generate_section_id(&title_reftext, parser);

        let manual_id = metadata
            .attrlist
            .as_ref()
            .and_then(|a| a.id())
            .or_else(|| metadata.anchor.as_ref().map(|anchor| anchor.data()));

        // Reftext precedence mirrors `Block::block_reftext`: an explicit
        // `reftext` attribute, then a `[[id,reftext]]` anchor reftext, then the
        // section title.
        let reftext = metadata
            .attrlist
            .as_ref()
            .and_then(|a| a.named_attribute("reftext").map(|a| a.value()))
            .or_else(|| metadata.anchor_reftext.as_ref().map(|span| span.data()))
            .unwrap_or(&title_reftext);

        let section_id = if sectids && manual_id.is_none() {
            let id = parser.generate_and_register_unique_id(
                &proposed_base_id,
                Some(reftext),
                RefType::Section,
            );
            if let Some(signifier) = xref_signifier {
                parser.set_ref_signifier(&id, signifier);
            }
            Some(id)
        } else {
            if let Some(manual_id) = manual_id {
                match parser.register_ref(manual_id, Some(reftext), RefType::Section) {
                    Ok(()) => {
                        if let Some(signifier) = xref_signifier {
                            parser.set_ref_signifier(manual_id, signifier);
                        }
                    }
                    Err(_duplicate_error) => {
                        warnings.push(Warning {
                            source: metadata.source.trim_remainder(level_and_title.after),
                            warning: WarningType::DuplicateId(manual_id.to_string()),
                            origin: None,
                        });
                    }
                }
            }

            None
        };

        // Restore "normal" top-level section type if exiting a level 1 appendix.
        if level == 1 && !discrete {
            parser.topmost_section_type = SectionType::Normal;
        }

        warnings.append(&mut maw_blocks.warnings);

        Some(MatchedItem {
            item: Self {
                level,
                section_title,
                blocks: blocks.item,
                source: source.trim_trailing_whitespace(),

                // A non-discrete section never keeps a block title; it was
                // stashed above for the next block parsed to claim.
                title_source: if discrete {
                    metadata.title_source
                } else {
                    None
                },
                title: if discrete {
                    metadata.title.clone()
                } else {
                    None
                },
                anchor: metadata.anchor,
                anchor_reftext: metadata.anchor_reftext,
                attrlist: metadata.attrlist.clone(),
                section_type,
                section_id,
                caption,
                section_number,
            },
            after: blocks.after,
        })
    }

    /// Return the section's level.
    ///
    /// The section title must be prefixed with a section marker, which
    /// indicates the section level. The number of equal signs in the marker
    /// represents the section level using a 0-based index (e.g., two equal
    /// signs represents level 1). A section marker can range from two to six
    /// equal signs and must be followed by a space.
    ///
    /// This function will return an integer between 1 and 5.
    pub fn level(&self) -> usize {
        self.level
    }

    /// Return a [`Span`] containing the section title source.
    pub fn section_title_source(&self) -> Span<'src> {
        self.section_title.original()
    }

    /// Return the processed section title after substitutions have been
    /// applied.
    pub fn section_title(&'src self) -> &'src str {
        self.section_title.rendered()
    }

    /// Return the type of this section (normal or appendix).
    pub fn section_type(&'src self) -> SectionType {
        self.section_type
    }

    /// Accessor intended to be used for testing only. Use the `id()` accessor
    /// in the `IsBlock` trait to retrieve the effective ID for this block,
    /// which considers both auto-generated IDs and manually-set IDs.
    #[cfg(test)]
    pub(crate) fn section_id(&'src self) -> Option<&'src str> {
        self.section_id.as_deref()
    }

    /// Return the section number assigned to this section, if any.
    pub fn section_number(&'src self) -> Option<&'src SectionNumber> {
        self.section_number.as_ref()
    }
}

/// Builds the appendix title prefix (caption) for an appendix root section.
///
/// The prefix combines the `appendix-caption` label (which defaults to
/// "`Appendix`"), the appendix letter (A, B, ...), and a separator. When
/// `appendix-caption` is set, the prefix is `"<label> <letter>: "`; when it is
/// unset (or empty), the label is dropped, leaving `"<letter>. "`. This mirrors
/// Ruby Asciidoctor.
fn appendix_caption(parser: &Parser, number: &SectionNumber) -> String {
    let letter = number.to_string();
    match parser.attribute_value("appendix-caption") {
        InterpretedValue::Value(label) if !label.is_empty() => format!("{label} {letter}: "),
        _ => format!("{letter}. "),
    }
}

/// Combines a reference signifier with a reference number for the
/// `full`/`short` xrefstyle label. When the signifier is set the label is
/// `"<signifier> <number>"` (e.g. `"Section 2.3"`); when it is unset (or empty)
/// — as after `:!section-refsig:` — the signifier is dropped and only the
/// number remains.
fn join_signifier(signifier: Option<&str>, number: &str) -> String {
    match signifier {
        Some(signifier) if !signifier.is_empty() => format!("{signifier} {number}"),
        _ => number.to_string(),
    }
}

impl<'src> IsBlock<'src> for SectionBlock<'src> {
    fn content_model(&self) -> ContentModel {
        ContentModel::Compound
    }

    fn content_mut(&mut self) -> Option<&mut Content<'src>> {
        // The section title is the section's own resolvable content.
        Some(&mut self.section_title)
    }

    fn raw_context(&self) -> CowStr<'src> {
        "section".into()
    }

    fn nested_blocks_mut(&mut self) -> &mut [Block<'src>] {
        &mut self.blocks
    }

    fn nested_blocks(&'src self) -> Iter<'src, Block<'src>> {
        self.blocks.iter()
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

    fn caption(&self) -> Option<&str> {
        self.caption.as_deref()
    }

    fn id(&'src self) -> Option<&'src str> {
        // First try the default implementation (explicit IDs from anchor or attrlist)
        self.anchor()
            .map(|a| a.data())
            .or_else(|| self.attrlist().and_then(|attrlist| attrlist.id()))
            // Fall back to auto-generated ID if no explicit ID is set
            .or(self.section_id.as_deref())
    }
}

impl<'src> HasSpan<'src> for SectionBlock<'src> {
    fn span(&self) -> Span<'src> {
        self.source
    }
}

impl std::fmt::Debug for SectionBlock<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SectionBlock")
            .field("level", &self.level)
            .field("section_title", &self.section_title)
            .field("blocks", &DebugSliceReference(&self.blocks))
            .field("source", &self.source)
            .field("title_source", &self.title_source)
            .field("title", &self.title)
            .field("anchor", &self.anchor)
            .field("anchor_reftext", &self.anchor_reftext)
            .field("attrlist", &self.attrlist)
            .field("section_type", &self.section_type)
            .field("section_id", &self.section_id)
            .field("caption", &self.caption)
            .field("section_number", &self.section_number)
            .finish()
    }
}

/// The lowest and highest levels a section heading may occupy. A syntactic
/// heading level (0 for `=`, up to 5 for `======`) shifted by `leveloffset`
/// must land within this inclusive range; a result outside it is clamped.
const MIN_SECTION_LEVEL: i32 = 1;
const MAX_SECTION_LEVEL: i32 = 5;

/// Strips an optional symmetric ATX title close from `title`: a trailing run of
/// `marker` exactly `count` long, preceded by whitespace (e.g. the ` ==` in
/// `== Title ==`). A run that does not match the opening marker (`== Title
/// ===`) or is not preceded by whitespace (`== Title==`) is left intact, and a
/// title consisting only of the close is left intact. Mirrors the trailing
/// `(?: +\1)?` group of Asciidoctor's section-title regex.
pub(crate) fn strip_symmetric_title_close(title: Span<'_>, marker: char, count: usize) -> Span<'_> {
    // The close must be separated from the title by an ASCII blank (space or
    // tab), matching Asciidoctor's `CG_BLANK` (`[ \t]`) — not arbitrary Unicode
    // whitespace, so e.g. `== Title<NBSP>==` keeps its `==` as title text.
    const BLANK: [char; 2] = [' ', '\t'];
    let close = marker.to_string().repeat(count);
    match title.data().strip_suffix(&close) {
        Some(without_close)
            if without_close.ends_with(BLANK)
                && !without_close.trim_end_matches(BLANK).is_empty() =>
        {
            title.slice_to(..without_close.trim_end_matches(BLANK).len())
        }
        _ => title,
    }
}

/// Parses a section title line, returning the section's *effective* level
/// (with `offset`, the running `leveloffset`, already applied) and the span of
/// the title text.
///
/// The syntactic level is 0-based: a bare `=` is 0, `==` is 1, up to `======`
/// at 5. `offset` shifts it to the effective level, which is then constrained
/// to the [`MIN_SECTION_LEVEL`]..=[`MAX_SECTION_LEVEL`] range:
///
/// * A bare `=` (syntactic level 0) that no positive offset lifts to level 1 or
///   beyond has no section representation; it is rejected as an unsupported
///   level-0 heading (recording a warning), preserving the single-document-
///   title rule.
/// * Any other heading whose effective level falls outside the supported range
///   is clamped to the nearest valid level and a warning is recorded.
fn parse_title_line<'src>(
    source: Span<'src>,
    offset: i32,
    warnings: &mut Vec<Warning<'src>>,
) -> Option<MatchedItem<'src, (usize, Span<'src>)>> {
    let mi = source.take_non_empty_line()?;
    let mut line = mi.item;

    let mut count = 0;

    let marker_char = if line.starts_with('=') { '=' } else { '#' };

    if marker_char == '=' {
        while let Some(mi) = line.take_prefix("=") {
            count += 1;
            line = mi.after;
        }
    } else {
        while let Some(mi) = line.take_prefix("#") {
            count += 1;
            line = mi.after;
        }
    }

    if count == 0 {
        return None;
    }

    if count > 6 {
        warnings.push(Warning {
            source: source.take_normalized_line().item,
            warning: WarningType::SectionHeadingLevelExceedsMaximum(count - 1),
            origin: None,
        });

        return None;
    }

    // Fold in the running `leveloffset`. `saturating_add` keeps a hostile
    // offset (e.g. an absolute `:leveloffset:` near `i32::MAX`) from
    // overflowing — a panic in debug builds and a wrap in release builds — the
    // syntactic level itself is at most 5.
    let syntactic_level = (count - 1) as i32;
    let effective_level = syntactic_level.saturating_add(offset);

    // A bare `=` (syntactic level 0) that no positive offset lifts to level 1
    // or beyond is a document title appearing in the body, which is not a
    // section (the single-document-title rule). Decline it exactly as an
    // un-offset level-0 heading is declined, rather than clamping it into a
    // section. This is checked before the whitespace requirement below so a
    // spaceless `=blah` is still reported, matching a bare level-0 heading.
    if syntactic_level == 0 && effective_level < MIN_SECTION_LEVEL {
        warnings.push(Warning {
            source: source.take_normalized_line().item,
            warning: WarningType::Level0SectionHeadingNotSupported,
            origin: None,
        });

        return None;
    }

    // The marker must be followed by whitespace to be a section title at all;
    // validate that before clamping the level so a non-title line such as
    // `==x` is declined quietly, without a spurious out-of-range warning.
    let title = line.take_required_whitespace()?;

    let title_span = strip_symmetric_title_close(title.after, marker_char, count);

    // A real section heading whose offset-adjusted level lands outside the
    // supported 1..=5 range is clamped into range and reported, rather than
    // producing an out-of-range (or, under a hostile offset, absurd) level.
    let level = if effective_level < MIN_SECTION_LEVEL {
        warnings.push(Warning {
            source: source.take_normalized_line().item,
            warning: WarningType::SectionHeadingLevelOutOfRange(
                effective_level,
                MIN_SECTION_LEVEL as usize,
            ),
            origin: None,
        });
        MIN_SECTION_LEVEL as usize
    } else if effective_level > MAX_SECTION_LEVEL {
        warnings.push(Warning {
            source: source.take_normalized_line().item,
            warning: WarningType::SectionHeadingLevelOutOfRange(
                effective_level,
                MAX_SECTION_LEVEL as usize,
            ),
            origin: None,
        });
        MAX_SECTION_LEVEL as usize
    } else {
        effective_level as usize
    };

    Some(MatchedItem {
        item: (level, title_span),
        after: mi.after,
    })
}

fn peer_or_ancestor_section<'src>(
    source: Span<'src>,
    level: usize,
    most_recent_level: &mut usize,
    warnings: &mut Vec<Warning<'src>>,
    parser: &Parser,
) -> bool {
    // Skip over any block metadata (title, anchor, attrlist) to find the actual
    // section line. We create a temporary parser to avoid modifying the real
    // parser state.
    let mut temp_parser = Parser::default();

    let block_metadata_maw = BlockMetadata::parse(source, &mut temp_parser);

    let block_metadata = block_metadata_maw.item;
    if block_metadata.is_discrete() {
        return false;
    }

    let source_after_metadata = block_metadata.block_start;

    // Compare effective levels: the boundary heading's `leveloffset` is read
    // from the *live* parser (every block up to this point, including any
    // `:leveloffset:` attribute entry, has already been applied), while `level`
    // is the current section's own effective level. A heading whose effective
    // level is below 1 has no section representation, so `parse_title_line`
    // returns `None` and it is treated as ordinary content — exactly as an
    // un-offset level-0 heading would be.
    //
    // Any warnings the heading would raise (a clamped level, an unsupported
    // level-0 heading, ...) are discarded here: this is only a look-ahead to
    // find the section boundary, and the heading is parsed again — recording
    // those warnings once — either as a child block of this section or in the
    // enclosing scope once this section ends.
    let mut ignored_warnings = vec![];
    if let Some(mi) = parse_title_line(
        source_after_metadata,
        parser.level_offset(),
        &mut ignored_warnings,
    ) {
        let found_level = mi.item.0;

        if found_level > *most_recent_level + 1 {
            warnings.push(Warning {
                source: source.take_normalized_line().item,
                warning: WarningType::SectionHeadingLevelSkipped(*most_recent_level, found_level),
                origin: None,
            });
        }

        *most_recent_level = found_level;

        found_level <= level
    } else {
        false
    }
}

/// Records a "section title out of sequence" warning for a *top-level* section
/// whose level skips ahead of level 1 — the document root's expected first
/// child level. The nested case (a section skipping a level under its *parent
/// section*) is handled during parsing by [`peer_or_ancestor_section`]; this
/// covers the document-root case (e.g. `= Doc` followed directly by `=== X`),
/// which that boundary check never sees.
///
/// At most one such warning is possible: any later top-level section is a peer
/// or ancestor of an earlier one (a deeper heading becomes a *child* instead),
/// so it can never skip ahead of `most_recent_level + 1`.
///
/// Discrete headings are not part of the section sequence and are skipped. The
/// caller restricts this to titled, non-`fragment` documents (a title-less
/// document or a section fragment has no level-0 root to sequence against).
pub(crate) fn root_section_sequence_warnings<'src>(blocks: &[Block<'src>]) -> Vec<Warning<'src>> {
    let mut warnings = vec![];
    let mut most_recent_level = 0;

    for block in blocks {
        let Block::Section(section) = block else {
            continue;
        };

        if section.section_type() == SectionType::Discrete {
            continue;
        }

        let found_level = section.level();

        if found_level > most_recent_level + 1 {
            warnings.push(Warning {
                source: section.span().take_normalized_line().item,
                warning: WarningType::SectionHeadingLevelSkipped(most_recent_level, found_level),
                origin: None,
            });
        }

        most_recent_level = found_level;
    }

    warnings
}

/// Propose a section ID from the section title.
///
/// This function is called when (1) no `id` attribute is specified explicitly,
/// and (2) the `sectids` document attribute is set.
///
/// The ID is generated as described in the AsciiDoc language definition in [How
/// a section ID is computed].
///
/// [How a section ID is computed](https://docs.asciidoctor.org/asciidoc/latest/sections/auto-ids/)
fn generate_section_id(title: &str, parser: &Parser) -> String {
    let idprefix = parser
        .attribute_value("idprefix")
        .as_maybe_str()
        .unwrap_or_default()
        .to_owned();

    let idseparator = parser
        .attribute_value("idseparator")
        .as_maybe_str()
        .unwrap_or_default()
        .to_owned();

    let mut gen_id = title.to_lowercase().to_owned();

    #[allow(clippy::unwrap_used)]
    static INVALID_SECTION_ID_CHARS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"<[^>]+>|&lt;[^&]*&gt;|&(?:[a-z][a-z]+\d{0,2}|#\d{2,5}|#x[\da-f]{2,4});|[^ \w\-.]+",
        )
        .unwrap()
    });

    gen_id = INVALID_SECTION_ID_CHARS
        .replace_all(&gen_id, "")
        .to_string();

    // Take only first character of separator if multiple provided.
    let sep = idseparator
        .chars()
        .next()
        .map(|s| s.to_string())
        .unwrap_or_default();

    gen_id = gen_id.replace([' ', '.', '-'], &sep);

    if !sep.is_empty() {
        while gen_id.contains(&format!("{}{}", sep, sep)) {
            gen_id = gen_id.replace(&format!("{}{}", sep, sep), &sep);
        }

        if gen_id.ends_with(&sep) {
            gen_id.pop();
        }

        // Strip a leading separator (e.g. from a title beginning with a space or
        // hyphen) before the prefix is applied, matching Ruby Asciidoctor. This
        // keeps a leading separator out of the final ID and avoids doubling it
        // up against a non-empty `idprefix` (e.g. `=== {sp}Heading` → `_heading`,
        // not `__heading`).
        if gen_id.starts_with(&sep) {
            gen_id = gen_id[sep.len()..].to_string();
        }
    }

    format!("{idprefix}{gen_id}")
}

/// Represents the type of a section.
///
/// This crate currently supports the `appendix` section style, which results in
/// special section numbering. All other sections are treated as `Normal`
/// sections.
#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub enum SectionType {
    /// Most sections are of this type.
    #[default]
    Normal,

    /// Represents a section with the style `appendix`.
    Appendix,

    /// Represents a discrete section heading.
    /// A discrete section heading will have no nested blocks.
    Discrete,
}

impl std::fmt::Debug for SectionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SectionType::Normal => write!(f, "SectionType::Normal"),
            SectionType::Appendix => write!(f, "SectionType::Appendix"),
            SectionType::Discrete => write!(f, "SectionType::Discrete"),
        }
    }
}

/// Represents an assigned section number.
///
/// Section numbers aren't assigned by default, but can be enabled using the
/// `sectnums` and `sectnumlevels` attributes as described in [Section Numbers].
///
/// [Section Numbers]: https://docs.asciidoctor.org/asciidoc/latest/sections/numbers/
#[derive(Clone, Default, Eq, PartialEq)]
pub struct SectionNumber {
    pub(crate) section_type: SectionType,
    pub(crate) components: Vec<usize>,

    // The letter (or, more generally, counter value) assigned to the appendix
    // this number belongs to, resolved from the `appendix-number` counter
    // (e.g. `"A"`, or `"β"` when the document sets `:appendix-number: α`).
    // Replaces the first component when the number is displayed. `None` for
    // normal section numbers.
    pub(crate) appendix_letter: Option<String>,
}

impl SectionNumber {
    /// Generate the next section number for the specified level, based on this
    /// section number.
    ///
    /// `level` should be between 1 and 5, though this is not enforced.
    pub(crate) fn assign_next_number(&mut self, level: usize) {
        // Drop any ID components beyond the desired level.
        self.components.truncate(level);

        if self.components.len() < level {
            self.components.resize(level, 1);
        } else if level > 0
            && let Some(component) = self.components.get_mut(level - 1)
        {
            *component += 1;
        }
    }

    /// Iterate over the components of the section number.
    pub fn components(&self) -> &[usize] {
        &self.components
    }

    /// Return the letter (or, more generally, `appendix-number` counter value)
    /// assigned to the appendix this number belongs to (e.g. `"A"`, or `"β"`
    /// when the document sets `:appendix-number: α`).
    ///
    /// Returns `None` for normal section numbers.
    pub fn appendix_letter(&self) -> Option<&str> {
        self.appendix_letter.as_deref()
    }
}

impl fmt::Display for SectionNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(
            &self
                .components
                .iter()
                .enumerate()
                .map(|(index, x)| {
                    if index == 0 && self.section_type == SectionType::Appendix {
                        // A parsed appendix number always carries its letter;
                        // the A, B, … derivation covers directly-constructed
                        // values that don't.
                        if let Some(letter) = &self.appendix_letter {
                            letter.clone()
                        } else {
                            char::from_u32(b'A' as u32 + (x - 1) as u32)
                                .unwrap_or('?')
                                .to_string()
                        }
                    } else {
                        x.to_string()
                    }
                })
                .collect::<Vec<String>>()
                .join("."),
        )
    }
}

impl fmt::Debug for SectionNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SectionNumber")
            .field("section_type", &self.section_type)
            .field("components", &DebugSliceReference(&self.components))
            .field("appendix_letter", &self.appendix_letter)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use crate::{
        blocks::{metadata::BlockMetadata, section::SectionType},
        tests::prelude::*,
    };

    #[test]
    fn impl_clone() {
        // Silly test to mark the #[derive(...)] line as covered.
        let mut parser = Parser::default();
        let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

        let b1 = crate::blocks::SectionBlock::parse(
            &BlockMetadata::new("== Section Title"),
            &mut parser,
            &mut warnings,
        )
        .unwrap();

        let b2 = b1.item.clone();
        assert_eq!(b1.item, b2);
    }

    #[test]
    fn err_empty_source() {
        let mut parser = Parser::default();
        let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

        assert!(
            crate::blocks::SectionBlock::parse(&BlockMetadata::new(""), &mut parser, &mut warnings)
                .is_none()
        );
    }

    #[test]
    fn err_only_spaces() {
        let mut parser = Parser::default();
        let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

        assert!(
            crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("    "),
                &mut parser,
                &mut warnings
            )
            .is_none()
        );
    }

    #[test]
    fn err_not_section() {
        let mut parser = Parser::default();
        let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

        assert!(
            crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("blah blah"),
                &mut parser,
                &mut warnings
            )
            .is_none()
        );
    }

    mod asciidoc_style_headers {
        use std::ops::Deref;

        use crate::{
            blocks::{ContentModel, MediaType, metadata::BlockMetadata, section::SectionType},
            tests::prelude::*,
        };

        #[test]
        fn err_missing_space_before_title() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            assert!(
                crate::blocks::SectionBlock::parse(
                    &BlockMetadata::new("=blah blah"),
                    &mut parser,
                    &mut warnings
                )
                .is_none()
            );
        }

        #[test]
        fn simplest_section_block() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("== Section Title"),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(mi.item.content_model(), ContentModel::Compound);
            assert_eq!(mi.item.raw_context().deref(), "section");
            assert_eq!(mi.item.resolved_context().deref(), "section");
            assert!(mi.item.declared_style().is_none());
            assert_eq!(mi.item.id().unwrap(), "_section_title");
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

            assert_eq!(
                mi.item,
                SectionBlock {
                    level: 1,
                    section_title: Content {
                        original: Span {
                            data: "Section Title",
                            line: 1,
                            col: 4,
                            offset: 3,
                        },
                        rendered: "Section Title",
                    },
                    blocks: &[],
                    source: Span {
                        data: "== Section Title",
                        line: 1,
                        col: 1,
                        offset: 0,
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
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 17,
                    offset: 16
                }
            );
        }

        #[test]
        fn has_child_block() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("== Section Title\n\nabc"),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(mi.item.content_model(), ContentModel::Compound);
            assert_eq!(mi.item.raw_context().deref(), "section");
            assert_eq!(mi.item.resolved_context().deref(), "section");
            assert!(mi.item.declared_style().is_none());
            assert_eq!(mi.item.id().unwrap(), "_section_title");
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

            assert_eq!(
                mi.item,
                SectionBlock {
                    level: 1,
                    section_title: Content {
                        original: Span {
                            data: "Section Title",
                            line: 1,
                            col: 4,
                            offset: 3,
                        },
                        rendered: "Section Title",
                    },
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "abc",
                                line: 3,
                                col: 1,
                                offset: 18,
                            },
                            rendered: "abc",
                        },
                        source: Span {
                            data: "abc",
                            line: 3,
                            col: 1,
                            offset: 18,
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
                        data: "== Section Title\n\nabc",
                        line: 1,
                        col: 1,
                        offset: 0,
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
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 3,
                    col: 4,
                    offset: 21
                }
            );
        }

        #[test]
        fn has_macro_block_with_extra_blank_line() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new(
                    "== Section Title\n\nimage::bar[alt=Sunset,width=300,height=400]\n\n",
                ),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(mi.item.content_model(), ContentModel::Compound);
            assert_eq!(mi.item.raw_context().deref(), "section");
            assert_eq!(mi.item.resolved_context().deref(), "section");
            assert!(mi.item.declared_style().is_none());
            assert_eq!(mi.item.id().unwrap(), "_section_title");
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

            assert_eq!(
                mi.item,
                SectionBlock {
                    level: 1,
                    section_title: Content {
                        original: Span {
                            data: "Section Title",
                            line: 1,
                            col: 4,
                            offset: 3,
                        },
                        rendered: "Section Title",
                    },
                    blocks: &[Block::Media(MediaBlock {
                        type_: MediaType::Image,
                        target: Span {
                            data: "bar",
                            line: 3,
                            col: 8,
                            offset: 25,
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
                                }
                            ],
                            anchor: None,
                            source: Span {
                                data: "alt=Sunset,width=300,height=400",
                                line: 3,
                                col: 12,
                                offset: 29,
                            }
                        },
                        source: Span {
                            data: "image::bar[alt=Sunset,width=300,height=400]",
                            line: 3,
                            col: 1,
                            offset: 18,
                        },
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    })],
                    source: Span {
                        data: "== Section Title\n\nimage::bar[alt=Sunset,width=300,height=400]",
                        line: 1,
                        col: 1,
                        offset: 0,
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
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 5,
                    col: 1,
                    offset: 63
                }
            );
        }

        #[test]
        fn has_child_block_with_errors() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new(
                    "== Section Title\n\nimage::bar[alt=Sunset,width=300,,height=400]",
                ),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(mi.item.content_model(), ContentModel::Compound);
            assert_eq!(mi.item.raw_context().deref(), "section");
            assert_eq!(mi.item.resolved_context().deref(), "section");
            assert!(mi.item.declared_style().is_none());
            assert_eq!(mi.item.id().unwrap(), "_section_title");
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

            assert_eq!(
                mi.item,
                SectionBlock {
                    level: 1,
                    section_title: Content {
                        original: Span {
                            data: "Section Title",
                            line: 1,
                            col: 4,
                            offset: 3,
                        },
                        rendered: "Section Title",
                    },
                    blocks: &[Block::Media(MediaBlock {
                        type_: MediaType::Image,
                        target: Span {
                            data: "bar",
                            line: 3,
                            col: 8,
                            offset: 25,
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
                                }
                            ],
                            anchor: None,
                            source: Span {
                                data: "alt=Sunset,width=300,,height=400",
                                line: 3,
                                col: 12,
                                offset: 29,
                            }
                        },
                        source: Span {
                            data: "image::bar[alt=Sunset,width=300,,height=400]",
                            line: 3,
                            col: 1,
                            offset: 18,
                        },
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    })],
                    source: Span {
                        data: "== Section Title\n\nimage::bar[alt=Sunset,width=300,,height=400]",
                        line: 1,
                        col: 1,
                        offset: 0,
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
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 3,
                    col: 45,
                    offset: 62
                }
            );

            assert_eq!(
                warnings,
                vec![Warning {
                    source: Span {
                        data: "alt=Sunset,width=300,,height=400",
                        line: 3,
                        col: 12,
                        offset: 29,
                    },
                    warning: WarningType::EmptyAttributeValue,
                }]
            );
        }

        #[test]
        fn dont_stop_at_child_section() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("== Section Title\n\nabc\n\n=== Section 2\n\ndef"),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(mi.item.content_model(), ContentModel::Compound);
            assert_eq!(mi.item.raw_context().deref(), "section");
            assert_eq!(mi.item.resolved_context().deref(), "section");
            assert!(mi.item.declared_style().is_none());
            assert_eq!(mi.item.id().unwrap(), "_section_title");
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

            assert_eq!(
                mi.item,
                SectionBlock {
                    level: 1,
                    section_title: Content {
                        original: Span {
                            data: "Section Title",
                            line: 1,
                            col: 4,
                            offset: 3,
                        },
                        rendered: "Section Title",
                    },
                    blocks: &[
                        Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "abc",
                                    line: 3,
                                    col: 1,
                                    offset: 18,
                                },
                                rendered: "abc",
                            },
                            source: Span {
                                data: "abc",
                                line: 3,
                                col: 1,
                                offset: 18,
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
                        Block::Section(SectionBlock {
                            level: 2,
                            section_title: Content {
                                original: Span {
                                    data: "Section 2",
                                    line: 5,
                                    col: 5,
                                    offset: 27,
                                },
                                rendered: "Section 2",
                            },
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "def",
                                        line: 7,
                                        col: 1,
                                        offset: 38,
                                    },
                                    rendered: "def",
                                },
                                source: Span {
                                    data: "def",
                                    line: 7,
                                    col: 1,
                                    offset: 38,
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
                                data: "=== Section 2\n\ndef",
                                line: 5,
                                col: 1,
                                offset: 23,
                            },
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                            section_type: SectionType::Normal,
                            section_id: Some("_section_2"),
                            caption: None,
                            section_number: None,
                        })
                    ],
                    source: Span {
                        data: "== Section Title\n\nabc\n\n=== Section 2\n\ndef",
                        line: 1,
                        col: 1,
                        offset: 0,
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
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 7,
                    col: 4,
                    offset: 41
                }
            );
        }

        #[test]
        fn stop_at_peer_section() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("== Section Title\n\nabc\n\n== Section 2\n\ndef"),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(mi.item.content_model(), ContentModel::Compound);
            assert_eq!(mi.item.raw_context().deref(), "section");
            assert_eq!(mi.item.resolved_context().deref(), "section");
            assert!(mi.item.declared_style().is_none());
            assert_eq!(mi.item.id().unwrap(), "_section_title");
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

            assert_eq!(
                mi.item,
                SectionBlock {
                    level: 1,
                    section_title: Content {
                        original: Span {
                            data: "Section Title",
                            line: 1,
                            col: 4,
                            offset: 3,
                        },
                        rendered: "Section Title",
                    },
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "abc",
                                line: 3,
                                col: 1,
                                offset: 18,
                            },
                            rendered: "abc",
                        },
                        source: Span {
                            data: "abc",
                            line: 3,
                            col: 1,
                            offset: 18,
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
                        data: "== Section Title\n\nabc",
                        line: 1,
                        col: 1,
                        offset: 0,
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
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "== Section 2\n\ndef",
                    line: 5,
                    col: 1,
                    offset: 23
                }
            );
        }

        #[test]
        fn stop_at_ancestor_section() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("=== Section Title\n\nabc\n\n== Section 2\n\ndef"),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(mi.item.content_model(), ContentModel::Compound);
            assert_eq!(mi.item.raw_context().deref(), "section");
            assert_eq!(mi.item.resolved_context().deref(), "section");
            assert!(mi.item.declared_style().is_none());
            assert_eq!(mi.item.id().unwrap(), "_section_title");
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

            assert_eq!(
                mi.item,
                SectionBlock {
                    level: 2,
                    section_title: Content {
                        original: Span {
                            data: "Section Title",
                            line: 1,
                            col: 5,
                            offset: 4,
                        },
                        rendered: "Section Title",
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
                        data: "=== Section Title\n\nabc",
                        line: 1,
                        col: 1,
                        offset: 0,
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
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "== Section 2\n\ndef",
                    line: 5,
                    col: 1,
                    offset: 24
                }
            );
        }

        #[test]
        fn section_title_with_markup() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("== Section with *bold* text"),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(
                mi.item.section_title_source(),
                Span {
                    data: "Section with *bold* text",
                    line: 1,
                    col: 4,
                    offset: 3,
                }
            );

            assert_eq!(
                mi.item.section_title(),
                "Section with <strong>bold</strong> text"
            );

            assert_eq!(mi.item.section_type(), SectionType::Normal);
            assert_eq!(mi.item.id().unwrap(), "_section_with_bold_text");
        }

        #[test]
        fn section_title_with_special_chars() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("== Section with <brackets> & ampersands"),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(
                mi.item.section_title_source(),
                Span {
                    data: "Section with <brackets> & ampersands",
                    line: 1,
                    col: 4,
                    offset: 3,
                }
            );

            assert_eq!(
                mi.item.section_title(),
                "Section with &lt;brackets&gt; &amp; ampersands"
            );

            assert_eq!(mi.item.id().unwrap(), "_section_with_ampersands");
        }

        #[test]
        fn err_level_0_section_heading() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let result = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("= Document Title"),
                &mut parser,
                &mut warnings,
            );

            assert!(result.is_none());

            assert_eq!(
                warnings,
                vec![Warning {
                    source: Span {
                        data: "= Document Title",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    warning: WarningType::Level0SectionHeadingNotSupported,
                }]
            );
        }

        #[test]
        fn err_section_heading_level_exceeds_maximum() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let result = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("======= Level 6 Section"),
                &mut parser,
                &mut warnings,
            );

            assert!(result.is_none());

            assert_eq!(
                warnings,
                vec![Warning {
                    source: Span {
                        data: "======= Level 6 Section",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    warning: WarningType::SectionHeadingLevelExceedsMaximum(6),
                }]
            );
        }

        #[test]
        fn valid_maximum_level_5_section() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("====== Level 5 Section"),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert!(warnings.is_empty());

            assert_eq!(mi.item.level(), 5);
            assert_eq!(mi.item.section_title(), "Level 5 Section");
            assert_eq!(mi.item.section_type(), SectionType::Normal);
            assert_eq!(mi.item.id().unwrap(), "_level_5_section");
        }

        #[test]
        fn warn_section_level_skipped() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("== Level 1\n\n==== Level 3 (skipped level 2)"),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(mi.item.level(), 1);
            assert_eq!(mi.item.section_title(), "Level 1");
            assert_eq!(mi.item.section_type(), SectionType::Normal);
            assert_eq!(mi.item.nested_blocks().len(), 1);
            assert_eq!(mi.item.id().unwrap(), "_level_1");

            assert_eq!(
                warnings,
                vec![Warning {
                    source: Span {
                        data: "==== Level 3 (skipped level 2)",
                        line: 3,
                        col: 1,
                        offset: 12,
                    },
                    warning: WarningType::SectionHeadingLevelSkipped(1, 3),
                }]
            );
        }
    }

    mod markdown_style_headings {
        use std::ops::Deref;

        use crate::{
            blocks::{ContentModel, MediaType, metadata::BlockMetadata, section::SectionType},
            tests::prelude::*,
        };

        #[test]
        fn err_missing_space_before_title() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            assert!(
                crate::blocks::SectionBlock::parse(
                    &BlockMetadata::new("#blah blah"),
                    &mut parser,
                    &mut warnings
                )
                .is_none()
            );
        }

        #[test]
        fn simplest_section_block() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("## Section Title"),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(mi.item.content_model(), ContentModel::Compound);
            assert_eq!(mi.item.raw_context().deref(), "section");
            assert_eq!(mi.item.resolved_context().deref(), "section");
            assert!(mi.item.declared_style().is_none());
            assert_eq!(mi.item.id().unwrap(), "_section_title");
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

            assert_eq!(
                mi.item,
                SectionBlock {
                    level: 1,
                    section_title: Content {
                        original: Span {
                            data: "Section Title",
                            line: 1,
                            col: 4,
                            offset: 3,
                        },
                        rendered: "Section Title",
                    },
                    blocks: &[],
                    source: Span {
                        data: "## Section Title",
                        line: 1,
                        col: 1,
                        offset: 0,
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
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 17,
                    offset: 16
                }
            );
        }

        #[test]
        fn has_child_block() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("## Section Title\n\nabc"),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(mi.item.content_model(), ContentModel::Compound);
            assert_eq!(mi.item.raw_context().deref(), "section");
            assert_eq!(mi.item.resolved_context().deref(), "section");
            assert!(mi.item.declared_style().is_none());
            assert_eq!(mi.item.id().unwrap(), "_section_title");
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

            assert_eq!(
                mi.item,
                SectionBlock {
                    level: 1,
                    section_title: Content {
                        original: Span {
                            data: "Section Title",
                            line: 1,
                            col: 4,
                            offset: 3,
                        },
                        rendered: "Section Title",
                    },
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "abc",
                                line: 3,
                                col: 1,
                                offset: 18,
                            },
                            rendered: "abc",
                        },
                        source: Span {
                            data: "abc",
                            line: 3,
                            col: 1,
                            offset: 18,
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
                        data: "## Section Title\n\nabc",
                        line: 1,
                        col: 1,
                        offset: 0,
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
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 3,
                    col: 4,
                    offset: 21
                }
            );
        }

        #[test]
        fn has_macro_block_with_extra_blank_line() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new(
                    "## Section Title\n\nimage::bar[alt=Sunset,width=300,height=400]\n\n",
                ),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(mi.item.content_model(), ContentModel::Compound);
            assert_eq!(mi.item.raw_context().deref(), "section");
            assert_eq!(mi.item.resolved_context().deref(), "section");
            assert!(mi.item.declared_style().is_none());
            assert_eq!(mi.item.id().unwrap(), "_section_title");
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

            assert_eq!(
                mi.item,
                SectionBlock {
                    level: 1,
                    section_title: Content {
                        original: Span {
                            data: "Section Title",
                            line: 1,
                            col: 4,
                            offset: 3,
                        },
                        rendered: "Section Title",
                    },
                    blocks: &[Block::Media(MediaBlock {
                        type_: MediaType::Image,
                        target: Span {
                            data: "bar",
                            line: 3,
                            col: 8,
                            offset: 25,
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
                                }
                            ],
                            anchor: None,
                            source: Span {
                                data: "alt=Sunset,width=300,height=400",
                                line: 3,
                                col: 12,
                                offset: 29,
                            }
                        },
                        source: Span {
                            data: "image::bar[alt=Sunset,width=300,height=400]",
                            line: 3,
                            col: 1,
                            offset: 18,
                        },
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    })],
                    source: Span {
                        data: "## Section Title\n\nimage::bar[alt=Sunset,width=300,height=400]",
                        line: 1,
                        col: 1,
                        offset: 0,
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
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 5,
                    col: 1,
                    offset: 63
                }
            );
        }

        #[test]
        fn has_child_block_with_errors() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new(
                    "## Section Title\n\nimage::bar[alt=Sunset,width=300,,height=400]",
                ),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(mi.item.content_model(), ContentModel::Compound);
            assert_eq!(mi.item.raw_context().deref(), "section");
            assert_eq!(mi.item.resolved_context().deref(), "section");
            assert!(mi.item.declared_style().is_none());
            assert_eq!(mi.item.id().unwrap(), "_section_title");
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

            assert_eq!(
                mi.item,
                SectionBlock {
                    level: 1,
                    section_title: Content {
                        original: Span {
                            data: "Section Title",
                            line: 1,
                            col: 4,
                            offset: 3,
                        },
                        rendered: "Section Title",
                    },
                    blocks: &[Block::Media(MediaBlock {
                        type_: MediaType::Image,
                        target: Span {
                            data: "bar",
                            line: 3,
                            col: 8,
                            offset: 25,
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
                                }
                            ],
                            anchor: None,
                            source: Span {
                                data: "alt=Sunset,width=300,,height=400",
                                line: 3,
                                col: 12,
                                offset: 29,
                            }
                        },
                        source: Span {
                            data: "image::bar[alt=Sunset,width=300,,height=400]",
                            line: 3,
                            col: 1,
                            offset: 18,
                        },
                        title_source: None,
                        title: None,
                        caption: None,
                        number: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    })],
                    source: Span {
                        data: "## Section Title\n\nimage::bar[alt=Sunset,width=300,,height=400]",
                        line: 1,
                        col: 1,
                        offset: 0,
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
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 3,
                    col: 45,
                    offset: 62
                }
            );

            assert_eq!(
                warnings,
                vec![Warning {
                    source: Span {
                        data: "alt=Sunset,width=300,,height=400",
                        line: 3,
                        col: 12,
                        offset: 29,
                    },
                    warning: WarningType::EmptyAttributeValue,
                }]
            );
        }

        #[test]
        fn dont_stop_at_child_section() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("## Section Title\n\nabc\n\n### Section 2\n\ndef"),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(mi.item.content_model(), ContentModel::Compound);
            assert_eq!(mi.item.raw_context().deref(), "section");
            assert_eq!(mi.item.resolved_context().deref(), "section");
            assert!(mi.item.declared_style().is_none());
            assert_eq!(mi.item.id().unwrap(), "_section_title");
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

            assert_eq!(
                mi.item,
                SectionBlock {
                    level: 1,
                    section_title: Content {
                        original: Span {
                            data: "Section Title",
                            line: 1,
                            col: 4,
                            offset: 3,
                        },
                        rendered: "Section Title",
                    },
                    blocks: &[
                        Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "abc",
                                    line: 3,
                                    col: 1,
                                    offset: 18,
                                },
                                rendered: "abc",
                            },
                            source: Span {
                                data: "abc",
                                line: 3,
                                col: 1,
                                offset: 18,
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
                        Block::Section(SectionBlock {
                            level: 2,
                            section_title: Content {
                                original: Span {
                                    data: "Section 2",
                                    line: 5,
                                    col: 5,
                                    offset: 27,
                                },
                                rendered: "Section 2",
                            },
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "def",
                                        line: 7,
                                        col: 1,
                                        offset: 38,
                                    },
                                    rendered: "def",
                                },
                                source: Span {
                                    data: "def",
                                    line: 7,
                                    col: 1,
                                    offset: 38,
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
                                data: "### Section 2\n\ndef",
                                line: 5,
                                col: 1,
                                offset: 23,
                            },
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                            section_type: SectionType::Normal,
                            section_id: Some("_section_2"),
                            caption: None,
                            section_number: None,
                        })
                    ],
                    source: Span {
                        data: "## Section Title\n\nabc\n\n### Section 2\n\ndef",
                        line: 1,
                        col: 1,
                        offset: 0,
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
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 7,
                    col: 4,
                    offset: 41
                }
            );
        }

        #[test]
        fn stop_at_peer_section() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("## Section Title\n\nabc\n\n## Section 2\n\ndef"),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(mi.item.content_model(), ContentModel::Compound);
            assert_eq!(mi.item.raw_context().deref(), "section");
            assert_eq!(mi.item.resolved_context().deref(), "section");
            assert!(mi.item.declared_style().is_none());
            assert_eq!(mi.item.id().unwrap(), "_section_title");
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

            assert_eq!(
                mi.item,
                SectionBlock {
                    level: 1,
                    section_title: Content {
                        original: Span {
                            data: "Section Title",
                            line: 1,
                            col: 4,
                            offset: 3,
                        },
                        rendered: "Section Title",
                    },
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "abc",
                                line: 3,
                                col: 1,
                                offset: 18,
                            },
                            rendered: "abc",
                        },
                        source: Span {
                            data: "abc",
                            line: 3,
                            col: 1,
                            offset: 18,
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
                        data: "## Section Title\n\nabc",
                        line: 1,
                        col: 1,
                        offset: 0,
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
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "## Section 2\n\ndef",
                    line: 5,
                    col: 1,
                    offset: 23
                }
            );
        }

        #[test]
        fn stop_at_ancestor_section() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("### Section Title\n\nabc\n\n## Section 2\n\ndef"),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(mi.item.content_model(), ContentModel::Compound);
            assert_eq!(mi.item.raw_context().deref(), "section");
            assert_eq!(mi.item.resolved_context().deref(), "section");
            assert!(mi.item.declared_style().is_none());
            assert_eq!(mi.item.id().unwrap(), "_section_title");
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

            assert_eq!(
                mi.item,
                SectionBlock {
                    level: 2,
                    section_title: Content {
                        original: Span {
                            data: "Section Title",
                            line: 1,
                            col: 5,
                            offset: 4,
                        },
                        rendered: "Section Title",
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
                        data: "### Section Title\n\nabc",
                        line: 1,
                        col: 1,
                        offset: 0,
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
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "## Section 2\n\ndef",
                    line: 5,
                    col: 1,
                    offset: 24
                }
            );
        }

        #[test]
        fn section_title_with_markup() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("## Section with *bold* text"),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(
                mi.item.section_title_source(),
                Span {
                    data: "Section with *bold* text",
                    line: 1,
                    col: 4,
                    offset: 3,
                }
            );

            assert_eq!(
                mi.item.section_title(),
                "Section with <strong>bold</strong> text"
            );

            assert_eq!(mi.item.section_type(), SectionType::Normal);
            assert_eq!(mi.item.id().unwrap(), "_section_with_bold_text");
        }

        #[test]
        fn section_title_with_special_chars() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("## Section with <brackets> & ampersands"),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(
                mi.item.section_title_source(),
                Span {
                    data: "Section with <brackets> & ampersands",
                    line: 1,
                    col: 4,
                    offset: 3,
                }
            );

            assert_eq!(
                mi.item.section_title(),
                "Section with &lt;brackets&gt; &amp; ampersands"
            );

            assert_eq!(mi.item.section_type(), SectionType::Normal);
        }

        #[test]
        fn err_level_0_section_heading() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let result = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("# Document Title"),
                &mut parser,
                &mut warnings,
            );

            assert!(result.is_none());

            assert_eq!(
                warnings,
                vec![Warning {
                    source: Span {
                        data: "# Document Title",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    warning: WarningType::Level0SectionHeadingNotSupported,
                }]
            );
        }

        #[test]
        fn err_section_heading_level_exceeds_maximum() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let result = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("####### Level 6 Section"),
                &mut parser,
                &mut warnings,
            );

            assert!(result.is_none());

            assert_eq!(
                warnings,
                vec![Warning {
                    source: Span {
                        data: "####### Level 6 Section",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    warning: WarningType::SectionHeadingLevelExceedsMaximum(6),
                }]
            );
        }

        #[test]
        fn valid_maximum_level_5_section() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("###### Level 5 Section"),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert!(warnings.is_empty());

            assert_eq!(mi.item.level(), 5);
            assert_eq!(mi.item.section_title(), "Level 5 Section");
            assert_eq!(mi.item.section_type(), SectionType::Normal);
            assert_eq!(mi.item.id().unwrap(), "_level_5_section");
        }

        #[test]
        fn warn_section_level_skipped() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("## Level 1\n\n#### Level 3 (skipped level 2)"),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(mi.item.level(), 1);
            assert_eq!(mi.item.section_title(), "Level 1");
            assert_eq!(mi.item.section_type(), SectionType::Normal);
            assert_eq!(mi.item.nested_blocks().len(), 1);
            assert_eq!(mi.item.id().unwrap(), "_level_1");

            assert_eq!(
                warnings,
                vec![Warning {
                    source: Span {
                        data: "#### Level 3 (skipped level 2)",
                        line: 3,
                        col: 1,
                        offset: 12,
                    },
                    warning: WarningType::SectionHeadingLevelSkipped(1, 3),
                }]
            );
        }
    }

    #[test]
    fn warn_multiple_section_levels_skipped() {
        let mut parser = Parser::default();
        let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

        let mi = crate::blocks::SectionBlock::parse(
            &BlockMetadata::new("== Level 1\n\n===== Level 4 (skipped levels 2 and 3)"),
            &mut parser,
            &mut warnings,
        )
        .unwrap();

        assert_eq!(mi.item.level(), 1);
        assert_eq!(mi.item.section_title(), "Level 1");
        assert_eq!(mi.item.section_type(), SectionType::Normal);
        assert_eq!(mi.item.nested_blocks().len(), 1);
        assert_eq!(mi.item.id().unwrap(), "_level_1");

        assert_eq!(
            warnings,
            vec![Warning {
                source: Span {
                    data: "===== Level 4 (skipped levels 2 and 3)",
                    line: 3,
                    col: 1,
                    offset: 12,
                },
                warning: WarningType::SectionHeadingLevelSkipped(1, 4),
            }]
        );
    }

    #[test]
    fn no_warning_for_consecutive_section_levels() {
        let mut parser = Parser::default();
        let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

        let mi = crate::blocks::SectionBlock::parse(
            &BlockMetadata::new("== Level 1\n\n=== Level 2 (no skip)"),
            &mut parser,
            &mut warnings,
        )
        .unwrap();

        assert_eq!(mi.item.level(), 1);
        assert_eq!(mi.item.section_title(), "Level 1");
        assert_eq!(mi.item.section_type(), SectionType::Normal);
        assert_eq!(mi.item.nested_blocks().len(), 1);
        assert_eq!(mi.item.id().unwrap(), "_level_1");

        assert!(warnings.is_empty());
    }

    #[test]
    fn section_id_generation_basic() {
        let input = "== Section One";
        let mut parser = Parser::default();
        let document = parser.parse(input);

        if let Some(crate::blocks::Block::Section(section)) = document.nested_blocks().next() {
            assert_eq!(section.id(), Some("_section_one"));
        } else {
            panic!("Expected section block");
        }
    }

    #[test]
    fn section_id_generation_with_special_characters() {
        let input = "== We're back! & Company";
        let mut parser = Parser::default();
        let document = parser.parse(input);

        if let Some(crate::blocks::Block::Section(section)) = document.nested_blocks().next() {
            assert_eq!(section.id(), Some("_were_back_company"));
        } else {
            panic!("Expected section block");
        }
    }

    #[test]
    fn section_id_generation_with_entities() {
        let input = "== Ben &amp; Jerry &#34;Ice Cream&#34;";
        let mut parser = Parser::default();
        let document = parser.parse(input);

        if let Some(crate::blocks::Block::Section(section)) = document.nested_blocks().next() {
            assert_eq!(section.id(), Some("_ben_jerry_ice_cream"));
        } else {
            panic!("Expected section block");
        }
    }

    #[test]
    fn section_id_generation_disabled_when_sectids_unset() {
        let input = ":!sectids:\n\n== Section One";
        let mut parser = Parser::default();
        let document = parser.parse(input);

        if let Some(crate::blocks::Block::Section(section)) = document.nested_blocks().next() {
            assert_eq!(section.id(), None);
        } else {
            panic!("Expected section block");
        }
    }

    #[test]
    fn section_id_generation_with_custom_prefix() {
        let input = ":idprefix: id_\n\n== Section One";
        let mut parser = Parser::default();
        let document = parser.parse(input);

        if let Some(crate::blocks::Block::Section(section)) = document.nested_blocks().next() {
            assert_eq!(section.id(), Some("id_section_one"));
        } else {
            panic!("Expected section block");
        }
    }

    #[test]
    fn section_id_generation_with_custom_separator() {
        let input = ":idseparator: -\n\n== Section One";
        let mut parser = Parser::default();
        let document = parser.parse(input);

        if let Some(crate::blocks::Block::Section(section)) = document.nested_blocks().next() {
            assert_eq!(section.id(), Some("_section-one"));
        } else {
            panic!("Expected section block");
        }
    }

    #[test]
    fn section_id_generation_with_empty_prefix() {
        let input = ":idprefix:\n\n== Section One";
        let mut parser = Parser::default();
        let document = parser.parse(input);

        if let Some(crate::blocks::Block::Section(section)) = document.nested_blocks().next() {
            assert_eq!(section.id(), Some("section_one"));
        } else {
            panic!("Expected section block");
        }
    }

    #[test]
    fn section_id_generation_removes_trailing_separator() {
        let input = ":idseparator: -\n\n== Section Title-";
        let mut parser = Parser::default();
        let document = parser.parse(input);

        if let Some(crate::blocks::Block::Section(section)) = document.nested_blocks().next() {
            assert_eq!(section.id(), Some("_section-title"));
        } else {
            panic!("Expected section block");
        }
    }

    #[test]
    fn section_id_generation_removes_leading_separator_when_prefix_empty() {
        let input = ":idprefix:\n:idseparator: -\n\n== -Section Title";
        let mut parser = Parser::default();
        let document = parser.parse(input);

        if let Some(crate::blocks::Block::Section(section)) = document.nested_blocks().next() {
            assert_eq!(section.id(), Some("section-title"));
        } else {
            panic!("Expected section block");
        }
    }

    #[test]
    fn section_id_generation_handles_multiple_trailing_separators() {
        let input = ":idseparator: _\n\n== Title with Multiple Dots...";
        let mut parser = Parser::default();
        let document = parser.parse(input);

        if let Some(crate::blocks::Block::Section(section)) = document.nested_blocks().next() {
            assert_eq!(section.id(), Some("_title_with_multiple_dots"));
        } else {
            panic!("Expected section block");
        }
    }

    #[test]
    fn warn_duplicate_manual_section_id() {
        let input = "[#my_id]\n== First Section\n\n[#my_id]\n== Second Section";
        let mut parser = Parser::default();
        let document = parser.parse(input);

        let mut warnings = document.warnings();

        assert_eq!(
            warnings.next().unwrap(),
            Warning {
                source: Span {
                    data: "[#my_id]\n== Second Section",
                    line: 4,
                    col: 1,
                    offset: 27,
                },
                warning: WarningType::DuplicateId("my_id".to_owned()),
            }
        );

        assert!(warnings.next().is_none());
    }

    #[test]
    fn section_with_custom_reftext_attribute() {
        let input = "[reftext=\"Custom Reference Text\"]\n== Section Title";
        let mut parser = Parser::default();
        let document = parser.parse(input);

        if let Some(crate::blocks::Block::Section(section)) = document.nested_blocks().next() {
            assert_eq!(section.id(), Some("_section_title"));
        } else {
            panic!("Expected section block");
        }

        let catalog = document.catalog();
        let entry = catalog.get_ref("_section_title");
        assert!(entry.is_some());
        assert_eq!(
            entry.unwrap().reftext,
            Some("Custom Reference Text".to_string())
        );
    }

    #[test]
    fn section_without_reftext_uses_title() {
        let input = "== Section Title";
        let mut parser = Parser::default();
        let document = parser.parse(input);

        if let Some(crate::blocks::Block::Section(section)) = document.nested_blocks().next() {
            assert_eq!(section.id(), Some("_section_title"));
        } else {
            panic!("Expected section block");
        }

        let catalog = document.catalog();
        let entry = catalog.get_ref("_section_title");
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().reftext, Some("Section Title".to_string()));
    }

    mod section_numbering {
        use crate::{blocks::Block, tests::prelude::*};

        #[test]
        fn single_section_with_sectnums() {
            let input = ":sectnums:\n\n== First Section";
            let mut parser = Parser::default();
            let document = parser.parse(input);

            if let Some(Block::Section(section)) = document.nested_blocks().next() {
                let section_number = section.section_number();
                assert!(section_number.is_some());
                assert_eq!(section_number.unwrap().to_string(), "1");
                assert_eq!(section_number.unwrap().components(), [1]);
            } else {
                panic!("Expected section block");
            }
        }

        #[test]
        fn multiple_level_1_sections() {
            let input = ":sectnums:\n\n== First Section\n\n== Second Section\n\n== Third Section";
            let mut parser = Parser::default();
            let document = parser.parse(input);

            let mut sections = document.nested_blocks().filter_map(|block| {
                if let Block::Section(section) = block {
                    Some(section)
                } else {
                    None
                }
            });

            let first = sections.next().unwrap();
            assert_eq!(first.section_number().unwrap().to_string(), "1");

            let second = sections.next().unwrap();
            assert_eq!(second.section_number().unwrap().to_string(), "2");

            let third = sections.next().unwrap();
            assert_eq!(third.section_number().unwrap().to_string(), "3");
        }

        #[test]
        fn nested_sections() {
            let input = ":sectnums:\n\n== Level 1\n\n=== Level 2\n\n==== Level 3";
            let document = Parser::default().parse(input);

            if let Some(Block::Section(level1)) = document.nested_blocks().next() {
                assert_eq!(level1.section_number().unwrap().to_string(), "1");

                if let Some(Block::Section(level2)) = level1.nested_blocks().next() {
                    assert_eq!(level2.section_number().unwrap().to_string(), "1.1");

                    if let Some(Block::Section(level3)) = level2.nested_blocks().next() {
                        assert_eq!(level3.section_number().unwrap().to_string(), "1.1.1");
                    } else {
                        panic!("Expected level 3 section");
                    }
                } else {
                    panic!("Expected level 2 section");
                }
            } else {
                panic!("Expected level 1 section");
            }
        }

        #[test]
        fn mixed_section_levels() {
            let input = ":sectnums:\n\n== First\n\n=== First.One\n\n=== First.Two\n\n== Second\n\n=== Second.One";
            let document = Parser::default().parse(input);

            let mut sections = document.nested_blocks().filter_map(|block| {
                if let Block::Section(section) = block {
                    Some(section)
                } else {
                    None
                }
            });

            let first = sections.next().unwrap();
            assert_eq!(first.section_number().unwrap().to_string(), "1");

            let first_one = first
                .nested_blocks()
                .filter_map(|block| {
                    if let Block::Section(section) = block {
                        Some(section)
                    } else {
                        None
                    }
                })
                .next()
                .unwrap();
            assert_eq!(first_one.section_number().unwrap().to_string(), "1.1");

            let first_two = first
                .nested_blocks()
                .filter_map(|block| {
                    if let Block::Section(section) = block {
                        Some(section)
                    } else {
                        None
                    }
                })
                .nth(1)
                .unwrap();
            assert_eq!(first_two.section_number().unwrap().to_string(), "1.2");

            let second = sections.next().unwrap();
            assert_eq!(second.section_number().unwrap().to_string(), "2");

            let second_one = second
                .nested_blocks()
                .filter_map(|block| {
                    if let Block::Section(section) = block {
                        Some(section)
                    } else {
                        None
                    }
                })
                .next()
                .unwrap();
            assert_eq!(second_one.section_number().unwrap().to_string(), "2.1");
        }

        #[test]
        fn sectnums_disabled() {
            let input = "== First Section\n\n== Second Section";
            let mut parser = Parser::default();
            let document = parser.parse(input);

            for block in document.nested_blocks() {
                if let Block::Section(section) = block {
                    assert!(section.section_number().is_none());
                }
            }
        }

        #[test]
        fn sectnums_explicitly_unset() {
            let input = ":!sectnums:\n\n== First Section\n\n== Second Section";
            let mut parser = Parser::default();
            let document = parser.parse(input);

            for block in document.nested_blocks() {
                if let Block::Section(section) = block {
                    assert!(section.section_number().is_none());
                }
            }
        }

        #[test]
        fn numbered_alias_enables_numbering() {
            // `numbered` is a legacy alias for `sectnums`: setting it numbers
            // sections just as `sectnums` would, and `is_attribute_set` reports
            // the primary name, mirroring Asciidoctor.
            let input = ":numbered:\n\n== First Section\n\n== Second Section";
            let mut parser = Parser::default();
            let document = parser.parse(input);

            assert!(parser.is_attribute_set("sectnums"));

            let mut sections = document.nested_blocks().filter_map(|block| {
                if let Block::Section(section) = block {
                    Some(section)
                } else {
                    None
                }
            });

            assert_eq!(
                sections
                    .next()
                    .unwrap()
                    .section_number()
                    .unwrap()
                    .to_string(),
                "1"
            );
            assert_eq!(
                sections
                    .next()
                    .unwrap()
                    .section_number()
                    .unwrap()
                    .to_string(),
                "2"
            );
        }

        #[test]
        fn numbered_alias_can_be_toggled_off_within_document() {
            // `numbered!` unsets the alias mid-document; sections after the
            // toggle are not numbered.
            let input =
                ":numbered:\n\n== Numbered\n\n:numbered!:\n\n== Unnumbered\n\n== Also Unnumbered";
            let mut parser = Parser::default();
            let document = parser.parse(input);

            let mut sections = document.nested_blocks().filter_map(|block| {
                if let Block::Section(section) = block {
                    Some(section)
                } else {
                    None
                }
            });

            assert_eq!(
                sections
                    .next()
                    .unwrap()
                    .section_number()
                    .unwrap()
                    .to_string(),
                "1"
            );
            assert!(sections.next().unwrap().section_number().is_none());
            assert!(sections.next().unwrap().section_number().is_none());
        }

        #[test]
        fn deep_nesting() {
            let input = ":sectnums:\n:sectnumlevels: 5\n\n== Level 1\n\n=== Level 2\n\n==== Level 3\n\n===== Level 4\n\n====== Level 5";
            let document = Parser::default().parse(input);

            if let Some(Block::Section(l1)) = document.nested_blocks().next() {
                assert_eq!(l1.section_number().unwrap().to_string(), "1");

                if let Some(Block::Section(l2)) = l1.nested_blocks().next() {
                    assert_eq!(l2.section_number().unwrap().to_string(), "1.1");

                    if let Some(Block::Section(l3)) = l2.nested_blocks().next() {
                        assert_eq!(l3.section_number().unwrap().to_string(), "1.1.1");

                        if let Some(Block::Section(l4)) = l3.nested_blocks().next() {
                            assert_eq!(l4.section_number().unwrap().to_string(), "1.1.1.1");

                            if let Some(Block::Section(l5)) = l4.nested_blocks().next() {
                                assert_eq!(l5.section_number().unwrap().to_string(), "1.1.1.1.1");
                            } else {
                                panic!("Expected level 5 section");
                            }
                        } else {
                            panic!("Expected level 4 section");
                        }
                    } else {
                        panic!("Expected level 3 section");
                    }
                } else {
                    panic!("Expected level 2 section");
                }
            } else {
                panic!("Expected level 1 section");
            }
        }
    }

    #[test]
    fn impl_debug() {
        let mut parser = Parser::default();
        let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

        let section = crate::blocks::SectionBlock::parse(
            &BlockMetadata::new("== Section Title"),
            &mut parser,
            &mut warnings,
        )
        .unwrap()
        .item;

        assert_eq!(
            format!("{section:#?}"),
            r#"SectionBlock {
    level: 1,
    section_title: Content {
        original: Span {
            data: "Section Title",
            line: 1,
            col: 4,
            offset: 3,
        },
        rendered: "Section Title",
    },
    blocks: &[],
    source: Span {
        data: "== Section Title",
        line: 1,
        col: 1,
        offset: 0,
    },
    title_source: None,
    title: None,
    anchor: None,
    anchor_reftext: None,
    attrlist: None,
    section_type: SectionType::Normal,
    section_id: Some(
        "_section_title",
    ),
    caption: None,
    section_number: None,
}"#
        );
    }

    mod section_type {
        use crate::blocks::section::SectionType;

        #[test]
        fn impl_debug() {
            let st = SectionType::Normal;
            assert_eq!(format!("{st:?}"), "SectionType::Normal");

            let st = SectionType::Appendix;
            assert_eq!(format!("{st:?}"), "SectionType::Appendix");

            let st = SectionType::Discrete;
            assert_eq!(format!("{st:?}"), "SectionType::Discrete");
        }
    }

    mod section_number {
        mod assign_next_number {
            use crate::blocks::section::SectionNumber;

            #[test]
            fn default() {
                let sn = SectionNumber::default();
                assert_eq!(sn.components(), []);
                assert_eq!(sn.to_string(), "");
                assert_eq!(
                    format!("{sn:?}"),
                    "SectionNumber { section_type: SectionType::Normal, components: &[], appendix_letter: None }"
                );
            }

            #[test]
            fn level_1() {
                let mut sn = SectionNumber::default();
                sn.assign_next_number(1);
                assert_eq!(sn.components(), [1]);
                assert_eq!(sn.to_string(), "1");
                assert_eq!(
                    format!("{sn:?}"),
                    "SectionNumber { section_type: SectionType::Normal, components: &[1], appendix_letter: None }"
                );
            }

            #[test]
            fn level_3() {
                let mut sn = SectionNumber::default();
                sn.assign_next_number(3);
                assert_eq!(sn.components(), [1, 1, 1]);
                assert_eq!(sn.to_string(), "1.1.1");
                assert_eq!(
                    format!("{sn:?}"),
                    "SectionNumber { section_type: SectionType::Normal, components: &[1, 1, 1], appendix_letter: None }"
                );
            }

            #[test]
            fn level_3_then_1() {
                let mut sn = SectionNumber::default();
                sn.assign_next_number(3);
                sn.assign_next_number(1);
                assert_eq!(sn.components(), [2]);
                assert_eq!(sn.to_string(), "2");
                assert_eq!(
                    format!("{sn:?}"),
                    "SectionNumber { section_type: SectionType::Normal, components: &[2], appendix_letter: None }"
                );
            }

            #[test]
            fn level_3_then_1_then_2() {
                let mut sn = SectionNumber::default();
                sn.assign_next_number(3);
                sn.assign_next_number(1);
                sn.assign_next_number(2);
                assert_eq!(sn.components(), [2, 1]);
                assert_eq!(sn.to_string(), "2.1");
                assert_eq!(
                    format!("{sn:?}"),
                    "SectionNumber { section_type: SectionType::Normal, components: &[2, 1], appendix_letter: None }"
                );
            }
        }

        mod assign_next_number_appendix {
            use crate::blocks::{SectionType, section::SectionNumber};

            #[test]
            fn default() {
                let sn = SectionNumber {
                    section_type: SectionType::Appendix,
                    components: vec![],
                    appendix_letter: None,
                };
                assert_eq!(sn.components(), []);
                assert_eq!(sn.to_string(), "");
                assert_eq!(
                    format!("{sn:?}"),
                    "SectionNumber { section_type: SectionType::Appendix, components: &[], appendix_letter: None }"
                );
            }

            #[test]
            fn level_1() {
                let mut sn = SectionNumber {
                    section_type: SectionType::Appendix,
                    components: vec![],
                    appendix_letter: None,
                };
                sn.assign_next_number(1);
                assert_eq!(sn.components(), [1]);
                assert_eq!(sn.to_string(), "A");
                assert_eq!(
                    format!("{sn:?}"),
                    "SectionNumber { section_type: SectionType::Appendix, components: &[1], appendix_letter: None }"
                );
            }

            #[test]
            fn level_3() {
                let mut sn = SectionNumber {
                    section_type: SectionType::Appendix,
                    components: vec![],
                    appendix_letter: None,
                };
                sn.assign_next_number(3);
                assert_eq!(sn.components(), [1, 1, 1]);
                assert_eq!(sn.to_string(), "A.1.1");
                assert_eq!(
                    format!("{sn:?}"),
                    "SectionNumber { section_type: SectionType::Appendix, components: &[1, 1, 1], appendix_letter: None }"
                );
            }

            #[test]
            fn level_3_then_1() {
                let mut sn = SectionNumber {
                    section_type: SectionType::Appendix,
                    components: vec![],
                    appendix_letter: None,
                };
                sn.assign_next_number(3);
                sn.assign_next_number(1);
                assert_eq!(sn.components(), [2]);
                assert_eq!(sn.to_string(), "B");
                assert_eq!(
                    format!("{sn:?}"),
                    "SectionNumber { section_type: SectionType::Appendix, components: &[2], appendix_letter: None }"
                );
            }

            #[test]
            fn level_3_then_1_then_2() {
                let mut sn = SectionNumber {
                    section_type: SectionType::Appendix,
                    components: vec![],
                    appendix_letter: None,
                };
                sn.assign_next_number(3);
                sn.assign_next_number(1);
                sn.assign_next_number(2);
                assert_eq!(sn.components(), [2, 1]);
                assert_eq!(sn.to_string(), "B.1");
                assert_eq!(
                    format!("{sn:?}"),
                    "SectionNumber { section_type: SectionType::Appendix, components: &[2, 1], appendix_letter: None }"
                );
            }

            #[test]
            fn appendix_letter_overrides_first_component() {
                let mut sn = SectionNumber {
                    section_type: SectionType::Appendix,
                    components: vec![],
                    appendix_letter: Some("\u{3b2}".to_owned()),
                };
                sn.assign_next_number(1);
                sn.assign_next_number(2);
                assert_eq!(sn.components(), [1, 1]);
                assert_eq!(sn.appendix_letter(), Some("\u{3b2}"));
                assert_eq!(sn.to_string(), "\u{3b2}.1");
                assert_eq!(
                    format!("{sn:?}"),
                    "SectionNumber { section_type: SectionType::Appendix, components: &[1, 1], appendix_letter: Some(\"\u{3b2}\") }"
                );
            }
        }
    }

    mod appendix_number_attribute {
        use crate::{blocks::Block, tests::prelude::*};

        // The `appendix-number` attribute is resolved as a counter (mirroring
        // Ruby Asciidoctor), so its value is the letter *before* the first
        // appendix and each appendix advances it.

        #[test]
        fn seeds_lettering_from_the_attribute() {
            let doc = Parser::default()
                .parse(":appendix-number: M\n\n[appendix]\n== One\n\n[appendix]\n== Two\n");

            let caps: Vec<Option<&str>> = all_sections(&doc).iter().map(|s| s.caption()).collect();
            assert_eq!(caps, vec![Some("Appendix N: "), Some("Appendix O: ")]);
        }

        #[test]
        fn increments_a_numeric_value_numerically() {
            let doc = Parser::default()
                .parse(":appendix-number: 9\n\n[appendix]\n== One\n\n[appendix]\n== Two\n");

            let caps: Vec<Option<&str>> = all_sections(&doc).iter().map(|s| s.caption()).collect();
            assert_eq!(caps, vec![Some("Appendix 10: "), Some("Appendix 11: ")]);
        }

        #[test]
        fn bare_attribute_resolves_to_default_seed() {
            // A bare `:appendix-number:` takes the built-in default `@`, the
            // character before `A`, so lettering still starts at `A`.
            let doc = Parser::default()
                .parse(":appendix-number:\n\n[appendix]\n== One\n\n[appendix]\n== Two\n");

            let caps: Vec<Option<&str>> = all_sections(&doc).iter().map(|s| s.caption()).collect();
            assert_eq!(caps, vec![Some("Appendix A: "), Some("Appendix B: ")]);
        }

        #[test]
        fn letters_section_numbers_of_appendix_and_subsections() {
            let doc = Parser::default()
                .parse(":sectnums:\n:appendix-number: \u{3b1}\n\n[appendix]\n== One\n\n=== Sub\n");

            let nums: Vec<Option<String>> = all_sections(&doc)
                .iter()
                .map(|s| s.section_number().map(|n| n.to_string()))
                .collect();
            assert_eq!(
                nums,
                vec![Some("\u{3b2}".to_owned()), Some("\u{3b2}.1".to_owned())]
            );
        }

        #[test]
        fn attribute_reads_back_as_the_current_letter() {
            // Advancing the counter stores the new value back into the
            // attribute, so a reference inside the appendix sees its letter.
            let doc = Parser::default().parse("[appendix]\n== One\n\nLetter {appendix-number}.\n");

            let section = first_section(&doc);
            let Some(Block::Simple(paragraph)) = section.nested_blocks().next() else {
                panic!("expected a simple block");
            };
            assert_eq!(paragraph.content().rendered(), "Letter A.");
        }
    }

    mod discrete_headings {
        use std::ops::Deref;

        use crate::{
            blocks::{ContentModel, metadata::BlockMetadata, section::SectionType},
            tests::prelude::*,
        };

        #[test]
        fn basic_case() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("[discrete]\n== Discrete Heading"),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(mi.item.content_model(), ContentModel::Compound);
            assert_eq!(mi.item.raw_context().deref(), "section");
            assert_eq!(mi.item.level(), 1);
            assert_eq!(mi.item.section_title(), "Discrete Heading");
            assert_eq!(mi.item.section_type(), SectionType::Discrete);
            assert!(mi.item.nested_blocks().next().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.attrlist().is_some());
            assert_eq!(mi.item.section_number(), None);
            assert!(warnings.is_empty());

            assert_eq!(
                mi.item.section_title_source(),
                Span {
                    data: "Discrete Heading",
                    line: 2,
                    col: 4,
                    offset: 14,
                }
            );

            assert_eq!(
                mi.item.span(),
                Span {
                    data: "[discrete]\n== Discrete Heading",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 2,
                    col: 20,
                    offset: 30,
                }
            );
        }

        #[test]
        fn float_style() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("[float]\n== Floating Heading"),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(mi.item.level(), 1);
            assert_eq!(mi.item.section_title(), "Floating Heading");
            assert_eq!(mi.item.section_type(), SectionType::Discrete);
            assert!(mi.item.nested_blocks().next().is_none());
            assert!(warnings.is_empty());
        }

        #[test]
        fn has_no_child_blocks() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("[discrete]\n== Discrete Heading\n\nThis is a paragraph."),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(mi.item.level(), 1);
            assert_eq!(mi.item.section_title(), "Discrete Heading");
            assert_eq!(mi.item.section_type(), SectionType::Discrete);

            // Discrete headings should have no nested blocks.
            assert!(mi.item.nested_blocks().next().is_none());

            // The paragraph should be left unparsed.
            assert_eq!(
                mi.after,
                Span {
                    data: "This is a paragraph.",
                    line: 4,
                    col: 1,
                    offset: 32,
                }
            );

            assert!(warnings.is_empty());
        }

        #[test]
        fn not_in_section_hierarchy() {
            let input = "== Section 1\n\n[discrete]\n=== Discrete\n\n=== Section 1.1";
            let mut parser = Parser::default();
            let document = parser.parse(input);

            let mut blocks = document.nested_blocks();

            // First should be "Section 1".
            if let Some(crate::blocks::Block::Section(section)) = blocks.next() {
                assert_eq!(section.section_title(), "Section 1");
                assert_eq!(section.level(), 1);
                assert_eq!(section.section_type(), SectionType::Normal);

                let mut children = section.nested_blocks();

                // First child should be the discrete heading.
                if let Some(crate::blocks::Block::Section(discrete)) = children.next() {
                    assert_eq!(discrete.section_title(), "Discrete");
                    assert_eq!(discrete.level(), 2);
                    assert_eq!(discrete.section_type(), SectionType::Discrete);
                    assert!(discrete.nested_blocks().next().is_none());
                } else {
                    panic!("Expected discrete heading block");
                }

                // Second child should be "Section 1.1".
                if let Some(crate::blocks::Block::Section(subsection)) = children.next() {
                    assert_eq!(subsection.section_title(), "Section 1.1");
                    assert_eq!(subsection.level(), 2);
                    assert_eq!(subsection.section_type(), SectionType::Normal);
                } else {
                    panic!("Expected subsection block");
                }
            } else {
                panic!("Expected section block");
            }
        }

        #[test]
        fn has_auto_id() {
            let input = "[discrete]\n== Discrete Heading";
            let mut parser = Parser::default();
            let document = parser.parse(input);

            if let Some(crate::blocks::Block::Section(section)) = document.nested_blocks().next() {
                // Discrete headings should generate auto IDs.
                assert_eq!(section.id(), Some("_discrete_heading"));
            } else {
                panic!("Expected section block");
            }
        }

        #[test]
        fn with_manual_id() {
            let input = "[discrete#my-id]\n== Discrete Heading";
            let mut parser = Parser::default();
            let document = parser.parse(input);

            if let Some(crate::blocks::Block::Section(section)) = document.nested_blocks().next() {
                // Manual IDs should still work with discrete headings.
                assert_eq!(section.id(), Some("my-id"));
            } else {
                panic!("Expected section block");
            }
        }

        #[test]
        fn no_section_number() {
            let input = ":sectnums:\n\n== Section 1\n\n[discrete]\n=== Discrete\n\n=== Section 1.1";
            let mut parser = Parser::default();
            let document = parser.parse(input);

            let mut blocks = document.nested_blocks();

            if let Some(crate::blocks::Block::Section(section)) = blocks.next() {
                assert_eq!(section.section_title(), "Section 1");
                assert!(section.section_number().is_some());

                let mut children = section.nested_blocks();

                // Discrete heading should not have a section number.
                if let Some(crate::blocks::Block::Section(discrete)) = children.next() {
                    assert_eq!(discrete.section_title(), "Discrete");
                    assert_eq!(discrete.section_number(), None);
                } else {
                    panic!("Expected discrete heading block");
                }

                // Regular subsection should have a section number.
                if let Some(crate::blocks::Block::Section(subsection)) = children.next() {
                    assert_eq!(subsection.section_title(), "Section 1.1");
                    assert!(subsection.section_number().is_some());
                } else {
                    panic!("Expected subsection block");
                }
            } else {
                panic!("Expected section block");
            }
        }

        #[test]
        fn title_can_have_markup() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("[discrete]\n== Discrete with *bold* text"),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(
                mi.item.section_title(),
                "Discrete with <strong>bold</strong> text"
            );
            assert_eq!(mi.item.section_type(), SectionType::Discrete);
            assert!(warnings.is_empty());
        }

        #[test]
        fn level_2() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("[discrete]\n=== Level 2 Discrete"),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(mi.item.level(), 2);
            assert_eq!(mi.item.section_title(), "Level 2 Discrete");
            assert_eq!(mi.item.section_type(), SectionType::Discrete);
            assert!(warnings.is_empty());
        }

        #[test]
        fn level_5() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("[discrete]\n====== Level 5 Discrete"),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(mi.item.level(), 5);
            assert_eq!(mi.item.section_title(), "Level 5 Discrete");
            assert_eq!(mi.item.section_type(), SectionType::Discrete);
            assert!(warnings.is_empty());
        }

        #[test]
        fn markdown_style() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("[discrete]\n## Discrete Heading"),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(mi.item.level(), 1);
            assert_eq!(mi.item.section_title(), "Discrete Heading");
            assert_eq!(mi.item.section_type(), SectionType::Discrete);
            assert!(warnings.is_empty());
        }

        #[test]
        fn with_block_title() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new(".Block Title\n[discrete]\n== Discrete Heading"),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(mi.item.level(), 1);
            assert_eq!(mi.item.section_title(), "Discrete Heading");
            assert_eq!(mi.item.section_type(), SectionType::Discrete);
            assert_eq!(mi.item.title(), Some("Block Title"));
            assert!(warnings.is_empty());
        }

        #[test]
        fn with_anchor() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new("[[my_anchor]]\n[discrete]\n== Discrete Heading"),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(mi.item.level(), 1);
            assert_eq!(mi.item.section_title(), "Discrete Heading");
            assert_eq!(mi.item.section_type(), SectionType::Discrete);
            assert_eq!(mi.item.id(), Some("my_anchor"));
            assert!(warnings.is_empty());
        }

        #[test]
        fn doesnt_include_subsequent_blocks() {
            let mut parser = Parser::default();
            let mut warnings: Vec<crate::warnings::Warning<'_>> = vec![];

            let mi = crate::blocks::SectionBlock::parse(
                &BlockMetadata::new(
                    "[discrete]\n== Discrete Heading\n\nparagraph\n\n== Next Section",
                ),
                &mut parser,
                &mut warnings,
            )
            .unwrap();

            assert_eq!(mi.item.level(), 1);
            assert_eq!(mi.item.section_title(), "Discrete Heading");
            assert_eq!(mi.item.section_type(), SectionType::Discrete);

            // Should have no child blocks.
            assert!(mi.item.nested_blocks().next().is_none());

            // The paragraph and next section should be unparsed.
            assert!(mi.after.data().contains("paragraph"));
            assert!(mi.after.data().contains("== Next Section"));

            assert!(warnings.is_empty());
        }
    }
}
