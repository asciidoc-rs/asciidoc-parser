//! The macros substitution step, split by macro family.

pub(super) mod anchors;
pub(super) mod image;
mod indexterm;
pub(super) mod links;
mod ui;
mod xref;

use anchors::{anchor_macros_level, biblio_anchor_level};
use image::{image_macros_level, range_is_verbatim_or_synthesized};
use indexterm::indexterm_macros_level;
use links::{email_level, inline_link_level, link_macro_level};
use ui::{kbd_btn_macros_level, menu_macros_level};
use xref::xref_macros_level;

use super::quotes::{Piece, emit_range, source_slice, text_slice};
use crate::{Parser, Span, inlines::InlineNode, strings::CowStr};

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
/// [`Attrlist`](crate::attributes::Attrlist) — the step that makes a macro node
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
/// `is_bibliography` is set — the one family that is not a level pass, since
/// its pattern is `^`-anchored to the whole content (see
/// [`biblio_anchor_level`]).
///
/// Each family is applied at each level in the **same order the string step
/// applies them** — keyboard/button, then menu, then image/icon, then
/// auto-links (`INLINE_LINK`), then the `link:`/`mailto:` macro
/// (`INLINE_LINK_MACRO`), then a bare e-mail address (`INLINE_EMAIL`) — so a
/// level's overlapping constructs resolve
/// identically. Like the other steps it descends
/// into the [`Styled`](crate::inlines::Styled)/[`Ref`](InlineNode::Ref)
/// children earlier steps created — a macro can appear inside a rendered span
/// (`*image:x[]*`), just as the string pipeline matches one inside a rendered
/// `<strong>` tag — then matches at each level.
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
/// whole match is **verbatim source** — no special character (`< > &`, an
/// atomic [`CharRef`](InlineNode::CharRef)) and no rendered
/// [`Styled`](crate::inlines::Styled) span falls inside it. The string pipeline
/// matches macros over *escaped, already-rendered* text, so a macro containing
/// (say) a `&` sees `&amp;` in its target/attrlist, and a self-describing node
/// cannot carry that escaped text as an `'src` slice. Such a macro is therefore
/// **left unrecognized** here for a later increment (the attribute-references
/// step and the cutover), mirroring how the quotes step documents its own
/// cross-span boundary (crossed delimiters). A family relaxes that gate where
/// the escaped piece is a delimiter *it consumes and never slices* — the
/// angle-bracketed URL's own `&lt;`/`&gt;` (see
/// `links::build_inline_link_node`) — or, for **every** family that can carry a
/// target, a display text, or any other computed value (the
/// **cross-reference**, the **`link:`/`mailto:` macro**, the **auto-link /
/// formal-URL**, the **bare e-mail**, the **image/icon**, and the **UI**
/// families), wherever the escaped text need not
/// ride on the node as an `'src` slice at all: none of those families' targets
/// is `Span`-typed, so each reads its values out of the match string (whose
/// entity bytes *are* the string pipeline's own) and rebuilds its display text
/// as structured children through [`macro_text_children`], keeping each special
/// as its own `CharRef` (see `xref::find_xref_matches`,
/// `links::find_link_macro_matches`, `links::build_inline_link_node`,
/// `links::email_level`, `image::build_image_node`, `ui::find_kbd_btn_matches`,
/// `ui::find_menu_matches`, and
/// [`range_has_no_opaque_piece`](image::range_has_no_opaque_piece)). A
/// **restored entity** (`&copy;`, `&#8217;` — an author-written entity the
/// replacements step un-escaped) rides on that same gate, since
/// [`build_match_string`](super::quotes::build_match_string) gives it its own
/// bytes too; it needs no per-family work, so it is admitted for the
/// index-term, anchor and STEM families as well, and a footnote's text — which
/// is structured children rather than a sliced value — carries either leaf as
/// its own child with no gate at all. What keeps
/// the stricter gate is never a *family* now, only the one **capture** that
/// must ride on the node as a real
/// [`Attrlist`](crate::attributes::Attrlist)`<'src>`, parsed from the source's
/// own bytes: a link's attribute-list-bearing display text and an image's
/// non-empty bracket (a cross-reference's own attribute list is parsed from a
/// normalized *copy*, so it takes both lifts). The auto-link family
/// additionally keeps a narrow deferral of its own for a bare URL whose
/// trailing-punctuation strip would cut inside an escaped special, and the
/// e-mail family one for an address *abutting* an opaque piece. The
/// differential corpus pins the cases each increment claims.
///
/// An **opaque** piece — a rendered span, an earlier-recognized macro node, a
/// masked passthrough — is admitted where a family carries it *structurally*
/// rather than reading its bytes: a **display or reference text** built with
/// [`macro_text_children`] keeps the piece's own node as a child (see
/// `xref::find_xref_matches`, `links::find_link_macro_matches`, and
/// `links::build_inline_link_node`, the three reference-bearing families to
/// have taken that lift), the way a footnote's content always has.
/// Every value a family *computes* — a target, an attribute list, a display
/// text baked into one `Text` — still defers on it, since the markup an opaque
/// piece folds to exists only at fold time.
pub(super) fn apply_macros<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    // The bibliography anchor (`[[[label]]]`) runs before every other family,
    // exactly as the string step runs its own `INLINE_BIBLIO_ANCHOR` pass
    // first. It runs *only here*, at the content's own top level — its pattern
    // is `^`-anchored, so it can only ever match the very start of the whole
    // content, never the start of a span's children (see
    // [`biblio_anchor_level`]) — which is why it sits outside
    // [`apply_macro_families`]'s own recursion.
    let nodes = biblio_anchor_level(nodes, root, parser);

    apply_macro_families(nodes, root, parser)
}

