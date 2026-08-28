//! A **whole-document** differential harness for the single-pass builder.
//!
//! The builder's own corpora
//! ([`inline_builder`](crate::content::inline_builder)'s test module) drive
//! [`SubstitutionGroup::apply`](crate::content::SubstitutionGroup)
//! on a bare [`Content`](crate::content::Content), which has no document
//! catalog and so **cannot resolve cross-references**. (The whole-document
//! sweep that used to sit beside this one drove the *Strategy-A recorder* and
//! retired with it.)
//!
//! That leaves one thing unverified, and it is the thing the
//! cutover rests on: **the tree the single-pass builder produces, folded
//! *after* cross-reference resolution has run, reproduces the rendered string
//! byte-for-byte.** Design §4.2 retires the deferred-cross-reference sentinel
//! system on exactly that basis — an `xref` is a
//! [`Ref`](crate::inlines::Ref)`{resolved: None}` node that resolution fills in
//! place, "non-destructive by construction, re-resolvable, no template" — and
//! §4.3 extends the same claim to section titles (whose own document-order
//! pass mutates the title's tree) and to footnote-embedded references (which
//! live in a footnote's subtree).
//!
//! This harness checks all three, over whole documents, in one parse: every
//! content location carries *both* its rendered string and its tree, so the two
//! are compared directly rather than reconstructed from a second parse.

use std::rc::Rc;

use crate::{
    Parser,
    blocks::{Block, FindBlocks, IsBlock, TableCellContent, TableRow},
    content::inline_builder::fold_html,
    inlines::InlineNode,
    parser::{HtmlSubstitutionRenderer, ModificationContext},
};

/// One content location: what the string pipeline rendered, and the tree the
/// builder produced for the same content.
struct Location<'a, 'src> {
    what: String,
    rendered: String,
    inlines: &'a [InlineNode<'src>],
}

/// Collects every content-bearing location in `doc` that carries a tree, in a
/// fixed document order.
fn locations<'src>(doc: &'src crate::Document<'src>) -> Vec<Location<'src, 'src>> {
    fn cells<'src>(row: &'src TableRow<'src>, out: &mut Vec<Location<'src, 'src>>) {
        for cell in row.cells() {
            // Only inline (`Simple`) cells carry a single `Content`; an
            // `AsciiDoc` cell is a nested standalone document and is out of
            // scope for this harness.
            if let TableCellContent::Simple(content) = cell.content() {
                out.push(Location {
                    what: "table cell".to_string(),
                    rendered: content.rendered_html().to_string(),
                    inlines: content.inlines(),
                });
            }
        }
    }

    fn walk<'src>(block: &'src Block<'src>, out: &mut Vec<Location<'src, 'src>>) {
        // Every block kind that directly carries content, taken through the
        // one pair of accessors that answers for all of them
        // ([`IsBlock::rendered_html_content`] and [`IsBlock::inlines`], the
        // structured counterpart of the first — "the same blocks carry each").
        // Reading them rather than matching per variant is what keeps this
        // walk from silently narrowing: a block kind added later, or one this
        // sweep's author did not think of (a quote, an admonition, a raw
        // delimited block, a list item), is covered because it implements the
        // trait, not because it was named here.
        if let (Some(rendered), Some(inlines)) = (block.rendered_html_content(), block.inlines()) {
            out.push(Location {
                what: block.raw_context().to_string(),
                rendered: rendered.to_string(),
                inlines,
            });
        }

        // A **block title** (`.Title`) is substituted content in its own
        // right and carries its own tree, but it is not the block's content,
        // so neither accessor above reaches it. Every block kind that can
        // carry one is named by `block_title_content`, which is the mutable
        // accessor the document-order title pass already uses, read-only.
        if let Some(title) = block.block_title_content() {
            out.push(Location {
                what: "block title".to_string(),
                rendered: title.rendered_html().to_string(),
                inlines: title.inlines(),
            });
        }

        // The two remaining locations that are *not* a block's own content,
        // and so are reached by none of the accessors above.
        match block {
            // A section's title: `SectionBlock` implements neither accessor
            // above (a section's own content is its children), so its title —
            // whose tree the document-order pass in `title_refs` mutates — is
            // reached only this way.
            Block::Section(section) => out.push(Location {
                what: "section title".to_string(),
                rendered: section.section_title().to_string(),
                inlines: section.section_title_inlines(),
            }),

            // A table's cells are not blocks at all.
            Block::Table(table) => {
                if let Some(header) = table.header_row() {
                    cells(header, out);
                }

                for row in table.body_rows() {
                    cells(row, out);
                }

                if let Some(footer) = table.footer_row() {
                    cells(footer, out);
                }
            }

            _ => {}
        }

        for child in block.child_blocks() {
            walk(child, out);
        }
    }

    let mut out = vec![];

    for block in doc.child_blocks() {
        walk(block, &mut out);
    }

    out
}

