//! A **frozen, checked-in oracle** for this module's differential corpora.
//!
//! # Why this exists
//!
//! Every corpus on this branch was born a differential: it rendered a fixture
//! two ways — through the string pipeline and through the tree — and asserted
//! the two agree. The step 6 cutover ended the independence that rested on
//! (once `rendered_html()` is a fold of the tree, a golden computed at test
//! time is the fold), so each corpus's golden became a **recording**:
//! `snapshots/<corpus>.txt`, checked in, reviewed like any other file, and
//! read rather than derived. The fold is compared against bytes that were
//! settled before it ran — which no amount of rearranging the fold can satisfy
//! tautologically.
//!
//! The string pipeline that produced those bytes is gone, so the recordings
//! now **stand alone**, exactly as the ~277 golden-HTML assertions (§5.3)
//! always have. While the pipeline existed, every checking run re-derived the
//! golden and asserted it still matched the recording (the drift guard), and
//! `ASCIIDOC_UPDATE_SNAPSHOTS=1` could regenerate a corpus from it; both went
//! with the pipeline. What remains is the read side, and one rule: **a
//! recording is edited by hand, reviewed like the behavior change it
//! records.**
//!
//! # Adding or changing a fixture
//!
//! A fixture with no recording fails with a message naming the corpus file;
//! where the caller holds the fold, the message includes the ready-to-paste
//! line. Adding that line asserts "this rendering is correct" — review it as
//! such, exactly as an expected-output literal in any golden test. Records are
//! one per line, `{source:?}\t{rendered:?}`, sorted by source; both halves are
//! `Debug`-escaped so a record is always exactly one physical line.
//!
//! A corpus of **documented divergences** (read through
//! [`matches_recording`]) records what the string pipeline used to produce,
//! and is the one kind that must never be "refreshed" from current behavior:
//! its rows are the frozen half of a comparison whose whole point is that the
//! fold may differ.

// Test-only harness: a malformed or missing recording is a broken corpus, and
// failing loudly at the point of breakage beats threading a `Result` through a
// test helper.
#![allow(clippy::panic)]
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::indexing_slicing)]

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard, OnceLock},
};

/// Where the recordings live, relative to the crate root.
const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/snapshots/");

/// One recording store: a directory of corpus files.
///
/// The directory is a **field rather than a global** so the harness can be
/// driven end to end by its own tests, over a temporary directory, instead of
/// only against the checked-in one.
struct Store<'a> {
    dir: &'a Path,
}

