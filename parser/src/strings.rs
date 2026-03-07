// Adapted from pulldown-cmark, which comes with the following license:
//
// Copyright 2015 Google Inc. All rights reserved.
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

//! String types that facilitate parsing.

use std::{
    borrow::{Borrow, Cow},
    fmt,
    hash::{Hash, Hasher},
    ops::Deref,
    str::from_utf8,
};

pub(crate) const MAX_INLINE_STR_LEN: usize = 3 * std::mem::size_of::<isize>() - 2;

/// Returned when trying to convert a `&str` into an [`InlineStr`] but it fails
/// because it doesn't fit.
#[derive(Debug)]
pub struct StringTooLongError;

/// An inline string that can contain almost three words
/// of UTF-8 text.
#[derive(Debug, Clone, Copy, Eq)]
pub struct InlineStr {
    inner: [u8; MAX_INLINE_STR_LEN],
    len: u8,
}

impl AsRef<str> for InlineStr {
    fn as_ref(&self) -> &str {
        self.deref()
    }
}

impl Hash for InlineStr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.deref().hash(state);
    }
}

impl From<char> for InlineStr {
    fn from(c: char) -> Self {
        let mut inner = [0u8; MAX_INLINE_STR_LEN];
        c.encode_utf8(&mut inner);
        let len = c.len_utf8() as u8;
        Self { inner, len }
    }
}

impl std::cmp::PartialEq<InlineStr> for InlineStr {
    fn eq(&self, other: &InlineStr) -> bool {
        self.deref() == other.deref()
    }
}

impl TryFrom<&str> for InlineStr {
    type Error = StringTooLongError;

    fn try_from(s: &str) -> Result<InlineStr, StringTooLongError> {
        let len = s.len();
        if len <= MAX_INLINE_STR_LEN {
            let mut inner = [0u8; MAX_INLINE_STR_LEN];

            debug_assert!(
                len <= MAX_INLINE_STR_LEN,
                "InlineStr: len {} exceeds MAX_INLINE_STR_LEN {}",
                len,
                MAX_INLINE_STR_LEN
            );

            if let Some(dest) = inner.get_mut(..len) {
                dest.copy_from_slice(s.as_bytes());
            }

            let len = len as u8;
            Ok(Self { inner, len })
        } else {
            Err(StringTooLongError)
        }
    }
}

impl Deref for InlineStr {
    type Target = str;

    fn deref(&self) -> &str {
        let len = self.len as usize;
        self.inner
            .get(..len)
            .and_then(|bytes| from_utf8(bytes).ok())
            .unwrap_or_default()
    }
}

impl fmt::Display for InlineStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

/// A copy-on-write string that can be owned, borrowed,
/// or inlined.
///
/// It is three words long.
///
/// NOTE: The [`Debug`] implementation for this struct elides the storage
/// mechanism that is chosen when pretty printing (as occurs when using the
/// `dbg!()` macro. To obtain that information, use the “normal” debug
/// formatting as shown below:
///
/// ```
/// # use asciidoc_parser::strings::CowStr;
///
/// let s: &'static str = "0123456789abcdefghijklm";
/// let s: CowStr = s.into();
/// assert_eq!(
///     format!("{s:?}"),
///     "CowStr::Borrowed(\"0123456789abcdefghijklm\")"
/// );
/// ```
#[derive(Eq)]
pub enum CowStr<'a> {
    /// An owned, immutable string.
    Boxed(Box<str>),
    /// A borrowed string.
    Borrowed(&'a str),
    /// A short inline string.
    Inlined(InlineStr),
}

impl AsRef<str> for CowStr<'_> {
    fn as_ref(&self) -> &str {
        self.deref()
    }
}

impl Hash for CowStr<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.deref().hash(state);
    }
}

impl std::clone::Clone for CowStr<'_> {
    fn clone(&self) -> Self {
        match self {
            CowStr::Boxed(s) => match InlineStr::try_from(&**s) {
                Ok(inline) => CowStr::Inlined(inline),
                Err(..) => CowStr::Boxed(s.clone()),
            },
            CowStr::Borrowed(s) => CowStr::Borrowed(s),
            CowStr::Inlined(s) => CowStr::Inlined(*s),
        }
    }
}

impl<'a> std::cmp::PartialEq<CowStr<'a>> for CowStr<'a> {
    fn eq(&self, other: &CowStr<'_>) -> bool {
        self.deref() == other.deref()
    }
}

impl<'a> From<&'a str> for CowStr<'a> {
    fn from(s: &'a str) -> Self {
        CowStr::Borrowed(s)
    }
}

