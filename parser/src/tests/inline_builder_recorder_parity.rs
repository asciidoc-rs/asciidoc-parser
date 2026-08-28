//! Structural cross-check: the Phase 4 single-pass builder's tree
//! ([`inline_builder::build`]) against the Strategy-A recorder's tree (built
//! directly by the now test-only `inline_tree` machinery, reproducing the
//! retired production recording path).
//!
//! Design §4.1 calls for exactly this during bring-up ("the node stream is
//! cross-checked against Strategy A's recorder to catch structural
//! regressions the HTML oracle cannot see") and §5.5's own risk table names
//! the residual gap directly: "two node trees fold to identical HTML,
//! masking a structural bug." Every differential corpus elsewhere in this
//! crate (the recorder's own [`NORMAL_CORPUS`](super::inline_recorder), and
//! [`inline_builder`](crate::content::inline_builder)'s own per-step and
//! whole-pipeline corpora) pins **byte-for-byte HTML parity** between a
//! tree's fold and the golden rendered string — but that is exactly the kind
//! of check two structurally different trees can both pass. This module
//! compares the trees **themselves**, node kind by node kind, instead.
//!
//! With Phase 4 step 5 landed, the builder now covers essentially the whole
//! vocabulary the recorder does, so a real structural mismatch here — not
//! merely a difference in one of the fields below — would be a genuine
//! regression, not an expected divergence. This was the last piece of
//! due-diligence design §5.2's own "next steps" list called for before the
//! Phase 4 "cutover" (step 6) could safely replace the recorder with the
//! builder as `Content`'s tree source — a swap that has since landed, which
//! is why the recorder side here is built by driving the (now test-only)
//! recorder machinery directly rather than by reading `Content::inlines()`:
//! the production accessor now returns the builder's own tree, and comparing
//! that to itself would prove nothing. The cross-check stays on as the
//! regression guard that the two independent constructions keep agreeing.
//!
//! A handful of fields are **intentionally** excluded from the comparison,
//! each already documented elsewhere in this crate as a known difference
//! between the two construction strategies, not a defect:
//!
//! - `location` — the recorder gives every node the whole-content span (the
//!   design §4.4 "migration stage" fallback every recorder-built node still
//!   uses); the builder gives each node its own precise span (§4.4's "precision
//!   stage", issue #944). Comparing spans here would fail by design on every
//!   single fixture.
//! - `attrs` (on [`Styled`](crate::inlines::Styled)/[`Ref`]/[`Image`]) — the
//!   recorder's `'static` second pass cannot carry a borrowed `Attrlist<'src>`
//!   into a node (see the design doc's Phase 3 step 2 note), so it is always
//!   `None`; the builder captures the real, parsed attribute list. The builder
//!   is strictly more informative, never less.
//! - `derived` / `xrefstyle` (on [`Ref`]) — likewise always `None` from the
//!   recorder (Phase 4 part 3c's own landed-as note); the builder populates
//!   them.
//! - `resolved` (on [`Ref`]) — both sides are built here with no resolution
//!   pass, so it is `None` on both regardless; excluded for robustness rather
//!   than because it is expected to differ.
//! - `reftext` (on [`Anchor`](crate::inlines::Anchor)) — an anchor's HTML is a
//!   function of its `id` alone, so `reftext` renders nothing the recorder
//!   could ever recover it from and is always `None` there, while the builder
//!   still carries it as a structural fact.
//! - `is_icon` (on [`Image`]) — the recorder's own recorded marker does not
//!   distinguish `icon:` from `image:` (both fold through the same kind), so it
//!   always reports `false`; the builder sets it honestly.
//! - `link_form` (on [`Ref`]) — all three link spellings render to the same `<a
//!   …>` markup, which is all the recorder has to recover from, so it is always
//!   `None` there; the builder records which pass built the node.
//!
//! The recorder side is **frozen** (`snapshots/recorder_trees.txt`). It is the
//! half of this differential that dies with
//! `SubstitutionGroup::apply_string_pipeline`: [`RecordingRenderer`] recovers a
//! tree out of what that pipeline *renders*, so once the pipeline goes there is
//! no recorder tree left and this module would be comparing [`build`] to
//! itself. Design §5.2's survey called this the **tree-shaped** freeze and the
//! harder half by a wide margin, because — unlike the two record-shaped corpora
//! — what the two sides hold is not equal and never was, so a freeze needs a
//! *per-side normal form* where a pairwise diff is all this module has. See
//! [`frozen_recorder_tree`] for the normal form that answers it (the restricted
//! one the recorder already satisfies, enumerated field by field just below),
//! and [`strip_unrecorded`] for the guard that keeps it total.
//!
//! Several more differences are structural rather than field-level, so they
//! are handled by the comparator itself instead of being excluded outright.
//! The common thread through all of them: the recorder can only recover a
//! leaf's structure from already-*rendered* output, while the builder's
//! leaves are drawn from *source*, so the two can legitimately draw leaf
//! boundaries differently even when the underlying content is identical.
//! [`consume_rendered_prefix`] is the one mechanism behind every case below —
//! see its own doc comment for how it works:
//!
//! - A handful of character-replacement types combine more than one output
//!   character into a single logical value (an em dash surrounded by spaces —
//!   [`CharacterReplacementType::EmDashSurroundedBySpaces`](crate::parser::CharacterReplacementType)
//!   — renders as *thin space, em dash, thin space*, three characters from one
//!   source match). The builder recognizes the whole match as **one**
//!   [`CharRef::Replacement`](crate::inlines::CharRef::Replacement) leaf
//!   carrying the combined value; the recorder sees three separate numeric-
//!   entity references in the output string and recovers **three** adjacent
//!   leaves, one per entity. Both fold to the same HTML.
//! - Adjacent `Text` node boundaries can legitimately differ: the builder gives
//!   an attribute-expanded run its own node even when it is plain text with no
//!   escaping of its own (design §3.4.1), so it sits as a distinct sibling
//!   beside the literal text around it, while the recorder — with no marker
//!   around plain text — recovers the whole run as one `Text` node.
//! - A source `&amp;`/`&lt;`/`&gt;`/`&#8217;`/… that the author wrote out
//!   literally passes through Asciidoctor's `specialcharacters` step unchanged
//!   (it is already a valid entity), rendering byte-identical to whatever
//!   *live* substitution — a special-character escape, or a typographic
//!   replacement that happens to render as the same entity — produces the same
//!   output. From already-rendered output alone the recorder cannot tell "the
//!   author wrote this entity" ([`CharRef::Entity`], the builder's
//!   classification) apart from the live classification that coincides with it
//!   — exactly the same set of entities [`classify_entity`] (the recorder's own
//!   recovery table) hard-codes, reproduced in [`RECORDER_ENTITY_TABLE`] and
//!   kept in lockstep with it by
//!   [`recorder_entity_table_matches_production_classify_entity`].
//! - A passthrough's content becomes its own [`Raw`](InlineNode::Raw) leaf in
//!   the builder's tree — but a passthrough's *restore* is a direct string
//!   splice with no renderer call for the recorder to intercept (design §4.2's
//!   "re-splicing is just keeping the node in place" describes the target
//!   architecture, not what today's post-hoc string recorder can observe), so
//!   the recorder recovers its content as indistinguishable plain
//!   `Text`/`CharRef` leaves (its own `<`/`>`/`&` still show up as entities in
//!   the output even though the builder's `Raw` value has already encoded them,
//!   since a passthrough's content still passes through `specialcharacters`
//!   when its own effective substitution order includes it, design §3.4.1).
//!
//! - An **unresolved** cross-reference's `children` (the author's own display
//!   text, `<<id,text>>`/`xref:id[text]`) is a structural fact the builder
//!   bakes in at build time regardless of resolution (design §4.1's own note on
//!   the node: "an empty text yields no children ... the fold reads as 'no text
//!   provided'"). The recorder can only recover what actually got *rendered*,
//!   and Asciidoctor's own unresolved-xref fallback renders the bracketed
//!   `[target]` form — never the author's display text — so the recorder's
//!   `children` for such a node is legitimately empty even when the builder's
//!   is not. This corpus builds both trees with no resolution pass, so every
//!   cross-reference is in exactly this state; a resolved reference (out of
//!   scope here) would not have this gap, since resolution mirrors the
//!   *destination*, not the text, onto a `children` list the builder already
//!   populated correctly.
//! - A footnote **reference** occurrence's own `id` (`footnote:disc[]`) never
//!   reaches the renderer's params at all once it *resolves* to a number — the
//!   fold renders just the number, dropping `id` — so the recorder cannot
//!   recover it, while the builder still carries it as a structural fact (see
//!   [`assert_node_equivalent`]'s own `Footnote` arm).
//! - A construct written **beside** a span, where the sub that recognizes it
//!   reads the character the span's own rendering ends with (``` "`a`"`code`
//!   ```, whose `` `code` `` both the real pipeline and the builder leave
//!   literal because `&#8221;` ends in a `;` the monospace sub's boundary class
//!   excludes; `**bold**https://example.org`, which both link because
//!   `</strong>` ends in a `>` the auto-link's prefix group accepts). This one is the recorder's own artifact rather than a builder
//!   deferral, and it is the seam Phase 1 already named when it left special
//!   characters and replacements unmarked ("their escaped output is re-consumed
//!   by later steps, so bracketing it would perturb recognition"): a
//!   [`RecordingRenderer`] emits its marker *outside* the markup it wraps, so a
//!   later sub matching over the recorded string reads that marker — which
//!   belongs to no boundary class — where the real pipeline reads the markup's
//!   own last character. The recorder therefore builds a span the real pipeline
//!   never rendered. A **transparent** span — one rendering to its body and
//!   nothing else, whose own body is therefore what a sibling reads
//!   (`[width=10]##x ##https://example.org`) — is the same artifact seen from
//!   the other side: the marker stands between that body and the construct
//!   beside it, so the recorder reads the marker where the real pipeline reads
//!   the space. Recognition **inside** a span is unaffected (the marker
//!   sits outside the tag or entity pair, so the interior reads the same `>` or
//!   `;` either way), which is why the sweep's own boundary fixtures one level
//!   *in* are cross-checked here normally; the sibling shapes are pinned
//!   instead by the parity corpora in
//!   [`inline_builder`](crate::content::inline_builder), against the real
//!   pipeline's own bytes.
//! - A [`Link`](RefVariant::Link)'s `roles`/`window` fields are not populated
//!   from its own attribute-list display text (`Ref::attrs`'s own doc comment)
//!   — the fold reads those straight off `attrs` instead — so they are skipped
//!   whenever `attrs` is present (see [`assert_node_equivalent`]'s own `Ref`
//!   arm).
//!
//! The corpus reuses (and, for the general-purpose sweep, duplicates
//! verbatim) the fixture sets already proven — by
//! [`inline_builder`](crate::content::inline_builder)'s own tests — to stay
//! inside the vocabulary the builder claims: a fixture exercising a form
//! either side still leaves deferred (documented throughout the design
//! doc's Phase 4 notes, e.g. a link/xref display text crossing a rendered
//! span) is intentionally excluded, since for those the two trees are
//! expected to differ.
//!
//! [`inline_builder::build`]: crate::content::inline_builder
//! [`Ref`]: crate::inlines::Ref
//! [`Image`]: crate::inlines::Image

