use crate::tests::prelude::*;

track_file!("ref/asciidoc-lang/docs/modules/subs/pages/macros.adoc");

non_normative!(
    r#"
= Macro Substitutions
:navtitle: Macros
:table-caption: Table
:y: Yes
//icon:check[role="green"]
:n: No
//icon:times[role="red"]

The content of inline and block macros, such as cross references, links, and block images, are processed by the macros substitution step.
The macros step replaces a macro's content with the appropriate built-in and user-defined configuration.

"#
);

mod default_macros_substitution {
    use crate::{blocks::Block, tests::prelude::*};

    non_normative!(
        r#"
== Default macros substitution

<<table-macros>> lists the specific blocks and inline elements the macros substitution step applies to automatically.

.Blocks and inline elements subject to the macros substitution
[#table-macros%autowidth,cols="~,^~"]
|===
|Blocks and elements |Substitution step applied by default

"#
    );

    #[test]
    fn attribute_entry_values() {
        verifies!(
            r#"
|Attribute entry values |Only the xref:pass:pass-macro.adoc#inline-pass[pass macro]

"#
        );

        let doc = Parser::default().parse(":not-icon: icon:heart[]\n:only: pass:q[*bold*]\n\nNot icon: pass:a[{not-icon}]\nOnly: pass:a[{only}]");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            sb1.content().rendered(),
            "Not icon: icon:heart[]\nOnly: <strong>bold</strong>"
        );
    }

