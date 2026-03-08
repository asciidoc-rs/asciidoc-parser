use crate::tests::prelude::*;

track_file!("docs/modules/subs/pages/macros.adoc");

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
    use crate::{
        Parser,
        blocks::{Block, IsBlock},
        tests::prelude::*,
    };

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

    #[ignore]
    #[test]
    fn macros() {
        // TO DO (https://github.com/asciidoc-rs/asciidoc-parser/issues/305):
        // I can't concieve of how macro substitutions can be applied _within_
        // macros. Deferring for now.

        verifies!(
            r#"
|Macros |{y}

"#
        );

        let doc = Parser::default()
            .parse(":icons:\n:heart: icon:heart[]\n\nClick image:pause.png[title=Pause pass:a[{heart}] Resume] when you need a break.");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            r#"Click <span class="image"><img src="pause.png" alt="pause" title="Pause &#169; Resume"></span> when you need a break."#
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

    #[ignore]
    #[test]
    fn tables() {
        to_do_verifies!(
            r#"
|Tables |Varies

"#
        );

        todo!("Write test once table parsing is implemented");

        // Blocked on https://github.com/asciidoc-rs/asciidoc-parser/issues/296:
        // Implement table parsing
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
    use crate::{
        Parser,
        blocks::{Block, IsBlock},
        tests::prelude::*,
    };

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