/// Every [`Footnote`](InlineNode::Footnote) node in `nodes`, in document order,
/// recursing into every container one can be nested inside.
fn footnote_subtrees<'a, 'src>(
    nodes: &'a [InlineNode<'src>],
    out: &mut Vec<&'a [InlineNode<'src>]>,
) {
    for node in nodes {
        match node {
            InlineNode::Footnote(footnote) => {
                // A bare reference (`footnote:id[]`) defines nothing and keeps
                // the empty subtree its node type documents; the catalog has
                // no text for it either.
                if !footnote.is_reference {
                    out.push(&footnote.children);
                }

                footnote_subtrees(&footnote.children, out);
            }

            InlineNode::Styled(styled) => footnote_subtrees(&styled.children, out),
            InlineNode::Ref(reference) => footnote_subtrees(&reference.children, out),
            InlineNode::IndexTerm(index_term) => footnote_subtrees(&index_term.children, out),

            _ => {}
        }
    }
}

/// A parser with `experimental` set, so the UI macros a fixture may carry are
/// recognized by both sides alike, and with tree building on.
fn parser() -> Parser {
    Parser::default().with_intrinsic_attribute_bool(
        "experimental",
        true,
        ModificationContext::Anywhere,
    )
}

/// Parses `source` once and asserts that folding each content location's tree
/// reproduces that location's own `rendered_html()` string — cross-references
/// resolved, section titles included, footnote texts included.
fn check_document(source: &str) -> Vec<String> {
    let mut parser = parser();
    let doc = parser.parse(source);

    let renderer = HtmlSubstitutionRenderer {};

    // The fold's `&Parser` supplies document render context (an image's safe
    // mode, `data-uri`, `icons`); a default one matches these fixtures, none of
    // which sets an attribute the image renderer reads.
    let fold_parser = Parser::default();

    let locations = locations(&doc);

    // Guard against a fixture that exercises nothing — and against the
    // subtler vacuity of a location whose tree is empty, which would fold to
    // the empty string and match an empty rendering.
    assert!(
        locations.iter().any(|l| !l.inlines.is_empty()),
        "document fixture folded no content for {source:?}"
    );

    for location in &locations {
        assert_eq!(
            fold_html(location.inlines, &renderer, &fold_parser.render_context()),
            location.rendered,
            "the {} fold diverged from its rendered string for {source:?}",
            location.what
        );
    }

    // A footnote's text is extracted out of the block it was written in, so it
    // reaches no `rendered_html()` string above; it is nonetheless substituted
    // inline content, and the source of a footnote node's subtree, so it folds
    // under the same invariant. The catalog's entries are in document order,
    // and so is this walk.
    let mut subtrees = vec![];

    for location in &locations {
        footnote_subtrees(location.inlines, &mut subtrees);
    }

    let catalog = doc.catalog();

    let texts: Vec<String> = catalog
        .footnotes()
        .iter()
        .map(|footnote| footnote.text.clone())
        .collect();

    let kinds = locations.iter().map(|l| l.what.clone()).collect();

    assert_eq!(
        subtrees.len(),
        texts.len(),
        "footnote subtree count differs from the catalog's for {source:?}"
    );

    for (subtree, text) in subtrees.iter().zip(texts.iter()) {
        assert_eq!(
            &fold_html(subtree, &renderer, &fold_parser.render_context()),
            text,
            "a footnote's subtree fold diverged from its registered text for {source:?}"
        );
    }

    kinds
}

