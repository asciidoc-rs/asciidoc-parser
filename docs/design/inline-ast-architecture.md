# Inline AST architecture

**Scope:** the crate's inline content model — a first-class inline AST (`InlineNode`), with
rendering as a fold over it.

---

## 1. Overview

Inline content (bold, links, images, footnotes, cross references, …) is modeled as a **tree**
of `InlineNode`s ([`parser/src/inlines/`](../../parser/src/inlines/mod.rs)), built directly
from source in a single forward pass
([`parser/src/content/inline_builder/`](../../parser/src/content/inline_builder/README.md)).
Recognizing a construct and rendering it are two separate steps: recognition produces nodes,
and rendering is a **fold** over the finished tree through an
[`InlineRenderer`](../../parser/src/parser/inline_renderer.rs) — the built-in
`HtmlInlineRenderer` is one such fold; any other implementation of that trait is another.

This is the direction the [Eclipse AsciiDoc Language project's Abstract Semantic Graph
(ASG)](https://gitlab.eclipse.org/eclipse/asciidoc-lang/asciidoc-lang/-/blob/d335f56572b656a7c9f84a5e0c76ea6f41f281e1/asg/schema.json)
also takes: inline content is structured data, and text substitution is a renderer's concern,
not the model's. This crate's node vocabulary is shaped after the ASG's — `span`/`ref`/
`literal`, with `variant` and `form` — extended to cover constructs the ASG does not model
(images, footnotes, UI macros, index terms, callouts, anchors, line breaks, STEM). See §8 for
why the crate does not emit conformant ASG output.

The crate previously modeled inline content as **string rewriting**: `Content` held one
mutable rendered string that each substitution step edited in place, with two construct
families — passthroughs, and deferred cross-references/footnote markers — smuggled through it
as Unicode sentinel characters, since neither can be represented in a flat string. That model,
its sentinel encodings, and the two-pass "record markers into a rendered string, then recover a
tree from them" strategy considered early on are retired from production code; see
[`parser/snapshots/README.md`](../../parser/snapshots/README.md) for that history where it's
relevant.

This direction resolves several long-standing limitations: inline content used to be exposed
only as opaque, pre-rendered HTML, with no read-only structural view and no per-node source
positions available to a caller. See §10 for the full mapping to the issues that tracked those
gaps.

---

## 2. Goals and non-goals

### Goals

1. A **public, read-only inline AST** exposed per content block, aligned with the Eclipse ASG
   core (span / ref / literal, with `variant` and `form`) and extended to cover the inline
   constructs this crate supports.
2. **Rendering is a fold over the AST.** `InlineRenderer` is an AST walker; HTML output is one
   projection, alternate backends are others.
3. **Byte-for-byte HTML parity** with the crate's historical string-based output, guarded by
   the golden-HTML oracle (§7).
4. **Per-node source locations**, populated as precisely as each construct allows (§3.4).
5. **`'src` borrowing** for untransformed text runs, so the common case does not allocate.

### Non-goals

- **Extensions** (custom inline macros) are out of scope, per the README.
- **A new output backend** (Markdown, DocBook, …) is not part of this crate's own scope, though
  the fold-over-AST design is what makes such backends tractable for a downstream consumer.
- **Reimplementing Asciidoctor's inline grammar.** Structure is derived from the same
  regex-detection events the crate has always used to recognize constructs; the *sink* is nodes
  rather than a rendered string, not the *recognition*. This is what preserves fidelity with
  Asciidoctor's own output.
- **Round-tripping AST → source.** The AST is a semantic graph, not a lossless CST.
- **Emitting conformant ASG.** See §8.

---

## 3. The data model

### 3.1 Design principles

- **ASG shapes, crate superset.** The four ASG shapes (span, ref, text/charref/raw) are the
  spine — because they are a good model, not because the crate serializes to them. Everything
  beyond them is an additional variant that renders richly in HTML. The crate keeps the ASG's
  *vocabulary* (`variant`, `form`, the literal trichotomy) so a conformant serializer would be
  cheap to add if the schema ever matures, but no projection to ASG-legal nodes exists (§8).
- **Logical text, not output text.** Nodes hold the reader's characters, not escaped HTML.
  HTML-escaping is the fold's job. This is what the `text`/`charref`/`raw` trichotomy (§3.2)
  encodes, and it is the single most important shift from the crate's original string model.
- **Structure by nesting.** Formatted spans and reference text hold child inlines, so `*a _b_
  c*` is a tree, not a flat run with tags.
- **Location on every node**, borrowed from `'src` where possible (§3.3).

The exact type definitions live in
[`parser/src/inlines/`](../../parser/src/inlines/mod.rs) and are not duplicated here; that
module is the authoritative reference.

### 3.2 The `text` / `charref` / `raw` trichotomy

In the crate's substitution order, special characters are escaped first, then attribute
references expand later and their result is *not* re-escaped — which is why `:x: <b>` then
`{x}` emits live HTML. In a string model this would be a subtle, security-relevant emergent
behavior. In the node model each of the three literal kinds is an **explicit node**:

- Ordinary source text → `Text` (logical; the fold escapes it).
- A literal `<`, `>`, `&`, or a `(C)`/`--`/smart-quote replacement → `CharRef` (the fold emits
  the right entity).
- Content that bypasses substitution entirely — a true passthrough (`+++…+++`, `pass:[…]`,
  `$$…$$`) — → `Raw` (the fold emits it verbatim).

This maps onto the ASG's `text` / `charref` / `raw` literals and means:

- **HTML parity is mechanical.** The fold is: escape `Text`, entity-encode `CharRef`, emit
  `Raw` verbatim, wrap `Styled`/`Ref` in tags.
- **The security behavior is legible.** "This document emits raw HTML" is visible as `Raw`
  nodes in the tree, rather than being an invisible property of a string.
- **Other backends are possible** without regex-mangling HTML, because they see `Raw` vs.
  `Text` rather than an already-escaped blob.

Whether a fragment becomes `Raw`, `Text`, or `CharRef` is decided by *which substitution steps
still act on it* under the content's effective substitution order (`normal`, `verbatim`, or a
custom `subs=` list) — not by a fixed property of where it came from. An attribute value
expanded mid-order carries whatever the *remaining* steps of that order would still do to it:
in the normal order, a value expanded at the `attributes` step has already passed
`specialcharacters`, so its literal `<`/`>`/`&` are emitted unescaped, but `replacements` and
`macros` still run afterward, so `(C)` in the value becomes a `CharRef` and a `link:`/`image:`
in it becomes a `Ref`/`Image`. Genuine passthroughs are the only fragments that are `Raw`
regardless of order, since they are extracted before every step and re-inserted after all of
them.

