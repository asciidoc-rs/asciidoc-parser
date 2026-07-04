use crate::tests::prelude::*;

track_file!("ref/asciidoc-lang/docs/modules/verbatim/pages/callouts.adoc");

non_normative!(
    r#"
= Callouts

Callout numbers (aka callouts) provide a means to add annotations to lines in a verbatim block.

"#
);

#[test]
fn callout_syntax() {
    verifies!(
        r#"
== Callout syntax

Each callout number used in a verbatim block must appear twice.
The first use, which goes within the verbatim block, marks the line being annotated (i.e., the target).
The second use, which goes below the verbatim block, defines the annotation text.
Multiple callout numbers may be used on a single line.

IMPORTANT: The callout number (at the target) must be placed at the end of the line.

Here's a basic example of a verbatim block that uses callouts:

.Callout syntax
[source#ex-basic,subs=-callouts]
....
include::example$callout.adoc[tag=basic]
....

The result of <<ex-basic>> is rendered below.

include::example$callout.adoc[tag=basic]

Since callout numbers can interfere with the syntax of the code they are annotating, an AsciiDoc processor provides several features to hide the callout numbers from both the source and the converted document.
The sections that follow detail these features.

"#
    );

    let doc = Parser::default().parse(
        "[source,ruby]\n----\nrequire 'asciidoctor' # <1>\ndoc = Asciidoctor::Document.new # <2>\n----\n<1> Imports the library\n<2> Creates the document\n",
    );

    // The callout numbers in the verbatim block are rendered as conums.
    assert_output_contains(&doc, r#"<b class="conum">(1)</b>"#);
    assert_output_contains(&doc, r#"<b class="conum">(2)</b>"#);

    // The annotation text below the block forms a callout list (`.colist`) with
    // one item per callout number.
    assert_css(&doc, ".colist ol li", 2);

    // The `subs=-callouts` modifier used in the syntax examples above suppresses
    // callout processing, so the callout numbers are shown literally.
    let raw = Parser::default().parse("[source,subs=-callouts]\n----\nrequire 'x' # <1>\n----\n");
    refute_output_contains(&raw, "conum");
    assert_output_contains(&raw, "# &lt;1&gt;");
}

#[test]
fn automatic_numbering() {
    verifies!(
        r#"
== Automatic numbering

Just like ordered lists, it's possible to allow the processor to automatically number the callouts.
To leverage this capability, you replace the numbers in each callout with a dot (e.g., `<.>`).
Each time the processor comes across a callout in either the verbatim block or the callout list, it selects the next number in the sequence (scoped to that block), starting from 1.

Let's return to the previous example to see how it looks if we use automatic numbering.

.Callout syntax with automatic numbering
[source#ex-auto,subs=-callouts]
....
include::example$callout.adoc[tag=auto]
....

The result is exactly the same as before.

"#
    );

    let doc = Parser::default().parse(
        "[source,ruby]\n----\nrequire 'asciidoctor' # <.>\ndoc = Asciidoctor::Document.new # <.>\n----\n<.> Imports the library\n<.> Creates the document\n",
    );

    // The `<.>` markers are numbered automatically, starting from 1.
    assert_output_contains(&doc, r#"<b class="conum">(1)</b>"#);
    assert_output_contains(&doc, r#"<b class="conum">(2)</b>"#);
    assert_css(&doc, ".colist ol li", 2);
}

#[test]
fn mixed_numbering() {
    verifies!(
        r#"
=== Mixed numbering

The `<.>` callouts are automatically numbered based on their sequence among other `<.>`, not any callouts that have an explicit number.
In other words, the automatic numbering is not aware of any explicit numbering.
Therefore, you should generally avoid mixing them.

However, if you want to repeat a number in the verbatim block, then you can use an explicit number to create additional occurrences of that callout number.

Let's consider an example:

.Callout syntax with mixed numbering
[source#ex-auto-repeat,subs=-callouts]
....
include::example$callout.adoc[tag=auto-repeat]
....

The result of <<ex-auto-repeat>> is rendered below.

include::example$callout.adoc[tag=auto-repeat]

The risk of this approach is that you have to keep track of which numbers are being assigned automatically.

"#
    );

    // The automatic numbering is not aware of the explicit `<1>`: the `<.>`
    // markers are numbered 1 and 2 in sequence, while the explicit `<1>` repeats
    // callout number 1. Asserted at the substitution level for an exact match.
    let mut content = crate::content::Content::from(crate::Span::new(
        "first &lt;.&gt;\nsecond &lt;.&gt;\nrepeat &lt;1&gt;",
    ));
    let parser = Parser::default();
    crate::content::SubstitutionStep::Callouts.apply(&mut content, &parser, None);
    assert_eq!(
        content.rendered(),
        "first <b class=\"conum\">(1)</b>\nsecond <b class=\"conum\">(2)</b>\nrepeat <b class=\"conum\">(1)</b>"
    );
}

#[test]
fn copy_and_paste_friendly_callouts() {
    verifies!(
        r#"
== Copy and paste friendly callouts

If you add callout numbers to example code in a verbatim (e.g., source) block, and a reader selects that source code in the generated HTML, we don't want the callout numbers to get caught up in the copied text.
If the reader pastes that example code into a code editor and tries to run it, the extra characters that define the callout numbers will likely lead to compile or runtime errors.
To mitigate this problem, and AsciiDoc processor uses a CSS rule to prevent the callouts from being selected.
That way, the callout numbers won't get copied.

On the other side of the coin, you don't want the callout annotations or CSS messing up your raw source code either.
You can tuck your callouts neatly behind line comments.
When font-based icons are enabled (e.g., `icons=font`), the AsciiDoc processor will recognize the line comments characters in front of a callout number--optionally offset by a space--and remove them when converting the document.
When font-based icons aren't enabled, the line comment characters are not removed so that the callout numbers remain hidden by the line comment.

Here are the line comments that are supported:

.Prevent callout copy and paste
[source#ex-prevent,subs=-callouts]
....
include::example$callout.adoc[tag=b-nonselect]
....

The result of <<ex-prevent>> is rendered below.

// The listing style must be added above the rendered block so it is rendered correctly because we're setting `source-language` in the component descriptor which automatically promotes unstyled listing blocks to source blocks.
[listing]
include::example$callout.adoc[tag=b-nonselect]

"#
    );

    // When font-based icons are not enabled, the line comment characters are
    // preserved ahead of the callout number, keeping the callout hidden behind
    // the comment in the raw source.
    let doc = Parser::default()
        .parse("[source,ruby]\n----\nputs 'Hello, world!' # <1>\n----\n<1> Ruby\n");
    assert_output_contains(&doc, r#"# <b class="conum">(1)</b>"#);

    // When font-based icons are enabled, the line comment characters are
    // removed.
    let doc = Parser::default()
        .with_intrinsic_attribute("icons", "font", ModificationContext::Anywhere)
        .parse("[source,ruby]\n----\nputs 'Hello, world!' # <1>\n----\n<1> Ruby\n");
    assert_output_contains(
        &doc,
        r#"'Hello, world!' <i class="conum" data-value="1"></i><b>(1)</b>"#,
    );
    refute_output_contains(&doc, "# <i");
}

#[test]
fn custom_line_comment_prefix() {
    verifies!(
        r#"
=== Custom line comment prefix

An AsciiDoc processor recognizes the most ubiquitous line comment prefixes as a convenience.
If the source language you're embedding does not support one of these line comment prefixes, you can customize the prefix using the `line-comment` attribute on the block.

Let's say we want to tuck a callout behind a line comment in Erlang code.
In this case, we would set the `line-comment` character to `%`, as shown in this example:

.Custom line comment prefix
[source#ex-line-comment,subs=-callouts]
....
include::example$callout.adoc[tag=line-comment]
....

The result of <<ex-line-comment>> is rendered below.

// The listing style must be added above the rendered block so it is rendered correctly because we're setting `source-language` in the component descriptor which automatically promotes unstyled listing blocks to source blocks.
[listing]
include::example$callout.adoc[tag=line-comment]

Even though it's not specified in the attribute, one space is still permitted immediately following the line comment prefix.

"#
    );

    // With `line-comment=%`, the `%` prefix (optionally followed by one space)
    // is recognized as a line comment hiding the callout.
    let doc = Parser::default().parse(
        "[source,erlang,line-comment=%]\n----\nhello_world() -> % <1>\n  io:fwrite(\"hello~n\"). %<2>\n----\n<1> Function head.\n<2> Output.\n",
    );
    assert_output_contains(&doc, r#"% <b class="conum">(1)</b>"#);
    assert_output_contains(&doc, r#"%<b class="conum">(2)</b>"#);
}

#[test]
fn disable_line_comment_processing() {
    verifies!(
        r#"
=== Disable line comment processing

If the source language you're embedding does not support trailing line comments, or the line comment prefix is being misinterpreted, you can disable this feature using the `line-comment` attribute.

Let's say we want to put a callout at the end of a block delimiter for an open block in AsciiDoc.
In this case, the processor will think the double hyphen is a line comment, when in fact it's the block delimiter.
We can disable line comment processing by setting the `line-comment` character to an empty value, as shown in this example:

.No line comment prefix
[source#ex-disable-line-comment,subs=-callouts]
....
include::example$callout.adoc[tag=disable-line-comment]
....

The result of <<ex-disable-line-comment>> is rendered below.

// The listing style must be added above the rendered block so it is rendered correctly because we're setting `source-language` in the component descriptor which automatically promotes unstyled listing blocks to source blocks.
[listing]
include::example$callout.adoc[tag=disable-line-comment]

Since the language doesn't support trailing line comments, there's no way to hide the callout number in the raw source.

"#
    );

    // With `line-comment=` (empty), no prefix is recognized, so the `--` before
    // the callout is preserved verbatim rather than being eaten as a comment.
    let doc = Parser::default().parse(
        "[source,asciidoc,line-comment=]\n----\n-- <1>\n----\n<1> The start of an open block.\n",
    );
    assert_output_contains(&doc, r#"-- <b class="conum">(1)</b>"#);
}

#[test]
fn xml_callouts() {
    verifies!(
        r#"
=== XML callouts

XML doesn't have line comments, so our "`tuck the callout behind a line comment`" trick doesn't work here.
To use callouts in XML, you must place the callout's angled brackets around the XML comment and callout number.

Here's how it appears in a listing:

.XML callout syntax
[source#ex-xml,subs=-callouts]
....
include::example$callout.adoc[tag=source-xml]
....

The result of <<ex-xml>> is rendered below.

include::example$callout.adoc[tag=source-xml]

Notice the comment has been replaced with a circled number that cannot be selected (if not using font icons it will be
rendered differently and selectable).
Now both you and the reader can copy and paste XML source code containing callouts without worrying about errors.

"#
    );

    // An XML callout places the angle brackets around the XML comment and
    // number; the rendered output keeps the comment delimiters around the conum.
    let doc = Parser::default().parse(
        "[source,xml]\n----\n<section>\n  <title>Section Title</title> <!--1-->\n</section>\n----\n<1> The title is required.\n",
    );
    assert_output_contains(&doc, r#"&lt;!--<b class="conum">(1)</b>--&gt;"#);
}

#[test]
fn callout_icons() {
    verifies!(
        r#"
== Callout icons

The font icons setting also enables callout icons drawn using CSS.

----
include::example$callout.adoc[tag=co-icon]
----
<.> Activates the font-based icons in the HTML5 backend.
<.> Admonition block that uses a font-based icon.
"#
    );

    // With `icons=font`, callout numbers render as font-based icons.
    let doc = Parser::default()
        .with_intrinsic_attribute("icons", "font", ModificationContext::Anywhere)
        .parse("----\n:icons: font <1>\n[TIP] <2>\n----\n<.> Activates the font-based icons in the HTML5 backend.\n<.> Admonition block that uses a font-based icon.\n");

    assert_output_contains(&doc, r#"<i class="conum" data-value="1"></i><b>(1)</b>"#);
    assert_output_contains(&doc, r#"<i class="conum" data-value="2"></i><b>(2)</b>"#);
}
