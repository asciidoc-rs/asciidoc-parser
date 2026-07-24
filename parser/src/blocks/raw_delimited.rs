use crate::{
    HasSpan, Parser, Span,
    attributes::Attrlist,
    blocks::{
        ChildBlocks, ContentModel, IsBlock, caption::assign_block_caption, metadata::BlockMetadata,
    },
    content::{Content, SubstitutionGroup},
    span::MatchedItem,
    strings::CowStr,
    warnings::{MatchAndWarnings, Warning, WarningType},
};

/// A delimited block that contains verbatim, raw, or comment text. The content
/// between the matching delimiters is not parsed for block syntax.
///
/// The following delimiters are recognized as raw delimited blocks:
///
/// | Delimiter | Content type |
/// |-----------|--------------|
/// | `////`    | Comment      |
/// | `----`    | Listing      |
/// | `` ``` `` | Listing (fenced) |
/// | `....`    | Literal      |
/// | `++++`    | Passthrough  |
///
/// The fenced code block delimiter (three backticks) is a shorthand for a
/// listing block. Like the open-block delimiter, it has a fixed length; four
/// or more backticks are not a fence.
///
/// A language may be declared on the opening fence (`` ```ruby ``). This is a
/// shorthand for a source block – equivalent to `[source,ruby]` over a listing
/// block – so the synthesized attribute list carries the `source` block style
/// and the language, and the closing fence is a bare `` ``` ``. This parser
/// records the language for a downstream renderer but performs no syntax
/// highlighting itself.
///
/// In addition, an open-block delimiter (`--`) is recognized here when it
/// carries a verbatim masquerade style: `source` or `listing` (parsed as a
/// listing block) or `literal` (parsed as a literal block). Every other open
/// block is handled by
/// [`CompoundDelimitedBlock`](crate::blocks::CompoundDelimitedBlock).
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct RawDelimitedBlock<'src> {
    content: Content<'src>,
    content_model: ContentModel,
    context: CowStr<'src>,
    source: Span<'src>,
    title_source: Option<Span<'src>>,
    title: Option<Content<'src>>,
    caption: Option<String>,
    number: Option<usize>,
    anchor: Option<Span<'src>>,
    anchor_reftext: Option<Span<'src>>,
    attrlist: Option<Attrlist<'src>>,
    substitution_group: SubstitutionGroup,
}

