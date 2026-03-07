use super::Span;

impl Span<'_> {
    /// Return a [`Span`] that is the same as the source, but with any trailing
    /// space or tab characters removed.
    pub(crate) fn trim_trailing_whitespace(&self) -> Self {
        // IMPORTANT: We don't use `str::trim_end` because Rust's definition of
        // whitespace doesn't match Asciidoc's definition.
        let new_len = self
            .data
            .trim_end_matches([' ', '\t', '\n', '\r', '\x0C', '\x0B'])
            .len();
        self.slice(0..new_len)
    }

    /// Return a [`Span`] that is the same as the source, but a single trailing
    /// line-ending (if found) removed.
    pub(crate) fn trim_trailing_line_end(&self) -> Self {
        if self.ends_with("\r\n") {
            self.slice(0..self.len() - 2)
        } else if self.ends_with('\n') {
            self.slice(0..self.len() - 1)
        } else {
            *self
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    mod trim_trailing_whitespace {
        use pretty_assertions_sorted::assert_eq;

        use crate::tests::prelude::*;

        #[test]
        fn empty_source() {
            let s = crate::Span::default().trim_trailing_whitespace();

            assert_eq!(
                s,
                Span {
                    data: "",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );
        }

        #[test]
        fn nothing_to_trim() {
            let s = crate::Span::new("foo").trim_trailing_whitespace();

            assert_eq!(
                s,
                Span {
                    data: "foo",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );
        }

        #[test]
        fn space_in_middle() {
            let s = crate::Span::new("foo bar").trim_trailing_whitespace();

            assert_eq!(
                s,
                Span {
                    data: "foo bar",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );
        }

        #[test]
        fn trailing_space() {
            let s = crate::Span::new("foo ").trim_trailing_whitespace();

            assert_eq!(
                s,
                Span {
                    data: "foo",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );
        }

        #[test]
        fn trailing_newlines() {
            let s = crate::Span::new("foo\n\n").trim_trailing_whitespace();

            assert_eq!(
                s,
                Span {
                    data: "foo",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );
        }
    }

    mod trim_trailing_line_end {
        use pretty_assertions_sorted::assert_eq;

        use crate::tests::prelude::*;

        #[test]
        fn empty_source() {
            let s = crate::Span::default().trim_trailing_line_end();

            assert_eq!(
                s,
                Span {
                    data: "",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );
        }

        #[test]
        fn nothing_to_trim() {
            let s = crate::Span::new("foo").trim_trailing_line_end();

            assert_eq!(
                s,
                Span {
                    data: "foo",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );
        }

        #[test]
        fn space_in_middle() {
            let s = crate::Span::new("foo bar").trim_trailing_line_end();

            assert_eq!(
                s,
                Span {
                    data: "foo bar",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );
        }

        #[test]
        fn trailing_space() {
            let s = crate::Span::new("foo ").trim_trailing_line_end();

            assert_eq!(
                s,
                Span {
                    data: "foo ",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );
        }

        #[test]
        fn trailing_newlines() {
            let s = crate::Span::new("foo\n\n").trim_trailing_line_end();

            assert_eq!(
                s,
                Span {
                    data: "foo\n",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );
        }

        #[test]
        fn trailing_windows_newlines() {
            let s = crate::Span::new("foo\n\r\n").trim_trailing_line_end();

            assert_eq!(
                s,
                Span {
                    data: "foo\n",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );
        }
    }
}