#![allow(clippy::unwrap_used)]

use std::{borrow::Cow, collections::VecDeque, rc::Rc};

use crate::{
    Parser, Span,
    content::{
        Content, SubstitutionGroup,
        inline_builder::{
            build,
            snapshot::{quote, recorded_golden, unquote},
        },
        inline_tree::{
            CharRefKind, RecordingRenderer, attach_footnote_subtrees, build_inline_tree,
            classify_entity,
        },
    },
    inlines::{
        Anchor, Callout, CalloutGuard, CharRef, Footnote, Image, IndexTerm, InlineNode, RawForm,
        Ref, RefVariant, SpanForm, Stem, StemNotation, StyleVariant, Styled, Ui, UiKind,
    },
    parser::{HtmlSubstitutionRenderer, ModificationContext},
    strings::CowStr,
};

/// Builds both trees for `source` under a document configured by `configure`
/// (called once per side, so each side advances its own, independently
/// counted document — the same two-independent-parsers discipline
/// [`inline_builder`](crate::content::inline_builder)'s own differential
/// corpora use, for the same reason: both sides advance real document
/// counters, such as footnote numbers, so sharing a parser would double
/// them), then asserts they are the same shape.
///
/// The recorder side reproduces the *retired* Strategy-A production path
/// directly — a [`RecordingRenderer`] wrapped around the built-in HTML
/// renderer, the real pipeline run over it, the recorded markers parsed into
/// a tree, and each defining footnote's subtree attached from the footnote
/// texts that pass registered — exactly what `SubstitutionGroup::apply` did
/// before the single-pass builder replaced the recorder as `Content`'s tree
/// source (the swap this module's cross-check cleared the way for). The
/// builder side is the production path's own [`build`].
fn assert_shapes_with(source: &str, configure: impl Fn() -> Parser) {
    let (recorder, events) = RecordingRenderer::new(Rc::new(HtmlSubstitutionRenderer {}));
    let recorder_parser = configure().with_inline_substitution_renderer(recorder);

    // A footnote's text never reaches the marked block string (it is extracted
    // out of the flow), so it is recovered from the footnotes the recording
    // pass registers; snapshot the registry length first so only this
    // content's own footnotes are picked up.
    let footnote_start = recorder_parser.catalog().footnotes.len();

    let mut content = Content::from(Span::new(source));
    SubstitutionGroup::Normal.apply_string_pipeline(&mut content, &recorder_parser, None);

    let footnote_texts: Vec<String> = recorder_parser
        .catalog()
        .footnotes
        .get(footnote_start..)
        .unwrap_or_default()
        .iter()
        .map(|footnote| footnote.text.clone())
        .collect();

    let marked = content.rendered_owned();
    let events = events.borrow();
    let mut recorder_tree = build_inline_tree(&marked, &events, Span::new(source));
    attach_footnote_subtrees(
        &mut recorder_tree,
        &footnote_texts,
        &events,
        Span::new(source),
    );

    let builder_parser = configure();
    let builder_tree = build(Span::new(source), &builder_parser, None);

    // The recorder side, frozen: still built live above (so the drift guard
    // has something to check), but what the comparison actually reads is the
    // recording. See `frozen_recorder_tree`.
    let decoded = frozen_recorder_tree(source, &recorder_tree);

    // The freeze's own guard, run on every fixture rather than in a test of its
    // own: the decoded tree must equal the live one field for field, once the
    // fields the recording deliberately drops are cleared. See
    // `strip_unrecorded` for why this is the whole of the risk surface.
    assert_eq!(
        strip_unrecorded(&recorder_tree, Span::new(source)),
        decoded,
        "the recording lost something the live recorder tree carries for {source:?}"
    );

    let recorder_tree = decoded;

    assert_trees_equivalent(&recorder_tree, &builder_tree, source);
}

fn assert_shapes(source: &str) {
    assert_shapes_with(source, Parser::default);
}

/// Asserts that two inline-node slices are the same shape, in order,
/// ignoring only the fields this module's own doc comment documents as
/// intentionally different between the two construction strategies, and
/// resolving the leaf-boundary differences the doc comment also documents
/// via [`consume_rendered_prefix`].
///
/// Nodes are consumed from the front of each side with [`VecDeque`] (rather
/// than walked by a fixed index) because resolving those differences can
/// require **splitting** a recorder `Text` node: a builder leaf's rendered
/// value is often only a *prefix* of the recorder's current text run, so the
/// unconsumed remainder is pushed back for the next iteration instead of
/// being dropped.
fn assert_trees_equivalent(recorder: &[InlineNode<'_>], builder: &[InlineNode<'_>], source: &str) {
    let mut recorder: VecDeque<InlineNode<'_>> = recorder.to_vec().into();
    let mut builder: VecDeque<InlineNode<'_>> = builder.to_vec().into();

    loop {
        if recorder.is_empty() && builder.is_empty() {
            break;
        }

        assert_eq!(
            recorder.is_empty(),
            builder.is_empty(),
            "node count differs for {source:?}\nrecorder leftover: {recorder:#?}\nbuilder leftover: {builder:#?}"
        );

        let b = builder.front().expect("checked non-empty above");

        // A `Text`/`Raw`/`CharRef` builder leaf is first tried against the
        // recorder's *rendered* bytes at this position — see
        // `consume_rendered_prefix`'s own doc comment for why this one
        // mechanism covers every leaf-boundary difference the module doc
        // comment documents. Only when that fails (or `b` is not a leaf at
        // all) do the two front nodes get popped and compared node-for-node.
        //
        // `allowed` restricts which *recorder* leaf kinds may participate,
        // so a byte-coincidental match can never paper over a genuine kind
        // regression: a `Text` target only ever matches recorder `Text`
        // (an ordinary `Text` node's value never contains an escaped
        // special character, so it has no business matching a `CharRef`'s
        // rendered entity), and a `CharRef` target only ever matches
        // recorder `CharRef`. `Raw` (a passthrough) is the one documented
        // exception that legitimately spans both kinds — its content still
        // passes through `specialcharacters`, so the recorder can recover
        // it as a mix of plain text and entities (design §3.4.1).
        let target: Option<(Cow<'_, str>, LeafKinds)> = match b {
            InlineNode::Text { value, .. } => {
                Some((Cow::Borrowed(value.as_ref()), LeafKinds::TextOnly))
            }
            InlineNode::Raw { value, form, .. } => {
                Some((raw_rendered(value, *form), LeafKinds::Mixed))
            }
            InlineNode::CharRef { value, .. } => {
                Some((char_ref_rendered(value), LeafKinds::CharRefOnly))
            }
            _ => None,
        };

        if let Some((target, allowed)) = target
            && consume_rendered_prefix(&mut recorder, target.as_ref(), allowed)
        {
            builder.pop_front();
            continue;
        }

        let r = recorder.pop_front().expect("checked non-empty above");
        let b = builder.pop_front().expect("checked non-empty above");

        assert_node_equivalent(&r, &b, source);
    }
}

/// The recorder's own entity-recovery table
/// ([`classify_entity`] in `content/inline_tree.rs`), reproduced here as
/// `(entity, recorded CharRef)` pairs so this module can go in both
/// directions: rendering a [`CharRef`] back to the output bytes it folds to
/// ([`char_ref_rendered`]), and, by the same table, recognizing when a
/// recorder leaf run and a builder leaf coincide on those bytes even though
/// their [`CharRef`] *classifications* differ (the `&amp;`/`&#8217;`/…
/// ambiguity the module doc comment describes).
///
/// This is a hand-reproduced copy of `classify_entity`'s own match arms, not
/// a shared definition (the two functions map in opposite directions — entity
/// to kind there, kind to entity here — over different types, the recorder's
/// own internal `CharRefKind` there and the public `CharRef` here), so it can
/// drift from its source silently.
/// [`recorder_entity_table_matches_production_classify_entity`] below guards
/// against that: it feeds every entity in this table through
/// the real `classify_entity` and asserts the result still matches, so a
/// future change to the production table fails this test immediately rather
/// than silently weakening (or spuriously failing) the parity corpus above.
const RECORDER_ENTITY_TABLE: &[(&str, CharRef<'static>)] = &[
    ("&lt;", CharRef::Special('<')),
    ("&gt;", CharRef::Special('>')),
    ("&amp;", CharRef::Special('&')),
    ("&#169;", CharRef::Replacement("\u{a9}")),
    ("&#174;", CharRef::Replacement("\u{ae}")),
    ("&#8482;", CharRef::Replacement("\u{2122}")),
    ("&#8201;", CharRef::Replacement("\u{2009}")),
    ("&#8212;", CharRef::Replacement("\u{2014}")),
    ("&#8203;", CharRef::Replacement("\u{200b}")),
    ("&#8230;", CharRef::Replacement("\u{2026}")),
    ("&#8592;", CharRef::Replacement("\u{2190}")),
    ("&#8656;", CharRef::Replacement("\u{21d0}")),
    ("&#8594;", CharRef::Replacement("\u{2192}")),
    ("&#8658;", CharRef::Replacement("\u{21d2}")),
    ("&#8217;", CharRef::Replacement("\u{2019}")),
    ("&#8216;", CharRef::Replacement("\u{2018}")),
];

/// Guards [`RECORDER_ENTITY_TABLE`] against drifting from its source of
/// truth (see that constant's own doc comment): feeds every entity in the
/// table through the real, production [`classify_entity`] and asserts the
/// recovered kind still matches what the table claims.
#[test]
fn recorder_entity_table_matches_production_classify_entity() {
    for (entity, expected) in RECORDER_ENTITY_TABLE {
        let kind = classify_entity(entity);

        let matches = match (&kind, expected) {
            (CharRefKind::Special(a), CharRef::Special(b)) => a == b,
            (CharRefKind::Replacement(a), CharRef::Replacement(b)) => a == b,
            (CharRefKind::Entity(a), CharRef::Entity(b)) => a.as_str() == b.as_ref(),
            _ => false,
        };

        assert!(
            matches,
            "RECORDER_ENTITY_TABLE's entry for {entity:?} ({expected:?}) no longer matches \
             production's classify_entity ({kind:?}) — update the table to match"
        );
    }
}

/// Renders a [`CharRef`] to the output bytes it folds to: the exact inverse
/// of [`classify_entity`], via the same [`RECORDER_ENTITY_TABLE`]. Falls
/// back to the value/name itself for a
/// [`CharRef::Entity`] the table does not name (an author-written entity
/// `classify_entity` has no special case for is carried through unchanged,
/// on both sides, the same way). A multi-character
/// [`CharRef::Replacement`] (an em dash surrounded by spaces, an ellipsis —
/// see the module doc comment) has no single table entry of its own, since
/// the recorder recovers it as several adjacent leaves, one per character;
/// it is rendered by looking up each character individually and
/// concatenating, mirroring that recovery exactly.
/// The bytes a [`Raw`](InlineNode::Raw) leaf contributes to the rendered
/// output, which is what the recorder recovered its own nodes from.
///
/// An [`AsIs`](RawForm::AsIs) value already *is* those bytes. An
/// [`Escaped`](RawForm::Escaped) one is the author's logical text that the fold
/// escapes — a `++<b>++` body is `<b>` on the node and `&lt;b&gt;` in the
/// output — so comparing the field directly would ask the recorder for bytes no
/// pipeline ever produced. This mirrors [`char_ref_rendered`], which has always
/// had to render its leaf for the same reason.
fn raw_rendered<'a>(value: &'a CowStr<'_>, form: RawForm) -> Cow<'a, str> {
    match form {
        RawForm::AsIs => Cow::Borrowed(value.as_ref()),

        RawForm::Escaped => Cow::Owned(
            value
                .chars()
                .map(|c| match c {
                    '<' => "&lt;".to_owned(),
                    '>' => "&gt;".to_owned(),
                    '&' => "&amp;".to_owned(),
                    other => other.to_string(),
                })
                .collect(),
        ),
    }
}

fn char_ref_rendered<'a>(value: &'a CharRef<'_>) -> Cow<'a, str> {
    if let Some((entity, _)) = RECORDER_ENTITY_TABLE.iter().find(|(_, cr)| cr == value) {
        return Cow::Borrowed(entity);
    }

    match value {
        CharRef::Entity(name) => Cow::Borrowed(name.as_ref()),
        CharRef::Special(c) => Cow::Owned(c.to_string()),

        CharRef::Replacement(v) => Cow::Owned(
            v.chars()
                .map(|c| match entity_for_char(c) {
                    Some(entity) => entity.to_owned(),
                    None => c.to_string(),
                })
                .collect(),
        ),
    }
}

/// Looks up `c`'s own entity in [`RECORDER_ENTITY_TABLE`], for decomposing a
/// multi-character [`CharRef::Replacement`] value one character at a time
/// (see [`char_ref_rendered`]'s own doc comment).
fn entity_for_char(c: char) -> Option<&'static str> {
    RECORDER_ENTITY_TABLE
        .iter()
        .find(|(_, cr)| matches!(cr, CharRef::Replacement(v) if v.chars().eq([c])))
        .map(|(entity, _)| *entity)
}

