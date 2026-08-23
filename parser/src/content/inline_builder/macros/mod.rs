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

use super::{
    quotes::{
        LevelContext, Piece, charref_entity, emit_range, source_slice, special_entity, text_slice,
    },
    special_chars::{Masked, unescaped_value_children},
};
use crate::{
    Parser, Span,
    attributes::{Attrlist, AttrlistContext},
    content::restored_entity_pattern,
    inlines::{CharRef, InlineNode},
    strings::CowStr,
};

/// Whether the escaping step has already run by the time the macros step reads
/// a value off the level's match string.
///
/// Design §3.4.1 decides what a fragment is worth by *where the steps sit* in
/// the group's effective order, not by where the fragment came from. The
/// attribute-references step already makes that decision for the value it
/// splices in
/// ([`SplicedSpecials`](super::attribute_refs::SplicedSpecials)); this is the
/// same decision for the one thing the macros step reads as bytes rather than
/// carries structurally — an attribute list's positional value, which comes
/// back from a *parse* and so has no range of nodes to rebuild from.
///
/// Both halves are decided in
/// [`build_for_group`](super::build_for_group), where the effective order is
/// in hand, and carried down to the families that compute such a value (the
/// `link:`/`mailto:` macro, the auto-link / formal-URL family, and the
/// cross-reference macro).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ComputedSpecials {
    /// `SpecialCharacters` sits **before** `Macros` in this order, so a
    /// computed value holds the already-escaped bytes the string replacer
    /// parsed out of its own escaped haystack: `&lt;` is an escaped special
    /// standing for a logical `<`, which [`escaped_value_children`] unwinds one
    /// level so the fold escapes it back exactly once (§3.4).
    Escaped,

    /// No `SpecialCharacters` step has run yet — either the order has none at
    /// all (`subs=attributes,macros`) or it runs *after* `Macros`
    /// (`subs=macros,specialcharacters`) — so a computed value holds the
    /// author's own bytes, which the string replacer splices in verbatim:
    /// `&lt;` is six literal characters, not an escaped `<`, and
    /// [`unescaped_value_children`] classifies it as such.
    Verbatim,
}

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
/// [`Attrlist`] — the step that makes a macro node
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
/// [`Attrlist`]`<'src>`, parsed from the source's
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
    masked: Masked<'_>,
    specials: ComputedSpecials,
) -> Vec<InlineNode<'src>> {
    // The bibliography anchor (`[[[label]]]`) runs before every other family,
    // exactly as the string step runs its own `INLINE_BIBLIO_ANCHOR` pass
    // first. It runs *only here*, at the content's own top level — its pattern
    // is `^`-anchored, so it can only ever match the very start of the whole
    // content, never the start of a span's children (see
    // [`biblio_anchor_level`]) — which is why it sits outside
    // [`apply_macro_families`]'s own recursion, and why it needs no
    // [`LevelContext`] of its own: the top level is the one level nothing
    // encloses.
    let nodes = biblio_anchor_level(nodes, root, parser, masked);

    apply_macro_families(nodes, root, parser, LevelContext::ROOT, masked, specials)
}

