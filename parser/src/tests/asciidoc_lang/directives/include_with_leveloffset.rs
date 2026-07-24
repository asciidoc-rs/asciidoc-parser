use crate::{
    blocks::Block,
    tests::prelude::{inline_file_handler::InlineFileHandler, *},
};

track_file!("ref/asciidoc-lang/docs/modules/directives/pages/include-with-leveloffset.adoc");

/// Collects the effective level and rendered title of every section in the
/// document, depth-first in document order. This is how these tests observe the
/// heading-level shifting that `leveloffset` performs.
fn section_levels(doc: &crate::Document<'_>) -> Vec<(usize, String)> {
    fn walk(block: &Block<'_>, out: &mut Vec<(usize, String)>) {
        if let Block::Section(section) = block {
            out.push((section.level(), section.section_title().to_string()));
        }
        for child in block.child_blocks() {
            walk(child, out);
        }
    }

    let mut out = vec![];
    for block in doc.child_blocks() {
        walk(block, &mut out);
    }
    out
}

non_normative!(
    r#"
= Offset Section Levels
//Partitioning Large Documents and using leveloffset
// [#include-partitioning]

When your document gets large, you can split it up into subdocuments for easier editing.

----
= My book

\include::chapter01.adoc[]

\include::chapter02.adoc[]

\include::chapter03.adoc[]
----

TIP: Note the empty lines before and after the include directives.
This practice is recommended whenever including AsciiDoc content to avoid unexpected results (e.g., a section title getting interpreted as a line at the end of a previous paragraph).

== Manipulate heading levels with leveloffset

"#
);

#[test]
fn pushes_headings_down_by_offset() {
    verifies!(
        r#"
The `leveloffset` attribute can help here by pushing all headings in the included document down by the specified number of levels.
This allows you to publish each chapter as a standalone document (complete with a document title), but still be able to include the chapters into a primary document (which has its own document title).

"#
    );

    // `chapter01.adoc` is a standalone document: it has its own level-0
    // document title (`= Chapter Title`) and a level-1 section (`== A
    // Section`). Included with `leveloffset=+1`, both headings are pushed down
    // one level — the document title becomes a level-1 section and its section
    // becomes level 2 — so it slots under the primary document's own title.
    let handler = InlineFileHandler::from_pairs([(
        "chapter01.adoc",
        "= Chapter Title\n\nChapter intro.\n\n== A Section\n\nSection body.",
    )]);

    let doc = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler)
        .parse("= My Book\n\ninclude::chapter01.adoc[leveloffset=+1]");

    assert_eq!(
        section_levels(&doc),
        vec![
            (1, "Chapter Title".to_string()),
            (2, "A Section".to_string()),
        ]
    );

    // Without the offset the level-0 document title of the included file is not
    // a valid body heading, so no shifting occurs and the title is rejected.
    let handler = InlineFileHandler::from_pairs([(
        "chapter01.adoc",
        "= Chapter Title\n\nChapter intro.\n\n== A Section\n\nSection body.",
    )]);

    let doc = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler)
        .parse("= My Book\n\ninclude::chapter01.adoc[]");

    assert_eq!(section_levels(&doc), vec![(1, "A Section".to_string())]);
}

non_normative!(
    r#"
You can easily assemble your book so that the chapter document titles become level 1 headings using:

----
= My Book

\include::chapter01.adoc[leveloffset=+1]

\include::chapter02.adoc[leveloffset=+1]

\include::chapter03.adoc[leveloffset=+1]
----

"#
);

#[test]
fn relative_offset_accumulates_across_nested_includes() {
    verifies!(
        r#"
Because the leveloffset is _relative_ (it begins with + or -), this works even if the included document has its own includes and leveloffsets.

"#
    );

    // `chapter.adoc` is itself assembled from an include with its own relative
    // `leveloffset=+1`. Because the offsets are relative, they compose: the
    // outer `+1` shifts the chapter's headings down one level, and the inner
    // `+1` accumulates on top of that (offset `+2`) for the sub-document's
    // headings.
    let handler = InlineFileHandler::from_pairs([
        (
            "chapter.adoc",
            "= Chapter Title\n\nChapter intro.\n\ninclude::section.adoc[leveloffset=+1]",
        ),
        (
            "section.adoc",
            "= Section Title\n\nSection intro.\n\n== A Subsection\n\nSubsection body.",
        ),
    ]);

    let doc = Parser::default()
        .with_safe_mode(SafeMode::Server)
        .with_include_file_handler(handler)
        .parse("= My Book\n\ninclude::chapter.adoc[leveloffset=+1]");

    assert_eq!(
        section_levels(&doc),
        vec![
            // `= Chapter Title` (level 0) + outer offset 1
            (1, "Chapter Title".to_string()),
            // `= Section Title` (level 0) + accumulated offset 2
            (2, "Section Title".to_string()),
            // `== A Subsection` (level 1) + accumulated offset 2
            (3, "A Subsection".to_string()),
        ]
    );
}

