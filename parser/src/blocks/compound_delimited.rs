use std::slice::Iter;

use crate::{
    HasSpan, Parser, Span,
    attributes::Attrlist,
    blocks::{
        Block, ContentModel, IsBlock, metadata::BlockMetadata, parse_utils::parse_blocks_until,
    },
    internal::debug::DebugSliceReference,
    span::MatchedItem,
    strings::CowStr,
    warnings::{MatchAndWarnings, Warning, WarningType},
};

/// A delimited block that can contain other blocks.
///
/// The following delimiters are recognized as compound delimited blocks:
///
/// | Delimiter | Content type |
/// |-----------|--------------|
/// | `====`    | Example      |
/// | `--`      | Open         |
/// | `****`    | Sidebar      |
/// | `____`    | Quote        |
#[derive(Clone, Eq, PartialEq)]
pub struct CompoundDelimitedBlock<'src> {
    blocks: Vec<Block<'src>>,
    context: CowStr<'src>,
    source: Span<'src>,
    title_source: Option<Span<'src>>,
    title: Option<String>,
    anchor: Option<Span<'src>>,
    anchor_reftext: Option<Span<'src>>,
    attrlist: Option<Attrlist<'src>>,
}

impl<'src> CompoundDelimitedBlock<'src> {
    pub(crate) fn is_valid_delimiter(line: &Span<'src>) -> bool {
        let data = line.data();

        if data == "--" {
            return true;
        }

        // TO DO (https://github.com/asciidoc-rs/asciidoc-parser/issues/145):
        // Seek spec clarity: Do the characters after the fourth char
        // have to match the first four?

        if data.len() >= 4 {
            if data.starts_with("====") {
                data.split_at(4).1.chars().all(|c| c == '=')
            } else if data.starts_with("****") {
                data.split_at(4).1.chars().all(|c| c == '*')
            } else if data.starts_with("____") {
                data.split_at(4).1.chars().all(|c| c == '_')
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
        let maybe_delimiter_text = delimiter.item.data();

        // TO DO (https://github.com/asciidoc-rs/asciidoc-parser/issues/146):
        // Seek spec clarity on whether three hyphens can be used to
        // delimit an open block. Assuming yes for now.
        let context = match maybe_delimiter_text
            .split_at_checked(maybe_delimiter_text.len().min(4))?
            .0
        {
            "====" => "example",
            "--" => "open",
            "****" => "sidebar",
            "____" => "quote",
            _ => return None,
        };

        if !Self::is_valid_delimiter(&delimiter.item) {
            return None;
        }

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

        let maw_blocks = parse_blocks_until(inside_delimiters, |_| false, parser);

        let blocks = maw_blocks.item;
        let source = metadata
            .source
            .trim_remainder(closing_delimiter.discard_all());

        Some(MatchAndWarnings {
            item: Some(MatchedItem {
                item: Self {
                    blocks: blocks.item,
                    context: context.into(),
                    source: source.trim_trailing_whitespace(),
                    title_source: metadata.title_source,
                    title: metadata.title.clone(),
                    anchor: metadata.anchor,
                    anchor_reftext: metadata.anchor_reftext,
                    attrlist: metadata.attrlist.clone(),
                },
                after,
            }),
            warnings: if closing_delimiter.is_empty() {
                let mut warnings = maw_blocks.warnings;
                warnings.insert(
                    0,
                    Warning {
                        source: delimiter.item,
                        warning: WarningType::UnterminatedDelimitedBlock,
                    },
                );
                warnings
            } else {
                maw_blocks.warnings
            },
        })
    }
}

impl<'src> IsBlock<'src> for CompoundDelimitedBlock<'src> {
    fn content_model(&self) -> ContentModel {
        ContentModel::Compound
    }

    fn raw_context(&self) -> CowStr<'src> {
        self.context.clone()
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
}

impl<'src> HasSpan<'src> for CompoundDelimitedBlock<'src> {
    fn span(&self) -> Span<'src> {
        self.source
    }
}

