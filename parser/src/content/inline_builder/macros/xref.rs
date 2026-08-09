//! Cross-reference recognition (`xref:id[…]`, `<<id>>`).

use super::{MacroMatch, MacroMatchKind, image::range_is_verbatim, rebuild_macro_level};
use crate::{
    Parser, Span,
    content::{
        INLINE_XREF,
        inline_builder::quotes::{Piece, build_match_string, source_slice},
        xref_target::{
            XrefTarget, interpret_xref_target, other_document_reference, this_document_reference,
        },
    },
    inlines::{InlineNode, Ref, RefVariant},
    parser::DerivedReference,
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
///   whole, and a target naming another document – or a file that was
///   included into this one in full, which is a reference within it after all
///   – carries a destination *derived* from the target itself, computed here
///   from the path attributes in effect at the reference (no catalog
///   consulted).
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

/// Matches `INLINE_XREF` at this level's escaped text, replacing each
/// recognized `xref:` macro with the [`Ref`](InlineNode::Ref)`{Xref}` node it
/// produces and leaving everything else in place.
pub(super) fn xref_macros_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    parser: &Parser,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes);

    // Cheap pre-filter: both the `xref:` macro form and the `<<id>>` shorthand
    // (seen here as `&lt;&lt;id&gt;&gt;`, since specials run before macros) are
    // recognized. The prefilter triggers on either the macro prefix or the
    // shorthand's `&lt;&lt;` opener, mirroring the string step's
    // `text.contains("&lt;&lt;") || (found_macroish && text.contains("xref:"))`
    // guard.
    if !s.contains("xref:") && !s.contains("&lt;&lt;") {
        return nodes;
    }

    let matches = find_xref_matches(&s, &pieces, root, parser);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every recognized cross-reference at this level – the `xref:` macro
/// form and the `<<id>>` shorthand – skipping any match that is not verbatim
/// enough to slice from `'src` or that this increment defers (see
/// [`build_xref_node`] and [`build_xref_shorthand_node`]).
fn find_xref_matches<'src>(
    s: &str,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Vec<MacroMatch<'src>> {
    let mut matches = Vec::new();

    for caps in INLINE_XREF.captures_iter(s) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();

        let full = whole.start()..whole.end();

        // The `xref:` macro (group 3) and the `<<…>>` shorthand (group 2) differ
        // in what must be verbatim to build a node. The macro's whole match is
        // sliced from `'src`, so all of it must be verbatim. The shorthand's
        // `&lt;&lt;` / `&gt;&gt;` delimiters are always `CharRef`s that the node
        // *consumes* rather than slices, so only its inner text (group 2) – the
        // id and any reference text – need be verbatim.
        let shorthand_inner = caps.get(2).map(|inner| inner.start()..inner.end());

        let verbatim = match &shorthand_inner {
            Some(inner) => range_is_verbatim(pieces, inner),
            None => range_is_verbatim(pieces, &full),
        };

        // An escape (`\xref:` / `\<<`) is honored by dropping the backslash and
        // keeping the rest literal, mirroring the string replacer's leading
        // `caps.get(1)` check. For the shorthand the unescape runs even when the
        // inner is not verbatim, because [`rebuild_macro_level`] emits the
        // `CharRef` delimiters (and any rendered span between them) whole; the
        // macro form keeps its established verbatim-first order.
        if whole.as_str().starts_with('\\') {
            if shorthand_inner.is_some() || verbatim {
                matches.push(MacroMatch {
                    kind: MacroMatchKind::Unescape {
                        backslash: full.start,
                    },
                    full,
                });
            }

            continue;
        }

        if !verbatim {
            continue;
        }

        let node = match &shorthand_inner {
            Some(inner) => build_xref_shorthand_node(inner.clone(), &full, pieces, root, parser),
            None => build_xref_node(&caps, &full, pieces, root, parser),
        };

        match node {
            Some(node) => matches.push(MacroMatch {
                kind: MacroMatchKind::Node {
                    consumed: full.clone(),
                    node: Box::new(node),
                },
                full,
            }),

            // A form this increment defers (an attribute-list-in-text macro or
            // a degenerate shorthand – see the two builders) is left as
            // literal source for a later increment.
            None => continue,
        }
    }

    matches
}

