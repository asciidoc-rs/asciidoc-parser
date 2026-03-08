use crate::{
    Parser,
    blocks::{Block, IsBlock},
    tests::prelude::*,
};

track_file!("docs/modules/text/pages/italic.adoc");

non_normative!(
    r#"
= Italic
// content written and moved upstream from Antora by @graphitefriction

Text is often italicized in order to stress a word or phrase, quote a speaker, or introduce a term.
Italic text slants slightly to the right, and depending on the font, may have cursive swashes and flourishes.

The italic presentation of text maps to the formatted text type known as *emphasis* in the AsciiDoc language.

"#
);

#[test]
fn italic_syntax() {
    verifies!(
        r#"
== Italic syntax

You can emphasize (aka italicize) a word or phrase by enclosing it in a single pair of underscores (e.g., `+_word_+`) (constrained).
You can emphasize bounded characters (i.e., characters within a word) by enclosing them in a pair of double underscores (e.g., `+char__act__ers+`) (unconstrained).

.Italic inline formatting
[#ex-italic]
----
An italic _word_, and an italic _phrase of text_.

Italic c__hara__cter__s__ within a word.
----

You don't need to use double underscores when an entire word or phrase marked as italic is directly followed by a common punctuation mark, such as `;`, `"`, and `!`.

The result of <<ex-italic>> is rendered below.

====
An italic _word_, and an italic _phrase of text_.

Italic c__hara__cter__s__ within a word.
====

"#
    );

    let doc = Parser::default().parse(
        "An italic _word_, and an italic _phrase of text_.\n\nItalic c__hara__cter__s__ within a word.",
    );

    let mut blocks = doc.nested_blocks();

    let block1 = blocks.next().unwrap();
    let Block::Simple(sb1) = block1 else {
        panic!("Unexpected block type: {block1:?}");
    };

    assert_eq!(
        sb1.content().rendered(),
        r#"An italic <em>word</em>, and an italic <em>phrase of text</em>."#
    );

    let block2 = blocks.next().unwrap();
    let Block::Simple(sb2) = block2 else {
        panic!("Unexpected block type: {block1:?}");
    };

    assert_eq!(
        sb2.content().rendered(),
        r#"Italic c<em>hara</em>cter<em>s</em> within a word."#
    );

    assert!(blocks.next().is_none());
}

#[test]
fn mixing_italic_with_other_formatting() {
    verifies!(
        r#"
== Mixing italic with other formatting

You can add multiple emphasis styles to italic text as long as the syntax is placed in the correct order.

.Order of inline formatting syntax
[#ex-mix]
----
`*_monospace bold italic phrase_*` & ``**__char__**``acter``**__s__**``
----

xref:monospace.adoc[Monospace syntax] (`++`++`) must be the outermost formatting pair.
xref:bold.adoc[Bold syntax] (`+*+`) must be outside the italics formatting pair.
Italic syntax is always the innermost formatting pair.

The result of <<ex-mix>> is rendered below.

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