/// Applies each macro family at this level — and, first, at every level nested
/// inside it — in the string step's own family order. See
/// [`apply_macros`], which wraps this with the once-per-content
/// bibliography-anchor pass.
///
/// # The boundary an enclosing construct presents
///
/// `ctx` is the [`LevelContext`] this level sits in — the character an
/// enclosing construct's own rendering presents immediately before it, which
/// the string pipeline's flat haystack holds there and a level matched in
/// isolation does not. Two of this step's families read exactly that
/// character: `INLINE_LINK`'s boundary-prefix group and `INLINE_EMAIL`'s
/// "prefix that causes a mismatch" group (see
/// [`links::inline_link_level`] and [`links::email_level`]). Without it,
/// `*doc@example.org*` links here while the string pipeline — reading the `>`
/// that ends `<strong>`, one of that group's own mismatch characters — leaves
/// the address literal.
///
/// Each level's own match string is wrapped in that character before any
/// family's pattern runs over it, and the level's [`Piece`]s are moved into the
/// wrapped string's coordinates with [`LevelContext::shift`] — so a family
/// reads one coordinate system throughout and nothing else in this module
/// changes signature. Only the **opening** character is applied, for the
/// reason [`LevelContext::shift`] itself documents: a boundary class reads one
/// character, while a macro *body* class consumes greedily and would swallow a
/// half-supplied closing one.
///
/// The seven other families read no character before their match — each is
/// anchored on a literal (`image:`, `kbd:`, `menu:`, `indexterm`, `((`, `[[`,
/// `anchor:`, `&lt;&lt;`, `xref:`, `link:`, `mailto:`) that no context
/// character can begin — so for them the wrap is inert, and they take it for
/// uniformity rather than for effect.
///
/// A span that renders **no markup of its own** encloses nothing, so its
/// children read what stands beside the *span* instead; that is
/// [`LevelContext::child_contexts`]'s own job, and this function takes the
/// answer it gives without caring which of the two it is.
///
/// # The boundary a sibling presents
///
/// The same two families read the character a construct written *beside* a
/// rendered span presents, which
/// [`build_match_string`](super::quotes::build_match_string) writes around
/// that span's own placeholder — the two outer characters of its markup, or,
/// for a span rendering to its **body** and nothing else, that body's own two
/// outer characters. Either one it can write only for a span the string
/// pipeline has really rendered, not for the passthrough-extraction pass's own
/// wrapper (see [`Masked`]), whose body the pipeline is holding inside its own
/// sentinel whatever that body renders as. This step is where `masked` is
/// carried, and only here, because these two prefix groups are the only
/// classes in the whole module that read a tag's `>` differently from the
/// bare placeholder: `**bold**https://example.org` links in the string
/// pipeline, reading the `>` that ends `</strong>`, where a quote sub's own
/// `(^|[^\w&;:}])` accepts the two alike. Every other step goes on passing
/// [`Masked::UNKNOWN`](super::special_chars::Masked::UNKNOWN), which is not a
/// shortcut but the same answer stated once.
fn apply_macro_families<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
    ctx: LevelContext,
    masked: Masked<'_>,
    specials: ComputedSpecials,
) -> Vec<InlineNode<'src>> {
    // Recurse into spans/refs first, matching the string pipeline's
    // whole-string pass. Each nested level is matched in the context its own
    // enclosing construct's rendering presents (see this function's own doc
    // comment); a span the built-in backend renders with no markup of its own
    // is transparent, so `child_contexts` hands its children the character its
    // own *siblings* present instead.
    let contexts = LevelContext::child_contexts(&nodes, ctx, masked);

    let nodes: Vec<InlineNode<'src>> = nodes
        .into_iter()
        .zip(contexts)
        .map(|(node, inner)| match node {
            // An extraction wrapper's body is **not** this content's to
            // substitute. The string pipeline holds it as a sentinel for every
            // step from here on, and the body was already substituted once —
            // by the separate, nested `Normal` build the `x-` compatibility
            // form (`[x-]++text++`) runs it through — so descending here would
            // apply this step's families to it a *second* time
            // (`[x-]++**b**https://example.org++` grows an `<a>` inside the
            // `<a>` that build already made). It is also the one place this
            // level's `masked` could not answer even if it did descend, since
            // the list is collected from one content's own top level and knows
            // nothing of a nested build's nodes.
            InlineNode::Styled(styled) if masked.covers(styled.location) => {
                InlineNode::Styled(styled)
            }

            InlineNode::Styled(mut styled) => {
                styled.children =
                    apply_macro_families(styled.children, root, parser, inner, masked, specials);
                InlineNode::Styled(styled)
            }

            InlineNode::Ref(mut reference) => {
                reference.children =
                    apply_macro_families(reference.children, root, parser, inner, masked, specials);
                InlineNode::Ref(reference)
            }

            other => other,
        })
        .collect();

    // The UI macros run before image/icon and only under `experimental`,
    // mirroring the string step's order and gate.
    let nodes = if parser.is_attribute_set("experimental") {
        let nodes = kbd_btn_macros_level(nodes, root, ctx, masked);
        menu_macros_level(nodes, root, ctx, masked)
    } else {
        nodes
    };

    let nodes = image_macros_level(nodes, root, parser, ctx, masked);

    // Index terms (`((term))`, `(((primary, secondary)))`, `indexterm:[…]`,
    // `indexterm2:[…]`) run after image/icon and before the link families,
    // mirroring the string step's order.
    let nodes = indexterm_macros_level(nodes, root, parser, ctx, masked);

    // A **visible** term's shown text is not a boundary the later families
    // stop at. The string replacer puts that text back into the one flat
    // haystack every later pass scans — `((a term with a link:t.html[T]))`
    // links exactly as if the parentheses were not there — so the families
    // below descend into the term's own
    // [`children`](crate::inlines::IndexTerm::children), which is where this
    // pass has just put that text. (A *concealed* term shows nothing, so the
    // replacer leaves no text behind for them to scan and its `children` are
    // empty.)
    //
    // Their own level, rather than one flat scan across the term and its
    // neighbours: a term renders its shown text with no markup of its own, so
    // its children read what stands beside the *term*, which is exactly the
    // transparent case [`LevelContext::child_contexts`] already answers.
    //
    // The scan comes first because most levels hold no such term at all, and
    // `child_contexts` builds a `Vec` (and, for a transparent span, this
    // level's whole match string) that would then be thrown away.
    let has_shown_term = nodes
        .iter()
        .any(|node| matches!(node, InlineNode::IndexTerm(term) if !term.children.is_empty()));

    let nodes = if has_shown_term {
        let contexts = LevelContext::child_contexts(&nodes, ctx, masked);

        nodes
            .into_iter()
            .zip(contexts)
            .map(|(node, inner)| match node {
                InlineNode::IndexTerm(mut index_term) if !index_term.children.is_empty() => {
                    index_term.children = apply_reference_families(
                        std::mem::take(&mut index_term.children),
                        root,
                        parser,
                        inner,
                        masked,
                        specials,
                    );

                    InlineNode::IndexTerm(index_term)
                }

                other => other,
            })
            .collect()
    } else {
        nodes
    };

    apply_reference_families(nodes, root, parser, ctx, masked, specials)

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

