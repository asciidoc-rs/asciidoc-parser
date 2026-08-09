//! The HTML-folding step: renders the built tree back to output bytes.

use super::{callouts::replacement_type_of, quotes::quote_type_of};
use crate::{
    Parser,
    attributes::{Attrlist, AttrlistContext},
    inlines::{
        Anchor, Callout, CalloutGuard, CharRef, Footnote, Image, IndexTerm, InlineNode, Ref,
        RefVariant, SpanForm, Stem, StemNotation, Ui, UiKind,
    },
    parser::{
        CalloutGuard as ParserCalloutGuard, CalloutRenderParams, FootnoteRenderParams,
        IconRenderParams, ImageRenderParams, IndexTermRenderParams, InlineSubstitutionRenderer,
        LinkRenderParams, MenuRenderParams, QuoteScope, QuoteType, SpecialCharacter,
        XrefRenderParams,
    },
    strings::CowStr,
};

/// Folds an inline node tree to output bytes through `renderer`.
///
/// This is the fold over the *public* [`InlineNode`] tree. It handles the node
/// kinds the transducer steps produce so far – [`Text`](InlineNode::Text),
/// [`CharRef`](InlineNode::CharRef), [`Styled`](crate::inlines::Styled),
/// [`Image`](InlineNode::Image), [`Ui`](InlineNode::Ui),
/// [`Ref`](InlineNode::Ref) (both link and cross-reference),
/// [`Anchor`](InlineNode::Anchor), [`IndexTerm`](InlineNode::IndexTerm),
/// [`Footnote`](InlineNode::Footnote), [`Callout`](InlineNode::Callout),
/// [`Stem`](InlineNode::Stem), and [`LineBreak`](InlineNode::LineBreak), plus
/// the design-legal [`Raw`](InlineNode::Raw) leaf; a later increment extends
/// it as the transducer grows new kinds.
pub(crate) fn fold_html(
    nodes: &[InlineNode<'_>],
    renderer: &dyn InlineSubstitutionRenderer,
    parser: &Parser,
) -> String {
    let mut out = String::new();
    fold_into_html(nodes, renderer, parser, &mut out);
    out
}

/// Appends the fold of `nodes` to `out` (the recursive worker for
/// [`fold_html`]).
///
/// `parser` is threaded through because rendering some nodes – an
/// [`Image`](InlineNode::Image), whose `render_image`/`render_icon` reads the
/// document's safe mode, `data-uri`, and `icons`/`icontype` attributes – is a
/// function of document context, not of the node alone.
fn fold_into_html(
    nodes: &[InlineNode<'_>],
    renderer: &dyn InlineSubstitutionRenderer,
    parser: &Parser,
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

            InlineNode::Raw { value, .. } => {
                out.push_str(value);
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
                fold_image(image, renderer, parser, out);
            }

            InlineNode::Ui(ui) => {
                fold_ui(ui, renderer, parser, out);
            }

            InlineNode::Ref(reference) if reference.variant == RefVariant::Link => {
                fold_link(reference, renderer, parser, out);
            }

            InlineNode::Ref(reference) if reference.variant == RefVariant::Xref => {
                fold_xref(reference, renderer, parser, out);
            }

            InlineNode::Anchor(anchor) => {
                fold_anchor(anchor, renderer, parser, out);
            }

            InlineNode::IndexTerm(index_term) => {
                fold_index_term(index_term, renderer, out);
            }

            InlineNode::Footnote(footnote) => {
                fold_footnote(footnote, renderer, out);
            }

            InlineNode::Callout(callout) => {
                fold_callout(callout, renderer, parser, out);
            }

            InlineNode::Stem(stem) => {
                fold_stem(stem, renderer, out);
            }

            InlineNode::Styled(styled) => {
                // Fold the children to the body, then wrap it exactly as the
                // string pipeline's quotes step did: the same `QuoteType`,
                // attribute list, and id it recognized, so the bytes match.
                let mut body = String::new();
                fold_into_html(&styled.children, renderer, parser, &mut body);

                let scope = match styled.form {
                    SpanForm::Constrained => QuoteScope::Constrained,
                    SpanForm::Unconstrained => QuoteScope::Unconstrained,
                };

                renderer.render_quoted_substitution(
                    quote_type_of(styled.variant),
                    scope,
                    styled.attrs.clone(),
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
fn render_char(ch: char, renderer: &dyn InlineSubstitutionRenderer, out: &mut String) {
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

/// Folds an [`Image`](InlineNode::Image) node, reconstructing the
/// [`ImageRenderParams`]/[`IconRenderParams`] the string pipeline built and
/// calling the same `render_image`/`render_icon`, so the output is
/// byte-for-byte identical. The pre-extracted `alt` (and, for an image,
/// `width`/`height`) come straight off the node; an icon's `size` and every
/// other rendered attribute (`title`, `link`, `format`, roles, …) are read back
/// from the node's own [`Attrlist`] – which is why the macro step
/// captured it.
fn fold_image(
    image: &Image<'_>,
    renderer: &dyn InlineSubstitutionRenderer,
    parser: &Parser,
    out: &mut String,
) {
    // A macro-built image always carries its attribute list; a node hand-built
    // without one folds through an empty list, sliced (empty) from the node's
    // own `'src` location so its lifetime matches.
    let empty;
    let attrlist = match &image.attrs {
        Some(attrlist) => attrlist,

        None => {
            empty = Attrlist::parse(image.location.slice(0..0), parser, AttrlistContext::Inline)
                .item
                .item;

            &empty
        }
    };

    let alt = image
        .alt
        .as_ref()
        .map(|a| a.to_string())
        .unwrap_or_default();

    if image.is_icon {
        let params = IconRenderParams {
            target: image.target.as_ref(),
            alt,
            size: attrlist
                .named_or_positional_attribute("size", 1)
                .map(|a| a.value()),
            attrlist,
            parser,
        };

        renderer.render_icon(&params, out);
    } else {
        let params = ImageRenderParams {
            target: image.target.as_ref(),
            alt,
            width: image.width.as_deref(),
            height: image.height.as_deref(),
            attrlist,
            parser,
        };

        renderer.render_image(&params, out);
    }
}

/// Folds a [`Ui`](InlineNode::Ui) node through the same
/// `render_keyboard`/`render_button`/`render_menu` the string pipeline's macros
/// step calls, reconstructing the render parameters from the keys / label /
/// menu path the macro step captured, so the output is byte-for-byte identical.
/// `parser` is threaded through because rendering a menu reads the document's
/// `icons` attribute to choose the caret between menu levels.
fn fold_ui(
    ui: &Ui<'_>,
    renderer: &dyn InlineSubstitutionRenderer,
    parser: &Parser,
    out: &mut String,
) {
    match &ui.kind {
        UiKind::Keyboard(keys) => {
            let keys: Vec<String> = keys.iter().map(|k| k.to_string()).collect();
            renderer.render_keyboard(&keys, out);
        }

        UiKind::Button(text) => {
            renderer.render_button(text.as_ref(), out);
        }

        UiKind::Menu {
            menu,
            submenus,
            item,
        } => {
            let submenus: Vec<String> = submenus.iter().map(|s| s.to_string()).collect();

            let params = MenuRenderParams {
                menu: menu.as_ref(),
                submenus: &submenus,
                menuitem: item.as_deref(),
                parser,
            };

            renderer.render_menu(&params, out);
        }
    }
}

/// Folds a link [`Ref`](InlineNode::Ref) node through the same `render_link`
/// the string pipeline's macros step calls, reconstructing the
/// [`LinkRenderParams`] from the node: the display text is the fold of the
/// children, the extra roles (`bare`) ride on the node's `roles`, and the
/// target/window come straight off the node. The attribute list is empty
/// because this increment recognizes only the attribute-list-free link forms
/// (see `link_macro_level`).
fn fold_link(
    reference: &Ref<'_>,
    renderer: &dyn InlineSubstitutionRenderer,
    parser: &Parser,
    out: &mut String,
) {
    let mut link_text = String::new();
    fold_into_html(&reference.children, renderer, parser, &mut link_text);

    // A link built by this increment carries no attribute list; fold through an
    // empty one, sliced (empty) from the node's own location so its lifetime
    // matches.
    let attrlist = Attrlist::parse(
        reference.location.slice(0..0),
        parser,
        AttrlistContext::Inline,
    )
    .item
    .item;

    let extra_roles: Vec<&str> = reference.roles.iter().map(|r| r.as_ref()).collect();

    let params = LinkRenderParams {
        target: reference.target.to_string(),
        link_text,
        extra_roles,
        window: reference.window.as_deref(),
        attrlist: &attrlist,
        parser,
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
/// (unresolved – no catalog-resolution pass runs, so `resolved` is always
/// `None` here) and a target that carries its own destination without a
/// catalog (`derived`, populated at build time – see the `Ref::derived` field
/// docs): an inter-document target, and the empty target naming the current
/// document as a whole. It does not yet parse an attribute-list-bearing
/// display text, so `xrefstyle` is always `None`; the cutover (design §5.2
/// Phase 4, step 6) wires catalog resolution to the tree.
fn fold_xref(
    reference: &Ref<'_>,
    renderer: &dyn InlineSubstitutionRenderer,
    parser: &Parser,
    out: &mut String,
) {
    let mut provided = String::new();
    fold_into_html(&reference.children, renderer, parser, &mut provided);

    let provided_text = if provided.is_empty() {
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
        xrefstyle: None,
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
/// output – the built-in backend ignores the reference text entirely – and is
/// the same verbatim boundary the node documents; a custom backend that needs
/// the full reference text unconditionally will get it once a re-flow consumer
/// pins richer `reftext` population.
fn fold_anchor(
    anchor: &Anchor<'_>,
    renderer: &dyn InlineSubstitutionRenderer,
    parser: &Parser,
    out: &mut String,
) {
    let reftext = anchor.reftext.as_ref().map(|children| {
        let mut s = String::new();
        fold_into_html(children, renderer, parser, &mut s);
        s
    });

    renderer.render_anchor(&anchor.id, reftext, out);
}

/// Folds an [`IndexTerm`](InlineNode::IndexTerm) through the same
/// `render_index_term` the string step's macros pass calls.
///
/// A **concealed** term (`visible == false`) has an empty `terms` and renders
/// nothing – the HTML backend generates no index. A **visible** (flow) term
/// carries its shown text as `terms[0]` in the *already-substituted* form the
/// recognizer computed (matching the seam's "already-substituted visible term"
/// contract), so `render_index_term` emits it verbatim and the fold reproduces
/// the string pipeline's bytes exactly.
///
/// The node's `terms` mirrors what the
/// [`inline_tree`](crate::content::inline_tree) recorder stores – the single
/// shown term for a visible node, empty for a concealed one; the richer
/// primary/secondary/tertiary structure the field can hold is left to a re-flow
/// consumer to pin (the field is provisional, per the node's Phase-0 note),
/// exactly as an anchor's `reftext` is.
fn fold_index_term(
    index_term: &IndexTerm<'_>,
    renderer: &dyn InlineSubstitutionRenderer,
    out: &mut String,
) {
    let visible_term = if index_term.visible {
        Some(index_term.terms.first().map_or("", CowStr::as_ref))
    } else {
        None
    };

    renderer.render_index_term(&IndexTermRenderParams { visible_term }, out);
}

/// Folds a [`Footnote`](InlineNode::Footnote) through the same
/// `render_footnote` the string step calls, reconstructing
/// [`FootnoteRenderParams`] entirely from the node's `is_reference`, `number`,
/// and `id` fields – no build-time state is needed (design §3.3.1).
///
/// Only the in-flow **marker** is folded here (`[1]`, or `[id]` for an
/// unresolved reference): `render_footnote` never emits the footnote's own
/// text into the flow – that text belongs in the document's separate
/// footnote list, a concern outside a single block's fold – so
/// `footnote.children` is not folded into `out` at all, the same relationship
/// [`fold_anchor`]'s `reftext` has to its own marker.
///
/// A defining occurrence (`is_reference == false`) always carries its number
/// (see `build_footnote_node`) and folds its own `id`, when it has one, into
/// the marker's `id` attribute. A reference either reuses an existing
/// footnote's number (`number: Some`, `id` never folded – matching the string
/// replacer, which renders a reference's `id` attribute only for the
/// *defining* occurrence) or, unresolved (`number: None`), falls back to
/// displaying its own `id` as the render params' `text`.
fn fold_footnote(
    footnote: &Footnote<'_>,
    renderer: &dyn InlineSubstitutionRenderer,
    out: &mut String,
) {
    let (index, id, text): (Option<&str>, Option<&str>, &str) = if footnote.is_reference {
        match footnote.number.as_deref() {
            Some(number) => (Some(number), None, ""),
            None => (None, None, footnote.id.as_deref().unwrap_or("")),
        }
    } else {
        (footnote.number.as_deref(), footnote.id.as_deref(), "")
    };

    renderer.render_footnote(
        &FootnoteRenderParams {
            index,
            id,
            is_reference: footnote.is_reference,
            text,
        },
        out,
    );
}

/// Folds a [`Callout`](InlineNode::Callout) through the same `render_callout`
/// the string step's `Callouts` group calls, reconstructing
/// [`CalloutRenderParams`] from the node's `number` and `guard` – no
/// build-time state is needed, mirroring [`fold_footnote`]. This is the
/// decoupling design §4.6 previews for Phase 5: the node is the canonical,
/// structured record; the `*RenderParams` struct is rebuilt from it fresh at
/// fold time, not carried alongside it.
fn fold_callout(
    callout: &Callout<'_>,
    renderer: &dyn InlineSubstitutionRenderer,
    parser: &Parser,
    out: &mut String,
) {
    let guard = match &callout.guard {
        CalloutGuard::LineComment(prefix) => ParserCalloutGuard::LineComment(prefix.as_ref()),
        CalloutGuard::Xml => ParserCalloutGuard::Xml,
    };

    renderer.render_callout(
        &CalloutRenderParams {
            number: callout.number.as_ref(),
            guard,
            parser,
        },
        out,
    );
}

/// Folds a [`Stem`](InlineNode::Stem) through the same
/// `render_quoted_substitution` the string pipeline's passthrough-restore step
/// calls for a STEM entry (design §3.3.1's fold-time seam). The node's `value`
/// already carries the resolved substitution group's output (special
/// characters only, by default), so the fold passes it straight through as
/// the body – no further processing is needed, mirroring how a STEM
/// passthrough is restored with no attribute list or id (`INLINE_STEM_MACRO`
/// captures neither).
fn fold_stem(stem: &Stem<'_>, renderer: &dyn InlineSubstitutionRenderer, out: &mut String) {
    let type_ = match stem.notation {
        StemNotation::AsciiMath => QuoteType::AsciiMath,
        StemNotation::LatexMath => QuoteType::LatexMath,
    };

    renderer.render_quoted_substitution(
        type_,
        QuoteScope::Unconstrained,
        None,
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

    use super::super::test_support::{build_src, fold_html};
    use crate::{
        Span,
        inlines::{CharRef, Image, InlineNode},
        parser::HtmlSubstitutionRenderer,
        strings::CowStr,
    };

    #[test]
    fn fold_emits_raw_verbatim() {
        // A `Raw` leaf is emitted without HTML-escaping, unlike `Text`; its `<`,
        // `>`, and `&` pass straight through.
        let location = Span::new("<b>raw &amp;</b>");

        let raw = InlineNode::Raw {
            value: CowStr::from(location.data()),
            location,
        };

        assert_eq!(
            fold_html(&[raw], &HtmlSubstitutionRenderer {}),
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

        assert_eq!(fold_html(&[node], &HtmlSubstitutionRenderer {}), "&#122;");
    }

    #[test]
    fn fold_folds_a_hand_built_image_without_an_attribute_list() {
        // The node types are public, so a consumer may hand-build an
        // [`Image`](InlineNode::Image) directly rather than through the macro
        // step – with no [`Attrlist`]. The fold handles that by folding through
        // an empty attribute list sliced from the node's own location, so a
        // hand-built image renders like the same macro would. (The macro step
        // always attaches an attribute list, so only a hand-built node reaches
        // this branch, exactly as [`fold_encodes_an_unknown_replacement_value_
        // per_character`] exercises a hand-built replacement.)
        let location = Span::new("image:sunset.jpg[Sunset]");

        let hand_built = InlineNode::Image(Image {
            is_icon: false,
            target: CowStr::from("sunset.jpg"),
            alt: Some(CowStr::from("Sunset")),
            width: None,
            height: None,
            attrs: None,
            location,
        });

        // The macro-built equivalent (which carries an attribute list) is the
        // oracle: the two must fold identically.
        let renderer = HtmlSubstitutionRenderer {};
        let macro_built = fold_html(&build_src(location), &renderer);

        assert_eq!(fold_html(&[hand_built], &renderer), macro_built);
    }
}
