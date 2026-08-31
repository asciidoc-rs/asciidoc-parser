//! Regression tests for documents that type the codepoints the string
//! substitution pipeline used as in-band control sentinels (issue #1235).
//!
//! That pipeline marked its own work inside the text it was substituting: a
//! deferred cross-reference left `U+E000 <index> U+E001` behind, a footnote
//! marker in a section title was bracketed with `U+E002`/`U+E003`, and an
//! extracted passthrough was stood in for by `U+0096 <index> U+0097`. Those
//! codepoints are unassigned (Private Use Area) or non-printing (C1), but a
//! document can type them, so the pipeline escaped them out of the document's
//! own text before substituting and restored them on the way out. The
//! single-pass builder needs none of that — it recognizes constructs by range
//! over the source, and a carried title's deferred template (the splice that
//! outlives the parse) is a structured piece list rather than a marked
//! string — so these tests now pin the simpler invariant that a typed
//! sentinel is ordinary content. The one in-band form left in production is a
//! footnote's deferred template (see `FootnoteDeferred::render`); retiring it
//! is the remaining slice of design §4.2's third sentinel system.
//!
//! Two properties are covered:
//!
//!   * the output carries exactly the cross-references, footnotes, and
//!     passthroughs the document wrote — a typed sentinel sequence is never
//!     read back as the parser's own; and
//!   * the typed codepoints reach the output unchanged, since a Private Use
//!     Area character is content like any other.
//!
//! One byte-pattern mechanism is still live, and the last several tests here
//! are about it rather than about issue #1235: `tokened_bracket`/`tokened_text`
//! write a `MASKED_PIECE_PLACEHOLDER` pair per masked piece into the text
//! `Attrlist::parse` splits, and every reader recovers which piece an
//! occurrence stands for by counting occurrences in the parsed text. Those
//! readers no longer have to tell a genuine occurrence from a document's own
//! copy of the same two codepoints: the two tokeners escape every non-tokened
//! byte they copy (`escape_masked_piece_bytes`), so every occurrence in a
//! tokened text is one a tokener wrote, and every consumer unescapes on the way
//! out — which is what makes a typed pair ordinary content here as much as
//! anywhere else in this file. Those tests come in three groups: four pin that
//! property (a typed pair, and the escape's own introducer, round-tripping
//! beside a real masked piece and alone); one records the one road that
//! **can** still forge an occurrence — `Attrlist::parse`'s own re-substitution
//! of an attribute reference over already-tokened text; and two pin the inputs
//! that rule out replacing the whole scheme with a byte-offset table (see the
//! design doc's §3.5, "A rejected refinement: carrying a byte-offset table
//! through `Attrlist::parse`").

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
fn a_placeholder_from_an_attribute_value_cannot_forge_an_image_restore() {
    // `image:`/`icon:` and the link families' display-text list are the one
    // place left where a masked passthrough or STEM expression is restored
    // by scanning parsed text for its own [`MASKED_PIECE_PLACEHOLDER`]
    // occurrence (`tokened_bracket`/`Attrlist::into_owned_restoring`), rather
    // than by node structure. An attribute reference is substituted into the
    // bracket *before* the image macro is even recognized, so an attribute
    // whose value happens to spell that same pair — sitting in the same
    // bracket as a real passthrough — would otherwise be indistinguishable
    // from the pipeline's own placeholder and splice the passthrough's
    // rendered body into `alt` instead of the literal text the document
    // defined.
    //
    // The forged value has to spell the placeholder **exactly**
    // (`MASKED_PIECE_PLACEHOLDER`'s own two adjacent codepoints, no digit
    // between them) and has to sit **ahead** of the real masked piece: this
    // test spelled the retired digit-carrying shape in the other order until
    // 2026-08-30, which made it pass with the guard of the day
    // (`escape_passthrough_sentinels`) removed — pinning that guard's *output*
    // rather than the forgery it existed to prevent. Both halves are kept
    // because they are what make the fixture a real forgery attempt.
    //
    // What stops it is no longer that guard, which is retired: it is the
    // tokener's own escape over every non-tokened byte it copies, so this
    // spliced spelling and the typed-in-the-clear one
    // (`a_typed_placeholder_beside_a_masked_piece_cannot_forge_a_bracket_restore`)
    // are now stopped by one mechanism. Being two-way, it also lets the value
    // reach the output as the document defined it rather than in escaped form.
    let doc = Parser::default().parse(concat!(
        ":forge: \u{96}\u{97}\n",
        "\n",
        "image:x.png[alt={forge},++real++]\n",
    ));

    let rendered = last_paragraph(&doc);

    assert!(
        !rendered.contains("alt=\"real\""),
        "the forged attribute value must not restore the passthrough's body: {rendered:?}"
    );

    // The genuine occurrence keeps the one body supplied, in the positional
    // slot it actually stands in (`width`), and the forged value reaches the
    // output verbatim.
    assert_eq!(
        rendered,
        "<span class=\"image\"><img src=\"x.png\" alt=\"\u{96}\u{97}\" width=\"real\"></span>"
    );
}

