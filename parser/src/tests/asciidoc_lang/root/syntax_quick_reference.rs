use crate::{
    blocks::{Block, BreakType, DataFormat, ListType, MediaType},
    tests::prelude::{inline_file_handler::InlineFileHandler, *},
};

track_file!("ref/asciidoc-lang/docs/modules/ROOT/pages/syntax-quick-reference.adoc");

// SDD coverage for the AsciiDoc Syntax Quick Reference page. Every line of the
// page is tiled into a coverage block. The page's examples and the behavior it
// describes are exercised against the parser, so the descriptive prose and
// example blocks are marked `verifies!` and backed by assertions. Only section
// headings, page metadata, authoring comments, and rendering/output-only notes
// (which this parser does not implement) are `non_normative!`.
//
// One feature described on the page is not yet implemented; it is marked
// `to_do_verifies!` with assertions documenting current behavior:
//   * `book` doctype parts / multi-part books (issue #380).
//
// Language-aware fenced code blocks (```lang, issue #615) are now recognized;
// both the bare ``` fence and the language-on-fence form are verified in
// `markdown_compatibility`.

#[test]
fn preamble() {
    non_normative!(
        r#"
= AsciiDoc Syntax Quick Reference
:navtitle: Syntax Quick Reference
:description: The quick reference for common AsciiDoc document and text formatting markup.
:collapsible:
:url-char-xml: https://en.wikipedia.org/wiki/List_of_XML_and_HTML_character_entity_references
:url-data-uri: https://developer.mozilla.org/en-US/docs/data_URIs
:!table-frame:
:!table-grid:
// release-version is used for an example; it's not the release version for this document
:release-version: 2.4.3

////
This document is not meant to be a replacement for the documentation of the AsciiDoc language itself.
It's meant to be a helpful guide you can give to a writer to refer to while in the thick of writing.
Think of it a quick reminder of the most common syntax and scenarios.
It should not go into any depth about AsciiDoc processing or the options you can use when converting to an output format.
////

[IMPORTANT]
The examples on this page demonstrate the output produced by the built-in HTML converter.
An AsciiDoc converter is expected to produce complementary output when generating other output formats, such as PDF, EPUB, and DocBook.

"#
    );
}