non_normative!(
    r#"
If you have lots of chapters to include and want them all to have the same offset, you can save some typing by setting `leveloffset` around the includes:

----
= My book

:leveloffset: +1

\include::chapter01.adoc[]

\include::chapter02.adoc[]

\include::chapter03.adoc[]

:leveloffset: -1
----

"#
);

#[test]
fn trailing_offset_returns_to_zero() {
    verifies!(
        r#"
The final line returns the level offset to 0.

"#
    );

    // A `:leveloffset: +1` attribute entry shifts every following heading down
    // one level; the trailing `:leveloffset: -1` accumulates back to 0, so
    // headings after it are no longer shifted. `= Wrapped Chapter` (level 0)
    // becomes a level-1 section while the offset is in effect, and `== After
    // Reset` (level 1) is left at level 1 once the offset returns to 0.
    let doc = Parser::default().parse(concat!(
        "= My Book\n\n",
        ":leveloffset: +1\n\n",
        "= Wrapped Chapter\n\n",
        "Chapter body.\n\n",
        ":leveloffset: -1\n\n",
        "== After Reset\n\n",
        "Body after reset.",
    ));

    assert_eq!(
        section_levels(&doc),
        vec![
            (1, "Wrapped Chapter".to_string()),
            (1, "After Reset".to_string()),
        ]
    );
}

non_normative!(
    r#"
Alternatively, you could use absolute levels:

----
:leveloffset: 1

//includes

:leveloffset: 0
----

Relative levels are preferred.
Absolute levels become awkward when you have nested includes since they aren't context aware.

////
That's also why it's important to surround the include directive by empty lines if it imports in a discrete structure.

You only want to place include files directly adjacent to one another if the imported content should be directly adjacent.

IMPORTANT: Take note of the empty lines between the include directives.
The empty line between include directives prevents the first and last lines of the included files from being adjoined.
This practice is *strongly* encouraged when combining document parts.
If you don't include these empty lines, you might find that the AsciiDoc processor swallows section titles.
This happens because the leading section title can get interpreted as the last line of the final paragraph in the preceding include.
Only place include directives on consecutive lines if the intent is for the includes to run together (such as in a listing block).
////
"#
);

// The remaining tests exercise behaviors the spec page only describes
// non-normatively (an absolute `leveloffset`, and a header-scoped offset) plus
// the parser's graceful handling of a malformed relative value. They are not
// tied to a normative claim, so they use plain `#[test]` rather than
// `verifies!`.

#[test]
fn absolute_offset_set_in_header_shifts_body_headings() {
    // The spec's "Alternatively, you could use absolute levels" note applies an
    // absolute `:leveloffset:` (here in the document header) rather than a
    // relative one. An absolute offset is stored verbatim and applied to every
    // following heading: `= Chapter One` (level 0) becomes a level-1 section and
    // its `== A Section` (level 1) becomes level 2.
    let doc = Parser::default().parse(concat!(
        "= My Book\n",
        ":leveloffset: 1\n",
        "\n",
        "= Chapter One\n",
        "\n",
        "== A Section",
    ));

    assert_eq!(
        section_levels(&doc),
        vec![(1, "Chapter One".to_string()), (2, "A Section".to_string()),]
    );
}

#[test]
fn malformed_relative_offset_is_ignored() {
    // A `leveloffset` whose relative form has no valid integer (`+x`) cannot be
    // resolved, so it is left as-is and read back as a zero offset: headings are
    // not shifted. `== Real Section` (level 1) therefore stays at level 1.
    let doc = Parser::default().parse(concat!(
        "= My Book\n",
        "\n",
        ":leveloffset: +x\n",
        "\n",
        "== Real Section",
    ));

    assert_eq!(section_levels(&doc), vec![(1, "Real Section".to_string())]);
}

