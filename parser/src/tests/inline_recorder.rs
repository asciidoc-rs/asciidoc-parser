//! Differential oracle for the inline AST, over the
//! [`inline_tree`](crate::content::inline_tree) recorder and the flag-gated
//! production tree path.
//!
//! This began as the Phase 1 bring-up oracle (a test-only recorder). In Phase 2
//! the recorder machinery moved into the parser proper and became the
//! production tree source; in Phase 4 the single-pass builder
//! ([`inline_builder`](crate::content::inline_builder)) replaced it there (the
//! step 6 tree-source swap) and the recorder returned to test-only status.
//! This module remains the **harness** for both:
//!
//! 1. The recorder-driven corpus keeps proving Strategy A's own invariants —
//!    stripping/folding the marked string reproduces the built-in renderer's
//!    `rendered_html()` output **byte-for-byte** (the *no-perturbation*
//!    invariant), and the recovered [`InlineNode`] tree is structurally
//!    faithful (node kinds, nesting, and the `constructs <= markers <= events`
//!    cross-check) — so the recorder stays honest as the independent
//!    construction the structural cross-check
//!    (`tests::inline_builder_recorder_parity`) compares the builder against.
//! 2. The wiring tests drive the **production** tree path — now the single-pass
//!    builder, run for every parse since the `with_inline_tree` opt-in retired
//!    — asserting what [`Content::inlines`](crate::content::Content) stores for
//!    real parsed documents.
//!
//! [inline AST architecture]: ../../../docs/design/inline-ast-architecture.md

#![allow(clippy::unwrap_used)]

use std::rc::Rc;

use crate::{
    Parser, Span,
    blocks::FindBlocks,
    content::{
        Content, SubstitutionGroup,
        inline_tree::{
            RecordingRenderer, build_inline_tree, fold_marked, is_reserved_sentinel,
            open_marker_count,
        },
    },
    inlines::{CharRef, InlineNode, RefVariant, SpanForm, StyleVariant, UiKind},
    parser::{HtmlSubstitutionRenderer, ModificationContext},
};

/// Rejects a fixture whose source uses a reserved recorder codepoint. Such a
/// character would be indistinguishable from recorder framing, so the oracle
/// fails fast rather than silently mis-validating the input. (The production
/// pipeline likewise reserves its own PUA sentinels; those inputs are an
/// accepted edge.)
fn assert_no_reserved_sentinels(source: &str) {
    assert!(
        !source.chars().any(is_reserved_sentinel),
        "fixture source contains a codepoint reserved for recorder sentinels: {source:?}"
    );
}

/// The number of recorded constructs (non-text nodes) in the tree — i.e. the
/// number of recorder [`Event`](crate::content::inline_tree)s the tree
/// consumed. Used to cross-check the tree against the recorder.
fn construct_count(nodes: &[InlineNode<'_>]) -> usize {
    nodes
        .iter()
        .map(|node| match node {
            // Text and recovered character references are not recorded events.
            InlineNode::Text { .. } | InlineNode::CharRef { .. } | InlineNode::Raw { .. } => 0,

            InlineNode::Styled(styled) => 1 + construct_count(&styled.children),
            InlineNode::Ref(reference) => 1 + construct_count(&reference.children),
            InlineNode::Footnote(footnote) => 1 + construct_count(&footnote.children),

            // A STEM node folds its body into `value`, so it has no child nodes;
            // it is one recorded construct, matching the recorder.
            InlineNode::Stem(_) => 1,

            _ => 1,
        })
        .sum()
}

/// Enables `:experimental:` so the UI macros (`kbd:`, `btn:`, `menu:`) are
/// recognized. Harmless for every other fixture.
fn experimental(parser: Parser) -> Parser {
    parser.with_intrinsic_attribute_bool("experimental", true, ModificationContext::Anywhere)
}

/// The result of running the oracle on one source string.
struct Oracle<'src> {
    /// The authoritative `rendered_html()` output of the unmodified pipeline.
    golden: String,

    /// The fold of the recovered tree (the recorder's marked string folded back
    /// to output bytes).
    folded: String,

    /// The recovered tree, projected onto the public vocabulary.
    tree: Vec<InlineNode<'src>>,

    /// The number of constructs the recorder captured.
    events: usize,

    /// The number of open-markers that reached the final string.
    markers: usize,

    /// The number of marked construct nodes in the recovered tree (recovered
    /// character references excluded, as they are not recorded events).
    constructs: usize,
}

/// Runs `source` through the pipeline twice under `group`: once with the
/// built-in HTML renderer (the golden output) and once with the production
/// [`RecordingRenderer`] (from which the tree is built). Both use a default
/// [`Parser`].
fn oracle<'src>(source: &'src str, group: &SubstitutionGroup) -> Oracle<'src> {
    assert_no_reserved_sentinels(source);

    // Golden pass.
    let golden_parser = experimental(Parser::default());
    let mut golden_content = Content::from(Span::new(source));
    group.apply_string_pipeline(&mut golden_content, &golden_parser, None);
    let golden = golden_content.rendered_owned();

    // Recording pass, driving the production recorder.
    let (recorder, events) = RecordingRenderer::new(Rc::new(HtmlSubstitutionRenderer {}));
    let recording_parser =
        experimental(Parser::default()).with_inline_substitution_renderer(recorder);
    let mut recording_content = Content::from(Span::new(source));
    group.apply_string_pipeline(&mut recording_content, &recording_parser, None);
    let marked = recording_content.rendered_owned();

    let markers = open_marker_count(&marked);
    let events_len = events.borrow().len();
    let tree = build_inline_tree(&marked, &events.borrow(), Span::new(source));
    let folded = fold_marked(&marked, &events.borrow());
    let constructs = construct_count(&tree);

    Oracle {
        golden,
        folded,
        tree,
        events: events_len,
        markers,
        constructs,
    }
}

/// Asserts the oracle's invariants for `source` under `group`:
///
/// 1. folding the recorder's marked string reproduces the golden output
///    byte-for-byte, and
/// 2. the recovered tree consumed exactly the recorded events (`constructs <=
///    markers <= events`).
///
/// Returns the recovered tree for callers that also want to assert on
/// structure.
fn check<'src>(source: &'src str, group: &SubstitutionGroup) -> Vec<InlineNode<'src>> {
    let result = oracle(source, group);

    assert_eq!(
        result.folded, result.golden,
        "fold of the inline tree diverged from rendered_html() for {source:?}"
    );

    // Cross-check the tree against the recorder along the honest chain
    // `constructs <= markers <= events`:
    //
    // * `markers <= events` — some recorded events (formatting inside a footnote's
    //   text) are extracted out of the block, so their marker never reaches the
    //   block string.
    // * `constructs <= markers` — some markers (formatting inside a construct
    //   rendered as a single leaf, e.g. a formatted xref reftext) are absorbed into
    //   that leaf's HTML rather than surfacing as their own node.
    assert!(
        result.markers <= result.events,
        "{} markers exceeded {} recorded events for {source:?}",
        result.markers,
        result.events,
    );

    assert!(
        result.constructs <= result.markers,
        "tree has {} constructs but the block string held {} markers for {source:?}",
        result.constructs,
        result.markers,
    );

    result.tree
}

fn check_normal(source: &str) -> Vec<InlineNode<'_>> {
    check(source, &SubstitutionGroup::Normal)
}

// ─── Byte-parity corpus ─────────────────────────────────────────────────────
//
// A differential harness over a broad set of inline fixtures. Each asserts the
// oracle's invariants; together they exercise every intercepted construct.

/// Inline fixtures covering the `normal` substitution group.
const NORMAL_CORPUS: &[&str] = &[
    // Plain text and the empty case.
    "",
    "plain text with no constructs",
    "text with trailing spaces   and   runs",
    // Special characters.
    "a < b && c > d",
    "1 < 2 & 3 > 0",
    "<>&",
    // Quotes: constrained.
    "One *word* is strong.",
    "An _emphasized_ phrase.",
    "Some `monospaced` text.",
    "A #highlighted# span.",
    "H~2~O and E = mc^2^.",
    "A bold *phrase of text* here.",
    // Quotes: unconstrained.
    "Bold c**hara**cter**s** in a word.",
    "un__frac__tured emphasis",
    "a##b##c",
    // Quotes: nested.
    "*a _b_ c*",
    "*bold _and italic_ mix*",
    "_*strong inside emphasis*_",
    // Quotes with roles / id shorthand.
    "[.myrole]#roled text#",
    "[#anchor]#anchored#",
    "[.a.b]#multi role#",
    // Quotes mixed with special characters (interleaving).
    "*a < b* and _c > d_",
    "code `x < y && z` here",
    // Smart quotes.
    "\"`double`\" and '`single`' quotes",
    // Character replacements.
    "(C) (R) (TM)",
    "An em -- dash and an ellipsis...",
    "arrows -> => <- <=",
    "Sam's apostrophe",
    "named &amp; numeric &#8217; entities",
    // Attribute references expand (no macro).
    "plain {backend} attribute",
    // Macros: links.
    "See https://example.org for details.",
    "A link:https://example.org[example] link.",
    "link:https://example.org[with *bold* text]",
    "mailto:a@b.com[email me]",
    // Macros: images / icons.
    "an image:photo.png[Alt Text] inline",
    "image:pic.png[Scaled,200,100]",
    // Macros: cross-references (unresolved fallback).
    "see <<target>> for more",
    "see <<target,the target>> now",
    "xref:other.adoc#frag[Other] doc",
    // Macros: footnotes.
    "A claim.footnote:[the evidence]",
    "Named.footnote:disc[a discussion] then footnote:disc[].",
    "A claim.footnote:[the *strong* evidence and a link:https://e.org[source]]",
    "Two notes.footnote:[first] and again.footnote:[second]",
    // Macros: UI.
    "Press kbd:[Ctrl+T] now.",
    "Press kbd:[Ctrl,Shift,N] now.",
    "Click btn:[Save] please.",
    "Choose menu:File[Save As > PDF].",
    // Macros: index terms.
    "A flow ((visible term)) here.",
    "A concealed (((primary,secondary))) term.",
    // Macros: anchors.
    "[[the-anchor]]Anchored paragraph.",
    // Line breaks (post-replacement), including one that *ends* the content
    // with no newline after it.
    "first line +\nsecond line",
    "only +",
    // Combined stress.
    "*bold* _em_ `code` (C) https://x.y[link] <<ref>> image:i.png[i]",
    "A mix of {backend}, *bold < text*, and a footnote:[with `code`].",
    // Escapes (formatting suppressed).
    r"a \*not bold* b",
    r"an \_not emphasized_ here",
    r"literal \`backtick\` text",
    // Deeper / adjacent nesting.
    "*a _b `c` d_ e*",
    "*one* *two* *three*",
    "_a_ and _b_ and _c_",
    "`code with *stars* inside`",
    // Unconstrained with adjacent text.
    "pre**mid**post and a__b__c",
    "x^sup^y and p~sub~q",
    // Multiple roles and an id together.
    "[.role1.role2#the-id]#decorated#",
    "[#only-id]#text#",
    // Monospace with special chars.
    "`a < b > c & d`",
    // Entities and replacements interleaved.
    "(C) then (R) then (TM) end",
    "First... then -- and -> arrows <-",
    "a &amp; b and &#8482; and &copy;",
    // Attribute reference that is undefined (kept verbatim) and defined.
    "an {undefined-attr} reference",
    // Links with attributes and roles.
    "link:https://example.org[Example,role=external,window=_blank]",
    "https://example.org[Example^]",
    "a https://example.org[] bare-macro link",
    // Autolinks in text.
    "visit https://example.org/path?q=1 now",
    // Images with roles / alt containing spaces.
    "image:a.png[An image with spaces,role=thumb]",
    "before image:b.svg[Vector] after",
    // Several xrefs (unresolved fallback).
    "<<a>> and <<b>> and <<c,C text>>",
    "xref:sec[with *bold* reftext]",
    // Index terms of each kind.
    "a ((flow term)) and (((c1, c2, c3))) end",
    "indexterm:[primary, secondary]",
    "indexterm2:[shown]",
    // Anchors in different positions.
    "[[mid-anchor]] after the anchor",
    "text before anchor:named[Ref Text] and after",
    // Passthroughs (should survive verbatim as-is).
    "a +++<b>raw</b>+++ passthrough",
    "inline pass:[<i>x</i>] macro",
    "math $$a < b$$ here",
    "a +literal *stars*+ b",
    // Hard breaks across multiple lines, and one on the final line only.
    "line one +\nline two +\nline three",
    "line one\nline two +",
    // A ` +` that ends a *span* is not at a line end in the rendered string
    // (its closing tag follows), so it stays literal — unlike one that ends the
    // content itself.
    "*bold +*",
    "*bold +\nmore +*",
    "a link:https://example.org[text +] here",
    // STEM inline (asciimath / latexmath).
    "stem:[a < b] expression",
    "asciimath:[a < b] inline",
    "latexmath:[x < y] inline",
    // Inline icon macro (renders via `render_icon`).
    "an icon:home[] icon",
    "icon:star[2x,role=gold] rated",
    // Empty-ish and whitespace.
    "   ",
    "a\nb\nc",
];

/// Inline fixtures for the `verbatim` group (special characters + callouts).
const VERBATIM_CORPUS: &[&str] = &[
    "plain verbatim",
    "verbatim < & > chars",
    "line of code <1>",
    "tag <.> auto callout",
    "line of xml code <!--1-->",
];

#[test]
fn normal_corpus_folds_byte_for_byte() {
    for source in NORMAL_CORPUS {
        check(source, &SubstitutionGroup::Normal);
    }
}