/// Which recorder leaf kinds [`consume_rendered_prefix`] may draw on for a
/// given builder target — see that function's own doc comment, and
/// [`assert_trees_equivalent`]'s own note on why this restriction exists
/// (so a byte-coincidental match can never paper over a genuine leaf-kind
/// regression).
#[derive(Clone, Copy, PartialEq)]
enum LeafKinds {
    /// Only a recorder [`Text`](InlineNode::Text) leaf may participate.
    TextOnly,
    /// Only a recorder [`CharRef`](InlineNode::CharRef) leaf may
    /// participate.
    CharRefOnly,
    /// Either may participate (a builder `Raw` target only).
    Mixed,
}

impl LeafKinds {
    fn allows_text(self) -> bool {
        matches!(self, Self::TextOnly | Self::Mixed)
    }

    fn allows_char_ref(self) -> bool {
        matches!(self, Self::CharRefOnly | Self::Mixed)
    }
}

/// The single mechanism behind every leaf-boundary difference this module's
/// own doc comment documents: if a prefix of `recorder`'s front nodes —
/// each [`Text`](InlineNode::Text) leaf contributing its value verbatim,
/// each [`CharRef`](InlineNode::CharRef) leaf contributing its own
/// *rendered* bytes ([`char_ref_rendered`]) — concatenates to exactly
/// `target`, consumes that prefix and returns `true`. A `CharRef` leaf is
/// atomic (only ever consumed whole); a trailing `Text` leaf is split when
/// `target` ends part-way through it, so the unconsumed remainder is kept in
/// place for the next call. Stops (and returns `false`, leaving `recorder`
/// untouched) at the first node that is neither, whose kind `allowed`
/// excludes, or whose bytes do not fit `target`.
///
/// This one function is what lets [`assert_trees_equivalent`] treat as
/// equivalent: a builder `Text` node against a wider (or narrower) recorder
/// text run; a builder `Raw` leaf (a passthrough) against the plain
/// `Text`/`CharRef` leaves the recorder recovers its content as; a builder
/// `CharRef::Replacement` combining several output characters against the
/// recorder's one-leaf-per-entity recovery of the same; and a builder
/// `CharRef::Entity` against the recorder's differently-classified but
/// byte-identical recovery.
fn consume_rendered_prefix(
    recorder: &mut VecDeque<InlineNode<'_>>,
    target: &str,
    allowed: LeafKinds,
) -> bool {
    enum Plan {
        WholeNodes(usize),
        SplitLast {
            whole_before: usize,
            remainder: String,
        },
    }

    let mut plan = None;
    let mut rendered = String::new();
    let mut whole_count = 0;

    for node in recorder.iter() {
        let piece: Cow<'_, str> = match node {
            InlineNode::Text { value, .. } if allowed.allows_text() => {
                Cow::Borrowed(value.as_ref())
            }
            InlineNode::CharRef { value, .. } if allowed.allows_char_ref() => {
                char_ref_rendered(value)
            }
            _ => break,
        };

        let Some(remaining_target) = target.get(rendered.len()..) else {
            break;
        };

        if let Some(after) = remaining_target.strip_prefix(piece.as_ref()) {
            rendered.push_str(&piece);
            whole_count += 1;

            if after.is_empty() {
                plan = Some(Plan::WholeNodes(whole_count));
                break;
            }

            continue;
        }

        // The whole piece doesn't fit; a splittable `Text` leaf whose own
        // *prefix* exactly completes `target` still counts as a match.
        if allowed.allows_text()
            && let InlineNode::Text { value, .. } = node
            && let Some(remainder) = value.as_ref().strip_prefix(remaining_target)
        {
            plan = Some(Plan::SplitLast {
                whole_before: whole_count,
                remainder: remainder.to_owned(),
            });
        }

        break;
    }

    match plan {
        Some(Plan::WholeNodes(n)) => {
            for _ in 0..n {
                recorder.pop_front();
            }

            true
        }

        Some(Plan::SplitLast {
            whole_before,
            remainder,
        }) => {
            for _ in 0..whole_before {
                recorder.pop_front();
            }

            if remainder.is_empty() {
                recorder.pop_front();
            } else if let Some(InlineNode::Text { value, .. }) = recorder.front_mut() {
                *value = CowStr::Boxed(remainder.into_boxed_str());
            }

            true
        }

        None => false,
    }
}

/// Asserts that `r` (from the recorder) and `b` (from the builder) are the
/// same node kind, with the same logical content, recursing into any
/// children.
fn assert_node_equivalent(r: &InlineNode<'_>, b: &InlineNode<'_>, source: &str) {
    match (r, b) {
        (InlineNode::Text { value: rv, .. }, InlineNode::Text { value: bv, .. }) => {
            assert_eq!(rv, bv, "Text value differs for {source:?}");
        }

        (InlineNode::CharRef { value: rv, .. }, InlineNode::CharRef { value: bv, .. }) => {
            // A genuine mismatch reaches here only when
            // `consume_rendered_prefix` (tried first, in
            // `assert_trees_equivalent`) already failed to reconcile the two,
            // so a plain equality check gives the clearest diagnostic.
            assert_eq!(rv, bv, "CharRef value differs for {source:?}");
        }

        (InlineNode::Raw { value: rv, .. }, InlineNode::Raw { value: bv, .. }) => {
            assert_eq!(rv, bv, "Raw value differs for {source:?}");
        }

        (InlineNode::Styled(rs), InlineNode::Styled(bs)) => {
            assert_eq!(
                rs.variant, bs.variant,
                "Styled variant differs for {source:?}"
            );
            assert_eq!(rs.form, bs.form, "Styled form differs for {source:?}");
            assert_eq!(rs.id, bs.id, "Styled id differs for {source:?}");
            assert_eq!(rs.roles, bs.roles, "Styled roles differ for {source:?}");
            assert_trees_equivalent(&rs.children, &bs.children, source);
        }

        (InlineNode::Ref(rr), InlineNode::Ref(br)) => {
            assert_eq!(rr.variant, br.variant, "Ref variant differs for {source:?}");
            assert_eq!(rr.target, br.target, "Ref target differs for {source:?}");

            // `link_form` is intentionally never compared: the recorder
            // recovers a link from its rendered `<a …>` markup, which all
            // three spellings share, so it is always `None` there, while the
            // builder carries the spelling as a structural fact (the same
            // one-sided richness an anchor's `reftext` and an image's
            // `is_icon` have).
            let _ = (&rr.link_form, &br.link_form);

            // When a `Link`'s display text carried its own attribute list,
            // `render_link` (and so `fold_link`) reads `role`/`window`
            // straight off `attrs` rather than the plain `roles`/`window`
            // fields (`Ref::attrs`'s own doc comment) — and indeed the
            // builder never populates those fields from the attrlist in
            // that case (only an auto-link's `bare` role, or a `^` window
            // suffix, ever populate them directly). The recorder, working
            // from the renderer's own params, still reports the role/window
            // that actually rendered, so the two are expected to differ
            // whenever `attrs` is present; they are compared exactly
            // otherwise.
            if br.attrs.is_none() {
                assert_eq!(rr.roles, br.roles, "Ref roles differ for {source:?}");
                assert_eq!(rr.window, br.window, "Ref window differs for {source:?}");
            }

            // See the module doc comment's own note on this: an unresolved
            // cross-reference's display text never reaches the recorder's
            // recovered `children` at all (Asciidoctor's own fallback
            // renders the bracketed target, not the text), while the
            // builder always bakes it in. This exemption is scoped as
            // narrowly as the asymmetry itself: it fires only when the
            // recorder's `children` is empty *and* the builder's is not —
            // exactly the shape the documented gap produces. Both sides
            // empty (a shorthand `<<id>>` with no display text at all,
            // where there is nothing to lose) still goes through the
            // ordinary comparison below, and a recorder holding unexpected
            // content is never silently waved through.
            let unresolved_xref_text =
                rr.variant == RefVariant::Xref && rr.children.is_empty() && !br.children.is_empty();

            if !unresolved_xref_text {
                assert_trees_equivalent(&rr.children, &br.children, source);
            }
        }

        (InlineNode::Image(ri), InlineNode::Image(bi)) => {
            // `is_icon` is intentionally never compared: the recorder does
            // not distinguish `icon:` from `image:` (both fold through the
            // same recorded marker) and always reports `false` (see the
            // production `leaf_node_of`'s own doc comment in
            // `content/inline_tree.rs`), while the builder sets it honestly.
            let _ = (ri.is_icon, bi.is_icon);

            assert_eq!(ri.target, bi.target, "Image target differs for {source:?}");
            assert_eq!(ri.alt, bi.alt, "Image alt differs for {source:?}");
            assert_eq!(ri.width, bi.width, "Image width differs for {source:?}");
            assert_eq!(ri.height, bi.height, "Image height differs for {source:?}");
        }

        (InlineNode::Footnote(rf), InlineNode::Footnote(bf)) => {
            assert_eq!(
                rf.is_reference, bf.is_reference,
                "Footnote is_reference differs for {source:?}"
            );
            assert_eq!(
                rf.number, bf.number,
                "Footnote number differs for {source:?}"
            );

            // A *resolved* reference occurrence's `id` never reaches the
            // renderer's own params at all — both `fold_footnote` and the
            // string pipeline's own replacer render just the resolved
            // number, dropping `id` (see `fold_footnote` in
            // `inline_builder/fold.rs`) — so the recorder has nothing to
            // recover it from; the builder still carries it as a structural
            // fact about the node regardless. The defining occurrence and an
            // *unresolved* reference (whose fallback rendering does use
            // `id`) are still compared exactly.
            let id_unobservable_by_recorder = bf.is_reference && bf.number.is_some();

            if !id_unobservable_by_recorder {
                assert_eq!(rf.id, bf.id, "Footnote id differs for {source:?}");
            }

            assert_trees_equivalent(&rf.children, &bf.children, source);
        }

        (InlineNode::Anchor(ra), InlineNode::Anchor(ba)) => {
            assert_eq!(ra.id, ba.id, "Anchor id differs for {source:?}");

            // `reftext` is intentionally never compared: an anchor's HTML is
            // a function of its `id` alone (design's own note on the anchor
            // increment), so it renders nothing the recorder could recover
            // `reftext` from and is always `None` there, while the builder
            // still carries it as a structural fact (see the module doc
            // comment).
            let _ = (&ra.reftext, &ba.reftext);
        }

        (InlineNode::Ui(ru), InlineNode::Ui(bu)) => {
            assert_eq!(ru.kind, bu.kind, "Ui kind differs for {source:?}");
        }

        (InlineNode::IndexTerm(ri), InlineNode::IndexTerm(bi)) => {
            // A visible term's `children` are the builder's one-sided
            // richness, of exactly the kind this sweep already records
            // elsewhere: the recorder recovers a shown term out of the
            // finished string, so it holds that text — and any span's markup
            // inside it — as a string alone, where the builder carries the
            // shown text as nodes and lets the fold render them (see
            // `IndexTerm::children`). So `children` is not compared, and
            // `terms` is compared wherever the builder computed one. A term
            // *enclosing a rendered span* is the case where it could not: its
            // markup exists only at fold time, so no string built at parse
            // time can spell it and the builder leaves `terms` empty.
            if !bi.terms.is_empty() {
                assert_eq!(ri.terms, bi.terms, "IndexTerm terms differ for {source:?}");
            }

            assert_eq!(
                ri.visible, bi.visible,
                "IndexTerm visible differs for {source:?}"
            );
        }

        (InlineNode::Callout(rc), InlineNode::Callout(bc)) => {
            assert_eq!(
                rc.number, bc.number,
                "Callout number differs for {source:?}"
            );
            assert_eq!(rc.guard, bc.guard, "Callout guard differs for {source:?}");
        }

        (InlineNode::Stem(rs), InlineNode::Stem(bs)) => {
            assert_eq!(
                rs.notation, bs.notation,
                "Stem notation differs for {source:?}"
            );
            assert_eq!(rs.value, bs.value, "Stem value differs for {source:?}");
        }

        (InlineNode::LineBreak { .. }, InlineNode::LineBreak { .. }) => {}

        (r, b) => panic!("node kind differs for {source:?}: recorder={r:?} builder={b:?}"),
    }
}

