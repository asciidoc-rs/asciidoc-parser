//! A differential harness for [`Content::passthroughs`], which is now a **view
//! over the inline tree** rather than the extraction pass's own list.
//!
//! Design §5.2's survey named this API as one of the six things `run_pipeline`
//! still solely owned, and called it the one where *deleting* the API was as
//! live an option as building a tree-backed view. The view is the chosen path,
//! and this is the corpus that gates it: for every fixture, what the string
//! pipeline extracted and what the tree holds must be the same facts.
//!
//! Three increments were needed before the tree could answer at all, and each
//! one is visible in what this corpus asserts:
//!
//!   * [`RawOrigin::Passthrough`](crate::inlines::RawOrigin) carries the
//!     resolved group and the author's pre-substitution body, because
//!     [`RawForm`](crate::inlines::RawForm) — the fold's *two-valued* view — is
//!     too coarse: a `pass:c,q[…]` body folds `AsIs` exactly as a `+++…+++`
//!     body does, and its `value` is already-substituted where
//!     [`Passthrough::text`] returns the input.
//!   * [`Stem`](crate::inlines::Stem) carries the same pair, plus its body's
//!     own nodes, so a passthrough *embedded* in an expression is still
//!     reachable.
//!   * `Passthrough` itself was narrowed to the two facts it exposes, so the
//!     view is lossless rather than quietly supplying `None` for two fields
//!     only the restore pass reads.
//!
//! The **order** is the one deliberate difference, and
//! [`the_view_returns_document_order`] pins it from both ends.

use crate::{
    Parser, Span,
    content::{Content, Passthroughs, SubstitutionGroup},
};

/// One passthrough as either side describes it: the author's body, and the
/// group it is restored under.
type Record = (String, SubstitutionGroup);

/// What the **string pipeline** extracts, in extraction order.
///
/// Read from a throwaway [`Passthroughs::extract_from`] over the same source
/// rather than from the content under test, whose own list is the view now —
/// comparing that against itself would assert nothing.
fn golden(source: &str, parser: &Parser) -> Vec<Record> {
    let mut scratch = Content::from(Span::new(source));

    Passthroughs::extract_from(&mut scratch, parser)
        .observable()
        .iter()
        .map(|pt| (pt.text().to_string(), pt.subs().clone()))
        .collect()
}

/// What the **view** returns, in document order.
fn view(content: &Content<'_>) -> Vec<Record> {
    content
        .passthroughs()
        .iter()
        .map(|pt| (pt.text().to_string(), pt.subs().clone()))
        .collect()
}

/// Parses `source` under the normal substitutions and returns both sides.
fn both(source: &str, parser: &Parser) -> (Vec<Record>, Vec<Record>) {
    let mut content = Content::from(Span::new(source));
    SubstitutionGroup::Normal.apply(&mut content, parser, None);

    (golden(source, parser), view(&content))
}

/// The forms whose extraction entry and tree node correspond one-to-one.
const CORPUS: &[&str] = &[
    // `+++…+++` and a bare `pass:[…]` — group `None`, body verbatim.
    "a +++<b>raw</b>+++ x",
    "a pass:[bare<b>] x",
    "+++ leading +++ and +++ trailing +++",
    // `++…++` and `$$…$$` — group `Verbatim`.
    "a ++lit<b>++ x",
    "a $$dollar<b>$$ x",
    "a ++one++ and $$two$$ and ++three++",
    // An explicit substitution list: the form whose `value` is *not* the
    // author's body, and whose group no `RawForm` can express.
    "a pass:c,q[c and *q* <b>] x",
    "a pass:q[just *quotes*] x",
    "a pass:n[normal <b> subs] x",
    "a pass:[plain] and pass:c[<escaped>] together",
    // An escaped closing bracket, which unescapes before either side records
    // the body.
    r"a pass:[br\]acket] x",
    r"a pass:c,q[*q* br\]acket] x",
    // Several forms in one content, so the *order* is compared too.
    "+++A+++ then ++B++ then $$C$$ then pass:[D]",
    "++B++ and pass:c,q[*E*] and +++A+++",
    // An **attribute-list-prefixed** passthrough, whose body is a `Raw` inside
    // the `Styled` wrapper the extraction pass records — the two still agree,
    // because the group the wrapper resolves is the body's own.
    "a [.role]++attr++ x",
    "a [.role]+++raw+++ x",
    // Inline **STEM**, an implicit passthrough: the default group, both other
    // notations, a body the group changes, and the two explicit-list
    // spellings whose group is neither `Stem` nor `None`.
    "a stem:[x^2] x",
    "a stem:[p < q] x",
    "a asciimath:[c < d] x",
    "a latexmath:[e < f] x",
    "a stem:c,q[g < *h*] x",
    "a stem:n[i < j] x",
    // The `x-` **compatibility marker**, whose `++…++` body goes through the
    // normal substitutions as a subtree — the spelling that forced the record
    // onto the wrapper — beside the two spellings whose body is a `Raw` leaf.
    "a [x-]++attr++ x",
    "a [x-]+++raw+++ x",
    "a [x-]`tick` x",
    // Every form that records, in one content, so the order is compared over
    // the whole set rather than within one kind.
    "+++A+++ and stem:[B] and [x-]++C++ and ++D++",
    // Inside containers the walk has to descend into.
    "*bold with ++lit++ inside*",
    "link:x.html[text with ++lit++ in it]",
    "footnote:[a note with +++<b>++++ inside]",
];

