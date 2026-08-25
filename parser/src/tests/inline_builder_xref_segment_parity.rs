//! A differential harness for the **deferred cross-reference segments** read
//! off the inline tree.
//!
//! Design §5.2's survey of what `run_pipeline` still solely owns named six
//! things, two of them blocked on one question rather than on effort. This is
//! the second of that pair: the [`XrefSegment`](crate::content::XrefSegment)
//! list a content carries for its deferred cross-references. Every field on a
//! segment is one a [`Ref`](crate::inlines::Ref)`{Xref}` node already holds —
//! the survey said as much — *except* `provided_text`, which the segment holds
//! as a string where the node holds its display text as children. The sibling
//! increment that answered the computed-**string**-slot question unblocked it,
//! and the answer here is the other one: a display text is markup by nature, so
//! the slot takes the **fold of those children**.
//!
//! What this harness pins is that reading, over whole documents: for every
//! content that deferred a cross-reference, the segments derived from the tree
//! are field-for-field what `InlineXrefReplacer` itself produced. Nothing is
//! wired — `block_tree_xref_segments` and `footnote_tree_xref_segments` are
//! staged building blocks, exactly as every recognition side effect was before
//! the cutover re-attached it — so this corpus is the only thing exercising
//! them, and it is what a later increment stands on when it deletes the
//! string pipeline's own construction.
//!
//! The documents are parsed with [`Parser::parse_deferred`], which does **not**
//! resolve, so every segment's `resolved` is `None` on both sides: this
//! compares what *recognition* produced, which is the half `run_pipeline`
//! owns. (`resolved` is resolution's output and is deliberately not carried
//! across by the derivation — see `xref_segment_from_node`.)

use crate::{
    Parser,
    blocks::{Block, FindBlocks},
    content::{Content, XrefSegment, block_tree_xref_segments, footnote_tree_xref_segments},
    parser::{HtmlSubstitutionRenderer, ModificationContext},
};

/// A parser with `experimental` set, so a fixture's UI macros are recognized,
/// matching the sibling whole-document harnesses.
fn parser() -> Parser {
    Parser::default().with_intrinsic_attribute_bool(
        "experimental",
        true,
        ModificationContext::Anywhere,
    )
}

/// Every [`Content`] in `doc` that carries a tree, in document order.
///
/// Unlike the [document-parity harness](super::inline_builder_document_parity)
/// next door, this one needs the `Content` itself rather than its rendered
/// string and tree, because the segments live on it — so it reaches each one
/// through the accessors that return a `&Content` rather than through
/// `IsBlock`'s two.
fn contents<'src>(doc: &'src crate::Document<'src>) -> Vec<(String, &'src Content<'src>)> {
    fn walk<'src>(block: &'src Block<'src>, out: &mut Vec<(String, &'src Content<'src>)>) {
        match block {
            Block::Simple(simple) => {
                out.push(("paragraph".to_string(), simple.content()));
            }

            Block::RawDelimited(raw) => {
                out.push(("raw delimited".to_string(), raw.content()));
            }

            Block::Admonition(admonition) => {
                if let Some(content) = admonition.content() {
                    out.push(("admonition".to_string(), content));
                }
            }

            Block::Quote(quote) => {
                if let Some(content) = quote.content() {
                    out.push(("quote".to_string(), content));
                }
            }

            _ => {}
        }

        // A block title (`.Title`) is substituted content in its own right and
        // defers its own cross-references.
        if let Some(title) = block.block_title_content() {
            out.push(("block title".to_string(), title));
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

/// Splits a content's own segments the way
/// [`block_tree_xrefs`](crate::content::block_tree_xrefs) and
/// [`footnote_tree_xrefs`](crate::content::footnote_tree_xrefs) split them: a
/// placeholder still in the template is block-level, one that has left it was
/// re-homed onto a footnote.
fn partition<'a>(
    template: &str,
    xrefs: &'a [XrefSegment],
) -> (Vec<&'a XrefSegment>, Vec<&'a XrefSegment>) {
    let mut block = vec![];
    let mut footnote = vec![];

    for (index, xref) in xrefs.iter().enumerate() {
        if template.contains(&Content::xref_placeholder(index)) {
            block.push(xref);
        } else {
            footnote.push(xref);
        }
    }

    (block, footnote)
}

