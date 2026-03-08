use std::borrow::Cow;

use regex::{Captures, Regex};

/// Similar to `[regex::Regex::replace_all]` but takes a `[LookaheadReplacer]`
/// which can look ahead after the match and potentially abort the replacement.
pub fn replace_with_lookahead<'h, LR: LookaheadReplacer>(
    regex: &Regex,
    mut haystack: &'h str,
    mut rep: LR,
) -> Cow<'h, str> {
    let mut new = String::with_capacity(haystack.len());
    let mut last_match = 0;

    'retry: loop {
        // Optimization: If we don't have any matches, we can continue to borrow the
        // source.
        let mut it = regex.captures_iter(haystack).enumerate().peekable();

        if new.is_empty() && it.peek().is_none() {
            return Cow::Borrowed(haystack);
        }

        for (_i, cap) in it {
            // unwrap on 0 is OK because captures only reports matches
            #[allow(clippy::unwrap_used)]
            let m = cap.get(0).unwrap();
            new.push_str(&haystack[last_match..m.start()]);

            let after = &haystack[m.end()..];

            match rep.replace_append(&cap, &mut new, after) {
                LookaheadResult::Continue => {
                    last_match = m.end();
                }

                LookaheadResult::SkipAheadAndRetry(n) => {
                    assert!(n > 0);
                    haystack = &haystack[m.start() + n..];
                    continue 'retry;
                }
            }
        }

        break;
    }

    new.push_str(&haystack[last_match..]);
    Cow::Owned(new)
}

/// Result of a call to `[LookaheadReplacer::replace_append`].
pub(crate) enum LookaheadResult {
    /// Replacement was successful. Continue as normal.
    Continue,

    /// Look-ahead was unsuccessful. Skip _n_ characters and restart the search.
    SkipAheadAndRetry(usize),
}

/// Alternative to
pub(crate) trait LookaheadReplacer {
    fn replace_append(
        &mut self,
        caps: &Captures<'_>,
        dst: &mut String,
        after_match: &str,
    ) -> LookaheadResult;
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::borrow::Cow;

    use regex::{Captures, Regex, RegexBuilder};

    use crate::internal::{LookaheadReplacer, LookaheadResult, replace_with_lookahead};

    #[test]
    fn example() {
        let re = Regex::new(r"(?m)^(\S+)[\s--\r\n]+(\S+)$").unwrap();

        let hay = "
Greetings  1973
Wild\t1973
BornToRun\t\t\t\t1975
Darkness                    1978
TheRiver 1980
";

        let new = replace_with_lookahead(&re, hay, "$2 $1");

        assert_eq!(
            new,
            "
1973 Greetings
1973 Wild
1975 BornToRun
1978 Darkness
1980 TheRiver
"
        );
    }

    #[test]
    fn no_match_optimization() {
        let re = Regex::new(r"(?m)^(\S+)[\s--\r\n]+(\S+)$").unwrap();

        let hay = "blah_blah_blah\nno_match";

        let new = replace_with_lookahead(&re, hay, "$2 $1");

        assert_eq!(new, Cow::Borrowed(hay));
    }

    impl LookaheadReplacer for &str {
        fn replace_append(
            &mut self,
            caps: &Captures<'_>,
            dst: &mut String,
            _after: &str,
        ) -> LookaheadResult {
            caps.expand(self, dst);
            LookaheadResult::Continue
        }
    }

    #[test]
    fn match_after_quote() {
        let re =
            RegexBuilder::new(r#"(^|[^\w&;:"'`}])(?:\[([^\[\]]+)\])?`(\S|\S.*?\S)`\b{end-half}"#)
                .dot_matches_new_line(true)
                .build()
                .unwrap();

        let hay = "Olaf had been with the company since the `'00s.\nHis desk overflowed with heaps of paper, apple cores and squeaky toys.\nWe couldn't find Olaf's keyboard.\nThe state of his desk was replicated, in triplicate, across all of\nthe werewolves`' desks.";

        let new = replace_with_lookahead(&re, hay, Rejectifier {});

        assert_eq!(
            new,
            "Olaf had been with the company since the `'HELLO00s.\nHis desk overflowed with heaps of paper, apple cores and squeaky toys.\nWe couldn't find Olaf's keyboard.\nThe state of his desk was replicated, in triplicate, across all of\nthe werewolves`' desks."
        );
    }

    struct Rejectifier {}

    impl LookaheadReplacer for Rejectifier {
        fn replace_append(
            &mut self,
            caps: &Captures<'_>,
            dst: &mut String,
            after: &str,
        ) -> LookaheadResult {
            assert_eq!(after, "' desks.");
            dst.push_str(&caps[0][0..3]);
            dst.push_str("HELLO");
            LookaheadResult::SkipAheadAndRetry(3)
        }
    }
}
