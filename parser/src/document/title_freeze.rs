//! Parse-time, demand-driven title cross-reference resolution.
//!
//! [`document::title_refs`](super::title_refs) resolves every title's
//! cross-references once, in document order, against the *complete* catalog —
//! which is correct for a title nothing forces to convert any earlier. It is
//! not the whole story: Asciidoctor also converts a section's title *eagerly*,
//! during parsing, whenever generating *another* section's auto-id needs this
//! title's reference text (`Section#id` calls `generate_id`, which reads the
//! referenced section's converted `title` when building the id string from a
//! title that embeds a cross-reference to it).
//!
//! Asciidoctor memoizes a section's converted title the first time anything
//! reads it, and never recomputes it afterward. When that first read happens
//! *during parsing* — because some auto-generated id demanded it — a forward
//! cross-reference embedded in the title that is not yet resolvable at that
//! moment is frozen as its bracketed `[target]` fallback forever: not just
//! for whoever demanded it, but for the title's own final rendering too, even
//! once the real target is parsed moments later. See issue #1110.
//!
//! This module is the parse-time half of that behavior, called from
//! [`SectionBlock::parse`](crate::blocks::SectionBlock::parse) exactly when a
//! section is about to auto-generate its own id — the one parse-time trigger
//! for eager conversion. [`resolve_now`] resolves and folds *that* section's
//! own title against the catalog as it currently stands (which is also what
//! its id gets built from), recursively [`demand`]-ing (and permanently
//! freezing) any *earlier* title it depends on. Freezing installs a
//! [`Resolution`] on the [`Parser`] so that:
//!
//! - [`document::title_refs`](super::title_refs)'s post-parse pass, which
//!   otherwise treats every title as freely recomputable, seeds its memo from
//!   [`Parser::frozen_title_resolution`] first and skips recomputing whatever
//!   it finds already frozen — installing the very same rendering (and resolved
//!   destinations) this module computed, for both the frozen section's own
//!   heading and anyone else's reference to it.
//! - A plain (non-title) cross-reference — resolved separately, and never
//!   re-resolved against a title's tree the way a title-to-title one is — sees
//!   the same frozen text too, because freezing installs it as the section's
//!   registered catalog reference text (`Catalog::set_reftext`).
//!
//! # It is the *target*, never the demander, that freezes
//!
//! Generating a section's own auto-id is what *triggers* this module, but it
//! is never what gets frozen: the id-generating section's own title is
//! resolved once more, fresh, by the ordinary post-parse pass, exactly as if
//! this module did not exist — it stays open to a target of its own, should
//! a still-later section's id happen to demand it. Only a title reached
//! through [`demand`] — an *earlier*, already-registered section this title
//! embeds a cross-reference to — is ever frozen. This matches Asciidoctor:
//! nothing about generating one's own id forces *that section's own*
//! `Section#xreftext` to be read (only the id string is needed for that),
//! whereas a title that itself references another section *does* need that
//! other section's `xreftext`, and reading it is what freezes it.
//!
//! A title nothing ever demands this way is untouched by this module: it
//! stays exactly as recomputable as it always was, resolved once the whole
//! document is known, by the ordinary post-parse pass — which is also what
//! ultimately resolves an id-generating section's own title, per the above.
//!
//! # A cross-document reference is never frozen
//!
//! A demanded title's own resolution is only actually installed as frozen
//! when it is *safe* to — see [`resolve_now`]'s own docs. A cross-document
//! target (`xref:other.adoc#topic[]`) can only ever be resolved by a host
//! supplied later, explicitly, to `Document::resolve_references` — arbitrarily
//! many times, each an independent sweep, per that method's own contract —
//! never by this module, which only ever consults the built-in,
//! same-document [`CatalogResolver`]. Freezing such a title anyway would
//! permanently bake in today's local-only fallback and deny every later sweep
//! the chance to do better. So a title reached through a cross-document
//! segment — its own, or one embedded (transitively) in a title it demands —
//! is resolved for this one demand and then left exactly as recomputable as
//! it was, open to a later demand that might find the catalog (or the
//! resolver) different next time.
//!
//! # Only a top-level splice, like a carried block title
//!
//! [`Parser`] cannot itself be generic over the source lifetime (see
//! [`OwnedTitle`](crate::content::OwnedTitle)'s own docs for why), so the
//! per-id state this module retains ([`RecomputableTitle`]) cannot hold a
//! borrowed inline tree. It renders through
//! [`render_xref_template`] instead —
//! the same *template* rendering a block title carried across a section
//! heading falls back to — which means a cross-reference **nested** inside
//! another construct (`*<<tgt>>*`) keeps whatever fallback text it had at the
//! title's own substitution time even after this module resolves it: only a
//! **top-level** reference's rendering reflects the demand-time resolution.
//! This is the same narrowing `fold_deferring_xrefs` documents for a carried
//! title, accepted here for the same reason: the alternative is retaining a
//! borrowed tree on the parser, which the crate's public API is not prepared
//! to pay for.

