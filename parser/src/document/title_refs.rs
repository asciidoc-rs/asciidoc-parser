//! Document-order resolution of cross-references embedded in titles.
//!
//! A cross-reference in a section (or block) title renders the *target's*
//! reference text as its link text. When targets reference each other — a
//! forward reference, or a circular one — the reference text of one title
//! depends on another, so the per-content resolution pass (which resolves each
//! [`Content`](crate::content::Content) in isolation) cannot get this right on
//! its own: it would resolve every title's cross-references independently,
//! against each target's *parse-time* reference text.
//!
//! This pass mirrors Asciidoctor, which converts each title exactly once, in
//! document order, and caches the result. While a title is being converted, a
//! cross-reference back to a title that is *already being converted* (a cycle)
//! falls back to the bracketed `[id]` form rather than recursing forever. When
//! a resolved reference text is spliced into another reference as its link
//! text, any nested anchor is dropped (handled in the renderer's
//! `render_xref`).
//!
//! The result is that a title's rendered form and the reference text every
//! other reference sees for it are computed together, with cycles broken the
//! same way Asciidoctor breaks them.

use std::collections::HashMap;

use crate::{
    HasSpan, Span,
    blocks::{Block, IsBlock},
    content::{
        XrefSegment, XrefTemplatePiece, fold_resolved_title, render_xref_template,
        resolved_destinations,
    },
    document::Catalog,
    inlines::InlineNode,
    parser::{
        InlineRenderer, ReferenceResolver, ReferenceWarnings, ResolutionContext, ResolvedReference,
    },
};

/// The resolved outcome of one title: its final rendering, plus the resolved
/// destinations of its cross-references in placeholder order.
///
/// The rendering is installed into the title's rendered string; the
/// `block_ordered` / `footnote_ordered` destinations are mirrored into the
/// title's inline tree (see [`Content::mirror_tree_xref_resolution`]), so both
/// views of the title agree.
///
/// [`Content::mirror_tree_xref_resolution`]: crate::content::Content::mirror_tree_xref_resolution
struct Resolution {
    /// The title's final rendered form, with cross-title references
    /// coordinated.
    rendered: String,

    /// The resolved destination of each title-level cross-reference, in
    /// document order, ready for mirroring into the title tree.
    block_ordered: Vec<Option<ResolvedReference>>,

    /// The resolved destination of each cross-reference embedded in a footnote
    /// the title carries, in segment order, ready for
    /// mirroring into the title tree's footnote subtrees.
    footnote_ordered: Vec<Option<ResolvedReference>>,
}

/// One title carrying cross-references, captured for the resolution pass.
struct TitleNode<'src> {
    /// The title's **block-level** cross-references, in the document order its
    /// own inline tree holds them.
    block: Vec<XrefSegment>,

    /// The cross-references the title's footnotes carry.
    footnote: Vec<XrefSegment>,

    /// The deferred template, which a title renders from when it cannot
    /// fold: one whose nodes did not survive the `'src`-erasing hop a carried
    /// block title travels on (see `carried_title_template`).
    template: Vec<XrefTemplatePiece>,

    /// The ID under which other cross-references reach this title's reference
    /// text — present only when the title *is* the target's reference text (no
    /// explicit `reftext`), so a cross-reference to it should render this
    /// resolved title. `None` for a title that is not referenceable this way
    /// (no ID, or an explicit reftext that shadows the title).
    map_id: Option<String>,

    /// The title's source span, for anchoring an unresolved-reference warning.
    source: Span<'src>,

    /// A copy of the title's inline tree, which the pass folds to produce
    /// `rendered` — see [`fold_resolved_title`] for why it is a copy.
    inlines: Vec<InlineNode<'src>>,

    /// The document attributes in force where this title was written, retained
    /// on its content because a fold running later than its parse cannot read
    /// them from the parser (design §4.2's second sentinel system).
    ///
    /// `None` leaves the title on the template path, as it does everywhere
    /// else.
    render_attributes: Option<crate::parser::ResolvedAttributes>,
}