/// The macro families that run **after** the index-term pass — the three link
/// spellings, inline anchors, and cross-references — in the string step's own
/// order.
///
/// Split out of [`apply_macro_families`] because it has two callers rather
/// than one: this level, and a visible index term's shown text, which the
/// string replacer hands back to exactly these passes. See the call site for
/// why the term's children are their own level.
fn apply_reference_families<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
    ctx: LevelContext,
    masked: Masked<'_>,
    specials: ComputedSpecials,
) -> Vec<InlineNode<'src>> {
    // Auto-links and formal-URL links (`INLINE_LINK`) run after the index-term
    // pass and before the `link:`/`mailto:` macro, mirroring the string step's
    // order (`INLINE_LINK` precedes `INLINE_LINK_MACRO`).
    let nodes = inline_link_level(nodes, root, parser, ctx, masked, specials);

    // The `link:`/`mailto:` macro runs after the auto-link pass, mirroring the
    // string step's order.
    let nodes = link_macro_level(nodes, root, parser, ctx, masked, specials);

    // A bare e-mail address (`doc@example.org`) runs after both URL-link
    // families and before the anchor pass, exactly where the string step runs
    // `InlineEmailReplacer` — so an address that is really the tail of a URL, or
    // a `mailto:` macro's own target, is already inside an opaque node (there,
    // already-rendered `<a …>` markup) and is not re-recognized.
    let nodes = email_level(nodes, root, ctx, masked);

    // Inline anchors (`[[id]]`, `anchor:id[…]`) run after the link families and
    // before cross-references, mirroring the string step's order. (The
    // bibliography-anchor pass the string step runs *first* — a `^`-anchored
    // `[[[id]]]`, recognized only inside a bibliography list item — runs in
    // [`apply_macros`], outside this recursion.)
    let nodes = anchor_macros_level(nodes, root, ctx, masked);

    // Cross-references (`xref:id[…]`) run after the anchor pass, mirroring the
    // string step's order.
    xref_macros_level(nodes, root, parser, ctx, masked, specials)
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
/// **restored entity** (`xref:sec[a &copy; b]`), or a **typographic
/// replacement** (`xref:sec[a (C) b]`) — or, degenerately, one
/// [`text_slice`] declines to recover — instead becomes
/// **structured children**, recovered with [`emit_range`]: the
/// leaf is its own [`CharRef`] child that folds
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
            let mut children = Vec::new();

            if unescape_bracket {
                emit_range_unescaping_brackets(raw_text, text_range, nodes, pieces, &mut children);
            } else {
                emit_range(nodes, pieces, text_range, &mut children);
            }

            children
        }
    }
}

/// Emits the nodes `range` covers into `out` (as [`emit_range`] does), with
/// each **escaped closing bracket**'s backslash dropped — the `\]` unescape
/// every bracket-bearing macro form performs, expressed structurally.
///
/// `raw_text` is the level's own match-string slice for `range` (the bytes the
/// string replacer's `replace("\\]", "]")` runs over), so the two agree on
/// which backslashes pair off: [`str::match_indices`] scans non-overlapping and
/// left to right, exactly as [`str::replace`] does, so a run of backslashes
/// resolves identically on both sides.
///
/// The unescape is a *gap* in the emitted ranges — every byte but the backslash
/// is emitted — rather than a `replace` over each recovered node. Doing it per
/// node would miss a pair astride two adjacent runs, which two
/// [`Text`](InlineNode::Text) nodes can be without an atomic piece between
/// them: an attribute expansion splices its value as its own node, so a value
/// ending in a backslash followed by a literal `]` (`:t: b\`, then
/// `xref:foo[a<{t}]x]`) puts the two characters in different runs. Skipping the
/// backslash by range is boundary-agnostic, and leaves every surviving fragment
/// borrowing `'src` (§4.5) where a rebuilt value would have had to own its
/// bytes.
///
/// Shared with the footnote family — whose content is *always* structured
/// children, so it has no one-`Text` fast path to take the unescape through —
/// so the two cannot drift on it.
pub(in crate::content::inline_builder) fn emit_range_unescaping_brackets<'src>(
    raw_text: &str,
    range: std::ops::Range<usize>,
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    out: &mut Vec<InlineNode<'src>>,
) {
    let mut cursor = range.start;

    for (offset, _) in raw_text.match_indices("\\]") {
        let backslash = range.start + offset;
        emit_range(nodes, pieces, cursor..backslash, out);
        cursor = backslash + 1;
    }

    emit_range(nodes, pieces, cursor..range.end, out);
}

