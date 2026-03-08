use super::{MatchedItem, Span};

impl<'src> Span<'src> {
    /// Split this span, consuming the exact character sequence `prefix` if
    /// found.
    ///
    /// Returns `None` if `prefix` is not found.
    pub(crate) fn take_prefix(self, prefix: &str) -> Option<MatchedItem<'src, Self>> {
        if self.data.starts_with(prefix) {
            Some(self.into_parse_result(prefix.len()))
        } else {
            None
        }
    }

    /// Split this span, consuming any white space.
    pub(crate) fn take_whitespace(self) -> MatchedItem<'src, Self> {
        self.take_while(|c| c == ' ' || c == '\t')
    }

    /// Split this span, consuming any white space including new-line.
    pub(crate) fn take_whitespace_with_newline(self) -> MatchedItem<'src, Self> {
        self.take_while(|c| c == ' ' || c == '\t' || c == '\n')
    }

    /// Split this span, consuming at least one white space character.
    ///
    /// Returns `None` if the first character is not a space or tab.
    pub(crate) fn take_required_whitespace(self) -> Option<MatchedItem<'src, Self>> {
        let mi = self.take_while(|c| c == ' ' || c == '\t');
        if mi.item.is_empty() { None } else { Some(mi) }
    }

    /// Split this span at the first character that doesn't match `predicate`.
    pub(crate) fn take_while<P>(self, predicate: P) -> MatchedItem<'src, Self>
    where
        P: Fn(char) -> bool,
    {
        match self.position(|c| !predicate(c)) {
            Some(n) => self.into_parse_result(n),
            None => self.into_parse_result(self.data.len()),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    mod take_prefix {
        use crate::tests::prelude::*;

        #[test]
        fn empty_source() {
            let span = crate::Span::default();
            assert!(span.take_prefix("foo").is_none());
        }

        #[test]
        fn mismatch() {
            let span = crate::Span::new(":abc");
            assert!(span.take_prefix("abc").is_none());
        }

        #[test]
        fn partial_match() {
            let span = crate::Span::new("abc");
            assert!(span.take_prefix("abcd").is_none());
        }

        #[test]
        fn match_with_remainder() {
            let s = crate::Span::new("ab:cd");
            let mi = s.take_prefix("ab").unwrap();

            assert_eq!(
                mi.item,
                Span {
                    data: "ab",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: ":cd",
                    line: 1,
                    col: 3,
                    offset: 2,
                }
            );
        }

        #[test]
        fn exact_match() {
            let s = crate::Span::new("ab:cd");
            let mi = s.take_prefix("ab:cd").unwrap();

            assert_eq!(
                mi.item,
                Span {
                    data: "ab:cd",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 6,
                    offset: 5,
                }
            );
        }
    }

    mod take_whitespace {
        use crate::tests::prelude::*;

        #[test]
        fn empty_source() {
            let span = crate::Span::default();
            let mi = span.take_whitespace();

            assert_eq!(
                mi.item,
                Span {
                    data: "",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );
        }

        #[test]
        fn immediate_false() {
            let s = crate::Span::new(":abc");
            let mi = s.take_whitespace();

            assert_eq!(
                mi.item,
                Span {
                    data: "",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: ":abc",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );
        }

        #[test]
        fn match_after_first() {
            let s = crate::Span::new(" \t:cd");
            let mi = s.take_whitespace();

            assert_eq!(
                mi.item,
                Span {
                    data: " \t",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: ":cd",
                    line: 1,
                    col: 3,
                    offset: 2,
                }
            );
        }

        #[test]
        fn doesnt_include_newline() {
            let s = crate::Span::new(" \t\n:cd");
            let mi = s.take_whitespace();

            assert_eq!(
                mi.item,
                Span {
                    data: " \t",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "\n:cd",
                    line: 1,
                    col: 3,
                    offset: 2,
                }
            );
        }

        #[test]
        fn all_whitespace() {
            let s = crate::Span::new("  \t ");
            let mi = s.take_whitespace();

            assert_eq!(
                mi.item,
                Span {
                    data: "  \t ",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 5,
                    offset: 4,
                }
            );
        }
    }

    mod take_whitespace_with_newline {
        use crate::tests::prelude::*;

        #[test]
        fn empty_source() {
            let span = crate::Span::default();
            let mi = span.take_whitespace_with_newline();

            assert_eq!(
                mi.item,
                Span {
                    data: "",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );
        }

        #[test]
        fn immediate_false() {
            let s = crate::Span::new(":abc");
            let mi = s.take_whitespace_with_newline();

            assert_eq!(
                mi.item,
                Span {
                    data: "",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: ":abc",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );
        }

        #[test]
        fn match_after_first() {
            let s = crate::Span::new(" \t:cd");
            let mi = s.take_whitespace_with_newline();

            assert_eq!(
                mi.item,
                Span {
                    data: " \t",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: ":cd",
                    line: 1,
                    col: 3,
                    offset: 2,
                }
            );
        }

        #[test]
        fn includes_newline() {
            let s = crate::Span::new(" \t\n:cd");
            let mi = s.take_whitespace_with_newline();

            assert_eq!(
                mi.item,
                Span {
                    data: " \t\n",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: ":cd",
                    line: 2,
                    col: 1,
                    offset: 3,
                }
            );
        }

        #[test]
        fn all_whitespace() {
            let s = crate::Span::new("  \t\n ");
            let mi = s.take_whitespace_with_newline();

            assert_eq!(
                mi.item,
                Span {
                    data: "  \t\n ",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 2,
                    col: 2,
                    offset: 5,
                }
            );
        }
    }

    mod take_required_whitespace {
        use crate::tests::prelude::*;

        #[test]
        fn empty_source() {
            let span = crate::Span::default();
            assert!(span.take_required_whitespace().is_none());
        }

        #[test]
        fn starts_with_non_whitespace() {
            let s = crate::Span::new(":abc");
            assert!(s.take_required_whitespace().is_none());
        }

        #[test]
        fn match_after_first() {
            let s = crate::Span::new(" \t:cd");
            let mi = s.take_required_whitespace().unwrap();

            assert_eq!(
                mi.item,
                Span {
                    data: " \t",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: ":cd",
                    line: 1,
                    col: 3,
                    offset: 2,
                }
            );
        }

        #[test]
        fn all_whitespace() {
            let s = crate::Span::new("  \t ");
            let mi = s.take_required_whitespace().unwrap();

            assert_eq!(
                mi.item,
                Span {
                    data: "  \t ",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 5,
                    offset: 4,
                }
            );
        }
    }

    mod take_while {
        use crate::tests::prelude::*;

        #[test]
        fn empty_source() {
            let span = crate::Span::default();
            let mi = span.take_while(|c| c != ':');

            assert_eq!(
                mi.item,
                Span {
                    data: "",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );
        }

        #[test]
        fn immediate_false() {
            let s = crate::Span::new(":abc");
            let mi = s.take_while(|c| c != ':');

            assert_eq!(
                mi.item,
                Span {
                    data: "",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: ":abc",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );
        }

        #[test]
        fn match_after_first() {
            let s = crate::Span::new("ab:cd");
            let mi = s.take_while(|c| c != ':');

            assert_eq!(
                mi.item,
                Span {
                    data: "ab",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: ":cd",
                    line: 1,
                    col: 3,
                    offset: 2,
                }
            );
        }

        #[test]
        fn non_empty_no_match() {
            let s = crate::Span::new("abcd");
            let mi = s.take_while(|c| c != ':');

            assert_eq!(
                mi.item,
                Span {
                    data: "abcd",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );

            assert_eq!(
                mi.after,
                Span {
                    data: "",
                    line: 1,
                    col: 5,
                    offset: 4,
                }
            );
        }
    }
}