#[test]
fn the_view_reports_the_same_facts_the_extraction_pass_does() {
    let parser = Parser::default();
    let mut seen = 0usize;

    for source in CORPUS {
        let (golden, mut view) = both(source, &parser);

        seen += view.len();

        // Compared as multisets: the *order* is deliberately different (a tree
        // walk is document order, the extraction pass is pass order), and
        // `the_view_returns_document_order` pins that difference on its own.
        // What this test is about is the facts — every entry the pass made, the
        // view reports, with the same body and the same group.
        // `SubstitutionGroup` is not `Ord`, so the key is the pair's own
        // rendering, which is exactly what the comparison below reads.
        let key = |(text, subs): &Record| (text.clone(), format!("{subs:?}"));

        let mut golden = golden;
        view.sort_by_key(key);
        golden.sort_by_key(key);

        assert_eq!(
            view, golden,
            "the view diverged from the extraction pass for {source:?}"
        );
    }

    // Guards against a corpus that stopped extracting anything, which would
    // otherwise compare empty against empty and report success.
    assert!(
        seen >= 25,
        "the corpus stopped exercising passthrough records: {seen}"
    );
}

#[test]
fn a_group_that_does_not_extract_reports_nothing() {
    // The gate moved, and the answer has to survive the move. Before the view,
    // a group that does not include the macros step simply never ran the
    // extraction pass, so there was nothing to retain; now the answer comes
    // from the tree not *holding* a passthrough node under such a group. Same
    // result, different reason — which is exactly the kind of substitution this
    // branch has twice shipped a hole in, so it is asserted rather than assumed.
    let parser = Parser::default();

    for group in [
        SubstitutionGroup::None,
        SubstitutionGroup::Verbatim,
        SubstitutionGroup::Stem,
    ] {
        for source in [
            "a ++lit++ and +++raw+++ x",
            "a pass:c,q[body] x",
            "a stem:[p < q] x",
            "a [x-]++attr++ x",
        ] {
            let mut content = Content::from(Span::new(source));
            group.apply(&mut content, &parser, None);

            assert!(
                content.passthroughs().is_empty(),
                "{group:?} reported {:?} for {source:?}",
                view(&content)
            );
        }
    }

    // And the two groups that *do* extract still report, so the loop above is
    // not passing because the fixtures stopped containing passthroughs.
    for group in [SubstitutionGroup::Normal, SubstitutionGroup::Header] {
        let mut content = Content::from(Span::new("a ++lit++ and +++raw+++ x"));
        group.apply(&mut content, &parser, None);

        assert_eq!(
            view(&content),
            [
                ("lit".to_string(), SubstitutionGroup::Verbatim),
                ("raw".to_string(), SubstitutionGroup::None),
            ],
            "{group:?}"
        );
    }
}

#[test]
fn the_view_returns_document_order() {
    // The order decision, pinned. The extraction pass pulls the bare `+…+` form
    // out in a second pass and STEM in a third, so a content mixing the forms
    // lists them in an order that has nothing to do with where the author wrote
    // them. The view walks the tree, which gives document order.
    //
    // This is a deliberate, documented difference rather than an accident, so it
    // is asserted from both ends: the view's order is exactly the source's, and
    // it is *not* the extraction pass's.
    let parser = Parser::default();

    for (source, expected) in [
        (
            "+++A+++ and stem:[B] and [x-]++C++ and ++D++",
            ["A", "B", "C", "D"].as_slice(),
        ),
        ("+bare+ then ++delim++", ["bare", "delim"].as_slice()),
        (
            "+b1+ and pass:[p] and +b2+ and ++d1++",
            ["b1", "p", "b2", "d1"].as_slice(),
        ),
    ] {
        let (golden, view) = both(source, &parser);

        let document: Vec<&str> = view.iter().map(|(text, _)| text.as_str()).collect();
        let extraction: Vec<String> = golden.into_iter().map(|(t, _)| t).collect();

        assert_eq!(document, expected, "document order for {source:?}");

        assert_ne!(
            document, extraction,
            "{source:?} no longer distinguishes the two orders; pick a fixture that does"
        );
    }
}

