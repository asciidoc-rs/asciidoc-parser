//! Image and icon macro recognition (`image:target[…]`, `icon:target[…]`).

use super::{MacroMatch, MacroMatchKind, rebuild_macro_level};
use crate::{
    Parser, Span,
    attributes::{Attrlist, AttrlistContext},
    content::{
        INLINE_IMAGE_MACRO, basename,
        inline_builder::{
            quotes::{
                LevelContext, Piece, build_match_string, replacement_entity, source_slice,
                text_slice,
            },
            special_chars::Masked,
        },
        normalize_text_lf_escaped_bracket,
    },
    inlines::{CharRef, Image, InlineNode},
    parser::{has_dangerous_scheme, has_dangerous_self_href, is_uri_ish},
    strings::CowStr,
    warnings::WarningType,
};

/// Matches `INLINE_IMAGE_MACRO` at this level's escaped text, replacing each
/// recognized match with the [`Image`](InlineNode::Image) node it produces and
/// leaving everything else in place.
pub(super) fn image_macros_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
    ctx: LevelContext,
    masked: Masked<'_>,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes, masked);

    // Cheap pre-filter mirroring the string step's `found_macroish`: an image
    // or icon macro needs its name prefix and an opening bracket.
    if !((s.contains("image:") || s.contains("icon:")) && s.contains('[')) {
        return nodes;
    }

    // Matched over the level wrapped in the boundary character its enclosing
    // construct presents, with the level's own pieces moved into that string's
    // coordinates — see `apply_macro_families`'s own doc comment.
    let (s, pieces) = ctx.shift(s, pieces);

    let matches = find_image_matches(&s, &pieces, root, parser, &nodes);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every image/icon macro at this level, skipping any whose match crosses
/// an [`opaque`](range_has_no_opaque_piece) piece (see
/// [`apply_macros`](super::apply_macros)).
fn find_image_matches<'src>(
    s: &str,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
    nodes: &[InlineNode<'src>],
) -> Vec<MacroMatch<'src>> {
    let mut matches = Vec::new();

    for caps in INLINE_IMAGE_MACRO.captures_iter(s) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();

        let full = whole.start()..whole.end();

        // An escape (`\image:`) is honored by dropping the backslash and
        // keeping the rest literal, mirroring `InlineImageMacroReplacer`'s own
        // leading `caps[0].starts_with('\\')` check — which it makes *before*
        // looking at anything else, so the escape needs no gate of its own
        // here either: dropping the backslash keeps the rest of the match as
        // its **own original nodes** (a rendered span or an escaped special
        // among them), which fold back to exactly the bytes the replacer's
        // `caps[0][1..]` emits. (This is the same check-order fix the
        // `footnoteref:`, menu, cross-reference, and link increments made for
        // their own families; before it, an escaped `\image:x.png[*bold*]`
        // whose match the gate rejected was left unrecognized, backslash and
        // all.)
        if whole.as_str().starts_with('\\') {
            matches.push(MacroMatch {
                kind: MacroMatchKind::Unescape {
                    backslash: full.start,
                },
                full,
            });

            continue;
        }

        // A match crossing a rendered span is left for a later increment;
        // every other piece the match string reproduces exactly — an expanded
        // attribute value, an escaped special, a restored entity — is
        // admitted, since the macro name, the target, and now the attribute
        // list are all read from bytes the string replacer's own haystack
        // carries there too.
        if !range_has_no_opaque_piece(nodes, pieces, &full) {
            continue;
        }

        let node = build_image_node(&caps, whole.as_str(), &full, pieces, root, parser, nodes);

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

/// Reports whether every piece overlapping the match-string range `range` is a
/// verbatim [`Text`](InlineNode::Text) run (non-atomic, non-synthesized). Only
/// then does the range map one-to-one onto contiguous source, so its captures
/// can slice `'src` directly. A [`synthesized`](Piece::synthesized) piece
/// (an attribute-expanded value, a `counter` directive) is rejected here too:
/// [`apply_character_replacements`](super::super::char_replacements::apply_character_replacements)
/// can recognize a construct inside one (it produces a leaf needing no `'src`
/// slice of its own — design §4.4's coarse fallback), but a macro node bakes
/// its target/attribute list straight from source, so it still needs a real
/// `'src` slice a synthesized run cannot provide — the same boundary an
/// escaped-special or a rendered span already documents.
pub(in crate::content::inline_builder) fn range_is_verbatim(
    pieces: &[Piece],
    range: &std::ops::Range<usize>,
) -> bool {
    for piece in pieces {
        let p_start = piece.s_start;
        let p_end = piece.s_start + piece.s_len;

        // Skip pieces that do not overlap the range.
        if p_end <= range.start || p_start >= range.end {
            continue;
        }

        if piece.atomic || piece.synthesized {
            return false;
        }
    }

    true
}

/// The relaxed counterpart of [`range_is_verbatim`]: also accepts a range
/// that overlaps a [`synthesized`](Piece::synthesized) piece (an attribute
/// expansion, or — reached at a tree's root — a filtered multi-line block's
/// own joined seed), rejecting only an [`atomic`](Piece::atomic) overlap (an
/// escaped special or a rendered span) — the boundary every macro family
/// still enforces unchanged. A caller that accepts a range under this check
/// still cannot slice `'src` directly for it (the same reason
/// [`range_is_verbatim`] exists at all); it needs
/// [`text_slice`] rather than
/// [`source_slice`] to recover the
/// range's own *text* precisely, since `source_slice` only ever offers a
/// synthesized piece's coarse *location* fallback (design §4.4), never its
/// exact bytes.
pub(in crate::content::inline_builder) fn range_is_verbatim_or_synthesized(
    pieces: &[Piece],
    range: &std::ops::Range<usize>,
) -> bool {
    for piece in pieces {
        let p_start = piece.s_start;
        let p_end = piece.s_start + piece.s_len;

        // Skip pieces that do not overlap the range.
        if p_end <= range.start || p_start >= range.end {
            continue;
        }

        if piece.atomic {
            return false;
        }
    }

    true
}