/// Rewrites the **opaque** pieces inside one computed text's own match-string
/// bytes into index-keyed `\u{96}`*n*`\u{97}` tokens, returning that text
/// alongside the nodes those tokens stand for.
///
/// This is [`tokened_bracket`](image::tokened_bracket)'s counterpart for a
/// value that is **carried structurally** rather than restored as bytes, and
/// the two differ in exactly one way. A masked passthrough or STEM expression
/// has a body known at build time, so `tokened_bracket` pairs each token with
/// it and the caller can splice those bytes back into a *parsed* value. A
/// rendered [`Styled`](crate::inlines::Styled) span — or any
/// earlier-recognized macro node — has no such body: its markup exists only
/// when the tree is folded. There is still nothing to restore *into bytes*,
/// but there is something to carry: the **node**. So this tokens every atomic
/// piece [`build_match_string`](super::quotes::build_match_string) stands in
/// as one placeholder, whatever kind it is, and the caller splices each node
/// back through [`restored_value_children`].
///
/// Why tokening at all, when the placeholder is already one opaque character:
/// so the split sees the string pipeline's own **shape** there. The
/// replacer parses an attribute list over the piece's *markup*, which carries
/// no `,`/`=`/`"` the split could read differently than it reads a token —
/// and a bare placeholder, being a single `Co` character, would be
/// indistinguishable from one *inside* a value once the parse hands that value
/// back. The token's index is what makes the walk back out
/// unambiguous, exactly as it is for
/// [`tokened_bracket`](image::tokened_bracket).
///
/// Renumbering is per call, from zero, so a token the parse drops shifts none
/// of the ones that survive.
pub(super) fn tokened_text<'src>(
    text: &str,
    range: &std::ops::Range<usize>,
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
) -> (String, Vec<InlineNode<'src>>) {
    let mut tokened = String::new();
    let mut carried: Vec<InlineNode<'src>> = Vec::new();

    // Walked **piece by piece** rather than by copying the gaps between the
    // opaque ones, because a byte of the match string may belong to no piece
    // at all: `styled_sibling_boundaries` wraps an opaque span's placeholder
    // in the two characters its own rendering presents to a neighbour (the `<`
    // and `>` of a tag), which exist for *recognition* and stand for markup
    // this token already carries whole. Copying them would splice a stray `<`
    // and `>` into the value beside the node.
    for piece in pieces {
        let p_start = piece.s_start;
        let p_end = piece.s_start + piece.s_len;

        // Skip pieces that do not overlap the range.
        if p_end <= range.start || p_start >= range.end {
            continue;
        }

        let lo = p_start.max(range.start);
        let hi = p_end.min(range.end);

        // The pieces that become a token are the **opaque** ones — atomic, and
        // not one of the three [`CharRef`](InlineNode::CharRef) leaves
        // [`build_match_string`](super::quotes::build_match_string) gives real
        // bytes to. Every other piece contributes its own bytes as it stands:
        // those are the string replacer's bytes there, so the split reads them
        // identically and the caller's own rebuild
        // ([`computed_value_children`]) already knows how to unwind them.
        let opaque = piece
            .atomic
            .then(|| nodes.get(piece.node_index))
            .flatten()
            .filter(|node| charref_entity(node).is_none());

        match opaque {
            Some(node) => {
                tokened.push_str(&format!("\u{96}{n}\u{97}", n = carried.len()));
                carried.push(node.clone());
            }

            None => tokened.push_str(
                text.get(lo.saturating_sub(range.start)..hi.saturating_sub(range.start))
                    .unwrap_or_default(),
            ),
        }
    }

    if carried.is_empty() {
        return (text.to_string(), carried);
    }

    (tokened, carried)
}

