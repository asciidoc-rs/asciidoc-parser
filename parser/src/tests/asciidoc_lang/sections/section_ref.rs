use crate::tests::prelude::*;

track_file!("docs/modules/sections/pages/section-ref.adoc");

// This page is a reference summary of the section-related document attributes
// and the special-section styles. Each row links to a dedicated page where the
// behavior is specified (and separately covered); here we verify the
// parser-observable claims this table makes: the documented default attribute
// values, which attributes are set by default, and that each listed style is
// recognized as a section's declared style. Rows that describe `book`-doctype
// behavior, Asciidoctor PDF or DocBook output, or pure rendering (section
// anchors and links) are non-normative for this parser.

/// Returns the first top-level section block in `doc`, panicking if there
/// isn't one.
fn first_section<'src>(
    doc: &'src crate::Document<'src>,
) -> &'src crate::blocks::SectionBlock<'src> {
    doc.nested_blocks()
        .find_map(|b| match b {
            crate::blocks::Block::Section(s) => Some(s),
            _ => None,
        })
        .expect("expected a section block")
}

non_normative!(
    r#"
= Section Attributes and Styles Reference

== Section attributes

[%autowidth]
|===
|Feature |Attribute |Value(s) |Notes

"#
);

#[test]
fn appendix_caption_attribute() {
    verifies!(
        r#"
|xref:appendix.adoc#caption[Appendix label]
|`appendix-caption`
|_Appendix_ (default) or user defined text
|

"#
    );

    // `appendix-caption` defaults to "Appendix", so the first appendix is
    // labeled "Appendix A: ".
    let doc = Parser::default().parse("[appendix]\n== Data Access Matrix");
    let captions: Vec<Option<&str>> = doc.nested_blocks().map(|b| b.caption()).collect();
    assert_eq!(captions, vec![Some("Appendix A: ")]);

    // A user-defined value overrides the label.
    let doc =
        Parser::default().parse(":appendix-caption: Exhibit\n\n[appendix]\n== Data Access Matrix");
    let captions: Vec<Option<&str>> = doc.nested_blocks().map(|b| b.caption()).collect();
    assert_eq!(captions, vec![Some("Exhibit A: ")]);
}

// `chapter-signifier` only applies in the `book` doctype (and its default is
// only set by Asciidoctor PDF), neither of which this parser models.
non_normative!(
    r#"
|xref:chapters.adoc#chapter-signifier[Chapter signifier]
|`chapter-signifier`
|_Chapter_ (default) or user defined text
|`book` doctype only; default only set in Asciidoctor PDF

"#
);

#[test]
fn discrete_attribute() {
    verifies!(
        r#"
|xref:discrete-headings.adoc[Discrete heading]
|`discrete`
|NA
|

"#
    );

    // The `discrete` style demotes a section title to a free-standing heading
    // that is not part of the section hierarchy.
    let doc = Parser::default().parse("[discrete]\n== Discrete Heading");
    let section = first_section(&doc);
    assert_eq!(section.declared_style(), Some("discrete"));
    assert_eq!(section.section_type(), SectionType::Discrete);
}

#[test]
fn idprefix_attribute() {
    verifies!(
        r#"
|xref:id-prefix-and-separator.adoc#prefix[ID prefix]
|`idprefix`
|`_` (default) or user defined text
|Set by default
//Set to prepend string to generated section ID

"#
    );

    // The default `idprefix` is `_`, so it is prepended to every generated ID.
    let doc = Parser::default().parse("== Section Title");
    assert_eq!(first_section(&doc).section_id(), Some("_section_title"));

    // A user-defined prefix replaces it.
    let doc = Parser::default()
        .with_intrinsic_attribute("idprefix", "~", ModificationContext::Anywhere)
        .parse("== Section Title");
    assert_eq!(first_section(&doc).section_id(), Some("~section_title"));
}

