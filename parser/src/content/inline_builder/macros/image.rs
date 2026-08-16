//! Image and icon macro recognition (`image:target[…]`, `icon:target[…]`).

use super::{MacroMatch, MacroMatchKind, rebuild_macro_level};
use crate::{
    Parser, Span,
    attributes::{Attrlist, AttrlistContext},
    content::{
        INLINE_IMAGE_MACRO, basename,
        inline_builder::quotes::{Piece, build_match_string, source_slice, text_slice},
        normalize_text_lf_escaped_bracket,
    },
    inlines::{Image, InlineNode},
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
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes);

    // Cheap pre-filter mirroring the string step's `found_macroish`: an image
    // or icon macro needs its name prefix and an opening bracket.
    if !((s.contains("image:") || s.contains("icon:")) && s.contains('[')) {
        return nodes;
    }

    let matches = find_image_matches(&s, &pieces, root, parser, &nodes);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every image/icon macro at this level, skipping any whose match crosses
/// an [`atomic`](Piece::atomic) piece (see
/// [`apply_macros`](super::apply_macros)) or whose attribute list
/// [`build_image_node`] cannot slice from `'src`.
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

        // A match crossing an escaped special or a rendered span is left for a
        // later increment; one crossing a `synthesized` run (an expanded
        // attribute value) is admitted here and gated more narrowly – on its
        // attribute list alone – inside `build_image_node`.
        if !range_is_verbatim_or_synthesized(pieces, &full) {
            continue;
        }

        if whole.as_str().starts_with('\\') {
            matches.push(MacroMatch {
                kind: MacroMatchKind::Unescape {
                    backslash: full.start,
                },
                full,
            });

            continue;
        }

        let Some(node) =
            build_image_node(&caps, whole.as_str(), &full, pieces, root, parser, nodes)
        else {
            // A non-empty attribute list crossing a synthesized run: an
            // `Attrlist<'src>` reads its own source span's bytes as content, so
            // there is nothing honest to parse it from. Left as literal source
            // for a later increment.
            continue;
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

/// Reports whether every piece overlapping the match-string range `range` is a
/// verbatim [`Text`](InlineNode::Text) run (non-atomic, non-synthesized). Only
/// then does the range map one-to-one onto contiguous source, so its captures
/// can slice `'src` directly. A [`synthesized`](Piece::synthesized) piece
/// (an attribute-expanded value, a `counter` directive) is rejected here too:
/// [`apply_character_replacements`](super::super::char_replacements::apply_character_replacements)
/// can recognize a construct inside one (it produces a leaf needing no `'src`
/// slice of its own – design §4.4's coarse fallback), but a macro node bakes
/// its target/attribute list straight from source, so it still needs a real
/// `'src` slice a synthesized run cannot provide – the same boundary an
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
/// expansion, or – reached at a tree's root – a filtered multi-line block's
/// own joined seed), rejecting only an [`atomic`](Piece::atomic) overlap (an
/// escaped special or a rendered span) – the boundary every macro family
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

/// Builds one [`Image`](InlineNode::Image) node from a recognized image/icon
/// match, pre-extracting the alt/width/height the way the string replacer does
/// so the fold reproduces the same bytes.
///
/// # What must be verbatim, and what need not be
///
/// The macro name and the target are read from the level's own **match
/// string** (`whole`, and [`text_slice`] for the target) rather than from a
/// source slice, so both are exact even when they come from a
/// [`synthesized`](Piece::synthesized) run – an expanded attribute value
/// (`image:{logo}[Logo]`), or a filtered multi-line block's own joined seed.
/// Only the node's `location` then takes design §4.4's coarse fallback.
///
/// The **attribute list** is the one part that cannot follow: an
/// [`Attrlist`]`<'src>` reads its own `Span<'src>`'s bytes *as content*, not
/// merely as a location tag, so it needs a real source slice. A non-empty
/// bracket crossing a synthesized run therefore returns `None` – the macro is
/// left literal for a later increment, exactly as the other boundaries in this
/// module are. An **empty** bracket (`image:{logo}[]`) needs no bytes at all
/// and parses from the same zero-length span an absent group already uses, so
/// it is recognized.
fn build_image_node<'src>(
    caps: &regex::Captures<'_>,
    whole: &str,
    full: &std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
    nodes: &[InlineNode<'src>],
) -> Option<InlineNode<'src>> {
    let location = source_slice(pieces, full.clone(), root);

    // The macro name (past any – here absent – escape) is `image:` or `icon:`.
    // It is read from the match string rather than `location.data()`, which is
    // the coarse enclosing span for a macro reached through a synthesized run.
    let is_icon = !whole.starts_with("image:");

    // Group 1 is the (optional) target; group 2 is the bracket text, which
    // always participates (it may be empty).
    let target = match caps.get(1) {
        None => CowStr::from(""),

        // The caller admitted no atomic piece in the match, so `text_slice`
        // always yields a value here – borrowed from `'src` for a verbatim
        // target, the expansion's own exact bytes for a synthesized one.
        Some(m) => text_slice(nodes, pieces, m.start()..m.end())?,
    };

    // Group 2 always participates – its own pattern carries an empty
    // alternative – so the degenerate fallback range here is unreachable, and
    // stands in for the same empty attribute list an absent group would mean
    // rather than adding a branch of its own that no input can take.
    let bracket_range = caps
        .get(2)
        .map_or(full.end..full.end, |m| m.start()..m.end());

    let bracket = if range_is_verbatim(pieces, &bracket_range) {
        source_slice(pieces, bracket_range, root)
    } else if bracket_range.is_empty() {
        // An empty attribute list carries no bytes to slice, so it parses from
        // a zero-length span wherever the macro sits.
        location.slice(0..0)
    } else {
        // A non-empty attribute list crossing a synthesized run: deferred (see
        // this function's own "what must be verbatim" note).
        return None;
    };

    let attrlist = Attrlist::parse(bracket, parser, AttrlistContext::Inline)
        .item
        .item;

    // The default alt text derives from the target's basename, with `_`/`-`
    // read as spaces – exactly the string replacer's `default_alt`.
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

    Some(InlineNode::Image(Image {
        is_icon,
        target,
        alt,
        width,
        height,
        attrs: Some(attrlist),
        location,
    }))
}