// ─── The freeze ─────────────────────────────────────────────────────────────

/// The recording this corpus's recorder side is frozen into.
const RECORDING: &str = "recorder_trees";

/// The recorder's tree for `source`, read back from the recording.
///
/// The recorder side is the half of this differential that dies with
/// `SubstitutionGroup::apply_string_pipeline`: [`RecordingRenderer`] recovers a
/// tree out of what that pipeline *renders*, so once the pipeline goes there is
/// no recorder tree to compare the builder's against, and the module would be
/// comparing `build` to itself. Design §5.2's survey called this the
/// **tree-shaped** freeze and the harder half by a wide margin — because,
/// unlike the two record-shaped corpora, what the two sides hold is not equal
/// and never was. [`assert_trees_equivalent`] is a *pairwise normalization*:
/// it ignores `location` and `attrs`, resolves leaf-boundary differences by
/// folding a builder leaf and consuming the recorder's rendered bytes, and can
/// split a recorder `Text` run to meet a builder leaf's edge. A freeze needs a
/// **per-side normal form**, which a pairwise diff is not.
///
/// The normal form this uses is the one the recorder itself already satisfies.
/// The module doc comment above enumerates, field by field, everything a
/// recorder-built node cannot carry — `attrs`, `derived`, `xrefstyle`,
/// `resolved`, an anchor's `reftext`, an image's `is_icon`, a ref's
/// `link_form` — and `location` is the whole-content span on every one of them.
/// So a recorder tree is *already* in a restricted form, and the recording
/// carries exactly the fields [`assert_node_equivalent`] and
/// [`consume_rendered_prefix`] read. Decoding rebuilds real
/// [`InlineNode`] values with those fields restored and every other field at
/// the value the recorder always gives it — which is why **not one assertion in
/// this module moves**, and why the comparator is untouched.
///
/// What keeps that honest while the pipeline still exists is
/// [`recorded_golden`]'s own drift guard: the live recorder is still built on
/// every fixture and its normal form still has to equal the recorded bytes.
/// What keeps it honest afterwards is the `strip_unrecorded` assertion
/// [`assert_shapes_with`] runs beside this call, on every fixture — the guard
/// against the one hazard a partial normal form has, the comparator growing a
/// read of a field the recording does not carry. See [`strip_unrecorded`].
fn frozen_recorder_tree<'src>(source: &'src str, live: &[InlineNode<'_>]) -> Vec<InlineNode<'src>> {
    let encoded = encode_nodes(live);
    decode_nodes(
        &recorded_golden(RECORDING, source, &encoded),
        Span::new(source),
    )
}

/// Encodes a node slice as one physical line: a decimal count, then each node
/// depth-first, every field tab-separated.
///
/// Counted rather than delimited, the way the side-effect corpus's own record
/// is, and for the same reason — a string field can hold any byte, including
/// whatever a delimiter would have used. Here the count does double duty: it is
/// also what nests, since a parent writes its child count and then its children
/// inline, so one flat field stream carries a tree.
fn encode_nodes(nodes: &[InlineNode<'_>]) -> String {
    let mut fields: Vec<String> = vec![];
    push_nodes(&mut fields, nodes);
    fields.join("\t")
}

fn push_nodes(fields: &mut Vec<String>, nodes: &[InlineNode<'_>]) {
    fields.push(nodes.len().to_string());

    for node in nodes {
        push_node(fields, node);
    }
}

/// One node: a kind tag, the fields the comparator reads for that kind, and
/// then — for a kind with children — its subtree.
///
/// A kind the recorder never builds is not written at all. `Raw` is the only
/// one: it is a builder-side leaf (a passthrough), and the recorder recovers
/// the same content as a mix of `Text` and `CharRef` out of the rendered bytes
/// — which is exactly the leaf-boundary difference `consume_rendered_prefix`
/// exists to resolve. Encoding it would be encoding a shape no recording can
/// ever hold.
fn push_node(fields: &mut Vec<String>, node: &InlineNode<'_>) {
    match node {
        InlineNode::Text { value, .. } => {
            fields.push("Text".to_string());
            fields.push(quote(value.as_ref()));
        }

        InlineNode::CharRef { value, .. } => {
            fields.push("CharRef".to_string());

            match value {
                CharRef::Special(c) => {
                    fields.push("Special".to_string());
                    fields.push(quote(&c.to_string()));
                }

                CharRef::Replacement(s) => {
                    fields.push("Replacement".to_string());
                    fields.push(quote(s));
                }

                CharRef::Entity(name) => {
                    fields.push("Entity".to_string());
                    fields.push(quote(name.as_ref()));
                }
            }
        }

        InlineNode::Styled(styled) => {
            fields.push("Styled".to_string());
            fields.push(format!("{:?}", styled.variant));
            fields.push(format!("{:?}", styled.form));
            push_option(fields, styled.id.as_deref());
            push_strings(fields, &styled.roles);
            push_nodes(fields, &styled.children);
        }

        InlineNode::Ref(ref_) => {
            fields.push("Ref".to_string());
            fields.push(format!("{:?}", ref_.variant));
            fields.push(quote(ref_.target.as_ref()));
            push_option(fields, ref_.window.as_deref());
            push_strings(fields, &ref_.roles);
            push_nodes(fields, &ref_.children);
        }

        InlineNode::Image(image) => {
            fields.push("Image".to_string());
            fields.push(quote(image.target.as_ref()));
            push_option(fields, image.alt.as_deref());
            push_option(fields, image.width.as_deref());
            push_option(fields, image.height.as_deref());
        }

        InlineNode::Footnote(footnote) => {
            fields.push("Footnote".to_string());
            push_option(fields, footnote.id.as_deref());
            push_option(fields, footnote.number.as_deref());
            fields.push(footnote.is_reference.to_string());
            push_nodes(fields, &footnote.children);
        }

        InlineNode::Anchor(anchor) => {
            fields.push("Anchor".to_string());
            fields.push(quote(anchor.id.as_ref()));
        }

        InlineNode::Ui(ui) => {
            fields.push("Ui".to_string());

            match &ui.kind {
                UiKind::Keyboard(keys) => {
                    fields.push("Keyboard".to_string());
                    push_strings(fields, keys);
                }

                UiKind::Button(label) => {
                    fields.push("Button".to_string());
                    fields.push(quote(label.as_ref()));
                }

                UiKind::Menu {
                    menu,
                    submenus,
                    item,
                } => {
                    fields.push("Menu".to_string());
                    fields.push(quote(menu.as_ref()));
                    push_strings(fields, submenus);
                    push_option(fields, item.as_deref());
                }
            }
        }

        InlineNode::IndexTerm(term) => {
            fields.push("IndexTerm".to_string());
            push_strings(fields, &term.terms);
            fields.push(term.visible.to_string());
        }

        InlineNode::Callout(callout) => {
            fields.push("Callout".to_string());
            fields.push(quote(callout.number.as_ref()));

            match &callout.guard {
                CalloutGuard::LineComment(prefix) => {
                    fields.push("LineComment".to_string());
                    fields.push(quote(prefix.as_ref()));
                }

                CalloutGuard::Xml => fields.push("Xml".to_string()),
            }
        }

        InlineNode::Stem(stem) => {
            fields.push("Stem".to_string());
            fields.push(format!("{:?}", stem.notation));
            fields.push(quote(stem.value.as_ref()));
        }

        InlineNode::LineBreak { .. } => fields.push("LineBreak".to_string()),

        InlineNode::Raw { .. } => panic!(
            "the recorder cannot build a Raw node — see `push_node` — so one reached the \
             recording that should not have: {node:?}"
        ),
    }
}

/// A string list as a count followed by its quoted items.
fn push_strings(fields: &mut Vec<String>, items: &[CowStr<'_>]) {
    fields.push(items.len().to_string());
    fields.extend(items.iter().map(|item| quote(item.as_ref())));
}

/// A present string as [`quote`] writes it, `None` as a bare `-` — which a
/// present value can never be mistaken for, since a quoted field always begins
/// with `"`.
fn push_option(fields: &mut Vec<String>, value: Option<&str>) {
    fields.push(value.map_or_else(|| "-".to_string(), quote));
}

/// Reverses [`encode_nodes`], rebuilding real [`InlineNode`] values.
///
/// `location` is the whole-content span on every node, which is not an
/// approximation: it is exactly what a recorder-built node carries (the design
/// §4.4 "migration stage" fallback the module doc comment names), and the
/// comparator excludes `location` from the comparison regardless. Every other
/// field the recording does not carry is set to the value the recorder always
/// gives it — `None`, `false`, or empty — for the reasons the module doc
/// comment enumerates one by one.
fn decode_nodes<'src>(encoded: &str, location: Span<'src>) -> Vec<InlineNode<'src>> {
    let mut fields = TreeFields::new(encoded);
    let nodes = fields.nodes(location);

    assert!(
        fields.exhausted(),
        "trailing fields in {RECORDING}.txt: {encoded:?}"
    );

    nodes
}

/// A left-to-right cursor over one record's tab-separated fields.
///
/// A struct rather than closures because decoding is recursive and every level
/// advances the same position.
struct TreeFields<'a> {
    fields: Vec<&'a str>,
    at: usize,
}

impl<'a> TreeFields<'a> {
    fn new(encoded: &'a str) -> Self {
        Self {
            // `"".split('\t')` yields one empty field rather than none, which
            // `count` would read as a malformed count. `encode_nodes` always
            // writes at least the top-level count, so an empty encoding guards
            // a corrupted recording rather than an empty tree (which encodes
            // as `"0"`).
            fields: if encoded.is_empty() {
                vec![]
            } else {
                encoded.split('\t').collect()
            },
            at: 0,
        }
    }

    fn next(&mut self, what: &str) -> String {
        let field = self
            .fields
            .get(self.at)
            .unwrap_or_else(|| panic!("truncated record in {RECORDING}.txt: missing {what}"));

        self.at += 1;
        (*field).to_string()
    }

    fn count(&mut self, what: &str) -> usize {
        let field = self.next(what);

        field
            .parse()
            .unwrap_or_else(|_| panic!("bad {what} count in {RECORDING}.txt: {field:?}"))
    }

    fn string(&mut self, what: &str) -> CowStr<'static> {
        CowStr::from(unquote(RECORDING, &self.next(what)))
    }

    fn option(&mut self, what: &str) -> Option<CowStr<'static>> {
        let field = self.next(what);
        (field != "-").then(|| CowStr::from(unquote(RECORDING, &field)))
    }

    fn strings(&mut self, what: &str) -> Vec<CowStr<'static>> {
        (0..self.count(what)).map(|_| self.string(what)).collect()
    }

    fn bool(&mut self, what: &str) -> bool {
        let field = self.next(what);

        field
            .parse()
            .unwrap_or_else(|_| panic!("bad {what} flag in {RECORDING}.txt: {field:?}"))
    }

    fn exhausted(&self) -> bool {
        self.at == self.fields.len()
    }

    fn nodes<'src>(&mut self, location: Span<'src>) -> Vec<InlineNode<'src>> {
        (0..self.count("node"))
            .map(|_| self.node(location))
            .collect()
    }

    fn node<'src>(&mut self, location: Span<'src>) -> InlineNode<'src> {
        let kind = self.next("node kind");

        match kind.as_str() {
            "Text" => InlineNode::Text {
                value: self.string("Text value"),
                location,
            },

            "CharRef" => InlineNode::CharRef {
                value: self.char_ref(),
                location,
            },

            "Styled" => InlineNode::Styled(Styled {
                variant: decode_style_variant(&self.next("Styled variant")),
                form: decode_span_form(&self.next("Styled form")),
                id: self.option("Styled id"),
                roles: self.strings("Styled role"),
                attrs: None,
                children: self.nodes(location),
                passthrough: None,
                location,
            }),

            "Ref" => InlineNode::Ref(Ref {
                variant: decode_ref_variant(&self.next("Ref variant")),
                target: self.string("Ref target"),
                window: self.option("Ref window"),
                roles: self.strings("Ref role"),
                children: self.nodes(location),
                resolved: None,
                derived: None,
                xrefstyle: None,
                attrs: None,
                link_form: None,
                location,
            }),

            "Image" => InlineNode::Image(Image {
                is_icon: false,
                target: self.string("Image target"),
                restored_target_ranges: vec![],
                alt: self.option("Image alt"),
                width: self.option("Image width"),
                height: self.option("Image height"),
                attrs: None,
                location,
            }),

            "Footnote" => InlineNode::Footnote(Footnote {
                id: self.option("Footnote id"),
                number: self.option("Footnote number"),
                is_reference: self.bool("Footnote is_reference"),
                children: self.nodes(location),
                location,
            }),

            "Anchor" => InlineNode::Anchor(Anchor {
                id: self.string("Anchor id"),
                reftext: None,
                is_bibliography: false,
                location,
            }),

            "Ui" => InlineNode::Ui(Ui {
                kind: self.ui_kind(),
                location,
            }),

            "IndexTerm" => InlineNode::IndexTerm(IndexTerm {
                terms: self.strings("IndexTerm term"),
                children: vec![],
                visible: self.bool("IndexTerm visible"),
                location,
            }),

            "Callout" => InlineNode::Callout(Callout {
                number: self.string("Callout number"),
                guard: self.callout_guard(),
                location,
            }),

            "Stem" => InlineNode::Stem(Stem {
                notation: decode_stem_notation(&self.next("Stem notation")),
                value: self.string("Stem value"),
                subs: SubstitutionGroup::Stem,
                source_text: None,
                children: vec![],
                location,
            }),

            "LineBreak" => InlineNode::LineBreak { location },

            other => panic!("unrecognized node kind in {RECORDING}.txt: {other:?}"),
        }
    }

    /// A [`CharRef`], whose `Replacement` arm holds a `&'static str` and so
    /// cannot simply be rebuilt from an owned decoded string.
    ///
    /// [`RECORDER_ENTITY_TABLE`] supplies the `'static` value, which is the
    /// right source for it rather than a convenience: the table *is* the set of
    /// replacements a recorder-built `CharRef` can hold, since the recorder
    /// recovers every one of them by feeding an entity it found in the rendered
    /// output through `classify_entity`. A multi-character replacement is not
    /// reachable here for the same reason — the recorder recovers one leaf per
    /// entity — and
    /// [`recorder_entity_table_matches_production_classify_entity`] already
    /// guards the table against drifting from that production function.
    fn char_ref(&mut self) -> CharRef<'static> {
        let kind = self.next("CharRef kind");

        match kind.as_str() {
            "Special" => {
                let value = unquote(RECORDING, &self.next("CharRef special"));
                let mut chars = value.chars();

                let c = chars
                    .next()
                    .unwrap_or_else(|| panic!("empty CharRef::Special in {RECORDING}.txt"));

                assert!(
                    chars.next().is_none(),
                    "multi-character CharRef::Special in {RECORDING}.txt: {value:?}"
                );

                CharRef::Special(c)
            }

            "Replacement" => {
                let value = unquote(RECORDING, &self.next("CharRef replacement"));

                RECORDER_ENTITY_TABLE
                    .iter()
                    .find_map(|(_, char_ref)| match char_ref {
                        CharRef::Replacement(s) if *s == value => Some(CharRef::Replacement(s)),
                        _ => None,
                    })
                    .unwrap_or_else(|| {
                        panic!(
                            "CharRef::Replacement {value:?} in {RECORDING}.txt is not one the \
                             recorder can build — RECORDER_ENTITY_TABLE has no entry for it"
                        )
                    })
            }

            "Entity" => CharRef::Entity(self.string("CharRef entity")),

            other => panic!("unrecognized CharRef kind in {RECORDING}.txt: {other:?}"),
        }
    }

    fn ui_kind(&mut self) -> UiKind<'static> {
        let kind = self.next("Ui kind");

        match kind.as_str() {
            "Keyboard" => UiKind::Keyboard(self.strings("Ui key")),
            "Button" => UiKind::Button(self.string("Ui label")),

            "Menu" => UiKind::Menu {
                menu: self.string("Ui menu"),
                submenus: self.strings("Ui submenu"),
                item: self.option("Ui item"),
            },

            other => panic!("unrecognized Ui kind in {RECORDING}.txt: {other:?}"),
        }
    }

    fn callout_guard(&mut self) -> CalloutGuard<'static> {
        let kind = self.next("Callout guard");

        match kind.as_str() {
            "LineComment" => CalloutGuard::LineComment(self.string("Callout guard prefix")),
            "Xml" => CalloutGuard::Xml,
            other => panic!("unrecognized Callout guard in {RECORDING}.txt: {other:?}"),
        }
    }
}

