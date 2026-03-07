// Adapted from Asciidoctor's #convert_quoted_text, found in
// https://github.com/asciidoctor/asciidoctor/blob/main/test/substitutions_test.rb.
//
// IMPORTANT: In porting this, I've disregarded compatibility mode (stated
// limitation of `asciidoc-parser` crate) and alternate (non-HTML) back ends.

mod dispatcher {
    use pretty_assertions_sorted::assert_eq;

    use crate::{Parser, blocks::SimpleBlockStyle, parser::ModificationContext, tests::prelude::*};

    #[test]
    fn apply_normal_substitutions() {
        let mut p = Parser::default().with_intrinsic_attribute(
            "inception_year",
            "2012",
            ModificationContext::Anywhere,
        );

        let maw = crate::blocks::Block::parse(
            crate::Span::new(
                "[blue]_http://asciidoc.org[AsciiDoc]_ & [red]*Ruby*\n&#167; Making +++<u>documentation</u>+++ together +\nsince (C) {inception_year}.",
            ),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "[blue]_http://asciidoc.org[AsciiDoc]_ & [red]*Ruby*\n&#167; Making +++<u>documentation</u>+++ together +\nsince (C) {inception_year}.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "<em class=\"blue\"><a href=\"http://asciidoc.org\">AsciiDoc</a></em> &amp; <strong class=\"red\">Ruby</strong>\n&#167; Making <u>documentation</u> together<br>\nsince &#169; 2012.",
                },
                source: Span {
                    data: "[blue]_http://asciidoc.org[AsciiDoc]_ & [red]*Ruby*\n&#167; Making +++<u>documentation</u>+++ together +\nsince (C) {inception_year}.",
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
            },)
        );
    }

    #[ignore]
    #[test]
    fn todo_migrate_from_ruby() {
        todo!(
            "{}",
            r###"
        # TODO
        # - test negatives
        # - test role on every quote type

        test 'should not drop trailing blank lines when performing substitutions' do
          para = block_from_string %([%hardbreaks]\nthis\nis\n-> {program})
          para.lines << ''
          para.lines << ''
          para.document.attributes['program'] = 'Asciidoctor'
          result = para.apply_subs para.lines
          assert_equal ['this<br>', 'is<br>', '&#8594; Asciidoctor<br>', '<br>', ''], result
          result = para.apply_subs para.lines * "\n"
          assert_equal %(this<br>\nis<br>\n&#8594; Asciidoctor<br>\n<br>\n), result
        end

        test 'should expand subs passed to expand_subs' do
          para = block_from_string %({program}\n*bold*\n2 > 1)
          para.document.attributes['program'] = 'Asciidoctor'
          assert_equal [:specialcharacters], (para.expand_subs [:specialchars])
          refute para.expand_subs([:none])
          assert_equal [:specialcharacters, :quotes, :attributes, :replacements, :macros, :post_replacements], (para.expand_subs [:normal])
        end

        test 'apply_subs should allow the subs argument to be nil' do
          block = block_from_string %([pass]\n*raw*)
          result = block.apply_subs block.source, nil
          assert_equal '*raw*', result
        end
        "###
        );
    }
}

mod quotes {
    use pretty_assertions_sorted::assert_eq;

    use crate::{
        Parser,
        blocks::SimpleBlockStyle,
        content::{SubstitutionGroup, SubstitutionStep},
        parser::ModificationContext,
        strings::CowStr,
        tests::prelude::*,
    };