impl<'src> RawDelimitedBlock<'src> {
    /// Returns a document-order iterator over this block's direct child blocks.
    ///
    /// A raw delimited block (listing, literal, or passthrough) holds verbatim
    /// or raw content rather than child blocks, so this iterator is always
    /// empty. See [`FindBlocks`](crate::blocks::FindBlocks) to search from a
    /// [`Block`](crate::blocks::Block) or [`Document`](crate::Document).
    pub fn child_blocks(&'src self) -> ChildBlocks<'src> {
        ChildBlocks::empty()
    }

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

    pub(crate) fn is_valid_delimiter(line: &Span<'src>) -> bool {
        let data = line.data();

        // The fenced code block delimiter is exactly three backticks, optionally
        // followed by a language on the opening fence (```ruby). Unlike the
        // four-character verbatim/raw delimiters, its backtick run has a fixed
        // length (as with the two-character open-block delimiter): a run of four
        // or more backticks is not a fence.
        if data == "```" || fenced_code_language(line).is_some() {
            return true;
        }

        // Every character after the initial four must match the fourth
        // (delimiter) character. This matches Asciidoctor, whose
        // `is_delimited_block?` requires the run following the leading char to
        // be uniform (see https://github.com/asciidoc-rs/asciidoc-parser/issues/145
        // and https://gitlab.eclipse.org/eclipse/asciidoc-lang/asciidoc-lang/-/issues/56):
        // `----xyz` is not a delimiter, but `----` plus any number of `-` is.

        if data.len() >= 4 {
            if data.starts_with("////") {
                data.split_at(4).1.chars().all(|c| c == '/')
            } else if data.starts_with("----") {
                data.split_at(4).1.chars().all(|c| c == '-')
            } else if data.starts_with("....") {
                data.split_at(4).1.chars().all(|c| c == '.')
            } else if data.starts_with("++++") {
                data.split_at(4).1.chars().all(|c| c == '+')
            } else {
                false
            }
        } else {
            false
        }
    }

    pub(crate) fn parse(
        metadata: &BlockMetadata<'src>,
        parser: &mut Parser,
    ) -> Option<MatchAndWarnings<'src, Option<MatchedItem<'src, Self>>>> {
        let delimiter = metadata.block_start.take_normalized_line();
        let delimiter_data = delimiter.item.data();

        // The line that closes the block. Every delimiter closes on a line that
        // matches it exactly, except a language-aware fenced code block
        // (```ruby), whose closing fence is a bare ``` – set below.
        let mut close_delimiter = delimiter_data;

        // The attribute list synthesized for a language-aware fenced code block.
        // For every other block the author's own attribute list (if any) is used.
        let mut fenced_attrlist: Option<Attrlist<'src>> = None;

        // A `--` open-block delimiter normally forms a compound (open) block, but
        // a verbatim masquerade style (`source`, `listing`, or `literal`) set on
        // it turns the block into a verbatim raw block. Every other delimiter
        // must be at least four characters long.
        let (content_model, context, mut substitution_group) = if delimiter_data == "--" {
            // A plain or compound-styled open block returns `None` here and is
            // handled by `CompoundDelimitedBlock` instead.
            open_block_verbatim_masquerade(metadata.attrlist.as_ref())?
        } else if delimiter_data == "```" {
            // A fenced code block (three backticks) is a verbatim listing block.
            // Its delimiter has a fixed length, so no trailing-character
            // validity check is required. The closing fence must match the
            // opening delimiter exactly, which the scan loop below enforces.
            (
                ContentModel::Verbatim,
                "listing",
                SubstitutionGroup::Verbatim,
            )
        } else if let Some(language) = fenced_code_language(&delimiter.item) {
            // A fenced code block whose opening fence carries a language
            // (```ruby) is shorthand for a source block: `[source,<language>]`
            // over a listing block. The closing fence is a bare ``` (the
            // language appears only on the opening fence). When the author has
            // not supplied their own attribute list, synthesize the equivalent
            // `[source,<language>]` so a downstream renderer can resolve the
            // source language; an explicit attribute list takes precedence.
            close_delimiter = "```";
            if metadata.attrlist.is_none() {
                fenced_attrlist = Some(Attrlist::source_with_language(language));
            }
            (
                ContentModel::Verbatim,
                "listing",
                SubstitutionGroup::Verbatim,
            )
        } else {
            if delimiter.item.len() < 4 {
                return None;
            }

            let block_type = match delimiter_data
                .split_at_checked(delimiter_data.len().min(4))?
                .0
            {
                "////" => (ContentModel::Raw, "comment", SubstitutionGroup::None),
                "----" => (
                    ContentModel::Verbatim,
                    "listing",
                    SubstitutionGroup::Verbatim,
                ),
                "...." => (
                    ContentModel::Verbatim,
                    "literal",
                    SubstitutionGroup::Verbatim,
                ),
                "++++" => pass_or_stem_block_type(metadata.attrlist.as_ref()),
                _ => {
                    return None;
                }
            };

            // The four-character delimiters require a validity check (the
            // trailing characters must match the first four); the `--` open
            // delimiter is matched exactly above.
            if !Self::is_valid_delimiter(&delimiter.item) {
                return None;
            }

            block_type
        };

        // The block's effective attribute list: the one synthesized for a
        // language-aware fenced code block, or otherwise the author's own list.
        let attrlist = fenced_attrlist.or_else(|| metadata.attrlist.clone());

        // Assign the caption (and its number) from the block's context. Among
        // the raw delimited contexts only `listing` is captionable (a `source`
        // block resolves to the `listing` context); for every other context
        // `assign_block_caption` returns `None`. The caption is computed once,
        // here, so the context counter is consumed exactly once regardless of
        // which return path the block takes below.
        let caption =
            assign_block_caption(parser, context, attrlist.as_ref(), metadata.title.is_some());
        let number = caption.as_ref().and_then(|c| c.number);
        let caption = caption.map(|c| c.prefix);

        let content_start = delimiter.after;
        let mut next = content_start;

        while !next.is_empty() {
            let line = next.take_normalized_line();
            if line.item.data() == close_delimiter {
                let content = content_start.trim_remainder(next).trim_trailing_line_end();

                let mut content: Content<'src> = content.into();

                // A comment block (`////` or a `[comment]` open block) is never
                // interpreted, so a `subs` attribute must not override its
                // (empty) substitution group; every other raw context honors a
                // `subs` override.
                if context != "comment" {
                    substitution_group =
                        substitution_group.override_via_attrlist(attrlist.as_ref(), Some(parser));
                }

                substitution_group.apply(&mut content, parser, attrlist.as_ref());

                return Some(MatchAndWarnings {
                    item: Some(MatchedItem {
                        item: Self {
                            content,
                            content_model,
                            context: context.into(),
                            source: metadata
                                .source
                                .trim_remainder(line.after)
                                .trim_trailing_line_end(),
                            title_source: metadata.title_source,
                            title: metadata.title.clone(),
                            caption: caption.clone(),
                            number,
                            anchor: metadata.anchor,
                            anchor_reftext: metadata.anchor_reftext,
                            attrlist: attrlist.clone(),
                            substitution_group,
                        },
                        after: line.after,
                    }),
                    warnings: vec![],
                });
            }

            next = line.after;
        }

        let content = content_start.trim_remainder(next).trim_trailing_line_end();

        Some(MatchAndWarnings {
            item: Some(MatchedItem {
                item: Self {
                    content: content.into(),
                    content_model,
                    context: context.into(),
                    source: metadata
                        .source
                        .trim_remainder(next)
                        .trim_trailing_line_end(),
                    title_source: metadata.title_source,
                    title: metadata.title.clone(),
                    caption,
                    number,
                    anchor: metadata.anchor,
                    anchor_reftext: metadata.anchor_reftext,
                    attrlist,
                    substitution_group,
                },
                after: next,
            }),
            warnings: vec![Warning {
                source: delimiter.item,
                warning: WarningType::UnterminatedDelimitedBlock,
                origin: None,
            }],
        })
    }

    /// Return the interpreted content of this block.
    pub fn content(&self) -> &Content<'src> {
        &self.content
    }
}

