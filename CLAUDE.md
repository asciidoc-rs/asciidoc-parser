# Project notes for Claude

## Preflight checklist (before considering a change done / opening or updating a PR)

Run all of the following for any change to `parser/` (or other Rust crates in this workspace):

1. `cargo fmt --check` (or `cargo fmt` then re-check) — the repo uses nightly-only rustfmt options
   (see `rustfmt.toml`), so warnings about unstable features on `cargo fmt` are expected noise, not
   failures.
2. `cargo clippy -p asciidoc-parser --all-targets` — must be clean. Note `#![deny(clippy::indexing_slicing)]`
   and similar crate-level lints in `parser/src/lib.rs`; prefer `.get()`/checked arithmetic over direct
   indexing outside `#[cfg(test)]` code.
3. `cargo test --workspace` — must be green.
4. **Code coverage** — check coverage for the lines/functions/branches actually touched by the change,
   not just that tests pass. This repo tracks coverage via `cargo-llvm-cov` in CI (see
   `.github/workflows/ci.yml`) and reports diff coverage on PRs via Codecov (`codecov.yml`). Locally:

   ```
   cargo llvm-cov -p asciidoc-parser --lib
   cargo llvm-cov report -p asciidoc-parser --show-missing-lines
   ```

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
