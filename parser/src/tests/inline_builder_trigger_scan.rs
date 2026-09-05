//! Pins the fused trigger scan's per-step byte classes (see
//! `content::inline_builder::trigger_scan`) against each step's own
//! recognizers, so a future needle cannot silently fall outside its step's
//! class and have a construct skipped for a lone-text level.
//!
//! Each class must be **conservative**: whenever a step's own sniff could
//! answer `true`, the value must carry at least one byte of that step's
//! class. The pins below spell every sniff's alternatives out as minimal
//! trigger strings — over-naming a byte costs one redundant call, but a
//! trigger string whose bytes all fall outside its class would disable the
//! construct, which is what these catch.

use crate::content::inline_builder::{
    ATTRIBUTE_TRIGGERS, MACRO_TRIGGERS, PASSTHROUGH_TRIGGERS, POST_REPLACEMENT_TRIGGERS,
    QUOTE_TRIGGERS, REPLACEMENT_TRIGGERS, SPECIAL_TRIGGERS, STEM_TRIGGERS, level_may_have_macros,
    maybe_has_replacements, trigger_mask,
};

/// Asserts that every one of `triggers` opens `class`.
fn assert_all_open(class: u32, triggers: &[&str], step: &str) {
    for trigger in triggers {
        assert_ne!(
            trigger_mask(trigger) & class,
            0,
            "{step}: trigger {trigger:?} carries no byte of its step's class"
        );
    }
}

#[test]
fn every_step_sniff_trigger_opens_its_class() {
    // Passthrough extraction: both internal phases' needles (`++`, `$$`,
    // `ss:` — the tail shared by `pass:` and `subs:`-attributed forms — and
    // the bare/attrlisted single-`+` phase's `+` and `-]`).
    assert_all_open(
        PASSTHROUGH_TRIGGERS,
        &["++", "$$", "ss:", "+", "-]"],
        "passthroughs",
    );

    // Inline STEM: every macro spelling its sniff admits.
    assert_all_open(
        STEM_TRIGGERS,
        &["stem:", "asciimath:", "latexmath:"],
        "stem",
    );

    // Specialcharacters (and the end-of-group unescaped-specials sweep).
    assert_all_open(SPECIAL_TRIGGERS, &["<", ">", "&"], "specialcharacters");

    // Quotes: a minimal construct of every quote sub. Each sub requires all
    // of its own markers, so one byte per construct in the class suffices.
    assert_all_open(
        QUOTE_TRIGGERS,
        &[
            "**b**", "*b*", "\"`b`\"", "'`b`'", "``b``", "`b`", "__b__", "_b_", "##b##", "#b#",
            "^b^", "~b~",
        ],
        "quotes",
    );

    // Attribute references and counter directives.
    assert_all_open(ATTRIBUTE_TRIGGERS, &["{attr}", "{counter:n}"], "attributes");

    // Character replacements: one trigger per alternative of the step's own
    // sniff pattern, pinned exhaustively against it below.
    assert_all_open(
        REPLACEMENT_TRIGGERS,
        &["&amp;", "'", "--", "...", "(C)", "(R)", "(TM)"],
        "replacements",
    );

    // Macros (the footnotes pass rides on the same gate; both of its
    // spellings carry the colon).
    assert_all_open(
        MACRO_TRIGGERS,
        &["footnote:[x]", "footnoteref:[x,id]"],
        "macros",
    );

    // Post-replacements: the ` +` break marker, and — under the `hardbreaks`
    // option, where no `+` is needed — the newline every break then rides on.
    assert_all_open(
        POST_REPLACEMENT_TRIGGERS,
        &["a +", "\n"],
        "post-replacements",
    );
}

#[test]
fn single_byte_sniffs_are_covered_exhaustively() {
    // For the two steps whose own sniff is reachable from here, check every
    // single-character value: wherever the step's sniff already answers
    // `true`, the fused class must too. (Multi-character alternatives are
    // pinned as whole trigger strings above.)
    for b in 0u8..=127 {
        let s = (b as char).to_string();

        if level_may_have_macros(&s) {
            assert_ne!(
                trigger_mask(&s) & MACRO_TRIGGERS,
                0,
                "macros: byte {b:#04x} opens the step's own gate but not its class"
            );
        }

        if maybe_has_replacements(&s) {
            assert_ne!(
                trigger_mask(&s) & REPLACEMENT_TRIGGERS,
                0,
                "replacements: byte {b:#04x} opens the step's own sniff but not its class"
            );
        }
    }
}
