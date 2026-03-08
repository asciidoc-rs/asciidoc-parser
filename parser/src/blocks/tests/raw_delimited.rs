mod parse {
    use crate::{blocks::ContentModel, tests::prelude::*, warnings::WarningType};

    #[test]
    fn err_invalid_delimiter() {
        let mut parser = Parser::default();

        assert!(
            crate::blocks::Block::parse(crate::Span::default(), &mut parser)
                .unwrap_if_no_warnings()
                .is_none()
        );

        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(crate::Span::new("..."), &mut parser)
            .unwrap_if_no_warnings()
            .unwrap();

        assert_eq!(
            mi.item,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "...",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "&#8230;&#8203;",
                },
                source: Span {
                    data: "...",
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
            })
        );

        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(crate::Span::new("++++x"), &mut parser)
            .unwrap_if_no_warnings()
            .unwrap();

        assert_eq!(
            mi.item,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "++++x",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "x",
                },
                source: Span {
                    data: "++++x",
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
            })
        );

        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(crate::Span::new("____x"), &mut parser)
            .unwrap_if_no_warnings()
            .unwrap();

        assert_eq!(
            mi.item,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "____x",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "____x",
                },
                source: Span {
                    data: "____x",
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
            })
        );

        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(crate::Span::new("====x"), &mut parser)
            .unwrap_if_no_warnings()
            .unwrap();

        assert_eq!(
            mi.item,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "====x",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "====x",
                },
                source: Span {
                    data: "====x",
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
            })
        );
    }

    #[test]
    fn err_unterminated() {
        let mut parser = Parser::default();
        let maw =
            crate::blocks::Block::parse(crate::Span::new("....\nblah blah blah"), &mut parser);

        let mi = maw.item.unwrap().clone();

        assert_eq!(
            mi.item,
            Block::RawDelimited(RawDelimitedBlock {
                content: Content {
                    original: Span {
                        data: "blah blah blah",
                        line: 2,
                        col: 1,
                        offset: 5,
                    },
                    rendered: "blah blah blah",
                },
                content_model: ContentModel::Verbatim,
                context: "literal",
                source: Span {
                    data: "....\nblah blah blah",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
                substitution_group: SubstitutionGroup::Verbatim,
            },)
        );

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
        blocks::{ContentModel, IsBlock},
        tests::prelude::*,
    };

    #[test]
    fn empty() {
        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(crate::Span::new("////\n////"), &mut parser)
            .unwrap_if_no_warnings()
            .unwrap();

        assert_eq!(
            mi.item,
            Block::RawDelimited(RawDelimitedBlock {
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
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
                substitution_group: SubstitutionGroup::None,
            })
        );

        assert_eq!(mi.item.content_model(), ContentModel::Raw);
        assert_eq!(mi.item.rendered_content(), Some(""));
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
    }

    #[test]
    fn title() {
        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(
            crate::Span::new(".comment\n////\nline1  \nline2\n////"),
            &mut parser,
        )
        .unwrap_if_no_warnings()
        .unwrap();

        assert_eq!(
            mi.item,
            Block::RawDelimited(RawDelimitedBlock {
                content: Content {
                    original: Span {
                        data: "line1  \nline2",
                        line: 3,
                        col: 1,
                        offset: 14,
                    },
                    rendered: "line1  \nline2",
                },
                content_model: ContentModel::Raw,
                context: "comment",
                source: Span {
                    data: ".comment\n////\nline1  \nline2\n////",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: Some(Span {
                    data: "comment",
                    line: 1,
                    col: 2,
                    offset: 1,
                },),
                title: Some("comment"),
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
                substitution_group: SubstitutionGroup::None,
            })
        );

        assert_eq!(mi.item.content_model(), ContentModel::Raw);
        assert_eq!(mi.item.rendered_content(), Some("line1  \nline2"));
        assert_eq!(mi.item.raw_context().as_ref(), "comment");
        assert_eq!(mi.item.resolved_context().as_ref(), "comment");
        assert!(mi.item.declared_style().is_none());
        assert!(mi.item.id().is_none());
        assert!(mi.item.roles().is_empty());
        assert!(mi.item.options().is_empty());
        assert_eq!(mi.item.substitution_group(), SubstitutionGroup::None);

        assert_eq!(
            mi.item.title_source().unwrap(),
            Span {
                data: "comment",
                line: 1,
                col: 2,
                offset: 1,
            }
        );

        assert_eq!(mi.item.title_source().unwrap().data(), "comment");
        assert_eq!(mi.item.title().unwrap(), "comment");

        assert!(mi.item.anchor().is_none());
        assert!(mi.item.anchor_reftext().is_none());
        assert!(mi.item.attrlist().is_none());
    }

    #[test]
    fn multiple_lines() {
        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(
            crate::Span::new("////\nline1  \nline2\n////"),
            &mut parser,
        )
        .unwrap_if_no_warnings()
        .unwrap();

        assert_eq!(
            mi.item,
            Block::RawDelimited(RawDelimitedBlock {
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
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
                substitution_group: SubstitutionGroup::None,
            })
        );

        assert_eq!(mi.item.content_model(), ContentModel::Raw);
        assert_eq!(mi.item.rendered_content(), Some("line1  \nline2"));
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
    }

    #[test]
    fn ignores_delimiter_prefix() {
        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(
            crate::Span::new("////\nline1  \n/////\nline2\n////"),
            &mut parser,
        )
        .unwrap_if_no_warnings()
        .unwrap();

        assert_eq!(
            mi.item,
            Block::RawDelimited(RawDelimitedBlock {
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
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
                substitution_group: SubstitutionGroup::None,
            })
        );

        assert_eq!(mi.item.content_model(), ContentModel::Raw);
        assert_eq!(mi.item.rendered_content(), Some("line1  \n/////\nline2"));
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
    }
}

mod listing {
    use crate::{
        blocks::{ContentModel, IsBlock},
        tests::prelude::*,
    };

    #[test]
    fn empty() {
        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(crate::Span::new("----\n----"), &mut parser)
            .unwrap_if_no_warnings()
            .unwrap();

        assert_eq!(
            mi.item,
            Block::RawDelimited(RawDelimitedBlock {
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
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
                substitution_group: SubstitutionGroup::Verbatim,
            })
        );

        assert_eq!(mi.item.content_model(), ContentModel::Verbatim);
        assert_eq!(mi.item.rendered_content(), Some(""));
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
    }

    #[test]
    fn multiple_lines() {
        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(
            crate::Span::new("----\nline1  \nline2\n----"),
            &mut parser,
        )
        .unwrap_if_no_warnings()
        .unwrap();

        assert_eq!(
            mi.item,
            Block::RawDelimited(RawDelimitedBlock {
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
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
                substitution_group: SubstitutionGroup::Verbatim,
            })
        );

        assert_eq!(mi.item.content_model(), ContentModel::Verbatim);
        assert_eq!(mi.item.rendered_content(), Some("line1  \nline2"));
        assert_eq!(mi.item.raw_context().as_ref(), "listing");
        assert_eq!(mi.item.resolved_context().as_ref(), "listing");
        assert!(mi.item.declared_style().is_none());
        assert_eq!(mi.item.nested_blocks().next(), None);
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
            mi.item.span(),
            Span {
                data: "----\nline1  \nline2\n----",
                line: 1,
                col: 1,
                offset: 0,
            }
        );
    }

    #[test]
    fn title() {
        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(
            crate::Span::new(".listing title\n----\nline1  \nline2\n----"),
            &mut parser,
        )
        .unwrap_if_no_warnings()
        .unwrap();

        assert_eq!(
            mi.item,
            Block::RawDelimited(RawDelimitedBlock {
                content: Content {
                    original: Span {
                        data: "line1  \nline2",
                        line: 3,
                        col: 1,
                        offset: 20,
                    },
                    rendered: "line1  \nline2",
                },
                content_model: ContentModel::Verbatim,
                context: "listing",
                source: Span {
                    data: ".listing title\n----\nline1  \nline2\n----",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: Some(Span {
                    data: "listing title",
                    line: 1,
                    col: 2,
                    offset: 1,
                },),
                title: Some("listing title"),
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
                substitution_group: SubstitutionGroup::Verbatim,
            })
        );

        assert_eq!(mi.item.content_model(), ContentModel::Verbatim);
        assert_eq!(mi.item.rendered_content(), Some("line1  \nline2"));
        assert_eq!(mi.item.raw_context().as_ref(), "listing");
        assert_eq!(mi.item.resolved_context().as_ref(), "listing");
        assert!(mi.item.declared_style().is_none());
        assert_eq!(mi.item.nested_blocks().next(), None);
        assert!(mi.item.id().is_none());
        assert!(mi.item.roles().is_empty());
        assert!(mi.item.options().is_empty());
        assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Verbatim);

        assert_eq!(
            mi.item.title_source().unwrap(),
            Span {
                data: "listing title",
                line: 1,
                col: 2,
                offset: 1,
            }
        );

        assert_eq!(mi.item.title_source().unwrap().data(), "listing title");
        assert_eq!(mi.item.title().unwrap(), "listing title");

        assert!(mi.item.anchor().is_none());
        assert!(mi.item.anchor_reftext().is_none());
        assert!(mi.item.attrlist().is_none());

        assert_eq!(
            mi.item.span(),
            Span {
                data: ".listing title\n----\nline1  \nline2\n----",
                line: 1,
                col: 1,
                offset: 0,
            }
        );
    }

    #[test]
    fn ignores_delimiter_prefix() {
        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(
            crate::Span::new("----\nline1  \n----/\nline2\n----"),
            &mut parser,
        )
        .unwrap_if_no_warnings()
        .unwrap();

        assert_eq!(
            mi.item,
            Block::RawDelimited(RawDelimitedBlock {
                content: Content {
                    original: Span {
                        data: "line1  \n----/\nline2",
                        line: 2,
                        col: 1,
                        offset: 5,
                    },
                    rendered: "line1  \n----/\nline2",
                },
                content_model: ContentModel::Verbatim,
                context: "listing",
                source: Span {
                    data: "----\nline1  \n----/\nline2\n----",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
                substitution_group: SubstitutionGroup::Verbatim,
            })
        );

        assert_eq!(mi.item.content_model(), ContentModel::Verbatim);
        assert_eq!(mi.item.rendered_content(), Some("line1  \n----/\nline2"));
        assert_eq!(mi.item.raw_context().as_ref(), "listing");
        assert_eq!(mi.item.resolved_context().as_ref(), "listing");
        assert!(mi.item.declared_style().is_none());
        assert_eq!(mi.item.nested_blocks().next(), None);
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
            mi.item.span(),
            Span {
                data: "----\nline1  \n----/\nline2\n----",
                line: 1,
                col: 1,
                offset: 0,
            }
        );
    }
}