/// The four small field enums, each recorded as its derived `Debug` spelling
/// and decoded by an exhaustive match.
///
/// Exhaustive rather than a `Debug`-string comparison in the record itself:
/// these are the fields [`assert_node_equivalent`] compares *by value*, so a
/// decoded tree has to hold the real variant. Each `panic!` arm is the guard
/// that a variant added to one of these enums cannot silently decode as
/// something else — a recording naming it fails loudly instead.
fn decode_style_variant(field: &str) -> StyleVariant {
    match field {
        "Strong" => StyleVariant::Strong,
        "Emphasis" => StyleVariant::Emphasis,
        "Code" => StyleVariant::Code,
        "Mark" => StyleVariant::Mark,
        "Superscript" => StyleVariant::Superscript,
        "Subscript" => StyleVariant::Subscript,
        "DoubleQuote" => StyleVariant::DoubleQuote,
        "SingleQuote" => StyleVariant::SingleQuote,
        "Unquoted" => StyleVariant::Unquoted,
        other => panic!("unrecognized style variant in {RECORDING}.txt: {other:?}"),
    }
}

fn decode_span_form(field: &str) -> SpanForm {
    match field {
        "Constrained" => SpanForm::Constrained,
        "Unconstrained" => SpanForm::Unconstrained,
        other => panic!("unrecognized span form in {RECORDING}.txt: {other:?}"),
    }
}