/// If `line` opens a language-aware fenced code block, return the language
/// declared on the opening fence.
///
/// A language-aware fence is exactly three backticks immediately followed by an
/// info string (`` ```ruby ``). The language is the first whitespace-delimited
/// token of that info string. A bare `` ``` `` fence, a run of four or more
/// backticks, and any non-fence line all return `None`.
fn fenced_code_language<'src>(line: &Span<'src>) -> Option<Span<'src>> {
    let rest = line.data().strip_prefix("```")?;

    // A bare fence carries no language; a fourth backtick makes this a longer
    // run, which is not a fence.
    if rest.is_empty() || rest.starts_with('`') {
        return None;
    }

    // The language is the first whitespace-delimited token of the info string
    // (any leading whitespace is skipped; trailing content is ignored).
    let info = line.discard(3).take_whitespace().after;
    let language = info.take_while(|c| c != ' ' && c != '\t').item;

    if language.is_empty() {
        None
    } else {
        Some(language)
    }
}

/// Resolve a raw or verbatim masquerade style set on an open block (`--`).
///
/// A block style replaces the open-block context only on an open block (every
/// other delimited block keeps its own context). This parser claims an open
/// block whose style turns it into a raw delimited block:
///
/// * the verbatim contexts – `source` and `listing` (both rendered as a listing
///   block), or `literal` – make the open block a verbatim raw block;
/// * the `pass` context makes the open block a passthrough (raw) block, whose
///   content is emitted with no substitutions and no block parsing; and
/// * the `comment` context makes the open block a comment block – the alternate
///   open-block form of a `////` comment block. Its content is retained in the
///   parsed model (this parser deliberately does not discard comments) but is
///   raw: no substitutions are applied and no AsciiDoc syntax within it,
///   including preprocessor directives, is interpreted.
///
/// Returns the resulting content model, context, and substitution group, or
/// `None` when the style is absent or names a context that keeps the compound
/// (open) content model (e.g. `sidebar`, `example`, `quote`), which is handled
/// elsewhere.
fn open_block_verbatim_masquerade(
    attrlist: Option<&Attrlist<'_>>,
) -> Option<(ContentModel, &'static str, SubstitutionGroup)> {
    match attrlist?.block_style()? {
        "source" | "listing" => Some((
            ContentModel::Verbatim,
            "listing",
            SubstitutionGroup::Verbatim,
        )),
        "literal" => Some((
            ContentModel::Verbatim,
            "literal",
            SubstitutionGroup::Verbatim,
        )),
        "pass" => Some((ContentModel::Raw, "pass", SubstitutionGroup::Pass)),
        "comment" => Some((ContentModel::Raw, "comment", SubstitutionGroup::None)),
        _ => None,
    }
}

/// Resolve the content model, context, and substitution group for a passthrough
/// (`++++`) delimited block.
///
/// A passthrough block normally has the `pass` context and applies no
/// substitutions. When it carries a STEM style (`stem`, `asciimath`, or
/// `latexmath`), it instead becomes a `stem` block: a raw block whose
/// expression has only the special characters substitution applied (the
/// notation's math delimiters are added by the converter at render time).
fn pass_or_stem_block_type(
    attrlist: Option<&Attrlist<'_>>,
) -> (ContentModel, &'static str, SubstitutionGroup) {
    match attrlist.and_then(|a| a.block_style()) {
        Some("stem") | Some("asciimath") | Some("latexmath") => {
            (ContentModel::Raw, "stem", SubstitutionGroup::Stem)
        }
        _ => (ContentModel::Raw, "pass", SubstitutionGroup::Pass),
    }
}

