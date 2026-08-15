//! The macros substitution step, split by macro family.

pub(super) mod anchors;
pub(super) mod image;
mod indexterm;
pub(super) mod links;
mod ui;
mod xref;

use anchors::{anchor_macros_level, biblio_anchor_level};
use image::image_macros_level;
use indexterm::indexterm_macros_level;
use links::{email_level, inline_link_level, link_macro_level};
use ui::{kbd_btn_macros_level, menu_macros_level};
use xref::xref_macros_level;

use super::quotes::{Piece, emit_range};
use crate::{Parser, Span, inlines::InlineNode};

/// The macros substitution, as a node transducer.
///
/// This increment recognizes **image and icon macros** (`image:target[…]`,
/// `icon:target[…]`), the **UI macros** (`kbd:[…]`, `btn:[…]`, `menu:…[…]`),
/// the **`link:`/`mailto:` macro** (`link:target[…]`, `mailto:addr[…]`),
/// **auto-links and formal-URL links** (`https://example.org`,
/// `https://example.org[text]`), and **bare e-mail addresses**
/// (`doc@example.org`), replacing each with an
/// [`Image`](InlineNode::Image), [`Ui`](InlineNode::Ui), or
/// [`Ref`](InlineNode::Ref) node. An image node carries its own owned
/// [`Attrlist`](crate::attributes::Attrlist) – the step that makes a macro node
/// *self-describing*, so the fold reconstructs the render parameters and calls
/// the same `render_image`/`render_icon` the string step calls; a UI node
/// carries the keys / label / menu path the string replacer computed, so its
/// fold calls the same `render_keyboard`/`render_button`/`render_menu`; a link
/// node (whether a `link:`/`mailto:` macro, an auto-link, or a bare e-mail
/// address) carries the
/// computed target, display text (as [`Text`](InlineNode::Text) children), and
/// roles/window, so its fold calls the same `render_link`. The remaining macro
/// families (cross-references, footnotes, index terms, anchors, STEM) are later
/// increments (see [`link_macro_level`], [`inline_link_level`], and
/// [`email_level`] for the link forms this increment defers).
///
/// It also recognizes the **bibliography anchor** (`[[[label]]]`) that prefixes
/// a bibliography list item, as an [`Anchor`](InlineNode::Anchor) node whose
/// `is_bibliography` is set – the one family that is not a level pass, since
/// its pattern is `^`-anchored to the whole content (see
/// [`biblio_anchor_level`]).
///
/// Each family is applied at each level in the **same order the string step
/// applies them** – keyboard/button, then menu, then image/icon, then
/// auto-links (`INLINE_LINK`), then the `link:`/`mailto:` macro
/// (`INLINE_LINK_MACRO`), then a bare e-mail address (`INLINE_EMAIL`) – so a
/// level's overlapping constructs resolve
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
/// cross-span boundary (crossed delimiters). A family relaxes that gate only
/// where the escaped piece is a delimiter *it consumes and never slices* – the
/// angle-bracketed URL's own `&lt;`/`&gt;` (see
/// `links::build_inline_link_node`) and a menu's `&gt;` submenu caret (see
/// `ui::menu_match_is_sliceable`) – not where the escaped text would have to
/// ride on the node. The differential corpus pins the cases each increment
/// claims.
pub(super) fn apply_macros<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    // The bibliography anchor (`[[[label]]]`) runs before every other family,
    // exactly as the string step runs its own `INLINE_BIBLIO_ANCHOR` pass
    // first. It runs *only here*, at the content's own top level – its pattern
    // is `^`-anchored, so it can only ever match the very start of the whole
    // content, never the start of a span's children (see
    // [`biblio_anchor_level`]) – which is why it sits outside
    // [`apply_macro_families`]'s own recursion.
    let nodes = biblio_anchor_level(nodes, root, parser);

    apply_macro_families(nodes, root, parser)
}

