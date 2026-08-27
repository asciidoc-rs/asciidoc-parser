//! Regression tests for documents that type the codepoints the substitution
//! pipeline uses as in-band control sentinels (issue #1235).
//!
//! The pipeline marks its own work inside the text it is substituting: a
//! deferred cross-reference leaves `U+E000 <index> U+E001` behind, a footnote
//! marker in a section title is bracketed with `U+E002`/`U+E003`, and an
//! extracted passthrough is stood in for by `U+0096 <index> U+0097`. Those
//! codepoints are unassigned (Private Use Area) or non-printing (C1), but a
//! document can type them, so they are escaped out of the document's own text
//! before substitution begins and restored on the way out.
//!
//! Two properties are covered:
//!
//!   * the output carries exactly the cross-references, footnotes, and
//!     passthroughs the document wrote — a typed sentinel sequence is never
//!     read back as the parser's own; and
//!   * the typed codepoints reach the output unchanged, since a Private Use
//!     Area character is content like any other.

use crate::{
    Document, Parser,
    blocks::{Block, FindBlocks, IsBlock, SimpleBlock},
};

/// Returns the rendered text of the last paragraph in `doc`, which is where
/// each of these documents writes the text under test (an anchor the
/// cross-references point at comes first).
fn last_paragraph<'a>(doc: &'a Document<'a>) -> &'a str {
    fn walk<'a>(blocks: impl Iterator<Item = &'a Block<'a>>, found: &mut Vec<&'a SimpleBlock<'a>>) {
        for block in blocks {
            if let Block::Simple(simple) = block {
                found.push(simple);
            } else {
                walk(block.child_blocks(), found);
            }
        }
    }

    let mut found = vec![];
    walk(doc.child_blocks(), &mut found);

    found
        .last()
        .expect("expected at least one simple block")
        .content()
        .rendered_html()
}

#[test]
fn a_typed_placeholder_cannot_forge_a_cross_reference() {
    // The document writes one cross-reference and, separately, the characters
    // that the cross-reference substitution uses to mark a deferred reference.
    let doc = Parser::default().parse(concat!(
        "[[a]]anchor\n",
        "\n",
        "<<a>> x\u{e000}0\u{e001}y\n",
    ));

    let rendered = last_paragraph(&doc);

    assert_eq!(
        rendered.matches("<a href=").count(),
        1,
        "the document wrote one cross-reference; got {rendered:?}"
    );

    // The typed codepoints are content, and reach the output as content.
    assert_eq!(rendered, "<a href=\"#a\">[a]</a> x\u{e000}0\u{e001}y");
}

#[test]
fn a_typed_placeholder_start_does_not_capture_a_real_placeholder() {
    // A lone start sentinel ahead of a real cross-reference would, unescaped,
    // make the reference's own placeholder look like the tail of a malformed
    // one — the case that reached `debug_assert!(false, …)` before this was
    // escaped.
    let doc = Parser::default().parse(concat!("[[a]]anchor\n", "\n", "\u{e000}<<a>>\n"));

    assert_eq!(
        last_paragraph(&doc),
        "\u{e000}<a href=\"#a\">[a]</a>",
        "the typed sentinel is content and the reference renders once"
    );
}

#[test]
fn a_typed_placeholder_is_inert_without_any_cross_reference() {
    // Nothing to forge here, but the text must still round-trip exactly.
    let doc = Parser::default().parse("a\u{e000}0\u{e001}b\n");
    assert_eq!(last_paragraph(&doc), "a\u{e000}0\u{e001}b");
}

#[test]
fn a_placeholder_from_an_attribute_value_cannot_forge_a_cross_reference() {
    // An attribute value is substituted into the text after it was escaped, so
    // it is escaped as it is spliced in; otherwise it is a second road to the
    // same forgery.
    let doc = Parser::default().parse(concat!(
        ":sentinel: \u{e000}0\u{e001}\n",
        "\n",
        "[[a]]anchor\n",
        "\n",
        "<<a>> {sentinel}\n",
    ));

    let rendered = last_paragraph(&doc);

    assert_eq!(rendered.matches("<a href=").count(), 1, "{rendered:?}");
    assert_eq!(rendered, "<a href=\"#a\">[a]</a> \u{e000}0\u{e001}");
}

#[test]
fn a_typed_passthrough_placeholder_cannot_forge_a_passthrough() {
    // The passthrough placeholders are the same kind of in-band mark, and are
    // escaped alongside the cross-reference ones: this document's `<b>` is
    // unescaped output the passthrough asked for, and must appear exactly once.
    let doc = Parser::default().parse("+++<b>+++ and \u{96}0\u{97} tail\n");

    let rendered = last_paragraph(&doc);

    assert_eq!(rendered.matches("<b>").count(), 1, "{rendered:?}");
    assert_eq!(rendered, "<b> and \u{96}0\u{97} tail");
}

