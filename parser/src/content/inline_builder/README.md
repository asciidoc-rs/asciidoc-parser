# `inline_builder/`

Builds the [`InlineNode`](../../inlines/mod.rs) tree that every parse's content is made of,
directly from source in a single forward pass. This is the module `parser/snapshots/README.md`
refers to when it talks about "the builder" — the two files complement each other: that one
documents the frozen golden recordings the differential corpora here read from, and its
"Background" section covers the crate's original string-substitution implementation in more
depth than is repeated below.

## From string rewriting to a tree

This crate used to implement inline content (bold, links, images, footnotes, cross references,
…) as **string rewriting**: `Content` held one mutable rendered string, and each substitution
step edited it in place, recognizing a construct and rendering it to HTML in the same motion.
Three things that don't fit in a flat string — passthroughs, deferred cross-references, and
footnote markers — were smuggled through it with Unicode sentinel characters, cut back out once
their real destination was known.

That model is retired. Inline content is now built as data: each step in this module is a
**transducer** over a node list, `Vec<InlineNode<'src>> -> Vec<InlineNode<'src>>`, that refines
the tree in place rather than recovering it after the fact from a rendered string. Passthroughs
are `Raw` nodes, a deferred cross-reference is a `Ref` node resolution fills in **in place**, and
a footnote marker is a `Footnote` node — no sentinel encoding is left in production code.
`SubstitutionGroup::apply` (`parser/src/content/substitution_group.rs`) builds this tree once per
content, and both `Content::rendered_html()` and every macro family's catalog/warning
registration are derived from it: the tree is the single source of truth, not a second
implementation running alongside a string-based one.

Building the tree directly, rather than recovering it from a string, gives two properties a
post-hoc recovery could not:

- **Honest per-node spans.** A node is sliced straight from the source `Span`, so its `location`
  reports the real `line`/`col`/`offset` of the construct (issue #944), instead of every node
  carrying the whole-content span.
- **`'src` borrowing by construction.** A verbatim text run's `value` borrows the very bytes its
  `location` covers, so the common case allocates nothing.

## The escaping-order rule

Several steps' comments refer to one recurring rule for how a construct's escaping status is
decided: whether a byte sequence like `<`/`>`/`&` is treated as already-escaped or as raw text
depends on where the `specialcharacters` step sits in the content's *effective* substitution
order (`Normal`'s built-in order runs it first; a `subs=` list can move it, and
`subs=attributes+` is the case that puts it after `attributes`). A step downstream of
`specialcharacters` sees markup as logical text; a step upstream of it — or a group that omits
`specialcharacters` altogether — sees raw, unescaped bytes and must classify them itself
(`classify_unescaped_specials`, `flatten_prior_markup`). Each step that depends on this reads the
order at build time (`SplicedSpecials`, `ComputedSpecials`) rather than assuming a fixed
position, since a `Custom` substitution list can reorder it.

## Match strings and recoverable pieces

Several node kinds have no `Span`-typed field of their own — an anchor's id, a bare e-mail
address, a UI macro's keys/label/menu path, an index term's shown text, a cross-reference's
target and reference text. Recognizing these inside content that has no single contiguous `'src`
slice (an expanded attribute reference, a synthesized multi-line seed) means reading a
**match string**: a level's own escaped/normalized text, built by `quotes::build_match_string`,
which carries the same escaped bytes and canonical entities the crate has always produced for
this content. A **recoverable piece** — an escaped special, a restored entity, a typographic
replacement, or a masked passthrough placeholder — can be read back out of that string exactly,
without needing an `'src` slice; what still defers is a piece that only exists as *rendered
markup* (a `Styled` span), since a span's tags exist only at fold time and there is no range of
source bytes to recover them from.

## Recognition side effects

Every macro family recognizes its construct without performing the catalog registration or
warning that goes with it — that happens once per parse, after the tree is built and folded, via
`apply_macro_side_effects` (and, for callouts, `apply_callout_side_effects`), which replays each
family's own side effect from the finished tree in the crate's usual family-pass order. Keeping
recognition and registration separate is what lets every family's tests build and inspect a tree
without a full `Parser`/`Content` round trip.

## Testing

Per-step differential corpora (`golden_*` helpers) pin each transducer against the frozen
recordings in `parser/snapshots/`; see that directory's README for the recording format and the
whole-corpus harnesses. This module's own `tests` submodule additionally runs the *real*,
assembled pipeline (`build`/`build_for_group` through `SubstitutionGroup::apply`) over broader
sweeps — a construct × container cross-product, combined-construct fixtures, and
synthesized-seed fixtures — to catch interactions between steps that a single-family corpus
cannot.
