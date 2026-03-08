use crate::{
    HasSpan, Parser, Span,
    attributes::Attrlist,
    blocks::{ContentModel, IsBlock, metadata::BlockMetadata},
    span::MatchedItem,
    strings::CowStr,
};

/// A break block is used to represent a thematic or page break macro.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Break<'src> {
    type_: BreakType,
    source: Span<'src>,
    title_source: Option<Span<'src>>,
    title: Option<String>,
    anchor: Option<Span<'src>>,
    attrlist: Option<Attrlist<'src>>,
}

/// A break may be one of two different types.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum BreakType {
    /// A thematic break (aka horizontal rule).
    Thematic,

    /// A hint to the converter to insert a page break.
    Page,
}

impl std::fmt::Debug for BreakType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BreakType::Thematic => write!(f, "BreakType::Thematic"),
            BreakType::Page => write!(f, "BreakType::Page"),
        }
    }
}

impl<'src> Break<'src> {
    pub(crate) fn parse(
        metadata: &BlockMetadata<'src>,
        _parser: &mut Parser,
    ) -> Option<MatchedItem<'src, Self>> {
        let line = metadata.block_start.take_normalized_line();

        let type_ = match line.item.data() {
            "'''" | "---" | "- - -" | "***" | "* * *" => BreakType::Thematic,
            "<<<" => BreakType::Page,
            _ => {
                return None;
            }
        };

        let source: Span = metadata.source.trim_remainder(line.after);
        let source = source.slice(0..source.trim().len());

        Some(MatchedItem {
            item: Self {
                type_,
                source,
                title_source: metadata.title_source,
                title: metadata.title.clone(),
                anchor: metadata.anchor,
                attrlist: metadata.attrlist.clone(),
            },

            after: line.after.discard_empty_lines(),
        })
    }

    /// Return the type of break detected.
    pub fn type_(&self) -> BreakType {
        self.type_
    }
}

impl<'src> IsBlock<'src> for Break<'src> {
    fn content_model(&self) -> ContentModel {
        ContentModel::Empty
    }

    fn raw_context(&self) -> CowStr<'src> {
        match self.type_ {
            BreakType::Thematic => "thematic_break",
            BreakType::Page => "page_break",
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
        None
    }

    fn attrlist(&'src self) -> Option<&'src Attrlist<'src>> {
        self.attrlist.as_ref()
    }
}

