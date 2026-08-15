//! UI macro recognition (`kbd:[…]`, `btn:[…]`, `menu:…[…]`).

use super::{
    MacroMatch, MacroMatchKind, image::range_is_verbatim_or_synthesized, rebuild_macro_level,
};
use crate::{
    Span,
    content::{
        INLINE_KBD_BTN_MACRO, INLINE_MENU_MACRO,
        inline_builder::quotes::{Piece, build_match_string, source_slice, text_slice},
        normalize_index_text, split_kbd_keys,
    },
    inlines::{InlineNode, Ui, UiKind},
    strings::CowStr,
};

/// The delimiter a menu macro's item list is split on: the *escaped* form of
/// the source `>` submenu caret, which is what the string replacer sees (the
/// special-characters step runs long before macros) and therefore what this
/// module's own match string presents too – as an atomic
/// [`CharRef`](InlineNode::CharRef) piece.
const SUBMENU_DELIMITER: &str = "&gt;";

/// The keyboard/button UI macro pass at a level: matches
/// [`INLINE_KBD_BTN_MACRO`] over the level's escaped text and replaces each
/// verbatim match with the [`Ui`](InlineNode::Ui) node it produces.
///
/// The caller runs this only under the `experimental` document attribute (see
/// [`apply_macros`](super::apply_macros)); a cheap prefilter still skips the
/// pattern sweep when no `kbd:`/`btn:` prefix with a `:[` is present, mirroring
/// the string step's `found_macroish_short` guard.
pub(super) fn kbd_btn_macros_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes);

    if !(s.contains(":[") && (s.contains("kbd:") || s.contains("btn:"))) {
        return nodes;
    }

    let matches = find_kbd_btn_matches(&s, &pieces, root);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every keyboard/button macro at this level, skipping any whose match
/// crosses an atomic piece (see [`apply_macros`](super::apply_macros)).
fn find_kbd_btn_matches<'src>(
    s: &str,
    pieces: &[Piece],
    root: Span<'src>,
) -> Vec<MacroMatch<'src>> {
    let mut matches = Vec::new();

    for caps in INLINE_KBD_BTN_MACRO.captures_iter(s) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();

        let full = whole.start()..whole.end();

        // A match crossing an escaped special or a rendered span is left for a
        // later increment: its bracket content would have to carry that
        // escaped/rendered text, which the node cannot hold.
        //
        // A [`synthesized`](Piece::synthesized) run (an attribute expansion,
        // or – reached at a tree's root – a filtered multi-line block's own
        // joined seed) *is* admitted: this family never slices `'src` for a
        // value at all – its keys and label come straight from the match
        // string, which carries a synthesized run's bytes exactly – so only
        // the node's `location` takes design §4.4's coarse fallback. The same
        // lift the anchor and bare-e-mail families already made, for the same
        // reason (see [`build_kbd_btn_node`]).
        if !range_is_verbatim_or_synthesized(pieces, &full) {
            continue;
        }

        // Group 1 is the (optional) escape backslash.
        if caps.get(1).is_some() {
            matches.push(MacroMatch {
                kind: MacroMatchKind::Unescape {
                    backslash: full.start,
                },
                full,
            });

            continue;
        }

        let node = build_kbd_btn_node(&caps, &full, pieces, root);

        matches.push(MacroMatch {
            kind: MacroMatchKind::Node {
                consumed: full.clone(),
                node: Box::new(node),
            },
            full,
        });
    }

    matches
}

/// Builds one [`Ui`](InlineNode::Ui) node from a keyboard/button match,
/// splitting the keys / normalizing the label exactly as the string replacer
/// does so the fold reproduces the same bytes.
///
/// Every value it computes comes from the **match string**, never from an
/// `'src` slice: on a verbatim match those bytes *are* the source text, and on
/// a [`synthesized`](Piece::synthesized) one they are the expanded value the
/// string pipeline itself matched over – which is precisely what lets this
/// family recognize a macro inside an expanded attribute value where a family
/// carrying an [`Attrlist`](crate::attributes::Attrlist)`<'src>` cannot. Only
/// the node's `location` falls back to the enclosing run's coarse span
/// (design §4.4) in the synthesized case.
fn build_kbd_btn_node<'src>(
    caps: &regex::Captures<'_>,
    full: &std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
) -> InlineNode<'src> {
    let location = source_slice(pieces, full.clone(), root);

    // Group 2 is the macro name; group 3 the bracketed content. Both always
    // participate in a non-escaped match.
    let content = caps.get(3).map_or("", |m| m.as_str());

    let kind = if caps.get(2).map(|m| m.as_str()) == Some("kbd") {
        let keys = split_kbd_keys(content)
            .into_iter()
            .map(CowStr::from)
            .collect();

        UiKind::Keyboard(keys)
    } else {
        UiKind::Button(CowStr::from(normalize_index_text(content, true)))
    };

    InlineNode::Ui(Ui { kind, location })
}

