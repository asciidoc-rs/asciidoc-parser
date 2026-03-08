use crate::tests::prelude::*;

track_file!("docs/modules/subs/pages/replacements.adoc");

non_normative!(
    r#"
= Character Replacement Substitutions
:navtitle: Character Replacements
:table-caption: Table
:y: Yes
//icon:check[role="green"]
:n: No
//icon:times[role="red"]
:url-html-ref: https://en.wikipedia.org/wiki/List_of_XML_and_HTML_character_entity_references
:url-unicode: https://en.wikipedia.org/wiki/List_of_Unicode_characters

"#
);

mod replacements {
    use crate::{
        Parser,
        blocks::{Block, IsBlock},
        tests::prelude::*,
    };

    non_normative!(
        r#"
The character replacement substitution step processes textual characters such as marks, arrows and dashes and replaces them with the decimal format of their Unicode code point, i.e., their <<char-ref-sidebar,numeric character reference>>.
The replacements step depends on the substitutions completed by the xref:special-characters.adoc[special characters step].

"#
    );

    verifies!(
        r#"
// Table of Textual symbol replacements is inserted below
include::partial$subs-symbol-repl.adoc[]

"#
    );

    #[test]
    fn copyright() {
        let doc = Parser::default().parse("(C)");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(sb1.content().rendered(), "&#169;");
    }

    #[test]
    fn registered() {
        let doc = Parser::default().parse("(R)");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(sb1.content().rendered(), "&#174;");
    }

    #[test]
    fn trademark() {
        let doc = Parser::default().parse("(TM)");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(sb1.content().rendered(), "&#8482;");
    }

    #[test]
    fn em_dash() {
        let doc = Parser::default().parse("abc--def");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(sb1.content().rendered(), "abc&#8212;&#8203;def");
    }

    #[test]
    fn em_dash_surrounded_by_spaces() {
        let doc = Parser::default().parse("abc -- def");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(sb1.content().rendered(), "abc&#8201;&#8212;&#8201;def");
    }

    #[test]
    fn ellipsis() {
        let doc = Parser::default().parse("...");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(sb1.content().rendered(), "&#8230;&#8203;");
    }

    #[test]
    fn single_right_arrow() {
        let doc = Parser::default().parse("->");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(sb1.content().rendered(), "&#8594;");
    }

    #[test]
    fn double_right_arrow() {
        let doc = Parser::default().parse("=>");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(sb1.content().rendered(), "&#8658;");
    }

    #[test]
    fn single_left_arrow() {
        let doc = Parser::default().parse("<-");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(sb1.content().rendered(), "&#8592;");
    }

    #[test]
    fn double_left_arrow() {
        let doc = Parser::default().parse("<=");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(sb1.content().rendered(), "&#8656;");
    }

    #[test]
    fn typographic_apostrophe() {
        let doc = Parser::default().parse("Sam's");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(sb1.content().rendered(), "Sam&#8217;s");
    }