use std::collections::{HashMap, HashSet};

use crate::{
    Parser,
    content::{XrefSegment, XrefTemplatePiece, render_xref_template, resolved_destinations},
    parser::{CatalogResolver, ReferenceResolver, ResolutionContext, ResolvedReference},
};

/// The resolved outcome of one title: its final rendering, plus the resolved
/// destinations of its cross-references in placeholder order.
///
/// Shared between this module and
/// [`document::title_refs`](super::title_refs), which is the only reason it
/// is [`Clone`]: a frozen resolution computed here is cloned into that pass's
/// memo, once per document, rather than recomputed.
#[derive(Clone, Debug)]
pub(crate) struct Resolution {
    /// The title's final rendered form, with cross-title references
    /// coordinated.
    pub(crate) rendered: String,

    /// The resolved destination of each title-level cross-reference, in
    /// document order, ready for mirroring into the title tree.
    pub(crate) block_ordered: Vec<Option<ResolvedReference>>,

    /// The resolved destination of each cross-reference embedded in a
    /// footnote the title carries, in segment order, ready for mirroring into
    /// the title tree's footnote subtrees.
    pub(crate) footnote_ordered: Vec<Option<ResolvedReference>>,
}

/// A fully-owned (`'static`-shaped) snapshot of a section heading's own
/// deferred cross-references — everything [`resolve_now`] needs to resolve
/// and fold it again later, without the borrowed inline tree
/// [`Content::deferred_parts`](crate::content::Content::deferred_parts)
/// itself cannot outlive.
///
/// Registered under the section's id as soon as it is known (whether or not
/// this section itself ever triggers eager conversion), so a *later*
/// section's own demand can still reach it.
#[derive(Clone, Debug)]
pub(crate) struct RecomputableTitle {
    /// The title's block-level cross-references, in document order.
    pub(crate) block: Vec<XrefSegment>,

    /// The cross-references the title's footnotes carry.
    pub(crate) footnote: Vec<XrefSegment>,

    /// The deferred template this title renders from — see the module docs'
    /// "Only a top-level splice" section.
    pub(crate) template: Vec<XrefTemplatePiece>,
}

/// Parse-time demand-driven title state, threaded on the [`Parser`] for the
/// life of one parse. See the module docs.
#[derive(Clone, Debug, Default)]
pub(crate) struct TitleFreezeState {
    /// Every non-explicit-reftext section's own deferred title parts, keyed
    /// by its registered id — a potential demand *target*, whether or not
    /// anything ever actually demands it.
    recomputable: HashMap<String, RecomputableTitle>,

    /// The frozen resolution for an id that was demanded during parsing.
    /// Once set for an id, never overwritten — that permanence is the
    /// freeze.
    frozen: HashMap<String, Resolution>,
}

impl TitleFreezeState {
    /// Returns the frozen resolution for `id`, if it has one.
    pub(crate) fn frozen_resolution(&self, id: &str) -> Option<&Resolution> {
        self.frozen.get(id)
    }

