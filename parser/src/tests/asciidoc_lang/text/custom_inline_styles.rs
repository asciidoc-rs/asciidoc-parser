use crate::tests::prelude::*;

track_file!("docs/modules/text/pages/custom-inline-styles.adoc");

non_normative!(
    r#"
= Using Custom Inline Styles

"#
);

mod custom_style_syntax {
    use crate::{
        Parser,
        blocks::{Block, IsBlock},
        tests::prelude::*,
    };

    #[test]
    fn built_in_role() {
        verifies!(
            r#"
== Custom style syntax

You can assign built-in roles (e.g., `big` or `underline`) or custom roles (e.g., `term` or `required`) to any formatted text.
These roles, in turn, can be used to apply styles to the text.
In HTML, this is done by mapping styles to the role in the stylesheet using a CSS class selector.

.Text with built-in role
[#ex-built-in]
----
include::example$text.adoc[tag=css-co]
----
. The first positional attribute is treated as a role.
You can assign it a custom or built-in CSS class.

The results of <<ex-built-in>> are displayed below.

====
include::example$text.adoc[tag=css]
====

"#
        );

        let doc = Parser::default().parse(
            "Do werewolves believe in [.small]#small print#?\n\n[.big]##O##nce upon an infinite loop.",
        );

        let mut blocks = doc.nested_blocks();

        let block1 = blocks.next().unwrap();
        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            sb1.content().rendered(),
            r#"Do werewolves believe in <span class="small">small print</span>?"#
        );

        let block2 = blocks.next().unwrap();
        let Block::Simple(sb2) = block2 else {
            panic!("Unexpected block type: {block2:?}");
        };

        assert_eq!(
            sb2.content().rendered(),
            r#"<span class="big">O</span>nce upon an infinite loop."#
        );

        assert!(blocks.next().is_none());
    }

    #[test]
    fn custom_roles() {
        verifies!(
            r#"
Although xref:text-span-built-in-roles.adoc#built-in[built-in roles] such as `big` and `small` are supported by most AsciiDoc processors, it's really better to define your own semantic role names and map styles to them accordingly.

Here's how you can assign a custom role to text so you can apply your own styles to it.

.Text with custom role
[#ex-custom]
----
include::example$text.adoc[tag=css-custom]
----

When <<ex-custom>> is converted to HTML, the word _asciidoctor_ is enclosed in a `<span>` element and the role `userinput` is used as the element's CSS class.

.HTML output
[,html]
----
include::example$text.adoc[tag=css-custom-html]
----

"#
        );

        let doc =
            Parser::default().parse("Type the word [.userinput]#asciidoctor# into the search bar.");

        let mut blocks = doc.nested_blocks();

        let block1 = blocks.next().unwrap();
        let Block::Simple(sb1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            sb1.content().rendered(),
            "Type the word <span class=\"userinput\">asciidoctor</span> into the search bar."
        );

        assert!(blocks.next().is_none());
    }

    non_normative!(
        r#"
The following example shows how you can assign styles to elements that have this role using a CSS class selector.

[,css]
----
.userinput {
  font-family: monospace;
  font-size: 1.1em;
  line-height: calc(1 / 1.1);
}
----
"#
    );
}