    #[test]
    fn comments() {
        verifies!(
            r#"
|Comments |{n}

"#
        );

        let doc = Parser::default().parse("////\nicon:heart[]\n////");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::RawDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "icon:heart[]");
    }

    #[test]
    fn examples() {
        verifies!(
            r#"
|Examples |{y}

"#
        );

        let doc = Parser::default().parse(":icons:\n\n====\nHello icon:heart[] Asciidoc.\n====");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::CompoundDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        // Dig an extra level deeper to get the simple block that has the content.
        let block1 = block1.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            r#"Hello <span class="icon"><img src="./images/icons/heart.png" alt="heart"></span> Asciidoc."#
        );
    }

    #[test]
    fn headers() {
        verifies!(
            r#"
|Headers |{n}

"#
        );

        let doc = Parser::default().parse("= Title icon:heart[]-less");

        let title = doc.header().title().unwrap();
        assert_eq!(title, "Title icon:heart[]-less");
    }

    #[test]
    fn literal_listings_and_source() {
        verifies!(
            r#"
|Literal, listings, and source |{n}

"#
        );

        let doc = Parser::default().parse("....\nfoo icon:heart[] bar\n....");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::RawDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "foo icon:heart[] bar");
    }

    #[test]
    fn macros() {
        verifies!(
            r#"
|Macros |{y}

"#
        );

        // Can one macro contain another? Yes. The macros substitution step is
        // applied to the *positional text* of a macro, so a macro nested in that
        // text is itself processed. The canonical example is an inline image in
        // the text of a link: the link text `image:logo.png[Logo]` is
        // substituted into an image span, which then becomes the link's content.
        let doc = Parser::default().parse("https://example.org[image:logo.png[Logo]]");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            r#"<a href="https://example.org"><span class="image"><img src="logo.png" alt="Logo"></span></a>"#
        );

        // The same nesting works when the inner macro appears mid-sentence in
        // the outer link's text.
        let doc =
            Parser::default().parse("See https://example.org[the image:logo.png[Logo] here].");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            r#"See <a href="https://example.org">the <span class="image"><img src="logo.png" alt="Logo"></span> here</a>."#
        );

        // Nesting also applies to the text of a cross-reference macro: the
        // inner image is processed inside the `xref:` target's text.
        let doc =
            Parser::default().parse("[[sec]]Target.\n\nSee xref:sec[image:logo.png[Logo]] now.");

        let block2 = doc.nested_blocks().nth(1).unwrap();

        let Block::Simple(block2) = block2 else {
            panic!("Unexpected block type: {block2:?}");
        };

        assert_eq!(
            block2.content().rendered(),
            r##"See <a href="#sec"><span class="image"><img src="logo.png" alt="Logo"></span></a> now."##
        );
    }

    #[test]
    fn open() {
        verifies!(
            r#"
|Open |{y}

"#
        );

        let doc = Parser::default().parse(":icons:\n\n--\nOpened icon:heart[] closed!\n--");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::CompoundDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        // Dig an extra level deeper to get the simple block that has the content.
        let block1 = block1.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            r#"Opened <span class="icon"><img src="./images/icons/heart.png" alt="heart"></span> closed!"#
        );
    }

    #[test]
    fn paragraphs() {
        verifies!(
            r#"
|Paragraphs |{y}

"#
        );

        let doc = Parser::default().parse(":icons:\n\nThis is a icon:heart[] paragraph.");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            r#"This is a <span class="icon"><img src="./images/icons/heart.png" alt="heart"></span> paragraph."#
        );
    }

    #[test]
    fn passthrough_blocks() {
        verifies!(
            r#"
|Passthrough blocks |{n}

"#
        );

        let doc = Parser::default().parse(":icons:\n\n++++\nfoo icon:heart[] bar\n++++");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::RawDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "foo icon:heart[] bar");
    }

    #[test]
    fn quotes_and_verses() {
        verifies!(
            r#"
|Quotes and verses |{y}

"#
        );

        let doc = Parser::default().parse(":icons:\n\n____\nThis icon:heart[] that\n____");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Quote(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        // Dig an extra level deeper to get the simple block that has the content.
        let block1 = block1.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            r#"This <span class="icon"><img src="./images/icons/heart.png" alt="heart"></span> that"#
        );
    }

    #[test]
    fn sidebars() {
        verifies!(
            r#"
|Sidebars |{y}

"#
        );

        let doc = Parser::default().parse(":icons:\n\n****\nStuff icon:heart[] nonsense\n****");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::CompoundDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        // Dig an extra level deeper to get the simple block that has the content.
        let block1 = block1.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            r#"Stuff <span class="icon"><img src="./images/icons/heart.png" alt="heart"></span> nonsense"#
        );
    }

    #[test]
    fn tables() {
        verifies!(
            r#"
|Tables |Varies

"#
        );

        // The macros substitution applies to default table cells but not to
        // literal (`l`) cells, hence "Varies".
        let doc = Parser::default()
            .parse("|===\n|https://example.org[Example]\nl|https://example.org[Example]\n|===");

        let Some(Block::Table(table)) = doc.nested_blocks().next() else {
            panic!("expected a table block");
        };

        let cells: Vec<_> = table.body_rows().iter().flat_map(|r| r.cells()).collect();

        let crate::blocks::TableCellContent::Simple(default_cell) = cells[0].content() else {
            panic!("expected simple cell content");
        };
        assert_eq!(
            default_cell.rendered(),
            r#"<a href="https://example.org">Example</a>"#
        );

        let crate::blocks::TableCellContent::Simple(literal_cell) = cells[1].content() else {
            panic!("expected simple cell content");
        };
        assert_eq!(literal_cell.rendered(), "https://example.org[Example]");
    }

    #[test]
    fn titles() {
        verifies!(
            r#"
|Titles |{y}
|===

"#
        );

        let doc = Parser::default()
            .parse(":icons:\n\n.Title icon:heart[] such\n****\nStuff > nonsense\n****");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::CompoundDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.title().unwrap(),
            r#"Title <span class="icon"><img src="./images/icons/heart.png" alt="heart"></span> such"#
        );
    }
}

mod macros_substitution_value {
    use crate::{blocks::Block, tests::prelude::*};

    non_normative!(
        r#"
== macros substitution value

The macros substitution step can be modified on blocks and inline elements.
"#
    );

    #[test]
    fn for_blocks() {
        verifies!(
            r#"
For blocks, the step's name, `macros`, can be assigned to the xref:apply-subs-to-blocks.adoc[subs attribute].
"#
        );

        let doc =
            Parser::default().parse(":icons:\n\n[subs=macros]\nHello icon:heart[] *Asciidoc*.");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            r#"Hello <span class="icon"><img src="./images/icons/heart.png" alt="heart"></span> *Asciidoc*."#
        );
    }

    #[test]
    fn for_inline_elements() {
        verifies!(
            r#"
For inline elements, the built-in values `m` or `macros` can be applied to xref:apply-subs-to-text.adoc[inline text] to add the macros substitution step.
"#
        );

        let doc = Parser::default()
            .parse(":icons:\n\npass:m[Hello icon:heart[\\] *Asciidoc*] and then ...");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            r#"Hello <span class="icon"><img src="./images/icons/heart.png" alt="heart"></span> *Asciidoc* and then &#8230;&#8203;"#
        );
    }
}
