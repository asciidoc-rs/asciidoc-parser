use crate::{
    HasSpan, Span,
    attributes::Attrlist,
    blocks::{Block, ChildBlocks, ContentModel, IsBlock},
    internal::debug::DebugSliceReference,
    strings::CowStr,
};

/// Content between the end of the document header and the first section title
/// in the document body is called the preamble.
#[derive(Clone, Eq, Hash, PartialEq)]
pub struct Preamble<'src> {
    blocks: Vec<Block<'src>>,
    source: Span<'src>,
}

impl<'src> Preamble<'src> {
    /// Returns a document-order iterator over this preamble's direct child
    /// blocks.
    ///
    /// For the full subtree, or to search from a [`Block`] or [`Document`], use
    /// [`FindBlocks`](crate::blocks::FindBlocks).
    ///
    /// [`Document`]: crate::Document
    pub fn child_blocks(&'src self) -> ChildBlocks<'src> {
        ChildBlocks::from_slice(&self.blocks)
    }

    pub(crate) fn from_blocks(blocks: Vec<Block<'src>>, source: Span<'src>) -> Self {
        let preamble_source = if let Some(last_block) = blocks.last() {
            let after_last = last_block.span().discard_all();
            source.trim_remainder(after_last)
        } else {
            // This clause is here as a fallback, but should not be reachable in practice. A
            // Preamble should only be constructed if there are content-bearing blocks
            // before the first section.
            source.trim_remainder(source)
        };

        Self {
            blocks,
            source: preamble_source,
        }
    }
}

impl<'src> IsBlock<'src> for Preamble<'src> {
    fn content_model(&self) -> ContentModel {
        ContentModel::Compound
    }

    fn raw_context(&self) -> CowStr<'src> {
        "preamble".into()
    }

    fn child_blocks_mut(&mut self) -> &mut [Block<'src>] {
        &mut self.blocks
    }

    fn title_source(&'src self) -> Option<Span<'src>> {
        None
    }

    fn title(&self) -> Option<&str> {
        None
    }

    fn anchor(&'src self) -> Option<Span<'src>> {
        None
    }

    fn anchor_reftext(&'src self) -> Option<Span<'src>> {
        None
    }

    fn attrlist(&'src self) -> Option<&'src Attrlist<'src>> {
        None
    }
}

impl<'src> HasSpan<'src> for Preamble<'src> {
    fn span(&self) -> Span<'src> {
        self.source
    }
}

impl std::fmt::Debug for Preamble<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Preamble")
            .field("blocks", &DebugSliceReference(&self.blocks))
            .field("source", &self.source)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use crate::{blocks::ContentModel, tests::prelude::*};

    fn doc_fixture() -> crate::Document<'static> {
        Parser::default().parse("= Document Title\n\nSome early words go here.\n\n== First Section")
    }

    fn fixture_preamble<'src>(
        doc: &'src crate::Document<'src>,
    ) -> &'src crate::blocks::Block<'src> {
        doc.child_blocks().next().unwrap()
    }

    #[test]
    fn impl_clone() {
        // Silly test to mark the #[derive(...)] line as covered.
        let doc = doc_fixture();

        let b1 = fixture_preamble(&doc);
        let b2 = b1.clone();

        assert_eq!(b1, &b2);
    }

    #[test]
    fn impl_debug() {
        let doc = doc_fixture();
        let preamble = fixture_preamble(&doc);

        let crate::blocks::Block::Preamble(preamble) = preamble else {
            panic!("Unexpected block: {preamble:#?}");
        };

        dbg!(&preamble);

        assert_eq!(
            format!("{preamble:#?}"),
            r#"Preamble {
    blocks: &[
        Block::Simple(
            SimpleBlock {
                content: Content {
                    original: Span {
                        data: "Some early words go here.",
                        line: 3,
                        col: 1,
                        offset: 18,
                    },
                    rendered: "Some early words go here.",
                },
                source: Span {
                    data: "Some early words go here.",
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
            },
        ),
    ],
    source: Span {
        data: "Some early words go here.",
        line: 3,
        col: 1,
        offset: 18,
    },
}"#
        );
    }

    #[test]
    fn impl_is_block() {
        let doc = doc_fixture();
        let preamble = fixture_preamble(&doc);

        assert_eq!(
            preamble,
            &Block::Preamble(Preamble {
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "Some early words go here.",
                            line: 3,
                            col: 1,
                            offset: 18,
                        },
                        rendered: "Some early words go here.",
                    },
                    source: Span {
                        data: "Some early words go here.",
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
                },),],
                source: Span {
                    data: "Some early words go here.",
                    line: 3,
                    col: 1,
                    offset: 18,
                },
            },)
        );

        assert_eq!(preamble.content_model(), ContentModel::Compound);
        assert!(preamble.rendered_content().is_none());
        assert_eq!(preamble.raw_context().as_ref(), "preamble");
        assert_eq!(preamble.resolved_context().as_ref(), "preamble");
        assert!(preamble.declared_style().is_none());

        let mut blocks = preamble.child_blocks();
        assert_eq!(
            blocks.next().unwrap(),
            &Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "Some early words go here.",
                        line: 3,
                        col: 1,
                        offset: 18,
                    },
                    rendered: "Some early words go here.",
                },
                source: Span {
                    data: "Some early words go here.",
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
            })
        );

        assert!(blocks.next().is_none());

        assert!(preamble.id().is_none());
        assert!(preamble.roles().is_empty());
        assert!(preamble.options().is_empty());
        assert!(preamble.title_source().is_none());
        assert!(preamble.title().is_none());
        assert!(preamble.anchor().is_none());
        assert!(preamble.anchor_reftext().is_none());
        assert!(preamble.attrlist().is_none());
        assert_eq!(preamble.substitution_group(), SubstitutionGroup::Normal);

        assert_eq!(
            format!("{preamble:#?}"),
            "Block::Preamble(\n    Preamble {\n        blocks: &[\n            Block::Simple(\n                SimpleBlock {\n                    content: Content {\n                        original: Span {\n                            data: \"Some early words go here.\",\n                            line: 3,\n                            col: 1,\n                            offset: 18,\n                        },\n                        rendered: \"Some early words go here.\",\n                    },\n                    source: Span {\n                        data: \"Some early words go here.\",\n                        line: 3,\n                        col: 1,\n                        offset: 18,\n                    },\n                    style: SimpleBlockStyle::Paragraph,\n                    title_source: None,\n                    title: None,\n                    caption: None,\n                    number: None,\n                    anchor: None,\n                    anchor_reftext: None,\n                    attrlist: None,\n                },\n            ),\n        ],\n        source: Span {\n            data: \"Some early words go here.\",\n            line: 3,\n            col: 1,\n            offset: 18,\n        },\n    },\n)"
        );

        assert_eq!(
            preamble.span(),
            Span {
                data: "Some early words go here.",
                line: 3,
                col: 1,
                offset: 18,
            }
        );
    }
}
