use crate::{blocks::Block, tests::prelude::*};

track_file!("ref/asciidoc-lang/docs/modules/subs/pages/post-replacements.adoc");

non_normative!(
    r#"
= Post Replacement Substitutions
:navtitle: Post Replacements
:table-caption: Table
:y: Yes
//icon:check[role="green"]
:n: No
//icon:times[role="red"]

"#
);

#[test]
fn replaces_plus_with_br() {
    verifies!(
        r#"
The line break character, `{plus}`, is replaced when the `post_replacements` substitution step runs.

"#
    );

    let doc = Parser::default().parse("first line +\nsecond line");

    let block1 = doc.nested_blocks().next().unwrap();

    let Block::Simple(sb1) = block1 else {
        panic!("Unexpected block type: {block1:?}");
    };

    assert_eq!(sb1.content().rendered(), "first line<br>\nsecond line");
}

mod default_post_replacements_substitution {
    use crate::{blocks::Block, tests::prelude::*};

    non_normative!(
        r#"
== Default post replacements substitution

<<table-post>> lists the specific blocks and inline elements the post replacements substitution step applies to automatically.

.Blocks and inline elements subject to the post replacements substitution
[#table-post%autowidth,cols="~,^~"]
|===
|Blocks and elements |Substitution step applied by default

"#
    );

    #[test]
    fn attribute_entry_values() {
        verifies!(
            r#"
|Attribute entry values |{n}

"#
        );

        // The post replacements step is _not_ applied when an attribute entry
        // value is parsed. A xref:attributes:wrap-values.adoc#hard[hard-wrapped]
        // attribute value therefore stores the line-break marker (` +`)
        // verbatim rather than converting it to a `<br>`. The marker is only
        // interpreted as a line break later, if and when the value is used in a
        // block whose substitutions include post_replacements.
        //
        // A passthrough block with `subs=attributes` resolves the attribute
        // reference but does not run post_replacements, so the stored ` +`
        // survives verbatim. This demonstrates the step was not applied to the
        // attribute value itself.
        let doc = Parser::default().parse(
            ":haiku: Write your docs in text, + \\\nAsciiDoc makes it easy, + \\\nNow get back to work!\n\n[subs=attributes]\n++++\n{haiku}\n++++",
        );

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::RawDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            "Write your docs in text, +\nAsciiDoc makes it easy, +\nNow get back to work!"
        );

        // By contrast, referencing the same attribute in a normal paragraph
        // _does_ produce line breaks, because post_replacements is applied to
        // the paragraph (not to the attribute value). This confirms the `<br>`
        // comes from the containing block, not from the attribute entry value.
        let doc = Parser::default().parse(
            ":haiku: Write your docs in text, + \\\nAsciiDoc makes it easy, + \\\nNow get back to work!\n\n{haiku}",
        );

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            "Write your docs in text,<br>\nAsciiDoc makes it easy,<br>\nNow get back to work!"
        );
    }

    #[test]
    fn comments() {
        verifies!(
            r#"
|Comments |{n}

"#
        );

        let doc = Parser::default().parse("////\nabc +\ndef\n////");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::RawDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "abc +\ndef");
    }

    #[test]
    fn examples() {
        verifies!(
            r#"
|Examples |{y}

"#
        );

        let doc = Parser::default().parse(":icons:\n\n====\nabc +\ndef\n====");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::CompoundDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        // Dig an extra level deeper to get the simple block that has the content.
        let block1 = block1.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "abc<br>\ndef");
    }

    #[test]
    fn headers() {
        verifies!(
            r#"
|Headers |{n}

"#
        );

        let doc = Parser::default().parse("= abc +\ndef");

        let title = doc.header().title().unwrap();
        assert_eq!(title, "abc +");
    }

    #[test]
    fn literal_listings_and_source() {
        verifies!(
            r#"
|Literal, listings, and source |{n}

"#
        );

        let doc = Parser::default().parse("....\nabc +\ndef\n....");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::RawDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "abc +\ndef");
    }

    #[test]
    fn macros() {
        verifies!(
            r#"
|Macros |{y} +
"#
        );

        let doc = Parser::default().parse(
            "Click image:pause.png[title=Pause pass:p[{abc +\ndef}] Resume] when you need a break.",
        );

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            "Click <span class=\"image\"><img src=\"pause.png\" alt=\"pause\" title=\"Pause {abc<br>\ndef} Resume\"></span> when you need a break."
        );
    }

    #[test]
    fn macros_except_pass_macro() {
        verifies!(
            r#"
(except passthrough macros)

"#
        );

        let doc =
            Parser::default().parse("Click +++*Pause* +\n and Resume+++ when you need a break.");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            "Click *Pause* +\n and Resume when you need a break."
        );
    }

    #[test]
    fn open() {
        verifies!(
            r#"
|Open |{y}

"#
        );

        let doc = Parser::default().parse(":icons:\n\n--\nabc +\ndef\n--");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::CompoundDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        // Dig an extra level deeper to get the simple block that has the content.
        let block1 = block1.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "abc<br>\ndef");
    }

    #[test]
    fn paragraphs() {
        verifies!(
            r#"
|Paragraphs |{y}

"#
        );

        let doc = Parser::default().parse("abc +\ndef");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "abc<br>\ndef");
    }

    #[test]
    fn passthrough_blocks() {
        verifies!(
            r#"
|Passthrough blocks |{n}

"#
        );

        let doc = Parser::default().parse("++++\nabc +\ndef\n++++");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::RawDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "abc +\ndef");
    }

    #[test]
    fn quotes_and_verses() {
        verifies!(
            r#"
|Quotes and verses |{y}

"#
        );

        let doc = Parser::default().parse("____\nabc +\ndef\n____");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Quote(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        // Dig an extra level deeper to get the simple block that has the content.
        let block1 = block1.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "abc<br>\ndef");
    }

    #[test]
    fn sidebars() {
        verifies!(
            r#"
|Sidebars |{y}

"#
        );

        let doc = Parser::default().parse("****\nabc +\ndef\n****");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::CompoundDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        // Dig an extra level deeper to get the simple block that has the content.
        let block1 = block1.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "abc<br>\ndef");
    }

    #[test]
    fn tables() {
        verifies!(
            r#"
|Tables |Varies

"#
        );

        // The post replacements substitution (the line-break `+`) applies to
        // default table cells but not to literal (`l`) cells, hence "Varies".
        let doc = Parser::default().parse("|===\n|abc +\ndef\nl|abc +\nghi\n|===");

        let Some(Block::Table(table)) = doc.nested_blocks().next() else {
            panic!("expected a table block");
        };

        let cells: Vec<_> = table.body_rows().iter().flat_map(|r| r.cells()).collect();

        let crate::blocks::TableCellContent::Simple(default_cell) = cells[0].content() else {
            panic!("expected simple cell content");
        };
        assert_eq!(default_cell.rendered(), "abc<br>\ndef");

        let crate::blocks::TableCellContent::Simple(literal_cell) = cells[1].content() else {
            panic!("expected simple cell content");
        };
        assert_eq!(literal_cell.rendered(), "abc +\nghi");
    }

    // A block title cannot span multiple lines via a trailing `+`: a title is a
    // single line, and a trailing ` +` is retained literally rather than joined to
    // the following line. Because a title never spans lines, the post_replacements
    // line-break substitution has nothing to act on here, so the `|Titles |{y}`
    // row has no observable effect to assert and is out of scope for verification.
    // Treated as non-normative.
    non_normative!(
        r#"
|Titles |{y}
|===

"#
    );
}

mod post_replacements_substitution_value {
    use crate::{blocks::Block, tests::prelude::*};

    non_normative!(
        r#"
== post_replacements substitution value

The post replacements substitution step can be modified on blocks and inline elements.
"#
    );

    #[test]
    fn for_blocks() {
        verifies!(
            r#"
For blocks, the step's name, `post_replacements`, can be assigned to the xref:apply-subs-to-blocks.adoc[subs attribute].
"#
        );

        // The step's name uses an underscore (`post_replacements`). Since the
        // `subs` value replaces the block's default substitution list, only
        // the line-break post replacement is applied; the bold text is left
        // as-is (verified against Asciidoctor 2.0.26).
        let doc = Parser::default().parse("[subs=post_replacements]\nabc *bold* +\ndef");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "abc *bold*<br>\ndef");
    }

    #[test]
    fn for_inline_elements() {
        verifies!(
            r#"
For inline elements, the built-in values `p` or `post_replacements` can be applied to xref:apply-subs-to-text.adoc[inline text] to add the post replacements substitution step.
"#
        );

        let doc = Parser::default().parse("pass:p[abc +\n *bold*]{sp}and then ...");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            "abc<br>\n *bold* and then &#8230;&#8203;"
        );
    }
}