/// Resolves the cross-references embedded in every section heading and block
/// title in `blocks`, in document order, coordinating references between titles
/// (including circular ones) the way Asciidoctor does.
///
/// The per-content pass skips section headings (see
/// [`Block::resolve_references`]) and never resolved block titles at all; this
/// pass owns both. It installs each title's final rendering directly and
/// reports any unresolved target in `warnings`.
pub(crate) fn resolve_title_references<'src>(
    blocks: &mut [Block<'src>],
    catalog: &Catalog,
    resolver: &dyn ReferenceResolver,
    renderer: &dyn InlineRenderer,
    warnings: &mut ReferenceWarnings<'src>,
    parser: &crate::Parser,
) {
    let mut nodes: Vec<TitleNode<'src>> = Vec::new();
    collect(blocks, &mut nodes);

    if nodes.is_empty() {
        return;
    }

    // Referenceable titles, keyed by ID (first registration wins, mirroring the
    // catalog's own duplicate handling). Each entry carries the node itself
    // alongside its index, so the recursion in `compute` never has to look a
    // node up by index.
    let mut id_to_node: HashMap<&str, (usize, &TitleNode<'src>)> = HashMap::new();
    for (index, node) in nodes.iter().enumerate() {
        if let Some(id) = &node.map_id {
            id_to_node.entry(id.as_str()).or_insert((index, node));
        }
    }

    let mut memo: Vec<Option<Resolution>> = (0..nodes.len()).map(|_| None).collect();
    let mut in_progress: Vec<bool> = vec![false; nodes.len()];

    for (index, node) in nodes.iter().enumerate() {
        compute(
            index,
            node,
            &id_to_node,
            catalog,
            resolver,
            renderer,
            &mut memo,
            &mut in_progress,
            warnings,
            parser,
        );
    }

    let mut index = 0;
    write_back(blocks, &memo, &mut index, renderer, warnings, parser);
}

/// Walks `blocks` in document order, collecting each section heading and block
/// title that carries cross-references.
fn collect<'src>(blocks: &mut [Block<'src>], nodes: &mut Vec<TitleNode<'src>>) {
    for block in blocks.iter_mut() {
        if let Block::Section(section) = block {
            // A section's resolvable title is its heading.
            if let Some(deferred) = section.section_title_deferred_parts() {
                let map_id = if section.has_explicit_reftext() {
                    None
                } else {
                    section.reference_id()
                };

                let block = deferred.block.to_vec();
                let footnote = deferred.footnote.to_vec();
                let template = deferred.template.to_vec();

                nodes.push(TitleNode {
                    block,
                    footnote,
                    template,
                    map_id,
                    source: section.section_title_source(),
                    inlines: section.section_title_inlines().to_vec(),
                    render_attributes: section.section_title_render_attributes().cloned(),
                });
            }
        }

        // A block's `.Title` decoration — a *discrete* heading's included,
        // which is the one section kind that keeps its own (a non-discrete
        // section's is carried into its first block; its heading was collected
        // above). A block title is not treated as a recomputable reference
        // target (`map_id` is `None`): its own cross-references are resolved,
        // but a reference *to* the block still uses the block's parse-time
        // reference text.
        //
        // The span is taken before the title borrow: `block` stays mutably
        // borrowed while the template is in scope.
        {
            let source = block.span();

            if let Some(title) = block.block_title_content_mut()
                && let Some(deferred) = title.deferred_parts()
            {
                let block = deferred.block.to_vec();
                let footnote = deferred.footnote.to_vec();
                let template = deferred.template.to_vec();

                nodes.push(TitleNode {
                    block,
                    footnote,
                    template,
                    map_id: None,
                    source,
                    inlines: title.inlines().to_vec(),
                    render_attributes: title.render_attributes().cloned(),
                });
            }
        }

        collect(block.child_blocks_mut(), nodes);
    }
}

