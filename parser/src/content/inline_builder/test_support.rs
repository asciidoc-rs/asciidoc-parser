//! Shared white-box test helpers used by two or more of this module's split
//! test files. A helper needed by only one destination file lives directly in
//! that file's own `#[cfg(test)] mod tests` instead; this module exists only
//! for the genuinely cross-file ones (see the top-level split's notes).

#![allow(clippy::indexing_slicing)]
#![allow(clippy::panic)]
#![allow(clippy::unwrap_used)]

use super::{build, quotes::apply_quotes, special_chars::apply_special_characters};
use crate::{
    HasSpan, Parser, Span,
    inlines::{CharRef, InlineNode, RawOrigin, Ref, RefVariant, SpanForm, StyleVariant},
    parser::HtmlInlineRenderer,
    strings::CowStr,
};

/// Folds the tree with the built-in HTML renderer and a default parser. The
/// parser is consulted only when the tree contains an
/// [`Image`](InlineNode::Image) node (for the document's safe mode,
/// `data-uri`, and `icons` attributes); tests that need a non-default
/// document call [`super::fold_html`] directly with their own parser.
pub(super) fn fold_html(nodes: &[InlineNode<'_>], renderer: &HtmlInlineRenderer) -> String {
    super::fold_html(nodes, renderer, &Parser::default().render_context())
}

/// Builds the single-pass tree for `source` with a default parser and no
/// block attribute list (the parser is consulted only for attributed-quote
/// attribute lists and the document-wide `hardbreaks-option` attribute).
pub(super) fn build_src(source: Span<'_>) -> Vec<InlineNode<'_>> {
    build(source, &Parser::default(), None)
}

/// Seeds the whole-source [`Text`](InlineNode::Text) node the transducer
/// steps refine, exactly as [`build`] does before running them.
pub(super) fn seed(source: Span<'_>) -> Vec<InlineNode<'_>> {
    vec![InlineNode::Text {
        value: CowStr::from(source.data()),
        location: source,
    }]
}

/// Builds the tree **through the special-characters step only**, so a test
/// can compare this partial state against the matching partial golden
/// recording (the full [`build`] runs later steps that would perturb it).
pub(super) fn build_through_special(source: Span<'_>) -> Vec<InlineNode<'_>> {
    apply_special_characters(seed(source))
}

/// Builds the tree **through the quotes step**, for the quotes-stage
/// differential (see [`build_through_special`]).
pub(super) fn build_through_quotes(source: Span<'_>) -> Vec<InlineNode<'_>> {
    apply_quotes(build_through_special(source), source, &Parser::default())
}

/// Asserts that `node` is a [`Text`](InlineNode::Text) whose `value`
/// borrows (does not allocate) and whose `location` selects `data` at
/// `line`/`col`.
pub(super) fn assert_text(node: &InlineNode<'_>, data: &str, line: usize, col: usize) {
    match node {
        InlineNode::Text { value, location } => {
            assert!(
                matches!(value, CowStr::Borrowed(_)),
                "text value should borrow from source, got {value:?}"
            );
            assert_eq!(value.as_ref(), data);
            assert_eq!(location.data(), data);
            assert_eq!(location.line(), line, "line for {data:?}");
            assert_eq!(location.col(), col, "col for {data:?}");
        }

        other => panic!("expected Text({data:?}), got {other:?}"),
    }
}

/// Asserts that `node` is a [`CharRef`](InlineNode::CharRef)`::Special` for
/// `ch` — an escaped special the macro families recover as its own child
/// rather than baking into a `Text` — and returns its `location` for further
/// inspection.
///
/// Deliberately written as a whole-node [`assert_eq!`] rather than as a
/// destructuring `let … else { panic!(…) }` or an `assert!` carrying a message:
/// both of those put a *failure-only* region on a line of its own — the
/// `panic!` arm, or the message argument of a wrapped macro call — which the
/// coverage report counts as an uncovered line at every call site (five of them
/// in `links.rs` alone). Every line here executes on the passing path, and
/// `assert_eq!` already prints both nodes when it fails.
pub(super) fn assert_special_char<'src>(node: &InlineNode<'src>, ch: char) -> Span<'src> {
    let location = node.span();

    let expected = InlineNode::CharRef {
        value: CharRef::Special(ch),
        location,
    };

    assert_eq!(*node, expected);

    location
}

/// Asserts that `node` is a [`CharRef`](InlineNode::CharRef)`::Entity` for
/// `entity` — a *restored* entity a macro family recovers as its own child
/// rather than baking into a `Text` the fold would escape a second time — and
/// returns its `location` for further inspection.
///
/// Written as a whole-node [`assert_eq!`] for the same coverage reason
/// [`assert_special_char`] is.
pub(super) fn assert_entity<'src>(node: &InlineNode<'src>, entity: &str) -> Span<'src> {
    let location = node.span();

    let expected = InlineNode::CharRef {
        value: CharRef::Entity(CowStr::from(entity.to_string())),
        location,
    };

    assert_eq!(*node, expected);

    location
}

/// Asserts that `node` is a [`Raw`](InlineNode::Raw) leaf whose **fold** is
/// `value`, returning its `location`.
///
/// The assertion is on the folded bytes rather than on the node's `value`
/// field, because a `Raw` node carries one of two
/// [`form`](crate::inlines::RawForm)s: `AsIs`, whose value already
/// *is* those bytes, and `Escaped`, whose value is the author's logical text
/// that the fold escapes. What every caller here means is "this passthrough
/// contributes these bytes", and that is the same question for both forms —
/// where reading the field is only the same question for one of them.
pub(super) fn assert_raw<'src>(node: &InlineNode<'src>, value: &str) -> Span<'src> {
    match node {
        InlineNode::Raw { location, .. } => {
            assert_eq!(
                fold_html(std::slice::from_ref(node), &HtmlInlineRenderer {}),
                value
            );

            *location
        }

        other => panic!("expected Raw({value:?}), got {other:?}"),
    }
}