mod pass {
    use crate::{
        blocks::{ContentModel, IsBlock},
        tests::prelude::*,
    };

    #[test]
    fn empty() {
        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(crate::Span::new("++++\n++++"), &mut parser)
            .unwrap_if_no_warnings()
            .unwrap();

        assert_eq!(
            mi.item,
            Block::RawDelimited(RawDelimitedBlock {
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
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
                substitution_group: SubstitutionGroup::Pass,
            })
        );

        assert_eq!(mi.item.content_model(), ContentModel::Raw);
        assert_eq!(mi.item.rendered_content(), Some(""));
        assert_eq!(mi.item.raw_context().as_ref(), "pass");
        assert_eq!(mi.item.resolved_context().as_ref(), "pass");
        assert!(mi.item.declared_style().is_none());
        assert_eq!(mi.item.nested_blocks().next(), None);
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
            mi.item.span(),
            Span {
                data: "++++\n++++",
                line: 1,
                col: 1,
                offset: 0,
            }
        );
    }

    #[test]
    fn multiple_lines() {
        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(
            crate::Span::new("++++\nline1  \nline2\n++++"),
            &mut parser,
        )
        .unwrap_if_no_warnings()
        .unwrap();

        assert_eq!(
            mi.item,
            Block::RawDelimited(RawDelimitedBlock {
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
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
                substitution_group: SubstitutionGroup::Pass,
            })
        );

        assert_eq!(mi.item.content_model(), ContentModel::Raw);
        assert_eq!(mi.item.rendered_content(), Some("line1  \nline2"));
        assert_eq!(mi.item.raw_context().as_ref(), "pass");
        assert_eq!(mi.item.resolved_context().as_ref(), "pass");
        assert!(mi.item.declared_style().is_none());
        assert_eq!(mi.item.nested_blocks().next(), None);
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
            mi.item.span(),
            Span {
                data: "++++\nline1  \nline2\n++++",
                line: 1,
                col: 1,
                offset: 0,
            }
        );
    }

    #[test]
    fn title() {
        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(
            crate::Span::new(".pass title\n++++\nline1  \nline2\n++++"),
            &mut parser,
        )
        .unwrap_if_no_warnings()
        .unwrap();

        assert_eq!(
            mi.item,
            Block::RawDelimited(RawDelimitedBlock {
                content: Content {
                    original: Span {
                        data: "line1  \nline2",
                        line: 3,
                        col: 1,
                        offset: 17,
                    },
                    rendered: "line1  \nline2",
                },
                content_model: ContentModel::Raw,
                context: "pass",
                source: Span {
                    data: ".pass title\n++++\nline1  \nline2\n++++",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: Some(Span {
                    data: "pass title",
                    line: 1,
                    col: 2,
                    offset: 1,
                },),
                title: Some("pass title"),
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
                substitution_group: SubstitutionGroup::Pass,
            })
        );

        assert_eq!(mi.item.content_model(), ContentModel::Raw);
        assert_eq!(mi.item.rendered_content(), Some("line1  \nline2"));
        assert_eq!(mi.item.raw_context().as_ref(), "pass");
        assert_eq!(mi.item.resolved_context().as_ref(), "pass");
        assert!(mi.item.declared_style().is_none());
        assert_eq!(mi.item.nested_blocks().next(), None);
        assert!(mi.item.id().is_none());
        assert!(mi.item.roles().is_empty());
        assert!(mi.item.options().is_empty());
        assert_eq!(mi.item.substitution_group(), SubstitutionGroup::Pass);

        assert_eq!(
            mi.item.title_source().unwrap(),
            Span {
                data: "pass title",
                line: 1,
                col: 2,
                offset: 1,
            }
        );

        assert_eq!(mi.item.title_source().unwrap().data(), "pass title");
        assert_eq!(mi.item.title().unwrap(), "pass title");

        assert!(mi.item.anchor().is_none());
        assert!(mi.item.anchor_reftext().is_none());
        assert!(mi.item.attrlist().is_none());

        assert_eq!(
            mi.item.span(),
            Span {
                data: ".pass title\n++++\nline1  \nline2\n++++",
                line: 1,
                col: 1,
                offset: 0,
            }
        );
    }

    #[test]
    fn ignores_delimiter_prefix() {
        let mut parser = Parser::default();

        let mi = crate::blocks::Block::parse(
            crate::Span::new("++++\nline1  \n++++/\nline2\n++++"),
            &mut parser,
        )
        .unwrap_if_no_warnings()
        .unwrap();

        assert_eq!(
            mi.item,
            Block::RawDelimited(RawDelimitedBlock {
                content: Content {
                    original: Span {
                        data: "line1  \n++++/\nline2",
                        line: 2,
                        col: 1,
                        offset: 5,
                    },
                    rendered: "line1  \n++++/\nline2",
                },
                content_model: ContentModel::Raw,
                context: "pass",
                source: Span {
                    data: "++++\nline1  \n++++/\nline2\n++++",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: None,
                substitution_group: SubstitutionGroup::Pass,
            })
        );

        assert_eq!(mi.item.content_model(), ContentModel::Raw);
        assert_eq!(mi.item.rendered_content(), Some("line1  \n++++/\nline2"));
        assert_eq!(mi.item.raw_context().as_ref(), "pass");
        assert_eq!(mi.item.resolved_context().as_ref(), "pass");
        assert!(mi.item.declared_style().is_none());
        assert_eq!(mi.item.nested_blocks().next(), None);
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
            mi.item.span(),
            Span {
                data: "++++\nline1  \n++++/\nline2\n++++",
                line: 1,
                col: 1,
                offset: 0,
            }
        );
    }
}
