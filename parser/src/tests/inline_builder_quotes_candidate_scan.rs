//! Pins the quotes step's candidate scan ([`find_matches`]) against the
//! unanchored sweep it replaced ([`reference_find_matches`]) and each sub's
//! scan needle ([`candidate_needle`]) against its own pattern.
//!
//! These live here rather than in the quotes module's own tests so their
//! lines stay out of the coverage measurement on every platform (this
//! directory is excluded when the lcov report is generated).
//!
//! [`find_matches`]: crate::content::inline_builder::find_matches
//! [`reference_find_matches`]: crate::content::inline_builder::reference_find_matches
//! [`candidate_needle`]: crate::content::inline_builder::candidate_needle

use crate::{
    content::inline_builder::{
        candidate_needle, closing_needle, find_matches, quote_subs, reference_find_matches,
    },
    parser::{QuoteScope, QuoteType},
};

#[test]
fn every_quote_sub_has_a_candidate_needle() {
    // The candidate scan hops between occurrences of each sub's own
    // opening delimiter, so a real sub answering an empty needle would
    // make its scan find nothing at all — and a needle that is not
    // literally the text the pattern's own opening delimiter spells
    // (escapes stripped) would skip real constructs. The three types no
    // `QuoteSub` carries are the only ones allowed to answer empty,
    // mirroring `sub_markers`'s empty answer for them.
    for sub in quote_subs() {
        let needle = candidate_needle(sub.type_, sub.scope);

        assert!(
            !needle.is_empty(),
            "{:?}/{:?} has no candidate needle",
            sub.type_,
            sub.scope
        );

        // The needle must be the pattern's own opening delimiter: the
        // literal run its source spells right after the optional-attrs
        // group, with regex escapes stripped.
        let (_, tail) = sub.source.split_once(r#"(?:\[([^\[\]]+)\])?"#).unwrap();
        let opening = tail.replace('\\', "");
        let needle_text = std::str::from_utf8(needle).unwrap();

        assert!(
            opening.as_bytes().starts_with(needle),
            "{:?}/{:?}: needle {needle_text:?} is not how {tail:?} opens",
            sub.type_,
            sub.scope,
        );

        // And the closing needle must be how the pattern's tail closes: the
        // literal run before the trailing zero-width boundary assertion,
        // escapes stripped. The span-decomposing group derivation
        // (`derive_groups`) trusts both needles as the match's own edges.
        let closing = closing_needle(sub.type_, sub.scope);
        let closing_text = std::str::from_utf8(closing).unwrap();
        let end = tail.strip_suffix(r"\b{end-half}").unwrap_or(tail);
        let closing_literal = end.replace('\\', "");

        assert!(
            closing_literal.as_bytes().ends_with(closing),
            "{:?}/{:?}: closing needle {closing_text:?} is not how {tail:?} ends",
            sub.type_,
            sub.scope,
        );
    }

    for type_ in [
        QuoteType::Unquoted,
        QuoteType::AsciiMath,
        QuoteType::LatexMath,
    ] {
        for scope in [QuoteScope::Constrained, QuoteScope::Unconstrained] {
            assert!(candidate_needle(type_, scope).is_empty(), "{type_:?}");
        }
    }
}

#[test]
fn candidate_scan_matches_the_unanchored_reference() {
    // The candidate scan must enumerate exactly the matches the
    // unanchored sweep it replaced would have found, in the same order,
    // with the same capture ranges. The fixtures concentrate on the
    // shapes the scan's start enumeration has to get right: attribute
    // lists that contain marker bytes (the one way a match starts well
    // before the candidate that anchors it), escapes riding the prefix
    // class, doubled and tripled delimiters, matches at either edge,
    // multi-byte characters beside delimiters, unclosed brackets, and
    // the constrained-monospace retry.
    let fixtures = [
        // Nothing to find.
        "",
        "plain prose with none of it",
        // The plain shapes, at every position.
        "*b*",
        "a *b* c",
        "*b* tail",
        "head *b*",
        "**b**",
        "a **b** c",
        "***b***",
        "*a*b*",
        "**a**b**",
        "_i_ and __ii__",
        "`m` and ``mm``",
        "#h# and ##hh##",
        "^s^ x ~t~",
        "\"`dq`\" '`sq`'",
        // Escapes (constrained: the backslash is the prefix char;
        // unconstrained: its own optional group).
        r"\*b*",
        r"a \*b* c",
        r"\**b**",
        r"\[a]*b*",
        r"[a]\*b*",
        // Attribute lists, including marker bytes *inside* the list —
        // the leftmost-faithfulness case.
        "[a]*b*",
        "x [.role]#m#",
        "[a*b]*c*",
        "[z#z]#A#",
        ".[z#z]#A#",
        "x[a#b]#body#",
        "[*]*b*",
        "[a]b *c*",
        // Unclosed and empty brackets.
        "[open *b*",
        "[]*b*",
        "]*b*",
        "[a][b]*c*",
        "*[a]b*",
        // Word-adjacent (constrained boundary failures).
        "a*b*c",
        "1*2*3",
        "*b*word",
        // Doubled-delimiter interplay.
        "**a* b*",
        "*a **b** c*",
        "``a` b`",
        // Multi-byte characters beside markers.
        "é*ü*é",
        "→*b*←",
        "«*b*»",
        "*é*",
        // Multi-line bodies (dot_matches_new_line).
        "*a\nb*",
        "**a\nb**",
        // The constrained-monospace retry chain.
        "`a`'",
        "x `a`' y",
        "`a`'`b`'",
        r"\`a`'",
        "`a`\"",
        "`a`` b``",
        // Smart quotes beside their own markers.
        "\"`a`\"b",
        "'`a`'.",
        "\"`a`'",
        // Prose full of the smart-quote subs' *first* bytes with no
        // construct — the shape the whole-delimiter needle exists for —
        // and the overlap traps beside it: a lone first byte directly
        // before a real construct, and a needle occurrence split across
        // an apostrophe and a monospace span.
        "It's the author's own text, \"quoted\" the plain way.",
        "''`a`'",
        "\"\"`a`\"",
        "it'`s`'",
        "'`a`''`b`'",
        // Dense soup.
        "*a* [b*c]*d* \\*e* [f]#g# `h`' **i**",
    ];

    for sub in quote_subs() {
        for fixture in fixtures {
            assert_eq!(
                find_matches(sub, fixture),
                reference_find_matches(sub, fixture),
                "candidate scan diverged for {:?}/{:?} on {fixture:?}",
                sub.type_,
                sub.scope,
            );
        }
    }
}