#[test]
fn absolute_offset_near_i32_max_does_not_overflow() {
    // A hostile absolute `:leveloffset:` at `i32::MAX` must not overflow when it
    // is folded into a heading's syntactic level (here 5, from `======`). The
    // effective level saturates instead of panicking (debug) or wrapping
    // (release), then clamps into the supported 1-5 range: the heading is still
    // a section, at level 5.
    let src = format!(
        "= My Book\n:leveloffset: {}\n\n====== Deep Heading",
        i32::MAX
    );
    let doc = Parser::default().parse(&src);

    assert_eq!(section_levels(&doc), vec![(5, "Deep Heading".to_string())]);

    // The offset is far outside the usable range, so it is reported once when
    // set, and the heading it clamps is reported once when parsed.
    let warnings: Vec<_> = doc.warnings().map(|w| w.warning.clone()).collect();
    assert_eq!(
        warnings,
        vec![
            WarningType::LeveloffsetExcludesAllHeadingLevels(i32::MAX),
            WarningType::SectionHeadingLevelOutOfRange(i32::MAX, 5),
        ]
    );
}

#[test]
fn relative_offset_accumulation_near_i32_max_does_not_overflow() {
    // Accumulating a relative `+1` on top of an offset already at `i32::MAX`
    // must not overflow while resolving the assignment. The offset saturates at
    // `i32::MAX`, so the following heading is still parsed as a section, clamped
    // to level 5.
    let src = format!(
        "= My Book\n:leveloffset: {}\n\n:leveloffset: +1\n\n====== Deep Heading",
        i32::MAX
    );
    let doc = Parser::default().parse(&src);

    assert_eq!(section_levels(&doc), vec![(5, "Deep Heading".to_string())]);

    // Both `:leveloffset:` entries are outside the usable range, so each is
    // reported when set; the clamped heading is reported once when parsed.
    let warnings: Vec<_> = doc.warnings().map(|w| w.warning.clone()).collect();
    assert_eq!(
        warnings,
        vec![
            WarningType::LeveloffsetExcludesAllHeadingLevels(i32::MAX),
            WarningType::LeveloffsetExcludesAllHeadingLevels(i32::MAX),
            WarningType::SectionHeadingLevelOutOfRange(i32::MAX, 5),
        ]
    );
}

// Collects the `WarningType` of every warning a parse produced, in order.
fn warning_types(doc: &crate::Document<'_>) -> Vec<crate::warnings::WarningType> {
    doc.warnings().map(|w| w.warning.clone()).collect()
}

#[test]
fn positive_offset_clamps_overshooting_child_heading_once() {
    // Offset `+3` is within the usable range, so setting it raises no warning,
    // and `== Parent` (level 1 -> 4) stays in range. `===== Child` overshoots
    // to 7 and is clamped to 5. Because Child sits one clamped level below
    // Parent it nests as a child (no skipped-level warning), and the clamp is
    // reported exactly once — not twice — even though the boundary look-ahead
    // also inspects it.
    let doc = Parser::default().parse(concat!(
        "= My Book\n\n",
        ":leveloffset: +3\n\n",
        "== Parent\n\n",
        "===== Child",
    ));

    assert_eq!(
        section_levels(&doc),
        vec![(4, "Parent".to_string()), (5, "Child".to_string())]
    );

    assert_eq!(
        warning_types(&doc),
        vec![WarningType::SectionHeadingLevelOutOfRange(7, 5)]
    );
}

#[test]
fn negative_offset_clamps_real_section_up_to_one() {
    // A real section heading pulled below level 1 by a negative offset is
    // clamped to 1 and reported. Offset `-2` is still usable (a deep heading
    // could land in range), so setting it raises no warning.
    let doc = Parser::default().parse(concat!(
        "= My Book\n\n",
        ":leveloffset: -2\n\n",
        "== Pulled Up\n\n",
        "body",
    ));

    assert_eq!(section_levels(&doc), vec![(1, "Pulled Up".to_string())]);
    assert_eq!(
        warning_types(&doc),
        vec![WarningType::SectionHeadingLevelOutOfRange(-1, 1)]
    );
}

