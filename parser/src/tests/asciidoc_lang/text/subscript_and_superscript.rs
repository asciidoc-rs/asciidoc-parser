use crate::tests::prelude::*;

track_file!("docs/modules/text/pages/subscript-and-superscript.adoc");

non_normative!(
    r#"
= Subscript and Superscript

Subscript and superscript text is common in mathematical expressions and chemical formulas.
When rendered, the size of subscripted and superscripted text is reduced.
Subscripted text is placed at the baseline and superscripted text above the baseline.
The size and precise placement of the text depends on the font and other stylesheet parameters applied to the converted document.

"#
);

mod subscript_and_superscript_syntax {
    use crate::{
        Parser,
        blocks::{Block, IsBlock},
        tests::prelude::*,
    };

    #[test]
    fn basic_syntax() {
        verifies!(
            r#"
== Subscript and superscript syntax

Text is rendered as subscript (below the baseline) when you enclose it in a pair of tildes (`{tilde}`).
Text is rendered as superscript (above the baseline) when you enclose it in a pair of carets (`{caret}`)

Superscript and subscript have unique boundary constraints in AsciiDoc that are neither constrained nor unconstrained.
Rather, they are unconstrained with the key restriction that the text must be continuous.
(It may not contain spaces).
This restriction is in place to avoid unexpected behavior where `{tilde}` and `{caret}` have meaning in other contexts.
It's a tradeoff to have a more predictable syntax.

Subscript:: One tilde (`{tilde}`) on either side of a continuous run of text makes it subscript.
Superscript:: One caret (`{caret}`) on either side of a continuous run of text makes it superscript.

.Subscript and superscript syntax
[#ex-basic]
----
include::example$text.adoc[tag=sub-sup]
----

The result of <<ex-basic>> is rendered below.

====
include::example$text.adoc[tag=sub-sup]
====

"#
        );

        let doc = Parser::default().parse(
            r#""`Well the H~2~O formula written on their whiteboard could be part
of a shopping list, but I don't think the local bodega sells
E=mc^2^,`" Lazarus replied."#,
        );

        let mut blocks = doc.nested_blocks();

        let block1 = blocks.next().unwrap();
        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            sb1.content().rendered(),
            "&#8220;Well the H<sub>2</sub>O formula written on their whiteboard could be part\nof a shopping list, but I don&#8217;t think the local bodega sells\nE=mc<sup>2</sup>,&#8221; Lazarus replied."
        );

        assert!(blocks.next().is_none());
    }

    #[test]
    fn example_with_space() {
        verifies!(
            r#"
If you need to include spaces in the superscript or subscript text, you must use the attribute reference `\{sp}` in place of the space character.

.Superscript syntax that contains spaces
[#ex-with-spaces]
----
include::example$text.adoc[tag=sup-with-spaces]
----

"#
        );

        let doc = Parser::default()
            .parse("The deepest body of water is Deep Creek Lake.^[citation{sp}needed]^");

        let mut blocks = doc.nested_blocks();

        let block1 = blocks.next().unwrap();
        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            sb1.content().rendered(),
            "The deepest body of water is Deep Creek Lake.<sup>[citation needed]</sup>"
        );

        assert!(blocks.next().is_none());
    }

    non_normative!(
        r#"
To write text that makes use of more complex variations and combinations of superscript and subscript, such as in equations and formulas, you're encourages to use the xref:stem:index.adoc[stem block or inline macro].
"#
    );
}