    /// Whether `id` is a *recomputable* demand target at all — frozen
    /// already, or still open to a demand — the same question
    /// `document::title_refs::compute`'s own `id_to_node.get(target_id)`
    /// asks of the post-parse pass's node list. `false` for a target that
    /// resolves through the catalog perfectly well but carries no
    /// cross-reference of its own (so was never registered here at all): its
    /// catalog reference text is already final, and [`demand`] must not be
    /// asked about it — asking would (correctly) report "not found" and the
    /// caller would wrongly read that as "no reference text", discarding a
    /// perfectly good one.
    fn is_recomputable(&self, id: &str) -> bool {
        self.frozen.contains_key(id) || self.recomputable.contains_key(id)
    }
}

/// Registers `id`'s own deferred title parts as a potential demand target —
/// called once the section's final id is known, for every section without an
/// explicit reftext.
///
/// Unconditional: nothing can have frozen `id` yet. Freezing only ever
/// reaches an id [`demand`] finds already registered here, and `id` itself
/// only becomes registered — atomically, within this same
/// [`SectionBlock::parse`](crate::blocks::SectionBlock::parse) call, with no
/// other section's parse interleaved — a few lines after the catalog
/// registration `Catalog::same_document_id` requires to resolve *to* it in
/// the first place. There is consequently no window in which some other,
/// still-parsing section could demand `id` before this call installs its
/// snapshot.
pub(crate) fn register_recomputable_title(parser: &mut Parser, id: &str, title: RecomputableTitle) {
    parser
        .title_freeze_state_mut()
        .recomputable
        .insert(id.to_string(), title);
}

/// Resolves and folds `node` right now, against the catalog as it currently
/// stands, recursively [`demand`]-ing (and freezing) any earlier title it
/// depends on. Called for the *id-generating* section's own title — the
/// computation that decides its id string — never installed as that
/// section's own frozen resolution (see the module docs' "It is the target,
/// never the demander" section): only what this walk reaches *through*
/// [`demand`] ever freezes.
///
/// This reports no warnings of its own for a target it cannot resolve: a
/// `PossibleInvalidReference` warning only ever survives as part of a
/// resolution *sweep*'s own output (`Document::replace_reference_warnings`
/// discards every prior one whenever a new sweep runs, including the one
/// [`Parser::parse`] itself always performs right after parsing finishes), so
/// a warning pushed here would be silently wiped moments later. Instead, a
/// frozen node's [`Resolution::block_ordered`]/[`Resolution::footnote_ordered`]
/// carries a
/// `None` for exactly the segments that never resolved, which
/// [`document::title_refs`](super::title_refs)'s own sweep reads back to
/// report them itself — the one place a warning is safe from being
/// overwritten by the very sweep it's part of.
///
/// The second element of the returned pair says whether this resolution is
/// safe to [`freeze`](demand): `false` when any segment reached — directly on
/// `node`, or transitively through a [`demand`]ed target — carries its own
/// [`DerivedReference`](crate::parser::DerivedReference) (a cross-document-
/// shaped target, e.g. `xref:other.adoc#topic[]`). A parse only ever consults
/// the built-in, same-document [`CatalogResolver`] (a host's own resolver, if
/// any, is supplied later, explicitly, to `Document::resolve_references`,
/// arbitrarily many times, each an independent sweep); permanently baking in
/// today's local-only fallback for such a target would deny every later sweep
/// the chance to resolve it properly, defeating that contract. A target this
/// walk simply cannot find yet (same-document-shaped, not `derived`, just not
/// registered) does *not* taint freezability — that is the ordinary, correct
/// case issue #1110 exists for, and Asciidoctor freezes it too.
pub(crate) fn resolve_now(
    node: &RecomputableTitle,
    parser: &mut Parser,
    in_progress: &mut HashSet<String>,
) -> (Resolution, bool) {
    let mut block = node.block.clone();
    let mut footnote = node.footnote.clone();
    let mut freezable = true;

    for xref in block.iter_mut().chain(footnote.iter_mut()) {
        if xref.derived.is_some() {
            freezable = false;
        }

        let mut resolved = {
            let catalog = parser.catalog();
            CatalogResolver::new(&catalog).resolve(&ResolutionContext {
                target: &xref.target,
                provided_text: xref.provided_text.as_deref(),
                derived: xref.derived.as_ref(),
            })
        };

        // Explicit link text is used verbatim, so a target that only
        // supplies its own reference text need not be consulted — mirroring
        // `document::title_refs::compute`'s identical check.
        let has_explicit_text = xref.provided_text.as_deref().is_some_and(|t| !t.is_empty());

        let target_id = if has_explicit_text {
            None
        } else {
            parser.catalog().same_document_id(&xref.target)
        };

        if let Some(reference) = resolved.as_mut()
            && let Some(target_id) = target_id.as_deref()
            && reference.href.strip_prefix('#') == Some(target_id)
            && parser.title_freeze_state().is_recomputable(target_id)
        {
            // Unlike `document::title_refs::compute`, there is no
            // `resolver_chose_text` check here: the only resolver a parse can
            // ever consult is the built-in `CatalogResolver` (a host's own
            // resolver, if any, is only ever supplied later, explicitly, to
            // `Document::resolve_references` — long after parsing finishes),
            // and `CatalogResolver`'s own text is always read from this same
            // catalog entry, so it can never disagree with it.
            //
            // `is_recomputable` above guarantees `demand` finds `target_id`
            // either already frozen or still registered, so the only way it
            // can still return `None` here is the cycle case — exactly
            // mirroring `document::title_refs::compute`'s own
            // `target_in_progress` branch, which is the one other place that
            // explicitly blanks a reference's text to the bracketed fallback
            // rather than leaving the catalog's already-good answer alone.
            reference.text = match demand(target_id, parser, in_progress) {
                Some((text, target_freezable)) => {
                    freezable &= target_freezable;
                    Some(text)
                }
                None => None,
            };
        }

        xref.resolved = resolved;
    }

    let block_ordered = resolved_destinations(&block);
    let footnote_ordered = resolved_destinations(&footnote);
    let rendered = render_xref_template(&node.template, &block, &*parser.renderer);

    (
        Resolution {
            rendered,
            block_ordered,
            footnote_ordered,
        },
        freezable,
    )
}