#[test]
fn a_marked_wrapper_is_one_entry_not_two() {
    // The invariant the wrapper marker creates, and the one the walk could get
    // wrong in a way no other test would catch. Two of the three
    // attribute-list-prefixed spellings put a `Raw` leaf *inside* the wrapper
    // carrying the same pair the wrapper does — so a walk that both read the
    // marker and descended into it would report each of them twice, while the
    // extraction pass records one entry.
    //
    // The third spelling (`[x-]++x++`) has no such leaf, which is why it cannot
    // be the only fixture here: it would pass either way.
    let parser = Parser::default();

    for source in [
        "a [.role]++dup++ x",
        "a [.role]+++dup+++ x",
        "a [x-]`dup` x",
        "a [x-]+++dup+++ x",
    ] {
        let (golden, view) = both(source, &parser);

        assert_eq!(
            view.len(),
            1,
            "{source:?} reported {} entries where the pass records 1: {view:?}",
            view.len()
        );

        assert_eq!(view, golden, "{source:?}");
    }
}

#[test]
fn a_stem_expression_embedding_a_passthrough_reports_both_entries() {
    // The shape that took the longest to close, and the one whose two sides
    // still disagree on a *body* while agreeing on the count.
    //
    // The pass records **two** entries: the inner passthrough, and the STEM
    // itself — whose own text keeps the `\u{96}0\u{97}` sentinel where that body
    // was lifted out. The view reports two as well, reaching the inner one
    // through `Stem::children`; but the STEM entry it reports holds the
    // **restored** body, because `stem_expression_value` splices each inner
    // body back in while computing the expression.
    //
    // Reporting the restored body is the decision, not an oversight: the
    // sentinel is an artifact of the extraction pass's own bookkeeping, and a
    // caller asking what the author wrote is better served by `x <b> y` than by
    // a private control character. The sentinel disappears entirely when step 6
    // deletes that pass.
    let parser = Parser::default();

    for (source, restored) in [
        ("a stem:[x +++<b>+++ y] z", "x <b> y"),
        ("a stem:[x $$lit$$ y] z", "x lit y"),
        ("a latexmath:[x ++e++ y] z", "x e y"),
    ] {
        let (golden, view) = both(source, &parser);

        assert_eq!(golden.len(), 2, "{source:?}");
        assert_eq!(view.len(), 2, "{source:?}");

        // Document order puts the STEM macro — which starts first — ahead of
        // the body embedded inside it, where the pass extracts the inner one
        // first and the STEM last.
        assert_eq!(view[0].1, SubstitutionGroup::Stem, "{source:?}");
        assert_eq!(view[0].0, restored, "{source:?}");

        // The inner entry is the one both sides describe identically.
        assert!(
            golden.contains(&view[1]),
            "{source:?}: the inner entry {:?} is not one the pass recorded: {golden:?}",
            view[1]
        );

        // The pass's own outer entry keeps the sentinel; the view's does not.
        assert!(golden[1].0.contains('\u{96}'), "{source:?}");
        assert!(!view[0].0.contains('\u{96}'), "{source:?}");
    }
}

#[test]
fn a_deferred_stem_macro_reports_only_its_inner_passthrough() {
    // The one shape still short of an entry, and a documented limitation rather
    // than a gap the view can close.
    //
    // Under an explicit **non-local** substitution list the expression is not
    // local to each run, so `build_stem_node` declines the macro outright (see
    // `subs_are_local`) and there is no `Stem` node at all — nothing to hold
    // `children`, and nothing to report the outer entry from. The view reports
    // only the inner passthrough where the pass reports both.
    //
    // Closing it means building the node anyway, which would risk a construct
    // spanning the boundary going unrecognized — a rendering regression traded
    // for a reporting one. It waits for the cutover that deletes the extraction
    // pass and with it the question.
    let parser = Parser::default();

    let (golden, view) = both("a stem:c,q[x +++<b>+++ y] z", &parser);

    assert_eq!(golden.len(), 2);
    assert_eq!(
        view,
        [("<b>".to_string(), SubstitutionGroup::None)],
        "the deferred STEM should leave only its inner passthrough reported"
    );
}