#[test]
fn paragraphs() {
    non_normative!(
        r#"
== Paragraphs

"#
    );

    verifies!(
        r#"
.Paragraph
[#ex-normal]
----
include::text:example$text.adoc[tag=b-para]
----

.View result of <<ex-normal>>
[%collapsible.result]
====
include::text:example$text.adoc[tag=b-para]
====

.Literal paragraph
[#ex-literal]
----
include::verbatim:example$literal.adoc[tag=qr-para]
----

.View result of <<ex-literal>>
[%collapsible.result]
====
include::verbatim:example$literal.adoc[tag=qr-para]
====

.Hard line breaks
[#ex-hardbreaks]
----
include::text:example$text.adoc[tag=hb-all]
----

.View result of <<ex-hardbreaks>>
[%collapsible.result]
====
include::text:example$text.adoc[tag=hb-all]
====

.Lead paragraph
[#ex-lead]
----
include::text:example$text.adoc[tag=qr-lead]
----

.View result of <<ex-lead>>
[%collapsible.result]
====
include::text:example$text.adoc[tag=qr-lead]
====

"#
    );

    non_normative!(
        r#"
TIP: The default Asciidoctor stylesheet automatically styles the first paragraph of the preamble as a xref:blocks:preamble-and-lead.adoc[lead paragraph] if no role is specified on that paragraph.

"#
    );

    // A blank line separates paragraphs; line breaks within a paragraph are not
    // displayed as hard breaks unless marked.
    let doc = Parser::default().parse("Roses are red,\nviolets are blue.\n\nA new paragraph.");
    assert_eq!(rendered_paragraphs(&doc).len(), 2);

    // A trailing `+` produces a hard line break.
    let doc = Parser::default().parse("Roses are red, +\nviolets are blue.");
    assert!(rendered_paragraphs(&doc)[0].contains("<br>"));

    // An indented line is a literal paragraph.
    let doc = Parser::default().parse(" a literal line");
    let Some(Block::Simple(sb)) = doc.nested_blocks().next() else {
        panic!("expected a simple block");
    };
    assert_eq!(sb.style(), SimpleBlockStyle::Literal);

    // A lead paragraph carries the `lead` role. (Its enlarged styling is a
    // stylesheet/output concern, so only the role is asserted here.)
    let doc =
        Parser::default().parse("[.lead]\nThis text is the lead paragraph.\n\nThis text is not.");
    assert!(
        doc.nested_blocks()
            .next()
            .unwrap()
            .roles()
            .contains(&"lead")
    );
}

#[test]
fn text_formatting() {
    non_normative!(
        r#"
== Text formatting

"#
    );

    verifies!(
        r#"
.Constrained bold, italic, and monospace
[#ex-constrained]
----
include::text:example$text.adoc[tag=constrained-bold-italic-mono]
----

.View result of <<ex-constrained>>
[%collapsible.result]
====
include::text:example$text.adoc[tag=constrained-bold-italic-mono]
====

.Unconstrained bold, italic, and monospace
[#ex-unconstrained]
----
include::text:example$text.adoc[tag=unconstrained-bold-italic-mono]
----

.View result of <<ex-unconstrained>>
[%collapsible.result]
====
include::text:example$text.adoc[tag=unconstrained-bold-italic-mono]
====

.Highlight, underline, strikethrough, and custom role
[#ex-lines]
----
include::text:example$text.adoc[tag=qr-all]
----

.View result of <<ex-lines>>
[%collapsible.result]
====
include::text:example$text.adoc[tag=qr-all]
====

.Superscript and subscript
[#ex-sub-sup]
----
include::text:example$text.adoc[tag=b-sub-sup]
----

.View result of <<ex-sub-sup>>
[%collapsible.result]
====
include::text:example$text.adoc[tag=b-sub-sup]
====

.Smart quotes and apostrophes
[#ex-curved]
----
include::text:example$text.adoc[tag=b-c-quote]
----

.View result of <<ex-curved>>
[%collapsible.result]
====
include::text:example$text.adoc[tag=b-c-quote]
====

"#
    );

    // Constrained bold, italic, and monospace.
    let doc = Parser::default().parse("A *bold* _italic_ `mono` word.");
    let r = &rendered_paragraphs(&doc)[0];
    assert!(r.contains("<strong>bold</strong>"));
    assert!(r.contains("<em>italic</em>"));
    assert!(r.contains("<code>mono</code>"));

    // Unconstrained forms with doubled marks.
    let doc = Parser::default().parse("**C**reate and c__hara__cters.");
    let r = &rendered_paragraphs(&doc)[0];
    assert!(r.contains("<strong>C</strong>reate"));
    assert!(r.contains("c<em>hara</em>cters"));

    // Superscript and subscript.
    let doc = Parser::default().parse("H~2~O and E=mc^2^");
    let r = &rendered_paragraphs(&doc)[0];
    assert!(r.contains("<sub>2</sub>"));
    assert!(r.contains("<sup>2</sup>"));

    // Highlight.
    let doc = Parser::default().parse("Mark #these words#.");
    assert!(rendered_paragraphs(&doc)[0].contains("<mark>these words</mark>"));

    // Smart (curved) quotes.
    let doc = Parser::default().parse("\"`double`\"");
    assert_eq!(rendered_paragraphs(&doc)[0], "&#8220;double&#8221;");
}

#[test]
fn links() {
    non_normative!(
        r#"
== Links

"#
    );

    verifies!(
        r#"
.Autolinks, URL macro, and mailto macro
[#ex-urls]
----
include::macros:example$url.adoc[tag=b-base]

include::macros:example$url.adoc[tag=b-scheme]
----

.View result of <<ex-urls>>
[%collapsible.result]
====
include::macros:example$url.adoc[tag=b-base]

include::macros:example$url.adoc[tag=b-scheme]
====

.URL macros with attributes
[#ex-linkattrs]
----
include::macros:example$url.adoc[tag=b-linkattrs]
----

.View result of <<ex-linkattrs>>
[%collapsible.result]
====
include::macros:example$url.adoc[tag=b-linkattrs]
====

IMPORTANT: The `link:` macro prefix is _not_ required when the target starts with a URL scheme like `https:`.
The URL scheme acts as an implicit macro prefix.

CAUTION: If the link text contains a comma and the text is followed by one or more named attributes, you must enclose the text in double quotes.
Otherwise, the text will be cut off at the comma (and the remaining text will get pulled into the attribute parsing).

.URLs with spaces and special characters
----
include::macros:example$url.adoc[tag=b-spaces]
----

.Link to relative file
----
link:index.html[Docs]
----

.Link using a Windows UNC path
----
include::macros:example$url.adoc[tag=b-windows]
----

.Inline anchors
----
include::attributes:example$id.adoc[tag=anchor]
----

.Cross references
[#ex-xrefs]
----
include::macros:example$xref.adoc[tag=b-base]
----

.View result of <<ex-xrefs>>
[%collapsible.result]
====
include::macros:example$xref.adoc[tag=b-base]
====

.Inter-document cross references
----
include::macros:example$xref.adoc[tag=b-inter]
----

"#
    );

    // The `link:` prefix is not required when the target begins with a URL
    // scheme: the scheme acts as an implicit macro prefix.
    let doc = Parser::default().parse("Visit https://asciidoctor.org[Asciidoctor] now.");
    assert!(
        rendered_paragraphs(&doc)[0]
            .contains(r#"<a href="https://asciidoctor.org">Asciidoctor</a>"#)
    );

    // A bare URL (no macro, no brackets) still autolinks.
    let doc = Parser::default().parse("https://asciidoctor.org - automatic!");
    assert!(
        rendered_paragraphs(&doc)[0].contains(r#"<a href="https://asciidoctor.org" class="bare">"#)
    );

    // With a named attribute present, link text containing a comma must be
    // quoted, otherwise it is truncated at the first comma.
    let doc =
        Parser::default().parse(r#"https://example.org["Google, DuckDuckGo, Ecosia",role=teal]"#);
    assert!(rendered_paragraphs(&doc)[0].contains("Google, DuckDuckGo, Ecosia"));
}

#[test]
fn document_header() {
    non_normative!(
        r#"
== Document header

"#
    );

    verifies!(
        r#"
The xref:document:header.adoc[document header] is optional.
The header may not contain any empty lines and must be separated from the content by at least one empty line.

.Title
----
include::document:example$title.adoc[tag=qr-title]
----

.Title and author line
----
include::document:example$header.adoc[tag=qr-author]
----

.Title, author line, and revision line
----
include::document:example$header.adoc[tag=qr-rev]
----

IMPORTANT: You cannot have a xref:document:revision-line.adoc[revision line] without an xref:document:author-line.adoc[author line].

.Document header with attribute entries
----
include::document:example$header.adoc[tag=qr-attributes]
----

"#
    );

    // The header is optional: a document may begin directly with body content.
    let doc = Parser::default().parse("Just a paragraph, no header.");
    assert!(doc.header().title().is_none());

    // The header is separated from the body by an empty line; the first empty
    // line ends the header and starts the body.
    let doc =
        Parser::default().parse("= Title\nAuthor Name <author@email.org>\n\nBody starts here.");
    assert_eq!(doc.header().title(), Some("Title"));
    assert!(doc.header().author_line().is_some());
    assert_eq!(
        rendered_paragraphs(&doc),
        vec!["Body starts here.".to_string()]
    );

    // A revision line cannot exist without an author line: given a title and a
    // single following line, that line is parsed as the author line.
    let doc = Parser::default().parse("= Title\nv2.0, 2019-03-22");
    assert!(doc.header().revision_line().is_none());

    // With an author line present, the third line is parsed as the revision line.
    let doc = Parser::default().parse("= Title\nAuthor Name <author@email.org>\nv2.0, 2019-03-22");
    assert!(doc.header().author_line().is_some());
    assert!(doc.header().revision_line().is_some());
}

#[test]
fn section_titles() {
    non_normative!(
        r#"
[#section-titles]
== Section titles

"#
    );

    verifies!(
        r#"
When the document type is `article` (the default), the document can only have one level 0 section title (`=`), which is the document title (i.e., doctitle).

.Article section levels
[#ex-article]
----
include::sections:example$section.adoc[tag=base]
----

.View result of <<ex-article>>
[%collapsible.result]
====
include::sections:example$section.adoc[tag=b-base]
====

"#
    );

    to_do_verifies!(
        r#"
The `book` document type can have additional level 0 section titles, which are interpreted as xref:sections:parts.adoc[parts].
The presence of at least one part implicitly makes the document a multi-part book.
"#
    );

    // TO DO (https://github.com/asciidoc-rs/asciidoc-parser/issues/380): the
    // `book` doctype's additional level-0 titles (parts) and multi-part books
    // are not yet implemented. Today a second level-0 heading always warns,
    // regardless of `:doctype: book`.
    let doc = Parser::default()
        .parse("= Book Title\n:doctype: book\n\n== Chapter One\n\n= Part Two\n\n== Chapter Two");
    assert!(
        doc.warnings()
            .any(|w| w.warning == WarningType::Level0SectionHeadingNotSupported)
    );

    verifies!(
        r#"

.Book section levels
----
include::sections:example$section.adoc[tag=book]
----

"#
    );

    non_normative!(
        r#"
////
xref:sections:title-links.adoc#link[sectlinks]::
When the document attribute `sectlinks` is set, section titles become self-links.
This feature allows a reader to easily bookmark the section.

xref:sections:title-links.adoc#anchor[sectanchors]::
When the document attribute `sectanchors` is set, a floating section icon anchor appears in front of the section title on hover.
This feature provides an alternate way for the reader to easily bookmark the section.
Section title anchors depend on support from the stylesheet to render properly.
////

"#
    );

    verifies!(
        r#"
.Discrete heading (not a section)
[#ex-discrete]
----
[discrete]
=== I'm an independent heading!

This paragraph is its sibling, not its child.
----

.View result of <<ex-discrete>>
[%collapsible.result]
====
[discrete]
=== I'm an independent heading!

This paragraph is its sibling, not its child.
====

"#
    );

    // In the default (article) doctype, only one level-0 title is allowed. A
    // second level-0 heading is not promoted to a section; it warns instead.
    let doc = Parser::default().parse("= Document Title\n\n= Second Level 0\n\n== A Section");
    assert!(
        doc.warnings()
            .any(|w| w.warning == WarningType::Level0SectionHeadingNotSupported)
    );

    // A discrete heading parses as a section-style heading marked discrete, but
    // it does not adopt the following content as a child: the heading and the
    // paragraph are siblings at the same level.
    let doc =
        Parser::default().parse("[discrete]\n=== Independent heading\n\nA sibling paragraph.");
    let blocks: Vec<_> = doc.nested_blocks().collect();
    assert_eq!(blocks.len(), 2);
    let Block::Section(heading) = blocks[0] else {
        panic!("expected a discrete section heading");
    };
    assert_eq!(heading.section_type(), SectionType::Discrete);
    assert_eq!(heading.nested_blocks().count(), 0);
    assert!(matches!(blocks[1], Block::Simple(_)));
}

#[test]
fn automatic_toc() {
    non_normative!(
        r#"
== Automatic TOC

"#
    );

    verifies!(
        r#"
.Activate Table of Contents for a document
----
= Document Title
Doc Writer <doc.writer@email.org>
:toc:
----

The Table of Contents`' xref:toc:title.adoc[title], xref:toc:levels.adoc[displayed section depth], and xref:toc:position.adoc[position] can be customized.

"#
    );

    // Setting the `toc` attribute activates the table of contents.
    let doc = Parser::default()
        .parse("= Document Title\nDoc Writer <doc.writer@email.org>\n:toc:\n\nContent.");
    assert!(doc.toc_mode().is_enabled());

    // Without it, the table of contents is disabled.
    let doc = Parser::default().parse("= Document Title\n\nContent.");
    assert!(!doc.toc_mode().is_enabled());
}

#[test]
fn includes() {
    non_normative!(
        r#"
== Includes

"#
    );

    verifies!(
        r#"
.Include document parts
----
include::directives:example$include.adoc[tag=base]
----

.Include content by tagged regions or lines
----
include::directives:example$include.adoc[tag=include-with-tag]

include::directives:example$include.adoc[tag=line]
----

.Include content from a URL
----
include::directives:example$include.adoc[tag=uri]
----

WARNING: Including content from a URL is potentially dangerous, so it's disabled if the safe mode is SECURE or greater.
Assuming the safe mode is less than SECURE, you must also set the `allow-uri-read` attribute to permit the AsciiDoc processor to read content from a URL.

"#
    );

    // Including content from a URL requires `allow-uri-read` and a safe mode
    // below SECURE.
    const URI: &str = "https://example.org/remote.adoc";
    let handler = InlineFileHandler::from_pairs([(URI, "Remote content.")]);
    let doc = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_primary_file_name("main.adoc")
        .with_include_file_handler(handler)
        .with_intrinsic_attribute("allow-uri-read", "", ModificationContext::Anywhere)
        .parse(&format!("include::{URI}[]"));
    assert!(
        rendered_paragraphs(&doc)
            .iter()
            .any(|p| p.contains("Remote content."))
    );

    // In SECURE mode the read is forcefully disabled even with `allow-uri-read`.
    let handler = InlineFileHandler::from_pairs([(URI, "SECRET CONTENT")]);
    let doc = Parser::default()
        .with_safe_mode(SafeMode::Secure)
        .with_primary_file_name("main.adoc")
        .with_include_file_handler(handler)
        .with_intrinsic_attribute("allow-uri-read", "", ModificationContext::Anywhere)
        .parse(&format!("include::{URI}[]"));
    assert!(
        rendered_paragraphs(&doc)
            .iter()
            .all(|p| !p.contains("SECRET CONTENT"))
    );
}

#[test]
fn lists() {
    non_normative!(
        r#"
== Lists

"#
    );

    verifies!(
        r#"
.Unordered list
[#ex-ul]
----
include::lists:example$unordered.adoc[tag=qr-base]
----

.View result of <<ex-ul>>
[%collapsible.result]
====
include::lists:example$unordered.adoc[tag=qr-base]
====

TIP: An empty line is required before and after a list to separate it from other blocks.
You can force two adjacent lists apart by adding an empty attribute list (i.e., `[]`) above the second list or by inserting an empty line followed by a line comment after the first list.
If you use a line comment, the convention is to use `//-` to provide a hint to other authors that it's serving as a list divider.

.Unordered list max level nesting
[#ex-ul-max]
----
include::lists:example$unordered.adoc[tag=max]
----

.View result of <<ex-ul-max>>
[%collapsible.result]
====
include::lists:example$unordered.adoc[tag=max]
====

The xref:lists:unordered.adoc#markers[unordered list marker] can be changed using a list style (e.g., `square`).

.Ordered list
[#ex-ol]
----
include::lists:example$ordered.adoc[tag=nest]
----

.View result of <<ex-ol>>
[%collapsible.result]
====
include::lists:example$ordered.adoc[tag=nest]
====

.Ordered list max level nesting
[#ex-ol-max]
----
include::lists:example$ordered.adoc[tag=max]
----

.View result of <<ex-ol-max>>
[%collapsible.result]
====
include::lists:example$ordered.adoc[tag=max]
====

Ordered lists support xref:lists:ordered.adoc#styles[numeration styles] such as `lowergreek` and `decimal-leading-zero`.

.Checklist
[#ex-check]
----
include::lists:example$checklist.adoc[tag=check]
----

.View result of <<ex-check>>
[%collapsible.result]
====
include::lists:example$checklist.adoc[tag=check]
====

.Description list
[#ex-dlist]
----
include::lists:example$description.adoc[tag=qr-base]
----

.View result of <<ex-dlist>>
[%collapsible.result]
====
include::lists:example$description.adoc[tag=qr-base]
====

.Question and answer list
[#ex-qa]
----
include::lists:example$description.adoc[tag=qa]
----

.View result of <<ex-qa>>
[%collapsible.result]
====
include::lists:example$description.adoc[tag=qa]
====

.Mixed
[#ex-mixed]
----
include::lists:example$description.adoc[tag=3-mix]
----

.View result of <<ex-mixed>>
[%collapsible.result]
====
include::lists:example$description.adoc[tag=3-mix]
====

TIP: Lists can be indented.
Leading whitespace is not significant.

.Complex content in outline lists
[#ex-complex]
----
include::lists:example$complex.adoc[tag=b-complex]
----

.View result of <<ex-complex>>
[%collapsible.result]
====
include::lists:example$complex.adoc[tag=b-complex]
====

"#
    );

    // A line comment (conventionally `//-`) forces two adjacent lists apart.
    let doc = Parser::default().parse("* Apples\n* Oranges\n\n//-\n\n* Walnuts\n* Almonds");
    assert_eq!(
        doc.nested_blocks()
            .filter(|b| matches!(b, Block::List(_)))
            .count(),
        2
    );

    // An empty attribute list (`[]`) also separates two adjacent lists.
    let doc = Parser::default().parse("* Apples\n* Oranges\n\n[]\n. Wash\n. Slice");
    assert_eq!(
        doc.nested_blocks()
            .filter(|b| matches!(b, Block::List(_)))
            .count(),
        2
    );

    // The unordered list marker can be changed with a list style (e.g. `square`),
    // which the parser captures as the list's declared style.
    let doc = Parser::default().parse("[square]\n* one\n* two");
    let Some(Block::List(list)) = doc.nested_blocks().next() else {
        panic!("expected a list");
    };
    assert_eq!(list.type_(), ListType::Unordered);
    assert_eq!(list.declared_style(), Some("square"));

    // Ordered lists support numeration styles (e.g. `lowergreek`), captured as
    // the list's declared style.
    let doc = Parser::default().parse("[lowergreek]\n. one\n. two");
    let Some(Block::List(list)) = doc.nested_blocks().next() else {
        panic!("expected a list");
    };
    assert_eq!(list.type_(), ListType::Ordered);
    assert_eq!(list.declared_style(), Some("lowergreek"));

    // Lists can be indented; leading whitespace is not significant, so these
    // three items remain a single flat list.
    let doc = Parser::default().parse("* Edgar Allan Poe\n * Sheri S. Tepper\n     * Bill Bryson");
    let Some(Block::List(list)) = doc.nested_blocks().next() else {
        panic!("expected a list");
    };
    assert_eq!(list.nested_blocks().count(), 3);

    // Checklist items expose their checkbox state.
    let doc = Parser::default().parse("* [x] done\n* [ ] todo\n* plain");
    let Some(Block::List(list)) = doc.nested_blocks().next() else {
        panic!("expected a list");
    };
    assert!(list.is_checklist());

    // Description lists.
    let doc = Parser::default().parse("CPU:: The brain of the computer.");
    let Some(Block::List(list)) = doc.nested_blocks().next() else {
        panic!("expected a list");
    };
    assert_eq!(list.type_(), ListType::Description);

    // Question-and-answer lists are description lists carrying the `qanda` style.
    let doc = Parser::default().parse("[qanda]\nWhat is the answer?::\nThis is the answer.");
    let Some(Block::List(list)) = doc.nested_blocks().next() else {
        panic!("expected a list");
    };
    assert_eq!(list.type_(), ListType::Description);
}

#[test]
fn images() {
    non_normative!(
        r#"
== Images

"#
    );

    verifies!(
        r#"
You can use the xref:macros:images-directory.adoc[imagesdir attribute] to avoid hard coding the common path to your images in every image macro.
The value of this attribute can be an absolute path, relative path, or base URL.
If the image target is a relative path, the attribute's value is prepended (i.e., it's resolved relative to the value of the `imagesdir` attribute).
If the image target is a URL or absolute path, the attribute's value is _not_ prepended.

.Block image macro
[#ex-image-blocks]
----
include::macros:example$image.adoc[tag=base]

include::macros:example$image.adoc[tag=alt]

include::macros:example$image.adoc[tag=qr-attr]

include::macros:example$image.adoc[tag=ab-url]
----

.View result of <<ex-image-blocks>>
[%collapsible.result]
====
include::macros:example$image.adoc[tag=qr-base]

include::macros:example$image.adoc[tag=qr-alt]

include::macros:example$image.adoc[tag=qr-attr]

include::macros:example$image.adoc[tag=ab-url]
====

Two colons following the image keyword in the macro (i.e., `image::`) indicates a block image (aka figure), whereas one colon following the image keyword (i.e., `image:`) indicates an inline image.
(All macros follow this pattern).
You use an inline image when you need to place the image in a line of text.
Otherwise, you should prefer the block form.

.Inline image macro
[#ex-image-inline]
----
include::macros:example$image.adoc[tag=inline]
----

.View result of <<ex-image-inline>>
[%collapsible.result]
====
include::macros:example$image.adoc[tag=qr-inline]
====

.Inline image macro with positioning role
[#ex-image-role]
----
include::macros:example$image.adoc[tag=in-role]
----

.View result of <<ex-image-role>>
[%collapsible.result]
====
include::macros:example$image.adoc[tag=qr-role]
====

.Embedded
----
include::macros:example$image.adoc[tag=data]
----

"#
    );

    non_normative!(
        r#"
When the `data-uri` attribute is set, all images in the document--including admonition icons--are embedded into the document as {url-data-uri}[data URIs].
You can also pass it as a command line argument using `-a data-uri`.

"#
    );

    // `imagesdir` is prepended to a relative image target ...
    let doc = Parser::default()
        .with_intrinsic_attribute("imagesdir", "images", ModificationContext::Anywhere)
        .parse("image:sunset.jpg[Sunset]");
    assert!(rendered_paragraphs(&doc)[0].contains(r#"src="images/sunset.jpg""#));

    // ... but not to a URL or absolute target.
    let doc = Parser::default()
        .with_intrinsic_attribute("imagesdir", "images", ModificationContext::Anywhere)
        .parse("image:https://example.org/sunset.jpg[Sunset]");
    assert!(rendered_paragraphs(&doc)[0].contains(r#"src="https://example.org/sunset.jpg""#));

    // `image::` (two colons) is a block image (a standalone media block) ...
    let doc = Parser::default().parse("image::sunset.jpg[]");
    let Some(Block::Media(m)) = doc.nested_blocks().next() else {
        panic!("expected a media block");
    };
    assert_eq!(m.type_(), MediaType::Image);

    // ... whereas `image:` (one colon) is an inline image within a line of text.
    let doc = Parser::default().parse("Click image:play.png[] to start.");
    assert!(rendered_paragraphs(&doc)[0].contains(r#"<span class="image">"#));
}

#[test]
fn audio() {
    non_normative!(
        r#"
== Audio

"#
    );

    verifies!(
        r#"
.Block audio macro
----
include::macros:example$audio.adoc[tag=basic]

include::macros:example$audio.adoc[tag=attrs]
----

You can control the audio settings using xref:macros:audio-and-video.adoc[additional attributes and options] on the macro.

"#
    );

    // A block audio macro produces an audio media block.
    let doc = Parser::default().parse("audio::ocean-waves.wav[start=60,opts=autoplay]");
    let Some(Block::Media(m)) = doc.nested_blocks().next() else {
        panic!("expected a media block");
    };
    assert_eq!(m.type_(), MediaType::Audio);
    assert_eq!(m.target().unwrap().data(), "ocean-waves.wav");
}

#[test]
fn videos() {
    non_normative!(
        r#"
== Videos

"#
    );

    verifies!(
        r#"
.Block video macro
----
include::macros:example$video.adoc[tag=base]

include::macros:example$video.adoc[tag=attr]
----

.Embedded YouTube video
----
include::macros:example$video.adoc[tag=youtube]
----

.Embedded Vimeo video
----
include::macros:example$video.adoc[tag=vimeo]
----

You can control the video settings using xref:macros:audio-and-video.adoc[additional attributes and options] on the macro.

"#
    );

    // Local, YouTube, and Vimeo videos all produce a video media block.
    let doc = Parser::default().parse("video::video-file.mp4[]");
    let Some(Block::Media(m)) = doc.nested_blocks().next() else {
        panic!("expected a media block");
    };
    assert_eq!(m.type_(), MediaType::Video);

    let doc = Parser::default().parse("video::RvRhUHTV_8k[youtube]");
    let Some(Block::Media(m)) = doc.nested_blocks().next() else {
        panic!("expected a media block");
    };
    assert_eq!(m.type_(), MediaType::Video);
    assert_eq!(
        m.macro_attrlist().nth_attribute(1).unwrap().value(),
        "youtube"
    );
}

#[test]
fn keyboard_button_and_menu_macros() {
    non_normative!(
        r#"
== Keyboard, button, and menu macros

"#
    );

    verifies!(
        r#"
IMPORTANT: You must set the `experimental` attribute in the document header to enable these macros.

.Keyboard macro
[#ex-kbd]
----
include::macros:example$ui.adoc[tag=qr-key]
----

.View result of <<ex-kbd>>
[%collapsible.result]
====
include::macros:example$ui.adoc[tag=qr-key]
====

.Menu macro
[#ex-menu]
----
include::macros:example$ui.adoc[tag=menu]
----

.View result of <<ex-menu>>
[%collapsible.result]
====
include::macros:example$ui.adoc[tag=menu]
====

.Button macro
[#ex-button]
----
include::macros:example$ui.adoc[tag=button]
----

.View result of <<ex-button>>
[%collapsible.result]
====
include::macros:example$ui.adoc[tag=button]
====

"#
    );

    // The keyboard, button, and menu macros require the `experimental`
    // attribute; without it they are rendered as literal text.
    let doc = Parser::default().parse("Press kbd:[F11] now.");
    assert!(rendered_paragraphs(&doc)[0].contains("kbd:[F11]"));

    // With `:experimental:` set, the macro is processed.
    let doc = Parser::default().parse(":experimental:\n\nPress kbd:[F11] now.");
    assert!(rendered_paragraphs(&doc)[0].contains("<kbd>F11</kbd>"));

    // Button and menu macros (also gated on `experimental`).
    let doc =
        Parser::default().parse(":experimental:\n\nClick btn:[OK] then select menu:File[Save].");
    let r = &rendered_paragraphs(&doc)[0];
    assert!(r.contains(r#"<b class="button">OK</b>"#));
    assert!(r.contains("menu"));
}

#[test]
fn literals_and_source_code() {
    non_normative!(
        r#"
== Literals and source code

"#
    );

    non_normative!(
        r#"
////
.Inline monospace only
[#ex-inline-code]
----
include::text:example$text.adoc[tag=b-mono-code]
----

.View result of <<ex-inline-code>>
[%collapsible.result]
====
include::text:example$text.adoc[tag=b-mono-code]
====
////

"#
    );

    verifies!(
        r#"
.Inline literal monospace
[#ex-inline-literal]
----
include::pass:example$pass.adoc[tag=backtick-plus]
----

.View result of <<ex-inline-literal>>
[%collapsible.result]
====
include::pass:example$pass.adoc[tag=backtick-plus]
====

.Literal paragraph
[#ex-literal-line]
----
include::verbatim:example$literal.adoc[tag=b-imp-code]
----

.View result of <<ex-literal-line>>
[%collapsible.result]
====
include::verbatim:example$literal.adoc[tag=b-imp-code]
====

.Literal block
[#ex-literal-block]
----
include::verbatim:example$literal.adoc[tag=b-block]
----

.View result of <<ex-literal-block>>
[%collapsible.result]
====
include::verbatim:example$literal.adoc[tag=b-block]
====

.Listing block with title
[#ex-listing]
------
include::verbatim:example$listing.adoc[tag=qr-listing]
------

.View result of <<ex-listing>>
[%collapsible.result]
====
[caption="Listing 1. "]
[listing]
include::verbatim:example$listing.adoc[tag=qr-listing]
====

.Source block with title and syntax highlighting
[#ex-highlight]
------
.Some Ruby code
include::verbatim:example$source.adoc[tag=src-base]
------

.View result of <<ex-highlight>>
[%collapsible.result]
====
[caption="Listing 1. "]
.Some Ruby code
include::verbatim:example$source.adoc[tag=src-base]
====

"#
    );

    non_normative!(
        r#"
[IMPORTANT]
====
You must enable xref:verbatim:source-highlighter.adoc[source highlighting] by setting the `source-highlighter` attribute in the document header, CLI, or API.

----
:source-highlighter: rouge
----

See xref:asciidoctor:syntax-highlighting:index.adoc[] to learn which values are accepted when using Asciidoctor.
====

"#
    );

    verifies!(
        r#"
.Source block with callouts
[#ex-callouts,subs=-callouts]
------
include::verbatim:example$callout.adoc[tag=b-src]
------

.View result of <<ex-callouts>>
[%collapsible.result]
====
include::verbatim:example$callout.adoc[tag=b-src]
====

.Make callouts non-selectable
[#ex-hide-callouts,subs=-callouts]
------
include::verbatim:example$callout.adoc[tag=b-nonselect]
------

.View result of <<ex-hide-callouts>>
[%collapsible.result]
====
include::verbatim:example$callout.adoc[tag=b-nonselect]
====

.Source block content included from a file
------
include::verbatim:example$source.adoc[tag=src-inc]
------

.Source block content included from file relative to source directory
------
include::verbatim:example$source.adoc[tag=rel]
------

.Strip leading indentation from partial file content
------
include::verbatim:example$source.adoc[tag=ind]
------

[NOTE]
====
The xref:directives:include-with-indent.adoc[indent attribute] is frequently used when including source code by xref:directives:include-tagged-regions.adoc[tagged region] or xref:directives:include-lines.adoc[lines].
It can be specified on the include directive itself or the enclosing literal, listing, or source block.

When indent is 0, the leading block indent is stripped.

When indent is greater than 0, the leading block indent is first stripped, then a block is indented by the number of columns equal to this value.
====

.Source paragraph (no empty lines)
[#ex-source-para]
----
include::verbatim:example$source.adoc[tag=src-para]
----

.View result of <<ex-source-para>>
[%collapsible.result]
====
include::verbatim:example$source.adoc[tag=src-para]
====

"#
    );

    // Inline literal monospace via backtick-plus.
    let doc = Parser::default().parse("Use `+{name}+` literally.");
    assert!(rendered_paragraphs(&doc)[0].contains("<code>{name}</code>"));

    // A delimited literal block is verbatim (formatting marks are not applied).
    let doc = Parser::default().parse("....\n*not bold*\n....");
    let Some(Block::RawDelimited(b)) = doc.nested_blocks().next() else {
        panic!("expected a raw delimited block");
    };
    assert!(b.content().rendered().contains("*not bold*"));

    // A source block parses as a listing-context raw delimited block.
    let doc = Parser::default().parse("[source,ruby]\n----\nputs 'hi'\n----");
    let Some(Block::RawDelimited(b)) = doc.nested_blocks().next() else {
        panic!("expected a raw delimited block");
    };
    assert!(b.content().rendered().contains("puts 'hi'"));

    // Source callouts render conums.
    let doc = Parser::default()
        .parse("[source,ruby]\n----\nputs 'hi' # <1>\n----\n<1> Prints a greeting.\n");
    assert_output_contains(&doc, r#"<b class="conum">(1)</b>"#);

    // A `[source,ruby]` paragraph (no delimiters) is a Source-styled simple block.
    let doc = Parser::default().parse("[source,ruby]\nputs 'hi'");
    let Some(Block::Simple(sb)) = doc.nested_blocks().next() else {
        panic!("expected a simple block");
    };
    assert_eq!(sb.style(), SimpleBlockStyle::Source);
}

#[test]
fn admonitions() {
    non_normative!(
        r#"
== Admonitions

"#
    );

    verifies!(
        r#"
.Admonition paragraph
[#ex-admon-para]
----
include::blocks:example$admonition.adoc[tag=b-para]
----

.View result of <<ex-admon-para>>
[%collapsible.result]
====
include::blocks:example$admonition.adoc[tag=b-para]
====

.Admonition block
[#ex-admon-block]
----
include::blocks:example$admonition.adoc[tag=b-bl]
----

.View result of <<ex-admon-block>>
[%collapsible.result]
=====
include::blocks:example$admonition.adoc[tag=b-bl]
=====

"#
    );

    // An admonition paragraph carries the admonition context and its style.
    let doc = Parser::default().parse("NOTE: Pay attention.");
    let b = doc.nested_blocks().next().unwrap();
    assert_eq!(b.resolved_context().as_ref(), "admonition");
    assert_eq!(b.declared_style(), Some("NOTE"));

    // An admonition block (delimited, compound content).
    let doc = Parser::default().parse("[IMPORTANT]\n====\nCompound content.\n====");
    assert_css(&doc, ".admonitionblock.important", 1);
}

#[test]
fn more_delimited_blocks() {
    non_normative!(
        r#"
== More delimited blocks

"#
    );

    verifies!(
        r#"
Any block can have a title.
A block title is defined using a line of text above the block that starts with a dot.
That dot cannot be followed by a space.
For block images, the title is displayed below the block.
For all other blocks, the title is typically displayed above it.

.Sidebar block
[#ex-sidebar]
----
include::blocks:example$sidebar.adoc[tag=delimited]
----

.View result of <<ex-sidebar>>
[%collapsible.result]
====
include::blocks:example$sidebar.adoc[tag=delimited]
====

.Example block
[#ex-example]
------
include::blocks:example$example.adoc[tag=base]
------

.View result of <<ex-example>>
[example%collapsible.result]
--
include::blocks:example$example.adoc[tag=base]
--

.Blockquotes
[#ex-quotes]
----
include::blocks:example$quote.adoc[tag=bl]

include::blocks:example$quote.adoc[tag=para]

include::blocks:example$quote.adoc[tag=no-cite]

include::blocks:example$quote.adoc[tag=link-text]

include::blocks:example$quote.adoc[tag=abbr]
----

.View result of <<ex-quotes>>
[%collapsible.result]
====
include::blocks:example$quote.adoc[tag=bl]

include::blocks:example$quote.adoc[tag=para]

include::blocks:example$quote.adoc[tag=no-cite]

include::blocks:example$quote.adoc[tag=link-text]

include::blocks:example$quote.adoc[tag=abbr]
====

.Open blocks
[#ex-open]
----
include::blocks:example$open.adoc[tag=base]

include::blocks:example$open.adoc[tag=src]
----

.View result of <<ex-open>>
[%collapsible.result]
====
include::blocks:example$open.adoc[tag=base]

include::blocks:example$open.adoc[tag=src]
====

.Passthrough block
[#ex-pass-block]
----
include::pass:example$pass.adoc[tag=b-bl]
----

.View result of <<ex-pass-block>>
[%collapsible.result]
====
include::pass:example$pass.adoc[tag=b-bl]
====

.Customize block substitutions
[#ex-block-subs,subs=+macros]
------
include::verbatim:example$listing.adoc[tag=subs]
------

.View result of <<ex-block-subs>>
[%collapsible.result]
====
include::verbatim:example$listing.adoc[tag=subs-out]
====

"#
    );

    // Any block can have a title, set by a line above it that starts with a dot
    // (the dot must not be followed by a space).
    let doc = Parser::default().parse(".Terminal Output\n....\ncode\n....");
    assert_eq!(
        doc.nested_blocks().next().unwrap().title(),
        Some("Terminal Output")
    );

    // A dot followed by a space is an ordered-list marker, not a block title.
    let doc = Parser::default().parse(". an item\n. another item");
    assert!(matches!(doc.nested_blocks().next(), Some(Block::List(_))));

    // Sidebar, example, open, and passthrough blocks.
    let doc = Parser::default().parse("****\nAn aside.\n****");
    assert_eq!(
        doc.nested_blocks()
            .next()
            .unwrap()
            .resolved_context()
            .as_ref(),
        "sidebar"
    );

    let doc = Parser::default().parse("====\nAn example.\n====");
    assert_eq!(
        doc.nested_blocks()
            .next()
            .unwrap()
            .resolved_context()
            .as_ref(),
        "example"
    );

    let doc = Parser::default().parse("--\nGeneric content.\n--");
    assert_eq!(
        doc.nested_blocks()
            .next()
            .unwrap()
            .resolved_context()
            .as_ref(),
        "open"
    );

    let doc = Parser::default().parse("++++\n<b>raw</b>\n++++");
    let Some(Block::RawDelimited(b)) = doc.nested_blocks().next() else {
        panic!("expected a raw delimited block");
    };
    assert!(b.content().rendered().contains("<b>raw</b>"));

    // A blockquote with attribution and citation.
    let doc = Parser::default().parse(
        "[quote,Abraham Lincoln,Gettysburg Address]\n____\nFour score and seven years ago...\n____",
    );
    let Some(Block::Quote(q)) = doc.nested_blocks().next() else {
        panic!("expected a quote block");
    };
    assert_eq!(q.attribution(), Some("Abraham Lincoln"));
    assert_eq!(q.citetitle(), Some("Gettysburg Address"));
}

#[test]
fn tables() {
    non_normative!(
        r#"
== Tables

"#
    );

    verifies!(
        r#"
.Table with a title, two columns, a header row, and two rows of content
[#ex-header-row]
----
include::tables:example$table.adoc[tag=b-base-h-co]
----
<.> Unless the `cols` attribute is specified, the number of columns is equal to the number of cell separators on the first (non-empty) line.
<.> When an empty line immediately follows a non-empty line at the start of the table, the cells in the first line get promoted to the table header.

.View result of <<ex-header-row>>
[%collapsible.result]
====
[caption="Table 1. "]
include::tables:example$table.adoc[tag=b-base-h]
====

.Table with two columns, a header row, and two rows of content
[#ex-cols]
----
include::tables:example$table.adoc[tag=b-col-h-co]
----
<.> The `+*+` in the `cols` attribute is the repeat operator.
It means repeat the column specification across the remaining columns.
In this case, we are repeating the default formatting across 2 columns.
When the cells in the header are not defined on a single line, you must use the `cols` attribute to set the number of columns in the table and the `%header` option (or `options=header` attribute) to promote the first row to the table header.

.View result of <<ex-cols>>
[%collapsible.result]
====
include::tables:example$table.adoc[tag=b-col-h]
====

.Table with three columns, a header row, and two rows of content
[#ex-cols-widths]
----
include::tables:example$table.adoc[tag=b-col-indv-co]
----
<.> In this example, the `cols` attribute has two functions.
It specifies that this table has three columns, and it sets their relative widths.

.View result of <<ex-cols-widths>>
[%collapsible.result]
====
[caption="Table 1. "]
include::tables:example$table.adoc[tag=b-col-indv]
====

.Table with column containing AsciiDoc content
[#ex-table-adoc]
----
include::tables:example$table.adoc[tag=b-col-a]
----

.View result of <<ex-table-adoc>>
[%collapsible.result]
====
include::tables:example$table.adoc[tag=b-col-a]
====

.Table from CSV data using shorthand
[#ex-csv]
----
include::tables:example$data.adoc[tag=s-csv]
----

.View result of <<ex-csv>>
[%collapsible.result]
====
include::tables:example$data.adoc[tag=s-csv]
====

.Table from CSV data
[#ex-csv-formal]
----
include::tables:example$data.adoc[tag=csv]
----

.View result of <<ex-csv-formal>>
[%collapsible.result]
====
include::tables:example$data.adoc[tag=csv]
====

.Table from CSV data included from file
[#ex-csv-include]
----
include::tables:example$data.adoc[tag=i-csv]
----

.Table from DSV data using shorthand
[#ex-dsv]
----
include::tables:example$data.adoc[tag=s-dsv]
----

.View result of <<ex-dsv>>
[%collapsible.result]
====
include::tables:example$data.adoc[tag=s-dsv]
====

.Table with formatted, aligned and merged cells
[#ex-cell-format]
----
include::tables:example$cell.adoc[tag=b-spec]
----

.View result of <<ex-cell-format>>
[%collapsible.result]
====
include::tables:example$cell.adoc[tag=b-spec]
====

"#
    );

    // Without `cols`, the column count equals the number of cell separators on
    // the first non-empty line.
    let doc = Parser::default().parse("|===\n|a |b |c\n|d |e |f\n|===");
    let Some(Block::Table(t)) = doc.nested_blocks().next() else {
        panic!("expected a table");
    };
    assert_eq!(t.columns().len(), 3);

    // An empty line immediately after the first non-empty line promotes the
    // first row to the table header.
    let doc = Parser::default().parse("|===\n|H1 |H2\n\n|a |b\n|===");
    let Some(Block::Table(t)) = doc.nested_blocks().next() else {
        panic!("expected a table");
    };
    assert!(t.header_row().is_some());

    // The `*` repeat operator expands a column spec across the columns ...
    let doc = Parser::default().parse("[cols=\"2*\"]\n|===\n|a |b\n|===");
    let Some(Block::Table(t)) = doc.nested_blocks().next() else {
        panic!("expected a table");
    };
    assert_eq!(t.columns().len(), 2);

    // ... and the `%header` option promotes the first row when the header cells
    // are not on a single line.
    let doc = Parser::default().parse("[%header,cols=\"2*\"]\n|===\n|H1\n|H2\n\n|a\n|b\n|===");
    let Some(Block::Table(t)) = doc.nested_blocks().next() else {
        panic!("expected a table");
    };
    assert!(t.header_row().is_some());

    // The `cols` attribute both sets the number of columns and their relative
    // widths.
    let doc = Parser::default().parse("[cols=\"1,1,2\"]\n|===\n|A |B |C\n|===");
    let Some(Block::Table(t)) = doc.nested_blocks().next() else {
        panic!("expected a table");
    };
    assert_eq!(t.columns().len(), 3);
    assert_eq!(t.columns()[2].width(), 2);

    // A CSV table using the shorthand delimiter.
    let doc =
        Parser::default().parse(",===\nArtist,Track,Genre\n\nBaauer,Harlem Shake,Hip Hop\n,===");
    let Some(Block::Table(t)) = doc.nested_blocks().next() else {
        panic!("expected a table");
    };
    assert_eq!(t.data_format(), DataFormat::Csv);

    // A DSV table using the shorthand delimiter.
    let doc =
        Parser::default().parse(":===\nArtist:Track:Genre\n\nRobyn:Indestructible:Dance\n:===");
    let Some(Block::Table(t)) = doc.nested_blocks().next() else {
        panic!("expected a table");
    };
    assert_eq!(t.data_format(), DataFormat::Dsv);

    // A cell may hold AsciiDoc block content via the `a` cell style.
    let doc = Parser::default().parse("|===\n|Normal |AsciiDoc\n\n|* not a list\na|* a list\n|===");
    let Some(Block::Table(t)) = doc.nested_blocks().next() else {
        panic!("expected a table");
    };
    assert!(matches!(
        t.body_rows()[0].cells()[1].content(),
        crate::blocks::TableCellContent::AsciiDoc(_)
    ));
}

#[test]
fn ids_roles_and_options() {
    non_normative!(
        r#"
== IDs, roles, and options

"#
    );

    verifies!(
        r#"
.Shorthand method for assigning block ID (anchor) and role
----
[#goals.incremental]
* Goal 1
* Goal 2
----

[TIP]
====
* To specify multiple roles using the shorthand syntax, delimit them by dots.
* The order of `id` and `role` values in the shorthand syntax does not matter.
====

.Formal method for assigning block ID (anchor) and role
----
[id="goals",role="incremental"]
* Goal 1
* Goal 2
----

.Explicit section ID (anchor)
----
[#null-values]
== Primitive types and null values
----

.Assign ID (anchor) and role to inline formatted text
----
[#id-name.role-name]`monospace text`

[#free-world.goals]*free the world*
----

.Shorthand method for assigning block options
----
[%header%footer%autowidth]
|===
|Header A |Header B
|Footer A |Footer B
|===
----

.Formal method for assigning block options
----
[options="header,footer,autowidth"]
|===
|Header A |Header B
|Footer A |Footer B
|===

// options can be shorted to opts
[opts="header,footer,autowidth"]
|===
|Header A |Header B
|Footer A |Footer B
|===
----

"#
    );

    // Shorthand block ID and role.
    let doc = Parser::default().parse("[#goals.incremental]\n* Goal 1\n* Goal 2");
    let list = doc.nested_blocks().next().unwrap();
    assert_eq!(list.id(), Some("goals"));
    assert!(list.roles().contains(&"incremental"));

    // Shorthand block options on a table.
    let doc = Parser::default().parse("[%header%footer]\n|===\n|A |B\n\n|c |d\n\n|e |f\n|===");
    let t = doc.nested_blocks().next().unwrap();
    assert!(t.has_option("header"));
    assert!(t.has_option("footer"));

    // Inline shorthand ID and role on formatted text.
    let doc = Parser::default().parse("[#free-world.goals]*free the world*");
    assert!(rendered_paragraphs(&doc)[0].contains(r#"id="free-world""#));
}

#[test]
fn comments() {
    non_normative!(
        r#"
== Comments

"#
    );

    verifies!(
        r#"
.Line and block comments
----
// A single-line comment

////
A multi-line comment.

Notice it's a delimited block.
////
----

"#
    );

    // A line comment is dropped from the rendered output (but line numbering is
    // preserved).
    let doc = Parser::default().parse("before\n// a comment\nafter");
    assert_eq!(rendered_paragraphs(&doc), vec!["before\nafter".to_string()]);
}

#[test]
fn breaks() {
    non_normative!(
        r#"
== Breaks

"#
    );

    verifies!(
        r#"
.Thematic break (aka horizontal rule)
[#ex-thematic]
----
before

'''

after
----

.View result of <<ex-thematic>>
[%collapsible.result]
====
before

'''

after
====

.Page break
----
<<<
----

"#
    );

    // A thematic break and a page break.
    assert_eq!(
        first_break(&Parser::default().parse("'''")).type_(),
        BreakType::Thematic
    );
    assert_eq!(
        first_break(&Parser::default().parse("<<<")).type_(),
        BreakType::Page
    );
}

#[test]
fn attributes_and_substitutions() {
    non_normative!(
        r#"
== Attributes and substitutions

"#
    );

    verifies!(
        r#"
.Attribute declaration and usage
[#ex-attributes]
----
:url-home: https://asciidoctor.org
:link-docs: https://asciidoctor.org/docs[documentation]
:summary: AsciiDoc is a mature, plain-text document format for \
       writing notes, articles, documentation, books, and more. \
       It's also a text processor & toolchain for translating \
       documents into various output formats (i.e., backends), \
       including HTML, DocBook, PDF and ePub.
:checkedbox: pass:normal[{startsb}&#10004;{endsb}]

Check out {url-home}[Asciidoctor]!

{summary}

Be sure to read the {link-docs} too!

{checkedbox} That's done!
----

.View result of <<ex-attributes>>
[%collapsible.result]
====
// I have to use a nested doc hack here, otherwise the attributes won't resolve
[.unstyled]
|===
a|
:url-home: https://asciidoctor.org
:link-docs: https://asciidoctor.org/docs[documentation]
:summary: AsciiDoc is a mature, plain-text document format for \
       writing notes, articles, documentation, books, and more. \
       It's also a text processor & toolchain for translating \
       documents into various output formats (i.e., backends), \
       including HTML, DocBook, PDF and ePub.
:checkedbox: pass:normal[{startsb}&#10004;{endsb}]

Check out {url-home}[Asciidoctor]!

{summary}

Be sure to read the {link-docs} too!

{checkedbox} That's done!
|===
====

To learn more about the available attributes and substitution groups see:

* xref:attributes:document-attributes-ref.adoc[]
* xref:attributes:character-replacement-ref.adoc[]
* xref:subs:apply-subs-to-blocks.adoc#subs-groups[Substitution Groups]

.Counter attributes
[#ex-counter]
----
include::attributes:example$counter.adoc[tag=base]
----

.View result of <<ex-counter>>
[%collapsible.result]
====
[caption="Table 1. "]
include::attributes:example$counter.adoc[tag=base]
====

"#
    );

    // An attribute entry is declared and then referenced.
    let mut parser = Parser::default();
    let doc = parser.parse(":name: Ada\n\nHello {name}.");
    assert_eq!(rendered_paragraphs(&doc)[0], "Hello Ada.");

    // Counter attributes increment on each reference.
    let doc = Parser::default().parse("{counter:seq}, {counter:seq}, {counter:seq}.");
    assert_eq!(rendered_paragraphs(&doc)[0], "1, 2, 3.");
}

#[test]
fn text_replacements() {
    non_normative!(
        r#"
== Text replacements

"#
    );

    verifies!(
        r#"
[frame=none,grid=rows]
include::subs:partial$subs-symbol-repl.adoc[]

Any named, numeric or hexadecimal {url-char-xml}[XML character reference^] is supported.

"#
    );

    // Named, numeric, and hexadecimal XML character references are all preserved
    // in the output.
    let doc = Parser::default().parse("Named &sect;, decimal &#167;, hex &#xA9; stay.");
    assert_eq!(
        rendered_paragraphs(&doc)[0],
        "Named &sect;, decimal &#167;, hex &#xA9; stay."
    );

    // Symbol replacements are applied.
    let doc = Parser::default().parse("(C) (R) (TM) then -> and <-");
    let r = &rendered_paragraphs(&doc)[0];
    assert!(r.contains("&#169;"));
    assert!(r.contains("&#174;"));
    assert!(r.contains("&#8482;"));
    assert!(r.contains("&#8594;"));
    assert!(r.contains("&#8592;"));
}

#[test]
fn escaping_substitutions() {
    non_normative!(
        r#"
== Escaping substitutions

"#
    );

    verifies!(
        r#"
.Backslash
[#ex-slash]
----
include::subs:example$subs.adoc[tag=backslash]
----

.View result of <<ex-slash>>
[%collapsible.result]
====
include::subs:example$subs.adoc[tag=backslash]
====

.Single and double plus inline passthroughs
[#ex-single-plus]
----
include::pass:example$pass.adoc[tag=plus]
----

.View result of <<ex-single-plus>>
[%collapsible.result]
====
include::pass:example$pass.adoc[tag=plus]
====

.Triple plus inline passthrough and inline pass macro
[#ex-inline-pass]
----
include::pass:example$pass.adoc[tag=b-3p-macro]
----

.View result of <<ex-inline-pass>>
[%collapsible.result]
====
include::pass:example$pass.adoc[tag=b-3p-macro]
====

"#
    );

    // A backslash prevents a substitution (here, the smart apostrophe).
    let doc = Parser::default().parse(r"Olaf\'s desk");
    assert_eq!(rendered_paragraphs(&doc)[0], "Olaf's desk");

    // A single-plus passthrough suppresses formatting substitutions.
    let doc = Parser::default().parse("+/user/{id}+");
    assert_eq!(rendered_paragraphs(&doc)[0], "/user/{id}");

    // The inline pass macro emits raw output.
    let doc = Parser::default().parse("pass:[<del>x</del>]");
    assert_eq!(rendered_paragraphs(&doc)[0], "<del>x</del>");
}

#[test]
fn bibliography() {
    non_normative!(
        r#"
== Bibliography

"#
    );

    verifies!(
        r#"
.Bibliography with inbound references
[#ex-biblio]
----
include::sections:example$bibliography.adoc[tag=base]
----

.View result of <<ex-biblio>>
[%collapsible.result]
====
|===
a|
include::sections:example$bibliography.adoc[tag=base]
|===
====

"#
    );

    // A bibliography anchor becomes a labeled target and an inbound reference
    // links to it.
    let doc = Parser::default()
        .parse("See <<pp>>.\n\n[bibliography]\n== References\n\n* [[[pp]]] Andy Hunt & Dave Thomas. 1999.\n");
    let paras = rendered_paragraphs(&doc);
    assert!(paras.iter().any(|p| p.contains(r##"href="#pp""##)));
    assert!(
        paras
            .iter()
            .any(|p| p.contains(r#"id="pp""#) && p.contains("[pp]"))
    );
}

#[test]
fn footnotes() {
    non_normative!(
        r#"
[#section-footnotes]
== Footnotes

"#
    );

    verifies!(
        r#"
.Normal and reusable footnotes
[#ex-footnotes]
----
include::macros:example$footnote.adoc[tag=base]
----

.View result of <<ex-footnotes>>
[%collapsible.result]
====
[.unstyled]
|===
a|
include::macros:example$footnote.adoc[tag=base]
|===
====

"#
    );

    // A footnote is registered in the catalog; a reusable footnote does not add
    // a second entry.
    let doc = Parser::default().parse("A statement.footnote:[A clarification.]");
    assert_eq!(doc.catalog().footnotes().len(), 1);

    let doc = Parser::default().parse("First.footnote:d1[Shared.] Again.footnote:d1[]");
    assert_eq!(doc.catalog().footnotes().len(), 1);
}

#[test]
fn markdown_compatibility() {
    non_normative!(
        r#"
[#markdown-compatibility]
== Markdown compatibility

"#
    );

    verifies!(
        r#"
Markdown compatible syntax is an optional feature of the AsciiDoc language and is currently only available when using Asciidoctor.

.Markdown-style headings
[#ex-md-headings]
----
include::sections:example$section.adoc[tag=md]
----

.View result of <<ex-md-headings>>
[%collapsible.result]
====
include::sections:example$section.adoc[tag=b-md]
====

"#
    );

    verifies!(
        r#"
.Fenced code block with syntax highlighting
[#ex-fenced]
----
include::verbatim:example$source.adoc[tag=fence]
----

.View result of <<ex-fenced>>
[%collapsible.result]
====
include::verbatim:example$source.adoc[tag=fence]
====
"#
    );

    // A language on the opening fence line (```ruby) is recognized as a source
    // listing block: equivalent to `[source,ruby]` over a listing block
    // (issue #615). The language is recorded as the second positional attribute
    // so a downstream renderer can highlight it; this parser performs no
    // highlighting itself. (The bare ``` fence is also supported; see the
    // bare-fence assertion at the end of this function.)
    let doc = Parser::default().parse("```ruby\nputs 'hi'\n```");
    let Some(Block::RawDelimited(b)) = doc.nested_blocks().next() else {
        panic!("expected a raw delimited (source listing) block");
    };
    assert_eq!(b.resolved_context().as_ref(), "listing");
    assert_eq!(b.declared_style(), Some("source"));
    assert_eq!(
        b.attrlist()
            .and_then(|a| a.nth_attribute(2))
            .map(|a| a.value()),
        Some("ruby")
    );
    assert!(b.content().rendered().contains("puts 'hi'"));

    verifies!(
        r#"

.Markdown-style blockquote
[#ex-md-quote]
----
include::blocks:example$quote.adoc[tag=md]
----

.View result of <<ex-md-quote>>
[%collapsible.result]
====
include::blocks:example$quote.adoc[tag=md]
====

.Markdown-style blockquote with block content
[#ex-md-blockquote]
----
include::blocks:example$quote.adoc[tag=md-alt]
----

.View result of <<ex-md-blockquote>>
[%collapsible.result]
====
include::blocks:example$quote.adoc[tag=md-alt]
====

.Markdown-style thematic breaks
[#ex-md-breaks]
----
---

- - -

***

* * *
----

.View result of <<ex-md-breaks>>
[%collapsible.result]
====
---

- - -

***

* * *
====


"#
    );

    non_normative!(
        r#"
////
Possible change for future to `%collapsible` blocks

.Normal
----
Paragraphs don't require any special markup in AsciiDoc.
A paragraph is just one or more lines of consecutive text.

To begin a new paragraph, separate it by at least one empty line.
Line breaks within a paragraph are not displayed.
----

.View Result (Normal)
[%collapsible.result]
====
Paragraphs don't require any special markup in AsciiDoc.
A paragraph is just one or more lines of consecutive text.

To begin a new paragraph, separate it by at least one empty line.
Line breaks within a paragraph are not displayed.
====

'''

.Normal
[tabs]
====
Source::
+
----
Paragraphs don't require any special markup in AsciiDoc.
A paragraph is just one or more lines of consecutive text.

To begin a new paragraph, separate it by at least one empty line.
Line breaks within a paragraph are not displayed.
----

Output::
+
--
Paragraphs don't require any special markup in AsciiDoc.
A paragraph is just one or more lines of consecutive text.

To begin a new paragraph, separate it by at least one empty line.
Line breaks within a paragraph are not displayed.
--
====
////
"#
    );

    // Markdown-style (ATX) headings become sections.
    let doc = Parser::default().parse("= Document Title\n\n## Section Level 1\n\ntext");
    assert!(doc.nested_blocks().any(|b| matches!(b, Block::Section(_))));

    // Markdown-style thematic breaks.
    for src in ["---", "- - -", "***", "* * *"] {
        assert_eq!(
            first_break(&Parser::default().parse(src)).type_(),
            BreakType::Thematic
        );
    }

    // Markdown-style blockquote.
    let doc = Parser::default()
        .parse("> I hold it that a little rebellion is a good thing.\n> -- Thomas Jefferson");
    assert!(matches!(doc.nested_blocks().next(), Some(Block::Quote(_))));

    // A bare Markdown-style fenced code block (```) is supported: the fence
    // opens a verbatim listing block (#599 / #613).
    let doc = Parser::default().parse("```\nputs 'hi'\n```");
    let Some(Block::RawDelimited(b)) = doc.nested_blocks().next() else {
        panic!("expected a raw delimited (listing) block");
    };
    assert_eq!(b.resolved_context().as_ref(), "listing");
    assert!(b.content().rendered().contains("puts 'hi'"));
}
