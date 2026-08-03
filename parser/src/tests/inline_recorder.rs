//! Differential oracle for the inline AST, over the production
//! [`inline_tree`](crate::content::inline_tree) module.
//!
//! This began as the Phase 1 bring-up oracle (a test-only recorder). In Phase 2
//! the recorder machinery moved into the parser proper
//! ([`inline_tree`](crate::content::inline_tree)); this module is now the
//! **harness** that drives it, keeping the same corpus and the two invariants
//! it proves:
//!
//! 1. Stripping/folding the recorder's marked string reproduces the built-in
//!    renderer's `rendered()` output **byte-for-byte** (the *no-perturbation*
//!    invariant).
//! 2. The recovered [`InlineNode`] tree is structurally faithful (node kinds,
//!    nesting, and the `constructs <= markers <= events` cross-check).
//!
//! Because the machinery is now the production code path, this corpus doubles
//! as the differential test for what
//! [`Content::inlines`](crate::content::Content) stores when inline-tree
//! building is enabled on the [`Parser`].
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

/// The number of recorded constructs (non-text nodes) in the tree – i.e. the
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
    /// The authoritative `rendered()` output of the unmodified pipeline.
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
    group.apply(&mut golden_content, &golden_parser, None);
    let golden = golden_content.rendered_owned();

    // Recording pass, driving the production recorder.
    let (recorder, events) = RecordingRenderer::new(Rc::new(HtmlSubstitutionRenderer {}));
    let recording_parser =
        experimental(Parser::default()).with_inline_substitution_renderer(recorder);
    let mut recording_content = Content::from(Span::new(source));
    group.apply(&mut recording_content, &recording_parser, None);
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
        "fold of the inline tree diverged from rendered() for {source:?}"
    );

    // Cross-check the tree against the recorder along the honest chain
    // `constructs <= markers <= events`:
    //
    // * `markers <= events` – some recorded events (formatting inside a footnote's
    //   text) are extracted out of the block, so their marker never reaches the
    //   block string.
    // * `constructs <= markers` – some markers (formatting inside a construct
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
    // Line breaks (post-replacement).
    "first line +\nsecond line",
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
    // Hard breaks across multiple lines.
    "line one +\nline two +\nline three",
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
// catalog). This harness parses whole documents – once normally, once with the
// recorder installed – and asserts that folding each simple block's recorded
// tree reproduces its `rendered()` string. It reaches resolved xrefs, list
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
                out.push(content.rendered().to_string());
            }
        }
    }

    fn walk(block: &Block<'_>, out: &mut Vec<String>) {
        if let Some(title) = block.title() {
            out.push(title.to_string());
        }

        match block {
            Block::Simple(simple) => out.push(simple.content().rendered().to_string()),

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

/// Parses `source` twice – normally and with the recorder – and asserts that
/// folding every content location's recorded tree reproduces its `rendered()`
/// output byte-for-byte, including any resolved cross-references, section
/// headings, block titles, and table cells.
fn check_document(source: &str) {
    assert_no_reserved_sentinels(source);

    let mut golden_parser = experimental(Parser::default());
    let golden_doc = golden_parser.parse(source);
    let golden = collect_rendered(&golden_doc);

    let (recorder, events) = RecordingRenderer::new(Rc::new(HtmlSubstitutionRenderer {}));
    let mut recording_parser =
        experimental(Parser::default()).with_inline_substitution_renderer(recorder);
    let recording_doc = recording_parser.parse(source);
    let events = events.borrow();

    let folded: Vec<String> = collect_rendered(&recording_doc)
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
        "document fold diverged from rendered() for {source:?}"
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
// These exercise the Phase 2 wiring: `Parser::with_inline_tree` builds each
// `Content`'s tree during the parse, so `Content::inlines()` is populated for
// real blocks (with document counters numbered correctly), and the default
// (flag-off) path leaves it empty.

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

#[test]
fn inline_tree_is_empty_by_default() {
    let mut parser = Parser::default();
    let doc = parser.parse("One *word* is strong.");

    assert!(
        first_simple_inlines(&doc).is_empty(),
        "the inline tree must be empty when tree building is disabled"
    );
}

#[test]
fn inline_tree_is_built_when_enabled() {
    let mut parser = Parser::default().with_inline_tree(true);
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

#[test]
fn inline_tree_numbers_footnotes_in_document_order() {
    // The recording pass clones the parser *before* the authoritative pass
    // advances the footnote counter, so a footnote in the second paragraph is
    // numbered "2" in its tree – matching `rendered()` – rather than restarting
    // at "1" (which a fresh-parser recording pass would produce).
    let mut parser = Parser::default().with_inline_tree(true);
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
