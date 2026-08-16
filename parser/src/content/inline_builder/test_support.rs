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
    content::{Content, Passthroughs, SubstitutionStep},
    inlines::{CharRef, InlineNode, Ref, RefVariant, SpanForm, StyleVariant},
    parser::HtmlSubstitutionRenderer,
    strings::CowStr,
};

/// Folds the tree with the built-in HTML renderer and a default parser. The
/// parser is consulted only when the tree contains an
/// [`Image`](InlineNode::Image) node (for the document's safe mode,
/// `data-uri`, and `icons` attributes); tests that need a non-default
/// document call [`super::fold_html`] directly with their own parser.
pub(super) fn fold_html(nodes: &[InlineNode<'_>], renderer: &HtmlSubstitutionRenderer) -> String {
    super::fold_html(nodes, renderer, &Parser::default())
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

/// Builds the tree **through the special-characters step only**, so a
/// staged differential test compares against the matching partial golden
/// (the full [`build`] runs later steps that would perturb it).
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

/// Asserts that `node` is a [`Raw`](InlineNode::Raw) with the given
/// `value`, returning its `location`.
pub(super) fn assert_raw<'src>(node: &InlineNode<'src>, value: &str) -> Span<'src> {
    match node {
        InlineNode::Raw {
            value: got,
            location,
        } => {
            assert_eq!(got.as_ref(), value);
            *location
        }

        other => panic!("expected Raw({value:?}), got {other:?}"),
    }
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

/// The string pipeline's output through the **macros** step for `source`,
/// used as the golden oracle: the five steps [`build`] runs, in order
/// (special characters, quotes, character replacements, macros, post
/// replacement), with `parser` as the document context. Attribute
/// references are skipped — exactly as the additive builder skips them — so
/// the fixtures deliberately contain none.
pub(super) fn golden_macros_with(source: &str, parser: &Parser) -> String {
    let mut content = Content::from(Span::new(source));
    SubstitutionStep::SpecialCharacters.apply(&mut content, parser, None);
    SubstitutionStep::Quotes.apply(&mut content, parser, None);
    SubstitutionStep::CharacterReplacements.apply(&mut content, parser, None);
    SubstitutionStep::Macros.apply(&mut content, parser, None);
    SubstitutionStep::PostReplacement.apply(&mut content, parser, None);
    content.rendered_str().to_string()
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

/// The string pipeline's output for `source`, used as the golden oracle for
/// both the passthrough and STEM increments (their steps share one
/// extraction pass, [`Passthroughs::extract_from`]): extract passthroughs
/// (including inline STEM macros), run the five steps [`build`] runs
/// (special characters, quotes, character replacements, macros, post
/// replacement), then restore them — exactly what
/// [`SubstitutionGroup::apply`](crate::content::SubstitutionGroup::apply)'s
/// `run_pipeline` does for [`SubstitutionGroup::Normal`]. Attribute
/// references are skipped, as elsewhere in this module's golden helpers.
pub(super) fn golden_passthroughs_with(source: &str, parser: &Parser) -> String {
    let mut content = Content::from(Span::new(source));
    let passthroughs = Passthroughs::extract_from(&mut content, parser);

    SubstitutionStep::SpecialCharacters.apply(&mut content, parser, None);
    SubstitutionStep::Quotes.apply(&mut content, parser, None);
    SubstitutionStep::CharacterReplacements.apply(&mut content, parser, None);
    SubstitutionStep::Macros.apply(&mut content, parser, None);
    SubstitutionStep::PostReplacement.apply(&mut content, parser, None);

    passthroughs.restore_to(&mut content, parser);
    content.rendered_str().to_string()
}

/// [`golden_passthroughs_with`] with a default parser.
pub(super) fn golden_passthroughs(source: &str) -> String {
    golden_passthroughs_with(source, &Parser::default())
}
