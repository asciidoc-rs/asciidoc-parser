//! Builds the inline AST **directly from source in a single forward pass**.
//!
//! This is the first brick of "Strategy B" (design §4.1): rather than recover
//! the tree from a post-substitution *marked string* – the
//! [`inline_tree`](crate::content::inline_tree) recorder's "Strategy A" – each
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
//!   node and threads it through the steps.
//! - [`apply_special_characters`] splits each `Text` run on `<`/`>`/`&` into
//!   precise-span [`Text`](InlineNode::Text) and
//!   [`CharRef`](InlineNode::CharRef) nodes.
//! - [`apply_quotes`] recognizes [quoted text] and wraps each matched run in a
//!   [`Styled`](crate::inlines::Styled) span, **introducing nesting** – `*a _b_
//!   c*` becomes a tree, not a flat run. It reuses the *exact* [`quote_subs`]
//!   the string pipeline matches with (changing the recognition *sink*, not the
//!   recognition), so its fold is byte-identical to the string step.
//! - [`apply_attribute_references`] recognizes attribute references (`{name}`),
//!   splicing a set attribute's resolved value into the node stream, classified
//!   into [`Text`](InlineNode::Text) and [`Raw`](InlineNode::Raw) runs per
//!   design §3.4.1 – a literal `<`/`>`/`&` in the value is *not* re-escaped,
//!   since [`apply_special_characters`] has already run. It reuses the shared
//!   [`ATTRIBUTE_REFERENCE`](crate::content::ATTRIBUTE_REFERENCE) pattern, so
//!   only the recognition *sink* differs. A `counter`/`counter2` directive and
//!   a missing-attribute reference under `AttributeMissing::Drop` /
//!   `::DropLine` are deferred (see [`apply_attribute_references`] for why); so
//!   is a construct *inside* an expanded value that
//!   [`apply_character_replacements`]/[`apply_macros`] would otherwise
//!   recognize – the value is a synthesized, non-`'src` run, and
//!   [`build_match_string`](quotes::build_match_string) does not yet look
//!   inside one (the same "not verbatim" boundary a macro over a rendered span
//!   already documents).
//! - [`apply_character_replacements`] recognizes [character replacements] –
//!   `(C)`, `--`, `...`, arrows, apostrophes, and restored entities – replacing
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
//!   `https://example.org[text]`), **cross-references** in both the
//!   `xref:` macro form (`xref:id[text]`) and the `<<id>>` shorthand, and
//!   **inline anchors** (`[[id]]`, `[[id,reftext]]`, `anchor:id[reftext]`) and
//!   **index terms** (`((term))`, `(((primary, secondary)))`, `indexterm:[…]`,
//!   `indexterm2:[…]`), replacing each with an [`Image`](InlineNode::Image),
//!   [`Ui`](InlineNode::Ui), [`Ref`](InlineNode::Ref),
//!   [`Anchor`](InlineNode::Anchor), or [`IndexTerm`](InlineNode::IndexTerm)
//!   node. An image node
//!   captures its own owned [`Attrlist`](crate::attributes::Attrlist) – the step that makes a macro node
//!   *self-describing*; a link or cross-reference node bakes its computed display
//!   text into [`Text`](InlineNode::Text) children so its fold needs no
//!   build-time state. Each family reuses the shared pattern the string step
//!   matches with ([`INLINE_IMAGE_MACRO`](crate::content::INLINE_IMAGE_MACRO), [`INLINE_KBD_BTN_MACRO`](crate::content::INLINE_KBD_BTN_MACRO),
//!   [`INLINE_MENU_MACRO`](crate::content::INLINE_MENU_MACRO), [`INLINE_LINK_MACRO`](crate::content::INLINE_LINK_MACRO), [`INLINE_LINK`](crate::content::INLINE_LINK),
//!   [`INLINE_XREF`](crate::content::INLINE_XREF), [`INLINE_ANCHOR`](crate::content::INLINE_ANCHOR), [`INLINE_INDEXTERM`](crate::content::INLINE_INDEXTERM)), builds
//!   `'src`-borrowing nodes for verbatim macros only (see [`apply_macros`] for
//!   the boundary the escaped-content case defers), and – for the UI macros – is
//!   recognized only under the `experimental` document attribute, exactly as the
//!   string step gates them. The cross-reference pass claims the same-document
//!   form in both spellings (`xref:id[text]` and `<<id>>` / `<<id,text>>`);
//!   inter-document targets, a document-as-a-whole reference, and an
//!   attribute-list text are deferred. An anchor renders from its id alone, so it
//!   is *always* recognized (never deferred): only a non-verbatim reference text
//!   – which does not reach the flow – leaves the node's `reftext` unpopulated.
//!   Likewise a *concealed* index term (`indexterm:[…]`, `(((…)))`) renders
//!   nothing, so it too is always recognized; a *visible* term (`indexterm2:[…]`,
//!   `((term))`) is deferred only when its shown text crosses a rendered span or
//!   carries an attribute list.
//! - [`apply_footnotes`] recognizes **footnotes** (`footnote:[…]`,
//!   `footnote:id[…]`, `footnote:id[]`), replacing each with a
//!   [`Footnote`](InlineNode::Footnote) node, folding through the shared
//!   [`INLINE_FOOTNOTE_MACRO`](crate::content::INLINE_FOOTNOTE_MACRO) pattern
//!   like every other family. It is its **own step** in [`build`], run once
//!   over the whole tree *after* [`apply_macros`] has resolved every other
//!   family at every level, rather than a level pass inside [`apply_macros`] –
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
//!   so – unlike the other families – a content crossing an already-recognized
//!   construct is not deferred: nesting is the point. Only the deprecated
//!   `footnoteref:` form and a content carrying an escaped closing bracket
//!   (`\]`) are deferred. Inline STEM (a passthrough-time construct) and the
//!   bibliography-anchor form are later increments.
//! - [`apply_post_replacements`] turns a trailing ` +` at the end of a line
//!   into a [`LineBreak`](InlineNode::LineBreak) leaf.
//! - [`fold_html`] folds the resulting leaves and spans back to output bytes
//!   through an
//!   [`InlineSubstitutionRenderer`](crate::parser::InlineSubstitutionRenderer)
//!   – the first fold over the *public* [`InlineNode`] tree (the recorder's
//!   [`fold_into`] folds an intermediate representation, not the public tree).
//!
//! [quoted text]: https://docs.asciidoctor.org/asciidoc/latest/subs/quotes/
//! [character replacements]:
//!     https://docs.asciidoctor.org/asciidoc/latest/subs/replacements/
//!
//! It is **additive and non-regressing**: nothing here is wired into the parse
//! path yet, so the authoritative string pipeline and the Strategy-A
//! [`Content::inlines`](crate::content::Content::inlines) tree are untouched.
//! Later increments extend the transducer to the remaining macro families,
//! attribute expansion, and passthroughs, at which point it can replace the
//! recorder, make `rendered_html()` a fold, and retire the sentinel systems.
//!
//! # A note on quote nesting
//!
//! The string pipeline realizes nesting by running the ordered [`quote_subs`]
//! over one growing string: an earlier sub renders `<strong>…</strong>` and a
//! later sub matches around (or inside) it. [`apply_quotes`] reproduces that by
//! applying each sub to the node tree in turn and, before matching at a level,
//! descending into the [`Styled`](crate::inlines::Styled) spans earlier subs
//! created – so `*a `b` c*` (strong containing a later-recognized monospace)
//! and `*a _b_ c*` nest correctly. A [`Styled`](crate::inlines::Styled) span an
//! earlier sub produced is otherwise **opaque** to a later sub at the same
//! level (represented by a single placeholder while matching), which is where
//! this single-pass recognition can, in principle, diverge from the string
//! pipeline's match-through-rendered-tags behavior for pathological cross-span
//! inputs; the differential corpus (§5.3) pins the cases this increment claims.
//!
//! [`fold_into`]: crate::content::inline_tree
//! [`Text`]: InlineNode::Text
//! [`quote_subs`]: crate::content::quote_subs

