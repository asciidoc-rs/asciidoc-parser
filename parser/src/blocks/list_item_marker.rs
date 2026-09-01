use std::sync::LazyLock;

use regex::Regex;

use crate::{
    HasSpan, Parser, Span,
    content::{Content, SubstitutionGroup, SubstitutionStep},
    span::MatchedItem,
    warnings::Warning,
};

/// The substitution group a description-list **term** takes: the `normal`
/// group's own order, minus the attribute-references step that already ran
/// during parsing (so `{blank}` and friends were resolved before the marker
/// was recognized). Asciidoctor substitutes a term with `normal` too; this
/// spelling is the same list with the one already-applied step removed.
static TERM_SUBSTITUTIONS: LazyLock<SubstitutionGroup> = LazyLock::new(|| {
    SubstitutionGroup::Custom(vec![
        SubstitutionStep::SpecialCharacters,
        SubstitutionStep::Quotes,
        SubstitutionStep::CharacterReplacements,
        SubstitutionStep::Macros,
        SubstitutionStep::PostReplacement,
    ])
});

/// A list item is signaled by one of several designated marker sequences.
#[derive(Clone, Eq, Hash, PartialEq)]
pub enum ListItemMarker<'src> {
    /// Unordered list (hyphen).
    Hyphen(Span<'src>),

    /// Unordered list (asterisks).
    Asterisks(Span<'src>),

    /// Unordered list (Unicode bullet).
    Bullet(Span<'src>),

    /// Ordered list (dots).
    Dots(Span<'src>),

    /// Uppercase letter followed by dot (alpha list).
    AlphaListCapital(Span<'src>),

    /// Lowercase letter followed by dot (alpha list).
    AlphaListLower(Span<'src>),

    /// Lowercase Roman numeral followed by closing paren.
    RomanNumeralLower(Span<'src>),

    /// Uppercase Roman numeral followed by closing paren.
    RomanNumeralUpper(Span<'src>),

    /// Explicit Arabic numeral followed by dot (e.g., "7.").
    ArabicNumeral(Span<'src>),

    /// A callout list marker (`<1>` or `<.>`), used to annotate lines in a
    /// preceding verbatim block.
    Callout(Span<'src>),

    /// A term to be defined.
    DefinedTerm {
        /// The name of the term being defined.
        term: Content<'src>,

        /// The marker (`::`, etc.) used to call out the definition.
        marker: Span<'src>,

        /// The source span for the entire term assembly.
        source: Span<'src>,
    },
}

impl<'src> ListItemMarker<'src> {
    pub(crate) fn starts_with_marker(source: Span<'src>) -> bool {
        // Discard leading whitespace before matching, mirroring `parse` (which
        // does the same for every marker kind), so both marker regexes see the
        // same input.
        let source = source.discard_whitespace();

        (may_be_list_item_marker(source.data()) && LIST_ITEM_MARKER.is_match(source.data()))
            || (may_be_callout_marker(source.data()) && CALLOUT_LIST_MARKER.is_match(source.data()))
    }

    pub(crate) fn parse(source: Span<'src>, parser: &Parser) -> Option<MatchedItem<'src, Self>> {
        let source = source.discard_whitespace();

        // A callout list marker (`<1>` or `<.>`) is not matched by
        // `LIST_ITEM_MARKER`, so it is checked first.
        if may_be_callout_marker(source.data())
            && let Some(captures) = CALLOUT_LIST_MARKER.captures(source.data())
        {
            let marker = source.slice(0..captures[1].len());
            let after = source.slice_from(captures[1].len()..).discard_whitespace();

            return Some(MatchedItem {
                item: Self::Callout(marker),
                after,
            });
        }

        if may_be_list_item_marker(source.data())
            && let Some(captures) = LIST_ITEM_MARKER.captures(source.data())
        {
            let marker = source.slice(0..captures[1].len());
            let marker_str = marker.data();
            let after = source.slice_from(captures[1].len()..).discard_whitespace();

            let first_char = captures[1].chars().next();

            let item = if marker_str == "-" {
                Self::Hyphen(marker)
            } else if marker_str.starts_with('*') {
                Self::Asterisks(marker)
            } else if marker_str == "•" {
                Self::Bullet(marker)
            } else if marker_str.starts_with('.') {
                Self::Dots(marker)
            } else if let Some(first_char) = first_char
                && first_char.is_ascii_uppercase()
                && marker_str.ends_with('.')
            {
                Self::AlphaListCapital(marker)
            } else if let Some(first_char) = first_char
                && first_char.is_ascii_lowercase()
                && marker_str.ends_with('.')
            {
                Self::AlphaListLower(marker)
            } else if marker_str.ends_with(')')
                && marker_str
                    .chars()
                    .take(marker_str.len() - 1)
                    .all(|c| "ivxlcdm".contains(c))
            {
                Self::RomanNumeralLower(marker)
            } else if marker_str.ends_with(')')
                && marker_str
                    .chars()
                    .take(marker_str.len() - 1)
                    .all(|c| "IVXLCDM".contains(c))
            {
                Self::RomanNumeralUpper(marker)
            } else if marker_str.ends_with('.')
                && marker_str
                    .chars()
                    .take(marker_str.len() - 1)
                    .all(|c| c.is_ascii_digit())
            {
                Self::ArabicNumeral(marker)
            } else {
                // Regex and if-else chain should be exhaustive. If not, treat
                // as non-match.
                return None;
            };

            return Some(MatchedItem { item, after });
        }

        // Don't match description list markers in comment lines.
        // Comment lines start with // but not /// (which is a valid term).
        let source_data = source.data();
        if source_data.starts_with("//") && !source_data.starts_with("///") {
            return None;
        }

        // The gate spares the engine both the scan that ends in a
        // rejected non-zero-offset match and — on the common
        // marker-free line — the whole search.
        if !first_line_may_hold_dlist_marker(source_data) {
            return None;
        }

        let captures = DESCRIPTION_LIST_MARKER.captures(source_data)?;

        // With multi-line mode enabled, ^ can match at any line start.
        // We only accept matches that start at the beginning of the source.
        let full_match = captures.get(0)?;
        if full_match.start() != 0 {
            return None;
        }

        let after = source.slice_from(full_match.end()..).discard_whitespace();

        let source = source
            .slice_to(..full_match.end())
            .trim_trailing_whitespace();

        let term_len = captures[1].len();
        let term = source.slice(0..term_len);
        let mut term: Content<'src> = term.into();

        // Apply attribute substitution to the term so that attribute
        // references like `{blank}` are resolved before
        // determining if this is a valid definition list
        // marker.
        SubstitutionStep::AttributeReferences.apply(&mut term, parser, None);

        let marker = source.slice_from(term_len..);

        Some(MatchedItem {
            item: Self::DefinedTerm {
                term,
                marker,
                source,
            },
            after,
        })
    }

    /// Apply the term's inline substitutions and register any leading inline
    /// anchors found in a description list term.
    ///
    /// This should be called after parsing a `DefinedTerm` marker when the list
    /// item is being kept (not just checked for existence). A description-list
    /// term receives the full `normal` substitution group, matching
    /// Asciidoctor, so `&`, `<`, and `>` are escaped and inline formatting is
    /// rendered. It also detects anchors like `[[id]]` or `[[id,reftext]]` at
    /// the start of the term text, registers them in the catalog, and renders
    /// the anchor.
    ///
    /// This method is a no-op for non-`DefinedTerm` markers.
    pub(crate) fn register_leading_anchors(
        &mut self,
        parser: &mut Parser,
        warnings: &mut Vec<Warning<'src>>,
    ) {
        let Self::DefinedTerm {
            term,
            marker: _,
            source: _,
        } = self
        else {
            return;
        };

        // A description-list term is substituted with the `normal` group, in
        // that group's own order minus its attribute-references step, which
        // already ran during parsing so the marker could be recognized at all.
        // Running it through `SubstitutionGroup::apply` rather than the steps
        // directly is what gives the term a **tree**: the seed, the
        // authoritative fold, and the replay of every recognition side effect
        // come with it, so a term no longer registers from the string
        // pipeline (this was the last content that did).
        TERM_SUBSTITUTIONS.apply_to_description_list_term(term, parser, warnings);
    }

    /// Return a mutable reference to the term content of a description-list
    /// marker, or `None` for other marker kinds.
    pub(crate) fn term_mut(&mut self) -> Option<&mut Content<'src>> {
        match self {
            Self::DefinedTerm { term, .. } => Some(term),
            _ => None,
        }
    }

    /// Returns the explicit number of a `<N>` callout marker, or `None` for an
    /// automatically-numbered (`<.>`) callout or any non-callout marker.
    pub(crate) fn callout_number(&self) -> Option<u32> {
        match self {
            Self::Callout(span) => span
                .data()
                .trim_start_matches('<')
                .trim_end_matches('>')
                .parse::<u32>()
                .ok(),
            _ => None,
        }
    }

    /// Test for equality, disregarding span offsets.
    pub(crate) fn is_match_for(&self, other: &Self) -> bool {
        match self {
            Self::Hyphen(self_span) => match other {
                Self::Hyphen(other_span) => self_span.data() == other_span.data(),
                _ => false,
            },

            Self::Asterisks(self_span) => match other {
                Self::Asterisks(other_span) => self_span.data() == other_span.data(),
                _ => false,
            },

            Self::Bullet(self_span) => match other {
                Self::Bullet(other_span) => self_span.data() == other_span.data(),
                _ => false,
            },

            Self::Dots(self_span) => match other {
                Self::Dots(other_span) => self_span.data() == other_span.data(),
                _ => false,
            },

            Self::AlphaListCapital(_self_span) => {
                matches!(other, Self::AlphaListCapital(_other_span))
            }

            Self::AlphaListLower(_self_span) => {
                matches!(other, Self::AlphaListLower(_other_span))
            }

            Self::RomanNumeralLower(_self_span) => {
                matches!(other, Self::RomanNumeralLower(_other_span))
            }

            Self::RomanNumeralUpper(_self_span) => {
                matches!(other, Self::RomanNumeralUpper(_other_span))
            }

            Self::ArabicNumeral(_self_span) => {
                matches!(other, Self::ArabicNumeral(_other_span))
            }

            Self::Callout(_self_span) => {
                matches!(other, Self::Callout(_other_span))
            }

            Self::DefinedTerm {
                term: _,
                marker: self_marker,
                source: _,
            } => match other {
                Self::DefinedTerm {
                    term: _,
                    marker: other_marker,
                    source: _,
                } => self_marker.data() == other_marker.data(),
                _ => false,
            },
        }
    }

    /// Returns the ordinal value for explicit markers, or `None` for implicit
    /// markers.
    ///
    /// Explicit markers like `x.`, `7.`, or `iv)` have specific sequence
    /// values. Implicit markers like `.` or `*` don't have ordinal values.
    pub(crate) fn ordinal_value(&self) -> Option<u32> {
        match self {
            Self::AlphaListLower(span) => {
                // "x." -> 24 (1-indexed: a=1, b=2, ..., x=24)
                let ch = span.data().chars().next()?;
                Some((ch as u32) - ('a' as u32) + 1)
            }

            Self::AlphaListCapital(span) => {
                // "X." -> 24 (1-indexed: A=1, B=2, ..., X=24)
                let ch = span.data().chars().next()?;
                Some((ch as u32) - ('A' as u32) + 1)
            }

            Self::ArabicNumeral(span) => {
                // "7." -> 7
                span.data().trim_end_matches('.').parse().ok()
            }

            Self::RomanNumeralLower(span) => {
                // "xvii)" -> 17
                parse_roman_numeral(span.data().trim_end_matches(')'))
            }

            Self::RomanNumeralUpper(span) => {
                // "XVII)" -> 17
                parse_roman_numeral(span.data().trim_end_matches(')'))
            }

            // Implicit markers (dots, asterisks, etc.) don't have ordinal values.
            _ => None,
        }
    }

    /// Converts an ordinal value back to the display form for this marker type.
    ///
    /// Used to generate warning messages about expected vs. actual sequence
    /// values.
    pub(crate) fn ordinal_to_marker_text(&self, ordinal: u32) -> Option<String> {
        match self {
            Self::AlphaListLower(_) => {
                // 24 -> "x"
                char::from_u32('a' as u32 + ordinal - 1).map(|c| c.to_string())
            }

            Self::AlphaListCapital(_) => {
                // 24 -> "X"
                char::from_u32('A' as u32 + ordinal - 1).map(|c| c.to_string())
            }

            Self::ArabicNumeral(_) => {
                // 7 -> "7"
                Some(ordinal.to_string())
            }

            Self::RomanNumeralLower(_) => {
                // 17 -> "xvii"
                Some(to_roman_numeral_lower(ordinal))
            }

            Self::RomanNumeralUpper(_) => {
                // 17 -> "XVII"
                Some(to_roman_numeral_upper(ordinal))
            }

            // Implicit markers don't have ordinal display forms.
            _ => None,
        }
    }
}

impl<'src> HasSpan<'src> for ListItemMarker<'src> {
    fn span(&self) -> Span<'src> {
        match self {
            Self::Hyphen(x) => *x,
            Self::Asterisks(x) => *x,
            Self::Bullet(x) => *x,
            Self::Dots(x) => *x,
            Self::AlphaListCapital(x) => *x,
            Self::AlphaListLower(x) => *x,
            Self::RomanNumeralLower(x) => *x,
            Self::RomanNumeralUpper(x) => *x,
            Self::ArabicNumeral(x) => *x,
            Self::Callout(x) => *x,

            Self::DefinedTerm {
                term: _,
                marker: _,
                source,
            } => *source,
        }
    }
}

impl std::fmt::Debug for ListItemMarker<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Hyphen(x) => f.debug_tuple("ListItemMarker::Hyphen").field(x).finish(),
            Self::Asterisks(x) => f.debug_tuple("ListItemMarker::Asterisks").field(x).finish(),
            Self::Bullet(x) => f.debug_tuple("ListItemMarker::Bullet").field(x).finish(),
            Self::Dots(x) => f.debug_tuple("ListItemMarker::Dots").field(x).finish(),

            Self::AlphaListCapital(x) => f
                .debug_tuple("ListItemMarker::AlphaListCapital")
                .field(x)
                .finish(),

            Self::AlphaListLower(x) => f
                .debug_tuple("ListItemMarker::AlphaListLower")
                .field(x)
                .finish(),

            Self::RomanNumeralLower(x) => f
                .debug_tuple("ListItemMarker::RomanNumeralLower")
                .field(x)
                .finish(),

            Self::RomanNumeralUpper(x) => f
                .debug_tuple("ListItemMarker::RomanNumeralUpper")
                .field(x)
                .finish(),

            Self::ArabicNumeral(x) => f
                .debug_tuple("ListItemMarker::ArabicNumeral")
                .field(x)
                .finish(),

            Self::Callout(x) => f.debug_tuple("ListItemMarker::Callout").field(x).finish(),

            Self::DefinedTerm {
                term,
                marker,
                source,
            } => f
                .debug_struct("ListItemMarker::DefinedTerm")
                .field("term", term)
                .field("marker", marker)
                .field("source", source)
                .finish(),
        }
    }
}

/// The bytes that can spell a Roman-numeral list marker, either case.
const ROMAN_NUMERAL_BYTES: &[u8] = b"ivxlcdmIVXLCDM";

/// A byte-level gate over [`CALLOUT_LIST_MARKER`]: every callout marker opens
/// with `<`, so a line that doesn't cannot match — and almost no ordinary
/// line starts with one.
fn may_be_callout_marker(s: &str) -> bool {
    s.as_bytes().first() == Some(&b'<')
}

/// A byte-level gate over [`LIST_ITEM_MARKER`], reading at most the first two
/// bytes: every alternative the pattern can match opens with one of a small
/// byte class, and its second byte is constrained enough to reject nearly
/// every ordinary prose line — whose first word continues with a second
/// letter — before the regex engine is consulted. The gate only ever errs
/// toward `true` (a mixed-case Roman run, a long asterisk run with no
/// trailing space); `a_gated_line_never_matches_its_marker_regex` pins that
/// no line it rejects is one the regex would have matched.
fn may_be_list_item_marker(s: &str) -> bool {
    let bytes = s.as_bytes();

    let Some(&first) = bytes.first() else {
        return false;
    };

    let second = bytes.get(1).copied();

    match first {
        // `-` is a single hyphen; its required whitespace follows directly.
        b'-' => matches!(second, Some(b' ' | b'\t')),

        // A `*`/`.` run continues, or ends at its required whitespace.
        b'*' => matches!(second, Some(b' ' | b'\t' | b'*')),
        b'.' => matches!(second, Some(b' ' | b'\t' | b'.')),

        // `\d+\.`: more digits, or the closing dot.
        b'0'..=b'9' => matches!(second, Some(b'.' | b'0'..=b'9')),

        // A letter opens `[a-zA-Z]\.` or a Roman-numeral run — anything
        // else, every ordinary word included, is rejected here.
        first if first.is_ascii_alphabetic() => match second {
            Some(b'.') => true,
            Some(b')') => ROMAN_NUMERAL_BYTES.contains(&first),
            Some(second) => {
                ROMAN_NUMERAL_BYTES.contains(&first) && ROMAN_NUMERAL_BYTES.contains(&second)
            }
            None => false,
        },

        // `•` (U+2022) opens with this byte; the rare full check stays the
        // regex's.
        0xe2 => true,

        _ => false,
    }
}

/// A byte-level gate over [`DESCRIPTION_LIST_MARKER`]: the only match `parse`
/// accepts starts at offset zero, and since the term's `.` cannot cross a
/// newline, the term and its `::`/`;;` delimiter both sit on the first line —
/// so a first line carrying neither delimiter cannot produce an accepted
/// match. (The multiline pattern could still match on a *later* line, but
/// `parse` has always rejected those; the gate also spares the engine that
/// scan-ahead.)
fn first_line_may_hold_dlist_marker(s: &str) -> bool {
    let line = s.split('\n').next().unwrap_or(s);

    line.contains("::") || line.contains(";;")
}

static LIST_ITEM_MARKER: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(
        r#"(?x)
            ^(                      # Capture group for list marker
                -                       # Hyphen (unordered list)
                |\*+                    # One or more asterisks (unordered list, up to 5 levels)
                |\.+                    # One or more dots (ordered list, up to 5 levels)
                |\u{2022}               # Bullet character • (unordered list)
                |\d+\.                  # Digits followed by dot (numbered list)
                |[a-zA-Z]\.             # Letter followed by dot (alpha list)
                |[ivxlcdm]+\)           # Lowercase Roman numerals followed by )
                |[IVXLCDM]+\)           # Uppercase Roman numerals followed by )
            )
            [\ \t]                  # Required whitespace after marker
        "#,
    )
    .unwrap()
});

