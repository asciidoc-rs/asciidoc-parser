//! UI macro recognition (`kbd:[…]`, `btn:[…]`, `menu:…[…]`).

use super::{MacroMatch, MacroMatchKind, image::range_has_no_opaque_piece, rebuild_macro_level};
use crate::{
    Span,
    content::{
        INLINE_KBD_BTN_MACRO, INLINE_MENU_MACRO,
        inline_builder::{
            quotes::{LevelContext, Piece, build_match_string, source_slice, text_slice},
            special_chars::Masked,
        },
        normalize_index_text, split_kbd_keys,
    },
    inlines::{InlineNode, Ui, UiKind},
    strings::CowStr,
};

/// The delimiter a menu macro's item list is split on: the *escaped* form of
/// the source `>` submenu caret, which is what the string replacer sees (the
/// special-characters step runs long before macros) and therefore what this
/// module's own match string presents too — as an atomic
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
    ctx: LevelContext,
    masked: Masked<'_>,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes, masked);

    if !(s.contains(":[") && (s.contains("kbd:") || s.contains("btn:"))) {
        return nodes;
    }

    // Matched over the level wrapped in the boundary character its enclosing
    // construct presents, with the level's own pieces moved into that string's
    // coordinates — see `apply_macro_families`'s own doc comment.
    let (s, pieces) = ctx.shift(s, pieces);

    let matches = find_kbd_btn_matches(&nodes, &s, &pieces, root);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every keyboard/button macro at this level, skipping any whose match
/// crosses an **opaque** piece (see [`apply_macros`](super::apply_macros)).
fn find_kbd_btn_matches<'src>(
    nodes: &[InlineNode<'src>],
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

        // Group 1 is the (optional) escape backslash. It is honored *before*
        // the gate below — and needs no gate of its own — because dropping
        // the backslash keeps the rest of the match as its own original nodes
        // (a rendered span among them), which fold back to exactly the bytes
        // the string replacer's `caps[0][1..]` emits. Mirrors that replacer's
        // own escape-first check order, and closes the same latent gap the
        // `footnoteref:`, menu, cross-reference, link, and image increments
        // each closed for their own families.
        if caps.get(1).is_some() {
            matches.push(MacroMatch {
                kind: MacroMatchKind::Unescape {
                    backslash: full.start,
                },
                full,
            });

            continue;
        }

        // A match crossing an **opaque** piece — a rendered span, an
        // earlier-recognized macro node, a masked passthrough — is left for a
        // later increment: `build_match_string` stands each in as one
        // placeholder where the string pipeline's own haystack holds the
        // markup it will fold to, so the keys or label read out of the match
        // string would not be the replacer's.
        //
        // Every *recoverable* piece is admitted: a
        // [`synthesized`](Piece::synthesized) run (an attribute expansion,
        // or — reached at a tree's root — a filtered multi-line block's own
        // joined seed), an escaped special (`kbd:[Ctrl&C]`), a restored
        // entity (`kbd:[Ctrl&copy;C]`), and a typographic replacement
        // (`kbd:[a(C)b]`). This family never slices `'src` for a value at all
        // — its keys and label come straight from the match string, which
        // carries every one of those pieces' bytes exactly — so only the
        // node's `location` takes design §4.4's coarse fallback (see
        // [`build_kbd_btn_node`]). Nor can a boundary split such a leaf: the
        // match is delimited by `kbd:`/`btn:`, `[`, and `]`, and its keys by
        // `,`/`+` — none of which occurs in `&lt;`, `&gt;`, `&amp;`, or a
        // restored entity's own `&name;` — so every atomic overlap is a
        // *wholly contained* entity, the same structural argument the
        // bare-e-mail and image families make for their own patterns.
        if !range_has_no_opaque_piece(nodes, pieces, &full) {
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
/// `'src` slice: on a verbatim match those bytes *are* the source text; on a
/// [`synthesized`](Piece::synthesized) one they are the expanded value the
/// string pipeline itself matched over; and across an escaped special or a
/// restored entity they are that leaf's own entity bytes (`&amp;`, `&copy;`) —
/// which is what the string replacer's own escaped haystack holds there, and
/// what `render_keyboard`/`render_button` then emit *verbatim*. That is
/// precisely what lets this family recognize a macro inside an expanded
/// attribute value, or across an entity, where a family carrying an
/// [`Attrlist`](crate::attributes::Attrlist)`<'src>` cannot. Only the node's
/// `location` falls back to the enclosing run's coarse span (design §4.4) in
/// the synthesized case.
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
/// [`CharRef`](InlineNode::CharRef)s by the time macros run. A name or item
/// text crossing any *other* escaped special (`menu:File[Save & Exit]`) or a
/// restored entity (`menu:&#8942;[More Tools, Extensions]`) is recognized too:
/// like a keyboard macro's keys, every value a menu node holds is read out of
/// the match string, whose bytes at such a leaf are exactly the ones the
/// string replacer's own escaped haystack carries and `render_menu` emits
/// verbatim (see [`find_menu_matches`]). What is still deferred is a name or
/// item text crossing an **opaque** piece — a rendered
/// [`Styled`](crate::inlines::Styled) span, an earlier-recognized macro node,
/// a masked passthrough — the one boundary every macro family keeps. (A
/// typographic replacement, `menu:File[Save (C) Exit]`, is admitted too, for
/// the same reason a restored entity is: it carries its own bytes.)
pub(super) fn menu_macros_level<'src>(
    nodes: Vec<InlineNode<'src>>,
    root: Span<'src>,
    ctx: LevelContext,
    masked: Masked<'_>,
) -> Vec<InlineNode<'src>> {
    let (s, pieces) = build_match_string(&nodes, masked);

    if !(s.contains("menu:") && s.contains('[')) {
        return nodes;
    }

    // Matched over the level wrapped in the boundary character its enclosing
    // construct presents, with the level's own pieces moved into that string's
    // coordinates — see `apply_macro_families`'s own doc comment.
    let (s, pieces) = ctx.shift(s, pieces);

    let matches = find_menu_matches(&nodes, &s, &pieces, root);

    if matches.is_empty() {
        return nodes;
    }

    rebuild_macro_level(&nodes, &pieces, &s, matches)
}