// ─── Document-level differential ────────────────────────────────────────────
//
// The block-level harness above drives `SubstitutionGroup::apply` on a bare
// `Content`, which cannot resolve cross-references (that needs a document
// catalog). This harness parses whole documents — once normally, once with the
// recorder installed — and asserts that folding each simple block's recorded
// tree reproduces its `rendered_html()` string. It reaches resolved xrefs, list
// items, section bodies, and the interactions between blocks.

/// Collects the rendered inline content of every content-bearing location in
/// `doc`, in a fixed document order.
fn collect_rendered(doc: &crate::Document<'_>) -> Vec<String> {
    use crate::blocks::{Block, IsBlock, TableCellContent, TableRow};

    fn cells(row: &TableRow<'_>, out: &mut Vec<String>) {
        for cell in row.cells() {
            // Only inline (`Simple`) cells carry a single `Content`; an
            // `AsciiDoc` cell is a nested standalone document and is out of
            // scope for this harness.
            if let TableCellContent::Simple(content) = cell.content() {
                out.push(content.rendered_html().to_string());
            }
        }
    }

    fn walk(block: &Block<'_>, out: &mut Vec<String>) {
        if let Some(title) = block.title() {
            out.push(title.to_string());
        }

        match block {
            Block::Simple(simple) => out.push(simple.content().rendered_html().to_string()),

            Block::Section(section) => out.push(section.section_title().to_string()),

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

/// Collects the rendered text of every footnote `doc` registered, in document
/// order.
///
/// A footnote's text is extracted out of the block it was written in, so it
/// never appears in any `rendered_html()` string [`collect_rendered`] reaches.
/// It is nonetheless substituted inline content — and the source of a footnote
/// node's subtree — so the differential harness folds it under the same
/// invariant.
fn collect_footnote_texts(doc: &crate::Document<'_>) -> Vec<String> {
    doc.catalog()
        .footnotes()
        .iter()
        .map(|footnote| footnote.text.clone())
        .collect()
}

/// Parses `source` twice — normally and with the recorder — and asserts that
/// folding every content location's recorded tree reproduces its
/// `rendered_html()` output byte-for-byte, including any resolved
/// cross-references, section headings, block titles, table cells, and footnote
/// texts.
fn check_document(source: &str) {
    assert_no_reserved_sentinels(source);

    let mut golden_parser = experimental(Parser::default());
    let golden_doc = golden_parser.parse(source);

    let mut golden = collect_rendered(&golden_doc);
    golden.extend(collect_footnote_texts(&golden_doc));

    let (recorder, events) = RecordingRenderer::new(Rc::new(HtmlSubstitutionRenderer {}));
    let mut recording_parser =
        experimental(Parser::default()).with_inline_substitution_renderer(recorder);
    let recording_doc = recording_parser.parse(source);
    let events = events.borrow();

    let mut marked = collect_rendered(&recording_doc);
    marked.extend(collect_footnote_texts(&recording_doc));

    let folded: Vec<String> = marked
        .iter()
        .map(|marked| fold_marked(marked, &events))
        .collect();

    // Guard against a fixture that exercises nothing (e.g. a table whose cells
    // a shallower walk would have skipped): a document fixture must fold at
    // least one content location.
    assert!(
        !golden.is_empty(),
        "document fixture folded no content for {source:?}"
    );

    assert_eq!(
        folded, golden,
        "document fold diverged from rendered_html() for {source:?}"
    );
}

/// Whole-document fixtures, including resolved cross-references (which the
/// block-level harness cannot reach).
const DOCUMENT_CORPUS: &[&str] = &[
    // Resolved cross-reference via an explicit anchor.
    "[[tgt]]The target paragraph.\n\nSee <<tgt>> for details.",
    "[[tgt]]Target.\n\nSee <<tgt,the target>> now.",
    // Several references, one unresolved.
    "[[a]]First.\n\n[[b]]Second.\n\nSee <<a>>, <<b>>, and <<missing>>.",
    // Multiple paragraphs with formatting.
    "First *para* here.\n\nSecond _para_ with `code` and (C).",
    // Footnotes across paragraphs.
    "Body text.footnote:[the first note]\n\nMore text.footnote:[the second note]",
    // Footnote-embedded content: the text is extracted out of the block, so
    // these fold under the footnote arm of the harness rather than the block one.
    "A claim.footnote:[the *strong* and _soft_ evidence]",
    "A claim.footnote:[a < b and (C) and an ellipsis...]",
    "A claim.footnote:[see link:https://example.org[the source]]",
    "A claim.footnote:[with an image:proof.png[Proof] inline]",
    // Footnote-embedded cross-references, resolved and unresolved.
    "[[tgt]]The target.\n\nA claim.footnote:[see <<tgt>> for details]",
    "[[tgt]]The target.\n\nA claim.footnote:[see xref:tgt[the target] now]",
    "A claim.footnote:[see <<missing>>]",
    "[[a]]A.\n\n[[b]]B.\n\nSee <<a>>.footnote:[and also <<b>>] and <<b>>.",
    // A named footnote defined once and referenced again.
    "Named.footnote:disc[a *discussion* of <<tgt>>] then footnote:disc[].\n\n[[tgt]]Target.",
    // Defining, referencing, and defining again, each carrying its own embedded
    // reference: the re-homed segments must stay aligned with the subtrees.
    "A.footnote:x[first <<t>>] B.footnote:x[] C.footnote:[second <<t>>]\n\n[[t]]T.",
    // A reference to an ID that was never defined (no number, no subtree).
    "A claim.footnote:never-defined[]",
    // A footnote nested inside a formatting span, so the subtree walk has to
    // descend through the `Styled` node to reach it.
    "*bold with a footnote:[a note] inside*",
    // Footnotes in list items, a table cell, and a block `.Title`.
    "* item.footnote:[list note]\n* second.footnote:[another note]",
    "|===\n| cell.footnote:[cell note] | plain\n|===",
    ".A title.footnote:[title note]\nA paragraph.",
    // A footnote arriving via attribute expansion.
    ":note: footnote:[from an attribute]\n\nA claim.{note}",
    // Multi-line footnote text (normalized onto one line).
    "A claim.footnote:[text spread\nacross\nlines]",
    // A footnote in a section heading (owned by the document-order title pass).
    "[[tgt]]The target.\n\n== A Heading.footnote:[see <<tgt>>]\n\nBody.\n",
    // A list with inline formatting per item.
    "* item *one*\n* item _two_ with `code`\n* item with a < b",
    // A section whose body references an explicitly-anchored target.
    "== A Section\n\n[[para]]Some text.\n\nBack to <<para>>.",
    // Image and link together in a paragraph.
    "See image:x.png[X] and also https://example.org[the site].",
    // Ordered list with entities and a quote.
    ". one -- two\n. three *four*",
    // A description list term and description.
    "Term:: a *bold* description with a < b",
    // Cross-reference to an auto-generated section ID (derived from a plain
    // title): a stress test that the recorder does not perturb ID generation.
    "== Introduction\n\nText.\n\n== Details\n\nSee <<_introduction>> above.",
    // A simple table whose cells carry inline formatting.
    "|===\n| *bold* cell | a < b\n| plain | (C)\n|===",
];

#[test]
fn document_corpus_folds_byte_for_byte() {
    for source in DOCUMENT_CORPUS {
        check_document(source);
    }
}

/// Parses `source` twice — with inline-tree building off (the default) and on —
/// and asserts the rendered output is identical, so enabling the flag cannot
/// perturb what the golden `.rendered_html()` assertions pin.
///
/// This also drives the whole flag-on production path over the corpus: the
/// counter-safe recording pass, the footnote-subtree attachment, and both
/// cross-reference mirrors. Each of those carries a debug assertion that fires
/// here if the recording pass and the authoritative pass ever enumerate
/// constructs differently, so the sweep is where a correlation break surfaces.
fn check_inline_tree_flag_parity(source: &str) {
    let mut off_parser = experimental(Parser::default());
    let off_doc = off_parser.parse(source);

    let mut off = collect_rendered(&off_doc);
    off.extend(collect_footnote_texts(&off_doc));

    let mut on_parser = experimental(Parser::default());
    let on_doc = on_parser.parse(source);

    let mut on = collect_rendered(&on_doc);
    on.extend(collect_footnote_texts(&on_doc));

    assert_eq!(
        on, off,
        "enabling inline-tree building changed the rendered output for {source:?}"
    );
}

#[test]
fn inline_tree_flag_leaves_rendered_output_unchanged() {
    for source in DOCUMENT_CORPUS
        .iter()
        .chain(NORMAL_CORPUS)
        .chain(VERBATIM_CORPUS)
    {
        check_inline_tree_flag_parity(source);
    }
}

#[test]
fn document_walk_reaches_section_headings_and_table_cells() {
    // Regression guard: section headings and table cells are not child simple
    // blocks, so `collect_rendered` must reach them explicitly (otherwise a
    // table fixture folds nothing and passes vacuously).
    let mut parser = experimental(Parser::default());
    let doc = parser.parse("== Heading\n\n|===\n| *bold* cell | plain\n|===");
    let rendered = collect_rendered(&doc);

    assert!(
        rendered.iter().any(|s| s == "Heading"),
        "section heading was not collected: {rendered:?}"
    );

    assert!(
        rendered.iter().any(|s| s == "<strong>bold</strong> cell"),
        "table cell content was not collected: {rendered:?}"
    );
}

#[test]
#[should_panic(expected = "reserved for recorder sentinels")]
fn source_using_a_reserved_sentinel_codepoint_is_rejected() {
    // A literal marker codepoint in the source would be indistinguishable from
    // recorder framing; the oracle rejects it rather than mis-validating.
    check("before \u{E010} after", &SubstitutionGroup::Normal);
}

/// A known limitation of the Strategy A oracle at document scope: when a
/// section title carries inline formatting, its *rendered* text (now bearing
/// recorder sentinels) feeds the cross-reference catalog as the reference's
/// display text, and a later `<<auto-id>>` to that section resolves differently
/// in the recorded parse than in the golden one. This is the "double render can
/// drift" property the design calls out (§4.1); it is retired by the
/// single-pass builder in Phase 4, which never re-renders. Plain-title sections
/// and explicitly-anchored targets (exercised above) are unaffected. Recorded
/// as an ignored test so the boundary is explicit rather than hidden.
#[test]
#[ignore = "Strategy A perturbs xref reftext derived from a formatted section title; fixed in Phase 4"]
fn formatted_section_title_xref_is_a_known_strategy_a_gap() {
    check_document("== The *Bold* Heading\n\nBody.\n\nSee <<_the_bold_heading>>.");
}

#[test]
fn verbatim_corpus_folds_byte_for_byte() {
    for source in VERBATIM_CORPUS {
        check(source, &SubstitutionGroup::Verbatim);
    }
}

// ─── Structural assertions ──────────────────────────────────────────────────
//
// Net-new guarantees the golden HTML string cannot express: node kinds and
// nesting. These confirm the tree is a real structural artifact, not merely a
// byte-parity shim.

#[test]
fn plain_text_is_a_single_borrowed_text_node() {
    let tree = check_normal("plain text");

    assert_eq!(tree.len(), 1);
    assert!(matches!(&tree[0], InlineNode::Text { value, .. } if value.as_ref() == "plain text"));
}

#[test]
fn special_character_splits_into_text_and_charref() {
    let tree = check_normal("a < b");

    // "a " → Text, "<" → CharRef, " b" → Text.
    assert_eq!(tree.len(), 3);
    assert!(matches!(&tree[0], InlineNode::Text { .. }));
    assert!(matches!(
        &tree[1],
        InlineNode::CharRef {
            value: CharRef::Special('<'),
            ..
        }
    ));
    assert!(matches!(&tree[2], InlineNode::Text { .. }));
}

#[test]
fn strong_wraps_a_text_child() {
    let tree = check_normal("*bold*");

    assert_eq!(tree.len(), 1);

    let InlineNode::Styled(styled) = &tree[0] else {
        panic!("expected a styled node, got {:?}", tree[0]);
    };

    assert_eq!(styled.variant, StyleVariant::Strong);
    assert_eq!(styled.form, SpanForm::Constrained);
    assert_eq!(styled.children.len(), 1);
    assert!(
        matches!(&styled.children[0], InlineNode::Text { value, .. } if value.as_ref() == "bold")
    );
}

#[test]
fn nested_formatting_is_a_tree() {
    // `*a _b_ c*` → Strong[ Text("a "), Emphasis[ Text("b") ], Text(" c") ].
    let tree = check_normal("*a _b_ c*");

    let InlineNode::Styled(strong) = &tree[0] else {
        panic!("expected strong, got {:?}", tree[0]);
    };

    assert_eq!(strong.variant, StyleVariant::Strong);
    assert_eq!(strong.children.len(), 3);

    let InlineNode::Styled(em) = &strong.children[1] else {
        panic!("expected nested emphasis, got {:?}", strong.children[1]);
    };

    assert_eq!(em.variant, StyleVariant::Emphasis);
    assert_eq!(em.children.len(), 1);
    assert!(matches!(&em.children[0], InlineNode::Text { value, .. } if value.as_ref() == "b"));
}

#[test]
fn unconstrained_form_is_recorded() {
    let tree = check_normal("a**b**c");

    let InlineNode::Styled(strong) = &tree[1] else {
        panic!("expected strong at index 1, got {:?}", tree);
    };

    assert_eq!(strong.variant, StyleVariant::Strong);
    assert_eq!(strong.form, SpanForm::Unconstrained);
}

#[test]
fn role_shorthand_is_captured_on_the_span() {
    let tree = check_normal("[.myrole]#text#");

    let InlineNode::Styled(styled) = &tree[0] else {
        panic!("expected styled, got {:?}", tree[0]);
    };

    assert!(
        styled.roles.iter().any(|r| r.as_ref() == "myrole"),
        "roles were {:?}",
        styled.roles
    );
}

#[test]
fn character_replacement_is_a_charref() {
    let tree = check_normal("(C)");

    assert_eq!(tree.len(), 1);
    assert!(matches!(
        &tree[0],
        InlineNode::CharRef {
            value: CharRef::Replacement(_),
            ..
        }
    ));
}

#[test]
fn inline_image_is_an_image_node() {
    let tree = check_normal("image:photo.png[Alt Text]");

    let image = tree
        .iter()
        .find_map(|n| match n {
            InlineNode::Image(image) => Some(image),
            _ => None,
        })
        .expect("expected an image node");

    assert_eq!(image.target.as_ref(), "photo.png");
    assert_eq!(image.alt.as_deref(), Some("Alt Text"));
}

#[test]
fn unresolved_xref_is_a_ref_node() {
    let tree = check_normal("see <<target>> here");

    let reference = tree
        .iter()
        .find_map(|n| match n {
            InlineNode::Ref(reference) => Some(reference),
            _ => None,
        })
        .expect("expected a ref node");

    assert_eq!(reference.variant, RefVariant::Xref);
    assert_eq!(reference.target.as_ref(), "target");
}

#[test]
fn footnote_marker_is_a_footnote_node() {
    let tree = check_normal("A claim.footnote:[evidence]");

    let footnote = tree
        .iter()
        .find_map(|n| match n {
            InlineNode::Footnote(footnote) => Some(footnote),
            _ => None,
        })
        .expect("expected a footnote node");

    assert!(!footnote.is_reference);
    assert_eq!(footnote.number.as_deref(), Some("1"));
}

#[test]
fn keyboard_macro_is_a_ui_node() {
    let tree = check_normal("Press kbd:[Ctrl+T].");

    let ui = tree
        .iter()
        .find_map(|n| match n {
            InlineNode::Ui(ui) => Some(ui),
            _ => None,
        })
        .expect("expected a ui node");

    let UiKind::Keyboard(keys) = &ui.kind else {
        panic!("expected a keyboard macro, got {:?}", ui.kind);
    };

    assert_eq!(keys.len(), 2);
    assert_eq!(keys[0].as_ref(), "Ctrl");
    assert_eq!(keys[1].as_ref(), "T");
}

#[test]
fn line_break_is_a_linebreak_node() {
    let tree = check("first +\nsecond", &SubstitutionGroup::Normal);

    assert!(
        tree.iter()
            .any(|n| matches!(n, InlineNode::LineBreak { .. })),
        "expected a line-break node in {tree:?}"
    );
}

#[test]
fn callout_in_verbatim_is_a_callout_node() {
    let tree = check("some code <1>", &SubstitutionGroup::Verbatim);

    assert!(
        tree.iter().any(|n| matches!(n, InlineNode::Callout(_))),
        "expected a callout node in {tree:?}"
    );
}

// ─── Wiring: the tree as a live artifact on `Content` ────────────────────────
//
// These exercise the Phase 2 wiring, now unconditional: every parse builds each
// `Content`'s tree, so `Content::inlines()` is populated for real blocks (with
// document counters numbered correctly) without a caller opting in.

/// Collects the first simple block's content from a parsed document.
fn first_simple_inlines<'a>(doc: &'a crate::Document<'a>) -> &'a [InlineNode<'a>] {
    use crate::blocks::Block;

    for block in doc.child_blocks() {
        if let Block::Simple(simple) = block {
            return simple.content().inlines();
        }
    }

    panic!("no simple block found");
}

/// The rendered (string-pipeline) content of the first simple block in `doc` —
/// the counterpart of [`first_simple_inlines`], for a test that pins both
/// views of the same content.
fn first_simple_rendered<'a>(doc: &'a crate::Document<'a>) -> &'a str {
    use crate::blocks::Block;

    for block in doc.child_blocks() {
        if let Block::Simple(simple) = block {
            return simple.content().rendered_html();
        }
    }

    panic!("no simple block found");
}

/// Every construct whose tree the builder can produce without consulting a
/// renderer — which, after the two passthrough-form increments, is everything
/// except the two shapes that run an arbitrary substitution list and the
/// footnote, whose catalog entry the build has to *render* to register (see
/// [`a_footnotes_catalog_entry_is_rendered_while_building`]).
const RENDERER_FREE_CONSTRUCTS: &[&str] = &[
    "a < b",
    "++a < b++",
    "+++a < b+++",
    "$$a < b$$",
    "+a < b+",
    "pass:[a < b]",
    "[.role]#a < b#",
    "[.a<b]#x#",
    "[x-]++a < b++",
    "image:x.png[a < b]",
    "link:x.html[a < b]",
    "<<tgt,a < b>>",
    "kbd:[Ctrl+T] and a < b",
    "a < b -- c (C) d",
];

/// A renderer that reports, through its own output, how many times it has been
/// called — the only way to observe an `Rc<dyn InlineSubstitutionRenderer>`,
/// which cannot be downcast to read a counter field.
#[derive(Debug, Default)]
struct OrdinalRenderer {
    calls: std::cell::Cell<usize>,
}

impl crate::parser::InlineSubstitutionRenderer for OrdinalRenderer {
    fn render_special_character(&self, _type_: crate::parser::SpecialCharacter, dest: &mut String) {
        self.calls.set(self.calls.get() + 1);
        dest.push_str(&format!("[{}]", self.calls.get()));
    }
}

/// Builds `source`'s tree and returns how many times the build consulted
/// `parser`'s renderer.
fn renderer_calls_while_building(source: &str) -> usize {
    let parser = Parser::default().with_inline_substitution_renderer(OrdinalRenderer::default());

    let _ = crate::content::inline_builder::build(crate::Span::new(source), &parser, None);

    let mut probe = String::new();
    parser
        .renderer
        .render_special_character(crate::parser::SpecialCharacter::Lt, &mut probe);

    // The probe is itself a call, so its ordinal is one more than the build's.
    probe
        .trim_matches(|c: char| !c.is_ascii_digit())
        .parse::<usize>()
        .expect("the probe reports its own ordinal")
        - 1
}

#[test]
fn building_the_tree_does_not_consult_the_documents_renderer() {
    // The property this increment has to hold, and the one the review of its
    // first attempt found broken: the tree is a *derived* artifact, so building
    // it must not be observable. A renderer with state is how "observable" gets
    // measured — a call the build makes lands between the string pipeline's,
    // and a *later block's* authoritative output shifts under a renderer that
    // has done nothing wrong.
    //
    // Measured at `build` rather than at `parse`, deliberately. The string
    // pipeline legitimately calls the renderer, and for some constructs more
    // than once: an unresolved cross-reference renders its fallback template
    // repeatedly, so `<<tgt,a < b>>` reaches five calls in a full parse — with
    // *and without* a tree, checked both ways. Counting a whole parse would
    // therefore pin the string pipeline's own behavior and call it this
    // increment's. Counting the build alone asks the one question that matters:
    // does building add any?
    for source in RENDERER_FREE_CONSTRUCTS {
        assert_eq!(
            renderer_calls_while_building(source),
            0,
            "building the tree for {source:?} consulted the renderer"
        );
    }
}

#[test]
fn the_three_non_specialcharacters_bodies_still_consult_the_renderer() {
    // The documented complement, pinned so it is a decision rather than a gap.
    // Three shapes, each for the same underlying reason: their value is not a
    // *specialcharacters* body, so no `RawForm` describes it and it has to be
    // computed at build time, against the parse's renderer.
    //
    //   - `pass:c,q[…]` and `stem:[…]` each run an **arbitrary substitution list**,
    //     which is a whole pipeline.
    //   - a bare `+…+` body enclosing an already-extracted construct interleaves
    //     escaped text with that construct's own **fold** bytes, which is a mixture
    //     rather than a form.
    //
    // All three owe `render_with` the same debt every frozen value on this
    // branch does (design §3.3.1), and when that lands this test is what should
    // start failing.
    for source in ["pass:c[a < b]", "stem:[x < y]", "+a $$b < c$$ d+"] {
        let calls = renderer_calls_while_building(source);

        assert!(
            calls > 0,
            "expected {source:?} to still consult the renderer while building, got {calls}"
        );
    }
}

/// Applies `source`'s whole substitution group — both passes and the fold —
/// and returns how many times that consulted `parser`'s renderer.
///
/// The sibling of [`renderer_calls_while_building`], which counts the build
/// alone. This one counts everything, which is the only way to see a body
/// being substituted an extra *time* rather than an extra *way*.
fn renderer_calls_while_applying(source: &str) -> usize {
    let parser = Parser::default().with_inline_substitution_renderer(OrdinalRenderer::default());
    let mut content = crate::content::Content::from(crate::Span::new(source));

    crate::content::SubstitutionGroup::Normal.apply(&mut content, &parser, None);

    let mut probe = String::new();
    parser
        .renderer
        .render_special_character(crate::parser::SpecialCharacter::Lt, &mut probe);

    probe
        .trim_matches(|c: char| !c.is_ascii_digit())
        .parse::<usize>()
        .expect("the probe reports its own ordinal")
        - 1
}

#[test]
fn a_passthrough_body_is_substituted_once_per_apply() {
    // A passthrough body reaches the renderer exactly once per `apply`,
    // pinned.
    //
    // This began as the guard on `Parser::in_inline_build`: a passthrough
    // carrying its own substitution list re-entered `SubstitutionGroup::apply`
    // for its body, and without the guard that nested call took a tree seed of
    // its own, substituting the body twice — once by its string pipeline and
    // once by its builder.
    //
    // The authoritative-pass closure removed the re-entry rather than the
    // double pass: `passthrough_text` builds and folds the body's own tree, so
    // there is no nested `apply` left to seed. The property this measures is
    // unchanged and still worth pinning — it is what says the closure did not
    // quietly add a pass — but it now holds *structurally* rather than by a
    // guard, which is why removing `in_inline_build`'s check no longer moves
    // these counts.
    //
    // Nothing stateless could see the difference either way: the extra pass
    // produced the same bytes. A *stateful* renderer can, which is what
    // `OrdinalRenderer` is for — under the old arrangement, dropping the guard
    // took each count below from three to four and shifted the ordinal
    // reaching the output (`a [3] b` became `a [4] b`).
    //
    // The exact number is not the claim; that the three agree is.
    for source in ["pass:c[a < b]", "pass:c,q[*a < b*]", "stem:[x < y]"] {
        assert_eq!(
            renderer_calls_while_applying(source),
            3,
            "{source:?} was substituted more times than the seam should ask for"
        );
    }

    // The bare-`+` mixture body is the control: it reaches the renderer through
    // a different path (its value interleaves escaped text with an extracted
    // construct's own fold, rather than re-entering `apply`), so it is *not*
    // moved by the guard. Without it, this fixture alone stays at three while
    // the three above go to four — which is what says the three are measuring
    // the re-entry rather than something ambient.
    assert_eq!(renderer_calls_while_applying("+a $$b < c$$ d+"), 3);
}

#[test]
fn a_footnotes_catalog_entry_is_rendered_while_building() {
    // The fourth build-time renderer consumer, and the only one that is not a
    // frozen-value debt: a footnote's *catalog entry*.
    //
    // Every other recognition side effect on this branch is staged and
    // replayed after the build, which is what lets the build stay
    // unobservable. A footnote's cannot be. Its entry is registered at
    // recognition — that is where the `footnote-number` counter advances, and
    // where a later `footnote:id[]` in the same content has to find the id
    // already registered — and the entry's payload is a *rendered* string, so
    // registering it and rendering it are one act. The string pipeline's
    // replacer does the same thing at the same moment, cutting already-
    // rendered bytes out of the string it is substituting.
    //
    // Pinned rather than merely allowed: a build that stopped rendering here
    // would go back to registering the raw match string, in which an
    // already-recognized construct is one opaque placeholder codepoint — the
    // divergence `inline_builder_side_effect_parity`'s footnote sweep exists
    // to catch.
    let calls = renderer_calls_while_building("footnote:[a < b]");

    assert!(
        calls > 0,
        "expected a footnote's entry to be rendered while building, got {calls} renderer calls"
    );

    // A footnote whose text needs no escaping needs no renderer either: the
    // fold routes only `<`, `>` and `&` through it. Without this the assertion
    // above would pass for any footnote at all, and would keep passing if the
    // fold were replaced by something that merely touched the renderer once.
    assert_eq!(
        renderer_calls_while_building("footnote:[plain text]"),
        0,
        "a footnote with nothing to escape must not consult the renderer"
    );
}

#[test]
fn inline_tree_is_built_by_default() {
    // The `with_inline_tree` opt-in is retired: a plain `Parser::default()`
    // builds the tree, where it used to leave `inlines()` empty.
    let mut parser = Parser::default();
    let doc = parser.parse("One *word* is strong.");

    assert!(
        !first_simple_inlines(&doc).is_empty(),
        "every parse must build the inline tree"
    );
}

#[test]
fn a_parse_that_builds_no_tree_keeps_the_string_pipelines_warnings() {
    // The other half of `build_inline_tree`, and the branch the inversion made
    // reachable in a second way.
    //
    // Since the inversion the seam sends `run_pipeline` to a *clone* whenever a
    // tree is being built, and to the real parser otherwise. "Otherwise" now
    // has two causes: a passthrough body re-entered from inside a build (the
    // reentrancy guard), and a parse configured to build no tree at all —
    // this one. The first runs inside a build whose blanket warning discard it
    // has to be carried across; the second does not, and must not be, or the
    // warnings would be stashed with nothing to hand them back.
    //
    // `with_inline_tree` is retired, so `build_inline_tree` is `true` for every
    // parse the public API can produce and only a crate test reaches this. It
    // is still the configuration the seam reads, and the branch it selects is
    // real, so it is exercised here rather than left to be assumed.
    let mut parser = Parser::default().with_intrinsic_attribute(
        "attribute-missing",
        "warn",
        crate::parser::ModificationContext::Anywhere,
    );

    parser.build_inline_tree = false;

    let doc = parser.parse("A paragraph with {missing} in it.");

    let warnings: Vec<_> = doc
        .warnings()
        .map(|warning| warning.warning.clone())
        .collect();

    assert_eq!(
        warnings,
        [
            crate::warnings::WarningType::SkippingReferenceToMissingAttribute(
                "missing".to_string()
            )
        ],
        "a tree-less parse lost the string pipeline's own warning"
    );

    // And no tree, which is what the configuration says.
    assert!(
        first_simple_inlines(&doc).is_empty(),
        "a parse with `build_inline_tree` off must build no tree"
    );
}

#[test]
fn inline_tree_is_built_for_a_default_parser() {
    let mut parser = Parser::default();
    let doc = parser.parse("One *word* is strong.");

    let tree = first_simple_inlines(&doc);

    // "One " → Text, "*word*" → Styled(Strong), " is strong." → Text.
    assert_eq!(tree.len(), 3);
    assert!(matches!(&tree[0], InlineNode::Text { .. }));

    let InlineNode::Styled(styled) = &tree[1] else {
        panic!("expected a styled node, got {:?}", tree[1]);
    };

    assert_eq!(styled.variant, StyleVariant::Strong);
    assert!(matches!(&tree[2], InlineNode::Text { .. }));
}

// ─── Public API: `IsBlock::inlines()` ───────────────────────────────────────
//
// The public inline accessor (design Phase 3) is the structured counterpart of
// `IsBlock::rendered_html_content()`: the same content-bearing blocks carry
// each. A content-bearing block returns `Some(tree)` (an empty tree when the
// flag is off); a block with no directly-contained inline content returns
// `None`.

#[test]
fn is_block_inlines_exposes_the_content_tree() {
    use crate::blocks::{Block, IsBlock};

    let mut parser = Parser::default();
    let doc = parser.parse("One *word* is strong.");

    let block = doc.child_blocks().next().expect("a block");

    // The trait accessor returns the same tree the block's `Content` carries.
    let via_trait = block.inlines().expect("a content-bearing block");
    let Block::Simple(simple) = block else {
        panic!("expected a simple block, got {block:?}");
    };

    assert_eq!(via_trait, simple.content().inlines());
    assert_eq!(via_trait.len(), 3);
    assert!(matches!(&via_trait[1], InlineNode::Styled(_)));
}

#[test]
fn is_block_inlines_is_some_and_empty_for_empty_content() {
    use crate::blocks::IsBlock;

    // `Some(&[])` and `None` mean different things, and the distinction
    // outlives the retired opt-in: `None` is a block with no direct inline
    // content at all (the test below), while `Some(&[])` is a block that has
    // content whose tree is legitimately empty. `build_for_group` returns no
    // nodes for empty content, which an empty listing block's body is.
    let mut parser = Parser::default();
    let doc = parser.parse("----\n----");

    let block = doc.child_blocks().next().expect("a block");

    assert_eq!(block.inlines(), Some(&[][..]));
}

#[test]
fn is_block_inlines_is_none_for_a_block_without_direct_content() {
    use crate::blocks::IsBlock;

    // A section is a compound block: its inline content lives in its child
    // blocks, so the section itself has no direct tree.
    let mut parser = Parser::default();
    let doc = parser.parse("== Title\n\nBody.");

    let section = doc.child_blocks().next().expect("a section block");

    assert_eq!(section.inlines(), None);
}

/// Names the [`Block`](crate::blocks::Block) variant, so a test can assert its
/// fixture reaches every one.
fn block_variant_name(block: &crate::blocks::Block<'_>) -> &'static str {
    use crate::blocks::Block;

    match block {
        Block::Simple(_) => "Simple",
        Block::Media(_) => "Media",
        Block::Section(_) => "Section",
        Block::List(_) => "List",
        Block::ListItem(_) => "ListItem",
        Block::RawDelimited(_) => "RawDelimited",
        Block::CompoundDelimited(_) => "CompoundDelimited",
        Block::Admonition(_) => "Admonition",
        Block::Quote(_) => "Quote",
        Block::Table(_) => "Table",
        Block::Preamble(_) => "Preamble",
        Block::Break(_) => "Break",
        Block::Toc(_) => "Toc",
        Block::DocumentAttribute(_) => "DocumentAttribute",
    }
}

#[test]
fn is_block_inlines_matches_rendered_html_content_across_every_block_kind() {
    use std::collections::BTreeSet;

    use crate::blocks::{FindBlocks, IsBlock};

    // A document exercising all fourteen `Block` variants: a title + section
    // give a `Preamble` (wrapping everything before the section) and a
    // `Section`; the leaf/container blocks cover the rest. Each is walked with
    // the inline-tree flag on so `inlines()` is populated for the
    // content-bearing kinds.
    let mut parser = Parser::default();
    let doc = parser.parse(concat!(
        "= Doc Title\n\n",
        "Preamble *para*.\n\n",
        "NOTE: A _note_.\n\n",
        "[verse]\n____\nA `verse`.\n____\n\n",
        "----\nlisting\n----\n\n",
        "====\nexample\n====\n\n",
        "* item one\n* item two\n\n",
        "|===\n|cell\n|===\n\n",
        "'''\n\n",
        "image::pic.png[Alt]\n\n",
        "toc::[]\n\n",
        ":bodyattr: v2\n\n",
        "== A Section\n\nSection body.\n",
    ));

    let mut seen = BTreeSet::new();

    for block in doc.descendant_blocks() {
        seen.insert(block_variant_name(block));

        // `inlines()` is the structured counterpart of `rendered_html_content()`:
        // the same content-bearing blocks carry each, so their presence agrees
        // for every kind (this also drives every `Block::inlines()` dispatch
        // arm and each block type's override).
        assert_eq!(
            block.inlines().is_some(),
            block.rendered_html_content().is_some(),
            "`inlines()`/`rendered_html_content()` presence disagree for a {} block",
            block_variant_name(block),
        );
    }

    // The fixture must actually reach every variant, or the walk above proves
    // nothing about the arms it never visited.
    let expected: BTreeSet<&str> = [
        "Simple",
        "Media",
        "Section",
        "List",
        "ListItem",
        "RawDelimited",
        "CompoundDelimited",
        "Admonition",
        "Quote",
        "Table",
        "Preamble",
        "Break",
        "Toc",
        "DocumentAttribute",
    ]
    .into_iter()
    .collect();

    assert_eq!(
        seen, expected,
        "the fixture did not reach every block variant"
    );
}

#[test]
fn is_block_inlines_carries_a_real_tree_for_each_content_bearing_kind() {
    use crate::blocks::{Block, FindBlocks, IsBlock};

    // Beyond `Some` vs `None`, the override bodies for the content-bearing block
    // kinds must return the *actual* parsed tree — so a formatting construct in
    // each shows up as a `Styled` node, not an empty slice.
    let mut parser = Parser::default();
    let doc = parser.parse(concat!(
        "NOTE: A _note_.\n\n",
        "[verse]\n____\nA `verse`.\n____\n\n",
        "----\nlisting\n----\n",
    ));

    let mut admonition_styled = None;
    let mut quote_styled = None;
    let mut listing_tree_len = None;

    for block in doc.descendant_blocks() {
        let Some(tree) = block.inlines() else {
            continue;
        };

        match block {
            Block::Admonition(_) => {
                admonition_styled = Some(has_styled(tree, StyleVariant::Emphasis))
            }
            Block::Quote(_) => quote_styled = Some(has_styled(tree, StyleVariant::Code)),
            Block::RawDelimited(_) => listing_tree_len = Some(tree.len()),
            _ => {}
        }
    }

    assert_eq!(
        admonition_styled,
        Some(true),
        "the admonition tree lost its `_note_` emphasis"
    );
    assert_eq!(
        quote_styled,
        Some(true),
        "the verse tree lost its `verse` code span"
    );

    // A listing applies verbatim subs, so its tree is a single text run — but a
    // populated one, proving the `RawDelimited` override returns the parse.
    assert_eq!(
        listing_tree_len,
        Some(1),
        "the listing tree should be one text run"
    );
}

/// Whether `nodes` contains a top-level [`Styled`](InlineNode::Styled) node of
/// the given variant.
fn has_styled(nodes: &[InlineNode<'_>], variant: StyleVariant) -> bool {
    nodes
        .iter()
        .any(|node| matches!(node, InlineNode::Styled(styled) if styled.variant == variant))
}

#[test]
fn inline_tree_honors_a_blocks_custom_subs_list() {
    // A `subs="quotes"` paragraph runs only the quotes step, so the
    // group-aware tree builder must mirror the string pipeline's own step
    // selection: the tree carries a `Styled` node for `*bold*`, while `<`
    // stays inside a plain `Text` run (no `CharRef` — the special-characters
    // step never ran) exactly as the rendered string leaves it unescaped.
    let mut parser = Parser::default();
    let doc = parser.parse("[subs=quotes]\nkeep *bold* and a < b\n");

    let tree = first_simple_inlines(&doc);

    assert!(
        has_styled(tree, StyleVariant::Strong),
        "the quotes step did not run for the custom subs list: {tree:?}"
    );

    assert!(
        !tree
            .iter()
            .any(|node| matches!(node, InlineNode::CharRef { .. })),
        "the special-characters step ran despite being absent from the subs list: {tree:?}"
    );
}

#[test]
fn inline_tree_for_none_group_content_is_the_untouched_seed() {
    // A `[pass]`-style paragraph applies no substitutions at all, so its tree
    // is the untouched seed: the content exactly as written, mirroring the
    // string pipeline leaving the text unchanged. It is not a *single* `Text`
    // run, though: because no `SpecialCharacters` step ever acted on it, each
    // literal `<`/`>`/`&` is a `Raw` leaf the fold emits verbatim rather than
    // a `Text` character the fold would escape (design §3.4.1).
    let mut parser = Parser::default();
    let doc = parser.parse("[pass]\n<b>raw</b> and *not bold*\n");

    let tree = first_simple_inlines(&doc);

    let rejoined: String = tree
        .iter()
        .map(|node| match node {
            InlineNode::Text { value, .. } | InlineNode::Raw { value, .. } => value.as_ref(),
            other => panic!("expected only text and raw-special leaves, got {other:?}"),
        })
        .collect();

    assert_eq!(
        rejoined, "<b>raw</b> and *not bold*",
        "expected the untouched seed: {tree:?}"
    );

    assert!(
        tree.iter()
            .any(|node| matches!(node, InlineNode::Raw { value, .. } if value.as_ref() == "<")),
        "expected the unescaped `<` to be a Raw leaf: {tree:?}"
    );

    // The rendered string is the string pipeline's own, unescaped output — the
    // invariant the classification above exists to let the fold reproduce.
    assert_eq!(first_simple_rendered(&doc), "<b>raw</b> and *not bold*");
}

#[test]
fn inline_tree_for_a_listing_block_carries_callout_nodes() {
    use crate::blocks::{Block, FindBlocks, IsBlock};

    // A listing block applies the verbatim group (special characters, then
    // callouts), so its tree must carry a `Callout` node for the trailing
    // `<1>` — driving the group-aware builder's `Callouts` dispatch through a
    // real parse.
    let mut parser = Parser::default();
    let doc = parser.parse("----\ncode line <1>\n----\n");

    let listing = doc
        .descendant_blocks()
        .find(|block| matches!(block, Block::RawDelimited(_)))
        .expect("a listing block");

    let tree = listing.inlines().expect("a content-bearing block");

    assert!(
        tree.iter()
            .any(|node| matches!(node, InlineNode::Callout(_))),
        "expected a callout node in the listing tree: {tree:?}"
    );
}

#[test]
fn the_xref_mirror_correlates_the_form_that_used_to_defer() {
    // The counterpart of the deferral divergence (design §5.2's step 6), seen
    // from resolution.
    //
    // `xref:sec[a *b, c* d,role=hl]` used to be deferred by the builder,
    // because the string replacer splits the attribute list over the span's
    // **markup** and the `<strong>b, c</strong>` it writes hides a comma the
    // split reads. The tree then held *fewer* cross-reference nodes than the
    // pipeline deferred, the positional mirror detected the count mismatch and
    // skipped, and this content stayed on the template path carrying the
    // replacer's own answer — an anchor cut short inside the tag.
    //
    // The tree's reading stands now, so the counts agree, the mirror
    // correlates, and the rendering is the fold's: a whole anchor with the span
    // intact inside it and the role where the author put it.
    let mut parser = Parser::default();
    let doc = parser.parse("[[sec]]The target.\n\nSee xref:sec[a *b, c* d,role=hl].");

    let rendered = collect_rendered(&doc);
    assert!(
        rendered
            .iter()
            .any(|s| s.contains(r##"<a href="#sec" class="hl">a <strong>b, c</strong> d</a>"##)),
        "the xref did not resolve through the tree: {rendered:?}"
    );

    // The malformed shape the replacer produces, pinned as *absent* — this is
    // the divergence, and it is the whole reason the deferral went.
    assert!(
        !rendered.iter().any(|s| s.contains("<strong>b</a>")),
        "the replacer's cut-short anchor came back: {rendered:?}"
    );

    // And the tree carries the node the mirror correlated against.
    let refs = collect_refs(&doc);
    assert!(
        refs.iter().any(|r| r.variant == RefVariant::Xref),
        "expected the form to be recognized in the tree: {refs:?}"
    );
}
#[test]
fn a_title_resolves_the_form_that_used_to_defer_on_its_own_path() {
    // The deferral divergence crossed with the **title** container, which
    // resolves on a path of its own: `title_refs::compute` coordinates
    // cross-title references and re-derives the ordered destinations itself,
    // rather than going through `Content::resolve_references`. Both paths have
    // to agree about this form, and a corpus covering it only on paragraphs (as
    // this one did) would never say so.
    //
    // Same form as `the_xref_mirror_correlates_the_form_that_used_to_defer`,
    // written as a block title. It used to keep the replacer's cut-short anchor
    // through the carve-out; the tree's reading stands on this path too.
    let mut parser = Parser::default();
    let doc = parser.parse("[[sec]]The target.\n\n.See xref:sec[a *b, c* d,role=hl]\nA paragraph.");

    // `collect_rendered` walks block titles as well as block content.
    let titles = collect_rendered(&doc);

    assert!(
        titles
            .iter()
            .any(|t| t.contains(r##"<a href="#sec" class="hl">a <strong>b, c</strong> d</a>"##)),
        "the title did not resolve through the tree: {titles:?}"
    );

    assert!(
        titles.iter().all(|t| !t.contains("<strong>b</a>")),
        "the replacer's cut-short anchor came back in a title: {titles:?}"
    );

    assert!(
        titles
            .iter()
            .all(|t| !t.contains('\u{E000}') && !t.contains('\u{E001}')),
        "a placeholder sentinel reached a rendered title: {titles:?}"
    );
}
#[test]
fn an_index_term_macro_hiding_a_reference_keeps_the_string_pipelines_rendering() {
    // The carve-out's **other** shape, and the one no test reached before the
    // deferred cross-references were read off the tree.
    //
    // `xref_mirror_is_skipped_when_the_tree_defers_a_reference_form` above
    // covers a content whose tree holds *fewer* cross-references than the
    // string pipeline deferred by one; this covers the case where it holds
    // **none**, which takes a different branch of `Content::set_tree_xrefs` (an
    // empty derived list, where that one has a short-by-one list) and is the
    // shape that would silently clear the deferred state altogether. Its own
    // family pins the recognition gap under `parse_deferred`
    // (`a_reference_inside_an_index_term_macro_keeps_its_documented_divergence`);
    // what was never crossed with it is a **resolved** parse, which is where
    // the rendering is decided.
    //
    // `indexterm2:[…]`'s shown term comes back from an attribute-list parse
    // rather than from a range of the match string, so the builder keeps it as
    // a string and builds no subtree — there is no node for the `<<b>>` inside
    // it. The string pipeline's answer therefore stands, placeholders and all,
    // and the reference still resolves.
    let mut parser = Parser::default();
    let doc = parser.parse("[[b]]B target.\n\nSee indexterm2:[<<b>>] here.");

    let rendered = collect_rendered(&doc);

    assert!(
        rendered.iter().any(|s| s.contains(r##"<a href="#b">"##)),
        "the carve-out must keep the string pipeline's resolved rendering: {rendered:?}"
    );

    // And nothing leaks: a cleared deferred state would leave the raw
    // placeholder sentinels in the output instead.
    assert!(
        rendered
            .iter()
            .all(|s| !s.contains('\u{E000}') && !s.contains('\u{E001}')),
        "a placeholder sentinel reached the rendered output: {rendered:?}"
    );

    // The tree holds no cross-reference node for it, which is *why* this
    // content is on the carve-out path. Asserting it here keeps the test
    // honest the day the builder learns the macro spelling: this fails, and
    // the fixture graduates to the ordinary folded path.
    let refs = collect_refs(&doc);
    assert!(
        refs.iter().all(|r| r.variant != RefVariant::Xref),
        "the macro spelling now yields a node; this fixture should fold: {refs:?}"
    );
}

#[test]
fn a_span_in_a_string_valued_xref_attribute_now_mirrors() {
    // The counterpart the increment *moved*: a rendered span reaching one of
    // the three values this family reads as a **string** used to defer for
    // want of anywhere to put a node. It no longer does — the slot takes the
    // author's own source for the piece — so the construct is recognized, the
    // counts agree, and the mirror installs the destination.
    let mut parser = Parser::default();
    let doc = parser.parse("[[sec]]The target.\n\nSee xref:sec[*bold*,role=*hl*].");

    let refs = collect_refs(&doc);

    assert!(
        refs.iter().any(|r| r.variant == RefVariant::Xref
            && r.roles.iter().any(|role| role.as_ref() == "*hl*")
            && r.resolved.as_ref().is_some_and(|res| res.href == "#sec")),
        "expected a resolved xref carrying the untranslated role: {refs:?}"
    );

    // And the rendering follows the tree rather than the string pipeline's
    // `class="&lt;strong&gt;hl&lt;/strong&gt;"` — the documented divergence.
    let rendered = collect_rendered(&doc);
    assert!(
        rendered
            .iter()
            .any(|s| s.contains("class=\"*hl*\"") && s.contains("<strong>bold</strong>")),
        "the rendered xref regressed: {rendered:?}"
    );
}

#[test]
fn a_footnote_subtree_that_defers_a_reference_form_still_mirrors_what_it_holds() {
    // The footnote-side counterpart, and the one place reading the deferred
    // cross-references **off the tree** does more than reproduce the string
    // pipeline's answer.
    //
    // One of the footnote's two embedded cross-references is a documented
    // builder divergence, so the footnote subtree holds one node where the
    // string pipeline re-homed two segments. That used to make the footnote
    // mirror skip: its list came from the flat, placeholder-indexed list, whose
    // footnote half (2) could not be correlated positionally against the
    // subtree's nodes (1), so the recognized `<<c>>` was left honestly
    // unresolved rather than misassigned.
    //
    // The tree's own walk partitions structurally, so the footnote list holds
    // exactly the nodes it belongs to and the correlation is exact: the
    // recognized `<<c>>` now carries its real destination, and the
    // unrecognized sibling simply is not in either list. Nothing rendered
    // changes — a fold emits a footnote's marker without descending into its
    // subtree — so what moves is only what a consumer reading `inlines()`
    // sees, in the direction of being right.
    //
    // (The deferred form here is a shorthand whose *id* crosses an opaque
    // piece — a masked passthrough — since a footnote's own bracketed text
    // ends at the first `]`, which rules out the bracket-carrying spellings.)
    let mut parser = Parser::default();
    let doc = parser.parse(
        "[[a]]A target.\n\n[[c]]Another.\n\nSee <<c>>.footnote:[see <<a +++b+++ c>> and <<c>>]",
    );

    let refs = collect_refs(&doc);

    // The block-level `<<c>>` still mirrors its resolved destination …
    assert!(
        refs.iter().any(|r| r.variant == RefVariant::Xref
            && r.resolved.as_ref().is_some_and(|res| res.href == "#c")),
        "the block-level xref should still resolve: {refs:?}"
    );

    // … and the footnote's recognized `<<c>>` carries its own destination,
    // where the flat list's un-correlatable footnote half used to leave it
    // unresolved. The unrecognized sibling contributes no node, so there is
    // exactly one to check.
    let footnote_refs = collect_footnote_refs(&doc);
    assert_eq!(
        footnote_refs.len(),
        1,
        "expected exactly the footnote's recognized xref node: {footnote_refs:?}"
    );
    assert_eq!(
        footnote_refs[0].resolved.as_ref().map(|r| r.href.as_str()),
        Some("#c"),
        "the footnote subtree's own reference should mirror: {footnote_refs:?}"
    );

    // The block's own rendering is unaffected either way, which is what makes
    // the change above a tree-fidelity one rather than an output one: a
    // footnote's text is extracted out of the block, so what the block renders
    // for it is the marker, which the fold emits without descending into the
    // subtree at all.
    let rendered = collect_rendered(&doc);
    assert!(
        rendered
            .iter()
            .any(|s| s.contains("href=\"#c\"") && s.contains("class=\"footnote\"")),
        "the rendered block regressed: {rendered:?}"
    );
}

/// Collects every cross-reference/link node found inside footnote subtrees of
/// simple blocks in `doc`.
fn collect_footnote_refs<'a>(doc: &'a crate::Document<'a>) -> Vec<crate::inlines::Ref<'a>> {
    use crate::blocks::Block;

    fn walk<'a>(
        nodes: &[InlineNode<'a>],
        in_footnote: bool,
        out: &mut Vec<crate::inlines::Ref<'a>>,
    ) {
        for node in nodes {
            match node {
                InlineNode::Footnote(footnote) => walk(&footnote.children, true, out),

                InlineNode::Ref(reference) => {
                    if in_footnote {
                        out.push(reference.clone());
                    }

                    walk(&reference.children, in_footnote, out);
                }

                InlineNode::Styled(styled) => walk(&styled.children, in_footnote, out),

                _ => {}
            }
        }
    }

    let mut out = vec![];

    for block in doc.child_blocks() {
        if let Block::Simple(simple) = block {
            walk(simple.content().inlines(), false, &mut out);
        }
    }

    out
}

#[test]
fn inline_tree_numbers_footnotes_in_document_order() {
    // The recording pass clones the parser *before* the authoritative pass
    // advances the footnote counter, so a footnote in the second paragraph is
    // numbered "2" in its tree — matching `rendered_html()` — rather than
    // restarting at "1" (which a fresh-parser recording pass would produce).
    let mut parser = Parser::default();
    let doc =
        parser.parse("Body text.footnote:[the first note]\n\nMore text.footnote:[the second note]");

    use crate::blocks::Block;

    let numbers: Vec<String> = doc
        .child_blocks()
        .filter_map(|block| match block {
            Block::Simple(simple) => Some(simple),
            _ => None,
        })
        .flat_map(|simple| simple.content().inlines().to_vec())
        .filter_map(|node| match node {
            InlineNode::Footnote(footnote) => {
                footnote.number.as_ref().map(|n| n.as_ref().to_string())
            }
            _ => None,
        })
        .collect();

    assert_eq!(numbers, vec!["1".to_string(), "2".to_string()]);
}

// ─── Wiring: cross-reference resolution reaches the tree ─────────────────────
//
// After a full parse (which resolves references against the document's own
// catalog), a cross-reference in the inline tree carries the same resolved
// destination the rendered string reflects — so a consumer that walks
// `inlines()` sees resolved xrefs, not just the parse-time `resolved: None`
// state the tree is first built with (design §4.3).

/// Collects every [`Ref`](InlineNode::Ref) node from the simple blocks of
/// `doc`, recursing into formatting spans and reference children.
fn collect_refs<'a>(doc: &'a crate::Document<'a>) -> Vec<crate::inlines::Ref<'a>> {
    use crate::blocks::Block;

    fn walk<'a>(nodes: &[InlineNode<'a>], out: &mut Vec<crate::inlines::Ref<'a>>) {
        for node in nodes {
            match node {
                InlineNode::Ref(reference) => {
                    out.push(reference.clone());
                    walk(&reference.children, out);
                }

                InlineNode::Styled(styled) => walk(&styled.children, out),
                InlineNode::Footnote(footnote) => walk(&footnote.children, out),
                _ => {}
            }
        }
    }

    let mut out = vec![];

    for block in doc.child_blocks() {
        if let Block::Simple(simple) = block {
            walk(simple.content().inlines(), &mut out);
        }
    }

    out
}

#[test]
fn inline_tree_xref_is_resolved_after_parse() {
    // The target is anchored in the first paragraph and referenced in the
    // second; after the parse's own-catalog resolution, the tree's xref node
    // carries the resolved `#tgt` destination.
    let mut parser = Parser::default();
    let doc = parser.parse("[[tgt]]The target paragraph.\n\nSee <<tgt>> for details.");

    let refs = collect_refs(&doc);
    let reference = refs
        .iter()
        .find(|r| r.variant == RefVariant::Xref)
        .expect("expected an xref node");

    let resolved = reference
        .resolved
        .as_ref()
        .expect("the xref should be resolved after parse");

    assert_eq!(resolved.href, "#tgt");
}

#[test]
fn inline_tree_xref_inside_formatting_is_resolved() {
    // The recursion reaches an xref nested inside a formatting span, so a
    // `Ref` node that lives under a `Styled` node is resolved too.
    let mut parser = Parser::default();
    let doc = parser.parse("[[tgt]]The target.\n\nSee *the <<tgt>> link* here.");

    let refs = collect_refs(&doc);
    let reference = refs
        .iter()
        .find(|r| r.variant == RefVariant::Xref)
        .expect("expected an xref node nested in formatting");

    assert_eq!(
        reference
            .resolved
            .as_ref()
            .expect("nested xref should be resolved")
            .href,
        "#tgt"
    );
}

#[test]
fn inline_tree_unresolved_xref_stays_none() {
    // A reference whose target has no catalog entry is left unresolved in the
    // tree, mirroring the unresolved-fallback the rendered string shows.
    let mut parser = Parser::default();
    let doc = parser.parse("See <<missing>> here.");

    let refs = collect_refs(&doc);
    let reference = refs
        .iter()
        .find(|r| r.variant == RefVariant::Xref)
        .expect("expected an xref node");

    assert!(
        reference.resolved.is_none(),
        "an unresolvable target must not carry a resolved destination"
    );
}

#[test]
fn inline_tree_resolution_walks_past_a_footnote_and_a_link() {
    // A block carrying a resolvable cross-reference alongside a footnote and a
    // link exercises the other arms of the resolution walk: the xref resolves,
    // the walk recurses into the footnote node's (currently empty) subtree
    // without disturbing it, and a link (a `Ref` of variant `Link`) passes
    // through untouched because only an `Xref` has a catalog destination.
    let mut parser = Parser::default();
    let doc = parser
        .parse("[[tgt]]The target.\n\nSee <<tgt>> and https://example.org[x].footnote:[a note]");

    use crate::blocks::Block;

    let tree: Vec<InlineNode<'_>> = doc
        .child_blocks()
        .filter_map(|block| match block {
            Block::Simple(simple) => Some(simple),
            _ => None,
        })
        .flat_map(|simple| simple.content().inlines().to_vec())
        .collect();

    // The cross-reference in the same block still resolves ...
    let refs = collect_refs(&doc);
    assert_eq!(
        refs.iter()
            .find(|r| r.variant == RefVariant::Xref)
            .and_then(|r| r.resolved.as_ref())
            .map(|resolved| resolved.href.as_str()),
        Some("#tgt")
    );

    // ... the link (a `Ref` of variant `Link`) is left untouched by resolution ...
    let link = refs
        .iter()
        .find(|r| r.variant == RefVariant::Link)
        .expect("expected a link node");
    assert!(
        link.resolved.is_none(),
        "a link has no catalog destination and must stay unresolved"
    );

    // ... and the footnote node survives the walk.
    assert!(
        tree.iter().any(|n| matches!(n, InlineNode::Footnote(_))),
        "expected a footnote node alongside the resolved xref in {tree:?}"
    );
}

#[test]
fn inline_tree_same_target_refs_keep_per_reference_resolution() {
    // Two references to the same target, resolved by a custom resolver that
    // varies its destination by the reference's provided text. Correlating each
    // tree node with its own segment positionally (rather than keying by target)
    // gives each node its own destination instead of collapsing both onto the
    // first.
    use crate::parser::{
        HtmlSubstitutionRenderer, ReferenceResolver, ResolutionContext, ResolvedReference,
    };

    #[derive(Debug)]
    struct PerTextResolver;

    impl ReferenceResolver for PerTextResolver {
        fn resolve(&self, context: &ResolutionContext<'_>) -> Option<ResolvedReference> {
            // Same target, distinct destination per provided text.
            let href = match context.provided_text {
                Some("first") => "#one",
                Some("second") => "#two",
                _ => "#default",
            };

            Some(ResolvedReference::new(href.to_string(), None))
        }
    }

    let mut parser = Parser::default();
    let mut doc = parser.parse_deferred("See <<tgt,first>> and <<tgt,second>>.");
    doc.resolve_references(&PerTextResolver, &HtmlSubstitutionRenderer {}, &parser);

    let hrefs: Vec<String> = collect_refs(&doc)
        .iter()
        .filter(|r| r.variant == RefVariant::Xref)
        .map(|r| {
            r.resolved
                .as_ref()
                .expect("each xref should resolve")
                .href
                .clone()
        })
        .collect();

    assert_eq!(hrefs, vec!["#one".to_string(), "#two".to_string()]);
}

#[test]
fn inline_tree_xref_resolution_matches_the_rendered_string() {
    // The tree's resolved destination and the rendered HTML are two projections
    // of the same resolution: the `#tgt` fragment in the resolved node is the
    // same href the rendered `<a>` carries.
    let mut parser = Parser::default();
    let doc = parser.parse("[[tgt]]The target.\n\nSee <<tgt,the target>> now.");

    use crate::blocks::Block;

    let rendered = doc
        .child_blocks()
        .filter_map(|block| match block {
            Block::Simple(simple) => Some(simple.content().rendered_html().to_string()),
            _ => None,
        })
        .find(|r| r.contains("<a href"))
        .expect("expected a rendered link");

    assert!(
        rendered.contains("href=\"#tgt\""),
        "rendered string should link to #tgt: {rendered:?}"
    );

    let refs = collect_refs(&doc);
    let reference = refs
        .iter()
        .find(|r| r.variant == RefVariant::Xref)
        .expect("expected an xref node");

    assert_eq!(
        reference
            .resolved
            .as_ref()
            .expect("xref should be resolved")
            .href,
        "#tgt"
    );
}

// The guard lives in `SubstitutionGroup::build_inline_tree` as a
// `debug_assert!`, so both the test and the stateful renderer it needs are
#[test]
fn inline_tree_build_tolerates_a_stateful_renderer() {
    /// A renderer whose output depends on mutable internal state: it emits
    /// different bytes on each invocation. Under the retired Strategy-A
    /// recorder, tree building re-ran the whole pipeline through the shared
    /// renderer instance, so such a renderer poisoned the recorded tree (and a
    /// debug assertion rejected it). The single-pass builder derives the tree
    /// from source instead — a `CharRef` node carries the *logical* character,
    /// not renderer output — so a stateful renderer is now safe: the
    /// authoritative rendered string sees the renderer's stateful bytes, and
    /// the tree stays logical.
    #[derive(Debug, Default)]
    struct FlipRenderer {
        flipped: std::cell::Cell<bool>,
    }

    impl crate::parser::InlineSubstitutionRenderer for FlipRenderer {
        fn render_special_character(
            &self,
            _type_: crate::parser::SpecialCharacter,
            dest: &mut String,
        ) {
            if self.flipped.replace(true) {
                dest.push_str("[second]");
            } else {
                dest.push_str("[first]");
            }
        }
    }

    let mut parser = Parser::default().with_inline_substitution_renderer(FlipRenderer::default());

    let doc = parser.parse("a < b");

    let block = doc.child_blocks().next().unwrap();
    let content = match block {
        crate::blocks::Block::Simple(simple) => simple.content(),
        _ => panic!("expected a simple block"),
    };

    // The authoritative rendered string reflects the stateful renderer's
    // output — the *second* invocation's, for now. `rendered_html()` is the
    // fold of the tree, and the string pipeline still runs ahead of it (it
    // produces the deferred cross-reference segments and fills the catalogs),
    // so this renderer is invoked twice per parse and the fold sees the later
    // state. That is the interim double render, not a property of the fold: it
    // becomes `[first]` again once the string pipeline stops being run for
    // output. Pinned exactly rather than loosely so that change shows up here
    // as a signal.
    assert_eq!(content.rendered_html(), "a [second] b");

    // … while the tree carries the logical special character, unpolluted by
    // the renderer's state.
    let has_logical_charref = content.inlines().iter().any(|node| {
        matches!(
            node,
            InlineNode::CharRef {
                value: CharRef::Special('<'),
                ..
            }
        )
    });
    assert!(
        has_logical_charref,
        "expected a logical CharRef('<') in the tree: {:?}",
        content.inlines()
    );
}

// ─── Wiring: title cross-reference resolution reaches the tree ───────────────
//
// A cross-reference embedded in a section heading or a block `.Title` is owned
// by the separate document-order title pass (`title_refs`), which coordinates
// cross-title references — including forward and circular ones — that the
// per-content pass cannot. That pass now mirrors each resolved destination into
// the title's own inline tree, so a consumer that walks a title's `inlines()`
// sees the same destinations the rendered title reflects — closing the
// section-/block-title follow-up of Phase 2 step 2 (design §4.3).

/// Collects every cross-reference/link node from the inline tree of every
/// section heading in `doc`, recursing into nested sections and into formatting
/// spans and reference children within each title.
fn collect_section_title_refs<'a>(doc: &'a crate::Document<'a>) -> Vec<crate::inlines::Ref<'a>> {
    use crate::blocks::{Block, FindBlocks};

    fn refs_in<'a>(nodes: &[InlineNode<'a>], out: &mut Vec<crate::inlines::Ref<'a>>) {
        for node in nodes {
            match node {
                InlineNode::Ref(reference) => {
                    out.push(reference.clone());
                    refs_in(&reference.children, out);
                }

                InlineNode::Styled(styled) => refs_in(&styled.children, out),
                InlineNode::Footnote(footnote) => refs_in(&footnote.children, out),
                _ => {}
            }
        }
    }

    fn walk<'a>(
        blocks: impl Iterator<Item = &'a Block<'a>>,
        out: &mut Vec<crate::inlines::Ref<'a>>,
    ) {
        for block in blocks {
            if let Block::Section(section) = block {
                refs_in(section.section_title_inlines(), out);
            }

            walk(block.child_blocks(), out);
        }
    }

    let mut out = vec![];
    walk(doc.child_blocks(), &mut out);
    out
}

#[test]
fn section_title_forward_xref_is_resolved_in_the_tree() {
    // The first heading forward-references a section defined later. The title
    // pass resolves it in document order and mirrors the destination into the
    // heading's tree, so the title's `Ref` node carries `#the-end`.
    let mut parser = Parser::default();
    let doc = parser.parse("== See <<the-end>>\n\n[#the-end]\n== The End\n");

    let refs = collect_section_title_refs(&doc);
    let reference = refs
        .iter()
        .find(|r| r.variant == RefVariant::Xref)
        .expect("expected an xref node in a section heading");

    assert_eq!(
        reference
            .resolved
            .as_ref()
            .expect("the title xref should be resolved after parse")
            .href,
        "#the-end"
    );
}

#[test]
fn circular_section_title_xrefs_are_both_resolved_in_the_tree() {
    // Two headings reference each other. The title pass breaks the cycle for the
    // rendered link *text*, but each reference still resolves to its target's
    // destination — and both destinations are now mirrored into the two
    // headings' trees.
    let mut parser = Parser::default();
    let doc = parser.parse("[#a]\n== See <<b>>\n\n[#b]\n== See <<a>>\n");

    let hrefs: Vec<String> = collect_section_title_refs(&doc)
        .iter()
        .filter(|r| r.variant == RefVariant::Xref)
        .map(|r| {
            r.resolved
                .as_ref()
                .expect("each circular title xref should resolve")
                .href
                .clone()
        })
        .collect();

    assert_eq!(hrefs, vec!["#b".to_string(), "#a".to_string()]);
}

#[test]
fn unresolved_section_title_xref_stays_none_in_the_tree() {
    // A heading whose cross-reference names no catalog entry is left unresolved
    // in the tree, mirroring the unresolved fallback the rendered title shows.
    let mut parser = Parser::default();
    let doc = parser.parse("== See <<missing>>\n");

    let refs = collect_section_title_refs(&doc);
    let reference = refs
        .iter()
        .find(|r| r.variant == RefVariant::Xref)
        .expect("expected an xref node in the heading");

    assert!(
        reference.resolved.is_none(),
        "an unresolvable title target must not carry a resolved destination"
    );
}

#[test]
fn block_title_xref_is_resolved_in_the_tree() {
    // A block `.Title` carrying a cross-reference is resolved by the same title
    // pass, which mirrors the destination into the block title's tree — a site
    // the per-content pass never resolved at all.
    let mut parser = Parser::default();
    let doc =
        parser.parse("[[tgt]]The target paragraph.\n\n.See <<tgt>>\nA captioned paragraph.\n");

    use crate::blocks::Block;

    let title = doc
        .child_blocks()
        .filter_map(|block| match block {
            Block::Simple(simple) => simple.title_content(),
            _ => None,
        })
        .find(|content| {
            content
                .inlines()
                .iter()
                .any(|node| matches!(node, InlineNode::Ref(_)))
        })
        .expect("expected a block title carrying a cross-reference");

    let reference = title
        .inlines()
        .iter()
        .find_map(|node| match node {
            InlineNode::Ref(reference) if reference.variant == RefVariant::Xref => Some(reference),
            _ => None,
        })
        .expect("expected an xref node in the block title");

    assert_eq!(
        reference
            .resolved
            .as_ref()
            .expect("the block title xref should be resolved after parse")
            .href,
        "#tgt"
    );
}

#[test]
fn re_resolving_a_title_clears_a_now_unresolved_tree_destination() {
    // Resolving a document twice, the second time with a resolver that no longer
    // recognizes the title's target, must leave the title tree's `Ref` node
    // *unresolved* — not stale at the first pass's destination. The title pass
    // rebuilds the ordered destinations each run (the template, and so the list
    // length, is stable), and the mirror overwrites each slot unconditionally
    // (to `Some` or `None`), so the tree tracks the rendered title rather than
    // retaining a superseded link.
    use crate::parser::{
        HtmlSubstitutionRenderer, ReferenceResolver, ResolutionContext, ResolvedReference,
    };

    /// Resolves every target to a fixed destination, or to nothing when `None`.
    #[derive(Debug)]
    struct FixedResolver(Option<&'static str>);

    impl ReferenceResolver for FixedResolver {
        fn resolve(&self, _context: &ResolutionContext<'_>) -> Option<ResolvedReference> {
            self.0
                .map(|href| ResolvedReference::new(href.to_string(), None))
        }
    }

    /// The resolved `href` of the first cross-reference in any section heading,
    /// scoped so the tree borrow is released before the next resolution.
    fn title_xref_href(doc: &crate::Document<'_>) -> Option<String> {
        collect_section_title_refs(doc)
            .iter()
            .find(|r| r.variant == RefVariant::Xref)
            .and_then(|r| r.resolved.as_ref())
            .map(|resolved| resolved.href.clone())
    }

    let mut parser = Parser::default();
    let mut doc = parser.parse_deferred("== See <<tgt>>\n\n[#tgt]\n== Target\n");

    // First pass: the resolver recognizes the target, so the heading's tree xref
    // carries its destination.
    doc.resolve_references(
        &FixedResolver(Some("#tgt")),
        &HtmlSubstitutionRenderer {},
        &parser,
    );
    assert_eq!(title_xref_href(&doc).as_deref(), Some("#tgt"));

    // Second pass: the resolver no longer recognizes the target, so the tree xref
    // must fall back to unresolved rather than keep the stale destination.
    doc.resolve_references(&FixedResolver(None), &HtmlSubstitutionRenderer {}, &parser);
    assert_eq!(
        title_xref_href(&doc),
        None,
        "a re-resolution that fails must clear the title tree's stale destination"
    );
}

// ─── Wiring: the footnote subtree and its cross-references ──────────────────
//
// A footnote's text is extracted out of the flow of the block during the macros
// substitution step — only its marker is left behind — so it never reaches the
// block's rendered string and the tree could not recover it from that string
// alone. The recording pass now also picks up the footnote texts it registered
// (which carry the recorder's markers, against the same event log) and parses
// each into the footnote node's own child subtree.
//
// That closes the second of the two follow-ups tracked from Phase 2 step 2: a
// cross-reference *inside* a footnote is re-homed out of the block template
// when the footnote text is extracted, so it lives in that subtree, and the
// shared mirror now installs its resolved destination there — the same
// destination the rendered footnote text reflects (design §4.3).

/// The first [`Footnote`](InlineNode::Footnote) node found anywhere in the
/// simple blocks of `doc`, in document order.
fn first_footnote<'a>(doc: &'a crate::Document<'a>) -> crate::inlines::Footnote<'a> {
    use crate::blocks::Block;

    fn find<'a>(nodes: &[InlineNode<'a>]) -> Option<crate::inlines::Footnote<'a>> {
        for node in nodes {
            let found = match node {
                InlineNode::Footnote(footnote) => Some(footnote.clone()),
                InlineNode::Styled(styled) => find(&styled.children),
                InlineNode::Ref(reference) => find(&reference.children),
                _ => None,
            };

            if found.is_some() {
                return found;
            }
        }

        None
    }

    for block in doc.child_blocks() {
        if let Block::Simple(simple) = block
            && let Some(footnote) = find(simple.content().inlines())
        {
            return footnote;
        }
    }

    panic!("no footnote node found");
}

/// Every cross-reference node in `nodes`, recursing through formatting spans,
/// reference children, and footnote subtrees.
fn refs_in<'a>(nodes: &[InlineNode<'a>]) -> Vec<crate::inlines::Ref<'a>> {
    let mut out = vec![];

    fn walk<'a>(nodes: &[InlineNode<'a>], out: &mut Vec<crate::inlines::Ref<'a>>) {
        for node in nodes {
            match node {
                InlineNode::Ref(reference) => {
                    out.push(reference.clone());
                    walk(&reference.children, out);
                }

                InlineNode::Styled(styled) => walk(&styled.children, out),
                InlineNode::Footnote(footnote) => walk(&footnote.children, out),
                _ => {}
            }
        }
    }

    walk(nodes, &mut out);
    out
}

