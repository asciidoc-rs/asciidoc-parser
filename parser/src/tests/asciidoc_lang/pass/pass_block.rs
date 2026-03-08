use crate::{
    Parser,
    blocks::{Block, IsBlock},
    tests::prelude::*,
};
track_file!("docs/modules/pass/pages/pass-block.adoc");

non_normative!(
    r#"
= Passthrough Blocks

The `pass` style and delimited passthrough block exclude the block's content from all substitutions unless the `subs` attribute is set.

"#
);

#[test]
fn pass_style_syntax() {
    verifies!(
        r#"
== Pass style syntax

The `pass` style can also be set on a paragraph or an open block.

[source]
----
include::example$pass.adoc[tag=pass-style]
----

"#
    );

    let doc = Parser::default().parse("[pass]\n<del>strike this</del> is marked as deleted.");

    let block1 = doc.nested_blocks().next().unwrap();

    let Block::Simple(sb1) = block1 else {
        panic!("Unexpected block type: {block1:?}");
    };

    assert_eq!(
        sb1.content().rendered(),
        "<del>strike this</del> is marked as deleted."
    );
}

#[test]
fn delimited_passthrough_block_syntax() {
    verifies!(
        r#"
== Delimited passthrough block syntax

A passthrough block is delimited by four plus signs (`pass:[++++]`).

[source]
----
include::example$pass.adoc[tag=bl]
----

(Keep in mind that AsciiDoc has a video macro, so this example is merely for demonstration.
However, a passthrough could come in handy if you need to output more sophisticated markup than what the built-in HTML converter produces).

"#
    );

    let doc = Parser::default().parse("++++\n<video poster=\"images/movie-reel.png\">\n  <source src=\"videos/writing-zen.webm\" type=\"video/webm\">\n</video>\n++++
");

    let block1 = doc.nested_blocks().next().unwrap();

    let Block::RawDelimited(rdb1) = block1 else {
        panic!("Unexpected block type: {block1:?}");
    };

    assert_eq!(
        rdb1.content().rendered(),
        "<video poster=\"images/movie-reel.png\">\n  <source src=\"videos/writing-zen.webm\" type=\"video/webm\">\n</video>"
    );
}

#[test]
fn control_substitutions_on_a_passthrough_block() {
    verifies!(
        r#"
== Control substitutions on a passthrough block

You can use the xref:subs:apply-subs-to-blocks.adoc[subs attribute to specify a comma-separated list of substitutions].
These substitutions will be applied to the content prior to it being reintroduced to the output document.

[source]
----
include::example$pass.adoc[tag=subs-bl]
----

"#
    );

    let doc = Parser::default().parse(
        ":name: This Is My Name\n\n[subs=attributes]\n++++\n{name}\nimage:tiger.png[]\n++++",
    );

    let block1 = doc.nested_blocks().next().unwrap();

    let Block::RawDelimited(sb1) = block1 else {
        panic!("Unexpected block type: {block1:?}");
    };

    assert_eq!(
        sb1.content().rendered(),
        "This Is My Name\nimage:tiger.png[]"
    );
}

#[test]
fn no_paragraph_wrapper() {
    verifies!(
        r#"
The content of the pass block does not get wrapped in a paragraph.
Therefore, you can use the `pass` style in combination with the `normal` substitution category to output content without generating a paragraph.

[source]
----
include::example$pass.adoc[tag=no-para]
----

WARNING: Using passthroughs to pass content (without substitutions) can couple your content to a specific output format, such as HTML.
In these cases, you should use conditional preprocessor directives to route passthrough content for different output formats based on the current backend.
"#
    );

    let doc = Parser::default()
        .parse("[subs=normal]\n++++\nNormal content which is not enclosed in a paragraph.\n++++");

    let block1 = doc.nested_blocks().next().unwrap();

    let Block::RawDelimited(rdb1) = block1 else {
        panic!("Unexpected block type: {block1:?}");
    };

    assert_eq!(
        rdb1.content().rendered(),
        "Normal content which is not enclosed in a paragraph."
    );
}
