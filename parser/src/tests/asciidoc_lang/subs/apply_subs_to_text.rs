use crate::tests::prelude::*;

track_file!("docs/modules/subs/pages/apply-subs-to-text.adoc");

non_normative!(
    r#"
= Customize the Substitutions Applied to Text

"#
);

mod shorthand_values {
    use crate::{
        blocks::{Block, IsBlock},
        tests::prelude::*,
    };

    non_normative!(
        r#"
The inline pass macro (`++pass:[]++`) accepts the shorthand values in addition to the longhand values for specifying substitution types.

"#
    );

    #[test]
    fn specialchars() {
        verifies!(
            r#"
* `c` or `specialchars`
"#
        );

        let doc = Parser::default().parse(
            "pass:c[This & _that_ and icon:github[\\] +\nanother line with a{sp}space there ...]",
        );

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            "This &amp; _that_ and icon:github[] +\nanother line with a{sp}space there ..."
        );
    }

    #[test]
    fn quotes() {
        verifies!(
            r#"
* `q` or `quotes`
"#
        );

        let doc = Parser::default().parse(
            "pass:q[This & _that_ and icon:github[\\] +\nanother line with a{sp}space there ...]",
        );

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            "This & <em>that</em> and icon:github[] +\nanother line with a{sp}space there ..."
        );
    }

    #[test]
    fn attributes() {
        verifies!(
            r#"
* `a` or `attributes`
"#
        );

        let doc = Parser::default().parse(
            "pass:a[This & _that_ and icon:github[\\] +\nanother line with a{sp}space there ...]",
        );

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            "This & _that_ and icon:github[] +\nanother line with a space there ..."
        );
    }

    #[test]
    fn replacements() {
        verifies!(
            r#"
* `r` or `replacements`
"#
        );

        let doc = Parser::default().parse(
            "pass:r[This &#169; _that_ and icon:github[\\] +\nanother line with a{sp}space there ...]",
        );

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            "This &#169; _that_ and icon:github[] +\nanother line with a{sp}space there &#8230;&#8203;"
        );
    }

    #[test]
    fn macros() {
        verifies!(
            r#"
* `m` or `macros`
"#
        );

        let doc = Parser::default().parse(
            "pass:m[This &#169; _that_ and icon:github[\\] +\nanother line with a{sp}space there ...]",
        );

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            "This &#169; _that_ and <span class=\"icon\">[github&#93;</span> +\nanother line with a{sp}space there ..."
        );
    }

    #[test]
    fn post_replacements() {
        verifies!(
            r#"
* `p` or `post_replacements`

"#
        );

        let doc = Parser::default().parse(
            "pass:p[This &#169; _that_ and icon:github[\\] +\nanother line with a{sp}space there ...]",
        );

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            "This &#169; _that_ and icon:github[]<br>\nanother line with a{sp}space there ..."
        );
    }
}

mod apply_substitutions_to_inline_text {
    use crate::{
        blocks::{Block, IsBlock},
        tests::prelude::*,
    };

    non_normative!(
        r#"
== Apply substitutions to inline text

Custom substitutions can also be applied to inline text with the xref:pass:pass-macro.adoc[pass macro].
"#
    );

    #[test]
    fn mark_span_deleted() {
        verifies!(
            r#"
For instance, let's assume you need to mark a span of text as deleted using the HTML element `<del>` in your AsciiDoc document.
You'd do this with the inline pass macro.

.Inline pass macro syntax
[source#ex-pass]
----
include::pass:example$pass.adoc[tag=in-macro]
----

The result of <<ex-pass>> is rendered below.

====
include::pass:example$pass.adoc[tag=in-macro]
====

"#
        );

        let doc =
            Parser::default().parse("The text pass:[<del>strike this</del>] is marked as deleted.");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            "The text <del>strike this</del> is marked as deleted."
        );
    }

    #[test]
    fn mark_span_deleted_with_formatting() {
        verifies!(
            r#"
However, you also need to bold the text and want to use the AsciiDoc markup for that formatting.
In this case, you'd assign the `quotes` substitution to the inline pass macro.

.Assign quotes to inline pass macro
[source#ex-sub-quotes]
----
include::pass:example$pass.adoc[tag=s-macro]
----

The result of <<ex-sub-quotes>> is rendered below.

====
include::pass:example$pass.adoc[tag=s-macro]
====

"#
        );

        let doc =
            Parser::default().parse(r#"The text pass:q[<del>strike *this*</del>] is marked as deleted, inside of which the word "`me`" is bold."#);

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            "The text <del>strike <strong>this</strong></del> is marked as deleted, inside of which the word &#8220;me&#8221; is bold."
        );
    }

    #[test]
    fn custom_substitutions_to_inline_text() {
        verifies!(
            r#"
You can also assign custom substitutions to inline text that's in a block.
In the listing block below, we want to process the inline formatting on the second line.

.Listing block with inline formatting
[source#ex-listing]
....
include::pass:example$pass.adoc[tag=sub-in]
....
<.> `macros` is assigned to `subs`, which allows the `pass` macro within the block to be processed.
<.> The `pass` macro is assigned the `quotes` value.
Text within the square brackets will be formatted.

The result of <<ex-listing>> is rendered below.

====
include::pass:example$pass.adoc[tag=sub-out]
====
"#
        );

        let doc =
            Parser::default().parse("[subs=+macros]\n----\nI better not contain *bold* or _italic_ text.\npass:quotes[But I should contain *bold* text.]\n----");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::RawDelimited(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            "I better not contain *bold* or _italic_ text.\nBut I should contain <strong>bold</strong> text."
        );
    }
}
