use crate::{
    Parser,
    blocks::{Block, IsBlock},
    tests::prelude::*,
};

track_file!("docs/modules/text/pages/highlight.adoc");

non_normative!(
    r#"
= Highlight
//New page, content from text-css.adoc

"#
);

#[test]
fn highlight_syntax() {
    verifies!(
        r#"
== Highlight syntax

When text is enclosed in a pair of single or double hash symbols (`#`), and no role is assigned to it, the text will be rendered as highlighted (aka marked) text for notation purposes.

.Highlighted style syntax
[#ex-highlight]
----
include::example$text.adoc[tag=highlight]
----

When <<ex-highlight>> is converted to HTML, it translates into the following output.

.Highlighted text HTML output
[,html]
----
include::example$text.adoc[tag=highlight-html]
----

The result of <<ex-highlight>> is rendered below.

====
include::example$text.adoc[tag=highlight]
====
"#
    );

    let doc = Parser::default().parse("Mark my words, #automation is essential#.");

    let mut blocks = doc.nested_blocks();

    let block1 = blocks.next().unwrap();
    let Block::Simple(sb1) = block1 else {
        panic!("Unexpected block type: {block1:?}");
    };

    assert_eq!(
        sb1.content().rendered(),
        "Mark my words, <mark>automation is essential</mark>."
    );

    assert!(blocks.next().is_none());
}
