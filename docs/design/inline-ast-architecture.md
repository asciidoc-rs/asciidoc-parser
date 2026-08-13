# Inline AST architecture

**Status:** proposal / request for comment
**Date:** 2026-08-02
**Target branch:** `inline-ast` (long-running feature branch)
**Scope:** replace the inline text-substitution content model with a first-class inline AST; make rendering a projection of that AST.

---

## 1. Motivation

### 1.1 What the AsciiDoc Language team is moving toward

The Eclipse AsciiDoc Language project defines an **Abstract Semantic Graph (ASG)** as the
canonical machine-readable representation of a parsed document, published as a JSON
Schema:
<https://gitlab.eclipse.org/eclipse/asciidoc-lang/asciidoc-lang/-/blob/main/asg/schema.json>.

In the ASG, a leaf block does **not** carry a rendered string. It carries an ordered array
of **inline nodes** (`inlines`). The inline vocabulary is deliberately small:

| ASG node        | `name`                     | discriminant fields                                              | children   |
| --------------- | -------------------------- | --------------------------------------------------------------- | ---------- |
| `inlineSpan`    | `"span"`                   | `variant ∈ {strong, emphasis, code, mark}`, `form ∈ {constrained, unconstrained}` | `inlines`  |
| `inlineRef`     | `"ref"`                    | `variant ∈ {link, xref}`, `target`                              | `inlines`  |
| `inlineLiteral` | `"text" \| "charref" \| "raw"` | `value` (string)                                            | *(none)*   |

Every node carries an optional `location` (a two-element `[start, end]` array of
`{ line, col, file }` boundaries). The direction of travel is explicit: **inline content
is structured data, and text substitution is an implementation detail of a renderer, not
the model itself.**

### 1.2 Where this crate is today

This crate implements inline content as **string rewriting**. `Content<'src>`
([`content.rs:40`](../../parser/src/content/content.rs)) holds a single mutable
`rendered: CowStr<'src>` field that each substitution step edits in place, plus two
sidecar mechanisms bolted onto that string:

- **Passthroughs** — extracted before substitution, re-spliced after, tracked by C1
  sentinel characters (`\u{96}` … `\u{97}`) embedded in the string.
- **Deferred cross-references** — captured as a placeholder template with Unicode
  Private-Use-Area sentinels (`\u{E000}` … `\u{E001}`), resolved in a later pass.
  Footnote markers use a third sentinel pair (`\u{E002}` … `\u{E003}`).

The substitution pipeline
([`substitution_group.rs`](../../parser/src/content/substitution_group.rs),
[`substitution_step.rs`](../../parser/src/content/substitution_step.rs),
[`macros.rs`](../../parser/src/content/macros.rs)) detects syntax with regexes and, for
each construct it recognizes, calls a method on
[`InlineSubstitutionRenderer`](../../parser/src/parser/inline_substitution_renderer.rs)
that writes **final output markup directly into a `&mut String`**. The default
`HtmlSubstitutionRenderer` emits HTML. There is no point at which inline structure exists
as data — it is recognized and rendered to string in the same motion.

The entire public inline surface is a flat string:

- `Content::rendered() -> &str`, `Content::original() -> Span`, `is_empty()`,
  `has_unresolved_refs()`, `passthroughs()`.
- `IsBlock::rendered_content() -> Option<&str>`, `IsBlock::title() -> Option<&str>`.
- `SimpleBlock::content() -> &Content`.

### 1.3 Why change now

This has been recognized as a limitation for a while. The relevant history:

- **#892** (closed): *"inline content is exposed only as opaque, pre-rendered HTML."* The
  problem statement. All three planned downstream tools (Zola backend, spec coverage,
  version diff) need semantic inline structure, not an HTML blob.
- **#942** (closed prototype): a working proof that a tree can be captured by re-running
  the pipeline with a marker-recording renderer, leaving `rendered()` byte-identical. It
  established the `InlineNode` shape and API feel but was explicitly not for merge (owned
  strings, no source spans, double substitution).
- **#943** (open): the *structure* axis — expose a read-only inline node tree. Deferred
  pending a real consumer to pin the API.
- **#944** (open): the *positioning* axis — a source-map sidecar, converging on a
  single-pass AST built by span containment. Notes that this would also retire the
  `attribute-missing` per-line hack from **#564**.

Those tickets treated the AST as an **additive, opt-in** layer *alongside* the
authoritative rendered string. This proposal **inverts** that: the AST becomes the
**canonical** inline representation, and the rendered string becomes a **fold over the
tree**. Two things make now the right time:

1. **The language is standardizing on the ASG.** Aligning the internal model with the ASG
   now — while we are pre-1.0 and free to break the API — avoids a far more disruptive
   migration after 1.0, and positions the crate to emit conformant ASG (and to pass the
   language TCK) as a first-class output.
2. **The rendered-string model is a local maximum.** Every capability the downstream tools
   want (per-node access, source spans, structural diff, alternate backends that re-flow
   rather than regex-mangle HTML) is blocked by the same root cause: structure is never
   materialized. Bolting more sidecars onto the string (a third sentinel system, a second
   capture pass) pays compounding complexity for each one.

---

## 2. Goals and non-goals

### Goals

1. A **public, read-only inline AST** exposed per content block, aligned with the Eclipse
   ASG core (span / ref / literal, with `variant` and `form`) and extended to cover the
   inline constructs this crate already supports (images, footnotes, UI macros, index
   terms, callouts, anchors, line breaks, STEM).
2. **Rendering is a fold over the AST.** `InlineSubstitutionRenderer` (or its successor)
   becomes an AST walker. HTML output is one projection; the ASG JSON is another.
3. **Byte-for-byte HTML parity** with today's output throughout the migration, guarded by
   the existing ~277 `.rendered()` golden-string assertions used as an oracle.
