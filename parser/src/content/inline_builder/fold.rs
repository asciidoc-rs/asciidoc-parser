//! The HTML-folding step: renders the built tree back to output bytes.

use super::{callouts::replacement_type_of, quotes::quote_type_of};
use crate::{
    attributes::Attrlist,
    content::{Content, XrefSegment, xref_segment_from_node},
    inlines::{
        Anchor, Callout, CharRef, Footnote, Image, IndexTerm, InlineNode, RawForm, Ref, RefVariant,
        SpanForm, Stem, StemNotation, Ui, UiKind,
    },
    parser::{
        IndexTermRenderParams, InlineRenderer, LinkRenderParams, QuoteScope, QuoteType,
        RenderContext, SpecialCharacter, XrefRenderParams,
    },
    strings::CowStr,
};

/// Folds an inline node tree to output bytes through `renderer`.
///
/// This is the fold over the *public* [`InlineNode`] tree. It handles the node
/// kinds the transducer steps produce so far — [`Text`](InlineNode::Text),
/// [`CharRef`](InlineNode::CharRef), [`Styled`](crate::inlines::Styled),
/// [`Image`](InlineNode::Image), [`Ui`](InlineNode::Ui),
/// [`Ref`](InlineNode::Ref) (both link and cross-reference),
/// [`Anchor`](InlineNode::Anchor), [`IndexTerm`](InlineNode::IndexTerm),
/// [`Footnote`](InlineNode::Footnote), [`Callout`](InlineNode::Callout),
/// [`Stem`](InlineNode::Stem), and [`LineBreak`](InlineNode::LineBreak), plus
/// the design-legal [`Raw`](InlineNode::Raw) leaf; a later increment extends
/// it as the transducer grows new kinds.
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
    let mut out = String::new();
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
/// [`Deferred`](Self::Deferred) is the placeholder-emitting mode — see
/// [`fold_deferring_xrefs`], which is the only way to reach it.
pub(crate) enum Xrefs<'a> {
    /// Render each cross-reference in place, through `render_xref`: the
    /// ordinary fold, which is what a block's flow shows.
    Rendered,

    /// Write a placeholder where each cross-reference stands, appending the
    /// segment that will fill it to this list. The placeholder's index is the
    /// segment's position in the list, so a template and its list are built in
    /// one pass and cannot fall out of order.
    Deferred(&'a mut Vec<XrefSegment>),
}

