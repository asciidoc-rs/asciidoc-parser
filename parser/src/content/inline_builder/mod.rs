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
//!   only the recognition *sink* differs. A `counter`/`counter2` directive
//!   resolves *and advances* the named counter via [`Parser::counter`], the
//!   same required side effect [`apply_footnotes`] performs for footnote
//!   numbering. A missing-attribute reference under `AttributeMissing::Drop` /
//!   `::DropLine` is deferred (see [`apply_attribute_references`] for why); so
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
//!   captures its own owned [`Attrlist`] – the step that makes a macro node
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
//!   passthrough (`stem:[+++<b>x</b>+++]`) is never deferred – the `Raw` is
//!   spliced into the node's value verbatim, unlike every other macro family's
//!   "crosses an already-recognized construct" boundary. A macro carrying an
//!   explicit substitution list (`stem:c,q[…]`) is recognized too: the list
//!   resolves to a [`SubstitutionGroup`](crate::content::SubstitutionGroup) the
//!   expression runs through in place of the bare macro's
//!   [`SubstitutionGroup::Stem`](crate::content::SubstitutionGroup::Stem) – the
//!   same `passthrough_text`-through-the-real-pipeline treatment that resolves
//!   the analogous `pass:c,q[…]` form (see [`apply_passthroughs`]'s doc
//!   comment), except a `Stem` node already has a single `value` field to hold
//!   the result, so no richer subtree is needed.
//! - [`apply_post_replacements`] turns a trailing ` +` at the end of a line
//!   into a [`LineBreak`](InlineNode::LineBreak) leaf. Under the block-wide
//!   `hardbreaks` option – the enclosing block's own `attrlist`, or the
//!   document's `hardbreaks-option` attribute – it instead turns *every* line
//!   ending into a break, stripping a redundant trailing ` +` rather than
//!   doubling it, mirroring [`apply_post_replacements`]'s own string-pipeline
//!   counterpart exactly.
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
//! # Staging the cutover's recognition side effects
//!
//! Every macro family above deliberately skips a **recognition side effect**
//! the string pipeline performs at the same point – registering an id, link,
//! or image target in the document catalog, or recording a warning – because
//! the additive builder still runs *alongside* the authoritative string
//! pipeline (each against its own, independent [`Parser`]), so performing one
//! here today would risk double-counting a registration once the two paths
//! ever share a `Parser`. Each family's own deferred side effects are staged
//! as their own reviewable building block – `register_image` and the `link=`
//! dangerous-scheme/self-href warning for `image:`/`icon:`, `register_link`
//! for the four link-macro forms, and the `register_ref` pair for anchors and
//! id-carrying attributed spans – and [`apply_macro_side_effects`] composes
//! all three, in the string pipeline's own family-pass order, into the single
//! call the eventual cutover makes exactly once per parse (design §5.2, Phase
//! 4 step 6). It is exercised only by its own tests (and its constituents'
//! own), against their own `Parser` – calling it for real still waits for the
//! single-pass builder to replace the recorder as `Content`'s tree source.
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
mod stem_step;

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
// Staged for the eventual cutover (see this module's own doc comment); not
// yet called from any real parse path, so – like `fold_html` above – reachable
// only via `cfg(test)` callers and future external callers today.
#[allow(unused_imports)]
pub(crate) use macros::apply_macro_side_effects;
use macros::apply_macros;
use passthrough_step::apply_passthroughs;
use post_replacements::apply_post_replacements;
use quotes::apply_quotes;
use special_chars::apply_special_characters;
use stem_step::apply_stem;

use crate::{Parser, Span, attributes::Attrlist, inlines::InlineNode, strings::CowStr};

/// Builds the inline tree for `source` in a single forward pass.
///
/// The tree is seeded as one borrowed whole-source [`Text`](InlineNode::Text)
/// node and refined by each substitution step in turn. `source` is the exact
/// text to process, so a caller controls precisely what is built; reconciling
/// with a block's line filtering and joining is a later increment's concern.
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
    let seed = vec![InlineNode::Text {
        value: CowStr::from(source.data()),
        location: source,
    }];

    // Passthroughs are extracted before every other step (mirroring
    // `Passthroughs::extract_from`, which the string pipeline runs ahead of
    // its own step loop), so their content is never touched by
    // specialcharacters, quotes, replacements, or macros.
    let nodes = apply_passthroughs(seed, source, parser);

    // Inline STEM is an implicit passthrough too, extracted last (mirroring
    // `Passthroughs::extract_from`'s own ordering) so a passthrough
    // placeholder nested inside a STEM expression survives.
    let nodes = apply_stem(nodes, source, parser);

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

    apply_post_replacements(nodes, source, parser, attrlist)
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
/// apply`](crate::content::SubstitutionGroup::apply) runs in production –
/// passthrough/STEM extraction, every step in true order, passthrough restore,
/// and deferred-reference finalization, all against one `Content` – which is
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
/// inside the vocabulary [`build`] already covers – it avoids the forms still
/// documented as deferred elsewhere in this module (e.g. an attribute value
/// that itself embeds a construct `CharacterReplacements`/`Macros` would
/// recognize).
#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::build;
    use crate::{
        Parser, Span,
        content::{Content, SubstitutionGroup, inline_builder::fold_html},
        parser::{HtmlSubstitutionRenderer, ModificationContext},
    };

    /// Runs `source` through the real, public `SubstitutionGroup::Normal`
    /// pipeline, exactly as a real block's content is substituted in
    /// production.
    fn golden(source: &str, parser: &Parser) -> String {
        let mut content = Content::from(Span::new(source));
        SubstitutionGroup::Normal.apply(&mut content, parser, None);
        content.rendered_str().to_string()
    }

    /// Builds and folds the single-pass tree for `source` – the same [`build`]
    /// a real cutover would call – through the built-in HTML renderer.
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
    /// lockstep – the same two-independent-parsers discipline this module's
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
        assert_parity_with(
            "kbd:[Ctrl+Alt+Del] opens the *Task Manager* via menu:File[Save].",
            || {
                Parser::default().with_intrinsic_attribute_bool(
                    "experimental",
                    true,
                    ModificationContext::Anywhere,
                )
            },
        );

        // Several link forms and a cross-reference in one sentence.
        assert_parity(
            "Visit https://example.org[the site] or mailto:a@example.org[email us], \
             then see <<conclusion,the conclusion>>.",
        );

        // A `counter` directive beside a quoted span carrying an attribute
        // reference and a STEM expression – the ordering fix documented in
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
    }
}
