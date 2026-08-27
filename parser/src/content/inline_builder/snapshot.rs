//! A **frozen, checked-in oracle** for this module's differential corpora.
//!
//! # Why this exists
//!
//! Every corpus on this branch is a differential: it renders a fixture two
//! ways — through the string pipeline and through the tree — and asserts the
//! two agree. That works only while the two are genuinely independent, and the
//! step 6 cutover ends that. Once `rendered_html()` is a fold of the tree, a
//! corpus that takes its golden by running the pipeline and reading `rendered`
//! is **comparing the fold against itself**, and passes for that reason. A
//! green suite says nothing about it.
//!
//! So the golden stops being computed at test time and becomes a recording:
//! `snapshots/<corpus>.txt`, checked in, reviewed like any other file, and read
//! rather than derived. The fold is then compared against bytes that were
//! settled before it ran — which no amount of rearranging the fold can satisfy
//! tautologically.
//!
//! # The asymmetry that makes it work
//!
//! [`assert_recorded`] takes the golden and the fold as **separate**
//! parameters, and they are not interchangeable:
//!
//! - the **fold** is only ever compared against the recording, never written to
//!   it;
//! - the **golden** is what `ASCIIDOC_UPDATE_SNAPSHOTS=1` writes, and — in
//!   normal runs — is itself checked against the recording, so a recording
//!   cannot silently rot while the string pipeline still exists.
//!
//! That second check is the transitional half. When the string pipeline is
//! finally deleted, callers stop passing a golden and the recordings stand
//! alone, exactly as the ~277 golden-HTML assertions (§5.3) already do.
//!
//! # Regenerating
//!
//! ```text
//! ASCIIDOC_UPDATE_SNAPSHOTS=1 cargo test -p asciidoc-parser --lib
//! ```
//!
//! That is a plain, multi-threaded `cargo test` on purpose: recording is safe
//! under concurrency, and it is pinned that way
//! (`concurrent_update_runs_keep_every_fixture`). It was not always — see the
//! note on the write in [`Store::recorded_for`].
//!
//! Recordings are **merged**, not replaced, so a filtered run only adds and
//! updates the fixtures it reached. Removing a fixture means deleting its line
//! by hand — deliberately, since a corpus silently shrinking is the failure
//! this whole file exists to prevent. Review the resulting diff: a line that
//! changes is a rendering that changed.
//!
//! # Format
//!
//! One record per line, `{source:?}\t{rendered:?}`, sorted by source. Both
//! halves are `Debug`-escaped so a record is always exactly one physical line
//! (a multi-line fixture's `\n` stays an escape), and sorting keeps the diff
//! stable when a corpus is reordered.

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

/// One recording store: a directory of corpus files, and whether this run is
/// regenerating them rather than checking against them.
///
/// The directory and the mode are **fields rather than globals** so the harness
/// can be driven end to end by its own tests — over a temporary directory, in
/// both modes — instead of only in the one configuration a `cargo test` run
/// happens to have. Recording machinery that is itself only half-exercised is a
/// poor foundation for a corpus that exists to stop things being
/// half-exercised.
struct Store<'a> {
    dir: &'a Path,
    updating: bool,
}

