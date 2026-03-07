use std::ops::{Range, RangeFrom, RangeTo};

use bytecount::num_chars;
use memchr::Memchr;

use super::Span;

impl<'src> Span<'src> {
    /// Returns the requested subrange of this input span.
    pub(crate) fn slice(&self, range: Range<usize>) -> Self {
        self.data
            .get(range)
            .map_or(*self, |s| self.slice_internal(s))
    }

    /// Returns the requested subrange of this input span.
    pub(crate) fn slice_from(&self, range: RangeFrom<usize>) -> Self {
        self.data
            .get(range)
            .map_or(*self, |s| self.slice_internal(s))
    }

    /// Returns the requested subrange of this input span.
    pub(crate) fn slice_to(&self, range: RangeTo<usize>) -> Self {
        self.data
            .get(range)
            .map_or(*self, |s| self.slice_internal(s))
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
                // When going to a new line, char starts at 1
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