/// Matches a callout list item marker: `<` followed by a number or `.`, then
/// `>`, then required whitespace. Mirrors Asciidoctor's `CalloutListRx`.
static CALLOUT_LIST_MARKER: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(
        r#"(?x)
            ^                       # Start of line
            (                       # Capture group 1: the marker
                <(?:\d+|\.)>            # `<` then digits or a dot, then `>`
            )
            [\ \t]                  # Required whitespace after marker
        "#,
    )
    .unwrap()
});

static DESCRIPTION_LIST_MARKER: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(
        r#"(?xm)
            ^                       # Start of line
            (                       # Capture group 1: Term being defined
                [^\ \t]                 # At least one non-whitespace character (start of term)
                .*?                     # Any characters (rest of term, non-greedy)
            )
            (?::::?:?|;;)           # Delimiter: ::, :::, ::::, or ;;
            (?:$|[\ \t])            # End of line or whitespace after marker
        "#,
    )
    .unwrap()
});

/// Parses a lowercase Roman numeral string into its numeric value.
fn parse_roman_numeral(s: &str) -> Option<u32> {
    let mut result: u32 = 0;
    let mut prev_value: u32 = 0;

    for ch in s.chars().rev() {
        let value = match ch {
            'i' | 'I' => 1,
            'v' | 'V' => 5,
            'x' | 'X' => 10,
            'l' | 'L' => 50,
            'c' | 'C' => 100,
            'd' | 'D' => 500,
            'm' | 'M' => 1000,
            _ => return None,
        };

        if value < prev_value {
            result -= value;
        } else {
            result += value;
        }
        prev_value = value;
    }

    if result > 0 { Some(result) } else { None }
}

