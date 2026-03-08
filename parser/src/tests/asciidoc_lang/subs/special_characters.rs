use crate::tests::prelude::*;

track_file!("docs/modules/subs/pages/special-characters.adoc");

non_normative!(
    r#"
= Special Character Substitutions
:navtitle: Special Characters
:table-caption: Table
:y: Yes
:n: No

"#
);

mod substitutions {
    use crate::{
        blocks::{Block, IsBlock},
        tests::prelude::*,
    };

    non_normative!(
        r#"
The special characters substitution step searches for three characters (`<`, `>`, `&`) and replaces them with their xref:replacements.adoc#char-ref-sidebar[named character references].

"#
    );

    #[test]
    fn less_than_symbol() {
        verifies!(
            r#"
* The less than symbol, `<`, is replaced with the named character reference `\&lt;`.
"#
        );

        let doc = Parser::default().parse("Replace this < with lt");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(sb1.content().rendered(), "Replace this &lt; with lt");
    }

    #[test]
    fn greater_than_symbol() {
        verifies!(
            r#"
* The greater than symbol, `>`, is replaced with the named character reference `\&gt;`.
"#
        );

        let doc = Parser::default().parse("Replace this > with gt");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(sb1.content().rendered(), "Replace this &gt; with gt");
    }

    #[test]
    fn ampersand() {
        verifies!(
            r#"
* An ampersand, `&`, is replaced with the named character reference `\&amp;`.

"#
        );

        let doc = Parser::default().parse("Replace this & with amp");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "Replace this &amp; with amp");
    }
}

mod default_special_characters_substitution {
    use crate::{
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

        let doc = Parser::default().parse(":lt-attr: <\n\nGoodbye {lt-attr} hello");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(sb1.content().rendered(), "Goodbye &lt; hello");
    }

    #[test]
    fn comments() {
        verifies!(
            r#"
|Comments |{n}

"#
        );

        let doc = Parser::default().parse("////\nabc < def\n////");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::RawDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "abc < def");
    }

    #[test]
    fn examples() {
        verifies!(
            r#"
|Examples |{y}

"#
        );

        let doc = Parser::default().parse("====\nHello & goodbye.\n====");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::CompoundDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        // Dig an extra level deeper to get the simple block that has the content.
        let block1 = block1.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "Hello &amp; goodbye.");
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
|Literal, listings, and source |{y}

"#
        );

        let doc = Parser::default().parse("....\nfoo > bar\n....");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::RawDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "foo &gt; bar");
    }

    #[test]
    fn macros() {
        verifies!(
            r#"
|Macros |{y} +
"#
        );

        let doc = Parser::default()
            .parse("Click image:pause.png[title=Pause & Resume] when you need a break.");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            r#"Click <span class="image"><img src="pause.png" alt="pause" title="Pause &amp; Resume"></span> when you need a break."#
        );
    }

    #[test]
    fn macros_except_pass_macro() {
        verifies!(
            r#"
(except triple plus and inline pass macros)

"#
        );

        let doc = Parser::default().parse("Click +++Pause & Resume+++ when you need a break.");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            r#"Click Pause & Resume when you need a break."#
        );
    }

    #[test]
    fn open() {
        verifies!(
            r#"
|Open |{y}

"#
        );

        let doc = Parser::default().parse("--\nOpened & closed!\n--");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::CompoundDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        // Dig an extra level deeper to get the simple block that has the content.
        let block1 = block1.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "Opened &amp; closed!");
    }

    #[test]
    fn paragraphs() {
        verifies!(
            r#"
|Paragraphs |{y}

"#
        );

        let doc = Parser::default().parse("This is a <paragraph>.");

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

        let doc = Parser::default().parse("++++\nfoo > bar\n++++");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::RawDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "foo > bar");
    }

    #[test]
    fn quotes_and_verses() {
        verifies!(
            r#"
|Quotes and verses |{y}

"#
        );

        let doc = Parser::default().parse("____\nThis & that\n____");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::CompoundDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        // Dig an extra level deeper to get the simple block that has the content.
        let block1 = block1.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "This &amp; that");
    }

    #[test]
    fn sidebars() {
        verifies!(
            r#"
|Sidebars |{y}

"#
        );

        let doc = Parser::default().parse("****\nStuff > nonsense\n****");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::CompoundDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        // Dig an extra level deeper to get the simple block that has the content.
        let block1 = block1.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "Stuff &gt; nonsense");
    }

    #[ignore]
    #[test]
    fn tables() {
        to_do_verifies!(
            r#"
|Tables |{y}

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

        let doc = Parser::default().parse(".Title & such\n****\nStuff > nonsense\n****");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::CompoundDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.title().unwrap(), "Title &amp; such");
    }
}

mod specialchars_substitution_value {
    use crate::{
        blocks::{Block, IsBlock},
        tests::prelude::*,
    };

    non_normative!(
        r#"
== specialchars substitution value

The special characters substitution step can be modified on blocks and inline elements.
"#
    );

    #[test]
    fn for_blocks() {
        verifies!(
            r#"
For blocks, the step's name, `specialchars`, can be assigned to the xref:apply-subs-to-blocks.adoc[subs attribute].
"#
        );

        let doc = Parser::default().parse("[subs=specialchars]\nabc<lt{sp}space");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "abc&lt;lt{sp}space");
    }

    #[test]
    fn for_inline_elements() {
        verifies!(
            r#"
For inline elements, the built-in values `c` or `specialchars` can be applied to xref:apply-subs-to-text.adoc[inline text] to add the special characters substitution step.

"#
        );

        let doc = Parser::default().parse("pass:c[abc<lt{sp}space]{sp}and then ...");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            "abc&lt;lt{sp}space and then &#8230;&#8203;"
        );
    }

    #[test]
    fn manually_escape_api_attributes() {
        verifies!(
            r#"
[NOTE]
====
Special character substitution precedes attribute substitution, so you need to manually escape any attributes containing special characters that you set in the CLI or API.
For example, on the command line, type `+-a toc-title="Sections, Tables \&amp; Figures"+` instead of `-a toc-title="Sections, Tables & Figures"`.
====
"#
        );

        let doc = Parser::default()
            .with_intrinsic_attribute(
                "toc-title",
                "Sections, Tables & Figures",
                ModificationContext::Anywhere,
            )
            .parse("The value of toc-title is {toc-title}.");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            "The value of toc-title is Sections, Tables & Figures."
        );
    }
}