impl Store<'_> {
    /// The real store: the checked-in directory, in the mode
    /// `ASCIIDOC_UPDATE_SNAPSHOTS` selects.
    fn real() -> Store<'static> {
        Store {
            dir: Path::new(DIR),
            updating: std::env::var_os("ASCIIDOC_UPDATE_SNAPSHOTS").is_some(),
        }
    }

    fn path(&self, corpus: &str) -> PathBuf {
        self.dir.join(format!("{corpus}.txt"))
    }

    /// Records in update mode (returning `None`), and otherwise looks the
    /// recording up and runs the drift guard against `golden`.
    ///
    /// Every panic here happens **after** the lock guard is dropped: an
    /// assertion is a panic, and panicking while holding the lock would poison
    /// it for every later corpus in the process, turning one honest failure
    /// into a cascade that hides it.
    fn recorded_for(&self, corpus: &str, source: &str, golden: &str) -> Option<String> {
        let key = self.path(corpus);

        let outcome = {
            let mut all = lock();

            let entries = all
                .entry(key.clone())
                .or_insert_with(|| load_from(&key, corpus));

            let decision = decide(entries, source, golden, self.updating);

            // The write happens **while the lock is still held**. It has to:
            // the file must reflect the map generation that was just produced,
            // and releasing first lets another thread's *older* generation land
            // last and silently delete every fixture recorded since. That is
            // not theoretical — with the write outside the lock, thirty-two
            // concurrent recordings leave a file holding **one** of them, and
            // the documented regeneration command is a plain, multi-threaded
            // `cargo test`, so that is the ordinary path rather than an exotic
            // one.
            //
            // Holding the lock across the write means an IO failure panics
            // under it. That is the rare, already-catastrophic case, and it is
            // exactly what `lock`'s poison recovery exists for; the *common*
            // panics (a missing recording, a conflict, a failed assertion) all
            // still happen below, after the guard is dropped.
            if let Decision::Recorded(snapshot) = &decision {
                std::fs::create_dir_all(self.dir).expect("create snapshots dir");
                write_to(&key, snapshot);
            }

            decision
        };

        match outcome {
            Decision::Recorded(_) => None,

            Decision::Conflict { existing } => panic!(
                "conflicting recordings for {source:?} in {corpus}.txt:\n  {existing:?}\n  \
                 {golden:?}"
            ),

            Decision::Missing => panic!(
                "no recording for {source:?} in {corpus}.txt — run `ASCIIDOC_UPDATE_SNAPSHOTS=1 \
                 cargo test -p asciidoc-parser --lib` and review the diff"
            ),

            Decision::Check(recorded) => {
                // The drift guard: the string pipeline must still produce what
                // was recorded. Deleted along with the string pipeline itself,
                // at which point the recording stands alone.
                assert_eq!(
                    golden, recorded,
                    "the string pipeline no longer produces the recorded rendering for {source:?} \
                     in {corpus}.txt"
                );

                Some(recorded)
            }
        }
    }

    /// [`assert_recorded`], against this store.
    fn assert_recorded(&self, corpus: &str, source: &str, golden: &str, folded: &str) {
        if let Some(recorded) = self.recorded_for(corpus, source, golden) {
            // The real assertion: the fold against bytes settled before it ran.
            assert_eq!(
                folded, recorded,
                "the fold diverged from the recorded rendering for {source:?} in {corpus}.txt"
            );
        }
    }

    /// [`recorded_golden`], against this store.
    fn recorded_golden(&self, corpus: &str, source: &str, golden: &str) -> String {
        self.recorded_for(corpus, source, golden)
            .unwrap_or_else(|| golden.to_string())
    }

    /// [`matches_recording`], against this store.
    fn matches_recording(&self, corpus: &str, source: &str, golden: &str, folded: &str) -> bool {
        match self.recorded_for(corpus, source, golden) {
            // Update mode records rather than compares. The recording is being
            // written *from* `golden`, so comparing against `golden` is the
            // same question the checking run will ask of the recording — which
            // keeps a regeneration run's own divergence set honest instead of
            // collapsing it to "nothing diverges".
            None => folded == golden,
            Some(recorded) => folded == recorded,
        }
    }
}

/// What a lookup decided, computed with **no filesystem and no environment**:
/// the caller performs any resulting IO, and raises any resulting panic once it
/// no longer holds the lock.
#[derive(Debug, PartialEq)]
enum Decision {
    /// Update mode: the golden was merged in, and these entries should be
    /// written out.
    Recorded(BTreeMap<String, String>),

    /// Update mode, but a fixture with this source is already recorded with a
    /// *different* rendering. Recording it would silently overwrite its twin
    /// and leave a file matching neither, so it is refused: the two fixtures
    /// need separate corpora, or one is an accidental duplicate.
    Conflict { existing: String },

    /// Check mode: compare the fold against these bytes.
    Check(String),

    /// Check mode, with nothing recorded for this source.
    Missing,
}

