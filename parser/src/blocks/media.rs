use crate::{
    HasSpan, Parser, Span,
    attributes::{Attrlist, AttrlistContext},
    blocks::{ContentModel, IsBlock, metadata::BlockMetadata},
    content::substitute_attributes_in_macro_target,
    span::MatchedItem,
    strings::CowStr,
    warnings::{MatchAndWarnings, Warning, WarningType},
};

/// A media block is used to represent an image, video, or audio block macro.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaBlock<'src> {
    type_: MediaType,
    target: Span<'src>,
    resolved_target: CowStr<'src>,
    macro_attrlist: Attrlist<'src>,
    source: Span<'src>,
    title_source: Option<Span<'src>>,
    title: Option<String>,
    anchor: Option<Span<'src>>,
    anchor_reftext: Option<Span<'src>>,
    attrlist: Option<Attrlist<'src>>,
}

/// Outcome of resolving attribute references in a media block's target via
/// [`MediaBlock::resolve_target`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TargetResolution {
    /// The target was resolved and stored; the block should be kept.
    Keep,

    /// The target referenced a missing attribute under
    /// `attribute-missing=drop-line`, so the entire block should be dropped.
    Drop,
}

/// A media type may be one of three different types.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum MediaType {
    /// Still image
    Image,

    /// Video
    Video,

    /// Audio
    Audio,
}

impl std::fmt::Debug for MediaType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MediaType::Image => write!(f, "MediaType::Image"),
            MediaType::Video => write!(f, "MediaType::Video"),
            MediaType::Audio => write!(f, "MediaType::Audio"),
        }
    }
}

impl<'src> MediaBlock<'src> {
    pub(crate) fn parse(
        metadata: &BlockMetadata<'src>,
        parser: &mut Parser,
    ) -> MatchAndWarnings<'src, Option<MatchedItem<'src, Self>>> {
        let line = metadata.block_start.take_normalized_line();

        // Line must end with `]`; otherwise, it's not a block macro.
        if !line.item.ends_with(']') {
            return MatchAndWarnings {
                item: None,
                warnings: vec![],
            };
        }

        let Some(name) = line.item.take_ident() else {
            return MatchAndWarnings {
                item: None,
                warnings: vec![],
            };
        };

        let type_ = match name.item.data() {
            "image" => MediaType::Image,
            "video" => MediaType::Video,
            "audio" => MediaType::Audio,
            _ => {
                return MatchAndWarnings {
                    item: None,
                    warnings: vec![],
                };
            }
        };

        let Some(colons) = name.after.take_prefix("::") else {
            return MatchAndWarnings {
                item: None,
                warnings: vec![Warning {
                    source: name.after,
                    warning: WarningType::MacroMissingDoubleColon,
                }],
            };
        };

        // The target field must exist and be non-empty.
        let target = colons.after.take_while(|c| c != '[');

        if target.item.is_empty() {
            return MatchAndWarnings {
                item: None,
                warnings: vec![Warning {
                    source: target.after,
                    warning: WarningType::MediaMacroMissingTarget,
                }],
            };
        }

        let Some(open_brace) = target.after.take_prefix("[") else {
            return MatchAndWarnings {
                item: None,
                warnings: vec![Warning {
                    source: target.after,
                    warning: WarningType::MacroMissingAttributeList,
                }],
            };
        };

        let attrlist = open_brace.after.slice(0..open_brace.after.len() - 1);
        // Note that we already checked that this line ends with a close brace.

        let macro_attrlist = Attrlist::parse(attrlist, parser, AttrlistContext::Inline);

        let source: Span = metadata.source.trim_remainder(line.after);
        let source = source.slice(0..source.trim().len());

        MatchAndWarnings {
            item: Some(MatchedItem {
                item: Self {
                    type_,
                    target: target.item,
                    // Attribute references in the target are resolved later, in
                    // `resolve_target` (which also decides whether a missing
                    // reference should drop the whole block); until then, the
                    // resolved target mirrors the raw target verbatim.
                    resolved_target: target.item.data().into(),
                    macro_attrlist: macro_attrlist.item.item,
                    source,
                    title_source: metadata.title_source,
                    title: metadata.title.clone(),
                    anchor: metadata.anchor,
                    anchor_reftext: metadata.anchor_reftext,
                    attrlist: metadata.attrlist.clone(),
                },

                after: line.after.discard_empty_lines(),
            }),
            warnings: macro_attrlist.warnings,
        }
    }

    /// Return a [`Span`] describing the macro name.
    pub fn type_(&self) -> MediaType {
        self.type_
    }