/// Installs each collected title's computed resolution, walking `blocks` in the
/// same document order as [`collect`] so `index` stays aligned. For every title
/// this installs both views the resolution carries: the coordinated rendered
/// string, and — mirrored into the title's inline tree — the resolved
/// destinations of its cross-references.
fn write_back<'src>(
    blocks: &mut [Block<'src>],
    memo: &[Option<Resolution>],
    index: &mut usize,
    renderer: &dyn InlineRenderer,
    warnings: &mut ReferenceWarnings<'src>,
    parser: &crate::Parser,
) {
    for block in blocks.iter_mut() {
        if let Block::Section(section) = block
            && section.section_title_deferred_parts().is_some()
        {
            if let Some(resolution) = memo.get(*index).and_then(Option::as_ref) {
                section.set_section_title_rendered(resolution.rendered.clone());
                section.mirror_section_title_tree_xrefs(
                    &resolution.block_ordered,
                    &resolution.footnote_ordered,
                );

                // **After** the mirror, not before: a footnote defined in
                // this heading is folded from the heading's own subtree, and
                // that subtree only carries the destinations just resolved
                // once `mirror_section_title_tree_xrefs` has installed them.
                // The fold `compute` took above cannot serve — it ran on a
                // *clone* holding only the block-level list, because the real
                // tree is not reachable while the pass is still computing.
                section
                    .section_title_content()
                    .collect_own_folded_footnotes(renderer, parser, warnings);
            }
            *index += 1;
        }

        // The block-title decoration's write-back — the same order [`collect`]
        // pushed them in: a section's heading first, then any block's own
        // `.Title`, a discrete heading's included.
        if let Some(title) = block.block_title_content_mut()
            && title.deferred_parts().is_some()
        {
            if let Some(resolution) = memo.get(*index).and_then(Option::as_ref) {
                title.set_rendered(resolution.rendered.clone());
                title.mirror_tree_xref_resolution(
                    &resolution.block_ordered,
                    &resolution.footnote_ordered,
                );

                // After the mirror, for the reason given in the section arm.
                title.collect_own_folded_footnotes(renderer, parser, warnings);
            }
            *index += 1;
        }

        write_back(
            block.child_blocks_mut(),
            memo,
            index,
            renderer,
            warnings,
            parser,
        );
    }
}