/// Performs the recognition side effects the string pipeline's own
/// `InlineImageMacroReplacer` attaches to an `image:`/`icon:` match –
/// registering the image target in the document's asset catalog (`image:`
/// only, and only when [`catalog_assets`](Parser::with_catalog_assets) is
/// enabled) and recording the `link=` dangerous-scheme/self-href warning –
/// by walking an already-built tree and reading each
/// [`Image`](InlineNode::Image) node's own stored fields instead of a regex
/// capture.
///
/// Every macro family this module recognizes defers exactly this kind of
/// side effect (see this file's own `register_image` note, and the anchor,
/// link, and footnote increments' own): while the additive builder runs
/// *alongside* the authoritative string pipeline – each against its own,
/// independent [`Parser`] – performing it from every additive pass would risk
/// double-counting a registration once the two paths ever share one `Parser`.
/// This function is that deferred piece, staged as its own building block for
/// the eventual cutover (design §5.2, Phase 4 step 6): re-attaching it for
/// real means calling it exactly once per parse, after the single-pass
/// builder replaces the recorder as `Content`'s tree source, so nothing here
/// is wired into a real parse yet – it is exercised only by this module's own
/// tests, against their own `Parser`.
///
/// Recurses into every container an `Image` node can be nested inside –
/// [`Styled`](InlineNode::Styled), [`Ref`](InlineNode::Ref), and
/// [`Footnote`](InlineNode::Footnote) children – mirroring exactly where
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
/// promotes into the anchor `href` (an icon has one only in image-icon mode –
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
        Parser, Span,
        content::inline_builder::{
            build, char_replacements::apply_character_replacements, macros::apply_macros,
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

        // The node captures its own attribute list – the property that makes it
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
        // `\image:…` drops the backslash and keeps the macro as literal text –
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
    fn a_macro_over_a_special_character_is_a_documented_divergence() {
        // The string pipeline matches macros over *escaped* text, so a target
        // containing `&` is matched as `a&amp;b.png`. A self-describing node
        // cannot carry that escaped text as an `'src` slice, so the single-pass
        // builder leaves such a macro *unrecognized* for a later increment (the
        // attribute-references step and the cutover). This is the documented
        // boundary of the additive image increment; the differential corpus
        // above deliberately excludes it.
        let nodes = apply_macros(
            build_through_special_and_replacements(Span::new("image:a&b.png[]")),
            Span::new("image:a&b.png[]"),
            &Parser::default(),
        );

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Image(_))),
            "a macro crossing an escaped special must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* build an image here.
        assert!(golden_macros("image:a&b.png[]").contains("<img"));
    }

    /// Builds the tree **through character replacements** (special characters,
    /// quotes, character replacements) – the state the macros step consumes –
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
        // registered in the asset catalog – mirroring
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
        // A `Ref`'s display children can hold an `Image` too – `apply_macros`
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
        // and nothing is rejected – see the companion
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
        // `self` (harmless) rather than the dangerous target – nothing is
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

    /// The real, public pipeline's output for `source` – the golden for the
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
        // still needs an honest `'src` slice – see the divergence test below –
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
            // A verbatim attribute list beside an expanded target, including
            // the positional width/height and a named attribute.
            "image:{logo}[Alt Text,200,100]",
            "image:{logo}[alt=Alt Text,width=200]",
            "image:{logo}[Logo,role=thumb]",
            // The whole macro arriving from an expanded value – recognized
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
        // span – design §4.4's documented split, the same one the anchor, UI,
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
        // from `location.data()` – which, for a wholly-expanded macro, is the
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
    fn an_attribute_list_inside_an_expanded_value_is_a_documented_divergence() {
        // A non-empty attribute list crossing a synthesized run is the one
        // part of an image macro that cannot follow the lift: an
        // `Attrlist<'src>` reads its own `Span<'src>`'s bytes *as content*, and
        // an expanded value has no source slice carrying them. The macro is
        // left literal rather than built with a wrong attribute list.
        //
        // If this boundary is ever lifted, fold these fixtures into the parity
        // corpus above.
        let parser = expanding_parser();

        for source in [
            "image:sunset.jpg[{caption}]",
            "image:{logo}[{caption},200]",
            // The whole macro arriving from an expanded value, this time with
            // a non-empty bracket – the empty-bracket spelling of the same
            // shape *is* recognized (see the parity corpus above).
            "see {img-src-alt} here",
        ] {
            let nodes = build(Span::new(source), &parser, None);

            assert!(
                nodes.iter().all(|n| !matches!(n, InlineNode::Image(_))),
                "an image whose attribute list crosses an expansion must stay literal: {nodes:?}"
            );

            assert!(
                golden_normal(source, &parser).contains("<img"),
                "the golden fixture stopped recognizing the image for {source:?}"
            );
        }
    }

    #[test]
    fn matches_the_golden_pipelines_registration_for_images_inside_expanded_values() {
        // The staged `register_image` side effect reads the node's own stored
        // `target`, which is now the *expanded* one – so the catalog must match
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
        // `Parser::parse`), independent of this module – so the builder's own
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