/// Builds one [`Ref`](InlineNode::Ref)`{Xref}` node from a verbatim `xref:`
/// macro match, computing the target and display text exactly as the string
/// replacer does so the fold reproduces the same bytes. Returns `None` for a
/// form this increment defers.
///
/// The scope this builder claims is every macro-form target *except* a text
/// carrying an attribute list; the `<<id>>` shorthand is built by
/// [`build_xref_shorthand_node`]. A same-document reference to a specific id
/// (`xref:install[]`) resolves through the catalog later (`derived: None`);
/// the empty target (`xref:#[]`), a target naming another document
/// (`xref:other.adoc#frag[]`), and a target naming this document (or a file
/// included into it in full) all carry a destination *derived* from the
/// target itself, computed by [`xref_target_and_derived`] exactly as the
/// string replacer computes it – so this builder no longer defers any target
/// shape. One form remains deferred to a later increment:
///
/// - a **text carrying an attribute list** (an `=`, for `window`/`role`/
///   `xrefstyle`): it is parsed as an [`Attrlist`](crate::attributes::Attrlist)
///   the node cannot hold yet, exactly as
///   [`build_link_node`](super::links::build_link_node) defers the analogous
///   link form.
///
/// The display text becomes the node's children as a single
/// [`Text`](InlineNode::Text), so the fold recovers the provided text by
/// folding the children and needs no build-time state; an empty text yields no
/// children, which the fold reads as "no text provided" (the bracketed `[id]`
/// fallback).
///
/// As in the additive builder generally, this performs *no* recognition side
/// effect – notably it does **not** register the reference for resolution,
/// which the string replacer does by recording a deferred `XrefSegment`; the
/// cutover (design §5.2 Phase 4, step 6) wires resolution to the tree.
fn build_xref_node<'src>(
    caps: &regex::Captures<'_>,
    full: &std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Option<InlineNode<'src>> {
    // Group 3 is the `xref:` macro target; when it is absent the match is the
    // shorthand form, which this increment defers.
    let raw_target = caps.get(3)?.as_str();

    let (target, derived) = xref_target_and_derived(raw_target, true, parser);

    let raw_text = caps.get(4).map_or("", |m| m.as_str());

    // A text carrying an attribute list (an `=`) is parsed from a
    // newline-normalized copy of the text into named attributes (`window`,
    // `role`, `xrefstyle`); it cannot be carried on the node yet, so defer the
    // whole macro, mirroring the string replacer's `raw_text.contains('=')`
    // branch and [`build_link_node`](super::links::build_link_node).
    if raw_text.contains('=') {
        return None;
    }

    let location = source_slice(pieces, full.clone(), root);

    let children = if raw_text.is_empty() {
        vec![]
    } else {
        // The provided text becomes the node's children, located at the
        // bracketed text.
        #[allow(clippy::unwrap_used)]
        let text_span = caps.get(4).unwrap();

        let text_location = source_slice(pieces, text_span.start()..text_span.end(), root);

        // An escaped bracket (`\]`) makes the logical text a computed (owned)
        // value – a *synthesized* `Text` whose value need not coincide with its
        // source, mirroring the string replacer's `raw_text.replace`. Without
        // one the text is verbatim, so it borrows the very bytes its location
        // covers (the builder's `'src`-borrowing goal).
        let value = if raw_text.contains("\\]") {
            CowStr::from(raw_text.replace("\\]", "]"))
        } else {
            CowStr::from(text_location.data())
        };

        vec![InlineNode::Text {
            value,
            location: text_location,
        }]
    };

    Some(InlineNode::Ref(Ref {
        variant: RefVariant::Xref,
        target: CowStr::from(target),
        children,
        roles: vec![],
        window: None,
        resolved: None,
        derived,
        location,
    }))
}