impl From<String> for CowStr<'_> {
    fn from(s: String) -> Self {
        CowStr::Boxed(s.into_boxed_str())
    }
}

impl From<char> for CowStr<'_> {
    fn from(c: char) -> Self {
        CowStr::Inlined(c.into())
    }
}

impl<'a> From<Cow<'a, str>> for CowStr<'a> {
    fn from(s: Cow<'a, str>) -> Self {
        match s {
            Cow::Borrowed(s) => CowStr::Borrowed(s),
            Cow::Owned(s) => CowStr::Boxed(s.into_boxed_str()),
        }
    }
}

impl<'a> From<CowStr<'a>> for Cow<'a, str> {
    fn from(s: CowStr<'a>) -> Self {
        match s {
            CowStr::Boxed(s) => Cow::Owned(s.to_string()),
            CowStr::Inlined(s) => Cow::Owned(s.to_string()),
            CowStr::Borrowed(s) => Cow::Borrowed(s),
        }
    }
}

impl<'a> From<Cow<'a, char>> for CowStr<'a> {
    fn from(s: Cow<'a, char>) -> Self {
        CowStr::Inlined(InlineStr::from(*s))
    }
}

impl Deref for CowStr<'_> {
    type Target = str;

    fn deref(&self) -> &str {
        match self {
            CowStr::Boxed(b) => b,
            CowStr::Borrowed(b) => b,
            CowStr::Inlined(s) => s.deref(),
        }
    }
}

impl Borrow<str> for CowStr<'_> {
    fn borrow(&self) -> &str {
        self.deref()
    }
}

impl CowStr<'_> {
    /// Convert the `CowStr` into an owned `String`.
    pub fn into_string(self) -> String {
        match self {
            CowStr::Boxed(b) => b.into(),
            CowStr::Borrowed(b) => b.to_owned(),
            CowStr::Inlined(s) => s.deref().to_owned(),
        }
    }
}

impl fmt::Display for CowStr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

