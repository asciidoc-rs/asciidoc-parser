//! Builds the inline AST **directly from source in a single forward pass**.
//!
//! This is the first brick of "Strategy B" (design §4.1): rather than recover
//! the tree from a post-substitution *marked string* – the
//! [`inline_tree`](crate::content::inline_tree) recorder's "Strategy A" – each
//! substitution step is recast as a **transducer** over a node list,
//! `Vec<InlineNode<'src>> -> Vec<InlineNode<'src>>`, that refines the tree in
//! place. Two properties fall out that Strategy A cannot offer:
//!
//! 1. **Honest per-node spans.** A node is sliced straight from the source
//!    [`Span`], so its `location` reports the real `line`/`col`/`offset` of the
//!    construct (issue #944), instead of every node carrying the whole-content
//!    span.
//! 2. **`'src` borrowing by construction.** A verbatim text run's `value`
//!    borrows the very bytes its `location` covers, so the common case does not
//!    allocate.
//!
//! # Status
//!
//! Strategy B "touches every step," so it lands incrementally under the
//! golden-HTML oracle. This module currently implements only the **foundation**
//! plus the first refinement:
//!
//! - [`build`] seeds a single borrowed whole-source [`Text`](InlineNode::Text)
//!   node and threads it through the steps.
//! - [`apply_special_characters`] splits each `Text` run on `<`/`>`/`&` into
//!   precise-span [`Text`](InlineNode::Text) and
//!   [`CharRef`](InlineNode::CharRef) nodes.
//! - [`fold_html`] folds the resulting leaves back to output bytes through an
//!   [`InlineSubstitutionRenderer`] – the first fold over the *public*
//!   [`InlineNode`] tree (the recorder's [`fold_into`] folds an intermediate
//!   representation, not the public tree).
//!
//! It is **additive and non-regressing**: nothing here is wired into the parse
//! path yet, so the authoritative string pipeline and the Strategy-A
//! [`Content::inlines`](crate::content::Content::inlines) tree are untouched.
//! Later increments extend the transducer to quotes, replacements, macros,
//! attribute expansion, and passthroughs, at which point it can replace the
//! recorder, make `rendered_html()` a fold, and retire the sentinel systems.
//!
//! [`fold_into`]: crate::content::inline_tree
//! [`Text`]: InlineNode::Text

// The transducer framework is deliberately broader than the single step wired
// up so far; later Strategy-B increments consume the rest.
#![allow(dead_code)]

use crate::{
    Span,
    inlines::{CharRef, InlineNode},
    parser::{InlineSubstitutionRenderer, SpecialCharacter},
    strings::CowStr,
};

/// Builds the inline tree for `source` in a single forward pass.
///
/// The tree is seeded as one borrowed whole-source [`Text`](InlineNode::Text)
/// node and refined by each substitution step in turn. `source` is the exact
/// text to process, so a caller controls precisely what is built; reconciling
/// with a block's line filtering and joining is a later increment's concern.
pub(crate) fn build(source: Span<'_>) -> Vec<InlineNode<'_>> {
    let seed = vec![InlineNode::Text {
        value: CowStr::from(source.data()),
        location: source,
    }];

    apply_special_characters(seed)
}

/// Reports whether `c` is one of the three characters the special-characters
/// substitution acts on.
fn is_special(c: char) -> bool {
    matches!(c, '<' | '>' | '&')
}

/// The special-characters substitution, as a node transducer: every
/// [`Text`](InlineNode::Text) run is split on `<`/`>`/`&` into precise-span
/// [`Text`](InlineNode::Text) and [`CharRef`](InlineNode::CharRef) nodes, and
/// every other node passes through (recursing into parent nodes' children).
///
/// A `Text` node's `value` borrows its `location` at this stage, so splitting
/// by the `location` span is faithful to the logical text.
fn apply_special_characters<'src>(nodes: Vec<InlineNode<'src>>) -> Vec<InlineNode<'src>> {
    let mut out = Vec::with_capacity(nodes.len());

    for node in nodes {
        match node {
            InlineNode::Text { location, .. } => {
                split_special_characters(location, &mut out);
            }

            InlineNode::Styled(mut styled) => {
                styled.children = apply_special_characters(styled.children);
                out.push(InlineNode::Styled(styled));
            }

            InlineNode::Ref(mut reference) => {
                reference.children = apply_special_characters(reference.children);
                out.push(InlineNode::Ref(reference));
            }

            other => out.push(other),
        }
    }

    out
}

