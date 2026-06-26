// Adapted from Asciidoctor's tables test suite, found in
// https://github.com/asciidoctor/asciidoctor/blob/main/test/tables_test.rb.
//
// IMPORTANT: In porting this, I've disregarded compatibility mode (stated
// limitation of `asciidoc-parser` crate) and alternate (non-HTML) back ends.

mod psv {
    use crate::tests::prelude::{inline_file_handler::InlineFileHandler, *};

    #[test]
    fn converts_simple_psv_table() {
        let doc = Parser::default().parse("|=======\n|A |B |C\n|a |b |c\n|1 |2 |3\n|=======");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table.tableblock.frame-all.grid-all.stretch", 1);
        assert_css(&doc, "table > colgroup > col[width=\"33.3333%\"]", 2);
        // Ruby uses `col:last-of-type`; the indexed XPath form is equivalent.
        assert_xpath(&doc, "(/table/colgroup/col)[3][@width=\"33.3334%\"]", 1);
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
            assert_xpath(
                &doc,
                &format!("(/table/tbody/tr)[{}]/td", rowi + 1),
                row.len(),
            );
            assert_xpath(
                &doc,
                &format!("(/table/tbody/tr)[{}]/td/p", rowi + 1),
                row.len(),
            );
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
    fn ignores_escaped_separators() {
        let doc = Parser::default().parse("|===\n|A \\| here| a \\| there\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > tbody > tr", 1);
        assert_css(&doc, "table > tbody > tr > td", 2);
        assert_xpath(&doc, "/table/tbody/tr/td[1]/p[text()=\"A | here\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr/td[2]/p[text()=\"a | there\"]", 1);
    }

    #[test]
    fn preserves_escaped_delimiters_at_the_end_of_the_line() {
        let doc = Parser::default()
            .parse("[%header,cols=\"1,1\"]\n|===\n|A |B\\|\n|A1 |B1\\|\n|A2 |B2\\|\n|===");

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
        let doc = Parser::default().parse("|===\n|A1 |\n|B1 |B2\n|C1 |C2\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > tbody > tr", 3);
        assert_xpath(&doc, "/table/tbody/tr[1]/td", 2);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[1]/p[text()=\"A1\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[2]/p", 0);
        assert_xpath(&doc, "/table/tbody/tr[2]/td[1]/p[text()=\"B1\"]", 1);
    }

    #[test]
    fn should_auto_recover_with_warning_if_missing_leading_separator_on_first_cell() {
        let doc = Parser::default().parse("|===\nA | here| a | there\n| x\n| y\n| z\n| end\n|===");

        // The content before the first separator (`A`) is recovered as the first
        // cell, so the first row has four cells and the table four columns.
        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > tbody > tr", 2);
        assert_css(&doc, "table > tbody > tr > td", 8);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[1]/p[text()=\"A\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[2]/p[text()=\"here\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[3]/p[text()=\"a\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[4]/p[text()=\"there\"]", 1);

        // The recovery logs an error pointing at the offending line (line 2).
        let warnings: Vec<_> = doc.warnings().collect();
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::TableMissingLeadingSeparator
        );
        assert_eq!(warnings[0].source.line(), 2);
    }

