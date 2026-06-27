use crate::tests::prelude::*;

track_file!("docs/modules/blocks/pages/admonitions.adoc");

non_normative!(
    r#"
= Admonitions

There are certain statements you may want to draw attention to by taking them out of the content's flow and labeling them with a priority.
These are called admonitions.
This page introduces you to admonition types AsciiDoc provides, how to add admonitions to your document, and how to enhance them using icons or emoji.

NOTE: The examples on this page (and in these docs) use a visual theme that differs from the style provided by AsciiDoc processors such as Asciidoctor.
The AsciiDoc language does not require that the admonitions be rendered using a particular style.
The only requirement is that they be offset from the main text and labeled appropriately according to their admonition type.

"#
);

#[test]
fn admonition_types() {
    verifies!(
        r#"
== Admonition types

The rendered style of an admonition is determined by the assigned type (i.e., name).
The AsciiDoc language provides five admonition types represented by the following labels:

* `NOTE`
* `TIP`
* `IMPORTANT`
* `CAUTION`
* `WARNING`

The label is specified either as the block style or as a special paragraph prefix.
The label becomes visible to the reader unless icons are enabled, in which case the icon is shown in its place.

"#
    );

    // The five admonition types, keyed by their uppercase label, each map to a
    // lowercase name that becomes the admonition's CSS class.
    let doc =
        Parser::default().parse("NOTE: n\n\nTIP: t\n\nIMPORTANT: i\n\nCAUTION: c\n\nWARNING: w");

    assert_css(&doc, ".admonitionblock.note", 1);
    assert_css(&doc, ".admonitionblock.tip", 1);
    assert_css(&doc, ".admonitionblock.important", 1);
    assert_css(&doc, ".admonitionblock.caution", 1);
    assert_css(&doc, ".admonitionblock.warning", 1);

    // The label may be specified as a paragraph prefix...
    let mut parser = Parser::default();
    let block =
        crate::blocks::Block::parse(crate::Span::new("NOTE: paragraph prefix"), &mut parser)
            .unwrap_if_no_warnings()
            .unwrap()
            .item;
    assert_eq!(block.resolved_context().as_ref(), "admonition");
    assert_eq!(block.declared_style(), Some("NOTE"));

    // ...or as the block style.
    let mut parser = Parser::default();
    let block = crate::blocks::Block::parse(crate::Span::new("[NOTE]\nblock style"), &mut parser)
        .unwrap_if_no_warnings()
        .unwrap()
        .item;
    assert_eq!(block.resolved_context().as_ref(), "admonition");
    assert_eq!(block.declared_style(), Some("NOTE"));
}