impl Store<'_> {
    /// The real store: the checked-in directory.
    fn real() -> Store<'static> {
        Store {
            dir: Path::new(DIR),
        }
    }

    fn path(&self, corpus: &str) -> PathBuf {
        self.dir.join(format!("{corpus}.txt"))
    }

    /// Looks the recording up. `None` means no fixture with this source is
    /// recorded; every panic a caller raises for that happens **outside** the
    /// lock, so one missing fixture cannot poison the map for every later
    /// corpus in the process.
    fn recorded_for(&self, corpus: &str, source: &str) -> Option<String> {
        let key = self.path(corpus);

        let mut all = lock();

        all.entry(key.clone())
            .or_insert_with(|| load_from(&key, corpus))
            .get(source)
            .cloned()
    }

    /// [`assert_recorded`], against this store.
    fn assert_recorded(&self, corpus: &str, source: &str, folded: &str) {
        match self.recorded_for(corpus, source) {
            Some(recorded) => {
                // The assertion the whole harness exists for: the fold against
                // bytes settled before it ran.
                assert_eq!(
                    folded, recorded,
                    "the fold diverged from the recorded rendering for {source:?} in {corpus}.txt"
                );
            }

            // The caller holds the fold, so the message can offer the
            // ready-to-paste line — adding it asserts the rendering is
            // correct, so review it as the golden it becomes.
            None => panic!(
                "no recording for {source:?} in {corpus}.txt — if this rendering is correct, add \
                 this line (keeping the file sorted):\n{}\t{}",
                quote(source),
                quote(folded)
            ),
        }
    }

    /// [`recorded`], against this store.
    fn recorded(&self, corpus: &str, source: &str) -> String {
        self.recorded_for(corpus, source).unwrap_or_else(|| {
            panic!(
                "no recording for {source:?} in {corpus}.txt — add a `{{source:?}}\\t\\
                 {{rendered:?}}` line for it (keeping the file sorted) and review it as the \
                 golden it becomes"
            )
        })
    }

    /// [`matches_recording`], against this store.
    fn matches_recording(&self, corpus: &str, source: &str, folded: &str) -> bool {
        folded == self.recorded(corpus, source)
    }
}

/// Parses one recording file into a `source -> rendered` map. `corpus` is
/// carried only to name the file in a failure.
///
/// A missing **file** is a broken corpus like a missing fixture is — there is
/// no update mode left to create one — but it is reported per fixture, by the
/// lookup, where the message can say what to create.
fn load_from(file: &Path, corpus: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();

    let text = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return out,
        Err(error) => panic!("cannot read {corpus}.txt: {error}"),
    };

    for line in text.lines() {
        let Some((key, value)) = line.split_once('\t') else {
            panic!("malformed recording line in {corpus}.txt: {line:?}");
        };

        out.insert(unquote(corpus, key), unquote(corpus, value));
    }

    out
}

/// Reverses the `{:?}` escaping [`quote`] applies.
///
/// Hand-rolled rather than pulled from a crate: the escapes `{:?}` emits for
/// the strings a corpus holds are a small, closed set (`\"`, `\\`, `\n`, `\r`,
/// `\t`, `\0`, and `\u{...}` for the Private-Use-Area sentinels and any other
/// non-printable), and a dependency in the dev graph for that is a poor trade.
/// An apostrophe is deliberately *not* in that set: `{:?}` on a `&str` leaves
/// it bare, so a `\'` branch here would be unreachable.
pub(crate) fn unquote(corpus: &str, field: &str) -> String {
    let body = field
        .strip_prefix('"')
        .and_then(|f| f.strip_suffix('"'))
        .unwrap_or_else(|| panic!("unquoted field in {corpus}.txt: {field:?}"));

    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }

        match chars.next() {
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('0') => out.push('\0'),

            Some('u') => {
                // `\u{...}`
                let mut hex = String::new();

                for c in chars.by_ref() {
                    match c {
                        '{' => {}
                        '}' => break,
                        _ => hex.push(c),
                    }
                }

                let code = u32::from_str_radix(&hex, 16)
                    .unwrap_or_else(|_| panic!("bad \\u escape in {corpus}.txt: {hex:?}"));

                out.push(
                    char::from_u32(code)
                        .unwrap_or_else(|| panic!("bad code point in {corpus}.txt: {code}")),
                );
            }

            other => panic!("unsupported escape in {corpus}.txt: \\{other:?}"),
        }
    }

    out
}

pub(crate) fn quote(s: &str) -> String {
    format!("{s:?}")
}

/// Every corpus file touched by this process, loaded once. Keyed by **path**
/// rather than by name, so a store over a temporary directory cannot collide
/// with the checked-in one.
fn recordings() -> &'static Mutex<BTreeMap<PathBuf, BTreeMap<String, String>>> {
    static RECORDINGS: OnceLock<Mutex<BTreeMap<PathBuf, BTreeMap<String, String>>>> =
        OnceLock::new();

    RECORDINGS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// Locks the recordings, **recovering from poisoning**.
///
/// A failing corpus assertion is a panic, and one panic must not take every
/// later test in the same process down with it: the default `unwrap()` would
/// turn the first genuine corpus failure into a cascade of `PoisonError`s that
/// hide it. Recovery is sound because a panic cannot leave this map
/// inconsistent — the only mutations are whole-entry inserts, and a panic
/// between two of them leaves a map that is merely missing an entry the next
/// caller reloads.
fn lock() -> MutexGuard<'static, BTreeMap<PathBuf, BTreeMap<String, String>>> {
    recordings()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Checks the tree's `folded` rendering of `source` against `corpus`'s recorded
/// known-good bytes.
pub(super) fn assert_recorded(corpus: &str, source: &str, folded: &str) {
    Store::real().assert_recorded(corpus, source, folded);
}

/// [`assert_recorded`], reporting whether the fold matched instead of asserting
/// it.
///
/// For the corpora whose subject is a **documented divergence** or a set of
/// them (the cross-product sweep): a fixture diverges exactly when the fold
/// differs from the recorded rendering — the bytes the string pipeline
/// produced while it existed, which is what such a corpus deliberately keeps.
pub(super) fn matches_recording(corpus: &str, source: &str, folded: &str) -> bool {
    Store::real().matches_recording(corpus, source, folded)
}

/// The recorded bytes for `source` in `corpus` — the value every `golden_*`
/// helper now returns.
///
/// Those helpers are the majority of this harness's surface: a per-family test
/// module reads its golden once, in a `golden_*` function, and each of its
/// several dozen call sites then uses that string however it likes — comparing
/// a fold against it, comparing it against a literal, asserting a *documented
/// divergence* from it with `assert_ne!`, or merely testing it with
/// `contains`. Routing the helper's return value through the recording covers
/// all of them at once, and is what made the string pipeline's deletion a
/// *local* change: each helper's body became this lookup, its callers did not
/// move, and every corpus goes on asserting exactly what it asserted before —
/// against bytes settled while the pipeline still existed.
pub(crate) fn recorded(corpus: &str, source: &str) -> String {
    Store::real().recorded(corpus, source)
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, path::PathBuf};

    use super::{Store, load_from, quote, unquote};

    /// The strings a recording actually has to survive: the escapes `{:?}`
    /// emits, and the Private-Use-Area sentinels the string pipeline's own
    /// output carried.
    const TRICKY: &[&str] = &[
        "",
        "plain",
        "a\tb",
        "a\nb",
        "a\r\nb",
        "quote \" and backslash \\",
        "escaped \\\" pair",
        "nul \0 byte",
        "apostrophe ' here",
        "\u{96}0\u{97}",
        "\u{e000}1\u{e001}",
        "unicode — em dash and &#8217;",
        "<strong>a</strong> &amp; b",
    ];

    /// A scratch directory of this test's own, removed when the guard drops so
    /// a failing test cannot leak one into the next run.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("asciidoc-parser-snapshot-{name}"));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn store(&self) -> Store<'_> {
            Store { dir: &self.0 }
        }

        /// Writes `corpus.txt` holding exactly `entries`, in the recorded
        /// format — the hand edit the module docs describe, performed by the
        /// test.
        fn record(&self, corpus: &str, entries: &BTreeMap<String, String>) {
            let mut out = String::new();

            for (source, rendered) in entries {
                out.push_str(&quote(source));
                out.push('\t');
                out.push_str(&quote(rendered));
                out.push('\n');
            }

            std::fs::write(self.0.join(format!("{corpus}.txt")), out).unwrap();
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn quote_round_trips_every_escape_a_recording_can_hold() {
        for original in TRICKY {
            let round_tripped = unquote("test", &quote(original));

            assert_eq!(
                &round_tripped.as_str(),
                original,
                "round trip failed for {original:?}"
            );
        }
    }

    #[test]
    fn a_file_round_trips_through_the_recorded_format() {
        let dir = TempDir::new("round-trip");

        let mut entries = BTreeMap::new();

        for (index, source) in TRICKY.iter().enumerate() {
            entries.insert((*source).to_string(), format!("rendered {index}\n{source}"));
        }

        dir.record("corpus", &entries);

        assert_eq!(load_from(&dir.0.join("corpus.txt"), "test"), entries);

        for (source, rendered) in &entries {
            assert_eq!(&dir.store().recorded("corpus", source), rendered);
        }
    }

    #[test]
    fn a_missing_file_loads_as_empty() {
        let dir = TempDir::new("missing-file");

        assert!(load_from(&dir.0.join("nope.txt"), "test").is_empty());
    }

    #[test]
    fn a_matching_fold_passes_and_a_divergent_one_is_reported() {
        let dir = TempDir::new("check");

        dir.record(
            "corpus",
            &BTreeMap::from([("src".to_string(), "recorded".to_string())]),
        );

        dir.store().assert_recorded("corpus", "src", "recorded");

        assert!(dir.store().matches_recording("corpus", "src", "recorded"));
        assert!(
            !dir.store()
                .matches_recording("corpus", "src", "a divergent fold")
        );
    }

    #[test]
    #[should_panic(expected = "the fold diverged from the recorded rendering")]
    fn a_fold_that_differs_from_the_recording_is_rejected() {
        let dir = TempDir::new("wrong-fold");

        dir.record(
            "corpus",
            &BTreeMap::from([("src".to_string(), "recorded".to_string())]),
        );

        dir.store().assert_recorded("corpus", "src", "a wrong fold");
    }

    #[test]
    #[should_panic(expected = "no recording for")]
    fn a_fixture_with_no_recording_offers_the_line_to_add() {
        let dir = TempDir::new("no-recording");

        dir.store().assert_recorded("corpus", "src", "folded");
    }

    #[test]
    #[should_panic(expected = "no recording for")]
    fn recorded_reports_a_missing_recording() {
        let dir = TempDir::new("recorded-missing");

        let _ = dir.store().recorded("corpus", "src");
    }
}