4. **Per-node source locations** designed into the public type from day one (populated
   coarsely at first, precisely once the single-pass builder lands — issue #944), so the
   public shape does not churn when positioning arrives.
5. **`'src` borrowing** for untransformed text runs, so the common case does not allocate.
6. A **transition plan** that keeps `main` continuously shippable and lets unrelated work
   proceed against `main` while this branch is in flight.

### Non-goals

- **Extensions** (custom inline macros) remain out of scope for 1.0, per the README.
- **A new output backend** (Markdown, DocBook, …) is not part of this work, though the
  fold-over-AST design is what makes such backends tractable later.
- **Reimplementing Asciidoctor's inline grammar.** Structure is still derived from the
  same regex-detection events the current pipeline uses; we change the *sink* (nodes
  instead of string), not the *recognition*. This preserves fidelity.
- **Round-tripping AST → source.** The AST is a semantic graph, not a lossless CST.

---

## 3. Proposed public data model

### 3.1 Design principles

- **ASG core, crate superset.** The four ASG shapes (span, ref, text/charref/raw) are the
  spine. Everything this crate supports beyond the ASG is an additional variant that
  *projects down* to ASG-legal nodes (usually `span`/`ref`/`text`) when emitting conformant
  ASG, and renders richly in HTML.
- **Logical text, not output text.** Nodes hold the reader's characters, not escaped HTML.
  HTML-escaping is the fold's job. This is what the ASG's `text` / `charref` / `raw`
  trichotomy encodes, and it is the single most important shift from the current model
  (see §3.4).
- **Structure by nesting.** Formatted spans and reference text hold child inlines, so
  `*a _b_ c*` is a tree, not a flat string with tags.
- **Location on every node**, borrowed from `'src`.

### 3.2 The core types

```rust
/// One inline node. Borrows text from the source where it can.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InlineNode<'src> {
    // ─── ASG literal nodes (leaves) ───────────────────────────────────

    /// Logical text between constructs. `value` is the *reader's* text
    /// (attribute references expanded, but NOT HTML-escaped); the HTML fold
    /// escapes `< > &`. `location` is the source `Span` this text derives from
    /// (see §3.2.1); in the verbatim case `value` borrows the very slice
    /// `location.data()` covers. ASG: `inlineLiteral name="text"`.
    Text { value: CowStr<'src>, location: Span<'src> },

    /// A character reference / typographic replacement: a special character
    /// (`<` `>` `&`), a character-replacement result (`(C)`, `--`, `...`,
    /// smart quotes, arrows), or a numeric/named entity. `value` is the logical
    /// character(s); the fold decides the concrete entity. `location` is the
    /// source that produced it (e.g. the `(C)` span). ASG:
    /// `inlineLiteral name="charref"`.
    CharRef { value: CharRef<'src>, location: Span<'src> },

    /// Verbatim, un-escaped output: passthrough content (`+++…+++`,
    /// `pass:[…]`, `$$…$$`), and any literal special character of an
    /// attribute expansion that the effective substitution order left
    /// unescaped (§3.4.1) — *not* the whole expansion, whose replacements
    /// and macros are still recognized. This node is the model's record of
    /// the language's "emit raw HTML by design" behavior (see §3.4). ASG:
    /// `inlineLiteral name="raw"`.
    Raw { value: CowStr<'src>, location: Span<'src> },

    // ─── ASG parent nodes ─────────────────────────────────────────────

    /// A formatted span. ASG: `inlineSpan`. The ASG `variant` set is
    /// {strong, emphasis, code, mark}; the crate extends it (superscript,
    /// subscript, smart-quoted, and role-only/unquoted spans) and projects
    /// those to the nearest ASG-legal form when emitting ASG.
    Styled(Styled<'src>),

    /// A link or cross-reference. ASG: `inlineRef`, `variant ∈ {link, xref}`.
    Ref(Ref<'src>),

    // ─── crate extensions (no ASG inline node yet) ────────────────────

    Image(Image<'src>),
    Footnote(Footnote<'src>),
    Anchor(Anchor<'src>),
    Ui(Ui<'src>),          // kbd: / btn: / menu:
    IndexTerm(IndexTerm<'src>),
    Callout(Callout<'src>),
    Stem(Stem<'src>),      // inline stem:/asciimath:/latexmath:
    LineBreak { location: Span<'src> },
}
```

Supporting types (sketch — field sets to be refined against the first consumer):

```rust
pub struct Styled<'src> {
    pub variant: StyleVariant,
    pub form: SpanForm,          // Constrained | Unconstrained (ASG `form`)
    pub id: Option<CowStr<'src>>,
    pub roles: Vec<CowStr<'src>>,
    pub attrs: Option<Attrlist<'src>>,
    pub children: Vec<InlineNode<'src>>,
    pub location: Span<'src>,
}

pub enum StyleVariant {
    // ASG-native:
    Strong, Emphasis, Code, Mark,
    // crate extensions (project to Mark/span-with-role for ASG):
    Superscript, Subscript, DoubleQuote, SingleQuote, Unquoted,
}

pub struct Ref<'src> {
    pub variant: RefVariant,     // Link | Xref (ASG `variant`)
    pub target: CowStr<'src>,    // raw target as written
    pub children: Vec<InlineNode<'src>>,   // display text (may be formatted)
    pub roles: Vec<CowStr<'src>>,
    pub window: Option<CowStr<'src>>,
    /// For Xref: resolved destination, filled by the resolution pass;
    /// `None` while unresolved or for a standalone (document-less) parse.
    pub resolved: Option<ResolvedReference>,
    pub location: Span<'src>,
}

pub enum CharRef<'src> {
    /// A special character that the HTML fold escapes (`<`→`&lt;` …).
    Special(char),
    /// A replacement whose logical value is one or more characters
    /// (e.g. `(C)` → U+00A9, `--` → em dash, `...` → ellipsis).
    Replacement(&'static str),   // logical character(s)
    /// An explicit numeric/named entity written by the author (`&#8217;`).
    Entity(CowStr<'src>),
}
```

#### 3.2.1 Locations reuse `Span`

Every node carries a `location: Span<'src>` — the crate's existing source-position type
([`span/mod.rs`](../../parser/src/span/mod.rs)): a borrowed source slice plus its
`line`/`col`/`offset`. It is `Copy` and cheap, so one per node costs nothing structural,
and `InlineNode` implements the existing [`HasSpan<'src>`] trait, so inline nodes locate
themselves exactly the way blocks already do. No bespoke location wrapper is introduced.

The `value` field on a leaf is *separate from* its `location` precisely because a node's
**logical payload is not always its source slice**. `Span` carries the *origin* (and, via
`data()`, the raw source bytes); `value` carries the *meaning*:

| node                            | `location.data()` (raw source) | `value` (logical)        |
| ------------------------------- | ------------------------------ | ------------------------ |
| `CharRef` from `(C)`            | `"(C)"`                        | `©` (U+00A9)             |
| `Text` from a `{name}` expansion | `"{name}"`                    | the attribute's value    |
| `Text`, a verbatim run          | `"hello"`                      | `"hello"` — *coincides*  |

They coincide only for verbatim borrowed text (where `value` is `CowStr::Borrowed` over the
same bytes `location` covers); they diverge for exactly the transformed cases the AST exists
to capture. A synthesized value (`©`, an expanded attribute, a joined multi-line run) cannot
live inside a `Span` at all — `Span::data()` is `&'src str` tied to real source bytes, and a
synthesized value has none — which is why the pairing is needed rather than a `Span` alone.

[`HasSpan<'src>`]: ../../parser/src/span/mod.rs

### 3.3 How blocks expose it

`Content<'src>` changes from *"a rendered string plus sidecars"* to *"a tree plus a cached
rendering."*

```rust
impl<'src> Content<'src> {
    /// The parsed inline tree. This is the canonical representation.
    pub fn inlines(&self) -> &[InlineNode<'src>];

    /// The default **HTML** rendering: a fold of `inlines()` with the
    /// built-in HTML renderer. Computed lazily and cached, so it can hand
    /// back `&str`. Renamed from today's `rendered()` to make the
    /// HTML-specificity explicit (§3.3.1, §7).
    pub fn rendered_html(&self) -> &str;

    /// Render this content to a caller-supplied backend. A pure fold over
    /// the same `inlines()`; returns an owned `String`, not cached.
    pub fn render_with(&self, renderer: &dyn InlineRenderer) -> String;

    pub fn original(&self) -> Span<'src>;   // unchanged
    pub fn is_empty(&self) -> bool;         // unchanged
}
```

`IsBlock` grows a structured accessor alongside the string one:

```rust
fn inlines(&'src self) -> Option<&'src [InlineNode<'src>]> { None }
// rendered_html_content()/title() retained as a fold, at least through
// migration (rendered_content() likewise renamed for HTML-specificity).
```

#### 3.3.1 Rendering, renderer selection, and caching

Making the AST canonical **decouples rendering from parsing**, and that changes what the
rendered-string accessor means — including its name, which becomes `rendered_html()` (from
today's `rendered()`) to signal the shift. Three points resolve it:

1. **Parsing no longer renders.** It produces the AST with every *order-dependent* fact
   already resolved *into node values* — footnote numbers, callout numbers, counters,
   attribute-expanded text, resolved cross-reference destinations. Rendering is then a
   **pure fold** `(AST, renderer, render-context) → String`, with no stateful document
   traversal left to do. (Today this is impossible: substitution bakes output into the
   string *during* parse, which is exactly why the #942 prototype had to *clone the parser*
   to capture a second view. Here, one parse feeds any number of renders.)

2. **`rendered_html()` has a fixed meaning: the built-in HTML backend.** It folds `inlines()`
   with the crate's `HtmlInlineRenderer` and the document's own resolved render-context. Its
   very name now states the backend — a deliberate improvement over today's `rendered()`,
   which silently returns whatever renderer was configured on the `Parser`, an ambiguous
   contract. So the answer to *"which `InlineRenderer` does the work when I call
   `rendered_html()`?"* is unambiguous: **always the built-in HTML one.**

3. **A custom (non-HTML) backend is a render-time argument, not a parse-time global.** The
   consumer never gets it invoked *through* `rendered_html()`; they call `render_with(&their_
   renderer)` (or `Document::render_to(&their_renderer)`, or walk `inlines()` themselves) —
   a pure fold they drive over the already-parsed tree. They can render the same document to
   several backends without reparsing.

**What caching means here.** The crate memoizes exactly **one** artifact — the default HTML
fold — lazily on `Content`, which is what lets `rendered_html()` return `&str`. That single
artifact has a stable identity (the built-in renderer + the document's own context). The AST
is *not* fully frozen at parse time, however: the resolution passes mutate reference nodes in
place (block cross-references via `Content::resolve_references`, and — separately — section
titles via `title_refs`, both in §4.3). The cache policy is therefore explicit:

- **Lazy, invalidate-on-mutation.** The cache is empty until first read and computed on
  demand. **Every mutation of a node the fold depends on clears it** — this covers *both*
  block-reference resolution and title resolution, and any future in-place edit. The next
  read recomputes.
- **Reading before resolution is legal and defined.** It yields the unresolved-fallback
  HTML (exactly as `rendered()` does today); once resolution runs and clears the cache, the
  following read reflects the resolved destinations. There is no window in which a stale
  pre-resolution string is returned as if final.
- Concretely this is the same recompute point today's code already has — `resolve_references`
  rebuilds the rendered string after resolving — expressed as cache invalidation rather than
  eager rebuild.

Arbitrary custom renderers are deliberately **not** cached by the crate: their outputs are
owned `String`s the caller may cache as it sees fit, since a crate-side cache would need to
be keyed by renderer identity/config — an unbounded space that is not the crate's
responsibility.

> **Decision:** rename `rendered()` → **`rendered_html()`** (and `rendered_content()` →
> `rendered_html_content()`), defined as the cached default-HTML fold; all other backends go
> through an explicit `render_with`/`render_to` fold and are the caller's to cache.
> Parse-time renderer configuration on `Parser` is **dropped** — the renderer moves to render
> time. *(This is the clean pre-1.0 break; the alternative — keep one `rendered()` whose
> meaning is configurable — was
> rejected because it reintroduces the "which renderer ran?" ambiguity the AST lets us
> finally remove.)*

### 3.4 The `text` / `charref` / `raw` trichotomy — the key idea

In the normal order, special characters are escaped *first* (step 1), then attribute
references expand *later* (step 3) and their result is *not* re-escaped — which is why
`:x: <b>` then `{x}` emits live HTML. In a string model this is a subtle, security-relevant
emergent behavior. In the node model each of the three literal kinds is an **explicit node**:

- Ordinary source text → `Text` (logical; the fold escapes it).
- A literal `<`, `>`, `&`, or a `(C)`/`--`/smart-quote replacement → `CharRef` (the fold
  emits the right entity).
- Content that bypasses substitution entirely — a true passthrough (`+++…+++`, `pass:[…]`,
  `$$…$$`) — → `Raw` (the fold emits it verbatim). *Whether a given fragment is `Raw`
  depends on the substitution order — see below.*

This maps **exactly** onto the ASG's `text` / `charref` / `raw` literals, and it means:

- **HTML parity is mechanical.** The fold is: escape `Text`, entity-encode `CharRef`, emit
  `Raw` verbatim, wrap `Styled`/`Ref` in tags. Because the *decisions* the current steps
  make are preserved as node kinds, the fold reproduces today's bytes.
- **The security behavior is now legible.** "This document emits raw HTML" is visible as
  `Raw` nodes in the tree, rather than being an invisible property of a string. The README
  security note gets a precise structural anchor.
- **Other backends become possible** without regex-mangling HTML, because they see
  `Raw`-vs-`Text` rather than an already-escaped blob.

#### 3.4.1 Node building follows the effective substitution order

The kind a fragment becomes is **not** a fixed property of where it came from; it is decided
by *which substitution steps still act on it* under the group's effective order (`normal`,
`verbatim`, or a custom `subs=` list). Attribute expansion is the case that makes this
concrete, and it is why an attribute value must **not** be blanket-classified as `Raw`:

- In the **normal** order (`specialcharacters → quotes → attributes → replacements →
  macros`), a value expanded at the *attributes* step has already passed `specialcharacters`
  and `quotes`, so its literal `<`/`>`/`&` are emitted unescaped (`Raw`) and `*bold*` in it
  stays literal — **but** `replacements` and `macros` still run afterward, so `(C)` in the
  value becomes a `CharRef` and a `link:`/`image:` in it becomes a `Ref`/`Image`. The
  expansion therefore yields a **mix** of node kinds, not a single `Raw` blob.
- Under a **custom order** that runs `attributes` *before* `specialcharacters`, the same
  value *is* subsequently escaped, so its `<` becomes a `CharRef::Special` (escaped by the
  fold), not `Raw`.

So the builder splices an expanded value into the node stream at the point the `attributes`
step runs and lets the *remaining* steps of the effective order classify its fragments —
mirroring exactly what the current string pipeline does to that text. Genuine passthroughs
are the only fragments that are `Raw` regardless of order, because they are extracted before
any step and re-inserted after all of them. This ordering-faithful policy is what keeps the
byte-for-byte parity claim above actually achievable.

### 3.5 ASG projection and conformance

Provide `Document::to_asg()` (and per-node projection) emitting JSON conformant to the
Eclipse schema. Crate-extension nodes project as follows (proposal):

| Crate node                         | ASG projection                                                        |
| ---------------------------------- | --------------------------------------------------------------------- |
| `Styled{Superscript/Subscript}`    | `span` with a role (`superscript`/`subscript`) — closest legal form   |
| `Styled{DoubleQuote/SingleQuote}`  | children with `charref` quote characters, no wrapping span             |
| `Styled{Unquoted}`                 | `span` carrying only id/roles                                          |
| `Image`, `Footnote`, `Ui`, `IndexTerm`, `Callout`, `Stem`, `Anchor` | best-effort `span`/`ref`/`text` (the ASG does not yet model these) |

This projection doubles as a **conformance test surface**: we can validate our ASG output
against the schema and, when the language TCK matures, run it. Where the ASG under-models
what we support, we document the projection choice and keep the richer native node.

---

## 4. Key implementation details

### 4.1 The central move: recognition sink = nodes, not string

The regex-detection layer (the `apply_*` step functions and the macro replacers) is sound
and hard-won; it encodes Asciidoctor's ordering quirks faithfully. We keep it. What
changes is the **sink**: instead of each recognized construct calling
`renderer.render_*(…, &mut String)`, it emits an `InlineNode` into a builder. Concretely:

- The step functions stop rewriting `content.rendered` and instead consume a list of nodes
  and produce a list of nodes (each step refines the tree: `SpecialCharacters` splits
  `Text` runs into `Text` + `CharRef`; `Quotes` wraps runs in `Styled`; `Macros` replaces
  matched spans with `Ref`/`Image`/`Footnote`/…; etc.).
- `InlineSubstitutionRenderer` is repurposed from a *string emitter invoked during
  substitution* into an **AST-walking backend** invoked by the fold. Its method set
  (`render_xref`, `render_image`, `render_quoted_substitution`, …) maps almost 1:1 onto
  node kinds, so `HtmlSubstitutionRenderer` largely survives — its methods now receive a
  node (or node fields) and append HTML, instead of being called mid-regex.

Two construction strategies were considered:

- **Strategy A — recording second pass (the #942 prototype).** Re-run the existing pipeline
  with a marker-recording renderer, parse markers into a tree. *Pros:* smallest change,
  proven. *Cons:* double substitution, owned strings, no honest spans, two artifacts that
  can drift.
- **Strategy B — single canonical pass, nodes as the sink (#944 convergence).** One pass
  builds the tree directly; the string is a fold. *Pros:* one artifact, `'src` borrowing,
  real spans, no drift. *Cons:* touches every step.

> **Decision: Strategy B.** The single canonical pass is the target architecture — it is
> the only option that delivers `'src` borrowing, honest per-node spans, and a single
> non-drifting artifact, all of which are stated goals (§2). Strategy A is **not** the end
> state; its marker-recording renderer is retained only as a **bring-up oracle** (§5.4).
>
> Because Strategy B touches every step, the migration reaches it incrementally under the
> phase gates of §5.2 rather than in one leap: the interior is cut over to nodes in Phase 2,
> and the true single-pass builder (with precise spans) lands in Phase 4. Throughout, *every
> step is gated by the HTML oracle* (§5.4), and during early bring-up the node stream is
> cross-checked against Strategy A's recorder to catch structural regressions the HTML
> oracle cannot see (two different node trees can fold to the same HTML).

### 4.2 Retiring the sentinel systems

All three sentinel mechanisms exist only because structure had to be smuggled through a
flat string. In the node model they become ordinary nodes:

- **Passthroughs** (`\u{96}`/`\u{97}`): a passthrough is a `Raw` node (or a `Styled`/other
  subtree if its `subs` re-run over it). Extraction still happens first (to protect its
  content from the other steps), but re-splicing is just *keeping the node in place*
  rather than replacing a sentinel. `Content::passthroughs()` can be retained as a
  filtered view over the tree.
- **Deferred cross-references** (`\u{E000}`/`\u{E001}` + template): an xref is a `Ref{
  variant: Xref, resolved: None }` node. Resolution walks the tree and fills
  `resolved` in place — non-destructive by construction, re-resolvable, no template.
- **Footnote markers** (`\u{E002}`/`\u{E003}`): a `Footnote` node. The
  "strip footnote markers from a section title's reftext/id" logic becomes a tree filter
  (drop `Footnote` nodes) instead of sentinel-span deletion.

### 4.3 Cross-reference and title resolution

The two-phase parse (`parse_deferred` then `resolve_against_own_catalog`) is retained, but
resolution operates on nodes:

- Per-block: walk `inlines()`, for each `Ref{Xref}` call the resolver, set `resolved`,
  report unresolved targets as warnings against the **node's** `location` (tighter than
  today's whole-content span).
- Section titles: the document-order title pass
  ([`title_refs.rs`](../../parser/src/document/title_refs.rs)) still runs separately
  (titles can forward/circularly reference), but it now mutates `Ref` nodes in the title's
  tree instead of re-rendering a template. The "resolve once, cache" model is preserved.
- Footnote-embedded xrefs (a known prototype gap) fall out naturally: the `Ref` node lives
  inside the `Footnote` node's subtree and is resolved by the same tree walk.

### 4.4 Source locations (issue #944)

Locations reuse the existing `Span<'src>` throughout (§3.2.1) — no new location type — and
`InlineNode` implements `HasSpan<'src>`. Each node carries its `Span` from the start;
populate in two stages:

1. **Migration stage:** nodes recognized directly from a source slice get a precise `Span`;
   nodes born from transformation (attribute expansion, synthesized entities) get a
   documented fallback (the reference's `Span`, or the enclosing block's). Because the field
   is already a `Span`, this is a matter of *which* `Span` a node is given — not a type
   change — so precision can improve later with no public churn.
2. **Precision stage (#944):** the single-pass builder assigns each construct the source
   range it consumed and slices interstitial `Text` directly from `'src`. The hard cases
   #944 enumerates — attribute expansion, passthrough mask/restore, synthesized text,
   lookahead/retry — get explicit policies there. Landing this lets the `attribute-missing`
   diagnostic (#564) drop its per-line correlation hack and use the offending node's span.

### 4.5 Lifetimes and allocation

- `Text`/`Raw` hold `CowStr<'src>`: `Borrowed` for verbatim runs (the common case),
  `Boxed` only when a value is synthesized (attribute expansion, joined multi-line runs).
  In the borrowed case the `value` and the node's `location.data()` are the *same* `'src`
  bytes, so no extra allocation is incurred to carry both.
- The current `from_filtered_lines` fast-path (borrow a single surviving line) generalizes:
  a paragraph with no inline constructs is a single borrowed `Text` node.
- `OwnedTitle` (the owned snapshot a pending section title rides on) becomes an owned node
  vector; the borrow/owned duality already exists, so this is a reshaping, not a new
  concept.

### 4.6 Renderer seam changes (breaking, intentional)

`InlineSubstitutionRenderer` and its ~13 `*RenderParams` structs are public and documented
as the alternate-backend seam. They will change shape (from "called during substitution
with a `&mut String`" to "called by the fold with a node"). This is an accepted pre-1.0
break. The method *set* is largely preserved, so a downstream implementer's mental model
survives; the `*RenderParams` structs are replaced by (or become borrowed views of) the
corresponding node types.

> **Decision (Phase 5): rename the trait to `InlineRenderer`.** The word "substitution"
> names the very mechanism this work removes — post-migration the trait is no longer invoked
> *during substitution*; it is an AST-walking backend invoked by the fold, so the old name
> would misdescribe it. `Inline` and `Renderer` both stay accurate, so only the middle word
> is dropped. The cascade is small and lands in the same Phase 5 pass:
>
> - `InlineSubstitutionRenderer` → **`InlineRenderer`**; `HtmlSubstitutionRenderer` →
>   **`HtmlInlineRenderer`**.
> - The one substitution-named method, `render_quoted_substitution`, is renamed to match its
>   node kind (e.g. `render_styled`).
> - The `*RenderParams` structs fold into the node types (e.g. `XrefRenderParams` → the
>   `Ref` node), so most are removed rather than renamed.
>
> Considered and set aside: `InlineBackend` (introduces a "backend" noun not used elsewhere),
> `InlineNodeRenderer` ("node" is redundant with "inline"), `InlineConverter` (the crate has
> already committed to "renderer" vocabulary).

---

## 5. Managing the transition

The overriding constraints: **`main` stays untouched** while this is in flight (so
unrelated work continues against it undisturbed), and **HTML output never regresses**. The
entire feature is developed on the long-running `inline-ast` branch and lands in `main` via
a **single merge commit at the end** — the phases below are *internal* milestones on that
branch, not separate submissions to `main`. Each is guarded by the golden-HTML oracle.

### 5.1 Branch strategy

- `inline-ast` is the long-running integration branch and the **sole staging ground**: all
  of the work accumulates here and reaches `main` only once, at the very end. Nothing is
  submitted to `main` incrementally.
- **Landing uses a real merge commit, not a squash** — a deliberate, one-off exception to
  the project's usual squash-merge policy, so the staged phase history (and the in-branch
  topic-branch merges) is preserved in `main` rather than flattened. This is warranted here
  precisely because the staged history is the record of how a large architectural change was
  made incrementally and kept green at each step.
- Per the project's PR policy, we **merge `main` into `inline-ast` periodically** (never
  rebase/rewrite in-flight history) to stay current.
- Feature work is done on short-lived topic branches cut from `inline-ast` and merged back
  into it, so review stays granular within the branch.
- **Pre-landing gate:** before the final merge to `main`, the branch is **preflighted
  against the Ruby-to-Rust `asciidoctor` port** — the only consumer currently underway
  (§6.6) and the one that implements/drives the renderer seam. Because the seam changes
  shape (Phase 5), this confirms the public API and the reshaped `InlineRenderer` actually
  serve a real consumer before they are locked into `main`.

### 5.2 Phased plan

Each phase is a reviewable unit with a clear exit gate.

- **Phase 0 — this document + skeleton.** ✅ **Done** (#1058, #1060). Land the `InlineNode`
  module (types only, no wiring) and this doc on `inline-ast`. Decisions in §6 resolved or
  explicitly deferred.
  *Exit:* types compile, doc reviewed.

- **Phase 1 — build the tree as an internal, non-public artifact; keep `rendered()`
  authoritative.** ✅ **Done**. Implement the node builder (Strategy A recorder first, as an
  oracle), producing a tree in parallel with the existing string. Nothing public changes.
  *Exit:* for the whole test corpus, the fold of the tree equals the existing
  `rendered()` **byte-for-byte** (the oracle, §5.4), and the tree cross-checks against the
  recorder.

  *Landed as:* a transparent marker-recording decorator over the built-in HTML renderer
  ([`RecordingRenderer`]), a builder that parses the recorded sentinel structure into an
  `InlineNode` tree, and a fold back to HTML, all behind a test-only differential harness
  ([`parser/src/tests/inline_recorder.rs`]) that asserts byte-for-byte parity over an inline
  fixture corpus and over whole parsed documents (including resolved cross-references).
  Special characters and character replacements are left unmarked (their escaped output is
  re-consumed by later steps, so bracketing it would perturb recognition) and their
  `CharRef` nodes are recovered by splitting text runs. One Strategy A drift case is known
  and recorded as an ignored test: an `<<auto-id>>` cross-reference whose target is a
  section with a *formatted* title, whose rendered (marker-bearing) text feeds the reference
  catalog. The single-pass builder (Phase 4) retires it by never re-rendering.

[`RecordingRenderer`]: ../../parser/src/content/inline_tree.rs
[`parser/src/tests/inline_recorder.rs`]: ../../parser/src/tests/inline_recorder.rs

- **Phase 2 — make the tree canonical; `rendered()` becomes a fold.** 🔶 **In progress.**
  Flip `Content` so the tree is the source of truth and `rendered()` is computed from it.
  Retire the sentinel systems (§4.2). This is the load-bearing internal cutover.
  *Exit:* all ~277 golden `.rendered()` assertions pass unchanged; sentinels deleted;
  benchmarks within an agreed budget of `main`.

  *Step 1 landed as (promote the tree into `Content`):* the Phase 1 recorder machinery
  moved out of the test build into a production module
  ([`content::inline_tree`](../../parser/src/content/inline_tree.rs)) that wraps the
  parser's *own* renderer (so the fold reproduces that renderer's bytes, not a hard-coded
  HTML backend). `Content` now carries a live
  [`inlines`](../../parser/src/content/content.rs) tree, populated during substitution and
  exposed via `Content::inlines()`. Tree building is gated behind an **opt-in** flag
  ([`Parser::with_inline_tree`](../../parser/src/parser/parser.rs)); with it off (the
  default) the parse path is byte- and performance-identical to before, so all ~277 golden
  assertions pass unchanged. With it on, each block's tree is built by a **counter-safe
  second pass**: `SubstitutionGroup::apply` clones the parser *before* the authoritative
  pass advances any document counter, runs the recording pipeline on the clone, and parses
  the recorded markers into the tree — so footnotes, callouts, and `{counter:…}` values are
  numbered identically to the authoritative output (regression-tested). The
  [`inline_recorder`](../../parser/src/tests/inline_recorder.rs) differential corpus now
  drives the production module directly, so its byte-for-byte fold parity is a test of the
  shipped code.

  *Step 2 landed as (cross-reference resolution reaches the tree):* the tree now **carries
  resolution** for block-content cross-references (§4.3), the first of the two prerequisites
  the authoritative fold needs. When resolution runs
  ([`Content::resolve_references`](../../parser/src/content/content.rs)), each resolved
  destination is mirrored into the corresponding [`Ref`](../../parser/src/inlines/ref_node.rs)
  node of the tree — reusing the same `target`→destination decisions the rendered string
  reflects (not a second, independently-invoked resolution), and recursing into formatting
  spans and reference children. So a consumer that walks `inlines()` after a parse sees
  resolved `#id` destinations rather than the parse-time `resolved: None` state the tree is
  first built with. This is non-destructive and re-resolvable, exactly like the string path.
  Two resolution sites remained unmirrored after this step and were tracked as follow-ups:
  section- and block-title cross-references (owned by the separate document-order title pass,
  [`title_refs`](../../parser/src/document/title_refs.rs)) and footnote-embedded
  cross-references (which live in a footnote subtree the tree does not yet populate). The first
  is closed by step 3 below.

  *Step 3 landed as (title cross-reference resolution reaches the tree):* the document-order
  title pass ([`title_refs`](../../parser/src/document/title_refs.rs)) now **mirrors** the
  destinations it resolves for each section heading and block `.Title` into that title's own
  inline tree, closing the first of the two step-2 follow-ups. The title pass exists precisely
  because a title's cross-references need cross-title coordination (forward and circular
  references between headings) that the per-content pass cannot do; it computes each title's
  resolved references once, in document order, and — reusing the *same* resolved segments that
  produce the rendered title (not a second resolution) — installs them into the title tree via
  the same walk the block path uses. The tree-facing mirror is factored into one shared entry
  point ([`Content::mirror_tree_xref_resolution`](../../parser/src/content/content.rs), fed by
  the placeholder-ordered [`block_tree_xrefs`](../../parser/src/content/content.rs)) that
  both the block pass and the title pass call, so the two paths cannot drift. Block titles —
  which the per-content pass never resolved at all — now carry resolved tree destinations too.
  The remaining unmirrored site is footnote-embedded cross-references, which still await the
  footnote subtree the tree does not yet populate.

  *Step 4 landed as (the footnote subtree, and the cross-references inside it):* the tree now
  populates the **[`Footnote`](../../parser/src/inlines/footnote.rs) node's child subtree** and
  resolves the cross-references that live in it, closing the second — and last — of the two
  step-2 follow-ups, so every resolution site §4.3 names is now mirrored. A footnote's text is
  extracted out of the flow of the block during the macros substitution step (only its marker
  is left behind), so it never reaches the block's rendered string and the tree could not
  recover it from that string alone. The recording pass therefore also picks up the footnote
  texts *it* registered — snapshotting the registry length first, so it takes only this
  content's — and parses each into its node's subtree. Those texts carry the recorder's markers
  against the *same* event log (the recorder brackets a footnote's constructs there too), so a
  subtree is recovered by the very parse the block string uses, and a block's defining footnote
  nodes line up one-to-one, in document order, with the footnotes its pipeline defined. A bare
  reference (`footnote:id[]`) defines nothing and keeps the empty subtree its node type
  documents.

  The cross-reference mirror then follows that structure. A footnote-embedded reference is
  **re-homed out of the block template** when the footnote's text is extracted, so
  [`block_tree_xrefs`](../../parser/src/content/content.rs) — which filters *to* the
  placeholders the template still splices — already excludes it. Its exact complement,
  [`footnote_tree_xrefs`](../../parser/src/content/content.rs), collects those same re-homed
  segments in the order the footnote subtrees hold them, and
  [`Content::mirror_tree_xref_resolution`](../../parser/src/content/content.rs) installs the two
  lists into the two disjoint parts of the tree: the block walk no longer descends into a
  footnote subtree (doing so would consume block-level slots and shift every following
  reference onto the wrong destination), and the footnote walk installs the re-homed
  destinations there instead. Both the block pass and the title pass feed both lists, so a
  footnote in a section heading or a block `.Title` goes through the same seam. As in steps 2
  and 3 this reuses the resolution the string path already performed rather than invoking the
  resolver a second time, and it is non-destructive and re-resolvable. The differential corpus
  gains footnote-embedded fixtures and now folds each footnote's *text* under the same
  byte-for-byte invariant as block content, alongside a corpus-wide sweep asserting that
  turning the flag **on** leaves every rendered string unchanged.

  *Still remaining in Phase 2 (not in these steps):* making `rendered()` *authoritatively* a
  fold of the tree and **deleting** the three production sentinel systems. These are deferred
  because, with Strategy A, an authoritative fold would inherit the known
  formatted-section-title drift (§4.1) and require re-folding refs at resolution time; the
  design sequences the true single-artifact cutover with the single-pass builder in Phase 4.
  The opt-in flag retires with it.

- **Phase 3 — expose the public inline API.** 🔶 **In progress.** `Content::inlines()`,
  `IsBlock::inlines()`, the public node types, and `render_with`/`render_to`. Rename
  `rendered()` → `rendered_html()` and `rendered_content()` → `rendered_html_content()`
  (§3.3.1) — a mechanical sweep of the ~277 golden assertions that leaves every *asserted
  string* untouched. Resolution reports at node granularity.
  *Exit:* node vocabulary reviewed against the `asciidoctor` port's needs (§6.6); the
  purely-structural navigation sugar kept minimal pending a re-flow consumer; doc + README
  updated (the security section gets its `Raw`-node anchor).

  *Step 1 landed as (expose the read-only tree accessor):* the tree accessor
  [`Content::inlines`](../../parser/src/content/content.rs) — populated during Phase 2 but
  crate-internal until now — is **public**, and a parallel
  [`IsBlock::inlines`](../../parser/src/blocks/is_block.rs) is the block-level counterpart of
  [`IsBlock::rendered_content`](../../parser/src/blocks/is_block.rs): the same content-bearing
  blocks carry each. A content-bearing block returns `Some(tree)` — an *empty* tree when
  [`with_inline_tree`](../../parser/src/parser/parser.rs) is off, since the block still has
  content the tree simply was not built for — and a block with no directly-contained inline
  content (a compound/section block) returns `None`. The node types were already public
  (Phase 0); this step opens the accessor that reaches them, the core of #943. The larger
  pieces of the phase — the `rendered()` → `rendered_html()` rename and the
  `render_with`/`render_to` fold — remain as later steps.

  *Step 2 landed as (rename the rendered-string accessors for HTML-specificity):* the
  public string accessors are renamed to state their backend —
  [`Content::rendered`](../../parser/src/content/content.rs)`()` →
  [`Content::rendered_html`](../../parser/src/content/content.rs)`()` and
  [`IsBlock::rendered_content`](../../parser/src/blocks/is_block.rs)`()` →
  [`IsBlock::rendered_html_content`](../../parser/src/blocks/is_block.rs)`()` (§3.3.1). This
  is the mechanical sweep §5.3 describes: every one of the ~277 golden `.rendered()`
  assertions (and the corpus-wide differential harness) is rewritten to call the new name
  while the *asserted output strings* are left untouched, so the oracle still pins the same
  bytes. The rename is **name-only**: the accessor still returns exactly what it returned
  before, including the output of a custom
  [`InlineSubstitutionRenderer`](../../parser/src/parser/inline_substitution_renderer.rs)
  installed via
  [`with_inline_substitution_renderer`](../../parser/src/parser/parser.rs) — the two changes
  the new name ultimately implies (making `rendered_html()` a *fold* of the tree, and
  dropping the parse-time renderer so it is *always* the built-in HTML backend) are the
  deferred remainder of Phase 2 and Phase 4, unchanged by this step. The `render_with` /
  `render_to` fold is the last Phase 3 piece – but attempting it revealed that a *faithful*
  fold needs the per-construct attrlist/parser back at fold time, which the `'static`
  Strategy-A recorder cannot carry into a node (every `Styled`/`Image` node is therefore
  built with `attrs: None`). The self-describing nodes such a fold needs come from the
  Phase 4 single-pass builder, so `render_with` / `render_to` is **resequenced to land after**
  that builder covers the inline vocabulary (see Phase 4's step list).

- **Phase 4 — precision spans + single-pass builder + ASG output.** 🔶 **In progress.** Land
  the single-pass builder (Strategy B) so the tree is built **directly from `'src`** – nodes
  carrying honest per-node spans (#944) and their own `Attrlist<'src>` (self-describing) –
  then make `rendered_html()` a fold of the tree, add `Document::to_asg()`, validate against
  the ASG schema, and retire the `attribute-missing` per-line hack (#564).
  *Exit:* ASG output validates; #944 hard-case policies documented and tested; #564 hack
  removed.

  *Step 1 landed as (the single-pass builder foundation + `SpecialCharacters`):* a new
  [`inline_builder`](../../parser/src/content/inline_builder.rs) module recasts a
  substitution step as a **transducer** over a node list (`Vec<InlineNode> → Vec<InlineNode>`) –
  the shape §4.1 describes. [`build`](../../parser/src/content/inline_builder.rs) seeds one
  borrowed whole-source `Text` node and threads it through the steps;
  [`apply_special_characters`](../../parser/src/content/inline_builder.rs) splits each `Text`
  run on `<`/`>`/`&` into precise-span `Text` and `CharRef::Special` nodes, sliced with the
  crate's own [`Span`](../../parser/src/span/slice.rs) primitives so each node's
  `line`/`col`/`offset` is honest (the precise spans #944 targets) and verbatim runs borrow
  from `'src`. [`fold_html`](../../parser/src/content/inline_builder.rs) is the **first fold
  over the public `InlineNode` tree** – the recorder's `fold_into` folds an intermediate
  representation, not the public tree – and is the seed of both `rendered_html()`-as-a-fold
  and `render_with`. This step is **additive**: nothing is wired into the parse path, so the
  string pipeline and the Strategy-A [`inlines()`](../../parser/src/content/content.rs) tree
  are untouched, and a differential test asserts the fold reproduces the string pipeline's
  special-characters output byte-for-byte, alongside precise-span assertions the Strategy-A
  tree cannot make.

  *Step 2 landed as (`Quotes` → `Styled`, introducing nesting):*
  [`apply_quotes`](../../parser/src/content/inline_builder.rs) recasts the quoted-text step as
  a transducer: it reuses the *exact* recognition rules the string pipeline matches with –
  [`quote_subs`](../../parser/src/content/substitution_step.rs), now shared `pub(crate)` – so
  the recognition is unchanged and only the *sink* differs (§4.1). Each rule is applied to the
  node tree in order; before matching at a level the transducer descends into the
  [`Styled`](../../parser/src/inlines/styled.rs) spans earlier rules created, so a later rule
  can match *inside* an earlier span – which is what makes `*a _b_ c*` and `*a `b` c*` nest
  into a tree. Matching runs over an **escaped working string** rebuilt from the level's leaves
  (a `CharRef` contributes its canonical entity, so the boundary classes the patterns key off
  – `&`, `;` – see exactly what the string pipeline's escaped text presents; an earlier span is
  one opaque placeholder), and each match maps back to precise `'src` spans: delimiters are
  consumed, the boundary prefix is kept, and an attributed span (`[.role]#…#`) parses and
  **retains its own `Attrlist<'src>`** (self-describing – better than the recorder's
  `attrs: None`), so [`fold_html`](../../parser/src/content/inline_builder.rs) renders it
  through the same `render_quoted_substitution` the string step calls. A broad differential
  corpus asserts the fold reproduces the string pipeline's output through the quotes step
  byte-for-byte (nesting, unconstrained forms, smart quotes, super/subscript, roles/ids,
  escapes, specials adjacent to delimiters, multi-line runs), alongside structural precise-span
  assertions the Strategy-A tree cannot make. The one intended divergence – *crossed* delimiters
  (`` `a *b` c* ``) whose overlapping ranges the string pipeline renders as malformed,
  improperly-nested tags that no tree can represent – is documented and pinned by a test: the
  builder seals the inner delimiter inside its opaque span and stays well-formed. This step is
  **additive**: nothing is wired into the parse path.

  *Step 3 landed as (`CharacterReplacements` → `CharRef`, `PostReplacement` → `LineBreak`):*
  two more transducer steps.
  [`apply_character_replacements`](../../parser/src/content/inline_builder.rs) recognizes the
  typographic replacements – `(C)`/`(R)`/`(TM)`, em dashes, ellipsis, apostrophes, arrows, and
  restored entities – replacing each with a
  [`CharRef::Replacement`](../../parser/src/inlines/char_ref.rs) (logical character(s)) or
  [`CharRef::Entity`](../../parser/src/inlines/char_ref.rs) (a named/numeric entity as written)
  leaf, and [`apply_post_replacements`](../../parser/src/content/inline_builder.rs) turns a
  trailing ` +` at the end of a line into a
  [`LineBreak`](../../parser/src/inlines/inline_node.rs) leaf. Both reuse the string pipeline's
  *exact* recognition, now shared `pub(crate)`
  ([`character_replacements`](../../parser/src/content/substitution_step.rs) and
  [`hard_line_break_pattern`](../../parser/src/content/substitution_step.rs)), so only the
  *sink* differs (§4.1). Like the string step, they match over the level's **escaped** working
  string (reusing the quotes step's leaf-to-string machinery), which is precisely why an arrow
  (`-&gt;`, `&lt;-`) or a restored entity (`&amp;copy;`) can straddle a `Text`/`CharRef`
  boundary and still be recognized as one construct; a word character the pattern anchors on
  (the `w` in `w--`, the letters around a `w'w` apostrophe) stays outside the consumed range and
  is kept by the surrounding gaps, and an escape (`\(C)`) drops its backslash and wraps nothing.
  Each leaf is sliced back to a precise `'src` span. The fold reconstructs the replacement's
  [`CharacterReplacementType`](../../parser/src/parser/inline_substitution_renderer.rs) from its
  logical value (a bijection) and renders through the same `render_character_replacement` /
  `render_line_break` the string step calls, so its output is byte-identical. A broad
  differential corpus pins symbols, dashes, ellipsis, apostrophes, boundary-straddling arrows
  and entities, escapes, replacements inside spans, and hard line breaks (including their
  interaction with spans and replacements), alongside structural precise-span assertions the
  Strategy-A tree cannot make. This step is **additive**: nothing is wired into the parse path.
  The block-wide `hardbreaks` option (which needs the block's attribute list, not yet threaded
  into the builder) is deferred to the cutover (step 6).

  *Step 4a landed as (`Macros` → `Image`, the first macro family):* the macros step is by far
  the largest – seven construct families plus deferred-xref recording and footnote text
  extraction – so it lands as its own sequence of sub-steps rather than one leap. The first,
  [`apply_macros`](../../parser/src/content/inline_builder.rs), recognizes **image and icon
  macros** (`image:target[…]`, `icon:target[…]`), replacing each with an
  [`Image`](../../parser/src/inlines/image.rs) node that captures its own owned
  `Attrlist<'src>` – the step that makes a macro node **self-describing**, the property a
  faithful fold needs (§3.3.1, Phase 3 step 2). It reuses the string pipeline's *exact*
  recognition, now shared `pub(crate)`
  ([`INLINE_IMAGE_MACRO`](../../parser/src/content/macros.rs)), so only the *sink* differs
  (§4.1); it descends into `Styled`/`Ref` children (a macro can sit inside a rendered span),
  pre-extracts the alt/width/height (`icon:` carries a `size`, read back from `attrs` at fold
  time) the way the string replacer does, and honors the `\image:` escape. An `is_icon` flag is
  added to the `Image` node so the two forms fold through the right renderer method. The
  [`fold_html`](../../parser/src/content/inline_builder.rs) fold gains a `&Parser` argument –
  rendering an image reads the document's safe mode, `data-uri`, and `icons`/`icontype`
  attributes – and reconstructs `ImageRenderParams`/`IconRenderParams` to call the same
  `render_image`/`render_icon`, so its output is byte-identical (differential corpora pin this
  under both the default document and one with `imagesdir`/`icons` set). Because the string
  pipeline matches macros over *escaped, already-rendered* text, a macro whose target or
  attribute list contains a special character (`< > &`) or a rendered span cannot be carried as
  an `'src` slice; such a macro is left **unrecognized** for a later increment (the
  attribute-references step and the cutover), a documented boundary pinned by a test, exactly as
  the quotes step documents crossed delimiters. The additive builder deliberately performs *no*
  recognition side effect (no `register_image` in the asset catalog, no dangerous-`link=` scheme
  warning); the cutover (step 6) must re-attach those. This step is **additive**: nothing is
  wired into the parse path. The remaining macro families (`Ref`, `Footnote`, `Ui`,
  `IndexTerm`, `Stem`, `Anchor`) are later sub-steps.

  *Step 4b(i) landed as (`Macros` → `Ui`, the UI-macro family):*
  [`apply_macros`](../../parser/src/content/inline_builder.rs) now also recognizes the **UI
  macros** – keyboard (`kbd:[…]`), button (`btn:[…]`), and menu (`menu:…[…]`) – replacing each
  with a [`Ui`](../../parser/src/inlines/ui.rs) node that carries the split keys / normalized
  label / menu path the string replacer computes. It reuses the string pipeline's *exact*
  recognition and splitting, now shared `pub(crate)`
  ([`INLINE_KBD_BTN_MACRO`](../../parser/src/content/macros.rs),
  [`INLINE_MENU_MACRO`](../../parser/src/content/macros.rs), `split_kbd_keys`,
  `normalize_index_text`), so only the *sink* differs (§4.1). Like the string step it runs the
  families **in order** – keyboard/button, then menu, then image/icon – and recognizes the UI
  macros only under the `experimental` document attribute, mirroring the gate exactly (with it
  off a `kbd:[…]` stays literal, in the tree as in the string). The
  [`fold_html`](../../parser/src/content/inline_builder.rs) fold reconstructs the render
  parameters from the node and calls the same `render_keyboard`/`render_button`/`render_menu`
  the string step calls (a menu reads the document's `icons` attribute for its caret), so its
  output is byte-identical (a differential corpus pins it under `experimental`, and a companion
  test pins that the macros stay literal *without* it). The image increment's match/rebuild
  plumbing is generalized into a shared `MacroMatch`/`rebuild_macro_level` seam the families
  now share. The one intended divergence is the `&gt;`-submenu form
  (`menu:View[Zoom > Reset]`): its `>` is always an escaped `CharRef` by the time macros run, so
  it fails the verbatim boundary and is left **unrecognized** for a later increment – documented
  and pinned by a test, exactly as step 4a defers a macro over a special character (the
  comma-delimited and bare/single-item menu forms *are* verbatim and covered). This step is
  **additive**: nothing is wired into the parse path. The remaining macro families (`Ref`,
  `Footnote`, `IndexTerm`, `Stem`, `Anchor`) are later sub-steps.

  *Step 4b(ii) landed as (`Macros` → `Ref{Link}`, the link half of the `Ref` family), in two
  parts:* the builder now recognizes every **link** form the recorder covers, each as a
  [`Ref`](../../parser/src/inlines/ref_node.rs)`{Link}` node whose fold routes through the same
  `render_link` the string step calls, so its output is byte-for-byte identical. **Part 1** added
  the explicit `link:`/`mailto:` macro (`INLINE_LINK_MACRO`), introducing the `Ref` node into the
  builder. **Part 2** added **auto-links** (a bare `https://example.org`) and **formal-URL links**
  (`https://example.org[text]`), matched by the shared `INLINE_LINK` pattern (now `pub(crate)`,
  its branch-resolving `NormalizedCaps` view shared with the string replacer so the two cannot
  drift on group numbering). Both reuse the string pipeline's *exact* recognition, changing only
  the *sink*, and neither adds a field to `Ref`: the computed display text is baked into a single
  [`Text`](../../parser/src/inlines/inline_node.rs) child (so the fold recovers `link_text` with no
  build-time state), the `bare` role rides on the node's `roles`, and a `^` suffix sets `window`.
  The auto-link part reproduces the replacer's boundary-prefix preservation and bare-URL
  trailing-punctuation stripping by generalizing the shared `MacroMatch` seam so a macro node can
  replace only a *sub-range* of its match – keeping a kept prefix before it and stripped
  punctuation after it – which the image/UI/`link:` families use degenerately (the node consumes
  the whole match). Because macros are matched over *escaped, already-rendered* text, a link whose
  URL crosses a special (`&`) or a rendered span is left **unrecognized** for a later increment,
  exactly the verbatim boundary step 4a documents; the angle-bracketed URL form (`<url>`) needs a
  leading `&lt;` and so is always non-verbatim, and a text carrying an attribute list (`=`, or a
  `mailto:` `,` subject) is deferred until the node can hold an `Attrlist<'src>`. The `link:` URL
  macro form (`link:https://…[…]`) is left to the `INLINE_LINK_MACRO` pass, which folds the
  identical node – the two passes run in the string step's order (`INLINE_LINK` before
  `INLINE_LINK_MACRO`). Differential corpora pin the verbatim link forms (labeled / bare / pathed,
  `mailto:`, other schemes, the `^` suffix, escapes, `hide-uri-scheme`, links next to and inside
  spans) byte-for-byte, alongside structural precise-span assertions the Strategy-A tree cannot
  make and divergence tests for each deferred form. These steps are **additive**: nothing is wired
  into the parse path. Cross-references (`Ref{Xref}`), footnotes, index terms, STEM, and anchors
  remain later sub-steps.

  *Follow-up landed as (the link family's own attribute-list-bearing display text, closing
  `Ref{Link}`'s last deferred form):* both link-recognizing passes now recognize a display text
  carrying an `=` (`link:x[text,role=hl]`, `https://x[text,role=hl]`) and a `mailto:` text
  carrying a `,` subject/body (`mailto:x[Team,Hello there]`) – the pair step 4b(ii) itself defers,
  and the mirror of the `xref:` macro's own attribute-list text (part 3c below). Unlike that xref
  form, a link's attribute list cannot be reduced to a few plain fields: `render_link` reads an
  `id`, a `title`, and the `nofollow`/`noopener` options straight off the `Attrlist` itself (not
  just `roles`/`window`, which `XrefRenderParams` alone needs), so `Ref` gains an
  `attrs: Option<Attrlist<'src>>` field – `None` unless a `Link`'s display text carried its own
  attribute list, always `None` for an `Xref`. This is also what step 4b(ii)'s own deferral notes
  as the blocker ("deferred until the node can hold an `Attrlist<'src>`"): the string replacers
  parse the attrlist from a *newline-normalized copy* of the text (so a multi-line
  `link[Foo\nBar,role=x]` reads as `Foo Bar`), which cannot become an honestly-borrowed
  `Attrlist<'src>` – but when the text has **no embedded newline**, that copy is byte-identical to
  the bracketed text's own `'src` slice, so the node can parse the *real* source span instead and
  carry a genuine borrow. A text that does embed a newline still needs the synthesized copy the
  node cannot hold, so that one narrow form remains deferred (pinned by its own divergence test,
  for both the `=` and `,` cases). Both call sites reuse
  [`extract_attributes_from_text`](../../parser/src/content/macros.rs) (now shared `pub(crate)`,
  its own signature relaxed from `&'src Span<'src>` to `Span<'src>` since `Span` is `Copy` and the
  reference added nothing the by-value form doesn't already give a caller building a node with the
  *same* `'src` as its input) and
  [`encode_uri_component`](../../parser/src/content/macros.rs) (likewise now shared `pub(crate)`,
  for the `mailto:` subject/body encoding), so the interpretation – including the "incidental `=`"
  fallback (`extract_attributes_from_text`'s own guard) and, for the `link:`/`mailto:` macro form,
  the exact unconditional-adoption behavior `InlineLinkMacroReplacer` has and
  `InlineLinkReplacer` does not (see the two call sites' own doc comments) – is reused byte-for-byte
  rather than re-derived. [`fold_link`](../../parser/src/content/inline_builder/fold.rs) now passes
  the node's own `attrs` through to `render_link` when present, falling back to the empty attrlist
  every other link already folds through. A display or reference text crossing a rendered span
  (`link:x[*bold*]`, `xref:id[*bold*]`, `<<id,*bold*>>`) remains a **separate**, still-fully-open
  boundary for every reference-bearing family (not touched by this follow-up): by the time macros
  run, `*bold*` is already a `Styled` node, not text, so recognizing it would need the node's
  display text to become structured children – the shape a footnote's own content already has –
  which no reference family has yet grown; each now carries its own divergence test pinning this
  (previously only the `<<id,*bold*>>` shorthand had one). Differential corpora extend the existing
  link/formal-URL fixtures with `role=`/multi-attribute and mailto subject/body combinations,
  alongside the incidental-`=` case and the multi-line divergence.

  *Step 4b(ii) part 3a landed as (`Macros` → `Ref{Xref}`, the same-document `xref:` macro form):*
  the builder now recognizes the **`xref:` cross-reference macro** (`xref:id[]`,
  `xref:id[Reference Text]`), each as a [`Ref`](../../parser/src/inlines/ref_node.rs)`{Xref}` node.
  It reuses the string pipeline's *exact* recognition – `INLINE_XREF` is now shared `pub(crate)` –
  so only the recognition *sink* changes (a node instead of the string step's deferred
  `XrefSegment` placeholder). As with links, no field is added to `Ref`: the provided text is baked
  into a single [`Text`](../../parser/src/inlines/inline_node.rs) child (verbatim text borrows from
  `'src`; an escaped `\]` synthesizes an owned value), and an empty text yields no children, which
  the fold reads as "no text provided" (the bracketed `[id]` fallback). The
  [`fold_html`](../../parser/src/content/inline_builder.rs) fold reconstructs `XrefRenderParams`
  from the node and routes through the same `render_xref` the string path feeds at resolution time,
  so its output is byte-for-byte identical – pinned by a new differential corpus that finalizes the
  string pipeline's deferred cross-references to the same **unresolved** fallback the additive
  builder (no resolution pass) produces. Because the additive builder never resolves, the fold
  always takes `render_xref`'s unresolved branch, where `xrefstyle` and a *derived* destination play
  no part; wiring resolution to the tree is the cutover's job (step 6). Four forms are deferred,
  each documented and pinned by a divergence test: the **shorthand** (`<<id>>`, always non-verbatim
  because its `&lt;`/`&gt;` delimiters are `CharRef`s by macro time, exactly as the angle-bracketed
  `<url>` link defers), an **inter-document** target (`xref:other.adoc#frag[]`, whose derived
  destination the node cannot carry yet), a **text carrying an attribute list** (`=`, parsed into
  `window`/`role`/`xrefstyle`, deferred until the node can hold an `Attrlist<'src>` – as the
  formal-URL link defers the same), and a macro whose **target or text crosses a special character
  or a rendered span** (`xref:foo[a<b]`, matched by the string pipeline over the *escaped* text,
  which a self-describing node cannot carry as an `'src` slice – the same verbatim boundary the
  image and auto-link increments document). This step is **additive**: nothing is wired into the
  parse path.

  *Step 4b(ii) part 3b landed as (`Macros` → `Ref{Xref}`, the same-document `<<id>>` shorthand):*
  the builder now recognizes the **shorthand cross-reference** (`<<id>>`, `<<id,Reference Text>>`)
  as the same [`Ref`](../../parser/src/inlines/ref_node.rs)`{Xref}` node the `xref:` macro form
  produces, folding through the identical `render_xref` so the output is byte-for-byte identical
  (pinned by extending the part-3a differential corpus with shorthand fixtures). It reuses the
  *same* shared `INLINE_XREF` pattern – the shorthand and macro forms are two branches of one
  regex – so recognition is unchanged and only the *sink* differs. The shorthand's key wrinkle is
  that, because special characters run before macros, its `<<`/`>>` delimiters are already escaped
  [`CharRef`](../../parser/src/inlines/char_ref.rs)s (`&lt;&lt;`/`&gt;&gt;`) by macro time, so the
  match is never *wholly* verbatim the way a `xref:` macro is. The recognizer therefore relaxes its
  verbatim gate for this one form: the delimiters are `CharRef`s the node **consumes** (dropped by
  [`rebuild_macro_level`](../../parser/src/content/inline_builder.rs), which emits atomic pieces
  whole) rather than slices, so only the shorthand's *inner* text – the id and any reference text –
  need be verbatim to slice from `'src`. The inner is split on the first `,` into an id and an
  optional reference text, each **trimmed** exactly as the string replacer's shorthand branch does;
  the trimmed text becomes the node's single [`Text`](../../parser/src/inlines/inline_node.rs) child
  (borrowing `'src`), an absent one yields no children (the bracketed `[id]` fallback), and the whole
  `<<…>>` is the node's `location`. An escaped `\<<…>>` drops its backslash and stays literal, handled
  before the verbatim gate so it works even across a non-verbatim inner. Four forms are deferred, each
  documented and pinned by a divergence test: an **inter-document** shorthand (`<<other#frag>>`, whose
  derived destination the node cannot carry yet – the same block as the inter-document `xref:` form),
  a **document-as-a-whole** shorthand (`<<>>`, an empty id resolving through a derived destination, as
  `xref:#[]` defers), a **`<<id,>>` with an empty reference text** (which the string replacer records
  as a *present-but-empty* text – an empty `<a>…</a>` – that an empty child vector cannot distinguish
  from "no text provided"), and a shorthand whose **id or text crosses a special character or a
  rendered span** (non-verbatim inner, the same boundary the macro form and the image/auto-link
  increments document; this also subsumes the string replacer's "id already contains rendered markup"
  guard, since such an id is non-verbatim). This step is **additive**: nothing is wired into the parse
  path. The two node-blocked forms both spellings share – inter-document targets and attribute-list
  text – remain for a later increment (part 3c), which needs new `Ref` fields pinned against a
  consumer.

  *Step 4b(ii) part 3c landed as (`Ref` grows a `derived` field, closing the inter-document half):*
  part 3c turned out to split cleanly along its own two deferred forms – an inter-document/
  document-as-a-whole target needs only a *destination* the node can carry opaquely (the exact
  `DerivedReference` type [`ResolutionContext`](../../parser/src/parser/reference_resolver.rs)
  already produces for the string pipeline's own non-catalog case), while an attribute-list-bearing
  display text needs the node to parse and hold `window`/`role`/`xrefstyle`. The first needed no
  "consumer" to pin its shape – it is a straight reuse of an existing, already-public type – so it
  lands now; the second (part 3c's attribute-list half) remains deferred, still needing a real
  consumer to pin how (or whether) `Ref` grows an `Attrlist<'src>`.
  [`Ref`](../../parser/src/inlines/ref_node.rs) gains `derived: Option<DerivedReference>` –
  `None` for a same-document reference (unchanged: still resolves through the catalog at the
  cutover) and `always None` for a [`Link`](../../parser/src/inlines/ref_node.rs), populated only
  for a cross-reference whose target carries its own destination. A new
  `xref_target_and_derived` helper in
  [`xref.rs`](../../parser/src/content/inline_builder/macros/xref.rs) mirrors
  `InlineXrefReplacer::replace_append`'s own target-interpretation match *exactly* – including its
  "a target naming this document, or a file included into it in full, is a same-document reference
  after all" special case (`Parser::docname`/`Parser::catalog_include_is_full`) – so both the
  `xref:` macro form and the `<<id>>` shorthand (which shares the helper, differing only in
  `macro_form`) now recognize every target shape: a same-document id, an inter-document target
  (`xref:other.adoc#frag[]`, `<<other#frag>>`), and the document-as-a-whole form (`xref:#[]`,
  `<<>>`). [`fold_xref`](../../parser/src/content/inline_builder/fold.rs) reconstructs
  `XrefRenderParams.derived` straight from the node, so a derived-carrying reference now folds
  through `render_xref`'s `(None, Some(derived))` branch instead of the unresolved fallback,
  byte-for-byte identical to the string pipeline (`xrefstyle` stays `None` – it plays no part in
  that branch, and the attribute-list half that would populate it for other cases is still
  deferred). As throughout this module, this performs *no* recognition side effect and nothing is
  wired into the parse path. A differential corpus extends the existing xref fixtures with
  inter-document (with and without a fragment, and a non-AsciiDoc extension kept as-is) and
  document-as-a-whole forms in both spellings, alongside a unit test pinning the
  "target names this document" special case. The one remaining deferred xref form – a text
  carrying an attribute list (part 3c's other half) – is unchanged, still pinned by its own
  divergence test.

  *Step 4b(ii) part 3c (attribute-list half) landed as (`Ref` grows `xrefstyle`, closing the last
  deferred xref form):* the `xref:` macro's own remaining deferred form – a bracketed text
  carrying an attribute list (an `=`, for `window`/`role`/`xrefstyle`) – is now recognized.
  [`xref_macro_text`](../../parser/src/content/inline_builder/macros/xref.rs) mirrors
  `InlineXrefReplacer::replace_append`'s own text interpretation exactly: it parses the text – from
  a newline-normalized copy, since the parse is not necessarily verbatim, exactly as the string
  replacer parses that same normalized copy rather than a source slice – as an
  [`Attrlist`](../../parser/src/attributes/attrlist.rs) whose first positional attribute becomes the
  display text and whose `window`/`role` named attributes populate the node's existing fields; a new
  [`Ref::xrefstyle`](../../parser/src/inlines/ref_node.rs) field (`Option<XrefStyle>`) carries a
  `xrefstyle=` override. As with the `<<other#frag>>` half, this needed no consumer to pin its
  shape – `window`/`roles` already existed as plain fields (not an `Attrlist<'src>`) because
  [`XrefRenderParams`](../../parser/src/parser/inline_substitution_renderer.rs) itself takes them
  that way, not a borrowed attribute list, so the node stores exactly what the fold already needs.
  When the attrlist parse finds no named attribute – the sole positional value is the whole
  normalized text – the `=` was incidental (mirroring Asciidoctor's own
  `extract_attributes_from_text` fallback); the text is then used as plain display text with no
  named attributes, exactly as if it carried no `=` at all. The parsed positional text becomes a
  *synthesized* `Text` child (no `'src` slice of its own, since it comes from the normalized,
  attrlist-parsed copy) whose location falls back to the bracketed text's own span (design §4.4),
  the same synthesized-value policy `apply_attribute_references` (step 5b) already established.

  Landing this also closed a latent gap the fold carried since the cross-reference increment first
  landed (part 3a): the *document-wide* `xrefstyle` attribute – which applies to every reference,
  not only one carrying its own `xrefstyle=` override – was never applied at all, because
  [`fold_xref`](../../parser/src/content/inline_builder/fold.rs) hard-coded `xrefstyle: None`. It now
  combines the node's own override with the document-wide default exactly as
  `InlineXrefReplacer` does (`xrefstyle_override.or_else(|| document_xrefstyle(parser))`), reusing
  the string pipeline's own [`document_xrefstyle`](../../parser/src/content/macros.rs) helper (now
  shared `pub(crate)`) rather than a second implementation. The `<<id>>` shorthand has no
  attribute-list text of its own – its node's `xrefstyle` override is always `None` – but still
  observes the document-wide default through this same fold-time combination, closing the sub-step
  in full: every form the design names for part 3c is now recognized. A differential corpus extends
  the existing xref fixtures with `role=`/`window=`/`xrefstyle=` combinations, a positional-text-free
  attribute list, and the incidental-`=` case (verified reachable in the builder's own verbatim-gated
  matching, unlike the nested-macro-substitution route the string pipeline's own incidental case
  takes); hand-built-node tests in `fold.rs` pin the document-wide-default and override-precedence
  behavior directly, since neither is observable through an unresolved fold (the tree does not yet
  reach catalog resolution – that remains step 6's job).

  *Step 4b(ii) part 4a landed as (`Macros` → `Anchor`, inline anchors):* the builder now recognizes
  **inline anchors** in both spellings – the `[[id]]` / `[[id,reftext]]` shorthand and the
  `anchor:id[reftext]` macro – as an [`Anchor`](../../parser/src/inlines/anchor.rs) node, folding
  through the same `render_anchor` the string step calls so the output is byte-for-byte identical
  (pinned by a new differential corpus). It reuses the string pipeline's *exact* recognition –
  [`INLINE_ANCHOR`](../../parser/src/content/macros.rs) is now shared `pub(crate)` – so only the
  recognition *sink* changes (a node instead of rendered markup). Taken out of pipeline order (it is
  listed last under part 4) precisely because it is the cleanest of the remaining families: an
  anchor's rendering (`<a id="…"></a>`) is a function of its **id alone**, and the pattern admits no
  special character in an id, so an id is always verbatim and an anchor is **always recognized** –
  there is no deferred-output boundary the way the link and cross-reference families have one. The id
  borrows from `'src` and the whole `[[…]]` / `anchor:…[…]` is the node's `location`. An escaped
  `\[[…` / `\anchor:…` drops its backslash and stays literal, and – because the id is always verbatim
  while a reference text may not be – the unescape needs no verbatim gate at all.

  The optional reference text becomes the node's `reftext` – a single
  [`Text`](../../parser/src/inlines/inline_node.rs) child – **when it is verbatim** (borrowing
  `'src`; a shorthand's trailing whitespace is trimmed and a macro's escaped `\]` is unescaped into an
  owned value, mirroring the string replacer). A reference text carrying a rendered span or an escaped
  special is *non-verbatim*; because it never reaches the flow (the anchor renders from its id alone),
  such an anchor is still recognized and rendered – the whole match, the rendered-span reference text
  included, is **consumed** by the node – but its `reftext` is left `None` rather than sliced wrongly
  from `'src`. This is the same verbatim boundary the other macro families document, expressed here as
  a node field the fold ignores rather than as a deferred construct; the field stays provisional
  pending a re-flow consumer (design §6.6). As the additive builder does throughout, the anchor pass
  performs *no* recognition side effect – it does **not** `register_ref` the id in the reference
  catalog (so a cross-reference can resolve against it), nor raise the duplicate-id warning the string
  replacer does; those, and the bibliography-anchor form (`[[[id]]]`, which the string step recognizes
  only inside a bibliography list item – a context the additive builder is not wired into), are the
  cutover's job (step 6). This step is **additive**: nothing is wired into the parse path. The
  remaining macro families (`Footnote`, `IndexTerm`, `Stem`) and the node-blocked cross-reference
  forms (part 3c) are later sub-steps.

  *Follow-up landed as (an anchor id inside an expanded attribute value, a latent correctness gap):*
  an audit of every macro family's own verbatim gate, prompted by the design's own "a macro inside an
  expanded value" boundary (§3.4.1, §4.1's `apply_macros` note) still being open for this family after
  step 5b closed it for `CharacterReplacements`, surfaced that part 4a's own claim – "an id is always
  verbatim … an anchor is *always* recognized" – was true only against the escaped-special/rendered-span
  boundary every other macro family documents, not against the *synthesized* one: unlike every other
  family, [`find_anchor_matches`] never checked the id capture against [`range_is_verbatim`] before
  slicing it, so an attribute reference whose expanded value happened to contain `[[id]]` (e.g.
  `:myattr: [[custom-id]]` then `{myattr}`)
  built an [`Anchor`](../../parser/src/inlines/anchor.rs) node whose `id`/`location` silently fell back
  to the *enclosing synthesized run's* coarse span (`{myattr}` itself) rather than the real id text –
  a wrong node, not a documented divergence, though unreachable from any real parse today since this
  module is not yet wired in (§5.2 Phase 4 step 6). [`build_anchor_node`] now checks the id's own range
  with the same [`range_is_verbatim`] every other family already uses and returns `None` when it fails,
  leaving the anchor unrecognized for a later increment exactly like every other family's own boundary –
  closing the gap between the doc comment's claim and the code. A new divergence test pins the exact
  scenario that exposed it (an attribute expanding to `[[custom-id]]`), alongside the golden pipeline's
  own confirmation that it *does* recognize the anchor once the value is spliced in, so a future
  boundary-lifting increment that fixes this for real has a corpus fixture ready to move out of the
  divergence test and into a parity one.

  [`find_anchor_matches`]: ../../parser/src/content/inline_builder/macros/anchors.rs
  [`range_is_verbatim`]: ../../parser/src/content/inline_builder/macros/image.rs

  *Step 4b(ii) part 4b landed as (`Macros` → `IndexTerm`, index terms):* the builder now recognizes
  **index terms** in both spellings – the `((term))` / `(((primary, secondary, tertiary)))` shorthand
  and the `indexterm:[…]` (concealed) / `indexterm2:[…]` (flow) macro – as an
  [`IndexTerm`](../../parser/src/inlines/index_term.rs) node, folding through the same
  `render_index_term` the string step calls so the output is byte-for-byte identical (pinned by a new
  differential corpus). It reuses the string pipeline's *exact* recognition –
  [`INLINE_INDEXTERM`](../../parser/src/content/macros.rs) is now shared `pub(crate)`, alongside the
  `strip_see_and_seealso` helper – so only the recognition *sink* changes. Like the anchor increment,
  a **concealed** term renders to nothing (a function of no shown text), so it is recognized regardless
  of what its argument crosses and its node carries an empty `terms`; a **visible** term shows its
  text in the flow and is recognized whenever that text – reconstructed from the level's escaped match
  string, so a `CharRef` entity or a stripped `see`/`see-also` clause (`&gt;&gt;` / `&amp;&gt;` by
  macro time) is handled as parity, not a divergence – crosses no opaque span. The node mirrors the
  Strategy-A recorder's shape (the shown term for a visible node, empty for a concealed one), leaving
  the richer primary/secondary/tertiary structure to a re-flow consumer to pin (the field is
  provisional, per the node's Phase-0 note). The shorthand reproduces the string replacer's
  trailing-`)` absorption (its `(?!\))` look-ahead re-creation) by folding the absorbed parens into
  the match, and keeps a literal parenthesis adjacent to a term via the shared `MacroMatch` sub-range
  seam. One subtle string-pipeline behavior is reproduced exactly: a level of *only* concealed
  shorthand terms (`(((coffee)))`, `(((a)))(((b)))`) accumulates no output and ends in a look-ahead
  retry, so [`replace_with_lookahead`](../../parser/src/internal/regex.rs) returns `Cow::Borrowed` and
  the string step leaves it **literal** – the builder detects that no-op and mirrors it. Two forms are
  deferred, each documented and pinned by a divergence test: a **visible term crossing a rendered
  span** (unreconstructable from the escaped string, the same verbatim boundary the other macro
  families document) and an **`indexterm2:[…]` carrying an attribute list** (an `=`, deferred until
  the node can hold an `Attrlist<'src>`, as the link/xref macros defer the same); the one escaped
  paren-wrapped shorthand the string replacer re-renders (`\(((x)))` → `(x)`) is likewise left literal.
  As throughout the additive builder, this performs *no* recognition side effect (the HTML backend
  builds no index, so the string replacer has none to skip either). This step is **additive**: nothing
  is wired into the parse path. Inline `Stem` is handled at passthrough time (step 5), not in the
  macros step.

  *Step 4b(ii) part 4c landed as (`Macros` → `Footnote`, the last macro family):* the builder now
  recognizes **footnotes** (`footnote:[…]`, `footnote:id[…]`, `footnote:id[]`) as a
  [`Footnote`](../../parser/src/inlines/footnote.rs) node, folding through the same `render_footnote`
  the string step calls so the output is byte-for-byte identical (pinned by a new differential
  corpus). It reuses the string pipeline's *exact* recognition –
  [`INLINE_FOOTNOTE_MACRO`](../../parser/src/content/macros.rs) is now shared `pub(crate)`, alongside
  the `normalize_footnote_text` helper – so only the recognition *sink* changes, and runs **last** in
  `apply_macros`, after cross-references, mirroring the string step's order exactly: a footnote's text
  is extracted from the flow, so any construct an earlier pass at this same level already recognized
  (an image, a link, an anchor, an index term, or now a cross-reference) is captured as *that
  construct's node* rather than being re-recognized from its source text.

  Two things set this increment apart from every prior macro family:

  - **Structured content, not a literal value.** A footnote's bracket content becomes the node's
    `children` via [`emit_range`](../../parser/src/content/inline_builder.rs) rather than a literal
    `'src` slice gated by [`range_is_verbatim`](../../parser/src/content/inline_builder.rs) the way a
    target or display text is elsewhere. A content range crossing an already-recognized construct is
    therefore *not* a boundary to defer on – nesting is the point, and `emit_range` clones that
    construct's node whole into the footnote's subtree, exactly mirroring how the string pipeline's
    footnote text captures an already-substituted macro verbatim.
  - **One *required* recognition side effect.** Every prior macro family performs *no* recognition
    side effect (no catalog registration, no warning), deferring that to the cutover (step 6) because
    omitting it does not change the fold's output bytes. A footnote's marker digits *are* the assigned
    footnote number, so this pass must call `Parser::footnote_index_for_id` / `Parser::define_footnote`
    – the same document-counter-advancing calls the string replacer makes – or the differential corpus
    could never pass. The two code paths never share a `Parser` (each independently numbers footnotes
    over the same source in the same left-to-right order), so this never double-counts a registration.
    The registered catalog `text` is a best-effort normalized rendering of the raw bracket content, not
    a fold (building the tree must not itself invoke a renderer), so – like every other deferred
    registration in this module – a tree-built footnote's `Document::catalog().footnotes()` entry is
    not yet byte-faithful; only the returned *number* is relied on.

  Two forms are deferred, each documented and pinned by a divergence test: the deprecated
  `footnoteref:[id,text]` / `footnoteref:[id]` form (which packs its id and text into one bracket, split
  differently, and – outside `compat-mode` – raises a deprecation warning neither of which this
  increment implements), and content carrying an escaped closing bracket (`\]`, which would need
  splicing a literal `]` into the middle of a `Text` piece the content range slices – a rebuild this
  increment does not attempt). This step is **additive**: nothing is wired into the parse path. With it,
  every macro family the recorder covers now has a single-pass counterpart.

  *Follow-up landed as (the deprecated `footnoteref:` form):* the first of part 4c's two deferred forms
  is closed. [`build_footnoteref_node`](../../parser/src/content/inline_builder/footnotes.rs) mirrors
  `InlineFootnoteMacroReplacer`'s own `raw.split_once(',')` exactly – `footnoteref:[id,text]` /
  `footnoteref:[id]` packs both into one bracket rather than taking the id from the macro target the way
  `footnote:id[…]` does, splitting on the *first* comma so a bracket with no comma is an id-only bare
  reference and a trailing comma (`footnoteref:[id,]`) yields *empty*, not absent, content (a defining
  occurrence with empty text) – a distinct shape from `footnote:id[]`'s own no-comma-at-all reference.
  Once split, the (id, content) pair resolves through the *same* three cases `build_footnote_node`
  already does (reuse an already-defined id's number, define a new id-carrying occurrence, or fall back
  to an unresolved reference for an id never defined), folding through the identical `render_footnote`,
  so the output is byte-for-byte identical to the golden pipeline's (pinned by extending the part 4c
  differential corpus with `footnoteref:` fixtures). The escape check
  (`whole.as_str().starts_with('\\')`) is hoisted ahead of the ref-vs-plain branch in
  `find_footnote_matches`, mirroring the string replacer's own check order exactly – it previously ran
  *after* the (then early-`continue`ing) `footnoteref:` branch, so an escaped `\footnoteref:[…]` was left
  fully unrecognized (backslash and all) rather than unescaped; this is fixed as a side effect of
  recognizing the form at all. As with every other macro family, the one side effect this increment does
  *not* yet perform is the deprecation warning itself (`DeprecatedFootnoterefMacro`) – a diagnostic that
  does not change the fold's output bytes, so – unlike the footnote number, which does – it remains
  deferred to the cutover (step 6) like every other family's own catalog/warning side effect. The one
  remaining deferred form from part 4c's own list, content carrying an escaped closing bracket (`\]`), is
  unchanged and applies identically to `footnoteref:`'s own bracket content (pinned by its own divergence
  test).

  *Step 5a landed as (Passthroughs → `Raw`, the delimited forms):* a new
  [`apply_passthroughs`](../../parser/src/content/inline_builder.rs) step – the **first** step
  [`build`](../../parser/src/content/inline_builder.rs) runs, ahead of `SpecialCharacters` –
  recognizes the triple-plus (`+++…+++`), double-plus (`++…++`), double-dollar (`$$…$$`), and bare
  `pass:[…]` macro (no explicit substitution list) as [`Raw`](../../parser/src/inlines/inline_node.rs)
  leaves, mirroring
  [`Passthroughs::extract_from`](../../parser/src/content/passthroughs.rs), which the string pipeline
  runs *before* its own step loop – so a passthrough's content is never touched by specialcharacters,
  quotes, replacements, or macros: it is a leaf, and every later step's match-string builder already
  treats an unrecognized node kind as one opaque placeholder, exactly as it already does for an
  earlier-built `Styled` span. It reuses the string pipeline's *exact* recognition –
  [`INLINE_PASS_MACRO`](../../parser/src/content/passthroughs.rs) is now shared `pub(crate)` – so only
  the recognition *sink* differs (§4.1). The triple-plus and bare `pass:[…]` forms resolve to
  `SubstitutionGroup::None` (nothing applies), so their content borrows `'src` directly (a `pass:[…]`
  body unescapes an escaped `\]`, as every other macro family's bracket content does, which makes the
  unescaped case owned instead); the double-plus and double-dollar forms resolve to
  `SubstitutionGroup::Verbatim` (special characters only) and are run through the real substitution
  pipeline rather than hand-escaped, so a custom `InlineSubstitutionRenderer`'s escaping is honored –
  the cost is an owned `Raw` value instead of a borrow.

  Three forms are deferred, each documented and pinned by a divergence test: an
  **attribute-list-prefixed** passthrough (`[quotes]++text++`, `[x-]\`text\``, `[attrs]+text+`), a
  **`pass:` macro carrying an explicit substitution list** (`pass:c,q[…]`, whose content would need a
  richer subtree than a single `Raw` leaf can hold – the same reason a footnote's content is
  structured children rather than a literal value), and the **bare unconstrained form** (`+text+`,
  matched by `INLINE_PASS` rather than `INLINE_PASS_MACRO` – its "must not follow a word" boundary
  needs a lookbehind Rust's regex engine cannot express, which the string replacer works around with a
  retry loop this increment does not reproduce). That same deferred boundary shows up once more,
  indirectly: an **escaped triple- or double-plus** (`\+++text+++`, `\++text++`) drops its backslash
  and keeps the delimited text literal here, but the string pipeline's *second* extraction pass
  (`INLINE_PASS`) re-scans that same de-escaped text and consumes its leading `+++`/`++` as a bare
  passthrough wrapping a shorter run – so these two escape forms are pinned as divergences rather than
  folded into the main parity corpus; an escaped `$$…$$` or `pass:[…]` has no such residue and stays
  parity. Inline STEM (`stem:[…]`, `asciimath:[…]`, `latexmath:[…]`) is an implicit passthrough too,
  but folds through its own [`Stem`](../../parser/src/inlines/stem.rs) node rather than `Raw`, so it is
  a separate, later increment. This step is **additive**: nothing is wired into the parse path.

  *Step 5b landed as (`AttributeReferences` → expanded-value splicing):* a new
  [`apply_attribute_references`](../../parser/src/content/inline_builder.rs) step is inserted between
  `Quotes` and `CharacterReplacements` – its position in the *normal* effective order
  (`specialcharacters → quotes → attributes → replacements → macros`, §3.4.1) – so whatever it splices
  into the tree is exactly what the two steps still ahead of it see. It reuses the string pipeline's
  *exact* recognition – [`ATTRIBUTE_REFERENCE`](../../parser/src/content/substitution_step.rs) is now
  shared `pub(crate)` – so only the recognition *sink* differs (§4.1): a reference to a **set**
  attribute has its resolved value spliced into the node stream, classified into
  [`Text`](../../parser/src/inlines/inline_node.rs) and [`Raw`](../../parser/src/inlines/inline_node.rs)
  runs by [`split_attribute_value`](../../parser/src/content/inline_builder.rs) – the §3.4.1 policy
  applied for the first time: because `SpecialCharacters` has already run and will not run again over
  spliced-in content, a literal `<`/`>`/`&` in the value becomes a `Raw` leaf (unescaped) rather than a
  `CharRef` (which the fold would re-escape), while everything else stays `Text`. An **escaped**
  reference (`\{name}`, `{name\}`, `\{name\}`) drops its backslash(es) and keeps the rest of its match
  as literal nodes, replacing nothing, mirroring `AttributeReplacer`'s `caps[1]`/`caps[5]` branch – and,
  because that check runs before any lookup, this works identically whether or not the named attribute
  is set. An `InterpretedValue::Set`/`::Unset` attribute (no textual value – the language leaves this
  case unclear, as the string replacer's own comment notes) expands to nothing, exactly as the string
  pipeline's replacer does.

  Three forms were initially deferred, each documented and pinned by a divergence test: a
  **`counter`/`counter2` directive**, whose resolution *advances* a document counter – a required side
  effect this additive step did not yet perform, the same reason every macro family deferred its own
  catalog/warning side effect until the footnote and cutover increments; a reference to a **missing**
  attribute under `AttributeMissing::Drop` / `::DropLine`, whose output *removes* content rather than
  leaving the reference literal – the behavior this step *does* reproduce, since it is also what the
  default `AttributeMissing::Skip` and `AttributeMissing::Warn` modes do (so those two are full parity,
  not a divergence); and a **construct inside an expanded value** that `CharacterReplacements` or
  `Macros` would recognize per §3.4.1 (a `(C)` becoming a `CharRef`, a `link:` becoming a `Ref`) but do
  not yet – a spliced value is a synthesized run with no `'src` slice of its own, and
  [`build_match_string`](../../parser/src/content/inline_builder.rs) (shared by those two steps and by
  `Quotes`) only treats a *verbatim* `Text` node (`value == location.data()`) as literal content for
  matching purposes; it does not yet look inside a synthesized one, so such a node is one opaque piece
  to them, exactly like an already-built `Styled` span. Lifting that boundary is a follow-up that only
  needs to extend `build_match_string` itself, not this step's splitting. The last two deferred forms
  are left **unrecognized** (no match, or no further node), so the surrounding gap logic reproduces the
  source text unchanged, exactly as an unrecognized macro is left for a later increment. This step is
  **additive**: nothing is wired into the parse path.

  *Follow-up landed as (the `counter`/`counter2` directive):*
  [`find_attribute_matches`](../../parser/src/content/inline_builder/attribute_refs.rs) now recognizes a
  `counter`/`counter2` directive (`{counter:name}`, `{counter2:name:seed}`), resolving *and advancing* the
  named counter via the parser's own [`counter`](../../parser/src/parser/parser.rs) method – the exact call
  `AttributeReplacer`'s counter branch makes – so the digits this step produces are the real, advanced
  sequence, not a placeholder. This is the same **required side effect** the footnote increment (part 4c)
  already established a precedent for: unlike every other macro family's catalog/warning side effect, a
  directive's resolved *value* is the number itself, so it cannot be deferred to the cutover without
  producing wrong output now. `counter` splices its new value in, classified by the same
  [`split_attribute_value`](../../parser/src/content/inline_builder/attribute_refs.rs) every other resolved
  reference uses; `counter2` advances silently and splices nothing, mirroring which directive spelling
  `AttributeReplacer` displays.

  A review caught a genuine ordering bug in the first version of this follow-up:
  [`apply_attribute_references`](../../parser/src/content/inline_builder/attribute_refs.rs) recurses into a
  `Styled` child's own content *before* processing its own level (so a later quote sub can match *inside*
  an earlier span – §4.1's nesting note), which means a directive nested in a span is, by construction,
  advanced *before* a plain-text directive that precedes that span in the source – reversing the numbering
  whenever a directive and a sibling span's own nested directive interleave (`{counter:n} *{counter:n}*`
  numbered `2`, then `1`, backwards). The fix is a dedicated
  [`resolve_counters`](../../parser/src/content/inline_builder/attribute_refs.rs) pass that runs once, before
  the splicing recursion: it merges, by source position, a level's own directive matches with its `Styled`
  siblings' placeholder positions (recursing into a sibling exactly when the merge reaches it), so every
  directive across the whole tree is advanced in true left-to-right document order regardless of nesting.
  Each resolved value is recorded keyed by the directive's absolute source byte offset, so the (unchanged)
  splicing recursion – which still visits levels in its own, differently-ordered sequence – looks values up
  by that stable key instead of resolving them itself, and the two passes agree because a `Styled` node's
  own placeholder occupies exactly one codepoint in its parent's match string regardless of what its
  (already-spliced-or-not) children contain, so recursion order never perturbs a parent level's own byte
  offsets.

  Because this additive builder is not yet wired into any real parse, its own `Parser` is never the one the
  authoritative string pipeline advances, so a differential fixture is free to build and fold against one
  independent default parser and compare against the golden string pipeline's own independent parser –
  exactly the two-independent-parsers discipline the footnote differential corpus already established –
  without the two sequences crossing over. The two forms 5b's own landed-as note still documents as
  deferred – a missing attribute under `Drop`/`DropLine`, and a construct inside an expanded value – remain
  outstanding.

  *Follow-up landed as (a character replacement inside an expanded value):* the *construct inside an
  expanded value* half of 5b's own deferred pair is closed for
  [`apply_character_replacements`](../../parser/src/content/inline_builder/char_replacements.rs) (the
  *macro* half remains deferred – see below). The shared match-string builder,
  [`build_match_string`](../../parser/src/content/inline_builder/quotes.rs), previously treated *any*
  non-verbatim node – a rendered span exactly as much as a synthesized (attribute-expanded) `Text` run – as
  one opaque placeholder, which is why a `(C)` inside `{note}` (`"(C) 2024"`) stayed unrecognized: the
  `Text` node's `value` differs from `location.data()`, so it fell through to that opaque case. It now
  splits a synthesized run into its own [`Piece`](../../parser/src/content/inline_builder/quotes.rs) kind –
  contributing the run's own `value` bytes to the match string (so a pattern sweep can match inside it) but
  flagged [`synthesized`](../../parser/src/content/inline_builder/quotes.rs), since those bytes have no
  honest `'src` counterpart. [`emit_range`](../../parser/src/content/inline_builder/quotes.rs) slices a
  synthesized piece's *value* (not its `location`) to the overlap and keeps the whole original `location` as
  every resulting fragment's coarse fallback span (design §4.4 – the same policy
  [`split_attribute_value`](../../parser/src/content/inline_builder/attribute_refs.rs) already gives every
  fragment of an expansion), and [`source_slice`](../../parser/src/content/inline_builder/quotes.rs) gained
  a start/end [`Bias`](../../parser/src/content/inline_builder/quotes.rs) so a boundary landing *inside* a
  synthesized piece falls back to that piece's whole node span while a boundary landing exactly on one of
  its own edges still resolves honestly (critically, to whatever construct comes immediately *after* it,
  not back into the synthesized run – see below).

  Because [`build_match_string`] is shared by every step in this module, this fix is also what keeps a
  *macro* family's own verbatim gate correctly rejecting a synthesized run now that it is no longer
  atomic: [`range_is_verbatim`](../../parser/src/content/inline_builder/macros/image.rs) (every macro
  family's own gate) and a new, narrower
  [`range_overlaps_synthesized`](../../parser/src/content/inline_builder/quotes.rs) (for the two macro
  families – the index-term shorthand and `indexterm2:[…]` – whose own recognition boundary was a bespoke
  opaque-span check rather than `range_is_verbatim`, since neither needs an honest `'src` slice for its
  *output*, only for deciding whether its *shown text* is reconstructable) both now reject a synthesized
  piece explicitly, so a macro inside an expanded value remains a documented divergence exactly as before,
  pinned by the existing (and one new, index-term) divergence test.

  Auditing every other consumer of the boundary-mapping helpers surfaced two real bugs, both fixed as part
  of this same follow-up rather than left as new divergences: (1) a boundary landing *exactly* on a
  synthesized piece's own edge was initially resolved via the same coarse whole-node-span fallback as an
  interior boundary, which is wrong whenever the piece's match-string length differs from its source
  length (the whole reason a piece is synthesized in the first place) – a construct recognized
  *immediately after* a synthesized run (e.g. a second `image:` macro right after an `{sp}` attribute
  reference) had its own location wrongly swallow the synthesized run's source bytes, corrupting an
  unrelated node's `is_icon` classification in one differential-corpus fixture; the fix skips that
  boundary to whichever piece comes next (or the past-the-last-piece fallback) instead of letting the
  synthesized piece claim it. (2) [`apply_footnotes`](../../parser/src/content/inline_builder/footnotes.rs)
  keeps its *own* copy of `emit_range`'s verbatim-slicing logic for the gaps around a recognized footnote
  ([`emit_range_recursing_footnotes`], which additionally recurses into a `Styled`/`Ref` child in place of
  cloning it whole); this copy had not been updated in step with `emit_range`'s own synthesized branch, so
  a plain attribute reference sitting beside (not inside) a footnote – reachable through the ordinary
  pipeline, since `apply_attribute_references` runs well before `apply_footnotes` – produced a corrupted,
  truncated `Text` value. Both are pinned by regression tests reproducing the exact fixture shapes that
  caught them, alongside unit tests exercising the fixed boundary-mapping helpers directly. This step is
  **additive**: nothing is wired into the parse path.

  *Step 5c landed as (`Callouts` → `Callout`, verbatim-group content):* a new
  [`apply_callouts`](../../parser/src/content/inline_builder.rs) step recognizes a trailing callout
  token (`<1>`, `<.>`, or `<!--1-->` for XML) in literal, listing, and source blocks as a
  [`Callout`](../../parser/src/inlines/callout.rs) node, folding through the same `render_callout` the
  string step calls so the output is byte-for-byte identical. This is the first increment for
  [`SubstitutionGroup::Verbatim`](../../parser/src/content/substitution_group.rs) rather than the
  *normal* order every step through 5b runs: literal/listing/source blocks apply only
  `SpecialCharacters` ahead of `Callouts`, so – unlike every other transducer in this module – the
  function need not descend into `Styled`/`Ref` children, since neither can exist at this point in that
  group's order. It reuses the string pipeline's *exact* recognition and trailing-position lookahead –
  [`build_callout_regexes`](../../parser/src/content/substitution_step.rs) is now shared `pub(crate)` –
  so only the recognition *sink* differs (§4.1): a match that fails the lookahead (not the last token on
  its line) is simply left out of the match list, so the surrounding gap reproduces its original nodes
  unchanged, the same outcome the string pipeline's `LookaheadReplacer` fallback produces.
  Auto-numbering (`<.>`) is scoped to one call of the step, exactly as the string replacer's counter is
  scoped to one block.

  This is also the node vocabulary's first real consumer for `Callout`, so its field set (Phase 0
  provisional) was refined here: alongside `number`, it now carries a `guard` – a new
  [`CalloutGuard`](../../parser/src/inlines/callout.rs) enum (`LineComment(prefix)` / `Xml`) recording
  which characters hide the callout in the raw source, decoupled from the render-seam's own
  `CalloutGuard` the same way every other node is decoupled from its `*RenderParams` counterpart (§4.6):
  the fold reconstructs `CalloutRenderParams` fresh from the node's `number` and `guard`, rather than the
  node carrying a render-params type directly. The Phase 1/2 recorder
  ([`inline_tree.rs`](../../parser/src/content/inline_tree.rs)), which had never captured a callout's
  guard (its differential corpus fixtures happened not to need it), now captures it too, so the two
  construction strategies agree on the node's shape.

  An escaped callout (`\<1>`) drops its backslash and stays literal, mirroring every other macro
  family's escape handling. As throughout the additive builder, this pass performs **no** recognition
  side effect: it does not call `Parser::register_callout`, deferring the callout catalog's validation
  to the cutover (step 6), exactly as the image and link increments defer their own catalog
  registrations. A differential corpus pins bare/explicit and auto-numbered callouts, multiple callouts
  on one line, XML callouts, the default line-comment prefixes, a custom or disabled `line-comment`
  (block-attribute and document-attribute), escapes, non-trailing-position and half-XML-comment
  non-matches, and both non-default `icons` renderings (font and image), alongside structural assertions
  on the node's `guard`. This step is **additive**: nothing is wired into the parse path.

  *Step 5d landed as (inline STEM → `Stem`, the first of 5d's four deferred forms):* a new
  [`apply_stem`](../../parser/src/content/inline_builder/stem_step.rs) step recognizes inline STEM
  macros (`stem:[…]`, `asciimath:[…]`, `latexmath:[…]`) as a [`Stem`](../../parser/src/inlines/stem.rs)
  node, folding through the same `render_quoted_substitution` the string pipeline's passthrough-restore
  step calls for a STEM entry, so the output is byte-for-byte identical. STEM is an **implicit
  passthrough** – `Passthroughs::extract_from` extracts it last, after both passthrough-macro passes, so
  that a passthrough placeholder nested inside a STEM expression survives – and `apply_stem` mirrors that
  ordering exactly: it is its own step, run immediately after
  [`apply_passthroughs`](../../parser/src/content/inline_builder/passthrough_step.rs) and ahead of every
  other step, so a STEM expression's content is never touched by specialcharacters, quotes, replacements,
  or macros (a `stem:[…]` written inside an already-extracted `+++…+++` passthrough is therefore *not*
  re-extracted, matching the string pipeline). It reuses the string pipeline's *exact* recognition and
  notation-resolution – `INLINE_STEM_MACRO` and `stem_notation` are now shared `pub(crate)` – so only the
  recognition *sink* differs (§4.1). The node's `value` is *not* the untouched source slice (unlike a
  macro node's target/display text elsewhere in this module): a STEM expression is unescaped (`\]` → `]`),
  has its legacy `latexmath` `$…$` wrapper dropped, and is run through the real substitution pipeline
  under its resolved substitution group ([`SubstitutionGroup::Stem`](../../parser/src/content/substitution_group.rs),
  special characters only, for a bare macro) via the passthrough step's own `passthrough_text` helper
  (now shared `pub(super)`) – so a custom `InlineSubstitutionRenderer`'s escaping is honored exactly as it
  would be for the string pipeline's own restore step, at the cost of an owned value rather than a `'src`
  borrow (the same trade-off the `++…++`/`$$…$$` passthrough forms make). The fold then passes that value
  straight through as `render_quoted_substitution`'s body, with no attribute list or id (the macro's
  pattern captures neither). The Phase-0 node doc (which described `value` as "the raw expression,
  carried through verbatim") was updated to match this decision.

  An escaped macro (`\stem:[…]`) drops its backslash and stays literal, mirroring every other macro
  family's escape handling. One form was initially deferred, documented and pinned by a divergence test: a
  macro carrying an **explicit substitution list** (`stem:c,q[…]`). This step is **additive**: nothing is
  wired into the parse path. The remaining three forms 5a already documents as deferred – an
  attribute-list-prefixed passthrough, a `pass:` macro with an explicit substitution list, and the bare
  unconstrained `+text+` form – remain for a later increment.

  *Follow-up landed as (the `stem:c,q[…]` explicit substitution list):* unlike a `pass:` macro's explicit
  list (deferred at the time, closed by step 5d part 3 below), a `Stem` node needs no richer subtree to
  carry this form: it already has a single `value` field, so the same treatment part 3 goes on to give
  `pass:` – running the expression through the **real substitution pipeline** under the list's resolved
  [`SubstitutionGroup`](../../parser/src/content/substitution_group.rs) – applies directly. A new
  `resolve_stem_subs` helper in
  [`stem_step.rs`](../../parser/src/content/inline_builder/stem_step.rs) resolves an explicit list (or
  falls back to [`SubstitutionGroup::Stem`] for a bare macro) via
  [`SubstitutionGroup::from_custom_string`] – the exact call [`InlineStemMacroReplacer`] makes, including
  its "skip and keep going" handling of an unrecognized name – and `stem_expression_value` substitutes the
  expression's `Text` runs through the resolved group instead of the hard-coded `Stem` group, so the fold
  is byte-for-byte identical to the string pipeline. As throughout this module, this does *not* raise the
  string pipeline's own `InvalidSubstitutionTypeForStemMacro` warning for an invalid name, deferring that
  side effect to the cutover (step 6) since it does not change the fold's output bytes. A differential
  corpus extends the existing STEM fixtures with a single- and multi-step list (applied in the order
  given), an unrecognized name skipped alongside a recognized one, and the escape form, mirroring the
  `pass:c,q[…]` corpus exactly.

  A review caught a genuine correctness gap in the first version of this follow-up: when the expression
  embeds an already-extracted [`Raw`](../../parser/src/inlines/inline_node.rs) passthrough (the case
  `build_stem_node`'s own doc comment covers, e.g. `stem:[+++x+++ text]`), `stem_expression_value`
  substitutes each surrounding `Text` run *independently* and splices the `Raw` back in verbatim between
  them. That is safe for the bare macro's default group (special characters only – a per-character,
  context-free substitution), which is the only group this splicing ever ran under before this follow-up –
  but an explicit list naming a step that needs more than one `Text` run of context (`Quotes`,
  `AttributeReferences`, `CharacterReplacements`, `Macros`, `PostReplacement`) can miss a construct whose
  two halves fall on either side of the `Raw` (`stem:q[*a +++x+++ b*]`): the string pipeline substitutes
  the *whole* expression as one string (the passthrough's content merely protected by its own sentinel, not
  absent from the string), so it finds the quote pair; splicing per fragment never sees a complete pair in
  either one. The fix is a new `subs_are_local` predicate – true when the resolved group's
  [`steps()`](../../parser/src/content/substitution_group.rs) are empty or contain only
  `SpecialCharacters` – that `build_stem_node` checks whenever the expression's `emit_range` recovers more
  than one node: a non-local explicit list beside a nested passthrough is left unrecognized (the same
  "documented divergence" shape every other boundary in this module takes) rather than silently diverging,
  while a *local* explicit list (`stem:c[…]`) beside the same nested passthrough is unaffected and still
  recognized. Two new tests pin this exactly: one confirming the local case still applies beside a nested
  passthrough, one confirming the non-local case is left unrecognized and diverges from the golden string
  pipeline's own (correct) output.

  [`InlineStemMacroReplacer`]: ../../parser/src/content/passthroughs.rs

  *Step 5d part 2 landed as (an attribute-list-prefixed passthrough → `Styled`):*
  [`apply_passthroughs`](../../parser/src/content/inline_builder/passthrough_step.rs) now recognizes all
  three attribute-list-prefixed forms 5a deferred – `[quotes]++text++`/`[quotes]+++text+++`/`[quotes]$$text$$`
  (`INLINE_PASS_MACRO`'s own attrlist branch), `` [x-]`text` `` and `[attrs]+text+` (`INLINE_PASS`, now
  shared `pub(crate)`) – each as a [`Styled`](../../parser/src/inlines/styled.rs) node (`Code` for
  monospace, `Unquoted` otherwise; always `Unconstrained`) whose attrlist is parsed the same way an
  attributed quote's is (the quotes step's `attributes_of`, now shared `pub(super)`), folding through the
  same `render_quoted_substitution` `PassthroughRestoreReplacer` calls when its stored passthrough carries
  a `type_`, so the output is byte-for-byte identical. The bare forms run as a genuinely **second pass**
  ([`apply_bare_attrlisted_pass_level`](../../parser/src/content/inline_builder/passthrough_step.rs)) over
  what the delimited pass leaves behind, mirroring `Passthroughs::extract_from`'s own two-regex order
  (`INLINE_PASS_MACRO` before `INLINE_PASS`) – which turns out to matter: an attribute-list-prefixed
  *delimiter* escape (`[attrs]\++text++`) drops its one backslash and leaves literal, unopaqued text
  behind, which the second pass then legitimately **re-recognizes** as its own (different) match, exactly
  as the string pipeline's own second regex pass does over its own once-substituted text – parity by
  construction, not a coincidence, once both passes exist.

  The legacy **`x-` compatibility marker** (an attrlist of exactly `x-`, or one ending in ` x-`) is the
  one case whose body is not a single `Raw` leaf: it switches the variant to `Code` and re-threads the
  body through the **full `Normal` substitution order** – special characters, quotes, attribute
  references, character replacements, macros, post-replacement, `SubstitutionGroup::Normal`'s own step
  list minus the passthrough/STEM extraction that already ran once, ahead of it – via a new
  [`apply_normal_subs`](../../parser/src/content/inline_builder/passthrough_step.rs) helper that chains
  the six existing step functions directly, mirroring `PassthroughRestoreReplacer`'s own recursive
  `pass.subs.apply(…)` call for that case as a node transducer rather than a second string pass. Only the
  `++` boundary (delimited) and the plus bare form trigger it; the backtick bare form's attrlist is
  *always* `x-`-eligible (the regex itself requires it) but its format mark keeps `subs` at `Verbatim`
  regardless, and `+++`/`$$` never switch at all – both mirrored exactly from `handle_quoted_text` and
  `InlinePassReplacer`.

  Two corner cases remain deferred, each documented and pinned by a divergence test: an **escaped
  bracket** (`\[attrs]++text++`), which unescapes to a literal `[attrs]` prefix *and* still recognizes the
  delimited text as an ordinary (non-attrlisted) passthrough – a kept-literal-prefix-with-one-dropped-char
  plus a node for the remainder, a shape neither `MacroMatchKind` variant expresses – and the
  **"prohibited prefix"** the string replacer's own retry loop protects (a bare attrlisted match
  immediately preceded by `\`, `:`, or `;`): rather than reproduce the retry, such a match is simply left
  unrecognized. This step is **additive**: nothing is wired into the parse path. The remaining two forms 5a
  documents as deferred – a `pass:` macro with an explicit substitution list, and the bare unconstrained
  `+text+` form with no attribute list at all – remain for a later increment.

  *Step 5d part 4 landed as (the bare unconstrained `+text+` form):* the last of the four forms 5a defers.
  [`find_bare_attrlisted_matches`](../../parser/src/content/inline_builder/passthrough_step.rs) now also
  recognizes `INLINE_PASS`'s third, attribute-list-free alternative, folding through a plain
  [`Raw`](../../parser/src/inlines/inline_node.rs) leaf – like the double-plus/double-dollar forms, an
  absent attrlist means no stored `type_`, so the restore never wraps the text in a rendered span, unlike
  the two attribute-list-prefixed bare forms part 2 landed. Unlike those two forms (matched via
  `\b{start-half}`, which does not by itself exclude a `\`/`:`/`;` prefix and so needed the "prohibited
  prefix" divergence just above), this form's own pattern already excludes that prefix directly in its
  *consuming* boundary group (`[^\w;:\\]`, which also encodes the "must not follow a word" rule) – so no
  runtime retry is needed at all: a match simply cannot start where the pattern's own character class would
  reject it, parity by construction rather than a divergence. The boundary character the pattern does
  consume ahead of the leading `+` (absent only when the match sits at the very start of the level) is not
  part of the construct itself; it is kept as literal text before the node, reusing the same kept-prefix
  `MacroMatch` sub-range the auto-link increment (part 2) introduced for a bare URL's own boundary prefix.
  An escaped mark (`\+text+`) drops the single backslash and keeps the rest of the match – the boundary
  character included – literal, with nothing left to re-scan it afterward (this is already the last pass),
  so it is plain parity rather than a divergence.

  Landing this form also **retired** the two divergences 5a's own escape handling documented: an escaped
  triple- or double-plus (`\+++text+++`, `\++text++`) drops its backslash and keeps the delimited text
  literal at the pass-macro level, and now that the bare unconstrained form is recognized too, the
  bare-form second pass legitimately re-scans that same de-escaped text and consumes its leading `+++`/`++`
  as a bare passthrough wrapping a shorter run (`+text` / `text`, one `+` left over as trailing literal
  text) – exactly what the string pipeline's own second regex pass does over its own once-substituted text.
  Both fixtures moved from their own divergence tests into the main differential corpus. The one form 5a
  documents as deferred that remains outstanding is a `pass:` macro with an explicit substitution list
  (`pass:c,q[…]`), which still needs a richer subtree than a single `Raw` leaf can hold. This step is
  **additive**: nothing is wired into the parse path.

  *Step 5d part 3 landed as (a `pass:` macro with an explicit substitution list → `Raw`, the last of 5d's
  four deferred forms):* the one form step 5a and part 4 both name as outstanding,
  [`build_pass_macro_subs_value`](../../parser/src/content/inline_builder/passthrough_step.rs), is
  recognized – but not, in the end, via a richer node subtree. Prototyping that shape first (threading the
  resolved [`SubstitutionGroup::Custom`] steps through this module's own transducers, the way the legacy
  `x-` compatibility marker's body already does via [`apply_normal_subs`](../../parser/src/content/inline_builder/passthrough_step.rs))
  surfaced a real bug, **pre-existing** in the already-landed `x-` marker path and independent of this
  step: [`apply_passthroughs`] runs *first* in [`build`](../../parser/src/content/inline_builder/mod.rs),
  so anything it splices is still visited by every one of `build`'s own later steps. That visit is a safe
  no-op for `Quotes` (delimiters are consumed, so a second pass finds nothing left) and
  `SpecialCharacters`/`CharacterReplacements`/`PostReplacement` (their own output is atomic or already
  stripped of what they match on) – but **not** for `Macros`: a link or cross-reference node's display
  text is *literal* text that reads exactly like the source it came from (by design, so the fold can
  recover it with no build-time state), so a second `Macros` pass recognizes it all over again and nests
  a second `Ref` inside the first. A prototype fixture (`[x-]++https://example.org++`, exercising the
  already-merged step 5d part 2) reproduces this today: `<code><a href="…"><a href="…">…</a></a></code>`,
  doubled. A list omitting a step `build`'s own fixed order still runs unconditionally (e.g. `pass:q[…]`,
  which never asks for `SpecialCharacters`) fails the opposite way: content the author's list deliberately
  left raw gets escaped anyway by `build`'s own later `SpecialCharacters` step. Solving this properly – so
  a spliced subtree is visited by *exactly* the steps its own resolved list named, once – is a splice-time
  protection mechanism this additive, pre-cutover module does not yet have; it is squarely the cutover's
  job (step 6, which does not splice mid-`build` at all).

  Given that, this increment takes the same shape every other deferred-until-now form in this file
  eventually adopts once it becomes tractable: the resolved list's body is rendered through the **real,
  string-based** substitution pipeline ([`SubstitutionGroup::apply`], via the passthrough step's own
  [`passthrough_text`](../../parser/src/content/inline_builder/passthrough_step.rs) helper already used for
  `++…++`/`$$…$$`/the bare unconstrained form) – exactly the call `PassthroughRestoreReplacer` makes for a
  stored `Passthrough` – producing an already-final HTML string that becomes a single
  [`Raw`](../../parser/src/inlines/inline_node.rs) leaf's `value` verbatim, folding through the identical
  byte-for-byte output the string pipeline produces. A `Raw` leaf is *opaque* to every one of `build`'s own
  later steps (never descended into, never re-matched), so it sidesteps both failure modes above
  regardless of which steps the author's list names or omits, and in which order – the list is applied
  once, by the real pipeline, and never touched again. An unrecognized substitution name in the list (e.g.
  `pass:bogus[…]`) is silently skipped – any recognized names are still honored – mirroring
  `SubstitutionGroup::from_custom_string`/`InlinePassMacroReplacer`'s own resolution; this additive pass
  does not yet raise the string pipeline's own `InvalidSubstitutionTypeForPassthroughMacro` warning for
  it, deferring that side effect to the cutover like every other macro family's own catalog/warning side
  effect, since it does not change the fold's output bytes. An escaped closing bracket (`pass:c[a\]b]`)
  unescapes before rendering, the same treatment every other `pass:[…]` bracket content gets – no longer a
  deferred corner the way a structured-children shape (a footnote's own content) would have forced. A
  differential corpus pins single- and multi-step lists (applied in the order given, not the *normal*
  effective order), an unrecognized name skipped alongside a recognized one, an empty resolved list (the
  content spliced back completely untouched), a list naming `Macros` (folding through real rendered
  markup), and the escape/unescape forms. This step is **additive**: nothing is wired into the parse path.
  Landing it closes out step 5d and, with it, step 5 in full.

  *Step 6 prep landed as (the image family's deferred recognition side effects, staged):* with step 5 done,
  every macro family the recorder covers now has a single-pass counterpart, but each one still skips the
  **recognition side effect** its string-pipeline replacer performs at the same point – registering an id,
  link, or image target in the document catalog, or recording a warning – deferring it "to the cutover"
  (step 6, below). Step 6 itself bundles a lot: swapping the recorder for the builder inside `Content`,
  making `rendered_html()` a fold, deleting the three sentinel systems, retiring the `with_inline_tree`
  flag, *and* re-attaching every deferred side effect all at once. Re-attaching the image family's own two
  side effects – [`register_image`](../../parser/src/parser/parser.rs) (for `image:`, gated on
  `catalog_assets`) and the `link=` dangerous-scheme/self-href warning `InlineImageMacroReplacer` records –
  turns out not to need the cutover itself: it is a standalone function,
  [`apply_image_side_effects`](../../parser/src/content/inline_builder/macros/image.rs), that walks an
  already-built tree and reads each [`Image`](../../parser/src/inlines/image.rs) node's own stored `target`
  and `attrs` instead of a regex capture, mirroring `InlineImageMacroReplacer::replace_append`'s own
  `link=self`/`link=`-scheme rejection logic (`link_self_resolves_to_src`, `has_dangerous_scheme`,
  `has_dangerous_self_href`, `is_uri_ish`) exactly. It recurses into every container an `Image` node can be
  nested inside – a `Styled` span, a `Ref`, or a `Footnote`'s own children – so a nested or footnote-embedded
  image is found too. Landing it now, ahead of the rest of step 6, gives the eventual cutover one fewer thing
  to get right in one leap: this piece is already written, tested against a broad fixture set (including a
  differential comparison against the golden string pipeline's own registrations, using the same
  two-independent-parsers discipline the footnote increment established), and reviewed on its own. As with
  every additive increment before it, **nothing is wired into a real parse path** – the function is called
  only by its own tests, against their own `Parser` – so calling it for real still waits for step 6, when it
  can be invoked exactly once per parse without double-counting a registration. A new
  [`Parser::catalog`](../../parser/src/parser/parser.rs) test-only accessor was added alongside it, so a test
  can inspect a `Parser`'s own live catalog directly (the registrations this function performs) without a
  full `Document` parse, whose own `Document::catalog()` is a separate, later snapshot. The remaining
  deferred side effects – an attributed span's and an anchor's own `register_ref` (plus the anchor's
  duplicate-id warning and the bibliography-anchor form), and `register_link` for the four link-macro forms –
  are unstaged and remain step 6's own work, alongside everything else step 6 bundles.

  *Step 6 prep landed as (the link family's deferred `register_link`, staged):* the same treatment, applied
  to the second of step 6's unstaged registrations. [`apply_link_side_effects`](../../parser/src/content/inline_builder/macros/links.rs)
  is a standalone function that walks an already-built tree and, for each [`Ref`](../../parser/src/inlines/ref_node.rs)`{Link}`
  node, calls [`register_link`](../../parser/src/parser/parser.rs) with the node's own stored `target` –
  no recomputation needed, since `target` already holds exactly the string the string pipeline's four link
  replacers (the `link:`/`mailto:` macro, the auto-link, and the formal-URL link – the bare e-mail form is a
  later increment, not yet built by this module) register. A cross-reference is also a `Ref` node but is
  never registered, mirroring the string pipeline's own link-only catalog. It recurses into every container
  a `Ref` node can be nested inside – a `Styled` span, another `Ref`'s own display children (so a link
  nested in a cross-reference's text, or vice versa, is found too), or a `Footnote`'s own children –
  mirroring the image increment's own recursion. As with the image side effects, **nothing is wired into a
  real parse path** – the function is exercised only by its own tests, against their own `Parser` – so
  calling it for real still waits for step 6. A broad differential corpus compares its registrations against
  the golden string pipeline's own, using the same two-independent-parsers discipline the image increment
  established. The remaining deferred side effect – an attributed span's and an anchor's own `register_ref`
  (plus the anchor's duplicate-id warning and the bibliography-anchor form) – is unstaged and remains step
  6's own work.

  *Step 6 prep landed as (the anchor and attributed-span family's deferred `register_ref`, staged – the
  last of step 6's unstaged registrations):* [`apply_ref_side_effects`](../../parser/src/content/inline_builder/macros/anchors.rs)
  is a standalone function that walks an already-built tree and, for each [`Anchor`](../../parser/src/inlines/anchor.rs)
  node and each id-carrying [`Styled`](../../parser/src/inlines/styled.rs) span, calls
  [`register_ref`](../../parser/src/parser/parser.rs) under `RefType::Anchor` – reading the node's own
  stored `id`/`reftext` instead of a regex capture. It reproduces the two divergent behaviors the string
  pipeline's two call sites give this one catalog side effect: an inline anchor additionally raises the
  duplicate-id warning `InlineAnchorReplacer` records (an attributed span's own registration stays silently
  non-fatal, mirroring the quotes step's own `let _ = register_ref(...)`), and a shorthand `[[id]]`
  immediately preceded by a `[` – the inner anchor of a `[[[id]]]` sequence appearing *outside* a
  bibliography list item – is recognized (already, by the existing node builder) but never registered,
  mirroring `InlineAnchorReplacer`'s own `is_bibliography_inner` check (recomputed here from the node's own
  source span rather than a haystack index, since the tree walk has no regex capture to read it from). The
  true bibliography-anchor construct itself (`[[[label]]]` inside a bibliography list item, `RefType::Bibliography`)
  is a separate, list-item-gated pass (`INLINE_BIBLIO_ANCHOR`) this builder does not yet recognize as its own
  node at all; that remains out of scope here, same as before. A description-list term's own leading-anchor
  pre-registration (`DefinedTerm::substitute`, `apply_macros_with_leading_anchor_registered`) is mirrored by
  a `leading_anchor_registered` parameter, so wiring this function in at that call site can suppress the same
  duplicate-id warning the string pipeline suppresses there. It recurses into every container an id-bearing
  node can be nested inside – a `Styled` span, a `Ref`'s own display children, or a `Footnote`'s own children
  – mirroring the image and link increments' own recursion. As with those, **nothing is wired into a real
  parse path** – the function is exercised only by its own tests, against their own `Parser` – so calling it
  for real still waits for step 6. A broad differential corpus compares its registrations against the golden
  string pipeline's own, using the same two-independent-parsers discipline the image increment established.
  With this landed, every recognition side effect step 5's macro families skip is now staged as its own
  unwired building block; step 6 itself – swapping the recorder for the builder, the fold, the sentinel
  deletions, and calling each staged function exactly once per parse – remains fully outstanding.

  *Step 6 prep landed as (a combined entry point for every staged side effect, plus a link-registration-order
  fix it surfaced):* with all three families' side effects staged individually, the cutover's own job of
  "calling each staged function exactly once per parse" needed one more piece: a single entry point that
  composes them in the right relative order, since the two link-recognizing families and the image/anchor
  families all write into catalogs and warnings the cutover must get right *together*, not just individually.
  [`apply_macro_side_effects`](../../parser/src/content/inline_builder/macros/mod.rs) is that entry point –
  it calls [`apply_image_side_effects`](../../parser/src/content/inline_builder/macros/image.rs), then
  [`apply_link_side_effects`](../../parser/src/content/inline_builder/macros/links.rs), then
  [`apply_ref_side_effects`](../../parser/src/content/inline_builder/macros/anchors.rs) – the same relative
  order the string pipeline's own macro passes run in (§4.1) – so that when more than one family's side
  effect touches the *same* shared list (concretely, [`Parser::record_substitution_warning`](../../parser/src/parser/parser.rs)'s
  one shared warnings list, which both the image family's dangerous-scheme warning and the anchor family's
  duplicate-id warning write to) the combined call lands them in the golden pipeline's own order. A new
  differential corpus exercises the composed call directly – a fixture mixing an image, both link forms, and
  an anchor in one content, and a fixture whose image and anchor *both* warn, asserting the two warnings land
  in image-then-anchor order against an independent golden parser (the same two-independent-parsers
  discipline every prior staged function's own corpus uses).

  Building that composed differential corpus surfaced a genuine ordering bug in the already-staged
  [`apply_link_side_effects`](../../parser/src/content/inline_builder/macros/links.rs): the string pipeline
  registers a link's target when its *own* replacer's pass matches it, and the auto-link/formal-URL pass
  (`INLINE_LINK`) and the `link:`/`mailto:` macro pass (`INLINE_LINK_MACRO`) are two separate, sequential
  whole-string passes (§4.1) – so the catalog ends up in **family-pass order, not true source order**: every
  auto-link/formal-URL link registers before every `link:`/`mailto:` macro, regardless of which appears first
  in the source (already pinned, independently of this module, by
  `catalog_records_link_targets_when_catalog_assets_enabled` in `tests/asciidoctor_rb/substitutions_test.rs`).
  The originally-staged function instead made one tree walk in document order, which is only ever correct by
  coincidence – a content that interleaves the two forms out of that relative order (`link:b.html[B] then
  https://a.example`) diverges: document order would register `b.html` first, but the golden catalog is
  `["https://a.example", "b.html"]`. No existing test exercised mixed forms in one content, so this had gone
  unnoticed. The fix makes two passes over the tree – every auto-link/formal-URL match first, then every
  `link:`/`mailto:` macro match – distinguishing the two from a node's own `location` alone (a `link:`/
  `mailto:` match's location always starts with its literal prefix, and the auto-link pass never builds a
  node for `INLINE_LINK`'s own link-macro branch, deferring that whole form to the macro pass – see
  [`inline_link_level`](../../parser/src/content/inline_builder/macros/links.rs)'s own doc comment – so this
  is a reliable signal, not a heuristic, and needs no new node field). A new test pins the interleaved case
  directly against the golden pipeline, and the broad differential fixture set gains an interleaved fixture
  too. As with every prior increment in this module, nothing here is wired into a real parse path – calling
  [`apply_macro_side_effects`] for real still waits for the single-pass builder to replace the recorder as
  `Content`'s tree source, which remains step 6's own, still fully outstanding, job.

  *Step 6 prep landed as (a whole-pipeline differential corpus against the real `SubstitutionGroup::apply`
  entry point):* every differential corpus landed so far (one per step/family) hand-chains only the
  [`SubstitutionStep`]s that step's own increment covers – skipping `AttributeReferences` unless the fixture
  needs it, and never running passthrough extraction/restore or deferred cross-reference finalization
  alongside the other steps. That pins each step in isolation, but it had never exercised the *fully
  assembled* pipeline [`SubstitutionGroup::Normal::apply`](../../parser/src/content/substitution_group.rs)
  runs in production – passthrough/STEM extraction, every step in true order, passthrough restore, and
  deferred-reference finalization, all against one `Content` – which is exactly what [`build`] (this
  module's own single call) must reproduce once the cutover wires it in. A new differential corpus, in
  [`inline_builder`](../../parser/src/content/inline_builder/mod.rs)'s own test module, closes that gap: each
  fixture calls the real, public `SubstitutionGroup::Normal.apply` as the golden and `build` + `fold_html`
  as the candidate, and – unlike every prior corpus, each scoped to one family – **combines** several
  construct families in one piece of content (quotes wrapping an attribute reference, a footnote whose own
  text carries a nested attribute reference, a passthrough beside an image macro, a `counter` directive
  beside a formatted span carrying an attribute reference and a STEM expression, an inline anchor beside an
  index term, escaped constructs beside a live attribute reference, …), so a boundary-crossing interaction
  between two steps that individually pass would still be caught. As with every prior corpus, a fixture stays
  inside the vocabulary `build` already covers, avoiding the forms still documented as deferred elsewhere in
  this module (an attribute value that itself embeds a construct `CharacterReplacements`/`Macros` would
  recognize, the `hardbreaks` option, a menu's `>` submenu form, …). Every fixture passed without needing a
  code change, giving the first real, end-to-end confirmation that the fully assembled single-pass builder
  – not just its individual steps – reproduces the real production pipeline's output, ahead of step 6's own
  wiring work. As with every prior increment, nothing here is wired into a real parse path.

  *Step 6 prep landed as (the `hardbreaks` option, closing a real cutover blocker):* auditing `build`'s
  vocabulary against a corpus-wide differential run (rather than the hand-picked fixtures every prior corpus
  in this module uses) surfaced that the `hardbreaks` block option was not just an unclaimed form like the
  others this module documents as deferred, but a **real blocker**: golden tests already exercise it (design
  §5.4's oracle), so cutting over `Content` to `build` while it stayed unhandled would have silently regressed
  them, not merely left a construct unrecognized. [`apply_post_replacements`](../../parser/src/content/inline_builder/post_replacements.rs)
  now takes the enclosing block's own `Attrlist` (`build` itself gains the same `Option<&Attrlist<'src>>`
  parameter, threaded through from its caller – every other step ignores it) and, when
  `parser.is_attribute_set("hardbreaks-option")` or the attrlist's own `%hardbreaks` option is set, runs a new
  `apply_hardbreaks` in place of the default ` +`-only form: every line ending in the level's own match string
  becomes a break, a redundant trailing ` +` is stripped rather than doubled, and the level's own last line
  (nothing follows its `\n`) never gets one – mirroring the string pipeline's own `lines()`-split, per-line
  `render_line_break`, rejoin exactly. Both forms now share one `emit_breaks` tail. A differential corpus pins
  the block-attrlist and document-attribute forms of the option, a redundant-` +`-stripping fixture, a
  single-line no-op, and recursion into a quoted span, against the real `SubstitutionGroup::Normal` pipeline;
  the whole-pipeline combined-constructs corpus above gains a hardbreaks-plus-attribute-reference-plus-span
  fixture too. As with every increment before it, this is purely additive: `build`'s new parameter is `None`
  at every existing call site, so no real parse path is touched.

  *Step 6 prep landed as (a structural cross-check against the Strategy-A recorder):* with the
  vocabulary essentially complete, the last thing missing before the real tree-source swap is the
  due-diligence design §4.1 and §5.5's own risk table call for directly: "the node stream is
  cross-checked against Strategy A's recorder to catch structural regressions the HTML oracle
  cannot see" – because "two node trees fold to identical HTML, masking a structural bug" is
  exactly the risk every prior corpus in this module, pinning HTML-fold parity alone, cannot rule
  out. A new test module,
  [`inline_builder_recorder_parity`](../../parser/src/tests/inline_builder_recorder_parity.rs),
  builds both trees for the same fixture – the recorder's via `Content::inlines()` under
  [`Parser::with_inline_tree`](../../parser/src/parser/parser.rs), the builder's via `build` – and
  compares them structurally, node kind by node kind, over the same broad general-purpose and
  combined-constructs fixture sets [`inline_builder`](../../parser/src/content/inline_builder.rs)'s
  own whole-pipeline corpus already proved stay inside `build`'s claimed vocabulary.

  The comparator ignores exactly the fields already documented elsewhere as one-sidedly richer on
  the builder (`attrs`, `derived`, `xrefstyle`, `resolved`, an anchor's `reftext`, an image's
  `is_icon`) and resolves every *leaf-boundary* difference – a builder `Text`/`Raw`/`CharRef` node
  whose rendered bytes are only a sub-range of what the recorder recovered, or vice versa – through
  one shared mechanism,
  [`consume_rendered_prefix`](../../parser/src/tests/inline_builder_recorder_parity.rs): it renders
  each recorder leaf back to the bytes it would fold to (a `Text` leaf verbatim, a `CharRef` leaf
  through the same table
  [`classify_entity`](../../parser/src/content/inline_tree.rs) uses, reproduced as
  `RECORDER_ENTITY_TABLE` so the two stay in lockstep) and matches a run of them against the
  builder leaf's own value, splitting a trailing `Text` leaf when the match ends part-way through
  it. This one mechanism is what makes a combined `CharRef::Replacement` (an em dash surrounded by
  spaces, recovered by the recorder as three adjacent entity leaves), an attribute-expansion `Text`
  boundary the builder draws but the recorder cannot see, a passthrough's `Raw` content (which the
  recorder – no renderer call to intercept during restore – recovers as plain `Text`/`CharRef`
  leaves instead), and a source-written entity that coincides byte-for-byte with a live
  classification (`&amp;`, `&#8217;`, …) all resolve the same way, rather than as separate
  special cases.

  Two further, non-leaf differences turned out to be genuine (if narrow) structural facts, not
  bugs, and are excluded with their own documented reasoning: an **unresolved** cross-reference's
  `children` is legitimately empty on the recorder side (Asciidoctor's own unresolved-xref fallback
  renders the bracketed target, never the author's display text, so the recorder – recovering only
  what actually rendered – has nothing to see), while the builder always bakes the display text in
  as a structural fact independent of resolution; and a footnote **reference** occurrence's `id`
  stops reaching the renderer's own params the moment it resolves to a number (`fold_footnote`
  renders just the number then), so the recorder cannot recover it there either, while the builder
  still carries it. A `Link`'s `roles`/`window` fields are also skipped whenever its own
  attribute-list display text populated `attrs` instead (`render_link` reads `role`/`window`
  straight off `attrs` in that case, so the plain fields are never populated from it – the same
  asymmetry `Ref::attrs`'s own doc comment already describes).

  Every fixture passed once these were accounted for, without needing a code change to `build`
  itself – the first structural (not merely byte-parity) confirmation that the single-pass
  builder's tree is a faithful counterpart to the recorder's, ahead of the real swap. As with every
  prior increment, nothing here is wired into the parse path.

  *Step 6 prep landed as (a synthesized-seed entry point, for a real block's filtered/joined
  content):* the last structural piece the tree-source swap needs is an answer to *what `Span<'src>`
  does `build` even run on*, given that a real block's `Content` does not always hold one: the common
  single-surviving-line case
  ([`Content::from_filtered_lines`](../../parser/src/content/content.rs)) borrows a contiguous `'src`
  slice, but a genuinely multi-line block (or any other filtered value) joins its surviving lines into
  an *owned* string with no honest `'src` slice of its own – exactly the shape
  `build`'s own `source: Span<'src>` parameter could not accept. [`build_from_value`](../../parser/src/content/inline_builder.rs)
  closes this: it generalizes `build`'s seed from a bare `Span<'src>` to the `(value, location)` pair
  `Content` itself is already built from, so `build` becomes a thin wrapper
  (`build_from_value(CowStr::from(source.data()), source, …)`) over it. When `value` coincides with
  `location.data()` this is the existing verbatim path, unchanged; when it does not, the seed is
  *synthesized* – and every downstream step already knows what to do with that, because it is the same
  verbatim/synthesized split [`apply_special_characters`](../../parser/src/content/inline_builder/special_chars.rs)'s
  own `split_text` already makes for a single node deeper in the tree, and the same coarse-fallback
  policy (design §4.4) an attribute-reference expansion or a `counter` directive's resolved value
  already receives. No step needed a code change: `build_match_string` and `source_slice`
  ([`quotes.rs`](../../parser/src/content/inline_builder/quotes.rs)) already treat *any* non-verbatim
  `Text` node this way regardless of where in the tree — or how early — it was produced, so a
  wholly-synthesized *root* seed is just the same mechanism reached one level higher than any increment
  had exercised it before.

  This closes the gap for every step that never needed a verbatim `'src` slice in the first place –
  quotes, specialcharacters, attribute references, character replacements, post-replacement (including
  `hardbreaks`), and a macro family (a bare `footnote:[…]`) whose own content is captured as children
  rather than a literal value – pinned by a differential corpus comparing `build_from_value` against the
  real pipeline for a simulated multi-line, indentation-filtered block. It does **not** lift the
  existing "a macro inside a synthesized run is deferred" boundary (§4.1's `apply_macros` note,
  [`range_is_verbatim`](../../parser/src/content/inline_builder/macros/image.rs)) for a family that
  needs its own verbatim target/id — a link, image, cross-reference, anchor, or non-concealed index
  term — since that boundary was written for a run *nested inside* an otherwise-verbatim tree and reads
  identically when the *whole* seed is that run; a wholly-synthesized `<<id>>` is therefore still left
  unrecognized today, a documented divergence pinned by its own test rather than silently regressed.
  Lifting that boundary — so a real multi-line block carrying one of those constructs also folds
  identically through `build_from_value` — remains a later increment's job, alongside the rest of step
  6's own wiring work. As with every prior increment, nothing here is wired into the parse path.

  *Step 6, first half, landed as (the tree-source swap: the single-pass builder replaces the
  recorder as `Content`'s tree source):* with every prep piece in place, the swap each of them
  named as "this step's own job" is done.
  [`SubstitutionGroup::apply`](../../parser/src/content/substitution_group.rs) no longer runs the
  Strategy-A recording pass at all: when tree building is enabled
  ([`Parser::with_inline_tree`](../../parser/src/parser/parser.rs)), it snapshots the
  **pre-substitution content value** and a clone of the parser before the authoritative pass runs
  – the same counter-safe discipline the recorder used, so footnote numbers and `{counter:…}`
  values come out identical to the authoritative output – then builds the tree with a new
  group-aware entry point,
  [`build_for_group`](../../parser/src/content/inline_builder/mod.rs), and stores it on
  [`Content::inlines`](../../parser/src/content/content.rs). `build_for_group` mirrors
  `run_pipeline`'s own step selection exactly: passthrough/STEM extraction runs first **iff** the
  group's steps include `Macros` or the group is `Header` (the same gate `run_pipeline` places on
  `Passthroughs::extract_from`), then each of the group's
  [`steps()`](../../parser/src/content/substitution_group.rs) runs in the group's own order, each
  recast as its node transducer – so a `subs=` custom list, the verbatim group's `Callouts`, the
  header/attribute-entry-value groups' two-step list, and the empty `Pass`/`None` lists (whose
  tree is the untouched seed) all follow the string pipeline's own selection, pinned by new
  per-group parity and structure tests. `build_from_value` is now the `Normal`-group special case
  of this entry point.

  What the swap changes for a tree consumer: every node now carries its **honest, precise span**
  (issue #944's precision stage, with the documented §4.4 coarse fallback for synthesized values)
  and macro nodes are **self-describing** (their own `Attrlist<'src>`, `derived`, `xrefstyle`) –
  the recorder's whole-content-span, `attrs: None` tree is gone from production. The fold-parity
  guarantee is now scoped to the builder's claimed vocabulary: a form documented as deferred
  (e.g. a display text crossing a rendered span) is left as **literal text** in the tree – never
  a wrong node – where the recorder, recovering structure from rendered output, could represent
  it. That scoping surfaced in exactly one production seam: the positional cross-reference
  resolution mirror (`mirror_tree_xref_resolution`), whose count-parity debug assertions assumed
  the tree holds one node per deferred segment. The mirror now **counts each list's slots first
  and skips a list whose count diverges** – leaving those nodes in their honest unresolved state
  rather than assigning destinations positionally onto the wrong nodes – pinned by new tests for
  both the block-level and footnote-embedded skip paths. The recorder's stateful-renderer hazard
  (its debug assertion, and the "requires a side-effect-free renderer" caveat on
  `with_inline_tree`) retires with the second rendering pass itself: the builder consults the
  configured renderer only where a node's *value* is defined as already-substituted text (a
  delimited passthrough's or STEM expression's body), so a stateful custom renderer now gets a
  logical, unpolluted tree – repinned by the corresponding test.

  The recorder ([`inline_tree`](../../parser/src/content/inline_tree.rs)) is not deleted but
  **retired to test-only oracle machinery** (`#[cfg(test)]`), exactly the §4.1 bring-up-oracle
  role: the differential harness drives it directly, and the structural cross-check
  ([`inline_builder_recorder_parity`](../../parser/src/tests/inline_builder_recorder_parity.rs))
  now builds its recorder side by driving that machinery itself – the production accessor returns
  the builder's tree, so reading `Content::inlines()` for both sides would compare the builder to
  itself – keeping the two independent constructions honestly comparable. The remainder of step 6
  is unchanged and still outstanding: making `rendered_html()` an authoritative fold of this
  tree, calling `apply_macro_side_effects` for real (which must wait for the fold, since until
  then the string pipeline still performs every registration), deleting the three production
  sentinel systems, and retiring the `with_inline_tree` flag.

  *Next steps (each a transducer step, gated by the golden-HTML oracle §5.3):*
  1. ✅ Foundation + `SpecialCharacters`.
  2. ✅ `Quotes` → `Styled`, introducing nesting (`*a _b_ c*` becomes a tree, not a flat run).
  3. ✅ `CharacterReplacements` → `CharRef::Replacement`, and `PostReplacement` → `LineBreak`.
  4. ✅ `Macros`, sliced by construct family, each capturing the owned `Attrlist<'src>` it carries –
     the step that makes nodes **self-describing** (and the one that finally unblocks
     `render_with`):
     - ✅ **4a.** `Image` / icon (`image:` / `icon:`).
     - ✅ **4b.** the remaining families, each its own sub-step:
       - ✅ **4b(i).** `Ui` (`kbd:` / `btn:` / `menu:`).
       - ✅ **4b(ii).** `Ref` (links / cross-references), `Footnote`, `IndexTerm`, `Stem`, `Anchor`,
         itself sliced into parts:
         - ✅ **part 1.** the `link:`/`mailto:` macro (`INLINE_LINK_MACRO`) → `Ref{Link}`.
         - ✅ **part 2.** auto-links and formal-URL links (`INLINE_LINK`) → `Ref{Link}`.
         - ✅ **part 3.** cross-references (`INLINE_XREF`) → `Ref{Xref}`, itself sliced:
           - ✅ **part 3a.** the same-document `xref:` macro form (`xref:id[text]`).
           - ✅ **part 3b.** the same-document `<<id>>` shorthand (`<<id>>` / `<<id,text>>`).
           - ✅ **part 3c.** the node-blocked forms both spellings share, split in two once landed:
             - ✅ inter-document targets and the document-as-a-whole form (a *derived*
               destination) – needed only an existing, already-public type (`Ref` gains
               `derived: Option<DerivedReference>`), no consumer required to pin its shape.
             - ✅ an attribute-list text (`window`/`role` reuse `Ref`'s existing plain fields; a
               new `Ref::xrefstyle: Option<XrefStyle>` field carries the macro-level override) –
               needed no consumer to pin its shape, since `XrefRenderParams` itself takes plain
               fields, not a borrowed `Attrlist<'src>`.
         - ✅ **part 4.** `Anchor`, `IndexTerm`, `Footnote`, each its own sub-step (inline `Stem` is
           *not* a macros-step family – it is extracted at passthrough time, so it lands in step 5):
           - ✅ **part 4a.** inline anchors (`[[id]]` / `anchor:id[…]`, `INLINE_ANCHOR`) → `Anchor`.
           - ✅ **part 4b.** index terms (`((term))` / `(((primary, secondary)))` / `indexterm:[…]` /
             `indexterm2:[…]`, `INLINE_INDEXTERM`) → `IndexTerm`.
           - ✅ **part 4c.** footnotes (`footnote:[…]` / `footnote:id[…]` / `footnote:id[]`,
             `INLINE_FOOTNOTE_MACRO`) → `Footnote` – the last macro family.
  5. `AttributeReferences` (expanded-value splicing, §3.4.1), passthroughs (`Raw`), and
     `Callouts` – completing the vocabulary the recorder covers, each its own sub-step:
     - ✅ **5a.** Passthroughs, the delimited forms (`+++…+++`, `++…++`, `$$…$$`, bare
       `pass:[…]`) → `Raw`.
     - ✅ **5b.** `AttributeReferences` (expanded-value splicing, §3.4.1).
     - ✅ **5c.** `Callouts` (verbatim-group content – literal, listing, and source blocks).
     - ✅ **5d.** the deferred forms `5a` documents, itself sliced into parts:
       - ✅ **part 1.** inline STEM (`stem:[…]`, `asciimath:[…]`, `latexmath:[…]`) → `Stem`,
         including an explicit substitution list (`stem:c,q[…]`).
       - ✅ **part 2.** an attribute-list-prefixed passthrough (`[quotes]++text++`, `` [x-]`text` ``,
         `[attrs]+text+`).
       - ✅ **part 3.** a `pass:` macro carrying an explicit substitution list (`pass:c,q[…]`) → `Raw`,
         rendered through the real substitution pipeline rather than a node subtree (see the step's own
         landed-as note for why).
       - ✅ **part 4.** the bare unconstrained `+text+` form.
  6. **Cut over:** swap the recorder for the single-pass builder in `Content`, make
     `rendered_html()` a fold, delete the three production sentinel systems (§4.2), and retire
     the `with_inline_tree` opt-in flag (the deferred remainder of Phase 2). Re-attach the
     recognition **side effects** the string pipeline performs that the additive builder skips –
     registering an inline id (an attributed span's anchor) in the reference catalog so
     cross-references resolve (#1087), registering a link target (`register_link`) and an image
     target (`register_image`) in the asset catalog so `Document::catalog()` stays complete, and
     the dangerous-scheme substitution warning. Doing this at the cutover (rather than in the
     additive passes, which run *alongside* the authoritative string pipeline) is what avoids
     double-counting each registration.
     - ✅ **prep.** The image family's own two side effects (`register_image`, the `link=`
       dangerous-scheme/self-href warning) are already written and tested as a standalone,
       unwired function, [`apply_image_side_effects`](../../parser/src/content/inline_builder/macros/image.rs)
       – see the step's own "landed as" note above.
     - ✅ **prep (links).** `register_link` for the four link-macro forms is likewise staged as a
       standalone, unwired function,
       [`apply_link_side_effects`](../../parser/src/content/inline_builder/macros/links.rs) – see the
       step's own "landed as" note above.
     - ✅ **prep (anchors).** The anchor/attributed-span `register_ref` pair – the last of step 6's
       unstaged registrations – is likewise staged as a standalone, unwired function,
       [`apply_ref_side_effects`](../../parser/src/content/inline_builder/macros/anchors.rs) – see the
       step's own "landed as" note above. Every deferred recognition side effect is now staged;
       calling each one for real, exactly once per parse, remains this step's own job.
     - ✅ **prep (whole-pipeline parity).** A combined, multi-family differential corpus confirms
       `build` reproduces the real, public `SubstitutionGroup::Normal::apply` entry point
       byte-for-byte – see the step's own "landed as" note above. Wiring `build` into `Content` in
       place of the recorder, and calling `apply_macro_side_effects` for real, remain this step's
       own job.
     - ✅ **prep (hardbreaks).** The `hardbreaks` block option – identified as a real cutover
       blocker, not merely an unclaimed form, since golden tests already exercise it – is now
       recognized by `apply_post_replacements`, which takes the enclosing block's `Attrlist` for
       it; see the step's own "landed as" note above. `build`'s new `Attrlist` parameter is `None`
       at every existing call site, so this is unwired like every other prep piece.
     - ✅ **prep (structural cross-check).** A corpus-wide comparison of the builder's tree against
       the Strategy-A recorder's, structurally rather than by HTML-fold parity alone – the
       due-diligence design §4.1/§5.5 call for before the swap – found every difference to be
       already-documented one-sided richness or a leaf-boundary artifact of recovering structure
       from rendered output, and none a genuine regression; see the step's own "landed as" note
       above. Wiring `build` into `Content` in place of the recorder remains this step's own job.
     - ✅ **prep (synthesized seed).** [`build_from_value`](../../parser/src/content/inline_builder.rs)
       generalizes `build`'s seed from a bare `Span<'src>` to the `(value, location)` pair `Content`
       itself is built from, so a genuinely multi-line, filtered block (whose joined `rendered` text
       has no single contiguous `'src` slice) can be processed too, not only the common
       single-surviving-line case – see the step's own "landed as" note above. A macro family needing
       its own verbatim target/id remains deferred for a wholly-synthesized seed, a documented
       divergence pinned by its own test. Wiring `build`/`build_from_value` into `Content` in place of
       the recorder remains this step's own job.
     - ✅ **prep (anchor synthesized boundary lifted).** The "macro inside a synthesized run"
       boundary §4.1/§4.4 describe – and the synthesized-seed note directly above still pins for
       link/image/xref – is **lifted for the anchor family**, the one macro whose node needs no
       `Span`-typed field beyond its own `location` (an id and an optional reftext, both plain
       text). A new [`text_slice`](../../parser/src/content/inline_builder/quotes.rs) helper
       reuses `emit_range`'s own verbatim/synthesized slicing to recover a macro's target *text*
       exactly – concatenating a synthesized piece's `value` instead of `source_slice`'s coarse
       whole-piece fallback – and a new
       [`range_is_verbatim_or_synthesized`](../../parser/src/content/inline_builder/macros/image.rs)
       gate accepts a synthesized overlap while still rejecting an atomic one (an escaped special
       or a rendered span, the boundary every family keeps). `build_anchor_node` and
       `build_anchor_reftext` (`macros/anchors.rs`) now use these instead of deferring: an id or
       reftext coming from an attribute expansion, or – reached at a tree's root via
       `build_from_value` – a filtered multi-line block's own joined seed, is recognized with its
       exact text, while the node's `location` keeps the coarse enclosing-span fallback design §4.4
       already establishes elsewhere (only the *text* needed precision; `Attrlist`-bearing families
       like image/link still cannot lift this boundary the same way, since `Attrlist::parse` reads
       its `source: Span<'src>`'s bytes as content, not just as a location tag – a real `'src` slice
       is not optional there). The `an_anchor_inside_an_expanded_attribute_value_is_a_documented_
       divergence` test #1177 added is now a parity test instead of a divergence test, per its own
       "if lifted, fold this into a parity corpus" note. Wiring `build`/`build_from_value` into
       `Content` in place of the recorder remains this step's own job.
     - ✅ **first half: the tree-source swap.** `SubstitutionGroup::apply` builds each content's
       tree with the single-pass builder – via the new group-aware
       [`build_for_group`](../../parser/src/content/inline_builder/mod.rs), mirroring
       `run_pipeline`'s own step selection per substitution group – in place of the Strategy-A
       recording pass, which is retired to test-only oracle machinery; see the step's own
       "landed as" note above. The remaining half – `rendered_html()` as an authoritative fold,
       `apply_macro_side_effects` called for real, the sentinel deletions, and the flag's
       retirement – is still outstanding.
  7. `render_with` / `render_to` (the Phase 3 remainder) and `Document::to_asg()`, now that
     nodes are self-describing; retire the `attribute-missing` per-line hack (#564).

- **Phase 5 — renderer seam v2.** Reshape `InlineSubstitutionRenderer` into the AST-walking
  form and rename it to `InlineRenderer` (§4.6); update the README backend story.
  *Exit:* seam documented; a smoke-test alternate renderer (in tests) walks the tree.

- **Landing — preflight + merge to `main`.** Preflight the whole branch against the
  `asciidoctor` port (§5.1) to confirm the public API and reshaped seam serve a real
  consumer, merge current `main` in one last time, then land `inline-ast` into `main` with a
  **merge commit** (not a squash — §5.1) so the staged history survives.
  *Exit:* `asciidoctor`-port preflight green; branch merged.

Phases 1–2 are the risk-bearing core; 3–5 are additive and can be paced against consumer
demand (echoing the #943/#944 "pin the API with a real consumer" discipline). All of them
land together in the single merge to `main` — the pacing is *within* the branch, not
staggered submissions to `main`.

### 5.3 The golden-HTML oracle (the safety net)

The ~277 existing `.rendered()` assertions comparing against literal HTML strings are the
migration's single most valuable asset. The invariant is the **asserted output strings**,
not the accessor name: they are kept **unchanged** and treated as an executable
specification of correct output (the Phase 3 rename to `.rendered_html()` rewrites only the
call, never the expected string). Any phase that changes internals must leave every one of
them green. This is what lets us re-architect the interior
aggressively without fear: the observable contract is pinned.

Add a **corpus-wide differential harness** early: parse every fixture, fold the tree, and
assert equality with the pre-migration `rendered()` (captured as a snapshot). This catches
regressions the hand-written assertions don't cover.

### 5.4 Test migration

- **Keep** the HTML-string assertions through Phases 1–4 as the oracle.
- **Add** structural assertions (on `inlines()`) for new guarantees the string can't express
  (nesting, node kinds, per-node spans, resolved xref destinations). These are net-new, not
  replacements.
- The alternate test renderer already in the suite
  ([`tests/inline_substitution_renderer.rs`](../../parser/src/tests/inline_substitution_renderer.rs))
  becomes the template for testing the reshaped seam in Phase 5.
- Only in a final cleanup (post-landing, optional) would any HTML-string test be restated
  structurally — and only where the structural form is strictly more precise.

### 5.5 Risks and mitigations

| Risk                                                                 | Mitigation                                                                                 |
| -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| HTML output drifts during the interior rewrite                        | Golden oracle (5.3) gates every phase; differential harness over all fixtures.             |
| Two node trees fold to identical HTML, masking a structural bug       | Cross-check against the recorder in Phase 1; add structural assertions in Phase 3.          |
| Performance regression (extra allocation for nodes vs. one string)    | `'src` borrowing for `Text`/`Raw`; single-node fast path for plain paragraphs; benchmark gate in Phase 2 (existing Criterion/CodSpeed benches). |
| Long-running branch drifts from `main`                                | Frequent `main`→branch merges (never rebase); land as one merge commit preserving staged history. |
| Public API pinned before a consumer proves it out                     | Keep `inlines()` behind the phase gate; align field sets with the `asciidoctor` port (§6.6), per #943's discipline. |
| ASG under-models crate constructs, forcing lossy projection           | Keep native rich nodes; document projection choices; treat ASG as an output, not the internal ceiling. |
| Attribute-expansion / passthrough span provenance is genuinely hard   | Ship Phase 3 with coarse fallback spans (shape is span-ready); defer precision to Phase 4 with #944's explicit policies. |

---

## 6. Decisions

*Accepted by the maintainer on 2026-08-02.* These were open questions in the initial draft;
the recommendations below were reviewed and adopted, and now stand as decisions for the
implementation.

1. **Node text: logical vs. rendered vs. source?** → **Logical** (reader's characters),
   with escaping deferred to the fold. This is what enables ASG conformance and clean
   backends. *(Resolves the #943 open question.)*
2. **Model every macro, or a catch-all?** → **Model** image / footnote / xref / link /
   anchor / kbd / btn / menu / index term / callout / stem as named variants (we already
   have the data for each from the renderer params); avoid the prototype's `Macro{kind,
   text}` catch-all, which loses structure. Reassess only if a construct proves to have no
   consumer need.
3. **Owned vs. borrowed strings?** → **`CowStr<'src>`**, borrowed by default.
4. **Spans from day one?** → **Yes**, as a field on every node, populated coarsely first
   (§4.4). Avoids a breaking reshape later.
5. **Retain the rendered-string accessor?** → **Yes**, renamed to `rendered_html()`, as a
   cached **default-HTML** fold, with custom backends routed through an explicit
   `render_with`/`render_to` and the parse-time renderer config dropped (§3.3.1) — this is
   the "approximate the existing rendered-content model" bonus, achieved for free.
6. **Which downstream tool pins the API?** → the **Ruby-to-Rust `asciidoctor` port** — the
   only consumer actually underway. It reproduces Asciidoctor's HTML exactly, so it walks
   and renders the *entire* inline vocabulary (images, footnotes, xrefs, callouts, UI
   macros, index terms, STEM, passthroughs) and exercises the renderer seam comprehensively.
   That makes it an excellent oracle for the **node kinds and the `InlineRenderer` seam**,
   and it doubles as a strong byte-exact parity check (§5.3).
   - *Caveat:* because the port mostly *renders* nodes rather than *re-flowing* them, it
     pins the seam and the node vocabulary far more than the purely-structural navigation
     sugar on `inlines()`. The re-flow consumers that would exercise that — the Zola
     backend, spec-coverage, and version-diff tools — have **not started**, so they inform
     the conceptual shape only (as reflected in §1.3). Per the #943 discipline, keep the
     structural-navigation conveniences minimal and let them be pinned by those consumers
     when they materialize; do not finalize them by guessing now.

---

## 7. The "bonus": approximating the rendered-content model

The prompt notes that approximating the current rendered-content model with the new
architecture is a bonus. This design achieves it **exactly, not approximately**:
`rendered_html()` (today's `rendered()`, renamed — §3.3.1) survives as a fold over the AST,
and the golden-HTML oracle guarantees it is byte-for-byte identical to today's output.
Consumers that only want the string keep the same one-line call (under the new name);
consumers that want structure get `inlines()`. The rendered model is
not a parallel artifact to keep in sync — it is a *view* of the canonical tree, so the
drift risk that plagued the #942 prototype is designed out.

---

## 8. Relationship to existing issues

- **#892** — this is its resolution (structured inline access), taken further to canonical.
- **#943** — subsumed: the read-only tree becomes canonical rather than additive. Its open
  questions are answered in §6.
- **#944** — Phase 4 is its "single-pass AST by span containment," and this design adopts
  its convergence framing.
- **#564** — retired in Phase 4 once nodes carry precise spans.
- **#942** — its `InlineNode` shape and recording renderer are reused as prior art and as
  the Phase 1 bring-up oracle; its known limitations (owned strings, no spans, double pass,
  drift) are the specific things Phases 2 and 4 eliminate.

## References

- Eclipse AsciiDoc Language ASG schema:
  <https://gitlab.eclipse.org/eclipse/asciidoc-lang/asciidoc-lang/-/blob/main/asg/schema.json>
- AsciiDoc substitutions: <https://docs.asciidoctor.org/asciidoc/latest/subs/>
- Internal issues: #892, #942, #943, #944, #564.
