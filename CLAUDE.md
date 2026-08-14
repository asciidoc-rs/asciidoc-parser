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
5. **Code coverage** — check coverage for the lines/functions/branches actually touched by the change,
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

## Inline AST branch

This repo has a long-running `inline-ast` feature branch (see `docs/design/inline-ast-architecture.md`)
implementing a structured inline AST alongside the existing string-substitution pipeline. Work on that
branch lands in small, additive increments, each gated by a byte-for-byte differential corpus against the
existing string pipeline's output (`golden_*` test helpers). When picking up the next step:

- Read `docs/design/inline-ast-architecture.md` §5.2 ("Phased plan") for the current state and the
  itemized checklist of landed/remaining sub-steps.
- Check merged PRs against the `inline-ast` branch (and any still-open ones) to confirm what's actually
  landed vs. what the design doc's checklist shows — the local `inline-ast` branch ref can be stale;
  fetch `origin/inline-ast` first.
- After landing a step, update the design doc's narrative (a new "*Step N landed as (...)*" paragraph)
  and its checklist entry, matching the existing entries' style — this is how every prior increment on
  this branch has recorded its landing.