/// Sources whose contents defer at least one cross-reference. Each names the
/// shape it is here for; the assertion covers every field of every segment, so
/// a fixture pins more than the one thing it was added for.
const CORPUS: &[&str] = &[
    // ---- the two spellings, with and without a display text -------------
    "See <<tgt>> here.\n\n[[tgt]]Target.",
    "See <<tgt,the target>> here.\n\n[[tgt]]Target.",
    "See xref:tgt[] here.\n\n[[tgt]]Target.",
    "See xref:tgt[the target] here.\n\n[[tgt]]Target.",
    // The present-but-empty text: a comma with nothing after it. This is the
    // distinction the `Option` keys on the *presence of a child* for.
    "See <<tgt,>> here.\n\n[[tgt]]Target.",
    "See xref:tgt[ ] here.\n\n[[tgt]]Target.",
    // ---- a display text that is markup ----------------------------------
    // The whole reason this slot takes the fold rather than the source: the
    // string replacer captures the *rendered* span out of its own haystack.
    "See <<tgt,*bold* text>> here.\n\n[[tgt]]Target.",
    "See xref:tgt[*bold* text] here.\n\n[[tgt]]Target.",
    "See xref:tgt[`code` and _em_] here.\n\n[[tgt]]Target.",
    "See <<tgt,a #mark# here>> here.\n\n[[tgt]]Target.",
    // A text carrying an escaped special, a restored entity, and a
    // typographic replacement — the three recoverable pieces.
    "See xref:tgt[a < b] here.\n\n[[tgt]]Target.",
    "See xref:tgt[Tom &copy; Jerry] here.\n\n[[tgt]]Target.",
    "See xref:tgt[Pause (C) Resume] here.\n\n[[tgt]]Target.",
    // A text carrying a passthrough, which the string pipeline restores into
    // the segment separately (`restore_deferred_xref_passthroughs`) and the
    // tree carries as a `Raw` child.
    "See xref:tgt[a +++<b>raw</b>+++ x] here.\n\n[[tgt]]Target.",
    "See xref:tgt[a $$literal$$ x] here.\n\n[[tgt]]Target.",
    // A nested macro in the display text.
    "See xref:tgt[see image:i.png[I]] here.\n\n[[tgt]]Target.",
    "See xref:tgt[see link:l.html[L]] here.\n\n[[tgt]]Target.",
    // ---- the attribute-list form, which fills the other fields ----------
    "See xref:tgt[Text,role=hl] here.\n\n[[tgt]]Target.",
    "See xref:tgt[Text,window=_blank] here.\n\n[[tgt]]Target.",
    "See xref:tgt[Text,role=a b,window=_blank] here.\n\n[[tgt]]Target.",
    "See xref:tgt[Text,xrefstyle=full] here.\n\n[[tgt]]Target.",
    // A document-wide `xrefstyle`, which the node resolves at build time.
    ":xrefstyle: full\n\nSee <<tgt>> here.\n\n[[tgt]]Target.",
    // ---- a target that names a document (a *derived* destination) -------
    "See <<other.adoc#sec>> here.",
    "See xref:other.adoc#sec[Elsewhere] here.",
    "See xref:other.adoc#[] here.",
    // ---- unresolved targets, which still produce a segment --------------
    "See <<nope>> here.",
    "See xref:nope[Text] here.",
    // ---- more than one reference in one content -------------------------
    "See <<a>> and <<b>> and <<a>> again.\n\n[[a]]A.\n\n[[b]]B.",
    "See <<a,First>> then xref:b[Second].\n\n[[a]]A.\n\n[[b]]B.",
    // ---- a reference nested inside a container --------------------------
    "See *a <<tgt>> b* here.\n\n[[tgt]]Target.",
    "See #a <<tgt>> b# here.\n\n[[tgt]]Target.",
    // ---- footnote-embedded references, the complementary list -----------
    "See footnote:[a note with <<tgt>> inside].\n\n[[tgt]]Target.",
    "See footnote:[see <<a>>] and <<b>>.\n\n[[a]]A.\n\n[[b]]B.",
    "See <<a>> then footnote:[see <<b>>] then <<c>>.\n\n[[a]]A.\n\n[[b]]B.\n\n[[c]]C.",
    // ---- other content-bearing locations --------------------------------
    ".A title with <<tgt>>\nParagraph body.\n\n[[tgt]]Target.",
    "[NOTE]\n====\nSee <<tgt>> here.\n====\n\n[[tgt]]Target.",
    "[quote]\n____\nSee <<tgt>> here.\n____\n\n[[tgt]]Target.",
];