    #[test]
    fn single_line_double_quoted_string() {
        let mut content =
            crate::content::Content::from(crate::Span::new(r#""`a few quoted words`""#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"&#8220;a few quoted words&#8221;"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn escaped_single_line_double_quoted_string() {
        let mut content =
            crate::content::Content::from(crate::Span::new(r#"\"`a few quoted words`""#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#""`a few quoted words`""#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn multi_line_double_quoted_string() {
        let mut content =
            crate::content::Content::from(crate::Span::new("\"`a few\nquoted words`\""));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(
                "&#8220;a few\nquoted words&#8221;"
                    .to_string()
                    .into_boxed_str()
            )
        );
    }

    #[test]
    fn double_quoted_string_with_inline_single_quote() {
        let mut content = crate::content::Content::from(crate::Span::new(r#""`Here's Johnny!`""#));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"&#8220;Here's Johnny!&#8221;"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn double_quoted_string_with_inline_backquote() {
        let mut content = crate::content::Content::from(crate::Span::new(r#""`Here`s Johnny!`""#));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"&#8220;Here`s Johnny!&#8221;"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn double_quoted_string_around_almost_monospaced_text() {
        let mut content =
            crate::content::Content::from(crate::Span::new(r#""``E=mc^2^` is the solution!`""#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(
                r#"&#8220;`E=mc<sup>2</sup>` is the solution!&#8221;"#
                    .to_string()
                    .into_boxed_str()
            )
        );
    }

    #[test]
    fn double_quoted_string_around_monospaced_text() {
        let mut content =
            crate::content::Content::from(crate::Span::new(r#""```E=mc^2^`` is the solution!`""#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(
                r#"&#8220;<code>E=mc<sup>2</sup></code> is the solution!&#8221;"#
                    .to_string()
                    .into_boxed_str()
            )
        );
    }

    #[test]
    fn single_line_single_quoted_string() {
        let mut content =
            crate::content::Content::from(crate::Span::new(r#"'`a few quoted words`'"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"&#8216;a few quoted words&#8217;"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn escaped_single_line_single_quoted_string() {
        let mut content =
            crate::content::Content::from(crate::Span::new(r#"\'`a few quoted words`'"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"'`a few quoted words`'"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn multi_line_single_quoted_string() {
        let mut content =
            crate::content::Content::from(crate::Span::new("'`a few\nquoted words`'"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(
                "&#8216;a few\nquoted words&#8217;"
                    .to_string()
                    .into_boxed_str()
            )
        );
    }

    #[test]
    fn single_quoted_string_with_inline_single_quote() {
        let mut content =
            crate::content::Content::from(crate::Span::new(r#"'`That isn't what I did.`'"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"&#8216;That isn't what I did.&#8217;"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn single_quoted_string_with_inline_backquote() {
        let mut content = crate::content::Content::from(crate::Span::new(r#"'`Here`s Johnny!`'"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"&#8216;Here`s Johnny!&#8217;"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn single_line_constrained_marked_string() {
        let mut content = crate::content::Content::from(crate::Span::new(r#"#a few words#"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"<mark>a few words</mark>"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn escaped_single_line_constrained_marked_string() {
        let mut content = crate::content::Content::from(crate::Span::new(r#"\#a few words#"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"#a few words#"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn multi_line_constrained_marked_string() {
        let mut content = crate::content::Content::from(crate::Span::new("#a few\nwords#"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed("<mark>a few\nwords</mark>".to_string().into_boxed_str())
        );
    }

    #[test]
    fn constrained_marked_string_should_not_match_entity_references() {
        let mut content = crate::content::Content::from(crate::Span::new(
            r##"111 #mark a# 222 "`quote a`" 333 #mark b# 444"##,
        ));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r##"111 <mark>mark a</mark> 222 &#8220;quote a&#8221; 333 <mark>mark b</mark> 444"##.to_string().into_boxed_str())
        );
    }

    #[test]
    fn single_line_unconstrained_marked_string() {
        let mut content =
            crate::content::Content::from(crate::Span::new(r###"##--anything goes ##"###));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(
                r###"<mark>--anything goes </mark>"###
                    .to_string()
                    .into_boxed_str()
            )
        );
    }

    #[test]
    fn escaped_single_line_unconstrained_marked_string() {
        let mut content =
            crate::content::Content::from(crate::Span::new(r###"\\##--anything goes ##"###));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r###"##--anything goes ##"###.to_string().into_boxed_str())
        );
    }

    #[test]
    fn multi_line_unconstrained_marked_string() {
        let mut content = crate::content::Content::from(crate::Span::new("##--anything\ngoes ##"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(
                "<mark>--anything\ngoes </mark>"
                    .to_string()
                    .into_boxed_str()
            )
        );
    }

    #[test]
    fn single_line_constrained_marked_string_with_role() {
        let mut content =
            crate::content::Content::from(crate::Span::new(r##"[statement]#a few words#"##));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(
                r#"<span class="statement">a few words</span>"#
                    .to_string()
                    .into_boxed_str()
            )
        );
    }

    #[test]
    fn does_not_recognize_attribute_list_with_left_square_bracket_on_formatted_text() {
        let mut content = crate::content::Content::from(crate::Span::new(
            r##"key: [ *before [.redacted]#redacted# after* ]"##,
        ));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(
                r#"key: [ <strong>before <span class="redacted">redacted</span> after</strong> ]"#
                    .to_string()
                    .into_boxed_str()
            )
        );
    }

    #[test]
    fn should_ignore_enclosing_square_brackets_when_processing_formatted_text_with_attribute() {
        let mut p = Parser::default();
        let maw =
            crate::blocks::Block::parse(crate::Span::new("nums = [1, 2, 3, [.blue]#4#]"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "nums = [1, 2, 3, [.blue]#4#]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"nums = [1, 2, 3, <span class="blue">4</span>]"#,
                },
                source: Span {
                    data: "nums = [1, 2, 3, [.blue]#4#]",
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
            },)
        );
    }

    #[test]
    fn single_line_constrained_strong_string() {
        let mut content =
            crate::content::Content::from(crate::Span::new(r#"*a few strong words*"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"<strong>a few strong words</strong>"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn escaped_single_line_constrained_strong_string() {
        let mut content =
            crate::content::Content::from(crate::Span::new(r#"\*a few strong words*"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"*a few strong words*"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn multi_line_constrained_strong_string() {
        let mut content = crate::content::Content::from(crate::Span::new("*a few\nstrong words*"));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(
                "<strong>a few\nstrong words</strong>"
                    .to_string()
                    .into_boxed_str()
            )
        );
    }

    #[test]
    fn constrained_strong_string_containing_an_asterisk() {
        let mut content = crate::content::Content::from(crate::Span::new("*bl*ck*-eye"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(
            content.rendered,
            CowStr::Boxed("<strong>bl*ck</strong>-eye".to_string().into_boxed_str())
        );
    }

    #[test]
    fn constrained_strong_string_containing_an_asterisk_and_multibyte_word_chars() {
        let mut content = crate::content::Content::from(crate::Span::new("*黑*眼圈*"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed("<strong>黑*眼圈</strong>".to_string().into_boxed_str())
        );
    }

    #[test]
    fn single_line_constrained_quote_variation_emphasized_string() {
        let mut content =
            crate::content::Content::from(crate::Span::new("_a few emphasized words_"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(
                "<em>a few emphasized words</em>"
                    .to_string()
                    .into_boxed_str()
            )
        );
    }

    #[test]
    fn escaped_single_line_constrained_quote_variation_emphasized_string() {
        let mut content =
            crate::content::Content::from(crate::Span::new("\\_a few emphasized words_"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed("_a few emphasized words_".to_string().into_boxed_str())
        );
    }

    #[test]
    fn escaped_single_quoted_string() {
        let mut content =
            crate::content::Content::from(crate::Span::new(r#"\'a few emphasized words'"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"\'a few emphasized words'"#)
        );
    }

    #[test]
    fn multi_line_constrained_emphasized_quote_variation_string() {
        let mut content =
            crate::content::Content::from(crate::Span::new("_a few\nemphasized words_"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed("<em>a few\nemphasized words</em>")
        );
    }

    #[test]
    fn single_quoted_string_containing_an_emphasized_phrase() {
        let mut content =
            crate::content::Content::from(crate::Span::new(r#"'`I told him, 'Just go for it!'`'"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"&#8216;I told him, 'Just go for it!'&#8217;"#)
        );
    }

    #[test]
    fn escaped_single_quotes_inside_emphasized_words_are_restored() {
        let mut content = crate::content::Content::from(crate::Span::new(r#"'Here\'s Johnny!'"#));
        let p = Parser::default();
        SubstitutionGroup::Normal.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed(r#"'Here's Johnny!'"#));
    }

    #[test]
    fn single_line_constrained_emphasized_underline_variation_string() {
        let mut content =
            crate::content::Content::from(crate::Span::new(r#"_a few emphasized words_"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"<em>a few emphasized words</em>"#)
        );
    }

    #[test]
    fn escaped_single_line_constrained_emphasized_underline_variation_string() {
        let mut content =
            crate::content::Content::from(crate::Span::new(r#"\_a few emphasized words_"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"_a few emphasized words_"#)
        );
    }

    #[test]
    fn multi_line_constrained_emphasized_underline_variation_string() {
        let mut content =
            crate::content::Content::from(crate::Span::new("_a few\nemphasized words_"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed("<em>a few\nemphasized words</em>")
        );
    }

    #[test]
    fn should_ignore_role_that_ends_with_transitional_role_on_constrained_monospace_span() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new("[foox-]`leave it alone`"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "[foox-]`leave it alone`",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<code class="foox-">leave it alone</code>"#,
                },
                source: Span {
                    data: "[foox-]`leave it alone`",
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
            },)
        );
    }

    #[test]
    fn escaped_single_line_constrained_monospace_string_with_forced_compat_role() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new(r#"[x-]\`leave it alone`"#), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"[x-]\`leave it alone`"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "[x-]`leave it alone`",
                },
                source: Span {
                    data: r#"[x-]\`leave it alone`"#,
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
            },)
        );
    }

    #[test]
    fn escaped_forced_compat_role_on_single_line_constrained_monospace_string() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new(r#"\[x-]`just *mono*`"#), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"\[x-]`just *mono*`"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "[x-]<code>just <strong>mono</strong></code>",
                },
                source: Span {
                    data: r#"\[x-]`just *mono*`"#,
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
            },)
        );
    }

    #[test]
    fn multi_line_constrained_monospaced_string() {
        let mut p = Parser::default().with_intrinsic_attribute(
            "monospaced",
            "monospaced",
            ModificationContext::Anywhere,
        );

        let maw =
            crate::blocks::Block::parse(crate::Span::new("`a few\n<{monospaced}> words`"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "`a few\n<{monospaced}> words`",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "<code>a few\n&lt;monospaced&gt; words</code>",
                },
                source: Span {
                    data: "`a few\n<{monospaced}> words`",
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
            },)
        );
    }

    #[test]
    fn single_line_unconstrained_strong_chars() {
        let mut content = crate::content::Content::from(crate::Span::new(r#"**Git**Hub"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed("<strong>Git</strong>Hub")
        );
    }

    #[test]
    fn escaped_single_line_unconstrained_strong_chars() {
        let mut content = crate::content::Content::from(crate::Span::new(r#"\**Git**Hub"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed("<strong>*Git</strong>*Hub")
        );
    }

    #[test]
    fn multi_line_unconstrained_strong_chars() {
        let mut content = crate::content::Content::from(crate::Span::new("**G\ni\nt\n**Hub"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed("<strong>G\ni\nt\n</strong>Hub")
        );
    }

    #[test]
    fn unconstrained_strong_chars_with_inline_asterisk() {
        let mut content = crate::content::Content::from(crate::Span::new("**bl*ck**-eye"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed("<strong>bl*ck</strong>-eye")
        );
    }

    #[test]
    fn unconstrained_strong_chars_with_role() {
        let mut content = crate::content::Content::from(crate::Span::new("Git[blue]**Hub**"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"Git<strong class="blue">Hub</strong>"#)
        );
    }

    #[test]
    fn escaped_unconstrained_strong_chars_with_role() {
        let mut content = crate::content::Content::from(crate::Span::new(r#"Git\[blue]**Hub**"#));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"Git[blue]<strong>*Hub</strong>*"#)
        );
    }

    #[test]
    fn single_line_unconstrained_emphasized_characters() {
        let mut content = crate::content::Content::from(crate::Span::new("__Git__Hub"));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed("<em>Git</em>Hub"));
    }

    #[test]
    fn escaped_single_line_unconstrained_emphasized_characters() {
        let mut content = crate::content::Content::from(crate::Span::new(r#"\__Git__Hub"#));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed("__Git__Hub"));
    }

    #[test]
    fn escaped_single_line_unconstrained_emphasized_characters_around_word() {
        let mut content = crate::content::Content::from(crate::Span::new(r#"\\__GitHub__"#));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed("__GitHub__"));
    }

    #[test]
    fn multi_line_unconstrained_emphasized_chars() {
        let mut content = crate::content::Content::from(crate::Span::new("__G\ni\nt\n__Hub"));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed("<em>G\ni\nt\n</em>Hub"));
    }

    #[test]
    fn unconstrained_emphasis_chars_with_role() {
        let mut content = crate::content::Content::from(crate::Span::new("[gray]__Git__Hub"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"<em class="gray">Git</em>Hub"#)
        );
    }

    #[test]
    fn escaped_unconstrained_emphasis_chars_with_role() {
        let mut content = crate::content::Content::from(crate::Span::new("\\[gray]__Git__Hub"));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed(r#"[gray]__Git__Hub"#));
    }

    #[test]
    fn single_line_constrained_monospaced_chars_1() {
        let mut content = crate::content::Content::from(crate::Span::new(
            "call [x-]+save()+ to persist the changes",
        ));

        let p = Parser::default();
        SubstitutionGroup::Normal.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"call <code>save()</code> to persist the changes"#)
        );
    }

    #[test]
    fn single_line_constrained_monospaced_chars_2() {
        let mut content =
            crate::content::Content::from(crate::Span::new("call `save()` to persist the changes"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"call <code>save()</code> to persist the changes"#)
        );
    }

    #[test]
    fn single_line_constrained_monospaced_chars_with_role_1() {
        let mut content = crate::content::Content::from(crate::Span::new(
            "call [method x-]+save()+ to persist the changes",
        ));

        let p = Parser::default();
        SubstitutionGroup::Normal.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"call <code class="method">save()</code> to persist the changes"#)
        );
    }

    #[test]
    fn single_line_constrained_monospaced_chars_with_role_2() {
        let mut content = crate::content::Content::from(crate::Span::new(
            "call [method]`save()` to persist the changes",
        ));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"call <code class="method">save()</code> to persist the changes"#)
        );
    }

    #[test]
    fn escaped_single_line_constrained_monospaced_chars() {
        let mut content = crate::content::Content::from(crate::Span::new(
            r#"call \`save()` to persist the changes"#,
        ));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"call `save()` to persist the changes"#)
        );
    }

    #[test]
    fn escaped_single_line_constrained_monospaced_chars_with_role() {
        let mut content = crate::content::Content::from(crate::Span::new(
            r#"call [method]\`save()` to persist the changes"#,
        ));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"call [method]`save()` to persist the changes"#)
        );
    }

    #[test]
    fn escaped_role_on_single_line_constrained_monospaced_chars() {
        let mut content = crate::content::Content::from(crate::Span::new(
            r#"call \[method]`save()` to persist the changes"#,
        ));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"call [method]<code>save()</code> to persist the changes"#)
        );
    }

    #[test]
    fn escaped_role_on_escaped_single_line_constrained_monospaced_chars() {
        let mut content = crate::content::Content::from(crate::Span::new(
            r#"call \[method]\`save()` to persist the changes"#,
        ));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"call \[method]`save()` to persist the changes"#)
        );
    }

    #[test]
    fn escaped_single_line_constrained_passthrough_string() {
        let mut content =
            crate::content::Content::from(crate::Span::new(r#"[x-]\+leave it alone+"#));

        let p = Parser::default();
        SubstitutionGroup::Normal.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"[x-]+leave it alone+"#)
        );
    }

    #[test]
    fn single_line_unconstrained_monospaced_chars_with_old_behavior_and_role() {
        // NOTE: Not in the Ruby test suite.
        let mut content = crate::content::Content::from(crate::Span::new("Git[test x-]++Hub++"));

        let p = Parser::default();
        SubstitutionGroup::Normal.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"Git<code class="test">Hub</code>"#)
        );
    }

    #[test]
    fn single_line_unconstrained_monospaced_chars_1() {
        let mut content = crate::content::Content::from(crate::Span::new("Git[x-]++Hub++"));
        let p = Parser::default();
        SubstitutionGroup::Normal.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed(r#"Git<code>Hub</code>"#));
    }

    #[test]
    fn single_line_unconstrained_monospaced_chars_2() {
        let mut content = crate::content::Content::from(crate::Span::new("Git``Hub``"));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed(r#"Git<code>Hub</code>"#));
    }

    #[test]
    fn escaped_single_line_unconstrained_monospaced_chars() {
        let mut content = crate::content::Content::from(crate::Span::new(r#"Git\``Hub``"#));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed(r#"Git``Hub``"#));
    }

    #[test]
    fn multi_line_unconstrained_monospaced_chars_1() {
        let mut content = crate::content::Content::from(crate::Span::new("Git[x-]++\nH\nu\nb++"));

        let p = Parser::default();
        SubstitutionGroup::Normal.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed("Git<code>\nH\nu\nb</code>")
        );
    }

    #[test]
    fn multi_line_unconstrained_monospaced_chars_2() {
        let mut content = crate::content::Content::from(crate::Span::new("Git``\nH\nu\nb``"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed("Git<code>\nH\nu\nb</code>")
        );
    }

    #[test]
    fn single_line_superscript_chars() {
        let mut content = crate::content::Content::from(crate::Span::new(
            "x^2^ = x * x, e = mc^2^, there's a 1^st^ time for everything",
        ));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(
                "x<sup>2</sup> = x * x, e = mc<sup>2</sup>, there\'s a 1<sup>st</sup> time for everything"
            )
        );
    }

    #[test]
    fn escaped_single_line_superscript_chars() {
        let mut content = crate::content::Content::from(crate::Span::new(r#"x\^2^ = x * x"#));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed("x^2^ = x * x"));
    }

    #[test]
    fn does_not_match_superscript_across_whitespace() {
        let mut content = crate::content::Content::from(crate::Span::new("x^(n\n-\n1)^"));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed("x^(n\n-\n1)^"));
    }

    #[test]
    fn allow_spaces_in_superscript_if_spaces_are_inserted_using_an_attribute_reference() {
        let mut content = crate::content::Content::from(crate::Span::new(
            "Night ^A{sp}poem{sp}by{sp}Jane{sp}Kondo^.",
        ));

        let p = Parser::default();

        SubstitutionGroup::Normal.apply(&mut content, &p, None);

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"Night <sup>A poem by Jane Kondo</sup>."#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn allow_spaces_in_superscript_if_text_is_wrapped_in_a_passthrough() {
        let mut content =
            crate::content::Content::from(crate::Span::new("Night ^+A poem by Jane Kondo+^."));

        let p = Parser::default();
        SubstitutionGroup::Normal.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed("Night <sup>A poem by Jane Kondo</sup>.")
        );
    }

    #[test]
    fn does_not_match_adjacent_superscript_chars() {
        let mut content = crate::content::Content::from(crate::Span::new("a ^^ b"));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed("a ^^ b"));
    }

    #[ignore]
    #[test]
    fn does_not_confuse_superscript_and_links_with_blank_window_shorthand() {
        // TO DO: Enable when macro substitution is implemented.
        let mut content = crate::content::Content::from(crate::Span::new(
            "http://localhost[Text^] on the 21^st^ and 22^nd^",
        ));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        // ^^^ TO DO: This needs to be the full substitution group, not just the Quotes
        // substition.

        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(
                r#"<a href="http://localhost" target="_blank" rel="noopener">Text</a> on the 21<sup>st</sup> and 22<sup>nd</sup>"#
            )
        );
    }

    #[test]
    fn single_line_subscript_chars() {
        let mut content = crate::content::Content::from(crate::Span::new("H~2~O"));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed("H<sub>2</sub>O"));
    }

    #[test]
    fn escaped_single_line_subscript_chars() {
        let mut content = crate::content::Content::from(crate::Span::new(r#"H\~2~O"#));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed("H~2~O"));
    }

    #[test]
    fn does_not_match_subscript_across_whitespace() {
        let mut content =
            crate::content::Content::from(crate::Span::new("project~ view\non\nGitHub~"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed("project~ view\non\nGitHub~")
        );
    }

    #[test]
    fn does_not_match_adjacent_subscript_chars() {
        let mut content = crate::content::Content::from(crate::Span::new("a ~~ b"));
        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());
        assert_eq!(content.rendered, CowStr::Borrowed("a ~~ b"));
    }

    #[test]
    fn does_not_match_subscript_across_distinct_urls() {
        let mut content = crate::content::Content::from(crate::Span::new(
            "http://www.abc.com/~def[DEF] and http://www.abc.com/~ghi[GHI]",
        ));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed("http://www.abc.com/~def[DEF] and http://www.abc.com/~ghi[GHI]")
        );
    }

    #[test]
    fn quoted_text_with_role_shorthand() {
        let mut content =
            crate::content::Content::from(crate::Span::new("[.white.red-background]#alert#"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"<span class="white red-background">alert</span>"#)
        );
    }

    #[test]
    fn quoted_text_with_id_shorthand() {
        let mut content = crate::content::Content::from(crate::Span::new("[#bond]#007#"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"<span id="bond">007</span>"#)
        );
    }

    #[test]
    fn quoted_text_with_id_and_role_shorthand() {
        let mut content =
            crate::content::Content::from(crate::Span::new("[#bond.white.red-background]#007#"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"<span id="bond" class="white red-background">007</span>"#)
        );
    }

    #[test]
    fn quoted_text_with_id_and_role_shorthand_with_roles_before_id() {
        let mut content =
            crate::content::Content::from(crate::Span::new("[.white.red-background#bond]#007#"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"<span id="bond" class="white red-background">007</span>"#)
        );
    }

    #[test]
    fn quoted_text_with_id_and_role_shorthand_with_roles_around_id() {
        let mut content =
            crate::content::Content::from(crate::Span::new("[.white#bond.red-background]#007#"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"<span id="bond" class="white red-background">007</span>"#)
        );
    }

    #[test]
    fn should_not_assign_role_attribute_if_shorthand_style_has_no_roles() {
        let mut content = crate::content::Content::from(crate::Span::new("[#idname]*blah*"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Borrowed(r#"<strong id="idname">blah</strong>"#)
        );
    }

    #[test]
    fn should_remove_trailing_spaces_from_role_defined_using_shorthand() {
        let mut content = crate::content::Content::from(crate::Span::new("[.rolename ]*blah*"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"<strong class="rolename">blah</strong>"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn should_allow_role_to_be_defined_using_attribute_reference() {
        let mut content = crate::content::Content::from(crate::Span::new("[{rolename}]#phrase#"));

        let p = Parser::default().with_intrinsic_attribute(
            "rolename",
            "red",
            ModificationContext::Anywhere,
        );

        SubstitutionGroup::Normal.apply(&mut content, &p, None);

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"<span class="red">phrase</span>"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn should_ignore_attributes_after_comma() {
        let mut content = crate::content::Content::from(crate::Span::new("[red, foobar]#alert#"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"<span class="red">alert</span>"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn should_remove_leading_and_trailing_spaces_around_role_after_ignoring_attributes_after_comma()
    {
        let mut content = crate::content::Content::from(crate::Span::new("[ red , foobar]#alert#"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(r#"<span class="red">alert</span>"#.to_string().into_boxed_str())
        );
    }

    #[test]
    fn should_not_assign_role_if_value_before_comma_is_empty() {
        let mut content = crate::content::Content::from(crate::Span::new("[,]#anonymous#"));

        let p = Parser::default();
        SubstitutionStep::Quotes.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed("anonymous".to_string().into_boxed_str())
        );
    }

    #[test]
    fn inline_passthrough_with_id_and_role_set_using_shorthand_1() {
        let mut content =
            crate::content::Content::from(crate::Span::new("[#idname.rolename]+pass+"));

        let p = Parser::default();
        SubstitutionGroup::Normal.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(
                r#"<span id="idname" class="rolename">pass</span>"#
                    .to_string()
                    .into_boxed_str()
            )
        );
    }

    #[test]
    fn inline_passthrough_with_id_and_role_set_using_shorthand_2() {
        let mut content =
            crate::content::Content::from(crate::Span::new("[.rolename#idname]+pass+"));

        let p = Parser::default();
        SubstitutionGroup::Normal.apply(&mut content, &p, None);
        assert!(!content.is_empty());

        assert_eq!(
            content.rendered,
            CowStr::Boxed(
                r#"<span id="idname" class="rolename">pass</span>"#
                    .to_string()
                    .into_boxed_str()
            )
        );
    }
}

mod macros {
    use pretty_assertions_sorted::assert_eq;

    use crate::{
        Parser, blocks::SimpleBlockStyle, content::SubstitutionStep, parser::ModificationContext,
        strings::CowStr, tests::prelude::*,
    };

    #[test]
    fn a_single_line_link_macro_should_be_interpreted_as_a_link() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new("link:/home.html[]"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "link:/home.html[]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="/home.html" class="bare">/home.html</a>"#,
                },
                source: Span {
                    data: "link:/home.html[]",
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
            },)
        );
    }

    #[test]
    fn a_single_line_link_macro_with_text_should_be_interpreted_as_a_link() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new("link:/home.html[Home]"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "link:/home.html[Home]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="/home.html">Home</a>"#,
                },
                source: Span {
                    data: "link:/home.html[Home]",
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
            },)
        );
    }

    #[test]
    fn a_mailto_macro_should_be_interpreted_as_a_mailto_link() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("mailto:doc.writer@asciidoc.org[]"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "mailto:doc.writer@asciidoc.org[]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="mailto:doc.writer@asciidoc.org">doc.writer@asciidoc.org</a>"#,
                },
                source: Span {
                    data: "mailto:doc.writer@asciidoc.org[]",
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
            },)
        );
    }

    #[test]
    fn a_mailto_macro_with_text_should_be_interpreted_as_a_mailto_link() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("mailto:doc.writer@asciidoc.org[Doc Writer]"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "mailto:doc.writer@asciidoc.org[Doc Writer]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="mailto:doc.writer@asciidoc.org">Doc Writer</a>"#,
                },
                source: Span {
                    data: "mailto:doc.writer@asciidoc.org[Doc Writer]",
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
            },)
        );
    }

    #[test]
    fn a_mailto_macro_with_text_and_subject_should_be_interpreted_as_a_mailto_link() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("mailto:doc.writer@asciidoc.org[Doc Writer, Pull request]"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "mailto:doc.writer@asciidoc.org[Doc Writer, Pull request]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="mailto:doc.writer@asciidoc.org?subject=Pull%20request">Doc Writer</a>"#,
                },
                source: Span {
                    data: "mailto:doc.writer@asciidoc.org[Doc Writer, Pull request]",
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
            },)
        );
    }

    #[test]
    fn a_mailto_macro_with_text_subject_and_body_should_be_interpreted_as_a_mailto_link() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new(
                "mailto:doc.writer@asciidoc.org[Doc Writer, Pull request, Please accept my pull request]",
            ),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "mailto:doc.writer@asciidoc.org[Doc Writer, Pull request, Please accept my pull request]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="mailto:doc.writer@asciidoc.org?subject=Pull%20request&amp;body=Please%20accept%20my%20pull%20request">Doc Writer</a>"#,
                },
                source: Span {
                    data: "mailto:doc.writer@asciidoc.org[Doc Writer, Pull request, Please accept my pull request]",
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
            },)
        );
    }

    #[test]
    fn a_mailto_macro_with_subject_and_body_only_should_use_e_mail_as_text() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new(
                "mailto:doc.writer@asciidoc.org[,Pull request,Please accept my pull request]",
            ),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "mailto:doc.writer@asciidoc.org[,Pull request,Please accept my pull request]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="mailto:doc.writer@asciidoc.org?subject=Pull%20request&amp;body=Please%20accept%20my%20pull%20request">doc.writer@asciidoc.org</a>"#,
                },
                source: Span {
                    data: "mailto:doc.writer@asciidoc.org[,Pull request,Please accept my pull request]",
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
            },)
        );
    }

    #[test]
    fn a_mailto_macro_supports_id_and_role_attributes() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("mailto:doc.writer@asciidoc.org[,id=contact,role=icon]"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "mailto:doc.writer@asciidoc.org[,id=contact,role=icon]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="mailto:doc.writer@asciidoc.org" id="contact" class="icon">doc.writer@asciidoc.org</a>"#,
                },
                source: Span {
                    data: "mailto:doc.writer@asciidoc.org[,id=contact,role=icon]",
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
            },)
        );
    }

    const EMAIL_ADDRESSES: &[(&str, &str)] = &[
        (
            "doc.writer@asciidoc.org",
            r#"<a href="mailto:doc.writer@asciidoc.org">doc.writer@asciidoc.org</a>"#,
        ),
        (
            "author+website@4fs.no",
            r#"<a href="mailto:author+website@4fs.no">author+website@4fs.no</a>"#,
        ),
        (
            "john@domain.uk.co",
            r#"<a href="mailto:john@domain.uk.co">john@domain.uk.co</a>"#,
        ),
        (
            "name@somewhere.else.com",
            r#"<a href="mailto:name@somewhere.else.com">name@somewhere.else.com</a>"#,
        ),
        (
            "joe_bloggs@mail_server.com",
            r#"<a href="mailto:joe_bloggs@mail_server.com">joe_bloggs@mail_server.com</a>"#,
        ),
        (
            "joe-bloggs@mail-server.com",
            r#"<a href="mailto:joe-bloggs@mail-server.com">joe-bloggs@mail-server.com</a>"#,
        ),
        (
            "joe.bloggs@mail.server.com",
            r#"<a href="mailto:joe.bloggs@mail.server.com">joe.bloggs@mail.server.com</a>"#,
        ),
        (
            "FOO@BAR.COM",
            r#"<a href="mailto:FOO@BAR.COM">FOO@BAR.COM</a>"#,
        ),
        (
            "docs@writing.ninja",
            r#"<a href="mailto:docs@writing.ninja">docs@writing.ninja</a>"#,
        ),
    ];

    #[test]
    fn should_recognize_inline_email_addresses() {
        for (input, expected) in EMAIL_ADDRESSES {
            let mut p = Parser::default();
            let maw = crate::blocks::Block::parse(crate::Span::new(input), &mut p);

            let block = maw.item.unwrap().item;

            assert_eq!(
                block,
                Block::Simple(SimpleBlock {
                    content: Content {
                        original: Span {
                            data: input,
                            line: 1,
                            col: 1,
                            offset: 0,
                        },
                        rendered: expected,
                    },
                    source: Span {
                        data: input,
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
                },)
            );
        }
    }

    #[test]
    fn should_recognize_inline_email_address_containing_an_ampersand() {
        let mut p = Parser::default();
        let maw =
            crate::blocks::Block::parse(crate::Span::new("bert&ernie@sesamestreet.com"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "bert&ernie@sesamestreet.com",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="mailto:bert&amp;ernie@sesamestreet.com">bert&amp;ernie@sesamestreet.com</a>"#,
                },
                source: Span {
                    data: "bert&ernie@sesamestreet.com",
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
            },)
        );
    }

    #[test]
    fn should_recognize_inline_email_address_surrounded_by_angle_brackets() {
        let mut p = Parser::default();
        let maw =
            crate::blocks::Block::parse(crate::Span::new("<doc.writer@asciidoc.org>"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "<doc.writer@asciidoc.org>",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"&lt;<a href="mailto:doc.writer@asciidoc.org">doc.writer@asciidoc.org</a>&gt;"#,
                },
                source: Span {
                    data: "<doc.writer@asciidoc.org>",
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
            },)
        );
    }

    #[test]
    fn should_ignore_escaped_inline_email_address() {
        let mut p = Parser::default();
        let maw =
            crate::blocks::Block::parse(crate::Span::new("\\doc.writer@asciidoc.org"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "\\doc.writer@asciidoc.org",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"doc.writer@asciidoc.org"#,
                },
                source: Span {
                    data: "\\doc.writer@asciidoc.org",
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
            },)
        );
    }

    #[test]
    fn a_single_line_raw_url_should_be_interpreted_as_a_link() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new("http://google.com"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "http://google.com",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="http://google.com" class="bare">http://google.com</a>"#,
                },
                source: Span {
                    data: "http://google.com",
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
            },)
        );
    }

    #[test]
    fn a_single_line_raw_url_with_text_should_be_interpreted_as_a_link() {
        let mut p = Parser::default();
        let maw =
            crate::blocks::Block::parse(crate::Span::new("http://google.com[Google]"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "http://google.com[Google]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="http://google.com">Google</a>"#,
                },
                source: Span {
                    data: "http://google.com[Google]",
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
            },)
        );
    }

    #[test]
    fn a_multi_line_raw_url_with_text_should_be_interpreted_as_a_link() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("http://google.com[Google\nHomepage]"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "http://google.com[Google\nHomepage]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "<a href=\"http://google.com\">Google\nHomepage</a>",
                },
                source: Span {
                    data: "http://google.com[Google\nHomepage]",
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
            },)
        );
    }

    #[test]
    fn a_single_line_raw_url_with_attribute_as_text_should_be_interpreted_as_a_link_with_resolved_attribute()
     {
        let mut p = Parser::default().with_intrinsic_attribute(
            "google_homepage",
            "Google Homepage",
            ModificationContext::Anywhere,
        );

        let maw = crate::blocks::Block::parse(
            crate::Span::new("http://google.com[{google_homepage}]"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "http://google.com[{google_homepage}]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="http://google.com">Google Homepage</a>"#,
                },
                source: Span {
                    data: "http://google.com[{google_homepage}]",
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
            },)
        );
    }

    #[test]
    fn should_not_resolve_an_escaped_attribute_in_link_text_1() {
        let mut p = Parser::default().with_intrinsic_attribute(
            "google_homepage",
            "Google Homepage",
            ModificationContext::Anywhere,
        );

        let maw = crate::blocks::Block::parse(
            crate::Span::new("http://google.com[\\{google_homepage}]"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "http://google.com[\\{google_homepage}]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="http://google.com">{google_homepage}</a>"#,
                },
                source: Span {
                    data: "http://google.com[\\{google_homepage}]",
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
            },)
        );
    }

    #[test]
    fn should_not_resolve_an_escaped_attribute_in_link_text_2() {
        let mut p = Parser::default().with_intrinsic_attribute(
            "google_homepage",
            "Google Homepage",
            ModificationContext::Anywhere,
        );

        let maw = crate::blocks::Block::parse(
            crate::Span::new("http://google.com?q=,[\\{google_homepage}]"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "http://google.com?q=,[\\{google_homepage}]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="http://google.com?q=,">{google_homepage}</a>"#,
                },
                source: Span {
                    data: "http://google.com?q=,[\\{google_homepage}]",
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
            },)
        );
    }

    #[test]
    fn a_single_line_escaped_raw_url_should_not_be_interpreted_as_a_link() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new("\\http://google.com"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "\\http://google.com",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"http://google.com"#,
                },
                source: Span {
                    data: "\\http://google.com",
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
            },)
        );
    }

    #[test]
    fn a_comma_separated_list_of_links_should_not_include_commas_in_links() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("http://foo.com, http://bar.com, http://example.org"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "http://foo.com, http://bar.com, http://example.org",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<a href="http://foo.com" class="bare">http://foo.com</a>, <a href="http://bar.com" class="bare">http://bar.com</a>, <a href="http://example.org" class="bare">http://example.org</a>"#,
                },
                source: Span {
                    data: "http://foo.com, http://bar.com, http://example.org",
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
            },)
        );
    }

    #[test]
    fn a_single_line_image_macro_should_be_interpreted_as_an_image() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new("image:tiger.png[]"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "image:tiger.png[]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<span class="image"><img src="tiger.png" alt="tiger"></span>"#,
                },
                source: Span {
                    data: "image:tiger.png[]",
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
            },)
        );
    }

    #[test]
    fn should_replace_underscore_and_hyphen_with_space_in_generated_alt_text_for_an_inline_image() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("image:tiger-with-family_1.png[]"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "image:tiger-with-family_1.png[]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<span class="image"><img src="tiger-with-family_1.png" alt="tiger with family 1"></span>"#,
                },
                source: Span {
                    data: "image:tiger-with-family_1.png[]",
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
            },)
        );
    }

    #[test]
    fn a_single_line_image_macro_with_text_should_be_interpreted_as_an_image_with_alt_text() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new("image:tiger.png[Tiger]"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "image:tiger.png[Tiger]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<span class="image"><img src="tiger.png" alt="Tiger"></span>"#,
                },
                source: Span {
                    data: "image:tiger.png[Tiger]",
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
            },)
        );
    }

    #[test]
    fn should_encode_special_characters_in_alt_text_of_inline_image() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new(r#"image:tiger-roar.png[A tiger's "roar" is < a bear's "growl"]"#),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"image:tiger-roar.png[A tiger's "roar" is < a bear's "growl"]"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<span class="image"><img src="tiger-roar.png" alt="A tiger&#8217;s &quot;roar&quot; is &lt; a bear&#8217;s &quot;growl&quot;"></span>"#,
                },
                source: Span {
                    data: r#"image:tiger-roar.png[A tiger's "roar" is < a bear's "growl"]"#,
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
            },)
        );
    }

    #[test]
    fn an_image_macro_with_svg_image_and_text_should_be_interpreted_as_an_image_with_alt_text() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new("image:tiger.svg[Tiger]"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "image:tiger.svg[Tiger]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<span class="image"><img src="tiger.svg" alt="Tiger"></span>"#,
                },
                source: Span {
                    data: "image:tiger.svg[Tiger]",
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
            },)
        );
    }

    #[ignore]
    #[test]
    fn todo_issue_272_inline_interactive() {
        // TO DO: Implement `inline` and `interactive` options for SVG images.
        // https://github.com/asciidoc-rs/asciidoc-parser/issues/272
        todo!(
            "{}",
            r###"
        test 'an image macro with an interactive SVG image and alt text should be converted to an object element' do
            para = block_from_string 'image:tiger.svg[Tiger,opts=interactive]', safe: Asciidoctor::SafeMode::SERVER, attributes: { 'imagesdir' => 'images' }
            assert_equal '<span class="image"><object type="image/svg+xml" data="images/tiger.svg"><span class="alt">Tiger</span></object></span>', para.sub_macros(para.source).gsub(/>\s+</, '><')
        end

        test 'an image macro with an interactive SVG image, fallback and alt text should be converted to an object element' do
            para = block_from_string 'image:tiger.svg[Tiger,fallback=tiger.png,opts=interactive]', safe: Asciidoctor::SafeMode::SERVER, attributes: { 'imagesdir' => 'images' }
            assert_equal '<span class="image"><object type="image/svg+xml" data="images/tiger.svg"><img src="images/tiger.png" alt="Tiger"></object></span>', para.sub_macros(para.source).gsub(/>\s+</, '><')
        end

        test 'an image macro with an inline SVG image should be converted to an svg element' do
            para = block_from_string 'image:circle.svg[Tiger,100,opts=inline]', safe: Asciidoctor::SafeMode::SERVER, attributes: { 'imagesdir' => 'fixtures', 'docdir' => testdir }
            result = para.sub_macros(para.source).gsub(/>\s+</, '><')
            assert_match(/<svg\s[^>]*width="100"[^>]*>/, result)
            refute_match(/<svg\s[^>]*width="500"[^>]*>/, result)
            refute_match(/<svg\s[^>]*height="500"[^>]*>/, result)
            refute_match(/<svg\s[^>]*style="[^>]*>/, result)
        end

        test 'should ignore link attribute if value is self and image target is inline SVG' do
            para = block_from_string 'image:circle.svg[Tiger,100,opts=inline,link=self]', safe: Asciidoctor::SafeMode::SERVER, attributes: { 'imagesdir' => 'fixtures', 'docdir' => testdir }
            result = para.sub_macros(para.source).gsub(/>\s+</, '><')
            assert_match(/<svg\s[^>]*width="100"[^>]*>/, result)
            refute_match(/<a href=/, result)
        end

        test 'an image macro with an inline SVG image should be converted to an svg element even when data-uri is set' do
            para = block_from_string 'image:circle.svg[Tiger,100,opts=inline]', safe: Asciidoctor::SafeMode::SERVER, attributes: { 'data-uri' => '', 'imagesdir' => 'fixtures', 'docdir' => testdir }
            assert_match(/<svg\s[^>]*width="100">/, para.sub_macros(para.source).gsub(/>\s+</, '><'))
        end

        test 'an image macro with an SVG image should not use an object element when safe mode is secure' do
            para = block_from_string 'image:tiger.svg[Tiger,opts=interactive]', attributes: { 'imagesdir' => 'images' }
            assert_equal '<span class="image"><img src="images/tiger.svg" alt="Tiger"></span>', para.sub_macros(para.source).gsub(/>\s+</, '><')
        end
        "###
        );
    }

    #[test]
    fn a_single_line_image_macro_with_text_containing_escaped_square_bracket_should_be_interpreted_as_an_image_with_alt_text()
     {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new(r#"image:tiger.png[[Another\] Tiger]"#),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"image:tiger.png[[Another\] Tiger]"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<span class="image"><img src="tiger.png" alt="[Another] Tiger"></span>"#,
                },
                source: Span {
                    data: r#"image:tiger.png[[Another\] Tiger]"#,
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
            },)
        );
    }

    #[test]
    fn an_escaped_image_macro_should_not_be_interpreted_as_an_image() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new(r#"\image:tiger.png[]"#), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"\image:tiger.png[]"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"image:tiger.png[]"#,
                },
                source: Span {
                    data: r#"\image:tiger.png[]"#,
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
            },)
        );
    }

    #[test]
    fn a_single_line_image_macro_with_text_and_dimensions_should_be_interpreted_as_an_image_with_alt_text_and_dimensions()
     {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new(r#"image:tiger.png[Tiger, 200, 100]"#),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"image:tiger.png[Tiger, 200, 100]"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<span class="image"><img src="tiger.png" alt="Tiger" width="200" height="100"></span>"#,
                },
                source: Span {
                    data: r#"image:tiger.png[Tiger, 200, 100]"#,
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
            },)
        );
    }

    #[ignore]
    #[test]
    fn docbook_support_for_image() {
        // The following tests from Ruby have not been ported because Docbook is
        // not in scope for this crate.
        //
        // * test 'a single-line image macro with text and dimensions should be
        //   interpreted as an image with alt text and dimensions in docbook'
        // * test 'a single-line image macro with scaledwidth attribute should
        //   be supported in docbook'
        // * test 'a single-line image macro with scaled attribute should be
        //   supported in docbook'
        // * test 'should pass through role on image macro to DocBook output'
    }

    #[test]
    fn a_single_line_image_macro_with_text_and_link_should_be_interpreted_as_a_linked_image_with_alt_text()
     {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new(
                r#"image:tiger.png[Tiger, link="http://en.wikipedia.org/wiki/Tiger"]"#,
            ),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"image:tiger.png[Tiger, link="http://en.wikipedia.org/wiki/Tiger"]"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<span class="image"><a class="image" href="http://en.wikipedia.org/wiki/Tiger"><img src="tiger.png" alt="Tiger"></a></span>"#,
                },
                source: Span {
                    data: r#"image:tiger.png[Tiger, link="http://en.wikipedia.org/wiki/Tiger"]"#,
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
            },)
        );
    }

    #[test]
    fn a_single_line_image_macro_with_text_and_link_to_self_should_be_interpreted_as_a_self_referencing_image_with_alt_text()
     {
        let mut p = Parser::default().with_intrinsic_attribute(
            "imagesdir",
            "img",
            ModificationContext::Anywhere,
        );

        let maw = crate::blocks::Block::parse(
            crate::Span::new(r#"image:tiger.png[Tiger, link=self]"#),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"image:tiger.png[Tiger, link=self]"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<span class="image"><a class="image" href="img/tiger.png"><img src="img/tiger.png" alt="Tiger"></a></span>"#,
                },
                source: Span {
                    data: r#"image:tiger.png[Tiger, link=self]"#,
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
            },)
        );
    }

    #[ignore]
    #[test]
    fn should_link_to_data_uri_if_value_of_link_attribute_is_self_and_inline_image_is_embedded() {
        todo!(
            "Port when server safe modes are implemented: {}",
            r###"
        test 'should link to data URI if value of link attribute is self and inline image is embedded' do
            para = block_from_string 'image:circle.svg[Tiger,100,link=self]', safe: Asciidoctor::SafeMode::SERVER, attributes: { 'data-uri' => '', 'imagesdir' => 'fixtures', 'docdir' => testdir }
            output = para.sub_macros(para.source).gsub(/>\s+</, '><')
            assert_xpath '//a[starts-with(@href,"data:image/svg+xml;base64,")]', output, 1
            assert_xpath '//img[starts-with(@src,"data:image/svg+xml;base64,")]', output, 1
        end
            "###
        );
    }

    #[test]
    fn rel_noopener_should_be_added_to_an_image_with_a_link_that_targets_the_blank_window() {
        let mut p = Parser::default();

        let maw = crate::blocks::Block::parse(
            crate::Span::new(
                r#"image:tiger.png[Tiger,link=http://en.wikipedia.org/wiki/Tiger,window=_blank]"#,
            ),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"image:tiger.png[Tiger,link=http://en.wikipedia.org/wiki/Tiger,window=_blank]"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<span class="image"><a class="image" href="http://en.wikipedia.org/wiki/Tiger" target="_blank" rel="noopener"><img src="tiger.png" alt="Tiger"></a></span>"#,
                },
                source: Span {
                    data: r#"image:tiger.png[Tiger,link=http://en.wikipedia.org/wiki/Tiger,window=_blank]"#,
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
            },)
        );
    }

    #[test]
    fn rel_noopener_should_be_added_to_an_image_with_a_link_that_targets_a_named_window_when_the_noopener_option_is_set()
     {
        let mut p = Parser::default();

        let maw = crate::blocks::Block::parse(
            crate::Span::new(
                r#"image:tiger.png[Tiger,link=http://en.wikipedia.org/wiki/Tiger,window=name,opts=noopener]"#,
            ),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"image:tiger.png[Tiger,link=http://en.wikipedia.org/wiki/Tiger,window=name,opts=noopener]"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<span class="image"><a class="image" href="http://en.wikipedia.org/wiki/Tiger" target="name" rel="noopener"><img src="tiger.png" alt="Tiger"></a></span>"#,
                },
                source: Span {
                    data: r#"image:tiger.png[Tiger,link=http://en.wikipedia.org/wiki/Tiger,window=name,opts=noopener]"#,
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
            },)
        );
    }

    #[test]
    fn rel_nofollow_should_be_added_to_an_image_with_a_link_when_the_nofollow_option_is_set() {
        let mut p = Parser::default();

        let maw = crate::blocks::Block::parse(
            crate::Span::new(
                r#"image:tiger.png[Tiger,link=http://en.wikipedia.org/wiki/Tiger,opts=nofollow]"#,
            ),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"image:tiger.png[Tiger,link=http://en.wikipedia.org/wiki/Tiger,opts=nofollow]"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<span class="image"><a class="image" href="http://en.wikipedia.org/wiki/Tiger" rel="nofollow"><img src="tiger.png" alt="Tiger"></a></span>"#,
                },
                source: Span {
                    data: r#"image:tiger.png[Tiger,link=http://en.wikipedia.org/wiki/Tiger,opts=nofollow]"#,
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
            },)
        );
    }

    #[test]
    fn a_multi_line_image_macro_with_text_and_dimensions_should_be_interpreted_as_an_image_with_alt_text_and_dimensions()
     {
        let mut p = Parser::default();

        let maw = crate::blocks::Block::parse(
            crate::Span::new("image:tiger.png[Another\nAwesome\nTiger, 200,\n100]"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "image:tiger.png[Another\nAwesome\nTiger, 200,\n100]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<span class="image"><img src="tiger.png" alt="Another Awesome Tiger" width="200" height="100"></span>"#,
                },
                source: Span {
                    data: "image:tiger.png[Another\nAwesome\nTiger, 200,\n100]",
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
            },)
        );
    }

    #[test]
    fn an_inline_image_macro_with_a_url_target_should_be_interpreted_as_an_image() {
        let mut p = Parser::default();

        let maw = crate::blocks::Block::parse(
            crate::Span::new(r#"Beware of the image:http://example.com/images/tiger.png[tiger]."#),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"Beware of the image:http://example.com/images/tiger.png[tiger]."#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"Beware of the <span class="image"><img src="http://example.com/images/tiger.png" alt="tiger"></span>."#,
                },
                source: Span {
                    data: r#"Beware of the image:http://example.com/images/tiger.png[tiger]."#,
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
            },)
        );
    }

    #[test]
    fn an_inline_image_macro_with_a_float_attribute_should_be_interpreted_as_a_floating_image() {
        let mut p = Parser::default();

        let maw = crate::blocks::Block::parse(
            crate::Span::new(
                r#"image:http://example.com/images/tiger.png[tiger, float="right"] Beware of the tigers!"#,
            ),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"image:http://example.com/images/tiger.png[tiger, float="right"] Beware of the tigers!"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<span class="image right"><img src="http://example.com/images/tiger.png" alt="tiger"></span> Beware of the tigers!"#,
                },
                source: Span {
                    data: r#"image:http://example.com/images/tiger.png[tiger, float="right"] Beware of the tigers!"#,
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
            },)
        );
    }

    #[test]
    fn should_prepend_value_of_imagesdir_attribute_to_inline_image_target_if_target_is_relative_path()
     {
        let mut p = Parser::default().with_intrinsic_attribute(
            "imagesdir",
            "./images",
            ModificationContext::Anywhere,
        );

        let maw = crate::blocks::Block::parse(
            crate::Span::new(r#"Beware of the image:tiger.png[tiger]."#),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"Beware of the image:tiger.png[tiger]."#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"Beware of the <span class="image"><img src="./images/tiger.png" alt="tiger"></span>."#,
                },
                source: Span {
                    data: r#"Beware of the image:tiger.png[tiger]."#,
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
            },)
        );
    }

    #[test]
    fn should_not_prepend_value_of_imagesdir_attribute_to_inline_image_target_if_target_is_absolute_path()
     {
        let mut p = Parser::default().with_intrinsic_attribute(
            "imagesdir",
            "./images",
            ModificationContext::Anywhere,
        );

        let maw = crate::blocks::Block::parse(
            crate::Span::new(r#"Beware of the image:/tiger.png[tiger]."#),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"Beware of the image:/tiger.png[tiger]."#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"Beware of the <span class="image"><img src="/tiger.png" alt="tiger"></span>."#,
                },
                source: Span {
                    data: r#"Beware of the image:/tiger.png[tiger]."#,
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
            },)
        );
    }

    #[test]
    fn should_not_prepend_value_of_imagesdir_attribute_to_inline_image_target_if_target_is_url() {
        let mut p = Parser::default().with_intrinsic_attribute(
            "imagesdir",
            "./images",
            ModificationContext::Anywhere,
        );

        let maw = crate::blocks::Block::parse(
            crate::Span::new(r#"Beware of the image:http://example.com/images/tiger.png[tiger]."#),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"Beware of the image:http://example.com/images/tiger.png[tiger]."#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"Beware of the <span class="image"><img src="http://example.com/images/tiger.png" alt="tiger"></span>."#,
                },
                source: Span {
                    data: r#"Beware of the image:http://example.com/images/tiger.png[tiger]."#,
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
            },)
        );
    }

    #[test]
    fn should_match_an_inline_image_macro_if_target_contains_a_space_character() {
        let mut p = Parser::default();

        let maw = crate::blocks::Block::parse(
            crate::Span::new(r#"Beware of the image:big cats.png[] around here."#),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"Beware of the image:big cats.png[] around here."#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"Beware of the <span class="image"><img src="big%20cats.png" alt="big cats"></span> around here."#,
                },
                source: Span {
                    data: r#"Beware of the image:big cats.png[] around here."#,
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
            },)
        );
    }

    #[test]
    fn should_not_match_an_inline_image_macro_if_target_contains_a_newline_character() {
        let mut p = Parser::default();

        let maw = crate::blocks::Block::parse(
            crate::Span::new("Fear not. There are no image:big\ncats.png[] around here."),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "Fear not. There are no image:big\ncats.png[] around here.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "Fear not. There are no image:big\ncats.png[] around here.",
                },
                source: Span {
                    data: "Fear not. There are no image:big\ncats.png[] around here.",
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
            },)
        );
    }

    #[test]
    fn should_not_match_an_inline_image_macro_if_target_begins_with_space_character() {
        let mut p = Parser::default();

        let maw = crate::blocks::Block::parse(
            crate::Span::new("Fear not. There are no image: big cats.png[] around here."),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "Fear not. There are no image: big cats.png[] around here.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "Fear not. There are no image: big cats.png[] around here.",
                },
                source: Span {
                    data: "Fear not. There are no image: big cats.png[] around here.",
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
            },)
        );
    }

    #[test]
    fn should_not_match_an_inline_image_macro_if_target_ends_with_space_character() {
        let mut p = Parser::default();

        let maw = crate::blocks::Block::parse(
            crate::Span::new("Fear not. There are no image:big cats.png [] around here."),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "Fear not. There are no image:big cats.png [] around here.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "Fear not. There are no image:big cats.png [] around here.",
                },
                source: Span {
                    data: "Fear not. There are no image:big cats.png [] around here.",
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
            },)
        );
    }

    #[test]
    fn should_not_detect_a_block_image_macro_found_inline() {
        let mut p = Parser::default();

        let maw = crate::blocks::Block::parse(
            crate::Span::new("Not an inline image macro image::tiger.png[]."),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "Not an inline image macro image::tiger.png[].",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "Not an inline image macro image::tiger.png[].",
                },
                source: Span {
                    data: "Not an inline image macro image::tiger.png[].",
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
            },)
        );
    }

    #[ignore]
    #[test]
    fn should_substitute_attributes_in_target_of_inline_image_in_section_title() {
        todo!(
            "Port this test when implementing safe modes: {}",
            r###"
            # NOTE this test verifies attributes get substituted eagerly in target of image in title
            test 'should substitute attributes in target of inline image in section title' do
                input = '== image:{iconsdir}/dot.gif[dot] Title'

                using_memory_logger do |logger|
                sect = block_from_string input, attributes: { 'data-uri' => '', 'iconsdir' => 'fixtures', 'docdir' => testdir }, safe: :server, catalog_assets: true
                assert_equal 1, sect.document.catalog[:images].size
                assert_equal 'fixtures/dot.gif', sect.document.catalog[:images][0].to_s
                assert_nil sect.document.catalog[:images][0].imagesdir
                assert_empty logger
                end
            end
        "###
        );
    }

    #[test]
    fn an_icon_macro_should_be_interpreted_as_an_icon_if_icons_are_enabled() {
        let mut content = crate::content::Content::from(crate::Span::new("icon:github[]"));

        let expected =
            r#"<span class="icon"><img src="./images/icons/github.png" alt="github"></span>"#;

        let p = Parser::default().with_intrinsic_attribute_bool(
            "icons",
            true,
            ModificationContext::ApiOnly,
        );

        SubstitutionStep::Macros.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(expected.to_string().into_boxed_str())
        );
    }

    #[test]
    fn an_icon_macro_should_be_interpreted_as_alt_text_if_icons_are_disabled() {
        let mut content = crate::content::Content::from(crate::Span::new("icon:github[]"));

        let expected = r#"<span class="icon">[github&#93;</span>"#;

        let p = Parser::default();

        SubstitutionStep::Macros.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(expected.to_string().into_boxed_str())
        );
    }

    #[test]
    fn should_not_mangle_icon_with_link_if_icons_are_disabled() {
        let mut content =
            crate::content::Content::from(crate::Span::new("icon:github[link=https://github.com]"));

        let expected = r#"<span class="icon"><a class="image" href="https://github.com">[github&#93;</a></span>"#;

        let p = Parser::default();

        SubstitutionStep::Macros.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(expected.to_string().into_boxed_str())
        );
    }

    #[ignore]
    #[test]
    fn should_not_mangle_icon_inside_link_if_icons_are_disabled() {
        // TO DO: Enable this test once http: links are supported.
        let mut content = crate::content::Content::from(crate::Span::new(
            "https://github.com[icon:github[] GitHub]",
        ));

        let expected =
            r#"<a href="https://github.com"><span class="icon">[github&#93;</span> GitHub</a>"#;

        let p = Parser::default();

        SubstitutionStep::Macros.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(expected.to_string().into_boxed_str())
        );
    }

    #[test]
    fn an_icon_macro_should_output_alt_text_if_icons_are_disabled_and_alt_is_given() {
        let mut content =
            crate::content::Content::from(crate::Span::new(r#"icon:github[alt="GitHub"]"#));

        let expected = r#"<span class="icon">[GitHub&#93;</span>"#;

        let p = Parser::default();

        SubstitutionStep::Macros.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(expected.to_string().into_boxed_str())
        );
    }

    #[test]
    fn an_icon_macro_should_be_interpreted_as_a_font_based_icon_when_icons_eq_font() {
        let mut content = crate::content::Content::from(crate::Span::new(r#"icon:github[]"#));

        let expected = r#"<span class="icon"><i class="fa fa-github"></i></span>"#;

        let p = Parser::default().with_intrinsic_attribute(
            "icons",
            "font",
            ModificationContext::ApiOnly,
        );

        SubstitutionStep::Macros.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(expected.to_string().into_boxed_str())
        );
    }

    #[test]
    fn an_icon_macro_with_a_size_should_be_interpreted_as_a_font_based_icon_with_a_size_when_icons_eq_font()
     {
        let mut content = crate::content::Content::from(crate::Span::new(r#"icon:github[4x]"#));

        let expected = r#"<span class="icon"><i class="fa fa-github fa-4x"></i></span>"#;

        let p = Parser::default().with_intrinsic_attribute(
            "icons",
            "font",
            ModificationContext::ApiOnly,
        );

        SubstitutionStep::Macros.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(expected.to_string().into_boxed_str())
        );
    }

    #[test]
    fn an_icon_macro_with_flip_should_be_interpreted_as_a_flipped_font_based_icon_when_icons_eq_font()
     {
        let mut content =
            crate::content::Content::from(crate::Span::new(r#"icon:shield[fw,flip=horizontal]"#));

        let expected =
            r#"<span class="icon"><i class="fa fa-shield fa-fw fa-flip-horizontal"></i></span>"#;

        let p = Parser::default().with_intrinsic_attribute(
            "icons",
            "font",
            ModificationContext::ApiOnly,
        );

        SubstitutionStep::Macros.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(expected.to_string().into_boxed_str())
        );
    }

    #[test]
    fn an_icon_macro_with_rotate_should_be_interpreted_as_a_rotated_font_based_icon_when_icons_eq_font()
     {
        let mut content =
            crate::content::Content::from(crate::Span::new(r#"icon:shield[fw,rotate=90]"#));

        let expected =
            r#"<span class="icon"><i class="fa fa-shield fa-fw fa-rotate-90"></i></span>"#;

        let p = Parser::default().with_intrinsic_attribute(
            "icons",
            "font",
            ModificationContext::ApiOnly,
        );

        SubstitutionStep::Macros.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(expected.to_string().into_boxed_str())
        );
    }

    #[test]
    fn an_icon_macro_with_a_role_and_title_should_be_interpreted_as_a_font_based_icon_with_a_class_and_title_when_icons_eq_font()
     {
        let mut content = crate::content::Content::from(crate::Span::new(
            r#"icon:heart[role="red", title="Heart me"]"#,
        ));

        let expected =
            r#"<span class="icon red"><i class="fa fa-heart" title="Heart me"></i></span>"#;

        let p = Parser::default().with_intrinsic_attribute(
            "icons",
            "font",
            ModificationContext::ApiOnly,
        );

        SubstitutionStep::Macros.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(expected.to_string().into_boxed_str())
        );
    }

    #[test]
    fn an_icon_macro_with_width_should_be_interpreted_as_an_icon_with_width_if_icons_are_enabled() {
        let mut content =
            crate::content::Content::from(crate::Span::new(r#"icon:github[width=32]"#));

        let expected = r#"<span class="icon"><img src="./images/icons/github.png" alt="github" width="32"></span>"#;

        let p = Parser::default().with_intrinsic_attribute_bool(
            "icons",
            true,
            ModificationContext::ApiOnly,
        );

        SubstitutionStep::Macros.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(expected.to_string().into_boxed_str())
        );
    }

    #[test]
    fn an_icon_macro_with_height_should_be_interpreted_as_an_icon_with_height_if_icons_are_enabled()
    {
        let mut content =
            crate::content::Content::from(crate::Span::new(r#"icon:github[height=24]"#));

        let expected = r#"<span class="icon"><img src="./images/icons/github.png" alt="github" height="24"></span>"#;

        let p = Parser::default().with_intrinsic_attribute_bool(
            "icons",
            true,
            ModificationContext::ApiOnly,
        );

        SubstitutionStep::Macros.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(expected.to_string().into_boxed_str())
        );
    }

    #[test]
    fn an_icon_macro_with_title_should_be_interpreted_as_an_icon_with_title_if_icons_are_enabled() {
        let mut content =
            crate::content::Content::from(crate::Span::new(r#"icon:github[title="GitHub Icon"]"#));

        let expected = r#"<span class="icon"><img src="./images/icons/github.png" alt="github" title="GitHub Icon"></span>"#;

        let p = Parser::default().with_intrinsic_attribute_bool(
            "icons",
            true,
            ModificationContext::ApiOnly,
        );

        SubstitutionStep::Macros.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(expected.to_string().into_boxed_str())
        );
    }

    #[ignore]
    #[test]
    fn todo_migrate_from_ruby_2() {
        todo!(
            "{}",
            r###"
        test 'a single-line footnote macro should be registered and output as a footnote' do
            para = block_from_string 'Sentence text footnote:[An example footnote.].'
            assert_equal %(Sentence text <sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>.), para.sub_macros(para.source)
            assert_equal 1, para.document.catalog[:footnotes].size
            footnote = para.document.catalog[:footnotes].first
            assert_equal 1, footnote.index
            assert_nil footnote.id
            assert_equal 'An example footnote.', footnote.text
        end

        test 'a multi-line footnote macro should be registered and output as a footnote without newline' do
            para = block_from_string "Sentence text footnote:[An example footnote\nwith wrapped text.]."
            assert_equal %(Sentence text <sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>.), para.sub_macros(para.source)
            assert_equal 1, para.document.catalog[:footnotes].size
            footnote = para.document.catalog[:footnotes].first
            assert_equal 1, footnote.index
            assert_nil footnote.id
            assert_equal 'An example footnote with wrapped text.', footnote.text
        end

        test 'an escaped closing square bracket in a footnote should be unescaped when converted' do
            para = block_from_string %(footnote:[a #{BACKSLASH}] b].)
            assert_equal %(<sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>.), para.sub_macros(para.source)
            assert_equal 1, para.document.catalog[:footnotes].size
            footnote = para.document.catalog[:footnotes].first
            assert_equal 'a ] b', footnote.text
        end

        test 'a footnote macro can be directly adjacent to preceding word' do
            para = block_from_string 'Sentence textfootnote:[An example footnote.].'
            assert_equal 'Sentence text<sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>.', para.sub_macros(para.source)
        end

        test 'a footnote macro may contain an escaped backslash' do
            para = block_from_string "footnote:[\\]]\nfootnote:[a \\] b]\nfootnote:[a \\]\\] b]"
            para.sub_macros para.source
            assert_equal 3, para.document.catalog[:footnotes].size
            footnote1 = para.document.catalog[:footnotes][0]
            assert_equal ']', footnote1.text
            footnote2 = para.document.catalog[:footnotes][1]
            assert_equal 'a ] b', footnote2.text
            footnote3 = para.document.catalog[:footnotes][2]
            assert_equal 'a ]] b', footnote3.text
        end

        test 'a footnote macro may contain a link macro' do
            para = block_from_string 'Share your code. footnote:[https://github.com[GitHub]]'
            assert_equal %(Share your code. <sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>), para.sub_macros(para.source)
            assert_equal 1, para.document.catalog[:footnotes].size
            footnote1 = para.document.catalog[:footnotes][0]
            assert_equal '<a href="https://github.com">GitHub</a>', footnote1.text
        end

        test 'a footnote macro may contain a plain URL' do
            para = block_from_string %(the JLine footnote:[https://github.com/jline/jline2]\nlibrary.)
            result = para.sub_macros para.source
            assert_equal %(the JLine <sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>\nlibrary.), result
            assert_equal 1, para.document.catalog[:footnotes].size
            fn1 = para.document.catalog[:footnotes].first
            assert_equal '<a href="https://github.com/jline/jline2" class="bare">https://github.com/jline/jline2</a>', fn1.text
        end

        test 'a footnote macro followed by a semi-colon may contain a plain URL' do
            para = block_from_string %(the JLine footnote:[https://github.com/jline/jline2];\nlibrary.)
            result = para.sub_macros para.source
            assert_equal %(the JLine <sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>;\nlibrary.), result
            assert_equal 1, para.document.catalog[:footnotes].size
            fn1 = para.document.catalog[:footnotes].first
            assert_equal '<a href="https://github.com/jline/jline2" class="bare">https://github.com/jline/jline2</a>', fn1.text
        end

        test 'a footnote macro may contain text formatting' do
            para = block_from_string 'You can download patches from the product page.footnote:[Only available with an _active_ subscription.]'
            para.convert
            footnotes = para.document.catalog[:footnotes]
            assert_equal 1, footnotes.size
            assert_equal 'Only available with an <em>active</em> subscription.', footnotes[0].text
        end

        test 'an externalized footnote macro may contain text formatting' do
            input = <<~'EOS'
            :fn-disclaimer: pass:q[footnote:[Only available with an _active_ subscription.]]

            You can download patches from the production page.{fn-disclaimer}
            EOS
            doc = document_from_string input
            doc.convert
            footnotes = doc.catalog[:footnotes]
            assert_equal 1, footnotes.size
            assert_equal 'Only available with an <em>active</em> subscription.', footnotes[0].text
        end

        test 'a footnote macro may contain a shorthand xref' do
            # specialcharacters escaping is simulated
            para = block_from_string 'text footnote:[&lt;&lt;_install,install&gt;&gt;]'
            doc = para.document
            doc.register :refs, ['_install', (Asciidoctor::Inline.new doc, :anchor, 'Install', type: :ref, target: '_install'), 'Install']
            catalog = doc.catalog
            assert_equal %(text <sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>), para.sub_macros(para.source)
            assert_equal 1, catalog[:footnotes].size
            footnote1 = catalog[:footnotes][0]
            assert_equal '<a href="#_install">install</a>', footnote1.text
        end

        test 'a footnote macro may contain an xref macro' do
            para = block_from_string 'text footnote:[xref:_install[install]]'
            doc = para.document
            doc.register :refs, ['_install', (Asciidoctor::Inline.new doc, :anchor, 'Install', type: :ref, target: '_install'), 'Install']
            catalog = doc.catalog
            assert_equal %(text <sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>), para.sub_macros(para.source)
            assert_equal 1, catalog[:footnotes].size
            footnote1 = catalog[:footnotes][0]
            assert_equal '<a href="#_install">install</a>', footnote1.text
        end

        test 'a footnote macro may contain an anchor macro' do
            para = block_from_string 'text footnote:[a [[b]] [[c\]\] d]'
            assert_equal %(text <sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>), para.sub_macros(para.source)
            assert_equal 1, para.document.catalog[:footnotes].size
            footnote1 = para.document.catalog[:footnotes][0]
            assert_equal 'a <a id="b"></a> [[c]] d', footnote1.text
        end

        test 'subsequent footnote macros with escaped URLs should be restored in DocBook' do
            input = 'foofootnote:[+http://example.com+]barfootnote:[+http://acme.com+]baz'

            result = convert_string_to_embedded input, doctype: 'inline', backend: 'docbook'
            assert_equal 'foo<footnote><simpara>http://example.com</simpara></footnote>bar<footnote><simpara>http://acme.com</simpara></footnote>baz', result
        end

        test 'should increment index of subsequent footnote macros' do
            para = block_from_string 'Sentence text footnote:[An example footnote.]. Sentence text footnote:[Another footnote.].'
            assert_equal 'Sentence text <sup class="footnote">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>. Sentence text <sup class="footnote">[<a id="_footnoteref_2" class="footnote" href="#_footnotedef_2" title="View footnote.">2</a>]</sup>.', para.sub_macros(para.source)
            assert_equal 2, para.document.catalog[:footnotes].size
            footnote1 = para.document.catalog[:footnotes][0]
            assert_equal 1, footnote1.index
            assert_nil footnote1.id
            assert_equal 'An example footnote.', footnote1.text
            footnote2 = para.document.catalog[:footnotes][1]
            assert_equal 2, footnote2.index
            assert_nil footnote2.id
            assert_equal 'Another footnote.', footnote2.text
        end

        test 'a footnoteref macro with id and single-line text should be registered and output as a footnote' do
            para = block_from_string 'Sentence text footnoteref:[ex1, An example footnote.].', attributes: { 'compat-mode' => '' }
            assert_equal %(Sentence text <sup class="footnote" id="_footnote_ex1">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>.), para.sub_macros(para.source)
            assert_equal 1, para.document.catalog[:footnotes].size
            footnote = para.document.catalog[:footnotes].first
            assert_equal 1, footnote.index
            assert_equal 'ex1', footnote.id
            assert_equal 'An example footnote.', footnote.text
        end

        test 'a footnoteref macro with id and multi-line text should be registered and output as a footnote without newlines' do
            para = block_from_string "Sentence text footnoteref:[ex1, An example footnote\nwith wrapped text.].", attributes: { 'compat-mode' => '' }
            assert_equal %(Sentence text <sup class="footnote" id="_footnote_ex1">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>.), para.sub_macros(para.source)
            assert_equal 1, para.document.catalog[:footnotes].size
            footnote = para.document.catalog[:footnotes].first
            assert_equal 1, footnote.index
            assert_equal 'ex1', footnote.id
            assert_equal 'An example footnote with wrapped text.', footnote.text
        end

        test 'a footnoteref macro with id should refer to footnoteref with same id' do
            para = block_from_string 'Sentence text footnoteref:[ex1, An example footnote.]. Sentence text footnoteref:[ex1].', attributes: { 'compat-mode' => '' }
            assert_equal %(Sentence text <sup class="footnote" id="_footnote_ex1">[<a id="_footnoteref_1" class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>. Sentence text <sup class="footnoteref">[<a class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]</sup>.), para.sub_macros(para.source)
            assert_equal 1, para.document.catalog[:footnotes].size
            footnote = para.document.catalog[:footnotes].first
            assert_equal 1, footnote.index
            assert_equal 'ex1', footnote.id
            assert_equal 'An example footnote.', footnote.text
        end

        test 'an unresolved footnote reference should produce a warning message and output fallback text in red' do
            input = 'Sentence text.footnote:ex1[]'
            using_memory_logger do |logger|
            para = block_from_string input
            output = para.sub_macros para.source
            assert_equal 'Sentence text.<sup class="footnoteref red" title="Unresolved footnote reference.">[ex1]</sup>', output
            assert_message logger, :WARN, 'invalid footnote reference: ex1'
            end
        end

        test 'using a footnoteref macro should generate a warning when compat mode is not enabled' do
            input = 'Sentence text.footnoteref:[fn1,Commentary on this sentence.]'
            using_memory_logger do |logger|
            para = block_from_string input
            para.sub_macros para.source
            assert_message logger, :WARN, 'found deprecated footnoteref macro: footnoteref:[fn1,Commentary on this sentence.]; use footnote macro with target instead'
            end
        end

        test 'inline footnote macro can be used to define and reference a footnote reference' do
            input = <<~'EOS'
            You can download the software from the product page.footnote:sub[Option only available if you have an active subscription.]

            You can also file a support request.footnote:sub[]

            If all else fails, you can give us a call.footnoteref:[sub]
            EOS

            using_memory_logger do |logger|
            output = convert_string_to_embedded input, attributes: { 'compat-mode' => '' }
            assert_css '#_footnotedef_1', output, 1
            assert_css 'p a[href="#_footnotedef_1"]', output, 3
            assert_css '#footnotes .footnote', output, 1
            assert_empty logger
            end
        end

        test 'should parse multiple footnote references in a single line' do
            input = 'notable text.footnote:id[about this [text\]], footnote:id[], footnote:id[]'
            output = convert_string_to_embedded input
            assert_xpath '(//p)[1]/sup[starts-with(@class,"footnote")]', output, 3
            assert_xpath '(//p)[1]/sup[@class="footnote"]', output, 1
            assert_xpath '(//p)[1]/sup[@class="footnoteref"]', output, 2
            assert_xpath '(//p)[1]/sup[starts-with(@class,"footnote")]/a[@class="footnote"][text()="1"]', output, 3
            assert_css '#footnotes .footnote', output, 1
        end

        test 'should not register footnote with id and text if id already registered' do
            input = <<~'EOS'
            :fn-notable-text: footnote:id[about this text]

            notable text.{fn-notable-text}

            more notable text.{fn-notable-text}
            EOS
            output = convert_string_to_embedded input
            assert_xpath '(//p)[1]/sup[@class="footnote"]', output, 1
            assert_xpath '(//p)[2]/sup[@class="footnoteref"]', output, 1
            assert_css '#footnotes .footnote', output, 1
        end

        test 'should not resolve an inline footnote macro missing both id and text' do
            input = <<~'EOS'
            The footnote:[] macro can be used for defining and referencing footnotes.

            The footnoteref:[] macro is now deprecated.
            EOS

            output = convert_string_to_embedded input
            assert_includes output, 'The footnote:[] macro'
            assert_includes output, 'The footnoteref:[] macro'
        end

        test 'inline footnote macro can define a numeric id without conflicting with auto-generated ID' do
            input = 'You can download the software from the product page.footnote:1[Option only available if you have an active subscription.]'

            output = convert_string_to_embedded input
            assert_css '#_footnote_1', output, 1
            assert_css 'p sup#_footnote_1', output, 1
            assert_css 'p a#_footnoteref_1', output, 1
            assert_css 'p a[href="#_footnotedef_1"]', output, 1
            assert_css '#footnotes #_footnotedef_1', output, 1
        end

        test 'inline footnote macro can define an id that uses any word characters in Unicode' do
            input = <<~'EOS'
            L'origine du mot forêt{blank}footnote:forêt[un massif forestier] est complexe.

            Qu'est-ce qu'une forêt ?{blank}footnote:forêt[]
            EOS
            output = convert_string_to_embedded input
            assert_css '#_footnote_forêt', output, 1
            assert_css '#_footnotedef_1', output, 1
            assert_xpath '//a[@class="footnote"][text()="1"]', output, 2
        end

        test 'should be able to reference a bibliography entry in a footnote' do
            input = <<~'EOS'
            Choose a design pattern.footnote:[See <<gof>> to find a collection of design patterns.]

            [bibliography]
            == Bibliography

            * [[[gof]]] Erich Gamma, et al. _Design Patterns: Elements of Reusable Object-Oriented Software._ Addison-Wesley. 1994.
            EOS

            result = convert_string_to_embedded input
            assert_include '<a href="#_footnoteref_1">1</a>. See <a href="#gof">[gof]</a> to find a collection of design patterns.', result
        end

        test 'footnotes in headings are expected to be numbered out of sequence' do
            input = <<~'EOS'
            == Section 1

            para.footnote:[first footnote]

            == Section 2footnote:[second footnote]

            para.footnote:[third footnote]
            EOS

            result = convert_string_to_embedded input
            footnote_refs = xmlnodes_at_css 'a.footnote', result
            footnote_defs = xmlnodes_at_css 'div.footnote', result
            assert_equal 3, footnote_refs.length
            assert_equal %w(1 1 2), footnote_refs.map(&:text)
            assert_equal 3, footnote_defs.length
            assert_equal ['1. second footnote', '1. first footnote', '2. third footnote'], footnote_defs.map(&:text).map(&:strip)
        end

        test 'a single-line index term macro with a primary term should be registered as an index reference' do
            sentence = "The tiger (Panthera tigris) is the largest cat species.\n"
            macros = ['indexterm:[Tigers]', '(((Tigers)))']
            macros.each do |macro|
            para = block_from_string "#{sentence}#{macro}"
            output = para.sub_macros para.source
            assert_equal sentence, output
            #assert_equal 1, para.document.catalog[:indexterms].size
            #assert_equal ['Tigers'], para.document.catalog[:indexterms].first
            end
        end

        test 'a single-line index term macro with primary and secondary terms should be registered as an index reference' do
            sentence = "The tiger (Panthera tigris) is the largest cat species.\n"
            macros = ['indexterm:[Big cats, Tigers]', '(((Big cats, Tigers)))']
            macros.each do |macro|
            para = block_from_string "#{sentence}#{macro}"
            output = para.sub_macros para.source
            assert_equal sentence, output
            #assert_equal 1, para.document.catalog[:indexterms].size
            #assert_equal ['Big cats', 'Tigers'], para.document.catalog[:indexterms].first
            end
        end

        test 'a single-line index term macro with primary, secondary and tertiary terms should be registered as an index reference' do
            sentence = "The tiger (Panthera tigris) is the largest cat species.\n"
            macros = ['indexterm:[Big cats,Tigers , Panthera tigris]', '(((Big cats,Tigers , Panthera tigris)))']
            macros.each do |macro|
            para = block_from_string "#{sentence}#{macro}"
            output = para.sub_macros para.source
            assert_equal sentence, output
            #assert_equal 1, para.document.catalog[:indexterms].size
            #assert_equal ['Big cats', 'Tigers', 'Panthera tigris'], para.document.catalog[:indexterms].first
            end
        end

        test 'a multi-line index term macro should be compacted and registered as an index reference' do
            sentence = "The tiger (Panthera tigris) is the largest cat species.\n"
            macros = ["indexterm:[Panthera\ntigris]", "(((Panthera\ntigris)))"]
            macros.each do |macro|
            para = block_from_string "#{sentence}#{macro}"
            output = para.sub_macros para.source
            assert_equal sentence, output
            #assert_equal 1, para.document.catalog[:indexterms].size
            #assert_equal ['Panthera tigris'], para.document.catalog[:indexterms].first
            end
        end

        test 'should escape concealed index term if second bracket is preceded by a backslash' do
            input = %[National Institute of Science and Technology (#{BACKSLASH}((NIST)))]
            doc = document_from_string input, standalone: false
            output = doc.convert
            assert_xpath '//p[text()="National Institute of Science and Technology (((NIST)))"]', output, 1
            #assert doc.catalog[:indexterms].empty?
        end

        test 'should only escape enclosing brackets if concealed index term is preceded by a backslash' do
            input = %[National Institute of Science and Technology #{BACKSLASH}(((NIST)))]
            doc = document_from_string input, standalone: false
            output = doc.convert
            assert_xpath '//p[text()="National Institute of Science and Technology (NIST)"]', output, 1
            #term = doc.catalog[:indexterms].first
            #assert_equal 1, term.size
            #assert_equal 'NIST', term.first
        end

        test 'should not split index terms on commas inside of quoted terms' do
            inputs = []
            inputs.push <<~'EOS'
            Tigers are big, scary cats.
            indexterm:[Tigers, "[Big\],
            scary cats"]
            EOS
            inputs.push <<~'EOS'
            Tigers are big, scary cats.
            (((Tigers, "[Big],
            scary cats")))
            EOS

            inputs.each do |input|
            para = block_from_string input
            output = para.sub_macros para.source
            assert_equal input.lines.first, output
            #assert_equal 1, para.document.catalog[:indexterms].size
            #terms = para.document.catalog[:indexterms].first
            #assert_equal 2, terms.size
            #assert_equal 'Tigers', terms.first
            #assert_equal '[Big], scary cats', terms.last
            end
        end

        test 'normal substitutions are performed on an index term macro' do
            sentence = "The tiger (Panthera tigris) is the largest cat species.\n"
            macros = ['indexterm:[*Tigers*]', '(((*Tigers*)))']
            macros.each do |macro|
            para = block_from_string "#{sentence}#{macro}"
            output = para.apply_subs para.source
            assert_equal sentence, output
            #assert_equal 1, para.document.catalog[:indexterms].size
            #assert_equal ['<strong>Tigers</strong>'], para.document.catalog[:indexterms].first
            end
        end

        test 'registers multiple index term macros' do
            sentence = 'The tiger (Panthera tigris) is the largest cat species.'
            macros = "(((Tigers)))\n(((Animals,Cats)))"
            para = block_from_string "#{sentence}\n#{macros}"
            output = para.sub_macros para.source
            assert_equal sentence, output.rstrip
            #assert_equal 2, para.document.catalog[:indexterms].size
            #assert_equal ['Tigers'], para.document.catalog[:indexterms][0]
            #assert_equal ['Animals', 'Cats'], para.document.catalog[:indexterms][1]
        end

        test 'an index term macro with round bracket syntax may contain round brackets in term' do
            sentence = "The tiger (Panthera tigris) is the largest cat species.\n"
            macro = '(((Tiger (Panthera tigris))))'
            para = block_from_string "#{sentence}#{macro}"
            output = para.sub_macros para.source
            assert_equal sentence, output
            #assert_equal 1, para.document.catalog[:indexterms].size
            #assert_equal ['Tiger (Panthera tigris)'], para.document.catalog[:indexterms].first
        end

        test 'visible shorthand index term macro should not consume trailing round bracket' do
            input = '(text with ((index term)))'
            expected = <<~'EOS'.chop
            (text with <indexterm>
            <primary>index term</primary>
            </indexterm>index term)
            EOS
            #expected_term = ['index term']
            para = block_from_string input, backend: :docbook
            output = para.sub_macros para.source
            assert_equal expected, output
            #indexterms_table = para.document.catalog[:indexterms]
            #assert_equal 1, indexterms_table.size
            #assert_equal expected_term, indexterms_table[0]
        end

        test 'visible shorthand index term macro should not consume leading round bracket' do
            input = '(((index term)) for text)'
            expected = <<~'EOS'.chop
            (<indexterm>
            <primary>index term</primary>
            </indexterm>index term for text)
            EOS
            #expected_term = ['index term']
            para = block_from_string input, backend: :docbook
            output = para.sub_macros para.source
            assert_equal expected, output
            #indexterms_table = para.document.catalog[:indexterms]
            #assert_equal 1, indexterms_table.size
            #assert_equal expected_term, indexterms_table[0]
        end

        test 'an index term macro with square bracket syntax may contain square brackets in term' do
            sentence = "The tiger (Panthera tigris) is the largest cat species.\n"
            macro = 'indexterm:[Tiger [Panthera tigris\\]]'
            para = block_from_string "#{sentence}#{macro}"
            output = para.sub_macros para.source
            assert_equal sentence, output
            #assert_equal 1, para.document.catalog[:indexterms].size
            #assert_equal ['Tiger [Panthera tigris]'], para.document.catalog[:indexterms].first
        end

        test 'a single-line index term 2 macro should be registered as an index reference and retain term inline' do
            sentence = 'The tiger (Panthera tigris) is the largest cat species.'
            macros = ['The indexterm2:[tiger] (Panthera tigris) is the largest cat species.', 'The ((tiger)) (Panthera tigris) is the largest cat species.']
            macros.each do |macro|
            para = block_from_string macro
            output = para.sub_macros para.source
            assert_equal sentence, output
            #assert_equal 1, para.document.catalog[:indexterms].size
            #assert_equal ['tiger'], para.document.catalog[:indexterms].first
            end
        end

        test 'a multi-line index term 2 macro should be compacted and registered as an index reference and retain term inline' do
            sentence = 'The panthera tigris is the largest cat species.'
            macros = ["The indexterm2:[ panthera\ntigris ] is the largest cat species.", "The (( panthera\ntigris )) is the largest cat species."]
            macros.each do |macro|
            para = block_from_string macro
            output = para.sub_macros para.source
            assert_equal sentence, output
            #assert_equal 1, para.document.catalog[:indexterms].size
            #assert_equal ['panthera tigris'], para.document.catalog[:indexterms].first
            end
        end

        test 'registers multiple index term 2 macros' do
            sentence = 'The ((tiger)) (Panthera tigris) is the largest ((cat)) species.'
            para = block_from_string sentence
            output = para.sub_macros para.source
            assert_equal 'The tiger (Panthera tigris) is the largest cat species.', output
            #assert_equal 2, para.document.catalog[:indexterms].size
            #assert_equal ['tiger'], para.document.catalog[:indexterms][0]
            #assert_equal ['cat'], para.document.catalog[:indexterms][1]
        end

        test 'should escape visible index term if preceded by a backslash' do
            sentence = "The #{BACKSLASH}((tiger)) (Panthera tigris) is the largest #{BACKSLASH}((cat)) species."
            para = block_from_string sentence
            output = para.sub_macros para.source
            assert_equal 'The ((tiger)) (Panthera tigris) is the largest ((cat)) species.', output
            #assert para.document.catalog[:indexterms].empty?
        end

        test 'normal substitutions are performed on an index term 2 macro' do
            sentence = 'The ((*tiger*)) (Panthera tigris) is the largest cat species.'
            para = block_from_string sentence
            output = para.apply_subs para.source
            assert_equal 'The <strong>tiger</strong> (Panthera tigris) is the largest cat species.', output
            #assert_equal 1, para.document.catalog[:indexterms].size
            #assert_equal ['<strong>tiger</strong>'], para.document.catalog[:indexterms].first
        end

        test 'index term 2 macro with round bracket syntex should not interfer with index term macro with round bracket syntax' do
            sentence = "The ((panthera tigris)) is the largest cat species.\n(((Big cats,Tigers)))"
            para = block_from_string sentence
            output = para.sub_macros para.source
            assert_equal "The panthera tigris is the largest cat species.\n", output
            #terms = para.document.catalog[:indexterms]
            #assert_equal 2, terms.size
            #assert_equal ['panthera tigris'], terms[0]
            #assert_equal ['Big cats', 'Tigers'], terms[1]
        end

        test 'should parse visible shorthand index term with see and seealso' do
            sentence = '((Flash >> HTML 5)) has been supplanted by ((HTML 5 &> CSS 3 &> SVG)).'
            output = convert_string_to_embedded sentence, backend: 'docbook'
            indexterm_flash = <<~'EOS'.chop
            <indexterm>
            <primary>Flash</primary>
            <see>HTML 5</see>
            </indexterm>
            EOS
            indexterm_html5 = <<~'EOS'.chop
            <indexterm>
            <primary>HTML 5</primary>
            <seealso>CSS 3</seealso>
            <seealso>SVG</seealso>
            </indexterm>
            EOS
            assert_includes output, indexterm_flash
            assert_includes output, indexterm_html5
        end

        test 'should parse concealed shorthand index term with see and seealso' do
            sentence = 'Flash(((Flash >> HTML 5))) has been supplanted by HTML 5(((HTML 5 &> CSS 3 &> SVG))).'
            output = convert_string_to_embedded sentence, backend: 'docbook'
            indexterm_flash = <<~'EOS'.chop
            <indexterm>
            <primary>Flash</primary>
            <see>HTML 5</see>
            </indexterm>
            EOS
            indexterm_html5 = <<~'EOS'.chop
            <indexterm>
            <primary>HTML 5</primary>
            <seealso>CSS 3</seealso>
            <seealso>SVG</seealso>
            </indexterm>
            EOS
            assert_includes output, indexterm_flash
            assert_includes output, indexterm_html5
        end

        test 'should parse visible index term macro with see and seealso' do
            sentence = 'indexterm2:[Flash,see=HTML 5] has been supplanted by indexterm2:[HTML 5,see-also="CSS 3, SVG"].'
            output = convert_string_to_embedded sentence, backend: 'docbook'
            indexterm_flash = <<~'EOS'.chop
            <indexterm>
            <primary>Flash</primary>
            <see>HTML 5</see>
            </indexterm>
            EOS
            indexterm_html5 = <<~'EOS'.chop
            <indexterm>
            <primary>HTML 5</primary>
            <seealso>CSS 3</seealso>
            <seealso>SVG</seealso>
            </indexterm>
            EOS
            assert_includes output, indexterm_flash
            assert_includes output, indexterm_html5
        end

        test 'should parse concealed index term macro with see and seealso' do
            sentence = 'Flashindexterm:[Flash,see=HTML 5] has been supplanted by HTML 5indexterm:[HTML 5,see-also="CSS 3, SVG"].'
            output = convert_string_to_embedded sentence, backend: 'docbook'
            indexterm_flash = <<~'EOS'.chop
            <indexterm>
            <primary>Flash</primary>
            <see>HTML 5</see>
            </indexterm>
            EOS
            indexterm_html5 = <<~'EOS'.chop
            <indexterm>
            <primary>HTML 5</primary>
            <seealso>CSS 3</seealso>
            <seealso>SVG</seealso>
            </indexterm>
            EOS
            assert_includes output, indexterm_flash
            assert_includes output, indexterm_html5
        end

        test 'should honor secondary and tertiary index terms when primary index term is quoted and contains equals sign' do
            sentence = 'Assigning variables.'
            expected = %(#{sentence}<indexterm><primary>name=value</primary><secondary>variable</secondary><tertiary>assignment</tertiary></indexterm>)
            macros = ['indexterm:["name=value",variable,assignment]', '(((name=value,variable,assignment)))']
            macros.each do |macro|
            para = block_from_string %(#{sentence}#{macro}), backend: 'docbook'
            output = (para.sub_macros para.source).tr ?\n, ''
            assert_equal expected, output
            end
        end

        context 'Button macro' do
            test 'btn macro' do
            para = block_from_string 'btn:[Save]', attributes: { 'experimental' => '' }
            assert_equal '<b class="button">Save</b>', para.sub_macros(para.source)
            end

            test 'btn macro that spans multiple lines' do
            para = block_from_string %(btn:[Rebase and\nmerge]), attributes: { 'experimental' => '' }
            assert_equal '<b class="button">Rebase and merge</b>', para.sub_macros(para.source)
            end

            test 'btn macro for docbook backend' do
            para = block_from_string 'btn:[Save]', backend: 'docbook', attributes: { 'experimental' => '' }
            assert_equal '<guibutton>Save</guibutton>', para.sub_macros(para.source)
            end
        end

        context 'Keyboard macro' do
            test 'kbd macro with single key' do
            para = block_from_string 'kbd:[F3]', attributes: { 'experimental' => '' }
            assert_equal '<kbd>F3</kbd>', para.sub_macros(para.source)
            end

            test 'kbd macro with single backslash key' do
            para = block_from_string "kbd:[#{BACKSLASH} ]", attributes: { 'experimental' => '' }
            assert_equal '<kbd>\</kbd>', para.sub_macros(para.source)
            end

            test 'kbd macro with single key, docbook backend' do
            para = block_from_string 'kbd:[F3]', backend: 'docbook', attributes: { 'experimental' => '' }
            assert_equal '<keycap>F3</keycap>', para.sub_macros(para.source)
            end

            test 'kbd macro with key combination' do
            para = block_from_string 'kbd:[Ctrl+Shift+T]', attributes: { 'experimental' => '' }
            assert_equal '<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>T</kbd></span>', para.sub_macros(para.source)
            end

            test 'kbd macro with key combination that spans multiple lines' do
            para = block_from_string %(kbd:[Ctrl +\nT]), attributes: { 'experimental' => '' }
            assert_equal '<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>T</kbd></span>', para.sub_macros(para.source)
            end

            test 'kbd macro with key combination, docbook backend' do
            para = block_from_string 'kbd:[Ctrl+Shift+T]', backend: 'docbook', attributes: { 'experimental' => '' }
            assert_equal '<keycombo><keycap>Ctrl</keycap><keycap>Shift</keycap><keycap>T</keycap></keycombo>', para.sub_macros(para.source)
            end

            test 'kbd macro with key combination delimited by pluses with spaces' do
            para = block_from_string 'kbd:[Ctrl + Shift + T]', attributes: { 'experimental' => '' }
            assert_equal '<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>T</kbd></span>', para.sub_macros(para.source)
            end

            test 'kbd macro with key combination delimited by commas' do
            para = block_from_string 'kbd:[Ctrl,Shift,T]', attributes: { 'experimental' => '' }
            assert_equal '<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>T</kbd></span>', para.sub_macros(para.source)
            end

            test 'kbd macro with key combination delimited by commas with spaces' do
            para = block_from_string 'kbd:[Ctrl, Shift, T]', attributes: { 'experimental' => '' }
            assert_equal '<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>T</kbd></span>', para.sub_macros(para.source)
            end

            test 'kbd macro with key combination delimited by plus containing a comma key' do
            para = block_from_string 'kbd:[Ctrl+,]', attributes: { 'experimental' => '' }
            assert_equal '<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>,</kbd></span>', para.sub_macros(para.source)
            end

            test 'kbd macro with key combination delimited by commas containing a plus key' do
            para = block_from_string 'kbd:[Ctrl, +, Shift]', attributes: { 'experimental' => '' }
            assert_equal '<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>+</kbd>+<kbd>Shift</kbd></span>', para.sub_macros(para.source)
            end

            test 'kbd macro with key combination where last key matches plus delimiter' do
            para = block_from_string 'kbd:[Ctrl + +]', attributes: { 'experimental' => '' }
            assert_equal '<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>+</kbd></span>', para.sub_macros(para.source)
            end

            test 'kbd macro with key combination where last key matches comma delimiter' do
            para = block_from_string 'kbd:[Ctrl, ,]', attributes: { 'experimental' => '' }
            assert_equal '<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>,</kbd></span>', para.sub_macros(para.source)
            end

            test 'kbd macro with key combination containing escaped bracket' do
            para = block_from_string 'kbd:[Ctrl + \]]', attributes: { 'experimental' => '' }
            assert_equal '<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>]</kbd></span>', para.sub_macros(para.source)
            end

            test 'kbd macro with key combination ending in backslash' do
            para = block_from_string "kbd:[Ctrl + #{BACKSLASH} ]", attributes: { 'experimental' => '' }
            assert_equal '<span class="keyseq"><kbd>Ctrl</kbd>+<kbd>\\</kbd></span>', para.sub_macros(para.source)
            end

            test 'kbd macro looks for delimiter beyond first character' do
            para = block_from_string 'kbd:[,te]', attributes: { 'experimental' => '' }
            assert_equal '<kbd>,te</kbd>', para.sub_macros(para.source)
            end

            test 'kbd macro restores trailing delimiter as key value' do
            para = block_from_string 'kbd:[te,]', attributes: { 'experimental' => '' }
            assert_equal '<kbd>te,</kbd>', para.sub_macros(para.source)
            end
        end

        context 'Menu macro' do
            test 'should process menu using macro sytnax' do
            para = block_from_string 'menu:File[]', attributes: { 'experimental' => '' }
            assert_equal '<b class="menuref">File</b>', para.sub_macros(para.source)
            end

            test 'should process menu for docbook backend' do
            para = block_from_string 'menu:File[]', backend: 'docbook', attributes: { 'experimental' => '' }
            assert_equal '<guimenu>File</guimenu>', para.sub_macros(para.source)
            end

            test 'should process multiple menu macros in same line' do
            para = block_from_string 'menu:File[] and menu:Edit[]', attributes: { 'experimental' => '' }
            assert_equal '<b class="menuref">File</b> and <b class="menuref">Edit</b>', para.sub_macros(para.source)
            end

            test 'should process menu with menu item using macro syntax' do
            para = block_from_string 'menu:File[Save As&#8230;]', attributes: { 'experimental' => '' }
            assert_equal '<span class="menuseq"><b class="menu">File</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">Save As&#8230;</b></span>', para.sub_macros(para.source)
            end

            test 'should process menu macro that spans multiple lines' do
            input = %(menu:Preferences[Compile\non\nSave])
            para = block_from_string input, attributes: { 'experimental' => '' }
            assert_equal %(<span class="menuseq"><b class="menu">Preferences</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">Compile\non\nSave</b></span>), para.sub_macros(para.source)
            end

            test 'should unescape escaped closing bracket in menu macro' do
            input = 'menu:Preferences[Compile [on\\] Save]'
            para = block_from_string input, attributes: { 'experimental' => '' }
            assert_equal '<span class="menuseq"><b class="menu">Preferences</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">Compile [on] Save</b></span>', para.sub_macros(para.source)
            end

            test 'should process menu with menu item using macro syntax when fonts icons are enabled' do
            para = block_from_string 'menu:Tools[More Tools &gt; Extensions]', attributes: { 'experimental' => '', 'icons' => 'font' }
            assert_equal '<span class="menuseq"><b class="menu">Tools</b>&#160;<i class="fa fa-angle-right caret"></i> <b class="submenu">More Tools</b>&#160;<i class="fa fa-angle-right caret"></i> <b class="menuitem">Extensions</b></span>', para.sub_macros(para.source)
            end

            test 'should process menu with menu item for docbook backend' do
            para = block_from_string 'menu:File[Save As&#8230;]', backend: 'docbook', attributes: { 'experimental' => '' }
            assert_equal '<menuchoice><guimenu>File</guimenu> <guimenuitem>Save As&#8230;</guimenuitem></menuchoice>', para.sub_macros(para.source)
            end

            test 'should process menu with menu item in submenu using macro syntax' do
            para = block_from_string 'menu:Tools[Project &gt; Build]', attributes: { 'experimental' => '' }
            assert_equal '<span class="menuseq"><b class="menu">Tools</b>&#160;<b class="caret">&#8250;</b> <b class="submenu">Project</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">Build</b></span>', para.sub_macros(para.source)
            end

            test 'should process menu with menu item in submenu for docbook backend' do
            para = block_from_string 'menu:Tools[Project &gt; Build]', backend: 'docbook', attributes: { 'experimental' => '' }
            assert_equal '<menuchoice><guimenu>Tools</guimenu> <guisubmenu>Project</guisubmenu> <guimenuitem>Build</guimenuitem></menuchoice>', para.sub_macros(para.source)
            end

            test 'should process menu with menu item in submenu using macro syntax and comma delimiter' do
            para = block_from_string 'menu:Tools[Project, Build]', attributes: { 'experimental' => '' }
            assert_equal '<span class="menuseq"><b class="menu">Tools</b>&#160;<b class="caret">&#8250;</b> <b class="submenu">Project</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">Build</b></span>', para.sub_macros(para.source)
            end

            test 'should process menu with menu item using inline syntax' do
            para = block_from_string '"File &gt; Save As&#8230;"', attributes: { 'experimental' => '' }
            assert_equal '<span class="menuseq"><b class="menu">File</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">Save As&#8230;</b></span>', para.sub_macros(para.source)
            end

            test 'should process menu with menu item in submenu using inline syntax' do
            para = block_from_string '"Tools &gt; Project &gt; Build"', attributes: { 'experimental' => '' }
            assert_equal '<span class="menuseq"><b class="menu">Tools</b>&#160;<b class="caret">&#8250;</b> <b class="submenu">Project</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">Build</b></span>', para.sub_macros(para.source)
            end

            test 'inline menu syntax should not match closing quote of XML attribute' do
            para = block_from_string '<span class="xmltag">&lt;node&gt;</span><span class="classname">r</span>', attributes: { 'experimental' => '' }
            assert_equal '<span class="xmltag">&lt;node&gt;</span><span class="classname">r</span>', para.sub_macros(para.source)
            end

            test 'should process menu macro with items containing multibyte characters' do
            para = block_from_string 'menu:视图[放大, 重置]', attributes: { 'experimental' => '' }
            assert_equal '<span class="menuseq"><b class="menu">视图</b>&#160;<b class="caret">&#8250;</b> <b class="submenu">放大</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">重置</b></span>', para.sub_macros(para.source)
            end

            test 'should process inline menu with items containing multibyte characters' do
            para = block_from_string '"视图 &gt; 放大 &gt; 重置"', attributes: { 'experimental' => '' }
            assert_equal '<span class="menuseq"><b class="menu">视图</b>&#160;<b class="caret">&#8250;</b> <b class="submenu">放大</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">重置</b></span>', para.sub_macros(para.source)
            end

            test 'should process a menu macro with a target that begins with a character reference' do
            para = block_from_string 'menu:&#8942;[More Tools, Extensions]', attributes: { 'experimental' => '' }
            assert_equal '<span class="menuseq"><b class="menu">&#8942;</b>&#160;<b class="caret">&#8250;</b> <b class="submenu">More Tools</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">Extensions</b></span>', para.sub_macros(para.source)
            end

            test 'should not process a menu macro with a target that ends with a space' do
            input = 'menu:foo [bar] menu:File[Save]'
            para = block_from_string input, attributes: { 'experimental' => '' }
            result = para.sub_macros para.source
            assert_xpath '/span[@class="menuseq"]', result, 1
            assert_xpath '//b[@class="menu"][text()="File"]', result, 1
            end

            test 'should process an inline menu that begins with a character reference' do
            para = block_from_string '"&#8942; &gt; More Tools &gt; Extensions"', attributes: { 'experimental' => '' }
            assert_equal '<span class="menuseq"><b class="menu">&#8942;</b>&#160;<b class="caret">&#8250;</b> <b class="submenu">More Tools</b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">Extensions</b></span>', para.sub_macros(para.source)
            end
        end
        end
        "###
        );
    }
}

mod passthroughs {
    use pretty_assertions_sorted::assert_eq;

    use crate::{
        Parser,
        blocks::SimpleBlockStyle,
        content::{Passthroughs, SubstitutionGroup, SubstitutionStep, passthroughs::Passthrough},
        parser::{ModificationContext, QuoteType},
        tests::prelude::*,
    };

    #[test]
    fn collect_inline_triple_plus_passthroughs() {
        let mut content =
            crate::content::Content::from(crate::Span::new("+++<code>inline code</code>+++"));

        let pt = Passthroughs::extract_from(&mut content);

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "+++<code>inline code</code>+++",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "\u{96}0\u{97}",
            }
        );

        assert_eq!(
            pt,
            Passthroughs(vec![Passthrough {
                text: "<code>inline code</code>".to_owned(),
                subs: SubstitutionGroup::None,
                type_: None,
                attrlist: None,
            },],)
        );
    }

    #[test]
    fn collect_inline_triple_plus_passthroughs_with_attrlist() {
        // NOTE: Not in the Ruby test suite.
        let mut content =
            crate::content::Content::from(crate::Span::new("[role]+++<code>inline code</code>+++"));

        let pt = Passthroughs::extract_from(&mut content);

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "[role]+++<code>inline code</code>+++",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "\u{96}0\u{97}",
            }
        );

        assert_eq!(
            pt,
            Passthroughs(vec![Passthrough {
                text: "<code>inline code</code>".to_owned(),
                subs: SubstitutionGroup::None,
                type_: Some(QuoteType::Unquoted,),
                attrlist: Some("role".to_owned(),),
            },],)
        );
    }

    #[test]
    fn collect_multiline_inline_triple_plus_passthroughs() {
        let mut content =
            crate::content::Content::from(crate::Span::new("+++<code>inline\ncode</code>+++"));

        let pt = Passthroughs::extract_from(&mut content);

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "+++<code>inline\ncode</code>+++",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "\u{96}0\u{97}",
            }
        );

        assert_eq!(
            pt,
            Passthroughs(vec![Passthrough {
                text: "<code>inline\ncode</code>".to_owned(),
                subs: SubstitutionGroup::None,
                type_: None,
                attrlist: None,
            },],)
        );
    }

    #[test]
    fn collect_inline_double_dollar_passthroughs() {
        let mut content =
            crate::content::Content::from(crate::Span::new("$$<code>{code}</code>$$"));

        let pt = Passthroughs::extract_from(&mut content);

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "$$<code>{code}</code>$$",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "\u{96}0\u{97}",
            }
        );

        assert_eq!(
            pt,
            Passthroughs(vec![Passthrough {
                text: "<code>{code}</code>".to_owned(),
                subs: SubstitutionGroup::Verbatim,
                type_: None,
                attrlist: None,
            },],)
        );
    }

    #[test]
    fn collect_inline_double_plus_passthroughs() {
        let mut content =
            crate::content::Content::from(crate::Span::new("++<code>{code}</code>++"));

        let pt = Passthroughs::extract_from(&mut content);

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "++<code>{code}</code>++",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "\u{96}0\u{97}",
            }
        );

        assert_eq!(
            pt,
            Passthroughs(vec![Passthrough {
                text: "<code>{code}</code>".to_owned(),
                subs: SubstitutionGroup::Verbatim,
                type_: None,
                attrlist: None,
            },],)
        );
    }

    #[test]
    fn should_not_crash_if_role_on_passthrough_is_enclosed_in_quotes_1() {
        let mut p = Parser::default();
        let maw =
            crate::blocks::Block::parse(crate::Span::new("['role']\\++This++++++++++++"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "['role']\\++This++++++++++++",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "+This+",
                },
                source: Span {
                    data: "['role']\\++This++++++++++++",
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
            },)
        );
    }

    #[test]
    fn should_not_crash_if_role_on_passthrough_is_enclosed_in_quotes_2() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("['role']\\+++++++++This++++++++++++"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "['role']\\+++++++++This++++++++++++",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "++This+",
                },
                source: Span {
                    data: "['role']\\+++++++++This++++++++++++",
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
            },)
        );
    }

    #[test]
    fn should_allow_inline_double_plus_passthrough_to_be_escaped_using_backslash() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("you need to replace `int a = n\\++;` with `int a = ++n;`!"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "you need to replace `int a = n\\++;` with `int a = ++n;`!",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "you need to replace <code>int a = n++;</code> with <code>int a = ++n;</code>!",
                },
                source: Span {
                    data: "you need to replace `int a = n\\++;` with `int a = ++n;`!",
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
            },)
        );
    }

    #[test]
    fn should_allow_inline_double_plus_passthrough_with_attributes_to_be_escaped_using_backslash() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new(r#"=[attrs]\\++text++"#), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"=[attrs]\\++text++"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "=[attrs]++text++",
                },
                source: Span {
                    data: r#"=[attrs]\\++text++"#,
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
            },)
        );
    }

    #[test]
    fn collect_multiline_inline_double_dollar_passthroughs() {
        let mut content =
            crate::content::Content::from(crate::Span::new("$$<code>\n{code}\n</code>$$"));

        let pt = Passthroughs::extract_from(&mut content);

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "$$<code>\n{code}\n</code>$$",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "\u{96}0\u{97}",
            }
        );

        assert_eq!(
            pt,
            Passthroughs(vec![Passthrough {
                text: "<code>\n{code}\n</code>".to_owned(),
                subs: SubstitutionGroup::Verbatim,
                type_: None,
                attrlist: None,
            },],)
        );
    }

    #[test]
    fn collect_passthroughs_from_inline_pass_macro() {
        let mut content = crate::content::Content::from(crate::Span::new(
            "pass:specialcharacters,quotes[<code>['code'\\]</code>]",
        ));

        let pt = Passthroughs::extract_from(&mut content);

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "pass:specialcharacters,quotes[<code>['code'\\]</code>]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "\u{96}0\u{97}",
            }
        );

        assert_eq!(
            pt,
            Passthroughs(vec![Passthrough {
                text: "<code>['code']</code>".to_owned(),
                subs: SubstitutionGroup::Custom(vec![
                    SubstitutionStep::SpecialCharacters,
                    SubstitutionStep::Quotes,
                ]),
                type_: None,
                attrlist: None,
            },],)
        );
    }

    #[test]
    fn collect_multiline_passthroughs_from_inline_pass_macro() {
        let mut content = crate::content::Content::from(crate::Span::new(
            "pass:specialcharacters,quotes[<code>['more\ncode'\\]</code>]",
        ));

        let pt = Passthroughs::extract_from(&mut content);

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "pass:specialcharacters,quotes[<code>['more\ncode'\\]</code>]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "\u{96}0\u{97}",
            }
        );

        assert_eq!(
            pt,
            Passthroughs(vec![Passthrough {
                text: "<code>['more\ncode']</code>".to_owned(),
                subs: SubstitutionGroup::Custom(vec![
                    SubstitutionStep::SpecialCharacters,
                    SubstitutionStep::Quotes,
                ]),
                type_: None,
                attrlist: None,
            },],)
        );
    }

    #[ignore]
    #[test]
    fn should_find_and_replace_placeholder_duplicated_by_substitution() {
        // TO DO: Restore test when macros are supported.

        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new(
                "+first passthrough+ followed by link:$$http://example.com/__u_no_format_me__$$[] with passthrough",
            ),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "+first passthrough+ followed by link:$$http://example.com/__u_no_format_me__$$[] with passthrough",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"first passthrough followed by <a href="http://example.com/__u_no_format_me__" class="bare">http://example.com/__u_no_format_me__</a> with passthrough"#,
                },
                source: Span {
                    data: "+first passthrough+ followed by link:$$http://example.com/__u_no_format_me__$$[] with passthrough",
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
            },)
        );
    }

    #[test]
    fn resolves_sub_shorthands_on_inline_pass_macro() {
        let mut content =
            crate::content::Content::from(crate::Span::new("pass:q,a[*<{backend}>*]"));

        let pt = Passthroughs::extract_from(&mut content);

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "pass:q,a[*<{backend}>*]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "\u{96}0\u{97}",
            }
        );

        assert_eq!(
            pt,
            Passthroughs(vec![Passthrough {
                text: "*<{backend}>*".to_owned(),
                subs: SubstitutionGroup::Custom(vec![
                    SubstitutionStep::Quotes,
                    SubstitutionStep::AttributeReferences,
                ]),
                type_: None,
                attrlist: None,
            },],)
        );

        let parser = Parser::default().with_intrinsic_attribute(
            "backend",
            "html5",
            ModificationContext::ApiOnly,
        );

        pt.0[0].subs.apply(&mut content, &parser, None);

        pt.restore_to(&mut content, &parser);

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "pass:q,a[*<{backend}>*]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "<strong><html5></strong>",
            }
        );
    }

    #[ignore]
    #[test]
    fn inline_pass_macro_supports_incremental_subs() {
        // TO DO: Restore this test once macro substitutions are implemented.
        let mut content = crate::content::Content::from(crate::Span::new("pass:n,-a[<{backend}>]"));
        let pt = Passthroughs::extract_from(&mut content);

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "pass:n,-a[<{backend}>]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "\u{96}0\u{97}",
            }
        );

        assert_eq!(
            pt,
            Passthroughs(vec![Passthrough {
                text: "<{backend}>".to_owned(),
                subs: SubstitutionGroup::Custom(vec![
                    SubstitutionStep::SpecialCharacters,
                    SubstitutionStep::Quotes,
                    SubstitutionStep::CharacterReplacements,
                    SubstitutionStep::Macros,
                    SubstitutionStep::PostReplacement,
                ]),
                type_: None,
                attrlist: None,
            },],)
        );

        let parser = Parser::default().with_intrinsic_attribute(
            "backend",
            "html5",
            ModificationContext::ApiOnly,
        );

        pt.0[0].subs.apply(&mut content, &parser, None);

        pt.restore_to(&mut content, &parser);

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "pass:q,a[*<{backend}>*]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "&lt;{backend}&gt;",
            }
        );
    }

    #[test]
    fn should_not_recognize_pass_macro_with_invalid_substitution_list_1() {
        let mut content = crate::content::Content::from(crate::Span::new("pass:,[foobar]"));
        let pt = Passthroughs::extract_from(&mut content);

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "pass:,[foobar]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "pass:,[foobar]",
            }
        );

        assert!(pt.0.is_empty());
    }

    #[test]
    fn should_not_recognize_pass_macro_with_invalid_substitution_list_2() {
        let mut content = crate::content::Content::from(crate::Span::new("pass:42[foobar]"));
        let pt = Passthroughs::extract_from(&mut content);

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "pass:42[foobar]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "pass:42[foobar]",
            }
        );

        assert!(pt.0.is_empty());
    }

    #[test]
    fn should_not_recognize_pass_macro_with_invalid_substitution_list_3() {
        let mut content = crate::content::Content::from(crate::Span::new("pass:a,[foobar]"));
        let pt = Passthroughs::extract_from(&mut content);

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "pass:a,[foobar]",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "pass:a,[foobar]",
            }
        );

        assert!(pt.0.is_empty());
    }

    #[test]
    fn should_allow_content_of_inline_pass_macro_to_be_empty() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new("pass:[]"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "pass:[]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "",
                },
                source: Span {
                    data: "pass:[]",
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
            },)
        );
    }

    #[test]
    fn restore_inline_passthroughs_without_subs() {
        // NOTE: Placeholder is surrounded by text to prevent reader from stripping
        // trailing boundary char (unique to test scenario).
        let mut content =
            crate::content::Content::from(crate::Span::new("some \u{96}0\u{97} to study"));

        let pt = Passthroughs(vec![Passthrough {
            text: "<code>inline code</code>".to_owned(),
            subs: SubstitutionGroup::None,
            type_: None,
            attrlist: None,
        }]);

        let parser = Parser::default();
        pt.restore_to(&mut content, &parser);

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "some \u{96}0\u{97} to study",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "some <code>inline code</code> to study",
            }
        );
    }

    #[test]
    fn restore_inline_passthroughs_with_subs() {
        // NOTE: Placeholder is surrounded by text to prevent reader from stripping
        // trailing boundary char (unique to test scenario).
        let mut content = crate::content::Content::from(crate::Span::new(
            "some \u{96}0\u{97} to study in the \u{96}1\u{97} programming language",
        ));

        let pt = Passthroughs(vec![
            Passthrough {
                text: "<code>{code}</code>".to_owned(),
                subs: SubstitutionGroup::Custom(vec![SubstitutionStep::SpecialCharacters]),
                type_: None,
                attrlist: None,
            },
            Passthrough {
                text: "{language}".to_owned(),
                subs: SubstitutionGroup::Custom(vec![SubstitutionStep::SpecialCharacters]),
                type_: None,
                attrlist: None,
            },
        ]);

        let parser = Parser::default();
        pt.restore_to(&mut content, &parser);

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "some \u{96}0\u{97} to study in the \u{96}1\u{97} programming language",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "some &lt;code&gt;{code}&lt;/code&gt; to study in the {language} programming language",
            }
        );
    }

    #[test]
    fn should_restore_nested_passthroughs() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("+Sometimes you feel pass:q[`mono`].+ Sometimes you +$$don't$$+."),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "+Sometimes you feel pass:q[`mono`].+ Sometimes you +$$don't$$+.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "Sometimes you feel <code>mono</code>. Sometimes you don't.",
                },
                source: Span {
                    data: "+Sometimes you feel pass:q[`mono`].+ Sometimes you +$$don't$$+.",
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
            },)
        );
    }

    #[ignore]
    #[test]
    fn should_not_fail_to_restore_remaining_passthroughs_after_processing_inline_passthrough_with_macro_substitution()
     {
        // TO DO: Enable this test when macro substitution is implemented.
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new("pass:m[.] pass:[.]"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "pass:m[.] pass:[.]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: ". .",
                },
                source: Span {
                    data: "pass:m[.] pass:[.]",
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
            },)
        );
    }

    #[test]
    fn should_honor_role_on_double_dollar_passthrough() {
        // NOTE: Not in the Ruby test suite.
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("Print the version using [var]$${asciidoctor-version}$$."),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "Print the version using [var]$${asciidoctor-version}$$.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"Print the version using <span class="var">{asciidoctor-version}</span>."#,
                },
                source: Span {
                    data: "Print the version using [var]$${asciidoctor-version}$$.",
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
            },)
        );
    }

    #[test]
    fn should_honor_role_on_double_plus_passthrough() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("Print the version using [var]++{asciidoctor-version}++."),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "Print the version using [var]++{asciidoctor-version}++.",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"Print the version using <span class="var">{asciidoctor-version}</span>."#,
                },
                source: Span {
                    data: "Print the version using [var]++{asciidoctor-version}++.",
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
            },)
        );
    }

    #[test]
    fn complex_inline_passthrough_macro_1() {
        let mut content = crate::content::Content::from(crate::Span::new(
            "$$[(] <'basic form'> <'logical operator'> <'basic form'> [)]$$",
        ));

        let pt = Passthroughs::extract_from(&mut content);

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: "$$[(] <'basic form'> <'logical operator'> <'basic form'> [)]$$",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "\u{96}0\u{97}",
            }
        );

        assert_eq!(
            pt,
            Passthroughs(vec![Passthrough {
                text: "[(] <'basic form'> <'logical operator'> <'basic form'> [)]".to_owned(),
                subs: SubstitutionGroup::Verbatim,
                type_: None,
                attrlist: None,
            },],)
        );
    }

    #[test]
    fn complex_inline_passthrough_macro_2() {
        let mut content = crate::content::Content::from(crate::Span::new(
            r#"pass:specialcharacters[[(\] <'basic form'> <'logical operator'> <'basic form'> [)\]]"#,
        ));

        let pt = Passthroughs::extract_from(&mut content);

        assert_eq!(
            content,
            Content {
                original: Span {
                    data: r#"pass:specialcharacters[[(\] <'basic form'> <'logical operator'> <'basic form'> [)\]]"#,
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                rendered: "\u{96}0\u{97}",
            }
        );

        assert_eq!(
            pt,
            Passthroughs(vec![Passthrough {
                text: r#"[(] <'basic form'> <'logical operator'> <'basic form'> [)]"#.to_owned(),
                subs: SubstitutionGroup::Custom(vec![SubstitutionStep::SpecialCharacters,],),
                type_: None,
                attrlist: None,
            },],)
        );
    }

    #[test]
    fn inline_pass_macro_with_a_composite_sub() {
        let mut p = Parser::default();
        let maw =
            crate::blocks::Block::parse(crate::Span::new("pass:verbatim[<{backend}>]"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "pass:verbatim[<{backend}>]",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"&lt;{backend}&gt;"#,
                },
                source: Span {
                    data: "pass:verbatim[<{backend}>]",
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
            },)
        );
    }

    #[test]
    fn should_support_constrained_passthrough_in_middle_of_monospace_span() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("a `foo +bar+ baz` kind of thing"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "a `foo +bar+ baz` kind of thing",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "a <code>foo bar baz</code> kind of thing",
                },
                source: Span {
                    data: "a `foo +bar+ baz` kind of thing",
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
            },)
        );
    }

    #[test]
    fn should_support_constrained_passthrough_in_monospace_span_preceded_by_escaped_boxed_attrlist_with_transitional_role()
     {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new(r#"\[x-]`foo +bar+ baz`"#), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"\[x-]`foo +bar+ baz`"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "[x-]<code>foo bar baz</code>",
                },
                source: Span {
                    data: r#"\[x-]`foo +bar+ baz`"#,
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
            },)
        );
    }

    #[test]
    fn should_treat_monospace_phrase_with_escaped_boxed_attrlist_with_transitional_role_as_monospace()
     {
        let mut p = Parser::default();
        let maw =
            crate::blocks::Block::parse(crate::Span::new(r#"\[x-]`*foo* +bar+ baz`"#), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"\[x-]`*foo* +bar+ baz`"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "[x-]<code><strong>foo</strong> bar baz</code>",
                },
                source: Span {
                    data: r#"\[x-]`*foo* +bar+ baz`"#,
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
            },)
        );
    }

    #[test]
    fn should_ignore_escaped_attrlist_with_transitional_role_on_monospace_phrase_if_not_proceeded_by_bracket()
     {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new(r#"\x-]`*foo* +bar+ baz`"#), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"\x-]`*foo* +bar+ baz`"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"\x-]<code><strong>foo</strong> bar baz</code>"#,
                },
                source: Span {
                    data: r#"\x-]`*foo* +bar+ baz`"#,
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
            },)
        );
    }

    #[test]
    fn should_not_process_passthrough_inside_transitional_literal_monospace_span() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("a [x-]`foo +bar+ baz` kind of thing"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "a [x-]`foo +bar+ baz` kind of thing",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "a <code>foo +bar+ baz</code> kind of thing",
                },
                source: Span {
                    data: "a [x-]`foo +bar+ baz` kind of thing",
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
            },)
        );
    }

    #[test]
    fn should_support_constrained_passthrough_in_monospace_phrase_with_attrlist() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new("[.role]`foo +bar+ baz`"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "[.role]`foo +bar+ baz`",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<code class="role">foo bar baz</code>"#,
                },
                source: Span {
                    data: "[.role]`foo +bar+ baz`",
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
            },)
        );
    }

    #[test]
    fn should_support_attrlist_on_a_literal_monospace_phrase() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(crate::Span::new("[.baz]`+foo--bar+`"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "[.baz]`+foo--bar+`",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"<code class="baz">foo--bar</code>"#,
                },
                source: Span {
                    data: "[.baz]`+foo--bar+`",
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
            },)
        );
    }

    #[test]
    fn should_not_process_an_escaped_passthrough_macro_inside_a_monospaced_phrase() {
        let mut p = Parser::default();
        let maw =
            crate::blocks::Block::parse(crate::Span::new(r#"use the `\pass:c[]` macro"#), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"use the `\pass:c[]` macro"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "use the <code>pass:c[]</code> macro",
                },
                source: Span {
                    data: r#"use the `\pass:c[]` macro"#,
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
            },)
        );
    }

    #[test]
    fn should_not_process_an_escaped_passthrough_macro_inside_a_monospaced_phrase_with_attributes()
    {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new(r#"use the [syntax]`\pass:c[]` macro"#),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"use the [syntax]`\pass:c[]` macro"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"use the <code class="syntax">pass:c[]</code> macro"#,
                },
                source: Span {
                    data: r#"use the [syntax]`\pass:c[]` macro"#,
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
            },)
        );
    }

    #[test]
    fn should_honor_an_escaped_single_plus_passthrough_inside_a_monospaced_phrase() {
        let mut p = Parser::default().with_intrinsic_attribute(
            "author",
            "Dan",
            ModificationContext::Anywhere,
        );

        let maw = crate::blocks::Block::parse(
            crate::Span::new(r#"use `\+{author}+` to show an attribute reference"#),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"use `\+{author}+` to show an attribute reference"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: r#"use <code>+Dan+</code> to show an attribute reference"#,
                },
                source: Span {
                    data: r#"use `\+{author}+` to show an attribute reference"#,
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
            },)
        );
    }

    mod math_macros {
        #[ignore]
        #[test]
        fn not_implemented() {
            todo!("Review Ruby Asciidoctor test suite for `context 'Math macros'`");
            // See https://github.com/asciidoc-rs/asciidoc-parser/issues/261.
        }
    }
}