/// Finds every menu macro at this level, skipping any whose match crosses an
/// **opaque** piece — the one boundary this family keeps, and the same
/// [`range_has_no_opaque_piece`] gate its keyboard/button sibling and every
/// other macro family applies.
///
/// A menu match may legitimately carry an atomic
/// [`CharRef`](InlineNode::CharRef) leaf, and always did for one spelling: the
/// `&gt;` submenu caret (`menu:View[Zoom > Reset]`), which is an escaped
/// special by the time macros run. That case used to be admitted by a bespoke,
/// caret-only check; it is now the general one, because *every* value a menu
/// node holds comes from the match string, whose bytes at such a leaf — a
/// special's canonical entity, a restored entity's own text — are exactly the
/// ones the string replacer's own escaped haystack carries and `render_menu`
/// emits verbatim. So a name or item text crossing any escaped special
/// (`menu:File[Save & Exit]`, `menu:a>b[Save]`) or restored entity
/// (`menu:&#8942;[More Tools, Extensions]`) is recognized, exactly as the
/// string pipeline recognizes it.
///
/// No boundary can split such a leaf, either: the match is delimited by
/// `menu:`, `[`, and `]`, and its item list by `&gt;` or `,` — none of which
/// occurs in `&lt;`, `&amp;`, or a restored entity's own `&name;`, while a
/// `&gt;` *is* the delimiter both pipelines split on — so every atomic overlap
/// is either wholly contained or consumed as the delimiter itself.
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
        // checked *before* the sliceability gate — and needs no gate of its
        // own — because dropping the backslash keeps the rest of the match as
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

        // A match crossing an opaque piece is left unrecognized, so the
        // surrounding gap reproduces the source unchanged (see
        // [`menu_macros_level`]).
        if !range_has_no_opaque_piece(nodes, pieces, &full) {
            continue;
        }

        let node = build_menu_node(&caps, &full, pieces, root, nodes);

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