/// Applies each macro family at this level – and, first, at every level nested
/// inside it – in the string step's own family order. See
/// [`apply_macros`], which wraps this with the once-per-content
/// bibliography-anchor pass.
fn apply_macro_families<'src>(
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
                styled.children = apply_macro_families(styled.children, root, parser);
                InlineNode::Styled(styled)
            }

            InlineNode::Ref(mut reference) => {
                reference.children = apply_macro_families(reference.children, root, parser);
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

    // A bare e-mail address (`doc@example.org`) runs after both URL-link
    // families and before the anchor pass, exactly where the string step runs
    // `InlineEmailReplacer` – so an address that is really the tail of a URL, or
    // a `mailto:` macro's own target, is already inside an opaque node (there,
    // already-rendered `<a …>` markup) and is not re-recognized.
    let nodes = email_level(nodes, root);

    // Inline anchors (`[[id]]`, `anchor:id[…]`) run after the link families and
    // before cross-references, mirroring the string step's order. (The
    // bibliography-anchor pass the string step runs *first* – a `^`-anchored
    // `[[[id]]]`, recognized only inside a bibliography list item – runs in
    // [`apply_macros`], outside this recursion.)
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

/// The eventual cutover's single entry point (design §5.2, Phase 4 step 6) for
/// **every** recognition side effect the macro families above defer –
/// composing [`anchors::apply_biblio_side_effects`],
/// [`image::apply_image_side_effects`], [`links::apply_link_side_effects`], and
/// [`anchors::apply_ref_side_effects`], each staged and tested as its own
/// standalone building block, into the one call the cutover makes exactly once
/// per parse.
///
/// # Ordering
///
/// The four are called in the same relative order the string pipeline's own
/// macro passes run in (the bibliography anchor, then image/icon, …, links, …,
/// anchors, …– see [`apply_macros`]'s own doc comment): bibliography anchor,
/// then image, then link, then anchor/ref.
/// This is not cosmetic – it is what keeps this function's output identical to
/// the golden pipeline's whenever more than one family's side effect touches
/// the *same* shared list. Concretely, [`Parser::record_substitution_warning`]
/// appends to one shared warnings list, and both
/// [`image::apply_image_side_effects`]'s dangerous-link-scheme warning and
/// [`anchors::apply_ref_side_effects`]'s duplicate-id warning write to it – a
/// content whose image triggers the first and whose anchor triggers the
/// second must see the image warning recorded first, exactly as it would from
/// the string pipeline's own image-then-anchor pass order. The same holds one
/// step earlier for [`anchors::apply_biblio_side_effects`]'s own duplicate-id
/// warning, which the string pipeline's first pass records ahead of both. (The
/// asset/ref catalogs the three write to are otherwise disjoint from one
/// another – images, links, and refs are three separate lists – so this
/// ordering does not, by itself, need to hold *within* a single catalog; see
/// [`links::apply_link_side_effects`]'s own doc comment for the finer-grained
/// ordering *within* the link family that the golden pipeline also requires.)
///
/// Index terms, cross-references, and footnotes are not part of this
/// function: index terms and cross-references perform no recognition side
/// effect at all (an index term has no catalog in the HTML backend; a
/// cross-reference is resolved, not registered), and a footnote's one
/// required side effect – its assigned number – is not deferred in the first
/// place (see [`apply_footnotes`](super::footnotes::apply_footnotes)'s own doc
/// comment); it already runs during [`build`](super::build), not here.
///
/// As with each of the three functions it composes, **nothing here is wired
/// into a real parse path yet** – it is exercised only by this module's own
/// tests, against their own `Parser`. `source` and `leading_anchor_registered`
/// are threaded straight through to
/// [`anchors::apply_ref_side_effects`] – see its own doc comment for both.
pub(crate) fn apply_macro_side_effects(
    nodes: &[InlineNode<'_>],
    parser: &Parser,
    source: Span<'_>,
    leading_anchor_registered: bool,
) {
    anchors::apply_biblio_side_effects(nodes, parser, source);
    image::apply_image_side_effects(nodes, parser, source);
    links::apply_link_side_effects(nodes, parser);
    anchors::apply_ref_side_effects(nodes, parser, source, leading_anchor_registered);
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

    use super::{super::test_support::golden_macros_with, apply_macro_side_effects, apply_macros};
    use crate::{
        Parser, Span,
        content::inline_builder::build,
        inlines::{InlineNode, Ref, RefVariant},
        strings::CowStr,
        warnings::WarningType,
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
            xrefstyle: None,
            attrs: None,
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

    #[test]
    fn registers_every_family_from_a_single_call() {
        let source =
            "image:a.png[] link:b.html[B] https://c.example [[anchor-id]] xref:anchor-id[]";
        let parser = Parser::default().with_catalog_assets(true);
        let nodes = build(Span::new(source), &parser, None);

        apply_macro_side_effects(&nodes, &parser, Span::new(source), false);

        let catalog = parser.catalog();
        assert_eq!(
            catalog
                .images()
                .iter()
                .map(|i| i.target.clone())
                .collect::<Vec<_>>(),
            ["a.png"]
        );
        assert_eq!(catalog.links(), ["https://c.example", "b.html"]);
        assert!(catalog.contains_id("anchor-id"));
    }

    #[test]
    fn matches_the_golden_pipelines_registrations_and_warning_order_for_mixed_families() {
        // A content that exercises every family this function composes in one
        // go: an image whose `link=` targets a dangerous scheme (a warning
        // from the image family) *before* a duplicate anchor id (a warning
        // from the anchor family) – the golden pipeline's own image-then-
        // anchor pass order (`apply_macros`'s own doc comment) must land the
        // two warnings in that order, not the reverse. Each side uses its own
        // *independent* parser (design §5.3's two-independent-parsers
        // discipline, established by the image increment's own differential
        // corpus).
        let source = "image:x.png[alt,link=javascript:alert(1)] then [[dup]] and [[dup]]";

        let builder_parser = Parser::default().with_catalog_assets(true);
        let nodes = build(Span::new(source), &builder_parser, None);
        apply_macro_side_effects(&nodes, &builder_parser, Span::new(source), false);

        let golden_parser = Parser::default().with_catalog_assets(true);
        golden_macros_with(source, &golden_parser);

        assert_eq!(
            builder_parser
                .catalog()
                .images()
                .iter()
                .map(|i| i.target.clone())
                .collect::<Vec<_>>(),
            golden_parser
                .catalog()
                .images()
                .iter()
                .map(|i| i.target.clone())
                .collect::<Vec<_>>(),
        );

        let builder_warnings: Vec<_> = builder_parser
            .drain_substitution_warnings_since(0)
            .into_iter()
            .map(|w| w.warning)
            .collect();
        let golden_warnings: Vec<_> = golden_parser
            .drain_substitution_warnings_since(0)
            .into_iter()
            .map(|w| w.warning)
            .collect();

        assert_eq!(builder_warnings, golden_warnings);
        assert_eq!(
            builder_warnings,
            [
                WarningType::UnsafeLinkSchemeRejected("javascript:alert(1)".to_string()),
                WarningType::DuplicateId("dup".to_string()),
            ]
        );
    }

    #[test]
    fn a_bibliography_entry_registers_before_every_other_family() {
        // The string pipeline runs its bibliography-anchor pass *first*, ahead
        // of every other macro family, so a duplicate bibliography id must be
        // warned about before an image's dangerous-link-scheme warning in the
        // one shared warnings list – the same ordering this function's own doc
        // comment records for image-before-anchor, one step earlier. Each side
        // uses its own independent parser.
        let source = "[[[dup]]] image:x.png[alt,link=javascript:alert(1)] entry";

        let builder_parser = Parser::default().with_catalog_assets(true);
        builder_parser.in_bibliography_list_item.set(true);
        builder_parser
            .register_ref("dup", None, crate::document::RefType::Bibliography)
            .unwrap();

        let nodes = build(Span::new(source), &builder_parser, None);
        apply_macro_side_effects(&nodes, &builder_parser, Span::new(source), false);

        let golden_parser = Parser::default().with_catalog_assets(true);
        golden_parser.in_bibliography_list_item.set(true);
        golden_parser
            .register_ref("dup", None, crate::document::RefType::Bibliography)
            .unwrap();

        golden_macros_with(source, &golden_parser);

        let warnings = |parser: &Parser| {
            parser
                .drain_substitution_warnings_since(0)
                .into_iter()
                .map(|w| w.warning)
                .collect::<Vec<_>>()
        };

        let builder_warnings = warnings(&builder_parser);

        assert_eq!(builder_warnings, warnings(&golden_parser));
        assert_eq!(
            builder_warnings,
            [
                WarningType::DuplicateId("dup".to_string()),
                WarningType::UnsafeLinkSchemeRejected("javascript:alert(1)".to_string()),
            ]
        );
    }
}
