//! Builds the inline AST **directly from source in a single forward pass**.
//!
//! This is the first brick of "Strategy B" (design §4.1): rather than recover
//! the tree from a post-substitution *marked string* — the
//! `inline_tree` recorder's "Strategy A", now test-only — each
//! substitution step is recast as a **transducer** over a node list,
//! `Vec<InlineNode<'src>> -> Vec<InlineNode<'src>>`, that refines the tree in
//! place. Two properties fall out that Strategy A cannot offer:
//!
//! 1. **Honest per-node spans.** A node is sliced straight from the source
//!    [`Span`], so its `location` reports the real `line`/`col`/`offset` of the
//!    construct (issue #944), instead of every node carrying the whole-content
//!    span.
//! 2. **`'src` borrowing by construction.** A verbatim text run's `value`
//!    borrows the very bytes its `location` covers, so the common case does not
//!    allocate.
//!
//! # Status
//!
//! Strategy B "touches every step," so it lands incrementally under the
//! golden-HTML oracle. This module currently implements the **foundation** plus
//! these refinements:
//!
//! - [`build`] seeds a single borrowed whole-source [`Text`](InlineNode::Text)
//!   node and threads it through the steps; [`build_from_value`] generalizes
//!   the seed to the `(value, location)` pair
//!   [`Content`](crate::content::Content) itself is built from, so a genuinely
//!   multi-line, filtered block — whose joined text has no single contiguous
//!   `'src` slice — can be processed too, with every node it produces falling
//!   back to `location` as its coarse span (design §4.4).
//! - [`apply_special_characters`] splits each `Text` run on `<`/`>`/`&` into
//!   precise-span [`Text`](InlineNode::Text) and
//!   [`CharRef`](InlineNode::CharRef) nodes.
//! - [`apply_quotes`] recognizes [quoted text] and wraps each matched run in a
//!   [`Styled`](crate::inlines::Styled) span, **introducing nesting** — `*a _b_
//!   c*` becomes a tree, not a flat run. It reuses the *exact* [`quote_subs`]
//!   the string pipeline matches with (changing the recognition *sink*, not the
//!   recognition), so its fold is byte-identical to the string step.
//! - [`apply_attribute_references`] recognizes attribute references (`{name}`),
//!   splicing a set attribute's resolved value into the node stream, classified
//!   into [`Text`](InlineNode::Text) and [`Raw`](InlineNode::Raw) runs per
//!   design §3.4.1 — a literal `<`/`>`/`&` in the value is *not* re-escaped,
//!   since [`apply_special_characters`] has already run. It reuses the shared
//!   [`ATTRIBUTE_REFERENCE`](crate::content::ATTRIBUTE_REFERENCE) pattern, so
//!   only the recognition *sink* differs. A `counter`/`counter2` directive
//!   resolves *and advances* the named counter via [`Parser::counter`], the
//!   same required side effect [`apply_footnotes`] performs for footnote
//!   numbering. A missing-attribute reference under `AttributeMissing::Drop` /
//!   `::DropLine` is deferred (see [`apply_attribute_references`] for why). A
//!   character replacement *inside* an expanded value ((C) → ©, `--`, …) is
//!   recognized, a follow-up increment having taught
//!   [`build_match_string`](quotes::build_match_string) to look inside a
//!   [`synthesized`](quotes::Piece::synthesized) run instead of treating it as
//!   opaque; a **macro** needing an honest `'src` slice for its own
//!   target/attribute list (a link or an image, the two that are left) still
//!   defers when that slice sits inside one, since a synthesized run's bytes
//!   have no source counterpart to slice for a type that requires a real `Span`
//!   (see [`range_is_verbatim`](macros::image::range_is_verbatim)) — but a
//!   macro needing only its own *text* (no `Span`-typed field) — an anchor's
//!   id, a bare e-mail address, a UI macro's keys/label/menu path, an index
//!   term's shown text, or a cross-reference's target and reference text — can
//!   now be recovered exactly via [`text_slice`](quotes::text_slice) or
//!   straight out of the match string instead (see
//!   [`range_is_verbatim_or_synthesized`](macros::image::range_is_verbatim_or_synthesized)).
//! - [`apply_character_replacements`] recognizes [character replacements] —
//!   `(C)`, `--`, `...`, arrows, apostrophes, and restored entities — replacing
//!   each with a [`CharRef::Replacement`](crate::inlines::CharRef::Replacement)
//!   or [`CharRef::Entity`](crate::inlines::CharRef::Entity) leaf. It reuses
//!   the shared
//!   [`character_replacements`](crate::content::character_replacements) rules
//!   and, like the string step, matches over the *escaped* text so an arrow
//!   (`-&gt;`) or entity (`&amp;copy;`) can straddle a `Text`/`CharRef`
//!   boundary.
//! - [`apply_macros`] recognizes **image and icon macros** (`image:target[…]`,
//!   `icon:target[…]`), the **UI macros** (`kbd:[…]`, `btn:[…]`, `menu:…[…]`),
//!   the **`link:`/`mailto:` macro** (`link:target[…]`, `mailto:addr[…]`),
//!   **auto-links and formal-URL links** (`https://example.org`,
//!   `https://example.org[text]`), **bare e-mail addresses**
//!   (`doc@example.org`), **cross-references** in both the
//!   `xref:` macro form (`xref:id[text]`) and the `<<id>>` shorthand, and
//!   **inline anchors** (`[[id]]`, `[[id,reftext]]`, `anchor:id[reftext]`) and
//!   **index terms** (`((term))`, `(((primary, secondary)))`, `indexterm:[…]`,
//!   `indexterm2:[…]`), replacing each with an [`Image`](InlineNode::Image),
//!   [`Ui`](InlineNode::Ui), [`Ref`](InlineNode::Ref),
//!   [`Anchor`](InlineNode::Anchor), or [`IndexTerm`](InlineNode::IndexTerm)
//!   node. An image node
//!   captures its own owned [`Attrlist`] — the step that makes a macro node
//!   *self-describing*; a link or cross-reference node bakes its computed display
//!   text into [`Text`](InlineNode::Text) children so its fold needs no
//!   build-time state. A link whose display text carried its own attribute list
//!   (`link:x[text,role=hl]`) likewise carries the parsed
//!   [`Attrlist`] on [`Ref::attrs`](crate::inlines::Ref::attrs)
//!   — needed because `render_link` reads more out of it than `roles`/`window`
//!   alone (an `id`, a `title`, the `nofollow`/`noopener` options) — recognized
//!   whenever that text has no embedded newline, so it can be sliced honestly
//!   from `'src` rather than a synthesized copy; a `mailto:` text's own
//!   comma-delimited subject/body is encoded straight into the target, the same
//!   way. A bare address needs no attribute list at all — it folds through the
//!   same `render_link` with a `mailto:` target and the address as its display
//!   text — so, like an anchor's id, it is recognized even inside a
//!   [`synthesized`](quotes::Piece::synthesized) run; an address carrying an
//!   escaped `&`, or one abutting an already-recognized construct (whose
//!   *rendered* last character the pattern's mismatch-prefix group reads and a
//!   placeholder hides — the same boundary the auto-link family's own required
//!   prefix already enforces), is deferred. Each family reuses the shared
//!   pattern the string step
//!   matches with ([`INLINE_IMAGE_MACRO`](crate::content::INLINE_IMAGE_MACRO), [`INLINE_KBD_BTN_MACRO`](crate::content::INLINE_KBD_BTN_MACRO),
//!   [`INLINE_MENU_MACRO`](crate::content::INLINE_MENU_MACRO), [`INLINE_LINK_MACRO`](crate::content::INLINE_LINK_MACRO), [`INLINE_LINK`](crate::content::INLINE_LINK),
//!   [`INLINE_EMAIL`](crate::content::INLINE_EMAIL),
//!   [`INLINE_XREF`](crate::content::INLINE_XREF), [`INLINE_ANCHOR`](crate::content::INLINE_ANCHOR), [`INLINE_INDEXTERM`](crate::content::INLINE_INDEXTERM)), builds
//!   `'src`-borrowing nodes for verbatim macros only (see [`apply_macros`] for
//!   the boundary the escaped-content case defers), and — for the UI macros — is
//!   recognized only under the `experimental` document attribute, exactly as the
//!   string step gates them. The cross-reference pass claims the same-document,
//!   inter-document, and document-as-a-whole target forms in both spellings
//!   (`xref:id[text]` and `<<id>>` / `<<id,text>>`), and the `xref:` macro's own
//!   attribute-list text, so what a cross-reference defers is decided entirely
//!   by the gate that precedes both builders — and that gate now admits a
//!   [`synthesized`](quotes::Piece::synthesized) run, since nothing on a
//!   `Ref{Xref}` node is `Span`-typed (its own attribute list is parsed from a
//!   normalized copy, never a source slice), so a cross-reference inside an
//!   expanded attribute value is recognized with its exact target and text.
//!   That same gate also admits an **escaped special** for the
//!   cross-reference family (`xref:sec[a<b]`, `<<sec,Tom & Jerry>>`) and for
//!   the **`link:`/`mailto:` macro** (`link:a&b.html[]`,
//!   `link:index.html[a < b]`): the
//!   level's match string carries a
//!   [`CharRef`](InlineNode::CharRef)`::Special`'s canonical entity, the very
//!   bytes the string replacer's own escaped haystack holds there, so every
//!   value these families read off that string is what the string replacer read.
//!   The display text then becomes **structured children** rather than one
//!   sliced or baked `Text` — the special stays the `CharRef` it already is,
//!   folding back to the same entity instead of being escaped twice — which for
//!   a *bare* link macro covers the target-derived text it shows as well. The
//!   one capture still holding that boundary in either family is a display text
//!   carrying an attribute list, parsed as a real
//!   [`Attrlist`]`<'src>` from the source's own
//!   bytes rather than the escaped copy the replacer parses.
//!   A display/reference text crossing a rendered *span*
//!   (`link:x[*bold*]`, `xref:id[*bold*]`, `<<id,*bold*>>`) remains deferred for
//!   every reference-bearing family: a span is one opaque placeholder here
//!   where the string pipeline's haystack holds its markup inline, and that
//!   markup — which only exists at fold time — is what the replacer's own
//!   attribute-list `=` probe and the pattern's `]` boundary read. An anchor
//!   renders from its id alone, so it is recognized whenever that id does not
//!   cross a rendered span — its character class rules out an escaped special
//!   entirely. Unlike every other reference-bearing family, an anchor is now
//!   recognized even when its id sits inside a
//!   [`synthesized`](quotes::Piece::synthesized) run (an attribute expansion,
//!   or — reached at a tree's root — a filtered multi-line block's own joined
//!   seed): [`text_slice`](quotes::text_slice) recovers the id's exact text
//!   there, with only the node's `location` falling back to the coarse
//!   enclosing span (design §4.4). A non-verbatim reference text — which does
//!   not reach the flow — still just leaves the node's `reftext` unpopulated
//!   without deferring the whole anchor.
//!   Likewise a *concealed* index term (`indexterm:[…]`, `(((…)))`) renders
//!   nothing, so it too is always recognized; a *visible* term (`indexterm2:[…]`,
//!   `((term))`) is deferred only when its shown text crosses a rendered span or
//!   carries an attribute list. The **UI**, **index-term**, and
//!   **cross-reference** families share the anchor's synthesized-run lift, and
//!   for the same reason — none of those nodes carries a `Span`-typed field, so
//!   a `kbd:`/`btn:`/`menu:` macro, an index term, or a cross-reference inside
//!   an expanded attribute value is recognized with its exact text (only the
//!   node's `location` taking design §4.4's coarse fallback).
//! - [`apply_footnotes`] recognizes **footnotes** (`footnote:[…]`,
//!   `footnote:id[…]`, `footnote:id[]`), replacing each with a
//!   [`Footnote`](InlineNode::Footnote) node, folding through the shared
//!   [`INLINE_FOOTNOTE_MACRO`](crate::content::INLINE_FOOTNOTE_MACRO) pattern
//!   like every other family. It is its **own step** in [`build`], run once
//!   over the whole tree *after* [`apply_macros`] has resolved every other
//!   family at every level, rather than a level pass inside [`apply_macros`] —
//!   because it is the one macro family whose recognition performs a *required*
//!   side effect: a footnote's marker digits are the number
//!   [`Parser::define_footnote`] / [`Parser::footnote_index_for_id`] assign, so
//!   numbering must follow true left-to-right source order regardless of
//!   nesting depth, which [`apply_macros`]'s depth-first child recursion does
//!   not guarantee (see [`apply_footnotes`]'s doc comment). Registering the
//!   number cannot be deferred to the cutover the way every other family's
//!   catalog/warning side effect is, without breaking output parity (see
//!   `build_footnote_node`). Its content becomes structured children via
//!   [`emit_range`](quotes::emit_range) rather than a literal attribute value,
//!   so — unlike the other families — a content crossing an already-recognized
//!   construct is not deferred: nesting is the point. The deprecated
//!   `footnoteref:[id,text]` / `footnoteref:[id]` form
//!   (`build_footnoteref_node`) is recognized too, splitting its one bracket on
//!   the first comma rather than taking an id from the macro target; only its
//!   own deprecation warning (a diagnostic, deferred to the cutover like every
//!   other family's) and a content carrying an escaped closing bracket (`\]`)
//!   remain deferred. The bibliography-anchor form is a later increment.
//! - [`apply_stem`] recognizes **inline STEM macros** (`stem:[…]`,
//!   `asciimath:[…]`, `latexmath:[…]`), replacing each with a
//!   [`Stem`](InlineNode::Stem) leaf. Like [`apply_passthroughs`], it is an
//!   implicit-passthrough step: it runs immediately after
//!   [`apply_passthroughs`] (mirroring `Passthroughs::extract_from`'s own
//!   ordering, which extracts STEM macros last, after both passthrough passes,
//!   *specifically so a nested passthrough placeholder survives*), so a STEM
//!   expression's content is never touched by specialcharacters, quotes,
//!   replacements, or macros. It reuses the shared
//!   [`INLINE_STEM_MACRO`](crate::content::INLINE_STEM_MACRO) pattern and
//!   [`stem_notation`](crate::content::stem_notation) helper, so only the
//!   recognition *sink* differs. Because it runs right after
//!   [`apply_passthroughs`], the only node kinds it can ever see are `Text` and
//!   [`Raw`](InlineNode::Raw); a match embedding an already-extracted `Raw`
//!   passthrough (`stem:[+++<b>x</b>+++]`) is never deferred — the `Raw` is
//!   spliced into the node's value verbatim, unlike every other macro family's
//!   "crosses an already-recognized construct" boundary. A macro carrying an
//!   explicit substitution list (`stem:c,q[…]`) is recognized too: the list
//!   resolves to a [`SubstitutionGroup`] the expression runs through in place
//!   of the bare macro's [`SubstitutionGroup::Stem`] — the same
//!   `passthrough_text`-through-the-real-pipeline treatment that resolves the
//!   analogous `pass:c,q[…]` form (see [`apply_passthroughs`]'s doc comment),
//!   except a `Stem` node already has a single `value` field to hold the
//!   result, so no richer subtree is needed.
//! - [`apply_post_replacements`] turns a trailing ` +` at the end of a line
//!   into a [`LineBreak`](InlineNode::LineBreak) leaf. Under the block-wide
//!   `hardbreaks` option — the enclosing block's own `attrlist`, or the
//!   document's `hardbreaks-option` attribute — it instead turns *every* line
//!   ending into a break, stripping a redundant trailing ` +` rather than
//!   doubling it, mirroring [`apply_post_replacements`]'s own string-pipeline
//!   counterpart exactly.
//! - [`fold_html`] folds the resulting leaves and spans back to output bytes
//!   through an
//!   [`InlineSubstitutionRenderer`](crate::parser::InlineSubstitutionRenderer)
//!   — the first fold over the *public* [`InlineNode`] tree (the recorder's
//!   `fold_into` folds an intermediate representation, not the public tree).
//!
//! [quoted text]: https://docs.asciidoctor.org/asciidoc/latest/subs/quotes/
//! [character replacements]:
//!     https://docs.asciidoctor.org/asciidoc/latest/subs/replacements/
//!
//! This module is now **the production tree source**: when inline-tree
//! building is enabled ([`Parser::with_inline_tree`](crate::Parser)),
//! `SubstitutionGroup::apply` calls [`build_for_group`] — the group-aware
//! entry point mirroring its own `run_pipeline` step selection — over the
//! pre-substitution content value, against a counter-safe clone of the
//! parser, and stores the result on
//! [`Content::inlines`](crate::content::Content::inlines). This replaced the
//! Strategy-A recorder (`content::inline_tree`, now test-only oracle
//! machinery) as the design's step 6 tree-source swap. The authoritative
//! rendered string is still produced by the string pipeline; making
//! `rendered_html()` a fold of this tree — and deleting the three production
//! sentinel systems — is the remaining half of the cutover. A form documented
//! as deferred elsewhere in this module is left as literal text in the tree
//! (never a wrong node), so the fold-parity guarantee is scoped to the
//! claimed vocabulary.
//!
//! # Staging the cutover's recognition side effects
//!
//! Every macro family above deliberately skips a **recognition side effect**
//! the string pipeline performs at the same point — registering an id, link,
//! or image target in the document catalog, or recording a warning — because
//! the builder still runs *alongside* the authoritative string pipeline
//! (against a counter-safe clone of the parser, whose mutations are
//! discarded), so performing one here today would double-count the
//! registration the string pipeline already performs. Each family's own
//! deferred side effects are staged as their own reviewable building block —
//! `register_image` and the `link=` dangerous-scheme/self-href warning for
//! `image:`/`icon:`, `register_link` for the five link forms, and the
//! `register_ref` pair for anchors and id-carrying attributed spans — and
//! [`apply_macro_side_effects`] composes all three, in the string pipeline's
//! own family-pass order, into the single call the cutover's remaining half
//! makes exactly once per parse (design §5.2, Phase 4 step 6): once
//! `rendered_html()` is a fold of this tree, the string pipeline no longer
//! performs the registrations and this call takes over. Until then it is
//! exercised only by its own tests (and its constituents' own), against
//! their own `Parser`.
//!
//! # A note on quote nesting
//!
//! The string pipeline realizes nesting by running the ordered [`quote_subs`]
//! over one growing string: an earlier sub renders `<strong>…</strong>` and a
//! later sub matches around (or inside) it. [`apply_quotes`] reproduces that by
//! applying each sub to the node tree in turn and, before matching at a level,
//! descending into the [`Styled`](crate::inlines::Styled) spans earlier subs
//! created — so `*a `b` c*` (strong containing a later-recognized monospace)
//! and `*a _b_ c*` nest correctly. A [`Styled`](crate::inlines::Styled) span an
//! earlier sub produced is otherwise **opaque** to a later sub at the same
//! level (represented by a single placeholder while matching), which is where
//! this single-pass recognition can, in principle, diverge from the string
//! pipeline's match-through-rendered-tags behavior for pathological cross-span
//! inputs; the differential corpus (§5.3) pins the cases this increment claims.
//!
//! [`Text`]: InlineNode::Text
//! [`quote_subs`]: crate::content::quote_subs