impl std::fmt::Debug for CompoundDelimitedBlock<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompoundDelimitedBlock")
            .field("blocks", &DebugSliceReference(&self.blocks))
            .field("context", &self.context)
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

    use crate::{blocks::metadata::BlockMetadata, tests::prelude::*};

    mod is_valid_delimiter {
        use crate::blocks::CompoundDelimitedBlock;

        #[test]
        fn comment() {
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("////")
            ));
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("/////")
            ));
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("/////////")
            ));

            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("///")
            ));
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("//-/")
            ));
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("////-")
            ));
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("//////////x")
            ));
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("//😀/")
            ));
        }

        #[test]
        fn example() {
            assert!(CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("====")
            ));
            assert!(CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("=====")
            ));
            assert!(CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("=======")
            ));

            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("===")
            ));
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("==-=")
            ));
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("====-")
            ));
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("==========x")
            ));
        }

        #[test]
        fn listing() {
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("----")
            ));
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("-----")
            ));
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("---------")
            ));
        }

        #[test]
        fn literal() {
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("....")
            ));
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new(".....")
            ));
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new(".........")
            ));
        }

        #[test]
        fn sidebar() {
            assert!(CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("****")
            ));
            assert!(CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("*****")
            ));
            assert!(CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("*********")
            ));

            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("***")
            ));
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("**-*")
            ));
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("****-")
            ));
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("**********x")
            ));
        }

        #[test]
        fn table() {
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("|===")
            ));
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new(",===")
            ));
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new(":===")
            ));
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("!===")
            ));
        }

        #[test]
        fn pass() {
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("++++")
            ));
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("+++++")
            ));
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("+++++++++")
            ));
        }

        #[test]
        fn quote() {
            assert!(CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("____")
            ));
            assert!(CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("_____")
            ));
            assert!(CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("_________")
            ));

            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("___")
            ));
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("__-_")
            ));
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("____-")
            ));
            assert!(!CompoundDelimitedBlock::is_valid_delimiter(
                &crate::Span::new("_________x")
            ));
        }
    }

    mod parse {
        use crate::{
            blocks::{SimpleBlockStyle, metadata::BlockMetadata},
            tests::prelude::*,
            warnings::WarningType,
        };

        #[test]
        fn err_invalid_delimiter() {
            let mut parser = Parser::default();
            assert!(
                crate::blocks::CompoundDelimitedBlock::parse(&BlockMetadata::new(""), &mut parser)
                    .is_none()
            );

            let mut parser = Parser::default();
            assert!(
                crate::blocks::CompoundDelimitedBlock::parse(
                    &BlockMetadata::new("///"),
                    &mut parser
                )
                .is_none()
            );

            let mut parser = Parser::default();
            assert!(
                crate::blocks::CompoundDelimitedBlock::parse(
                    &BlockMetadata::new("////x"),
                    &mut parser
                )
                .is_none()
            );

            let mut parser = Parser::default();
            assert!(
                crate::blocks::CompoundDelimitedBlock::parse(
                    &BlockMetadata::new("--x"),
                    &mut parser
                )
                .is_none()
            );

            let mut parser = Parser::default();
            assert!(
                crate::blocks::CompoundDelimitedBlock::parse(
                    &BlockMetadata::new("****x"),
                    &mut parser
                )
                .is_none()
            );

            let mut parser = Parser::default();
            assert!(
                crate::blocks::CompoundDelimitedBlock::parse(
                    &BlockMetadata::new("__\n__"),
                    &mut parser
                )
                .is_none()
            );
        }

        #[test]
        fn err_unterminated() {
            let mut parser = Parser::default();

            let maw = crate::blocks::CompoundDelimitedBlock::parse(
                &BlockMetadata::new("====\nblah blah blah"),
                &mut parser,
            )
            .unwrap();

            assert_eq!(
                maw.item.unwrap().item,
                CompoundDelimitedBlock {
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "blah blah blah",
                                line: 2,
                                col: 1,
                                offset: 5,
                            },
                            rendered: "blah blah blah",
                        },
                        source: Span {
                            data: "blah blah blah",
                            line: 2,
                            col: 1,
                            offset: 5,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    context: "example",
                    source: Span {
                        data: "====\nblah blah blah",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },
            );

            assert_eq!(
                maw.warnings,
                vec![Warning {
                    source: Span {
                        data: "====",
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
        use crate::{blocks::metadata::BlockMetadata, tests::prelude::*};

        #[test]
        fn empty() {
            let mut parser = Parser::default();
            assert!(
                crate::blocks::CompoundDelimitedBlock::parse(
                    &BlockMetadata::new("////\n////"),
                    &mut parser
                )
                .is_none()
            );
        }

        #[test]
        fn multiple_lines() {
            let mut parser = Parser::default();
            assert!(
                crate::blocks::CompoundDelimitedBlock::parse(
                    &BlockMetadata::new("////\nline1  \nline2\n////"),
                    &mut parser
                )
                .is_none()
            );
        }
    }

    mod example {
        use crate::{
            blocks::{ContentModel, IsBlock, SimpleBlockStyle, metadata::BlockMetadata},
            content::SubstitutionGroup,
            tests::prelude::*,
        };

        #[test]
        fn empty() {
            let mut parser = Parser::default();

            let maw = crate::blocks::CompoundDelimitedBlock::parse(
                &BlockMetadata::new("====\n===="),
                &mut parser,
            )
            .unwrap();

            let mi = maw.item.unwrap().clone();

            assert_eq!(
                mi.item,
                CompoundDelimitedBlock {
                    blocks: &[],
                    context: "example",
                    source: Span {
                        data: "====\n====",
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

            assert_eq!(mi.item.content_model(), ContentModel::Compound);
            assert!(mi.item.rendered_content().is_none());
            assert_eq!(mi.item.raw_context().as_ref(), "example");
            assert_eq!(mi.item.resolved_context().as_ref(), "example");
            assert!(mi.item.declared_style().is_none());
            assert!(mi.item.nested_blocks().next().is_none());
            assert!(mi.item.id().is_none());
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);
        }

        #[test]
        fn multiple_blocks() {
            let mut parser = Parser::default();

            let maw = crate::blocks::CompoundDelimitedBlock::parse(
                &BlockMetadata::new("====\nblock1\n\nblock2\n===="),
                &mut parser,
            )
            .unwrap();

            let mi = maw.item.unwrap().clone();

            assert_eq!(
                mi.item,
                CompoundDelimitedBlock {
                    blocks: &[
                        Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "block1",
                                    line: 2,
                                    col: 1,
                                    offset: 5,
                                },
                                rendered: "block1",
                            },
                            source: Span {
                                data: "block1",
                                line: 2,
                                col: 1,
                                offset: 5,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "block2",
                                    line: 4,
                                    col: 1,
                                    offset: 13,
                                },
                                rendered: "block2",
                            },
                            source: Span {
                                data: "block2",
                                line: 4,
                                col: 1,
                                offset: 13,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                    ],
                    context: "example",
                    source: Span {
                        data: "====\nblock1\n\nblock2\n====",
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

            assert_eq!(mi.item.content_model(), ContentModel::Compound);
            assert_eq!(mi.item.raw_context().as_ref(), "example");
            assert_eq!(mi.item.resolved_context().as_ref(), "example");
            assert!(mi.item.declared_style().is_none());
            assert!(mi.item.id().is_none());
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

            let mut blocks = mi.item.nested_blocks();
            assert_eq!(
                blocks.next().unwrap(),
                &Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "block1",
                            line: 2,
                            col: 1,
                            offset: 5,
                        },
                        rendered: "block1",
                    },
                    source: Span {
                        data: "block1",
                        line: 2,
                        col: 1,
                        offset: 5,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },)
            );

            assert_eq!(
                blocks.next().unwrap(),
                &Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "block2",
                            line: 4,
                            col: 1,
                            offset: 13,
                        },
                        rendered: "block2",
                    },
                    source: Span {
                        data: "block2",
                        line: 4,
                        col: 1,
                        offset: 13,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },)
            );

            assert!(blocks.next().is_none());
        }

        #[test]
        fn nested_blocks() {
            let mut parser = Parser::default();

            let maw = crate::blocks::CompoundDelimitedBlock::parse(
                &BlockMetadata::new("====\nblock1\n\n=====\nblock2\n=====\n===="),
                &mut parser,
            )
            .unwrap();

            let mi = maw.item.unwrap().clone();

            assert_eq!(
                mi.item,
                CompoundDelimitedBlock {
                    blocks: &[
                        Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "block1",
                                    line: 2,
                                    col: 1,
                                    offset: 5,
                                },
                                rendered: "block1",
                            },
                            source: Span {
                                data: "block1",
                                line: 2,
                                col: 1,
                                offset: 5,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::CompoundDelimited(CompoundDelimitedBlock {
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "block2",
                                        line: 5,
                                        col: 1,
                                        offset: 19,
                                    },
                                    rendered: "block2",
                                },
                                source: Span {
                                    data: "block2",
                                    line: 5,
                                    col: 1,
                                    offset: 19,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            context: "example",
                            source: Span {
                                data: "=====\nblock2\n=====",
                                line: 4,
                                col: 1,
                                offset: 13,
                            },
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        })
                    ],
                    context: "example",
                    source: Span {
                        data: "====\nblock1\n\n=====\nblock2\n=====\n====",
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

            assert_eq!(mi.item.content_model(), ContentModel::Compound);
            assert_eq!(mi.item.raw_context().as_ref(), "example");
            assert_eq!(mi.item.resolved_context().as_ref(), "example");
            assert!(mi.item.declared_style().is_none());
            assert!(mi.item.id().is_none());
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

            let mut blocks = mi.item.nested_blocks();
            assert_eq!(
                blocks.next().unwrap(),
                &Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "block1",
                            line: 2,
                            col: 1,
                            offset: 5,
                        },
                        rendered: "block1",
                    },
                    source: Span {
                        data: "block1",
                        line: 2,
                        col: 1,
                        offset: 5,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },)
            );

            assert_eq!(
                blocks.next().unwrap(),
                &Block::CompoundDelimited(CompoundDelimitedBlock {
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "block2",
                                line: 5,
                                col: 1,
                                offset: 19,
                            },
                            rendered: "block2",
                        },
                        source: Span {
                            data: "block2",
                            line: 5,
                            col: 1,
                            offset: 19,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    context: "example",
                    source: Span {
                        data: "=====\nblock2\n=====",
                        line: 4,
                        col: 1,
                        offset: 13,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                })
            );

            assert!(blocks.next().is_none());
        }
        #[test]
        fn no_panic_for_utf8_code_point_using_more_than_one_byte() {
            let mut parser = Parser::default();
            assert!(
                crate::blocks::CompoundDelimitedBlock::parse(
                    &BlockMetadata::new("===😀"),
                    &mut parser
                )
                .is_none()
            );
        }
    }

    mod listing {
        use crate::{blocks::metadata::BlockMetadata, tests::prelude::*};

        #[test]
        fn empty() {
            let mut parser = Parser::default();
            assert!(
                crate::blocks::CompoundDelimitedBlock::parse(
                    &BlockMetadata::new("----\n----"),
                    &mut parser
                )
                .is_none()
            );
        }

        #[test]
        fn multiple_lines() {
            let mut parser = Parser::default();
            assert!(
                crate::blocks::CompoundDelimitedBlock::parse(
                    &BlockMetadata::new("----\nline1  \nline2\n----"),
                    &mut parser
                )
                .is_none()
            );
        }
    }

    mod literal {
        use crate::{blocks::metadata::BlockMetadata, tests::prelude::*};

        #[test]
        fn empty() {
            let mut parser = Parser::default();
            assert!(
                crate::blocks::CompoundDelimitedBlock::parse(
                    &BlockMetadata::new("....\n...."),
                    &mut parser
                )
                .is_none()
            );
        }

        #[test]
        fn multiple_lines() {
            let mut parser = Parser::default();
            assert!(
                crate::blocks::CompoundDelimitedBlock::parse(
                    &BlockMetadata::new("....\nline1  \nline2\n...."),
                    &mut parser
                )
                .is_none()
            );
        }
    }

    mod open {
        use crate::{
            blocks::{BreakType, ContentModel, IsBlock, SimpleBlockStyle, metadata::BlockMetadata},
            content::SubstitutionGroup,
            tests::prelude::*,
        };

        #[test]
        fn empty() {
            let mut parser = Parser::default();

            let maw = crate::blocks::CompoundDelimitedBlock::parse(
                &BlockMetadata::new("--\n--"),
                &mut parser,
            )
            .unwrap();

            let mi = maw.item.unwrap().clone();

            assert_eq!(
                mi.item,
                CompoundDelimitedBlock {
                    blocks: &[],
                    context: "open",
                    source: Span {
                        data: "--\n--",
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

            assert_eq!(mi.item.content_model(), ContentModel::Compound);
            assert_eq!(mi.item.raw_context().as_ref(), "open");
            assert_eq!(mi.item.resolved_context().as_ref(), "open");
            assert!(mi.item.declared_style().is_none());
            assert!(mi.item.nested_blocks().next().is_none());
            assert!(mi.item.id().is_none());
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);
        }

        #[test]
        fn multiple_blocks() {
            let mut parser = Parser::default();

            let maw = crate::blocks::CompoundDelimitedBlock::parse(
                &BlockMetadata::new("--\nblock1\n\nblock2\n--"),
                &mut parser,
            )
            .unwrap();

            let mi = maw.item.unwrap().clone();

            assert_eq!(
                mi.item,
                CompoundDelimitedBlock {
                    blocks: &[
                        Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "block1",
                                    line: 2,
                                    col: 1,
                                    offset: 3,
                                },
                                rendered: "block1",
                            },
                            source: Span {
                                data: "block1",
                                line: 2,
                                col: 1,
                                offset: 3,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "block2",
                                    line: 4,
                                    col: 1,
                                    offset: 11,
                                },
                                rendered: "block2",
                            },
                            source: Span {
                                data: "block2",
                                line: 4,
                                col: 1,
                                offset: 11,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                    ],
                    context: "open",
                    source: Span {
                        data: "--\nblock1\n\nblock2\n--",
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

            assert_eq!(mi.item.content_model(), ContentModel::Compound);
            assert_eq!(mi.item.raw_context().as_ref(), "open");
            assert_eq!(mi.item.resolved_context().as_ref(), "open");
            assert!(mi.item.declared_style().is_none());
            assert!(mi.item.id().is_none());
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

            let mut blocks = mi.item.nested_blocks();
            assert_eq!(
                blocks.next().unwrap(),
                &Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "block1",
                            line: 2,
                            col: 1,
                            offset: 3,
                        },
                        rendered: "block1",
                    },
                    source: Span {
                        data: "block1",
                        line: 2,
                        col: 1,
                        offset: 3,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },)
            );

            assert_eq!(
                blocks.next().unwrap(),
                &Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "block2",
                            line: 4,
                            col: 1,
                            offset: 11,
                        },
                        rendered: "block2",
                    },
                    source: Span {
                        data: "block2",
                        line: 4,
                        col: 1,
                        offset: 11,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },)
            );

            assert!(blocks.next().is_none());
        }

        #[test]
        fn nested_blocks() {
            // Spec says three hyphens does NOT mark an open block.
            let mut parser = Parser::default();

            let maw = crate::blocks::CompoundDelimitedBlock::parse(
                &BlockMetadata::new("--\nblock1\n\n---\nblock2\n---\n--"),
                &mut parser,
            )
            .unwrap();

            let mi = maw.item.unwrap().clone();

            assert_eq!(
                mi.item,
                CompoundDelimitedBlock {
                    blocks: &[
                        Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "block1",
                                    line: 2,
                                    col: 1,
                                    offset: 3,
                                },
                                rendered: "block1",
                            },
                            source: Span {
                                data: "block1",
                                line: 2,
                                col: 1,
                                offset: 3,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::Break(Break {
                            type_: BreakType::Thematic,
                            source: Span {
                                data: "---",
                                line: 4,
                                col: 1,
                                offset: 11,
                            },
                            title_source: None,
                            title: None,
                            anchor: None,
                            attrlist: None,
                        },),
                        Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "block2\n---",
                                    line: 5,
                                    col: 1,
                                    offset: 15,
                                },
                                rendered: "block2\n---",
                            },
                            source: Span {
                                data: "block2\n---",
                                line: 5,
                                col: 1,
                                offset: 15,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                    ],
                    context: "open",
                    source: Span {
                        data: "--\nblock1\n\n---\nblock2\n---\n--",
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

            assert_eq!(mi.item.content_model(), ContentModel::Compound);
            assert_eq!(mi.item.raw_context().as_ref(), "open");
            assert_eq!(mi.item.resolved_context().as_ref(), "open");
            assert!(mi.item.declared_style().is_none());
            assert!(mi.item.id().is_none());
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

            let mut blocks = mi.item.nested_blocks();
            assert_eq!(
                blocks.next().unwrap(),
                &Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "block1",
                            line: 2,
                            col: 1,
                            offset: 3,
                        },
                        rendered: "block1",
                    },
                    source: Span {
                        data: "block1",
                        line: 2,
                        col: 1,
                        offset: 3,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },)
            );

            assert_eq!(
                blocks.next().unwrap(),
                &Block::Break(Break {
                    type_: BreakType::Thematic,
                    source: Span {
                        data: "---",
                        line: 4,
                        col: 1,
                        offset: 11,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    attrlist: None,
                },)
            );

            assert_eq!(
                blocks.next().unwrap(),
                &Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "block2\n---",
                            line: 5,
                            col: 1,
                            offset: 15,
                        },
                        rendered: "block2\n---",
                    },
                    source: Span {
                        data: "block2\n---",
                        line: 5,
                        col: 1,
                        offset: 15,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },)
            );

            assert!(blocks.next().is_none());
        }
    }

    mod sidebar {
        use crate::{
            blocks::{ContentModel, IsBlock, SimpleBlockStyle, metadata::BlockMetadata},
            content::SubstitutionGroup,
            tests::prelude::*,
        };

        #[test]
        fn empty() {
            let mut parser = Parser::default();

            let maw = crate::blocks::CompoundDelimitedBlock::parse(
                &BlockMetadata::new("****\n****"),
                &mut parser,
            )
            .unwrap();

            let mi = maw.item.unwrap().clone();

            assert_eq!(
                mi.item,
                CompoundDelimitedBlock {
                    blocks: &[],
                    context: "sidebar",
                    source: Span {
                        data: "****\n****",
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

            assert_eq!(mi.item.content_model(), ContentModel::Compound);
            assert_eq!(mi.item.raw_context().as_ref(), "sidebar");
            assert_eq!(mi.item.resolved_context().as_ref(), "sidebar");
            assert!(mi.item.declared_style().is_none());
            assert!(mi.item.nested_blocks().next().is_none());
            assert!(mi.item.id().is_none());
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);
        }

        #[test]
        fn multiple_blocks() {
            let mut parser = Parser::default();

            let maw = crate::blocks::CompoundDelimitedBlock::parse(
                &BlockMetadata::new("****\nblock1\n\nblock2\n****"),
                &mut parser,
            )
            .unwrap();

            let mi = maw.item.unwrap().clone();

            assert_eq!(
                mi.item,
                CompoundDelimitedBlock {
                    blocks: &[
                        Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "block1",
                                    line: 2,
                                    col: 1,
                                    offset: 5,
                                },
                                rendered: "block1",
                            },
                            source: Span {
                                data: "block1",
                                line: 2,
                                col: 1,
                                offset: 5,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "block2",
                                    line: 4,
                                    col: 1,
                                    offset: 13,
                                },
                                rendered: "block2",
                            },
                            source: Span {
                                data: "block2",
                                line: 4,
                                col: 1,
                                offset: 13,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                    ],
                    context: "sidebar",
                    source: Span {
                        data: "****\nblock1\n\nblock2\n****",
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

            assert_eq!(mi.item.content_model(), ContentModel::Compound);
            assert_eq!(mi.item.raw_context().as_ref(), "sidebar");
            assert_eq!(mi.item.resolved_context().as_ref(), "sidebar");
            assert!(mi.item.declared_style().is_none());
            assert!(mi.item.id().is_none());
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

            let mut blocks = mi.item.nested_blocks();
            assert_eq!(
                blocks.next().unwrap(),
                &Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "block1",
                            line: 2,
                            col: 1,
                            offset: 5,
                        },
                        rendered: "block1",
                    },
                    source: Span {
                        data: "block1",
                        line: 2,
                        col: 1,
                        offset: 5,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },)
            );

            assert_eq!(
                blocks.next().unwrap(),
                &Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "block2",
                            line: 4,
                            col: 1,
                            offset: 13,
                        },
                        rendered: "block2",
                    },
                    source: Span {
                        data: "block2",
                        line: 4,
                        col: 1,
                        offset: 13,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },)
            );

            assert!(blocks.next().is_none());
        }

        #[test]
        fn nested_blocks() {
            let mut parser = Parser::default();

            let maw = crate::blocks::CompoundDelimitedBlock::parse(
                &BlockMetadata::new("****\nblock1\n\n*****\nblock2\n*****\n****"),
                &mut parser,
            )
            .unwrap();

            let mi = maw.item.unwrap().clone();

            assert_eq!(
                mi.item,
                CompoundDelimitedBlock {
                    blocks: &[
                        Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "block1",
                                    line: 2,
                                    col: 1,
                                    offset: 5,
                                },
                                rendered: "block1",
                            },
                            source: Span {
                                data: "block1",
                                line: 2,
                                col: 1,
                                offset: 5,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::CompoundDelimited(CompoundDelimitedBlock {
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "block2",
                                        line: 5,
                                        col: 1,
                                        offset: 19,
                                    },
                                    rendered: "block2",
                                },
                                source: Span {
                                    data: "block2",
                                    line: 5,
                                    col: 1,
                                    offset: 19,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            context: "sidebar",
                            source: Span {
                                data: "*****\nblock2\n*****",
                                line: 4,
                                col: 1,
                                offset: 13,
                            },
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        })
                    ],
                    context: "sidebar",
                    source: Span {
                        data: "****\nblock1\n\n*****\nblock2\n*****\n****",
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

            assert_eq!(mi.item.content_model(), ContentModel::Compound);
            assert_eq!(mi.item.raw_context().as_ref(), "sidebar");
            assert_eq!(mi.item.resolved_context().as_ref(), "sidebar");
            assert!(mi.item.declared_style().is_none());
            assert!(mi.item.id().is_none());
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

            let mut blocks = mi.item.nested_blocks();
            assert_eq!(
                blocks.next().unwrap(),
                &Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "block1",
                            line: 2,
                            col: 1,
                            offset: 5,
                        },
                        rendered: "block1",
                    },
                    source: Span {
                        data: "block1",
                        line: 2,
                        col: 1,
                        offset: 5,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },)
            );

            assert_eq!(
                blocks.next().unwrap(),
                &Block::CompoundDelimited(CompoundDelimitedBlock {
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "block2",
                                line: 5,
                                col: 1,
                                offset: 19,
                            },
                            rendered: "block2",
                        },
                        source: Span {
                            data: "block2",
                            line: 5,
                            col: 1,
                            offset: 19,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    context: "sidebar",
                    source: Span {
                        data: "*****\nblock2\n*****",
                        line: 4,
                        col: 1,
                        offset: 13,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                })
            );

            assert!(blocks.next().is_none());
        }
    }

    mod table {
        use crate::{blocks::metadata::BlockMetadata, tests::prelude::*};

        #[test]
        fn empty() {
            let mut parser = Parser::default();
            assert!(
                crate::blocks::CompoundDelimitedBlock::parse(
                    &BlockMetadata::new("|===\n|==="),
                    &mut parser
                )
                .is_none()
            );

            let mut parser = Parser::default();
            assert!(
                crate::blocks::CompoundDelimitedBlock::parse(
                    &BlockMetadata::new(",===\n,==="),
                    &mut parser
                )
                .is_none()
            );

            let mut parser = Parser::default();
            assert!(
                crate::blocks::CompoundDelimitedBlock::parse(
                    &BlockMetadata::new(":===\n:==="),
                    &mut parser
                )
                .is_none()
            );

            let mut parser = Parser::default();
            assert!(
                crate::blocks::CompoundDelimitedBlock::parse(
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
                crate::blocks::CompoundDelimitedBlock::parse(
                    &BlockMetadata::new("|===\nline1  \nline2\n|==="),
                    &mut parser
                )
                .is_none()
            );

            let mut parser = Parser::default();
            assert!(
                crate::blocks::CompoundDelimitedBlock::parse(
                    &BlockMetadata::new(",===\nline1  \nline2\n,==="),
                    &mut parser
                )
                .is_none()
            );

            let mut parser = Parser::default();
            assert!(
                crate::blocks::CompoundDelimitedBlock::parse(
                    &BlockMetadata::new(":===\nline1  \nline2\n:==="),
                    &mut parser
                )
                .is_none()
            );

            let mut parser = Parser::default();
            assert!(
                crate::blocks::CompoundDelimitedBlock::parse(
                    &BlockMetadata::new("!===\nline1  \nline2\n!==="),
                    &mut parser
                )
                .is_none()
            );
        }
    }

    mod pass {
        use crate::{blocks::metadata::BlockMetadata, tests::prelude::*};

        #[test]
        fn empty() {
            let mut parser = Parser::default();
            assert!(
                crate::blocks::CompoundDelimitedBlock::parse(
                    &BlockMetadata::new("++++\n++++"),
                    &mut parser
                )
                .is_none()
            );
        }

        #[test]
        fn multiple_lines() {
            let mut parser = Parser::default();
            assert!(
                crate::blocks::CompoundDelimitedBlock::parse(
                    &BlockMetadata::new("++++\nline1  \nline2\n++++"),
                    &mut parser
                )
                .is_none()
            );
        }
    }

    mod quote {
        use crate::{
            blocks::{ContentModel, IsBlock, SimpleBlockStyle, metadata::BlockMetadata},
            content::SubstitutionGroup,
            tests::prelude::*,
        };

        #[test]
        fn empty() {
            let mut parser = Parser::default();

            let maw = crate::blocks::CompoundDelimitedBlock::parse(
                &BlockMetadata::new("____\n____"),
                &mut parser,
            )
            .unwrap();

            let mi = maw.item.unwrap().clone();

            assert_eq!(
                mi.item,
                CompoundDelimitedBlock {
                    blocks: &[],
                    context: "quote",
                    source: Span {
                        data: "____\n____",
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

            assert_eq!(mi.item.content_model(), ContentModel::Compound);
            assert_eq!(mi.item.raw_context().as_ref(), "quote");
            assert_eq!(mi.item.resolved_context().as_ref(), "quote");
            assert!(mi.item.declared_style().is_none());
            assert!(mi.item.nested_blocks().next().is_none());
            assert!(mi.item.id().is_none());
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);
        }

        #[test]
        fn multiple_blocks() {
            let mut parser = Parser::default();

            let maw = crate::blocks::CompoundDelimitedBlock::parse(
                &BlockMetadata::new("____\nblock1\n\nblock2\n____"),
                &mut parser,
            )
            .unwrap();

            let mi = maw.item.unwrap().clone();

            assert_eq!(
                mi.item,
                CompoundDelimitedBlock {
                    blocks: &[
                        Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "block1",
                                    line: 2,
                                    col: 1,
                                    offset: 5,
                                },
                                rendered: "block1",
                            },
                            source: Span {
                                data: "block1",
                                line: 2,
                                col: 1,
                                offset: 5,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "block2",
                                    line: 4,
                                    col: 1,
                                    offset: 13,
                                },
                                rendered: "block2",
                            },
                            source: Span {
                                data: "block2",
                                line: 4,
                                col: 1,
                                offset: 13,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                    ],
                    context: "quote",
                    source: Span {
                        data: "____\nblock1\n\nblock2\n____",
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

            assert_eq!(mi.item.content_model(), ContentModel::Compound);
            assert_eq!(mi.item.raw_context().as_ref(), "quote");
            assert_eq!(mi.item.resolved_context().as_ref(), "quote");
            assert!(mi.item.declared_style().is_none());
            assert!(mi.item.id().is_none());
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

            let mut blocks = mi.item.nested_blocks();
            assert_eq!(
                blocks.next().unwrap(),
                &Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "block1",
                            line: 2,
                            col: 1,
                            offset: 5,
                        },
                        rendered: "block1",
                    },
                    source: Span {
                        data: "block1",
                        line: 2,
                        col: 1,
                        offset: 5,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },)
            );

            assert_eq!(
                blocks.next().unwrap(),
                &Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "block2",
                            line: 4,
                            col: 1,
                            offset: 13,
                        },
                        rendered: "block2",
                    },
                    source: Span {
                        data: "block2",
                        line: 4,
                        col: 1,
                        offset: 13,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },)
            );

            assert!(blocks.next().is_none());
        }

        #[test]
        fn nested_blocks() {
            let mut parser = Parser::default();

            let maw = crate::blocks::CompoundDelimitedBlock::parse(
                &BlockMetadata::new("____\nblock1\n\n_____\nblock2\n_____\n____"),
                &mut parser,
            )
            .unwrap();

            let mi = maw.item.unwrap().clone();

            assert_eq!(
                mi.item,
                CompoundDelimitedBlock {
                    blocks: &[
                        Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "block1",
                                    line: 2,
                                    col: 1,
                                    offset: 5,
                                },
                                rendered: "block1",
                            },
                            source: Span {
                                data: "block1",
                                line: 2,
                                col: 1,
                                offset: 5,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),
                        Block::CompoundDelimited(CompoundDelimitedBlock {
                            blocks: &[Block::Simple(SimpleBlock {
                                content: Content {
                                    original: Span {
                                        data: "block2",
                                        line: 5,
                                        col: 1,
                                        offset: 19,
                                    },
                                    rendered: "block2",
                                },
                                source: Span {
                                    data: "block2",
                                    line: 5,
                                    col: 1,
                                    offset: 19,
                                },
                                style: SimpleBlockStyle::Paragraph,
                                title_source: None,
                                title: None,
                                anchor: None,
                                anchor_reftext: None,
                                attrlist: None,
                            },),],
                            context: "quote",
                            source: Span {
                                data: "_____\nblock2\n_____",
                                line: 4,
                                col: 1,
                                offset: 13,
                            },
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        })
                    ],
                    context: "quote",
                    source: Span {
                        data: "____\nblock1\n\n_____\nblock2\n_____\n____",
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

            assert_eq!(mi.item.content_model(), ContentModel::Compound);
            assert_eq!(mi.item.raw_context().as_ref(), "quote");
            assert_eq!(mi.item.resolved_context().as_ref(), "quote");
            assert!(mi.item.declared_style().is_none());
            assert!(mi.item.id().is_none());
            assert!(mi.item.roles().is_empty());
            assert!(mi.item.options().is_empty());
            assert!(mi.item.title_source().is_none());
            assert!(mi.item.title().is_none());
            assert!(mi.item.anchor().is_none());
            assert!(mi.item.anchor_reftext().is_none());
            assert!(mi.item.attrlist().is_none());
            assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);

            let mut blocks = mi.item.nested_blocks();
            assert_eq!(
                blocks.next().unwrap(),
                &Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "block1",
                            line: 2,
                            col: 1,
                            offset: 5,
                        },
                        rendered: "block1",
                    },
                    source: Span {
                        data: "block1",
                        line: 2,
                        col: 1,
                        offset: 5,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                },)
            );

            assert_eq!(
                blocks.next().unwrap(),
                &Block::CompoundDelimited(CompoundDelimitedBlock {
                    blocks: &[Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "block2",
                                line: 5,
                                col: 1,
                                offset: 19,
                            },
                            rendered: "block2",
                        },
                        source: Span {
                            data: "block2",
                            line: 5,
                            col: 1,
                            offset: 19,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),],
                    context: "quote",
                    source: Span {
                        data: "_____\nblock2\n_____",
                        line: 4,
                        col: 1,
                        offset: 13,
                    },
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                })
            );

            assert!(blocks.next().is_none());
        }
    }

    #[test]
    fn impl_debug() {
        let mut parser = Parser::default();

        let cdb = crate::blocks::CompoundDelimitedBlock::parse(
            &BlockMetadata::new("====\nblock1\n\nblock2\n===="),
            &mut parser,
        )
        .unwrap()
        .unwrap_if_no_warnings()
        .unwrap()
        .item;

        assert_eq!(
            format!("{cdb:#?}"),
            r#"CompoundDelimitedBlock {
    blocks: &[
        Block::Simple(
            SimpleBlock {
                content: Content {
                    original: Span {
                        data: "block1",
                        line: 2,
                        col: 1,
                        offset: 5,
                    },
                    rendered: "block1",
                },
                source: Span {
                    data: "block1",
                    line: 2,
                    col: 1,
                    offset: 5,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },
        ),
        Block::Simple(
            SimpleBlock {
                content: Content {
                    original: Span {
                        data: "block2",
                        line: 4,
                        col: 1,
                        offset: 13,
                    },
                    rendered: "block2",
                },
                source: Span {
                    data: "block2",
                    line: 4,
                    col: 1,
                    offset: 13,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
            },
        ),
    ],
    context: "example",
    source: Span {
        data: "====\nblock1\n\nblock2\n====",
        line: 1,
        col: 1,
        offset: 0,
    },
    title_source: None,
    title: None,
    anchor: None,
    anchor_reftext: None,
    attrlist: None,
}"#
        );
    }
}
