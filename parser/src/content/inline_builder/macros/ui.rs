//! UI macro recognition (`kbd:[…]`, `btn:[…]`, `menu:…[…]`).

use super::{MacroMatch, MacroMatchKind, image::range_is_verbatim, rebuild_macro_level};
use crate::{
    Span,
    content::{
        INLINE_KBD_BTN_MACRO, INLINE_MENU_MACRO,
        inline_builder::quotes::{Piece, build_match_string, source_slice},
        normalize_index_text, split_kbd_keys,
    },
    inlines::{InlineNode, Ui, UiKind},
    strings::CowStr,
};

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

/// Finds every keyboard/button macro at this level, skipping any whose match is
/// not wholly verbatim source (see [`apply_macros`](super::apply_macros)).
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

        // Only a wholly-verbatim match maps its bracket content 1:1 onto source;
        // a match crossing an escaped special or a rendered span is left for a
        // later increment.
        if !range_is_verbatim(pieces, &full) {
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

/// Builds one [`Ui`](InlineNode::Ui) node from a verbatim keyboard/button
/// match, splitting the keys / normalizing the label exactly as the string
/// replacer does so the fold reproduces the same bytes. On a verbatim match the
/// bracket content is source text, so this splits/normalizes precisely what the
/// string step splits/normalizes from the (identical, un-escaped) rendered
/// text.
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
/// level's escaped text and replaces each verbatim match with the
/// [`Ui`](InlineNode::Ui) node it produces.
///
/// The caller runs this only under the `experimental` document attribute (see
/// [`apply_macros`](super::apply_macros)); a cheap prefilter still skips the
/// pattern sweep when no `menu:` prefix with an opening bracket is present. The
/// `&gt;`-submenu form is always non-verbatim (its `>` is an escaped
/// [`CharRef`](InlineNode::CharRef) by the time macros run) and is therefore
/// deferred; the comma-delimited and bare/single-item forms are verbatim and
/// handled here.
pub(super) fn menu_macros_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes);

    if !(s.contains("menu:") && s.contains('[')) {
        return nodes;
    }

    let matches = find_menu_matches(&s, &pieces, root);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every menu macro at this level, skipping any whose match is not wholly
/// verbatim source (see [`apply_macros`](super::apply_macros)).
fn find_menu_matches<'src>(s: &str, pieces: &[Piece], root: Span<'src>) -> Vec<MacroMatch<'src>> {
    let mut matches = Vec::new();

    for caps in INLINE_MENU_MACRO.captures_iter(s) {
        // `unwrap` on group 0 is safe: a capture always has an overall match.
        #[allow(clippy::unwrap_used)]
        let whole = caps.get(0).unwrap();

        let full = whole.start()..whole.end();

        if !range_is_verbatim(pieces, &full) {
            continue;
        }

        // The menu pattern's escape is an uncaptured leading `\?`.
        if whole.as_str().starts_with('\\') {
            matches.push(MacroMatch {
                kind: MacroMatchKind::Unescape {
                    backslash: full.start,
                },
                full,
            });

            continue;
        }

        let node = build_menu_node(&caps, &full, pieces, root);

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

/// Builds one [`Ui`](InlineNode::Ui) menu node from a verbatim match. The menu
/// name borrows from `'src`; the submenu path and trailing item are split
/// exactly as the string replacer splits them (owned, because a split part is
/// trimmed).
fn build_menu_node<'src>(
    caps: &regex::Captures<'_>,
    full: &std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
) -> InlineNode<'src> {
    let location = source_slice(pieces, full.clone(), root);

    // Group 1 (the menu name) is mandatory in the pattern, and on a verbatim
    // match it slices straight from `'src`, so the name borrows.
    // `unwrap` is safe: the pattern cannot match without group 1.
    #[allow(clippy::unwrap_used)]
    let name = caps.get(1).unwrap();

    let menu = CowStr::from(source_slice(pieces, name.start()..name.end(), root).data());

    // Group 2 (the items) is optional.
    let (submenus, item) = split_menu_items(caps.get(2).map(|m| m.as_str()));

    InlineNode::Ui(Ui {
        kind: UiKind::Menu {
            menu,
            submenus,
            item,
        },
        location,
    })
}

/// Splits a menu macro's item list into its submenu path and trailing item,
/// reproducing the string replacer's delimiter handling: a `&gt;` (from a
/// source `>`) takes precedence over a comma, the last part is the menu item,
/// and any earlier parts are submenus. With no delimiter the whole
/// (right-trimmed) list is a single item, and an absent list (an empty `[]`) is
/// a bare menu reference. Because a `&gt;` never survives into a *verbatim*
/// match (see [`menu_macros_level`]), only the comma / single-item / bare
/// branches run in practice; the `&gt;` branch is kept so the split stays
/// faithful.
fn split_menu_items<'src>(items: Option<&str>) -> (Vec<CowStr<'src>>, Option<CowStr<'src>>) {
    let Some(items) = items else {
        return (vec![], None);
    };

    let mut items = items.to_string();
    if items.contains(']') {
        items = items.replace("\\]", "]");
    }

    let delim = if items.contains("&gt;") {
        Some("&gt;")
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
    fn a_menu_with_a_submenu_caret_is_a_documented_divergence() {
        // The submenu form `menu:View[Zoom > Reset]` uses `>` as the level
        // delimiter, but by the time macros run the special-characters step has
        // turned `>` into a `&gt;` `CharRef`. A self-describing node cannot carry
        // that escaped text as an `'src` slice, so the single-pass builder leaves
        // the macro *unrecognized* here (deferred to a later increment), exactly
        // as the image increment defers a macro over a special character.
        let source = "menu:View[Zoom > Reset]";
        let nodes = build_ui(Span::new(source));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ui(_))),
            "a menu crossing an escaped `>` must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* build a menuseq with a
        // submenu here – the divergence this test documents.
        assert!(golden_macros_with(source, &experimental_parser()).contains(r#"class="submenu""#));
    }

    #[test]
    fn a_kbd_macro_over_a_special_character_is_a_documented_divergence() {
        // A `kbd:` whose bracket content contains a special character (`<`) is
        // matched by the string pipeline over the *escaped* text (`kbd:[a&lt;b]`),
        // but a self-describing node cannot carry that escaped text as an `'src`
        // slice, so the single-pass builder leaves the macro *unrecognized* here
        // (deferred to a later increment), exactly as it defers the `&gt;`-submenu
        // menu form and the image increment defers a macro over a special.
        let nodes = build_ui(Span::new("kbd:[a<b]"));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ui(_))),
            "a kbd macro crossing an escaped special must be left unrecognized: {nodes:?}"
        );

        // The string pipeline, by contrast, *does* build a keyboard macro here.
        assert!(golden_macros_with("kbd:[a<b]", &experimental_parser()).contains("<kbd>"));
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

        // The `&gt;` submenu delimiter takes precedence over a comma. This form
        // never reaches [`build_menu_node`] through the verbatim boundary (its
        // source `>` is an escaped `CharRef` by macro time), so it is exercised
        // directly here to keep [`split_menu_items`] a faithful reproduction of
        // the string replacer's splitting.
        assert_eq!(
            go(Some("View &gt; Zoom &gt; Reset")),
            (
                vec!["View".to_string(), "Zoom".to_string()],
                Some("Reset".to_string())
            )
        );
    }
}
