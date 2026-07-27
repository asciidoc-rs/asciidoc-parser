use crate::{
    HasSpan, Parser, Span,
    attributes::{Attrlist, AttrlistContext},
    blocks::{ChildBlocks, ContentModel, IsBlock, metadata::BlockMetadata},
    content::Content,
    span::MatchedItem,
    strings::CowStr,
    warnings::MatchAndWarnings,
};

/// A TOC block represents the `toc::[]` block macro, which marks the position
/// where a table of contents should be rendered when `toc-placement` is set to
/// `macro`.
///
/// The macro carries no target; its optional attribute list can override the
/// per-macro settings Asciidoctor honors on the generated TOC (`id`, `levels`,
/// and `role`), while a block title above the macro overrides `toc-title`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TocBlock<'src> {
    macro_attrlist: Attrlist<'src>,
    source: Span<'src>,
    title_source: Option<Span<'src>>,
    title: Option<Content<'src>>,
    anchor: Option<Span<'src>>,
    anchor_reftext: Option<Span<'src>>,
    attrlist: Option<Attrlist<'src>>,
}

impl<'src> TocBlock<'src> {
    /// Returns a document-order iterator over this block's direct child blocks.
    ///
    /// A TOC block never has child blocks, so this iterator is always empty.
    /// See [`FindBlocks`](crate::blocks::FindBlocks) to search from a
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

        let Some(name) = line.item.take_block_macro_name() else {
            return MatchAndWarnings {
                item: None,
                warnings: vec![],
            };
        };

        if name.item.data() != "toc" {
            return MatchAndWarnings {
                item: None,
                warnings: vec![],
            };
        }

        let Some(colons) = name.after.take_prefix("::") else {
            return MatchAndWarnings {
                item: None,
                warnings: vec![],
            };
        };

        // The `toc` block macro takes no target: the double colon must be
        // followed immediately by the attribute list. Anything else (e.g.
        // `toc::foo[]`) is not a TOC block macro, so quietly decline and let it
        // fall through to a paragraph.
        let Some(open_brace) = colons.after.take_prefix("[") else {
            return MatchAndWarnings {
                item: None,
                warnings: vec![],
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

    /// Return the macro's attribute list.
    ///
    /// **IMPORTANT:** This is the list of attributes _within_ the macro block
    /// definition itself (e.g. `toc::[levels=2]`), which Asciidoctor uses to
    /// override the per-macro TOC settings (`id`, `levels`, and `role`).
    ///
    /// See also [`attrlist()`] for attributes that can be defined before the
    /// macro invocation.
    ///
    /// [`attrlist()`]: Self::attrlist()
    pub fn macro_attrlist(&'src self) -> &'src Attrlist<'src> {
        &self.macro_attrlist
    }
}

impl<'src> IsBlock<'src> for TocBlock<'src> {
    fn content_model(&self) -> ContentModel {
        ContentModel::Empty
    }

    fn raw_context(&self) -> CowStr<'src> {
        "toc".into()
    }