#[test]
fn idseparator_attribute() {
    verifies!(
        r#"
|xref:id-prefix-and-separator.adoc#separator[ID word separator]
|`idseparator`
|`_` (default) or user defined character
|Set by default
//Set to insert character between words in generated section ID

"#
    );

    // The default `idseparator` is `_`, so spaces, hyphens, and periods between
    // words become underscores.
    let doc = Parser::default().parse("== Extra-long section.title");
    assert_eq!(
        first_section(&doc).section_id(),
        Some("_extra_long_section_title")
    );

    // A user-defined separator replaces it.
    let doc = Parser::default()
        .with_intrinsic_attribute("idseparator", ".", ModificationContext::Anywhere)
        .parse("== Extra-long section.title");
    assert_eq!(
        first_section(&doc).section_id(),
        Some("_extra.long.section.title")
    );
}

// `part-signifier` and `partnums` only apply to the `book` doctype, which this
// parser does not model. Section anchors and links (`sectanchors`/`sectlinks`)
// are pure rendering concerns and produce no parser-observable change.
non_normative!(
    r#"
|xref:part-numbers-and-labels.adoc#part-signifier[Part signifier]
|`part-signifier`
|_Part_ (default) or user defined text
|`book` doctype only

|xref:part-numbers-and-labels.adoc[Part numbers]
|`partnums`
|NA
|`book` doctype only

|xref:title-links.adoc#anchor[Section anchor]
|`sectanchors`
|NA
|

"#
);

#[test]
fn sectids_attribute() {
    verifies!(
        r#"
|xref:auto-ids.adoc[Section ID]
|`sectids`
|NA
|Set by default
//Autogenerates section IDs by default

"#
    );

    // `sectids` is set by default, so sections get an autogenerated ID.
    let doc = Parser::default().parse("== Section Title");
    assert_eq!(first_section(&doc).section_id(), Some("_section_title"));

    // Unsetting `sectids` disables ID generation.
    let doc = Parser::default().parse(":!sectids:\n\n== Section Title");
    assert_eq!(first_section(&doc).section_id(), None);
}

// Section links are a pure rendering concern with no parser-observable change.
non_normative!(
    r#"
|xref:title-links.adoc#link[Section link]
|`sectlinks`
|NA
|

"#
);

#[test]
fn sectnums_attribute() {
    verifies!(
        r#"
|xref:numbers.adoc[Section numbers] and xref:special-section-numbers.adoc[special section numbers]
|`sectnums`
|empty (default) or `all`
|Can be toggled on or off in the flow of the document
// replaces numbered in AsciiDoc.py

"#
    );

    // Not set by default: sections aren't numbered.
    let doc = Parser::default().parse("== Section");
    assert!(first_section(&doc).section_number().is_none());

    // Setting `sectnums` (empty value) numbers the sections.
    let doc = Parser::default().parse(":sectnums:\n\n== Section");
    assert!(first_section(&doc).section_number().is_some());

    // The `all` value is also accepted and enables numbering.
    let doc = Parser::default().parse(":sectnums: all\n\n== Section");
    assert!(first_section(&doc).section_number().is_some());

    // `sectnums` can be toggled on and off in the flow of the document.
    let doc = Parser::default().parse(":sectnums:\n\n== One\n\n:sectnums!:\n\n== Two");
    let numbers: Vec<bool> = doc
        .nested_blocks()
        .filter_map(|b| match b {
            crate::blocks::Block::Section(s) => Some(s.section_number().is_some()),
            _ => None,
        })
        .collect();
    assert_eq!(numbers, vec![true, false]);
}