#[test]
fn document_title_with_nonpositive_offset_is_still_rejected() {
    // The single-document-title rule is preserved: a bare `= Title` in the body
    // that no positive offset lifts into range is declined (not clamped into a
    // level-1 section). Offset `-1` is usable, so only the level-0 warning is
    // raised and no section is produced.
    let doc = Parser::default().parse(concat!(
        "= My Book\n\n",
        ":leveloffset: -1\n\n",
        "= Orphan Title\n\n",
        "body",
    ));

    assert!(section_levels(&doc).is_empty());
    assert_eq!(
        warning_types(&doc),
        vec![WarningType::Level0SectionHeadingNotSupported]
    );
}

#[test]
fn offsets_at_the_usable_boundary_do_not_warn() {
    // `+5` is the largest usable offset: a level-0 `= Chapter` still lands at
    // level 5. `-4` is the smallest: a level-5 `====== Deep` still lands at
    // level 1. Neither boundary offset is reported, and neither heading is
    // clamped.
    let high =
        Parser::default().parse(concat!("= My Book\n\n", ":leveloffset: 5\n\n", "= Chapter",));
    assert_eq!(section_levels(&high), vec![(5, "Chapter".to_string())]);
    assert!(warning_types(&high).is_empty());

    let low = Parser::default().parse(concat!(
        "= My Book\n\n",
        ":leveloffset: -4\n\n",
        "====== Deep",
    ));
    assert_eq!(section_levels(&low), vec![(1, "Deep".to_string())]);
    assert!(warning_types(&low).is_empty());
}

#[test]
fn offset_outside_usable_range_warns_when_set() {
    // Just past each usable boundary, no heading can land in range, so the
    // offset is reported the moment it is set — once per assignment, ahead of
    // any per-heading clamp warning.
    let high =
        Parser::default().parse(concat!("= My Book\n\n", ":leveloffset: 6\n\n", "= Chapter",));
    assert_eq!(section_levels(&high), vec![(5, "Chapter".to_string())]);
    assert_eq!(
        warning_types(&high),
        vec![
            WarningType::LeveloffsetExcludesAllHeadingLevels(6),
            WarningType::SectionHeadingLevelOutOfRange(6, 5),
        ]
    );

    let low = Parser::default().parse(concat!(
        "= My Book\n\n",
        ":leveloffset: -5\n\n",
        "====== Deep",
    ));
    assert_eq!(section_levels(&low), vec![(1, "Deep".to_string())]);
    assert_eq!(
        warning_types(&low),
        vec![
            WarningType::LeveloffsetExcludesAllHeadingLevels(-5),
            WarningType::SectionHeadingLevelOutOfRange(0, 1),
        ]
    );
}

#[test]
fn impossible_offset_set_in_header_warns() {
    // An unusable `leveloffset` set in the document header is reported via the
    // header attribute path, and the body heading it clamps is reported once.
    let doc = Parser::default().parse(concat!("= My Book\n", ":leveloffset: 9\n\n", "== Section",));

    assert_eq!(section_levels(&doc), vec![(5, "Section".to_string())]);
    assert_eq!(
        warning_types(&doc),
        vec![
            WarningType::LeveloffsetExcludesAllHeadingLevels(9),
            WarningType::SectionHeadingLevelOutOfRange(10, 5),
        ]
    );
}

#[test]
fn extreme_negative_relative_offset_is_applied_not_stored_as_absolute() {
    // `-2147483648` has a magnitude larger than `i32::MAX`, so stripping the
    // sign and parsing the magnitude as `i32` would fail and store the value as
    // an absolute offset (`i32::MIN`), clamping every later heading to level 1.
    // Parsed as a signed delta it instead accumulates: on top of the preceding
    // `2147483647` it resolves to `-1`, leaving `====== Deep` (syntactic level
    // 5) at level 4.
    let doc = Parser::default().parse(concat!(
        "= My Book\n\n",
        ":leveloffset: 2147483647\n\n",
        ":leveloffset: -2147483648\n\n",
        "====== Deep",
    ));

    assert_eq!(section_levels(&doc), vec![(4, "Deep".to_string())]);

    // Only the first (absolute, out-of-range) offset warns; the resolved `-1`
    // is usable, so no impossible-offset or clamp warning follows.
    assert_eq!(
        warning_types(&doc),
        vec![WarningType::LeveloffsetExcludesAllHeadingLevels(i32::MAX)]
    );
}
