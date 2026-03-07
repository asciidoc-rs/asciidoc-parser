use crate::tests::prelude::*;

track_file!("docs/modules/subs/pages/prevent.adoc");

non_normative!(
    r#"
= Escape and Prevent Substitutions

The AsciiDoc syntax offers several approaches for preventing substitutions from being applied.

"#
);

mod escape_with_backslashes {
    use pretty_assertions_sorted::assert_eq;

    use crate::{
        Parser,
        blocks::{Block, IsBlock},
        tests::prelude::*,
    };

    non_normative!(
        r#"
== Escape with backslashes

"#
    );

    #[ignore]
    #[test]
    fn punctuation() {
        // TO DO (https://github.com/asciidoc-rs/asciidoc-parser/issues/316):
        // Some of the macros described here are not yet implemented, so this test can't
        // work properly.
        to_do_verifies!(
            r#"
To prevent a punctuation character from being interpreted as an attribute reference or formatting syntax (e.g., `+_+`, `+^+`) in normal content, prepend the character with a backslash (`\`).

.Prevent unintended substitutions with a backslash in normal content
[source#ex-backslash]
----
include::example$subs.adoc[tag=backslash]
----

The backslash can also prevent character replacements, macros, and attribute replacements.
The results of <<ex-backslash>> are below.

====
include::example$subs.adoc[tag=backslash]
====

Notice that the backslash is removed so it doesn't display in your output.

"#
        );

        let doc = Parser::default().parse(
            r###"
In /items/\{id}, the id attribute isn't replaced.
The curly braces around it are preserved.

\*Stars* isn't displayed as bold text.
The asterisks around it are preserved.

\&sect; appears as an entity reference.
It's not converted into the section symbol (&#167;).

\=> The backslash prevents the equals sign followed by a greater
than sign from combining to form a double arrow character (=>).

\[[Word]] is not interpreted as an anchor.
The double brackets around it are preserved.

[\[[Word]]] is not interpreted as a bibliography anchor.
The triple brackets around it are preserved.

\((DD AND CC) OR (DD AND EE)) is not interpreted as a flow index term.
The double brackets around it are preserved.

The URL \https://example.org isn't converted into an active link.
"###,
        );

        let result = doc
            .nested_blocks()
            .map(|block| {
                let Block::Simple(block) = block else {
                    panic!("Unexpected block type: {block:?}");
                };

                format!("{}\n\n", block.content().rendered())
            })
            .collect::<String>();

        assert_eq!(
            result,
            "In /items/{id}, the id attribute isn&#8217;t replaced.\nThe curly braces around it are preserved.\n\n*Stars* isn&#8217;t displayed as bold text.\nThe asterisks around it are preserved.\n\n&sect; appears as an entity reference.\nIt&#8217;s not converted into the section symbol (&#167;).\n\n=&gt; The backslash prevents the equals sign followed by a greater than sign from combining to form a double arrow character (&#8658;).\n\n[[Word]] is not interpreted as an anchor.\nThe double brackets around it are preserved.\n\n[[[Word]]] is not interpreted as a bibliography anchor.\nThe triple brackets around it are preserved.\n\n((DD AND CC) OR (DD AND EE)) is not interpreted as a flow index term.\nThe double brackets around it are preserved.\n\nThe URL https://example.org isn&#8217;t converted into an active link.\n\n"
        );
    }

    #[test]
    fn double_slash() {
        verifies!(
            r#"
To prevent two adjacent characters (e.g., +__+, pass:[##]), from being interpreted as AsciiDoc syntax you need to precede it with two backslashes (`+\\+`).

.Prevent unintended substitutions with two backslashes in normal content
[source#ex-double-slash]
----
include::example$subs.adoc[tag=double-slash]
----

The results of <<ex-double-slash>> are below.

====
include::example$subs.adoc[tag=double-slash]
====

"#
        );

        let doc = Parser::default().parse(
            "The text \\\\__func__ will appear with two underscores\nin front of it and after it.\nIt won't be italicized.",
        );

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            "The text __func__ will appear with two underscores\nin front of it and after it.\nIt won&#8217;t be italicized."
        );
    }
}

mod passthrough {
    use pretty_assertions_sorted::assert_eq;

    use crate::{
        Parser,
        blocks::{Block, IsBlock},
        tests::prelude::*,
    };

    non_normative!(
        r#"
== Passthroughs

A passthrough is the primary mechanism by which to escape content in AsciiDoc.
They're far more comprehensive and consistent than using a backslash.
As the name implies, a passthrough passes content directly through to the output document without applying any substitutions.

You can control and prevent substitutions in inline text with the xref:pass:pass-macro.adoc[inline passthrough macros] and for entire blocks of content with the xref:pass:pass-block.adoc[block passthrough].

The inline `{plus}` passthrough takes precedence over all other inline formatting.
Therefore, if you need to output a literal plus when it would otherwise match a passthrough, you have two options.

"#
    );

    #[test]
    fn plus_attribute() {
        verifies!(
            r#"
First, you can escape it using the `\{plus}` attribute reference:

[source]
----
`{plus}` and `{plus}`
----

"#
        );

        let doc = Parser::default().parse("`{plus}` and `{plus}`");

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            "<code>&#43;</code> and <code>&#43;</code>"
        );
    }

    #[test]
    fn backslash() {
        verifies!(
            r#"
Alternately, you can escape the pair using a backslash.

[source]
----
`\+` and `+`
----

The backslash is only required before the pair, not before each occurance of the plus.
"#
        );

        let doc = Parser::default().parse(r#"`\+` and `+`"#);

        let block1 = doc.nested_blocks().next().unwrap();

        let Block::Simple(block1) = block1 else {
            panic!("Unexpected block type: {block1:?}");
        };

        assert_eq!(
            block1.content().rendered(),
            "<code>+</code> and <code>+</code>"
        );
    }
}
