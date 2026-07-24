//! Helper for defining named, opaque iterator types over borrowed slices.

/// Defines a public, named iterator that borrows a slice but hides the concrete
/// [`std::slice::Iter`] behind an opaque wrapper, so the container type is not
/// part of the public contract.
///
/// The generated type forwards the full slice-iterator behavior it would be
/// reasonable to depend on: it is an [`ExactSizeIterator`] (so `len()` works),
/// a [`DoubleEndedIterator`], and a
/// [`FusedIterator`](std::iter::FusedIterator).
macro_rules! opaque_slice_iter {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident<$lt:lifetime> yielding $item:ty;
    ) => {
        $(#[$meta])*
        $vis struct $name<$lt>(::std::slice::Iter<$lt, $item>);

        impl<$lt> $name<$lt> {
            /// Wraps a slice's iterator in this opaque type.
            pub(crate) fn new(slice: &$lt [$item]) -> Self {
                Self(slice.iter())
            }
        }

        impl<$lt> ::std::iter::Iterator for $name<$lt> {
            type Item = &$lt $item;

            #[inline]
            fn next(&mut self) -> Option<Self::Item> {
                self.0.next()
            }

            #[inline]
            fn size_hint(&self) -> (usize, Option<usize>) {
                self.0.size_hint()
            }

            #[inline]
            fn nth(&mut self, n: usize) -> Option<Self::Item> {
                self.0.nth(n)
            }
        }

        impl<$lt> ::std::iter::ExactSizeIterator for $name<$lt> {}

        impl<$lt> ::std::iter::DoubleEndedIterator for $name<$lt> {
            #[inline]
            fn next_back(&mut self) -> Option<Self::Item> {
                self.0.next_back()
            }
        }

        impl<$lt> ::std::iter::FusedIterator for $name<$lt> {}
    };
}

pub(crate) use opaque_slice_iter;