#[test]
fn footnote_carries_its_text_as_child_nodes() {
    // The footnote's text is a subtree, not an opaque string: the formatting
    // inside it is structure the block's rendered string never carries.
    let mut parser = Parser::default();
    let doc = parser.parse("A claim.footnote:[the *strong* evidence]");

    let footnote = first_footnote(&doc);

    assert!(!footnote.is_reference);
    assert_eq!(footnote.number.as_deref(), Some("1"));

    // "the " → Text, "*strong*" → Styled(Strong), " evidence" → Text.
    assert_eq!(footnote.children.len(), 3);
    assert!(
        matches!(&footnote.children[0], InlineNode::Text { value, .. } if value.as_ref() == "the ")
    );

    let InlineNode::Styled(styled) = &footnote.children[1] else {
        panic!("expected a styled node, got {:?}", footnote.children[1]);
    };

    assert_eq!(styled.variant, StyleVariant::Strong);
    assert!(
        matches!(&styled.children[0], InlineNode::Text { value, .. } if value.as_ref() == "strong")
    );

    assert!(
        matches!(&footnote.children[2], InlineNode::Text { value, .. } if value.as_ref() == " evidence")
    );
}

#[test]
fn footnote_subtree_carries_a_nested_macro() {
    // A macro substituted before footnotes (here a link) is captured as part of
    // the footnote's text, so it surfaces as a node of the footnote's subtree
    // rather than being absorbed into the block.
    let mut parser = Parser::default();
    let doc = parser.parse("A claim.footnote:[see link:https://example.org[the source]]");

    let footnote = first_footnote(&doc);

    let reference = footnote
        .children
        .iter()
        .find_map(|node| match node {
            InlineNode::Ref(reference) => Some(reference),
            _ => None,
        })
        .expect("expected a link node inside the footnote subtree");

    assert_eq!(reference.variant, RefVariant::Link);
    assert_eq!(reference.target.as_ref(), "https://example.org");
}