/// The menu UI macro pass at a level: matches [`INLINE_MENU_MACRO`] over the
/// level's escaped text and replaces each recognized match with the
/// [`Ui`](InlineNode::Ui) node it produces.
///
/// The caller runs this only under the `experimental` document attribute (see
/// [`apply_macros`](super::apply_macros)); a cheap prefilter still skips the
/// pattern sweep when no `menu:` prefix with an opening bracket is present.
///
/// All three item-list spellings are handled: the bare/single-item form, the
/// comma-delimited form, and the `&gt;`-submenu form
/// (`menu:View[Zoom > Reset]`), whose delimiters are escaped
/// [`CharRef`](InlineNode::CharRef)s by the time macros run and so need the
/// relaxed gate [`menu_match_is_sliceable`] applies (see its own doc comment).
/// A name or item text crossing any *other* escaped special, or a rendered
/// [`Styled`](crate::inlines::Styled) span, is still deferred – the verbatim
/// boundary every macro family documents. A
/// [`synthesized`](Piece::synthesized) run is **not** deferred: see
/// [`menu_match_is_sliceable`].
pub(super) fn menu_macros_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes);

    if !(s.contains("menu:") && s.contains('[')) {
        return nodes;
    }

    let matches = find_menu_matches(&nodes, &s, &pieces, root);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every menu macro at this level, skipping any whose name or item text
/// cannot be recovered (see [`menu_match_is_sliceable`], which
/// [`build_menu_node`] applies once the match's own capture groups are
/// resolved).
fn find_menu_matches<'src>(
    nodes: &[InlineNode<'src>],
    s: &str,
    pieces: &[Piece],
    root: Span<'src>,
) -> Vec<MacroMatch<'src>> {
    let mut matches = Vec::new();

    for caps in INLINE_MENU_MACRO.captures_iter(s) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();

        let full = whole.start()..whole.end();

        // The menu pattern's escape is an uncaptured leading `\?`. It is
        // checked *before* the sliceability gate – and needs no gate of its
        // own – because dropping the backslash keeps the rest of the match as
        // its own original nodes (an escaped special or a rendered span among
        // them), which fold back to exactly the bytes the string replacer's
        // `caps[0][1..]` emits. Mirrors the same hoist the `footnoteref:`
        // increment made for the identical reason.
        if whole.as_str().starts_with('\\') {
            matches.push(MacroMatch {
                kind: MacroMatchKind::Unescape {
                    backslash: full.start,
                },
                full,
            });

            continue;
        }

        let Some(node) = build_menu_node(&caps, &full, s, pieces, root, nodes) else {
            // A name or item text this increment cannot slice from `'src`:
            // left unrecognized, so the surrounding gap reproduces the source
            // unchanged (see [`menu_macros_level`]).
            continue;
        };

        matches.push(MacroMatch {
            kind: MacroMatchKind::Node {
                consumed: full.clone(),
                node: Box::new(node),
            },
            full,
        });
    }

    matches
}

