//! Cross-reference recognition (`xref:id[…]`, `<<id>>`).

// Referenced by the doc comments below, whose own rebuild is the one this
// family reaches through `restored_value_children`; the code no longer calls
// it directly.
// Referenced by the doc comments below; the code itself reaches the level's
// match string through [`shifted_level`]'s shared slot now.
#[allow(unused_imports)]
use super::super::quotes::build_match_string;
#[allow(unused_imports)]
use super::computed_value_children;
use super::{
    ComputedSpecials, LevelSniff, MacroMatch, MacroMatchKind, XREF_DIGRAMS,
    image::{range_has_no_opaque_piece, range_is_substitution_restorable},
    links::restore_masked_passthroughs,
    macro_text_children, rebuild_macro_level, restored_value_children, tokened_text,
    untranslated_value,
};
use crate::{
    Parser, Span,
    attributes::{
        Attrlist,
        element_attribute::{MASKED_PIECE_PLACEHOLDER_END, MASKED_PIECE_PLACEHOLDER_START},
    },
    content::{
        INLINE_XREF, document_xrefstyle,
        inline_builder::{
            quotes::{LevelContext, Piece, source_slice},
            special_chars::Masked,
        },
        xref_target::{
            XrefTarget, interpret_xref_target, other_document_reference, this_document_reference,
        },
    },
    inlines::{InlineNode, Ref, RefVariant},
    parser::{DerivedReference, XrefStyle},
    strings::CowStr,
};

/// Interprets a cross-reference `target` and computes the pieces the [`Ref`]
/// node needs to render it, mirroring
/// [`InlineXrefReplacer::replace_append`](crate::content::macros)'s own target
/// interpretation exactly so the fold reproduces the same bytes:
///
/// - a same-document reference to a specific id resolves through the catalog
///   later, so it carries no *derived* destination (`derived: None`);
/// - the empty target (`xref:#[]`, `<<>>`) names the current document as a
///   whole, and a target naming another document — or a file that was included
///   into this one in full, which is a reference within it after all — carries
///   a destination *derived* from the target itself, computed here from the
///   path attributes in effect at the reference (no catalog consulted).
///
/// The returned target is the node's `Ref::target` (see its field docs): the
/// interpreted id for a same-document reference, the fragment for a
/// same-document inclusion, or the raw target as written for a genuine
/// inter-document reference.
fn xref_target_and_derived(
    raw_target: &str,
    macro_form: bool,
    parser: &Parser,
) -> (String, Option<DerivedReference>) {
    match interpret_xref_target(raw_target, macro_form) {
        XrefTarget::SameDocument(id) if id.is_empty() => {
            (id, Some(this_document_reference(parser)))
        }

        XrefTarget::SameDocument(id) => (id, None),

        // A target that names *this* document, or a file that was included
        // into it in full, is a reference within it after all.
        XrefTarget::OtherDocument {
            path,
            source,
            fragment,
        } if source
            && (parser.docname().as_deref() == Some(path.as_str())
                || parser.catalog_include_is_full(&path)) =>
        {
            match fragment {
                Some(fragment) => (fragment, None),
                None => (String::new(), Some(this_document_reference(parser))),
            }
        }

        XrefTarget::OtherDocument {
            path,
            source,
            fragment,
        } => {
            let derived = other_document_reference(parser, &path, source, fragment.as_deref());
            (raw_target.to_string(), Some(derived))
        }
    }
}

/// Both the `xref:` macro form and the `<<id>>` shorthand (seen here as
/// `&lt;&lt;id&gt;&gt;`, since specials run before macros) are recognized.
/// Triggers on either the macro prefix or the shorthand's `&lt;&lt;` opener,
/// mirroring the string step's `text.contains("&lt;&lt;") ||
/// (found_macroish && text.contains("xref:"))` guard. Shared between
/// [`xref_macros_level`]'s pre-build sniff and its post-build one, so the two
/// answers cannot drift apart.
fn xref_prefilter(s: &str) -> bool {
    s.contains("xref:") || s.contains("&lt;&lt;")
}

/// Matches `INLINE_XREF` at this level's escaped text, replacing each
/// recognized `xref:` macro with the [`Ref`](InlineNode::Ref)`{Xref}` node it
/// produces and leaving everything else in place.
pub(super) fn xref_macros_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
    ctx: LevelContext,
    masked: Masked<'_>,
    specials: ComputedSpecials,
    level: &mut LevelSniff,
) -> Vec<InlineNode<'src>> {
    let (s, pieces, digrams) = level.shifted(&nodes, ctx, masked);

    // Cheap pre-filter: both the `xref:` macro form and the `<<id>>` shorthand
    // (seen here as `&lt;&lt;id&gt;&gt;`, since specials run before macros) are
    // recognized. The prefilter triggers on either the macro prefix or the
    // shorthand's `&lt;&lt;` opener, mirroring the string step's
    // `text.contains("&lt;&lt;") || (found_macroish && text.contains("xref:"))`
    // guard.
    if digrams & XREF_DIGRAMS == 0 || !xref_prefilter(s) {
        return nodes;
    }

    let matches = find_xref_matches(&nodes, s, pieces, root, parser, specials);

    if matches.is_empty() {
        return nodes;
    }

    let rebuilt = rebuild_macro_level(&nodes, pieces, s, matches);
    level.invalidate();
    rebuilt
}

/// A shorthand whose id holds rendered inline markup is not a valid
/// reference — Asciidoctor leaves it untouched
/// (`<<link:https://example.com[], Example>>`) — so this refuses it the same
/// way, via `id.contains('<')`.
///
/// This became reachable only with
/// [`range_is_substitution_restorable`].
/// Before it, markup in an id was always an *opaque* piece, so the gate refused
/// the match before any id existed to check — which is why
/// [`build_xref_shorthand_node`] documents needing no counterpart to the guard.
/// A substitution-produced `<` is not opaque (`:markup: <b>x</b>`, then
/// `<<{markup}>>`), so the check has to be made for real now.
///
/// It reads the **restored** id, since that is what the replacer's `id` holds:
/// a `<` hiding behind a placeholder is still a `<` to it.
fn shorthand_id_has_no_rendered_markup(
    s: &str,
    id_range: &std::ops::Range<usize>,
    nodes: &[InlineNode<'_>],
    pieces: &[Piece],
    parser: &Parser,
) -> bool {
    let matched = s.get(id_range.start..id_range.end).unwrap_or_default();

    !restored_range(matched, id_range.clone(), nodes, pieces, parser).contains('<')
}

/// The bytes a range of the level's match string holds once every placeholder
/// standing in for a **substitution-produced** [`Raw`](InlineNode::Raw) leaf is
/// filled in — the fully-resolved bytes the construct's rendered value is
/// computed from.
///
/// Borrowed unchanged when the range crosses no such leaf, which is every
/// ordinary cross-reference.
///
/// Only reached for a range
/// [`range_is_substitution_restorable`]
/// admitted, so the splice never reaches a masked construct — whose bytes the
/// replacer would *not* have held yet, and which keeps its match deferred.
fn restored_range<'a>(
    matched: &'a str,
    range: std::ops::Range<usize>,
    nodes: &[InlineNode<'_>],
    pieces: &[Piece],
    parser: &Parser,
) -> std::borrow::Cow<'a, str> {
    restore_masked_passthroughs(matched, &range, nodes, pieces, parser.renderer.as_ref())
        .map_or(std::borrow::Cow::Borrowed(matched), |(text, _)| {
            std::borrow::Cow::Owned(text)
        })
}

/// Finds every recognized cross-reference at this level — the `xref:` macro
/// form and the `<<id>>` shorthand — skipping any match whose **computed
/// values** cross an **opaque** piece (see [`range_has_no_opaque_piece`]).
/// That gate is the family's *only* deferral: both builders claim every target
/// and text shape an admitted match can carry.
///
/// The gate covers the bytes the node *reads*, not the whole match. A
/// cross-reference computes two values from the level's match string — its
/// **target** (the `xref:` macro's group 3, the shorthand's own id half) and,
/// when the text carries an attribute list, that list's parsed positional value
/// — and each needs a match string whose bytes are already fully resolved and
/// escaped. A **reference text**, by contrast, becomes *structured children*
/// ([`macro_text_children`]), so it needs no recoverable bytes at all; see the
/// rendered-span section below.
///
/// A [`synthesized`](Piece::synthesized) run (an attribute expansion, or —
/// reached at a tree's root — a filtered multi-line block's own joined seed)
/// **is** admitted, which is what lets a cross-reference be recognized inside
/// an expanded attribute value (`xref:{id}[{text}]`, `<<{id},text>>`). Nothing
/// on a cross-reference node is `Span`-typed: its target and reference text
/// come straight out of the level's match string — which carries a synthesized
/// run's bytes exactly — and its own attribute list is parsed from a normalized
/// *copy* rather than a source slice (see
/// [`xref_macro_text`]), so `attrs` is always `None` here. Only the node's
/// `location` (and its children's) takes the coarse fallback span used when a
/// construct has no `Span`-typed field of its own. This
/// is the same lift the anchor, bare-e-mail, UI, and index-term families
/// already made, and for the same reason; the families that hold a real
/// [`Attrlist`]`<'src>` (image, link) still cannot make it.
///
/// An **escaped special** (`xref:sec[a<b]`, `<<sec,Tom & Jerry>>`) and a
/// **restored entity** (`xref:sec[Tom &copy; Jerry]`) are admitted for the same
/// reason: the level's match string carries the
/// [`CharRef`](InlineNode::CharRef) leaf's own bytes — a `Special`'s canonical
/// entity, an `Entity`'s entity itself — so every
/// value this family computes off that string (the target, the
/// `raw_text.contains('=')` attribute-list probe, the attrlist parse itself,
/// the shorthand's `split_once(',')`) sees the construct's fully-escaped
/// bytes directly. The reference *text* is then rebuilt as structured children
/// rather than one sliced [`Text`](InlineNode::Text) (see
/// [`macro_text_children`]), so the leaf folds back to its own bytes instead of
/// being escaped twice — and the attribute-list branch, whose value comes back
/// from a parse rather than from a range, re-derives the same split with
/// [`computed_value_children`].
///
/// # A rendered span inside the reference text
///
/// A **rendered span** — a [`Styled`](crate::inlines::Styled) span, an
/// already-recognized macro node, a masked passthrough — is *not* recoverable:
/// it is one opaque placeholder here, standing in for markup (or a passthrough
/// mask) that exists only at fold time. It is nonetheless admitted **inside a
/// reference text**,
/// because a reference text is the one capture this family never reads as
/// bytes: it becomes the node's children through [`macro_text_children`], whose
/// [`emit_range`](super::super::quotes::emit_range) path clones the opaque
/// piece's own node whole into them — so the text is carried *structurally*,
/// and the fold re-renders exactly that markup. This is the same "nesting is
/// the point" recovery a footnote's own content has always used, applied to
/// the display text of a reference.
///
/// What that admission cannot do is make the *recognition* agree in every case:
/// matching over one placeholder instead of the markup itself reads a
/// different extent whenever that markup carries a character the pattern is
/// sensitive to, which leaves two documented divergences of *extent* (each
/// pinned by its own test); in both, the well-formed reading is the tree's,
/// not the one a match over the raw, markup-perturbed text would give —
/// exactly as the quotes step's crossed-delimiter divergence is:
///
/// - a `]` inside the span (`xref:sec[*a ] b*]`), which would end the macro
///   form's own lazy text capture early if matched over raw markup, but not
///   here;
/// - a `&gt;&gt;` inside the span (`<<sec,*a >> b*>>`), the shorthand's own
///   terminator, for the same reason.
///
/// A text carrying an **attribute list** keeps the stricter gate, and is
/// therefore deferred whenever it crosses an opaque piece: its display text
/// comes back from an [`Attrlist`] parse of the match string rather than from a
/// range of it (see [`xref_macro_text`]), and a placeholder inside a *parsed*
/// value cannot be mapped back to the node it stands in for — the same reason
/// the image and link families defer their own `Attrlist`-bearing captures. The
/// probe for that branch is `raw_text.contains('=')`, read here off the match
/// string; matching over rendered markup instead would read it off the markup
/// itself, so a span whose markup carries an `=` (an attributed span, a link,
/// an image) would take the attribute-list branch where this one stays plain.
/// That costs nothing wherever the parse finds the `=` incidental (an attribute
/// list with no comma to split on yields one positional value equal to the
/// whole text, which is every unattributed markup shape) and is a third
/// documented divergence otherwise.
/// The two shapes an [`INLINE_XREF`] match decomposes into, in match-string
/// byte offsets: the `&lt;&lt;…&gt;&gt;` shorthand's inner text (the pattern's
/// group 2), or the `xref:` macro's target and bracketed text (groups 3
/// and 4). The escape backslash (group 1) is not carried here — the caller
/// reads it off the span's first byte, as it always has.
#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
enum XrefGroups {
    /// A `&lt;&lt;refid[, reftext]&gt;&gt;` shorthand.
    Shorthand {
        /// The text between the escaped angle brackets.
        inner: std::ops::Range<usize>,
    },

    /// An `xref:target[text]` macro.
    Macro {
        /// The target between `xref:` and the opening `[`.
        target: std::ops::Range<usize>,

        /// The bracketed display text (possibly empty).
        text: std::ops::Range<usize>,
    },
}

/// The escaped shorthand delimiters' shared length (`&lt;&lt;` and
/// `&gt;&gt;`).
const XREF_SHORTHAND_DELIM: usize = "&lt;&lt;".len();

/// Decomposes one [`INLINE_XREF`] match — `m`, the engine-reported span
/// starting at `start` — into its groups, replacing the capture-engine
/// resolution ([`find_xref_matches`] searches bounds-only): every group the
/// pattern captures is fully determined by the span. After the optional
/// escape backslash, the first byte tells the two alternatives apart (`&` for
/// the shorthand, `x` for the macro). The shorthand's inner text is the span
/// minus its fixed-width delimiters. The macro's target runs from `xref:` to
/// the span's first `[` — the target class excludes `[`, so that `[` is the
/// pattern's own — and the text runs from there to the `]` the span ends
/// with, wherever the lazy body stopped.
///
/// The `unwrap_or` fallback below cannot be taken on an engine-produced span
/// (the macro alternative always carries its `[`); it exists only to keep
/// this reading total. `xref_groups_match_the_capture_engine` pins the whole
/// derivation against the capture engine across the differential corpus.
fn xref_groups(m: &str, start: usize) -> XrefGroups {
    let escape = usize::from(m.as_bytes().first() == Some(&b'\\'));

    if m.as_bytes().get(escape) == Some(&b'&') {
        return XrefGroups::Shorthand {
            inner: (start + escape + XREF_SHORTHAND_DELIM)
                ..(start + m.len() - XREF_SHORTHAND_DELIM),
        };
    }

    let target_start = escape + "xref:".len();
    let bracket = m.find('[').unwrap_or(target_start);

    XrefGroups::Macro {
        target: (start + target_start)..(start + bracket),
        text: (start + bracket + 1)..(start + m.len() - 1),
    }
}