impl<'src> HasSpan<'src> for Break<'src> {
    fn span(&self) -> Span<'src> {
        self.source
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::ops::Deref;

    use crate::{
        blocks::{BreakType, ContentModel, IsBlock, metadata::BlockMetadata},
        content::SubstitutionGroup,
        tests::prelude::*,
    };

    #[test]
    fn impl_clone() {
        // Silly test to mark the #[derive(...)] line as covered.
        let mut parser = Parser::default();

        let b1 = crate::blocks::Break::parse(&BlockMetadata::new("'''"), &mut parser)
            .unwrap()
            .item;

        let b2 = b1.clone();
        assert_eq!(b1, b2);
    }

    #[test]
    fn err_empty_source() {
        let mut parser = Parser::default();
        assert!(crate::blocks::Break::parse(&BlockMetadata::new(""), &mut parser).is_none());
    }

    #[test]
    fn err_only_spaces() {
        let mut parser = Parser::default();
        assert!(crate::blocks::Break::parse(&BlockMetadata::new("    "), &mut parser).is_none());
    }

    #[test]
    fn err_unknown_break_pattern() {
        let mut parser = Parser::default();
        assert!(crate::blocks::Break::parse(&BlockMetadata::new("=="), &mut parser).is_none());
        assert!(crate::blocks::Break::parse(&BlockMetadata::new("~~~"), &mut parser).is_none());
        assert!(crate::blocks::Break::parse(&BlockMetadata::new("****"), &mut parser).is_none());
        assert!(crate::blocks::Break::parse(&BlockMetadata::new(">>>"), &mut parser).is_none());
    }

    #[test]
    fn thematic_break_triple_apostrophe() {
        let mut parser = Parser::default();

        let mi = crate::blocks::Break::parse(&BlockMetadata::new("'''"), &mut parser).unwrap();

        assert_eq!(
            mi.item,
            Break {
                type_: BreakType::Thematic,
                source: Span {
                    data: "'''",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                attrlist: None,
            }
        );

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 1,
                col: 4,
                offset: 3
            }
        );

        assert_eq!(mi.item.content_model(), ContentModel::Empty);
        assert_eq!(mi.item.raw_context().deref(), "thematic_break");
        assert_eq!(mi.item.type_(), BreakType::Thematic);
        assert!(mi.item.nested_blocks().next().is_none());
        assert!(mi.item.title_source().is_none());
        assert!(mi.item.title().is_none());
        assert!(mi.item.anchor().is_none());
        assert!(mi.item.anchor_reftext().is_none());
        assert!(mi.item.attrlist().is_none());
        assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);
    }

    #[test]
    fn thematic_break_triple_hyphen() {
        let mut parser = Parser::default();

        let mi = crate::blocks::Break::parse(&BlockMetadata::new("---"), &mut parser).unwrap();

        assert_eq!(
            mi.item,
            Break {
                type_: BreakType::Thematic,
                source: Span {
                    data: "---",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                attrlist: None,
            }
        );

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 1,
                col: 4,
                offset: 3
            }
        );

        assert_eq!(mi.item.content_model(), ContentModel::Empty);
        assert_eq!(mi.item.raw_context().deref(), "thematic_break");
        assert_eq!(mi.item.type_(), BreakType::Thematic);
    }

    #[test]
    fn thematic_break_spaced_hyphen() {
        let mut parser = Parser::default();

        let mi = crate::blocks::Break::parse(&BlockMetadata::new("- - -"), &mut parser).unwrap();

        assert_eq!(
            mi.item,
            Break {
                type_: BreakType::Thematic,
                source: Span {
                    data: "- - -",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                attrlist: None,
            }
        );

        assert_eq!(mi.item.type_(), BreakType::Thematic);
    }

    #[test]
    fn thematic_break_triple_asterisk() {
        let mut parser = Parser::default();

        let mi = crate::blocks::Break::parse(&BlockMetadata::new("***"), &mut parser).unwrap();

        assert_eq!(
            mi.item,
            Break {
                type_: BreakType::Thematic,
                source: Span {
                    data: "***",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                attrlist: None,
            }
        );

        assert_eq!(mi.item.content_model(), ContentModel::Empty);
        assert_eq!(mi.item.raw_context().deref(), "thematic_break");
        assert_eq!(mi.item.type_(), BreakType::Thematic);
    }

    #[test]
    fn thematic_break_spaced_asterisk() {
        let mut parser = Parser::default();

        let mi = crate::blocks::Break::parse(&BlockMetadata::new("* * *"), &mut parser).unwrap();

        assert_eq!(
            mi.item,
            Break {
                type_: BreakType::Thematic,
                source: Span {
                    data: "* * *",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                attrlist: None,
            }
        );

        assert_eq!(mi.item.type_(), BreakType::Thematic);
    }

    #[test]
    fn page_break() {
        let mut parser = Parser::default();

        let mi = crate::blocks::Break::parse(&BlockMetadata::new("<<<"), &mut parser).unwrap();

        assert_eq!(
            mi.item,
            Break {
                type_: BreakType::Page,
                source: Span {
                    data: "<<<",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                attrlist: None,
            }
        );

        assert_eq!(
            mi.after,
            Span {
                data: "",
                line: 1,
                col: 4,
                offset: 3
            }
        );

        assert_eq!(mi.item.content_model(), ContentModel::Empty);
        assert_eq!(mi.item.raw_context().deref(), "page_break");
        assert_eq!(mi.item.type_(), BreakType::Page);
        assert!(mi.item.nested_blocks().next().is_none());
        assert!(mi.item.title_source().is_none());
        assert!(mi.item.title().is_none());
        assert!(mi.item.anchor().is_none());
        assert!(mi.item.anchor_reftext().is_none());
        assert!(mi.item.attrlist().is_none());
        assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Normal);
    }

    #[test]
    fn thematic_break_with_trailing_whitespace() {
        let mut parser = Parser::default();

        let mi = crate::blocks::Break::parse(&BlockMetadata::new("'''   "), &mut parser).unwrap();

        assert_eq!(
            mi.item,
            Break {
                type_: BreakType::Thematic,
                source: Span {
                    data: "'''",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                attrlist: None,
            }
        );

        assert_eq!(mi.item.type_(), BreakType::Thematic);
    }

    #[test]
    fn page_break_with_trailing_whitespace() {
        let mut parser = Parser::default();

        let mi = crate::blocks::Break::parse(&BlockMetadata::new("<<<   "), &mut parser).unwrap();

        assert_eq!(
            mi.item,
            Break {
                type_: BreakType::Page,
                source: Span {
                    data: "<<<",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                attrlist: None,
            }
        );

        assert_eq!(mi.item.type_(), BreakType::Page);
    }

    mod break_type {
        mod impl_debug {
            use crate::blocks::BreakType;

            #[test]
            fn thematic() {
                let break_type = BreakType::Thematic;
                let debug_output = format!("{:?}", break_type);
                assert_eq!(debug_output, "BreakType::Thematic");
            }

            #[test]
            fn page() {
                let break_type = BreakType::Page;
                let debug_output = format!("{:?}", break_type);
                assert_eq!(debug_output, "BreakType::Page");
            }
        }
    }
}