/// Reports whether a menu match's own text can be recovered, the menu
/// family's counterpart of
/// [`range_is_verbatim_or_synthesized`].
///
/// It differs from that check in exactly one admitted case: a
/// [`SUBMENU_DELIMITER`] (`&gt;`) *inside the item list*. Every other macro
/// family requires its whole match to be verbatim before it will build a
/// self-describing node, and an escaped special is an atomic piece that check
/// rejects outright – which is what made the `&gt;`-submenu form
/// (`menu:View[Zoom > Reset]`) unrecognizable, whatever its item texts looked
/// like. But a submenu caret carries no value the node ever slices: like the
/// `<<id>>` shorthand's own `&lt;&lt;`/`&gt;&gt;` delimiters, the string
/// replacer *consumes* it as the list's delimiter and emits it nowhere (the
/// rendered caret between levels comes from `render_menu`, not from the source
/// character). Only the item texts on either side of it need to be verbatim,
/// and they are checked here exactly as before.
///
/// Everything else is unchanged: an atomic piece that is *not* an item-list
/// caret – another escaped special (`menu:File[A & B]`, and a `&`/`>` in the
/// menu *name*, which the pattern admits) or a rendered
/// [`Styled`](crate::inlines::Styled) span – still fails.
///
/// A [`synthesized`](Piece::synthesized) run (an attribute expansion, or –
/// reached at a tree's root – a filtered multi-line block's own joined seed)
/// is admitted: the item list is split straight out of the match string, and
/// the menu *name*, the one value that used to need an `'src` slice, is now
/// recovered exactly by [`text_slice`] – the same lift the anchor family made
/// for its id, and for the same reason (a [`Ui`] node carries no `Span`-typed
/// field, so nothing on it needs real source bytes). Only the node's
/// `location` keeps design §4.4's coarse fallback.
fn menu_match_is_sliceable(
    s: &str,
    pieces: &[Piece],
    full: &std::ops::Range<usize>,
    items: Option<&std::ops::Range<usize>>,
) -> bool {
    for piece in pieces {
        let p_start = piece.s_start;
        let p_end = piece.s_start + piece.s_len;

        // Skip pieces that do not overlap the match.
        if p_end <= full.start || p_start >= full.end {
            continue;
        }

        if !piece.atomic {
            continue;
        }

        // The one admitted atomic piece: a submenu caret the node consumes as
        // the item list's delimiter. Its match-string bytes identify it
        // unambiguously – a rendered span contributes a single placeholder
        // character, and the only other atomic pieces are the two remaining
        // special-character entities.
        if s.get(p_start..p_end) != Some(SUBMENU_DELIMITER) {
            return false;
        }

        let inside_items = items.is_some_and(|items| p_start >= items.start && p_end <= items.end);

        if !inside_items {
            return false;
        }
    }

    true
}

/// Builds one [`Ui`](InlineNode::Ui) menu node from a match whose own text
/// [`menu_match_is_sliceable`] accepts – the gate lives here rather than in
/// [`find_menu_matches`] because it needs the match's own capture groups to
/// know which sub-range may carry a submenu caret. Returns `None` for a match
/// it rejects, which the caller leaves unrecognized.
///
/// The menu name borrows from `'src` in the verbatim case (and is recovered as
/// an owned value from a [`synthesized`](Piece::synthesized) run, via
/// [`text_slice`]); the submenu path and trailing item are split exactly as the
/// string replacer splits them (owned, because a split part is trimmed).
fn build_menu_node<'src>(
    caps: &regex::Captures<'_>,
    full: &std::ops::Range<usize>,
    s: &str,
    pieces: &[Piece],
    root: Span<'src>,
    nodes: &[InlineNode<'src>],
) -> Option<InlineNode<'src>> {
    // Group 1 (the menu name) is mandatory in the pattern; group 2 (the items)
    // is optional. `unwrap` is safe: the pattern cannot match without group 1.
    #[allow(clippy::unwrap_used)]
    let name = caps.get(1).unwrap();

    let items = caps.get(2).map(|m| m.start()..m.end());

    if !menu_match_is_sliceable(s, pieces, full, items.as_ref()) {
        return None;
    }

    let location = source_slice(pieces, full.clone(), root);

    // The gate above admits no atomic piece in the name, so `text_slice`
    // always yields its exact text – borrowed from `'src` when the name is
    // verbatim, owned when it comes from a synthesized run.
    let menu = text_slice(nodes, pieces, name.start()..name.end())?;

    let (submenus, item) = split_menu_items(caps.get(2).map(|m| m.as_str()));

    Some(InlineNode::Ui(Ui {
        kind: UiKind::Menu {
            menu,
            submenus,
            item,
        },
        location,
    }))
}