non_normative!(
    r#"
.Caution vs. Warning
[#caution-vs-warning]
****
When choosing the admonition type, you may find yourself getting confused between "`caution`" and "`warning`" as these words are often used interchangeably.
Here's a simple rule to help you differentiate the two:

* Use *CAUTION* to advise the reader to _act_ carefully (i.e., exercise care).
* Use *WARNING* to inform the reader of danger, harm, or consequences that exist.

The word caution in this context translates into attention in French, which is often a good reference for how it should be applied.

To find a deeper analysis, see https://www.differencebetween.com/difference-between-caution-and-vs-warning/.
****

"#
);

#[test]
fn admonition_paragraph_syntax() {
    verifies!(
        r#"
== Admonition syntax

When you want to call attention to a single paragraph, start the first line of the paragraph with the label you want to use.
The label must be uppercase and followed by a colon (`:`).

.Admonition paragraph syntax
[#ex-label]
----
include::example$admonition.adoc[tag=para-c]
----
<.> The label must be uppercase and immediately followed by a colon (`:`).
<.> Separate the first line of the paragraph from the label by a single space.

The result of <<ex-label>> is displayed below.

include::example$admonition.adoc[tag=para]

"#
    );

    // The `para` example tag from the included file.
    let doc = Parser::default().parse(
        "WARNING: Wolpertingers are known to nest in server racks.\nEnter at your own risk.",
    );

    assert_css(&doc, ".admonitionblock.warning", 1);
    assert_xpath(
        &doc,
        "//td[@class=\"content\"][normalize-space(text())=\"Wolpertingers are known to nest in server racks. Enter at your own risk.\"]",
        1,
    );

    // The label must be uppercase: a lowercase label is not an admonition.
    let doc = Parser::default().parse("note: This is not an admonition.");
    assert_css(&doc, ".admonitionblock", 0);
    assert_css(&doc, ".paragraph", 1);

    // The label must be separated from the content by a space: without one, it
    // is not an admonition.
    let doc = Parser::default().parse("NOTE:This is not an admonition.");
    assert_css(&doc, ".admonitionblock", 0);
    assert_css(&doc, ".paragraph", 1);
}

#[test]
fn admonition_block_syntax() {
    verifies!(
        r#"
When you want to apply an admonition to compound content, set the label as a style attribute on a block.
As seen in the next example, admonition labels are commonly set on example blocks.
This behavior is referred to as *masquerading*.
The label must be uppercase when set as an attribute on a block.

.Admonition block syntax
[#ex-block]
----
include::example$admonition.adoc[tag=bl-c]
----
<.> Set the label in an attribute list on a delimited block.
The label must be uppercase.
<.> Admonition styles are commonly set on example blocks.
Example blocks are delimited by four equal signs (`====`).

The result of <<ex-block>> is displayed below.

include::example$admonition.adoc[tag=bl-nest]

"#
    );

    // The `bl-nest` example tag: an `[IMPORTANT]` style masquerading on an
    // example block produces an admonition with compound content.
    let doc = Parser::default().parse(
        "[IMPORTANT]\n.Feeding the Werewolves\n======\nWhile werewolves are hardy community members, keep in mind the following dietary concerns:\n\n. They are allergic to cinnamon.\n. More than two glasses of orange juice in 24 hours makes them howl in harmony with alarms and sirens.\n. Celery makes them sad.\n======",
    );

    assert_css(&doc, ".admonitionblock.important", 1);
    // The block title renders inside the content cell.
    assert_xpath(
        &doc,
        "//td[@class=\"content\"]/div[@class=\"title\"][text()=\"Feeding the Werewolves\"]",
        1,
    );
    // The compound content holds the nested ordered list.
    assert_css(&doc, ".admonitionblock.important td.content .olist", 1);

    // The label must be uppercase: a lowercase style is not an admonition (it
    // remains an example block).
    let doc = Parser::default().parse("[important]\n====\nNot an admonition.\n====");
    assert_css(&doc, ".admonitionblock", 0);
    assert_css(&doc, ".exampleblock", 1);
}

#[test]
fn enable_admonition_icons() {
    verifies!(
        r#"
== Enable admonition icons

In the examples above, the admonition is rendered in a callout box with the style label in the gutter.
You can replace the textual labels with font icons by setting the `icons` attribute on the document and assigning it the value `font`.

.Admonition paragraph with icons set
[#ex-icon]
----
= Document Title
:icons: font

include::example$admonition.adoc[tag=para]
----

Learn more about using Font Awesome or custom icons with admonitions in xref:macros:icons-font.adoc[].

"#
    );

    // With `icons` set to `font`, the textual label is replaced with a font
    // icon.
    let doc = Parser::default().parse(
        "= Document Title\n:icons: font\n\nWARNING: Wolpertingers are known to nest in server racks.\nEnter at your own risk.",
    );

    assert_css(
        &doc,
        ".admonitionblock.warning td.icon i.fa.icon-warning",
        1,
    );
    assert_css(&doc, "td.icon .title", 0);

    // Without the `icons` attribute, the textual label is shown.
    let doc = Parser::default().parse("WARNING: text");
    assert_css(&doc, "td.icon i", 0);
    assert_xpath(
        &doc,
        "//td[@class=\"icon\"]/div[@class=\"title\"][text()=\"Warning\"]",
        1,
    );
}

#[test]
fn using_emoji_for_admonition_icons() {
    verifies!(
        r#"
== Using emoji for admonition icons

If image-based or font-based icons are not available, you can leverage the admonition caption to display an emoji (or any symbol from Unicode) in the place of the admonition label, thus giving you an alternative way to make admonition icons.

If the `icons` attribute is not set on the document, the admonition label is shown as text (e.g., CAUTION).
The text for this label comes from an AsciiDoc attribute.
The name of the attribute is `<type>-caption`, where `<type>` is the admonition type in lowercase.
For example, the attribute for a tip admonition is `tip-caption`.

Instead of a word, you can assign a Unicode glyph to this attribute:

----
:tip-caption: 💡

[TIP]
It's possible to use Unicode glyphs as admonition icons.
----

Here's the result you get in the HTML:

[,html]
----
<td class="icon">
<div class="title">💡</div>
</td>
----

"#
    );

    // When icons are not set, the label is shown as text drawn from the
    // `<type>-caption` attribute, which defaults to the capitalized type name.
    let doc = Parser::default().parse("CAUTION: Slippery when wet.");
    assert_xpath(
        &doc,
        "//td[@class=\"icon\"]/div[@class=\"title\"][text()=\"Caution\"]",
        1,
    );

    // Assigning a Unicode glyph to the `<type>-caption` attribute makes that
    // glyph the label.
    let doc = Parser::default().parse(
        ":tip-caption: 💡\n\n[TIP]\nIt's possible to use Unicode glyphs as admonition icons.",
    );
    assert_xpath(
        &doc,
        "//td[@class=\"icon\"]/div[@class=\"title\"][text()=\"💡\"]",
        1,
    );
}

non_normative!(
    r#"
Instead of entering the glyph directly, you can enter a character reference instead.
However, since you're defining the character reference in an attribute entry, you (currently) have to disable substitutions on the value.

----
:tip-caption: pass:[&#128161;]

[TIP]
It's possible to use Unicode glyphs as admonition icons.
----

On GitHub, the HTML output from the AsciiDoc processor is run through a postprocessing filter that substitutes emoji shortcodes with emoji symbols.
That means you can use these shortcodes instead in the value of the attribute:

----
\ifdef::env-github[]
:tip-caption: :bulb:
\endif::[]

[TIP]
It's possible to use emojis as admonition icons on GitHub.
----

When the document is processed through the GitHub interface, the shortcodes get replaced with real emojis.
This is the only known way to get admonition icons to work on GitHub.
"#
);
