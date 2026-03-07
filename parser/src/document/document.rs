//! Describes the top-level document structure.

use std::{marker::PhantomData, slice::Iter};

use self_cell::self_cell;

use crate::{
    Parser, Span,
    attributes::Attrlist,
    blocks::{Block, ContentModel, IsBlock, Preamble, parse_utils::parse_blocks_until},
    document::{Catalog, Header},
    internal::debug::DebugSliceReference,
    parser::SourceMap,
    strings::CowStr,
    warnings::Warning,
};

/// A document represents the top-level block element in AsciiDoc. It consists
/// of an optional document header and either a) one or more sections preceded
/// by an optional preamble or b) a sequence of top-level blocks only.
///
/// The document can be configured using a document header. The header is not a
/// block itself, but contributes metadata to the document, such as the document
/// title and document attributes.
///
/// The `Document` structure is a self-contained package of the original content
/// that was parsed and the data structures that describe that parsed content.
/// The API functions on this struct can be used to understand the parse
/// results.
#[derive(Eq, PartialEq)]
pub struct Document<'src> {
    internal: Internal,
    _phantom: PhantomData<&'src ()>,
}

/// Internal dependent struct containing the actual data members that reference
/// the owned source.
#[derive(Debug, Eq, PartialEq)]
struct InternalDependent<'src> {
    header: Header<'src>,
    blocks: Vec<Block<'src>>,
    source: Span<'src>,
    warnings: Vec<Warning<'src>>,
    source_map: SourceMap,
    catalog: Catalog,
}

self_cell! {
    /// Internal implementation struct containing the actual data members.
    struct Internal {
        owner: String,
        #[covariant]
        dependent: InternalDependent,
    }
    impl {Debug, Eq, PartialEq}
}

impl<'src> Document<'src> {
    pub(crate) fn parse(source: &str, source_map: SourceMap, parser: &mut Parser) -> Self {
        let owned_source = source.to_string();

        let internal = Internal::new(owned_source, |owned_src| {
            let source = Span::new(owned_src);

            let mi = Header::parse(source, parser);
            let after_header = mi.item.after;

            parser.sectnumlevels = parser
                .attribute_value("sectnumlevels")
                .as_maybe_str()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(3);

            let header = mi.item.item;
            let mut warnings = mi.warnings;

            let mut maw_blocks = parse_blocks_until(after_header, |_| false, parser);

            if !maw_blocks.warnings.is_empty() {
                warnings.append(&mut maw_blocks.warnings);
            }

            let mut blocks = maw_blocks.item.item;
            let mut has_content_blocks = false;
            let mut preamble_split_index: Option<usize> = None;

            // Only look for preamble content if document has a title.
            // Asciidoctor only creates a preamble when there's a document title.
            if header.title().is_some() {
                for (index, block) in blocks.iter().enumerate() {
                    match block {
                        Block::DocumentAttribute(_) => (),
                        Block::Section(_) => {
                            if has_content_blocks {
                                preamble_split_index = Some(index);
                            }
                            break;
                        }
                        _ => {
                            has_content_blocks = true;
                        }
                    }
                }
            }

            if let Some(index) = preamble_split_index {
                let mut section_blocks = blocks.split_off(index);

                let preamble = Preamble::from_blocks(blocks, after_header);

                section_blocks.insert(0, Block::Preamble(preamble));
                blocks = section_blocks;
            }

            InternalDependent {
                header,
                blocks,
                source: source.trim_trailing_whitespace(),
                warnings,
                source_map,
                catalog: parser.take_catalog(),
            }
        });

        Self {
            internal,
            _phantom: PhantomData,
        }
    }

    /// Return the document header.
    pub fn header(&self) -> &Header<'_> {
        &self.internal.borrow_dependent().header
    }

    /// Return an iterator over any warnings found during parsing.
    pub fn warnings(&self) -> Iter<'_, Warning<'_>> {
        self.internal.borrow_dependent().warnings.iter()
    }

    /// Return a [`Span`] describing the entire document source.
    pub fn span(&self) -> Span<'_> {
        self.internal.borrow_dependent().source
    }

    /// Return the source map that tracks original file locations.
    pub fn source_map(&self) -> &SourceMap {
        &self.internal.borrow_dependent().source_map
    }

    /// Return the document catalog for accessing referenceable elements.
    pub fn catalog(&self) -> &Catalog {
        &self.internal.borrow_dependent().catalog
    }
}