/// Splits the text covered by `location` into alternating
/// [`Text`](InlineNode::Text) runs and [`CharRef`](InlineNode::CharRef)
/// specials, pushing each onto `out`.
///
/// Every sub-span is sliced from `location` with the crate's span primitives,
/// so its `line`/`col`/`offset` stay honest; a run is never emitted empty.
fn split_special_characters<'src>(location: Span<'src>, out: &mut Vec<InlineNode<'src>>) {
    let mut rest = location;

    while let Some(pos) = rest.position(is_special) {
        // Emit the borrowed text run preceding the special, when non-empty.
        if pos > 0 {
            let text = rest.slice_to(..pos);

            out.push(InlineNode::Text {
                value: CowStr::from(text.data()),
                location: text,
            });
        }

        // The three specials are ASCII, so the match is exactly one byte wide.
        let ch_span = rest.slice(pos..pos + 1);

        let ch = ch_span.data().chars().next().unwrap_or('\u{FFFD}');

        out.push(InlineNode::CharRef {
            value: CharRef::Special(ch),
            location: ch_span,
        });

        rest = rest.slice_from(pos + 1..);
    }

    if !rest.data().is_empty() {
        out.push(InlineNode::Text {
            value: CowStr::from(rest.data()),
            location: rest,
        });
    }
}