#[test]
fn sectnumlevels_attribute() {
    verifies!(
        r#"
|xref:numbers.adoc#numlevels[Section numbers level depth]
|`sectnumlevels`
|`3` (default) when `sectnums` is set; 0 through 5
|Value of `0` is the same as `sectnums!`

"#
    );

    // With `sectnums` set, levels 1 through 3 are numbered by default
    // (`sectnumlevels` defaults to 3), so a level 4 section is not numbered.
    let doc = Parser::default().parse(":sectnums:\n\n== L1\n\n=== L2\n\n==== L3\n\n===== L4");
    let numbered: Vec<bool> = collect_section_numbering(&doc);
    assert_eq!(numbered, vec![true, true, true, false]);

    // Reducing `sectnumlevels` to 2 stops level 3 (and deeper) from being
    // numbered.
    let doc =
        Parser::default().parse(":sectnums:\n:sectnumlevels: 2\n\n== L1\n\n=== L2\n\n==== L3");
    let numbered: Vec<bool> = collect_section_numbering(&doc);
    assert_eq!(numbered, vec![true, true, false]);

    // A value of `0` is effectively the same as `sectnums!`: nothing is
    // numbered.
    let doc = Parser::default().parse(":sectnums:\n:sectnumlevels: 0\n\n== L1\n\n=== L2");
    let numbered: Vec<bool> = collect_section_numbering(&doc);
    assert_eq!(numbered, vec![false, false]);
}

/// Walks every section in `doc` (depth first, document order) and reports
/// whether each carries a section number.
fn collect_section_numbering(doc: &crate::Document<'_>) -> Vec<bool> {
    fn walk(block: &crate::blocks::Block<'_>, out: &mut Vec<bool>) {
        if let crate::blocks::Block::Section(section) = block {
            out.push(section.section_number().is_some());
        }
        for child in block.nested_blocks() {
            walk(child, out);
        }
    }

    let mut out = vec![];
    for block in doc.nested_blocks() {
        walk(block, &mut out);
    }
    out
}

// The `untitled` option only affects DocBook output, and this parser models a
// single article-style document, so the closing of the attributes table and the
// scaffolding of the styles table are non-normative here. The `Doctypes` and
// `Converters` columns of the styles table likewise describe doctype
// availability and converter output that this parser does not model; only the
// `Style Name` cell of each row is verified (in `section_styles`).
non_normative!(
    r#"
|xref:special-section-titles.adoc[Hide special section title]
|`options` or `%`
|`untitled`
|DocBook only; can only be set on special sections
|===

== Section styles

[%autowidth]
|===
|Feature |Style Name |Doctypes |Converters

|xref:abstract.adoc[Abstract]
"#
);

#[test]
fn section_styles() {
    verifies!(
        r#"
|`abstract`
"#
    );

    non_normative!(
        r#"
|`article`
|All

|Acknowledgments
"#
    );

    verifies!(
        r#"
|`acknowledgments`
"#
    );

    non_normative!(
        r#"
|`book`
|All

|xref:appendix.adoc[Appendix]
"#
    );

    verifies!(
        r#"
|`appendix`
"#
    );

    non_normative!(
        r#"
|All
|All

|xref:bibliography.adoc[Bibliography]
"#
    );

    verifies!(
        r#"
|`bibliography`
"#
    );

    non_normative!(
        r#"
|All
|All

|xref:colophon.adoc[Colophon]
"#
    );

    verifies!(
        r#"
|`colophon`
"#
    );

    non_normative!(
        r#"
|`book`
|All

|xref:dedication.adoc[Dedication]
"#
    );

    verifies!(
        r#"
|`dedication`
"#
    );

    non_normative!(
        r#"
|`book`
|All

|xref:glossary.adoc[Glossary]
"#
    );

    verifies!(
        r#"
|`glossary`
"#
    );

    non_normative!(
        r#"
|All
|All

|xref:user-index.adoc[Index]
"#
    );

    verifies!(
        r#"
|`index`
"#
    );

    non_normative!(
        r#"
|All
|DocBook

|xref:preface.adoc[Preface]
"#
    );

    verifies!(
        r#"
|`preface`
"#
    );

    non_normative!(
        r#"
|`book`
|All
|===
"#
    );

    // Each listed style is recognized as a section's declared style.
    for style in [
        "abstract",
        "acknowledgments",
        "appendix",
        "bibliography",
        "colophon",
        "dedication",
        "glossary",
        "index",
        "preface",
    ] {
        let src = format!("[{style}]\n== Special Section");
        let doc = Parser::default().parse(&src);
        assert_eq!(first_section(&doc).declared_style(), Some(style));
    }
}
