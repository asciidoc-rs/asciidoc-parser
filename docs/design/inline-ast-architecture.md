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
  `render_to` fold is the last Phase 3 piece — but attempting it revealed that a *faithful*
  fold needs the per-construct attrlist/parser back at fold time, which the `'static`
  Strategy-A recorder cannot carry into a node (every `Styled`/`Image` node is therefore
  built with `attrs: None`). The self-describing nodes such a fold needs come from the
  Phase 4 single-pass builder, so `render_with` / `render_to` is **resequenced to land after**
  that builder covers the inline vocabulary (see Phase 4's step list).

- **Phase 4 — precision spans + single-pass builder + ASG output.** 🔶 **In progress.** Land
  the single-pass builder (Strategy B) so the tree is built **directly from `'src`** — nodes
  carrying honest per-node spans (#944) and their own `Attrlist<'src>` (self-describing) —
  then make `rendered_html()` a fold of the tree, add `Document::to_asg()`, validate against
  the ASG schema, and retire the `attribute-missing` per-line hack (#564).
  *Exit:* ASG output validates; #944 hard-case policies documented and tested; #564 hack
  removed.

  *Step 1 landed as (the single-pass builder foundation + `SpecialCharacters`):* a new
  [`inline_builder`](../../parser/src/content/inline_builder.rs) module recasts a
  substitution step as a **transducer** over a node list (`Vec<InlineNode> → Vec<InlineNode>`) —
  the shape §4.1 describes. [`build`](../../parser/src/content/inline_builder.rs) seeds one
  borrowed whole-source `Text` node and threads it through the steps;
  [`apply_special_characters`](../../parser/src/content/inline_builder.rs) splits each `Text`
  run on `<`/`>`/`&` into precise-span `Text` and `CharRef::Special` nodes, sliced with the
  crate's own [`Span`](../../parser/src/span/slice.rs) primitives so each node's
  `line`/`col`/`offset` is honest (the precise spans #944 targets) and verbatim runs borrow
  from `'src`. [`fold_html`](../../parser/src/content/inline_builder.rs) is the **first fold
  over the public `InlineNode` tree** — the recorder's `fold_into` folds an intermediate
  representation, not the public tree — and is the seed of both `rendered_html()`-as-a-fold
  and `render_with`. This step is **additive**: nothing is wired into the parse path, so the
  string pipeline and the Strategy-A [`inlines()`](../../parser/src/content/content.rs) tree
  are untouched, and a differential test asserts the fold reproduces the string pipeline's
  special-characters output byte-for-byte, alongside precise-span assertions the Strategy-A
  tree cannot make.

  *Step 2 landed as (`Quotes` → `Styled`, introducing nesting):*
  [`apply_quotes`](../../parser/src/content/inline_builder.rs) recasts the quoted-text step as
  a transducer: it reuses the *exact* recognition rules the string pipeline matches with —
  [`quote_subs`](../../parser/src/content/substitution_step.rs), now shared `pub(crate)` — so
  the recognition is unchanged and only the *sink* differs (§4.1). Each rule is applied to the
  node tree in order; before matching at a level the transducer descends into the
  [`Styled`](../../parser/src/inlines/styled.rs) spans earlier rules created, so a later rule
  can match *inside* an earlier span — which is what makes `*a _b_ c*` and `*a `b` c*` nest
  into a tree. Matching runs over an **escaped working string** rebuilt from the level's leaves
  (a `CharRef` contributes its canonical entity, so the boundary classes the patterns key off
  — `&`, `;` — see exactly what the string pipeline's escaped text presents; an earlier span is
  one opaque placeholder), and each match maps back to precise `'src` spans: delimiters are
  consumed, the boundary prefix is kept, and an attributed span (`[.role]#…#`) parses and
  **retains its own `Attrlist<'src>`** (self-describing — better than the recorder's
  `attrs: None`), so [`fold_html`](../../parser/src/content/inline_builder.rs) renders it
  through the same `render_quoted_substitution` the string step calls. A broad differential
  corpus asserts the fold reproduces the string pipeline's output through the quotes step
  byte-for-byte (nesting, unconstrained forms, smart quotes, super/subscript, roles/ids,
  escapes, specials adjacent to delimiters, multi-line runs), alongside structural precise-span
  assertions the Strategy-A tree cannot make. The one intended divergence — *crossed* delimiters
  (`` `a *b` c* ``) whose overlapping ranges the string pipeline renders as malformed,
  improperly-nested tags that no tree can represent — is documented and pinned by a test: the
  builder seals the inner delimiter inside its opaque span and stays well-formed. This step is
  **additive**: nothing is wired into the parse path.

  *Step 3 landed as (`CharacterReplacements` → `CharRef`, `PostReplacement` → `LineBreak`):*
  two more transducer steps.
  [`apply_character_replacements`](../../parser/src/content/inline_builder.rs) recognizes the
  typographic replacements — `(C)`/`(R)`/`(TM)`, em dashes, ellipsis, apostrophes, arrows, and
  restored entities — replacing each with a
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
  the largest — seven construct families plus deferred-xref recording and footnote text
  extraction — so it lands as its own sequence of sub-steps rather than one leap. The first,
  [`apply_macros`](../../parser/src/content/inline_builder.rs), recognizes **image and icon
  macros** (`image:target[…]`, `icon:target[…]`), replacing each with an
  [`Image`](../../parser/src/inlines/image.rs) node that captures its own owned
  `Attrlist<'src>` — the step that makes a macro node **self-describing**, the property a
  faithful fold needs (§3.3.1, Phase 3 step 2). It reuses the string pipeline's *exact*
  recognition, now shared `pub(crate)`
  ([`INLINE_IMAGE_MACRO`](../../parser/src/content/macros.rs)), so only the *sink* differs
  (§4.1); it descends into `Styled`/`Ref` children (a macro can sit inside a rendered span),
  pre-extracts the alt/width/height (`icon:` carries a `size`, read back from `attrs` at fold
  time) the way the string replacer does, and honors the `\image:` escape. An `is_icon` flag is
  added to the `Image` node so the two forms fold through the right renderer method. The
  [`fold_html`](../../parser/src/content/inline_builder.rs) fold gains a `&Parser` argument —
  rendering an image reads the document's safe mode, `data-uri`, and `icons`/`icontype`
  attributes — and reconstructs `ImageRenderParams`/`IconRenderParams` to call the same
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
  macros** — keyboard (`kbd:[…]`), button (`btn:[…]`), and menu (`menu:…[…]`) — replacing each
  with a [`Ui`](../../parser/src/inlines/ui.rs) node that carries the split keys / normalized
  label / menu path the string replacer computes. It reuses the string pipeline's *exact*
  recognition and splitting, now shared `pub(crate)`
  ([`INLINE_KBD_BTN_MACRO`](../../parser/src/content/macros.rs),
  [`INLINE_MENU_MACRO`](../../parser/src/content/macros.rs), `split_kbd_keys`,
  `normalize_index_text`), so only the *sink* differs (§4.1). Like the string step it runs the
  families **in order** — keyboard/button, then menu, then image/icon — and recognizes the UI
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
  it fails the verbatim boundary and is left **unrecognized** for a later increment — documented
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
  replace only a *sub-range* of its match — keeping a kept prefix before it and stripped
  punctuation after it — which the image/UI/`link:` families use degenerately (the node consumes
  the whole match). Because macros are matched over *escaped, already-rendered* text, a link whose
  URL crosses a special (`&`) or a rendered span is left **unrecognized** for a later increment,
  exactly the verbatim boundary step 4a documents; the angle-bracketed URL form (`<url>`) needs a
  leading `&lt;` and so is always non-verbatim, and a text carrying an attribute list (`=`, or a
  `mailto:` `,` subject) is deferred until the node can hold an `Attrlist<'src>`. The `link:` URL
  macro form (`link:https://…[…]`) is left to the `INLINE_LINK_MACRO` pass, which folds the
  identical node — the two passes run in the string step's order (`INLINE_LINK` before
  `INLINE_LINK_MACRO`). Differential corpora pin the verbatim link forms (labeled / bare / pathed,
  `mailto:`, other schemes, the `^` suffix, escapes, `hide-uri-scheme`, links next to and inside
  spans) byte-for-byte, alongside structural precise-span assertions the Strategy-A tree cannot
  make and divergence tests for each deferred form. These steps are **additive**: nothing is wired
  into the parse path. Cross-references (`Ref{Xref}`), footnotes, index terms, STEM, and anchors
  remain later sub-steps.

  *Follow-up landed as (the link family's own attribute-list-bearing display text, closing
  `Ref{Link}`'s last deferred form):* both link-recognizing passes now recognize a display text
  carrying an `=` (`link:x[text,role=hl]`, `https://x[text,role=hl]`) and a `mailto:` text
  carrying a `,` subject/body (`mailto:x[Team,Hello there]`) — the pair step 4b(ii) itself defers,
  and the mirror of the `xref:` macro's own attribute-list text (part 3c below). Unlike that xref
  form, a link's attribute list cannot be reduced to a few plain fields: `render_link` reads an
  `id`, a `title`, and the `nofollow`/`noopener` options straight off the `Attrlist` itself (not
  just `roles`/`window`, which `XrefRenderParams` alone needs), so `Ref` gains an
  `attrs: Option<Attrlist<'src>>` field — `None` unless a `Link`'s display text carried its own
  attribute list, always `None` for an `Xref`. This is also what step 4b(ii)'s own deferral notes
  as the blocker ("deferred until the node can hold an `Attrlist<'src>`"): the string replacers
  parse the attrlist from a *newline-normalized copy* of the text (so a multi-line
  `link[Foo\nBar,role=x]` reads as `Foo Bar`), which cannot become an honestly-borrowed
  `Attrlist<'src>` — but when the text has **no embedded newline**, that copy is byte-identical to
  the bracketed text's own `'src` slice, so the node can parse the *real* source span instead and
  carry a genuine borrow. A text that does embed a newline still needs the synthesized copy the
  node cannot hold, so that one narrow form remains deferred (pinned by its own divergence test,
  for both the `=` and `,` cases). Both call sites reuse
  [`extract_attributes_from_text`](../../parser/src/content/macros.rs) (now shared `pub(crate)`,
  its own signature relaxed from `&'src Span<'src>` to `Span<'src>` since `Span` is `Copy` and the
  reference added nothing the by-value form doesn't already give a caller building a node with the
  *same* `'src` as its input) and
  [`encode_uri_component`](../../parser/src/content/macros.rs) (likewise now shared `pub(crate)`,
  for the `mailto:` subject/body encoding), so the interpretation — including the "incidental `=`"
  fallback (`extract_attributes_from_text`'s own guard) and, for the `link:`/`mailto:` macro form,
  the exact unconditional-adoption behavior `InlineLinkMacroReplacer` has and
  `InlineLinkReplacer` does not (see the two call sites' own doc comments) — is reused byte-for-byte
  rather than re-derived. [`fold_link`](../../parser/src/content/inline_builder/fold.rs) now passes
  the node's own `attrs` through to `render_link` when present, falling back to the empty attrlist
  every other link already folds through. A display or reference text crossing a rendered span
  (`link:x[*bold*]`, `xref:id[*bold*]`, `<<id,*bold*>>`) remains a **separate**, still-fully-open
  boundary for every reference-bearing family (not touched by this follow-up): by the time macros
  run, `*bold*` is already a `Styled` node, not text, so recognizing it would need the node's
  display text to become structured children — the shape a footnote's own content already has —
  which no reference family has yet grown; each now carries its own divergence test pinning this
  (previously only the `<<id,*bold*>>` shorthand had one). Differential corpora extend the existing
  link/formal-URL fixtures with `role=`/multi-attribute and mailto subject/body combinations,
  alongside the incidental-`=` case and the multi-line divergence.

  *Step 4b(ii) part 3a landed as (`Macros` → `Ref{Xref}`, the same-document `xref:` macro form):*
  the builder now recognizes the **`xref:` cross-reference macro** (`xref:id[]`,
  `xref:id[Reference Text]`), each as a [`Ref`](../../parser/src/inlines/ref_node.rs)`{Xref}` node.
  It reuses the string pipeline's *exact* recognition — `INLINE_XREF` is now shared `pub(crate)` —
  so only the recognition *sink* changes (a node instead of the string step's deferred
  `XrefSegment` placeholder). As with links, no field is added to `Ref`: the provided text is baked
  into a single [`Text`](../../parser/src/inlines/inline_node.rs) child (verbatim text borrows from
  `'src`; an escaped `\]` synthesizes an owned value), and an empty text yields no children, which
  the fold reads as "no text provided" (the bracketed `[id]` fallback). The
  [`fold_html`](../../parser/src/content/inline_builder.rs) fold reconstructs `XrefRenderParams`
  from the node and routes through the same `render_xref` the string path feeds at resolution time,
  so its output is byte-for-byte identical — pinned by a new differential corpus that finalizes the
  string pipeline's deferred cross-references to the same **unresolved** fallback the additive
  builder (no resolution pass) produces. Because the additive builder never resolves, the fold
  always takes `render_xref`'s unresolved branch, where `xrefstyle` and a *derived* destination play
  no part; wiring resolution to the tree is the cutover's job (step 6). Four forms are deferred,
  each documented and pinned by a divergence test: the **shorthand** (`<<id>>`, always non-verbatim
  because its `&lt;`/`&gt;` delimiters are `CharRef`s by macro time, exactly as the angle-bracketed
  `<url>` link defers), an **inter-document** target (`xref:other.adoc#frag[]`, whose derived
  destination the node cannot carry yet), a **text carrying an attribute list** (`=`, parsed into
  `window`/`role`/`xrefstyle`, deferred until the node can hold an `Attrlist<'src>` — as the
  formal-URL link defers the same), and a macro whose **target or text crosses a special character
  or a rendered span** (`xref:foo[a<b]`, matched by the string pipeline over the *escaped* text,
  which a self-describing node cannot carry as an `'src` slice — the same verbatim boundary the
  image and auto-link increments document). This step is **additive**: nothing is wired into the
  parse path.

  *Step 4b(ii) part 3b landed as (`Macros` → `Ref{Xref}`, the same-document `<<id>>` shorthand):*
  the builder now recognizes the **shorthand cross-reference** (`<<id>>`, `<<id,Reference Text>>`)
  as the same [`Ref`](../../parser/src/inlines/ref_node.rs)`{Xref}` node the `xref:` macro form
  produces, folding through the identical `render_xref` so the output is byte-for-byte identical
  (pinned by extending the part-3a differential corpus with shorthand fixtures). It reuses the
  *same* shared `INLINE_XREF` pattern — the shorthand and macro forms are two branches of one
  regex — so recognition is unchanged and only the *sink* differs. The shorthand's key wrinkle is
  that, because special characters run before macros, its `<<`/`>>` delimiters are already escaped
  [`CharRef`](../../parser/src/inlines/char_ref.rs)s (`&lt;&lt;`/`&gt;&gt;`) by macro time, so the
  match is never *wholly* verbatim the way a `xref:` macro is. The recognizer therefore relaxes its
  verbatim gate for this one form: the delimiters are `CharRef`s the node **consumes** (dropped by
  [`rebuild_macro_level`](../../parser/src/content/inline_builder.rs), which emits atomic pieces
  whole) rather than slices, so only the shorthand's *inner* text — the id and any reference text —
  need be verbatim to slice from `'src`. The inner is split on the first `,` into an id and an
  optional reference text, each **trimmed** exactly as the string replacer's shorthand branch does;
  the trimmed text becomes the node's single [`Text`](../../parser/src/inlines/inline_node.rs) child
  (borrowing `'src`), an absent one yields no children (the bracketed `[id]` fallback), and the whole
  `<<…>>` is the node's `location`. An escaped `\<<…>>` drops its backslash and stays literal, handled
  before the verbatim gate so it works even across a non-verbatim inner. Four forms are deferred, each
  documented and pinned by a divergence test: an **inter-document** shorthand (`<<other#frag>>`, whose
  derived destination the node cannot carry yet — the same block as the inter-document `xref:` form),
  a **document-as-a-whole** shorthand (`<<>>`, an empty id resolving through a derived destination, as
  `xref:#[]` defers), a **`<<id,>>` with an empty reference text** (which the string replacer records
  as a *present-but-empty* text — an empty `<a>…</a>` — that an empty child vector cannot distinguish
  from "no text provided"), and a shorthand whose **id or text crosses a special character or a
  rendered span** (non-verbatim inner, the same boundary the macro form and the image/auto-link
  increments document; this also subsumes the string replacer's "id already contains rendered markup"
  guard, since such an id is non-verbatim). This step is **additive**: nothing is wired into the parse
  path. The two node-blocked forms both spellings share — inter-document targets and attribute-list
  text — remain for a later increment (part 3c), which needs new `Ref` fields pinned against a
  consumer.

  *Step 4b(ii) part 3c landed as (`Ref` grows a `derived` field, closing the inter-document half):*
  part 3c turned out to split cleanly along its own two deferred forms — an inter-document/
  document-as-a-whole target needs only a *destination* the node can carry opaquely (the exact
  `DerivedReference` type [`ResolutionContext`](../../parser/src/parser/reference_resolver.rs)
  already produces for the string pipeline's own non-catalog case), while an attribute-list-bearing
  display text needs the node to parse and hold `window`/`role`/`xrefstyle`. The first needed no
  "consumer" to pin its shape — it is a straight reuse of an existing, already-public type — so it
  lands now; the second (part 3c's attribute-list half) remains deferred, still needing a real
  consumer to pin how (or whether) `Ref` grows an `Attrlist<'src>`.
  [`Ref`](../../parser/src/inlines/ref_node.rs) gains `derived: Option<DerivedReference>` —
  `None` for a same-document reference (unchanged: still resolves through the catalog at the
  cutover) and `always None` for a [`Link`](../../parser/src/inlines/ref_node.rs), populated only
  for a cross-reference whose target carries its own destination. A new
  `xref_target_and_derived` helper in
  [`xref.rs`](../../parser/src/content/inline_builder/macros/xref.rs) mirrors
  `InlineXrefReplacer::replace_append`'s own target-interpretation match *exactly* — including its
  "a target naming this document, or a file included into it in full, is a same-document reference
  after all" special case (`Parser::docname`/`Parser::catalog_include_is_full`) — so both the
  `xref:` macro form and the `<<id>>` shorthand (which shares the helper, differing only in
  `macro_form`) now recognize every target shape: a same-document id, an inter-document target
  (`xref:other.adoc#frag[]`, `<<other#frag>>`), and the document-as-a-whole form (`xref:#[]`,
  `<<>>`). [`fold_xref`](../../parser/src/content/inline_builder/fold.rs) reconstructs
  `XrefRenderParams.derived` straight from the node, so a derived-carrying reference now folds
  through `render_xref`'s `(None, Some(derived))` branch instead of the unresolved fallback,
  byte-for-byte identical to the string pipeline (`xrefstyle` stays `None` — it plays no part in
  that branch, and the attribute-list half that would populate it for other cases is still
  deferred). As throughout this module, this performs *no* recognition side effect and nothing is
  wired into the parse path. A differential corpus extends the existing xref fixtures with
  inter-document (with and without a fragment, and a non-AsciiDoc extension kept as-is) and
  document-as-a-whole forms in both spellings, alongside a unit test pinning the
  "target names this document" special case. The one remaining deferred xref form — a text
  carrying an attribute list (part 3c's other half) — is unchanged, still pinned by its own
  divergence test.

  *Step 4b(ii) part 3c (attribute-list half) landed as (`Ref` grows `xrefstyle`, closing the last
  deferred xref form):* the `xref:` macro's own remaining deferred form — a bracketed text
  carrying an attribute list (an `=`, for `window`/`role`/`xrefstyle`) — is now recognized.
  [`xref_macro_text`](../../parser/src/content/inline_builder/macros/xref.rs) mirrors
  `InlineXrefReplacer::replace_append`'s own text interpretation exactly: it parses the text — from
  a newline-normalized copy, since the parse is not necessarily verbatim, exactly as the string
  replacer parses that same normalized copy rather than a source slice — as an
  [`Attrlist`](../../parser/src/attributes/attrlist.rs) whose first positional attribute becomes the
  display text and whose `window`/`role` named attributes populate the node's existing fields; a new
  [`Ref::xrefstyle`](../../parser/src/inlines/ref_node.rs) field (`Option<XrefStyle>`) carries a
  `xrefstyle=` override. As with the `<<other#frag>>` half, this needed no consumer to pin its
  shape — `window`/`roles` already existed as plain fields (not an `Attrlist<'src>`) because
  [`XrefRenderParams`](../../parser/src/parser/inline_substitution_renderer.rs) itself takes them
  that way, not a borrowed attribute list, so the node stores exactly what the fold already needs.
  When the attrlist parse finds no named attribute — the sole positional value is the whole
  normalized text — the `=` was incidental (mirroring Asciidoctor's own
  `extract_attributes_from_text` fallback); the text is then used as plain display text with no
  named attributes, exactly as if it carried no `=` at all. The parsed positional text becomes a
  *synthesized* `Text` child (no `'src` slice of its own, since it comes from the normalized,
  attrlist-parsed copy) whose location falls back to the bracketed text's own span (design §4.4),
  the same synthesized-value policy `apply_attribute_references` (step 5b) already established.

  Landing this also closed a latent gap the fold carried since the cross-reference increment first
  landed (part 3a): the *document-wide* `xrefstyle` attribute — which applies to every reference,
  not only one carrying its own `xrefstyle=` override — was never applied at all, because
  [`fold_xref`](../../parser/src/content/inline_builder/fold.rs) hard-coded `xrefstyle: None`. It now
  combines the node's own override with the document-wide default exactly as
  `InlineXrefReplacer` does (`xrefstyle_override.or_else(|| document_xrefstyle(parser))`), reusing
  the string pipeline's own [`document_xrefstyle`](../../parser/src/content/macros.rs) helper (now
  shared `pub(crate)`) rather than a second implementation. The `<<id>>` shorthand has no
  attribute-list text of its own — its node's `xrefstyle` override is always `None` — but still
  observes the document-wide default through this same fold-time combination, closing the sub-step
  in full: every form the design names for part 3c is now recognized. A differential corpus extends
  the existing xref fixtures with `role=`/`window=`/`xrefstyle=` combinations, a positional-text-free
  attribute list, and the incidental-`=` case (verified reachable in the builder's own verbatim-gated
  matching, unlike the nested-macro-substitution route the string pipeline's own incidental case
  takes); hand-built-node tests in `fold.rs` pin the document-wide-default and override-precedence
  behavior directly, since neither is observable through an unresolved fold (the tree does not yet
  reach catalog resolution — that remains step 6's job).

  *Step 4b(ii) part 4a landed as (`Macros` → `Anchor`, inline anchors):* the builder now recognizes
  **inline anchors** in both spellings — the `[[id]]` / `[[id,reftext]]` shorthand and the
  `anchor:id[reftext]` macro — as an [`Anchor`](../../parser/src/inlines/anchor.rs) node, folding
  through the same `render_anchor` the string step calls so the output is byte-for-byte identical
  (pinned by a new differential corpus). It reuses the string pipeline's *exact* recognition —
  [`INLINE_ANCHOR`](../../parser/src/content/macros.rs) is now shared `pub(crate)` — so only the
  recognition *sink* changes (a node instead of rendered markup). Taken out of pipeline order (it is
  listed last under part 4) precisely because it is the cleanest of the remaining families: an
  anchor's rendering (`<a id="…"></a>`) is a function of its **id alone**, and the pattern admits no
  special character in an id, so an id is always verbatim and an anchor is **always recognized** —
  there is no deferred-output boundary the way the link and cross-reference families have one. The id
  borrows from `'src` and the whole `[[…]]` / `anchor:…[…]` is the node's `location`. An escaped
  `\[[…` / `\anchor:…` drops its backslash and stays literal, and — because the id is always verbatim
  while a reference text may not be — the unescape needs no verbatim gate at all.

  The optional reference text becomes the node's `reftext` — a single
  [`Text`](../../parser/src/inlines/inline_node.rs) child — **when it is verbatim** (borrowing
  `'src`; a shorthand's trailing whitespace is trimmed and a macro's escaped `\]` is unescaped into an
  owned value, mirroring the string replacer). A reference text carrying a rendered span or an escaped
  special is *non-verbatim*; because it never reaches the flow (the anchor renders from its id alone),
  such an anchor is still recognized and rendered — the whole match, the rendered-span reference text
  included, is **consumed** by the node — but its `reftext` is left `None` rather than sliced wrongly
  from `'src`. This is the same verbatim boundary the other macro families document, expressed here as
  a node field the fold ignores rather than as a deferred construct; the field stays provisional
  pending a re-flow consumer (design §6.6). As the additive builder does throughout, the anchor pass
  performs *no* recognition side effect — it does **not** `register_ref` the id in the reference
  catalog (so a cross-reference can resolve against it), nor raise the duplicate-id warning the string
  replacer does; those, and the bibliography-anchor form (`[[[id]]]`, which the string step recognizes
  only inside a bibliography list item — a context the additive builder is not wired into), are the
  cutover's job (step 6). This step is **additive**: nothing is wired into the parse path. The
  remaining macro families (`Footnote`, `IndexTerm`, `Stem`) and the node-blocked cross-reference
  forms (part 3c) are later sub-steps.

  *Follow-up landed as (an anchor id inside an expanded attribute value, a latent correctness gap):*
  an audit of every macro family's own verbatim gate, prompted by the design's own "a macro inside an
  expanded value" boundary (§3.4.1, §4.1's `apply_macros` note) still being open for this family after
  step 5b closed it for `CharacterReplacements`, surfaced that part 4a's own claim — "an id is always
  verbatim … an anchor is *always* recognized" — was true only against the escaped-special/rendered-span
  boundary every other macro family documents, not against the *synthesized* one: unlike every other
  family, [`find_anchor_matches`] never checked the id capture against [`range_is_verbatim`] before
  slicing it, so an attribute reference whose expanded value happened to contain `[[id]]` (e.g.
  `:myattr: [[custom-id]]` then `{myattr}`)
  built an [`Anchor`](../../parser/src/inlines/anchor.rs) node whose `id`/`location` silently fell back
  to the *enclosing synthesized run's* coarse span (`{myattr}` itself) rather than the real id text —
  a wrong node, not a documented divergence, though unreachable from any real parse today since this
  module is not yet wired in (§5.2 Phase 4 step 6). [`build_anchor_node`] now checks the id's own range
  with the same [`range_is_verbatim`] every other family already uses and returns `None` when it fails,
  leaving the anchor unrecognized for a later increment exactly like every other family's own boundary —
  closing the gap between the doc comment's claim and the code. A new divergence test pins the exact
  scenario that exposed it (an attribute expanding to `[[custom-id]]`), alongside the golden pipeline's
  own confirmation that it *does* recognize the anchor once the value is spliced in, so a future
  boundary-lifting increment that fixes this for real has a corpus fixture ready to move out of the
  divergence test and into a parity one.

  [`find_anchor_matches`]: ../../parser/src/content/inline_builder/macros/anchors.rs
  [`range_is_verbatim`]: ../../parser/src/content/inline_builder/macros/image.rs

  *Step 4b(ii) part 4b landed as (`Macros` → `IndexTerm`, index terms):* the builder now recognizes
  **index terms** in both spellings — the `((term))` / `(((primary, secondary, tertiary)))` shorthand
  and the `indexterm:[…]` (concealed) / `indexterm2:[…]` (flow) macro — as an
  [`IndexTerm`](../../parser/src/inlines/index_term.rs) node, folding through the same
  `render_index_term` the string step calls so the output is byte-for-byte identical (pinned by a new
  differential corpus). It reuses the string pipeline's *exact* recognition —
  [`INLINE_INDEXTERM`](../../parser/src/content/macros.rs) is now shared `pub(crate)`, alongside the
  `strip_see_and_seealso` helper — so only the recognition *sink* changes. Like the anchor increment,
  a **concealed** term renders to nothing (a function of no shown text), so it is recognized regardless
  of what its argument crosses and its node carries an empty `terms`; a **visible** term shows its
  text in the flow and is recognized whenever that text — reconstructed from the level's escaped match
  string, so a `CharRef` entity or a stripped `see`/`see-also` clause (`&gt;&gt;` / `&amp;&gt;` by
  macro time) is handled as parity, not a divergence — crosses no opaque span. The node mirrors the
  Strategy-A recorder's shape (the shown term for a visible node, empty for a concealed one), leaving
  the richer primary/secondary/tertiary structure to a re-flow consumer to pin (the field is
  provisional, per the node's Phase-0 note). The shorthand reproduces the string replacer's
  trailing-`)` absorption (its `(?!\))` look-ahead re-creation) by folding the absorbed parens into
  the match, and keeps a literal parenthesis adjacent to a term via the shared `MacroMatch` sub-range
  seam. One subtle string-pipeline behavior is reproduced exactly: a level of *only* concealed
  shorthand terms (`(((coffee)))`, `(((a)))(((b)))`) accumulates no output and ends in a look-ahead
  retry, so [`replace_with_lookahead`](../../parser/src/internal/regex.rs) returns `Cow::Borrowed` and
  the string step leaves it **literal** — the builder detects that no-op and mirrors it. Two forms are
  deferred, each documented and pinned by a divergence test: a **visible term crossing a rendered
  span** (unreconstructable from the escaped string, the same verbatim boundary the other macro
  families document) and an **`indexterm2:[…]` carrying an attribute list** (an `=`, deferred until
  the node can hold an `Attrlist<'src>`, as the link/xref macros defer the same — deferred at the
  time, closed by a step 6 prep below, which finds the node needs no such list at all); the one
  escaped paren-wrapped shorthand the string replacer re-renders (`\(((x)))` → `(x)`) is likewise
  left literal — also at the time, and also closed by a step 6 prep below, as a *pair* of matches
  rather than a new node shape.
  As throughout the additive builder, this performs *no* recognition side effect (the HTML backend
  builds no index, so the string replacer has none to skip either). This step is **additive**: nothing
  is wired into the parse path. Inline `Stem` is handled at passthrough time (step 5), not in the
  macros step.

  *Step 4b(ii) part 4c landed as (`Macros` → `Footnote`, the last macro family):* the builder now
  recognizes **footnotes** (`footnote:[…]`, `footnote:id[…]`, `footnote:id[]`) as a
  [`Footnote`](../../parser/src/inlines/footnote.rs) node, folding through the same `render_footnote`
  the string step calls so the output is byte-for-byte identical (pinned by a new differential
  corpus). It reuses the string pipeline's *exact* recognition —
  [`INLINE_FOOTNOTE_MACRO`](../../parser/src/content/macros.rs) is now shared `pub(crate)`, alongside
  the `normalize_footnote_text` helper — so only the recognition *sink* changes, and runs **last** in
  `apply_macros`, after cross-references, mirroring the string step's order exactly: a footnote's text
  is extracted from the flow, so any construct an earlier pass at this same level already recognized
  (an image, a link, an anchor, an index term, or now a cross-reference) is captured as *that
  construct's node* rather than being re-recognized from its source text.

  Two things set this increment apart from every prior macro family:

  - **Structured content, not a literal value.** A footnote's bracket content becomes the node's
    `children` via [`emit_range`](../../parser/src/content/inline_builder.rs) rather than a literal
    `'src` slice gated by [`range_is_verbatim`](../../parser/src/content/inline_builder.rs) the way a
    target or display text is elsewhere. A content range crossing an already-recognized construct is
    therefore *not* a boundary to defer on — nesting is the point, and `emit_range` clones that
    construct's node whole into the footnote's subtree, exactly mirroring how the string pipeline's
    footnote text captures an already-substituted macro verbatim.
  - **One *required* recognition side effect.** Every prior macro family performs *no* recognition
    side effect (no catalog registration, no warning), deferring that to the cutover (step 6) because
    omitting it does not change the fold's output bytes. A footnote's marker digits *are* the assigned
    footnote number, so this pass must call `Parser::footnote_index_for_id` / `Parser::define_footnote`
    — the same document-counter-advancing calls the string replacer makes — or the differential corpus
    could never pass. The two code paths never share a `Parser` (each independently numbers footnotes
    over the same source in the same left-to-right order), so this never double-counts a registration.
    The registered catalog `text` is a best-effort normalized rendering of the raw bracket content, not
    a fold (building the tree must not itself invoke a renderer), so — like every other deferred
    registration in this module — a tree-built footnote's `Document::catalog().footnotes()` entry is
    not yet byte-faithful; only the returned *number* is relied on.

  Two forms are deferred, each documented and pinned by a divergence test: the deprecated
  `footnoteref:[id,text]` / `footnoteref:[id]` form (which packs its id and text into one bracket, split
  differently, and — outside `compat-mode` — raises a deprecation warning neither of which this
  increment implements), and content carrying an escaped closing bracket (`\]`, which would need
  splicing a literal `]` into the middle of a `Text` piece the content range slices — a rebuild this
  increment does not attempt). This step is **additive**: nothing is wired into the parse path. With it,
  every macro family the recorder covers now has a single-pass counterpart.

  *Follow-up landed as (the deprecated `footnoteref:` form):* the first of part 4c's two deferred forms
  is closed. [`build_footnoteref_node`](../../parser/src/content/inline_builder/footnotes.rs) mirrors
  `InlineFootnoteMacroReplacer`'s own `raw.split_once(',')` exactly — `footnoteref:[id,text]` /
  `footnoteref:[id]` packs both into one bracket rather than taking the id from the macro target the way
  `footnote:id[…]` does, splitting on the *first* comma so a bracket with no comma is an id-only bare
  reference and a trailing comma (`footnoteref:[id,]`) yields *empty*, not absent, content (a defining
  occurrence with empty text) — a distinct shape from `footnote:id[]`'s own no-comma-at-all reference.
  Once split, the (id, content) pair resolves through the *same* three cases `build_footnote_node`
  already does (reuse an already-defined id's number, define a new id-carrying occurrence, or fall back
  to an unresolved reference for an id never defined), folding through the identical `render_footnote`,
  so the output is byte-for-byte identical to the golden pipeline's (pinned by extending the part 4c
  differential corpus with `footnoteref:` fixtures). The escape check
  (`whole.as_str().starts_with('\\')`) is hoisted ahead of the ref-vs-plain branch in
  `find_footnote_matches`, mirroring the string replacer's own check order exactly — it previously ran
  *after* the (then early-`continue`ing) `footnoteref:` branch, so an escaped `\footnoteref:[…]` was left
  fully unrecognized (backslash and all) rather than unescaped; this is fixed as a side effect of
  recognizing the form at all. As with every other macro family, the one side effect this increment does
  *not* yet perform is the deprecation warning itself (`DeprecatedFootnoterefMacro`) — a diagnostic that
  does not change the fold's output bytes, so — unlike the footnote number, which does — it remains
  deferred to the cutover (step 6) like every other family's own catalog/warning side effect. The one
  remaining deferred form from part 4c's own list, content carrying an escaped closing bracket (`\]`), is
  unchanged and applies identically to `footnoteref:`'s own bracket content (pinned by its own divergence
  test).

  *Step 5a landed as (Passthroughs → `Raw`, the delimited forms):* a new
  [`apply_passthroughs`](../../parser/src/content/inline_builder.rs) step — the **first** step
  [`build`](../../parser/src/content/inline_builder.rs) runs, ahead of `SpecialCharacters` —
  recognizes the triple-plus (`+++…+++`), double-plus (`++…++`), double-dollar (`$$…$$`), and bare
  `pass:[…]` macro (no explicit substitution list) as [`Raw`](../../parser/src/inlines/inline_node.rs)
  leaves, mirroring
  [`Passthroughs::extract_from`](../../parser/src/content/passthroughs.rs), which the string pipeline
  runs *before* its own step loop — so a passthrough's content is never touched by specialcharacters,
  quotes, replacements, or macros: it is a leaf, and every later step's match-string builder already
  treats an unrecognized node kind as one opaque placeholder, exactly as it already does for an
  earlier-built `Styled` span. It reuses the string pipeline's *exact* recognition —
  [`INLINE_PASS_MACRO`](../../parser/src/content/passthroughs.rs) is now shared `pub(crate)` — so only
  the recognition *sink* differs (§4.1). The triple-plus and bare `pass:[…]` forms resolve to
  `SubstitutionGroup::None` (nothing applies), so their content borrows `'src` directly (a `pass:[…]`
  body unescapes an escaped `\]`, as every other macro family's bracket content does, which makes the
  unescaped case owned instead); the double-plus and double-dollar forms resolve to
  `SubstitutionGroup::Verbatim` (special characters only) and are run through the real substitution
  pipeline rather than hand-escaped, so a custom `InlineSubstitutionRenderer`'s escaping is honored —
  the cost is an owned `Raw` value instead of a borrow.

  Three forms are deferred, each documented and pinned by a divergence test: an
  **attribute-list-prefixed** passthrough (`[quotes]++text++`, `[x-]\`text\``, `[attrs]+text+`), a
  **`pass:` macro carrying an explicit substitution list** (`pass:c,q[…]`, whose content would need a
  richer subtree than a single `Raw` leaf can hold — the same reason a footnote's content is
  structured children rather than a literal value), and the **bare unconstrained form** (`+text+`,
  matched by `INLINE_PASS` rather than `INLINE_PASS_MACRO` — its "must not follow a word" boundary
  needs a lookbehind Rust's regex engine cannot express, which the string replacer works around with a
  retry loop this increment does not reproduce). That same deferred boundary shows up once more,
  indirectly: an **escaped triple- or double-plus** (`\+++text+++`, `\++text++`) drops its backslash
  and keeps the delimited text literal here, but the string pipeline's *second* extraction pass
  (`INLINE_PASS`) re-scans that same de-escaped text and consumes its leading `+++`/`++` as a bare
  passthrough wrapping a shorter run — so these two escape forms are pinned as divergences rather than
  folded into the main parity corpus; an escaped `$$…$$` or `pass:[…]` has no such residue and stays
  parity. Inline STEM (`stem:[…]`, `asciimath:[…]`, `latexmath:[…]`) is an implicit passthrough too,
  but folds through its own [`Stem`](../../parser/src/inlines/stem.rs) node rather than `Raw`, so it is
  a separate, later increment. This step is **additive**: nothing is wired into the parse path.

  *Step 5b landed as (`AttributeReferences` → expanded-value splicing):* a new
  [`apply_attribute_references`](../../parser/src/content/inline_builder.rs) step is inserted between
  `Quotes` and `CharacterReplacements` — its position in the *normal* effective order
  (`specialcharacters → quotes → attributes → replacements → macros`, §3.4.1) — so whatever it splices
  into the tree is exactly what the two steps still ahead of it see. It reuses the string pipeline's
  *exact* recognition — [`ATTRIBUTE_REFERENCE`](../../parser/src/content/substitution_step.rs) is now
  shared `pub(crate)` — so only the recognition *sink* differs (§4.1): a reference to a **set**
  attribute has its resolved value spliced into the node stream, classified into
  [`Text`](../../parser/src/inlines/inline_node.rs) and [`Raw`](../../parser/src/inlines/inline_node.rs)
  runs by [`split_attribute_value`](../../parser/src/content/inline_builder.rs) — the §3.4.1 policy
  applied for the first time: because `SpecialCharacters` has already run and will not run again over
  spliced-in content, a literal `<`/`>`/`&` in the value becomes a `Raw` leaf (unescaped) rather than a
  `CharRef` (which the fold would re-escape), while everything else stays `Text`. An **escaped**
  reference (`\{name}`, `{name\}`, `\{name\}`) drops its backslash(es) and keeps the rest of its match
  as literal nodes, replacing nothing, mirroring `AttributeReplacer`'s `caps[1]`/`caps[5]` branch — and,
  because that check runs before any lookup, this works identically whether or not the named attribute
  is set. An `InterpretedValue::Set`/`::Unset` attribute (no textual value — the language leaves this
  case unclear, as the string replacer's own comment notes) expands to nothing, exactly as the string
  pipeline's replacer does.

  Three forms were initially deferred, each documented and pinned by a divergence test: a
  **`counter`/`counter2` directive**, whose resolution *advances* a document counter — a required side
  effect this additive step did not yet perform, the same reason every macro family deferred its own
  catalog/warning side effect until the footnote and cutover increments; a reference to a **missing**
  attribute under `AttributeMissing::Drop` / `::DropLine`, whose output *removes* content rather than
  leaving the reference literal — the behavior this step *does* reproduce, since it is also what the
  default `AttributeMissing::Skip` and `AttributeMissing::Warn` modes do (so those two are full parity,
  not a divergence); and a **construct inside an expanded value** that `CharacterReplacements` or
  `Macros` would recognize per §3.4.1 (a `(C)` becoming a `CharRef`, a `link:` becoming a `Ref`) but do
  not yet — a spliced value is a synthesized run with no `'src` slice of its own, and
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
  named counter via the parser's own [`counter`](../../parser/src/parser/parser.rs) method — the exact call
  `AttributeReplacer`'s counter branch makes — so the digits this step produces are the real, advanced
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
  an earlier span — §4.1's nesting note), which means a directive nested in a span is, by construction,
  advanced *before* a plain-text directive that precedes that span in the source — reversing the numbering
  whenever a directive and a sibling span's own nested directive interleave (`{counter:n} *{counter:n}*`
  numbered `2`, then `1`, backwards). The fix is a dedicated
  [`resolve_counters`](../../parser/src/content/inline_builder/attribute_refs.rs) pass that runs once, before
  the splicing recursion: it merges, by source position, a level's own directive matches with its `Styled`
  siblings' placeholder positions (recursing into a sibling exactly when the merge reaches it), so every
  directive across the whole tree is advanced in true left-to-right document order regardless of nesting.
  Each resolved value is recorded keyed by the directive's absolute source byte offset, so the (unchanged)
  splicing recursion — which still visits levels in its own, differently-ordered sequence — looks values up
  by that stable key instead of resolving them itself, and the two passes agree because a `Styled` node's
  own placeholder occupies exactly one codepoint in its parent's match string regardless of what its
  (already-spliced-or-not) children contain, so recursion order never perturbs a parent level's own byte
  offsets.

  Because this additive builder is not yet wired into any real parse, its own `Parser` is never the one the
  authoritative string pipeline advances, so a differential fixture is free to build and fold against one
  independent default parser and compare against the golden string pipeline's own independent parser —
  exactly the two-independent-parsers discipline the footnote differential corpus already established —
  without the two sequences crossing over. The two forms 5b's own landed-as note still documents as
  deferred — a missing attribute under `Drop`/`DropLine`, and a construct inside an expanded value — remain
  outstanding.

  *Follow-up landed as (a character replacement inside an expanded value):* the *construct inside an
  expanded value* half of 5b's own deferred pair is closed for
  [`apply_character_replacements`](../../parser/src/content/inline_builder/char_replacements.rs) (the
  *macro* half remains deferred — see below). The shared match-string builder,
  [`build_match_string`](../../parser/src/content/inline_builder/quotes.rs), previously treated *any*
  non-verbatim node — a rendered span exactly as much as a synthesized (attribute-expanded) `Text` run — as
  one opaque placeholder, which is why a `(C)` inside `{note}` (`"(C) 2024"`) stayed unrecognized: the
  `Text` node's `value` differs from `location.data()`, so it fell through to that opaque case. It now
  splits a synthesized run into its own [`Piece`](../../parser/src/content/inline_builder/quotes.rs) kind —
  contributing the run's own `value` bytes to the match string (so a pattern sweep can match inside it) but
  flagged [`synthesized`](../../parser/src/content/inline_builder/quotes.rs), since those bytes have no
  honest `'src` counterpart. [`emit_range`](../../parser/src/content/inline_builder/quotes.rs) slices a
  synthesized piece's *value* (not its `location`) to the overlap and keeps the whole original `location` as
  every resulting fragment's coarse fallback span (design §4.4 — the same policy
  [`split_attribute_value`](../../parser/src/content/inline_builder/attribute_refs.rs) already gives every
  fragment of an expansion), and [`source_slice`](../../parser/src/content/inline_builder/quotes.rs) gained
  a start/end [`Bias`](../../parser/src/content/inline_builder/quotes.rs) so a boundary landing *inside* a
  synthesized piece falls back to that piece's whole node span while a boundary landing exactly on one of
  its own edges still resolves honestly (critically, to whatever construct comes immediately *after* it,
  not back into the synthesized run — see below).

  Because [`build_match_string`] is shared by every step in this module, this fix is also what keeps a
  *macro* family's own verbatim gate correctly rejecting a synthesized run now that it is no longer
  atomic: [`range_is_verbatim`](../../parser/src/content/inline_builder/macros/image.rs) (every macro
  family's own gate) and a new, narrower
  [`range_overlaps_synthesized`](../../parser/src/content/inline_builder/quotes.rs) (for the two macro
  families — the index-term shorthand and `indexterm2:[…]` — whose own recognition boundary was a bespoke
  opaque-span check rather than `range_is_verbatim`, since neither needs an honest `'src` slice for its
  *output*, only for deciding whether its *shown text* is reconstructable) both now reject a synthesized
  piece explicitly, so a macro inside an expanded value remains a documented divergence exactly as before,
  pinned by the existing (and one new, index-term) divergence test.

  Auditing every other consumer of the boundary-mapping helpers surfaced two real bugs, both fixed as part
  of this same follow-up rather than left as new divergences: (1) a boundary landing *exactly* on a
  synthesized piece's own edge was initially resolved via the same coarse whole-node-span fallback as an
  interior boundary, which is wrong whenever the piece's match-string length differs from its source
  length (the whole reason a piece is synthesized in the first place) — a construct recognized
  *immediately after* a synthesized run (e.g. a second `image:` macro right after an `{sp}` attribute
  reference) had its own location wrongly swallow the synthesized run's source bytes, corrupting an
  unrelated node's `is_icon` classification in one differential-corpus fixture; the fix skips that
  boundary to whichever piece comes next (or the past-the-last-piece fallback) instead of letting the
  synthesized piece claim it. (2) [`apply_footnotes`](../../parser/src/content/inline_builder/footnotes.rs)
  keeps its *own* copy of `emit_range`'s verbatim-slicing logic for the gaps around a recognized footnote
  ([`emit_range_recursing_footnotes`], which additionally recurses into a `Styled`/`Ref` child in place of
  cloning it whole); this copy had not been updated in step with `emit_range`'s own synthesized branch, so
  a plain attribute reference sitting beside (not inside) a footnote — reachable through the ordinary
  pipeline, since `apply_attribute_references` runs well before `apply_footnotes` — produced a corrupted,
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
  `SpecialCharacters` ahead of `Callouts`, so — unlike every other transducer in this module — the
  function need not descend into `Styled`/`Ref` children, since neither can exist at this point in that
  group's order. It reuses the string pipeline's *exact* recognition and trailing-position lookahead —
  [`build_callout_regexes`](../../parser/src/content/substitution_step.rs) is now shared `pub(crate)` —
  so only the recognition *sink* differs (§4.1): a match that fails the lookahead (not the last token on
  its line) is simply left out of the match list, so the surrounding gap reproduces its original nodes
  unchanged, the same outcome the string pipeline's `LookaheadReplacer` fallback produces.
  Auto-numbering (`<.>`) is scoped to one call of the step, exactly as the string replacer's counter is
  scoped to one block.

  This is also the node vocabulary's first real consumer for `Callout`, so its field set (Phase 0
  provisional) was refined here: alongside `number`, it now carries a `guard` — a new
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
  passthrough** — `Passthroughs::extract_from` extracts it last, after both passthrough-macro passes, so
  that a passthrough placeholder nested inside a STEM expression survives — and `apply_stem` mirrors that
  ordering exactly: it is its own step, run immediately after
  [`apply_passthroughs`](../../parser/src/content/inline_builder/passthrough_step.rs) and ahead of every
  other step, so a STEM expression's content is never touched by specialcharacters, quotes, replacements,
  or macros (a `stem:[…]` written inside an already-extracted `+++…+++` passthrough is therefore *not*
  re-extracted, matching the string pipeline). It reuses the string pipeline's *exact* recognition and
  notation-resolution — `INLINE_STEM_MACRO` and `stem_notation` are now shared `pub(crate)` — so only the
  recognition *sink* differs (§4.1). The node's `value` is *not* the untouched source slice (unlike a
  macro node's target/display text elsewhere in this module): a STEM expression is unescaped (`\]` → `]`),
  has its legacy `latexmath` `$…$` wrapper dropped, and is run through the real substitution pipeline
  under its resolved substitution group ([`SubstitutionGroup::Stem`](../../parser/src/content/substitution_group.rs),
  special characters only, for a bare macro) via the passthrough step's own `passthrough_text` helper
  (now shared `pub(super)`) — so a custom `InlineSubstitutionRenderer`'s escaping is honored exactly as it
  would be for the string pipeline's own restore step, at the cost of an owned value rather than a `'src`
  borrow (the same trade-off the `++…++`/`$$…$$` passthrough forms make). The fold then passes that value
  straight through as `render_quoted_substitution`'s body, with no attribute list or id (the macro's
  pattern captures neither). The Phase-0 node doc (which described `value` as "the raw expression,
  carried through verbatim") was updated to match this decision.

  An escaped macro (`\stem:[…]`) drops its backslash and stays literal, mirroring every other macro
  family's escape handling. One form was initially deferred, documented and pinned by a divergence test: a
  macro carrying an **explicit substitution list** (`stem:c,q[…]`). This step is **additive**: nothing is
  wired into the parse path. The remaining three forms 5a already documents as deferred — an
  attribute-list-prefixed passthrough, a `pass:` macro with an explicit substitution list, and the bare
  unconstrained `+text+` form — remain for a later increment.

  *Follow-up landed as (the `stem:c,q[…]` explicit substitution list):* unlike a `pass:` macro's explicit
  list (deferred at the time, closed by step 5d part 3 below), a `Stem` node needs no richer subtree to
  carry this form: it already has a single `value` field, so the same treatment part 3 goes on to give
  `pass:` — running the expression through the **real substitution pipeline** under the list's resolved
  [`SubstitutionGroup`](../../parser/src/content/substitution_group.rs) — applies directly. A new
  `resolve_stem_subs` helper in
  [`stem_step.rs`](../../parser/src/content/inline_builder/stem_step.rs) resolves an explicit list (or
  falls back to [`SubstitutionGroup::Stem`] for a bare macro) via
  [`SubstitutionGroup::from_custom_string`] — the exact call [`InlineStemMacroReplacer`] makes, including
  its "skip and keep going" handling of an unrecognized name — and `stem_expression_value` substitutes the
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
  them. That is safe for the bare macro's default group (special characters only — a per-character,
  context-free substitution), which is the only group this splicing ever ran under before this follow-up —
  but an explicit list naming a step that needs more than one `Text` run of context (`Quotes`,
  `AttributeReferences`, `CharacterReplacements`, `Macros`, `PostReplacement`) can miss a construct whose
  two halves fall on either side of the `Raw` (`stem:q[*a +++x+++ b*]`): the string pipeline substitutes
  the *whole* expression as one string (the passthrough's content merely protected by its own sentinel, not
  absent from the string), so it finds the quote pair; splicing per fragment never sees a complete pair in
  either one. The fix is a new `subs_are_local` predicate — true when the resolved group's
  [`steps()`](../../parser/src/content/substitution_group.rs) are empty or contain only
  `SpecialCharacters` — that `build_stem_node` checks whenever the expression's `emit_range` recovers more
  than one node: a non-local explicit list beside a nested passthrough is left unrecognized (the same
  "documented divergence" shape every other boundary in this module takes) rather than silently diverging,
  while a *local* explicit list (`stem:c[…]`) beside the same nested passthrough is unaffected and still
  recognized. Two new tests pin this exactly: one confirming the local case still applies beside a nested
  passthrough, one confirming the non-local case is left unrecognized and diverges from the golden string
  pipeline's own (correct) output.

  [`InlineStemMacroReplacer`]: ../../parser/src/content/passthroughs.rs

  *Step 5d part 2 landed as (an attribute-list-prefixed passthrough → `Styled`):*
  [`apply_passthroughs`](../../parser/src/content/inline_builder/passthrough_step.rs) now recognizes all
  three attribute-list-prefixed forms 5a deferred — `[quotes]++text++`/`[quotes]+++text+++`/`[quotes]$$text$$`
  (`INLINE_PASS_MACRO`'s own attrlist branch), `` [x-]`text` `` and `[attrs]+text+` (`INLINE_PASS`, now
  shared `pub(crate)`) — each as a [`Styled`](../../parser/src/inlines/styled.rs) node (`Code` for
  monospace, `Unquoted` otherwise; always `Unconstrained`) whose attrlist is parsed the same way an
  attributed quote's is (the quotes step's `attributes_of`, now shared `pub(super)`), folding through the
  same `render_quoted_substitution` `PassthroughRestoreReplacer` calls when its stored passthrough carries
  a `type_`, so the output is byte-for-byte identical. The bare forms run as a genuinely **second pass**
  ([`apply_bare_attrlisted_pass_level`](../../parser/src/content/inline_builder/passthrough_step.rs)) over
  what the delimited pass leaves behind, mirroring `Passthroughs::extract_from`'s own two-regex order
  (`INLINE_PASS_MACRO` before `INLINE_PASS`) — which turns out to matter: an attribute-list-prefixed
  *delimiter* escape (`[attrs]\++text++`) drops its one backslash and leaves literal, unopaqued text
  behind, which the second pass then legitimately **re-recognizes** as its own (different) match, exactly
  as the string pipeline's own second regex pass does over its own once-substituted text — parity by
  construction, not a coincidence, once both passes exist.

  The legacy **`x-` compatibility marker** (an attrlist of exactly `x-`, or one ending in ` x-`) is the
  one case whose body is not a single `Raw` leaf: it switches the variant to `Code` and re-threads the
  body through the **full `Normal` substitution order** — special characters, quotes, attribute
  references, character replacements, macros, post-replacement, `SubstitutionGroup::Normal`'s own step
  list minus the passthrough/STEM extraction that already ran once, ahead of it — via a new
  [`apply_normal_subs`](../../parser/src/content/inline_builder/passthrough_step.rs) helper that chains
  the six existing step functions directly, mirroring `PassthroughRestoreReplacer`'s own recursive
  `pass.subs.apply(…)` call for that case as a node transducer rather than a second string pass. Only the
  `++` boundary (delimited) and the plus bare form trigger it; the backtick bare form's attrlist is
  *always* `x-`-eligible (the regex itself requires it) but its format mark keeps `subs` at `Verbatim`
  regardless, and `+++`/`$$` never switch at all — both mirrored exactly from `handle_quoted_text` and
  `InlinePassReplacer`.

  Two corner cases remain deferred, each documented and pinned by a divergence test: an **escaped
  bracket** (`\[attrs]++text++`), which unescapes to a literal `[attrs]` prefix *and* still recognizes the
  delimited text as an ordinary (non-attrlisted) passthrough — a kept-literal-prefix-with-one-dropped-char
  plus a node for the remainder, a shape neither `MacroMatchKind` variant expresses — and the
  **"prohibited prefix"** the string replacer's own retry loop protects (a bare attrlisted match
  immediately preceded by `\`, `:`, or `;`): rather than reproduce the retry, such a match is simply left
  unrecognized. This step is **additive**: nothing is wired into the parse path. The remaining two forms 5a
  documents as deferred — a `pass:` macro with an explicit substitution list, and the bare unconstrained
  `+text+` form with no attribute list at all — remain for a later increment.

  *Step 5d part 4 landed as (the bare unconstrained `+text+` form):* the last of the four forms 5a defers.
  [`find_bare_attrlisted_matches`](../../parser/src/content/inline_builder/passthrough_step.rs) now also
  recognizes `INLINE_PASS`'s third, attribute-list-free alternative, folding through a plain
  [`Raw`](../../parser/src/inlines/inline_node.rs) leaf — like the double-plus/double-dollar forms, an
  absent attrlist means no stored `type_`, so the restore never wraps the text in a rendered span, unlike
  the two attribute-list-prefixed bare forms part 2 landed. Unlike those two forms (matched via
  `\b{start-half}`, which does not by itself exclude a `\`/`:`/`;` prefix and so needed the "prohibited
  prefix" divergence just above), this form's own pattern already excludes that prefix directly in its
  *consuming* boundary group (`[^\w;:\\]`, which also encodes the "must not follow a word" rule) — so no
  runtime retry is needed at all: a match simply cannot start where the pattern's own character class would
  reject it, parity by construction rather than a divergence. The boundary character the pattern does
  consume ahead of the leading `+` (absent only when the match sits at the very start of the level) is not
  part of the construct itself; it is kept as literal text before the node, reusing the same kept-prefix
  `MacroMatch` sub-range the auto-link increment (part 2) introduced for a bare URL's own boundary prefix.
  An escaped mark (`\+text+`) drops the single backslash and keeps the rest of the match — the boundary
  character included — literal, with nothing left to re-scan it afterward (this is already the last pass),
  so it is plain parity rather than a divergence.

  Landing this form also **retired** the two divergences 5a's own escape handling documented: an escaped
  triple- or double-plus (`\+++text+++`, `\++text++`) drops its backslash and keeps the delimited text
  literal at the pass-macro level, and now that the bare unconstrained form is recognized too, the
  bare-form second pass legitimately re-scans that same de-escaped text and consumes its leading `+++`/`++`
  as a bare passthrough wrapping a shorter run (`+text` / `text`, one `+` left over as trailing literal
  text) — exactly what the string pipeline's own second regex pass does over its own once-substituted text.
  Both fixtures moved from their own divergence tests into the main differential corpus. The one form 5a
  documents as deferred that remains outstanding is a `pass:` macro with an explicit substitution list
  (`pass:c,q[…]`), which still needs a richer subtree than a single `Raw` leaf can hold. This step is
  **additive**: nothing is wired into the parse path.

  *Step 5d part 3 landed as (a `pass:` macro with an explicit substitution list → `Raw`, the last of 5d's
  four deferred forms):* the one form step 5a and part 4 both name as outstanding,
  [`build_pass_macro_subs_value`](../../parser/src/content/inline_builder/passthrough_step.rs), is
  recognized — but not, in the end, via a richer node subtree. Prototyping that shape first (threading the
  resolved [`SubstitutionGroup::Custom`] steps through this module's own transducers, the way the legacy
  `x-` compatibility marker's body already does via [`apply_normal_subs`](../../parser/src/content/inline_builder/passthrough_step.rs))
  surfaced a real bug, **pre-existing** in the already-landed `x-` marker path and independent of this
  step: [`apply_passthroughs`] runs *first* in [`build`](../../parser/src/content/inline_builder/mod.rs),
  so anything it splices is still visited by every one of `build`'s own later steps. That visit is a safe
  no-op for `Quotes` (delimiters are consumed, so a second pass finds nothing left) and
  `SpecialCharacters`/`CharacterReplacements`/`PostReplacement` (their own output is atomic or already
  stripped of what they match on) — but **not** for `Macros`: a link or cross-reference node's display
  text is *literal* text that reads exactly like the source it came from (by design, so the fold can
  recover it with no build-time state), so a second `Macros` pass recognizes it all over again and nests
  a second `Ref` inside the first. A prototype fixture (`[x-]++https://example.org++`, exercising the
  already-merged step 5d part 2) reproduces this today: `<code><a href="…"><a href="…">…</a></a></code>`,
  doubled. A list omitting a step `build`'s own fixed order still runs unconditionally (e.g. `pass:q[…]`,
  which never asks for `SpecialCharacters`) fails the opposite way: content the author's list deliberately
  left raw gets escaped anyway by `build`'s own later `SpecialCharacters` step. Solving this properly — so
  a spliced subtree is visited by *exactly* the steps its own resolved list named, once — is a splice-time
  protection mechanism this additive, pre-cutover module does not yet have; it is squarely the cutover's
  job (step 6, which does not splice mid-`build` at all).

  Given that, this increment takes the same shape every other deferred-until-now form in this file
  eventually adopts once it becomes tractable: the resolved list's body is rendered through the **real,
  string-based** substitution pipeline ([`SubstitutionGroup::apply`], via the passthrough step's own
  [`passthrough_text`](../../parser/src/content/inline_builder/passthrough_step.rs) helper already used for
  `++…++`/`$$…$$`/the bare unconstrained form) — exactly the call `PassthroughRestoreReplacer` makes for a
  stored `Passthrough` — producing an already-final HTML string that becomes a single
  [`Raw`](../../parser/src/inlines/inline_node.rs) leaf's `value` verbatim, folding through the identical
  byte-for-byte output the string pipeline produces. A `Raw` leaf is *opaque* to every one of `build`'s own
  later steps (never descended into, never re-matched), so it sidesteps both failure modes above
  regardless of which steps the author's list names or omits, and in which order — the list is applied
  once, by the real pipeline, and never touched again. An unrecognized substitution name in the list (e.g.
  `pass:bogus[…]`) is silently skipped — any recognized names are still honored — mirroring
  `SubstitutionGroup::from_custom_string`/`InlinePassMacroReplacer`'s own resolution; this additive pass
  does not yet raise the string pipeline's own `InvalidSubstitutionTypeForPassthroughMacro` warning for
  it, deferring that side effect to the cutover like every other macro family's own catalog/warning side
  effect, since it does not change the fold's output bytes. An escaped closing bracket (`pass:c[a\]b]`)
  unescapes before rendering, the same treatment every other `pass:[…]` bracket content gets — no longer a
  deferred corner the way a structured-children shape (a footnote's own content) would have forced. A
  differential corpus pins single- and multi-step lists (applied in the order given, not the *normal*
  effective order), an unrecognized name skipped alongside a recognized one, an empty resolved list (the
  content spliced back completely untouched), a list naming `Macros` (folding through real rendered
  markup), and the escape/unescape forms. This step is **additive**: nothing is wired into the parse path.
  Landing it closes out step 5d and, with it, step 5 in full.

  *Step 6 prep landed as (the image family's deferred recognition side effects, staged):* with step 5 done,
  every macro family the recorder covers now has a single-pass counterpart, but each one still skips the
  **recognition side effect** its string-pipeline replacer performs at the same point — registering an id,
  link, or image target in the document catalog, or recording a warning — deferring it "to the cutover"
  (step 6, below). Step 6 itself bundles a lot: swapping the recorder for the builder inside `Content`,
  making `rendered_html()` a fold, deleting the three sentinel systems, retiring the `with_inline_tree`
  flag, *and* re-attaching every deferred side effect all at once. Re-attaching the image family's own two
  side effects — [`register_image`](../../parser/src/parser/parser.rs) (for `image:`, gated on
  `catalog_assets`) and the `link=` dangerous-scheme/self-href warning `InlineImageMacroReplacer` records —
  turns out not to need the cutover itself: it is a standalone function,
  [`apply_image_side_effects`](../../parser/src/content/inline_builder/macros/image.rs), that walks an
  already-built tree and reads each [`Image`](../../parser/src/inlines/image.rs) node's own stored `target`
  and `attrs` instead of a regex capture, mirroring `InlineImageMacroReplacer::replace_append`'s own
  `link=self`/`link=`-scheme rejection logic (`link_self_resolves_to_src`, `has_dangerous_scheme`,
  `has_dangerous_self_href`, `is_uri_ish`) exactly. It recurses into every container an `Image` node can be
  nested inside — a `Styled` span, a `Ref`, or a `Footnote`'s own children — so a nested or footnote-embedded
  image is found too. Landing it now, ahead of the rest of step 6, gives the eventual cutover one fewer thing
  to get right in one leap: this piece is already written, tested against a broad fixture set (including a
  differential comparison against the golden string pipeline's own registrations, using the same
  two-independent-parsers discipline the footnote increment established), and reviewed on its own. As with
  every additive increment before it, **nothing is wired into a real parse path** — the function is called
  only by its own tests, against their own `Parser` — so calling it for real still waits for step 6, when it
  can be invoked exactly once per parse without double-counting a registration. A new
  [`Parser::catalog`](../../parser/src/parser/parser.rs) test-only accessor was added alongside it, so a test
  can inspect a `Parser`'s own live catalog directly (the registrations this function performs) without a
  full `Document` parse, whose own `Document::catalog()` is a separate, later snapshot. The remaining
  deferred side effects — an attributed span's and an anchor's own `register_ref` (plus the anchor's
  duplicate-id warning and the bibliography-anchor form), and `register_link` for the four link-macro forms —
  are unstaged and remain step 6's own work, alongside everything else step 6 bundles.

  *Step 6 prep landed as (the link family's deferred `register_link`, staged):* the same treatment, applied
  to the second of step 6's unstaged registrations. [`apply_link_side_effects`](../../parser/src/content/inline_builder/macros/links.rs)
  is a standalone function that walks an already-built tree and, for each [`Ref`](../../parser/src/inlines/ref_node.rs)`{Link}`
  node, calls [`register_link`](../../parser/src/parser/parser.rs) with the node's own stored `target` —
  no recomputation needed, since `target` already holds exactly the string the string pipeline's four link
  replacers (the `link:`/`mailto:` macro, the auto-link, and the formal-URL link — the bare e-mail form is a
  later increment, not yet built by this module) register. A cross-reference is also a `Ref` node but is
  never registered, mirroring the string pipeline's own link-only catalog. It recurses into every container
  a `Ref` node can be nested inside — a `Styled` span, another `Ref`'s own display children (so a link
  nested in a cross-reference's text, or vice versa, is found too), or a `Footnote`'s own children —
  mirroring the image increment's own recursion. As with the image side effects, **nothing is wired into a
  real parse path** — the function is exercised only by its own tests, against their own `Parser` — so
  calling it for real still waits for step 6. A broad differential corpus compares its registrations against
  the golden string pipeline's own, using the same two-independent-parsers discipline the image increment
  established. The remaining deferred side effect — an attributed span's and an anchor's own `register_ref`
  (plus the anchor's duplicate-id warning and the bibliography-anchor form) — is unstaged and remains step
  6's own work.

  *Step 6 prep landed as (the anchor and attributed-span family's deferred `register_ref`, staged — the
  last of step 6's unstaged registrations):* [`apply_ref_side_effects`](../../parser/src/content/inline_builder/macros/anchors.rs)
  is a standalone function that walks an already-built tree and, for each [`Anchor`](../../parser/src/inlines/anchor.rs)
  node and each id-carrying [`Styled`](../../parser/src/inlines/styled.rs) span, calls
  [`register_ref`](../../parser/src/parser/parser.rs) under `RefType::Anchor` — reading the node's own
  stored `id`/`reftext` instead of a regex capture. It reproduces the two divergent behaviors the string
  pipeline's two call sites give this one catalog side effect: an inline anchor additionally raises the
  duplicate-id warning `InlineAnchorReplacer` records (an attributed span's own registration stays silently
  non-fatal, mirroring the quotes step's own `let _ = register_ref(...)`), and a shorthand `[[id]]`
  immediately preceded by a `[` — the inner anchor of a `[[[id]]]` sequence appearing *outside* a
  bibliography list item — is recognized (already, by the existing node builder) but never registered,
  mirroring `InlineAnchorReplacer`'s own `is_bibliography_inner` check (recomputed here from the node's own
  source span rather than a haystack index, since the tree walk has no regex capture to read it from). The
  true bibliography-anchor construct itself (`[[[label]]]` inside a bibliography list item, `RefType::Bibliography`)
  is a separate, list-item-gated pass (`INLINE_BIBLIO_ANCHOR`) this builder does not yet recognize as its own
  node at all; that remains out of scope here, same as before. A description-list term's own leading-anchor
  pre-registration (`DefinedTerm::substitute`, `apply_macros_with_leading_anchor_registered`) is mirrored by
  a `leading_anchor_registered` parameter, so wiring this function in at that call site can suppress the same
  duplicate-id warning the string pipeline suppresses there. It recurses into every container an id-bearing
  node can be nested inside — a `Styled` span, a `Ref`'s own display children, or a `Footnote`'s own children
  — mirroring the image and link increments' own recursion. As with those, **nothing is wired into a real
  parse path** — the function is exercised only by its own tests, against their own `Parser` — so calling it
  for real still waits for step 6. A broad differential corpus compares its registrations against the golden
  string pipeline's own, using the same two-independent-parsers discipline the image increment established.
  With this landed, every recognition side effect step 5's macro families skip is now staged as its own
  unwired building block; step 6 itself — swapping the recorder for the builder, the fold, the sentinel
  deletions, and calling each staged function exactly once per parse — remains fully outstanding.

  *Step 6 prep landed as (a combined entry point for every staged side effect, plus a link-registration-order
  fix it surfaced):* with all three families' side effects staged individually, the cutover's own job of
  "calling each staged function exactly once per parse" needed one more piece: a single entry point that
  composes them in the right relative order, since the two link-recognizing families and the image/anchor
  families all write into catalogs and warnings the cutover must get right *together*, not just individually.
  [`apply_macro_side_effects`](../../parser/src/content/inline_builder/macros/mod.rs) is that entry point —
  it calls [`apply_image_side_effects`](../../parser/src/content/inline_builder/macros/image.rs), then
  [`apply_link_side_effects`](../../parser/src/content/inline_builder/macros/links.rs), then
  [`apply_ref_side_effects`](../../parser/src/content/inline_builder/macros/anchors.rs) — the same relative
  order the string pipeline's own macro passes run in (§4.1) — so that when more than one family's side
  effect touches the *same* shared list (concretely, [`Parser::record_substitution_warning`](../../parser/src/parser/parser.rs)'s
  one shared warnings list, which both the image family's dangerous-scheme warning and the anchor family's
  duplicate-id warning write to) the combined call lands them in the golden pipeline's own order. A new
  differential corpus exercises the composed call directly — a fixture mixing an image, both link forms, and
  an anchor in one content, and a fixture whose image and anchor *both* warn, asserting the two warnings land
  in image-then-anchor order against an independent golden parser (the same two-independent-parsers
  discipline every prior staged function's own corpus uses).

  Building that composed differential corpus surfaced a genuine ordering bug in the already-staged
  [`apply_link_side_effects`](../../parser/src/content/inline_builder/macros/links.rs): the string pipeline
  registers a link's target when its *own* replacer's pass matches it, and the auto-link/formal-URL pass
  (`INLINE_LINK`) and the `link:`/`mailto:` macro pass (`INLINE_LINK_MACRO`) are two separate, sequential
  whole-string passes (§4.1) — so the catalog ends up in **family-pass order, not true source order**: every
  auto-link/formal-URL link registers before every `link:`/`mailto:` macro, regardless of which appears first
  in the source (already pinned, independently of this module, by
  `catalog_records_link_targets_when_catalog_assets_enabled` in `tests/asciidoctor_rb/substitutions_test.rs`).
  The originally-staged function instead made one tree walk in document order, which is only ever correct by
  coincidence — a content that interleaves the two forms out of that relative order (`link:b.html[B] then
  https://a.example`) diverges: document order would register `b.html` first, but the golden catalog is
  `["https://a.example", "b.html"]`. No existing test exercised mixed forms in one content, so this had gone
  unnoticed. The fix makes two passes over the tree — every auto-link/formal-URL match first, then every
  `link:`/`mailto:` macro match — distinguishing the two from a node's own `location` alone (a `link:`/
  `mailto:` match's location always starts with its literal prefix, and the auto-link pass never builds a
  node for `INLINE_LINK`'s own link-macro branch, deferring that whole form to the macro pass — see
  [`inline_link_level`](../../parser/src/content/inline_builder/macros/links.rs)'s own doc comment — so this
  is a reliable signal, not a heuristic, and needs no new node field). A new test pins the interleaved case
  directly against the golden pipeline, and the broad differential fixture set gains an interleaved fixture
  too. As with every prior increment in this module, nothing here is wired into a real parse path — calling
  [`apply_macro_side_effects`] for real still waits for the single-pass builder to replace the recorder as
  `Content`'s tree source, which remains step 6's own, still fully outstanding, job.

  *Step 6 prep landed as (a whole-pipeline differential corpus against the real `SubstitutionGroup::apply`
  entry point):* every differential corpus landed so far (one per step/family) hand-chains only the
  [`SubstitutionStep`]s that step's own increment covers — skipping `AttributeReferences` unless the fixture
  needs it, and never running passthrough extraction/restore or deferred cross-reference finalization
  alongside the other steps. That pins each step in isolation, but it had never exercised the *fully
  assembled* pipeline [`SubstitutionGroup::Normal::apply`](../../parser/src/content/substitution_group.rs)
  runs in production — passthrough/STEM extraction, every step in true order, passthrough restore, and
  deferred-reference finalization, all against one `Content` — which is exactly what [`build`] (this
  module's own single call) must reproduce once the cutover wires it in. A new differential corpus, in
  [`inline_builder`](../../parser/src/content/inline_builder/mod.rs)'s own test module, closes that gap: each
  fixture calls the real, public `SubstitutionGroup::Normal.apply` as the golden and `build` + `fold_html`
  as the candidate, and — unlike every prior corpus, each scoped to one family — **combines** several
  construct families in one piece of content (quotes wrapping an attribute reference, a footnote whose own
  text carries a nested attribute reference, a passthrough beside an image macro, a `counter` directive
  beside a formatted span carrying an attribute reference and a STEM expression, an inline anchor beside an
  index term, escaped constructs beside a live attribute reference, …), so a boundary-crossing interaction
  between two steps that individually pass would still be caught. As with every prior corpus, a fixture stays
  inside the vocabulary `build` already covers, avoiding the forms still documented as deferred elsewhere in
  this module (an attribute value that itself embeds a construct `CharacterReplacements`/`Macros` would
  recognize, the `hardbreaks` option, a menu's `>` submenu form, …). Every fixture passed without needing a
  code change, giving the first real, end-to-end confirmation that the fully assembled single-pass builder
  — not just its individual steps — reproduces the real production pipeline's output, ahead of step 6's own
  wiring work. As with every prior increment, nothing here is wired into a real parse path.

  *Step 6 prep landed as (the `hardbreaks` option, closing a real cutover blocker):* auditing `build`'s
  vocabulary against a corpus-wide differential run (rather than the hand-picked fixtures every prior corpus
  in this module uses) surfaced that the `hardbreaks` block option was not just an unclaimed form like the
  others this module documents as deferred, but a **real blocker**: golden tests already exercise it (design
  §5.4's oracle), so cutting over `Content` to `build` while it stayed unhandled would have silently regressed
  them, not merely left a construct unrecognized. [`apply_post_replacements`](../../parser/src/content/inline_builder/post_replacements.rs)
  now takes the enclosing block's own `Attrlist` (`build` itself gains the same `Option<&Attrlist<'src>>`
  parameter, threaded through from its caller — every other step ignores it) and, when
  `parser.is_attribute_set("hardbreaks-option")` or the attrlist's own `%hardbreaks` option is set, runs a new
  `apply_hardbreaks` in place of the default ` +`-only form: every line ending in the level's own match string
  becomes a break, a redundant trailing ` +` is stripped rather than doubled, and the level's own last line
  (nothing follows its `\n`) never gets one — mirroring the string pipeline's own `lines()`-split, per-line
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
  cannot see" — because "two node trees fold to identical HTML, masking a structural bug" is
  exactly the risk every prior corpus in this module, pinning HTML-fold parity alone, cannot rule
  out. A new test module,
  [`inline_builder_recorder_parity`](../../parser/src/tests/inline_builder_recorder_parity.rs),
  builds both trees for the same fixture — the recorder's via `Content::inlines()` under
  [`Parser::with_inline_tree`](../../parser/src/parser/parser.rs), the builder's via `build` — and
  compares them structurally, node kind by node kind, over the same broad general-purpose and
  combined-constructs fixture sets [`inline_builder`](../../parser/src/content/inline_builder.rs)'s
  own whole-pipeline corpus already proved stay inside `build`'s claimed vocabulary.

  The comparator ignores exactly the fields already documented elsewhere as one-sidedly richer on
  the builder (`attrs`, `derived`, `xrefstyle`, `resolved`, an anchor's `reftext`, an image's
  `is_icon`) and resolves every *leaf-boundary* difference — a builder `Text`/`Raw`/`CharRef` node
  whose rendered bytes are only a sub-range of what the recorder recovered, or vice versa — through
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
  recorder — no renderer call to intercept during restore — recovers as plain `Text`/`CharRef`
  leaves instead), and a source-written entity that coincides byte-for-byte with a live
  classification (`&amp;`, `&#8217;`, …) all resolve the same way, rather than as separate
  special cases.

  Two further, non-leaf differences turned out to be genuine (if narrow) structural facts, not
  bugs, and are excluded with their own documented reasoning: an **unresolved** cross-reference's
  `children` is legitimately empty on the recorder side (Asciidoctor's own unresolved-xref fallback
  renders the bracketed target, never the author's display text, so the recorder — recovering only
  what actually rendered — has nothing to see), while the builder always bakes the display text in
  as a structural fact independent of resolution; and a footnote **reference** occurrence's `id`
  stops reaching the renderer's own params the moment it resolves to a number (`fold_footnote`
  renders just the number then), so the recorder cannot recover it there either, while the builder
  still carries it. A `Link`'s `roles`/`window` fields are also skipped whenever its own
  attribute-list display text populated `attrs` instead (`render_link` reads `role`/`window`
  straight off `attrs` in that case, so the plain fields are never populated from it — the same
  asymmetry `Ref::attrs`'s own doc comment already describes).

  Every fixture passed once these were accounted for, without needing a code change to `build`
  itself — the first structural (not merely byte-parity) confirmation that the single-pass
  builder's tree is a faithful counterpart to the recorder's, ahead of the real swap. As with every
  prior increment, nothing here is wired into the parse path.

  *Step 6 prep landed as (a synthesized-seed entry point, for a real block's filtered/joined
  content):* the last structural piece the tree-source swap needs is an answer to *what `Span<'src>`
  does `build` even run on*, given that a real block's `Content` does not always hold one: the common
  single-surviving-line case
  ([`Content::from_filtered_lines`](../../parser/src/content/content.rs)) borrows a contiguous `'src`
  slice, but a genuinely multi-line block (or any other filtered value) joins its surviving lines into
  an *owned* string with no honest `'src` slice of its own — exactly the shape
  `build`'s own `source: Span<'src>` parameter could not accept. [`build_from_value`](../../parser/src/content/inline_builder.rs)
  closes this: it generalizes `build`'s seed from a bare `Span<'src>` to the `(value, location)` pair
  `Content` itself is already built from, so `build` becomes a thin wrapper
  (`build_from_value(CowStr::from(source.data()), source, …)`) over it. When `value` coincides with
  `location.data()` this is the existing verbatim path, unchanged; when it does not, the seed is
  *synthesized* — and every downstream step already knows what to do with that, because it is the same
  verbatim/synthesized split [`apply_special_characters`](../../parser/src/content/inline_builder/special_chars.rs)'s
  own `split_text` already makes for a single node deeper in the tree, and the same coarse-fallback
  policy (design §4.4) an attribute-reference expansion or a `counter` directive's resolved value
  already receives. No step needed a code change: `build_match_string` and `source_slice`
  ([`quotes.rs`](../../parser/src/content/inline_builder/quotes.rs)) already treat *any* non-verbatim
  `Text` node this way regardless of where in the tree — or how early — it was produced, so a
  wholly-synthesized *root* seed is just the same mechanism reached one level higher than any increment
  had exercised it before.

  This closes the gap for every step that never needed a verbatim `'src` slice in the first place —
  quotes, specialcharacters, attribute references, character replacements, post-replacement (including
  `hardbreaks`), and a macro family (a bare `footnote:[…]`) whose own content is captured as children
  rather than a literal value — pinned by a differential corpus comparing `build_from_value` against the
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
  — the same counter-safe discipline the recorder used, so footnote numbers and `{counter:…}`
  values come out identical to the authoritative output — then builds the tree with a new
  group-aware entry point,
  [`build_for_group`](../../parser/src/content/inline_builder/mod.rs), and stores it on
  [`Content::inlines`](../../parser/src/content/content.rs). `build_for_group` mirrors
  `run_pipeline`'s own step selection exactly: passthrough/STEM extraction runs first **iff** the
  group's steps include `Macros` or the group is `Header` (the same gate `run_pipeline` places on
  `Passthroughs::extract_from`), then each of the group's
  [`steps()`](../../parser/src/content/substitution_group.rs) runs in the group's own order, each
  recast as its node transducer — so a `subs=` custom list, the verbatim group's `Callouts`, the
  header/attribute-entry-value groups' two-step list, and the empty `Pass`/`None` lists (whose
  tree is the untouched seed) all follow the string pipeline's own selection, pinned by new
  per-group parity and structure tests. `build_from_value` is now the `Normal`-group special case
  of this entry point.

  What the swap changes for a tree consumer: every node now carries its **honest, precise span**
  (issue #944's precision stage, with the documented §4.4 coarse fallback for synthesized values)
  and macro nodes are **self-describing** (their own `Attrlist<'src>`, `derived`, `xrefstyle`) —
  the recorder's whole-content-span, `attrs: None` tree is gone from production. The fold-parity
  guarantee is now scoped to the builder's claimed vocabulary: a form documented as deferred
  (e.g. a display text crossing a rendered span) is left as **literal text** in the tree — never
  a wrong node — where the recorder, recovering structure from rendered output, could represent
  it. That scoping surfaced in exactly one production seam: the positional cross-reference
  resolution mirror (`mirror_tree_xref_resolution`), whose count-parity debug assertions assumed
  the tree holds one node per deferred segment. The mirror now **counts each list's slots first
  and skips a list whose count diverges** — leaving those nodes in their honest unresolved state
  rather than assigning destinations positionally onto the wrong nodes — pinned by new tests for
  both the block-level and footnote-embedded skip paths. The recorder's stateful-renderer hazard
  (its debug assertion, and the "requires a side-effect-free renderer" caveat on
  `with_inline_tree`) retires with the second rendering pass itself: the builder consults the
  configured renderer only where a node's *value* is defined as already-substituted text (a
  delimited passthrough's or STEM expression's body), so a stateful custom renderer now gets a
  logical, unpolluted tree — repinned by the corresponding test.

  The recorder ([`inline_tree`](../../parser/src/content/inline_tree.rs)) is not deleted but
  **retired to test-only oracle machinery** (`#[cfg(test)]`), exactly the §4.1 bring-up-oracle
  role: the differential harness drives it directly, and the structural cross-check
  ([`inline_builder_recorder_parity`](../../parser/src/tests/inline_builder_recorder_parity.rs))
  now builds its recorder side by driving that machinery itself — the production accessor returns
  the builder's tree, so reading `Content::inlines()` for both sides would compare the builder to
  itself — keeping the two independent constructions honestly comparable. The remainder of step 6
  is unchanged and still outstanding: making `rendered_html()` an authoritative fold of this
  tree, calling `apply_macro_side_effects` for real (which must wait for the fold, since until
  then the string pipeline still performs every registration), deleting the three production
  sentinel systems, and retiring the `with_inline_tree` flag.

  *Step 6 prep landed as (the §3.4.1 classification for an order that never escapes, closing a
  second real cutover blocker):* with the tree-source swap done, the next thing the remaining half
  of step 6 needs — making `rendered_html()` an **authoritative** fold — is to know where the fold
  still diverges from the string pipeline over the *whole* corpus, not over the hand-picked
  fixtures each family's own differential corpus uses. A corpus-wide audit (tree building forced on
  for every parse in the test suite, each content's tree folded and compared against that same
  content's own rendered string) — the same due-diligence sweep that surfaced the `hardbreaks`
  blocker above — found a second one of exactly that kind: not an unclaimed form the tree merely
  leaves literal, but a **wrong answer** for content golden tests already exercise.

  The cause is design §3.4's own definition read one step too narrowly. A
  [`Text`](../../parser/src/inlines/inline_node.rs) node is *logical* text that **the fold
  escapes**, which is exactly right when the `SpecialCharacters` step acted on the content — and
  exactly wrong when that step never runs, because there the string pipeline emits the author's
  `<`/`>`/`&` untouched. Every group whose effective order omits `specialcharacters` took that
  path and folded escaped entities where the string pipeline emits raw ones: a passthrough block
  ([`Pass`](../../parser/src/content/substitution_group.rs)), a comment block
  ([`None`](../../parser/src/content/substitution_group.rs)), and every `subs=` custom list that
  omits it (`subs=quotes`, `subs=attributes`, `subs=macros`, `subs=callouts` on a listing block,
  the empty `subs=","` list, …). This is §3.4.1's own policy — "the kind a fragment becomes is
  **not** a fixed property of where it came from; it is decided by which substitution steps still
  act on it under the group's effective order" — applied to the *seed* rather than, as step 5b
  first applied it, to a spliced attribute value: a literal special no `SpecialCharacters` step
  ever acts on is a [`Raw`](../../parser/src/inlines/inline_node.rs) leaf.

  [`classify_unescaped_specials`](../../parser/src/content/inline_builder/special_chars.rs) is that
  classification. It shares its whole split with
  [`apply_special_characters`](../../parser/src/content/inline_builder/special_chars.rs) — one
  `SpecialLeaf` discriminant now selects `CharRef::Special` or `Raw`, so the two cannot drift on
  where a boundary falls — and therefore keeps the same span discipline: a verbatim run's leaves
  are sliced from `'src` with honest `line`/`col`/`offset` (#944) while a synthesized run's fall
  back to the whole enclosing span (§4.4). [`build_for_group`](../../parser/src/content/inline_builder/mod.rs)
  runs it **last**, after the group's own steps, gated on
  `!steps.contains(&SubstitutionStep::SpecialCharacters)` — and running it last, rather than in
  place of the step that is absent, is what keeps it faithful: under such an order the string
  pipeline's own steps also match over text in which the specials are still literal, so every
  transducer must go on seeing them as ordinary `Text` characters, not as the opaque leaf a `Raw`
  node is to [`build_match_string`](../../parser/src/content/inline_builder/quotes.rs). Only the
  finished tree's *classification* differs; nothing about recognition changes. It recurses into
  every container a text run can be nested inside — a `Styled` span, a `Ref`'s display children,
  an `Anchor`'s reference text, and a `Footnote`'s own children — mirroring the containers
  [`fold_html`](../../parser/src/content/inline_builder/fold.rs) itself descends into, since a
  `subs=` list omitting `specialcharacters` can still name `quotes` and `macros`.

  A new differential corpus crosses a set of specials-bearing fixtures (bare specials in every
  position; specials that would look like a construct *once escaped*, so the classification is
  pinned not to perturb — or depend on — recognition; specials beside and inside each construct
  these orders can build; multi-line runs) with every real group that takes this path, each
  fixture driven through the real, public `SubstitutionGroup::apply` as the golden. Two existing
  tests that pinned the *old* shape — "`Pass`/`None` yield one untouched `Text` run" and
  `subs=quotes`'s own "`<` stays literal text" — are rewritten to assert the classification
  instead; neither had called the module's own `assert_group_parity` helper, which is precisely
  why the divergence went unnoticed, so both now do. As with every prep piece before it, nothing
  further is wired in: `rendered_html()` remains the string pipeline's own string, and this changes
  only what a tree consumer reads back (and what the eventual authoritative fold will emit).

  The audit also leaves the rest of step 6 a map. Every remaining whole-corpus divergence under the
  *normal* order is a form this file already documents as deferred — a display or reference text
  crossing a rendered span, an angle-bracketed `<url>` link, a bare e-mail auto-link, `<<id,>>`'s
  present-but-empty text, the `menu:View[Zoom > Reset]` submenu form, a macro inside an expanded
  attribute value, a missing attribute under `AttributeMissing::Drop`/`::DropLine`, and the
  bibliography anchor (`[[[label]]]`) — plus one category not previously named: an effective order
  that runs `SpecialCharacters` **after** a step that already produced markup (`subs=quotes,
  specialcharacters`), where the string pipeline escapes the very tags the earlier step emitted.
  That last one is structurally different from the rest, and harder: a tree whose markup exists
  only at fold time has no rendered tags for a later escaping step to act on, so it needs its own
  policy rather than another recognition increment.

  *Step 6 prep landed as (`Macros` → the bare e-mail auto-link, the last of the link family's
  spellings, closing one of that map's divergences):* the audit above leaves the remaining half of
  step 6 an itemized list of whole-corpus divergences to close before `rendered_html()` can become
  an **authoritative** fold; this closes the first of them. The builder now recognizes a **bare
  e-mail address** written in the flow (`doc.writer@example.com`) as the same
  [`Ref`](../../parser/src/inlines/ref_node.rs)`{Link}` node the two URL-link passes build, folding
  through the identical `render_link` so the output is byte-for-byte identical. It reuses the string
  pipeline's *exact* recognition — [`INLINE_EMAIL`](../../parser/src/content/macros.rs) is now shared
  `pub(crate)` — so only the recognition *sink* differs (§4.1), and no field is added to `Ref`: the
  target is the address prefixed with `mailto:` and the display text is the address itself, baked
  into a single [`Text`](../../parser/src/inlines/inline_node.rs) child, with no `bare` role and no
  `hide-uri-scheme` handling (`InlineEmailReplacer` passes `extra_roles: vec![]` and the raw address,
  unlike the bare-URL auto-link's own branch).

  [`email_level`](../../parser/src/content/inline_builder/macros/links.rs) runs **after** both
  URL-link passes and before the anchor pass, exactly where the string step runs
  `InlineEmailReplacer` — which is what makes the pattern's own "prefix that causes a mismatch" group
  reproduce identically: a `mailto:` macro's target or a URL's user-info/path is, by then, inside an
  opaque node here (inside already-rendered `<a …>` markup there), so it is never re-recognized in
  either pipeline. A `\` escape drops its backslash and leaves the address literal; any other
  mismatch prefix (`>`, `:`, `/`) leaves the whole match untouched, which is exactly what recording
  no match at all does. Two forms are left unrecognized, each documented and pinned by its own
  divergence test. The first is an address carrying a literal `&` (`a&b@example.org`, admitted by the
  pattern's own `&amp;` local-part alternative), which is an atomic
  [`CharRef`](../../parser/src/inlines/char_ref.rs) by macro time — the same escaped-special boundary
  every other macro family documents.

  The second is new in kind, and worth recording as its own category for the authoritative fold: an
  address **abutting an already-recognized construct** (`**bold**doc@example.org`,
  `link:x[y]doc@example.org`). The mismatch-prefix group reads the character immediately *before* the
  address, and in the string pipeline that character comes out of already-rendered markup —
  `</strong>`, `</a>`, and `<img …>` all end in `>`, one of the three mismatch characters, so the
  address stays literal there — while
  [`build_match_string`](../../parser/src/content/inline_builder/quotes.rs) stands the construct in as
  one opaque `SPAN_PLACEHOLDER` that belongs to no mismatch class. This is the "a tree whose markup
  exists only at fold time has no rendered tags for a later step to act on" category the audit note
  above names, reached through a *boundary class* rather than an escaping step; a review of the first
  version of this increment (which recognized the address, building a link the string pipeline does
  not) surfaced it. [`find_email_matches`](../../parser/src/content/inline_builder/macros/links.rs)
  now defers on a placeholder immediately before the address, so the tree carries literal text rather
  than a wrong node — the same outcome the sibling **auto-link** family already reaches structurally,
  its own boundary-prefix group being *required*, so a placeholder simply fails its match. That
  pre-existing instance (`**bold**https://example.org`, which the string pipeline links and this
  module has always left literal) was undocumented; it now has its own divergence test alongside the
  e-mail one, so both directions of the one boundary are recorded together. The deferral is
  deliberately **unconditional** rather than keyed on what the preceding node would render to: a
  construct that renders to nothing (a concealed index term) or that is still sentinel-masked when the
  macros step runs (a passthrough, a STEM expression) is one the string pipeline *does* link, so those
  defer too — reading that would mean invoking a renderer while building the tree, which this module
  does not do. (The Strategy-A recorder cannot pin this shape either: its marker-emitting renderer
  changes the very boundary character the pattern reads, so it links the address where the plain
  pipeline does not — which is why the structural cross-check's fixture set deliberately excludes it.)

  An address sitting inside a
  [`synthesized`](../../parser/src/content/inline_builder/quotes.rs) run is, by contrast, **not**
  deferred: like an anchor's id (and unlike a URL link's own target or attribute list), an e-mail node
  carries no `Span`-typed field, so [`text_slice`](../../parser/src/content/inline_builder/quotes.rs)
  recovers the exact address from an attribute expansion — or from a filtered multi-line block's own
  joined seed, reached at a tree's root through
  [`build_from_value`](../../parser/src/content/inline_builder.rs) — with only the node's `location`
  taking design §4.4's coarse fallback.

  The family's own deferred recognition side effect is staged along with it:
  [`apply_link_side_effects`](../../parser/src/content/inline_builder/macros/links.rs) — already the
  staged `register_link` for the four URL-link forms — now makes a **third** pass for the bare
  address, since the string pipeline's `InlineEmailReplacer` is a third whole-string pass after
  `INLINE_LINK` and `INLINE_LINK_MACRO`, so the catalog lands in family-pass order (an address
  registers after every URL link in the content regardless of source order), exactly the ordering
  concern the combined-entry-point increment already surfaced *within* the link family. The two forms
  are told apart with no new node field: only `email_level` builds a `mailto:`-scheme target whose
  own `location` does not start with a literal `mailto:`/`link:` prefix. Differential corpora pin the
  address forms, the mismatch prefixes, the escape, an address beside and inside other constructs and
  inside a footnote's own extracted text, and the registration order (against an independent golden
  parser, the same two-independent-parsers discipline every prior staged function's corpus uses);
  fixtures are added to the whole-pipeline combined-constructs corpus (including the
  attribute-expanded address, which needs the real `AttributeReferences` step), to the
  synthesized-seed sweep, and to the structural recorder cross-check. As with every prep piece before
  it, nothing further is wired in.

  *Step 6 prep landed as (`Macros` → the angle-bracketed URL, the second of that map's divergences):*
  the builder now recognizes [`INLINE_LINK`](../../parser/src/content/macros.rs)'s **ANGLE branch** —
  an angle-bracketed URL (`<https://example.org>`) and the bracketed spelling that keeps its `&lt;`
  (`<https://example.org[text]`) — as the same [`Ref`](../../parser/src/inlines/ref_node.rs)`{Link}`
  node the branch's non-angle sibling builds, folding through the identical `render_link`.

  What blocked it was the *shape* of the family's verbatim gate rather than anything about the branch
  itself. Every macro family requires its whole match to be verbatim `'src` before it will build a
  self-describing node, and this branch's own delimiters are by construction the one thing that check
  rejects: `&lt;` and `&gt;` reach the macros step as escaped
  [`CharRef`](../../parser/src/inlines/inline_node.rs) leaves — atomic pieces — under every effective
  order that escapes specials, so `range_is_verbatim` refused the branch outright, whatever the URL
  between them looked like. But those delimiters carry no value a node ever slices: the string
  replacer's own angle path emits **neither** of them, replacing the whole match with the rendered
  link. So the gate moves out of `find_inline_link_matches` and into
  [`build_inline_link_node`](../../parser/src/content/inline_builder/macros/links.rs) — where the
  branch's own capture groups are already resolved — and, for the ANGLE branch, covers only the
  **interior** between the delimiters: the scheme, the URL, and any `[…]` attribute list, the only
  parts a node reads. Nothing about the boundary itself is relaxed: an angle URL crossing an escaped
  special (`<https://example.org/?a=1&b=2>`) or a display text crossing a rendered span
  (`<https://example.org[*bold*]`) still defers, each with its own divergence test, exactly as its
  non-angle sibling does, and a wholly-synthesized seed still defers as the URL-link family already
  documents (its target needs an honest `'src` slice).

  The branch's three alternatives then split the way the replacer's own
  `is_angle && attrlist.is_none()` condition splits them. `<url>` is a *separate computation* there —
  no boundary prefix kept, no trailing-punctuation strip, no bare-scheme rejection (so `<http://;>`
  is a link whose target is `http://;`, the very target the bare branch rejects), always the `bare`
  role — and is mirrored by a new
  [`build_angle_link_node`](../../parser/src/content/inline_builder/macros/links.rs) whose node
  `consumed` range is the **whole match**, delimiters included, so `rebuild_macro_level` re-emits
  neither. `<url[text]` keeps its `&lt;` and needs no new code at all: it flows through the general
  path, whose `consumed` range already starts at the scheme, so the kept prefix is emitted straight
  from the `CharRef`'s own node. The third — an unterminated `<url`, with no closing `&gt;` and no
  `[…]` — the replacer emits unchanged, so the builder builds nothing for it either. Both escapes the
  angle path honors (a `\` before the `<`, and one before the scheme) become the same
  `Unescape` the family already had, and no field is added to `Ref`: the target is the scheme plus
  the bracketed body, and the display text is that target under the `hide-uri-scheme` strip both
  forms already share.

  The family's staged registration needs no angle-specific case either: an angle link is
  `InlineLinkReplacer`'s *own* pass — the first of the three link passes — and
  [`link_form`](../../parser/src/content/inline_builder/macros/links.rs) already classifies it there
  from the node's `location` and `target`, so
  [`apply_link_side_effects`](../../parser/src/content/inline_builder/macros/links.rs) picks it up in
  family-pass order unchanged, pinned by a new test against an independent golden parser. A new
  differential corpus pins the branch's spellings, both escapes, the missing strip and missing
  bare-scheme rejection, `hide-uri-scheme`, and the form beside and inside other constructs and inside
  a footnote's own extracted text; fixtures are added to the whole-pipeline combined-constructs
  corpus, the broad general-purpose sweep, and the structural recorder cross-check. As with every prep
  piece before it, nothing further is wired in.

  *Step 6 prep landed as (`Macros` → the `&gt;`-submenu menu form, the third of that map's
  divergences):* the builder now recognizes the **submenu spelling of the menu UI macro**
  (`menu:View[Zoom > Reset]`, `menu:View[Tools > Options > Advanced]`) as the same
  [`Ui`](../../parser/src/inlines/ui.rs) node its comma-delimited and bare/single-item siblings
  build, folding through the identical `render_menu` so the output is byte-for-byte identical.
  Nothing about the *split* changed: [`split_menu_items`](../../parser/src/content/inline_builder/macros/ui.rs)
  has always reproduced `InlineMenuMacroReplacer`'s own delimiter handling in full — a `&gt;` taking
  precedence over a comma, the last part the menu item and any earlier ones submenus — and was
  covered by its own unit test precisely because no *fixture* could reach that branch.

  What blocked it is the same thing the angle-bracketed URL increment directly above hit, in the
  one other family whose match legitimately carries a delimiter it never slices. A menu's level
  delimiter is written `>`, so by the time the macros step runs it is an escaped
  [`CharRef`](../../parser/src/inlines/char_ref.rs) — an *atomic* piece
  [`range_is_verbatim`](../../parser/src/content/inline_builder/macros/image.rs) rejects outright,
  which is why the whole-match gate refused the form whatever its item texts looked like. But a
  caret carries no value the node ever reads back out: like the `<<id>>` shorthand's own
  `&lt;&lt;`/`&gt;&gt;` delimiters, the string replacer *consumes* it as the item list's delimiter
  and emits it nowhere (the rendered caret between levels comes from `render_menu`, not from the
  source character). A new [`menu_match_is_sliceable`](../../parser/src/content/inline_builder/macros/ui.rs)
  gate — applied inside [`build_menu_node`](../../parser/src/content/inline_builder/macros/ui.rs),
  where the match's own capture groups are already resolved, exactly as the angle branch moved its
  gate into `build_inline_link_node` — therefore admits one atomic piece and one only: a `&gt;`
  *inside the item list*. Everything else is unchanged and still deferred, each pinned by its own
  divergence test: any other escaped special in an item text (`menu:File[A & B]`), a `&`- or
  `>`-carrying menu **name** (`menu:A&B[Save]`, `menu:a>b[Save]` — the pattern admits both, and
  neither is a delimiter the node consumes), an item text crossing a rendered span
  (`menu:File[*S* > As]`), and a match crossing a [`synthesized`](../../parser/src/content/inline_builder/quotes.rs)
  run (`menu:View[{zoom} > Reset]` — the relaxation is for an *atomic* piece, not for a run with no
  `'src` slice for the name and item texts to borrow). The admitted caret is identified from its own
  match-string bytes rather than a new node lookup, which is unambiguous: a rendered span contributes
  a single placeholder character, and the only other atomic pieces are the two remaining
  special-character entities.

  Landing this also closed a small latent gap of exactly the kind the `footnoteref:` increment
  closed for its own family: the menu pass ran its escape branch *after* the verbatim gate, so an
  escaped macro whose match the gate rejected (`\menu:View[Zoom > Reset]`, and still today
  `\menu:File[A & B]`) was left fully unrecognized — backslash and all — where the string replacer
  drops the backslash and keeps the rest. The check is hoisted ahead of the gate, and needs no gate
  of its own: dropping the backslash keeps the rest of the match as its *own original nodes* (an
  escaped special or a rendered span among them), which fold back to exactly the bytes
  `caps[0][1..]` emits. A differential corpus pins the submenu form with and without spaces, at one
  and several levels, taking precedence over a comma, with a leading caret, with an escaped closing
  bracket, beside a second menu macro, inside a rendered span, and escaped — plus the escaped
  non-sliceable match the hoist fixes; a structural test pins the node's own levels and its
  source-accurate `location` (the carets are one byte each there, four in the match string); and
  fixtures are added to the whole-pipeline combined-constructs corpus and to the structural recorder
  cross-check, which compares the two constructions' *splitting* of the item list rather than only
  the HTML they fold to. The UI family performs no recognition side effect in either pipeline (a
  menu registers nothing), so there is none to stage. As with every prep piece before it, nothing
  further is wired in.

  *Step 6 prep landed as (`Macros` → the bibliography anchor, the fourth of that map's
  divergences):* the builder now recognizes the **bibliography anchor** (`[[[label]]]`,
  `[[[label,xreftext]]]`) that prefixes a bibliography list item's principal text, as an
  [`Anchor`](../../parser/src/inlines/anchor.rs) node carrying a new `is_bibliography` flag —
  the same "one node kind, two forms told apart by a flag rather than by re-reading the source"
  shape an [`Image`](../../parser/src/inlines/image.rs)'s own `is_icon` already has. It reuses the
  string pipeline's *exact* recognition — [`INLINE_BIBLIO_ANCHOR`](../../parser/src/content/macros.rs)
  is now shared `pub(crate)` — and its gate: the pass fires only when the parser flags that it is
  substituting a bibliography list item's principal text
  ([`in_bibliography_list_item`](../../parser/src/parser/parser.rs), set in
  [`list_item`](../../parser/src/blocks/list_item.rs)), which the tree build sees because
  [`SubstitutionGroup::apply`](../../parser/src/content/substitution_group.rs) clones the parser —
  flag included — before the authoritative pass runs. This is the divergence the audit's own map
  reached *through the parse context* rather than through a construct's spelling: with the
  tree-source swap landed, a real bibliography entry's tree was folding to
  `[<a id="gof"></a>]` where the string pipeline emits `<a id="gof"></a>[gof]`.

  Two things make it unlike the other macro families. First, it is **not a level pass**: its
  pattern is `^`-anchored to the whole content (a `[[[…]]]` later in the entry is left to the
  ordinary inline-anchor pass, which renders it but never catalogs its id — the `is_bibliography_inner`
  suppression already mirrored in [`anchors`](../../parser/src/content/inline_builder/macros/anchors.rs)),
  so it runs once, outside `apply_macros`'s own recursion, ahead of every other family exactly as the
  string step runs it first. Second, the bracketed label the replacer renders (`[label]`, or
  `[xreftext]`) is **left in the flow** — emitted as the sibling nodes following the anchor, sliced
  from the match's own outer brackets and its label range — rather than becoming the node's children:
  that is what lets every family after it see the label exactly as the string pipeline's own later
  passes see the text the replacer pushed (an auto-link or a `link:` macro written in an xreftext is
  linked in both), with no new container for any walk — recognition, side effects, or the eventual
  fold — to descend into. The node's own `reftext` instead carries that bracketed label as the
  **registered** reference text — what a cross-reference to the entry displays — taken from the level's
  match string, i.e. in the *already-substituted* form the replacer itself registers (the contract an
  [`IndexTerm`](../../parser/src/inlines/index_term.rs)'s `terms` already uses), so `[[[gof,A & B]]]`
  catalogs `[A &amp; B]` byte-for-byte as the string path does. [`fold_anchor`](../../parser/src/content/inline_builder/fold.rs)
  passes `render_anchor` a `None` reftext for such a node, mirroring the replacer's own
  `render_anchor(id, None, …)` — the label reaches the output from the flow, not from the node.

  One shape is deferred, documented and pinned by its own divergence test: a label crossing an
  **opaque piece** — a rendered span, a passthrough or STEM expression (not restored yet), or a
  character replacement (`(C)`, a smart apostrophe) — which
  [`build_match_string`](../../parser/src/content/inline_builder/quotes.rs) stands in as a single
  placeholder rather than the markup or entity the string haystack holds there, so the
  already-substituted label cannot be reconstructed. That is exactly the boundary the index-term
  family's own visible term documents, and — for the character replacements — the one every macro
  family already has at this point (the shared match string serves the quotes step too, where the
  replacements have not yet run). An escaped special is *not* affected (a `CharRef::Special`
  contributes its canonical entity), and a bibliography anchor reached through a synthesized run (an
  attribute expansion, or a filtered block's joined seed) is recognized, the run contributing its
  expanded value to the match string just as it does to the string pipeline's haystack.

  The family's deferred recognition side effect is staged alongside it:
  [`apply_biblio_side_effects`](../../parser/src/content/inline_builder/macros/anchors.rs) registers
  the entry under `RefType::Bibliography` with the node's bracketed reftext and raises the same
  duplicate-id warning, and
  [`apply_macro_side_effects`](../../parser/src/content/inline_builder/macros/mod.rs) calls it
  **first**, before the image, link, and anchor/ref functions — the string pipeline's own pass order,
  which matters because a bibliography anchor's duplicate-id warning and an image's
  dangerous-link-scheme warning land in the one shared warnings list (the same ordering concern that
  entry point already records for image-before-anchor, one step earlier). `apply_ref_side_effects`
  correspondingly skips a bibliography anchor, so composing the two neither double-registers the entry
  nor warns against itself. A differential corpus pins both spellings, the label character classes and
  the non-recognized leading-digit label, a `[[[…]]]` that is not at the entry's start, the
  non-escape of a leading backslash, labels carrying an auto-link / a `link:` macro / an escaped
  special, and constructs after the anchor; structural tests pin the node and the flow nodes that
  follow it; registration parity is asserted against an independent golden parser (id, reference text,
  and `RefType` alike), as is the cross-family warning order; and fixtures are added to the
  whole-pipeline combined-constructs corpus and to the structural recorder cross-check — where the two
  constructions reach the same anchor-then-label shape from opposite directions, the recorder
  recovering it from the rendered output. A whole-document test drives the real parse path end to end,
  folding a real bibliography list's trees to their own rendered strings. As with every prep piece
  before it, nothing further is wired in.

  *Step 6 prep landed as (`AttributeReferences` → the `attribute-missing` drop modes, the fifth
  of that map's divergences — and its third real blocker):* the builder now honors the two
  [`attribute-missing`](https://docs.asciidoctor.org/asciidoc/latest/attributes/unresolved-references/#missing)
  modes that *remove* content — `drop` (drop the reference, and the line if that emptied it) and
  `drop-line` (drop the whole line the reference sits on) — closing the last of the audit's
  divergences that golden tests already exercise. Like `hardbreaks` and the unescaped-specials
  classification before it, this is a **blocker** rather than an unclaimed form: real fixtures in
  `tests/asciidoctor_rb/attributes_test.rs` set both modes, so an authoritative fold over a tree
  that left every such reference literal would silently regress them. The other two modes are
  unchanged and were already parity: `skip` (the default) and `warn` both leave the reference in
  place, which [`apply_attribute_references`](../../parser/src/content/inline_builder/attribute_refs.rs)
  reproduces by recording no match at all.

  What the two dropping modes need, and what the node stream did not have, is **line
  granularity**: `apply_attributes` gets it by splitting `Content::rendered` on `\n` and
  processing — and sometimes discarding — one line at a time. A new
  [`MissingHandling`](../../parser/src/content/inline_builder/attribute_refs.rs) carries the
  resolved mode down the transducer's own recursion, a missing reference becomes a
  `DropMissing` match (consumed, splicing nothing) rather than being skipped, and
  [`surviving_lines`](../../parser/src/content/inline_builder/attribute_refs.rs) reproduces that
  same split over a level's own **match string** — whose `\n` bytes are the very ones the rendered
  string carries — returning the line ranges the level still emits. It mirrors the string loop's
  own two decisions exactly: `drop-line` drops a line once it carries a missing reference, `drop`
  only when removing that reference is all it took to empty the line (Asciidoctor's
  `reject_if_empty`, reconstructed by
  [`line_replacement`](../../parser/src/content/inline_builder/attribute_refs.rs) from the level's
  own matches, `\r` guard included).
  [`rebuild_attribute_level`](../../parser/src/content/inline_builder/attribute_refs.rs) then emits
  one survivor at a time, re-emitting between consecutive survivors the `\n` that terminated the
  *previous* survivor — a real source byte, so a dropped line costs the tree no honest span (#944):
  every surviving node still borrows from `'src` with its own precise `line`/`col`, pinned by a
  structural test. When nothing is dropped — every level under `skip`/`warn`, and the overwhelming
  majority under the other two — the line list is the single whole-level range, so the rebuild is
  byte- and structure-identical to the flat walk it has always been.

  Two shapes are deferred, each documented and pinned by a divergence test, and both falling back
  to leaving the reference literal (this step's own pre-increment behavior). The first is a
  **`Styled` span straddling a line break**: a span is one opaque `SPAN_PLACEHOLDER` piece in the
  match string, so its interior `\n`s are invisible here while the string pipeline, holding the
  span's *rendered markup* inline by this point, still splits on them — the line correspondence the
  drop rests on would be wrong, so the drop is disabled for the whole content when one is present
  (a masked passthrough or STEM expression is *not* affected: the string pipeline holds a sentinel
  for it here too, so a multi-line one collapses its lines on both sides alike). The second is a
  missing reference **nested inside a span, under `drop-line`** *when that span is the one being
  dropped from within* — the nested level cannot see the enclosing line, so the enclosing level
  detects it instead, via a recursive `subtree_has_missing_reference` walk that reuses
  `find_attribute_matches`' own recognition; only the multi-line case above escapes it. Under
  `drop` the nested case needs no such detection at all, since removing a reference is a purely
  local edit and a span keeps its enclosing line non-empty either way.

  That detection has an ordering constraint of its own, caught in review:
  [`styled_drop_indices`](../../parser/src/content/inline_builder/attribute_refs.rs) must run
  **before** the splicing recursion, not from inside the level that consumes it. The string
  pipeline replaces every reference on a line in one `replace_all` pass, which never re-scans its
  own replacements — so an attribute whose value happens to *be* reference-shaped (`:x: {nope}`,
  then `{x}`) leaves that text in the output literally, and it neither is nor arms a missing
  reference. Run after the recursion, the walk would have read the already-spliced value as one and
  dropped a line the string pipeline keeps. Hoisting the whole detection to
  `apply_attribute_references` — which then hands the resulting node indices down, translated to
  match-string offsets at the level that uses them (sound because the recursion only rewrites a
  span's *children*, and a span contributes one placeholder piece whatever they are) — is what keeps
  the two in step, and it keeps a synthesized *seed* scanned, since its text is pre-expansion
  content in its own right. The corpus gains fixtures for both halves of this: a reference-shaped
  value, and a value carrying a newline (where the two pipelines agree by construction, since both
  split into lines *before* expanding).

  As with every macro family's own catalog/warning side effect, the `drop-line` mode's *diagnostic*
  (Asciidoctor's "dropping line containing reference to missing attribute", a
  `SkippingReferenceToMissingAttribute` warning) is **not** raised here: it does not change the
  fold's output bytes, so it is deferred to the cutover along with `warn` mode's own warning, whose
  output this step already reproduces. Differential corpora pin both modes over reference-only
  lines dropped at the start, middle, and end of a content, several dropped lines in a row, two
  references on one dropped line, a resolvable reference beside a missing one, escapes (never a
  missing reference, so never a drop), other constructs on both the dropped and the surviving
  lines, and single-line spans; a test pins that a `counter` directive on a *dropped* line has
  still advanced (it resolves in `resolve_counters`' own document-order pass, before any line is
  dropped, exactly as the string loop advances it as it reaches it); fixtures are added to the
  whole-pipeline combined-constructs corpus; and a whole-document test drives the real parse path
  end to end. As with every prep piece before it, nothing further is wired in.

  *Step 6 prep landed as (`<<id,>>`'s present-but-empty reference text, the sixth of that map's
  divergences — and its fourth real blocker):* the builder now recognizes the one cross-reference
  form it still deferred, closing out the `Ref{Xref}` family. A shorthand's reference text is made
  *present* by its **comma**, not by what follows it: `InlineXrefReplacer`'s own
  `inner.split_once(',')` records `<<id,>>` (and `<<id,   >>`) as `Some("")`, which renders an
  empty `<a href="#id"></a>`, where a comma-less `<<id>>` records `None` and falls back to the
  target's own reference text (or a bracketed `[id]` when it resolves to none). Like `hardbreaks`,
  the unescaped-specials classification, and the `attribute-missing` drop modes before it, this is
  a **blocker** rather than an unclaimed form: a golden test already exercises it
  (`xref_should_use_title_of_target_as_link_text_when_explicit_link_text_is_empty` in
  `tests/asciidoctor_rb/links_test.rs`, design §5.3's oracle), so an authoritative fold over a tree
  that left the shorthand literal would silently regress it.

  What had blocked it was the *representation*, not the recognition, and closing it needed no new
  node field. The step-3b note reads "an empty child vector cannot distinguish a present-but-empty
  text from no text provided" — true, but the distinction was never the *vector's* to carry:
  [`build_xref_shorthand_node`](../../parser/src/content/inline_builder/macros/xref.rs) now builds
  exactly one [`Text`](../../parser/src/inlines/inline_node.rs) child whenever the shorthand carries
  a comma, empty value and all (a zero-length `'src` borrow where the trim left it, so even the
  degenerate case keeps an honest span), and
  [`fold_xref`](../../parser/src/content/inline_builder/fold.rs) keys `XrefRenderParams::provided_text`
  on the **presence of a child** rather than on the bytes the children fold to. Every text the
  builder recognizes is baked into exactly one child, so the two cases cannot collide — the
  `xref:` macro form's own empty text (`xref:id[]`, and an attribute list whose positional value is
  absent or empty) records `None` in the string replacer too, and correspondingly builds no child.
  With this the shorthand builder is **total**: what a cross-reference defers is now decided
  entirely by the verbatim gate that precedes both builders, so the family's own
  `Option`-returning "this increment defers" machinery is removed rather than left as an
  unreachable branch.

  That representation has one invariant to keep, and it is the one thing outside the family this
  step touched: an empty `Text` node must survive the steps that walk one. Both splitters in
  [`special_chars`](../../parser/src/content/inline_builder/special_chars.rs) deliberately never
  emit an empty run (there is nothing in one to escape), so splitting an empty node *deletes* it —
  which is exactly what happened to a shorthand recognized under an effective order that omits
  `specialcharacters` and therefore ends in
  [`classify_unescaped_specials`](../../parser/src/content/inline_builder/special_chars.rs) (a
  `subs=macros` block whose source spells the delimiters out as `&lt;&lt;install,&gt;&gt;`, which
  the string pipeline matches as written). [`split_text`](../../parser/src/content/inline_builder/special_chars.rs)
  now keeps an empty value as the node it already is. The one place an empty `Text` is *not*
  wanted is the tree's root seed for empty content, where it would be a leaf carrying nothing for
  every consumer to skip; [`build_for_group`](../../parser/src/content/inline_builder/mod.rs) now
  declines to seed one at all, which also makes the empty-content tree the same (`[]`) under every
  group rather than only under those that ran a splitting step.

  A differential corpus extends the existing xref fixtures with both spellings of the empty text
  (bare and whitespace-only) and one over a *derived* destination — `render_xref`'s
  `(None, Some(derived))` branch, which unlike the resolved branch renders `Some("")` as an empty
  body rather than falling back — alongside structural tests pinning the empty child's own
  zero-length span, its absence for a comma-less shorthand, and the classification fixture above;
  fixtures are added to the whole-pipeline combined-constructs corpus, the broad general-purpose
  sweep, and the structural recorder cross-check. A whole-document test drives the real parse path
  end to end on the golden test's own shape, where the reference **resolves**: reaching resolution
  is itself part of the fix, since the positional mirror
  ([`mirror_tree_xref_resolution`](../../parser/src/content/content.rs)) skips a list whose node
  count diverges from the string pipeline's deferred segments, so an unrecognized shorthand used to
  cost its whole content the resolved destinations. The [`Ref::children`](../../parser/src/inlines/ref_node.rs)
  field docs record the distinction for a tree consumer. As with every prep piece before it,
  nothing further is wired in.

  *Step 6 prep landed as (the UI and index-term families inside an expanded attribute value, the
  seventh of that map's divergences):* the audit's map names "a macro inside an expanded attribute
  value" as one of its remaining items, and the two macro families whose nodes carry **no
  `Span`-typed field at all** — [`Ui`](../../parser/src/inlines/ui.rs) (`kbd:`/`btn:`/`menu:`) and
  [`IndexTerm`](../../parser/src/inlines/index_term.rs) — now close their half of it. This is the
  *same* lift the anchor family made for its id (the "prep (anchor synthesized boundary lifted)"
  note above) and the bare-e-mail family made for its address, applied to the two families that
  were left: a family that never needs a real `'src` slice has no reason to defer inside a
  [`synthesized`](../../parser/src/content/inline_builder/quotes.rs) run, and each of these
  computes every value it holds either straight out of the level's **match string** — which carries
  a synthesized run's bytes exactly — or, for the one exception, through
  [`text_slice`](../../parser/src/content/inline_builder/quotes.rs).

  The change is correspondingly small, which is the point: the boundary was drawn by *one gate per
  family*, not by anything structural. `kbd:`/`btn:` swaps
  [`range_is_verbatim`](../../parser/src/content/inline_builder/macros/image.rs) for
  [`range_is_verbatim_or_synthesized`](../../parser/src/content/inline_builder/macros/image.rs)
  (its keys and label already came from the match string via `split_kbd_keys` /
  `normalize_index_text`); `menu:` drops the `piece.synthesized` rejection from
  [`menu_match_is_sliceable`](../../parser/src/content/inline_builder/macros/ui.rs) and reads its
  one `'src`-sliced value — the menu *name* — through `text_slice` instead of
  [`source_slice`](../../parser/src/content/inline_builder/quotes.rs), which would have offered
  only that run's coarse span (§4.4) rather than the name's exact bytes; and both index-term
  spellings drop their `range_overlaps_synthesized` check, keeping only the
  `SPAN_PLACEHOLDER` one, since a term's shown text is reconstructed from the match string and
  nowhere else. Every other boundary each family draws is untouched: an escaped special or a
  rendered span in a `kbd:`/`btn:` bracket, in a menu *name* or item text (the admitted `&gt;`
  submenu caret aside), or in a visible term still defers, as does an `indexterm2:` term carrying
  an attribute list. As throughout, only the node's **`location`** takes the coarse enclosing-span
  fallback (§4.4); the values themselves are exact.

  Differential corpora drive each family's expanded-value forms — an expanded menu name, item
  list, and submenu path; expanded keyboard keys and button labels; a whole macro arriving from an
  expanded value; both index-term spellings, visible and concealed, whole and partial, and a kept
  literal parenthesis beside an expanded term — through the real, public
  `SubstitutionGroup::Normal::apply` as the golden, since these need the `AttributeReferences`
  step the family-scoped `golden_macros` helper deliberately omits. Structural tests pin the
  exact-value/coarse-location split; the wholly-synthesized **seed** path
  ([`build_from_value`](../../parser/src/content/inline_builder/mod.rs), a filtered multi-line
  block) is covered for both families; a whole-document test drives the real parse path end to end;
  and fixtures are added to the whole-pipeline combined-constructs corpus and to the structural
  recorder cross-check — where the recorder side, which has always recovered these from the string
  pipeline's own render params, can now be compared *structurally* against a builder that
  recognizes them too. Re-running the corpus-wide fold-parity audit (tree building forced on for
  every parse in the suite) confirms the divergence set strictly **shrank**: the three sources the
  divergence tests pinned are gone and no new one appeared. As with every prep piece before it,
  nothing further is wired in.

  *Step 6 prep landed as (cross-references inside an expanded attribute value, closing the third
  family of that same divergence):* the increment directly above closed "a macro inside an expanded
  attribute value" for the two families whose nodes carry no `Span`-typed field, and named the
  families that hold an [`Attrlist`](../../parser/src/attributes/attrlist.rs)`<'src>` "or another
  `Span`-typed field (image, link, cross-reference)" as still deferring. Auditing that grouping
  found the **cross-reference** family does not belong in it: a
  [`Ref`](../../parser/src/inlines/ref_node.rs)`{Xref}` node is built with `attrs: None` in both
  spellings, because the `xref:` macro's own attribute-list text is parsed from a *newline-normalized
  copy* rather than a source slice
  ([`xref_macro_text`](../../parser/src/content/inline_builder/macros/xref.rs), mirroring
  `InlineXrefReplacer`, which parses that same copy) — and every other value the node holds is a
  computed `String` or a display text. So the family needs no honest `'src` slice at all, and the
  same lift the anchor, bare-e-mail, UI, and index-term families made applies to it unchanged.

  As with those, the change is **one gate per family**:
  [`find_xref_matches`](../../parser/src/content/inline_builder/macros/xref.rs) swaps
  [`range_is_verbatim`](../../parser/src/content/inline_builder/macros/image.rs) for
  [`range_is_verbatim_or_synthesized`](../../parser/src/content/inline_builder/macros/image.rs), and
  the two builders read what used to come from an `'src` slice out of the level's **match string**
  instead — which carries a [`synthesized`](../../parser/src/content/inline_builder/quotes.rs) run's
  bytes exactly, and is the very text the string replacer's own `inner.split_once(',')` /
  `raw_text.contains('=')` see. `build_xref_shorthand_node` splits the shorthand's inner on the match
  string rather than on the inner's source slice, and both builders take a display text's value from
  the match string when it crosses a synthesized run while still *borrowing* it from `'src`
  (via the location `source_slice` gives) when it does not, so the common case allocates nothing
  (§4.5). Only the node's `location` (and its children's) takes §4.4's coarse enclosing-span
  fallback; every value is exact.
  Every other boundary the family draws is untouched: an [`atomic`](../../parser/src/content/inline_builder/quotes.rs)
  piece — an escaped special, a rendered span, or an expanded value's own unescaped `<` (a
  [`Raw`](../../parser/src/inlines/inline_node.rs) leaf, §3.4.1) — still defers, each with its own
  divergence test, and the string pipeline's own `id.contains('<')` guard leaves that last one
  literal too, so the two agree.

  Differential corpora drive both spellings' expanded-value forms — an expanded id, reference text,
  attribute-list text, and inter-document target; an expanded shorthand id and trimmed reference
  text; a present-but-empty text behind an expanded id; a cross-reference inside a rendered span —
  through the real, public `SubstitutionGroup::Normal::apply` as the golden, since these need the
  `AttributeReferences` step the family-scoped `golden_xref_with` helper deliberately omits.
  Structural tests pin the exact-value/coarse-location split for both spellings; a whole-document
  test drives the real parse path end to end; and fixtures are added to the whole-pipeline
  combined-constructs corpus and to the structural recorder cross-check. The wholly-synthesized
  **seed** path ([`build_from_value`](../../parser/src/content/inline_builder/mod.rs)) gains
  cross-reference fixtures too, and the test that pinned that path's deferral — which used an
  `<<target>>` fixture — now pins it with a `link:` macro, the family for which
  [`Attrlist::parse`](../../parser/src/attributes/attrlist.rs) genuinely does read its `source:
  Span<'src>`'s bytes as content, so a real `'src` slice is not optional. Re-running the corpus-wide
  fold-parity audit (tree building forced on for every parse in the suite) confirms the divergence
  set strictly **shrank**: four whole-corpus sources are gone and no new one appeared. As with every
  prep piece before it, nothing further is wired in.

  *Step 6 prep landed as (the image and link families inside an expanded attribute value,
  closing the last of that divergence):* the two increments above closed "a macro inside an
  expanded attribute value" for every family whose node carries no `Span`-typed field; the two
  that are left — [`Image`](../../parser/src/inlines/image.rs) and the
  [`Ref`](../../parser/src/inlines/ref_node.rs)`{Link}` family, the two that hold a real
  [`Attrlist`](../../parser/src/attributes/attrlist.rs)`<'src>` — now make the same lift for
  everything *except* that attribute list, which is the one value in this whole module that a
  match string genuinely cannot supply:
  [`Attrlist::parse`](../../parser/src/attributes/attrlist.rs) reads its `source: Span<'src>`'s
  bytes **as content**, not merely as a location tag, so an expanded value — which has no `'src`
  slice of its own — leaves it nothing to parse. The boundary therefore moves from *the family*
  to *the one capture that needs a slice*, which is what makes the common real-world spellings
  (`image:{logo}[Logo]`, `link:{url}[Docs]`, `https://{host}/path`) work while the genuinely
  blocked ones stay documented divergences.

  Concretely, the same one-gate-per-family swap
  ([`range_is_verbatim`](../../parser/src/content/inline_builder/macros/image.rs) →
  [`range_is_verbatim_or_synthesized`](../../parser/src/content/inline_builder/macros/image.rs))
  the four prior families made, plus a narrower check where a slice is still required:

  - **Image / icon.** [`build_image_node`](../../parser/src/content/inline_builder/macros/image.rs)
    reads the macro *name* from the level's match string rather than from `location.data()` (which
    is the coarse enclosing span for a wholly-expanded macro, so the `image:`-vs-`icon:`
    discriminant would otherwise be read off the attribute *reference*), and its *target* through
    [`text_slice`](../../parser/src/content/inline_builder/quotes.rs) — still borrowing from `'src`
    when the target is verbatim (§4.5), exact when it is not. The **bracket** keeps the verbatim
    check: a non-empty attribute list crossing a synthesized run leaves the macro literal, while an
    **empty** one (`image:{logo}[]`) needs no bytes at all and parses from the same zero-length span
    an absent group already used — so even a wholly-expanded `image:x.png[]` is recognized.
  - **Links.** Every value the three URL-link passes hold — the scheme, the URL, the bracketed
    display text — is already computed out of the match string, so
    [`build_inline_link_node`](../../parser/src/content/inline_builder/macros/links.rs) and
    [`find_link_macro_matches`](../../parser/src/content/inline_builder/macros/links.rs) needed only
    the gate swap. The **attribute-list-bearing display text** (a `link:`/formal-URL `=`, a
    `mailto:` `,` subject) keeps the verbatim check, for the same `Attrlist<'src>` reason and
    alongside the multi-line form the family already defers there.
  - **One extra rule, for the `link:`/`mailto:` macro pass only.** That pass additionally requires
    its own `link:`/`mailto:` **marker** to be verbatim, so a *wholly* expanded macro (`{m}`, where
    `:m: link:index.html[Docs]`) stays literal. The reason is not the node but the staged side
    effect: [`link_form`](../../parser/src/content/inline_builder/macros/links.rs) tells this pass's
    nodes from the other two link passes' by whether the node's `location` starts with that literal
    marker — the "no new node field" signal the combined-entry-point increment introduced to replay
    the string pipeline's family-pass registration order — and a coarse §4.4 location cannot carry
    it. A marker written in the source (`link:{url}[Docs]`) keeps an honest location end to end and
    is recognized. The URL-link and bare-e-mail passes need no such rule: their own targets never
    carry the `link:`/`mailto:` scheme `link_form` keys on, so a wholly expanded `{site}`
    auto-link *is* recognized.

  Building the differential corpora surfaced a latent test-harness hazard worth recording, of
  exactly the kind these lifts keep exposing: the family-scoped golden helper
  ([`golden_macros_with`](../../parser/src/content/inline_builder/test_support.rs)) deliberately
  omits the `AttributeReferences` step, so a fixture carrying a reference makes the two sides read
  *different* text. The link family's own registration corpus used `{sp}` as an interleaving
  separator, which was inert only while every link family deferred inside a synthesized run: with
  the lift, the builder sees the expanded space and links a following bare URL where the golden —
  still holding a literal `}` boundary character `INLINE_LINK` rejects — does not. Those separators
  are now plain spaces, which is what the fixture always meant (and which makes its interleaved
  registration-order assertion non-vacuous for the first time). Every expanded-value fixture in this
  increment is driven through the real, public `SubstitutionGroup::Normal::apply` instead.

  Differential corpora drive each family's expanded-value forms — an expanded image/icon target,
  whole or partial, with a verbatim, an empty, and a positional/named attribute list; an expanded
  `link:`/`mailto:` target and display text; expanded auto-link, formal-URL and angle-bracketed
  hosts; a wholly expanded auto-link — through that real entry point as the golden, alongside
  structural tests pinning the exact-value/coarse-location split and the match-string-read macro
  name. Registration parity is asserted for both staged side effects against an independent golden
  parser (the same two-independent-parsers discipline), including the interleaved link forms that
  pin the marker rule above. Fixtures are added to the whole-pipeline combined-constructs corpus,
  the synthesized-seed sweep (whose own deferral test keeps its `link:` macro fixture — now
  deferred by the marker rule rather than by a whole-family gate), and the structural recorder
  cross-check. Re-running the corpus-wide fold-parity audit (tree building forced on for every
  parse in the suite) confirms the divergence set strictly **shrank**: one more whole-corpus source
  is gone and no new one appeared. As with every prep piece before it, nothing further is wired in.

  *Step 6 prep landed as (a cross-reference text crossing an escaped special, via structured
  display-text children):* every macro family in this module draws a boundary at an **atomic**
  piece — "an escaped special or a rendered span" — and the two have been treated as one boundary
  throughout, relaxed only where a family *consumes* the escaped delimiter and never slices it
  (the angle-bracketed URL's `&lt;`/`&gt;`, a menu's `&gt;` submenu caret, the `<<id>>` shorthand's
  own `&lt;&lt;`/`&gt;&gt;`). Auditing that pairing for the cross-reference family — the one whose
  node holds nothing `Span`-typed at all, which is what already let it lift the *synthesized*-run
  half of the same gate — found the two halves are not alike. A rendered span is opaque because
  [`build_match_string`](../../parser/src/content/inline_builder/quotes.rs) stands it in as one
  `SPAN_PLACEHOLDER` where the string pipeline's own haystack holds its **markup** inline; an
  escaped [`CharRef`](../../parser/src/inlines/char_ref.rs)`::Special`, by contrast, contributes its
  canonical entity — the *very bytes* that haystack carries at that position. So a family that
  reads its values out of the match string sees exactly what the string replacer sees, and the
  gate need not reject it.

  A third gate, [`range_has_no_opaque_piece`](../../parser/src/content/inline_builder/macros/image.rs),
  expresses that distinction alongside its two siblings, and
  [`find_xref_matches`](../../parser/src/content/inline_builder/macros/xref.rs) adopts it — the same
  one-gate-per-family swap the synthesized-run lifts made. Every value the family computes then
  follows unchanged, because each already came from the match string: the target (`xref:foo&bar[…]`
  reads the id `foo&amp;bar`, exactly as the string replacer does), the `raw_text.contains('=')`
  attribute-list probe, the attrlist parse itself (already over a newline-normalized *copy*, never a
  source slice), and the shorthand's own `inner.split_once(',')`. The shorthand's
  `id.contains('<')` guard needs no counterpart either: a literal `<` can only come from rendered
  markup, which is still opaque, so the two pipelines agree by construction.

  The one thing that did have to change is the **shape of the reference text**. Baking an
  already-escaped `&lt;` into a single [`Text`](../../parser/src/inlines/inline_node.rs) child would
  have the fold escape it a second time (a `Text` node is *logical* text — design §3.4), so a text
  crossing an escaped special is instead rebuilt as **structured children** with
  [`emit_range`](../../parser/src/content/inline_builder/quotes.rs): the special stays the `CharRef`
  it already is, keeping its own precise `'src` span (#944), and folds back to the same entity the
  string replacer's text carries. This is the shape a footnote's own content already has, reached
  here for the first time by a reference-bearing family — and it is the mechanism the *remaining*
  half of this boundary needs (see below). The one text a family computes rather than slices — an
  attribute list's positional value, parsed off the escaped copy — takes the complementary
  treatment: `unescape_specials` puts the entity back to its character so the node holds logical
  text and the fold performs the single escape, a no-op for every text that carries no escaped
  special. The `xref:` macro form's own `raw_text.replace("\\]", "]")` unescape becomes a **gap in
  the emitted ranges** — every byte but the backslash is emitted — rather than a rewrite of each
  recovered run: a review caught that two adjacent `Text` runs need no atomic piece between them
  (an attribute expansion splices its value as its own node), so a value ending in a backslash
  followed by a literal `]` puts the pair astride two runs, which a per-run rewrite would miss.
  Skipping it by range is boundary-agnostic, and leaves every surviving fragment borrowing `'src`
  (§4.5) where a rewritten value would have had to own its bytes. The `<<id,text>>` shorthand takes
  no such unescape at all — it has no bracket to escape, and `InlineXrefReplacer`'s own shorthand
  branch performs no replace — so the shared helper takes it as a parameter rather than applying it
  to both spellings (the first draft of this increment applied it to both, which the shorthand
  fixtures added here now pin against).

  Landing this also closed a latent gap of exactly the kind the `footnoteref:` and menu increments
  closed for their own families: the macro form ran its escape branch *after* the gate, so an
  escaped macro the gate rejected (`\xref:sec[with *bold* reftext]`) was left fully unrecognized —
  backslash and all — where the string replacer drops the backslash and keeps the rest. The check is
  hoisted ahead of the gate, mirroring `InlineXrefReplacer`'s own `caps.get(1)`-first order, and
  needs no gate of its own: dropping the backslash keeps the rest of the match as its own original
  nodes, which fold to exactly the bytes `caps[0][1..]` emits. (The shorthand already ran its escape
  first, for the same reason.)

  What remains deferred is the boundary's other half, unchanged and still the audit map's own
  outstanding item: a display or reference text crossing a **rendered span**
  (`xref:id[*bold*]`, `<<id,*bold*>>`, `link:x[*bold*]`). Its markup exists only at fold time, and
  that markup is what the string replacer's `=` probe and the pattern's `]` boundary read — so
  admitting it would mean invoking a renderer while building the tree, which this module does not
  do (the same reasoning the bare-e-mail increment's boundary-class deferral records). The four
  reference-bearing link/image families likewise keep the escaped-special boundary for now: each
  holds a value that must ride on the node as an `'src` slice or as an already-final computed
  string, so the lift is not the single gate swap it is here. Differential corpora extend the
  existing cross-reference fixtures with an escaped special in a reference text (both spellings), in
  an attribute-list text, in a *target*, escaped, inside a rendered span, and beside other
  constructs, alongside structural tests pinning the recovered children's own precise spans and the
  logical-text round trip; fixtures are added to the whole-pipeline combined-constructs corpus, the
  broad general-purpose sweep, and the structural recorder cross-check, and a whole-document test
  drives the real parse path end to end. Re-running the corpus-wide fold-parity audit (tree building
  forced on for every parse in the suite) confirms no new divergence appeared. As with every prep
  piece before it, nothing further is wired in.

  *Step 6 prep landed as (a `link:`/`mailto:` macro crossing an escaped special, the second family
  to take the third gate):* the increment above named the four reference-bearing link/image families
  as still holding the escaped-special boundary, "each holds a value that must ride on the node as an
  `'src` slice or as an already-final computed string". Auditing that claim family by family shows it
  is true of the *whole* family for none of them, and of exactly one **capture** in the
  `link:`/`mailto:` macro — which is the same shape the expanded-value lift already took for this
  family ("the boundary moved from *the family* to *the one capture that still needs an `'src`
  slice*"). So [`find_link_macro_matches`](../../parser/src/content/inline_builder/macros/links.rs)
  makes the same one-gate swap to
  [`range_has_no_opaque_piece`](../../parser/src/content/inline_builder/macros/image.rs), and a macro
  whose **target** or **display text** crosses an escaped special (`link:a&b.html[]`,
  `link:index.html[a < b]`, `mailto:a&b@example.org[]`) is recognized as the same
  [`Ref`](../../parser/src/inlines/ref_node.rs)`{Link}` node its verbatim spelling builds. Nothing on
  the node needs the source's own `<`/`>`/`&`: the target is a computed string this pass reads out of
  the level's match string (`a&amp;b.html` — the very bytes `InlineLinkMacroReplacer` computes from
  its own escaped haystack), which is also what `has_dangerous_scheme`, the `,`/`=` attribute-list
  probes, and the `^`/`\]` handling see, and what the eventual
  [`register_link`](../../parser/src/content/inline_builder/macros/links.rs) records.

  The display text takes the structured-children shape the cross-reference family introduced, and the
  mechanism is now literally shared: `xref_text_children` moves to
  [`macro_text_children`](../../parser/src/content/inline_builder/macros/mod.rs) alongside the other
  cross-family macro helpers, so the one subtle part — recovering the text with `emit_range` (each
  special staying its own `CharRef`, with its own precise `'src` span, escaped once rather than
  twice) and expressing the `\]` unescape as a *gap* in the emitted ranges rather than a per-run
  rewrite — cannot drift between the two families that now perform it. The link macro's own `^`
  window suffix is one ASCII byte past that text, so it is simply left outside the emitted range.
  A **bare** macro is the case that made this more than the gate swap: its shown text is not a
  bracketed slice but the target — the whole target, or (under `hide-uri-scheme`) its scheme-stripped
  tail. Rather than bake that already-escaped string into one `Text` node (which the fold would escape
  a second time, and which the structural recorder cross-check immediately caught as a shape
  regression), the builder recovers it from *the target group's own range*: `URI_SNIFF` is
  `^`-anchored, so the strip always leaves a suffix, and the children start that many bytes into the
  range. The one text this family still computes rather than slices — an attribute list's positional
  value — needs no unescaping counterpart, because its own branch requires a **verbatim** `'src`
  slice, which carries no entity; that capture is therefore the one that keeps the escaped-special
  boundary (`Attrlist::parse` reads its source span's bytes as content, and those bytes are the
  source's `<`, not the `&lt;` the replacer parses from its escaped copy). `unescape_specials` stays
  in the cross-reference family, where the complementary case — a positional value parsed off the
  *escaped* copy — actually arises, with its doc note now recording why a caller applies it only where
  its value's own range crosses such a special (under an effective order that never escapes, a literal
  `&` survives into a verbatim run and there is no entity to undo).

  As in the cross-reference, `footnoteref:`, and menu increments, the family's escape check is
  **hoisted ahead of the gate**, mirroring `InlineLinkMacroReplacer`'s own
  `caps[0].starts_with('\\')`-first order and closing the same latent gap: an escaped
  `\link:x[*bold*]` whose match the gate rejects now still drops its backslash, where before it was
  left unrecognized, backslash and all. Differential corpora extend the family's fixtures with an
  escaped special in a target, in a display text, in both, beside the `\]` unescape and the `^`
  suffix, and in an escaped macro, alongside structural tests pinning the recovered children's own
  precise spans (including the `hide-uri-scheme` slice past the scheme) and divergence tests for the
  two forms that still defer (an attribute-list text crossing a special, in both spellings, and a
  display text crossing a rendered span). Registration parity is asserted for the escaped-special
  forms against an independent golden parser, and fixtures are added to the whole-pipeline
  combined-constructs corpus, the broad general-purpose sweep, and the structural recorder
  cross-check. Re-running the corpus-wide fold-parity audit (tree building forced on for every parse
  in the suite) confirms no new divergence appeared and that the surviving set is unchanged — this
  form was pinned by the family's own divergence test rather than by any whole-corpus fixture, which
  is why the audit's own itemized map never listed it. The remaining half of that map item — a
  display or reference text crossing a **rendered span** — is unchanged, as is the escaped-special
  boundary for the auto-link/formal-URL, bare-e-mail, and image families, each of which holds a value
  (`INLINE_LINK`'s own trailing-punctuation arithmetic over the target, an address baked as its own
  display text, an `Attrlist<'src>`) that a later increment must take up on its own terms. As with
  every prep piece before it, nothing further is wired in.

  *Step 6 prep landed as (the auto-link / formal-URL family crossing an escaped special, the third
  family to take the third gate):* the increment above named `INLINE_LINK`'s own
  trailing-punctuation arithmetic as the value that keeps this family from the same one-gate swap.
  Auditing it shows the arithmetic is not the obstacle the note assumed — it runs over the *match
  string*, exactly as `InlineLinkReplacer`'s runs over its own escaped haystack, so the two agree by
  construction — but it does have one *boundary* consequence, and that is the whole of what still
  defers. So [`build_inline_link_node`](../../parser/src/content/inline_builder/macros/links.rs)
  makes the swap to
  [`range_has_no_opaque_piece`](../../parser/src/content/inline_builder/macros/image.rs), for
  **both** of the pattern's branches: a bare auto-link, a formal URL link, and an angle-bracketed
  URL whose target or display text crosses an escaped special (`https://example.org/?a=1&b=2`,
  `https://example.org[a < b]`, `<https://example.org/a&b>`) is recognized as the same
  [`Ref`](../../parser/src/inlines/ref_node.rs)`{Link}` node its verbatim spelling builds, folding
  through the identical `render_link`. Nothing on the node needs the source's own `<`/`>`/`&`: the
  target is a computed string read off the match string, and the display text becomes **structured
  children** through [`macro_text_children`](../../parser/src/content/inline_builder/macros/mod.rs),
  the shared helper the cross-reference family introduced and the `link:` macro generalized — for a
  *bare* link (an auto-link's URL, an angle link's interior) too, whose shown text is recovered from
  the URL group's own range, offset past the `hide-uri-scheme` strip exactly as a bare `link:`
  macro's is, rather than baked already-escaped into one `Text` the fold would escape twice. The
  ANGLE branch keeps its own narrower gate — the `&lt;`/`&gt;` delimiters are escaped specials the
  node consumes and never slices, so only the *interior* is gated — now expressed inside
  [`build_angle_link_node`](../../parser/src/content/inline_builder/macros/links.rs) beside the
  branch's two escapes. As in the four increments before it, the family's escape check is **hoisted
  ahead of the gate**, mirroring `InlineLinkReplacer`'s own scheme-backslash-first order and closing
  the same latent gap: an escaped `\https://example.org/*bold*` whose match the gate rejects now
  still drops its backslash.

  What the trailing-punctuation strip does cost is one narrow deferral, of a kind no earlier family
  has had: the strip keys off the *target's final character* (a `;` or `:`, plus an adjacent `)`),
  and a bare URL ending in a literal `&` reaches this pass as `…&amp;` — whose own final `;`
  satisfies it. The string replacer happily splits that entity (target `…&amp`, a literal `;` after
  the link); a node list cannot, because the boundary would fall *inside* a
  [`CharRef`](../../parser/src/inlines/char_ref.rs) leaf that
  [`emit_range`](../../parser/src/content/inline_builder/quotes.rs) can only emit whole. That one
  form is left literal, pinned by its own divergence test, alongside the two boundaries the family
  shares with its siblings: an attribute-list-bearing display text (parsed as a real
  `Attrlist<'src>` from the source's own bytes) and a text or URL crossing a **rendered span**.

  Landing this also closed a latent gap that had been live since the cross-reference increment first
  introduced `macro_text_children`, and that this increment's own corpus-wide audit surfaced: the
  helper recovered a non-crossing text by re-reading its `source_slice(…).data()`, on the reasoning
  that a *verbatim* range maps one-to-one onto source. It does not always — a verbatim range need
  not be **contiguous** in the source. An earlier step can drop a byte from the flow without
  splicing a node in its place, and
  [`apply_attribute_references`](../../parser/src/content/inline_builder/attribute_refs.rs) does
  exactly that for an escaped reference (`link:x[\{name}]`), dropping the backslash as a *gap* in
  the ranges it emits and leaving two adjacent verbatim runs whose match-string bytes run on while
  their source spans skip one. Re-reading the enclosing span put the backslash back — a character
  the string pipeline no longer carries — so all three families that build a display text through
  the helper rendered it. The value now comes from
  [`text_slice`](../../parser/src/content/inline_builder/quotes.rs), which slices the pieces
  themselves (still borrowing `'src` for a single run, §4.5) and so cannot reintroduce a byte the
  flow dropped; a range it declines falls through to the structured rebuild rather than to a
  defensive fallback. Differential corpora extend the family's three fixture sets (non-angle,
  `hide-uri-scheme`, and angle) with an escaped special in a target, in a display text, in both,
  beside the `\]` unescape, the `^` suffix, the scheme strip and the punctuation strip, and in an
  escaped link, alongside structural tests pinning the recovered children's own precise spans and
  divergence tests for each form that still defers (an attribute-list text crossing a special, a
  bare URL whose strip would split one, a restored entity, and a rendered span in either branch).
  Registration parity is asserted for the escaped-special forms against an independent golden
  parser, and fixtures are added to the whole-pipeline combined-constructs corpus, the broad
  general-purpose sweep, and the structural recorder cross-check. Re-running the corpus-wide
  fold-parity audit (tree building forced on for every parse in the suite) confirms no new
  divergence appeared and that four previously-surviving ones are gone. The escaped-special boundary
  now remains only for the **bare-e-mail** and **image** families, each of which holds a value (an
  address baked as its own display text, an `Attrlist<'src>`) that a later increment must take up on
  its own terms. As with every prep piece before it, nothing further is wired in.

  *Step 6 prep landed as (the bare e-mail auto-link crossing an escaped special — the fourth
  family, and the first to need no gate at all):* the increment above named the **bare-e-mail**
  and **image** families as the two still holding the escaped-special boundary, the first of
  them because it holds "an address baked as its own display text". Auditing that shows the
  address is not baked but *sliced*, exactly like a bare `link:` macro's own target-derived
  text — the shape that family already solved — so the lift needs no new mechanism: the target
  is a computed string read off the level's **match string**
  (`mailto:a&amp;b@example.com` — the very bytes `InlineEmailReplacer` computes from its own
  escaped haystack, and what [`register_link`](../../parser/src/content/inline_builder/macros/links.rs)
  records), and the display text goes through
  [`macro_text_children`](../../parser/src/content/inline_builder/macros/mod.rs), the shared
  helper, so each special stays the [`CharRef`](../../parser/src/inlines/char_ref.rs) it
  already is — with its own precise `'src` span (#944) — instead of being baked, already
  escaped, into one [`Text`](../../parser/src/inlines/inline_node.rs) the fold would escape a
  second time. `a&b@example.com` is therefore recognized as the same
  [`Ref`](../../parser/src/inlines/ref_node.rs)`{Link}` node its verbatim spelling builds,
  folding through the identical `render_link`.

  What *is* new in kind is that this family takes the lift with **no gate**. Its three
  predecessors each swapped [`range_is_verbatim`](../../parser/src/content/inline_builder/macros/image.rs)
  (or its synthesized-admitting sibling) for
  [`range_has_no_opaque_piece`](../../parser/src/content/inline_builder/macros/image.rs); here
  that swap would add a branch no input can reach. An address **cannot cross an opaque piece**:
  every such piece is exactly one [`SPAN_PLACEHOLDER`](../../parser/src/content/inline_builder/quotes.rs)
  (U+E0F0, Unicode category `Co`), which none of [`INLINE_EMAIL`](../../parser/src/content/macros.rs)'s
  character classes admit — not the local part's `[\w_]` / `[\w\-.%+]`, not the domain's
  `[\p{L}\p{Nd}_\-.]`, not the TLD's `[a-zA-Z]` — and a match can neither *begin* nor *end*
  strictly inside an escaped special's entity, since an entity's leading `&` is in no class the
  domain or TLD accepts and its trailing `;` is neither a local-part character nor the `@` a
  local part must be followed by. So the only atomic piece an address range can overlap is a
  **wholly contained** `&amp;` — precisely the one this lift admits. That is the same structural
  argument the sibling auto-link family already makes for its own *required* boundary-prefix
  group ("a placeholder simply fails its match"), reached here through the match's interior
  rather than its boundary. The family's `Option`-returning "this increment defers" machinery is
  removed rather than left unreachable — as the `<<id,>>` increment removed the cross-reference
  family's — so [`build_email_node`](../../parser/src/content/inline_builder/macros/links.rs) is
  now **total**, and the one deferral the family keeps is the *abutting* boundary class it
  already documented, which the placeholder-**prefix** check expresses (a range gate never
  could).

  Differential corpora extend the family's fixtures with a crossing address alone, in flow,
  doubled, and carrying two specials; with a literal `&` the pattern's classes do *not* admit (in
  the domain, and opening the local part), where neither pipeline matches; with an address beside
  but not crossing a special; with the escape (`\a&b@example.com`); and with a construct *inside*
  what would otherwise be an address — a rendered span, a character replacement, a smart em dash —
  which pins the no-gate invariant above as fixtures rather than only as prose. The family's own
  divergence test becomes a **parity** test asserting the three recovered children and the `&`'s
  own precise span, per its "if lifted, fold this into a parity corpus" convention, and the
  abutting-divergence test gains a character-replacement fixture (`a(C)b@example.com`) — the same
  class reached with no macro at all, since `&#169;`'s trailing `;` is no mismatch character.
  Registration parity is asserted for the escaped-special forms against an independent golden
  parser, and fixtures are added to the whole-pipeline combined-constructs corpus, the broad
  general-purpose sweep, and the structural recorder cross-check. Re-running the corpus-wide
  fold-parity audit (tree building forced on for every parse in the suite) confirms the divergence
  set strictly **shrank**: one previously-surviving source is gone (`bert&ernie@sesamestreet.com`,
  a real golden fixture) and no new one appeared. The escaped-special boundary now remains only
  for the **image** family, whose [`Attrlist`](../../parser/src/attributes/attrlist.rs)`<'src>`
  bracket is the one value in this module a match string genuinely cannot supply. As with every
  prep piece before it, nothing further is wired in.

  *Step 6 prep landed as (the image family crossing an escaped special — the fifth and last
  family, closing the boundary as a **family** boundary altogether):* the increment above named
  the **image** family as the one still holding the escaped-special boundary, "whose
  [`Attrlist`](../../parser/src/attributes/attrlist.rs)`<'src>` bracket is the one value in this
  module a match string genuinely cannot supply". That is true of the *bracket* — and of nothing
  else the family holds. The expanded-value increment had already moved the family's boundary onto
  that one capture for a synthesized run (an image's macro name and target read from the match
  string and [`text_slice`](../../parser/src/content/inline_builder/quotes.rs), an empty bracket
  parsed from a zero-length span); this makes the same one-gate swap for an escaped special, so
  [`find_image_matches`](../../parser/src/content/inline_builder/macros/image.rs) takes
  [`range_has_no_opaque_piece`](../../parser/src/content/inline_builder/macros/image.rs) and
  `image:a&b.png[]` is recognized as the same
  [`Image`](../../parser/src/inlines/image.rs) node its verbatim spelling builds. The target
  comes off the match string (`a&amp;b.png` — the very bytes `InlineImageMacroReplacer` reads as
  `caps[1]`, renders as the `src`, derives its `default_alt` from, and
  [`apply_image_side_effects`](../../parser/src/content/inline_builder/macros/image.rs)
  registers), reached as `text_slice`'s fallback so a verbatim or synthesized target still
  borrows `'src` (§4.5) rather than allocating.

  This family needs no display-text recovery at all — an image has no shown text, only an `alt`
  the attribute list or the target's own basename supplies — so
  [`macro_text_children`](../../parser/src/content/inline_builder/macros/mod.rs), the shared helper
  its four predecessors each needed, has no part here. Nor can a boundary split an entity: the
  pattern's match begins at `i` (or a backslash) and ends at `]`, and each capture is delimited by
  `image:`/`icon:`, `[`, and `]` — none of which occurs in `&lt;`, `&gt;`, or `&amp;` — so every
  atomic overlap an image range can have is a **wholly contained** entity, the same structural
  argument the bare-e-mail family makes for its own classes. What survives is therefore exactly the
  bracket: a **non-empty attribute list** crossing a special keeps
  [`range_is_verbatim`](../../parser/src/content/inline_builder/macros/image.rs), since
  `Attrlist::parse` reads its `source: Span<'src>`'s bytes *as content* and the source holds one
  character where the match string holds an entity. The escaped-special boundary is no longer a
  *family* boundary anywhere in the macros step — only a **capture** boundary, and only for the
  three attribute lists (a link's and a cross-reference's attribute-list-bearing display text, and
  an image's bracket) that must ride on their node as a real `Attrlist<'src>`.

  The family's escape check is hoisted ahead of the gate — the same fix the `footnoteref:`, menu,
  cross-reference, and link increments each made, closing the same latent gap: before it, an
  escaped `\image:x.png[*bold*]` whose match the gate rejected was left unrecognized *with its
  backslash*, where the string replacer's own leading `caps[0].starts_with('\\')` check drops it.
  That is a fold-parity fix in its own right, pinned by its own test.

  Differential corpora extend the family's fixtures with a target crossing each of the three
  specials, doubled, carrying a verbatim attribute list (positional alt, positional width/height,
  named attributes, and both `link=` forms `apply_image_side_effects` reads), in flow, beside a
  sibling family that took the same lift, inside a rendered span, escaped, and beside — rather than
  crossing — a special; plus a target crossing *both* a synthesized run and an escaped special,
  which only the level's own match string carries the bytes of. The
  `a_macro_over_a_special_character_is_a_documented_divergence` test becomes a **parity** test with
  a structural companion asserting the entity-bearing target, the derived default alt, and the
  node's still-precise `location`, per the "if lifted, fold this into a parity corpus" convention;
  the two boundaries that remain get divergence tests of their own — a non-empty bracket crossing a
  special, and a *restored entity* in the target (`image:a&amp;b.png[]`, where
  `CharacterReplacements` un-escapes what `SpecialCharacters` escaped, leaving an opaque leaf
  rather than entity bytes) — as does the rendered span the old test's fixture is repointed at.
  Registration parity is asserted for the escaped-special forms against an independent golden
  parser, and fixtures are added to the whole-pipeline combined-constructs corpus, the broad
  general-purpose sweep, and the structural recorder cross-check. Re-running the corpus-wide
  fold-parity audit (tree building forced on for every parse in the suite) confirms the divergence
  set strictly **shrank**: one previously-surviving source is gone
  (`image:data:image/svg+xml,<svg onload='alert(1)'></svg>[alt,link=self]`, a real golden fixture
  in `parser/src/tests/security.rs`) and no new one appeared. The only escaped-special divergence
  left anywhere in the macros step is an attribute list; a display or reference text crossing a
  **rendered span** still defers everywhere. As with every prep piece before it, nothing further is
  wired in.

  *Step 6 prep landed as (a **restored entity** as the second recoverable piece, closing that
  divergence for every family at once):* the increment above left a restored entity
  (`&amp;copy;` written in the source, which `SpecialCharacters` escapes to `&amp;amp;copy;` and
  `CharacterReplacements` then un-escapes back to `&amp;copy;`) as the image family's own last
  deferral, and the auto-link family's alongside it. Both are closed here, and not family by
  family: recoverability is a property of the **piece**, not of the family reading across it, so
  naming the new class once lifts the boundary everywhere a family's gate is already
  [`range_has_no_opaque_piece`](../../parser/src/content/inline_builder/macros/image.rs).

  [`build_match_string`](../../parser/src/content/inline_builder/quotes.rs) gives a
  [`CharRef`](../../parser/src/inlines/char_ref.rs)`::Entity` leaf its **own bytes**, exactly as it
  already gives a `CharRef::Special` its canonical entity, and for the identical reason: those
  bytes *are* what the string pipeline's haystack holds at that position from the replacements step
  onward, and the fold emits them verbatim (`fold`'s `Entity` arm), so the two sides agree with **no
  renderer involved** — the distinction that separates this class from a rendered span, whose markup
  exists only at fold time. The leaf stays `atomic` (it is one indivisible node, never sliced) but
  becomes *recoverable*, which is precisely the distinction the third gate draws. Nothing else in
  any family changes: a target or computed value reads the entity's bytes off the match string, and
  a display text keeps the leaf as its own child through `macro_text_children`'s `emit_range` path,
  where one `Text` holding `&amp;copy;` would have had its `&amp;` escaped a second time.

  One computed value had no range to rebuild from, and is the increment's only new code: a
  cross-reference's **attribute-list positional text**, which comes back from an `Attrlist` parse of
  a normalized *copy* of the match string rather than from a range of it. The single-`Text`
  `unescape_specials` that branch used is replaced by
  [`escaped_value_children`](../../parser/src/content/inline_builder/macros/xref.rs), which
  re-derives §3.4's trichotomy from the value's own bytes: an escaped special becomes the character
  a `Text` holds *logically* (re-escaped once at fold time), a restored entity becomes its own
  `CharRef::Entity` child (emitted verbatim). The scan is left to right, one `&amp;` at a time,
  because the two classes nest — `&amp;amp;copy;` is a literal `&amp;` followed by the letters
  `copy;`, **not** a `&amp;copy;` entity — and only consuming the `&amp;amp;` first tells them
  apart, the same one-level unwind the fold performs in reverse. The entity-name class itself is
  hoisted into a shared `ENTITY_NAME` constant that both the restore-entities replacement rule and
  the new [`restored_entity_pattern`](../../parser/src/content/substitution_step.rs) are built from,
  so the two spellings of one class cannot drift.

  What does **not** move is the boundary held by a gate other than the opaque-piece one: the UI
  family (a `kbd:`/`btn:` key or label, a menu name or item) and a footnote's text each still need a
  value they can slice from `'src`, for a restored entity exactly as for an escaped special, and a
  test pins the two spellings side by side so they move together whenever that boundary is lifted.
  The three attribute-list captures that must ride on a node as a real `Attrlist<'src>` (a link's
  attribute-list-bearing display text, an image's non-empty bracket) likewise keep
  `range_is_verbatim` — the source holds `&amp;amp;copy;` where the match string holds `&amp;copy;`
  — while a cross-reference's own attribute list, parsed from a normalized copy, takes both lifts.

  Differential corpora gain, per family, a target and a display text crossing a restored entity in
  each of its spellings (`&amp;amp;`, a named entity, a numeric one), doubled, crossing *both* an
  entity and an escaped special, in flow, inside a rendered span, beside a sibling family, escaped,
  and beside — rather than crossing — an entity; plus structural companions asserting the recovered
  child's own precise span, the entity-bearing target and derived default alt, and the one-level
  unwind of a doubly-escaped entity. The two `…_over_a_restored_entity_is_a_documented_divergence`
  tests become **parity** tests per the "if lifted, fold this into a parity corpus" convention, and
  the boundaries that remain get divergence tests of their own. Fixtures are added to the
  whole-pipeline broad sweep, the group-parity corpus (so an order that never reaches the
  replacements step is exercised too), and the structural recorder cross-check.

  Re-running the corpus-wide fold-parity audit (tree building forced on for every parse in the
  suite) confirms the divergence set strictly **shrank**: five previously-surviving sources are
  gone, all of them real golden fixtures rather than constructed ones —
  `http://example.com[sam&#93;ple]bracket]`, `l&#8217;http://www.irit.fr[IRIT]`,
  `link:My&#32;Documents/report.pdf[Get Report]`, and the two `menu:Tools[… &gt; …]` submenu forms,
  whose caret is a restored entity — and no pre-existing one appeared. (The set's only additions are
  this increment's own new divergence-pinning fixtures for the untouched UI/footnote boundary.) As
  with every prep piece before it, nothing further is wired in: `rendered_html()` remains the string
  pipeline's own string.

  *Step 6 prep landed as (a **reference text** crossing a rendered span — the cross-reference
  family, first of the last remaining class):* with every recoverable piece now admitted, a
  **rendered span** — a [`Styled`](../../parser/src/inlines/styled.rs) span, an already-recognized
  macro node, a masked passthrough — was the one class still deferring as a whole, and the audit
  map's own last *normal*-order item ("a display or reference text crossing a rendered span"). It
  is closed here for the cross-reference family, in both spellings, the same family that went
  first for the escaped-special lift and for the same reason: nothing on a `Ref{Xref}` node is
  `Span`-typed.

  The lift is not another gate relaxation but a change of *which bytes the gate covers*. A
  rendered span is genuinely unrecoverable —
  [`build_match_string`](../../parser/src/content/inline_builder/quotes.rs) stands it in as one
  placeholder where the string pipeline's haystack holds markup that exists only at fold time,
  which is exactly what separates it from a `CharRef` leaf — so every value this family *computes*
  off the match string (its target, an attribute list's parsed positional value) still needs
  [`range_has_no_opaque_piece`](../../parser/src/content/inline_builder/macros/image.rs). A
  **reference text** computes nothing: it becomes structured children through
  [`macro_text_children`](../../parser/src/content/inline_builder/macros/mod.rs), whose
  [`emit_range`](../../parser/src/content/inline_builder/quotes.rs) path clones the opaque piece's
  own **node** whole into them, so the text carries the construct itself rather than the markup it
  will fold to — the "nesting is the point" recovery a footnote's own content has always used,
  applied to a reference's display text.
  [`find_xref_matches`](../../parser/src/content/inline_builder/macros/xref.rs) therefore gates
  the macro form's *target* (group 3) rather than its whole match, and the shorthand's *id half*
  (its inner up to the first `,`, factored out as `shorthand_id_range` so the gate and
  [`build_xref_shorthand_node`](../../parser/src/content/inline_builder/macros/xref.rs) split on
  the same byte) rather than its whole inner. That the shorthand's split cannot be moved unnoticed
  by a comma inside the markup falls straight out of it: such a piece would have to sit in the id
  half, which the gate then rejects. No builder changed at all — `macro_text_children` already
  routed a range it could not slice through `emit_range`, which already cloned an atomic piece
  whole.

  What the structural recovery cannot do is make the *recognition* agree in every case, and this
  is the first increment where that gap is real rather than avoidable: the string replacer matches
  over the markup where the builder matches over one placeholder standing in for it, so the two
  read the same extent only while that markup carries no character the pattern is sensitive to —
  and the builder cannot know what it carries without folding, which building a tree must never do
  (a fold is renderer-dependent; recognition must not be). Three shapes are therefore documented
  divergences, each pinned by its own test, and in each the string pipeline's reading is the
  markup-perturbed one while the tree's is the well-formed one — the same shape of intended
  divergence the quotes step's own crossed delimiters have: a `]` inside the span (which ends the
  macro form's lazy text capture early for the string replacer), a `&gt;&gt;` inside it (the
  shorthand's own terminator), and markup carrying an `=` beside a comma elsewhere in the text
  (which sends the string replacer down its attribute-list branch, where the parse then keeps only
  what precedes that comma). The text shape that keeps the stricter gate outright is a text
  carrying its own **attribute list**: its display text comes back from an `Attrlist` parse of the
  match string rather than from a range of it, and a placeholder inside a *parsed* value has no
  node to be mapped back to — the same reason the image and link families defer their own
  `Attrlist`-bearing captures.

  A differential corpus pins the reference text crossing every opaque shape the earlier steps can
  produce — each quoted form (including an attributed span, whose markup carries the `=` the
  string replacer's own probe reads and this one does not), an image, a link, an anchor, an index
  term, a masked passthrough, a span beside an escaped special and beside a restored entity,
  escaped, in flow, and inside a rendered span of its own — in both spellings, alongside a
  structural test asserting the recovered children's own precise spans (three children where the
  single-`Text` shape this replaced could express one). The passthrough fixtures live in the
  whole-pipeline sweep rather than the family's own step-driven corpus, since only the real
  `SubstitutionGroup::apply` brackets the steps with the extraction that makes a passthrough
  opaque on *both* sides. Fixtures are added to that sweep, to the group-parity corpus (so an
  order that never escapes exercises the recovered children too), and to the structural recorder
  cross-check — where this form can finally be compared at all, since the tree the builder now
  produces is the shape the recorder recovers from the rendered markup. Two tests that used this
  very divergence as their *vehicle* — the resolution mirror's own count-mismatch skip, block-side
  and footnote-side — move to forms that still defer.

  Re-running the corpus-wide fold-parity audit confirms the divergence set strictly **shrank**:
  four previously-surviving sources are gone — the two spellings of
  `xref:sec[with *bold* reftext]`, one nesting an image macro
  (`See xref:sec[image:logo.png[Logo]] now.`), and the golden corpus's own tigers fixture, whose
  reference text is a code span wrapping a passthrough — and the set's only addition is this
  increment's own new divergence-pinning fixture. As with every prep piece before it, nothing
  further is wired in. The remaining families — the `link:`/`mailto:` macro, the auto-link /
  formal-URL family, and the image family's own attribute list — still defer a capture crossing a
  rendered span, each a later increment; and the audit's one structurally different item (an
  effective order that runs `SpecialCharacters` *after* a step that already produced markup,
  `subs=quotes,specialcharacters`) is unchanged, still needing its own policy rather than another
  recognition increment.

  *Step 6 prep landed as (a display text crossing a rendered span — the `link:`/`mailto:` macro
  family, second of that last class):* the same lift, applied to the second of the three families
  the increment above left holding the boundary, and by the same move: not a gate relaxation but a
  change of *which bytes the gate covers*. The `link:`/`mailto:` macro reads exactly one value off
  the level's match string — its **target** — so
  [`find_link_macro_matches`](../../parser/src/content/inline_builder/macros/links.rs) now applies
  [`range_has_no_opaque_piece`](../../parser/src/content/inline_builder/macros/image.rs) to the
  target group (`INLINE_LINK_MACRO`'s group 3) rather than to the whole match. The bracketed
  **display text** reads nothing: it becomes the node's children through the shared
  [`macro_text_children`](../../parser/src/content/inline_builder/macros/mod.rs), whose
  [`emit_range`](../../parser/src/content/inline_builder/quotes.rs) path clones the opaque piece's
  own node whole into them, so the text carries the construct rather than the markup it will fold
  to. The macro's own `link:`/`mailto:` marker and its brackets need no gate — those bytes are
  literal, and no atomic piece (a placeholder, or an entity delimited by `&` and `;`) can supply
  them — and the marker keeps its own stricter, verbatim gate for `link_form`'s sake. **No builder
  changed at all**, and neither did `apply_link_side_effects`: a node's target, which is what it
  registers, is exactly the value that kept the gate.

  What still defers here is what the cross-reference increment's own boundary predicted, one family
  over: a **target** crossing an opaque piece (a target carrying an unconstrained code span, and —
  the shape the audit actually surfaces — a target wrapped in a passthrough,
  `link:++https://…++[]`), since a target is computed off the match string; and a display text
  carrying its own **attribute list**
  (`link:x[a *b* c,role=hl]`, `mailto:a@x.org[a *b* c,Subj]`), whose value comes back from an
  `Attrlist` parse of the source's bytes, where a placeholder inside a *parsed* value has no node
  to be mapped back to. Each gets its own divergence test.

  The recognition-agreement gap this class introduces takes the same three shapes it did for the
  cross-reference family, read through this family's own pattern and probes: a `]` inside the span
  (which ends `INLINE_LINK_MACRO`'s lazy text capture early for the string replacer), markup
  carrying an `=` beside a comma elsewhere in a `link:` text (the replacer's attribute-list probe,
  fired on the markup this side never sees), and a comma inside the span of a `mailto:` text (that
  spelling's own probe). Each is pinned by its own test, and in each the string pipeline's reading
  is the markup-perturbed one while the tree's is the well-formed one — so the test asserts *both*
  that the two disagree and that the tree still builds the well-formed node.

  A differential corpus pins the display text crossing every opaque shape the earlier steps can
  produce — each quoted form (including an attributed span, whose markup carries the `=` the string
  replacer's own probe reads and this one does not), an image, an icon, a UI macro, an index term, a
  span beside an escaped special and beside a restored entity, a span in a target that itself
  crosses an escaped special, under the `\]` unescape and the `^` window suffix, escaped, in flow,
  beside a sibling macro, and inside a rendered span of its own — in both the `link:` and `mailto:`
  spellings, alongside a structural test asserting the recovered children's own precise spans (three
  children where the single-`Text` shape this replaced could express one). As before, the
  passthrough fixtures live in the whole-pipeline sweep rather than the family's own step-driven
  corpus, since only the real `SubstitutionGroup::apply` brackets the steps with the extraction that
  makes a passthrough opaque on *both* sides. Fixtures are added to that sweep, the broad
  general-purpose sweep, the group-parity corpus, and the structural recorder cross-check.

  Re-running the corpus-wide fold-parity audit confirms the divergence set strictly **shrank**: one
  previously-surviving source is gone — `link:https://example.org[with *bold* text]`, a real fixture
  in the Phase 1 byte-parity corpus — and no new one appeared. As with every prep piece before it,
  nothing further is wired in. The auto-link / formal-URL family and the image family's own
  attribute list still defer a capture crossing a rendered span, each a later increment.

  *Step 6 prep landed as (a display text crossing a rendered span — the auto-link / formal-URL
  family, third and last of the reference-bearing families):* the same lift once more, applied to
  the family the increment above left holding the boundary, and by the same move: a change of
  *which bytes the gate covers*, not a relaxation of the gate itself. `INLINE_LINK`'s formal
  spelling (`https://example.org[a *b* c]`, and the ANGLE branch's `[…]` alternative
  `<https://example.org[a *b* c]`, which keeps its `&lt;` and takes the same general path) reads
  every value it computes — the boundary prefix it inspects for an invalid quoted URL, the scheme,
  and the URL that becomes the **target** — out of bytes that all lie *before* the bracketed
  display text, so
  [`build_inline_link_node`](../../parser/src/content/inline_builder/macros/links.rs) now applies
  [`range_has_no_opaque_piece`](../../parser/src/content/inline_builder/macros/image.rs) to the
  match up to that text rather than to the whole match. The text itself reads nothing: it becomes
  the node's children through the shared
  [`macro_text_children`](../../parser/src/content/inline_builder/macros/mod.rs), whose
  [`emit_range`](../../parser/src/content/inline_builder/quotes.rs) path clones the opaque piece's
  own node whole into them. The closing `]` needs no gate — that byte is literal, and no atomic
  piece (a placeholder, or an entity delimited by `&` and `;`) can supply it. **No builder changed
  at all**, and neither did `apply_link_side_effects`: a node's target, which is what it registers,
  is exactly the value that kept the gate.

  Two of this family's forms have no bracketed text to carry structurally, and so keep the gate over
  every byte they cover: a **bare** auto-link, whose shown text is a slice of the target's own range
  (`https://example.org/*bold*x`), and the `<url>` form, whose whole interior the target is computed
  from — the half of the boundary [`build_angle_link_node`](../../parser/src/content/inline_builder/macros/links.rs)
  has always kept, now the only one of the two halves left. What defers beyond them is what the two
  predecessor increments predicted: a **target** crossing an opaque piece, and a display text
  carrying its own **attribute list** (`https://example.org[a *b* c,role=hl]`), whose value comes
  back from an `Attrlist` parse of the source's bytes, where a placeholder inside a *parsed* value
  has no node to be mapped back to. Each gets its own divergence test; the audit's own real-corpus
  instance of the second (`https://chat.asciidoc.org[*project chat*^,role=green]`) survives with it.

  The recognition-agreement gap this class introduces takes two of the three shapes it took for the
  `link:`/`mailto:` macro, read through this pattern's own captures — a `]` inside the span (which
  ends `INLINE_LINK`'s lazy text capture early for the string replacer), and markup carrying an `=`
  beside a comma elsewhere in the text (the replacer's attribute-list probe, fired on markup this
  side never sees). There is no third: `INLINE_LINK` has no `mailto:` spelling with a comma probe of
  its own. Each is pinned by its own test, which asserts *both* that the two pipelines disagree and
  that the tree still builds the well-formed node.

  A differential corpus pins the display text crossing every opaque shape the earlier steps can
  produce — each quoted form (including an attributed span, whose markup carries the `=` the string
  replacer's own probe reads and this one does not), an image, an icon, an index term, a character
  replacement, a UI macro (under `experimental`, the gate the string step and the builder share), a
  span beside an escaped special and beside a restored entity, a span beside the target's own
  escaped special, under the `\]` unescape and the `^` window suffix, escaped, in flow, beside a
  sibling link of the other spelling, and inside a rendered span of its own — in the plain spelling
  and in the ANGLE branch's `[…]` alternative, alongside a structural test asserting the recovered
  children's own precise spans. As before, the passthrough fixtures live in the whole-pipeline sweep
  rather than the family's own step-driven corpus, since only the real `SubstitutionGroup::apply`
  brackets the steps with the extraction that makes a passthrough opaque on *both* sides. Fixtures
  are added to that sweep, the broad general-purpose sweep, the group-parity corpus, and the
  structural recorder cross-check.

  Re-running the corpus-wide fold-parity audit confirms the divergence set strictly **shrank**:
  **seven** previously-surviving sources are gone — among them three real golden-corpus fixtures
  (`Ask questions in the https://chat.asciidoc.org[*community chat*].`,
  `http://example.org/community/team.html[Ze_**Project** team]`, and
  `I advise you to https://google.com[Google for +\+]`), the image-nesting
  `See https://example.org[the image:logo.png[Logo] here].`, and a multi-line attributed span
  (`https://example.com[[.role]#Foo\nBar#]`) — and no new one appeared. As with every prep piece
  before it, nothing further is wired in. A display or reference text crossing a rendered span is
  now closed as a whole class: what still defers is one **computed** capture per family — a target,
  and the image family's own attribute list — plus the audit's one structurally different item (an
  effective order that runs `SpecialCharacters` *after* a step that already produced markup,
  `subs=quotes,specialcharacters`), which still needs its own policy rather than another recognition
  increment.

  *Step 6 prep landed as (the UI family crossing an escaped special or a restored entity — the
  last family holding a gate of its own, and a harness hazard it uncovered in the footnote
  family):* every prior increment in this sequence names the **UI** family
  ([`Ui`](../../parser/src/inlines/ui.rs): `kbd:`/`btn:`/`menu:`) and **a footnote's text** as the
  two that keep a stricter gate than
  [`range_has_no_opaque_piece`](../../parser/src/content/inline_builder/macros/image.rs) "for a
  restored entity exactly as for an escaped special", pinned side by side so the pair moves
  together. Auditing that pairing found the two are not alike, and neither needed what the note
  assumed.

  The **UI family** takes the same one-gate-per-family swap its five predecessors made, and needs
  nothing else: every value a `Ui` node holds — a keyboard macro's split keys, a button's
  normalized label, a menu's name, submenu path, and item — is text the string replacer itself
  computes from its own **escaped haystack**, which `render_keyboard`/`render_button`/`render_menu`
  then emit *verbatim*. The node therefore holds it in that same already-substituted form (the
  contract an [`IndexTerm`](../../parser/src/inlines/index_term.rs)'s `terms` already uses, now
  recorded on the node's own docs) and reads it straight out of the level's **match string**, whose
  bytes at either [`CharRef`](../../parser/src/inlines/char_ref.rs) leaf are exactly the ones that
  haystack carries. So `kbd:[Ctrl&C]`, `btn:[Save &copy; Close]`, `menu:F&le[Save]`,
  `menu:File[Save & Exit]`, and — a real golden fixture — `menu:&#8942;[More Tools, Extensions]`
  are now recognized, folding byte-for-byte identically. The menu's own bespoke gate
  (`menu_match_is_sliceable`, which admitted one atomic piece and one only: a `&gt;` submenu caret
  *inside the item list*) is **deleted** rather than relaxed — that case is now just an instance of
  the general one — and with it the family's `Option`-returning "this increment defers" machinery,
  leaving [`build_menu_node`](../../parser/src/content/inline_builder/macros/ui.rs) total, exactly
  as the `<<id,>>` and bare-e-mail increments retired their own families'. The one value the menu
  used to slice from `'src`, its **name**, still borrows through
  [`text_slice`](../../parser/src/content/inline_builder/quotes.rs) whenever it can and falls back
  to the match string only when it crosses a leaf `text_slice` cannot slice. As in the
  `footnoteref:`, menu, cross-reference, link, and image increments, the keyboard/button pass's own
  escape check is **hoisted ahead of the gate**, closing the same latent gap: an escaped
  `\kbd:[*a*]` whose match the gate rejects now still drops its backslash.

  The **footnote text** needed no code change at all — it was already at parity, and had been since
  the family landed: a footnote's content is *structured children*
  ([`emit_range`](../../parser/src/content/inline_builder/quotes.rs) keeps a `CharRef` leaf as its
  own child), so the family never sliced `'src` for that value and never had a gate to relax. What
  made it look like a boundary was the **test harness**: the divergence test that pinned it drove
  the golden pipeline and the builder from *one shared `Parser`*, so each fixture's footnote was
  numbered twice (`1` on the golden side, `2` on the built side) and the two sides "diverged" for a
  reason that had nothing to do with the entity. That is precisely the hazard the
  two-independent-parsers discipline exists for (established by the footnote increment itself, and
  recorded again when the link family's own registration corpus was found using `{sp}` as an inert
  separator that stopped being inert): the test is rewritten to configure one parser per side, and
  both families' fixtures move into it as a **parity** corpus, per the "if lifted, fold this into a
  parity corpus" convention.

  With this, the escaped-special / restored-entity boundary is closed for **every** macro family:
  what keeps [`range_is_verbatim`](../../parser/src/content/inline_builder/macros/image.rs) is only
  the three attribute-list captures that must ride on their node as a real
  [`Attrlist`](../../parser/src/attributes/attrlist.rs)`<'src>`. The UI family's remaining boundary
  is the one every family keeps — a match crossing an **opaque** piece (a rendered span, an
  already-recognized macro node, a character replacement), whose markup exists only at fold time —
  now pinned by its own divergence test in place of the two this increment retires. A differential
  corpus pins each family's keys, label, name, and item list crossing each escaped special and each
  spelling of a restored entity, both at once, in flow, doubled, beside a sibling family, inside a
  rendered span, escaped, and beside — rather than crossing — a special; structural tests pin the
  already-substituted values and the node's still-precise source `location`; fixtures are added to
  the whole-pipeline combined-constructs corpus, the group-parity corpus (an order that never
  escapes, where the author's own `&` survives into a verbatim run), the synthesized-seed sweep, and
  the structural recorder cross-check — where the two constructions reach the same values from
  opposite directions, the recorder recovering them from the string pipeline's own render params.
  A whole-document test drives the real parse path end to end. Re-running the corpus-wide
  fold-parity audit (tree building forced on for every parse in the suite) confirms the divergence
  set strictly **shrank**: ten previously-surviving sources are gone — among them the two real
  golden fixtures `menu:&#8942;[More Tools, Extensions]` and `menu:File[Save As&#8230;]`, whose
  name and item carry a restored entity — and no new one appeared. As with every prep piece before
  it, nothing further is wired in.

  *Step 6 prep landed as (`AttributeReferences` under an order that escapes **after** expanding,
  closing the last category the audit map named):* the corpus-wide audit that surfaced the
  `hardbreaks` and unescaped-specials blockers left step 6 an itemized list of divergences to close,
  and named one category as not previously seen: "an effective order that runs `SpecialCharacters`
  **after** a step that already produced markup (`subs=quotes, specialcharacters`), where the string
  pipeline escapes the very tags the earlier step emitted". Re-running that audit now that every
  itemized *normal*-order divergence is closed leaves exactly two sources in it, and they turn out to
  be two different problems wearing one label. This closes the first, and pins the second.

  The first is **not** about markup at all: it is `AttributeReferences`, whose spliced value is
  ordinary *text* at the moment the step runs. [`split_attribute_value`](../../parser/src/content/inline_builder/attribute_refs.rs)
  has always classified a literal `<`/`>`/`&` in a resolved value as a
  [`Raw`](../../parser/src/inlines/inline_node.rs) leaf, on the stated ground that
  "`apply_special_characters` has already run and will not run again over spliced-in content". That
  ground holds for every *built-in* group that runs both steps — `Normal`, `Title`, `Header`, and
  `AttributeEntryValue` all escape first and expand later — but a `subs=` list can reverse the two,
  and **`subs=attributes+`**, which *prepends* the step, is the documented AsciiDoc idiom for
  inspecting what a `pass:quotes[…]` attribute entry actually stores. So `MyApp<sup>2</sup>` renders
  as `MyApp&lt;sup&gt;2&lt;/sup&gt;` — a wrong answer for content a golden test already exercises,
  making this a **blocker** like `hardbreaks`, the unescaped-specials classification, the
  `attribute-missing` drop modes, and `<<id,>>`'s present-but-empty text before it, not an unclaimed
  form.

  The fix is §3.4.1 read the same way that policy has been read at every other seam — "the kind a
  fragment becomes is decided by which substitution steps still act on it under the group's effective
  order" — applied here to one question: does a `SpecialCharacters` step still run *after* this one?
  A new [`SplicedSpecials`](../../parser/src/content/inline_builder/attribute_refs.rs) carries the
  answer, decided in [`build_for_group`](../../parser/src/content/inline_builder/mod.rs) where the
  order is in hand and threaded down to the classifier. Under `Verbatim` (every built-in order, and
  every order that never escapes at all) nothing changes. Under `EscapedLater` the value is spliced
  as one ordinary [`Text`](../../parser/src/inlines/inline_node.rs) run and **left unsplit**, for
  [`apply_special_characters`](../../parser/src/content/inline_builder/special_chars.rs) to split at
  its own position in the order — the same "classify where the step actually sits" discipline
  [`classify_unescaped_specials`](../../parser/src/content/inline_builder/special_chars.rs) follows by
  running last. Deferring the split there rather than doing it here is also what keeps the
  *intervening* steps faithful: a `Raw` leaf is opaque to
  [`build_match_string`](../../parser/src/content/inline_builder/quotes.rs) where a `Text` run's bytes
  are not, so a macro reading across a spliced `&` (`link:{host}/x[go]` with `host` set to
  `a&b.example.org`) is now recognized exactly as the string pipeline's own macros pass recognizes it
  over the already-escaped text. The value's spliced nodes keep the §4.4 coarse fallback span they
  always had.

  The second source is the one the map's own example names, and it stays deferred with a divergence
  test of its own: `subs=quotes,specialcharacters`, where the quotes step's `<strong>` tags are what
  the escaping step acts on. That is structurally different — a tree's markup exists only at fold
  time, so there is nothing for the escaping transducer to act on — and settling it means deciding
  what the tree should even hold (a `Styled` node whose fold is escaped, or pre-rendered text that
  abandons the structure), which is a policy question rather than another classification. It is
  reachable both as a block's own `subs=` list and, nested, through a `pass:q,c[…]` passthrough,
  and both spellings are pinned.

  A differential corpus crosses specials-bearing fixtures — the stored value alone and in flow, an
  author's own special beside a spliced one, adjacent splices, a value-less `Set` attribute that
  splices nothing, an escaped and a missing reference, a `counter` directive, a multi-line seed, and
  a macro recognized *across* a spliced special — with every real group that takes this path:
  `attributes+` over each of the two base groups it is written on (a paragraph's `Normal` and a
  listing block's `Verbatim`) and the explicit `subs=` lists naming the two steps in that order.
  Structural tests pin the leaf kinds from both directions (`CharRef` under the reversed order,
  `Raw` under the built-in one, the same coarse span either way), and a whole-document test drives
  the AsciiDoc docs' own `subs=attributes+` listing block end to end through the real parse path.
  Re-running the corpus-wide fold-parity audit confirms the divergence set strictly **shrank**: the
  real golden fixture `{app-name}` under `subs=attributes+` is gone, and no new divergence appeared.
  As with every prep piece before it, nothing further is wired in.

  *Step 6 prep landed as (a **footnote** inside an expanded attribute value — the last family to
  make that lift, and the only one that was building a **wrong node** rather than deferring):* the
  "a macro inside an expanded attribute value" map item was closed family by family — anchors,
  the bare e-mail address, the UI and index-term families, cross-references, and finally images and
  links — but the list those increments worked through was the list of families that *defer* inside
  a [`synthesized`](../../parser/src/content/inline_builder/quotes.rs) run, and the **footnote**
  family was never on it. It does not defer: it has no gate at all, so it recognized such a macro
  all along and read its **id** through
  [`source_slice`](../../parser/src/content/inline_builder/quotes.rs), whose only answer for a
  synthesized range is §4.4's coarse enclosing span. For the AsciiDoc docs' own
  *externalized-footnote* idiom (`:fn-disclaimer: footnote:disclaimer[…]`, then `{fn-disclaimer}`)
  that span is the attribute **reference**, so the node's id came out as `{fn-disclaimer}` — a wrong
  node, of exactly the kind #1177 fixed for an anchor's own id, and a *worse* one here because a
  footnote's id is the key the family's one required side effect registers and looks a number up
  by: every later reference to the real id renumbered, and the rendered
  `id="_footnote_{fn-disclaimer}"` attribute carried the reference too. Three real golden fixtures
  exercised it (`externalized_footnote` and `externalized_footnote_with_text_formatting` in
  `tests/asciidoc_lang/macros/footnote.rs`, and
  `should_not_register_footnote_with_id_and_text_if_id_already_registered` in
  `tests/asciidoctor_rb/substitutions_test.rs`), which makes this a **blocker** for the
  authoritative fold like `hardbreaks`, the unescaped-specials classification, the
  `attribute-missing` drop modes, `<<id,>>`'s present-but-empty text, and an order that escapes
  after expanding before it — not an unclaimed form.

  The fix is the one every sibling family already made, reached through the value rather than
  through a gate: a new
  [`footnote_id_text`](../../parser/src/content/inline_builder/footnotes.rs) recovers the id with
  [`text_slice`](../../parser/src/content/inline_builder/quotes.rs) — borrowing `'src` for a
  verbatim id (§4.5), the expansion's own exact bytes for a synthesized one — falling back to the
  level's **match string** where `text_slice` declines, which is what the string replacer itself
  reads (`caps[2]`, or the first half of a `footnoteref:` bracket, out of its own escaped haystack)
  and precisely the string it registers under. Only the node's `location` keeps §4.4's coarse
  fallback. The `footnote:` form needs **no gate** for this, on the same structural argument the
  bare-e-mail family makes for its address: its id is `[\w-]+`, which admits neither an entity's
  `&`/`;` nor the `SPAN_PLACEHOLDER` an opaque piece contributes (category `Co`, which `\w` does
  not match), so such a range can only ever overlap `Text` pieces. The deprecated `footnoteref:`
  form's id is *whatever precedes the first comma* in an arbitrary bracket, so it can cross both
  recoverable pieces — where the match-string fallback gives it exactly the already-substituted id
  the replacer splits out (`footnoteref:[a&b,…]` registers `a&amp;b`) — and an **opaque** one,
  which is the one shape it now rejects with
  [`range_has_no_opaque_piece`](../../parser/src/content/inline_builder/macros/image.rs), leaving
  the macro literal rather than building an id no pipeline would produce (its own divergence test).

  A differential corpus drives the expanded-value forms — the whole macro arriving from an
  expansion (defining, then referencing, and the anonymous spelling), an id expanded whole or in
  part, the deprecated form's own id half, and content from an expansion — through the real, public
  `SubstitutionGroup::Normal::apply` as the golden, since these need the `AttributeReferences` step
  the family-scoped `golden_macros` helper deliberately omits, each fixture with its own pair of
  independent parsers (the two-independent-parsers discipline this family established). Structural
  tests pin the exact-id/coarse-location split and the match-string-read `footnoteref:` id; the
  wholly-synthesized **seed** path
  ([`build_from_value`](../../parser/src/content/inline_builder/mod.rs), a filtered multi-line
  block) gains an id-carrying footnote, which used to take the whole seed as its id; a
  whole-document test drives the externalized-footnote shape end to end through the real parse
  path; and fixtures are added to the whole-pipeline combined-constructs corpus, the
  synthesized-seed sweep, and the structural recorder cross-check — where the recorder side, which
  has always recovered these from the string pipeline's own render params, can now be compared
  *structurally* against a builder that reads the same id. Re-running the corpus-wide fold-parity
  audit (tree building forced on for every parse in the suite) confirms the divergence set strictly
  **shrank**: the three real golden sources above are gone and no new one appeared. What survives it
  is now only the three `Attrlist<'src>` captures and the `subs=quotes,specialcharacters` policy
  item. As with every prep piece before it, nothing further is wired in.

  *Step 6 prep landed as (an **image's attribute list** with no `'src` slice — the first of the
  three `Attrlist<'src>` captures):* with every family's own gate now the shared opaque-piece one,
  what the audit still names beside the `subs=quotes,specialcharacters` policy item is the
  **three attribute-list captures** — an image's non-empty bracket, and the `link:`/`mailto:` and
  auto-link families' attribute-list-bearing display texts — each of which every previous increment
  held back for the same structural reason: [`Attrlist::parse`](../../parser/src/attributes/attrlist.rs)
  reads its `source: Span<'src>`'s bytes **as content**, not merely as a location tag, so a capture
  with no honest `'src` slice had nothing to parse from. That reason turns out to be about
  *parsing*, not about *holding*: an [`Attrlist`](../../parser/src/attributes/attrlist.rs) keeps
  nothing `Span`-typed but its own location — its
  [`ElementAttribute`](../../parser/src/attributes/element_attribute.rs)s are
  [`CowStr`](../../parser/src/strings.rs)s and everything else is plain data — so a list parsed from
  a temporary can be *kept* if its strings are detached from it.

  A new [`Attrlist::into_owned`](../../parser/src/attributes/attrlist.rs) is that detachment (built
  on [`ElementAttribute::into_owned`](../../parser/src/attributes/element_attribute.rs) and
  [`CowStr::into_owned`](../../parser/src/strings.rs), which prefers the inline representation for a
  short string exactly as `Clone` does): it rebuilds the list with every borrowed string copied and
  a caller-supplied span as its new location tag. The image family is the first capture to use it.
  [`bracket_attrlist`](../../parser/src/content/inline_builder/macros/image.rs) keeps the `'src`
  slice — and so the borrow (§4.5) — for a verbatim bracket, and for every other one parses the
  **match string's** own bytes instead, which is not a substitute for the source but the *exact*
  thing `InlineImageMacroReplacer` parses (`Attrlist::parse(Span::new(&caps[2]), …)`, over its own
  escaped, already-expanded haystack), then owns the result onto the bracket's coarse source span —
  design §4.4's fallback, the same one the node's `location` already takes. So
  `image:sunset.jpg[{caption}]`, `image:tiger.svg[A <b> & "c",opts=interactive]`, and
  `image:x.png[Tom &amp; Jerry]` are now recognized, the family's remaining `range_is_verbatim`
  call disappears with them, and `build_image_node` becomes **total** — the whole match keeping
  only [`range_has_no_opaque_piece`](../../parser/src/content/inline_builder/macros/image.rs), the
  boundary every family keeps, since a rendered span's markup exists only at fold time and the
  string replacer parses that markup where this would parse one placeholder.

  Nothing about the family's staged side effects changes: `register_image` reads the node's
  `target`, and the `link=` dangerous-scheme warning reads the very attribute list this increment
  makes available, so a `link=` in a bracket with no `'src` slice now records its warning too (its
  own test). The three `…_is_a_documented_divergence` tests this closes — for an escaped special, a
  restored entity, and an expanded value — become parity corpora per the "if lifted, fold this into
  a parity corpus" convention, joined by structural companions pinning the owned values and the
  coarse location tag; `attribute_refs.rs`'s own "an attrlist-bearing macro inside a spliced value"
  divergence is re-pointed at the link family, which still holds it, with the image half rewritten
  as a recognition test. Fixtures are added to the whole-pipeline combined-constructs corpus, the
  synthesized-seed sweep (where an image's *non-empty* bracket now joins the empty one), the
  group-parity corpus, and the structural recorder cross-check — comparable there for the first
  time, since both constructions now read the same `caps[2]`. Re-running the corpus-wide
  fold-parity audit confirms the divergence set strictly **shrank**: seven sources are gone, four of
  them real golden ones (`image:tiger.svg[…]`, `image:missing.svg[…]`, and two
  `image:pause.png[title=…]` forms), and no new one appeared. What survives is the two remaining
  `Attrlist<'src>` captures, the `subs=quotes,specialcharacters` policy item, and the opaque-piece
  boundary itself. As with every prep piece before it, nothing further is wired in.

  *Step 6 prep landed as (the two **link-family display texts**, closing the `Attrlist<'src>`
  captures — and with them the last boundary drawn around a *capture*):* what the image increment
  left were the other two of the audit's three attribute-list captures — the `link:`/`mailto:`
  macro's bracketed display text and the auto-link / formal-URL family's — each of which the two
  passes had been gating with the same hand-rolled pair of checks (`range_is_verbatim`, then "and
  no embedded newline"), deferring an attribute-list text that crossed an escaped special, a
  restored entity, or an expanded value, and one that simply spanned two lines. All four fall to
  the image family's own move, expressed once as a shared
  [`text_attrlist`](../../parser/src/content/inline_builder/macros/links.rs) that both passes — and
  all three of their call sites (a `link:` `=`, a `mailto:` `,`, and a formal URL's `=`) — now go
  through.

  What that helper parses is the string replacers' own input: `link_text.replace('\n', " ")`, the
  newline-normalized copy of the *pre*-`\]`-unescape bracketed text. When the text's own `'src`
  slice **is** those bytes it is parsed from the source and keeps its borrow (§4.5); otherwise the
  copy itself is parsed and the result is
  [`into_owned`](../../parser/src/attributes/attrlist.rs)ed onto design §4.4's coarse span, exactly
  as [`bracket_attrlist`](../../parser/src/content/inline_builder/macros/image.rs) does. Both
  halves of that test are load bearing, and neither is quite the `range_is_verbatim` check it
  replaces. The range must be verbatim because bytes can coincide without the text being the
  source's: [`build_match_string`](../../parser/src/content/inline_builder/quotes.rs) gives a
  *restored* entity leaf its own bytes as written, so `link:x[a &copy; b,role=hl]` reads identically
  either way while its parsed value is escaped text. And the bytes must be compared even for a
  verbatim range, because such a range need not be *contiguous* in the source — the
  attribute-references step drops an escaped reference's backslash as a gap
  (`link:x[\{name},role=hl]`), so the enclosing slice carries a byte the replacer's text does not.
  That second half closes a latent gap the verbatim path had carried since the family's
  attribute-list form first landed.

  The positional value a *match-string* parse returns is already-escaped text, where a node holds
  logical text — the same mismatch the cross-reference family met when its own attribute-list value
  stopped being verbatim. So it is rebuilt through that family's
  [`escaped_value_children`](../../parser/src/content/inline_builder/macros/mod.rs), moved out of
  `macros/xref.rs` and shared next to
  [`macro_text_children`](../../parser/src/content/inline_builder/macros/mod.rs) so the three
  families cannot drift: an escaped special becomes the character it stands for (inside a `Text`
  the fold escapes back) and a restored entity its own `CharRef::Entity` leaf (which the fold emits
  verbatim). A value parsed from a *verbatim* slice is logical text already and stays one
  synthesized `Text`, as before.

  The one shape still deferred is the **opaque-piece** boundary every family keeps, applied here to
  the text rather than to the match: a display text carrying an attribute list *and* crossing a
  rendered span stays literal, because a placeholder inside a **parsed** value has no node it can
  be mapped back to — which is why this capture keeps a gate at all while the same family's
  attribute-list-free display text (carried structurally, never read as bytes) does not. Sharing
  `escaped_value_children` also surfaced, and now pins, a narrow divergence it has always carried
  for *every* family that computes an attribute-list value: under an effective order that never
  escapes, a non-verbatim text carrying an author's own literal `&lt;` is unwound one level too
  far, since an attribute list is parsed under any order that runs `Macros` while §3.4.1's own
  answer needs the effective order threaded down to each family (its own divergence test, for all
  three spellings).

  Six `…_is_a_documented_divergence` tests become parity corpora per the "if lifted, fold this into
  a parity corpus" convention — an attribute-list text over an escaped special, over a restored
  entity, over a multi-line text, and inside an expanded value, in each family's spelling — joined
  by structural companions pinning the rebuilt entity leaf, the owned values, and the coarse
  location tag; `attribute_refs.rs`'s own "an attrlist-bearing display text inside a spliced value"
  divergence, re-pointed at this family by the image increment, becomes a recognition test. Fixtures
  are added to the whole-pipeline combined-constructs corpus, the synthesized-seed sweep, the
  group-parity corpus, and the structural recorder cross-check — comparable there for the first
  time, since both constructions now read the same `link_text_for_attrlist`. Re-running the
  corpus-wide fold-parity audit confirms the divergence set strictly **shrank**: two real golden
  sources are gone (`https://example.com[Foo\nBar,role=foobar]` and
  `https://example.com[What You Need\n= What You Get]`) and no new one appeared. What survives is
  the `subs=quotes,specialcharacters` policy item and the opaque-piece boundary itself. As with
  every prep piece before it, nothing further is wired in.

  *Step 6 prep landed as (a **typographic replacement** as the third — and last — recoverable
  piece):* the two increments that named the recoverable classes
  — an escaped special, then a restored entity — both stopped short of the third leaf
  [`CharacterReplacements`](../../parser/src/content/inline_builder/char_replacements.rs) produces:
  a [`CharRef`](../../parser/src/inlines/char_ref.rs)`::Replacement` (`(C)` → ©, `'` → ’, `--`,
  `...`, the arrows). Until now
  [`build_match_string`](../../parser/src/content/inline_builder/quotes.rs) stood one in as a single
  opaque `SPAN_PLACEHOLDER`, so every macro family read across it as if it were a rendered span —
  and a fresh corpus-wide audit showed exactly what that cost, in three *real* golden fixtures
  rather than constructed ones: `image:pause.png[title=Pause (C) Resume]`,
  `image:tiger-roar.png[A tiger's "roar" is < a bear's "growl"]` (an apostrophe *and* an escaped
  special in one bracket), and `<<Cub => Tiger>>`, whose `=>` is an arrow by macro time.

  It is closed the way the restored-entity increment was — once, for **every** family, since
  recoverability is a property of the piece rather than of the family reading across it. A
  `CharRef::Replacement` leaf now contributes the entity the **built-in** backend renders it as
  ([`replacement_entity`](../../parser/src/content/inline_builder/quotes.rs): `&#169;` for `(C)`,
  `&#8217;` for `'`), which is precisely what the string pipeline's own haystack holds at that
  position from the replacements step onward; the leaf stays `atomic` (one indivisible node, never
  sliced) but becomes *recoverable*, and
  [`range_has_no_opaque_piece`](../../parser/src/content/inline_builder/macros/image.rs) admits it,
  its recoverability test mirroring the new arm's own guard so a hand-built node carrying a value no
  rule produces stays opaque on both sides. Nothing else changed: no builder, no family gate, no
  node kind, and no side-effect pass — a target or computed value simply reads the replacement's
  bytes off the match string, and a display text keeps the leaf as its own child through
  `macro_text_children`'s `emit_range` path, folding back through the renderer to the same bytes.

  One thing does separate this class from a restored entity, and it is the reason the increment is
  its own step rather than a line in the last one: the fold routes a replacement **through the
  renderer** (`render_character_replacement`), where an entity leaf is emitted verbatim. Using the
  canonical rendering here is therefore the same deliberate compromise
  [`special_entity`](../../parser/src/content/inline_builder/quotes.rs) has always made for an
  escaped special — a custom backend changes what the fold *emits*, not the recognition the AsciiDoc
  patterns were written against — and `replacement_entity_matches_the_built_in_renderer` pins the
  table against `HtmlSubstitutionRenderer` so the two cannot drift. The replacements step's own rule
  loop gains the same fidelity as a side effect: a later rule now matches over an earlier one's
  emitted bytes exactly as the string pipeline's sequential passes do, instead of over a placeholder.

  Three `…_is_a_documented_divergence` tests shed fixtures per the "if lifted, fold this into a
  parity corpus" convention — the bibliography label over `(C)`/`'` (now its own parity test,
  asserting the *registered* reference text as well as the fold), the UI family's opaque-piece test,
  and the e-mail family's abutting test, where an address after a replacement (`a(C)b@example.com`,
  whose `;` is no mismatch character) now links exactly as the string pipeline links it. The
  footnote-mirror skip test, which had reached for a replacement as its deferred form, is re-pointed
  at a masked passthrough. Differential corpora gain, per family, a target/id and a display or
  reference text crossing a replacement, an image's and a link's **attribute list** crossing one,
  a match crossing every recoverable class at once, doubled, in flow, inside a rendered span, beside
  a sibling family, escaped, and beside — rather than crossing — a replacement; plus structural
  companions pinning the recovered `CharRef::Replacement` child and the owned, coarsely-located
  attribute list a match-string parse yields. Fixtures are added to the whole-pipeline broad sweep
  and combined-constructs corpus, the synthesized-seed sweep, the attribute-reference corpus (a
  replacement that exists only because an attribute expanded, read across by a macro), and the
  structural recorder cross-check.

  Re-running the corpus-wide fold-parity audit confirms the divergence set strictly **shrank**: the
  three real golden sources above are gone (its unique set falls from 45 to 42) and no new one
  appeared. The categories that survive are unchanged from the last increment — the
  `subs=quotes,specialcharacters` policy item, the forms individual families already document as
  deferred, and the opaque-piece boundary itself, which is now purely a **rendered-markup**
  boundary: every piece it still rejects is one whose bytes exist only at fold time. As with every
  prep piece before it, nothing further is wired in.

  *Step 6 prep landed as (a footnote's own escaped closing bracket):* with the opaque-piece
  boundary reduced to fold-time-only markup, the next audit item is not a *piece* class at all but
  one family's own last deferred **form**: a footnote whose bracket content carries an escaped
  closing bracket (`footnote:[a note ending in a\]bracket]`). The string replacer unescapes it to a
  literal `]` ([`normalize_footnote_text`](../../parser/src/content/macros.rs)); the builder left the
  whole macro unrecognized, its own note reasoning that "unescaping it would mean splicing a literal
  `]` into the middle of a `Text` piece the content range slices, which … would require rebuilding
  part of the content's own node structure around the splice". A **blocker**, like the six audit
  items before it, since four real golden sources exercise the form — including
  `text footnote:[a [[b]] [[c\]\] d]` and an id-carrying definition whose two later
  `footnote:id[]` references inherit its number, so failing to recognize the first left every one of
  them unresolved.

  What closes it is that the rebuild the note calls for was *already written*, for a different
  family: the reference-bearing families' own
  [`macro_text_children`](../../parser/src/content/inline_builder/macros/mod.rs) has expressed the
  identical unescape as a **gap in the emitted ranges** — every byte but the backslash is emitted —
  since the increment that gave a display text structured children. That loop is lifted out as the
  shared [`emit_range_unescaping_brackets`](../../parser/src/content/inline_builder/macros/mod.rs)
  and [`footnote_children`](../../parser/src/content/inline_builder/footnotes.rs) emits through it,
  so the two families cannot drift on which backslashes pair off (`match_indices` scans
  non-overlapping and left to right, exactly as the replacer's `replace` does). `footnote:[a \] b]`
  becomes the two `'src`-borrowing `Text` children `a ` and `] b` — a gap, never an owned rebuild —
  and the three `\]` gates (both `footnote:` branches and the `footnoteref:` form's whole bracket)
  are deleted rather than relaxed. The catalog `text` this pass registers already went through
  `normalize_footnote_text`, so it agreed with the string pipeline all along; a test now pins that
  the subtree agrees with it. The one asymmetry is the `footnoteref:` form's **id** half, which the
  string replacer takes from the raw bracket's first-comma split and never normalizes: an id
  carrying a `\]` keeps its backslash on both sides, pinned by its own parity test.

  Two `…_is_a_documented_divergence` tests shed their fixtures per the "if lifted, fold this into a
  parity corpus" convention, becoming parity-plus-structure tests for both spellings. The footnote
  differential corpus gains the escape in every position (interior, leading, trailing), doubled,
  twice in one content, beside a construct the content captures as a node, beside an escaped
  special, in a definition whose number a later reference reuses, in a pair whose second footnote's
  number depends on the first being recognized, and under the macro's *own* escape; fixtures are
  added to the whole-pipeline broad sweep, the combined-constructs corpus, and the structural
  recorder cross-check. Re-running the corpus-wide fold-parity audit confirms the divergence set
  strictly **shrank** — the four real golden sources are gone and no new one appeared. As with every
  prep piece before it, nothing further is wired in.

  *Step 6 prep landed as (an order that escapes **after** a markup-producing step — the last
  category the audit's own map named):* the corpus-wide audit's itemized map ended with one
  category no recognition or classification increment could close: "an effective order that runs
  `SpecialCharacters` **after** a step that already produced markup (`subs=quotes,
  specialcharacters`), where the string pipeline escapes the very tags the earlier step emitted.
  That last one is structurally different from the rest, and harder: a tree whose markup exists
  only at fold time has no rendered tags for a later escaping step to act on, so it needs its own
  policy rather than another recognition increment." That policy is settled here, and it is a
  short one: **reach fold time for that one node, early.**

  [`flatten_prior_markup`](../../parser/src/content/inline_builder/special_chars.rs) runs
  immediately ahead of [`apply_special_characters`](../../parser/src/content/inline_builder/special_chars.rs)
  whenever the escaping step is not the order's first — never for a built-in group, every one of
  which escapes first, so this is a `subs=` list's question alone. Each node an earlier step of the
  same order already turned into markup is folded through the configured renderer and the result
  becomes one [`Text`](../../parser/src/inlines/inline_node.rs) node's value, which the escaping
  step then splits like any other text. Nothing new is invented for it: a `Text` node is *logical*
  text the fold escapes (§3.4), so the single escape lands exactly where the string pipeline's does,
  and "a node's value is already-substituted text" is the same seam a delimited passthrough's and a
  STEM expression's body already use — the only place this module consults the renderer while
  building. It is also what the document genuinely says under such an order: the content is no
  longer a strong span, it is text that reads like a tag, and the tree now holds it as such (pinned
  by a structural test, not only by the fold's bytes).

  The one subtlety is *which* bytes that early fold must produce. The string pipeline's haystack at
  that moment holds `<strong>a < b</strong>` — the tags written, the author's own specials **not
  yet escaped**, both escaped together by the one pass that follows. A finished fold would escape
  the children on the way out, so `as_pre_escape` rewrites every `Text` run nested *inside* the node
  as a [`Raw`](../../parser/src/inlines/inline_node.rs) leaf ("emit verbatim") first; every other
  leaf already carries its own already-substituted bytes (a `CharRef` an earlier `replacements` step
  built, a `LineBreak` `post_replacements` wrote) and needs no rewrite.

  What must **not** be folded is what the string pipeline is holding as a *placeholder* rather than
  as markup at that point, since no escaping step acts on those either: a passthrough body or inline
  STEM expression (extracted ahead of every step, restored after the last one) and a **deferred
  cross-reference** (recorded by the macros step as an `XrefSegment`, rendered by
  [`Content::finalize_deferred`](../../parser/src/content/content.rs) once every step has run —
  which is why `subs=macros,specialcharacters` emits an unescaped `<a href="#sec">` where it escapes
  a link's). A leaf says which it is by its own node kind; the one *container* the extraction pass
  builds — an attribute-list-prefixed passthrough's own `Styled` wrapper — is told from a
  quotes-step span by `masked_locations`, a set of identities collected from the tree extraction
  produced, before any step ran (sound because extraction recognizes only a wholly verbatim match,
  so each such node carries an honest `'src` span no later step's node can claim).

  Two shapes stay divergent, each pinned by its own test. A placeholder construct **nested inside**
  such a node (`*a +++<x>+++ b*`, `*see xref:sec[S] now*`, `link:index.html[a +++<b>+++ c]`) leaves
  the whole node unflattened: folding it would inline the placeholder's content into the escaped
  text, where the string pipeline escapes *around* the placeholder and restores it unescaped
  afterwards — and splitting one node's fold back around its placeholder descendants is the sentinel
  mechanism itself, which this module deliberately does not have. And a `link:`/`mailto:` **macro**
  written inside flattened markup (`*a link:index.html[Docs] b*` under
  `subs=quotes,specialcharacters,macros`) is a boundary this policy *inherits* rather than
  introduces: the flattened run is synthesized, and that one pass alone requires its own marker to be
  verbatim so [`link_form`](../../parser/src/content/inline_builder/macros/links.rs) can replay the
  string pipeline's family-pass registration order — the same boundary a wholly expanded `{m}` macro
  already has. Every other family reads what it needs from the level's match string and is recognized
  in flattened text exactly as the string pipeline recognizes it in the escaped tags (an auto-link, an
  image, an anchor, an index term, and a footnote are each pinned doing so).

  A differential corpus crosses markup-bearing fixtures with the real `subs=` lists that reverse the
  two steps: each markup-producing step ahead of the escaping one on its own (`quotes`,
  `replacements`, `macros`, `post_replacements`, `callouts`), several together, an expanding step
  ahead of both, a multi-line content, and the orders that additionally run a step *after* the
  escaping one — so the flattened text is what those later steps read, exactly as the string
  pipeline's later steps read the escaped tags. Re-running the corpus-wide fold-parity audit (tree
  building forced on for every parse in the suite) confirms the divergence set strictly **shrank**:
  the category's own source is gone and no pre-existing one appeared (the set's only additions are
  this increment's own new divergence-pinning fixtures). With it, every item the audit's map named
  is closed. As with every prep piece before it, nothing further is wired in: `rendered_html()`
  remains the string pipeline's own string.

  *Step 6 prep landed as (an **attributed span's** attribute list, read from the escaped text):*
  with the audit's map closed, re-running the corpus-wide fold-parity sweep turns up one more
  **blocker** of the kind the map's own items were — not an unclaimed form the tree leaves literal,
  but a *wrong* answer for content a golden test already exercises, and this one a **security**
  test: `['a<b&c']*bold*`, whose rendered `class` the string pipeline escapes
  (`class="'a&lt;b&amp;c'"`) and the fold did not. The cause is the one attribute list this module
  still parsed from `'src`. Every macro family's list is parsed out of the level's **match
  string** — an image's bracket, and both link families' display texts, by the two increments
  directly above — but the *quotes* step's own attributed span kept the source slice, and by the
  time that step runs `SpecialCharacters` has already escaped the author's `<`/`&`, so the
  string replacer's haystack (and the `class`/`id` value it renders verbatim) carries the entity
  where the source carries the character. Parsing the source slice put an **unescaped** `<`, `>`,
  or `&` into rendered markup — one entity's worth of the escaping the `"`-escaping beside it
  exists to provide.

  [`quote_attributes`](../../parser/src/content/inline_builder/quotes.rs) is the same one move
  those two increments made: a **verbatim** range's match-string bytes *are* its source bytes, so
  it keeps the `'src` parse and its borrow (§4.5) — every ordinary `[.role]#text#` — and every
  other range parses from a [`Span::new`](../../parser/src/span/primitives.rs) over the match-string
  slice and [`into_owned`](../../parser/src/attributes/attrlist.rs)s the result onto §4.4's coarse
  span. No node field changed: a `Styled` span already carried its `id`, `roles`, and own
  `Attrlist<'src>`; only the bytes they are read from did.

  Landing it closed a latent gap in
  [`Attrlist::into_owned`](../../parser/src/attributes/attrlist.rs)'s own stated contract — "every
  parsed field is a `CowStr`, so nothing but the location depends on the original span" — which one
  accessor falsifies: [`quoted_text_fallback_role`](../../parser/src/attributes/attrlist.rs) reads
  the list's own *text*, not a parsed attribute, because Asciidoctor's
  `parse_quoted_text_attributes` takes a quote-delimited first positional (`['role']`) verbatim,
  quote characters included, straight off the source. A re-tagged list would have read the coarse
  span's raw bytes — precisely the security fixture's own spelling. `into_owned` now carries an
  owned copy of the text it was parsed from and that accessor reads it, so the contract holds for
  every family that owns a list off a temporary, not just this one.

  Two shapes stay divergent, each pinned by its own test. An attribute list crossing an **opaque**
  piece (`[.a+++x+++b]#y#`) is the boundary every macro family draws, drawn here for the first
  time: the string pipeline parses its list with the passthrough's own placeholder inside it — one
  atomic character no comma can hide behind — and restores that passthrough's text into the
  rendered `class` afterwards, where splicing the text in at parse time would let a comma inside it
  split the list. Such a match is now dropped from the level's match list rather than built into a
  wrong node, leaving the construct literal and putting it exactly where a rejected look-ahead
  leaves one (the surrounding gap reproduces its original nodes, and a later sub may still match
  there). And an attribute list **rewritten by a later step** of the same order (`['{myrole}']`,
  `[.a(C)c]`, `[.a&amp;b]`) is `flatten_prior_markup`'s own category seen from the other side: under
  the *normal* order the steps after `quotes` go on matching over the whole rendered string, the
  markup that step just wrote included, so they rewrite bytes that live only inside a rendered
  attribute — which a tree, whose markup exists at fold time alone, has nowhere to hold. That one
  needs its own policy rather than another parse-source choice.

  The quotes differential corpus gains every attribute-list spelling carrying a special (a
  shorthand role, an id, a block-style positional, a `role=` value, a quoted positional, several at
  once, and the same on the unconstrained branch), alongside the `"`-escaping composing with it and
  the escaped-with-attributes form; the passthrough corpus gains the mirror-image fixtures that
  pin why *its* attrlist keeps the source parse (its extraction pass runs ahead of the escaping
  step, so the string pipeline reads the author's raw bytes there too); and the staged
  `apply_ref_side_effects` corpus gains an id-carrying fixture, since the id it registers is now
  the escaped one the golden pipeline registers. Re-running the corpus-wide fold-parity audit (tree
  building forced on for every parse in the suite) confirms the divergence set strictly **shrank**:
  the security fixture's own source is gone and no pre-existing one appeared (the set's only
  additions are this increment's own new divergence-pinning fixtures). As with every prep piece
  before it, nothing further is wired in.

  *Step 6 prep landed as (the boundary characters an enclosing span presents to a nested
  level):* re-running the corpus-wide fold-parity sweep after the increment above turns up one
  more **blocker** of the same kind — a *wrong* answer for content a golden test already
  exercises, rather than an unclaimed form: ``` `"``end points``"` ```, which the string
  pipeline renders ``` `&#8220;`end points`&#8221;` ``` (the inner backticks staying literal)
  where the fold wrapped them in a `<code>` span. Its cause is a category no increment had named,
  and this closes it.

  The string pipeline has no levels. A step matches over one flat string in which an earlier
  step's construct is already **rendered markup**, so a pattern's boundary classes read that
  markup's own characters — and this module's transducers match one *level* at a time, where the
  same position is the start or end of the haystack. The two agree for a construct written at the
  content's own top level and diverge for one written **inside** a span: the double-quote sub runs
  before the monospace one, so by the time monospace matches, the string pipeline's haystack holds
  `&#8220;` — whose `;` that sub's own `(^|[^\w&;:}])` boundary class excludes — where a level
  matched in isolation shows `^`. The same fact reaches the character-replacements step through
  the spaced em dash's `(^|\n| )--( |\n|$)`, at *either* edge of any span (`*x --*` renders
  `<strong>x --</strong>`, the `--` staying literal because `<` follows it there).

  A new [`LevelContext`](../../parser/src/content/inline_builder/quotes.rs) carries the pair of
  characters an enclosing construct's rendering presents — the last of its opening markup and the
  first of its closing markup — and a level is matched inside them: its own match string is
  wrapped in the two before the pattern runs, and every offset the pattern reports is mapped back
  with [`LevelContext::unshift`](../../parser/src/content/inline_builder/quotes.rs), clipped to the
  level. So only *recognition* changes; every range a caller goes on to slice stays in the level's
  own coordinates, no shared machinery
  ([`build_match_string`](../../parser/src/content/inline_builder/quotes.rs) and its 24 callers)
  changes signature, and the root level borrows its haystack untouched. A pattern may legitimately
  *consume* a context character — a constrained sub's boundary group is exactly that — and the
  clip drops it rather than emitting it, which is right because that group is text the sub keeps
  and the enclosing span already carries it.

  Which characters a span presents is decided by its own rendering shape, from the **built-in**
  backend: `>`/`<` for every variant that wraps its body in a tag, `;`/`&` for the two smart-quote
  variants (`&#8220;…&#8221;`), and — for the one variant whose shape depends on what its
  attribute list resolves to — by rendering it around a probe body. Reading the built-in backend
  here is the same deliberate compromise
  [`special_entity`](../../parser/src/content/inline_builder/quotes.rs) and
  [`replacement_entity`](../../parser/src/content/inline_builder/quotes.rs) already make (a custom
  backend changes what the fold *emits*, not the recognition the AsciiDoc patterns were written
  against), and `styled_boundaries_match_the_built_in_renderer` pins the shapes against the
  renderer itself so the two cannot drift. The
  [`post_replacements`](../../parser/src/content/inline_builder/post_replacements.rs) step had
  already reasoned this way for its own `$` — "a nested `Styled` or `Ref` level is always followed
  by its own closing markup … so a ` +` ending a span is not at a line end there" — with a
  boolean; this generalizes that one step's flag into a pair of characters every step's patterns
  can read, and the two steps whose patterns read one (`Quotes` and `CharacterReplacements`) now
  do.

  Three shapes stay divergent, each pinned by its own test. A construct written **beside** an
  entity-rendered span at its own level (``` `"`a`"`code` ```) reads the last character of that
  span's *closing* markup in the string pipeline's haystack, where `build_match_string` stands
  the whole span in as one `SPAN_PLACEHOLDER` belonging to no boundary class at all — the same
  boundary class the bare-e-mail family already documents from the other direction
  (`**bold**doc@example.org`); answering it means classifying the placeholder itself, which every
  family reading one would then see. The **macros** step's own families read a boundary character
  too (the auto-link's prefix group, the bare e-mail's mismatch-prefix one) and each finds its
  matches through its own `find_*_matches`, so giving them the context is a step-shaped increment
  of its own (`*doc@example.org*` links where the string pipeline, reading the `>` that ends
  `<strong>`, leaves the address literal). And a **transparent** span — an unquoted one whose
  attribute list resolves to neither a role nor an id, so it renders to its body and nothing else
  — has its children inherit the context it sits in, which is right whenever the span is all its
  parent's level holds and wrong when a sibling follows it (`*[width=10]#x --# --*`), since the
  haystack then shows what that sibling begins with; modelling that means deriving a level's
  context from its *siblings* rather than from its enclosing construct alone.

  The quotes and character-replacements differential corpora gain each construct written against
  an enclosing span's own edge in every variant's rendering shape, the same constructs away from
  either edge (where both pipelines match), the rules that read no boundary of their own, and the
  same fixtures at the content's own top level; fixtures are added to the whole-pipeline broad
  sweep and combined-constructs corpus, to the group-parity corpus (the boundary a span presents
  comes from its rendering, which no effective order changes), and to the structural recorder
  cross-check — where the recorder, recovering what actually rendered, reads the string pipeline's
  own answer. A whole-document test drives the golden fixture's own shape end to end through the
  real parse path. Re-running the corpus-wide fold-parity audit (tree building forced on for every
  parse in the suite) confirms the divergence set strictly **shrank**: the golden source above is
  gone and no pre-existing one appeared (the set's only additions are this increment's own new
  divergence-pinning fixtures). As with every prep piece before it, nothing further is wired in.

  *Step 6 prep landed as (the same boundary characters, for the macros step's own families):*
  the increment above closes the boundary question for the two steps whose *rules* read one, and
  names the third — "the **macros** step's own families read a boundary character too … and each
  finds its matches through its own `find_*_matches`, so giving them the context is a step-shaped
  increment of its own" — as its own next piece. This is that piece.

  Two of the step's families read the character immediately before a match: the auto-link's
  boundary-prefix group (`( ^ | [\ \t\p{Zs}] | [>\(\)\[\];"'] )`) and the bare e-mail's
  "prefix that causes a mismatch" one (`([\\>:/]?)`). Inside a span the string pipeline reads that
  span's own rendered markup there, so `*doc@example.org*` keeps the address literal — `<strong>`
  ends in `>`, one of the e-mail pattern's three mismatch characters — where a level matched in
  isolation shows a start anchor and links it. That was the divergence the increment above pinned
  with a test of its own; it is now parity, and its fixtures are folded into the corpora exactly as
  that test's own note asked.

  [`apply_macro_families`](../../parser/src/content/inline_builder/macros/mod.rs) therefore carries
  the [`LevelContext`](../../parser/src/content/inline_builder/quotes.rs) down the same recursion
  the two other steps take (a span's own rendering for its children, `INSIDE_REF` for a reference's,
  the enclosing context inherited through a transparent span) and hands it to every family. What
  differs is how a family *uses* it. `Quotes` and `CharacterReplacements` map each reported offset
  back with [`unshift`](../../parser/src/content/inline_builder/quotes.rs); a macro family cannot,
  because it does not merely report ranges — it reads the match string's own bytes through the
  level's [`Piece`](../../parser/src/content/inline_builder/quotes.rs)s (`emit_range`,
  `source_slice`, each of the three range gates), so haystack offsets and level offsets must not
  coexist. A new [`LevelContext::shift`](../../parser/src/content/inline_builder/quotes.rs) removes
  the second coordinate system instead of translating between them: it wraps the level's match
  string and moves its pieces into the wrapped string's own coordinates, so nothing else in the
  module changes signature and every gate and slice goes on reading one system. The context
  characters belong to no piece — they are the *enclosing* construct's — so a range reaching one
  contributes nothing, which is exactly what `unshift`'s own clip does (a boundary prefix is text
  the enclosing span already carries).

  Only the **opening** character is applied, and the asymmetry is the point. A *boundary* class
  reads exactly one character, so one character answers it: `<strong>` ends in `>` and `&#8220;` in
  `;`, which is precisely what these two groups inspect. A macro
  *body* class consumes greedily instead — the bare URL's own `[^\s\[\]<]*` excludes a `<` (so a
  tag-rendered span's closing markup already stops it in both pipelines) but admits an `&`, and at a
  smart quote's closing `&#8220;…&#8221;` the string pipeline swallows the whole entity into the
  target and leaves a stray `;` behind. Supplying the level one `&` would build a *third*,
  differently wrong target, so the closing character is dropped rather than half-supplied and that
  shape stays exactly as divergent as it already was, with a divergence test of its own. The other
  spellings read no character before their match at all — each is anchored on a literal (`image:`,
  `kbd:`, `menu:`, `indexterm`, `((`, `[[`, `anchor:`, `&lt;&lt;`, `xref:`, `link:`, `mailto:`) no
  context character can begin — so for them the wrap is inert, and they take it for uniformity
  rather than for effect. The bibliography anchor takes none: its pattern is `^`-anchored to the
  content's own top level, the one level nothing encloses.

  Two shapes stay divergent besides the closing half, each pinned by its own test. A **transparent**
  span — one rendering to its body and nothing else — has its children inherit the context it
  sits in, which is right when the span is all its parent's level holds and wrong when a
  **sibling**
  precedes it (`*x [width=10]#doc@example.org#*`, where the string pipeline reads the space that
  sibling ends with): the transparent-span half of the class the increment above already documents
  for its own steps, and closing it still means deriving a level's context from its siblings. And an
  address **abutting** an opaque piece keeps the deferral
  [`email_level`](../../parser/src/content/inline_builder/macros/links.rs) already documents — a
  placeholder belongs to no mismatch class, one level out from what a context answers.

  The e-mail and auto-link families each gain a differential corpus of their own construct written
  against a span's edge in every variant's rendering shape (tag-rendered and entity-rendered), the
  same constructs away from either edge, the escaped spellings, a replacement or restored entity in
  the boundary position, and the same fixtures at the content's own top level, alongside a
  structural assertion that the deferred address builds no node. Fixtures are added to the
  whole-pipeline broad sweep and combined-constructs corpus, to the group-parity corpus (a span's
  boundary comes from its rendering, which no effective order changes), and to the structural
  recorder cross-check, where the recorder — recovering what actually rendered — reads the
  string pipeline's own answer; a whole-document test drives both decisions end to end through the
  real parse path. Re-running the corpus-wide fold-parity audit (tree building forced on for every
  parse in the suite) confirms the divergence set strictly **shrank**: the two e-mail entries are
  gone and no new one appeared. As with every prep piece before it, nothing further is wired in.

  *Step 6 prep landed as (the boundary characters a span presents to its siblings):* the two
  increments above answer what an enclosing construct presents to the level *inside* it, and both
  name the same remaining half — a construct written **beside** a rendered span at its own level,
  where `build_match_string` stands the whole span in as one `SPAN_PLACEHOLDER` belonging to no
  boundary class at all. This is that half, for the class where the answer is unambiguous.

  The mechanism is the mirror image of a [`LevelContext`](../../parser/src/content/inline_builder/quotes.rs):
  where that wraps a *level* in the two characters its enclosing construct's rendering presents,
  [`build_match_string`](../../parser/src/content/inline_builder/quotes.rs) now wraps an opaque
  node's *placeholder* in the two its own rendering presents to a sibling — the **first** character
  of its opening markup and the **last** of its closing one, read from the same probe
  [`styled_boundaries`](../../parser/src/content/inline_builder/quotes.rs) takes its own pair from
  (now factored into one
  [`probe_styled_boundaries_markup`](../../parser/src/content/inline_builder/quotes.rs) the two read
  from opposite ends, pinned against the built-in renderer by a test of its own). Those two
  characters belong to **no piece** — they are the opaque node's markup, and the node already has
  the placeholder's piece — so a range reaching one contributes nothing, exactly as a
  `LevelContext`'s do: `emit_range` finds no piece overlapping it, and every gate skips it as
  non-overlapping. Nothing else in the module changes signature, and no gate, slice, or rebuild
  changes at all; the one adjustment is in
  [`s_to_src`](../../parser/src/content/inline_builder/quotes.rs), where a **leading** boundary
  character is the first offset that ever falls before the first piece and now resolves to that
  piece's own source start rather than dropping through to the past-the-last fallback.

  Only the **entity-rendered** variants are classified — the two smart quotes, `&#8220;…&#8221;`
  — and the scope is not about what a variant renders but about what
  [`styled_sibling_boundaries`](../../parser/src/content/inline_builder/quotes.rs) can *tell*. A
  `Styled` node reaching `build_match_string` is not necessarily a span the string pipeline has
  rendered: the passthrough-extraction pass builds one of its own for an attribute-list-prefixed
  passthrough (`[quotes]++text++`), which the string pipeline is holding as its own
  `\u{96}…\u{97}` sentinel rather than as markup for every step this module runs. A sibling reads
  that sentinel — which is exactly what the bare placeholder already reads as to every class in
  play (both are non-word, and in none of `&;:}`, `[>\(\)\[\];"']`, or `[\\>:/]`) — so leaving such
  a wrapper unclassified is the *right* answer, not an approximation. Telling one apart from a
  genuinely rendered span needs the identity
  [`masked_locations`](../../parser/src/content/inline_builder/special_chars.rs) collects before any
  step runs, which `build_match_string` does not have, and **both** wrappers that pass builds are
  tag-rendered; a smart-quote span, which it never builds, can only have come from the quotes step.

  Nothing is lost by drawing the line there, because a tag's `<`/`>` read exactly as the placeholder
  they would replace does to every *quote* boundary class — the classes that separate a `>` from a
  placeholder are the **macros** step's two prefix groups, and neither can reach an entity-rendered
  span at all: the sub that builds one requires a non-word character after its closing `` `" ``,
  while a URL and a bare address each begin with a word character. So this increment's whole effect
  lands in the quotes step, which is exactly where the divergence was. Four shapes are closed:
  ``` "`a`"`code` ```, ``` '`a`'`code` ```, `` "`a`"#mark# ``, and ``` "`a`"'`b`' ``` — each one
  the string pipeline leaves literal (or, for the last, wraps differently), reading the `;` that
  ends `&#8221;` where a placeholder said nothing.

  The divergence test the first increment left behind becomes a **parity** corpus, exactly as its own
  note asked, gaining each construct written against a smart-quote span's outer edge in both
  directions, the same constructs one character further out (where a space intervenes and both
  pipelines match), the tag-rendered shapes that were and remain at parity either way, and the same
  constructs at the content's own top level; a companion test pins that a tag-rendered wrapper still
  contributes the bare placeholder, asserted on the match string itself since no quote boundary class
  could tell the two apart. Fixtures are added to the whole-pipeline broad sweep and
  combined-constructs corpus and to the group-parity corpus (what a span presents to a sibling comes
  from its rendering, which no effective order changes), and a whole-document test drives the shape
  end to end through the real parse path. The **structural recorder cross-check** is the one corpus
  these shapes do not join, and the reason is worth recording: a
  [`RecordingRenderer`](../../parser/src/content/inline_tree.rs) emits its marker *outside* the
  markup it wraps, so a later sub matching over the recorded string reads that marker where the real
  pipeline reads the markup's own last character — the recorder builds a `<code>` span the real
  pipeline never rendered. That is the same perturbation Phase 1 named when it left special
  characters and replacements unmarked, it is one-sided (recognition *inside* a span is unaffected,
  since the marker sits outside the tag or entity pair, which is why the two increments above are
  cross-checked there normally), and it is now documented in that module's own header. Re-running the
  corpus-wide fold-parity audit (tree building forced on for every parse in the suite) shows the
  divergence set **unchanged**: no golden source in the suite writes a construct against a
  smart-quote span's outer edge, so unlike the six blockers before it this is an unclaimed form
  rather than a wrong answer for content already under test — and, as every increment requires, no
  new divergence appeared. As with every prep piece before it, nothing further is wired in.

  What still defers is the rest of the same class: the **tag-rendered** half, which waits on a way to
  carry `masked_locations`' identity into `build_match_string`; a **transparent** span's siblings
  (the other half of the class the first increment documents, still needing a level's context derived
  from its siblings rather than from its enclosing construct alone); and the **closing** character
  the macros step declines to half-supply.

  *Step 6 prep landed as (the boundary a **transparent** span's own siblings present):* the three
  increments above answer what an enclosing construct presents to the level inside it, what the
  macros step's families read there, and what a rendered span presents to a sibling — and all three
  name the same remaining shape: a span that renders to its **body and nothing else** (an unquoted
  span whose attribute list resolves to neither a role nor an id), where
  [`LevelContext::inside_styled`](../../parser/src/content/inline_builder/quotes.rs) hands the
  children whatever the span itself sees. That is right while the span is all its level holds, and
  wrong the moment a **sibling** precedes it: the string pipeline, having no levels, shows what that
  sibling rendered where the inherited context still shows the enclosing construct's markup.

  A new [`LevelContext::child_contexts`](../../parser/src/content/inline_builder/quotes.rs) answers a
  whole level at once — one context per node, so the three recursions that had each spelled out
  `inside_styled` and `INSIDE_REF` now zip a vector instead — and derives a transparent span's own
  half from the level's **match string**. That is the point of the mechanism rather than an
  implementation detail: the match string is the one place every node kind's presented bytes are
  already spelled out (a text run's, a `CharRef`'s entity, and the `SPAN_PLACEHOLDER` wrapped in
  whatever the increment above can say), so the character is *read* rather than recomputed per node
  kind and the two cannot drift. Each side falls back to the enclosing context where the level ends,
  which is exactly where the span really is the first thing its level holds. Building that match
  string is worth it only when such a span is present, so it is built on demand and every other level
  answers from `styled_boundaries` alone, allocating nothing new.

  Two limits are drawn deliberately, and both are the previous increments' own rules applied again.
  Only the **opening** character is carried, for the reason
  [`LevelContext::shift`](../../parser/src/content/inline_builder/quotes.rs) gives one level in and
  then some: a *boundary* class reads one character and, where it consumes one, the replacer writes
  it back — which [`unshift`](../../parser/src/content/inline_builder/quotes.rs)'s clip reproduces by
  leaving the character with the sibling that owns it — while a *delimiter* swallows it instead, and
  what the replacer swallows it **deletes**, which a level's rebuild cannot do to a node another
  level owns (`x[width=10]##d #c###`, whose closing `#` is the sibling, keeps its own divergence
  test). And a **bare placeholder** reports *nothing* rather than a character: it is what
  `build_match_string` writes for a node this module cannot describe, so reporting it would
  manufacture an answer where the level previously read its own `^` — and `^` is what the auto-link's
  prefix group accepts there anyway, so `*x*[width=10]#https://example.org#` would have gone from
  parity to divergence. An unclassified neighbour leaves the span inheriting, the same line
  [`styled_sibling_boundaries`](../../parser/src/content/inline_builder/quotes.rs) draws for the same
  reason, and `haystack` gains the one generalization this needs — the two halves applied
  independently, since a sibling can supply an opening character to a level whose closing one is
  still the content's own end.

  The [`character replacements`](../../parser/src/content/inline_builder/char_replacements.rs) step
  is deliberately **not** given this. Its one boundary-reading rule is the spaced em dash, whose
  replacement consumes the spaces it matches on both sides rather than writing them back — so even an
  opening character a sibling owns would be emitted here *and* left there. Supplying it would make
  `*x [width=10]#-- y#*` differently wrong rather than right, so that step goes on inheriting and
  `a_replacement_beside_a_transparent_span_is_a_documented_divergence` keeps its shape, its note now
  carrying the sharper reason (not "a strictly larger walk" but "one level's rebuild would have to
  consume a node another level owns"). The two steps that *can* take it do: the macros step's bare
  e-mail and auto-link families (`*x [width=10]#doc@example.org#*`, the shape the first of these
  increments pinned, whose test is now a parity corpus exactly as its own note asked), and the quotes
  step's constrained `#mark#` — the only boundary-reading sub that can run after a transparent span
  exists, since the span it looks across must have been built by the unconstrained `##mark##` one
  place ahead of it (`x[width=10]###c# d##`).

  Each family's construct gains a differential corpus written against a transparent span's edge with
  a word character, a space, an entity-rendered span and a tag-rendered one beside it, the same
  constructs with no sibling at all (where the enclosing context is still the answer), the same at
  the content's own top level, and a sibling *after* the span (which supplies nothing), alongside a
  unit test of `child_contexts` itself and a structural assertion that the address builds a real
  link. Fixtures are added to the whole-pipeline broad sweep and combined-constructs corpus, to the
  group-parity corpus (what a sibling renders comes from its own rendering, which no effective order
  changes), and to the structural recorder cross-check, and a whole-document test drives both shapes
  end to end through the real parse path. Re-running the corpus-wide fold-parity audit (tree building
  forced on for every parse in the suite) shows the divergence set **unchanged** — no golden source
  writes a construct inside a transparent span, so like the increment above this is an unclaimed form
  rather than a wrong answer for content already under test — and, as every increment requires, no
  new divergence appeared. As with every prep piece before it, nothing further is wired in.

  What still defers is the rest of the same class: the **tag-rendered** half, which waits on a way to
  carry `masked_locations`' identity into `build_match_string` (and which now shows up in two places
  — what a rendered span presents to a sibling, and what an unclassified neighbour presents to a
  transparent span); the **closing** character both the macros step and this increment decline to
  half-supply; a **transparent** span read *as* a sibling, which presents its own body's last
  character where the placeholder says nothing; and the character-replacements step's own
  consume-across-levels case above.

  *Step 6 prep landed as (the extraction pass's identity, and with it the tag-rendered half):*
  the two increments above both stop at the same place and name the same blocker: a
  [`Styled`](../../parser/src/inlines/styled.rs) node reaching
  [`build_match_string`](../../parser/src/content/inline_builder/quotes.rs) is not necessarily a span
  the string pipeline has rendered, because the passthrough-extraction pass builds one of its own for
  an attribute-list-prefixed passthrough (`[quotes]++text++`) that the pipeline is still holding as
  its `\u{96}…\u{97}` sentinel for every step this module runs. Both wrappers that pass builds are
  **tag-rendered**, so classifying a tag-rendered span by its rendering alone would sometimes hand a
  sibling a `>` the string pipeline does not present — and telling the two apart needs the *identity*
  [`masked_locations`](../../parser/src/content/inline_builder/special_chars.rs) collects before any
  step runs. This increment carries that identity into recognition, and closes the half it was
  blocking.

  The identity travels as a
  [`Masked`](../../parser/src/content/inline_builder/special_chars.rs), whose point is its **third
  state**: a caller that does not hold the list says `Masked::UNKNOWN` rather than passing an empty
  one, which would claim that nothing is a wrapper. An unknown identity leaves every tag-rendered
  span with the bare placeholder — the answer the module already gave — so every step but one goes on
  passing it and is byte-identical to before. Only the **macros** step is given the real list, and
  that is not a shortcut but the scope the previous increment's own note had already drawn: the two
  classes in the whole module that read a tag's `>` differently from the placeholder are
  `INLINE_LINK`'s boundary-prefix group and `INLINE_EMAIL`'s mismatch-prefix one, and a quote sub's
  `(^|[^\w&;:}])` accepts the two alike. `masked_locations` itself becomes unconditional (it had been
  taken only for the order that escapes late, `flatten_prior_markup`'s own consumer), since a
  wrapper's identity has to be recorded before any step runs whatever the order is.

  The identity answers for **one content's own** constructs, and one path puts constructs somewhere
  else: the `x-` compatibility form (`[x-]++text++`) substitutes its body through a *separate,
  nested* `Normal` build, so a wrapper **that** build extracts is invisible to this content's list.
  Consulting the list there would find no entry and call the wrapper a rendered span — the one wrong
  answer the type exists to prevent. Nothing consults it, because the macros step no longer descends
  into a wrapper at all: a wrapper's body is not this content's to substitute (the string pipeline
  holds it as a sentinel for every step from here on) and the nested build already substituted it
  once, so descending had been applying the step's families a *second* time. That was inert while a
  bare placeholder stopped every boundary class; it stops being inert the moment a span presents a
  character, which is how `[x-]++**b**https://example.org++` had been growing an `<a>` inside the
  `<a>` the nested build already made. Skipping the descent is the faithful reading, and it closes
  that shape rather than deferring it: the fold now matches the string pipeline's bytes exactly.

  With it,
  [`styled_sibling_boundaries`](../../parser/src/content/inline_builder/quotes.rs) stops being a
  two-variant special case and becomes what its mirror image
  [`styled_boundaries`](../../parser/src/content/inline_builder/quotes.rs) already was — every
  variant answered from its own rendering shape, with the one whose shape its attribute list decides
  probed. The two entity-rendered variants keep their unconditional answer, because the extraction
  pass builds neither; a **transparent** span falls out by itself, having no markup to take a
  character from, and defers with its own note. Nothing else changes: the two characters still belong
  to no piece, so no gate, slice, or rebuild moves.

  What reaches parity is `**bold**https://example.org` and every tag-rendered variant of it — the
  string pipeline reads the `>` that ends `</strong>` where the tree read a placeholder belonging to
  no class, so it linked and the tree did not — together with the reverse direction (a span written
  against a bare URL's tail, where the `<` that opens the tag is what ends the URL body) and the
  shape one level in: `*x*[width=10]#doc@example.org#`, where a **transparent** span's own sibling is
  tag-rendered and
  [`child_contexts`](../../parser/src/content/inline_builder/quotes.rs) now hands it that same `>`
  — one of the e-mail pattern's mismatch characters — instead of the `^` it had been inheriting. Both
  of those were divergence tests the two increments above left behind, and both become parity corpora
  exactly as their own notes asked. The extraction pass's wrapper keeps its own test, asserting that
  `[quotes]++x++https://example.org` stays literal in both pipelines — the new divergence this
  increment would have introduced had it classified by rendering alone.

  Fixtures join the whole-pipeline broad sweep and combined-constructs corpus and the group-parity
  corpus (what a span presents to a sibling comes from its rendering, which no effective order
  changes), a unit test pins `styled_sibling_boundaries` against the built-in renderer in all three
  states of the identity, and a whole-document test drives the shapes end to end through the real
  parse path. The **structural recorder cross-check** is again the one corpus these shapes do not
  join, for the reason the increment above recorded: a
  [`RecordingRenderer`](../../parser/src/content/inline_tree.rs) emits its marker *outside* the
  markup it wraps, so a later sub reads that marker where the real pipeline reads the markup's own
  outer character. Re-running the corpus-wide fold-parity audit (tree building forced on for every
  parse in the suite) shows the divergence set **unchanged** — no golden source in the suite writes a
  bare URL against a rendered span's closing edge — and, as every increment requires, no new
  divergence appeared. As with every prep piece before it, nothing further is wired in.

  What still defers is the rest of the same class, now one item shorter and one item unblocked. A
  **transparent** span read *as* a sibling presents its own body's last character where the
  placeholder says nothing (`[width=10]##x ##https://example.org`, which the string pipeline links on
  the space the body ends with), and that was gated on this same identity for the same reason —
  `[width=10]++x ++` is an extraction wrapper that renders its body and nothing else, so classifying
  by rendering alone would have been wrong for it too. `Masked` tells the two apart, so the next
  increment can take it; what it still needs is a way to *read* a transparent span's own outer
  characters, which are its children's rather than any markup of its own. Beyond that: the **closing**
  character both the macros step and the increment above decline to half-supply, and the
  character-replacements step's own consume-across-levels case above.

  *Step 6 prep landed as (a **transparent** span read as a sibling):* the increment above closed the
  tag-rendered half of what a span presents to its siblings and named what it left: a span rendering
  to its **body and nothing else** presents no markup for
  [`styled_sibling_boundaries`](../../parser/src/content/inline_builder/quotes.rs) to take a
  character from, so [`build_match_string`](../../parser/src/content/inline_builder/quotes.rs) went
  on standing it in as one bare placeholder — where the string pipeline, having no levels, holds
  that body itself. `[width=10]##x ##https://example.org` links there on the space the body ends
  with, and the tree read a placeholder belonging to no boundary class.

  A transparent span's two outer characters are its **children's**, so
  [`transparent_sibling_boundaries`](../../parser/src/content/inline_builder/quotes.rs) reads them
  out of the children's own match string rather than recomputing them per node kind — the same move
  [`child_contexts`](../../parser/src/content/inline_builder/quotes.rs) made one increment ago for
  the same reason: the match string is the one place every node kind's presented bytes are already
  spelled out (a text run's, a `CharRef`'s entity, a nested span's own placeholder wrapped in
  whatever this module can say there), so the two cannot drift, and a transparent span nested inside
  another answers from *its* children with no second mechanism. The pair becomes two independent
  `Option`s, because a body can begin with something this module cannot describe and still end in
  something it can; every markup-rendering variant goes on answering both or neither, and a
  [`SPAN_PLACEHOLDER`](../../parser/src/content/inline_builder/quotes.rs) at either edge reports
  *nothing* rather than manufacturing a character — the line `preceding_character` already draws.

  The **identity** is what makes this safe, and it is why this increment had to wait for the one
  above: `[width=10]++x ++` is an extraction wrapper that renders its body and nothing else too, and
  what the string pipeline holds there is its own `\u{96}…\u{97}` sentinel, not the body — so
  classifying a transparent span by its rendering alone would have handed the URL beside it a space
  the pipeline never presents. A transparent span takes exactly the same `masked` guard every
  tag-rendered one takes, which also keeps the scope the previous increments drew: only the
  **macros** step holds the real list, so only its two boundary-reading families see any of this and
  every other step is byte-identical to before. `child_contexts` gains the one adjustment this
  forces — the character a transparent span presents to its *neighbour* now sits between them, so
  the lookup steps back over it to reach what the span's own children read, which is what precedes
  the **span**.

  What reaches parity is that shape and its mirror image: `[width=10]##x ##https://example.org` and
  `[width=10]##x ##doc@example.org` (and the same one level in, inside a `*strong*` span, and with a
  sibling of its own before the span), whatever node kind carries the body's last character — a text
  run after a tag-rendered child, a restored entity, a typographic replacement, an escaped special —
  together with `https://example.org[width=10]## x##`, where the body's **first** character is what
  ends a bare URL written against the span's opening edge, and the negative half, where a body
  ending in a word character or in one of the e-mail pattern's own mismatch characters leaves the
  construct literal in both. The extraction pass's wrapper keeps its own test, pinning that
  `[width=10]++x ++https://example.org` stays literal in both pipelines — the new divergence a
  rendering-only classification would have introduced.

  Fixtures join the whole-pipeline broad sweep and combined-constructs corpus and the group-parity
  corpus (what a span presents to a sibling comes from its own rendering, which no effective order
  changes), unit tests pin `styled_sibling_boundaries` against the built-in renderer in all three
  states of the identity and the match string's own wrap directly, another pins that a transparent
  span's children still read what precedes the span, and a whole-document test drives both shapes
  end to end through the real parse path. The **structural recorder cross-check** is again the one
  corpus these shapes do not join, and for the same reason as the two increments above seen from the
  other side: a [`RecordingRenderer`](../../parser/src/content/inline_tree.rs) emits its marker
  *outside* the span it wraps, so it stands between a transparent span's body and the construct
  beside it and the recorder reads the marker where the real pipeline reads the space. Re-running
  the corpus-wide fold-parity audit (tree building forced on for every parse in the suite) shows the
  divergence set **unchanged** — no golden source writes a construct beside a transparent span —
  and, as every increment requires, no new divergence appeared. Coverage stays diff-neutral. As with
  every prep piece before it, nothing further is wired in.

  What still defers is the rest of the same class, now one item shorter. A bare URL whose **body
  class** wants more than one character still cannot swallow a transparent span's whole body the way
  the string pipeline's flat haystack lets it (`https://example.org[width=10]##x##`, which the
  pipeline links as `https://example.orgx`): the character the span presents is right, and a match
  crossing the span is not this level's to build, so that keeps its own divergence test. Beyond
  that: the **closing** character both the macros step and the sibling increments decline to
  half-supply, and the character-replacements step's own consume-across-levels case.

  *Step 6 prep landed as (a `link:`/`mailto:` target crossing a **passthrough**):* with the
  boundary-character seam settled — its three remaining deferrals all of the keep-forever,
  markup-perturbed kind — re-running the corpus-wide fold-parity audit and classifying what remains
  by *source* found the largest golden-exercised class to be something no note had named: a masked
  passthrough in a `link:`/`mailto:` macro's **target**, the documented idioms
  `link:++https://example.org/now_this__link_works.html++[]` (formatting characters kept literal)
  and `link:pass:[My Documents/report.pdf][Get Report]` (a space, which the target class itself
  rejects) — five real fixtures across the asciidoc-lang corpus. The string replacer swallows the
  `\u{96}`*n*`\u{97}` sentinel into the target (its `[^\s\[\]]+` class admits it exactly as it
  admits the tree's placeholder, so the two recognize the same extent), and
  [`Passthroughs::restore_to`](../../parser/src/content/passthroughs.rs) then splices the extracted
  body's substituted text over every sentinel in the rendered string. A
  [`Raw`](../../parser/src/inlines/inline_node.rs) node's `value` **is** that substituted text,
  known at build time — which is what separates a masked passthrough from a rendered span, whose
  markup exists only at fold time — so a fourth gate,
  [`range_is_restorable`](../../parser/src/content/inline_builder/macros/image.rs), admits it for a
  value the caller *restores*, and
  [`restore_masked_passthroughs`](../../parser/src/content/inline_builder/macros/links.rs)
  substitutes the value for the placeholder, finishing the computed target into exactly the
  restored string's bytes. Every *decision* the replacer makes stays on the bytes as matched — the
  `URI_SNIFF` strip under `hide-uri-scheme` sniffs the masked target, so a scheme hidden inside the
  passthrough keeps its scheme in the shown text, as the golden does — and the shown text itself is
  structural: a bare macro's `emit_range` carries the `Raw` node whole, folding to the restored
  bytes with no re-escaping.

  One reading is deliberately **not** faithful: the dangerous-scheme check runs over the *restored*
  target, where the string replacer checks its own masked haystack — through which
  `link:++javascript:alert(1)++[]` passes, the restore then completing a live `javascript:` link in
  the golden output. The tree defers it instead, a security divergence test pinning the safe
  reading over byte parity. The staged
  [`apply_link_side_effects`](../../parser/src/content/inline_builder/macros/links.rs) similarly
  registers the node's honest restored target where the string pipeline registers the sentinel
  bytes verbatim (its restore rewrites only the rendered string, never the catalog) — a wart the
  cutover deliberately will not reproduce, pinned by its own test rather than by a golden-catalog
  comparison.

  What reaches parity is the five golden fixtures and the class around them: both idioms bare and
  labeled, a passthrough covering part of the target (`link:https://++example.org/a++[]`,
  `link:a++b c++d[T]`), the `mailto:` spellings (with and without a subject/body attrlist), an
  attribute list on a text beside a restored target, the triple-plus form, escapes, and the macro
  inside a rendered span. Fixtures join a new differential corpus, a `hide-uri-scheme` pair pins
  the masked strip in both directions, structural tests pin the restored target and the
  `Raw`-child text, and a whole-document test drives the shapes end to end through the real parse
  path. Re-running the corpus-wide audit shows **no new divergence and five closed** — the first
  increment since the audit map closed to shrink the golden set. The other families keep the
  boundary, each newly pinned with the reason it cannot make the same move yet: the
  cross-reference family *must* keep it (a deferred xref's target is read **before** restore can
  reach it, and the golden output for `xref:++id++[]` leaks the raw sentinel into its own `href` —
  the tree's literal reading is the well-formed one), the image family computes several
  masked-arithmetic values off the same bytes (`default_alt`'s basename/`_`/`-` rewrites run over
  the sentinel, so `image:++a_b-c.jpg++[]` keeps `alt="a_b-c.jpg"`), and the auto-link family
  reads boundary-prefix and trailing-strip arithmetic off them; each is a later increment by the
  same restore-the-value, decide-over-the-masked-bytes move, where its own goldens call for it.
  Coverage stays diff-neutral. As with every prep piece before it, nothing further is wired in.

  *Step 6 prep landed as (an **image/icon** target crossing a passthrough):* the first of the two
  families the increment above left named takes the same restore-the-value,
  decide-over-the-masked-bytes move, and needed two more mechanisms to take it. **Recognition**
  first: [`INLINE_IMAGE_MACRO`](../../parser/src/content/macros.rs)'s target class is the one in
  the module that requires *two* characters (`[^:\s\[\n][^\[\n]*?[^\s\[\n]`), so a target written
  wholly inside a passthrough — one placeholder character — could not even match, where the string
  replacer's three-byte sentinel does.
  [`widen_masked_passthroughs`](../../parser/src/content/inline_builder/macros/image.rs) rewrites
  the family's match string so each masked passthrough's placeholder becomes a sentinel-shaped
  `\u{96}`*n*`\u{97}` token — the very bytes the string pipeline's own haystack holds there —
  moving the pieces into the rewritten string's coordinates, so recognition agrees byte-for-byte
  without touching the shared pattern, and no token byte can begin or end a match or reach an
  output node. Then the gates move from the family to the values, as the `link:`/`mailto:`
  increment's did: the **target** — the one value this family computes — takes
  [`range_is_restorable`](../../parser/src/content/inline_builder/macros/image.rs), finishing into
  the restored bytes through the shared
  [`restore_masked_passthroughs`](../../parser/src/content/inline_builder/macros/links.rs), while
  the **bracket**, which comes back from a *parse* (`Attrlist::parse` would read a placeholder's
  bytes as content, and the string pipeline's own parse swallows the sentinel into a value that
  only restores after the split — so a body carrying a `,` or `=` stays one attribute there),
  keeps the opaque-piece gate, its own restore-inside-each-parsed-value a later increment's call.
  The second mechanism is the **`default_alt` arithmetic**, the family's one pre-restore
  computation: the string pipeline derives `basename(target.replace(['_', '-'], " "))` over the
  sentinel-holding bytes and restores whatever survives into the rendered `alt`, so
  [`masked_default_alt`](../../parser/src/content/inline_builder/macros/image.rs) reproduces
  exactly that — the arithmetic over the masked bytes (every cut point a byte no token contains,
  so a token survives whole or drops whole), then an index-keyed restore, as
  `Passthroughs::restore_to`'s own numbering is — which is how `image:++a_b-c.jpg++[]` keeps
  `alt="a_b-c.jpg"` where the verbatim spelling shows `a b c`, and `image:++dir_1++/++file_2++.png`
  restores the surviving token with its *own* body after the stem cut drops the first.

  What reaches parity is both idioms bare and labeled (`image:++sunset.jpg++[Alt]`,
  `image:pass:[chart,v2.png][]`), partial masks at either edge and mid-target, a URI target
  crossing one, the `icon:` form, the triple-plus form, escapes, the macro inside a rendered span,
  and two in one flow. Two shapes are deliberately **not** faithful, each the tree's well-formed
  reading pinned by its own test: a dangerous scheme smuggled into a `link=self` target
  (`image:++javascript:alert(1)++[link=self]` — the string renderer checks the sentinel and the
  restore completes a live link; the fold's renderer checks the restored target and drops the
  anchor), and a **space** restored into the target (`image:pass:[My Documents/chart.png][]` — the
  fold's `web_path` percent-encodes the restored space into the `src`, where the string pipeline
  normalized its space-free sentinel and spliced the raw space in afterwards). The staged
  [`apply_image_side_effects`](../../parser/src/content/inline_builder/macros/image.rs) registers
  the node's honest restored target where the string pipeline registers sentinel bytes verbatim —
  the same catalog wart the link increment declined to reproduce, pinned the same way. Fixtures
  join a new in-module differential corpus, structural tests pin the restored target and the
  masked-derived alt, and a whole-document test drives the shapes end to end through the real
  parse path. Re-running the corpus-wide audit shows the divergence set **unchanged** — no golden
  source exercises an image target over a passthrough — and no new divergence appeared. Coverage
  stays diff-neutral. As with every prep piece before it, nothing further is wired in.

  *Step 6 prep landed as (an **auto-link / formal-URL** target crossing a passthrough):* the last
  family of the class, and the only one of the three to need no new mechanism at all — the gates
  moved onto the values and the computed target learned to finish into the restored bytes, and
  nothing else changed. **Recognition** needed no widening, unlike the image family's:
  [`INLINE_LINK`](../../parser/src/content/macros.rs)'s three URL classes each admit the
  `\u{96}`*n*`\u{97}` sentinel and the tree's one-character placeholder alike (`[^\s\[\]]+` for a
  formal target, `[^\s]+?` between the angle delimiters, and the bare branch's
  `[^\s\[\]<]*[^\s,.?!\[\]<\)]`, whose trailing-character half admits the last byte of either
  spelling), so the two pipelines end the match at the same source construct. The gate then
  becomes [`range_is_restorable`](../../parser/src/content/inline_builder/macros/image.rs) over the
  whole range this pass *reads* — the boundary prefix, the scheme, and the URL — rather than over
  the URL alone, because the URL is the only capture in it a placeholder can reach: this family's
  boundary-prefix class admits none of the characters
  [`build_match_string`](../../parser/src/content/inline_builder/quotes.rs) stands a piece in as,
  and its scheme is literal ASCII no single-character piece can supply. The angle path
  ([`build_angle_link_node`](../../parser/src/content/inline_builder/macros/links.rs)) takes the
  same swap over its own interior, which *is* the target's range.

  The **boundary-prefix and trailing-strip arithmetic** the note above named as this family's
  blocker turned out to need no masked reading of its own — it already is one. Both decisions are
  made before the restore, over the bytes as matched, and a placeholder answers each exactly as the
  string replacer's sentinel answers it: neither spelling is the `"`/`'` an invalid quoted URL is
  rejected on, and neither is the `;` or `:` the strip keys off — so a `;` *inside* a passthrough
  stays in the target in both pipelines (`https://example.org/a++;++`) while a literal one after it
  is stripped in both (`see https://example.org/++a++; now`). The `hide-uri-scheme` strip is the one
  place this family is *simpler* than the `link:`/`mailto:` one, which must sniff its masked target
  because a passthrough can hide the whole scheme there: `INLINE_LINK` requires the scheme to be
  **literal**, so [`URI_SNIFF`](../../parser/src/content/macros.rs)'s `^`-anchored match covers the
  same bytes in the masked and restored spellings, leaving the offset a valid cut in the match
  string — where a bare link's shown text is recovered from, carrying the
  [`Raw`](../../parser/src/inlines/inline_node.rs) node itself and folding to the restored bytes
  with no re-escaping, as a bare `link:` macro's does.

  What reaches parity is both documented idioms in every spelling this family has —
  `https://++example.org/now_this__link_works.html++` and `https://example.orgpass:[/a b]` bare,
  formal, and angle-bracketed — plus partial masks at either edge, mid-target, and two in one
  target, the strip in both directions, the other schemes the pattern admits, a display text beside
  a restored target (carrying markup, its own attribute list, a `^` window suffix, or a passthrough
  of its own), the triple-plus form, escapes, the link inside a rendered span, and two in one flow;
  a `hide-uri-scheme` pair pins the strip, structural tests pin the restored target and the
  `Raw`-child text, and a whole-document test drives the shapes end to end through the real parse
  path. Re-running the corpus-wide audit shows the divergence set **unchanged** — no golden source
  exercises an auto-link target over a passthrough (the five that named this class all spell the
  `link:` macro) — and no new divergence appeared. Coverage stays diff-neutral, and as with every
  prep piece before it, nothing further is wired in.

  Three shapes are deliberately left where they were, each pinned by its own test. A `"` restored
  into the target is escaped by the fold's `encode_attribute_value` where the string pipeline
  encoded its quote-free sentinel and spliced the raw `"` into the finished `href` — closing the
  attribute it lands in — so the tree's is the well-formed reading, and the one both sibling
  families' restores already take. An **attribute-list display text** crossing a passthrough keeps
  the opaque-piece gate inside `text_attrlist`, as its rendered-span sibling does: that text comes
  back from a *parse*, and a placeholder inside a parsed value has no node to map back to. And a
  **STEM** expression in the target stays literal: the same extraction pass masks it, and its
  rendered value is known at build time too, but it builds a [`Stem`](../../parser/src/inlines/stem.rs)
  node rather than a `Raw` one — a lift worth making for all three families at once rather than
  here alone, exactly as the `link:`/`mailto:` family left it.

  With this the restore-the-value class is closed for every family that computes a target. What
  still defers is the **bracket half** — restoring inside each *parsed* attribute-list value (an
  image's bracket, the three families' attribute-list display texts), where the string pipeline's
  own parse swallows the sentinel into a value that only restores after the split — the **STEM**
  half just named, and the keeps: the cross-reference family's pre-restore target (whose golden
  leaks the raw sentinel into its own `href`), and the four well-formed readings these three
  increments pinned.

  *Step 6 prep landed as (a masked **STEM** expression in a computed target):* the lift the three
  restore-the-value increments above each deferred to "the STEM step's own increment, across all
  three families at once", made in one place rather than three. A STEM expression is an *implicit*
  passthrough — [`Passthroughs::extract_from`](../../parser/src/content/passthroughs.rs) masks it in
  the very same pass, before any substitution step runs — so it stands in the string pipeline's
  haystack as the same `\u{96}`*n*`\u{97}` sentinel and in this module's match string as the same
  one-character placeholder. What kept it out was only that the restore machinery reached into a
  [`Raw`](../../parser/src/inlines/inline_node.rs) node's `value` by name, and a masked STEM builds a
  [`Stem`](../../parser/src/inlines/stem.rs) node instead.

  One family is deliberately **left out**: `image:`/`icon:`. Its target is the only one re-processed
  by [`web_path`](../../parser/src/parser/path_resolver.rs) at fold time, and `web_path`
  *posixifies the platform separator* — so a restored body carrying a backslash comes out rewritten
  on a Windows-separator resolver. The string pipeline never meets this, because its `web_path` runs
  over the backslash-free **sentinel** and the restore splices the body into the finished `src`
  afterwards. For a passthrough that is an exotic body — the increment above pinned exactly one such
  case, a restored *space* the fold percent-encodes. For STEM it is **every** body: a rendered
  expression always carries a backslash (`\$…\$`, `\(…\)`), so `image:stem:[x].png[]` would render
  `src="/$x/$.png"` on Windows against the golden's `src="\$x\$.png"`. Deferring keeps this family's
  `src` identical on every platform, where restoring would make it differ by one; the two link
  families, whose targets reach the `href` as computed, take the restore. That split is what
  [`Restorable`](../../parser/src/content/inline_builder/macros/image.rs) names at the gate, and two
  tests pin it — one that the image family stays literal, one that its fold is byte-identical under
  either separator, so the difference is visible on a Posix runner instead of only on Windows CI.

  The fix is a **pair of shared helpers** rather than a widened `matches!` at each site:
  [`node_is_restorable`](../../parser/src/content/inline_builder/macros/image.rs) is the cheap
  discriminant the two gates use (so a range about to be *rejected* costs no rendering), and
  [`restorable_body`](../../parser/src/content/inline_builder/macros/image.rs) produces the bytes the
  two restores splice. The invariant they rest on is that a restored body is **exactly what the fold
  of that node emits**: a `Raw` leaf's is its `value`, which the fold emits verbatim (so it is
  borrowed, not rendered); a `Stem` leaf's is
  [`fold_stem`](../../parser/src/content/inline_builder/fold.rs)'s own output — the same
  `render_quoted_substitution` call, over the already-substituted `value` with no attribute list or
  id, that `PassthroughRestoreReplacer` makes for a STEM entry. Reusing that one function is what
  keeps the restore and the fold from drifting, and a unit test pins the two helpers to the same set.
  With the pair in place, the two sites the link families use —
  [`range_is_restorable`](../../parser/src/content/inline_builder/macros/image.rs) (which now takes
  the kinds its caller admits) and
  [`restore_masked_passthroughs`](../../parser/src/content/inline_builder/macros/links.rs) — extend
  to both masked kinds at once, and both link families follow. The image family's own two pieces,
  [`widen_masked_passthroughs`](../../parser/src/content/inline_builder/macros/image.rs) and
  [`masked_default_alt`](../../parser/src/content/inline_builder/macros/image.rs), are untouched:
  that family is unchanged by this increment. Recognition needed no widening at all here — both link
  families' target classes swallow the one-character placeholder as they swallow the sentinel — and
  every pre-restore decision keeps reading the bytes as matched, which a placeholder answers as the
  sentinel does.

  One difference from the passthrough class is worth recording, because it runs the *safe* way: a
  STEM body cannot smuggle a dangerous scheme. `link:++javascript:…++[]` defers with a security
  divergence test because a passthrough's body is restored **bare**, so the string replacer's masked
  check misses a live scheme; a STEM body is restored **wrapped** in its notation's delimiters
  (`\$javascript:alert(1)\$`), which is not a scheme in either pipeline — so
  `link:stem:[javascript:alert(1)][]` and an image `link=self` over a STEM target both reach plain
  parity, with no divergence to pin. The `renderer` these restores use is the **parser's**, mirroring
  `restore_to`'s own: a computed target freezes its STEM bytes at build time exactly as the string
  pipeline freezes them into its `href`, where a `Stem` node standing in the flow is rendered at fold
  time instead — the two agree whenever the fold uses the parser's renderer, which is the seam §3.3.1
  defines and the only one `Content` uses.

  What reaches parity is every spelling the two link families have: the `link:`/`mailto:` macro (bare
  and labeled, the expression at either edge, in the middle, and twice in one target), auto-links,
  formal-URL links and the two angle-bracketed forms — with all three notations, an explicit
  substitution list (`stem:c,q[…]`), the `hide-uri-scheme` strip, the trailing-punctuation strip in
  both directions, a display text beside a restored target, a STEM beside a passthrough in either
  order, both path separators, and the whole set inside a rendered span, escaped, and end to end
  through the real parse path. Re-running the corpus-wide audit shows the divergence set
  **unchanged** — no golden source writes a STEM expression inside a computed target — and no new
  divergence appeared. Coverage stays diff-neutral, and as with every prep piece before it, nothing
  further is wired in.

  Beyond the image family just described, one more shape is left where it was: an **attribute-list
  display text** over a STEM expression keeps the opaque-piece gate, as its passthrough sibling does,
  because it comes back from a *parse* and a placeholder inside a parsed value has no node to map
  back to.

  With this the restore-the-value class is closed for both masked kinds in the two families whose
  computed target reaches the output as computed. What still defers is the **bracket half** —
  restoring inside each *parsed* attribute-list value (an image's bracket, the three families'
  attribute-list display texts), where the string pipeline's own parse swallows the sentinel into a
  value that only restores after the split — the **image family's STEM target**, whose fold-time
  `web_path` is the blocker just described (closing it means keeping this family's restore out of
  `web_path`'s way, not widening a gate), and the keeps: the cross-reference family's pre-restore
  target (whose golden leaks the raw sentinel into its own `href`), and the well-formed readings
  these four increments pinned.

  *Step 6 prep landed as (a masked passthrough inside an image's **parsed bracket** — the bracket
  half's first family):* the four increments above closed the restore-the-value class for every
  family that *computes* a target off the match string. What each of them deferred, in the same
  words, was the **bracket half**: restoring inside a value that comes back from a **parse**, where
  "the string pipeline's own parse swallows the sentinel into a value that only restores after the
  split". This takes the first of that half's four captures, the `image:`/`icon:` bracket, by
  reproducing exactly that order.

  The order is the whole point, and it is what a restore-then-parse cannot reproduce.
  [`Attrlist::parse`](../../parser/src/attributes/attrlist.rs) reads the `\u{96}`*n*`\u{97}`
  sentinel as one opaque run carrying none of the `,`/`=`/`"` bytes the split reads, so
  `image:x.png[++a,b++]` is **one** positional whose value is `a,b` — restoring first would divide
  it into two. So the bracket is put into that same shape before the parse and restored after it:
  [`tokened_bracket`](../../parser/src/content/inline_builder/macros/image.rs) rewrites each masked
  piece in the bracket's match-string bytes to an index-keyed token (normalizing two spellings into
  one — [`widen_masked_passthroughs`](../../parser/src/content/inline_builder/macros/image.rs) has
  already widened a `Raw` piece for *recognition*, but numbered per level), and a new
  [`Attrlist::into_owned_restoring`](../../parser/src/attributes/attrlist.rs) — the restoring
  sibling of the [`into_owned`](../../parser/src/attributes/attrlist.rs) the first attribute-list
  increment added — splices each body into the parsed values on the way out. Restoration is
  index-keyed, as [`Passthroughs::restore_to`](../../parser/src/content/passthroughs.rs) is, so a
  token the split discarded does not shift the ones that survive.

  One thing had to be *shifted* rather than recomputed. An
  [`ElementAttribute`](../../parser/src/attributes/element_attribute.rs)'s
  `shorthand_item_indices` are byte offsets into its own `value`, and a restore that lengthens the
  value ahead of them would leave them pointing mid-word — so
  [`into_owned_restoring`](../../parser/src/attributes/element_attribute.rs) moves each offset past
  every substitution that ends at or before it. Shifting is the faithful move and re-deriving is
  not: a token holds none of the `#`/`.`/`%` delimiters the shorthand scan keys off, so the items
  the string pipeline found over its sentinel are the same items, only further along
  (`image:x.png[++abc++.myrole]` keeps the `myrole` role while its `alt` becomes `abc.myrole`),
  where a re-derivation would find a delimiter *inside* a restored body that the string pipeline
  never sees. The no-token path keeps the plain `CowStr::into_owned` conversion, so nothing that
  does not carry a token pays for this.

  The family's gate simply becomes the one its target already uses —
  [`range_is_restorable`](../../parser/src/content/inline_builder/macros/image.rs) with
  [`Restorable::Passthrough`](../../parser/src/content/inline_builder/macros/image.rs) — in place
  of [`range_has_no_opaque_piece`](../../parser/src/content/inline_builder/macros/image.rs), so
  target and bracket now admit exactly the same kinds. Recognition needed no widening for the
  bracket: `INLINE_IMAGE_MACRO`'s bracket class swallows either spelling. `alt`, `width`, and
  `height` are read off the attribute list, so they follow with no code of their own.

  A masked **STEM** expression is deliberately still deferred here, for the reason the target's own
  increment gave and one more site: the bracket has a `web_path`-bound value of its own — an
  interactive SVG's `fallback=`, run through `image_src` — and every rendered STEM body carries a
  backslash `web_path` would posixify on a Windows-separator resolver. Keeping both halves of this
  family on `Restorable::Passthrough` keeps its `src` identical on every platform, and leaves the
  family's STEM story exactly one item rather than two.

  What reaches parity is the whole bracket vocabulary over a passthrough: the plain and partial
  alt, several tokens in one bracket, the split invariant in all three spellings (a body carrying a
  `,`, an `=`, or sitting inside a quoted value), named values (`title=`, `role=`), the positional
  width/height slots, both shorthand items after a token, a restored `&` (which
  `encode_attribute_value` passes through in both pipelines), the `pass:[…]` and triple-plus
  spellings, an attribute reference hidden inside the body (which neither pipeline expands, both
  parsing the masked text), target and bracket both masked, the icon form, and the whole set inside
  a rendered span, twice in one flow, and escaped. Re-running the corpus-wide fold-parity audit
  shows the divergence set strictly **shrank** — a real golden source is gone
  (`image:pause.png[title=Pause pass:p[{abc +\ndef}] Resume]`) and no new divergence appeared.
  Coverage stays diff-neutral, and as with every prep piece before it, nothing further is wired in.

  Three shapes are left where they were, each pinned. As in the target's own increment, one of them
  runs the **safe** way rather than the byte-parity way: the renderer's dangerous-scheme check reads
  the `link=` attribute, which now carries the *restored* bytes, so
  `image:x.png[Alt,link=++javascript:alert(1)++]` renders without its anchor where the string
  pipeline checked the sentinel its own parse put there, passed it, and let the restore complete a
  live link. A restored `"` is escaped by the fold's
  `encode_attribute_value` where the string pipeline encoded its quote-free sentinel and spliced
  the raw quote into the finished `alt="…"`, closing the attribute — the tree's is the well-formed
  reading, and the same one the two link families' restores already take for a target. And an
  author's own sentinel-shaped bytes survive the tree's restore where the string pipeline's
  `replace_all` over the *finished* string rewrites them too — its own wart, and the reading the
  sibling target test already pins.

  What still defers of the bracket half is its other three captures — the `link:`/`mailto:` and
  auto-link families' attribute-list display texts, which keep the opaque-piece gate inside
  [`text_attrlist`](../../parser/src/content/inline_builder/macros/links.rs) — plus the image
  family's STEM target and bracket, and the keeps: the cross-reference family's pre-restore target,
  and the well-formed readings these five increments pinned.

  *Step 6 prep landed as (a masked construct inside a link's **display-text attribute list** — the
  bracket half's other three captures):* the increment above took the bracket half's first family
  and named the rest of it in one phrase — "the `link:`/`mailto:` and auto-link families'
  attribute-list display texts". Those three captures are three call sites of a *single* function,
  [`text_attrlist`](../../parser/src/content/inline_builder/macros/links.rs), so one gate closes
  all three: the `link:` macro's `=` list (roles / id / title / window), a `mailto:`'s `,` list
  (its subject and body), and the auto-link / formal-URL family's own `=` list.

  The *order* is the same one the image bracket found, and the machinery is now literally shared.
  [`tokened_bracket`](../../parser/src/content/inline_builder/macros/image.rs) becomes
  `pub(in inline_builder)` and takes the
  [`Restorable`](../../parser/src/content/inline_builder/macros/image.rs) kinds its caller admits,
  so the two families cannot disagree about what a token may stand for: the image bracket keeps
  `Passthrough` (its `web_path`-bound `fallback=`), while a display-text list passes
  `PassthroughOrStem`, having no re-processing of its own — a `Stem` is admitted here on its first
  bracket-half increment rather than waiting for one of its own. It returns
  [`MaskedPiece`](../../parser/src/content/inline_builder/macros/image.rs) — the node *and* its
  body, produced by the one `node_is_restorable`/`restorable_body` chain — because the two callers
  need different halves of it, which is the whole novelty of this increment.

  What is new is the **sink**. The image bracket's restore ends in a string: each body is spliced
  into the parsed attribute *values*, which the fold then emits into `alt="…"`. A link's display
  text ends in the node's **children**, and there the honest restore is the node itself.
  [`restored_value_children`](../../parser/src/content/inline_builder/macros/links.rs) re-splits
  the parsed positional on the very tokens
  [`tokened_bracket`](../../parser/src/content/inline_builder/macros/image.rs) placed —
  index-keyed and left to right, as `Passthroughs::restore_to` is, so a token the split discarded
  is simply not found and the ones after it splice by their own index — handing each run to
  [`escaped_value_children`](../../parser/src/content/inline_builder/macros/mod.rs) and each token
  to the masked node, cloned whole. That is exactly what a *sliced* display text has always done
  with an opaque piece (`emit_range` clones the node), so the two paths now agree; splicing the
  restored **bytes** instead would have the fold escape them a second time, turning
  `link:x[++<b>a</b>++,role=hl]`'s golden `&lt;b&gt;` into `&amp;lt;b&amp;gt;`. The list's own
  values still take the byte restore
  ([`Attrlist::into_owned_restoring`](../../parser/src/attributes/attrlist.rs)), so `role=`,
  `title=`, and `id=` reach the fold as the string pipeline's restore leaves them.

  One shape defers, and it is the family's own analogue of the image bracket's `web_path`: a
  `mailto:`'s **subject or body**. Those two positionals are read *before* the restore —
  `encode_uri_component` folds them into the `href` — and the string pipeline percent-encodes its
  own sentinel there (`?subject=%C2%960%C2%97`), which `Passthroughs::restore_to` then cannot find
  in the finished attribute. Its golden *leaks* the encoded sentinel, so there is nothing to
  reproduce: this is the cross-reference family's pre-restore boundary, kept for the same reason.
  What is new is that it is drawn per **slot** rather than per family — `text_attrlist` takes the
  positional numbers its caller reads early, and only a token that lands in one of them defers the
  match — so `mailto:x@y.com[++Tom, Jr++ R,Subject]`, whose masked piece is in the display text
  beside a plain subject, still reaches parity.

  What reaches parity is the whole display-text vocabulary over both masked kinds: the text as one
  piece of the list at either edge, several in one text, and the whole text; the split invariant in
  both spellings (a body carrying the `,` or the `=` the split reads); a body inside a named value,
  a quoted value, and both halves of one list at once; a token the split discards; the shorthand
  items after a token (`link:x[++abc++#myid.myrole,role=hl]` keeps its `myid` and `myrole`); the
  `^` window suffix and the `\]` unescape past a token; all three STEM notations and an explicit
  substitution list; a body that is live markup, an attribute reference neither pipeline expands,
  and the escaped and restored bytes the value already admitted around it; the `mailto:` and
  auto-link spellings, the angle form that keeps its `&lt;`, a restored target beside a restored
  text; and the whole set inside a rendered span, twice in one flow, in a footnote's extracted
  text, and end to end through the real parse path. Re-running the corpus-wide fold-parity audit
  shows the divergence set **unchanged** — no golden source writes an attribute list *and* a
  masked construct in one display text — and no new divergence appeared. Coverage stays
  diff-neutral, and
  as with every prep piece before it, nothing further is wired in.

  Three shapes are left where they were, each pinned, and one of them again runs the **safe** way
  rather than the byte-parity way. A restored `"` in a `title=` is escaped by the fold's
  `encode_attribute_value` where the string pipeline encoded its quote-free sentinel and spliced
  the raw quote into the finished `title="…"` — the same well-formed reading the image bracket's
  `alt` takes (the `id=` slot, emitted unescaped in both, stays byte-identical). A `window=` or
  `opts=` is a value the renderer *decides* on rather than emits, and it now reads the restored
  bytes where the string pipeline tested its own sentinel and found neither `_blank` nor
  `nofollow`: `link:x[T,window=++_blank++]` gains the `rel="noopener"` hardening its golden
  omits — the same class as the image family's `link=` dangerous-scheme check, and the same
  direction. And
  an author's own sentinel-shaped bytes survive this restore where the string pipeline's
  `replace_all` over the *finished* string rewrites them too, the wart the image bracket's own
  sibling test already pins.

  With this the **bracket half is closed for the two link families**, and the restore-the-value
  class as a whole is down to one family: the `image:`/`icon:` STEM target and its bracket's
  `fallback=`, both blocked on the same fold-time `web_path` (closing them means keeping the
  restore out of `web_path`'s way, not widening a gate). Beyond that only the keeps remain: the
  cross-reference family's pre-restore target, a `mailto:`'s subject and body for the same
  pre-restore reason, and the well-formed readings these six increments pinned.

  *Step 6 prep landed as (a masked STEM expression in the `image:`/`icon:` family — the
  restore-the-value class closed):* the class's last family, and the increment every note above
  deferred in the same words — "the STEM target and its bracket's `fallback=`, both blocked on the
  same fold-time `web_path`". Closing it meant exactly what those words prescribed: **keeping the
  restore out of `web_path`'s way**, not widening a gate. The string pipeline's resolver never
  sees a restored byte — its `web_path` runs while the masked construct is still the
  `\u{96}`*n*`\u{97}` sentinel, an opaque run carrying no space to percent-encode, no backslash to
  posixify (the platform-dependence that made this family defer), and no `/` or `.` for the
  segment arithmetic to read — and `Passthroughs::restore_to` splices the body into the finished
  `src` afterwards. The fold now reproduces that order at the same seam: an
  [`Image`](../../parser/src/inlines/image.rs) node records which byte ranges of its restored
  target came from a masked body (`restored_target_ranges` — the record
  [`restore_masked_passthroughs`](../../parser/src/content/inline_builder/macros/links.rs) already
  produces in the course of splicing, now kept), and the built-in renderer re-masks exactly those
  ranges into index-keyed sentinel-shaped tokens, resolves, and splices the bodies back
  ([`mask_restored_ranges`](../../parser/src/parser/inline_substitution_renderer.rs) /
  [`splice_restored_bodies`](../../parser/src/parser/inline_substitution_renderer.rs) — index-keyed
  as `restore_to` is, so a token the resolver's `..` arithmetic consumes is simply dropped, in
  both pipelines). The bracket's own two `web_path`-bound values ride the same mechanism through
  the attribute list:
  [`ElementAttribute::into_owned_restoring`](../../parser/src/attributes/element_attribute.rs)
  records each splice's range on the attribute, and an interactive SVG's `fallback=` and a
  macro-level `imagesdir=` resolve over them masked — the directory's and the target's mask sets
  sharing one token numbering where the resolver joins the two into one path.

  With `web_path` out of the way, the gate split `Restorable` named is **deleted rather than
  relaxed**: every restoring site admits both masked kinds
  ([`node_is_restorable`](../../parser/src/content/inline_builder/macros/image.rs), now total over
  the pair), `widen_masked_passthroughs` becomes
  [`widen_masked_pieces`](../../parser/src/content/inline_builder/macros/image.rs) (the image
  target's two-character class needs a STEM widened exactly as it needed a passthrough), and
  [`masked_default_alt`](../../parser/src/content/inline_builder/macros/image.rs) takes a `Stem`
  piece's body from [`restorable_body`](../../parser/src/content/inline_builder/macros/image.rs) —
  the fold's own bytes, so the two directions cannot drift.

  What reaches parity is the family's whole masked vocabulary: the wholly- and partially-masked
  target in both macro forms and all three notations, the default-alt arithmetic over the masked
  bytes, both masked kinds in one target and in one bracket, the split invariant (a `,` or `=`
  inside a rendered body), named values, the positional slots, a role, target and bracket both
  masked, a macro-level `imagesdir=` (masked directory, masked target, and both at once), an
  interactive SVG's `fallback=` in all three masked spellings, a token dropped whole by the `..`
  arithmetic, the shapes inside a rendered span, two in one flow, escapes, and end to end through
  the real parse path; the staged side effect registers the honest restored target. The move also
  closes a divergence this class had already pinned: the restored **space** the fold used to
  percent-encode into the `src` (`image:pass:[My Documents/chart.png][]`) now stays out of
  `web_path`'s way exactly as the sentinel did, so that test is a parity test now. A platform
  pair pins the reason the family deferred this for so long: the fold is byte-identical under
  either separator, and equal to the golden, whose own resolver only ever saw the sentinel.
  Re-running the corpus-wide fold-parity audit shows the divergence set **unchanged** — no golden
  source writes an image over a STEM expression — and no new divergence appeared. Coverage stays
  diff-neutral, and as with every prep piece before it, nothing further is wired in.

  With this the **restore-the-value class is closed**: every value any family computes, or parses
  out of a bracket, admits both masked kinds. What remains are only the keeps: the
  cross-reference family's pre-restore target and a `mailto:`'s subject and body (each golden
  leaking the pipeline's own sentinel into an `href` no restore then reaches), and the
  well-formed/safe readings the class's increments pinned — a restored `"` escaped by the fold, a
  `window=`/`opts=`/`link=` the renderer *decides* on reading restored bytes, and an author's own
  sentinel-shaped bytes surviving the tree's per-token restore.

  *Step 6 prep landed as (a computed value classified by where the escaping step sits):* the one
  divergence the class above left behind that was not a *keep* — an author's own `&lt;` in a
  **computed** value under an effective order that never escaped, which
  [`escaped_value_children`](../../parser/src/content/inline_builder/macros/mod.rs) unwound one
  level too far — closed exactly as its own note prescribed: "by where the escaping step actually
  sits", which needed the effective order threaded down to each family. A computed value is the one
  thing the macros step reads as *bytes* rather than carrying structurally (it comes back from an
  [`Attrlist`](../../parser/src/attributes/attrlist.rs) parse, so there is no range of nodes to
  rebuild it from), and an attribute list is parsed under *any* order that runs `Macros` — so both
  readings of `&lt;` are reachable, and picking one by assumption was the bug.

  A new [`ComputedSpecials`](../../parser/src/content/inline_builder/macros/mod.rs) carries the
  decision, made in [`build_for_group`](../../parser/src/content/inline_builder/mod.rs) where the
  order is in hand — the same seam, and the same shape, as
  [`SplicedSpecials`](../../parser/src/content/inline_builder/attribute_refs.rs) already uses for
  the attribute-references step — and
  [`computed_value_children`](../../parser/src/content/inline_builder/macros/mod.rs) dispatches to
  one of two halves: the existing trichotomy unwind when the escaping step has already run, and a
  new [`unescaped_value_children`](../../parser/src/content/inline_builder/special_chars.rs) when
  it has not, which reaches for
  [`split_text`](../../parser/src/content/inline_builder/special_chars.rs) — the very splitter
  [`classify_unescaped_specials`](../../parser/src/content/inline_builder/special_chars.rs) uses
  over the finished tree — so the two cannot drift on what a literal special is worth. The
  condition is the step's **position**, not its presence, which is what makes the second half cover
  an order that escapes *after* `Macros` too: there the final classification pass never runs, and
  `flatten_prior_markup` folds the node's markup before the escaping step splits the result, so the
  value has to be classified here or not at all. That is a real gain rather than a hypothetical
  one — under `subs=attributes,macros,specialcharacters` the cross-reference family (the one
  family the string pipeline is still holding as a deferred placeholder when the escaping step
  runs, so `flatten_prior_markup` leaves it alone) now reaches parity in **both** spellings, where
  a bare `<` had diverged.

  What reaches parity is the four-cell truth table — a `<` and a `&lt;` in a computed value,
  against an order that escapes first and one that never escapes — over all three families that
  compute one (the `link:`/`mailto:` macro's `=` list, the auto-link / formal-URL family's, and the
  cross-reference macro's), plus the same three fixtures folded into the never-escapes group-parity
  corpus beside a restored entity and a masked passthrough in the same value. Threading changed
  fourteen signatures and no node kind, gate, or builder body. Re-running the corpus-wide
  fold-parity audit shows the three fixtures **gone** from the divergence set and no new
  divergence; coverage is diff-neutral, with one consequence worth recording: the escaped half's
  bare-`&` arm, which the never-escapes corpus used to be the only thing exercising, is now
  unreachable through any order — every `&` in a
  level's match string belongs to a `CharRef` leaf that opens a class, and a *literal* one is a
  `Raw` leaf the match string stands in as an opaque placeholder — so it is kept as the scan's
  totality arm and pinned by a direct unit test instead. As with every prep piece before it,
  nothing further is wired in.

  With this, every §3.4.1 classification the builder makes is decided by where the steps actually
  sit rather than by where a fragment came from. What remains of the audit's own remainder are the
  boundary-class halves the sibling increments named — a macro body class wanting more than one
  presented character, the closing character, and the character-replacements step's
  consume-across-levels case — and the keeps.

  *Step 6 prep landed as (a trailing-punctuation strip that cuts an escaped special in half):*
  the auto-link family's own last-named gap — "the one form this family's escaped-special lift
  does not reach", pinned since the lift landed as
  `a_bare_url_whose_trailing_strip_would_split_a_special_is_a_documented_divergence` and reproduced
  by two of the language description's own auto-link fixtures. A bare URL ending in a literal `&`,
  `<`, or `>` reaches the macros step as that special's own entity, whose final `;` satisfies the
  trailing-punctuation strip: the string replacer splits the entity happily (`href` ending `&amp`,
  a literal `;` left after the link), while the tree's boundary fell *inside* a
  [`CharRef`](../../parser/src/inlines/inline_node.rs) leaf and left the whole match literal.

  Closing it needed one distinction rather than a new mechanism: a match boundary is answerable
  exactly where a piece's **match-string bytes are the bytes its own fold emits**, which is true of
  the three `CharRef` leaves and of nothing else the module stands in as a placeholder. There
  either half *is* those bytes, so
  [`emit_range`](../../parser/src/content/inline_builder/quotes.rs) now cuts such a piece into two
  [`Raw`](../../parser/src/inlines/inline_node.rs) leaves — each folding verbatim, so every
  partition of the entity folds to the entity — while every other atomic piece, standing in for
  markup that exists only at fold time, still clones whole. Neither half has an honest `'src` slice
  of its own (the source holds one character, or `(C)`, where the match string holds an entity), so
  both keep the leaf's whole `location`, design §4.4's coarse fallback. The classification is one
  new [`charref_entity`](../../parser/src/content/inline_builder/quotes.rs), which
  [`atomic_piece_is_recoverable`](../../parser/src/content/inline_builder/macros/image.rs) — the
  `range_has_no_opaque_piece` predicate that used to spell the same three arms out a second time —
  now delegates to, so the gate and the bytes it admits cannot disagree;
  `charref_entity_matches_the_match_strings_own_bytes` pins both against
  [`build_match_string`](../../parser/src/content/inline_builder/quotes.rs)' own arms.

  The family's gate is then **deleted rather than relaxed**: the bytes the strip cuts off are a
  literal `;`, `:`, or `)`, which no opaque piece can supply (the module stands one in as a single
  non-ASCII placeholder), so the only piece the boundary can fall inside is a `Text` run or a
  `CharRef` leaf and both are now cut. What reaches parity is all three escaped specials at a bare
  URL's end, a restored entity (`&copy;`) and a typographic replacement (`(C)`) — the class's
  other two leaves, split by the same cut — the two golden fixtures this closes, the strip's
  two-byte `);` form over a leaf the link keeps whole, in flow, doubled, inside a rendered span,
  escaped, under `hide-uri-scheme` (whose scheme count reaches past a split leaf as it does past
  a whole one), the bracketed and angle spellings that apply no strip at all, and end to end
  through the real parse path. Re-running the corpus-wide fold-parity audit shows those two
  fixtures **gone** from the divergence set and no new divergence; coverage is diff-neutral, and
  as with every prep piece before it, nothing further is wired in.

  What still defers is unchanged: the boundary-class halves the sibling increments named — a macro
  body class wanting more than one presented character, the closing character, and the
  character-replacements step's consume-across-levels case — and the keeps.

  *Step 6 prep landed as (an escaped attribute-list bracket, one match's source doing two
  things):* with the boundary and restore-the-value classes closed down to their keeps, re-running
  the corpus-wide fold-parity audit and classifying what remains by *source* leaves the
  passthrough-extraction pass's own last named deferral — an **escaped bracket** ahead of a
  delimited passthrough (`\[attrs]++text++`), pinned since step 5d part 2 landed as
  `an_escaped_attrlist_bracket_is_a_documented_divergence` and exercised by
  [`Passthroughs`](../../parser/src/content/passthroughs.rs)' own golden fixture. The string
  replacer's [`handle_quoted_text`](../../parser/src/content/passthroughs.rs) does *two* things
  there — writes the bracket back as a literal `[attrs]` prefix with its backslash dropped, then
  stores the delimited text as an **ordinary** passthrough, its attribute list discarded rather
  than carried — where
  [`find_passthrough_matches`](../../parser/src/content/inline_builder/passthrough_step.rs) could
  express one or the other and so left the whole construct literal.

  Closing it needed no new mechanism at all, which is the increment's own finding: the shape its
  divergence note called "a kept-literal-prefix-with-one-dropped-char, plus a node for the
  remainder, a shape neither
  [`MacroMatchKind`](../../parser/src/content/inline_builder/macros/mod.rs) variant expresses" is
  not one match wanting a third variant but **two adjacent matches** —
  an [`Unescape`](../../parser/src/content/inline_builder/macros/mod.rs) over the bracket, then a
  [`Node`](../../parser/src/content/inline_builder/macros/mod.rs) over the delimited remainder —
  and [`rebuild_macro_level`](../../parser/src/content/inline_builder/macros/mod.rs) already
  composes any two adjacent matches, gap by gap, without knowing they came from one regex capture.
  The node the second match carries is
  [`build_passthrough_node`](../../parser/src/content/inline_builder/passthrough_step.rs)' — the
  unattrlisted builder every bare `+++`/`++`/`$$` form already uses — reached with the *delimited*
  sub-range rather than the whole match, so the discarded attribute list is discarded by
  construction: there is no `Styled` wrapper to suppress and no `x-` marker to not honor, because
  the branch that reads them is never entered. Nothing else in the module changes, and no variant,
  gate, or signature moves.

  What reaches parity is every boundary an escaped bracket can precede (`+++`, `++`, `$$`), the
  `x-` marker whose monospace-and-`Normal`-subs treatment the escape *removes*, a body carrying
  markup and specials (escaped for `++`/`$$`, raw for `+++`, with quotes never running over either),
  the escaped form beside its unescaped twin in one flow, a kept prefix carrying a special
  character and one carrying quote syntax — both substituted by the later steps exactly as the
  string pipeline substitutes its own literal `[attrs]` — the construct in flow and spanning a
  newline, and the same under the never-escapes group-parity orders, where whether the prefix's
  `<` is escaped is the *order's* decision while the remainder stays opaque either way. The
  formerly pinned divergence becomes a parity test that also asserts the shape: a `Text` prefix and
  a plain [`Raw`](../../parser/src/inlines/inline_node.rs) leaf, never the `Styled` span the
  unescaped spelling builds. Fixtures join the whole-pipeline broad sweep and combined-constructs
  corpus, the group-parity corpus, and — unlike the boundary-class increments, whose shapes a
  [`RecordingRenderer`](../../parser/src/content/inline_tree.rs)'s marker perturbs — the
  **structural recorder cross-check**, which reads them normally (the marker sits outside the
  markup it wraps, and nothing here reads a neighbour's boundary character); a whole-document test
  drives both shapes end to end through the real parse path. Re-running the corpus-wide fold-parity
  audit shows the `Passthroughs` fixture **gone** from the divergence set and no new divergence;
  coverage is diff-neutral, and as with every prep piece before it, nothing further is wired in.

  What still defers in this pass is its other named deferral, the **prohibited-prefix** retry the
  string replacer works around by hand for the two bare attribute-list-prefixed forms
  (`index:[attrs]+text+`, ``\[x-]`text` ``), which keeps its own divergence test — and which is
  where writing *both* escapes at once (`\[attrs]\++text++`) lands, the delimiter escape winning
  the branch and the literal `++text++` it leaves behind sitting behind exactly the `\` that
  second pass declines. Beyond that the remainder is unchanged: the boundary-class halves the
  sibling increments named — a macro body class wanting more than one presented character, the
  closing character, and the character-replacements step's consume-across-levels case — and the
  keeps.

  *Step 6 prep landed as (the passthrough pass's prohibited-prefix retry, the last of its own two
  named deferrals):* the sibling increment's closing sentence, taken in order. `INLINE_PASS`'
  two attribute-list-prefixed options open with `\b{start-half}`, which does not by itself exclude
  the `\`/`:`/`;` prefix Asciidoctor's own pattern rejects with a lookbehind Rust's regex engine
  does not have, so
  [`InlinePassReplacer`](../../parser/src/content/passthroughs.rs) answers it at *run* time: it
  writes the rejected match's first character back verbatim and runs `INLINE_PASS.replace_all`
  again over the rest of that same match. The tree had only the first half of that — it dropped
  such a match — and the increment's finding is that the second half is the load-bearing one. The
  retry is not a formality that re-confirms a rejection: it routinely recognizes a *different,
  shorter* construct the leading `[` was hiding, most often the bare unconstrained form over the
  very same body (`index:[attrs]+text+` → a literal `[attrs]` and an **ordinary** passthrough over
  `text`, so no `Styled` span and no `x-` monospace), which is content the tree was leaving
  entirely literal.

  Reproducing it needed no new mechanism either, only a shape change to the scan:
  `find_bare_attrlisted_matches`' capture loop moves into a
  [`collect_bare_pass_matches`](../../parser/src/content/inline_builder/passthrough_step.rs) that
  scans a *sub-range* of the level's match string and appends to a shared match list, and the
  prohibited case calls it again over the same match minus its leading `[` instead of skipping.
  Because both options open with `\[` (nothing in either alternative precedes the bracket), the
  character split off is always that one ASCII byte, and the `[` no match now covers is emitted by
  [`rebuild_macro_level`](../../parser/src/content/inline_builder/macros/mod.rs) as an ordinary
  gap — the same composition the escaped-bracket increment leaned on, reached from the other
  direction. Recursion terminates because each retry region is strictly shorter than the match
  that produced it. Scanning a slice is what the string replacer does too (its retry sees `rem`,
  not the level), so word boundaries and `^` are computed against the same text in both; the one
  cost is that a capture's offsets are then relative to the slice, so an `offset` is threaded into
  the two node builders that read them and rebased there — pinned by a test that asserts the built
  leaf's `location` is the body's own source bytes rather than a range shifted left by the retry's
  start. The prefix test itself keeps reading the level string's own preceding byte where the
  replacer reads its *output* so far, the one-byte approximation this scan has always made; at the
  start of a retry region it is exact by construction, since the byte before is the `[` the retry
  just split off.

  What reaches parity is all three prohibited prefixes, both options (the backtick one finds
  nothing to retry — `` x-]`text` `` matches neither an attribute-list option, whose bracket the
  retry just consumed, nor the bare form, which needs a `+` — so the construct falls to the later
  quotes step, exactly as the replacer's own empty retry leaves it), the `x-` marker the retry's
  ordinary form drops, a body carrying specials and quote syntax, an attribute list carrying a
  special, in flow, spanning a newline, twice in one flow, beside an unprefixed twin that still
  builds its `Styled` span, the delimiter escapes the retry's own second scan honors
  (`index:[attrs]\+text+`, and the two-backslash form it declines), and — the shape the sibling
  increment named — writing **both** of this pass's escapes at once (`\[attrs]\++text++`), where
  the delimiter escape wins the first pass's branch and the literal `++text++` it leaves behind
  sits behind exactly the `\` this second pass declines, so only the retry reaches the `+text+`
  inside it. Fixtures join the whole-pipeline broad sweep and combined-constructs corpus, the
  group-parity corpus, and the structural recorder cross-check; a whole-document test drives both
  shapes end to end through the real parse path. Re-running the corpus-wide fold-parity audit
  shows two of the `asciidoctor` port's own golden fixtures **gone** from the divergence set —
  `should_support_constrained_passthrough_in_monospace_span_preceded_by_escaped_boxed_attrlist_with_transitional_role`
  and its `*foo*` twin, where the retry reaches a `+bar+` nested inside an escaped `` [x-]`…` ``
  — and no new divergence; coverage is diff-neutral, and as with every prep piece before it,
  nothing further is wired in.

  What still defers is no longer anything this pass named for itself: with both its deferrals
  closed, the remainder is the boundary-class halves the sibling increments named — a macro body
  class wanting more than one presented character, the closing character, and the
  character-replacements step's consume-across-levels case — plus the keeps, and the
  bare-attrlisted body whose content the *first* pass already recognizes
  (`[method x-]+pass:[<b>]+`), which is the module's own `range_is_verbatim` boundary rather than
  a gap in this pass's recognition.

  *Step 6 prep landed as (an `indexterm2:[…]` attribute list, and the deferral that was waiting
  on nothing):* with the passthrough-extraction pass's own two deferrals closed, re-running the
  corpus-wide fold-parity audit and classifying what remains by *source* leaves the **index-term**
  family holding the largest golden-exercised share of it — and the one piece of that share whose
  own note named a blocker: an `indexterm2:[…]` whose argument carries an `=`, deferred at step
  4b(ii) part 4b "until the node can hold an `Attrlist<'src>`, as the link/xref macros defer the
  same", and pinned since by `an_indexterm2_attribute_list_is_a_documented_divergence`. Two of the
  `asciidoctor` port's own golden fixtures spell it —
  `indexterm2:[Flash,see=HTML 5] and indexterm2:[HTML 5,see-also="CSS 3, SVG"] done.` and
  `Only named indexterm2:[see=HTML 5] here.` — where the tree was leaving the whole macro literal.

  The increment's finding is that the blocker had lapsed: the node needs no attribute list. An
  `=` in the argument makes it an **attribute list whose first positional attribute is the shown
  term** (`indexterm2:[Coffee, region=Kona]` shows `Coffee`), and everything else the list holds —
  a `see`, a `see-also`, a `region` — names an entry in an index this crate's HTML backend does
  not build, so it reaches the flow through nothing. The link and cross-reference families capture
  their own [`Attrlist<'src>`](../../parser/src/attributes/attrlist.rs) because a role, an id, or a
  `window=` there changes what the fold emits; an index term's whole render surface is
  [`IndexTermRenderParams`](../../parser/src/parser/inline_substitution_renderer.rs), which carries
  the shown term and nothing else. So a new
  [`shown_macro_term`](../../parser/src/content/inline_builder/macros/indexterm.rs) *consumes* the
  list where [`InlineIndextermReplacer`](../../parser/src/content/macros.rs) consumes it — the
  same `Attrlist::parse` over the same normalized copy, the same `nth_attribute(1)`, the same
  fall back to the whole argument when the list has no positional attribute at all — and the
  [`IndexTerm`](../../parser/src/inlines/index_term.rs) node goes on holding the one
  already-substituted shown text every other visible spelling gives it. No node field, variant, or
  gate moves; only the level's `parser` is threaded down to the family, which every sibling family
  already takes.

  The argument is read from this level's escaped match string, which holds exactly what the string
  pipeline's flat haystack holds at that position, so the two parse the same bytes — and the
  family's *other* deferral is untouched and still decides first: a shown term crossing an opaque
  span is unreconstructable from that string, so `indexterm2:[*bold* term,region=Kona]` stays
  literal and keeps a divergence test, now written against the attribute-list spelling too. The
  shorthand spelling has no attribute list to parse (the string replacer strips its ` >> ` /
  ` &> ` clause instead, which the tree already mirrored), so it is unchanged.

  What reaches parity is both golden spellings, a list with no positional attribute (shown
  verbatim, `=` included), a quoted positional attribute whose own comma is not a separator, an
  empty first positional attribute, a special character in either half (an entity in both pipelines
  by macro time, so the list is parsed from the same escaped bytes), a typographic replacement and
  a restored entity in the shown half, the macro in flow, twice in one flow, inside a rendered
  span, spanning a newline that `normalize_index_text` collapses before either side parses, the
  escaped form that still drops its backslash, the concealed spelling that ignores its argument
  entirely, an attribute list arriving from an **expanded value** in either half, and the
  never-escapes group-parity orders, where what the order decides is only which bytes the list is
  parsed from. The formerly pinned divergence becomes a parity test that also asserts the shape:
  one `IndexTerm` carrying `Coffee` and the whole macro's location, never an attribute list.
  Fixtures join the whole-pipeline broad sweep and combined-constructs corpus, the group-parity
  corpus, and the structural recorder cross-check; a whole-document test drives the shape end to
  end through the real parse path, in a paragraph and in a section heading's own title. Re-running
  the corpus-wide fold-parity audit shows both golden fixtures **gone** from the divergence set and
  no new divergence; coverage is diff-neutral, and as with every prep piece before it, nothing
  further is wired in.

  What still defers in this family is what its own note named beside this one: a **visible term
  crossing a rendered span** (both spellings), which is the module-wide `range_is_verbatim`
  boundary rather than anything this family owns, and the one escaped paren-wrapped shorthand the
  string replacer re-renders (`\(((x)))` → `(x)`). Beyond the family the remainder is unchanged:
  the boundary-class halves the sibling increments named — a macro body class wanting more than
  one presented character, the closing character, and the character-replacements step's
  consume-across-levels case — plus the keeps, and the bare-attrlisted body whose content the
  passthrough pass's first scan already recognizes (`[method x-]+pass:[<b>]+`).

  *Step 6 prep landed as (the escaped paren-wrapped shorthand, the index-term family's other named
  deferral):* the sibling increment's closing sentence, taken in order — the half of what it named
  that this family does own, the other half being the module-wide `range_is_verbatim` boundary. An
  **escaped** index-term shorthand drops its backslash and keeps the rest literal, which is what the
  builder did for every spelling of it; the string replacer has one exception, and its own branch
  says why: *an escaped concealed term still processes a nested flow term*. Where the escaped
  match's `encl_text` is itself **paren-wrapped**
  ([`InlineIndextermReplacer`](../../parser/src/content/macros.rs)'s
  `encl_text.starts_with('(') && encl_text.ends_with(')')`), the replacer strips those wrapping
  parentheses off and renders what is left as a *visible* term between two literal parens, so
  `\(((x)))` collapses to `(x)` rather than staying literal. The tree left the whole match literal
  and pinned the difference — the shape Asciidoctor's own
  `should only escape enclosing brackets if concealed index term is preceded by a backslash` spells,
  which is where the corpus-wide fold-parity audit's remainder was still holding it.

  Closing it needed no new mechanism, only the observation that this is **one match's source doing
  two things** — the shape the escaped-attribute-list bracket increment already answered — so it
  becomes a *pair* of matches that
  [`rebuild_macro_level`](../../parser/src/content/inline_builder/macros/mod.rs) composes as it
  composes any two adjacent ones: an
  [`Unescape`](../../parser/src/content/inline_builder/macros/mod.rs) whose match **is** the
  backslash it drops (so it emits nothing, exactly what the replacer does with that byte), then a
  [`Node`](../../parser/src/content/inline_builder/macros/mod.rs) over the rest whose `consumed`
  sub-range stops one byte inside each end — the same kept-parenthesis
  narrowing the unescaped spellings already use for a single adjacent paren, applied to both ends at
  once. Neither `MacroMatchKind` variant, nor the node, nor any gate or signature moves; the nested
  term itself takes the visible branch's whole existing arithmetic (`normalize_index_text` then
  `strip_see_and_seealso`), since it *is* that branch. The look-ahead bookkeeping falls out too: the
  pair's `Node` half carries the `is_skip` the absorbed parens imply and a `rendered_nonempty` that
  is unconditionally true — two kept parentheses are output whatever the term renders to — so a
  concealed term sitting beside one is consumed rather than left literal by the
  [`Cow::Borrowed`](../../parser/src/internal/regex.rs) no-op the level would otherwise be.

  The family's *other* deferral is untouched and still decides first: the nested term is a shown
  one, so a term crossing an opaque span is unreconstructable from this level's escaped string and
  `\(((*bold* term)))` keeps the plain escaped shape with its own divergence test. What reaches
  parity is the shape itself alone and in flow, a `see` / `see-also` clause stripped to its primary,
  a newline collapsed and a term trimmed, a special character and a restored entity and a
  typographic replacement in the shown text, an empty nested term, the trailing-paren absorption
  running first so the wrapper's right half is a paren the closing pair absorbed
  (`\(((x))))`, `\((((x))))`), the literal-staying twins beside it (`\((x))`, `\(((x))`,
  `\((x)))`, `\indexterm:[x]`), twice in one flow, beside a rendered span, inside one, and the
  nested term arriving from an **expanded value**. The formerly pinned divergence becomes a parity
  test that also asserts the shape: a literal `(`, one visible `IndexTerm`, a literal `)`, and the
  node's location the match minus the backslash the pair's first match drops. Fixtures join the
  whole-pipeline broad sweep and combined-constructs corpus, the group-parity corpus, and the
  structural recorder cross-check; a whole-document test drives the shape end to end through the
  real parse path, in a paragraph and in a section heading's own title. Re-running the corpus-wide
  fold-parity audit shows that golden fixture **gone** from the divergence set and no new
  divergence; coverage is diff-neutral, and as with every prep piece before it, nothing further is
  wired in.

  What still defers in this family is now only the half that was never its own: a **visible term
  crossing a rendered span**, in every spelling — shorthand, macro, attribute list, and this
  increment's nested one — which is the module-wide `range_is_verbatim` boundary. Beyond the
  family the remainder is unchanged: the boundary-class halves the sibling increments named — a
  macro body class wanting more than one presented character, the closing character, and the
  character-replacements step's consume-across-levels case — plus the keeps, and the
  bare-attrlisted body whose content the passthrough pass's first scan already recognizes
  (`[method x-]+pass:[<b>]+`).

  *Step 6 prep landed as (a visible term enclosing a rendered span — the index-term
  family's last deferral):* the half the increment above named as *never its own*, and the
  first lift in this family that changes what an [`IndexTerm`](../../parser/src/inlines/index_term.rs)
  node **holds** rather than which ranges a gate admits. A visible term shows its text in the
  flow, and the string replacer reads that text straight out of its own already-rendered
  haystack — where an earlier-recognized construct already stands as its markup — so
  `((*tiger*))` shows `<strong>tiger</strong>`. The builder's match string stands the same
  construct in as one opaque
  [`SPAN_PLACEHOLDER`](../../parser/src/content/inline_builder/quotes.rs), whose markup exists
  only at fold time, so every spelling of such a term was left literal and pinned by a
  divergence test — the last of the audit's remainder this family held, and the shape
  `The ((*tiger*)) (Panthera tigris) …` spells in the `asciidoc-lang` corpus.

  The finding is that the shown text needs no string at all. It is not a value this family
  *decides* anything from — every decision (`trim`, the ` >> ` / ` &> ` clause, the
  attribute-list `=`) is made over the bytes as matched — so it can be carried as the nodes it
  encloses and rendered at fold time, which is exactly the move the reference-bearing families
  made for their own display texts. [`IndexTerm`](../../parser/src/inlines/index_term.rs) gains
  a `children` field, the same one [`Ref`](../../parser/src/inlines/ref_node.rs) and
  [`Footnote`](../../parser/src/inlines/footnote.rs) already carry, and
  [`fold_index_term`](../../parser/src/content/inline_builder/fold.rs) folds it into the
  already-substituted string
  [`IndexTermRenderParams`](../../parser/src/parser/inline_substitution_renderer.rs) takes —
  the relationship [`fold_html`](../../parser/src/content/inline_builder/fold.rs)'s own
  `link_text` has to `Ref::children`, reached for the same reason. Because the fold uses the
  surrounding flow's renderer, the enclosed span is rendered by a *custom* backend too, where
  a term frozen at build time would have carried the built-in one's markup.

  What made it a range rather than a string is
  [`shown_term_range`](../../parser/src/content/inline_builder/macros/indexterm.rs), which
  re-expresses `normalize_index_text` and `strip_see_and_seealso` as a **narrowing**: `trim` is
  one already; the `\n` → ` ` collapse is length-preserving, so it moves no offset and becomes a
  one-space [`Text`](../../parser/src/inlines/inline_node.rs) node the emit writes; the `\]`
  unescape is a *gap* between two emitted ranges, the same structural unescape
  [`emit_range_unescaping_brackets`](../../parser/src/content/inline_builder/macros/mod.rs)
  performs for the reference families; and a `see` / `see-also` clause is a suffix, so the
  primary it leaves is a prefix that maps straight back. The two never combine — the macro form
  unescapes and does not strip, the shorthand strips and does not unescape — so neither
  perturbs the other. A shorthand that **absorbed** trailing parentheses is the one case whose
  `encl_text` is not contiguous in the match string (the matched `))` sits between the enclosed
  text and the absorbed run), so it is carried as the pair of ranges it really is
  ([`TermSource`](../../parser/src/content/inline_builder/macros/indexterm.rs)) and every
  narrowing maps back through it — which is what brings `((*a*)))`, `((*a*))))` and `(((*a*))`
  along with the plain spelling.

  The computed string stays for every term whose shown range holds no placeholder, so the
  spellings already at parity are byte-identical and `terms` goes on carrying them; the two
  fields are never both populated. One spelling still defers, and it is the one the sibling
  increments' `Attrlist<'src>` captures already drew: an argument that is **both** an attribute
  list and enclosing such a construct, whose shown term comes back from
  [`Attrlist::parse`](../../parser/src/attributes/attrlist.rs) rather than from a range, so
  there is nothing to carry — `indexterm2:[*bold* term,region=Kona]` keeps its divergence test.

  What reaches parity is the three visible spellings alone and in flow, the span at either edge
  and as the whole term, a monospace / nested / attributed / entity-rendered span, two spans in
  one term and two terms in one level, each normalization the shown term still performs (a
  collapsed newline, a trimmed edge, a stripped `see` and `see-also` clause, the macro form's
  escaped closing bracket), all three absorbed-paren shapes, the concealed and plain-escaped
  twins beside it, and the term inside a rendered span and beside other constructs. The three
  formerly pinned divergences become parity tests that also assert the shape: one `IndexTerm`
  with an empty `terms`, the enclosed span itself in `children`, and the node's whole-match
  location. Re-running the corpus-wide fold-parity audit shows that golden fixture **gone** from
  the divergence set and no new divergence; coverage is diff-neutral, and as with every prep
  piece before it, nothing further is wired in.

  With this the index-term family defers nothing but the `Attrlist` capture above. Beyond the
  family the remainder is unchanged: the boundary-class halves the sibling increments named — a
  macro body class wanting more than one presented character, the closing character, and the
  character-replacements step's consume-across-levels case — plus the keeps, and the
  bare-attrlisted body whose content the passthrough pass's first scan already recognizes
  (`[method x-]+pass:[<b>]+`).

  *Step 6 prep landed as (an attribute-list reference text enclosing a rendered span — the
  cross-reference family):* the capture the rendered-span class left behind. When that class was
  closed for the three reference-bearing families, each kept one shape: *a text carrying its own
  attribute list*, whose display text comes back from an
  [`Attrlist`](../../parser/src/attributes/attrlist.rs) **parse** rather than from a range of the
  match string — and a parsed value, the note said, is bytes, which a rendered span has none of
  until fold time. `xref:sec[*bold*,role=hl]` therefore stayed literal, one of the audit's
  remaining golden-exercised divergences.

  The finding is that the parse hands that value back to become the node's **children**, so it
  needs no bytes either — only a way to survive the split intact. That is exactly what the
  restore-the-value class already built for a masked passthrough:
  [`tokened_bracket`](../../parser/src/content/inline_builder/macros/image.rs) puts a bracket into
  the string pipeline's own *shape* before the parse, and
  [`restored_value_children`](../../parser/src/content/inline_builder/macros/mod.rs) re-splits the
  parsed value on its tokens and splices the masked **node** back in. The only thing that made a
  span different was the pairing of each token with a body, which
  [`Attrlist::into_owned_restoring`](../../parser/src/attributes/attrlist.rs) needs and a span
  cannot supply. A cross-reference needs no such pairing at all: a
  [`Ref`](../../parser/src/inlines/ref_node.rs)`{Xref}` carries `attrs: None`, and its `window` /
  `role` / `xrefstyle` are plain fields read straight off the parse. So a new
  [`tokened_text`](../../parser/src/content/inline_builder/macros/mod.rs) tokens **every** opaque
  piece — a rendered span, an earlier-recognized macro node, a masked passthrough or STEM alike —
  with no body to pair, and `restored_value_children`, lifted out of the link family and shared,
  splices each node into the children.

  The boundary that remains is drawn per **slot**, exactly as
  [`text_attrlist`](../../parser/src/content/inline_builder/macros/links.rs)'s own `pre_restore`
  draws it: a token reaching the `window=`, `role=`, or `xrefstyle=` this family reads as a
  *string* names markup that exists only at fold time where a string is needed, so that shape
  defers with its own divergence test. Deciding it means making the same tokened parse the builder
  makes, in the gate — which is where this family's contract puts every deferral ("both builders
  claim every shape they are handed"), and where it already re-derives the shorthand's own comma
  split for the same reason.

  One detail is this increment's own: `tokened_text` walks the level **piece by piece** rather
  than copying the gaps between the opaque ones, because a byte of the match string may belong to
  no piece at all — [`styled_sibling_boundaries`](../../parser/src/content/inline_builder/quotes.rs)
  wraps an opaque span's placeholder in the two characters its rendering presents to a neighbour,
  which exist for *recognition* and stand for markup the token already carries whole. Copying them
  would splice a stray `<` and `>` into the value beside the node.

  What reaches parity is the golden spelling alone and in flow, the span at either edge and in the
  middle of the text, each of the three named attributes beside it, other span kinds and two spans
  in one text, the recoverable pieces the text already carried (an escaped special, a restored
  entity, a typographic replacement) beside a span, a collapsed newline, an `=` the parse finds
  *incidental* (which falls through to the plain-text path that has carried an opaque piece all
  along), and — reached through the same tokening — a **masked passthrough** or **STEM expression**
  in such a text, which this family had deferred too. The formerly pinned divergence becomes a
  parity test that also asserts the shape: the enclosed span itself as a child, with the named
  attributes on the node's own fields. Fixtures join the whole-pipeline broad sweep and
  combined-constructs corpus, the group-parity corpus, and the structural recorder cross-check —
  which can now compare this form, the recorder having always recovered it structurally; a
  whole-document test drives the shape end to end through the real parse path, in a paragraph and
  in a section heading's own title. Coverage is diff-neutral, and nothing further is wired in.

  What still defers of this class is the **image** family's attribute list, the one capture with no
  display text to carry: its bracket's values are read as strings by `render_image` (a `title=`, an
  `alt=`), so a span there is the per-slot boundary above with every slot on the wrong side. Beyond
  it the remainder is unchanged: the boundary-class halves the sibling increments named — a macro
  body class wanting more than one presented character, the closing character, and the
  character-replacements step's consume-across-levels case — plus the keeps.

  *Next steps (each a transducer step, gated by the golden-HTML oracle §5.3):*
  1. ✅ Foundation + `SpecialCharacters`.
  2. ✅ `Quotes` → `Styled`, introducing nesting (`*a _b_ c*` becomes a tree, not a flat run).
  3. ✅ `CharacterReplacements` → `CharRef::Replacement`, and `PostReplacement` → `LineBreak`.
  4. ✅ `Macros`, sliced by construct family, each capturing the owned `Attrlist<'src>` it carries —
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
               destination) — needed only an existing, already-public type (`Ref` gains
               `derived: Option<DerivedReference>`), no consumer required to pin its shape.
             - ✅ an attribute-list text (`window`/`role` reuse `Ref`'s existing plain fields; a
               new `Ref::xrefstyle: Option<XrefStyle>` field carries the macro-level override) —
               needed no consumer to pin its shape, since `XrefRenderParams` itself takes plain
               fields, not a borrowed `Attrlist<'src>`.
         - ✅ **part 4.** `Anchor`, `IndexTerm`, `Footnote`, each its own sub-step (inline `Stem` is
           *not* a macros-step family — it is extracted at passthrough time, so it lands in step 5):
           - ✅ **part 4a.** inline anchors (`[[id]]` / `anchor:id[…]`, `INLINE_ANCHOR`) → `Anchor`.
           - ✅ **part 4b.** index terms (`((term))` / `(((primary, secondary)))` / `indexterm:[…]` /
             `indexterm2:[…]`, `INLINE_INDEXTERM`) → `IndexTerm`.
           - ✅ **part 4c.** footnotes (`footnote:[…]` / `footnote:id[…]` / `footnote:id[]`,
             `INLINE_FOOTNOTE_MACRO`) → `Footnote` — the last macro family.
  5. `AttributeReferences` (expanded-value splicing, §3.4.1), passthroughs (`Raw`), and
     `Callouts` — completing the vocabulary the recorder covers, each its own sub-step:
     - ✅ **5a.** Passthroughs, the delimited forms (`+++…+++`, `++…++`, `$$…$$`, bare
       `pass:[…]`) → `Raw`.
     - ✅ **5b.** `AttributeReferences` (expanded-value splicing, §3.4.1).
     - ✅ **5c.** `Callouts` (verbatim-group content — literal, listing, and source blocks).
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
     recognition **side effects** the string pipeline performs that the additive builder skips —
     registering an inline id (an attributed span's anchor) in the reference catalog so
     cross-references resolve (#1087), registering a link target (`register_link`) and an image
     target (`register_image`) in the asset catalog so `Document::catalog()` stays complete, and
     the dangerous-scheme substitution warning. Doing this at the cutover (rather than in the
     additive passes, which run *alongside* the authoritative string pipeline) is what avoids
     double-counting each registration.
     - ✅ **prep.** The image family's own two side effects (`register_image`, the `link=`
       dangerous-scheme/self-href warning) are already written and tested as a standalone,
       unwired function, [`apply_image_side_effects`](../../parser/src/content/inline_builder/macros/image.rs)
       — see the step's own "landed as" note above.
     - ✅ **prep (links).** `register_link` for the four link-macro forms is likewise staged as a
       standalone, unwired function,
       [`apply_link_side_effects`](../../parser/src/content/inline_builder/macros/links.rs) — see the
       step's own "landed as" note above.
     - ✅ **prep (anchors).** The anchor/attributed-span `register_ref` pair — the last of step 6's
       unstaged registrations — is likewise staged as a standalone, unwired function,
       [`apply_ref_side_effects`](../../parser/src/content/inline_builder/macros/anchors.rs) — see the
       step's own "landed as" note above. Every deferred recognition side effect is now staged;
       calling each one for real, exactly once per parse, remains this step's own job.
     - ✅ **prep (whole-pipeline parity).** A combined, multi-family differential corpus confirms
       `build` reproduces the real, public `SubstitutionGroup::Normal::apply` entry point
       byte-for-byte — see the step's own "landed as" note above. Wiring `build` into `Content` in
       place of the recorder, and calling `apply_macro_side_effects` for real, remain this step's
       own job.
     - ✅ **prep (hardbreaks).** The `hardbreaks` block option — identified as a real cutover
       blocker, not merely an unclaimed form, since golden tests already exercise it — is now
       recognized by `apply_post_replacements`, which takes the enclosing block's `Attrlist` for
       it; see the step's own "landed as" note above. `build`'s new `Attrlist` parameter is `None`
       at every existing call site, so this is unwired like every other prep piece.
     - ✅ **prep (structural cross-check).** A corpus-wide comparison of the builder's tree against
       the Strategy-A recorder's, structurally rather than by HTML-fold parity alone — the
       due-diligence design §4.1/§5.5 call for before the swap — found every difference to be
       already-documented one-sided richness or a leaf-boundary artifact of recovering structure
       from rendered output, and none a genuine regression; see the step's own "landed as" note
       above. Wiring `build` into `Content` in place of the recorder remains this step's own job.
     - ✅ **prep (synthesized seed).** [`build_from_value`](../../parser/src/content/inline_builder.rs)
       generalizes `build`'s seed from a bare `Span<'src>` to the `(value, location)` pair `Content`
       itself is built from, so a genuinely multi-line, filtered block (whose joined `rendered` text
       has no single contiguous `'src` slice) can be processed too, not only the common
       single-surviving-line case — see the step's own "landed as" note above. A macro family needing
       its own verbatim target/id remains deferred for a wholly-synthesized seed, a documented
       divergence pinned by its own test. Wiring `build`/`build_from_value` into `Content` in place of
       the recorder remains this step's own job.
     - ✅ **prep (anchor synthesized boundary lifted).** The "macro inside a synthesized run"
       boundary §4.1/§4.4 describe — and the synthesized-seed note directly above still pins for
       link/image/xref — is **lifted for the anchor family**, the one macro whose node needs no
       `Span`-typed field beyond its own `location` (an id and an optional reftext, both plain
       text). A new [`text_slice`](../../parser/src/content/inline_builder/quotes.rs) helper
       reuses `emit_range`'s own verbatim/synthesized slicing to recover a macro's target *text*
       exactly — concatenating a synthesized piece's `value` instead of `source_slice`'s coarse
       whole-piece fallback — and a new
       [`range_is_verbatim_or_synthesized`](../../parser/src/content/inline_builder/macros/image.rs)
       gate accepts a synthesized overlap while still rejecting an atomic one (an escaped special
       or a rendered span, the boundary every family keeps). `build_anchor_node` and
       `build_anchor_reftext` (`macros/anchors.rs`) now use these instead of deferring: an id or
       reftext coming from an attribute expansion, or — reached at a tree's root via
       `build_from_value` — a filtered multi-line block's own joined seed, is recognized with its
       exact text, while the node's `location` keeps the coarse enclosing-span fallback design §4.4
       already establishes elsewhere (only the *text* needed precision; `Attrlist`-bearing families
       like image/link still cannot lift this boundary the same way, since `Attrlist::parse` reads
       its `source: Span<'src>`'s bytes as content, not just as a location tag — a real `'src` slice
       is not optional there). The `an_anchor_inside_an_expanded_attribute_value_is_a_documented_
       divergence` test #1177 added is now a parity test instead of a divergence test, per its own
       "if lifted, fold this into a parity corpus" note. Wiring `build`/`build_from_value` into
       `Content` in place of the recorder remains this step's own job.
     - ✅ **first half: the tree-source swap.** `SubstitutionGroup::apply` builds each content's
       tree with the single-pass builder — via the new group-aware
       [`build_for_group`](../../parser/src/content/inline_builder/mod.rs), mirroring
       `run_pipeline`'s own step selection per substitution group — in place of the Strategy-A
       recording pass, which is retired to test-only oracle machinery; see the step's own
       "landed as" note above. The remaining half — `rendered_html()` as an authoritative fold,
       `apply_macro_side_effects` called for real, the sentinel deletions, and the flag's
       retirement — is still outstanding.
     - ✅ **prep (unescaped specials).** A corpus-wide fold-parity audit — the same sweep that
       identified `hardbreaks` — surfaced a second real blocker: under an effective order whose
       steps omit `specialcharacters` (a `Pass` or `None` block, or a `subs=` list without it),
       a literal `<`/`>`/`&` must be a `Raw` leaf the fold emits verbatim, not the `Text` run the
       fold escapes (§3.4.1 applied to the seed).
       [`classify_unescaped_specials`](../../parser/src/content/inline_builder/special_chars.rs)
       runs last in `build_for_group` for exactly those orders; see the step's own "landed as"
       note above, which also records what the audit leaves outstanding for the authoritative
       fold.
     - ✅ **prep (bare e-mail auto-link).** The first of that audit's own itemized divergences is
       closed: [`email_level`](../../parser/src/content/inline_builder/macros/links.rs) recognizes a
       bare address (`doc@example.org`, `INLINE_EMAIL`) as the same `Ref{Link}` node the two
       URL-link passes build — the last of the link family's spellings — and
       [`apply_link_side_effects`](../../parser/src/content/inline_builder/macros/links.rs) gains
       the third registration pass its own family-pass order requires; see the step's own "landed
       as" note above. Recognized inside a synthesized run too (an e-mail node carries no
       `Span`-typed field, like an anchor's id). Two forms defer: an address carrying an escaped
       `&`, and one abutting an already-recognized construct — the latter a newly-named category
       (a *boundary class* the string pipeline reads out of rendered markup, which a placeholder
       hides), whose pre-existing mirror image in the auto-link family
       (`**bold**https://example.org`) is now documented and pinned too.
     - ✅ **prep (angle-bracketed URL).** The second of that audit's divergences is closed:
       `INLINE_LINK`'s **ANGLE branch** — `<https://example.org>`, and the
       `<https://example.org[text]` spelling that keeps its `&lt;` — is recognized as the same
       `Ref{Link}` node its non-angle sibling builds. The family's verbatim gate moves into
       [`build_inline_link_node`](../../parser/src/content/inline_builder/macros/links.rs) and, for
       this branch, covers only the *interior* between the `&lt;`/`&gt;` delimiters — themselves
       escaped `CharRef`s, and emitted by neither pipeline — with
       [`build_angle_link_node`](../../parser/src/content/inline_builder/macros/links.rs) mirroring
       the replacer's own `<url>` special case (whole match consumed, no trailing-punctuation strip,
       always `bare`); see the step's own "landed as" note above. The branch's third alternative (an
       unterminated `<url`) stays literal in both pipelines, and an interior crossing an escaped
       special or a rendered span still defers, each with its own divergence test.
     - ✅ **prep (`&gt;`-submenu menu form).** The third of that audit's divergences is closed:
       `menu:View[Zoom > Reset]` is recognized as the same `Ui` node the comma-delimited and
       bare/single-item forms build, via a new
       [`menu_match_is_sliceable`](../../parser/src/content/inline_builder/macros/ui.rs) gate that
       admits one atomic piece — a `&gt;` caret *inside the item list*, which the node consumes as
       the list's delimiter and neither pipeline emits — applied inside `build_menu_node` exactly as
       the angle branch moved its own gate; see the step's own "landed as" note above. Any other
       escaped special, a caret or `&` in the menu *name*, a rendered span, and a synthesized run all
       still defer, each with its own divergence test. The family's escape check is hoisted ahead of
       the gate (the `footnoteref:` increment's own fix, for the identical reason), so an escaped
       macro the gate rejects still drops its backslash.
     - ✅ **prep (bibliography anchor).** The fourth of that audit's divergences is closed — and the
       first it reached through the *parse context* rather than a construct's spelling:
       `[[[label]]]` prefixing a bibliography list item's principal text is recognized as an `Anchor`
       node carrying a new `is_bibliography` flag (as an `Image`'s `is_icon` tells its two forms
       apart), gated on the same `Parser::in_bibliography_list_item` flag the string step reads and
       the tree build already inherits through `SubstitutionGroup::apply`'s parser clone. It runs
       once, `^`-anchored, ahead of every other family and outside `apply_macros`'s recursion, and
       leaves the bracketed label **in the flow** as sibling nodes (so every later family sees it
       exactly as the string pipeline's later passes do) while the node's `reftext` carries that same
       label as the entry's *registered* reference text, in already-substituted form.
       [`apply_biblio_side_effects`](../../parser/src/content/inline_builder/macros/anchors.rs)
       stages the `register_ref` and is called first by `apply_macro_side_effects`, matching the
       string pipeline's own pass order for the shared warnings list; see the step's own "landed as"
       note above. A label crossing an opaque piece (a rendered span, a passthrough, or a character
       replacement) still defers, with its own divergence test.
     - ✅ **prep (`attribute-missing` drop modes).** The fifth of that audit's divergences is
       closed — and, like `hardbreaks` and the unescaped-specials classification, a **blocker**
       rather than an unclaimed form, since golden tests set both modes:
       [`apply_attribute_references`](../../parser/src/content/inline_builder/attribute_refs.rs)
       now honors `attribute-missing=drop` and `=drop-line`. A new
       [`MissingHandling`](../../parser/src/content/inline_builder/attribute_refs.rs) carries the
       mode down the recursion and
       [`surviving_lines`](../../parser/src/content/inline_builder/attribute_refs.rs) reproduces
       `apply_attributes`' own line loop over a level's match string, whose `\n` bytes are the
       rendered string's own; the rebuild re-emits a real source `\n` between survivors, so a
       dropped line costs no honest span. See the step's own "landed as" note above. A span
       straddling a line break disables the drop for the whole content (its interior newlines are
       hidden behind one placeholder), with its own divergence test; the `drop-line` diagnostic is
       deferred to the cutover like every other family's warning.
     - ✅ **prep (`<<id,>>`'s present-but-empty text).** The sixth of that audit's divergences is
       closed — a **blocker** like the three before it, since a golden test exercises it — and with
       it the `Ref{Xref}` family: a shorthand's comma is what makes its reference text *present*,
       so [`build_xref_shorthand_node`](../../parser/src/content/inline_builder/macros/xref.rs)
       always builds one [`Text`](../../parser/src/inlines/inline_node.rs) child for a
       comma-carrying shorthand (empty value and all) and
       [`fold_xref`](../../parser/src/content/inline_builder/fold.rs) keys `provided_text` on that
       child's *presence* rather than on what it folds to — no new node field, and the family's
       "this increment defers" machinery removed, its verbatim gate now the only deferral. Keeping
       the marker alive made [`split_text`](../../parser/src/content/inline_builder/special_chars.rs)
       preserve an empty `Text` node instead of splitting it away to nothing, and
       [`build_for_group`](../../parser/src/content/inline_builder/mod.rs) stop seeding one for
       empty content. See the step's own "landed as" note above.
     - ✅ **prep (UI + index terms inside an expanded value).** The seventh of that audit's
       divergences — "a macro inside an expanded attribute value" — is closed for the two macro
       families whose nodes carry no `Span`-typed field, which is the same reason the anchor and
       bare-e-mail families already lifted it: `kbd:`/`btn:`/`menu:`
       ([`ui`](../../parser/src/content/inline_builder/macros/ui.rs)) and index terms
       ([`indexterm`](../../parser/src/content/inline_builder/macros/indexterm.rs)) now recognize a
       macro sitting inside a [`synthesized`](../../parser/src/content/inline_builder/quotes.rs)
       run, computing every value from the level's match string (or, for the menu *name*, through
       [`text_slice`](../../parser/src/content/inline_builder/quotes.rs)) with only the node's
       `location` taking §4.4's coarse fallback; see the step's own "landed as" note above. Every
       other boundary the two families draw — an escaped special or a rendered span in a bracket,
       a menu name, or a visible term, and an `indexterm2:` attribute-list term — is unchanged.
       The families that carry an [`Attrlist`](../../parser/src/attributes/attrlist.rs)`<'src>` or
       another `Span`-typed field (image, link, cross-reference) still defer here, so that half of
       the map item remains open.
     - ✅ **prep (cross-references inside an expanded value).** The same map item, closed for a
       third family: a `Ref{Xref}` node is built with `attrs: None` in both spellings — the `xref:`
       macro's own attribute list is parsed from a newline-normalized *copy*, not a source slice —
       so the cross-reference family never needed an honest `'src` slice either, and
       [`find_xref_matches`](../../parser/src/content/inline_builder/macros/xref.rs) makes the same
       one-gate swap to `range_is_verbatim_or_synthesized`, reading its target and reference text
       out of the match string and through `text_slice`; see the step's own "landed as" note above.
     - ✅ **prep (images and links inside an expanded value).** The map item closed in full, for the
       two families that hold a real `Attrlist<'src>`: the same one-gate swap, with the boundary
       moved from *the family* to *the one capture that still needs an `'src` slice* — an image's
       non-empty attribute list and a link's attribute-list-bearing display text, since
       `Attrlist::parse` reads its own source span's bytes as content. An image's macro name and
       target now come from the match string / `text_slice`, and an empty bracket parses from a
       zero-length span, so even a wholly-expanded `image:x.png[]` is recognized. The
       `link:`/`mailto:` macro pass additionally requires its own literal marker to be verbatim,
       because that marker is how `link_form` tells its nodes from the other link passes' when
       replaying the string pipeline's registration order; see the step's own "landed as" note
       above.
     - ✅ **prep (a cross-reference text crossing an escaped special).** The first half of the
       audit's last remaining *normal*-order divergence — "a display or reference text crossing a
       rendered span" — split apart: an escaped special is **not** like a rendered span, because
       `build_match_string` gives a `CharRef::Special` its canonical entity (the string pipeline's
       own haystack bytes) while a span is one opaque placeholder standing in for markup that
       exists only at fold time. A third gate,
       [`range_has_no_opaque_piece`](../../parser/src/content/inline_builder/macros/image.rs),
       names that distinction, and the cross-reference family — which holds nothing `Span`-typed —
       adopts it with the same one-gate swap the synthesized-run lifts made, its reference text
       becoming **structured children** via `emit_range` (so the special stays its own `CharRef`
       and is escaped once, not twice) and an attribute-list positional value being unescaped back
       to logical text. The family's escape check is hoisted ahead of the gate (the `footnoteref:`
       and menu increments' own fix, for the identical reason). See the step's own "landed as"
       note above. A text crossing a *rendered span* still defers, for every reference-bearing
       family, as does an escaped special for the link and image families.
     - ✅ **prep (a `link:`/`mailto:` macro crossing an escaped special).** The second family to
       take that third gate, by the same one-gate swap: a macro whose **target** or **display
       text** crosses an escaped special (`link:a&b.html[]`, `link:index.html[a < b]`,
       `mailto:a&b@example.org[]`) is recognized, since the target is a computed string read off
       the match string and the display text becomes structured children — through
       [`macro_text_children`](../../parser/src/content/inline_builder/macros/mod.rs), the
       cross-reference family's own helper, now shared so the two cannot drift. A *bare* macro's
       target-derived text is recovered from the target group's own range (`URI_SNIFF` being
       `^`-anchored, the `hide-uri-scheme` strip always leaves a suffix) rather than baked, already
       escaped, into one `Text` the fold would escape twice. The family's escape check is hoisted
       ahead of the gate, closing the same latent gap its three predecessors did. The one capture
       that keeps the boundary is a display text carrying an attribute list, parsed as a real
       `Attrlist<'src>` from the source's own bytes; see the step's own "landed as" note above.
       The auto-link/formal-URL, bare-e-mail, and image families still keep it.
     - ✅ **prep (the auto-link / formal-URL family crossing an escaped special).** The third family
       to take that third gate, in both of `INLINE_LINK`'s branches: a bare auto-link, a formal URL
       link, and an angle-bracketed URL whose target or display text crosses one
       (`https://example.org/?a=1&b=2`, `https://example.org[a < b]`, `<https://example.org/a&b>`)
       is recognized, by the same swap and the same structured-children recovery — the ANGLE branch
       keeping its own interior-only gate, now expressed inside `build_angle_link_node`, and the
       family's escape check hoisted ahead of the gate as its four predecessors' were.
       `INLINE_LINK`'s trailing-punctuation arithmetic, named earlier as this family's blocker,
       turns out to agree with the replacer's by construction (both run over the escaped text) and
       to cost only one narrow deferral: a bare URL ending in a literal `&`, whose strip would cut
       inside the `&amp;` leaf. Landing this also fixed a latent gap in `macro_text_children` — a
       *verbatim* range need not be contiguous in the source, so the value now comes from
       `text_slice` rather than from re-reading the enclosing span, which had been reinstating the
       backslash of an escaped attribute reference (`link:x[\{name}]`) for all three families that
       use the helper. See the step's own "landed as" note above. The bare-e-mail and image families
       still keep the escaped-special boundary, and a display text crossing a rendered span still
       defers everywhere.
     - ✅ **prep (the bare e-mail auto-link crossing an escaped special).** The fourth family to
       lift that boundary, and the first to need **no gate** for it: `a&b@example.com` is
       recognized as the same `Ref{Link}` node its verbatim spelling builds, its target read off
       the match string (`mailto:a&amp;b@example.com`, the bytes `InlineEmailReplacer` computes
       and registers) and its shown text — the address itself, a *slice* rather than a baked
       string — recovered through the shared `macro_text_children`. An address cannot cross an
       opaque piece at all: `INLINE_EMAIL`'s character classes admit no `SPAN_PLACEHOLDER`
       (category `Co`), and a match can neither begin nor end inside an entity (its `&` fits no
       domain/TLD class; its `;` is neither a local-part character nor the `@` a local part needs),
       so the only atomic overlap possible is a wholly-contained `&amp;` — exactly what the lift
       admits, making `range_has_no_opaque_piece` here a branch no input reaches.
       `build_email_node` is correspondingly **total**, its `Option` machinery removed rather than
       left unreachable, and the family's one remaining deferral is the *abutting* boundary class
       (a placeholder-**prefix** check, not a range gate), now pinned for a character replacement
       too. See the step's own "landed as" note above. Only the **image** family still keeps the
       escaped-special boundary, and a display text crossing a rendered span still defers
       everywhere.
     - ✅ **prep (the image family crossing an escaped special).** The fifth and last family to
       lift that boundary, which closes it as a *family* boundary altogether: `image:a&b.png[]`
       is recognized as the same `Image` node its verbatim spelling builds, its target read off
       the match string (`a&amp;b.png`, the `caps[1]` the string replacer renders as the `src`,
       derives `default_alt` from, and registers) through `text_slice`'s own fallback, so a
       verbatim or synthesized target still borrows `'src`. An image has no shown text, so the
       shared `macro_text_children` has no part here; and no capture boundary can split an entity,
       since each is delimited by `image:`/`icon:`, `[`, or `]`. What remains is exactly the
       **non-empty attribute list**, which keeps `range_is_verbatim` because `Attrlist::parse`
       reads its own source span's bytes as content — so the escaped special is now only a
       *capture* boundary, for the three attribute lists that must ride on a node as a real
       `Attrlist<'src>`. The family's escape check is hoisted ahead of the gate, closing the same
       latent gap its four predecessors did. A restored entity in the target, and a rendered span
       anywhere in the match, still defer, each with its own divergence test. See the step's own
       "landed as" note above.
     - ✅ **prep (a restored entity, the second recoverable piece).** The escaped-special lift's own
       leftover — a *restored* entity (`&amp;copy;`, `&#8217;`), which the image and auto-link
       families each pinned as their last deferral — is closed for **every** family at once, since
       recoverability is a property of the piece rather than of the family reading across it:
       [`build_match_string`](../../parser/src/content/inline_builder/quotes.rs) gives a
       `CharRef::Entity` leaf its own bytes (what the string pipeline's haystack holds there, and
       what the fold emits verbatim — no renderer involved, which is what separates it from a
       rendered span) and
       [`range_has_no_opaque_piece`](../../parser/src/content/inline_builder/macros/image.rs) admits
       it. The one computed value with no range to rebuild from — a cross-reference's attribute-list
       positional text — gets
       [`escaped_value_children`](../../parser/src/content/inline_builder/macros/xref.rs), which
       re-derives §3.4's trichotomy from the value's own bytes, scanning left to right so a
       doubly-escaped `&amp;amp;copy;` unwinds exactly one level. The UI family and a footnote's
       text keep their own stricter gates (pinned for both spellings side by side), as do the
       attribute-list captures that need a real `Attrlist<'src>`. See the step's own "landed as"
       note above. Only a **rendered span** now defers as a whole class.
     - ✅ **prep (a reference text crossing a rendered span — the cross-reference family).** The
       first family of that last class, and the first lift that is not a gate relaxation but a
       change of *which bytes the gate covers*: a rendered span stays unrecoverable (its markup
       exists only at fold time), so every value a family **computes** keeps
       [`range_has_no_opaque_piece`](../../parser/src/content/inline_builder/macros/image.rs), while
       a **reference text** — which computes nothing, becoming structured children whose
       [`emit_range`](../../parser/src/content/inline_builder/quotes.rs) path clones the piece's own
       node whole — needs no gate at all.
       [`find_xref_matches`](../../parser/src/content/inline_builder/macros/xref.rs) therefore gates
       the macro form's target and the shorthand's id half (`shorthand_id_range`) rather than the
       whole match; no builder changed. Three shapes stay divergent because the string replacer
       matches over the markup where this matches over one placeholder — a `]` or a `&gt;&gt;`
       inside the span, and markup carrying an `=` beside a comma — each the markup-perturbed
       reading against the tree's well-formed one, as the quotes step's crossed delimiters are; a
       text carrying its own attribute list keeps the stricter gate outright. See the step's own
       "landed as" note above. The `link:`/`mailto:`, auto-link/formal-URL, and image families keep
       the boundary, each a later increment.
     - ✅ **prep (a display text crossing a rendered span — the `link:`/`mailto:` macro family).**
       The second family of that last class, by the identical move:
       [`find_link_macro_matches`](../../parser/src/content/inline_builder/macros/links.rs) gates
       the **target** (`INLINE_LINK_MACRO`'s group 3) — the one value this family computes off the
       match string — rather than the whole match, while the bracketed **display text**, which
       computes nothing, is carried structurally through the shared
       [`macro_text_children`](../../parser/src/content/inline_builder/macros/mod.rs). No builder
       changed, and neither did `apply_link_side_effects` (a node's target, what it registers, is
       exactly the value that kept the gate). What defers is a target crossing an opaque piece and
       a display text carrying its own `Attrlist`, each with its own divergence test; the
       recognition-agreement gap takes this family's own three shapes (a `]` inside the span, an
       `=` beside a comma in a `link:` text, a comma inside the span of a `mailto:` text). See the
       step's own "landed as" note above. The auto-link / formal-URL family and the image family's
       own attribute list keep the boundary, each a later increment.
     - ✅ **prep (a display text crossing a rendered span — the auto-link / formal-URL family).**
       The third and last reference-bearing family of that class, by the identical move:
       [`build_inline_link_node`](../../parser/src/content/inline_builder/macros/links.rs) applies
       [`range_has_no_opaque_piece`](../../parser/src/content/inline_builder/macros/image.rs) to the
       match *up to* the bracketed display text — every value this family computes (the boundary
       prefix, the scheme, the URL that becomes the target) lies before it — while the text itself,
       which computes nothing, is carried structurally through the shared
       [`macro_text_children`](../../parser/src/content/inline_builder/macros/mod.rs). No builder
       changed, and neither did `apply_link_side_effects`. The family's two textless forms keep the
       gate over everything they cover: a **bare** auto-link, whose shown text is a slice of its own
       target, and the `<url>` form, whose whole interior the target is computed from. What defers is
       a target crossing an opaque piece and a display text carrying its own `Attrlist`, each with
       its own divergence test; the recognition-agreement gap takes two of the three shapes the
       `link:`/`mailto:` family's did (a `]` inside the span, an `=` beside a comma in the text —
       this pattern has no `mailto:` comma probe of its own). See the step's own "landed as" note
       above. Only the **image** family's attribute list — the one capture with no display text to
       carry — still keeps the boundary.
     - ✅ **prep (the UI family crossing an escaped special or a restored entity).** The last
       family holding a gate of its own takes the same one-gate swap to
       [`range_has_no_opaque_piece`](../../parser/src/content/inline_builder/macros/image.rs):
       every value a [`Ui`](../../parser/src/inlines/ui.rs) node holds is already-substituted text
       read out of the match string — the very bytes the string replacer computes from its own
       escaped haystack and `render_keyboard`/`render_button`/`render_menu` emit verbatim — so the
       menu family's bespoke caret-only gate is deleted rather than relaxed, and
       [`build_menu_node`](../../parser/src/content/inline_builder/macros/ui.rs) becomes total. A
       **footnote's text**, pinned beside it as the other half of that boundary, turned out to be
       at parity already (its content is structured children, never a sliced value); the
       "divergence" was a shared-`Parser` numbering artifact in the test that pinned it, now
       rewritten as a two-independent-parsers parity corpus. See the step's own "landed as" note
       above. What remains is the opaque-piece boundary every family keeps, plus the three
       `Attrlist<'src>` captures.
     - ✅ **prep (an order that escapes after expanding).** The last category the corpus-wide audit
       named — "an effective order that runs `SpecialCharacters` **after** a step that already
       produced markup" — is closed for the half that is not about markup at all, and a **blocker**
       like the four before it since a golden test exercises it: under `subs=attributes+` (the
       documented idiom for inspecting a `pass:quotes[…]` attribute entry) the string pipeline
       escapes the spliced value, so
       [`split_attribute_value`](../../parser/src/content/inline_builder/attribute_refs.rs) can no
       longer assume the escaping step already ran. A new
       [`SplicedSpecials`](../../parser/src/content/inline_builder/attribute_refs.rs), decided in
       [`build_for_group`](../../parser/src/content/inline_builder/mod.rs) where the effective order
       is in hand, splices the value as one ordinary `Text` run for the still-ahead
       `SpecialCharacters` step to split — §3.4.1 applied to the step's *position*, not merely its
       presence; see the step's own "landed as" note above. The category's other half — a
       markup-producing step (`subs=quotes,specialcharacters`) whose emitted tags the escaping step
       acts on — needs a policy of its own rather than another classification, and is closed by the
       increment directly below.
     - ✅ **prep (a footnote inside an expanded value).** The last family to make that lift, and the
       only one that was not deferring but building a **wrong node**: the footnote family has no
       gate, so it always recognized such a macro and read its id through `source_slice`, whose
       coarse §4.4 fallback for a synthesized range is the attribute *reference* itself
       (`{fn-disclaimer}`) — the id being the key its one required side effect registers and looks
       a number up by, so every later reference renumbered. A **blocker**, like the five before it,
       since three real golden fixtures exercise the externalized-footnote idiom. A new
       [`footnote_id_text`](../../parser/src/content/inline_builder/footnotes.rs) recovers the id
       with [`text_slice`](../../parser/src/content/inline_builder/quotes.rs), falling back to the
       level's match string — what the string replacer itself reads and registers — with only the
       node's `location` keeping the coarse span. The `footnote:` form needs no gate (its `[\w-]+`
       id admits neither an entity's `&`/`;` nor the `SPAN_PLACEHOLDER`, the bare-e-mail family's
       own structural argument); the deprecated `footnoteref:` form's arbitrary id half takes
       [`range_has_no_opaque_piece`](../../parser/src/content/inline_builder/macros/image.rs), so a
       rendered span in it defers with its own divergence test. See the step's own "landed as" note
       above.
     - ✅ **prep (an image's attribute list).** The first of the three `Attrlist<'src>` captures —
       the last boundary drawn around a *capture* rather than around an opaque piece — is lifted:
       an [`Attrlist`](../../parser/src/attributes/attrlist.rs) holds nothing `Span`-typed but its
       own location tag, so a new
       [`Attrlist::into_owned`](../../parser/src/attributes/attrlist.rs) lets a list parsed from a
       temporary be kept, and
       [`bracket_attrlist`](../../parser/src/content/inline_builder/macros/image.rs) parses a
       bracket with no `'src` slice from the level's **match string** — the same
       `Attrlist::parse(Span::new(&caps[2]), …)` the string replacer performs — owning the result
       onto §4.4's coarse span. `image:sunset.jpg[{caption}]` and
       `image:tiger.svg[A <b> & "c",opts=interactive]` are recognized; the family's last
       `range_is_verbatim` call is gone and `build_image_node` is total. See the step's own "landed
       as" note above. The two link-family display-text captures are the next increment.
     - ✅ **prep (the two link-family display texts, closing the `Attrlist<'src>` captures).** The
       remaining two captures — the `link:`/`mailto:` macro's and the auto-link/formal-URL family's
       attribute-list-bearing display text — take the same move, through one shared
       [`text_attrlist`](../../parser/src/content/inline_builder/macros/links.rs): a text whose
       source slice *is* the newline-normalized copy the string replacers parse keeps the `'src`
       parse and its borrow, and every other text is parsed from the level's **match string** and
       [`into_owned`](../../parser/src/attributes/attrlist.rs)ed onto §4.4's coarse span. The
       positional value such a parse returns is already-escaped text, so it is rebuilt through the
       cross-reference family's own
       [`escaped_value_children`](../../parser/src/content/inline_builder/macros/mod.rs), now
       shared by all three families. See the step's own "landed as" note above. With this the only
       boundary any macro family still draws is the **opaque-piece** one every family shares.
     - ✅ **prep (a typographic replacement, the third recoverable piece).** The last leaf
       `build_match_string` stood in as an opaque placeholder — a
       [`CharRef`](../../parser/src/inlines/char_ref.rs)`::Replacement` (`(C)` → ©, `'` → ’, the
       arrows) — now contributes the entity the built-in backend renders it as
       ([`replacement_entity`](../../parser/src/content/inline_builder/quotes.rs)), the string
       pipeline's own haystack bytes there, and
       [`range_has_no_opaque_piece`](../../parser/src/content/inline_builder/macros/image.rs) admits
       it. Lifted once for every family, as the restored-entity increment was; no builder, gate, or
       node kind changed. Three real golden fixtures stop diverging
       (`image:pause.png[title=Pause (C) Resume]`, `image:tiger-roar.png[A tiger's "roar" …]`,
       `<<Cub => Tiger>>`). See the step's own "landed as" note above. With this the
       **opaque-piece** boundary — the only one any macro family draws — rejects nothing but
       pieces whose bytes exist at fold time alone.
     - ✅ **prep (a footnote's escaped closing bracket).** The footnote family's own last deferred
       form — a bracket content carrying a `\]`, which four real golden sources exercise — is
       closed by the rebuild it had said it lacked: the reference-bearing families' own
       gap-emitting unescape, lifted out of
       [`macro_text_children`](../../parser/src/content/inline_builder/macros/mod.rs) as the shared
       [`emit_range_unescaping_brackets`](../../parser/src/content/inline_builder/macros/mod.rs)
       and emitted through by
       [`footnote_children`](../../parser/src/content/inline_builder/footnotes.rs), so the
       backslash is a gap between two `'src`-borrowing children rather than an owned rebuild and
       the two families cannot drift on which backslashes pair off; see the step's own "landed as"
       note above. The `footnoteref:` form's **id** half keeps its backslash on both sides (the
       string replacer never normalizes it), pinned by its own parity test.
     - ✅ **prep (an order that escapes after a markup-producing step).** The audit map's last
       item, and the one category no recognition or classification increment could close: under
       `subs=quotes,specialcharacters` the string pipeline escapes the tags the quotes step already
       wrote, while a tree's markup exists only at fold time. The policy is to reach fold time for
       that one node early —
       [`flatten_prior_markup`](../../parser/src/content/inline_builder/special_chars.rs) folds each
       node an earlier step of the same order turned into markup and keeps the result as one
       [`Text`](../../parser/src/inlines/inline_node.rs) node's value, which the escaping step then
       splits like any other text, so §3.4's single escape lands exactly where the string
       pipeline's does. Nodes the string pipeline holds as a *placeholder* at that moment — a
       passthrough or STEM body, and a **deferred cross-reference** — are excluded, told apart by
       node kind or (for an attribute-list-prefixed passthrough's own `Styled` wrapper) by
       `masked_locations`, collected before any step ran. What defers is such a construct *nested
       inside* a flattened node, and a `link:`/`mailto:` macro written inside flattened markup (the
       family's own pre-existing verbatim-marker rule), each with its own divergence test. See the
       step's own "landed as" note above. With this every item the corpus-wide audit's map named is
       closed.
     - ✅ **prep (an attributed span's attribute list).** The last attribute list this module
       parsed from `'src`, and — like `hardbreaks`, the unescaped-specials classification, and the
       four blockers before it — a *wrong* answer rather than an unclaimed form, for content a
       golden **security** test exercises: the quotes step runs after `SpecialCharacters`, so the
       string replacer parses (and renders verbatim into the `class`/`id`) the **escaped** text,
       where the source slice carries the author's raw `<`/`&`.
       [`quote_attributes`](../../parser/src/content/inline_builder/quotes.rs) takes the same
       match-string parse the image bracket and the two link display texts already took, keeping
       the `'src` parse and its borrow for a verbatim list; and
       [`Attrlist::into_owned`](../../parser/src/attributes/attrlist.rs) now keeps the text it was
       parsed from, so `quoted_text_fallback_role` — the one accessor reading a list's own text
       rather than a parsed attribute — goes on reading it. A list crossing an opaque piece is
       dropped rather than built wrong, and one a *later* step of the same order rewrites inside
       the emitted markup stays divergent, each with its own test; see the step's own "landed as"
       note above.

     - ✅ **prep (an enclosing span's boundary characters).** The sweep that followed the
       increment above turned up one more blocker of the same kind — ``` `"``end points``"` ```,
       a golden fixture whose inner backticks the string pipeline leaves literal and the fold
       wrapped in a `<code>` span — and with it a category no increment had named: a pattern's
       boundary classes
       read the *rendered markup* an earlier step wrote, where a transducer matching one level at
       a time reads that level's own start or end. A new
       [`LevelContext`](../../parser/src/content/inline_builder/quotes.rs) wraps a level's match
       string in the two characters its enclosing construct's rendering presents (`>`/`<` for a
       tag, `;`/`&` for a smart quote, probed for the one variant whose shape its attribute list
       decides — pinned against the built-in renderer) and maps every offset back with
       [`unshift`](../../parser/src/content/inline_builder/quotes.rs), so only recognition changes
       and no shared machinery changes signature. The two steps whose patterns read a boundary —
       `Quotes` and `CharacterReplacements` (the spaced em dash, at either edge of any span) —
       take it; see the step's own "landed as" note above. What still defers is the same class one
       level out (a construct *beside* a span, where the placeholder belongs to no boundary class),
       the macros step's own families, and a transparent span's siblings, each with its own
       divergence test.

     - ✅ **prep (the same boundary, for the macros step).** The third of the categories the
       increment above names — the **macros** step's own boundary-reading families, the
       auto-link's prefix group and the bare e-mail's mismatch-prefix one — takes the context too,
       closing
       `*doc@example.org*` (which the string pipeline leaves literal, reading the `>` that ends
       `<strong>`). `apply_macro_families` carries a `LevelContext` down its own recursion to every
       family, but where the two other steps map each offset back with `unshift`, a macro family
       reads the match string's *bytes* through the level's `Piece`s — so a new
       [`LevelContext::shift`](../../parser/src/content/inline_builder/quotes.rs) moves those pieces
       into the wrapped string's coordinates instead, leaving one coordinate system and no shared
       signature changed. Only the **opening** character is applied: a boundary class reads one
       character, while a macro body class consumes greedily and would swallow a half-supplied
       closing one. See the step's own "landed as" note above. What still defers is that closing
       half (a bare URL at an entity-rendered span's own closing edge), a transparent span's
       siblings, and the abutting-placeholder class the e-mail family already documents, each with
       its own divergence test.

     - ✅ **prep (the same boundary, for a span's siblings).** The half both increments above name —
       a construct written *beside* a rendered span, where
       [`build_match_string`](../../parser/src/content/inline_builder/quotes.rs) stood the whole span
       in as one placeholder — is closed for the class whose answer is unambiguous: that placeholder
       is now wrapped in the two characters the span's own rendering presents to a sibling (the first
       of its opening markup and the last of its closing one, the mirror image of the pair a
       `LevelContext` carries), which belong to no piece and so change recognition alone. Only the two
       **entity**-rendered variants are classified, because a `Styled` node here may be the
       passthrough-extraction pass's own wrapper — which the string pipeline holds as a *sentinel*,
       reading exactly as the bare placeholder does — and both wrappers that pass builds are
       tag-rendered; see the step's own "landed as" note above. What still defers is the tag-rendered
       half (which needs `masked_locations`' identity here), a transparent span's siblings, and the
       macros step's own closing character, each with its own divergence note.

     - ✅ **prep (the same boundary, for a transparent span's own siblings).** The shape all three
       increments above name — a span rendering to its **body and nothing else**, whose children had
       inherited the enclosing construct's markup rather than reading what stands beside the span —
       is closed for the two steps that can act on it. A new
       [`LevelContext::child_contexts`](../../parser/src/content/inline_builder/quotes.rs) answers a
       whole level at once and derives a transparent span's own context from the level's **match
       string**, the one place every node kind's presented bytes are already spelled out, so nothing
       is recomputed per node kind. Only the **opening** character is carried (a delimiter swallows a
       closing one, and what the replacer swallows it deletes — which a level's rebuild cannot do to
       a node another level owns), and a bare placeholder reports nothing rather than manufacturing a
       character the level had not been reading. The `character replacements` step is deliberately
       excluded: its one boundary-reading rule *consumes* the spaces it matches, so even an opening
       character would be written twice. See the step's own "landed as" note above. What still defers
       is the tag-rendered half, the closing character, a transparent span read *as* a sibling, and
       that consume-across-levels case.

     - ✅ **prep (the extraction pass's identity, and the tag-rendered half it was blocking).** The
       blocker all three increments above name — a tag-rendered [`Styled`] node may be the
       passthrough-extraction pass's own wrapper, which the string pipeline holds as a sentinel
       rather than as markup, and telling one from a rendered span needs
       [`masked_locations`](../../parser/src/content/inline_builder/special_chars.rs)' identity — is
       closed by carrying that identity into recognition as a
       [`Masked`](../../parser/src/content/inline_builder/special_chars.rs), whose third state
       (`UNKNOWN`) lets a caller say it does not hold the list rather than claim nothing is a
       wrapper. Only the **macros** step is given the real one, which is the scope the increment
       above had already drawn: `INLINE_LINK`'s and `INLINE_EMAIL`'s prefix groups are the only
       classes in the module that read a tag's `>` differently from the bare placeholder. With it,
       [`styled_sibling_boundaries`](../../parser/src/content/inline_builder/quotes.rs) answers every
       variant from its own rendering, closing `**bold**https://example.org` (and its reverse, and
       `*x*[width=10]#doc@example.org#` one level in) while `[quotes]++x++https://example.org`
       — the new divergence a rendering-only classification would have introduced — stays literal in
       both, and — because the macros step no longer descends into a wrapper, whose body a *nested*
       build has already substituted — `[x-]++**b**https://example.org++` folds to the string
       pipeline's own bytes rather than to an `<a>` nested inside an `<a>`. See the step's own
       "landed as" note above. What still defers is the closing character,
       the character-replacements step's consume-across-levels case, and a transparent span read *as*
       a sibling, which this unblocks but does not take.

     - ✅ **prep (the same boundary, for a transparent span read *as* a sibling).** The half the
       increment above unblocked but did not take: a span rendering to its **body and nothing else**
       presents no markup, so
       [`build_match_string`](../../parser/src/content/inline_builder/quotes.rs) stood it in as a
       bare placeholder where the string pipeline holds that body — and
       `[width=10]##x ##https://example.org` links there on the space the body ends with. A
       transparent span's outer characters are its children's, so
       [`transparent_sibling_boundaries`](../../parser/src/content/inline_builder/quotes.rs) reads
       them out of the children's own **match string**, the same place
       [`child_contexts`](../../parser/src/content/inline_builder/quotes.rs) reads a level's
       siblings from, and the pair becomes two independent halves since a body's two ends are
       described separately. The
       [`Masked`](../../parser/src/content/inline_builder/special_chars.rs) guard is what makes it
       safe — `[width=10]++x ++` is an extraction wrapper that renders its body and nothing else too
       — and keeps the scope at the **macros** step alone; `child_contexts` steps back over the
       character a transparent span now presents to its neighbour, so its own children go on reading
       what precedes the span. See the step's own "landed as" note above. What still defers is a
       body class wanting more than one character (`https://example.org[width=10]##x##`), the
       closing character, and the character-replacements step's consume-across-levels case.

     - ✅ **prep (a `link:`/`mailto:` target crossing a passthrough).** The largest
       golden-exercised class the audit's remainder held — five asciidoc-lang fixtures spelling
       the two documented idioms, `link:++…++[]` and `link:pass:[…][…]` — is closed by a fourth
       gate: [`range_is_restorable`](../../parser/src/content/inline_builder/macros/image.rs)
       admits a masked passthrough's [`Raw`](../../parser/src/inlines/inline_node.rs) piece for a
       value the caller **restores**, since the node's `value` is the very substituted body
       `Passthroughs::restore_to` splices over the string pipeline's sentinel, and
       [`restore_masked_passthroughs`](../../parser/src/content/inline_builder/macros/links.rs)
       finishes the computed target into the restored string's own bytes — every decision (the
       `URI_SNIFF` strip) still made over the bytes as matched, and a bare macro's shown text
       carried structurally as the `Raw` node itself. A smuggled dangerous scheme
       (`link:++javascript:…++[]`) defers with a security divergence test — the string replacer's
       masked check misses it and emits a live link — and the staged side effect registers the
       honest restored target where the string pipeline registers sentinel bytes. The xref family
       (a deferred target read before restore, whose golden leaks the sentinel), the image family
       (masked `default_alt` arithmetic), and the auto-link family (boundary/strip arithmetic)
       keep the boundary, each pinned. See the step's own "landed as" note above.

     - ✅ **prep (an image/icon target crossing a passthrough).** The first of the two families
       that note named takes the same restore-the-value, decide-over-the-masked-bytes move:
       [`widen_masked_passthroughs`](../../parser/src/content/inline_builder/macros/image.rs)
       widens each masked passthrough's placeholder to the string pipeline's own sentinel shape
       (the family's target class is the module's one two-character-minimum pattern, which a bare
       placeholder cannot match), the target takes
       [`range_is_restorable`](../../parser/src/content/inline_builder/macros/image.rs) while the
       *parsed* bracket keeps the opaque-piece gate, and
       [`masked_default_alt`](../../parser/src/content/inline_builder/macros/image.rs) reproduces
       the `default_alt` derivation over the masked bytes with an index-keyed restore of whatever
       survives — so `image:++a_b-c.jpg++[]` keeps `alt="a_b-c.jpg"` exactly as the golden does.
       A `link=self` dangerous target smuggled in a passthrough, and a space the fold-time
       `web_path` percent-encodes where the golden normalized the space-free sentinel, each defer
       as the tree's well-formed reading with its own test; the staged side effect registers the
       honest restored target. See the step's own "landed as" note above. The auto-link /
       formal-URL family is now the class's last open item.

     - ✅ **prep (an auto-link / formal-URL target crossing a passthrough).** The class's last
       family, and the only one to need no new mechanism: [`INLINE_LINK`](../../parser/src/content/macros.rs)'s
       three URL classes admit the sentinel and the placeholder alike — the bare branch's trailing
       character class included — so recognition agrees with no widening, and the gate simply
       becomes [`range_is_restorable`](../../parser/src/content/inline_builder/macros/image.rs)
       over the whole range this pass reads (the URL being the only capture in it a placeholder can
       reach) and over the angle path's own interior. The boundary-prefix and trailing-strip
       arithmetic this family was said to be blocked on already runs over the bytes as matched,
       which a placeholder answers as the sentinel does — no quote, and no `;`/`:` for the strip —
       and the `hide-uri-scheme` sniff needs no masked reading of its own here, since this
       family's scheme is literal in the pattern and `URI_SNIFF` covers the same bytes either way.
       A restored `"` (escaped by the fold where the golden splices it raw into a finished
       `href`), an attribute-list display text over a passthrough (the parse's own gate), and a
       **STEM** expression in the target (a `Stem` node, not a `Raw` one — a lift for all three
       families at once) each defer with their own test; the staged side effect registers the
       honest restored target. See the step's own "landed as" note above. With this the class is
       closed for every family that computes a target; the bracket half — restoring inside each
       *parsed* attribute-list value — is what remains of it.

     - ✅ **prep (a masked STEM expression in a computed target).** The lift the three increments
       above each deferred to "the STEM step's own increment, across all three families at once".
       A masked STEM is the *other* node kind the one extraction pass produces — an implicit
       passthrough, standing in both haystacks as the same sentinel and the same placeholder — so
       the whole class extends to it through a pair of shared helpers,
       [`node_is_restorable`](../../parser/src/content/inline_builder/macros/image.rs) (the cheap
       gate discriminant) and
       [`restorable_body`](../../parser/src/content/inline_builder/macros/image.rs) (the bytes),
       which together carry the two sites the link families use. The invariant is that a restored
       body is exactly what the *fold* of that node emits, so a `Stem`'s comes from
       [`fold_stem`](../../parser/src/content/inline_builder/fold.rs) itself — the same
       `render_quoted_substitution` call `PassthroughRestoreReplacer` makes — and the two
       directions cannot drift. Unlike its passthrough sibling this one needs no security
       divergence: a STEM body is restored *wrapped* in its notation's delimiters, so it cannot
       smuggle a live scheme. The **`image:`/`icon:` family is deliberately left out** and its
       family code untouched: its target alone is re-processed by `web_path` at fold time, which
       posixifies the platform separator, and *every* rendered STEM body carries a backslash — so
       restoring there would make the `src` differ by platform. That split is what
       [`Restorable`](../../parser/src/content/inline_builder/macros/image.rs) names at the gate,
       pinned by a literal-stays test and a both-separators fold test. The attribute-list display
       texts keep the opaque-piece gate. See the step's own "landed as" note above. The bracket
       half and the image family's own STEM target are what remain of the class.

     - ✅ **prep (a masked passthrough inside an image's parsed bracket).** The first of the
       **bracket half**'s four captures — the half every restore-the-value increment above deferred
       in the same words, where the value comes back from a *parse* rather than being computed off
       the match string. Reproducing the string pipeline means reproducing its *order*:
       [`Attrlist::parse`](../../parser/src/attributes/attrlist.rs) reads the sentinel as one
       opaque run carrying none of the `,`/`=`/`"` bytes the split reads, so
       [`tokened_bracket`](../../parser/src/content/inline_builder/macros/image.rs) puts the
       bracket into that shape before the parse and a new
       [`Attrlist::into_owned_restoring`](../../parser/src/attributes/attrlist.rs) splices each
       body into the parsed values after it — index-keyed, so a discarded token shifts nothing.
       An [`ElementAttribute`](../../parser/src/attributes/element_attribute.rs)'s shorthand
       offsets are *shifted* past each substitution rather than re-derived, since a token holds
       none of the delimiters the scan keys off. The gate becomes the target's own
       [`range_is_restorable`](../../parser/src/content/inline_builder/macros/image.rs)/[`Restorable::Passthrough`](../../parser/src/content/inline_builder/macros/image.rs)
       pair, so both halves of the family admit the same kinds; a masked **STEM** stays deferred
       here too, the bracket having a `web_path`-bound value of its own (an interactive SVG's
       `fallback=`). A restored `"` and an author's own sentinel-shaped bytes each defer with their
       own test. See the step's own "landed as" note above. The bracket half's other three
       captures — the three families' attribute-list display texts — and the image family's STEM
       are what remain of the class.

     - ✅ **prep (a masked construct inside a link's display-text attribute list).** The bracket
       half's other three captures at once — the `link:` macro's `=` list, a `mailto:`'s `,` list,
       and the auto-link / formal-URL family's `=` list — which are three call sites of one
       function, [`text_attrlist`](../../parser/src/content/inline_builder/macros/links.rs), so
       one gate closes them all. It takes the image bracket's own order (token before the parse,
       restore after it) through a
       [`tokened_bracket`](../../parser/src/content/inline_builder/macros/image.rs) now shared by
       the two families, each passing the
       [`Restorable`](../../parser/src/content/inline_builder/macros/image.rs) kinds its own gate
       admits — `PassthroughOrStem` here, this family having no `web_path` of its own. What is new
       is the **sink**: a display text becomes the node's *children*, so the restore is
       structural — [`restored_value_children`](../../parser/src/content/inline_builder/macros/links.rs)
       re-splits the parsed value on its own tokens and splices the masked **node** back in, where
       splicing its bytes into a `Text` would have the fold escape live markup a second time. The
       one shape that defers is a `mailto:`'s **subject or body**, whose `encode_uri_component`
       runs before the restore and percent-encodes the string pipeline's own sentinel into an
       `href` nothing then restores — the cross-reference family's pre-restore boundary, drawn per
       *slot* so a masked display text beside a plain subject still lands. A restored `"` in the
       fold-escaped `title`, a `window=`/`opts=` the renderer *decides* on rather than emits (the
       safe reading, as the image family's own `link=` check is), and an author's own
       sentinel-shaped bytes each keep their divergence with their own test. See the step's own
       "landed as" note above. What remains of the class is the image family's STEM target and
       bracket, and the keeps.

     - ✅ **prep (a masked STEM expression in the `image:`/`icon:` family — the
       restore-the-value class closed).** The class's last family: target and bracket admit both
       masked kinds, and the fold-time `web_path` this family alone sits behind runs with each
       restored range **masked** back to the sentinel shape and spliced afterwards — the string
       pipeline's own resolve-then-restore order, reproduced at the same seam.
       [`Image::restored_target_ranges`](../../parser/src/inlines/image.rs) carries the record on
       the node (`ImageRenderParams`/`IconRenderParams` hand it to the renderer), and
       [`ElementAttribute::into_owned_restoring`](../../parser/src/attributes/element_attribute.rs)
       records each splice for the bracket's two `web_path`-bound values (an interactive SVG's
       `fallback=`, a macro-level `imagesdir=`). The `Restorable` gate split is deleted rather
       than relaxed, and the formerly-pinned restored-space `src` divergence reaches parity by
       the same move. See the step's own "landed as" note above. Only the keeps remain: the
       cross-reference family's and a `mailto:` subject/body's pre-restore boundaries, and the
       well-formed readings.

     - ✅ **prep (a computed value classified by where the escaping step sits).** The last
       divergence the restore-the-value class left that was not a *keep*, and the last §3.4.1
       classification the builder still made by assumption: a value the macros step **computes**
       off the level's match string — the one thing it reads as bytes rather than carrying
       structurally, since it comes back from an
       [`Attrlist`](../../parser/src/attributes/attrlist.rs) parse — was always read as
       already-escaped, so under an order that never escaped an author's own `&lt;` was unwound one
       level too far. A new
       [`ComputedSpecials`](../../parser/src/content/inline_builder/macros/mod.rs), decided in
       [`build_for_group`](../../parser/src/content/inline_builder/mod.rs) exactly as
       [`SplicedSpecials`](../../parser/src/content/inline_builder/attribute_refs.rs) is and
       threaded down to the three families that compute such a value, carries the decision, and
       [`computed_value_children`](../../parser/src/content/inline_builder/macros/mod.rs)
       dispatches to the existing unwind or to a new
       [`unescaped_value_children`](../../parser/src/content/inline_builder/special_chars.rs) that
       reuses `classify_unescaped_specials`' own splitter. The condition is the step's *position*,
       not its presence, so an order that escapes **after** `Macros` takes the second half too —
       which brings the cross-reference family to parity there in both spellings. See the step's
       own "landed as" note above.

     - ✅ **prep (a trailing strip that cuts an escaped special).** The auto-link family's own
       last-named gap: a bare URL ending in a literal `&`, `<`, or `>` satisfies the
       trailing-punctuation strip on that special's *entity*, which the string replacer splits and
       the tree could not. A match boundary is answerable wherever a piece's match-string bytes are
       the bytes its own fold emits — the three
       [`CharRef`](../../parser/src/inlines/inline_node.rs) leaves and nothing else — so
       [`emit_range`](../../parser/src/content/inline_builder/quotes.rs) cuts one into two
       [`Raw`](../../parser/src/inlines/inline_node.rs) halves, each folding verbatim, and one new
       [`charref_entity`](../../parser/src/content/inline_builder/quotes.rs) answers both that
       classification and the bytes. The family's gate is deleted rather than relaxed (the stripped
       bytes are a literal `;`, `:`, or `)`, which no opaque piece can supply), and the formerly
       pinned divergence becomes a parity test. See the step's own "landed as" note above.

     - ✅ **prep (an escaped attribute-list bracket).** The passthrough-extraction pass's own last
       named deferral: `\[attrs]++text++`, where the string replacer writes the bracket back as a
       literal `[attrs]` prefix *and* stores the delimited text as an **ordinary** passthrough
       (its attribute list discarded), while
       [`find_passthrough_matches`](../../parser/src/content/inline_builder/passthrough_step.rs)
       could express one or the other. The shape its divergence note called one neither
       [`MacroMatchKind`](../../parser/src/content/inline_builder/macros/mod.rs) variant expresses
       turns out to be **two adjacent matches** — an `Unescape` over the bracket, then a `Node`
       over the delimited remainder — which
       [`rebuild_macro_level`](../../parser/src/content/inline_builder/macros/mod.rs) already
       composes, so no variant, gate, or signature moves. The node is the unattrlisted
       [`build_passthrough_node`](../../parser/src/content/inline_builder/passthrough_step.rs)
       reached with the delimited sub-range, so the discarded list is discarded by construction.
       The formerly pinned divergence becomes a parity test. See the step's own "landed as" note
       above. What this pass still defers is its **prohibited-prefix** retry.

     - ✅ **prep (the prohibited-prefix retry).** The passthrough-extraction pass's last named
       deferral, and its own last: `INLINE_PASS`' two attribute-list-prefixed options open with
       `\b{start-half}`, which does not exclude the `\`/`:`/`;` prefix Asciidoctor rejects with a
       lookbehind Rust's regex engine lacks, so
       [`InlinePassReplacer`](../../parser/src/content/passthroughs.rs) writes the rejected
       match's first character back and re-scans the rest of that same match. That retry is
       load-bearing rather than a re-confirmed rejection — it routinely finds a *shorter*
       construct the leading `[` was hiding (`index:[attrs]+text+` → a literal `[attrs]` and an
       **ordinary** passthrough over `text`) — so dropping the match, as the tree did, left real
       content literal. The capture loop moves into a
       [`collect_bare_pass_matches`](../../parser/src/content/inline_builder/passthrough_step.rs)
       that scans a sub-range of the level's match string, and the prohibited case calls it again
       over the match minus its leading `[` (always one ASCII byte, since both options open with
       `\[`); the bracket no match covers is an ordinary
       [`rebuild_macro_level`](../../parser/src/content/inline_builder/macros/mod.rs) gap, so no
       variant or shape moves — only an `offset` threaded into the two node builders that read
       capture offsets, since a retry's captures are relative to the slice it scanned. Writing
       **both** of this pass's escapes at once (`\[attrs]\++text++`) lands here too, and the
       formerly pinned divergence becomes a parity test. See the step's own "landed as" note
       above. What this pass still defers is nothing it named for itself; what remains is the
       sibling increments' boundary-class halves and the keeps.

     - ✅ **prep (an `indexterm2:[…]` attribute list).** The largest golden-exercised class the
       audit's remainder held after the passthrough pass closed its own two deferrals, and the one
       piece of it whose note named a blocker: an `indexterm2:[…]` argument carrying an `=`,
       deferred at part 4b "until the node can hold an `Attrlist<'src>`". The blocker had lapsed —
       an index term's whole render surface is
       [`IndexTermRenderParams`](../../parser/src/parser/inline_substitution_renderer.rs), which
       carries the shown term and nothing else, so the list decides only *which* of the argument's
       own bytes are shown and is **consumed** rather than carried. A new
       [`shown_macro_term`](../../parser/src/content/inline_builder/macros/indexterm.rs) does that
       where [`InlineIndextermReplacer`](../../parser/src/content/macros.rs) does — the same
       `Attrlist::parse` over the same normalized copy, the same `nth_attribute(1)`, the same fall
       back to the whole argument where the list has no positional attribute
       (`indexterm2:[see=HTML 5]`) — and no node field, variant, or gate moves. Two of the
       `asciidoctor` port's golden fixtures reach parity; the family's *other* deferral still
       decides first, so a shown term crossing an opaque span stays literal and keeps its
       divergence test, now written against the attribute-list spelling too. See the step's own
       "landed as" note above. What still defers in this family is that span-crossing term (the
       module-wide `range_is_verbatim` boundary) and the escaped paren-wrapped shorthand
       `\(((x)))`; beyond it the remainder is unchanged.

     - ✅ **prep (the escaped paren-wrapped shorthand).** The half of that closing sentence the
       index-term family actually owns. An escaped shorthand drops its backslash and stays
       literal, which is what the builder did for every spelling; the string replacer has one
       exception, and its own branch says why — *an escaped concealed term still processes a
       nested flow term*. Where the escaped match's `encl_text` is itself paren-wrapped,
       [`InlineIndextermReplacer`](../../parser/src/content/macros.rs) strips those parentheses
       off and renders what is left as a **visible** term between two literal parens, so
       `\(((x)))` collapses to `(x)` — the shape Asciidoctor's own
       `should only escape enclosing brackets…` golden spells, and where the audit's remainder
       was holding it. It is one match's source doing two things, so it becomes the *pair* the
       escaped-attribute-list bracket increment already answered with: an
       [`Unescape`](../../parser/src/content/inline_builder/macros/mod.rs) whose match **is** the
       backslash it drops, then a [`Node`](../../parser/src/content/inline_builder/macros/mod.rs)
       whose `consumed` sub-range stops one byte inside each end — the family's own
       kept-parenthesis narrowing, applied to both ends at once — which
       [`rebuild_macro_level`](../../parser/src/content/inline_builder/macros/mod.rs) composes as
       it composes any two adjacent matches. No variant, node field, gate, or signature moves;
       the nested term takes the visible branch's existing arithmetic because it *is* that
       branch. The formerly pinned divergence becomes a parity test that also asserts the shape.
       See the step's own "landed as" note above. What still defers in this family is now only
       the half that was never its own: a visible term crossing a rendered span, in every
       spelling, which is the module-wide `range_is_verbatim` boundary; beyond it the remainder
       is unchanged.

     - ✅ **prep (a visible term enclosing a rendered span).** The index-term family's last
       deferral, and the half the increment above named as never its own. A visible term shows
       its text in the flow, and the string replacer reads that text out of its own
       already-rendered haystack, so `((*tiger*))` shows `<strong>tiger</strong>` where the
       builder's match string holds one opaque placeholder. The shown text needed no string:
       nothing this family decides is read from it — `trim`, the ` >> ` / ` &> ` clause and the
       attribute-list `=` are all decided over the bytes as matched — so
       [`IndexTerm`](../../parser/src/inlines/index_term.rs) gains the `children`
       [`Ref`](../../parser/src/inlines/ref_node.rs) and
       [`Footnote`](../../parser/src/inlines/footnote.rs) already carry, and
       [`fold_index_term`](../../parser/src/content/inline_builder/fold.rs) folds them into the
       already-substituted string the seam takes — with the *surrounding flow's* renderer, so a
       custom backend writes the enclosed span too.
       [`shown_term_range`](../../parser/src/content/inline_builder/macros/indexterm.rs)
       re-expresses `normalize_index_text` and `strip_see_and_seealso` as a narrowing (the `\n`
       collapse becoming a one-space node, the `\]` unescape a gap), and
       [`TermSource`](../../parser/src/content/inline_builder/macros/indexterm.rs) carries the
       absorbed-paren spelling's two non-adjacent ranges. The computed string stays wherever the
       shown range holds no placeholder, so every spelling already at parity is byte-identical.
       Only the `Attrlist`-bearing argument still defers, its shown term coming back from a parse
       rather than a range; see the step's own "landed as" note above.

     - ✅ **prep (an attribute-list reference text enclosing a rendered span).** The capture the
       rendered-span class left behind, closed for the **cross-reference** family. A text carrying
       its own attribute list gets its display text back from an
       [`Attrlist`](../../parser/src/attributes/attrlist.rs) *parse* rather than from a range, and
       a parsed value is bytes — which a rendered span has none of until fold time. But the parse
       hands that value back to become the node's **children**, so it needs no bytes either, only
       a way to survive the split: a new
       [`tokened_text`](../../parser/src/content/inline_builder/macros/mod.rs) rewrites every
       opaque piece into the index-keyed token
       [`tokened_bracket`](../../parser/src/content/inline_builder/macros/image.rs) already uses,
       and [`restored_value_children`](../../parser/src/content/inline_builder/macros/mod.rs) —
       lifted out of the link family and shared — splices each **node** back in. Unlike a masked
       construct no token needs a *body*, which is the only thing that had made a span different;
       a `Ref{Xref}` carries `attrs: None`, so nothing rides on the list itself. The boundary is
       redrawn per **slot**: a piece reaching the `window=` / `role=` / `xrefstyle=` this family
       reads as a string still defers, decided in the gate where every cross-reference deferral is
       decided. A masked passthrough or STEM in such a text comes along for free. See the step's
       own "landed as" note above. The **image** family's bracket — the one capture with no
       display text to carry — is what remains of the class.

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