impl fmt::Debug for CowStr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            write!(f, "{:?}", self.as_ref())
        } else {
            match self {
                Self::Boxed(b) => f.debug_tuple("CowStr::Boxed").field(b).finish(),
                Self::Borrowed(b) => f.debug_tuple("CowStr::Borrowed").field(b).finish(),
                Self::Inlined(s) => f.debug_tuple("CowStr::Inlined").field(s).finish(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    mod string_too_long_err {
        use crate::strings::StringTooLongError;

        #[test]
        fn impl_debug() {
            let e = StringTooLongError;
            assert_eq!(format!("{e:#?}"), "StringTooLongError");
        }
    }

    mod inline_str {
        use std::ops::Deref;

        use crate::strings::*;

        #[test]
        fn from_ascii() {
            let s: InlineStr = 'a'.into();
            assert_eq!("a", s.deref());
        }

        #[test]
        fn from_unicode() {
            let s: InlineStr = '🍔'.into();
            assert_eq!("🍔", s.deref());
        }

        #[test]
        fn impl_debug() {
            let s: InlineStr = 'a'.into();
            assert_eq!(
                format!("{s:#?}"),
                r#"InlineStr {
    inner: [
        97,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ],
    len: 1,
}"#
            );
        }

        #[test]
        fn impl_hash() {
            use std::{
                collections::hash_map::DefaultHasher,
                hash::{Hash, Hasher},
            };

            let mut hasher = DefaultHasher::new();
            "🍔".hash(&mut hasher);
            let expected = hasher.finish();

            let s: InlineStr = '🍔'.into();
            let mut hasher = DefaultHasher::new();
            s.hash(&mut hasher);
            let actual = hasher.finish();

            let s: InlineStr = 'a'.into();
            let mut hasher = DefaultHasher::new();
            s.hash(&mut hasher);
            let mismatch = hasher.finish();

            assert_eq!(expected, actual);
            assert_ne!(expected, mismatch);
        }

        #[test]
        fn impl_partial_eq() {
            let s1: InlineStr = '🍔'.into();
            let s2: InlineStr = '🍔'.into();
            let s3: InlineStr = 'a'.into();

            assert_eq!(s1, s2);
            assert_ne!(s1, s3);
        }

        #[test]
        #[allow(clippy::assertions_on_constants)]
        fn max_len_atleast_four() {
            // we need 4 bytes to store a char
            assert!(MAX_INLINE_STR_LEN >= 4);
        }

        #[test]
        #[cfg(target_pointer_width = "64")]
        fn fits_twentytwo() {
            let s = "0123456789abcdefghijkl";
            let stack_str = InlineStr::try_from(s).unwrap();
            assert_eq!(stack_str.deref(), s);
        }

        #[test]
        #[cfg(target_pointer_width = "64")]
        fn doesnt_fit_twentythree() {
            let s = "0123456789abcdefghijklm";
            let _stack_str = InlineStr::try_from(s).unwrap_err();
        }
    }

    mod cow_str {
        use std::{
            borrow::{Borrow, Cow},
            ops::Deref,
        };

        use crate::strings::*;

        #[test]
        fn size() {
            let size = std::mem::size_of::<CowStr>();
            let word_size = std::mem::size_of::<isize>();
            assert_eq!(3 * word_size, size);
        }

        #[test]
        fn char_to_string() {
            let c = '藏';
            let smort: CowStr = c.into();
            let owned: String = smort.to_string();
            let expected = "藏".to_owned();
            assert_eq!(expected, owned);
        }

        #[test]
        #[cfg(target_pointer_width = "64")]
        fn small_boxed_str_clones_to_stack() {
            let s = "0123456789abcde".to_owned();
            let smort: CowStr = s.into();
            let smort_clone = smort.clone();

            if let CowStr::Inlined(..) = smort_clone {
            } else {
                panic!("Expected a Inlined variant!");
            }
        }

        #[test]
        fn cow_to_cow_str() {
            let s = "some text";
            let cow = Cow::Borrowed(s);
            let actual = CowStr::from(cow);
            let expected = CowStr::Borrowed(s);
            assert_eq!(actual, expected);
            assert!(variant_eq(&actual, &expected));

            let s = "some text".to_string();
            let cow: Cow<str> = Cow::Owned(s.clone());
            let actual = CowStr::from(cow);
            let expected = CowStr::Boxed(s.into_boxed_str());
            assert_eq!(actual, expected);
            assert!(variant_eq(&actual, &expected));
        }

        #[test]
        fn cow_str_to_cow() {
            let s = "some text";
            let cow_str = CowStr::Borrowed(s);
            let actual = Cow::from(cow_str);
            let expected = Cow::Borrowed(s);
            assert_eq!(actual, expected);
            assert!(variant_eq(&actual, &expected));

            let s = "s";
            let inline_str: InlineStr = InlineStr::try_from(s).unwrap();
            let cow_str = CowStr::Inlined(inline_str);
            let actual = Cow::from(cow_str);
            let expected: Cow<str> = Cow::Owned(s.to_string());
            assert_eq!(actual, expected);
            assert!(variant_eq(&actual, &expected));

            let s = "s";
            let cow_str = CowStr::Boxed(s.to_string().into_boxed_str());
            let actual = Cow::from(cow_str);
            let expected: Cow<str> = Cow::Owned(s.to_string());
            assert_eq!(actual, expected);
            assert!(variant_eq(&actual, &expected));
        }

        #[test]
        fn cow_char_to_cow_str() {
            let c = 'c';
            let cow: Cow<char> = Cow::Owned(c);
            let actual = CowStr::from(cow);
            let expected = CowStr::Inlined(InlineStr::from(c));
            assert_eq!(actual, expected);
            assert!(variant_eq(&actual, &expected));

            let c = 'c';
            let cow: Cow<char> = Cow::Borrowed(&c);
            let actual = CowStr::from(cow);
            let expected = CowStr::Inlined(InlineStr::from(c));
            assert_eq!(actual, expected);
            assert!(variant_eq(&actual, &expected));
        }

        fn variant_eq<T>(a: &T, b: &T) -> bool {
            std::mem::discriminant(a) == std::mem::discriminant(b)
        }

        #[test]
        fn impl_debug_pretty_print_for_inline() {
            let c = '藏';
            let s: CowStr = c.into();

            assert_eq!(format!("{s:#?}"), r#""藏""#);
        }

        #[test]
        fn impl_debug_pretty_print_for_boxed() {
            let s = "blah blah blah".to_string();
            let s: CowStr = s.into();

            assert_eq!(format!("{s:#?}"), r#""blah blah blah""#);
        }

        #[test]
        fn impl_debug_pretty_print_for_borrowed() {
            let s: &'static str = "0123456789abcdefghijklm";
            let s: CowStr = s.into();

            assert_eq!(format!("{s:#?}"), r#""0123456789abcdefghijklm""#);
        }

        #[test]
        fn impl_debug_for_inline() {
            let c = '藏';
            let s: CowStr = c.into();

            assert_eq!(
                format!("{s:?}"),
                "CowStr::Inlined(InlineStr { inner: [232, 151, 143, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], len: 3 })"
            );
        }

        #[test]
        fn impl_debug_for_boxed() {
            let s = "blah blah blah".to_string();
            let s: CowStr = s.into();

            assert_eq!(format!("{s:?}"), "CowStr::Boxed(\"blah blah blah\")");
        }

        #[test]
        fn impl_debug_for_borrowed() {
            let s: &'static str = "0123456789abcdefghijklm";
            let s: CowStr = s.into();

            assert_eq!(
                format!("{s:?}"),
                "CowStr::Borrowed(\"0123456789abcdefghijklm\")"
            );
        }

        #[test]
        fn impl_clone_boxed_long() {
            let s = "this string won't fit in a box".to_owned();
            let s: CowStr = s.into();
            if let CowStr::Boxed(_) = s {
            } else {
                panic!("Expected Boxed case");
            }

            let s2 = s.clone();
            assert_eq!(s.deref(), s2.deref());

            if let CowStr::Boxed(_) = s2 {
            } else {
                panic!("Expected Boxed clone");
            }
        }

        #[test]
        fn impl_clone_borrowed() {
            let s = "this long string is borrowed";
            let s: CowStr = s.into();
            if let CowStr::Borrowed(_) = s {
            } else {
                panic!("Expected Borrowed case");
            }

            let s2 = s.clone();
            assert_eq!(s.deref(), s2.deref());

            if let CowStr::Borrowed(_) = s2 {
            } else {
                panic!("Expected Borrowed clone");
            }
        }

        #[test]
        fn impl_clone_inlined() {
            let s: CowStr = 's'.into();
            if let CowStr::Inlined(_) = s {
            } else {
                panic!("Expected Inlined case");
            }

            let s2 = s.clone();
            assert_eq!(s.deref(), s2.deref());

            if let CowStr::Inlined(_) = s2 {
            } else {
                panic!("Expected Inlined clone");
            }
        }

        #[test]
        fn impl_hash() {
            use std::{
                collections::hash_map::DefaultHasher,
                hash::{Hash, Hasher},
            };

            let mut hasher = DefaultHasher::new();
            "🍔".hash(&mut hasher);
            let expected = hasher.finish();

            let s: CowStr = '🍔'.into();
            if let CowStr::Inlined(_) = s {
            } else {
                panic!("Expected Inlined case");
            }
            let mut hasher = DefaultHasher::new();
            s.hash(&mut hasher);
            let actual = hasher.finish();
            assert_eq!(expected, actual);

            let s = CowStr::Borrowed("🍔");
            let mut hasher = DefaultHasher::new();
            s.hash(&mut hasher);
            let actual = hasher.finish();
            assert_eq!(expected, actual);

            let s = "🍔".to_owned();
            let s: CowStr = s.into();
            if let CowStr::Boxed(_) = s {
            } else {
                panic!("Expected Boxed case");
            }
            let mut hasher = DefaultHasher::new();
            s.hash(&mut hasher);
            assert_eq!(expected, actual);
        }

        #[test]
        fn impl_from_str() {
            let s = "xyz";
            let s: CowStr = s.into();
            assert_eq!(s.deref(), "xyz");

            if let CowStr::Borrowed(_) = s {
            } else {
                panic!("Expected Borrowed case");
            }
        }

        #[test]
        fn impl_borrow() {
            let s: CowStr = "xyz".into();
            let s: &str = s.borrow();
            assert_eq!(s, "xyz");
        }

        #[test]
        fn into_string_boxed() {
            let s = "this string won't fit in a box".to_owned();
            let s: CowStr = s.into();
            if let CowStr::Boxed(_) = s {
            } else {
                panic!("Expected Boxed case");
            }

            let s2 = s.into_string();
            assert_eq!(&s2, "this string won't fit in a box");
        }

        #[test]
        fn into_string_borrowed() {
            let s = "this long string is borrowed";
            let s: CowStr = s.into();
            if let CowStr::Borrowed(_) = s {
            } else {
                panic!("Expected Borrowed case");
            }

            let s2 = s.into_string();
            assert_eq!(&s2, "this long string is borrowed");
        }

        #[test]
        fn into_string_inlined() {
            let s: CowStr = 's'.into();
            if let CowStr::Inlined(_) = s {
            } else {
                panic!("Expected Inlined case");
            }

            let s2 = s.into_string();
            assert_eq!(&s2, "s");
        }
    }
}