fn decode_ref_variant(field: &str) -> RefVariant {
    match field {
        "Link" => RefVariant::Link,
        "Xref" => RefVariant::Xref,
        other => panic!("unrecognized ref variant in {RECORDING}.txt: {other:?}"),
    }
}

fn decode_stem_notation(field: &str) -> StemNotation {
    match field {
        "AsciiMath" => StemNotation::AsciiMath,
        "LatexMath" => StemNotation::LatexMath,
        other => panic!("unrecognized stem notation in {RECORDING}.txt: {other:?}"),
    }
}

/// Clears exactly the fields [`encode_nodes`] does **not** carry, so a live
/// recorder tree can be compared to a decoded one by plain equality.
///
/// This is the whole of the freeze's risk surface, written down in one place.
/// A partial normal form has exactly one hazard: a field the comparator reads
/// that the recording does not carry decodes as a default, and the comparison
/// silently weakens with every test still green. Enumerating the dropped fields
/// here — rather than reasoning about them — turns that into a **total** check:
/// [`assert_shapes_with`] asserts `strip(live) == decoded` for every fixture
/// the corpus drives, so a field added to `InlineNode` (or newly populated by
/// the recorder) fails loudly until someone decides whether the recording
/// should carry it.
///
/// Every field cleared here is one the module doc comment already documents as
/// unobservable on the recorder side, or one no assertion reads:
/// `attrs`/`derived`/`xrefstyle`/`resolved`/`link_form`/`reftext`/`is_icon` are
/// the documented always-`None`/`false` set; `location` is the whole-content
/// span the design §4.4 migration stage gives every recorder node, and the
/// comparator excludes it outright; and `Styled::passthrough`,
/// `IndexTerm::children`, `Image::restored_target_ranges` and a `Stem`'s
/// `subs`/`source_text`/`children` are builder-only structure the comparator
/// never reads on either side.
fn strip_unrecorded<'src>(
    nodes: &[InlineNode<'src>],
    location: Span<'src>,
) -> Vec<InlineNode<'src>> {
    nodes
        .iter()
        .map(|node| match node.clone() {
            InlineNode::Text { value, .. } => InlineNode::Text { value, location },

            InlineNode::CharRef { value, .. } => InlineNode::CharRef { value, location },

            InlineNode::Raw {
                value,
                form,
                origin,
                ..
            } => InlineNode::Raw {
                value,
                form,
                origin,
                location,
            },

            InlineNode::Styled(mut styled) => {
                styled.attrs = None;
                styled.passthrough = None;
                styled.children = strip_unrecorded(&styled.children, location);
                styled.location = location;
                InlineNode::Styled(styled)
            }

            InlineNode::Ref(mut ref_) => {
                ref_.resolved = None;
                ref_.derived = None;
                ref_.xrefstyle = None;
                ref_.attrs = None;
                ref_.link_form = None;
                ref_.children = strip_unrecorded(&ref_.children, location);
                ref_.location = location;
                InlineNode::Ref(ref_)
            }

            InlineNode::Image(mut image) => {
                image.is_icon = false;
                image.restored_target_ranges = vec![];
                image.attrs = None;
                image.location = location;
                InlineNode::Image(image)
            }

            InlineNode::Footnote(mut footnote) => {
                footnote.children = strip_unrecorded(&footnote.children, location);
                footnote.location = location;
                InlineNode::Footnote(footnote)
            }

            InlineNode::Anchor(mut anchor) => {
                anchor.reftext = None;
                anchor.is_bibliography = false;
                anchor.location = location;
                InlineNode::Anchor(anchor)
            }

            InlineNode::Ui(mut ui) => {
                ui.location = location;
                InlineNode::Ui(ui)
            }

            InlineNode::IndexTerm(mut term) => {
                term.children = vec![];
                term.location = location;
                InlineNode::IndexTerm(term)
            }

            InlineNode::Callout(mut callout) => {
                callout.location = location;
                InlineNode::Callout(callout)
            }

            InlineNode::Stem(mut stem) => {
                stem.subs = SubstitutionGroup::Stem;
                stem.source_text = None;
                stem.children = vec![];
                stem.location = location;
                InlineNode::Stem(stem)
            }

            InlineNode::LineBreak { .. } => InlineNode::LineBreak { location },
        })
        .collect()
}

// ─── Corpus ─────────────────────────────────────────────────────────────────

/// A broad, general-purpose sweep of inline fixtures under a default
/// document — the same fixture set (duplicated verbatim, not shared code)
/// [`inline_builder`](crate::content::inline_builder)'s own
/// `fold_matches_the_real_pipeline_across_a_broad_general_purpose_sweep` test
/// uses to pin HTML-fold parity against the real pipeline. Reusing a
/// fixture set already known to stay inside the builder's claimed vocabulary
/// keeps this corpus meaningful: a mismatch here is either a genuine
/// structural bug or a sign that the fixture set drifted out of that
/// vocabulary (in which case the HTML-fold test above would fail too).
#[test]
fn shapes_match_across_a_broad_general_purpose_sweep() {
    let fixtures = [
        "",
        "plain text with no constructs",
        "text with trailing spaces   and   runs",
        "a < b && c > d",
        "1 < 2 & 3 > 0",
        "<>&",
        "One *word* is strong.",
        "An _emphasized_ phrase.",
        "Some `monospaced` text.",
        "A #highlighted# span.",
        "H~2~O and E = mc^2^.",
        "A bold *phrase of text* here.",
        "Bold c**hara**cter**s** in a word.",
        "un__frac__tured emphasis",
        "a##b##c",
        "*a _b_ c*",
        "*bold _and italic_ mix*",
        "_*strong inside emphasis*_",
        "[.myrole]#roled text#",
        "[#anchor]#anchored#",
        "[.a.b]#multi role#",
        "*a < b* and _c > d_",
        "code `x < y && z` here",
        "\"`double`\" and '`single`' quotes",
        "(C) (R) (TM)",
        "An em -- dash and an ellipsis...",
        "arrows -> => <- <=",
        "Sam's apostrophe",
        "named &amp; numeric &#8217; entities",
        "plain {backend} attribute",
        "See https://example.org for details.",
        "An angle-bracketed <https://example.org> link.",
        "<https://example.org[the site] keeps its bracket.",
        "A link:https://example.org[example] link.",
        "mailto:a@b.com[email me]",
        "write to doc.writer@example.com today",
        "write to a&b@example.com today",
        "an image:photo.png[Alt Text] inline",
        "image:pic.png[Scaled,200,100]",
        "an image:a&b.png[Query] inline",
        "see <<target>> for more",
        "see <<target,the target>> now",
        "see <<target,Tom & Jerry>> now",
        "xref:other.adoc#frag[Other] doc",
        "xref:target[a < b & c] doc",
        "xref:target[the *bold* steps,role=hl] doc",
        "link:a&b.html[x] macro",
        "link:index.html[Tom & Jerry] macro",
        "mailto:a&b@example.org[] address",
        "https://example.org/?a=1&b=2 auto-link",
        "https://example.org/a&b[Text] formal",
        "https://example.org[Tom & Jerry] formal",
        "<https://example.org/x&y> angle",
        r"http://google.com[\{google_homepage}]",
        "A claim.footnote:[the evidence]",
        "Named.footnote:disc[a discussion] then footnote:disc[].",
        "A claim.footnote:[the *strong* evidence and a link:https://e.org[source]]",
        "Two notes.footnote:[first] and again.footnote:[second]",
        r"A claim.footnote:[see \[the appendix\]] here.",
        r"footnoteref:[disc,a note ending in a\]bracket]",
        "Press kbd:[Ctrl+T] now.",
        "Press kbd:[Ctrl,Shift,N] now.",
        "Click btn:[Save] please.",
        "Choose menu:File[Save As > PDF].",
        "A flow ((visible term)) here.",
        "A concealed (((primary,secondary))) term.",
        "[[the-anchor]]Anchored paragraph.",
        "first line +\nsecond line",
        "only +",
        "*bold* _em_ `code` (C) https://x.y[link] <<ref>> image:i.png[i]",
        "A mix of {backend}, *bold < text*, and a footnote:[with `code`].",
        r"a \*not bold* b",
        r"an \_not emphasized_ here",
        r"literal \`backtick\` text",
        "*a _b `c` d_ e*",
        "*one* *two* *three*",
        "_a_ and _b_ and _c_",
        "`code with *stars* inside`",
        "pre**mid**post and a__b__c",
        "x^sup^y and p~sub~q",
        "[.role1.role2#the-id]#decorated#",
        "[#only-id]#text#",
        "`a < b > c & d`",
        "(C) then (R) then (TM) end",
        "First... then -- and -> arrows <-",
        "a &amp; b and &#8482; and &copy;",
        "an {undefined-attr} reference",
        "link:https://example.org[Example,role=external,window=_blank]",
        "https://example.org[Example^]",
        "a https://example.org[] bare-macro link",
        "visit https://example.org/path?q=1 now",
        "image:a.png[An image with spaces,role=thumb]",
        "before image:b.svg[Vector] after",
        "<<a>> and <<b>> and <<c,C text>> and <<d,>>",
        "a ((flow term)) and (((c1, c2, c3))) end",
        "indexterm:[primary, secondary]",
        "indexterm2:[shown]",
        "indexterm2:[Flash,see=HTML 5] then indexterm2:[see-also=\"CSS 3\"]",
        r"an escaped \(((Coffee))) shorthand and its \((literal)) twin",
        "a ((*bold* term)) and an indexterm2:[_em_ term] enclosing a span",
        "[[mid-anchor]] after the anchor",
        "text before anchor:named[Ref Text] and after",
        "a +++<b>raw</b>+++ passthrough",
        // A bare `+…+` form whose body encloses a construct the first
        // extraction pass already replaced is deliberately *not* in this
        // sweep. The builder makes one `Raw` leaf whose value already carries
        // the inner passthrough's restored body — what the string pipeline's
        // own entry holds by restore time — while the recorder, recovering
        // structure from the finished string, sees the inner construct's own
        // markers and splits the same text into several leaves. That is the
        // leaf-boundary asymmetry this module's doc comment describes, not a
        // divergence: the two fold to the same bytes, which the whole-pipeline
        // sweeps pin
        // (`fold_matches_the_string_pipeline_for_a_bare_form_over_an_extracted_passthrough`).
        r"an escaped \[attrs]++<b>*x*</b>++ bracket",
        r"an escaped \[x-]++*bold*++ bracket",
        "a prohibited index:[attrs]+text+ prefix",
        r"a prohibited \[x-]`text` prefix",
        "inline pass:[<i>x</i>] macro",
        "math $$a < b$$ here",
        "a +literal *stars*+ b",
        "line one +\nline two +\nline three",
        "line one\nline two +",
        "*bold +*",
        "*bold +\nmore +*",
        "stem:[a < b] expression",
        "asciimath:[a < b] inline",
        "latexmath:[x < y] inline",
        "an icon:home[] icon",
        "icon:star[2x,role=gold] rated",
        // A construct written against an enclosing span's own edge, where the
        // string pipeline's haystack holds that span's rendered markup: the
        // recorder recovers what actually rendered, so this compares the
        // builder's boundary-context decision against the string pipeline's
        // own reading structurally as well as by the bytes it folds to.
        r#""``end points``""#,
        r#""`_e_`""#,
        r#""`x `code` y`""#,
        "*x --*",
        "*-- x*",
        "[.r]#x --#",
        r#""`x --`""#,
        // The same seam for the macros step's own boundary-reading families —
        // the bare e-mail's mismatch prefix and the auto-link's boundary
        // prefix — where the recorder, recovering what actually rendered,
        // reads the string pipeline's own answer.
        "*doc@example.org*",
        "_doc@example.org writes_",
        r#""`doc@example.org`""#,
        "*write to doc@example.org now*",
        "*https://example.org*",
        r#""`https://example.org[Docs]`""#,
        "   ",
        "a\nb\nc",
    ];

    for fixture in fixtures {
        assert_shapes(fixture);
    }
}