// The fold ([`fold_html`]) and the staged recognition side effects
// ([`apply_macro_side_effects`] and its constituents) are consumed only by
// tests until the authoritative-fold half of the cutover wires them into
// `Content` (design §5.2, Phase 4 step 6).
#![allow(dead_code)]

mod attribute_refs;
mod callouts;
mod char_replacements;
mod fold;
mod footnotes;
mod macros;
mod passthrough_step;
mod post_replacements;
mod quotes;
mod special_chars;
mod stem_step;

#[cfg(test)]
mod test_support;

use attribute_refs::apply_attribute_references;
use callouts::apply_callouts;
use char_replacements::apply_character_replacements;
// Consumed only by `cfg(test)` callers and future external callers until the
// authoritative-fold half of the cutover wires it into `Content`.
#[allow(unused_imports)]
pub(crate) use fold::fold_html;
use footnotes::apply_footnotes;
// Staged for the eventual authoritative-fold half of the cutover (see this
// module's own doc comment); not yet called from any real parse path, so —
// unlike the step re-exports below — reachable only via `cfg(test)` callers
// and future external callers today.
#[allow(unused_imports)]
pub(crate) use macros::apply_macro_side_effects;
use macros::apply_macros;
use passthrough_step::apply_passthroughs;
use post_replacements::apply_post_replacements;
use quotes::apply_quotes;
use special_chars::{apply_special_characters, classify_unescaped_specials};
use stem_step::apply_stem;