### 3.3 Locations

Every node carries a `location: Span<'src>` — the crate's existing source-position type
([`span/mod.rs`](../../parser/src/span/mod.rs)) — and `InlineNode` implements `HasSpan<'src>`,
so inline nodes locate themselves the way blocks do.

A node's `value` is separate from its `location` because a node's logical payload is not
always its source slice:

| node                              | `location.data()` (raw source) | `value` (logical)     |
| ---------------------------------- | ------------------------------- | ---------------------- |
| `CharRef` from `(C)`               | `"(C)"`                          | `©` (U+00A9)           |
| `Text` from a `{name}` expansion   | `"{name}"`                       | the attribute's value  |
| `Text`, a verbatim run             | `"hello"`                        | `"hello"` — *coincides* |

They coincide only for verbatim borrowed text; they diverge for the transformed cases the AST
exists to capture. A synthesized value (an expanded attribute, a joined multi-line run) cannot
live inside a `Span` at all — `Span::data()` is `&'src str` tied to real source bytes — which
is why the pairing is needed rather than a `Span` alone.

### 3.4 Source-location edge cases

Each node's `location` is sliced directly from the source `Span` it was recognized from. A few
categories of node cannot get a fully precise span, and fall back predictably rather than each
inventing its own policy.

**The mechanism behind every fallback.** A match-string range maps back to a source `Span`
through one function,
[`source_slice`](../../parser/src/content/inline_builder/quotes.rs) (and the byte-offset
mapper underneath it, `s_to_src`). A boundary inside an ordinary verbatim `Text` piece maps
one-to-one — its match-string position *is* its source position. A boundary inside an
**atomic** piece (a rendered span, an escaped special, a masked passthrough/STEM placeholder,
an earlier-recognized macro node) has no honest position of its own, so it snaps to that
piece's *nearer* edge. A boundary inside a **synthesized** piece (a run with no single
contiguous `'src` slice of its own — an expanded attribute value, a filtered multi-line
block's joined seed) snaps to that piece's *whole* span regardless of exactly where inside it
the boundary falls. Four cases in the crate fall back to this:

1. **Attribute expansion.** A resolved `{attribute}` reference's (or a `counter`/`counter2`
   directive's) value is spliced into the node stream as one or more `Text`/`Raw` nodes —
   split at any literal `<`, `>`, `&` it carries (§3.2) — and every node the splice produces
   carries the *whole reference's own span* as its `location`: `{name}` is a five-byte match
   that might expand to a much longer (or zero-length) value, and there is no source position
   inside that expansion for a byte to honestly claim
   ([`split_attribute_value`](../../parser/src/content/inline_builder/attribute_refs.rs)). A
   macro recognized *inside* such an expansion (`image:{logo}[Logo]`) inherits the same
   fallback one level up: its target and attribute-list values are still recovered exactly
   (via the match string, or
   [`text_slice`](../../parser/src/content/inline_builder/quotes.rs)), but the node's own
   `location` is the whole macro's own bytes as written, not the expansion's.

2. **Passthrough mask/restore.** A passthrough construct's own node — `+++text+++`,
   `++text++`, `$$text$$`, `pass:[text]`, or the bare `+text+` form — gets a precise span
   covering its whole delimited construct, delimiters included, sliced straight from `'src`
   like any other macro node
   ([`build_passthrough_node`](../../parser/src/content/inline_builder/passthrough_step.rs)).
   Only when that passthrough's own body encloses *another*, already-extracted masked
   construct — `+a $$b$$ c+` — does its *value* need restoring, each inner placeholder
   replaced by that construct's own rendered body
   ([`substitute_and_restore`](../../parser/src/content/inline_builder/passthrough_step.rs));
   the value comes back exact either way, and it is only the *outer* node's own `location`
   that can fall to the coarse policy above.

   The same split shows up one layer further out, in the `image:`/`icon:` bracket and the link
   families' display-text list: a masked construct there is *tokened* to a placeholder before
   the bracket is parsed as an `Attrlist`, and each surviving token is restored with that
   construct's own body once the parse returns
   ([`tokened_bracket`](../../parser/src/content/inline_builder/macros/image.rs),
   [`Attrlist::into_owned_restoring`](../../parser/src/attributes/attrlist.rs)). The *values*
   this produces are exact, but the `Attrlist` itself is `into_owned`'d onto the bracket's own
   coarse span.

3. **Synthesized text.** The builder's own seed can itself be synthesized — a filtered
   multi-line block whose surviving lines were joined with `\n` has no single contiguous
   `'src` slice of its own — and
   [`build_from_value`](../../parser/src/content/inline_builder/mod.rs) draws the same line
   `apply_special_characters`'s `split_text` draws one node deeper in the tree: when the seed
   `value` is exactly `location.data()`, every node built from it gets an honest, precise
   span; when it differs, every node gets `location`'s own coarse span. Recognition itself
   still succeeds from a wholly-synthesized seed — a `link:` macro folds correctly when built
   entirely from one — it is only `location` that falls back.

4. **Lookahead/retry.** Two passes re-scan a slice of their own match string rather than
   accepting or rejecting a match outright: the passthrough-extraction pass's
   *prohibited-prefix* retry — an attribute-list-prefixed bare form (`index:[attrs]+text+`,
   `` \[x-]`text` ``) that turns out to sit behind a `\`, `:`, or `;` writes that first
   character back verbatim and rescans the rest of the same match, recursively — and the
   quotes step's own monospace-before-quote retry, which slices the haystack forward and
   re-searches on a rejected look-ahead. Both retries rebase every capture offset back to the
   level's own match-string coordinates before `source_slice`/`s_to_src` ever see them, so no
   error from the retry's own bookkeeping survives into the node's `location`.

   A **failed** lookahead is simpler: the verbatim-content callout pass's own trailing-position
   lookahead either matches or it does not, and a lookahead that fails is not retried — no
   node is ever born from one, so there is no span policy for it to need.

---

### 3.5 A rejected refinement: carrying a byte-offset table through `Attrlist::parse`

The mask/restore mechanism in §3.4 case 2 recovers a masked construct's body by scanning the
parsed value for the placeholder pair and pairing occurrences positionally with a token list
built alongside it (`tokened_bracket`/`tokened_text`). A `Piece` table one layer up
(`quotes.rs`) has the stronger property of carrying byte offsets through directly rather than
re-scanning; extending that property down through `Attrlist::parse`'s own split was considered
and rejected, for two reasons that don't show up at the call site:

- **`Attrlist::parse` re-substitutes attribute references over the tokened text** whenever it
  holds a `{` and a `}` — and that changes byte offsets. A `subs=` list naming `macros` without
  `attributes` reaches this step with every reference still unresolved, so the inner
  substitution expands them *after* `tokened_bracket` has already written its placeholder,
  moving every following occurrence. Ordinal (positional) restoration is indifferent to this; a
  byte-offset table is not.
- **A parsed attribute's value is not a slice of the text that was split at all.**
  `ElementAttribute::parse` skips whitespace, strips the name/quotes, unescapes `\"`, and
  trims — none of it reported back — so remapping tokened-text coordinates into value
  coordinates would mean threading a byte mapping through most of
  [`parser/src/attributes/`](../../parser/src/attributes/).