#[test]
fn a_typed_placeholder_inside_a_passthrough_cannot_forge_a_cross_reference() {
    // A passthrough's text is spliced back in after the cross-reference
    // substitution ran, so it is restored into text that already carries
    // placeholders.
    let doc = Parser::default().parse(concat!(
        "[[a]]anchor\n",
        "\n",
        "<<a>> +x\u{e000}0\u{e001}+\n",
    ));

    let rendered = last_paragraph(&doc);

    assert_eq!(rendered.matches("<a href=").count(), 1, "{rendered:?}");
    assert_eq!(rendered, "<a href=\"#a\">[a]</a> x\u{e000}0\u{e001}");
}

#[test]
fn a_typed_marker_sentinel_does_not_truncate_a_section_reftext() {
    // The footnote-marker sentinels bracket a marker so it can be excised from
    // a section's reference text and auto-generated ID. A typed start sentinel
    // would otherwise open a span that the real marker's end sentinel closes,
    // swallowing the title text in between.
    let doc = Parser::default().parse(concat!(
        "== Alpha\u{e002}beta footnote:[a note]\n",
        "\n",
        "body\n",
    ));

    let Some(Block::Section(section)) = doc.child_blocks().next() else {
        panic!("expected a section block");
    };

    let title = section.section_title();

    // The heading keeps every character the document wrote, plus its footnote
    // marker.
    assert!(title.starts_with("Alpha\u{e002}beta"), "{title:?}");
    assert!(title.contains(r#"class="footnote""#), "{title:?}");

    // The reference text (and the ID derived from it) drops only the footnote
    // marker.
    assert_eq!(section.id(), Some("_alphabeta"));
}

/// Returns the rendered text of the last paragraph together with the number of
/// warnings the parse recorded — an unresolved cross-reference both renders its
/// fallback and warns, so the count is what separates "resolved" from "looks
/// resolved".
fn last_paragraph_and_warnings(source: &str) -> (String, usize) {
    let doc = Parser::default().parse(source);
    let warnings = doc.warnings().count();

    let mut rendered = String::new();
    for block in doc.descendant_blocks() {
        if let Block::Simple(simple) = block {
            rendered = simple.content().rendered_html().to_string();
        }
    }

    (rendered, warnings)
}

#[test]
fn a_natural_cross_reference_matches_a_title_holding_a_sentinel() {
    // A natural cross-reference matches on the target's *reference text*, which
    // for a section is its rendered title — the document's own text, held
    // outside the escaped form the substitution works in. The target is read
    // out of escaped text, so it leaves escaped form to be matched, or a title
    // carrying a reserved codepoint could never be referenced by name.
    let (rendered, warnings) =
        last_paragraph_and_warnings("== Alpha\u{e000}beta\n\nSee <<Alpha\u{e000}beta>>.\n");

    assert_eq!(warnings, 0, "reference did not resolve: {rendered:?}");
    assert_eq!(
        rendered,
        "See <a href=\"#_alphabeta\">Alpha\u{e000}beta</a>."
    );
}

#[test]
fn a_cross_reference_matches_an_id_holding_a_sentinel() {
    // An ID assigned to inline quoted text is read out of escaped text and
    // registered, so it leaves escaped form on the way into the catalog to meet
    // the (likewise unescaped) target.
    let (rendered, warnings) =
        last_paragraph_and_warnings("[#a\u{e000}b]#phrase#\n\nSee <<a\u{e000}b>>.\n");

    assert_eq!(warnings, 0, "reference did not resolve: {rendered:?}");
    assert_eq!(
        rendered, "See <a href=\"#a\u{e000}b\">[a\u{e000}b]</a>.",
        "the ID reaches the output as the document wrote it"
    );
}

#[test]
fn a_cross_reference_matches_a_block_id_holding_a_sentinel() {
    // A block's ID comes from its attribute list, which is parsed from the
    // source and so never enters escaped form at all. The target it is matched
    // against does, which is why the two meet unescaped.
    let (rendered, warnings) =
        last_paragraph_and_warnings("[#a\u{e000}b]\nparagraph\n\nSee <<a\u{e000}b>>.\n");

    assert_eq!(warnings, 0, "reference did not resolve: {rendered:?}");
    assert_eq!(rendered, "See <a href=\"#a\u{e000}b\">[a\u{e000}b]</a>.");
}

#[test]
fn a_typed_escape_introducer_round_trips() {
    // The codepoint that introduces an escaped sentinel is itself escaped, so a
    // document that types it — even followed by a character that is otherwise
    // an escape tag — gets it back verbatim rather than as some other sentinel.
    let doc = Parser::default().parse("x\u{e004}ay\u{e004}gz\n");
    assert_eq!(last_paragraph(&doc), "x\u{e004}ay\u{e004}gz");
}