use crate::{
    Parser, Span,
    attributes::Attrlist,
    content::{SubstitutionGroup, SubstitutionStep},
    inlines::InlineNode,
    strings::CowStr,
};

/// Builds the inline tree for `source` in a single forward pass.
///
/// The tree is seeded as one borrowed whole-source [`Text`](InlineNode::Text)
/// node and refined by each substitution step in turn. `source` is the exact
/// text to process, so a caller controls precisely what is built.
///
/// This is the common-case wrapper around [`build_from_value`] for a `source`
/// that is itself the verbatim seed (the overwhelming majority of content,
/// per [`Content::from_filtered_lines`](crate::content::Content::from_filtered_lines)'s
/// own single-surviving-line fast path); see `build_from_value` for the
/// filtered/joined case a real block's content can also produce.
///
/// `parser` is consulted to parse the attribute list of an attributed quote
/// (`[.role]#…#`) and to read the document-wide `hardbreaks-option`
/// attribute; a caller with no document context can pass a default
/// [`Parser`]. `attrlist` is the enclosing block's own attribute list (for
/// its `hardbreaks` option, [`apply_post_replacements`]'s only consumer of
/// it today); a caller with no block context can pass `None`.
pub(crate) fn build<'src>(
    source: Span<'src>,
    parser: &Parser,
    attrlist: Option<&Attrlist<'src>>,
) -> Vec<InlineNode<'src>> {
    build_from_value(CowStr::from(source.data()), source, parser, attrlist)
}

/// Builds the inline tree from an already-computed content **value**,
/// anchored at `location` — the single-pass counterpart of the `(value,
/// location)` pair [`Content`](crate::content::Content) itself is built from
/// (see [`Content::from_filtered`](crate::content::Content::from_filtered) /
/// [`Content::from_filtered_lines`](crate::content::Content::from_filtered_lines)).
///
/// `location` seeds every downstream construct's coarse-fallback span (design
/// §4.4) and is threaded through as each step's own `root`/`source`
/// parameter, so it must be a `Span` that contains — and is used consistently
/// with — the source bytes `value` was ultimately derived from.
///
/// Two cases fall out of the relationship between `value` and `location`,
/// exactly mirroring the verbatim/synthesized split
/// [`apply_special_characters`]'s own [`split_text`](special_chars) already
/// makes for a single node deeper in the tree:
///
/// - **`value` is exactly `location.data()`** (the common case — a plain
///   paragraph, or any content
///   [`from_filtered_lines`](crate::content::Content::from_filtered_lines)'s
///   single-surviving-line fast path borrowed unmodified): every node built
///   from it gets an honest, precise `'src` span, sliced straight from
///   `location` (issue #944).
/// - **`value` differs from `location.data()`** (a multi-line block whose
///   surviving lines were joined with `\n`, or any other filtered value with no
///   single contiguous `'src` slice of its own): the seed is *synthesized* —
///   every node the pipeline builds from it still gets a real `InlineNode`, but
///   one whose `location` coarsely falls back to the whole of `location`
///   (design §4.4's documented migration-stage policy), the same fallback an
///   attribute-reference expansion or a `counter` directive's resolved value
///   already receives. This is what lets the single-pass builder process a
///   real, multi-line, filtered block's content — not just a single contiguous
///   source span — while every already-landed step's shared
///   [`build_match_string`](quotes::build_match_string) /
///   [`source_slice`](quotes::source_slice) machinery handles the fallback with
///   no further change: it already treats *any* non-verbatim `Text` node this
///   way, regardless of where in the tree — or how early — that node was
///   produced.
pub(crate) fn build_from_value<'src>(
    value: CowStr<'src>,
    location: Span<'src>,
    parser: &Parser,
    attrlist: Option<&Attrlist<'src>>,
) -> Vec<InlineNode<'src>> {
    build_for_group(
        &SubstitutionGroup::Normal,
        value,
        location,
        parser,
        attrlist,
    )
}

