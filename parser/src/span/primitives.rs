use super::{MatchedItem, Span};

impl<'src> Span<'src> {
    /// Split the span, consuming an identifier if found.
    ///
    /// IMPORTANT: This function is not quite deprecated yet, but its use is
    /// strongly discouraged. The concept of "identifier" is not crisply defined
    /// in the Asciidoc documentation, so – for now – we're borrowing the
    /// definition from Rust, which is a single alphabetic character or
    /// underscore, followed by any number of alphanumeric characters or
    /// underscores.
    ///
    /// Please use more specific functions, such as `take_attr_name` or
    /// `take_user_attr_name`, when possible.
    pub(crate) fn take_ident(self) -> Option<MatchedItem<'src, Self>> {
        let mut chars = self.data.char_indices();

        let (_, c) = chars.next()?;
        if !c.is_ascii_alphabetic() && c != '_' {
            return None;
        }

        for (index, c) in chars {
            if !c.is_ascii_alphanumeric() && c != '_' {
                return Some(self.into_parse_result(index));
            }
        }

        Some(self.into_parse_result(self.len()))
    }

    /// Split the span, consuming an attribute name if found.
    ///
    /// An [attribute name] consists of a word character (letter or numeral)
    /// followed by any number of word or `-` characters (e.g., `see-also`).
    ///
    /// [attribute name]: https://docs.asciidoctor.org/asciidoc/latest/attributes/positional-and-named-attributes/#attribute-list-parsing
    pub(crate) fn take_attr_name(self) -> Option<MatchedItem<'src, Self>> {
        let mut chars = self.data.char_indices();

        let (_, c) = chars.next()?;
        if !c.is_ascii_alphanumeric() {
            return None;
        }

        for (index, c) in chars {
            if !c.is_ascii_alphanumeric() && c != '-' {
                return Some(self.into_parse_result(index));
            }
        }

        Some(self.into_parse_result(self.len()))
    }

    /// Split the span, consuming a user-defined attribute name if found.
    ///
    /// [User-defined attribute names] must:
    ///
    /// * be at least one character long,
    /// * begin with a word character (`a`-`z`, `0`-`9`, or `_`), and
    /// * only contain word characters and hyphens (`-`).
    ///
    /// A user-defined attribute name cannot contain dots (`.`) or spaces.
    /// Although uppercase characters are permitted in an attribute name, the
    /// name is converted to lowercase before being stored. For example,
    /// `URL-REPO` and `URL-Repo` are treated as `url-repo` when a document is
    /// loaded or converted. A best practice is to only use lowercase letters in
    /// the name and avoid starting the name with a number.
    ///
    /// IMPORTANT: This function does _not_ perform the lower-case normalization
    /// defined above.
    ///
    /// [User-defined attribute names]: https://docs.asciidoctor.org/asciidoc/latest/attributes/names-and-values/#user-defined
    pub(crate) fn take_user_attr_name(self) -> Option<MatchedItem<'src, Self>> {
        let mut chars = self.data.char_indices();

        let (_, c) = chars.next()?;
        if !c.is_ascii_alphanumeric() && c != '_' {
            return None;
        }

        for (index, c) in chars {
            if !c.is_ascii_alphanumeric() && c != '_' && c != '-' {
                return Some(self.into_parse_result(index));
            }
        }

        Some(self.into_parse_result(self.len()))
    }

    /// Returns [`true`] if the span properly forms an [XML Name].
    ///
    /// [XML Name]: https://www.w3.org/TR/REC-xml/#NT-Name
    pub(crate) fn is_xml_name(self) -> bool {
        let mut chars = self.data.chars();

        if let Some(c) = chars.next() {
            match c {
                ':' | '_' => (),
                '\u{037e}' => {
                    return false;
                }
                _ => {
                    if !c.is_alphabetic() {
                        return false;
                    }
                }
            }
        } else {
            return false;
        }

        chars.all(|c| match c {
            ':' | '_' | '.' | '-' | '\u{00b7}' => true,
            '\u{037e}' => false,
            _ => c.is_alphanumeric(),
        })
    }

    /// Split the span, consuming one quoted string if found.
    ///
    /// A string is defined as a single quote or double quote character,
    /// followed by any number of other characters, and terminated by a matching
    /// single or double quote character.
    ///
    /// The single or double quote may be included in the string by preceding it
    /// with a backslash. No other backslash escape sequences are recognized.
    ///
    /// **IMPORTANT:** The [`Span`] that is returned does not include the start
    /// or ending quote, but _does_ include (without transformation) any
    /// escaped quotes.
    pub(crate) fn take_quoted_string(self) -> Option<MatchedItem<'src, Self>> {
        let mut chars = self.data.char_indices();

        let delimiter = match chars.next() {
            Some((_, '\'')) => '\'',
            Some((_, '"')) => '"',
            _ => {
                return None;
            }
        };

        let mut prev_was_backslash = false;

        for (index, c) in chars {
            if c == delimiter && !prev_was_backslash {
                return Some(MatchedItem {
                    item: self.slice(1..index),
                    after: self.slice_from(index + 1..),
                });
            }
            prev_was_backslash = c == '\\';
        }

        // Didn't find closing delimiter.
        None
    }

    /// Given a second [`Span`], which must be a trailing remainder of `self`,
    /// return the portion of `self` that excludes the second (remainder).
    ///
    /// Note that the trailing remainder condition is not enforced.
    pub(crate) fn trim_remainder(self, after: Span<'src>) -> Span<'src> {
        let offset = (self.offset + self.len()).min(after.offset);

        if offset <= self.offset {
            // Invalid input: We'll respond with an empty slice.
            self.slice(0..0)
        } else {
            let trim_len = offset - self.offset;
            self.slice(0..trim_len)
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    mod take_ident {
        use crate::tests::prelude::*;

        #[test]
        fn empty_source() {
            let span = crate::Span::default();
            assert!(span.take_ident().is_none());
        }

        #[test]
        fn starts_with_non_word() {
            let span = crate::Span::new("#not-a-proper-name");
            assert!(span.take_ident().is_none());
        }

        #[test]
        fn starts_with_hyphen() {
            let span = crate::Span::new("-not-a-proper-name");
            assert!(span.take_ident().is_none());
        }

        #[test]
        fn starts_with_number() {
            let span = crate::Span::new("9not-a-proper-name");
            assert!(span.take_ident().is_none());
        }

        #[test]
        fn stops_at_non_ident() {
            let span = crate::Span::new("x#");
            let mi = span.take_ident().unwrap();

            assert_eq!(
                mi.item,
                Span {
                    data: "x",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "#",
                    line: 1,
                    col: 2,
                    offset: 1
                }
            );
        }

        #[test]
        fn alpha_numeric() {
            let span = crate::Span::new("i94!");
            let mi = span.take_ident().unwrap();

            assert_eq!(
                mi.item,
                Span {
                    data: "i94",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "!",
                    line: 1,
                    col: 4,
                    offset: 3
                }
            );
        }

        #[test]
        fn starts_with_underscore() {
            let span = crate::Span::new("_i94!");
            let mi = span.take_ident().unwrap();

            assert_eq!(
                mi.item,
                Span {
                    data: "_i94",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "!",
                    line: 1,
                    col: 5,
                    offset: 4
                }
            );
        }

        #[test]
        fn contains_underscores() {
            let span = crate::Span::new("blah_blah_94 = foo");
            let mi = span.take_ident().unwrap();

            assert_eq!(
                mi.item,
                Span {
                    data: "blah_blah_94",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: " = foo",
                    line: 1,
                    col: 13,
                    offset: 12
                }
            );
        }

        #[test]
        fn contains_hyphens() {
            let span = crate::Span::new("blah-blah-94 = foo");
            let mi = span.take_ident().unwrap();

            assert_eq!(
                mi.item,
                Span {
                    data: "blah",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "-blah-94 = foo",
                    line: 1,
                    col: 5,
                    offset: 4
                }
            );
        }

        #[test]
        fn stops_at_eof() {
            let span = crate::Span::new("xyz");
            let mi = span.take_ident().unwrap();

            assert_eq!(
                mi.item,
                Span {
                    data: "xyz",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 4,
                    offset: 3
                }
            );
        }
    }

    mod take_attr_name {
        use crate::tests::prelude::*;

        #[test]
        fn empty_source() {
            let span = crate::Span::default();
            assert!(span.take_attr_name().is_none());
        }

        #[test]
        fn starts_with_non_word() {
            let span = crate::Span::new("#not-a-proper-name");
            assert!(span.take_attr_name().is_none());
        }

        #[test]
        fn starts_with_hyphen() {
            let span = crate::Span::new("-not-a-proper-name");
            assert!(span.take_attr_name().is_none());
        }

        #[test]
        fn stops_at_non_ident() {
            let span = crate::Span::new("x#");
            let mi = span.take_attr_name().unwrap();

            assert_eq!(
                mi.item,
                Span {
                    data: "x",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "#",
                    line: 1,
                    col: 2,
                    offset: 1
                }
            );
        }

        #[test]
        fn numeric() {
            let span = crate::Span::new("94!");
            let mi = span.take_attr_name().unwrap();

            assert_eq!(
                mi.item,
                Span {
                    data: "94",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "!",
                    line: 1,
                    col: 3,
                    offset: 2
                }
            );
        }

        #[test]
        fn contains_hyphens() {
            let span = crate::Span::new("blah-blah-94 = foo");
            let mi = span.take_attr_name().unwrap();

            assert_eq!(
                mi.item,
                Span {
                    data: "blah-blah-94",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: " = foo",
                    line: 1,
                    col: 13,
                    offset: 12
                }
            );
        }

        #[test]
        fn stops_at_eof() {
            let span = crate::Span::new("xyz");
            let mi = span.take_attr_name().unwrap();

            assert_eq!(
                mi.item,
                Span {
                    data: "xyz",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 4,
                    offset: 3
                }
            );
        }
    }

    mod take_user_attr_name {
        use crate::tests::prelude::*;

        #[test]
        fn empty_source() {
            let span = crate::Span::default();
            assert!(span.take_user_attr_name().is_none());
        }

        #[test]
        fn starts_with_non_word() {
            let span = crate::Span::new("#not-a-proper-name");
            assert!(span.take_user_attr_name().is_none());
        }

        #[test]
        fn starts_with_hyphen() {
            let span = crate::Span::new("-not-a-proper-name");
            assert!(span.take_user_attr_name().is_none());
        }

        #[test]
        fn stops_at_non_ident() {
            let span = crate::Span::new("x#");
            let mi = span.take_user_attr_name().unwrap();

            assert_eq!(
                mi.item,
                Span {
                    data: "x",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "#",
                    line: 1,
                    col: 2,
                    offset: 1
                }
            );
        }

        #[test]
        fn numeric() {
            let span = crate::Span::new("94!");
            let mi = span.take_user_attr_name().unwrap();

            assert_eq!(
                mi.item,
                Span {
                    data: "94",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "!",
                    line: 1,
                    col: 3,
                    offset: 2
                }
            );
        }

        #[test]
        fn contains_hyphens() {
            let span = crate::Span::new("blah-blah-94 = foo");
            let mi = span.take_user_attr_name().unwrap();

            assert_eq!(
                mi.item,
                Span {
                    data: "blah-blah-94",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: " = foo",
                    line: 1,
                    col: 13,
                    offset: 12
                }
            );
        }

        #[test]
        fn stops_at_eof() {
            let span = crate::Span::new("xyz");
            let mi = span.take_user_attr_name().unwrap();

            assert_eq!(
                mi.item,
                Span {
                    data: "xyz",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 4,
                    offset: 3
                }
            );
        }
    }

    #[test]
    fn is_xml_name() {
        assert!(!crate::Span::default().is_xml_name());
        assert!(crate::Span::new("a").is_xml_name());
        assert!(crate::Span::new("a9").is_xml_name());

        assert!(crate::Span::new("install").is_xml_name());
        assert!(crate::Span::new("data-structures").is_xml_name());
        assert!(crate::Span::new("error-handling").is_xml_name());
        assert!(crate::Span::new("subject-and-body").is_xml_name());
        assert!(crate::Span::new("unset_an_attribute").is_xml_name());
        assert!(crate::Span::new(":a").is_xml_name());
        assert!(crate::Span::new("_a").is_xml_name());

        assert!(!crate::Span::new("install the gem").is_xml_name());
        assert!(!crate::Span::new("3 blind mice").is_xml_name());
        assert!(!crate::Span::new("-about-the-author").is_xml_name());
        assert!(!crate::Span::new("\u{037e}abc").is_xml_name());
        assert!(!crate::Span::new("ab\u{037e}c").is_xml_name());
    }

    mod take_quoted_string {
        use crate::tests::prelude::*;

        #[test]
        fn empty_source() {
            let span = crate::Span::default();
            assert!(span.take_quoted_string().is_none());
        }

        #[test]
        fn unterminated_double_quote() {
            let span = crate::Span::new("\"xxx");
            assert!(span.take_quoted_string().is_none());
        }

        #[test]
        fn double_quoted_string() {
            let span = crate::Span::new("\"abc\"def");
            let mi = span.take_quoted_string().unwrap();

            assert_eq!(
                mi.item,
                Span {
                    data: "abc",
                    line: 1,
                    col: 2,
                    offset: 1
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "def",
                    line: 1,
                    col: 6,
                    offset: 5
                }
            );
        }

        #[test]
        fn double_quoted_with_escape() {
            let span = crate::Span::new("\"a\\\"bc\"def");
            let mi = span.take_quoted_string().unwrap();

            assert_eq!(
                mi.item,
                Span {
                    data: "a\\\"bc",
                    line: 1,
                    col: 2,
                    offset: 1
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "def",
                    line: 1,
                    col: 8,
                    offset: 7
                }
            );
        }

        #[test]
        fn double_quoted_with_single_quote() {
            let span = crate::Span::new("\"a'bc\"def");
            let mi = span.take_quoted_string().unwrap();

            assert_eq!(
                mi.item,
                Span {
                    data: "a'bc",
                    line: 1,
                    col: 2,
                    offset: 1
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "def",
                    line: 1,
                    col: 7,
                    offset: 6
                }
            );
        }

        #[test]
        fn unterminated_single_quote() {
            let span = crate::Span::new("'xxx");
            assert!(span.take_quoted_string().is_none());
        }

        #[test]
        fn single_quoted_string() {
            let span = crate::Span::new("'abc'def");
            let mi = span.take_quoted_string().unwrap();

            assert_eq!(
                mi.item,
                Span {
                    data: "abc",
                    line: 1,
                    col: 2,
                    offset: 1
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "def",
                    line: 1,
                    col: 6,
                    offset: 5
                }
            );
        }

        #[test]
        fn single_quoted_with_escape() {
            let span = crate::Span::new("'a\\'bc'def");
            let mi = span.take_quoted_string().unwrap();

            assert_eq!(
                mi.item,
                Span {
                    data: "a\\'bc",
                    line: 1,
                    col: 2,
                    offset: 1
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "def",
                    line: 1,
                    col: 8,
                    offset: 7
                }
            );
        }

        #[test]
        fn single_quoted_with_double_quote() {
            let span = crate::Span::new("'a\"bc'def");
            let mi = span.take_quoted_string().unwrap();

            assert_eq!(
                mi.item,
                Span {
                    data: "a\"bc",
                    line: 1,
                    col: 2,
                    offset: 1
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "def",
                    line: 1,
                    col: 7,
                    offset: 6
                }
            );
        }
    }

    mod trim_remainder {
        use crate::tests::prelude::*;

        fn advanced_span(source: &'static str, skip: usize) -> crate::Span<'static> {
            let span = crate::Span::new(source);
            span.slice_from(skip..)
        }

        #[test]
        fn empty_spans() {
            let source = advanced_span("abcdef", 6);
            let after = crate::Span::default();

            assert_eq!(
                source.trim_remainder(after),
                Span {
                    data: "",
                    line: 1,
                    col: 7,
                    offset: 6
                }
            );
        }

        #[test]
        fn rem_equals_source() {
            let source = advanced_span("abcdef", 6);
            let after = crate::Span::new("abcdef");

            assert_eq!(
                source.trim_remainder(after),
                Span {
                    data: "",
                    line: 1,
                    col: 7,
                    offset: 6
                }
            );
        }

        #[test]
        fn rem_too_long() {
            // This is nonsense input, but we should at least not panic in this case.

            let source = advanced_span("abcdef", 6);
            let after = crate::Span::new("abcdef_bogus_bogus");

            assert_eq!(
                source.trim_remainder(after),
                Span {
                    data: "",
                    line: 1,
                    col: 7,
                    offset: 6
                }
            );
        }

        #[test]
        fn rem_is_subset_of_source() {
            let source = advanced_span("abcdef", 2);
            let after = advanced_span("abcdef", 4);

            assert_eq!(
                source.trim_remainder(after),
                Span {
                    data: "cd",
                    line: 1,
                    col: 3,
                    offset: 2
                }
            );
        }

        #[test]
        fn rem_is_incomplete_subset_of_source() {
            let source = crate::Span::new("abc\ndef\n");
            let line1 = source.take_normalized_line();
            let line2 = line1.after.take_line();

            assert_eq!(
                source.trim_remainder(line2.item),
                Span {
                    data: "abc\n",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }
    }
}
