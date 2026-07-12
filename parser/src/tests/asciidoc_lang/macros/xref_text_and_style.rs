use crate::tests::prelude::*;

track_file!("ref/asciidoc-lang/docs/modules/macros/pages/xref-text-and-style.adoc");

non_normative!(
    r##"
= Cross Reference Text and Styles

You can customize the style of the automatic cross reference text using the `xrefstyle` document attribute.
This customization brings the cross reference text formatting from the DocBook toolchain to AsciiDoc processing, specifically during conversion.

CAUTION: Since this is a newer feature of the AsciiDoc language, it may not be supported by all converters.
Where you can find support for it is in Asciidoctor's HTML, PDF, and EPUB 3 converters.
It's not supported by the DocBook converter since it's a feature the DocBook toolchain already provides.

"##
);

#[test]
fn default_styling_uses_title_or_reftext() {
    verifies!(
        r##"
== Default styling

By default, the cross reference text matches the title of the referenced element.
For example, if you're linking to a section titled “Installation”, the text of the cross reference link appears as:

====
Installation
====

If the reftext attribute is specified on the referenced element, that value is preferred over its title.
For example, let's assume the section from the previous example was written as:

[source]
----
[reftext="Installation Procedure"]
=== Installation
----

In this case, the text of the cross reference link appears as:

====
Installation Procedure
====

Attribute references are substituted in the reftext during parsing and reftext substitutions (specialchars, quotes, and replacements) are applied to the value when it's used during conversion.

If the reftext is not specified, the text of the cross reference is automatically generated.
By default, this text is the title of the reference.

"##
    );

    // By default the cross-reference text is the target's title.
    let doc = Parser::default().parse("See <<Installation>>.\n\n== Installation\n");

    assert_eq!(
        rendered_paragraphs(&doc),
        &[r##"See <a href="#_installation">Installation</a>."##]
    );

    // An explicit reftext on the target is preferred over its title.
    let doc = Parser::default().parse(
        "See <<install>>.\n\n[reftext=\"Installation Procedure\",id=install]\n== Installation\n",
    );

    assert_eq!(
        rendered_paragraphs(&doc),
        &[r##"See <a href="#install">Installation Procedure</a>."##]
    );
}

non_normative!(
    r##"
== Cross reference styles

The generated text of a cross reference is controlled by the xrefstyle.
It will also vary for different element types (section, figure, etc).
Let's consider the following document to learn how the xrefstyle value affects the generated text of a cross reference.

[,asciidoc]
----
== Installation

.Big Cats
image::big-cats.png[]
----

"##
);

/// Builds the spec's running-example document under the given `header` lines: a
/// section titled "Installation" numbered 2.3 and a captioned figure "Big Cats"
/// (Figure 1), preceded by a paragraph that cross-references both.
fn installation_and_figure(header: &str) -> String {
    format!(
        "{header}\n\n\
         Section: <<install>>. Figure: <<big-cats>>.\n\n\
         == One\n\n== Two\n\n=== Two-A\n\n=== Two-B\n\n\
         [#install]\n=== Installation\n\n\
         .Big Cats\n[#big-cats]\nimage::big-cats.png[]\n"
    )
}

/// Parses `input` and returns its first rendered paragraph (the one carrying
/// the cross references in these tests).
fn first_paragraph(input: &str) -> String {
    rendered_paragraphs(&Parser::default().parse(input))
        .into_iter()
        .next()
        .unwrap_or_default()
}

#[test]
fn cross_reference_styles() {
    verifies!(
        r##"
There are three built-in styles supported by the xrefstyle document attribute that you can choose from to customize the generated text of a cross reference.

 :xrefstyle: full:: Uses the signifier for the reference followed by the reference number and emphasized (chapter or appendix) or title enclosed in quotes (e.g., Section 2.3, “Installation”) (e.g., Figure 1, “Big Cats”).

 :xrefstyle: short:: Uses the signifier for the reference followed by the reference number (e.g., Section 2.3) (e.g., Figure 1).

 :xrefstyle: basic:: Uses the title only, only applying emphasis if the reference is a chapter or appendix (e.g., Installation) (e.g., Big Cats).

The `xrefstyle` attribute can also be specified directly on the xref:xref.adoc[xref macro] to override the xrefstyle value for a single reference (e.g., `+xref:installation[xrefstyle=short]+`).
The element attribute supports the same three styles.

The xrefstyle formatting only applies to references that have both a title and number (or explicit caption), but no explicit reftext.
If the reference is a chapter or an appendix, the title is displayed in italics instead of quotes (even when the xrefstyle is basic).

Let's assume you want to reference a section titled “Installation” that has the number 2.3.
The *full* style is displayed as:

====
Section 2.3, “Installation”
====

The *short* style is displayed as:

====
Section 2.3
====

The *basic* style is displayed as:

====
Installation
====

The *full* and *short* styles only apply for references that have a caption.
Specifically, the corresponding `<context>-caption` attribute must be set for the target's block type (e.g., `listing-caption` for listing blocks, `example-caption` for example blocks, `table-caption` for tables, etc.).
Otherwise, the *basic* style is used.

"##
    );

    // `full`: the signifier and number, then the title in typographic quotes.
    // A section takes its signifier from `section-refsig` ("Section"); a figure
    // (captioned image) from its `figure-caption` label ("Figure").
    assert_eq!(
        first_paragraph(&installation_and_figure(":sectnums:\n:xrefstyle: full")),
        r##"Section: <a href="#install">Section 2.3, &#8220;Installation&#8221;</a>. Figure: <a href="#big-cats">Figure 1, &#8220;Big Cats&#8221;</a>."##
    );

    // `short`: the signifier and number only.
    assert_eq!(
        first_paragraph(&installation_and_figure(":sectnums:\n:xrefstyle: short")),
        r##"Section: <a href="#install">Section 2.3</a>. Figure: <a href="#big-cats">Figure 1</a>."##
    );

    // `basic`: the title only (no emphasis, since neither is a chapter or
    // appendix).
    assert_eq!(
        first_paragraph(&installation_and_figure(":sectnums:\n:xrefstyle: basic")),
        r##"Section: <a href="#install">Installation</a>. Figure: <a href="#big-cats">Big Cats</a>."##
    );

    // The `xrefstyle=` attribute on the xref macro overrides the document value
    // for a single reference.
    let doc = Parser::default().parse(
        ":sectnums:\n:xrefstyle: full\n\n\
         Full: <<install>>. Short: xref:install[xrefstyle=short].\n\n\
         == One\n\n== Two\n\n=== Two-A\n\n=== Two-B\n\n[#install]\n=== Installation\n",
    );
    assert_eq!(
        rendered_paragraphs(&doc),
        &[
            r##"Full: <a href="#install">Section 2.3, &#8220;Installation&#8221;</a>. Short: <a href="#install">Section 2.3</a>."##
        ]
    );

    // A chapter or appendix title is emphasized (rendered in `<em>`) instead of
    // quoted — in every style, including `basic`. An appendix is lettered (A,
    // B, ...) and takes its signifier from `appendix-refsig`.
    let appendix = |style: &str| {
        format!(":xrefstyle: {style}\n\nSee <<data>>.\n\n[appendix]\n[#data]\n== Data\n")
    };
    assert_eq!(
        first_paragraph(&appendix("full")),
        r##"See <a href="#data">Appendix A, <em>Data</em></a>."##
    );
    assert_eq!(
        first_paragraph(&appendix("short")),
        r##"See <a href="#data">Appendix A</a>."##
    );
    assert_eq!(
        first_paragraph(&appendix("basic")),
        r##"See <a href="#data"><em>Data</em></a>."##
    );

    // The formatting applies only when the target has no explicit reftext: an
    // explicit reftext is used verbatim, whatever the style.
    let doc = Parser::default().parse(
        ":sectnums:\n:xrefstyle: full\n\nSee <<install>>.\n\n\
         [reftext=\"the installer\"]\n[#install]\n== Installation\n",
    );
    assert_eq!(
        rendered_paragraphs(&doc),
        &[r##"See <a href="#install">the installer</a>."##]
    );

    // The *full* and *short* styles apply only to a target with a caption. A
    // listing has no caption unless `listing-caption` is set, so it falls back
    // to *basic* (its title) — then gains a "Listing 1" caption once the
    // attribute is set.
    let listing = ":xrefstyle: short\n\nSee <<lst>>.\n\n.My Code\n[#lst]\n----\ncode\n----\n";
    assert_eq!(
        first_paragraph(listing),
        r##"See <a href="#lst">My Code</a>."##
    );
    assert_eq!(
        first_paragraph(&format!(":listing-caption: Listing\n{listing}")),
        r##"See <a href="#lst">Listing 1</a>."##
    );
}

#[test]
fn reference_signifiers() {
    verifies!(
        r##"
== Reference signifiers

You can use document attributes to customize the signifier that is placed in front of the reference's number.
This [.term]*reference signifier* indicates the reference's type (e.g., Chapter or Section).

* `chapter-refsig` -- defines the signifier to use for a cross reference to a chapter (default: Chapter)
* `section-refsig` -- defines the signifier to use for a cross reference to a section (default: Section)
* `appendix-refsig` -- defines the signifier to use for a cross reference to an appendix (default: Appendix)

(The signifier attribute for a part cross reference will be introduced once numeration is supported for parts).

For example, to customize the word “Section”, define the `section-refsig` attribute in the document header:

[source]
----
:section-refsig: Sect.
----

The *full* xrefstyle would then be displayed as:

====
Sect. 2.3, “Installation”
====

The *short* xrefstyle would be displayed as:

====
Sect. 2.3
====

If you unset the attribute, the signifier is dropped from the cross reference text.
For example:

[source]
----
:!section-refsig:
----

In this case, the *full* xrefstyle will display only the number and title:

====
2.3, “Installation”
====

The *short* xrefstyle will fall back to the number only:

====
2.3
====

The *basic* xrefstyle is unaffected by the value of the signifier.

"##
    );

    // Customizing `section-refsig` replaces the word in front of a section's
    // number. (`chapter-refsig` applies only in the `book` doctype, which this
    // parser does not model.) The figure signifier, from `figure-caption`, is
    // unaffected.
    assert_eq!(
        first_paragraph(&installation_and_figure(
            ":sectnums:\n:xrefstyle: full\n:section-refsig: Sect."
        )),
        r##"Section: <a href="#install">Sect. 2.3, &#8220;Installation&#8221;</a>. Figure: <a href="#big-cats">Figure 1, &#8220;Big Cats&#8221;</a>."##
    );
    assert_eq!(
        first_paragraph(&installation_and_figure(
            ":sectnums:\n:xrefstyle: short\n:section-refsig: Sect."
        )),
        r##"Section: <a href="#install">Sect. 2.3</a>. Figure: <a href="#big-cats">Figure 1</a>."##
    );

    // Unsetting the signifier drops the word, leaving only the number; *short*
    // then falls back to the number alone.
    assert_eq!(
        first_paragraph(&installation_and_figure(
            ":sectnums:\n:xrefstyle: full\n:!section-refsig:"
        )),
        r##"Section: <a href="#install">2.3, &#8220;Installation&#8221;</a>. Figure: <a href="#big-cats">Figure 1, &#8220;Big Cats&#8221;</a>."##
    );
    assert_eq!(
        first_paragraph(&installation_and_figure(
            ":sectnums:\n:xrefstyle: short\n:!section-refsig:"
        )),
        r##"Section: <a href="#install">2.3</a>. Figure: <a href="#big-cats">Figure 1</a>."##
    );

    // The *basic* xrefstyle uses the title only, so it is unaffected by the
    // signifier's value (set or unset).
    assert_eq!(
        first_paragraph(&installation_and_figure(
            ":sectnums:\n:xrefstyle: basic\n:section-refsig: Sect."
        )),
        r##"Section: <a href="#install">Installation</a>. Figure: <a href="#big-cats">Big Cats</a>."##
    );
}

#[test]
fn anchor_reftext_suppresses_signifier() {
    // A `[[id,reftext]]` block anchor supplies an explicit reftext, which is
    // used verbatim even under `full`/`short` — the section is *not* rendered
    // as "Section 2.3". (Regression: the signifier gate previously checked only
    // the `reftext` attribute, not the anchor reftext.)
    let input = "\
        :sectnums:\n:xrefstyle: full\n\n\
        See <<install>>.\n\n\
        == One\n\n== Two\n\n=== Two-A\n\n=== Two-B\n\n\
        [[install,Read the install guide]]\n=== Installation\n";

    assert_eq!(
        first_paragraph(input),
        r##"See <a href="#install">Read the install guide</a>."##
    );
}

#[test]
fn non_integer_caption_counter_keeps_signifier() {
    // A captioned block whose context counter holds a non-integer value (here
    // `figure-number` seeded to render "Figure B") is genuinely auto-numbered
    // even though it exposes no bare integer number. `full`/`short` must still
    // build the signifier from its caption rather than falling back to the
    // title. (Regression: the gate previously required an integer number.)
    let numbered = "\
        :xrefstyle: {style}\n:figure-number: A\n\n\
        See <<big-cats>>.\n\n\
        .Big Cats\n[#big-cats]\nimage::big-cats.png[]\n";

    assert_eq!(
        first_paragraph(&numbered.replace("{style}", "full")),
        r##"See <a href="#big-cats">Figure B, &#8220;Big Cats&#8221;</a>."##
    );
    assert_eq!(
        first_paragraph(&numbered.replace("{style}", "short")),
        r##"See <a href="#big-cats">Figure B</a>."##
    );

    // An explicit caption override, by contrast, is not numbered and gets no
    // signifier, so `full` falls back to the title. An override can come from
    // the block attrlist, the image macro's own attrlist, or a document-wide
    // `caption` attribute — each suppresses the signifier.
    let expected = r##"See <a href="#big-cats">Big Cats</a>."##;

    let block_attr_override = "\
        :xrefstyle: full\n\n\
        See <<big-cats>>.\n\n\
        .Big Cats\n[#big-cats,caption=\"Plate \"]\nimage::big-cats.png[]\n";
    assert_eq!(first_paragraph(block_attr_override), expected);

    let macro_attr_override = "\
        :xrefstyle: full\n\n\
        See <<big-cats>>.\n\n\
        .Big Cats\n[#big-cats]\nimage::big-cats.png[caption=\"Plate \"]\n";
    assert_eq!(first_paragraph(macro_attr_override), expected);

    let document_wide_override = "\
        :xrefstyle: full\n:caption: Plate \n\n\
        See <<big-cats>>.\n\n\
        .Big Cats\n[#big-cats]\nimage::big-cats.png[]\n";
    assert_eq!(first_paragraph(document_wide_override), expected);
}

non_normative!(
    r##"
Only the aforementioned styles are provided out of the box.
Support for a custom formatting string is planned.
Refer to https://github.com/asciidoctor/asciidoctor/issues/2212[#2212^] for details.
Until then, you can implement custom formatting in a custom converter or overriding the xreftext method on the node.
"##
);
