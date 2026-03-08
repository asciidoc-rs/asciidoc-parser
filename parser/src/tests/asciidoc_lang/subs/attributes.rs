use crate::tests::prelude::*;

track_file!("docs/modules/subs/pages/attributes.adoc");

non_normative!(
    r#"
= Attribute References Substitution
:navtitle: Attribute References
:table-caption: Table
:y: Yes
//icon:check[role="green"]
:n: No
//icon:times[role="red"]

Attribute references are replaced with the values of the attribute they reference when processed by the attributes substitution step.

"#
);

mod default_attributes_substitution {
    use crate::{
        Parser,
        blocks::{Block, IsBlock},
        tests::prelude::*,
    };

    non_normative!(
        r#"
== Default special characters substitution

<<table-special>> lists the specific blocks and inline elements the special characters substitution step applies to automatically.

.Blocks and inline elements subject to the special characters substitution
[#table-special%autowidth,cols="~,^~"]
|===
|Blocks and elements |Substitution step applied by default

"#
    );

    #[test]
    fn attribute_entry_values() {
        verifies!(
            r#"
|Attribute entry values |{y}

"#
        );

        let doc = Parser::default().parse(":lt-attr: abc{sp}def\n\nGoodbye {lt-attr} hello");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(sb1.content().rendered(), "Goodbye abc def hello");
    }

    #[test]
    fn comments() {
        verifies!(
            r#"
|Comments |{n}

"#
        );

        let doc = Parser::default().parse("////\nabc {sp} def\n////");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::RawDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "abc {sp} def");
    }

    #[test]
    fn examples() {
        verifies!(
            r#"
|Examples |{y}

"#
        );

        let doc = Parser::default().parse("====\nHello{sp}goodbye.\n====");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::CompoundDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        // Dig an extra level deeper to get the simple block that has the content.
        let block1 = block1.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "Hello goodbye.");
    }

    #[test]
    fn headers() {
        verifies!(
            r#"
|Headers |{y}

"#
        );

        let doc = Parser::default().parse("= Title & So{sp}On");

        let title = doc.header().title().unwrap();
        assert_eq!(title, "Title &amp; So On");
    }

    #[test]
    fn literal_listings_and_source() {
        verifies!(
            r#"
|Literal, listings, and source |{n}

"#
        );

        let doc = Parser::default().parse("....\nfoo{sp}bar\n....");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::RawDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "foo{sp}bar");
    }

    #[test]
    fn macros() {
        verifies!(
            r#"
|Macros |{y} +
"#
        );

        let doc = Parser::default()
            .parse("Click image:pause.png[title=Pause{sp}Resume] when you need a break.");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            r#"Click <span class="image"><img src="pause.png" alt="pause" title="Pause Resume"></span> when you need a break."#
        );
    }

    #[test]
    fn macros_except_pass_macro() {
        verifies!(
            r#"
(except triple plus and inline pass macros)

"#
        );

        let doc = Parser::default().parse("Click +++Pause{sp}Resume+++ when you need a break.");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            r#"Click Pause{sp}Resume when you need a break."#
        );
    }

    #[test]
    fn open() {
        verifies!(
            r#"
|Open |{y}

"#
        );

        let doc = Parser::default().parse("--\nOpened{sp}closed!\n--");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::CompoundDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        // Dig an extra level deeper to get the simple block that has the content.
        let block1 = block1.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "Opened closed!");
    }

    #[test]
    fn paragraphs() {
        verifies!(
            r#"
|Paragraphs |{y}

"#
        );

        let doc = Parser::default().parse("This is a{sp}<paragraph>.");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            r#"This is a &lt;paragraph&gt;."#
        );
    }

    #[test]
    fn passthrough_blocks() {
        verifies!(
            r#"
|Passthrough blocks |{n}

"#
        );

        let doc = Parser::default().parse("++++\nfoo{sp}bar\n++++");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::RawDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "foo{sp}bar");
    }

    #[test]
    fn quotes_and_verses() {
        verifies!(
            r#"
|Quotes and verses |{y}

"#
        );

        let doc = Parser::default().parse("____\nThis{sp}that\n____");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::CompoundDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        // Dig an extra level deeper to get the simple block that has the content.
        let block1 = block1.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "This that");
    }

    #[test]
    fn sidebars() {
        verifies!(
            r#"
|Sidebars |{y}

"#
        );

        let doc = Parser::default().parse("****\nStuff{sp}nonsense\n****");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::CompoundDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        // Dig an extra level deeper to get the simple block that has the content.
        let block1 = block1.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "Stuff nonsense");
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

        let doc = Parser::default().parse(".Title{sp}such\n****\nStuff > nonsense\n****");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::CompoundDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.title().unwrap(), "Title such");
    }
}

mod attributes_substitution_value {
    use crate::{
        Parser,
        blocks::{Block, IsBlock},
        tests::prelude::*,
    };

    non_normative!(
        r#"
== attributes substitution value

The attributes substitution step can be modified on blocks and the inline passthrough.
"#
    );

    #[test]
    fn for_blocks() {
        verifies!(
            r#"
For blocks, the step's name, `attributes`, can be assigned to the xref:apply-subs-to-blocks.adoc[subs attribute].
"#
        );

        let doc = Parser::default().parse("[subs=attributes]\nabc<lt{sp}space");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "abc<lt space");
    }

    #[test]
    fn for_inline_elements() {
        verifies!(
            r#"
For an inline passthrough, the built-in values `a` or `attributes` can be applied to xref:apply-subs-to-text.adoc[inline text] to add or remove the attributes substitution step.
"#
        );

        let doc = Parser::default().parse("pass:a[abc<lt{sp}space]{sp}and then ...");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            "abc<lt space and then &#8230;&#8203;"
        );
    }

    #[test]
    fn escape_with_backslash() {
        verifies!(
            r#"
Single occurrences of an attribute reference can be escaped by prefixing the expression with a backslash.
"#
        );

        let doc = Parser::default().parse(r#"This is not a\{sp}space."#);

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "This is not a{sp}space.");
    }
}