impl<'src> IsBlock<'src> for RawDelimitedBlock<'src> {
    fn content_model(&self) -> ContentModel {
        self.content_model
    }

    fn content_mut(&mut self) -> Option<&mut Content<'src>> {
        Some(&mut self.content)
    }

    fn rendered_content(&self) -> Option<&str> {
        Some(self.content.rendered())
    }

    fn raw_context(&self) -> CowStr<'src> {
        self.context.clone()
    }

    fn title_source(&'src self) -> Option<Span<'src>> {
        self.title_source
    }

    fn title(&self) -> Option<&str> {
        self.title.as_ref().map(Content::rendered_str)
    }

    fn caption(&self) -> Option<&str> {
        self.caption.as_deref()
    }

    fn number(&self) -> Option<usize> {
        self.number
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

    fn substitution_group(&'src self) -> SubstitutionGroup {
        self.substitution_group.clone()
    }
}

impl<'src> HasSpan<'src> for RawDelimitedBlock<'src> {
    fn span(&self) -> Span<'src> {
        self.source
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    mod is_valid_delimiter {
        use crate::blocks::RawDelimitedBlock;

        #[test]
        fn comment() {
            assert!(RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "////"
            )));
            assert!(RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "/////"
            )));
            assert!(RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "/////////"
            )));

            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "///"
            )));
            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "//-/"
            )));
            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "////-"
            )));
            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "//////////x"
            )));
        }

        #[test]
        fn example() {
            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "===="
            )));
            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "====="
            )));
            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "==="
            )));
        }

        #[test]
        fn listing() {
            assert!(RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "----"
            )));
            assert!(RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "-----"
            )));
            assert!(RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "---------"
            )));

            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "---"
            )));
            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "--/-"
            )));
            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "----/"
            )));
            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "----------x"
            )));
        }

        #[test]
        fn fenced() {
            // The fenced code block delimiter is exactly three backticks ...
            assert!(RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "```"
            )));

            // ... optionally followed by a language on the opening fence.
            assert!(RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "```java"
            )));
            assert!(RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "```ruby"
            )));

            // A run of four or more backticks is not a fence (the backtick run
            // has a fixed length, unlike the four-character delimiters).
            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "````"
            )));
            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "``"
            )));

            // Backticks followed only by whitespace declare no language, so the
            // line is not a language-aware fence.
            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "```\t"
            )));
            assert!(super::super::fenced_code_language(&crate::Span::new("```  ")).is_none());

            // A language token is extracted from the info string, skipping any
            // leading whitespace and ignoring anything after the first token.
            assert_eq!(
                super::super::fenced_code_language(&crate::Span::new("``` ruby extra"))
                    .map(|s| s.data()),
                Some("ruby")
            );
        }

        #[test]
        fn literal() {
            assert!(RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "...."
            )));
            assert!(RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "....."
            )));
            assert!(RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "........."
            )));

            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "..."
            )));
            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "../."
            )));
            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "..../"
            )));
            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "..........x"
            )));
        }

        #[test]
        fn sidebar() {
            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "****"
            )));
            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "*****"
            )));
            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "***"
            )));
        }

        #[test]
        fn table() {
            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "|==="
            )));
            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                ",==="
            )));
            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                ":==="
            )));
            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "!==="
            )));
        }

        #[test]
        fn pass() {
            assert!(RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "++++"
            )));
            assert!(RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "+++++"
            )));
            assert!(RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "+++++++++"
            )));

            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "+++"
            )));
            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "++/+"
            )));
            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "++++/"
            )));
            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "++++++++++x"
            )));
        }

        #[test]
        fn quote() {
            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "____"
            )));
            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "_____"
            )));
            assert!(!RawDelimitedBlock::is_valid_delimiter(&crate::Span::new(
                "___"
            )));
        }
    }

    mod parse {
        use crate::{blocks::metadata::BlockMetadata, tests::prelude::*};

        #[test]
        fn err_invalid_delimiter() {
            let mut parser = Parser::default();
            assert!(
                crate::blocks::RawDelimitedBlock::parse(&BlockMetadata::new(""), &mut parser)
                    .is_none()
            );

            let mut parser = Parser::default();
            assert!(
                crate::blocks::RawDelimitedBlock::parse(&BlockMetadata::new("..."), &mut parser)
                    .is_none()
            );

            let mut parser = Parser::default();
            assert!(
                crate::blocks::RawDelimitedBlock::parse(&BlockMetadata::new("++++x"), &mut parser)
                    .is_none()
            );

            let mut parser = Parser::default();
            assert!(
                crate::blocks::RawDelimitedBlock::parse(&BlockMetadata::new("____x"), &mut parser)
                    .is_none()
            );

            let mut parser = Parser::default();
            assert!(
                crate::blocks::RawDelimitedBlock::parse(&BlockMetadata::new("====x"), &mut parser)
                    .is_none()
            );

            let mut parser = Parser::default();
            assert!(
                crate::blocks::RawDelimitedBlock::parse(&BlockMetadata::new("==\n=="), &mut parser)
                    .is_none()
            );

            let mut parser = Parser::default();
            assert!(
                crate::blocks::RawDelimitedBlock::parse(&BlockMetadata::new("===😀"), &mut parser)
                    .is_none()
            );
        }

        #[test]
        fn err_unterminated() {
            let mut parser = Parser::default();

            let maw = crate::blocks::RawDelimitedBlock::parse(
                &BlockMetadata::new("....\nblah blah blah"),
                &mut parser,
            )
            .unwrap();

            assert_eq!(
                maw.warnings,
                vec![Warning {
                    source: Span {
                        data: "....",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    warning: WarningType::UnterminatedDelimitedBlock,
                }]
            );
        }
    }

    mod comment {
        use crate::{
            blocks::{ContentModel, IsBlock, metadata::BlockMetadata},
            tests::prelude::*,
        };

        #[test]
        fn empty() {
            let mut parser = Parser::default();
            let maw = crate::blocks::RawDelimitedBlock::parse(
                &BlockMetadata::new("////\n////"),
                &mut parser,
            )
            .unwrap();

            let mi = maw.item.unwrap().clone();

            assert_eq!(
                mi.item,
                RawDelimitedBlock {
                    content: Content {
                        original: Span {
                            data: "",
                            line: 2,
                            col: 1,
                            offset: 5,
                        },
                        rendered: "",
                    },
                    content_model: ContentModel::Raw,
                    context: "comment",
                    source: Span {
                        data: "////\n////",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    substitution_group: SubstitutionGroup::None,
                }
            );

            assert_eq!(mi.item.content_model(), ContentModel::Raw);
            assert_eq!(mi.item.rendered_content().unwrap(), "");
            assert_eq!(mi.item.raw_context().as_ref(), "comment");
            assert_eq!(mi.item.resolved_context().as_ref(), "comment");
            assert!(mi.item.declared_style().is_none());
            assert!(mi.item.content().is_empty());
            assert!(mi.item.id().is_none());
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::None);
        }

        #[test]
        fn multiple_lines() {
            let mut parser = Parser::default();

            let maw = crate::blocks::RawDelimitedBlock::parse(
                &BlockMetadata::new("////\nline1  \nline2\n////"),
                &mut parser,
            )
            .unwrap();

            let mi = maw.item.unwrap().clone();

            assert_eq!(
                mi.item,
                RawDelimitedBlock {
                    content: Content {
                        original: Span {
                            data: "line1  \nline2",
                            line: 2,
                            col: 1,
                            offset: 5,
                        },
                        rendered: "line1  \nline2",
                    },
                    content_model: ContentModel::Raw,
                    context: "comment",
                    source: Span {
                        data: "////\nline1  \nline2\n////",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    substitution_group: SubstitutionGroup::None,
                }
            );

            assert_eq!(mi.item.content_model(), ContentModel::Raw);
            assert_eq!(mi.item.rendered_content().unwrap(), "line1  \nline2");
            assert_eq!(mi.item.raw_context().as_ref(), "comment");
            assert_eq!(mi.item.resolved_context().as_ref(), "comment");
            assert!(mi.item.declared_style().is_none());
            assert!(mi.item.id().is_none());
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::None);

            assert_eq!(
                mi.item.content(),
                Content {
                    original: Span {
                        data: "line1  \nline2",
                        line: 2,
                        col: 1,
                        offset: 5,
                    },
                    rendered: "line1  \nline2",
                }
            );
        }

        #[test]
        fn ignores_delimiter_prefix() {
            let mut parser = Parser::default();

            let maw = crate::blocks::RawDelimitedBlock::parse(
                &BlockMetadata::new("////\nline1  \n/////\nline2\n////"),
                &mut parser,
            )
            .unwrap();

            let mi = maw.item.unwrap().clone();

            assert_eq!(
                mi.item,
                RawDelimitedBlock {
                    content: Content {
                        original: Span {
                            data: "line1  \n/////\nline2",
                            line: 2,
                            col: 1,
                            offset: 5,
                        },
                        rendered: "line1  \n/////\nline2",
                    },
                    content_model: ContentModel::Raw,
                    context: "comment",
                    source: Span {
                        data: "////\nline1  \n/////\nline2\n////",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    substitution_group: SubstitutionGroup::None,
                }
            );

            assert_eq!(mi.item.content_model(), ContentModel::Raw);
            assert_eq!(mi.item.raw_context().as_ref(), "comment");
            assert_eq!(mi.item.resolved_context().as_ref(), "comment");
            assert!(mi.item.declared_style().is_none());
            assert!(mi.item.id().is_none());
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::None);

            assert_eq!(
                mi.item.content(),
                Content {
                    original: Span {
                        data: "line1  \n/////\nline2",
                        line: 2,
                        col: 1,
                        offset: 5,
                    },
                    rendered: "line1  \n/////\nline2",
                }
            );
        }

        #[test]
        fn no_panic_for_utf8_code_point_using_more_than_one_byte() {
            let mut parser = Parser::default();
            assert!(
                crate::blocks::RawDelimitedBlock::parse(&BlockMetadata::new("///😀"), &mut parser)
                    .is_none()
            );
        }
    }

    mod example {
        use crate::{blocks::metadata::BlockMetadata, tests::prelude::*};

        #[test]
        fn empty() {
            let mut parser = Parser::default();
            assert!(
                crate::blocks::RawDelimitedBlock::parse(
                    &BlockMetadata::new("====\n===="),
                    &mut parser
                )
                .is_none()
            );
        }

        #[test]
        fn multiple_lines() {
            let mut parser = Parser::default();
            assert!(
                crate::blocks::RawDelimitedBlock::parse(
                    &BlockMetadata::new("====\nline1  \nline2\n===="),
                    &mut parser
                )
                .is_none()
            );
        }
    }

    mod listing {
        use crate::{
            blocks::{ContentModel, metadata::BlockMetadata},
            content::SubstitutionStep,
            tests::prelude::*,
        };

        #[test]
        fn empty() {
            let mut parser = Parser::default();

            let maw = crate::blocks::RawDelimitedBlock::parse(
                &BlockMetadata::new("----\n----"),
                &mut parser,
            )
            .unwrap();

            let mi = maw.item.unwrap().clone();

            assert_eq!(
                mi.item,
                RawDelimitedBlock {
                    content: Content {
                        original: Span {
                            data: "",
                            line: 2,
                            col: 1,
                            offset: 5,
                        },
                        rendered: "",
                    },
                    content_model: ContentModel::Verbatim,
                    context: "listing",
                    source: Span {
                        data: "----\n----",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    substitution_group: SubstitutionGroup::Verbatim,
                }
            );

            assert_eq!(mi.item.content_model(), ContentModel::Verbatim);
            assert_eq!(mi.item.raw_context().as_ref(), "listing");
            assert_eq!(mi.item.resolved_context().as_ref(), "listing");
            assert!(mi.item.declared_style().is_none());
            assert!(mi.item.content().is_empty());
            assert!(mi.item.id().is_none());
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Verbatim);
        }

        #[test]
        fn multiple_lines() {
            let mut parser = Parser::default();

            let maw = crate::blocks::RawDelimitedBlock::parse(
                &BlockMetadata::new("----\nline1  \nline2\n----"),
                &mut parser,
            )
            .unwrap();

            let mi = maw.item.unwrap().clone();

            assert_eq!(
                mi.item,
                RawDelimitedBlock {
                    content: Content {
                        original: Span {
                            data: "line1  \nline2",
                            line: 2,
                            col: 1,
                            offset: 5,
                        },
                        rendered: "line1  \nline2",
                    },
                    content_model: ContentModel::Verbatim,
                    context: "listing",
                    source: Span {
                        data: "----\nline1  \nline2\n----",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    substitution_group: SubstitutionGroup::Verbatim,
                }
            );

            assert_eq!(mi.item.content_model(), ContentModel::Verbatim);
            assert_eq!(mi.item.raw_context().as_ref(), "listing");
            assert_eq!(mi.item.resolved_context().as_ref(), "listing");
            assert!(mi.item.declared_style().is_none());
            assert!(mi.item.id().is_none());
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Verbatim);

            assert_eq!(
                mi.item.content(),
                Content {
                    original: Span {
                        data: "line1  \nline2",
                        line: 2,
                        col: 1,
                        offset: 5,
                    },
                    rendered: "line1  \nline2",
                }
            );
        }

        #[test]
        fn overrides_sub_group_via_subs_attribute() {
            let mut parser = Parser::default();

            let maw = crate::blocks::RawDelimitedBlock::parse(
                &BlockMetadata::new("[subs=quotes]\n----\nline1 < *line2*\n----"),
                &mut parser,
            )
            .unwrap();

            let mi = maw.item.unwrap().clone();

            assert_eq!(
                mi.item,
                RawDelimitedBlock {
                    content: Content {
                        original: Span {
                            data: "line1 < *line2*",
                            line: 3,
                            col: 1,
                            offset: 19,
                        },
                        rendered: "line1 < <strong>line2</strong>",
                    },
                    content_model: ContentModel::Verbatim,
                    context: "listing",
                    source: Span {
                        data: "[subs=quotes]\n----\nline1 < *line2*\n----",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: Some(Attrlist {
                        attributes: &[ElementAttribute {
                            name: Some("subs"),
                            value: "quotes",
                            shorthand_items: &[],
                        },],
                        anchor: None,
                        source: Span {
                            data: "subs=quotes",
                            line: 1,
                            col: 2,
                            offset: 1,
                        },
                    },),
                    substitution_group: SubstitutionGroup::Custom(vec![SubstitutionStep::Quotes]),
                }
            );

            assert_eq!(mi.item.content_model(), ContentModel::Verbatim);
            assert_eq!(mi.item.raw_context().as_ref(), "listing");
            assert_eq!(mi.item.resolved_context().as_ref(), "listing");
            assert!(mi.item.declared_style().is_none());
            assert!(mi.item.id().is_none());
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());

            assert_eq!(
                mi.item.attrlist().unwrap(),
                Attrlist {
                    attributes: &[ElementAttribute {
                        name: Some("subs"),
                        value: "quotes",
                        shorthand_items: &[],
                    },],
                    anchor: None,
                    source: Span {
                        data: "subs=quotes",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                }
            );

            assert_eq!(
                mi.item.substitution_group(),
                SubstitutionGroup::Custom(vec![SubstitutionStep::Quotes])
            );

            assert_eq!(
                mi.item.content(),
                Content {
                    original: Span {
                        data: "line1 < *line2*",
                        line: 3,
                        col: 1,
                        offset: 19,
                    },
                    rendered: "line1 < <strong>line2</strong>",
                }
            );
        }

        #[test]
        fn ignores_delimiter_prefix() {
            let mut parser = Parser::default();

            let maw = crate::blocks::RawDelimitedBlock::parse(
                &BlockMetadata::new("----\nline1  \n-----\nline2\n----"),
                &mut parser,
            )
            .unwrap();

            let mi = maw.item.unwrap().clone();

            assert_eq!(
                mi.item,
                RawDelimitedBlock {
                    content: Content {
                        original: Span {
                            data: "line1  \n-----\nline2",
                            line: 2,
                            col: 1,
                            offset: 5,
                        },
                        rendered: "line1  \n-----\nline2",
                    },
                    content_model: ContentModel::Verbatim,
                    context: "listing",
                    source: Span {
                        data: "----\nline1  \n-----\nline2\n----",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    substitution_group: SubstitutionGroup::Verbatim,
                }
            );

            assert_eq!(mi.item.content_model(), ContentModel::Verbatim);
            assert_eq!(mi.item.raw_context().as_ref(), "listing");
            assert_eq!(mi.item.resolved_context().as_ref(), "listing");
            assert!(mi.item.declared_style().is_none());
            assert!(mi.item.id().is_none());
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Verbatim);

            assert_eq!(
                mi.item.content(),
                Content {
                    original: Span {
                        data: "line1  \n-----\nline2",
                        line: 2,
                        col: 1,
                        offset: 5,
                    },
                    rendered: "line1  \n-----\nline2",
                }
            );

            assert_eq!(
                mi.item.content(),
                Content {
                    original: Span {
                        data: "line1  \n-----\nline2",
                        line: 2,
                        col: 1,
                        offset: 5,
                    },
                    rendered: "line1  \n-----\nline2",
                }
            );
        }
    }

    mod sidebar {
        use crate::{blocks::metadata::BlockMetadata, tests::prelude::*};

        #[test]
        fn empty() {
            let mut parser = Parser::default();
            assert!(
                crate::blocks::RawDelimitedBlock::parse(
                    &BlockMetadata::new("****\n****"),
                    &mut parser
                )
                .is_none()
            );
        }

        #[test]
        fn multiple_lines() {
            let mut parser = Parser::default();
            assert!(
                crate::blocks::RawDelimitedBlock::parse(
                    &BlockMetadata::new("****\nline1  \nline2\n****"),
                    &mut parser
                )
                .is_none()
            );
        }
    }

    mod table {
        use crate::{blocks::metadata::BlockMetadata, tests::prelude::*};

        #[test]
        fn empty() {
            let mut parser = Parser::default();
            assert!(
                crate::blocks::RawDelimitedBlock::parse(
                    &BlockMetadata::new("|===\n|==="),
                    &mut parser
                )
                .is_none()
            );

            let mut parser = Parser::default();
            assert!(
                crate::blocks::RawDelimitedBlock::parse(
                    &BlockMetadata::new(",===\n,==="),
                    &mut parser
                )
                .is_none()
            );

            let mut parser = Parser::default();
            assert!(
                crate::blocks::RawDelimitedBlock::parse(
                    &BlockMetadata::new(":===\n:==="),
                    &mut parser
                )
                .is_none()
            );

            let mut parser = Parser::default();
            assert!(
                crate::blocks::RawDelimitedBlock::parse(
                    &BlockMetadata::new("!===\n!==="),
                    &mut parser
                )
                .is_none()
            );
        }

        #[test]
        fn multiple_lines() {
            let mut parser = Parser::default();
            assert!(
                crate::blocks::RawDelimitedBlock::parse(
                    &BlockMetadata::new("|===\nline1  \nline2\n|==="),
                    &mut parser
                )
                .is_none()
            );

            let mut parser = Parser::default();
            assert!(
                crate::blocks::RawDelimitedBlock::parse(
                    &BlockMetadata::new(",===\nline1  \nline2\n,==="),
                    &mut parser
                )
                .is_none()
            );

            let mut parser = Parser::default();
            assert!(
                crate::blocks::RawDelimitedBlock::parse(
                    &BlockMetadata::new(":===\nline1  \nline2\n:==="),
                    &mut parser
                )
                .is_none()
            );

            let mut parser = Parser::default();
            assert!(
                crate::blocks::RawDelimitedBlock::parse(
                    &BlockMetadata::new("!===\nline1  \nline2\n!==="),
                    &mut parser
                )
                .is_none()
            );
        }
    }

    mod pass {
        use crate::{
            blocks::{ContentModel, metadata::BlockMetadata},
            tests::prelude::*,
        };

        #[test]
        fn empty() {
            let mut parser = Parser::default();
            let maw = crate::blocks::RawDelimitedBlock::parse(
                &BlockMetadata::new("++++\n++++"),
                &mut parser,
            )
            .unwrap();

            let mi = maw.item.unwrap().clone();

            assert_eq!(
                mi.item,
                RawDelimitedBlock {
                    content: Content {
                        original: Span {
                            data: "",
                            line: 2,
                            col: 1,
                            offset: 5,
                        },
                        rendered: "",
                    },
                    content_model: ContentModel::Raw,
                    context: "pass",
                    source: Span {
                        data: "++++\n++++",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    substitution_group: SubstitutionGroup::Pass,
                }
            );

            assert_eq!(mi.item.content_model(), ContentModel::Raw);
            assert_eq!(mi.item.raw_context().as_ref(), "pass");
            assert_eq!(mi.item.resolved_context().as_ref(), "pass");
            assert!(mi.item.declared_style().is_none());
            assert!(mi.item.content().is_empty());
            assert!(mi.item.id().is_none());
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Pass);
        }

        #[test]
        fn multiple_lines() {
            let mut parser = Parser::default();

            let maw = crate::blocks::RawDelimitedBlock::parse(
                &BlockMetadata::new("++++\nline1  \nline2\n++++"),
                &mut parser,
            )
            .unwrap();

            let mi = maw.item.unwrap().clone();

            assert_eq!(
                mi.item,
                RawDelimitedBlock {
                    content: Content {
                        original: Span {
                            data: "line1  \nline2",
                            line: 2,
                            col: 1,
                            offset: 5,
                        },
                        rendered: "line1  \nline2",
                    },
                    content_model: ContentModel::Raw,
                    context: "pass",
                    source: Span {
                        data: "++++\nline1  \nline2\n++++",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    substitution_group: SubstitutionGroup::Pass,
                }
            );

            assert_eq!(mi.item.content_model(), ContentModel::Raw);
            assert_eq!(mi.item.raw_context().as_ref(), "pass");
            assert_eq!(mi.item.resolved_context().as_ref(), "pass");
            assert!(mi.item.declared_style().is_none());
            assert!(mi.item.id().is_none());
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Pass);

            assert_eq!(
                mi.item.content(),
                Content {
                    original: Span {
                        data: "line1  \nline2",
                        line: 2,
                        col: 1,
                        offset: 5,
                    },
                    rendered: "line1  \nline2",
                }
            );
        }

        #[test]
        fn ignores_delimiter_prefix() {
            let mut parser = Parser::default();

            let maw = crate::blocks::RawDelimitedBlock::parse(
                &BlockMetadata::new("++++\nline1  \n+++++\nline2\n++++"),
                &mut parser,
            )
            .unwrap();

            let mi = maw.item.unwrap().clone();

            assert_eq!(
                mi.item,
                RawDelimitedBlock {
                    content: Content {
                        original: Span {
                            data: "line1  \n+++++\nline2",
                            line: 2,
                            col: 1,
                            offset: 5,
                        },
                        rendered: "line1  \n+++++\nline2",
                    },
                    content_model: ContentModel::Raw,
                    context: "pass",
                    source: Span {
                        data: "++++\nline1  \n+++++\nline2\n++++",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    caption: None,
                    number: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                    substitution_group: SubstitutionGroup::Pass,
                }
            );

            assert_eq!(mi.item.content_model(), ContentModel::Raw);
            assert_eq!(mi.item.raw_context().as_ref(), "pass");
            assert_eq!(mi.item.resolved_context().as_ref(), "pass");
            assert!(mi.item.declared_style().is_none());
            assert!(mi.item.id().is_none());
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Pass);

            assert_eq!(
                mi.item.content(),
                Content {
                    original: Span {
                        data: "line1  \n+++++\nline2",
                        line: 2,
                        col: 1,
                        offset: 5,
                    },
                    rendered: "line1  \n+++++\nline2",
                }
            );
        }
    }

    mod stem {
        use crate::{blocks::ContentModel, tests::prelude::*};

        /// A `[stem]` passthrough block becomes a `stem` block whose expression
        /// has only the special characters substitution applied. The notation's
        /// math delimiters are added by the converter at render time, so they
        /// do not appear in the parsed content.
        #[test]
        fn stem_style_block() {
            let doc = Parser::default().parse("[stem]\n++++\na < b\n++++");
            let block = doc.child_blocks().next().unwrap();

            assert_eq!(block.content_model(), ContentModel::Raw);
            assert_eq!(block.raw_context().as_ref(), "stem");
            assert_eq!(block.resolved_context().as_ref(), "stem");
            assert_eq!(block.declared_style(), Some("stem"));
            assert_eq!(block.rendered_content(), Some("a &lt; b"));
            assert_eq!(block.substitution_group(), SubstitutionGroup::Stem);
            assert!(doc.warnings().next().is_none());
        }

        #[test]
        fn asciimath_style_block() {
            let doc = Parser::default().parse("[asciimath]\n++++\nx^2\n++++");
            let block = doc.child_blocks().next().unwrap();

            assert_eq!(block.raw_context().as_ref(), "stem");
            assert_eq!(block.declared_style(), Some("asciimath"));
            assert_eq!(block.rendered_content(), Some("x^2"));
        }

        #[test]
        fn latexmath_style_block() {
            let doc = Parser::default().parse("[latexmath]\n++++\nC = \\alpha\n++++");
            let block = doc.child_blocks().next().unwrap();

            assert_eq!(block.raw_context().as_ref(), "stem");
            assert_eq!(block.declared_style(), Some("latexmath"));
            assert_eq!(block.rendered_content(), Some(r"C = \alpha"));
        }

        /// Without a STEM style, a `++++` block remains a `pass` block with no
        /// substitutions applied.
        #[test]
        fn unstyled_block_is_still_pass() {
            let doc = Parser::default().parse("++++\na < b\n++++");
            let block = doc.child_blocks().next().unwrap();

            assert_eq!(block.raw_context().as_ref(), "pass");
            assert_eq!(block.rendered_content(), Some("a < b"));
            assert_eq!(block.substitution_group(), SubstitutionGroup::Pass);
        }

        /// An explicit `subs` attribute still overrides a STEM block's default
        /// substitution group.
        #[test]
        fn subs_attribute_overrides_stem_default() {
            let doc = Parser::default().parse("[stem,subs=none]\n++++\na < b\n++++");
            let block = doc.child_blocks().next().unwrap();

            assert_eq!(block.raw_context().as_ref(), "stem");
            assert_eq!(block.rendered_content(), Some("a < b"));
            assert_eq!(block.substitution_group(), SubstitutionGroup::None);
        }
    }

    mod quote {
        use crate::{blocks::metadata::BlockMetadata, tests::prelude::*};

        #[test]
        fn empty() {
            let mut parser = Parser::default();
            assert!(
                crate::blocks::RawDelimitedBlock::parse(
                    &BlockMetadata::new("____\n____"),
                    &mut parser
                )
                .is_none()
            );
        }

        #[test]
        fn multiple_lines() {
            let mut parser = Parser::default();
            assert!(
                crate::blocks::RawDelimitedBlock::parse(
                    &BlockMetadata::new("____\nline1  \nline2\n____"),
                    &mut parser
                )
                .is_none()
            );
        }
    }
}