/// Fixtures combining several construct families in one piece of content —
/// the same shape (and, again, duplicated fixtures rather than shared code)
/// as
/// [`inline_builder`](crate::content::inline_builder)'s own
/// `fold_matches_the_real_pipeline_across_combined_constructs` test, so a
/// boundary-crossing interaction between families that individually pass is
/// exercised structurally too.
#[test]
fn shapes_match_across_combined_constructs() {
    let with_product = || {
        Parser::default().with_intrinsic_attribute(
            "product",
            "Widget",
            ModificationContext::Anywhere,
        )
    };

    assert_shapes_with("The {product} is *fast* and reliable.", with_product);

    assert_shapes_with(
        "See <<intro>> for details.footnote:[Also check the {product} docs.]",
        with_product,
    );

    assert_shapes("+++<u>raw</u>+++ combined with *bold* and image:foo.png[Alt Text].");

    assert_shapes(
        r"An escaped \[.role *x*]++<b>raw</b>++ beside [.role]++<b>raw</b>++ and *bold*.",
    );

    assert_shapes(
        "An index:[.role]+<b>raw</b>+ beside [.role]+<b>raw</b>+ and *bold* and image:foo.png[Alt].",
    );

    assert_shapes("Equation stem:[x^2+y^2=z^2] appears in *bold* text with (C) 2024.");

    let with_experimental = || {
        Parser::default().with_intrinsic_attribute_bool(
            "experimental",
            true,
            ModificationContext::Anywhere,
        )
    };

    assert_shapes_with(
        "kbd:[Ctrl+Alt+Del] opens the *Task Manager* via menu:File[Save].",
        with_experimental,
    );

    // A menu's `&gt;` submenu form: the recorder recovers the submenu path
    // from the render params it intercepts, so this compares the two
    // constructions' own splitting of the item list, not just the HTML they
    // fold to.
    assert_shapes_with(
        "Choose menu:View[Zoom > Reset] in the *Task Manager* (C) 2024.",
        with_experimental,
    );

    // The same comparison for the family's last lift: a keyboard key crossing
    // an escaped special and a menu name crossing a restored entity. Both
    // constructions hold these values in their already-substituted form (the
    // recorder because it reads the string pipeline's own render params, the
    // builder because it reads the match string), so the two meet exactly.
    assert_shapes_with(
        "Press kbd:[Ctrl&C] then choose menu:&#8942;[More Tools, Extensions].",
        with_experimental,
    );

    assert_shapes(
        "Visit https://example.org[the site] or mailto:a@example.org[email us], \
         then see <<conclusion,the conclusion>>.",
    );

    assert_shapes(
        "Visit https://example.org or link:docs.html[the docs], \
         or just write to doc@example.org.",
    );

    // The same families reached through a **transparent** span, which renders
    // to its body and nothing else — so what a construct inside it reads is
    // whatever stands beside the span.
    assert_shapes(
        "Mail *write to [width=10]#doc@example.org# now* or \
         x [width=10]#doc@example.org# here.",
    );

    // A bare address whose local part carries an escaped special: the builder
    // recovers its shown text as structured children (the `&` staying its own
    // `CharRef`), which is the shape the recorder reaches from the opposite
    // direction — recovering it from the rendered anchor text.
    assert_shapes("Write to a&b@example.com or plain@example.org today.");

    // The last family to lift the escaped-special boundary: an image whose
    // target carries one, beside a plain image whose bracket does *not* — the
    // capture that still defers — so both the recognized and the literal shape
    // meet the recorder's own reconstruction.
    assert_shapes("See image:a&b.png[Chart] and image:plain.png[Alt Text] today.");

    // A *restored* entity, the second atomic piece the match string carries
    // real bytes for: a target crossing one, and a display text crossing one
    // (which the builder keeps as its own `CharRef::Entity` child, the shape
    // the recorder reaches by re-reading the rendered anchor text).
    assert_shapes("See image:a&copy;b.png[Chart] and link:x.html[a &copy; b] today.");

    // A *typographic replacement*, the third such piece: a target crossing
    // one, and a display text crossing one (which the builder keeps as its own
    // `CharRef::Replacement` child, the shape the recorder reaches by
    // re-reading the rendered anchor text).
    assert_shapes("See image:a(C)b.png[Chart] and link:x.html[a (C) b] today.");

    assert_shapes_with(
        "{counter:step}. Step one uses *{product}* and stem:[x+1].",
        with_product,
    );

    assert_shapes_with(
        "[[custom-id]]Anchored *text* referencing ((index term)) and {product}.",
        with_product,
    );

    assert_shapes_with(
        r"An escaped \*not bold\* attribute {product} and \(C) not replaced.",
        with_product,
    );

    assert_shapes_with(
        "First footnote:[a {product} note] then footnote:[b unrelated note], \
         and finally <<see-also>>.",
        with_product,
    );

    assert_shapes_with(
        "Line one uses *{product}*.\nLine two is plain.\nLine three too.",
        || {
            Parser::default()
                .with_intrinsic_attribute("product", "Widget", ModificationContext::Anywhere)
                .with_intrinsic_attribute_bool(
                    "hardbreaks-option",
                    true,
                    ModificationContext::Anywhere,
                )
        },
    );

    // A bibliography entry. The two constructions arrive at the same shape from
    // opposite directions: the builder emits the anchor node and then leaves the
    // bracketed label in the flow, while the recorder recovers that same
    // anchor-then-text sequence out of the rendered output (the string
    // replacer's `render_anchor` call is bracketed by markers, and the label it
    // pushes after it is not).
    let in_bibliography_list_item = || {
        let parser = Parser::default();
        parser.in_bibliography_list_item.set(true);
        parser
    };

    assert_shapes_with(
        "[[[gof,GoF]]] Gamma, Erich et al. _Design Patterns_.",
        in_bibliography_list_item,
    );

    assert_shapes_with(
        "[[[gof]]] An entry with an https://example.org link.",
        in_bibliography_list_item,
    );

    // A UI macro and an index term spliced in by attribute references. The
    // recorder has always recovered these (it reads the string pipeline's own
    // render params, and the pipeline expands the reference before matching);
    // the builder now recognizes them too, so the two constructions'
    // *structures* — not just the HTML they fold to — can finally be compared
    // for this shape.
    assert_shapes_with(
        "Press kbd:[{key}] then choose menu:{view}[Zoom > Reset] in *{product}*.",
        || {
            Parser::default()
                .with_intrinsic_attribute_bool("experimental", true, ModificationContext::Anywhere)
                .with_intrinsic_attribute("product", "Widget", ModificationContext::Anywhere)
                .with_intrinsic_attribute("view", "View", ModificationContext::Anywhere)
                .with_intrinsic_attribute("key", "Ctrl+T", ModificationContext::Anywhere)
        },
    );

    assert_shapes_with(
        "The (({product})) index term beside *bold* text and indexterm:[{product}, docs].",
        with_product,
    );

    // Cross-references spliced in by attribute references, in both spellings —
    // the same comparison, for the family that just made the same lift.
    assert_shapes_with(
        "See xref:{id}[{label}] and <<{id},the {product} steps>> here.",
        || {
            Parser::default()
                .with_intrinsic_attribute("id", "install", ModificationContext::Anywhere)
                .with_intrinsic_attribute("label", "Install Now", ModificationContext::Anywhere)
                .with_intrinsic_attribute("product", "Widget", ModificationContext::Anywhere)
        },
    );

    // Links and images spliced in by attribute references — the last two
    // families to make that lift. What the link family still defers (a wholly
    // expanded `link:` macro) is deliberately absent here: the recorder
    // recognizes it, so a fixture carrying one would compare a node against
    // literal text rather than two constructions of the same node.
    let with_link_attributes = || {
        Parser::default()
            .with_intrinsic_attribute("url", "index.html", ModificationContext::Anywhere)
            .with_intrinsic_attribute("host", "example.org", ModificationContext::Anywhere)
            .with_intrinsic_attribute("label", "Docs", ModificationContext::Anywhere)
            .with_intrinsic_attribute("logo", "sunset.jpg", ModificationContext::Anywhere)
            .with_intrinsic_attribute("product", "Widget", ModificationContext::Anywhere)
    };

    assert_shapes_with(
        "Read link:{url}[{label}] or visit https://{host}/docs about {product}.",
        with_link_attributes,
    );

    assert_shapes_with(
        "See image:{logo}[Logo] and image:{logo}[] beside a *bold* run.",
        with_link_attributes,
    );

    // An image whose *attribute list* has no `'src` slice — crossing an
    // expansion, and crossing an escaped special. The builder now parses the
    // bracket from its match string, which is the same haystack the string
    // replacer the recorder wraps reads its own `caps[2]` out of, so the two
    // constructions carry the same attributes and can be compared.
    assert_shapes_with(
        "See image:{logo}[{label},200] and image:x.png[a < b,role=hl] here.",
        with_link_attributes,
    );

    // The two link-family display-text attribute lists, likewise with no
    // `'src` slice of their own. Both constructions now read the same bytes
    // (the builder its match string, the recorder the string replacer's own
    // `link_text_for_attrlist`), so the two are comparable here for the first
    // time. The fixture crosses an *expansion* only: a display text crossing
    // an escaped special is a leaf-boundary artifact rather than a shape
    // difference — the recorder recovers a `CharRef` by splitting the rendered
    // text where the builder's parsed value is one run — which is the same
    // one-sided richness the sweep documents elsewhere.
    assert_shapes_with(
        "Read link:{url}[{label},role=hl] or https://{host}[{label},window=_blank] here.",
        with_link_attributes,
    );

    // A cross-reference whose reference text crosses a **rendered span**, in
    // both spellings: the builder now carries that text as structured children
    // (the span's own node cloned into them), which is the shape the recorder
    // recovers from the rendered markup, so the two constructions can finally
    // be compared for this form.
    assert_shapes("See xref:sec[the *bold* steps] and <<sec,a _slanted_ label>> here.");

    // The same text carrying an **attribute list**, whose parsed positional
    // value is those same children — tokened before the split and spliced back
    // as the span's own node.
    assert_shapes("See xref:sec[the *bold* steps,role=hl] here.");

    // The `link:`/`mailto:` macro's own version of that lift, the second family
    // to take it — comparable here for the same reason.
    assert_shapes("Read link:index.html[the *bold* docs] or mailto:a@example.org[write _now_].");

    // And the auto-link / formal-URL family's, the third — in the plain
    // spelling and in the ANGLE branch's `[…]` alternative, which keeps its
    // `&lt;`.
    assert_shapes("Visit https://example.org[the *bold* docs] now.");
    assert_shapes("Visit <https://example.org[an _angle_ label] now.");

    // A footnote spliced in by an attribute reference — the externalized
    // footnote idiom — and one whose id alone comes from an expansion. The
    // recorder has always recovered these (it reads the string pipeline's own
    // render params, and the pipeline expands the reference before matching);
    // the builder now reads the id from the expansion too, so the two
    // constructions' *structures* can be compared for this shape rather than a
    // node against one carrying the reference as its id.
    assert_shapes_with(
        "A bold statement about *{product}*!{fn-disclaimer} \
         Another outrageous statement.{fn-disclaimer}",
        || {
            Parser::default()
                .with_intrinsic_attribute("product", "Widget", ModificationContext::Anywhere)
                .with_intrinsic_attribute(
                    "fn-disclaimer",
                    "footnote:disclaimer[Opinions are my own.]",
                    ModificationContext::Anywhere,
                )
        },
    );

    assert_shapes_with(
        "A claim.footnote:{id}[a {product} note] and again.footnote:disc[]",
        || {
            Parser::default()
                .with_intrinsic_attribute("product", "Widget", ModificationContext::Anywhere)
                .with_intrinsic_attribute("id", "disc", ModificationContext::Anywhere)
        },
    );
}

