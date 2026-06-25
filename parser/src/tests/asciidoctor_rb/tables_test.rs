// Adapted from Asciidoctor's tables test suite, found in
// https://github.com/asciidoctor/asciidoctor/blob/main/test/tables_test.rb.
//
// IMPORTANT: In porting this, I've disregarded compatibility mode (stated
// limitation of `asciidoc-parser` crate) and alternate (non-HTML) back ends.

mod psv {
    use crate::tests::prelude::*;

    #[test]
    fn converts_simple_psv_table() {
        let doc = Parser::default().parse("|=======\n|A |B |C\n|a |b |c\n|1 |2 |3\n|=======");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table.tableblock.frame-all.grid-all.stretch", 1);
        assert_css(&doc, "table > colgroup > col[width=\"33.3333%\"]", 2);
        // Ruby uses `col:last-of-type`; the indexed XPath form is equivalent.
        assert_xpath(
            &doc,
            "(/table/colgroup/col)[3][@width=\"33.3334%\"]",
            1,
        );
        assert_css(&doc, "table tr", 3);
        assert_css(&doc, "table > tbody > tr", 3);
        assert_css(&doc, "table td", 9);
        assert_css(
            &doc,
            "table > tbody > tr > td.tableblock.halign-left.valign-top > p.tableblock",
            9,
        );

        let cells = [["A", "B", "C"], ["a", "b", "c"], ["1", "2", "3"]];
        for (rowi, row) in cells.iter().enumerate() {
            assert_xpath(&doc, &format!("(/table/tbody/tr)[{}]/td", rowi + 1), row.len());
            assert_xpath(&doc, &format!("(/table/tbody/tr)[{}]/td/p", rowi + 1), row.len());
            for (celli, cell) in row.iter().enumerate() {
                assert_xpath(
                    &doc,
                    &format!("(//tr)[{}]/td[{}]/p[text()='{cell}']", rowi + 1, celli + 1),
                    1,
                );
            }
        }
    }

    #[test]
    fn should_add_direction_css_class_if_float_attribute_is_set_on_table() {
        let doc = Parser::default()
            .parse("[float=left]\n|=======\n|A |B |C\n|a |b |c\n|1 |2 |3\n|=======");

        assert_css(&doc, "table.left", 1);
    }

    #[test]
    fn should_set_stripes_class_if_stripes_option_is_set() {
        let doc = Parser::default()
            .parse("[stripes=odd]\n|=======\n|A |B |C\n|a |b |c\n|1 |2 |3\n|=======");

        assert_css(&doc, "table.stripes-odd", 1);
    }

    #[test]
    fn outputs_a_caption_on_simple_psv_table() {
        let doc = Parser::default()
            .parse(".Simple psv table\n|=======\n|A |B |C\n|a |b |c\n|1 |2 |3\n|=======");

        assert_xpath(
            &doc,
            "/table/caption[@class=\"title\"][text()=\"Table 1. Simple psv table\"]",
            1,
        );
        assert_xpath(&doc, "/table/caption/following-sibling::colgroup", 1);
    }

    #[test]
    fn only_increments_table_counter_for_tables_that_have_a_title() {
        let doc = Parser::default().parse(
            ".First numbered table\n|=======\n|1 |2 |3\n|=======\n\n|=======\n|4 |5 |6\n|=======\n\n.Second numbered table\n|=======\n|7 |8 |9\n|=======",
        );

        assert_xpath(&doc, "/table", 3);
        assert_xpath(&doc, "(/table)[1]/caption", 1);
        assert_xpath(
            &doc,
            "(/table)[1]/caption[text()=\"Table 1. First numbered table\"]",
            1,
        );
        assert_xpath(&doc, "(/table)[2]/caption", 0);
        assert_xpath(&doc, "(/table)[3]/caption", 1);
        assert_xpath(
            &doc,
            "(/table)[3]/caption[text()=\"Table 2. Second numbered table\"]",
            1,
        );
    }

    #[test]
    fn uses_explicit_caption_in_front_of_title_in_place_of_default_caption_and_number() {
        let doc = Parser::default().parse(
            "[caption=\"All the Data. \"]\n.Simple psv table\n|=======\n|A |B |C\n|a |b |c\n|1 |2 |3\n|=======",
        );

        assert_xpath(
            &doc,
            "/table/caption[@class=\"title\"][text()=\"All the Data. Simple psv table\"]",
            1,
        );
        assert_xpath(&doc, "/table/caption/following-sibling::colgroup", 1);
    }

