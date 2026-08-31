//! Pins the macros step's shared trigger-byte gate
//! ([`level_may_have_macros`]) against every macro family's own sniff
//! needles, so a future needle cannot silently fall outside the byte class
//! and have its family skipped for a lone-text level.
//!
//! [`level_may_have_macros`]: crate::content::inline_builder::level_may_have_macros

use crate::content::inline_builder::level_may_have_macros;

#[test]
fn every_macro_family_needle_carries_a_gate_byte() {
    // The gate in `apply_macros` skips the whole step for a lone `Text`
    // value holding none of its five bytes, so a minimal construct of
    // every family — spelled as the *value* the step reads, meaning
    // post-escaping for the xref's angle form — must answer `true`, or
    // the gate would silently disable that family.
    for construct in [
        "[[[biblio]]]",
        "[[id]]",
        "anchor:id[]",
        "image:a.png[alt]",
        "icon:heart[]",
        "kbd:[F1]",
        "btn:[OK]",
        "menu:File[Save]",
        "((term))",
        "indexterm:[primary]",
        "indexterm2:[shown]",
        "https://example.com",
        "link:page.html[text]",
        "mailto:a@example.org[]",
        "doc@example.org",
        "xref:section[]",
        "&lt;&lt;section&gt;&gt;",
    ] {
        assert!(
            level_may_have_macros(construct),
            "the gate would wrongly skip {construct:?}"
        );
    }

    // Each of the five bytes opens the gate on its own, so no family's
    // needle rides on a byte the class does not carry.
    for value in [":", "[", "(", "@", "&"] {
        assert!(
            level_may_have_macros(value),
            "gate byte {value:?} does not open the gate"
        );
    }

    // And the shape the gate exists for answers `false`.
    assert!(!level_may_have_macros(
        "plain prose with nothing any macro family recognizes"
    ));
}
