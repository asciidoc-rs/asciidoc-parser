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

This repo has a long-running `inline-ast` feature branch (see `docs/design/inline-ast-architecture.md`)
implementing a structured inline AST alongside the existing string-substitution pipeline. Work on that
branch lands in small, additive increments, each gated by a byte-for-byte differential corpus against the
existing string pipeline's output (`golden_*` test helpers). When picking up the next step:

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

- **Don't read the whole design doc — it is ~4,500 lines.** Read §5.2 ("Phased plan") for the phase
  status, then the **last two** "*Step N (prep) landed as (…)*" paragraphs and the **tail** of the
  Phase 4 checklist. The closing paragraph of the most recent increment always names what is left
  ("what still defers is …"), which is the menu the next increment is picked from.
- `git log --oneline origin/main..origin/inline-ast` is the fastest authoritative record of what has
  actually landed — the local `inline-ast` ref can be stale, so fetch first. Every increment's commit
  subject carries its own step number, so the log doubles as the real checklist when the doc's own
  lags. Listing merged PRs against the branch tells you the same thing more slowly; do that only when
  you need to check for a still-open one.
- **One increment per branch/PR.** Every prior increment is a single narrow behavior change plus its
  corpora and its design-doc note. Resist bundling two.
- **Open the PR as a draft**, not ready-to-review — the maintainer flips it when they pick it up.
- After landing a step, update the design doc's narrative (a new "*Step N landed as (...)*" paragraph)
  and its checklist entry, matching the existing entries' style — this is how every prior increment on
  this branch has recorded its landing.
- **Close with a progress estimate.** In your final summary to the user, say how many further sessions
  the *whole* inline-AST branch looks like — not just the step you landed. See below for how to ground
  it.

### Estimating what is left

The branch has been running long enough that "how much further?" is a standing question, and a session
that has just surveyed §5.2 is the cheapest place to answer it. Ground the estimate in four things
rather than guessing:

1. **The unchecked items in the Phase 4 checklist**, plus the phase *exit gates*, Phase 5, and
   Landing. Phase 4's step list is now fully ticked — step 7 is closed: `render_with` and the
   `attribute-missing` retirement landed, and both `Document::render_to` and
   `Document::to_asg()` are recorded as *not being built* (the latter by §6's decision 7 — the
   ASG schema is a parked 2023 draft that cannot express the crate's inline vocabulary; see
   §3.5). **A ticked step list is not a met exit gate**, though, and the two are easy to
   conflate: Phases 2, 3 and 4 are all still marked 🔶 *In progress*, and at least one gate
   item is demonstrably outstanding — Phase 3's *Exit:* requires the README's security section
   to gain its `Raw`-node anchor, and `README.md` does not mention `Raw` at all. Read each
   phase's own *Exit:* line; do not infer a phase is done from its step ticks.
2. **The "what still defers" sentence** in the newest landed-as note — those are the increments
   already named and sized.
3. **The observed rate.** Every increment so far is one branch, one PR, one session, so an increment
   count *is* a session count. `git log --oneline origin/main..origin/inline-ast` gives you the run
   rate directly.
4. **The two things that move the number most:** the step 6 cutover itself is bundled work (the
   authoritative fold, wiring each staged side effect for real, deleting three sentinel systems,
   retiring the `with_inline_tree` flag) and is worth several sessions on its own; and the corpus-wide
   audit has repeatedly *discovered* new preps mid-flight, so the remaining-prep count is a floor, not
   a ceiling.

Give a **range** with the reasoning attached, and say plainly which parts are firm (an enumerated
checklist item) and which are open-ended (anything gated on an audit that has not run yet). A single
confident number would be false precision.

### The corpus-wide fold-parity audit

Several increments were found (or cleared) by an audit the design doc refers to as "tree building
forced on for every parse in the suite". It is not checked in — rebuild it as a throwaway patch, run
it on your branch **and** on `origin/inline-ast`, and compare the two sets. The bar every increment
has to clear is *no **new** divergence*; a set that also shrinks is a bonus, not a requirement (an
increment closing a form no golden source exercises leaves the set unchanged).

In `parser/src/content/substitution_group.rs`, inside `SubstitutionGroup::apply`:

1. Force the seed on: `let tree_seed = if parser.build_inline_tree {` → `let tree_seed = if true {`.
2. Just before `content.set_inlines(tree)`, fold the tree with `HtmlInlineRenderer` and, when
   the result differs from `content.rendered`, append `src` / `rendered` / `folded` to a log file —
   one `writeln!` per divergence, formatting all three with `{:?}` (see the third gotcha below).

Then:

```
cargo test --workspace -- --test-threads=1     # see the gotchas below
sort -u <logfile> > after.txt                  # repeat on origin/inline-ast for before.txt
comm -13 before.txt after.txt                  # must be empty: these are NEW divergences
comm -23 before.txt after.txt                  # divergences this increment closed
```

Four gotchas that will silently waste a run — each produces a plausible-looking but wrong answer
rather than an error:

- **Log to a file, not `eprintln!`.** `cargo test` captures a passing test's output, so `eprintln!`
  divergences vanish unless every run also passes `--nocapture` (which then interleaves them with the
  harness's own progress lines).
- **`--test-threads=1`.** Tests append to the log concurrently otherwise, and the interleaved lines
  make the two sets impossible to `comm`.
- **Format the three values with `{:?}`, not `{}`.** `sort -u` and `comm` are line-based, so the
  whole comparison rests on one record being one physical line. Debug-escaping is what guarantees
  that: a multi-line block's content comes out as `\n` rather than as real newlines. Under `{}` such
  a record spills across lines that then get deduplicated against unrelated records' lines, and a
  genuinely new divergence can drop out of `comm -13` — the one output the audit exists to read.
- **Revert the patch before *any* `git stash`, and get the baseline from a detached checkout of the
  base rather than from a stash.** Both halves of this have bitten. Applying the patch to a tree that
  already holds your real changes and then stashing sweeps *both* away together, and the later `stash
  pop` brings *both* back — which is how the audit patch reached a commit and turned every CI job red.
  And once your work is committed, `git stash` has nothing to stash, so the "before" run silently
  measures your own branch and the two sets match for the wrong reason. `git checkout <base-sha>`,
  patch, run, **take the probe back out with the same targeted edit that put it in**, then
  `git checkout <branch>` has neither failure mode. Take it out that way on the base too, not with a
  whole-file discard: a discard happens to be harmless *there* — the detached base holds nothing but
  the probe — but the recipe is the thing that gets copied, and the fourth papercut below is what
  happens when it is copied onto a branch where the file also holds your work.

Revert the patch before committing, and verify it by grepping for what should be **absent**
(`scratchpad`, `OpenOptions`, the log path) rather than for the changes you meant to keep. Confirming
that your own edits survived says nothing about whether the probe went with them.

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