/// Splits a menu macro's item list into its submenu path and trailing item,
/// reproducing the string replacer's delimiter handling: a
/// [`SUBMENU_DELIMITER`] (from a source `>`) takes precedence over a comma,
/// the last part is the menu item, and any earlier parts are submenus. With no
/// delimiter the whole (right-trimmed) list is a single item, and an absent
/// list (an empty `[]`) is a bare menu reference.
///
/// The list it splits is the *match string* text – the same escaped text the
/// string replacer splits – so the caret branch keys off `&gt;` here exactly as
/// it does there. Every part the split yields is verbatim source text (the
/// carets are the only non-source pieces a match may carry, and they are
/// consumed by the split itself – see [`menu_match_is_sliceable`]), so each
/// part is the very text `render_menu` receives from the string pipeline.
fn split_menu_items<'src>(items: Option<&str>) -> (Vec<CowStr<'src>>, Option<CowStr<'src>>) {
    let Some(items) = items else {
        return (vec![], None);
    };

    let mut items = items.to_string();
    if items.contains(']') {
        items = items.replace("\\]", "]");
    }

    let delim = if items.contains(SUBMENU_DELIMITER) {
        Some(SUBMENU_DELIMITER)
    } else if items.contains(',') {
        Some(",")
    } else {
        None
    };

    if let Some(delim) = delim {
        let mut parts: Vec<String> = items.split(delim).map(|i| i.trim().to_string()).collect();
        let item = parts.pop().map(CowStr::from);

        (parts.into_iter().map(CowStr::from).collect(), item)
    } else {
        (vec![], Some(CowStr::from(items.trim_end().to_string())))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]

    use super::super::super::test_support::{
        assert_styled, assert_text, build_src, fold_html, golden_macros, golden_macros_with,
    };
    use crate::{
        Parser, Span,
        content::inline_builder::build,
        inlines::{InlineNode, SpanForm, StyleVariant, Ui, UiKind},
        parser::HtmlSubstitutionRenderer,
        strings::CowStr,
    };

    /// A parser with the `experimental` attribute set, so the UI macros are
    /// recognized (the string step gates them on it, and the builder mirrors
    /// that gate).
    fn experimental_parser() -> Parser {
        use crate::parser::ModificationContext;

        Parser::default().with_intrinsic_attribute_bool(
            "experimental",
            true,
            ModificationContext::Anywhere,
        )
    }

    /// Builds the single-pass tree for `source` under [`experimental_parser`].
    fn build_ui(source: Span<'_>) -> Vec<InlineNode<'_>> {
        build(source, &experimental_parser(), None)
    }

    #[test]
    fn fold_matches_the_string_pipeline_through_ui_macros() {
        // For each fixture, folding the single-pass tree (all five steps, under
        // `experimental`) reproduces the string pipeline's output byte-for-byte.
        // This is the differential corpus (design §5.3) that pins the UI-macro
        // increment. Every fixture is deliberately *verbatim* (no `<`/`>`/`&`
        // inside a macro), the boundary this increment claims.
        let fixtures = [
            // No UI macro despite macro-ish characters.
            "kbd is a word, not a macro",
            "a menu without a bracket menu:File stays literal",
            // Passes the `kbd:`/`:[` prefilter but never forms a `kbd:[…]`, so
            // the pattern sweep finds nothing and the level returns unchanged.
            "note kbd: and a :[ bracket here",
            // Keyboard: single key and sequences (both delimiters).
            "kbd:[Enter]",
            "press kbd:[F11] to go full screen",
            "kbd:[Ctrl+T]",
            "kbd:[Ctrl+Shift+N]",
            "kbd:[Ctrl,T]",
            // A key whose value is the delimiter, and an escaped bracket.
            "kbd:[Ctrl + +]",
            "kbd:[Ctrl+\\]]",
            // Button: plain, embedded, and whitespace-normalized.
            "btn:[Save]",
            "click btn:[OK] to continue",
            "btn:[ Trim Me ]",
            // Menu: bare, single item, multi-word item, comma submenu path.
            "menu:File[]",
            "menu:File[Save]",
            "menu:File[Save As]",
            "menu:Tools[Project, Build]",
            "menu:View[Tool Windows, Project, Structure]",
            // Menu: the `&gt;` submenu form, whose delimiters are escaped
            // `CharRef`s by the time macros run (the relaxed gate this
            // increment adds), with and without surrounding spaces, at one and
            // several levels, and taking precedence over a comma.
            "menu:View[Zoom > Reset]",
            "menu:View[Zoom>Reset]",
            "menu:File[Save As > PDF]",
            "menu:View[Tools > Options > Advanced]",
            "menu:File[Save, As > PDF]",
            "menu:File[> Leading caret]",
            "menu:File[Save a\\] file > Now]",
            "Choose menu:View[Zoom > Reset] to reset the zoom.",
            "menu:View[Zoom > Reset] and menu:File[Save]",
            "*menu:View[Zoom > Reset]*",
            "\\menu:View[Zoom > Reset]",
            // An escaped macro the gate would *reject* (its item list crosses
            // an escaped `&`): the escape is honored ahead of the gate, so the
            // backslash is dropped here exactly as the string replacer drops
            // it.
            "\\menu:File[A & B]",
            // Several UI macros together, and next to another macro family.
            "See kbd:[F1] for help and btn:[Go] to run.",
            "kbd:[A] then image:x.png[X]",
            // A UI macro inside a rendered span.
            "*press kbd:[Esc]*",
            "_click btn:[OK] now_",
            // Escapes: the macro stays literal, minus the backslash.
            "\\kbd:[X]",
            "\\btn:[Y]",
            "\\menu:File[Save]",
        ];

        let renderer = HtmlSubstitutionRenderer {};
        let parser = experimental_parser();

        for fixture in fixtures {
            let folded = crate::content::inline_builder::fold_html(
                &build(Span::new(fixture), &parser, None),
                &renderer,
                &parser,
            );

            assert_eq!(
                folded,
                golden_macros_with(fixture, &parser),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn ui_macros_are_literal_without_experimental() {
        // Without `experimental`, the string step does not recognize the UI
        // macros, and neither does the builder: the fold reproduces the literal
        // (default-parser) output byte-for-byte.
        let fixtures = ["kbd:[Ctrl+T]", "btn:[Save]", "menu:File[Save]"];

        let renderer = HtmlSubstitutionRenderer {};

        for fixture in fixtures {
            let nodes = build_src(Span::new(fixture));

            assert!(
                nodes.iter().all(|n| !matches!(n, InlineNode::Ui(_))),
                "no UI node without experimental: {nodes:?}"
            );

            assert_eq!(
                fold_html(&nodes, &renderer),
                golden_macros(fixture),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    /// Asserts that `node` is a [`Ui`], returning it for further inspection.
    fn assert_ui<'a, 'src>(node: &'a InlineNode<'src>) -> &'a Ui<'src> {
        match node {
            InlineNode::Ui(ui) => ui,

            other => panic!("expected Ui, got {other:?}"),
        }
    }

    #[test]
    fn a_kbd_macro_becomes_a_ui_node() {
        let nodes = build_ui(Span::new("kbd:[Enter]"));

        assert_eq!(nodes.len(), 1);
        let ui = assert_ui(&nodes[0]);

        match &ui.kind {
            UiKind::Keyboard(keys) => assert_eq!(keys, &[CowStr::from("Enter")]),

            other => panic!("expected Keyboard, got {other:?}"),
        }

        // Its location covers the whole macro, delimiters included.
        assert_eq!(ui.location.data(), "kbd:[Enter]");
        assert_eq!(ui.location.line(), 1);
        assert_eq!(ui.location.col(), 1);
    }

    #[test]
    fn a_kbd_sequence_splits_into_keys() {
        // The `+`/`,` delimiter selects how the sequence is split into keys,
        // exactly as the string replacer's `split_kbd_keys` does.
        let nodes = build_ui(Span::new("kbd:[Ctrl+Shift+N]"));

        match &assert_ui(&nodes[0]).kind {
            UiKind::Keyboard(keys) => assert_eq!(
                keys,
                &[
                    CowStr::from("Ctrl"),
                    CowStr::from("Shift"),
                    CowStr::from("N"),
                ]
            ),

            other => panic!("expected Keyboard, got {other:?}"),
        }
    }

    #[test]
    fn a_btn_macro_becomes_a_ui_node() {
        // The label is normalized (surrounding whitespace folded) like the
        // string replacer's `normalize_index_text`.
        let nodes = build_ui(Span::new("btn:[ Save ]"));

        match &assert_ui(&nodes[0]).kind {
            UiKind::Button(text) => assert_eq!(text.as_ref(), "Save"),

            other => panic!("expected Button, got {other:?}"),
        }
    }

    #[test]
    fn a_menu_with_a_comma_path_splits_submenu_and_item() {
        // The last comma-separated part is the menu item; earlier parts are the
        // submenu path. The menu name borrows from source.
        let nodes = build_ui(Span::new("menu:View[Tool Windows, Project, Structure]"));

        match &assert_ui(&nodes[0]).kind {
            UiKind::Menu {
                menu,
                submenus,
                item,
            } => {
                assert!(matches!(menu, CowStr::Borrowed(_)));
                assert_eq!(menu.as_ref(), "View");
                assert_eq!(
                    submenus,
                    &[CowStr::from("Tool Windows"), CowStr::from("Project")]
                );
                assert_eq!(item.as_deref(), Some("Structure"));
            }

            other => panic!("expected Menu, got {other:?}"),
        }
    }

    #[test]
    fn a_bare_menu_reference_has_no_item() {
        // `menu:File[]` is a bare reference: no submenus and no item.
        let nodes = build_ui(Span::new("menu:File[]"));

        match &assert_ui(&nodes[0]).kind {
            UiKind::Menu {
                menu,
                submenus,
                item,
            } => {
                assert_eq!(menu.as_ref(), "File");
                assert!(submenus.is_empty());
                assert_eq!(item.as_deref(), None);
            }

            other => panic!("expected Menu, got {other:?}"),
        }
    }

    #[test]
    fn an_escaped_ui_macro_stays_literal() {
        // `\kbd:[X]` drops the backslash and keeps the macro as literal text –
        // no UI node.
        let nodes = build_ui(Span::new("\\kbd:[X]"));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ui(_))),
            "an escaped UI macro must not produce a UI node: {nodes:?}"
        );

        assert_eq!(
            crate::content::inline_builder::fold_html(
                &nodes,
                &HtmlSubstitutionRenderer {},
                &experimental_parser()
            ),
            golden_macros_with("\\kbd:[X]", &experimental_parser())
        );
    }

    #[test]
    fn a_ui_macro_is_recognized_inside_a_span() {
        // A UI macro can appear inside a rendered span; the transducer descends
        // into the span body and builds the node there.
        let nodes = build_ui(Span::new("*press kbd:[Esc]*"));

        let children = assert_styled(&nodes[0], StyleVariant::Strong, SpanForm::Constrained);
        assert_eq!(children.len(), 2);
        assert_text(&children[0], "press ", 1, 2);

        match &assert_ui(&children[1]).kind {
            UiKind::Keyboard(keys) => assert_eq!(keys, &[CowStr::from("Esc")]),

            other => panic!("expected Keyboard, got {other:?}"),
        }
    }

    #[test]
    fn a_menu_with_a_submenu_caret_splits_into_levels() {
        // The submenu form uses `>` as the level delimiter, and by the time
        // macros run the special-characters step has turned each one into a
        // `&gt;` `CharRef` – an atomic piece the family's verbatim gate used to
        // reject outright. A caret carries no value the node slices (the string
        // replacer consumes it as the delimiter and emits it nowhere), so the
        // relaxed gate admits it and the node splits into its levels, with only
        // the item texts around it required to be verbatim.
        let nodes = build_ui(Span::new("menu:View[Tools > Options > Advanced]"));

        assert_eq!(nodes.len(), 1);
        let ui = assert_ui(&nodes[0]);

        match &ui.kind {
            UiKind::Menu {
                menu,
                submenus,
                item,
            } => {
                assert!(matches!(menu, CowStr::Borrowed(_)));
                assert_eq!(menu.as_ref(), "View");
                assert_eq!(submenus, &[CowStr::from("Tools"), CowStr::from("Options")]);
                assert_eq!(item.as_deref(), Some("Advanced"));
            }

            other => panic!("expected Menu, got {other:?}"),
        }

        // Its location covers the whole macro in *source* terms – the carets
        // are one byte each there, four in the match string.
        assert_eq!(ui.location.data(), "menu:View[Tools > Options > Advanced]");
        assert_eq!(ui.location.line(), 1);
        assert_eq!(ui.location.col(), 1);
    }

    #[test]
    fn a_menu_over_a_special_character_is_a_documented_divergence() {
        // A submenu caret is the *only* escaped special a menu match may carry
        // (the node consumes it as the item list's delimiter). Any other one –
        // an `&` in the item list, or in the menu name the pattern also admits
        // it in – is matched by the string pipeline over the *escaped* text
        // (`menu:File[A &amp; B]`), which a self-describing node cannot carry
        // as an `'src` slice, so the single-pass builder leaves the macro
        // *unrecognized* here (deferred to a later increment), exactly as the
        // image increment defers a macro over a special character.
        for source in [
            "menu:File[A & B]",
            "menu:A&B[Save]",
            // A caret in the *name* – which the pattern admits – is not a
            // delimiter the node consumes, so it is not admitted either: the
            // name would have to carry the escaped `&gt;` the node cannot
            // slice from `'src`.
            "menu:a>b[Save]",
            "menu:File[*S* > As]",
        ] {
            let nodes = build_ui(Span::new(source));

            assert!(
                nodes.iter().all(|n| !matches!(n, InlineNode::Ui(_))),
                "a menu crossing a non-caret special must be left unrecognized: {nodes:?}"
            );

            // The string pipeline, by contrast, *does* build a menu here – the
            // divergence this test documents.
            assert!(
                golden_macros_with(source, &experimental_parser()).contains(r#"class="menu"#),
                "expected the golden pipeline to build a menu for {source:?}"
            );
        }
    }

    #[test]
    fn a_kbd_macro_over_a_special_character_is_a_documented_divergence() {
        // A `kbd:` whose bracket content contains a special character (`<`) is
        // matched by the string pipeline over the *escaped* text (`kbd:[a&lt;b]`),
        // but a self-describing node cannot carry that escaped text as an `'src`
        // slice, so the single-pass builder leaves the macro *unrecognized* here
        // (deferred to a later increment), exactly as the image increment defers
        // a macro over a special. Unlike a menu's own submenu caret, a keyboard
        // macro's specials are part of a key the node *does* slice, so there is
        // nothing here to relax.
        let nodes = build_ui(Span::new("kbd:[a<b]"));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ui(_))),
            "a kbd macro crossing an escaped special must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* build a keyboard macro here.
        assert!(golden_macros_with("kbd:[a<b]", &experimental_parser()).contains("<kbd>"));
    }

    /// A parser with `experimental` plus the attributes the
    /// expanded-value fixtures below reference.
    fn expanding_parser() -> Parser {
        use crate::parser::ModificationContext;

        experimental_parser()
            .with_intrinsic_attribute("zoom", "Zoom", ModificationContext::Anywhere)
            .with_intrinsic_attribute("view", "View", ModificationContext::Anywhere)
            .with_intrinsic_attribute("key", "Ctrl+T", ModificationContext::Anywhere)
            .with_intrinsic_attribute("label", "Save", ModificationContext::Anywhere)
            .with_intrinsic_attribute("macro-src", "kbd:[Esc]", ModificationContext::Anywhere)
    }

    /// The real, public pipeline's output for `source` – the golden for the
    /// expanded-value fixtures, which need the `AttributeReferences` step the
    /// module's own [`golden_macros`] helper deliberately omits.
    fn golden_normal(source: &str, parser: &Parser) -> String {
        use crate::content::{Content, SubstitutionGroup};

        let mut content = Content::from(Span::new(source));
        SubstitutionGroup::Normal.apply(&mut content, parser, None);
        content.rendered_str().to_string()
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_ui_macros_inside_expanded_values() {
        // A UI macro whose name, keys, label, or item list crosses a
        // *synthesized* run (an attribute expansion) is now recognized: a
        // [`Ui`] node carries no `Span`-typed field, so every value it holds
        // comes straight from the match string – which carries a synthesized
        // run's bytes exactly – or, for the menu name, from `text_slice`. This
        // is the same lift the anchor family made for its id, and it closes
        // the divergence `a_menu_inside_an_expanded_value_is_a_documented_
        // divergence` used to pin.
        let parser = expanding_parser();

        let fixtures = [
            // The item list, the submenu path, and the menu name.
            "menu:View[{zoom} > Reset]",
            "menu:View[{zoom}, Reset]",
            "menu:{view}[Zoom > Reset]",
            "menu:{view}[{zoom} > Reset]",
            "menu:{view}[]",
            // Keyboard keys and a button label.
            "kbd:[{key}]",
            "kbd:[{key}+Shift]",
            "btn:[{label}]",
            "press kbd:[{key}] then btn:[{label}]",
            // The whole macro arriving from an expanded value.
            "{macro-src}",
            "before {macro-src} after",
        ];

        for source in fixtures {
            let nodes = build(Span::new(source), &parser, None);

            assert_eq!(
                crate::content::inline_builder::fold_html(
                    &nodes,
                    &HtmlSubstitutionRenderer {},
                    &parser
                ),
                golden_normal(source, &parser),
                "fold diverged from the string pipeline for {source:?}"
            );
        }
    }

    #[test]
    fn a_menu_inside_an_expanded_value_keeps_a_coarse_location() {
        // The values are exact; only the node's `location` falls back to the
        // enclosing synthesized run's coarse span (design §4.4), since an
        // expanded value's bytes have no `'src` counterpart of their own. The
        // menu name recovered from such a run is necessarily owned.
        let parser = expanding_parser();

        let source = "menu:{view}[{zoom} > Reset]";
        let nodes = build(Span::new(source), &parser, None);

        assert_eq!(nodes.len(), 1, "{nodes:?}");

        let InlineNode::Ui(ui) = &nodes[0] else {
            panic!("expected a Ui node, got {:?}", nodes[0]);
        };

        match &ui.kind {
            UiKind::Menu {
                menu,
                submenus,
                item,
            } => {
                assert_eq!(menu.as_ref(), "View");
                assert!(matches!(menu, CowStr::Boxed(_)), "{menu:?}");
                assert_eq!(submenus.len(), 1);
                assert_eq!(submenus[0].as_ref(), "Zoom");
                assert_eq!(item.as_deref(), Some("Reset"));
            }

            other => panic!("expected a menu, got {other:?}"),
        }

        // The whole match is the node's location; its `{view}`/`{zoom}` bytes
        // are the source's, not the expanded value's.
        assert_eq!(ui.location.data(), source);
        assert_eq!(ui.location.line(), 1);
        assert_eq!(ui.location.col(), 1);
    }

    #[test]
    fn ui_macros_are_recognized_when_the_whole_seed_is_synthesized() {
        // The same lift reached at the tree's *root* rather than a nested
        // splice: `build_from_value`'s synthesized-seed path (the shape
        // `Content::from_filtered_lines` produces for a genuinely multi-line,
        // filtered block), mirroring the anchor family's own
        // `an_anchor_is_recognized_when_the_whole_seed_is_synthesized`.
        use crate::content::inline_builder::build_from_value;

        let filtered = "press kbd:[Ctrl+T]\nor menu:View[Zoom > Reset]";
        let source = "  press kbd:[Ctrl+T]\n  or menu:View[Zoom > Reset]";

        let parser = experimental_parser();
        let nodes = build_from_value(
            CowStr::from(filtered.to_string()),
            Span::new(source),
            &parser,
            None,
        );

        let ui_nodes = nodes
            .iter()
            .filter(|n| matches!(n, InlineNode::Ui(_)))
            .count();

        assert_eq!(ui_nodes, 2, "expected both UI macros: {nodes:?}");

        assert_eq!(
            crate::content::inline_builder::fold_html(
                &nodes,
                &HtmlSubstitutionRenderer {},
                &parser
            ),
            golden_normal(filtered, &parser),
            "fold diverged from the string pipeline for the synthesized seed"
        );
    }

    #[test]
    fn a_real_documents_expanded_ui_macro_reaches_its_tree() {
        // End-to-end, through the real parse path: a document attribute whose
        // value feeds a UI macro. The rendered string and the fold of the
        // block's own tree agree, and the tree carries the recognized node
        // rather than the literal text it used to.
        use crate::blocks::{FindBlocks, IsBlock};

        let doc = Parser::default()
            .with_inline_tree(true)
            .parse(":experimental:\n:view: View\n\nChoose menu:{view}[Zoom > Reset].");

        let blocks: Vec<_> = doc.descendant_blocks().collect();
        let rendered = blocks[0].rendered_html_content().unwrap();
        let inlines = blocks[0].inlines().unwrap();

        assert!(
            rendered.contains(r#"class="submenu""#),
            "rendered: {rendered}"
        );

        assert!(
            inlines.iter().any(|n| matches!(n, InlineNode::Ui(_))),
            "expected a Ui node in the block's tree: {inlines:?}"
        );

        assert_eq!(
            crate::content::inline_builder::fold_html(
                inlines,
                &HtmlSubstitutionRenderer {},
                &Parser::default()
            ),
            rendered,
            "fold diverged from the rendered string for {inlines:?}"
        );
    }

    #[test]
    fn split_menu_items_reproduces_the_delimiter_handling() {
        use super::split_menu_items;

        // Helper to compare against owned expectations.
        let go = |items: Option<&str>| {
            let (submenus, item) = split_menu_items(items);

            (
                submenus.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                item.map(|i| i.to_string()),
            )
        };

        // An absent list (an empty `[]`) is a bare menu reference.
        assert_eq!(go(None), (vec![], None));

        // No delimiter: the whole (right-trimmed) list is a single item.
        assert_eq!(go(Some("Save As ")), (vec![], Some("Save As".to_string())));

        // Comma: the last part is the item, earlier parts are submenus.
        assert_eq!(
            go(Some("Project, Build")),
            (vec!["Project".to_string()], Some("Build".to_string()))
        );

        // An escaped `]` inside the list is unescaped before splitting.
        assert_eq!(
            go(Some("a\\]b, c")),
            (vec!["a]b".to_string()], Some("c".to_string()))
        );

        // The `&gt;` submenu delimiter takes precedence over a comma – checked
        // directly here on the escaped text the split actually receives, since
        // a fixture's own source spells the delimiter `>`.
        assert_eq!(
            go(Some("View &gt; Zoom &gt; Reset")),
            (
                vec!["View".to_string(), "Zoom".to_string()],
                Some("Reset".to_string())
            )
        );
    }
}