    #[test]
    fn performs_normal_substitutions_on_cell_content() {
        let doc = Parser::default()
            .parse(":show_title: Cool new show\n|===\n|{show_title} |Coming soon...\n|===");

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
    fn should_preserve_leading_spaces_but_not_leading_newlines_or_trailing_spaces_in_literal_table_cells()
     {
        let doc =
            Parser::default().parse("[cols=2*]\n|===\nl|\n  one\n  two\nthree\n\n  | normal\n|===");

        assert_css(&doc, "table pre", 1);
        assert_xpath(&doc, "/table//pre[text()=\"  one\n  two\nthree\"]", 1);
    }

    #[test]
    fn should_ignore_v_table_cell_style() {
        let doc =
            Parser::default().parse("[cols=2*]\n|===\nv|\n  one\n  two\nthree\n\n  | normal\n|===");

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
        let doc = Parser::default()
            .parse("[options=\"autowidth\"]\n|=======\n|A |B |C\n|a |b |c\n|1 |2 |3\n|=======");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table.fit-content", 1);
        assert_css(&doc, "table[style*=\"width\"]", 0);
        assert_css(&doc, "table colgroup col", 3);
        assert_css(&doc, "table colgroup col[style*=\"width\"]", 0);
    }

    #[test]
    fn does_not_assign_column_width_for_autowidth_columns_in_html_output() {
        let doc = Parser::default()
            .parse("[cols=\"15%,3*~\"]\n|=======\n|A |B |C |D\n|a |b |c |d\n|1 |2 |3 |4\n|=======");

        // The fixed first column keeps its computed percentage in `colpcwidth`
        // and carries the HTML `width` attribute; it is not autowidth.
        assert_xpath(&doc, "(/table/colgroup/col)[1][@colpcwidth=\"15\"]", 1);
        assert_xpath(&doc, "(/table/colgroup/col)[1][@width=\"15%\"]", 1);
        assert_xpath(&doc, "(/table/colgroup/col)[1][@autowidth-option]", 0);

        // Each autowidth column still has a computed `colpcwidth` (the remaining
        // space shared three ways, truncated to four places with the balance
        // donated to the last column) and the `autowidth-option` marker, but no
        // HTML `width` attribute.
        for i in 2..=3 {
            assert_xpath(
                &doc,
                &format!("(/table/colgroup/col)[{i}][@colpcwidth=\"28.3333\"]"),
                1,
            );
        }
        assert_xpath(&doc, "(/table/colgroup/col)[4][@colpcwidth=\"28.3334\"]", 1);
        for i in 2..=4 {
            assert_xpath(
                &doc,
                &format!("(/table/colgroup/col)[{i}][@autowidth-option]"),
                1,
            );
            assert_xpath(&doc, &format!("(/table/colgroup/col)[{i}][@width]"), 0);
        }

        // The HTML-output expectations from the Ruby test: every column renders a
        // `<col>`, but only the single fixed column carries a `width` attribute.
        assert_css(&doc, "table", 1);
        assert_css(&doc, "table colgroup col", 4);
        assert_css(&doc, "table colgroup col[width]", 1);
        assert_css(&doc, "table colgroup col[width=\"15%\"]", 1);
        assert_css(&doc, "table colgroup col[autowidth-option]", 3);
    }

    #[test]
    fn can_assign_autowidth_to_all_columns_even_when_table_has_a_width() {
        let doc = Parser::default().parse(
            "[cols=\"4*~\",width=50%]\n|=======\n|A |B |C |D\n|a |b |c |d\n|1 |2 |3 |4\n|=======",
        );

        // Every column is autowidth, so each takes an equal 25% share in
        // `colpcwidth` and carries the `autowidth-option` marker; none carries an
        // HTML `width` attribute even though the table itself has a width.
        for i in 1..=4 {
            assert_xpath(
                &doc,
                &format!("(/table/colgroup/col)[{i}][@colpcwidth=\"25\"]"),
                1,
            );
            assert_xpath(
                &doc,
                &format!("(/table/colgroup/col)[{i}][@autowidth-option]"),
                1,
            );
            assert_xpath(&doc, &format!("(/table/colgroup/col)[{i}][@width]"), 0);
        }

        // The explicit table width is still rendered, and no column carries a
        // width-bearing `style` (the Ruby HTML-output expectations).
        assert_css(&doc, "table", 1);
        assert_css(&doc, "table[width=\"50%\"]", 1);
        assert_css(&doc, "table colgroup col", 4);
        assert_css(&doc, "table colgroup col[style]", 0);
        assert_css(&doc, "table colgroup col[autowidth-option]", 4);
    }

    // Backend-specific test omitted: DocBook ("equally distributes remaining
    // column width to autowidth columns in DocBook output").

    // Backend-specific test omitted: DocBook ("should compute column widths
    // based on pagewidth when width is set on table in DocBook output").

    #[test]
    fn explicit_table_width_is_used_even_when_autowidth_option_is_specified() {
        let doc = Parser::default()
            .parse("[%autowidth,width=75%]\n|=======\n|A |B |C\n|a |b |c\n|1 |2 |3\n|=======");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table[width]", 1);
        assert_css(&doc, "table colgroup col", 3);
        assert_css(&doc, "table colgroup col[style*=\"width\"]", 0);
    }

    #[test]
    fn first_row_sets_number_of_columns_when_not_specified() {
        let doc =
            Parser::default().parse("|===\n|first |second |third |fourth\n|1 |2 |3\n|4\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 4);
        assert_css(&doc, "table > tbody > tr", 2);
        assert_xpath(&doc, "(/table/tbody/tr)[1]/td", 4);
        assert_xpath(&doc, "(/table/tbody/tr)[2]/td", 4);
    }

    #[test]
    fn colspec_attribute_using_asterisk_syntax_sets_number_of_columns() {
        let doc = Parser::default().parse("[cols=\"3*\"]\n|===\n|A |B |C |a |b |c |1 |2 |3\n|===");

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
    fn table_with_explicit_deprecated_colspec_syntax_can_have_multiple_rows_on_a_single_line() {
        let doc = Parser::default().parse("[cols=\"3\"]\n|===\n|one |two\n|1 |2 |a |b\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 3);
        assert_css(&doc, "table > tbody > tr", 2);
    }

    #[test]
    fn columns_are_added_for_empty_records_in_colspec_attribute() {
        let doc = Parser::default().parse("[cols=\"<,\"]\n|===\n|one |two\n|1 |2 |a |b\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > tbody > tr", 3);
    }

    #[test]
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

    // Backend-specific test omitted: DocBook ("table with header and footer
    // docbook").

    // Backend-specific test omitted: DocBook ("should set horizontal and
    // vertical alignment when converting to DocBook").

    #[test]
    fn should_preserve_frame_value_ends_when_converting_to_html() {
        let doc = Parser::default().parse("[frame=ends]\n|===\n|A |B |C\n|===");

        assert_css(&doc, "table.frame-ends", 1);
    }

    #[test]
    fn should_normalize_frame_value_topbot_as_ends_when_converting_to_html() {
        let doc = Parser::default().parse("[frame=topbot]\n|===\n|A |B |C\n|===");

        assert_css(&doc, "table.frame-ends", 1);
    }

    // Backend-specific test omitted: DocBook ("should preserve frame value
    // topbot when converting to DocBook").

    // Backend-specific test omitted: DocBook ("should convert frame value ends
    // to topbot when converting to DocBook").

    // Backend-specific test omitted: DocBook ("table with landscape orientation
    // in DocBook").

    #[test]
    fn table_with_implicit_header_row() {
        let doc = Parser::default()
            .parse("|===\n|Column 1 |Column 2\n\n|Data A1\n|Data B1\n\n|Data A2\n|Data B2\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > thead", 1);
        assert_css(&doc, "table > thead > tr", 1);
        assert_css(&doc, "table > thead > tr > th", 2);
        assert_css(&doc, "table > tbody", 1);
        assert_css(&doc, "table > tbody > tr", 2);
    }

    #[test]
    fn table_with_implicit_header_row_only() {
        let doc = Parser::default().parse("|===\n|Column 1 |Column 2\n\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > thead", 1);
        assert_css(&doc, "table > thead > tr", 1);
        assert_css(&doc, "table > thead > tr > th", 2);
        assert_css(&doc, "table > tbody", 0);
    }

    #[test]
    fn table_with_implicit_header_row_when_other_options_set() {
        let doc = Parser::default()
            .parse("[%autowidth]\n|===\n|Column 1 |Column 2\n\n|Data A1\n|Data B1\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table[style*=\"width\"]", 0);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > thead", 1);
        assert_css(&doc, "table > thead > tr", 1);
        assert_css(&doc, "table > thead > tr > th", 2);
        assert_css(&doc, "table > tbody", 1);
        assert_css(&doc, "table > tbody > tr", 1);
    }

    #[test]
    fn no_implicit_header_row_if_second_line_not_blank() {
        let doc = Parser::default()
            .parse("|===\n|Column 1 |Column 2\n|Data A1\n|Data B1\n\n|Data A2\n|Data B2\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > thead", 0);
        assert_css(&doc, "table > tbody", 1);
        assert_css(&doc, "table > tbody > tr", 3);
    }

    #[test]
    fn no_implicit_header_row_if_cell_in_first_line_spans_multiple_lines() {
        let doc =
            Parser::default().parse("[cols=2*]\n|===\n|A1\n\n\nA1 continued|B1\n\n|A2\n|B2\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > thead", 0);
        assert_css(&doc, "table > tbody", 1);
        assert_css(&doc, "table > tbody > tr", 2);
        assert_xpath(&doc, "(//td)[1]/p", 2);
    }

    #[test]
    fn should_format_first_cell_as_literal_if_there_is_no_implicit_header_row_and_column_has_l_style()
     {
        let doc = Parser::default().parse("[cols=\"1l,1\"]\n|===\n|literal\n|normal\n|===");

        assert_css(&doc, "tbody pre", 1);
        assert_css(&doc, "tbody p.tableblock", 1);
    }

    #[test]
    fn should_format_first_cell_as_asciidoc_if_there_is_no_implicit_header_row_and_column_has_a_style()
     {
        let doc = Parser::default().parse("[cols=\"1a,1\"]\n|===\n| * list\n| normal\n|===");

        assert_css(&doc, "tbody .ulist", 1);
        assert_css(&doc, "tbody p.tableblock", 1);
    }

    #[test]
    fn should_interpret_leading_indent_if_first_cell_is_asciidoc_and_there_is_no_implicit_header_row()
     {
        let doc = Parser::default().parse("[cols=\"1a,1\"]\n|===\n|\n  literal\n| normal\n|===");

        assert_css(&doc, "tbody pre", 1);
        assert_css(&doc, "tbody p.tableblock", 1);
    }

    #[test]
    fn should_format_first_cell_as_asciidoc_if_there_is_no_implicit_header_row_and_cell_has_a_style()
     {
        let doc = Parser::default().parse("|===\na| * list\n| normal\n|===");

        assert_css(&doc, "tbody .ulist", 1);
        assert_css(&doc, "tbody p.tableblock", 1);
    }

    #[test]
    fn no_implicit_header_row_if_asciidoc_cell_in_first_line_spans_multiple_lines() {
        let doc = Parser::default().parse(
            "[cols=2*]\n|===\na|contains AsciiDoc content\n\n* a\n* b\n* c\na|contains no AsciiDoc content\n\njust text\n|A2\n|B2\n|===",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > thead", 0);
        assert_css(&doc, "table > tbody", 1);
        assert_css(&doc, "table > tbody > tr", 2);
        assert_xpath(&doc, "(//td)[1]//ul", 1);
    }

    #[test]
    fn no_implicit_header_row_if_first_line_blank() {
        let doc = Parser::default().parse(
            "|===\n\n|Column 1 |Column 2\n\n|Data A1\n|Data B1\n\n|Data A2\n|Data B2\n\n|===",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > thead", 0);
        assert_css(&doc, "table > tbody", 1);
        assert_css(&doc, "table > tbody > tr", 3);
    }

    #[test]
    fn no_implicit_header_row_if_noheader_option_is_specified() {
        let doc = Parser::default().parse(
            "[%noheader]\n|===\n|Column 1 |Column 2\n\n|Data A1\n|Data B1\n\n|Data A2\n|Data B2\n|===",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > thead", 0);
        assert_css(&doc, "table > tbody", 1);
        assert_css(&doc, "table > tbody > tr", 3);
    }

    #[test]
    fn styles_not_applied_to_header_cells() {
        let doc = Parser::default().parse(
            "[cols=\"1h,1s,1e\",options=\"header,footer\"]\n|===\n|Name |Occupation| Website\n|Octocat |Social coding| https://github.com\n|Name |Occupation| Website\n|===",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > thead > tr > th", 3);
        assert_css(&doc, "table > thead > tr > th > *", 0);

        assert_css(&doc, "table > tfoot > tr > th", 1);
        assert_css(&doc, "table > tfoot > tr > td", 2);
        assert_css(&doc, "table > tfoot > tr > td > p > strong", 1);
        assert_css(&doc, "table > tfoot > tr > td > p > em", 1);

        assert_css(&doc, "table > tbody > tr > th", 1);
        assert_css(&doc, "table > tbody > tr > td", 2);
        assert_css(&doc, "table > tbody > tr > td > p.header", 0);
        assert_css(&doc, "table > tbody > tr > td > p > strong", 1);
        assert_css(&doc, "table > tbody > tr > td > p > em > a", 1);
    }

    #[test]
    fn should_apply_text_formatting_to_cells_in_implicit_header_row_when_column_has_a_style() {
        let doc = Parser::default()
            .parse("[cols=\"2*a\"]\n|===\n| _foo_ | *bar*\n\n| * list item\n| paragraph\n|===");

        assert_xpath(&doc, "(//thead/tr/th)[1]/em[text()=\"foo\"]", 1);
        assert_xpath(&doc, "(//thead/tr/th)[2]/strong[text()=\"bar\"]", 1);
        assert_css(&doc, "tbody .ulist", 1);
        assert_css(&doc, "tbody .paragraph", 1);
    }

    #[test]
    fn should_apply_style_and_text_formatting_to_cells_in_first_row_if_no_implicit_header() {
        let doc = Parser::default()
            .parse("[cols=\"s,e\"]\n|===\n| _strong_ | *emphasis*\n| strong\n| emphasis\n|===");

        assert_xpath(
            &doc,
            "((//tbody/tr)[1]/td)[1]//strong/em[text()=\"strong\"]",
            1,
        );
        assert_xpath(
            &doc,
            "((//tbody/tr)[1]/td)[2]//em/strong[text()=\"emphasis\"]",
            1,
        );
        assert_xpath(
            &doc,
            "((//tbody/tr)[2]/td)[1]//strong[text()=\"strong\"]",
            1,
        );
        assert_xpath(&doc, "((//tbody/tr)[2]/td)[2]//em[text()=\"emphasis\"]", 1);
    }

    #[test]
    fn vertical_table_headers_use_th_element_instead_of_header_class() {
        let doc = Parser::default().parse(
            "[cols=\"1h,1s,1e\"]\n|===\n\n|Name |Occupation| Website\n\n|Octocat |Social coding| https://github.com\n\n|Name |Occupation| Website\n\n|===",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > tbody > tr > th", 3);
        assert_css(&doc, "table > tbody > tr > td", 6);
        assert_css(&doc, "table > tbody > tr .header", 0);
        assert_css(&doc, "table > tbody > tr > td > p > strong", 3);
        assert_css(&doc, "table > tbody > tr > td > p > em", 3);
        assert_css(&doc, "table > tbody > tr > td > p > em > a", 1);
    }

    #[test]
    fn supports_horizontal_and_vertical_source_data_with_blank_lines_and_table_header() {
        let doc = Parser::default().parse(
            ".Horizontal and vertical source data\n[width=\"80%\",cols=\"3,^2,^2,10\",options=\"header\"]\n|===\n|Date |Duration |Avg HR |Notes\n\n|22-Aug-08 |10:24 | 157 |\nWorked out MSHR (max sustainable heart rate) by going hard\nfor this interval.\n\n|22-Aug-08 |23:03 | 152 |\nBack-to-back with previous interval.\n\n|24-Aug-08 |40:00 | 145 |\nModerately hard interspersed with 3x 3min intervals (2 min\nhard + 1 min really hard taking the HR up to 160).\n\nI am getting in shape!\n\n|===",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table[width=\"80%\"]", 1);
        assert_xpath(
            &doc,
            "/table/caption[@class=\"title\"][text()=\"Table 1. Horizontal and vertical source data\"]",
            1,
        );
        assert_css(&doc, "table > colgroup > col", 4);
        // Ruby uses `col:nth-child(N)`; the indexed XPath form is equivalent.
        assert_xpath(&doc, "(/table/colgroup/col)[1][@width=\"17.647%\"]", 1);
        assert_xpath(&doc, "(/table/colgroup/col)[2][@width=\"11.7647%\"]", 1);
        assert_xpath(&doc, "(/table/colgroup/col)[3][@width=\"11.7647%\"]", 1);
        assert_xpath(&doc, "(/table/colgroup/col)[4][@width=\"58.8236%\"]", 1);
        assert_css(&doc, "table > thead", 1);
        assert_css(&doc, "table > thead > tr", 1);
        assert_css(&doc, "table > thead > tr > th", 4);
        assert_css(&doc, "table > tbody > tr", 3);
        assert_xpath(&doc, "(/table/tbody/tr)[1]/td", 4);
        assert_xpath(&doc, "(/table/tbody/tr)[2]/td", 4);
        assert_xpath(&doc, "(/table/tbody/tr)[3]/td", 4);
        assert_xpath(
            &doc,
            "/table/tbody/tr[1]/td[4]/p[text()='Worked out MSHR (max sustainable heart rate) by going hard\nfor this interval.']",
            1,
        );
        assert_xpath(&doc, "(/table/tbody/tr)[3]/td[4]/p", 2);
        assert_xpath(
            &doc,
            "/table/tbody/tr[3]/td[4]/p[2][text()=\"I am getting in shape!\"]",
            1,
        );
    }

    #[test]
    fn percentages_as_column_widths() {
        let doc =
            Parser::default().parse("[cols=\"<.^10%,<90%\"]\n|===\n|column A |column B\n|===");

        assert_xpath(&doc, "/table/colgroup/col", 2);
        assert_xpath(&doc, "(/table/colgroup/col)[1][@width=\"10%\"]", 1);
        assert_xpath(&doc, "(/table/colgroup/col)[2][@width=\"90%\"]", 1);
    }

    #[test]
    fn spans_alignments_and_styles() {
        let doc = Parser::default().parse(
            "[cols=\"e,m,^,>s\",width=\"25%\"]\n|===\n|1 >s|2 |3 |4\n^|5 2.2+^.^|6 .3+<.>m|7\n^|8\nd|9 2+>|10\n|===",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col[width=\"25%\"]", 4);
        assert_css(&doc, "table > tbody > tr", 4);
        assert_css(&doc, "table > tbody > tr > td", 10);

        // Ruby uses `tr:nth-child(N)` / `td:nth-child(N)`; the indexed XPath form
        // is equivalent and supported by the test DOM.
        assert_xpath(&doc, "(/table/tbody/tr)[1]/td", 4);
        assert_xpath(&doc, "(/table/tbody/tr)[2]/td", 3);
        assert_xpath(&doc, "(/table/tbody/tr)[3]/td", 1);
        assert_xpath(&doc, "(/table/tbody/tr)[4]/td", 2);

        assert_xpath(
            &doc,
            "((/table/tbody/tr)[1]/td)[1][@class=\"halign-left valign-top\"]/p/em",
            1,
        );
        assert_xpath(
            &doc,
            "((/table/tbody/tr)[1]/td)[2][@class=\"halign-right valign-top\"]/p/strong",
            1,
        );
        assert_xpath(
            &doc,
            "((/table/tbody/tr)[1]/td)[3][@class=\"halign-center valign-top\"]/p",
            1,
        );
        assert_xpath(
            &doc,
            "((/table/tbody/tr)[1]/td)[3][@class=\"halign-center valign-top\"]/p/*",
            0,
        );
        assert_xpath(
            &doc,
            "((/table/tbody/tr)[1]/td)[4][@class=\"halign-right valign-top\"]/p/strong",
            1,
        );

        assert_xpath(
            &doc,
            "((/table/tbody/tr)[2]/td)[1][@class=\"halign-center valign-top\"]/p/em",
            1,
        );
        assert_xpath(
            &doc,
            "((/table/tbody/tr)[2]/td)[2][@class=\"halign-center valign-middle\"][@colspan=\"2\"][@rowspan=\"2\"]/p/code",
            1,
        );
        assert_xpath(
            &doc,
            "((/table/tbody/tr)[2]/td)[3][@class=\"halign-left valign-bottom\"][@rowspan=\"3\"]/p/code",
            1,
        );

        assert_xpath(
            &doc,
            "((/table/tbody/tr)[3]/td)[1][@class=\"halign-center valign-top\"]/p/em",
            1,
        );

        assert_xpath(
            &doc,
            "((/table/tbody/tr)[4]/td)[1][@class=\"halign-left valign-top\"]/p",
            1,
        );
        assert_xpath(
            &doc,
            "((/table/tbody/tr)[4]/td)[1][@class=\"halign-left valign-top\"]/p/em",
            0,
        );
        assert_xpath(
            &doc,
            "((/table/tbody/tr)[4]/td)[2][@class=\"halign-right valign-top\"][@colspan=\"2\"]/p/code",
            1,
        );
    }

    #[test]
    fn sets_up_columns_correctly_if_first_row_has_cell_that_spans_columns() {
        let doc =
            Parser::default().parse("|===\n2+^|AAA |CCC\n|AAA |BBB |CCC\n|AAA |BBB |CCC\n|===");

        assert_xpath(&doc, "(/table/tbody/tr)[1]/td", 2);
        assert_xpath(&doc, "((/table/tbody/tr)[1]/td)[1][@colspan=\"2\"]", 1);
        // Ruby uses `td:nth-child(1)[colspan]` / `td:nth-child(2):not([colspan])`;
        // since row 1 has exactly one cell carrying a colspan, asserting the row
        // has a single colspanned cell is equivalent.
        assert_xpath(&doc, "(/table/tbody/tr)[1]/td[@colspan]", 1);
        assert_xpath(&doc, "(/table/tbody/tr)[2]/td", 3);
        assert_xpath(&doc, "(/table/tbody/tr)[2]/td[@colspan]", 0);
        assert_xpath(&doc, "(/table/tbody/tr)[3]/td", 3);
        assert_xpath(&doc, "(/table/tbody/tr)[3]/td[@colspan]", 0);
    }

    #[test]
    fn supports_repeating_cells() {
        let doc = Parser::default().parse("|===\n3*|A\n|1 3*|2\n|b |c\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 3);
        assert_css(&doc, "table > tbody > tr", 3);
        assert_xpath(&doc, "(/table/tbody/tr)[1]/td", 3);
        assert_xpath(&doc, "(/table/tbody/tr)[2]/td", 3);
        assert_xpath(&doc, "(/table/tbody/tr)[3]/td", 3);

        assert_xpath(&doc, "/table/tbody/tr[1]/td[1]/p[text()=\"A\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[2]/p[text()=\"A\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[3]/p[text()=\"A\"]", 1);

        assert_xpath(&doc, "/table/tbody/tr[2]/td[1]/p[text()=\"1\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[2]/td[2]/p[text()=\"2\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[2]/td[3]/p[text()=\"2\"]", 1);

        assert_xpath(&doc, "/table/tbody/tr[3]/td[1]/p[text()=\"2\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[3]/td[2]/p[text()=\"b\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[3]/td[3]/p[text()=\"c\"]", 1);
    }

    // Backend-specific test omitted: DocBook ("calculates colnames correctly
    // when using implicit column count and single cell with colspan").

    // Backend-specific test omitted: DocBook ("calculates colnames correctly
    // when using implicit column count and cells with mixed colspans").

    // Backend-specific test omitted: DocBook ("assigns unique column names for
    // table with implicit column count and colspans in first row").

    #[test]
    fn should_drop_row_but_preserve_remaining_rows_after_cell_with_colspan_exceeds_number_of_columns()
     {
        let doc = Parser::default().parse("[cols=2*]\n|===\n3+|A\n|B\na|C\n\nmore C\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table tr", 1);
        assert_xpath(&doc, "/table/tbody/tr/td[1]/p[text()=\"B\"]", 1);

        let warnings: Vec<_> = doc.warnings().collect();
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::TableCellExceedsColumnCount
        );
        assert_eq!(warnings[0].source.line(), 3);
    }

    #[test]
    fn should_drop_last_row_if_last_cell_in_table_has_colspan_that_exceeds_specified_number_of_columns()
     {
        let doc = Parser::default().parse("[cols=2*]\n|===\n|a 2+|b\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table *", 0);

        let warnings: Vec<_> = doc.warnings().collect();
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::TableCellExceedsColumnCount
        );
        assert_eq!(warnings[0].source.line(), 3);
    }

    #[test]
    fn should_drop_last_row_if_last_cell_in_table_has_colspan_that_exceeds_implicit_number_of_columns()
     {
        let doc = Parser::default().parse("|===\n|a |b\n|c 2+|d\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table tr", 1);
        assert_xpath(&doc, "/table/tbody/tr/td[1]/p[text()=\"a\"]", 1);

        let warnings: Vec<_> = doc.warnings().collect();
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::TableCellExceedsColumnCount
        );
        assert_eq!(warnings[0].source.line(), 3);
    }

    #[test]
    #[ignore]
    // TODO (issue TBD): The crate mis-groups cells into rows when a leading row's
    // colspans overflow the column count: for this input it produces seven
    // single-cell rows instead of dropping the overflowing first row and forming
    // one row of seven cells. Enable once row grouping accounts for the dropped
    // colspan row.
    fn should_take_colspan_into_account_when_taking_cells_for_row() {
        let doc = Parser::default()
            .parse("[cols=7]\n|===\n2+|a 2+|b 2+|c 2+|d\n|e |f |g |h |i |j |k\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table tr", 1);
        assert_css(&doc, "table tr td", 7);

        let warnings: Vec<_> = doc.warnings().collect();
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::TableCellExceedsColumnCount
        );
    }

    #[test]
    fn should_drop_incomplete_row_at_end_of_table_and_log_an_error() {
        let doc = Parser::default().parse("[cols=2*]\n|===\n|a |b\n|c |d\n|e\n|===");

        // The incomplete `|e` row is dropped, leaving two rows, and an error is
        // logged against its line (line 5).
        assert_css(&doc, "table", 1);
        assert_css(&doc, "table tr", 2);

        let warnings: Vec<_> = doc.warnings().collect();
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::TableDroppingIncompleteRowAtEndOfTable
        );
        assert_eq!(warnings[0].source.line(), 5);
    }

    #[test]
    fn should_apply_cell_style_for_column_to_repeated_content() {
        let doc = Parser::default().parse(
            "[cols=\",^l\"]\n|===\n|Paragraphs |Literal\n\n2*|The discussion about what is good,\nwhat is beautiful, what is noble,\nwhat is pure, and what is true\ncould always go on.\n\nWhy is that important?\nWhy would I like to do that?\n\nBecause that's the only conversation worth having.\n\nAnd whether it goes on or not after I die, I don't know.\nBut, I do know that it is the conversation I want to have while I am still alive.\n\nWhich means that to me the offer of certainty,\nthe offer of complete security,\nthe offer of an impermeable faith that can't give way\nis an offer of something not worth having.\n\nI want to live my life taking the risk all the time\nthat I don't know anything like enough yet...\nthat I haven't understood enough...\nthat I can't know enough...\nthat I am always hungrily operating on the margins\nof a potentially great harvest of future knowledge and wisdom.\n\nI wouldn't have it any other way.\n|===",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > thead", 1);
        assert_css(&doc, "table > thead > tr", 1);
        assert_css(&doc, "table > thead > tr > th", 2);
        assert_css(&doc, "table > tbody", 1);
        assert_css(&doc, "table > tbody > tr", 1);
        assert_css(&doc, "table > tbody > tr > td", 2);

        // The duplicated cell (`2*|`) is cloned into both columns, and each clone
        // takes its own column's style: the first (default) column renders the
        // content as seven paragraphs, the second (`^l`) column as a single
        // centered literal block. (Ruby uses `td:nth-child(N)`; the equivalent
        // positional `td[N]` is expressed in XPath here.)
        assert_xpath(
            &doc,
            "/table/tbody/tr/td[1][@class=\"halign-left valign-top\"]/p[@class=\"tableblock\"]",
            7,
        );
        assert_xpath(
            &doc,
            "/table/tbody/tr/td[2][@class=\"halign-center valign-top\"]/div[@class=\"literal\"]/pre",
            1,
        );

        // The literal block preserves every line of the cell, including the blank
        // lines between paragraphs (26 lines total).
        let vdom = doc.to_virtual_dom();
        let pre = query_xpath(&vdom, "/table/tbody/tr/td[2]//pre");
        assert_eq!(pre.len(), 1);
        assert_eq!(
            pre[0].text.as_deref().unwrap_or_default().lines().count(),
            26
        );
    }

    #[test]
    fn should_not_split_paragraph_at_line_containing_only_blank_that_is_directly_adjacent_to_non_blank_lines()
     {
        let doc = Parser::default().parse(
            "|===\n|paragraph\n{blank}\nstill one paragraph\n{blank}\nstill one paragraph\n|===",
        );

        // Each `{blank}` line renders as an empty line but is not blank in the
        // source, so the cell stays a single paragraph.
        assert_css(&doc, "p.tableblock", 1);
    }

    #[test]
    fn should_strip_trailing_newlines_when_splitting_paragraphs() {
        let doc = Parser::default()
            .parse("|===\n|first wrapped\nparagraph\n\nsecond paragraph\n\nthird paragraph\n|===");

        assert_xpath(
            &doc,
            "(//p[@class=\"tableblock\"])[1][text()=\"first wrapped\nparagraph\"]",
            1,
        );
        assert_xpath(
            &doc,
            "(//p[@class=\"tableblock\"])[2][text()=\"second paragraph\"]",
            1,
        );
        assert_xpath(
            &doc,
            "(//p[@class=\"tableblock\"])[3][text()=\"third paragraph\"]",
            1,
        );
    }

    #[test]
    #[ignore]
    // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/456): The
    // admonition inside the open block does not render as `.admonitionblock`.
    // Enable when admonitions are implemented.
    fn basic_asciidoc_cell() {
        let doc = Parser::default().parse("|===\na|--\nNOTE: content\n\ncontent\n--\n|===");

        assert_css(&doc, "table.tableblock", 1);
        assert_css(&doc, "table.tableblock td.tableblock", 1);
        assert_css(&doc, "table.tableblock td.tableblock .openblock", 1);
        assert_css(
            &doc,
            "table.tableblock td.tableblock .openblock .admonitionblock",
            1,
        );
        assert_css(
            &doc,
            "table.tableblock td.tableblock .openblock .paragraph",
            1,
        );
    }

    #[test]
    fn asciidoc_table_cell_should_be_wrapped_in_div_with_class_content() {
        let doc = Parser::default().parse("|===\na|AsciiDoc table cell\n|===");

        assert_css(&doc, "table.tableblock td.tableblock > div.content", 1);
        assert_css(
            &doc,
            "table.tableblock td.tableblock > div.content > div.paragraph",
            1,
        );
    }

    #[test]
    fn doctype_can_be_set_in_asciidoc_table_cell() {
        let doc = Parser::default().parse("|===\na|\n:doctype: inline\n\ncontent\n|===");

        // An `inline` doctype renders the cell's lone paragraph as bare inline
        // content, with no `.paragraph` wrapper.
        assert_css(&doc, "table.tableblock", 1);
        assert_css(&doc, "table.tableblock .paragraph", 0);
    }

    #[test]
    fn should_reset_doctype_to_default_in_asciidoc_table_cell() {
        // The parent is a `book`, but a cell resets to the default `article`
        // doctype, so `{doctype}` is `article` and only the
        // `backend-html5-doctype-article` derived attribute is defined (an
        // undefined reference is left literal).
        let doc = Parser::default().parse(
            "= Book Title\n:doctype: book\n\n== Chapter 1\n\n|===\na|\n= AsciiDoc Table Cell\n\ndoctype={doctype}\n{backend-html5-doctype-article}\n{backend-html5-doctype-book}\n|===",
        );

        assert_rendered_contains(&doc, "doctype=article");
        refute_rendered_contains(&doc, "{backend-html5-doctype-article}");
        assert_rendered_contains(&doc, "{backend-html5-doctype-book}");
    }

    #[test]
    fn should_update_doctype_related_attributes_in_asciidoc_table_cell_when_doctype_is_set() {
        // Setting `:doctype: book` in the cell updates `{doctype}` and the
        // derived `backend-html5-doctype-*` attribute accordingly.
        let doc = Parser::default().parse(
            "= Document Title\n:doctype: article\n\n== Chapter 1\n\n|===\na|\n= AsciiDoc Table Cell\n:doctype: book\n\ndoctype={doctype}\n{backend-html5-doctype-book}\n{backend-html5-doctype-article}\n|===",
        );

        assert_rendered_contains(&doc, "doctype=book");
        refute_rendered_contains(&doc, "{backend-html5-doctype-book}");
        assert_rendered_contains(&doc, "{backend-html5-doctype-article}");
    }

    // Deferred: "should not allow AsciiDoc table cell to set a document
    // attribute that was hard set by the API" (Ruby 1457) observes the lock via
    // a NOTE admonition's icon, and admonitions are not implemented. The same
    // API-lock path is covered by
    // `should_not_allow_locked_attribute_unset_in_parent_document_to_be_set_in_asciidoc_table_cell`.

    // Deferred: "should not allow AsciiDoc table cell to set a document
    // attribute that was hard unset by the API" (Ruby 1472) — same admonition
    // dependency as above.

    #[test]
    fn should_keep_attribute_unset_in_asciidoc_table_cell_if_unset_in_parent_document() {
        // `sectids` and `table-caption` are unset in the parent and stay unset
        // in the cell: the headings get no id, and both the outer table and the
        // nested table render their title as a bare `<caption>`.
        let doc = Parser::default().parse(
            ":!sectids:\n:!table-caption:\n\n== Outer Heading\n\n.Outer Table\n|===\na|\n\n== Inner Heading\n\n.Inner Table\n!===\n! table cell\n!===\n|===",
        );

        assert_xpath(&doc, "//h2[@id]", 0);
        assert_xpath(&doc, "//caption[text()=\"Outer Table\"]", 1);
        assert_xpath(&doc, "//caption[text()=\"Inner Table\"]", 1);
    }

    #[test]
    fn should_allow_attribute_unset_in_parent_document_to_be_set_in_asciidoc_table_cell() {
        // `sectids` is unset in the parent header (not locked), so the cell may
        // set it: the heading after `:sectids:` in the cell gets an id.
        let doc = Parser::default().parse(
            ":!sectids:\n\n== No ID\n\n|===\na|\n\n== No ID\n\n:sectids:\n\n== Has ID\n|===",
        );

        assert_css(&doc, "h2", 3);
        assert_xpath(&doc, "(//h2)[1][@id]", 0);
        assert_xpath(&doc, "(//h2)[2][@id]", 0);
        assert_xpath(&doc, "(//h2)[3][@id=\"_has_id\"]", 1);
    }

    #[test]
    fn should_not_allow_locked_attribute_unset_in_parent_document_to_be_set_in_asciidoc_table_cell()
    {
        // `sectids` is hard unset by the API, so the cell's `:sectids:` is
        // ignored and no heading gets an id.
        let doc = Parser::default()
            .with_intrinsic_attribute_bool("sectids", false, ModificationContext::ApiOnly)
            .parse("== No ID\n\n|===\na|\n\n== No ID\n\n:sectids:\n\n== Has ID\n|===");

        assert_css(&doc, "h2", 3);
        assert_xpath(&doc, "//h2[@id]", 0);
    }

    #[test]
    fn showtitle_can_be_enabled_in_asciidoc_table_cell_if_unset_in_parent_document() {
        // The parent hides its title; the cell shows its own. (`showtitle` and
        // `notitle` are complements, so both spellings are exercised.)
        for (parent, cell) in [(":!showtitle:", ":showtitle:"), (":notitle:", ":!notitle:")] {
            let doc = Parser::default().parse(&format!(
                "= Document Title\n{parent}\n\n|===\na|\n= Nested Document Title\n{cell}\n\ncontent\n|==="
            ));
            assert_css(&doc, "h1", 1);
            assert_css(&doc, ".tableblock h1", 1);
        }
    }

    #[test]
    fn showtitle_can_be_enabled_in_asciidoc_table_cell_if_unset_by_api() {
        for (name, value, cell) in [
            ("showtitle", false, ":showtitle:"),
            ("notitle", true, ":!notitle:"),
        ] {
            let doc = Parser::default()
                .with_intrinsic_attribute_bool(name, value, ModificationContext::ApiOnly)
                .parse(&format!(
                    "= Document Title\n\n|===\na|\n= Nested Document Title\n{cell}\n\ncontent\n|==="
                ));
            assert_css(&doc, "h1", 1);
            assert_css(&doc, ".tableblock h1", 1);
        }
    }

    #[test]
    fn showtitle_can_be_disabled_in_asciidoc_table_cell_if_set_in_parent_document() {
        // The parent shows its title; the cell hides its own.
        for (parent, cell) in [(":showtitle:", ":!showtitle:"), (":!notitle:", ":notitle:")] {
            let doc = Parser::default().parse(&format!(
                "= Document Title\n{parent}\n\n|===\na|\n= Nested Document Title\n{cell}\n\ncontent\n|==="
            ));
            assert_css(&doc, "h1", 1);
            assert_css(&doc, ".tableblock h1", 0);
        }
    }

    #[test]
    fn showtitle_can_be_disabled_in_asciidoc_table_cell_if_set_by_api() {
        for (name, value, cell) in [
            ("showtitle", true, ":!showtitle:"),
            ("notitle", false, ":notitle:"),
        ] {
            let doc = Parser::default()
                .with_intrinsic_attribute_bool(name, value, ModificationContext::ApiOnly)
                .parse(&format!(
                    "= Document Title\n\n|===\na|\n= Nested Document Title\n{cell}\n\ncontent\n|==="
                ));
            assert_css(&doc, "h1", 1);
            assert_css(&doc, ".tableblock h1", 0);
        }
    }

    #[test]
    #[ignore]
    // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/456): The
    // NOTE admonition and nested blocks do not render as `.admonitionblock` /
    // `.dlist` inside the cell. Enable when admonitions are implemented.
    fn asciidoc_content() {
        let doc = Parser::default().parse(
            "[cols=\"1e,1,5a\"]\n|===\n|Name |Backends |Description\n\n|badges |xhtml11, html5 |\nLink badges.\n\n[NOTE]\n====\nThe path names are relative.\n====\n|docinfo |All backends |\nThese attributes control docinfo:\n\ndocinfo:: Include x\ndocinfo1:: Include y\n|===",
        );

        assert_css(&doc, "table.tableblock > tbody > tr", 2);
        assert_xpath(
            &doc,
            "((/table/tbody/tr)[1]/td)[3]//*[@class=\"admonitionblock\"]",
            1,
        );
        assert_xpath(&doc, "((/table/tbody/tr)[2]/td)[3]//*[@class=\"dlist\"]", 1);
    }

    #[test]
    fn should_preserve_leading_indentation_in_contents_of_asciidoc_table_cell_if_contents_starts_with_newline()
     {
        let doc = Parser::default().parse("|===\na|\n $ command\na| paragraph\n|===");

        // Source-map line numbers: the table starts on line 1; cell 1's
        // separator is on line 2 and its inner document begins on line 3 (the
        // indented `$ command`); cell 2 is on line 4.
        let table = match doc.nested_blocks().next() {
            Some(crate::blocks::Block::Table(table)) => table,
            other => panic!("expected a table block, got {other:?}"),
        };
        assert_eq!(table.span().line(), 1);

        let cell1 = &table.body_rows()[0].cells()[0];
        assert_eq!(cell1.span().line(), 2);
        let crate::blocks::TableCellContent::AsciiDoc(inner) = cell1.content() else {
            panic!("expected an AsciiDoc cell");
        };
        assert_eq!(inner.blocks()[0].span().line(), 3);

        let cell2 = &table.body_rows()[1].cells()[0];
        assert_eq!(cell2.span().line(), 4);

        assert_css(&doc, "td", 2);
        assert_xpath(&doc, "(//td)[1]//*[@class=\"literalblock\"]", 1);
        assert_xpath(&doc, "(//td)[2]//*[@class=\"paragraph\"]", 1);
        assert_xpath(&doc, "(//pre)[1][text()=\"$ command\"]", 1);
        assert_xpath(&doc, "(//p)[1][text()=\"paragraph\"]", 1);
    }

    #[test]
    fn preprocessor_directive_on_first_line_of_an_asciidoc_table_cell_should_be_processed() {
        let handler =
            InlineFileHandler::from_pairs([("fixtures/include-file.adoc", "included content")]);
        let mut parser = Parser::default().with_include_file_handler(handler);
        let doc = parser.parse("|===\na|include::fixtures/include-file.adoc[]\n|===");

        // The `include::` on the cell's first line is expanded, so the cell holds
        // the included content rather than the literal directive.
        assert_rendered_contains(&doc, "included content");
    }

    // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/542): Deferred:
    // "error about unresolved preprocessor directive on first line of an AsciiDoc
    // table cell should have correct cursor" (Ruby 1728) asserts the file/line
    // cursor of the unresolved-directive error, which needs the cell's nested
    // source map threaded back to the parent — not yet implemented.

    #[test]
    #[ignore]
    // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/543):
    // Cross-reference resolution from inside an AsciiDoc table cell to a
    // reference in the main document.
    fn cross_reference_link_in_an_asciidoc_table_cell_should_resolve_to_reference_in_main_document()
    {
    }

    #[test]
    #[ignore]
    // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/543):
    // Cataloging an anchor at the start of an AsciiDoc table cell as a document
    // reference.
    fn should_discover_anchor_at_start_of_cell_and_register_it_as_a_reference() {}

    #[test]
    #[ignore]
    // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/543):
    // Cataloging an anchor at the start of a cell in an implicit header row when
    // the column has a style.
    fn should_catalog_anchor_at_start_of_cell_in_implicit_header_row_when_column_has_a_style() {}

    #[test]
    #[ignore]
    // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/543):
    // Cataloging an anchor at the start of a cell in an explicit header row when
    // the column has a style.
    fn should_catalog_anchor_at_start_of_cell_in_explicit_header_row_when_column_has_a_style() {}

    #[test]
    #[ignore]
    // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/543):
    // Cataloging an anchor at the start of a cell in the first row.
    fn should_catalog_anchor_at_start_of_cell_in_first_row() {}

    #[test]
    #[ignore]
    // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/544):
    // Footnotes must not be shared between an AsciiDoc table cell and the main
    // document.
    fn footnotes_should_not_be_shared_between_an_asciidoc_table_cell_and_the_main_document() {}

    // Backend-specific test omitted: DocBook ("callout numbers should be
    // globally unique, including AsciiDoc table cells"). Out of scope: it
    // asserts only against DocBook output (`backend: 'docbook'`, `//co` /
    // `//callout` elements), which the crate does not target.

    // Out of scope (omitted): compatibility mode is a stated limitation of the
    // crate, so the three "compat mode ... in AsciiDoc table cell" tests are not
    // ported.

    #[test]
    fn nested_table() {
        let doc = Parser::default().parse(
            "[cols=\"1,2a\"]\n|===\n|Normal cell\n|Cell with nested table\n[cols=\"2,1\"]\n!===\n!Nested table cell 1 !Nested table cell 2\n!===\n|===",
        );

        assert_css(&doc, "table", 2);
        assert_css(&doc, "table table", 1);
        // Ruby uses `td:nth-child(2) table`; the nested table sits in the second
        // cell of the (single) outer row.
        assert_xpath(&doc, "/table/tbody/tr/td[2]//table", 1);
        assert_xpath(&doc, "/table/tbody/tr/td[2]//table/tbody/tr/td", 2);
    }

    #[test]
    fn can_set_format_of_nested_table_to_psv() {
        let doc = Parser::default().parse(
            "[cols=\"2*\"]\n|===\n|normal cell\na|\n[format=psv]\n!===\n!nested cell\n!===\n|===",
        );

        assert_css(&doc, "table", 2);
        assert_css(&doc, "table table", 1);
        assert_xpath(&doc, "/table/tbody/tr/td[2]//table", 1);
        assert_xpath(&doc, "/table/tbody/tr/td[2]//table/tbody/tr/td", 1);
    }

    // Attribute/option inheritance into the nested AsciiDoc-cell document is
    // implemented; the following remain unported for narrower reasons. The
    // `to_dir` test needs the nested cell exposed as an introspectable document
    // object carrying inherited options; the `toc` tests need TOC rendering (a
    // separate unimplemented feature).

    #[test]
    #[ignore]
    // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/545): AsciiDoc
    // table cell should expose the nested document for introspection of inherited
    // options (here, `to_dir`).
    fn asciidoc_table_cell_should_inherit_to_dir_option_from_parent_document() {}

    #[test]
    #[ignore]
    // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/546): AsciiDoc
    // table cell should not inherit the toc setting.
    fn asciidoc_table_cell_should_not_inherit_toc_setting_from_parent_document() {}

    #[test]
    #[ignore]
    // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/546): enabling
    // toc in an AsciiDoc table cell.
    fn should_be_able_to_enable_toc_in_an_asciidoc_table_cell() {}

    #[test]
    #[ignore]
    // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/546): enabling
    // toc in an AsciiDoc table cell even if hard unset by the API.
    fn should_be_able_to_enable_toc_in_an_asciidoc_table_cell_even_if_hard_unset_by_api() {}

    #[test]
    #[ignore]
    // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/546): enabling
    // toc in both the outer document and an AsciiDoc table cell.
    fn should_be_able_to_enable_toc_in_both_outer_document_and_in_an_asciidoc_table_cell() {}

    #[test]
    fn document_in_an_asciidoc_table_cell_should_not_see_doctitle_of_parent() {
        let doc = Parser::default()
            .parse("= Document Title\n\n[cols=\"1a\"]\n|===\n|AsciiDoc content\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > tbody > tr > td", 1);

        // The cell is its own (nested) document and does not inherit the parent's
        // doctitle, so its lone paragraph is not promoted into a preamble: the
        // cell renders a bare `.paragraph`, never a `#preamble`.
        assert_css(&doc, "table > tbody > tr > td #preamble", 0);
        assert_css(&doc, "table > tbody > tr > td .paragraph", 1);
    }

    #[test]
    #[ignore]
    // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/547):
    // per-cell background colors depend on the `{set:cellbgcolor:...}` /
    // `{set:cellbgcolor!}` attribute-assignment reference, which the
    // substitution layer does not implement. `{set:...}` is of uncertain status
    // in the AsciiDoc language, so this is deferred pending that decision.
    fn cell_background_color() {}

    #[test]
    fn should_warn_if_table_block_is_not_terminated() {
        let doc = Parser::default().parse("outside\n\n|===\n|\ninside\n\nstill inside\n\neof");

        assert_xpath(&doc, "/table", 1);

        let warnings: Vec<_> = doc.warnings().collect();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].warning, WarningType::UnterminatedDelimitedBlock);
        assert_eq!(warnings[0].source.line(), 3);
    }

    #[test]
    #[ignore]
    // TODO (https://github.com/asciidoc-rs/asciidoc-parser/issues/542): an
    // unterminated example block inside an AsciiDoc table cell that is itself
    // attached to a list item — Asciidoctor reports the warning at the inner
    // block's line (9). Enable once the cursor/line of nested-cell warnings is
    // tracked (same nested-cell source-map work as #542).
    fn should_show_correct_line_number_in_warning_about_unterminated_block_inside_asciidoc_table_cell()
     {
        let doc = Parser::default().parse(
            "outside\n\n* list item\n+\n|===\n|cell\na|inside\n\n====\nunterminated example block\n|===\n\neof",
        );

        assert_xpath(&doc, "//ul//table", 1);

        let warnings: Vec<_> = doc.warnings().collect();
        assert!(
            warnings
                .iter()
                .any(|w| w.warning == WarningType::UnterminatedDelimitedBlock
                    && w.source.line() == 9)
        );
    }

    #[test]
    fn custom_separator_for_an_asciidoc_table_cell() {
        let doc = Parser::default().parse(
            "[cols=2,separator=!]\n|===\n!Pipe output to vim\na!\n----\nasciidoctor -o - -s test.adoc | view -\n----\n|===",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > tbody > tr", 1);
        assert_xpath(&doc, "(/table/tbody/tr)[1]/td", 2);
        assert_xpath(&doc, "((/table/tbody/tr)[1]/td)[1]//p", 1);
        assert_xpath(
            &doc,
            "((/table/tbody/tr)[1]/td)[2]//*[@class=\"listingblock\"]",
            1,
        );
    }

    // Backend-specific tests omitted: DocBook ("table with breakable option
    // docbook 5", "table with unbreakable option docbook 5").

    #[test]
    fn no_implicit_header_row_if_cell_in_first_line_is_quoted_and_spans_multiple_lines() {
        let doc =
            Parser::default().parse("[cols=2*l]\n,===\n\"A1\n\nA1 continued\",B1\nA2,B2\n,===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > thead", 0);
        assert_css(&doc, "table > tbody", 1);
        assert_css(&doc, "table > tbody > tr", 2);
        assert_xpath(&doc, "(//td)[1]//pre[text()=\"A1\n\nA1 continued\"]", 1);
    }
}

mod dsv {
    use crate::tests::prelude::*;

    #[test]
    fn converts_simple_dsv_table() {
        let doc = Parser::default().parse(
            "[width=\"75%\",format=\"dsv\"]\n|===\nroot:x:0:0:root:/root:/bin/bash\nbin:x:1:1:bin:/bin:/sbin/nologin\nmysql:x:27:27:MySQL\\:Server:/var/lib/mysql:/bin/bash\ngdm:x:42:42::/var/lib/gdm:/sbin/nologin\nsshd:x:74:74:Privilege-separated SSH:/var/empty/sshd:/sbin/nologin\nnobody:x:99:99:Nobody:/:/sbin/nologin\n|===",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col[width=\"14.2857%\"]", 6);
        // Ruby uses `col:last-of-type`; the indexed XPath form is equivalent.
        assert_xpath(&doc, "(/table/colgroup/col)[7][@width=\"14.2858%\"]", 1);
        assert_css(&doc, "table > tbody > tr", 6);
        assert_xpath(&doc, "//tr[4]/td[5]/p/text()", 0);
        assert_xpath(&doc, "//tr[3]/td[5]/p[text()=\"MySQL:Server\"]", 1);
    }

    #[test]
    fn dsv_format_shorthand() {
        let doc = Parser::default().parse(":===\na:b:c\n1:2:3\n:===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 3);
        assert_css(&doc, "table > tbody > tr", 2);
        assert_xpath(&doc, "(/table/tbody/tr)[1]/td", 3);
        assert_xpath(&doc, "(/table/tbody/tr)[2]/td", 3);
    }

    #[test]
    fn single_cell_in_dsv_table_should_only_produce_single_row() {
        let doc = Parser::default().parse(":===\nsingle cell\n:===");

        assert_css(&doc, "table td", 1);
    }

    #[test]
    fn should_treat_trailing_colon_as_an_empty_cell() {
        let doc = Parser::default().parse(":===\nA1:\nB1:B2\nC1:C2\n:===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > tbody > tr", 3);
        assert_xpath(&doc, "/table/tbody/tr[1]/td", 2);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[1]/p[text()=\"A1\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[2]/p", 0);
        assert_xpath(&doc, "/table/tbody/tr[2]/td[1]/p[text()=\"B1\"]", 1);
    }
}

mod csv {
    use crate::tests::prelude::{inline_file_handler::InlineFileHandler, *};

    #[test]
    fn should_treat_trailing_comma_as_an_empty_cell() {
        let doc = Parser::default().parse(",===\nA1,\nB1,B2\nC1,C2\n,===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > tbody > tr", 3);
        assert_xpath(&doc, "/table/tbody/tr[1]/td", 2);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[1]/p[text()=\"A1\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[2]/p", 0);
        assert_xpath(&doc, "/table/tbody/tr[2]/td[1]/p[text()=\"B1\"]", 1);
    }

    #[test]
    fn should_log_error_but_not_crash_if_cell_data_has_unclosed_quote() {
        let doc = Parser::default().parse(",===\na,b\nc,\"\n,===");

        // The unclosed quote recovers to an empty cell without crashing, and an
        // error is logged for line 3 (the `c,"` line).
        assert_css(&doc, "table", 1);
        assert_css(&doc, "table td", 4);
        assert_xpath(&doc, "(//td)[4]/p", 0);

        let warnings: Vec<_> = doc.warnings().collect();
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::TableCsvDataHasUnclosedQuote
        );
        assert_eq!(warnings[0].source.line(), 3);
    }

    #[test]
    fn should_preserve_newlines_in_quoted_csv_values() {
        let doc = Parser::default().parse(
            "[cols=\"1,1,1l\"]\n,===\n\"A\nB\nC\",\"one\n\ntwo\n\nthree\",\"do\n\nre\n\nme\"\n,===",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 3);
        assert_css(&doc, "table > tbody > tr", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td", 3);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[1]/p[text()=\"A\nB\nC\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[2]/p", 3);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[2]/p[1][text()=\"one\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[2]/p[2][text()=\"two\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[2]/p[3][text()=\"three\"]", 1);
        assert_xpath(
            &doc,
            "/table/tbody/tr[1]/td[3]//pre[text()=\"do\n\nre\n\nme\"]",
            1,
        );
    }

    #[test]
    fn should_not_drop_trailing_empty_cell_in_tsv_data_when_loaded_from_an_include_file() {
        // The `1\t2\t` record ends with a tab, so its third field is empty. The
        // include is the delivery mechanism; the behavior under test is that the
        // trailing empty cell survives.
        let handler = InlineFileHandler::from_pairs([(
            "fixtures/data.tsv",
            "First\tSecond\tThird\na\tb\tc\n1\t2\t\nx\ty\tz\n",
        )]);
        let mut parser = Parser::default().with_include_file_handler(handler);
        let doc = parser.parse("[%header,format=tsv]\n|===\ninclude::fixtures/data.tsv[]\n|===");

        assert_css(&doc, "table > tbody > tr", 3);
        assert_css(&doc, "table > tbody > tr:nth-child(1) > td", 3);
        assert_css(&doc, "table > tbody > tr:nth-child(2) > td", 3);
        assert_css(&doc, "table > tbody > tr:nth-child(3) > td", 3);
        assert_css(
            &doc,
            "table > tbody > tr:nth-child(2) > td:nth-child(3):empty",
            1,
        );
    }

    #[test]
    fn mixed_unquoted_records_and_quoted_records_with_escaped_quotes_commas_and_wrapped_lines() {
        let doc = Parser::default().parse(
            "[format=\"csv\",options=\"header\"]\n|===\nYear,Make,Model,Description,Price\n1997,Ford,E350,\"ac, abs, moon\",3000.00\n1999,Chevy,\"Venture \"\"Extended Edition\"\"\",\"\",4900.00\n1999,Chevy,\"Venture \"\"Extended Edition, Very Large\"\"\",,5000.00\n1996,Jeep,Grand Cherokee,\"MUST SELL!\nair, moon roof, loaded\",4799.00\n2000,Toyota,Tundra,\"\"\"This one's gonna to blow you're socks off,\"\" per the sticker\",10000.00\n2000,Toyota,Tundra,\"Check it, \"\"this one's gonna to blow you're socks off\"\", per the sticker\",10000.00\n|===",
        );

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col[width=\"20%\"]", 5);
        assert_css(&doc, "table > thead > tr", 1);
        assert_css(&doc, "table > tbody > tr", 6);
        assert_xpath(
            &doc,
            "((//tbody/tr)[1]/td)[4]/p[text()=\"ac, abs, moon\"]",
            1,
        );
        assert_xpath(
            &doc,
            "((//tbody/tr)[2]/td)[3]/p[text()='Venture \"Extended Edition\"']",
            1,
        );
        assert_xpath(
            &doc,
            "((//tbody/tr)[4]/td)[4]/p[text()=\"MUST SELL!\nair, moon roof, loaded\"]",
            1,
        );
        assert_xpath(
            &doc,
            "((//tbody/tr)[5]/td)[4]/p[text()='\"This one\u{2019}s gonna to blow you\u{2019}re socks off,\" per the sticker']",
            1,
        );
        assert_xpath(
            &doc,
            "((//tbody/tr)[6]/td)[4]/p[text()='Check it, \"this one\u{2019}s gonna to blow you\u{2019}re socks off\", per the sticker']",
            1,
        );
    }

    #[test]
    fn should_allow_quotes_around_a_csv_value_to_be_on_their_own_lines() {
        let doc = Parser::default().parse("[cols=2*]\n,===\n\"\nA\n\",\"\nB\n\"\n,===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 2);
        assert_css(&doc, "table > tbody > tr", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td", 2);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[1]/p[text()=\"A\"]", 1);
        assert_xpath(&doc, "/table/tbody/tr[1]/td[2]/p[text()=\"B\"]", 1);
    }

    #[test]
    fn csv_format_shorthand() {
        let doc = Parser::default().parse(",===\na,b,c\n1,2,3\n,===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 3);
        assert_css(&doc, "table > tbody > tr", 2);
        assert_xpath(&doc, "(/table/tbody/tr)[1]/td", 3);
        assert_xpath(&doc, "(/table/tbody/tr)[2]/td", 3);
    }

    #[test]
    fn tsv_as_format() {
        let doc = Parser::default().parse("[format=tsv]\n,===\na\tb\tc\n1\t2\t3\n,===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 3);
        assert_css(&doc, "table > tbody > tr", 2);
        assert_xpath(&doc, "(/table/tbody/tr)[1]/td", 3);
        assert_xpath(&doc, "(/table/tbody/tr)[2]/td", 3);
    }

    #[test]
    fn custom_csv_separator() {
        let doc = Parser::default().parse("[format=csv,separator=;]\n|===\na;b;c\n1;2;3\n|===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 3);
        assert_css(&doc, "table > tbody > tr", 2);
        assert_xpath(&doc, "(/table/tbody/tr)[1]/td", 3);
        assert_xpath(&doc, "(/table/tbody/tr)[2]/td", 3);
    }

    #[test]
    fn tab_as_separator() {
        let doc = Parser::default().parse("[separator=\\t]\n,===\na\tb\tc\n1\t2\t3\n,===");

        assert_css(&doc, "table", 1);
        assert_css(&doc, "table > colgroup > col", 3);
        assert_css(&doc, "table > tbody > tr", 2);
        assert_xpath(&doc, "(/table/tbody/tr)[1]/td", 3);
        assert_xpath(&doc, "(/table/tbody/tr)[2]/td", 3);
    }

    #[test]
    fn single_cell_in_csv_table_should_only_produce_single_row() {
        let doc = Parser::default().parse(",===\nsingle cell\n,===");

        assert_css(&doc, "table td", 1);
    }

    #[test]
    fn cell_formatted_with_asciidoc_style() {
        let doc = Parser::default().parse(
            "[cols=\"1,1,1a\",separator=;]\n,===\nelement;description;example\n\nthematic break,a visible break; also known as a horizontal rule;---\n,===",
        );

        assert_css(&doc, "table tbody hr", 1);
    }

    #[test]
    fn should_strip_whitespace_around_contents_of_asciidoc_cell() {
        let doc = Parser::default().parse(
            "[cols=\"1,1,1a\",separator=;]\n,===\nelement;description;example\n\nparagraph;contiguous lines of words and phrases;\"\n  one sentence, one line\n  \"\n,===",
        );

        assert_xpath(
            &doc,
            "/table/tbody//*[@class=\"paragraph\"]/p[text()=\"one sentence, one line\"]",
            1,
        );
    }
}