/// Folds `nodes` into a **placeholder template** plus the cross-reference
/// segments that fill it, in placeholder order — the pair
/// [`FootnoteDeferred`](crate::content::FootnoteDeferred) is built from.
///
/// A footnote's text is lifted out of the block it was written in, so a
/// cross-reference inside it is never reached by the document-order pass that
/// resolves the block's own references; the footnote has to carry its
/// references itself, as a template plus a segment list, and resolve them from
/// the catalog later ([`Footnote::resolve_references`]). The string pipeline
/// reaches that pair by *re-homing* the block template's own placeholders,
/// which are already sitting in the footnote's captured text
/// (its `rehome_xref_placeholders`).
/// The tree has no such placeholders to re-home — a cross-reference is a node
/// — so it writes its own: this fold emits one per
/// [`Xref`](RefVariant::Xref) node and records that node's segment as it goes.
///
/// # Why the template is built at *registration* time
///
/// A footnote's catalog entry is registered when the footnote is recognized,
/// which is the same moment the string replacer registers its own. That is not
/// a convenience: `Footnote::resolve_references` runs on the **catalog entry**,
/// driven from the catalog, with no access to the tree the footnote came from,
/// so nothing later in the parse can go back and derive the pair. Recognition
/// is also the last moment the subtree is still final —
/// [`apply_post_replacements`](super::apply_post_replacements) descends into a
/// [`Styled`](crate::inlines::Styled)/[`Ref`](InlineNode::Ref) child but not
/// into a [`Footnote`](InlineNode::Footnote)'s, exactly as the string pipeline
/// has the footnote's text out of the flat string by then.
///
/// # This is a *build-time* fold, and the only one
///
/// Building the tree is otherwise unobservable — it consults no renderer, a
/// property `tests::inline_tree`'s own
/// `building_the_tree_does_not_consult_the_documents_renderer` measures with a
/// stateful renderer. A footnote is the documented exception, and not by
/// choice: its catalog entry is a **required** recognition side effect (the
/// same reason its number is), and that entry's payload is a *rendered
/// string*, so registering it and rendering it are one act. The string
/// pipeline does exactly the same thing at exactly the same moment — its
/// footnote replacer cuts already-rendered bytes out of the flat string it is
/// substituting.
///
/// This adds no second rendering of the subtree. `fold_footnote` writes only
/// the in-flow **marker**, never the footnote's children, so the block's own
/// fold does not re-render what this one rendered: a footnote's subtree is
/// folded exactly once per parse, here — the same once the string pipeline
/// spends on it.
///
/// [`Footnote::resolve_references`]: crate::document::Footnote::resolve_references
pub(crate) fn fold_deferring_xrefs(
    nodes: &[InlineNode<'_>],
    renderer: &dyn InlineRenderer,
    context: &RenderContext,
) -> (String, Vec<XrefSegment>) {
    let mut out = String::new();
    let mut segments = Vec::new();

    // The footnote axis is inert here whichever way it is set: `nodes` is one
    // footnote's own children, and footnotes do not nest (the string
    // pipeline's lazy bracket match cannot recognize one inside another's
    // content either), so no `Footnote` node can be reached. `Marked` is the
    // ordinary fold, which is the honest default for a mode nothing consults.
    fold_into_html(
        nodes,
        renderer,
        context,
        Footnotes::Marked,
        &mut Xrefs::Deferred(&mut segments),
        &mut out,
    );

    (out, segments)
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
/// for a heading's reference text, and the whole sentinel system (design
/// §4.2's **first of three**) is deleted.
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
                for ch in value.chars() {
                    render_char(ch, renderer, out);
                }
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
                for ch in value.chars() {
                    render_char(ch, renderer, out);
                }
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

            InlineNode::Ref(reference) if reference.variant == RefVariant::Link => {
                fold_link(reference, renderer, context, footnotes, xrefs, out);
            }

            InlineNode::Ref(reference) if reference.variant == RefVariant::Xref => {
                fold_xref(reference, renderer, context, footnotes, xrefs, out);
            }

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
                // Fold the children to the body, then wrap it exactly as the
                // string pipeline's quotes step did: the same `QuoteType`,
                // attribute list, and id it recognized, so the bytes match.
                let mut body = String::new();
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

            other => {
                // The steps wired up so far produce only `Text`,
                // `CharRef::Special`, `Styled`, `Image`, `Ui`, `Ref` (link and
                // cross-reference), `Anchor`, `IndexTerm`, `Footnote`,
                // `Callout`, `Stem`, and `LineBreak` nodes, and this fold
                // additionally emits the design-legal `Raw` leaf; no other
                // node kind reaches the fold in this increment. A later
                // increment fills in the arms above as the transducer grows
                // new kinds.
                // Guard against a premature caller in debug builds and emit
                // nothing in release, mirroring the safe defensive fallback in
                // [`content`](super::content).
                debug_assert!(
                    false,
                    "inline_builder::fold_html reached an unsupported node kind: {other:?}"
                );
            }
        }
    }
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

/// Folds an [`Image`](InlineNode::Image) node through the same
/// `render_image`/`render_icon` the string pipeline's macros step calls, so the
/// output is byte-for-byte identical — handing over the node itself, since
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

/// Folds a [`Ui`](InlineNode::Ui) node through the same
/// `render_keyboard`/`render_button`/`render_menu` the string pipeline's macros
/// step calls, reconstructing the render parameters from the keys / label /
/// menu path the macro step captured, so the output is byte-for-byte identical.
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

/// Folds a link [`Ref`](InlineNode::Ref) node through the same `render_link`
/// the string pipeline's macros step calls, reconstructing the
/// [`LinkRenderParams`] from the node: the display text is the fold of the
/// children, the extra roles (`bare`) ride on the node's `roles`, and the
/// target/window and the attribute list come straight off the node. That list
/// is [`Ref::attrs`], which is always present — empty when the display text
/// carried none — and `render_link` needs the real thing rather than just
/// `roles`/`window`, because it reads an `id`, a `title` and the `nofollow` /
/// `noopener` options out of it; see that field's own docs.
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

    let extra_roles: Vec<&str> = reference.roles.iter().map(|r| r.as_ref()).collect();

    let params = LinkRenderParams {
        target: reference.target.to_string(),
        link_text,
        extra_roles,
        window: reference.window.as_deref(),
        attrlist: &reference.attrs,
        context,
    };

    renderer.render_link(&params, out);
}