#[test]
fn footnote_reference_keeps_an_empty_subtree() {
    // `footnote:id[]` defines nothing — it re-uses an earlier footnote's number —
    // so it consumes no footnote text and keeps the empty subtree its node type
    // documents, while the defining occurrence carries the text.
    let mut parser = Parser::default();
    let doc = parser.parse("Named.footnote:disc[a *discussion*] then footnote:disc[].");

    use crate::blocks::Block;

    let footnotes: Vec<crate::inlines::Footnote<'_>> = doc
        .child_blocks()
        .filter_map(|block| match block {
            Block::Simple(simple) => Some(simple),
            _ => None,
        })
        .flat_map(|simple| simple.content().inlines().to_vec())
        .filter_map(|node| match node {
            InlineNode::Footnote(footnote) => Some(footnote),
            _ => None,
        })
        .collect();

    assert_eq!(footnotes.len(), 2);

    assert!(!footnotes[0].is_reference);
    assert_eq!(footnotes[0].children.len(), 2);

    assert!(footnotes[1].is_reference);
    assert!(
        footnotes[1].children.is_empty(),
        "a bare reference defines no text: {:?}",
        footnotes[1].children
    );
}

#[test]
fn footnote_subtrees_track_document_order_across_blocks() {
    // Each block's recording pass picks up only the footnotes *that* block
    // defined (the registry length is snapshotted first), so the second
    // paragraph's footnote gets its own text rather than the first one's.
    let mut parser = Parser::default();
    let doc = parser.parse("One.footnote:[first note]\n\nTwo.footnote:[second note]");

    use crate::blocks::Block;

    let texts: Vec<String> = doc
        .child_blocks()
        .filter_map(|block| match block {
            Block::Simple(simple) => Some(simple),
            _ => None,
        })
        .flat_map(|simple| simple.content().inlines().to_vec())
        .filter_map(|node| match node {
            InlineNode::Footnote(footnote) => Some(footnote),
            _ => None,
        })
        .map(|footnote| {
            footnote
                .children
                .iter()
                .filter_map(|child| match child {
                    InlineNode::Text { value, .. } => Some(value.as_ref().to_string()),
                    _ => None,
                })
                .collect::<String>()
        })
        .collect();

    assert_eq!(
        texts,
        vec!["first note".to_string(), "second note".to_string()]
    );
}