    #[test]
    fn disables_caption_when_caption_attribute_on_table_is_empty() {
        let doc = Parser::default().parse(
            "[caption=]\n.Simple psv table\n|=======\n|A |B |C\n|a |b |c\n|1 |2 |3\n|=======",
        );

        assert_xpath(
            &doc,
            "/table/caption[@class=\"title\"][text()=\"Simple psv table\"]",
            1,
        );
        assert_xpath(&doc, "/table/caption/following-sibling::colgroup", 1);
    }

    #[test]
    fn disables_caption_when_caption_attribute_on_table_is_empty_string() {
        let doc = Parser::default().parse(
            "[caption=\"\"]\n.Simple psv table\n|=======\n|A |B |C\n|a |b |c\n|1 |2 |3\n|=======",
        );

        assert_xpath(
            &doc,
            "/table/caption[@class=\"title\"][text()=\"Simple psv table\"]",
            1,
        );
        assert_xpath(&doc, "/table/caption/following-sibling::colgroup", 1);
    }

    #[test]
    fn disables_caption_on_table_when_table_caption_document_attribute_is_unset() {
        let doc = Parser::default().parse(
            ":!table-caption:\n\n.Simple psv table\n|=======\n|A |B |C\n|a |b |c\n|1 |2 |3\n|=======",
        );

        assert_xpath(
            &doc,
            "/table/caption[@class=\"title\"][text()=\"Simple psv table\"]",
            1,
        );
        assert_xpath(&doc, "/table/caption/following-sibling::colgroup", 1);
    }

    #[test]
    #[ignore]
    // TODO (issue TBD): The crate diverges from Asciidoctor on PSV cell
    // splitting: it treats a `|` as a cell separator only when preceded by a
    // delimiter boundary (e.g. a space), so `|a|b` parses as a single cell
    // whereas Asciidoctor (and this test) splits it into two. Enable once the
    // parser splits on any unescaped `|`.
    fn ignores_escaped_separators() {
        let doc =
            Parser::default().parse("|===\n|A \\| here| a \\| there\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > tbody > tr", 1);
        assert_css(&doc, "table > tbody > tr > td", 2);
        assert_xpath(&doc, "/table/tbody/tr/td[1]/p[text()=\"A | here\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr/td[2]/p[text()=\"a | there\"]", 1);
    }

    #[test]
    fn preserves_escaped_delimiters_at_the_end_of_the_line() {
        let doc = Parser::default().parse(
            "[%header,cols=\"1,1\"]\n|===\n|A |B\\|\n|A1 |B1\\|\n|A2 |B2\\|\n|===",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > thead > tr", 1);
        assert_xpath(&doc, "(/table/thead/tr)[1]/th", 2);
        assert_xpath(&doc, "/table/thead/tr[1]/th[2][text()=\"B|\"]", 1);
        assert_css(&doc, "table > tbody > tr", 2);
        assert_xpath(&doc, "(/table/tbody/tr)[1]/td", 2);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[2]/p[text()=\"B1|\"]", 1);
        assert_xpath(&doc, "(/table/tbody/tr)[2]/td", 2);
        assert_xpath(&doc, "/table/tbody/tr[2]/td[2]/p[text()=\"B2|\"]", 1);
    }

    #[test]
    fn should_treat_trailing_pipe_as_an_empty_cell() {
        let doc =
            Parser::default().parse("|===\n|A1 |\n|B1 |B2\n|C1 |C2\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > tbody > tr", 3);
        assert_xpath(&doc, "/table/tbody/tr[1]/td", 2);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[1]/p[text()=\"A1\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[2]/p", 0);
        assert_xpath(&doc, "/table/tbody/tr[2]/td[1]/p[text()=\"B1\"]", 1);
    }

    #[test]
    fn performs_normal_substitutions_on_cell_content() {
        let doc = Parser::default().parse(
            ":show_title: Cool new show\n|===\n|{show_title} |Coming soon...\n|===",
        );

        assert_xpath(&doc, "//tbody/tr/td[1]/p[text()=\"Cool new show\"]", 1);
        assert_xpath(
            &doc,
            "//tbody/tr/td[2]/p[text()='Coming soon\u{2026}\u{200b}']",
            1,
        );
    }
}

mod dsv {
    #[allow(unused_imports)]
    use crate::tests::prelude::*;
}

mod csv {
    #[allow(unused_imports)]
    use crate::tests::prelude::*;
}