#[test]
fn a_typed_placeholder_beside_a_masked_piece_cannot_forge_a_bracket_restore() {
    // The road `escape_passthrough_sentinels` never covered, now closed.
    //
    // That guard sat at `split_attribute_value`, the attribute-reference
    // step's own content-level splice, so it saw only a value *spliced* into
    // the bracket. A document that types the pair **in the clear** never
    // passes through that splice, and ahead of a real masked piece it captured
    // that piece's body exactly as a spliced forgery would (this test pinned
    // that as a known gap until 2026-08-31).
    //
    // What closes it is provenance where the placeholder is *written*:
    // `tokened_bracket` escapes every non-tokened byte it copies
    // (`escape_masked_piece_bytes`), so the only occurrences in the text
    // `Attrlist::parse` splits are the ones it wrote itself, and the restore
    // has nothing to be fooled by. The escape being two-way is what lets the
    // typed pair itself land in `alt` as the document wrote it rather than in
    // escaped form.
    let doc = Parser::default().parse("image:x.png[alt=\u{96}\u{97},++real++]\n");

    let rendered = last_paragraph(&doc);

    assert!(
        !rendered.contains("alt=\"real\""),
        "the typed pair must not restore the passthrough's body: {rendered:?}"
    );

    assert_eq!(
        rendered,
        "<span class=\"image\"><img src=\"x.png\" alt=\"\u{96}\u{97}\" width=\"real\"></span>"
    );

    // The link family's display-text list shares `tokened_bracket`, so it
    // shares the fix: `++real++` stays the second entry (no positional display
    // text, hence the `bare` role and the target standing in for the text),
    // and the typed pair reaches the role verbatim.
    let doc = Parser::default().parse("link:x[role=\u{96}\u{97},++real++]\n");

    let rendered = last_paragraph(&doc);

    assert!(
        !rendered.contains("class=\"bare real\""),
        "the typed pair must not restore the passthrough's body: {rendered:?}"
    );

    assert_eq!(rendered, "<a href=\"x\" class=\"bare \u{96}\u{97}\">x</a>");
}

#[test]
fn a_typed_placeholder_round_trips_through_a_bracket_untouched() {
    // The other half of what the tokener's escape has to be: two-way. With no
    // masked piece anywhere near it, a typed pair is ordinary content, and the
    // escape written over it on the way into `Attrlist::parse` has to come
    // back off before the value is rendered — every consumer of a tokened
    // parse unescapes (`unescape_masked_piece_bytes`), including the
    // no-placeholder early return in `restore_into` that does no restoring at
    // all.
    let doc = Parser::default().parse("image:x.png[alt=a\u{96}\u{97}b]\n");

    assert_eq!(
        last_paragraph(&doc),
        "<span class=\"image\"><img src=\"x.png\" alt=\"a\u{96}\u{97}b\"></span>"
    );

    // The link family's display-text list, whose values take the same restore
    // and whose *text* takes `restored_value_children`'s own rebuild.
    let doc = Parser::default().parse("link:x[role=a\u{96}\u{97}b]\n");

    assert_eq!(
        last_paragraph(&doc),
        "<a href=\"x\" class=\"bare a\u{96}\u{97}b\">x</a>"
    );

    // And the cross-reference family's `tokened_text`, whose display text and
    // roles are read back through `restored_value_children` and
    // `untranslated_value` respectively.
    let doc = Parser::default().parse(concat!(
        "[[a]]anchor\n",
        "\n",
        "xref:a[t\u{96}\u{97}u,role=r\u{96}\u{97}s]\n",
    ));

    assert_eq!(
        last_paragraph(&doc),
        "<a href=\"#a\" class=\"r\u{96}\u{97}s\">t\u{96}\u{97}u</a>"
    );
}