    fn title_source(&'src self) -> Option<Span<'src>> {
        self.title_source
    }

    fn title(&self) -> Option<&str> {
        self.title.as_ref().map(Content::rendered_str)
    }

    fn id(&'src self) -> Option<&'src str> {
        // In addition to a block anchor (`[[id]]`/`[#id]`) or the block
        // attribute list above the macro, a TOC block may carry its ID as a
        // named `id=` attribute _inside_ the macro attribute list (e.g.
        // `toc::[id=contents]`), which the trait default does not consider.
        // Fall back to that last, so the block-level forms win.
        self.anchor()
            .map(|a| a.data())
            .or_else(|| self.attrlist().and_then(|attrlist| attrlist.id()))
            .or_else(|| self.macro_attrlist.id())
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

impl<'src> HasSpan<'src> for TocBlock<'src> {
    fn span(&self) -> Span<'src> {
        self.source
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::ops::Deref;

    use crate::{
        blocks::{ContentModel, metadata::BlockMetadata},
        tests::prelude::*,
    };

    #[test]
    fn impl_clone() {
        // Silly test to mark the #[derive(...)] line as covered.
        let mut parser = Parser::default();

        let b1 = crate::blocks::TocBlock::parse(&BlockMetadata::new("toc::[]"), &mut parser)
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
            crate::blocks::TocBlock::parse(&BlockMetadata::new(""), &mut parser)
                .unwrap_if_no_warnings()
                .is_none()
        );
    }

    #[test]
    fn err_not_toc_macro() {
        // A different macro name is not a TOC block.
        let mut parser = Parser::default();
        assert!(
            crate::blocks::TocBlock::parse(&BlockMetadata::new("image::foo.png[]"), &mut parser)
                .unwrap_if_no_warnings()
                .is_none()
        );
    }

    #[test]
    fn err_macro_name_not_word_char() {
        // A macro name must begin with a word character; a leading `#` is
        // rejected before any name is captured (see `take_block_macro_name`).
        let mut parser = Parser::default();
        assert!(
            crate::blocks::TocBlock::parse(&BlockMetadata::new("#toc::[]"), &mut parser)
                .unwrap_if_no_warnings()
                .is_none()
        );
    }

    #[test]
    fn err_macro_name_not_exactly_toc() {
        // A name that merely starts with `toc` (e.g. `tocx`) is a different
        // macro and is not recognized as a TOC block.
        let mut parser = Parser::default();
        assert!(
            crate::blocks::TocBlock::parse(&BlockMetadata::new("tocx::[]"), &mut parser)
                .unwrap_if_no_warnings()
                .is_none()
        );
    }

    #[test]
    fn err_not_closed() {
        let mut parser = Parser::default();
        assert!(
            crate::blocks::TocBlock::parse(&BlockMetadata::new("toc::["), &mut parser)
                .unwrap_if_no_warnings()
                .is_none()
        );
    }

    #[test]
    fn err_missing_double_colon() {
        // A single colon is an inline macro form, not a block macro.
        let mut parser = Parser::default();
        assert!(
            crate::blocks::TocBlock::parse(&BlockMetadata::new("toc:[]"), &mut parser)
                .unwrap_if_no_warnings()
                .is_none()
        );
    }

    #[test]
    fn err_has_target() {
        // The `toc` block macro takes no target.
        let mut parser = Parser::default();
        assert!(
            crate::blocks::TocBlock::parse(&BlockMetadata::new("toc::foo[]"), &mut parser)
                .unwrap_if_no_warnings()
                .is_none()
        );
    }

    #[test]
    fn simplest_toc_macro() {
        let mut parser = Parser::default();

        let mi = crate::blocks::TocBlock::parse(&BlockMetadata::new("toc::[]"), &mut parser)
            .unwrap_if_no_warnings()
            .unwrap();

        assert_eq!(
            mi.item,
            TocBlock {
                macro_attrlist: Attrlist {
                    attributes: &[],
                    anchor: None,
                    source: Span {
                        data: "",
                        line: 1,
                        col: 7,
                        offset: 6,
                    }
                },
                source: Span {
                    data: "toc::[]",
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
                col: 8,
                offset: 7
            }
        );

        assert_eq!(mi.item.content_model(), ContentModel::Empty);
        assert_eq!(mi.item.raw_context().deref(), "toc");
        assert_eq!(mi.item.resolved_context().deref(), "toc");
        assert!(mi.item.child_blocks().next().is_none());
        assert!(mi.item.title_source().is_none());
        assert!(mi.item.title().is_none());
        assert!(mi.item.anchor().is_none());
        assert!(mi.item.anchor_reftext().is_none());
        assert!(mi.item.attrlist().is_none());
        assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);
    }

    #[test]
    fn macro_attrlist_overrides() {
        // The macro attribute list carries the per-macro TOC overrides.
        let mut parser = Parser::default();

        let mi = crate::blocks::TocBlock::parse(
            &BlockMetadata::new("toc::[id=contents,levels=2,role=toc2]"),
            &mut parser,
        )
        .unwrap_if_no_warnings()
        .unwrap();

        let macro_attrlist = mi.item.macro_attrlist();
        assert_eq!(macro_attrlist.id().unwrap(), "contents");
        assert_eq!(
            macro_attrlist.named_attribute("levels").unwrap().value(),
            "2"
        );
        assert_eq!(
            macro_attrlist.named_attribute("role").unwrap().value(),
            "toc2"
        );

        // The macro `id=` is surfaced through the block's `id()`.
        assert_eq!(mi.item.id().unwrap(), "contents");
    }
}