#[test]
fn derived_segments_match_the_string_pipelines_own() {
    let mut saw_block = 0usize;
    let mut saw_footnote = 0usize;

    for source in CORPUS {
        let mut parser = parser();

        // `parse_deferred` does not resolve, so both sides carry
        // `resolved: None` and this compares recognition alone.
        let doc = parser.parse_deferred(source);
        let context = parser.render_context();
        let renderer = HtmlSubstitutionRenderer {};

        for (what, content) in contents(&doc) {
            let Some((template, xrefs)) = content.deferred_parts() else {
                continue;
            };

            let (golden_block, golden_footnote) = partition(template, xrefs);

            let derived_block = block_tree_xref_segments(content.inlines(), &renderer, &context);

            let derived_footnote =
                footnote_tree_xref_segments(content.inlines(), &renderer, &context);

            saw_block += derived_block.len();
            saw_footnote += derived_footnote.len();

            let golden_block: Vec<XrefSegment> = golden_block.into_iter().cloned().collect();

            let golden_footnote: Vec<XrefSegment> = golden_footnote.into_iter().cloned().collect();

            assert_eq!(
                derived_block, golden_block,
                "block-level segments diverged for {what} of {source:?}"
            );

            assert_eq!(
                derived_footnote, golden_footnote,
                "footnote segments diverged for {what} of {source:?}"
            );
        }
    }

    // Guards against a vacuous pass: a corpus that stopped deferring (or a
    // walk that stopped finding anything) would otherwise compare empty
    // against empty for every fixture and report success.
    assert!(
        saw_block >= 40,
        "the corpus stopped exercising block-level segments: {saw_block}"
    );

    assert!(
        saw_footnote >= 3,
        "the corpus stopped exercising footnote segments: {saw_footnote}"
    );
}

#[test]
fn a_rendered_span_in_a_string_read_slot_keeps_its_documented_divergence() {
    // The one shape in this family whose derived segment is *not* the string
    // pipeline's, and the divergence is deliberate: the increment that
    // answered the computed-string-slot question gave `role=` / `window=` /
    // `xrefstyle=` the author's **untranslated source**, because the string
    // path there has no output worth matching (it captures the deferred
    // template before passthroughs are restored, so it leaks a sentinel, and
    // it spells a rendered span into a class name).
    //
    // What this pins is that the two answers coexist on one segment: the
    // `role` differs by that rule, while `provided_text` — the field this
    // increment is about — is byte-identical, because a display text is markup
    // by nature and takes the fold of the node's children.
    let source = "See xref:tgt[*bold*,role=*hl*] here.\n\n[[tgt]]Target.";

    let mut parser = parser();
    let doc = parser.parse_deferred(source);
    let context = parser.render_context();
    let renderer = HtmlSubstitutionRenderer {};

    let (_, content) = contents(&doc)
        .into_iter()
        .find(|(_, content)| content.deferred_parts().is_some())
        .expect("the fixture must defer a cross-reference");

    let (template, xrefs) = content.deferred_parts().unwrap();
    let (golden, _) = partition(template, xrefs);
    let derived = block_tree_xref_segments(content.inlines(), &renderer, &context);

    assert_eq!(derived.len(), 1);
    assert_eq!(golden.len(), 1);

    // The field this increment reads off the tree: identical.
    assert_eq!(
        derived[0].provided_text.as_deref(),
        Some("<strong>bold</strong>")
    );

    assert_eq!(derived[0].provided_text, golden[0].provided_text);

    // The field the sibling increment deliberately answers differently.
    assert_eq!(derived[0].roles, ["*hl*"]);
    assert_eq!(golden[0].roles, ["<strong>hl</strong>"]);

    // Every other field still agrees, so the divergence is confined to the
    // one slot its own increment named.
    assert_eq!(derived[0].target, golden[0].target);
    assert_eq!(derived[0].window, golden[0].window);
    assert_eq!(derived[0].xrefstyle, golden[0].xrefstyle);
    assert_eq!(derived[0].derived, golden[0].derived);
}
