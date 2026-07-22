/// Returns `true` when `c` is a *word character* for the purpose of attribute
/// naming.
///
/// This mirrors Asciidoctor's `\p{Word}` character class (`CG_WORD` /
/// `CC_WORD`), which is used to recognize attribute-entry names, sanitize them,
/// and match attribute references. Unlike the ASCII-only `\w` used elsewhere in
/// this crate, it accepts the full Unicode range of letters and digits (plus
/// `_`), so `café`, `سمن`, and `_foo` are all valid names.
///
/// Rust's standard library cannot classify Unicode marks or `Join_Control`
/// characters, so this is a close approximation of `\p{Word}`: it matches any
/// alphabetic or numeric character (`char::is_alphanumeric`) or an underscore.
/// The regex used to match attribute *references* is written to accept exactly
/// the same set, so an entry name and a reference to it always agree.
pub(crate) fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}
