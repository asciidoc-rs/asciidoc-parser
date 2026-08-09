//! The macros substitution step, split by macro family.

mod anchors;
pub(super) mod image;
mod indexterm;
mod links;
mod ui;
mod xref;

use anchors::anchor_macros_level;
use image::image_macros_level;
use indexterm::indexterm_macros_level;
use links::{inline_link_level, link_macro_level};
use ui::{kbd_btn_macros_level, menu_macros_level};
use xref::xref_macros_level;

use super::quotes::{Piece, emit_range};
use crate::{Parser, Span, inlines::InlineNode};

/// The macros substitution, as a node transducer.
///
/// This increment recognizes **image and icon macros** (`image:target[…]`,
/// `icon:target[…]`), the **UI macros** (`kbd:[…]`, `btn:[…]`, `menu:…[…]`),
/// the **`link:`/`mailto:` macro** (`link:target[…]`, `mailto:addr[…]`), and
/// **auto-links and formal-URL links** (`https://example.org`,
/// `https://example.org[text]`), replacing each with an
/// [`Image`](InlineNode::Image), [`Ui`](InlineNode::Ui), or
/// [`Ref`](InlineNode::Ref) node. An image node carries its own owned
/// [`Attrlist`](crate::attributes::Attrlist) – the step that makes a macro node
/// *self-describing*, so the fold reconstructs the render parameters and calls
/// the same `render_image`/`render_icon` the string step calls; a UI node
/// carries the keys / label / menu path the string replacer computed, so its
/// fold calls the same `render_keyboard`/`render_button`/`render_menu`; a link
/// node (whether a `link:`/`mailto:` macro or an auto-link) carries the
/// computed target, display text (as [`Text`](InlineNode::Text) children), and
/// roles/window, so its fold calls the same `render_link`. The remaining macro
/// families (cross-references, footnotes, index terms, anchors, STEM) are later
/// increments (see [`link_macro_level`] and [`inline_link_level`] for the link
/// forms this increment defers).
///
/// Each family is applied at each level in the **same order the string step
/// applies them** – keyboard/button, then menu, then image/icon, then
/// auto-links (`INLINE_LINK`), then the `link:`/`mailto:` macro
/// (`INLINE_LINK_MACRO`) – so a level's overlapping constructs resolve
/// identically. Like the other steps it descends
/// into the [`Styled`](crate::inlines::Styled)/[`Ref`](InlineNode::Ref)
/// children earlier steps created – a macro can appear inside a rendered span
/// (`*image:x[]*`), just as the string pipeline matches one inside a rendered
/// `<strong>` tag – then matches at each level.
///
/// # The UI macros are gated on `experimental`
///
/// The string step recognizes `kbd:`/`btn:`/`menu:` only when the
/// `experimental` document attribute is set (an optimization that skips the
/// work in the common case); this transducer mirrors that gate exactly, so with
/// `experimental` off a `kbd:[…]` stays literal here just as it does in the
/// string output.
///
/// # Scope: verbatim macros only
///
/// A recognized macro is built into an `'src`-borrowing node only when its
/// whole match is **verbatim source** – no special character (`< > &`, an
/// atomic [`CharRef`](InlineNode::CharRef)) and no rendered
/// [`Styled`](crate::inlines::Styled) span falls inside it. The string pipeline
/// matches macros over *escaped, already-rendered* text, so a macro containing
/// (say) a `&` sees `&amp;` in its target/attrlist, and a self-describing node
/// cannot carry that escaped text as an `'src` slice. Such a macro is therefore
/// **left unrecognized** here for a later increment (the attribute-references
/// step and the cutover), mirroring how the quotes step documents its own
/// cross-span boundary (crossed delimiters). For a menu this notably defers the
/// `&gt;`-submenu form (`menu:View[Zoom > Reset]`), whose `>` is always an
/// escaped [`CharRef`](InlineNode::CharRef) by the time macros run; the
/// comma-delimited and bare/single-item forms are verbatim and covered. The
/// differential corpus pins the verbatim cases this increment claims.
pub(super) fn apply_macros<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    // Recurse into spans/refs first, matching the string pipeline's
    // whole-string pass.
    let nodes: Vec<InlineNode<'src>> = nodes
        .into_iter()
        .map(|node| match node {
            InlineNode::Styled(mut styled) => {
                styled.children = apply_macros(styled.children, root, parser);
                InlineNode::Styled(styled)
            }

            InlineNode::Ref(mut reference) => {
                reference.children = apply_macros(reference.children, root, parser);
                InlineNode::Ref(reference)
            }

            other => other,
        })
        .collect();

    // The UI macros run before image/icon and only under `experimental`,
    // mirroring the string step's order and gate.
    let nodes = if parser.is_attribute_set("experimental") {
        let nodes = kbd_btn_macros_level(nodes, root);
        menu_macros_level(nodes, root)
    } else {
        nodes
    };

    let nodes = image_macros_level(nodes, root, parser);

    // Index terms (`((term))`, `(((primary, secondary)))`, `indexterm:[…]`,
    // `indexterm2:[…]`) run after image/icon and before the link families,
    // mirroring the string step's order.
    let nodes = indexterm_macros_level(nodes, root);

    // Auto-links and formal-URL links (`INLINE_LINK`) run after the index-term
    // pass and before the `link:`/`mailto:` macro, mirroring the string step's
    // order (`INLINE_LINK` precedes `INLINE_LINK_MACRO`).
    let nodes = inline_link_level(nodes, root, parser);

    // The `link:`/`mailto:` macro runs after the auto-link pass, mirroring the
    // string step's order.
    let nodes = link_macro_level(nodes, root, parser);

    // Inline anchors (`[[id]]`, `anchor:id[…]`) run after the link families and
    // before cross-references, mirroring the string step's order. The e-mail
    // pass the string step runs between the link macro and the anchor is a later
    // increment, so with it absent this preserves the relative order for the
    // constructs the builder recognizes so far. The bibliography-anchor pass the
    // string step runs *first* (a `^`-anchored `[[[id]]]`) fires only inside a
    // bibliography list item – a context the additive builder is not wired into –
    // so it is a cutover concern, not this pass's.
    let nodes = anchor_macros_level(nodes, root);

    // Cross-references (`xref:id[…]`) run after the anchor pass, mirroring the
    // string step's order.
    xref_macros_level(nodes, root, parser)

    // Footnotes are **not** handled here. Every other family's recognition is
    // order-independent (no cross-node side effect), so it is safe for them to
    // run under this function's "resolve a whole subtree's children, then this
    // level" recursion (see the closure at the top of this function). A
    // footnote's assigned number is *not* order-independent, so it needs its
    // own recursive walk that visits nodes in true left-to-right source order
    // regardless of nesting depth – see [`apply_footnotes`], run once, as its
    // own step in [`build`], after `apply_macros` has fully resolved every
    // other family at every level.
}