mod replacements {
    use pretty_assertions_sorted::assert_eq;

    use crate::{
        Parser, blocks::SimpleBlockStyle, content::SubstitutionStep, strings::CowStr,
        tests::prelude::*,
    };

    #[test]
    fn unescapes_xml_entities() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("< &quot; &there4; &#34; &#x22; >"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "< &quot; &there4; &#34; &#x22; >",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "&lt; &quot; &there4; &#34; &#x22; &gt;",
                },
                source: Span {
                    data: "< &quot; &there4; &#34; &#x22; >",
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
            },)
        );
    }

    #[test]
    fn replaces_arrows() {
        let mut p = Parser::default();
        let maw =
            crate::blocks::Block::parse(crate::Span::new(r#"<- -> <= => \<- \-> \<= \=>"#), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"<- -> <= => \<- \-> \<= \=>"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "&#8592; &#8594; &#8656; &#8658; &lt;- -&gt; &lt;= =&gt;",
                },
                source: Span {
                    data: r#"<- -> <= => \<- \-> \<= \=>"#,
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
            },)
        );
    }

    #[test]
    fn replaces_dashes() {
        let mut content = crate::content::Content::from(crate::Span::new(
            r#"-- foo foo--bar foo\--bar foo -- bar foo \-- bar
stuff in between
-- foo
stuff in between
foo --
stuff in between
foo --
"#,
        ));

        let expected = r#"&#8201;&#8212;&#8201;foo foo&#8212;&#8203;bar foo--bar foo&#8201;&#8212;&#8201;bar foo -- bar
stuff in between&#8201;&#8212;&#8201;foo
stuff in between
foo&#8201;&#8212;&#8201;stuff in between
foo&#8201;&#8212;&#8201;"#;

        let p = Parser::default();
        SubstitutionStep::CharacterReplacements.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(expected.to_string().into_boxed_str())
        );
    }

    #[test]
    fn replaces_dashes_between_multibyte_word_characters() {
        let mut content = crate::content::Content::from(crate::Span::new("富--巴"));

        let p = Parser::default();
        SubstitutionStep::CharacterReplacements.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed("富&#8212;&#8203;巴".to_string().into_boxed_str())
        );
    }

    #[test]
    fn replaces_marks() {
        let mut content =
            crate::content::Content::from(crate::Span::new(r#"(C) (R) (TM) \(C) \(R) \(TM)"#));

        let p = Parser::default();
        SubstitutionStep::CharacterReplacements.apply(&mut content, &p, None);

        assert_eq!(
            content.rendered,
            CowStr::Boxed(
                "&#169; &#174; &#8482; (C) (R) (TM)"
                    .to_string()
                    .into_boxed_str()
            )
        );
    }

    #[test]
    fn preserves_entity_references() {
        let input = "&amp; &#169; &#10004; &#128512; &#x2022; &#x1f600;";

        let mut content = crate::content::Content::from(crate::Span::new(input));
        let p = Parser::default();
        SubstitutionStep::CharacterReplacements.apply(&mut content, &p, None);
        assert_eq!(
            content.rendered,
            CowStr::Boxed(input.to_string().into_boxed_str())
        );
    }

    #[test]
    fn only_preserves_named_entities_with_two_or_more_letters() {
        let input = "&amp; &a; &gt;";

        let mut content = crate::content::Content::from(crate::Span::new(input));
        let p = Parser::default();
        SubstitutionStep::CharacterReplacements.apply(&mut content, &p, None);

        assert_eq!(
            content.rendered,
            CowStr::Boxed(input.to_string().into_boxed_str())
        );
    }

    #[test]
    fn replaces_punctuation() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new(r#"John's Hideout is the Whites`' place... foo\'bar"#),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: r#"John's Hideout is the Whites`' place... foo\'bar"#,
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "John&#8217;s Hideout is the Whites&#8217; place&#8230;&#8203; foo'bar",
                },
                source: Span {
                    data: r#"John's Hideout is the Whites`' place... foo\'bar"#,
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
            },)
        );
    }

    #[ignore]
    #[test]
    fn todo_migrate_from_ruby() {
        todo!(
            "{}",
            r###"
      test 'should replace right single quote marks' do
        given = [
          %(`'Twas the night),
          %(a `'57 Chevy!),
          %(the whites`' place),
          %(the whites`'.),
          %(the whites`'--where the wild things are),
          %(the whites`'\nhave),
          %(It's Mary`'s little lamb.),
          %(consecutive single quotes '' are not modified),
          %(he is 6' tall),
          %(\\`'),
        ]
        expected = [
          %(&#8217;Twas the night),
          %(a &#8217;57 Chevy!),
          %(the whites&#8217; place),
          %(the whites&#8217;.),
          %(the whites&#8217;--where the wild things are),
          %(the whites&#8217;\nhave),
          %(It&#8217;s Mary&#8217;s little lamb.),
          %(consecutive single quotes '' are not modified),
          %(he is 6' tall),
          %(`'),
        ]
        given.size.times do |i|
          para = block_from_string given[i]
          assert_equal expected[i], para.sub_replacements(para.source)
        end
      end
    end
    "###
        );
    }
}

mod post_replacements {
    use pretty_assertions_sorted::assert_eq;

    use crate::{Parser, blocks::SimpleBlockStyle, tests::prelude::*};

    #[test]
    fn line_break_inserted_after_line_with_line_break_character() {
        let mut p = Parser::default();
        let maw =
            crate::blocks::Block::parse(crate::Span::new("First line +\nSecond line"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "First line +\nSecond line",
                        line: 1,
                        col: 1,
                        offset: 0,
                    },
                    rendered: "First line<br>\nSecond line",
                },
                source: Span {
                    data: "First line +\nSecond line",
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
            },)
        );
    }

    #[test]
    fn line_break_inserted_after_line_wrap_with_hardbreaks_enabled() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("[%hardbreaks]\nFirst line\nSecond line"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "First line\nSecond line",
                        line: 2,
                        col: 1,
                        offset: 14,
                    },
                    rendered: "First line<br>\nSecond line",
                },
                source: Span {
                    data: "[%hardbreaks]\nFirst line\nSecond line",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: Some(Attrlist {
                    attributes: &[ElementAttribute {
                        name: None,
                        shorthand_items: &["%hardbreaks"],
                        value: "%hardbreaks"
                    },],
                    anchor: None,
                    source: Span {
                        data: "%hardbreaks",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },)
        );
    }

    #[test]
    fn line_break_character_stripped_from_end_of_line_with_hardbreaks_enabled() {
        let mut p = Parser::default();
        let maw = crate::blocks::Block::parse(
            crate::Span::new("[%hardbreaks]\nFirst line +\nSecond line"),
            &mut p,
        );

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "First line +\nSecond line",
                        line: 2,
                        col: 1,
                        offset: 14,
                    },
                    rendered: "First line<br>\nSecond line",
                },
                source: Span {
                    data: "[%hardbreaks]\nFirst line +\nSecond line",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: Some(Attrlist {
                    attributes: &[ElementAttribute {
                        name: None,
                        shorthand_items: &["%hardbreaks"],
                        value: "%hardbreaks"
                    },],
                    anchor: None,
                    source: Span {
                        data: "%hardbreaks",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },)
        );
    }

    #[test]
    fn line_break_not_inserted_for_single_line_with_hardbreaks_enabled() {
        let mut p = Parser::default();
        let maw =
            crate::blocks::Block::parse(crate::Span::new("[%hardbreaks]\nFirst line"), &mut p);

        let block = maw.item.unwrap().item;

        assert_eq!(
            block,
            Block::Simple(SimpleBlock {
                content: Content {
                    original: Span {
                        data: "First line",
                        line: 2,
                        col: 1,
                        offset: 14,
                    },
                    rendered: "First line",
                },
                source: Span {
                    data: "[%hardbreaks]\nFirst line",
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                style: SimpleBlockStyle::Paragraph,
                title_source: None,
                title: None,
                anchor: None,
                anchor_reftext: None,
                attrlist: Some(Attrlist {
                    attributes: &[ElementAttribute {
                        name: None,
                        shorthand_items: &["%hardbreaks"],
                        value: "%hardbreaks"
                    },],
                    anchor: None,
                    source: Span {
                        data: "%hardbreaks",
                        line: 1,
                        col: 2,
                        offset: 1,
                    },
                },),
            },)
        );
    }
}

mod resolve_subs {
    // use pretty_assertions_sorted::assert_eq;

    #[ignore]
    #[test]
    fn todo_migrate_from_ruby() {
        todo!(
            "{}",
            r###"

    context 'Resolve subs' do
      test 'should resolve subs for block' do
        doc = empty_document parse: true
        block = Asciidoctor::Block.new doc, :paragraph
        block.attributes['subs'] = 'quotes,normal'
        block.commit_subs
        assert_equal [:quotes, :specialcharacters, :attributes, :replacements, :macros, :post_replacements], block.subs
      end

      test 'should resolve specialcharacters sub as highlight for source block when source highlighter is coderay' do
        doc = empty_document attributes: { 'source-highlighter' => 'coderay' }, parse: true
        block = Asciidoctor::Block.new doc, :listing, content_model: :verbatim
        block.style = 'source'
        block.attributes['subs'] = 'specialcharacters'
        block.attributes['language'] = 'ruby'
        block.commit_subs
        assert_equal [:highlight], block.subs
      end

      test 'should resolve specialcharacters sub as highlight for source block when source highlighter is pygments', if: ENV['PYGMENTS_VERSION'] do
        doc = empty_document attributes: { 'source-highlighter' => 'pygments' }, parse: true
        block = Asciidoctor::Block.new doc, :listing, content_model: :verbatim
        block.style = 'source'
        block.attributes['subs'] = 'specialcharacters'
        block.attributes['language'] = 'ruby'
        block.commit_subs
        assert_equal [:highlight], block.subs
      end

      test 'should not replace specialcharacters sub with highlight for source block when source highlighter is not set' do
        doc = empty_document parse: true
        block = Asciidoctor::Block.new doc, :listing, content_model: :verbatim
        block.style = 'source'
        block.attributes['subs'] = 'specialcharacters'
        block.attributes['language'] = 'ruby'
        block.commit_subs
        assert_equal [:specialcharacters], block.subs
      end

      test 'should not use subs if subs option passed to block constructor is nil' do
        doc = empty_document parse: true
        block = Asciidoctor::Block.new doc, :paragraph, source: '*bold* _italic_', subs: nil, attributes: { 'subs' => 'quotes' }
        assert_empty block.subs
        block.commit_subs
        assert_empty block.subs
      end

      test 'should not use subs if subs option passed to block constructor is empty array' do
        doc = empty_document parse: true
        block = Asciidoctor::Block.new doc, :paragraph, source: '*bold* _italic_', subs: [], attributes: { 'subs' => 'quotes' }
        assert_empty block.subs
        block.commit_subs
        assert_empty block.subs
      end

      test 'should use subs from subs option passed to block constructor' do
        doc = empty_document parse: true
        block = Asciidoctor::Block.new doc, :paragraph, source: '*bold* _italic_', subs: [:specialcharacters], attributes: { 'subs' => 'quotes' }
        assert_equal [:specialcharacters], block.subs
        block.commit_subs
        assert_equal [:specialcharacters], block.subs
      end

      test 'should use subs from subs attribute if subs option is not passed to block constructor' do
        doc = empty_document parse: true
        block = Asciidoctor::Block.new doc, :paragraph, source: '*bold* _italic_', attributes: { 'subs' => 'quotes' }
        assert_empty block.subs
        # in this case, we have to call commit_subs to resolve the subs
        block.commit_subs
        assert_equal [:quotes], block.subs
      end

      test 'should use subs from subs attribute if subs option passed to block constructor is :default' do
        doc = empty_document parse: true
        block = Asciidoctor::Block.new doc, :paragraph, source: '*bold* _italic_', subs: :default, attributes: { 'subs' => 'quotes' }
        assert_equal [:quotes], block.subs
        block.commit_subs
        assert_equal [:quotes], block.subs
      end

      test 'should use built-in subs if subs option passed to block constructor is :default and subs attribute is absent' do
        doc = empty_document parse: true
        block = Asciidoctor::Block.new doc, :paragraph, source: '*bold* _italic_', subs: :default
        assert_equal [:specialcharacters, :quotes, :attributes, :replacements, :macros, :post_replacements], block.subs
        block.commit_subs
        assert_equal [:specialcharacters, :quotes, :attributes, :replacements, :macros, :post_replacements], block.subs
      end
    end
    "###
        );
    }
}