/// The bytes the **string replacer's own haystack** holds for a text
/// [`tokened_text`] tokened: each token replaced by what the fold of the node
/// it stands for emits, with the parser's own renderer — the renderer the
/// string pipeline ran. A **masked** construct's token is left alone: the
/// replacer's own haystack holds a sentinel there too.
///
/// This is the other half of the token's contract. Tokening is what makes the
/// attribute-list *split* reproducible: a bracket's `,` / `=` / `"` are the
/// only bytes it reads, and an opaque piece's placeholder carries none of
/// them. But the replacer splits over the piece's own **markup**, which may:
/// `xref:sec[a *b, c* d,role=hl]` renders `a <strong>b, c</strong> d`, and its
/// list splits at the comma *inside* the tag. A caller compares the two
/// parses to find out (see [`tokened_split_agrees`]).
pub(super) fn restored_markup_text(
    tokened: &str,
    carried: &[InlineNode<'_>],
    parser: &Parser,
) -> String {
    let mut out = tokened.to_string();

    for (n, node) in carried.iter().enumerate() {
        // A **masked** construct is left as its token. The replacer's haystack
        // holds its own `\u{96}`*n*`\u{97}` sentinel there — not the body,
        // which `Passthroughs::restore_to` splices only after every step has
        // run — so the two sides already agree on those bytes, and expanding
        // one here would compare the tokened split against a string the
        // replacer never split.
        if image::node_is_restorable(node) {
            continue;
        }

        let markup =
            super::fold::fold_html(std::slice::from_ref(node), parser.renderer.as_ref(), parser);

        out = out.replace(&format!("\u{96}{n}\u{97}"), &markup);
    }

    out
}

/// Whether an attribute list parsed from a [`tokened_text`] text splits the
/// same way the string replacer's own parse of the **markup** does.
///
/// A token stands in for markup the replacer really sees, so the two parses
/// agree exactly when no character the split reads is hidden inside a piece.
/// Rather than guess at that from the bytes — an ordinary `*bold*` hides
/// nothing, while `[.r]#x#` renders a `"` and an `=` that the split
/// nonetheless reads harmlessly — this asks the parser: it compares the two
/// lists attribute by attribute, expanding the tokened side's tokens first, so
/// a split that moved shows up as a value that differs.
///
/// It answers only the *split*, not what the caller then does with each value.
/// A token reaching a value the caller reads as a **string** — a
/// cross-reference's `window=` / `role=` / `xrefstyle=` — splits identically
/// on both sides and still has no bytes the caller can use, so a caller with
/// such a slot asks [`holds_carried_token`] about it as well.
pub(super) fn tokened_split_agrees(
    tokened: &str,
    carried: &[InlineNode<'_>],
    parser: &Parser,
) -> bool {
    let restored = restored_markup_text(tokened, carried, parser);

    let tokened_attrs = Attrlist::parse(Span::new(tokened), parser, AttrlistContext::Inline)
        .item
        .item;

    let restored_attrs = Attrlist::parse(Span::new(&restored), parser, AttrlistContext::Inline)
        .item
        .item;

    let tokened_list: Vec<_> = tokened_attrs.attributes().collect();
    let restored_list: Vec<_> = restored_attrs.attributes().collect();

    tokened_list.len() == restored_list.len()
        && tokened_list
            .iter()
            .zip(restored_list.iter())
            .all(|(tokened_attr, restored_attr)| {
                tokened_attr.name() == restored_attr.name()
                    && restored_markup_text(tokened_attr.value(), carried, parser)
                        == restored_attr.value()
            })
}

/// Whether `value` — one attribute the parse handed back — still holds a
/// [`tokened_text`] token, which means an opaque piece landed in a value the
/// caller reads as a **string** rather than carrying structurally.
///
/// A node has no bytes there, so such a match is deferred. This is the same
/// per-*slot* boundary [`text_attrlist`](links)'s own `pre_restore` draws,
/// reached for the stronger reason: a masked construct at least has a body to
/// splice.
pub(super) fn holds_carried_token(value: &str) -> bool {
    value.contains('\u{96}')
}

/// Rebuilds a computed value that still holds [`tokened_text`] /
/// [`tokened_bracket`](image::tokened_bracket) tokens as a node's children:
/// each token becomes the node it stands for, and the bytes around it take
/// [`computed_value_children`]'s own rebuild.
///
/// Splicing the node rather than its bytes is what keeps the fold honest. A
/// [`Raw`](InlineNode::Raw) leaf's body is emitted verbatim and a
/// [`Stem`](InlineNode::Stem) leaf's is rendered at fold time — which is
/// exactly what `Passthroughs::restore_to` splices into the string pipeline's
/// own display text — where those same bytes spliced into a
/// [`Text`](InlineNode::Text) would be escaped a second time (design §3.4):
/// `link:x[++<b>a</b>++,role=hl]` would show `&amp;lt;b&amp;gt;` against the
/// golden's `&lt;b&gt;`. A rendered
/// [`Styled`](crate::inlines::Styled) span has no bytes to splice at all —
/// its markup exists only at fold time — so for it the node *is* the only
/// honest reading.
///
/// The walk is index-keyed and left to right, like
/// [`Passthroughs::restore_to`](crate::content::Passthroughs)'s own: each
/// token is sought only in the bytes after the previous one, and a token the
/// parse dropped (a value the split discarded) is simply not found, leaving
/// the ones after it to splice by their own index.
pub(super) fn restored_value_children<'src>(
    text: &str,
    restores: &[InlineNode<'src>],
    location: Span<'src>,
    specials: ComputedSpecials,
) -> Vec<InlineNode<'src>> {
    let mut children: Vec<InlineNode<'src>> = Vec::new();
    let mut rest = text;

    for (n, node) in restores.iter().enumerate() {
        let token = format!("\u{96}{n}\u{97}");

        if let Some(pos) = rest.find(&token) {
            children.append(&mut computed_value_children(
                rest.get(..pos).unwrap_or_default(),
                location,
                specials,
            ));

            children.push(node.clone());
            rest = rest.get(pos + token.len()..).unwrap_or_default();
        }
    }

    children.append(&mut computed_value_children(rest, location, specials));

    children
}

/// Rebuilds a value the macros step **computed** off the level's match string
/// as the children a node shows, taking whichever half of design §3.4.1
/// `specials` names.
///
/// A computed value is the one thing this step reads as *bytes* rather than
/// carrying structurally: it comes back from an
/// [`Attrlist`] parse, so there is no range of
/// nodes to rebuild it from and its classification has to be re-derived from
/// the bytes themselves. What those bytes are worth is exactly what
/// [`ComputedSpecials`] answers — see its two halves,
/// [`escaped_value_children`] and [`unescaped_value_children`].
pub(super) fn computed_value_children<'src>(
    text: &str,
    location: Span<'src>,
    specials: ComputedSpecials,
) -> Vec<InlineNode<'src>> {
    match specials {
        ComputedSpecials::Escaped => escaped_value_children(text, location),
        ComputedSpecials::Verbatim => unescaped_value_children(text, location),
    }
}