#[test]
fn a_typed_placeholder_round_trips_beside_a_real_masked_piece() {
    // The same round-trip with a genuine masked piece in the same bracket, so
    // the restore walk actually runs: the typed pair has to survive the walk
    // that substitutes the piece beside it, and the piece has to land in its
    // own positional slot rather than in the forged one.
    let doc = Parser::default().parse("image:x.png[alt=a\u{96}\u{97}b,++real++]\n");

    assert_eq!(
        last_paragraph(&doc),
        "<span class=\"image\"><img src=\"x.png\" alt=\"a\u{96}\u{97}b\" width=\"real\"></span>"
    );

    // The link family's *display text*, where the pair rides in the same
    // value as the piece rather than in a neighbouring one — so the split
    // `restored_value_children` makes on the genuine occurrence has to leave
    // the typed one alone, and `computed_value_children` has to unescape the
    // runs either side of it.
    let doc = Parser::default().parse("link:x[a\u{96}\u{97}b ++real++,role=hl]\n");

    assert_eq!(
        last_paragraph(&doc),
        "<a href=\"x\" class=\"hl\">a\u{96}\u{97}b real</a>"
    );
}

#[test]
fn a_typed_escape_introducer_round_trips_through_a_bracket() {
    // The escape has to be unambiguous against *itself*, so
    // `escape_masked_piece_bytes` escapes its own introducer
    // (`\u{e005}` → `\u{e005}g`). Without that, a document that types the
    // introducer followed by one of the escape's tag characters — `s` here,
    // which names the placeholder's start codepoint — would get back a
    // `\u{96}` it never wrote.
    let doc = Parser::default().parse("image:x.png[alt=a\u{e005}sb,++real++]\n");

    assert_eq!(
        last_paragraph(&doc),
        "<span class=\"image\"><img src=\"x.png\" alt=\"a\u{e005}sb\" width=\"real\"></span>"
    );

    let doc = Parser::default().parse(concat!(
        "[[a]]anchor\n",
        "\n",
        "xref:a[t\u{e005}su ++real++,role=hl]\n",
    ));

    assert_eq!(
        last_paragraph(&doc),
        "<a href=\"#a\" class=\"hl\">t\u{e005}su real</a>"
    );
}

#[test]
fn an_attrlist_level_expansion_can_still_forge_a_bracket_restore() {
    // The one road left, recorded rather than fixed.
    //
    // The tokener escapes the bytes it *copies*, which settles every
    // occurrence in the text it hands to `Attrlist::parse`. But that parse
    // runs an attribute-reference substitution of its own over that text
    // whenever it holds both a `{` and a `}` — and a `subs=` list naming
    // `macros` without `attributes` reaches the macros step with every
    // reference still unresolved, so the inner substitution is the one that
    // expands it, *after* the escape ran. A value spelling the placeholder
    // therefore lands in the tokened text unescaped and takes the real
    // passthrough's body, exactly as a typed pair used to.
    //
    // This is the same re-substitution that blocks the byte-offset table (see
    // `an_attrlist_level_reference_expansion_moves_a_placeholder_in_the_tokened_text`
    // and the design doc's §3.5); closing it means escaping inside that
    // parse, which is a mechanism change of its own and a separate increment.
    let doc = Parser::default().parse(concat!(
        ":forge: \u{96}\u{97}\n",
        "\n",
        "[subs=macros]\n",
        "image:x.png[alt={forge},++real++]\n",
    ));

    assert_eq!(
        last_paragraph(&doc),
        "<span class=\"image\"><img src=\"x.png\" alt=\"real\" width=\"\u{96}\u{97}\"></span>",
        "known gap: the attrlist-level expansion captured the passthrough's body"
    );

    // The same parse also splices a bare escape introducer past the tokener,
    // where `restore_into`'s walk copies it through rather than reading the
    // byte after it as a tag.
    let doc = Parser::default().parse(concat!(
        ":stray: p\u{e005}zq\n",
        "\n",
        "[subs=macros]\n",
        "image:x.png[alt={stray},++real++]\n",
    ));

    assert_eq!(
        last_paragraph(&doc),
        "<span class=\"image\"><img src=\"x.png\" alt=\"p\u{e005}zq\" width=\"real\"></span>"
    );
}