/// Converts a numeric value to a lowercase Roman numeral string.
fn to_roman_numeral_lower(mut n: u32) -> String {
    const NUMERALS: &[(u32, &str)] = &[
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
    ];

    let mut result = String::new();
    for &(value, numeral) in NUMERALS {
        while n >= value {
            result.push_str(numeral);
            n -= value;
        }
    }
    result
}

/// Converts a numeric value to an uppercase Roman numeral string.
fn to_roman_numeral_upper(mut n: u32) -> String {
    const NUMERALS: &[(u32, &str)] = &[
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];

    let mut result = String::new();
    for &(value, numeral) in NUMERALS {
        while n >= value {
            result.push_str(numeral);
            n -= value;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use crate::{span::MatchedItem, tests::prelude::*};

    fn lim_parse<'a>(
        source: &'a str,
    ) -> Option<MatchedItem<'a, crate::blocks::ListItemMarker<'a>>> {
        let parser = Parser::default();
        crate::blocks::ListItemMarker::parse(crate::Span::new(source), &parser)
    }

    /// Parses `source` as a description-list marker and returns the term text
    /// after the term's inline substitutions have run (i.e. after
    /// `register_leading_anchors`).
    fn term_rendered(source: &str) -> String {
        let mut parser = Parser::default();

        let mut item = crate::blocks::ListItemMarker::parse(crate::Span::new(source), &parser)
            .unwrap()
            .item;

        let mut warnings = vec![];
        item.register_leading_anchors(&mut parser, &mut warnings);
        assert!(warnings.is_empty());

        match &item {
            crate::blocks::ListItemMarker::DefinedTerm { term, .. } => {
                term.rendered_html().to_string()
            }
            other => panic!("expected a defined-term marker, got {other:#?}"),
        }
    }

    #[test]
    fn a_gated_line_never_matches_its_marker_regex() {
        // The byte gates in front of the marker regexes may only ever err
        // toward running the regex: a line a gate rejects must be one its
        // regex rejects too (for the description-list gate: one whose match
        // `parse` would reject anyway, an accepted match starting at offset
        // zero on the first line). The corpus walks every gate branch from
        // both sides — each first-byte class with an accepting and a
        // rejecting second byte, and the gate-true-but-regex-false shapes
        // the gates deliberately leave to the engine.
        use super::{
            CALLOUT_LIST_MARKER, DESCRIPTION_LIST_MARKER, LIST_ITEM_MARKER,
            first_line_may_hold_dlist_marker, may_be_callout_marker, may_be_list_item_marker,
        };

        let lines = [
            "",
            // Hyphen.
            "- item",
            "-\titem",
            "-item",
            "-",
            // Asterisk runs.
            "* item",
            "** item",
            "*item",
            "*",
            "**no-space",
            // Dot runs.
            ". item",
            ".. item",
            ".item",
            ".",
            // Bullet (and another codepoint sharing its first byte).
            "\u{2022} item",
            "\u{2022}item",
            "\u{2014} not a bullet",
            // Arabic numerals.
            "9. item",
            "99. item",
            "9x",
            "9",
            "99",
            // Alpha lists.
            "a. item",
            "Z. item",
            "a) item",
            "a",
            // Roman numerals, both cases — and a mixed-case run only the
            // regex rejects.
            "i) item",
            "xiv) item",
            "IX) item",
            "iV) item",
            "ix item",
            // Ordinary prose.
            "The renderer walks the tree",
            "it",
            "词 leading multibyte",
            // Callouts.
            "<1> item",
            "<.> item",
            "<1>no-space",
            "x<1>",
            // Description lists.
            "term:: definition",
            "term;; definition",
            "term::",
            "no delimiter here",
            "//term:: in a comment",
            "later\nterm:: definition",
        ];

        for line in lines {
            if !may_be_list_item_marker(line) {
                assert!(
                    !LIST_ITEM_MARKER.is_match(line),
                    "gate rejected {line:?}, which the list-item regex matches"
                );
            }

            if !may_be_callout_marker(line) {
                assert!(
                    !CALLOUT_LIST_MARKER.is_match(line),
                    "gate rejected {line:?}, which the callout regex matches"
                );
            }

            if !first_line_may_hold_dlist_marker(line) {
                assert!(
                    DESCRIPTION_LIST_MARKER
                        .captures(line)
                        .and_then(|caps| caps.get(0))
                        .is_none_or(|whole| whole.start() != 0),
                    "gate rejected {line:?}, whose description-list match \
                     `parse` would accept"
                );
            }
        }
    }

    #[test]
    fn term_special_characters_are_escaped() {
        // A description-list term receives the full `normal` substitution
        // group, so the special characters `&`, `<`, and `>` are
        // escaped rather than passed through verbatim. The horizontal
        // and qanda list variants share this code path, since their
        // terms are the same `DefinedTerm` marker.
        assert_eq!(term_rendered("a & b:: desc"), "a &amp; b");
        assert_eq!(term_rendered("a < b:: desc"), "a &lt; b");
        assert_eq!(term_rendered("a > b:: desc"), "a &gt; b");

        // The remaining `normal` steps also apply: inline formatting (quotes)
        // and character replacements are rendered in the term.
        assert_eq!(term_rendered("*bold*:: desc"), "<strong>bold</strong>");
        assert_eq!(term_rendered("A(C):: desc"), "A&#169;");
        assert_eq!(term_rendered("A(TM):: desc"), "A&#8482;");
    }

    #[test]
    fn term_passthrough_payload_is_protected() {
        // Passthrough spans in a term are extracted before the substitution
        // steps run and restored afterward, so their payloads bypass special
        // characters and quotes exactly as they would in ordinary content.
        assert_eq!(term_rendered("+++<b>a & b</b>+++:: desc"), "<b>a & b</b>");
        assert_eq!(term_rendered("pass:[<b>]:: desc"), "<b>");

        // Only special characters are applied inside a double-plus span.
        assert_eq!(term_rendered("++<b>++:: desc"), "&lt;b&gt;");
    }

    #[test]
    fn term_with_bracket_prefix_but_no_valid_anchor() {
        // A term that starts with `[[` but is not a well-formed inline anchor
        // (an ID may not start with a digit, and the anchor must be closed) is
        // not registered as a reference; the bracketed text is left as-is by
        // the macros step and no warning is emitted.
        assert_eq!(term_rendered("[[1bad]] foo:: desc"), "[[1bad]] foo");
        assert_eq!(term_rendered("[[ nope:: desc"), "[[ nope");
    }

    #[test]
    fn hyphen() {
        assert!(lim_parse("-").is_none());
        assert!(lim_parse("-- x").is_none());

        let lim = lim_parse("- blah").unwrap();

        assert_eq!(
            lim.item,
            ListItemMarker::Hyphen(Span {
                data: "-",
                line: 1,
                col: 1,
                offset: 0,
            },)
        );

        assert_eq!(
            lim.after,
            Span {
                data: "blah",
                line: 1,
                col: 3,
                offset: 2,
            }
        );

        assert_eq!(
            lim.item.span(),
            Span {
                data: "-",
                line: 1,
                col: 1,
                offset: 0,
            }
        );

        assert_eq!(
            format!("{lim:#?}", lim = lim.item),
            "ListItemMarker::Hyphen(\n    Span {\n        data: \"-\",\n        line: 1,\n        col: 1,\n        offset: 0,\n    },\n)"
        );
    }

    #[test]
    fn asterisks() {
        assert!(lim_parse("*").is_none());
        assert!(lim_parse("*- x").is_none());

        let lim = lim_parse("* blah").unwrap();

        assert_eq!(
            lim.item,
            ListItemMarker::Asterisks(Span {
                data: "*",
                line: 1,
                col: 1,
                offset: 0,
            },)
        );

        assert_eq!(
            lim.after,
            Span {
                data: "blah",
                line: 1,
                col: 3,
                offset: 2,
            }
        );

        assert_eq!(
            lim.item.span(),
            Span {
                data: "*",
                line: 1,
                col: 1,
                offset: 0,
            }
        );

        assert_eq!(
            format!("{lim:#?}", lim = lim.item),
            "ListItemMarker::Asterisks(\n    Span {\n        data: \"*\",\n        line: 1,\n        col: 1,\n        offset: 0,\n    },\n)"
        );

        let lim = lim_parse("***** blah").unwrap();

        assert_eq!(
            lim.item,
            ListItemMarker::Asterisks(Span {
                data: "*****",
                line: 1,
                col: 1,
                offset: 0,
            },)
        );

        assert_eq!(
            lim.after,
            Span {
                data: "blah",
                line: 1,
                col: 7,
                offset: 6,
            }
        );

        assert_eq!(
            lim.item.span(),
            Span {
                data: "*****",
                line: 1,
                col: 1,
                offset: 0,
            }
        );

        assert_eq!(
            format!("{lim:#?}", lim = lim.item),
            "ListItemMarker::Asterisks(\n    Span {\n        data: \"*****\",\n        line: 1,\n        col: 1,\n        offset: 0,\n    },\n)"
        );
    }

    #[test]
    fn dots() {
        assert!(lim_parse(".").is_none());
        assert!(lim_parse(".- x").is_none());

        let lim = lim_parse(". blah").unwrap();

        assert_eq!(
            lim.item,
            ListItemMarker::Dots(Span {
                data: ".",
                line: 1,
                col: 1,
                offset: 0,
            },)
        );

        assert_eq!(
            lim.after,
            Span {
                data: "blah",
                line: 1,
                col: 3,
                offset: 2,
            }
        );

        assert_eq!(
            lim.item.span(),
            Span {
                data: ".",
                line: 1,
                col: 1,
                offset: 0,
            }
        );

        assert_eq!(
            format!("{lim:#?}", lim = lim.item),
            "ListItemMarker::Dots(\n    Span {\n        data: \".\",\n        line: 1,\n        col: 1,\n        offset: 0,\n    },\n)"
        );

        let lim = lim_parse("..... blah").unwrap();

        assert_eq!(
            lim.item,
            ListItemMarker::Dots(Span {
                data: ".....",
                line: 1,
                col: 1,
                offset: 0,
            },)
        );

        assert_eq!(
            lim.after,
            Span {
                data: "blah",
                line: 1,
                col: 7,
                offset: 6,
            }
        );

        assert_eq!(
            lim.item.span(),
            Span {
                data: ".....",
                line: 1,
                col: 1,
                offset: 0,
            }
        );

        assert_eq!(
            format!("{lim:#?}", lim = lim.item),
            "ListItemMarker::Dots(\n    Span {\n        data: \".....\",\n        line: 1,\n        col: 1,\n        offset: 0,\n    },\n)"
        );
    }

    #[test]
    fn roman_numeral_lower() {
        assert!(lim_parse("i").is_none());
        assert!(lim_parse("i.").is_none());

        let lim = lim_parse("i) blah").unwrap();

        assert_eq!(
            lim.item,
            ListItemMarker::RomanNumeralLower(Span {
                data: "i)",
                line: 1,
                col: 1,
                offset: 0,
            },)
        );

        assert_eq!(
            lim.after,
            Span {
                data: "blah",
                line: 1,
                col: 4,
                offset: 3,
            }
        );

        assert_eq!(
            lim.item.span(),
            Span {
                data: "i)",
                line: 1,
                col: 1,
                offset: 0,
            }
        );

        assert_eq!(
            format!("{lim:#?}", lim = lim.item),
            "ListItemMarker::RomanNumeralLower(\n    Span {\n        data: \"i)\",\n        line: 1,\n        col: 1,\n        offset: 0,\n    },\n)"
        );

        let lim = lim_parse("xvii) blah").unwrap();

        assert_eq!(
            lim.item,
            ListItemMarker::RomanNumeralLower(Span {
                data: "xvii)",
                line: 1,
                col: 1,
                offset: 0,
            },)
        );

        assert_eq!(
            lim.after,
            Span {
                data: "blah",
                line: 1,
                col: 7,
                offset: 6,
            }
        );
    }

    #[test]
    fn roman_numeral_upper() {
        assert!(lim_parse("I").is_none());
        assert!(lim_parse("I.").is_none());

        let lim = lim_parse("I) blah").unwrap();

        assert_eq!(
            lim.item,
            ListItemMarker::RomanNumeralUpper(Span {
                data: "I)",
                line: 1,
                col: 1,
                offset: 0,
            },)
        );

        assert_eq!(
            lim.after,
            Span {
                data: "blah",
                line: 1,
                col: 4,
                offset: 3,
            }
        );

        assert_eq!(
            lim.item.span(),
            Span {
                data: "I)",
                line: 1,
                col: 1,
                offset: 0,
            }
        );

        assert_eq!(
            format!("{lim:#?}", lim = lim.item),
            "ListItemMarker::RomanNumeralUpper(\n    Span {\n        data: \"I)\",\n        line: 1,\n        col: 1,\n        offset: 0,\n    },\n)"
        );

        let lim = lim_parse("XVII) blah").unwrap();

        assert_eq!(
            lim.item,
            ListItemMarker::RomanNumeralUpper(Span {
                data: "XVII)",
                line: 1,
                col: 1,
                offset: 0,
            },)
        );

        assert_eq!(
            lim.after,
            Span {
                data: "blah",
                line: 1,
                col: 7,
                offset: 6,
            }
        );
    }

    #[test]
    fn alpha_list_lower() {
        assert!(lim_parse("a").is_none());
        assert!(lim_parse("a)").is_none());

        let lim = lim_parse("a. blah").unwrap();

        assert_eq!(
            lim.item,
            ListItemMarker::AlphaListLower(Span {
                data: "a.",
                line: 1,
                col: 1,
                offset: 0,
            },)
        );

        assert_eq!(
            lim.after,
            Span {
                data: "blah",
                line: 1,
                col: 4,
                offset: 3,
            }
        );

        assert_eq!(
            lim.item.span(),
            Span {
                data: "a.",
                line: 1,
                col: 1,
                offset: 0,
            }
        );

        assert_eq!(
            format!("{lim:#?}", lim = lim.item),
            "ListItemMarker::AlphaListLower(\n    Span {\n        data: \"a.\",\n        line: 1,\n        col: 1,\n        offset: 0,\n    },\n)"
        );

        let lim = lim_parse("x. blah").unwrap();

        assert_eq!(
            lim.item,
            ListItemMarker::AlphaListLower(Span {
                data: "x.",
                line: 1,
                col: 1,
                offset: 0,
            },)
        );

        assert_eq!(
            lim.after,
            Span {
                data: "blah",
                line: 1,
                col: 4,
                offset: 3,
            }
        );
    }

    #[test]
    fn callout() {
        // Not callout markers: no leading bracket, no trailing whitespace, or
        // a non-numeric/non-dot body.
        assert!(lim_parse("1> blah").is_none());
        assert!(lim_parse("<1>blah").is_none());
        assert!(lim_parse("<1>").is_none());
        assert!(lim_parse("<a> blah").is_none());

        let lim = lim_parse("<1> blah").unwrap();

        assert_eq!(
            lim.item,
            ListItemMarker::Callout(Span {
                data: "<1>",
                line: 1,
                col: 1,
                offset: 0,
            },)
        );

        assert_eq!(
            lim.after,
            Span {
                data: "blah",
                line: 1,
                col: 5,
                offset: 4,
            }
        );

        assert_eq!(
            lim.item.span(),
            Span {
                data: "<1>",
                line: 1,
                col: 1,
                offset: 0,
            }
        );

        // Ordinal helpers do not apply to callout markers.
        assert!(lim.item.ordinal_value().is_none());
        assert!(lim.item.ordinal_to_marker_text(1).is_none());

        // Callout markers of any number match each other (so a single list is
        // formed), but not markers of other kinds.
        let lim2 = lim_parse("<.> blah").unwrap();
        assert_eq!(
            lim2.item,
            ListItemMarker::Callout(Span {
                data: "<.>",
                line: 1,
                col: 1,
                offset: 0,
            },)
        );
        assert!(lim.item.is_match_for(&lim2.item));
        assert!(!lim.item.is_match_for(&lim_parse("- blah").unwrap().item));

        // An explicit `<N>` marker reports its number; an automatic `<.>`
        // marker and any non-callout marker report `None`.
        assert_eq!(lim.item.callout_number(), Some(1));
        assert!(lim2.item.callout_number().is_none());
        assert!(lim_parse("- blah").unwrap().item.callout_number().is_none());

        assert_eq!(
            format!("{:#?}", lim.item),
            "ListItemMarker::Callout(\n    Span {\n        data: \"<1>\",\n        line: 1,\n        col: 1,\n        offset: 0,\n    },\n)"
        );

        assert!(crate::blocks::ListItemMarker::starts_with_marker(
            crate::Span::new("<1> blah")
        ));
        assert!(!crate::blocks::ListItemMarker::starts_with_marker(
            crate::Span::new("1> blah")
        ));

        // Leading whitespace is discarded consistently for every marker kind,
        // matching `parse`.
        assert!(crate::blocks::ListItemMarker::starts_with_marker(
            crate::Span::new("  <1> blah")
        ));
        assert!(crate::blocks::ListItemMarker::starts_with_marker(
            crate::Span::new("  - blah")
        ));
    }
}
