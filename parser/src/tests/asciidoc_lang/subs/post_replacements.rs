use pretty_assertions_sorted::assert_eq;

use crate::{
    Parser,
    blocks::{Block, IsBlock},
    tests::prelude::*,
};

track_file!("docs/modules/subs/pages/post-replacements.adoc");

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
    use pretty_assertions_sorted::assert_eq;

    use crate::{
        Parser,
        blocks::{Block, IsBlock},
        tests::prelude::*,
    };

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

    #[ignore]
    #[test]
    fn attribute_entry_values() {
        // TO DO (https://github.com/asciidoc-rs/asciidoc-parser/issues/307):
        // Search for an example of post-replacement mattering for attribute values.
        //
        // No test here because I don't know how to test this.
        to_do_verifies!(
            r#"
|Attribute entry values |{n}

"#
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

    #[ignore]
    #[test]
    fn titles() {
        // TO DO (https://github.com/asciidoc-rs/asciidoc-parser/issues/308):
        //
        // Not sure if a block title can span multiple lines. If so, what does that
        // syntax look like and how does it interact with post-replacement substitution?
        to_do_verifies!(
            r#"
|Titles |{y}
|===

"#
        );

        let doc = Parser::default().parse(".Title abc +\ndef\n****\nStuff > nonsense\n****");

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

mod post_replacements_substitution_value {
    use pretty_assertions_sorted::assert_eq;

    use crate::{
        Parser,
        blocks::{Block, IsBlock},
        tests::prelude::*,
    };

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

        let doc = Parser::default().parse("[subs=post-replacements]\nabc *bold* +\ndef");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            "abc <strong>bold</strong><br>\ndef"
        );
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
