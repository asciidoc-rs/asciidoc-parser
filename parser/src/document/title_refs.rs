//! Document-order resolution of cross-references embedded in titles.
//!
//! A cross-reference in a section (or block) title renders the *target's*
//! reference text as its link text. When targets reference each other – a
//! forward reference, or a circular one – the reference text of one title
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
    content::{XrefSegment, render_xref_template},
    document::Catalog,
    parser::{InlineSubstitutionRenderer, ReferenceResolver, ReferenceWarnings, ResolutionContext},
};

/// One title carrying cross-references, captured for the resolution pass.
struct TitleNode<'src> {
    /// The placeholder-bearing template captured when the title was finalized.
    template: String,

    /// The title's cross-references, in placeholder order.
    xrefs: Vec<XrefSegment>,

    /// The ID under which other cross-references reach this title's reference
    /// text – present only when the title *is* the target's reference text (no
    /// explicit `reftext`), so a cross-reference to it should render this
    /// resolved title. `None` for a title that is not referenceable this way
    /// (no ID, or an explicit reftext that shadows the title).
    map_id: Option<String>,

    /// The title's source span, for anchoring an unresolved-reference warning.
    source: Span<'src>,
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
    renderer: &dyn InlineSubstitutionRenderer,
    warnings: &mut ReferenceWarnings<'src>,
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

    let mut memo: Vec<Option<String>> = vec![None; nodes.len()];
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
        );
    }

    let mut index = 0;
    write_back(blocks, &memo, &mut index);
}

/// Walks `blocks` in document order, collecting each section heading and block
/// title that carries cross-references.
fn collect<'src>(blocks: &mut [Block<'src>], nodes: &mut Vec<TitleNode<'src>>) {
    for block in blocks.iter_mut() {
        if let Block::Section(section) = block {
            // A section's resolvable title is its heading.
            if let Some((template, xrefs)) = section.section_title_deferred_parts() {
                let map_id = if section.has_explicit_reftext() {
                    None
                } else {
                    section.reference_id()
                };

                nodes.push(TitleNode {
                    template: template.to_string(),
                    xrefs: xrefs.to_vec(),
                    map_id,
                    source: section.section_title_source(),
                });
            }
        } else {
            // A non-section block's `.Title` decoration. A block title is not
            // treated as a recomputable reference target (`map_id` is `None`):
            // its own cross-references are resolved, but a reference *to* the
            // block still uses the block's parse-time reference text.
            //
            // The span is taken before the title borrow: `block` stays
            // mutably borrowed while the template is in scope.
            let source = block.span();

            if let Some((template, xrefs)) = block
                .block_title_content_mut()
                .and_then(|title| title.deferred_parts())
            {
                nodes.push(TitleNode {
                    template: template.to_string(),
                    xrefs: xrefs.to_vec(),
                    map_id: None,
                    source,
                });
            }
        }

        collect(block.nested_blocks_mut(), nodes);
    }
}

/// Installs the computed rendering for each collected title, walking `blocks`
/// in the same document order as [`collect`] so `index` stays aligned.
fn write_back<'src>(blocks: &mut [Block<'src>], memo: &[Option<String>], index: &mut usize) {
    for block in blocks.iter_mut() {
        if let Block::Section(section) = block {
            if section.section_title_deferred_parts().is_some() {
                if let Some(rendered) = memo.get(*index).and_then(Option::as_ref) {
                    section.set_section_title_rendered(rendered.clone());
                }
                *index += 1;
            }
        } else if let Some(title) = block.block_title_content_mut()
            && title.deferred_parts().is_some()
        {
            if let Some(rendered) = memo.get(*index).and_then(Option::as_ref) {
                title.set_rendered(rendered.clone());
            }
            *index += 1;
        }

        write_back(block.nested_blocks_mut(), memo, index);
    }
}

/// Computes (and memoizes) the resolved rendering of the title at `index`.
///
/// A cross-reference in the title whose target is itself a referenceable title
/// renders that target's resolved title (recursively), unless the target is
/// currently being computed – a cycle – in which case its link text falls back
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
    renderer: &dyn InlineSubstitutionRenderer,
    memo: &mut [Option<String>],
    in_progress: &mut [bool],
    warnings: &mut ReferenceWarnings<'src>,
) -> String {
    if let Some(Some(rendered)) = memo.get(index) {
        return rendered.clone();
    }

    if let Some(flag) = in_progress.get_mut(index) {
        *flag = true;
    }

    let mut xrefs = node.xrefs.clone();

    for xref in xrefs.iter_mut() {
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
        // the target anywhere else – e.g. an Antora-style resolver pointing at
        // another document – keeps its result untouched, even when its display
        // text happens to coincide with this document's reference text.
        //
        // For a locally-resolved reference, a display text the resolver chose
        // itself is likewise kept; the locally computed title only replaces
        // text that is absent or that merely echoes the catalog's frozen
        // (parse-time) reference text – the stale value this pass exists to
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
                // Recurse, unless the target is mid-computation – a cycle – in
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
                    ))
                };
            }
        }

        // A target that resolved to nothing – and did not carry its own derived
        // destination – is an unresolved reference, reported against the title.
        if resolved.is_none() && xref.derived.is_none() {
            warnings.unresolved(&xref.target, node.source);
        }

        xref.resolved = resolved;
    }

    let rendered = render_xref_template(&node.template, &xrefs, renderer);

    if let Some(flag) = in_progress.get_mut(index) {
        *flag = false;
    }
    if let Some(slot) = memo.get_mut(index) {
        *slot = Some(rendered.clone());
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
