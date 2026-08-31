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
   subject on `inline-ast`. A commit message that used another type is therefore cosmetic, and not
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

## Inline AST branch

This repo has a long-running `inline-ast` feature branch, not yet merged into `main`, that replaced
the crate's original string-substitution inline-content pipeline with a structured inline AST (a tree
of `InlineNode`s — see `parser/src/content/inline_builder/README.md` and
`docs/design/inline-ast-architecture.md`). **That migration is done**, not in progress: every parse
builds the tree unconditionally, `Content::rendered_html()` is a fold over it, every macro family's
catalog/warning registration is replayed from it, and the original string-substitution pipeline and
its three sentinel-character encodings (for passthroughs, deferred cross-references, and footnote
markers) are fully retired from production code — see `parser/snapshots/README.md` for that history.
`golden_*` test helpers compare against **frozen recordings** of the old pipeline's output
(`parser/snapshots/*.txt`), not a live second implementation.

There is no more step-by-step migration checklist to work through. `docs/design/inline-ast-architecture.md`
used to be a ~12,000-line proposal-plus-build-log tracking that migration phase by phase; once the
migration it tracked was done, it was rewritten down to a normal, ~400-line architecture reference (data
model, source-location policy, rendering model, the ASG-conformance decision) with no phase/step tracking
left in it and no "next phase" to read off it. Do not look for a §5.2 "Phased plan," a "Step N landed as"
note, or an exit-gate checklist — none of that exists in the doc any more, and CLAUDE.md previously
pointed at it; that guidance was itself part of the migration-tracking apparatus and has been removed
along with the doc structure it depended on.

What typically remains on this branch, until it lands in `main`, is narrower and case-by-case: polish and
documentation work (this section's own rewrite is an example), small mechanism refinements, and whatever
the branch's own pre-landing validation against the Ruby-to-Rust `asciidoctor` port still requires. There
is no standing document that enumerates it. To find out what's actually landed and what's in flight:

- `git log --oneline origin/main..origin/inline-ast` is the authoritative record of everything the branch
  carries that `main` doesn't — fetch first, since a stale local `inline-ast` ref under-reports it.
- Open PRs against `inline-ast` (not `main`) are the record of what's in flight right now.
- If the user's request doesn't name a specific next step, ask rather than inventing a step-tracking
  scheme to justify one — there isn't a checklist to consult, and guessing at "the next increment" is
  how the removed apparatus accumulated in the first place.

When picking up work here:

- **Branch from `origin/inline-ast`, not `main`.** This file, the design doc, and the whole
  `inline_builder` module exist only on that branch, so a work branch cut from `main` starts with none
  of them and every "read the design doc" instruction dead-ends:

  ```
  git fetch origin inline-ast
  git checkout -b <work-branch> origin/inline-ast
  ```

  Use `-b`, not `-B`, so a name collision fails loudly instead of silently resetting a branch that
  had commits on it. The one exception is a work branch the session harness **already created** — it
  cuts it from `main`, which is the case this bullet exists for — where re-pointing is exactly what
  you want and the only thing discarded is the `main` tip it was mistakenly cut from:

  ```
  git checkout -B <work-branch> origin/inline-ast   # only to re-point a harness-created branch
  ```

- **One change per branch/PR.** Resist bundling unrelated cleanups into one PR.
- **Open the PR as a draft**, not ready-to-review — the maintainer flips it when they pick it up.

### Coverage on this branch

Judge coverage on a **diff** basis, not an absolute one: `cargo llvm-cov report -p asciidoc-parser
--show-missing-lines` on your branch and on `origin/inline-ast`, then check that the changed file's
missed-region and missed-line **counts** are unchanged. Get the baseline by checking out the base
commit, not with `git stash` — a stash taken after you have committed is empty, and the "baseline"
run then measures your own branch. (`cargo llvm-cov report` also reuses the last run's profile data,
so re-run `cargo llvm-cov` itself after switching, not just `report`.) The files this branch
touches already sit at ~99% with a handful of long-documented defensive branches, so the absolute
number tells you nothing about what you added.

### Three local papercuts

- `cargo-llvm-cov` and the nightly `rustfmt` component are often absent from a fresh session; install
  both before the first preflight (`cargo install cargo-llvm-cov --locked`,
  `rustup component add --toolchain nightly rustfmt`) rather than mid-checklist.
- Test fixtures in this module are full of backticks and quotes. An `r#"…"#` literal whose *contents*
  contain a `"` immediately followed by a `#` — which a smart-quote-then-mark fixture does — is
  terminated by that pair, producing a wall of unrelated syntax errors far from the real line. Use
  `r##"…"##` for those.
- **Never remove a throwaway probe with `git checkout -- <file>`.** The instrumentation for an audit
  or a measurement usually lands in a file you are also editing for real, and that command discards
  the file's *whole* working state, not just the probe — it destroyed uncommitted work three times in
  one session. Take the probe out with the same targeted edit that put it in, or commit a checkpoint
  first and squash it away at the end.