/// Builds one [`Ref`](InlineNode::Ref)`{Xref}` node from a `<<id>>` shorthand
/// cross-reference, computing the target and display text exactly as the string
/// replacer's shorthand branch does so the fold reproduces the same bytes.
/// Returns `None` for a form this increment defers.
///
/// `inner` is the shorthand's inner text (`INLINE_XREF` group 2) in
/// match-string coordinates; the caller guarantees it is verbatim, so its
/// match-string bytes coincide with source. It is split on the first `,` into
/// an id and an optional reference text, each trimmed – mirroring the string
/// replacer's `inner.split_once(',')` with `id.trim()` / `text.trim()`. The
/// reference text becomes the node's single [`Text`](InlineNode::Text) child
/// (an empty text yields no children, which the fold reads as "no text
/// provided" – the bracketed `[id]` fallback), and the whole `<<…>>` – its
/// `CharRef` delimiters included – is the node's `location`.
///
/// The scope this builder claims is every shorthand target *except* one whose
/// reference text is present but empty. A same-document shorthand
/// (`<<install>>`) resolves through the catalog later (`derived: None`); an
/// inter-document shorthand (`<<other#frag>>`) and the document-as-a-whole
/// shorthand (`<<>>`, an empty id) both carry a destination *derived* from the
/// target itself, computed by [`xref_target_and_derived`] exactly as the macro
/// form's – so this builder no longer defers any target shape. One form
/// remains deferred, left as literal source for a later increment and pinned
/// by a divergence test:
///
/// - a **`<<id,>>` with an empty reference text**: the string replacer records
///   this as a *present-but-empty* text (rendering an empty `<a>…</a>`), which
///   an empty child vector cannot distinguish from "no text provided" – so the
///   whole shorthand is deferred rather than rendered with the wrong fallback.
///
/// A shorthand whose id already carries a rendered `<` (an earlier-substituted
/// macro, e.g. `<<link:https://example.com[], Example>>`) – which the string
/// replacer leaves untouched – cannot reach here at all: the `<` is a
/// `CharRef`, so the inner is not verbatim and the caller never calls this
/// builder.
///
/// As in the additive builder generally, this performs *no* recognition side
/// effect – notably it does **not** register the reference for resolution; the
/// cutover (design §5.2 Phase 4, step 6) wires resolution to the tree.
fn build_xref_shorthand_node<'src>(
    inner: std::ops::Range<usize>,
    full: &std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
    parser: &Parser,
) -> Option<InlineNode<'src>> {
    // The inner is verbatim (the caller checked), so its source slice's bytes
    // coincide with the match string's – a byte offset within `inner_data` maps
    // to a match-string offset by adding `inner.start`.
    let inner_span = source_slice(pieces, inner.clone(), root);
    let inner_data = inner_span.data();

    // Split an optional ", reference text" off the id at the first comma.
    let comma = inner_data.find(',');

    let raw_id = match comma {
        Some(index) => &inner_data[..index],
        None => inner_data,
    };

    let (target, derived) = xref_target_and_derived(raw_id.trim(), false, parser);

    let location = source_slice(pieces, full.clone(), root);

    let children = match comma {
        None => vec![],

        Some(index) => {
            let raw_text = &inner_data[index + 1..];
            let trimmed = raw_text.trim();

            // A `<<id,>>` with an empty (or whitespace-only) reference text is a
            // present-but-empty text the node cannot represent (see the doc
            // comment); defer the whole shorthand.
            if trimmed.is_empty() {
                return None;
            }

            // Locate the trimmed reference text at its source. It is verbatim, so
            // the `Text` child borrows the very bytes its location covers.
            let lead = raw_text.len() - raw_text.trim_start().len();

            let text_start = inner.start + index + 1 + lead;
            let text_location = source_slice(pieces, text_start..text_start + trimmed.len(), root);

            vec![InlineNode::Text {
                value: CowStr::from(text_location.data()),
                location: text_location,
            }]
        }
    };

    Some(InlineNode::Ref(Ref {
        variant: RefVariant::Xref,
        target: CowStr::from(target),
        children,
        roles: vec![],
        window: None,
        resolved: None,
        derived,
        location,
    }))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use super::super::super::test_support::{
        assert_styled, assert_text, build_src, fold_html, link_text_of,
    };
    use crate::{
        Parser, Span,
        content::{Content, SubstitutionStep},
        inlines::{InlineNode, Ref, RefVariant, SpanForm, StyleVariant},
        parser::HtmlSubstitutionRenderer,
    };

    /// The string pipeline's output through the **macros** step for `source`,
    /// with any deferred cross-references finalized to their unresolved
    /// fallback. Unlike [`golden_macros`], the macros step defers a
    /// cross-reference to a placeholder rather than rendering it, so the
    /// placeholder must be finalized – no catalog resolution runs, so the
    /// result is the unresolved-fallback rendering the additive builder's
    /// fold (always unresolved) must reproduce.
    fn golden_xref_with(source: &str, parser: &Parser) -> String {
        let mut content = Content::from(Span::new(source));
        SubstitutionStep::SpecialCharacters.apply(&mut content, parser, None);
        SubstitutionStep::Quotes.apply(&mut content, parser, None);
        SubstitutionStep::CharacterReplacements.apply(&mut content, parser, None);
        SubstitutionStep::Macros.apply(&mut content, parser, None);
        SubstitutionStep::PostReplacement.apply(&mut content, parser, None);

        // Finalize as the real pipeline does after the last step, capturing the
        // placeholder template and rebuilding the unresolved fallback.
        content.finalize_deferred(&HtmlSubstitutionRenderer {});
        content.rendered_str().to_string()
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
        // reproduces the string pipeline's output byte-for-byte. This is the
        // differential corpus (design §5.3) that pins the cross-reference
        // increment. Every fixture is a *verbatim* cross-reference in either
        // spelling, whether it resolves through the catalog (same-document) or
        // through a target-derived destination (inter-document, or the
        // document-as-a-whole form) – the boundary this increment claims (an
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
            // An inter-document target – with and without a fragment, and a
            // non-AsciiDoc extension kept as-is – and the document-as-a-whole
            // form (an empty target).
            "xref:other.adoc#frag[Elsewhere]",
            "xref:other.adoc[]",
            "xref:refcard.pdf[Reference Card]",
            "xref:#[]",
            // An escaped `]` inside the text is unescaped.
            "xref:foo[a\\]b]",
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
            // An inter-document shorthand – with and without a fragment – and
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
        ];

        let renderer = HtmlSubstitutionRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build_src(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden_xref(fixture),
                "fold diverged from the string pipeline for {fixture:?}"
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
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_xref("xref:install[]")
        );
    }

    #[test]
    fn an_xref_display_text_is_located_at_its_source() {
        // The display text's `Text` child locates at the bracketed text, not the
        // whole macro.
        let nodes = build_src(Span::new("xref:install[Installation]"));

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.children.len(), 1);
        assert_text(&reference.children[0], "Installation", 1, 14);
    }

    #[test]
    fn an_explicit_same_document_xref_stores_the_interpreted_id() {
        // `xref:#install[]` uses the explicit-`#` same-document form. The node's
        // `target` is the *interpreted* id (`install`), not the raw `#install`:
        // it is the value the renderer builds the `href` from and resolution
        // keys on, matching the string pipeline and the recorder tree (see the
        // `Ref::target` field docs). Storing `#install` would fold to
        // `href="##install"` and break parity.
        let nodes = build_src(Span::new("xref:#install[Install]"));

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "install");
        assert_eq!(link_text_of(reference), "Install");

        // The fold reproduces the string pipeline's `href="#install"` exactly.
        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});
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
        // `\xref:…` drops the backslash and keeps the macro as literal text – no
        // reference node – exactly as the string replacer's escape branch does.
        let source = "\\xref:install[Installation]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an escaped xref must not produce a reference node: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_xref(source)
        );
    }

    #[test]
    fn an_xref_shorthand_becomes_a_ref_node() {
        // The `<<id,text>>` shorthand builds the same `Ref{Xref}` node the
        // `xref:` macro does, even though its `&lt;&lt;` / `&gt;&gt;` delimiters
        // are `CharRef`s: the node consumes them and slices its verbatim inner.
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
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_xref("<<install>>")
        );
    }

    #[test]
    fn an_xref_shorthand_display_text_is_located_at_its_trimmed_source() {
        // The reference text's `Text` child locates at the *trimmed* text within
        // the shorthand, not at the whole shorthand and not including the
        // surrounding whitespace the string replacer trims.
        let nodes = build_src(Span::new("<<install, Install Now >>"));

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "install");
        assert_eq!(reference.children.len(), 1);

        // `<<install, ` is 11 characters, so the text starts at column 12.
        assert_text(&reference.children[0], "Install Now", 1, 12);
    }

    #[test]
    fn an_xref_shorthand_is_recognized_inside_a_span() {
        // A shorthand can appear inside a rendered span; the transducer descends
        // into the span body and builds the node there.
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
        // `\<<id>>` drops the backslash and keeps the shorthand as literal text –
        // no reference node – exactly as the string replacer's escape branch
        // does. Its delimiters are non-verbatim `CharRef`s, so this also exercises
        // the escape path that does not require a verbatim inner.
        let source = "\\<<install,Install Now>>";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an escaped shorthand must not produce a reference node: {nodes:?}"
        );

        assert_eq!(
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
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
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_xref(source)
        );
    }

    #[test]
    fn an_empty_xref_shorthand_becomes_a_ref_node() {
        // `<<>>` names the document as a whole: an empty id that resolves through
        // a *derived* destination computed from the document's own attributes,
        // exactly as the empty `xref:#[]` macro form does.
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
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_xref(source)
        );
    }

    #[test]
    fn an_xref_shorthand_with_empty_text_is_a_documented_divergence() {
        // `<<id,>>` records a *present-but-empty* reference text: the string
        // replacer renders an empty `<a href="#id"></a>`, whereas an empty child
        // vector is indistinguishable from "no text provided" (the `[id]`
        // fallback). The builder cannot represent the distinction, so it defers
        // the whole shorthand (left literal).
        let source = "<<install,>>";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "a shorthand with an empty text must be left unrecognized: {nodes:?}"
        );

        // The string pipeline builds an anchor with an empty body, which the
        // bracketed fallback the builder would produce does not match.
        assert!(golden_xref(source).contains(r##"href="#install">"##));
        assert!(!golden_xref(source).contains("[install]"));
    }

    #[test]
    fn an_xref_shorthand_over_a_rendered_span_is_a_documented_divergence() {
        // A shorthand whose reference text is a rendered span (`<<x,*bold*>>`) has
        // a non-verbatim inner – the span is opaque – so the builder cannot slice
        // its text from `'src` and leaves the shorthand unrecognized, exactly as
        // it defers a macro crossing a rendered span.
        let source = "<<x,*bold*>>";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "a shorthand crossing a rendered span must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* build a reference here.
        assert!(golden_xref(source).contains("<a href"));
    }

    #[test]
    fn an_inter_document_xref_becomes_a_ref_node() {
        // An inter-document target (`other.adoc#frag`) carries a *derived*
        // destination computed from the target itself – the AsciiDoc extension
        // stripped, the output suffix substituted in – mirroring the string
        // replacer's own target interpretation exactly.
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

        let folded = fold_html(&nodes, &HtmlSubstitutionRenderer {});
        assert!(
            folded.contains(r#"href="other.html#frag""#),
            "folded: {folded}"
        );
        assert_eq!(folded, golden_xref(source));
    }

    #[test]
    fn an_xref_over_a_special_character_is_a_documented_divergence() {
        // A cross-reference whose text contains `<` is matched by the string
        // pipeline over the *escaped* text (`xref:foo[a&lt;b]`). A self-describing
        // node cannot carry that escaped text as an `'src` slice, so the
        // single-pass builder leaves such a macro *unrecognized* for a later
        // increment, exactly as the image and auto-link increments defer a macro
        // crossing a special character.
        let source = "xref:foo[a<b]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an xref crossing an escaped special must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* build a reference here.
        assert!(golden_xref(source).contains("<a href"));
    }

    #[test]
    fn an_xref_attribute_list_is_a_documented_divergence() {
        // An `xref:` text carrying an `=` splits into an attribute list (here a
        // role), which the string replacer parses from a newline-normalized copy
        // of the text – not from `'src`. The builder cannot carry that as an
        // `Attrlist<'src>` yet, so it defers the whole macro (left literal).
        let source = "xref:install[Installation,role=hl]";
        let nodes = build_src(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ref(_))),
            "an attribute-list-in-text xref must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, applies the role.
        assert!(golden_xref(source).contains(r#"class="hl""#));
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
            fold_html(&nodes, &HtmlSubstitutionRenderer {}),
            golden_xref(source)
        );
    }

    #[test]
    fn a_this_document_xref_target_is_treated_as_same_document() {
        // A target naming *this* document by its own `docname` (or a file
        // included into it in full) is a reference within it after all: the
        // element it names is in the catalog being built right now, so the node
        // carries the same-document target (the fragment) with no derived
        // destination – exactly as an explicit `#id` shorthand does.
        let parser = Parser::default().with_primary_file_name("mydoc.adoc");

        let source = "xref:mydoc.adoc#install[Install]";
        let root = Span::new(source);
        let nodes = super::super::super::build(root, &parser);

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "install");
        assert_eq!(reference.derived, None);

        let folded = super::super::super::fold_html(&nodes, &HtmlSubstitutionRenderer {}, &parser);
        assert!(folded.contains(r##"href="#install""##), "folded: {folded}");
        assert_eq!(folded, golden_xref_with(source, &parser));
    }

    #[test]
    fn a_fragmentless_this_document_xref_target_is_document_as_a_whole() {
        // A target naming *this* document with no fragment (`xref:mydoc.adoc[]`)
        // is, like the empty target (`xref:#[]`), a reference to the document as
        // a whole: the same `this_document_reference` derived destination, not a
        // same-document id to resolve through the catalog.
        let parser = Parser::default().with_primary_file_name("mydoc.adoc");

        let source = "xref:mydoc.adoc[Home]";
        let root = Span::new(source);
        let nodes = super::super::super::build(root, &parser);

        let reference = assert_xref(&nodes[0]);
        assert_eq!(reference.target.as_ref(), "");

        #[allow(clippy::expect_used)]
        let derived = reference
            .derived
            .as_ref()
            .expect("a fragmentless self-reference carries a derived destination");
        assert_eq!(derived.href, "#");

        let folded = super::super::super::fold_html(&nodes, &HtmlSubstitutionRenderer {}, &parser);
        assert_eq!(folded, golden_xref_with(source, &parser));
    }
}