// The transducer framework is deliberately broader than the single step wired
// up so far; later Strategy-B increments consume the rest.
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

#[cfg(test)]
mod test_support;

use attribute_refs::apply_attribute_references;
use char_replacements::apply_character_replacements;
// Reachable only via `cfg(test)` callers and future external callers today
// (mirroring the crate-wide `#![allow(dead_code)]` above), so unlike the
// other step re-exports below (each consumed by `build`), this one is not
// itself consumed within this module.
#[allow(unused_imports)]
pub(crate) use fold::fold_html;
use footnotes::apply_footnotes;
use macros::apply_macros;
use passthrough_step::apply_passthroughs;
use post_replacements::apply_post_replacements;
use quotes::apply_quotes;
use special_chars::apply_special_characters;

use crate::{Parser, Span, inlines::InlineNode, strings::CowStr};

/// Builds the inline tree for `source` in a single forward pass.
///
/// The tree is seeded as one borrowed whole-source [`Text`](InlineNode::Text)
/// node and refined by each substitution step in turn. `source` is the exact
/// text to process, so a caller controls precisely what is built; reconciling
/// with a block's line filtering and joining is a later increment's concern.
///
/// `parser` is consulted only to parse the attribute list of an attributed
/// quote (`[.role]#…#`); a caller with no document context can pass a default
/// [`Parser`].
pub(crate) fn build<'src>(source: Span<'src>, parser: &Parser) -> Vec<InlineNode<'src>> {
    let seed = vec![InlineNode::Text {
        value: CowStr::from(source.data()),
        location: source,
    }];

    // Passthroughs are extracted before every other step (mirroring
    // `Passthroughs::extract_from`, which the string pipeline runs ahead of
    // its own step loop), so their content is never touched by
    // specialcharacters, quotes, replacements, or macros.
    let nodes = apply_passthroughs(seed, source, parser);
    let nodes = apply_special_characters(nodes);
    let nodes = apply_quotes(nodes, source, parser);

    // Attribute references sit here in the *normal* effective order
    // (specialcharacters → quotes → attributes → replacements → macros,
    // design §3.4.1): by this point `<`/`>`/`&` are already `CharRef` leaves
    // and quoted spans are already `Styled` nodes, and whatever this step
    // splices in is exactly what `apply_character_replacements` and
    // `apply_macros` – still ahead – see and refine.
    let nodes = apply_attribute_references(nodes, source, parser);

    let nodes = apply_character_replacements(nodes, source);
    let nodes = apply_macros(nodes, source, parser);

    // Footnotes are their own step, run once over the *whole* tree after
    // every other macro family has been resolved at every level – see
    // `apply_footnotes`'s doc comment for why this cannot be folded into
    // `apply_macros` as an ordinary level pass.
    let nodes = apply_footnotes(nodes, source, parser);

    apply_post_replacements(nodes, source)
}