#[test]
fn footnote_embedded_xref_is_resolved_in_the_tree() {
    // The cross-reference lives in the footnote's subtree, and its destination
    // is mirrored there from the same resolution sweep that resolved the block's
    // own references — so the tree agrees with the rendered footnote text.
    let mut parser = Parser::default();
    let doc = parser.parse("[[tgt]]The target.\n\nA claim.footnote:[see <<tgt>> for details]");

    let footnote = first_footnote(&doc);

    let reference = refs_in(&footnote.children)
        .into_iter()
        .find(|r| r.variant == RefVariant::Xref)
        .expect("expected an xref node inside the footnote subtree");

    assert_eq!(reference.target.as_ref(), "tgt");
    assert_eq!(
        reference
            .resolved
            .as_ref()
            .expect("the footnote-embedded xref should be resolved after parse")
            .href,
        "#tgt"
    );

    // The rendered footnote text is the other projection of that same
    // resolution, so the two must agree.
    let text = &doc.catalog().footnotes()[0].text;
    assert!(
        text.contains("href=\"#tgt\""),
        "the rendered footnote text should link to #tgt: {text:?}"
    );
}

#[test]
fn footnote_embedded_xref_does_not_displace_block_xrefs() {
    // A footnote-embedded reference is re-homed out of the block template, so it
    // must not consume a slot of the block-level list. Were the block walk to
    // descend into the footnote subtree, the reference *after* the footnote
    // would take the footnote's destination.
    let mut parser = Parser::default();
    let doc = parser.parse(
        "[[one]]First.\n\n[[two]]Second.\n\nSee <<one>>.footnote:[also <<two>>] and <<two>>.",
    );

    use crate::blocks::Block;

    let tree: Vec<InlineNode<'_>> = doc
        .child_blocks()
        .filter_map(|block| match block {
            Block::Simple(simple) => Some(simple),
            _ => None,
        })
        .flat_map(|simple| simple.content().inlines().to_vec())
        .collect();

    // The two block-level references keep their own destinations ...
    let block_hrefs: Vec<String> = tree
        .iter()
        .filter_map(|node| match node {
            InlineNode::Ref(reference) if reference.variant == RefVariant::Xref => reference
                .resolved
                .as_ref()
                .map(|resolved| resolved.href.clone()),
            _ => None,
        })
        .collect();

    assert_eq!(block_hrefs, vec!["#one".to_string(), "#two".to_string()]);

    // ... and the footnote-embedded one gets its own.
    let footnote = first_footnote(&doc);
    let embedded = refs_in(&footnote.children)
        .into_iter()
        .find(|r| r.variant == RefVariant::Xref)
        .expect("expected an xref inside the footnote subtree");

    assert_eq!(
        embedded
            .resolved
            .as_ref()
            .expect("the footnote-embedded xref should resolve")
            .href,
        "#two"
    );
}