    /// Return a [`Span`] describing the macro target.
    ///
    /// This is the target exactly as written in the source, _before_ any
    /// attribute references within it are resolved. See
    /// [`resolved_target()`](Self::resolved_target) for the resolved form.
    pub fn target(&'src self) -> Option<&'src Span<'src>> {
        Some(&self.target)
    }

    /// Return the macro target after any attribute references within it have
    /// been resolved (honoring the [`attribute-missing`] document attribute).
    ///
    /// For the common case of a target with no attribute references, this is
    /// identical to the text of [`target()`](Self::target).
    ///
    /// [`attribute-missing`]: https://docs.asciidoctor.org/asciidoc/latest/attributes/unresolved-references/#missing
    pub fn resolved_target(&self) -> &str {
        self.resolved_target.as_ref()
    }

    /// Resolve attribute references in this block's target, honoring the
    /// [`attribute-missing`] document attribute.
    ///
    /// On success the resolved target is stored (see
    /// [`resolved_target()`](Self::resolved_target)) and
    /// [`TargetResolution::Keep`] is returned. When the target references a
    /// missing attribute and `attribute-missing=drop-line` is in effect,
    /// [`TargetResolution::Drop`] is returned and the caller drops the
    /// entire block.
    ///
    /// [`attribute-missing`]: https://docs.asciidoctor.org/asciidoc/latest/attributes/unresolved-references/#missing
    pub(crate) fn resolve_target(&mut self, parser: &Parser) -> TargetResolution {
        match substitute_attributes_in_macro_target(self.target, parser) {
            Some(resolved) => {
                self.resolved_target = resolved;
                TargetResolution::Keep
            }
            None => TargetResolution::Drop,
        }
    }

    /// Return the macro's attribute list.
    ///
    /// **IMPORTANT:** This is the list of attributes _within_ the macro block
    /// definition itself.
    ///
    /// See also [`attrlist()`] for attributes that can be defined before the
    /// macro invocation.
    ///
    /// [`attrlist()`]: Self::attrlist()
    pub fn macro_attrlist(&'src self) -> &'src Attrlist<'src> {
        &self.macro_attrlist
    }
}

impl<'src> IsBlock<'src> for MediaBlock<'src> {
    fn content_model(&self) -> ContentModel {
        ContentModel::Empty
    }

    fn raw_context(&self) -> CowStr<'src> {
        match self.type_ {
            MediaType::Audio => "audio",
            MediaType::Image => "image",
            MediaType::Video => "video",
        }
        .into()
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