/// Builds the inline tree from an already-computed content **value** under the
/// substitution group that governs the content — the group-aware counterpart
/// of [`build_from_value`], mirroring the step selection
/// [`SubstitutionGroup::apply`](crate::content::SubstitutionGroup)'s own
/// `run_pipeline` makes for the string.
///
/// The mapping reproduces `run_pipeline` exactly:
///
/// - Passthrough (and, with it, inline-STEM) extraction runs first, ahead of
///   every step, **iff** the group's steps include
///   [`Macros`](SubstitutionStep::Macros) or the group is
///   [`Header`](SubstitutionGroup::Header) — the same condition `run_pipeline`
///   gates `Passthroughs::extract_from` on.
/// - Each of the group's [`steps()`](SubstitutionGroup::steps) then runs in the
///   group's own order, each recast as its node transducer. The
///   [`Macros`](SubstitutionStep::Macros) step runs [`apply_macros`] and then
///   [`apply_footnotes`] — footnotes are part of the string pipeline's macros
///   step, but are their own transducer here so numbering follows true source
///   order (see [`apply_footnotes`]'s doc comment).
///
/// A group whose steps are empty ([`Pass`](SubstitutionGroup::Pass) /
/// [`None`](SubstitutionGroup::None), or an empty custom list) yields the
/// untouched seed: a single [`Text`](InlineNode::Text) node holding `value`,
/// exactly as the string pipeline leaves such content's text unchanged. Empty
/// content is seeded with **no** node at all rather than an empty one: a leaf
/// carrying nothing is not something a consumer should have to skip, and the
/// steps would have nothing to do with it anyway. (The distinction matters
/// because an empty `Text` node *elsewhere* in the tree is meaningful — a
/// `<<id,>>` cross-reference's present-but-empty reference text is exactly one
/// — so the steps themselves preserve one where they find it.)
pub(crate) fn build_for_group<'src>(
    group: &SubstitutionGroup,
    value: CowStr<'src>,
    location: Span<'src>,
    parser: &Parser,
    attrlist: Option<&Attrlist<'src>>,
) -> Vec<InlineNode<'src>> {
    let steps = group.steps();

    if value.is_empty() {
        return vec![];
    }

    let mut nodes = vec![InlineNode::Text { value, location }];

    // Passthroughs are extracted before every other step (mirroring
    // `Passthroughs::extract_from`, which the string pipeline runs ahead of
    // its own step loop, under the same group condition), so their content is
    // never touched by specialcharacters, quotes, replacements, or macros.
    if steps.contains(&SubstitutionStep::Macros) || group == &SubstitutionGroup::Header {
        nodes = apply_passthroughs(nodes, location, parser);

        // Inline STEM is an implicit passthrough too, extracted last
        // (mirroring `Passthroughs::extract_from`'s own ordering) so a
        // passthrough placeholder nested inside a STEM expression survives.
        nodes = apply_stem(nodes, location, parser);
    }

    for step in steps {
        nodes = match step {
            SubstitutionStep::SpecialCharacters => apply_special_characters(nodes),

            SubstitutionStep::Quotes => apply_quotes(nodes, location, parser),

            // In the *normal* effective order (specialcharacters → quotes →
            // attributes → replacements → macros, design §3.4.1), by this
            // point `<`/`>`/`&` are already `CharRef` leaves and quoted spans
            // are already `Styled` nodes, and whatever this step splices in
            // is exactly what the steps still ahead see and refine.
            SubstitutionStep::AttributeReferences => {
                apply_attribute_references(nodes, location, parser)
            }

            SubstitutionStep::CharacterReplacements => {
                apply_character_replacements(nodes, location)
            }

            SubstitutionStep::Macros => {
                // Footnotes are their own transducer, run once over the
                // *whole* tree after every other macro family has been
                // resolved at every level — see `apply_footnotes`'s doc
                // comment for why this cannot be folded into `apply_macros`
                // as an ordinary level pass.
                let nodes = apply_macros(nodes, location, parser);
                apply_footnotes(nodes, location, parser)
            }

            SubstitutionStep::PostReplacement => {
                apply_post_replacements(nodes, location, parser, attrlist)
            }

            SubstitutionStep::Callouts => apply_callouts(nodes, location, parser, attrlist),
        };
    }

    // Design §3.4.1: a literal `<`/`>`/`&` that no `SpecialCharacters` step
    // ever acted on is emitted verbatim by the string pipeline, so it is a
    // `Raw` leaf here rather than the `Text` run the fold would escape. This
    // runs last, after the group's own steps, so every transducer still sees
    // the specials as ordinary text — exactly as the string pipeline's steps
    // do under such an order.
    if !steps.contains(&SubstitutionStep::SpecialCharacters) {
        nodes = classify_unescaped_specials(nodes);
    }

    nodes
}

