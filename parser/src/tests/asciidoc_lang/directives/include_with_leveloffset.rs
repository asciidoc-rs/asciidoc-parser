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
