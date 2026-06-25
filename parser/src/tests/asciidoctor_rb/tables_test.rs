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

    #[test]
    fn should_only_substitute_specialchars_for_literal_table_cells() {
        let doc = Parser::default().parse("|===\nl|one\n*two*\nthree\n<four>\n|===");

        // Ruby compares the serialized `<pre>one\n*two*\nthree\n&lt;four&gt;</pre>`;
        // the test DOM decodes entities in `text()`, so this asserts the decoded
        // content (formatting markup left literal, specialchars escaped then
        // decoded back).
        assert_css(&doc, "table pre", 1);
        assert_xpath(&doc, "/table//pre[text()=\"one\n*two*\nthree\n<four>\"]", 1);
    }

    #[test]
    #[ignore]
    // TODO (issue TBD): The crate strips the leading indentation from the first
    // content line of a literal (`l`) cell, producing "one\n  two\nthree"
    // instead of Asciidoctor's "  one\n  two\nthree". A literal cell should
    // preserve leading spaces on every line. Enable once fixed.
    fn should_preserve_leading_spaces_but_not_leading_newlines_or_trailing_spaces_in_literal_table_cells()
     {
        let doc = Parser::default()
            .parse("[cols=2*]\n|===\nl|\n  one\n  two\nthree\n\n  | normal\n|===");

        assert_css(&doc, "table pre", 1);
        assert_xpath(&doc, "/table//pre[text()=\"  one\n  two\nthree\"]", 1);
    }

    #[test]
    fn should_ignore_v_table_cell_style() {
        let doc = Parser::default()
            .parse("[cols=2*]\n|===\nv|\n  one\n  two\nthree\n\n  | normal\n|===");

        // The unrecognized `v` style is ignored, so the cell renders as a normal
        // paragraph (`p.tableblock`) with leading newlines and trailing spaces
        // stripped but interior indentation preserved.
        assert_xpath(
            &doc,
            "(/table/tbody/tr/td)[1]/p[@class=\"tableblock\"][text()=\"one\n  two\nthree\"]",
            1,
        );
    }

    #[test]
    fn table_and_column_width_not_assigned_when_autowidth_option_is_specified() {
        let doc = Parser::default().parse(
            "[options=\"autowidth\"]\n|=======\n|A |B |C\n|a |b |c\n|1 |2 |3\n|=======",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table.fit-content", 1);
        assert_css(&doc, "table[style*=\"width\"]", 0);
        assert_css(&doc, "table colgroup col", 3);
        assert_css(&doc, "table colgroup col[style*=\"width\"]", 0);
    }

    #[test]
    fn does_not_assign_column_width_for_autowidth_columns_in_html_output() {
        let doc = Parser::default().parse(
            "[cols=\"15%,3*~\"]\n|=======\n|A |B |C |D\n|a |b |c |d\n|1 |2 |3 |4\n|=======",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table colgroup col", 4);
        assert_css(&doc, "table colgroup col[width]", 1);
        assert_css(&doc, "table colgroup col[width=\"15%\"]", 1);
    }

    #[test]
    fn can_assign_autowidth_to_all_columns_even_when_table_has_a_width() {
        let doc = Parser::default().parse(
            "[cols=\"4*~\",width=50%]\n|=======\n|A |B |C |D\n|a |b |c |d\n|1 |2 |3 |4\n|=======",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table[width=\"50%\"]", 1);
        assert_css(&doc, "table colgroup col", 4);
        assert_css(&doc, "table colgroup col[style]", 0);
    }

    // Backend-specific test omitted: DocBook ("equally distributes remaining
    // column width to autowidth columns in DocBook output").

    // Backend-specific test omitted: DocBook ("should compute column widths
    // based on pagewidth when width is set on table in DocBook output").

    #[test]
    fn explicit_table_width_is_used_even_when_autowidth_option_is_specified() {
        let doc = Parser::default().parse(
            "[%autowidth,width=75%]\n|=======\n|A |B |C\n|a |b |c\n|1 |2 |3\n|=======",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table[width]", 1);
        assert_css(&doc, "table colgroup col", 3);
        assert_css(&doc, "table colgroup col[style*=\"width\"]", 0);
    }

    #[test]
    fn first_row_sets_number_of_columns_when_not_specified() {
        let doc = Parser::default()
            .parse("|===\n|first |second |third |fourth\n|1 |2 |3\n|4\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 4);
        assert_css(&doc, "table > tbody > tr", 2);
        assert_xpath(&doc, "(/table/tbody/tr)[1]/td", 4);
        assert_xpath(&doc, "(/table/tbody/tr)[2]/td", 4);
    }

    #[test]
    fn colspec_attribute_using_asterisk_syntax_sets_number_of_columns() {
        let doc = Parser::default()
            .parse("[cols=\"3*\"]\n|===\n|A |B |C |a |b |c |1 |2 |3\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > tbody > tr", 3);
    }

    #[test]
    fn table_with_explicit_column_count_can_have_multiple_rows_on_a_single_line() {
        let doc = Parser::default().parse("[cols=\"3*\"]\n|===\n|one |two\n|1 |2 |a |b\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 3);
        assert_css(&doc, "table > tbody > tr", 2);
    }

    #[test]
    #[ignore]
    // TODO (issue TBD): The crate does not support the deprecated bare-integer
    // colspec (`cols="3"` meaning three columns); it parses a single column of
    // width 3. Enable once the deprecated syntax is supported.
    fn table_with_explicit_deprecated_colspec_syntax_can_have_multiple_rows_on_a_single_line() {
        let doc = Parser::default().parse("[cols=\"3\"]\n|===\n|one |two\n|1 |2 |a |b\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 3);
        assert_css(&doc, "table > tbody > tr", 2);
    }

    #[test]
    #[ignore]
    // TODO (issue TBD): The crate does not add a column for an empty trailing
    // record in the colspec (`cols="<,"` should yield two columns); it parses a
    // single column. Enable once empty colspec records are honored.
    fn columns_are_added_for_empty_records_in_colspec_attribute() {
        let doc = Parser::default().parse("[cols=\"<,\"]\n|===\n|one |two\n|1 |2 |a |b\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > tbody > tr", 3);
    }

    #[test]
    #[ignore]
    // TODO (issue TBD): The crate does not accept a semicolon as the colspec
    // separator (`cols="1s;3m"` should yield two columns); it parses a single
    // column. Enable once `;`-separated colspecs are supported.
    fn cols_may_be_separated_by_semi_colon_instead_of_comma() {
        let doc = Parser::default().parse("[cols=\"1s;3m\"]\n|===\n| strong\n| mono\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "col[width=\"25%\"]", 1);
        assert_css(&doc, "col[width=\"75%\"]", 1);
        assert_xpath(&doc, "(//td)[1]//strong", 1);
        assert_xpath(&doc, "(//td)[2]//code", 1);
    }

    #[test]
    fn cols_attribute_may_include_spaces() {
        let doc = Parser::default().parse("[cols=\" 1, 1 \"]\n|===\n|one |two |1 |2 |a |b\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "col[width=\"50%\"]", 2);
        assert_css(&doc, "table > tbody > tr", 3);
    }

    #[test]
    fn blank_cols_attribute_should_be_ignored() {
        let doc = Parser::default().parse("[cols=\" \"]\n|===\n|one |two\n|1 |2 |a |b\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "col[width=\"50%\"]", 2);
        assert_css(&doc, "table > tbody > tr", 3);
    }

    #[test]
    fn empty_cols_attribute_should_be_ignored() {
        let doc = Parser::default().parse("[cols=\"\"]\n|===\n|one |two\n|1 |2 |a |b\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "col[width=\"50%\"]", 2);
        assert_css(&doc, "table > tbody > tr", 3);
    }

    #[test]
    fn table_with_header_and_footer() {
        let doc = Parser::default().parse(
            "[options=\"header,footer\"]\n|===\n|Item       |Quantity\n|Item 1     |1\n|Item 2     |2\n|Item 3     |3\n|Total      |6\n|===",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > thead", 1);
        assert_css(&doc, "table > thead > tr", 1);
        assert_css(&doc, "table > thead > tr > th", 2);
        assert_css(&doc, "table > tfoot", 1);
        assert_css(&doc, "table > tfoot > tr", 1);
        assert_css(&doc, "table > tfoot > tr > td", 2);
        assert_css(&doc, "table > tbody", 1);
        assert_css(&doc, "table > tbody > tr", 3);

        // Ruby additionally asserts the section order is thead, tbody, tfoot;
        // the renderer emits them in that order.
        assert_xpath(&doc, "/table/thead/following-sibling::tbody", 1);
        assert_xpath(&doc, "/table/tbody/following-sibling::tfoot", 1);
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