/// Applies each macro family at this level — and, first, at every level nested
/// inside it — in the string step's own family order. See
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
    // `InlineEmailReplacer` — so an address that is really the tail of a URL, or
    // a `mailto:` macro's own target, is already inside an opaque node (there,
    // already-rendered `<a …>` markup) and is not re-recognized.
    let nodes = email_level(nodes, root);

    // Inline anchors (`[[id]]`, `anchor:id[…]`) run after the link families and
    // before cross-references, mirroring the string step's order. (The
    // bibliography-anchor pass the string step runs *first* — a `^`-anchored
    // `[[[id]]]`, recognized only inside a bibliography list item — runs in
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
    // regardless of nesting depth — see [`apply_footnotes`], run once, as its
    // own step in [`build`], after `apply_macros` has fully resolved every
    // other family at every level.
}

/// The eventual cutover's single entry point (design §5.2, Phase 4 step 6) for
/// **every** recognition side effect the macro families above defer —
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
/// anchors, … — see [`apply_macros`]'s own doc comment): bibliography anchor,
/// then image, then link, then anchor/ref.
/// This is not cosmetic — it is what keeps this function's output identical to
/// the golden pipeline's whenever more than one family's side effect touches
/// the *same* shared list. Concretely, [`Parser::record_substitution_warning`]
/// appends to one shared warnings list, and both
/// [`image::apply_image_side_effects`]'s dangerous-link-scheme warning and
/// [`anchors::apply_ref_side_effects`]'s duplicate-id warning write to it — a
/// content whose image triggers the first and whose anchor triggers the
/// second must see the image warning recorded first, exactly as it would from
/// the string pipeline's own image-then-anchor pass order. The same holds one
/// step earlier for [`anchors::apply_biblio_side_effects`]'s own duplicate-id
/// warning, which the string pipeline's first pass records ahead of both. (The
/// asset/ref catalogs the three write to are otherwise disjoint from one
/// another — images, links, and refs are three separate lists — so this
/// ordering does not, by itself, need to hold *within* a single catalog; see
/// [`links::apply_link_side_effects`]'s own doc comment for the finer-grained
/// ordering *within* the link family that the golden pipeline also requires.)
///
/// Index terms, cross-references, and footnotes are not part of this
/// function: index terms and cross-references perform no recognition side
/// effect at all (an index term has no catalog in the HTML backend; a
/// cross-reference is resolved, not registered), and a footnote's one
/// required side effect — its assigned number — is not deferred in the first
/// place (see [`apply_footnotes`](super::footnotes::apply_footnotes)'s own doc
/// comment); it already runs during [`build`](super::build), not here.
///
/// As with each of the three functions it composes, **nothing here is wired
/// into a real parse path yet** — it is exercised only by this module's own
/// tests, against their own `Parser`. `source` and `leading_anchor_registered`
/// are threaded straight through to
/// [`anchors::apply_ref_side_effects`] — see its own doc comment for both.
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
    /// nodes, replacing nothing — mirroring the string replacer's
    /// `caps[0][1..]`. The backslash is at the match start for the
    /// prefix-less macros, and at the scheme for an escaped auto-link whose
    /// match carries a boundary prefix.
    Unescape { backslash: usize },

    /// A recognized macro, built into its node ([`Image`](InlineNode::Image),
    /// [`Ui`](InlineNode::Ui), [`Ref`](InlineNode::Ref), …). Boxed to keep this
    /// enum small — a macro node is far larger than the
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