/// The most relaxed of the three gates: accepts a range whose overlapping
/// pieces are all ones the tree can reproduce **exactly** — a
/// [`Text`](InlineNode::Text) run (verbatim or
/// [`synthesized`](Piece::synthesized)) or any of the three
/// [`CharRef`](InlineNode::CharRef) leaves, an *escaped special*
/// ([`Special`](CharRef::Special)), a *restored entity*
/// ([`Entity`](CharRef::Entity)), or a *typographic replacement*
/// ([`Replacement`](CharRef::Replacement)) — rejecting only an **opaque**
/// piece: a rendered [`Styled`](crate::inlines::Styled) span, an
/// earlier-recognized macro node, or a masked passthrough or STEM expression,
/// each of which [`build_match_string`] stands in as one
/// `SPAN_PLACEHOLDER` rather than the markup or entity the string pipeline's
/// own haystack holds there.
///
/// All three `CharRef` leaves are admissible for the same reason: their
/// match-string bytes — a special's canonical entity (`&lt;`, `&gt;`,
/// `&amp;`), a restored entity's own text (`&copy;`, `&#8217;`), a
/// replacement's built-in rendering (`&#169;` for `(C)`, `&#8217;` for `'`,
/// via [`replacement_entity`]) — are the very byte sequence the string
/// pipeline's own haystack carries at that position, so a family that reads its
/// values out of the match string sees exactly what the string replacer sees.
/// What such a family cannot do is *slice* those bytes from `'src` (the source
/// holds one character, or `(C)`, where the match string holds an entity, and
/// `&amp;copy;` where it holds `&copy;`), so a value that must ride on the node
/// as an `'src` slice — an [`Attrlist`]`<'src>`, an
/// [`Image`](InlineNode::Image)'s bracket — keeps [`range_is_verbatim`], and a
/// *display text* recovered under this gate is rebuilt as structured children
/// with [`emit_range`](super::super::quotes::emit_range) (the leaf staying its
/// own `CharRef` child, which folds back to the same bytes) rather than as one
/// sliced `Text`.
pub(in crate::content::inline_builder) fn range_has_no_opaque_piece(
    nodes: &[InlineNode<'_>],
    pieces: &[Piece],
    range: &std::ops::Range<usize>,
) -> bool {
    for piece in pieces {
        let p_start = piece.s_start;
        let p_end = piece.s_start + piece.s_len;

        // Skip pieces that do not overlap the range.
        if p_end <= range.start || p_start >= range.end {
            continue;
        }

        if !piece.atomic {
            continue;
        }

        if !atomic_piece_is_recoverable(nodes, piece) {
            return false;
        }
    }

    true
}

/// Tells whether one [`atomic`](Piece::atomic) piece is a leaf
/// [`build_match_string`] gives real bytes to — the classification
/// [`range_has_no_opaque_piece`] applies per overlapping piece (see its own
/// doc comment for why exactly these three [`CharRef`] leaves qualify).
fn atomic_piece_is_recoverable(nodes: &[InlineNode<'_>], piece: &Piece) -> bool {
    // The atomic pieces `build_match_string` gives real bytes to are the
    // three `CharRef` leaves — an escaped special, a restored entity, and
    // a typographic replacement the built-in backend has a rendering for;
    // everything else it stands in as one placeholder. The
    // `replacement_entity` test mirrors that arm's own guard, so a
    // hand-built node carrying a value no rule produces stays opaque here
    // exactly as it does there.
    match nodes.get(piece.node_index) {
        Some(InlineNode::CharRef {
            value: CharRef::Special(_) | CharRef::Entity(_),
            ..
        }) => true,

        Some(InlineNode::CharRef {
            value: CharRef::Replacement(value),
            ..
        }) => replacement_entity(value).is_some(),

        _ => false,
    }
}

/// [`range_has_no_opaque_piece`], further admitting a masked **passthrough**
/// — a [`Raw`](InlineNode::Raw) piece — for a value the caller *restores*
/// rather than reads: the placeholder's bytes are not the string pipeline's
/// (its haystack holds the `\u{96}`*n*`\u{97}` sentinel there), but the
/// passthrough's own substituted body **is** known at build time — it is the
/// `Raw` node's `value`, the very text
/// [`Passthroughs::restore_to`](crate::content::Passthroughs) splices over
/// the sentinel after the steps run — so a computed value that substitutes it
/// for the placeholder (see [`restore_masked_passthroughs`](super::links))
/// finishes with exactly the restored string's bytes.
///
/// That makes this gate right only for a value whose *recognition* treats the
/// masked span as one swallowed token and whose *use* happens after restore —
/// today the `link:`/`mailto:` macro family's **target**, whose
/// `[^\s\[\]]+` body class swallows the sentinel and the placeholder alike,
/// and whose bytes reach the output (the `href`, and a bare macro's shown
/// text) only in the restored rendered string. A family that *matches over*
/// the masked bytes with a class the two spellings answer differently, or
/// reads them into a value the string pipeline uses **before** restore (a
/// deferred cross-reference's target, captured into its placeholder template
/// with the sentinel still in it — see
/// `a_deferred_xref_target_over_a_passthrough_is_a_documented_divergence`),
/// keeps [`range_has_no_opaque_piece`].
pub(in crate::content::inline_builder) fn range_is_restorable(
    nodes: &[InlineNode<'_>],
    pieces: &[Piece],
    range: &std::ops::Range<usize>,
) -> bool {
    for piece in pieces {
        let p_start = piece.s_start;
        let p_end = piece.s_start + piece.s_len;

        // Skip pieces that do not overlap the range.
        if p_end <= range.start || p_start >= range.end {
            continue;
        }

        if !piece.atomic {
            continue;
        }

        if matches!(nodes.get(piece.node_index), Some(InlineNode::Raw { .. })) {
            continue;
        }

        if !atomic_piece_is_recoverable(nodes, piece) {
            return false;
        }
    }

    true
}

/// Builds one [`Image`](InlineNode::Image) node from a recognized image/icon
/// match, pre-extracting the alt/width/height the way the string replacer does
/// so the fold reproduces the same bytes.
///
/// # What must be verbatim, and what need not be
///
/// The macro name and the target are read from the level's own **match
/// string** (`whole`, and [`text_slice`] for the target) rather than from a
/// source slice, so both are exact even when they come from a
/// [`synthesized`](Piece::synthesized) run — an expanded attribute value
/// (`image:{logo}[Logo]`) or a filtered multi-line block's own joined seed —
/// or cross an **escaped special** (`image:a&b.png[]`, whose target the string
/// replacer reads as `a&amp;b.png` out of its own escaped haystack: the very
/// bytes this match string carries, and the ones
/// [`apply_image_side_effects`] registers). Only the node's `location` then
/// takes design §4.4's coarse fallback.
///
/// The **attribute list** follows the same rule, one step removed. An
/// [`Attrlist`]`<'src>` reads its own `Span<'src>`'s bytes *as content*, not
/// merely as a location tag, so a bracket with no honest `'src` slice — one
/// crossing a [`synthesized`](Piece::synthesized) run
/// (`image:sunset.jpg[{caption}]`), an escaped special
/// (`image:x.png[a < b]`), or a restored entity (`image:x.png[Tom &amp;
/// Jerry]`) — cannot be parsed from the source. It is parsed from the **match
/// string** instead, through [`bracket_attrlist`], which is what the string
/// replacer itself parses (`Attrlist::parse(Span::new(&caps[2]), …)`, over its
/// own escaped, already-expanded haystack); the resulting list is then
/// [`into_owned`](Attrlist::into_owned)ed off that temporary and tagged with
/// design §4.4's coarse span. A bracket that *is* verbatim keeps its `'src`
/// slice, so its attribute values still borrow (§4.5).
fn build_image_node<'src>(
    caps: &regex::Captures<'_>,
    whole: &str,
    full: &std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
    nodes: &[InlineNode<'src>],
) -> InlineNode<'src> {
    let location = source_slice(pieces, full.clone(), root);

    // The macro name (past any — here absent — escape) is `image:` or `icon:`.
    // It is read from the match string rather than `location.data()`, which is
    // the coarse enclosing span for a macro reached through a synthesized run.
    let is_icon = !whole.starts_with("image:");

    // Group 1 is the (optional) target; group 2 is the bracket text, which
    // always participates (it may be empty).
    let target = match caps.get(1) {
        None => CowStr::from(""),

        // Borrowed from `'src` for a verbatim target (§4.5), the expansion's
        // own exact bytes for a synthesized one. A target crossing an escaped
        // special has no `'src` slice at all — the source holds one character
        // where the match string holds an entity — so it falls back to the
        // match string's own bytes, which is what `text_slice` declines to
        // recover and precisely what the string replacer reads as `caps[1]`.
        Some(m) => text_slice(nodes, pieces, m.start()..m.end())
            .unwrap_or_else(|| CowStr::from(m.as_str().to_string())),
    };

    // Group 2 always participates — its own pattern carries an empty
    // alternative — so the degenerate fallback here is unreachable, and stands
    // in for the same empty attribute list an absent group would mean rather
    // than adding a branch of its own that no input can take.
    let (bracket_text, bracket_range) = caps.get(2).map_or(("", full.end..full.end), |m| {
        (m.as_str(), m.start()..m.end())
    });

    let attrlist = bracket_attrlist(bracket_text, bracket_range, pieces, root, parser);

    // The default alt text derives from the target's basename, with `_`/`-`
    // read as spaces — exactly the string replacer's `default_alt`.
    let default_alt = basename(&target.replace(['_', '-'], " "));

    // Pre-extract the resolved alt/width/height into owned values, ending the
    // `&'src self`-tied borrows before the attribute list is moved into the node
    // (the same read-then-move shape [`attributes_of`] uses). An icon carries a
    // `size` rather than width/height, recomputed at fold time from `attrs`.
    let (alt, width, height) = if is_icon {
        let alt = attrlist.named_attribute("alt").map_or(default_alt, |a| {
            normalize_text_lf_escaped_bracket(a.value())
        });

        (Some(CowStr::from(alt)), None, None)
    } else {
        let alt = attrlist
            .named_or_positional_attribute("alt", 1)
            .map_or(default_alt, |a| {
                normalize_text_lf_escaped_bracket(a.value())
            });

        let width = attrlist
            .named_or_positional_attribute("width", 2)
            .map(|a| CowStr::from(a.value().to_string()));

        let height = attrlist
            .named_or_positional_attribute("height", 3)
            .map(|a| CowStr::from(a.value().to_string()));

        (Some(CowStr::from(alt)), width, height)
    };

    InlineNode::Image(Image {
        is_icon,
        target,
        alt,
        width,
        height,
        attrs: Some(attrlist),
        location,
    })
}

/// Parses the macro's bracket into the [`Attrlist`]`<'src>` its node carries.
///
/// A **verbatim** bracket is parsed straight from its `'src` slice, so its
/// attribute names and values borrow from the source (§4.5) — the shape every
/// ordinary `image:x.png[Alt,200]` takes.
///
/// Any other bracket has no `'src` slice whose bytes are the attrlist text: a
/// [`synthesized`](Piece::synthesized) run holds the expansion's bytes where
/// the source holds `{caption}`, and a
/// [`CharRef`](InlineNode::CharRef) leaf holds one character (`<`) or the
/// entity as written (`&amp;copy;`) where the match string holds the escaped
/// or restored form (`&lt;`, `&copy;`). Those *match-string* bytes are exactly
/// the ones `InlineImageMacroReplacer` parses out of its own haystack, so this
/// parses the same bytes — from a [`Span::new`] over the capture, whose
/// `line`/`col`/`offset` are meaningless and never escape this function —
/// and [`into_owned`](Attrlist::into_owned)s the result onto the bracket's
/// coarse source span (design §4.4), the same fallback the node's `location`
/// takes.
///
/// An **empty** bracket carries no bytes either way and parses from a
/// zero-length slice of the macro's own span.
fn bracket_attrlist<'src>(
    bracket_text: &str,
    bracket_range: std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Attrlist<'src> {
    let bracket = source_slice(pieces, bracket_range.clone(), root);

    if bracket_range.is_empty() {
        // An empty attribute list carries no bytes to slice, so it parses from
        // a zero-length span wherever the macro sits.
        return parse_attrlist(bracket.slice(0..0), parser);
    }

    if range_is_verbatim(pieces, &bracket_range) {
        return parse_attrlist(bracket, parser);
    }

    parse_attrlist(Span::new(bracket_text), parser).into_owned(bracket)
}

/// Parses one inline attribute list, discarding the warnings — the shared
/// spelling both of [`bracket_attrlist`]'s paths use.
fn parse_attrlist<'a>(source: Span<'a>, parser: &Parser) -> Attrlist<'a> {
    Attrlist::parse(source, parser, AttrlistContext::Inline)
        .item
        .item
}

