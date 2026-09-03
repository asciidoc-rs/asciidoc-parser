//! The HTML-folding step: renders the built tree back to output bytes.

use super::{callouts::replacement_type_of, quotes::quote_type_of};
use crate::{
    attributes::Attrlist,
    content::{XrefSegment, XrefTemplatePiece, render_xref_segment, xref_segment_from_node},
    inlines::{
        Anchor, Callout, CharRef, Footnote, Image, IndexTerm, InlineNode, RawForm, Ref, RefVariant,
        SpanForm, Stem, StemNotation, Ui, UiKind,
    },
    parser::{
        InlineRenderer, QuoteScope, QuoteType, RenderContext, SpecialCharacter, XrefRenderParams,
    },
    span::HasSpan,
    strings::CowStr,
};

/// Folds an inline node tree to output bytes through `renderer`.
///
/// This is the fold over the *public* [`InlineNode`] tree. It handles every
/// node kind the type admits — [`Text`](InlineNode::Text),
/// [`CharRef`](InlineNode::CharRef), [`Raw`](InlineNode::Raw),
/// [`Styled`](crate::inlines::Styled), [`Image`](InlineNode::Image),
/// [`Ui`](InlineNode::Ui), [`Ref`](InlineNode::Ref) (both link and
/// cross-reference), [`Anchor`](InlineNode::Anchor),
/// [`IndexTerm`](InlineNode::IndexTerm), [`Footnote`](InlineNode::Footnote),
/// [`Callout`](InlineNode::Callout), [`Stem`](InlineNode::Stem), and
/// [`LineBreak`](InlineNode::LineBreak) — so the match is exhaustive with no
/// defensive fallback arm.
///
/// `context` is the document state this fold renders under — see
/// [`RenderContext`]. It is taken as a parameter rather than derived from a
/// [`Parser`](crate::Parser) here because a fold does not necessarily run
/// during the parse that produced the tree: content carrying a deferred
/// cross-reference is folded again after resolution, under the attributes that
/// were in effect where it was *written* rather than wherever the parse ended
/// up.
pub(crate) fn fold_html(
    nodes: &[InlineNode<'_>],
    renderer: &dyn InlineRenderer,
    context: &RenderContext,
) -> String {
    // Sized from the content's own source extent: rendered HTML is the
    // source text plus markup, so the source length seeds the buffer at the
    // plain-prose case's exact size and one growth doubling absorbs typical
    // markup. An estimate — the string grows normally past it.
    let source_len: usize = nodes.iter().map(|node| node.span().data().len()).sum();
    let mut out = String::with_capacity(source_len + 16);
    fold_into_html(
        nodes,
        renderer,
        context,
        Footnotes::Marked,
        &mut Xrefs::Rendered,
        &mut out,
    );
    out
}

/// Whether a fold writes a [`Footnote`](InlineNode::Footnote)'s in-flow
/// marker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Footnotes {
    /// Write it: the ordinary fold, which is what the flow of a block shows.
    Marked,

    /// Omit it, marker and all, leaving the surrounding text as though the
    /// footnote had not been written — see [`fold_reference_text`].
    Stripped,
}

/// Whether a fold *renders* a cross-reference or *defers* it.
///
/// The fold's second mode axis, alongside [`Footnotes`]. Both exist for the
/// same reason: a tree is folded to more than one string, and which string is
/// wanted is a question about node kinds rather than about bytes.
///
/// [`Deferred`](Self::Deferred) is the segment-recording mode — see
/// [`fold_deferring_xrefs`], which is the only way to reach it.
pub(crate) enum Xrefs<'a> {
    /// Render each cross-reference in place, through `render_xref`: the
    /// ordinary fold, which is what a block's flow shows.
    Rendered,

    /// Record each cross-reference's segment to this list as it is reached —
    /// appended in document order, so a reference's position in the list is
    /// stable regardless of how deep the recursion that reached it — and
    /// still render it in place, exactly as [`Rendered`](Self::Rendered)
    /// would. [`fold_deferring_xrefs`] is the one caller: it uses the
    /// recorded segments to build splice points for a footnote's **top-level**
    /// references, and relies on this rendering-in-place to give a
    /// **nested** one's nothing-recorded literal text the same bytes it would
    /// have under [`Rendered`](Self::Rendered) — see that function's own docs.
    Deferred(&'a mut Vec<XrefSegment>),
}

