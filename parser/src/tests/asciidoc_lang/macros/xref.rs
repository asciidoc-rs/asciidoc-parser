use crate::tests::prelude::*;

track_file!("ref/asciidoc-lang/docs/modules/macros/pages/xref.adoc");

non_normative!(
    r##"
= Cross References

A link to another location within the current AsciiDoc document or in another AsciiDoc document is called a [.term]*cross reference* (also referred to as an [.term]*xref*).
To create a cross reference, you first need to define the location where the reference will point (i.e., the anchor).
Then, you need to use one of the forms of the inline xref macro to create a reference to that location.
From there, you can customize the text of the reference in various ways.

//don't change the id of this section unless you also change the example in the Internal cross references section (below this section)
[#anchors]
== Automatic anchors

It's important to understand that many anchors are already defined for you.
Using default settings, the AsciiDoc processor automatically creates an anchor for every section and discrete heading.
It does so by generating an ID for that section (or discrete heading) and registering that ID in the references catalog.
You can then use that ID as the target of a cross reference.

For example, considering the following section.

[source]
----
= Section Title
----

The AsciiDoc processor automatically assigns the ID `_section_title` to this section, which you can then use as the target of an xref to create a reference to this section.
You can also customize how this ID is generated.
Refer to xref:sections:auto-ids.adoc[] for more information about how an AsciiDoc processor generates these IDs.

If you're referring to a content element other than a section, you'll need to define an anchor on that element explicitly.

"##
);

non_normative!(
    r##"
== Internal cross references

In AsciiDoc, the shorthand xref is used to create a cross reference to an element (e.g., section, block, list item, etc.) that has an ID within the same document.
The shorthand xref is processed by the macros substitution.

If the cross reference specifies both an ID and text, the text is formatted and used as the link text.
If the cross reference only specifies the ID, the reftext of the target element (typically the formatted title) is automatically used as the link text.
If the element does not define reftext, a stylized form of the ID is used instead.
Whether the ID is assigned explicitly on the referenced element or auto-generated does not affect how this mechanism works.

Currently, an AsciiDoc processor can resolve a cross reference to the following elements:

* Section (ID or block anchor)
* Block (ID or block anchor)
* Block macro (ID)
* Inline anchor anywhere in a paragraph
* Inline anchor at the start of a list item or table cell
* Bibliography anchor in a bibliography list

Note that the processor cannot resolve the ID assigned to a span of formatted text.
If the cross reference cannot be resolved, and verbose mode is enabled, the AsciiDoc processor issues a warning about a possible invalid reference.
In this case, the output document will reference the target blindly, so it's possible it will still function.

You create a cross reference by enclosing the ID of the target block or section (or the path of another document with an optional anchor) in double angled brackets.

"##
);

#[test]
fn resolves_reference_by_target_id() {
    verifies!(
        r##"
.Cross reference using the ID of the target section
[source#ex-section]
----
include::example$xref.adoc[tag=base]
----

The result of <<ex-section>> is displayed below.

====
include::example$xref.adoc[tag=base]
====

"##
    );

    let doc = Parser::default().parse("See <<anchors>>.\n\n[#anchors]\n== Automatic anchors\n");

    assert_eq!(
        rendered_paragraphs(&doc),
        &[r##"See <a href="#anchors">Automatic anchors</a>."##]
    );
}

#[test]
fn reference_with_explicit_link_text() {
    verifies!(
        r##"
=== Explicit link text

Converters usually use the reftext of the target as the default text of the link.
When the document is parsed, attribute references in the reftext are substituted immediately.
When the reftext is displayed, additional reftext substitutions are applied to the text (specialchars, quotes, and replacements).

You can override the reftext of the target by specifying alternative text at the location of the cross reference.
After the ID, add a comma and then enter the custom text you want the cross reference to display.

.Cross reference with custom xreflabel text
[source#ex-custom]
----
include::example$xref.adoc[tag=text]
----

In this case, the target will be assumed to be an ID within the same document even if it contains a dot (`.`).

"##
    );

    // After the ID, a comma introduces the custom link text.
    let doc =
        Parser::default().parse("See <<the-id,Custom Text>>.\n\n[#the-id]\n== Some Section\n");

    assert_eq!(
        rendered_paragraphs(&doc),
        &[r##"See <a href="#the-id">Custom Text</a>."##]
    );
}

#[test]
fn inline_xref_macro_form() {
    verifies!(
        r##"
You can also use the inline xref macro as an alternative to the xref shorthand.

.Inline xref macro
[source]
----
include::example$xref.adoc[tag=xref-macro]
----

However, it's best to reserve the use of the xref macro for creating interdocument cross references.

When using the xref macro, if the target contains a dot (`.`), it will be treated as a reference to another document, not an ID within the same document.
If the intention is to link to an ID within the same document, the target must be proceeded by a hash (`#`).

"##
    );

    // The macro form resolves the same way as the shorthand.
    let doc =
        Parser::default().parse("See xref:the-id[Custom Text].\n\n[#the-id]\n== Some Section\n");

    assert_eq!(
        rendered_paragraphs(&doc),
        &[r##"See <a href="#the-id">Custom Text</a>."##]
    );

    // A `#`-prefixed target is an explicit reference to an ID in the same document.
    let doc = Parser::default().parse("See xref:#the-id[].\n\n[#the-id]\n== Some Section\n");

    assert_eq!(
        rendered_paragraphs(&doc),
        &[r##"See <a href="#the-id">Some Section</a>."##]
    );
}

#[test]
fn natural_cross_reference() {
    verifies!(
        r##"
=== Natural cross reference

You can also create a reference to a block or section using its title rather than its ID.
This type of reference is referred to as a [.term]*natural cross reference*.
The title must contain at least one space character or contain at least one uppercase letter.
//(If you are using Ruby < 2.4, that uppercase letter is restricted to the basic Latin charset).

.Cross reference using a section's title
[source#ex-title]
----
include::example$xref.adoc[tag=xref-title]
----

As a rule of thumb, the natural cross reference should only be used for rapid development or short-lived content.
As the content matures, you should switch to using IDs for referencing, ideally IDs which are declared explicitly.
By doing so, it ensures your references have maximum stability and are shielded against title revisions.

"##
    );

    // A reference by title resolves to the target's generated ID, using the
    // title as the link text.
    let doc = Parser::default().parse("See <<Some Section>>.\n\n== Some Section\n");

    assert_eq!(
        rendered_paragraphs(&doc),
        &[r##"See <a href="#_some_section">Some Section</a>."##]
    );
}

#[test]
fn target_a_blank_window() {
    verifies!(
        r##"
== Target a blank window

You can use the `window` attribute on the xref macro to control the link target (equivalent to the xref:link-macro-attribute-parsing.adoc#target-a-blank-window[window] attribute of the link macro).
Configuring a link that points to a location outside the current site is common practice to avoid disrupting the reader's flow.
This is a behavior that is specific to HTML output.

Most of the time, you’ll use the window attribute to target a blank window.
To enable this behavior, you set the window attribute to the special value _blank.

[source]
----
xref:page.adoc[window=_blank]
----

In the HTML output, the value of the window attribute is assigned to the target attribute on the <a> tag (e.g., target=_blank).
When the target is _blank, the processor will automatically add the `rel=noopener` attribute as well.

NOTE: The blank window shorthand, `^`, only works with the link macro.
"##
    );

    // The `window` attribute becomes the `target` attribute in the HTML output;
    // the special value `_blank` also adds `rel="noopener"`.
    let doc =
        Parser::default().parse("xref:the-id[Go,window=_blank]\n\n[#the-id]\n== Some Section\n");

    assert_eq!(
        rendered_paragraphs(&doc),
        &[r##"<a href="#the-id" target="_blank" rel="noopener">Go</a>"##]
    );
}