/// Performs the recognition side effects the string pipeline's own
/// `InlineImageMacroReplacer` attaches to an `image:`/`icon:` match —
/// registering the image target in the document's asset catalog (`image:`
/// only, and only when [`catalog_assets`](Parser::with_catalog_assets) is
/// enabled) and recording the `link=` dangerous-scheme/self-href warning —
/// by walking an already-built tree and reading each
/// [`Image`](InlineNode::Image) node's own stored fields instead of a regex
/// capture.
///
/// Every macro family this module recognizes defers exactly this kind of
/// side effect (see this file's own `register_image` note, and the anchor,
/// link, and footnote increments' own): while the additive builder runs
/// *alongside* the authoritative string pipeline — each against its own,
/// independent [`Parser`] — performing it from every additive pass would risk
/// double-counting a registration once the two paths ever share one `Parser`.
/// This function is that deferred piece, staged as its own building block for
/// the eventual cutover (design §5.2, Phase 4 step 6): re-attaching it for
/// real means calling it exactly once per parse, after the single-pass
/// builder replaces the recorder as `Content`'s tree source, so nothing here
/// is wired into a real parse yet — it is exercised only by this module's own
/// tests, against their own `Parser`.
///
/// Recurses into every container an `Image` node can be nested inside —
/// [`Styled`](InlineNode::Styled), [`Ref`](InlineNode::Ref), and
/// [`Footnote`](InlineNode::Footnote) children — mirroring exactly where
/// [`apply_macros`](super::apply_macros) and the footnote increment's own
/// `emit_range` can place one.
pub(crate) fn apply_image_side_effects(
    nodes: &[InlineNode<'_>],
    parser: &Parser,
    source: Span<'_>,
) {
    for node in nodes {
        match node {
            InlineNode::Image(image) => {
                if !image.is_icon {
                    parser.register_image(
                        image.target.to_string(),
                        parser
                            .attribute_value("imagesdir")
                            .as_maybe_str()
                            .map(str::to_owned),
                    );
                }

                if let Some(rejected) = rejected_link_target(image, parser) {
                    parser.record_substitution_warning(
                        source,
                        WarningType::UnsafeLinkSchemeRejected(rejected.to_owned()),
                    );
                }
            }

            InlineNode::Styled(styled) => {
                apply_image_side_effects(&styled.children, parser, source);
            }

            InlineNode::Ref(reference) => {
                apply_image_side_effects(&reference.children, parser, source);
            }

            InlineNode::Footnote(footnote) => {
                apply_image_side_effects(&footnote.children, parser, source);
            }

            _ => {}
        }
    }
}

/// Mirrors `InlineImageMacroReplacer::link_self_resolves_to_src`: whether
/// `link=self` on this image/icon node resolves to a real `src` the renderer
/// promotes into the anchor `href` (an icon has one only in image-icon mode —
/// icons enabled and not font-based).
fn link_self_resolves_to_src(image: &Image<'_>, parser: &Parser) -> bool {
    !image.is_icon
        || (parser.is_attribute_set("icons")
            && parser.attribute_value("icons").as_maybe_str() != Some("font"))
}

/// Mirrors `InlineImageMacroReplacer::replace_append`'s own `link=`
/// rejection check, returning the target string the renderer would refuse to
/// promote into an `href`, if any.
fn rejected_link_target<'a>(image: &'a Image<'_>, parser: &Parser) -> Option<&'a str> {
    let link = image.attrs.as_ref()?.named_attribute("link")?;

    if link.value() == "self" {
        (link_self_resolves_to_src(image, parser)
            && has_dangerous_self_href(&image.target, is_uri_ish(&image.target)))
        .then_some(image.target.as_ref())
    } else {
        has_dangerous_scheme(link.value()).then_some(link.value())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use super::super::super::test_support::{
        assert_styled, assert_text, build_src, build_through_quotes, fold_html, golden_macros,
        golden_macros_with,
    };
    use crate::{
        HasSpan, Parser, Span,
        content::inline_builder::{
            build, char_replacements::apply_character_replacements, macros::apply_macros,
            special_chars::Masked,
        },
        inlines::{Image, InlineNode, SpanForm, StyleVariant},
        parser::HtmlSubstitutionRenderer,
        strings::CowStr,
    };

    #[test]
    fn fold_matches_the_string_pipeline_through_macros() {
        // For each fixture, folding the single-pass tree (all five steps)
        // reproduces the string pipeline's output byte-for-byte. This is the
        // differential corpus (design §5.3) that pins the image/icon increment.
        let fixtures = [
            // No macro despite macro-ish characters.
            "plain text",
            "a colon : and a bracket [ apart",
            "image without a bracket image:foo.png stays literal",
            // Images: empty, alt, defaulted alt, dimensions, named attrs.
            "image:sunset.jpg[]",
            "image:sunset.jpg[Sunset]",
            "image:sunset.jpg[Sunset Mountain]",
            "image:photo.png[Alt Text,200,100]",
            "image:photo.png[alt=Alt Text,width=200,height=100]",
            "image:a_b-c.png[]",
            "image:d/e/f.png[]",
            "image:logo.png[Logo,role=thumb]",
            "image:logo.png[title=Hover text]",
            "image:logo.png[link=https://example.org]",
            "image:logo.png[float=left]",
            // Icons.
            "icon:tags[]",
            "icon:home[Home]",
            "icon:home[size=2x]",
            // An icon's named `alt` attribute (its value normalized, escaped
            // brackets unescaped).
            "icon:home[alt=Home Page]",
            "icon:home[alt=a\\]b]",
            // A macro embedded in surrounding flow, and next to other constructs.
            "See image:sunset.jpg[Sunset] here.",
            "*bold* then image:x.png[X] and _em_",
            "before image:a.png[A] middle image:b.png[B] after",
            "a copyright (C) then image:x.png[X]",
            // Escapes: the macro stays literal, minus the backslash.
            "\\image:sunset.jpg[]",
            "\\image:sunset.jpg[Sunset]",
            "\\icon:home[]",
            // A macro inside a rendered span (recognized inside the span body).
            "*see image:x.png[X]*",
            "_image:y.png[Y] in em_",
        ];

        let renderer = HtmlSubstitutionRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_src(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden_macros(fixture),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn fold_matches_the_string_pipeline_with_document_context() {
        // The image/icon fold reads document attributes (an icon's `icons`
        // mode, an image's `imagesdir`), so the parity must hold under a
        // non-default document too. Build and fold with the *same* parser the
        // golden uses.
        use crate::parser::ModificationContext;

        let parser = Parser::default()
            .with_intrinsic_attribute("imagesdir", "assets/img", ModificationContext::Anywhere)
            .with_intrinsic_attribute("icons", "font", ModificationContext::Anywhere)
            .with_intrinsic_attribute("icontype", "svg", ModificationContext::Anywhere);

        let fixtures = [
            "image:sunset.jpg[Sunset]",
            "image:sub/dir/pic.png[Pic,320]",
            "icon:heart[]",
            "icon:heart[2x]",
            "icon:heart[size=lg,role=fav]",
            "text with icon:star[] inline",
        ];

        let renderer = HtmlSubstitutionRenderer {};

        for fixture in fixtures {
            let folded = crate::content::inline_builder::fold_html(
                &build(Span::new(fixture), &parser, None),
                &renderer,
                &parser,
            );

            assert_eq!(
                folded,
                golden_macros_with(fixture, &parser),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    /// Asserts that `node` is an [`Image`](InlineNode::Image), returning it for
    /// further inspection.
    fn assert_image<'a, 'src>(node: &'a InlineNode<'src>) -> &'a Image<'src> {
        match node {
            InlineNode::Image(image) => image,

            other => panic!("expected Image, got {other:?}"),
        }
    }

    #[test]
    fn an_image_macro_becomes_a_self_describing_node() {
        let nodes = build_src(Span::new("image:sunset.jpg[Sunset]"));

        assert_eq!(nodes.len(), 1);
        let image = assert_image(&nodes[0]);

        assert!(!image.is_icon);

        // The target borrows from source (no allocation), and the alt is the
        // supplied positional value.
        assert!(matches!(image.target, CowStr::Borrowed(_)));
        assert_eq!(image.target.as_ref(), "sunset.jpg");
        assert_eq!(image.alt.as_deref(), Some("Sunset"));
        assert_eq!(image.width, None);
        assert_eq!(image.height, None);

        // The node captures its own attribute list — the property that makes it
        // self-describing (and unblocks a faithful fold).
        assert!(image.attrs.is_some(), "the attribute list is retained");

        // Its location covers the whole macro, delimiters included.
        assert_eq!(image.location.data(), "image:sunset.jpg[Sunset]");
        assert_eq!(image.location.line(), 1);
        assert_eq!(image.location.col(), 1);
    }

    #[test]
    fn image_dimensions_are_captured_positionally() {
        let nodes = build_src(Span::new("image:p.png[Alt,200,100]"));

        let image = assert_image(&nodes[0]);
        assert_eq!(image.alt.as_deref(), Some("Alt"));
        assert_eq!(image.width.as_deref(), Some("200"));
        assert_eq!(image.height.as_deref(), Some("100"));
    }

    #[test]
    fn image_default_alt_derives_from_the_basename() {
        // With no alt, the default is the target's basename with `_`/`-` read as
        // spaces and the extension dropped.
        let nodes = build_src(Span::new("image:a_b-c.png[]"));

        let image = assert_image(&nodes[0]);
        assert_eq!(image.alt.as_deref(), Some("a b c"));
    }

    #[test]
    fn an_icon_macro_becomes_an_icon_node() {
        let nodes = build_src(Span::new("icon:home[size=2x]"));

        let image = assert_image(&nodes[0]);
        assert!(image.is_icon);
        assert_eq!(image.target.as_ref(), "home");

        // An icon has no positional width/height; its `size` lives in the
        // attribute list (read back at fold time), and its default alt is the
        // target itself.
        assert_eq!(image.width, None);
        assert_eq!(image.height, None);
        assert_eq!(image.alt.as_deref(), Some("home"));
    }

    #[test]
    fn an_image_macro_is_recognized_inside_a_span() {
        // A macro can appear inside a rendered span; the transducer descends
        // into the span body and builds the node there.
        let nodes = build_src(Span::new("*see image:x.png[X]*"));

        let children = assert_styled(&nodes[0], StyleVariant::Strong, SpanForm::Constrained);
        assert_eq!(children.len(), 2);
        assert_text(&children[0], "see ", 1, 2);

        let image = assert_image(&children[1]);
        assert_eq!(image.target.as_ref(), "x.png");
        assert_eq!(image.alt.as_deref(), Some("X"));
    }

    #[test]
    fn an_escaped_image_macro_stays_literal() {
        // `\image:…` drops the backslash and keeps the macro as literal text —
        // no image node.
        let nodes = build_src(Span::new("\\image:sunset.jpg[Sunset]"));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Image(_))),
            "an escaped macro must not produce an image node: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros("\\image:sunset.jpg[Sunset]")
        );
    }

    #[test]
    fn an_escaped_macro_over_a_rendered_span_still_drops_its_backslash() {
        // The escape check runs *ahead* of the gate (the same check-order fix
        // the `footnoteref:`, menu, cross-reference, and link families made),
        // so a macro the gate would reject still honors its own escape:
        // dropping the backslash leaves the rest as its own nodes, which fold
        // back to exactly the bytes `caps[0][1..]` emits.
        let source = "\\image:x.png[*bold*]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Image(_))),
            "an escaped macro must not produce an image node: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_macros(source)
        );
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_a_target_crossing_an_escaped_special() {
        // The string pipeline matches macros over *escaped* text, so a target
        // containing `&` is matched as `a&amp;b.png`. Those entity bytes are
        // exactly what this level's match string carries, so the node's target
        // is read off it and the fold reproduces the same `src`/default alt —
        // the escaped special being the one atomic piece
        // `range_has_no_opaque_piece` admits.
        let fixtures = [
            // A target crossing each of the three specials, alone and doubled.
            "image:a&b.png[]",
            "image:a<b.png[]",
            "image:a>b.png[]",
            "image:a&b&c.png[]",
            "icon:a&b[]",
            // A verbatim attribute list beside such a target: positional alt,
            // positional width/height, and named attributes (including the
            // `link=` forms `apply_image_side_effects` reads).
            "image:a&b.png[Alt]",
            "image:a&b.png[Alt,200,100]",
            "image:a&b.png[alt=Alt Text,role=thumb]",
            "image:a&b.png[link=self]",
            "image:a&b.png[alt=X,link=javascript:alert(1)]",
            // In surrounding flow, doubled, inside a rendered span, and beside
            // a sibling family that takes the same lift.
            "before image:a&b.png[A] after",
            "image:a&b.png[] image:c&d.png[]",
            "*image:a&b.png[A]*",
            "image:a&b.png[Alt] and link:x&y.html[]",
            // A special the target's own character classes reject (a newline
            // is impossible, but a space ends the target), so neither pipeline
            // builds a macro.
            "image:a & b.png[]",
            // The escape still keeps the macro literal, backslash dropped.
            "\\image:a&b.png[A]",
            // A special *beside* the macro rather than inside it.
            "image:sunset.jpg[]&amp;",
        ];

        let renderer = HtmlSubstitutionRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_src(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden_macros(fixture),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn a_target_crossing_an_escaped_special_carries_the_entity_bytes() {
        // The node's `target` is the string replacer's own `caps[1]` — the
        // escaped haystack's bytes, not the source's single `&` — which is
        // also what `apply_image_side_effects` registers. The default alt
        // derives from that same string, exactly as `default_alt` does.
        let source = "image:a&b.png[]";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        let image = assert_image(&nodes[0]);

        assert_eq!(image.target.as_ref(), "a&amp;b.png");
        assert_eq!(image.alt.as_deref(), Some("a&amp;b"));

        // The whole macro's span is still precise: neither the match's start
        // (`i`, or a backslash) nor its end (`]`) can fall inside an entity,
        // so no boundary lands in an atomic piece.
        assert_eq!(image.location.data(), source);
        assert_eq!(image.location.line(), 1);
        assert_eq!(image.location.col(), 1);
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_an_attribute_list_crossing_an_escaped_special() {
        // The bracket has no `'src` slice here — the source holds one
        // character where the match string holds an entity — so it is parsed
        // from the match string, which is the very `caps[2]` the string
        // replacer parses out of its own escaped haystack, and owned off that
        // temporary. Every attribute the two read is therefore the same.
        let fixtures = [
            // A special in a positional alt, in a named value, and in both a
            // named value and the target at once.
            "image:x.png[a < b]",
            "image:x.png[alt=a & b]",
            "image:a&b.png[a < b]",
            // Every special, and more than one in the same bracket.
            "image:x.png[a > b]",
            "image:x.png[a < b & c > d]",
            // Beside the positional width/height, and with a role.
            "image:x.png[a & b,200,100]",
            "image:x.png[a & b,role=thumb]",
            // The `icon:` spelling, whose size is read back from `attrs` at
            // fold time rather than pre-extracted.
            "icon:home[a & b]",
            "icon:home[2x,role=a & b]",
            // In surrounding flow, inside a rendered span, and doubled.
            "before image:x.png[a < b] after",
            "*image:x.png[a < b]*",
            "image:x.png[a & b] image:y.png[c & d]",
            // The escape still keeps the macro literal, backslash dropped.
            "\\image:x.png[a < b]",
        ];

        let renderer = HtmlSubstitutionRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_src(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden_macros(fixture),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn an_attribute_list_crossing_an_escaped_special_is_owned_and_coarsely_located() {
        // The parsed values are the *escaped* ones the string replacer reads
        // (`a &lt; b`, not `a < b`), they own their bytes rather than
        // borrowing from the temporary they were parsed from, and the list's
        // own span falls back to the bracket's coarse source range (design
        // §4.4) — the same split the node's `location` already takes for a
        // synthesized run.
        let source = "image:x.png[a < b,role=hl]";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        let image = assert_image(&nodes[0]);

        assert_eq!(image.alt.as_deref(), Some("a &lt; b"));

        let attrs = image.attrs.as_ref().unwrap();
        assert_eq!(attrs.nth_attribute(1).unwrap().value(), "a &lt; b");
        assert_eq!(attrs.named_attribute("role").unwrap().value(), "hl");

        // The location tag is the bracket's own source text, which is *not*
        // what was parsed — the coarse fallback, kept honest as a location.
        assert_eq!(attrs.span().data(), "a < b,role=hl");

        // The whole macro's span is still precise: neither the match's start
        // nor its end can fall inside an entity.
        assert_eq!(image.location.data(), source);
        assert_eq!(image.location.line(), 1);
        assert_eq!(image.location.col(), 1);
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_a_target_crossing_a_restored_entity() {
        // An author-written entity (`&amp;copy;`, `&amp;#8217;`) is escaped by
        // `SpecialCharacters` and then *restored* by `CharacterReplacements`
        // into a `CharRef::Entity` leaf whose value is the entity itself. Those
        // bytes are what the string pipeline's own haystack carries from the
        // replacements step onward, and what the fold emits verbatim, so
        // `range_has_no_opaque_piece` admits the leaf exactly as it admits an
        // escaped special.
        let fixtures = [
            // A target crossing a restored entity — the `&amp;` spelling of
            // `&`, a named entity, and a numeric one.
            "image:a&amp;b.png[]",
            "image:&lt;.png[]",
            "image:a&copy;b.png[]",
            "image:a&#8217;b.png[]",
            "icon:a&copy;b[]",
            // Beside a verbatim attribute list, and doubled.
            "image:a&copy;b.png[Alt]",
            "image:a&copy;b.png[Alt,200,100]",
            "image:a&copy;b&reg;c.png[]",
            // A target crossing *both* a restored entity and an escaped
            // special, which only the match string carries the bytes of.
            "image:a&copy;b&c.png[]",
            // In surrounding flow, inside a rendered span, and beside a
            // sibling family that takes the same lift.
            "before image:a&copy;b.png[A] after",
            "*image:a&copy;b.png[A]*",
            "image:a&copy;b.png[Alt] and link:x&reg;y.html[]",
            // The escape still keeps the macro literal, backslash dropped.
            "\\image:a&copy;b.png[A]",
            // An entity *beside* the macro rather than inside it.
            "&copy;image:sunset.jpg[]&reg;",
        ];

        let renderer = HtmlSubstitutionRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_src(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden_macros(fixture),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn a_target_crossing_a_restored_entity_carries_the_entity_bytes() {
        // As for an escaped special, the node's `target` is the string
        // replacer's own `caps[1]` — here the *restored* entity's bytes, which
        // are also the source's own — and the default alt derives from that
        // same string.
        let source = "image:a&copy;b.png[]";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        let image = assert_image(&nodes[0]);

        assert_eq!(image.target.as_ref(), "a&copy;b.png");
        assert_eq!(image.alt.as_deref(), Some("a&copy;b"));

        // The whole macro's span is still precise: neither the match's start
        // nor its end can fall inside an entity.
        assert_eq!(image.location.data(), source);
        assert_eq!(image.location.line(), 1);
        assert_eq!(image.location.col(), 1);
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_an_attribute_list_crossing_a_restored_entity() {
        // A restored entity takes the same lift as an escaped special, for the
        // same reason: the source holds `&amp;copy;` where the match string
        // holds `&copy;`, so the bracket has no `'src` slice — and the match
        // string's bytes are the ones the string replacer parses.
        let fixtures = [
            "image:x.png[Tom &amp; Jerry]",
            "image:x.png[alt=a &copy; b]",
            "image:x.png[a &#8217; b]",
            // An entity in the bracket *and* in the target.
            "image:a&copy;b.png[Tom &amp; Jerry]",
            // An entity and an escaped special in the same bracket.
            "image:x.png[a &copy; b < c]",
            // Beside the positional width/height, and the `icon:` spelling.
            "image:x.png[Tom &amp; Jerry,200,100]",
            "icon:home[Tom &amp; Jerry]",
            // In surrounding flow and inside a rendered span.
            "before image:x.png[Tom &amp; Jerry] after",
            "*image:x.png[Tom &amp; Jerry]*",
            // The escape still keeps the macro literal, backslash dropped.
            "\\image:x.png[Tom &amp; Jerry]",
        ];

        let renderer = HtmlSubstitutionRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_src(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden_macros(fixture),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_a_macro_crossing_a_character_replacement() {
        // A typographic replacement (`(C)`, `(R)`, `'`, `...`) is the third
        // recoverable piece, admitted for the same reason the two `CharRef`
        // entity leaves are: `build_match_string` gives it the entity the
        // built-in backend renders it as, which is what the string pipeline's
        // own haystack carries from the replacements step onward — so both the
        // target read off that string and the bracket parsed from it are the
        // string replacer's own bytes.
        let fixtures = [
            // A target crossing one, alone and beside an attribute list.
            "image:a(C)b.png[]",
            "image:a(C)b.png[Alt]",
            "icon:a(C)b[]",
            // An **attribute list** crossing one — the shape three real
            // fixtures in this crate's own corpora write.
            "image:pause.png[title=Pause (C) Resume]",
            "image:x.png[A tiger's roar]",
            "image:x.png[alt=a (C) b]",
            "image:x.png[Wait...]",
            "image:x.png[Tom (C) Jerry,200,100]",
            "icon:home[Tom (C) Jerry]",
            // A replacement in the bracket *and* in the target, and one
            // beside an escaped special and a restored entity.
            "image:a(C)b.png[Tom (C) Jerry]",
            "image:x.png[a (C) b < c &copy; d]",
            // In surrounding flow, inside a rendered span, doubled, and beside
            // a sibling family that takes the same lift.
            "before image:x.png[Tom (C) Jerry] after",
            "*image:x.png[Tom (C) Jerry]*",
            "image:a(C)b.png[] image:c(R)d.png[]",
            "image:x.png[Tom (C) Jerry] and link:x(R)y.html[]",
            // The escape still keeps the macro literal, backslash dropped.
            "\\image:x.png[Tom (C) Jerry]",
            // A replacement *beside* the macro rather than inside it.
            "(C)image:sunset.jpg[](R)",
        ];

        let renderer = HtmlSubstitutionRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_src(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden_macros(fixture),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn an_attribute_list_crossing_a_character_replacement_reads_the_rendered_entity() {
        // The structural companion: the bracket has no `'src` slice (the
        // source holds `(C)` where the match string holds `&#169;`), so it is
        // parsed from the match string and owned onto design §4.4's coarse
        // span — carrying the already-substituted value the string replacer
        // parses, entity and all.
        let source = "image:x.png[title=Pause (C) Resume]";
        let nodes = build_src(Span::new(source));

        assert_eq!(nodes.len(), 1);
        let image = assert_image(&nodes[0]);

        let attrlist = image.attrs.as_ref().unwrap();
        assert_eq!(
            attrlist.named_attribute("title").unwrap().value(),
            "Pause &#169; Resume"
        );

        assert_eq!(image.location.data(), source);
    }

    #[test]
    fn a_macro_over_a_rendered_span_is_a_documented_divergence() {
        // A rendered span inside the match is the boundary this family keeps:
        // `build_match_string` stands it in as one `SPAN_PLACEHOLDER`, so
        // neither the target nor the bracket has bytes to read there.
        //
        // If this boundary is ever lifted, fold these fixtures into the parity
        // corpus above.
        let source = "image:x.png[*bold*]";

        let nodes = apply_macros(
            build_through_special_and_replacements(Span::new(source)),
            Span::new(source),
            &Parser::default(),
            Masked::UNKNOWN,
        );

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Image(_))),
            "a macro crossing a rendered span must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* build an image here.
        assert!(golden_macros(source).contains("<img"));
    }

    #[test]
    fn an_image_target_over_a_passthrough_is_a_documented_divergence() {
        // The image family keeps the opaque-piece gate over a masked
        // passthrough. Its builder computes more than one value off the
        // masked bytes — the target feeds `src`, the `default_alt`
        // derivation (whose basename/extension/`_`/`-` rewrites the string
        // pipeline applies to the sentinel bytes, so `image:++a_b-c.jpg++[]`
        // keeps `alt="a_b-c.jpg"` where the verbatim spelling shows `a b c`),
        // and the registered asset — so the lift needs the
        // masked-arithmetic-then-restore treatment applied per value, an
        // increment of its own. The string pipeline swallows the sentinel
        // into the target and restores it in the rendered `src`.
        use super::super::super::test_support::golden_passthroughs;

        let source = "image:++sunset.jpg++[Alt]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Image(_))),
            "an image target over a passthrough must stay literal: {nodes:?}"
        );

        let golden = golden_passthroughs(source);

        assert!(
            golden.contains("src=\"sunset.jpg\""),
            "expected the documented divergence to still reproduce: {golden:?}"
        );

        assert_ne!(golden, fold_html(&nodes, &HtmlSubstitutionRenderer {}));
    }

    #[test]
    fn matches_the_golden_pipelines_registration_for_a_target_crossing_an_escaped_special() {
        // The staged `register_image` reads the node's own stored `target`,
        // which is now the escaped one — so it must be byte-identical to the
        // `caps[1]` the string replacer registers. Two independent parsers, as
        // elsewhere in this module.
        let fixtures = [
            "image:a&b.png[]",
            "image:a<b.png[Alt]",
            "image:a&b.png[] image:c&d.png[]",
            "icon:a&b[]",
            "\\image:a&b.png[A]",
        ];

        for fixture in fixtures {
            let builder_parser = Parser::default().with_catalog_assets(true);
            let nodes = build_with(Span::new(fixture), &builder_parser);
            apply_image_side_effects(&nodes, &builder_parser, Span::new(fixture));

            let golden_parser = Parser::default().with_catalog_assets(true);
            golden_macros_with(fixture, &golden_parser);

            let got: Vec<_> = builder_parser
                .catalog()
                .images()
                .iter()
                .map(|i| i.target.clone())
                .collect();

            let want: Vec<_> = golden_parser
                .catalog()
                .images()
                .iter()
                .map(|i| i.target.clone())
                .collect();

            assert_eq!(got, want, "registered images diverged for {fixture:?}");
        }
    }

    /// Builds the tree **through character replacements** (special characters,
    /// quotes, character replacements) — the state the macros step consumes —
    /// so a test can drive [`apply_macros`] directly.
    fn build_through_special_and_replacements(source: Span<'_>) -> Vec<InlineNode<'_>> {
        apply_character_replacements(build_through_quotes(source), source)
    }

    // ---- `apply_image_side_effects` (staged for the eventual cutover) -----

    use super::apply_image_side_effects;
    use crate::warnings::WarningType;

    /// Builds the single-pass tree for `source` against `parser` (unlike
    /// [`build_src`], which always uses its own fresh default parser).
    fn build_with<'src>(source: Span<'src>, parser: &Parser) -> Vec<InlineNode<'src>> {
        build(source, parser, None)
    }

    #[test]
    fn registers_an_image_target_when_catalog_assets_is_enabled() {
        let source = "image:sunset.jpg[Sunset]";
        let parser = Parser::default().with_catalog_assets(true);
        let nodes = build_with(Span::new(source), &parser);

        apply_image_side_effects(&nodes, &parser, Span::new(source));

        let catalog = parser.catalog();
        let images = catalog.images();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].target, "sunset.jpg");
    }

    #[test]
    fn does_not_register_an_icon_as_an_image() {
        // `icon:` shares the `Image` node with `image:`, but only `image:` is
        // registered in the asset catalog — mirroring
        // `InlineImageMacroReplacer`'s own `caps[0].starts_with("image:")`
        // gate.
        let source = "icon:home[]";
        let parser = Parser::default().with_catalog_assets(true);
        let nodes = build_with(Span::new(source), &parser);

        apply_image_side_effects(&nodes, &parser, Span::new(source));

        assert!(parser.catalog().images().is_empty());
    }

    #[test]
    fn registration_is_a_no_op_when_catalog_assets_is_disabled() {
        // `catalog_assets` defaults to off; `register_image` is then a no-op,
        // mirroring the string pipeline's own `Parser::register_image`.
        let source = "image:sunset.jpg[Sunset]";
        let parser = Parser::default();
        let nodes = build_with(Span::new(source), &parser);

        apply_image_side_effects(&nodes, &parser, Span::new(source));

        assert!(parser.catalog().images().is_empty());
    }

    #[test]
    fn registers_an_image_nested_inside_a_styled_span_and_a_footnote() {
        // An `Image` node can be nested inside a `Styled` span (matched inside
        // a rendered span, mirroring the string pipeline) or captured whole
        // into a `Footnote`'s own children (the footnote increment's own
        // `emit_range`); both containers must be walked.
        let source = "*see image:a.png[]* and footnote:[see image:b.png[]]";
        let parser = Parser::default().with_catalog_assets(true);
        let nodes = build_with(Span::new(source), &parser);

        apply_image_side_effects(&nodes, &parser, Span::new(source));

        let catalog = parser.catalog();
        let images = catalog.images();
        assert_eq!(images.len(), 2);
        assert_eq!(images[0].target, "a.png");
        assert_eq!(images[1].target, "b.png");
    }

    #[test]
    fn registers_an_image_nested_inside_a_refs_display_children() {
        // A `Ref`'s display children can hold an `Image` too — `apply_macros`
        // itself descends into a `Ref`'s own children before matching at its
        // level (see `apply_macros_recognizes_a_macro_inside_reference_children`
        // in `macros/mod.rs`'s own tests), so a hand-built `Ref` exercises the
        // same container here.
        use crate::inlines::{Ref, RefVariant};

        let root = Span::new("image:a.png[]");
        let image = build_with(root, &Parser::default());
        assert_eq!(image.len(), 1);

        let reference = InlineNode::Ref(Ref {
            variant: RefVariant::Link,
            target: CowStr::from("https://example.org"),
            children: image,
            roles: vec![],
            window: None,
            resolved: None,
            derived: None,
            xrefstyle: None,
            attrs: None,
            location: root,
        });

        let parser = Parser::default().with_catalog_assets(true);
        apply_image_side_effects(std::slice::from_ref(&reference), &parser, root);

        let catalog = parser.catalog();
        let images = catalog.images();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].target, "a.png");
    }

    #[test]
    fn matches_the_golden_pipelines_registration_for_a_broad_fixture_set() {
        // Each fixture uses its own pair of *independent* parsers (design
        // §5.3's two-independent-parsers discipline, already established by
        // the footnote increment's own differential corpus): one that the
        // additive builder builds against and this function then walks, one
        // that the real string pipeline (`golden_macros_with`) runs against
        // directly. Because neither path is wired into the other, comparing
        // their two catalogs after the fact is the whole test.
        let fixtures = [
            "image:sunset.jpg[Sunset]",
            "icon:home[]",
            "image:sunset.jpg[Sunset]{sp}image:other.png[]",
            "image without a bracket image:foo.png stays literal",
            "\\image:sunset.jpg[Sunset]",
            // A bracket with no `'src` slice of its own: the attribute list
            // is parsed from the match string, but `register_image` reads the
            // node's `target`, so the catalogs must still agree.
            "image:sunset.jpg[a < b]",
            "image:a&b.png[Tom &amp; Jerry,200]",
        ];

        for fixture in fixtures {
            let builder_parser = Parser::default().with_catalog_assets(true);
            let nodes = build_with(Span::new(fixture), &builder_parser);
            apply_image_side_effects(&nodes, &builder_parser, Span::new(fixture));

            let golden_parser = Parser::default().with_catalog_assets(true);
            golden_macros_with(fixture, &golden_parser);

            let got: Vec<_> = builder_parser
                .catalog()
                .images()
                .iter()
                .map(|i| i.target.clone())
                .collect();
            let want: Vec<_> = golden_parser
                .catalog()
                .images()
                .iter()
                .map(|i| i.target.clone())
                .collect();

            assert_eq!(got, want, "registered images diverged for {fixture:?}");
        }
    }

    #[test]
    fn records_the_dangerous_scheme_warning_from_a_bracket_with_no_source_slice() {
        // The `link=` warning reads the node's stored attribute list, so a
        // bracket parsed from the match string must carry it exactly as a
        // verbatim one does — here reached through an escaped special earlier
        // in the same bracket, which is what denies it an `'src` slice.
        let source = "image:safe.png[a < b,link=javascript:alert(1)]";
        let parser = Parser::default();
        let nodes = build_with(Span::new(source), &parser);

        let before = parser.substitution_warnings_len();
        apply_image_side_effects(&nodes, &parser, Span::new(source));
        let warnings = parser.drain_substitution_warnings_since(before);

        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::UnsafeLinkSchemeRejected("javascript:alert(1)".to_string())
        );
    }

    #[test]
    fn records_the_dangerous_scheme_warning_for_an_explicit_link_target() {
        let source = "image:safe.png[alt,link=javascript:alert(1)]";
        let parser = Parser::default();
        let nodes = build_with(Span::new(source), &parser);

        let before = parser.substitution_warnings_len();
        apply_image_side_effects(&nodes, &parser, Span::new(source));
        let warnings = parser.drain_substitution_warnings_since(before);

        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::UnsafeLinkSchemeRejected("javascript:alert(1)".to_string())
        );
    }

    #[test]
    fn records_the_dangerous_scheme_warning_case_insensitively() {
        let source = "image:safe.png[alt,link=JavaScript:alert(1)]";
        let parser = Parser::default();
        let nodes = build_with(Span::new(source), &parser);

        let before = parser.substitution_warnings_len();
        apply_image_side_effects(&nodes, &parser, Span::new(source));
        let warnings = parser.drain_substitution_warnings_since(before);

        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn does_not_warn_for_a_safe_explicit_link_target() {
        let source = "image:safe.png[alt,link=https://example.org]";
        let parser = Parser::default();
        let nodes = build_with(Span::new(source), &parser);

        let before = parser.substitution_warnings_len();
        apply_image_side_effects(&nodes, &parser, Span::new(source));

        assert_eq!(parser.substitution_warnings_len(), before);
    }

    #[test]
    fn records_the_warning_for_a_dangerous_image_target_promoted_by_link_self() {
        // `link=self` resolves the anchor `href` to the image's own `src`
        // (`target`); a dangerous target is rejected exactly as an explicit
        // `link=` value would be.
        let source = "image:javascript:alert(1)[alt,link=self]";
        let parser = Parser::default();
        let nodes = build_with(Span::new(source), &parser);

        let before = parser.substitution_warnings_len();
        apply_image_side_effects(&nodes, &parser, Span::new(source));
        let warnings = parser.drain_substitution_warnings_since(before);

        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::UnsafeLinkSchemeRejected("javascript:alert(1)".to_string())
        );
    }

    #[test]
    fn does_not_warn_for_link_self_on_a_safe_image_target() {
        let source = "image:safe.png[alt,link=self]";
        let parser = Parser::default();
        let nodes = build_with(Span::new(source), &parser);

        let before = parser.substitution_warnings_len();
        apply_image_side_effects(&nodes, &parser, Span::new(source));

        assert_eq!(parser.substitution_warnings_len(), before);
    }

    #[test]
    fn records_the_warning_for_a_dangerous_icon_target_promoted_by_link_self_in_image_icon_mode() {
        // An `icon:` target only promotes into a live `href` in image-icon
        // mode (`icons` set, not `font`); with `icons` unset (the default) an
        // icon has no `src` at all, so `link=self` stays the literal `self`
        // and nothing is rejected — see the companion
        // `does_not_warn_for_link_self_on_a_font_icon` test for that case.
        use crate::parser::ModificationContext;

        let source = "icon:javascript:alert(1)[link=self]";
        let parser =
            Parser::default().with_intrinsic_attribute("icons", "", ModificationContext::ApiOnly);
        let nodes = build_with(Span::new(source), &parser);

        let before = parser.substitution_warnings_len();
        apply_image_side_effects(&nodes, &parser, Span::new(source));
        let warnings = parser.drain_substitution_warnings_since(before);

        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].warning,
            WarningType::UnsafeLinkSchemeRejected("javascript:alert(1)".to_string())
        );
    }

    #[test]
    fn does_not_warn_for_link_self_on_a_font_icon() {
        // A font icon has no `src`, so `link=self` resolves to the literal
        // `self` (harmless) rather than the dangerous target — nothing is
        // rejected, mirroring
        // `font_icon_link_self_with_dangerous_target_keeps_literal_self_without_warning`
        // in `parser/src/tests/security.rs`.
        use crate::parser::ModificationContext;

        let source = "icon:javascript:alert(1)[link=self]";
        let parser = Parser::default().with_intrinsic_attribute(
            "icons",
            "font",
            ModificationContext::ApiOnly,
        );
        let nodes = build_with(Span::new(source), &parser);

        let before = parser.substitution_warnings_len();
        apply_image_side_effects(&nodes, &parser, Span::new(source));

        assert_eq!(parser.substitution_warnings_len(), before);
    }

    /// A parser carrying the attributes the expanded-value fixtures below
    /// reference.
    fn expanding_parser() -> Parser {
        use crate::parser::ModificationContext;

        Parser::default()
            .with_intrinsic_attribute("logo", "sunset.jpg", ModificationContext::Anywhere)
            .with_intrinsic_attribute("dir", "assets", ModificationContext::Anywhere)
            .with_intrinsic_attribute("iconname", "home", ModificationContext::Anywhere)
            .with_intrinsic_attribute("caption", "Sunset", ModificationContext::Anywhere)
            .with_intrinsic_attribute(
                "img-src",
                "image:sunset.jpg[]",
                ModificationContext::Anywhere,
            )
            .with_intrinsic_attribute(
                "img-src-alt",
                "image:sunset.jpg[Sunset]",
                ModificationContext::Anywhere,
            )
    }

    /// The real, public pipeline's output for `source` — the golden for the
    /// expanded-value fixtures, which need the `AttributeReferences` step
    /// [`golden_macros_with`] deliberately omits.
    fn golden_normal(source: &str, parser: &Parser) -> String {
        use crate::content::{Content, SubstitutionGroup};

        let mut content = Content::from(Span::new(source));
        SubstitutionGroup::Normal.apply(&mut content, parser, None);
        content.rendered_str().to_string()
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_images_inside_expanded_values() {
        // An image or icon macro whose *target* (or whose whole macro name)
        // crosses a synthesized run is now recognized: the name and target are
        // read from the level's match string, which carries an expanded value's
        // bytes exactly, so only the node's `location` takes design §4.4's
        // coarse fallback. The macro's own attribute list is the one part that
        // still needs an honest `'src` slice — see the divergence test below —
        // so every fixture here either carries a verbatim bracket or an empty
        // one.
        let parser = expanding_parser();

        let fixtures = [
            // An expanded target, with and without an attribute list.
            "image:{logo}[Logo]",
            "image:{logo}[]",
            "icon:{iconname}[]",
            "icon:{iconname}[Home]",
            // A partially-expanded target: the expansion is one piece of it.
            "image:{dir}/sunset.jpg[Sunset]",
            "image:{dir}/{logo}[]",
            // A target crossing *both* a synthesized run and an escaped
            // special, so neither the source nor the expansion alone carries
            // its bytes — only the level's own match string does.
            "image:{dir}/a&b.png[]",
            "image:{dir}/a&b.png[Alt]",
            // A verbatim attribute list beside an expanded target, including
            // the positional width/height and a named attribute.
            "image:{logo}[Alt Text,200,100]",
            "image:{logo}[alt=Alt Text,width=200]",
            "image:{logo}[Logo,role=thumb]",
            // The whole macro arriving from an expanded value — recognized
            // only because its bracket is empty (see the divergence test for
            // the non-empty case).
            "see {img-src} here",
            // Embedded in surrounding flow, beside another macro, and inside a
            // rendered span.
            "See image:{logo}[Logo] here.",
            "image:{logo}[A] then image:other.png[B]",
            "*image:{logo}[Logo]*",
            // An escape still keeps the macro literal.
            "\\image:{logo}[Logo]",
        ];

        let renderer = HtmlSubstitutionRenderer {};

        for fixture in fixtures {
            let folded = crate::content::inline_builder::fold_html(
                &build(Span::new(fixture), &parser, None),
                &renderer,
                &parser,
            );

            assert_eq!(
                folded,
                golden_normal(fixture, &parser),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn an_image_inside_an_expanded_value_keeps_a_coarse_location() {
        // The target is recovered *exactly* (through `text_slice`), while the
        // node's `location` falls back to the enclosing synthesized run's own
        // span — design §4.4's documented split, the same one the anchor, UI,
        // index-term, and cross-reference families already take.
        let parser = expanding_parser();
        let source = "image:{logo}[Logo]";
        let nodes = build(Span::new(source), &parser, None);

        assert_eq!(nodes.len(), 1);
        let image = assert_image(&nodes[0]);

        assert!(!image.is_icon);
        assert_eq!(image.target.as_ref(), "sunset.jpg");
        assert_eq!(image.alt.as_deref(), Some("Logo"));

        // The whole macro's span: its own source bytes, not the expansion's.
        assert_eq!(image.location.data(), source);
    }

    #[test]
    fn an_icons_name_inside_an_expanded_value_is_read_from_the_match_string() {
        // The `image:`/`icon:` discriminant is read from the match string, not
        // from `location.data()` — which, for a wholly-expanded macro, is the
        // attribute reference rather than the macro.
        use crate::parser::ModificationContext;

        let parser = Parser::default().with_intrinsic_attribute(
            "icon-src",
            "icon:home[]",
            ModificationContext::Anywhere,
        );

        let nodes = build(Span::new("{icon-src}"), &parser, None);

        assert_eq!(nodes.len(), 1);
        let image = assert_image(&nodes[0]);

        assert!(
            image.is_icon,
            "the macro name must come from the match string"
        );
        assert_eq!(image.target.as_ref(), "home");
        assert_eq!(image.location.data(), "{icon-src}");
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_an_attribute_list_inside_an_expanded_value() {
        // A non-empty attribute list crossing a synthesized run takes the same
        // lift: the expansion's bytes live only in the level's match string —
        // which is exactly the already-substituted haystack the string
        // replacer parses its own `caps[2]` out of — so the bracket is parsed
        // from there and owned off it.
        let parser = expanding_parser();

        let fixtures = [
            // An expanded alt, whole and in part, with and without an
            // expanded target beside it.
            "image:sunset.jpg[{caption}]",
            "image:{logo}[{caption}]",
            "image:sunset.jpg[The {caption} photo]",
            // An expanded value in a *named* attribute, and beside the
            // positional width/height.
            "image:sunset.jpg[alt={caption}]",
            "image:{logo}[{caption},200]",
            "image:{logo}[{caption},role=thumb]",
            // The whole macro arriving from an expanded value, this time with
            // a non-empty bracket — the empty-bracket spelling of the same
            // shape was already recognized.
            "see {img-src-alt} here",
            // An expansion and an escaped special in the same bracket, so
            // neither the source nor the expansion alone carries its bytes.
            "image:sunset.jpg[{caption} < x]",
            // The `icon:` spelling, in surrounding flow, and inside a
            // rendered span.
            "icon:{iconname}[{caption}]",
            "See image:sunset.jpg[{caption}] here.",
            "*image:sunset.jpg[{caption}]*",
            // An escape still keeps the macro literal.
            "\\image:sunset.jpg[{caption}]",
        ];

        let renderer = HtmlSubstitutionRenderer {};

        for fixture in fixtures {
            let folded = crate::content::inline_builder::fold_html(
                &build(Span::new(fixture), &parser, None),
                &renderer,
                &parser,
            );

            assert_eq!(
                folded,
                golden_normal(fixture, &parser),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn an_attribute_list_inside_an_expanded_value_is_owned_and_coarsely_located() {
        // The attribute values are the expansion's own bytes, owned rather
        // than borrowed from the temporary they were parsed from; the list's
        // span takes design §4.4's coarse fallback — here the whole enclosing
        // synthesized run — exactly as the node's `location` does.
        let parser = expanding_parser();
        let source = "image:sunset.jpg[{caption},role=hl]";
        let nodes = build(Span::new(source), &parser, None);

        assert_eq!(nodes.len(), 1);
        let image = assert_image(&nodes[0]);

        assert_eq!(image.alt.as_deref(), Some("Sunset"));

        let attrs = image.attrs.as_ref().unwrap();
        assert_eq!(attrs.nth_attribute(1).unwrap().value(), "Sunset");
        assert_eq!(attrs.named_attribute("role").unwrap().value(), "hl");

        // The location tag covers the bracket's own source, `{caption}`
        // included — the coarse fallback, not the parsed text.
        assert_eq!(attrs.span().data(), "{caption},role=hl");
        assert_eq!(image.location.data(), source);
    }

    #[test]
    fn matches_the_golden_pipelines_registration_for_images_inside_expanded_values() {
        // The staged `register_image` side effect reads the node's own stored
        // `target`, which is now the *expanded* one — so the catalog must match
        // the golden pipeline's, which registers what its own expanded haystack
        // matched. Two independent parsers, as elsewhere in this module.
        let fixtures = [
            "image:{logo}[Logo]",
            "image:{logo}[]",
            "image:{dir}/{logo}[]",
            "see {img-src} here",
            "image:{logo}[A] then image:other.png[B]",
        ];

        for fixture in fixtures {
            let builder_parser = expanding_parser().with_catalog_assets(true);
            let nodes = build(Span::new(fixture), &builder_parser, None);
            apply_image_side_effects(&nodes, &builder_parser, Span::new(fixture));

            let golden_parser = expanding_parser().with_catalog_assets(true);
            golden_normal(fixture, &golden_parser);

            let got: Vec<_> = builder_parser
                .catalog()
                .images()
                .iter()
                .map(|i| i.target.clone())
                .collect();

            let want: Vec<_> = golden_parser
                .catalog()
                .images()
                .iter()
                .map(|i| i.target.clone())
                .collect();

            assert_eq!(got, want, "registered images diverged for {fixture:?}");
        }
    }

    #[test]
    fn a_targetless_macro_yields_an_empty_target() {
        // `INLINE_IMAGE_MACRO`'s target group is optional, so `image:[…]`
        // matches with it absent and the node's target is the empty string
        // (its default alt deriving from that, exactly as `default_alt` does
        // for any other target).
        //
        // This is a structural test rather than a differential one on purpose:
        // the string pipeline's own `InlineImageMacroReplacer` reads that
        // group as `&caps[1]`, which **panics** for this shape, so there is no
        // golden to compare against. That panic is a pre-existing bug in the
        // shared string pipeline (it reproduces on `main`, through an ordinary
        // `Parser::parse`), independent of this module — so the builder's own
        // handling of the shape is pinned here and the fix belongs in its own
        // change against `main`.
        let nodes = build_src(Span::new("image:[Alt Text]"));

        assert_eq!(nodes.len(), 1);
        let image = assert_image(&nodes[0]);

        assert!(!image.is_icon);
        assert_eq!(image.target.as_ref(), "");
        assert_eq!(image.alt.as_deref(), Some("Alt Text"));
        assert_eq!(image.location.data(), "image:[Alt Text]");
    }
}
