use crate::{
    Parser,
    blocks::{Block, IsBlock},
    tests::prelude::*,
};

track_file!("docs/modules/text/pages/monospace.adoc");

non_normative!(
    r#"
= Monospace

In AsciiDoc, a span of text enclosed in a single pair of backticks (`{backtick}`) is displayed using a fixed-width (i.e., monospaced) font.
Monospace text formatting is typically used to represent text shown in computer terminals or code editors (often referred to as a codespan).

The monospace presentation of text maps to the formatted text type known as *monospaced* in the AsciiDoc language.

"#
);

#[test]
fn constrained() {
    verifies!(
        r#"
== Constrained

Here's an example:

.Constrained monospace syntax
[#ex-constrain]
----
include::example$text.adoc[tag=mono]
----

The result of <<ex-constrain>> is rendered below.

====
include::example$text.adoc[tag=mono]
====

"#
    );

    let doc = Parser::default().parse(
        "\"`Wait!`\" Indigo plucked a small vial from her desk's top drawer and held it toward us.\nThe vial's label read: `E=mc^2^`; the `E` represents _energy_,\nbut also pure _genius!_",
    );

    let mut blocks = doc.nested_blocks();

    let block1 = blocks.next().unwrap();
    let Block::Simple(sb1) = block1 else {
        panic!("Unexpected block type: {block1:?}");
    };

    assert_eq!(
        sb1.content().rendered(),
        "&#8220;Wait!&#8221; Indigo plucked a small vial from her desk&#8217;s top drawer and held it toward us.\nThe vial&#8217;s label read: <code>E=mc<sup>2</sup></code>; the <code>E</code> represents <em>energy</em>,\nbut also pure <em>genius!</em>"
    );

    assert!(blocks.next().is_none());
}

#[test]
fn unconstrained() {
    verifies!(
        r#"
== Unconstrained

As with other types of text formatting, if the text is bounded by word characters on either side, it must be enclosed in a double pair of backtick characters (`{backtick}{backtick}`) in order for the formatting to be applied.

Here's an example:

----
The command will re``link`` all packages.
----

"#
    );

    let doc = Parser::default().parse("The command will re``link`` all packages.");

    let mut blocks = doc.nested_blocks();

    let block1 = blocks.next().unwrap();
    let Block::Simple(sb1) = block1 else {
        panic!("Unexpected block type: {block1:?}");
    };

    assert_eq!(
        sb1.content().rendered(),
        "The command will re<code>link</code> all packages."
    );

    assert!(blocks.next().is_none());
}

#[test]
fn mixed_formatting() {
    verifies!(
        r#"
== Mixed Formatting

Monospaced text can also be formatted in bold or italic or both, as long as the markup pairs are entered in the right order.
The monospace markup must be the outermost formatting mark, then the bold marks, then the italic marks.

.Order of inline formatting syntax
[#ex-mix]
----
`*_monospaced bold italic_*`
----

The result of <<ex-mix>> is rendered below.

====
`*_monospaced bold italic_*`
====

"#
    );

    let doc = Parser::default().parse("`*_monospaced bold italic_*`");

    let mut blocks = doc.nested_blocks();

    let block1 = blocks.next().unwrap();
    let Block::Simple(sb1) = block1 else {
        panic!("Unexpected block type: {block1:?}");
    };

    assert_eq!(
        sb1.content().rendered(),
        "<code><strong><em>monospaced bold italic</em></strong></code>"
    );

    assert!(blocks.next().is_none());
}

non_normative!(
    r#"
== Literal Monospace

To learn how to make monospace text that's not otherwise formatted, see xref:literal-monospace.adoc[].
"#
);