fn find_xref_matches<'src>(
    nodes: &[InlineNode<'src>],
    s: &str,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
    specials: ComputedSpecials,
) -> Vec<MacroMatch<'src>> {
    let mut matches = Vec::new();

    for whole in INLINE_XREF.find_iter(s) {
        let full = whole.range();
        let groups = xref_groups(whole.as_str(), full.start);

        // The `xref:` macro and the `<<…>>` shorthand reach the same two
        // computed values by different spellings. Whichever it is, the gate
        // covers exactly the bytes the node *reads* — its target, and an
        // attribute-list text's parsed value — and not the ones it carries
        // structurally (a reference text) or consumes without reading (the
        // shorthand's own `&lt;&lt;` / `&gt;&gt;` delimiters, always
        // `CharRef`s by macro time).
        let recoverable = match &groups {
            XrefGroups::Shorthand { inner } => {
                // The shorthand's id is its inner up to the first `,` — the
                // very split `build_xref_shorthand_node` makes. A comma the
                // *markup* of an opaque piece contributes
                // cannot move that split unnoticed: such a piece would have to
                // sit in the id half, which this gate then rejects.
                let id_range = shorthand_id_range(s, inner);

                range_is_substitution_restorable(nodes, pieces, &id_range)
                    && shorthand_id_has_no_rendered_markup(s, &id_range, nodes, pieces, parser)
            }

            XrefGroups::Macro { target, text } => {
                // The macro form's target always participates in this
                // branch. Its `xref:` prefix and brackets need no gate of their
                // own: those bytes are literal, and no atomic piece — a
                // placeholder, or an entity delimited by `&` and `;` — can
                // supply them.
                //
                // (The empty-slice fallback cannot be taken: the range is the
                // engine-reported span's, so it always slices.)
                let text_str = s.get(text.clone()).unwrap_or_default();

                // A text carrying an `=` is read as an attribute list. Its
                // *positional* value becomes the node's children, so an opaque
                // piece there is carried as the node itself
                // ([`tokened_text`]); a piece reaching one of the three values
                // this family reads as a **string** — a `window=`, a `role=`,
                // an `xrefstyle=` — has no bytes to be read as, and that shape
                // alone keeps the gate. Deciding it means performing the same
                // tokened parse the builder performs, on the same bytes: this
                // gate already re-derives the shorthand's own comma split for
                // the same reason.
                let attrlist_text = text_str.contains('=');

                range_is_substitution_restorable(nodes, pieces, target)
                    && (!attrlist_text
                        || range_has_no_opaque_piece(nodes, pieces, text)
                        || attrlist_text_carries_its_opaque_pieces(
                            text_str, text, nodes, pieces, parser,
                        ))
            }
        };

        // An escape (`\xref:` / `\<<`) is honored by dropping the backslash and
        // keeping the rest literal. That check is made *before* looking at
        // anything else, so the escape needs no gate of its own here either:
        // dropping the backslash keeps the rest of the match as its
        // **own original nodes** (a rendered span or an escaped special
        // among them), which fold back to exactly the bytes the
        // replacer's `caps[0][1..]` emits. (This is the same
        // check-order fix the `footnoteref:` and menu increments made
        // for their own families; before it, an escaped `\xref:sec[*
        // bold*]` whose match the gate rejected was left unrecognized,
        // backslash and all.)
        if whole.as_str().starts_with('\\') {
            matches.push(MacroMatch {
                kind: MacroMatchKind::Unescape {
                    backslash: full.start,
                },
                full,
            });

            continue;
        }

        if !recoverable {
            continue;
        }

        // Both builders claim every shape they are handed, so an admitted match
        // always yields a node: what a cross-reference *defers* is decided by
        // the gate above, not by the builders.
        let node = match groups {
            XrefGroups::Shorthand { inner } => {
                build_xref_shorthand_node(inner, &full, nodes, s, pieces, root, parser)
            }
            XrefGroups::Macro { target, text } => build_xref_node(
                target, text, &full, nodes, s, pieces, root, parser, specials,
            ),
        };

        matches.push(MacroMatch {
            kind: MacroMatchKind::Node {
                consumed: full.clone(),
                node: Box::new(node),
            },
            full,
        });
    }

    matches
}

/// Whether an attribute-list display text enclosing an opaque piece can be
/// **carried** — the one thing [`find_xref_matches`]'s gate still asks of such
/// a text.
///
/// A [`Ref`]`{Xref}` node holds no [`Attrlist`] of its own: its display text
/// becomes children, and its `window` / `role` / `xrefstyle` are plain strings
/// read off the parse. So the boundary is drawn per **slot** rather than per
/// family, exactly as [`text_attrlist`](super::links)'s own `pre_restore` draws
/// it — a token in the positional value is a node the caller splices back
/// ([`restored_value_children`]), while a token in any of the three computed
/// values names markup that exists only at fold time where a string is needed,
/// and the whole match is left literal.
///
/// The tokened parse this makes is the same one
/// [`xref_macro_text`] makes to build, over the same bytes. Re-deriving it
/// here is what keeps the deferral decision in the gate, where this family's
/// own contract puts it ("both builders claim every shape they are handed").
///
/// Reached only for a text the caller's own
/// [`range_has_no_opaque_piece`] has already refused, so the tokening below
/// always produces at least one token; a text carrying none would answer
/// `true` here, which is what that caller already decided for itself.
fn attrlist_text_carries_its_opaque_pieces(
    raw_text: &str,
    text_range: &std::ops::Range<usize>,
    nodes: &[InlineNode<'_>],
    pieces: &[Piece],
    parser: &Parser,
) -> bool {
    // Author-written token bytes make every token in this text ambiguous — see
    // `text_carries_author_written_token_bytes`.
    if text_carries_author_written_token_bytes(raw_text) {
        return false;
    }

    let (tokened, _carried) = tokened_text(&raw_text.replace('\n', " "), text_range, nodes, pieces);

    let attrlist = Attrlist::parse_tokened(Span::new(&tokened), parser)
        .item
        .item;

    // An incidental `=` (the parse finds no named attribute) leaves the whole
    // text as the sole positional value, which the builder then rebuilds as
    // plain text through [`macro_text_children`] — a path that carries any
    // node structurally already.
    let named_attributes_split = attrlist
        .nth_attribute(1)
        .is_none_or(|first| first.value() != tokened);

    if !named_attributes_split {
        return true;
    }

    // A token reaching one of the three values this family reads as a
    // **string** — `window`, `xrefstyle`, a role — would have no bytes to put
    // in a string slot on its own: `untranslated_value` gives the slot the
    // author's *source* for the piece the token stands for instead, which is
    // a value a string can hold. See its doc comment for the rules and for
    // the deliberate divergence from Asciidoctor that follows.
    true
}

/// Whether `raw_text` — a macro's own **match-string** bytes, before any
/// tokening — carries either byte of the
/// [`MASKED_PIECE_PLACEHOLDER`](crate::attributes::element_attribute::MASKED_PIECE_PLACEHOLDER)
/// pair a [`tokened_text`] occurrence is built from.
///
/// It should not: [`build_match_string`] stands an opaque piece in as
/// [`SPAN_PLACEHOLDER`](super::super::quotes), a private-use codepoint, so
/// every occurrence of either codepoint reaching here is one the **author**
/// wrote — checked individually, not as the adjacent pair, since a stray
/// half sitting beside the *other* half completed by a real placeholder
/// nearby would be just as ambiguous as a whole stray pair.
///
/// Those bytes used to make the whole tokening ambiguous: an occurrence was
/// found by searching the parsed value for its bytes, and the search could
/// not tell an author's own copy from the one this pass emitted. They no
/// longer do — [`tokened_text`] escapes every byte it copies
/// ([`escape_masked_piece_bytes`](crate::attributes::element_attribute::escape_masked_piece_bytes)),
/// so an author's copy is not those bytes by the time the parse sees it —
/// which leaves this gate **conservative** rather than load-bearing. It is
/// kept because the recorded golden this family's corpus compares against
/// reads such a text differently — the recording's own passthrough masking
/// used these same two codepoints, so a text carrying them reached its
/// `xref` pass already confused — and lifting the gate would have the tree
/// claim a shape whose fold diverges from that recording.
///
/// It is a deferral, not a rewrite: the tree claims no construct.
fn text_carries_author_written_token_bytes(raw_text: &str) -> bool {
    raw_text.contains([MASKED_PIECE_PLACEHOLDER_START, MASKED_PIECE_PLACEHOLDER_END])
}

/// The match-string range of a `<<…>>` shorthand's **id half**: its inner up to
/// the first `,`, or the whole inner when it carries none — the very split
/// [`build_xref_shorthand_node`] then makes on the same bytes, matching
/// Asciidoctor's own `inner.split_once(',')`.
///
/// This is the half [`find_xref_matches`] gates, since the id is the one value
/// the shorthand *reads* off the match string; the reference text after the
/// comma is carried structurally and so needs no gate.
fn shorthand_id_range(s: &str, inner: &std::ops::Range<usize>) -> std::ops::Range<usize> {
    let inner_data = s.get(inner.start..inner.end).unwrap_or_default();

    match inner_data.find(',') {
        Some(comma) => inner.start..inner.start + comma,
        None => inner.clone(),
    }
}

/// Builds one [`Ref`](InlineNode::Ref)`{Xref}` node from a verbatim `xref:`
/// macro match, computing the target and display text to match Asciidoctor's
/// own rendering byte-for-byte.
///
/// The scope this builder claims is every macro-form target, including a text
/// carrying an attribute list; the `<<id>>` shorthand is built by
/// [`build_xref_shorthand_node`] and never carries one (see
/// [`Ref::xrefstyle`]'s field docs). A same-document reference to a specific
/// id (`xref:install[]`) resolves through the catalog later (`derived:
/// None`); the empty target (`xref:#[]`), a target naming another document
/// (`xref:other.adoc#frag[]`), and a target naming this document (or a file
/// included into it in full) all carry a destination *derived* from the
/// target itself, computed by [`xref_target_and_derived`].
///
/// The display text becomes the node's children as a single
/// [`Text`](InlineNode::Text), so the fold recovers the provided text by
/// folding the children and needs no build-time state; an empty text yields no
/// children, which the fold reads as "no text provided" (the bracketed `[id]`
/// fallback). See [`xref_macro_text`] for how a text carrying an attribute
/// list (an `=`) is interpreted.
///
/// As in the additive builder generally, this performs *no* recognition side
/// effect — notably it does **not** register the reference for resolution
/// itself; that happens once per parse, at fold time, via
/// `xref_segment_from_node`.
#[allow(clippy::too_many_arguments)]
fn build_xref_node<'src>(
    target: std::ops::Range<usize>,
    text: std::ops::Range<usize>,
    full: &std::ops::Range<usize>,
    nodes: &[InlineNode<'src>],
    s: &str,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
    specials: ComputedSpecials,
) -> InlineNode<'src> {
    // `target` and `text` are the `xref:` macro's own groups, derived from
    // the span by [`xref_groups`]: the caller routes a shorthand match to
    // [`build_xref_shorthand_node`] instead. (The empty-slice fallbacks
    // cannot be taken — both ranges slice the engine-reported span.)
    let target_str = s.get(target.clone()).unwrap_or_default();

    // The target's fully-resolved bytes. A leaf the match
    // string stands in as a placeholder — an expanded attribute value's `&`,
    // say (`xref:{cpp}[…]`, where `{cpp}` is `C&#43;&#43;`) — contributes its
    // own bytes here. The gate admits only such leaves, so the splice always
    // finishes the value into bytes already fully resolved; a
    // *masked* construct, whose bytes are not yet resolved, keeps the
    // match deferred instead.
    let restored_target = restored_range(target_str, target, nodes, pieces, parser);

    let (target, derived) = xref_target_and_derived(restored_target.as_ref(), true, parser);

    let raw_text = s.get(text.clone()).unwrap_or_default();
    let (children, window, roles, xrefstyle) =
        xref_macro_text(raw_text, text, nodes, pieces, root, parser, specials);

    let location = source_slice(pieces, full.clone(), root);

    InlineNode::Ref(Ref {
        variant: RefVariant::Xref,
        link_form: None,
        target: CowStr::from(target),
        children,
        roles,
        window,
        resolved: None,
        derived,

        // The *effective* style, not the macro's override: an
        // `xrefstyle=` on the macro wins, and otherwise the document-wide
        // `xrefstyle` **in effect at this point in the document** is resolved
        // into the node here — every order-dependent fact is resolved into
        // node values at build time, so the fold stays pure.
        xrefstyle: xrefstyle.or_else(|| document_xrefstyle(parser)),
        attrs: Attrlist::empty(location.slice(0..0)),
        location,
    })
}