#[test]
fn the_tree_codec_round_trips_every_kind_the_recorder_builds() {
    // The corpus drives the codec broadly but not exhaustively: several
    // spellings never appear in it (a bare `menu:` with no item, an XML-guarded
    // callout, `StyleVariant::SingleQuote`), and a recording has to be able to
    // hold whichever the recorder produces. The bar is
    // `decode(encode(x)) == x`, since that equality is what every assertion in
    // this module rests on once the recorder side is a lookup.
    let location = Span::new("x");

    let leaf = |value: &'static str| InlineNode::Text {
        value: CowStr::from(value),
        location,
    };

    let nodes = vec![
        // Every leaf kind, including each `CharRef` arm. `Replacement` is
        // drawn from `RECORDER_ENTITY_TABLE`, which is the only set the
        // decoder can rebuild a `&'static str` from — and the only set the
        // recorder can produce.
        leaf("plain text"),
        InlineNode::CharRef {
            value: CharRef::Special('<'),
            location,
        },
        InlineNode::CharRef {
            value: CharRef::Replacement("\u{2014}"),
            location,
        },
        InlineNode::CharRef {
            value: CharRef::Entity(CowStr::from("&hellip;")),
            location,
        },
        InlineNode::LineBreak { location },
        // Every `StyleVariant` and both `SpanForm`s, with an id, roles, and a
        // nested subtree — the nesting is what exercises the counted format's
        // one interesting property.
        InlineNode::Styled(Styled {
            variant: StyleVariant::SingleQuote,
            form: SpanForm::Unconstrained,
            id: Some(CowStr::from("the-id")),
            roles: vec![CowStr::from("a"), CowStr::from("b")],
            attrs: None,
            children: vec![
                leaf("nested"),
                InlineNode::Styled(Styled {
                    variant: StyleVariant::Mark,
                    form: SpanForm::Constrained,
                    id: None,
                    roles: vec![],
                    attrs: None,
                    children: vec![leaf("deeper")],
                    passthrough: None,
                    location,
                }),
            ],
            passthrough: None,
            location,
        }),
        InlineNode::Ref(Ref {
            variant: RefVariant::Xref,
            target: CowStr::from("tgt"),
            window: Some(CowStr::from("_blank")),
            roles: vec![CowStr::from("external")],
            children: vec![leaf("label")],
            resolved: None,
            derived: None,
            xrefstyle: None,
            attrs: None,
            link_form: None,
            location,
        }),
        InlineNode::Image(Image {
            is_icon: false,
            target: CowStr::from("x.png"),
            restored_target_ranges: vec![],
            alt: Some(CowStr::from("Alt")),
            width: Some(CowStr::from("10")),
            height: None,
            attrs: None,
            location,
        }),
        InlineNode::Footnote(Footnote {
            id: Some(CowStr::from("fid")),
            number: Some(CowStr::from("2")),
            is_reference: true,
            children: vec![leaf("note")],
            location,
        }),
        InlineNode::Anchor(Anchor {
            id: CowStr::from("anchor-id"),
            reftext: None,
            is_bibliography: false,
            location,
        }),
        // All three `UiKind`s, including a bare menu (no item) and a menu with
        // submenus — the corpus has the second but not the first.
        InlineNode::Ui(Ui {
            kind: UiKind::Keyboard(vec![CowStr::from("Ctrl"), CowStr::from("T")]),
            location,
        }),
        InlineNode::Ui(Ui {
            kind: UiKind::Button(CowStr::from("Save")),
            location,
        }),
        InlineNode::Ui(Ui {
            kind: UiKind::Menu {
                menu: CowStr::from("File"),
                submenus: vec![CowStr::from("Export")],
                item: Some(CowStr::from("PDF")),
            },
            location,
        }),
        InlineNode::Ui(Ui {
            kind: UiKind::Menu {
                menu: CowStr::from("File"),
                submenus: vec![],
                item: None,
            },
            location,
        }),
        InlineNode::IndexTerm(IndexTerm {
            terms: vec![CowStr::from("primary"), CowStr::from("secondary")],
            children: vec![],
            visible: true,
            location,
        }),
        // Both `CalloutGuard`s — the corpus never produces the XML one.
        InlineNode::Callout(Callout {
            number: CowStr::from("1"),
            guard: CalloutGuard::LineComment(CowStr::from("# ")),
            location,
        }),
        InlineNode::Callout(Callout {
            number: CowStr::from("2"),
            guard: CalloutGuard::Xml,
            location,
        }),
        InlineNode::Stem(Stem {
            notation: StemNotation::LatexMath,
            value: CowStr::from("x^2"),
            subs: SubstitutionGroup::Stem,
            source_text: None,
            children: vec![],
            location,
        }),
        // The bytes a line-based format has to survive, in a string position of
        // every shape the record holds: a plain field, a quoted `Option`, a
        // string-list item. A literal `-` in a *present* field is the one that
        // would read back as `None` if `push_option` wrote values bare.
        leaf("a\tb\nc \"d\" \\e"),
        InlineNode::Styled(Styled {
            variant: StyleVariant::Strong,
            form: SpanForm::Constrained,
            id: Some(CowStr::from("-")),
            roles: vec![CowStr::from("a\tb"), CowStr::from("-")],
            attrs: None,
            children: vec![],
            passthrough: None,
            location,
        }),
    ];

    for variant in [
        StyleVariant::Strong,
        StyleVariant::Emphasis,
        StyleVariant::Code,
        StyleVariant::Mark,
        StyleVariant::Superscript,
        StyleVariant::Subscript,
        StyleVariant::DoubleQuote,
        StyleVariant::SingleQuote,
        StyleVariant::Unquoted,
    ] {
        let one = vec![InlineNode::Styled(Styled {
            variant,
            form: SpanForm::Constrained,
            id: None,
            roles: vec![],
            attrs: None,
            children: vec![],
            passthrough: None,
            location,
        })];

        assert_eq!(
            decode_nodes(&encode_nodes(&one), location),
            one,
            "round trip for {variant:?}"
        );
    }

    let encoded = encode_nodes(&nodes);

    assert!(
        !encoded.contains('\n'),
        "a record must stay one physical line: {encoded:?}"
    );

    assert_eq!(decode_nodes(&encoded, location), nodes);

    // The empty tree, which encodes as a bare count rather than as nothing.
    assert_eq!(encode_nodes(&[]), "0");
    assert_eq!(decode_nodes("0", location), vec![]);
}

#[test]
fn the_tree_codec_rejects_a_corrupted_recording() {
    // A recording is hand-editable, so the codec's panics are a reachable
    // failure surface rather than defensive code. Each case below names a
    // distinct way one can go wrong.
    let location = Span::new("x");

    for (encoded, expected) in [
        // Nothing at all, where a top-level count is required.
        ("", "missing node"),
        // A count that is not a number.
        ("x", "bad node count"),
        // A count that over-reads its list.
        ("1", "missing node kind"),
        // A count that under-reads it, leaving fields behind.
        ("0\tText", "trailing fields"),
        // A node kind that does not exist.
        ("1\tBogus", "unrecognized node kind"),
        // Each small enum's own unknown spelling.
        ("1\tStyled\tBogus", "unrecognized style variant"),
        ("1\tStyled\tStrong\tBogus", "unrecognized span form"),
        ("1\tRef\tBogus", "unrecognized ref variant"),
        ("1\tStem\tBogus", "unrecognized stem notation"),
        ("1\tCharRef\tBogus", "unrecognized CharRef kind"),
        ("1\tUi\tBogus", "unrecognized Ui kind"),
        ("1\tCallout\t\"1\"\tBogus", "unrecognized Callout guard"),
        // A field that should be quoted and is not.
        ("1\tText\tbare", "unquoted field"),
        // A `bool` field that is neither.
        (
            "1\tFootnote\t-\t-\tmaybe\t0",
            "bad Footnote is_reference flag",
        ),
        // A `CharRef::Special` holding more than one character.
        ("1\tCharRef\tSpecial\t\"ab\"", "multi-character"),
        ("1\tCharRef\tSpecial\t\"\"", "empty CharRef::Special"),
        // A replacement the recorder could never have produced, so the decoder
        // has no `'static` value to rebuild it from.
        (
            "1\tCharRef\tReplacement\t\"\u{2764}\"",
            "RECORDER_ENTITY_TABLE has no entry for it",
        ),
    ] {
        let message = std::panic::catch_unwind(|| decode_nodes(encoded, location))
            .expect_err(&format!("{encoded:?} decoded without complaint"));

        let message = message
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| message.downcast_ref::<&str>().copied())
            .unwrap_or("");

        assert!(
            message.contains(expected),
            "{encoded:?} panicked with {message:?}, expected it to mention {expected:?}"
        );
    }
}