#[test]
fn an_attrlist_level_reference_expansion_moves_a_placeholder_in_the_tokened_text() {
    // Evidence for the design-doc note named above: the byte offsets
    // `tokened_bracket` could record for the occurrences it writes are not
    // stable across the parse that consumes them.
    //
    // `Attrlist::parse` runs an attribute-reference substitution of its own
    // over the text it is handed, whenever that text holds both a `{` and a
    // `}`. A `subs=` list naming `macros` but not `attributes` reaches the
    // macros step with every reference still unresolved — the content-level
    // pass never ran — so that inner substitution is the one that expands
    // `{name}` here, *after* `tokened_bracket` has already written its
    // placeholder into the same string. The text splits as
    // `alt=a-much-longer-value,title=\u{96}\u{97}` where the tokener wrote
    // `alt={name},title=\u{96}\u{97}`: the occurrence sits at byte 30, not at
    // byte 17.
    //
    // Restoration by *position* is unaffected — the occurrence count either
    // side of the expansion is what it walks — which is why this renders
    // correctly today and would not under a byte-offset table.
    let doc = Parser::default().parse(concat!(
        ":name: a-much-longer-value\n",
        "\n",
        "[subs=macros]\n",
        "image:x.png[alt={name},title=++real++]\n",
    ));

    assert_eq!(
        last_paragraph(&doc),
        "<span class=\"image\"><img src=\"x.png\" alt=\"a-much-longer-value\" title=\"real\"></span>"
    );

    // The link family's display-text list reaches the same inner
    // substitution through the same `Attrlist::parse` call.
    let doc = Parser::default().parse(concat!(
        ":name: a-much-longer-value\n",
        "\n",
        "[subs=macros]\n",
        "link:x[++real++,role={name}]\n",
    ));

    assert_eq!(
        last_paragraph(&doc),
        "<a href=\"x\" class=\"a-much-longer-value\">real</a>"
    );
}

#[test]
fn a_quoted_values_own_unescape_moves_a_placeholder_in_the_parsed_value() {
    // The second, brace-independent half of the same evidence.
    //
    // Even if the text `Attrlist::parse` splits were byte-identical to what
    // `tokened_bracket` wrote, a *parsed value* is not a slice of it:
    // `ElementAttribute::parse` skips leading whitespace, strips the name and
    // `=`, strips the quotes, rewrites `\"` to `"` with a plain
    // `String::replace`, and trims trailing spaces — and reports none of
    // that back, returning only the attribute and the offset the entry ends
    // at. The `\"` here sits ahead of the placeholder and shortens the value
    // by one byte, so an offset recorded against the tokened text lands one
    // byte late inside the value even before the four bytes `alt="` accounts
    // for.
    let doc = Parser::default().parse("image:x.png[alt=\"a \\\" b ++real++ c\"]\n");

    assert_eq!(
        last_paragraph(&doc),
        "<span class=\"image\"><img src=\"x.png\" alt=\"a &quot; b real c\"></span>"
    );
}