/// Interprets the bracketed display text of an `xref:` macro, mirroring
/// [`InlineXrefReplacer::replace_append`](crate::content::macros)'s own text
/// interpretation exactly so the fold reproduces the same bytes: a text
/// carrying an `=` is parsed — from a newline-normalized copy, since the parse
/// is not necessarily verbatim (matching Asciidoctor, which parses
/// the same normalized copy rather than a source slice) — as an
/// [`Attrlist`], whose first positional attribute becomes the display text
/// and whose `window`/`role`/`xrefstyle` named attributes are honored. If the
/// attrlist parse finds no named attribute — the sole positional value is the
/// whole normalized text — the `=` was incidental (e.g. an already-rendered
/// inner macro such as `xref:sec[image:...[]]`, whose HTML contains `=` and
/// `"`), not a real attribute list; the text is then used as plain text with
/// no named attributes, matching Asciidoctor's `extract_attributes_from_text`.
///
/// Returns the display-text children, the window, the roles, and the
/// `xrefstyle` override (`None` unless the macro carries its own `xrefstyle=`
/// attribute; the document-wide default is applied later, at fold time — see
/// [`Ref::xrefstyle`]'s field docs).
fn xref_macro_text<'src>(
    raw_text: &str,
    text_range: std::ops::Range<usize>,
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
    specials: ComputedSpecials,
) -> (
    Vec<InlineNode<'src>>,
    Option<CowStr<'src>>,
    Vec<CowStr<'src>>,
    Option<XrefStyle>,
) {
    if raw_text.is_empty() {
        return (vec![], None, vec![], None);
    }

    if raw_text.contains('=') {
        // Tokened before the parse, so an opaque piece the text encloses reads
        // to the split as one indivisible run standing in for its rendered
        // markup (see [`tokened_text`]). A text enclosing none comes
        // back byte-identical.
        //
        let (normalized, carried) =
            tokened_text(&raw_text.replace('\n', " "), &text_range, nodes, pieces);

        let attrlist = Attrlist::parse_tokened(Span::new(&normalized), parser)
            .item
            .item;

        let first = attrlist.nth_attribute(1).map(|a| a.value().to_string());

        if first.as_deref() != Some(normalized.as_str()) {
            // The three values this family reads as **strings**. Each is read
            // off the *tokened* parse, so a value enclosing a rendered span or
            // a masked passthrough holds a placeholder occurrence rather than
            // any bytes of its own; `untranslated_value` puts the author's
            // source back in its place (see its own doc comment for the two
            // rules and for the deliberate divergence from Asciidoctor that
            // follows). Each is a *different* attribute from `carried`'s own
            // global sequence, so each needs its own starting offset (see
            // `untranslated_value`'s doc comment for why a bare occurrence
            // cannot re-align itself).
            let window = attrlist.named_attribute("window").map(|a| {
                let start = attrlist.named_attribute_token_offset("window").unwrap_or(0);

                CowStr::from(untranslated_value(
                    a.value(),
                    carried.get(start..).unwrap_or_default(),
                ))
            });

            // A role from the first positional attribute's own shorthand
            // items and one from a named `role=` attribute are two different
            // attributes, so each needs its own starting offset (see
            // `Attrlist::roles_with_token_offset`'s own doc comment).
            let roles = attrlist
                .roles_with_token_offset()
                .into_iter()
                .map(|(r, start)| {
                    CowStr::from(untranslated_value(
                        r,
                        carried.get(start..).unwrap_or_default(),
                    ))
                })
                .collect();

            let xrefstyle = attrlist.named_attribute("xrefstyle").map(|a| {
                let start = attrlist
                    .named_attribute_token_offset("xrefstyle")
                    .unwrap_or(0);

                XrefStyle::parse(&untranslated_value(
                    a.value(),
                    carried.get(start..).unwrap_or_default(),
                ))
            });

            let children = match first.filter(|s| !s.is_empty()) {
                None => vec![],
                Some(text) => {
                    // The parsed positional attribute is a synthesized value
                    // with no `'src` slice of its own (it comes from the
                    // normalized, attrlist-parsed copy, not the source
                    // directly); it falls back to the bracketed text's own
                    // span, the same synthesized-value location policy
                    // `apply_attribute_references` already establishes.
                    let location = source_slice(pieces, text_range.clone(), root);

                    // Each occurrence this value still holds becomes the node
                    // it stands for, so an enclosed span is carried as the
                    // construct itself and rendered at fold time; the bytes
                    // around it take the same rebuild an occurrence-free value
                    // does.
                    let start = attrlist.nth_attribute_token_offset(1).unwrap_or(0);
                    restored_value_children(
                        &text,
                        carried.get(start..).unwrap_or_default(),
                        location,
                        specials,
                    )
                }
            };

            return (children, window, roles, xrefstyle);
        }

        // The `=` was incidental; fall through to plain-text handling.
    }

    (
        plain_xref_text(raw_text, text_range, nodes, pieces, root),
        None,
        vec![],
        None,
    )
}

/// Builds the display-text children for a text with no attribute list (or one
/// whose `=` was incidental), matching Asciidoctor's own
/// `raw_text.replace("\\]", "]")` unescape.
fn plain_xref_text<'src>(
    raw_text: &str,
    text_range: std::ops::Range<usize>,
    nodes: &[InlineNode<'src>],
    pieces: &[Piece],
    root: Span<'src>,
) -> Vec<InlineNode<'src>> {
    macro_text_children(raw_text, text_range, true, nodes, pieces, root)
}