#[test]
fn unresolved_footnote_embedded_xref_stays_none() {
    // A footnote-embedded reference whose target has no catalog entry is left
    // unresolved in the subtree, mirroring the unresolved fallback the rendered
    // footnote text shows.
    let mut parser = Parser::default();
    let doc = parser.parse("A claim.footnote:[see <<missing>>]");

    let footnote = first_footnote(&doc);
    let reference = refs_in(&footnote.children)
        .into_iter()
        .find(|r| r.variant == RefVariant::Xref)
        .expect("expected an xref node inside the footnote subtree");

    assert!(
        reference.resolved.is_none(),
        "an unresolvable footnote target must not carry a resolved destination"
    );
}

#[test]
fn re_resolving_clears_a_now_unresolved_footnote_tree_destination() {
    // The footnote mirror overwrites each slot unconditionally, exactly as the
    // block and title mirrors do, so a second resolution that no longer
    // recognizes the target clears the subtree's destination rather than leaving
    // the first pass's stale link behind.
    use crate::parser::{
        HtmlSubstitutionRenderer, ReferenceResolver, ResolutionContext, ResolvedReference,
    };

    /// Resolves every target to a fixed destination, or to nothing when `None`.
    #[derive(Debug)]
    struct FixedResolver(Option<&'static str>);

    impl ReferenceResolver for FixedResolver {
        fn resolve(&self, _context: &ResolutionContext<'_>) -> Option<ResolvedReference> {
            self.0
                .map(|href| ResolvedReference::new(href.to_string(), None))
        }
    }

    /// The resolved `href` of the first cross-reference in the first footnote's
    /// subtree, scoped so the tree borrow is released before the next
    /// resolution.
    fn footnote_xref_href(doc: &crate::Document<'_>) -> Option<String> {
        refs_in(&first_footnote(doc).children)
            .into_iter()
            .find(|r| r.variant == RefVariant::Xref)
            .and_then(|r| r.resolved)
            .map(|resolved| resolved.href)
    }

    let mut parser = Parser::default();
    let mut doc = parser.parse_deferred("A claim.footnote:[see <<tgt>>]");

    doc.resolve_references(
        &FixedResolver(Some("#tgt")),
        &HtmlSubstitutionRenderer {},
        &parser,
    );
    assert_eq!(footnote_xref_href(&doc).as_deref(), Some("#tgt"));

    doc.resolve_references(&FixedResolver(None), &HtmlSubstitutionRenderer {}, &parser);
    assert_eq!(
        footnote_xref_href(&doc),
        None,
        "a re-resolution that fails must clear the footnote subtree's stale destination"
    );
}

#[test]
fn section_title_footnote_carries_its_subtree_and_resolved_xref() {
    // A footnote in a section heading is owned by the document-order title pass,
    // which resolves the heading's cross-references and now mirrors the
    // footnote-embedded ones into the heading tree's footnote subtree too.
    use crate::blocks::{Block, FindBlocks};

    let mut parser = Parser::default();
    let doc = parser.parse("[[tgt]]The target.\n\n== A Heading.footnote:[see <<tgt>>]\n\nBody.\n");

    fn headings<'a>(
        blocks: impl Iterator<Item = &'a Block<'a>>,
        out: &mut Vec<Vec<InlineNode<'a>>>,
    ) {
        for block in blocks {
            if let Block::Section(section) = block {
                out.push(section.section_title_inlines().to_vec());
            }

            headings(block.child_blocks(), out);
        }
    }

    let mut titles = vec![];
    headings(doc.child_blocks(), &mut titles);

    let footnote = titles
        .iter()
        .flatten()
        .find_map(|node| match node {
            InlineNode::Footnote(footnote) => Some(footnote),
            _ => None,
        })
        .expect("expected a footnote node in the section heading tree");

    assert!(
        !footnote.children.is_empty(),
        "the heading's footnote should carry its text as a subtree"
    );

    let reference = refs_in(&footnote.children)
        .into_iter()
        .find(|r| r.variant == RefVariant::Xref)
        .expect("expected an xref inside the heading footnote's subtree");

    assert_eq!(
        reference
            .resolved
            .as_ref()
            .expect("the heading footnote's xref should be resolved after parse")
            .href,
        "#tgt"
    );
}