/// One recognized macro match at a level, in absolute match-string byte
/// offsets. Shared across the macro families (image/icon, UI, and later
/// increments), which differ only in how they *build* a match's node.
pub(super) struct MacroMatch<'src> {
    /// The whole match, `[start, end)`.
    pub(super) full: std::ops::Range<usize>,

    /// What to emit in place of `full`.
    pub(super) kind: MacroMatchKind<'src>,
}

pub(super) enum MacroMatchKind<'src> {
    /// An escaped macro (`\image:…`, `\kbd:[…]`, `\https://…`): drop the single
    /// backslash at `backslash` and keep the rest of the match as literal
    /// nodes, replacing nothing – mirroring the string replacer's
    /// `caps[0][1..]`. The backslash is at the match start for the
    /// prefix-less macros, and at the scheme for an escaped auto-link whose
    /// match carries a boundary prefix.
    Unescape { backslash: usize },

    /// A recognized macro, built into its node ([`Image`](InlineNode::Image),
    /// [`Ui`](InlineNode::Ui), [`Ref`](InlineNode::Ref), …). Boxed to keep this
    /// enum small – a macro node is far larger than the
    /// [`Unescape`](Self::Unescape) variant.
    ///
    /// The node replaces only the `consumed` sub-range of the match; the match
    /// text before it (`[full.start, consumed.start)`, a boundary prefix a
    /// non-angle auto-link keeps) and after it (`[consumed.end, full.end)`, the
    /// trailing punctuation a bare URL strips) is kept as literal. For the
    /// macro families that consume their whole match (image, UI,
    /// `link:`/`mailto:`), `consumed` equals the full match, so no prefix
    /// or suffix is kept.
    Node {
        consumed: std::ops::Range<usize>,
        node: Box<InlineNode<'src>>,
    },
}