/// Folds an inline node tree to output bytes through `renderer`.
///
/// This is the first fold over the *public* [`InlineNode`] tree. It currently
/// handles only the leaves the [`apply_special_characters`] step produces; a
/// later increment extends it as the transducer grows new node kinds.
pub(crate) fn fold_html(
    nodes: &[InlineNode<'_>],
    renderer: &dyn InlineSubstitutionRenderer,
) -> String {
    let mut out = String::new();
    fold_into_html(nodes, renderer, &mut out);
    out
}

/// Appends the fold of `nodes` to `out` (the recursive worker for
/// [`fold_html`]).
fn fold_into_html(
    nodes: &[InlineNode<'_>],
    renderer: &dyn InlineSubstitutionRenderer,
    out: &mut String,
) {
    for node in nodes {
        match node {
            InlineNode::Text { value, .. } => {
                // `Text` is logical (un-escaped) text; the fold escapes it. The
                // builder never leaves a special inside a `Text`, so this is
                // belt-and-suspenders, but it keeps the fold correct in its own
                // right.
                for ch in value.chars() {
                    render_char(ch, renderer, out);
                }
            }

            InlineNode::Raw { value, .. } => {
                out.push_str(value);
            }

            InlineNode::CharRef {
                value: CharRef::Special(ch),
                ..
            } => {
                render_char(*ch, renderer, out);
            }

            other => {
                // The `SpecialCharacters` step produces only `Text` and
                // `CharRef::Special` leaves, and this fold additionally emits the
                // design-legal `Raw` leaf; no other node kind reaches the fold in
                // this increment. A later increment fills in the arms above as
                // the transducer grows new kinds. Guard against a premature
                // caller in debug builds and emit nothing in release, mirroring
                // the safe defensive fallback in [`content`](super::content).
                debug_assert!(
                    false,
                    "inline_builder::fold_html reached an unsupported node kind: {other:?}"
                );
            }
        }
    }
}

/// Appends `ch` to `out`, routing the three special characters through
/// `renderer` (so a custom renderer's escaping is honored) and pushing any
/// other character verbatim.
fn render_char(ch: char, renderer: &dyn InlineSubstitutionRenderer, out: &mut String) {
    let type_ = match ch {
        '<' => SpecialCharacter::Lt,
        '>' => SpecialCharacter::Gt,
        '&' => SpecialCharacter::Ampersand,

        _ => {
            out.push(ch);
            return;
        }
    };

    renderer.render_special_character(type_, out);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use super::{apply_special_characters, build, fold_html};
    use crate::{
        Span,
        content::{Content, SubstitutionStep},
        inlines::{CharRef, InlineNode, Ref, RefVariant, SpanForm, StyleVariant, Styled},
        parser::HtmlSubstitutionRenderer,
        strings::CowStr,
    };

    /// Asserts that `node` is a [`Text`](InlineNode::Text) whose `value`
    /// borrows (does not allocate) and whose `location` selects `data` at
    /// `line`/`col`.
    fn assert_text(node: &InlineNode<'_>, data: &str, line: usize, col: usize) {
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

    /// Asserts that `node` is a special [`CharRef`](InlineNode::CharRef) for
    /// `ch`, located at `col` on line 1 with `offset`.
    fn assert_special(node: &InlineNode<'_>, ch: char, col: usize, offset: usize) {
        match node {
            InlineNode::CharRef {
                value: CharRef::Special(got),
                location,
            } => {
                assert_eq!(*got, ch);
                assert_eq!(location.data(), ch.to_string());
                assert_eq!(location.col(), col, "col for {ch:?}");
                assert_eq!(location.byte_offset(), offset, "offset for {ch:?}");
            }

            other => panic!("expected CharRef::Special({ch:?}), got {other:?}"),
        }
    }

    #[test]
    fn splits_text_and_specials_with_precise_spans() {
        let nodes = build(Span::new("a<b>c&d"));

        assert_eq!(nodes.len(), 7);
        assert_text(&nodes[0], "a", 1, 1);
        assert_special(&nodes[1], '<', 2, 1);
        assert_text(&nodes[2], "b", 1, 3);
        assert_special(&nodes[3], '>', 4, 3);
        assert_text(&nodes[4], "c", 1, 5);
        assert_special(&nodes[5], '&', 6, 5);
        assert_text(&nodes[6], "d", 1, 7);
    }

    #[test]
    fn all_specials_yield_only_char_refs() {
        let nodes = build(Span::new("<>&"));

        assert_eq!(nodes.len(), 3);
        assert_special(&nodes[0], '<', 1, 0);
        assert_special(&nodes[1], '>', 2, 1);
        assert_special(&nodes[2], '&', 3, 2);
    }

    #[test]
    fn adjacent_specials_produce_no_empty_runs() {
        let nodes = build(Span::new("<<"));

        assert_eq!(nodes.len(), 2);
        assert_special(&nodes[0], '<', 1, 0);
        assert_special(&nodes[1], '<', 2, 1);
    }

    #[test]
    fn plain_text_is_a_single_borrowed_node() {
        let nodes = build(Span::new("hello"));

        assert_eq!(nodes.len(), 1);
        assert_text(&nodes[0], "hello", 1, 1);
    }

    #[test]
    fn empty_source_yields_no_nodes() {
        assert!(build(Span::new("")).is_empty());
    }

    #[test]
    fn a_run_spanning_a_newline_tracks_line_and_col() {
        // The text run between the two specials includes the newline, so the
        // node after it is located on line 2.
        let nodes = build(Span::new("a<\nb>"));

        assert_eq!(nodes.len(), 4);
        assert_text(&nodes[0], "a", 1, 1);
        assert_special(&nodes[1], '<', 2, 1);

        // The middle run is "\nb": it starts right after `<` (line 1, col 3) and
        // carries into line 2.
        assert_text(&nodes[2], "\nb", 1, 3);

        // The closing `>` lands on line 2.
        match &nodes[3] {
            InlineNode::CharRef {
                value: CharRef::Special('>'),
                location,
            } => {
                assert_eq!(location.line(), 2);
                assert_eq!(location.col(), 2);
            }

            other => panic!("expected CharRef::Special('>'), got {other:?}"),
        }
    }

    #[test]
    fn special_characters_recurses_into_styled_children() {
        // A custom `subs` order can run quotes before special characters, so the
        // step must descend into a `Styled` span's children.
        let loc = Span::new("a<b");

        let styled = InlineNode::Styled(Styled {
            variant: StyleVariant::Strong,
            form: SpanForm::Constrained,
            id: None,
            roles: vec![],
            attrs: None,
            children: vec![InlineNode::Text {
                value: CowStr::from(loc.data()),
                location: loc,
            }],
            location: loc,
        });

        let out = apply_special_characters(vec![styled]);

        assert_eq!(out.len(), 1);

        match &out[0] {
            InlineNode::Styled(styled) => {
                assert_eq!(styled.children.len(), 3);
                assert_text(&styled.children[0], "a", 1, 1);
                assert_special(&styled.children[1], '<', 2, 1);
                assert_text(&styled.children[2], "b", 1, 3);
            }

            other => panic!("expected Styled, got {other:?}"),
        }
    }

    #[test]
    fn special_characters_recurses_into_ref_children() {
        // A reference's display text is likewise refined in place.
        let loc = Span::new("x&y");

        let reference = InlineNode::Ref(Ref {
            variant: RefVariant::Link,
            target: CowStr::from("https://example.com"),
            children: vec![InlineNode::Text {
                value: CowStr::from(loc.data()),
                location: loc,
            }],
            roles: vec![],
            window: None,
            resolved: None,
            location: loc,
        });

        let out = apply_special_characters(vec![reference]);

        assert_eq!(out.len(), 1);

        match &out[0] {
            InlineNode::Ref(reference) => {
                assert_eq!(reference.children.len(), 3);
                assert_text(&reference.children[0], "x", 1, 1);
                assert_special(&reference.children[1], '&', 2, 1);
                assert_text(&reference.children[2], "y", 1, 3);
            }

            other => panic!("expected Ref, got {other:?}"),
        }
    }

    #[test]
    fn special_characters_passes_other_nodes_through() {
        // A node kind the step does not split (here a line break) is forwarded
        // unchanged.
        let location = Span::new("");
        let out = apply_special_characters(vec![InlineNode::LineBreak { location }]);

        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], InlineNode::LineBreak { .. }));
    }

    #[test]
    fn fold_emits_raw_verbatim() {
        // A `Raw` leaf is emitted without HTML-escaping, unlike `Text`; its `<`,
        // `>`, and `&` pass straight through.
        let location = Span::new("<b>raw &amp;</b>");

        let raw = InlineNode::Raw {
            value: CowStr::from(location.data()),
            location,
        };

        assert_eq!(
            fold_html(&[raw], &HtmlSubstitutionRenderer {}),
            "<b>raw &amp;</b>"
        );
    }

    /// The string pipeline's special-characters output for `source`, used as
    /// the golden oracle: `Content::from` then the `SpecialCharacters`
    /// step.
    fn golden(source: &str) -> String {
        let parser = crate::Parser::default();
        let mut content = Content::from(Span::new(source));
        SubstitutionStep::SpecialCharacters.apply(&mut content, &parser, None);
        content.rendered_str().to_string()
    }

    #[test]
    fn fold_matches_the_string_pipeline_byte_for_byte() {
        // Special-characters-only fixtures: for these, folding the single-pass
        // tree reproduces the string pipeline's escaped output exactly.
        let fixtures = [
            "",
            "plain text",
            "a<b>c&d",
            "<>&",
            "<<",
            "&<>&",
            "less < and & more >",
            "trailing &",
            "multi\nline < with & specials >",
            "unicode π < ω &",
        ];

        let renderer = HtmlSubstitutionRenderer {};

        for fixture in fixtures {
            let folded = fold_html(&build(Span::new(fixture)), &renderer);

            assert_eq!(
                folded,
                golden(fixture),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }
}