The mechanism that *would* get the equivalent construction-time guarantee is different: escape
the placeholder's own codepoints in the bytes `tokened_bracket`/`tokened_text` **copy** from
their non-tokened pieces, at the one moment provenance is still known, rather than escaping the
content-level splice that feeds them
([`escape_passthrough_sentinels`](../../parser/src/content/inline_builder/attribute_refs.rs),
§3.4 case 2). That would let `escape_passthrough_sentinels` retire, but it requires auditing
every consumer of a tokened parse to apply the matching un-escape, so it remains a scoped,
not-yet-taken-up follow-up rather than something this document tracks as in progress.

## 4. Cross-reference and title resolution

Parsing happens in two phases — an initial parse that leaves cross-references unresolved, then
a resolution pass against the document's own catalog — and resolution operates on nodes:

- **Per-block:** walk `inlines()`; for each `Ref{Xref}` node, call the resolver, set
  `resolved`, and report unresolved targets as warnings against the node's own `location`.
- **Section titles:** the document-order title pass
  ([`title_refs.rs`](../../parser/src/document/title_refs.rs)) runs separately, since titles
  can forward- or circularly reference each other, but mutates `Ref` nodes in the title's own
  tree rather than re-rendering a template.
- **Footnote-embedded cross-references:** the `Ref` node lives inside the `Footnote` node's own
  subtree and is resolved by the same tree walk that resolves the rest of a footnote's content.

Resolution is non-destructive and re-resolvable: walking the tree and setting `resolved` does
not prevent a second resolution pass (for example, for incremental builds or multiple output
targets).

---

## 5. Rendering

`Content::rendered_html()` is the default HTML rendering: a fold of `inlines()` through the
crate's built-in `HtmlInlineRenderer`, computed lazily and cached on first read.
`Content::render_with(renderer, parser)` folds the same tree through any other
`InlineRenderer` implementation, returning an owned `String` — a pure function of the tree, not
cached by the crate (a caller with a custom renderer is responsible for caching its own output
if it needs to).

Parsing does not render: it produces the tree with every order-dependent fact already resolved
into node values (footnote numbers, callout numbers, counters, attribute-expanded text,
resolved cross-reference destinations where known at parse time). Rendering is then a pure
fold — `(tree, renderer, render context) → String` — so one parse can feed any number of
renders, to any number of backends, without reparsing.

`InlineRenderer` ([`parser/src/parser/inline_renderer.rs`](../../parser/src/parser/inline_renderer.rs))
is the seam: an AST-walking backend invoked by the fold, with one method per node kind it needs
to render (`render_xref`, `render_image`, `render_styled`, …). Each method receives the
relevant node (or its fields) and appends to the output.

**Cache invalidation.** `rendered_html()`'s cache is empty until first read and computed on
demand. Reading before cross-reference resolution has run is legal and defined — it yields the
unresolved-fallback HTML — and once resolution runs and clears the cache, the next read
reflects the resolved destinations.

---

## 6. Lifetimes and allocation