    #[test]
    fn preserve_character_reference() {
        verifies!(
            r#"
For example, to produce the `&#167;` symbol you could write `\&sect;`, `\&#x00A7;`, or `\&#167;`.
When the document is processed, `replacements` will preserve the section symbol reference so the reference will appear as &#167; when rendered.
"#
        );

        let doc = Parser::default().parse("In &sect; 8.1.5, we say ...");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            sb1.content().rendered(),
            "In &sect; 8.1.5, we say &#8230;&#8203;"
        );
    }

    #[test]
    fn preserve_numeric_character_reference() {
        verifies!(
            r#"
In turn, `\&#167;` in the AsciiDoc source will display as &#167; in the rendered document.

"#
        );

        let doc = Parser::default().parse("In &#167; 8.1.5, we say ...");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            sb1.content().rendered(),
            "In &#167; 8.1.5, we say &#8230;&#8203;"
        );
    }

    non_normative!(
        r#"
An AsciiDoc processor allows you to use any of the named character references (aka named entities) defined in HTML (e.g., \&euro; resolves to &#8364;).
However, using named character references can cause problems when generating non-HTML output such as PDF because the lookup table needed to resolve these names may not be defined.
The recommendation is avoid using named character references, with the exception of the well-known ones defined in XML (i.e., lt, gt, amp, quot, apos).
Instead, use numeric character references (e.g., \&#8364;).

[#char-ref-sidebar]
.Anatomy of a character reference
****
A character reference is a standard sequence of characters that is substituted for a single character by an AsciiDoc processor.
There are two types of character references: named character references and numeric character references.

A named character reference (often called a _character entity reference_) is a short name that refers to a character (i.e., glyph).
To make the reference, the name must be prefixed with an ampersand (`&`) and end with a semicolon (`;`).

For example:

* `\&dagger;` displays as &#8224;
* `\&euro;` displays as &#8364;
* `\&loz;` displays as &#9674;

Numeric character references are the decimal or hexadecimal Universal Character Set/Unicode code points which refer to a character.

* The decimal code point references are prefixed with an ampersand (`&`), followed by a hash (`&#35;`), and end with a semicolon (`;`).
* Hexadecimal code point references are prefixed with an ampersand (`&`), followed by a hash (`&#35;`), followed by a lowercase `x`, and end with a semicolon (`;`).

For example:

* `\&#x2020;` or `\&#8224;` displays as &#8224;
* `\&#x20AC;` or `\&#8364;` displays as &#8364;
* `\&#x25CA;` or `\&#9674;` displays as &#x25CA;

Developers may be more familiar with using *Unicode escape sequences* to perform text substitutions.
For example, to produce an `&#64;` sign using a Unicode escape sequence, you would prefix the hexadecimal Unicode code point with a backslash (`\`) and an uppercase or lowercase `u`, i.e. `u0040`.
However, the AsciiDoc syntax doesn't recognize Unicode escape sequences at this time.
****

TIP: AsciiDoc also provides built-in attributes for representing some common symbols.
These attributes and their corresponding output are listed in xref:attributes:character-replacement-ref.adoc[].

            "#
    );
}

mod blocks_and_inline_elements_subject_to_the_replacements_substitution {
    use crate::{
        Parser,
        blocks::{Block, IsBlock},
        tests::prelude::*,
    };

    non_normative!(
        r#"
.Blocks and inline elements subject to the replacements substitution
[#table-replace%autowidth,cols="~,^~"]
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

        let doc = Parser::default().parse(":copy: (C)\n\npass:a[ab {copy} def]");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(sb1.content().rendered(), "ab (C) def");
    }

    #[test]
    fn comments() {
        verifies!(
            r#"
|Comments |{n}

"#
        );

        let doc = Parser::default().parse("////\nab (C) def\n////");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::RawDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "ab (C) def");
    }

    #[test]
    fn examples() {
        verifies!(
            r#"
|Examples |{y}

"#
        );

        let doc = Parser::default().parse("====\nHello (C) goodbye.\n====");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::CompoundDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        // Dig an extra level deeper to get the simple block that has the content.
        let block1 = block1.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "Hello &#169; goodbye.");
    }

    #[test]
    fn headers() {
        verifies!(
            r#"
|Headers |{n}

"#
        );

        let doc = Parser::default().parse("= Title (C) So On");

        let title = doc.header().title().unwrap();
        assert_eq!(title, "Title (C) So On");
    }

    #[test]
    fn literal_listings_and_source() {
        verifies!(
            r#"
|Literal, listings, and source |{n}

"#
        );

        let doc = Parser::default().parse("....\nfoo (C) bar\n....");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::RawDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "foo (C) bar");
    }

    #[test]
    fn macros() {
        verifies!(
            r#"
|Macros |{y} +
"#
        );

        let doc = Parser::default()
            .parse("Click image:pause.png[title=Pause (C) Resume] when you need a break.");

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
    fn macros_except_pass_macro() {
        verifies!(
            r#"
(except triple plus and inline pass macros)

"#
        );

        let doc = Parser::default().parse("Click +++Pause (C) Resume+++ when you need a break.");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            r#"Click Pause (C) Resume when you need a break."#
        );
    }

    #[test]
    fn open() {
        verifies!(
            r#"
|Open |{y}

"#
        );

        let doc = Parser::default().parse("--\nOpened (C) closed!\n--");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::CompoundDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        // Dig an extra level deeper to get the simple block that has the content.
        let block1 = block1.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "Opened &#169; closed!");
    }

    #[test]
    fn paragraphs() {
        verifies!(
            r#"
|Paragraphs |{y}

"#
        );

        let doc = Parser::default().parse("This is a (C) paragraph.");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            r#"This is a &#169; paragraph."#
        );
    }

    #[test]
    fn passthrough_blocks() {
        verifies!(
            r#"
|Passthrough blocks |{n}

"#
        );

        let doc = Parser::default().parse("++++\nfoo (C) bar\n++++");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::RawDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "foo (C) bar");
    }

    #[test]
    fn quotes_and_verses() {
        verifies!(
            r#"
|Quotes and verses |{y}

"#
        );

        let doc = Parser::default().parse("____\nThis (C) that\n____");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::CompoundDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        // Dig an extra level deeper to get the simple block that has the content.
        let block1 = block1.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "This &#169; that");
    }

    #[test]
    fn sidebars() {
        verifies!(
            r#"
|Sidebars |{y}

"#
        );

        let doc = Parser::default().parse("****\nStuff (C) nonsense\n****");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::CompoundDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        // Dig an extra level deeper to get the simple block that has the content.
        let block1 = block1.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "Stuff &#169; nonsense");
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

        let doc = Parser::default().parse(".Title (C) such\n****\nStuff > nonsense\n****");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::CompoundDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.title().unwrap(), "Title &#169; such");
    }
}

mod replacements_substitution_value {
    use crate::{
        Parser,
        blocks::{Block, IsBlock},
        tests::prelude::*,
    };

    non_normative!(
        r#"
== replacements substitution value

The replacements substitution step can be modified on blocks and inline elements.
"#
    );

    #[test]
    fn for_blocks() {
        verifies!(
            r#"
For blocks, the step's name, `replacements`, can be assigned to the xref:apply-subs-to-blocks.adoc[subs attribute].
"#
        );

        let doc = Parser::default().parse("[subs=replacements]\nabc<lt (C) *bold*");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(block1.content().rendered(), "abc<lt &#169; *bold*");
    }

    #[test]
    fn for_inline_elements() {
        verifies!(
            r#"
For inline elements, the built-in values `r` or `replacements` can be applied to xref:apply-subs-to-text.adoc[inline text] to add the replacements substitution step.

"#
        );

        let doc = Parser::default().parse("pass:r[abc<lt (C) *bold*] and then ...");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            "abc<lt &#169; *bold* and then &#8230;&#8203;"
        );
    }

    #[test]
    fn warning_dependency() {
        verifies!(
            r#"
WARNING: The replacements step depends on the substitutions completed by the xref:special-characters.adoc[special characters step].
This is important to keep in mind when applying the `replacements` value to blocks and inline elements.
"#
        );

        let doc = Parser::default().parse("pass:r[left-arrow <- not here] but <- there ...");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            "left-arrow <- not here but &#8592; there &#8230;&#8203;"
        );
    }
}
