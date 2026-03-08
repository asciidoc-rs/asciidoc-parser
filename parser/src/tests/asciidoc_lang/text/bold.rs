use crate::{
    Parser,
    blocks::{Block, IsBlock},
    tests::prelude::*,
};

track_file!("docs/modules/text/pages/bold.adoc");

non_normative!(
    r#"
= Bold
// content written and moved upstream from Antora by @graphitefriction

Text that is marked up as bold will stand out against the regular, surrounding text due to the application of a thicker and/or darker font.
Bold is useful when the text needs to catch the attention of a site visitor quickly scanning a page.

The bold presentation of text maps to the formatted text type known as *strong* in the AsciiDoc language.

"#
);

#[test]
fn bold_syntax() {
    verifies!(
        r#"
== Bold syntax

You can mark a word or phrase as bold by enclosing it in a single pair of asterisks (e.g., `+*word*+`) (constrained).
You can mark bounded characters (i.e., characters within a word) as bold by enclosing them in a pair of double asterisks (e.g., `+char**act**ers+`) (unconstrained).

.Bold inline formatting
[#ex-bold]
----
A bold *word*, and a bold *phrase of text*.

Bold c**hara**cter**s** within a word.
----

You don't need to use double asterisks when an entire word or phrase marked as bold is directly followed by a common punctuation mark, such as `;`, `"`, and `!`.

The results of <<ex-bold>> are displayed below.

====
A bold *word*, and a bold *phrase of text*.

Bold c**hara**cter**s** within a word.
====

"#
    );

    let doc = Parser::default().parse(
        "A bold *word*, and a bold *phrase of text*.\n\nBold c**hara**cter**s** within a word.",
    );

    let mut blocks = doc.nested_blocks();

    let block1 = blocks.next().unwrap();
    let Block::Simple(sb1) = block1 else {
        panic!("Unexpected block type: {block1:?}");
    };

    assert_eq!(
        sb1.content().rendered(),
        r#"A bold <strong>word</strong>, and a bold <strong>phrase of text</strong>."#
    );

    let block2 = blocks.next().unwrap();
    let Block::Simple(sb2) = block2 else {
        panic!("Unexpected block type: {block1:?}");
    };

    assert_eq!(
        sb2.content().rendered(),
        r#"Bold c<strong>hara</strong>cter<strong>s</strong> within a word."#
    );

    assert!(blocks.next().is_none());
}

#[test]
fn mixing_bold_with_other_formatting() {
    verifies!(
        r#"
== Mixing bold with other formatting

You can add multiple emphasis styles to bold text as long as the syntax is placed in the correct order.

.Order of inline formatting syntax
[#ex-mix]
----
`*_monospace bold italic phrase_*` & ``**__char__**``acter``**__s__**``
----

xref:monospace.adoc[Monospace syntax] (`++`++`) must be the outermost formatting pair (i.e., outside the bold formatting pair).
xref:italic.adoc[Italic syntax] (`+_+`) is always the innermost formatting pair.

The results of <<ex-mix>> are displayed below.

====
`*_monospace bold italic phrase_*` & ``**__char__**``acter``**__s__**``
====
"#
    );

    let doc = Parser::default()
        .parse("`*_monospace bold italic phrase_*` & ``**__char__**``acter``**__s__**``");

    let mut blocks = doc.nested_blocks();

    let block1 = blocks.next().unwrap();
    let Block::Simple(sb1) = block1 else {
        panic!("Unexpected block type: {block1:?}");
    };

    assert_eq!(
        sb1.content().rendered(),
        r#"<code><strong><em>monospace bold italic phrase</em></strong></code> &amp; <code><strong><em>char</em></strong></code>acter<code><strong><em>s</em></strong></code>"#
    );

    assert!(blocks.next().is_none());
}
