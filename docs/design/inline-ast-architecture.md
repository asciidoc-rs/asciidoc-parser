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

- **Phase 4 — precision spans + ASG output.** Land the single-pass builder (Strategy B) and
  `Document::to_asg()`; validate against the ASG schema; retire the `attribute-missing`
  per-line hack (#564).
  *Exit:* ASG output validates; #944 hard-case policies documented and tested; #564 hack
  removed.

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
