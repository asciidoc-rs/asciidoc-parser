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

            InlineNode::Styled(styled) => derived(&styled.children, out),
            InlineNode::Ref(reference) => derived(&reference.children, out),
            InlineNode::Footnote(footnote) => derived(&footnote.children, out),
            InlineNode::IndexTerm(index_term) => derived(&index_term.children, out),

            _ => {}
        }
    }
}

/// The forms whose extraction entry and tree node correspond one-to-one.
///
/// The two that do not are deferred to the view's own increment and pinned by
/// [`the_two_forms_the_tree_records_nothing_for`] below.
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

        assert_eq!(
            tree,
            golden(&content),
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
fn the_two_forms_the_tree_records_nothing_for() {
    // Two forms are deliberately outside the corpus above, and in both the tree
    // records *nothing* where the extraction pass records an entry — so this is
    // the view's problem rather than the record's, since the fact this
    // increment adds lives on a `Raw` node and neither form builds one.
    let parser = Parser::default();

    let records = |source: &str| -> (Vec<Record>, Vec<Record>) {
        let mut content = Content::from(Span::new(source));
        SubstitutionGroup::Normal.apply(&mut content, &parser, None);

        let mut tree = vec![];
        derived(content.inlines(), &mut tree);

        (golden(&content), tree)
    };

    // The `x-` **compatibility marker** sends the body through the normal
    // substitutions as a node subtree rather than holding it as one opaque
    // `Raw`, which is why its entry's group is `Normal` — the one
    // attribute-list-prefixed spelling that differs from its siblings above.
    let (golden_records, tree) = records("a [x-]++attr++ x");

    assert_eq!(
        golden_records,
        [("attr".to_string(), SubstitutionGroup::Normal)]
    );

    assert!(
        tree.is_empty(),
        "the compat marker's body must not record a Raw: {tree:?}"
    );

    // An inline **STEM** body is a `Stem` node, not a `Raw` one.
    let (golden_records, tree) = records("a stem:[x^2] x");

    assert_eq!(
        golden_records,
        [("x^2".to_string(), SubstitutionGroup::Stem)]
    );

    assert!(
        tree.is_empty(),
        "a STEM body must not record a Raw: {tree:?}"
    );
}
