# `snapshots/`

Frozen, checked-in golden recordings for the `inline-ast` branch's differential test
corpora. Each file is a **recording**, not a fixture list: it holds output that was once
produced by the old string-substitution pipeline, captured before that pipeline was
deleted (as part of [#1059](https://github.com/asciidoc-rs/asciidoc-parser/pull/1059)).
Tests now read these files as a fixed oracle instead of re-deriving the expected output 
at test time.

## Format

One `<corpus>.txt` file per corpus. Each line is one fixture:

```
"<source>"\t"<rendered>"
```

- Tab-separated, one record per physical line, sorted by `<source>`.
- Both fields are `{:?}` (Rust `Debug`) escaped — this is what guarantees a record is
  exactly one line even when the source or output contains newlines, tabs, or the
  Unicode Private-Use-Area sentinel characters the old pipeline used internally.
- A blank/empty string is still a valid field: `""`.

## Why these are frozen rather than generated

Every corpus here started life as a genuine differential test: a fixture was rendered two
ways — through the old string-substitution pipeline and through the new inline-AST
builder — and the test asserted the two matched. Once the AST's HTML fold became the
*only* pipeline (the string pipeline was deleted), that comparison would have become
tautological (the fold compared against itself). So each corpus's expected side was
recorded once, checked in, and is now read rather than re-derived — exactly like the
crate's ~277 golden-HTML string assertions elsewhere in the test suite.

**A recording is edited by hand, and reviewed like the behavior change it records.**
Adding or changing a line is asserting "this rendering is correct" — treat it as an
expected-output literal in any other golden test, not as generated data.

Most corpora record **known-good HTML output** for a fixture. A few instead record a
**documented divergence** — behavior the old string pipeline had that the tree
deliberately does not reproduce (`xref_passthrough_divergence.txt`), or a frozen
non-HTML side effect such as catalog registrations and warnings
(`side_effects.txt`, `passthrough_records.txt`). Those must never be "refreshed" from
current behavior — their entire point is to preserve what the retired pipeline used to
do, so the diff between it and the fold stays visible and reviewed.

## How it's used

The loader lives in
[`parser/src/content/inline_builder/snapshot.rs`](../src/content/inline_builder/snapshot.rs).
It parses a corpus file into a `source -> rendered` map (cached per file for the test
process) and exposes:

- `recorded(corpus, source)` — the recorded rendering for `source`, panicking with a
  ready-to-paste line if the fixture isn't recorded yet.
- `assert_recorded(corpus, source, folded)` — asserts a freshly-computed fold matches the
  recording.
- `matches_recording(corpus, source, folded)` — same comparison, returned as a `bool`
  instead of asserted, for tests that track a fixed *set* of known divergences (see
  `cross_product.txt`'s use in
  [`inline_builder/mod.rs`](../src/content/inline_builder/mod.rs)).

Individual `inline_builder` submodules (e.g. `attribute_refs.rs`, `callouts.rs`,
`char_replacements.rs`, `macros/mod.rs`) wrap `recorded()` in their own small
`golden_*(source)` helpers, so most call sites just read `golden_foo(source)` and never
touch `snapshot.rs` directly. Two whole-corpus differential harnesses live outside the
module, in `parser/src/tests/`: `inline_builder_side_effect_parity.rs` and
`inline_builder_passthrough_record_parity.rs`.

## Adding or changing a fixture

Run the test. A fixture with no recording fails with a message naming the corpus file and
the exact `"<source>"\t"<rendered>"` line to add. Paste that line into the named
`<corpus>.txt`, keeping the file sorted by source, and review it the same way you'd review
any other expected-output string in a test — it is now the specification for that input.

## What the corpora cover

| File | Covers |
| --- | --- |
| `anchors_attributes.txt` | Inline anchors reached through attribute-reference expansion |
| `attribute_refs.txt` | Attribute reference (`{name}`) recognition, including counters |
| `attribute_refs_missing_drop.txt` / `_drop-line.txt` | `attribute-missing` set to `drop` / `drop-line` |
| `build_for_group.txt` | The five-step builder run over custom `subs=` groups |
| `build_for_group_escaped_reference.txt` | An escaped `\{name}` surviving a custom group |
| `build_for_group_recoverable_piece.txt` | A recognized construct recovered from an opaque match-string piece |
| `build_for_group_restored_entity.txt` | A restored numeric/named entity surviving a custom group |
| `build_from_value.txt` | The whole-pipeline builder seeded from a `(value, location)` pair (multi-line/filtered content) |
| `callouts.txt`, `callouts_icons_font.txt`, `callouts_icons_image.txt` | Callout marker recognition, and its `icons=font`/`icons=image` renderings |
| `char_replacements.txt` | Character/typographic replacements (`(C)`, `--`, smart quotes, arrows, …) |
| `cross_product.txt` | A construct × container sweep, read via `matches_recording` to track a fixed set of known container/construct divergences |
| `footnotes_build_from_value.txt`, `footnotes_expanded_attribute.txt`, `footnotes_normal.txt` | Footnote macro recognition and numbering |
| `group_order_escaping.txt`, `group_order_verbatim.txt` | How substitution order affects escaping of an already-escaped/verbatim value |
| `image_normal.txt` | `image:`/`icon:` macro recognition |
| `indexterm_expanded.txt` | Index terms (`((…))`, `(((…)))`, `indexterm2:`) with attribute expansion inside |
| `links_normal.txt` | `link:`, bare URL, and autolink recognition |
| `macros.txt` | The general macro family sweep (the largest corpus) |
| `macros_experimental.txt` | UI macros (`kbd:`, `btn:`, `menu:`) under the `experimental` attribute |
| `macros_hide_uri_scheme.txt` | Autolinks under `hide-uri-scheme` |
| `macros_imagesdir.txt` | `image:`/`icon:` resolution against a document `imagesdir` |
| `passthrough_records.txt` | Frozen [`Content::passthroughs`] extraction facts (not rendered HTML) — see `inline_builder_passthrough_record_parity.rs` |
| `passthroughs.txt`, `passthroughs_hide_uri_scheme.txt` | Passthrough (`+++…+++`, `pass:[…]`, `$$…$$`, `` `+…+` ``) recognition |
| `post_replacements.txt`, `post_replacements_hardbreaks_block.txt`, `post_replacements_hardbreaks_document.txt` | Hard line-break (`+` at end of line) handling, plain and under `hardbreaks-option` |
| `quotes.txt` | Quoted-text formatting (`*bold*`, `_em_`, `` `code` ``, `#mark#`, super/subscript, smart quotes) |
| `side_effects.txt` | Frozen non-HTML side effects (catalog registrations, warnings) — see `inline_builder_side_effect_parity.rs` |
| `special_chars.txt` | Bare `<`, `>`, `&` escaping |
| `ui_normal.txt` | `kbd:`/`btn:`/`menu:` macros with attribute expansion inside |
| `whole_pipeline.txt` | A broad general sweep through the full normal-order pipeline |
| `xref_macros.txt`, `xref_normal.txt`, `xref_whole_pipeline.txt` | Cross-reference (`<<...>>`, `xref:`) recognition and resolution |
| `xref_passthrough_divergence.txt` | A **documented divergence**: a passthrough body inside an xref target, where the tree deliberately does not match the old pipeline's output |

For the deeper "why" — the sentinel systems these corpora replaced, the step-6 cutover
that froze them, and the branch's overall architecture — see
[`docs/design/inline-ast-architecture.md`](../../docs/design/inline-ast-architecture.md).
