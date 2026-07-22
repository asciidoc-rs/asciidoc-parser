use std::sync::LazyLock;

use regex::Regex;

/// A single word character, as defined by Unicode (UTS #18 Annex C): a letter,
/// a mark, a decimal digit, a connector punctuation (e.g. `_`), or a join
/// control (e.g. ZWNJ). The regex crate's `\w` implements exactly this set,
/// which is also Asciidoctor's `\p{Word}` (`CG_WORD` / `CC_WORD`).
static WORD_CHAR: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r"^\w$").unwrap()
});

/// Returns `true` when `c` is a *word character* for the purpose of attribute
/// naming.
///
/// This mirrors Asciidoctor's `\p{Word}` character class (`CG_WORD` /
/// `CC_WORD`), which is used to recognize attribute-entry names, sanitize
/// them, and match attribute references. Unlike the ASCII-only `\w` used
/// elsewhere in this crate, it accepts the full Unicode range: any letter or
/// decimal digit, plus marks, connector punctuation (e.g. `_`), and join
/// controls (e.g. ZWNJ), so `café`, `سمن`, and `_foo` are all valid names.
///
/// The regexes that match attribute *references* use the same `\w` class, so
/// an entry name and a reference to it always agree — including on decomposed
/// forms (`e` + combining acute) and Persian names that embed a ZWNJ.
pub(crate) fn is_word_char(c: char) -> bool {
    let mut buf = [0u8; 4];
    WORD_CHAR.is_match(c.encode_utf8(&mut buf))
}

#[cfg(test)]
mod tests {
    use super::is_word_char;

    #[test]
    fn ascii_word_chars() {
        assert!(is_word_char('a'));
        assert!(is_word_char('Z'));
        assert!(is_word_char('0'));
        assert!(is_word_char('_'));
    }

    #[test]
    fn unicode_letters_and_digits() {
        // Latin letter with accent, Arabic letter, CJK ideograph, and a
        // non-ASCII decimal digit are all word characters.
        assert!(is_word_char('é'));
        assert!(is_word_char('س'));
        assert!(is_word_char('中'));
        assert!(is_word_char('٣')); // Arabic-Indic digit three.
    }

    #[test]
    fn marks_and_join_controls() {
        // `\p{Word}` includes combining marks and join controls, so a
        // decomposed name (`e` + combining acute) and a Persian name embedding
        // a ZWNJ round-trip through sanitization unchanged.
        assert!(is_word_char('\u{0301}')); // Combining acute accent (a mark).
        assert!(is_word_char('\u{200c}')); // Zero-width non-joiner (ZWNJ).
        assert!(is_word_char('\u{200d}')); // Zero-width joiner (ZWJ).
    }

    #[test]
    fn non_word_chars() {
        assert!(!is_word_char(' '));
        assert!(!is_word_char('-'));
        assert!(!is_word_char('#'));
        assert!(!is_word_char(':'));
        assert!(!is_word_char('^'));
    }
}