/// Builds the display-text children for a macro whose text is taken **straight
/// from the level's match string** (rather than computed by an attribute-list
/// parse or derived from the target). Shared by the families that recognize
/// such a text — the cross-reference spellings
/// ([`xref::build_xref_node`](xref) and its shorthand) and the
/// `link:`/`mailto:` macro ([`links::build_link_node`](links)) — so the one
/// subtle part, the escaped-special rebuild below, cannot drift between them.
///
/// `raw_text` is the match string's own bytes for `text_range`, and
/// `unescape_bracket` selects the one behavior the callers do *not* share: the
/// `xref:` and `link:`/`mailto:` macro forms unescape an escaped closing
/// bracket (`\]`) in their bracketed text, mirroring their replacers' own
/// `replace("\\]", "]")`, while the `<<id,text>>` shorthand — which has no
/// bracket to escape, and whose own branch of `InlineXrefReplacer` performs no
/// such replace — keeps the pair literal.
///
/// The common case is one [`Text`](InlineNode::Text) child: an unescaped
/// bracket makes its logical value a computed (owned) one — a *synthesized*
/// `Text` whose value need not coincide with its source — while otherwise
/// [`text_slice`] recovers the range's own bytes, borrowing a single verbatim
/// run (the builder's `'src`-borrowing goal, §4.5) and, for a text crossing a
/// [`synthesized`](Piece::synthesized) run, taking the match string's bytes —
/// the expanded value exactly, and the very text the string replacer matched
/// over — as an owned value, with only its location falling back to the
/// enclosing run's coarse span (design §4.4).
///
/// [`text_slice`] rather than `text_location.data()`, because a verbatim range
/// is **not always contiguous in the source**: an earlier step can drop a byte
/// from the flow without splicing a node in its place, leaving two adjacent
/// verbatim runs whose match-string bytes run on while their source spans skip
/// one. An escaped attribute reference is exactly that (`link:x[\{name}]`,
/// whose backslash
/// [`apply_attribute_references`](super::attribute_refs::apply_attribute_references)
/// drops as a *gap* in the ranges it emits), and re-reading the enclosing
/// source span would put the backslash back — a text the string pipeline no
/// longer carries. Slicing the pieces themselves, as [`emit_range`] already
/// does for the structured path below, cannot reintroduce it.
///
/// A text crossing an **escaped special** (`xref:sec[a<b]`, `link:x[a<b]`) or a
/// **restored entity** (`xref:sec[a &copy; b]`) — or, degenerately, one
/// [`text_slice`] declines to recover — instead becomes
/// **structured children**, recovered with [`emit_range`]: the
/// leaf is its own [`CharRef`](crate::inlines::CharRef) child that folds
/// back to the same bytes the string replacer's text carries, where one `Text`
/// child holding the match string's `&lt;` (or `&copy;`) would be escaped a
/// second time by
/// the fold (design §3.4).
///
/// A text crossing an **opaque** piece (a rendered span, an
/// earlier-recognized macro node, a masked passthrough) takes that same
/// structured path, and is where it earns its keep: [`emit_range`] clones the
/// piece's whole node into the children, so the text carries the construct
/// itself rather than the markup it will fold to — the recovery a footnote's
/// own content has always used. Only a caller that admits such a piece reaches
/// this (see `xref::find_xref_matches`, `links::find_link_macro_matches`, and
/// `links::build_inline_link_node`);
/// the callers whose gate is still
/// [`range_has_no_opaque_piece`](image::range_has_no_opaque_piece) throughout
/// never hand one in.
pub(super) fn macro_text_children<'src>(
    raw_text: &str,
    text_range: std::ops::Range<usize>,
    unescape_bracket: bool,
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    root: Span<'src>,
) -> Vec<InlineNode<'src>> {
    let text_location = source_slice(pieces, text_range.clone(), root);

    // The one-`Text` value, when this range has one: `None` selects the
    // structured rebuild below.
    let value = if !range_is_verbatim_or_synthesized(pieces, &text_range) {
        None
    } else if unescape_bracket && raw_text.contains("\\]") {
        Some(CowStr::from(raw_text.replace("\\]", "]")))
    } else {
        text_slice(nodes, pieces, text_range.clone())
    };

    match value {
        Some(value) => vec![InlineNode::Text {
            value,
            location: text_location,
        }],

        None => {
            // The text crosses an escaped special (the only atomic piece the
            // callers' gate admits) — or, degenerately, is a range `text_slice`
            // declined to recover. Rebuild it out of the nodes it covers, so each
            // special stays the `CharRef` it already is.
            //
            // The macro forms' `\]` unescape is expressed here as a *gap* in the
            // emitted ranges — every byte but the backslash is emitted — rather
            // than as a `replace` over each recovered node. Doing it per node would
            // miss a pair astride two adjacent runs, which two `Text` nodes can be
            // without an atomic piece between them: an attribute expansion splices
            // its value as its own node, so a value ending in a backslash followed
            // by a literal `]` (`:t: b\`, then `xref:foo[a<{t}]x]`) puts the two
            // characters in different runs. Skipping the backslash by range is
            // boundary-agnostic, and leaves every surviving fragment borrowing
            // `'src` (§4.5) where a rebuilt value would have had to own its bytes.
            let mut children = Vec::new();
            let mut cursor = text_range.start;

            if unescape_bracket {
                // `match_indices` scans non-overlapping and left to right, exactly
                // as `str::replace` does, so a run of backslashes pairs off
                // identically.
                for (offset, _) in raw_text.match_indices("\\]") {
                    let backslash = text_range.start + offset;
                    emit_range(nodes, pieces, cursor..backslash, &mut children);
                    cursor = backslash + 1;
                }
            }

            emit_range(nodes, pieces, cursor..text_range.end, &mut children);

            children
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use super::{super::test_support::golden_macros_with, apply_macro_side_effects, apply_macros};
    use crate::{
        Parser, Span,
        content::{
            Content, SubstitutionGroup,
            inline_builder::{build, build_for_group, fold_html},
        },
        inlines::{InlineNode, Ref, RefVariant},
        parser::{HtmlSubstitutionRenderer, ModificationContext},
        strings::CowStr,
        warnings::WarningType,
    };

    #[test]
    fn a_display_text_after_an_escaped_attribute_reference_drops_the_backslash() {
        // `macro_text_children` recovers its value with `text_slice`, not by
        // re-reading the enclosing source span, precisely because a *verbatim*
        // range need not be contiguous in the source: the attribute-references
        // step drops an escaped reference's backslash as a gap in the ranges it
        // emits, leaving two adjacent verbatim runs whose match-string bytes
        // run on while their source spans skip one. Re-reading the span would
        // put the backslash back — a text the string pipeline no longer
        // carries — so this pins all three families that build a display text
        // through the shared helper.
        let parser = Parser::default();

        for source in [
            r"http://google.com[\{name}]",
            r"link:index.html[\{name}]",
            r"mailto:team@example.org[\{name}]",
            r"xref:target[\{name}]",
            r"<<target,a \{name} b>>",
        ] {
            let mut content = Content::from(Span::new(source));
            SubstitutionGroup::Normal.apply(&mut content, &parser, None);

            let nodes = build_for_group(
                &SubstitutionGroup::Normal,
                CowStr::from(source),
                Span::new(source),
                &parser,
                None,
            );

            // The golden itself carries no backslash — the escaped reference
            // drops it — so this one comparison pins both the parity and the
            // drop. (No `{source:?}` message: a multi-line assertion's message
            // argument is a failure-only region, which the coverage report
            // counts as an uncovered line in a file whose tests it measures.)
            let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {}, &parser);

            assert_eq!(folded, content.rendered_html());
            assert!(!folded.contains('\\'));
        }
    }

    #[test]
    fn the_other_families_recognize_a_construct_crossing_a_restored_entity() {
        // A restored entity is a property of the *piece*, not of any one
        // family, so admitting it lifts the boundary wherever a family's own
        // gate is the opaque-piece one — including the families with no
        // escaped-special corpus of their own, which this pins. (The UI
        // family and a footnote's text reach the same parity through their
        // own route; see
        // `the_ui_and_footnote_families_recognize_a_construct_crossing_a_recoverable_piece`
        // below.)
        let parser = Parser::default().with_intrinsic_attribute(
            "experimental",
            "",
            ModificationContext::Anywhere,
        );

        for source in [
            // Index terms: the flow form, the concealed form, and both macros.
            "((term &copy; other))",
            "(((a &copy; b, c)))",
            "indexterm:[a &copy; b]",
            "indexterm2:[a &copy; b]",
            // Anchors: the macro form, the shorthand, and the bibliography
            // anchor.
            "anchor:id[Tom &copy; Jerry]",
            "[[i&copy;d]]text",
            "[[[b&copy;bref]]] entry",
            // A footnote *id*, which is read off the match string (its text
            // is a separate capture — see the companion test).
            "footnote:i&copy;d[Tom]",
            // A bare e-mail address, whose own local part admits an entity.
            "doc&copy;a@example.org",
            // Inline STEM, whose expression is a passthrough.
            "stem:[a &copy; b]",
        ] {
            let mut content = Content::from(Span::new(source));
            SubstitutionGroup::Normal.apply(&mut content, &parser, None);

            let nodes = build_for_group(
                &SubstitutionGroup::Normal,
                CowStr::from(source),
                Span::new(source),
                &parser,
                None,
            );

            assert_eq!(
                fold_html(&nodes, &HtmlSubstitutionRenderer {}, &parser),
                content.rendered_html(),
                "fold diverged from the string pipeline for {source:?}"
            );
        }
    }

    #[test]
    fn the_ui_and_footnote_families_recognize_a_construct_crossing_a_recoverable_piece() {
        // The two families this module's `the_other_families_recognize_a_
        // construct_crossing_a_restored_entity` companion once excluded now
        // take the same lift, closing the escaped-special / restored-entity
        // boundary for every macro family:
        //
        // - The **UI** family (`kbd:`/`btn:`/`menu:`) swapped its own gate for the
        //   shared opaque-piece one. Every value a `Ui` node holds is
        //   already-substituted text read out of the match string, which is exactly
        //   what the string replacer computes from its own escaped haystack.
        //
        // - A **footnote's text** needed no code change at all: its content is
        //   structured children (`emit_range` keeps a `CharRef` leaf as its own child),
        //   so it never sliced `'src` for a value in the first place. What made this
        //   look like a boundary was the *harness*: the test that pinned it drove the
        //   golden pipeline and the builder from one shared `Parser`, so each fixture's
        //   footnote was numbered twice (`1` on the golden side, `2` on the built side)
        //   and the two sides "diverged" for a reason that had nothing to do with the
        //   entity. Hence `parity`, below, which configures one parser per side — the
        //   two-independent-parsers discipline every footnote-bearing corpus in this
        //   module already uses.
        let configure = || {
            Parser::default().with_intrinsic_attribute(
                "experimental",
                "",
                ModificationContext::Anywhere,
            )
        };

        let parity = |source: &str| {
            let mut content = Content::from(Span::new(source));
            SubstitutionGroup::Normal.apply(&mut content, &configure(), None);

            let built_parser = configure();

            let nodes = build_for_group(
                &SubstitutionGroup::Normal,
                CowStr::from(source),
                Span::new(source),
                &built_parser,
                None,
            );

            assert_eq!(
                fold_html(&nodes, &HtmlSubstitutionRenderer {}, &built_parser),
                content.rendered_html(),
                "fold diverged from the string pipeline for {source:?}"
            );
        };

        for (special, entity) in [
            ("kbd:[Ctrl&C]", "kbd:[Ctrl&copy;C]"),
            ("btn:[Save & Close]", "btn:[Save &copy; Close]"),
            ("menu:F&le[Save]", "menu:F&copy;le[Save]"),
            ("menu:File[Save & Exit]", "menu:File[Save &copy; Exit]"),
            ("footnote:[Tom & Jerry]", "footnote:[Tom &copy; Jerry]"),
        ] {
            parity(special);
            parity(entity);
        }
    }

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
        // from the anchor family) — the golden pipeline's own image-then-
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
        // one shared warnings list — the same ordering this function's own doc
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