/// Builds one [`Ui`](InlineNode::Ui) menu node from a match
/// [`find_menu_matches`]' gate accepts. It is **total**: with an opaque piece
/// already excluded, every shape the pattern can match yields a node (the
/// family's own "this increment defers" machinery retired with the gate that
/// needed the match's capture groups to decide).
///
/// The menu name borrows from `'src` when it is verbatim, is recovered as an
/// owned value from a [`synthesized`](Piece::synthesized) run (via
/// [`text_slice`]), and — when it crosses an escaped special or a restored
/// entity, which [`text_slice`] declines because it cannot slice one — comes
/// from the **match string**, i.e. in the already-substituted form
/// `render_menu` receives from the string pipeline and emits verbatim
/// (`menu:F&le[Save]`'s name is `F&amp;le` on both sides). The submenu path
/// and trailing item are split exactly as the string replacer splits them, out
/// of that same match string (owned, because a split part is trimmed).
fn build_menu_node<'src>(
    caps: &regex::Captures<'_>,
    full: &std::ops::Range<usize>,
    pieces: &[Piece],
    root: Span<'src>,
    nodes: &[InlineNode<'src>],
) -> InlineNode<'src> {
    // Group 1 (the menu name) is mandatory in the pattern; group 2 (the items)
    // is optional. `unwrap` is safe: the pattern cannot match without group 1.
    #[allow(clippy::unwrap_used)]
    let name = caps.get(1).unwrap();

    let location = source_slice(pieces, full.clone(), root);

    let menu = text_slice(nodes, pieces, name.start()..name.end())
        .unwrap_or_else(|| CowStr::from(name.as_str().to_string()));

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
/// reproducing the string replacer's delimiter handling: a
/// [`SUBMENU_DELIMITER`] (from a source `>`) takes precedence over a comma,
/// the last part is the menu item, and any earlier parts are submenus. With no
/// delimiter the whole (right-trimmed) list is a single item, and an absent
/// list (an empty `[]`) is a bare menu reference.
///
/// The list it splits is the *match string* text — the same escaped text the
/// string replacer splits — so the caret branch keys off `&gt;` here exactly as
/// it does there, and so does every other part: an item carrying an escaped
/// special or a restored entity keeps that leaf's own entity bytes, which is
/// precisely the text `render_menu` receives from the string pipeline and
/// emits verbatim.
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
        assert_styled, assert_text, build_src, fold_html, golden_macros, golden_macros_in,
    };
    use crate::{
        Parser, Span,
        content::inline_builder::build,
        inlines::{InlineNode, SpanForm, StyleVariant, Ui, UiKind},
        parser::HtmlInlineRenderer,
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
        // `experimental`) reproduces the string pipeline's output
        // byte-for-byte. This is the differential corpus (design §5.3)
        // that pins the UI-macro increment. Every fixture is
        // deliberately *verbatim* (no `<`/`>`/`&` inside a macro), the
        // boundary this increment claims.
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

        let renderer = HtmlInlineRenderer {};
        let parser = experimental_parser();

        for fixture in fixtures {
            let folded = crate::content::inline_builder::fold_html(
                &build(Span::new(fixture), &parser, None),
                &renderer,
                &parser.render_context(),
            );

            assert_eq!(
                folded,
                golden_macros_in("macros_experimental", fixture, &parser),
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

        let renderer = HtmlInlineRenderer {};

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
        // `\kbd:[X]` drops the backslash and keeps the macro as literal text —
        // no UI node.
        let nodes = build_ui(Span::new("\\kbd:[X]"));

        assert!(
            nodes.iter().all(|n| !matches!(n, InlineNode::Ui(_))),
            "an escaped UI macro must not produce a UI node: {nodes:?}"
        );

        assert_eq!(
            crate::content::inline_builder::fold_html(
                &nodes,
                &HtmlInlineRenderer {},
                &experimental_parser().render_context()
            ),
            golden_macros_in("macros_experimental", "\\kbd:[X]", &experimental_parser())
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
        // `&gt;` `CharRef` — an atomic piece the family's verbatim gate used to
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

        // Its location covers the whole macro in *source* terms — the carets
        // are one byte each there, four in the match string.
        assert_eq!(ui.location.data(), "menu:View[Tools > Options > Advanced]");
        assert_eq!(ui.location.line(), 1);
        assert_eq!(ui.location.col(), 1);
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_ui_macros_crossing_a_recoverable_piece() {
        // A UI macro whose name, keys, label, or item list crosses an
        // **escaped special** (`&`, `<`, `>`), a **restored entity**
        // (`&copy;`, `&#8942;`), or a **typographic replacement** (`(C)`, a
        // smart apostrophe) is recognized: all three are atomic pieces
        // `build_match_string` gives *real bytes* to — the very bytes the
        // string replacer's own escaped haystack carries there — and every
        // value a `Ui` node holds is read out of that match string and emitted
        // verbatim by `render_keyboard`/`render_button`/`render_menu`. So the
        // fold reproduces the string pipeline's output byte-for-byte, which is
        // what these fixtures pin. (This closes the boundary the
        // `a_menu_over_a_special_character_is_a_documented_divergence` and
        // `a_kbd_macro_over_a_special_character_is_a_documented_divergence`
        // tests used to pin, per their own "fold this into a parity corpus"
        // convention.)
        let parser = experimental_parser();
        let renderer = HtmlInlineRenderer {};

        let fixtures = [
            // Keyboard keys crossing each escaped special, alone and in a
            // sequence.
            "kbd:[Ctrl&C]",
            "kbd:[a<b]",
            "kbd:[a>b]",
            "kbd:[Ctrl+a&b]",
            "kbd:[a&b,c<d]",
            // A button label crossing one, and one crossing two.
            "btn:[Save & Close]",
            "btn:[a<b>c]",
            // A menu *name* crossing a special the pattern admits, including
            // the `>` that is not an item-list delimiter.
            "menu:F&le[Save]",
            "menu:a>b[Save]",
            "menu:A&B[]",
            // A menu *item list* crossing one, beside and instead of the
            // submenu caret the family already admitted.
            "menu:File[A & B]",
            "menu:File[Save & Exit > Now]",
            "menu:File[Save > A & B]",
            // Restored entities, the second recoverable piece: a key, a
            // label, a menu name, and an item.
            "kbd:[Ctrl&copy;C]",
            "btn:[Save &copy; Close]",
            "menu:&#8942;[More Tools, Extensions]",
            "menu:F&copy;le[Save]",
            "menu:File[Save &copy; Exit]",
            "menu:File[Save As&#8230;]",
            // Typographic replacements, the third recoverable piece: a key,
            // a label, a menu name, and an item, over both a `(C)`-style
            // replacement and a smart apostrophe.
            "kbd:[a(C)b]",
            "btn:[Save (C) Close]",
            "menu:File[Save (C) Exit]",
            "menu:File[Save > A (C) B]",
            "kbd:[O'Reilly]",
            "btn:[O'Reilly]",
            "menu:O'Reilly[Save]",
            "menu:File[O'Reilly]",
            // A match crossing every kind of recoverable piece at once.
            "kbd:[a&b&copy;c]",
            "menu:File[A & B &copy; C]",
            "menu:File[A & B &copy; C (C) D]",
            // In flow, doubled, and beside a sibling macro family.
            "press kbd:[Ctrl&C] to copy",
            "kbd:[a&b] then kbd:[c&d]",
            "menu:File[A & B] and btn:[Go &copy; Now]",
            "kbd:[a&b] then image:x.png[X]",
            // Inside a rendered span (the span encloses the macro rather than
            // crossing it, so the gate is satisfied at the span's own level).
            "*press kbd:[Ctrl&C]*",
            "_menu:File[A & B]_",
            // Escaped: the backslash is dropped and the rest stays literal,
            // entity and all.
            "\\kbd:[Ctrl&C]",
            "\\menu:File[A & B]",
            "\\btn:[Save &copy; Close]",
            // Beside — rather than crossing — a special.
            "a & kbd:[Enter] b",
        ];

        for fixture in fixtures {
            let folded = crate::content::inline_builder::fold_html(
                &build(Span::new(fixture), &parser, None),
                &renderer,
                &parser.render_context(),
            );

            assert_eq!(
                folded,
                golden_macros_in("macros_experimental", fixture, &parser),
                "fold diverged from the string pipeline for {fixture:?}"
            );
        }
    }

    #[test]
    fn a_kbd_macro_crossing_a_recoverable_piece_keeps_the_substituted_bytes() {
        // The structural counterpart of the parity corpus above: a `Ui` node's
        // values are *already-substituted* text (the contract an `IndexTerm`'s
        // `terms` already uses), because that is what `render_keyboard` and
        // friends receive from the string pipeline and emit verbatim. So a key
        // crossing an escaped special carries the entity, not the source's own
        // `&` — while the node's `location` still covers the macro in *source*
        // terms (the entity is one byte there, five in the match string).
        let nodes = build_ui(Span::new("kbd:[Ctrl&C]"));

        assert_eq!(nodes.len(), 1, "{nodes:?}");
        let ui = assert_ui(&nodes[0]);

        match &ui.kind {
            UiKind::Keyboard(keys) => assert_eq!(keys, &[CowStr::from("Ctrl&amp;C")]),

            other => panic!("expected Keyboard, got {other:?}"),
        }

        assert_eq!(ui.location.data(), "kbd:[Ctrl&C]");
        assert_eq!(ui.location.line(), 1);
        assert_eq!(ui.location.col(), 1);
    }

    #[test]
    fn a_menu_name_crossing_a_restored_entity_keeps_the_entity_bytes() {
        // The menu name is the one value this family used to slice from
        // `'src` (through `text_slice`). When it crosses a recoverable atomic
        // piece — here a restored entity, which `text_slice` declines because
        // it cannot slice one — the name comes from the match string instead,
        // in the same already-substituted form the string replacer's own
        // params carry. The item list, split from that same string, is
        // unaffected.
        let nodes = build_ui(Span::new("menu:&#8942;[More Tools, Extensions]"));

        assert_eq!(nodes.len(), 1, "{nodes:?}");
        let ui = assert_ui(&nodes[0]);

        match &ui.kind {
            UiKind::Menu {
                menu,
                submenus,
                item,
            } => {
                assert_eq!(menu.as_ref(), "&#8942;");
                assert_eq!(submenus, &[CowStr::from("More Tools")]);
                assert_eq!(item.as_deref(), Some("Extensions"));
            }

            other => panic!("expected Menu, got {other:?}"),
        }

        assert_eq!(ui.location.data(), "menu:&#8942;[More Tools, Extensions]");
    }

    #[test]
    fn a_ui_macro_crossing_an_opaque_piece_is_a_documented_divergence() {
        // The one boundary this family keeps, and the same one every other
        // macro family keeps: a match crossing an **opaque** piece — a
        // rendered span, an already-recognized macro node — is left
        // unrecognized. `build_match_string` stands each in as a single
        // placeholder where the string pipeline's own haystack holds the
        // markup it will fold to, so a value read out of the match string
        // would not be the replacer's, and reading the markup would mean
        // folding while building the tree (which this module never does).
        for source in [
            // A rendered span inside a keyboard, button, and menu macro.
            "kbd:[*a*]",
            "btn:[a `b` c]",
            "menu:File[*S* > As]",
        ] {
            let nodes = build_ui(Span::new(source));

            assert!(
                nodes.iter().all(|n| !matches!(n, InlineNode::Ui(_))),
                "a UI macro crossing an opaque piece must be left unrecognized: {nodes:?}"
            );

            // The string pipeline, by contrast, *does* build a UI macro here —
            // the divergence this test documents. (If a later increment lifts
            // it, fold these fixtures into the parity corpus above.)
            let golden = golden_macros_in("macros_experimental", source, &experimental_parser());

            assert!(
                golden.contains("<kbd>")
                    || golden.contains(r#"class="button"#)
                    || golden.contains(r#"class="menu"#),
                "expected the golden pipeline to build a UI macro for {source:?}"
            );
        }
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

    /// The real, public pipeline's output for `source` — the golden for the
    /// expanded-value fixtures, which need the `AttributeReferences` step the
    /// module's own [`golden_macros`] helper deliberately omits.
    fn golden_normal(source: &str, _parser: &Parser) -> String {
        crate::content::inline_builder::snapshot::recorded("ui_normal", source)
    }

    #[test]
    fn fold_matches_the_string_pipeline_for_ui_macros_inside_expanded_values() {
        // A UI macro whose name, keys, label, or item list crosses a
        // *synthesized* run (an attribute expansion) is now recognized: a
        // [`Ui`] node carries no `Span`-typed field, so every value it holds
        // comes straight from the match string — which carries a synthesized
        // run's bytes exactly — or, for the menu name, from `text_slice`. This
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
                    &HtmlInlineRenderer {},
                    &parser.render_context()
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

        let parser = experimental_parser();

        for (filtered, source) in [
            (
                "press kbd:[Ctrl+T]\nor menu:View[Zoom > Reset]",
                "  press kbd:[Ctrl+T]\n  or menu:View[Zoom > Reset]",
            ),
            // The recoverable pieces reached through a synthesized seed: a key
            // crossing an escaped special, a menu name crossing a restored
            // entity, and a label crossing a typographic replacement, none of
            // which has an `'src` slice of its own here — every value still
            // comes from the match string.
            (
                "press kbd:[Ctrl&C]\nor menu:&#8942;[Zoom > Reset]",
                "  press kbd:[Ctrl&C]\n  or menu:&#8942;[Zoom > Reset]",
            ),
            (
                "press kbd:[a(C)b]\nor btn:[O'Reilly]",
                "  press kbd:[a(C)b]\n  or btn:[O'Reilly]",
            ),
        ] {
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
                    &HtmlInlineRenderer {},
                    &parser.render_context()
                ),
                golden_normal(filtered, &parser),
                "fold diverged from the string pipeline for {filtered:?}"
            );
        }
    }

    #[test]
    fn a_real_documents_expanded_ui_macro_reaches_its_tree() {
        // End-to-end, through the real parse path: a document attribute whose
        // value feeds a UI macro. The rendered string and the fold of the
        // block's own tree agree, and the tree carries the recognized node
        // rather than the literal text it used to.
        use crate::blocks::{FindBlocks, IsBlock};

        let doc = Parser::default()
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
                &HtmlInlineRenderer {},
                &Parser::default().render_context()
            ),
            rendered,
            "fold diverged from the rendered string for {inlines:?}"
        );
    }

    #[test]
    fn a_real_documents_ui_macro_crossing_an_entity_reaches_its_tree() {
        // End-to-end, through the real parse path, for the piece class this
        // increment admits: a keyboard macro whose key crosses an escaped
        // special and a menu whose name crosses a restored entity. The
        // rendered string and the fold of the block's own tree agree, and the
        // tree carries recognized nodes rather than the literal text it used
        // to.
        use crate::blocks::{FindBlocks, IsBlock};

        let doc = Parser::default().parse(
            ":experimental:\n\nPress kbd:[Ctrl&C] then menu:&#8942;[More Tools, Extensions].",
        );

        let blocks: Vec<_> = doc.descendant_blocks().collect();
        let rendered = blocks[0].rendered_html_content().unwrap();
        let inlines = blocks[0].inlines().unwrap();

        assert!(rendered.contains("Ctrl&amp;C"), "rendered: {rendered}");
        assert!(rendered.contains("&#8942;"), "rendered: {rendered}");

        assert_eq!(
            inlines
                .iter()
                .filter(|n| matches!(n, InlineNode::Ui(_)))
                .count(),
            2,
            "expected both Ui nodes in the block's tree: {inlines:?}"
        );

        assert_eq!(
            crate::content::inline_builder::fold_html(
                inlines,
                &HtmlInlineRenderer {},
                &Parser::default().render_context()
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

        // The `&gt;` submenu delimiter takes precedence over a comma — checked
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
