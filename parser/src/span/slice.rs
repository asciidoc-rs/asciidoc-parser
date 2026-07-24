use std::ops::{Range, RangeFrom, RangeTo};

use bytecount::num_chars;
use memchr::Memchr;

use super::Span;

impl<'src> Span<'src> {
    /// Returns the requested subrange of this input span.
    ///
    /// The range is expected to be in bounds; callers pass internally-derived
    /// ranges, so an out-of-bounds range indicates a parser bug. Debug builds
    /// assert loudly to surface such a bug in tests. Release builds fall back
    /// to an empty span (rather than silently returning the whole span, which
    /// would masquerade as valid location data).
    pub(crate) fn slice(&self, range: Range<usize>) -> Self {
        debug_assert!(
            self.data.get(range.clone()).is_some(),
            "slice: range {:?} is out of bounds for data of length {}",
            range,
            self.data.len()
        );

        self.data
            .get(range)
            .map_or_else(|| self.empty(), |s| self.slice_internal(s))
    }

    /// Returns the requested subrange of this input span.
    ///
    /// The range is expected to be in bounds; callers pass internally-derived
    /// ranges, so an out-of-bounds range indicates a parser bug. Debug builds
    /// assert loudly to surface such a bug in tests. Release builds fall back
    /// to an empty span (rather than silently returning the whole span, which
    /// would masquerade as valid location data).
    pub(crate) fn slice_from(&self, range: RangeFrom<usize>) -> Self {
        debug_assert!(
            self.data.get(range.clone()).is_some(),
            "slice_from: range {:?} is out of bounds for data of length {}",
            range,
            self.data.len()
        );

        self.data
            .get(range)
            .map_or_else(|| self.empty(), |s| self.slice_internal(s))
    }

    /// Returns the requested subrange of this input span.
    ///
    /// The range is expected to be in bounds; callers pass internally-derived
    /// ranges, so an out-of-bounds range indicates a parser bug. Debug builds
    /// assert loudly to surface such a bug in tests. Release builds fall back
    /// to an empty span (rather than silently returning the whole span, which
    /// would masquerade as valid location data).
    pub(crate) fn slice_to(&self, range: RangeTo<usize>) -> Self {
        debug_assert!(
            self.data.get(range).is_some(),
            "slice_to: range {:?} is out of bounds for data of length {}",
            range,
            self.data.len()
        );

        self.data
            .get(range)
            .map_or_else(|| self.empty(), |s| self.slice_internal(s))
    }

    /// Returns an empty span anchored at the start of this span.
    ///
    /// Used as a clearly-degenerate fallback when an internal slice request is
    /// out of bounds in a release build; debug builds panic before reaching
    /// here so the underlying parser bug surfaces in tests.
    fn empty(&self) -> Self {
        Self {
            data: "",
            line: self.line,
            col: self.col,
            offset: self.offset,
        }
    }

    /// Returns the first position where `predicate` returns `true`.
    pub(crate) fn position<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(char) -> bool,
    {
        for (o, c) in self.data.char_indices() {
            if predicate(c) {
                return Some(o);
            }
        }

        None
    }

    fn slice_internal(&self, slice_data: &'src str) -> Self {
        let offset = offset(self.data, slice_data);

        if offset == 0 {
            return Self {
                data: slice_data,
                line: self.line,
                col: self.col,
                offset: self.offset,
            };
        }

        debug_assert!(
            offset <= self.data.len(),
            "slice_internal: offset {} is out of bounds for data of length {}",
            offset,
            self.data.len()
        );

        let old_data = self.data.get(..offset).unwrap_or(self.data);
        let new_line_iter = Memchr::new(b'\n', old_data.as_bytes());

        let mut lines_to_add = 0;
        let mut last_index = None;
        for i in new_line_iter {
            lines_to_add += 1;
            last_index = Some(i);
        }

        let last_index = last_index.map_or(0, |v| v + 1);
        let col = old_data.as_bytes().get(last_index..).map_or(0, num_chars);

        Self {
            data: slice_data,
            line: self.line + lines_to_add,
            col: if lines_to_add == 0 {
                self.col + col
            } else {
                // When going to a new line, char starts at 1.
                col + 1
            },
            offset: self.offset + offset,
        }
    }
}

fn offset(first: &str, second: &str) -> usize {
    let p1 = first.as_ptr();
    let p2 = second.as_ptr();
    p2 as usize - p1 as usize
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    mod slice {
        use crate::tests::prelude::*;

        #[test]
        fn base_case() {
            let s = crate::Span::new("abcdef");

            assert_eq!(
                s.slice(1..4),
                Span {
                    data: "bcd",
                    line: 1,
                    col: 2,
                    offset: 1,
                }
            );
        }

        #[test]
        #[should_panic(expected = "slice: range 4..8 is out of bounds for data of length 6")]
        fn out_of_bounds_panics() {
            let s = crate::Span::new("abcdef");
            let _ = s.slice(4..8);
        }
    }

    mod slice_from {
        use crate::tests::prelude::*;

        #[test]
        fn base_case() {
            let s = crate::Span::new("abcdef");

            assert_eq!(
                s.slice_from(2..),
                Span {
                    data: "cdef",
                    line: 1,
                    col: 3,
                    offset: 2,
                }
            );
        }

        #[test]
        #[should_panic(expected = "slice_from: range 7.. is out of bounds for data of length 6")]
        fn out_of_bounds_panics() {
            let s = crate::Span::new("abcdef");
            let _ = s.slice_from(7..);
        }
    }

    mod slice_to {
        use crate::tests::prelude::*;

        #[test]
        fn base_case() {
            let s = crate::Span::new("abcdef");

            assert_eq!(
                s.slice_to(..3),
                Span {
                    data: "abc",
                    line: 1,
                    col: 1,
                    offset: 0,
                }
            );
        }

        #[test]
        #[should_panic(expected = "slice_to: range ..9 is out of bounds for data of length 6")]
        fn out_of_bounds_panics() {
            let s = crate::Span::new("abcdef");
            let _ = s.slice_to(..9);
        }
    }
}
