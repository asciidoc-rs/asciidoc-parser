pub(crate) mod base64;
pub(crate) mod chars;
pub(crate) mod debug;
pub(crate) mod opaque_iter;
mod regex;
pub(crate) use chars::is_word_char;
pub(crate) use regex::{LookaheadReplacer, LookaheadResult, replace_with_lookahead};