impl<'src> HasSpan<'src> for MediaBlock<'src> {
    fn span(&self) -> Span<'src> {
        self.source
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::ops::Deref;

    use crate::{
        blocks::{ContentModel, MediaType, metadata::BlockMetadata},
        tests::prelude::*,
    };

    #[test]
    fn impl_clone() {
        // Silly test to mark the #[derive(...)] line as covered.
        let mut parser = Parser::default();

        let b1 =
            crate::blocks::MediaBlock::parse(&BlockMetadata::new("image::foo.jpg[]"), &mut parser)
                .unwrap_if_no_warnings()
                .unwrap()
                .item;

        let b2 = b1.clone();
        assert_eq!(b1, b2);
    }

    #[test]
    fn err_empty_source() {
        let mut parser = Parser::default();
        assert!(
            crate::blocks::MediaBlock::parse(&BlockMetadata::new(""), &mut parser)
                .unwrap_if_no_warnings()
                .is_none()
        );
    }

    #[test]
    fn err_only_spaces() {
        let mut parser = Parser::default();
        assert!(
            crate::blocks::MediaBlock::parse(&BlockMetadata::new("    "), &mut parser)
                .unwrap_if_no_warnings()
                .is_none()
        );
    }

    #[test]
    fn err_macro_name_not_ident() {
        let mut parser = Parser::default();
        let maw = crate::blocks::MediaBlock::parse(
            &BlockMetadata::new("98xyz::bar[blah,blap]"),
            &mut parser,
        );

        assert!(maw.item.is_none());
        assert!(maw.warnings.is_empty());
    }

    #[test]
    fn err_missing_double_colon() {
        let mut parser = Parser::default();
        let maw = crate::blocks::MediaBlock::parse(
            &BlockMetadata::new("image:bar[blah,blap]"),
            &mut parser,
        );

        assert!(maw.item.is_none());

        assert_eq!(
            maw.warnings,
            vec![Warning {
                source: Span {
                    data: ":bar[blah,blap]",
                    line: 1,
                    col: 6,
                    offset: 5,
                },
                warning: WarningType::MacroMissingDoubleColon,
            }]
        );
    }

    #[test]
    fn err_missing_macro_attrlist() {
        let mut parser = Parser::default();
        let maw = crate::blocks::MediaBlock::parse(
            &BlockMetadata::new("image::barblah,blap]"),
            &mut parser,
        );

        assert!(maw.item.is_none());

        assert_eq!(
            maw.warnings,
            vec![Warning {
                source: Span {
                    data: "",
                    line: 1,
                    col: 21,
                    offset: 20,
                },
                warning: WarningType::MacroMissingAttributeList,
            }]
        );
    }

    #[test]
    fn err_unknown_type() {
        let mut parser = Parser::default();
        assert!(
            crate::blocks::MediaBlock::parse(&BlockMetadata::new("imagex::bar[]"), &mut parser)
                .unwrap_if_no_warnings()
                .is_none()
        );
    }

    #[test]
    fn err_no_attr_list() {
        let mut parser = Parser::default();
        assert!(
            crate::blocks::MediaBlock::parse(&BlockMetadata::new("image::bar"), &mut parser)
                .unwrap_if_no_warnings()
                .is_none()
        );
    }

    #[test]
    fn err_attr_list_not_closed() {
        let mut parser = Parser::default();
        assert!(
            crate::blocks::MediaBlock::parse(&BlockMetadata::new("image::bar[blah"), &mut parser)
                .unwrap_if_no_warnings()
                .is_none()
        );
    }

    #[test]
    fn err_unexpected_after_attr_list() {
        let mut parser = Parser::default();
        assert!(
            crate::blocks::MediaBlock::parse(
                &BlockMetadata::new("image::bar[blah]bonus"),
                &mut parser
            )
            .unwrap_if_no_warnings()
            .is_none()
        );
    }

    #[test]
    fn simplest_block_macro() {
        let mut parser = Parser::default();

        let mi = crate::blocks::MediaBlock::parse(&BlockMetadata::new("image::[]"), &mut parser);
        assert!(mi.item.is_none());

        assert_eq!(
            mi.warnings,
            vec![Warning {
                source: Span {
                    data: "[]",
                    line: 1,
                    col: 8,
                    offset: 7,
                },
                warning: WarningType::MediaMacroMissingTarget,
            }]
        );
    }

    #[test]
    fn has_target() {
        let mut parser = Parser::default();

        let mi = crate::blocks::MediaBlock::parse(&BlockMetadata::new("image::bar[]"), &mut parser)
            .unwrap_if_no_warnings()
            .unwrap();

        assert_eq!(
            mi.item,
            MediaBlock {
                type_: MediaType::Image,
                target: Span {
                    data: "bar",
                    line: 1,
                    col: 8,
                    offset: 7,
                },
                macro_attrlist: Attrlist {
                    attributes: &[],
                    anchor: None,
                    source: Span {
                        data: "",
                        line: 1,
                        col: 12,
                        offset: 11,
                    }
                },
                source: Span {
                    data: "image::bar[]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            }
        );

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 1,
                col: 13,
                offset: 12
            }
        );

        assert_eq!(mi.item.content_model(), ContentModel::Empty);
        assert_eq!(mi.item.raw_context().deref(), "image");
        assert!(mi.item.nested_blocks().next().is_none());
        assert!(mi.item.title_source().is_none());
        assert!(mi.item.title().is_none());
        assert!(mi.item.anchor().is_none());
        assert!(mi.item.anchor_reftext().is_none());
        assert!(mi.item.attrlist().is_none());
        assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);
    }

    #[test]
    fn has_target_and_attrlist() {
        let mut parser = Parser::default();

        let mi =
            crate::blocks::MediaBlock::parse(&BlockMetadata::new("image::bar[blah]"), &mut parser)
                .unwrap_if_no_warnings()
                .unwrap();

        assert_eq!(
            mi.item,
            MediaBlock {
                type_: MediaType::Image,
                target: Span {
                    data: "bar",
                    line: 1,
                    col: 8,
                    offset: 7,
                },
                macro_attrlist: Attrlist {
                    attributes: &[ElementAttribute {
                        name: None,
                        shorthand_items: &["blah"],
                        value: "blah"
                    }],
                    anchor: None,
                    source: Span {
                        data: "blah",
                        line: 1,
                        col: 12,
                        offset: 11,
                    }
                },
                source: Span {
                    data: "image::bar[blah]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
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
    fn audio() {
        let mut parser = Parser::default();

        let mi = crate::blocks::MediaBlock::parse(&BlockMetadata::new("audio::bar[]"), &mut parser)
            .unwrap_if_no_warnings()
            .unwrap();

        assert_eq!(
            mi.item,
            MediaBlock {
                type_: MediaType::Audio,
                target: Span {
                    data: "bar",
                    line: 1,
                    col: 8,
                    offset: 7,
                },
                macro_attrlist: Attrlist {
                    attributes: &[],
                    anchor: None,
                    source: Span {
                        data: "",
                        line: 1,
                        col: 12,
                        offset: 11,
                    }
                },
                source: Span {
                    data: "audio::bar[]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            }
        );

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 1,
                col: 13,
                offset: 12
            }
        );

        assert_eq!(mi.item.content_model(), ContentModel::Empty);
        assert_eq!(mi.item.raw_context().deref(), "audio");
        assert!(mi.item.nested_blocks().next().is_none());
        assert!(mi.item.title_source().is_none());
        assert!(mi.item.title().is_none());
        assert!(mi.item.anchor().is_none());
        assert!(mi.item.anchor_reftext().is_none());
        assert!(mi.item.attrlist().is_none());
        assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);
    }

    #[test]
    fn video() {
        let mut parser = Parser::default();

        let mi = crate::blocks::MediaBlock::parse(&BlockMetadata::new("video::bar[]"), &mut parser)
            .unwrap_if_no_warnings()
            .unwrap();

        assert_eq!(
            mi.item,
            MediaBlock {
                type_: MediaType::Video,
                target: Span {
                    data: "bar",
                    line: 1,
                    col: 8,
                    offset: 7,
                },
                macro_attrlist: Attrlist {
                    attributes: &[],
                    anchor: None,
                    source: Span {
                        data: "",
                        line: 1,
                        col: 12,
                        offset: 11,
                    }
                },
                source: Span {
                    data: "video::bar[]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            }
        );

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 1,
                col: 13,
                offset: 12
            }
        );

        assert_eq!(mi.item.content_model(), ContentModel::Empty);
        assert_eq!(mi.item.raw_context().deref(), "video");
        assert!(mi.item.nested_blocks().next().is_none());
        assert!(mi.item.title_source().is_none());
        assert!(mi.item.title().is_none());
        assert!(mi.item.anchor().is_none());
        assert!(mi.item.anchor_reftext().is_none());
        assert!(mi.item.attrlist().is_none());
        assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);
    }

    #[test]
    fn err_duplicate_comma() {
        let mut parser = Parser::default();
        let maw = crate::blocks::MediaBlock::parse(
            &BlockMetadata::new("image::bar[blah,,blap]"),
            &mut parser,
        );

        let mi = maw.item.unwrap().clone();

        assert_eq!(
            mi.item,
            MediaBlock {
                type_: MediaType::Image,
                target: Span {
                    data: "bar",
                    line: 1,
                    col: 8,
                    offset: 7,
                },
                macro_attrlist: Attrlist {
                    attributes: &[
                        ElementAttribute {
                            name: None,
                            shorthand_items: &["blah"],
                            value: "blah"
                        },
                        ElementAttribute {
                            name: None,
                            shorthand_items: &[],
                            value: "blap"
                        }
                    ],
                    anchor: None,
                    source: Span {
                        data: "blah,,blap",
                        line: 1,
                        col: 12,
                        offset: 11,
                    }
                },
                source: Span {
                    data: "image::bar[blah,,blap]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            }
        );

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 1,
                col: 23,
                offset: 22
            }
        );

        assert_eq!(
            maw.warnings,
            vec![Warning {
                source: Span {
                    data: "blah,,blap",
                    line: 1,
                    col: 12,
                    offset: 11,
                },
                warning: WarningType::EmptyAttributeValue,
            }]
        );
    }

    mod target_resolution {
        #![allow(clippy::indexing_slicing)]

        use crate::{
            blocks::{MediaBlock, media::TargetResolution, metadata::BlockMetadata},
            parser::ModificationContext,
            tests::prelude::*,
            warnings::WarningType,
        };

        /// Parses `input` as a media block and resolves its target against
        /// `parser`, returning the resolved [`MediaBlock`] (or `None` if the
        /// block was dropped).
        fn resolve<'i>(input: &'i str, parser: &mut Parser) -> Option<MediaBlock<'i>> {
            let mut block = MediaBlock::parse(&BlockMetadata::new(input), parser)
                .unwrap_if_no_warnings()
                .unwrap()
                .item;

            match block.resolve_target(parser) {
                TargetResolution::Keep => Some(block),
                TargetResolution::Drop => None,
            }
        }

        fn parser_with_mode(mode: &str) -> Parser {
            Parser::default().with_intrinsic_attribute(
                "attribute-missing",
                mode,
                ModificationContext::Anywhere,
            )
        }

        #[test]
        fn target_without_reference_is_unchanged() {
            // The fast path (no `{`) returns the borrowed target verbatim.
            let mut p = Parser::default();
            let block = resolve("image::foo.png[]", &mut p).unwrap();
            assert_eq!(block.resolved_target(), "foo.png");
        }

        #[test]
        fn resolves_a_defined_reference() {
            let mut p = Parser::default().with_intrinsic_attribute(
                "name",
                "bar",
                ModificationContext::Anywhere,
            );
            let block = resolve("image::pre-{name}-post.png[]", &mut p).unwrap();
            assert_eq!(block.resolved_target(), "pre-bar-post.png");
        }

        #[test]
        fn skip_leaves_a_missing_reference_in_place() {
            // `skip` is the default `attribute-missing` mode.
            let mut p = Parser::default();
            let block = resolve("image::a{missing}b.png[]", &mut p).unwrap();
            assert_eq!(block.resolved_target(), "a{missing}b.png");
            assert!(p.take_substitution_warnings().is_empty());
        }

        #[test]
        fn drop_removes_only_the_missing_reference() {
            let mut p = parser_with_mode("drop");
            let block = resolve("image::a{missing}b.png[]", &mut p).unwrap();
            assert_eq!(block.resolved_target(), "ab.png");
        }

        #[test]
        fn warn_leaves_the_reference_and_records_a_warning() {
            let mut p = parser_with_mode("warn");
            let block = resolve("image::a{missing}b.png[]", &mut p).unwrap();
            assert_eq!(block.resolved_target(), "a{missing}b.png");

            let warnings = p.take_substitution_warnings();
            assert_eq!(warnings.len(), 1);
            assert_eq!(
                warnings[0].warning,
                WarningType::SkippingReferenceToMissingAttribute("missing".to_string())
            );
        }

        #[test]
        fn drop_line_drops_the_whole_block() {
            let mut p = parser_with_mode("drop-line");
            assert!(resolve("image::a{missing}b.png[]", &mut p).is_none());
        }

        #[test]
        fn drop_line_keeps_a_block_whose_reference_resolves() {
            let mut p = parser_with_mode("drop-line").with_intrinsic_attribute(
                "name",
                "bar",
                ModificationContext::Anywhere,
            );
            let block = resolve("image::{name}.png[]", &mut p).unwrap();
            assert_eq!(block.resolved_target(), "bar.png");
        }

        #[test]
        fn drop_line_drops_a_top_level_block_but_keeps_following_blocks() {
            // Exercises the drop at the document (non-list) level, which flows
            // through `parse_blocks_until`.
            let doc = Parser::default().parse(
                ":attribute-missing: drop-line\n\nimage::{unresolved}[]\n\nparagraph after\n",
            );

            assert_css(&doc, ".imageblock", 0);
            assert_css(&doc, ".paragraph", 1);
        }

        #[test]
        fn drop_line_drops_a_top_level_audio_block() {
            // Audio is a block macro too, so it honors `drop-line` just like an
            // image block.
            let doc = Parser::default().parse(
                ":attribute-missing: drop-line\n\naudio::{unresolved}[]\n\nparagraph after\n",
            );

            assert_css(&doc, ".audioblock", 0);
            assert_css(&doc, ".paragraph", 1);
        }

        #[test]
        fn escaped_missing_reference_never_drops_the_block() {
            // An escaped reference is not a missing reference, so even under
            // `drop-line` the block survives. (As elsewhere in the crate, the
            // escaping backslash is preserved verbatim by the attribute
            // substitution.)
            let mut p = parser_with_mode("drop-line");
            let block = resolve("image::a\\{missing}b.png[]", &mut p).unwrap();
            assert_eq!(block.resolved_target(), "a\\{missing}b.png");
            assert!(p.take_substitution_warnings().is_empty());
        }
    }

    mod media_type {
        mod impl_debug {
            use crate::blocks::MediaType;

            #[test]
            fn image() {
                let media_type = MediaType::Image;
                let debug_output = format!("{:?}", media_type);
                assert_eq!(debug_output, "MediaType::Image");
            }

            #[test]
            fn video() {
                let media_type = MediaType::Video;
                let debug_output = format!("{:?}", media_type);
                assert_eq!(debug_output, "MediaType::Video");
            }

            #[test]
            fn audio() {
                let media_type = MediaType::Audio;
                let debug_output = format!("{:?}", media_type);
                assert_eq!(debug_output, "MediaType::Audio");
            }
        }
    }
}