/// Builds one [`Ref`](InlineNode::Ref)`{Xref}` node from a `<<id>>` shorthand
/// cross-reference, computing the target and display text to match
/// Asciidoctor's own shorthand handling byte-for-byte.
///
/// `inner` is the shorthand's inner text (`INLINE_XREF` group 2) in
/// match-string coordinates. It is split on the first `,` into an id and an
/// optional reference text, each trimmed — matching Asciidoctor's
/// `inner.split_once(',')` with `id.trim()` / `text.trim()`, which runs over
/// the very same bytes.
///
/// The caller guarantees the **id half** (see [`shorthand_id_range`]) crosses
/// no **opaque** piece (see [`range_has_no_opaque_piece`]), so the match string
/// carries the id's bytes exactly — whether they are source bytes (a verbatim
/// run), an expanded attribute value's (a [`synthesized`](Piece::synthesized)
/// run), or an escaped special's own entity — and so, therefore, does the
/// comma that split them. The reference text after that comma carries no such
/// guarantee: it becomes the node's children through [`macro_text_children`] —
/// a single [`Text`](InlineNode::Text) borrowed from `'src` in the common
/// verbatim case, owned when it crosses a synthesized run, structured
/// when it crosses an escaped special or an opaque piece (whose own node the
/// children then carry). The whole `<<…>>` — its `CharRef` delimiters
/// included — is the node's `location` (a synthesized run's coarse enclosing
/// span, for a construct with no `Span`-typed field of its own).
///
/// **A comma is what makes a text *present*, not what it contains.**
/// Asciidoctor's own split records `<<id,>>` (and `<<id,   >>`) as a
/// *present-but-empty* text — `Some("")`, which renders an empty `<a>…</a>`
/// rather than the bracketed `[id]` fallback `None` renders — so a shorthand
/// carrying a comma always builds at least one child, empty value and
/// all (a zero-length `'src` borrow at the position the trim left — an empty
/// text crosses nothing, so it takes [`macro_text_children`]'s single-child
/// path). The fold
/// keys "was a text provided?" on the *presence* of a child rather than on
/// what it folds to, so the two cases stay distinct end to end; see
/// [`fold_xref`](super::super::fold). A shorthand with no comma keeps an empty
/// child vector, which the fold reads as "no text provided".
///
/// The scope this builder claims is every shorthand target. A same-document
/// shorthand (`<<install>>`) resolves through the catalog later (`derived:
/// None`); an inter-document shorthand (`<<other#frag>>`) and the
/// document-as-a-whole shorthand (`<<>>`, an empty id) both carry a
/// destination *derived* from the target itself, computed by
/// [`xref_target_and_derived`] exactly as the macro form's.
///
/// A shorthand whose id already carries a rendered `<` (an earlier-substituted
/// macro, e.g. `<<link:https://example.com[], Example>>`) — which Asciidoctor's
/// own `id.contains('<')` guard leaves untouched — cannot reach here
/// at all: rendered markup is an *opaque* piece, so the caller never calls this
/// builder. That is why no counterpart to the guard is needed here: an id
/// carrying a merely *escaped* `<`, which the gate does admit, is an entity
/// by macro time, so this builder never sees a bare `<` there.
///
/// As in the additive builder generally, this performs *no* recognition side
/// effect — notably it does **not** register the reference for resolution
/// itself; that happens once per parse, at fold time, via
/// `xref_segment_from_node`.
fn build_xref_shorthand_node<'src>(
    inner: std::ops::Range<usize>,
    full: &std::ops::Range<usize>,
    nodes: &[InlineNode<'src>],
    s: &str,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> InlineNode<'src> {
    // The inner crosses no atomic piece (the caller checked), so the match
    // string carries its logical bytes exactly — which is what
    // Asciidoctor's own `inner.split_once(',')` sees. Reading them here rather
    // than through the inner's source slice is what lets a shorthand inside an
    // expanded attribute value be recognized: a synthesized run has no `'src`
    // slice of its own. A byte offset within `inner_data` maps to a
    // match-string offset by adding `inner.start`.
    let inner_data = s.get(inner.start..inner.end).unwrap_or_default();

    // Split an optional ", reference text" off the id at the first comma.
    //
    // Split on the **matched** bytes, not on restored ones. Only the id half is
    // read as a string and so only it is restored (below); the text half
    // becomes structured children, and its range is in match-string
    // coordinates — restoring the whole inner first would shift every offset
    // the text is then sliced with. No comma can hide behind a placeholder
    // anyway: a substitution leaves a `Raw` leaf only for `<`, `>`, and `&`, so
    // a comma in an expanded value is `Text` and stands in the match string
    // itself.
    let comma = inner_data.find(',');

    let raw_id = match comma {
        Some(index) => &inner_data[..index],
        None => inner_data,
    };

    // The id's fully-resolved bytes — see
    // [`restored_range`]. The `trim` is applied after, on the restored value,
    // matching Asciidoctor's own trim of its `id`.
    let id_range = inner.start..inner.start + raw_id.len();
    let restored_id = restored_range(raw_id, id_range, nodes, pieces, parser);

    let (target, derived) = xref_target_and_derived(restored_id.trim(), false, parser);

    let location = source_slice(pieces, full.clone(), root);

    let children = match comma {
        None => vec![],

        Some(index) => {
            let raw_text = &inner_data[index + 1..];
            let trimmed = raw_text.trim();

            // Locate the trimmed reference text at its source. A verbatim text
            // borrows the very bytes its location covers — a zero-length borrow
            // when the text is empty (or whitespace-only), which is the
            // present-but-empty text the doc comment describes — while a
            // synthesized one keeps its exact expanded bytes against the
            // enclosing run's coarse location.
            let lead = raw_text.len() - raw_text.trim_start().len();

            let text_start = inner.start + index + 1 + lead;
            let text_range = text_start..text_start + trimmed.len();

            // The same one-child-or-structured-children split the macro form
            // makes, reached through the shared helper — but with **no** `\]`
            // unescape: the shorthand has no bracket to escape, and
            // Asciidoctor's own shorthand branch performs no such replace, so a
            // `\]` written here stays literal. An empty (or
            // whitespace-only) text crosses nothing, so it takes the helper's
            // single-child path and keeps the zero-length child the fold keys
            // `provided_text` on.
            macro_text_children(trimmed, text_range, false, nodes, pieces, root)
        }
    };

    InlineNode::Ref(Ref {
        variant: RefVariant::Xref,
        link_form: None,
        target: CowStr::from(target),
        children,
        roles: vec![],
        window: None,
        resolved: None,
        derived,

        // The shorthand carries no attribute list, so its effective style is
        // the document-wide one, resolved here for the same reason the macro
        // form resolves it here.
        xrefstyle: document_xrefstyle(parser),
        attrs: Attrlist::empty(location.slice(0..0)),
        location,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use super::super::super::{
        build,
        test_support::{
            assert_entity, assert_styled, assert_text, build_src, fold_html, link_text_of,
        },
    };
    use crate::{
        HasSpan, Parser, Span,
        inlines::{CharRef, InlineNode, Ref, RefVariant, SpanForm, StyleVariant},
        parser::{HtmlInlineRenderer, XrefStyle},
    };

    #[test]
    fn xref_groups_match_the_capture_engine() {
        // `xref_groups` derives the pattern's groups from each match span;
        // this pins it against the capture engine's own reading, group for
        // group, across a corpus covering both alternatives, escapes, empty
        // and comma-carrying shorthand inners, empty and attribute-carrying
        // macro texts, escaped and nested brackets, multibyte targets, and
        // matches at every offset a multi-match haystack produces.
        use super::{INLINE_XREF, XrefGroups, xref_groups};

        let haystacks = [
            "&lt;&lt;a&gt;&gt;",
            "&lt;&lt;a,Reference Text&gt;&gt;",
            "&lt;&lt;&gt;&gt;",
            r"\&lt;&lt;a&gt;&gt;",
            "&lt;&lt;a&gt;b&gt;&gt; tail",
            "xref:t[]",
            "xref:t[Text]",
            "xref:t[window=_blank,role=x]",
            r"\xref:t[x]",
            r"xref:t[a\]b]",
            "xref:t[with [bracket]",
            "xref:\u{e9}l\u{e8}ve[\u{fc}ber]",
            "pre &lt;&lt;a&gt;&gt; mid xref:b[c] post \\xref:d[e]",
            "xref:a[]xref:b[B]&lt;&lt;c,d&gt;&gt;",
        ];

        for haystack in haystacks {
            let mut any = false;

            for caps in INLINE_XREF.captures_iter(haystack) {
                any = true;

                let whole = caps.get(0).unwrap();

                let expected = match (caps.get(2), caps.get(3), caps.get(4)) {
                    (Some(inner), None, None) => XrefGroups::Shorthand {
                        inner: inner.range(),
                    },
                    (None, Some(target), Some(text)) => XrefGroups::Macro {
                        target: target.range(),
                        text: text.range(),
                    },
                    other => panic!("unexpected group participation {other:?}"),
                };

                assert_eq!(
                    xref_groups(whole.as_str(), whole.start()),
                    expected,
                    "derivation diverged from the capture engine over \
                     {haystack:?} at {}",
                    whole.start(),
                );
            }

            assert!(any, "corpus haystack {haystack:?} produced no match");
        }
    }

    /// The frozen recording of `source`'s rendered output through the
    /// **whole** `Normal` group — the attributes step included — with any
    /// deferred cross-reference finalized to its unresolved fallback.
    ///
    /// [`golden_xref`] deliberately drives the macro-family steps only, which
    /// is right for a verbatim fixture and wrong for one whose target is
    /// *attribute-expanded*: it would leave `{cpp}` unexpanded on the golden
    /// side while the builder expands it, and report the difference as a
    /// divergence the fixture does not have.
    fn golden_whole_pipeline(source: &str) -> String {
        crate::content::inline_builder::snapshot::recorded("xref_whole_pipeline", source)
    }

    /// The frozen recording of `source`'s rendered output through the
    /// **macros** step, with any deferred cross-references finalized to their
    /// unresolved fallback. Unlike [`golden_macros`], the macros step
    /// defers a cross-reference to a placeholder rather than rendering it,
    /// so the placeholder must be finalized — no catalog resolution runs,
    /// so the result is the unresolved-fallback rendering the additive
    /// builder's fold (always unresolved) must reproduce.
    fn golden_xref_with(source: &str, _parser: &Parser) -> String {
        crate::content::inline_builder::snapshot::recorded("xref_macros", source)
    }

    /// [`golden_xref_with`] with a default parser.
    fn golden_xref(source: &str) -> String {
        golden_xref_with(source, &Parser::default())
    }

    /// Asserts that `node` is a cross-reference [`Ref`](InlineNode::Ref), and
    /// returns it.
    fn assert_xref<'a, 'src>(node: &'a InlineNode<'src>) -> &'a Ref<'src> {
        match node {
            InlineNode::Ref(reference) if reference.variant == RefVariant::Xref => reference,

            other => panic!("expected an xref Ref, got {other:?}"),
        }
    }

    #[test]
    fn fold_matches_the_string_pipeline_through_xrefs() {
        // For each fixture, folding the single-pass tree (all five steps)
        // reproduces the frozen recording byte-for-byte. This is the
        // differential corpus that pins cross-reference behavior. Every
        // fixture is a *verbatim* cross-reference in either
        // spelling, whether it resolves through the catalog (same-document) or
        // through a target-derived destination (inter-document, or the
        // document-as-a-whole form) — the boundary this family claims (an
        // attribute-list text, and a shorthand crossing a special/span, are
        // deferred and live in divergence tests below).
        let fixtures = [
            // No cross-reference despite macro-ish characters.
            "plain text without a reference",
            "an xref without a bracket xref:foo stays literal",
            // Macro form: bracketed reference text, and empty (bracketed
            // fallback).
            "xref:install[Installation]",
            "xref:install[]",
            "xref:sect-one[Section One]",
            // An explicit same-document reference (`#id`).
            "xref:#install[Install]",
            // An inter-document target — with and without a fragment, and a
            // non-AsciiDoc extension kept as-is — and the document-as-a-whole
            // form (an empty target).
            "xref:other.adoc#frag[Elsewhere]",
            "xref:other.adoc[]",
            "xref:refcard.pdf[Reference Card]",
            "xref:#[]",
            // An escaped `]` inside the text is unescaped.
            "xref:foo[a\\]b]",
            // A text carrying an attribute list (an `=`): the first positional
            // attribute is the display text, and `window`/`role`/`xrefstyle`
            // named attributes are honored.
            "xref:install[Installation,role=hl]",
            "xref:install[Installation,window=_blank]",
            "xref:install[Installation,role=hl,window=_blank,xrefstyle=full]",
            // An attribute list with no positional text at all: no display
            // text, only the named attributes.
            "xref:install[role=hl]",
            // An `=` that is not a real attribute list: no valid attribute
            // name precedes it, so the attrlist parse yields one positional
            // value spanning the whole text — the incidental case.
            "xref:install[=text]",
            // A macro embedded in surrounding flow, and next to other constructs.
            "See xref:install[the guide] for details.",
            "*bold* then xref:x[X] and _em_",
            "a copyright (C) then xref:x[X]",
            // Escapes: the macro stays literal, minus the backslash.
            "\\xref:install[Installation]",
            "\\xref:install[]",
            // A macro inside a rendered span (recognized inside the span body).
            "*see xref:x[X]*",
            "_xref:y[Y] in em_",
            // Shorthand form: bare id (bracketed fallback) and with reference
            // text, seen post-special-chars as `&lt;&lt;id&gt;&gt;`.
            "<<install>>",
            "<<install,Install Now>>",
            "<<sect-one,Section One>>",
            // The shorthand reads a dotted target as an id (unlike the macro).
            "<<a.b.c>>",
            // The id and reference text are each trimmed around the comma.
            "<< spaced , Trimmed Text >>",
            // A *present-but-empty* reference text: the comma makes the text
            // present, so this renders an empty `<a>…</a>` rather than the
            // bracketed `[id]` fallback a comma-less shorthand renders. A
            // whitespace-only text trims to the same thing.
            "<<install,>>",
            "<<install,   >>",
            // The same, with a target carrying its own derived destination
            // (the branch of `render_xref` an empty text reaches differently
            // from the unresolved one).
            "<<other#frag,>>",
            // An inter-document shorthand — with and without a fragment — and
            // the document-as-a-whole shorthand (an empty id).
            "<<other#frag,Elsewhere>>",
            "<<other#>>",
            "<<>>",
            // A shorthand embedded in surrounding flow, and next to other
            // constructs; and both spellings together.
            "See <<install>> now.",
            "*bold* then <<x,X>> and _em_",
            "<<install>> and xref:install[Installation]",
            // Escapes: the shorthand stays literal, minus the backslash.
            "\\<<install>>",
            "\\<<install,Install Now>>",
            // A shorthand inside a rendered span (recognized inside the body).
            "*see <<x>>*",
            "_<<y,Y>> in em_",
            // A reference text crossing an *escaped special*: the match string
            // carries the entity itself, so
            // both spellings are recognized, the text becoming structured
            // children (a `CharRef` between two `Text` runs) that fold back to
            // the same entity.
            "xref:foo[a<b]",
            "xref:install[Tom & Jerry]",
            "xref:install[1 < 2 > 0 & true]",
            "xref:install[a<b\\]c]",
            "<<foo,a<b>>",
            "<<install,Tom & Jerry>>",
            // A `\]` in a shorthand's text is *not* unescaped (only the macro
            // form's bracketed text is), with and without a crossed special.
            "<<foo,a\\]b>>",
            "<<foo,a<b\\]c>>",
            "<< spaced , Tom & Jerry >>",
            // An attribute-list text whose positional value crosses one: the
            // value is parsed off the already-escaped match string, so the node
            // holds the *logical* text and the fold re-escapes it.
            "xref:install[Tom & Jerry,role=hl]",
            "xref:install[a<b,window=_blank]",
            "xref:install[a > b,role=hl]",
            // A *target* crossing one. The macro form reads it the same way
            // Asciidoctor does (an id of `foo&amp;bar`); the shorthand
            // form's own `id.contains('<')` guard never fires, since an escaped
            // special is an entity by macro time.
            "xref:foo&bar[Ampersand]",
            "<<foo&bar,Ampersand>>",
            // Escaped, crossing one: the backslash is dropped and the rest
            // stays literal.
            "\\xref:install[Tom & Jerry]",
            "\\<<install,Tom & Jerry>>",
            // Crossing one *inside* a rendered span, and beside other
            // constructs.
            "*see xref:x[a & b]*",
            "a copyright (C) then <<x,a & b>>",
        ];

        let renderer = HtmlInlineRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_src(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden_xref(fixture),
                "fold diverged from the frozen recording for {fixture:?}"
            );
        }
    }

    #[test]
    fn an_xref_macro_becomes_a_ref_node() {
        let nodes = build_src(Span::new("xref:install[Installation]"));

        assert_eq!(nodes.len(), 1);
        let reference = assert_xref(&nodes[0]);

        assert_eq!(reference.target.as_ref(), "install");
        assert_eq!(link_text_of(reference), "Installation");
        assert!(reference.roles.is_empty());
        assert_eq!(reference.window, None);
        assert_eq!(reference.resolved, None);

        // Its location covers the whole macro, the `[…]` included.
        assert_eq!(reference.location.data(), "xref:install[Installation]");
        assert_eq!(reference.location.line(), 1);
        assert_eq!(reference.location.col(), 1);
    }

    #[test]
    fn an_empty_xref_macro_has_no_children() {
        // An empty text yields no children; the fold reads that as "no text
        // provided" and renders the bracketed `[id]` fallback.
        let nodes = build_src(Span::new("xref:install[]"));

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "install");
        assert!(reference.children.is_empty());

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_xref("xref:install[]")
        );
    }

    #[test]
    fn an_xref_display_text_is_located_at_its_source() {
        // The display text's `Text` child locates at the bracketed text, not
        // the whole macro.
        let nodes = build_src(Span::new("xref:install[Installation]"));

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.children.len(), 1);
        assert_text(&reference.children[0], "Installation", 1, 14);
    }

    #[test]
    fn an_explicit_same_document_xref_stores_the_interpreted_id() {
        // `xref:#install[]` uses the explicit-`#` same-document form. The
        // node's `target` is the *interpreted* id (`install`), not the
        // raw `#install`: it is the value the renderer builds the
        // `href` from and resolution keys on (see the `Ref::target` field
        // docs). Storing `#install` would fold to `href="##install"`
        // instead.
        let nodes = build_src(Span::new("xref:#install[Install]"));

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "install");
        assert_eq!(link_text_of(reference), "Install");

        // The fold produces `href="#install"` exactly.
        let folded = fold_html(&nodes, &HtmlInlineRenderer {});
        assert!(folded.contains(r##"href="#install""##), "folded: {folded}");
        assert_eq!(folded, golden_xref("xref:#install[Install]"));
    }

    #[test]
    fn an_xref_is_recognized_inside_a_span() {
        // A cross-reference can appear inside a rendered span; the transducer
        // descends into the span body and builds the node there.
        let nodes = build_src(Span::new("*see xref:x[X]*"));

        let children = assert_styled(&nodes[0], StyleVariant::Strong, SpanForm::Constrained);
        assert_eq!(children.len(), 2);
        assert_text(&children[0], "see ", 1, 2);

        let reference = assert_xref(&children[1]);
        assert_eq!(reference.target.as_ref(), "x");
        assert_eq!(link_text_of(reference), "X");
    }

    #[test]
    fn an_escaped_xref_stays_literal() {
        // `\xref:…` drops the backslash and keeps the macro as literal text —
        // no reference node.
        let source = "\\xref:install[Installation]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an escaped xref must not produce a reference node: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_xref(source)
        );
    }

    #[test]
    fn an_escaped_xref_the_gate_rejects_still_drops_its_backslash() {
        // The escape check runs *ahead* of the gate: a macro the gate rejects
        // (here, an *attribute-list* display text crossing a rendered span,
        // the one text shape that still needs its own bytes) still drops its
        // backslash and keeps the rest — the rendered span included — as its
        // own nodes, which fold to exactly what `caps[0][1..]` emits. Before
        // this order, such a match was left unrecognized, backslash and all.
        let source = "\\xref:sec[*bold*,role=hl]";
        let nodes = build_src(Span::new(source));

        let folded = fold_html(&nodes, &HtmlInlineRenderer {});
        assert!(!folded.starts_with('\\'), "folded: {folded}");
        assert_eq!(folded, golden_xref(source));
    }

    #[test]
    fn an_xref_shorthand_becomes_a_ref_node() {
        // The `<<id,text>>` shorthand builds the same `Ref{Xref}` node the
        // `xref:` macro does, even though its `&lt;&lt;` / `&gt;&gt;`
        // delimiters are `CharRef`s: the node consumes them and slices
        // its verbatim inner.
        let nodes = build_src(Span::new("<<install,Install Now>>"));

        assert_eq!(nodes.len(), 1);
        let reference = assert_xref(&nodes[0]);

        assert_eq!(reference.target.as_ref(), "install");
        assert_eq!(link_text_of(reference), "Install Now");
        assert!(reference.roles.is_empty());
        assert_eq!(reference.window, None);
        assert_eq!(reference.resolved, None);

        // Its location covers the whole shorthand, the `<<` / `>>` included.
        assert_eq!(reference.location.data(), "<<install,Install Now>>");
        assert_eq!(reference.location.line(), 1);
        assert_eq!(reference.location.col(), 1);
    }

    #[test]
    fn a_bare_xref_shorthand_has_no_children() {
        // A shorthand without a `, reference text` yields no children; the fold
        // reads that as "no text provided" and renders the bracketed `[id]`
        // fallback, exactly as the empty `xref:id[]` macro does.
        let nodes = build_src(Span::new("<<install>>"));

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "install");
        assert!(reference.children.is_empty());

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_xref("<<install>>")
        );
    }

    #[test]
    fn an_xref_shorthand_display_text_is_located_at_its_trimmed_source() {
        // The reference text's `Text` child locates at the *trimmed* text
        // within the shorthand, not at the whole shorthand and not
        // including the surrounding whitespace that gets trimmed.
        let nodes = build_src(Span::new("<<install, Install Now >>"));

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "install");
        assert_eq!(reference.children.len(), 1);

        // `<<install, ` is 11 characters, so the text starts at column 12.
        assert_text(&reference.children[0], "Install Now", 1, 12);
    }

    #[test]
    fn an_xref_shorthand_is_recognized_inside_a_span() {
        // A shorthand can appear inside a rendered span; the transducer
        // descends into the span body and builds the node there.
        let nodes = build_src(Span::new("*see <<x,X>>*"));

        let children = assert_styled(&nodes[0], StyleVariant::Strong, SpanForm::Constrained);
        assert_eq!(children.len(), 2);
        assert_text(&children[0], "see ", 1, 2);

        let reference = assert_xref(&children[1]);
        assert_eq!(reference.target.as_ref(), "x");
        assert_eq!(link_text_of(reference), "X");
    }

    #[test]
    fn an_escaped_xref_shorthand_stays_literal() {
        // `\<<id>>` drops the backslash and keeps the shorthand as literal text
        // — no reference node. Its delimiters are non-verbatim `CharRef`s, so
        // this also exercises the escape path that does not require a
        // verbatim inner.
        let source = "\\<<install,Install Now>>";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an escaped shorthand must not produce a reference node: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_xref(source)
        );
    }

    #[test]
    fn an_inter_document_xref_shorthand_becomes_a_ref_node() {
        // An inter-document shorthand target (`other#frag`) carries a *derived*
        // destination computed from the target itself, exactly as the
        // inter-document `xref:` macro form does.
        let source = "<<other#frag,Elsewhere>>";
        let nodes = build_src(Span::new(source));

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "other#frag");
        assert_eq!(link_text_of(reference), "Elsewhere");
        assert_eq!(reference.resolved, None);

        #[allow(clippy::expect_used)]
        let derived = reference
            .derived
            .as_ref()
            .expect("an inter-document shorthand carries a derived destination");
        assert_eq!(derived.href, "other.html#frag");
        assert_eq!(derived.text, "other.html");

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_xref(source)
        );
    }

    #[test]
    fn an_empty_xref_shorthand_becomes_a_ref_node() {
        // `<<>>` names the document as a whole: an empty id that resolves
        // through a *derived* destination computed from the document's
        // own attributes, exactly as the empty `xref:#[]` macro form
        // does.
        let source = "<<>>";
        let nodes = build_src(Span::new(source));

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "");
        assert!(reference.children.is_empty());
        assert_eq!(reference.resolved, None);

        #[allow(clippy::expect_used)]
        let derived = reference
            .derived
            .as_ref()
            .expect("a document-as-a-whole shorthand carries a derived destination");
        assert_eq!(derived.href, "#");

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_xref(source)
        );
    }

    #[test]
    fn an_xref_shorthand_with_an_empty_text_keeps_it_present() {
        // `<<id,>>` records a *present-but-empty* reference text: it
        // renders an empty `<a href="#install"></a>`, not the
        // bracketed `[install]` fallback a comma-less shorthand renders. The
        // node keeps the distinction structurally — the text is present as one
        // empty `Text` child — so the fold reproduces the same bytes.
        let source = "<<install,>>";
        let nodes = build_src(Span::new(source));

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "install");
        assert_eq!(reference.children.len(), 1);
        assert_text(&reference.children[0], "", 1, 11);

        let golden = golden_xref(source);
        assert!(golden.contains(r##"href="#install">"##), "{golden}");
        assert!(!golden.contains("[install]"), "{golden}");
        assert_eq!(fold_html(&nodes, &HtmlInlineRenderer {}), golden);
    }

    #[test]
    fn an_xref_shorthand_without_a_comma_provides_no_text() {
        // The complement of the test above, and what makes the empty `Text`
        // child load-bearing rather than noise: with no comma there is no text
        // to provide, so the node carries *no* child and the fold renders the
        // bracketed fallback. Both shorthands fold to an `<a>` element; only
        // the presence of a child tells the two bodies apart.
        let source = "<<install>>";
        let nodes = build_src(Span::new(source));

        let reference = assert_xref(&nodes[0]);
        assert!(reference.children.is_empty());

        assert!(golden_xref(source).contains("[install]"));
        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_xref(source)
        );
    }

    #[test]
    fn a_real_documents_empty_shorthand_text_reaches_its_tree() {
        // End-to-end, through the real parse path, and with the reference
        // *resolved*: this is the shape that makes the form a blocker for the
        // fold rather than an unclaimed one — a golden test
        // already exercises it (`xref_should_use_title_of_target_as_link_text_
        // when_explicit_link_text_is_empty` in `tests/asciidoctor_rb/
        // links_test.rs`, part of the ported `asciidoctor` test suite).
        // Resolution reaches the node too: the positional mirror skips a list
        // whose node count diverges from the number of deferred segments the
        // tree produces, so leaving the shorthand unrecognized would cost the
        // whole content its resolved destinations.
        use crate::blocks::{FindBlocks, IsBlock};

        let doc = Parser::default().parse("<<tigers,>>\n\n[#tigers]\n== Tigers");

        let blocks: Vec<_> = doc.descendant_blocks().collect();
        let rendered = blocks[0].rendered_html_content().unwrap();
        let inlines = blocks[0].inlines().unwrap();

        // The empty explicit text falls back to the target's own reference
        // text, exactly as Asciidoctor's resolved branch does.
        assert_eq!(rendered, r##"<a href="#tigers">Tigers</a>"##);

        let reference = assert_xref(&inlines[0]);
        assert_eq!(reference.children.len(), 1);
        assert!(reference.resolved.is_some(), "{reference:?}");

        assert_eq!(
            super::super::super::fold_html(
                inlines,
                &HtmlInlineRenderer {},
                &Parser::default().render_context()
            ),
            rendered,
            "fold diverged from the rendered string for {inlines:?}"
        );
    }

    #[test]
    fn a_whitespace_only_xref_shorthand_text_trims_to_an_empty_present_text() {
        // The reference text is trimmed the same way Asciidoctor trims
        // it, so a whitespace-only text is the same present-but-empty text —
        // its zero-length span sitting where the trim left it, after the
        // leading whitespace.
        let source = "<<install,   >>";
        let nodes = build_src(Span::new(source));

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.children.len(), 1);
        assert_text(&reference.children[0], "", 1, 14);

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_xref(source)
        );
    }

    #[test]
    fn a_shorthand_reference_text_carries_a_rendered_span_as_its_own_child() {
        // A reference text crossing a rendered span is carried *structurally*:
        // the span is one opaque placeholder in the match string, but
        // `macro_text_children` recovers the text with `emit_range`, which
        // clones the span's own node whole into the reference's children. The
        // fold then re-renders exactly that markup.
        let source = "<<x,a *bold* b>>";
        let nodes = build_src(Span::new(source));

        let reference = assert_xref(&nodes[0]);

        // Three children: the text before the span, the span itself, the text
        // after it — each borrowing its own precise `'src` slice, which the
        // one-`Text`-child shape this replaced could not express.
        assert_eq!(reference.children.len(), 3);
        assert_text(&reference.children[0], "a ", 1, 5);

        let styled = assert_styled(
            &reference.children[1],
            StyleVariant::Strong,
            SpanForm::Constrained,
        );

        assert_text(&styled[0], "bold", 1, 8);
        assert_text(&reference.children[2], " b", 1, 13);

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_xref(source)
        );
    }

    #[test]
    fn an_inter_document_xref_becomes_a_ref_node() {
        // An inter-document target (`other.adoc#frag`) carries a *derived*
        // destination computed from the target itself — the AsciiDoc extension
        // stripped, the output suffix substituted in.
        let source = "xref:other.adoc#frag[Elsewhere]";
        let nodes = build_src(Span::new(source));

        let reference = assert_xref(&nodes[0]);

        // Unlike a same-document reference, the node's target is the raw target
        // as written, not an interpreted id (see the `Ref::target` field docs).
        assert_eq!(reference.target.as_ref(), "other.adoc#frag");
        assert_eq!(link_text_of(reference), "Elsewhere");
        assert_eq!(reference.resolved, None);

        #[allow(clippy::expect_used)]
        let derived = reference
            .derived
            .as_ref()
            .expect("an inter-document xref carries a derived destination");
        assert_eq!(derived.href, "other.html#frag");
        assert_eq!(derived.text, "other.html");

        let folded = fold_html(&nodes, &HtmlInlineRenderer {});
        assert!(
            folded.contains(r#"href="other.html#frag""#),
            "folded: {folded}"
        );
        assert_eq!(folded, golden_xref(source));
    }

    #[test]
    fn an_xref_text_over_a_special_character_becomes_structured_children() {
        // A cross-reference whose text contains `<` is matched over the
        // *escaped* text (`xref:foo[a&lt;b]`), which is
        // exactly what the level's match string carries too. The text becomes
        // structured children rather than one sliced `Text`, so the escaped
        // special stays the `CharRef` it already is — folding back to the same
        // entity, where one `Text` holding `&lt;` would be escaped twice.
        let source = "xref:foo[a<b]";
        let nodes = build_src(Span::new(source));

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "foo");
        assert_eq!(reference.children.len(), 3);

        // Each recovered piece keeps its own precise span (#944).
        assert_text(&reference.children[0], "a", 1, 10);

        match &reference.children[1] {
            InlineNode::CharRef {
                value: CharRef::Special(ch),
                location,
            } => {
                assert_eq!(*ch, '<');
                assert_eq!(location.data(), "<");
                assert_eq!(location.col(), 11);
            }

            other => panic!("expected an escaped special child, got {other:?}"),
        }

        assert_text(&reference.children[2], "b", 1, 12);

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_xref(source)
        );
    }

    #[test]
    fn an_xref_shorthand_text_over_a_special_character_becomes_structured_children() {
        // The shorthand's own version of the case directly above: the id is
        // read from the match string (where an escaped special is its entity,
        // so the `id.contains('<')` guard never fires there
        // either) and the trimmed reference text becomes structured children.
        let source = "<< spaced , Tom & Jerry >>";
        let nodes = build_src(Span::new(source));

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "spaced");
        assert_eq!(reference.children.len(), 3);

        assert_text(&reference.children[0], "Tom ", 1, 13);
        assert!(matches!(
            reference.children[1],
            InlineNode::CharRef {
                value: CharRef::Special('&'),
                ..
            }
        ));
        assert_text(&reference.children[2], " Jerry", 1, 18);

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_xref(source)
        );
    }

    #[test]
    fn an_xref_attribute_list_text_over_a_special_character_holds_logical_text() {
        // The attribute-list branch computes its display text by *parsing* the
        // already-escaped match-string text, so the positional value comes back
        // holding `&amp;`. A `Text` node holds logical text the fold escapes,
        // so the entity is put back to its character here and
        // re-escaped at fold time — one round trip, not two escapes.
        let source = "xref:install[Tom & Jerry,role=hl]";
        let nodes = build_src(Span::new(source));

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.children.len(), 1);
        assert_eq!(link_text_of(reference), "Tom & Jerry");
        assert_eq!(reference.roles.len(), 1);
        assert_eq!(reference.roles[0].as_ref(), "hl");

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_xref(source)
        );
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_a_cross_reference_crossing_a_restored_entity() {
        // A restored entity (`&copy;`, `&#8217;`) is admitted for the same
        // reason an escaped special is: the level's match string carries its
        // own bytes — the fully-resolved bytes from the character-replacements
        // step onward — and the fold emits them verbatim.
        let fixtures = [
            // A reference text crossing one, in both spellings.
            "xref:sec[Tom &copy; Jerry]",
            "<<sec,Tom &copy; Jerry>>",
            "xref:sec[&copy;]",
            "xref:sec[&#8217;t is]",
            // A *target* crossing one, in both spellings.
            "xref:s&copy;c[Text]",
            "<<s&copy;c>>",
            // An attribute-list text crossing one — the capture this family
            // parses from a normalized copy rather than an `'src` slice, so
            // (unlike the link and image families) it takes the lift too.
            "xref:sec[Tom &copy; Jerry,role=hl]",
            "xref:sec[&copy;&reg;,role=hl,window=_blank]",
            // A text crossing both a restored entity and an escaped special.
            "xref:sec[a &copy; b < c]",
            "xref:sec[a &copy; b < c,role=hl]",
            // A *doubly* escaped entity: `&amp;copy;` is a literal `&`
            // followed by the letters `copy;`, not an entity, and the fold
            // must unwind exactly one level.
            "xref:sec[Tom &amp;copy; Jerry]",
            "xref:sec[Tom &amp;copy; Jerry,role=hl]",
            // In flow, inside a rendered span, doubled, and escaped.
            "see xref:sec[Tom &copy; Jerry] now",
            "*xref:sec[Tom &copy; Jerry]*",
            "xref:a[&copy;] and xref:b[&reg;]",
            "\\xref:sec[Tom &copy; Jerry]",
        ];

        let renderer = HtmlInlineRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_src(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden_xref(fixture),
                "fold diverged from the frozen recording for {fixture:?}"
            );
        }
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_a_cross_reference_crossing_a_character_replacement() {
        // A typographic replacement is the third recoverable piece, admitted
        // for the same reason the two entity leaves are: the level's match
        // string carries the entity the built-in backend renders it as — the
        // same fully-resolved bytes from the character-replacements step
        // onward — and the fold routes the leaf back through the renderer to
        // those same bytes.
        let fixtures = [
            // A reference text crossing one, in both spellings.
            "xref:sec[Tom (C) Jerry]",
            "<<sec,Tom (C) Jerry>>",
            "xref:sec[O'Reilly]",
            "xref:sec[Wait...]",
            // A *target* crossing one, in both spellings — the second the
            // shape a real fixture in this crate's own corpora writes.
            "xref:s(C)c[Text]",
            "<<s(C)c>>",
            "<<Cub => Tiger>>",
            // An attribute-list text crossing one.
            "xref:sec[Tom (C) Jerry,role=hl]",
            "xref:sec[O'Reilly,role=hl,window=_blank]",
            // A text crossing a replacement, an escaped special, and a
            // restored entity at once.
            "xref:sec[a (C) b < c &copy; d]",
            "xref:sec[a (C) b < c &copy; d,role=hl]",
            // In flow, inside a rendered span, doubled, and escaped.
            "see xref:sec[Tom (C) Jerry] now",
            "*xref:sec[Tom (C) Jerry]*",
            "xref:a[(C)] and xref:b[(R)]",
            "\\xref:sec[Tom (C) Jerry]",
        ];

        let renderer = HtmlInlineRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_src(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden_xref(fixture),
                "fold diverged from the frozen recording for {fixture:?}"
            );
        }
    }

    #[test]
    fn a_reference_text_crossing_a_restored_entity_keeps_the_entity_as_its_own_child() {
        // The plain-text path rebuilds the text through `emit_range`, so the
        // entity stays the leaf it already is rather than being baked into a
        // `Text` the fold would escape a second time.
        let source = "xref:sec[Tom &copy; Jerry]";
        let nodes = build_src(Span::new(source));

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.children.len(), 3);
        assert_text(&reference.children[0], "Tom ", 1, 10);

        // The leaf's own span is precise — the entity as the author wrote it,
        // which is also the value it carries (the `SpecialCharacters` escape
        // the replacements step undid leaves no trace in either).
        let entity = assert_entity(&reference.children[1], "&copy;");
        assert_eq!(entity.data(), "&copy;");
        assert_eq!(entity.col(), 14);

        assert_text(&reference.children[2], " Jerry", 1, 20);
    }

    #[test]
    fn an_attribute_list_text_crossing_a_restored_entity_splits_the_entity_out() {
        // The attribute-list branch has no range to rebuild from — its value
        // comes back from an `Attrlist` parse of a normalized *copy* — so
        // `escaped_value_children` re-derives the same split from the value's
        // own bytes: the escaped special becomes the character a `Text` holds
        // logically, and the restored entity its own `CharRef` leaf. Both fold
        // back to one escape level, as Asciidoctor's own text does.
        let source = "xref:sec[Tom &copy; & Jerry,role=hl]";
        let nodes = build_src(Span::new(source));

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.roles.len(), 1);
        assert_eq!(reference.children.len(), 3);

        // Every part of a parsed positional value shares the bracketed text's
        // own coarse span — it has no `'src` slice of its own.
        let text_span = reference.children[0].span();
        assert_eq!(text_span.data(), "Tom &copy; & Jerry,role=hl");

        match &reference.children[0] {
            InlineNode::Text { value, location } => {
                assert_eq!(value.as_ref(), "Tom ");
                assert_eq!(*location, text_span);
            }

            other => panic!("expected a Text run, got {other:?}"),
        }

        assert_eq!(assert_entity(&reference.children[1], "&copy;"), text_span);

        match &reference.children[2] {
            InlineNode::Text { value, location } => {
                // The escaped special comes back as the *character*, which the
                // fold escapes once.
                assert_eq!(value.as_ref(), " & Jerry");
                assert_eq!(*location, text_span);
            }

            other => panic!("expected a Text run, got {other:?}"),
        }

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_xref(source)
        );
    }

    #[test]
    fn a_doubly_escaped_entity_in_an_attribute_list_text_unwinds_one_level() {
        // `&amp;copy;` in the source is a literal `&` followed by the letters
        // `copy;`, which the match string carries as `&amp;copy;` too (the
        // `SpecialCharacters` escape of the `&`, which the restore-entities
        // rule declines because `amp;copy` is not an entity name). Scanning
        // left to right consumes the `&amp;` first, so the value is one `Text`
        // holding `&copy;` *logically* — which the fold escapes back to
        // `&amp;copy;` — not an entity leaf that would emit `&copy;`.
        let source = "xref:sec[Tom &amp;copy; Jerry,role=hl]";
        let nodes = build_src(Span::new(source));

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.children.len(), 1);
        assert_eq!(link_text_of(reference), "Tom &copy; Jerry");

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_xref(source)
        );
    }

    #[test]
    fn an_escaped_bracket_survives_a_structured_xref_text() {
        // The macro form's own `raw_text.replace("\\]", "]")` unescape, applied
        // to a text that *also* crosses an escaped special: the backslash is a
        // gap between two emitted ranges, so the `]` after it starts a fresh —
        // still `'src`-borrowing — run rather than being rebuilt into an owned
        // value.
        let source = "xref:foo[a<b\\]c]";
        let nodes = build_src(Span::new(source));

        let reference = assert_xref(&nodes[0]);

        // `link_text_of` reads only the `Text` children, so the unescaped
        // bracket shows up there while the special rides on its own `CharRef`.
        assert_eq!(reference.children.len(), 4);
        assert_eq!(link_text_of(reference), "ab]c");
        assert_text(&reference.children[3], "]c", 1, 14);

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_xref(source)
        );
    }

    #[test]
    fn an_escaped_bracket_stays_literal_in_a_shorthand_text() {
        // The shorthand has no bracket to escape, and `InlineXrefReplacer`'s
        // own shorthand branch performs no `\]` replace — so unlike the macro
        // form, a `\]` written in a shorthand's reference text stays literal in
        // both pipelines. Pinned in both the plain and the structured
        // (special-crossing) shapes, since the two take different paths.
        for source in ["<<foo,a\\]b>>", "<<foo,a<b\\]c>>"] {
            let nodes = build_src(Span::new(source));

            let reference = assert_xref(&nodes[0]);
            assert!(
                link_text_of(reference).contains("\\]"),
                "the shorthand must keep its backslash: {reference:?}"
            );

            assert_eq!(
                fold_html(&nodes, &HtmlInlineRenderer {}),
                golden_xref(source),
                "fold diverged for {source:?}"
            );
        }
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_a_text_crossing_a_rendered_span() {
        // The differential corpus for cross-reference text: a display or
        // reference
        // text crossing an **opaque** piece — a rendered span, an
        // already-recognized macro node, a masked passthrough — in both
        // spellings. The text is carried structurally (each opaque piece's own
        // node becomes a child), so the fold re-renders exactly that markup.
        let fixtures = [
            // (A masked passthrough is opaque here too, and restored only
            // after every step, but
            // this oracle runs the steps directly, without the extraction the
            // real `SubstitutionGroup::apply` performs around them, so those
            // fixtures live in the whole-pipeline sweep instead; see
            // `inline_builder::tests`.)
            //
            // The macro form: a span at the end, in the middle, at the start,
            // and spanning the whole text.
            "xref:sec[with *bold* reftext]",
            "xref:sec[*bold* leads]",
            "xref:sec[*bold*]",
            "xref:sec[_em_ and `code` and #mark#]",
            // Every quoted form the earlier step can have produced, including
            // an attributed span (whose markup carries an `=` the string
            // replacer's own attribute-list probe reads, and this one does not:
            // with no comma to split on, the parse yields one positional value
            // equal to the whole text, so both take the plain-text path).
            "xref:sec[[.hl]#roled#]",
            "xref:sec[super^script^ and sub~script~]",
            // An already-recognized macro node of another family: an image, a
            // link, an anchor, an index term.
            "xref:sec[the image:logo.png[Logo] here]",
            "xref:sec[a link:https://example.org[site] inside]",
            "xref:sec[an ((index term)) inside]",
            // A span *and* an escaped special / restored entity in one text —
            // the recoverable and structural recoveries side by side.
            "xref:sec[a < b and *bold*]",
            "xref:sec[a &copy; b and *bold*]",
            // Escaped: the backslash is dropped and the span stays in the flow.
            "\\xref:sec[with *bold* reftext]",
            // In surrounding flow, and inside a rendered span of its own.
            "See xref:sec[the *bold* one] for details.",
            "*see xref:sec[a _b_ c]*",
            // The shorthand: the same shapes after the comma.
            "<<x,*bold*>>",
            "<<x,a *bold* b>>",
            "<<x,_em_ then `code`>>",
            "<<x,[.hl]#roled#>>",
            "<<x,the image:logo.png[Logo] here>>",
            "<<x, *trimmed* >>",
            "<<x,a < b and *bold*>>",
            "\\<<x,*bold*>>",
            "a copyright (C) then <<x,*bold*>>",
        ];

        let renderer = HtmlInlineRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_src(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden_xref(fixture),
                "fold diverged from the frozen recording for {fixture:?}"
            );
        }
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_an_attribute_list_text_enclosing_a_span() {
        // The text shape that used to keep the stricter gate. A text carrying
        // an `=` is read as an attribute list, and its display text comes back
        // from that *parse* rather than from a range — but the value that
        // parse hands back is the node's **children**, so an enclosed
        // construct needs no bytes: it is tokened before the split
        // ([`tokened_text`]) and spliced back as the node itself
        // ([`restored_value_children`]).
        for fixture in [
            // The golden spelling, alone and in flow.
            "xref:sec[*bold*,role=hl]",
            "See xref:sec[*bold*,role=hl].",
            // The span at either edge and in the middle of the text.
            "xref:sec[*a* b,role=hl]",
            "xref:sec[a *b*,role=hl]",
            "xref:sec[a *b* c,role=hl]",
            // Each of the three named attributes this family reads, with the
            // span in the *positional* value beside it.
            "xref:sec[*a* b,window=_blank]",
            "xref:sec[*a* b,xrefstyle=full]",
            "xref:sec[*a* b,role=hl,window=_blank]",
            // Other span kinds, and two spans in one text.
            "xref:sec[`code` here,role=hl]",
            "xref:sec[[.r]#x# here,role=hl]",
            "xref:sec[*a* and _b_,role=hl]",
            // Beside the recoverable pieces the text already carried: an
            // escaped special, a restored entity, a typographic replacement.
            "xref:sec[a < *b*,role=hl]",
            "xref:sec[a &copy; *b*,role=hl]",
            "xref:sec[a (C) *b*,role=hl]",
            // A newline the parse's own normalization collapses, on either
            // side of the span.
            "xref:sec[*a*\nb,role=hl]",
            // An `=` the parse finds **incidental** — no attribute name can
            // hold the spaces before it, so the whole text stays the sole
            // positional value and the builder falls through to its plain-text
            // path, which has carried an opaque piece structurally all along.
            "xref:sec[a *b* c=d]",
            "xref:sec[*b* c=d]",
            // (A **masked** construct in the same text is exercised against
            // the *whole* pipeline instead — `golden_xref` deliberately skips
            // the passthrough-extraction pass, so the two sides would read
            // different text here. See
            // `fold_matches_the_real_pipeline_for_a_masked_construct_in_an_attribute_list_text`.)
            //
            // The shorthand spelling is unchanged: its reference text never
            // carried an attribute list at all.
            "<<sec,*bold*>>",
        ] {
            assert_eq!(
                fold_html(&build_src(Span::new(fixture)), &HtmlInlineRenderer {}),
                golden_xref(fixture),
                "fold diverged from the frozen recording for {fixture:?}"
            );
        }
    }

    #[test]
    fn an_attribute_list_text_enclosing_a_span_carries_it_as_children() {
        // The shape behind the parity above: the enclosed span itself is a
        // child, not the markup it will fold to, and the named attributes the
        // parse split off still reach the node's own plain fields.
        let nodes = build_src(Span::new("xref:sec[*bold* here,role=hl]"));

        assert_eq!(nodes.len(), 1);
        let reference = assert_xref(&nodes[0]);

        assert_eq!(reference.target.as_ref(), "sec");
        assert_eq!(reference.roles.len(), 1);
        assert_eq!(reference.roles[0].as_ref(), "hl");

        match &reference.children[..] {
            [InlineNode::Styled(styled), InlineNode::Text { value, .. }] => {
                assert_eq!(styled.location.data(), "*bold*");

                // The bytes around the token take the same rebuild every
                // attribute-list value takes: an owned run off the parse,
                // whose location is the bracketed text's own coarse span.
                assert_eq!(value.as_ref(), " here");
            }

            other => panic!("expected a span and a text run, got {other:?}"),
        }
    }

    #[test]
    fn an_attribute_list_delimiter_inside_a_span_is_the_trees_to_read() {
        // The deferral divergence, decided in favor of
        // the tree.
        //
        // A token carries none of the `,` / `=` / `"` a bracket split reads, so
        // the tree's split sees `a ␖ d,role=hl` — a display text and a role.
        // Splitting over the piece's own rendered **markup** instead gives a
        // different answer: `a *b, c*
        // d` renders `a <strong>b, c</strong> d`, whose list splits at the
        // comma *inside the tag*, ending the anchor at `a <strong>b` and
        // leaving it unbalanced. Asciidoctor does the same.
        //
        // This used to defer where the two readings disagreed, which made the
        // presence of a comma inside a span decide whether the macro was
        // recognized **at all** — the fixtures below came out as literal text.
        // Splitting over rendered markup is the wrong answer, and reproducing
        // it was never on the table, so the tree's reading stands and this
        // crate diverges from Asciidoctor here.
        for (source, expected) in [
            (
                "xref:sec[a *b, c* d,role=hl]",
                "<a href=\"#sec\" class=\"hl\">a <strong>b, c</strong> d</a>",
            ),
            (
                "xref:sec[a `b, c` d,role=hl]",
                "<a href=\"#sec\" class=\"hl\">a <code>b, c</code> d</a>",
            ),
        ] {
            let nodes = build_src(Span::new(source));

            let Some(InlineNode::Ref(ref_)) = nodes.first() else {
                panic!("the tree must now recognize the macro: {nodes:?}");
            };

            // The role landed as a role rather than being swallowed into the
            // display text, which is the half a split over rendered markup
            // would lose.
            assert_eq!(
                ref_.roles.iter().map(|r| r.as_ref()).collect::<Vec<_>>(),
                ["hl"],
                "for {source:?}"
            );

            // And the span survives whole inside the display text.
            assert!(
                ref_.children
                    .iter()
                    .any(|child| matches!(child, InlineNode::Styled(_))),
                "the display text must keep its span: {:?}",
                ref_.children
            );

            assert_eq!(
                fold_html(&nodes, &HtmlInlineRenderer {}),
                expected,
                "for {source:?}"
            );

            // The divergence, stated as bytes: the frozen recording cuts the
            // anchor short inside the tag it just wrote.
            assert_ne!(golden_xref(source), expected, "for {source:?}");
        }

        // The boundary of the class, unchanged: without a *named* attribute to
        // split off there is no attribute list at all, so the same comma is at
        // parity through the plain-text path, and a span whose markup carries
        // no delimiter was never affected (`[.r]#x#` renders a `"` and an `=`
        // the split reads harmlessly).
        for source in ["xref:sec[a *b, c* d]", "xref:sec[[.r]#x# here,role=hl]"] {
            assert_eq!(
                fold_html(&build_src(Span::new(source)), &HtmlInlineRenderer {}),
                golden_xref(source),
                "fold diverged from the frozen recording for {source:?}"
            );
        }
    }
    #[test]
    fn an_author_written_token_byte_defers_the_match() {
        // The bytes a token is built from are `\u{96}` and `\u{97}`, and an
        // **author** can write them: `build_match_string` stands an opaque
        // piece in as a private-use codepoint, so every one reaching the gate
        // is the author's own. They make the tokening ambiguous exactly where
        // this increment reads a value as a *string* — the search for a token
        // cannot tell the author's bytes from the pass's own, and would splice
        // a node's source into the author's text while leaving the real token
        // standing. Such a text defers, which is what the per-slot check this
        // increment replaced did for the same bytes.
        for source in [
            "xref:sec[*b*,role=\u{96}0\u{97}hl]",
            "xref:sec[*b*,role=hl\u{96}0\u{97}]",
            "xref:sec[+++p+++,role=\u{96}0\u{97}hl]",
        ] {
            let nodes = build_src(Span::new(source));

            assert!(
                nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
                "an author-written token byte must defer the match: {nodes:?}"
            );

            // The frozen recording builds one, keeping the author's bytes:
            // passthrough restoration ran over the *finished* string, which
            // never rewrote a `role=` it did not extract into.
            assert!(golden_whole_pipeline(source).contains("<a href"));
        }

        // A text with **no** opaque piece never reaches that gate, and needs
        // not to: there is no token to confuse, so the author's bytes pass
        // through to the slot unchanged.
        let source = "xref:sec[a,role=\u{96}0\u{97}hl]";
        assert_eq!(
            fold_html(&build_src(Span::new(source)), &HtmlInlineRenderer {}),
            golden_whole_pipeline(source)
        );
    }

    #[test]
    fn a_computed_attribute_read_as_a_string_takes_the_untranslated_source() {
        // The boundary this increment moved, drawn per **slot**: a `window=`,
        // a `role=`, or an `xrefstyle=` is read as a **string**, and an
        // enclosed span has no bytes to be read as — its markup exists only at
        // fold time. That used to leave the whole reference unrecognized.
        // Now the slot takes the *source* the author wrote for the piece,
        // which is a value a string can hold, and the reference is recognized.
        let nodes = build_src(Span::new("xref:sec[*a*,role=*b*]"));
        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.roles.len(), 1);
        assert_eq!(reference.roles[0].as_ref(), "*b*");

        let nodes = build_src(Span::new("xref:sec[a,window=*b*]"));
        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.window.as_deref(), Some("*b*"));

        // A masked passthrough contributes its **body**, not its source span:
        // the `+++` delimiters are syntax saying *do not substitute this*, so
        // the body is exactly the literal text asked for — a value that used
        // to reach this slot only as its passthrough-placeholder token, never
        // as `full` itself, so it could never select a style.
        let nodes = build_src(Span::new("xref:sec[a,xrefstyle=+++full+++]"));
        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.xrefstyle, Some(XrefStyle::Full));

        // An attribute reference is untouched by either rule — it is resolved
        // before this value is read, and only markup the *Quotes* step made is
        // unwound.
        let mut parser = Parser::default();
        parser = parser.with_intrinsic_attribute(
            "rn",
            "myrole",
            crate::parser::ModificationContext::Anywhere,
        );

        let nodes = build(Span::new("xref:sec[a,role={rn}]"), &parser, None);

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.roles.len(), 1);
        assert_eq!(reference.roles[0].as_ref(), "myrole");
    }

    #[test]
    fn a_masked_piece_in_a_preceding_named_attribute_does_not_misattribute_a_later_one() {
        // `role=` and `window=` are two different named attributes, tokened
        // by the same shared `carried` sequence. Asciidoctor's own attribute
        // numbering (every comma-delimited entry consumes a position,
        // including named ones) means the *display text* can only ever be
        // the bracket's first entry — so it is never actually reachable
        // after a preceding named attribute, and cannot exercise this —
        // but two named attributes, in either order, both carrying their own
        // masked construct, are exactly the shape
        // `Attrlist::named_attribute_token_offset` exists to keep straight:
        // without it, `window`'s own restore would read off a `carried`
        // slice that starts too early, splicing `role`'s body into `window`
        // and leaving `window`'s own body stranded (or the other way
        // around, in the reverse order).
        for (source, roles, window) in [
            ("xref:sec[role=++r++,window=++w++]", ["r"], "w"),
            ("xref:sec[window=++w++,role=++r++]", ["r"], "w"),
        ] {
            let nodes = build_src(Span::new(source));
            let reference = assert_xref(&nodes[0]);

            assert_eq!(
                reference
                    .roles
                    .iter()
                    .map(|r| r.as_ref())
                    .collect::<Vec<_>>(),
                roles,
                "for {source:?}"
            );
            assert_eq!(reference.window.as_deref(), Some(window), "for {source:?}");
        }
    }

    #[test]
    fn two_masked_roles_in_the_same_attribute_do_not_misattribute_each_other() {
        // `role=++a++ ++b++` is one attribute, not two — `roles()` (and
        // `roles_with_token_offset`) split its value on the space into two
        // roles, each carrying its own placeholder. Both start from the same
        // attribute-level offset, so the second role's own restore has to
        // additionally skip past the first role's own occurrence rather than
        // reusing that shared starting point (Greptile
        // https://github.com/asciidoc-rs/asciidoc-parser/pull/1349#discussion_r3890749214) —
        // otherwise both roles would come back as `"a"`, and `"b"` would
        // never be reached.
        let nodes = build_src(Span::new("xref:sec[a,role=++a-role++ ++b-role++]"));
        let reference = assert_xref(&nodes[0]);

        assert_eq!(
            reference
                .roles
                .iter()
                .map(|r| r.as_ref())
                .collect::<Vec<_>>(),
            ["a-role", "b-role"]
        );
    }

    #[test]
    fn an_untranslated_string_attribute_is_escaped_by_the_renderer() {
        // What the slot holds is *text*, and the renderer escapes it for the
        // attribute it is building — so a body carrying a `"` or an `&` lands
        // inert rather than breaking out of the tag. The frozen recording
        // cannot make this guarantee: a passthrough there is restored into
        // the rendered string only after every escape has run, so it never
        // reaches the value at all — leaking the sentinel that stood for it
        // instead.
        for (source, expected) in [
            (
                "xref:sec[a,role=+++x&y\"z+++]",
                "<a href=\"#sec\" class=\"x&amp;y&quot;z\">a</a>",
            ),
            (
                "xref:sec[a,window=+++_bl\"ank+++]",
                "<a href=\"#sec\" target=\"_bl&quot;ank\">a</a>",
            ),
        ] {
            assert_eq!(
                fold_html(&build_src(Span::new(source)), &HtmlInlineRenderer {}),
                expected,
                "the fold regressed for {source:?}"
            );

            assert!(
                golden_whole_pipeline(source).contains('\u{96}'),
                "the frozen recording is expected to leak its sentinel for {source:?}"
            );
        }

        // A value with nothing opaque in it is at parity, as it always was.
        let source = "xref:sec[a,role=hl]";
        assert_eq!(
            fold_html(&build_src(Span::new(source)), &HtmlInlineRenderer {}),
            golden_xref(source)
        );
    }

    #[test]
    fn a_span_whose_markup_perturbs_the_string_pipeline_is_a_documented_divergence() {
        // What the structural recovery cannot do is make the *recognition*
        // agree in every case: matching over the span's rendered markup
        // instead of the one placeholder standing in for it reads a different
        // extent whenever that markup carries a character the pattern is
        // sensitive to. These are the three shapes
        // where it does — and in each the well-formed reading is the tree's,
        // not the markup-perturbed one a match over raw text would give
        // (a truncated text, a text the attribute-list
        // parse cut in half) — exactly as
        // the quotes step's own crossed-delimiter divergence is.
        for source in [
            // A `]` inside the span would end the macro form's lazy text
            // capture early if matched over raw markup, but not here.
            "xref:sec[a *b ] c* d]",
            // A `>>` inside the span is the shorthand's own terminator, which
            // a match over raw markup would see as `&gt;&gt;`.
            "<<x,a *b >> c* d>>",
            // Markup carrying an `=` (an attributed span) *and* a comma
            // elsewhere in the text: matching over raw markup would have its
            // attribute-list probe fire on the markup's own `=`, and the
            // parse then split the text at that comma, keeping only what
            // precedes it.
            "xref:sec[one, [.hl]#two#]",
        ] {
            let nodes = build_src(Span::new(source));

            assert_ne!(
                fold_html(&nodes, &HtmlInlineRenderer {}),
                golden_xref(source),
                "{source:?} now agrees with the frozen recording; fold it into the parity corpus"
            );
        }
    }

    #[test]
    fn a_deferred_xref_target_over_a_passthrough_is_a_documented_divergence() {
        // The cross-reference family keeps the opaque-piece gate over a
        // masked passthrough — unlike the `link:`/`mailto:` family, which
        // restores one into its target — because a deferred cross-reference's
        // target used to be captured into the deferred segment while the
        // haystack still held the `\u{96}`*n*`\u{97}` passthrough sentinel,
        // before the restore pass — which rewrote only the rendered string —
        // could reach it, so the sentinel bytes leaked into the recorded
        // output's own `href` and fallback text. The tree defers
        // instead and folds the restored literal — the well-formed reading
        // against that recorded, leaked one.
        let source = "xref:++someid++[]";

        // Recorded, so the leaked bytes this divergence is *about* outlive the
        // retired mechanism that produced them (see [`snapshot`]).
        let golden = crate::content::inline_builder::snapshot::recorded(
            "xref_passthrough_divergence",
            source,
        );

        assert!(
            golden.contains('\u{96}'),
            "expected the frozen recording's sentinel leak to still reproduce: {golden:?}"
        );

        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an xref target over a passthrough must stay literal: {nodes:?}"
        );

        assert_eq!(fold_html(&nodes, &HtmlInlineRenderer {}), "xref:someid[]");
    }

    #[test]
    fn an_xref_attribute_list_text_populates_window_role_and_xrefstyle() {
        // An `xref:` text carrying an `=` splits into an attribute list: the
        // first positional attribute becomes the display text, and the
        // `window`/`role`/`xrefstyle` named attributes populate the node's own
        // fields, parsed from a newline-normalized copy of the text (mirroring
        // `InlineXrefReplacer`'s own attrlist parse).
        let source = "xref:install[Installation,role=hl,window=_blank,xrefstyle=full]";
        let nodes = build_src(Span::new(source));

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "install");
        assert_eq!(link_text_of(reference), "Installation");
        assert_eq!(
            reference
                .roles
                .iter()
                .map(|r| r.as_ref())
                .collect::<Vec<_>>(),
            vec!["hl"]
        );
        assert_eq!(reference.window.as_deref(), Some("_blank"));
        assert_eq!(reference.xrefstyle, Some(XrefStyle::Full));

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_xref(source)
        );
    }

    #[test]
    fn an_xref_attribute_list_with_no_positional_text_has_no_children() {
        // An attribute list with no positional value at all (only named
        // attributes) yields no display text — the same "no text provided"
        // fallback an empty `xref:id[]` uses — but still honors the named
        // attributes.
        let source = "xref:install[role=hl]";
        let nodes = build_src(Span::new(source));

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "install");
        assert!(reference.children.is_empty());
        assert_eq!(
            reference
                .roles
                .iter()
                .map(|r| r.as_ref())
                .collect::<Vec<_>>(),
            vec!["hl"]
        );

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_xref(source)
        );
    }

    #[test]
    fn an_xref_shorthand_has_no_style_of_its_own() {
        // The `<<id>>` shorthand has no attribute-list text to carry an
        // override (see the `Ref::xrefstyle` field docs), so its effective
        // style is whatever the document says — `None` under a parser with no
        // `xrefstyle` set.
        let nodes = build_src(Span::new("<<install,Install Now>>"));

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.xrefstyle, None);
    }

    /// A parser whose document-wide `xrefstyle` attribute is `value`.
    fn parser_with_xrefstyle(value: &str) -> Parser {
        Parser::default().with_intrinsic_attribute(
            "xrefstyle",
            value,
            crate::parser::ModificationContext::Anywhere,
        )
    }

    #[test]
    fn a_node_carries_the_effective_xrefstyle_not_the_override() {
        // `Ref::xrefstyle` is the style **in effect where the reference was
        // written**, resolved at build time. That is what makes the fold a pure
        // function of the tree, which in turn is what lets it run at
        // reference-resolution time — long after the parse, when the
        // document-wide `xrefstyle` may have been rebound by a later
        // `:xrefstyle:` line. See
        // `fold_xref_reads_the_effective_xrefstyle_off_the_node`, and the
        // whole-document fixtures in `inline_builder_document_parity`, which
        // fail without this.
        let parser = parser_with_xrefstyle("full");

        // Both spellings pick the document-wide style up.
        for source in ["<<install>>", "xref:install[]", "xref:install[Install Now]"] {
            let nodes = build(Span::new(source), &parser, None);
            let reference = assert_xref(&nodes[0]);

            assert_eq!(
                reference.xrefstyle,
                Some(XrefStyle::Full),
                "no document-wide style on the node for {source:?}"
            );
        }

        // A macro-level `xrefstyle=` still wins over it.
        let nodes = build(
            Span::new("xref:install[Install,xrefstyle=short]"),
            &parser,
            None,
        );
        assert_eq!(assert_xref(&nodes[0]).xrefstyle, Some(XrefStyle::Short));

        // And the *bare* `:xrefstyle:` spelling, which `document_xrefstyle`
        // reads as `Basic` rather than parsing a value.
        let set = Parser::default().with_intrinsic_attribute_bool(
            "xrefstyle",
            true,
            crate::parser::ModificationContext::Anywhere,
        );

        let nodes = build(Span::new("<<install>>"), &set, None);
        assert_eq!(assert_xref(&nodes[0]).xrefstyle, Some(XrefStyle::Basic));
    }

    #[test]
    fn fold_matches_the_string_pipeline_under_a_document_wide_xrefstyle() {
        // The differential corpus for the reading above: with the style
        // resolved into the node rather than read at fold time, the fold still
        // reproduces the frozen recording's bytes — `document_xrefstyle` is
        // called at build time, in the very same pass the recording reflects.
        //
        // These fold to the *unresolved* fallback (a bare `Content` has no
        // catalog), which is the shape both sides agree on here; the resolved
        // shape, where `xrefstyle` actually changes the bytes, is pinned over
        // whole documents in `inline_builder_document_parity`.
        let renderer = HtmlInlineRenderer {};

        for style in ["full", "short", "basic"] {
            let parser = parser_with_xrefstyle(style);

            for fixture in [
                "<<install>>",
                "<<install,Install Now>>",
                "xref:install[]",
                "xref:install[Install Now]",
                "xref:install[Install,xrefstyle=short]",
                "See <<install>> and xref:other.adoc#frag[] now.",
            ] {
                let folded = fold_html(&build(Span::new(fixture), &parser, None), &renderer);

                assert_eq!(
                    folded,
                    golden_xref_with(fixture, &parser),
                    "fold diverged from the frozen recording for {fixture:?} under xrefstyle={style:?}"
                );
            }
        }
    }

    #[test]
    fn an_incidental_equals_in_xref_text_is_not_an_attribute_list() {
        // `=text` contains an `=`, but no valid attribute name precedes it (an
        // attribute name cannot start with `=`), so the attrlist parse finds
        // one positional value spanning the *whole* text rather than a named
        // attribute — the `=` was incidental, mirroring
        // `InlineXrefReplacer`'s own `extract_attributes_from_text` fallback.
        // The text is then used as plain display text with no named
        // attributes, exactly as if it carried no `=` at all.
        let source = "xref:install[=text]";
        let nodes = build_src(Span::new(source));

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "install");
        assert_eq!(link_text_of(reference), "=text");
        assert!(reference.roles.is_empty());
        assert_eq!(reference.window, None);
        assert_eq!(reference.xrefstyle, None);

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_xref(source)
        );
    }

    #[test]
    fn an_empty_same_document_xref_becomes_a_ref_node() {
        // `xref:#[]` names the document as a whole: an empty same-document id
        // that resolves through a *derived* destination (`this_document_
        // reference`), computed from the document's own attributes without
        // consulting any catalog.
        let source = "xref:#[]";
        let nodes = build_src(Span::new(source));

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "");
        assert!(reference.children.is_empty());
        assert_eq!(reference.resolved, None);

        #[allow(clippy::expect_used)]
        let derived = reference
            .derived
            .as_ref()
            .expect("a document-as-a-whole xref carries a derived destination");
        assert_eq!(derived.href, "#");

        assert_eq!(
            fold_html(&nodes, &HtmlInlineRenderer {}),
            golden_xref(source)
        );
    }

    #[test]
    fn a_this_document_xref_target_is_treated_as_same_document() {
        // A target naming *this* document by its own `docname` (or a file
        // included into it in full) is a reference within it after all: the
        // element it names is in the catalog being built right now, so the node
        // carries the same-document target (the fragment) with no derived
        // destination — exactly as an explicit `#id` shorthand does.
        let parser = Parser::default().with_primary_file_name("mydoc.adoc");

        let source = "xref:mydoc.adoc#install[Install]";
        let root = Span::new(source);
        let nodes = super::super::super::build(root, &parser, None);

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "install");
        assert_eq!(reference.derived, None);

        let folded = super::super::super::fold_html(
            &nodes,
            &HtmlInlineRenderer {},
            &parser.render_context(),
        );
        assert!(folded.contains(r##"href="#install""##), "folded: {folded}");
        assert_eq!(folded, golden_xref_with(source, &parser));
    }

    #[test]
    fn a_fragmentless_this_document_xref_target_is_document_as_a_whole() {
        // A target naming *this* document with no fragment
        // (`xref:mydoc.adoc[]`) is, like the empty target (`xref:#[]`),
        // a reference to the document as a whole: the same
        // `this_document_reference` derived destination, not a
        // same-document id to resolve through the catalog.
        let parser = Parser::default().with_primary_file_name("mydoc.adoc");

        let source = "xref:mydoc.adoc[Home]";
        let root = Span::new(source);
        let nodes = super::super::super::build(root, &parser, None);

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "");

        #[allow(clippy::expect_used)]
        let derived = reference
            .derived
            .as_ref()
            .expect("a fragmentless self-reference carries a derived destination");
        assert_eq!(derived.href, "#");

        let folded = super::super::super::fold_html(
            &nodes,
            &HtmlInlineRenderer {},
            &parser.render_context(),
        );
        assert_eq!(folded, golden_xref_with(source, &parser));
    }

    /// A parser carrying the attributes the expanded-value fixtures below
    /// reference.
    fn expanding_parser() -> Parser {
        use crate::parser::ModificationContext;

        Parser::default()
            .with_intrinsic_attribute("id", "install", ModificationContext::Anywhere)
            .with_intrinsic_attribute("label", "Install Now", ModificationContext::Anywhere)
            .with_intrinsic_attribute("doc", "other.adoc", ModificationContext::Anywhere)
            .with_intrinsic_attribute(
                "xref-src",
                "<<install,Install>>",
                ModificationContext::Anywhere,
            )
            // A value *ending* in a backslash, so an expansion followed by a
            // literal `]` puts an escaped bracket astride two adjacent `Text`
            // runs (see `macro_text_children`'s own note).
            .with_intrinsic_attribute("trailing-backslash", "b\\", ModificationContext::Anywhere)
    }

    /// The real, public pipeline's output for `source` — the golden for the
    /// expanded-value fixtures, which need the `AttributeReferences` step
    /// [`golden_xref_with`] deliberately omits (it also finalizes the deferred
    /// cross-references, which this does through the group's own pipeline).
    fn golden_normal(source: &str, _parser: &Parser) -> String {
        crate::content::inline_builder::snapshot::recorded("xref_normal", source)
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_xrefs_inside_expanded_values() {
        // A cross-reference whose target or reference text crosses a
        // *synthesized* run (an attribute expansion) is recognized:
        // nothing on a `Ref{Xref}` node is `Span`-typed — its target and text
        // come from the match string, which carries a synthesized run's bytes
        // exactly — so only the node's `location` takes
        // the coarse fallback span used when a construct has no `Span`-typed
        // field of its own. This is the same lift the anchor,
        // bare-e-mail, UI, and index-term families already made.
        let parser = expanding_parser();

        let fixtures = [
            // The macro form: an expanded target, an expanded text, both.
            "xref:{id}[Install]",
            "xref:install[{label}]",
            "xref:{id}[{label}]",
            "xref:{id}[]",
            // An expanded inter-document target keeps its derived destination.
            "xref:{doc}#frag[Elsewhere]",
            // An expanded value inside a longer text, and beside literal text.
            "see xref:{id}[the {label} page] now",
            // The macro form's attribute-list text, expanded.
            "xref:{id}[{label}, window=_blank]",
            // The shorthand: an expanded id, an expanded reference text, both.
            "<<{id}>>",
            "<<install,{label}>>",
            "<<{id},{label}>>",
            // A present-but-empty reference text survives an expanded id.
            "<<{id},>>",
            // The whole cross-reference arriving from an expanded value. The
            // shorthand's `<<` is *literal* in the expanded value (it never
            // passes through `specialcharacters`), so neither pipeline
            // recognizes it as a shorthand — the tree and the string agree
            // that it stays literal.
            "{xref-src}",
            "before {xref-src} after",
            // A cross-reference inside a rendered span, itself carrying an
            // expansion.
            "*xref:{id}[{label}]*",
            // An escaped bracket *astride two adjacent runs* — the expansion's
            // trailing backslash and the literal `]` after it — in a text that
            // also crosses an escaped special, so the structured-children path
            // runs. The unescape must skip the backslash across the run
            // boundary, which is why it is applied to the emitted ranges rather
            // than to each recovered run.
            "xref:foo[a<{trailing-backslash}]x]",
            "<<foo,a<{trailing-backslash}]x>>",
        ];

        for source in fixtures {
            let nodes = super::super::super::build(Span::new(source), &parser, None);

            assert_eq!(
                super::super::super::fold_html(
                    &nodes,
                    &HtmlInlineRenderer {},
                    &parser.render_context()
                ),
                golden_normal(source, &parser),
                "fold diverged from the frozen recording for {source:?}"
            );
        }
    }

    #[test]
    fn an_xref_inside_an_expanded_value_keeps_a_coarse_location() {
        // The values are exact; only the node's `location` (and its children's)
        // falls back to the enclosing synthesized run's coarse span,
        // since an expanded value's bytes have no `'src` counterpart of
        // their own. A reference text recovered from such a run is necessarily
        // owned rather than borrowed.
        use crate::strings::CowStr;

        let parser = expanding_parser();

        let source = "xref:{id}[{label}]";
        let nodes = super::super::super::build(Span::new(source), &parser, None);

        assert_eq!(nodes.len(), 1, "{nodes:?}");

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "install");
        assert_eq!(reference.children.len(), 1);

        match &reference.children[0] {
            InlineNode::Text { value, .. } => {
                assert_eq!(value.as_ref(), "Install Now");
                assert!(matches!(value, CowStr::Boxed(_)), "{value:?}");
            }

            other => panic!("expected a Text child, got {other:?}"),
        }

        // The whole match is the node's location; its `{id}`/`{label}` bytes
        // are the source's, not the expanded values'.
        assert_eq!(reference.location.data(), source);
        assert_eq!(reference.location.line(), 1);
        assert_eq!(reference.location.col(), 1);
    }

    #[test]
    fn an_xref_shorthand_inside_an_expanded_value_keeps_its_exact_text() {
        // The shorthand's own version of the split above: the id and the
        // trimmed reference text are read out of the match string, which
        // carries the expanded bytes exactly, while the node's location keeps
        // the coarse fallback.
        let parser = expanding_parser();

        let source = "<<{id}, {label} >>";
        let nodes = super::super::super::build(Span::new(source), &parser, None);

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "install");

        assert_eq!(reference.children.len(), 1);
        match &reference.children[0] {
            InlineNode::Text { value, .. } => assert_eq!(value.as_ref(), "Install Now"),
            other => panic!("expected a Text child, got {other:?}"),
        }

        assert_eq!(
            super::super::super::fold_html(
                &nodes,
                &HtmlInlineRenderer {},
                &parser.render_context()
            ),
            golden_normal(source, &parser)
        );
    }

    #[test]
    fn an_xref_target_may_be_attribute_expanded() {
        // `{cpp}` is `C&#43;&#43;`, and an expanded attribute value's `&` is
        // left unescaped, since the attributes step runs after
        // `specialcharacters` — so the target crosses two
        // `Raw` leaves, which the match string stands in as placeholders.
        //
        // Those leaves are `RawOrigin::Substitution`: nothing extracted them
        // and nothing restores them, so these are exactly the bytes a
        // rendered value holds. Filling the placeholders in therefore
        // reproduces that value rather than departing from it — where a
        // *masked* passthrough, not yet restored at this point, keeps its
        // match deferred
        // (`a_deferred_xref_target_over_a_passthrough_is_a_documented_divergence`).
        //
        let renderer = HtmlInlineRenderer {};

        for fixture in [
            "see xref:{cpp}[{cpp}].",
            "see xref:{cpp}[].",
            "see <<{cpp}>>.",
            "see <<{cpp},the {cpp} page>>.",
        ] {
            let nodes = build_src(Span::new(fixture));

            assert_eq!(
                fold_html(&nodes, &renderer),
                golden_whole_pipeline(fixture),
                "fold diverged from the frozen recording for {fixture:?}"
            );
        }

        // The target itself is the *restored* value, not the placeholders a
        // first attempt at this left in it.
        let nodes = build_src(Span::new("see xref:{cpp}[{cpp}]."));
        let reference = assert_xref(&nodes[1]);

        assert_eq!(reference.target.as_ref(), "C&#43;&#43;");
    }

    #[test]
    fn an_xref_over_a_rendered_span_in_an_expanded_value_is_still_deferred() {
        // Lifting the boundary admits a *synthesized* run, not an
        // [`atomic`](Piece::atomic) one: an expanded value whose own `<` became
        // a `Raw` leaf (the attributes step runs after
        // `specialcharacters`, so a literal special in a value is emitted
        // unescaped) is opaque, so the shorthand around it still defers.
        // Asciidoctor leaves it literal too, for its own reason: its
        // `id.contains('<')` guard.
        use crate::parser::ModificationContext;

        let parser = Parser::default().with_intrinsic_attribute(
            "markup",
            "<b>x</b>",
            ModificationContext::Anywhere,
        );

        let source = "<<{markup}>>";
        let nodes = super::super::super::build(Span::new(source), &parser, None);

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "a shorthand crossing an opaque piece must be left unrecognized: {nodes:?}"
        );

        assert_eq!(
            super::super::super::fold_html(
                &nodes,
                &HtmlInlineRenderer {},
                &parser.render_context()
            ),
            golden_normal(source, &parser)
        );
    }

    #[test]
    fn a_real_documents_expanded_xref_reaches_its_tree() {
        // End-to-end, through the real parse path: a document attribute whose
        // value feeds a cross-reference. The rendered string and the fold of
        // the block's own tree agree, and the tree carries the recognized node
        // rather than the literal text it used to.
        use crate::blocks::{FindBlocks, IsBlock};

        let doc = Parser::default()
            .parse(":id: install\n\n[#install]\n== Install\n\nSee xref:{id}[the install steps].");

        let block = doc
            .descendant_blocks()
            .find(|b| {
                b.rendered_html_content()
                    .is_some_and(|c| c.contains("install steps"))
            })
            .unwrap();

        let rendered = block.rendered_html_content().unwrap();
        let inlines = block.inlines().unwrap();

        assert!(
            rendered.contains(r##"href="#install""##),
            "rendered: {rendered}"
        );

        assert!(
            inlines.iter().any(|n| matches!(n, InlineNode::Ref(_))),
            "expected a Ref node in the block's tree: {inlines:?}"
        );
    }

    #[test]
    fn a_real_documents_special_bearing_xref_text_reaches_its_tree() {
        // End-to-end, through the real parse path: a reference text carrying an
        // `&`. The block's own tree folds to the rendered string byte-for-byte
        // — the entity emitted once, not escaped a second time — and carries
        // the reference as a node rather than the literal text it used to.
        use crate::blocks::{FindBlocks, IsBlock};

        let doc = Parser::default()
            .parse("[#install]\n== Install\n\nSee xref:install[Tom & Jerry] for details.");

        let block = doc
            .descendant_blocks()
            .find(|b| {
                b.rendered_html_content()
                    .is_some_and(|c| c.contains("for details"))
            })
            .unwrap();

        let rendered = block.rendered_html_content().unwrap();
        let inlines = block.inlines().unwrap();

        assert!(rendered.contains("Tom &amp; Jerry"), "rendered: {rendered}");

        assert_eq!(
            fold_html(inlines, &HtmlInlineRenderer {}),
            rendered,
            "the block's tree must fold to its own rendered string"
        );
    }

    #[test]
    fn a_real_documents_attribute_listed_xref_text_over_a_span_reaches_its_tree() {
        // End-to-end, through the real parse path, on the shape that named this
        // increment: an attribute-list reference text enclosing a rendered
        // span. The block's tree folds to the rendered string byte-for-byte —
        // the span's markup written once, by the fold — and carries the
        // reference as a node rather than the literal macro it used to. Driven
        // in block content and in a heading's own title, whose `Title` group
        // runs the same macros step.
        use crate::blocks::{Block, FindBlocks, IsBlock};

        let doc = Parser::default().parse(concat!(
            "[#install]\n",
            "== See xref:install[the *bold* steps,role=hl]\n",
            "\n",
            "See xref:install[the *bold* steps,role=hl] for details.\n",
        ));

        let block = doc
            .descendant_blocks()
            .find(|b| {
                b.rendered_html_content()
                    .is_some_and(|c| c.contains("for details"))
            })
            .unwrap();

        let rendered = block.rendered_html_content().unwrap();
        let inlines = block.inlines().unwrap();

        assert!(
            rendered.contains(r#"class="hl""#) && rendered.contains("<strong>bold</strong>"),
            "rendered: {rendered}"
        );

        assert_eq!(
            fold_html(inlines, &HtmlInlineRenderer {}),
            rendered,
            "the block's tree must fold to its own rendered string"
        );

        let mut folded_titles = 0;

        for block in doc.descendant_blocks() {
            let Block::Section(section) = block else {
                continue;
            };

            assert_eq!(
                fold_html(section.section_title_inlines(), &HtmlInlineRenderer {}),
                section.section_title(),
                "fold diverged from the rendered section title"
            );

            folded_titles += 1;
        }

        assert_eq!(folded_titles, 1, "expected the heading to carry a tree");
    }
}