/// Rebuilds a level's node list from its macro matches: each gap keeps its
/// original nodes; each match becomes either its literal text (an escape, with
/// the leading backslash dropped) or the built macro node. Shared across the
/// macro families, which differ only in how they produce the [`MacroMatch`]
/// list.
pub(super) fn rebuild_macro_level<'src>(
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    s: &str,
    matches: Vec<MacroMatch<'src>>,
) -> Vec<InlineNode<'src>> {
    let mut out = Vec::new();
    let mut cursor = 0usize;

    for m in matches {
        let MacroMatch { full, kind } = m;

        match kind {
            MacroMatchKind::Unescape { backslash } => {
                // Keep the whole match with the single backslash dropped.
                emit_range(nodes, pieces, cursor..backslash, &mut out);
                emit_range(nodes, pieces, (backslash + 1)..full.end, &mut out);
            }

            MacroMatchKind::Node { consumed, node } => {
                // The gap runs to the node, absorbing any kept boundary prefix;
                // the node replaces `consumed`; any stripped trailing
                // punctuation after it is kept as literal.
                emit_range(nodes, pieces, cursor..consumed.start, &mut out);
                out.push(*node);
                emit_range(nodes, pieces, consumed.end..full.end, &mut out);
            }
        }

        cursor = full.end;
    }

    if cursor < s.len() {
        emit_range(nodes, pieces, cursor..s.len(), &mut out);
    }

    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use super::apply_macros;
    use crate::{
        Parser, Span,
        inlines::{InlineNode, Ref, RefVariant},
        strings::CowStr,
    };

    #[test]
    fn apply_macros_recognizes_a_macro_inside_reference_children() {
        // The macros step descends into a [`Ref`](InlineNode::Ref)'s display
        // children before matching at its own level (mirroring the string
        // pipeline's whole-string pass). The builder never leaves an
        // unrecognized macro inside freshly-built reference children, so this
        // descent is exercised directly: a hand-built `Ref` whose child text is
        // an `image:` macro has that macro recognized inside it.
        let root = Span::new("image:x.png[X]");

        let reference = InlineNode::Ref(Ref {
            variant: RefVariant::Link,
            target: CowStr::from("https://example.org"),
            children: vec![InlineNode::Text {
                value: CowStr::from(root.data()),
                location: root,
            }],
            roles: vec![],
            window: None,
            resolved: None,
            derived: None,
            location: root,
        });

        let out = apply_macros(vec![reference], root, &Parser::default());

        // The reference survives, and its single child is now the recognized
        // image node.
        assert_eq!(out.len(), 1);

        match &out[0] {
            InlineNode::Ref(reference) => {
                assert_eq!(reference.children.len(), 1);

                assert!(
                    matches!(reference.children[0], InlineNode::Image(_)),
                    "expected the child macro to be recognized, got {:?}",
                    reference.children[0]
                );
            }

            other => panic!("expected the Ref to survive, got {other:?}"),
        }
    }
}