/// Folds a cross-reference [`Ref`](InlineNode::Ref) node through the same
/// `render_xref` the string pipeline's macros step feeds at resolution time,
/// reconstructing the [`XrefRenderParams`] from the node: the provided text is
/// the fold of the children (empty children ⇒ no text, so the renderer emits
/// the bracketed `[id]` fallback), and the target/window/roles/derived come
/// straight off the node.
///
/// This increment recognizes both a same-document reference to a specific id
/// (unresolved — no catalog-resolution pass runs, so `resolved` is always
/// `None` here) and a target that carries its own destination without a
/// catalog (`derived`, populated at build time — see the `Ref::derived` field
/// docs): an inter-document target, and the empty target naming the current
/// document as a whole. The `xrefstyle` is taken straight off the node, which
/// carries the **effective** style resolved at build time (see the
/// `Ref::xrefstyle` field docs), so this fold consults no document state for
/// it; the cutover (design §5.2 Phase 4, step 6) wires catalog resolution to
/// the tree.
fn fold_xref(
    reference: &Ref<'_>,
    renderer: &dyn InlineRenderer,
    context: &RenderContext,
    footnotes: Footnotes,
    xrefs: &mut Xrefs<'_>,
    out: &mut String,
) {
    // A deferring fold writes a placeholder here instead of a rendering, and
    // captures the segment that will fill it — see [`Xrefs::Deferred`]. The
    // children are *not* folded into `out`: they are this reference's own
    // display text, which the segment carries (rendered, by
    // [`xref_segment_from_node`]) rather than the flow.
    if let Xrefs::Deferred(segments) = xrefs {
        let index = segments.len();
        segments.push(xref_segment_from_node(reference, renderer, context));
        out.push_str(&Content::xref_placeholder(index));
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
    // present-but-empty text (one empty `Text` child) that the string replacer
    // carries as `Some("")` — an empty `<a>…</a>` — which an absent text
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

/// Folds an [`Anchor`](InlineNode::Anchor) through the same `render_anchor` the
/// string step calls. The built-in HTML backend emits only `<a id="…"></a>`, so
/// the reference text never reaches the flow; a custom backend that consults it
/// (e.g. one using it as an `xreflabel`) receives the folded reference text
/// **when the node captured it**.
///
/// That capture is verbatim-only: a *non-verbatim* reference text (a rendered
/// span or an escaped special) is `None` on the node (see
/// `build_anchor_reftext`), so `render_anchor` receives `None` here where the
/// string replacer would have passed the substituted text. This changes no HTML
/// output — the built-in backend ignores the reference text entirely — and is
/// the same verbatim boundary the node documents; a custom backend that needs
/// the full reference text unconditionally will get it once a re-flow consumer
/// pins richer `reftext` population.
///
/// A **bibliography** anchor (`[[[label]]]`) passes `None` regardless of what
/// its node carries, mirroring its own replacer exactly: that pass calls
/// `render_anchor(id, None, …)` and pushes the bracketed label into the flow
/// itself. The label is in the flow here too — as the sibling nodes following
/// this one (see `biblio_anchor_level`) — so folding the node's `reftext`,
/// which holds that same bracketed label as the entry's *registered* reference
/// text, into `render_anchor` would hand a custom backend a reference text the
/// string pipeline never passes it.
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

/// Folds an [`IndexTerm`](InlineNode::IndexTerm) through the same
/// `render_index_term` the string step's macros pass calls.
///
/// A **concealed** term (`visible == false`) has an empty `terms` and renders
/// nothing — the HTML backend generates no index. A **visible** (flow) term
/// carries its shown text as `terms[0]` in the *already-substituted* form the
/// recognizer computed (matching the seam's "already-substituted visible term"
/// contract), so `render_index_term` emits it verbatim and the fold reproduces
/// the string pipeline's bytes exactly.
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
/// consumer to pin (the field is provisional, per the node's Phase-0 note),
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

    renderer.render_index_term(&IndexTermRenderParams { visible_term }, out);
}

/// Folds a [`Footnote`](InlineNode::Footnote) through the same
/// `render_footnote` the string step calls — handing over the node itself,
/// since everything the marker needs is on it (`is_reference`, `number`, `id`)
/// and no build-time state is required (design §3.3.1).
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

/// Folds a [`Callout`](InlineNode::Callout) through the same `render_callout`
/// the string step's `Callouts` group calls, handing over the node itself —
/// no build-time state is needed, mirroring [`fold_footnote`]. This is design
/// §4.6's Phase 5 reshape arrived at: the node was already the canonical
/// structured record, and the render-params struct this fold used to rebuild
/// from it (along with a second `CalloutGuard` differing only in whether the
/// prefix was a `CowStr` or a `&str`) is gone.
fn fold_callout(
    callout: &Callout<'_>,
    renderer: &dyn InlineRenderer,
    context: &RenderContext,
    out: &mut String,
) {
    renderer.render_callout(callout, context, out);
}

/// Folds a [`Stem`](InlineNode::Stem) through the same
/// `render_styled` the string pipeline's passthrough-restore step
/// calls for a STEM entry (design §3.3.1's fold-time seam). The node's `value`
/// already carries the resolved substitution group's output (special
/// characters only, by default), so the fold passes it straight through as
/// the body — no further processing is needed, mirroring how a STEM
/// passthrough is restored with no attribute list or id (`INLINE_STEM_MACRO`
/// captures neither).
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
        inlines::{CharRef, Image, InlineNode, RawForm, RawOrigin, Ref, RefVariant},
        parser::{
            HtmlInlineRenderer, ModificationContext, ResolvedReference, XrefSignifier, XrefStyle,
        },
        strings::CowStr,
    };

    #[test]
    fn fold_reference_text_omits_a_headings_footnote_markers() {
        // The tree's answer to the footnote-marker sentinel system this
        // increment deleted: the two strings `Section::parse` needs — the
        // rendering the heading shows, and the footnote-free text its reference
        // and auto-generated id are derived from — are two *folds of the same
        // tree*, and "which regions were footnote markers" is a question about
        // node kinds rather than about bytes.
        //
        // Every fixture is checked from both ends. The heading's own rendering
        // is compared against the string pipeline, which still produces it and
        // is the golden-HTML oracle (§5.3). The footnote-free reference text is
        // compared against a **literal** expected string: with the sentinel
        // strip gone there is no second implementation left to differentiate
        // against, so the expectation is written down instead — these are the
        // exact bytes that strip produced, captured before it was deleted.
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
            // state: one for each side of the comparison (design §5.3).
            let golden_parser = Parser::default();
            let mut golden = crate::content::Content::from(Span::new(fixture));
            crate::content::SubstitutionGroup::Title.apply(&mut golden, &golden_parser, None);

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
        // A `Raw` leaf is emitted without HTML-escaping, unlike `Text`; its `<`,
        // `>`, and `&` pass straight through.
        let location = Span::new("<b>raw &amp;</b>");

        let raw = InlineNode::Raw {
            value: CowStr::from(location.data()),
            form: RawForm::AsIs,
            origin: RawOrigin::Passthrough {
                subs: crate::content::SubstitutionGroup::None,
                source_text: None,
            },
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

        let hand_built = InlineNode::Image(Image {
            is_icon: false,
            target: CowStr::from("sunset.jpg"),
            restored_target_ranges: vec![],
            alt: Some(CowStr::from("Sunset")),
            width: None,
            height: None,
            attrs: crate::attributes::Attrlist::empty(location.slice(0..0)),
            location,
        });

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

        let hand_built = InlineNode::Image(Image {
            is_icon: false,
            target: CowStr::from("sunset.jpg"),
            restored_target_ranges: vec![3..99, 100..200],
            alt: Some(CowStr::from("Sunset")),
            width: None,
            height: None,
            attrs: crate::attributes::Attrlist::empty(location.slice(0..0)),
            location,
        });

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
        InlineNode::Ref(Ref {
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
        })
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
        // silently re-style a reference the string pipeline had already styled.
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
}