// ─── Unit: the two footnote-subtree walks ───────────────────────────────────
//
// A footnote can be written inside a link's display text, which puts a
// `Footnote` node under a `Ref` node. Both new walks descend through reference
// children (and formatting spans) to reach such a subtree. The recording pass
// cannot currently produce that shape — a footnote nested in link text makes
// the Strategy A fold diverge from `rendered_html()`, a pre-existing limitation
// the Phase 4 single-pass builder retires — so the walks are driven directly
// here.

/// A cross-reference node, unresolved.
fn unresolved_xref() -> InlineNode<'static> {
    crate::inlines::InlineNode::Ref(crate::inlines::Ref {
        variant: RefVariant::Xref,
        link_form: None,
        target: "tgt".into(),
        children: vec![],
        roles: vec![],
        window: None,
        resolved: None,
        derived: None,
        xrefstyle: None,
        attrs: None,
        location: Span::new(""),
    })
}

/// A defining footnote node holding `children`.
fn defining_footnote(children: Vec<InlineNode<'static>>) -> InlineNode<'static> {
    InlineNode::Footnote(crate::inlines::Footnote {
        id: None,
        number: Some("1".into()),
        is_reference: false,
        children,
        location: Span::new(""),
    })
}

/// A link node holding `children` as its display text.
fn link_over(children: Vec<InlineNode<'static>>) -> InlineNode<'static> {
    InlineNode::Ref(crate::inlines::Ref {
        variant: RefVariant::Link,
        link_form: Some(crate::inlines::LinkForm::Macro),
        target: "https://example.org".into(),
        children,
        roles: vec![],
        window: None,
        resolved: None,
        derived: None,
        xrefstyle: None,
        attrs: None,
        location: Span::new(""),
    })
}

/// A strong span holding `children`.
fn strong_over(children: Vec<InlineNode<'static>>) -> InlineNode<'static> {
    InlineNode::Styled(crate::inlines::Styled {
        variant: StyleVariant::Strong,
        form: SpanForm::Constrained,
        id: None,
        roles: vec![],
        attrs: None,
        children,
        passthrough: None,
        location: Span::new(""),
    })
}

/// A `Content` carrying `inlines` as its inline tree.
fn content_with(inlines: Vec<InlineNode<'static>>) -> Content<'static> {
    let mut content = Content::from(Span::new("source"));
    content.set_inlines(inlines);
    content
}

#[test]
fn footnote_subtree_attaches_through_a_reference_child() {
    let mut tree = vec![link_over(vec![defining_footnote(vec![])])];

    crate::content::inline_tree::attach_footnote_subtrees(
        &mut tree,
        &["the note".to_string()],
        &[],
        Span::new(""),
    );

    let footnote = first_footnote_in(&tree);
    assert_eq!(footnote.children.len(), 1);
    assert!(
        matches!(&footnote.children[0], InlineNode::Text { value, .. } if value.as_ref() == "the note")
    );
}

#[test]
fn footnote_subtree_attaches_through_a_formatting_span() {
    let mut tree = vec![strong_over(vec![defining_footnote(vec![])])];

    crate::content::inline_tree::attach_footnote_subtrees(
        &mut tree,
        &["the note".to_string()],
        &[],
        Span::new(""),
    );

    assert_eq!(first_footnote_in(&tree).children.len(), 1);
}

#[test]
fn footnote_subtree_attachment_is_a_no_op_without_texts() {
    // The overwhelmingly common case: content that defined no footnote.
    let mut tree = vec![defining_footnote(vec![])];

    crate::content::inline_tree::attach_footnote_subtrees(&mut tree, &[], &[], Span::new(""));

    assert!(first_footnote_in(&tree).children.is_empty());
}

#[test]
fn footnote_xref_mirror_reaches_through_a_reference_child() {
    let mut content = content_with(vec![link_over(vec![defining_footnote(vec![
        unresolved_xref(),
    ])])]);

    content.mirror_tree_xref_resolution(&[], &fixed_destination());

    assert_eq!(resolved_href_in(content.inlines()).as_deref(), Some("#tgt"));
}

#[test]
fn footnote_xref_mirror_reaches_through_a_formatting_span() {
    let mut content = content_with(vec![strong_over(vec![defining_footnote(vec![
        unresolved_xref(),
    ])])]);

    content.mirror_tree_xref_resolution(&[], &fixed_destination());

    assert_eq!(resolved_href_in(content.inlines()).as_deref(), Some("#tgt"));
}

#[test]
fn footnote_xref_mirror_leaves_a_block_level_reference_alone() {
    // A block-level cross-reference belongs to the *other* list; the footnote
    // walk must not consume a footnote slot for it.
    let mut content = content_with(vec![unresolved_xref()]);

    content.mirror_tree_xref_resolution(&[None], &[]);

    assert_eq!(resolved_href_in(content.inlines()), None);
}

#[test]
fn a_description_list_term_carries_an_inline_tree() {
    // A description-list term used to run the substitution steps directly,
    // which left it the one content with no tree at all — and, with the
    // recognition side effects now replayed from the tree, the last content
    // still registering from the string pipeline. It runs through
    // `SubstitutionGroup::apply_to_description_list_term` now, so it has both.
    let mut parser = Parser::default();
    let doc = parser.parse("[[cpu]]*CPU*:: The brain of the computer.");

    let tree = description_term_tree(&doc);

    assert!(
        matches!(tree.first(), Some(InlineNode::Anchor(anchor)) if anchor.id.as_ref() == "cpu"),
        "expected the term's leading anchor as a node: {tree:?}"
    );

    assert!(
        matches!(tree.get(1), Some(InlineNode::Styled(_))),
        "expected the term's formatting span as a node: {tree:?}"
    );
}

#[test]
fn a_term_registers_its_leading_anchor_from_the_tree() {
    // The term's own rule — a leading `[[id]]` takes the **rest of the term**
    // as its default reference text — now runs from the tree, between the
    // build and the replay (see
    // `SubstitutionGroup::apply_to_description_list_term`). An explicit
    // `[[id,reftext]]` still wins over the default.
    let mut parser = Parser::default();
    let doc = parser.parse("[[cpu]]CPU:: The brain.\n\n[[gpu,Graphics]]GPU:: The other one.");

    assert_eq!(
        doc.catalog().get_ref("cpu").unwrap().reftext.as_deref(),
        Some("CPU")
    );

    assert_eq!(
        doc.catalog().get_ref("gpu").unwrap().reftext.as_deref(),
        Some("Graphics")
    );
}

#[test]
fn a_terms_default_reftext_is_the_rendering_of_the_rest() {
    // The one deliberate difference reading that rule off the tree makes. The
    // regex it replaces ran *before* the macros step, so a term whose
    // remainder held a macro registered the macro's **source** as the
    // reference text; the fold of the same nodes gives the rendering, which is
    // what every other reference text on this branch is.
    let mut parser = Parser::default();
    let doc = parser.parse("[[x]]image:a.png[]Term:: d");

    assert_eq!(
        doc.catalog().get_ref("x").unwrap().reftext.as_deref(),
        Some(r#"<span class="image"><img src="a.png" alt="a"></span>Term"#)
    );
}

#[test]
fn a_term_performs_each_recognition_side_effect_exactly_once() {
    // What the replay has to get right for a term specifically: the string
    // pipeline's own registrations are suppressed for the term's pass now, so
    // a construct that registers must be recorded once by the replay — not
    // twice, and not zero times.
    let mut parser = Parser::default().with_catalog_assets(true);
    let doc = parser.parse("image:a.png[]Term:: https://example.org[Link]");

    let catalog = doc.catalog();

    assert_eq!(
        catalog
            .images
            .iter()
            .map(|i| i.target.as_str())
            .collect::<Vec<_>>(),
        vec!["a.png"]
    );

    // The description (not the term) carries the link, which registers through
    // the ordinary content path — so the two halves of the item agree.
    assert_eq!(
        catalog.links.iter().map(|l| l.as_str()).collect::<Vec<_>>(),
        vec!["https://example.org"]
    );
}

#[test]
fn a_terms_default_reftext_folds_before_nested_references_resolve() {
    // A term's leading anchor is registered while the term is *parsed*, which
    // is what lets a later cross-reference resolve it at all — and that is
    // before any cross-reference *inside* the term has a destination. So a
    // `<<b>>` in the term contributes its **unresolved fallback** to the
    // catalogued reference text, and keeps it: the entry is never revisited.
    //
    // This is a limitation of registering at parse time, not of reading the
    // reftext from the tree, and the tree is the better of the two readings.
    // The regex this increment replaced ran before the macros step, so it
    // caught the reference as escaped *source* (`See &lt;&lt;b&gt;&gt;`) —
    // never a link at all. Both are unchanged by whether the target is
    // defined before or after the term.
    for source in [
        "[[a]]See <<b>>:: d\n\n[[b]]Target:: e\n\nRef: <<a>>.",
        "[[b]]Target:: e\n\n[[a]]See <<b>>:: d\n\nRef: <<a>>.",
    ] {
        let mut parser = Parser::default();
        let doc = parser.parse(source);

        assert_eq!(
            doc.catalog().get_ref("a").unwrap().reftext.as_deref(),
            Some(r##"See <a href="#b">[b]</a>"##),
            "the term's catalogued reftext regressed for {source:?}"
        );

        let rendered = collect_rendered(&doc);
        assert!(
            rendered
                .iter()
                .any(|s| s.contains(r##"<a href="#a">See [b]</a>"##)),
            "the reference to the term regressed: {rendered:?}"
        );
    }
}

/// The inline tree of the first description-list term in `doc`.
fn description_term_tree<'a>(doc: &'a crate::Document<'a>) -> Vec<InlineNode<'a>> {
    use crate::blocks::{Block, FindBlocks, ListItemMarker};

    for block in doc.descendant_blocks() {
        if let Block::ListItem(item) = block
            && let ListItemMarker::DefinedTerm { term, .. } = item.list_item_marker()
        {
            return term.inlines().to_vec();
        }
    }

    panic!("no description-list term in the document");
}

/// One resolved destination, for the footnote-embedded list.
fn fixed_destination() -> Vec<Option<crate::parser::ResolvedReference>> {
    vec![Some(crate::parser::ResolvedReference::new(
        "#tgt".to_string(),
        None,
    ))]
}

/// The footnote node the single wrapper node of `nodes` holds, descending
/// through a reference child or a formatting span to reach it.
fn first_footnote_in<'a>(nodes: &[InlineNode<'a>]) -> crate::inlines::Footnote<'a> {
    match nodes.first().expect("expected one node") {
        InlineNode::Footnote(footnote) => footnote.clone(),
        InlineNode::Ref(reference) => first_footnote_in(&reference.children),
        InlineNode::Styled(styled) => first_footnote_in(&styled.children),
        other => panic!("expected a footnote-bearing node, got {other:?}"),
    }
}

/// The resolved `href` of the first cross-reference in `nodes`, using the same
/// recursion the other resolution tests use.
fn resolved_href_in(nodes: &[InlineNode<'_>]) -> Option<String> {
    refs_in(nodes)
        .into_iter()
        .find(|r| r.variant == RefVariant::Xref)
        .and_then(|r| r.resolved)
        .map(|resolved| resolved.href)
}

#[test]
fn a_passthrough_body_renders_under_every_order_its_own_list_can_name() {
    // The capability the authoritative-pass closure rests on, pinned directly.
    //
    // `passthrough_text` used to hand the body to the string pipeline, which
    // runs a resolved step list in the author's own order. It builds the body's
    // own tree now, so the claim that has to hold is that
    // `build_for_group` honors that order too — including the orders no
    // built-in group has, where the escaping step comes *after* a step that
    // already produced markup.
    //
    // That is not hypothetical: sweeping the suite, a passthrough body reaches
    // its rendering under fifteen distinct `Custom` spellings, and
    // `Custom([Quotes, SpecialCharacters])` and
    // `Custom([AttributeReferences, Quotes])` are among them. The fixtures
    // below name both orders of each pair explicitly, so a builder that
    // silently normalized to the *normal* order (specialcharacters first)
    // would fail here rather than in whichever corpus happened to carry one.
    let parser = Parser::default().with_intrinsic_attribute(
        "tag",
        "<b>bold</b>",
        crate::parser::ModificationContext::Anywhere,
    );

    for (source, expected) in [
        // `c` alone, and `q` alone — the single-step controls.
        ("pass:c[a < b]", "a &lt; b"),
        ("pass:q[*x* &]", "<strong>x</strong> &"),
        // Both orders of the escaping/quotes pair. `c,q` escapes first, so the
        // `<` the author wrote is an entity by the time quotes runs; `q,c`
        // runs quotes first and then escapes what it produced, which is why
        // the `<strong>` tags come out as entities.
        ("pass:c,q[*x* < y]", "<strong>x</strong> &lt; y"),
        ("pass:q,c[*x* < y]", "&lt;strong&gt;x&lt;/strong&gt; &lt; y"),
        // Both orders of the attributes/quotes pair. With `a` first the
        // expanded value is present when quotes runs; the expansion carries
        // markup either way, and neither order escapes it since `c` is absent.
        (
            "pass:a,q[{tag} and *x*]",
            "<b>bold</b> and <strong>x</strong>",
        ),
        (
            "pass:q,a[*x* and {tag}]",
            "<strong>x</strong> and <b>bold</b>",
        ),
        // Attributes with escaping after it: the spliced value is escaped like
        // any other text, which is the case `SplicedSpecials` exists for.
        ("pass:a,c[{tag}]", "&lt;b&gt;bold&lt;/b&gt;"),
        // And escaping before it, where the spliced value is left alone.
        ("pass:c,a[{tag} < x]", "<b>bold</b> &lt; x"),
        // The empty list, which renders the body untouched.
        ("pass:[]", ""),
    ] {
        let mut content = crate::content::Content::from(crate::Span::new(source));
        crate::content::SubstitutionGroup::Normal.apply(&mut content, &parser, None);

        assert_eq!(content.rendered_str(), expected, "for {source:?}");
    }
}
