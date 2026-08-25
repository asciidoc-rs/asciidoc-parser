//! A differential harness for the passthrough facts a
//! [`Raw`](crate::inlines::InlineNode::Raw) node records.
//!
//! Design §5.2's survey named [`Content::passthroughs`] as one of the six
//! things `run_pipeline` still solely owns, and called it the one where
//! *deleting* the API was as live an option as building a tree-backed view.
//! The view is the chosen path, and this is its first prerequisite: the tree
//! did not hold enough to answer either of the two things a
//! [`Passthrough`](crate::content::Passthrough) exposes.
//!
//! [`RawForm`](crate::inlines::RawForm) is the fold's *two-valued* view — emit
//! or escape — and five of the seven passthrough forms are exactly recoverable
//! from it. The two that are not are what
//! [`RawOrigin::Passthrough`](crate::inlines::RawOrigin) now carries:
//!
//!   * **`subs`.** A `pass:c,q[…]` body folds `AsIs` exactly as a `+++…+++`
//!     body does, so the form cannot tell the two apart, and the group is what
//!     [`Passthrough::subs`](crate::content::Passthrough::subs) returns.
//!   * **`source_text`.** An arbitrary group needs the substitution pipeline,
//!     which a fold — taking a renderer and a
//!     [`RenderContext`](crate::parser::RenderContext), not a `Parser` — has no
//!     way to reach. So a `pass:c,q[…]` body is substituted at *build* time and
//!     `value` holds the result, where
//!     [`Passthrough::text`](crate::content::Passthrough::text) returns the
//!     input. Recording the input beside the result is what keeps the author's
//!     own bytes answerable from the tree.
//!
//! Nothing reads either field yet — the view itself is a later increment — so
//! this corpus is what pins them, by comparing every record the tree holds
//! against the entry the string pipeline extracted for the same source.

use crate::{
    Parser, Span,
    content::{Content, SubstitutionGroup},
    inlines::{InlineNode, RawOrigin},
};

/// One passthrough as either side describes it: the author's body, and the
/// group it is restored under.
type Record = (String, SubstitutionGroup);

/// What the string pipeline extracted, in extraction order.
fn golden(content: &Content<'_>) -> Vec<Record> {
    content
        .passthroughs()
        .iter()
        .map(|pt| (pt.text().to_string(), pt.subs().clone()))
        .collect()
}

/// What the tree records, in document order.
///
/// A [`Raw`](InlineNode::Raw) node of
/// [`Passthrough`](RawOrigin::Passthrough) origin is one entry; its body is
/// `source_text` where the build-time substitution moved `value` away from the
/// author's bytes, and `value` itself everywhere else.
fn derived(nodes: &[InlineNode<'_>], out: &mut Vec<Record>) {
    for node in nodes {
        match node {
            InlineNode::Raw {
                value,
                origin: RawOrigin::Passthrough { subs, source_text },
                ..
            } => {
                let text = source_text
                    .clone()
                    .unwrap_or_else(|| value.as_ref().to_string());

                out.push((text, subs.clone()));
            }

            // A STEM expression is an *implicit* passthrough: one entry, whose
            // body is `source_text` wherever the group changed it.
            InlineNode::Stem(stem) => {
                let text = stem
                    .source_text
                    .clone()
                    .unwrap_or_else(|| stem.value.as_ref().to_string());

                out.push((text, stem.subs.clone()));
            }

            // A **marked** span is an attribute-list-prefixed passthrough's
            // wrapper, and the wrapper is what the extraction pass records as
            // one entry — so it contributes its own record and the walk does
            // **not** descend. Descending would double-count the two spellings
            // whose body is also a `Raw` leaf carrying the same pair
            // (`[.role]++x++`, `` [x-]`x` ``); see
            // `a_marked_wrapper_is_one_entry_not_two`.
            InlineNode::Styled(styled) => match &styled.passthrough {
                Some(wrapper) => out.push((wrapper.text.clone(), wrapper.subs.clone())),
                None => derived(&styled.children, out),
            },
            InlineNode::Ref(reference) => derived(&reference.children, out),
            InlineNode::Footnote(footnote) => derived(&footnote.children, out),
            InlineNode::IndexTerm(index_term) => derived(&index_term.children, out),

            _ => {}
        }
    }
}

/// The forms whose extraction entry and tree node correspond one-to-one.
///
/// Every form now records; what differs is the *order*, which
/// [`the_view_returns_document_order`] pins on its own.
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
fn a_raw_node_records_the_passthrough_it_came_from() {
    let parser = Parser::default();
    let mut seen = 0usize;

    for source in CORPUS {
        let mut content = Content::from(Span::new(source));
        SubstitutionGroup::Normal.apply(&mut content, &parser, None);

        let mut tree = vec![];
        derived(content.inlines(), &mut tree);

        seen += tree.len();

        // Compared as multisets: the *order* is deliberately different now (a
        // tree walk is document order, the extraction pass is pass order), and
        // `the_view_returns_document_order` pins that difference on its own.
        // What this test is about is the facts — every entry the pass made, the
        // tree records, with the same body and the same group.
        // `SubstitutionGroup` is not `Ord`, so the key is the pair's own
        // rendering — which is exactly what the comparison below reads.
        let key = |(text, subs): &Record| (text.clone(), format!("{subs:?}"));

        let (mut tree, mut golden) = (tree, golden(&content));
        tree.sort_by_key(key);
        golden.sort_by_key(key);

        assert_eq!(
            tree, golden,
            "the tree's passthrough records diverged from the extraction pass for {source:?}"
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
fn the_view_returns_document_order() {
    // The order decision, pinned. `Content::passthroughs()` returns *extraction*
    // order, and the bare `+…+` form is pulled out in a second pass while STEM
    // is pulled out in its own — so a content mixing the forms lists them in an
    // order that has nothing to do with where the author wrote them. A tree walk
    // gives document order, which is what the view returns.
    //
    // This is a deliberate, documented difference rather than an accident, so it
    // is asserted from both ends: the tree's order is exactly the source's, and
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
        let mut content = Content::from(Span::new(source));
        SubstitutionGroup::Normal.apply(&mut content, &parser, None);

        let mut tree = vec![];
        derived(content.inlines(), &mut tree);

        let document: Vec<&str> = tree.iter().map(|(text, _)| text.as_str()).collect();
        let extraction: Vec<String> = golden(&content).into_iter().map(|(t, _)| t).collect();

        assert_eq!(document, expected, "document order for {source:?}");

        assert_ne!(
            document, extraction,
            "{source:?} no longer distinguishes the two orders; pick a fixture that does"
        );
    }
}

#[test]
fn a_marked_wrapper_is_one_entry_not_two() {
    // The invariant the wrapper marker creates, and the one a walk could get
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
        let mut content = Content::from(Span::new(source));
        SubstitutionGroup::Normal.apply(&mut content, &parser, None);

        let mut tree = vec![];
        derived(content.inlines(), &mut tree);

        assert_eq!(
            tree.len(),
            1,
            "{source:?} recorded {} entries where the pass records 1: {tree:?}",
            tree.len()
        );

        assert_eq!(tree, golden(&content), "{source:?}");
    }
}