/// The whole decision, as a pure function of the entries and the mode.
fn decide(
    entries: &mut BTreeMap<String, String>,
    source: &str,
    golden: &str,
    updating: bool,
) -> Decision {
    if !updating {
        return match entries.get(source) {
            Some(recorded) => Decision::Check(recorded.clone()),
            None => Decision::Missing,
        };
    }

    if let Some(existing) = entries.get(source)
        && existing != golden
    {
        return Decision::Conflict {
            existing: existing.clone(),
        };
    }

    entries.insert(source.to_string(), golden.to_string());

    Decision::Recorded(entries.clone())
}

/// Parses one recording file into a `source -> rendered` map. A missing file
/// reads as empty, which is what lets a brand-new corpus be created by a single
/// `ASCIIDOC_UPDATE_SNAPSHOTS=1` run. `corpus` is carried only to name the file
/// in a parse failure.
fn load_from(file: &Path, corpus: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();

    let text = match std::fs::read_to_string(file) {
        Ok(text) => text,

        // A *missing* file reads as empty: that is what lets a brand-new corpus
        // be created by one update run. Any other failure must not — treating,
        // say, a permission error as "no fixtures recorded" would have an
        // update run rewrite the file from scratch and silently shrink the
        // corpus to whatever this invocation happened to reach.
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

fn write_to(file: &Path, entries: &BTreeMap<String, String>) {
    let mut out = String::new();

    for (source, rendered) in entries {
        out.push_str(&quote(source));
        out.push('\t');
        out.push_str(&quote(rendered));
        out.push('\n');
    }

    std::fs::write(file, out).expect("write recording");
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

/// Every corpus file touched by this process, loaded once and (in update mode)
/// accumulated into. Keyed by **path** rather than by name, so a store over a
/// temporary directory cannot collide with the checked-in one.
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
/// known-good bytes — and, while the string pipeline still exists, checks that
/// the recording still agrees with its `golden` too.
///
/// See the module docs for why the two are separate parameters and why only one
/// of them is ever written.
pub(super) fn assert_recorded(corpus: &str, source: &str, golden: &str, folded: &str) {
    Store::real().assert_recorded(corpus, source, golden, folded);
}

/// [`assert_recorded`], reporting whether the fold matched instead of asserting
/// it.
///
/// For the cross-product sweep, whose subject is the **set** of diverging
/// (container, construct) pairs rather than any one pair: a pair diverges
/// exactly when the fold differs from the recorded rendering. The drift guard
/// still asserts — a recording the string pipeline no longer agrees with is a
/// broken recording however its caller uses it — and update mode still records
/// only the golden.
pub(super) fn matches_recording(corpus: &str, source: &str, golden: &str, folded: &str) -> bool {
    Store::real().matches_recording(corpus, source, golden, folded)
}

/// Freezes a golden-producing helper's output into `corpus`, and hands back the
/// **recorded** bytes for the caller to assert against.
///
/// This is [`assert_recorded`] turned inside out, for the corpora whose golden
/// is produced by a helper that never sees the fold. Those helpers are the
/// majority: a per-family test module computes its golden once, in a
/// `golden_*` function, and each of its several dozen call sites then uses that
/// string however it likes — comparing a fold against it, comparing it against
/// a literal, asserting a *documented divergence* from it with `assert_ne!`, or
/// merely testing it with `contains`. There is no single assertion to wrap.
///
/// Routing the helper's *return value* through the recording covers all of them
/// at once, and keeps the same asymmetry: the golden is the only thing
/// `ASCIIDOC_UPDATE_SNAPSHOTS=1` writes, and in a checking run it is verified
/// against the recording (the drift guard) before the recording — not the
/// freshly computed golden — is what the caller gets back. So every one of
/// those call sites is already comparing against bytes settled before the fold
/// ran, without a single one of them being edited.
///
/// It is also what makes the string pipeline's deletion a *local* change: a
/// helper's body becomes a lookup, its callers do not move, and the corpus goes
/// on asserting exactly what it asserted before.
///
/// In update mode there is nothing recorded to hand back, so the caller gets
/// the golden and its own assertion compares the fold against it — which is the
/// question a checking run will then ask of the recording, so a regeneration
/// run stays as honest as the run that follows it (the same reasoning
/// [`matches_recording`] documents).
pub(crate) fn recorded_golden(corpus: &str, source: &str, golden: &str) -> String {
    Store::real().recorded_golden(corpus, source, golden)
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, path::PathBuf};

    use super::{Decision, Store, decide, load_from, quote, unquote, write_to};

    /// The strings a recording actually has to survive: the escapes `{:?}`
    /// emits, and the Private-Use-Area sentinels the string pipeline's own
    /// output carries.
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

        fn store(&self, updating: bool) -> Store<'_> {
            Store {
                dir: &self.0,
                updating,
            }
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
    fn a_file_round_trips_through_write_and_load() {
        let dir = TempDir::new("round-trip");
        let file = dir.0.join("corpus.txt");

        let mut entries = BTreeMap::new();

        for (index, source) in TRICKY.iter().enumerate() {
            entries.insert((*source).to_string(), format!("rendered {index}\n{source}"));
        }

        write_to(&file, &entries);

        assert_eq!(load_from(&file, "test"), entries);

        // Sorted by source, one physical line per record — the two properties
        // the format exists for.
        let text = std::fs::read_to_string(&file).unwrap();
        assert_eq!(text.lines().count(), entries.len());

        let keys: Vec<String> = text
            .lines()
            .map(|line| unquote("test", line.split_once('\t').unwrap().0))
            .collect();

        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted);
    }

    #[test]
    fn a_missing_file_loads_as_empty_so_a_new_corpus_can_be_created() {
        let dir = TempDir::new("missing-file");

        assert!(load_from(&dir.0.join("nope.txt"), "test").is_empty());
    }

    // ─── `decide`, the whole policy as a pure function ───────────────────────

    #[test]
    fn check_mode_returns_the_recording_or_reports_it_missing() {
        let mut entries = BTreeMap::from([("src".to_string(), "recorded".to_string())]);

        assert_eq!(
            decide(&mut entries, "src", "golden", false),
            Decision::Check("recorded".to_string())
        );

        assert_eq!(
            decide(&mut entries, "absent", "golden", false),
            Decision::Missing
        );

        // Check mode never writes.
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn update_mode_records_the_golden_and_never_the_fold() {
        let mut entries = BTreeMap::new();
        let recorded = BTreeMap::from([("src".to_string(), "golden".to_string())]);

        // The golden is what lands, and the snapshot handed back for writing is
        // exactly it — the fold is not a parameter of `decide` at all, which is
        // the asymmetry stated structurally rather than by convention.
        assert_eq!(
            decide(&mut entries, "src", "golden", true),
            Decision::Recorded(recorded.clone())
        );

        assert_eq!(entries, recorded);

        // Idempotent: recording the same pair again changes nothing.
        assert_eq!(
            decide(&mut entries, "src", "golden", true),
            Decision::Recorded(recorded.clone())
        );

        assert_eq!(entries, recorded);
    }

    #[test]
    fn update_mode_refuses_two_fixtures_sharing_a_source_but_not_a_rendering() {
        let mut entries = BTreeMap::new();

        let _ = decide(&mut entries, "src", "one", true);

        assert_eq!(
            decide(&mut entries, "src", "another", true),
            Decision::Conflict {
                existing: "one".to_string()
            }
        );

        // The conflicting value is refused, not merged.
        assert_eq!(entries.get("src").map(String::as_str), Some("one"));
    }

    // ─── the `Store` end to end, in both modes ───────────────────────────────

    #[test]
    fn an_update_run_writes_a_file_a_checking_run_then_reads() {
        let dir = TempDir::new("update-then-check");

        // Update mode records the *golden*, ignoring the fold entirely — the
        // asymmetry the whole harness rests on. Here the fold is deliberately
        // wrong, and is recorded nowhere.
        dir.store(true)
            .assert_recorded("corpus", "src", "golden", "a wrong fold");

        assert_eq!(
            std::fs::read_to_string(dir.0.join("corpus.txt")).unwrap(),
            "\"src\"\t\"golden\"\n"
        );

        // A checking run over the same store accepts a fold matching what was
        // recorded …
        dir.store(false)
            .assert_recorded("corpus", "src", "golden", "golden");
    }

    #[test]
    #[should_panic(expected = "the fold diverged from the recorded rendering")]
    fn a_checking_run_rejects_a_fold_that_differs_from_the_recording() {
        let dir = TempDir::new("wrong-fold");

        dir.store(true)
            .assert_recorded("corpus", "src", "golden", "golden");

        dir.store(false)
            .assert_recorded("corpus", "src", "golden", "a wrong fold");
    }

    #[test]
    #[should_panic(expected = "the string pipeline no longer produces the recorded rendering")]
    fn the_drift_guard_rejects_a_golden_that_no_longer_matches() {
        let dir = TempDir::new("drift");

        dir.store(true)
            .assert_recorded("corpus", "src", "golden", "golden");

        dir.store(false)
            .assert_recorded("corpus", "src", "a changed golden", "golden");
    }

    #[test]
    #[should_panic(expected = "no recording for")]
    fn a_fixture_with_no_recording_says_how_to_create_one() {
        let dir = TempDir::new("no-recording");

        dir.store(false)
            .assert_recorded("corpus", "src", "golden", "folded");
    }

    #[test]
    #[should_panic(expected = "conflicting recordings")]
    fn the_store_surfaces_a_conflict_as_a_panic() {
        let dir = TempDir::new("conflict");

        dir.store(true)
            .assert_recorded("corpus", "src", "one", "one");

        dir.store(true)
            .assert_recorded("corpus", "src", "another", "another");
    }

    #[test]
    fn matches_recording_reports_rather_than_asserting() {
        let dir = TempDir::new("matches");

        // Update mode compares against the golden being recorded, so a
        // regeneration run's own divergence set stays honest.
        assert!(
            dir.store(true)
                .matches_recording("corpus", "src", "golden", "golden")
        );
        assert!(!dir.store(true).matches_recording(
            "corpus",
            "other",
            "golden",
            "a divergent fold"
        ));

        // Checking mode compares against the recording, and reports rather
        // than panicking either way.
        assert!(
            dir.store(false)
                .matches_recording("corpus", "src", "golden", "golden")
        );
        assert!(
            !dir.store(false)
                .matches_recording("corpus", "src", "golden", "a divergent fold")
        );
    }

    #[test]
    fn recorded_golden_hands_back_the_recording_and_still_guards_drift() {
        let dir = TempDir::new("recorded-golden");

        // Update mode has nothing recorded yet, so the caller gets its own
        // golden back — and that is what lands in the file.
        assert_eq!(
            dir.store(true).recorded_golden("corpus", "src", "golden"),
            "golden"
        );

        assert_eq!(
            std::fs::read_to_string(dir.0.join("corpus.txt")).unwrap(),
            "\"src\"\t\"golden\"\n"
        );

        // A checking run hands back the *recording*, which is what makes a
        // caller's `assert_eq!(folded, golden_x(source))` compare against bytes
        // settled before the fold ran.
        assert_eq!(
            dir.store(false).recorded_golden("corpus", "src", "golden"),
            "golden"
        );
    }

    #[test]
    #[should_panic(expected = "the string pipeline no longer produces the recorded rendering")]
    fn recorded_golden_rejects_a_golden_that_no_longer_matches() {
        let dir = TempDir::new("recorded-golden-drift");

        let _ = dir.store(true).recorded_golden("corpus", "src", "golden");
        let _ = dir
            .store(false)
            .recorded_golden("corpus", "src", "a changed golden");
    }

    #[test]
    #[should_panic(expected = "no recording for")]
    fn recorded_golden_reports_a_missing_recording() {
        let dir = TempDir::new("recorded-golden-missing");

        let _ = dir.store(false).recorded_golden("corpus", "src", "golden");
    }

    #[test]
    fn a_panic_while_holding_the_lock_does_not_poison_it_for_later_corpora() {
        // The bug this pins: the harness's own `#[should_panic]` cases used to
        // panic *while holding* the recordings lock, so every later corpus in
        // the process failed with `PoisonError` instead of its real result —
        // one honest failure cascading into a wall of unrelated ones. Ordinary
        // `cargo test` ordering hid it; `cargo llvm-cov`'s did not.
        //
        // Two things fix it, and this covers the second: assertions now run
        // after the guard is dropped, *and* the lock recovers from poisoning if
        // something panics under it anyway.
        let poisoner = std::thread::spawn(|| {
            let _guard = super::lock();
            panic!("poisoning the recordings lock on purpose");
        });

        assert!(
            poisoner.join().is_err(),
            "the poisoning thread was supposed to panic"
        );

        // The lock is poisoned now. Every later caller must still get a usable
        // map rather than an error.
        let recovered = super::lock();
        drop(recovered);

        // And the harness still works end to end through it.
        let dir = TempDir::new("after-poisoning");

        dir.store(true)
            .assert_recorded("corpus", "src", "golden", "golden");

        dir.store(false)
            .assert_recorded("corpus", "src", "golden", "golden");
    }

    #[test]
    fn concurrent_update_runs_keep_every_fixture() {
        // The bug this pins: the write used to happen *after* the lock was
        // released, so a thread that had cloned an older, smaller map could
        // land last and overwrite everything recorded since. Measured with the
        // write outside the lock, thirty-two concurrent recordings left a file
        // holding **one** of them.
        //
        // It matters because the documented regeneration command is a plain
        // `cargo test`, which is multi-threaded — a maintainer regenerating the
        // corpus would have silently got a fraction of it.
        const FIXTURES: usize = 32;

        let dir = TempDir::new("concurrent-update");
        let path = dir.0.clone();

        std::thread::scope(|scope| {
            for i in 0..FIXTURES {
                let path = &path;

                scope.spawn(move || {
                    Store {
                        dir: path,
                        updating: true,
                    }
                    .assert_recorded(
                        "corpus",
                        &format!("src{i}"),
                        "golden",
                        "golden",
                    );
                });
            }
        });

        let recorded = load_from(&dir.0.join("corpus.txt"), "corpus");

        assert_eq!(recorded.len(), FIXTURES);

        for i in 0..FIXTURES {
            assert_eq!(
                recorded.get(&format!("src{i}")).map(String::as_str),
                Some("golden"),
                "src{i} was lost from the recording"
            );
        }
    }

    #[test]
    #[should_panic(expected = "cannot read")]
    fn a_read_failure_that_is_not_a_missing_file_is_loud() {
        // A missing file reads as empty so a new corpus can be created. Any
        // other failure must not: treating it as "no fixtures recorded" would
        // have an update run rewrite the file and shrink the corpus to whatever
        // that invocation reached. A directory standing where the file belongs
        // is the portable way to provoke a non-`NotFound` error.
        let dir = TempDir::new("unreadable");
        std::fs::create_dir_all(dir.0.join("corpus.txt")).unwrap();

        let _ = load_from(&dir.0.join("corpus.txt"), "corpus");
    }

    #[test]
    fn the_real_store_points_at_the_checked_in_recordings() {
        let store = Store::real();

        assert!(store.path("whole_pipeline").is_file());
        assert!(!store.updating, "a plain `cargo test` run must not record");
    }

    // ─── malformed recordings ────────────────────────────────────────────────

    #[test]
    #[should_panic(expected = "malformed recording line")]
    fn a_line_without_a_tab_is_rejected() {
        let dir = TempDir::new("malformed");
        let file = dir.0.join("corpus.txt");
        std::fs::write(&file, "no tab here\n").unwrap();

        let _ = load_from(&file, "test");
    }

    #[test]
    #[should_panic(expected = "unquoted field")]
    fn an_unquoted_field_is_rejected() {
        let dir = TempDir::new("unquoted");
        let file = dir.0.join("corpus.txt");
        std::fs::write(&file, "bare\t\"quoted\"\n").unwrap();

        let _ = load_from(&file, "test");
    }

    #[test]
    #[should_panic(expected = "unsupported escape")]
    fn an_escape_the_format_never_emits_is_rejected() {
        unquote("test", r#""a\qb""#);
    }

    #[test]
    #[should_panic(expected = "bad \\u escape")]
    fn a_malformed_unicode_escape_is_rejected() {
        unquote("test", r#""a\u{zz}b""#);
    }

    #[test]
    #[should_panic(expected = "bad code point")]
    fn an_out_of_range_code_point_is_rejected() {
        unquote("test", r#""a\u{110000}b""#);
    }
}