/// Computes (and memoizes) the resolved rendering of the title at `index`.
///
/// A cross-reference in the title whose target is itself a referenceable title
/// renders that target's resolved title (recursively), unless the target is
/// currently being computed — a cycle — in which case its link text falls back
/// to the bracketed `[target]` form, exactly as Asciidoctor breaks the cycle.
/// The nested anchor that results when a resolved title is used as link text is
/// dropped by the renderer.
#[allow(clippy::too_many_arguments)]
fn compute<'src>(
    index: usize,
    node: &TitleNode<'src>,
    id_to_node: &HashMap<&str, (usize, &TitleNode<'src>)>,
    catalog: &Catalog,
    resolver: &dyn ReferenceResolver,
    renderer: &dyn InlineRenderer,
    memo: &mut [Option<Resolution>],
    in_progress: &mut [bool],
    warnings: &mut ReferenceWarnings<'src>,
    parser: &crate::Parser,
) -> String {
    if let Some(Some(resolution)) = memo.get(index) {
        return resolution.rendered.clone();
    }

    if let Some(flag) = in_progress.get_mut(index) {
        *flag = true;
    }

    let mut block = node.block.clone();
    let mut footnote = node.footnote.clone();

    // The block-level and footnote-embedded references are resolved by the same
    // rules here — including the local-title recursion below — where
    // `Content::resolve_references` reports only the block-level ones. That
    // difference is pre-existing: a title reports every unresolved target it
    // carries, wherever in the title it sits.
    for xref in block.iter_mut().chain(footnote.iter_mut()) {
        // The catalog holds the document's own text, which is exactly what a
        // tree-read segment's target carries — same as
        // `Content::resolve_references`.
        let mut resolved = resolver.resolve(&ResolutionContext {
            target: &xref.target,
            provided_text: xref.provided_text.as_deref(),
            derived: xref.derived.as_ref(),
        });

        // Explicit link text is used verbatim, so a target that only supplies
        // its own reference text need not be consulted (and cannot start a
        // cycle). Empty explicit text (`<<id,>>`) is treated as absent.
        let has_explicit_text = xref.provided_text.as_deref().is_some_and(|t| !t.is_empty());

        // The resolver is authoritative: only a reference it resolved is
        // eligible for local title text, and then only when its destination is
        // the local target itself (the `#id` fragment). A resolver that mapped
        // the target anywhere else — e.g. an Antora-style resolver pointing at
        // another document — keeps its result untouched, even when its display
        // text happens to coincide with this document's reference text.
        //
        // For a locally-resolved reference, a display text the resolver chose
        // itself is likewise kept; the locally computed title only replaces
        // text that is absent or that merely echoes the catalog's frozen
        // (parse-time) reference text — the stale value this pass exists to
        // correct.
        if !has_explicit_text
            && let Some(reference) = resolved.as_mut()
            && let Some(target_id) = lookup_id(catalog, &xref.target)
            && let Some(&(target_index, target_node)) = id_to_node.get(target_id.as_str())
            && reference.href.strip_prefix('#') == Some(target_id.as_str())
        {
            let catalog_reftext = catalog
                .get_ref(&target_id)
                .and_then(|entry| entry.reftext.as_deref());

            let resolver_chose_text = reference
                .text
                .as_deref()
                .is_some_and(|text| Some(text) != catalog_reftext);

            if !resolver_chose_text {
                // The target's reference text is its own (resolved) title.
                // Recurse, unless the target is mid-computation — a cycle — in
                // which case its link text is the bracketed fallback.
                let target_in_progress = in_progress.get(target_index).copied().unwrap_or(false);
                reference.text = if target_in_progress {
                    None
                } else {
                    Some(compute(
                        target_index,
                        target_node,
                        id_to_node,
                        catalog,
                        resolver,
                        renderer,
                        memo,
                        in_progress,
                        warnings,
                        parser,
                    ))
                };
            }
        }

        // A target that resolved to nothing — and did not carry its own derived
        // destination — is an unresolved reference, reported against the title.
        if resolved.is_none() && xref.derived.is_none() {
            warnings.unresolved(&xref.target, node.source);
        }

        xref.resolved = resolved;
    }

    // The resolved destinations, in the document order the title's own tree
    // holds its cross-reference nodes in — which is the order the two lists
    // were read off that tree in, so they line up one-to-one when mirrored
    // (see `write_back`). These are the very segments the rendering below is
    // computed from, so the tree cannot disagree with the string.
    let block_ordered = resolved_destinations(&block);
    let footnote_ordered = resolved_destinations(&footnote);

    // The title's rendering is a **fold of its tree**, with the destinations
    // just resolved installed into it — the same answer `Content::refold` gives
    // a deferred paragraph, reached here rather than after the pass because
    // this string is also what a reference *to* this title splices in as its
    // link text. Rendering the template as well, and keeping only one, would
    // put every deferred title through a host renderer twice.
    //
    // The template is the fallback for a title with no tree to fold: a block
    // title carried across a section heading, whose inline nodes cannot cross
    // the `'src`-erasing hop it travels on. Every other title folds.
    let rendered = node
        .render_attributes
        .as_ref()
        .and_then(|attributes| {
            fold_resolved_title(&node.inlines, &block_ordered, attributes, renderer, parser)
        })
        .unwrap_or_else(|| render_xref_template(&node.template, &block, renderer));

    if let Some(flag) = in_progress.get_mut(index) {
        *flag = false;
    }
    if let Some(slot) = memo.get_mut(index) {
        *slot = Some(Resolution {
            rendered: rendered.clone(),
            block_ordered,
            footnote_ordered,
        });
    }
    rendered
}

/// Resolves a cross-reference target to a catalog ID the same way
/// [`CatalogResolver`](crate::parser::CatalogResolver) does: a direct ID match
/// first, then a natural (reference-text) match. Only same-document IDs are
/// returned, which is exactly the set of titles this pass can recompute.
fn lookup_id(catalog: &Catalog, target: &str) -> Option<String> {
    if catalog.contains_id(target) {
        Some(target.to_string())
    } else {
        catalog.resolve_id(target)
    }
}
