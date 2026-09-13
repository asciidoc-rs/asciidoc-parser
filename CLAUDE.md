# Project notes for Claude

## Preflight checklist (before considering a change done / opening or updating a PR)

Run all of the following for any change to `parser/` (or other Rust crates in this workspace):

1. `cargo +nightly fmt --all -- --check` (or `cargo +nightly fmt --all` then re-check) — this is the
   exact command CI's "Enforce Rust code format" job runs, and **nightly is required, not optional**:
   `rustfmt.toml` sets nightly-only options (`wrap_comments`, `format_code_in_doc_comments`,
   `imports_granularity`, …) that *stable* rustfmt silently ignores after printing a "can't set …,
   unstable features are only available in nightly channel" warning for each. So plain
   `cargo fmt --check` passes on code CI rejects — most often over-wide doc comments, since
   `wrap_comments` is off on stable. Those per-option warnings are expected noise on either channel;
   the exit status is what matters. Install with `rustup toolchain install nightly --component rustfmt`.
2. `cargo clippy -p asciidoc-parser --all-targets` — must be clean. Note `#![deny(clippy::indexing_slicing)]`
   and similar crate-level lints in `parser/src/lib.rs`; prefer `.get()`/checked arithmetic over direct
   indexing outside `#[cfg(test)]` code.
3. `cargo test --workspace` — must be green.
4. `cargo +nightly doc --no-deps --document-private-items` — must be clean; CI's "Verify internal crate
   documentation" job runs exactly this (nightly because the crate uses `doc_cfg`). `parser/src/lib.rs`
   has `#![deny(warnings)]`, which implies `#![deny(rustdoc::broken_intra_doc_links)]`, so a doc comment
   is a *compile-time* surface: an intra-doc link that does not resolve **fails the build**. None of
   the other steps here catch it — `clippy`, `test`, and `fmt` never run rustdoc — so this is its own
   preflight step, not something the rest of the checklist covers. The usual trap is linking to an item
   that is private to another module (e.g. ``[`InlineLinkReplacer`]`` from `content::inline_builder`,
   which lives in `content::macros`) —
   either add an explicit link-reference definition (``/// [`Foo`]: crate::path::to::module``, as
   sibling code in `parser/src/content/inline_builder/macros/links.rs` does) or drop the brackets and
   leave it as plain `code` text.
5. **The PR title** must be a Conventional Commit whose *type* is one of the five
   `.commitlintrc.no-scope.yml` allows: `fix`, `feat`, `chore`, `update`, `doc`. **`refactor` is not
   among them** and CI's "Conventional commits validation" job rejects it — a pure deletion or a
   mechanism retirement is `chore`. The description must also start with a capital letter or digit
   and must not end with a period (both errors); over 70 characters is a warning, not a failure.
   Only the *title* is checked — `.github/workflows/pr_title.yml` triggers on `edited`, so correcting
   it re-runs the job on its own — and since the repo squash-merges, that title becomes the commit
   subject on `main`. A commit message that used another type is therefore cosmetic, and not
   worth a force-push.
6. **Code coverage** — check coverage for the lines/functions/branches actually touched by the change,
   not just that tests pass. This repo tracks coverage via `cargo-llvm-cov` in CI (see
   `.github/workflows/ci.yml`) and reports diff coverage on PRs via Codecov (`codecov.yml`). Locally
   (install it first if needed: `cargo install cargo-llvm-cov --locked`):

   ```
   cargo llvm-cov -p asciidoc-parser --lib
   cargo llvm-cov report -p asciidoc-parser --show-missing-lines
   ```

   `--show-missing-lines` reports lines but not *regions*; when the summary shows more missed regions
   than missed lines, render the annotated source to see which sub-expressions are uncovered:

   ```
   cargo llvm-cov report -p asciidoc-parser --text --output-dir <dir>
   ```

   then read the file's `.txt` under `<dir>/text/coverage/…` and look for the `^0` carets.

   For each newly-uncovered line/branch in a changed file, decide deliberately rather than skipping it:
   - If it's reachable in practice, add a test that exercises it (a differential-corpus fixture, a
     direct unit test of the specific branch, etc.).
   - If it's genuinely unreachable (e.g. a regex capture group that always participates, so an
     `Option::unwrap()` can never panic), prefer removing the dead defensive branch over leaving it
     untested — see how sibling code in `parser/src/content/inline_builder/` handles this.
   - `other => panic!(...)` fallback arms inside test assertions (only reachable when the test itself
     would fail) are expected/idiomatic in this codebase's test style and are not gaps to chase.

   Judge coverage on a **diff** basis, not an absolute one: run `cargo llvm-cov report -p
   asciidoc-parser --show-missing-lines` on your branch and again on the base branch (`origin/main`),
   then check that a changed file's missed-region and missed-line **counts** are unchanged apart from
   what your change accounts for. Get the baseline by checking out the base commit, not with
   `git stash` — a stash taken after you have committed is empty, and the "baseline" run then measures
   your own branch. (`cargo llvm-cov report` also reuses the last run's profile data, so re-run
   `cargo llvm-cov` itself after switching, not just `report`.) Files under `parser/src/content/
   inline_builder/` already sit at ~99% with a handful of long-documented defensive branches, so the
   absolute number there tells you nothing about what you added.

## Local environment notes

- `cargo-llvm-cov` and the nightly `rustfmt` component are often absent from a fresh session; install
  both before the first preflight (`cargo install cargo-llvm-cov --locked`,
  `rustup component add --toolchain nightly rustfmt`) rather than mid-checklist.
- Test fixtures under `parser/src/content/inline_builder/` are full of backticks and quotes. An
  `r#"…"#` literal whose *contents* contain a `"` immediately followed by a `#` — which a
  smart-quote-then-mark fixture does — is terminated by that pair, producing a wall of unrelated
  syntax errors far from the real line. Use `r##"…"##` for those.
- **Never remove a throwaway probe with `git checkout -- <file>`.** The instrumentation for an audit
  or a measurement usually lands in a file you are also editing for real, and that command discards
  the file's *whole* working state, not just the probe. Take the probe out with the same targeted edit
  that put it in, or commit a checkpoint first and squash it away at the end.