/// Folds `nodes` — a footnote's own top-level children — into a **structured
/// template** plus every cross-reference segment the footnote's text carries
/// (top-level and nested alike, in document order) — the pair
/// [`FootnoteDeferred`](crate::content::FootnoteDeferred) is built from.
///
/// A footnote's text is lifted out of the block it was written in, so a
/// cross-reference inside it is never reached by the document-order pass that
/// resolves the block's own references; the footnote has to carry its
/// references itself, as a template plus a segment list, and resolve them from
/// the catalog later ([`Footnote::resolve_references`]).
///
/// # Only a top-level reference becomes a splice point
///
/// This walks `nodes` at the **top level** only, exactly as
/// `carried_title_template` (design's carried-title analog) does: a top-level
/// [`Xref`](RefVariant::Xref) node contributes an
/// [`Xref`](XrefTemplatePiece::Xref) piece and its own segment
/// (via [`xref_segment_from_node`], the same reading every other segment
/// derivation uses); every other top-level node folds to one
/// [`Literal`](XrefTemplatePiece::Literal) piece. A
/// cross-reference **nested** inside that other node — `footnote:[*<<tgt>>*]`,
/// a `<<tgt>>` inside a styled span's body — cannot itself become a splice
/// point: its placeholder would sit *inside* the span's own rendered markup,
/// which a flat piece list cannot represent (it has no
/// "inside another piece" position). It is folded in
/// [`Deferred`](Xrefs::Deferred) mode regardless, though — not
/// [`Rendered`](Xrefs::Rendered) — so its segment is still recorded: it is
/// still resolved and, if unresolvable, still warned about by
/// [`FootnoteDeferred::resolve`](crate::content::FootnoteDeferred::resolve),
/// the same as a top-level one. Only its *rendering* is narrowed, to the
/// unresolved fallback baked into its enclosing literal forever — measured, not
/// assumed: every footnote in the suite reaches
/// `Content::collect_own_folded_footnotes`, whose tree-fold answer is what
/// production `text` actually reflects, both before and after any resolution
/// sweep (see [`Footnote::resolve_references`]'s own docs for the instrumented
/// count); this template is only ever rendered once, at registration, before
/// any reference — nested or not — has a resolved destination to lose.
/// `a_reference_nested_in_a_span_of_a_footnote_stays_its_fallback` pins the
/// boundary, and the complementary
/// `an_unresolvable_reference_nested_in_a_footnote_is_still_warned_about` pins
/// that the segment (and so the warning) survives it.
///
/// # Renderer callback count, not order, is preserved
///
/// A top-level reference's segment is recorded here but not rendered — its
/// bytes come later, from
/// [`render_xref_template`](crate::content::render_xref_template) walking the
/// piece it became. A nested one is rendered immediately, in this
/// same pass (`fold_xref`'s [`Deferred`](Xrefs::Deferred) branch renders in
/// place rather than writing a placeholder). Summed across a footnote's whole
/// registration (this fold, then the render that computes its initial `text`),
/// every reference — top-level or nested — is still rendered exactly once, so
/// a stateful renderer sees the same *number* of `render_xref` calls this
/// registration always made. What is not preserved is strict document-order
/// *interleaving* between the two kinds in one footnote that mixes them: every
/// nested reference's call happens during this fold, ahead of every top-level
/// one's, which happens afterward. No golden source mixes a top-level and a
/// nested cross-reference in the same footnote, so this is unmeasured rather
/// than accepted-and-verified — flag it if a consumer's stateful renderer ever
/// needs the stronger guarantee.
///
/// # Why the template is built at *registration* time
///
/// A footnote's catalog entry is registered when the footnote is recognized.
/// That is not
/// a convenience: `Footnote::resolve_references` runs on the **catalog entry**,
/// driven from the catalog, with no access to the tree the footnote came from,
/// so nothing later in the parse can go back and derive the pair. Recognition
/// is also the last moment the subtree is still final —
/// [`apply_post_replacements`](super::apply_post_replacements) descends into a
/// [`Styled`](crate::inlines::Styled)/[`Ref`](InlineNode::Ref) child but not
/// into a [`Footnote`](InlineNode::Footnote)'s — matching Asciidoctor's own
/// order, whose footnote text is already extracted from the flow by the time
/// post-replacements run.
///
/// # This is a *build-time* fold, and the only one
///
/// Building the tree is otherwise unobservable — it consults no renderer, a
/// property `tests::inline_tree`'s own
/// `building_the_tree_does_not_consult_the_documents_renderer` measures with a
/// stateful renderer. A footnote is the documented exception, and not by
/// choice: its catalog entry is a **required** recognition side effect (the
/// same reason its number is), and that entry's payload is a *rendered
/// string*, so registering it and rendering it are one act — as they were in
/// this crate's original string-substitution implementation too, whose
/// footnote replacer cut already-rendered bytes out of the flat string it was
/// substituting.
///
/// This adds no second rendering of the subtree. `fold_footnote` writes only
/// the in-flow **marker**, never the footnote's children, so the block's own
/// fold does not re-render what this one rendered: a footnote's subtree is
/// folded exactly once per parse, here.
///
/// [`Footnote::resolve_references`]: crate::document::Footnote::resolve_references
pub(crate) fn fold_deferring_xrefs(
    nodes: &[InlineNode<'_>],
    renderer: &dyn InlineRenderer,
    context: &RenderContext,
) -> (Vec<XrefTemplatePiece>, Vec<XrefSegment>) {
    let mut template: Vec<XrefTemplatePiece> = Vec::new();
    let mut segments = Vec::new();

    for node in nodes {
        if let InlineNode::Ref(reference) = node
            && reference.variant == RefVariant::Xref
        {
            let index = segments.len();
            segments.push(xref_segment_from_node(reference, renderer, context));
            template.push(XrefTemplatePiece::Xref(index));
            continue;
        }

        // Every other top-level node — including one that *contains* a
        // nested cross-reference — folds to one literal run. `Deferred` is
        // still the mode threaded through, not `Rendered`: a nested
        // reference's segment must still be recorded (see this function's
        // own docs), it is just rendered in place rather than addressable as
        // its own piece.
        //
        // The footnote axis is inert here whichever way it is set: `nodes`
        // is one footnote's own children, and footnotes do not nest (a
        // footnote's own lazy bracket match cannot recognize one inside
        // another's content either), so no `Footnote` node can be reached.
        // `Marked` is the ordinary fold, which is the honest default for a
        // mode nothing consults.
        let mut literal = String::new();
        fold_into_html(
            std::slice::from_ref(node),
            renderer,
            context,
            Footnotes::Marked,
            &mut Xrefs::Deferred(&mut segments),
            &mut literal,
        );

        match template.last_mut() {
            Some(XrefTemplatePiece::Literal(text)) => text.push_str(&literal),
            _ => template.push(XrefTemplatePiece::Literal(literal)),
        }
    }

    (template, segments)
}

/// The fold a **section title** contributes as its reference text and as the
/// source of its auto-generated id: [`fold_html`] with each footnote's in-flow
/// marker omitted.
///
/// A footnote in a heading is a real, document-order footnote — it is numbered
/// and it renders in the heading itself — but its marker must not appear in
/// the text a cross-reference to that heading shows, nor in the id derived
/// from that text (issue #594).
///
/// The string pipeline used to reach that answer with a sentinel system: it
/// rendered the title **once** with the footnote renderer bracketing each
/// marker in a pair of Private-Use-Area codepoints, cut the bracketed regions
/// out for the reference text, and then removed the now-spent sentinels from
/// the title's own rendering and from its deferred cross-reference template.
/// Rendering once is what kept counters and attribute-expanded footnotes from
/// being processed twice; the sentinels were what made one render yield two
/// strings.
///
/// A tree needs none of it. The footnote is a node, so the two strings are two
/// folds of the same tree, and "which regions were footnote markers" is a
/// question about node kinds rather than about bytes — still one substitution
/// pass, so the counters are still processed once. `Section::parse` calls this
/// for a heading's reference text, and the whole sentinel system — one of the
/// three Unicode-sentinel hacks the string pipeline used (see this module's
/// own `README.md`) — is deleted.
///
/// The strip recurses, exactly as the byte-level one does over the whole
/// rendered string: a footnote nested inside a rendered span, or inside a
/// cross-reference's own display text, is omitted there too.
pub(crate) fn fold_reference_text(
    nodes: &[InlineNode<'_>],
    renderer: &dyn InlineRenderer,
    context: &RenderContext,
) -> String {
    let mut out = String::new();
    fold_into_html(
        nodes,
        renderer,
        context,
        Footnotes::Stripped,
        &mut Xrefs::Rendered,
        &mut out,
    );
    out
}

/// Appends the fold of `nodes` to `out` (the recursive worker for
/// [`fold_html`]).
///
/// `context` is threaded through because rendering some nodes — an
/// [`Image`](InlineNode::Image), whose `render_image`/`render_icon` reads the
/// document's safe mode, `data-uri`, and `icons`/`icontype` attributes — is a
/// function of document state, not of the node alone. `footnotes` is
/// threaded through because a heading's own reference text omits its footnote
/// markers wherever they sit — see [`fold_reference_text`].
fn fold_into_html(
    nodes: &[InlineNode<'_>],
    renderer: &dyn InlineRenderer,
    context: &RenderContext,
    footnotes: Footnotes,
    xrefs: &mut Xrefs<'_>,
    out: &mut String,
) {
    for node in nodes {
        match node {
            InlineNode::Text { value, .. } => {
                // `Text` is logical (un-escaped) text; the fold escapes it. The
                // builder never leaves a special inside a `Text`, so this is
                // belt-and-suspenders, but it keeps the fold correct in its own
                // right.
                render_text(value, renderer, out);
            }

            InlineNode::Raw {
                value,
                form: RawForm::AsIs,
                ..
            } => {
                out.push_str(value);
            }

            // Literal text that later steps could not see into, but which is
            // not raw output: the fold escapes it exactly as it escapes a
            // `Text` run, with whatever renderer *this* fold was given.
            InlineNode::Raw {
                value,
                form: RawForm::Escaped,
                ..
            } => {
                render_text(value, renderer, out);
            }

            InlineNode::CharRef {
                value: CharRef::Special(ch),
                ..
            } => {
                render_char(*ch, renderer, out);
            }

            InlineNode::CharRef {
                value: CharRef::Replacement(value),
                ..
            } => match replacement_type_of(value) {
                // A replacement the builder produced routes through the renderer,
                // so a custom backend can override the entity it emits.
                Some(type_) => renderer.render_character_replacement(type_, out),

                // A value the builder never produces (only a hand-built node can
                // reach here) still folds losslessly: the general rule every
                // known replacement follows is one decimal numeric entity per
                // logical character.
                None => {
                    for ch in value.chars() {
                        out.push_str(&format!("&#{};", ch as u32));
                    }
                }
            },

            InlineNode::CharRef {
                value: CharRef::Entity(entity),
                ..
            } => {
                // `Entity` carries the character reference exactly as written
                // (`&copy;`); it is already a valid entity, emitted verbatim.
                out.push_str(entity);
            }

            InlineNode::LineBreak { .. } => {
                renderer.render_line_break(out);
            }

            InlineNode::Image(image) => {
                fold_image(image, renderer, context, out);
            }

            InlineNode::Ui(ui) => {
                fold_ui(ui, renderer, context, out);
            }

            InlineNode::Ref(reference) => match reference.variant {
                RefVariant::Link => {
                    fold_link(reference, renderer, context, footnotes, xrefs, out);
                }

                RefVariant::Xref => {
                    fold_xref(reference, renderer, context, footnotes, xrefs, out);
                }
            },

            InlineNode::Anchor(anchor) => {
                fold_anchor(anchor, renderer, context, footnotes, out);
            }

            InlineNode::IndexTerm(index_term) => {
                fold_index_term(index_term, renderer, context, footnotes, xrefs, out);
            }

            InlineNode::Footnote(footnote) => {
                // A heading's own reference text omits the marker entirely —
                // the node is skipped rather than folded to nothing, since
                // `render_footnote` is a backend's to define and a backend
                // that emitted anything would leak it (see
                // [`fold_reference_text`]).
                if footnotes == Footnotes::Marked {
                    fold_footnote(footnote, renderer, out);
                }
            }

            InlineNode::Callout(callout) => {
                fold_callout(callout, renderer, context, out);
            }

            InlineNode::Stem(stem) => {
                fold_stem(stem, renderer, out);
            }

            InlineNode::Styled(styled) => {
                // Fold the children to the body, then wrap it with the same
                // `QuoteType`, attribute list, and id the quotes step
                // recognized, matching Asciidoctor's own output.
                // Sized like `fold_html`'s own buffer: the span's own
                // source extent, which its body's text plus nested markup
                // stays near for typical spans.
                let mut body = String::with_capacity(styled.location.data().len() + 16);
                fold_into_html(
                    &styled.children,
                    renderer,
                    context,
                    footnotes,
                    xrefs,
                    &mut body,
                );

                let scope = match styled.form {
                    SpanForm::Constrained => QuoteScope::Constrained,
                    SpanForm::Unconstrained => QuoteScope::Unconstrained,
                };

                renderer.render_styled(
                    quote_type_of(styled.variant),
                    scope,
                    &styled.attrs,
                    styled.id.as_ref().map(|id| id.to_string()),
                    &body,
                    out,
                );
            }
        }
    }
}

/// Appends `text` — logical, un-escaped text — to `out`, routing each of the
/// three special characters through `renderer` exactly as [`render_char`]
/// does and copying every run between them wholesale.
///
/// This is [`render_char`] applied to every character of `text`, spelled as
/// one `memchr` sweep per special instead of a per-character loop: the runs
/// between specials — for most text, all of it — are appended as one
/// `push_str` each. The output is byte-identical for any renderer, since
/// only the three characters `render_char` routes through the renderer are
/// ever handed to it.
pub(super) fn render_text(text: &str, renderer: &dyn InlineRenderer, out: &mut String) {
    let mut rest = text;

    while let Some(pos) = memchr::memchr3(b'<', b'>', b'&', rest.as_bytes()) {
        // `pos` is the offset of a byte `memchr3` itself just found in
        // `rest`, and each of the three searched-for bytes is ASCII, so the
        // split point is in bounds and on a character boundary: this
        // `split_at` cannot panic.
        let (clean, special_and_rest) = rest.split_at(pos);

        out.push_str(clean);

        let mut chars = special_and_rest.chars();

        // `special_and_rest` starts with the byte just found, so there is
        // always a first character to take, and it is one of the three
        // specials `render_char` routes through the renderer.
        for ch in chars.by_ref().take(1) {
            render_char(ch, renderer, out);
        }

        rest = chars.as_str();
    }

    out.push_str(rest);
}

/// Appends `ch` to `out`, routing the three special characters through
/// `renderer` (so a custom renderer's escaping is honored) and pushing any
/// other character verbatim.
pub(super) fn render_char(ch: char, renderer: &dyn InlineRenderer, out: &mut String) {
    let type_ = match ch {
        '<' => SpecialCharacter::Lt,
        '>' => SpecialCharacter::Gt,
        '&' => SpecialCharacter::Ampersand,

        _ => {
            out.push(ch);
            return;
        }
    };

    renderer.render_special_character(type_, out);
}

/// Folds an [`Image`](InlineNode::Image) node through `render_image`/
/// `render_icon` — handing over the node itself, since
/// `target`, `alt`, `width`/`height` and the restored-range list are all on it.
///
/// The attribute list — which a renderer reads `title`, `link`, `format`, roles
/// and an icon's `size` out of — is on the node too, and unconditionally: a
/// node built without one carries
/// [`Attrlist::empty`](crate::attributes::Attrlist::empty), so neither this
/// fold nor a backend has a fallback to write.
fn fold_image(
    image: &Image<'_>,
    renderer: &dyn InlineRenderer,
    context: &RenderContext,
    out: &mut String,
) {
    if image.is_icon {
        renderer.render_icon(image, context, out);
    } else {
        renderer.render_image(image, context, out);
    }
}

/// Folds a [`Ui`](InlineNode::Ui) node through
/// `render_keyboard`/`render_button`/`render_menu`, reconstructing the render
/// parameters from the keys / label / menu path the macro step captured.
/// `context` is threaded through because rendering a menu reads the document's
/// `icons` attribute to choose the caret between menu levels.
fn fold_ui(ui: &Ui<'_>, renderer: &dyn InlineRenderer, context: &RenderContext, out: &mut String) {
    match &ui.kind {
        UiKind::Keyboard(keys) => {
            renderer.render_keyboard(keys, out);
        }

        UiKind::Button(text) => {
            renderer.render_button(text.as_ref(), out);
        }

        UiKind::Menu {
            menu,
            submenus,
            item,
        } => {
            renderer.render_menu(menu.as_ref(), submenus, item.as_deref(), context, out);
        }
    }
}

/// Folds a link [`Ref`](InlineNode::Ref) node through `render_link` —
/// handing over the node itself,
/// since the target, window, roles (the `bare` class an auto-recognized URL
/// picks up) and attribute list are all on it.
///
/// The one argument beside it is the display text, because that is the **fold
/// of the node's children** and so cannot live on a node: it is a per-render
/// result, not a parse-time fact. `render_link` reads the real attribute list
/// rather than just `roles`/`window`, because an `id`, a `title` and the
/// `nofollow` / `noopener` options come out of it; see [`Ref::attrs`]'s own
/// docs.
fn fold_link(
    reference: &Ref<'_>,
    renderer: &dyn InlineRenderer,
    context: &RenderContext,
    footnotes: Footnotes,
    xrefs: &mut Xrefs<'_>,
    out: &mut String,
) {
    let mut link_text = String::new();
    fold_into_html(
        &reference.children,
        renderer,
        context,
        footnotes,
        xrefs,
        &mut link_text,
    );

    renderer.render_link(reference, &link_text, out);
}

/// Folds a cross-reference [`Ref`](InlineNode::Ref) node through
/// `render_xref`, reconstructing the [`XrefRenderParams`] from the node: the
/// provided text is the fold of the children (empty children ⇒ no text, so
/// the renderer emits the bracketed `[id]` fallback), and the
/// target/window/roles/derived come straight off the node.
///
/// This handles both a same-document reference to a specific id — resolved by
/// a later catalog-resolution pass that writes the outcome back into the
/// node's own [`Ref::resolved`] field, which this fold simply reads — and a
/// target that carries its own destination without a catalog (`derived`,
/// populated at build time — see the `Ref::derived` field docs): an
/// inter-document target, and the empty target naming the current document as
/// a whole. The `xrefstyle` is taken straight off the node, which carries the
/// **effective** style resolved at build time (see the `Ref::xrefstyle` field
/// docs), so this fold consults no document state for it.
fn fold_xref(
    reference: &Ref<'_>,
    renderer: &dyn InlineRenderer,
    context: &RenderContext,
    footnotes: Footnotes,
    xrefs: &mut Xrefs<'_>,
    out: &mut String,
) {
    // A deferring fold additionally captures this reference's segment before
    // rendering it — see [`Xrefs::Deferred`]. The rendering itself still goes
    // straight to `out`, from the segment (not from `reference.children`
    // again, which the segment already folded once via
    // [`xref_segment_from_node`] to compute its own `provided_text` — folding
    // them a second time here would double the renderer's callback count for
    // every non-trivial display text). This is always the reference's own
    // *unresolved* fallback: a fold never has a resolved destination to
    // render instead, since resolution runs later, over the recorded
    // segment, not over a tree mid-fold.
    if let Xrefs::Deferred(segments) = xrefs {
        let segment = xref_segment_from_node(reference, renderer, context);
        render_xref_segment(&segment, renderer, out);
        segments.push(segment);
        return;
    }

    let mut provided = String::new();
    fold_into_html(
        &reference.children,
        renderer,
        context,
        footnotes,
        xrefs,
        &mut provided,
    );

    // Whether a display text was *provided* is the presence of a child, not
    // what that child folds to: the `<<id,>>` shorthand records a
    // present-but-empty text (one empty `Text` child), matching Asciidoctor's
    // own `Some("")` — an empty `<a>…</a>` — which an absent text
    // (`None`, the bracketed `[id]` / reference-text fallback) renders quite
    // differently. Every text the builder recognizes is baked into exactly one
    // child, so the two cases never collide.
    let provided_text = if reference.children.is_empty() {
        None
    } else {
        Some(provided.as_str())
    };

    // `XrefRenderParams::roles` is `&[String]`; the node's `CowStr` roles are
    // materialized into a `String` vector for the borrow.
    let roles: Vec<String> = reference.roles.iter().map(|r| r.to_string()).collect();

    let params = XrefRenderParams {
        target: reference.target.as_ref(),
        provided_text,
        window: reference.window.as_deref(),
        roles: &roles,

        // Taken straight off the node: the *effective* style is resolved into
        // it at build time (see [`Ref::xrefstyle`]), so this fold consults no
        // document state and stays a pure function of the tree — which is what
        // lets it run at reference-resolution time, long after the parse whose
        // `xrefstyle` was in effect.
        xrefstyle: reference.xrefstyle,
        derived: reference.derived.as_ref(),
        resolved: reference.resolved.as_ref(),
    };

    renderer.render_xref(&params, out);
}

/// Folds an [`Anchor`](InlineNode::Anchor) through `render_anchor`. The
/// built-in HTML backend emits only `<a id="…"></a>`, so
/// the reference text never reaches the flow; a custom backend that consults it
/// (e.g. one using it as an `xreflabel`) receives the folded reference text
/// **when the node captured it**.
///
/// That capture is verbatim-only: a *non-verbatim* reference text (a rendered
/// span or an escaped special) is `None` on the node (see
/// `build_anchor_reftext`), so `render_anchor` receives `None` here where
/// Asciidoctor would have passed the substituted text. This changes no HTML
/// output — the built-in backend ignores the reference text entirely — and is
/// the same verbatim boundary the node documents; a custom backend that needs
/// the full reference text unconditionally will get it once a re-flow consumer
/// pins richer `reftext` population.
///
/// A **bibliography** anchor (`[[[label]]]`) passes `None` regardless of what
/// its node carries, matching Asciidoctor's own handling exactly: it calls
/// `render_anchor(id, None, …)` and pushes the bracketed label into the flow
/// itself. The label is in the flow here too — as the sibling nodes following
/// this one (see `biblio_anchor_level`) — so folding the node's `reftext`,
/// which holds that same bracketed label as the entry's *registered* reference
/// text, into `render_anchor` would hand a custom backend a reference text
/// Asciidoctor never passes it.
fn fold_anchor(
    anchor: &Anchor<'_>,
    renderer: &dyn InlineRenderer,
    context: &RenderContext,
    footnotes: Footnotes,
    out: &mut String,
) {
    let reftext = if anchor.is_bibliography {
        None
    } else {
        anchor.reftext.as_ref().map(|children| {
            let mut s = String::new();

            // Always a *rendering*, even under a deferring fold, which is
            // why this function takes no sink to pass along: an anchor's
            // reference text reaches `render_anchor`, not `out`, so a
            // placeholder written here would join the segment list without
            // ever appearing in the template, shifting every later
            // placeholder onto the wrong segment. Not descending is the same
            // call `collect_tree_xref_segments` makes, for the same reason.
            fold_into_html(
                children,
                renderer,
                context,
                footnotes,
                &mut Xrefs::Rendered,
                &mut s,
            );

            s
        })
    };

    renderer.render_anchor(&anchor.id, reftext, out);
}

/// Folds an [`IndexTerm`](InlineNode::IndexTerm) through
/// `render_index_term`.
///
/// A **concealed** term (`visible == false`) has an empty `terms` and renders
/// nothing — the HTML backend generates no index. A **visible** (flow) term
/// carries its shown text as `terms[0]` in the *already-substituted* form the
/// recognizer computed (matching the seam's "already-substituted visible term"
/// contract), so `render_index_term` emits it verbatim and the fold reproduces
/// those bytes exactly.
///
/// A visible term whose text encloses an earlier-recognized construct
/// (`((*tiger*))`) carries it as [`children`](IndexTerm::children) instead, and
/// this folds them — with the same `renderer`, so the enclosed span's markup is
/// whatever the surrounding flow's would be — into the same
/// already-substituted string the seam takes. That is the one thing a
/// build-time `terms[0]` could not hold: a span's markup exists only at fold
/// time. It is the relationship [`fold_link`]'s own `link_text` has to
/// [`Ref::children`](crate::inlines::Ref::children), reached for the same
/// reason. The two are never both populated, so the branch is a straight
/// either/or rather than a precedence rule.
///
/// The node's `terms` mirrors what the
/// (test-only) `inline_tree` recorder stores — the single
/// shown term for a visible node, empty for a concealed one; the richer
/// primary/secondary/tertiary structure the field can hold is left to a re-flow
/// consumer to pin (the field is provisional, per its own doc comment),
/// exactly as an anchor's `reftext` is.
fn fold_index_term(
    index_term: &IndexTerm<'_>,
    renderer: &dyn InlineRenderer,
    context: &RenderContext,
    footnotes: Footnotes,
    xrefs: &mut Xrefs<'_>,
    out: &mut String,
) {
    let mut folded_children = String::new();

    let visible_term = if index_term.visible {
        if index_term.children.is_empty() {
            Some(index_term.terms.first().map_or("", CowStr::as_ref))
        } else {
            fold_into_html(
                &index_term.children,
                renderer,
                context,
                footnotes,
                xrefs,
                &mut folded_children,
            );
            Some(folded_children.as_str())
        }
    } else {
        None
    };

    renderer.render_index_term(index_term, visible_term, out);
}

/// Folds a [`Footnote`](InlineNode::Footnote) through
/// `render_footnote` — handing over the node itself,
/// since everything the marker needs is on it (`is_reference`, `number`, `id`)
/// and no build-time state is required.
///
/// Only the in-flow **marker** is folded here (`[1]`, or `[id]` for an
/// unresolved reference): `render_footnote` never emits the footnote's own
/// text into the flow — that text belongs in the document's separate
/// footnote list, a concern outside a single block's fold — so
/// `footnote.children` is not folded into `out` at all, the same relationship
/// [`fold_anchor`]'s `reftext` has to its own marker.
///
/// Which of the three markers a node produces is now the *renderer's* reading
/// of it rather than this fold's — see
/// [`render_footnote`](crate::parser::InlineRenderer::render_footnote)'s own
/// documentation for the three cases, and `HtmlInlineRenderer` for the reading
/// the built-in backend makes.
fn fold_footnote(footnote: &Footnote<'_>, renderer: &dyn InlineRenderer, out: &mut String) {
    renderer.render_footnote(footnote, out);
}

/// Folds a [`Callout`](InlineNode::Callout) through `render_callout`,
/// handing over the node itself — no build-time state is needed, mirroring
/// [`fold_footnote`]. The node was already the canonical structured record,
/// so the render-params struct this fold used to rebuild from it (along with
/// a second `CalloutGuard` differing only in whether the prefix was a
/// `CowStr` or a `&str`) is gone.
fn fold_callout(
    callout: &Callout<'_>,
    renderer: &dyn InlineRenderer,
    context: &RenderContext,
    out: &mut String,
) {
    renderer.render_callout(callout, context, out);
}

/// Folds a [`Stem`](InlineNode::Stem) through `render_styled`. The node's
/// `value` already carries the resolved substitution group's output (special
/// characters only, by default), so the fold passes it straight through as
/// the body — no further processing is needed, matching how Asciidoctor
/// restores a STEM passthrough with no attribute list or id
/// (`INLINE_STEM_MACRO` captures neither).
///
/// Shared with the macro families, whose *restore* of a masked STEM
/// expression into a computed target must emit exactly the bytes this fold
/// does — see
/// [`restorable_body`](super::macros::image::restorable_body) — so the two
/// directions cannot drift.
pub(in crate::content::inline_builder) fn fold_stem(
    stem: &Stem<'_>,
    renderer: &dyn InlineRenderer,
    out: &mut String,
) {
    let type_ = match stem.notation {
        StemNotation::AsciiMath => QuoteType::AsciiMath,
        StemNotation::LatexMath => QuoteType::LatexMath,
    };

    // A STEM macro carries no attribute list of its own, and the string
    // pipeline's passthrough restore passed none either.
    let attrlist = Attrlist::empty(stem.location.slice(0..0));

    renderer.render_styled(
        type_,
        QuoteScope::Unconstrained,
        &attrlist,
        None,
        stem.value.as_ref(),
        out,
    );
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use super::{
        super::test_support::{build_src, fold_html},
        fold_html as fold_html_with_context,
    };
    use crate::{
        Parser, Span,
        inlines::{
            CharRef, Image, InlineNode, PassthroughOrigin, RawForm, RawOrigin, Ref, RefVariant,
        },
        parser::{
            HtmlInlineRenderer, ModificationContext, ResolvedReference, XrefSignifier, XrefStyle,
        },
        strings::CowStr,
    };

    #[test]
    fn fold_reference_text_omits_a_headings_footnote_markers() {
        // The tree's answer to the footnote-marker sentinel system that was
        // deleted: the two strings `Section::parse` needs — the
        // rendering the heading shows, and the footnote-free text its reference
        // and auto-generated id are derived from — are two *folds of the same
        // tree*, and "which regions were footnote markers" is a question about
        // node kinds rather than about bytes.
        //
        // Every fixture is checked from both ends. The heading's own rendering
        // is compared against `Content::rendered_html()` for the same source
        // under the `Title` group — the public entry point's own
        // build-and-fold of the same tree, confirming the two paths agree.
        // The footnote-free reference text is compared against a **literal**
        // expected string: with the sentinel strip gone there is no second
        // implementation left to differentiate against, so the expectation is
        // written down instead — these are the exact bytes that strip
        // produced, captured before it was deleted.
        let fixtures = [
            // No footnote at all: the two strings are the same.
            ("Plain title", "Plain title"),
            ("A *bold* title", "A <strong>bold</strong> title"),
            ("Tom & Jerry", "Tom &amp; Jerry"),
            // The shape that names this: a footnote in a heading.
            ("Section 2footnote:[second footnote]", "Section 2"),
            ("footnote:[leading] Section", " Section"),
            ("Section footnote:[middle] title", "Section  title"),
            ("A footnote:[one] and footnote:[two] title", "A  and  title"),
            // A named footnote.
            ("Named.footnote:disc[a discussion]", "Named."),
            // The marker nested inside constructs the omission has to recurse
            // through: a rendered span, and a cross-reference's display text.
            (
                "A *bold footnote:[inside a span] title*",
                "A <strong>bold  title</strong>",
            ),
            (
                "See xref:sec[the footnote:[inside a text] steps] here",
                "See <a href=\"#sec\">the footnote:[inside a text</a> steps] here",
            ),
            // Beside the other constructs a title can carry, so the omission is
            // shown to remove the marker and nothing else.
            (
                "A footnote:[note] and image:x.png[Alt] and `code`",
                "A  and <span class=\"image\"><img src=\"x.png\" alt=\"Alt\"></span> and <code>code</code>",
            ),
            (
                "A footnote:[note] and a (C) and an -- em dash",
                "A  and a &#169; and an&#8201;&#8212;&#8201;em dash",
            ),
            (
                "A footnote:[note] and a +++<b>raw</b>+++ passthrough",
                "A  and a <b>raw</b> passthrough",
            ),
        ];

        let renderer = HtmlInlineRenderer {};

        for (fixture, expected_reftext) in fixtures {
            // Two independent parsers, since a footnote's number is document
            // state: one for each side of the comparison.
            let golden_parser = Parser::default();
            let mut golden = crate::content::Content::from(Span::new(fixture));
            crate::content::SubstitutionGroup::Title.apply(&mut golden, &golden_parser, None);

            // The `golden` side above went through `SubstitutionGroup::apply`
            // (the authoritative pass — fold, catalog registration, the
            // works); this side calls `build_for_group` directly to get the
            // same tree without any of that, then folds it independently
            // through `fold_html_with_context` below. Comparing the two
            // outputs is exactly what proves the two code paths agree — see
            // `build_for_group`'s own doc comment for why they're two paths
            // in the first place.
            let built_parser = Parser::default();
            let nodes = crate::content::inline_builder::build_for_group(
                &crate::content::SubstitutionGroup::Title,
                CowStr::from(fixture),
                Span::new(fixture),
                &built_parser,
                None,
            );

            assert_eq!(
                fold_html_with_context(&nodes, &renderer, &built_parser.render_context()),
                golden.rendered_html(),
                "the heading's own rendering diverged for {fixture:?}"
            );

            assert_eq!(
                super::fold_reference_text(&nodes, &renderer, &built_parser.render_context()),
                expected_reftext,
                "the footnote-free reference text diverged for {fixture:?}"
            );
        }
    }

    #[test]
    fn fold_emits_raw_verbatim() {
        // A `Raw` leaf is emitted without HTML-escaping, unlike `Text`; its
        // `<`, `>`, and `&` pass straight through.
        let location = Span::new("<b>raw &amp;</b>");

        let raw = InlineNode::Raw {
            value: CowStr::from(location.data()),
            form: RawForm::AsIs,
            origin: RawOrigin::Passthrough(Box::new(PassthroughOrigin {
                subs: crate::content::SubstitutionGroup::None,
                source_text: None,
            })),
            location,
        };

        assert_eq!(
            fold_html(&[raw], &HtmlInlineRenderer {}),
            "<b>raw &amp;</b>"
        );
    }

    #[test]
    fn fold_encodes_an_unknown_replacement_value_per_character() {
        // A `CharRef::Replacement` value the builder never produces (only a
        // hand-built node can carry one) still folds losslessly: one decimal
        // numeric entity per logical character. Here `z` is U+007A.
        let location = Span::new("z");

        let node = InlineNode::CharRef {
            value: CharRef::Replacement("z"),
            location,
        };

        assert_eq!(fold_html(&[node], &HtmlInlineRenderer {}), "&#122;");
    }

    #[test]
    fn fold_folds_a_hand_built_image_without_an_attribute_list() {
        // The node types are public, so a consumer may hand-build an
        // [`Image`](InlineNode::Image) directly rather than through the macro
        // step — with no [`Attrlist`]. The fold handles that by folding through
        // an empty attribute list sliced from the node's own location, so a
        // hand-built image renders like the same macro would. (The macro step
        // always attaches an attribute list, so only a hand-built node reaches
        // this branch, exactly as [`fold_encodes_an_unknown_replacement_value_
        // per_character`] exercises a hand-built replacement.)
        let location = Span::new("image:sunset.jpg[Sunset]");

        let hand_built = InlineNode::Image(Box::new(Image {
            is_icon: false,
            target: CowStr::from("sunset.jpg"),
            restored_target_ranges: vec![],
            alt: Some(CowStr::from("Sunset")),
            width: None,
            height: None,
            attrs: crate::attributes::Attrlist::empty(location.slice(0..0)),
            location,
        }));

        // The macro-built equivalent (which carries an attribute list) is the
        // oracle: the two must fold identically.
        let renderer = HtmlInlineRenderer {};
        let macro_built = fold_html(&build_src(location), &renderer);

        assert_eq!(fold_html(&[hand_built], &renderer), macro_built);
    }

    #[test]
    fn fold_skips_a_hand_built_restored_range_off_the_target() {
        // The node types are public, so a consumer may hand-build an
        // [`Image`](InlineNode::Image) whose `restored_target_ranges` do not
        // fall on the target's own bytes (the builder never produces one).
        // The renderer's masking skips such a range rather than splitting the
        // target, so the fold still resolves and renders the plain path.
        let location = Span::new("image:sunset.jpg[Sunset]");

        let hand_built = InlineNode::Image(Box::new(Image {
            is_icon: false,
            target: CowStr::from("sunset.jpg"),
            restored_target_ranges: vec![3..99, 100..200],
            alt: Some(CowStr::from("Sunset")),
            width: None,
            height: None,
            attrs: crate::attributes::Attrlist::empty(location.slice(0..0)),
            location,
        }));

        let renderer = HtmlInlineRenderer {};

        assert!(
            fold_html(&[hand_built], &renderer).contains(r#"src="sunset.jpg""#),
            "a range off the target's bytes must not disturb the src"
        );
    }

    /// A resolved cross-reference to a target that carries a signifier (a
    /// numbered/captioned element), with no explicit display text — the shape
    /// `xrefstyle` formatting actually changes (design's `apply_xrefstyle`
    /// only alters output when a signifier is present).
    fn resolved_xref_with_signifier(xrefstyle: Option<XrefStyle>) -> InlineNode<'static> {
        InlineNode::Ref(Box::new(Ref {
            variant: RefVariant::Xref,
            link_form: None,
            target: CowStr::from("install"),
            children: vec![],
            roles: vec![],
            window: None,
            resolved: Some(ResolvedReference {
                href: "#install".to_string(),
                text: None,
                signifier: Some(XrefSignifier {
                    label: "Section 2".to_string(),
                    emphasize: false,
                }),
            }),
            derived: None,
            xrefstyle,
            attrs: crate::attributes::Attrlist::empty(Span::new("").slice(0..0)),
            location: Span::new(""),
        }))
    }

    #[test]
    fn fold_xref_reads_the_effective_xrefstyle_off_the_node() {
        // The fold consults **no** document state for `xrefstyle`: the
        // effective style is resolved into the node at build time (see the
        // `Ref::xrefstyle` field docs), so the same node folds the same way
        // whatever the parser it is handed says.
        //
        // That is the property the deferred-cross-reference retirement needs.
        // A re-fold runs at reference-resolution time, and the document-wide
        // `xrefstyle` in effect *there* is whatever the last `:xrefstyle:` line
        // in the document left set — not what was in effect where the
        // reference was written. Reading it at fold time would therefore
        // silently re-style a reference that was already styled correctly at
        // the point it was written.
        let renderer = HtmlInlineRenderer {};

        // A parser whose document-wide `xrefstyle` says `full`, which the
        // fold must ignore in both directions.
        let full = Parser::default().with_intrinsic_attribute(
            "xrefstyle",
            "full",
            ModificationContext::Anywhere,
        );

        // `None` on the node means "no style", not "ask the document".
        let unstyled = fold_html_with_context(
            &[resolved_xref_with_signifier(None)],
            &renderer,
            &full.render_context(),
        );

        assert!(!unstyled.contains("Section 2"), "folded: {unstyled}");

        // And a node carrying a style is honored even by a parser that has
        // none set.
        let styled = fold_html_with_context(
            &[resolved_xref_with_signifier(Some(XrefStyle::Full))],
            &renderer,
            &Parser::default().render_context(),
        );

        assert!(styled.contains("Section 2, &#8220;"), "folded: {styled}");
    }

    #[test]
    fn fold_xref_honors_each_style_the_node_can_carry() {
        // The complement of the test above: a node carrying `Short` folds as
        // `Short`, with the document-wide `full` again ignored.
        let renderer = HtmlInlineRenderer {};
        let nodes = [resolved_xref_with_signifier(Some(XrefStyle::Short))];

        let parser = Parser::default().with_intrinsic_attribute(
            "xrefstyle",
            "full",
            ModificationContext::Anywhere,
        );

        let folded = fold_html_with_context(&nodes, &renderer, &parser.render_context());

        // `Short` shows only the signifier label, with none of `Full`'s
        // quoted-title suffix.
        assert!(folded.contains(">Section 2<"), "folded: {folded}");
        assert!(!folded.contains("&#8220;"), "folded: {folded}");
    }

    #[test]
    fn render_text_matches_render_char_character_by_character() {
        // `render_text` is `render_char` applied to every character, spelled
        // as a bulk sweep; this pins the two as byte-identical across the
        // shapes the sweep has to get right — no special at all, specials at
        // either edge, adjacent specials, and multi-byte characters between
        // them. The per-character loop is also what keeps `render_char`'s
        // ordinary-character arm exercised now that production callers hand
        // it only specials.
        let renderer = HtmlInlineRenderer {};

        for text in [
            "",
            "plain text with nothing special",
            "a < b & c > d",
            "<&>",
            "&&&",
            "ends with a special <",
            "> starts with one",
            "unicode — bullet • and a < special",
        ] {
            let mut bulk = String::new();
            super::render_text(text, &renderer, &mut bulk);

            let mut per_char = String::new();

            for ch in text.chars() {
                super::render_char(ch, &renderer, &mut per_char);
            }

            assert_eq!(bulk, per_char, "render_text diverged for {text:?}");
        }
    }
}
