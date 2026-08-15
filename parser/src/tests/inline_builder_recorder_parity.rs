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
//!
//! Several more differences are structural rather than field-level, so they
//! are handled by the comparator itself instead of being excluded outright.
//! The common thread through all of them: the recorder can only recover a
//! leaf's structure from already-*rendered* output, while the builder's
//! leaves are drawn from *source*, so the two can legitimately draw leaf
//! boundaries differently even when the underlying content is identical.
//! [`consume_rendered_prefix`] is the one mechanism behind every case below –
//! see its own doc comment for how it works:
//!
//! - A handful of character-replacement types combine more than one output
//!   character into a single logical value (an em dash surrounded by spaces –
//!   [`CharacterReplacementType::EmDashSurroundedBySpaces`](crate::parser::CharacterReplacementType)
//!   – renders as *thin space, em dash, thin space*, three characters from one
//!   source match). The builder recognizes the whole match as **one**
//!   [`CharRef::Replacement`](crate::inlines::CharRef::Replacement) leaf
//!   carrying the combined value; the recorder sees three separate numeric-
//!   entity references in the output string and recovers **three** adjacent
//!   leaves, one per entity. Both fold to the same HTML.
//! - Adjacent `Text` node boundaries can legitimately differ: the builder gives
//!   an attribute-expanded run its own node even when it is plain text with no
//!   escaping of its own (design §3.4.1), so it sits as a distinct sibling
//!   beside the literal text around it, while the recorder – with no marker
//!   around plain text – recovers the whole run as one `Text` node.
//! - A source `&amp;`/`&lt;`/`&gt;`/`&#8217;`/… that the author wrote out
//!   literally passes through Asciidoctor's `specialcharacters` step unchanged
//!   (it is already a valid entity), rendering byte-identical to whatever
//!   *live* substitution – a special-character escape, or a typographic
//!   replacement that happens to render as the same entity – produces the same
//!   output. From already-rendered output alone the recorder cannot tell "the
//!   author wrote this entity" ([`CharRef::Entity`], the builder's
//!   classification) apart from the live classification that coincides with it
//!   – exactly the same set of entities [`classify_entity`] (the recorder's own
//!   recovery table) hard-codes, reproduced in [`RECORDER_ENTITY_TABLE`] and
//!   kept in lockstep with it by
//!   [`recorder_entity_table_matches_production_classify_entity`].
//! - A passthrough's content becomes its own [`Raw`](InlineNode::Raw) leaf in
//!   the builder's tree – but a passthrough's *restore* is a direct string
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
//!   `[target]` form – never the author's display text – so the recorder's
//!   `children` for such a node is legitimately empty even when the builder's
//!   is not. This corpus builds both trees with no resolution pass, so every
//!   cross-reference is in exactly this state; a resolved reference (out of
//!   scope here) would not have this gap, since resolution mirrors the
//!   *destination*, not the text, onto a `children` list the builder already
//!   populated correctly.
//! - A footnote **reference** occurrence's own `id` (`footnote:disc[]`) never
//!   reaches the renderer's params at all once it *resolves* to a number – the
//!   fold renders just the number, dropping `id` – so the recorder cannot
//!   recover it, while the builder still carries it as a structural fact (see
//!   [`assert_node_equivalent`]'s own `Footnote` arm).
//! - A [`Link`](RefVariant::Link)'s `roles`/`window` fields are not populated
//!   from its own attribute-list display text (`Ref::attrs`'s own doc comment)
//!   – the fold reads those straight off `attrs` instead – so they are skipped
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
        inline_builder::build,
        inline_tree::{
            CharRefKind, RecordingRenderer, attach_footnote_subtrees, build_inline_tree,
            classify_entity,
        },
    },
    inlines::{CharRef, InlineNode, RefVariant},
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
/// directly – a [`RecordingRenderer`] wrapped around the built-in HTML
/// renderer, the real pipeline run over it, the recorded markers parsed into
/// a tree, and each defining footnote's subtree attached from the footnote
/// texts that pass registered – exactly what `SubstitutionGroup::apply` did
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
    SubstitutionGroup::Normal.apply(&mut content, &recorder_parser, None);

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
        // recorder's *rendered* bytes at this position – see
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
        // exception that legitimately spans both kinds – its content still
        // passes through `specialcharacters`, so the recorder can recover
        // it as a mix of plain text and entities (design §3.4.1).
        let target: Option<(Cow<'_, str>, LeafKinds)> = match b {
            InlineNode::Text { value, .. } => {
                Some((Cow::Borrowed(value.as_ref()), LeafKinds::TextOnly))
            }
            InlineNode::Raw { value, .. } => {
                Some((Cow::Borrowed(value.as_ref()), LeafKinds::Mixed))
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
/// a shared definition (the two functions map in opposite directions – entity
/// to kind there, kind to entity here – over different types, the recorder's
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
/// [`CharRef::Replacement`] (an em dash surrounded by spaces, an ellipsis –
/// see the module doc comment) has no single table entry of its own, since
/// the recorder recovers it as several adjacent leaves, one per character;
/// it is rendered by looking up each character individually and
/// concatenating, mirroring that recovery exactly.
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
/// given builder target – see that function's own doc comment, and
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
/// own doc comment documents: if a prefix of `recorder`'s front nodes –
/// each [`Text`](InlineNode::Text) leaf contributing its value verbatim,
/// each [`CharRef`](InlineNode::CharRef) leaf contributing its own
/// *rendered* bytes ([`char_ref_rendered`]) – concatenates to exactly
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
            // recorder's `children` is empty *and* the builder's is not –
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
            // renderer's own params at all – both `fold_footnote` and the
            // string pipeline's own replacer render just the resolved
            // number, dropping `id` (see `fold_footnote` in
            // `inline_builder/fold.rs`) – so the recorder has nothing to
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
            assert_eq!(ri.terms, bi.terms, "IndexTerm terms differ for {source:?}");
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
        "an image:photo.png[Alt Text] inline",
        "image:pic.png[Scaled,200,100]",
        "see <<target>> for more",
        "see <<target,the target>> now",
        "xref:other.adoc#frag[Other] doc",
        "A claim.footnote:[the evidence]",
        "Named.footnote:disc[a discussion] then footnote:disc[].",
        "A claim.footnote:[the *strong* evidence and a link:https://e.org[source]]",
        "Two notes.footnote:[first] and again.footnote:[second]",
        "Press kbd:[Ctrl+T] now.",
        "Press kbd:[Ctrl,Shift,N] now.",
        "Click btn:[Save] please.",
        "Choose menu:File[Save As > PDF].",
        "A flow ((visible term)) here.",
        "A concealed (((primary,secondary))) term.",
        "[[the-anchor]]Anchored paragraph.",
        "first line +\nsecond line",
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
        "[[mid-anchor]] after the anchor",
        "text before anchor:named[Ref Text] and after",
        "a +++<b>raw</b>+++ passthrough",
        "inline pass:[<i>x</i>] macro",
        "math $$a < b$$ here",
        "a +literal *stars*+ b",
        "line one +\nline two +\nline three",
        "stem:[a < b] expression",
        "asciimath:[a < b] inline",
        "latexmath:[x < y] inline",
        "an icon:home[] icon",
        "icon:star[2x,role=gold] rated",
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

    assert_shapes(
        "Visit https://example.org[the site] or mailto:a@example.org[email us], \
         then see <<conclusion,the conclusion>>.",
    );

    assert_shapes(
        "Visit https://example.org or link:docs.html[the docs], \
         or just write to doc@example.org.",
    );

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
}