/// Rebuilds an **already-escaped computed value** — a value a family read off
/// the level's own match string rather than out of the tree, here a reference-
/// or link-family attribute list's positional value — as the nodes that fold
/// back to exactly those bytes, all sharing `location` (the value has no `'src`
/// slice of its own, so design §4.4's coarse fallback is the only honest span
/// for any part of it).
///
/// The rebuild is design §3.4's trichotomy applied to a string:
/// [`build_match_string`](super::quotes::build_match_string) gives an escaped
/// special its canonical entity and a *restored* entity its own bytes, while a
/// [`Text`](InlineNode::Text) node holds **logical** text the fold escapes. So
/// the two classes come apart here — the special becomes the character it
/// stands for (inside a `Text` the fold escapes back), and the restored entity
/// becomes its own [`CharRef`](InlineNode::CharRef)`::Entity` leaf (which the
/// fold emits verbatim) — where one `Text` holding both would escape the
/// entity's `&` a second time.
///
/// The scan is left to right, one `&` at a time, precisely because the two
/// classes can nest: `&amp;copy;` is a literal `&` followed by the letters
/// `copy;`, **not** a `&copy;` entity, and only consuming the `&amp;` first
/// tells them apart. That is the same one-level unwind the fold performs in
/// reverse.
///
/// Splitting on the three specials is exact rather than heuristic: inside such
/// a value an `&` can only ever open one of these two classes or stand alone —
/// a verbatim run holds no special at all (the `SpecialCharacters` step split
/// every one into its own leaf) and a synthesized run's own specials are
/// [`Raw`](InlineNode::Raw) leaves, which are opaque and so rejected by the
/// caller's gate. Under an effective order that never escapes (§3.4.1) a
/// literal `&` *does* survive into a verbatim run, and is left exactly as it
/// is here, since neither class matches it.
///
/// That last branch — a bare `&` opening neither class — is what makes the scan
/// total over arbitrary bytes, and no effective order reaches it: this half is
/// called only for an order that has already escaped
/// ([`ComputedSpecials::Escaped`], dispatched by
/// [`computed_value_children`]), and under one of those every `&` in the
/// level's match string belongs to a leaf that opens a class — a
/// [`Special`](CharRef::Special)'s canonical entity, a restored
/// [`Entity`](CharRef::Entity)'s own bytes, or a typographic
/// [`Replacement`](CharRef::Replacement)'s numeric one. A *literal* `&` (an
/// attribute expansion splicing one in after the escaping step) is a
/// [`Raw`](InlineNode::Raw) leaf, which
/// [`build_match_string`](super::quotes::build_match_string) stands in as an
/// opaque placeholder carrying no `&` at all — so it never reaches a computed
/// value as bytes. The branch is pinned by a direct unit test rather than by
/// the corpus.
///
/// Visible past this module for [`unescaped_value_children`]'s own doc link
/// alone; [`computed_value_children`] is the entry point every caller takes.
pub(super) fn escaped_value_children<'src>(
    text: &str,
    location: Span<'src>,
) -> Vec<InlineNode<'src>> {
    let mut children: Vec<InlineNode<'src>> = Vec::new();
    let mut pending = String::new();
    let mut rest = text;

    while let Some(at) = rest.find('&') {
        let (before, from_amp) = rest.split_at(at);

        // Everything before this `&` is ordinary logical text.
        pending.push_str(before);

        let consumed = if let Some((ch, len)) = special_of(from_amp) {
            // An escaped special: the character it stands for, which the fold
            // escapes back to these very bytes.
            pending.push(ch);
            len
        } else if let Some(entity) = restored_entity_pattern().find(from_amp) {
            // A restored entity: its own leaf, emitted verbatim by the fold.
            if !pending.is_empty() {
                children.push(InlineNode::Text {
                    value: CowStr::from(std::mem::take(&mut pending)),
                    location,
                });
            }

            children.push(InlineNode::CharRef {
                value: CharRef::Entity(CowStr::from(entity.as_str().to_string())),
                location,
            });

            // The pattern is `^`-anchored, so the match ends where the entity
            // does.
            entity.end()
        } else {
            // A bare `&` that opens neither class: ordinary logical text like
            // any other byte. This is what makes the scan total over arbitrary
            // bytes; no effective order presents one here (see this function's
            // own doc comment), so it is pinned by
            // [`a_bare_ampersand_in_a_computed_value_stays_logical_text`]
            // rather than by the corpus.
            pending.push('&');
            '&'.len_utf8()
        };

        rest = from_amp.get(consumed..).unwrap_or_default();
    }

    pending.push_str(rest);

    if !pending.is_empty() {
        children.push(InlineNode::Text {
            value: CowStr::from(pending),
            location,
        });
    }

    children
}