#[test]
fn a_placeholder_from_an_attribute_value_cannot_forge_a_link_display_text_restore() {
    // The `link:` macro's display-text list shares `tokened_bracket` and
    // `Attrlist::into_owned_restoring` with the image family's bracket (see
    // `a_placeholder_from_an_attribute_value_cannot_forge_an_image_restore`,
    // including why the forged value has to spell the pair exactly and sit
    // ahead of the real passthrough), so the same forgery reaches it through
    // a `role=` attribute sitting beside a real passthrough in the same
    // display-text list, absent the same escape.
    let doc = Parser::default().parse(concat!(
        ":forge: \u{96}\u{97}\n",
        "\n",
        "link:x[role={forge},++real++]\n",
    ));

    let rendered = last_paragraph(&doc);

    assert!(
        !rendered.contains("class=\"bare real\""),
        "the forged attribute value must not restore the passthrough's body: {rendered:?}"
    );

    // With no positional attribute left to be the display text, the target
    // stands in for it and the auto-link `bare` role joins the value the
    // document defined — which reaches the role verbatim, the tokener's escape
    // being two-way.
    assert_eq!(rendered, "<a href=\"x\" class=\"bare \u{96}\u{97}\">x</a>");
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
fn a_typed_placeholder_in_a_carried_title_cannot_forge_a_cross_reference() {
    // A block title carried across a section heading is the one content that
    // renders from a deferred template in production, synthesized from the
    // title's inline tree (`carried_title_template`). The template is a
    // structured piece list — a splice point is a variant, not a byte pattern
    // — so the typed sequence here is just bytes inside a literal piece:
    // nothing scans it, and no escaping is needed to keep it apart from the
    // real cross-reference spliced beside it.
    let doc = Parser::default().parse(concat!(
        "[[a]]anchor\n",
        "\n",
        ".x\u{e000}0\u{e001}y <<a>>\n",
        "== Section\n",
        "\n",
        "para\n",
    ));

    let Some(Block::Section(section)) = doc.child_blocks().nth(1) else {
        panic!("expected a section block");
    };

    let title = section
        .child_blocks()
        .next()
        .expect("expected the section's first child block")
        .title()
        .expect("expected the carried title");

    assert_eq!(
        title.matches("<a href=").count(),
        1,
        "the title wrote one cross-reference; got {title:?}"
    );

    assert_eq!(title, "x\u{e000}0\u{e001}y <a href=\"#a\">[a]</a>");
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

/// A source whose ids carry a typed escape introducer beside both
/// cross-reference spellings. This began as the carve-out's one suite shape —
/// a form the tree deferred and the string pipeline's template rendered — and
/// each mechanism it exercised has since been retired under it (the deferral
/// divergence took the form, the oracle deletion took the carve-out), with the
/// expected bytes simply carrying the current reading. `\u{e004}b` in the id
/// is the sequence a decode-once pass corrupts: `\u{e004}` introduces an
/// escaped sentinel and `b` is one of its tags.
const SENTINEL_ID_SOURCE: &str = concat!(
    "[#a\u{e004}b]\n",
    "The target.\n",
    "\n",
    "See <<a\u{e004}b>> and xref:a\u{e004}b[a *b, c* d,role=hl].\n",
);

#[test]
fn a_resolved_destination_holding_a_sentinel_survives_resolution() {
    // The destination comes back from the **resolver**, which was handed the
    // document's own text and answered in kind — so it is not in escaped form
    // and must not be decoded on its way into the output. The paragraph's
    // rendering is a fold of its tree at the end of resolution, whose text is
    // the document's own; the historical hazard this pins was the template
    // path's decode-once pass, which turned `#a\u{e004}b` into `#a\u{e001}`.
    //
    // The second anchor is whole rather than cut short inside its span since
    // the deferral divergence (design §5.2's step 6) — the sentinel handling
    // this test is about is unchanged by that, and the expected bytes simply
    // carry the tree's reading now.
    let (rendered, warnings) = last_paragraph_and_warnings(SENTINEL_ID_SOURCE);

    assert_eq!(warnings, 0, "reference did not resolve: {rendered:?}");

    assert_eq!(
        rendered,
        concat!(
            "See <a href=\"#a\u{e004}b\">[a\u{e004}b]</a> and ",
            "<a href=\"#a\u{e004}b\" class=\"hl\">a <strong>b, c</strong> d</a>."
        ),
        "the id reaches the output as the document wrote it"
    );
}

#[test]
fn a_title_keeps_a_resolved_sentinel_too() {
    // The same crossing with the **title** container, which resolves through
    // the document-order title pass on a path of its own.
    let mut parser = Parser::default();

    let doc = parser.parse(concat!(
        "[#a\u{e004}b]\n",
        "The target.\n",
        "\n",
        ".See <<a\u{e004}b>> and xref:a\u{e004}b[a *b, c* d,role=hl]\n",
        "A paragraph.\n",
    ));

    let titles: Vec<String> = doc
        .descendant_blocks()
        .filter_map(|block| block.title().map(str::to_string))
        .collect();

    assert!(
        titles
            .iter()
            .any(|t| t.contains("href=\"#a\u{e004}b\"") && !t.contains('\u{e001}')),
        "a resolved destination was decoded in a title: {titles:?}"
    );
}
