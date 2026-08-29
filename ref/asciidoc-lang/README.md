# AsciiDoc Language reference snapshot

This folder contains a **snapshot** of material imported from the
[AsciiDoc Language project](https://gitlab.eclipse.org/eclipse/asciidoc-lang/asciidoc-lang)
maintained by the Eclipse Foundation. It is included so that `asciidoc-parser`
can be developed against — and measure its coverage of — the official AsciiDoc
language description.

These files are **not** built, run, or otherwise compiled by the crates in this
repository. They are a static, pinned copy kept purely for reference and for the
spec-coverage tooling in [`../../sdd`](../../sdd). (The `track_file!` markers in
`parser/src/tests` refer to the `.adoc` pages under `docs/modules` below.)

## Source

| | |
| --- | --- |
| Project | AsciiDoc Language (Eclipse Foundation) |
| Upstream repository | <https://gitlab.eclipse.org/eclipse/asciidoc-lang/asciidoc-lang> |
| Pinned commit | `d335f56572b656a7c9f84a5e0c76ea6f41f281e1` |
| Commit date | 2026-06-15 |
| Commit summary | _remove unused partials and add glossary to document attributes page_ |

The pinned commit is the exact `asciidoc-lang/main` revision that was brought in
by the most recent update from upstream (asciidoc-parser
[PR #507](https://github.com/asciidoc-rs/asciidoc-parser/pull/507),
`update-from-asciidoc-lang-2026-06`).

## What is included

Two upstream folders are snapshotted here, both at the pinned commit above:

- [`docs/`](./docs) (i.e. `ref/asciidoc-lang/docs`) — the Antora documentation
  site that constitutes the AsciiDoc language description.
- [`asg/`](./asg) (i.e. `ref/asciidoc-lang/asg`) — the JSON Schema for AsciiDoc's
  Abstract Semantic Graph (ASG), which is the structure the AsciiDoc TCK validates
  an implementation's output against. The schema itself is
  [`asg/schema.json`](./asg/schema.json); the `package.json`, `bin/`, `lib/`, and
  `test/` entries alongside it are upstream's own Ajv-based validation harness,
  copied so the snapshot matches upstream exactly — nothing here runs `npm`. For
  what the schema says and what it means for this crate, see
  [`docs/design/inline-ast-architecture.md`](../../docs/design/inline-ast-architecture.md)
  §3.5.

The upstream `spec/` folder is **not** included at this time; we may revisit
importing it later.

## How to update this snapshot

1. Fetch the desired revision from the upstream repository:
   `git fetch https://gitlab.eclipse.org/eclipse/asciidoc-lang/asciidoc-lang main`
2. Replace the contents of [`docs/`](./docs) and [`asg/`](./asg) with the upstream
   `docs/` and `asg/` folders at that revision, so that the whole snapshot stays
   at a single revision.
3. Update the **Pinned commit**, **Commit date**, and **Commit summary** rows in
   the table above to the new upstream commit.
4. Review and adjust the `track_file!("ref/asciidoc-lang/docs/...")` markers in
   `parser/src/tests` for any pages that were added, removed, or renamed, then
   regenerate spec coverage with `cd sdd && cargo run`.

## License

The user documentation in [`docs/`](./docs) is made available under the terms of
a [Creative Commons Attribution 4.0 International License](https://creativecommons.org/licenses/by/4.0/)
(CC-BY-4.0); see [`docs/LICENSE`](./docs/LICENSE).

The ASG schema and harness in [`asg/`](./asg) are covered by the project-wide
Eclipse Public License v 2.0 (EPL-2.0). Upstream, `asg/` carries neither a
`LICENSE` file of its own nor any per-file license header, and the CC-BY-4.0
grant does not extend to it: [`docs/LICENSE`](./docs/LICENSE) scopes itself to
content "under this directory", and upstream's `NOTICE.adoc` likewise confines
CC-BY-4.0 to "the end user documentation content" while declaring EPL-2.0
(`SPDX-License-Identifier: EPL-2.0`) for the technology specification and its
accompanying materials. Upstream's root `README.adoc` describes the same split.

The AsciiDoc Language project as a whole is made available under the terms of the
Eclipse Public License v 2.0 (EPL-2.0); see the
[project LICENSE](https://gitlab.eclipse.org/eclipse/asciidoc-lang/asciidoc-lang/-/blob/main/LICENSE)
for the full text.

These license terms apply to the contents of this `ref/asciidoc-lang` folder
only, and are separate from the MIT OR Apache-2.0 terms that cover the rest of
this repository.