/// The character an escaped special's canonical entity at the start of `rest`
/// stands for, with the entity's own length, or `None` when `rest` does not
/// open one.
fn special_of(rest: &str) -> Option<(char, usize)> {
    // The same three characters `SpecialCharacters` splits into their own
    // leaves, read through `build_match_string`'s own mapping rather than a
    // second copy of it.
    for ch in ['<', '>', '&'] {
        let entity = special_entity(ch);

        if rest.starts_with(entity) {
            return Some((ch, entity.len()));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use super::{
        super::{special_chars::Masked, test_support::golden_macros_with},
        ComputedSpecials, apply_macro_side_effects, apply_macros, escaped_value_children,
    };
    use crate::{
        Parser, Span,
        content::{
            Content, SubstitutionGroup, SubstitutionStep,
            inline_builder::{build, build_for_group, fold_html},
        },
        inlines::{CharRef, InlineNode, Ref, RefVariant},
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
    fn a_computed_value_is_classified_by_where_the_escaping_step_sits() {
        // [`escaped_value_children`] reads its input as the *escaped* text a
        // family took off the level's match string, so it unwinds one level of
        // §3.4's trichotomy: `&lt;` becomes the logical `<` a `Text` node
        // holds, which the fold escapes back. That is exact under an order
        // that runs `SpecialCharacters` **before** `Macros` — every built-in
        // group — and exactly wrong under one that does not, where the same
        // six bytes are the author's own and the string replacer splices them
        // in untouched.
        //
        // An attribute list is parsed under *any* order that runs `Macros`, so
        // both readings are reachable, and which one a value takes is decided
        // the way §3.4.1 decides every other classification: by where the
        // escaping step actually sits in the group's effective order. This
        // pins that decision from both sides over one fixture per family — the
        // three that compute an attribute-list value — so neither half can
        // drift into the other's order.
        let parser = Parser::default().with_intrinsic_attribute(
            "product",
            "Widget",
            ModificationContext::Anywhere,
        );

        // Escapes before `Macros`, as every built-in group does.
        let escaping = SubstitutionGroup::Custom(vec![
            SubstitutionStep::SpecialCharacters,
            SubstitutionStep::AttributeReferences,
            SubstitutionStep::Macros,
        ]);

        // Never escapes at all — `subs=attributes,macros`.
        let verbatim = SubstitutionGroup::Custom(vec![
            SubstitutionStep::AttributeReferences,
            SubstitutionStep::Macros,
        ]);

        // The value is non-verbatim in every fixture (an attribute expansion
        // splices into it), which is what routes it through the computed-value
        // rebuild rather than through `macro_text_children`'s own ranges.
        for text in ["{product} < y", "{product} &lt; y"] {
            for shape in [
                "xref:sec[TEXT,role=hl] here",
                "link:index.html[TEXT,role=hl] here",
                "https://example.org[TEXT,role=hl] here",
            ] {
                let source = shape.replace("TEXT", text);

                // The truth table this increment closes, in the order the two
                // groups are declared above:
                //
                //   `<`     escaping → `&lt;`     (the escaping step ran, so
                //                                  the value holds `&lt;`,
                //                                  which unwinds to a logical
                //                                  `<` the fold escapes back)
                //   `<`     verbatim → `<`        (nothing escaped it, in
                //                                  either pipeline)
                //   `&lt;`  escaping → `&amp;lt;` (the escaping step escaped
                //                                  the author's own `&`)
                //   `&lt;`  verbatim → `&lt;`     (the author's six bytes,
                //                                  spliced in untouched — the
                //                                  cell that used to unwind
                //                                  one level too far)
                for (group, expected) in [
                    (
                        &escaping,
                        if text.contains('&') {
                            "Widget &amp;lt; y"
                        } else {
                            "Widget &lt; y"
                        },
                    ),
                    (
                        &verbatim,
                        if text.contains('&') {
                            "Widget &lt; y"
                        } else {
                            "Widget < y"
                        },
                    ),
                ] {
                    let mut content = Content::from(Span::new(&source));
                    group.apply(&mut content, &parser, None);

                    let nodes = build_for_group(
                        group,
                        CowStr::from(source.clone()),
                        Span::new(&source),
                        &parser,
                        None,
                    );

                    let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {}, &parser);

                    assert_eq!(folded, content.rendered_html(), "{source:?}");
                    assert!(folded.contains(expected), "{source:?}: {folded:?}");
                }
            }
        }

        // The `Verbatim` half is *not* "the order has no escaping step" but
        // "none has run yet", so it covers an order that escapes **after**
        // `Macros` too. Only the cross-reference family can be pinned to
        // parity there, because it is the one family whose node the string
        // pipeline is still holding as a deferred placeholder when the
        // escaping step runs — so `flatten_prior_markup` leaves it alone,
        // where a link's already-emitted `<a …>` markup is escaped by the
        // string pipeline and is the separate divergence
        // `a_markup_producing_step_before_the_escaping_one_is_a_documented_divergence`
        // pins. Both spellings reach parity; the bare `<` did not before the
        // decision became position-aware.
        let escaping_last = SubstitutionGroup::Custom(vec![
            SubstitutionStep::AttributeReferences,
            SubstitutionStep::Macros,
            SubstitutionStep::SpecialCharacters,
        ]);

        for (source, expected) in [
            ("xref:sec[{product} < y,role=hl] here", "Widget < y"),
            ("xref:sec[{product} &lt; y,role=hl] here", "Widget &lt; y"),
        ] {
            let mut content = Content::from(Span::new(source));
            escaping_last.apply(&mut content, &parser, None);

            let nodes = build_for_group(
                &escaping_last,
                CowStr::from(source),
                Span::new(source),
                &parser,
                None,
            );

            let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {}, &parser);

            assert_eq!(folded, content.rendered_html(), "{source:?}");
            assert!(folded.contains(expected), "{source:?}: {folded:?}");
        }
    }

    #[test]
    fn a_bare_ampersand_in_a_computed_value_stays_logical_text() {
        // The third arm of `escaped_value_children`'s scan — an `&` opening
        // neither of the two recoverable classes — is what makes it total over
        // arbitrary bytes, and no effective order presents one: the half runs
        // only for an order that has already escaped, and there every `&` in
        // the level's match string is a `CharRef` leaf's own entity, while a
        // literal one is a `Raw` leaf the match string stands in as an opaque
        // placeholder. So it is pinned directly, on the classification it
        // makes: one logical `Text` run, which the fold escapes back to the
        // `&amp;` the string pipeline would have emitted.
        let src = Span::new("x");
        let children = escaped_value_children("a & b &copy; c &lt; d", src);

        assert_eq!(
            children
                .iter()
                .filter(|n| matches!(n, InlineNode::Text { .. }))
                .count(),
            2,
            "{children:?}"
        );

        // The bare `&` stays inside its own run as logical text; only the
        // entity comes out as its own leaf, and only the escaped special is
        // unwound.
        assert!(
            matches!(&children[0], InlineNode::Text { value, .. } if value.as_ref() == "a & b "),
            "{children:?}"
        );
        assert!(
            matches!(&children[1], InlineNode::CharRef { value: CharRef::Entity(e), .. }
                if e.as_ref() == "&copy;"),
            "{children:?}"
        );
        assert!(
            matches!(&children[2], InlineNode::Text { value, .. } if value.as_ref() == " c < d"),
            "{children:?}"
        );
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
            link_form: Some(crate::inlines::LinkForm::Macro),
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

        let out = apply_macros(
            vec![reference],
            root,
            &Parser::default(),
            Masked::UNKNOWN,
            ComputedSpecials::Escaped,
        );

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
    fn a_family_matching_across_an_earlier_familys_markup_is_a_documented_divergence() {
        // Found by the cross-product sweep in this module's parent
        // (`fold_matches_the_real_pipeline_for_every_construct_in_every_container`),
        // and the same class as
        // [`flatten_prior_markup`](super::super::special_chars::flatten_prior_markup)'s
        // own — a step acting on another step's *emitted markup* — one level
        // finer: not a later **step** reading an earlier step's tags, but a
        // later **family of this same step** reading an earlier family's.
        //
        // The string pipeline runs its macro families as passes over one flat
        // string, so by the time the cross-reference and footnote passes run,
        // that string already holds the `</a>` the link pass wrote. Their
        // patterns match straight through it, and the bracket a family reads
        // as its own can begin inside one link's display text and end outside
        // it: `link:t.html[pre xref:tgt[T] post]` renders an `<a>` nested
        // inside an `<a>`, because the cross-reference's own bracketed text is
        // `T</a> post`.
        //
        // A tree has no tags to match through. The link's display text is a
        // subtree, and every family that runs after the link pass sees nodes,
        // so a bracket cannot span the boundary at all — the construct stays
        // where it was written, and the link keeps the text it was given.
        //
        // This is a **keep**: the tree's answer is the well-formed one, and
        // the string pipeline's is markup no backend would choose to emit.
        let parser = Parser::default();

        for (source, golden_html, folded_html) in [
            (
                "x link:t.html[pre xref:tgt[T] post] y",
                r##"x <a href="t.html">pre <a href="#tgt">T</a> post</a> y"##,
                r##"x <a href="t.html">pre xref:tgt[T</a> post] y"##,
            ),
            (
                "x link:t.html[pre footnote:[n] post] y",
                concat!(
                    r##"x <a href="t.html">pre <sup class="footnote">[<a id="_footnoteref_1" "##,
                    r##"class="footnote" href="#_footnotedef_1" title="View footnote.">1</a>]"##,
                    r##"</sup> y"##,
                ),
                r##"x <a href="t.html">pre footnote:[n</a> post] y"##,
            ),
        ] {
            let mut content = Content::from(Span::new(source));
            SubstitutionGroup::Normal.apply(&mut content, &parser, None);

            assert_eq!(content.rendered_str(), golden_html);

            assert_eq!(
                fold_html(
                    &build(Span::new(source), &Parser::default(), None),
                    &HtmlSubstitutionRenderer {},
                    &Parser::default(),
                ),
                folded_html,
            );
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