/// A whole-pipeline differential corpus: [`build`] against the *real*,
/// public [`SubstitutionGroup::apply`] entry point, over fixtures that
/// **combine** several construct families in one piece of content.
///
/// Every other differential corpus in this module (each landed alongside its
/// own step, e.g. [`test_support::golden_macros`]) hand-chains only the
/// [`SubstitutionStep`]s that step's own increment covers, skipping
/// `AttributeReferences` unless the fixture needs it, and never runs
/// passthrough extraction/restore or deferred cross-reference finalization
/// alongside the other steps. That is enough to pin each step in isolation,
/// but it never exercises the *fully assembled* pipeline
/// [`SubstitutionGroup::Normal.
/// apply`](crate::content::SubstitutionGroup::apply) runs in production —
/// passthrough/STEM extraction, every step in true order, passthrough restore,
/// and deferred-reference finalization, all against one `Content` — which is
/// exactly what [`build`] (this module's own single call) must reproduce once
/// the cutover (design §5.2, Phase 4 step 6) wires it in. This closes that gap:
/// each fixture below mixes constructs that were previously verified only in
/// separate, single-family corpora (quotes nested around an attribute
/// reference, a footnote whose text itself carries an attribute reference, a
/// passthrough beside a macro, a counter directive beside a formatted span, …),
/// so a boundary-crossing interaction between two steps that individually pass
/// would still be caught here.
///
/// As with every other corpus in this module, a fixture is chosen to stay
/// inside the vocabulary [`build`] already covers — it avoids the forms still
/// documented as deferred elsewhere in this module (e.g. an attribute value
/// that itself embeds a construct `CharacterReplacements`/`Macros` would
/// recognize).
#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::{build, build_from_value};
    use crate::{
        Parser, Span,
        content::{Content, SubstitutionGroup, inline_builder::fold_html},
        parser::{HtmlSubstitutionRenderer, ModificationContext},
        strings::CowStr,
    };

    /// Runs `source` through the real, public `SubstitutionGroup::Normal`
    /// pipeline, exactly as a real block's content is substituted in
    /// production.
    fn golden(source: &str, parser: &Parser) -> String {
        let mut content = Content::from(Span::new(source));
        SubstitutionGroup::Normal.apply(&mut content, parser, None);
        content.rendered_str().to_string()
    }

    /// Builds and folds the single-pass tree for `source` — the same [`build`]
    /// a real cutover would call — through the built-in HTML renderer.
    fn built(source: &str, parser: &Parser) -> String {
        let nodes = build(Span::new(source), parser, None);
        fold_html(&nodes, &HtmlSubstitutionRenderer {}, parser)
    }

    /// Asserts that `source` folds identically whether taken through the real
    /// production pipeline or through the single-pass builder, under a
    /// document configured by `configure`.
    ///
    /// `configure` is called once per side rather than sharing one `Parser`
    /// between them: `golden`'s `SubstitutionGroup::apply` and `built`'s
    /// `build` each advance document counters (footnote numbers,
    /// `{counter:...}` values) for real, so sharing a parser would double
    /// them. Two independently-built parsers, identically configured, see
    /// the same fixture in the same left-to-right order and so stay in
    /// lockstep — the same two-independent-parsers discipline this module's
    /// other differential corpora already use (see e.g. `footnotes.rs`).
    fn assert_parity_with(source: &str, configure: impl Fn() -> Parser) {
        assert_eq!(
            golden(source, &configure()),
            built(source, &configure()),
            "fold diverged from the real pipeline for {source:?}"
        );
    }

    fn assert_parity(source: &str) {
        assert_parity_with(source, Parser::default);
    }

    /// A broad, general-purpose sweep of inline fixtures — reusing the shape
    /// of the Phase 1 byte-parity corpus
    /// ([`NORMAL_CORPUS`](crate::tests::inline_recorder)) that pins the
    /// Strategy-A recorder — run against `build` + `fold_html` instead. Unlike
    /// the hand-picked combined-constructs corpus above, this one is not
    /// curated to stay inside `build`'s claimed vocabulary, so it is exactly
    /// the kind of audit that caught the `hardbreaks` cutover blocker (see the
    /// design doc's own "step 6 prep" note on that increment): a fixture here
    /// that turns out to diverge is either a real, previously-unknown blocker
    /// (fix it) or a form some other increment already documents as deferred
    /// (drop it from this sweep and make sure it has its own divergence test
    /// instead — e.g. a link/xref display text crossing a rendered span).
    #[test]
    fn fold_matches_the_real_pipeline_across_a_broad_general_purpose_sweep() {
        let fixtures = [
            "",
            "plain text with no constructs",
            "text with trailing spaces   and   runs",
            "a < b && c > d",
            "1 < 2 & 3 > 0",
            "<>&",
            "One *word* is strong.",
            "An _emphasized_ phrase.",
            "Some `monospaced` text.",
            "A #highlighted# span.",
            "H~2~O and E = mc^2^.",
            "A bold *phrase of text* here.",
            "Bold c**hara**cter**s** in a word.",
            "un__frac__tured emphasis",
            "a##b##c",
            "*a _b_ c*",
            "*bold _and italic_ mix*",
            "_*strong inside emphasis*_",
            "[.myrole]#roled text#",
            "[#anchor]#anchored#",
            "[.a.b]#multi role#",
            "*a < b* and _c > d_",
            "code `x < y && z` here",
            "\"`double`\" and '`single`' quotes",
            "(C) (R) (TM)",
            "An em -- dash and an ellipsis...",
            "arrows -> => <- <=",
            "Sam's apostrophe",
            "named &amp; numeric &#8217; entities",
            "plain {backend} attribute",
            "See https://example.org for details.",
            "An angle-bracketed <https://example.org> link.",
            "<https://example.org[the site] keeps its bracket.",
            "unterminated <https://example.org stays literal",
            "A link:https://example.org[example] link.",
            "mailto:a@b.com[email me]",
            "write to doc.writer@example.com today",
            "**bold**doc@example.com",
            "link:index.html[Docs]doc@example.com",
            "an image:photo.png[Alt Text] inline",
            "image:pic.png[Scaled,200,100]",
            "see <<target>> for more",
            "see <<target,the target>> now",
            "see <<target,Tom & Jerry>> now",
            "xref:other.adoc#frag[Other] doc",
            "xref:target[a < b & c] doc",
            "link:a&b.html[x] macro",
            "link:index.html[Tom & Jerry] macro",
            "mailto:a&b@example.org[] address",
            "A claim.footnote:[the evidence]",
            "Named.footnote:disc[a discussion] then footnote:disc[].",
            "A claim.footnote:[the *strong* evidence and a link:https://e.org[source]]",
            "Two notes.footnote:[first] and again.footnote:[second]",
            "Press kbd:[Ctrl+T] now.",
            "Press kbd:[Ctrl,Shift,N] now.",
            "Click btn:[Save] please.",
            "Choose menu:File[Save As > PDF].",
            "A flow ((visible term)) here.",
            "A concealed (((primary,secondary))) term.",
            "[[the-anchor]]Anchored paragraph.",
            "first line +\nsecond line",
            "*bold* _em_ `code` (C) https://x.y[link] <<ref>> image:i.png[i]",
            "A mix of {backend}, *bold < text*, and a footnote:[with `code`].",
            r"a \*not bold* b",
            r"an \_not emphasized_ here",
            r"literal \`backtick\` text",
            "*a _b `c` d_ e*",
            "*one* *two* *three*",
            "_a_ and _b_ and _c_",
            "`code with *stars* inside`",
            "pre**mid**post and a__b__c",
            "x^sup^y and p~sub~q",
            "[.role1.role2#the-id]#decorated#",
            "[#only-id]#text#",
            "`a < b > c & d`",
            "(C) then (R) then (TM) end",
            "First... then -- and -> arrows <-",
            "a &amp; b and &#8482; and &copy;",
            "an {undefined-attr} reference",
            "link:https://example.org[Example,role=external,window=_blank]",
            "https://example.org[Example^]",
            "a https://example.org[] bare-macro link",
            "visit https://example.org/path?q=1 now",
            "image:a.png[An image with spaces,role=thumb]",
            "before image:b.svg[Vector] after",
            "<<a>> and <<b>> and <<c,C text>> and <<d,>>",
            "a ((flow term)) and (((c1, c2, c3))) end",
            "indexterm:[primary, secondary]",
            "indexterm2:[shown]",
            "[[mid-anchor]] after the anchor",
            "text before anchor:named[Ref Text] and after",
            "a +++<b>raw</b>+++ passthrough",
            "inline pass:[<i>x</i>] macro",
            "math $$a < b$$ here",
            "a +literal *stars*+ b",
            "line one +\nline two +\nline three",
            "stem:[a < b] expression",
            "asciimath:[a < b] inline",
            "latexmath:[x < y] inline",
            "an icon:home[] icon",
            "icon:star[2x,role=gold] rated",
            "   ",
            "a\nb\nc",
        ];

        for fixture in fixtures {
            assert_parity(fixture);
        }
    }

    #[test]
    fn fold_matches_the_real_pipeline_across_combined_constructs() {
        let with_product = || {
            Parser::default().with_intrinsic_attribute(
                "product",
                "Widget",
                ModificationContext::Anywhere,
            )
        };

        // Quotes wrapping an attribute reference: the quotes step matches
        // `*...*` before the reference expands, so the splice must still
        // reach inside the already-built `Styled` span.
        assert_parity_with("The {product} is *fast* and reliable.", with_product);

        // A cross-reference and a footnote in the same sentence, the
        // footnote's own text carrying a nested attribute reference.
        assert_parity_with(
            "See <<intro>> for details.footnote:[Also check the {product} docs.]",
            with_product,
        );

        // A delimited passthrough beside a quoted span and an image macro.
        assert_parity("+++<u>raw</u>+++ combined with *bold* and image:foo.png[Alt Text].");

        // Inline STEM beside a quoted span and a character replacement.
        assert_parity("Equation stem:[x^2+y^2=z^2] appears in *bold* text with (C) 2024.");

        // UI macros (kbd/menu, gated on `experimental`) beside a quoted span.
        let with_experimental = || {
            Parser::default().with_intrinsic_attribute_bool(
                "experimental",
                true,
                ModificationContext::Anywhere,
            )
        };

        assert_parity_with(
            "kbd:[Ctrl+Alt+Del] opens the *Task Manager* via menu:File[Save].",
            with_experimental,
        );

        // A menu's `&gt;` submenu form beside a quoted span and a character
        // replacement — its caret delimiters are escaped specials, so this
        // crosses the relaxed gate with the other families live.
        assert_parity_with(
            "Choose menu:View[Zoom > Reset] in the *Task Manager* (C) 2024.",
            with_experimental,
        );

        // Several link forms and a cross-reference in one sentence.
        assert_parity(
            "Visit https://example.org[the site] or mailto:a@example.org[email us], \
             then see <<conclusion,the conclusion>>.",
        );

        // Both cross-reference spellings carrying an *escaped special* in
        // their reference text, beside a quoted span and a live attribute
        // reference: the text becomes structured children (a `CharRef` between
        // two `Text` runs) with the other families running around it.
        assert_parity_with(
            "See xref:intro[Tom & Jerry] and <<intro,1 < 2>> about *{product}*.",
            with_product,
        );

        // Both shorthand cross-reference bodies in one sentence — a
        // present-but-empty reference text (`<<id,>>`, an empty `<a>…</a>`)
        // beside one with no text at all (`<<id>>`, the bracketed fallback) —
        // with the other families live around them.
        assert_parity_with(
            "See <<intro,>> and <<intro>> about *{product}*, or <<intro,the intro>>.",
            with_product,
        );

        // All three link-recognizing passes in one sentence, the bare address
        // last (its own pass runs after both URL-link families).
        assert_parity(
            "Visit https://example.org or link:docs.html[the docs], \
             or just write to doc@example.org.",
        );

        // A `link:`/`mailto:` macro carrying an *escaped special* in its
        // target and in its display text, beside a quoted span and a live
        // attribute reference: the display text becomes structured children (a
        // `CharRef` between two `Text` runs) while a bare macro's
        // target-derived text is unescaped back to logical text, with the
        // other families running around them.
        assert_parity_with(
            "See link:a&b.html[Tom & Jerry] and link:c&d.html[] \
             or mailto:x&y@example.org[] about *{product}*.",
            with_product,
        );

        // Every spelling of the URL-link family, including both ANGLE-branch
        // forms, beside a quoted span and a character replacement — the
        // delimiters of the angle forms are escaped specials, so this crosses
        // the relaxed verbatim gate with the other families live.
        assert_parity(
            "See <https://example.org> and <https://example.org/docs[the docs] \
             beside *bold* text and (C) 2024, plus https://plain.example.",
        );

        // A bare e-mail address spliced in by an attribute reference — a
        // *synthesized* run, which the e-mail family recognizes exactly the
        // way the anchor family does (see `links.rs`'s own note). The real
        // `AttributeReferences` step runs here, unlike the family-scoped
        // corpora.
        assert_parity_with("Write to {contact} about *{product}*.", || {
            Parser::default()
                .with_intrinsic_attribute(
                    "contact",
                    "doc@example.com",
                    ModificationContext::Anywhere,
                )
                .with_intrinsic_attribute("product", "Widget", ModificationContext::Anywhere)
        });

        // A `counter` directive beside a quoted span carrying an attribute
        // reference and a STEM expression — the ordering fix documented in
        // this module's `attribute_refs.rs` follow-up note.
        assert_parity_with(
            "{counter:step}. Step one uses *{product}* and stem:[x+1].",
            with_product,
        );

        // An inline anchor, a quoted span, an index term, and an attribute
        // reference together.
        assert_parity_with(
            "[[custom-id]]Anchored *text* referencing ((index term)) and {product}.",
            with_product,
        );

        // Escaped constructs (quotes, a character replacement) beside a live
        // attribute reference.
        assert_parity_with(
            r"An escaped \*not bold\* attribute {product} and \(C) not replaced.",
            with_product,
        );

        // Multiple footnotes (numbered in document order) and a
        // cross-reference, one footnote's text carrying an attribute
        // reference.
        assert_parity_with(
            "First footnote:[a {product} note] then footnote:[b unrelated note], \
             and finally <<see-also>>.",
            with_product,
        );

        // The document-wide `hardbreaks-option` attribute breaking every
        // line, one of which carries a quoted span and an attribute
        // reference.
        assert_parity_with(
            "Line one uses *{product}*.\nLine two is plain.\nLine three too.",
            || {
                Parser::default()
                    .with_intrinsic_attribute("product", "Widget", ModificationContext::Anywhere)
                    .with_intrinsic_attribute_bool(
                        "hardbreaks-option",
                        true,
                        ModificationContext::Anywhere,
                    )
            },
        );

        // A bibliography entry: the `^`-anchored bibliography-anchor pass runs
        // ahead of every other macro family, and the bracketed label it leaves
        // in the flow is then seen by those families exactly as the string
        // pipeline's later passes see it.
        let in_bibliography_list_item = || {
            let parser = Parser::default().with_intrinsic_attribute(
                "product",
                "Widget",
                ModificationContext::Anywhere,
            );

            parser.in_bibliography_list_item.set(true);
            parser
        };

        assert_parity_with(
            "[[[gof,GoF]]] Gamma, Erich et al. _Design Patterns_. https://example.org",
            in_bibliography_list_item,
        );

        assert_parity_with(
            "[[[widget]]] The {product} handbook, with a footnote:[note] and an [[extra]] anchor.",
            in_bibliography_list_item,
        );

        // The two `attribute-missing` modes that *remove* content, each with
        // a resolvable reference and other constructs on both the dropped and
        // the surviving lines. These run through the real pipeline, so the
        // builder's own line-drop pass must line up with `apply_attributes`'s
        // line loop across every other step (see `attribute_refs.rs`).
        let missing_mode = |mode: &'static str| {
            move || {
                Parser::default()
                    .with_intrinsic_attribute(
                        "attribute-missing",
                        mode,
                        ModificationContext::Anywhere,
                    )
                    .with_intrinsic_attribute("product", "Widget", ModificationContext::Anywhere)
            }
        };

        assert_parity_with(
            "The *{product}* is here.\n{nope} and image:x.png[X]\nA closing (C) line.",
            missing_mode("drop-line"),
        );

        assert_parity_with(
            "A {nope} in *bold {product}* text.\n{nope}\nlink:docs.html[Docs] {nope} tail.",
            missing_mode("drop"),
        );

        // UI macros and index terms spliced in by attribute references —
        // *synthesized* runs, which those two families now recognize exactly
        // the way the anchor and bare-e-mail families do (neither node carries
        // a `Span`-typed field). The real `AttributeReferences` step runs
        // here, unlike the family-scoped corpora.
        let with_ui_attributes = || {
            Parser::default()
                .with_intrinsic_attribute_bool("experimental", true, ModificationContext::Anywhere)
                .with_intrinsic_attribute("product", "Widget", ModificationContext::Anywhere)
                .with_intrinsic_attribute("view", "View", ModificationContext::Anywhere)
                .with_intrinsic_attribute("key", "Ctrl+T", ModificationContext::Anywhere)
        };

        assert_parity_with(
            "Press kbd:[{key}] then choose menu:{view}[Zoom > Reset] in *{product}*.",
            with_ui_attributes,
        );

        assert_parity_with(
            "The (({product})) index term beside *bold* text and indexterm:[{product}, docs].",
            with_product,
        );

        // Cross-references spliced in by attribute references, in both
        // spellings and in both halves (target and reference text) — the same
        // synthesized-run lift, for the family whose attribute list is parsed
        // from a normalized copy rather than a source slice, so nothing on its
        // node is `Span`-typed.
        let with_xref_attributes = || {
            Parser::default()
                .with_intrinsic_attribute("id", "install", ModificationContext::Anywhere)
                .with_intrinsic_attribute("label", "Install Now", ModificationContext::Anywhere)
                .with_intrinsic_attribute("product", "Widget", ModificationContext::Anywhere)
        };

        assert_parity_with(
            "See *xref:{id}[{label}]* and <<{id},the {product} steps>> plus a (C) mark.",
            with_xref_attributes,
        );

        // Links and images spliced in by attribute references — the last two
        // families to make that lift, and the two that hold a real
        // `Attrlist<'src>`. Their targets and display texts come from the
        // match string like every other family's values; what still defers is
        // an image's non-empty attribute list, a link's attribute-list-bearing
        // display text, and a wholly expanded `link:` macro (each pinned by
        // its own divergence test in `macros/image.rs` and `macros/links.rs`).
        let with_link_attributes = || {
            Parser::default()
                .with_intrinsic_attribute("url", "index.html", ModificationContext::Anywhere)
                .with_intrinsic_attribute("host", "example.org", ModificationContext::Anywhere)
                .with_intrinsic_attribute("label", "Docs", ModificationContext::Anywhere)
                .with_intrinsic_attribute("logo", "sunset.jpg", ModificationContext::Anywhere)
                .with_intrinsic_attribute("product", "Widget", ModificationContext::Anywhere)
        };

        assert_parity_with(
            "Read *link:{url}[{label}]* or visit https://{host}/docs about {product}.",
            with_link_attributes,
        );

        assert_parity_with(
            "See image:{logo}[Logo] and image:{logo}[] beside <https://{host}> and a (C) mark.",
            with_link_attributes,
        );
    }

    /// [`build_from_value`] against the real pipeline, seeded from a
    /// **synthesized** value — one that differs from the `location` span it
    /// is anchored at, the shape `Content::from_filtered_lines` produces for
    /// a genuinely multi-line block once per-line filtering (leading-indent
    /// stripping, in these fixtures) leaves the joined text with no single
    /// contiguous `'src` slice of its own.
    ///
    /// Each fixture pairs an indented multi-line `source` (the `location`,
    /// standing in for the block's real, unfiltered span) with the
    /// corresponding de-indented `filtered` text (the `value`, standing in
    /// for `Content::rendered` after per-line filtering). The golden
    /// comparison is against `filtered` taken through the real pipeline as
    /// ordinary contiguous source — exactly the text a real `Content` would
    /// hold pre-substitution — since that is the logical content being
    /// substituted regardless of which span it is anchored at. This is the
    /// same kind of blocker-audit
    /// `fold_matches_the_real_pipeline_across_a_broad_general_purpose_sweep`
    /// already caught once for `hardbreaks`: it exercises every step through
    /// [`build_from_value`]'s synthesized-seed path, at the tree's *root*
    /// rather than a nested spliced value. A macro construct that needs its
    /// own verbatim `'src` slice (a link/xref/image target or id) is *not*
    /// exercised here — see
    /// `a_macro_construct_is_deferred_when_the_whole_seed_is_synthesized`
    /// below for that documented boundary.
    #[test]
    fn fold_matches_the_real_pipeline_when_seeded_from_a_synthesized_value() {
        let fixtures = [
            ("  plain text", "plain text"),
            ("  a < b && c > d", "a < b && c > d"),
            ("  *bold* and _emphasis_", "*bold* and _emphasis_"),
            ("  line one\n  *bold* line two", "line one\n*bold* line two"),
            (
                "  a (C) mark\n  and an em -- dash",
                "a (C) mark\nand an em -- dash",
            ),
            (
                "  A claim.footnote:[the *evidence*]\n  continues here.",
                "A claim.footnote:[the *evidence*]\ncontinues here.",
            ),
            ("  first line +\n  second line", "first line +\nsecond line"),
            // A bare e-mail address in a wholly-synthesized seed: like an
            // anchor's id, and unlike a URL link's own target, an address is
            // recovered exactly by `text_slice` rather than deferred (see
            // `a_macro_construct_is_deferred_when_the_whole_seed_is_synthesized`
            // below for the families that still defer here).
            (
                "  write to doc@example.com\n  today",
                "write to doc@example.com\ntoday",
            ),
            // An index term in a wholly-synthesized seed: like an anchor's id
            // and a bare address, its shown text is recovered exactly (from
            // the match string) rather than deferred. The UI macros need the
            // `experimental` attribute, so they are exercised by `ui.rs`'s own
            // `ui_macros_are_recognized_when_the_whole_seed_is_synthesized`
            // instead.
            (
                "  a ((flow term)) here\n  and a (((c1, c2))) one",
                "a ((flow term)) here\nand a (((c1, c2))) one",
            ),
            // A cross-reference in a wholly-synthesized seed, in both
            // spellings: nothing on a `Ref{Xref}` node is `Span`-typed, so its
            // target and reference text come from the match string (or
            // `text_slice`) rather than an `'src` slice. The link and image
            // families, which hold a real `Attrlist<'src>`, still defer — see
            // `a_macro_construct_is_deferred_when_the_whole_seed_is_synthesized`
            // below.
            (
                "  see <<target>> and\n  xref:other[the other one]",
                "see <<target>> and\nxref:other[the other one]",
            ),
            // The URL-link family in a wholly-synthesized seed, and an image
            // macro with an *empty* bracket: neither needs an `'src` slice
            // (the empty attribute list parses from a zero-length span). An
            // image with a non-empty bracket, and the `link:`/`mailto:` macro
            // in any spelling, still defer — see
            // `a_macro_construct_is_deferred_when_the_whole_seed_is_synthesized`
            // below.
            (
                "  visit https://example.org[the site]\n  or https://plain.example",
                "visit https://example.org[the site]\nor https://plain.example",
            ),
            (
                "  see image:sunset.jpg[] and\n  icon:home[] here",
                "see image:sunset.jpg[] and\nicon:home[] here",
            ),
        ];

        for (source, filtered) in fixtures {
            let golden_parser = Parser::default();
            let built_parser = Parser::default();

            let golden = golden(filtered, &golden_parser);

            let nodes = build_from_value(
                CowStr::from(filtered.to_string()),
                Span::new(source),
                &built_parser,
                None,
            );
            let built = fold_html(&nodes, &HtmlSubstitutionRenderer {}, &built_parser);

            assert_eq!(
                built, golden,
                "fold diverged from the real pipeline for synthesized value {filtered:?} \
                 (location {source:?})"
            );
        }
    }

    /// The group-aware entry point ([`build_for_group`]) against the real
    /// pipeline, for the substitution groups whose step lists differ from
    /// [`SubstitutionGroup::Normal`] — pinning both the per-group step
    /// selection and the passthrough-extraction gate (`Macros` in the steps,
    /// or the `Header` group), which mirror `run_pipeline`'s own.
    mod build_for_group {
        #![allow(clippy::panic)]
        #![allow(clippy::unwrap_used)]

        use super::super::{build_for_group, fold_html};
        use crate::{
            Parser, Span,
            content::{Content, SubstitutionGroup},
            inlines::InlineNode,
            parser::{HtmlSubstitutionRenderer, ModificationContext},
            strings::CowStr,
        };

        fn parser_with_product() -> Parser {
            Parser::default().with_intrinsic_attribute(
                "product",
                "Widget",
                ModificationContext::Anywhere,
            )
        }

        fn build_group<'src>(
            group: &SubstitutionGroup,
            source: &'src str,
            parser: &Parser,
        ) -> Vec<InlineNode<'src>> {
            build_for_group(group, CowStr::from(source), Span::new(source), parser, None)
        }

        /// The real pipeline's own output for `source` under `group` — the
        /// golden every parity assertion in this module compares against.
        fn golden_for_group(group: &SubstitutionGroup, source: &str) -> String {
            let golden_parser = parser_with_product();
            let mut content = Content::from(Span::new(source));
            group.apply(&mut content, &golden_parser, None);
            content.rendered_str().to_string()
        }

        /// Runs `source` through the real pipeline under `group` and asserts
        /// the group-aware tree folds to the same bytes.
        fn assert_group_parity(group: &SubstitutionGroup, source: &str) {
            let golden = golden_for_group(group, source);

            let built_parser = parser_with_product();
            let nodes = build_group(group, source, &built_parser);
            let built = fold_html(&nodes, &HtmlSubstitutionRenderer {}, &built_parser);

            assert_eq!(
                built, golden,
                "group-aware fold diverged from the real pipeline for {source:?} under {group:?}"
            );
        }

        #[test]
        fn header_group_extracts_passthroughs_and_expands_attributes() {
            // The header group runs special characters and attribute
            // references only — but, uniquely among the non-`Macros` groups,
            // it *does* extract passthroughs (`run_pipeline`'s own gate).
            let source = "v pass:[<raw>] {product} *plain* < end";
            let parser = parser_with_product();
            let nodes = build_group(&SubstitutionGroup::Header, source, &parser);

            // The pass macro was extracted (a `Raw` leaf) …
            assert!(
                nodes.iter().any(
                    |n| matches!(n, InlineNode::Raw { value, .. } if value.as_ref() == "<raw>")
                ),
                "expected the pass macro's Raw leaf: {nodes:?}"
            );

            // … the attribute reference expanded …
            assert!(
                nodes
                    .iter()
                    .any(|n| matches!(n, InlineNode::Text { value, .. } if value.as_ref().contains("Widget"))),
                "expected the expanded attribute value: {nodes:?}"
            );

            // … and the quotes step did not run.
            assert!(
                !nodes.iter().any(|n| matches!(n, InlineNode::Styled(_))),
                "the quotes step must not run for the header group: {nodes:?}"
            );

            assert_group_parity(&SubstitutionGroup::Header, source);
        }

        #[test]
        fn attribute_entry_value_group_does_not_extract_passthroughs() {
            // The attribute-entry-value group runs the same two steps as the
            // header group but does *not* extract passthroughs, so a
            // `pass:[…]` stays literal text — mirroring `run_pipeline`'s
            // extraction gate exactly.
            let source = "v pass:[x] {product}";
            let parser = parser_with_product();
            let nodes = build_group(&SubstitutionGroup::AttributeEntryValue, source, &parser);

            assert!(
                !nodes.iter().any(|n| matches!(n, InlineNode::Raw { .. })),
                "a pass macro must stay literal for the attribute-entry-value group: {nodes:?}"
            );

            assert_group_parity(&SubstitutionGroup::AttributeEntryValue, source);
        }

        #[test]
        fn pass_and_none_groups_yield_the_untouched_seed() {
            // Neither group runs a single step, so the seed reaches the fold
            // untouched *as text* — but "untouched" is a claim about the bytes
            // the fold emits, not about the node kinds: because no
            // `SpecialCharacters` step ever acted on it, a literal `<`/`>`/`&`
            // is a `Raw` leaf the fold emits verbatim, not a `Text` run the
            // fold would escape (design §3.4.1). Everything between the
            // specials is one borrowed `Text` run, and nothing else is
            // recognized.
            let source = "<b>raw</b> *not bold* {product}";
            let parser = parser_with_product();

            for group in [SubstitutionGroup::Pass, SubstitutionGroup::None] {
                let nodes = build_group(&group, source, &parser);

                assert!(
                    nodes
                        .iter()
                        .all(|n| matches!(n, InlineNode::Text { .. } | InlineNode::Raw { .. })),
                    "expected only text and raw-special leaves under {group:?}: {nodes:?}"
                );

                assert!(
                    nodes.iter().any(
                        |n| matches!(n, InlineNode::Raw { value, .. } if value.as_ref() == "<")
                    ),
                    "expected the unescaped `<` to be a Raw leaf under {group:?}: {nodes:?}"
                );

                // That the leaves rejoin to the *untouched* seed is what the
                // parity assertion itself pins: neither group runs a step, so
                // the string pipeline's own output for this content is the
                // source verbatim, and the fold must reproduce it.
                assert_eq!(
                    golden_for_group(&group, source),
                    source,
                    "the string pipeline must leave this content untouched under {group:?}"
                );

                assert_group_parity(&group, source);
            }
        }

        #[test]
        fn stem_group_applies_special_characters_only() {
            let source = "*a* < {product}";
            let parser = parser_with_product();
            let nodes = build_group(&SubstitutionGroup::Stem, source, &parser);

            // `<` became a CharRef; `*a*` and `{product}` stayed literal.
            assert!(
                nodes
                    .iter()
                    .any(|n| matches!(n, InlineNode::CharRef { .. })),
                "expected the escaped special character: {nodes:?}"
            );
            assert!(
                !nodes.iter().any(|n| matches!(n, InlineNode::Styled(_))),
                "the quotes step must not run for the stem group: {nodes:?}"
            );

            assert_group_parity(&SubstitutionGroup::Stem, source);
        }

        #[test]
        fn a_custom_list_runs_exactly_the_named_steps_in_order() {
            // `subs=quotes` alone: `*bold*` is recognized, `<` is emitted
            // unescaped (a `Raw` leaf, not a `CharRef` — design §3.4.1, since
            // no `SpecialCharacters` step acts on it), and `{product}` stays
            // unexpanded.
            let (group, invalid) = SubstitutionGroup::from_custom_string(None, "quotes");
            assert!(invalid.is_empty());

            let source = "keep *bold* and a < b with {product}";
            let parser = parser_with_product();
            let nodes = build_group(&group, source, &parser);

            assert!(
                nodes.iter().any(|n| matches!(n, InlineNode::Styled(_))),
                "expected the quoted span: {nodes:?}"
            );
            assert!(
                !nodes
                    .iter()
                    .any(|n| matches!(n, InlineNode::CharRef { .. })),
                "the special-characters step must not run for subs=quotes: {nodes:?}"
            );
            assert!(
                nodes
                    .iter()
                    .any(|n| matches!(n, InlineNode::Raw { value, .. } if value.as_ref() == "<")),
                "the unescaped `<` must be a Raw leaf for subs=quotes: {nodes:?}"
            );
            assert!(
                nodes
                    .iter()
                    .any(|n| matches!(n, InlineNode::Text { value, .. } if value.as_ref().contains("{product}"))),
                "the attribute reference must stay literal for subs=quotes: {nodes:?}"
            );

            assert_group_parity(&group, source);
        }

        #[test]
        fn a_custom_list_including_macros_extracts_passthroughs() {
            // A custom list naming `macros` triggers passthrough extraction
            // (the same `steps.contains(Macros)` gate `run_pipeline` uses), so
            // a `+++…+++` passthrough is a `Raw` leaf even under a custom
            // list.
            let (group, invalid) = SubstitutionGroup::from_custom_string(None, "macros");
            assert!(invalid.is_empty());

            let source = "a +++<u>raw</u>+++ and image:pic.png[Alt]";
            let parser = parser_with_product();
            let nodes = build_group(&group, source, &parser);

            assert!(
                nodes.iter().any(
                    |n| matches!(n, InlineNode::Raw { value, .. } if value.as_ref() == "<u>raw</u>")
                ),
                "expected the passthrough's Raw leaf: {nodes:?}"
            );
            assert!(
                nodes.iter().any(|n| matches!(n, InlineNode::Image(_))),
                "expected the image macro's node: {nodes:?}"
            );

            assert_group_parity(&group, source);
        }

        /// A differential corpus for the §3.4.1 classification the group-aware
        /// entry point applies last: under an effective order whose steps
        /// **never include** `SpecialCharacters`, a literal `<`/`>`/`&` is
        /// emitted verbatim by the string pipeline, so it must fold verbatim
        /// here too.
        ///
        /// This is the case a `Text` node cannot express on its own — `Text`
        /// is logical text the fold *escapes* (design §3.4) — so every fixture
        /// below folded to escaped entities before the classification landed,
        /// diverging from the real pipeline. The corpus crosses a set of
        /// specials-bearing fixtures with every real group that takes this
        /// path: the two step-less groups (`Pass` for a passthrough block,
        /// `None` for a comment block) and the `subs=` custom lists that omit
        /// `specialcharacters`.
        #[test]
        fn a_group_that_never_escapes_folds_its_specials_verbatim() {
            let custom = |subs: &str| {
                let (group, invalid) = SubstitutionGroup::from_custom_string(None, subs);
                assert!(invalid.is_empty(), "unexpected invalid subs in {subs:?}");
                group
            };

            let groups = [
                SubstitutionGroup::Pass,
                SubstitutionGroup::None,
                // `subs=","` resolves to an empty step list (Ruby's
                // `split(',')` drops the trailing empties), the third way to
                // reach a group that runs nothing at all.
                custom(","),
                custom("quotes"),
                custom("attributes"),
                custom("replacements"),
                custom("macros"),
                custom("post_replacements"),
                custom("callouts"),
                custom("quotes,attributes"),
                custom("quotes,attributes,replacements,macros,post_replacements"),
            ];

            let fixtures = [
                // Bare specials, in every position a run can put them.
                "<b>raw</b>",
                "a < b & c > d",
                "<",
                "&",
                "leading < text",
                "text trailing >",
                "<>&",
                "a<b>c",
                // Specials that would look like a construct once escaped —
                // the escaped `&lt;&lt;` is what an `<<id>>` shorthand keys
                // off, and an escaped `&` is what a restored entity keys off,
                // so these pin that the classification does not perturb (or
                // depend on) recognition.
                "<<tigers>> stays literal",
                // The converse: a source-written `&lt;&lt;…&gt;&gt;` *is* the
                // shorthand's escaped spelling, so the group that runs
                // `macros` recognizes it (the string pipeline matches the very
                // bytes the author wrote). Its present-but-empty reference
                // text is one empty `Text` child, which the classification
                // must carry through rather than split away — splitting emits
                // no empty run, and the fold reads an absent child as "no text
                // provided" (a different rendering entirely).
                "&lt;&lt;install,&gt;&gt;",
                "&#169; stays literal",
                "-> and <- stay literal",
                // Specials beside each construct these orders *can*
                // recognize, so the classification is exercised inside and
                // around a built node's own children.
                "keep *bold < text* and a < b",
                "a < b {product} > c",
                "an <b>image</b> image:pic.png[Alt] here",
                "a footnote:[note < text] and a < b",
                "line one < +\nline two > end",
                // Multi-line, so a run spanning a newline is split the same
                // way as a single-line one.
                "first < line\nsecond & line\nthird > line",
            ];

            for group in &groups {
                for source in fixtures {
                    assert_group_parity(group, source);
                }
            }
        }
    }

    /// A documented divergence, not a bug: two shapes stay unrecognized when
    /// the *whole* seed is synthesized.
    ///
    /// The first is a construct needing a real
    /// [`Attrlist`](crate::attributes::Attrlist)`<'src>` — an image macro's
    /// non-empty attribute list, or a link's attribute-list-bearing display
    /// text — because
    /// [`Attrlist::parse`](crate::attributes::Attrlist::parse) reads its
    /// `source: Span<'src>`'s bytes as content, not merely as a location tag,
    /// so a real `'src` slice is not optional there. This is the same
    /// [`range_is_verbatim`](macros::image::range_is_verbatim) boundary that
    /// defers those shapes *nested inside* an attribute expansion's spliced
    /// value (see [`apply_attribute_references`]'s own doc comment), now
    /// reached at the tree's root instead of a nested splice.
    ///
    /// The second — exercised by this test's own fixture — is a
    /// `link:`/`mailto:` macro whose own marker is not verbatim, which a
    /// wholly-synthesized seed guarantees: that marker is what keeps the
    /// node's `location` telling this pass's nodes from the other link
    /// passes' when
    /// [`apply_link_side_effects`](macros::links::apply_link_side_effects)
    /// replays the string pipeline's registration order (see
    /// [`link_macro_level`](macros::links::link_macro_level)).
    ///
    /// Everything else is recognized here, including an image macro with an
    /// *empty* bracket and every URL-link spelling — both exercised by the
    /// parity sweep above, alongside the steps and families that never needed
    /// a verbatim slice in the first place (quotes, specialcharacters,
    /// attribute references, character replacements, post-replacement, and the
    /// anchor / bare-e-mail / UI / index-term / cross-reference families, each
    /// of which computes every value it holds from the match string or
    /// [`text_slice`](quotes::text_slice)).
    #[test]
    fn a_macro_construct_is_deferred_when_the_whole_seed_is_synthesized() {
        let filtered = "see link:https://example.org[Example] here";
        let source = "  see link:https://example.org[Example] here";

        let golden_parser = Parser::default();
        let golden = golden(filtered, &golden_parser);
        assert!(
            golden.contains(r#"href="https://example.org""#),
            "golden fixture stopped recognizing the link: {golden:?}"
        );

        let built_parser = Parser::default();
        let nodes = build_from_value(
            CowStr::from(filtered.to_string()),
            Span::new(source),
            &built_parser,
            None,
        );
        let built = fold_html(&nodes, &HtmlSubstitutionRenderer {}, &built_parser);

        assert_ne!(
            built, golden,
            "expected the documented divergence to still reproduce; the boundary may have \
             been lifted — if so, fold this fixture back into the parity sweep above"
        );
        assert_eq!(built, "see link:https://example.org[Example] here");
    }
}