#[test]
fn fold_matches_the_rendered_string_after_resolution() {
    // The corpus this harness exists for: documents whose cross-references
    // actually resolve, so the fold is exercised *after* the resolution pass
    // has mutated the tree in place.
    let mut kinds: Vec<String> = vec![];

    for source in [
        // Resolved cross-references, both spellings, in block content.
        "[[tgt]]The target paragraph.\n\nSee <<tgt>> for details.",
        "[[tgt]]Target.\n\nSee <<tgt,the target>> now.",
        "[[tgt]]Target.\n\nSee xref:tgt[the target] now.",
        // Several references, one unresolved, so both branches of the fold's
        // `resolved` handling are reached in one document.
        "[[a]]First.\n\n[[b]]Second.\n\nSee <<a>>, <<b>>, and <<missing>>.",
        // A reference resolving to a *section*, whose destination text is the
        // heading's own reference text — the title pass's own output.
        "== Install\n\nSee <<_install>> for details.",
        "[#setup]\n== Set Up\n\nSee <<setup>> and <<setup,the setup>>.",
        // Forward and circular references between headings, which is why the
        // title pass exists at all.
        "[#one]\n== See <<two>>\n\nBody.\n\n[#two]\n== See <<one>>\n\nBody.",
        // A reference inside a rendered span, and one inside a formatted
        // heading — resolution has to reach into the span's children.
        "[[tgt]]Target.\n\nSee *the <<tgt>> here*.",
        "[[tgt]]Target.\n\n== A *bold* <<tgt>> heading\n\nBody.",
        // Footnote-embedded cross-references, resolved and unresolved: the
        // reference lives in the footnote's own subtree.
        "[[tgt]]The target.\n\nA claim.footnote:[see <<tgt>> for details]",
        "A claim.footnote:[see <<missing>>]",
        "[[a]]A.\n\n[[b]]B.\n\nSee <<a>>.footnote:[and also <<b>>] and <<b>>.",
        // A footnote in a heading, beside a reference to that heading.
        "[#sect]\n== Section footnote:[a note]\n\nSee <<sect>>.",
        // Document-wide `xrefstyle`, which only shows on a reference whose
        // target carries a signifier (hence `:sectnums:`). The effective style
        // is a *document-order* fact — a `:xrefstyle:` line rebinds it for
        // everything after it — so these are what pin that the node carries the
        // style in effect where the reference was written rather than the fold
        // asking the document for it. Both spellings, and both the set and the
        // rebound state.
        ":sectnums:\n:xrefstyle: full\n\n== Install\n\nSee <<_install>> and xref:_install[].",
        ":sectnums:\n\n== Install\n\nSee <<_install>>.\n\n:xrefstyle: full\n\nAgain <<_install>>.",
        ":sectnums:\n:xrefstyle: full\n\n== Install\n\nSee <<_install>>.\n\n:xrefstyle: short\n\nAgain <<_install>>.",
        // A macro-level `xrefstyle=` beside a document-wide one it overrides.
        ":sectnums:\n:xrefstyle: full\n\n== Install\n\nSee xref:_install[xrefstyle=short].",
        // A footnote nested in a child that falls between two of its level's
        // own: the catalog's footnote list is in document order, so this
        // fixture fails on the *subtree-to-text* pairing — not only on the
        // fold — if the numbering order slips.
        "before footnote:[a] *span footnote:[b]* after footnote:[c]",
        "before footnote:[a] ((a term footnote:[b])) after footnote:[c]",
        // Footnote texts carrying their own inline constructs.
        "A claim.footnote:[the *strong* and _soft_ evidence]",
        "A claim.footnote:[see link:https://example.org[the source]]",
        // A named footnote defined once and referenced again: the reference
        // defines nothing, so it contributes no subtree.
        "Named.footnote:disc[a *discussion*] then footnote:disc[].",
        // Table cells, which carry their own `Content` and their own trees.
        "|===\n|A *bold* cell |A `code` cell\n|===",
        // Block titles, which carry their own tree — including one whose own
        // cross-reference the document-order title pass resolves, on each of
        // the block kinds that can carry one.
        "[[tgt]]Target.\n\n.A *bold* title with <<tgt>>\nA paragraph.",
        "[[tgt]]Target.\n\n.A title with <<tgt>>\n|===\n|cell\n|===",
        "[[tgt]]Target.\n\n.A title with <<tgt>>\n====\nAn example.\n====",
        "[[tgt]]Target.\n\n.A title with <<tgt>>\n----\nlisting\n----",
        "[[tgt]]Target.\n\n.A title with <<tgt>>\n* An item",
        "[[tgt]]Target.\n\n.A title with <<tgt>>\nimage::pic.png[Alt]",
        "[[tgt]]Target.\n\n.A title with <<tgt>>\n[NOTE]\n====\nnote body\n====",
        "[[tgt]]Target.\n\n.A title with <<tgt>>\n[quote]\n____\nquoted\n____",
        "[[tgt]]Target.\n\n.A title with <<tgt>>\n'''",
        "[[tgt]]Target.\n\n.A title with <<tgt>>\ntoc::[]",
        "[[tgt]]Target.\n\n|===\n|See <<tgt>> here\n|===",
        // Nested blocks, so the walk's recursion is exercised.
        "====\nAn example with *bold*.\n\nAnd a second para.\n====",
        "* A *list* item\n* Another with <<tgt>>\n\n[[tgt]]Target.",
        // The other content-bearing block kinds the walk reaches through
        // `IsBlock` rather than by name: a quote, an admonition, the raw
        // delimited blocks (whose verbatim groups run a *different* step
        // selection through `build_for_group`), a media block's own alt text,
        // and a list's principal text beside its attached blocks.
        "[quote,Author]\n____\nA quote with *bold* and <<tgt>>.\n____\n\n[[tgt]]Target.",
        "____\nA bare quote with `code` and an image:q.png[Q].\n____",
        "NOTE: An admonition with *bold* and a link:n.html[note].",
        "[NOTE]\n====\nA block admonition with <<tgt>>.\n====\n\n[[tgt]]Target.",
        "----\nlisting with a < b and *no* quotes\n----",
        "....\nliteral with a < b and *no* quotes\n....",
        "[source,rust]\n----\nlet x = a < b; // <1>\n----\n<1> A callout.",
        "++++\n<p>passthrough with a < b</p>\n++++",
        // A media block carries a tree, but an empty one — its alt text lives
        // in the attribute list, not in inline content — so it rides beside a
        // paragraph rather than standing alone, which the non-vacuity guard
        // would (rightly) reject.
        "image::pic.png[An *alt* text]\n\nA *para* beside it.",
        "* Principal *text*\n+\nAn attached para with <<tgt>>.\n\n[[tgt]]Target.",
        "term:: A *definition* with <<tgt>>.\n\n[[tgt]]Target.",
        // Plain documents, so the harness is not only about references.
        "First *para* here.\n\nSecond _para_ with `code` and (C).",
        "== Heading\n\nBody with an image:x.png[Alt] and a kbd:[Ctrl,T].",
    ] {
        kinds.extend(check_document(source));
    }

    // The walk reads `IsBlock` rather than matching per variant, which is what
    // keeps it from silently narrowing. Pin the whole set of location kinds
    // the corpus reaches, rather than a hand-picked few: a walk that stopped
    // visiting a kind, or a fixture that stopped producing one, shrinks this
    // set and fails here instead of passing over a location nobody looked at.
    //
    // The four beyond `paragraph` / `section title` / `table cell` are what
    // reading the trait added — the raw-delimited family (`listing`,
    // `literal`, `pass`, whose verbatim groups run a *different* step
    // selection through `build_for_group`) and an inline `admonition`. A
    // *quote* block is not among them and needs no fixture of its own: like
    // every compound delimited block it carries no content directly, and the
    // paragraphs inside it are reached by this walk's own recursion.
    kinds.sort();
    kinds.dedup();

    assert_eq!(
        kinds,
        [
            "admonition",
            "block title",
            "listing",
            "literal",
            "paragraph",
            "pass",
            "section title",
            "table cell",
        ]
    );

    // And the property the corpus above exists to exercise, asserted directly
    // so the sweep cannot pass by never reaching a resolved reference at all.
    // Both branches of the fold's `resolved` handling are covered: a reference
    // that resolved carries its destination and its text, and one that did not
    // carries the bracketed fallback.
    let mut parser = parser();
    let doc = parser.parse("[[tgt]]Target.\n\nSee <<tgt,the target>> and <<missing>>.");

    let folded: Vec<String> = locations(&doc)
        .iter()
        .map(|l| {
            fold_html(
                l.inlines,
                &HtmlSubstitutionRenderer {},
                &Parser::default().render_context(),
            )
        })
        .collect();

    assert!(
        folded
            .iter()
            .any(|f| f.contains(r##"<a href="#tgt">the target</a>"##)),
        "the sweep never reached a resolved reference: {folded:?}"
    );

    assert!(
        folded.iter().any(|f| f.contains("[missing]")),
        "the sweep never reached an unresolved reference: {folded:?}"
    );
}

#[test]
fn a_stateful_renderer_is_not_required_for_the_fold() {
    // The fold takes the renderer as an argument, so the same tree renders
    // through a second, independently-constructed renderer to the same bytes —
    // the property `render_with` will rest on (design §3.3.1). Driven here on
    // a document whose references resolve, since that is this harness's own
    // subject.
    let mut parser = parser();
    let doc = parser.parse("[[tgt]]Target.\n\nSee <<tgt>> and *bold* and footnote:[a note].");

    let fold_parser = Parser::default();

    for location in locations(&doc) {
        let once = fold_html(
            location.inlines,
            &HtmlSubstitutionRenderer {},
            &fold_parser.render_context(),
        );
        let twice = fold_html(
            location.inlines,
            &HtmlSubstitutionRenderer {},
            &fold_parser.render_context(),
        );

        assert_eq!(once, twice);
        assert_eq!(once, location.rendered);
    }

    // And through a renderer handed out as a shared `Rc`, which is how
    // `Parser::with_inline_substitution_renderer` installs one.
    let shared: Rc<HtmlSubstitutionRenderer> = Rc::new(HtmlSubstitutionRenderer {});

    for location in locations(&doc) {
        assert_eq!(
            fold_html(
                location.inlines,
                shared.as_ref(),
                &fold_parser.render_context()
            ),
            location.rendered
        );
    }
}

// The document-attribute state each content retains for the fold that will run
// *after* resolution — design §4.2's second sentinel system, whose retirement
// is the increment this one stages.

#[test]
fn document_stays_send_and_sync() {
    // Retaining anything from the parser's *configuration* — its renderer,
    // path resolver, or file handlers, all `Rc<dyn …>` — would take this away,
    // silently, since nothing else pins it. `Parser` itself is deliberately
    // neither; a `Document` is both, and a consumer that parses on one thread
    // and renders on another depends on that.
    //
    // This is why a content retains its `ResolvedAttributes` (the
    // order-dependent half, `Arc`-shared and thread-safe) rather than a whole
    // `RenderContext`.
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<crate::Document<'static>>();
    assert_sync::<crate::Document<'static>>();
}

#[test]
fn only_deferred_content_retains_its_render_attributes() {
    use crate::blocks::Block;

    // Retention is deliberately narrow: a context costs a handful of
    // reference-count bumps, but the overwhelming majority of content is never
    // re-rendered and so never folded later than its parse.
    let mut parser = crate::Parser::default();

    let doc = parser.parse("[[tgt]]Plain paragraph.\n\nSee <<tgt>> and <<missing>>.");

    let contexts: Vec<bool> = doc
        .child_blocks()
        .filter_map(|block| match block {
            Block::Simple(simple) => Some(simple.content().render_attributes().is_some()),
            _ => None,
        })
        .collect();

    // The anchor-bearing paragraph carries no cross-reference of its own, so
    // it needs no context; the one that does, does.
    assert_eq!(contexts, vec![false, true]);

    // And a block with no cross-references anywhere never gets one.
    let doc = parser.parse("Just prose with an image:x.png[X].");

    let Some(Block::Simple(simple)) = doc.child_blocks().next() else {
        panic!("expected a simple block");
    };

    assert!(simple.content().render_attributes().is_none());
}

#[test]
fn each_content_retains_the_attributes_of_its_own_point_in_the_document() {
    use crate::blocks::Block;

    // The reason the context is taken during the parse rather than
    // reconstructed at resolution time. Document attributes are mutable parse
    // state: `:imagesdir:` rebinds for everything after it, so the value in
    // effect at the *end* of a document is not generally the value in effect
    // where a given content was written.
    //
    // Two cross-reference-bearing paragraphs straddling a `:imagesdir:` line
    // must therefore disagree about it — which is exactly what a fold running
    // after resolution has to reproduce, and what a single end-of-parse
    // snapshot could not.
    let mut parser = crate::Parser::default();

    let doc = parser.parse(concat!(
        "[[tgt]]Target.\n\n",
        "First <<tgt>>.\n\n",
        ":imagesdir: img\n\n",
        "Second <<tgt>>.",
    ));

    let dirs: Vec<String> = doc
        .child_blocks()
        .filter_map(|block| match block {
            Block::Simple(simple) => simple.content().render_attributes(),
            _ => None,
        })
        .map(|attributes| {
            attributes
                .attribute_value("imagesdir")
                .as_maybe_str()
                .unwrap_or("<unset>")
                .to_string()
        })
        .collect();

    assert_eq!(dirs, vec!["<unset>".to_string(), "img".to_string()]);
}

// The retirement of the deferred-cross-reference sentinel system (design
// §4.2's second): a deferred content's rendering is a fold of its tree, taken
// at the end of resolution.
//
// The fold-against-template comparison that used to sit here
// (`the_fold_reproduces_the_template_for_every_deferred_content`) retired with
// the oracle pipeline: `rendered_from_template` rendered the placeholder
// template the string pipeline captured on a production parse, and no
// production content carries one any more. The golden-HTML assertions across
// the suite are what hold that line now — they pinned the same bytes before
// and after the pipeline stopped running, which is the byte-for-byte gate the
// deletion increment itself was validated against.

#[test]
fn at_least_one_fold_was_authoritative() {
    use crate::blocks::Block;
    // The tree, not just the rendered string, carries resolution's answer: a
    // resolved cross-reference is installed back into its own node, which is
    // what `Content::refold` folds the rendering from. Pinned directly, since
    // the fold-against-template comparison that used to make this observable
    // retired with the oracle pipeline.
    use crate::inlines::{InlineNode, RefVariant};

    let mut parser = parser();
    let doc = parser.parse("[[tgt]]Target.\n\nSee <<tgt>>.");

    let has_resolved_xref = doc.child_blocks().any(|block| {
        let Block::Simple(simple) = block else {
            return false;
        };

        simple.content().inlines().iter().any(|node| {
            matches!(node, InlineNode::Ref(reference)
                if reference.variant == RefVariant::Xref && reference.resolved.is_some())
        })
    });

    assert!(
        has_resolved_xref,
        "expected the tree to carry the resolved cross-reference the fold reads"
    );
}

#[test]
fn resolution_renders_a_deferred_content_once() {
    use crate::parser::{CatalogResolver, InlineSubstitutionRenderer, XrefRenderParams};

    // A renderer is a host-supplied trait object, so *how many times* it is
    // called during one resolution pass is observable — a stateful one (a
    // recorder, a numbering backend, anything counting its own callbacks) sees
    // whatever this pass does.
    //
    // Both renderings exist: the template's (`rebuild_rendered`) and the
    // tree's (`refold`). Exactly one of them runs for any given content —
    // whichever is authoritative for it — rather than one running and then
    // being overwritten by the other.
    #[derive(Debug, Default)]
    struct CountingRenderer {
        xrefs: std::cell::Cell<usize>,
    }

    impl InlineSubstitutionRenderer for CountingRenderer {
        fn render_xref(&self, params: &XrefRenderParams, dest: &mut String) {
            self.xrefs.set(self.xrefs.get() + 1);
            HtmlSubstitutionRenderer {}.render_xref(params, dest);
        }
    }

    // Two cross-references, so a doubled pass reads 4 rather than 2 and the
    // assertion cannot pass by an off-by-one coincidence.
    let mut parser = parser();
    let mut doc = parser.parse_deferred("[[tgt]]Target.\n\nSee <<tgt>> and <<tgt>> again.");

    let catalog = doc.catalog().clone();
    let resolver = CatalogResolver::new(&catalog);
    let counting = CountingRenderer::default();

    doc.resolve_references(&resolver, &counting, &parser);

    assert_eq!(
        counting.xrefs.get(),
        2,
        "resolution rendered this content's cross-references more than once"
    );
}

#[test]
fn a_folded_heading_keeps_the_cross_title_coordination() {
    // A title's rendering is a fold of its tree too, taken by the
    // document-order pass (`title_refs`) in place of the template render it
    // used to do — not after it, since that string is also the link text a
    // reference *to* this title splices in, and computing both would put every
    // deferred title through a host renderer twice.
    //
    // What makes a title its own case is the coordination: the pass resolves
    // titles in document order and lets one title's rendering become another's
    // reference text, breaking cycles the way Asciidoctor does. That survives
    // the fold because it rides on the *destination* — a `ResolvedReference`
    // carries the text the pass computed, the mirror installs it on the node,
    // and `fold_xref` reads it there — rather than on the template.
    //
    // So the oracle here is the coordinated answer itself, not the title's own
    // template: a title's deferred segments are never resolved in place (the
    // pass resolves a clone), so rendering that template would give the
    // *uncoordinated* fallback and prove nothing.
    let cases = [
        // A forward reference: `<<second>>` names a heading parsed later, whose
        // reference text is its own title. The uncoordinated answer is `[second]`.
        (
            "== See <<second>>\n\nBody.\n\n[[second]]\n== The Second\n\nMore.",
            vec![r##"See <a href="#second">The Second</a>"##, "The Second"],
        ),
        // A cycle: computing `a` recurses into `b`, which reaches `a` while it
        // is still in progress and falls back to the bracketed `[a]` rather
        // than recursing forever. `b`'s rendering is then spliced into `a`'s as
        // link text, where the renderer drops the nested anchor.
        (
            "[[a]]\n== A cites <<b>>\n\nBody.\n\n[[b]]\n== B cites <<a>>\n\nMore.",
            vec![
                r##"A cites <a href="#b">B cites [a]</a>"##,
                r##"B cites <a href="#a">[a]</a>"##,
            ],
        ),
        // A footnote inside a heading: its embedded reference is re-homed out
        // of the title's own template, so the two correlation lists partition.
        (
            "[[tgt]]Target.\n\n== A Heading.footnote:[see <<tgt>>]\n\nBody.",
            vec![concat!(
                "A Heading.",
                r##"<sup class="footnote">[<a id="_footnoteref_1" class="footnote" "##,
                r##"href="#_footnotedef_1" title="View footnote.">1</a>]</sup>"##
            )],
        ),
    ];

    for (source, expected) in cases {
        let mut parser = parser();
        let doc = parser.parse(source);

        let headings: Vec<String> = doc
            .child_blocks()
            .filter_map(|block| match block {
                crate::blocks::Block::Section(section) => Some(section.section_title().to_string()),
                _ => None,
            })
            .collect();

        assert_eq!(headings, expected, "for {source:?}");
    }
}

#[test]
fn the_title_pass_renders_each_title_once() {
    use crate::parser::{CatalogResolver, InlineSubstitutionRenderer, XrefRenderParams};

    // The title-side counterpart of `resolution_renders_a_deferred_content_once`,
    // and the property that makes the fold a *replacement* for the document-order
    // pass's template render rather than an addition to it.
    //
    // The pass needs each title's rendering while it runs — it is the link text
    // a reference to that title splices in — so the fold has to happen inside
    // `compute`, in place of `render_xref_template`. Folding after the pass
    // instead (in its write-back walk, the obvious place) would leave both, and
    // every deferred title would reach a host renderer twice.
    #[derive(Debug, Default)]
    struct CountingRenderer {
        xrefs: std::cell::Cell<usize>,
    }

    impl InlineSubstitutionRenderer for CountingRenderer {
        fn render_xref(&self, params: &XrefRenderParams, dest: &mut String) {
            self.xrefs.set(self.xrefs.get() + 1);
            HtmlSubstitutionRenderer {}.render_xref(params, dest);
        }
    }

    // Two headings, one cross-reference each, and neither references the other
    // — so the pass has no reason to render either more than once and the count
    // is exactly the number of cross-references.
    let mut parser = parser();
    let mut doc = parser.parse_deferred(concat!(
        "[[tgt]]Target.\n\n",
        "== First sees <<tgt>>\n\n",
        "Body.\n\n",
        "== Second sees <<tgt>>\n\n",
        "More.",
    ));

    let catalog = doc.catalog().clone();
    let resolver = CatalogResolver::new(&catalog);
    let counting = CountingRenderer::default();

    doc.resolve_references(&resolver, &counting, &parser);

    assert_eq!(
        counting.xrefs.get(),
        2,
        "the document-order title pass rendered a title more than once"
    );
}
