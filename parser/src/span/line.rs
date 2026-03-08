use super::{MatchedItem, Span};

impl<'src> Span<'src> {
    /// Split the span, consuming a single line from the source.
    ///
    /// A line is terminated by end-of-input or a single `\n` character
    /// or a single `\r\n` sequence. The end of line sequence is consumed
    /// but not included in the returned line.
    pub(crate) fn take_line(self) -> MatchedItem<'src, Self> {
        let line = match self.find('\n') {
            Some(index) => self.into_parse_result(index),
            None => self.into_parse_result(self.len()),
        };

        line.trim_after_start_matches('\n')
            .trim_item_end_matches('\r')
    }

    /// Split the span, consuming a single line and normalizing it.
    ///
    /// A line is terminated by end-of-input or a single `\n` character
    /// or a single `\r\n` sequence. The end of line sequence is consumed
    /// but not included in the returned line.
    ///
    /// All trailing spaces are removed from the line.
    pub(crate) fn take_normalized_line(self) -> MatchedItem<'src, Self> {
        self.take_line().trim_item_trailing_spaces()
    }

    /// Split the span, consuming a single _normalized, non-empty_ line from the
    /// source if one exists.
    ///
    /// A line is terminated by end-of-input or a single `\n` character
    /// or a single `\r\n` sequence. The end of line sequence is consumed
    /// but not included in the returned line.
    ///
    /// All trailing spaces are removed from the line.
    ///
    /// Returns `None` if the line becomes empty after trailing spaces have been
    /// removed.
    pub(crate) fn take_non_empty_line(self) -> Option<MatchedItem<'src, Self>> {
        self.split_at_match_non_empty(|c| c == '\n')
            .map(|mi| {
                mi.trim_after_start_matches('\n')
                    .trim_item_end_matches('\r')
                    .trim_item_trailing_spaces()
            })
            .filter(|line| !line.item.is_empty())
    }

    /// Split the span, assuming the span begins with an empty line.
    ///
    /// An empty line may contain any number of white space characters.
    ///
    /// Returns `None` if the first line of the span contains any
    /// non-white-space characters.
    pub(crate) fn take_empty_line(self) -> Option<MatchedItem<'src, Self>> {
        let l = self.take_line();

        if l.item.data().bytes().all(|b| b == b' ' || b == b'\t') {
            Some(l)
        } else {
            None
        }
    }

    /// Discard zero or more empty lines.
    ///
    /// Return the original span if no empty lines are found.
    pub(crate) fn discard_empty_lines(self) -> Span<'src> {
        let mut i = self;

        while !i.data().is_empty() {
            match i.take_empty_line() {
                Some(line) => i = line.after,
                None => break,
            }
        }

        i
    }

    /// Split the span, consuming one normalized, non-empty line that may be
    /// continued onto subsequent lines with an explicit continuation marker.
    ///
    /// A line is terminated by end-of-input or a single `\n` character
    /// or a single `\r\n` sequence. The end of line sequence is consumed
    /// but not included in the returned line.
    ///
    /// A line is _not_ terminated by `\n` when preceded by a `\` character.
    ///
    /// SPEC QUESTION: Is it allowed to have a `+` character followed by
    /// white space? For now, I'm saying yes.
    ///
    /// Trailing white spaces are removed from the final line and are not
    /// removed from any lines with continuations.
    ///
    /// Returns `None` if the line becomes empty after trailing spaces have been
    /// removed.
    pub(crate) fn take_line_with_continuation(self) -> Option<MatchedItem<'src, Self>> {
        // Consume any number of lines terminated by '\\'.
        let mut mi = self.into_parse_result(0);
        while let Some(new_pr) = mi.after.one_line_with_continuation() {
            mi = new_pr;
        }

        // Consume at most one line without a `\\` terminator.
        mi = mi.after.take_line();

        let mi = self
            .into_parse_result(mi.after.byte_offset() - self.byte_offset())
            .trim_item_end_matches('\n')
            .trim_item_end_matches('\r')
            .trim_item_trailing_spaces();

        if mi.item.is_empty() { None } else { Some(mi) }
    }

    fn one_line_with_continuation(self) -> Option<MatchedItem<'src, Self>> {
        let line = self.take_normalized_line();
        if line.item.ends_with('\\') {
            Some(line)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    mod take_line {
        use crate::tests::prelude::*;

        #[test]
        fn empty_source() {
            let span = crate::Span::default();
            let l = span.take_line();

            assert_eq!(
                l.after,
                Span {
                    data: "",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );

            assert_eq!(
                l.item,
                Span {
                    data: "",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn simple_line() {
            let span = crate::Span::new("abc");
            let l = span.take_line();

            assert_eq!(
                l.after,
                Span {
                    data: "",
                    line: 1,
                    col: 4,
                    offset: 3
                }
            );

            assert_eq!(
                l.item,
                Span {
                    data: "abc",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn trailing_space() {
            let span = crate::Span::new("abc ");
            let l = span.take_line();

            assert_eq!(
                l.after,
                Span {
                    data: "",
                    line: 1,
                    col: 5,
                    offset: 4
                }
            );

            assert_eq!(
                l.item,
                Span {
                    data: "abc ",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn consumes_lf() {
            // Should consume but not return \n.

            let span = crate::Span::new("abc\ndef");
            let l = span.take_line();

            assert_eq!(
                l.after,
                Span {
                    data: "def",
                    line: 2,
                    col: 1,
                    offset: 4
                }
            );

            assert_eq!(
                l.item,
                Span {
                    data: "abc",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn consumes_crlf() {
            // Should consume but not return \r\n.

            let span = crate::Span::new("abc\r\ndef");
            let l = span.take_line();

            assert_eq!(
                l.after,
                Span {
                    data: "def",
                    line: 2,
                    col: 1,
                    offset: 5
                }
            );

            assert_eq!(
                l.item,
                Span {
                    data: "abc",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn doesnt_consume_lfcr() {
            // Should consume \n but not a subsequent \r.

            let span = crate::Span::new("abc\n\rdef");
            let l = span.take_line();

            assert_eq!(
                l.after,
                Span {
                    data: "\rdef",
                    line: 2,
                    col: 1,
                    offset: 4
                }
            );

            assert_eq!(
                l.item,
                Span {
                    data: "abc",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn standalone_cr_doesnt_end_line() {
            // Shouldn't terminate line at \r without \n.

            let span = crate::Span::new("abc\rdef");
            let l = span.take_line();

            assert_eq!(
                l.after,
                Span {
                    data: "",
                    line: 1,
                    col: 8,
                    offset: 7
                }
            );

            assert_eq!(
                l.item,
                Span {
                    data: "abc\rdef",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }
    }

    mod take_normalized_line {
        use crate::tests::prelude::*;

        #[test]
        fn empty_source() {
            let span = crate::Span::default();
            let line = span.take_normalized_line();

            assert_eq!(
                line.after,
                Span {
                    data: "",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn simple_line() {
            let span = crate::Span::new("abc");
            let line = span.take_normalized_line();

            assert_eq!(
                line.after,
                Span {
                    data: "",
                    line: 1,
                    col: 4,
                    offset: 3
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "abc",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn discards_trailing_space() {
            let span = crate::Span::new("abc ");
            let line = span.take_normalized_line();

            assert_eq!(
                line.after,
                Span {
                    data: "",
                    line: 1,
                    col: 5,
                    offset: 4
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "abc",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn discards_multiple_trailing_spaces() {
            let span = crate::Span::new("abc   ");
            let line = span.take_normalized_line();

            assert_eq!(
                line.after,
                Span {
                    data: "",
                    line: 1,
                    col: 7,
                    offset: 6
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "abc",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn consumes_lf() {
            // Should consume but not return \n.

            let span = crate::Span::new("abc  \ndef");
            let line = span.take_normalized_line();

            assert_eq!(
                line.after,
                Span {
                    data: "def",
                    line: 2,
                    col: 1,
                    offset: 6
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "abc",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn consumes_crlf() {
            // Should consume but not return \r\n.

            let span = crate::Span::new("abc  \r\ndef");
            let line = span.take_normalized_line();

            assert_eq!(
                line.after,
                Span {
                    data: "def",
                    line: 2,
                    col: 1,
                    offset: 7
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "abc",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn doesnt_consume_lfcr() {
            // Should consume \n but not a subsequent \r.

            let span = crate::Span::new("abc  \n\rdef");
            let line = span.take_normalized_line();

            assert_eq!(
                line.after,
                Span {
                    data: "\rdef",
                    line: 2,
                    col: 1,
                    offset: 6
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "abc",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn standalone_cr_doesnt_end_line() {
            // Shouldn't terminate normalized line at \r without \n.

            let span = crate::Span::new("abc   \rdef");
            let line = span.take_normalized_line();

            assert_eq!(
                line.after,
                Span {
                    data: "",
                    line: 1,
                    col: 11,
                    offset: 10
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "abc   \rdef",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }
    }

    mod take_non_empty_line {
        use crate::tests::prelude::*;

        #[test]
        fn empty_source() {
            let span = crate::Span::default();
            assert!(span.take_non_empty_line().is_none());
        }

        #[test]
        fn only_spaces() {
            let span = crate::Span::new("   ");
            assert!(span.take_non_empty_line().is_none());
        }

        #[test]
        fn simple_line() {
            let span = crate::Span::new("abc");
            let line = span.take_non_empty_line().unwrap();

            assert_eq!(
                line.after,
                Span {
                    data: "",
                    line: 1,
                    col: 4,
                    offset: 3
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "abc",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn discards_trailing_space() {
            let span = crate::Span::new("abc ");
            let line = span.take_non_empty_line().unwrap();

            assert_eq!(
                line.after,
                Span {
                    data: "",
                    line: 1,
                    col: 5,
                    offset: 4
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "abc",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn consumes_lf() {
            // Should consume but not return \n.

            let span = crate::Span::new("abc  \ndef");
            let line = span.take_non_empty_line().unwrap();

            assert_eq!(
                line.after,
                Span {
                    data: "def",
                    line: 2,
                    col: 1,
                    offset: 6
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "abc",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn consumes_crlf() {
            // Should consume but not return \r\n.

            let span = crate::Span::new("abc  \r\ndef");
            let line = span.take_non_empty_line().unwrap();

            assert_eq!(
                line.after,
                Span {
                    data: "def",
                    line: 2,
                    col: 1,
                    offset: 7
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "abc",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn doesnt_consume_lfcr() {
            // Should consume \n but not a subsequent \r.

            let span = crate::Span::new("abc  \n\rdef");
            let line = span.take_non_empty_line().unwrap();

            assert_eq!(
                line.after,
                Span {
                    data: "\rdef",
                    line: 2,
                    col: 1,
                    offset: 6
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "abc",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn standalone_cr_doesnt_end_line() {
            // Shouldn't terminate line at \r without \n.

            let span = crate::Span::new("abc   \rdef");
            let line = span.take_non_empty_line().unwrap();

            assert_eq!(
                line.after,
                Span {
                    data: "",
                    line: 1,
                    col: 11,
                    offset: 10
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "abc   \rdef",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }
    }

    mod take_empty_line {
        use crate::tests::prelude::*;

        #[test]
        fn empty_source() {
            let span = crate::Span::default();
            let line = span.take_empty_line().unwrap();

            assert_eq!(
                line.after,
                Span {
                    data: "",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn simple_line() {
            let span = crate::Span::new("abc");
            assert!(span.take_empty_line().is_none());
        }

        #[test]
        fn leading_space() {
            let span = crate::Span::new("  abc");
            assert!(span.take_empty_line().is_none());
        }

        #[test]
        fn consumes_spaces() {
            // Should consume a source containing only spaces.

            let span = crate::Span::new("     ");
            let line = span.take_empty_line().unwrap();

            assert_eq!(
                line.after,
                Span {
                    data: "",
                    line: 1,
                    col: 6,
                    offset: 5
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "     ",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn consumes_spaces_and_tabs() {
            // Should consume a source containing spaces and tabs.

            let span = crate::Span::new("  \t  ");
            let line = span.take_empty_line().unwrap();

            assert_eq!(
                line.after,
                Span {
                    data: "",
                    line: 1,
                    col: 6,
                    offset: 5
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "  \t  ",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn consumes_lf() {
            // Should consume but not return \n.

            let span = crate::Span::new("   \ndef");
            let line = span.take_empty_line().unwrap();

            assert_eq!(
                line.after,
                Span {
                    data: "def",
                    line: 2,
                    col: 1,
                    offset: 4
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "   ",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn consumes_crlf() {
            // Should consume but not return \r\n.

            let span = crate::Span::new("   \r\ndef");
            let line = span.take_empty_line().unwrap();

            assert_eq!(
                line.after,
                Span {
                    data: "def",
                    line: 2,
                    col: 1,
                    offset: 5
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "   ",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn doesnt_consume_lfcr() {
            // Should consume \n but not a subsequent \r.

            let span = crate::Span::new("   \n\rdef");
            let line = span.take_empty_line().unwrap();

            assert_eq!(
                line.after,
                Span {
                    data: "\rdef",
                    line: 2,
                    col: 1,
                    offset: 4
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "   ",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn standalone_cr_doesnt_end_line() {
            // A "line" with \r and no immediate \n is not considered empty.

            let span = crate::Span::new("   \rdef");
            assert!(span.take_empty_line().is_none());
        }
    }

    mod discard_empty_lines {
        use crate::tests::prelude::*;

        #[test]
        fn empty_source() {
            let span = crate::Span::default();
            let after = span.discard_empty_lines();
            assert_eq!(after, crate::Span::new("",));
        }

        #[test]
        fn consumes_empty_line() {
            let span = crate::Span::new("\nabc");
            let after = span.discard_empty_lines();

            assert_eq!(
                after,
                Span {
                    data: "abc",
                    line: 2,
                    col: 1,
                    offset: 1
                }
            );
        }

        #[test]
        fn doesnt_consume_non_empty_line() {
            let span = crate::Span::new("abc");
            let after = span.discard_empty_lines();

            assert_eq!(
                after,
                Span {
                    data: "abc",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn doesnt_consume_leading_space() {
            let span = crate::Span::new("   abc");
            let after = span.discard_empty_lines();

            assert_eq!(
                after,
                Span {
                    data: "   abc",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn consumes_line_with_only_spaces() {
            let span = crate::Span::new("   \nabc");
            let after = span.discard_empty_lines();

            assert_eq!(
                after,
                Span {
                    data: "abc",
                    line: 2,
                    col: 1,
                    offset: 4
                }
            );
        }

        #[test]
        fn consumes_spaces_and_tabs() {
            let span = crate::Span::new(" \t \nabc");
            let after = span.discard_empty_lines();

            assert_eq!(
                after,
                Span {
                    data: "abc",
                    line: 2,
                    col: 1,
                    offset: 4
                }
            );
        }

        #[test]
        fn consumes_multiple_lines() {
            let span = crate::Span::new("\n  \t \n\nabc");
            let after = span.discard_empty_lines();

            assert_eq!(
                after,
                Span {
                    data: "abc",
                    line: 4,
                    col: 1,
                    offset: 7
                }
            );
        }

        #[test]
        fn consumes_crlf() {
            // Should consume \r\n sequence.

            let span = crate::Span::new("  \r\ndef");
            let after = span.discard_empty_lines();

            assert_eq!(
                after,
                Span {
                    data: "def",
                    line: 2,
                    col: 1,
                    offset: 4
                }
            );
        }

        #[test]
        fn doesnt_consume_lfcr() {
            // Should consume \n but not a subsequent \r.

            let span = crate::Span::new("   \n\rdef");
            let after = span.discard_empty_lines();

            assert_eq!(
                after,
                Span {
                    data: "\rdef",
                    line: 2,
                    col: 1,
                    offset: 4
                }
            );
        }

        #[test]
        fn standalone_cr_doesnt_end_line() {
            // A "line" with \r and no immediate \n is not considered empty.

            let span = crate::Span::new("   \rdef");
            let after = span.discard_empty_lines();

            assert_eq!(
                after,
                Span {
                    data: "   \rdef",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }
    }

    mod take_line_with_continuation {
        use crate::tests::prelude::*;

        #[test]
        fn empty_source() {
            let span = crate::Span::default();
            assert!(span.take_line_with_continuation().is_none());
        }

        #[test]
        fn only_spaces() {
            let span = crate::Span::new("   ");
            assert!(span.take_line_with_continuation().is_none());
        }

        #[test]
        fn simple_line() {
            let span = crate::Span::new("abc");
            let line = span.take_line_with_continuation().unwrap();

            assert_eq!(
                line.after,
                Span {
                    data: "",
                    line: 1,
                    col: 4,
                    offset: 3
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "abc",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn discards_trailing_space() {
            let span = crate::Span::new("abc ");
            let line = span.take_line_with_continuation().unwrap();

            assert_eq!(
                line.after,
                Span {
                    data: "",
                    line: 1,
                    col: 5,
                    offset: 4
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "abc",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn consumes_lf() {
            // Should consume but not return \n.

            let span = crate::Span::new("abc  \ndef");
            let line = span.take_line_with_continuation().unwrap();

            assert_eq!(
                line.after,
                Span {
                    data: "def",
                    line: 2,
                    col: 1,
                    offset: 6
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "abc",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn consumes_crlf() {
            // Should consume but not return \r\n.

            let span = crate::Span::new("abc  \r\ndef");
            let line = span.take_line_with_continuation().unwrap();

            assert_eq!(
                line.after,
                Span {
                    data: "def",
                    line: 2,
                    col: 1,
                    offset: 7
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "abc",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn doesnt_consume_lfcr() {
            // Should consume \n but not a subsequent \r.

            let span = crate::Span::new("abc  \n\rdef");
            let line = span.take_line_with_continuation().unwrap();

            assert_eq!(
                line.after,
                Span {
                    data: "\rdef",
                    line: 2,
                    col: 1,
                    offset: 6
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "abc",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn standalone_cr_doesnt_end_line() {
            // Shouldn't terminate line at \r without \n.

            let span = crate::Span::new("abc   \rdef");
            let line = span.take_line_with_continuation().unwrap();

            assert_eq!(
                line.after,
                Span {
                    data: "",
                    line: 1,
                    col: 11,
                    offset: 10
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "abc   \rdef",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn simple_continuation() {
            let span = crate::Span::new("abc \\\ndef");
            let line = span.take_line_with_continuation().unwrap();

            assert_eq!(
                line.after,
                Span {
                    data: "",
                    line: 2,
                    col: 4,
                    offset: 9
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "abc \\\ndef",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn simple_continuation_with_crlf() {
            let span = crate::Span::new("abc \\\r\ndef");
            let line = span.take_line_with_continuation().unwrap();

            assert_eq!(
                line.after,
                Span {
                    data: "",
                    line: 2,
                    col: 4,
                    offset: 10
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "abc \\\r\ndef",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn continuation_with_trailing_space() {
            let span = crate::Span::new("abc \\   \ndef");
            let line = span.take_line_with_continuation().unwrap();

            assert_eq!(
                line.after,
                Span {
                    data: "",
                    line: 2,
                    col: 4,
                    offset: 12
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "abc \\   \ndef",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn multiple_continuations() {
            let span = crate::Span::new("abc \\\ndef\\\nghi");
            let line = span.take_line_with_continuation().unwrap();

            assert_eq!(
                line.after,
                Span {
                    data: "",
                    line: 3,
                    col: 4,
                    offset: 14
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "abc \\\ndef\\\nghi",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn terminates_on_line_without_trailing_slash() {
            let span = crate::Span::new("abc \\\ndef  \nghi");
            let line = span.take_line_with_continuation().unwrap();

            assert_eq!(
                line.after,
                Span {
                    data: "ghi",
                    line: 3,
                    col: 1,
                    offset: 12
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "abc \\\ndef",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }

        #[test]
        fn doesnt_consume_empty_line() {
            let span = crate::Span::new("abc \\\ndef\n\nghi");
            let line = span.take_line_with_continuation().unwrap();

            assert_eq!(
                line.after,
                Span {
                    data: "\nghi",
                    line: 3,
                    col: 1,
                    offset: 10
                }
            );

            assert_eq!(
                line.item,
                Span {
                    data: "abc \\\ndef",
                    line: 1,
                    col: 1,
                    offset: 0
                }
            );
        }
    }
}