/// Asserts that `node` is a [`Raw`](InlineNode::Raw) leaf of exactly `form`,
/// carrying exactly `value` — the field-level assertion
/// [`assert_raw`] deliberately does not make.
///
/// Used where the *shape* is the subject: that a `+++…+++` body is `AsIs`
/// output while a `++…++` body is `Escaped` logical text, which is what keeps
/// a passthrough's escaping a property of the fold's renderer rather than of
/// the parse's — and that both are
/// [`Passthrough`](RawOrigin::Passthrough)-origin, unlike the `Raw` leaves a
/// substitution leaves behind in place.
/// A [`Passthrough`](RawOrigin::Passthrough) origin carrying `subs` and no
/// `source_text` — the shape every form but `pass:c,q[…]` takes.
///
/// Spelled out at each call site rather than defaulted, so a test states the
/// group it expects instead of letting it be inferred from the
/// [`RawForm`](crate::inlines::RawForm) beside it. The two deliberately
/// disagree for the bare `+…+` form, which is `Verbatim` but folds `AsIs`.
pub(super) fn passthrough(subs: crate::content::SubstitutionGroup) -> RawOrigin {
    RawOrigin::Passthrough {
        subs,
        source_text: None,
    }
}

pub(super) fn assert_raw_form(
    node: &InlineNode<'_>,
    form: crate::inlines::RawForm,
    origin: RawOrigin,
    value: &str,
) {
    // Compared as a whole node rather than by matching out the two fields,
    // which keeps this free of a fallback arm only a failing test could reach.
    // `location` is taken from `node` itself, so it is deliberately not part of
    // the assertion — this helper's subject is the form and the value, and
    // [`assert_raw`] already hands a caller the location to check.
    assert_eq!(
        node,
        &InlineNode::Raw {
            value: CowStr::from(value),
            form,

            origin,
            location: node.span(),
        }
    );
}

/// Asserts that `node` is a [`Styled`](crate::inlines::Styled) span of
/// `variant`/`form` with `children` children, and returns those children for
/// further inspection.
pub(super) fn assert_styled<'a, 'src>(
    node: &'a InlineNode<'src>,
    variant: StyleVariant,
    form: SpanForm,
) -> &'a [InlineNode<'src>] {
    match node {
        InlineNode::Styled(styled) => {
            assert_eq!(styled.variant, variant, "variant");
            assert_eq!(styled.form, form, "form");
            &styled.children
        }

        other => panic!("expected Styled({variant:?}), got {other:?}"),
    }
}

/// The frozen recording (see `parser/snapshots/README.md`) through the
/// **macros** step for `source`: the five steps [`build`] runs, in order,
/// with attribute references skipped, frozen into `snapshots/macros.txt`.
///
/// The `_parser` no longer participates — a recording is keyed by source alone
/// — but the parameter stays so the several dozen call sites that configured
/// one do not churn; where a parser made a shared source render differently,
/// the fixture already lives in its own named corpus (see
/// [`golden_macros_in`]).
pub(super) fn golden_macros_with(source: &str, _parser: &Parser) -> String {
    golden_macros_in("macros", source, _parser)
}

/// [`golden_macros_with`], reading a named corpus.
///
/// One corpus is keyed by source alone, so two fixtures sharing a source but
/// not a rendering need separate corpora. The handful of tests whose parser
/// made a shared source render differently name their own corpus here;
/// everything else takes the default one.
pub(super) fn golden_macros_in(corpus: &str, source: &str, _parser: &Parser) -> String {
    super::snapshot::recorded(corpus, source)
}

/// [`golden_macros_with`] with a default parser.
pub(super) fn golden_macros(source: &str) -> String {
    golden_macros_with(source, &Parser::default())
}

/// Asserts that `node` is a link [`Ref`], returning it for further
/// inspection.
pub(super) fn assert_link<'a, 'src>(node: &'a InlineNode<'src>) -> &'a Ref<'src> {
    match node {
        InlineNode::Ref(reference) if reference.variant == RefVariant::Link => reference,

        other => panic!("expected a link Ref, got {other:?}"),
    }
}

/// The concatenated value of a link node's [`Text`](InlineNode::Text)
/// display children (the reconstructed `link_text`).
pub(super) fn link_text_of(reference: &Ref<'_>) -> String {
    let mut s = String::new();

    for child in &reference.children {
        if let InlineNode::Text { value, .. } = child {
            s.push_str(value);
        }
    }

    s
}

/// The frozen recording (see `parser/snapshots/README.md`) for `source`, for
/// both the passthrough and STEM families (their steps shared one extraction
/// pass): extraction, the five steps [`build`] runs, then the restore, read
/// back from `snapshots/passthroughs.txt`. The `_parser` stays for the same
/// non-churn reason [`golden_macros_with`]'s does.
pub(super) fn golden_passthroughs_with(source: &str, _parser: &Parser) -> String {
    golden_passthroughs_in("passthroughs", source, _parser)
}

/// [`golden_passthroughs_with`], reading a named corpus (see
/// [`golden_macros_in`] for why a caller would name one).
pub(super) fn golden_passthroughs_in(corpus: &str, source: &str, _parser: &Parser) -> String {
    super::snapshot::recorded(corpus, source)
}

/// [`golden_passthroughs_with`] with a default parser.
pub(super) fn golden_passthroughs(source: &str) -> String {
    golden_passthroughs_with(source, &Parser::default())
}