impl<'src> IsBlock<'src> for Document<'src> {
    fn content_model(&self) -> ContentModel {
        ContentModel::Compound
    }

    fn raw_context(&self) -> CowStr<'src> {
        "document".into()
    }

    fn nested_blocks(&'src self) -> Iter<'src, Block<'src>> {
        self.internal.borrow_dependent().blocks.iter()
    }

    fn title_source(&'src self) -> Option<Span<'src>> {
        // Document title is reflected in the Header.
        None
    }

    fn title(&self) -> Option<&str> {
        // Document title is reflected in the Header.
        None
    }

    fn anchor(&'src self) -> Option<Span<'src>> {
        None
    }

    fn anchor_reftext(&'src self) -> Option<Span<'src>> {
        None
    }

    fn attrlist(&'src self) -> Option<&'src Attrlist<'src>> {
        // Document attributes are reflected in the Header.
        None
    }
}

impl std::fmt::Debug for Document<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let dependent = self.internal.borrow_dependent();
        f.debug_struct("Document")
            .field("header", &dependent.header)
            .field("blocks", &DebugSliceReference(&dependent.blocks))
            .field("source", &dependent.source)
            .field("warnings", &DebugSliceReference(&dependent.warnings))
            .field("source_map", &dependent.source_map)
            .field("catalog", &dependent.catalog)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::{collections::HashMap, ops::Deref};

    use pretty_assertions_sorted::assert_eq;

    use crate::{
        Parser,
        blocks::{ContentModel, IsBlock, MediaType, SimpleBlockStyle},
        content::SubstitutionGroup,
        document::RefType,
        tests::prelude::*,
        warnings::WarningType,
    };

    #[test]
    fn empty_source() {
        let doc = Parser::default().parse("");

        assert_eq!(doc.content_model(), ContentModel::Compound);
        assert_eq!(doc.raw_context().deref(), "document");
        assert_eq!(doc.resolved_context().deref(), "document");
        assert!(doc.declared_style().is_none());
        assert!(doc.id().is_none());
        assert!(doc.roles().is_empty());
        assert!(doc.title_source().is_none());
        assert!(doc.title().is_none());
        assert!(doc.anchor().is_none());
        assert!(doc.anchor_reftext().is_none());
        assert!(doc.attrlist().is_none());
        assert_eq!(doc.substitution_group(), SubstitutionGroup::Normal);

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "",
                        line: 1,
                        col: 1,
                        offset: 0
                    },
                },
                source: Span {
                    data: "",
                    line: 1,
                    col: 1,
                    offset: 0
                },
                blocks: &[],
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }

    #[test]
    fn only_spaces() {
        assert_eq!(
            Parser::default().parse("    "),
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "",
                        line: 1,
                        col: 5,
                        offset: 4
                    },
                },
                source: Span {
                    data: "",
                    line: 1,
                    col: 1,
                    offset: 0
                },
                blocks: &[],
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }

    #[test]
    fn one_simple_block() {
        let doc = Parser::default().parse("abc");
        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "",
                        line: 1,
                        col: 1,
                        offset: 0
                    },
                },
                source: Span {
                    data: "abc",
                    line: 1,
                    col: 1,
                    offset: 0
                },
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "abc",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: "abc",
                    },
                    source: Span {
                        data: "abc",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                })],
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );

        assert!(doc.anchor().is_none());
        assert!(doc.anchor_reftext().is_none());
    }

    #[test]
    fn two_simple_blocks() {
        assert_eq!(
            Parser::default().parse("abc\n\ndef"),
            Document {
                header: Header {
                    title_source: None,
                    title: None,
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "",
                        line: 1,
                        col: 1,
                        offset: 0
                    },
                },
                source: Span {
                    data: "abc\n\ndef",
                    line: 1,
                    col: 1,
                    offset: 0
                },
                blocks: &[
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "abc",
                                line: 1,
                                col: 1,
                                offset: 0,
                            },
                            rendered: "abc",
                        },
                        source: Span {
                            data: "abc",
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    }),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "def",
                                line: 3,
                                col: 1,
                                offset: 5,
                            },
                            rendered: "def",
                        },
                        source: Span {
                            data: "def",
                            line: 3,
                            col: 1,
                            offset: 5,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    })
                ],
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }

    #[test]
    fn two_blocks_and_title() {
        assert_eq!(
            Parser::default().parse("= Example Title\n\nabc\n\ndef"),
            Document {
                header: Header {
                    title_source: Some(Span {
                        data: "Example Title",
                        line: 1,
                        col: 3,
                        offset: 2,
                    }),
                    title: Some("Example Title"),
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "= Example Title",
                        line: 1,
                        col: 1,
                        offset: 0,
                    }
                },
                blocks: &[
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "abc",
                                line: 3,
                                col: 1,
                                offset: 17,
                            },
                            rendered: "abc",
                        },
                        source: Span {
                            data: "abc",
                            line: 3,
                            col: 1,
                            offset: 17,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    }),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "def",
                                line: 5,
                                col: 1,
                                offset: 22,
                            },
                            rendered: "def",
                        },
                        source: Span {
                            data: "def",
                            line: 5,
                            col: 1,
                            offset: 22,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    })
                ],
                source: Span {
                    data: "= Example Title\n\nabc\n\ndef",
                    line: 1,
                    col: 1,
                    offset: 0
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }

    #[test]
    fn blank_lines_before_header() {
        let doc = Parser::default().parse("\n\n= Example Title\n\nabc\n\ndef");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: Some(Span {
                        data: "Example Title",
                        line: 3,
                        col: 3,
                        offset: 4,
                    },),
                    title: Some("Example Title",),
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "= Example Title",
                        line: 3,
                        col: 1,
                        offset: 2,
                    },
                },
                blocks: &[
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "abc",
                                line: 5,
                                col: 1,
                                offset: 19,
                            },
                            rendered: "abc",
                        },
                        source: Span {
                            data: "abc",
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
                    },),
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "def",
                                line: 7,
                                col: 1,
                                offset: 24,
                            },
                            rendered: "def",
                        },
                        source: Span {
                            data: "def",
                            line: 7,
                            col: 1,
                            offset: 24,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: "\n\n= Example Title\n\nabc\n\ndef",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }

    #[test]
    fn blank_lines_and_comment_before_header() {
        let doc =
            Parser::default().parse("\n// ignore this comment\n= Example Title\n\nabc\n\ndef");

        assert_eq!(
            doc,
            Document {
                header: Header {
                    title_source: Some(Span {
                        data: "Example Title",
                        line: 3,
                        col: 3,
                        offset: 26,
                    },),
                    title: Some("Example Title",),
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[Span {
                        data: "// ignore this comment",
                        line: 2,
                        col: 1,
                        offset: 1,
                    },],
                    source: Span {
                        data: "// ignore this comment\n= Example Title",
                        line: 2,
                        col: 1,
                        offset: 1,
                    },
                },
                blocks: &[
                    Block::Simple(SimpleBlock {
                        content: Content {
                            original: Span {
                                data: "abc",
                                line: 5,
                                col: 1,
                                offset: 41,
                            },
                            rendered: "abc",
                        },
                        source: Span {
                            data: "abc",
                            line: 5,
                            col: 1,
                            offset: 41,
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
                                data: "def",
                                line: 7,
                                col: 1,
                                offset: 46,
                            },
                            rendered: "def",
                        },
                        source: Span {
                            data: "def",
                            line: 7,
                            col: 1,
                            offset: 46,
                        },
                        style: SimpleBlockStyle::Paragraph,
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                    },),
                ],
                source: Span {
                    data: "\n// ignore this comment\n= Example Title\n\nabc\n\ndef",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }

    #[test]
    fn extra_space_before_title() {
        assert_eq!(
            Parser::default().parse("=   Example Title\n\nabc"),
            Document {
                header: Header {
                    title_source: Some(Span {
                        data: "Example Title",
                        line: 1,
                        col: 5,
                        offset: 4,
                    }),
                    title: Some("Example Title"),
                    attributes: &[],
                    author_line: None,
                    revision_line: None,
                    comments: &[],
                    source: Span {
                        data: "=   Example Title",
                        line: 1,
                        col: 1,
                        offset: 0,
                    }
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
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                })],
                source: Span {
                    data: "=   Example Title\n\nabc",
                    line: 1,
                    col: 1,
                    offset: 0
                },
                warnings: &[],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }

    #[test]
    fn err_bad_header() {
        assert_eq!(
            Parser::default().parse(
                "= Title\nJane Smith <jane@example.com>\nv1, 2025-09-28\nnot an attribute\n"
            ),
            Document {
                header: Header {
                    title_source: Some(Span {
                        data: "Title",
                        line: 1,
                        col: 3,
                        offset: 2,
                    }),
                    title: Some("Title"),
                    attributes: &[],
                    author_line: Some(AuthorLine {
                        authors: &[Author {
                            name: "Jane Smith",
                            firstname: "Jane",
                            middlename: None,
                            lastname: Some("Smith"),
                            email: Some("jane@example.com"),
                        }],
                        source: Span {
                            data: "Jane Smith <jane@example.com>",
                            line: 2,
                            col: 1,
                            offset: 8,
                        },
                    }),
                    revision_line: Some(RevisionLine {
                        revnumber: Some("1",),
                        revdate: "2025-09-28",
                        revremark: None,
                        source: Span {
                            data: "v1, 2025-09-28",
                            line: 3,
                            col: 1,
                            offset: 38,
                        },
                    },),
                    comments: &[],
                    source: Span {
                        data: "= Title\nJane Smith <jane@example.com>\nv1, 2025-09-28",
                        line: 1,
                        col: 1,
                        offset: 0,
                    }
                },
                blocks: &[Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: "not an attribute",
                            line: 4,
                            col: 1,
                            offset: 53,
                        },
                        rendered: "not an attribute",
                    },
                    source: Span {
                        data: "not an attribute",
                        line: 4,
                        col: 1,
                        offset: 53,
                    },
                    style: SimpleBlockStyle::Paragraph,
                    title_source: None,
                    title: None,
                    anchor: None,
                    anchor_reftext: None,
                    attrlist: None,
                })],
                source: Span {
                    data: "= Title\nJane Smith <jane@example.com>\nv1, 2025-09-28\nnot an attribute",
                    line: 1,
                    col: 1,
                    offset: 0
                },
                warnings: &[Warning {
                    source: Span {
                        data: "not an attribute",
                        line: 4,
                        col: 1,
                        offset: 53,
                    },
                    warning: WarningType::DocumentHeaderNotTerminated,
                },],
                source_map: SourceMap(&[]),
                catalog: Catalog::default(),
            }
        );
    }

    #[test]
    fn err_bad_header_and_bad_macro() {
        let doc = Parser::default().parse("= Title\nJane Smith <jane@example.com>\nv1, 2025-09-28\nnot an attribute\n\n== Section Title\n\nimage::bar[alt=Sunset,width=300,,height=400]");

        assert_eq!(
            Document {
                header: Header {
                    title_source: Some(Span {
                        data: "Title",
                        line: 1,
                        col: 3,
                        offset: 2,
                    }),
                    title: Some("Title"),
                    attributes: &[],
                    author_line: Some(AuthorLine {
                        authors: &[Author {
                            name: "Jane Smith",
                            firstname: "Jane",
                            middlename: None,
                            lastname: Some("Smith"),
                            email: Some("jane@example.com"),
                        }],
                        source: Span {
                            data: "Jane Smith <jane@example.com>",
                            line: 2,
                            col: 1,
                            offset: 8,
                        },
                    }),
                    revision_line: Some(RevisionLine {
                        revnumber: Some("1"),
                        revdate: "2025-09-28",
                        revremark: None,
                        source: Span {
                            data: "v1, 2025-09-28",
                            line: 3,
                            col: 1,
                            offset: 38,
                        },
                    },),
                    comments: &[],
                    source: Span {
                        data: "= Title\nJane Smith <jane@example.com>\nv1, 2025-09-28",
                        line: 1,
                        col: 1,
                        offset: 0,
                    }
                },
                blocks: &[
                    Block::Preamble(Preamble {
                        blocks: &[Block::Simple(SimpleBlock {
                            content: Content {
                                original: Span {
                                    data: "not an attribute",
                                    line: 4,
                                    col: 1,
                                    offset: 53,
                                },
                                rendered: "not an attribute",
                            },
                            source: Span {
                                data: "not an attribute",
                                line: 4,
                                col: 1,
                                offset: 53,
                            },
                            style: SimpleBlockStyle::Paragraph,
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "not an attribute",
                            line: 4,
                            col: 1,
                            offset: 53,
                        },
                    },),
                    Block::Section(SectionBlock {
                        level: 1,
                        section_title: Content {
                            original: Span {
                                data: "Section Title",
                                line: 6,
                                col: 4,
                                offset: 74,
                            },
                            rendered: "Section Title",
                        },
                        blocks: &[Block::Media(MediaBlock {
                            type_: MediaType::Image,
                            target: Span {
                                data: "bar",
                                line: 8,
                                col: 8,
                                offset: 96,
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
                                    },
                                ],
                                anchor: None,
                                source: Span {
                                    data: "alt=Sunset,width=300,,height=400",
                                    line: 8,
                                    col: 12,
                                    offset: 100,
                                },
                            },
                            source: Span {
                                data: "image::bar[alt=Sunset,width=300,,height=400]",
                                line: 8,
                                col: 1,
                                offset: 89,
                            },
                            title_source: None,
                            title: None,
                            anchor: None,
                            anchor_reftext: None,
                            attrlist: None,
                        },),],
                        source: Span {
                            data: "== Section Title\n\nimage::bar[alt=Sunset,width=300,,height=400]",
                            line: 6,
                            col: 1,
                            offset: 71,
                        },
                        title_source: None,
                        title: None,
                        anchor: None,
                        anchor_reftext: None,
                        attrlist: None,
                        section_type: SectionType::Normal,
                        section_id: Some("_section_title"),
                        section_number: None,
                    },)
                ],
                source: Span {
                    data: "= Title\nJane Smith <jane@example.com>\nv1, 2025-09-28\nnot an attribute\n\n== Section Title\n\nimage::bar[alt=Sunset,width=300,,height=400]",
                    line: 1,
                    col: 1,
                    offset: 0
                },
                warnings: &[
                    Warning {
                        source: Span {
                            data: "not an attribute",
                            line: 4,
                            col: 1,
                            offset: 53,
                        },
                        warning: WarningType::DocumentHeaderNotTerminated,
                    },
                    Warning {
                        source: Span {
                            data: "alt=Sunset,width=300,,height=400",
                            line: 8,
                            col: 12,
                            offset: 100,
                        },
                        warning: WarningType::EmptyAttributeValue,
                    },
                ],
                source_map: SourceMap(&[]),
                catalog: Catalog {
                    refs: HashMap::from([(
                        "_section_title",
                        RefEntry {
                            id: "_section_title",
                            reftext: Some("Section Title",),
                            ref_type: RefType::Section,
                        }
                    ),]),
                    reftext_to_id: HashMap::from([("Section Title", "_section_title"),]),
                }
            },
            doc
        );
    }

    #[test]
    fn impl_debug() {
        let doc = Parser::default().parse("= Example Title\n\nabc\n\ndef");

        assert_eq!(
            format!("{doc:#?}"),
            r#"Document {
    header: Header {
        title_source: Some(
            Span {
                data: "Example Title",
                line: 1,
                col: 3,
                offset: 2,
            },
        ),
        title: Some(
            "Example Title",
        ),
        attributes: &[],
        author_line: None,
        revision_line: None,
        comments: &[],
        source: Span {
            data: "= Example Title",
            line: 1,
            col: 1,
            offset: 0,
        },
    },
    blocks: &[
        Block::Simple(
            SimpleBlock {
                content: Content {
                    original: Span {
                        data: "abc",
                        line: 3,
                        col: 1,
                        offset: 17,
                    },
                    rendered: "abc",
                },
                source: Span {
                    data: "abc",
                    line: 3,
                    col: 1,
                    offset: 17,
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
                        data: "def",
                        line: 5,
                        col: 1,
                        offset: 22,
                    },
                    rendered: "def",
                },
                source: Span {
                    data: "def",
                    line: 5,
                    col: 1,
                    offset: 22,
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
    source: Span {
        data: "= Example Title\n\nabc\n\ndef",
        line: 1,
        col: 1,
        offset: 0,
    },
    warnings: &[],
    source_map: SourceMap(&[]),
    catalog: Catalog {
        refs: HashMap::from([]),
        reftext_to_id: HashMap::from([]),
    },
}"#
        );
    }
}
