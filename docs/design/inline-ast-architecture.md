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
    ///
    /// Takes `parser` as landed: a fold needs a `RenderContext`, whose
    /// path resolver and file handlers are `Rc<dyn …>` parse-wide
    /// configuration. Freezing those per content would cost `Content` — and
    /// with it `Document` — its `Send`/`Sync`, so the caller supplies the
    /// parser it already holds; only the *order-dependent* half (the
    /// document attributes) is retained per content. See step 7's
    /// "landed as" note.
    pub fn render_with(&self, renderer: &dyn InlineRenderer, parser: &Parser) -> String;

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
   consumer never gets it invoked *through* `rendered_html()`; they call
   `render_with(&their_renderer, &parser)`, or walk `inlines()` themselves — a pure fold they
   drive over the already-parsed tree. They can render the same document to several backends
   without reparsing.

   *A `Document::render_to` was sketched here alongside it and is **not** being built; see
   step 7's note.*

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
> through an explicit `render_with` fold and are the caller's to cache.
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

#### The escaping pass, and where escaped form stops

`main` closed a defect in the sentinel systems while this branch was rewriting them: a
document can *type* the codepoints the string pipeline reserves, so its own copies are
escaped before substitution begins and restored on the way out (`escape_sentinels` /
`unescape_sentinels`, [#1235](https://github.com/asciidoc-rs/asciidoc-parser/issues/1235)).
Merging that fix here settled two things worth recording.

**The tree needs no escaping, and gets none.** The single-pass builder recognizes
constructs by *range over the source*, never by scanning a rendered string for its own
marks, so a codepoint the document typed is never mistaken for one the parser wrote. The
one private-use codepoint the builder does use — `SPAN_PLACEHOLDER` (`\u{E0F0}`), standing
in for an already-recognized span inside a level's match string — is already handled the
same way: `passthrough_step` walks by *piece* rather than by character precisely so a
literal `+b\u{E0F0}c+` is not read as a placeholder. So the escaping is the string
pipeline's alone, applied at the two ends of `run_pipeline`.

**Escaped form is confined, and what leaves it is marked.** Three values outlive
`run_pipeline` in escaped form: the deferred placeholder template, this pipeline's own
cross-reference segments where the §4.2 carve-out keeps them, and a footnote catalog entry
the string replacer registered. Each has a reader that must hand the document its own text
back — a catalog lookup, an unresolved-reference warning, a template re-render — and *which
pipeline produced the value* is not recoverable from the text: unescaping a tree-derived
value would corrupt one that legitimately contains the escape introducer, which is the same
confusion the escaping exists to end. So the producer is carried rather than guessed:
`DeferredContent::from_tree` already said it for a content's segments (and `TitleNode` for a
title's), `FootnoteDeferred::sentinels_escaped` says it for a footnote's entry, and
`document_text` is the one place that asks. All three go with `run_pipeline` itself.

**And the decode is per-piece, not per-rendering.** Rendering a template splices the
**resolver's** answer — a destination, a reference text drawn from the catalog — into the
pipeline's own escaped text. The resolver was handed the document's own text and answered in
kind, so its bytes are not in escaped form; decoding the *finished* rendering in one pass
decodes them too, and an id such as `#a\u{E004}b` comes out as `#a\u{E001}`. So
`render_template` leaves escaped form run by run as it walks — the template's literal text
and the four segment fields the substitution itself read back (`target`, `provided_text`,
`window`, `roles`), never `resolved` or `derived` — and nothing decodes the result. That in
turn makes `finalize_deferred`'s rebuild the *first* way out of escaped form, so
`run_pipeline`'s own tail decode is gated on there being nothing deferred; otherwise a
content that both defers a reference and types the escape introducer is decoded twice.

The branch's reserved set is correspondingly **two** systems, not three: `\u{E002}` /
`\u{E003}` are absent from `RESERVED_SENTINELS`, because the footnote-marker system is
already gone — a heading's reference text is a second fold of its own tree
(`fold_reference_text`), so nothing reserves those codepoints to escape a document out of.

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
  `IsBlock::inlines()`, the public node types, and `render_with`. Rename
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
  pieces of the phase — the `rendered()` → `rendered_html()` rename and the `render_with`
  fold — remain as later steps.

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
    not yet byte-faithful; only the returned *number* is relied on. Both halves of that — the
    approximate `text` and the premise behind it — held at the time and were closed by a step 6 prep
    below, which finds the entry is a *required* side effect whose payload is a rendered string, so
    it is the one thing a build has to fold.

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
  [a `pass:` macro with an explicit substitution list](../../parser/src/content/inline_builder/passthrough_step.rs), is
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
  defers with its own divergence test.

  The token is also what makes the *split* reproducible — a bracket's `,` / `=` / `"` are the
  only bytes it reads, and a placeholder carries none of them — but the replacer splits over the
  piece's own **markup**, which may: `xref:sec[a *b, c* d,role=hl]` renders
  `a <strong>b, c</strong> d`, whose list splits at the comma *inside* the tag and ends the
  anchor there. Rather than guess at that from the bytes — an ordinary `*bold*` hides nothing,
  while `[.r]#x#` renders a `"` and an `=` the split reads harmlessly — the gate asks the parser:
  [`tokened_split_agrees`](../../parser/src/content/inline_builder/macros/mod.rs) parses the
  tokened text *and* the restored markup and compares them attribute by attribute, expanding the
  tokened side's tokens first, so a split that moved shows up as a value that differs. Where the
  two readings differ about the match's own extent the match is **deferred**, not recognized and
  pinned: the tree never claims a construct the rendered document does not agree with. Deciding it means making the same tokened parse the builder
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

  *Step 6 prep landed as (a `link:`/`mailto:` macro whose own marker is not verbatim — the
  family's last non-verbatim rule):* the one boundary in this module drawn around neither a
  *piece* nor a *capture* but around the node's own **`location`**. Three passes recognize a
  link — the auto-link / formal-URL family, the `link:`/`mailto:` macro, and the bare e-mail
  address — and the string pipeline runs them as three passes over the whole content, so the
  asset catalog fills in *pass* order rather than document order.
  [`apply_link_side_effects`](../../parser/src/content/inline_builder/macros/links.rs) replays
  that by walking the tree three times, which needs to know which pass built each node; it
  answered by reading the node's `location` back and asking whether it starts with a literal
  `link:` / `mailto:`. That made the location load-bearing, so a macro whose own marker is not
  verbatim source had to be **deferred**: a *wholly* expanded macro (`:m: link:index.html[Docs]`,
  then `{m}`), one reached through a wholly-synthesized seed, and one written inside markup an
  earlier step of the same order flattened — three separate documented divergences, each pinned
  with the same "if this boundary is ever lifted (with a signal that does not depend on the
  location), fold this fixture into the parity corpus" note.

  The signal is the node's to carry, and Phase 4's whole business is making nodes
  **self-describing**. [`Ref`](../../parser/src/inlines/ref_node.rs) gains
  `link_form: Option<`[`LinkForm`](../../parser/src/inlines/ref_node.rs)`>`, set by whichever
  pass builds the node and `None` for a cross-reference, and the registration walk reads it
  instead of re-deriving anything. The field earns its place on its own terms rather than as an
  internal marker: the three spellings render alike but are *written* differently, so a consumer
  writing AsciiDoc back out needs exactly this to reproduce the source, and one reporting on a
  document can tell an explicit macro from an automatically-recognized URL — the same kind of
  fact an [`Image`](../../parser/src/inlines/image.rs)'s `is_icon` and an
  [`Anchor`](../../parser/src/inlines/anchor.rs)'s `is_bibliography` already carry. With it the
  `range_is_verbatim` call on the marker is **deleted rather than relaxed**, and the private
  `link_form` function with it.

  All three formerly pinned divergences become parity tests. What reaches parity is the whole
  macro from one expansion alone and in flow, the `mailto:` spelling, a macro whose marker and
  target both come from expansions, one beside the two other link spellings (whose own
  registration passes run before and after this one), one inside a rendered span, one in a
  wholly-synthesized seed (a filtered multi-line block), and one inside flattened markup under
  `subs=quotes,specialcharacters,macros`. A separate corpus drives the **registration order**
  through the real pipeline on both sides — the thing the location used to be read for —
  including an expanded macro interleaved with a bare URL and a bare address, so the three-pass
  order is pinned for the newly-recognized form as it already is for the verbatim one. A
  structural test asserts the shape: a `Macro`-form node whose `location` is the whole attribute
  reference, design §4.4's coarse span. The recorder cross-check adds `link_form` to its list of
  one-sided-richness exemptions, beside `is_icon` and an anchor's `reftext`: all three spellings
  render to the same `<a …>` markup, which is all the recorder has to recover from.
  Re-running the corpus-wide fold-parity audit shows both golden-exercised fixtures **gone** from
  the divergence set and no new divergence; coverage is diff-neutral, and nothing further is
  wired in.

  With this the `link:`/`mailto:` family draws no boundary the other macro families do not. What
  remains of the rendered-span class is the **image** family's attribute list, the one capture
  with no display text to carry; beyond it the remainder is unchanged — the boundary-class halves
  the sibling increments named, plus the keeps.

  *Step 6 prep landed as (a bare `+…+` body enclosing an already-extracted passthrough):* the
  passthrough-extraction step runs as **two passes** — `INLINE_PASS_MACRO` (`+++…+++`, `++…++`,
  `$$…$$`, `pass:[…]`), then `INLINE_PASS` over what it left behind — so a bare `+…+` body can
  enclose a construct the first pass already replaced. Two documented AsciiDoc idioms spell it,
  and the language docs' own sentence exercises both:
  `` +Sometimes you feel pass:q[`mono`].+ Sometimes you +$$don't$$+. ``

  The string pipeline sees its own **sentinel** there and does the least interesting thing
  possible with it: it treats it as ordinary body text, applies the verbatim substitution to the
  body *with the sentinel still in it*, stores that as this passthrough's own entry, and lets the
  final restore splice the inner body in afterwards. The builder was reading its body through
  [`source_slice`](../../parser/src/content/inline_builder/quotes.rs) and gating on
  [`range_is_verbatim`](../../parser/src/content/inline_builder/macros/image.rs), so an
  already-built [`Raw`](../../parser/src/inlines/inline_node.rs) leaf in the body — atomic, with
  no `'src` slice of its own — deferred the whole match and left the `+` delimiters literal.

  It reproduces that order exactly instead. The body is read from the level's **match string**,
  where the already-built leaf stands as one
  [`SPAN_PLACEHOLDER`](../../parser/src/content/inline_builder/quotes.rs) — the same shape the
  sentinel has — and a new
  [`substitute_and_restore`](../../parser/src/content/inline_builder/passthrough_step.rs) walks
  the body's own [`Piece`](../../parser/src/content/inline_builder/quotes.rs)s: each run of
  ordinary text between two restorable pieces goes through
  [`passthrough_text`](../../parser/src/content/inline_builder/passthrough_step.rs) on its own,
  and each piece contributes what the fold of that node emits
  ([`restorable_body`](../../parser/src/content/inline_builder/macros/image.rs)) verbatim.
  Substituting first and splicing after is the whole point: it is what keeps an inner `<b>` from
  being escaped a second time. Substituting run by run gives the same bytes as substituting the
  whole body at once, because the verbatim group is `specialcharacters` alone — a per-character
  map, so it distributes over concatenation and no match can span a run boundary.

  Walking by *piece* rather than scanning the substituted string for the placeholder character is
  what makes the splice unambiguous. `SPAN_PLACEHOLDER` is an ordinary private-use character, so a
  source can spell one **literally** (`+b\u{E0F0}c+`), and a scan reads the two alike: it would
  splice a node's body at the literal one and then run out of bodies before the real placeholder,
  silently dropping everything after it. The pieces say which is which; the literal character is
  ordinary text inside a run and comes through untouched. Pinned at either edge, alone, twice in
  one body, and on both sides of a real restored body.

  The gate becomes
  [`range_is_restorable`](../../parser/src/content/inline_builder/macros/image.rs) — nothing else
  can reach this pass, which runs before the escaping, quotes, and macros steps, so a `CharRef`
  leaf or a rendered span does not exist yet; a
  [`synthesized`](../../parser/src/content/inline_builder/quotes.rs) run comes along too, since
  the body no longer slices `'src`.

  What reaches parity is both documented idioms, each delimited form the first pass recognizes
  inside the body, a restored body carrying markup (emitted once, not escaped twice) beside one
  whose own specials the substitution *does* escape, the extracted construct at either edge and as
  the whole body and twice in one body, a STEM expression (the other node kind the one extraction
  pass produces), and the escaped attribute-list bracket's own **retry**, whose re-scan finds a
  shorter bare form that may itself enclose one — which is how
  `['role']\+++++++++This++++++++++++` reaches parity as a second closed golden fixture without a
  line of its own. The forms already at parity are unchanged. A structural test asserts the shape:
  one `Raw` leaf whose value already carries the inner body, exactly as the string pipeline's own
  entry does by restore time. The two **attribute-list-prefixed** bare forms keep the verbatim
  gate — their attribute list is parsed from an `'src` span and the `x-` spelling re-runs a whole
  nested `Normal` build over the body — so `[method x-]+pass:[<b>]+` stays deferred with its own
  test. The structural recorder sweep deliberately omits this shape: the builder makes one leaf
  where the recorder, recovering structure from the finished string, sees the inner construct's
  own markers and splits the same text into several — the leaf-boundary asymmetry that module's
  doc comment already describes, not a divergence.

  Re-running the corpus-wide fold-parity audit shows **two** golden fixtures gone from the
  divergence set and no new divergence; coverage is diff-neutral, and nothing further is wired in.

  *Step 6 prep landed as (an attribute-list display text enclosing a rendered span — the two
  link families):* the same capture the cross-reference family closed, for the two families that
  hold a real [`Attrlist`](../../parser/src/attributes/attrlist.rs)`<'src>` on the node. Their
  display text comes back from the same parse, so it needs the same token; what was different is
  that a token has to *leave* that parse with something in it, since
  [`Ref::attrs`](../../parser/src/inlines/ref_node.rs) rides on the node and
  `render_link` reads more out of it than the display text.

  A masked construct always had a body for that —
  [`Passthroughs::restore_to`](../../parser/src/content/passthroughs.rs) splices exactly those
  bytes into the string pipeline's own finished string, so restoring one into a parsed value is
  faithful wherever the value goes. A rendered span has no such body, and the design has said so
  since the class was opened: its markup exists only at fold time. The finding is that it has one
  for *this* purpose. [`tokened_bracket`](../../parser/src/content/inline_builder/macros/image.rs)
  gains a [`Tokened`](../../parser/src/content/inline_builder/macros/image.rs) kind; under
  `MaskedOrRendered` it admits any opaque piece and pairs it with the **build-time fold**, taken
  with the parser's own renderer — the same trade
  [`restorable_body`](../../parser/src/content/inline_builder/macros/image.rs) already makes for a
  `Stem`, and the very bytes the string replacer's own attribute list holds there.

  What makes that safe is that the frozen bytes never reach output. The display text is carried
  **structurally** — [`restored_value_children`](../../parser/src/content/inline_builder/macros/mod.rs)
  splices each piece's own *node* into the children, so the fold renders the span with whatever
  renderer the fold is using — and the frozen copy only ever lands in the node's `attrs`, which
  no renderer reads for the display text. Every *other* slot is one `render_link` writes out (an
  `id`, a `title`, a `role`, a `window`, an option), where a frozen body would put the built-in
  backend's markup in a custom backend's output, so a token reaching one defers the whole match:
  [`rendered_token_escaped_the_display_text`](../../parser/src/content/inline_builder/macros/links.rs)
  draws that per-**slot** boundary, and a *masked* piece is exempt from it for the reason above.
  The split-agreement check
  ([`tokened_split_agrees`](../../parser/src/content/inline_builder/macros/mod.rs)) applies here
  too, and one more thing falls out of writing the walk this way: `tokened_bracket` now walks the
  level **piece by piece** rather than copying the gaps between the tokened ones, because a byte
  of the match string may belong to no piece at all — `styled_sibling_boundaries` wraps a
  placeholder in the two characters its rendering presents to a neighbour, and copying them would
  splice a stray `<` and `>` into the parsed value.

  What reaches parity is the golden spelling from the language docs
  (`https://chat.asciidoc.org[*project chat*^,role=green]`), each family with the span at either
  edge and in the middle, each named attribute `render_link` reads with the span in the positional
  value beside it, other span kinds and two spans in one text, the `^` new-window suffix riding
  after the span, and a collapsed newline. Fixtures join the whole-pipeline broad sweep and
  combined-constructs corpus and the structural recorder cross-check. Re-running the corpus-wide
  fold-parity audit shows that golden fixture **gone** from the divergence set and no new
  divergence; coverage is diff-neutral, and nothing further is wired in.

  With this the rendered-span class is closed for every family that has a display text to carry.
  What remains of it is the **image** family's attribute list, the one capture with none: every
  value its bracket holds — an `alt`, a `title`, a `width` — is one `render_image` writes out, so
  the per-slot rule above puts all of them on the wrong side. Closing it needs a value the fold
  *materializes* rather than one frozen at build time, which is a shape no increment has needed
  yet; it is left for the maintainer to weigh against the cutover, and the golden it holds
  (`image:pause.png[title=*Pause* and Resume]`) keeps its divergence test. Beyond it the
  remainder is unchanged: the boundary-class halves the sibling increments named, plus the keeps.

  *Step 6 prep landed as (a section title's footnote-free reference text — the first sentinel
  system to lose its reason to exist):* the increments above were all found by asking the
  corpus-wide fold-parity audit what still diverges. This one was found by asking a different
  question — *what breaks if `rendered_html()` becomes the fold today?* — run as a throwaway
  experiment over the whole suite. The answer is encouragingly small, and its largest single
  cluster is one root cause: a **footnote in a section heading**.

  A footnote in a heading is a real, document-order footnote — it is numbered, and the heading
  renders its marker — but that marker must not appear in the text a cross-reference to the
  heading shows, nor in the id auto-generated from that text (issue #594). The string pipeline
  needs two strings out of one title and cannot afford two renders (counters and
  attribute-expanded footnotes would advance twice), so it renders **once** with the footnote
  renderer bracketing each marker in a sentinel pair
  ([`FOOTNOTE_MARKER_START`](../../parser/src/content/content.rs) …
  [`FOOTNOTE_MARKER_END`](../../parser/src/content/content.rs), enabled for that render alone by
  `Parser::mark_footnote_spans`), cuts the bracketed regions out
  ([`strip_footnote_marker_spans`](../../parser/src/content/content.rs)), and then removes the
  spent sentinels from the title's own rendering *and* from its deferred cross-reference template
  ([`Content::remove_footnote_marker_sentinels`](../../parser/src/content/content.rs)).

  A tree needs none of that. The footnote is a **node**, so the two strings are two folds of the
  same tree, and "which regions were footnote markers" is a question about node kinds rather than
  about bytes. [`fold_reference_text`](../../parser/src/content/inline_builder/fold.rs) is
  [`fold_html`](../../parser/src/content/inline_builder/fold.rs) with each footnote's in-flow
  marker omitted — the node **skipped** rather than folded to nothing, since `render_footnote` is
  a backend's to define and one that emitted anything would leak it — and the omission recurses
  exactly as the byte-level cut does over the whole rendered string, so a footnote inside a
  rendered span or inside a cross-reference's own display text is omitted there too. Nothing else
  about the fold changes: a `Footnotes` parameter threads down the recursion with `Marked` at
  every existing call site.

  A differential test pins the equivalence from **both** ends, over a corpus of titles: for each,
  `fold_html` reproduces the heading's own rendering (sentinels removed) and `fold_reference_text`
  reproduces the footnote-free text, each against the string pipeline's own two answers computed
  the way `Section::parse` computes them. The corpus covers a title with no footnote at all
  (where the two strings coincide and the strip is a no-op on both sides), one at either edge and
  in the middle, two in one title, a named footnote, a marker nested inside a rendered span and
  inside a cross-reference's display text, and a footnote beside each other construct a title can
  carry.

  This is the **first of the three sentinel systems** §4.2 names to lose its reason to exist.
  Deleting it — along with `mark_footnote_spans`, `strip_footnote_marker_spans`, and
  `remove_footnote_marker_sentinels` — is the cutover's own job, so as with every prep piece
  before it nothing is wired in and the corpus-wide audit is unchanged.

  *What the experiment says about what is left.* Forcing the tree on for every parse and making
  `rendered` the fold leaves **15** of ~5,370 tests failing, in three clusters: this footnote
  cluster (5), the documented keeps whose golden output the cutover deliberately changes for the
  better (~5, each already carrying its own divergence test), and tests of the opt-in flag and of
  the Strategy-A recorder harness that retire with the flag itself (~5). That is a far smaller
  blast radius than "several sessions" suggested when this step was written — the prep work has
  done its job — though the experiment is deliberately crude (it neither calls the staged side
  effects nor sequences the fold against resolution), so the cutover's own ordering work is not
  measured by it.

  *Step 6 prep landed as (a whole-document parity harness for the builder, after resolution):*
  the increment above measured what the cutover breaks; this one closes the gap that measurement
  exposed in what the branch *verifies*.

  Two corpus-wide harnesses meet at this step and neither covers it. The builder's own corpora
  drive [`SubstitutionGroup::apply`](../../parser/src/content/substitution_group.rs) on a bare
  [`Content`](../../parser/src/content/content.rs), which has no document catalog and therefore
  **cannot resolve cross-references** at all. The whole-document sweep that does reach resolution
  ([`inline_recorder`](../../parser/src/tests/inline_recorder.rs)'s `check_document`) drives the
  *Strategy-A recorder*, which the first half of this step retired to test-only oracle machinery.
  So the one property the deferred-cross-reference sentinel system's retirement rests on — **the
  single-pass builder's tree, folded *after* resolution has run, reproduces the rendered string
  byte-for-byte** — was asserted only by the handful of hand-written whole-document tests
  individual increments added for their own shapes.

  [`inline_builder_document_parity`](../../parser/src/tests/inline_builder_document_parity.rs)
  asserts it over a corpus. It needs only **one** parse, which is what makes it simpler than its
  recorder counterpart: with [`with_inline_tree`](../../parser/src/parser/parser.rs) on, every
  content location carries both its rendered string and its tree, so the two are compared
  directly rather than reconstructed from a second, renderer-perturbed parse.

  The walk reaches every location a tree lives in, and reaches it through the *one pair of
  accessors that answers for every block kind* —
  [`IsBlock::rendered_html_content`](../../parser/src/blocks/is_block.rs) and its structured
  counterpart `inlines` — rather than by matching per variant. That is what keeps a corpus-wide
  sweep from silently narrowing: a block kind nobody thought to name is covered because it
  implements the trait. Beyond a paragraph's content it therefore reaches an `admonition` and the
  whole raw-delimited family (`listing`, `literal`, `pass`, whose verbatim groups run a *different*
  step selection through `build_for_group`). Two locations are not a block's own content and are
  named explicitly: a section title (whose own document-order pass in
  [`title_refs`](../../parser/src/document/title_refs.rs) mutates the title's tree, and whose
  `SectionBlock` implements neither accessor, a section's content being its children), and a table
  cell, which is not a block at all — and a **block title** (`.Title`), which is substituted content
  in its own right, carries its own tree, and belongs to the block without being its content. Every
  block kind that can carry one is named by a read-only counterpart of the mutable accessor the
  document-order title pass already uses. A compound delimited block — a quote, an example — needs
  no entry of its own: it carries no content directly, and the paragraphs inside it are reached by
  the walk's own recursion. A footnote's own subtree is matched against the catalog's registered texts in
  document order.

  The corpus is built around resolution, since that is the thing nothing else exercised: both
  cross-reference spellings resolving in block content, a mix of resolved and unresolved in one
  document, a reference to a *section* (whose destination text is the heading's own reference
  text), forward and circular references between headings, a reference inside a rendered span and
  inside a formatted heading, footnote-embedded references both resolved and unresolved, a
  footnote in a heading beside a reference to that heading, a named footnote defined once and
  referenced again, table cells, and nested blocks. Two guards keep it from passing vacuously: a
  fixture must fold at least one **non-empty** tree, and the sweep must reach both branches of
  the fold's `resolved` handling — a reference that carries its destination and its text, and one
  that carries the bracketed fallback. A third pins the *set* of location kinds the sweep reaches,
  so a walk that stopped visiting one, or a fixture that stopped producing one, shrinks the set and
  fails rather than passing over a location nobody looked at.

  It passes as written, which is the result worth recording: §4.3's claim that resolution "walks
  the tree and fills `resolved` in place — non-destructive by construction, re-resolvable, no
  template" now has a corpus behind it rather than a design argument. A second test pins the
  property `render_with` will rest on (§3.3.1): the same tree folded twice, and folded through a
  renderer handed out as a shared `Rc` the way
  [`Parser::with_inline_substitution_renderer`](../../parser/src/parser/parser.rs) installs one,
  gives the same bytes. This is test-only: nothing is wired in, and the corpus-wide fold-parity
  audit is unchanged.

  *Step 6 prep landed as (a corpus-wide side-effect parity harness, and the index term it found):*
  the increment above closed one half of what the blast-radius experiment left unmeasured — it
  "neither calls the staged side effects nor sequences the fold against resolution". This one
  closes the other half, and unlike its sibling it does **not** pass as written.

  Recognizing a construct is not only about the bytes it renders. Four passes of the string
  pipeline also write down what they saw — an image's target and a link's in the asset catalog, an
  inline anchor's and a bibliography entry's id in the reference catalog, and a dangerous `link=`
  scheme in the shared warnings list — and step 6 has to replay all four from the tree, exactly
  once per parse and in the string pipeline's own pass order. That is what
  [`apply_macro_side_effects`](../../parser/src/content/inline_builder/macros/mod.rs) is staged to
  do, and until now it was pinned only by hand-written fixtures inside its own module, one per
  ordering rule it has to honor.

  [`inline_builder_side_effect_parity`](../../parser/src/tests/inline_builder_side_effect_parity.rs)
  drives a corpus through both sides on two independently-configured parsers (§5.3's discipline —
  each side registers into the parser it is given, so a shared one would see every entry twice and
  fire every duplicate-id warning spuriously) and compares *everything either side wrote down*:
  catalog entries in registration order, ids with their reftext and kind, and warnings in the
  order the one shared list received them. The corpus covers each family alone and in company, the
  link family's three-pass order (which fills the catalog in *pass* rather than document order),
  an `imagesdir` in force, a registration reached from inside another construct's subtree, ids
  duplicated across two families, and — the shapes most likely to over-register — an escaped
  macro, one sealed in a passthrough, and one in monospace, each beside a live twin so an
  over-eager replay shows up as a length mismatch rather than as two empty lists. Three guards
  keep it honest: the corpus as a whole must write something, each of the four lists must be
  reached, and the fixtures that write *nothing* are named rather than counted — `icon:` among
  them, which shares the image family's pass and node kind but is not an asset.

  What it found is one root cause with two faces. [`IndexTerm`](../../parser/src/inlines/index_term.rs)
  is the **fourth** node kind carrying `children` — added by the visible-term increment above, after
  all three side-effect walks were written — and none of the three descended into it, so an image,
  link, or anchor a visible term encloses was recognized, rendered, and then registered nowhere.
  The walks now recurse into it, and their doc comments say why four is the whole list: a fifth
  child-bearing kind would be a new place a macro node can hide, and this sweep is what catches one.

  The other face is a **recognition** gap the same fixtures expose, and it is not this increment's
  to close. The string pipeline replaces a visible term with its shown text and nothing else, so
  that text goes on sitting in the one flat haystack every later pass scans: a `link:` macro, a
  bare URL, an anchor, or a cross-reference written inside `((…))` is recognized by the pass that
  runs *after* the index-term pass, exactly as if the parentheses were not there. A tree cannot do
  that with the shown text it currently keeps — a plain visible term's is an already-substituted
  *string* in `terms`, not a subtree — so the families after this one have no nodes to descend
  into. (The families *before* it are unaffected, which is why the image half of the sweep passes:
  their construct is already a node when the term encloses it, and rides along in `children`. So
  is a construct inside a rendered span the term encloses, whose children this step resolves in
  full before any of this level's families run.) Closing it needs a visible term's shown text to be
  nodes in **every** case, not only when it encloses a rendered span, *and* the families after this
  one to descend into them — a change to what the node carries, so it is its own increment.
  Deferred and pinned by a divergence test, with its parity complement beside it.

  This is the first divergence found by asking about side effects rather than about bytes, and it
  was invisible to the fold-parity audit for a reason worth recording: the audit's corpus had no
  fixture putting a later family inside a visible term, so the *rendering* half of the same root
  cause had gone unseen too. Re-running the audit shows no change to the divergence set, since the
  shapes that expose it are new here. Coverage is diff-neutral; nothing further is wired in.

  *Step 6 prep landed as (a visible index term's shown text, handed back to the later families):*
  the recognition half the sweep above named, closed for the shorthand spellings.

  A visible term's shown text is not a boundary the other macro families stop at. The string
  replacer replaces `((term))` with that text and nothing else, so the text goes on sitting in the
  one flat haystack every later pass scans: a `link:` macro, a bare URL or address, an inline
  anchor, or a cross-reference written inside `((…))` is recognized by the pass that runs after the
  index-term pass, exactly as if the parentheses were not there.

  Two things had to change for a tree to say the same. First, what the node **carries**:
  [`shown_term`](../../parser/src/content/inline_builder/macros/indexterm.rs) built a `children`
  subtree only when no already-substituted string could express the shown text (a term enclosing a
  rendered span), and a plain term's text was a string in
  [`terms`](../../parser/src/inlines/index_term.rs) alone — nothing for a later family to descend
  into. It now builds **both**, from the same range: `children` is the shown text's authoritative
  form (a term's text is a region of the document, not an opaque string), and `terms` carries the
  same text as the single string
  [`IndexTermRenderParams`](../../parser/src/parser/inline_substitution_renderer.rs) takes whenever
  one can express it. The two agree by construction —
  [`shown_term_range`](../../parser/src/content/inline_builder/macros/indexterm.rs) answers the
  normalization as offsets and `emit_shown_term_range` performs the two remaining rewrites
  structurally — which is what makes carrying both a widening rather than a choice, and leaves
  every existing `terms` reader working.

  Second, **who sees them**: the five families that run after this one (the three link spellings,
  inline anchors, cross-references) move into an
  [`apply_reference_families`](../../parser/src/content/inline_builder/macros/mod.rs) that
  [`apply_macro_families`](../../parser/src/content/inline_builder/macros/mod.rs) now calls twice —
  once for this level, and once per visible term, over the term's own children. Their own level
  rather than one flat scan, because a term renders its shown text with no markup of its own, so
  its children read what stands beside the *term* — exactly the transparent case
  [`LevelContext::child_contexts`](../../parser/src/content/inline_builder/quotes.rs) already
  answers, and the same treatment a transparent span's children get.

  Making `children` the common case immediately surfaced a **third** face of the sweep's own root
  cause, in a walk nobody had thought to check:
  [`classify_unescaped_specials`](../../parser/src/content/inline_builder/special_chars.rs) — which
  under an order omitting `specialcharacters` turns a literal `<`/`>`/`&` into a `Raw` leaf — also
  did not descend into `IndexTerm`, so `an escaped \(((a & b))) term` under a `Macros`-only group
  escaped an ampersand the string pipeline leaves alone. The existing `build_for_group` corpus
  caught it the moment the term's text became nodes.

  What reaches parity is each later family inside a term (`link:`, `mailto:`, a bare URL, a bare
  address, `[[id]]`, `anchor:id[…]`), at either edge of the shown text and as the whole of it,
  twice in one term, two families in one term, beside a twin outside the term so the pass order
  that fills the asset catalog is exercised from both sides, the two paren-keeping spellings whose
  shown text is a narrowing, inside a rendered span and with one inside the term, and the nested
  `((… ((term)) …))` shorthand. A cross-reference cannot be asserted this way — `golden_macros`
  runs a bare `Content`, which has no catalog, so *every* `xref:`/`<<…>>` is left as the deferred
  sentinel there — so a whole-document test drives both spellings through the real parse path
  instead, in block content and in a heading's own title. The side-effect sweep gains the same
  shapes on its side.

  What still defers is the **macro** spellings (`indexterm:[…]`, `indexterm2:[…]`), which keep
  their shown text as a string alone: an attribute list's shown term is the value `Attrlist::parse`
  returns for its first positional attribute, which is not a range of this level's match string, so
  nodes built from that range would not agree with it — and this family cannot tell that case from
  the plain one before the list is parsed. Closing it needs the attribute-list narrowing itself
  expressed as a range of the match string, the way `shown_term_range` already expresses `trim` and
  the `see` strip; pinned by its own divergence test. The structural recorder sweep's index-term
  comparison is relaxed to match the widened node: `children` is one-sided richness it never
  compares, and `terms` is compared wherever the builder computed one.

  Re-running the corpus-wide fold-parity audit shows no change to the divergence set; coverage is
  diff-neutral, and nothing further is wired in.

  *Step 6 prep landed as (an anchor's reference text — the fifth nested node list):*
  the side-effect sweep's own doc comment claimed the walks reach "exactly the four node kinds that
  carry `children`". Asking that claim the sweep's own question — *is there a fifth?* — turns up one
  immediately, and it is the one a walk written by matching on `children` is bound to miss:
  [`Anchor::reftext`](../../parser/src/inlines/anchor.rs) is a nested node list that is not named
  like one.

  Adding a corpus row for it exposed **two** divergences at once, on every anchor spelling
  (`[[id,…]]`, `anchor:id[…]`, `[[[label]]]`). A construct the reference text encloses
  (`[[a,see image:t.png[T]]]`) was not registered — the walks did not descend there — and the
  registered **reference text itself** differed: the string replacer catalogs the text it has
  already rendered (`see <span class="image">…`), where the builder catalogued nothing at all.

  Both come from one place.
  [`build_anchor_reftext`](../../parser/src/content/inline_builder/macros/anchors.rs) deferred a
  reference text crossing an atomic piece, leaving the field `None` — the same class every sibling
  family has closed in turn, and closed the same way here: the text is carried **structurally**, as
  the nodes the range covers, which is what the field's own `Vec<InlineNode<'src>>` type has always
  allowed. The two byte rewrites the verbatim path performs with `str` methods become ranges (a
  shorthand's `trim_end` narrows the range, since trailing whitespace is ordinary text and never a
  placeholder; a macro's escaped `\]` drops its backslash as a gap between two emitted ranges, the
  same structural unescape the reference-bearing families already use), and the verbatim path keeps
  its exact prior shape, including its precisely-sliced `location`.

  With nodes there, the registered string is a **mixed** fold, and the mix is the interesting part:
  a reference text's own `Text` runs carry the level's *match-string* bytes — already substituted,
  since a reference text is read after the escaping and quotes steps have run — so folding one would
  escape it a second time (`[&#169; 1995]` → `[&amp;#169; 1995]`). Those contribute their value as
  it stands, exactly as the field's original single-`Text` reader gave it; only an enclosed
  construct, whose bytes exist nowhere until the tree is folded, is folded, through the parser's own
  renderer. That trade is safe for the same reason it is in the link families: the bytes go into the
  **catalog** rather than straight to output, and a cross-reference reaching them is rendered by
  this same renderer.

  Nothing about an anchor's *rendering* changes — `render_anchor` emits the id and nothing else, so
  the fold-parity audit could never have seen any of this. What changes is what a **cross-reference
  to the anchor** shows, which a whole-document test now drives end to end. The formerly pinned
  divergence (`[[id,*bold*]]` leaves `reftext` unpopulated) becomes a structural parity test.

  One shape was checked and deliberately left alone: the footnote pass does **not** descend into a
  reference text, and must not — the anchor replacer consumes that text rather than emitting it, so
  a `footnote:[…]` written there never reaches the string pipeline's footnote pass either, and both
  sides agree that nothing is numbered.

  Re-running the corpus-wide fold-parity audit shows no change to the divergence set; coverage is
  diff-neutral, and nothing further is wired in.

  *Step 6 prep landed as (footnote numbering in true source order):* the increment above was found
  by asking the side-effect sweep's own claim whether it was true. This one was found the same way,
  of a different claim — and this claim was **false**.

  [`apply_footnotes`](../../parser/src/content/inline_builder/footnotes.rs) exists for exactly one
  reason, which its own doc comment states: every other macro family is recognition-order
  independent, but a footnote's assigned **number** is a side effect of recognition order itself, so
  this pass "walks `nodes` in true source order … recursing into a `Styled`/`Ref` child at exactly
  the point that child falls between two such recognitions". It did not. `find_footnote_matches`
  **built** every node during its regex scan of the level — assigning every number at this level
  before `rebuild_footnote_level` descended into any child — so a footnote nested in a child that
  falls between two of its level's own was numbered *after* both:

  ```
  before footnote:[a] *span footnote:[b]* after footnote:[c]
      string pipeline → 1, 2, 3
      tree (before)   → 1, 3, 2
  ```

  Every existing corpus missed it, for a reason worth recording: a nested footnote *before* every
  sibling, or *after* every sibling, numbers the same either way. Only the between position
  disagrees, and no fixture anywhere had one. The fold-parity audit could not have found it either
  — its corpus is the same fixtures.

  The fix is to defer construction. `find_footnote_matches` becomes a pure scan returning a
  `FootnoteMatch` per occurrence — an escape (decided during the scan, since the string replacer's
  own `starts_with('\\')` check runs before its ref-vs-plain branch) or a **candidate** carrying its
  capture — and `rebuild_footnote_level` builds each candidate's node at the moment its walk reaches
  it, immediately after the gap before it has been emitted and recursed into. A candidate that turns
  out to be one of the unrecognized forms advances the cursor no further than its own start, so its
  text joins the following gap exactly as it did when the scan dropped such a match: the same bytes,
  emitted by the same range walk.

  The same pass gains the container the increments above named:
  [`IndexTerm`](../../parser/src/inlines/index_term.rs), whose visible term's shown text reaches the
  flow and so is scanned by the string replacer's footnote pass like any other text. An
  [`Anchor`](../../parser/src/inlines/anchor.rs)'s `reftext` is deliberately **not** descended into,
  and the asymmetry is the point: the anchor replacer *consumes* that text rather than emitting it,
  so a `footnote:[…]` written there never reaches the string pipeline's footnote pass either — a
  fixture pins that both sides number nothing.

  The corpus is built entirely around the between position, since that is the only arrangement the
  two orders disagree about: each container with a plain sibling on either side, nested two deep,
  two children each carrying one, a child carrying two of its own, a named footnote reused from
  inside a child (whose number is taken rather than assigned), and a child carrying an unrecognized
  form beside a real one, so the cursor handling for a candidate that does not build is exercised in
  a nested walk too. Two fixtures join the whole-document harness, where the catalog's footnote list
  is in document order — so a slip fails the *subtree-to-text pairing* there, not only the fold.

  Re-running the corpus-wide fold-parity audit shows no change to the divergence set; coverage is
  diff-neutral, and nothing further is wired in.

  *Step 6 prep landed as (a cross-product sweep: every construct in every container):* the three
  increments above each closed a walk that failed to descend into a container — a visible index
  term's shown text, an anchor's reference text, a footnote nested in a child — and in each case
  the reason no corpus caught it was the same. **The construct and the container were each covered,
  but never crossed.** Every corpus on this branch is a list of fixtures someone thought to write
  down, and what was broken was what nobody thought to write down.

  This one is generated instead. `fold_matches_the_real_pipeline_for_every_construct_in_every_container`
  takes a list of the nested node lists a tree can hold a construct in — a rendered span, a
  smart-quoted span, each of the three display texts a `Ref` carries, a visible index term's shown
  text, a footnote's text, an anchor's reference text — crossed with a list of every construct this
  module recognizes, and asserts fold parity for the whole product. Adding either extends the sweep
  by a **row or a column** rather than by one case.

  The pairs that diverge are pinned as a *set* rather than skipped, so a pair joining it — or
  leaving it, which a fix does — fails the sweep either way. There are three, and both root causes
  were new:

  *A later family matching across an earlier family's own markup.* The string pipeline runs its
  macro families as passes over one flat string, so by the time the cross-reference and footnote
  passes run, that string already holds the `</a>` the link pass wrote — and their patterns match
  straight through it. `link:t.html[pre xref:tgt[T] post]` renders an `<a>` **nested inside an
  `<a>`**, because the cross-reference's own bracketed text is `T</a> post`. A tree has no tags to
  match through: the link's display text is a subtree, so a bracket cannot span the boundary at all.
  This is the same class as
  [`flatten_prior_markup`](../../parser/src/content/inline_builder/special_chars.rs)'s own, one
  level finer — not a later *step* reading an earlier step's tags, but a later *family of this same
  step* reading an earlier family's — and a **keep**: the tree's answer is the well-formed one.

  *A post-replacement inside a cross-reference's display text.* A link's display text and a
  cross-reference's sit in the same position in the source, and the post-replacements step treats
  them alike. The string pipeline cannot: by then a link has been rendered inline into the one flat
  string that step scans, so its text gets its `<br>`, while a **deferred** cross-reference has been
  replaced by a sentinel pair whose text lives in a *template* the step never sees. The same bytes
  in the same place get a line break in one and not in the other, decided by nothing the author
  wrote. A tree has one answer for both, which is what §4.2's retirement of the
  deferred-cross-reference sentinel makes true for real — so this is a keep that **closes at the
  cutover**, and a third piece of evidence for that retirement, after the corpus in the
  document-parity harness and the fold's own `resolved` handling.

  Both are driven through a real document where it matters (a bare `Content` has no catalog, so
  every cross-reference is left as its sentinel there and would differ for a second, unrelated
  reason). The sweep itself is test-only. Re-running the corpus-wide fold-parity audit picks up
  exactly the three fixtures the two divergence tests add and nothing else — which is what a
  documented keep looks like in that log, and the first time this branch has *added* to it on
  purpose. No content that passed before diverges now. Coverage is diff-neutral, and nothing
  further is wired in.

  *Step 6 prep landed as (the image family's attribute list):* the increment that opened the
  rendered-span class for the link families closed it for every family with a display text to carry
  and left the image family's bracket open — "a value the fold materializes rather than one frozen
  at build time, which is a shape no increment has needed yet". Re-running the cutover experiment
  says the cutover cannot wait for it.

  Forcing the tree on for every parse and making `rendered` the fold leaves **18** of ~5,390 tests
  failing, and every one is accounted for: 5 divergence tests that compare the two paths the cutover
  leaves only one of, 5 footnote-in-a-heading (which `fold_reference_text` exists to close), 3 tests
  of the `with_inline_tree` flag, 2 Strategy-A recorder sweeps, and **3 golden fixtures whose output
  genuinely changes**. Those three split two-one: `` #`CB###2`# `` and its twin get *better*, where
  the tree emits well-formed markup and the string pipeline interleaves tags;
  `['{myrole}']++text++` gets worse, which is this branch's own documented attribute-list keep; and
  `image:pause.png[title=*Pause* and Resume]`, a fixture from the language docs, lost its whole
  macro — not a keep, but the deferred boundary coming due.

  The image bracket now admits a rendered span, and the argument that kept it out cuts the other way
  here. The per-slot rule the link families draw — a frozen span must not reach a slot the renderer
  writes out — defers *every* image bracket, because every value an image's bracket holds is one
  `render_image` writes out. A link has somewhere else to put a rendered span, since its display
  text becomes the node's children; an image has no display text at all. So the rule's cost for this
  family is the macro itself.

  And the frozen bytes are simply the bytes. The string replacer reads its own bracket out of a
  haystack the quotes step has already rendered with this same renderer, so
  `title="<strong>Pause</strong> and Resume"` is what it writes too; freezing the span's build-time
  fold reproduces that exactly rather than approximating it. It is also the same trade
  `bracket_attrlist`'s masked branch already makes for a `Stem`, whose body is likewise a build-time
  fold. What a frozen value cannot survive is being folded again through a *different* renderer —
  `render_with`, which does not exist yet and which every other frozen value on this branch owes the
  same debt to.

  The one thing a rendered piece must still satisfy is the split: a token carries none of the
  `,` / `=` / `"` an attribute list splits on and a span's markup may, so the two parses are compared
  attribute by attribute and a disagreement defers the whole match. The **target** half of the old
  boundary stays, and now has its own reason rather than sharing the bracket's: a target is resolved
  as a path, where splicing markup in has no meaning.

  That leaves the cutover **one** golden regression rather than two.

  *Step 6 prep landed as (a quoted role, read from the attribute list's substituted text):* the
  cutover's last golden regression, and the one the increment above named as "this branch's own
  documented attribute-list keep". It was not a keep. The keep it was filed under is
  `flatten_prior_markup`'s category — a later step of the same order rewriting bytes that live only
  inside markup an earlier step emitted — and three of that keep's four shapes really are that. This
  one was a *wrong answer* in `Attrlist` itself, which the string pipeline's own later step happened
  to paper over.

  [`Attrlist::parse`](../../parser/src/attributes/attrlist.rs) expands attribute references over the
  whole list **before** splitting it, so every *parsed* field a caller reads is already expanded —
  which is why `[{myrole}]`, `[.{myrole}]`, and `[#{myrole}]` were at parity in the tree all along.
  [`quoted_text_fallback_role`](../../parser/src/attributes/attrlist.rs) is the one accessor that
  reads the list's own **text** instead (Asciidoctor's `parse_quoted_text_attributes` takes a
  quote-delimited first positional verbatim, quote characters included), and `parse` computed that
  expanded text as `source_cow` and then *threw it away* — leaving that accessor to fall back to the
  raw `source` span. So `['{myrole}']` yielded the role `'{myrole}'`.

  The string pipeline hid it. Under the *normal* order the attribute-references step runs after
  quotes, so it rewrote the `{myrole}` sitting inside the `class="'{myrole}'"` the quotes step had
  just written, and the final bytes came out right — by the same accident this branch's keep
  describes. The passthrough family has no such accident available, because its extraction pass runs
  ahead of every step; `PassthroughRestoreReplacer` therefore substitutes the *stored* attrlist body
  itself, at restore time, and its own comment says why. Two mechanisms, one surface, and the tree
  reproduced neither.

  The fix is `parse` retaining `source_cow` as [`source_text`](../../parser/src/attributes/attrlist.rs)
  whenever the substitution changed anything — the field the attributed-span increment added for
  precisely this accessor — with
  [`into_owned`](../../parser/src/attributes/attrlist.rs) and `into_owned_restoring` carrying
  `source_text()` forward instead of re-reading `self.source.data()`, so a list rebuilt from a
  temporary does not reinstate the `{name}` spelling. That is Asciidoctor's own order:
  `sub_attributes` over the list, *then* the verbatim first positional. No node field, gate, or
  signature moves, and the string pipeline's rendered output is unchanged everywhere the later step
  was already covering for it.

  What lands here is therefore both halves at once. `['{myrole}']*bold*` — the quotes family, and
  the shape the keep pinned — and `['{myrole}']++text++` — the passthrough family, and the cutover's
  regression — become one parity corpus,
  `a_quoted_role_reads_the_attribute_lists_substituted_text`, together with the `+…+` and backtick
  spellings, a named attribute after the quoted positional, the comma-inside-the-quotes truncation
  (which runs after the substitution, so an expansion introducing a comma truncates too), a missing
  attribute under the default `attribute-missing=skip` (a no-op expansion, so the raw text is what
  the role was already reading), and the three unquoted spellings that never took this path, pinned
  so they still do not. The keep's own test keeps its other three shapes — a typographic
  replacement, a restored entity, and a later sub's own span — which are genuinely markup-reading.

  Re-running the corpus-wide fold-parity audit confirms the divergence set strictly **shrank**, by
  exactly those two fixtures and nothing else. Coverage is diff-neutral (`attrlist.rs` stays at
  100%), and nothing further is wired in. Re-running the cutover experiment leaves it at **zero**
  golden regressions: 16 failing tests, all of them the divergence tests, footnote-in-a-heading
  cases, flag tests, and recorder sweeps the cutover itself resolves, plus the one golden the tree
  gets *better*.

  *Step 6 landed as (a frozen recording, so the corpora survive the fold):* a prerequisite the
  cutover needs and nothing had noticed. Every corpus on this branch is a differential — render a
  fixture through the string pipeline, render it through the tree, assert the two agree — and that
  works only while the two are independent constructions. Making `rendered_html()` a fold ends
  that. A corpus that takes its golden by running `SubstitutionGroup::apply` and reading
  `rendered` is then **comparing the fold against itself**, and passes for that reason.

  This is not a hypothetical. Simulating the cutover (seed forced on, `rendered` := the fold) and
  then *deliberately sabotaging* the fold — one stray byte appended to every
  [`Raw`](../../parser/src/inlines/inline_node.rs) leaf — leaves the 259-fixture whole-pipeline
  corpus **green**. Two hundred and fifty-nine fixtures asserting nothing, with no test failing to
  say so.

  So the golden stops being computed at test time and becomes a **recording**:
  `parser/snapshots/<corpus>.txt`, checked in, reviewed like any other file, read rather than
  derived. [`snapshot::assert_recorded`](../../parser/src/content/inline_builder/snapshot.rs)
  takes the golden and the fold as *separate* parameters, and they are deliberately not
  interchangeable — the **fold** is only ever compared against the recording, never written to
  it, while the **golden** is the only thing `ASCIIDOC_UPDATE_SNAPSHOTS=1` writes. That asymmetry
  is the whole mechanism: no rearrangement of the fold can satisfy a recording tautologically,
  because the fold cannot author one.

  In normal runs the golden is *also* checked against the recording. That is the transitional
  half — a drift guard, so a recording cannot rot while the string pipeline still exists — and it
  is deleted along with the string pipeline, at which point the recordings stand alone exactly as
  the ~277 golden-HTML assertions (§5.3) already do. Recordings are merged rather than replaced
  on update, so a filtered run only adds what it reached; removing a fixture is a deliberate hand
  edit, since a corpus silently shrinking is the failure this file exists to prevent.

  Two corpora take it here: the 259-fixture whole-pipeline sweep and the 119-pair cross-product.
  The cross-product needed a variant — its subject is the *set* of diverging pairs rather than any
  one pair, so [`matches_recording`](../../parser/src/content/inline_builder/snapshot.rs) reports
  instead of asserting, and a pair diverges exactly when the fold differs from the recording. Its
  three pinned pairs otherwise collapse to none under the cutover, which is the same tautology
  wearing a different hat.

  Verified by construction rather than by assertion: under a simulated cutover (with `golden`
  driving the string pipeline directly, which the fold increment must also do or the drift guard
  fires), all three snapshot-backed corpora **pass** with an honest fold and **fail** with the
  sabotaged one, where the old assertion shape passed. Regeneration is idempotent.

  **The generator is the string pipeline, deliberately.** Asciidoctor 2.0.26 was measured as the
  alternative and is a genuinely independent oracle, but on this corpus it agrees on 202 of 259
  fixtures; most of the remainder is the harness not passing each fixture's document attributes,
  and the rest is real crate-versus-Asciidoctor divergence (an image role over a passthrough, an
  attribute-list-prefixed span's role, `<<d,>>`'s empty text). Recording Asciidoctor is worth
  doing — it is the only oracle that outlives the string pipeline *and* is not this crate — but it
  is spec-conformance work with a divergence ledger of its own, not tautology work, and it should
  not ride along here. Noted as a follow-up with the numbers attached.

  What remains is the other corpora: roughly twenty golden-producing helpers across
  `inline_builder`'s per-family test modules and `parser/src/tests/`, each with its own parser
  configuration. **They are the fold increment's prerequisite list, not this one's leftovers** —
  the cutover must not land until every corpus it would render tautological is either recorded or
  knowingly exempted.

  *Step 6 landed as (the golden-HTML oracle, as a callable):* the other half of the recording
  increment's finding, and the half that covers the corpora the recordings do not. A corpus takes
  its golden by running `SubstitutionGroup::apply` and reading `rendered`; the cutover makes
  `rendered` a fold, so the corpus compares the fold against itself. Recordings answer that
  durably, but converting twenty-odd corpora — each with its own parser configuration — is a large
  change to make *inside* the cutover, where a mistake is indistinguishable from a real divergence.

  A new test-only
  [`apply_string_pipeline`](../../parser/src/content/substitution_group.rs) answers it cheaply
  instead: `run_pipeline` on its own, with no tree and no fold. **25 golden-producing call sites
  across 14 files** now take their golden from it, so every one goes on differentiating against a
  genuinely independent construction for as long as the string pipeline exists — which is exactly
  the window the cutover needs. The recordings remain the durable answer for when it does not.

  What makes this landable *now*, ahead of the cutover, is that it is byte-identical today: the
  tree is still additive, so `apply` and `apply_string_pipeline` differ only in work the golden
  never reads. The whole suite stays green with **no test edited** and both recordings byte-for-byte
  unchanged — a claim the cutover itself could no longer make, which is the reason to spend a
  separate increment on it.

  Two categories deliberately keep `apply`. The ~277 **golden-HTML assertions** (§5.3), whose
  subject is `rendered_html()` itself: they must go on exercising the production entry point, and
  after the cutover they are precisely what validates the fold. And
  [`passthrough_text`](../../parser/src/content/inline_builder/passthrough_step.rs)'s own call,
  which is production code rather than a golden.

  Measured under a simulated cutover with a deliberately sabotaged fold: six corpora flip from
  passing to failing that would otherwise have passed — `fold_matches_the_string_pipeline_for_xrefs_inside_expanded_values`,
  the four `build_for_group` order tests, and the cross-reference family's deferral test. The
  other nineteen sites are tautological by construction under the cutover whether or not a given
  probe happens to perturb them; a gross sabotage breaks so much through the golden assertions
  that it understates the loss, which is the point of having the corpora at all.

  *Step 6 landed as (a passthrough body is literal text, not raw output):* the cutover blocker
  review found on the always-on increment, and a conflation in §3.4's trichotomy that had been
  there since the passthrough step landed.

  [`Raw`](../../parser/src/inlines/inline_node.rs) was documented as "verbatim, un-escaped output
  … `+++…+++`, `pass:[…]`, `$$…$$`". The first two are that. `$$…$$` and `++…++` are **not**:
  their substitution group applies special characters, so their body is the author's *literal
  text*, and something has to escape it. The builder did — at build time, through whichever
  renderer the `Parser` happened to carry — and froze the result into the node. Two things
  followed, and the review caught the second:

  1. Folding that tree through a *different* renderer emitted the parse-time renderer's escaping
     instead, which is precisely what `render_with` exists to not do. Measured with a
     non-HTML backend: the string pipeline produced `a [LT] b` while the fold produced
     `a &lt; b`.
  2. Building the tree **invoked** the document's renderer for a value nothing reads. With the
     tree built for every parse, a renderer carrying state saw extra calls and a *later block's*
     authoritative output shifted under it — a real regression, and one no test paired a stateful
     renderer with a passthrough to catch.

  The obvious fix — hand the builder's parser clone a built-in renderer so it cannot touch the
  document's — is wrong, and measurably so: it makes (2) go away by making (1) worse, silently
  HTML-escaping a custom backend's passthrough body. What the two failures share is the *freezing*,
  not the renderer.

  So `Raw` gains a [`form`](../../parser/src/inlines/inline_node.rs): `AsIs`, whose value already
  is the bytes to emit, and `Escaped`, whose value is logical text the fold escapes exactly as it
  escapes a [`Text`](../../parser/src/inlines/inline_node.rs) run. Both stay **opaque** — no
  transducer step matches inside either, which is what keeps them one node kind rather than two —
  and the escaping moves to the one place a renderer is chosen. The `Escaped` sites also stop
  allocating, since they now borrow the author's bytes instead of owning an escaped copy.

  Opacity is why the obvious alternative does not work: a `++…++` body is *semantically* a `Text`
  run, and folds identically to one, but
  [`build_match_string`](../../parser/src/content/inline_builder/quotes.rs) decides atomicity by
  node **kind** — a `Text` piece is splittable, so later steps would match into a passthrough body.
  The node has to stay `Raw`; only its escaping moves.

  The whole suite passes with **no expectation changed**. That is the claim worth making: the
  observable bytes are identical, which a change to *where* escaping happens should be, and the
  corpus-wide fold-parity audit agrees — the divergence set is byte-identical, 235 rows before and
  after, none added and none closed. `assert_raw` now asserts a node's **fold** rather than its
  `value` field, because "this passthrough contributes these bytes" is the same question for both
  forms where reading the field is only the same question for one; that is what let ~25 existing
  expectations stand unchanged. The Strategy-A cross-check needed the mirror of that:
  [`raw_rendered`](../../parser/src/tests/inline_builder_recorder_parity.rs) renders a `Raw` leaf
  before matching it against the recorder's own bytes, exactly as `char_ref_rendered` has always
  had to.

  What still freezes a value, each for its own reason: a `pass:c,q[…]` body (its explicit
  substitution list runs a whole pipeline, the deferral 5d part 3 documents for itself), a bare
  `+…+` body enclosing an already-extracted passthrough (its value interleaves escaped text with
  another node's fold bytes, so no single form describes it), and a `Stem` body. Each is a narrow
  form, and each owes the same debt to `render_with` that every frozen value on this branch does.

  *Step 6 landed as (the bare `+…+` body, which is literal text too):* the increment above closed
  the passthrough family's build-time escaping for `++…++` and `$$…$$` and named three shapes that
  still freeze a value. Sweeping *every* construct with a call-counting renderer — rather than
  taking that list on trust — confirms exactly three, and one of them is not narrow at all: the
  **bare `+…+` form**, which is `SubstitutionGroup::Verbatim` like its delimited siblings and is
  ordinary AsciiDoc.

  It could not say so while one shape stood in the way. A bare body *enclosing a construct the
  first extraction pass already replaced* (`+a $$b$$ c+`) interleaves escaped text with that
  node's own **fold** bytes, and no single form describes the mixture. The previous increment read
  that as "the bare form freezes", when what it actually means is "the *mixture* freezes" — and
  the mixture is rare where the plain body is not. So
  [`substitute_and_restore`](../../parser/src/content/inline_builder/passthrough_step.rs) now
  detects it rather than assuming it: with nothing restorable inside the body, the value is the
  author's bytes and the fold escapes them
  ([`Escaped`](../../parser/src/inlines/inline_node.rs)); only the genuine mixture keeps `AsIs`.

  What remains is **three** shapes, and the third is this increment's own leftover rather than a
  new one: a `pass:c,q[…]` body and a `Stem` body each run an arbitrary substitution list, and the
  mixture above — a bare body enclosing an already-extracted construct — interleaves escaped text
  with that construct's fold bytes. None of the three is a *specialcharacters* body, so no
  `RawForm` describes any of them; all three need the fold-time laziness §3.3.1 will bring, and owe
  `render_with` the same debt every frozen value on this branch does. Every *other* construct makes
  **zero** renderer calls while its tree is built, which the increment below pins as a sweep.

  Corpus-wide fold-parity audit: divergence set byte-identical, 60 rows either side. Coverage is
  diff-neutral.
  *Step 6 landed as (the tree, built for every parse — the `with_inline_tree` flag retired):*
  the first piece of the cutover proper, and deliberately the one that changes **no output at
  all**. Every prior increment measured the cutover by forcing the seed on in a throwaway patch;
  this makes that half real while leaving `rendered` exactly where it is, so the whole corpus
  runs against a tree that is built but not yet read.

  That ordering is the point. Turning the builder on for every parse and making the fold
  authoritative are two independent risks — a builder that panics or mis-recognizes somewhere in
  ~5,390 tests, and a fold whose bytes differ — and bundling them would report both as one red
  suite. Split, the first is falsifiable on its own: with `rendered` still the string pipeline's,
  the tree is purely additive, so the *only* tests that can fail are the ones asserting the tree
  is absent. Two did, and nothing else moved.

  `Parser::with_inline_tree` is gone from the public API. The `build_inline_tree` field it set
  stays, defaulting to `true`, because the flag's plumbing was always doing a second job:
  `SubstitutionGroup::apply` clears it on the counter-safe clone it hands the builder, since
  building a tree re-enters `apply` for a passthrough body whose own substitution list must run
  (`passthrough_text`), and that nested content needs no tree. It is now documented as the
  recursion guard it is, with no public surface.

  The two tests that asserted absence are rewritten rather than deleted. `inline_tree_is_empty_by_default`
  becomes `inline_tree_is_built_by_default`, its assertion inverted. `is_block_inlines_is_some_but_empty_when_flag_off`
  keeps the distinction it was really pinning — `Some(&[])` is a content-bearing block, `None` a
  block with no direct content, and the two are not interchangeable — but reaches `Some(&[])`
  through content that is *genuinely* empty (`----\n----`) rather than through a flag. Every
  other call site simply drops a `.with_inline_tree(true)` that is now the default.

  What this costs is performance, and it is the cutover's cost rather than this increment's: a
  parser clone and a full recognition pass per content, on every parse. It is paid back when the
  string pipeline stops being run for its output, which is what the increments below do.

  The claim above — "changes no output at all" — needs one qualification, and it is the review
  finding that sent this increment back the first time. Building the tree is only unobservable if
  the build does not *call the document's renderer*, and it used to: a passthrough body was escaped
  at build time, so a renderer carrying state saw extra calls and a **later block's** authoritative
  output shifted under it. The two increments above closed that for every
  specialcharacters body, and
  `building_the_tree_does_not_consult_the_documents_renderer` now sweeps every construct to keep
  it closed — measured at `build` rather than at `parse`, deliberately, since the string pipeline
  legitimately calls the renderer and for an unresolved cross-reference calls it five times, with
  and without a tree alike. Counting a whole parse would pin the string pipeline's own behavior and
  call it this increment's.

  Three shapes still consult it, each pinned by
  `the_three_non_specialcharacters_bodies_still_consult_the_renderer` so they are a decision rather
  than a gap: a `pass:c,q[…]` body, a `Stem` body, and a bare `+…+` body enclosing an
  already-extracted construct. So the honest form of the claim is: **this changes no output for any
  renderer that does not carry state**, and for one that does, only through those three.
  *Step 6 landed as (the footnote-marker sentinel system, deleted — the first of the three):*
  the cutover's second piece, and the first §4.2 deletion. `Section::parse` now gets a heading's
  footnote-free reference text from
  [`fold_reference_text`](../../parser/src/content/inline_builder/fold.rs) instead of from a
  byte-level cut, and the whole sentinel apparatus goes with it: `FOOTNOTE_MARKER_START` /
  `FOOTNOTE_MARKER_END`, `strip_footnote_marker_spans`,
  `Content::remove_footnote_marker_sentinels`, `Parser::mark_footnote_spans`, and the two
  `dest.push(…)` calls in [`InlineFootnoteMacroReplacer`](../../parser/src/content/macros.rs)
  that emitted them.

  The property being traded is worth stating precisely, because it is *not* "the fold is
  authoritative" — `rendered` is still the string pipeline's here, and stays that way until the
  next increment. What moves is one derived string: a heading's reference text and the id
  derived from it. The sentinels existed only to make **one** render yield **two** strings, so
  that counters and attribute-expanded footnotes were not processed twice. A tree yields two
  strings from one render for free — they are two folds of it — so the constraint the sentinels
  bought is satisfied by construction and the mechanism is pure overhead.

  The prep increment that staged `fold_reference_text` pinned the equivalence against a corpus
  of thirteen titles. That corpus is a list of fixtures someone thought to write down, which is
  the failure mode the cross-product sweep was built to answer, so the swap was measured
  corpus-wide first instead: a throwaway patch computed **both** answers for every section title
  in the suite and logged any disagreement. Across ~5,390 tests there were **zero**. That is the
  evidence the swap rests on; the fixture corpus survives as its written-down form.

  With the strip gone there is no second implementation left to differentiate the reference text
  against, so `fold_reference_text_matches_the_sentinel_strip` becomes
  `fold_reference_text_omits_a_headings_footnote_markers`: the heading's own rendering is still
  compared against the string pipeline (the §5.3 oracle, which still produces it), while the
  reference text is compared against a **literal** expected string — the exact bytes the strip
  produced, captured before it was deleted. The four unit tests of the strip itself go with the
  function.

  Re-running the corpus-wide fold-parity audit shows the divergence set **shrink by 16**, every
  one of them a title whose only disagreement was the sentinel pair itself, which neither side
  now emits. One row is re-spelled rather than added: a recorder test's own
  `MARK_OPEN`/`MARK_CLOSE` fixture, which had appeared in both a with- and a without-sentinel
  form and now appears once. Coverage is diff-neutral.

  *Step 6 landed as (`rendered_html()` as an authoritative fold):* the cutover's third piece, and
  the one the whole branch has been building toward. `SubstitutionGroup::apply` now sets
  `content.rendered` from [`fold_html`](../../parser/src/content/inline_builder/fold.rs), so what
  `rendered_html()` returns *is* the tree — the string pipeline still runs (it produces the
  deferred cross-reference segments and fills the catalogs), but its output is no longer what a
  caller reads.

  Content carrying a **deferred cross-reference** is the one exception, and it is temporary. Such
  a content's rendered string is rebuilt from a placeholder template every time resolution runs,
  so making the fold authoritative there means teaching resolution to *re-fold* instead — which
  is the deferred-cross-reference sentinel system's own retirement (§4.2's second), and its own
  increment. Rather than interleave the two mechanisms, such a content keeps the template path end
  to end and the fold takes everything else. The predicate is one line
  (`content.deferred_parts().is_none()`) and the increment below deletes it.

  The interesting work here was not the fold. It was that **every differential corpus on this
  branch was about to become tautological.** A parity test takes its golden by running
  `SubstitutionGroup::apply` and reading `rendered` — which is now the fold, so the test would
  have been comparing the fold against itself and passing for that reason. Twenty-one call sites
  across twelve files were in that shape, and nothing about a green suite would have said so. A
  new test-only [`apply_string_pipeline`](../../parser/src/content/substitution_group.rs) exposes
  `run_pipeline` on its own — the §5.3 oracle as a callable — and every golden-producing site now
  goes through it. The sites that drive individual `SubstitutionStep`s by hand were already
  building no tree and are untouched, as are the ~277 golden assertions, which read
  `rendered_html()` as the *subject* rather than as a differential golden and are precisely what
  the fold now has to satisfy.

  Four of the six tests the cutover experiment predicted would fail were that shape and are fixed
  by the rewiring alone. Of the two that remain, one is
  `inline_tree_build_tolerates_a_stateful_renderer`, whose renderer emits different bytes on each
  call: it is now invoked twice per parse (once by the string pipeline, once by the fold), so the
  rendered string shows the *second* invocation. That is the interim double render, not a property
  of the fold, and it is pinned exactly — `[second]` rather than loosely — so that it shows up as
  a signal when the string pipeline stops being run for output.

  The other is the **one golden output the cutover changes**, and it changes for the better.
  `#`CB###2`#` — an `asciidoc-lang` fixture whose own prose calls the result "a scrambled mess" —
  rendered a `<mark>` opened inside a `<code>` and closed outside it, because the string
  pipeline's highlight sub matched across the markup its monospace sub had already written. A
  tree has no tags to match through, so the extra `#`s stay literal and the nesting is
  well-formed. This is the same class as the keep the cross-product sweep documents for the macro
  families, reached one step out; the golden is updated with that reasoning written beside it.

  Re-running the corpus-wide fold-parity audit needs its own adjustment for the same reason the
  corpora did — the probe has to log *before* the fold overwrites `rendered`, or it compares the
  fold against itself. So adjusted, the production divergence set goes 18 → 9 with **no new
  divergence**. The nine that remain are fully accounted for: one is the changed golden above,
  four are custom-renderer tests (the audit's probe folds with the built-in renderer by
  construction, so it disagrees with a `[LT]`/`[first]`/`<figure>` backend — pre-existing, and
  not a divergence), and four are deferred-cross-reference content, which the increment below
  closes. The nine that *left* the log are the documented keeps, and they left because their
  tests now drive the string pipeline directly rather than because anything was fixed — worth
  saying plainly, since the audit can no longer see a fixture whose test bypasses `apply`.

  Coverage is diff-neutral (`substitution_group.rs` stays at 100%).

  *Step 6 landed as (a cross-reference's effective `xrefstyle`, resolved into the node):* the
  first prep for the deferred-cross-reference retirement, and one the fold-parity audit cannot
  see. [`Ref::xrefstyle`](../../parser/src/inlines/ref_node.rs) held the macro's own
  `xrefstyle=` **override**, and
  [`fold_xref`](../../parser/src/content/inline_builder/fold.rs) supplied the fallback itself
  (`reference.xrefstyle.or_else(|| document_xrefstyle(parser))`). That is fine only while the fold
  runs in the same pass as the parse. The effective style is a *document-order* fact — a
  `:xrefstyle:` line rebinds it for everything after it — so a fold running **later** than the
  parse reads whatever the last such line left set, and silently re-styles a reference the string
  pipeline had already styled. A re-fold at reference-resolution time is exactly that later fold,
  which is what the retirement below needs.

  So the field becomes the **effective** style, resolved at build time by whichever builder makes
  the node ([`build_xref_node`](../../parser/src/content/inline_builder/macros/xref.rs) for the
  macro form, [`build_xref_shorthand_node`](../../parser/src/content/inline_builder/macros/xref.rs)
  for `<<id>>`), which is the same `document_xrefstyle` reading `InlineXrefReplacer` makes, in the
  same pass. `None` now means *no style* rather than *ask the document*, and the fold consults no
  document state for it — §3.3.1 point 1, applied to one more order-dependent fact.

  What pins it is not the audit, which compares `apply`'s own fold against `run_pipeline` in the
  same parse and therefore reads the same attribute on both sides (0 new, 0 closed — correctly).
  It is the whole-document harness
  ([`inline_builder_document_parity`](../../parser/src/tests/inline_builder_document_parity.rs)),
  whose fold runs after resolution with a *fresh* `Parser` for render context: four fixtures with
  `:sectnums:` and a document-wide `:xrefstyle:` — set at the top, rebound midway, rebound to a
  different value, and overridden per-macro — all of which **fail on the base branch** (`Install`
  where the string pipeline says `Section 1, &#8220;Install&#8221;`) and pass here. That the
  harness's own `fold_parser` is a default one is what made the gap visible at all.

  Coverage is diff-neutral (`fold.rs` 3/3, `xref.rs` 16/7, `ref_node.rs` 0/0 — identical to base).

  *Step 6 landed as (each deferred content retains its own document attributes):* the second
  prep, and the one that makes a later fold *possible* rather than merely correct. A fold running
  after the parse needs the document state its content was written under, and by then the parse
  has moved on — so a content carrying a deferred cross-reference now keeps its
  [`ResolvedAttributes`](../../parser/src/parser/resolved_attributes.rs), snapshotted where "now"
  is still that point in the document. Retention is narrow on purpose: deferred content is the
  only content whose rendering is rebuilt after the parse, so it is the only content that will be
  folded after it. Everything else — nearly everything — keeps `None`.

  It retains the *attributes* rather than a whole `RenderContext`, and the difference is not
  cosmetic. A `RenderContext` holds the path resolver and the two file handlers as `Rc<dyn …>`, so
  retaining one would cost `Content` — and with it
  [`Document`](../../parser/src/document/document.rs) — its `Send`/`Sync`. `Document` is both today
  (`Parser`, holding six `Rc<dyn …>`, deliberately is not), nothing pinned it, and the regression
  would have been silent;
  `document_stays_send_and_sync` now pins it. Splitting the halves is also the more accurate model:
  document attributes are **order-dependent** (a `:imagesdir:` line rebinds them for everything
  after it) so they must be frozen per content, while the resolver and handlers are **parse-wide
  configuration** that cannot change mid-parse, so freezing them buys nothing. The increment that
  folds at resolution time assembles a context from this plus that configuration, which its caller
  already holds. `ResolvedAttributes` is `Arc`-shared internally, so retention allocates nothing
  beyond its box, and it is `Eq`, so `OwnedTitle` keeps its derived equality.

  Nothing read it yet, which is the staging: landing the retention while it is a provable no-op is
  what keeps the step that consumes it falsifiable. Audit: 54 divergences before and after, 0 new,
  0 closed. Coverage diff-neutral.

  *Step 6 landed as (the fold takes a `RenderContext`, not a `Parser`):* the third prep, and the
  one that finishes what merging `main` started. `RenderContext` landed on `main` as *the document
  state a renderer reads*; the merge wired the string pipeline's own call sites to it but left
  [`fold_html`](../../parser/src/content/inline_builder/fold.rs) taking a `&Parser` and building a
  context per rendered element. Faithful, but the wrong shape — a fold running later than its parse
  cannot derive its context from the live parser, because the attributes have moved on. So the fold
  takes the context, threaded from the one place that starts it, which is also strictly less work:
  the per-element construction is gone, and with it the ~117ns per rendered element #1265 measured.

  Two things had to move for that signature to exist. `fold_image` and `fold_link` parsed an
  *empty* `Attrlist` for a hand-built node carrying none, which needs a parser;
  [`Attrlist::empty`](../../parser/src/attributes/attrlist.rs) builds the zero-attribute list
  directly, keeping the node's own zero-length location so lifetime and position still match (and
  removing an attribute-reference substitution pass from that path). And the builder's own five
  internal folds — a masked piece's body, an anchor's reference text, the pre-escape probe, the
  restored-markup splice, the title's reference text — each take `parser.render_context()` at their
  call site, where a parser is in hand. No behavior change: the context a fold now receives is the
  one it was building for itself.

  *Step 6 landed as (a `Raw` node records where its content came from):* the fourth prep, and the
  first of the two builder preps the retirement's own audit turned up.
  [`Raw`](../../parser/src/inlines/inline_node.rs) gains an
  [`origin`](../../parser/src/inlines/inline_node.rs) orthogonal to its `form`: `Passthrough` for
  content the extraction pass pulled out before any step ran (`+++…+++`, `++…++`, `$$…$$`,
  `pass:[…]`, an inline STEM body), `Substitution` for raw output a substitution produced in place
  — an expanded attribute value's literal special (which §3.4.1 leaves unescaped, the value having
  expanded *after* `specialcharacters` ran), a special an effective order never escaped, or a slice
  of an entity's own bytes.

  It answers two questions. For a **consumer** it sharpens the security story §3.4 gives this node
  kind: "this document emits raw HTML" is visible as `Raw` nodes, and whether that came from an
  author writing an explicit passthrough or from a substitution expanding an attribute value is the
  difference between a deliberate escape hatch and a value that may have arrived from elsewhere.
  For the **builder** it is the difference between content the extraction pass is *holding* and
  content that is simply there. A cross-reference's target is captured into its deferred segment
  before the restore pass runs, so a computed value reading a `Passthrough` node's bytes sees the
  extraction sentinel, while one reading a `Substitution` node's sees exactly what the string
  replacer saw.

  Recording it on the node is the point. A first attempt inferred the same distinction from the
  `Masked` list the extraction pass builds, and it does not work: that list is keyed by location
  identity and is empty on call paths where the identity is not in hand, so the same node was
  classified differently depending on which pass asked. Provenance is a fact about the node, so it
  belongs on the node — the same reasoning that put `form` there. Nothing read it yet.

  *Step 6 landed as (a cross-reference whose target is attribute-expanded):* the fifth prep, the
  first thing `RawOrigin` is *for*, and one of the two regressions the retirement's probe found.
  `xref:{cpp}[{cpp}]` was left literal by the builder while the string pipeline recognized it:
  `{cpp}` is `C&#43;&#43;`, §3.4.1 leaves an expanded value's `&` unescaped, so the target crosses
  two `Raw` leaves that the match string stands in as placeholders — and the gate deferred on them.
  Those leaves are `Substitution`: nothing extracted them and nothing restores them, so the string
  replacer's own haystack held exactly these bytes, and filling the placeholders in **reaches**
  parity rather than departing from it. `range_is_substitution_restorable` admits only such leaves;
  a masked passthrough stays deferred, which is the distinction the whole thing rests on — it is
  restored by a *later* pass, and a deferred cross-reference's target is captured before that pass
  runs, so the string pipeline's own `href` holds the sentinel there.

  Two things it turned up. The shorthand needs the replacer's `id.contains('<')` guard **for real**
  now: `build_xref_shorthand_node` documented needing no counterpart, because markup in an id was
  always an opaque piece and the gate refused the match before any id existed — but a
  substitution-produced `<` is not opaque (`:markup: <b>x</b>`, then `<<{markup}>>`), and the check
  has to be made against the *restored* id. And only the **id half** of a shorthand may be restored:
  restoring the whole inner first shifts every offset the reference text is then sliced with, which
  corrupted `<<sec,a +++<b>x</b>+++ text>>` into a trailing `&gt;&`. The text becomes structured
  children in match-string coordinates and must stay there; only the id is read as a string. The new
  test's golden comes from the *whole* `Normal` group rather than `golden_xref`, which drives the
  macro-family steps only and would leave `{cpp}` unexpanded on the golden side.

  *Step 6 landed as (the deferred-cross-reference sentinel system, retired — the second of the
  three):* the fold becomes authoritative for the one content kind the cutover had to carve out.
  Such a content's rendering is rebuilt from a placeholder template on **every** resolution pass
  ([`Content::rebuild_rendered`](../../parser/src/content/content.rs)), so a fold taken at parse
  time would be overwritten by the next pass — and would anyway be answering a question the
  document has not settled, since the destinations are not known yet. It is folded at the **end of
  resolution** instead ([`Content::refold`](../../parser/src/content/content.rs)), which is the
  same answer one step later, assembled from the attributes the content retained (prep 2) paired
  with the parser's own configuration through a `RenderContext` (prep 3).

  The carve-out does not disappear so much as **narrow to something principled**. It was
  `content.deferred_parts().is_none()` — *any* deferred cross-reference keeps the template. It is
  now the count match
  [`mirror_tree_xref_resolution`](../../parser/src/content/content.rs) already computes: the tree's
  block-level cross-reference nodes against the segments whose placeholders are still in the
  template. Where they agree, the tree holds every cross-reference the string pipeline deferred and
  a fold of it *is* this content's rendering. Where they differ, the builder left one of its
  documented forms unrecognized, so folding would **lose** the construct rather than render it
  differently — and the template stays. That gate self-liquidates: each builder prep that teaches
  one more form shrinks it, and it vanishes when none is left. The **footnote** half of the mirror
  is deliberately not part of the gate — a footnote's text is extracted out of the block, so a fold
  emits the marker and never descends into the subtree whose correlation was skipped.

  Exactly **one** of the two renderings runs — the fold or the template, never one overwritten by
  the other. That is not merely thrift: a renderer is a host-supplied trait object, so a stateful
  one (a recorder, a numbering backend, anything counting its own callbacks) would see every
  callback for a deferred content twice in a single resolution pass. A probe measuring the fold
  against the template *inside* the pass showed exactly that as 11 rows of recorder counter drift
  — the symptom, read at first as an artifact of the measurement.
  `resolution_renders_a_deferred_content_once` pins the property with a counting renderer, which
  `Document::resolve_references` takes as an ordinary argument.

  Retiring the double render costs the whole-document parity harness its oracle for exactly the
  content this step changed: `check_document` folds each location's tree and compares it against
  `rendered_html()`, which for a deferred content now *is* that fold. So the template's answer
  becomes reachable on its own — `Content::rendered_from_template`, test-only — and
  `the_fold_reproduces_the_template_for_every_deferred_content` compares the two over a corpus of
  fourteen cross-reference fixtures (both spellings, empty and absent reference texts, unresolved
  targets, several in one content, a formatting span, an inter-document target, a footnote-embedded
  reference, an attribute-styled one, a table cell, a block title). Both new tests fail if their
  property is removed, checked by removing it.

  Sizing the step took a probe over the whole suite (fold vs. template at every resolution):
  **14** divergences, of which 11 were the recorder drift above, one is an *improvement* the code's
  own comment predicted, and two were real regressions — both of which became preps 4/5 above. That
  improvement is `a_post_replacement_in_a_cross_reference_text_is_now_at_parity`, a divergence
  pinned since the cross-product sweep with a note saying it would close exactly here: a
  `+`-line-break in a cross-reference's display text now gets its `<br>`, because a display text is
  a subtree either way and the fold walks subtrees, where the string pipeline's step never saw
  inside a template.

  What still defers is the **shape** the second regression had: `xref:sec[*bold*,role=*hl*]` — a
  rendered span reaching a value the macro families read as a *string* (`role=`, `window=`,
  `xrefstyle=`). It is the last known member of the class, it is what keeps the narrowed gate from
  being vacuous, and it is a builder prep like the five above rather than anything this system owes.
  Also still deferred: a **section or block title's** deferred cross-references, which
  [`title_refs`](../../parser/src/document/title_refs.rs) resolves in document order with
  cross-title coordination the per-content pass cannot do, and which still take their rendering from
  that pass's template. Retiring the template *itself* — the sentinel constants, `render_template`,
  `DeferredContent::template` — waits on both, and on the whole-document parity harness no longer
  needing the template as the oracle the fold is checked against.

  *Step 6 landed as (a title's rendering joins the fold, closing the retirement's other half):*
  the increment above retired the deferred-cross-reference sentinel system for content resolved by
  [`Content::resolve_references`](../../parser/src/content/content.rs) and named a title as what
  still deferred. A title is not resolved per content: a cross-reference between two titles —
  forward, or circular — needs coordination the per-content pass cannot do, so
  [`title_refs`](../../parser/src/document/title_refs.rs) computes every title's rendering together,
  in document order, breaking cycles the way Asciidoctor does. That pass rendered the template.

  It now folds instead, and **where** it folds is the whole design. The obvious place is the pass's
  write-back walk, after the resolutions are computed — and that is wrong: the pass needs each
  title's rendering *while it runs*, because that string is the link text a reference **to** that
  title splices in. Folding afterwards would leave the template render standing as the coordination
  input and add a second one, which is precisely the double callback the increment above removed. So
  the fold happens inside `compute`, in place of `render_xref_template`, with the template kept as
  the fallback. `the_title_pass_renders_each_title_once` pins it with a counting renderer, and fails
  on the write-back placement.

  Folding there means folding without a `&mut` to the blocks — the pass walks them again afterwards
  to install what it computed — so [`fold_resolved_title`](../../parser/src/content/content.rs)
  folds a **clone** of the title's tree, and the real tree still takes the later mirror. Only the
  block-level destinations are installed into that clone: a fold emits a footnote's *marker* and
  never descends into its subtree, so the mirror's footnote half has no counterpart here (it was
  written, found dead by coverage, and removed).

  The coordination survives the fold because it never lived in the template. A
  [`ResolvedReference`](../../parser/src/parser/reference_resolver.rs) carries the reference text
  the pass computed for its target, `assign_tree_xrefs` installs it on the node, and `fold_xref`
  reads it there — so the fold reproduces the coordinated answer, cycles and all, rather than a
  per-title one.
  `a_folded_heading_keeps_the_cross_title_coordination` pins three shapes (a forward reference, a
  cycle whose inner link text falls back to `[id]` with the nested anchor dropped, and a footnote in
  a heading) — and passes on the base branch too, which is the point: **this increment changes no
  output.** Measured over the whole suite, 59 of 60 deferred titles take the fold and every one of
  them reproduces the template byte for byte; the 60th has an empty tree and keeps the template.

  It costs callbacks, and the review that found it was right to. Folding renders the **whole**
  title, where the template render it replaces touched only the placeholders: measured on
  `== A *bold* image:x.png[X] a < b <<t>>`, total renderer calls go 13 → 16, and the three added are
  duplicates of calls the string pipeline already made at parse time. That is the transitional cost
  of an authoritative fold rather than one this path invents — a plain paragraph's `<` is rendered
  twice *today* (once by `run_pipeline`, once by the fold that overwrites its answer), and a
  deferred paragraph has paid exactly this since the increment above; the fixtures that were already
  folding measure 9 → 9 and 12 → 12, unchanged. A title was the last content on the cheaper template
  path, and it was cheaper only because its rendering was less correct. The duplication ends for all
  of them together when step 6 takes the string pipeline off the production path; it cannot end
  here, because folding a tree means rendering it. What *is* avoidable, and avoided, is rendering
  the same title twice within the pass.

  It also corrected an oracle the increment above introduced.
  `the_fold_reproduces_the_template_for_every_deferred_content` walked block titles, and could not:
  a title's own deferred segments are never resolved in place — the pass resolves a *clone* of them,
  since the coordination is cross-title — so `rendered_from_template` renders the **uncoordinated**
  fallback for one. It passed only because every title fixture in that corpus happened to coincide
  (`[tgt]` on both sides). A section-title fixture made it fail honestly; titles are out of that
  test now, and covered by the coordination pins instead.

  What still defers is `xref:sec[*bold*,role=*hl*]` — a rendered span reaching a value the macro
  families read as a *string*. It is no longer only a cross-reference concern: `Ref::roles` is
  `Vec<CowStr>`, and the link and image families draw the same boundary in their own gates, so
  closing it means giving a computed *string* slot somewhere to put fold-time markup rather than
  teaching one family one form. Retiring the template itself — the sentinel constants,
  `render_template`, `DeferredContent::template` — waits on that and on
  `Content::rendered_from_template`, which is now the only thing keeping the fold differentiated
  against an independent construction.

  *Step 6 landed as (the recognition side effects, re-attached for real):* the third of the four
  things step 6 asks for, and the first that is not about the rendered string at all. Recognizing a
  construct also means *writing it down*: an `image:` macro records its target in the asset catalog,
  the three link passes record theirs, an inline anchor and a bibliography entry register their ids
  in the reference catalog, and an image whose `link=` names a dangerous scheme records a warning.
  All four were staged — built, composed as
  [`apply_macro_side_effects`](../../parser/src/content/inline_builder/macros/mod.rs), and pinned by
  their own corpus-wide harness — but every one of their doc comments still ended "nothing here is
  wired into a real parse path yet". They are wired now.

  The switch is a **suppression window**, not a deletion.
  [`SubstitutionGroup::apply`](../../parser/src/content/substitution_group.rs) sets
  `Parser::suppress_macro_side_effects` around its authoritative string pass and replays the four
  from the tree afterwards. The string replacers' own `register_image` / `register_link` /
  `register_ref` calls and the image family's dangerous-scheme warning stay where they are, gated;
  deleting them outright is what the *next* step does, when `run_pipeline` leaves the production
  path. Both paths share one `Parser` now, which is exactly why step 6 says to re-attach the side
  effects at the cutover — "which is what avoids double-counting each registration".

  Three details decided themselves. The replay runs **after** the whole string pass rather than
  during its macros step: across contents that preserves document order, and within one content
  `apply_macro_side_effects` already composes the families in the string pipeline's own pass order,
  which is what its own doc comment was written to guarantee. A suppressed `register_ref` returns
  **`Ok`**, not an error, because the string replacer raises its duplicate-id warning on `Err` and
  that warning is the replay's to raise. And the *link* family's dangerous-scheme warning is **not**
  suppressed, because the replay does not carry it — only the image family's is among the four.

  Two things fell out of testing rather than design. The window was first gated on a tree actually
  being built; the whole suite passes without that gate, and the reason is sound rather than
  incidental — the only content reaching `apply` without a tree is a passthrough body the *builder*
  re-enters for, handed the counter-safe clone whose catalog is discarded with it. The gate is gone.
  And the flag is saved and restored rather than cleared, which no fixture currently distinguishes
  (the suite is green either way, since none puts a registering construct after a nested `apply`
  within one content); it is kept because it is correct by construction rather than by that absence,
  and the code says so.

  The audit is a whole-suite catalog diff rather than a rendered-string one, since no rendered byte
  changes: every parse in the suite dumps its images, links, refs and warnings, and the **3,773**
  records are byte-identical between this branch and its base. Dropping the replay fails **69**
  tests; hoisting the window from one pass to a whole parse fails **161** — the first says the tree
  is really the source now, the second says the description-list **term** carve-out is real. A term
  runs the substitution steps directly rather than through `SubstitutionGroup::apply`, so it builds
  no tree and has nothing to replay from; it stays correct only because it never enters the window.
  That is the last thing still registering from the string pipeline, and it goes when the pipeline
  does.

  *Step 6 landed as (the callout registration, the last recognition side effect):* the increment
  above re-attached the four **macro** side effects and left one behind, because it is not a macro
  family at all: a callout is recognized in *verbatim* content, where the macros step never runs.
  [`apply_callout_side_effects`](../../parser/src/content/inline_builder/callouts.rs) replays it the
  same way, as a sibling call rather than part of the composition, and the suppression flag widens
  from `suppress_macro_side_effects` to `suppress_recognition_side_effects` to say so.

  It is simpler than the macro four in two ways and more subtle in one. Simpler: a
  [`Callout`](../../parser/src/inlines/callout.rs) node already carries its **resolved** number — an
  auto-numbered `<.>` was resolved when the node was built — so the replay consults no counter, and
  the ordering that matters is satisfied for free, since the consumer
  ([`Parser::callout_defined`](../../parser/src/parser/parser.rs)) reads it when the *following*
  block, the callout list, is parsed.

  Subtle: a duplicated registration here would be **invisible**. `CalloutCatalog` holds one
  `Vec<u32>`, read through `contains` and cleared per list, so registering `1` twice answers exactly
  as registering it once — unlike the asset and reference catalogs, whose entries a consumer reads
  in order and where a double shows immediately. So the gate on the string pipeline's own
  `register_callout` buys nothing observable *today*; it is there for correctness by construction
  and for the day the pipeline goes, and the code says that rather than implying a check it cannot
  make. What *is* falsifiable is the replay: without it, `no callout found for <N>` fires for every
  callout list in the suite.

  Two tests drive it through whole parses — a matching list that must not warn paired with an
  overshooting one that must, and an auto-numbered `<.>` pair — and both fail when the replay is
  removed. With this, every recognition side effect the string pipeline performs is performed from
  the tree, and what is left of step 6 is the string pipeline itself.

  *What "the string pipeline itself" still owns (surveyed, not yet built):* with every recognition
  side effect replayed from the tree, `run_pipeline` is no longer the source of anything a *caller*
  reads — but it is still the only producer of six things the parse needs internally. They are not
  one increment, and three of them are blocked on a decision rather than on effort, so the survey is
  recorded here rather than re-derived each session.

  - **A deferred cross-reference's segments** (`XrefSegment`). The node carries every field except
    [`provided_text`](../../parser/src/content/content.rs), which the segment holds as a **string**
    while the node holds its display text as *children*. Blocked on the same question as
    `role=*hl*` below.
  - **A footnote's catalog text.** Ordering, not shape: the footnote registry is consulted *during*
    the same content's substitution, for a `footnote:id[]` back-reference, so it cannot be replayed
    after the pass the way a callout's registration could.
  - **`{counter:…}` advancement.** The builder is handed a counter-safe *clone* precisely so a
    counter advances once, from the pipeline; and the value is spliced during substitution, so it
    cannot come from a tree built afterwards. This one is only solvable **by** removing the
    pipeline, not before it.
  - **The link family's dangerous-scheme warning** — the one side effect the replay does not carry.
    A dangerous scheme leaves the macro *literal*, so there is no
    [`Ref`](../../parser/src/inlines/ref_node.rs) node to hang it on. This originally read
    "recording it needs a node-level fact, the way `RawOrigin` was one"; measuring later showed it
    does not — it is the one side effect that is never **suppressed**, so the string pass records it
    until `run_pipeline` is deleted, and at the cutover the builder holds the real parser and can
    record it inline. See the step's own "landed as" note below. **Cutover work, not prep.**
  - **[`Content::passthroughs()`](../../parser/src/content/content.rs).** §4.2 says it "can be
    retained as a filtered view over the tree", and the view needs more than exists: a
    `Passthrough` carries `subs`, `type_` and `attrlist`, and no
    [`Raw`](../../parser/src/inlines/inline_node.rs) field reconstructs a resolved substitution
    group — while the `[attrs]+++…+++` form builds a
    `Styled` node rather than a `Raw` at all. It is `pub` with **no production consumer**, only its
    own tests, so *deleting* it is as live an option as building the view.
  - **The description-list term carve-out**, which registers from the string pipeline because it
    runs the steps directly and builds no tree.

  Two of the six turn on one question: whether a computed **string** slot — `Ref::roles`,
  `window`, `xrefstyle`, and the link and image families' equivalents — can hold markup that exists
  only at fold time. Today it cannot, which is why `xref:sec[*bold*,role=*hl*]` stays deferred and
  why a segment's `provided_text` cannot come from a node. Answering it once unblocks both; leaving
  it unanswered blocks both no matter how much else lands.

  *Step 6 landed as (a computed string slot takes the author's untranslated source):* the survey
  above named one question two of its six items turn on — whether a computed **string** slot
  (`Ref::roles`, `window`, `xrefstyle`, and the link and image families' equivalents) can hold
  markup that exists only at fold time — and observed that answering it once unblocks both. This
  increment answers it, for the cross-reference family.

  There were three answers available, and the first two are what the two systems had already picked.
  The **string pipeline** spells the piece into the slot: a rendered span reaches `role=` as its own
  markup (`class="&lt;strong&gt;hl&lt;/strong&gt;"`), and a masked passthrough reaches it as a bare
  `\u{96}0\u{97}` sentinel, because the deferred cross-reference's template is captured before
  passthroughs are restored. The **builder** refused the whole match, which is why
  `xref:sec[*bold*,role=*hl*]` had stayed deferred since part 3c. Neither is what the author wrote.
  What they wrote is the *source*, and a source is a value a string can hold, so that is the third
  answer and the one taken:
  [`untranslated_value`](../../parser/src/content/inline_builder/macros/mod.rs) walks a computed
  value's [`tokened_text`](../../parser/src/content/inline_builder/macros/mod.rs) tokens the way
  `restored_value_children` walks them and replaces each with the untranslated text of the node it
  stands for.

  "Untranslated" is per node kind, and the two cases differ for a reason. A
  [`Raw`](../../parser/src/inlines/inline_node.rs) leaf contributes its **value**, not its source
  span: a passthrough's `+++` / `++` / `$$` delimiters are syntax saying *do not substitute this*,
  so the body is precisely the literal text the author asked for, and the delimiters have no
  business in a class name. Every other node contributes its **source span**, so `role=*hl*` yields
  `*hl*` rather than the `<strong>hl</strong>` the Quotes step made of it. Attribute references and
  special characters are untouched by either rule — they are resolved before the value is read, so
  `role={rolename}` still expands — and only markup the *Quotes* step produced is unwound.

  With that, the per-**slot** boundary `attrlist_text_carries_its_opaque_pieces` drew disappears:
  its three "no token may reach `window` / `xrefstyle` / a role" checks are gone and
  `holds_carried_token` with them, so the family's gate is now the *split* question alone. That
  question still has members — `xref:sec[a *b, c* d,role=hl]`, whose rendered `<strong>b,
  c</strong>` hides a comma the attribute split reads, so the two readings disagree about the
  match's own extent — which is what keeps the narrowed re-fold gate from becoming vacuous now that
  the string-slot class has left it.

  The rule also makes the fold **safer** than the string it is replacing, which is worth recording
  because it is a deliberate divergence rather than a bug found. A slot's value is text, and the
  renderer escapes text for the attribute it is building, so `role=+++x&y"z+++` lands as
  `class="x&amp;y&quot;z"` — inert. The string pipeline cannot make that guarantee anywhere: a
  passthrough is restored into the *rendered* string after every escape has already run, so a
  crafted body can close an attribute and open another. That is Asciidoctor's behavior too
  ([asciidoctor#2661](https://github.com/asciidoctor/asciidoctor/issues/2661), open since 2018 and
  settled there as intended), and this crate keeps matching it on the string path; the tree simply
  does not reproduce it. For a cross-reference the string path does not even reach the value — it
  leaks the sentinel — so there is no output worth matching.

  Two unit tests pin the rule (the three slots, the `Raw`-versus-span split via
  `xrefstyle=+++full+++`, and `role={rn}` still expanding) and the escaping, and the recorder test
  that pinned the old deferral is now two: one on the split disagreement that still defers, one on
  the form that now resolves through the mirror. Audit: 48 rows before, 49 after, with three rows
  moving and all three the same two fixtures — `xref:sec[*bold*,role=*hl*]` leaving the unrecognized
  set for `class="*hl*"`, and the split-disagreement fixture appearing only because the rewritten
  recorder test introduces that source at document scope for the first time.

  One guard came out of review. The bytes a token is built from — `\u{96}` and `\u{97}` — are ones
  an **author** can write, and `build_match_string` stands an opaque piece in as a private-use
  codepoint, so every one reaching the gate is the author's own. They make the tokening ambiguous
  exactly where a value is read as a *string*: the search for a token cannot tell them apart from
  the pass's own, so it would splice a node's source into the author's text and leave the real token
  standing. [`Passthroughs::restore_to`](../../parser/src/content/passthroughs.rs) has the same
  blind spot — the wart the image bracket's own sibling test pins — but it reaches it over the
  *finished* string, where a `role=` it never restores into simply keeps the author's bytes. So such
  a text **defers**, which is precisely what the per-slot check this increment replaced did for the
  same bytes, and the four shapes are byte-identical to the increment's own base.

  *What this unblocks, and what it deliberately does not:* the survey's other blocked item goes with
  it — a deferred cross-reference's segment holds its `provided_text` as a string where the node
  holds children, and a display text is markup by nature, so that slot takes the fold of the
  children rather than the rule above. The image and link families are **not** the mechanical
  follow-on they look like, and the probe that established it is worth recording: applying the same
  rule to the image bracket moves `image:pause.png[title=*Pause* and Resume]` — a fixture from the
  AsciiDoc language docs — from `title="<strong>Pause</strong> and Resume"` to `title="*Pause* and
  Resume"` (three tests in the suite, no more). That is a **parity break** rather than a closed
  deferral, and the slot has a reading neither system offers — the span's plain text, `title="Pause
  and Resume"`, which is the only one an HTML `title` can actually show. So it wants deciding on its
  own rather than inheriting this increment's answer, and until it is decided the image family keeps
  freezing the rendered markup and the link family keeps refusing the match.

  *Step 6 landed as (a description-list term joins the tree — the last content registering from the
  string pipeline):* the increment that re-attached the recognition side effects measured what still
  registered outside the tree by widening its suppression window to a whole parse and counting the
  failures: 161, of which the second cause was "the description-list **term** carve-out, the last
  thing still registering from the string pipeline". This is that carve-out.

  A term ran the substitution steps **directly** — `Passthroughs::extract_from`, five
  `SubstitutionStep::apply` calls, `restore_to`, `finalize_deferred` — a hand copy of
  `run_pipeline`'s body written before there was a tree to build. So a term had no tree, its
  rendering was the string pipeline's rather than a fold, and its constructs registered from the
  replacers. It runs through
  [`SubstitutionGroup::apply`](../../parser/src/content/substitution_group.rs) now, under a group
  spelled out for it: the `normal` order minus its attribute-references step, which already ran
  during *parsing* so the `::` marker could be recognized at all. The seed, the authoritative fold,
  the suppression window and the replay all come with that.

  One rule had to move rather than disappear. A leading `[[id]]` / `[[id,reftext]]` in a term takes
  the **rest of the term** as its default reference text, so `[[cpu]]CPU::` makes `<<cpu>>` display
  *CPU* — a fact about where the anchor sits in a *term*, not about the node, so it cannot live in
  [`apply_ref_side_effects`](../../parser/src/content/inline_builder/macros/anchors.rs). It runs
  from the same tree instead, at the one point where the tree exists and nothing has registered from
  it yet: between the build and the replay. The replay is then told the anchor is already registered
  — which is what `leading_anchor_registered`, the parameter the side-effect increment staged and
  every caller passed `false`, has been waiting for.

  Reading that rule off the tree makes one deliberate difference. The regex it replaces ran *before*
  the macros step, so a term whose remainder held a macro registered the macro's **source**:
  `[[x]]image:a.png[]Term::` catalogued `image:a.png[]Term` as the reference text. The fold of the
  same nodes gives the rendering, which is what every other reference text on this branch is. Two
  tests pin the rule and that difference; a third pins the property the suppression window newly
  puts at risk for a term, that a registering construct in one is recorded exactly **once**.

  A second difference is a limitation of *when* a term registers rather than of reading the rule off
  the tree, and review asked for it to be pinned. The leading anchor is registered while the term is
  **parsed** — which is what lets a later cross-reference resolve it at all — and that is before any
  cross-reference *inside* the term has a destination, so a `<<b>>` there contributes its unresolved
  fallback to the catalogued reference text and keeps it: the entry is never revisited. The tree is
  the better of the two readings even so. The regex ran before the macros step, so it caught the
  same reference as escaped *source* (`See &lt;&lt;b&gt;&gt;`), never a link at all; the fold gives
  `See <a href="#b">[b]</a>`. Neither depends on whether the target is defined before or after the
  term, and `a_terms_default_reftext_folds_before_nested_references_resolve` pins both directions.

  Three pieces of machinery go with the carve-out: the `LEADING_INLINE_ANCHOR` regex,
  `apply_macros_with_leading_anchor_registered`, and `InlineAnchorReplacer`'s own
  `leading_anchor_registered` field and the check it guarded. The string replacers' *registrations*
  stay — deleting those is what happens when `run_pipeline` leaves the production path — but nothing
  passes `true` through them any more, so the branch that read it was dead.

  The audit is the cleanest kind of no-op: **49** rows either side, 0 new and 0 closed. A term now
  goes through the same fold every other content does, and it reproduces the string pipeline's own
  bytes for every term in the suite. Coverage is diff-neutral on all four changed files. Both halves
  are falsifiable: dropping the term's leading-anchor registration fails five tests, and passing
  `false` for the flag while still registering fails two (a doubled duplicate-id warning).

  *Step 6 landed as (a computed attribute slot keeps its fold-time markup, made inert where it
  lands):* the increment above answered the string-slot question for the cross-reference family and
  said in the same breath that the image and link families were **not** the mechanical follow-on
  they looked like — applying the same rule there moves
  `image:pause.png[title=*Pause* and Resume]`, a fixture from the AsciiDoc language docs, from
  `title="<strong>Pause</strong> and Resume"` to `title="*Pause* and Resume"`. Three readings were
  available and the third, `title="Pause and Resume"` (the span's plain text, the only one an HTML
  `title` can actually show), belonged to neither system. The decision is the first: **the title
  example goes on working as it does today**, with the security concern that made the
  cross-reference rule attractive answered on its own terms rather than by giving up the formatting.

  The two families were not in the same place, which is what made this one increment rather than
  two. The **image** family already froze a rendered span's markup into its bracket — it had to,
  since every value an image holds is one `render_image` writes out, so the alternative deferred
  every such bracket and the language-docs fixture lost its whole macro (the increment that closed
  the cutover's last-but-one golden regression). The **link** families drew the opposite rule for
  the same bytes: `text_attrlist` admitted a rendered span in the display text, which becomes the
  node's children, and refused the whole match when a token reached any other slot
  (`rendered_token_escaped_the_display_text`), so `link:index.html[Docs,role=*hl*]`,
  `title=*Pause*`, `window=*x*` and `id=*x*` were all left **literal** — not a different rendering
  of the macro but no macro at all, where the string pipeline builds one. That gate is deleted, so
  the three `Attrlist`-bearing families now share the image bracket's rule and the same
  `tokened_split_agrees` check remains the only thing a rendered piece must satisfy.

  What makes the frozen bytes the *right* bytes is that they are the string replacer's own: it
  parses its attribute list out of a haystack the quotes step has already rendered with this same
  renderer, so `title="<strong>Pause</strong> and Resume"` is what it writes too. The cost is the
  one the image family already carries and every frozen value on this branch owes — a *later* fold
  through a different renderer would see the parse-time renderer's markup there — and §4.6 is
  where a fold-**materialized** attribute value belongs. Until then this is parity, and it is now
  uniform across the families rather than one of them being an exception.

  The security half is what the decision asks for, and it is a real gap rather than a restatement.
  A value reaching an attribute is escaped for the `"` delimiter *where it lands* — that is the
  policy `render_image`, `render_icon`, and `render_xref` already state in so many words — and
  `render_link` and `render_icon_or_image` left three out of it: a link's `id`, the `class` both
  of them join their roles into, and the `target` a `window=` writes. Each is reachable today,
  with no tree involved:
  `link:x[Docs,role=+++a"b+++]` renders `class="a"b"` and injects whatever follows. They take
  `encode_attribute_value` now, like the `href` and `title` beside them. That is what separates
  this reading from the string pipeline's: escaping at render time is inert for markup an author
  wrote (`<strong>` carries no quote, so the formatting survives untouched) and fatal to a
  breakout — while the string pipeline splices a passthrough's body into the *finished* string
  after every escape has run, which is asciidoctor#2661 and which the tree goes on not
  reproducing. `a_rendered_slot_cannot_break_out_of_its_attribute` pins the pair.

  Audit: **36** rows either side, 0 new and 0 closed. (The count is not comparable with the
  earlier increments' because the sweep itself had to change: `rendered` *is* the fold now, so
  comparing the fold against it answers nothing. It captures the string pipeline's own answer
  where `apply` still has it — after `run_pipeline`, before the fold overwrites it — and
  compares there instead, which also means a deferred cross-reference's content, folded later, no
  longer contributes.) No golden source in the suite writes a rendered span into a link's
  attribute list, which is why the class survived this long, and none writes a `"` into one of the
  three newly-escaped slots. Falsifiability comes from the corpora instead: two rows join the
  side-effect sweep, and they fail on base for a reason worth naming — a deferred macro is not
  merely rendered literally, it also **registers nothing**, so
  `link:https://example.org[Docs,role=*hl*]` was missing from `Document::catalog()` where the
  string pipeline records it. The divergence test that pinned the old per-slot deferral is now a
  parity corpus of ten fixtures, with the split-disagreement half kept as its own test.
  Coverage is diff-neutral on all changed files.

  *What this leaves:* the cross-reference family keeps the previous increment's rule rather than
  joining this one, and the two are consistent because their string paths are: a deferred
  cross-reference's template is captured **before** passthroughs are restored, so the string
  pipeline leaks a `\u{96}0\u{97}` sentinel into `class=` and escapes a rendered span into
  `class="&lt;strong&gt;hl&lt;/strong&gt;"` — there is no output there worth matching, so the
  author's source is the better answer, while here there is one and it is Asciidoctor's. What still
  defers in the link families is the split disagreement (`link:index.html[a *b, c* d,role=hl]`, the
  markup-perturbed reading against the well-formed one) and a `mailto:`'s pre-restore subject and
  body. `render_anchor`'s own `id`, and an inline SVG's rewritten `width`/`height`, still
  interpolate unescaped; neither is reachable from this class (an anchor id admits no span, and an
  inline SVG has no attribute list a span survives into), so both are left where they are.


  *Step 6 landed as (a deferred cross-reference's segments, read off the tree):* the survey of what
  `run_pipeline` still owns named six things and said two of them turned on **one** question —
  whether a computed slot can hold markup that exists only at fold time. The increment two above
  answered it for the computed *string* slots and named this as the other: an
  [`XrefSegment`](../../parser/src/content/content.rs) holds every field a `Ref{Xref}` node already
  carries — `target`, `window`, `roles`, `xrefstyle` and `derived` are plain values the builder
  resolved at recognition time — **except** `provided_text`, which the segment holds as a string
  where the node holds its display text as *children*.

  That slot takes the **fold of those children**, and the two answers differ for a stated reason
  rather than by accident. A computed string slot had no output worth matching: the string
  pipeline captures a deferred cross-reference's template *before* passthroughs are restored, so a
  `role=` leaks a `\u{96}0\u{97}` sentinel, and where it does not leak it spells rendered markup
  into a class name. A **display text** is the opposite case — it is markup by nature, and the
  string replacer captures exactly `<strong>bold</strong>` out of its own already-rendered haystack
  — so here the fold *reproduces* the byte string rather than approximating it. One family, two
  slots, two answers, each matching what its own string path actually produces.

  [`block_tree_xref_segments`](../../parser/src/content/content.rs) and
  [`footnote_tree_xref_segments`](../../parser/src/content/content.rs) are the derivation, walking
  exactly the traversals [`assign_tree_xrefs`](../../parser/src/content/content.rs) and
  `assign_footnote_tree_xrefs` already walk — so a derived segment and an installed destination
  address the same node, and the block/footnote partition is the one
  `block_tree_xrefs`/`footnote_tree_xrefs` already draw. Nothing is wired: they are staged building
  blocks under `#[allow(dead_code)]` and a `cfg(test)` re-export, the same staging every recognition
  side effect was under before the cutover re-attached it.

  Two decisions are worth recording because a later reader will otherwise re-derive them. The fold
  runs **at the end of the parse**, with the renderer the parse carried, which is where the string
  replacer computes it too; deriving it at *resolution* time would read whatever renderer that
  caller passed and hand the resolver a different `ResolutionContext` than the string pipeline does
  — the same renderer-timing hazard review raised against the increment above, avoided here by
  construction rather than argued about. And [`resolved`](../../parser/src/content/content.rs) is
  deliberately **not** carried across: it is resolution's *output*, filled in later and mirrored
  back onto the node, so a node re-read after a sweep yields the segment it yielded before one,
  which is what makes the derivation idempotent.

  [`inline_builder_xref_segment_parity`](../../parser/src/tests/inline_builder_xref_segment_parity.rs)
  is the corpus, over whole documents parsed with `parse_deferred` (so both sides carry
  `resolved: None` and the comparison is of *recognition* alone): 44 fixtures across both
  spellings, the present-but-empty text, a display text carrying each of the three recoverable
  pieces, a passthrough, a nested macro, the attribute-list form's other fields, a derived
  destination, an unresolved target, several references in one content, a reference nested in a
  span, the footnote-embedded complement, and every other content-bearing location a deferred
  reference reaches — a **section title** (the one the document-order pass owns, including a
  forward reference and a reference between two titles) and a **table cell** among them. Every
  field of every segment is compared, and two vacuity guards keep a corpus that stopped deferring
  from passing.
  Exactly **one** shape diverges and it is the sibling increment's own, pinned as such: for
  `xref:tgt[*bold*,role=*hl*]` the `role` differs by that increment's rule while `provided_text` is
  byte-identical — which is precisely the claim this one makes.

  Review turned up a real one, and it was **not** confined to the new code: a cross-reference
  written inside a *visible index term* is deferred by the string pipeline (the replacer's
  haystack holds the term's shown text inline) while every one of these walks skipped
  [`IndexTerm`](../../parser/src/inlines/index_term.rs), the fifth nested node list and the one a
  walk written by matching on `children` is bound to miss. The side-effect sweep found exactly this
  for its own three walks an increment ago; `count_tree_xrefs`, `assign_tree_xrefs` and their two
  footnote siblings still had it. The failure mode is worse than a dropped entry:
  `See <<a>> and ((term <<b>>)) and <<c>>` derived `[a, c]` against a golden `[a, b, c]`, so once
  wired every reference *after* the hidden one would take the wrong destination. All six walks —
  the two new collectors and the four that predate them — now descend, and the corpus fixture that
  pins it is deliberately the *between-two-visible-ones* shape rather than the simpler one, since
  that is the shape a misalignment shows up in. The pre-existing walks were safe rather than wrong
  before this, because the count guard declined to correlate on the mismatch; what they were not is
  complete, and the tree was silently never authoritative for such a content.

  The index-term family's own remaining deferral shows up from this side too and stays: the
  `indexterm2:[…]` **macro** spelling reads its shown term from an attribute-list parse rather
  than from a range, so the builder keeps it as a string and builds no subtree — there is no
  node to derive a segment from. That is the builder's documented limitation, not the
  derivation's, and the same count guard keeps it safe; its own test pins the difference, so the
  day the builder learns that spelling the fixture moves into the corpus.

  Audit: **36** rows either side, 0 new and 0 closed — the walk fix changes when the correlation
  *succeeds*, not what any content renders to. Coverage is diff-neutral on both changed files
  (`content.rs` 21 missed regions and 8 missed lines, `section.rs` 40 and 39, either side).
  Falsifiable in three places, and the third had to be built: deriving `provided_text` from the
  children's source spans instead of their fold fails the corpus; reverting either collector's
  `IndexTerm` arm fails it on the between-two-visible-ones fixture; and reverting the *count* or
  *assign* arm failed nothing at all, since the corpus never reaches those. That gap is what
  `a_reference_hidden_by_an_index_term_still_correlates_onto_its_own_node` closes — it resolves
  the same shape for real and asserts each node's own `href`, which is the assertion a
  misalignment fails where a count would pass.

  *What this leaves of the survey:* three items. `Content::passthroughs()` as a tree-backed view
  (public-API shaping, and *deleting* it is as live an option as building it), the link family's
  dangerous-scheme warning (which needs a node-level fact, since a rejected macro is left literal
  and there is no `Ref` node to hang it on), and the two the survey called hard-blocked — a
  footnote's catalog text (an ordering problem: the registry is read *during* the same content's
  substitution) and `{counter:}` advancement (solvable only **by** removing the pipeline). With
  this one staged, the string-slot question the survey raised has no open consequences left.


  *Step 6 landed as (a `Raw` node records the passthrough it came from):* the survey's fifth item —
  [`Content::passthroughs()`](../../parser/src/content/content.rs) — is the one it called a live
  choice rather than a task: "*deleting* it is as live an option as building the view", since it is
  `pub` with no production consumer. The choice is made, and it is the **middle path**: keep the API
  and back it with the tree. This is that decision's first prerequisite, and measuring the gap first
  made it much smaller than the survey feared.

  The public surface is two methods, [`text`](../../parser/src/content/passthroughs.rs) and
  [`subs`](../../parser/src/content/passthroughs.rs) — `type_` and `attrlist` are `pub(crate)`, with
  no accessor — and **five of the seven** passthrough forms are already exactly recoverable from
  what the tree holds: `+++…+++` and a bare `pass:[…]` are
  [`AsIs`](../../parser/src/inlines/inline_node.rs)/`None`, `++…++` and `$$…$$` are
  `Escaped`/`Verbatim`, and a `Stem` node names its own group. So the survey's "chunkier than it
  looks" is right but localizes to two facts, both about the same form.

  A `pass:c,q[…]` body defeats [`RawForm`](../../parser/src/inlines/inline_node.rs) twice. It folds
  `AsIs` exactly as a `+++…+++` body does, so the form cannot tell an arbitrary group from no group
  at all; and an arbitrary group needs the substitution pipeline, which a fold — taking a renderer
  and a `RenderContext` rather than a `Parser`, as the increment that made that change established
  — has no way to reach, so the body is substituted at **build** time and `value` holds the
  *result* where `text()` returns the *input*. Both facts now ride on the node:
  [`RawOrigin::Passthrough`](../../parser/src/inlines/inline_node.rs) gains `subs` and
  `source_text`.

  Putting them inside the variant rather than beside it is what keeps this small and honest. Only
  the passthrough-origin construction sites change — six in the lib, all in
  [`passthrough_step`](../../parser/src/content/inline_builder/passthrough_step.rs), which is
  exactly where the extraction pass already knows the answer — and the invariant becomes
  structural: a group exists precisely when the origin is a passthrough, with no `Option` that could
  be `None` for one. `RawOrigin` loses `Copy` and gains no lifetime, `source_text` being an owned
  `String` present for one rare form.

  `RawForm` stays. It is the *fold's* contract — emit or escape — and the two deliberately disagree
  where the bare `+…+` form folds `AsIs` while its group is `Verbatim`, which two of this module's
  own tests now state rather than infer. That disagreement is the argument for keeping both: a
  reader asking what the fold does and a reader asking what the author wrote are asking different
  questions.

  [`inline_builder_passthrough_record_parity`](../../parser/src/tests/inline_builder_passthrough_record_parity.rs)
  is the corpus, comparing every record the tree holds against the entry the extraction pass made
  for the same source, in order: the delimited forms, the two `pass:` spellings, four explicit
  substitution lists, an escaped closing bracket on both, several forms in one content, the
  attribute-list-prefixed spelling, and the three containers the walk descends into. Both new fields
  are falsifiable — dropping `source_text`, or reporting `None` for a resolved list, each fails it.

  Two forms record **nothing** where the pass records an entry, and both are the view's problem
  rather than the record's: an inline **STEM** body is a `Stem` node, not a `Raw` one, and the `x-`
  **compatibility marker** sends its body through the normal substitutions as a subtree (which is
  why its entry's group is `Normal`, the one attribute-list-prefixed spelling that differs from its
  siblings). Their own test pins both. Audit: 36 rows either side, 0 new and 0 closed — no rendered
  byte moves, the fold reading `form` exactly as before. Coverage diff-neutral.

  *What the view still needs:* those two forms reached from wherever the tree does hold them, and
  then the walk itself — `Content::passthroughs()` returning a view built from the tree rather than
  a field the extraction pass fills. One thing to check before trusting either: this increment
  compared **document** order against extraction order and found them equal for every fixture, but
  the bare `+…+` form is extracted in a *second* pass, so the two orders are not equal by
  construction and the view's own increment owes that its own corpus.


  *Step 6 landed as (the last two passthrough forms record their own):* the increment before this
  one closed five of the seven forms and named the two it could not: an inline **STEM** body, which
  is a [`Stem`](../../parser/src/inlines/stem.rs) node rather than a `Raw` one, and the `x-`
  **compatibility marker**, whose body goes through the normal substitutions as a subtree. Both now
  record, so every form the extraction pass makes an entry for has one in the tree — with a single
  exception review found, recorded at the end of this note.

  Neither was recoverable, and measuring said so rather than assuming it. A `Stem` node carried
  **neither** fact: its group varies by spelling (`Stem` for a bare macro, `Custom([…])` for
  `stem:c,q[…]`, `Normal` for `stem:n[…]`) where the node kind alone would imply one answer, and
  its `value` is already-substituted text (`p &lt; q`) where
  [`Passthrough::text`](../../parser/src/content/passthroughs.rs) returns the author's `p < q`. It
  gains the same `subs` / `source_text` pair `RawOrigin::Passthrough` took, spelled out rather than
  shared: the two node kinds are far apart and a common struct would have reshaped a public type one
  increment after landing it.

  The compatibility marker turned out to be **one spelling of three**, which is what made the fix
  narrow. ``[x-]`tick` `` and `[x-]+++raw+++` already recorded correctly — their bodies are `Raw`
  leaves — and only `[x-]++attr++` has no leaf at all, its body being a subtree. So the record goes
  where the *entry* is:
  [`Styled`](../../parser/src/inlines/styled.rs) gains one
  `Option<`[`PassthroughWrapper`](../../parser/src/inlines/styled.rs)`>`, `None` for every ordinary
  span, `Some` for the wrapper the extraction pass builds. One `Option` rather than two fields,
  because the two facts are meaningless apart and absent from the overwhelming majority of spans.

  Marking the wrapper for *all three* spellings rather than only the broken one is the choice worth
  recording, because it creates an invariant a walk can get wrong: two of the three also carry the
  same pair on a `Raw` leaf **inside** the wrapper, so a walk that read the marker *and* descended
  into it would report them twice where the pass records once.
  `a_marked_wrapper_is_one_entry_not_two` pins it over all three prefixed spellings —
  deliberately including the two that *would* double-count, since the one that forced the marker
  has no inner leaf and would pass either way.

  *The order is now a decision, not an accident.* The previous increment observed document order and
  extraction order to be equal for every fixture and warned that they are not equal by construction.
  They are not: the bare `+…+` form is pulled out in a second pass and STEM in its own, so
  `+++A+++ and stem:[B] and [x-]++C++ and ++D++` extracts as `A, C, D, B` where the author wrote
  `A, B, C, D`. The view will return **document order** — the tree's own — which is a deliberate,
  documented behavior change to a `pub` method whose only consumers today are its own tests, and
  which costs nothing that survives the cutover, since extraction order is an artifact of the
  two-pass implementation step 6 deletes. The corpus therefore compares the two sides as
  **multisets** — its subject is the facts — and `the_view_returns_document_order` pins the order
  from both ends: the tree's list is exactly the source's, and it is *not* the pass's, so a fixture
  that stopped distinguishing them fails rather than quietly weakening.

  Audit: 36 rows either side, 0 new and 0 closed — no rendered byte moves. Coverage diff-neutral on
  all three changed files. Three claims, three sabotages: dropping a `Stem`'s `source_text`,
  reporting its group as the bare-macro default, and descending into a marked wrapper each fail the
  corpus. (The first sabotage initially appeared to pass and did not: `source_text: None` leaves the
  computed source unused, which `#![deny(warnings)]` makes a *compile* error, so no test ran — a
  reminder that "the suite did not fail" and "the suite disagreed" are different readings of the
  same exit code.)

  *The one shape still short of an entry,* which review found and the corpus had a blind spot for:
  a STEM expression **embedding** an already-extracted passthrough. The pass records **two** entries
  there — the inner body, and the STEM itself, whose own text keeps the `\u{96}0\u{97}` sentinel
  where that body was lifted out — while
  [`stem_expression_value`](../../parser/src/content/inline_builder/stem_step.rs) splices each inner
  body back in, so the tree keeps one `Stem` holding the *restored* text and the inner leaf is gone.
  Under an explicit substitution list it is sharper still: the expression is not local to each run,
  so `apply_stem` builds no node at all and the tree records only the **inner** passthrough. This is
  a limitation rather than a regression — a `Stem` carried neither fact before this increment, so
  the outer entry was unrecorded either way — and
  `a_stem_expression_embedding_a_passthrough_records_one_entry_of_two` pins all of it, shapes and
  counts. Closing it most likely means keeping the inner nodes as the `Stem`'s **children** instead
  of folding them into its value, which is a structural change and its own increment.

  *What the view still needs:* the walk, and the nested-STEM shape above. Every other form records;
  what remains is
  `Content::passthroughs()` returning a view built from the tree rather than a field the extraction
  pass fills, and — because this repository's `CHANGELOG.md` is generated by `release-plz` from
  commit messages rather than hand-edited — that increment's own commit message is what has to carry
  the ordering change into the release notes.

  *Step 6 landed as (a `Stem` keeps its body's own nodes):* the increment before this one closed six
  of the seven passthrough forms and left one shape short of an entry — a STEM expression
  **embedding** an already-extracted passthrough, where the pass records two entries and the tree
  recorded one. This is the structural half of closing it, landed on its own so that the view's
  increment can be purely about the walk.

  The fix turned out narrower than the note predicting it. That note named the *outer* entry's text
  as an obstacle — the pass keeps the `\u{96}0\u{97}` sentinel where the inner body was lifted out,
  while [`stem_expression_value`](../../parser/src/content/inline_builder/stem_step.rs) splices it
  back — and the decision taken was that the view should report the **restored** body. Probing the
  tree said it already does: once a `Stem` carries its own group and source (the increment above),
  the outer entry reads `("x <b> y", Stem)` on both sides. So only the **inner** entry was ever
  missing, and the whole change is to stop folding the body's nodes away. Measuring before building
  turned a structural redesign into one field.

  [`Stem`](../../parser/src/inlines/stem.rs) gains `children`. `value` is the *rendering* of those
  nodes, which is what the fold emits; `children` is what they **are**. The two are redundant for
  the overwhelmingly common flat body — a single `Text` run — and differ exactly when the body
  embeds a passthrough, which is the case the field exists for. Nothing reads it yet, so this moves
  no rendered byte: the fold still reads `value`, and the suite is green with no expectation edited.

  A second test pins the invariant that keeps the change from spreading. `apply_stem` runs
  immediately after `apply_passthroughs`, whose output is `Text` / `Raw` only, so a STEM body can
  never hold a cross-reference, a macro or a span — and therefore no *other* walk in the crate is
  now obliged to descend into `Stem::children`. `a_stem_bodys_nodes_are_only_text_and_raw` asserts
  it over five bodies that each try to smuggle one in. Asserted rather than assumed, because this
  branch has twice shipped a walk that missed a container it should have descended into — the
  `IndexTerm` children, and this same nested passthrough — and both times the corpus had covered the
  construct and the container separately but never crossed. If a later step ever moves `apply_stem`
  ahead of the extraction pass, that test fails and names the walks that then need revisiting.

  Audit: 37 rows either side, 0 new and 0 closed. (One row more than the previous increment
  reported, from this session's rebuild of the throwaway patch rather than from anything on the
  branch — `origin/inline-ast` measures 37 under the same patch. What carries across increments is
  the *difference*, never the absolute count.) Coverage diff-neutral outside the new tests' own
  `panic!` arms. Two sabotages: dropping the body's nodes, and keeping only its `Text` runs. The
  second is the one that matters, since it is the `Raw` leaf that carries the record and a corpus
  that only counted children would have passed it.

  *What the view still needs:* the walk, and one narrower shape than before. A STEM macro carrying
  an **explicit non-local substitution list** over an embedded passthrough
  (`stem:c,q[x +++<b>+++ y]`) is declined by `build_stem_node` outright — the list cannot be applied
  run-by-run around the embedded body without risking a construct that spans the boundary — so
  there is no `Stem` node at all to hold `children`, and the tree records only the inner
  passthrough where the pass records both. That is the view increment's to answer, not this one's.

  *Step 6 landed as (a `Passthrough` holds only what it exposes):* the last prep before the walk,
  and one the walk could not have done cleanly on its own.
  [`Content::passthroughs`](../../parser/src/content/content.rs) hands out
  [`Passthrough`](../../parser/src/content/passthroughs.rs) values, and *four* fields were reachable
  through that `pub` type where only **two** are documented or exposed. The other two —
  `type_` and `attrlist` — are restore-pass machinery, and neither is recoverable from the tree:
  `type_` distinguishes two attribute-list-prefixed spellings a
  [`Styled`](../../parser/src/inlines/styled.rs) wrapper renders identically, and `attrlist` is the
  author's **unsubstituted** source where the node holds an
  [`Attrlist`](../../parser/src/attributes/attrlist.rs) parsed from the *substituted* one.

  A tree-built view could therefore only have supplied `None` for both — and `Passthrough` derives
  `PartialEq`, `Eq` and `Hash` over its fields, so that is not invisible: the same `pub` type would
  have meant different things depending on which side built the value. The fix is the one this
  branch has reached for before. #1287 put a passthrough's facts *inside*
  [`RawOrigin::Passthrough`](../../parser/src/inlines/inline_node.rs) rather than beside it on
  `Raw`, so the invariant became structural rather than a rule the next increment had to remember;
  this does the same from the other end. `Passthrough` keeps `text` and `subs` — exactly what it
  documents and exposes — and a crate-internal `ExtractedPassthrough` carries that plus the two
  facts only the restore pass reads. The view is then lossless *by construction* rather than by
  care.

  There is one observable consequence, and it is the point rather than a side effect: two entries
  with the same body and the same group are now **equal**, where the attribute-list-prefixed one
  used to differ from the bare one on fields no accessor ever returned.
  `two_entries_with_the_same_body_and_group_are_equal` pins it, and asserts from the other side too
  — the pair still discriminates on body and on group — so the test is about the fields that left
  rather than about `PartialEq` having stopped discriminating at all.

  What makes this safe to assert is that the two facts had already been written down. Sixteen
  checked-in golden expectations in `substitutions_test.rs` carry `type_` and `attrlist` literally
  (`Some(QuoteType::Unquoted)`, `Some("role")`), ported from Asciidoctor's own suite; this increment
  **moves** them to the entry, it does not change one. Sabotaging either — the wrapper's `attrlist`
  or the STEM entry's `type_` — fails 21 and 22 tests respectively, which is the measurement that
  the split lost nothing rather than the claim.

  Audit: 37 rows either side, 0 new and 0 closed. Coverage diff-neutral outside the new test's own
  `panic!` arm.

  *What the view still needs:* only the walk now — `Content::passthroughs()` built from the tree, in
  document order, with the ordering change carried in that increment's own **commit message**, since
  this repository's `CHANGELOG.md` is generated by `release-plz` rather than hand-edited.

  *Step 6 landed as (`Content::passthroughs()` is a view over the tree):* the survey's fifth item,
  closed. Four increments built up to this one — a `Raw` node recording its origin's group and
  pre-substitution body, the last two forms recording, a `Stem` keeping its body's own nodes, and
  `Passthrough` narrowed to the two facts it exposes — and what is left here is the walk they were
  all for. `Passthrough::from_tree` replaces the retained extraction list; the extraction pass still
  builds its own, because the restore pass indexes into it by sentinel, but that list is now private
  to a single pipeline run and nothing outside this module reads it.

  *The order is the deliberate difference,* announced two increments ago and now real. The pass
  pulls the bare `+…+` form out in a second sweep and STEM in a third, so
  `+++A+++ and stem:[B] and [x-]++C++ and ++D++` extracts as `A, C, D, B` where the author wrote
  `A, B, C, D`; the view walks the tree and returns document order. This is a behavior change to a
  `pub` method, so it is carried into the release notes by the increment's own **commit message** —
  this repository's `CHANGELOG.md` is generated by `release-plz` rather than hand-edited. It costs
  nothing that survives the cutover: extraction order was an artifact of the multi-pass
  implementation step 6 deletes, never a documented property of the method.

  *The corpus changed subject, which matters more than it sounds.* It used to compare a walk written
  in the test file against `Content::passthroughs()`; now that the method **is** the walk, that
  comparison would assert nothing, so `golden` re-runs
  [`Passthroughs::extract_from`](../../parser/src/content/passthroughs.rs) over a throwaway
  `Content` and reads the string pipeline's own answer. `Passthroughs::observable` survives from the
  previous increment for exactly that, now `#[cfg(test)]`: it is the extraction pass's answer, and
  nothing but the differential corpus has a reason to want it.

  *One thing was asserted rather than assumed, and the reason is this branch's own history.* The
  gate on which contents report anything **moved**: a group without the macros step used to report
  nothing because the extraction pass never ran, and now reports nothing because the tree holds no
  passthrough node under that group. Same answer, different mechanism — which is precisely the
  substitution that hid the `IndexTerm` gap and the nested-STEM gap, both of which were "the
  construct works, the container works, nobody crossed them".
  `a_group_that_does_not_extract_reports_nothing` crosses them: three non-extracting groups against
  four passthrough spellings, plus the two groups that *do* extract, so the loop cannot pass by the
  fixtures having quietly stopped containing passthroughs.

  *What still diverges,* and it is two things rather than one. A STEM expression **embedding** a
  passthrough now reports both entries, but the outer one holds the **restored** body where the pass
  keeps its `\u{96}0\u{97}` sentinel — a decision, since the sentinel is the extraction pass's own
  bookkeeping and disappears with it. And under an explicit **non-local** substitution list
  (`stem:c,q[x +++<b>+++ y]`) `build_stem_node` declines the macro outright, so there is no node to
  report the outer entry from at all; building one anyway would risk a construct spanning the
  embedded body going unrecognized, trading a reporting divergence for a rendering one. Both are
  pinned by their own tests rather than left to be rediscovered.

  Audit: 37 rows either side, 0 new and 0 closed. Coverage exactly diff-neutral — 3 missed regions
  and 3 missed lines on `passthroughs.rs` before and after, the same three `panic!` arms — so every
  line of the new walk is exercised. Three sabotages: dropping the `Stem::children` descent,
  descending into a marked wrapper, and reversing the walk's order each fail a different test.

  *What this leaves of the survey:* three items. Two are the ones only the cutover itself can
  answer — the footnote catalog's text, which is an ordering problem, and `{counter:}` advancement.
  The third is the link family's **dangerous-scheme warning**, the one recognition side effect the
  replay does not carry: a dangerous scheme leaves the macro *literal*, so there is no
  [`Ref`](../../parser/src/inlines/ref_node.rs) node to hang it on, and recording it needs a
  node-level fact the way [`RawOrigin`](../../parser/src/inlines/inline_node.rs) was one.

  *Step 6 landed as (the link family's dangerous-scheme warning is cutover work, not prep):* a
  finding rather than a change. With
  [`Content::passthroughs()`](../../parser/src/content/content.rs) closed, this was the survey's
  last item that is not hard-blocked, and the obvious next increment was to give it the node-level
  fact the survey said it needed. Measuring first said it needs no such thing.

  *Why every other side effect needs one.* The builder runs against a **clone** of the parser
  (`SubstitutionGroup::apply`'s `tree_seed`), so anything it records at build time is discarded with
  that clone. A side effect therefore has to be written into the *tree* and replayed from it against
  the real parser — which is what `apply_macro_side_effects` and `apply_callout_side_effects` do,
  and why a fact with no node to live on (a `link:` macro rejected for a dangerous scheme leaves no
  [`Ref`](../../parser/src/inlines/ref_node.rs) node; probing the tree shows the whole line collapse
  into a single `Text` run) looked like it needed a new one.

  *Why this one does not.* It is the single side effect that is **not suppressed** during the
  authoritative string pass — see
  [`suppress_recognition_side_effects`](../../parser/src/parser/parser.rs), whose own doc already
  says so. The string pipeline records it, it works today, and it keeps working until
  `run_pipeline` is deleted. At the cutover the clone is gone: the builder holds the real parser,
  and its rejection site in
  [`build_link_node`](../../parser/src/content/inline_builder/macros/links.rs) — which already takes
  `parser` — becomes one `record_substitution_warning` call. Nothing has to survive a replay,
  because there is no longer a replay.

  So building the fact now would mean adding a `pub`
  [`RawOrigin`](../../parser/src/inlines/inline_node.rs) variant, and a rejected macro emitting a
  `Raw` node where it currently emits nothing, purely to bridge a gap that closes on its own —
  with a real byte-parity hazard along the way, since the
  level's match string is the **masked** haystack and the node's value would have to be the restored
  text rather than the matched bytes. That is a public type widened for the duration of a transition
  and narrowed again after it.

  The survey's six items therefore resolve as **three landed** (the cross-reference segments, the
  description-list term carve-out, and `Content::passthroughs()`) and **three that are the cutover
  itself** — the footnote catalog's text, `{counter:}` advancement, and this warning. What is left
  before step 6 is step 6.

  *Step 6 landed as (the deferred cross-references, read off the tree):* the first increment of
  step 6 proper, and the one that takes the **first** of the six survey items from a staged
  building block to the production answer.
  [`block_tree_xref_segments`](../../parser/src/content/content.rs) and
  [`footnote_tree_xref_segments`](../../parser/src/content/content.rs) have been written, corpused
  and unwired since they landed; [`Content::set_tree_xrefs`](../../parser/src/content/content.rs)
  installs what they return, so what a content carries for its deferred cross-references is what
  its **tree** said, not what `InlineXrefReplacer` recorded.

  *The partition is the substantive part, not the fields.* The string pipeline produces one flat
  list indexed by placeholder, and tells a block-level reference from one re-homed into a footnote
  by asking which placeholders its template still splices — so `block_tree_xrefs` /
  `footnote_tree_xrefs` had to re-derive that split on every resolution. The two walks partition
  **structurally**, so a [`DeferredContent`](../../parser/src/content/content.rs) now holds the two
  lists it always wanted and the template-reading split is gone from the tree-sourced path. Both
  halves then correlate positionally with the same walks that install destinations back, which is
  what makes the mirror exact rather than merely safe.

  *That exactness is the one behavior change, and it is an improvement.* A footnote subtree holding
  fewer references than the string pipeline re-homed used to make the **footnote** mirror decline
  outright — the flat list's footnote half could not be positionally correlated — leaving a
  perfectly well-recognized `<<c>>` inside the footnote unresolved in the tree. Its own list is now
  exactly the nodes it belongs to, so it resolves.
  `a_footnote_subtree_that_defers_a_reference_form_still_mirrors_what_it_holds` (renamed from
  `footnote_xref_mirror_is_skipped_…`) pins it from both ends. Nothing rendered moves: a fold emits
  a footnote's marker without descending into its subtree, so this is what a consumer reading
  `inlines()` sees, in the direction of being right.

  *What did **not** happen here is the deletion,* and measuring says why. The carve-out
  [`from_tree`](../../parser/src/content/content.rs) names — the tree holding fewer
  cross-references than the string pipeline deferred — is not a formality: the builder's documented
  unrecognized set is non-empty, and where it applies the string pipeline's answer is the *better*
  one. Two shapes reach it. `xref:sec[a *b, c* d,role=hl]` was already pinned; instrumenting the
  `rebuild_rendered` arm across the whole suite showed it is the **only** content in ~5,400 tests
  that takes the template path under a resolved parse. The second, `indexterm2:[<<b>>]`, was pinned
  only under `parse_deferred` — the construct and the *resolved* container had never been crossed —
  and it takes a different branch (an **empty** derived list, where the first is short by one), the
  branch that would otherwise clear the deferred state and render `&lt;&lt;b&gt;&gt;` where the
  document says `<a href="#b">`. Both now have resolved-path tests, and a third crosses the
  carve-out with the **title** container, which resolves through `title_refs::compute` rather than
  `Content::resolve_references` and so splits the flat list on a path of its own.

  So the sentinel system's *retirement* is complete and its *deletion* is gated on something no
  cutover increment can supply: the builder recognizing every cross-reference form the replacer
  does. That is a prep question, and naming it is this increment's other result.

  Audit: 37 rows either side, 0 new and 0 closed — the divergent **source set** is byte-identical;
  two rows move by a stateful test renderer's own callback counters, which the footnote-mirror
  improvement shifts. Coverage exactly diff-neutral on all four changed production files (21 missed
  regions / 8 missed lines on `content.rs`, 5 / 0 on `title_refs.rs`, 0 / 0 on
  `substitution_group.rs`, unchanged on `section.rs`). Three sabotages fail three distinct sets: a
  no-op wiring, a dropped `provided_text`, and a dropped footnote partition; removing the carve-out
  fails exactly the two shapes above.

  *What still defers* is the deletion itself, which now has a named prerequisite of its own — a
  block title carried across a section heading arrives at the claiming block with its inline nodes
  dropped, because `Parser::pending_block_title` has no `'src` lifetime to carry them, so it is the
  one content whose rendering cannot be a fold at all. It is the 60th of the 60 deferred titles the
  title-fold increment counted, and it keeps the template path for that reason rather than for the
  carve-out's.

  *Step 6 landed as (four recognition diagnostics, recorded where they are recognized):* the second
  increment of step 6 proper, and the first to move a side effect the tree-walk **replay cannot
  carry**. `apply_macro_side_effects` works because a registration has a node to hang on; these four
  have none. A `link:` macro with a dangerous scheme stays *literal*, an invalid substitution name
  in a `pass:`/`stem:` list is *skipped*, a `footnoteref:` builds the same node its modern spelling
  does, and a reference to an undefined footnote looks exactly like a forward one mid-parse. So they
  are recorded at the **recognition site** — the sites that already hold `parser` — and carried
  across afterwards.

  *Carrying them across is the mechanism, and it is where the increment's two real bugs were.* The
  builder runs against the counter-safe clone, whose warning buffer is discarded with it, so the
  obvious move is to drain that buffer after the build and push it onto the real parser
  ([`push_substitution_warnings`](../../parser/src/parser/parser.rs)). That is wrong twice over, and
  the suite caught both.

  First, the clone's warning buffer also collects what the builder records **incidentally**, through
  machinery it shares with the string pipeline: an
  [`Attrlist`](../../parser/src/attributes/attrlist.rs) parse expands attribute references over the
  list's own text, and where that text is a *match string* rather than document source, the
  resulting `attribute-missing` warning carries an offset that cannot be mapped back. The string
  pipeline discards exactly those at its own site; carrying the whole buffer surfaced them
  mislocated against the document root, which
  `passthrough_attrlist_drop_line_does_not_leak_a_mislocated_warning` fails on. The fix is to keep
  the two apart at the source: a diagnostic the builder *means* to report goes to its own buffer
  ([`record_builder_diagnostic`](../../parser/src/parser/parser.rs)), and only that buffer is
  transplanted.

  Second, a build **nests**. `passthrough_text` re-enters `SubstitutionGroup::apply` for a
  passthrough body, and that call clones the parser it is given — copying the new buffer along with
  everything else — so a nested drain that took the whole buffer carried the *outer* build's pending
  diagnostics across a second time. `pass:bogus[…]` reported twice. So the drain takes a **mark**,
  exactly as [`drain_substitution_warnings_since`](../../parser/src/parser/parser.rs) does, and each
  build leaves with exactly its own.

  *The string pipeline's four copies are suppressed*, not deleted — the same
  [`suppress_recognition_side_effects`](../../parser/src/parser/parser.rs) window every registration
  already rides, and deleting the replacers is what the increment that removes `run_pipeline` does.
  The transplant happens **before** `apply_macro_side_effects`, because the string pipeline raised
  these during its own pass and ahead of the registrations the replay performs, and that relative
  order is what [`inline_builder_side_effect_parity`](../../parser/src/tests/inline_builder_side_effect_parity.rs)
  compares.

  *That harness had to be taught the new channel,* which is worth recording because it is the shape
  a corpus goes quiet in: it drives the builder side directly rather than through `apply`, so
  without its own transplant every fixture exercising one of these four would have compared a
  warning against nothing and passed. Seven fixtures were added — one per class, one crossing two
  classes to pin their order against each other, and one crossing a diagnostic with a registration
  to pin the order between the two kinds.

  Audit: 37 rows either side, 0 new and 0 closed. Coverage exactly diff-neutral on all **eight**
  changed files (`parser.rs` 87/73, `links.rs` 18/9, `passthrough_step.rs` 32/14, `stem_step.rs`
  13/6, `footnotes.rs` 5/3, `macros.rs` 2/1, `passthroughs.rs` 3/3, `substitution_group.rs` 0/0 —
  missed regions / missed lines, identical on both sides). Three sabotages fail three distinct sets:
  dropping the transplant fails five tests, one per class; un-suppressing the string pipeline's link
  copy fails `rejected_scheme_records_a_warning` on the double; and draining without the mark fails
  the parity harness on the nested re-transplant.

  *What still defers is the fifth,* `SkippingReferenceToMissingAttribute` — the `attribute-missing`
  `warn` and `drop-line` diagnostic — and it is held back for a reason rather than for room. It is
  not a macro family: it belongs to the **attributes step**, it is located per *line* through
  `Content::source_lines` rather than against the content's span, and it is the very diagnostic
  whose incidental recording the first bug above is about. Giving it the same treatment means
  deciding which of the two buffers each of its sites writes to, which is its own increment's
  question.

  *Step 6 landed as (every corpus frozen, so deleting the string pipeline stays honest):* the
  third increment of step 6 proper, and the one that pays down the debt the recording increment
  named and deliberately left: "roughly twenty golden-producing helpers across `inline_builder`'s
  per-family test modules … **the fold increment's prerequisite list, not this one's leftovers**".
  This is that list, closed.

  The hazard is one step further out than the recording increment's own. The fold landed and those
  corpora stayed honest, because
  [`apply_string_pipeline`](../../parser/src/content/substitution_group.rs) kept `run_pipeline`
  callable with no tree and no fold — that increment's whole point. What they do not survive is
  `run_pipeline` being **deleted**, which is step 6's last act: at that moment a corpus whose golden
  is *computed* has nothing left to compute it from but the fold, and `assert_eq!(folded, golden)`
  becomes `assert_eq!(x, x)`.

  *Measured, not argued.* Simulating that world for one corpus — `golden_macros_in` computing its
  golden from the tree, since after the deletion there is nothing else — leaves
  `fold_matches_the_string_pipeline_through_link_macros` **green** with a stray byte appended to
  every fold. Forty-eight fixtures asserting nothing. Routing the same helper's return value through
  a recording fails it on the first fixture.

  *The mechanism is [`assert_recorded`](../../parser/src/content/inline_builder/snapshot.rs) turned
  inside out.* These corpora have no single assertion to wrap: a helper computes the golden once,
  and its several dozen callers then compare a fold against it, compare it against a literal, assert
  a documented **divergence** from it with `assert_ne!`, or merely test it with `contains`. So
  [`recorded_golden`](../../parser/src/content/inline_builder/snapshot.rs) freezes the helper's
  *return value* instead, keeping the same asymmetry — the golden is the only thing
  `ASCIIDOC_UPDATE_SNAPSHOTS=1` writes; in a checking run it is verified against the recording (the
  drift guard) and then the **recording**, not the freshly computed golden, is what the caller gets
  back. Every call site is therefore already comparing against bytes settled before the fold ran,
  and **not one of them was edited**: 28 recording sites — 16 golden helpers and 12 inline
  comparisons — against the roughly 550 call sites they feed. It also makes the deletion local: a
  helper's body becomes a lookup, and its callers do not move.

  *A corpus is keyed by its source alone,* which is the one thing that needed care: two fixtures
  sharing a source under different parser configurations are two conflicting recordings of one key.
  `decide`'s existing `Conflict` refuses that rather than merging, loudly, so the policy is
  self-enforcing rather than a convention — measuring turned up exactly seven such tests
  (`hide-uri-scheme`, `imagesdir`+`icons`, `experimental`, two `icons` renderings,
  `attribute-missing`'s modes, and `%hardbreaks` against the document attribute), each of which now
  names its own corpus through an `_in` variant. The `build_for_group` corpus takes the same
  question from the other side: the group is *the* variable, so it goes into the key rather than
  into thirty file names.

  **Thirty-nine corpora, 3,422 fixtures**, up from two and 378. What did **not** need one is worth
  recording too: a comparison against a *literal* already has a checked-in oracle and survives the
  deletion untouched (the string-pipeline half is simply deleted with it), which covers
  `quotes.rs`'s two attribute-list divergence tests, `attribute_refs.rs`'s `subs=attributes+` fold,
  and `macros/mod.rs`'s family-crossing table. The one computed golden left with no literal beside
  it — the sentinel leak `a_deferred_xref_target_over_a_passthrough_is_a_documented_divergence`
  asserts with `contains` — is recorded, so the leaked bytes the divergence is *about* outlive the
  pipeline that leaks them.

  Audit: 37 rows either side, 0 new and 0 closed — necessarily, since the whole increment is
  `#[cfg(test)]`: every changed line in the fifteen production files falls after that file's own
  `#[cfg(test)]` boundary, and the other two — `snapshot` and `test_support` — are themselves
  declared under one. Coverage exactly
  diff-neutral (595 missed regions / 332 missed lines / 43 missed functions, identical on both
  sides), with `snapshot.rs` at 100%.

  *What still defers is the structural half.* Three corpora in `parser/src/tests/` compare **trees
  and records**, not HTML: [`inline_recorder`](../../parser/src/tests/inline_recorder.rs),
  [`inline_builder_recorder_parity`](../../parser/src/tests/inline_builder_recorder_parity.rs), and
  [`inline_builder_passthrough_record_parity`](../../parser/src/tests/inline_builder_passthrough_record_parity.rs).
  Freezing those means designing a serialization for
  [`InlineNode`](../../parser/src/inlines/inline_node.rs) — a different mechanism, not a wider
  sweep of this one — and the first of the three retires
  *with* the string pipeline in any case, since both of its sides come from it. The one that
  genuinely needs freezing is the recorder-versus-builder structural cross-check, whose oracle is
  the Strategy-A recorder §5.4 retires. That is its own increment.

  *Step 6 landed as (the fifth diagnostic, and the last one):* the increment that closes the set
  the diagnostics increment opened, and it landed for a reason worth recording: **it is a
  prerequisite for the cutover's remaining swap, not a tidy-up after it.**

  The prompt for it was a throwaway probe of the *inversion* — giving the string pipeline the
  counter-safe clone and the builder the real parser, which is what makes `run_pipeline` a pure
  oracle and lets three sentinel systems go. That probe fails **27 tests of 5,474** (against 218 for
  a naive `run_pipeline` removal), and they collapse into exactly **two** root causes: the footnote
  catalog entry, and this diagnostic. Neither is unblocked *by* the inversion; both block it. The
  order in the plan was backwards, and the probe is what said so. Worth noting too is what did
  **not** appear: `{counter:}`, which had been carried as a blocker on the strength of an audit
  measurement, is a non-issue — the counter-safe clone seeds both passes from the same
  pre-substitution state whichever way round they run.

  `SkippingReferenceToMissingAttribute` was held back from the diagnostics increment for three
  stated reasons, and each turned out to be answerable rather than deep. *It belongs to the
  attributes step, not to a macro family* — so it is recorded in
  [`apply_attribute_references`](../../parser/src/content/inline_builder/attribute_refs.rs) rather
  than at a macro's recognition site, which changes where the code lives and nothing else. *It is
  located per line* — but only because that is how the string pipeline recovers a span it has
  otherwise lost (`warning_source` matches a per-line byte range back against the line's text, and
  degrades to the whole content when there are no source lines). The tree needs none of that:
  [`source_slice`](../../parser/src/content/inline_builder/quotes.rs) maps the match's own range
  back to `'src` directly, which is the same mapping every node's `location` already takes. *And it
  is the very diagnostic whose incidental recording was that increment's first bug* — which is
  precisely why it is landable now: the two-buffer split that fixed that bug is what keeps an
  `Attrlist` parse's own `attribute-missing` warning (recorded through
  `record_substitution_warning`, over a match string, unmappable) apart from this one (recorded
  through `record_builder_diagnostic`, deliberately). The earlier increment paid for this one.

  *Two things in the design are load-bearing, and both were found by sabotage rather than by
  reading.*

  **The diagnostic is decided by the document's mode, not by
  [`MissingHandling`](../../parser/src/content/inline_builder/attribute_refs.rs).** That enum
  answers what a missing reference *renders* as, and it collapses distinctions this needs: `skip`
  and `warn` are both `Literal` because they emit the same bytes, and both `for_content` and
  `nested` fall back to `Literal` for the two shapes whose line correspondence the transducer
  cannot reproduce. None of that is a reason to stop diagnosing — the string pipeline scans a flat
  rendered string in which a span's contents are ordinary text, so it warns for a nested reference
  and for a line-straddling span alike. Reading `AttributeMissing` directly keeps the deferrals
  about output bytes, where they belong. It also means the `drop-line` divergence this transducer
  documents is a divergence in **bytes only**: the diagnostic agrees.

  **The order has to be restored, not kept.** The splicing recursion visits a `Styled` child's
  content *before* its own level, so `{alpha} *bold {beta}*` finds `beta` first while the string
  pipeline's line scan sees `alpha` first — the same hazard `resolve_counters` exists to correct
  for counter directives. A stable sort by source offset fixes it, and the sort is *the* thing
  nothing pinned: removing it left the whole suite green. So the increment's real test is a new
  configured pair in
  [`inline_builder_side_effect_parity`](../../parser/src/tests/inline_builder_side_effect_parity.rs)
  sweeping both diagnosing modes over ten fixtures, four of them straddling a span, with a
  non-vacuity guard per fixture. That harness compares warnings **in order**, which is exactly the
  question.

  Audit: 37 rows either side, 0 new and 0 closed. Coverage exactly diff-neutral on both changed
  production files (`attribute_refs.rs` 11/6, `substitution_step.rs` 2/0 — missed regions / missed
  lines, identical on both sides). Three sabotages fail three distinct sets: dropping the builder's
  recording fails the same **eight** tests the inversion probe named as this cause, un-suppressing
  the string pipeline's copy fails the same eight on the double, and removing the sort fails only
  the new parity sweep.

  *What still defers is the footnote catalog entry* — the other root cause the probe named, and the
  last thing between here and the inversion.
  [`register_footnote_number`](../../parser/src/content/inline_builder/footnotes.rs) registers
  `normalize_footnote_text(raw_content)`, the *unrendered* text, and threads no cross-reference
  placeholders through, so `define_footnote` never builds a `FootnoteDeferred` for a tree-built
  footnote. Both are invisible today because the registration lands on the discarded clone; invert,
  and they become the catalog (`footnotes[0].text` empty where the string pipeline registers
  `<a href="…">GitHub</a>`, and a footnote's own `<<tgt>>` no longer resolving). The entry's `text`
  has to become a fold of the footnote's own subtree, and the entry has to carry the deferred
  segments. It is testable ahead of the inversion through the same side-effect parity harness,
  which drives the builder directly and so can see what the clone registers.

  *Step 6 landed as (the footnote catalog entry, folded from its own subtree):* the probe's other
  root cause, and the last thing between here and the inversion.

  [`register_footnote_number`](../../parser/src/content/inline_builder/footnotes.rs) now takes the
  footnote's `children` instead of its raw bracket text, and builds the catalog entry by **folding
  that subtree**. What it registered before was the *match string*, in which an already-recognized
  construct is one opaque `SPAN_PLACEHOLDER` codepoint: `footnote:[see https://github.com[GitHub]]`
  registered `"see \u{e0f0}"` where the string pipeline registers
  `"see <a href=\"https://github.com\">GitHub</a>"`. Invisible today only because the registration
  lands on the discarded clone.

  **The mechanism is a second mode axis on the fold.** `fold.rs` already threads
  `Footnotes::Marked`/`Stripped` — "does this fold write a footnote's in-flow marker?" — for the
  same underlying reason: a tree is folded to more than one string, and which one is wanted is a
  question about node kinds rather than about bytes. `Xrefs::Rendered`/`Deferred(&mut Vec<XrefSegment>)`
  is the same shape for cross-references. Under `Deferred`, `fold_xref` writes
  `Content::xref_placeholder(n)` and pushes `xref_segment_from_node(...)` instead of rendering, so
  the new entry point [`fold_deferring_xrefs`](../../parser/src/content/inline_builder/fold.rs)
  yields the placeholder **template** and the segment list **in one pass**, in matching order, which
  is exactly the pair `define_footnote` turns into a `FootnoteDeferred`. The string replacer reaches
  the same pair from the other direction — it re-homes the block template's placeholders out of the
  already-substituted text it cut the footnote from — so a footnote's own `<<tgt>>` now resolves on
  either side. Reusing `xref_segment_from_node`, which the block-level walk already uses, is what
  keeps the two readings of "what does this node defer?" from drifting.

  Refolding the subtree *after* resolution was considered and rejected: `Footnote::resolve_references`
  runs on the **catalog entry**, driven from the catalog, with no access to the tree — it would need
  whole new plumbing. Registration is also the last moment the subtree is final;
  `apply_post_replacements` descends into a `Styled`/`Ref` child but not into a `Footnote`'s, and the
  string pipeline agrees because the footnote's text is out of the flat string by then.

  **The cost is a documented exception to an invariant, and it is not optional.** Building the tree
  is otherwise unobservable — `inline_recorder`'s
  `building_the_tree_does_not_consult_the_documents_renderer` measures that with a stateful
  renderer — and this is the first fold that runs *during* a build. It cannot be avoided: a
  footnote's catalog entry is a **required** recognition side effect (the same reason its number is;
  a second `footnote:id[]` in the same content has to find the first one's id already registered),
  and the entry's payload is a rendered string, so registering it and rendering it are one act. The
  string pipeline does exactly the same thing at exactly the same moment. It adds no *second*
  rendering: `fold_footnote` writes only the in-flow marker, never the children, so a footnote's
  subtree is folded exactly once per parse either way. `footnote:[a < b]` moves out of
  `RENDERER_FREE_CONSTRUCTS` into a new pinned test that also asserts the complement
  (`footnote:[plain text]` still consults nothing), so the exception is a decision rather than a gap.

  **The harness found a bug nothing else could see.** Extending `SideEffects` in
  [`inline_builder_side_effect_parity`](../../parser/src/tests/inline_builder_side_effect_parity.rs)
  to carry the footnote catalog — compared *whole*, including `location`, which `Footnote`'s own
  `Debug` omits — failed immediately on a fixture whose text matched byte-for-byte: the builder was
  anchoring the entry at the **macro's** span where the string replacer anchors it at the enclosing
  **content's**. That location is what a footnote's unresolved-reference warning is reported
  against, so it was inert only while tree-built footnotes never carried a `FootnoteDeferred`. It
  now passes `root`, the same span this pass already hands
  `record_builder_diagnostic`. The sweep adds 32 footnote fixtures plus a `compat-mode` pair for the
  deprecated `footnoteref:` spelling, and guards reachability of the deferred half specifically (a
  corpus registering footnotes but none carrying a cross-reference would compare `None` against
  `None`).

  **Two entry-level divergences survive, both pinned.** A passthrough or a STEM expression inside a
  footnote: the string pipeline restores a passthrough *after* the macros step, over the whole block
  string, by which time the footnote's text has been cut out of it — so its entry keeps a raw
  passthrough sentinel that nothing will ever replace (`\u{96}0\u{97}` reaching public API, one of
  §4.2's three sentinel systems leaking). The tree has no sentinels and folds the restored text, so
  here the **tree is right and the string pipeline is wrong**, which is why it is pinned as a
  divergence rather than matched. And a cross-reference inside a *link's display text*, which the
  builder does not recognize at all — a pre-existing gap in the link family that shows identically
  outside any footnote, and which the entry is merely where it becomes visible in a side effect. A
  third, kept out of the corpus rather than pinned: outside compat mode a construct-bearing
  `footnoteref:` raises a deprecation warning quoting the matched macro, and each pipeline quotes
  its own placeholder alphabet — a divergence in the warning's payload that the tree cannot close
  from its side, since it has no string haystack to quote.

  Audit: 37 rows either side, 0 new and 0 closed. Five rows differ only in the test-only Strategy-A
  recorder's PUA event-index digits, which shift because the build-time fold records events through
  that renderer too; normalizing those digits away collapses both sets to the same 36 lines with
  nothing added or removed. Coverage exactly diff-neutral on all three changed production files
  (`content.rs` 21/8, `fold.rs` 3/3, `footnotes.rs` 5/3 — missed regions / missed lines, identical
  on both sides). Six sabotages, each failing a distinct set: discarding the captured segments,
  shifting the placeholder index by one (which reaches a `debug_assert!` in `render_template` and
  fails twelve suites), dropping the `trim`, dropping the newline collapse, re-anchoring at the
  macro span, and folding the deferred reference's children into the flow after its placeholder.
  Removing the fold entirely fails all four new or extended tests at once. A seventh could not be
  written: `fold_anchor` takes no sink at all, so an anchor's reference text — which reaches
  `render_anchor` rather than `out`, and would therefore contribute a segment with no placeholder —
  is structurally prevented from deferring rather than merely told not to.

  *What still defers is the inversion itself* — giving the string pipeline the counter-safe clone
  and the builder the real parser. Both of the probe's root causes are now closed, so re-running it
  is the next increment's first act. After that: deleting `run_pipeline`, the three sentinel
  systems, and the `with_inline_tree` flag; and the structural freeze the recording increment left
  owed — the three corpora that compare **trees and records** rather than HTML
  (`inline_recorder`, `inline_builder_recorder_parity`,
  `inline_builder_passthrough_record_parity`), which needs an `InlineNode` serialization rather than
  a wider sweep of the HTML recording mechanism.

  *Step 6 landed as (the inversion):* the swap the last two increments were prep for. The string
  pipeline and the builder have exchanged parsers.

  Until now `apply_inner` ran `run_pipeline` on the **real** parser with its recognition side effects
  suppressed, and built the tree on a **counter-safe clone** whose mutations were thrown away. That is
  backwards for everything except history: the tree is authoritative for the rendered string, so the
  pass that writes what the document keeps should be the one holding the parser that keeps it. Now
  `run_pipeline` runs on the clone — its counters, its catalog registrations and its warnings are all
  discarded with it — and `build_for_group` runs on the real parser.

  **`run_pipeline` is now a pure oracle**: it computes a string and writes nothing anyone reads. That
  is the whole point of doing this before deleting it — the deletion becomes a deletion rather than a
  rewrite, and until then the differential corpora go on calling it through `apply_string_pipeline`
  and go on differentiating for real.

  *Three things had to move with it, and only three.*

  **The recursion guard needed a cell.** `build_inline_tree` was doing two jobs: *configuration*
  ("does this parse build trees") and *reentrancy* ("is one already building"). The seam could clear
  the second because the parser driving the build was an owned clone; it is a shared reference now, so
  reentrancy moved to [`Parser::in_inline_build`](../../parser/src/parser/parser.rs) and the two come
  apart cleanly.

  **The build's own warning buffer became the whole of the separation.** A build records its
  deliberate diagnostics through `record_builder_diagnostic`. What it records *incidentally* through
  `record_substitution_warning` — an `Attrlist` parsed out of a match string raising its own
  `attribute-missing` warning at an offset into that string, which is not a position in the document
  source — used to be discarded by sitting on the clone. It does not any more, so the seam discards it
  out loud, with the same `substitution_warnings_len`/`truncate_substitution_warnings` idiom every
  other owned-source substitution in the crate already uses. `['{missing}']++x++` is the shape, and
  `passthrough_attrlist_drop_line_does_not_leak_a_mislocated_warning` is what caught it — the one test
  that failed on the first green build of the inversion.

  *That discard was too broad on the first pass, and review caught it.* One thing inside a build's
  window is **not** incidental: the re-entrant `apply` a passthrough body gets, which takes no tree
  seed and so is that body's authoritative pass. Its warnings are ordinary, located warnings — and the
  blanket truncation ate every one of them, silently, with the whole suite green (`pass:a[{missing}]`
  under `attribute-missing=warn` reported nothing where the base reports a
  `SkippingReferenceToMissingAttribute`). A high-water mark cannot fix it: incidental and
  authoritative warnings interleave within one build, so the range to discard is not a suffix. The
  nested pass therefore moves its own warnings out of the window the moment it finishes
  ([`Parser::nested_authoritative_warnings`](../../parser/src/parser/parser.rs)) and the seam hands
  them back after truncating, which keeps the invariant ("a build's substitution-buffer output is
  incidental") exactly as stated while making it true.
  [`passthrough_body_warnings_survive_the_builds_own_discard`](../../parser/src/content/passthroughs.rs)
  is the pin, and it is the complement of the mislocated-warning test above — the two together are the
  whole of the distinction.

  A second review pass then asked what *location* those rescued warnings carry, which is worth
  recording: the answer is "the wrong one, and always has been". `passthrough_text` substitutes a body
  as **owned** text, so a warning the body raises is located against that text rather than against the
  document and comes out at offset 0 however far in the passthrough sits. `origin/inline-ast` reports
  byte-for-byte the same span for every fixture, so the rescue is behavior-preserving down to the
  offset — the bug is in how a body's warnings are located, and closing it means deciding whether such
  a warning should be *remapped* onto the body's position or *discarded* the way every other
  owned-source substitution's is. That is its own change; the increment below takes it, and answers
  *remap*. The "byte-for-byte the same span" claim held for every fixture reached for here but not
  for all of them — the `pass:` macro whose own list includes `a` moves, which is what that increment
  starts from.

  **A passthrough body's re-entry changed hands.** `passthrough_text` re-enters
  `SubstitutionGroup::apply` for a body carrying its own substitution list, and that re-entry now
  happens from inside the build, on the real parser. It takes no tree seed (the guard), so its string
  pipeline *is* the authoritative pass for that body and registers directly — which is the same net
  effect the suppressed arrangement had, reached the other way round: the enclosing tree cannot replay
  a body's constructs, because a body folds to one `Raw` value rather than to nodes.

  *That last one is where the increment's real test came from, and it took a sabotage to find.* With
  the guard removed the whole suite stays green — the nested body is simply substituted twice, and the
  two passes agree on every byte, so a stateless renderer cannot tell. A **stateful** one can: the
  branch's own `OrdinalRenderer` (the device §4.2's observability tests already use) counts three
  renderer calls for `pass:c[a < b]` with the guard and four without, and the ordinal that reaches the
  output moves from `a [3] b` to `a [4] b` — a real output change for any backend whose renderer
  carries state.
  [`a_passthrough_body_is_substituted_once_per_apply`](../../parser/src/tests/inline_recorder.rs) pins
  it, with the bare-`+` mixture body as the control that does *not* move (it reaches the renderer
  through a different path), and it is the only test in the suite that catches either half of the
  guard — the seed condition or the `replace(true)`.

  Three further sabotages fail loudly rather than subtly, which is what says the inversion itself is
  pinned by the existing suite: putting the string pipeline back on the real parser fails twenty-plus
  tests (everything registers and counts twice), putting the builder back on a clone fails ten
  (nothing advances the real counters at all), and keeping the build's incidental warnings fails the
  mislocated-warning guard. Two more cover the correction: removing the nested pass's rescue, or
  never handing the rescued warnings back, each fails only the new complement test.

  One further branch the inversion made reachable is exercised rather than assumed. `run_pipeline`
  goes to the real parser in two cases now — a passthrough body inside a build, and a parse that
  builds no tree at all — and only the first must have its warnings carried across a pending
  truncation. `with_inline_tree` is retired, so the second is reachable only from a crate test, and
  [`a_parse_that_builds_no_tree_keeps_the_string_pipelines_warnings`](../../parser/src/tests/inline_recorder.rs)
  is it: it is what keeps `build_inline_tree`'s remaining, configuration half from being a branch
  nothing ever takes.

  Audit: **37 rows either side, 0 new and 0 closed** — the inversion moves no divergence at all. The
  raw count on the branch reads 43; all six extra rows belong to this increment's own new test, which
  is the first in the suite to drive `SubstitutionGroup::apply` under a stateful renderer, and they
  *are* the phenomenon it measures (the oracle and the fold reading successive ordinals off one shared
  counter). Deleting just that test returns the count to 37 against an unchanged baseline, which is
  how the attribution was checked rather than assumed.

  Coverage is **not** diff-neutral here, and the exception is the point. `substitution_group.rs` stays
  at 100% and `passthroughs.rs` at 3/3; `parser.rs` goes from 87/73 to 90/76 (missed regions / missed
  lines), and the three added lines are exactly the `if self.suppress_recognition_side_effects.get() { return }` early returns in
  `register_ref`, `register_image` and `register_link`. Nothing sets that window any more — the seam
  was its only setter — so the mechanism is **vestigial** as of this increment. It is left standing on
  purpose: removing three of its ten read sites while leaving the field and the other seven would be
  worse than removing none, and the whole of it goes with `run_pipeline` in the increment that is
  already scoped to take it.

  *What still defers is the deletion itself* — `run_pipeline`, the three sentinel systems (design
  §4.2), the `suppress_recognition_side_effects` window, and the `with_inline_tree` flag, whose
  configuration half is now all that `build_inline_tree` carries. Separately still owed: the
  structural freeze — the three corpora that compare **trees and records** rather than HTML
  (`inline_recorder`, `inline_builder_recorder_parity`,
  `inline_builder_passthrough_record_parity`), which needs an `InlineNode` serialization.

  *Step 6 landed as (the passthrough record corpus, frozen):* the first slice of the deletion, and
  a **scoping** increment as much as a code one. The menu the inversion left is four deletions and a
  freeze; surveying the actual surface says it is not four-plus-one but a different shape, and this
  increment lands the piece that survey puts first.

  *What the survey found, since it revises the menu above.* `apply_string_pipeline` is `#[cfg(test)]`
  and `run_pipeline` has exactly **two** production callers, both in `apply_inner`: the oracle pass
  on the clone, whose output `content.rendered` is overwritten by the fold three statements later,
  and the `tree_seed == None` pass, which is *authoritative*. So the deletion is not gated on the
  oracle at all — it is gated on that second branch, which a passthrough body's re-entry reaches
  from inside a build (the guard) and which nothing else can answer for while a body folds to one
  `Raw` value rather than to nodes. That is production work, and it is the real blocker; the ~30
  test call sites are the tractable half.

  Those test call sites are also **not** homogeneous, which is what splits the freeze. Most are
  already frozen — either through [`recorded_golden`](../../parser/src/content/inline_builder/snapshot.rs)
  (the thirty-nine corpora of the freeze increment) or against a literal golden written into the
  test, which is a freeze by another name; for both, deleting the pipeline is a mechanical drop of
  an argument. What is left is the corpora that compare something other than HTML, and they divide
  by the *shape of what they compare*, not by whether they are "structural":

  - **record-shaped** — a flat list of plainly serializable facts.
    `inline_builder_passthrough_record_parity` (a `(body, group)` pair per passthrough) and
    `inline_builder_side_effect_parity` (a catalog-and-warnings snapshot). Neither needs an
    `InlineNode` serialization; each needs a codec for its own record.
  - **tree-shaped** — `inline_builder_recorder_parity` and `inline_recorder`, which do need one,
    and whose comparison is not equality but
    [`assert_trees_equivalent`](../../parser/src/tests/inline_builder_recorder_parity.rs) — a
    pairwise normalization that deliberately ignores `location` and `attrs` and splits a recorder
    `Text` run to meet a builder leaf boundary. Freezing those means choosing a per-side normal
    form, which a pairwise diff is not, and is the harder half by a wide margin.

  Two corrections to the menu fall out. `inline_builder_passthrough_record_parity` is filed above
  with the corpora needing an `InlineNode` serialization and does not need one — its golden is
  [`Passthroughs::extract_from`](../../parser/src/content/passthroughs.rs), a flat list. And
  `inline_builder_side_effect_parity` is **missing** from the list entirely, though it is a live
  differential calling `apply_string_pipeline` and dies with the same deletion. The freeze is four
  corpora, not three.

  *This increment freezes the first of the record-shaped pair*, in the shape the freeze increment
  established: the golden helper's body becomes a lookup and not one of the module's six assertions
  moves. The one design difference is that it is a **round trip** rather than a string comparison.
  `recorded_golden` hands back bytes, which is all the golden-HTML corpora ever wanted; this
  corpus's assertions read the golden's *structure* — its length, whether it `contains` one of the
  view's entries, and whether its outer STEM entry still carries the `\u{96}` extraction sentinel —
  so the recording is decoded back into `Vec<(String, SubstitutionGroup)>` and the tests go on
  reading it exactly as they did. The codec is the increment's only new machinery: a body quoted
  with the store's own [`quote`](../../parser/src/content/inline_builder/snapshot.rs), a group as
  its `Debug` spelling, tab-separated, so a record stays one physical line the way the recording
  format requires.

  *That sentinel is the reason to freeze this corpus rather than retire it with the pass.*
  `golden[1].0.contains('\u{96}')` asserts a byte that exists only while the extraction pass does,
  and it pins a documented difference between the two sides — the pass keeps the sentinel where the
  view reports the restored body. Retiring the corpus with the pass would make that difference
  untestable at exactly the moment it stops being observable; freezing it keeps the artifact the
  deletion removes, in bytes, on the other side of the deletion. The recording carries it
  (`"x \u{96}0\u{97} y"`), which is what makes this a freeze worth doing rather than a formality.

  The codec is the one part the corpus cannot exercise whole — its fixtures produce four groups and
  two custom steps between them — so
  [`the_record_codec_round_trips_every_spelling`](../../parser/src/tests/inline_builder_passthrough_record_parity.rs)
  sweeps every group and step spelling, the empty list (the arm `decode` special-cases, since
  `"".split('\t')` yields one empty field rather than none), and the bodies that would break a
  line-based format. The drift guard was checked by corrupting a recorded body: two fixtures fail
  with "the string pipeline no longer produces the recorded rendering", which is the guard doing the
  job it will keep doing until the pipeline goes.

  Nothing in production moved, so the audit is a formality rather than a measurement — **63 rows
  either side, 0 new and 0 closed** — and coverage is diff-neutral by construction: the two changed
  production files differ only in a visibility keyword, add no executable line, and both sit at
  100% (0 missed regions, 0 missed lines) either side. The codec itself lives under `src/tests/`,
  which the coverage report does not measure at all, so its round-trip test is what covers it rather
  than a number.

  *That 63 is not the 37 the previous increments reported, and the difference is the recipe rather
  than the branch.* The audit as CLAUDE.md records it begins by forcing the tree seed on
  (`if parser.build_inline_tree` → `if true`), which is now a **no-op** — every parse builds a tree
  since the `with_inline_tree` opt-in retired — and forcing it breaks the one test that turns the
  flag off deliberately. And the fold now *writes* `content.rendered`, so the string pipeline's own
  output is no longer what sits there after `apply`; it is what sits there in the window between the
  pipeline's return and the fold's assignment, which is where the comparison has to go.
  `set_tree_xrefs` runs in that window but touches only `deferred`, so the value is still the
  pipeline's. Both sides here were measured with that corrected recipe, which is what makes the
  comparison like-for-like; the bar is unchanged (no **new** row), and the absolute count is only
  comparable against a run of the same recipe.

  *What still defers* is the rest of the decomposition this increment's survey drew, in the order
  the survey puts it: (1) the second record-shaped corpus,
  `inline_builder_side_effect_parity` — the same freeze over a richer record, and the one the menu
  omitted; (2) the tree-shaped freeze, `InlineNode` serialization and a per-side normal form for
  `inline_builder_recorder_parity` and `inline_recorder`, which is where the `InlineNode`
  serialization the menu names actually belongs; (3) the **authoritative-pass closure** — the
  `tree_seed == None` branch, which is the production blocker and the only one of these that is not
  test-side; and only then (4) the deletion itself: `run_pipeline`, `apply_string_pipeline`, the
  escaping pass, the three sentinel systems (§4.2), the `suppress_recognition_side_effects` window,
  and `Parser::build_inline_tree`. The public `with_inline_tree` opt-in named in the menu is already
  gone — only doc prose still mentions it — so what remains under that name is the field.

  *Step 6 landed as (a passthrough body's warnings, located):* the open question above, answered in
  favor of **remapping**. A warning raised while substituting a passthrough body now points at the
  reference the author wrote, not at the document start.

  Review of the inversion filed this as an *escape*: the unguarded `Attrlist::parse` in
  `PassthroughRestoreReplacer` sits inside the interval the nested authoritative pass drains, so
  `pass:m[['{missing}']++x++]` under `attribute-missing=warn` should surface a mislocated
  attribute-list diagnostic rescued as authoritative. Measuring against the inversion's parent said
  otherwise, twice. That fixture raises no warning at all, on either side, under either mode. And
  bracketing the `Attrlist::parse` the way its neighbour is bracketed changes no surfaced warning
  anywhere: logging every warning that call records across the whole suite finds three, all with
  `in_inline_build` false — recorded on the oracle's clone, discarded with it, never rescued. The
  enclosing build's own discard already covers that site by construction, so the guard was left off
  rather than added as unreachable defensive code with no test able to reach it.

  The same measurement found a real divergence beside it. Adding `a` to a `pass:` macro's list is what
  exposes it: `pass:m,a[['{alpha}']++x++ and {beta}]` reported `alpha` at offset 2 before the
  inversion and at offset 0 after it, while the body's own `{beta}` stayed correctly at 30 on both
  sides — and under `drop-line` the `alpha` warning was *absent* before and present after. Rendered
  output is byte-identical either side; this is a diagnostics-only shift. Its cause is that the
  warning changed which pass records it. Before the inversion the body's re-entry built a tree of its
  own, and the builder's `attribute_refs` recorded the reference at its offset *within the body* (2 in
  `['{alpha}'`) read as though it were a document offset. After it, `Parser::in_inline_build` stops
  that nested build, so the body's string pass records instead — and, finding no source line to
  correlate against, falls back to the whole-body span, i.e. offset 0.

  **Both numbers were wrong**, which is what took restoring the older one off the table: `{alpha}`
  sits at offset 11, and neither 2 nor 0 points at it. The single root cause is that
  [`passthrough_text`](../../parser/src/content/inline_builder/passthrough_step.rs) seeded the body's
  `Content` from an unanchored `Span::new`, so a body's references had no document position to be
  located against by *either* pass. It now takes the body's own `Span` and carries per-line spans the
  way block construction does, which is what makes the location precise rather than merely in the
  right neighborhood — without them `apply_attributes` falls back to the whole-body span and every
  reference in a body reports at the body's start. Retained spans are only used when the retained text
  still equals the matched reference, so a body whose rendered lines have drifted from its source
  lines falls back rather than mislocating; and a body reached through an unescaped `\]` copy, whose
  bytes no longer line up with the document, keeps the unanchored form deliberately.

  Every fixture now reports the reference's own offset: `alpha` at 11 and `beta` at 30 in the shape
  above, `pass:a[{missing}]` at 7, and — the one that says the two paths agree rather than both merely
  moving — `pass:a[{missing}]` on the third line of a document at `(3, 33)`, where the plain
  non-passthrough control has always been `(3, 26)`.
  [`a_rescued_passthrough_warning_points_at_the_reference_itself`](../../parser/src/content/passthroughs.rs)
  pins that pair, replacing the test that pinned the old answer, and
  [`a_nested_attributed_passthrough_locates_each_reference_separately`](../../parser/src/content/passthroughs.rs)
  covers the nested-attributed shape nothing covered before — the gap the shift went unnoticed
  through — with two references in one source so each is checked on its own rather than one offset
  happening to be right.

  Audit: **63 rows either side, 0 new and 0 closed**, run with the corrected recipe the previous note
  records (a fold-versus-`rendered` compare at the assignment itself, no forced seed) so the count is
  directly comparable against that increment's own 63. The set comparison is the invariant either way,
  and it was also run against the increment's original base with a second, independently shaped patch
  (the canonical fold-before-`set_inlines`, 53 rows) with the same answer.

  Coverage is diff-neutral: missed **regions** and missed **lines** are unchanged in all three touched
  files (22/10, 13/6 and 3/3), and the missed lines are the same lines either side rather than merely
  the same count.

  *What still defers* is unchanged from the note above: the survey's decomposition in its own order —
  the second record-shaped corpus, the tree-shaped freeze, the authoritative-pass closure, and only
  then the deletion itself.

  *Step 6 landed as (the side-effect corpus, frozen — the record-shaped pair closed):* the second
  item on the survey's own list, and the one the pre-survey menu had omitted entirely.
  `inline_builder_side_effect_parity` is a live differential whose golden side is
  `SubstitutionGroup::apply_string_pipeline`; without a freeze, deleting that pass would leave every
  assertion in the module comparing the builder against itself, silently and with the suite green.

  The shape is the freeze increment's: the helper computing the golden becomes a lookup, the pipeline
  goes on being called and goes on being checked against the recording on every fixture (the drift
  guard), and **not one of the module's assertions changes what it asserts**. It is a round trip
  rather than a byte comparison for the same reason the passthrough corpus's is — the assertions read
  the golden's *structure*: each of five list lengths, whether a footnote entry's `deferred` is
  `Some`, a footnote's `text`, and the ids the reference catalog holds.

  *Where this record is genuinely richer, and what that cost.* The passthrough record is a
  `(body, group)` pair. This one is five lists, one of them `Vec<Footnote>` compared **whole** — index,
  id, text, deferred cross-references and location. Three decisions fall out, and the middle one is
  the increment's only real trap.

  **`WarningType` is recorded as its `Debug` spelling, not decoded into the enum.** Fifty-odd
  variants, and a recording has to reconstruct whichever a fixture produced; `Debug` is total over
  the enum where a hand-written decoder is a fifty-arm match kept in sync by hand, and it is
  injective for these payloads (a variant name plus `String` fields), so equality over spellings is
  equality over values. `RefType` keeps its real type — three variants, a three-arm decoder — which
  is what lets `the_replay_is_not_a_no_op_for_any_family` go on asserting `RefType::Anchor` directly.
  Its `WarningType` literals go through the same spelling via a `spellings` helper, so that assertion
  still names the two values rather than a pair of hand-written strings.

  **A `Footnote` is recorded field by field, and reaching for its own `Debug` would have been a
  silent loss.** `Footnote`'s `Debug` omits `location` — and `location` is one of the five facts this
  corpus exists to compare. Encoding the struct whole the way `warnings` is encoded would have
  dropped it from the freeze with nothing failing. `FootnoteDeferred`'s `Debug` omits
  `sentinels_escaped` in the same way, and there it costs nothing: the harness already normalizes
  that field away before comparing, because it records which *pipeline* built the entry, which is
  precisely what a differential must not read as a difference. The normalization is kept rather than
  dropped as now-redundant — the statement should not rest on which fields a `Debug` impl happens to
  print.

  **This is the first corpus whose recording key is not the fixture source alone.** It runs the
  *same* source under more than one parser configuration — `Hello, {alpha}!` is swept under both
  `attribute-missing=warn` and `attribute-missing=drop-line`, which write different warning lists —
  so a source-only key collides and the store reports a `Decision::Conflict` on the second. The key
  is `config\u{1}source`, with the separator chosen over a readable `[tag] ` prefix because fixtures
  in the corpus genuinely begin with `[` (`[[the-anchor]]…`).

  The format is counted rather than delimited: five variable-length lists in a row, each a decimal
  count followed by its entries' fields, every string field through the store's own `quote` (a
  footnote's text spans lines, and the string pipeline's own output carries Private-Use-Area
  sentinels), and `-` for an absent `Option` — unambiguous because a present value is always quoted
  and so always begins with `"`.

  The codec gets two tests of its own, since the corpus drives only a narrow slice of the record's
  shape space (no fixture registers an image *and* a footnote *and* a warning at once, and
  `RefType::Section` never appears at all).
  [`the_record_codec_round_trips_every_shape`](../../parser/src/tests/inline_builder_side_effect_parity.rs)
  sweeps the empty record, one with every list populated and every `Option` in both states, and one
  holding the bytes a line-based format has to survive — a tab, a newline, a quote, a backslash, a
  literal `-` in a *present* field, and the sentinels.
  [`the_record_codec_rejects_a_corrupted_recording`](../../parser/src/tests/inline_builder_side_effect_parity.rs)
  covers the failure surface, which a hand-editable recording makes reachable rather than defensive:
  a count that over-reads its list, one that under-reads it and leaves fields behind (which the
  truncation guard cannot catch, hence the explicit exhaustion check), an unquoted field, an unknown
  `RefType` spelling, and a malformed location.

  The drift guard was checked by corrupting a recorded footnote text: two fixtures fail with "the
  string pipeline no longer produces the recorded rendering", which is the guard doing the job it
  will keep doing until the pipeline goes.

  **Nothing in production moved** — the whole change is under `src/tests/` plus the new recording —
  so the audit is a formality (**63 rows either side, 0 new and 0 closed**, the same corrected recipe
  the previous note records) and coverage is diff-neutral by construction rather than by measurement:
  the totals are byte-identical either side (85888 regions / 572 missed, 57210 lines / 323 missed).
  The codec lives under `src/tests/`, which the coverage report does not measure at all, so its two
  tests are what cover it rather than a number.

  *What still defers* is the survey's list with its first item struck: (1) the tree-shaped freeze —
  `InlineNode` serialization **and** a per-side normal form for `inline_builder_recorder_parity` and
  `inline_recorder`, whose comparison is a pairwise normalization rather than equality, and which is
  the harder half of the freeze by a wide margin; (2) the **authoritative-pass closure** — the
  `tree_seed == None` branch, the production blocker and the only remaining item that is not
  test-side; and only then (3) the deletion itself: `run_pipeline`, `apply_string_pipeline`, the
  escaping pass, the three sentinel systems (§4.2), the `suppress_recognition_side_effects` window,
  and `Parser::build_inline_tree`.

  *Step 6 landed as (the tree-shaped freeze — and one corpus that does not freeze):* the survey's
  next item, and the half it called "harder by a wide margin". It is, though not quite where the
  survey pointed: the item turns out to be **one** corpus rather than two, and finding that out is
  half the increment.

  *`inline_recorder` is not a freeze candidate at all.* The survey files it beside
  `inline_builder_recorder_parity` as tree-shaped, both needing an `InlineNode` serialization.
  Reading it says otherwise: its `oracle` runs the string pipeline **twice** — once under the HTML
  renderer for the golden, once under `RecordingRenderer` for the tree — and every one of its
  assertions (`folded == golden`, and `constructs <= markers <= events`) compares two products of
  that same pipeline. There is no builder side anywhere in the module. Freezing it would record both
  halves of `folded == golden` and then compare a recorded value against itself: vacuous, and
  green forever. Its subject is the Strategy-A recorder machinery in `content/inline_tree.rs`, which
  is already test-only, and which has exactly one other user — the corpus below. So it **retires with
  the pass** rather than freezing, and the freeze is one corpus, not two.

  *That is what makes freezing `inline_builder_recorder_parity` the whole of the item*, and it is
  where the difficulty the survey named actually lives. This corpus does have two independent sides,
  and the recorder's is the one that dies: `RecordingRenderer` recovers a tree out of what the string
  pipeline *renders*, so with the pipeline gone the module would compare `build` to itself. But the
  two sides are not equal and never were — `assert_trees_equivalent` is a **pairwise normalization**
  that ignores `location` and `attrs`, folds a builder leaf to rendered bytes to consume the
  recorder's, and will split a recorder `Text` run to meet a builder leaf's edge. A freeze needs a
  per-side normal form, which a pairwise diff is not.

  *The normal form is the one the recorder already satisfies*, which is why this cost far less than
  the survey feared. The module's own doc comment already enumerates, field by field, everything a
  recorder-built node cannot carry — `attrs`, `derived`, `xrefstyle`, `resolved`, an anchor's
  `reftext`, an image's `is_icon`, a ref's `link_form` — and `location` is the whole-content span on
  every one of them (design §4.4's migration stage). A recorder tree is therefore *already* in a
  restricted form. The recording carries exactly the fields the comparator reads, and decoding
  rebuilds real `InlineNode` values with every other field at the value the recorder always gives it.
  **The comparator is untouched, and not one assertion in the module moves.**

  *A partial normal form has exactly one hazard, and it is worth naming because nothing else in this
  branch has had it:* a field the comparator reads that the recording does not carry decodes as a
  default, and the comparison silently weakens with every test still green. Reasoning about that is
  not enough, so
  [`strip_unrecorded`](../../parser/src/tests/inline_builder_recorder_parity.rs) writes the dropped
  set down in one place and the harness asserts `strip(live) == decoded` on **every fixture**, by
  plain equality. That makes the check *total* rather than argued: a field added to `InlineNode`, or
  newly populated by the recorder, fails it until someone decides whether the recording should carry
  it. Both halves were sabotaged to confirm they bite — dropping `Styled::roles` or
  `Footnote::number` from the encoder fails the guard on two fixtures each, and corrupting a recorded
  `Styled` variant fails the drift guard with "the string pipeline no longer produces the recorded
  rendering".

  The format is the counted one the side-effect corpus established, with the count doing double duty:
  it is also what nests, since a parent writes its child count and then its children inline, so one
  flat field stream carries a tree. `Raw` is the one `InlineNode` kind the encoder refuses outright —
  it is a builder-side leaf, and the recorder recovers the same content as a mix of `Text` and
  `CharRef`, which is precisely the leaf-boundary difference `consume_rendered_prefix` exists to
  resolve. A `CharRef::Replacement` holds a `&'static str` and so cannot be rebuilt from a decoded
  `String`; the value comes from `RECORDER_ENTITY_TABLE`, which is the *right* source rather than a
  convenience — that table is exactly the set a recorder-built `CharRef` can hold, and
  `recorder_entity_table_matches_production_classify_entity` already guards it against drifting from
  the production `classify_entity`.

  The codec's two tests cover what the corpus does not drive: a bare `menu:` with no item, an
  XML-guarded callout, `StyleVariant::SingleQuote`, and a literal `-` in a *present* optional field
  (the one that would read back as `None` if the option encoding wrote values bare), plus the
  corrupted-recording surface a hand-editable file makes reachable — an over-reading count, an
  under-reading one that leaves fields behind, an unquoted field, each small enum's unknown spelling,
  a multi-character `CharRef::Special`, and a replacement the table cannot rebuild.

  **Nothing in production moved** — the whole change is under `src/tests/` plus the new recording — so
  the audit is a formality (**63 rows either side, 0 new and 0 closed**) and coverage is byte-identical
  either side (85945 regions / 572 missed, 57253 lines / 323 missed).

  *What still defers* is two items, not three: (1) the **authoritative-pass closure** — the
  `tree_seed == None` branch, the production blocker and the only remaining item that is not
  test-side; and then (2) the deletion itself, which this increment has now enlarged and simplified at
  once: `run_pipeline`, `apply_string_pipeline`, the escaping pass, the three sentinel systems (§4.2),
  the `suppress_recognition_side_effects` window, `Parser::build_inline_tree` — **and** the whole
  Strategy-A recorder (`content/inline_tree.rs`, `RecordingRenderer`) together with the
  `inline_recorder` corpus that tests it, which the finding above says goes with the pass rather than
  outliving it.

  *Step 6 landed as (the authoritative-pass closure):* the last item on the survey's list that is
  not test-side, and the production blocker the deletion has been waiting on. It is a **four-line
  change**, which is the surprise worth recording.

  The blocker, as the survey stated it: `run_pipeline` has two production callers in `apply_inner`.
  The first is the oracle, whose output the fold overwrites three statements later. The second is
  the `tree_seed == None` branch, which is *authoritative* — a passthrough body re-entering
  `SubstitutionGroup::apply` from inside a build takes no tree seed (the `in_inline_build` guard), so
  its string pipeline is that body's real pass. Nothing could answer for it "while a body folds to
  one `Raw` value rather than to nodes".

  *The obstacle was misidentified, and the increment is mostly finding that out.* The reasoning above
  descends from the deleted `build_pass_macro_subs_value` helper, whose doc comment argued that a
  body cannot be threaded through this module's transducers because
  [`build`](../../parser/src/content/inline_builder/mod.rs) runs a fixed *normal* order, so any
  structural node the body's own resolved subset produced would be visited again by whichever steps
  come after — and a macro's display text is not idempotent under a second pass. Every word of that
  is true, and none of it applies: **it is an argument about splicing the body's nodes into the
  enclosing level, not about computing the body's string.** Folding the body's own tree and wrapping
  the result in one `Raw` leaf keeps it exactly as opaque as `subs.apply` did.

  And the capability was already there. [`build_for_group`](../../parser/src/content/inline_builder/mod.rs)
  has taken a `SubstitutionGroup` since the cutover began, runs `group.steps()` **in the group's own
  order**, and already handles the orders no built-in group has — a `Custom` list that puts the
  escaping step *after* a step that produced markup, which is what `flatten_prior_markup` and
  `SplicedSpecials` exist for. So `passthrough_text` needed only to build and fold rather than
  substitute.

  *The measurement is the increment's evidence, and it is unambiguous.* Instrumenting the `None`
  branch across the whole suite: **112 hits before, 1 after.** The 111 that went were all
  `in_inline_build=true` — the passthrough-body re-entry, every one of them. The single hit that
  remains is `build_inline_tree=false`, the crate test that turns the flag off deliberately
  (`a_parse_that_builds_no_tree_keeps_the_string_pipelines_warnings`). Production no longer reaches
  the string pipeline at all.

  *What a body's rendering now costs, measured before the change:* `passthrough_text` is reached 378
  times in the suite — `Stem` 218, `Verbatim` 63, `Normal` 2, and 95 across **fifteen distinct
  `Custom` spellings**, among them `Custom([Quotes, SpecialCharacters])` and
  `Custom([AttributeReferences, Quotes])`. The out-of-order pairs are the ones that say this is not a
  fixed-order problem in disguise, and
  [`a_passthrough_body_renders_under_every_order_its_own_list_can_name`](../../parser/src/tests/inline_recorder.rs)
  pins both orders of both pairs — `pass:c,q[*x* < y]` renders `<strong>x</strong> &lt; y` where
  `pass:q,c[*x* < y]` renders `&lt;strong&gt;x&lt;/strong&gt; &lt; y`, so the fixture discriminates
  order rather than merely exercising it.

  *Three explanations elsewhere in the crate became false and were corrected rather than left to rot.*
  `Parser::in_inline_build`'s reentrancy guard no longer has a re-entry to guard: the property
  [`a_passthrough_body_is_substituted_once_per_apply`](../../parser/src/tests/inline_recorder.rs)
  measures still holds and is still worth pinning, but it holds *structurally* now rather than by the
  guard, which is why removing the guard's check no longer moves those counts.
  [`passthrough_body_warnings_survive_the_builds_own_discard`](../../parser/src/content/passthroughs.rs)
  passes unchanged, but by a different route — the body's `attribute-missing` diagnostic goes through
  the build's own `record_builder_diagnostic` and is carried across by the enclosing seam, instead of
  being raised into the discarded range and rescued by `Parser::nested_authoritative_warnings`. And
  the `Raw`/`source_text` notes that said "an arbitrary group needs the substitution pipeline" now
  say what is actually true: it needs a `Parser`, which a fold does not have, so the body is rendered
  at build time.

  Audit: **63 rows either side, 0 new and 0 closed** — a production change that moves no divergence
  at all, which is the strongest form the bar takes.

  Coverage is **not** diff-neutral, and the exception is the point, exactly as it was for the
  inversion. `passthrough_step.rs` stays at 22/10 (missed regions / missed lines) and `passthroughs.rs`
  at 3/3; `substitution_group.rs` goes from 0/0 to 6/6, and those six lines are precisely the
  `nested_authoritative_warnings` block inside the branch this increment just made unreachable.
  `Parser::nested_authoritative_warnings` and `in_inline_build`'s reentrancy half are **vestigial** as
  of here. Both are left standing on purpose — the same call the inversion made for
  `suppress_recognition_side_effects`, and for the same reason: they are two-ended mechanisms that go
  whole with `run_pipeline`, and removing one end while leaving the other would be worse than removing
  neither.

  *What still defers* is the deletion, and only the deletion: `run_pipeline`, `apply_string_pipeline`,
  the escaping pass, the three sentinel systems (§4.2), the `suppress_recognition_side_effects`
  window, `Parser::nested_authoritative_warnings`, `Parser::in_inline_build`,
  `Parser::build_inline_tree`, and the Strategy-A recorder (`content/inline_tree.rs`,
  `RecordingRenderer`) with the `inline_recorder` corpus that tests it. Every corpus that used to
  depend on the string pipeline for its golden is frozen; nothing in production calls it. The one
  test that still does — the `build_inline_tree=false` parse — is what the deletion has to decide
  about, and it is a decision about a flag rather than a blocker.

  *Step 6 landed as (the deletion, surveyed — and it is not one increment):* the closure left
  "the deletion, and only the deletion", which is true of the *blockers* and misleading about the
  work. Surveying the actual surface says the deletion has a substantial sub-problem inside it, and
  this increment measures it rather than discovering it mid-flight.

  *The easy half is easier than it looks.* `run_pipeline` has **four references in the whole crate**,
  all in `substitution_group.rs`: the oracle call, the `None` branch (test-only since the closure),
  the call inside the `#[cfg(test)]` `apply_string_pipeline`, and its own definition. There is no
  scattered call graph to unpick. `apply_string_pipeline`'s ~28 call sites are all `#[cfg(test)]`
  golden helpers whose corpora are now frozen, so each is a mechanical drop of an argument.

  *The hard half is one function, and it is not the rendered string.*
  [`Content::set_tree_xrefs`](../../parser/src/content/content.rs) reads the string pipeline's own
  **`deferred` state** — not its bytes — and keeps two things from it: the placeholder `template`,
  which only that pipeline produces, and a carve-out where the tree holds fewer cross-references than
  the pipeline deferred, in which case the pipeline's whole answer stands. Deleting the oracle
  without answering both would silently break deferred cross-reference resolution.

  *Measured across the suite*, `set_tree_xrefs` resolves three ways:

  | outcome | hits | share |
  |---|---|---|
  | nothing deferred — the oracle's answer is irrelevant | 12,852 | 96.7% |
  | tree segments installed, `template` still the pipeline's | 436 | 3.3% |
  | **carve-out** — the pipeline's whole answer stands | 6 | 0.05% |

  So the template looks like the larger debt by volume. (The 436 here is a count of `set_tree_xrefs`
  calls that installed a tree answer, not of contents rendering from a template. A later note
  measures `render_template` itself and finds the debt is *smaller* than this suggests, not larger.)
  The carve-out is six hits over five
  sources — and reading what each one *is* matters more than the count, because the five are **two
  different kinds**, and only one of them is a gap.

  - **A closable gap.** `indexterm2:[<<b>>]` — a cross-reference inside an index term's *macro*
    spelling. The visible shorthands carry their shown text as `children`, so a reference inside them
    is a node the collectors walk; the macro spelling's term comes back from an attribute-list parse
    instead, so the builder keeps it as a string and there is no node to derive a segment from. It is
    the index-term family's own documented limitation, and its own test said in as many words that the
    day the builder learns the macro spelling, that test fails and the fixture moves into the parity
    corpus. This one closes like any other prep — and did, in the increment that follows this survey,
    exactly that way.

  - **A deliberate deferral, and not a gap at all.** The other four sources are the
    `xref:sec[a *b, c* d,role=hl]` shape (with a trailing period, and paired with a `<<a␄b>>`
    shorthand). The tree does not fail to recognize this — it **declines** to.
    [`an_attribute_list_delimiter_inside_a_span_defers_the_match`](../../parser/src/content/inline_builder/macros/xref.rs)
    is the rule: the string replacer splits the attribute list over the piece's own *markup*, and
    `a *b, c* d` renders `a <strong>b, c</strong> d`, whose list splits at the comma **inside the
    tag**, ending the anchor there. Where the two readings disagree about the match's own extent the
    tree defers rather than claiming a construct the rendered document does not agree with.

  *That second kind is the finding, and it changes what the deletion has to decide.* The carve-out is
  not a bug to be closed out from under `set_tree_xrefs`; for these four it is the mechanism by which
  the string pipeline's answer — the one the tree deliberately refuses to reproduce — reaches the
  output. **When `run_pipeline` goes there is no such answer left to fall back to.** So the deletion
  has to make a *behavioral* choice for this shape: either the tree learns to reproduce a split that
  lands inside a tag (the reading it rejects on purpose), or the rendered output for an attribute
  list whose delimiter hides inside a span changes.

  **Decided (maintainer, at the survey): *diverge*.** Emitting the replacer's `<strong>` split is
  conceptually the wrong answer, so the tree's own reading stands and this branch accepts a
  documented divergence from both the string pipeline and Asciidoctor here.

  The two readings are worth stating precisely, because the divergence is not a near-miss — one of
  them is simply malformed. The tree tokenises an already-built `Styled` node to an opaque
  [`SPAN_PLACEHOLDER`](../../parser/src/content/inline_builder/quotes.rs) carrying none of the `,`
  `=` `"` bytes a bracket split reads, so `xref:sec[a *b, c* d,role=hl]` splits into a display text
  of `a ␖ d` and a `role=hl` — the reading the author obviously meant. The replacer splits over the
  *rendered* markup `a <strong>b, c</strong> d,role=hl`, so the comma **inside**
  `<strong>b, c</strong>` becomes a delimiter and the anchor's text ends at `a <strong>b`, with the
  tag left unbalanced. Asciidoctor produces the same unbalanced output, which is why this is a
  divergence from it too.

  So the work this turns into is *removing* a deferral rather than adding a recognition:
  [`tokened_split_agrees`](../../parser/src/content/inline_builder/macros/mod.rs) exists to make the
  tree decline where the two parses disagree, and this class is exactly where declining is now the
  wrong answer.

  *The scope question that raises answers itself empirically.* Logging every decline
  `tokened_split_agrees` makes across the whole suite finds **two distinct cases**, and they are the
  same shape twice: `a ␖ d,role=hl` against `a <strong>b, c</strong> d,role=hl`, and the identical
  pair with `` `b, c` `` in place of `*b, c*` — two attributes on the tokened side against three on
  the restored one, every time. So "narrow the decline to the unbalanced-markup class" and "remove
  the decline" are the same change against this suite; there is no second sub-class to preserve it
  for.

  That is evidence about the corpus rather than about the language, so the *principle* is what should
  be written down when it lands: a bracket split must not read bytes that a markup-producing step
  introduced. The tokened side is exactly the side that holds to it, which is why it is the reading
  the divergence keeps. The frozen corpora record the choice rather than adjudicate it, so each is
  re-recorded to the tree's answer as the change lands.

  *So the decomposition, in dependency order:* (1) the `indexterm2:` gap, the one closable member of
  the carve-out; (2) **the deferral divergence above**, now decided and so ordinary work — narrowing
  or lifting `tokened_split_agrees`'s decline for this class, with its scope settled against the
  audit's own set; (3) the **template** —
  can the tree produce its own placeholder template, or does `refold` make one unnecessary for a
  tree-backed content?; (4) the oracle call itself, a two-line deletion once (1)–(3) hold; (5) the
  ~28 `apply_string_pipeline` test call sites and `run_pipeline`'s own removal, mechanical; (6) the
  three sentinel systems (§4.2), the `suppress_recognition_side_effects` window,
  `nested_authoritative_warnings`, `in_inline_build` and `Parser::build_inline_tree`, all vestigial
  and all going together; (7) the Strategy-A recorder (`content/inline_tree.rs`,
  `RecordingRenderer`) and the `inline_recorder` corpus that tests it, which the tree-shaped freeze
  established retire with the pass rather than outliving it.

  Nothing in production moved in this increment — it is a measurement and a decomposition — so there
  is no audit or coverage claim to make beyond the suite staying green.

  *Step 6 landed as (the deferral divergence, taken):* the survey's item (2), decided by the
  maintainer and now implemented. `tokened_split_agrees` is gone, with `restored_markup_text` (its
  only helper) and the image family's `bracket_is_recognizable` (which became always-`true`), and all
  three call sites — xref, links, image — with them.

  *What the gate actually cost is clearer from its removal than from its presence.* It deferred a
  match whenever the tokened split and the replacer's markup split disagreed, which meant **a comma
  inside a span decided whether the macro was recognized at all**. Before this increment
  `xref:sec[a *b, c* d,role=hl]` folded to the literal text `xref:sec[a <strong>b, c</strong>
  d,role=hl]`; the identical fixture without the comma, `xref:sec[a *b* d,role=hl]`, folded to a
  proper anchor. The gate was not choosing between two readings of a construct — it was refusing the
  construct.

  Now all three families read it the way they already read the comma-free case:

  | source | before | after |
  |---|---|---|
  | `xref:sec[a *b, c* d,role=hl]` | literal text | `<a href="#sec" class="hl">a <strong>b, c</strong> d</a>` |
  | `link:index.html[a *b, c* d,role=hl]` | literal text | `<a href="index.html" class="hl">a <strong>b, c</strong> d</a>` |
  | `image:x.png[a *b, c* d,title=hl]` | literal text | `alt="a <strong>b, c</strong> d" title="hl"` |

  The image row is the one worth checking rather than assuming, and it was: the baseline already
  renders `alt="a <strong>b</strong> d"` for the comma-free `image:x.png[a *b* d,title=hl]`, so
  markup reaching an `alt` is this family's **existing** behavior and the change only stops the comma
  from suppressing recognition. Nothing new leaks into an attribute.

  *The audit reads differently here than on any previous increment, and the difference is the
  increment.* **63 rows either side, 61 distinct sources either side, and the two source sets are
  identical** — no divergent source appeared or disappeared. What moved is the *tree's* column on
  exactly four rows: the `(source, rendered)` pairs are byte-identical either side (a set comparison
  over those two columns differs in nothing at all), while `folded` goes from the literal text to the
  anchor. The divergence against the string pipeline **persists** on those four, because the pipeline
  still writes the cut-short `a <strong>b`; what changed is that the tree's answer went from *absent*
  to *right*. Reporting this as "four rows closed" would have been wrong, and it is worth saying so:
  the bar this branch uses ("no new row") is a proxy for "no unintended behavior change", and an
  intended one shows up as a row whose fold moved rather than as a row that left.

  Five tests asserted the deferral and now assert the reading; each keeps its own subject.
  `an_attribute_list_delimiter_inside_a_span_is_the_trees_to_read` and
  `a_span_whose_markup_splits_the_attribute_list_is_the_trees_to_read` pin the fold and, with
  `assert_ne!` against the golden, pin the divergence itself as bytes.
  `the_xref_mirror_correlates_the_form_that_used_to_defer` is the carve-out closing from resolution's
  side — the counts agree, the positional mirror correlates instead of skipping, and the content
  leaves the template path — with `a_title_resolves_the_form_that_used_to_defer_on_its_own_path`
  saying the same for `title_refs::compute`'s separate path.
  `a_resolved_destination_holding_a_sentinel_survives_the_template_path` keeps its sentinel subject
  and simply carries the new bytes.

  Coverage is neutral in total (578 missed regions / 329 missed lines either side) and improves where
  the dead code went: `image.rs` drops from 5/2 to 4/1. `xref.rs` goes from 16/7 to 17/8, and the one
  added line is the `let … else { panic!(…) }` in the rewritten test — the test-assertion fallback
  this repository's conventions already exclude.

  *What still defers* is the survey's list with items (1) and (2) struck: the `indexterm2:` gap, the
  **template**, then the oracle call, the test call sites and `run_pipeline`, the vestigial
  mechanisms, and the Strategy-A recorder with `inline_recorder`.

  *Step 6 landed as (the `indexterm2:` gap — and the carve-out emptied):* the survey's item (1), the
  last member of the `set_tree_xrefs` carve-out, and the increment that makes the carve-out itself
  unreachable.

  The gap was real and the family's own note had named it: an `indexterm2:[…]` shown term came back
  from an attribute-list parse rather than from a range of the match string, so the builder kept it
  as a **string** and built no subtree. A `<<b>>` inside it was never recognized at all — the term
  computed to the escaped literal `&lt;&lt;b&gt;&gt;` — where the visible shorthand `((<<b>>))`
  carries its shown text as `children` that the later macro families descend into.

  *What made it closable is that the narrowing has a byte.* [`shown_macro_term`] already decides the
  two cases by one test: an argument holding no `=` is not an attribute list, so it returns the
  argument unchanged — the term *is* the whole shown range. Only with an `=` does it parse a list and
  take the first positional attribute, `Coffee` out of `Coffee, region=Kona`, which is the value that
  is not a range of the match string. The `None` arm beside it already tested the same byte. So the
  fix is to carry `shown.children` exactly when the term was not narrowed by a list, and the family's
  documented limitation narrows from "the macro spelling" to "the macro spelling *with an attribute
  list*".

  That distinction is not theoretical: passing the children through unconditionally was tried first
  and `indexterm2:[Coffee, region=Kona]` folded to `Coffee, region=Kona` where the pipeline shows
  `Coffee`. The original note was right about why, and the guard is what separates the halves.

  *The carve-out is now empty.* Instrumenting `Content::set_tree_xrefs`'s carve-out branch across the
  whole suite: it fired six times when the survey measured it, once after the deferral divergence
  closed the four `xref:` rows, and **zero times now**. Both of its members are gone, so the branch
  where "the tree holds fewer cross-references than the pipeline deferred, and the pipeline's whole
  answer stands" is unreachable. Coverage does not flag it — `content.rs` sits at 4 missed regions and
  0 missed lines either side — so this is recorded here rather than left for the deletion to
  rediscover: the carve-out can go with `run_pipeline`, and the **template** is now the only thing
  `set_tree_xrefs` still takes from the string pipeline's `deferred` state.

  Audit: **62 rows against the baseline's 63, 0 new and 1 closed**, and the closed row is
  `See indexterm2:[<<b>>] here.` — the tree now agreeing with the pipeline. The contrast with the
  deferral divergence one increment earlier is the whole distinction between the two kinds of
  carve-out member: there the pipeline was wrong and the divergence persisted while the tree's answer
  went from absent to right; here the pipeline was right and the tree caught up, so the row simply
  leaves. Coverage is diff-neutral (578 missed regions / 329 missed lines either side).

  Three tests moved, each the one its own comment said would.
  `a_reference_inside_an_index_term_macro_keeps_its_documented_divergence` failed with its own
  message — "the macro spelling now yields a subtree; fold this into the parity corpus" — and its
  fixture, plus a straddling one that pins the alignment, is now in
  [`CORPUS`](../../parser/src/tests/inline_builder_xref_segment_parity.rs).
  `a_later_family_inside_a_macro_spelling_term_is_a_documented_divergence` becomes
  `…_is_parity_without_a_list`, with `a_macro_spelling_term_narrowed_by_a_list_keeps_its_documented_divergence`
  holding the boundary the lift stops at. And
  `an_index_term_macro_hiding_a_reference_keeps_the_string_pipelines_rendering` becomes
  `…_folds_through_the_tree` — worth noting because it kept *passing* after the change while asserting
  the carve-out was in effect: `collect_refs` does not walk an `IndexTerm`'s shown text, so its
  "the tree holds no cross-reference node" check had gone vacuous. It now reads the term's own
  `children`, which is where the node actually lives.

  *What still defers:* the **template**, then the oracle call, the ~28 `apply_string_pipeline` test
  call sites and `run_pipeline` itself, the vestigial mechanisms (now including the carve-out), and
  the Strategy-A recorder with `inline_recorder`.

  *Step 6 landed as (the template, measured — it is two named cases, not a majority path):* a
  scoping note rather than a change, and it exists because the deletion survey's own number for this
  item was measuring the wrong population.

  That survey called the template "the larger debt by volume (436 contents)". **436 is a count of
  calls to `set_tree_xrefs` that installed a tree answer, not a count of contents that render from a
  template** — different populations. The first attempt to correct it made the same class of mistake
  one step further in: it instrumented the *arm choice* in `Content::resolve_references` — `refold`,
  which folds the tree, against `rebuild_rendered`, which renders a template — and read 330 calls
  over 210 distinct sources for the fold against 5,944 over 2,437 for the template. That reads as a
  template-majority path. It is not one. **`rebuild_rendered` opens with an early return on
  `self.deferred.is_none()`, and every one of those 5,944 calls took it.** They are no-ops on
  contents that never deferred anything. Counting an arm is not counting a render.

  Instrumenting `render_template` itself — the single function every template render funnels through
  — gives the population the item is actually about:

  | production call site | calls | distinct templates |
  |---|---|---|
  | `finalize_deferred` → `rebuild_rendered` | 794 | 170 |
  | `Footnote::render` | 225 | 24 |
  | `title_refs`'s fold fallback | **1** | **1** |
  | `resolve_references` → `rebuild_rendered` | **0** | — |

  So the two headline claims inverted together. Not eight percent of deferring contents fold their
  tree — **all of them do**: the 330 fold calls over 210 distinct sources are exactly the contents
  carrying deferred state, and no content with deferred state reaches `resolve_references`'s template
  arm at all. And the template is not the majority path; from the resolution pass it renders **zero
  times**.

  *What is left is therefore two named cases and one line of bookkeeping,* which is a much smaller
  and much more specific item than either the survey or its first correction implied:

  - **`finalize_deferred`'s render (794 calls, 170 templates) is not separate scope.** It runs at the
    end of `run_pipeline`, rebuilding `rendered` as the *unresolved fallback* so the string is clean
    for a caller that reads it before resolution. It is the string pipeline's own answer to its own
    question, and it goes when `run_pipeline` goes — an increment already enumerated in the
    decomposition, not a new one.
  - **`Footnote::render` (225 calls, 24 distinct templates) is the one genuinely separate consumer.**
    A footnote holds its own template and its own segment list and re-renders from them, on a path
    `refold` never touches. This is the real remaining template work, and it is footnote-shaped
    rather than content-shaped.
  - **`title_refs`'s fallback fires exactly once in the whole suite.** One call, one template,
    `"See \u{e000}0\u{e001}"` — and its shape is precisely what the code comment beside it predicts:
    `from_tree` true and `render_attributes` present, but `inlines` **empty**. It is the carried block
    title whose inline nodes cannot cross the `'src`-erasing hop across a section heading, so there is
    no tree to fold and the template is the only answer available. One source is a boundary, not a
    gap.

  *What this does to the decomposition.* The item keeps its position — still gated behind nothing —
  but it shrinks and splits. The question is not "can the tree produce a placeholder template", and
  not "why does the tree's answer reach only one content in twelve" (it reaches all of them); it is
  **"can a footnote fold its own tree, and can a carried block title keep one across the `'src` hop?"**
  Two concrete questions with two concrete subjects, where there was one vague volume estimate.

  *And the methodological point, which cost two rounds to learn.* Both wrong numbers were proxies
  measured one seam away from the thing they were supposed to describe — first a `set_tree_xrefs`
  call count standing in for a render, then an arm choice standing in for a render, when the arm's
  `else` branch is overwhelmingly a no-op. The carve-out taught the same lesson at a smaller scale:
  reading what each case *is* beats counting the cases. Here it took instrumenting the leaf function
  and nothing above it.

  *Step 6 landed as (`build_inline_tree` retired, and the seam's two "otherwise" causes told
  apart):* the first of the vestigial mechanisms the deletion survey enumerated, removed on its own.

  `Parser::build_inline_tree` is what remained of the `with_inline_tree` opt-in after that switch was
  retired. It was `true` for every parser a caller can construct, and its only surviving consumer was
  a crate test that set it to `false` in order to exercise the field. Measured across the suite it is
  false for **1 of 13,299** parses reaching the seam — that one test.

  So the field is gone, and `SubstitutionGroup::apply`'s seed condition drops from
  `parser.build_inline_tree && !parser.in_inline_build.get()` to the reentrancy guard alone. No
  production parse changes: the conjunct removed was true in every one of the other 13,298.

  *What the increment actually turned up*, and the reason it is worth a note rather than a line in a
  commit message: **the seam's two "otherwise" causes were never equivalent, and the retired one was
  carrying the test.** The `None` branch is taken when no tree is built, which had two causes — a
  parse configured to build none, and a pass re-entered under the reentrancy guard — and the obvious
  move was to re-point the test from the first cause at the second. That fails, and the failure is
  the interesting part:

  - A **tree-less parse** must *keep* the string pipeline's warnings where they are raised. Nothing
    encloses it, so there is nobody to hand them back.
  - A pass under the **reentrancy guard** must *stash* them in `nested_authoritative_warnings`, for
    the enclosing build to restore.

  The `None` branch does the second (`if parser.in_inline_build.get()`), so a test that sets the
  guard with no build around it sees the warnings stashed and dropped. That is not a bug — it is a
  state production cannot construct — but it means the warning half of the old test belonged to the
  cause being retired, and asserting it from the surviving cause would have pinned the behavior of an
  impossible parser. The replacement asserts only what the guard actually contracts: **a pass under it
  builds no tree.**

  *What this does not do.* It leaves `in_inline_build` and `nested_authoritative_warnings` in place.
  They are the two ends of one mechanism — the guard stashes, the enclosing build restores — and the
  seam's own comment already says removing one end while leaving the other is worse than removing
  neither. `in_inline_build` is true for **0 of 13,299** parses at this seam, so the pair is dead in
  practice and is the next increment; but it is dead by *measurement*, and the guard is what stands
  between a re-entry and a nested tree build, so retiring it is a deliberate step rather than a
  side effect of this one.

  *Step 6 landed as (the seam has one path — the reentrancy guard and its warning transport
  retired):* the second vestigial mechanism, and the one that takes a whole branch with it.

  `Parser::in_inline_build` was the guard that kept a tree build from recursing into one: a
  passthrough carrying its own substitution list re-entered `SubstitutionGroup::apply` for its body,
  and that nested call had to take no tree seed. The authoritative-pass closure removed the re-entry
  rather than the double pass — `passthrough_text` builds and folds the body's own tree through
  `build_for_group` directly — so the guard has had nothing to guard against since.

  Two independent readings agree, which is what made this safe to remove rather than merely plausible:

  - **Structural.** No production code under `content::inline_builder` calls
    `SubstitutionGroup::apply` at all. Every such call in that module is inside a `mod tests`.
  - **Measured.** The guard was observed set for **0 of the 13,299** parses the suite reaches this
    seam with.

  So `tree_seed` is now unconditional (`let tree_seed = content.rendered.clone()`), and with the
  guard gone the `None` arm of the match it fed is unreachable and goes too — **taking the
  authoritative `run_pipeline` call site with it.** `run_pipeline` survives with exactly one caller,
  the oracle, which is the separate increment.

  *The warning transport goes in the same motion, both ends together.* `nested_authoritative_warnings`
  existed because a nested authoritative pass raised real warnings inside the range a build discards
  as incidental, so they had to be moved aside and handed back: the stash lived in the `None` arm,
  the restore after the build. The seam's own comment insisted these two go whole rather than one at
  a time, and they do — the branch that fed the stash is the branch being deleted, so the restore has
  nothing left to restore. What remains after the build is now an unqualified truncate, with no
  exception to explain.

  *What checks it.* `a_passthrough_body_is_substituted_once_per_apply` is the test this increment
  rests on, and it was written for exactly this. A stateless renderer cannot see a doubled pass — the
  extra pass produced the same bytes — so it drives an `OrdinalRenderer` whose output depends on how
  many times it is called. Under the old arrangement, dropping the guard moved each of its counts
  from three to four and shifted the ordinal reaching the output (`a [3] b` became `a [4] b`). The
  guard is dropped here and **the counts do not move**, which is the property standing in for the
  guard now that the guard is gone. Anyone tempted to change the seed condition again should read
  that test first.

  The crate test that pinned the guard directly (added one increment earlier, when the retirement of
  `build_inline_tree` left the guard as the branch's only remaining cause) goes with it: its subject
  no longer exists.

  Fold-parity audit: 62 distinct divergences on this branch and on the base alike — zero new, zero
  closed. Coverage improves again rather than staying neutral: `content/substitution_group.rs` reaches
  **100% of regions and 100% of lines**, its last missed region being the arm just deleted.

  *Step 6 landed as (a footnote folds its own subtree):* the first of the two template consumers
  the survey named, closed — and closed without the lifetime work the survey assumed it needed.

  The survey framed this as blocked on one fact: a footnote's catalog entry outlives the parse
  borrow, `InlineNode` carries a `Span<'src>`, so the entry cannot hold a tree and holds a
  placeholder template instead. Closing it looked like a choice between threading `'src` through
  `Footnote`/`Catalog`/`Document` and building an owned inline-node representation. **Neither is
  needed.** The entry does not have to *hold* a tree — the tree is already somewhere else, alive, at
  exactly the moment the entry needs re-rendering. The defining
  [`Footnote`](../../parser/src/inlines/footnote.rs) node in the enclosing content's own tree carries
  the footnote's children, and `Document::resolve_references` holds the blocks and the catalog
  together in one `with_dependent_mut` closure. So the fold happens where the tree is and only a
  `String` travels to the catalog. Nothing gains a lifetime.

  Threading `'src` would in fact have been the *worse* of the two: the catalog is assembled on the
  `Parser`, which is deliberately lifetime-free and reused across documents, so `Footnote<'src>`
  forces `Parser<'src>` and breaks that reuse. The public-API cost the survey worried about was the
  smaller half of that option.

  *Where the fold is collected, and why not in a walker of its own.* Each content folds its own
  defining footnotes during `Content::resolve_references` and pushes `(index, text)` onto
  `ReferenceWarnings` — an accumulator already threaded through **exactly** the traversal that
  reaches every content. That matters more than it looks. A parallel walker has to rediscover two
  sub-parses the generic `child_blocks_mut()` walk misses, a Markdown-style blockquote's blocks and
  an AsciiDoc table cell's, and the scoping measurement for this step was itself written with a
  walker that missed the first: it reported exactly one unreachable footnote across the suite, which
  is small enough to look like a boundary and be mistaken for one. Riding the existing traversal
  makes that class of miss unrepresentable.

  *Crossing an owned sub-parse is deliberately not automatic.* `rehome_into` carries warnings out
  and leaves the folded texts behind, because the right answer differs by sub-parse: a Markdown
  blockquote's blocks register their footnotes on the **document's** catalog, so its site carries
  them out; an AsciiDoc table cell keeps a footnote list of its own, so its site installs them there
  and carries nothing. Getting this backwards is not a lost fold but a **wrong** one — footnote
  indices restart per catalog, so a cell's footnote `1` would overwrite the document's footnote `1`.

  *One retention change was needed.* A content whose only cross-references sit *inside* a footnote
  defers nothing itself — the replacer captures those onto the footnote's own state — so it did not
  retain the render attributes a later fold needs. `SubstitutionGroup::apply` now retains them for a
  content that defines a footnote as well as for one that defers a cross-reference. Both are the same
  rule: a content whose rendering is rebuilt after the parse needs the attributes it was written
  under.

  *What still uses the template, precisely.* A footnote defined in a **section heading**. Its content
  is resolved by the document-order title pass rather than by `Content::resolve_references`, so no
  fold is collected for it and `Footnote::resolve_references` falls back to its placeholder template.
  Measured across the suite: **49 of 55** resolved footnotes fold; the 6 that do not are that case.
  So the fallback arm is not speculative — it has a real, named user, which is a better position than
  the survey's "no such footnote is known".

  *What checks it.* Nothing in the differential corpora can: the fold and the template produce the
  same bytes, which is the point of the step and why all 5,529 existing tests stayed green the moment
  the fold was wired in. `a_footnotes_rendering_is_refolded_from_its_subtree_at_resolution` is the
  test that can, and it uses the same instrument as the reentrancy step before it — a renderer with
  *state*. `OrdinalRenderer` stamps an increasing ordinal into every special character, so a `<`
  inside a footnote carries one ordinal if it was rendered once at parse time and spliced ever after,
  and a higher one if the subtree was folded again at resolution. It caught a real gap on its first
  run: the retention change above exists because that test failed without it, while every other test
  passed.

  Fold-parity audit: 62 distinct divergences on the base, 63 on this branch — the one new row is this
  step's own new fixture, whose source does not exist in the base corpus; no pre-existing source
  changed. Coverage diff-neutral (`content/content.rs` 30 missed regions and 20 missed lines, both
  unchanged).

  *Step 6 landed as (the title pass folds its footnotes too — the footnote template is now unused):*
  the small remainder of the increment before it, and the one that takes the count to all of them.

  The footnote fold left 6 of 55 resolved footnotes on the placeholder template: those defined in a
  **section heading**. A heading is not resolved by `Content::resolve_references` at all — the
  document-order title pass (`document::title_refs`) owns it, because a cross-reference *between* two
  titles needs coordination the per-content pass cannot do — so nothing collected a fold for it.

  *Where the fold goes, and why not where the title's own fold happens.* The obvious site is
  `compute`, beside `fold_resolved_title`. It is the wrong one. That fold runs on a **clone** of the
  tree carrying only `block_ordered`, because the pass holds no `&mut` to the blocks while it is still
  computing — and a footnote's rendering needs `footnote_ordered`, which reaches the real tree only in
  `write_back`. So the collection happens in `write_back`, immediately after each
  `mirror_*_tree_xrefs` call has installed the destinations.

  That also answers the walker question the same way the increment before it did: `write_back`
  already visits exactly the headings and block titles that matter, in document order, with the
  access needed. Extending it beats a third walk beside `collect` and `write_back` that could drift
  from both. It costs three parameters on a private function with one caller.

  Both passes now ask for folds through one method,
  [`Content::collect_own_folded_footnotes`](../../parser/src/content/content.rs), which folds under
  the content's own retained attributes or does nothing if it has none. Neither pass re-derives
  "which attributes, and is there anything to fold". `SectionBlock::section_title_content` loses its
  `#[cfg(test)]` gate to serve it — the accessor's doc comment already described this caller
  hypothetically.

  *Measured: 55 of 55 folds, 0 templates.* Every footnote the crate resolves now renders from its own
  subtree. `FootnoteDeferred::render` keeps exactly one caller, `Parser::define_footnote`'s
  parse-time unresolved fallback, which belongs to the string pipeline and goes with the oracle.

  *The fallback arm stays, and is now unit-tested rather than deleted.* It is unreachable by
  **measurement**, not by construction: a footnote whose defining content retained no render
  attributes would still land there, and rendering its template is the honest answer — better than
  leaving the parse-time text standing as though resolution had never run. This branch keeps
  confusing the two, so the arm is pinned directly by
  `a_footnote_folds_when_given_one_and_renders_its_template_otherwise`, which drives both arms with an
  empty cross-reference list so the choice itself is the only subject.

  Fold-parity audit: 63 distinct divergences on the base and on this branch alike — zero new, zero
  closed. Coverage diff-neutral (`document/catalog.rs` 2 missed regions and 0 missed lines, both
  unchanged; workspace total unchanged at 605 / 348).

  *Step 6 landed as (the carried title's template is the tree's own — and the suppression window
  measured dead):* the template item's last production consumer switched producers, which is the
  increment that genuinely unblocks the oracle call's deletion: nothing on the production path
  reads the string pipeline's placeholder template any more.

  [`Content::to_owned_title`](../../parser/src/content/content.rs) now takes the parser and — for
  a title that defers a cross-reference and still holds its tree — synthesizes the placeholder
  template and the segment list its indices point into from that tree, at the stash site
  (`SectionBlock::parse`), while the nodes are still alive. The `'src`-erasing hop across
  `Parser::pending_block_title` is unchanged, and so is the render path: the snapshot arrives at
  the claiming block node-less and renders by splicing, exactly as before, but the splice's inputs
  are the tree's.

  *Why the synthesis is a gap walk and not one `fold_deferring_xrefs` call.* The footnote template
  is one such call, and inheriting it here was the obvious move — and the wrong one, for a reason
  worth keeping: in that fold's output a document-typed `U+E000 0 U+E001` (the issue #1235 class
  the `sentinels` suite pins) is **byte-identical** to a real placeholder, and nothing downstream
  can tell them apart. The footnote template carries that ambiguity today and gets away with it
  because resolution renders a footnote from its subtree and the template is a parse-time fallback;
  a carried title's template is *rendered*, so it cannot. The string pipeline never had the problem
  because it escaped the whole haystack before any step ran — so the synthesis reproduces exactly
  that form: [`carried_title_template`](../../parser/src/content/content.rs) walks the tree's
  top-level nodes, a cross-reference node contributing a raw placeholder plus its segment (through
  `xref_segment_from_node`, the derivation every other segment shares) and every other node
  contributing its own fold passed through `escape_sentinels`. Escaped bytes are the document's,
  raw sentinels are the fold's own, and the template stays in the escaped form the render path
  (`EscapedForm { template: true, … }`) already handles — no new case anywhere downstream.
  Verified against the base byte-for-byte on the typed-sentinel and two-reference probes, and
  pinned by `a_typed_placeholder_in_a_carried_title_cannot_forge_a_cross_reference`.

  *The boundary the top-level walk draws*, pinned by
  `a_reference_nested_in_a_span_of_a_carried_title_stays_its_fallback`: a cross-reference nested
  inside another top-level construct (a styled span, a visible index term) folds with its
  enclosing node as template text — baked as its unresolved fallback, neither resolved nor
  reported — where the pipeline's template spliced it, its placeholder sitting inside the rendered
  markup. Measured at zero: no golden source carries a nested cross-reference in a carried block
  title (the suite's one carried deferring title is `.See <<goal>>`, whose reference is
  top-level). Nothing survives the hop to do better; if the synthesis ever learns nested
  placeholders, the test's own comment says where it moves.

  *The condition's other exits are real, and one grew its own test.* A title re-stashed past an
  empty section body reaches `to_owned_title` a second time, node-less, and must keep the template
  the first hop synthesized rather than synthesize from the empty tree
  (`a_title_restashed_past_an_empty_section_keeps_its_template`). The cost is one extra fold per
  carried *deferring* title — population across the suite: one — on the transitional
  double-render this branch already pays everywhere until the oracle goes.

  *And the measurement the deletion survey asked for:* `Parser::suppress_recognition_side_effects`
  documents itself as "set by `SubstitutionGroup::apply` around its authoritative string pass" and
  slated to die with the oracle. By measurement it is dead **now**: the field is initialized
  `false`, read at ten sites, and set nowhere — the inversion removed its only setter when the
  string pass moved onto the discarded clone, and every remaining `.get()` guard is a branch that
  cannot take. Its retirement is a pure deletion, independent of the call's, and stays with the
  vestigial-mechanisms item where the survey filed it.

  *What still defers*, more precisely than the survey could say: the oracle call itself — now a
  deletion with no template question inside it — carrying the `set_tree_xrefs` rewrite (derive
  the deferred state from the tree, which deletes the carve-out branch and `template_splices`
  with it), the template-emptiness debug asserts, and the retirement of
  `inline_builder_xref_segment_parity` and the document-parity harness's template comparison,
  each of which already documents that it goes when `run_pipeline` does; then the
  `apply_string_pipeline` call sites and `run_pipeline` itself; the vestigial mechanisms
  (the suppression window above, the `from_tree: false` machinery, the sentinel escape/unescape
  pair); and the Strategy-A recorder with `inline_recorder`.

  Fold-parity audit: 63 distinct divergences on the base and on this branch alike — zero new,
  zero closed (the seam this increment touches is behind the audit's comparison, not in it).
  Coverage diff-neutral: `content/content.rs` 30 missed regions and 20 missed lines,
  `blocks/section.rs` 40 and 39, workspace totals 605 and 348, all matching the base.

  *Step 6 landed as (the oracle call deleted — the seam is single-pass):* the deletion the
  survey enumerated as item (4), landed with the tail the carried-title increment left it.
  `SubstitutionGroup::apply` no longer runs the string pipeline at all: the build is seeded
  straight from the content's value, and `run_pipeline` survives only as the test oracle behind
  `apply_string_pipeline` — where every differential corpus already takes its golden.

  *The seam's other half is `set_tree_xrefs`, now the sole producer of deferred state.* It
  derives both segment lists from the tree it is handed, keyed by a short-circuiting boolean walk
  (`tree_defers_xrefs`) so the overwhelmingly common cross-reference-free content answers in one
  cheap pass — the same structural economy the old early return bought by asking the string
  pipeline. The **carve-out is deleted** with the pipeline it fell back to (`template_splices`
  with it), on the measurement the `indexterm2:` increment recorded: zero hits across the suite,
  both member kinds closed. A production `DeferredContent` now carries no template — the two
  templates production still renders are the footnote entry's and the carried title's, each
  synthesized from its own tree — and no `string_xrefs` snapshot: that field, and the test-only
  `rendered_from_template`, went with the harnesses that read them.

  *Two harnesses retired, each exactly as its own documentation promised.*
  `inline_builder_xref_segment_parity` compared the tree's segment derivation against the
  pipeline's own flat list ("the golden goes when `run_pipeline` does, and the corpus with it");
  the document-parity harness's `the_fold_reproduces_the_template_for_every_deferred_content`
  compared the fold against the pipeline's template render. With the pipeline off the production
  path neither has a golden that is not the tree itself. What holds the byte-for-byte line is the
  golden-HTML assertion suite, which pinned identical bytes on either side of this increment —
  the deletion changed **no rendered output anywhere in the suite**.

  *The stateful-renderer pins moved exactly as their own comments predicted*, which is the
  increment's best evidence of what it removed: `inline_tree_build_tolerates_a_stateful_renderer`
  went from `a [second] b` to `a [first] b` ("it becomes `[first]` again once the string pipeline
  stops being run for output"), and `a_passthrough_body_is_substituted_once_per_apply`'s counts
  dropped from three to one. That is the transitional double render — every content rendered once
  by the pipeline for nobody and once by the fold for real — ending for all contents together, as
  the fold-cost note in `fold_resolved_title` said it would.

  *What is deliberately still here.* `run_pipeline` and the machinery only it reaches — the
  `Passthroughs` extraction pass, its replacers, the sentinel escape/unescape pair, the
  `substitute_attributes_in_text` helper — are twelve items now annotated
  `#[allow(dead_code)]` as vestigial: they compile into production builds unused, because the
  test-only oracle still calls them, and they are item (5)'s deletion, not this one's. The
  `from_tree: false` machinery (the escaped-form reads, `template_partition`) is likewise the
  oracle path's alone now.

  *The audit needed a reconstructed oracle*, since the comparison it runs no longer exists at the
  seam: the throwaway probe clones the content, runs `run_pipeline` on it against a parser clone
  — the same position and the same inputs the deleted call had — and compares the fold against
  that. **Zero new rows.** The set shrinks 63 → 56, every departure structural: rows contributed
  by the retired corpus's own fixtures, and ordinal-renderer rows whose bytes existed only while
  the pipeline consumed ordinals ahead of the fold. Coverage: `content.rs` *improves* to 28
  missed regions / 18 missed lines (30 / 20 before), `substitution_group.rs` holds 100%, and the
  vestigial `macros.rs` / `passthroughs.rs` gain 5 and 1 missed **regions** on unchanged
  missed-line sets — sub-line paths only production parses reached, dying with the machinery.

  *What still defers:* the `apply_string_pipeline` call sites, `run_pipeline`, and the vestigial
  machinery it keeps compiled (item 5); the `suppress_recognition_side_effects` window (measured
  dead), the `from_tree: false` paths and the `EscapedForm` split they justify (item 6); and the
  Strategy-A recorder with `inline_recorder` (item 7).

  *Step 6 landed as (the Strategy-A recorder retired — and the corpus split along its real
  subject):* the survey's item (7), pulled ahead of item (5) by a dependency the enumeration had
  backwards: `inline_recorder`'s oracle and the structural cross-check both *drive*
  `apply_string_pipeline`, so the test-only pipeline callable cannot be deleted while the recorder
  harnesses live. The recorder goes first.

  Deleted whole: `content::inline_tree` (the `RecordingRenderer` and its marker-sentinel fold —
  design §5.2's Strategy A, test-only since the tree-source swap),
  `inline_builder_recorder_parity` (the structural cross-check, its due diligence long since
  discharged at the swap), and `snapshots/recorder_trees.txt`, its frozen recording. The
  tree-shaped-freeze note had already ruled `inline_recorder`'s corpus unfreezable — its oracle
  runs the string pipeline twice, so every recording would compare the pipeline against itself —
  and said it retires with the pass. It has.

  *The file split along its real subject, which its name concealed.* Two thirds of
  `tests/inline_recorder.rs` was never about the recorder: the `is_block_inlines` surface, the
  resolution mirror, footnote subtrees, title-tree resolution, the description-list term, the
  ordinal-renderer pins, and the passthrough-order sweep all drive the **production** tree through
  ordinary parses. Those tests move whole to `tests/inline_tree.rs` (with `collect_rendered`, the
  walk two divergence-mirror tests still need); what retired with the recorder is its oracle and
  `check`/`check_document` machinery, the corpus tests comparing the recorder's tree against the
  pipeline's bytes, the node-shape tests of the *recorder's* tree (the builder's shapes have their
  own corpora), the reserved-sentinel rejection (a recorder-mechanism guard), and the
  `attach_footnote_subtrees` unit tests, whose subject was recorder machinery.

  Audit: **zero new rows**, and the set falls 56 → 12 — all 44 departures are rows whose
  `rendered` carries the recorder's own `U+E010`/`U+E02x` marker codepoints, contributed by the
  deleted harnesses' recorder-wrapped parses and gone with them. Coverage: every surviving file's
  missed-region count is unchanged; the only movement in the report is `inline_tree.rs`'s own rows
  leaving with the file (workspace total 609 → 606 missed regions).

  *What still defers:* item (5), now genuinely unblocked — the `apply_string_pipeline` call sites
  (the golden helpers go goldenless against their frozen recordings, per `snapshot`'s own
  transition plan), `run_pipeline`, and the vestigial machinery it keeps compiled — then item (6)'s
  window, `from_tree: false` paths, and `EscapedForm` collapse.

  *Step 6 landed as (`run_pipeline` deleted, and every corpus goldenless):* the survey's item
  (5), landed the increment after the recorder retirement unblocked it. `apply_string_pipeline`
  and `run_pipeline` are gone; every golden helper's body is the lookup the freeze design promised
  ("a helper's body becomes a lookup, its callers do not move"), reading `snapshots/<corpus>.txt`
  through a [`snapshot`](../../parser/src/content/inline_builder/snapshot.rs) API that no longer
  takes a golden at all. The drift guard and `ASCIIDOC_UPDATE_SNAPSHOTS` update mode went with the
  pipeline that fed them: a recording is now edited by hand and reviewed as the behavior change it
  records — the missing-fixture panic prints the ready-to-paste line where the caller holds the
  fold — and a divergence corpus's rows are frozen pipeline bytes that must never be refreshed
  from current behavior.

  *The seventeen structural-golden tests the survey's "mechanical" count missed.* Not every golden
  was a string: the per-family registration-parity tests read what the pipeline **registered** —
  catalogs and warning lists off a golden parser the helpers ran the steps against as a side
  effect. Each is now **frozen at the last differentially-verified parity**: the builder side is
  untouched by this increment and the suite was green against the live pipeline one commit
  earlier, so the literals baked into those tests are the pipeline's own answers, recorded the
  same way every corpus's bytes were. The three link-order tests already asserted literals and
  merely lost their trailing cross-checks; the two divergence tests among them
  (`a_family_matching_across_an_earlier_familys_markup…`, the quotes attribute-list one) keep the
  pipeline's rendering in the fixture as the recorded half and pin the divergence with
  `assert_ne!`.

  *Production is untouched by construction.* The seam's `apply_inner` did not change a line;
  outside comments, the whole diff is deletions of `#[cfg(test)]` and dead items (the callable
  pair, the sentinel-escaping tests that pinned `run_pipeline`'s own escape gating,
  `Passthroughs::observable`) plus the test-side rework. The golden-HTML suite and every frozen
  corpus pass unchanged, which is this increment's whole gate; the fold-parity audit's comparison
  is no longer constructible — there is no pipeline left to reconstruct — and by the same fact
  has nothing left to check here.

  *Coverage moves, and honestly.* The vestigial string-pipeline machinery — `macros.rs`'s
  replacers, `passthroughs.rs`'s extraction pass, the pipeline-only substitution steps — was
  exercised almost entirely by the oracle runs the golden helpers performed; with those gone its
  uncovered mass surfaces (workspace missed regions 606 → 1,775, all of it in the four files the
  tail deletion removes). Changed lines are covered; the uncovered lines are the dead machinery
  itself, and they leave with it.

  *What still defers:* the tail — item (6): the vestigial machinery whole (the replacers, the
  extraction pass, the sentinel escape window and `suppress_recognition_side_effects`, the
  `from_tree: false` paths and the `EscapedForm` split, `set_deferred_xrefs`,
  `finalize_deferred`), with the shared regexes and the two substitution steps the header/author
  machinery still runs in production (`SpecialCharacters`, `AttributeReferences`) staying behind.

  *Step 6 landed as (the tail's first slice — the compiler-verified dead set, and the suppression
  window closed):* item (6), cut where the compiler can vouch for every deletion: everything in
  this slice was `#[allow(dead_code)]`-annotated vestigial or became unreachable the moment those
  annotations came off, so stripping them and deleting what then failed to build *is* the
  increment. Gone: the extraction/restore machinery (`Passthroughs`, `ExtractedPassthrough`, the
  four replacers, `PASS_WITH_INDEX`); the sentinel escape window
  (`Content::escape_sentinels`/`unescape_sentinels` — the module-level pair stays, the carried
  title's template synthesis reads it); `finalize_deferred` and
  `restore_deferred_xref_passthroughs`; the free-standing `substitute_attributes_in_text`; and
  `suppress_recognition_side_effects` whole — measured again as a `Cell` nothing sets, so its
  seven remaining guards were constant branches and their removal changes nothing by
  construction. The `inline_builder` module's own crate-wide `#![allow(dead_code)]` goes too: the
  fold and the side-effect replays it excused have been production since the cutover, and what
  the allowance was actually hiding by the end was a leftover test helper.

  *The upstream ports migrated rather than died.* Asciidoctor's `extract_passthroughs` tests
  (`substitutions_test.rs`'s `passthroughs` mod) drove the extraction machinery directly.
  Sixteen now read the production view instead — `Content::passthroughs()` off a parsed simple
  block, exactly as the upstream tests read `@passthroughs` off `block_from_string` — and every
  one passed on first run, because the view was already differentially pinned
  (`inline_builder_passthrough_record_parity`). Two pin the restore pass itself, hand-planting
  placeholder sentinels in source text; restoration has no analog — the tree folds a passthrough
  body in place, it never re-splices a placeholder-bearing string — so they convert to
  `non_normative!` with that reason recorded. The `free_standing_text` mod went whole with the
  function it pinned.

  *Coverage recovers on schedule.* Workspace missed regions 1,775 → 1,296 (`passthroughs.rs`
  back to ~99%); the remaining mass is `macros.rs`'s replacers and the pipeline-only steps —
  the tail's second slice. Two new tests pin the string macros step's now-unguarded warning
  twins (the dangerous `link:` scheme rejection, `footnoteref:`'s deprecation and
  invalid-reference pair) until that slice deletes the step they live in.

  *What still defers:* the tail's second slice — the five pipeline-only `SubstitutionStep::apply`
  arms and the string replacers behind them (`macros.rs`'s mass, `apply_quotes`,
  `apply_character_replacements`, `apply_post_replacements`, `apply_callouts`), whose ~130
  remaining direct-step test call sites (96 in `substitutions_test.rs` alone) migrate to the
  tree the way this increment's sixteen did; the `from_tree: false` paths, the `EscapedForm`
  split, and `set_deferred_xrefs` — reachable only from that machinery — go with it.
  `SpecialCharacters` and `AttributeReferences` stay behind as before: `document/author.rs` runs
  both in production through direct step calls.

  *Step 6 landed as (the tail's second slice — the string replacers deleted, and one divergence
  the migration itself caught):* the fourteen replacer structs, `apply_macros`, the four
  pipeline-only step functions (`apply_quotes`, `apply_character_replacements`,
  `apply_post_replacements`, `apply_callouts`), the `LookaheadReplacer` machinery
  (`internal/regex.rs` whole, its "do NOT fix before the cutover" known bug finally moot),
  `set_deferred_xrefs`, `rehome_xref_placeholders`, and the replacer-only helpers
  (`strip_see_and_seealso`, `normalize_footnote_text`, `NormalizedCaps::scheme`) — 2,600 lines
  down, 375 up. What stays in `macros.rs` is exactly the shared surface the builder reads: the
  eleven recognition regexes, `NormalizedCaps`, and the text helpers. `SubstitutionStep::apply`
  survives with its two production arms (`SpecialCharacters`, `AttributeReferences` — the
  header/author machinery's direct calls) and one exhaustiveness arm that refuses the five
  deleted steps loudly, pinned by a `#[should_panic]` test rather than left uncovered.

  *The migration ran first, as its own gate.* All ~118 remaining direct-step test call sites
  (the 96 in `substitutions_test.rs`, the step's own unit tests, `security.rs`, the renderer
  parity sweep) moved to `SubstitutionGroup::Custom(vec![step])` — the production seam — before
  anything was deleted, so every fixture ran as a divergence probe against the tree while the
  string implementation still existed to compare against in spirit. **117 passed unchanged; the
  one failure was real**: the string footnote replacer re-creates Asciidoctor's `(?!</a>)`
  look-ahead and the builder's footnote family did not, so a macros-only order
  (`x footnote:[note]</a>`) built a footnote upstream refuses. The guard now lives in
  `find_footnote_matches`, keyed on the *escaped-form* match string — so under the normal order
  a document's own `</a>` reaches the pass as `&lt;/a&gt;` and the footnote is still recognized,
  exactly the string pipeline's behavior in both orders, pinned from both sides by a new unit
  test. No golden fixture had ever exercised the corner: direct step calls never passed through
  the audit's forced-build seam, which is why the migration had to be the probe.

  *Three golden helpers were still running the pipeline for nothing.* The dead-prelude species
  #1319's codecov fix first exposed: `golden_attributes_in`, `golden_replacements`, and
  `golden_callouts_in` each ran the string steps live and then returned the frozen recording,
  discarding the run. Collapsed to the lookup, keeping their parameters so call sites do not
  churn.

  *Coverage closes the loop.* The freeze increment's regression (606 → 1,775 missed regions,
  "all of it in the four files the tail deletion removes") reverses on schedule: 1,775 → 1,296
  (first slice) → **639**, with `macros.rs` at 99.6% (617 missed regions → 3) and
  `substitution_step.rs` at 99.8%. The debt the freeze took on knowingly is paid.

  *What still defers:* the constant-folding collapse the first slice's measurement set up — the
  `from_tree` field is now `true` for every constructible `DeferredContent` (`set_tree_xrefs` is
  the only producer left), so the `!from_tree` branches, the `EscapedForm` split, and the
  template-partition reads reduce to constants — deferred to its own increment because they
  thread through the live resolve/refold path; then step 7 (`render_with`/`render_to`,
  `Document::to_asg()`, the `attribute-missing` per-line hack), Phase 5, and Landing.

  *Step 6 landed as (the constant fold — item (6) closed):* the collapse the two slices set up,
  now that every flag has exactly one producer left. `DeferredContent::from_tree` (always
  `true`), `DeferredParts::from_tree` and `TitleNode::from_tree` (its copies),
  `FootnoteDeferred::sentinels_escaped` (always `false`: the string replacer's `true` call died
  with the replacer, and `define_footnote` loses the parameter with it) are gone, and every
  branch they gated folds: the resolve loops read a segment's `target` directly — a tree read
  *is* the document's own text — `template_partition` and its callers' else-arms are deleted,
  and the `EscapedForm` pair reduces to `render_template`'s one honest bool
  (`template_escaped`: `true` for a `DeferredContent`'s synthesized carried-title template,
  `false` for a `FootnoteDeferred`'s fold). The side-effect parity harness drops its
  `set_sentinels_escaped` normalization — there is no encoding difference left to normalize.

  *The dead arm the fold exposed.* Collapsing `resolve_references`' template arm showed its body
  was already unreachable: a deferring body content always has its tree installed and its
  attributes retained (the fold arm always binds), and the one template-rendering content — the
  carried block title — never enters the per-content resolution walk at all, because titles are
  resolved by the document-order title pass (`title_refs`), which splices through
  `render_xref_template`. Coverage agreed (the splice line had zero hits on the base too), so
  `Content::rebuild_rendered` is deleted rather than kept as an untested arm, per the same
  dead-defensive-branch doctrine every prior increment has applied.

  *Coverage:* 639 → **582** missed regions — below the 606 the whole freeze-and-delete arc
  started from. Every changed line covered.

  *One pre-existing bug rode along, because the review of this increment found it.* A
  **discrete** heading is the one section kind that keeps its own `.Title` decoration (every
  other section's is carried into its first block), and no pass resolved it: the title pass's
  `Section` arm reads only the heading, and `Block::block_title_content_mut` mapped `Section`
  to `None`, so its else-branch never saw one either. A cross-reference in such a title stayed
  at its unresolved fallback. This predates the whole deletion arc — the two probes render
  byte-identically on the base — and `rebuild_rendered` was never its write-back, so the fold
  above neither caused it nor could have fixed it by staying. `SectionBlock` now exposes the
  same `title_content_mut` seam every other titled block does, and both title-pass walks run
  their block-title branch for every block: heading first, decoration second, in the same order
  on each side.

  *What still defers:* step 7 (`render_with`/`render_to`, `Document::to_asg()`, the
  `attribute-missing` per-line hack #564), Phase 5's renderer seam, and Landing. Item (6) — and
  with it step 6's decomposition — is closed.

  *Step 7 landed as (the `attribute-missing` per-line hack retired — #564 closed):* the first
  of step 7's three pieces, and the one the cutover had already paid for without collecting.
  Issue #564's chosen approach was a **positional per-line correlation**: the attributes step
  ran over `Content::rendered` — text earlier steps had already lengthened — so a rendered
  offset had no constant delta back to source, and the fix was to retain each surviving line's
  source `Span` at construction, pre-scan that line for `{…}` matches, pair the *k*-th rendered
  reference with the *k*-th source one, and check the matched text to catch drift.

  *The builder makes the whole correlation unnecessary,* because it never has the problem:
  it recognizes each reference against `'src` and hands its **own node span** to
  `record_builder_diagnostic`. That is the honest answer the correlation approximated, and it
  has been serving production since the cutover — which measurement confirmed: a probe on the
  string step's `source_lines` branch fires for exactly five sources across the whole suite,
  every one of them the hack's own unit tests. A production content that carries per-line
  spans (a simple block) goes through the builder; a content that reaches the string step
  (an author line, a docinfo file, a `Custom` group over a caller's string) never had them.

  *One live consumer survives, and it needs no correlation either.* The macro-target path
  (`substitute_attributes_in_macro_target`, for `image::`/`video::`/`audio::` targets)
  substitutes a haystack that **is** its own source text — so the offsets the regex reports
  are already source offsets, and the span is sliced directly
  (`AttributeReplacer::over_its_own_source`). The new test asserting it names exactly
  `{missing}` passed on its first run, which is the proof the arithmetic reproduces what the
  pre-scan computed.

  *So the whole chain goes:* `source_line`, `source_matches`, `match_index` and the
  drift text-check in `AttributeReplacer`; `Content::source_lines` and its accessor;
  `from_filtered_lines`'s `line_spans` parameter and its per-line `debug_assert`; and
  `simple.rs`'s `filtered_line_spans` bookkeeping — a `Vec<Span>` built for every paragraph in
  every document, now unbuilt. Seven of the hack's tests were **kept, not deleted**: they
  assert precise per-reference spans, and re-pointing them at `SubstitutionGroup::Normal` —
  the production seam — leaves them asserting the same spans against the builder, so #564's
  acceptance criteria stay pinned by the mechanism that actually serves them.

  *What still defers:* step 7's other two pieces — `render_with`/`render_to` and
  `Document::to_asg()` — then Phase 5 and Landing.

  *Step 7 landed as (`Content::render_with` — one parse, any number of renders):* the
  Phase 3 remainder's core, and the first piece of this branch that is a **capability for
  callers** rather than an internal reshaping. A `Content` now folds its own tree through any
  backend the caller supplies, with no reparse and no parser reconfiguration — which is the
  promise §3.3.1 makes ("one parse feeds any number of renders") finally cashed, and it is a
  pure fold precisely because the builder resolved every order-dependent fact into node values
  at parse time.

  *The signature takes `parser`, and the reason was already settled in the code.* §3.3's
  sketch was `render_with(&self, renderer)`, which cannot work: a fold needs a
  [`RenderContext`](../../parser/src/parser/render_context.rs), which pairs the document
  attributes with the path resolver and the file handlers. The attributes are order-dependent
  and must be *this content's*; the resolver and handlers are parse-wide configuration that
  cannot change mid-parse — and they are `Rc<dyn …>`, so freezing them per content would cost
  `Content`, and with it `Document`, its `Send`/`Sync` (which `document_stays_send_and_sync`
  pins). `Content::render_attributes`' own doc had already reasoned this through and concluded
  that supplying "the parser the caller already holds" is the cheaper half of the trade. The
  sketch is corrected above rather than the decision revisited.

  *Retention widened from the deferring contents to all of them.* The attribute snapshot was
  taken only for content the crate itself re-renders (a deferred cross-reference, a defined
  footnote), on the rationale that "the overwhelming majority of content is never re-rendered
  and so never folded later than its parse". `render_with` makes later folding a first-class
  public operation for *every* content, which retires that premise: narrowing retention would
  leave the new API silently wrong, and wrong only for documents that rebind `:icons:`,
  `:data-uri:` or the safe mode mid-flight — the least forgivable shape for a bug. The cost is
  one box per content whose contents are `Arc`-shared, and the guard is a test that renders two
  paragraphs straddling an `:icons:` line and asserts they disagree: the first still folds as a
  font icon after the document has rebound the attribute out from under it.

  *A content with no tree returns its literal text.* `Content`'s public `From<Span>` builds one
  that was never substituted — no tree to fold, no attributes to fold it under — so the fold is
  skipped and the text returned unchanged, which is also what `rendered_html()` gives. (The
  shape that first looked like this case, a `[comment]` paragraph, turned out **not** to be one:
  `SubstitutionGroup::None` still runs the seam and still builds a one-`Text` tree, which folds
  back to the same literal bytes.)

  *What still defers:* `Document::render_to` (the document-level convenience over this), then
  `Document::to_asg()`, Phase 5 and Landing.

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
       decided. The token also has to make the *split* reproducible, which it does exactly when no
       character the split reads is hidden inside a piece — so the gate parses the tokened text and
       the restored markup and compares them attribute by attribute
       ([`tokened_split_agrees`](../../parser/src/content/inline_builder/macros/mod.rs)),
       deferring a match whose two readings differ about its extent. A masked passthrough or STEM in such a text comes along for free. See the step's
       own "landed as" note above. The **image** family's bracket — the one capture with no
       display text to carry — is what remains of the class.

     - ✅ **prep (a `link:`/`mailto:` macro whose own marker is not verbatim).** The one boundary
       in this module drawn around neither a piece nor a capture but around the node's own
       **`location`**. The string pipeline registers links in *pass* order rather than document
       order, so
       [`apply_link_side_effects`](../../parser/src/content/inline_builder/macros/links.rs) walks
       the tree three times — and it told the passes apart by reading the node's `location` back
       and asking whether it starts with a literal `link:` / `mailto:`. That made the location
       load-bearing, deferring every macro whose marker is not verbatim source: a wholly expanded
       `{m}`, one in a wholly-synthesized seed, and one inside markup an earlier step of the same
       order flattened. The signal is the node's to carry, which is Phase 4's whole business:
       [`Ref`](../../parser/src/inlines/ref_node.rs) gains
       `link_form: Option<`[`LinkForm`](../../parser/src/inlines/ref_node.rs)`>`, set by whichever
       pass builds it — a fact a consumer writing AsciiDoc back out needs in its own right, like
       an image's `is_icon` — and the marker's `range_is_verbatim` call is deleted rather than
       relaxed. All three formerly pinned divergences become parity tests, with a separate corpus
       driving the three-pass registration order through the real pipeline on both sides; see the
       step's own "landed as" note above.

     - ✅ **prep (a bare `+…+` body enclosing an already-extracted passthrough).** The
       passthrough-extraction step runs as two passes, so a bare `+…+` body can enclose a
       construct the first pass already replaced (`+a $$b$$ c+`,
       `` +you feel pass:q[`mono`].+ ``, both documented idioms). The string pipeline treats its
       own sentinel there as ordinary body text — substituting over it and letting the final
       restore splice the inner body in afterwards — where the builder read its body through
       `source_slice` and deferred on the atomic leaf. It now reads the body from the level's
       **match string** and walks that body's own `Piece`s
       ([`substitute_and_restore`](../../parser/src/content/inline_builder/passthrough_step.rs)):
       each run of ordinary text between two restorable pieces goes through
       [`passthrough_text`](../../parser/src/content/inline_builder/passthrough_step.rs) on its
       own, and each piece contributes its fold bytes verbatim — substitute first, restore after,
       which is what keeps an inner `<b>` from being escaped twice. Walking by piece rather than
       scanning for the placeholder character is what disambiguates a `SPAN_PLACEHOLDER` a source
       spells **literally**, which a scan would splice a body at and then silently truncate the
       rest of the body. The gate becomes
       [`range_is_restorable`](../../parser/src/content/inline_builder/macros/image.rs). The
       escaped-bracket retry's own re-scan inherits the lift, closing a second golden fixture; the
       two attribute-list-prefixed bare forms keep the verbatim gate, so `[method x-]+pass:[<b>]+`
       stays deferred. See the step's own "landed as" note above.

     - ✅ **prep (an attribute-list display text enclosing a rendered span — the two link
       families).** The same capture the cross-reference family closed, for the two families that
       hold a real [`Attrlist`](../../parser/src/attributes/attrlist.rs)`<'src>` on the node. A
       token has to *leave* their parse with something in it, since
       [`Ref::attrs`](../../parser/src/inlines/ref_node.rs) rides on the node — and a rendered span
       turns out to have a body for that purpose after all:
       [`tokened_bracket`](../../parser/src/content/inline_builder/macros/image.rs) gains a
       [`Tokened`](../../parser/src/content/inline_builder/macros/image.rs) kind whose
       `MaskedOrRendered` admits any opaque piece and pairs it with the **build-time fold**, the
       same trade `restorable_body` already makes for a `Stem`. It is safe because those bytes
       never reach output: the display text is carried structurally (the fold renders the span
       itself), and the frozen copy only lands in `attrs`, which no renderer reads for it. Every
       other slot is one `render_link` writes out, so a token reaching one defers the match. The
       walk also moves piece by piece, so a placeholder's sibling-boundary characters are not
       copied into the parsed value. See the step's own "landed as" note above. The **image**
       family's bracket — the one capture with no display text to carry — is what remains of the
       class, and needs a fold-*materialized* value rather than a frozen one.

     - ✅ **prep (a section title's footnote-free reference text).** The first of the three
       sentinel systems §4.2 names to lose its reason to exist, and the first prep found by asking
       *what breaks if `rendered_html()` becomes the fold today?* rather than by the fold-parity
       audit. A footnote in a heading must not appear in the text a cross-reference shows nor in
       the auto-generated id, and the string pipeline gets those two strings out of **one** render
       by bracketing each marker in a sentinel pair, cutting the bracketed regions out, and then
       removing the spent sentinels. A tree needs none of it: the footnote is a node, so the two
       strings are two folds of the same tree.
       [`fold_reference_text`](../../parser/src/content/inline_builder/fold.rs) skips each
       [`Footnote`](../../parser/src/inlines/footnote.rs) node wherever it sits, and a differential
       test pins the equivalence from both ends over a corpus of titles. See the step's own
       "landed as" note above, which also records what the experiment says about the cutover's
       remaining blast radius.

     - ✅ **prep (a whole-document parity harness for the builder, after resolution).** The gap
       the measurement above exposed in what the branch *verifies*, rather than in what it
       recognizes. The builder's own corpora run on a bare `Content`, which cannot resolve
       cross-references; the whole-document sweep that does reach resolution drives the
       *Strategy-A recorder*, which this step retired to test-only machinery. So the property the
       deferred-cross-reference sentinel system's retirement rests on — the builder's tree, folded
       **after** resolution, reproducing the rendered string — had no corpus behind it.
       [`inline_builder_document_parity`](../../parser/src/tests/inline_builder_document_parity.rs)
       gives it one, in a single parse (with the tree flag on, each location carries both its
       rendered string and its tree), reaching every content-bearing block kind through `IsBlock`
       rather than by variant — paragraphs, admonitions, and the raw-delimited family — plus
       section titles, table cells, and footnote subtrees, with guards against vacuity and against
       the walk narrowing. It passes as written. See the step's own "landed as" note above.

     - ✅ **prep (a corpus-wide side-effect parity harness).** The other half of what the
       blast-radius measurement left unmeasured: it "neither calls the staged side effects nor
       sequences the fold against resolution", and the harness above closed only the second
       clause. `apply_macro_side_effects` — the replay of the four registrations the string
       pipeline performs at recognition time — was pinned only by hand-written fixtures inside its
       own module.
       [`inline_builder_side_effect_parity`](../../parser/src/tests/inline_builder_side_effect_parity.rs)
       drives a corpus through both sides on two independent parsers and compares every catalog
       entry, id, and warning either wrote, in order. Unlike its sibling it does **not** pass as
       written: `IndexTerm` is the fourth node kind carrying `children`, added after all three
       side-effect walks were written, and none descended into it. The walks now do. The same
       fixtures expose a *recognition* gap with the same root cause — the families that run after
       the index-term pass cannot reach a plain visible term's shown text, which is a string
       rather than a subtree — deferred to its own increment and pinned by a divergence test with
       its parity complement beside it. See the step's own "landed as" note above.

     - ✅ **prep (a visible index term's shown text, handed back to the later families).** The
       recognition half the sweep above named, closed for the shorthand spellings. A visible term's
       shown text is not a boundary the other macro families stop at — the string replacer puts it
       back into the one flat haystack every later pass scans — so `shown_term` now builds the
       `children` subtree **always**, not only when no string can express the text, and the five
       families that run after this one move into an `apply_reference_families` that
       `apply_macro_families` calls once for this level and once per visible term over its own
       children (their own level, the transparent case `child_contexts` already answers). `terms`
       goes on carrying the same text as a string wherever one exists, so the node is widened
       rather than reshaped. Making `children` the common case surfaced a third walk with the same
       omission — `classify_unescaped_specials` — caught by the existing `build_for_group` corpus.
       The macro spellings still defer, since an attribute list's shown term is not a range of the
       match string; pinned by their own divergence test. See the step's own "landed as" note
       above.

     - ✅ **prep (an anchor's reference text, the fifth nested node list).** The side-effect sweep's
       own claim that the walks reach "exactly the four node kinds that carry `children`" asked of
       itself turns up a fifth immediately: `Anchor::reftext` is a nested node list not named like
       one, and so the one a walk written by matching on `children` is bound to miss. A corpus row
       for it exposed two divergences on every anchor spelling — a construct the reference text
       encloses went unregistered, and the registered reference *text* differed, since
       `build_anchor_reftext` deferred a text crossing an atomic piece and left the field `None`.
       The text is now carried structurally (the same close every sibling family made for its own
       display text), the three walks descend into it, and the registered string is a mixed fold:
       a `Text` run contributes its already-substituted match-string bytes unchanged, an enclosed
       construct is folded through the parser's own renderer. An anchor's rendering does not change
       — which is why the fold-parity audit could never have seen this — but what a cross-reference
       to it shows does, pinned end to end. See the step's own "landed as" note above.

     - ✅ **prep (footnote numbering in true source order).** `apply_footnotes` exists to walk the
       tree in source order, because a footnote's number is a side effect of recognition order —
       and it did not: `find_footnote_matches` **built** every node during its scan of the level,
       assigning every number there before the rebuild descended into any child, so a footnote
       nested in a child falling *between* two of its level's own was numbered after both. Every
       corpus missed it because a nested footnote before or after every sibling numbers the same
       either way, and no fixture had one in between. Construction is now deferred: the scan returns
       a capture per occurrence, and each node — and its number — is made at the moment the rebuild
       walk reaches it. The same pass gains `IndexTerm` (a visible term's shown text reaches the
       flow) and deliberately still skips an anchor's `reftext` (which the anchor replacer
       consumes). See the step's own "landed as" note above.

     - ✅ **prep (a cross-product sweep).** The three increments above each closed a walk that
       failed to descend into a container, and each time the reason no corpus caught it was that
       the construct and the container were covered separately but never crossed. This sweep is
       **generated** rather than listed: every nested node list a tree can hold a construct in,
       crossed with every construct the module recognizes, with the diverging pairs pinned as a set
       so one joining or leaving it fails either way. Three pairs diverge, on two root causes both
       new — a later macro family matching *across* the markup an earlier family of the same step
       emitted (a keep: the tree's answer is the well-formed one), and a post-replacement inside a
       cross-reference's display text, which the string pipeline never scans because a deferred
       cross-reference's text lives in a template (a keep that closes at the cutover, and a third
       piece of evidence for retiring that sentinel). See the step's own "landed as" note above.

     - ✅ **prep (the image family's attribute list).** Re-running the cutover experiment says the
       cutover leaves **18** of ~5,390 tests failing — 5 divergence tests that compare two paths the
       cutover leaves one of, 5 footnote-in-a-heading (which `fold_reference_text` closes), 3 flag
       tests, 2 recorder sweeps, and **3 golden fixtures whose output genuinely changes**. Two of
       those three the tree gets better or already documents; the third,
       `image:pause.png[title=*Pause* and Resume]` from the language docs, lost its whole macro —
       the boundary the link-family increment deferred, coming due. The image bracket now admits a
       rendered span: the per-slot rule that kept it out defers *every* image bracket (every value
       an image holds is one `render_image` writes out, and an image has no display text to carry a
       span structurally the way a link does), while the frozen bytes are exactly the bytes the
       string replacer reads out of its own already-rendered haystack — the same trade this
       function's masked branch already makes for a `Stem`. The split-agreement check still applies,
       and the **target** half of the boundary stays, now on its own reason (a target is resolved as
       a path). That leaves the cutover **one** golden regression rather than two. See the step's
       own "landed as" note above.

     - ✅ **prep (a quoted role, read from the attribute list's substituted text).** The cutover's
       **last golden regression**, filed as a keep by the increment above and not one: the
       attribute-references half of `flatten_prior_markup`'s category was a wrong answer in
       [`Attrlist`](../../parser/src/attributes/attrlist.rs) itself, which the string pipeline's own
       later step papered over. `parse` expands attribute references over the whole list before
       splitting it — so every *parsed* field is already expanded — but discarded that expanded text,
       leaving [`quoted_text_fallback_role`](../../parser/src/attributes/attrlist.rs), the one
       accessor that reads the list's own text rather than a parsed attribute, on the raw `source`
       span. `parse` now retains it as
       [`source_text`](../../parser/src/attributes/attrlist.rs) — the field the attributed-span
       increment added for exactly this accessor — and `into_owned`/`into_owned_restoring` carry
       `source_text()` forward, which is Asciidoctor's own `sub_attributes`-then-verbatim order. The
       quotes family's `['{myrole}']*bold*` and the passthrough family's `['{myrole}']++text++` land
       together as one parity corpus; the keep's other three shapes (a typographic replacement, a
       restored entity, a later sub's own span) are genuinely markup-reading and stay. See the step's
       own "landed as" note above. **The cutover now stands at zero golden regressions.**

     - ✅ **prep (a frozen recording, so the corpora survive the fold).** A cutover prerequisite
       nothing had noticed: a corpus that takes its golden from `apply` + `rendered` compares the
       **fold against itself** once `rendered_html()` is a fold, and passes for that reason.
       Demonstrated, not assumed — under a simulated cutover a deliberately sabotaged fold leaves
       the 259-fixture whole-pipeline corpus green. The golden becomes a checked-in recording
       (`parser/snapshots/`), read rather than derived, with
       [`assert_recorded`](../../parser/src/content/inline_builder/snapshot.rs) taking golden and
       fold as separate parameters so the fold can never author a recording; the golden is also
       checked against it as a drift guard, deleted with the string pipeline. Two corpora take it
       (the whole-pipeline sweep and the cross-product, the latter via
       [`matches_recording`](../../parser/src/content/inline_builder/snapshot.rs) since its
       subject is the diverging *set*). The generator is the string pipeline; Asciidoctor was
       measured (202/259) and deferred as spec-conformance work. See the step's own "landed as"
       note above. The remaining ~20 corpora are the **fold increment's prerequisite list**.

     - ✅ **prep (the golden-HTML oracle, as a callable).** That prerequisite list, discharged.
       A test-only [`apply_string_pipeline`](../../parser/src/content/substitution_group.rs) runs
       `run_pipeline` alone, and **25 golden-producing call sites across 14 files** take their
       golden from it rather than from `apply` — so each goes on differentiating against an
       independent construction for as long as the string pipeline exists, which is the window the
       cutover needs (the recordings are the durable answer for after). Landable ahead of the
       cutover precisely because it is byte-identical today: the whole suite stays green with **no
       test edited** and both recordings unchanged. The ~277 golden-HTML assertions keep `apply`
       deliberately — their subject is `rendered_html()` itself. See the step's own "landed as"
       note above.

     - ✅ **prep (the bare `+…+` body, which is literal text too).** A call-counting sweep over
       every construct — rather than trusting the previous increment's list — finds exactly three
       that still invoke the renderer while the tree is built, and the bare `+…+` form is one:
       ordinary AsciiDoc, and `Verbatim` like its delimited siblings. What actually freezes is the
       *mixture* a body enclosing an already-extracted construct produces, not the form, so
       [`substitute_and_restore`](../../parser/src/content/inline_builder/passthrough_step.rs)
       detects the mixture instead of assuming it. What is left is three shapes — a `pass:c,q[…]`
       body, a `Stem` body, and the mixture itself — none of them a *specialcharacters* body, all
       needing fold-time laziness.
       See the step's own "landed as" note above.

     - ✅ **prep (a passthrough body is literal text, not raw output).** The blocker review found
       on the always-on increment, and a conflation in §3.4's trichotomy: `++…++` / `$$…$$` apply
       special characters, so their body is the author's *literal text*, not raw output — and the
       builder was escaping it at build time through whichever renderer the parse carried. That
       both broke a custom backend's fold (measured: `a &lt; b` where the pipeline gives
       `a [LT] b`) and made building the tree *invoke* the document's renderer, shifting a later
       block's output for a stateful one. [`Raw`](../../parser/src/inlines/inline_node.rs) gains a
       [`form`](../../parser/src/inlines/inline_node.rs) (`AsIs` / `Escaped`); both stay opaque —
       which they must, since `build_match_string` decides atomicity by node *kind* — and the
       escaping moves to the fold. No expectation changed, and the audit's divergence set is
       byte-identical. See the step's own "landed as" note above. **This unblocks the always-on
       increment.**
     - ✅ **the flag, retired: the tree is built for every parse.** The cutover's first piece,
       and deliberately the one that changes no output: `SubstitutionGroup::apply` always takes
       the tree seed, while `rendered` stays the string pipeline's. Splitting "build it always"
       from "read it" is what makes each falsifiable alone — with the tree still additive, the
       only tests that *can* fail are the two asserting its absence, and only those did.
       `Parser::with_inline_tree` is gone from the public API; the `build_inline_tree` field it
       set survives as the **recursion guard** its plumbing was always also serving (the clone
       handed to the builder clears it, so a passthrough body's own re-entrant `apply` seeds no
       tree). See the step's own "landed as" note above. What remains of step 6 is
       `rendered_html()` as the fold, the three sentinel deletions, and the side effects wired
       for real.

     - ✅ **the footnote-marker sentinel system, deleted (the first of the three).**
       `Section::parse` takes a heading's footnote-free reference text from
       [`fold_reference_text`](../../parser/src/content/inline_builder/fold.rs) rather than from
       a byte-level cut, and `FOOTNOTE_MARKER_START`/`_END`, `strip_footnote_marker_spans`,
       `Content::remove_footnote_marker_sentinels`, `Parser::mark_footnote_spans`, and the
       replacer's two `dest.push(…)` calls all go. `rendered` is still the string pipeline's:
       what moves is one *derived* string. The sentinels existed only to make one render yield
       two strings (so counters ran once), which a tree does by construction. Measured
       corpus-wide before the swap — both answers computed for every section title in the suite,
       **zero** disagreements — rather than on the staged fixture corpus alone; the audit's
       divergence set shrinks by 16. See the step's own "landed as" note above.

     - ✅ **`rendered_html()` as an authoritative fold.** `SubstitutionGroup::apply` sets
       `content.rendered` from [`fold_html`](../../parser/src/content/inline_builder/fold.rs), so
       what a caller reads *is* the tree. Content carrying a deferred cross-reference keeps the
       template path for now (its rendered string is rebuilt on every resolution, so the fold
       there is the second sentinel system's own retirement) — one predicate, deleted by the next
       increment. The substantive work was that every differential corpus was about to become
       **tautological**, taking its golden from `apply` + `rendered`; a test-only
       [`apply_string_pipeline`](../../parser/src/content/substitution_group.rs) exposes
       `run_pipeline` alone and 21 sites across 12 files now use it. One golden changes, for the
       better (`#`CB###2`#`, whose mis-nested `<mark>`/`<code>` was an artifact of substituting
       over rendered markup). Audit: production divergences 18 → 9, none new, all nine accounted
       for. See the step's own "landed as" note above.

     - ✅ **prep (a cross-reference's effective `xrefstyle`, resolved into the node).** The first
       prep for the retirement below, and the first found by asking *what does a fold that runs
       later than its parse read differently?* [`Ref::xrefstyle`](../../parser/src/inlines/ref_node.rs)
       held only the macro's `xrefstyle=` override, with
       [`fold_xref`](../../parser/src/content/inline_builder/fold.rs) supplying the document-wide
       fallback at fold time. The effective style is a document-order fact — a `:xrefstyle:` line
       rebinds it for everything after it — so a re-fold at reference-resolution time would read
       the *end-of-parse* value and re-style a reference the string pipeline had already styled.
       The field becomes the effective style, resolved at build time by the same
       `document_xrefstyle` call `InlineXrefReplacer` makes in the same pass; `None` now means *no
       style* rather than *ask the document*. Pinned by four whole-document fixtures that fail on
       base, not by the audit, which cannot see this class. See the step's own "landed as" note
       above.

     - ✅ **prep (each deferred content retains its own document attributes).** A fold running
       later than its parse needs the document state its content was written under, and the parse
       has moved on; a content carrying a deferred cross-reference now keeps its
       [`ResolvedAttributes`](../../parser/src/parser/resolved_attributes.rs), snapshotted where
       "now" is still that point in the document. The *attributes* and not a whole `RenderContext`:
       that holds `Rc<dyn …>` handlers, so retaining one would cost
       [`Document`](../../parser/src/document/document.rs) its `Send`/`Sync` — which it has today,
       nothing pinned, and `document_stays_send_and_sync` now does. Attributes are order-dependent
       and must be frozen; the resolver and handlers are parse-wide and need not be. A provable
       no-op: audit 54 rows either side, 0 new, 0 closed. See the step's own "landed as" note above.

     - ✅ **prep (the fold takes a `RenderContext`, not a `Parser`).** Finishing what merging
       `main` started: the merge left [`fold_html`](../../parser/src/content/inline_builder/fold.rs)
       taking a `&Parser` and building a context *per rendered element*, which a later-than-parse
       fold cannot do. The context is threaded from the one place that starts it — strictly less
       work, and the ~117ns per rendered element #1265 measured goes with it.
       [`Attrlist::empty`](../../parser/src/attributes/attrlist.rs) replaces the parser-needing
       empty-list parse in `fold_image`/`fold_link`, and the builder's five internal folds take
       `parser.render_context()` at their own call sites. No behavior change. See the step's own
       "landed as" note above.

     - ✅ **prep (a `Raw` node records where its content came from).**
       [`Raw`](../../parser/src/inlines/inline_node.rs) gains an `origin` orthogonal to its `form`:
       `Passthrough` for what the extraction pass pulled out before any step ran, `Substitution` for
       raw output a substitution produced in place. For a consumer it sharpens §3.4's security story
       (a deliberate escape hatch vs. a value that may have arrived from elsewhere); for the builder
       it separates content the extraction pass is *holding* from content that is simply there —
       which the prep below needs. Inferring it from the extraction pass's `Masked` list was tried
       first and does not work: that list is keyed by location identity and empty on some call
       paths, so the same node classified differently depending on who asked. Nothing read it yet.
       See the step's own "landed as" note above.

     - ✅ **prep (a cross-reference whose target is attribute-expanded).** The first thing
       `RawOrigin` is for, and one of the two regressions the retirement's probe found.
       `xref:{cpp}[{cpp}]` was left literal while the string pipeline recognized it — the target
       crosses two `Raw` leaves the match string stands in as placeholders. They are `Substitution`
       leaves, so the string replacer's own haystack held exactly those bytes and filling them in
       *reaches* parity; a masked passthrough stays deferred, since it is restored by a later pass
       than the one that captures a deferred target. Turned up two more things: the shorthand needs
       the replacer's `id.contains('<')` guard for real (a substitution-produced `<` is not opaque),
       and only the **id half** may be restored — restoring the whole inner shifts the offsets the
       reference text is sliced with. See the step's own "landed as" note above.

     - ✅ **the deferred-cross-reference sentinel system, retired (the second of the three).**
       [`Content::refold`](../../parser/src/content/content.rs) folds a deferred content at the
       **end of resolution** — the same answer one step later — from the attributes it retained
       paired with the parser's configuration. The carve-out narrows rather than vanishing: from
       *any* deferred cross-reference to the count match
       [`mirror_tree_xref_resolution`](../../parser/src/content/content.rs) already computes, so the
       fold is authoritative except where the builder is known not to recognize the construct, and
       the gate self-liquidates as preps land. A whole-suite probe sized it: 14 divergences, 11 the
       test-only recorder's counter, 1 an improvement its own comment predicted, 2 real regressions
       — both closed as the two preps above. Exactly one of the two renderings runs: rendering both
       and keeping the second would be *observable*, not merely wasteful, since a stateful host
       renderer would see every callback twice in one pass
       (`resolution_renders_a_deferred_content_once`). That costs the whole-document harness its
       oracle for this content, so the template's answer becomes reachable test-only and a
       fixture corpus compares the two directly. What
       still defers is `xref:sec[*bold*,role=*hl*]` (a rendered span reaching a string-read value)
       and a **title's** deferred cross-references, which `title_refs` still renders from the
       template. See the step's own "landed as" note above.

     - ✅ **a title's rendering joins the fold — the retirement's other half.**
       [`title_refs`](../../parser/src/document/title_refs.rs) computes every title's rendering
       together, in document order, so a cross-reference between titles coordinates; it now folds
       the title's tree in place of rendering its template. **Inside `compute`**, not in the
       write-back walk — the pass needs each rendering *while it runs*, as the link text a
       reference to that title splices in, so folding afterwards would leave both and render every
       deferred title twice (`the_title_pass_renders_each_title_once`, which fails on that
       placement). It folds a **clone** of the tree, since the pass holds no `&mut` to the blocks
       there, and installs only the block-level destinations: a fold emits a footnote's marker
       without descending into its subtree. The coordination rides on the destinations, not the
       template, so it survives — pinned over a forward reference, a cycle, and a footnote in a
       heading, all of which pass on base too: **this changes no output.** 59 of 60 deferred titles
       in the suite take the fold, each reproducing the template byte for byte; the 60th has an
       empty tree. It costs three duplicate renderer callbacks per deferred title (13 → 16 on a
       heading carrying a span, an image and a special character) — the transitional cost of an
       authoritative fold, which every other content already pays and which ends for all of them
       when step 6 takes the string pipeline off the production path. It also corrected an oracle:
       the previous increment's template differential
       walked block titles, whose segments are never resolved in place, and passed only by
       coincidence. See the step's own "landed as" note above.

     - ✅ **the recognition side effects, re-attached for real.** The third of step 6's four asks,
       and the first that is not about the rendered string:
       [`apply_macro_side_effects`](../../parser/src/content/inline_builder/macros/mod.rs) — an
       image target, three link passes' targets, an anchor's and a bibliography entry's id, and the
       image family's dangerous-`link=` warning — runs from the tree, once per content, while
       `Parser::suppress_macro_side_effects` gates the string pipeline's own copies for the same
       pass. A **suppression window** rather than a deletion, because deleting the replacers'
       registrations is what the next step does when `run_pipeline` leaves the production path.
       A suppressed `register_ref` returns `Ok`, since the duplicate-id warning is the replay's to
       raise; the *link* family's dangerous-scheme warning is not suppressed, since the replay does
       not carry it. No rendered byte changes, so the audit is a catalog diff: **3,773** parses,
       images/links/refs/warnings byte-identical to base. Dropping the replay fails 69 tests;
       hoisting the window to a whole parse fails 161 — the second is the description-list **term**
       carve-out, the last thing still registering from the string pipeline. See the step's own
       "landed as" note above.

     - ✅ **the callout registration, the last recognition side effect.** Not a macro family — a
       callout is recognized in *verbatim* content, where the macros step never runs — so
       [`apply_callout_side_effects`](../../parser/src/content/inline_builder/callouts.rs) replays
       it as a sibling call, and the flag widens to `suppress_recognition_side_effects`. A
       `Callout` node already carries its **resolved** number, so the replay consults no counter,
       and the consumer (`Parser::callout_defined`) reads it a block later, when the callout list
       is parsed. A duplicate here would be *invisible* — `CalloutCatalog` is one `Vec<u32>` read
       through `contains` — so the gate buys correctness by construction rather than anything
       observable, and says so; what is falsifiable is the replay, without which every callout list
       in the suite warns. With this, **every** recognition side effect is performed from the tree,
       and what is left of step 6 is the string pipeline itself. See the step's own "landed as"
       note above.

     - ✅ **a computed string slot takes the author's untranslated source.** The survey of what
       `run_pipeline` still owns named one question two of its six items turn on: whether a
       computed **string** slot — `Ref::roles`, `window`, `xrefstyle`, and the link and image
       families' equivalents — can hold markup that exists only at fold time. It can, as the
       author's *source* for the piece:
       [`untranslated_value`](../../parser/src/content/inline_builder/macros/mod.rs) replaces each
       token with the node's value (a `Raw` leaf — the passthrough delimiters are syntax, the body
       is the literal text) or its source span (everything else), leaving attribute references and
       special characters alone. The cross-reference family's per-slot gate and
       `holds_carried_token` go with it, so `xref:sec[*bold*,role=*hl*]` is recognized and resolves
       through the mirror, and what still defers is the *split* disagreement alone
       (`xref:sec[a *b, c* d,role=hl]`) — which is what keeps the narrowed re-fold gate non-vacuous
       — plus a text carrying **author-written** `\u{96}`/`\u{97}` bytes, which make every token
       in it ambiguous.
       The fold is also safer than the string it replaces: a slot's value is text the renderer
       escapes, where a passthrough restored into the *rendered* string is not (asciidoctor#2661,
       matched on the string path, not reproduced on the tree's). Audit: 48 rows before, 49 after,
       all three moving rows the same two fixtures. This also unblocks the survey's other item (a
       deferred segment's `provided_text` takes the fold of the node's children — a display text is
       markup by nature). The image and link families do **not** follow automatically: the same
       rule there is a *parity break* on an AsciiDoc-language-docs fixture
       (`image:pause.png[title=*Pause* and Resume]`) rather than a closed deferral, and the slot has
       a third reading neither system offers, so it wants deciding on its own. See the step's own
       "landed as" note above.

     - ✅ **a description-list term joins the tree.** The carve-out the side-effect increment
       measured and named: a term ran the substitution steps *directly* — a hand copy of
       `run_pipeline`'s body predating the tree — so it had no tree, its rendering was not a fold,
       and its constructs registered from the replacers. It goes through
       [`SubstitutionGroup::apply`](../../parser/src/content/substitution_group.rs) now, under the
       `normal` order minus its attribute-references step (which already ran during *parsing*, so
       the `::` marker could be recognized). The term's one rule of its own — a leading `[[id]]`
       takes the **rest of the term** as its default reference text — runs from the same tree,
       between the build and the replay, which is what finally passes `leading_anchor_registered`
       its `true`. Reading it off the tree makes one deliberate difference: the default reference
       text is now the *rendering* of the rest, where the pre-macros regex catalogued a macro's
       source (`[[x]]image:a.png[]Term::`). A term also registers while it is *parsed*, so a
       cross-reference inside it contributes its unresolved fallback to the catalogued reference
       text and keeps it — a limitation of *when*, not of the tree, and still the better of the two
       readings (the regex caught the same reference as escaped source, never a link).
       `LEADING_INLINE_ANCHOR`,
       `apply_macros_with_leading_anchor_registered` and `InlineAnchorReplacer`'s own flag go with
       it. Audit: 49 rows either side, 0 new, 0 closed; coverage diff-neutral on all four files.
       See the step's own "landed as" note above.

     - ✅ **a computed attribute slot keeps its fold-time markup, made inert where it lands.** The
       question the increment above left open and named as needing a decision of its own: whether
       the image and link families' computed **string** slots should take the author's untranslated
       source, as the cross-reference family's now do. They should not — that moves
       `image:pause.png[title=*Pause* and Resume]`, an AsciiDoc-language-docs fixture, off the
       answer both this crate and Asciidoctor give — so the two families keep the *fold-time*
       markup, and the security property the source rule bought is obtained where it actually
       belongs, at the attribute the value lands in. The **image** family already froze such a span
       into its bracket; the **link** families deferred the whole macro whenever a token reached a
       slot `render_link` writes out, leaving `link:index.html[Docs,role=*hl*]` literal and its
       target unregistered. `rendered_token_escaped_the_display_text` is deleted, so the three
       [`Attrlist`](../../parser/src/attributes/attrlist.rs)-bearing families share one rule and
       [`tokened_split_agrees`](../../parser/src/content/inline_builder/macros/mod.rs) is the only
       thing a rendered piece must still satisfy. The safety half closes a real gap in the policy
       `render_image`/`render_icon`/`render_xref` already state: a link's `id` and `class` and the
       `target` a `window=` writes took no `encode_attribute_value`, so
       `link:x[Docs,role=+++a"b+++]` renders `class="a"b"` and injects what follows — reachable
       today with no tree involved. Audit: 36 rows either side, 0 new, 0 closed (the sweep itself
       had to be re-derived, `rendered` being the fold now); coverage diff-neutral on all changed
       files. See the step's own "landed as" note above.

     - ✅ **a deferred cross-reference's segments, read off the tree.** The second of the two
       survey items that turned on the computed-slot question, and the one the string-slot
       increment named as taking the *other* answer. An
       [`XrefSegment`](../../parser/src/content/content.rs) holds every field a `Ref{Xref}` node
       already carries but `provided_text`, which the segment holds as a string where the node holds
       its display text as **children** — so that slot takes the **fold** of them. A display text
       is markup by nature and the string replacer captures `<strong>bold</strong>` out of its
       own already-rendered haystack, where a computed *string* slot had no output worth matching
       (its template is captured before passthroughs are restored, so it leaks a sentinel). One
       family, two slots, two answers, each matching what its own string path produces.
       [`block_tree_xref_segments`](../../parser/src/content/content.rs) and
       [`footnote_tree_xref_segments`](../../parser/src/content/content.rs) walk exactly
       `assign_tree_xrefs`' and `assign_footnote_tree_xrefs`' own traversals, so a derived segment
       and an installed destination address the same node; both are staged and unwired, like every
       recognition side effect before its re-attachment. The fold runs at the **end of the parse**
       with the parse's own renderer — deriving it at resolution time would hand the resolver a
       different `ResolutionContext` than the string pipeline does — and `resolved` is
       deliberately not carried across, which is what makes the derivation idempotent.
       [`inline_builder_xref_segment_parity`](../../parser/src/tests/inline_builder_xref_segment_parity.rs)
       compares every field of every segment over 37 whole-document fixtures; exactly one shape
       diverges and it is the sibling increment's own `role=`, pinned beside a `provided_text` that
       is byte-identical. Review found one real bug, and not in the new code: a cross-reference
       inside a **visible index term** is deferred by the string pipeline while all four
       pre-existing xref walks skipped `IndexTerm`, so a reference hidden *between* two visible
       ones misaligned every one after it. All six walks now descend; the count guard had kept the
       old ones safe rather than complete. Audit: 36 rows either side, 0 new and 0 closed;
       coverage diff-neutral on both changed files. See the step's own "landed as" note above.

     - ✅ **prep (a `Raw` node records the passthrough it came from).** The survey's fifth item is
       the one it called a live *choice* — `Content::passthroughs()` is `pub` with no production
       consumer, so deleting it was as real an option as backing it with the tree. The choice is the
       middle path, and this is its first prerequisite. Measuring first shrank it: the public
       surface is two methods (`text`/`subs` — `type_` and `attrlist` have no accessor), and **five
       of seven** forms are already exactly recoverable from
       [`RawForm`](../../parser/src/inlines/inline_node.rs) plus the node kind. A `pass:c,q[…]` body
       defeats it twice — it folds `AsIs` just as `+++…+++` does, so the form cannot tell an
       arbitrary group from none; and an arbitrary group needs the substitution pipeline, which a
       fold taking a `RenderContext` rather than a `Parser` cannot reach, so the body is substituted
       at build time and `value` holds the result where `text()` returns the input. Both facts now
       ride inside
       [`RawOrigin::Passthrough`](../../parser/src/inlines/inline_node.rs) (`subs`, `source_text`)
       rather than beside it, so only the six passthrough-origin construction sites change and the
       invariant is structural. `RawForm` stays — it is the fold's contract, and the two
       deliberately disagree where the bare `+…+` form folds `AsIs` under a `Verbatim` group.
       Two forms record nothing where the pass records an entry (an inline STEM body, and the `x-`
       compatibility marker's subtree), pinned by their own test. Audit: 36 rows either side, 0 new
       and 0 closed; coverage diff-neutral. See the step's own "landed as" note above.

     - ✅ **prep (the last two passthrough forms record their own).** The two the increment above
       named: an inline **STEM** body (a [`Stem`](../../parser/src/inlines/stem.rs) node, not a
       `Raw` one) and the `x-` **compatibility marker** (whose body is a subtree). A `Stem` carried
       *neither* fact — its group varies by spelling (`Stem` / `Custom([…])` / `Normal`) and its
       `value` is already-substituted where `text()` is the author's — so it gains the same
       `subs`/`source_text` pair, spelled out rather than shared. The marker turned out to be one
       spelling of three: ``[x-]`tick` `` and `[x-]+++raw+++` already recorded via their `Raw`
       leaves, and only `[x-]++attr++` has none, so
       [`Styled`](../../parser/src/inlines/styled.rs) gains one
       `Option<`[`PassthroughWrapper`](../../parser/src/inlines/styled.rs)`>` — `None` for every
       ordinary span — marking the wrapper the extraction pass builds, which is what it records as
       one entry. Marking all three spellings creates the invariant that a walk must **not** descend
       into a marked wrapper, pinned over the two that would otherwise double-count. The **order**
       is now a decided difference rather than an observed coincidence: extraction order is a
       two-pass artifact, so the view will return document order, and the corpus compares facts as
       multisets while `the_view_returns_document_order` pins the order from both ends. Review found
       the one shape still short of an entry, which the corpus had a blind spot for: a STEM
       expression **embedding** a passthrough, where the pass records two entries (the inner body,
       and the STEM whose own text keeps the sentinel) and the tree records one — or, under an
       explicit substitution list, only the inner one, `apply_stem` building no node at all. A
       limitation rather than a regression, since a `Stem` carried neither fact before; pinned by
       its own test and owed to the view's increment. Audit: 36 rows either side, 0 new and 0
       closed; coverage diff-neutral on all three files. See the step's own "landed as" note above.

     - ✅ **prep (a `Stem` keeps its body's own nodes).** The structural half of the one shape the
       entry above left short of a record: a STEM expression **embedding** an already-extracted
       passthrough, where the pass makes two entries and the tree made one.
       [`Stem`](../../parser/src/inlines/stem.rs) gains `children` — the body's own nodes, where
       `value` is their *rendering* — so the inner `Raw` leaf and the record it carries survive
       instead of being folded into the value. Nothing reads the field yet, so no rendered byte
       moves. Measuring first narrowed the increment to that one field: the outer entry, which the
       note predicting this work named as the harder half, already agrees once a `Stem` carries its
       own group and source. A second test pins the invariant that keeps the change from spreading —
       `apply_stem` runs immediately after `apply_passthroughs`, so a STEM body holds only
       `Text` / `Raw` and no *other* walk is obliged to descend into it. Still short of a record is
       the explicit **non-local** substitution list (`stem:c,q[x +++<b>+++ y]`), which
       `build_stem_node` declines outright, leaving no node to hold anything; that is the view
       increment's to answer. Audit: 37 rows either side, 0 new and 0 closed; coverage diff-neutral
       outside the new tests' own `panic!` arms. See the step's own "landed as" note above.

     - ✅ **prep (a `Passthrough` holds only what it exposes).** The last prep before the walk.
       [`Content::passthroughs`](../../parser/src/content/content.rs) hands out a `pub`
       [`Passthrough`](../../parser/src/content/passthroughs.rs) with **four** fields where only two
       are documented or exposed; the other two are restore-pass machinery and neither is
       recoverable from the tree (`type_` separates two prefixed spellings a
       [`Styled`](../../parser/src/inlines/styled.rs) wrapper renders identically, `attrlist` is the
       author's *unsubstituted* source where the node holds an
       [`Attrlist`](../../parser/src/attributes/attrlist.rs) parsed from the substituted one). A
       tree-built view could only have supplied `None`, and the derived `PartialEq` / `Eq` / `Hash`
       make that visible. So the type is split: `Passthrough` keeps `text` and `subs`, and a
       crate-internal `ExtractedPassthrough` carries those plus the two restore-only facts — the
       same "make the invariant structural" move #1287 made from the other end. The one observable
       consequence is the point: two entries with the same body and group are now equal, pinned
       from both sides by `two_entries_with_the_same_body_and_group_are_equal`. Sixteen checked-in
       golden expectations carrying `type_` / `attrlist` **move**, unchanged; sabotaging either
       fails 21 and 22 tests. Audit: 37 rows either side, 0 new and 0 closed; coverage diff-neutral
       outside the new test's own `panic!` arm. See the step's own "landed as" note above.

     - ✅ **`Content::passthroughs()` is a view over the tree.** The survey's fifth item, closed —
       the four preps above were all for this walk.
       [`Passthrough::from_tree`](../../parser/src/content/passthroughs.rs) replaces the retained
       extraction list: a `Raw` of passthrough origin is an entry, a
       [`Stem`](../../parser/src/inlines/stem.rs) is an entry *and* a container to descend, and a
       **marked** [`Styled`](../../parser/src/inlines/styled.rs) wrapper is an entry the walk must
       **not** descend into. The extraction pass still builds its own list — the restore pass
       indexes into it by sentinel — but that list is now private to one pipeline run, and
       `Passthroughs::observable` survives only as `#[cfg(test)]`, for the corpus that compares the
       two answers. The **order** is now document order, announced two increments ago and carried
       into the release notes by the commit message (`CHANGELOG.md` is `release-plz`-generated).
       The corpus changed subject to match: `golden` re-runs `Passthroughs::extract_from` over a
       throwaway `Content`, because comparing the method against a copy of itself would assert
       nothing. `a_group_that_does_not_extract_reports_nothing` crosses three non-extracting groups
       with four spellings, because the *gate* moved — same answer, different mechanism, which is
       the exact substitution that hid the previous two gaps. Still divergent, each pinned: a
       nested STEM entry holds the restored body where the pass keeps the sentinel, and a non-local
       explicit subs list builds no node to report the outer entry from. Audit: 37 rows either
       side, 0 new and 0 closed; coverage exactly diff-neutral (3 missed regions and 3 missed lines
       before and after). See the step's own "landed as" note above.

     - ✅ **the deferred cross-references, read off the tree.** The first of the survey's six items
       to go from staged machinery to the production answer:
       [`Content::set_tree_xrefs`](../../parser/src/content/content.rs) installs what
       [`block_tree_xref_segments`](../../parser/src/content/content.rs) and
       [`footnote_tree_xref_segments`](../../parser/src/content/content.rs) return, so a content's
       deferred cross-references are its **tree's**. The substantive part is the *partition*: the
       two walks split block-level from footnote-embedded structurally, where the string pipeline
       produced one flat list and re-derived the split from which placeholders its template still
       spliced. Both halves then correlate positionally with the walks that install destinations
       back, which makes the footnote mirror exact rather than merely safe — the one behavior
       change, and an improvement (a recognized reference inside a footnote whose sibling the
       builder defers now resolves in the tree; nothing rendered moves). The sentinel system is
       **not** deleted here, and measuring says why: the carve-out
       [`from_tree`](../../parser/src/content/content.rs) names is reached by two real shapes, one
       of which (`indexterm2:[<<b>>]`) had been pinned only under `parse_deferred` and would
       otherwise have regressed a resolved document's output. Deleting it is gated on the builder
       recognizing every cross-reference form the replacer does — a prep question — plus the one
       content that has no tree to fold at all (a block title carried across a section heading,
       whose nodes cannot cross the `'src`-erasing `pending_block_title` hop). Audit: 37 rows
       either side, 0 new and 0 closed; coverage exactly diff-neutral on all four changed
       production files. See the step's own "landed as" note above.

     - ✅ **four recognition diagnostics, recorded where they are recognized.** The first side
       effects the tree-walk replay **cannot** carry: a rejected `link:` macro stays literal, an
       invalid substitution name in a `pass:`/`stem:` list is skipped, a `footnoteref:` builds the
       same node its modern spelling does, and an undefined footnote reference looks exactly like a
       forward one — so none of them leaves a node to replay from. Each is recorded at its own
       recognition site and carried onto the real parser afterwards, with the string pipeline's copy
       joining the existing
       [`suppress_recognition_side_effects`](../../parser/src/parser/parser.rs) window. The
       mechanism is where the work was: the builder's deliberate diagnostics need a buffer of their
       own ([`record_builder_diagnostic`](../../parser/src/parser/parser.rs)), because the clone's
       warning buffer also collects what an `Attrlist` parse records *incidentally* over a match
       string — warnings the string pipeline discards and which would otherwise surface mislocated —
       and the drain needs a **mark**, because `passthrough_text` re-enters `apply` and the nested
       clone would carry the outer build's diagnostics across twice. Both were caught by the suite,
       not by inspection. Audit: 37 rows either side, 0 new and 0 closed; coverage exactly
       diff-neutral on all eight changed files. What still defers is the fifth diagnostic,
       `SkippingReferenceToMissingAttribute`, which belongs to the attributes *step* rather than to
       a macro family, is located per line, and is the very diagnostic the incidental-recording
       hazard is about. See the step's own "landed as" note above.

     - ✅ **every corpus frozen, so deleting the string pipeline stays honest.** The prerequisite
       list the recording increment named and left: roughly twenty golden-producing helpers whose
       golden is *computed*, and which have nothing left to compute it from once `run_pipeline` is
       deleted — at which point `assert_eq!(folded, golden)` is `assert_eq!(x, x)`. Simulating that
       world for one corpus leaves `fold_matches_the_string_pipeline_through_link_macros` green with
       a stray byte appended to every fold; routing the same helper through a recording fails it on
       the first fixture. [`recorded_golden`](../../parser/src/content/inline_builder/snapshot.rs)
       is [`assert_recorded`](../../parser/src/content/inline_builder/snapshot.rs) turned inside
       out — it freezes the **helper's return value**, since these corpora have no single assertion
       to wrap (a caller may compare a fold against the golden, compare it against a literal, assert
       an `assert_ne!` divergence from it, or merely `contains` it). Same asymmetry, so every one of
       roughly 550 call sites now compares against bytes settled before the fold ran without a
       single one being edited, and the eventual deletion is local: a helper's body becomes a
       lookup. **Thirty-nine corpora, 3,422 fixtures**, up from two and 378; seven tests whose
       parser makes a shared source render differently name their own corpus, a policy `decide`'s
       existing `Conflict` enforces rather than convention. A comparison against a *literal* needed
       nothing and is left alone. Audit: 37 rows either side, 0 new and 0 closed — necessarily, the
       whole increment being `#[cfg(test)]`; coverage exactly diff-neutral. What still defers is the
       **structural** half: the three `parser/src/tests/` corpora that compare trees and records
       rather than HTML, which need an `InlineNode` serialization rather than a wider sweep of this
       mechanism. See the step's own "landed as" note above.

     - ✅ **the fifth diagnostic, and the last one.** `SkippingReferenceToMissingAttribute` — the
       `attribute-missing` `warn` and `drop-line` diagnostic — recorded at the builder's own
       recognition site and carried across, with the string pipeline's copy joining the existing
       [`suppress_recognition_side_effects`](../../parser/src/parser/parser.rs) window. It landed
       **because a probe of the inversion said it had to**: giving the string pipeline the
       counter-safe clone and the builder the real parser fails 27 tests of 5,474, collapsing into
       exactly two root causes — this diagnostic and the footnote catalog entry. Neither is
       unblocked by the inversion; both block it, which is the reverse of how the plan had them.
       (`{counter:}`, carried as a third blocker, does not appear at all: the clone seeds both
       passes from the same pre-substitution state whichever way round they run.) Each of the three
       reasons this was held back turned out to be answerable: it lives in the attributes step
       rather than a macro family, which only moves the code; its per-line location is the string
       pipeline *recovering* a span the tree never loses
       ([`source_slice`](../../parser/src/content/inline_builder/quotes.rs) maps the match's range
       to `'src` directly); and the incidental-recording hazard it is the subject of is exactly what
       the previous increment's two-buffer split already fixed. Two things are load-bearing: the
       diagnostic reads [`AttributeMissing`](../../parser/src/content/substitution_step.rs) directly rather than
       `MissingHandling`, which collapses `skip` with `warn` and falls back to `Literal` for shapes
       whose *bytes* it cannot reproduce — so the `drop-line` divergence stays a divergence in bytes
       only; and the order must be **restored** by a stable sort, since the splicing recursion
       visits a span's content before its own level. The sort was the one thing nothing pinned
       (removing it left the whole suite green), so the increment's real test is a new configured
       pair in
       [`inline_builder_side_effect_parity`](../../parser/src/tests/inline_builder_side_effect_parity.rs)
       sweeping both diagnosing modes over ten fixtures, four straddling a span. Audit: 37 rows
       either side, 0 new and 0 closed; coverage exactly diff-neutral on both changed production
       files. What still defers is the footnote catalog entry — the probe's other root cause, and
       the last thing before the inversion. See the step's own "landed as" note above.

     - ✅ **the footnote catalog entry, folded from its own subtree.** The probe's other root
       cause, and the last thing before the inversion.
       [`register_footnote_number`](../../parser/src/content/inline_builder/footnotes.rs) takes the
       footnote's `children` rather than its raw bracket text and folds them, so the entry stops
       being the *match string* (in which an already-recognized construct is one opaque
       `SPAN_PLACEHOLDER` codepoint) and becomes the same bytes the string replacer captures out of
       its already-substituted haystack. The mechanism is a **second mode axis** on the fold —
       `Xrefs::Rendered`/`Deferred`, alongside the existing `Footnotes::Marked`/`Stripped` — under
       which `fold_xref` writes a placeholder and pushes `xref_segment_from_node(...)` instead of
       rendering, so one pass yields the template *and* the segment list in matching order: exactly
       the pair `define_footnote` turns into a `FootnoteDeferred`, which a tree-built footnote never
       had. The cost is a documented exception to "building the tree consults no renderer": the
       entry is a required recognition side effect whose payload is a rendered string, so
       registering it and rendering it are one act (and `fold_footnote` never folds a footnote's
       children into the flow, so nothing is rendered twice). Extending
       [`inline_builder_side_effect_parity`](../../parser/src/tests/inline_builder_side_effect_parity.rs)
       to compare the footnote catalog *whole* immediately caught a second bug — the entry was
       anchored at the macro's span where the string pipeline anchors it at the enclosing content's,
       which is where a footnote's unresolved-reference warning is reported. Audit: 37 rows either
       side, 0 new and 0 closed (five rows shift only in the test-only recorder's PUA index digits);
       coverage exactly diff-neutral on all three changed production files. What still defers is the
       inversion itself. See the step's own "landed as" note above.

     - ✅ **the inversion.** `run_pipeline` moves onto the counter-safe clone and
       `build_for_group` onto the real parser, so the pass that is authoritative for the rendered
       string is the one holding the parser that keeps what it recognizes — and `run_pipeline`
       becomes a pure oracle, which is what makes its deletion a deletion rather than a rewrite.
       Three things move with it: the reentrancy half of `build_inline_tree` becomes
       [`Parser::in_inline_build`](../../parser/src/parser/parser.rs) (a shared reference cannot have
       a field cleared on it); the build's *incidental* `record_substitution_warning` calls, which
       used to be discarded by sitting on the clone, are now truncated explicitly; and a passthrough
       body's re-entry becomes the authoritative pass for that body, since the enclosing tree folds
       it to one `Raw` value and cannot replay its constructs. The guard is the piece nothing pinned
       — removing it leaves the suite green, the body merely substituted twice — so the increment's
       real test measures it with the branch's own `OrdinalRenderer`, where three calls become four
       and the ordinal reaching the output shifts. Audit: 37 rows either side, 0 new and 0 closed
       (the branch's raw 43 is this increment's own new test, checked by deleting it). Coverage is
       deliberately *not* diff-neutral: `parser.rs` gains three uncovered lines, the
       `suppress_recognition_side_effects` early returns, now unreachable because the seam was that
       window's only setter — it is vestigial as of here and goes whole with `run_pipeline`. See the
       step's own "landed as" note above.

     - ✅ **the passthrough record corpus, frozen — and the deletion, scoped.** The first slice
       of the deletion, and a scoping increment as much as a code one. Surveying the surface revises
       the menu twice. `apply_string_pipeline` is `#[cfg(test)]` and `run_pipeline` has two
       production callers: the oracle pass, whose output the fold overwrites, and the
       `tree_seed == None` pass, which is **authoritative** (a passthrough body's re-entry from
       inside a build). So the deletion is gated on that second branch — production work — not on
       the oracle. And the corpora still to freeze divide by the shape of what they compare:
       *record-shaped* (`inline_builder_passthrough_record_parity`,
       `inline_builder_side_effect_parity`), which need only a codec for their own record, and
       *tree-shaped* (`inline_builder_recorder_parity`, `inline_recorder`), which need the
       `InlineNode` serialization **and** a per-side normal form, since their comparison is a
       pairwise normalization rather than equality. The list above misfiles the passthrough corpus
       with the tree ones and omits the side-effect corpus, which is a live differential that dies
       with the same deletion: the freeze is four corpora, not three. This increment lands the first
       record-shaped one, as a **round trip** rather than a string comparison because its assertions
       read the golden's structure — including whether the outer STEM entry still carries the
       `\u{96}` extraction sentinel, the artifact the deletion removes and the reason to freeze this
       corpus rather than retire it with the pass. Audit: 63 rows either side, 0 new and 0 closed
       (nothing in production moved; 63 rather than the previous increments' 37 because the recipe
       was corrected, not the branch — see the note above). Coverage diff-neutral: both changed
       production files add no executable line and stay at 100%; the codec is under `src/tests/`,
       which the report does not measure, and is covered whole by its own
       `the_record_codec_round_trips_every_spelling`. See the step's own "landed as" note above.

     - ✅ **a passthrough body's warnings are located against the body's own span.** The
       follow-up the inversion's review opened, and the one place its swap moved a diagnostic:
       `pass:m,a[['{alpha}']++x++ and {beta}]` reported `alpha` at offset 2 before and 0 after
       (absent before, present after, under `drop-line`), while `{beta}` stayed at 30 on both sides.
       Neither number pointed at `{alpha}`, which sits at 11 — the body was substituted from an
       unanchored `Span::new`, so *both* passes had nothing to locate against. `passthrough_text`
       now takes the body's own `Span` with per-line spans, and every reference reports where it is
       written. The escape the review actually named — the unguarded `Attrlist::parse` — does not
       reach the surfaced set: every warning it records across the suite lands on the oracle's
       clone. Rendered output is unchanged; this is diagnostics only. See the step's own "landed as"
       note above.

     - ✅ **the side-effect corpus, frozen — the record-shaped pair closed.** The survey's own
       next item, and the one the pre-survey menu omitted: `inline_builder_side_effect_parity` is a
       live differential whose golden side is `apply_string_pipeline`, so without a freeze the
       deletion would leave every assertion in it comparing the builder against itself, silently.
       Same shape as the freeze above — the golden helper becomes a lookup, the pipeline goes on
       being checked against the recording, and no assertion changes what it asserts — and a round
       trip for the same reason, since the assertions read the golden's structure (five list
       lengths, a footnote's `text`, whether its `deferred` is `Some`, the ids in the catalog).
       The **richer** record is where this one differs, and it costs three decisions.
       `WarningType` is recorded as its `Debug` spelling rather than decoded — fifty-odd variants,
       injective for these payloads — while `RefType` keeps its real type, so the module's positive
       assertions still name `RefType::Anchor` and (through a `spellings` helper) the two
       `WarningType` values themselves. A `Footnote` is recorded **field by field**, because its
       own `Debug` omits `location` and encoding the struct whole would have dropped one of the
       five compared facts from the freeze with nothing failing; `FootnoteDeferred`'s `Debug` omits
       `sentinels_escaped` in the same way, which costs nothing only because the harness already
       normalizes that field away. And it is the first corpus whose recording key is not the
       fixture source alone: the same source is swept under `attribute-missing=warn` and
       `=drop-line`, which write different warning lists, so the key is `config\u{1}source`. The
       format is counted rather than delimited (five variable-length lists in a row), and the codec
       carries its own round-trip and corruption tests, the corpus driving only a narrow slice of
       the record's shape space. Nothing in production moved: audit 63 rows either side, 0 new and
       0 closed, and coverage byte-identical either side. See the step's own "landed as"
       note above.

     - ✅ **the tree-shaped freeze — and one corpus that does not freeze.** The survey's harder
       half, and one place it was wrong about the shape. `inline_recorder` is filed as tree-shaped
       and is not a freeze candidate at all: its `oracle` runs the string pipeline **twice** (once
       for the golden, once under `RecordingRenderer`), so every assertion in it compares two
       products of the same pipeline and freezing it would compare a recorded value against itself.
       It retires with the pass. That leaves `inline_builder_recorder_parity` as the whole item —
       and it is the one that genuinely needs a per-side normal form, since
       `assert_trees_equivalent` is a pairwise normalization (it ignores `location`/`attrs`, folds a
       builder leaf to consume the recorder's rendered bytes, and splits a recorder `Text` run at a
       builder leaf's edge) rather than equality. The normal form is the one the recorder **already
       satisfies**: the module doc comment's own list of fields a recorder-built node cannot carry
       *is* the restriction, so the recording holds exactly the fields the comparator reads and
       decoding rebuilds real `InlineNode` values — the comparator is untouched and no assertion
       moves. A partial normal form's one hazard (the comparator growing a read of a field the
       recording does not carry) is closed by `strip_unrecorded`, which writes the dropped set down
       in one place so the harness can assert `strip(live) == decoded` by plain equality on every
       fixture, making the check total rather than argued. `Raw` is refused outright (a builder-side
       leaf the recorder recovers as `Text`/`CharRef`), and a `CharRef::Replacement`'s `&'static str`
       comes from `RECORDER_ENTITY_TABLE`, already drift-guarded against production's
       `classify_entity`. Nothing in production moved: audit 63 rows either side, 0 new and 0 closed,
       and coverage byte-identical. See the step's own "landed as" note above.

     - ✅ **the authoritative-pass closure — the last production blocker.** `run_pipeline`'s second
       production caller, the `tree_seed == None` branch, was authoritative for one content: a
       passthrough body re-entering `apply` from inside a build. It closes in four lines, because the
       obstacle was misidentified. The argument inherited from the deleted `build_pass_macro_subs_value`
       — that a body cannot be threaded through this module's transducers, since `build` runs a fixed
       normal order and would revisit any structural node the body's own subset produced — is about
       **splicing** the body's nodes into the enclosing level, not about computing its string; folding
       the body's own tree and wrapping it in one `Raw` leaf keeps it exactly as opaque as
       `subs.apply` did. And `build_for_group` already runs an arbitrary group's steps in that group's
       own order, including the `Custom` orders that put the escaping step after a step that produced
       markup. Measured: the branch goes from **112 hits to 1** across the suite, the 111 that went
       all being the passthrough re-entry, the one that remains being the crate test that turns
       `build_inline_tree` off. The body's rendering was reached 378 times under fifteen distinct
       `Custom` spellings including out-of-order pairs, and
       `a_passthrough_body_renders_under_every_order_its_own_list_can_name` pins both orders of two of
       them. Audit: 63 rows either side, 0 new and 0 closed. Coverage deliberately *not* diff-neutral:
       `substitution_group.rs` gains six uncovered lines, exactly the `nested_authoritative_warnings`
       block this makes unreachable — vestigial as of here, and left standing to go whole with
       `run_pipeline`. See the step's own "landed as" note above.

     - ℹ️ **the deletion is not one increment, and the hard half is `set_tree_xrefs`.** Surveying
       the surface the closure left. `run_pipeline` has four references in the whole crate, all in
       `substitution_group.rs`, and `apply_string_pipeline`'s ~28 call sites are `#[cfg(test)]`
       helpers over frozen corpora — so the mechanical half really is mechanical. What is not is
       `Content::set_tree_xrefs`, which reads the string pipeline's **`deferred` state** rather than
       its bytes: it keeps the placeholder `template`, which only that pipeline produces, and a
       carve-out where the tree defers fewer cross-references than the pipeline did. Measured across
       the suite it resolves 12,852 / 436 / 6 — nothing-deferred, tree-segments-with-the-pipeline's-
       template, and carve-out. The template (436 contents) is the larger debt by volume; the
       carve-out is six hits over five sources, and reading what each *is* matters more than the
       count, because they are **two different kinds and only one is a gap**. `indexterm2:[<<b>>]` is
       the index-term family's own documented limitation and closes like any other prep. The other
       four are the `xref:sec[a *b, c* d,role=hl]` shape, where the tree does not fail to recognize
       but **declines** to: the replacer splits the attribute list over the piece's rendered markup
       and lands the split *inside* a `<strong>` tag, and the tree defers rather than claim a
       construct the rendered document does not agree with. So the carve-out is the mechanism by
       which that answer reaches the output, and deleting `run_pipeline` leaves nothing to fall back
       to — a behavioral decision, **taken at the survey: diverge.** The replacer's split lands inside
       `<strong>b, c</strong>` and leaves the anchor's text as `a <strong>b`, unbalanced; emitting
       that is conceptually wrong, so the tree's own reading stands and the branch accepts a
       documented divergence from the string pipeline and Asciidoctor. Decomposition: the
       `indexterm2:` gap, the deferral divergence (now ordinary work — narrowing
       `tokened_split_agrees`'s decline, scope settled against the audit's set), the template, the
       oracle call, the test call sites and `run_pipeline`, the vestigial mechanisms, then the
       Strategy-A recorder with `inline_recorder`. See the step's own "landed as" note above.

     - ✅ **the deferral divergence, taken.** The survey's item (2), decided by the maintainer and
       implemented: `tokened_split_agrees` is gone, with `restored_markup_text` and the image
       family's now-always-true `bracket_is_recognizable`, and all three call sites. What the gate
       cost is clearer from its removal — it deferred whenever the tokened split and the replacer's
       markup split disagreed, which meant **a comma inside a span decided whether the macro was
       recognized at all**: `xref:sec[a *b, c* d,role=hl]` folded to literal text where
       `xref:sec[a *b* d,role=hl]` folded to an anchor. All three families now read the comma case
       the way they already read the comma-free one. The image row was checked rather than assumed —
       the baseline already renders `alt="a <strong>b</strong> d"` without the comma, so markup in an
       `alt` is pre-existing behavior and nothing new leaks into an attribute. The audit reads
       differently here than anywhere else on this branch: 63 rows and 61 sources either side with
       **identical source sets** and identical `(source, rendered)` pairs, while `folded` moves on
       exactly four rows from the literal text to the anchor. The divergence persists — the pipeline
       still writes the cut-short `a <strong>b` — so this is not "four rows closed"; the tree's answer
       went from absent to right, which is what an *intended* behavior change looks like under a bar
       written to catch unintended ones. Coverage neutral in total, improving where the dead code
       went. See the step's own "landed as" note above.

     - ✅ **the `indexterm2:` gap — and the carve-out emptied.** The survey's item (1), the last
       member of the `set_tree_xrefs` carve-out. An `indexterm2:[…]` shown term came back from an
       attribute-list parse rather than a range of the match string, so the builder kept it as a
       string and built no subtree — a `<<b>>` inside it computed to the escaped literal
       `&lt;&lt;b&gt;&gt;` and was never recognized, where the shorthand `((<<b>>))` carries children
       the later families descend into. What made it closable is that the narrowing has a byte:
       `shown_macro_term` already returns its argument unchanged when it holds no `=`, so the term
       *is* the whole shown range and the range's nodes describe the same text; only with an `=` does
       a list narrow it to a first positional attribute. Carrying `shown.children` under exactly that
       condition narrows the family's documented limitation from "the macro spelling" to "the macro
       spelling with an attribute list" — and passing them through unconditionally was tried first,
       folding `indexterm2:[Coffee, region=Kona]` to the whole bracket, which is what the original
       note warned of. **The carve-out is now unreachable**: instrumented across the suite it fired
       six times at the survey, once after the deferral divergence, and zero now, so it goes with
       `run_pipeline` and the template is all `set_tree_xrefs` still takes from the pipeline. Audit:
       62 rows against 63, 0 new and 1 closed — the pipeline was right here and the tree caught up,
       the mirror image of the divergence increment before it. Coverage diff-neutral. See the step's
       own "landed as" note above.

     - ℹ️ **the template is two named cases, not a majority path — and both prior figures were the
       wrong population.** The deletion survey called it "the larger debt by volume (436 contents)";
       436 counts `set_tree_xrefs` calls that installed a tree answer. Instrumenting the *arm choice*
       in `resolve_references` corrected that with a worse number — 330 fold calls over 210 distinct
       sources against 5,944 over 2,437 for `rebuild_rendered` — because `rebuild_rendered` early-
       returns on `deferred.is_none()` and **all 5,944 took that return**. Instrumenting
       `render_template` itself gives the real population: `finalize_deferred` 794 calls / 170
       templates, `Footnote::render` 225 / 24, `title_refs`'s fallback **1 / 1**, and
       `resolve_references` **0**. So every content that defers a cross-reference folds its tree (not
       8% of them), the template renders zero times from the resolution pass, `finalize_deferred`'s
       render goes with `run_pipeline`, and what remains is `Footnote::render` plus a single carried
       block title whose `inlines` are empty because they cannot cross the `'src`-erasing hop. See
       the step's own "landed as" note above.

     - ✅ **`build_inline_tree` retired — the last remnant of the `with_inline_tree` opt-in.** The
       field was `true` for every parser the public API can construct (measured false for 1 of 13,299
       parses reaching the seam: the one crate test that set it), so the seed condition drops to the
       reentrancy guard alone and no production parse changes. The increment turned up that the
       seam's two "otherwise" causes are **not** equivalent — a tree-less parse must *keep* the
       string pipeline's warnings, a pass under the reentrancy guard must *stash* them for the
       enclosing build — so the old test's warning assertion belonged to the retired cause and the
       replacement pins what the guard actually contracts: a pass under it builds no tree.
       `in_inline_build` and `nested_authoritative_warnings` stay: they are the two ends of one
       mechanism, dead by measurement (0 of 13,299) but not by construction. See the step's own
       "landed as" note above.

     - ✅ **the reentrancy guard and its warning transport retired — the seam has one path.**
       `Parser::in_inline_build` guarded against a re-entry the authoritative-pass closure already
       removed; two independent readings agree it is dead (no production code under
       `content::inline_builder` calls `SubstitutionGroup::apply` — every such call there is inside a
       `mod tests` — and the guard was set for 0 of 13,299 parses at the seam). `tree_seed` is now
       unconditional, so the `None` arm is unreachable and goes, **taking the authoritative
       `run_pipeline` call site with it**; `run_pipeline` survives with one caller, the oracle.
       `nested_authoritative_warnings` goes in the same motion, both ends, since the branch that fed
       its stash is the branch deleted. `a_passthrough_body_is_substituted_once_per_apply` — whose
       `OrdinalRenderer` is what can see a doubled pass at all — is the check: its counts do not
       move. Audit zero-new/zero-closed; `substitution_group.rs` reaches 100% regions and lines. See
       the step's own "landed as" note above.

     - ✅ **a footnote folds its own subtree — the first template consumer closed, and without the
       lifetime work the survey assumed.** The catalog entry does not need to *hold* a tree: the
       defining `Footnote` node in the enclosing content's tree carries the children, and
       `Document::resolve_references` holds blocks and catalog in one closure, so only a `String`
       travels. (Threading `'src` would have been the worse option anyway — the catalog is assembled
       on the reuse-across-documents `Parser`, so `Footnote<'src>` forces `Parser<'src>`.) Folds ride
       `ReferenceWarnings`, the accumulator already threaded through exactly the traversal that
       reaches every content, so the two owned sub-parses a generic walk misses cannot be forgotten;
       crossing them is explicit per site, because a table cell's footnote `1` must never overwrite
       the document's. Contents that define a footnote now retain their render attributes too.
       **Still on the template:** a footnote in a section heading, resolved by the title pass — 49 of
       55 fold. Pinned by a stateful-renderer test, since no output comparison can see the
       difference. Audit: one new row, this step's own fixture; no pre-existing source moved.
       Coverage diff-neutral. See the step's own "landed as" note above.

     - ✅ **the title pass folds its footnotes too — 55 of 55, and the footnote template is unused.**
       The 6 footnotes still on the template were those defined in a section heading, which
       `document::title_refs` owns rather than `Content::resolve_references`. The fold belongs in
       `write_back`, not beside `fold_resolved_title`: that fold runs on a *clone* carrying only
       `block_ordered`, while a footnote's rendering needs `footnote_ordered`, which reaches the real
       tree only when `write_back` mirrors it. Extending `write_back` also avoids a third walk beside
       `collect` and `write_back` that could drift from both. Both passes now collect through one
       `Content::collect_own_folded_footnotes`. `FootnoteDeferred::render` is left with a single
       caller, the parse-time unresolved fallback, which goes with the oracle. The template arm of
       `Footnote::resolve_references` stays — unreachable by measurement, not by construction — and is
       pinned by a direct unit test rather than deleted. Audit zero-new/zero-closed; coverage
       diff-neutral. See the step's own "landed as" note above.

     - ✅ **the carried title's template is the tree's own — the template item closed, and the
       suppression window measured dead.** `Content::to_owned_title` takes the parser and, for a
       title deferring a cross-reference, synthesizes the placeholder template and its segment
       list from the title's own tree at the stash site, while the nodes are alive; the
       `'src`-erasing hop and the splice-rendering restored side are unchanged, so the one content
       that renders from a template in production no longer reads anything the string pipeline
       produced. The synthesis is a **top-level gap walk**, not one `fold_deferring_xrefs` call:
       each gap's fold passes through `escape_sentinels` and each placeholder is emitted raw, so
       the template keeps the escaped form the render path already handles and a document-typed
       placeholder (issue #1235) stays distinguishable from a real one — the ambiguity the
       footnote template carries but never renders, and a carried title would have. The price is a
       cross-reference *nested* inside a top-level construct, which bakes as its unresolved
       fallback (measured zero in the suite; pinned with its own test, as is the empty-section
       re-stash that must keep the first hop's template). Byte-identical against the base on the
       typed-sentinel and two-reference probes. Also measured here, as the deletion survey asked:
       `suppress_recognition_side_effects` is initialized `false`, read at ten sites, and **set
       nowhere** — dead since the inversion, its retirement a pure deletion filed with the
       vestigial mechanisms. Audit 63 rows either side, 0 new and 0 closed; coverage
       diff-neutral. See the step's own "landed as" note above.

     - ✅ **the oracle call deleted — the seam is single-pass.** `SubstitutionGroup::apply` no
       longer runs the string pipeline; `run_pipeline` survives only as the test oracle behind
       `apply_string_pipeline`, and the machinery only it reaches (the extraction pass, its
       replacers, the sentinel escape pair — twelve items) is annotated vestigial and goes with
       item (5). `set_tree_xrefs` is the sole producer of deferred state, deriving both lists
       from the tree behind a short-circuiting `tree_defers_xrefs` walk; the **carve-out** and
       `template_splices` are deleted on the measurement that emptied them, and a production
       `DeferredContent` carries no template and no `string_xrefs` snapshot. Two harnesses
       retired exactly as their own docs promised (`inline_builder_xref_segment_parity`, the
       document-parity template comparison); the golden-HTML suite pinned identical bytes either
       side — no rendered output moved anywhere. The stateful-renderer pins moved exactly as
       their comments predicted (`a [second] b` → `a [first] b`; passthrough-body counts three →
       one): the transitional double render ends for all contents together. Audit via a
       reconstructed-oracle probe: zero new rows, 63 → 56 with every departure structural.
       Coverage: `content.rs` improves to 28/18, `substitution_group.rs` holds 100%; the
       vestigial files gain six sub-line regions that die with item (5). See the step's own
       "landed as" note above.

     - ✅ **the Strategy-A recorder retired — pulled ahead of the call-site deletion it
       blocked.** `content::inline_tree` (the `RecordingRenderer` and marker-sentinel fold),
       `inline_builder_recorder_parity`, and `snapshots/recorder_trees.txt` deleted whole; the
       `inline_recorder` corpus retired exactly as the tree-shaped-freeze note said it must (its
       oracle runs the string pipeline twice, so it can never be frozen). The dependency the
       survey's ordering missed: both harnesses *drive* `apply_string_pipeline`, so item (5)
       cannot delete it while they live. Two thirds of `tests/inline_recorder.rs` was production
       -tree testing under a recorder's name and moves whole to `tests/inline_tree.rs`; what died
       is the oracle machinery, the recorder-tree corpora and shape tests, and the
       `attach_footnote_subtrees` units. Audit zero-new, 56 → 12 with all departures the deleted
       harnesses' own marker-bearing rows; coverage unchanged on every surviving file. See the
       step's own "landed as" note above.

     - ✅ **`run_pipeline` deleted — every corpus goldenless.** `apply_string_pipeline` and
       `run_pipeline` are gone; golden helpers read the frozen recordings through a `snapshot`
       API with no golden parameter, and the drift guard and update mode went with the pipeline
       that fed them (a recording is hand-edited and reviewed as the behavior change it records).
       Seventeen structural-golden tests — registration catalogs and warning orders read off a
       pipeline-run parser, which no string recording could freeze — are frozen as literals at
       the last differentially-verified parity. Production untouched by construction (the seam's
       `apply_inner` did not change a line); the golden-HTML suite and every frozen corpus pass
       unchanged. Coverage surfaces the vestigial machinery's uncovered mass (606 → 1,775 missed
       regions, all in the four files item (6) deletes). See the step's own "landed as" note
       above.

     - ✅ **the tail's first slice: the compiler-verified dead set deleted, the suppression
       window closed.** Item (6) cut along what the compiler can vouch for: the
       `#[allow(dead_code)]` annotations came off and everything that then failed to build went —
       the extraction/restore machinery (`Passthroughs`, `ExtractedPassthrough`, four replacers,
       `PASS_WITH_INDEX`), the `Content` sentinel escape window, `finalize_deferred`,
       `restore_deferred_xref_passthroughs`, `substitute_attributes_in_text`, and
       `suppress_recognition_side_effects` whole (a `Cell` nothing sets; its seven guards were
       constant branches). Sixteen upstream `extract_passthroughs` ports migrated to the
       production `Content::passthroughs()` view and passed on first run; the two restore-pass
       ports converted to `non_normative!` (restoration has no analog — the tree folds in
       place). Coverage recovers 1,775 → 1,296 missed regions; what remains is the second
       slice's (`macros.rs`'s replacers, the five pipeline-only step arms and their ~130
       direct-step test call sites). See the step's own "landed as" note above.

     - ✅ **the tail's second slice: the string replacers deleted, the migration as its own
       gate.** All ~118 remaining direct-step test call sites moved to
       `SubstitutionGroup::Custom(vec![step])` *before* the deletion, each fixture a divergence
       probe: 117 passed unchanged, and the one failure was a real recognition gap — the
       builder's footnote family lacked Asciidoctor's `(?!</a>)` look-ahead, now implemented on
       the escaped-form match string and pinned from both orders. Then the cut: fourteen
       replacers, `apply_macros`, four step functions, `internal/regex.rs` whole,
       `set_deferred_xrefs`, `rehome_xref_placeholders`, and the replacer-only helpers — 2,600
       lines; the shared regexes and text helpers stay, and `SubstitutionStep::apply` keeps its
       two production arms plus a `#[should_panic]`-pinned refusal for the five deleted ones.
       Coverage: 1,296 → 639 missed regions (`macros.rs` 617 → 3) — the freeze's debt paid.
       What remains of item (6) is the `from_tree`/`EscapedForm` constant-folding collapse, its
       own increment. See the step's own "landed as" note above.

     - ✅ **the constant fold — item (6) closed.** Every flag with one producer left folds:
       `from_tree` (always true) and `sentinels_escaped` (always false) deleted with every
       branch they gated; `template_partition` gone; `EscapedForm` reduced to
       `render_template`'s one honest `template_escaped` bool. The fold exposed
       `Content::rebuild_rendered` as already unreachable — titles resolve through the
       document-order title pass, never the per-content walk — and it is deleted per the
       dead-defensive-branch doctrine, with coverage as the witness (zero hits on the base
       too). Coverage: 639 → 582 missed regions, below the arc's 606 starting point. See the
       step's own "landed as" note above.

     - ℹ️ **the *link* family's dangerous-scheme warning is part of this step, not a prep for it.**
       The last survey item that is not hard-blocked, and the one the survey said would need "a
       node-level fact, the way `RawOrigin` was one". Measuring says otherwise: it is the single
       side effect that is never **suppressed**, so the string pass records it right up to the
       cutover, and at the cutover the builder holds the *real* parser rather than the clone whose
       warnings are discarded — so its existing rejection site in
       [`build_link_node`](../../parser/src/content/inline_builder/macros/links.rs), which already
       takes `parser`, records it directly. No tree fact to replay, because there is no replay; and
       no `pub` type widened for the duration of a transition and narrowed after it. See the step's
       own "landed as" note above.

  7. `render_with` (the Phase 3 remainder) and `Document::to_asg()`, now that nodes are
     self-describing; retire the `attribute-missing` per-line hack (#564).
     - ℹ️ **`Document::render_to` is not being built.** It appears in §3.3.1 only as an
       unspecified parenthetical beside `render_with` — no return type, no account of what it
       would assemble — and it does not survive being specified. The renderer it would take is
       an [`InlineSubstitutionRenderer`](../../parser/src/parser/inline_substitution_renderer.rs),
       whose fifteen methods are *inline* constructs to the last one: there is no
       `render_paragraph`, no `render_section`, no `render_list`. A document-level fold through
       it could only concatenate inline renderings with no block structure around them, which
       is not a document in any backend. Assembling one is the **converter's** job — the
       Ruby-to-Rust `asciidoctor` port is the consumer that does it (§6's question 6), and this
       crate deliberately exposes a *model*: `Document` has accessors for the header, authors,
       attributes, TOC, catalog and blocks, and has never had a rendering surface. The
       capability that was genuinely missing is per-content folding through a caller's backend,
       and `render_with` is that; a caller who wants every content rendered writes the walk,
       which is three lines and theirs to shape. Building `render_to` would commit the crate to
       an opinion about document assembly it has spent its whole design avoiding.

     - ✅ **`Content::render_with`.** A public pure fold of a content's own tree through a
       caller-supplied backend — one parse, any number of renders. Takes `parser` rather than
       the doc's original zero-argument sketch: a `RenderContext`'s resolver and file handlers
       are `Rc<dyn …>` parse-wide configuration, and freezing them per content would cost
       `Content`/`Document` their `Send`/`Sync`. Attribute retention widens from the deferring
       contents to all of them, since the new API folds any of them later than its parse;
       pinned by a test where two paragraphs straddling an `:icons:` line disagree. See the
       step's own "landed as" note above.

     - ✅ **the `attribute-missing` per-line hack retired (#564).** The correlation existed
       because the string step's haystack was `Content::rendered`; the builder recognizes
       against `'src` and records its own node span, so it never had the problem. Measured
       production-dead (the probe fires only for the hack's own tests), and the one live
       consumer left — the macro-target path — substitutes its own source text, so a match's
       offsets *are* source offsets. `Content::source_lines`, `from_filtered_lines`'s
       `line_spans`, and `simple.rs`'s per-paragraph `Vec<Span>` go with it; the seven
       precise-span tests are re-pointed at the production seam rather than deleted. See the
       step's own "landed as" note above.

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
   `render_with` and the parse-time renderer config dropped (§3.3.1) — this is
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
- **#564** — ✅ retired: the builder's node spans replaced the per-line correlation (Phase 4, step 7).
- **#942** — its `InlineNode` shape and recording renderer are reused as prior art and as
  the Phase 1 bring-up oracle; its known limitations (owned strings, no spans, double pass,
  drift) are the specific things Phases 2 and 4 eliminate.

## References

- Eclipse AsciiDoc Language ASG schema:
  <https://gitlab.eclipse.org/eclipse/asciidoc-lang/asciidoc-lang/-/blob/main/asg/schema.json>
- AsciiDoc substitutions: <https://docs.asciidoctor.org/asciidoc/latest/subs/>
- Internal issues: #892, #942, #943, #944, #564.