- `Text`/`Raw` hold `CowStr<'src>`: `Borrowed` for verbatim runs (the common case), owned only
  when a value is synthesized (attribute expansion, joined multi-line runs). In the borrowed
  case the `value` and the node's `location.data()` are the *same* `'src` bytes, so carrying
  both costs nothing extra.
- A paragraph with no inline constructs is a single borrowed `Text` node.

---

## 7. Testing

Golden-HTML string assertions (`.rendered_html()` compared against literal expected output)
are the crate's primary correctness oracle for inline content: any internal change that alters
observable HTML output fails one. Where a differential comparison against the crate's retired
string-substitution implementation was once useful during development, those comparisons are
now frozen, checked-in recordings rather than a live second implementation — see
[`parser/snapshots/README.md`](../../parser/snapshots/README.md) for the recording format and
how individual `inline_builder` submodules use it.

Structural assertions (on `inlines()` directly) supplement the golden-HTML assertions for
guarantees a rendered string can't express — node kinds, nesting, per-node spans, resolved
cross-reference destinations.

---

## 8. Relationship to the Eclipse ASG

The crate does not build a conformant ASG serializer (`Document::to_asg()`). Reading the
published schema (pinned at revision `d335f565`, `$id`
`https://schemas.asciidoc.org/asg/1-0-0/draft-01`) shows the projection is not expressible: no
inline node has a role, id, or attribute slot; `inlineSpan.variant` is a closed four-value enum
with no room for this crate's superscript/subscript/unquoted-span variants; and there is no
table block at all. The schema is also an unmaintained 2023 draft — untouched since
2023-09-24 — whose own fixture suite has been failing validation since a later commit narrowed
a field's type without updating the sample data that exercises it. A lossy projection against
an artifact in that state cannot usefully distinguish a real projection bug from expected
lossiness, so it would not strengthen the crate's actual safety net (the golden-HTML oracle,
§7).

What the crate *does* keep is the ASG's **shape**: node names, `variant`/`form`, and the
`text`/`charref`/`raw` trichotomy (§3.2) are aligned with the schema's vocabulary, so a
conformant serializer would be a small, mechanical addition rather than a redesign if the
schema ever matures.

**Revisit this if** upstream `asg/` gets a revision that (1) gives inline nodes somewhere to
put roles and ids, (2) adds a table block plus inline nodes for footnotes, images, index
terms, UI macros, callouts, STEM, and line breaks (or an extension point admitting them), and
(3) validates its own fixtures. None of that is on a schedule; there is no date to check back
on, only upstream activity to notice.

---

## 9. Decisions

These were open questions during the initial design and were accepted by the maintainer; they
stand as the crate's own settled decisions.

1. **Node text: logical vs. rendered vs. source?** → **Logical** (reader's characters), with
   escaping deferred to the fold. This is what enables clean alternate backends.
2. **Model every macro, or a catch-all?** → **Model** image / footnote / xref / link / anchor /
   kbd / btn / menu / index term / callout / stem as named variants, rather than a catch-all
   `Macro{kind, text}` that loses structure.
3. **Owned vs. borrowed strings?** → **`CowStr<'src>`**, borrowed by default.
4. **Spans from day one?** → **Yes**, as a field on every node (§3.3).
5. **Retain a rendered-string accessor?** → **Yes** — `rendered_html()`, a cached default-HTML
   fold (§5); custom backends go through `render_with`.
6. **Which downstream tool pins the API?** → the Ruby-to-Rust `asciidoctor` port, the crate's
   most demanding consumer: it walks and renders the entire inline vocabulary, so it exercises
   the renderer seam and node kinds comprehensively, and its own byte-exact HTML parity
   requirement is a strong secondary check on §7's oracle.
7. **Emit conformant ASG (`Document::to_asg()`)?** → **No.** See §8.

---

## 10. Relationship to existing issues

- **#892** — resolved: inline content is exposed as structure, not opaque HTML.
- **#943** — subsumed: the read-only tree is canonical rather than an additive, opt-in layer.
- **#944** — resolved: the single-pass builder assigns per-node source positions by span
  containment (§3.4).
- **#564** — resolved: the `attribute-missing` diagnostic uses the offending node's own span
  rather than a per-line correlation hack.
- **#942** — superseded: its `InlineNode` shape and marker-recording renderer were reused as
  prior art and as a bring-up oracle during development; its limitations (owned strings, no
  spans, a second pass, drift between two artifacts) are exactly what the current design
  eliminates.

## References

- Eclipse AsciiDoc Language ASG schema (pinned; unchanged upstream since 2023 — §8):
  <https://gitlab.eclipse.org/eclipse/asciidoc-lang/asciidoc-lang/-/blob/d335f56572b656a7c9f84a5e0c76ea6f41f281e1/asg/schema.json>
- AsciiDoc substitutions: <https://docs.asciidoctor.org/asciidoc/latest/subs/>