/// Demands `target_id`'s reference text against the catalog as it currently
/// stands. When the result is safe to (see [`resolve_now`]'s own docs), it is
/// frozen — permanently, for every future reference, including `target_id`'s
/// own final rendered heading. See the module docs.
///
/// Returns `None` when `target_id` does not name a known recomputable title
/// (not registered yet, or registered with an explicit reftext, in which case
/// it is not a recomputable target at all) or is already mid-resolution
/// higher up this same call chain — a cycle, broken exactly as
/// `document::title_refs::compute` breaks one: the bracketed fallback,
/// *without* freezing anything (the section is still being computed further
/// up the stack, which is what freezes it once that frame returns). Otherwise
/// returns the resolved text alongside whether it was frozen, so a caller
/// folding this text into its *own* title knows whether it, in turn, is still
/// safe to freeze.
fn demand(
    target_id: &str,
    parser: &mut Parser,
    in_progress: &mut HashSet<String>,
) -> Option<(String, bool)> {
    if let Some(resolution) = parser.title_freeze_state().frozen_resolution(target_id) {
        return Some((resolution.rendered.clone(), true));
    }

    if !in_progress.insert(target_id.to_string()) {
        return None;
    }

    let snapshot = parser
        .title_freeze_state()
        .recomputable
        .get(target_id)
        .cloned();
    let result = snapshot.map(|node| resolve_now(&node, parser, in_progress));

    in_progress.remove(target_id);

    result.map(|(resolution, freezable)| {
        let rendered = resolution.rendered.clone();

        if freezable {
            parser.set_ref_reftext(target_id, rendered.clone());
            parser
                .title_freeze_state_mut()
                .frozen
                .insert(target_id.to_string(), resolution);
        }

        (rendered, freezable)
    })
}
